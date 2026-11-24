#[cfg(unix)]
use std::fs::Permissions;
use std::{
    borrow::Cow,
    cmp,
    collections::VecDeque,
    ffi::OsStr,
    fmt,
    fs::FileTimes,
    marker,
    path::{Component, Path, PathBuf},
    pin::Pin,
    task::{Context, Poll},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

#[cfg(feature = "runtime-smol")]
use async_fs as fs;
use futures_core::ready;
#[cfg(feature = "runtime-smol")]
use futures_lite::io::{
    self, AsyncRead as Read, AsyncReadExt, AsyncSeekExt, Error, ErrorKind, SeekFrom,
};
#[cfg(feature = "runtime-tokio")]
use tokio::{
    fs,
    io::{self, AsyncRead as Read, AsyncReadExt, AsyncSeekExt, Error, ErrorKind, SeekFrom},
};

use crate::{
    Archive, COPY_CHUNK_SIZE, Header, PaxExtensions, error::TarError, fs_canonicalize,
    header::bytes2path, other, pax::pax_extensions, symlink_metadata,
};

/// A read-only view into an entry of an archive.
///
/// This structure is a window into a portion of a borrowed archive which can
/// be inspected. It acts as a file handle by implementing the Reader trait. An
/// entry cannot be rewritten once inserted into an archive.
pub struct Entry<R: Read + Unpin> {
    fields: EntryFields<R>,
    _ignored: marker::PhantomData<Archive<R>>,
}

impl<R: Read + Unpin> fmt::Debug for Entry<R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Entry")
            .field("fields", &self.fields)
            .finish()
    }
}

// private implementation detail of `Entry`, but concrete (no type parameters)
// and also all-public to be constructed from other modules.
pub struct EntryFields<R: Read + Unpin> {
    pub long_pathname: Option<Vec<u8>>,
    pub long_linkname: Option<Vec<u8>>,
    pub pax_extensions: Option<Vec<u8>>,
    pub header: Header,
    pub size: u64,
    pub header_pos: u64,
    pub file_pos: u64,
    pub data: VecDeque<EntryIo<R>>,
    pub unpack_xattrs: bool,
    pub preserve_permissions: bool,
    pub mask: u32,
    pub preserve_mtime: bool,
    pub(crate) read_state: Option<EntryIo<R>>,
}

impl<R: Read + Unpin> fmt::Debug for EntryFields<R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EntryFields")
            .field("long_pathname", &self.long_pathname)
            .field("long_linkname", &self.long_linkname)
            .field("pax_extensions", &self.pax_extensions)
            .field("header", &self.header)
            .field("size", &self.size)
            .field("header_pos", &self.header_pos)
            .field("file_pos", &self.file_pos)
            .field("data", &self.data)
            .field("unpack_xattrs", &self.unpack_xattrs)
            .field("preserve_permissions", &self.preserve_permissions)
            .field("mask", &self.mask)
            .field("preserve_mtime", &self.preserve_mtime)
            .field("read_state", &self.read_state)
            .finish()
    }
}

pub enum EntryIo<R: Read + Unpin> {
    Pad(io::Take<io::Repeat>),
    Data(io::Take<R>),
}

impl<R: Read + Unpin> fmt::Debug for EntryIo<R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EntryIo::Pad(_) => write!(f, "EntryIo::Pad"),
            EntryIo::Data(_) => write!(f, "EntryIo::Data"),
        }
    }
}

/// When unpacking items the unpacked thing is returned to allow custom
/// additional handling by users. Today the File is returned, in future
/// the enum may be extended with kinds for links, directories etc.
#[derive(Debug)]
#[non_exhaustive]
pub enum Unpacked {
    /// A file was unpacked.
    File(fs::File),
    /// A directory, hardlink, symlink, or other node was unpacked.
    Other,
}

impl<R: Read + Unpin> Entry<R> {
    /// Returns the path name for this entry.
    ///
    /// This method may fail if the pathname is not valid Unicode and this is
    /// called on a Windows platform.
    ///
    /// Note that this function will convert any `\` characters to directory
    /// separators, and it will not always return the same value as
    /// `self.header().path()` as some archive formats have support for longer
    /// path names described in separate entries.
    ///
    /// It is recommended to use this method instead of inspecting the `header`
    /// directly to ensure that various archive formats are handled correctly.
    pub fn path(&self) -> io::Result<Cow<'_, Path>> {
        self.fields.path()
    }

    /// Returns the raw bytes listed for this entry.
    ///
    /// Note that this function will convert any `\` characters to directory
    /// separators, and it will not always return the same value as
    /// `self.header().path_bytes()` as some archive formats have support for
    /// longer path names described in separate entries.
    pub fn path_bytes(&self) -> Cow<'_, [u8]> {
        self.fields.path_bytes()
    }

    /// Returns the link name for this entry, if any is found.
    ///
    /// This method may fail if the pathname is not valid Unicode and this is
    /// called on a Windows platform. `Ok(None)` being returned, however,
    /// indicates that the link name was not present.
    ///
    /// Note that this function will convert any `\` characters to directory
    /// separators, and it will not always return the same value as
    /// `self.header().link_name()` as some archive formats have support for
    /// longer path names described in separate entries.
    ///
    /// It is recommended to use this method instead of inspecting the `header`
    /// directly to ensure that various archive formats are handled correctly.
    pub fn link_name(&self) -> io::Result<Option<Cow<'_, Path>>> {
        self.fields.link_name()
    }

    /// Returns the link name for this entry, in bytes, if listed.
    ///
    /// Note that this will not always return the same value as
    /// `self.header().link_name_bytes()` as some archive formats have support for
    /// longer path names described in separate entries.
    pub fn link_name_bytes(&self) -> Option<Cow<'_, [u8]>> {
        self.fields.link_name_bytes()
    }

    /// Returns an iterator over the pax extensions contained in this entry.
    ///
    /// Pax extensions are a form of archive where extra metadata is stored in
    /// key/value pairs in entries before the entry they're intended to
    /// describe. For example this can be used to describe long file name or
    /// other metadata like atime/ctime/mtime in more precision.
    ///
    /// The returned iterator will yield key/value pairs for each extension.
    ///
    /// `None` will be returned if this entry does not indicate that it itself
    /// contains extensions, or if there were no previous extensions describing
    /// it.
    ///
    /// Note that global pax extensions are intended to be applied to all
    /// archive entries.
    ///
    /// Also note that this function will read the entire entry if the entry
    /// itself is a list of extensions.
    pub async fn pax_extensions(&mut self) -> io::Result<Option<PaxExtensions<'_>>> {
        self.fields.pax_extensions().await
    }

    /// Returns access to the header of this entry in the archive.
    ///
    /// This provides access to the metadata for this entry in the archive.
    pub fn header(&self) -> &Header {
        &self.fields.header
    }

    /// Returns the starting position, in bytes, of the header of this entry in
    /// the archive.
    ///
    /// The header is always a contiguous section of 512 bytes, so if the
    /// underlying reader implements `Seek`, then the slice from `header_pos` to
    /// `header_pos + 512` contains the raw header bytes.
    pub fn raw_header_position(&self) -> u64 {
        self.fields.header_pos
    }

    /// Returns the starting position, in bytes, of the file of this entry in
    /// the archive.
    ///
    /// If the file of this entry is continuous (e.g. not a sparse file), and
    /// if the underlying reader implements `Seek`, then the slice from
    /// `file_pos` to `file_pos + entry_size` contains the raw file bytes.
    pub fn raw_file_position(&self) -> u64 {
        self.fields.file_pos
    }

    /// Writes this file to the specified location.
    ///
    /// This function will write the entire contents of this file into the
    /// location specified by `dst`. Metadata will also be propagated to the
    /// path `dst`.
    ///
    /// This function will create a file at the path `dst`, and it is required
    /// that the intermediate directories are created. Any existing file at the
    /// location `dst` will be overwritten.
    ///
    /// > **Note**: This function does not have as many sanity checks as
    /// > `Archive::unpack` or `Entry::unpack_in`. As a result if you're
    /// > thinking of unpacking untrusted tarballs you may want to review the
    /// > implementations of the previous two functions and perhaps implement
    /// > similar logic yourself.
    ///
    /// # Examples
    #[cfg_attr(feature = "runtime-smol", doc = "```no_run")]
    #[cfg_attr(feature = "runtime-tokio", doc = "```ignore")]
    /// # fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> { smol::block_on(async {
    /// #
    /// use smol::fs::File;
    /// use smol::stream::StreamExt;
    /// use async_tar::Archive;
    ///
    /// let mut ar = Archive::new(File::open("foo.tar").await?);
    /// let mut entries = ar.entries()?;
    /// let mut i = 0;
    /// while let Some(file) = entries.next().await {
    ///     let mut file = file?;
    ///     file.unpack(format!("file-{}", i)).await?;
    ///     i += 1;
    /// }
    /// #
    /// # Ok(()) }) }
    /// ```
    pub async fn unpack<P: AsRef<Path>>(&mut self, dst: P) -> io::Result<Unpacked> {
        self.fields.unpack(None, dst.as_ref()).await
    }

    /// Extracts this file under the specified path, avoiding security issues.
    ///
    /// This function will write the entire contents of this file into the
    /// location obtained by appending the path of this file in the archive to
    /// `dst`, creating any intermediate directories if needed. Metadata will
    /// also be propagated to the path `dst`. Any existing file at the location
    /// `dst` will be overwritten.
    ///
    /// This function carefully avoids writing outside of `dst`. If the file has
    /// a '..' in its path, this function will skip it and return false.
    ///
    /// # Examples
    #[cfg_attr(feature = "runtime-smol", doc = "```no_run")]
    #[cfg_attr(feature = "runtime-tokio", doc = "```ignore")]
    /// # fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> { smol::block_on(async {
    /// #
    /// use smol::fs::File;
    /// use async_tar::Archive;
    /// use smol::stream::StreamExt;
    ///
    /// let mut ar = Archive::new(File::open("foo.tar").await?);
    /// let mut entries = ar.entries()?;
    /// let mut i = 0;
    /// while let Some(file) = entries.next().await {
    ///     let mut file = file.unwrap();
    ///     file.unpack_in("target").await?;
    ///     i += 1;
    /// }
    /// #
    /// # Ok(()) }) }
    /// ```
    pub async fn unpack_in<P: AsRef<Path>>(&mut self, dst: P) -> io::Result<bool> {
        self.fields.unpack_in(dst.as_ref(), None).await
    }

    /// Unpacks this entry into `dst`, reusing `canon_dst` as its canonical form when given.
    pub(crate) async fn unpack_in_canonical(
        &mut self,
        dst: &Path,
        canon_dst: Option<&Path>,
    ) -> io::Result<bool> {
        self.fields.unpack_in(dst, canon_dst).await
    }

    /// Reads the small regular file this entry unpacks to under `dst`.
    pub(crate) async fn read_small_file_in(&mut self, dst: &Path) -> io::Result<Option<SmallFile>> {
        self.fields.read_small_file_in(dst).await
    }

    /// Indicate whether extended file attributes (xattrs on Unix) are preserved
    /// when unpacking this entry.
    ///
    /// This flag is disabled by default and is currently only implemented on
    /// Unix using xattr support. This may eventually be implemented for
    /// Windows, however, if other archive implementations are found which do
    /// this as well.
    pub fn set_unpack_xattrs(&mut self, unpack_xattrs: bool) {
        self.fields.unpack_xattrs = unpack_xattrs;
    }

    /// Indicate whether extended permissions (like suid on Unix) are preserved
    /// when unpacking this entry.
    ///
    /// This flag is disabled by default and is currently only implemented on
    /// Unix.
    pub fn set_preserve_permissions(&mut self, preserve: bool) {
        self.fields.preserve_permissions = preserve;
    }

    /// Sets the permission bits cleared when unpacking, as a umask clears them.
    ///
    /// The default is the archive's mask. Only Unix applies a mask.
    pub fn set_mask(&mut self, mask: u32) {
        self.fields.mask = mask;
    }

    /// Indicate whether access time information is preserved when unpacking
    /// this entry.
    ///
    /// This flag is enabled by default.
    pub fn set_preserve_mtime(&mut self, preserve: bool) {
        self.fields.preserve_mtime = preserve;
    }
}

#[cfg(feature = "runtime-smol")]
impl<R: Read + Unpin> Read for Entry<R> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        into: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.fields).poll_read(cx, into)
    }
}

#[cfg(feature = "runtime-tokio")]
impl<R: Read + Unpin> Read for Entry<R> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        into: &mut tokio::io::ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut self.fields).poll_read(cx, into)
    }
}

impl<R: Read + Unpin> EntryFields<R> {
    pub fn from(entry: Entry<R>) -> Self {
        entry.fields
    }

    pub fn into_entry(self) -> Entry<R> {
        Entry {
            fields: self,
            _ignored: marker::PhantomData,
        }
    }

    /// Appends the rest of this entry to `out`. The caller keeps `out` across a `Pending`.
    pub(crate) fn poll_read_all(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        out: &mut Vec<u8>,
    ) -> Poll<io::Result<()>> {
        // Preallocate some data but don't let ourselves get too crazy now.
        if out.capacity() == 0 {
            out.reserve(cmp::min(self.size, 128 * 1024) as usize);
        }

        // Copied from futures::ReadToEnd
        ready!(poll_read_all_internal(self, cx, out))?;
        Poll::Ready(Ok(()))
    }

    pub async fn read_all(&mut self) -> io::Result<Vec<u8>> {
        // Preallocate some data but don't let ourselves get too crazy now.
        let cap = cmp::min(self.size, 128 * 1024);
        let mut v = Vec::with_capacity(cap as usize);
        self.read_to_end(&mut v).await.map(|_| v)
    }

    fn path(&self) -> io::Result<Cow<'_, Path>> {
        bytes2path(self.path_bytes())
    }

    pub(crate) fn path_bytes(&self) -> Cow<'_, [u8]> {
        if let Some(ref bytes) = self.long_pathname {
            if let Some(&0) = bytes.last() {
                Cow::Borrowed(&bytes[..bytes.len() - 1])
            } else {
                Cow::Borrowed(bytes)
            }
        } else {
            if let Some(field) = self.pax_value(b"path") {
                return Cow::Borrowed(field);
            }
            self.header.path_bytes()
        }
    }

    pub(crate) fn pax_value(&self, key: &[u8]) -> Option<&[u8]> {
        let pax = self.pax_extensions.as_ref()?;
        // POSIX gives the last of repeated records precedence, as GNU tar does.
        pax_extensions(pax)
            .filter_map(Result::ok)
            .filter(|f| f.key_bytes() == key)
            .last()
            .map(|f| f.value_bytes())
    }

    /// Gets the path in a "lossy" way, used for error reporting ONLY.
    fn path_lossy(&self) -> String {
        String::from_utf8_lossy(&self.path_bytes()).to_string()
    }

    fn link_name(&self) -> io::Result<Option<Cow<'_, Path>>> {
        match self.link_name_bytes() {
            Some(bytes) => bytes2path(bytes).map(Some),
            None => Ok(None),
        }
    }

    pub(crate) fn link_name_bytes(&self) -> Option<Cow<'_, [u8]>> {
        match self.long_linkname {
            Some(ref bytes) => {
                if let Some(&0) = bytes.last() {
                    Some(Cow::Borrowed(&bytes[..bytes.len() - 1]))
                } else {
                    Some(Cow::Borrowed(bytes))
                }
            }
            None => match self.pax_value(b"linkpath") {
                Some(field) => Some(Cow::Borrowed(field)),
                None => self.header.link_name_bytes(),
            },
        }
    }

    async fn pax_extensions(&mut self) -> io::Result<Option<PaxExtensions<'_>>> {
        if self.pax_extensions.is_none() {
            if !self.header.entry_type().is_pax_global_extensions()
                && !self.header.entry_type().is_pax_local_extensions()
            {
                return Ok(None);
            }
            self.pax_extensions = Some(self.read_all().await?);
        }
        Ok(Some(pax_extensions(self.pax_extensions.as_ref().unwrap())))
    }

    async fn unpack_in(&mut self, dst: &Path, canon_dst: Option<&Path>) -> io::Result<bool> {
        // A metadata entry unpacks to no file and makes no directories above its name.
        if self.header.entry_type().is_metadata() {
            return Ok(true);
        }

        let Some(file_dst) = self.join_path(dst)? else {
            return Ok(false);
        };

        // A name of only slashes, `.` parts and drive prefixes joins to `dst` itself,
        // so nothing is unpacked.
        if *dst == *file_dst {
            return Ok(true);
        }

        // Skip entries without a parent (i.e. outside of FS root)
        let parent = match file_dst.parent() {
            Some(p) => p,
            None => return Ok(false),
        };

        let canon_dst = match canon_dst {
            Some(canon_dst) => canon_dst.to_path_buf(),
            None => fs_canonicalize(dst).await.map_err(|err| {
                Error::new(
                    err.kind(),
                    format!("{} while canonicalizing {}", err, dst.display()),
                )
            })?,
        };

        if !self.unpack_xattrs {
            let small = self.read_small_file(&file_dst).await.map_err(|e| {
                TarError::new(&format!("failed to unpack `{}`", file_dst.display()), e)
            })?;
            if let Some(small) = small {
                crate::unblock(move || small.unpack_in(&canon_dst)).await?;
                return Ok(true);
            }
        }

        let (canon, dir) = (canon_dst.clone(), parent.to_path_buf());
        crate::unblock(move || create_dir_inside(&canon, &dir)).await?;

        self.unpack(Some(&canon_dst), &file_dst)
            .await
            .map_err(|e| TarError::new(&format!("failed to unpack `{}`", file_dst.display()), e))?;

        Ok(true)
    }

    /// Reads the small regular file this entry unpacks to under `dst`.
    ///
    /// Returns `None`, reading nothing, for any other entry, or for a skipped or
    /// metadata entry.
    pub(crate) async fn read_small_file_in(&mut self, dst: &Path) -> io::Result<Option<SmallFile>> {
        if self.unpack_xattrs || self.header.entry_type().is_metadata() {
            return Ok(None);
        }
        let Some(file_dst) = self.join_path(dst)? else {
            return Ok(None);
        };
        if *dst == *file_dst || file_dst.parent().is_none() {
            return Ok(None);
        }
        self.read_small_file(&file_dst).await.map_err(|e| {
            TarError::new(&format!("failed to unpack `{}`", file_dst.display()), e).into()
        })
    }

    /// Joins this entry's path to `dst`, returning `None` for a path with a `..` component.
    fn join_path(&self, dst: &Path) -> io::Result<Option<PathBuf>> {
        // Notes regarding bsdtar 2.8.3 / libarchive 2.8.3:
        // * Leading '/'s are trimmed. For example, `///test` is treated as
        //   `test`.
        // * If the filename contains '..', then the file is skipped when
        //   extracting the tarball.
        // * '//' within a filename is effectively skipped. An error is
        //   logged, but otherwise the effect is as if any two or more
        //   adjacent '/'s within the filename were consolidated into one
        //   '/'.
        //
        // Most of this is handled by the `path` module of the standard
        // library, but we specially handle a few cases here as well.

        let mut file_dst = dst.to_path_buf();
        {
            let path = self.path().map_err(|e| {
                TarError::new(
                    &format!("invalid path in entry header: {}", self.path_lossy()),
                    e,
                )
            })?;
            for part in path.components() {
                match part {
                    // Leading '/' characters, root paths, and '.'
                    // components are just ignored and treated as "empty
                    // components"
                    Component::Prefix(..) | Component::RootDir | Component::CurDir => continue,

                    // If any part of the filename is '..', then skip over
                    // unpacking the file to prevent directory traversal
                    // security issues.  See, e.g.: CVE-2001-1267,
                    // CVE-2002-0399, CVE-2005-1918, CVE-2007-4131
                    Component::ParentDir => return Ok(None),

                    Component::Normal(part) if is_windows_reserved(part) => {
                        return Err(other(&format!(
                            "a name Windows reads as a drive, stream or device: {}",
                            self.path_lossy()
                        )));
                    }

                    Component::Normal(part) => file_dst.push(part),
                }
            }
        }
        Ok(Some(file_dst))
    }

    /// Reads this entry as a small regular file to write at `dst`.
    ///
    /// Returns `None`, reading nothing, for any other entry.
    async fn read_small_file(&mut self, dst: &Path) -> io::Result<Option<SmallFile>> {
        let kind = self.header.entry_type();
        if kind.is_dir() || kind.is_hard_link() || kind.is_symlink() || kind.is_metadata() {
            return Ok(None);
        }
        // A name ending in a slash in an old header is a directory, as in `unpack`.
        if self.header.as_ustar().is_none() && self.path_bytes().ends_with(b"/") {
            return Ok(None);
        }
        let mut data = match self.data.front() {
            Some(EntryIo::Data(d))
                if self.data.len() == 1 && d.limit() <= COPY_CHUNK_SIZE as u64 =>
            {
                match self.data.pop_front() {
                    Some(EntryIo::Data(d)) => d,
                    _ => return Ok(None),
                }
            }
            _ => return Ok(None),
        };

        let name = String::from_utf8_lossy(&self.header.path_bytes()).into_owned();
        let expected = data.limit();
        let mut bytes = Vec::with_capacity(expected as usize);
        let read = data.read_to_end(&mut bytes).await.and_then(|n| {
            if n as u64 == expected {
                Ok(())
            } else {
                Err(other("failed to write entire file"))
            }
        });
        if let Err(err) = read {
            return Err(TarError::new(
                &format!("failed to unpack `{}` into `{}`", name, dst.display()),
                err,
            )
            .into());
        }

        // An mtime beyond the range of `SystemTime` stays unset.
        let mtime = self
            .header
            .mtime()
            .ok()
            .and_then(|mtime| UNIX_EPOCH.checked_add(Duration::from_secs(mtime)))
            .filter(|_| self.preserve_mtime);
        Ok(Some(SmallFile {
            dst: dst.to_path_buf(),
            name,
            data: bytes,
            mtime,
            mode: self.header.mode().ok(),
            preserve_permissions: self.preserve_permissions,
            mask: self.mask,
        }))
    }

    /// Unpack as destination directory `dst`.
    async fn unpack_dir(&mut self, dst: &Path) -> io::Result<()> {
        // If the directory already exists just let it slide
        match fs::create_dir(dst).await {
            Ok(()) => Ok(()),
            Err(err) => {
                if err.kind() == ErrorKind::AlreadyExists {
                    // A symlink must fail here, or set_perms would chmod its target.
                    let prev = symlink_metadata(dst).await;
                    if prev.map(|m| m.is_dir()).unwrap_or(false) {
                        return Ok(());
                    }
                }
                Err(Error::new(
                    err.kind(),
                    format!("{} when creating dir {}", err, dst.display()),
                ))
            }
        }
    }

    /// Returns access to the header of this entry in the archive.
    async fn unpack(&mut self, target_base: Option<&Path>, dst: &Path) -> io::Result<Unpacked> {
        let kind = self.header.entry_type();

        if kind.is_dir() {
            self.unpack_dir(dst).await?;
            if let Ok(mode) = self.header.mode() {
                set_perms(dst, None, mode, self.preserve_permissions, self.mask).await?;
            }
            return Ok(Unpacked::Other);
        } else if kind.is_hard_link() || kind.is_symlink() {
            let src = match self.link_name()? {
                Some(name) => name,
                None => {
                    return Err(other(&format!(
                        "hard link listed for {} but no link name found",
                        String::from_utf8_lossy(self.header.as_bytes())
                    )));
                }
            };

            if src.iter().count() == 0 {
                return Err(other(&format!(
                    "symlink destination for {} is empty",
                    String::from_utf8_lossy(self.header.as_bytes())
                )));
            }

            if kind.is_hard_link() {
                let link_src = match target_base {
                    // If we're unpacking within a directory then ensure that
                    // the destination of this hard link is both present and
                    // inside our own directory. This is needed because we want
                    // to make sure to not overwrite anything outside the root.
                    //
                    // Note that this logic is only needed for hard links
                    // currently. With symlinks the `validate_inside_dst` which
                    // happens before this method as part of `unpack_in` will
                    // use canonicalization to ensure this guarantee. For hard
                    // links though they're canonicalized to their existing path
                    // so we need to validate at this time.
                    Some(p) => {
                        let link_src = p.join(src);
                        let (canon_dst, path) = (p.to_path_buf(), link_src.clone());
                        crate::unblock(move || validate_inside_dst(&canon_dst, &path)).await?;
                        link_src
                    }
                    None => src.into_owned(),
                };
                fs::hard_link(&link_src, dst).await.map_err(|err| {
                    Error::new(
                        err.kind(),
                        format!(
                            "{} when hard linking {} to {}",
                            err,
                            link_src.display(),
                            dst.display()
                        ),
                    )
                })?;
            } else {
                symlink(&src, dst).await.map_err(|err| {
                    Error::new(
                        err.kind(),
                        format!(
                            "{} when symlinking {} to {}",
                            err,
                            src.display(),
                            dst.display()
                        ),
                    )
                })?;
            };
            return Ok(Unpacked::Other);

            #[cfg(target_arch = "wasm32")]
            #[allow(unused_variables)]
            #[expect(
                clippy::unused_async,
                reason = "keeps the signature of the other platforms"
            )]
            async fn symlink(src: &Path, dst: &Path) -> io::Result<()> {
                Err(io::Error::other("Not implemented"))
            }

            #[cfg(windows)]
            async fn symlink(src: &Path, dst: &Path) -> io::Result<()> {
                #[cfg(feature = "runtime-smol")]
                {
                    async_fs::windows::symlink_file(src, dst).await
                }
                #[cfg(feature = "runtime-tokio")]
                {
                    tokio::fs::symlink_file(src, dst).await
                }
            }

            #[cfg(any(unix, target_os = "redox"))]
            async fn symlink(src: &Path, dst: &Path) -> io::Result<()> {
                #[cfg(feature = "runtime-smol")]
                async_fs::unix::symlink(src, dst).await?;
                #[cfg(feature = "runtime-tokio")]
                tokio::fs::symlink(src, dst).await?;

                Ok(())
            }
        } else if kind.is_metadata() {
            return Ok(Unpacked::Other);
        };

        // Old BSD-tar compatibility.
        // Names that have a trailing slash should be treated as a directory.
        // Only applies to old headers.
        if self.header.as_ustar().is_none() && self.path_bytes().ends_with(b"/") {
            self.unpack_dir(dst).await?;
            if let Ok(mode) = self.header.mode() {
                set_perms(dst, None, mode, self.preserve_permissions, self.mask).await?;
            }
            return Ok(Unpacked::Other);
        }

        // Note the lack of `else` clause above. According to the FreeBSD
        // documentation:
        //
        // > A POSIX-compliant implementation must treat any unrecognized
        // > typeflag value as a regular file.
        //
        // As a result if we don't recognize the kind we just write out the file
        // as we would normally.

        // A small file is read from the archive first, then written in one blocking call.
        if let Some(small) = self.read_small_file(dst).await? {
            let file = crate::unblock(move || small.write()).await?;
            if self.unpack_xattrs {
                set_xattrs(self, dst).await?;
            }
            return Ok(Unpacked::File(fs::File::from(file)));
        }

        // Ensure we write a new file rather than overwriting in-place which
        // is attackable; if an existing file is found unlink it.
        // An mtime beyond the range of `SystemTime` stays unset.
        let mtime = self
            .header
            .mtime()
            .ok()
            .and_then(|mtime| UNIX_EPOCH.checked_add(Duration::from_secs(mtime)))
            .filter(|_| self.preserve_mtime);

        let mut f = async {
            let dst = dst.to_path_buf();
            let mut f = fs::File::from(crate::unblock(move || create_new_replacing(&dst)).await?);
            for io in self.data.drain(..) {
                match io {
                    EntryIo::Data(mut d) => {
                        let expected = d.limit();
                        if crate::copy(&mut d, &mut f, expected).await? != expected {
                            return Err(other("failed to write entire file"));
                        }
                    }
                    EntryIo::Pad(d) => {
                        // TODO: checked cast to i64
                        let to = SeekFrom::Current(d.limit() as i64);
                        let size = f.seek(to).await?;
                        f.set_len(size).await?;
                    }
                }
            }
            Ok::<fs::File, io::Error>(f)
        }
        .await
        .map_err(|e| {
            let header = self.header.path_bytes();
            TarError::new(
                &format!(
                    "failed to unpack `{}` into `{}`",
                    String::from_utf8_lossy(&header),
                    dst.display()
                ),
                e,
            )
        })?;

        if let Some(mtime) = mtime {
            let times = FileTimes::new().set_accessed(mtime).set_modified(mtime);
            std::fs::OpenOptions::new()
                .write(true)
                .open(dst)
                .and_then(|file| file.set_times(times))
                .map_err(|e| {
                    TarError::new(&format!("failed to set mtime for `{}`", dst.display()), e)
                })?;
        }
        if let Ok(mode) = self.header.mode() {
            set_perms(
                dst,
                Some(&mut f),
                mode,
                self.preserve_permissions,
                self.mask,
            )
            .await?;
        }
        if self.unpack_xattrs {
            set_xattrs(self, dst).await?;
        }
        return Ok(Unpacked::File(f));

        async fn set_perms(
            dst: &Path,
            f: Option<&mut fs::File>,
            mode: u32,
            preserve: bool,
            mask: u32,
        ) -> Result<(), TarError> {
            _set_perms(dst, f, mode, preserve, mask).await.map_err(|e| {
                TarError::new(
                    &format!(
                        "failed to set permissions to {:o} \
                         for `{}`",
                        mode,
                        dst.display()
                    ),
                    e,
                )
            })
        }

        #[cfg(any(unix, target_os = "redox"))]
        async fn _set_perms(
            dst: &Path,
            f: Option<&mut fs::File>,
            mode: u32,
            preserve: bool,
            mask: u32,
        ) -> io::Result<()> {
            use std::os::unix::prelude::*;

            let mode = if preserve { mode } else { mode & 0o777 } & !mask;
            let perm = Permissions::from_mode(mode as _);
            match f {
                Some(f) => f.set_permissions(perm).await,
                None => fs::set_permissions(dst, perm).await,
            }
        }

        #[cfg(windows)]
        async fn _set_perms(
            dst: &Path,
            f: Option<&mut fs::File>,
            mode: u32,
            _preserve: bool,
            _mask: u32,
        ) -> io::Result<()> {
            if mode & 0o200 == 0o200 {
                return Ok(());
            }
            match f {
                Some(f) => {
                    let mut perm = f.metadata().await?.permissions();
                    perm.set_readonly(true);
                    f.set_permissions(perm).await
                }
                None => {
                    let mut perm = fs::metadata(dst).await?.permissions();
                    perm.set_readonly(true);
                    fs::set_permissions(dst, perm).await
                }
            }
        }

        #[cfg(target_arch = "wasm32")]
        #[allow(unused_variables)]
        #[expect(
            clippy::unused_async,
            reason = "keeps the signature of the other platforms"
        )]
        async fn _set_perms(
            dst: &Path,
            f: Option<&mut fs::File>,
            mode: u32,
            _preserve: bool,
            _mask: u32,
        ) -> io::Result<()> {
            Err(io::Error::other("Not implemented"))
        }

        #[cfg(all(unix, feature = "xattr"))]
        async fn set_xattrs<R: Read + Unpin>(
            me: &mut EntryFields<R>,
            dst: &Path,
        ) -> io::Result<()> {
            use std::{ffi::OsStr, os::unix::prelude::*};

            let exts = match me.pax_extensions().await {
                Ok(Some(e)) => e,
                _ => return Ok(()),
            };
            let exts = exts
                .filter_map(Result::ok)
                .filter_map(|e| {
                    let key = e.key_bytes();
                    let prefix = b"SCHILY.xattr.";
                    if key.starts_with(prefix) {
                        Some((&key[prefix.len()..], e))
                    } else {
                        None
                    }
                })
                .map(|(key, e)| (OsStr::from_bytes(key), e.value_bytes()));

            for (key, value) in exts {
                xattr::set(dst, key, value).map_err(|e| {
                    TarError::new(
                        &format!(
                            "failed to set extended \
                             attributes to {}. \
                             Xattrs: key={:?}, value={:?}.",
                            dst.display(),
                            key,
                            String::from_utf8_lossy(value)
                        ),
                        e,
                    )
                })?;
            }

            Ok(())
        }
        // Windows does not completely support posix xattrs
        // https://en.wikipedia.org/wiki/Extended_file_attributes#Windows_NT
        #[cfg(any(
            windows,
            target_os = "redox",
            not(feature = "xattr"),
            target_arch = "wasm32"
        ))]
        // This keeps the async signature of the xattr version.
        #[allow(clippy::unused_async)]
        async fn set_xattrs<R: Read + Unpin>(_: &mut EntryFields<R>, _: &Path) -> io::Result<()> {
            Ok(())
        }
    }
}

/// A regular file small enough to unpack from memory in one blocking call.
pub(crate) struct SmallFile {
    dst: PathBuf,
    name: String,
    data: Vec<u8>,
    mtime: Option<SystemTime>,
    mode: Option<u32>,
    preserve_permissions: bool,
    mask: u32,
}

impl SmallFile {
    /// Returns the size of the file's data.
    pub(crate) fn len(&self) -> usize {
        self.data.len()
    }

    /// Creates the file's missing parent directories inside `canon_dst`, then writes it.
    pub(crate) fn unpack_in(&self, canon_dst: &Path) -> io::Result<std::fs::File> {
        if let Some(parent) = self.dst.parent() {
            create_dir_inside(canon_dst, parent)?;
        }
        self.write().map_err(|e| {
            TarError::new(&format!("failed to unpack `{}`", self.dst.display()), e).into()
        })
    }

    /// Writes the file with its mtime and permissions, replacing anything at its path.
    fn write(&self) -> io::Result<std::fs::File> {
        let file = self.write_data().map_err(|e| {
            TarError::new(
                &format!(
                    "failed to unpack `{}` into `{}`",
                    self.name,
                    self.dst.display()
                ),
                e,
            )
        })?;
        if let Some(mode) = self.mode {
            set_file_perms(&file, mode, self.preserve_permissions, self.mask).map_err(|e| {
                TarError::new(
                    &format!(
                        "failed to set permissions to {:o} for `{}`",
                        mode,
                        self.dst.display()
                    ),
                    e,
                )
            })?;
        }
        Ok(file)
    }

    fn write_data(&self) -> io::Result<std::fs::File> {
        use std::io::Write as _;

        // A new file is created, unlinking any existing one, since writing in place is attackable.
        let mut file = create_new_replacing(&self.dst)?;
        file.write_all(&self.data)?;
        if let Some(mtime) = self.mtime {
            let times = FileTimes::new().set_accessed(mtime).set_modified(mtime);
            file.set_times(times).map_err(|e| {
                Error::new(
                    e.kind(),
                    format!("{e} when setting the mtime of `{}`", self.dst.display()),
                )
            })?;
        }
        Ok(file)
    }
}

/// Creates `dir` and its missing ancestors, and checks each resolves inside `canon_dst`.
fn create_dir_inside(canon_dst: &Path, dir: &Path) -> io::Result<()> {
    create_missing_dirs(canon_dst, dir)
        .map_err(|e| TarError::new(&format!("failed to create `{}`", dir.display()), e))?;
    validate_inside_dst(canon_dst, dir)
}

fn create_missing_dirs(canon_dst: &Path, dir: &Path) -> io::Result<()> {
    let mut ancestor = dir;
    let mut dirs_to_create = Vec::new();

    // Any error but NotFound, such as a path too long for the OS, ends the walk.
    while let Err(err) = std::fs::symlink_metadata(ancestor) {
        if err.kind() != ErrorKind::NotFound {
            return Err(err);
        }
        dirs_to_create.push(ancestor);
        if let Some(parent) = ancestor.parent() {
            ancestor = parent;
        } else {
            break;
        }
    }
    for ancestor in dirs_to_create.into_iter().rev() {
        if let Some(parent) = ancestor.parent() {
            validate_inside_dst(canon_dst, parent)?;
        }
        std::fs::create_dir(ancestor)?;
    }
    Ok(())
}

fn validate_inside_dst(canon_dst: &Path, file_dst: &Path) -> io::Result<()> {
    // Abort if target (canonical) parent is outside of `dst`
    let canon_parent = std::fs::canonicalize(file_dst).map_err(|err| {
        Error::new(
            err.kind(),
            format!("{} while canonicalizing {}", err, file_dst.display()),
        )
    })?;
    if !canon_parent.starts_with(canon_dst) {
        let err = TarError::new(
            &format!(
                "trying to unpack outside of destination path: {}",
                canon_dst.display()
            ),
            // TODO: use ErrorKind::InvalidInput here? (minor breaking change)
            Error::other("Invalid argument"),
        );
        return Err(err.into());
    }
    Ok(())
}

#[cfg(any(unix, target_os = "redox"))]
fn set_file_perms(file: &std::fs::File, mode: u32, preserve: bool, mask: u32) -> io::Result<()> {
    use std::os::unix::prelude::*;

    let mode = if preserve { mode } else { mode & 0o777 } & !mask;
    file.set_permissions(Permissions::from_mode(mode as _))
}

#[cfg(windows)]
fn set_file_perms(file: &std::fs::File, mode: u32, _preserve: bool, _mask: u32) -> io::Result<()> {
    if mode & 0o200 == 0o200 {
        return Ok(());
    }
    let mut perm = file.metadata()?.permissions();
    perm.set_readonly(true);
    file.set_permissions(perm)
}

#[cfg(target_arch = "wasm32")]
#[allow(unused_variables)]
fn set_file_perms(file: &std::fs::File, mode: u32, preserve: bool, mask: u32) -> io::Result<()> {
    Err(io::Error::other("Not implemented"))
}

#[cfg(feature = "runtime-smol")]
impl<R: Read + Unpin> Read for EntryFields<R> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        into: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        loop {
            if self.read_state.is_none() {
                self.read_state = self.data.pop_front();
            }

            if let Some(io) = &mut self.read_state {
                let ret = Pin::new(io).poll_read(cx, into);
                match ret {
                    Poll::Ready(Ok(0)) => {
                        self.read_state = None;
                        if self.data.is_empty() {
                            return Poll::Ready(Ok(0));
                        }
                        continue;
                    }
                    Poll::Ready(Ok(val)) => {
                        return Poll::Ready(Ok(val));
                    }
                    Poll::Ready(Err(err)) => {
                        return Poll::Ready(Err(err));
                    }
                    Poll::Pending => {
                        return Poll::Pending;
                    }
                }
            }
            // Unable to pull another value from `data`, so we are done.
            return Poll::Ready(Ok(0));
        }
    }
}

#[cfg(feature = "runtime-tokio")]
impl<R: Read + Unpin> Read for EntryFields<R> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        into: &mut tokio::io::ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        loop {
            if self.read_state.is_none() {
                self.read_state = self.data.pop_front();
            }

            if let Some(io) = &mut self.read_state {
                let start = into.filled().len();
                let ret = Pin::new(io).poll_read(cx, into);
                match ret {
                    Poll::Ready(Ok(())) => {
                        let diff = into.filled().len() - start;
                        if diff == 0 {
                            self.read_state = None;
                            if self.data.is_empty() {
                                return Poll::Ready(Ok(()));
                            }
                            continue;
                        } else {
                            return Poll::Ready(Ok(()));
                        }
                    }
                    Poll::Ready(Err(err)) => {
                        return Poll::Ready(Err(err));
                    }
                    Poll::Pending => {
                        return Poll::Pending;
                    }
                }
            }
            // Unable to pull another value from `data`, so we are done.
            return Poll::Ready(Ok(()));
        }
    }
}

#[cfg(feature = "runtime-smol")]
impl<R: Read + Unpin> Read for EntryIo<R> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        into: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        match &mut *self {
            EntryIo::Pad(io) => Pin::new(io).poll_read(cx, into),
            EntryIo::Data(io) => {
                let n = ready!(Pin::new(&mut *io).poll_read(cx, into))?;
                if n == 0 && !into.is_empty() && io.limit() > 0 {
                    return Poll::Ready(Err(truncation_error()));
                }
                Poll::Ready(Ok(n))
            }
        }
    }
}

#[cfg(feature = "runtime-tokio")]
impl<R: Read + Unpin> Read for EntryIo<R> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        into: &mut tokio::io::ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        match &mut *self {
            EntryIo::Pad(io) => Pin::new(io).poll_read(cx, into),
            EntryIo::Data(io) => {
                let filled = into.filled().len();
                ready!(Pin::new(&mut *io).poll_read(cx, into))?;
                if into.filled().len() == filled && into.remaining() > 0 && io.limit() > 0 {
                    return Poll::Ready(Err(truncation_error()));
                }
                Poll::Ready(Ok(()))
            }
        }
    }
}

/// Creates `dst` for writing, replacing a file or link already there.
fn create_new_replacing(dst: &Path) -> io::Result<std::fs::File> {
    let open = || {
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(dst)
    };
    match open() {
        Err(err) if err.kind() == ErrorKind::AlreadyExists => match std::fs::remove_file(dst) {
            Ok(()) => open(),
            Err(e) if e.kind() == ErrorKind::NotFound => open(),
            Err(e) => Err(e),
        },
        result => result,
    }
}

/// Returns the error for a truncated member.
fn truncation_error() -> Error {
    Error::new(ErrorKind::UnexpectedEof, "archive member is truncated")
}

/// Returns whether Windows reads a name component as a drive, a stream or a device.
#[cfg(windows)]
fn is_windows_reserved(part: &OsStr) -> bool {
    part.as_encoded_bytes().contains(&b':') || is_device_name(part)
}

#[cfg(not(windows))]
fn is_windows_reserved(_: &OsStr) -> bool {
    false
}

/// Returns whether Windows opens `name` as a device, such as `NUL` or `COM1`.
#[cfg(windows)]
fn is_device_name(name: &OsStr) -> bool {
    use std::os::windows::ffi::OsStrExt;

    #[link(name = "ntdll")]
    unsafe extern "system" {
        fn RtlIsDosDeviceName_U(name: *const u16) -> u32;
    }

    // A device name may be followed by an extension, a stream or spaces.
    let Some(name) = name.to_str() else {
        return false;
    };
    let base = name.split(['.', ':']).next().unwrap_or_default();
    let base = base.trim_end_matches(' ');
    if !is_device_base_name(base) {
        return false;
    }
    if base.len() == name.len() {
        return true;
    }

    // Since Windows 11 a device name with an extension, such as `CON.txt`, is a
    // file, so Windows decides.
    let wide: Vec<u16> = OsStr::new(name).encode_wide().chain([0]).collect();
    // SAFETY: `wide` is a NUL-terminated UTF-16 string that outlives the call.
    unsafe { RtlIsDosDeviceName_U(wide.as_ptr()) != 0 }
}

/// Returns whether `name` is a Windows device name without an extension.
#[cfg(windows)]
fn is_device_base_name(name: &str) -> bool {
    let name = name.to_ascii_uppercase();
    if matches!(
        name.as_str(),
        "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
    ) {
        return true;
    }
    match name.split_at_checked(3) {
        Some(("COM" | "LPT", number)) => {
            matches!(number.as_bytes(), [b'1'..=b'9'])
                || matches!(number, "\u{b9}" | "\u{b2}" | "\u{b3}")
        }
        _ => false,
    }
}

struct Guard<'a> {
    buf: &'a mut Vec<u8>,
    len: usize,
}

impl Drop for Guard<'_> {
    fn drop(&mut self) {
        unsafe {
            self.buf.set_len(self.len);
        }
    }
}

#[cfg(feature = "runtime-smol")]
fn poll_read_all_internal<R: Read + ?Sized>(
    mut rd: Pin<&mut R>,
    cx: &mut Context<'_>,
    buf: &mut Vec<u8>,
) -> Poll<io::Result<usize>> {
    let mut g = Guard {
        len: buf.len(),
        buf,
    };
    let ret;
    loop {
        if g.len == g.buf.len() {
            unsafe {
                g.buf.reserve(32);
                let capacity = g.buf.capacity();
                g.buf.set_len(capacity);

                let buf = &mut g.buf[g.len..];
                std::ptr::write_bytes(buf.as_mut_ptr(), 0, buf.len());
            }
        }

        match ready!(rd.as_mut().poll_read(cx, &mut g.buf[g.len..])) {
            Ok(0) => {
                ret = Poll::Ready(Ok(g.len));
                break;
            }
            Ok(n) => g.len += n,
            Err(e) => {
                ret = Poll::Ready(Err(e));
                break;
            }
        }
    }

    ret
}

#[cfg(feature = "runtime-tokio")]
fn poll_read_all_internal<R: Read + ?Sized>(
    mut rd: Pin<&mut R>,
    cx: &mut Context<'_>,
    buf: &mut Vec<u8>,
) -> Poll<io::Result<usize>> {
    let mut g = Guard {
        len: buf.len(),
        buf,
    };
    let ret;
    loop {
        if g.len == g.buf.len() {
            unsafe {
                g.buf.reserve(32);
                let capacity = g.buf.capacity();
                g.buf.set_len(capacity);

                let buf = &mut g.buf[g.len..];
                std::ptr::write_bytes(buf.as_mut_ptr(), 0, buf.len());
            }
        }

        let mut read_buf = io::ReadBuf::new(&mut g.buf[g.len..]);
        let start = read_buf.filled().len();
        match ready!(rd.as_mut().poll_read(cx, &mut read_buf)) {
            Ok(()) => {
                let diff = read_buf.filled().len() - start;
                if diff == 0 {
                    ret = Poll::Ready(Ok(g.len));
                    break;
                } else {
                    g.len += diff;
                }
            }
            Err(e) => {
                ret = Poll::Ready(Err(e));
                break;
            }
        }
    }

    ret
}
