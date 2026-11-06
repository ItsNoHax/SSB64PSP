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

use alloc::vec::Vec;

use crate::item_attributes::{name, ELEMENTS};
use crate::item_sounds::WEAPONS;
use crate::source::{be_u16, be_u32};
use crate::tables::{le_bytes, Ctx};
use crate::Result;

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

/// `TableId::WeaponAttributes`' words per weapon, in this order:
/// `attack_offsets` (2 × 3), `map_coll` (`top, center, bottom, width`),
/// `size, angle, kb_scale, damage, element` ([`ELEMENTS`] index),
/// `kb_weight, shield_damage`, then `can_setoff, can_rehit_fighter,
/// can_rehit_item, can_hop, can_reflect, can_absorb, can_shield`, `kb_base`.
pub const WORDS: usize = 25;

fn shot(data: &[u8], file: u32, base: usize) -> Result<[i32; WORDS]> {
    let word = |at: usize| be_u32(data, base + at);
    let half = |at: usize| be_u16(data, base + at).map(|h| i32::from(h as i16));
    let bit = |w: u32, b: u32| i32::from(w & (1 << b) != 0);
    let s = word(SIZE_WORD)?;
    let a = word(ATTACK_WORD)?;
    let f = word(FLAGS_WORD)?;
    let mut w = [0i32; WORDS];
    for (i, v) in w[..6].iter_mut().enumerate() {
        *v = half(ATTACK_OFFSETS + 2 * i)?;
    }
    for i in 0..4 {
        w[6 + i] = half(MAP_COLL + 2 * i)?;
    }
    w[10] = (s >> 16) as i32;
    w[11] = ((s << 16) as i32) >> 22;
    w[12] = (a >> 22) as i32;
    w[13] = ((a >> 14) & 0xFF) as i32;
    w[14] = name(&ELEMENTS, (a >> 10) & 0xF, file, base + ATTACK_WORD)? as i32;
    w[15] = (a & 0x3FF) as i32;
    w[16] = (f as i32) >> 24;
    for (i, b) in [21, 6, 7, 5, 4, 3, 2].into_iter().enumerate() {
        w[17 + i] = bit(f, b);
    }
    w[24] = (word(KNOCKBACK_BASE_WORD)? >> 22) as i32;
    Ok(w)
}

/// `TableId::WeaponAttributes`: [`WORDS`] per [`WEAPONS`] record.
pub fn attributes_bytes(c: &Ctx) -> Result<Vec<u8>> {
    let mut out = Vec::with_capacity(WEAPONS.len() * WORDS);
    for (_, file, at) in WEAPONS {
        out.extend(shot(&c.rom.file(file)?.data, file, at as usize)?);
    }
    Ok(le_bytes(&out, i32::to_le_bytes))
}

/// The Smog `DObj`'s X scale after each of its first [`SMOG_PLAYS`] plays.
fn smog_scale(data: &[u8]) -> Result<[f32; SMOG_PLAYS]> {
    use crate::figatree::JointPose;
    use crate::objanim::{joint_scripts, StageJoint};
    let bad = |offset| crate::Error::BadRelocation {
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

/// `TableId::SmogScale`: the Smog's child `DObj` X scale after each play of
/// `dITCommonObject_DogasSmog_Weapon_animjoints` (the make frame's first).
pub fn smog_scale_bytes(c: &Ctx) -> Result<Vec<u8>> {
    Ok(le_bytes(
        &smog_scale(&c.rom.file(SMOG_FILE)?.data)?,
        f32::to_le_bytes,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let w = shot(&d, 0, 0).unwrap();
        assert_eq!(w[..10], [0, -100, 0, 0, 0, 0, 10, -10, 0, 0]);
        assert_eq!(w[10..17], [200, -5, 40, 10, 2, 60, -8]);
        assert_eq!(w[17..], [1, 0, 1, 0, 0, 1, 0, 50]);
    }
}
