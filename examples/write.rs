extern crate async_tar;

use async_tar::Builder;
use smol::fs::File;

fn main() {
    smol::block_on(async {
        let file = File::create("foo.tar").await.unwrap();
        let mut a = Builder::new(file);

        a.append_path("README.md").await.unwrap();
        a.append_file("lib.rs", &mut File::open("src/lib.rs").await.unwrap())
            .await
            .unwrap();
    });
}
