//! Which manager effects and weapons the N64 draws entirely on task head 1
//! (`gSYTaskmanDLHeads[1]`), read from their descs' display callbacks and
//! render flags rather than listed by name. A camera pass runs every head-0
//! list before any head-1 list, so a whole object on head 1 draws after
//! every opaque object of its camera (RE-422). The converter marks those
//! objects' lists `head1` (`mesh::SequenceItem::stream`).

use crate::effect::{MANAGER_EFFECT_DISPLAYS, MANAGER_EFFECT_DL_LINKS, MANAGER_EFFECT_KEYS};

// `WEAPON_DESC_FLAGS`: each `ll*WeaponAttributes` record's
// `(attributes file, attributes offset, WPDesc.flags)`, generated at build
// time from the user's ROM by `crates/ssb-tablegen` (never committed).
include!(concat!(env!("OUT_DIR"), "/weapon_tables.rs"));

/// `lbCommonDObjScaleXProcDisplay` (`lb/lbcommon.c:2067`): walks the whole
/// tree with `lbCommonDrawDObjScaleX`, which puts every node's matrix and
/// list on head 1 (`lbcommon.c:2024-2064`).
pub const LB_COMMON_DOBJ_SCALE_X_PROC_DISPLAY: u32 = 0x800C_B4B0;
/// `efManagerYoshiShieldProcDisplay` (`ef/efmanager.c:4148`): sets ENV on
/// head 1, then `gcDrawDObjDLHead1` and `efDisplayCLDProcDisplay`.
pub const EF_MANAGER_YOSHI_SHIELD_PROC_DISPLAY: u32 = 0x8010_1180;
/// The `EFDesc.proc_display`s that draw the effect's object on head 1
/// alone. Every other manager effect's display either draws on head 0
/// (`gcDrawDObjTreeForGObj`, `efManagerImpactWaveProcDisplay`) or draws
/// `DObjDLLink` lists, whose `list_id` names the head.
const WHOLE_HEAD1_EFFECT_DISPLAYS: [u32; 2] = [
    LB_COMMON_DOBJ_SCALE_X_PROC_DISPLAY,
    EF_MANAGER_YOSHI_SHIELD_PROC_DISPLAY,
];

/// The DL links `efDisplayMakeCLD` gives `efDisplayCLDProcDisplay` at
/// priority 3 (`ef/efdisplay.c:28-32`): head 1's state for the
/// priority-2 effects drawn on them (RE-421).
pub const CLD_LINKS: [u8; 2] = [15, 18];

/// A whole-head-1 manager effect: its object and whether its link starts
/// head 1 under `efDisplayCLDProcDisplay`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Head1Effect {
    pub key: (u32, u32),
    pub cld: bool,
}

/// Every manager effect whose display draws its object on head 1 alone.
pub fn whole_head1_effects() -> impl Iterator<Item = Head1Effect> {
    MANAGER_EFFECT_KEYS
        .iter()
        .zip(MANAGER_EFFECT_DISPLAYS)
        .zip(MANAGER_EFFECT_DL_LINKS)
        .filter(|((_, display), _)| WHOLE_HEAD1_EFFECT_DISPLAYS.contains(display))
        .map(|((&key, _), link)| Head1Effect {
            key,
            cld: CLD_LINKS.contains(link),
        })
}

/// `WEAPON_FLAG_DOBJLINKS` (`wp/wpdef.h:15`): `wpManagerMakeWeapon` draws
/// the weapon with a `DObjDLLink` walker, whose `list_id` names the head;
/// without it the weapon gets `wpDisplayDLHead1` or `func_ovl3_80167618`
/// (`lbCommonDObjScaleXProcDisplay`), both head 1 (`wp/wpmanager.c:
/// 259-270`, `wp/wpdisplay.c:172-188`).
pub const WEAPON_FLAG_DOBJLINKS: u8 = 0x02;

/// Weapons whose maker replaces the display `wpManagerMakeWeapon` chose
/// with one that draws on head 0: Clefairy's swarm
/// (`llITCommonDataPippiSwarmWeaponAttributes`) gets
/// `itPippiWeaponSwarmProcDisplay` (`it/itmonster/itspear.c:408`), whose
/// `itPippiWeaponSwarmRenderSwarm` sets `G_RM_AA_XLU_SURF` on head 0 and
/// draws with `gcDrawDObjTreeForGObj` (`itspear.c:359-368`). The other two
/// replacements keep head 1: `wpYoshiEggThrowProcDisplay` calls
/// `wpDisplayDLHead1` (`wpyoshieggthrow.c:245-252`), and
/// `wpDisplayPKThunderProcDisplay` draws link lists (`wpnesspkthunder.c:431`).
pub const HEAD0_DISPLAY_WEAPONS: [(u32, u32); 1] = [(251, 0xCBC)];

/// The whole-head-1 manager effect drawing the object at `place`, if any.
pub fn whole_head1_effect(place: (u32, u32)) -> Option<Head1Effect> {
    whole_head1_effects().find(|e| e.key == place)
}

/// The objects (`WPAttributes.data`: a display list, or a `DObjDesc`
/// tree with `WEAPON_FLAG_DOBJDESC`) of every weapon drawn on head 1
/// alone, as `(file, offset)`. Several records can name one object.
pub fn whole_head1_weapons() -> impl Iterator<Item = (u32, u32)> {
    WEAPON_DESC_FLAGS
        .iter()
        .filter(|&&(file, at, flags, _)| {
            flags & WEAPON_FLAG_DOBJLINKS == 0 && !HEAD0_DISPLAY_WEAPONS.contains(&(file, at))
        })
        .filter_map(|&(.., data)| data)
}

/// Whether the object at `place` is a whole-head-1 weapon's or manager
/// effect's, and if so the state head 1 starts it under.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Head1Object {
    /// A weapon: `wpDisplayDrawNormal` resets head 1 itself (RE-379).
    Weapon,
    /// A manager effect; `cld` when its link starts head 1 under
    /// `efDisplayCLDProcDisplay`, else the camera's head-1 default.
    Effect { cld: bool },
}

/// The whole-head-1 object at `place`, if any. A weapon's object that a
/// manager effect also draws (Yoshi's egg, the Star Rod's star) is a
/// `Weapon`: both draw it on head 1.
pub fn whole_head1_object(place: (u32, u32)) -> Option<Head1Object> {
    if whole_head1_weapons().any(|p| p == place) {
        return Some(Head1Object::Weapon);
    }
    whole_head1_effects()
        .find(|e| e.key == place)
        .map(|e| Head1Object::Effect { cld: e.cld })
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;

    /// The effects `lbCommonDObjScaleXProcDisplay` and
    /// `efManagerYoshiShieldProcDisplay` draw (`ef/efmanager.c`):
    /// DamageFlyOrbs, DamageFlySparks, ShockSmall, FireSpark,
    /// CaptureKirbyStar, CaptainFalconPunch (link 15, `:790-811`) and
    /// YoshiEggEscape on links 15/18; YoshiEntryEgg on link 10
    /// (`:1312-1333`).
    #[test]
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
    fn whole_head1_effects_are_the_scale_x_and_yoshi_shield_effects() {
        let mut got: Vec<_> = whole_head1_effects().map(|e| (e.key, e.cld)).collect();
        got.sort_unstable();
        assert_eq!(
            got,
            [
                ((83, 0x7E80), true),
                ((83, 0x8FA0), true),
                ((84, 0x1500), true),
                ((84, 0x2040), true),
                ((86, 0x5458), true),
                ((333, 0x0760), true),
                ((338, 0xA860), true),
                ((354, 0x0530), false),
            ]
        );
    }

    /// Flags 0x00 or 0x01 in the decomp's `WPDesc`s, less Clefairy's swarm;
    /// the Fire Flower's, Charizard's and Charmander's flames have no object.
    #[test]
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
    fn whole_head1_weapons_are_the_unlinked_ones() {
        let mut got: Vec<_> = whole_head1_weapons().collect();
        got.sort_unstable();
        got.dedup();
        assert_eq!(
            got,
            [
                (86, 0x40A8),
                (86, 0x5458),
                (86, 0xAB98),
                (86, 0xC520),
                (86, 0xE4A8),
                (86, 0xF9D8),
                (153, 0x1C50),
                (297, 0x1A8),
                (316, 0x40),
                (320, 0xE0D8),
                (321, 0x270),
                (325, 0x610),
                (336, 0x168),
                (338, 0xA860),
                (342, 0x270),
                (344, 0x2CB8),
            ]
        );
        assert_eq!(whole_head1_object((86, 0x13598)), None);
        assert_eq!(
            whole_head1_object((354, 0x530)),
            Some(Head1Object::Effect { cld: false })
        );
        assert_eq!(whole_head1_object((338, 0xA860)), Some(Head1Object::Weapon));
    }
}
