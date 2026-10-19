//! The item-made weapons' `WPAttributes` (`wptypes.h`) and the Smog's
//! per-play scale, for `ssb_game::monster_weapon`.
//!
//! Each record is a [`item_sounds::WEAPONS`](crate::item_sounds::WEAPONS)
//! entry (`ll*WeaponAttributes`, `include/reloc_data.us.h`). The struct is
//! 0x34 bytes, bitfields MSB first in 32-bit units:
//!
//! | Offset | Contents |
//! |---|---|
//! | 0x10 | `attack_offsets[2]`: two `Vec3h` |
//! | 0x1C | `map_coll_{top,center,bottom,width}`: four `s16` |
//! | 0x24 | `size:16`, `angle:10` (signed) |
//! | 0x28 | `knockback_scale:10`, `damage:8`, `element:4`, `knockback_weight:10` |
//! | 0x2C | `shield_damage:8` (signed), `attack_count:2`, `can_setoff:1`, `sfx:10`, `priority:3`, `can_rehit_item`, `can_rehit_fighter`, `can_hop`, `can_reflect`, `can_absorb`, `can_shield`, two unused bits |
//! | 0x30 | `knockback_base:10` |
//!
//! The Smog (`itdogas.c`) scales its attack by its child `DObj`'s X scale,
//! which its fixed `AnimJoint` script drives: the generator replays that
//! script with `ssb_rom::objanim` from the `DObjDesc`'s rest pose, as
//! `gcAddDObjAnimJoint` followed by one `gcPlayAnimAll` per frame does.

use std::fmt::Write;

use crate::figatree::JointPose;
use crate::item_attributes::{name, ELEMENTS};
use crate::item_sounds::WEAPONS;
use crate::objanim::{joint_scripts, StageJoint};
use crate::source::{be_u16, be_u32};
use crate::{emit, Error, Result, Source};

/// `ShotKind` order: the `item_sounds::WEAPONS` names of its records.
pub const SHOTS: [&str; 13] = [
    "HITOKAGE_FLAME",
    "FUSHIGIBANA_RAZOR",
    "WARK_ROCK",
    "NYARS_COIN",
    "LIZARDON_FLAME",
    "SPEAR_SWARM",
    "PIPPI_SWARM",
    "KAMEX_HYDRO",
    "STARMIE_SWIFT",
    "DOGAS_SMOG",
    "LGUN_AMMO",
    "STARROD",
    "FFLOWER_FLAME",
];

const ATTACK_OFFSETS: usize = 0x10;
const MAP_COLL: usize = 0x1C;
const SIZE_WORD: usize = 0x24;
const ATTACK_WORD: usize = 0x28;
const FLAGS_WORD: usize = 0x2C;
const KNOCKBACK_BASE_WORD: usize = 0x30;

/// `llITCommonObjectFileID`: the item models and their animations.
pub const SMOG_FILE: u32 = 86;
/// `dITCommonObject_DogasSmog_Weapon_data_DObjDesc[3]`.
pub const SMOG_DOBJ_DESCS: usize = 0x13100;
/// `dITCommonObject_DogasSmog_Weapon_animjoints[2]`.
pub const SMOG_ANIM_JOINTS: u32 = 0x13190;
/// The animated `DObj`: the second descriptor, whose X scale the Smog reads.
pub const SMOG_JOINT: usize = 1;
/// `sizeof(DObjDesc)`: `id`, `dl`, then `translate`, `rotate`, `scale`.
const DOBJ_DESC_SIZE: usize = 0x2C;
const DOBJ_DESC_TRANSLATE: usize = 0x8;
/// `ITDOGAS_SMOG_LIFETIME`: the plays the table covers.
pub const SMOG_PLAYS: usize = 30;

fn shot(data: &[u8], file: u32, base: usize) -> Result<String> {
    let word = |at: usize| be_u32(data, base + at);
    let bit = |w: u32, b: u32| w & (1 << b) != 0;
    let map: Vec<String> = (0..4)
        .map(|i| {
            Ok(emit::f32_lit(f32::from(
                be_u16(data, base + MAP_COLL + 2 * i)? as i16,
            )))
        })
        .collect::<Result<_>>()?;
    let s = word(SIZE_WORD)?;
    let a = word(ATTACK_WORD)?;
    let f = word(FLAGS_WORD)?;
    let element = name(&ELEMENTS, (a >> 10) & 0xF, file, base + ATTACK_WORD)?;
    let offsets: Vec<String> = (0..2)
        .map(|i| {
            let c: Vec<String> = (0..3)
                .map(|j| {
                    Ok(emit::f32_lit(f32::from(
                        be_u16(data, base + ATTACK_OFFSETS + 6 * i + 2 * j)? as i16,
                    )))
                })
                .collect::<Result<_>>()?;
            Ok(format!("[{}]", c.join(", ")))
        })
        .collect::<Result<_>>()?;
    let mut w = String::from("ShotAttributes {\n");
    let _ = writeln!(
        w,
        "        attack_offsets: [{}, {}],",
        offsets[0], offsets[1]
    );
    let _ = writeln!(
        w,
        "        map_coll: BodyColl {{ top: {}, center: {}, bottom: {}, width: {} }},",
        map[0], map[1], map[2], map[3]
    );
    let _ = writeln!(w, "        size: {},", emit::f32_lit((s >> 16) as f32));
    let _ = writeln!(w, "        angle: {},", ((s << 16) as i32) >> 22);
    let _ = writeln!(w, "        kb_scale: {},", a >> 22);
    let _ = writeln!(w, "        damage: {},", (a >> 14) & 0xFF);
    let _ = writeln!(w, "        element: Element::{element},");
    let _ = writeln!(w, "        kb_weight: {},", a & 0x3FF);
    let _ = writeln!(w, "        shield_damage: {},", (f as i32) >> 24);
    let _ = writeln!(w, "        can_setoff: {},", bit(f, 21));
    let _ = writeln!(w, "        can_rehit_fighter: {},", bit(f, 6));
    let _ = writeln!(w, "        can_rehit_item: {},", bit(f, 7));
    let _ = writeln!(w, "        can_hop: {},", bit(f, 5));
    let _ = writeln!(w, "        can_reflect: {},", bit(f, 4));
    let _ = writeln!(w, "        can_absorb: {},", bit(f, 3));
    let _ = writeln!(w, "        can_shield: {},", bit(f, 2));
    let _ = writeln!(w, "        kb_base: {},", word(KNOCKBACK_BASE_WORD)? >> 22);
    w.push_str("    }");
    Ok(w)
}

const STUB_SHOT: &str = "ShotAttributes {\n\
    \x20       attack_offsets: [[0.0, 0.0, 0.0], [0.0, 0.0, 0.0]],\n\
    \x20       map_coll: BodyColl { top: 0.0, center: 0.0, bottom: 0.0, width: 0.0 },\n\
    \x20       size: 0.0,\n\
    \x20       angle: 0,\n\
    \x20       kb_scale: 0,\n\
    \x20       damage: 0,\n\
    \x20       element: Element::Normal,\n\
    \x20       kb_weight: 0,\n\
    \x20       shield_damage: 0,\n\
    \x20       can_setoff: false,\n\
    \x20       can_rehit_fighter: false,\n\
    \x20       can_rehit_item: false,\n\
    \x20       can_hop: false,\n\
    \x20       can_reflect: false,\n\
    \x20       can_absorb: false,\n\
    \x20       can_shield: false,\n\
    \x20       kb_base: 0,\n    }";

/// The Smog `DObj`'s X scale after each of its first [`SMOG_PLAYS`] plays.
fn smog_scale(data: &[u8]) -> Result<[f32; SMOG_PLAYS]> {
    let bad = |offset| Error::BadRelocation {
        file: SMOG_FILE,
        offset,
    };
    let script = joint_scripts(data, SMOG_ANIM_JOINTS, SMOG_JOINT + 1)[SMOG_JOINT]
        .ok_or(bad(SMOG_ANIM_JOINTS as usize))?;
    let desc = SMOG_DOBJ_DESCS + SMOG_JOINT * DOBJ_DESC_SIZE + DOBJ_DESC_TRANSLATE;
    let v = |i: usize| -> Result<f32> { Ok(f32::from_bits(be_u32(data, desc + 4 * i)?)) };
    let mut pose = JointPose {
        translate: [v(0)?, v(1)?, v(2)?],
        rotate: [v(3)?, v(4)?, v(5)?],
        scale: [v(6)?, v(7)?, v(8)?],
    };
    let mut joint = StageJoint::start_changed(script, 0.0);
    let mut out = [0.0; SMOG_PLAYS];
    for x in out.iter_mut() {
        joint
            .tick(data, 1.0, &mut pose)
            .map_err(|_| bad(script as usize))?;
        *x = pose.scale[0];
    }
    Ok(out)
}

pub fn generate(rom: Option<&Source>) -> Result<String> {
    let mut w = emit::header("the item weapons' attributes (`WPAttributes`) and the Smog's scale");
    w.push_str(
        "/// Per `ShotKind`: its `ll*WeaponAttributes` record.\n\
         #[rustfmt::skip]\n",
    );
    let _ = writeln!(
        w,
        "pub static SHOT_ATTRIBUTES: [ShotAttributes; {}] = [",
        SHOTS.len()
    );
    for shot_name in SHOTS {
        let &(_, file, at) = WEAPONS
            .iter()
            .find(|(n, _, _)| *n == shot_name)
            .expect("every shot is a WEAPONS record");
        let body = match rom {
            Some(rom) => shot(&rom.file(file)?.data, file, at as usize)?,
            None => STUB_SHOT.to_string(),
        };
        let _ = writeln!(w, "    // {shot_name}\n    {body},");
    }
    w.push_str("];\n\n");
    w.push_str(
        "/// Every `ll*WeaponAttributes` record (`item_sounds::WEAPONS`), the\n\
         /// fighters', the boss's and the stages' weapons too, by its name there.\n\
         #[rustfmt::skip]\n\
         pub mod weapon {\n    use super::*;\n\n",
    );
    for &(weapon_name, file, at) in WEAPONS.iter() {
        let body = match rom {
            Some(rom) => shot(&rom.file(file)?.data, file, at as usize)?,
            None => STUB_SHOT.to_string(),
        };
        let _ = writeln!(w, "    pub const {weapon_name}: ShotAttributes = {body};");
    }
    w.push_str("}\n\n");
    let scale = match rom {
        Some(rom) => smog_scale(&rom.file(SMOG_FILE)?.data)?,
        None => [0.0; SMOG_PLAYS],
    };
    w.push_str(
        "/// The Smog's child `DObj` X scale after each play of\n\
         /// `dITCommonObject_DogasSmog_Weapon_animjoints` (the make frame's first).\n",
    );
    let cells: Vec<String> = scale.iter().map(|&x| emit::f32_lit(x)).collect();
    let _ = writeln!(
        w,
        "pub const SMOG_SCALE: [f32; {SMOG_PLAYS}] = [{}];",
        cells.join(", ")
    );
    Ok(w)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stub_tables_have_every_shot() {
        let text = generate(None).unwrap();
        assert_eq!(
            text.matches("ShotAttributes {\n").count(),
            SHOTS.len() + WEAPONS.len()
        );
        assert!(text.contains("pub const SMOG_SCALE: [f32; 30]"));
    }

    #[test]
    fn fields_decode_msb_first() {
        let mut d = vec![0u8; 0x34];
        d[ATTACK_OFFSETS + 2..ATTACK_OFFSETS + 4].copy_from_slice(&(-100i16).to_be_bytes());
        d[MAP_COLL..MAP_COLL + 2].copy_from_slice(&10i16.to_be_bytes());
        d[MAP_COLL + 2..MAP_COLL + 4].copy_from_slice(&(-10i16).to_be_bytes());
        // size 200, angle -5.
        let s: u32 = (200 << 16) | ((-5i32 as u32 & 0x3FF) << 6);
        d[SIZE_WORD..SIZE_WORD + 4].copy_from_slice(&s.to_be_bytes());
        // kb_scale 40, damage 10, Electric, kb_weight 60.
        let a: u32 = (40 << 22) | (10 << 14) | (2 << 10) | 60;
        d[ATTACK_WORD..ATTACK_WORD + 4].copy_from_slice(&a.to_be_bytes());
        // shield_damage -8, can_setoff, rehit item, absorb.
        let f: u32 = (0xF8 << 24) | (1 << 21) | (1 << 7) | (1 << 3);
        d[FLAGS_WORD..FLAGS_WORD + 4].copy_from_slice(&f.to_be_bytes());
        d[KNOCKBACK_BASE_WORD..KNOCKBACK_BASE_WORD + 4]
            .copy_from_slice(&(50u32 << 22).to_be_bytes());
        let text = shot(&d, 0, 0).unwrap();
        for want in [
            "attack_offsets: [[0.0, -100.0, 0.0], [0.0, 0.0, 0.0]]",
            "top: 10.0, center: -10.0",
            "size: 200.0",
            "angle: -5",
            "kb_scale: 40",
            "damage: 10",
            "element: Element::Electric",
            "kb_weight: 60",
            "shield_damage: -8",
            "can_setoff: true",
            "can_rehit_fighter: false",
            "can_rehit_item: true",
            "can_absorb: true",
            "can_shield: false",
            "kb_base: 50",
        ] {
            assert!(text.contains(want), "{want} in\n{text}");
        }
    }
}
