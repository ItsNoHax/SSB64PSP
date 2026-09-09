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

/// Effects whose authored rest pose is intentionally invisible until their
/// AObjEvent32 runtime starts. The ROM gives Ness PK Flash scales X/Y of
/// `1e-5`, Samus's entry point scale Y of `1e-5`, and Link's spin-attack
/// material primitive alpha of zero at frame zero.
pub const MANAGER_EFFECT_REST_INVISIBLE_KEYS: &[(u32, u32)] =
    &[(84, 0x6D00), (349, 0x0B90), (353, 0x11C0)];

#[cfg(test)]
mod tests {
    use super::{MANAGER_EFFECT_KEYS, MANAGER_EFFECT_REST_INVISIBLE_KEYS};
    use alloc::collections::BTreeSet;

    #[test]
    fn manager_effect_keys_are_46_unique_objects() {
        let unique: BTreeSet<_> = MANAGER_EFFECT_KEYS.iter().copied().collect();
        assert_eq!(MANAGER_EFFECT_KEYS.len(), 46);
        assert_eq!(unique.len(), MANAGER_EFFECT_KEYS.len());
        assert!(MANAGER_EFFECT_REST_INVISIBLE_KEYS
            .iter()
            .all(|key| unique.contains(key)));
    }
}
