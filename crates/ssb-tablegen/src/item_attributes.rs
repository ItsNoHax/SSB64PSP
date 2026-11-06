//! The items' `ITAttributes` (`ittypes.h`), for `ssb_game::item`: every
//! field gameplay reads, and `spin_speed`.
//!
//! The records are [`item_sounds::ITEMS`](crate::item_sounds::ITEMS), each
//! an `ll*ItemAttributes` symbol of `include/reloc_data.us.h`. The struct is
//! 0x48 bytes; IDO packs its bitfields MSB first into 32-bit units, and a
//! 16-bit field starts on a halfword:
//!
//! | Offset | Contents |
//! |---|---|
//! | 0x10 | bits 31..27: `is_display_xlu`, `is_item_dobjs`, `is_display_colanim`, `is_give_hitlag`, `weight` |
//! | 0x12 | `attack_offset0_{x,y,z}`, `attack_offset1_{x,y,z}`: six `s16` |
//! | 0x1E | `damage_coll_offset`, `damage_coll_size`: `Vec3h` each |
//! | 0x2A | `map_coll_{top,center,bottom,width}`: four `s16` |
//! | 0x32 | `size`: `u16` |
//! | 0x34 | `angle:10` (signed), `knockback_scale:10`, `damage:8`, `element:4` |
//! | 0x38 | `knockback_weight:10`, `shield_damage:8` (signed), `attack_count:2`, `can_setoff:1`, `hit_sfx:10` |
//! | 0x3C | `priority:3`, `can_rehit_item`, `can_rehit_fighter`, `can_hop`, `can_reflect`, `can_shield`, `knockback_base:10`, `type:4`, `hitstatus:4`, two unused bits |
//! | 0x40 | `drop_sfx:10`, `throw_sfx:10`, `smash_sfx:10` |
//! | 0x44 | `vel_scale:9`; `spin_speed`: `u16` at 0x46 |
//!
//! Each record also carries its sounds ([`item_sounds`]).

use alloc::vec::Vec;

use crate::item_sounds::{item_sounds, ITEMS, IT_HIT_WORD, IT_SFX_WORD};
use crate::source::{be_u16, be_u32};
use crate::tables::{le_bytes, Ctx};
use crate::{Error, Result};

/// `ITAttackEvent[4]` tables (`ll*AttackEvents`, `reloc_data.us.h`): the
/// constant's name, file and offset.
pub const ATTACK_EVENTS: [(&str, u32, u32); 8] = [
    ("CAPSULE", 251, 0x98),
    ("MSBOMB", 251, 0x404),
    ("BOMBHEI", 251, 0x46C),
    ("BOX", 251, 0x614),
    ("TARU", 251, 0x67C),
    ("EGG", 251, 0xB14),
    ("LINK_BOMB", 225, 0x88),
    ("TARUBOMB", 0x127, 0xF0),
];
/// Events per table.
pub const ATTACK_EVENT_COUNT: usize = 4;
/// `sizeof(ITAttackEvent)`: `timer:8`, `angle:10` (signed), `damage:8` in
/// the first word, MSB first; `size` the next halfword.
const ATTACK_EVENT_SIZE: usize = 8;
/// `llITCommonDataContainerVelocitiesY` (one per `ITKind` up to the
/// containers'): file, offset, count.
pub const CONTAINER_VELOCITIES_Y: (u32, usize, usize) = (251, 0x0, 20);
/// `llLinkMainBombBloatScales`: file, offset, count.
pub const LINK_BOMB_BLOAT_SCALES: (u32, usize, usize) = (225, 0xA8, 6);

/// `sizeof(ITAttributes)`.
pub const SIZE: usize = 0x48;
const FLAGS: usize = 0x10;
const ATTACK_OFFSETS: usize = 0x12;
const DAMAGE_COLL_OFFSET: usize = 0x1E;
const DAMAGE_COLL_SIZE: usize = 0x24;
const MAP_COLL: usize = 0x2A;
const HIT_SIZE: usize = 0x32;
const ATTACK_WORD: usize = 0x34;
const KNOCKBACK_WORD: usize = 0x38;
const FLAGS_WORD: usize = 0x3C;
const VEL_SCALE_WORD: usize = 0x44;
const SPIN_SPEED: usize = 0x46;

/// `nGMHitElement*`, as `ssb_game::combat::Element` names them.
pub const ELEMENTS: [&str; 7] = [
    "Normal", "Fire", "Electric", "Slash", "Coin", "Freezing", "Sleep",
];
/// `nITType*`, as `ssb_game::item::ItemType` names them.
pub const ITEM_TYPES: [&str; 7] = [
    "Damage", "Swing", "Shoot", "Throw", "Touch", "Consume", "Fighter",
];
/// `nGMHitStatus*`, as `ssb_game::combat::HitStatus` names them.
pub const HIT_STATUSES: [&str; 4] = ["None", "Normal", "Invincible", "Intangible"];

/// A field's index in `names`, or an error naming the record.
pub fn name(names: &[&'static str], v: u32, file: u32, at: usize) -> Result<u32> {
    if (v as usize) < names.len() {
        Ok(v)
    } else {
        Err(Error::BadRelocation { file, offset: at })
    }
}

/// `TableId::ItemAttributes`' words per item, in this order:
///
/// | Word | Field |
/// |---|---|
/// | 0, 1, 2 | `is_give_hitlag`, `is_display_colanim`, `weight` is light |
/// | 3..9 | `attack_offsets` (two `Vec3`, integers) |
/// | 9..12, 12..15 | `damage_coll_offset`, `damage_coll_size` |
/// | 15..19 | `map_coll` `top, center, bottom, width` |
/// | 19..23 | `size`, `angle`, `kb_scale`, `damage` |
/// | 23 | `element` ([`ELEMENTS`] index) |
/// | 24..28 | `kb_weight`, `shield_damage`, `attack_count`, `can_setoff` |
/// | 28..34 | `priority`, `can_rehit_item`, `can_rehit_fighter`, `can_hop`, `can_reflect`, `can_shield` |
/// | 34, 35, 36 | `kb_base`, `ty` ([`ITEM_TYPES`] index), `hitstatus` ([`HIT_STATUSES`] index) |
/// | 37 | `vel_scale` |
/// | 38..42 | sounds: `hit, drop, throw, smash` |
/// | 42 | `spin_speed` |
pub const WORDS: usize = 43;

/// One record's [`WORDS`].
fn record(data: &[u8], file: u32, base: usize) -> Result<[i32; WORDS]> {
    if base + SIZE > data.len() {
        return Err(Error::BadRelocation { file, offset: base });
    }
    let word = |at: usize| be_u32(data, base + at);
    let half = |at: usize| be_u16(data, base + at).map(|h| i32::from(h as i16));
    let bit = |w: u32, b: u32| i32::from(w & (1 << b) != 0);
    let flags = word(FLAGS)?;
    let a = word(ATTACK_WORD)?;
    let k = word(KNOCKBACK_WORD)?;
    let f = word(FLAGS_WORD)?;
    let mut w = [0i32; WORDS];
    w[0] = bit(flags, 28);
    w[1] = bit(flags, 29);
    w[2] = bit(flags, 27);
    for i in 0..6 {
        w[3 + i] = half(ATTACK_OFFSETS + 2 * i)?;
    }
    for i in 0..3 {
        w[9 + i] = half(DAMAGE_COLL_OFFSET + 2 * i)?;
        w[12 + i] = half(DAMAGE_COLL_SIZE + 2 * i)?;
    }
    for i in 0..4 {
        w[15 + i] = half(MAP_COLL + 2 * i)?;
    }
    w[19] = be_u16(data, base + HIT_SIZE)?.into();
    w[20] = (a as i32) >> 22;
    w[21] = ((a >> 12) & 0x3FF) as i32;
    w[22] = ((a >> 4) & 0xFF) as i32;
    w[23] = name(&ELEMENTS, a & 0xF, file, base + ATTACK_WORD)? as i32;
    w[24] = (k >> 22) as i32;
    w[25] = ((k << 10) as i32) >> 24;
    w[26] = ((k >> 12) & 3) as i32;
    w[27] = bit(k, 11);
    w[28] = (f >> 29) as i32;
    for (i, b) in [28, 27, 26, 25, 24].into_iter().enumerate() {
        w[29 + i] = bit(f, b);
    }
    w[34] = ((f >> 14) & 0x3FF) as i32;
    w[35] = name(&ITEM_TYPES, (f >> 10) & 0xF, file, base + FLAGS_WORD)? as i32;
    w[36] = name(&HIT_STATUSES, (f >> 6) & 0xF, file, base + FLAGS_WORD)? as i32;
    w[37] = (word(VEL_SCALE_WORD)? >> 23) as i32;
    let sounds = item_sounds(word(IT_HIT_WORD)?, word(IT_SFX_WORD)?);
    for (i, s) in sounds.into_iter().enumerate() {
        w[38 + i] = s.into();
    }
    w[42] = be_u16(data, base + SPIN_SPEED)?.into();
    Ok(w)
}

/// `TableId::ItemAttributes`: [`WORDS`] per [`ITEMS`] record.
pub fn attributes_bytes(c: &Ctx) -> Result<Vec<u8>> {
    let mut out = Vec::with_capacity(ITEMS.len() * WORDS);
    for (_, file, at) in ITEMS {
        out.extend(record(&c.rom.file(file)?.data, file, at as usize)?);
    }
    Ok(le_bytes(&out, i32::to_le_bytes))
}

/// One `ITAttackEvent` as `[timer, angle, damage, size]`.
fn attack_event(data: &[u8], at: usize) -> Result<[i32; 4]> {
    let w = be_u32(data, at)?;
    let size = be_u16(data, at + 4)?;
    Ok([
        (w >> 24) as i32,
        ((w << 8) as i32) >> 22,
        ((w >> 6) & 0xFF) as i32,
        size.into(),
    ])
}

/// `TableId::ItemAttackEvents`: [`ATTACK_EVENTS`]' events, `[timer, angle,
/// damage, size]` each.
pub fn attack_events_bytes(c: &Ctx) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    for (_, file, at) in ATTACK_EVENTS {
        let data = c.rom.file(file)?.data;
        for i in 0..ATTACK_EVENT_COUNT {
            out.extend(attack_event(&data, at as usize + i * ATTACK_EVENT_SIZE)?);
        }
    }
    Ok(le_bytes(&out, i32::to_le_bytes))
}

fn floats(c: &Ctx, (file, at, count): (u32, usize, usize)) -> Result<Vec<u8>> {
    let data = c.rom.file(file)?.data;
    let f: Vec<f32> = (0..count)
        .map(|i| be_u32(&data, at + 4 * i).map(f32::from_bits))
        .collect::<Result<_>>()?;
    Ok(le_bytes(&f, f32::to_le_bytes))
}

/// `TableId::ContainerVelocitiesY`.
pub fn container_velocities_bytes(c: &Ctx) -> Result<Vec<u8>> {
    floats(c, CONTAINER_VELOCITIES_Y)
}

/// `TableId::LinkBombBloatScales`.
pub fn bloat_scales_bytes(c: &Ctx) -> Result<Vec<u8>> {
    floats(c, LINK_BOMB_BLOAT_SCALES)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attack_events_decode_msb_first() {
        // timer 4, angle -30, damage 20, size 150.
        let w: u32 = (4 << 24) | (((-30i32 as u32) & 0x3FF) << 14) | (20 << 6);
        let mut d = w.to_be_bytes().to_vec();
        d.extend_from_slice(&150u16.to_be_bytes());
        d.extend_from_slice(&[0, 0]);
        assert_eq!(attack_event(&d, 0).unwrap(), [4, -30, 20, 150]);
    }

    #[test]
    fn fields_decode_msb_first() {
        let mut d = vec![0u8; SIZE];
        // Light, hitlag, colanim; attack offset 1 y = 350; damage offset
        // y = 200; size 300.
        d[FLAGS] = 0b0011_1000;
        d[ATTACK_OFFSETS + 8..ATTACK_OFFSETS + 10].copy_from_slice(&350i16.to_be_bytes());
        d[DAMAGE_COLL_OFFSET + 2..DAMAGE_COLL_OFFSET + 4].copy_from_slice(&200i16.to_be_bytes());
        d[HIT_SIZE..HIT_SIZE + 2].copy_from_slice(&300u16.to_be_bytes());
        // angle 361, kb_scale 80, damage 10, element Slash.
        let a: u32 = (361 << 22) | (80 << 12) | (10 << 4) | 3;
        d[ATTACK_WORD..ATTACK_WORD + 4].copy_from_slice(&a.to_be_bytes());
        // kb_weight 400, shield_damage -8, 2 attacks, can_setoff.
        let k: u32 = (400 << 22) | ((0xF8 & 0xFF) << 14) | (2 << 12) | (1 << 11);
        d[KNOCKBACK_WORD..KNOCKBACK_WORD + 4].copy_from_slice(&k.to_be_bytes());
        // priority 1, rehit item, shield, kb_base 60, Consume, Normal.
        let f: u32 = (1 << 29) | (1 << 28) | (1 << 24) | (60 << 14) | (5 << 10) | (1 << 6);
        d[FLAGS_WORD..FLAGS_WORD + 4].copy_from_slice(&f.to_be_bytes());
        d[VEL_SCALE_WORD..VEL_SCALE_WORD + 4].copy_from_slice(&(110u32 << 23 | 120).to_be_bytes());
        let w = record(&d, 0, 0).unwrap();
        assert_eq!(w[..3], [1, 1, 1]);
        assert_eq!(w[3..9], [0, 0, 0, 0, 350, 0]);
        assert_eq!(w[9..12], [0, 200, 0]);
        assert_eq!(w[19..24], [300, 361, 80, 10, 3]);
        assert_eq!(w[24..28], [400, -8, 2, 1]);
        assert_eq!(w[28..34], [1, 1, 0, 0, 0, 1]);
        assert_eq!(w[34..38], [60, 5, 1, 110]);
        assert_eq!(w[42], 120);
    }

    #[test]
    fn out_of_range_names_are_errors() {
        let mut d = vec![0u8; SIZE];
        d[ATTACK_WORD + 3] = 7;
        assert!(record(&d, 0, 0).is_err());
    }
}
