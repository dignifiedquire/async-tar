use async_std::io::{Error, ErrorKind};

pub use crate::async_tar::archive::{Archive, Entries};
pub use crate::async_tar::builder::Builder;
pub use crate::async_tar::entry::{Entry, Unpacked};
pub use crate::async_tar::entry_type::EntryType;
pub use crate::async_tar::header::GnuExtSparseHeader;
pub use crate::async_tar::header::{
    GnuHeader, GnuSparseHeader, Header, HeaderMode, OldHeader, UstarHeader,
};
pub use crate::async_tar::pax::{PaxExtension, PaxExtensions};

mod archive;
mod builder;
mod entry;
mod entry_type;
mod error;
mod header;
mod pax;

fn other(msg: &str) -> Error {
    Error::new(ErrorKind::Other, msg)
}
