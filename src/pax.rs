use std::str;

#[cfg(feature = "runtime-async-std")]
use async_std::io;
#[cfg(feature = "runtime-tokio")]
use tokio::io;

use crate::other;

/// An iterator over the pax extensions in an archive entry.
///
/// This iterator yields structures which can themselves be parsed into
/// key/value pairs.
pub struct PaxExtensions<'entry> {
    data: &'entry [u8],
}

/// A key/value pair corresponding to a pax extension.
pub struct PaxExtension<'entry> {
    key: &'entry [u8],
    value: &'entry [u8],
}

pub fn pax_extensions(a: &[u8]) -> PaxExtensions<'_> {
    PaxExtensions { data: a }
}

impl<'entry> Iterator for PaxExtensions<'entry> {
    type Item = io::Result<PaxExtension<'entry>>;

    fn next(&mut self) -> Option<io::Result<PaxExtension<'entry>>> {
        while !self.data.is_empty() {
            if self.data[0] == 0 || self.data[0] == b'\n' {
                self.data = &self.data[1..];
                continue;
            }

            let space_idx = match self.data.iter().position(|b| *b == b' ') {
                Some(idx) => idx,
                None => {
                    self.data = &[];
                    return Some(Err(other("malformed pax extension: no space after length")));
                }
            };

            let len_str = match str::from_utf8(&self.data[..space_idx]) {
                Ok(s) => s,
                Err(_) => {
                    self.data = &[];
                    return Some(Err(other("malformed pax extension: non-utf8 length")));
                }
            };

            let total_len = match len_str.parse::<usize>() {
                Ok(len) if len > space_idx => len,
                _ => {
                    self.data = &[];
                    return Some(Err(other("malformed pax extension: invalid length")));
                }
            };

            if self.data.len() < total_len {
                self.data = &[];
                return Some(Err(other("malformed pax extension: record truncated")));
            }

            let record = &self.data[..total_len];
            self.data = &self.data[total_len..];

            let record_content = if record.ends_with(b"\n") {
                &record[..record.len() - 1]
            } else {
                record
            };

            let kv = &record_content[space_idx + 1..];
            let equals_idx = match kv.iter().position(|b| *b == b'=') {
                Some(idx) => idx,
                None => {
                    return Some(Err(other("malformed pax extension: no '=' in record")));
                }
            };

            return Some(Ok(PaxExtension {
                key: &kv[..equals_idx],
                value: &kv[equals_idx + 1..],
            }));
        }

        None
    }
}

impl<'entry> PaxExtension<'entry> {
    /// Returns the key for this key/value pair parsed as a string.
    ///
    /// May fail if the key isn't actually utf-8.
    pub fn key(&self) -> Result<&'entry str, str::Utf8Error> {
        str::from_utf8(self.key)
    }

    /// Returns the underlying raw bytes for the key of this key/value pair.
    pub fn key_bytes(&self) -> &'entry [u8] {
        self.key
    }

    /// Returns the value for this key/value pair parsed as a string.
    ///
    /// May fail if the value isn't actually utf-8.
    pub fn value(&self) -> Result<&'entry str, str::Utf8Error> {
        str::from_utf8(self.value)
    }

    /// Returns the underlying raw bytes for this value of this key/value pair.
    pub fn value_bytes(&self) -> &'entry [u8] {
        self.value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_standard_pax() {
        let data = b"20 path=foo/bar/baz\n14 size=12345\n";
        let mut extensions = pax_extensions(data);
        let ext1 = extensions.next().unwrap().unwrap();
        assert_eq!(ext1.key().unwrap(), "path");
        assert_eq!(ext1.value().unwrap(), "foo/bar/baz");

        let ext2 = extensions.next().unwrap().unwrap();
        assert_eq!(ext2.key().unwrap(), "size");
        assert_eq!(ext2.value().unwrap(), "12345");

        assert!(extensions.next().is_none());
    }

    #[test]
    fn test_parse_pax_with_binary_newlines() {
        // Value containing embedded '\n' (0x0A) bytes, as found in xattr signatures
        let mut data = Vec::new();
        let value = b"binary\nwith\nnewlines\nand\x00nulls";
        let key = b"SCHILY.xattr.test";
        let rest_len = 3 + key.len() + value.len();
        let len = rest_len + 2; // 2 digit length
        let len_str = format!("{}", len);
        data.extend_from_slice(len_str.as_bytes());
        data.push(b' ');
        data.extend_from_slice(key);
        data.push(b'=');
        data.extend_from_slice(value);
        data.push(b'\n');
        data.extend_from_slice(b"14 size=12345\n");

        let mut extensions = pax_extensions(&data);
        let ext1 = extensions.next().unwrap().unwrap();
        assert_eq!(ext1.key().unwrap(), "SCHILY.xattr.test");
        assert_eq!(ext1.value_bytes(), value);

        let ext2 = extensions.next().unwrap().unwrap();
        assert_eq!(ext2.key().unwrap(), "size");
        assert_eq!(ext2.value().unwrap(), "12345");

        assert!(extensions.next().is_none());
    }
}
