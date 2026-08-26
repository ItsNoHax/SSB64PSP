//! ROM identification and validation.

use alloc::string::String;

use crate::{Error, Result};

/// Size of a Super Smash Bros. 64 cartridge dump.
pub const ROM_SIZE: usize = 16 * 1024 * 1024;

/// Game revision.
///
/// Only the US revision is supported. The decompilation also builds a
/// byte-matching JP ROM, so a `Jp` variant is a natural extension — but its
/// constants must be read out of the decomp's JP linker script and verified
/// against a real JP dump before being added here. Guessing them would produce
/// an extractor that silently emits garbage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Region {
    /// North America, internal code `NALE`.
    Us,
}

impl Region {
    /// Every revision this crate knows how to read.
    pub const ALL: &'static [Region] = &[Region::Us];

    /// SHA-1 of the supported dump for this region.
    pub const fn sha1(self) -> &'static str {
        match self {
            Region::Us => "e2929e10fccc0aa84e5776227e798abc07cedabf",
        }
    }

    /// MD5 of the supported dump, for cross-checking against the decomp README.
    pub const fn md5(self) -> &'static str {
        match self {
            Region::Us => "f7c52568a31aadf26e14dc2b6416b2ed",
        }
    }

    /// Four-character internal game code stored at ROM offset 0x3B.
    pub const fn game_code(self) -> &'static [u8; 4] {
        match self {
            Region::Us => b"NALE",
        }
    }

    /// ROM offset of the `relocData` asset archive's file table.
    ///
    /// `lLBRelocTableAddr = relocData_ROM_START` in the decomp's
    /// `symbols/linker_constants.txt`; `relocData` is the segment at
    /// `0x1AC870` in `smashbrothers.us.yaml`.
    pub const fn reloc_table_offset(self) -> usize {
        match self {
            Region::Us => 0x1AC870,
        }
    }

    /// Number of files in the `relocData` archive.
    ///
    /// `lLBRelocTableFilesNum = 0x000854` in the decomp's linker constants.
    pub const fn reloc_file_count(self) -> u32 {
        match self {
            Region::Us => 0x854, // 2132
        }
    }
}

/// What we learned about a candidate ROM file.
#[derive(Debug, Clone)]
pub struct RomInfo {
    pub region: Region,
    pub sha1: String,
    /// Internal name from the cartridge header (offset 0x20, 20 bytes).
    pub internal_name: String,
}

/// Detects the byte order of a raw cartridge dump from its 4-byte magic.
///
/// A big-endian `.z64` starts with `80 37 12 40`. The two common mangled forms
/// are `.v64` (byte-swapped pairs) and `.n64` (word-swapped / little-endian).
fn detect_byte_order(data: &[u8]) -> core::result::Result<(), &'static str> {
    match data.get(..4) {
        Some([0x80, 0x37, 0x12, 0x40]) => Ok(()),
        Some([0x37, 0x80, 0x40, 0x12]) => Err("byte-swapped (.v64)"),
        Some([0x40, 0x12, 0x37, 0x80]) => Err("little-endian (.n64)"),
        _ => Err("unrecognised"),
    }
}

/// Lowercase hex encoding of a byte slice.
fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(DIGITS[(b >> 4) as usize] as char);
        out.push(DIGITS[(b & 0xF) as usize] as char);
    }
    out
}

/// Validates a ROM image and identifies its revision.
///
/// This is the gate the build system runs before extracting anything: an
/// unrecognised ROM must fail loudly rather than silently produce garbage
/// assets that are hard to debug several layers downstream.
pub fn identify(data: &[u8]) -> Result<RomInfo> {
    if data.len() != ROM_SIZE {
        return Err(Error::BadRomSize(data.len()));
    }
    detect_byte_order(data).map_err(|detected| Error::WrongByteOrder { detected })?;

    let sha1 = {
        let mut h = sha1_smol::Sha1::new();
        h.update(data);
        // Hex-encode by hand rather than via `Digest::to_string`: that goes
        // through `Display`, which sha1_smol only provides with `std`, and this
        // crate has to build `no_std` for the device.
        hex(&h.digest().bytes())
    };

    let region = Region::ALL
        .iter()
        .copied()
        .find(|r| r.sha1() == sha1)
        .ok_or_else(|| Error::UnsupportedRevision { sha1: sha1.clone() })?;

    let internal_name = String::from_utf8_lossy(&data[0x20..0x34]).trim_end().into();

    Ok(RomInfo {
        region,
        sha1,
        internal_name,
    })
}

/// Reads `len` bytes at `offset`, or fails with a bounds error.
pub fn slice(data: &[u8], offset: usize, len: usize) -> Result<&[u8]> {
    data.get(offset..offset + len)
        .ok_or(Error::OutOfBounds { offset, len })
}

/// Reads a big-endian `u32` at `offset`.
pub fn read_u32(data: &[u8], offset: usize) -> Result<u32> {
    let b = slice(data, offset, 4)?;
    Ok(u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
}

/// Reads a big-endian `u16` at `offset`.
pub fn read_u16(data: &[u8], offset: usize) -> Result<u16> {
    let b = slice(data, offset, 2)?;
    Ok(u16::from_be_bytes([b[0], b[1]]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    #[test]
    fn rejects_wrong_size() {
        assert!(matches!(identify(&[0u8; 16]), Err(Error::BadRomSize(16))));
    }

    #[test]
    fn rejects_byteswapped_dump() {
        let mut data = vec![0u8; ROM_SIZE];
        data[..4].copy_from_slice(&[0x37, 0x80, 0x40, 0x12]);
        assert!(matches!(
            identify(&data),
            Err(Error::WrongByteOrder {
                detected: "byte-swapped (.v64)"
            })
        ));
    }

    #[test]
    fn rejects_unknown_revision() {
        let mut data = vec![0u8; ROM_SIZE];
        data[..4].copy_from_slice(&[0x80, 0x37, 0x12, 0x40]);
        assert!(matches!(
            identify(&data),
            Err(Error::UnsupportedRevision { .. })
        ));
    }
}
