use std::{
    cmp,
    pin::Pin,
    sync::{Arc, Mutex},
};

use async_std::{
    io,
    io::prelude::*,
    path::Path,
    prelude::*,
    stream::Stream,
    task::{Context, Poll},
};
use pin_project::pin_project;

use crate::{
    entry::{EntryFields, EntryIo},
    error::TarError,
    other, Entry, GnuExtSparseHeader, GnuSparseHeader, Header,
};

/// A top-level representation of an archive file.
///
/// This archive can have an entry added to it and it can be iterated over.
#[derive(Debug)]
pub struct Archive<R: Read + Unpin> {
    inner: Arc<Mutex<ArchiveInner<R>>>,
}

impl<R: Read + Unpin> Clone for Archive<R> {
    fn clone(&self) -> Self {
        Archive {
            inner: self.inner.clone(),
        }
    }
}

#[pin_project]
#[derive(Debug)]
pub struct ArchiveInner<R: Read + Unpin> {
    pos: u64,
    unpack_xattrs: bool,
    preserve_permissions: bool,
    preserve_mtime: bool,
    ignore_zeros: bool,
    #[pin]
    obj: R,
}

/// Configure the archive.
pub struct ArchiveBuilder<R: Read + Unpin> {
    obj: R,
    unpack_xattrs: bool,
    preserve_permissions: bool,
    preserve_mtime: bool,
    ignore_zeros: bool,
}

impl<R: Read + Unpin> ArchiveBuilder<R> {
    /// Create a new builder.
    pub fn new(obj: R) -> Self {
        ArchiveBuilder {
            unpack_xattrs: false,
            preserve_permissions: false,
            preserve_mtime: true,
            ignore_zeros: false,
            obj,
        }
    }

    /// Indicate whether extended file attributes (xattrs on Unix) are preserved
    /// when unpacking this archive.
    ///
    /// This flag is disabled by default and is currently only implemented on
    /// Unix using xattr support. This may eventually be implemented for
    /// Windows, however, if other archive implementations are found which do
    /// this as well.
    pub fn set_unpack_xattrs(mut self, unpack_xattrs: bool) -> Self {
        self.unpack_xattrs = unpack_xattrs;
        self
    }

    /// Indicate whether extended permissions (like suid on Unix) are preserved
    /// when unpacking this entry.
    ///
    /// This flag is disabled by default and is currently only implemented on
    /// Unix.
    pub fn set_preserve_permissions(mut self, preserve: bool) -> Self {
        self.preserve_permissions = preserve;
        self
    }

    /// Indicate whether access time information is preserved when unpacking
    /// this entry.
    ///
    /// This flag is enabled by default.
    pub fn set_preserve_mtime(mut self, preserve: bool) -> Self {
        self.preserve_mtime = preserve;
        self
    }

    /// Ignore zeroed headers, which would otherwise indicate to the archive that it has no more
    /// entries.
    ///
    /// This can be used in case multiple tar archives have been concatenated together.
    pub fn set_ignore_zeros(mut self, ignore_zeros: bool) -> Self {
        self.ignore_zeros = ignore_zeros;
        self
    }

    /// Construct the archive, ready to accept inputs.
    pub fn build(self) -> Archive<R> {
        let Self {
            unpack_xattrs,
            preserve_permissions,
            preserve_mtime,
            ignore_zeros,
            obj,
        } = self;

        Archive {
            inner: Arc::new(Mutex::new(ArchiveInner {
                unpack_xattrs,
                preserve_permissions,
                preserve_mtime,
                ignore_zeros,
                obj,
                pos: 0,
            })),
        }
    }
}

impl<R: Read + Unpin> Archive<R> {
    /// Create a new archive with the underlying object as the reader.
    pub fn new(obj: R) -> Archive<R> {
        Archive {
            inner: Arc::new(Mutex::new(ArchiveInner {
                unpack_xattrs: false,
                preserve_permissions: false,
                preserve_mtime: true,
                ignore_zeros: false,
                obj,
                pos: 0,
            })),
        }
    }

    /// Unwrap this archive, returning the underlying object.
    pub fn into_inner(self) -> Result<R, Self> {
        match Arc::try_unwrap(self.inner) {
            Ok(inner) => Ok(inner.into_inner().unwrap().obj),
            Err(inner) => Err(Self { inner }),
        }
    }

    /// Construct an stream over the entries in this archive.
    ///
    /// Note that care must be taken to consider each entry within an archive in
    /// sequence. If entries are processed out of sequence (from what the
    /// stream returns), then the contents read for each entry may be
    /// corrupted.
    pub fn entries(self) -> io::Result<Entries<R>> {
        if self.inner.lock().unwrap().pos != 0 {
            return Err(other(
                "cannot call entries unless archive is at \
                 position 0",
            ));
        }

        Ok(Entries {
            archive: self,
            current: (0, None, 0, None),
            gnu_longlink: None,
            gnu_longname: None,
            pax_extensions: None,
        })
    }

    /// Construct an stream over the raw entries in this archive.
    ///
    /// Note that care must be taken to consider each entry within an archive in
    /// sequence. If entries are processed out of sequence (from what the
    /// stream returns), then the contents read for each entry may be
    /// corrupted.
    pub fn entries_raw(self) -> io::Result<RawEntries<R>> {
        if self.inner.lock().unwrap().pos != 0 {
            return Err(other(
                "cannot call entries_raw unless archive is at \
                 position 0",
            ));
        }

        Ok(RawEntries {
            archive: self,
            current: (0, None, 0),
        })
    }

    /// Unpacks the contents tarball into the specified `dst`.
    ///
    /// This function will iterate over the entire contents of this tarball,
    /// extracting each file in turn to the location specified by the entry's
    /// path name.
    ///
    /// This operation is relatively sensitive in that it will not write files
    /// outside of the path specified by `dst`. Files in the archive which have
    /// a '..' in their path are skipped during the unpacking process.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> { async_std::task::block_on(async {
    /// #
    /// use async_std::fs::File;
    /// use async_tar::Archive;
    ///
    /// let mut ar = Archive::new(File::open("foo.tar").await?);
    /// ar.unpack("foo").await?;
    /// #
    /// # Ok(()) }) }
    /// ```
    pub async fn unpack<P: AsRef<Path>>(self, dst: P) -> io::Result<()> {
        let mut entries = self.entries()?;
        let mut pinned = Pin::new(&mut entries);
        while let Some(entry) = pinned.next().await {
            let mut file = entry.map_err(|e| TarError::new("failed to iterate over archive", e))?;
            file.unpack_in(dst.as_ref()).await?;
        }
        Ok(())
    }
}

/// Stream of `Entry`s.
pub struct Entries<R: Read + Unpin> {
    archive: Archive<R>,
    current: (u64, Option<Header>, usize, Option<GnuExtSparseHeader>),
    gnu_longname: Option<Vec<u8>>,
    gnu_longlink: Option<Vec<u8>>,
    pax_extensions: Option<Vec<u8>>,
}

macro_rules! ready_opt_err {
    ($val:expr) => {
        match async_std::task::ready!($val) {
            Some(Ok(val)) => val,
            Some(Err(err)) => return Poll::Ready(Some(Err(err))),
            None => return Poll::Ready(None),
        }
    };
}

macro_rules! ready_err {
    ($val:expr) => {
        match async_std::task::ready!($val) {
            Ok(val) => val,
            Err(err) => return Poll::Ready(Some(Err(err))),
        }
    };
}

impl<R: Read + Unpin> Stream for Entries<R> {
    type Item = io::Result<Entry<Archive<R>>>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        loop {
            let archive = self.archive.clone();
            let (next, current_header, current_header_pos, _) = &mut self.current;
            let entry = ready_opt_err!(poll_next_raw(
                archive,
                next,
                current_header,
                current_header_pos,
                cx
            ));

            let is_recognized_header =
                entry.header().as_gnu().is_some() || entry.header().as_ustar().is_some();
            if is_recognized_header && entry.header().entry_type().is_gnu_longname() {
                if self.gnu_longname.is_some() {
                    return Poll::Ready(Some(Err(other(
                        "two long name entries describing \
                         the same member",
                    ))));
                }

                let mut ef = EntryFields::from(entry);
                let val = ready_err!(Pin::new(&mut ef).poll_read_all(cx));
                self.gnu_longname = Some(val);
                continue;
            }

            if is_recognized_header && entry.header().entry_type().is_gnu_longlink() {
                if self.gnu_longlink.is_some() {
                    return Poll::Ready(Some(Err(other(
                        "two long name entries describing \
                         the same member",
                    ))));
                }
                let mut ef = EntryFields::from(entry);
                let val = ready_err!(Pin::new(&mut ef).poll_read_all(cx));
                self.gnu_longlink = Some(val);
                continue;
            }

            if is_recognized_header && entry.header().entry_type().is_pax_local_extensions() {
                if self.pax_extensions.is_some() {
                    return Poll::Ready(Some(Err(other(
                        "two pax extensions entries describing \
                         the same member",
                    ))));
                }
                let mut ef = EntryFields::from(entry);
                let val = ready_err!(Pin::new(&mut ef).poll_read_all(cx));
                self.pax_extensions = Some(val);
                continue;
            }

            let mut fields = EntryFields::from(entry);
            fields.long_pathname = self.gnu_longname.take();
            fields.long_linkname = self.gnu_longlink.take();
            fields.pax_extensions = self.pax_extensions.take();

            let archive = self.archive.clone();
            let (next, _, current_pos, current_ext) = &mut self.current;
            ready_err!(poll_parse_sparse_header(
                archive,
                next,
                current_ext,
                current_pos,
                &mut fields,
                cx
            ));

            return Poll::Ready(Some(Ok(fields.into_entry())));
        }
    }
}

/// Stream of raw `Entry`s.
pub struct RawEntries<R: Read + Unpin> {
    archive: Archive<R>,
    current: (u64, Option<Header>, usize),
}

impl<R: Read + Unpin> Stream for RawEntries<R> {
    type Item = io::Result<Entry<Archive<R>>>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let archive = self.archive.clone();
        let (next, current_header, current_header_pos) = &mut self.current;
        poll_next_raw(archive, next, current_header, current_header_pos, cx)
    }
}

fn poll_next_raw<R: Read + Unpin>(
    archive: Archive<R>,
    next: &mut u64,
    current_header: &mut Option<Header>,
    current_header_pos: &mut usize,
    cx: &mut Context<'_>,
) -> Poll<Option<io::Result<Entry<Archive<R>>>>> {
    let mut header_pos = *next;

    loop {
        let archive = archive.clone();
        // Seek to the start of the next header in the archive
        if current_header.is_none() {
            let delta = *next - archive.inner.lock().unwrap().pos;
            match async_std::task::ready!(poll_skip(archive.clone(), cx, delta)) {
                Ok(_) => {}
                Err(err) => return Poll::Ready(Some(Err(err))),
            }

            *current_header = Some(Header::new_old());
            *current_header_pos = 0;
        }

        let header = current_header.as_mut().unwrap();

        // EOF is an indicator that we are at the end of the archive.
        match async_std::task::ready!(poll_try_read_all(
            archive.clone(),
            cx,
            header.as_mut_bytes(),
            current_header_pos,
        )) {
            Ok(true) => {}
            Ok(false) => return Poll::Ready(None),
            Err(err) => return Poll::Ready(Some(Err(err))),
        }

        // If a header is not all zeros, we have another valid header.
        // Otherwise, check if we are ignoring zeros and continue, or break as if this is the
        // end of the archive.
        if !header.as_bytes().iter().all(|i| *i == 0) {
            *next += 512;
            break;
        }

        if !archive.inner.lock().unwrap().ignore_zeros {
            return Poll::Ready(None);
        }

        *next += 512;
        header_pos = *next;
    }

    let header = current_header.as_mut().unwrap();

    // Make sure the checksum is ok
    let sum = header.as_bytes()[..148]
        .iter()
        .chain(&header.as_bytes()[156..])
        .fold(0, |a, b| a + (*b as u32))
        + 8 * 32;
    let cksum = header.cksum()?;
    if sum != cksum {
        return Poll::Ready(Some(Err(other("archive header checksum mismatch"))));
    }

    let file_pos = *next;
    let size = header.entry_size()?;

    let data = EntryIo::Data(archive.clone().take(size));
    drop(header);

    let header = current_header.take().unwrap();

    let ArchiveInner {
        unpack_xattrs,
        preserve_mtime,
        preserve_permissions,
        ..
    } = &*archive.inner.lock().unwrap();

    let ret = EntryFields {
        size,
        header_pos,
        file_pos,
        data: vec![data],
        header,
        long_pathname: None,
        long_linkname: None,
        pax_extensions: None,
        unpack_xattrs: *unpack_xattrs,
        preserve_permissions: *preserve_permissions,
        preserve_mtime: *preserve_mtime,
        read_state: None,
    };

    // Store where the next entry is, rounding up by 512 bytes (the size of
    // a header);
    let size = (size + 511) & !(512 - 1);
    *next += size;

    Poll::Ready(Some(Ok(ret.into_entry())))
}

fn poll_parse_sparse_header<R: Read + Unpin>(
    archive: Archive<R>,
    next: &mut u64,
    current_ext: &mut Option<GnuExtSparseHeader>,
    current_ext_pos: &mut usize,
    entry: &mut EntryFields<Archive<R>>,
    cx: &mut Context<'_>,
) -> Poll<io::Result<()>> {
    if !entry.header.entry_type().is_gnu_sparse() {
        return Poll::Ready(Ok(()));
    }

    let gnu = match entry.header.as_gnu() {
        Some(gnu) => gnu,
        None => return Poll::Ready(Err(other("sparse entry type listed but not GNU header"))),
    };

    // Sparse files are represented internally as a list of blocks that are
    // read. Blocks are either a bunch of 0's or they're data from the
    // underlying archive.
    //
    // Blocks of a sparse file are described by the `GnuSparseHeader`
    // structure, some of which are contained in `GnuHeader` but some of
    // which may also be contained after the first header in further
    // headers.
    //
    // We read off all the blocks here and use the `add_block` function to
    // incrementally add them to the list of I/O block (in `entry.data`).
    // The `add_block` function also validates that each chunk comes after
    // the previous, we don't overrun the end of the file, and each block is
    // aligned to a 512-byte boundary in the archive itself.
    //
    // At the end we verify that the sparse file size (`Header::size`) is
    // the same as the current offset (described by the list of blocks) as
    // well as the amount of data read equals the size of the entry
    // (`Header::entry_size`).
    entry.data.truncate(0);

    let mut cur = 0;
    let mut remaining = entry.size;
    {
        let data = &mut entry.data;
        let reader = archive.clone();
        let size = entry.size;
        let mut add_block = |block: &GnuSparseHeader| -> io::Result<_> {
            if block.is_empty() {
                return Ok(());
            }
            let off = block.offset()?;
            let len = block.length()?;

            if (size - remaining) % 512 != 0 {
                return Err(other(
                    "previous block in sparse file was not \
                     aligned to 512-byte boundary",
                ));
            } else if off < cur {
                return Err(other(
                    "out of order or overlapping sparse \
                     blocks",
                ));
            } else if cur < off {
                let block = io::repeat(0).take(off - cur);
                data.push(EntryIo::Pad(block));
            }
            cur = off
                .checked_add(len)
                .ok_or_else(|| other("more bytes listed in sparse file than u64 can hold"))?;
            remaining = remaining.checked_sub(len).ok_or_else(|| {
                other(
                    "sparse file consumed more data than the header \
                     listed",
                )
            })?;
            data.push(EntryIo::Data(reader.clone().take(len)));
            Ok(())
        };
        for block in gnu.sparse.iter() {
            add_block(block)?
        }
        if gnu.is_extended() {
            let started_header = current_ext.is_some();
            if !started_header {
                let mut ext = GnuExtSparseHeader::new();
                ext.isextended[0] = 1;
                *current_ext = Some(ext);
                *current_ext_pos = 0;
            }

            let ext = current_ext.as_mut().unwrap();
            while ext.is_extended() {
                match async_std::task::ready!(poll_try_read_all(
                    archive.clone(),
                    cx,
                    ext.as_mut_bytes(),
                    current_ext_pos,
                )) {
                    Ok(true) => {}
                    Ok(false) => return Poll::Ready(Err(other("failed to read extension"))),
                    Err(err) => return Poll::Ready(Err(err)),
                }

                *next += 512;
                for block in &ext.sparse {
                    add_block(block)?;
                }
            }
        }
    }
    if cur != gnu.real_size()? {
        return Poll::Ready(Err(other(
            "mismatch in sparse file chunks and \
             size in header",
        )));
    }
    entry.size = cur;
    if remaining > 0 {
        return Poll::Ready(Err(other(
            "mismatch in sparse file chunks and \
             entry size in header",
        )));
    }

    Poll::Ready(Ok(()))
}

impl<R: Read + Unpin> Read for Archive<R> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        into: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        let mut lock = self.inner.lock().unwrap();
        let mut inner = Pin::new(&mut *lock);
        let r = Pin::new(&mut inner.obj);

        let res = async_std::task::ready!(r.poll_read(cx, into));
        match res {
            Ok(i) => {
                inner.pos += i as u64;
                Poll::Ready(Ok(i))
            }
            Err(err) => Poll::Ready(Err(err)),
        }
    }
}

/// Try to fill the buffer from the reader.
///
/// If the reader reaches its end before filling the buffer at all, returns `false`.
/// Otherwise returns `true`.
fn poll_try_read_all<R: Read + Unpin>(
    mut source: R,
    cx: &mut Context<'_>,
    buf: &mut [u8],
    pos: &mut usize,
) -> Poll<io::Result<bool>> {
    while *pos < buf.len() {
        match async_std::task::ready!(Pin::new(&mut source).poll_read(cx, &mut buf[*pos..])) {
            Ok(0) => {
                if *pos == 0 {
                    return Poll::Ready(Ok(false));
                }

                return Poll::Ready(Err(other("failed to read entire block")));
            }
            Ok(n) => *pos += n,
            Err(err) => return Poll::Ready(Err(err)),
        }
    }

    *pos = 0;
    Poll::Ready(Ok(true))
}

/// Skip n bytes on the given source.
fn poll_skip<R: Read + Unpin>(
    mut source: R,
    cx: &mut Context<'_>,
    mut amt: u64,
) -> Poll<io::Result<()>> {
    let mut buf = [0u8; 4096 * 8];
    while amt > 0 {
        let n = cmp::min(amt, buf.len() as u64);
        match async_std::task::ready!(Pin::new(&mut source).poll_read(cx, &mut buf[..n as usize])) {
            Ok(n) if n == 0 => {
                return Poll::Ready(Err(other("unexpected EOF during skip")));
            }
            Ok(n) => {
                amt -= n as u64;
            }
            Err(err) => return Poll::Ready(Err(err)),
        }
    }

    Poll::Ready(Ok(()))
}

#[cfg(test)]
mod tests {
    use super::*;

    assert_impl_all!(async_std::fs::File: Send, Sync);
    assert_impl_all!(Entries<async_std::fs::File>: Send, Sync);
    assert_impl_all!(Archive<async_std::fs::File>: Send, Sync);
    assert_impl_all!(Entry<Archive<async_std::fs::File>>: Send, Sync);
}
