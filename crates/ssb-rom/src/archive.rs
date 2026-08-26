//! The `relocData` archive: Smash 64's asset filesystem.
//!
//! A ~9.5 MB region near the end of the ROM holding 2132 numbered files —
//! sprites, animations, hitbox data, stage models, texture atlases, fighter
//! attribute tables. Every asset the game loads at runtime comes from here.
//!
//! Layout, from `lbRelocInitSetup` / `lbRelocLoadAndRelocFile`
//! (`src/lb/lbreloc.c`):
//!
//! ```text
//! table_lo ──► [TableEntry; file_count + 1]   (12 bytes each; last is a sentinel)
//! table_hi ──► file 0 data ─ file 0 extern-id list ─ file 1 data ─ ...
//! ```
//!
//! `table_hi = table_lo + (file_count + 1) * 12`, and every entry's
//! `data_offset` is relative to `table_hi`.
//!
//! Loading a file is three steps:
//!
//! 1. Copy `compressed_size` words from ROM, VPK0-decoding them if flagged.
//! 2. Walk the **intern** relocation chain, rewriting each slot from a
//!    `{next, word_index}` pair into a pointer to `base + word_index * 4`.
//! 3. Walk the **extern** chain the same way, except each slot points into a
//!    *different* file. The target file IDs are a `u16` array sitting in ROM
//!    immediately after this file's data, consumed in chain order.
//!
//! Both chains are singly linked *through the slots being patched*: a slot
//! holds the word index of the next slot to patch, terminated by `0xFFFF`.
//! Patching therefore destroys the chain, which is why we read the link before
//! writing.

use alloc::collections::BTreeMap;
use alloc::vec::Vec;

use crate::rom::{self, Region};
use crate::{vpk0, Error, Result};

/// Terminator for both relocation chains.
const CHAIN_END: u16 = 0xFFFF;

/// Size of one `LBTableEntry` on ROM.
pub const TABLE_ENTRY_SIZE: usize = 12;

/// One entry of the archive's file table (`LBTableEntry`).
///
/// Note that all four size/offset fields are counted in **32-bit words**, not
/// bytes, and that `compressed_size` is the on-ROM footprint even for
/// uncompressed files.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TableEntry {
    /// Whether the payload is a VPK0 stream.
    pub compressed: bool,
    /// Byte offset of the payload, relative to `table_hi`.
    pub data_offset: u32,
    /// Word index of the first intern relocation slot, or `0xFFFF`.
    pub reloc_intern: u16,
    /// On-ROM payload size, in words.
    pub compressed_words: u16,
    /// Word index of the first extern relocation slot, or `0xFFFF`.
    pub reloc_extern: u16,
    /// Decompressed payload size, in words.
    pub decompressed_words: u16,
}

impl TableEntry {
    fn parse(raw: &[u8]) -> Self {
        let w0 = u32::from_be_bytes([raw[0], raw[1], raw[2], raw[3]]);
        TableEntry {
            compressed: (w0 >> 31) != 0,
            data_offset: w0 & 0x7FFF_FFFF,
            reloc_intern: u16::from_be_bytes([raw[4], raw[5]]),
            compressed_words: u16::from_be_bytes([raw[6], raw[7]]),
            reloc_extern: u16::from_be_bytes([raw[8], raw[9]]),
            decompressed_words: u16::from_be_bytes([raw[10], raw[11]]),
        }
    }

    /// Payload size in bytes once decompressed.
    pub fn size(&self) -> usize {
        self.decompressed_words as usize * 4
    }

    /// On-ROM payload size in bytes.
    pub fn rom_size(&self) -> usize {
        self.compressed_words as usize * 4
    }
}

/// A file's payload plus the relocations that still need resolving.
#[derive(Debug, Clone)]
pub struct File {
    pub id: u32,
    /// Decompressed bytes, with intern relocations already applied.
    pub data: Vec<u8>,
    /// Extern relocations: `(byte offset of the slot, target file, target byte
    /// offset within that file)`.
    ///
    /// These are left unresolved because the final address depends on where
    /// the consumer decides to place each file. `romtool` records them in the
    /// manifest; the runtime applies them when it lays files out in PSP RAM.
    pub extern_relocs: Vec<ExternReloc>,
    /// Intern relocation slots, as `(slot byte offset, target byte offset)`.
    /// Retained so a consumer can re-apply them after relocating the buffer.
    pub intern_relocs: Vec<InternReloc>,
}

/// A pointer slot that targets another file in the archive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExternReloc {
    /// Byte offset of the pointer slot within this file.
    pub at: u32,
    /// File the pointer targets.
    pub target_file: u16,
    /// Byte offset within the target file.
    pub target_offset: u32,
}

/// A pointer slot that targets this same file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InternReloc {
    /// Byte offset of the pointer slot within this file.
    pub at: u32,
    /// Byte offset the pointer targets, within this same file.
    pub target: u32,
}

/// Reader over the `relocData` archive of a validated ROM image.
pub struct Archive<'a> {
    rom: &'a [u8],
    table_lo: usize,
    table_hi: usize,
    entries: Vec<TableEntry>,
}

impl<'a> Archive<'a> {
    /// Parses the archive's file table.
    pub fn open(rom_data: &'a [u8], region: Region) -> Result<Self> {
        let table_lo = region.reloc_table_offset();
        let count = region.reloc_file_count() as usize;
        // The table carries one extra sentinel entry; the original uses it to
        // find where the last file's extern-id list ends.
        let table_hi = table_lo + (count + 1) * TABLE_ENTRY_SIZE;

        let raw = rom::slice(rom_data, table_lo, (count + 1) * TABLE_ENTRY_SIZE)?;
        let entries = raw
            .chunks_exact(TABLE_ENTRY_SIZE)
            .map(TableEntry::parse)
            .collect();

        Ok(Archive {
            rom: rom_data,
            table_lo,
            table_hi,
            entries,
        })
    }

    /// Number of real files (excluding the trailing sentinel).
    pub fn len(&self) -> usize {
        self.entries.len() - 1
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// ROM offset of the file table.
    pub fn table_offset(&self) -> usize {
        self.table_lo
    }

    /// ROM offset the entries' `data_offset` fields are relative to.
    pub fn data_base(&self) -> usize {
        self.table_hi
    }

    /// The table entry for `id`, including the sentinel at `len()`.
    pub fn entry(&self, id: u32) -> Option<&TableEntry> {
        self.entries.get(id as usize)
    }

    pub fn entries(&self) -> &[TableEntry] {
        &self.entries[..self.len()]
    }

    /// Loads and relocates one file.
    pub fn load(&self, id: u32) -> Result<File> {
        let entry = *self
            .entry(id)
            .filter(|_| (id as usize) < self.len())
            .ok_or(Error::OutOfBounds {
                offset: id as usize,
                len: 1,
            })?;

        let data_at = self.table_hi + entry.data_offset as usize;

        let mut data = if entry.compressed {
            let stream = self.rom.get(data_at..).ok_or(Error::OutOfBounds {
                offset: data_at,
                len: 0,
            })?;
            vpk0::decompress(stream)?
        } else {
            rom::slice(self.rom, data_at, entry.rom_size())?.to_vec()
        };

        // An uncompressed file's on-ROM size is already its full size; a
        // compressed one should decode to exactly `decompressed_words`.
        data.resize(entry.size(), 0);

        let intern_relocs = Self::walk_intern(&mut data, id, entry.reloc_intern)?;

        // Extern target IDs live in ROM right after the payload.
        let extern_ids_at = data_at + entry.rom_size();
        let extern_relocs = self.walk_extern(&mut data, id, entry.reloc_extern, extern_ids_at)?;

        Ok(File {
            id,
            data,
            extern_relocs,
            intern_relocs,
        })
    }

    /// Applies the intern chain in place, returning the slots it touched.
    fn walk_intern(data: &mut [u8], id: u32, head: u16) -> Result<Vec<InternReloc>> {
        let mut out = Vec::new();
        let mut cursor = head;
        while cursor != CHAIN_END {
            let at = cursor as usize * 4;
            let slot = data.get(at..at + 4).ok_or(Error::BadRelocation {
                file: id,
                offset: at,
            })?;
            // The slot currently holds `{ u16 next, u16 target_word }`.
            let next = u16::from_be_bytes([slot[0], slot[1]]);
            let target_word = u16::from_be_bytes([slot[2], slot[3]]);
            let target = target_word as u32 * 4;

            // Rewrite the slot as a file-relative byte offset. The original
            // writes an absolute RAM pointer here; we keep it relative so the
            // extracted asset is position-independent, and let whoever loads it
            // rebase using `intern_relocs`.
            data[at..at + 4].copy_from_slice(&target.to_be_bytes());

            out.push(InternReloc {
                at: at as u32,
                target,
            });
            cursor = next;
        }
        Ok(out)
    }

    /// Walks the extern chain, pairing each slot with a target file ID read
    /// from the ROM-side ID list.
    fn walk_extern(
        &self,
        data: &mut [u8],
        id: u32,
        head: u16,
        mut ids_at: usize,
    ) -> Result<Vec<ExternReloc>> {
        let mut out = Vec::new();
        let mut cursor = head;
        while cursor != CHAIN_END {
            let at = cursor as usize * 4;
            let slot = data.get(at..at + 4).ok_or(Error::BadRelocation {
                file: id,
                offset: at,
            })?;
            let next = u16::from_be_bytes([slot[0], slot[1]]);
            let target_word = u16::from_be_bytes([slot[2], slot[3]]);

            let target_file = rom::read_u16(self.rom, ids_at)?;
            ids_at += 2;

            // Zero the slot: its value is meaningless until the target file's
            // load address is known.
            data[at..at + 4].copy_from_slice(&[0; 4]);

            out.push(ExternReloc {
                at: at as u32,
                target_file,
                target_offset: target_word as u32 * 4,
            });
            cursor = next;
        }
        Ok(out)
    }

    /// Cross-checks a file's extern chain length against ROM geometry.
    ///
    /// This is the archive's strongest self-consistency check, and the reason
    /// it works is worth spelling out.
    ///
    /// The number of extern pointers can be derived two *independent* ways:
    ///
    /// 1. By walking the linked chain embedded in the **decompressed payload**.
    /// 2. By measuring the gap in **ROM** between the end of this file's data
    ///    and the start of the next file's, which is exactly the `u16` target-ID
    ///    array (`lbRelocGetExternBytesNum` bounds its scan this way).
    ///
    /// Route 1 depends on VPK0 having decoded every byte correctly; route 2
    /// does not depend on decompression at all. If they agree for a compressed
    /// file, the decompressor reproduced the payload exactly — a single wrong
    /// byte would derail the chain.
    ///
    /// Returns `(chain_length, id_list_length)`.
    pub fn verify_extern_chain(&self, id: u32) -> Result<(usize, usize)> {
        let entry = *self.entry(id).ok_or(Error::OutOfBounds {
            offset: id as usize,
            len: 1,
        })?;
        let next = *self.entry(id + 1).ok_or(Error::OutOfBounds {
            offset: id as usize + 1,
            len: 1,
        })?;

        let file = self.load(id)?;

        let data_end = entry.data_offset as usize + entry.rom_size();
        let id_list_bytes = (next.data_offset as usize).saturating_sub(data_end);

        Ok((file.extern_relocs.len(), id_list_bytes / 2))
    }

    /// Loads `id` and, transitively, every file it points at.
    ///
    /// This mirrors `lbRelocGetExternBytesNum`'s recursive walk: loading a
    /// stage or fighter pulls in its whole dependency closure.
    pub fn load_closure(&self, id: u32) -> Result<BTreeMap<u32, File>> {
        let mut loaded = BTreeMap::new();
        let mut queue = alloc::vec![id];

        while let Some(next) = queue.pop() {
            if loaded.contains_key(&next) {
                continue;
            }
            let file = self.load(next)?;
            for r in &file.extern_relocs {
                if !loaded.contains_key(&u32::from(r.target_file)) {
                    queue.push(u32::from(r.target_file));
                }
            }
            loaded.insert(next, file);
        }
        Ok(loaded)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_entry_bitfields() {
        // compressed=1, offset=0x57A0, intern=304, comp=3192, extern=0xFFFF, dec=8528
        let raw = [
            0x80, 0x00, 0x57, 0xA0, 0x01, 0x30, 0x0C, 0x78, 0xFF, 0xFF, 0x21, 0x50,
        ];
        let e = TableEntry::parse(&raw);
        assert!(e.compressed);
        assert_eq!(e.data_offset, 0x57A0);
        assert_eq!(e.reloc_intern, 304);
        assert_eq!(e.compressed_words, 3192);
        assert_eq!(e.reloc_extern, CHAIN_END);
        assert_eq!(e.decompressed_words, 8528);
        assert_eq!(e.size(), 8528 * 4);
    }

    #[test]
    fn intern_chain_rewrites_slots_and_terminates() {
        // Two chained slots: word 0 -> word 2, both targeting word 4.
        let mut data = alloc::vec![0u8; 24];
        data[0..4].copy_from_slice(&[0x00, 0x02, 0x00, 0x04]); // next=2, target=word 4
        data[8..12].copy_from_slice(&[0xFF, 0xFF, 0x00, 0x05]); // next=END, target=word 5

        let relocs = Archive::walk_intern(&mut data, 0, 0).unwrap();
        assert_eq!(
            relocs,
            [
                InternReloc { at: 0, target: 16 },
                InternReloc { at: 8, target: 20 },
            ]
        );
        assert_eq!(&data[0..4], &16u32.to_be_bytes());
        assert_eq!(&data[8..12], &20u32.to_be_bytes());
    }

    #[test]
    fn intern_chain_out_of_bounds_is_an_error() {
        let mut data = alloc::vec![0u8; 8];
        assert!(matches!(
            Archive::walk_intern(&mut data, 7, 100),
            Err(Error::BadRelocation { file: 7, .. })
        ));
    }
}
