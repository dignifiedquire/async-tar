//! An example of listing the file names of entries in an archive.
//!
//! Takes a tarball on stdin and prints out all of the entries inside.

use async_tar::Archive;
#[cfg(feature = "runtime-smol")]
use smol::stream::StreamExt;
#[cfg(feature = "runtime-tokio")]
use tokio::io::stdin;
#[cfg(feature = "runtime-tokio")]
use tokio_stream::StreamExt;

#[cfg(feature = "runtime-smol")]
fn stdin() -> smol::Unblock<std::io::Stdin> {
    smol::Unblock::new(std::io::stdin())
}

async fn inner_main() {
    let ar = Archive::new(stdin());
    let mut entries = ar.entries().unwrap();
    while let Some(file) = entries.next().await {
        let f = file.unwrap();
        println!("{}", f.path().unwrap().display());
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
