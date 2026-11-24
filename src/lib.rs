//! A library for reading and writing TAR archives in an async fashion.
//!
//! This library provides utilities necessary to manage [TAR archives][1]
//! abstracted over a reader or writer. Great strides are taken to ensure that
//! an archive is never required to be fully resident in memory, and all objects
//! provide largely a streaming interface to read bytes from.
//!
//! [1]: http://en.wikipedia.org/wiki/Tar_%28computing%29

// More docs about the detailed tar format can also be found here:
// http://www.freebsd.org/cgi/man.cgi?query=tar&sektion=5&manpath=FreeBSD+8-current

// NB: some of the coding patterns and idioms here may seem a little strange.
//     This is currently attempting to expose a super generic interface while
//     also not forcing clients to codegen the entire crate each time they use
//     it. To that end lots of work is done to ensure that concrete
//     implementations are all found in this crate and the generic functions are
//     all just super thin wrappers (e.g. easy to codegen).

#[cfg(all(feature = "runtime-smol", feature = "runtime-tokio"))]
compile_error!(
    "Features `runtime-smol` and `runtime-tokio` are mutually exclusive. Please enable only one."
);

#[cfg(not(any(feature = "runtime-smol", feature = "runtime-tokio")))]
compile_error!("Either `runtime-smol` or `runtime-tokio` feature must be enabled.");

#[cfg(feature = "runtime-tokio")]
use std::panic;
use std::{
    fs::Metadata,
    io::Error,
    path::{Path, PathBuf},
};

#[cfg(feature = "runtime-smol")]
use async_fs as fs;
#[cfg(feature = "runtime-smol")]
use futures_lite::io::{self, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
#[cfg(feature = "runtime-tokio")]
use tokio::fs;
#[cfg(feature = "runtime-tokio")]
use tokio::{
    io::{self, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    task,
};

pub use crate::{
    archive::{Archive, ArchiveBuilder, Entries, RawEntries},
    builder::Builder,
    entry::{Entry, Unpacked},
    entry_type::EntryType,
    header::{
        GnuExtSparseHeader, GnuHeader, GnuSparseHeader, Header, HeaderMode, OldHeader, UstarHeader,
    },
    pax::{PaxExtension, PaxExtensions},
};

mod archive;
mod builder;
mod entry;
mod entry_type;
mod error;
mod header;
mod pax;

#[cfg(test)]
#[macro_use]
extern crate static_assertions;

/// The largest chunk copied at once, since each file read or write may be a blocking call.
const COPY_CHUNK_SIZE: usize = 128 * 1024;

fn other(msg: &str) -> Error {
    Error::other(msg)
}

/// Runs `f` on the runtime's thread pool for blocking calls.
#[cfg(feature = "runtime-smol")]
async fn unblock<T, F>(f: F) -> io::Result<T>
where
    T: Send + 'static,
    F: FnOnce() -> io::Result<T> + Send + 'static,
{
    blocking::unblock(f).await
}

/// Runs `f` on the runtime's thread pool for blocking calls.
#[cfg(feature = "runtime-tokio")]
async fn unblock<T, F>(f: F) -> io::Result<T>
where
    T: Send + 'static,
    F: FnOnce() -> io::Result<T> + Send + 'static,
{
    match task::spawn_blocking(f).await {
        Ok(result) => result,
        Err(err) => match err.try_into_panic() {
            Ok(payload) => panic::resume_unwind(payload),
            Err(err) => Err(io::Error::other(err)),
        },
    }
}

/// Copies `reader` to `writer` in chunks of up to 128 KiB, flushes `writer`, and returns the
/// bytes copied.
///
/// The buffer is no larger than `size`, the number of bytes expected.
async fn copy<R, W>(reader: &mut R, writer: &mut W, size: u64) -> io::Result<u64>
where
    R: AsyncRead + Unpin + ?Sized,
    W: AsyncWrite + Unpin + ?Sized,
{
    let len = usize::try_from(size).map_or(COPY_CHUNK_SIZE, |size| size.clamp(1, COPY_CHUNK_SIZE));
    let mut buf = vec![0; len];
    let mut copied = 0;
    loop {
        let n = reader.read(&mut buf).await?;
        if n == 0 {
            writer.flush().await?;
            return Ok(copied);
        }
        writer.write_all(&buf[..n]).await?;
        copied += n as u64;
    }
}

pub(crate) async fn fs_canonicalize(path: &Path) -> io::Result<PathBuf> {
    fs::canonicalize(path).await
}

async fn symlink_metadata(p: &Path) -> io::Result<Metadata> {
    fs::symlink_metadata(p).await
}

async fn metadata(p: &Path) -> io::Result<Metadata> {
    fs::metadata(p).await
}
