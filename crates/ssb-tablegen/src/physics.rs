//! Mario's `FTAttributes` physics head (`fttypes.h`), for
//! `ssb_game::physics::PhysicsAttributes::mario`, the roster's reference
//! values that `PhysicsAttributes::default` and the gameplay tests use.
//!
//! The head is a run of `f32` words from the struct's start (`size` at word
//! 0, `jumps_max` an `s32` at word 25), the order `ssb_rom::fighter::
//! FighterAttributes::decode` reads; the file and offset are
//! [`FIGHTERS`](crate::fighters::FIGHTERS)' Mario entry (`dMarioMain_attr`,
//! file 203).

use alloc::vec::Vec;

use crate::fighters::FIGHTERS;
use crate::source::be_u32;
use crate::Result;

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
pub const JUMPS_MAX: usize = 25;
/// `FTAttributes::map_coll`: `top`, `center`, `bottom`, `width`.
const MAP_COLL: usize = 39;

/// The `MarioPhysics` table: [`FIELDS`]' words in their order, then
/// `map_coll`'s four (`f32` bits; `jumps_max` an `s32`).
pub fn bytes(c: &crate::tables::Ctx) -> Result<Vec<u8>> {
    let mario = &FIGHTERS[0];
    assert_eq!(mario.name, "Mario", "FIGHTERS is in FTKind order");
    let data = c.rom.file(mario.main_file)?.data;
    let word = |index: usize| be_u32(&data, mario.attributes as usize + 4 * index);
    let words = FIELDS
        .iter()
        .map(|&(_, index)| index)
        .chain(MAP_COLL..MAP_COLL + 4)
        .map(word)
        .collect::<Result<Vec<u32>>>()?;
    Ok(crate::tables::le_words(&words))
}

/// Words in the `MarioPhysics` table.
pub const WORDS: usize = FIELDS.len() + 4;
