//! Each fighter's joint masks, for `ssb_game::modelpart`: `FTAttributes`'
//! `setup_parts` and `animlock` flag pairs, and which of its high-detail
//! `DObjDesc`s carry a display list (RE-425, RE-509); its `hiddenparts`
//! from joint 4 on (RE-468), its `textureparts_container` (RE-426) and the
//! `TextureIDCurrent` each texture part's costume script gives its `MObj`
//! (RE-569).
//!
//! `ssb-rom`'s `fighter::setup_parts`, `fighter::animlock`,
//! `fighter::common_parts`, `fighter::hidden_part` and
//! `fighter::texture_parts` decode the same fields;
//! `crates/ssb-rom/tests/model_parts.rs`, `fighter_fidelity.rs` and
//! `texture_parts.rs` check the generated tables against them.

use alloc::vec::Vec;

use crate::archive::File;
use crate::figatree_order::HiddenPart;
use crate::fighters::FIGHTERS;
use crate::matanim::{MaterialJoint, TRACK_TEXTURE_ID_CURRENT};
use crate::source::{be_u32, OverlayData, OVL2};
use crate::{Error, Result, Source};

/// `FTAttributes::setup_parts` (`u32 *`, two flag words).
pub const SETUP_PARTS_OFFSET: u32 = 0x29C;
/// `FTAttributes::animlock` (`u32 *`, two flag words).
pub const ANIMLOCK_OFFSET: u32 = 0x2A0;
/// `FTAttributes::commonparts_container` (`FTCommonPartContainer *`);
/// `commonparts[0].dobjdesc` is its first word, an extern pointer into the
/// fighter's model file.
pub const COMMONPARTS_OFFSET: u32 = 0x2D4;
/// `FTAttributes::hiddenparts` (`FTHiddenPart *`), immediately before
/// `commonparts_container`.
pub const HIDDENPARTS_OFFSET: u32 = 0x2D0;
/// `sizeof(FTHiddenPart)`: `root_joint_id`, `parent_joint_id`,
/// `partindex_0x8`, `joint_kind`, one word each.
pub const HIDDENPART_SIZE: u32 = 16;
/// `FTAttributes::textureparts_container` (`FTTexturePartContainer *`).
pub const TEXTUREPARTS_OFFSET: u32 = 0x330;
/// `FTTexturePartContainer`: two `FTTexturePart { u8 joint_id; u8
/// detail[2]; }`, 3 bytes each. A fighter the decompilation gives one part
/// reads the following bytes as its second and never sets it.
pub const TEXTURE_PARTS: usize = 2;
/// `nFTPartsJointCommonStart` and `FTPARTS_JOINT_NUM_MAX`: the hidden
/// parts `ssb_game::modelpart` keeps are those whose root joint lies in
/// this range (joints 0..3 are the fixed `TopN`/translate/rotate joints).
pub const HIDDEN_ROOTS: core::ops::Range<u32> = 4..37;
/// `FTKind`s (indices into `FIGHTERS`) whose hidden parts are emitted
/// empty: Master Hand (`nFTKindBoss`). `dBossMain_hiddenparts` holds one
/// root in range (joint 5), but the port gives Master Hand no model-part
/// state (`ssb_game::modelpart::joint_masks` is `None` for it, so
/// `apply_hidden_parts` never runs), and `ssb-rom`'s check skips it.
pub const NO_HIDDEN_PARTS: [usize; 1] = [12];
/// Each fighter's `d<Name>Main_hiddenparts[N]` length, in `FIGHTERS`
/// (`FTKind`) order: `refs/ssb-decomp-re/src/relocData/*Main.c`
/// (`grep 'hiddenparts\['`). The struct holds only the pointer, so the
/// length is layout the generator cannot read; walking past it reads the
/// next table.
pub const HIDDEN_PART_COUNTS: [u8; 27] = [
    4, 4, 4, 13, 4, 6, 5, 4, 8, 4, 4, 4, 4, // Mario .. Boss
    4, 4, 4, 4, 13, 4, 6, 5, 4, 6, 4, 4, 4, 4, // MMario .. GDonkey
];
/// `nFTPartsJointCommonStart`: `DObjDesc` `n` is joint `n + 4`.
pub const JOINT_COMMON_START: u32 = 4;
/// `sizeof(FTCommonPart)`: `dobjdesc`, `p_mobjsubs`,
/// `p_costume_matanim_joints` and `flags`.
pub const COMMONPART_SIZE: u32 = 16;
/// `FTAttributes::modelparts_container` (`FTModelPartContainer *`), one
/// `FTModelPartDesc *` per joint from joint 4 on.
pub const MODELPARTS_OFFSET: u32 = 0x328;
/// `MObj`s in one chain: more than any fighter part holds.
pub const CHAIN_MAX: u32 = 16;
/// `sizeof(DObjDesc)`: `id`, `dl`, then translate, rotate and scale.
pub const DOBJDESC_SIZE: u32 = 44;
/// `DOBJ_ARRAY_MAX`: the `id` that ends a `DObjDesc` array.
pub const DOBJ_ARRAY_MAX: u32 = 18;

fn missing(file: &File, at: u32) -> Error {
    Error::BadRelocation {
        file: file.id,
        offset: at as usize,
    }
}

/// The intern pointer at `at`, as a byte offset into `file`.
fn intern(file: &File, at: u32) -> Result<u32> {
    file.intern_relocs
        .iter()
        .find(|r| r.at == at)
        .map(|r| r.target)
        .ok_or_else(|| missing(file, at))
}

/// A `u32 *flags` pair as one mask, bit `n` for descriptor `n`: the first
/// word covers 0..31 and the second 32..63, each most-significant bit first
/// (`lbCommonSetupFighterPartsDObjs`, `ftParamSetAnimLocks`).
fn flag_pair(file: &File, at: u32) -> Result<u64> {
    let target = intern(file, at)? as usize;
    let (lo, hi) = (be_u32(&file.data, target)?, be_u32(&file.data, target + 4)?);
    let bit = |w: u32, i: u32| u64::from(w >> (31 - i) & 1);
    Ok((0..32).fold(0u64, |m, i| m | bit(lo, i) << i | bit(hi, i) << (i + 32)))
}

/// Which descriptors of `commonparts[0].dobjdesc` have a display list (a
/// non-null `dl` the archive relocated), up to the `DOBJ_ARRAY_MAX` entry.
fn dl_mask(rom: &Source, main: &File, attributes: u32) -> Result<u64> {
    let container = intern(main, attributes + COMMONPARTS_OFFSET)?;
    let link = main
        .extern_relocs
        .iter()
        .find(|r| r.at == container)
        .ok_or_else(|| missing(main, container))?;
    let model = rom.file(u32::from(link.target_file))?;
    let mut mask = 0u64;
    for n in 0..64u32 {
        let at = link.target_offset + n * DOBJDESC_SIZE;
        if be_u32(&model.data, at as usize)? == DOBJ_ARRAY_MAX {
            return Ok(mask);
        }
        let dl = be_u32(&model.data, at as usize + 4)?;
        if dl != 0 && model.intern_relocs.iter().any(|r| r.at == at + 4) {
            mask |= 1 << n;
        }
    }
    Err(missing(&model, link.target_offset))
}

/// `hiddenparts[0..count]` whose root joint is in [`HIDDEN_ROOTS`], as
/// `(index, root joint, has a display list)`; `dl` is [`dl_mask`]'s bit for
/// descriptor `root - 4`.
fn hidden_parts(main: &File, attributes: u32, count: u8, dl: u64) -> Result<Vec<(u8, u8, bool)>> {
    let table = intern(main, attributes + HIDDENPARTS_OFFSET)?;
    let mut parts = Vec::new();
    for i in 0..u32::from(count) {
        let root = be_u32(&main.data, (table + i * HIDDENPART_SIZE) as usize)?;
        if HIDDEN_ROOTS.contains(&root) {
            parts.push((i as u8, root as u8, dl >> (root - 4) & 1 != 0));
        }
    }
    Ok(parts)
}

/// What `figatree_order::figatree_order` builds a fighter's tree from.
pub struct Skeleton {
    /// Every high-detail `DObjDesc`'s `id & 0xFFF`, to the `DOBJ_ARRAY_MAX`
    /// entry.
    pub depths: Vec<u32>,
    /// `setup_parts`.
    pub mask: u64,
    /// `hiddenparts[0..HIDDEN_PART_COUNTS[k]]`, every root joint included.
    pub hidden: Vec<HiddenPart>,
}

/// Fighter `k`'s [`Skeleton`]: `lbCommonSetupFighterPartsDObjs` builds the
/// tree from `commonparts[0].dobjdesc` and `setup_parts`, and
/// `ftMainUpdateHiddenPartID` inserts `hiddenparts[i]` (RE-432).
pub fn skeleton(rom: &Source, k: usize) -> Result<Skeleton> {
    let f = &FIGHTERS[k];
    let main = rom.file(f.main_file)?;
    let mask = flag_pair(&main, f.attributes + SETUP_PARTS_OFFSET)?;
    let container = intern(&main, f.attributes + COMMONPARTS_OFFSET)?;
    let (file, at) = extern_at(&main, container).ok_or_else(|| missing(&main, container))?;
    let model = rom.file(file)?;
    let mut depths = Vec::new();
    loop {
        let id = be_u32(
            &model.data,
            (at + depths.len() as u32 * DOBJDESC_SIZE) as usize,
        )?;
        if id == DOBJ_ARRAY_MAX {
            break;
        }
        if depths.len() == 64 {
            return Err(missing(&model, at));
        }
        depths.push(id & 0xFFF);
    }
    let table = intern(&main, f.attributes + HIDDENPARTS_OFFSET)?;
    let hidden = (0..u32::from(HIDDEN_PART_COUNTS[k]))
        .map(|i| {
            let word = |w: u32| be_u32(&main.data, (table + i * HIDDENPART_SIZE + w * 4) as usize);
            Ok(HiddenPart {
                root_joint_id: word(0)?,
                parent_joint_id: word(1)?,
                joint_kind: word(3)?,
            })
        })
        .collect::<Result<_>>()?;
    Ok(Skeleton {
        depths,
        mask,
        hidden,
    })
}

/// Each texture part's `(joint, [high-detail MObj, low-detail MObj])`.
type TextureParts = [(u8, [u8; 2]); TEXTURE_PARTS];

/// `textureparts_container->textureparts[0..2]` as `(joint, detail)`, or
/// `None` for a NULL container or one outside the fighter's main file
/// (`ssb-rom`'s `fighter::texture_parts`).
fn texture_parts(main: &File, attributes: u32) -> Result<Option<TextureParts>> {
    let at = attributes + TEXTUREPARTS_OFFSET;
    let target = match main.intern_relocs.iter().find(|r| r.at == at) {
        Some(r) => r.target,
        None => match main
            .extern_relocs
            .iter()
            .find(|r| r.at == at && u32::from(r.target_file) == main.id)
        {
            Some(r) => r.target_offset,
            None => return Ok(None),
        },
    } as usize;
    let raw = main
        .data
        .get(target..target + 3 * TEXTURE_PARTS)
        .ok_or_else(|| missing(main, at))?;
    Ok(Some(core::array::from_fn(|i| {
        (raw[3 * i], [raw[3 * i + 1], raw[3 * i + 2]])
    })))
}

/// The extern pointer at `at` in `file`, as `(target file, offset)`.
fn extern_at(file: &File, at: u32) -> Option<(u32, u32)> {
    file.extern_relocs
        .iter()
        .find(|r| r.at == at)
        .map(|r| (u32::from(r.target_file), r.target_offset))
}

/// The intern pointer at `at`, or `None` for a NULL slot (no relocation).
fn intern_at(file: &File, at: u32) -> Option<u32> {
    file.intern_relocs
        .iter()
        .find(|r| r.at == at)
        .map(|r| r.target)
}

/// One texture part's made `TextureIDCurrent` at each detail, and whether
/// its joint has an `FTModelPartDesc` (RE-569).
type TexturePartMade = ([i8; 2], bool);

/// What each texture part's `MObj` shows when `lbCommonSetupFighterPartsDObjs`
/// (or a model-part change on a joint without an `FTModelPartDesc`) makes
/// it: `gcAddMObjForDObj` zeroes `texture_id_curr`, then
/// `lbCommonAddMObjForFighterPartsDObj` plays the `MObj`'s costume script
/// once at `anim_frame = costume` (`gcAddMObjMatAnimJoint`,
/// `gcParseMObjMatAnimJoint`, `gcPlayMObjMatAnim`). Detail `d` takes
/// `commonparts[1]`'s chain only where its descriptor has a list (the
/// `detail_id` rule), and the `MObj` at `textureparts[p].detail[d]` in it.
/// `costumes` is how many costumes the fighter can wear; every one must
/// give the same id, since `ssb_game::modelpart` keeps one per detail.
fn texture_part_made(
    rom: &Source,
    main: &File,
    attributes: u32,
    parts: &TextureParts,
    costumes: u32,
) -> Result<[TexturePartMade; TEXTURE_PARTS]> {
    let container = intern(main, attributes + COMMONPARTS_OFFSET)?;
    let descs = intern(main, attributes + MODELPARTS_OFFSET)?;
    let commonpart =
        |detail: u32, field: u32| extern_at(main, container + detail * COMMONPART_SIZE + field);
    let mut out = [([0i8; 2], false); TEXTURE_PARTS];
    for (slot, &(joint, index)) in out.iter_mut().zip(parts) {
        let Some(n) = u32::from(joint).checked_sub(JOINT_COMMON_START) else {
            continue;
        };
        slot.1 = intern_at(main, descs + n * 4).is_some();
        for d in 0..2u32 {
            let detail_id = if d == 0 {
                0
            } else {
                let (file, dobjdesc) = commonpart(1, 0).ok_or_else(|| missing(main, container))?;
                let model = rom.file(file)?;
                u32::from(intern_at(&model, dobjdesc + n * DOBJDESC_SIZE + 4).is_some())
            };
            // `p_mobjsubs[n]`: the NULL-terminated chain the `MObj`s are
            // made from, which must reach the part's `MObj`.
            let Some((file, subs)) = commonpart(detail_id, 4) else {
                continue;
            };
            let model = rom.file(file)?;
            let chain = intern_at(&model, subs + n * 4).map_or(0, |chain| {
                (0..CHAIN_MAX)
                    .take_while(|&m| intern_at(&model, chain + m * 4).is_some())
                    .count() as u32
            });
            let m = u32::from(index[d as usize]);
            if m >= chain {
                continue;
            }
            let Some((file, table)) = commonpart(detail_id, 8) else {
                continue;
            };
            let model = rom.file(file)?;
            let Some(script) = intern_at(&model, table + n * 4)
                .and_then(|scripts| intern_at(&model, scripts + m * 4))
            else {
                continue;
            };
            let ids: Vec<i8> = (0..costumes)
                .map(|costume| {
                    let mut joint = MaterialJoint::start(script, costume as f32);
                    joint
                        .tick(&model.data, 1.0)
                        .map_err(|_| missing(&model, script))?;
                    // `mobj->texture_id_curr = value`, a `u16`.
                    let id = joint
                        .track_value(TRACK_TEXTURE_ID_CURRENT)
                        .map_or(0, |v| v as u16);
                    i8::try_from(id).map_err(|_| missing(&model, script))
                })
                .collect::<Result<_>>()?;
            assert!(
                ids.iter().all(|&id| id == ids[0]),
                "file {} script 0x{script:X}: the texture id differs by costume ({ids:?}), \
                 which ssb_game::modelpart does not model",
                model.id
            );
            slot.0[d as usize] = ids[0];
        }
    }
    Ok(out)
}

/// How many costumes fighter `k` can wear: `dFTParamCostumeIDs[k].develop +
/// 1` for the twelve playable fighters, 1 for the rest
/// (`ftParamGetCostumeCommonID`).
fn costume_count(ovl2: &OverlayData, k: usize) -> Result<u32> {
    let (at, n) = crate::battle_data::COSTUME_IDS;
    if k >= n {
        return Ok(1);
    }
    Ok(u32::from(ovl2.u8(at + k as u32 * 8 + 7)?) + 1)
}

/// Most hidden parts any fighter has ([`HIDDEN_PART_COUNTS`]).
pub const HIDDEN_PARTS_MAX: usize = 13;
const _: () = {
    let mut k = 0;
    while k < HIDDEN_PART_COUNTS.len() {
        assert!(HIDDEN_PART_COUNTS[k] as usize <= HIDDEN_PARTS_MAX);
        k += 1;
    }
};
/// Bytes per fighter in the `HiddenParts` table: the count, then
/// `(index, root joint, has a display list)` per part.
pub const HIDDEN_PART_BYTES: usize = 1 + 3 * HIDDEN_PARTS_MAX;
/// Words per fighter in the `JointMasks` table: `setup_parts`,
/// `setup_parts & has a display list` and `animlock`, each as `(low, high)`
/// halves of the 64-bit mask.
pub const JOINT_MASK_WORDS: usize = 6;
/// Bytes per fighter in the `TextureParts` table: present, then `(joint,
/// high-detail MObj, low-detail MObj)` per part, then one pad byte.
pub const TEXTURE_PART_BYTES: usize = 8;
/// Bytes per fighter in the `TexturePartMade` table: `(high, low, has an
/// FTModelPartDesc)` per part (`i8` ids as bits), then two pad bytes.
pub const TEXTURE_PART_MADE_BYTES: usize = 8;

/// The `JointMasks` table, in `FIGHTERS` (`FTKind`) order.
pub fn joint_masks_bytes(c: &crate::tables::Ctx) -> Result<Vec<u8>> {
    let mut words = Vec::with_capacity(FIGHTERS.len() * JOINT_MASK_WORDS);
    for f in FIGHTERS.iter() {
        let main = c.rom.file(f.main_file)?;
        let setup = flag_pair(&main, f.attributes + SETUP_PARTS_OFFSET)?;
        let dl = dl_mask(c.rom, &main, f.attributes)?;
        let lock = flag_pair(&main, f.attributes + ANIMLOCK_OFFSET)?;
        for mask in [setup, dl & setup, lock] {
            words.extend([mask as u32, (mask >> 32) as u32]);
        }
    }
    Ok(crate::tables::le_words(&words))
}

/// The `HiddenParts` table: `hiddenparts` whose root is joint 4 or later;
/// none for [`NO_HIDDEN_PARTS`].
pub fn hidden_parts_bytes(c: &crate::tables::Ctx) -> Result<Vec<u8>> {
    let mut out = Vec::with_capacity(FIGHTERS.len() * HIDDEN_PART_BYTES);
    for (k, f) in FIGHTERS.iter().enumerate() {
        let mut row = [0u8; HIDDEN_PART_BYTES];
        if !NO_HIDDEN_PARTS.contains(&k) {
            let main = c.rom.file(f.main_file)?;
            let dl = dl_mask(c.rom, &main, f.attributes)?;
            let parts = hidden_parts(&main, f.attributes, HIDDEN_PART_COUNTS[k], dl)?;
            row[0] = parts.len() as u8;
            for (cell, &(i, root, dl)) in row[1..].as_chunks_mut::<3>().0.iter_mut().zip(&parts) {
                cell.copy_from_slice(&[i, root, u8::from(dl)]);
            }
        }
        out.extend_from_slice(&row);
    }
    Ok(out)
}

/// The `TextureParts` table: `textureparts_container`, absent when NULL.
pub fn texture_parts_bytes(c: &crate::tables::Ctx) -> Result<Vec<u8>> {
    let mut out = Vec::with_capacity(FIGHTERS.len() * TEXTURE_PART_BYTES);
    for f in FIGHTERS.iter() {
        let main = c.rom.file(f.main_file)?;
        let mut row = [0u8; TEXTURE_PART_BYTES];
        if let Some(parts) = texture_parts(&main, f.attributes)? {
            row[0] = 1;
            for (cell, &(joint, [hi, lo])) in
                row[1..7].as_chunks_mut::<3>().0.iter_mut().zip(&parts)
            {
                cell.copy_from_slice(&[joint, hi, lo]);
            }
        }
        out.extend_from_slice(&row);
    }
    Ok(out)
}

/// The `TexturePartMade` table ([`texture_part_made`]); zeros for a fighter
/// without texture parts.
pub fn texture_part_made_bytes(c: &crate::tables::Ctx) -> Result<Vec<u8>> {
    let ovl2 = OverlayData::new(Some(c.rom), OVL2);
    let mut out = Vec::with_capacity(FIGHTERS.len() * TEXTURE_PART_MADE_BYTES);
    for (k, f) in FIGHTERS.iter().enumerate() {
        let main = c.rom.file(f.main_file)?;
        let mut row = [0u8; TEXTURE_PART_MADE_BYTES];
        if let Some(parts) = texture_parts(&main, f.attributes)? {
            let costumes = costume_count(&ovl2, k)?;
            let made = texture_part_made(c.rom, &main, f.attributes, &parts, costumes)?;
            for (cell, ([hi, lo], desc)) in row[..6].as_chunks_mut::<3>().0.iter_mut().zip(made) {
                cell.copy_from_slice(&[hi as u8, lo as u8, u8::from(desc)]);
            }
        }
        out.extend_from_slice(&row);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hidden_part_counts_follow_fighters() {
        assert_eq!(HIDDEN_PART_COUNTS.len(), FIGHTERS.len());
        assert_eq!(FIGHTERS[NO_HIDDEN_PARTS[0]].name, "Boss");
    }
}
