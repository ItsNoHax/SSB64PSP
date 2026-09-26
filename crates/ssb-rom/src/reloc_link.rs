//! The extern-relocation loader (D-011, `docs/decisions/D-011.md`):
//! lay a scene's `relocData` file closure out in one block and resolve
//! every pointer slot, as `lbRelocLoadFilesExtern` does (`lb/lbreloc.c`).
//!
//! [`crate::archive::Archive::load`] leaves intern slots as file-relative
//! byte offsets and extern slots zeroed, because the addresses depend on
//! where each file lands. This module is the missing second half:
//!
//! 1. **Closure** — the caller loads it with
//!    [`crate::archive::Archive::load_closure`].
//! 2. **Layout** — [`layout`] assigns offsets in the source's order.
//!    `lbRelocLoadFilesExtern` places each requested file at the next
//!    16-byte-aligned heap position (`LBRELOC_CACHE_ALIGN`), and while it
//!    patches that file's extern chain it places every target it has not
//!    placed yet, depth first, in chain order. A file is recorded before its
//!    externs are walked, so cycles resolve to the one copy.
//! 3. **Patch** — [`link`] copies every file into one buffer and rewrites
//!    each intern slot to `base + file + target` and each extern slot to
//!    `base + target file + target offset`.
//!
//! The linked bytes keep the ROM's big-endian layout, pointer slots
//! included, so they are the same image the original builds in RDRAM at
//! `base`. Nothing in the PSP build consumes raw `relocData` yet (the pack
//! stores converted, position-independent assets); this gives a future
//! runtime loader and host tools the original's layout to compare against.

use alloc::collections::BTreeMap;
use alloc::vec::Vec;

use crate::archive::{Archive, File};

/// `LBRELOC_CACHE_ALIGN`: 16 bytes.
pub const ALIGN: u32 = 16;

const fn align(x: u32) -> u32 {
    (x + (ALIGN - 1)) & !(ALIGN - 1)
}

/// A link failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkError {
    /// A root or extern target is not in the supplied closure.
    MissingFile(u32),
    /// A relocation slot lies outside its file.
    SlotOutOfBounds { file: u32, at: u32 },
    /// The block would pass the 32-bit address space.
    TooLarge,
}

impl core::fmt::Display for LinkError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match *self {
            LinkError::MissingFile(id) => write!(f, "file {id} is not in the loaded closure"),
            LinkError::SlotOutOfBounds { file, at } => {
                write!(f, "relocation slot 0x{at:X} lies outside file {file}")
            }
            LinkError::TooLarge => write!(f, "the linked block passes 4 GiB"),
        }
    }
}

/// Where each file sits in the block, and the block's size.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Layout {
    /// File ID → byte offset from the block start.
    pub offsets: BTreeMap<u32, u32>,
    /// Placement order.
    pub order: Vec<u32>,
    /// `sLBRelocExternFileHeap - heap`: the bytes the block uses.
    pub size: u32,
}

/// Step 2: `lbRelocLoadFilesExtern`'s placement for `roots`, in order.
pub fn layout(files: &BTreeMap<u32, File>, roots: &[u32]) -> Result<Layout, LinkError> {
    layout_after(files, roots, &alloc::collections::BTreeSet::new())
}

/// [`layout`] when `loaded` files are already resident: the source's
/// `lbRelocFindStatusBufferFile` returns their existing address, so they
/// take no space and their externs are not walked again. Their slots are
/// not in [`Layout::offsets`]; [`link`] needs the full closure.
pub fn layout_after(
    files: &BTreeMap<u32, File>,
    roots: &[u32],
    loaded: &alloc::collections::BTreeSet<u32>,
) -> Result<Layout, LinkError> {
    let mut out = Layout::default();
    for &root in roots {
        place(files, root, loaded, &mut out)?;
    }
    Ok(out)
}

/// `lbRelocGetExternBufferFile` followed by the extern walk of
/// `lbRelocLoadAndRelocFile`. An explicit stack keeps the depth-first order
/// without recursion: each frame is a file and how far its extern chain has
/// been walked.
fn place(
    files: &BTreeMap<u32, File>,
    root: u32,
    loaded: &alloc::collections::BTreeSet<u32>,
    out: &mut Layout,
) -> Result<(), LinkError> {
    let mut stack: Vec<(u32, usize)> = Vec::new();
    let visit = |id: u32, out: &mut Layout, stack: &mut Vec<(u32, usize)>| {
        if out.offsets.contains_key(&id) || loaded.contains(&id) {
            return Ok(());
        }
        let file = files.get(&id).ok_or(LinkError::MissingFile(id))?;
        let at = align(out.size);
        let len = u32::try_from(file.data.len()).map_err(|_| LinkError::TooLarge)?;
        out.size = at.checked_add(len).ok_or(LinkError::TooLarge)?;
        out.offsets.insert(id, at);
        out.order.push(id);
        stack.push((id, 0));
        Ok(())
    };
    visit(root, out, &mut stack)?;
    while let Some(&mut (id, ref mut next)) = stack.last_mut() {
        let file = &files[&id];
        match file.extern_relocs.get(*next) {
            Some(r) => {
                *next += 1;
                visit(u32::from(r.target_file), out, &mut stack)?;
            }
            None => {
                stack.pop();
            }
        }
    }
    Ok(())
}

/// `lbRelocGetAllocSize(roots)` against an empty status buffer: the bytes
/// the source reserves before calling `lbRelocLoadFilesExtern`.
///
/// Computed only from the file table and the ROM's `u16` extern-ID arrays
/// (`lbRelocGetExternBytesNum`), never from decompressed chains, so it is an
/// independent check of [`layout`]. Each root adds `ALIGN16(total)` first;
/// each file not yet seen adds `ALIGN16(decompressed bytes)` plus its
/// targets, in ROM order.
pub fn alloc_size(archive: &Archive<'_>, roots: &[u32]) -> crate::Result<u32> {
    let mut seen = alloc::collections::BTreeSet::new();
    let mut total = 0u32;
    for &root in roots {
        total = align(total);
        let mut stack = alloc::vec![root];
        while let Some(id) = stack.pop() {
            if !seen.insert(id) {
                continue;
            }
            let entry = archive.entry(id).ok_or(crate::Error::OutOfBounds {
                offset: id as usize,
                len: 1,
            })?;
            total += align(entry.size() as u32);
            // Visiting order does not change a sum over unique files.
            stack.extend(archive.extern_ids(id)?.into_iter().map(u32::from));
        }
    }
    Ok(total)
}

/// The linked block and its layout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Linked {
    pub layout: Layout,
    /// `layout.size` bytes: every file at its offset, alignment gaps zeroed,
    /// every pointer slot resolved against `base`.
    pub bytes: Vec<u8>,
}

/// Steps 2 and 3: lays out `roots` and resolves every slot for a block that
/// will live at address `base`.
pub fn link(files: &BTreeMap<u32, File>, roots: &[u32], base: u32) -> Result<Linked, LinkError> {
    let layout = layout(files, roots)?;
    let mut bytes = alloc::vec![0u8; layout.size as usize];
    for (&id, &at) in &layout.offsets {
        let file = &files[&id];
        let at = at as usize;
        bytes[at..at + file.data.len()].copy_from_slice(&file.data);
    }
    let write = |bytes: &mut [u8], file: u32, slot: u32, value: u32| {
        let len = files[&file].data.len();
        if slot as usize + 4 > len {
            return Err(LinkError::SlotOutOfBounds { file, at: slot });
        }
        let at = (layout.offsets[&file] + slot) as usize;
        bytes[at..at + 4].copy_from_slice(&value.to_be_bytes());
        Ok(())
    };
    for (&id, &at) in &layout.offsets {
        let file = &files[&id];
        for r in &file.intern_relocs {
            write(
                &mut bytes,
                id,
                r.at,
                base.wrapping_add(at).wrapping_add(r.target),
            )?;
        }
        for r in &file.extern_relocs {
            let target = *layout
                .offsets
                .get(&u32::from(r.target_file))
                .ok_or(LinkError::MissingFile(u32::from(r.target_file)))?;
            write(
                &mut bytes,
                id,
                r.at,
                base.wrapping_add(target).wrapping_add(r.target_offset),
            )?;
        }
    }
    Ok(Linked { layout, bytes })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::archive::{ExternReloc, InternReloc};

    fn file(id: u32, words: usize, interns: &[(u32, u32)], externs: &[(u32, u16, u32)]) -> File {
        File {
            id,
            data: alloc::vec![0xEE; words * 4],
            intern_relocs: interns
                .iter()
                .map(|&(at, target)| InternReloc { at, target })
                .collect(),
            extern_relocs: externs
                .iter()
                .map(|&(at, target_file, target_offset)| ExternReloc {
                    at,
                    target_file,
                    target_offset,
                })
                .collect(),
        }
    }

    fn word(bytes: &[u8], at: u32) -> u32 {
        let at = at as usize;
        u32::from_be_bytes(bytes[at..at + 4].try_into().unwrap())
    }

    /// File 10 points at 20 then 30; 20 points at 30 and back at 10. The
    /// depth-first walk places 10, 20, 30, each 16-byte aligned.
    fn closure() -> BTreeMap<u32, File> {
        let mut files = BTreeMap::new();
        files.insert(10, file(10, 5, &[(0, 8)], &[(4, 20, 0), (12, 30, 4)]));
        files.insert(20, file(20, 3, &[], &[(0, 30, 8), (4, 10, 16)]));
        files.insert(30, file(30, 4, &[(8, 0)], &[]));
        files
    }

    #[test]
    fn layout_follows_the_source_placement_order() {
        let l = layout(&closure(), &[10]).unwrap();
        assert_eq!(l.order, [10, 20, 30]);
        // 20 bytes -> 32; 12 bytes -> 48; 16 bytes.
        assert_eq!(l.offsets[&10], 0);
        assert_eq!(l.offsets[&20], 32);
        assert_eq!(l.offsets[&30], 48);
        assert_eq!(l.size, 64);
    }

    #[test]
    fn link_resolves_intern_and_extern_slots_against_the_base() {
        let base = 0x8040_0000;
        let linked = link(&closure(), &[10], base).unwrap();
        let b = &linked.bytes;
        assert_eq!(word(b, 0), base + 8, "intern: own file + target");
        assert_eq!(word(b, 4), base + 32, "extern: file 20 at 32");
        assert_eq!(word(b, 12), base + 48 + 4, "extern: file 30 at 48, +4");
        assert_eq!(word(b, 32), base + 48 + 8);
        assert_eq!(word(b, 36), base + 16, "a cycle resolves to the one copy");
        assert_eq!(word(b, 48 + 8), base + 48);
        assert_eq!(&b[20..32], &[0; 12], "alignment gap stays zero");
        assert_eq!(word(b, 16), 0xEEEE_EEEE, "non-pointer data is copied");
    }

    /// A resident target (the status buffer's) takes no space, so the next
    /// file moves into its place; its own externs are not walked again.
    #[test]
    fn resident_files_are_skipped() {
        let loaded = [20].into_iter().collect();
        let l = layout_after(&closure(), &[10], &loaded).unwrap();
        assert_eq!(l.order, [10, 30]);
        assert_eq!(l.offsets[&30], 32);
        assert_eq!(l.size, 48);
    }

    #[test]
    fn a_later_root_reuses_files_already_placed() {
        let l = layout(&closure(), &[30, 10]).unwrap();
        assert_eq!(l.order, [30, 10, 20]);
    }

    #[test]
    fn a_missing_target_or_bad_slot_is_an_error() {
        let mut files = closure();
        files.remove(&30);
        assert_eq!(link(&files, &[10], 0), Err(LinkError::MissingFile(30)));
        let mut files = closure();
        files.get_mut(&30).unwrap().intern_relocs[0].at = 16;
        assert_eq!(
            link(&files, &[10], 0),
            Err(LinkError::SlotOutOfBounds { file: 30, at: 16 })
        );
    }
}
