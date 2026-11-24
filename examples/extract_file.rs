//! An example of extracting a file in an archive.
//!
//! Takes a tarball on standard input, looks for an entry with a listed file
//! name as the first argument provided, and then prints the contents of that
//! file to stdout.

use std::{env::args_os, io, path::Path};

use async_tar::Archive;
#[cfg(feature = "runtime-smol")]
use smol::{
    Unblock,
    io::{AsyncWriteExt, copy},
    stream::StreamExt,
};
#[cfg(feature = "runtime-tokio")]
use tokio::{
    io::{AsyncWriteExt, copy, stdin, stdout},
    runtime::Runtime,
};
#[cfg(feature = "runtime-tokio")]
use tokio_stream::StreamExt;

#[cfg(feature = "runtime-smol")]
fn stdin() -> Unblock<io::Stdin> {
    Unblock::new(io::stdin())
}

#[cfg(feature = "runtime-smol")]
fn stdout() -> Unblock<io::Stdout> {
    Unblock::new(io::stdout())
}

async fn inner_main() -> io::Result<()> {
    let first_arg = args_os()
        .nth(1)
        .ok_or_else(|| io::Error::other("usage: extract_file <path>"))?;
    let filename = Path::new(&first_arg);
    let ar = Archive::new(stdin());
    let mut out = stdout();
    let mut entries = ar.entries()?;
    while let Some(file) = entries.next().await {
        let mut f = file?;
        if f.path()? == filename {
            copy(&mut f, &mut out).await?;
        }
    }
    out.flush().await
}

#[cfg(feature = "runtime-smol")]
fn main() -> io::Result<()> {
    smol::block_on(inner_main())
}

#[cfg(feature = "runtime-tokio")]
fn main() -> io::Result<()> {
    Runtime::new()?.block_on(inner_main())
}
