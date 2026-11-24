use std::str;

#[cfg(feature = "runtime-smol")]
use futures_lite::io;
#[cfg(feature = "runtime-tokio")]
use tokio::io;

use crate::other;

/// An iterator over the pax extensions in an archive entry.
///
/// This iterator yields structures which can themselves be parsed into
/// key/value pairs.
#[derive(Debug)]
pub struct PaxExtensions<'entry> {
    data: &'entry [u8],
}

/// A key/value pair corresponding to a pax extension.
#[derive(Debug)]
pub struct PaxExtension<'entry> {
    key: &'entry [u8],
    value: &'entry [u8],
}

pub(crate) fn pax_extensions(a: &[u8]) -> PaxExtensions<'_> {
    PaxExtensions { data: a }
}

impl<'entry> Iterator for PaxExtensions<'entry> {
    type Item = io::Result<PaxExtension<'entry>>;

    fn next(&mut self) -> Option<io::Result<PaxExtension<'entry>>> {
        let first = *self.data.first()?;

        // A NUL ends the records, and only NUL padding may follow it.
        if first == 0 {
            let is_padding = self.data.iter().all(|b| *b == 0);
            self.data = &[];
            return (!is_padding).then(|| Err(other("malformed pax extension: data after NUL")));
        }

        let Some((len_field, _)) = split_once(self.data, b' ') else {
            self.data = &[];
            return Some(Err(other("malformed pax extension: no space after length")));
        };

        let Ok(len_str) = str::from_utf8(len_field) else {
            self.data = &[];
            return Some(Err(other("malformed pax extension: non-utf8 length")));
        };

        let is_decimal = !len_str.is_empty() && len_str.bytes().all(|b| b.is_ascii_digit());
        let total_len = match len_str.parse::<usize>() {
            Ok(len) if is_decimal && len > len_field.len() => len,
            _ => {
                self.data = &[];
                return Some(Err(other("malformed pax extension: invalid length")));
            }
        };

        let Some((record, rest)) = self.data.split_at_checked(total_len) else {
            self.data = &[];
            return Some(Err(other("malformed pax extension: record truncated")));
        };
        self.data = rest;

        let Some(record_content) = record.strip_suffix(b"\n") else {
            self.data = &[];
            return Some(Err(other(
                "malformed pax extension: no newline at its length",
            )));
        };

        // The record starts with the length field, so its first space ends that field.
        let Some((key, value)) =
            split_once(record_content, b' ').and_then(|(_, kv)| split_once(kv, b'='))
        else {
            return Some(Err(other("malformed pax extension: no '=' in record")));
        };

        Some(Ok(PaxExtension { key, value }))
    }
}

/// Splits `bytes` around the first `delimiter`, or returns `None` without one.
fn split_once(bytes: &[u8], delimiter: u8) -> Option<(&[u8], &[u8])> {
    let index = bytes.iter().position(|b| *b == delimiter)?;
    Some((bytes.get(..index)?, bytes.get(index + 1..)?))
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
