//! Discovery and inventory of display lists inside relocated archive files.
//!
//! Plan §8 says to measure what Smash actually renders rather than
//! speculatively supporting every N64 feature. This module does the measuring.
//!
//! ## Finding display lists without guessing
//!
//! A naive scan for `G_ENDDL`-terminated runs produces false positives all over
//! non-display-list data, because zero padding decodes as `G_NOOP` and short
//! integer data often decodes as valid-looking commands.
//!
//! Relocation targets look like an attractive candidate set — every one is a
//! pointer the game itself follows — but they are the *wrong* set: most reloc
//! targets are the vertex-array pointers carried by `G_VTX`, not list starts.
//! Decoding a "display list" at a vertex array yields garbage that passes
//! structural validation. That approach found 762 lists of which 666 failed to
//! convert.
//!
//! The discriminator that works is **semantic**: a real display list fills its
//! own vertex cache before drawing triangles. [`find_root_display_lists`]
//! therefore scans every aligned offset and keeps only lists that convert
//! cleanly through [`crate::mesh::convert`]. Two hard RSP limits do most of the
//! cheap filtering first: the vertex cache holds 32 entries, and triangle
//! indices must fall within it.

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::vec::Vec;

use crate::archive::File;
use crate::dl::{self, Cmd};

/// Minimum commands before a candidate is considered a real display list.
/// Two is enough for a degenerate `Call` + `End`, which does occur.
const MIN_COMMANDS: usize = 2;

/// Size of the F3DEX2 vertex cache, in vertices.
///
/// This is a hard RSP limit, which makes it an excellent validity test: a
/// `G_VTX` claiming to load more than this, or a triangle indexing past it,
/// cannot be a real command and is therefore random data that happens to
/// decode.
pub const VTX_CACHE_SIZE: u8 = 32;

/// A display list found inside a file.
#[derive(Debug, Clone)]
pub struct FoundDl {
    /// Byte offset within the file.
    pub offset: u32,
    pub commands: Vec<Cmd>,
}

impl FoundDl {
    /// Byte offset one past this list's `G_ENDDL`.
    pub fn end_offset(&self) -> u32 {
        self.offset + (self.commands.len() * dl::CMD_SIZE) as u32
    }

    /// Triangles this list draws directly (not counting called sub-lists).
    pub fn triangle_count(&self) -> usize {
        self.commands
            .iter()
            .map(|c| match c {
                Cmd::Tri1(_) => 1,
                Cmd::Tri2(..) => 2,
                _ => 0,
            })
            .sum()
    }

    /// Offsets of display lists this one calls or branches to.
    pub fn referenced_lists(&self) -> Vec<u32> {
        self.commands
            .iter()
            .filter_map(|c| match c {
                Cmd::Call(a) | Cmd::Branch(a) => Some(a.0),
                _ => None,
            })
            .collect()
    }
}

/// Decides whether a decoded command run is really a display list.
///
/// Strict on purpose: a false positive pollutes the inventory that later design
/// decisions rest on, and a false negative just means one list is missed.
fn is_plausible(cmds: &[Cmd], file_len: usize) -> bool {
    if cmds.len() < MIN_COMMANDS {
        return false;
    }
    // Must terminate properly rather than running off the end.
    if !matches!(cmds.last(), Some(Cmd::End)) {
        return false;
    }
    // Any unrecognised opcode means we are looking at arbitrary data.
    if cmds.iter().any(|c| matches!(c, Cmd::Other { .. })) {
        return false;
    }
    // Must actually do something a renderer cares about.
    let does_work = cmds.iter().any(|c| {
        matches!(
            c,
            Cmd::Vtx { .. }
                | Cmd::Tri1(_)
                | Cmd::Tri2(..)
                | Cmd::Call(_)
                | Cmd::Branch(_)
                | Cmd::SetTimg { .. }
        )
    });
    if !does_work {
        return false;
    }
    cmds.iter().all(|c| match *c {
        // A call target is either a relocated pointer (segment 0, a
        // file-relative offset) or a *segmented* address the RSP resolves at
        // runtime — Smash uses segment 0x0E for the graphics heap, e.g.
        // `DE000000 0E000000` in file 105. Segmented targets cannot be
        // followed at build time, but their presence does not make the list
        // invalid, so only segment-0 addresses are bounds-checked.
        Cmd::Call(addr) | Cmd::Branch(addr) => addr.segment() != 0 || (addr.0 as usize) < file_len,

        // The RSP vertex cache holds 32 entries. A load that overflows it, or
        // loads nothing, is impossible on real hardware.
        Cmd::Vtx {
            count,
            dest_index,
            addr,
        } => {
            (addr.0 as usize) < file_len
                && count > 0
                && count <= VTX_CACHE_SIZE
                && dest_index.saturating_add(count) <= VTX_CACHE_SIZE
        }

        // Triangles may only index within the cache.
        Cmd::Tri1(v) => v.iter().all(|&i| i < VTX_CACHE_SIZE),
        Cmd::Tri2(a, b) => a.iter().chain(b.iter()).all(|&i| i < VTX_CACHE_SIZE),

        _ => true,
    })
}

/// How candidate display-list start offsets are chosen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Candidates {
    /// Only relocation targets. High precision: every one is a pointer the game
    /// itself follows. May miss lists reached by computed offset rather than by
    /// a relocated pointer.
    RelocTargets,
    /// Every 8-byte-aligned offset. Higher recall, and slower, but the
    /// validation in [`is_plausible`] is strict enough that false positives
    /// stay rare. Use this to check the coverage of `RelocTargets`.
    Exhaustive,
}

/// Finds every display list in a relocated file.
pub fn find_display_lists(file: &File) -> Vec<FoundDl> {
    find_display_lists_with(file, Candidates::RelocTargets)
}

/// Maps each file id to the set of byte offsets that *other* files' extern
/// relocations point at.
///
/// A display list living in file B but referenced only from file A has nothing
/// in B's own intern chain pointing at it, so scanning B in isolation misses
/// it entirely. Building the cross-file edge set first recovers those — using
/// the game's real pointer graph rather than guessing at offsets.
pub type CrossFileTargets = BTreeMap<u32, BTreeSet<u32>>;

/// Collects inbound extern-relocation targets for every file.
pub fn cross_file_targets<'a>(files: impl Iterator<Item = &'a File>) -> CrossFileTargets {
    let mut map: CrossFileTargets = BTreeMap::new();
    for f in files {
        for r in &f.extern_relocs {
            map.entry(u32::from(r.target_file))
                .or_default()
                .insert(r.target_offset);
        }
    }
    map
}

/// Finds display lists at an explicit set of candidate offsets.
pub fn find_display_lists_at(file: &File, candidates: &BTreeSet<u32>) -> Vec<FoundDl> {
    scan_candidates(file, candidates)
}

/// Finds **self-contained** display lists — the production discovery path.
///
/// Earlier attempts used relocation targets as candidate DL starts. That turns
/// out to be wrong: most reloc targets are the *vertex array* pointers carried
/// by `G_VTX`, not list starts, and decoding a "display list" at a vertex array
/// produces plausible-looking garbage that passes structural validation.
///
/// The reliable discriminator is semantic rather than structural: **a real
/// display list fills its own vertex cache before drawing**. So this scans
/// every aligned offset and keeps only the lists that convert cleanly via
/// [`crate::mesh::convert`] and actually produce triangles. Garbage fails with
/// `EmptyCacheSlot` almost immediately.
///
/// Overlapping results are then reduced to the outermost list, since a real
/// list's interior offsets often decode as valid suffixes of it.
pub fn find_root_display_lists(file: &File) -> Vec<FoundDl> {
    let mut found: Vec<FoundDl> = Vec::new();

    for off in (0..file.data.len().saturating_sub(dl::CMD_SIZE - 1)).step_by(dl::CMD_SIZE) {
        // Cheap structural filter first; conversion is far more expensive and
        // the overwhelming majority of offsets are not display lists.
        //
        // Decoding *at* `off` rather than at zero is what lets a `G_SETTIMG`
        // in a discovered list find its relocation: the slot a relocation is
        // keyed by is a file offset, so a list decoded as if it began at zero
        // looks up the wrong one and reads every cross-file texture as null
        // (RE-047).
        let Ok(cmds) = dl::decode_list_at(&file.data[off..], off as u32) else {
            continue;
        };
        if !is_plausible(&cmds, file.data.len()) {
            continue;
        }
        // The semantic test: a real list fills its own vertex cache.
        let draws = crate::mesh::convert(&cmds, crate::mesh::Source::of(file))
            .map(|m| m.triangle_count() > 0)
            .unwrap_or(false);
        if draws {
            found.push(FoundDl {
                offset: off as u32,
                commands: cmds,
            });
        }
    }

    // A real list's interior offsets frequently decode as valid suffixes of it.
    // Keep only outermost lists: drop any whose byte range is contained in
    // another's. Scanning forward, a list is contained iff some earlier list
    // ends at or beyond its end.
    let mut kept: Vec<FoundDl> = Vec::new();
    let mut covered_to = 0u32;
    for d in found {
        if d.offset >= covered_to {
            covered_to = d.end_offset();
            kept.push(d);
        }
    }
    kept
}

/// Finds display lists using the given candidate strategy.
pub fn find_display_lists_with(file: &File, how: Candidates) -> Vec<FoundDl> {
    let candidates: BTreeSet<u32> = match how {
        Candidates::RelocTargets => {
            let targets: BTreeSet<u32> = file.intern_relocs.iter().map(|r| r.target).collect();
            // Offset 0 only as a fallback. Injecting it unconditionally adds a
            // candidate that starts in whatever precedes the first real list,
            // walks through it, and terminates at the same `G_ENDDL` — a
            // duplicate that then has to be filtered back out.
            if targets.is_empty() {
                core::iter::once(0).collect()
            } else {
                targets
            }
        }
        Candidates::Exhaustive => (0..file.data.len() as u32).step_by(dl::CMD_SIZE).collect(),
    };
    scan_candidates(file, &candidates)
}

fn scan_candidates(file: &File, candidates: &BTreeSet<u32>) -> Vec<FoundDl> {
    let mut found = Vec::new();
    for &off in candidates {
        let start = off as usize;
        if start >= file.data.len() || !start.is_multiple_of(dl::CMD_SIZE) {
            continue;
        }
        // Absolute, for the same reason as `find_root_display_lists`.
        let Ok(cmds) = dl::decode_list_at(&file.data[start..], off) else {
            continue;
        };
        if is_plausible(&cmds, file.data.len()) {
            found.push(FoundDl {
                offset: off,
                commands: cmds,
            });
        }
    }

    // Several candidates can terminate at the same `G_ENDDL` — a pointer into
    // the middle of a list, or (in Exhaustive mode) every offset within one.
    // Keep the **earliest** start for each terminator, which is the most
    // complete list; the others are suffixes of it and would double-count.
    let mut earliest: BTreeMap<u32, u32> = BTreeMap::new();
    for d in &found {
        earliest
            .entry(d.end_offset())
            .and_modify(|start| *start = (*start).min(d.offset))
            .or_insert(d.offset);
    }
    found.retain(|d| earliest.get(&d.end_offset()) == Some(&d.offset));
    found
}

/// Aggregate statistics across many files.
///
/// This is the evidence base for the converter: which opcodes must be handled,
/// which texture formats must be supported, and how big the geometry is.
#[derive(Debug, Default, Clone)]
pub struct Inventory {
    /// Files that contained at least one display list.
    pub files_with_dls: usize,
    pub display_lists: usize,
    pub triangles: usize,
    /// `G_VTX` commands, and the total vertices they load.
    pub vertex_loads: usize,
    pub vertices_loaded: usize,
    /// Largest `count` seen on a single `G_VTX`. Bounds the vertex cache.
    pub max_vtx_batch: u8,
    /// Occurrences of each opcode.
    pub opcodes: BTreeMap<u8, usize>,
    /// `(format, bit size)` pairs seen on `G_SETTILE`, and how often.
    pub texture_formats: BTreeMap<(u8, u8), usize>,
    /// Geometry mode bits set, and how often.
    pub geometry_mode_set: BTreeMap<u32, usize>,
    /// Distinct TLUT load sizes, indicating palette sizes in use.
    pub tlut_sizes: BTreeMap<u16, usize>,
    /// Commands per display list, for sizing conversion buffers.
    pub max_commands: usize,
}

impl Inventory {
    /// Folds one file's display lists into the running totals.
    pub fn add_file(&mut self, dls: &[FoundDl]) {
        if dls.is_empty() {
            return;
        }
        self.files_with_dls += 1;
        self.display_lists += dls.len();

        for d in dls {
            self.max_commands = self.max_commands.max(d.commands.len());
            for cmd in &d.commands {
                self.record(cmd);
            }
        }
    }

    fn record(&mut self, cmd: &Cmd) {
        let op = opcode_of(cmd);
        *self.opcodes.entry(op).or_default() += 1;

        match *cmd {
            Cmd::Tri1(_) => self.triangles += 1,
            Cmd::Tri2(..) => self.triangles += 2,
            Cmd::Vtx { count, .. } => {
                self.vertex_loads += 1;
                self.vertices_loaded += count as usize;
                self.max_vtx_batch = self.max_vtx_batch.max(count);
            }
            Cmd::SetTile { format, size, .. } => {
                *self.texture_formats.entry((format, size)).or_default() += 1;
            }
            Cmd::LoadTlut { count, .. } => {
                *self.tlut_sizes.entry(count).or_default() += 1;
            }
            Cmd::GeometryMode { set, .. } => {
                // Record individual bits, which is what a converter switches on.
                for bit in 0..32 {
                    if set & (1 << bit) != 0 {
                        *self.geometry_mode_set.entry(1 << bit).or_default() += 1;
                    }
                }
            }
            _ => {}
        }
    }
}

/// The opcode byte a decoded command came from.
fn opcode_of(cmd: &Cmd) -> u8 {
    match *cmd {
        Cmd::Vtx { .. } => dl::G_VTX,
        Cmd::Tri1(_) => dl::G_TRI1,
        Cmd::Tri2(..) => dl::G_TRI2,
        Cmd::Call(_) | Cmd::Branch(_) => dl::G_DL,
        Cmd::End => dl::G_ENDDL,
        Cmd::Mtx { .. } => dl::G_MTX,
        Cmd::PopMtx { .. } => dl::G_POPMTX,
        Cmd::GeometryMode { .. } => dl::G_GEOMETRYMODE,
        Cmd::Texture { .. } => dl::G_TEXTURE,
        Cmd::SetTimg { .. } => dl::G_SETTIMG,
        Cmd::SetTile { .. } => dl::G_SETTILE,
        Cmd::SetTileSize { .. } => dl::G_SETTILESIZE,
        Cmd::LoadBlock { .. } => dl::G_LOADBLOCK,
        Cmd::LoadTlut { .. } => dl::G_LOADTLUT,
        Cmd::SetPrimColor { .. } => dl::G_SETPRIMCOLOR,
        Cmd::SetEnvColor(_) => dl::G_SETENVCOLOR,
        Cmd::SetBlendColor(_) => dl::G_SETBLENDCOLOR,
        Cmd::SetFogColor(_) => dl::G_SETFOGCOLOR,
        Cmd::SetCombine { .. } => dl::G_SETCOMBINE,
        Cmd::SetOtherModeH { .. } => dl::G_SETOTHERMODE_H,
        Cmd::SetOtherModeL { .. } => dl::G_SETOTHERMODE_L,
        Cmd::Sync(op) => op,
        Cmd::Other { opcode, .. } => opcode,
    }
}

/// Human-readable name for an opcode, for reports.
pub fn opcode_name(op: u8) -> &'static str {
    match op {
        dl::G_NOOP => "G_NOOP",
        dl::G_VTX => "G_VTX",
        dl::G_MODIFYVTX => "G_MODIFYVTX",
        dl::G_CULLDL => "G_CULLDL",
        dl::G_BRANCH_Z => "G_BRANCH_Z",
        dl::G_TRI1 => "G_TRI1",
        dl::G_TRI2 => "G_TRI2",
        dl::G_QUAD => "G_QUAD",
        dl::G_TEXTURE => "G_TEXTURE",
        dl::G_POPMTX => "G_POPMTX",
        dl::G_GEOMETRYMODE => "G_GEOMETRYMODE",
        dl::G_MTX => "G_MTX",
        dl::G_MOVEWORD => "G_MOVEWORD",
        dl::G_MOVEMEM => "G_MOVEMEM",
        dl::G_DL => "G_DL",
        dl::G_ENDDL => "G_ENDDL",
        dl::G_SPNOOP => "G_SPNOOP",
        dl::G_SETOTHERMODE_L => "G_SETOTHERMODE_L",
        dl::G_SETOTHERMODE_H => "G_SETOTHERMODE_H",
        dl::G_TEXRECT => "G_TEXRECT",
        dl::G_RDPLOADSYNC => "G_RDPLOADSYNC",
        dl::G_RDPPIPESYNC => "G_RDPPIPESYNC",
        dl::G_RDPTILESYNC => "G_RDPTILESYNC",
        dl::G_RDPFULLSYNC => "G_RDPFULLSYNC",
        dl::G_SETSCISSOR => "G_SETSCISSOR",
        dl::G_LOADTLUT => "G_LOADTLUT",
        dl::G_SETTILESIZE => "G_SETTILESIZE",
        dl::G_LOADBLOCK => "G_LOADBLOCK",
        dl::G_LOADTILE => "G_LOADTILE",
        dl::G_SETTILE => "G_SETTILE",
        dl::G_FILLRECT => "G_FILLRECT",
        dl::G_SETFOGCOLOR => "G_SETFOGCOLOR",
        dl::G_SETBLENDCOLOR => "G_SETBLENDCOLOR",
        dl::G_SETPRIMCOLOR => "G_SETPRIMCOLOR",
        dl::G_SETENVCOLOR => "G_SETENVCOLOR",
        dl::G_SETCOMBINE => "G_SETCOMBINE",
        dl::G_SETTIMG => "G_SETTIMG",
        _ => "?",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::archive::InternReloc;

    /// Builds a file containing one display list at `offset`.
    fn file_with(offset: u32, cmds: &[[u8; 8]], len: usize) -> File {
        let mut data = alloc::vec![0u8; len];
        for (i, c) in cmds.iter().enumerate() {
            let at = offset as usize + i * 8;
            data[at..at + 8].copy_from_slice(c);
        }
        File {
            id: 0,
            data,
            extern_relocs: Vec::new(),
            intern_relocs: alloc::vec![InternReloc {
                at: 0,
                target: offset
            }],
        }
    }

    fn cmd(w0: u32, w1: u32) -> [u8; 8] {
        let mut r = [0u8; 8];
        r[..4].copy_from_slice(&w0.to_be_bytes());
        r[4..].copy_from_slice(&w1.to_be_bytes());
        r
    }

    #[test]
    fn finds_a_display_list_at_a_reloc_target() {
        // G_VTX loading 4 verts from offset 0x40, one triangle, end.
        let vtx = cmd(0x0100_0000 | (4 << 12) | 8, 0x40); // n=4, v0=0
        let tri = cmd(0x0500_0204, 0);
        let end = cmd(0xDF00_0000, 0);
        let f = file_with(16, &[vtx, tri, end], 256);

        let dls = find_display_lists(&f);
        assert_eq!(dls.len(), 1);
        assert_eq!(dls[0].offset, 16);
        assert_eq!(dls[0].triangle_count(), 1);
    }

    /// A discovered list's `G_SETTIMG` slot has to be a *file* offset, because
    /// that is what a relocation is keyed by. Decoding the list as if it began
    /// at zero looks the relocation up at the wrong address, finds nothing, and
    /// reports a cross-file texture as an unresolved null — which is what made
    /// the texture report claim 71 failures the scene-graph path did not have
    /// (RE-047).
    #[test]
    fn a_discovered_lists_settimg_slot_is_a_file_offset() {
        const AT: u32 = 0x80;
        let vtx = cmd(0x0100_0000 | (4 << 12) | 8, 0x40);
        // A zeroed texture address: the archive blanks a cross-file pointer.
        let timg = cmd(0xFD10_0000, 0);
        let tri = cmd(0x0500_0204, 0);
        let end = cmd(0xDF00_0000, 0);
        let mut f = file_with(AT, &[vtx, timg, tri, end], 512);
        f.intern_relocs = alloc::vec![InternReloc { at: 0, target: AT }];

        let dls = find_display_lists(&f);
        let found = dls.iter().find(|d| d.offset == AT).expect("list at AT");
        let slot = found
            .commands
            .iter()
            .find_map(|c| match c {
                dl::Cmd::SetTimg { slot, .. } => Some(*slot),
                _ => None,
            })
            .expect("a G_SETTIMG");
        // Second command, second word: AT + 8 + 4.
        assert_eq!(
            slot,
            AT + 12,
            "the slot must be where the pointer lives in the file"
        );
    }

    #[test]
    fn rejects_unterminated_runs() {
        let tri = cmd(0x0500_0204, 0);
        let f = file_with(0, &[tri, tri], 16);
        assert!(find_display_lists(&f).is_empty());
    }

    #[test]
    fn rejects_data_containing_unknown_opcodes() {
        let junk = cmd(0xAB00_0000, 0);
        let end = cmd(0xDF00_0000, 0);
        let f = file_with(0, &[junk, end], 32);
        assert!(find_display_lists(&f).is_empty());
    }

    #[test]
    fn rejects_pointers_outside_the_file() {
        // A G_VTX pointing past the end cannot be a relocated pointer.
        let vtx = cmd(0x0100_0000 | (4 << 12), 0xFFFF);
        let end = cmd(0xDF00_0000, 0);
        let f = file_with(0, &[vtx, end], 64);
        assert!(find_display_lists(&f).is_empty());
    }

    #[test]
    fn rejects_lists_that_do_no_work() {
        // Sync + end is valid GBI but carries no geometry or material.
        let sync = cmd(0xE700_0000, 0);
        let end = cmd(0xDF00_0000, 0);
        let f = file_with(0, &[sync, end], 32);
        assert!(find_display_lists(&f).is_empty());
    }

    #[test]
    fn inventory_accumulates_across_files() {
        let vtx = cmd(0x0100_0000 | (8 << 12), 0x40);
        let tri2 = cmd(0x0600_0204, 0x0006_080A);
        let end = cmd(0xDF00_0000, 0);
        let f = file_with(0, &[vtx, tri2, end], 256);
        let dls = find_display_lists(&f);

        let mut inv = Inventory::default();
        inv.add_file(&dls);
        inv.add_file(&dls);

        assert_eq!(inv.files_with_dls, 2);
        assert_eq!(inv.display_lists, 2);
        assert_eq!(inv.triangles, 4); // TRI2 is two triangles, twice
        assert_eq!(inv.vertex_loads, 2);
        assert_eq!(inv.vertices_loaded, 16);
        assert_eq!(inv.max_vtx_batch, 8);
        assert_eq!(inv.opcodes[&dl::G_TRI2], 2);
    }

    #[test]
    fn empty_file_contributes_nothing() {
        let mut inv = Inventory::default();
        inv.add_file(&[]);
        assert_eq!(inv.files_with_dls, 0);
    }
}
