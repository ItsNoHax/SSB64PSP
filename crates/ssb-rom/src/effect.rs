//! Source-backed identities for the original effect manager's display assets.
//!
//! `ef/efmanager.c` has 53 static `EFDesc` records. Three are controller-only
//! and four pairs share one display asset, leaving these 46 unique objects.
//! The host pack verifier and the PSP audit build consume the same keys so an
//! exhaustive visual run cannot silently drift from the source inventory.

pub const MANAGER_EFFECT_KEYS: &[(u32, u32)] = &[
    (83, 0x7750),
    (83, 0x7E80),
    (83, 0x7C28),
    (83, 0x8FA0),
    (83, 0xCAC8),
    (84, 0x1500),
    (84, 0x2040),
    (84, 0x2760),
    (84, 0x3398),
    (84, 0x53E8),
    (84, 0x6D00),
    (85, 0x0628),
    (85, 0x2AC0),
    (85, 0x3170),
    (163, 0x0300),
    (346, 0x02B0),
    (338, 0xA860),
    (347, 0x0800),
    (347, 0x1640),
    (341, 0x95B0),
    (342, 0x2258),
    (348, 0x0B20),
    (348, 0x0DF8),
    (348, 0x12E8),
    (348, 0x1DA8),
    (348, 0x2390),
    (348, 0x2888),
    (349, 0x0380),
    (349, 0x0B90),
    (350, 0x0B08),
    (350, 0x5FC0),
    (333, 0x0760),
    (351, 0x2130),
    (352, 0x09A8),
    (335, 0x9050),
    (335, 0x9A10),
    (353, 0x03F8),
    (353, 0x07B8),
    (353, 0x11C0),
    (86, 0x9430),
    (86, 0x5458),
    (354, 0x0530),
    (339, 0x0960),
    (355, 0x07C8),
    (356, 0x0608),
    (161, 0x2C30),
];

/// `o_anim_joint` from the source `EFDesc` corresponding to each entry in
/// [`MANAGER_EFFECT_KEYS`]. A missing value means that descriptor has no DObj
/// transform animation; material-only animation is tracked separately.
///
/// Poké Ball uses the left-facing table selected by
/// `efManagerMBallThrownMakeEffect`; its right-facing sibling is the same
/// effect variant and will be added when facing-dependent spawning exists.
pub const MANAGER_EFFECT_ANIM_JOINTS: &[Option<u32>] = &[
    Some(0x7800),
    Some(0x7F40),
    Some(0x7D40),
    Some(0x9050),
    Some(0xCAE0),
    None,
    Some(0x20D0),
    Some(0x28A0),
    Some(0x34A0),
    Some(0x54D0),
    Some(0x6D90),
    Some(0x0710),
    Some(0x2B70),
    Some(0x32B0),
    None,
    Some(0x0340),
    None,
    Some(0x0890),
    Some(0x1720),
    None,
    Some(0x22E0),
    None,
    Some(0x13F0),
    Some(0x1470),
    Some(0x1E30),
    Some(0x24D0),
    None,
    Some(0x0410),
    Some(0x0C20),
    Some(0x0B90),
    None,
    None,
    Some(0x2270),
    Some(0x0A30),
    None,
    Some(0x9AC0),
    Some(0x0840),
    Some(0x0B60),
    Some(0x1250),
    Some(0x95E0),
    None,
    Some(0x0600),
    Some(0x0B90),
    Some(0x0850),
    Some(0x06C0),
    None,
];

/// `o_matanim_joint` from the same source descriptors, aligned with
/// [`MANAGER_EFFECT_KEYS`]. These are `AObjEvent32 ***` tables: one outer
/// entry per DObj and one inner script pointer per MObj in that node.
///
/// The manager selects player 0's `DeadExplode1` table at runtime and the
/// left-facing Poké Ball table for the descriptor-selected audit variant.
/// The alternate player/facing tables remain gameplay selection work, just
/// like the transform variants documented beside
/// [`MANAGER_EFFECT_ANIM_JOINTS`].
pub const MANAGER_EFFECT_MAT_ANIM_JOINTS: &[Option<u32>] = &[
    Some(0x7860),
    None,
    Some(0x7DA0),
    Some(0x90C0),
    Some(0xCB40),
    Some(0x1570),
    Some(0x2170),
    Some(0x2AB0),
    Some(0x35A0),
    Some(0x58E0),
    Some(0x6E20),
    Some(0x0860),
    None,
    Some(0x3490),
    None,
    None,
    None,
    // PikachuUnk: RE-178 corrects a former 0x0890 here, which is actually
    // `dPikachuSpecial2_UnkAnimJoint_AnimJoint` — the *transform* joint table
    // `MANAGER_EFFECT_ANIM_JOINTS` already names correctly above. The real
    // `UnkMatAnimJoint` table is 0x70 bytes further, at 0x900.
    Some(0x0900),
    Some(0x1A80),
    None,
    Some(0x2350),
    None,
    None,
    None,
    None,
    None,
    None,
    Some(0x0480),
    None,
    Some(0x0C00),
    None,
    Some(0x0830),
    Some(0x2D70),
    Some(0x0AD0),
    None,
    Some(0x9BB0),
    Some(0x0B90),
    Some(0x0BF0),
    Some(0x12F0),
    Some(0x950C),
    None,
    Some(0x0780),
    None,
    None,
    None,
    None,
];

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
/// * MBallThrown (file 86 @ 0x9430): its own `DObjDesc` array
///   (`dITCommonObject_MBall_Item_data_DObjDesc[5]`) has exactly 4 real nodes
///   before the `DOBJ_ARRAY_MAX` (18) terminator sentinel. The real
///   `gcAddMatAnimJointAll` walk (`objanim.c`) advances its `p_matanim_joints`
///   cursor once per real `DObj` and only dereferences it *before* each
///   advance, so a 4-node tree only ever reads table slots 0-3 — all `NULL`
///   here (confirmed against the ROM: `dITCommonObject_MBall_Item_data_
///   remainder_gap_0x950C[5] = { NULL, NULL, NULL, NULL, <script> }`). Slot 4,
///   the only populated one, sits one past what this specific tree ever
///   visits; nothing in the traced source calls this table by raw index
///   either, so its content — plausibly authored for a different item
///   sharing this same common `ITCommonObject` file — is genuinely
///   unreachable for this effect, not a missed attachment.
pub const MANAGER_EFFECT_MAT_ANIM_UNREACHABLE_KEYS: &[(u32, u32)] = &[(83, 0x7C28), (86, 0x9430)];

#[cfg(test)]
mod tests {
    use super::{
        MANAGER_EFFECT_ANIM_JOINTS, MANAGER_EFFECT_KEYS, MANAGER_EFFECT_MAT_ANIM_JOINTS,
        MANAGER_EFFECT_MAT_ANIM_UNREACHABLE_KEYS, MANAGER_EFFECT_REST_INVISIBLE_KEYS,
    };
    use alloc::collections::BTreeSet;

    #[test]
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
        assert_eq!(MANAGER_EFFECT_MAT_ANIM_UNREACHABLE_KEYS.len(), 2);
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
