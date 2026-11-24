//! An example of writing an archive from files on disk.

use std::io;

use async_tar::Builder;
#[cfg(feature = "runtime-smol")]
use smol::fs::File;
#[cfg(feature = "runtime-tokio")]
use tokio::{fs::File, runtime::Runtime};

async fn inner_main() -> io::Result<()> {
    let file = File::create("foo.tar").await?;
    let mut a = Builder::new(file);

    a.append_path("README.md").await?;
    a.append_file("lib.rs", &mut File::open("src/lib.rs").await?)
        .await?;
    a.finish().await
}

#[cfg(feature = "runtime-smol")]
fn main() -> io::Result<()> {
    smol::block_on(inner_main())
}

#[cfg(feature = "runtime-tokio")]
fn main() -> io::Result<()> {
    Runtime::new()?.block_on(inner_main())
}
