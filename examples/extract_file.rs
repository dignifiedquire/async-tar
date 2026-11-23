//! An example of extracting a file in an archive.
//!
//! Takes a tarball on standard input, looks for an entry with a listed file
//! name as the first argument provided, and then prints the contents of that
//! file to stdout.

use std::{env::args_os, path::Path};

use async_tar::Archive;
#[cfg(feature = "runtime-smol")]
use smol::{
    io::{AsyncWriteExt, copy},
    stream::StreamExt,
};
#[cfg(feature = "runtime-tokio")]
use tokio::io::{AsyncWriteExt, copy, stdin, stdout};
#[cfg(feature = "runtime-tokio")]
use tokio_stream::StreamExt;

#[cfg(feature = "runtime-smol")]
fn stdin() -> smol::Unblock<std::io::Stdin> {
    smol::Unblock::new(std::io::stdin())
}

#[cfg(feature = "runtime-smol")]
fn stdout() -> smol::Unblock<std::io::Stdout> {
    smol::Unblock::new(std::io::stdout())
}

async fn inner_main() {
    let first_arg = args_os().nth(1).unwrap();
    let filename = Path::new(&first_arg);
    let ar = Archive::new(stdin());
    let mut out = stdout();
    let mut entries = ar.entries().unwrap();
    while let Some(file) = entries.next().await {
        let mut f = file.unwrap();
        if f.path().unwrap() == filename {
            copy(&mut f, &mut out).await.unwrap();
        }
    }
    out.flush().await.unwrap();
}

#[cfg(feature = "runtime-smol")]
fn main() {
    smol::block_on(inner_main());
}

#[cfg(feature = "runtime-tokio")]
fn main() {
    tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(inner_main());
}
