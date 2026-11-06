//! Fighter motion scripts (`ftMotionCommand` bytecode), for
//! `ssb_game::motion`, as the pack's `TABLES` section holds them.
//!
//! A status's hitboxes, hit-status windows and flag timings are bytecode in
//! each fighter's `<id>_<Name>MainMotion` archive file, which
//! `ftMainSetStatus` starts and `ftMainUpdateMotionEventsAll` runs. The
//! words are carried as the ROM holds them, with each pointer turned into a
//! word index: into the fighter's own blob, into `FTCommonMoveset` (with
//! [`COMMON_BIT`]), or `NONE_PTR` for a pointer into any other file (throw
//! and damage descriptors). Alongside: each motion's script start,
//! animation length and clip from `dFT<Name>MotionDescs`, the special
//! statuses' motion ids, the combat fields of `FTAttributes`, and the demo
//! scripts of `dFT<Name>SubMotionDescs`.
//!
//! The tables (`TableId::Motion*`, `CombatAttrs`, `Demo*`):
//!
//! | Table | Element | Per |
//! |---|---|---|
//! | `MotionWords` | `u32` | each blob back to back: the 13 own `MainMotion` blobs, Metal Mario's own then Mario's |
//! | `MotionDescs` | 4 `u32` | each fighter's motions: script, `anim_desc`, length \| clip << 16, `END_BLOCKS` run |
//! | `MotionSpecial` | `i16` | each base fighter's special statuses' motion ids |
//! | `MotionIndex` | 8 `u32` | `FTKind`: words, motions, special, demo words: start and count each |
//! | `CommonStatusMotion` | `i16` | common status |
//! | `CommonMovesetWords` | `u32` | `FTCommonMoveset` |
//! | `MotionEndBlocks` | `u8` | the `END_BLOCKS` pool |
//! | `CombatAttrs` | [`ATTR_WORDS`] `u32` | `FTKind`: the `FTAttributes` words `ssb_game::motion::CombatAttrs` decodes |
//! | `DemoWords` | `u32` | each fighter's demo blob back to back |
//! | `DemoRows` | 24 `u32` | `FTKind`: each demo row's script word, `NONE_PTR` for none |

use alloc::collections::BTreeMap;
use alloc::vec::Vec;

use crate::anim_length::Lengths;
use crate::figatree_order::{figatree_order, TreeJoint};
use crate::fighters::{
    self, demo_words, motion_descs, Fighter, MainMotion, MotionDesc, RegionKind,
    ANIM_FLAG_SUBMOTION_SCRIPT, COMMON_MOVESET, FIGHTERS, NO_OFFSET,
};
use crate::model_parts::{skeleton, Skeleton};
use crate::source::OVL1;
use crate::tables::{le_bytes, le_words, Ctx};
use crate::{Result, Source};

/// A pointer outside the blob.
const NONE_PTR: u32 = 0xFFFF_FFFF;
/// Word indexes into the shared `FTCommonMoveset` carry this bit.
const COMMON_BIT: u32 = 0x4000_0000;
/// `nFTDemoStatusNull` .. `nFTDemoStatusIntroR` and the opening's nine
/// fighter-specific rows.
pub const DEMO_ROWS: usize = 24;
/// `MotionIndex` words per fighter.
pub const INDEX_WORDS: usize = 8;
/// `CombatAttrs` words per fighter: `size`, `rebound_anim_length`,
/// `shield_size`, `shield_break_vel_y`, `jostle_width`, `jostle_x`,
/// `hit_detect_range[3]` (`f32` bits); `effect_joint_ids[5]`,
/// `joint_itemlight_id` (`s32`); `dead_fgm_ids[2]`, `deadup_sfx`,
/// `damage_sfx`, `smash_sfx[3]`, `heavyget_sfx` (`u16`, one word each);
/// the `is_have_*` bitfield word; `joint_rfoot_id`, `joint_lfoot_id`
/// (`s32`); `joint_rfoot_rotate`, `joint_lfoot_rotate`, `unk_0x31C`,
/// `unk_0x320`, `item_pickup[8]`, `throw_walk*_anim_length[3]` (`f32`
/// bits).
pub const ATTR_WORDS: usize = 41;
/// `MotionEndBlocks`' bytes (layout: the pool's size, checked by the
/// reader).
pub const END_BLOCK_BYTES: usize = 16221;

/// Whether `a` and `b` are the same name.
const fn same(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    let mut i = 0;
    while i < a.len() {
        if a[i] != b[i] {
            return false;
        }
        i += 1;
    }
    true
}

/// The `MainMotion` words of fighter `name`'s blob: its own file, then (a
/// variant with a file of its own, Metal Mario) its base's.
const fn blob_words(f: &Fighter) -> usize {
    let Some(own) = f.main_motion else {
        return 0;
    };
    let mut n = own.words as usize;
    if let Some(base) = f.base {
        let mut i = 0;
        while i < FIGHTERS.len() {
            if same(FIGHTERS[i].name, base) {
                if let Some(b) = FIGHTERS[i].main_motion {
                    n += b.words as usize;
                }
            }
            i += 1;
        }
    }
    n
}

/// `MotionWords`' words: every blob (layout).
pub const MOTION_WORDS: usize = {
    let (mut n, mut i) = (0, 0);
    while i < FIGHTERS.len() {
        n += blob_words(&FIGHTERS[i]);
        i += 1;
    }
    n
};

/// `MotionDescs`' motions: every fighter's (layout).
pub const MOTION_COUNT: usize = {
    let (mut n, mut i) = (0, 0);
    while i < FIGHTERS.len() {
        n += FIGHTERS[i].motion_descs.1;
        i += 1;
    }
    n
};

/// `MotionSpecial`'s entries: every base fighter's special statuses
/// (layout).
pub const SPECIAL_COUNT: usize = {
    let (mut n, mut i) = (0, 0);
    while i < FIGHTERS.len() {
        if let Some((_, count)) = FIGHTERS[i].special_status_descs {
            n += count;
        }
        i += 1;
    }
    n
};

/// The demo blob's words: from its first array to `dFT<Name>SubMotionDescs`.
const fn demo_blob_words(f: &Fighter) -> usize {
    match f.demo {
        Some(d) => ((f.sub_motion_descs.0 - (OVL1.rom + (d.vram - OVL1.vram))) / 4) as usize,
        None => 0,
    }
}

/// `DemoWords`' words: every demo blob (layout).
pub const DEMO_WORDS: usize = {
    let (mut n, mut i) = (0, 0);
    while i < FIGHTERS.len() {
        n += demo_blob_words(&FIGHTERS[i]);
        i += 1;
    }
    n
};

/// `nFTCommonStatusSpecialStart`: `CommonStatusMotion`'s entries.
pub const COMMON_STATUS_COUNT: usize = crate::stat_flags::COMMON_STATUS_DESCS.1;
/// A `MainMotion` file as words, its pointers resolved, the first word at
/// index `base` of the blob it is part of.
fn motion_words(rom: &Source, file: &MainMotion, base: u32) -> Result<Vec<u32>> {
    let f = rom.file(file.file)?;
    let mut words: Vec<u32> = f
        .data
        .as_chunks::<4>()
        .0
        .iter()
        .map(|c| u32::from_be_bytes(*c))
        .collect();
    // The archive pads each file to 16 bytes past its last array.
    assert!(words.len() >= file.words as usize && words.len() - (file.words as usize) < 4);
    words.truncate(file.words as usize);
    for r in &f.intern_relocs {
        words[r.at as usize / 4] = base + r.target / 4;
    }
    for r in &f.extern_relocs {
        words[r.at as usize / 4] = if u32::from(r.target_file) == COMMON_MOVESET.file {
            COMMON_BIT | (r.target_offset / 4)
        } else {
            NONE_PTR
        };
    }
    for region in file.regions {
        let span = region.start as usize..(region.start + region.len) as usize;
        for w in &mut words[span] {
            match region.kind {
                RegionKind::Data => *w = 0,
                RegionKind::Pointers if *w == 0 => *w = NONE_PTR,
                RegionKind::Pointers => {}
            }
        }
    }
    Ok(words)
}

/// `MotionDesc::end_blocks` without a run in `END_BLOCKS`.
const NO_END_BLOCKS: u32 = 0xFFFF;

/// `MotionDesc::anim_desc`'s hidden-part bits (`ftMainSetStatus` masks out
/// the five low flags; bit `31 - i` is `hiddenparts[i]`).
const HIDDEN_PART_BITS: u32 = 0xFFFF_FFE0;

/// The `END_BLOCKS` pool: length-prefixed runs of the deciding joint's block
/// payloads, shared by the motions whose runs are equal.
#[derive(Default)]
struct EndPool {
    words: Vec<u8>,
    /// By `(FTKind, anim_file, hidden-part bits)`: the deciding joint
    /// depends on the fighter's tree as well as the file.
    at: BTreeMap<(usize, u32, u32), u32>,
    runs: BTreeMap<Vec<u8>, u32>,
}

impl EndPool {
    /// The run for `desc`'s figatree on fighter `k`, when it ends after
    /// `length` frames.
    fn end_of(
        &mut self,
        rom: &Source,
        lengths: &Lengths,
        skeleton: &Skeleton,
        k: usize,
        desc: &MotionDesc,
        length: u32,
    ) -> u32 {
        if desc.anim_file == 0 || length == 0 || length == 0xFFFF {
            return NO_END_BLOCKS;
        }
        let parts = desc.anim_desc & HIDDEN_PART_BITS;
        if let Some(&at) = self.at.get(&(k, desc.anim_file, parts)) {
            return at;
        }
        let order: Vec<u32> =
            figatree_order(&skeleton.depths, skeleton.mask, &skeleton.hidden, parts)
                .unwrap_or_else(|| {
                    panic!(
                        "{}: motion file {} parts 0x{parts:08X} names a hidden part whose parent \
                     does not exist",
                        FIGHTERS[k].name, desc.anim_file
                    )
                })
                .into_iter()
                .map(TreeJoint::joint_id)
                .collect();
        let at = match lengths.end_blocks(rom, desc.anim_file, &order) {
            Some(b)
                if b.iter().map(|&p| u32::from(p)).sum::<u32>() == length
                    && b.len() <= 0xFF
                    && b.iter().all(|&p| p <= 0xFF) =>
            {
                let b: Vec<u8> = b.iter().map(|&p| p as u8).collect();
                let words = &mut self.words;
                *self.runs.entry(b).or_insert_with_key(|b| {
                    let at = words.len() as u32;
                    assert!(at < NO_END_BLOCKS, "END_BLOCKS overflow");
                    words.push(b.len() as u8);
                    words.extend(b);
                    at
                })
            }
            _ => NO_END_BLOCKS,
        };
        self.at.insert((k, desc.anim_file, parts), at);
        at
    }
}

fn length_of(rom: &Source, lengths: &Lengths, desc: &MotionDesc) -> (u32, u32) {
    if desc.anim_file == 0 {
        return (0, 0xFFFE);
    }
    let frames = match lengths.frames(rom, desc.anim_file) {
        Some(Some(n)) => n,
        _ => 0xFFFF,
    };
    let clip = match lengths.clip(rom, desc.anim_file) {
        Some((n, _)) if n >= 0x8000 => 0xFFFF,
        Some((n, loops)) => n | if loops { 0x8000 } else { 0 },
        None => 0xFFFF,
    };
    (frames, clip)
}

/// A fighter's `CombatAttrs` words ([`ATTR_WORDS`]), as the ROM holds
/// them: `ssb_game::motion` decodes them.
fn attr_words(rom: &Source, f: &Fighter) -> Result<[u32; ATTR_WORDS]> {
    let main = rom.file(f.main_file)?;
    let at = f.attributes as usize;
    let word = |off: usize| crate::source::be_u32(&main.data, at + off);
    // `FTAttributes` from `dead_fgm_ids` (0xB4, after `cliffcatch_coll`)
    // to `heavyget_sfx` (0xE8, after `item_pickup` and the two throw
    // scales).
    let half = |off: usize| -> Result<u32> {
        let w = word(off & !3)?;
        Ok(if off & 2 == 0 { w >> 16 } else { w & 0xFFFF })
    };
    let mut out = Vec::with_capacity(ATTR_WORDS);
    // `FTAttributes`: size, rebound_anim_length, shield_size,
    // shield_break_vel_y, jostle_width, jostle_x, hit_detect_range.
    for off in [0x00, 0x1C, 0x74, 0x78, 0x80, 0x84, 0x290, 0x294, 0x298] {
        out.push(word(off)?);
    }
    // `effect_joint_ids[5]`, `joint_itemlight_id`.
    for i in 0..5 {
        out.push(word(0x2A4 + 4 * i)?);
    }
    out.push(word(0x33C)?);
    for off in [0xB4, 0xB6, 0xB8, 0xBA, 0xBC, 0xBE, 0xC0, 0xE8] {
        out.push(half(off)?);
    }
    // `is_have_attack11..is_have_voice`.
    out.push(word(0x100)?);
    // `joint_rfoot_id`, `joint_lfoot_id`, then their `*_rotate`
    // (0x2FC..0x30C, after `shield_anim_joints`).
    for i in 0..2 {
        out.push(word(0x2FC + 8 * i)?);
    }
    for i in 0..2 {
        out.push(word(0x300 + 8 * i)?);
    }
    // `unk_0x31C` and `unk_0x320`: `func_ovl2_800DDF74`'s reach clamp
    // and slope angle.
    out.push(word(0x31C)?);
    out.push(word(0x320)?);
    // `item_pickup` (0xC4): an `FTItemPickup`'s four `Vec2f`.
    for i in 0..8 {
        out.push(word(0xC4 + 4 * i)?);
    }
    // `throw_walkslow_anim_length` to `throw_walkfast_anim_length`.
    for i in 0..3 {
        out.push(word(0x10 + 4 * i)?);
    }
    Ok(out.try_into().expect("ATTR_WORDS"))
}

/// The fighter's demo blob, its pointers turned into word indexes, and each
/// demo row's script word (`NONE_PTR`: none).
fn demo(rom: &Source, f: &Fighter) -> Result<(Vec<u32>, [u32; DEMO_ROWS])> {
    let mut rows = [NONE_PTR; DEMO_ROWS];
    let (vram, mut blob) = demo_words(rom, f)?;
    let len = blob.len();
    let index = |ptr: u32| -> u32 {
        assert!(
            ptr >= vram && (ptr - vram).is_multiple_of(4),
            "{}: demo pointer 0x{ptr:08X}",
            f.name
        );
        let i = (ptr - vram) / 4;
        assert!((i as usize) < len, "{}: demo pointer past the blob", f.name);
        i
    };
    if let Some(demo) = f.demo {
        for &p in demo.pointers {
            blob[p as usize] = index(blob[p as usize]);
        }
    }
    let subs = motion_descs(rom, f.sub_motion_descs)?;
    for (row, desc) in rows.iter_mut().zip(&subs) {
        if desc.offset != NO_OFFSET && desc.offset != 0 {
            *row = index(desc.offset);
        }
    }
    Ok((blob, rows))
}

/// Every motion table, read once per section ([`Ctx`]).
pub struct Tables {
    words: Vec<u32>,
    descs: Vec<u32>,
    special: Vec<i16>,
    index: Vec<u32>,
    common_status: Vec<i16>,
    common_words: Vec<u32>,
    end_blocks: Vec<u8>,
    attrs: Vec<u32>,
    demo_words: Vec<u32>,
    demo_rows: Vec<u32>,
}

/// `(start, count)` of a run appended to `v`.
fn span<T>(v: &[T], from: usize) -> [u32; 2] {
    [from as u32, (v.len() - from) as u32]
}

fn read(rom: &Source, lengths: &Lengths) -> Result<Tables> {
    let mut pool = EndPool::default();
    let mut t = Tables {
        words: Vec::with_capacity(MOTION_WORDS),
        descs: Vec::with_capacity(MOTION_COUNT * 4),
        special: Vec::with_capacity(SPECIAL_COUNT),
        index: Vec::with_capacity(FIGHTERS.len() * INDEX_WORDS),
        common_status: fighters::common_status_motions(rom)?,
        common_words: motion_words(rom, &COMMON_MOVESET, COMMON_BIT)?,
        end_blocks: Vec::new(),
        attrs: Vec::with_capacity(FIGHTERS.len() * ATTR_WORDS),
        demo_words: Vec::with_capacity(DEMO_WORDS),
        demo_rows: Vec::with_capacity(FIGHTERS.len() * DEMO_ROWS),
    };
    // Each fighter's `[words, special]` spans, by `FTKind`: a variant
    // without a file of its own reads its base's.
    let mut spans: Vec<([u32; 2], [u32; 2])> = Vec::with_capacity(FIGHTERS.len());
    for (k, f) in FIGHTERS.iter().enumerate() {
        let base = f.base.map(|b| {
            FIGHTERS
                .iter()
                .position(|g| g.name == b)
                .expect("a base of FIGHTERS")
        });
        // Metal Mario's own MainMotion comes first in its blob; the base's
        // (`file_submotion`) follows it, its pointers moved by its length.
        let mut sub_base = 0;
        let words = match (f.main_motion, base) {
            (Some(own), None) => {
                let from = t.words.len();
                t.words.extend(motion_words(rom, &own, 0)?);
                span(&t.words, from)
            }
            (Some(own), Some(b)) => {
                let from = t.words.len();
                let own_words = motion_words(rom, &own, 0)?;
                sub_base = own_words.len() as u32;
                let base_mm = FIGHTERS[b].main_motion.expect("base MainMotion");
                t.words.extend(own_words);
                t.words.extend(motion_words(rom, &base_mm, sub_base)?);
                span(&t.words, from)
            }
            (None, Some(b)) => spans[b].0,
            (None, None) => panic!("{}: no MainMotion file", f.name),
        };
        let special = match (f.special_status_descs, base) {
            (Some((at, count)), _) => {
                let from = t.special.len();
                t.special.extend(fighters::status_motions(rom, at, count)?);
                span(&t.special, from)
            }
            (None, Some(b)) => spans[b].1,
            (None, None) => [0, 0],
        };
        spans.push((words, special));

        let skel = skeleton(rom, k)?;
        let motions_from = t.descs.len() / 4;
        for d in motion_descs(rom, f.motion_descs)? {
            let script = if d.offset == NO_OFFSET {
                NONE_PTR
            } else if base.is_none() {
                d.offset / 4
            } else {
                assert_eq!(d.offset % 4, 0, "{}: unaligned script", f.name);
                if d.anim_desc & ANIM_FLAG_SUBMOTION_SCRIPT != 0 {
                    sub_base + d.offset / 4
                } else {
                    assert!(
                        f.main_motion.is_some(),
                        "{}: script names no MainMotion file",
                        f.name
                    );
                    d.offset / 4
                }
            };
            let (length, clip) = length_of(rom, lengths, &d);
            let end = pool.end_of(rom, lengths, &skel, k, &d, length);
            assert!(length <= 0xFFFF && clip <= 0xFFFF && end <= 0xFFFF);
            t.descs
                .extend([script, d.anim_desc, length | clip << 16, end]);
        }
        let motions = [
            motions_from as u32,
            (t.descs.len() / 4 - motions_from) as u32,
        ];

        let (blob, rows) = demo(rom, f)?;
        let demo_from = t.demo_words.len();
        t.demo_words.extend(blob);
        let demo = span(&t.demo_words, demo_from);
        t.demo_rows.extend(rows);

        t.index.extend(words);
        t.index.extend(motions);
        t.index.extend(special);
        t.index.extend(demo);
        t.attrs.extend(attr_words(rom, f)?);
    }
    t.end_blocks = pool.words;
    assert_eq!(t.words.len(), MOTION_WORDS);
    assert_eq!(t.descs.len(), MOTION_COUNT * 4);
    assert_eq!(t.special.len(), SPECIAL_COUNT);
    assert_eq!(t.demo_words.len(), DEMO_WORDS);
    assert_eq!(t.common_status.len(), COMMON_STATUS_COUNT);
    assert_eq!(t.end_blocks.len(), END_BLOCK_BYTES, "END_BLOCKS' size");
    Ok(t)
}

/// The tables, read once per section ([`Ctx`]).
fn cached<'c>(c: &'c Ctx) -> Result<&'c Tables> {
    if let Some(t) = c.motion.get() {
        return Ok(t);
    }
    let t = read(c.rom, &c.lengths)?;
    Ok(c.motion.get_or_init(|| t))
}

/// The `MotionWords` table.
pub fn words_bytes(c: &Ctx) -> Result<Vec<u8>> {
    Ok(le_words(&cached(c)?.words))
}

/// The `MotionDescs` table.
pub fn descs_bytes(c: &Ctx) -> Result<Vec<u8>> {
    Ok(le_words(&cached(c)?.descs))
}

/// The `MotionSpecial` table.
pub fn special_bytes(c: &Ctx) -> Result<Vec<u8>> {
    Ok(le_bytes(&cached(c)?.special, i16::to_le_bytes))
}

/// The `MotionIndex` table.
pub fn index_bytes(c: &Ctx) -> Result<Vec<u8>> {
    Ok(le_words(&cached(c)?.index))
}

/// The `CommonStatusMotion` table: `dFTCommonActionStatusDescs[..]
/// .mflags.motion_id`, by status id (`-1`/`-2`: no motion).
pub fn common_status_bytes(c: &Ctx) -> Result<Vec<u8>> {
    Ok(le_bytes(&cached(c)?.common_status, i16::to_le_bytes))
}

/// The `CommonMovesetWords` table.
pub fn common_words_bytes(c: &Ctx) -> Result<Vec<u8>> {
    Ok(le_words(&cached(c)?.common_words))
}

/// The `MotionEndBlocks` table.
pub fn end_blocks_bytes(c: &Ctx) -> Result<Vec<u8>> {
    Ok(cached(c)?.end_blocks.clone())
}

/// The `CombatAttrs` table.
pub fn attrs_bytes(c: &Ctx) -> Result<Vec<u8>> {
    Ok(le_words(&cached(c)?.attrs))
}

/// The `DemoWords` table.
pub fn demo_words_bytes(c: &Ctx) -> Result<Vec<u8>> {
    Ok(le_words(&cached(c)?.demo_words))
}

/// The `DemoRows` table.
pub fn demo_rows_bytes(c: &Ctx) -> Result<Vec<u8>> {
    Ok(le_words(&cached(c)?.demo_rows))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The deciding joint's run for fighter `name`'s motions on `file`.
    fn end_run(rom: &Source, name: &str, file: u32) -> Vec<u16> {
        let k = FIGHTERS.iter().position(|f| f.name == name).unwrap();
        let desc = motion_descs(rom, FIGHTERS[k].motion_descs)
            .unwrap()
            .into_iter()
            .find(|d| d.anim_file == file)
            .unwrap();
        let skel = skeleton(rom, k).unwrap();
        let parts = desc.anim_desc & HIDDEN_PART_BITS;
        let order: Vec<u32> = figatree_order(&skel.depths, skel.mask, &skel.hidden, parts)
            .unwrap()
            .into_iter()
            .map(TreeJoint::joint_id)
            .collect();
        Lengths::default().end_blocks(rom, file, &order).unwrap()
    }

    /// The table's last script is not always the deciding joint's: hidden
    /// parts (Kirby's guard, `anim_desc` 0xA0000000, inserts `TransN` and
    /// `YRotN`) move entries onto other joints, and Mario's `DamageAir2`
    /// table runs past his tree. Each run is the script of the highest
    /// joint id bound to one (`lbCommonAddFighterPartsFigatree`,
    /// `ftParamUpdateAnimKeys`).
    #[test]
    fn the_deciding_joint_follows_the_bound_tree() {
        let Some(path) = crate::test_rom(env!("CARGO_MANIFEST_DIR")) else {
            return;
        };
        let rom = Source::open(&path).unwrap();
        // Kirby GuardOn: the last entry's script is one 4-frame block.
        assert_eq!(end_run(&rom, "Kirby", 1320), [1, 1, 1, 1]);
        assert_eq!(end_run(&rom, "NKirby", 1320), [1, 1, 1, 1]);
        assert_eq!(
            end_run(&rom, "Kirby", 1321),
            [2, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1]
        );
        assert_eq!(
            end_run(&rom, "Mario", 533),
            [1, 1, 1, 1, 6, 1, 8, 1, 1, 2, 1]
        );
        assert_eq!(end_run(&rom, "Purin", 1486), [3, 1, 1, 1, 1, 1]);
        let yoshi = end_run(&rom, "Yoshi", 1944);
        assert_eq!(yoshi[..4], [3, 1, 1, 7]);
        assert_eq!(yoshi.len(), 31);
    }
}
