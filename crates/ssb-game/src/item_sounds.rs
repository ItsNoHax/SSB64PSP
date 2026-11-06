//! The FGM ids the items' and weapons' attribute records carry:
//! `ITAttributes::{hit_sfx, drop_sfx, throw_sfx, smash_sfx}`,
//! `WPAttributes::sfx` and Saffron's `ITMonsterEvent::fgm_id`.
//!
//! They are ROM data from the pack's tables (never committed; zeros in a
//! stub build): the items' in their `ItemAttributes`
//! (`crate::item::attributes`), the weapons' and the events' here
//! (`TableId::WeaponSounds`, `TableId::MonsterEventSounds`), named after the
//! decomp's `ll*WeaponAttributes` and `ll*HitParties` symbols.

/// An `ITAttributes`' four sound fields.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ItemSounds {
    /// `hit_sfx`: copied to `attack_coll.fgm_id` by `itManagerMakeItem`.
    pub hit: u16,
    /// `drop_sfx` (`itMainSetFighterDrop`).
    pub drop: u16,
    /// `throw_sfx` (`itMainSetFighterThrow`).
    pub throw: u16,
    /// `smash_sfx` (`itMainSetFighterThrow`, smash throws).
    pub smash: u16,
}

impl ItemSounds {
    /// No sounds: a template that every record built from it overrides.
    pub const NONE: ItemSounds = ItemSounds {
        hit: 0,
        drop: 0,
        throw: 0,
        smash: 0,
    };
}

/// `WPAttributes::sfx` of every `ll*WeaponAttributes` record, in
/// `ssb_tablegen`'s `item_sounds::WEAPONS` order. Constant addresses: a hit
/// reads one.
pub mod weapon {
    macro_rules! sounds {
        ($i:expr;) => {};
        ($i:expr; $name:ident $(, $rest:ident)*) => {
            pub static $name: ssb_tables::DArr<u16> =
                ssb_tables::DArr::new(ssb_tables::TableId::WeaponSounds, 2 * ($i));
            sounds!($i + 1; $($rest),*);
        };
    }
    sounds!(0; MARIO_FIREBALL, LUIGI_FIREBALL, FOX_BLASTER, SAMUS_BOMB, SAMUS_CHARGE_SHOT,
        LINK_SPIN_ATTACK, LINK_BOOMERANG, KIRBY_CUTTER, NESS_PK_THUNDER, NESS_PK_THUNDER_TRAIL,
        NESS_PK_FIRE, PIKACHU_THUNDER_HEAD, PIKACHU_THUNDER_TRAIL, PIKACHU_THUNDER_JOLT_AIR,
        PIKACHU_THUNDER_JOLT_GROUND, YOSHI_EGG_THROW, YOSHI_STAR, BOSS_BULLET_NORMAL,
        BOSS_BULLET_HARD, LGUN_AMMO, FFLOWER_FLAME, STARROD, STARROD_SMASH, WARK_ROCK,
        NYARS_COIN, LIZARDON_FLAME, SPEAR_SWARM, KAMEX_HYDRO, STARMIE_SWIFT, DOGAS_SMOG,
        PIPPI_SWARM, ARWING_LASER_2D, ARWING_LASER_3D, HITOKAGE_FLAME, FUSHIGIBANA_RAZOR);
    const _: () = assert!(ssb_tables::TableId::WeaponSounds.size() == 2 * 35);
}

/// Both `ITMonsterEvent::fgm_id`s of each Saffron `ll*HitParties` table.
pub mod monster_event {
    pub static PORYGON_HIT_PARTIES: ssb_tables::DArr<[u16; 2]> =
        ssb_tables::DArr::new(ssb_tables::TableId::MonsterEventSounds, 0);
    pub static FUSHIGIBANA_HIT_PARTIES: ssb_tables::DArr<[u16; 2]> =
        ssb_tables::DArr::new(ssb_tables::TableId::MonsterEventSounds, 4);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::item::attributes as item;
    use crate::sound::id::*;

    /// Spot checks against `src/relocData/*.c`'s initialisers.
    #[test]
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
    fn generated_ids_match_the_reloc_data_initialisers() {
        assert_eq!(
            item::BOMBHEI.sounds,
            ItemSounds {
                hit: nSYAudioFGMExplodeL,
                drop: nSYAudioFGMItemThrow,
                throw: nSYAudioFGMItemThrow,
                smash: nSYAudioFGMItemThrow,
            }
        );
        assert_eq!(item::HARISEN.sounds.hit, nSYAudioFGMHarisenHit);
        assert_eq!(item::CAPSULE.sounds.hit, nSYAudioFGMPunchL);
        assert_eq!(item::TARGET.sounds.hit, nSYAudioFGMBumperHit);
        assert_eq!(*weapon::MARIO_FIREBALL, 28);
        assert_eq!(*weapon::LUIGI_FIREBALL, 28);
        assert_eq!(
            *monster_event::PORYGON_HIT_PARTIES,
            [nSYAudioFGMPunchL, nSYAudioFGMPunchL]
        );
        assert_eq!(
            *monster_event::FUSHIGIBANA_HIT_PARTIES,
            [nSYAudioFGMPunchL, nSYAudioFGMPunchL]
        );
    }
}
