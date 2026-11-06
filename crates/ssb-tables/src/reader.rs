//! The `TABLES` section's directory (format: `ssb_tablegen::tables`).

use crate::{Kind, TableId, ENTRY_SIZE, HEADER_SIZE, MAGIC, SECTION_VERSION};

/// Why a section was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SectionError {
    /// Shorter than its header or directory.
    TooSmall,
    BadMagic(u32),
    BadVersion(u32),
    /// Directory entry `n` names an unknown kind or lies outside the section.
    BadEntry(u32),
}

impl core::fmt::Display for SectionError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            SectionError::TooSmall => write!(f, "the tables section is truncated"),
            SectionError::BadMagic(m) => write!(f, "the tables section has magic {m:#010x}"),
            SectionError::BadVersion(v) => write!(
                f,
                "the tables section is version {v}; this build reads {SECTION_VERSION}"
            ),
            SectionError::BadEntry(n) => write!(f, "tables section entry {n} is malformed"),
        }
    }
}

fn u16_at(d: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([d[at], d[at + 1]])
}

fn u32_at(d: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([d[at], d[at + 1], d[at + 2], d[at + 3]])
}

/// A validated section: every directory entry names a known kind and lies
/// inside the bytes.
#[derive(Debug, Clone, Copy)]
pub struct Section<'a> {
    bytes: &'a [u8],
    count: usize,
}

impl<'a> Section<'a> {
    /// Checks the header and every directory entry.
    pub fn parse(bytes: &'a [u8]) -> Result<Section<'a>, SectionError> {
        if bytes.len() < HEADER_SIZE {
            return Err(SectionError::TooSmall);
        }
        let magic = u32_at(bytes, 0);
        if magic != MAGIC {
            return Err(SectionError::BadMagic(magic));
        }
        let version = u32_at(bytes, 4);
        if version != SECTION_VERSION {
            return Err(SectionError::BadVersion(version));
        }
        let count = u32_at(bytes, 8) as usize;
        let dir_end = count
            .checked_mul(ENTRY_SIZE)
            .and_then(|n| n.checked_add(HEADER_SIZE))
            .ok_or(SectionError::TooSmall)?;
        if bytes.len() < dir_end {
            return Err(SectionError::TooSmall);
        }
        let section = Section { bytes, count };
        for n in 0..count {
            let at = HEADER_SIZE + n * ENTRY_SIZE;
            let offset = u32_at(bytes, at + 4) as usize;
            let len = u32_at(bytes, at + 8) as usize;
            if Kind::from_u16(u16_at(bytes, at + 2)).is_none()
                || offset < dir_end
                || offset.checked_add(len).is_none_or(|end| end > bytes.len())
            {
                return Err(SectionError::BadEntry(n as u32));
            }
        }
        Ok(section)
    }

    /// Tables in the directory.
    pub fn len(&self) -> usize {
        self.count
    }

    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// The section's bytes.
    pub fn bytes(&self) -> &'a [u8] {
        self.bytes
    }

    /// Table `id`'s element kind and bytes; `None` when the section lacks
    /// it (the stub reads it as zeros).
    pub fn get(&self, id: TableId) -> Option<(Kind, &'a [u8])> {
        (0..self.count).find_map(|n| {
            let at = HEADER_SIZE + n * ENTRY_SIZE;
            (u16_at(self.bytes, at) == id as u16).then(|| {
                let kind = Kind::from_u16(u16_at(self.bytes, at + 2)).expect("checked by parse");
                let offset = u32_at(self.bytes, at + 4) as usize;
                let len = u32_at(self.bytes, at + 8) as usize;
                (kind, &self.bytes[offset..offset + len])
            })
        })
    }
}
