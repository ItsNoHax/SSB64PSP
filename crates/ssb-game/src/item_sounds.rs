//! The FGM ids the items' and weapons' attribute records carry:
//! `ITAttributes::{hit_sfx, drop_sfx, throw_sfx, smash_sfx}`,
//! `WPAttributes::sfx` and Saffron's `ITMonsterEvent::fgm_id`.
//!
//! They are ROM data, generated at build time from the user's ROM by
//! `crates/ssb-tablegen` (never committed; zeros in a stub build). The
//! constants are named after the decomp's `ll*ItemAttributes` and
//! `ll*WeaponAttributes` symbols.

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

#[allow(clippy::all)]
mod generated {
    use super::ItemSounds;
    include!(concat!(env!("OUT_DIR"), "/item_sounds.rs"));
}
pub use generated::{item, monster_event, weapon};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sound::id::*;

    /// Spot checks against `src/relocData/*.c`'s initialisers.
    #[test]
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
    fn generated_ids_match_the_reloc_data_initialisers() {
        assert_eq!(
            item::BOMBHEI,
            ItemSounds {
                hit: nSYAudioFGMExplodeL,
                drop: nSYAudioFGMItemThrow,
                throw: nSYAudioFGMItemThrow,
                smash: nSYAudioFGMItemThrow,
            }
        );
        assert_eq!(item::HARISEN.hit, nSYAudioFGMHarisenHit);
        assert_eq!(item::CAPSULE.hit, nSYAudioFGMPunchL);
        assert_eq!(item::TARGET.hit, nSYAudioFGMBumperHit);
        assert_eq!(weapon::MARIO_FIREBALL, 28);
        assert_eq!(weapon::LUIGI_FIREBALL, 28);
        assert_eq!(
            monster_event::PORYGON_HIT_PARTIES,
            [nSYAudioFGMPunchL, nSYAudioFGMPunchL]
        );
        assert_eq!(
            monster_event::FUSHIGIBANA_HIT_PARTIES,
            [nSYAudioFGMPunchL, nSYAudioFGMPunchL]
        );
    }
}
