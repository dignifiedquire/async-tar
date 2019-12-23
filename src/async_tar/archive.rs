use std::cell::{Cell, RefCell};
use std::cmp;
use std::marker;
use std::path::Path;
use std::pin::Pin;

use async_std::io;
use async_std::io::prelude::*;
use async_std::prelude::*;
use async_std::stream::Stream;
use async_std::task::{Context, Poll};
use pin_cell::PinCell;
use pin_project::{pin_project, project};

use crate::async_tar::entry::{EntryFields, EntryIo};
use crate::async_tar::error::TarError;
use crate::async_tar::other;
use crate::async_tar::{Entry, GnuExtSparseHeader, GnuSparseHeader, Header};

/// A top-level representation of an archive file.
///
/// This archive can have an entry added to it and it can be iterated over.
pub struct Archive<R: ?Sized + Read + Unpin> {
    inner: ArchiveInner<R>,
}

#[pin_project]
pub struct ArchiveInner<R: ?Sized> {
    pos: Cell<u64>,
    unpack_xattrs: bool,
    preserve_permissions: bool,
    preserve_mtime: bool,
    ignore_zeros: bool,
    #[pin]
    obj: PinCell<R>,
}

/// A stream over the entries of an archive.
#[pin_project]
pub struct Entries<'a, R: 'a + Read + Unpin> {
    #[pin]
    fields: EntriesFields<'a>,
    _ignored: marker::PhantomData<&'a Archive<R>>,
}

#[pin_project]
struct EntriesFields<'a> {
    archive: &'a Archive<dyn Read + Unpin + 'a>,
    next: u64,
    state: EntriesFieldsState<'a>,
    raw: bool,
}

#[pin_project]
enum EntriesFieldsState<'a> {
    NotPolled,
    Reading(Box<dyn Future<Output = io::Result<Option<Entry<'a, io::Empty>>>> + 'a + Unpin>),
    Done,
}

impl<R: Read + Unpin> Archive<R> {
    /// Create a new archive with the underlying object as the reader.
    pub fn new(obj: R) -> Archive<R> {
        Archive {
            inner: ArchiveInner {
                unpack_xattrs: false,
                preserve_permissions: false,
                preserve_mtime: true,
                ignore_zeros: false,
                obj: PinCell::new(obj),
                pos: Cell::new(0),
            },
        }
    }

    /// Unwrap this archive, returning the underlying object.
    pub fn into_inner(self) -> R {
        let c: RefCell<R> = self.inner.obj.into();
        c.into_inner()
    }

    /// Construct an stream over the entries in this archive.
    ///
    /// Note that care must be taken to consider each entry within an archive in
    /// sequence. If entries are processed out of sequence (from what the
    /// stream returns), then the contents read for each entry may be
    /// corrupted.
    pub fn entries(&mut self) -> io::Result<Entries<R>> {
        let me: &mut Archive<dyn Read + Unpin> = self;
        me._entries().map(|fields| Entries {
            fields: fields,
            _ignored: marker::PhantomData,
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
    /// use async_std::fs::File;
    /// use tar::async_tar::Archive;
    ///
    /// let mut ar = Archive::new(File::open("foo.tar").await.unwrap());
    /// ar.unpack("foo").await.unwrap();
    /// ```
    pub async fn unpack<P: AsRef<Path>>(&mut self, dst: P) -> io::Result<()> {
        let me: &mut Archive<dyn Read + Unpin> = self;
        me._unpack(dst.as_ref()).await
    }

    /// Indicate whether extended file attributes (xattrs on Unix) are preserved
    /// when unpacking this archive.
    ///
    /// This flag is disabled by default and is currently only implemented on
    /// Unix using xattr support. This may eventually be implemented for
    /// Windows, however, if other archive implementations are found which do
    /// this as well.
    pub fn set_unpack_xattrs(&mut self, unpack_xattrs: bool) {
        self.inner.unpack_xattrs = unpack_xattrs;
    }

    /// Indicate whether extended permissions (like suid on Unix) are preserved
    /// when unpacking this entry.
    ///
    /// This flag is disabled by default and is currently only implemented on
    /// Unix.
    pub fn set_preserve_permissions(&mut self, preserve: bool) {
        self.inner.preserve_permissions = preserve;
    }

    /// Indicate whether access time information is preserved when unpacking
    /// this entry.
    ///
    /// This flag is enabled by default.
    pub fn set_preserve_mtime(&mut self, preserve: bool) {
        self.inner.preserve_mtime = preserve;
    }

    /// Ignore zeroed headers, which would otherwise indicate to the archive that it has no more
    /// entries.
    ///
    /// This can be used in case multiple tar archives have been concatenated together.
    pub fn set_ignore_zeros(&mut self, ignore_zeros: bool) {
        self.inner.ignore_zeros = ignore_zeros;
    }
}

impl<'a> Archive<dyn Read + Unpin + 'a> {
    fn _entries(&mut self) -> io::Result<EntriesFields> {
        if self.inner.pos.get() != 0 {
            return Err(other(
                "cannot call entries unless archive is at \
                 position 0",
            ));
        }
        Ok(EntriesFields {
            archive: self,
            state: EntriesFieldsState::NotPolled,
            next: 0,
            raw: false,
        })
    }

    async fn _unpack(&mut self, dst: &Path) -> io::Result<()> {
        let mut entries = self._entries()?;
        let mut pinned = Pin::new(&mut entries);
        while let Some(entry) = pinned.next().await {
            let mut file = entry.map_err(|e| TarError::new("failed to iterate over archive", e))?;
            file.unpack_in(dst).await?;
        }
        Ok(())
    }

    async fn skip(&self, mut amt: u64) -> io::Result<()> {
        let mut buf = [0u8; 4096 * 8];
        while amt > 0 {
            let n = cmp::min(amt, buf.len() as u64);
            let n = (&self.inner).read(&mut buf[..n as usize]).await?;
            if n == 0 {
                return Err(other("unexpected EOF during skip"));
            }
            amt -= n as u64;
        }
        Ok(())
    }
}

impl<'a, R: Read + Unpin + 'a> Entries<'a, R> {
    /// Indicates whether this Stream will return raw entries or not.
    ///
    /// If the raw list of entries are returned, then no preprocessing happens
    /// on account of this library, for example taking into account GNU long name
    /// or long link archive members. Raw iteration is disabled by default.
    pub fn raw(self, raw: bool) -> Entries<'a, R> {
        Entries {
            fields: EntriesFields {
                raw: raw,
                ..self.fields
            },
            _ignored: marker::PhantomData,
        }
    }
}
impl<'a, R: Read + Unpin> Stream for Entries<'a, R> {
    type Item = io::Result<Entry<'a, R>>;

    fn poll_next(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> std::task::Poll<Option<io::Result<Entry<'a, R>>>> {
        let mut this = self.project();
        let fields = Pin::new(&mut this.fields);
        let poll = async_std::task::ready!(fields.poll_next(cx));
        match poll {
            Some(r) => Poll::Ready(Some(r.map(|e| EntryFields::from(e).into_entry()))),
            None => Poll::Ready(None),
        }
    }
}

impl<'a> EntriesFields<'a> {
    async fn next_entry_raw(&'a mut self) -> io::Result<Option<Entry<'a, io::Empty>>> {
        let mut header = Header::new_old();
        let mut header_pos = self.next;

        loop {
            // Seek to the start of the next header in the archive
            let delta = self.next - self.archive.inner.pos.get();
            self.archive.skip(delta).await?;

            // EOF is an indicator that we are at the end of the archive.
            if !try_read_all(&mut &self.archive.inner, header.as_mut_bytes()).await? {
                return Ok(None);
            }

            // If a header is not all zeros, we have another valid header.
            // Otherwise, check if we are ignoring zeros and continue, or break as if this is the
            // end of the archive.
            if !header.as_bytes().iter().all(|i| *i == 0) {
                self.next += 512;
                break;
            }

            if !self.archive.inner.ignore_zeros {
                return Ok(None);
            }

            self.next += 512;
            header_pos = self.next;
        }

        // Make sure the checksum is ok
        let sum = header.as_bytes()[..148]
            .iter()
            .chain(&header.as_bytes()[156..])
            .fold(0, |a, b| a + (*b as u32))
            + 8 * 32;
        let cksum = header.cksum()?;
        if sum != cksum {
            return Err(other("archive header checksum mismatch"));
        }

        let file_pos = self.next;
        let size = header.entry_size()?;

        let ret = EntryFields {
            size: size,
            header_pos: header_pos,
            file_pos: file_pos,
            data: vec![EntryIo::Data((&self.archive.inner).take(size))],
            header: header,
            long_pathname: None,
            long_linkname: None,
            pax_extensions: None,
            unpack_xattrs: self.archive.inner.unpack_xattrs,
            preserve_permissions: self.archive.inner.preserve_permissions,
            preserve_mtime: self.archive.inner.preserve_mtime,
        };

        // Store where the next entry is, rounding up by 512 bytes (the size of
        // a header);
        let size = (size + 511) & !(512 - 1);
        self.next += size;

        Ok(Some(ret.into_entry()))
    }

    async fn next_entry(&'a mut self) -> io::Result<Option<Entry<'a, io::Empty>>> {
        if self.raw {
            return self.next_entry_raw().await;
        }

        let mut gnu_longname = None;
        let mut gnu_longlink = None;
        let mut pax_extensions = None;
        let mut processed: usize = 0;
        loop {
            processed += 1;
            let next = self.next_entry_raw().await?;
            let entry = match next {
                Some(entry) => entry,
                None if processed > 1 => {
                    return Err(other(
                        "members found describing a future member \
                         but no future member found",
                    ));
                }
                None => return Ok(None),
            };

            if entry.header().as_gnu().is_some() && entry.header().entry_type().is_gnu_longname() {
                if gnu_longname.is_some() {
                    return Err(other(
                        "two long name entries describing \
                         the same member",
                    ));
                }
                gnu_longname = Some(EntryFields::from(entry).read_all().await?);
                continue;
            }

            if entry.header().as_gnu().is_some() && entry.header().entry_type().is_gnu_longlink() {
                if gnu_longlink.is_some() {
                    return Err(other(
                        "two long name entries describing \
                         the same member",
                    ));
                }
                gnu_longlink = Some(EntryFields::from(entry).read_all().await?);
                continue;
            }

            if entry.header().as_ustar().is_some()
                && entry.header().entry_type().is_pax_local_extensions()
            {
                if pax_extensions.is_some() {
                    return Err(other(
                        "two pax extensions entries describing \
                         the same member",
                    ));
                }
                pax_extensions = Some(EntryFields::from(entry).read_all().await?);
                continue;
            }

            let mut fields = EntryFields::from(entry);
            fields.long_pathname = gnu_longname;
            fields.long_linkname = gnu_longlink;
            fields.pax_extensions = pax_extensions;
            self.parse_sparse_header(&mut fields).await?;
            return Ok(Some(fields.into_entry()));
        }
    }

    async fn parse_sparse_header(&mut self, entry: &mut EntryFields<'a>) -> io::Result<()> {
        if !entry.header.entry_type().is_gnu_sparse() {
            return Ok(());
        }
        let gnu = match entry.header.as_gnu() {
            Some(gnu) => gnu,
            None => return Err(other("sparse entry type listed but not GNU header")),
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
            let reader = &self.archive.inner;
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
                data.push(EntryIo::Data(reader.take(len)));
                Ok(())
            };
            for block in gnu.sparse.iter() {
                add_block(block)?
            }
            if gnu.is_extended() {
                let mut ext = GnuExtSparseHeader::new();
                ext.isextended[0] = 1;
                while ext.is_extended() {
                    if !try_read_all(&mut &self.archive.inner, ext.as_mut_bytes()).await? {
                        return Err(other("failed to read extension"));
                    }

                    self.next += 512;
                    for block in ext.sparse.iter() {
                        add_block(block)?;
                    }
                }
            }
        }
        if cur != gnu.real_size()? {
            return Err(other(
                "mismatch in sparse file chunks and \
                 size in header",
            ));
        }
        entry.size = cur;
        if remaining > 0 {
            return Err(other(
                "mismatch in sparse file chunks and \
                 entry size in header",
            ));
        }
        Ok(())
    }
}

impl<'a> Stream for EntriesFields<'a> {
    type Item = io::Result<Entry<'a, io::Empty>>;

    #[project]
    fn poll_next(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<io::Result<Entry<'a, io::Empty>>>> {
        let this = self.project();

        loop {
            #[project]
            match Pin::new(&mut *this.state).project() {
                EntriesFieldsState::NotPolled => {
                    // self.state = EntriesFieldsState::Reading(Box::new(self.next_entry()));
                    unimplemented!()
                }
                EntriesFieldsState::Reading(f) => match Pin::new(f).poll(cx) {
                    Poll::Pending => return Poll::Pending,
                    Poll::Ready(r) => {
                        if r.is_err() {
                            *this.state = EntriesFieldsState::Done;
                            return Poll::Ready(r.transpose());
                        } else {
                            *this.state = EntriesFieldsState::NotPolled;
                            return Poll::Ready(r.transpose());
                        }
                    }
                },
                EntriesFieldsState::Done => {
                    return Poll::Ready(None);
                }
            }
        }
    }
}

impl<'a, R: ?Sized + Read + Unpin> Read for &'a ArchiveInner<R> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        into: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        let this_0: Pin<&mut ArchiveInner<R>> = Pin::new(&mut *self);
        let this = this_0.project();
        let mut r = this.obj.as_ref().borrow_mut();

        let res = async_std::task::ready!(pin_cell::PinMut::as_mut(&mut r).poll_read(cx, into));
        match res {
            Ok(i) => {
                self.pos.set(self.pos.get() + i as u64);
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
async fn try_read_all<R: Read + Unpin>(r: &mut R, buf: &mut [u8]) -> io::Result<bool> {
    let mut read = 0;
    while read < buf.len() {
        match r.read(&mut buf[read..]).await? {
            0 => {
                if read == 0 {
                    return Ok(false);
                }

                return Err(other("failed to read entire block"));
            }
            n => read += n,
        }
    }
    Ok(true)
}
