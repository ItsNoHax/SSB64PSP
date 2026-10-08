//! The FGM ids in the items' `ITAttributes` (`hit_sfx`, `drop_sfx`,
//! `throw_sfx`, `smash_sfx`), the weapons' `WPAttributes` (`sfx`) and the
//! two Saffron `ITMonsterEvent` tables (`fgm_id`), for
//! `ssb_game::item_sounds`.
//!
//! Every record's file and offset is its `ll*ItemAttributes`,
//! `ll*WeaponAttributes` or `ll*HitParties` symbol in
//! `include/reloc_data.us.h`; the file ids are its `ll*FileID` values.

use std::fmt::Write;

use crate::emit;
use crate::source::be_u32;
use crate::{Result, Source};

/// `ITAttributes` records: constant name, archive file, offset.
pub const ITEMS: [(&str, u32, u32); 45] = [
    ("CAPSULE", 251, 0x50),
    ("TOMATO", 251, 0xB8),
    ("HEART", 251, 0x100),
    ("STAR", 251, 0x148),
    ("SWORD", 251, 0x190),
    ("BAT", 251, 0x1D8),
    ("HARISEN", 251, 0x220),
    ("LGUN", 251, 0x268),
    ("FFLOWER", 251, 0x2E4),
    ("HAMMER", 251, 0x374),
    ("MSBOMB", 251, 0x3BC),
    ("BOMBHEI", 251, 0x424),
    ("STARROD", 251, 0x48C),
    ("GSHELL", 251, 0x53C),
    ("RSHELL", 251, 0x584),
    ("BOX", 251, 0x5CC),
    ("TARU", 251, 0x634),
    ("NBUMPER", 251, 0x69C),
    ("MBALL", 251, 0x6E4),
    ("WARK", 251, 0x72C),
    ("KABIGON", 251, 0x7A8),
    ("TOSAKINTO", 251, 0x7F0),
    ("MEW", 251, 0x838),
    ("NYARS", 251, 0x880),
    ("LIZARDON", 251, 0x8FC),
    ("SPEAR", 251, 0x98C),
    ("KAMEX", 251, 0xA08),
    ("MLUCKY", 251, 0xA84),
    ("EGG", 251, 0xACC),
    ("STARMIE", 251, 0xB34),
    ("SAWAMURA", 251, 0xBB0),
    ("DOGAS", 251, 0xBF8),
    ("PIPPI", 251, 0xC74),
    ("GBUMPER", 251, 0xCF0),
    // `llLinkMainBombItemAttributes`.
    ("LINK_BOMB", 225, 0x40),
    // `llNessSpecial1PKFireItemAttributes`.
    ("PK_FIRE", 240, 0x34),
    ("PAKKUN", 260, 0x120),
    ("POWER_BLOCK", 260, 0xD8),
    ("GLUCKY", 264, 0xBC),
    ("MARUMINE", 264, 0x104),
    ("PORYGON", 264, 0x16C),
    ("HITOKAGE", 264, 0x1FC),
    ("FUSHIGIBANA", 264, 0x278),
    ("TARUBOMB", 295, 0xA8),
    // `dITTargetItemDesc`: offset 0 of `gSC1PBonusStageItemFile`
    // (`253_ITBonus1ObjectHeader`).
    ("TARGET", 253, 0x0),
];

/// `WPAttributes` records: constant name, archive file, offset.
pub const WEAPONS: [(&str, u32, u32); 35] = [
    ("MARIO_FIREBALL", 204, 0x0),
    ("LUIGI_FIREBALL", 222, 0x0),
    ("FOX_BLASTER", 210, 0x0),
    ("SAMUS_BOMB", 217, 0xC),
    ("SAMUS_CHARGE_SHOT", 218, 0x0),
    ("LINK_SPIN_ATTACK", 225, 0xC),
    ("LINK_BOOMERANG", 226, 0x0),
    ("KIRBY_CUTTER", 229, 0x8),
    ("NESS_PK_THUNDER", 239, 0xC),
    ("NESS_PK_THUNDER_TRAIL", 239, 0x40),
    ("NESS_PK_FIRE", 240, 0x0),
    ("PIKACHU_THUNDER_HEAD", 243, 0xC),
    ("PIKACHU_THUNDER_TRAIL", 243, 0x40),
    ("PIKACHU_THUNDER_JOLT_AIR", 244, 0x0),
    ("PIKACHU_THUNDER_JOLT_GROUND", 244, 0x34),
    ("YOSHI_EGG_THROW", 247, 0xC),
    ("YOSHI_STAR", 247, 0x40),
    ("BOSS_BULLET_NORMAL", 249, 0x774),
    ("BOSS_BULLET_HARD", 249, 0x7A8),
    ("LGUN_AMMO", 251, 0x2B0),
    ("FFLOWER_FLAME", 251, 0x32C),
    ("STARROD", 251, 0x4D4),
    ("STARROD_SMASH", 251, 0x508),
    ("WARK_ROCK", 251, 0x774),
    ("NYARS_COIN", 251, 0x8C8),
    ("LIZARDON_FLAME", 251, 0x944),
    ("SPEAR_SWARM", 251, 0x9D4),
    ("KAMEX_HYDRO", 251, 0xA50),
    ("STARMIE_SWIFT", 251, 0xB7C),
    ("DOGAS_SMOG", 251, 0xC40),
    ("PIPPI_SWARM", 251, 0xCBC),
    ("ARWING_LASER_2D", 262, 0xBC),
    ("ARWING_LASER_3D", 262, 0xF0),
    ("HITOKAGE_FLAME", 264, 0x244),
    ("FUSHIGIBANA_RAZOR", 264, 0x308),
];

/// `ITMonsterEvent[2]` tables: constant name, file, offset.
pub const MONSTER_EVENTS: [(&str, u32, u32); 2] = [
    ("PORYGON_HIT_PARTIES", 264, 0x1B4),
    ("FUSHIGIBANA_HIT_PARTIES", 264, 0x2C0),
];

/// Bytes per `ITMonsterEvent`; `fgm_id` is the halfword at 0x20.
pub const MONSTER_EVENT_SIZE: usize = 0x24;
const MONSTER_EVENT_FGM: usize = 0x20;

/// `ITAttributes`: the word holding `hit_sfx` (bits 21..30, MSB first, of
/// the unit after `knockback_weight`, `shield_damage`, `attack_count` and
/// `can_setoff`), and the word holding `drop_sfx`, `throw_sfx` and
/// `smash_sfx` (bits 0..9, 10..19, 20..29).
const IT_HIT_WORD: usize = 0x38;
const IT_SFX_WORD: usize = 0x40;
/// `WPAttributes`: `sfx` is bits 11..20 of the word at 0x2C, after
/// `shield_damage`, `attack_count` and `can_setoff`.
const WP_SFX_WORD: usize = 0x2C;

/// A 10-bit field starting at MSB-first bit `first` of `word`.
pub fn field10(word: u32, first: u32) -> u16 {
    ((word >> (32 - 10 - first)) & 0x3FF) as u16
}

/// `(hit_sfx, drop_sfx, throw_sfx, smash_sfx)` from an `ITAttributes`.
pub fn item_sounds(hit_word: u32, sfx_word: u32) -> [u16; 4] {
    [
        field10(hit_word, 21),
        field10(sfx_word, 0),
        field10(sfx_word, 10),
        field10(sfx_word, 20),
    ]
}

/// `sfx` from a `WPAttributes`.
pub fn weapon_sound(word: u32) -> u16 {
    field10(word, 11)
}

pub fn generate(rom: Option<&Source>) -> Result<String> {
    let mut w = emit::header("the FGM ids of the items' and weapons' attributes");
    w.push_str("pub mod item {\n    use super::ItemSounds;\n\n");
    for (name, file, at) in ITEMS {
        let [hit, drop, throw, smash] = match rom {
            Some(rom) => {
                let data = rom.file(file)?.data;
                let at = at as usize;
                item_sounds(
                    be_u32(&data, at + IT_HIT_WORD)?,
                    be_u32(&data, at + IT_SFX_WORD)?,
                )
            }
            None => [0; 4],
        };
        let _ = writeln!(
            w,
            "    pub const {name}: ItemSounds = ItemSounds {{ hit: {hit}, drop: {drop}, throw: {throw}, smash: {smash} }};"
        );
    }
    w.push_str("}\n\npub mod weapon {\n");
    for (name, file, at) in WEAPONS {
        let sfx = match rom {
            Some(rom) => weapon_sound(be_u32(&rom.file(file)?.data, at as usize + WP_SFX_WORD)?),
            None => 0,
        };
        let _ = writeln!(w, "    pub const {name}: u16 = {sfx};");
    }
    w.push_str("}\n\npub mod monster_event {\n");
    for (name, file, at) in MONSTER_EVENTS {
        let mut ids = [0u16; 2];
        if let Some(rom) = rom {
            let data = rom.file(file)?.data;
            for (i, id) in ids.iter_mut().enumerate() {
                let off = at as usize + i * MONSTER_EVENT_SIZE + MONSTER_EVENT_FGM;
                *id = (be_u32(&data, off & !3)? >> 16) as u16;
            }
        }
        let _ = writeln!(w, "    pub const {name}: [u16; 2] = [{}, {}];", ids[0], ids[1]);
    }
    w.push_str("}\n");
    Ok(w)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ten_bit_fields_read_msb_first() {
        // `hit_sfx` = 0x2AB at bits 21..30 of an otherwise set word.
        let hit = !(0x3FF << 1) | (0x2AB << 1);
        let sfx = (0x111 << 22) | (0x222 << 12) | (0x333 << 2) | 3;
        assert_eq!(item_sounds(hit, sfx), [0x2AB, 0x111, 0x222, 0x333]);
        assert_eq!(weapon_sound(0x155 << 11), 0x155);
        assert_eq!(weapon_sound(!(0x3FF << 11)), 0);
    }

    #[test]
    fn records_do_not_overlap_within_a_file() {
        // `ITAttributes` is 0x48 bytes and `WPAttributes` 0x34.
        let mut spans: Vec<(u32, u32, u32)> = ITEMS
            .iter()
            .map(|&(_, f, at)| (f, at, at + 0x48))
            .chain(WEAPONS.iter().map(|&(_, f, at)| (f, at, at + 0x34)))
            .collect();
        spans.sort();
        spans.dedup();
        for pair in spans.windows(2) {
            if pair[0].0 == pair[1].0 {
                assert!(pair[0].2 <= pair[1].1, "{pair:?}");
            }
        }
    }
}
