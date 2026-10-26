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

use std::fmt::Write;

use crate::archive::File;
use crate::fighters::FIGHTERS;
use crate::matanim::{MaterialJoint, TRACK_TEXTURE_ID_CURRENT};
use crate::source::{be_u32, OverlayData, OVL2};
use crate::{emit, Error, Result, Source};

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

pub fn generate(rom: Option<&Source>) -> Result<String> {
    let mut joints = Vec::with_capacity(FIGHTERS.len());
    let mut locks = Vec::with_capacity(FIGHTERS.len());
    let mut hidden = Vec::with_capacity(FIGHTERS.len());
    let mut textures = Vec::with_capacity(FIGHTERS.len());
    let mut made_rows = Vec::with_capacity(FIGHTERS.len());
    let ovl2 = OverlayData::new(rom, OVL2);
    for (k, f) in FIGHTERS.iter().enumerate() {
        let (setup, dl, lock, parts, texture, made) = match rom {
            Some(rom) => {
                let main = rom.file(f.main_file)?;
                let setup = flag_pair(&main, f.attributes + SETUP_PARTS_OFFSET)?;
                let dl = dl_mask(rom, &main, f.attributes)?;
                let lock = flag_pair(&main, f.attributes + ANIMLOCK_OFFSET)?;
                let parts = hidden_parts(&main, f.attributes, HIDDEN_PART_COUNTS[k], dl)?;
                let parts = if NO_HIDDEN_PARTS.contains(&k) {
                    Vec::new()
                } else {
                    parts
                };
                let texture = texture_parts(&main, f.attributes)?;
                let made = match &texture {
                    Some(t) => {
                        texture_part_made(rom, &main, f.attributes, t, costume_count(&ovl2, k)?)?
                    }
                    None => Default::default(),
                };
                (setup, dl & setup, lock, parts, texture, made)
            }
            None => (0, 0, 0, Vec::new(), None, Default::default()),
        };
        let cells: Vec<String> = made
            .iter()
            .map(|([hi, lo], desc)| format!("([{hi}, {lo}], {desc})"))
            .collect();
        made_rows.push(format!("    [{}], // {}", cells.join(", "), f.name));
        joints.push(format!("    (0x{setup:x}, 0x{dl:x}), // {}", f.name));
        locks.push(format!("    0x{lock:x}, // {}", f.name));
        let cells: Vec<String> = parts
            .iter()
            .map(|(i, root, dl)| format!("({i}, {root}, {dl})"))
            .collect();
        hidden.push(format!("    &[{}], // {}", cells.join(", "), f.name));
        textures.push(match texture {
            Some(t) => {
                let cells: Vec<String> = t
                    .iter()
                    .map(|(joint, [hi, lo])| format!("({joint}, [{hi}, {lo}])"))
                    .collect();
                format!("    Some([{}]), // {}", cells.join(", "), f.name)
            }
            None => format!("    None, // {}", f.name),
        });
    }
    let mut out = emit::header("each fighter's setup_parts, display-list and animlock joint masks");
    out.push_str(
        "/// Per `FTKind`: `(setup_parts, setup_parts & has a display list)`, bit\n\
         /// `n` for descriptor `n`.\n",
    );
    let _ = writeln!(
        out,
        "#[rustfmt::skip]\npub const JOINT_MASKS: [(u64, u64); {}] = [\n{}\n];\n",
        joints.len(),
        joints.join("\n")
    );
    out.push_str("/// Per `FTKind`: `FTAttributes::animlock`, bit `n` for descriptor `n`.\n");
    let _ = writeln!(
        out,
        "#[rustfmt::skip]\npub const ANIMLOCK_MASKS: [u64; {}] = [\n{}\n];\n",
        locks.len(),
        locks.join("\n")
    );
    out.push_str(
        "/// Per `FTKind`: `FTAttributes::hiddenparts` whose root is joint 4 or\n\
         /// later, as `(index, root joint, high-detail descriptor has a display\n\
         /// list)`; empty for Master Hand.\n",
    );
    let _ = writeln!(
        out,
        "#[rustfmt::skip]\npub static HIDDEN_PARTS: [&[(u8, u8, bool)]; {}] = [\n{}\n];\n",
        hidden.len(),
        hidden.join("\n")
    );
    out.push_str(
        "/// Per `FTKind`: `FTAttributes::textureparts_container`, each part's\n\
         /// `(joint, [high-detail MObj, low-detail MObj])`; `None` when NULL.\n",
    );
    let _ = writeln!(
        out,
        "#[rustfmt::skip]\npub const TEXTURE_PARTS: [Option<[(u8, [u8; 2]); {TEXTURE_PARTS}]>; {}] = [\n{}\n];",
        textures.len(),
        textures.join("\n")
    );
    out.push_str(
        "\n/// Per `FTKind` and texture part: the `TextureIDCurrent` its costume\n\
         /// script gives the part's `MObj` when the `FTCommonPart` chain makes\n\
         /// it, at high and low detail, and whether the part's joint has an\n\
         /// `FTModelPartDesc` (whose parts carry their own scripts).\n",
    );
    let _ = writeln!(
        out,
        "#[rustfmt::skip]\npub const TEXTURE_PART_MADE: [[([i8; 2], bool); {TEXTURE_PARTS}]; {}] = [\n{}\n];",
        made_rows.len(),
        made_rows.join("\n")
    );
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stub_tables_are_empty() {
        let text = generate(None).unwrap();
        assert!(text.contains("pub static HIDDEN_PARTS: [&[(u8, u8, bool)]; 27]"));
        assert_eq!(text.matches("    &[], //").count(), 27);
        assert!(text.contains("pub const TEXTURE_PARTS: [Option<[(u8, [u8; 2]); 2]>; 27]"));
        assert_eq!(text.matches("    None, //").count(), 27);
        assert!(text.contains("pub const TEXTURE_PART_MADE: [[([i8; 2], bool); 2]; 27]"));
        assert_eq!(
            text.matches("    [([0, 0], false), ([0, 0], false)], //")
                .count(),
            27
        );
    }

    #[test]
    fn hidden_part_counts_follow_fighters() {
        assert_eq!(HIDDEN_PART_COUNTS.len(), FIGHTERS.len());
        assert_eq!(FIGHTERS[NO_HIDDEN_PARTS[0]].name, "Boss");
    }
}
