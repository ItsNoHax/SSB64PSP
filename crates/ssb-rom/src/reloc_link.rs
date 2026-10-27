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
//!
//! [`place_into`] is the same placement over any [`FileSource`] (the ROM
//! closure through [`Files`], or the pack's file table) into a reusable,
//! index-based [`Placement`]: [`Order::Original`] for the N64 placement,
//! [`Order::Ascending`] for the pack's read runs (`scene_files`). The pack
//! needs no slot patching: its scripts never reach an extern slot
//! (`reloc_link_pack_tests.rs`), so only placement applies to it.

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
    link_into(&mut bytes, files, &layout, base as usize, Endian::Big)?;
    Ok(Linked { layout, bytes })
}

/// The byte order [`link_into`] writes slot words in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Endian {
    /// The ROM's order: the image the original builds in RDRAM (RE-341).
    Big,
    /// The running CPU's order (little-endian on the PSP).
    Native,
}

/// Step 3 in place: `dst` already holds every file of `layout` at its
/// offset (read there, not copied); rewrites each intern slot to
/// `base + file + target` and each extern slot to
/// `base + target file + target offset`. `base` is the address of
/// `dst[0]`; slot words are 32 bits (`TooLarge` past 4 GiB).
pub fn link_into(
    dst: &mut [u8],
    files: &BTreeMap<u32, File>,
    layout: &Layout,
    base: usize,
    endian: Endian,
) -> Result<(), LinkError> {
    let base = u32::try_from(base).map_err(|_| LinkError::TooLarge)?;
    if dst.len() < layout.size as usize {
        return Err(LinkError::TooLarge);
    }
    let write = |dst: &mut [u8], file: u32, slot: u32, value: u32| {
        let len = files
            .get(&file)
            .ok_or(LinkError::MissingFile(file))?
            .data
            .len();
        if slot as usize + 4 > len {
            return Err(LinkError::SlotOutOfBounds { file, at: slot });
        }
        let at = (layout.offsets[&file] + slot) as usize;
        let bytes = match endian {
            Endian::Big => value.to_be_bytes(),
            Endian::Native => value.to_ne_bytes(),
        };
        dst[at..at + 4].copy_from_slice(&bytes);
        Ok(())
    };
    for (&id, &at) in &layout.offsets {
        let file = files.get(&id).ok_or(LinkError::MissingFile(id))?;
        for r in &file.intern_relocs {
            write(dst, id, r.at, base.wrapping_add(at).wrapping_add(r.target))?;
        }
        for r in &file.extern_relocs {
            let target = *layout
                .offsets
                .get(&u32::from(r.target_file))
                .ok_or(LinkError::MissingFile(u32::from(r.target_file)))?;
            write(
                dst,
                id,
                r.at,
                base.wrapping_add(target).wrapping_add(r.target_offset),
            )?;
        }
    }
    Ok(())
}

/// A file table [`place_into`] lays out: files by dense index, each with a
/// length and an ordered extern-ID list (duplicates kept: one per slot,
/// the order `lbRelocLoadAndRelocFile` walks them in).
///
/// Implemented by [`Files`] (ROM `relocData`, decompressed sizes) and by
/// [`crate::pack::Pack`] (the pack's file table: converted bytes,
/// `FileDesc` deps).
pub trait FileSource {
    /// Files in the table; indices are `0..count`.
    fn count(&self) -> u32;
    /// `id`'s index, if the table has it.
    fn index(&self, id: u32) -> Option<u32>;
    /// The file ID at `index`.
    fn id(&self, index: u32) -> u32;
    /// The bytes file `index` occupies.
    fn len(&self, index: u32) -> u32;
    /// Entries in file `index`'s extern-ID list.
    fn extern_count(&self, index: u32) -> u32;
    /// Entry `k` of file `index`'s extern-ID list.
    fn extern_id(&self, index: u32, k: u32) -> u32;
    /// Appends file `index`'s whole extern-ID list to `out`: what
    /// [`closure`] walks, in one table read where the source stores the
    /// list together.
    fn extend_externs(&self, index: u32, out: &mut Vec<u32>) {
        out.extend((0..self.extern_count(index)).map(|k| self.extern_id(index, k)));
    }
    /// Where file `index`'s bytes start in the source's own storage, when
    /// it has one (the pack's blob offset): files whose bytes are adjacent
    /// there can share one read.
    fn start(&self, _index: u32) -> Option<u32> {
        None
    }
    /// Whether an ID the table lacks is a file with no bytes and no externs
    /// (the pack omits those, RE-475) rather than a [`LinkError::MissingFile`].
    fn absent_is_empty(&self) -> bool {
        false
    }
}

/// [`FileSource`] over a loaded closure ([`Archive::load_closure`]).
pub struct Files<'a> {
    ids: Vec<u32>,
    files: Vec<&'a File>,
}

impl<'a> Files<'a> {
    pub fn new(files: &'a BTreeMap<u32, File>) -> Self {
        Self {
            ids: files.keys().copied().collect(),
            files: files.values().collect(),
        }
    }
}

impl FileSource for Files<'_> {
    fn count(&self) -> u32 {
        self.ids.len() as u32
    }
    fn index(&self, id: u32) -> Option<u32> {
        self.ids.binary_search(&id).ok().map(|i| i as u32)
    }
    fn id(&self, index: u32) -> u32 {
        self.ids[index as usize]
    }
    fn len(&self, index: u32) -> u32 {
        self.files[index as usize].data.len() as u32
    }
    fn extern_count(&self, index: u32) -> u32 {
        self.files[index as usize].extern_relocs.len() as u32
    }
    fn extern_id(&self, index: u32, k: u32) -> u32 {
        u32::from(self.files[index as usize].extern_relocs[k as usize].target_file)
    }
}

impl FileSource for crate::pack::Pack<'_> {
    fn count(&self) -> u32 {
        self.file_count()
    }
    fn index(&self, id: u32) -> Option<u32> {
        self.file_index(id)
    }
    fn id(&self, index: u32) -> u32 {
        self.file_at(index).map_or(u32::MAX, |d| d.file)
    }
    fn len(&self, index: u32) -> u32 {
        self.file_at(index).map_or(0, |d| d.len)
    }
    fn extern_count(&self, index: u32) -> u32 {
        self.file_at(index).map_or(0, |d| d.dep_count)
    }
    fn extern_id(&self, index: u32, k: u32) -> u32 {
        self.file_at(index)
            .and_then(|d| self.file_dep(&d, k))
            .unwrap_or(u32::MAX)
    }
    fn extend_externs(&self, index: u32, out: &mut Vec<u32>) {
        if let Some(d) = self.file_at(index) {
            out.extend((0..d.dep_count).map(|k| self.file_dep(&d, k).unwrap_or(u32::MAX)));
        }
    }
    fn start(&self, index: u32) -> Option<u32> {
        self.file_at(index).map(|d| d.blob_start)
    }
    fn absent_is_empty(&self) -> bool {
        true
    }
}

/// `PSP_CACHE_LINE`: every read run starts on a line and is padded to whole
/// lines, so no line holds bytes of two runs (RE-360, RE-476).
pub const LINE: u32 = 64;

/// The order [`place_into`] lays files out in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Order {
    /// `lbRelocLoadFilesExtern`'s: depth first along each extern chain,
    /// each file at the next 16-byte boundary (RE-341). A held file's
    /// externs are not walked (`lbRelocFindStatusBufferFile`). No runs.
    Original,
    /// File-index order, grouped into read runs: files adjacent in the
    /// source (`start`, 16-byte spacing, as `scene_files::load_missing`)
    /// share a run of at most `max_run` bytes; each run starts on a
    /// [`LINE`] and the next starts past its padded end; files keep the
    /// source's spacing inside a run, so a run is one read. The closure is
    /// walked through held files (their externs may have been dropped).
    Ascending { max_run: u32 },
}

/// Files `first..=last` (indices) read together: source bytes
/// `start..start + len` land at block offset `offset`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Run {
    pub first: u32,
    pub last: u32,
    pub start: u32,
    pub len: u32,
    pub offset: u32,
}

/// [`Layout`] by file index, for a loader that must not build maps per
/// scene: reuse one value across scenes and its vectors keep their
/// capacity.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Placement {
    /// File index → byte offset in the block, or [`Placement::NONE`]
    /// (outside the closure, held, or with no bytes).
    pub offset: Vec<u32>,
    /// File index → in the closure (held files included).
    pub want: Vec<bool>,
    /// Indices placed, in placement order (files with no bytes included:
    /// they take no space).
    pub order: Vec<u32>,
    /// [`Order::Ascending`]'s read runs, ascending.
    pub runs: Vec<Run>,
    /// Bytes the block needs.
    pub size: u32,
    stack: Vec<(u32, u32)>,
    ids: Vec<u32>,
}

impl Placement {
    pub const NONE: u32 = u32::MAX;

    fn reset(&mut self, count: u32) {
        self.offset.clear();
        self.offset.resize(count as usize, Self::NONE);
        self.want.clear();
        self.want.resize(count as usize, false);
        self.order.clear();
        self.runs.clear();
        self.stack.clear();
        self.size = 0;
    }

    /// [`Layout`] form (file IDs), for comparison with [`layout`].
    pub fn to_layout(&self, src: &impl FileSource) -> Layout {
        let mut out = Layout {
            size: self.size,
            ..Layout::default()
        };
        for &i in &self.order {
            out.order.push(src.id(i));
            out.offsets.insert(src.id(i), self.offset[i as usize]);
        }
        out
    }
}

/// `roots` and every file they reach through extern IDs, by index, into
/// `want` (the `lbRelocGetAllocSize` walk). An ID the table lacks is an
/// error unless the source says such files are empty.
pub fn closure(
    src: &impl FileSource,
    roots: &[u32],
    want: &mut Vec<bool>,
    stack: &mut Vec<u32>,
) -> Result<(), LinkError> {
    want.clear();
    want.resize(src.count() as usize, false);
    stack.clear();
    stack.extend_from_slice(roots);
    while let Some(id) = stack.pop() {
        let Some(i) = src.index(id) else {
            if src.absent_is_empty() {
                continue;
            }
            return Err(LinkError::MissingFile(id));
        };
        if core::mem::replace(&mut want[i as usize], true) {
            continue;
        }
        src.extend_externs(i, stack);
    }
    Ok(())
}

/// [`layout_after`] over any [`FileSource`], in `order`, into `out`.
/// `held(index)`: the file is already resident, so it takes no space.
pub fn place_into(
    src: &impl FileSource,
    roots: &[u32],
    held: impl Fn(u32) -> bool,
    order: Order,
    out: &mut Placement,
) -> Result<(), LinkError> {
    out.reset(src.count());
    match order {
        Order::Original => place_original(src, roots, &held, out),
        Order::Ascending { max_run } => place_ascending(src, roots, &held, max_run, out),
    }
}

fn place_original(
    src: &impl FileSource,
    roots: &[u32],
    held: &impl Fn(u32) -> bool,
    out: &mut Placement,
) -> Result<(), LinkError> {
    let mut stack = core::mem::take(&mut out.stack);
    let visit = |id: u32, out: &mut Placement, stack: &mut Vec<(u32, u32)>| {
        let Some(i) = src.index(id) else {
            return if src.absent_is_empty() {
                Ok(())
            } else {
                Err(LinkError::MissingFile(id))
            };
        };
        if out.want[i as usize] {
            return Ok(());
        }
        out.want[i as usize] = true;
        if held(i) {
            return Ok(());
        }
        let at = align(out.size);
        out.size = at.checked_add(src.len(i)).ok_or(LinkError::TooLarge)?;
        out.offset[i as usize] = at;
        out.order.push(i);
        stack.push((i, 0));
        Ok(())
    };
    let mut result = Ok(());
    'roots: for &root in roots {
        if let Err(e) = visit(root, out, &mut stack) {
            result = Err(e);
            break;
        }
        while let Some(&mut (i, ref mut next)) = stack.last_mut() {
            if *next < src.extern_count(i) {
                let id = src.extern_id(i, *next);
                *next += 1;
                if let Err(e) = visit(id, out, &mut stack) {
                    result = Err(e);
                    break 'roots;
                }
            } else {
                stack.pop();
            }
        }
    }
    stack.clear();
    out.stack = stack;
    result
}

fn place_ascending(
    src: &impl FileSource,
    roots: &[u32],
    held: &impl Fn(u32) -> bool,
    max_run: u32,
    out: &mut Placement,
) -> Result<(), LinkError> {
    let mut want = core::mem::take(&mut out.want);
    let mut ids = core::mem::take(&mut out.ids);
    let walked = closure(src, roots, &mut want, &mut ids);
    out.want = want;
    out.ids = ids;
    walked?;
    place_runs(src, held, max_run, out)
}

/// [`Order::Ascending`]'s read runs over `out.want` as the caller set it:
/// a [`closure`] (what [`place_into`] does), or any set of files, such as
/// one file a scene needs at once or the rest of a cancelled read.
/// `held(index)`: the file is already resident, so it takes no space.
pub fn place_runs(
    src: &impl FileSource,
    held: impl Fn(u32) -> bool,
    max_run: u32,
    out: &mut Placement,
) -> Result<(), LinkError> {
    let count = src.count() as usize;
    out.want.resize(count, false);
    out.offset.clear();
    out.offset.resize(count, Placement::NONE);
    out.order.clear();
    out.runs.clear();
    out.size = 0;
    let line = |x: u32| x.checked_next_multiple_of(LINE).ok_or(LinkError::TooLarge);
    let mut run: Option<Run> = None;
    for i in 0..src.count() {
        if !out.want[i as usize] || held(i) {
            continue;
        }
        let len = src.len(i);
        if len == 0 {
            out.order.push(i);
            continue;
        }
        let start = src.start(i);
        let joins = match (&run, start) {
            (Some(r), Some(s)) => {
                s == (r.start + r.len).next_multiple_of(ALIGN)
                    && u64::from(s) + u64::from(len) - u64::from(r.start) <= u64::from(max_run)
            }
            _ => false,
        };
        if joins {
            let r = run.as_mut().unwrap();
            let s = start.unwrap();
            r.len = s + len - r.start;
            r.last = i;
            out.offset[i as usize] = r.offset + (s - r.start);
        } else {
            if let Some(r) = run.take() {
                out.size = line(r.offset + r.len)?;
                out.runs.push(r);
            }
            let offset = line(out.size)?;
            run = Some(Run {
                first: i,
                last: i,
                start: start.unwrap_or(0),
                len,
                offset,
            });
            out.offset[i as usize] = offset;
        }
        out.order.push(i);
    }
    if let Some(r) = run {
        out.size = line(r.offset + r.len)?;
        out.runs.push(r);
    }
    Ok(())
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

    #[test]
    fn original_order_over_a_file_source_equals_layout() {
        let files = closure();
        let src = Files::new(&files);
        let mut p = Placement::default();
        for roots in [&[10][..], &[30, 10], &[20]] {
            place_into(&src, roots, |_| false, Order::Original, &mut p).unwrap();
            assert_eq!(p.to_layout(&src), layout(&files, roots).unwrap());
        }
    }

    #[test]
    fn layout_after_skips_held() {
        let files = closure();
        let src = Files::new(&files);
        let held = src.index(20).unwrap();
        let mut p = Placement::default();
        place_into(&src, &[10], |i| i == held, Order::Original, &mut p).unwrap();
        let old = layout_after(&files, &[10], &[20].into_iter().collect()).unwrap();
        assert_eq!(p.to_layout(&src), old);
        assert_eq!(old.order, [10, 30], "20 is held; 30 is reached from 10");
        assert_eq!(p.offset[held as usize], Placement::NONE);
        assert!(p.want[held as usize]);
        // Ascending walks through held files: 30 is wanted through 20 too.
        place_into(
            &src,
            &[20],
            |i| i == held,
            Order::Ascending { max_run: 1 << 20 },
            &mut p,
        )
        .unwrap();
        assert_eq!(p.to_layout(&src).order, [10, 30]);
    }

    #[test]
    fn native_endian_slot_roundtrip() {
        let files = closure();
        let big = link(&files, &[10], 0x0880_0000).unwrap();
        let mut native = alloc::vec![0u8; big.layout.size as usize];
        for (&id, &at) in &big.layout.offsets {
            native[at as usize..][..files[&id].data.len()].copy_from_slice(&files[&id].data);
        }
        link_into(
            &mut native,
            &files,
            &big.layout,
            0x0880_0000,
            Endian::Native,
        )
        .unwrap();
        let slots: Vec<u32> = big
            .layout
            .offsets
            .iter()
            .flat_map(|(id, &at)| {
                let f = &files[id];
                f.intern_relocs
                    .iter()
                    .map(|r| r.at)
                    .chain(f.extern_relocs.iter().map(|r| r.at))
                    .map(move |slot| at + slot)
            })
            .collect();
        assert_eq!(slots.len(), 6);
        for at in slots {
            let n = u32::from_ne_bytes(native[at as usize..][..4].try_into().unwrap());
            assert_eq!(n, word(&big.bytes, at));
        }
        assert_eq!(
            link_into(&mut native, &files, &big.layout, 1 << 40, Endian::Native),
            Err(LinkError::TooLarge)
        );
    }

    /// Files with a storage start: 1 and 2 adjacent (16-byte spacing),
    /// 3 after a gap, 4 adjacent to 3, 5 empty.
    struct Stored(&'static [(u32, u32, u32, &'static [u32])]);

    impl FileSource for Stored {
        fn count(&self) -> u32 {
            self.0.len() as u32
        }
        fn index(&self, id: u32) -> Option<u32> {
            self.0.iter().position(|f| f.0 == id).map(|i| i as u32)
        }
        fn id(&self, index: u32) -> u32 {
            self.0[index as usize].0
        }
        fn len(&self, index: u32) -> u32 {
            self.0[index as usize].2
        }
        fn extern_count(&self, index: u32) -> u32 {
            self.0[index as usize].3.len() as u32
        }
        fn extern_id(&self, index: u32, k: u32) -> u32 {
            self.0[index as usize].3[k as usize]
        }
        fn start(&self, index: u32) -> Option<u32> {
            Some(self.0[index as usize].1)
        }
        fn absent_is_empty(&self) -> bool {
            true
        }
    }

    const STORED: Stored = Stored(&[
        (1, 0x08, 0x24, &[3, 2, 99]),
        (2, 0x30, 0x10, &[]),
        (3, 0x100, 0x44, &[4, 5]),
        (4, 0x150, 0x80, &[]),
        (5, 0x1D0, 0, &[1]),
    ]);

    #[test]
    fn ascending_runs_are_line_aligned_and_keep_source_spacing() {
        let mut p = Placement::default();
        place_into(
            &STORED,
            &[1],
            |_| false,
            Order::Ascending { max_run: 0x1000 },
            &mut p,
        )
        .unwrap();
        assert_eq!(p.order, [0, 1, 2, 3, 4]);
        assert_eq!(
            p.runs,
            [
                Run {
                    first: 0,
                    last: 1,
                    start: 0x08,
                    len: 0x38,
                    offset: 0
                },
                Run {
                    first: 2,
                    last: 3,
                    start: 0x100,
                    len: 0xD0,
                    offset: 0x40
                },
            ]
        );
        assert_eq!(&p.offset[..4], [0, 0x28, 0x40, 0x90]);
        assert_eq!(p.size, 0x140, "the last run is padded to a line");
        // A run limit splits 3 and 4; 2 held leaves 1 alone.
        place_into(
            &STORED,
            &[1],
            |i| i == 1,
            Order::Ascending { max_run: 0x60 },
            &mut p,
        )
        .unwrap();
        assert_eq!(
            p.runs
                .iter()
                .map(|r| (r.first, r.offset))
                .collect::<Vec<_>>(),
            [(0, 0), (2, 0x40), (3, 0xC0)]
        );
        assert_eq!(p.offset[1], Placement::NONE);
        assert_eq!(p.size, 0x140);
    }
}

#[cfg(test)]
#[path = "reloc_link_pack_tests.rs"]
mod pack_tests;

#[cfg(test)]
#[path = "reloc_link_sim_tests.rs"]
pub(crate) mod sim_tests;
