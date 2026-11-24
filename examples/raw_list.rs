//! An example of listing raw entries in an archive.
//!
//! Takes a tarball on stdin and prints out all of the entries inside.

use std::io;

use async_tar::Archive;
#[cfg(feature = "runtime-smol")]
use smol::{Unblock, stream::StreamExt};
#[cfg(feature = "runtime-tokio")]
use tokio::{io::stdin, runtime::Runtime};
#[cfg(feature = "runtime-tokio")]
use tokio_stream::StreamExt;

#[cfg(feature = "runtime-smol")]
fn stdin() -> Unblock<io::Stdin> {
    Unblock::new(io::stdin())
}

async fn inner_main() -> io::Result<()> {
    let ar = Archive::new(stdin());
    let mut i = 0;
    let mut entries = ar.entries_raw()?;
    while let Some(file) = entries.next().await {
        println!("-------------------------- Entry {}", i);
        let mut f = file?;
        println!("path: {}", f.path()?.display());
        println!("size: {}", f.header().size()?);
        println!("entry size: {}", f.header().entry_size()?);
        println!("link name: {:?}", f.link_name()?);
        println!("file type: {:#x}", f.header().entry_type().as_byte());
        println!("mode: {:#o}", f.header().mode()?);
        println!("uid: {}", f.header().uid()?);
        println!("gid: {}", f.header().gid()?);
        println!("mtime: {}", f.header().mtime()?);
        println!(
            "username: {:?}",
            f.header().username().map_err(io::Error::other)?
        );
        println!(
            "groupname: {:?}",
            f.header().groupname().map_err(io::Error::other)?
        );

        if f.header().as_ustar().is_some() {
            println!("kind: UStar");
        } else if f.header().as_gnu().is_some() {
            println!("kind: GNU");
        } else {
            println!("kind: normal");
        }

        if let Ok(Some(extensions)) = f.pax_extensions().await {
            println!("pax extensions:");
            for e in extensions {
                let e = e?;
                println!(
                    "\t{:?} = {:?}",
                    String::from_utf8_lossy(e.key_bytes()),
                    String::from_utf8_lossy(e.value_bytes())
                );
            }
        }
        i += 1;
    }
    Ok(())
}

#[cfg(feature = "runtime-smol")]
fn main() -> io::Result<()> {
    smol::block_on(inner_main())
}

#[cfg(feature = "runtime-tokio")]
fn main() -> io::Result<()> {
    Runtime::new()?.block_on(inner_main())
}
