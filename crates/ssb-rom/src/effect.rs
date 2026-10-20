//! Source-backed identities for the original effect manager's display assets.
//!
//! `ef/efmanager.c` has 53 static `EFDesc` records. Three are controller-only
//! and four pairs share one display asset, leaving these 46 unique objects.
//! The host pack verifier and the PSP audit build consume the same keys so an
//! exhaustive visual run cannot silently drift from the source inventory.

/// `llEFCommonEffects1QuakeMag0AnimJoint` to `...Mag3...` in file 83:
/// the quakes' single-`DObj` animations (`efManagerQuakeMakeEffect`), whose
/// translation moves the battle camera (RE-416).
pub const QUAKE_ANIM_FILE: u32 = 83;
pub const QUAKE_ANIM_JOINTS: [u32; 4] = [0xCBC0, 0xCC20, 0xCCF0, 0xCDC0];
/// The `AnimDesc::EFFECT` slot of quake magnitude 0; the others follow.
/// Past every [`MANAGER_EFFECT_KEYS`] slot.
pub const QUAKE_ANIM_SLOT: u32 = 0x100;

/// Pikachu's Thunder frames (RE-417): the head, trail and fading segment
/// all draw file 341's list at 0x94F8 with `MObjSub` 0x9420's
/// `sprites[texture_id_curr]`. The pack keys one mesh per sprite by its
/// offset, in `texture_id_curr` order (`WPPIKACHUTHUNDER_TEXTURES_NUM`).
pub const PIKACHU_THUNDER_FILE: u32 = 341;
pub const PIKACHU_THUNDER_SPRITES: [u32; 4] = [0x9020, 0x8C18, 0x8810, 0x8408];

/// Ness's PK Thunder trail frames (RE-550): `wpNessPKThunderTrailProcUpdate`
/// sets `mobj->texture_id_curr = syUtilsRandIntRange(WPPKTHUNDER_TEXTURES_NUM
/// - 1)` (0..=2) every tick, so the trail list (file 335's `DObjDLLink` array
/// at 0x8B40, one list at [`NESS_PK_TRAIL_LIST`]) draws one of the three
/// sprites of `MObjSub` 0x89D8. The pack keys one mesh per sprite by its
/// offset, in `texture_id_curr` order.
pub const NESS_PK_TRAIL_FILE: u32 = 335;
pub const NESS_PK_TRAIL_LIST: u32 = 0x8A98;
pub const NESS_PK_TRAIL_SPRITES: [u32; 3] = [0x85C8, 0x81C0, 0x7DB8];

/// `dEFManagerYoshiEggLayAnimJoints` (`efManagerYoshiEggLaySetAnim`'s
/// indices 0 and 1): the Egg Lay egg's Wait and Break tables in file 339,
/// played on its tree (RE-417). Index 2, the Throw table, is the `EFDesc`'s
/// own (`MANAGER_EFFECT_ANIM_JOINTS`).
pub const YOSHI_EGG_LAY_KEY: (u32, u32) = (339, 0x0960);
pub const YOSHI_EGG_LAY_ANIM_JOINTS: [u32; 2] =
    crate::d_arrays::EF_MANAGER_YOSHI_EGG_LAY_ANIM_JOINTS;
/// The `AnimDesc::EFFECT` slot of the Wait table; Break follows.
pub const YOSHI_EGG_LAY_ANIM_SLOT: u32 = 0x104;

/// `dEFManagerPikachuThunderShockEffectDesc`'s tree in file 347
/// (`llPikachuSpecial2ThunderShockDObjDesc`). Its `EFDesc` carries spark
/// animation 0 (`...ThunderShock0AnimJoint` at 0x1720);
/// `efManagerPikachuThunderShockMakeEffect` adds 1 or 2 in its place:
/// `...ThunderShock1AnimJoint` and `...ThunderShock2AnimJoint`.
pub const PIKACHU_THUNDER_SHOCK_KEY: (u32, u32) = (347, 0x1640);
pub const PIKACHU_THUNDER_SHOCK_ANIM_JOINTS: [u32; 2] = [0x1850, 0x1970];
/// The `AnimDesc::EFFECT` slot of animation 1; 2 follows.
pub const PIKACHU_THUNDER_SHOCK_ANIM_SLOT: u32 = 0x106;
/// Node 3's one `MObj` script under each spark animation
/// (`llPikachuSpecial2ThunderShock{0,1,2}MatAnimJoint` at 0x1A80, 0x1AC0
/// and 0x1B10, through their inner lists at 0x1ABC, 0x1B04 and 0x1B54),
/// as file 347 offsets. The pack binds 0's.
pub const PIKACHU_THUNDER_SHOCK_MAT_SCRIPTS: [u32; 3] = [0x1A90, 0x1AD0, 0x1B20];

/// `dEFManagerFoxReflectorEffectDesc`'s tree in file 346
/// (`llFoxSpecial2ReflectorDObjDesc`), whose `EFDesc` carries the Start
/// table. `efManagerFoxReflectorSetAnimID` adds index 1 to 3 of
/// `dEFManagerFoxReflectorAnimJointOffsets` in its place: Loop, Hit, End.
pub const FOX_REFLECTOR_KEY: (u32, u32) = (346, 0x02B0);
pub const FOX_REFLECTOR_ANIM_JOINTS: [u32; 3] = {
    let table = crate::d_arrays::EF_MANAGER_FOX_REFLECTOR_ANIM_JOINT_OFFSETS;
    [table[1], table[2], table[3]]
};
/// The `AnimDesc::EFFECT` slot of the Loop table; Hit and End follow.
pub const FOX_REFLECTOR_ANIM_SLOT: u32 = 0x108;

/// The transform tables a maker adds to a manager effect's tree in place
/// of its `EFDesc`'s own: `(first AnimDesc::EFFECT slot, object key,
/// tables)`, one slot per table.
pub const VARIANT_ANIMS: [(u32, (u32, u32), &[u32]); 3] = [
    (
        YOSHI_EGG_LAY_ANIM_SLOT,
        YOSHI_EGG_LAY_KEY,
        &YOSHI_EGG_LAY_ANIM_JOINTS,
    ),
    (
        PIKACHU_THUNDER_SHOCK_ANIM_SLOT,
        PIKACHU_THUNDER_SHOCK_KEY,
        &PIKACHU_THUNDER_SHOCK_ANIM_JOINTS,
    ),
    (
        FOX_REFLECTOR_ANIM_SLOT,
        FOX_REFLECTOR_KEY,
        &FOX_REFLECTOR_ANIM_JOINTS,
    ),
];

/// One [`ENTRY_ANIMS`] row.
pub type EntryAnim = (u32, (u32, u32), u32, Option<u32>, &'static [(u32, u32)]);

/// The entry animations the makers add in place of, or beside, an
/// `EFDesc`'s own table (RE-425), each an `AnimDesc::EFFECT` slot from
/// [`ENTRY_ANIM_SLOT`]: `(slot, object key, animation file, AnimJoint
/// table, (script, node) singles)`.
///
/// * `efManagerFoxEntryArwingMakeEffect` plays file 346's
///   `llFoxSpecial2EntryArwingRAnimJoint` (0x9E0) or `...LAnimJoint`
///   (0x590) on file 161's Arwing tree (0x2C30) from its first node.
/// * `efManagerCaptainEntryCarMakeEffect` plays
///   `llCaptainSpecial2_6200_AnimJoint` on the car (350 + 0x5FC0) and
///   gives nodes 3, 5, 7 and 9 `llCaptainSpecial2_6518_AnimJoint` and 4, 6,
///   8 and 10 `..._6598_...`.
/// * `efManagerMBallThrownMakeEffect` plays
///   `llITCommonDataMBallThrownRAnimJoint` (86 + 0x9690) facing right; the
///   left table is the `EFDesc`'s ([`MANAGER_EFFECT_ANIM_JOINTS`]).
/// * `efManagerKirbyEntryStarMakeEffect` plays
///   `llKirbySpecial2EntryStarRAnimJoint` (348 + 0x1EA0) facing right.
pub const ENTRY_ANIMS: [EntryAnim; 5] = [
    (
        ENTRY_ARWING_R_ANIM_SLOT,
        (161, 0x2C30),
        346,
        Some(0x09E0),
        &[],
    ),
    (
        ENTRY_ARWING_R_ANIM_SLOT + 1,
        (161, 0x2C30),
        346,
        Some(0x0590),
        &[],
    ),
    (
        ENTRY_CAR_ANIM_SLOT,
        (350, 0x5FC0),
        350,
        Some(0x6200),
        &[
            (0x6518, 3),
            (0x6598, 4),
            (0x6518, 5),
            (0x6598, 6),
            (0x6518, 7),
            (0x6598, 8),
            (0x6518, 9),
            (0x6598, 10),
        ],
    ),
    (ENTRY_BALL_R_ANIM_SLOT, (86, 0x9430), 86, Some(0x9690), &[]),
    (
        ENTRY_KIRBY_STAR_R_ANIM_SLOT,
        (348, 0x1DA8),
        348,
        Some(0x1EA0),
        &[],
    ),
];
/// The car's nodes `efManagerCaptainEntryCarMakeEffect` gives a
/// `nGCMatrixKindRecalcRotRpyRSca` (kind 44) matrix after their own: its
/// exhaust sprites face the camera (RE-425).
pub const ENTRY_CAR_KEY: (u32, u32) = (350, 0x5FC0);
pub const ENTRY_CAR_BILLBOARD_NODES: [u32; 4] = [3, 5, 7, 9];

/// The first [`ENTRY_ANIMS`] slot, past the Egg Lay's.
pub const ENTRY_ANIM_SLOT: u32 = 0x110;
/// The Arwing's rightward table; the leftward one follows.
pub const ENTRY_ARWING_R_ANIM_SLOT: u32 = ENTRY_ANIM_SLOT;
pub const ENTRY_CAR_ANIM_SLOT: u32 = ENTRY_ANIM_SLOT + 2;
pub const ENTRY_BALL_R_ANIM_SLOT: u32 = ENTRY_ANIM_SLOT + 3;
pub const ENTRY_KIRBY_STAR_R_ANIM_SLOT: u32 = ENTRY_ANIM_SLOT + 4;

/// File 35, `FTEmblemModels` (`llFTEmblemModelsFileID`): the series
/// emblems `mnVSResultsMakeEmblem` and `mnCharactersMakeEmblem` make.
pub const EMBLEM_FILE: u32 = 35;

/// One series emblem's `llFTEmblemModels*` offsets
/// (`include/reloc_data.us.h`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Emblem {
    pub dobjdesc: u32,
    pub mobjsub: u32,
    /// `AObjEvent32 ***`: one script per `MObj`. Each steps `Light1Color`
    /// and `Light2Color` one frame per colour: red, blue, yellow, green,
    /// then the CPU's grey.
    pub matanim_joint: u32,
}

/// The ten series emblems: Mario, Fox, Donkey, Metroid, Zelda, Yoshi,
/// FZero, Kirby, PMonsters and Mother.
pub const EMBLEMS: [Emblem; 10] = emblems();

/// [`EMBLEMS`] from `mnVSResultsMakeEmblem`'s `dobjdescs`, `mobjsubs` and
/// `matanim_joints`, by fighter kind: each series at its first fighter.
const fn emblems() -> [Emblem; 10] {
    let mut out = [Emblem {
        dobjdesc: 0,
        mobjsub: 0,
        matanim_joint: 0,
    }; 10];
    let mut f = EMBLEM_BY_FIGHTER.len();
    while f > 0 {
        f -= 1;
        out[EMBLEM_BY_FIGHTER[f] as usize] = Emblem {
            dobjdesc: crate::d_arrays::MN_VS_RESULTS_MAKE_EMBLEM_DOBJDESCS[f],
            mobjsub: crate::d_arrays::MN_VS_RESULTS_MAKE_EMBLEM_MOBJSUBS[f],
            matanim_joint: crate::d_arrays::MN_VS_RESULTS_MAKE_EMBLEM_MATANIM_JOINTS[f],
        };
    }
    out
}

/// `mnVSResultsMakeEmblem`'s `dobjdescs[fkind]`, as an [`EMBLEMS`] index,
/// for the twelve playable kinds: each distinct `DObjDesc` in order of its
/// first fighter (Luigi shares Mario's, Jigglypuff Pikachu's).
pub const EMBLEM_BY_FIGHTER: [u8; 12] = emblem_by_fighter();

const fn emblem_by_fighter() -> [u8; 12] {
    let d = crate::d_arrays::MN_VS_RESULTS_MAKE_EMBLEM_DOBJDESCS;
    let mut out = [0u8; 12];
    let mut series = 0u8;
    let mut f = 0;
    while f < out.len() {
        let mut g = 0;
        while g < f && d[g] != d[f] {
            g += 1;
        }
        out[f] = if g < f {
            out[g]
        } else {
            series += 1;
            series - 1
        };
        f += 1;
    }
    out
}

// The 46 manager effects the port draws, from `dEFManager<Name>EffectDesc`
// (`ef/efmanager.c`), generated at build time from the user's ROM by
// `crates/ssb-tablegen` (never committed):
//
// * `MANAGER_EFFECT_KEYS`: `(file, o_dobjdesc)` of each desc; the file is
//   the one its `file_head` variable holds.
// * `MANAGER_EFFECT_ANIM_JOINTS`: `o_anim_joint` of each desc. A missing
//   value means that descriptor has no DObj transform animation;
//   material-only animation is tracked separately. Poke Ball uses the
//   left-facing table selected by `efManagerMBallThrownMakeEffect`; its
//   right-facing sibling is in [`ENTRY_ANIMS`].
// * `MANAGER_EFFECT_MAT_ANIM_JOINTS`: `o_matanim_joint` of each desc,
//   aligned with the keys. These are `AObjEvent32 ***` tables: one outer
//   entry per DObj and one inner script pointer per MObj in that node.
//   The manager selects the player's `DeadExplode` table at runtime
//   (`dEFManagerDeadExplodeMatAnimJoints`); player 0's is listed. The
//   alternate player/facing tables remain gameplay selection work.
// * `slot::NAME`: each effect's index in the three tables.
include!(concat!(env!("OUT_DIR"), "/effect_tables.rs"));

/// Effects whose authored rest pose is intentionally invisible until their
/// AObjEvent32 runtime starts. The ROM gives Ness PK Flash scales X/Y of
/// `1e-5`, Samus's entry point scale Y of `1e-5`, and Link's spin-attack
/// material primitive alpha of zero at frame zero.
pub const MANAGER_EFFECT_REST_INVISIBLE_KEYS: &[(u32, u32)] =
    &[(84, 0x6D00), (349, 0x0B90), (353, 0x11C0)];

/// Effects whose `o_matanim_joint` in [`MANAGER_EFFECT_MAT_ANIM_JOINTS`] is
/// real and non-`NULL`, but whose only script(s) never attach to a live
/// primitive because the *source* `gcAddMatAnimJointAll` walk itself never
/// reaches them — not a converter gap. RE-178/RE-179 traced each directly:
///
/// * ImpactWave (file 83 @ 0x7C28): the one real script
///   (`aobjEvent32SetVal0RateBlock`, file 83 @ 0x7DA4) only ever writes the
///   identity UV transform (`TraU`/`TraV`/`ScaU`/`ScaV` = 0, 0, 1.0, 1.0) —
///   it drives none of `PaletteID`/`TextureIDCurrent`/`TextureIDNext`/colour,
///   the only tracks a sprite/palette resolver reads, and even a UV-transform
///   consumer (which nothing in this renderer implements) would render it
///   identically to not running the script at all.
///
/// RE-425 removed MBallThrown (file 86 @ 0x9430): RE-178 read the table at
/// 0x950C, which `llITCommonDataMBallMatAnimJoint` names for the item, not
/// the thrown effect; `efManagerMBallThrownMakeEffect` sets
/// `llITCommonDataMBallThrownLMatAnimJoint` (0x9740) or `...R...` (0x9810),
/// whose node-3 script cycles the closed ball's eight textures.
pub const MANAGER_EFFECT_MAT_ANIM_UNREACHABLE_KEYS: &[(u32, u32)] = &[(83, 0x7C28)];

#[cfg(test)]
mod tests {
    use super::{
        MANAGER_EFFECT_ANIM_JOINTS, MANAGER_EFFECT_KEYS, MANAGER_EFFECT_MAT_ANIM_JOINTS,
        MANAGER_EFFECT_MAT_ANIM_UNREACHABLE_KEYS, MANAGER_EFFECT_REST_INVISIBLE_KEYS,
    };
    use alloc::collections::BTreeSet;

    #[test]
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
    fn manager_effect_keys_are_46_unique_objects() {
        let unique: BTreeSet<_> = MANAGER_EFFECT_KEYS.iter().copied().collect();
        assert_eq!(MANAGER_EFFECT_KEYS.len(), 46);
        assert_eq!(unique.len(), MANAGER_EFFECT_KEYS.len());
        assert_eq!(MANAGER_EFFECT_ANIM_JOINTS.len(), MANAGER_EFFECT_KEYS.len());
        assert_eq!(
            MANAGER_EFFECT_MAT_ANIM_JOINTS.len(),
            MANAGER_EFFECT_KEYS.len()
        );
        assert_eq!(
            MANAGER_EFFECT_ANIM_JOINTS
                .iter()
                .filter(|anim| anim.is_some())
                .count(),
            35
        );
        assert_eq!(
            MANAGER_EFFECT_MAT_ANIM_JOINTS
                .iter()
                .filter(|anim| anim.is_some())
                .count(),
            26
        );
        assert!(MANAGER_EFFECT_REST_INVISIBLE_KEYS
            .iter()
            .all(|key| unique.contains(key)));

        let has_mat_anim: BTreeSet<_> = MANAGER_EFFECT_KEYS
            .iter()
            .zip(MANAGER_EFFECT_MAT_ANIM_JOINTS)
            .filter_map(|(key, mat)| mat.is_some().then_some(*key))
            .collect();
        let unreachable: BTreeSet<_> = MANAGER_EFFECT_MAT_ANIM_UNREACHABLE_KEYS
            .iter()
            .copied()
            .collect();
        assert_eq!(MANAGER_EFFECT_MAT_ANIM_UNREACHABLE_KEYS.len(), 1);
        assert_eq!(
            unreachable.len(),
            MANAGER_EFFECT_MAT_ANIM_UNREACHABLE_KEYS.len()
        );
        // Each is a real, non-`NULL` `o_matanim_joint` (otherwise it would
        // already be excluded from `mat_animated` for an unrelated reason,
        // and this list would be documenting nothing).
        assert!(unreachable.is_subset(&has_mat_anim));
    }
}
