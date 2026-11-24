//! Indicate for the type of file described by a header.
//!
//! Each `Header` has an `entry_type` method returning an instance of this type
//! which can be used to inspect what the header is describing.
//!
//! See <https://en.wikipedia.org/wiki/Tar_%28computing%29#UStar_format>

/// A non-exhaustive enum representing the possible entry types
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[non_exhaustive]
pub enum EntryType {
    /// Regular file
    Regular,
    /// Hard link
    Link,
    /// Symbolic link
    Symlink,
    /// Character device
    Char,
    /// Block device
    Block,
    /// Directory
    Directory,
    /// Named pipe (fifo)
    Fifo,
    /// Implementation-defined 'high-performance' type, treated as regular file
    Continuous,
    /// GNU extension - long file name
    GNULongName,
    /// GNU extension - long link name (link target)
    GNULongLink,
    /// GNU extension - sparse file
    GNUSparse,
    /// Global extended header
    XGlobalHeader,
    /// Extended Header
    XHeader,
    /// Unknown header,
    Other(u8),
}

impl EntryType {
    /// Creates a new entry type from a raw byte.
    ///
    /// Note that the other named constructors of entry type may be more
    /// appropriate to create a file type from.
    pub fn new(byte: u8) -> Self {
        match byte {
            b'\x00' | b'0' => Self::Regular,
            b'1' => Self::Link,
            b'2' => Self::Symlink,
            b'3' => Self::Char,
            b'4' => Self::Block,
            b'5' => Self::Directory,
            b'6' => Self::Fifo,
            b'7' => Self::Continuous,
            b'x' => Self::XHeader,
            b'g' => Self::XGlobalHeader,
            b'L' => Self::GNULongName,
            b'K' => Self::GNULongLink,
            b'S' => Self::GNUSparse,
            other => Self::Other(other),
        }
    }

    /// Returns the raw underlying byte that this entry type represents.
    pub fn as_byte(self) -> u8 {
        match self {
            Self::Regular => b'0',
            Self::Link => b'1',
            Self::Symlink => b'2',
            Self::Char => b'3',
            Self::Block => b'4',
            Self::Directory => b'5',
            Self::Fifo => b'6',
            Self::Continuous => b'7',
            Self::XHeader => b'x',
            Self::XGlobalHeader => b'g',
            Self::GNULongName => b'L',
            Self::GNULongLink => b'K',
            Self::GNUSparse => b'S',
            Self::Other(other) => other,
        }
    }

    /// Creates a new entry type representing a regular file.
    pub fn file() -> Self {
        Self::Regular
    }

    /// Creates a new entry type representing a hard link.
    pub fn hard_link() -> Self {
        Self::Link
    }

    /// Creates a new entry type representing a symlink.
    pub fn symlink() -> Self {
        Self::Symlink
    }

    /// Creates a new entry type representing a character special device.
    pub fn character_special() -> Self {
        Self::Char
    }

    /// Creates a new entry type representing a block special device.
    pub fn block_special() -> Self {
        Self::Block
    }

    /// Creates a new entry type representing a directory.
    pub fn dir() -> Self {
        Self::Directory
    }

    /// Creates a new entry type representing a FIFO.
    pub fn fifo() -> Self {
        Self::Fifo
    }

    /// Creates a new entry type representing a contiguous file.
    pub fn contiguous() -> Self {
        Self::Continuous
    }

    /// Returns whether this type represents a regular file.
    pub fn is_file(self) -> bool {
        self == Self::Regular
    }

    /// Returns whether this type represents a hard link.
    pub fn is_hard_link(self) -> bool {
        self == Self::Link
    }

    /// Returns whether this type represents a symlink.
    pub fn is_symlink(self) -> bool {
        self == Self::Symlink
    }

    /// Returns whether this type represents a character special device.
    pub fn is_character_special(self) -> bool {
        self == Self::Char
    }

    /// Returns whether this type represents a block special device.
    pub fn is_block_special(self) -> bool {
        self == Self::Block
    }

    /// Returns whether this type represents a directory.
    pub fn is_dir(self) -> bool {
        self == Self::Directory
    }

    /// Returns whether this type represents a FIFO.
    pub fn is_fifo(self) -> bool {
        self == Self::Fifo
    }

    /// Returns whether this type represents a contiguous file.
    pub fn is_contiguous(self) -> bool {
        self == Self::Continuous
    }

    /// Returns whether this type represents a GNU long name header.
    pub fn is_gnu_longname(self) -> bool {
        self == Self::GNULongName
    }

    /// Returns whether this type represents a GNU sparse header.
    pub fn is_gnu_sparse(self) -> bool {
        self == Self::GNUSparse
    }

    /// Returns whether this type represents a GNU long link header.
    pub fn is_gnu_longlink(self) -> bool {
        self == Self::GNULongLink
    }

    /// Returns whether this type represents a GNU long name header.
    pub fn is_pax_global_extensions(self) -> bool {
        self == Self::XGlobalHeader
    }

    /// Returns whether this type represents a GNU long link header.
    pub fn is_pax_local_extensions(self) -> bool {
        self == Self::XHeader
    }

    /// Returns whether this type carries only metadata, such as a GNU volume label (V).
    pub(crate) fn is_metadata(self) -> bool {
        self.is_pax_global_extensions()
            || self.is_pax_local_extensions()
            || self.is_gnu_longname()
            || self.is_gnu_longlink()
            || self.as_byte() == b'V'
    }
}
