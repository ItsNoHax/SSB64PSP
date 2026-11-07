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

/// A cartridge dump's byte order, from its first word.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ByteOrder {
    /// Big-endian, as the N64 reads it: `80 37 12 40`.
    Z64,
    /// Byte pairs swapped: `37 80 40 12`.
    V64,
    /// Each 32-bit word reversed (little-endian): `40 12 37 80`.
    N64,
}

impl ByteOrder {
    /// The order of a dump starting with `magic` (its first 4 bytes).
    pub fn detect(magic: &[u8]) -> Option<Self> {
        match magic.get(..4)? {
            [0x80, 0x37, 0x12, 0x40] => Some(Self::Z64),
            [0x37, 0x80, 0x40, 0x12] => Some(Self::V64),
            [0x40, 0x12, 0x37, 0x80] => Some(Self::N64),
            _ => None,
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::Z64 => "big-endian (.z64)",
            Self::V64 => "byte-swapped (.v64)",
            Self::N64 => "little-endian (.n64)",
        }
    }

    /// Swaps raw dump bytes to `.z64` in place. `raw` starts at a dump
    /// offset that is a multiple of 4 and its length is one too.
    pub fn to_z64(self, raw: &mut [u8]) {
        match self {
            Self::Z64 => {}
            Self::V64 => raw
                .as_chunks_mut::<2>()
                .0
                .iter_mut()
                .for_each(|w| w.swap(0, 1)),
            Self::N64 => raw
                .as_chunks_mut::<4>()
                .0
                .iter_mut()
                .for_each(|w| w.reverse()),
        }
    }
}

/// Detects the byte order of a raw cartridge dump from its 4-byte magic:
/// [`identify`] reads `.z64` only.
fn detect_byte_order(data: &[u8]) -> core::result::Result<(), &'static str> {
    match ByteOrder::detect(data) {
        Some(ByteOrder::Z64) => Ok(()),
        Some(order) => Err(order.name()),
        None => Err("unrecognised"),
    }
}

/// Swaps a whole `.z64`, `.v64` or `.n64` dump to `.z64` in place,
/// returning the order it was in. A length that is not a multiple of 4
/// or an unknown first word is refused (and `data` left alone).
pub fn normalize(data: &mut [u8]) -> Result<ByteOrder> {
    let order = ByteOrder::detect(data).ok_or(Error::WrongByteOrder {
        detected: "unrecognised",
    })?;
    if !data.len().is_multiple_of(4) {
        return Err(Error::BadRomSize(data.len()));
    }
    order.to_z64(data);
    Ok(order)
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
    identify_reader(crate::rom_read::RomRef::Bytes(data))
}

/// [`identify`] through a reader (a `rom_read::PagedRom` on the PSP: one
/// forward pass in 64 KiB reads, the same SHA-1).
pub fn identify_reader(rom: crate::rom_read::RomRef<'_>) -> Result<RomInfo> {
    if rom.len() != ROM_SIZE {
        return Err(Error::BadRomSize(rom.len()));
    }
    detect_byte_order(&rom.read(0, 4)?).map_err(|detected| Error::WrongByteOrder { detected })?;

    let sha1 = {
        let mut h = sha1_smol::Sha1::new();
        match rom.as_bytes() {
            Some(data) => h.update(data),
            None => {
                let mut buf = alloc::vec![0; 64 * 1024];
                for at in (0..rom.len()).step_by(buf.len()) {
                    let n = buf.len().min(rom.len() - at);
                    rom.read_into(at, &mut buf[..n])?;
                    h.update(&buf[..n]);
                }
            }
        }
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

    let internal_name = String::from_utf8_lossy(&rom.read(0x20, 0x14)?)
        .trim_end()
        .into();

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
    use alloc::vec::Vec;

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

    /// `.v64` and `.n64` dumps swap back to the `.z64` bytes; garbage is
    /// refused untouched.
    #[test]
    fn normalizes_swapped_dumps() {
        let z64: Vec<u8> = [0x80, 0x37, 0x12, 0x40, 1, 2, 3, 4, 5, 6, 7, 8].to_vec();
        let v64: Vec<u8> = [0x37, 0x80, 0x40, 0x12, 2, 1, 4, 3, 6, 5, 8, 7].to_vec();
        let n64: Vec<u8> = [0x40, 0x12, 0x37, 0x80, 4, 3, 2, 1, 8, 7, 6, 5].to_vec();
        for (mut d, want) in [
            (z64.clone(), ByteOrder::Z64),
            (v64, ByteOrder::V64),
            (n64, ByteOrder::N64),
        ] {
            assert_eq!(normalize(&mut d).unwrap(), want);
            assert_eq!(d, z64);
        }
        let mut garbage = vec![1u8, 2, 3, 4];
        assert!(matches!(
            normalize(&mut garbage),
            Err(Error::WrongByteOrder {
                detected: "unrecognised"
            })
        ));
        assert_eq!(garbage, [1, 2, 3, 4]);
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
