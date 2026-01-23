//! An example of extracting a file in an archive.
//!
//! Takes a tarball on standard input, looks for an entry with a listed file
//! name as the first argument provided, and then prints the contents of that
//! file to stdout.

extern crate async_tar;

use smol::{io::copy, stream::StreamExt, Unblock};
use std::env::args_os;
use std::io::{stdin, stdout};
use std::path::Path;

use async_tar::Archive;

fn main() {
    let first_arg = args_os().nth(1).unwrap();
    let filename = Path::new(&first_arg);

    let input = Unblock::new(stdin());
    let mut output = Unblock::new(stdout());

    smol::block_on(async {
        let archives = Archive::new(input);
        let mut entries = archives.entries().unwrap();

        while let Some(file) = entries.next().await {
            let mut f = file.unwrap();
            if f.path().unwrap() == filename {
                copy(&mut f, &mut output).await.unwrap();
            }
        }
    });
}
