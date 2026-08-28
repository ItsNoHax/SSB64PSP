//! Material objects (`MObj`) — the per-node render state that is *not* in the
//! display list.
//!
//! A fighter's display list configures its tiles and then calls into segment
//! `0x0E`, the runtime graphics heap. `gcDrawMObjForDObj` fills that segment
//! with a small display list built from the node's `MObj` chain, so the call
//! lands on commands that exist only at run time. The bit the file is missing
//! is almost always the palette: Samus's joint list sets up a CI4 render tile,
//! calls `0x0E000000`, and immediately runs `G_LOADTLUT` — the `G_SETTIMG`
//! naming the palette came from the `MObj`.
//!
//! The `MObjSub` descriptors those `MObj`s are built from *are* in the file.
//! `gcSetupCustomDObjsWithMObj` reads them through a `MObjSub **table[]` laid
//! out in lockstep with the `DObjDesc` array, so slot `i` belongs to node `i`,
//! and each slot points at a NULL-terminated `MObjSub *` list. The display
//! list picks one of them by index: the segment-`0x0E` target is
//! `0x0E000000 + 8 * i`, because `gcDrawMObjForDObj` writes one 8-byte
//! `gSPBranchList` per `MObj` at the head of the heap.
//!
//! Which table goes with which graph is not guessed: [`PartTables`] reads it
//! out of the `FTCommonPart` records that name both. [`demand`] then gives an
//! independent check on the result — decoding the display lists says how many
//! `MObj`s each node calls for, and the paired table's chains have to be
//! exactly that long. Across the archive they are, for every node whose chain
//! this crate can follow.

use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use core::ops::Range;

use crate::archive::File;
use crate::dl::Cmd;

/// `sizeof(MObjSub)`.
pub const MOBJSUB_SIZE: u32 = 0x78;

/// Segment holding the display list `gcDrawMObjForDObj` builds.
pub const GRAPHICS_HEAP_SEGMENT: u8 = 0x0E;

/// Bytes per `MObj` entry point in that segment: one `gSPBranchList`.
const ENTRY_SIZE: u32 = 8;

// Field offsets within `MObjSub`.
const F_SIZ: u32 = 0x03;
const F_SPRITES: u32 = 0x04;
const F_PALETTES: u32 = 0x2C;
const F_FLAGS: u32 = 0x30;
const F_PRIMCOLOR: u32 = 0x50;
const F_ENVCOLOR: u32 = 0x58;
const F_BLENDCOLOR: u32 = 0x5C;

/// `G_IM_SIZ_8b`, the only size that implies a 256-entry TLUT.
const G_IM_SIZ_8B: u8 = 1;

const MOBJ_FLAG_ALPHA: u16 = 1 << 0;
const MOBJ_FLAG_SPLIT: u16 = 1 << 1;
const MOBJ_FLAG_PALETTE: u16 = 1 << 2;
const MOBJ_FLAG_FRAC: u16 = 1 << 4;
const MOBJ_FLAG_PRIMCOLOR: u16 = 1 << 9;
const MOBJ_FLAG_ENVCOLOR: u16 = 1 << 10;
const MOBJ_FLAG_BLENDCOLOR: u16 = 1 << 11;

/// The commands one `MObj` contributes, in the order `gcDrawMObjForDObj`
/// emits them.
///
/// Only the fields that survive into a converted mesh are kept. Everything
/// indexed by a runtime counter is read at its initial value, because
/// `gcAddMObjForDObj` zeroes `palette_id`, `texture_id_curr` and
/// `texture_id_next` — index 0 is the neutral costume and the first frame of
/// any material animation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MObjMaterial {
    /// Byte offset of the `MObjSub` this was read from. Kept so a recovered
    /// chain can be checked against the offsets the decomp annotates.
    pub at: u32,
    /// `palettes[0]`, set as the texture image so a following `G_LOADTLUT`
    /// picks it up.
    pub palette: Option<u32>,
    /// True when the `MObj` runs the TLUT load itself instead of leaving it to
    /// the display list.
    pub loads_tlut: bool,
    /// Entries that load reads, from `MObjSub::siz`. Only meaningful when
    /// `loads_tlut`.
    pub palette_entries: u16,
    /// `sprites[0]`, set as the texture image after any palette work.
    pub sprite: Option<u32>,
    pub prim_color: Option<[u8; 4]>,
    pub env_color: Option<[u8; 4]>,
    pub blend_color: Option<[u8; 4]>,
}

impl MObjMaterial {
    /// Whether this `MObj` sets any state a converted mesh can carry.
    pub fn contributes(&self) -> bool {
        self.palette.is_some()
            || self.sprite.is_some()
            || self.prim_color.is_some()
            || self.env_color.is_some()
            || self.blend_color.is_some()
    }
}

/// A node's `MObj` chain, indexed the way the display list indexes it.
pub type NodeMaterials = Vec<MObjMaterial>;

/// A `MObjSub **table[]` recovered from a file, parallel to a scene graph.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MObjTable {
    /// Byte offset of the table within the file.
    pub offset: u32,
    /// One entry per graph node, in `DObjDesc` array order.
    pub nodes: Vec<NodeMaterials>,
}

fn read_u32(data: &[u8], at: u32) -> Option<u32> {
    let at = at as usize;
    Some(u32::from_be_bytes(data.get(at..at + 4)?.try_into().ok()?))
}

fn read_u16(data: &[u8], at: u32) -> Option<u16> {
    let at = at as usize;
    Some(u16::from_be_bytes(data.get(at..at + 2)?.try_into().ok()?))
}

fn read_rgba(data: &[u8], at: u32) -> Option<[u8; 4]> {
    let at = at as usize;
    data.get(at..at + 4)?.try_into().ok()
}

/// How many `MObj`s a display list expects, i.e. one past the highest
/// segment-`0x0E` entry it calls.
///
/// Callees are followed, since a joint list that shares a common tail can call
/// the heap from inside it. `visited` keeps a cycle from running away.
pub fn demand(cmds: &[Cmd], file: &[u8]) -> usize {
    fn walk(cmds: &[Cmd], file: &[u8], depth: u32, max: &mut usize) {
        for cmd in cmds {
            let (Cmd::Call(addr) | Cmd::Branch(addr)) = *cmd else {
                continue;
            };
            if addr.segment() == GRAPHICS_HEAP_SEGMENT {
                let index = addr.offset() / ENTRY_SIZE;
                *max = (*max).max(index as usize + 1);
            } else if addr.segment() == 0 && depth < MAX_DEPTH {
                let at = addr.0 as usize;
                if let Some(sub) = file.get(at..).and_then(|d| crate::dl::decode_list(d).ok()) {
                    walk(&sub, file, depth + 1, max);
                }
            }
        }
    }
    const MAX_DEPTH: u32 = 8;
    let mut max = 0;
    walk(cmds, file, 0, &mut max);
    max
}

/// Reads the `MObjSub *` list at `at`, NULL-terminated, or `None` if the bytes
/// there are not one.
///
/// Every non-terminating entry must be a relocated pointer. The archive loader
/// knows exactly which slots hold pointers, so a run of plausible-looking
/// integers cannot pass.
fn read_list(file: &File, is_ptr: &dyn Fn(u32) -> bool, at: u32) -> Option<Vec<u32>> {
    let mut subs = Vec::new();
    let mut cursor = at;
    loop {
        let word = read_u32(&file.data, cursor)?;
        if word == 0 && !is_ptr(cursor) {
            return (!subs.is_empty()).then_some(subs);
        }
        if !is_ptr(cursor) || !fits(&file.data, word..word + MOBJSUB_SIZE) {
            return None;
        }
        subs.push(word);
        cursor = cursor.checked_add(4)?;
        // A list this long is not a material chain; bail rather than scanning
        // the rest of the file.
        if subs.len() > MAX_CHAIN {
            return None;
        }
    }
}

/// `gcDrawMObjForDObj` walks the whole chain every frame, so these stay short;
/// the longest in the archive is well under this.
const MAX_CHAIN: usize = 16;

fn fits(data: &[u8], r: Range<u32>) -> bool {
    r.end as usize <= data.len() && r.start <= r.end
}

/// Reads one `MObjSub` into the material its `MObj` would emit.
fn read_material(file: &File, is_ptr: &dyn Fn(u32) -> bool, at: u32) -> Option<MObjMaterial> {
    let data = &file.data;
    let flags = read_u16(data, at + F_FLAGS)?;
    // `MOBJ_FLAG_NONE` is not "no material": the drawing code substitutes a
    // default that enables texturing but no palette, so it contributes nothing
    // we can recover here.
    let indirect = |field: u32| -> Option<u32> {
        let array = read_u32(data, at + field)?;
        // The array itself, and its first element, both have to be real
        // pointers. `sprites` is NULL on every fighter joint — the texture
        // lives in the display list — so this returns `None` far more often
        // than it succeeds, and that is correct.
        (array != 0 && is_ptr(at + field) && is_ptr(array))
            .then(|| read_u32(data, array))
            .flatten()
            .filter(|&target| target != 0)
    };

    let palette = (flags & MOBJ_FLAG_PALETTE != 0)
        .then(|| indirect(F_PALETTES))
        .flatten();
    // `gcDrawMObjForDObj` emits the texture image twice under different
    // guards: `FRAC | SPLIT` stages the *next* frame's texels for a block
    // load, and `FRAC | ALPHA` sets the one actually sampled. Reading only the
    // first missed every material that just names a texture — Dream Land's
    // ground among them, which drew white because its `G_SETTIMG` is a zero
    // the `MObj` was supposed to fill in (RE-045). Both indices are zero in a
    // static read, so accepting any of the three flags reads the same address
    // as either guard would.
    let sprite = (flags & (MOBJ_FLAG_FRAC | MOBJ_FLAG_SPLIT | MOBJ_FLAG_ALPHA) != 0)
        .then(|| indirect(F_SPRITES))
        .flatten();
    let flagged = |bit: u16, field: u32| (flags & bit != 0).then(|| read_rgba(data, at + field))?;

    Some(MObjMaterial {
        at,
        palette,
        loads_tlut: palette.is_some() && flags & (MOBJ_FLAG_SPLIT | MOBJ_FLAG_ALPHA) != 0,
        // `gDPLoadTLUTCmd(.., sub.siz == G_IM_SIZ_8b ? 0xFF : 0xF)`, as a count.
        palette_entries: if data.get((at + F_SIZ) as usize) == Some(&G_IM_SIZ_8B) {
            256
        } else {
            16
        },
        sprite,
        prim_color: flagged(MOBJ_FLAG_PRIMCOLOR, F_PRIMCOLOR),
        env_color: flagged(MOBJ_FLAG_ENVCOLOR, F_ENVCOLOR),
        blend_color: flagged(MOBJ_FLAG_BLENDCOLOR, F_BLENDCOLOR),
    })
}

/// Reads the `MObjSub **table[]` at a known offset, for a graph of
/// `node_count` nodes.
///
/// A slot is NULL, or points at a NULL-terminated `MObjSub *` chain. Stage
/// files reach chains in a *different* archive file through an extern
/// relocation; those are accepted as well-formed but read back empty, since
/// resolving them means loading the target file and nothing that needs them
/// yet does.
pub fn read_table(file: &File, offset: u32, node_count: usize) -> Option<MObjTable> {
    let slots = pointer_slots(file);
    let is_ptr = |at: u32| slots.binary_search(&at).is_ok();
    let external: Vec<u32> = {
        let mut v: Vec<u32> = file.extern_relocs.iter().map(|r| r.at).collect();
        v.sort_unstable();
        v
    };

    let mut nodes = Vec::with_capacity(node_count);
    for i in 0..node_count {
        let slot = offset.checked_add(4 * i as u32)?;
        let target = read_u32(&file.data, slot)?;
        if external.binary_search(&slot).is_ok() {
            nodes.push(Vec::new());
            continue;
        }
        if target == 0 && !is_ptr(slot) {
            nodes.push(Vec::new());
            continue;
        }
        if !is_ptr(slot) {
            return None;
        }
        nodes.push(
            read_list(file, &is_ptr, target)?
                .iter()
                .map(|&at| read_material(file, &is_ptr, at))
                .collect::<Option<_>>()?,
        );
    }
    Some(MObjTable { offset, nodes })
}

/// Sorted slot offsets the archive loader relocated, for membership tests.
fn pointer_slots(file: &File) -> Vec<u32> {
    let mut slots: Vec<u32> = file.intern_relocs.iter().map(|r| r.at).collect();
    slots.sort_unstable();
    slots
}

/// Which `MObjSub **table[]` belongs to which scene graph, recovered from the
/// `FTCommonPart` records that name both.
///
/// ```c
/// struct FTCommonPart {
///     DObjDesc *dobjdesc;
///     MObjSub ***p_mobjsubs;
///     AObjEvent32 ***p_costume_matanim_joints;
///     u8 flags;
/// };
/// ```
///
/// A fighter's `*Main` file holds these; the pointers cross into its `*Model`
/// file as extern relocations, which the archive records exactly. So two
/// adjacent extern slots, the first landing on a `DObjDesc` array we already
/// recovered and the second on the same file, name a graph and its table with
/// no guessing at all.
///
/// That matters more than it looks. Searching a model file for a table that
/// merely *fits* its graph is close to useless: Samus has two 33-node graphs
/// with identical display-list demands and two equally well-formed tables, and
/// across the archive a fits-the-graph search picked the same table as these
/// records only about half the time. Rather than a coin flip on which
/// palettes a fighter wears, graphs no record names simply get none.
#[derive(Debug, Default, Clone)]
pub struct PartTables {
    /// `(model file id, DObjDesc array offset) -> table offset`.
    by_graph: BTreeMap<(u32, u32), u32>,
    /// The same key, mapped to `p_costume_matanim_joints` — the third pointer
    /// of the record, which supplies the per-costume colours that overwrite
    /// the ones baked into `MObjSub` (RE-040).
    costumes: BTreeMap<(u32, u32), u32>,
}

impl PartTables {
    /// Scans `files` for the records.
    ///
    /// `accept(model_file, graph_offset, table_offset)` decides whether a
    /// candidate pair is real; callers should require both that a `DObjDesc`
    /// array starts at `graph_offset` and that [`read_table`] parses at
    /// `table_offset`. Two adjacent pointers into one file is a common enough
    /// shape that it needs the second half: `FTAttributes` stores
    /// `dobj_lookup` immediately before `shield_anim_joints`, and both point
    /// into the same `*ShieldPose` file, so 51 of those matched the record
    /// shape exactly until the table itself had to parse.
    pub fn scan<'a>(
        files: impl Iterator<Item = &'a File>,
        accept: impl Fn(u32, u32, u32) -> bool,
    ) -> Self {
        let mut by_graph = BTreeMap::new();
        let mut costumes = BTreeMap::new();
        for file in files {
            let targets: BTreeMap<u32, (u16, u32)> = file
                .extern_relocs
                .iter()
                .map(|r| (r.at, (r.target_file, r.target_offset)))
                .collect();
            for (&at, &(model, graph)) in &targets {
                // The `p_mobjsubs` slot sits immediately after `dobjdesc` and
                // points into the same file.
                let Some(&(same, table)) = targets.get(&(at + 4)) else {
                    continue;
                };
                if same == model && accept(model as u32, graph, table) {
                    by_graph.insert((model as u32, graph), table);
                    // `p_costume_matanim_joints` sits one slot further on, in
                    // the same file again.
                    if let Some(&(also, list)) = targets.get(&(at + 8)) {
                        if also == model {
                            costumes.insert((model as u32, graph), list);
                        }
                    }
                }
            }
        }
        PartTables { by_graph, costumes }
    }

    /// Records a pairing found some other way — stage layers name theirs
    /// through `MPGroundDesc` rather than `FTCommonPart`; see
    /// [`crate::stage`].
    pub fn insert(&mut self, file: u32, graph_offset: u32, table_offset: u32) {
        self.by_graph.insert((file, graph_offset), table_offset);
    }

    /// Where a graph's per-costume colour lists live, if the record named any.
    pub fn costumes_for(&self, file: u32, graph_offset: u32) -> Option<u32> {
        self.costumes.get(&(file, graph_offset)).copied()
    }

    pub fn table_for(&self, file: u32, graph_offset: u32) -> Option<u32> {
        self.by_graph.get(&(file, graph_offset)).copied()
    }

    pub fn len(&self) -> usize {
        self.by_graph.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_graph.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::archive::{ExternReloc, InternReloc};
    use alloc::vec;

    const TABLE: u32 = 0x00;
    const LIST_A: u32 = 0x20;
    const LIST_B: u32 = 0x30;
    const SUB_A: u32 = 0x40;
    const SUB_B: u32 = SUB_A + MOBJSUB_SIZE;
    const PALSET: u32 = 0x140;
    const PALETTE: u32 = 0x180;

    /// A five-node table: node 1 chains two `MObjSub`s, node 4 chains one,
    /// the rest are NULL.
    ///
    /// The gap between the two occupied slots matters. With a single chain,
    /// any window that lands the pointer on the one slot that needs one
    /// matches, and the tests below could not tell a correct read from an
    /// off-by-one — which is also why the real search is only trustworthy
    /// against a graph's full node vector.
    fn fixture() -> File {
        let mut data = vec![0u8; 0x200];
        let mut relocs = Vec::new();
        let mut put = |at: u32, v: u32| {
            data[at as usize..at as usize + 4].copy_from_slice(&v.to_be_bytes());
            relocs.push(InternReloc { at, target: v });
        };
        put(TABLE + 0x04, LIST_A);
        put(TABLE + 0x10, LIST_B);
        put(LIST_A, SUB_A);
        put(LIST_A + 4, SUB_B);
        put(LIST_B, SUB_A);
        put(SUB_A + F_PALETTES, PALSET);
        put(SUB_B + F_PALETTES, PALSET);
        put(PALSET, PALETTE);
        for sub in [SUB_A, SUB_B] {
            data[(sub + F_FLAGS) as usize + 1] = MOBJ_FLAG_PALETTE as u8;
        }

        File {
            id: 0,
            data,
            extern_relocs: Vec::new(),
            intern_relocs: relocs,
        }
    }

    #[test]
    fn reads_a_table_and_its_chains() {
        let table = read_table(&fixture(), TABLE, 5).expect("table");
        assert_eq!(table.offset, TABLE);
        assert_eq!(table.nodes[0], Vec::new());
        assert_eq!(table.nodes[1].len(), 2);
        assert_eq!(table.nodes[1][0].palette, Some(PALETTE));
        assert_eq!(table.nodes[1][0].at, SUB_A);
        assert_eq!(table.nodes[4].len(), 1);
    }

    #[test]
    fn a_chain_entry_that_was_never_relocated_is_not_a_pointer() {
        // Without the relocation the word is just an integer that happens to
        // land inside the file, which is exactly what a length-and-range check
        // alone would wave through.
        let mut file = fixture();
        file.intern_relocs.retain(|r| r.at != LIST_A);
        assert_eq!(read_table(&file, TABLE, 5), None);
    }

    #[test]
    fn a_slot_relocated_into_another_file_reads_back_empty() {
        // Stage tables reach chains that live in a different archive file.
        // The table is still well-formed; we just cannot follow that slot.
        let mut file = fixture();
        file.intern_relocs.retain(|r| r.at != TABLE + 0x10);
        file.extern_relocs.push(ExternReloc {
            at: TABLE + 0x10,
            target_file: 7,
            target_offset: 0x40,
        });
        let table = read_table(&file, TABLE, 5).expect("table");
        assert_eq!(table.nodes[4], Vec::new());
        assert_eq!(table.nodes[1].len(), 2);
    }

    #[test]
    fn a_part_record_names_the_table_next_to_the_graph_it_points_at() {
        let namer = File {
            id: 1,
            data: vec![0u8; 0x20],
            // `dobjdesc` then `p_mobjsubs`, both into file 9.
            extern_relocs: vec![
                ExternReloc {
                    at: 0x00,
                    target_file: 9,
                    target_offset: 0x3520,
                },
                ExternReloc {
                    at: 0x04,
                    target_file: 9,
                    target_offset: 0x0000,
                },
            ],
            intern_relocs: Vec::new(),
        };
        let tables = PartTables::scan([&namer].into_iter(), |f, g, _| (f, g) == (9, 0x3520));
        assert_eq!(tables.table_for(9, 0x3520), Some(0));
        assert_eq!(tables.table_for(9, 0x69D0), None);
    }

    #[test]
    fn a_pointer_pair_that_does_not_start_at_a_graph_is_not_a_part_record() {
        let namer = File {
            id: 1,
            data: vec![0u8; 0x20],
            extern_relocs: vec![
                ExternReloc {
                    at: 0x00,
                    target_file: 9,
                    target_offset: 0x1234,
                },
                ExternReloc {
                    at: 0x04,
                    target_file: 9,
                    target_offset: 0x0000,
                },
            ],
            intern_relocs: Vec::new(),
        };
        let tables = PartTables::scan([&namer].into_iter(), |f, g, _| (f, g) == (9, 0x3520));
        assert!(tables.is_empty());
    }

    #[test]
    fn demand_is_one_past_the_highest_heap_entry_called() {
        use crate::dl::SegAddr;
        let cmds = [
            Cmd::Call(SegAddr(0x0E00_0010)),
            Cmd::Call(SegAddr(0x0E00_0000)),
            Cmd::End,
        ];
        assert_eq!(demand(&cmds, &[]), 3);
        assert_eq!(demand(&[Cmd::End], &[]), 0);
    }
}
