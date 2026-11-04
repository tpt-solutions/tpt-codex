// SPDX-License-Identifier: MIT OR Apache-2.0
//! A small ZIP container reader and writer, built for the Office formats (`.xlsx`, `.docx`).
//!
//! Written here, rather than taken from the `zip` crate, because `zip`'s default features pull in
//! Apache-2.0-only and other crates that the license gate rejects. DEFLATE comes from `flate2`
//! with its pure-Rust backend, and CRC-32 from `crc32fast`.
//!
//! Supported on read: stored (method 0) and DEFLATE (method 8) entries, from one disk, without ZIP64,
//! without encryption. Anything else is an error, not a guess. Every entry is checked against its
//! declared size and CRC-32, and an entry may not expand beyond [`MAX_ENTRY_BYTES`].
//!
//! Supported on write: DEFLATE entries with ASCII names and a fixed timestamp, so the same input
//! always gives the same bytes.

use std::fmt;
use std::io::{Read, Write};

use flate2::Compression;
use flate2::read::DeflateDecoder;
use flate2::write::DeflateEncoder;

/// Largest uncompressed size accepted for one entry. Office parts are far smaller.
pub const MAX_ENTRY_BYTES: u64 = 64 * 1024 * 1024;
/// Most entries accepted in one archive.
pub const MAX_ENTRIES: usize = 4096;
/// Largest total uncompressed size, summed over the declared sizes of all entries.
pub const MAX_TOTAL_BYTES: u64 = 256 * 1024 * 1024;

const SIG_LOCAL: u32 = 0x0403_4b50;
const SIG_CENTRAL: u32 = 0x0201_4b50;
const SIG_END: u32 = 0x0605_4b50;
const METHOD_STORED: u16 = 0;
const METHOD_DEFLATE: u16 = 8;
/// 1980-01-01, the earliest DOS date. Used for every written entry.
const DOS_DATE: u16 = (1 << 5) | 1;

/// Why an archive could not be read or written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ZipError {
    /// The bytes are not a ZIP archive (no end-of-central-directory record).
    NotZip,
    /// The archive ends before a structure it declares.
    Truncated,
    /// A structure is inconsistent: wrong signature, bad offset, size mismatch.
    Corrupt(String),
    /// A feature this reader does not support: ZIP64, encryption, other compression methods.
    Unsupported(String),
    /// An entry expands beyond [`MAX_ENTRY_BYTES`].
    TooLarge {
        /// The entry's name.
        name: String,
    },
    /// An entry's CRC-32 does not match its data.
    BadChecksum {
        /// The entry's name.
        name: String,
    },
    /// The named entry is not in the archive.
    NotFound(String),
    /// A name given to the writer is not usable (empty, absolute, containing `..`, or not ASCII),
    /// or was given twice.
    BadName(String),
}

impl fmt::Display for ZipError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotZip => f.write_str("not a zip archive"),
            Self::Truncated => f.write_str("zip archive is truncated"),
            Self::Corrupt(what) => write!(f, "zip archive is corrupt: {what}"),
            Self::Unsupported(what) => write!(f, "zip feature not supported: {what}"),
            Self::TooLarge { name } => write!(f, "zip entry '{name}' is larger than the limit"),
            Self::BadChecksum { name } => write!(f, "zip entry '{name}' fails its checksum"),
            Self::NotFound(name) => write!(f, "zip entry '{name}' not found"),
            Self::BadName(name) => write!(f, "zip entry name '{name}' is not allowed"),
        }
    }
}

impl std::error::Error for ZipError {}

#[derive(Debug, Clone)]
struct Entry {
    name: String,
    method: u16,
    crc: u32,
    compressed: usize,
    uncompressed: usize,
    data_start: usize,
}

/// A read-only view of a ZIP archive held in memory.
#[derive(Debug)]
pub struct Archive<'a> {
    data: &'a [u8],
    entries: Vec<Entry>,
}

fn u16_at(data: &[u8], at: usize) -> Result<u16, ZipError> {
    data.get(at..at + 2)
        .map(|b| u16::from_le_bytes([b[0], b[1]]))
        .ok_or(ZipError::Truncated)
}

fn u32_at(data: &[u8], at: usize) -> Result<u32, ZipError> {
    data.get(at..at + 4)
        .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        .ok_or(ZipError::Truncated)
}

impl<'a> Archive<'a> {
    /// Reads the central directory of `data`. Entry data is checked when it is read.
    pub fn open(data: &'a [u8]) -> Result<Self, ZipError> {
        // The end record is 22 bytes plus a comment of up to 65 535 bytes, at the very end.
        let window_start = data.len().saturating_sub(22 + 65_535);
        let eocd = (window_start..=data.len().saturating_sub(22))
            .rev()
            .find(|&at| data.get(at..at + 4) == Some(&SIG_END.to_le_bytes()))
            .ok_or(ZipError::NotZip)?;

        let disk = u16_at(data, eocd + 4)?;
        let cd_disk = u16_at(data, eocd + 6)?;
        let count = u16_at(data, eocd + 10)? as usize;
        let cd_size = u32_at(data, eocd + 12)? as usize;
        let cd_offset = u32_at(data, eocd + 16)? as usize;
        if disk != 0 || cd_disk != 0 {
            return Err(ZipError::Unsupported("multi-disk archives".into()));
        }
        if count == 0xFFFF || cd_offset == 0xFFFF_FFFF {
            return Err(ZipError::Unsupported("ZIP64".into()));
        }
        if count > MAX_ENTRIES {
            return Err(ZipError::Unsupported(format!(
                "archives with more than {MAX_ENTRIES} entries"
            )));
        }
        if cd_offset.checked_add(cd_size).is_none_or(|end| end > eocd) {
            return Err(ZipError::Corrupt(
                "central directory is outside the archive".into(),
            ));
        }

        let mut entries = Vec::with_capacity(count);
        let mut at = cd_offset;
        for _ in 0..count {
            if u32_at(data, at)? != SIG_CENTRAL {
                return Err(ZipError::Corrupt("bad central directory entry".into()));
            }
            let flags = u16_at(data, at + 8)?;
            let method = u16_at(data, at + 10)?;
            let crc = u32_at(data, at + 16)?;
            let compressed = u32_at(data, at + 20)? as usize;
            let uncompressed = u32_at(data, at + 24)? as usize;
            let name_len = u16_at(data, at + 28)? as usize;
            let extra_len = u16_at(data, at + 30)? as usize;
            let comment_len = u16_at(data, at + 32)? as usize;
            let local = u32_at(data, at + 42)? as usize;
            let name_bytes = data
                .get(at + 46..at + 46 + name_len)
                .ok_or(ZipError::Truncated)?;
            let name = String::from_utf8(name_bytes.to_vec())
                .map_err(|_| ZipError::Unsupported("entry names that are not UTF-8".into()))?;
            at += 46 + name_len + extra_len + comment_len;

            if flags & 1 != 0 {
                return Err(ZipError::Unsupported(format!("encrypted entry '{name}'")));
            }
            if method != METHOD_STORED && method != METHOD_DEFLATE {
                return Err(ZipError::Unsupported(format!(
                    "compression method {method} in '{name}'"
                )));
            }
            if compressed == 0xFFFF_FFFF_usize || uncompressed == 0xFFFF_FFFF_usize {
                return Err(ZipError::Unsupported("ZIP64 entry sizes".into()));
            }

            // The local header repeats the name and has its own extra field length.
            if u32_at(data, local)? != SIG_LOCAL {
                return Err(ZipError::Corrupt(format!("bad local header for '{name}'")));
            }
            let local_name = u16_at(data, local + 26)? as usize;
            let local_extra = u16_at(data, local + 28)? as usize;
            let data_start = local + 30 + local_name + local_extra;
            if data_start + compressed > data.len() {
                return Err(ZipError::Truncated);
            }
            entries.push(Entry {
                name,
                method,
                crc,
                compressed,
                uncompressed,
                data_start,
            });
        }
        let total: u64 = entries.iter().map(|e| e.uncompressed as u64).sum();
        if total > MAX_TOTAL_BYTES {
            return Err(ZipError::Unsupported(
                "archives that declare more than 256 MiB of content".into(),
            ));
        }
        Ok(Self { data, entries })
    }

    /// The names of all entries, in archive order.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.entries.iter().map(|e| e.name.as_str())
    }

    /// Reads and checks one entry.
    pub fn read(&self, name: &str) -> Result<Vec<u8>, ZipError> {
        let entry = self
            .entries
            .iter()
            .find(|e| e.name == name)
            .ok_or_else(|| ZipError::NotFound(name.to_string()))?;
        if entry.uncompressed as u64 > MAX_ENTRY_BYTES {
            return Err(ZipError::TooLarge {
                name: name.to_string(),
            });
        }
        let raw = &self.data[entry.data_start..entry.data_start + entry.compressed];
        let out = match entry.method {
            METHOD_STORED => {
                if raw.len() != entry.uncompressed {
                    return Err(ZipError::Corrupt(format!("size mismatch in '{name}'")));
                }
                raw.to_vec()
            }
            _ => {
                let mut out = Vec::with_capacity(entry.uncompressed);
                // Read one byte past the limit, so an entry that expands too far is caught.
                DeflateDecoder::new(raw)
                    .take(MAX_ENTRY_BYTES + 1)
                    .read_to_end(&mut out)
                    .map_err(|_| ZipError::Corrupt(format!("cannot inflate '{name}'")))?;
                if out.len() as u64 > MAX_ENTRY_BYTES {
                    return Err(ZipError::TooLarge {
                        name: name.to_string(),
                    });
                }
                if out.len() != entry.uncompressed {
                    return Err(ZipError::Corrupt(format!("size mismatch in '{name}'")));
                }
                out
            }
        };
        if crc32fast::hash(&out) != entry.crc {
            return Err(ZipError::BadChecksum {
                name: name.to_string(),
            });
        }
        Ok(out)
    }
}

/// Builds a ZIP archive in memory.
#[derive(Debug, Default)]
pub struct ArchiveWriter {
    entries: Vec<(String, Vec<u8>)>,
}

impl ArchiveWriter {
    /// An empty archive.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds an entry. Names must be ASCII, relative, without `..`, and unique.
    pub fn add(&mut self, name: &str, data: Vec<u8>) -> Result<(), ZipError> {
        let bad = name.is_empty()
            || !name.is_ascii()
            || name.starts_with('/')
            || name.split('/').any(|part| part == "..")
            || name.len() > u16::MAX as usize;
        if bad || self.entries.iter().any(|(n, _)| n == name) {
            return Err(ZipError::BadName(name.to_string()));
        }
        if self.entries.len() >= MAX_ENTRIES {
            return Err(ZipError::Unsupported(format!(
                "archives with more than {MAX_ENTRIES} entries"
            )));
        }
        if data.len() as u64 > u32::MAX as u64 {
            return Err(ZipError::Unsupported("entries of 4 GiB or more".into()));
        }
        self.entries.push((name.to_string(), data));
        Ok(())
    }

    /// Writes the archive. The output depends only on the entries added, in order.
    pub fn finish(self) -> Result<Vec<u8>, ZipError> {
        let mut out = Vec::new();
        let mut central = Vec::new();
        for (name, data) in &self.entries {
            let crc = crc32fast::hash(data);
            let compressed = deflate(data)?;
            let offset = out.len() as u32;
            let name_bytes = name.as_bytes();

            put_u32(&mut out, SIG_LOCAL);
            put_u16(&mut out, 20); // version needed
            put_u16(&mut out, 0); // flags
            put_u16(&mut out, METHOD_DEFLATE);
            put_u16(&mut out, 0); // time
            put_u16(&mut out, DOS_DATE);
            put_u32(&mut out, crc);
            put_u32(&mut out, compressed.len() as u32);
            put_u32(&mut out, data.len() as u32);
            put_u16(&mut out, name_bytes.len() as u16);
            put_u16(&mut out, 0); // extra
            out.extend_from_slice(name_bytes);
            out.extend_from_slice(&compressed);

            put_u32(&mut central, SIG_CENTRAL);
            put_u16(&mut central, 20); // version made by
            put_u16(&mut central, 20); // version needed
            put_u16(&mut central, 0); // flags
            put_u16(&mut central, METHOD_DEFLATE);
            put_u16(&mut central, 0); // time
            put_u16(&mut central, DOS_DATE);
            put_u32(&mut central, crc);
            put_u32(&mut central, compressed.len() as u32);
            put_u32(&mut central, data.len() as u32);
            put_u16(&mut central, name_bytes.len() as u16);
            put_u16(&mut central, 0); // extra
            put_u16(&mut central, 0); // comment
            put_u16(&mut central, 0); // disk
            put_u16(&mut central, 0); // internal attributes
            put_u32(&mut central, 0); // external attributes
            put_u32(&mut central, offset);
            central.extend_from_slice(name_bytes);
        }
        let cd_offset = out.len();
        if cd_offset as u64 + central.len() as u64 > u32::MAX as u64 {
            return Err(ZipError::Unsupported("archives of 4 GiB or more".into()));
        }
        out.extend_from_slice(&central);

        put_u32(&mut out, SIG_END);
        put_u16(&mut out, 0); // disk
        put_u16(&mut out, 0); // central directory disk
        put_u16(&mut out, self.entries.len() as u16);
        put_u16(&mut out, self.entries.len() as u16);
        put_u32(&mut out, central.len() as u32);
        put_u32(&mut out, cd_offset as u32);
        put_u16(&mut out, 0); // comment
        Ok(out)
    }
}

fn deflate(data: &[u8]) -> Result<Vec<u8>, ZipError> {
    let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
    encoder
        .write_all(data)
        .and_then(|()| encoder.finish())
        .map_err(|e| ZipError::Corrupt(format!("cannot deflate: {e}")))
}

fn put_u16(out: &mut Vec<u8>, v: u16) {
    out.extend_from_slice(&v.to_le_bytes());
}

fn put_u32(out: &mut Vec<u8>, v: u32) {
    out.extend_from_slice(&v.to_le_bytes());
}
