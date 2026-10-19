//! Mario's `FTAttributes` physics head (`fttypes.h`), for
//! `ssb_game::physics::PhysicsAttributes::MARIO`, the roster's reference
//! values that `PhysicsAttributes::default` and the gameplay tests use.
//!
//! The head is a run of `f32` words from the struct's start (`size` at word
//! 0, `jumps_max` an `s32` at word 25), the order `ssb_rom::fighter::
//! FighterAttributes::decode` reads; the file and offset are
//! [`FIGHTERS`](crate::fighters::FIGHTERS)' Mario entry (`dMarioMain_attr`,
//! file 203).

use std::fmt::Write;

use crate::fighters::FIGHTERS;
use crate::source::be_u32;
use crate::{emit, Result, Source};

/// `PhysicsAttributes` fields and their `FTAttributes` word.
pub const FIELDS: [(&str, usize); 24] = [
    ("size", 0),
    ("traction", 9),
    ("dash_speed", 10),
    ("dash_decel", 11),
    ("run_speed", 12),
    ("walk_speed_mul", 8),
    ("jump_vel_x", 14),
    ("jump_height_mul", 15),
    ("jump_height_base", 16),
    ("jumpaerial_vel_x", 17),
    ("jumpaerial_height", 18),
    ("air_accel", 19),
    ("air_speed_max_x", 20),
    ("air_friction", 21),
    ("gravity", 22),
    ("tvel_base", 23),
    ("tvel_fast", 24),
    ("jumps_max", JUMPS_MAX),
    ("weight", 26),
    ("kneebend_anim_length", 13),
    ("dash_to_run", 28),
    ("walkslow_anim_length", 1),
    ("walkmiddle_anim_length", 2),
    ("walkfast_anim_length", 3),
];
/// `FTAttributes::jumps_max`, the head's one integer.
const JUMPS_MAX: usize = 25;
/// `FTAttributes::map_coll`: `top`, `center`, `bottom`, `width`.
const MAP_COLL: usize = 39;

pub fn generate(rom: Option<&Source>) -> Result<String> {
    let mario = &FIGHTERS[0];
    assert_eq!(mario.name, "Mario", "FIGHTERS is in FTKind order");
    let data = match rom {
        Some(rom) => Some(rom.file(mario.main_file)?.data),
        None => None,
    };
    let mut w = emit::header("Mario's `FTAttributes` physics (`dMarioMain_attr`)");
    w.push_str("/// Mario's `FTAttributes` physics head.\npub const MARIO: PhysicsAttributes = PhysicsAttributes {\n");
    for (field, index) in FIELDS {
        let bits = match &data {
            Some(d) => be_u32(d, mario.attributes as usize + 4 * index)?,
            None => 0,
        };
        let value = if index == JUMPS_MAX {
            (bits as i32).to_string()
        } else {
            emit::f32_lit(f32::from_bits(bits))
        };
        let _ = writeln!(w, "    {field}: {value},");
    }
    w.push_str("};\n\n/// Mario's `FTAttributes::map_coll`: top, center, bottom, width.\npub const MARIO_MAP_COLL: [f32; 4] = [");
    let mut cells = Vec::new();
    for i in 0..4 {
        let bits = match &data {
            Some(d) => be_u32(d, mario.attributes as usize + 4 * (MAP_COLL + i))?,
            None => 0,
        };
        cells.push(emit::f32_lit(f32::from_bits(bits)));
    }
    let _ = writeln!(w, "{}];", cells.join(", "));
    Ok(w)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stub_is_zero() {
        let text = generate(None).unwrap();
        assert!(text.contains("    gravity: 0.0,\n"));
        assert!(text.contains("    jumps_max: 0,\n"));
        assert_eq!(text.matches(": 0").count(), FIELDS.len());
        assert!(text.contains("MARIO_MAP_COLL: [f32; 4] = [0.0, 0.0, 0.0, 0.0];"));
    }
}
