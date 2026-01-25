//! An example of extracting a file in an archive.
//!
//! Takes a tarball on standard input, looks for an entry with a listed file
//! name as the first argument provided, and then prints the contents of that
//! file to stdout.

use async_tar::Archive;

use std::env::args_os;
use std::path::Path;

#[cfg(feature = "runtime-smol")]
use {
    smol::{Unblock, io::copy, stream::StreamExt},
    std::io::{stdin, stdout},
};

#[cfg(feature = "runtime-tokio")]
use {
    tokio::io::{copy, stdin, stdout},
    tokio_stream::StreamExt,
};

async fn inner_main() {
    let first_arg = args_os().nth(1).unwrap();
    let filename = Path::new(&first_arg);

    #[cfg(feature = "runtime-smol")]
    let stdin = Unblock::new(stdin());

    #[cfg(feature = "runtime-smol")]
    let mut stdout = Unblock::new(stdout());

    #[cfg(feature = "runtime-tokio")]
    let stdin = stdin();

    #[cfg(feature = "runtime-tokio")]
    let mut stdout = stdout();

    let ar = Archive::new(stdin);
    let mut entries = ar.entries().unwrap();
    while let Some(file) = entries.next().await {
        let mut f = file.unwrap();
        if f.path().unwrap() == filename {
            copy(&mut f, &mut stdout).await.unwrap();
        }
    }
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
