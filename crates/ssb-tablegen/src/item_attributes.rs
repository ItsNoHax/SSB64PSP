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
//! The emitted records take their sounds from `crate::item_sounds::item`.

use std::fmt::Write;

use crate::item_sounds::ITEMS;
use crate::source::{be_u16, be_u32};
use crate::{emit, Error, Result, Source};

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
const ATTACK_EVENT_COUNT: usize = 4;
/// `sizeof(ITAttackEvent)`: `timer:8`, `angle:10` (signed), `damage:8` in
/// the first word, MSB first; `size` the next halfword.
const ATTACK_EVENT_SIZE: usize = 8;
/// `f32` arrays: the constant's name, its symbol, file, offset and count.
/// `llITCommonDataContainerVelocitiesY` has one per `ITKind` up to the
/// containers'.
const FLOAT_TABLES: [(&str, &str, u32, usize, usize); 2] = [
    (
        "CONTAINER_VELOCITIES_Y",
        "llITCommonDataContainerVelocitiesY",
        251,
        0x0,
        20,
    ),
    (
        "LINK_BOMB_BLOAT_SCALES",
        "llLinkMainBombBloatScales",
        225,
        0xA8,
        6,
    ),
];

/// One `ITAttackEvent` as `(timer, angle, damage, size)`.
fn attack_event(data: &[u8], at: usize) -> Result<String> {
    let w = be_u32(data, at)?;
    let size = be_u16(data, at + 4)?;
    let angle = ((w << 8) as i32) >> 22;
    Ok(format!(
        "({}, {angle}, {}, {})",
        w >> 24,
        (w >> 6) & 0xFF,
        emit::f32_lit(f32::from(size))
    ))
}

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
const ITEM_TYPES: [&str; 7] = [
    "Damage", "Swing", "Shoot", "Throw", "Touch", "Consume", "Fighter",
];
/// `nGMHitStatus*`, as `ssb_game::combat::HitStatus` names them.
const HIT_STATUSES: [&str; 4] = ["None", "Normal", "Invincible", "Intangible"];

/// A field's name in `names`, or an error naming the record.
pub fn name(names: &[&'static str], v: u32, file: u32, at: usize) -> Result<&'static str> {
    names
        .get(v as usize)
        .copied()
        .ok_or(Error::BadRelocation { file, offset: at })
}

/// `count` big-endian `s16`s from `at` as `f32` literals.
fn halves(data: &[u8], at: usize, count: usize) -> Result<Vec<String>> {
    (0..count)
        .map(|i| Ok(emit::f32_lit(f32::from(be_u16(data, at + 2 * i)? as i16))))
        .collect()
}

fn vec3(c: &[String]) -> String {
    format!("Vec3::new({}, {}, {})", c[0], c[1], c[2])
}

/// One record's `ItemAttributes` literal (without `sounds`) and its
/// `spin_speed`.
fn record(data: &[u8], file: u32, base: usize) -> Result<(String, u16)> {
    if base + SIZE > data.len() {
        return Err(Error::BadRelocation { file, offset: base });
    }
    let word = |at: usize| be_u32(data, base + at);
    let bit = |w: u32, b: u32| w & (1 << b) != 0;
    let flags = word(FLAGS)?;
    let attack = halves(data, base + ATTACK_OFFSETS, 6)?;
    let offset = halves(data, base + DAMAGE_COLL_OFFSET, 3)?;
    let size = halves(data, base + DAMAGE_COLL_SIZE, 3)?;
    let map = halves(data, base + MAP_COLL, 4)?;
    let hit_size = be_u16(data, base + HIT_SIZE)?;
    let a = word(ATTACK_WORD)?;
    let k = word(KNOCKBACK_WORD)?;
    let f = word(FLAGS_WORD)?;
    let vel_scale = word(VEL_SCALE_WORD)? >> 23;
    let spin = be_u16(data, base + SPIN_SPEED)?;
    let mut w = String::new();
    let _ = writeln!(w, "ItemAttributes {{");
    let _ = writeln!(w, "        is_give_hitlag: {},", bit(flags, 28));
    let _ = writeln!(w, "        is_display_colanim: {},", bit(flags, 29));
    let weight = if bit(flags, 27) { "Light" } else { "Heavy" };
    let _ = writeln!(w, "        weight: ItemWeight::{weight},");
    let _ = writeln!(
        w,
        "        attack_offsets: [{}, {}],",
        vec3(&attack[..3]),
        vec3(&attack[3..])
    );
    let _ = writeln!(w, "        damage_coll_offset: {},", vec3(&offset));
    let _ = writeln!(w, "        damage_coll_size: {},", vec3(&size));
    let _ = writeln!(
        w,
        "        map_coll: BodyColl {{ top: {}, center: {}, bottom: {}, width: {} }},",
        map[0], map[1], map[2], map[3]
    );
    let _ = writeln!(w, "        size: {},", emit::f32_lit(f32::from(hit_size)));
    let _ = writeln!(w, "        angle: {},", (a as i32) >> 22);
    let _ = writeln!(w, "        kb_scale: {},", (a >> 12) & 0x3FF);
    let _ = writeln!(w, "        damage: {},", (a >> 4) & 0xFF);
    let element = name(&ELEMENTS, a & 0xF, file, base + ATTACK_WORD)?;
    let _ = writeln!(w, "        element: Element::{element},");
    let _ = writeln!(w, "        kb_weight: {},", k >> 22);
    let _ = writeln!(w, "        shield_damage: {},", ((k << 10) as i32) >> 24);
    let _ = writeln!(w, "        attack_count: {},", (k >> 12) & 3);
    let _ = writeln!(w, "        can_setoff: {},", bit(k, 11));
    let _ = writeln!(w, "        priority: {},", f >> 29);
    let _ = writeln!(w, "        can_rehit_item: {},", bit(f, 28));
    let _ = writeln!(w, "        can_rehit_fighter: {},", bit(f, 27));
    let _ = writeln!(w, "        can_hop: {},", bit(f, 26));
    let _ = writeln!(w, "        can_reflect: {},", bit(f, 25));
    let _ = writeln!(w, "        can_shield: {},", bit(f, 24));
    let _ = writeln!(w, "        kb_base: {},", (f >> 14) & 0x3FF);
    let ty = name(&ITEM_TYPES, (f >> 10) & 0xF, file, base + FLAGS_WORD)?;
    let _ = writeln!(w, "        ty: ItemType::{ty},");
    let hitstatus = name(&HIT_STATUSES, (f >> 6) & 0xF, file, base + FLAGS_WORD)?;
    let _ = writeln!(w, "        hitstatus: HitStatus::{hitstatus},");
    let _ = writeln!(w, "        vel_scale: {vel_scale},");
    Ok((w, spin))
}

/// The literal a stub build gets: every field zero or its first variant.
fn stub() -> String {
    let zero = "Vec3::new(0.0, 0.0, 0.0)";
    format!(
        "ItemAttributes {{\n\
         \x20       is_give_hitlag: false,\n\
         \x20       is_display_colanim: false,\n\
         \x20       weight: ItemWeight::Heavy,\n\
         \x20       attack_offsets: [{zero}, {zero}],\n\
         \x20       damage_coll_offset: {zero},\n\
         \x20       damage_coll_size: {zero},\n\
         \x20       map_coll: BodyColl {{ top: 0.0, center: 0.0, bottom: 0.0, width: 0.0 }},\n\
         \x20       size: 0.0,\n\
         \x20       angle: 0,\n\
         \x20       kb_scale: 0,\n\
         \x20       damage: 0,\n\
         \x20       element: Element::Normal,\n\
         \x20       kb_weight: 0,\n\
         \x20       shield_damage: 0,\n\
         \x20       attack_count: 0,\n\
         \x20       can_setoff: false,\n\
         \x20       priority: 0,\n\
         \x20       can_rehit_item: false,\n\
         \x20       can_rehit_fighter: false,\n\
         \x20       can_hop: false,\n\
         \x20       can_reflect: false,\n\
         \x20       can_shield: false,\n\
         \x20       kb_base: 0,\n\
         \x20       ty: ItemType::Damage,\n\
         \x20       hitstatus: HitStatus::None,\n\
         \x20       vel_scale: 0,\n"
    )
}

pub fn generate(rom: Option<&Source>) -> Result<String> {
    let mut w = emit::header("the items' attributes (`ITAttributes`)");
    let mut spins = Vec::new();
    w.push_str(
        "/// Each item's `ITAttributes`, named after its `ll*ItemAttributes` symbol.\n\
         #[rustfmt::skip]\n\
         pub mod attributes {\n    use super::*;\n\n",
    );
    for (name, file, at) in ITEMS {
        let (body, spin) = match rom {
            Some(rom) => record(&rom.file(file)?.data, file, at as usize)?,
            None => (stub(), 0),
        };
        let _ = write!(
            w,
            "    pub const {name}: ItemAttributes = {body}        sounds: crate::item_sounds::item::{name},\n    }};\n"
        );
        spins.push((name, spin));
    }
    w.push_str(
        "}\n\n/// Each item's `ITAttributes::spin_speed`, percent.\n\
         pub mod spin {\n",
    );
    for (name, spin) in spins {
        let _ = writeln!(w, "    pub const {name}: u16 = {spin};");
    }
    w.push_str(
        "}\n\n/// Each `ll*AttackEvents` table: `(timer, angle, damage, size)` per\n\
         /// `ITAttackEvent`.\n\
         #[rustfmt::skip]\n\
         pub mod attack_events {\n",
    );
    for (name, file, at) in ATTACK_EVENTS {
        let rows: Vec<String> = match rom {
            Some(rom) => {
                let data = rom.file(file)?.data;
                (0..ATTACK_EVENT_COUNT)
                    .map(|i| attack_event(&data, at as usize + i * ATTACK_EVENT_SIZE))
                    .collect::<Result<_>>()?
            }
            None => vec!["(0, 0, 0, 0.0)".to_string(); ATTACK_EVENT_COUNT],
        };
        let _ = writeln!(
            w,
            "    pub const {name}: [(u16, i32, i32, f32); {ATTACK_EVENT_COUNT}] = [{}];",
            rows.join(", ")
        );
    }
    w.push_str("}\n");
    for (name, symbol, file, at, count) in FLOAT_TABLES {
        let cells: Vec<String> = match rom {
            Some(rom) => {
                let data = rom.file(file)?.data;
                (0..count)
                    .map(|i| Ok(emit::f32_lit(f32::from_bits(be_u32(&data, at + 4 * i)?))))
                    .collect::<Result<_>>()?
            }
            None => vec!["0.0".to_string(); count],
        };
        let _ = writeln!(
            w,
            "\n/// `{symbol}`.\npub const {name}: [f32; {count}] = [{}];",
            cells.join(", ")
        );
    }
    Ok(w)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stub_records_cover_every_item() {
        let text = generate(None).unwrap();
        assert_eq!(
            text.matches(": ItemAttributes = ItemAttributes {").count(),
            ITEMS.len()
        );
        assert_eq!(text.matches(": u16 = 0;").count(), ITEMS.len());
    }

    #[test]
    fn attack_events_decode_msb_first() {
        // timer 4, angle -30, damage 20, size 150.
        let w: u32 = (4 << 24) | (((-30i32 as u32) & 0x3FF) << 14) | (20 << 6);
        let mut d = w.to_be_bytes().to_vec();
        d.extend_from_slice(&150u16.to_be_bytes());
        d.extend_from_slice(&[0, 0]);
        assert_eq!(attack_event(&d, 0).unwrap(), "(4, -30, 20, 150.0)");
        let text = generate(None).unwrap();
        assert_eq!(
            text.matches("; 4] = [(0, 0, 0, 0.0)").count(),
            ATTACK_EVENTS.len()
        );
        assert!(text.contains("pub const CONTAINER_VELOCITIES_Y: [f32; 20]"));
        assert!(text.contains("pub const LINK_BOMB_BLOAT_SCALES: [f32; 6]"));
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
        let (text, spin) = record(&d, 0, 0).unwrap();
        assert_eq!(spin, 120);
        for want in [
            "is_give_hitlag: true",
            "is_display_colanim: true",
            "weight: ItemWeight::Light",
            "attack_offsets: [Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.0, 350.0, 0.0)]",
            "damage_coll_offset: Vec3::new(0.0, 200.0, 0.0)",
            "size: 300.0",
            "angle: 361",
            "kb_scale: 80",
            "damage: 10",
            "element: Element::Slash",
            "kb_weight: 400",
            "shield_damage: -8",
            "attack_count: 2",
            "can_setoff: true",
            "priority: 1",
            "can_rehit_item: true",
            "can_rehit_fighter: false",
            "can_shield: true",
            "kb_base: 60",
            "ty: ItemType::Consume",
            "hitstatus: HitStatus::Normal",
            "vel_scale: 110",
        ] {
            assert!(text.contains(want), "{want} in\n{text}");
        }
    }
}
