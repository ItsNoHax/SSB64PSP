//! The quakes' and the Thunder Shock spark's lives, for
//! `ssb_game::effect::life`: the `gcPlayAnimAll` call, counting the make's
//! as 1, after which the effect's `GObj::anim_frame` first reads at or below
//! zero and its `proc_update` ejects it (`efManagerHaveStructProcUpdate`).
//!
//! Plays each `DObj`'s `AObjEvent32` script as `gcParseDObjAnimJoint`
//! (`src/sys/objanim.c:268-634`) advances its clock at `anim_speed` 1: the
//! words each command consumes and which commands add to `anim_wait`.
//! `ssb_rom::objanim` plays the same scripts in full; this is a second,
//! deliberately minimal reading (`ssb-rom/tests/display_effects.rs`
//! compares the two).

use std::fmt::Write;

use crate::emit;
use crate::source::be_u32;
use crate::{Error, Result, Source};

/// `gEFManagerFiles[0]` (`EFCommonEffects1`).
pub const QUAKE_FILE: u32 = 83;
/// `llEFCommonEffects1QuakeMag{0,1,2,3}AnimJoint`: one script for the
/// quake's single `DObj` (`efManagerQuakeMakeEffect`'s `switch`).
pub const QUAKE_TABLES: [u32; 4] = [0xCBC0, 0xCC20, 0xCCF0, 0xCDC0];
/// `gFTDataPikachuSpecial2`.
pub const SHOCK_FILE: u32 = 347;
/// `llPikachuSpecial2ThunderShockDObjDesc`: the spark's tree, built in
/// array order (`gcSetupCustomDObjs`), so its pre-order walk
/// (`gcAddAnimAll`, `gcPlayAnimAll`) is array order too.
pub const SHOCK_DESC: usize = 0x1640;
/// `DObjDesc` entries before the `DOBJ_ARRAY_MAX` terminator.
pub const SHOCK_NODES: usize = 4;
/// `llPikachuSpecial2ThunderShock{0,1,2}AnimJoint`: 0 is the `EFDesc`'s,
/// 1 and 2 the ones `efManagerPikachuThunderShockMakeEffect` adds in its
/// place and plays once.
pub const SHOCK_TABLES: [u32; 3] = [0x1720, 0x1850, 0x1970];

/// `sizeof(DObjDesc)`: id, display list, translate, rotate, scale.
const DOBJ_DESC_SIZE: usize = 0x2C;
/// `DOBJ_ARRAY_MAX`, the `DObjDesc` terminator id.
const DOBJ_ARRAY_MAX: u32 = 18;
/// `AOBJ_ANIM_END` (`F32_MIN / 3`).
const ANIM_END: f32 = f32::MIN / 3.0;
/// Plays before a script that never ends is an error.
const MAX_PLAYS: u16 = 600;

/// One `DObj`'s animation clock.
struct Joint {
    /// Next command, a file offset.
    at: usize,
    wait: f32,
    frame: f32,
    /// `anim_wait == AOBJ_ANIM_CHANGED`: added, not yet played.
    changed: bool,
}

impl Joint {
    /// One `gcParseDObjAnimJoint` call; `gobj` is the `GObj::anim_frame`.
    fn play(&mut self, data: &[u8], file: u32, gobj: &mut f32) -> Result<()> {
        let bad = |offset| Error::BadRelocation { file, offset };
        if self.changed {
            self.changed = false;
            self.wait = -self.frame;
        } else {
            self.wait -= 1.0;
            self.frame += 1.0;
            *gobj = self.frame;
            if self.wait > 0.0 {
                return Ok(());
            }
        }
        loop {
            let w = be_u32(data, self.at)?;
            let (op, flags, payload) = (w >> 25, (w >> 15) & 0x3FF, (w & 0x7FFF) as f32);
            let values = |per: u32| 4 + (flags.count_ones() * per) as usize * 4;
            match op {
                // `End` keeps its place: an ended clock reaches it again
                // each play and reports `AOBJ_ANIM_END`.
                0 => {
                    self.frame = self.wait;
                    *gobj = self.wait;
                    self.wait = ANIM_END;
                    return Ok(());
                }
                // `Wait`, `SetFlags`, 16.
                2 | 15 | 16 => {
                    self.wait += payload;
                    self.at += 4;
                }
                // `SetValBlock`, `SetVal0RateBlock`, `SetValAfterBlock`.
                3 | 8 | 10 => {
                    self.wait += payload;
                    self.at += values(1);
                }
                // `SetVal`, `SetTargetRate`, `SetVal0Rate`, `SetValAfter`.
                4 | 7 | 9 | 11 => self.at += values(1),
                // `SetValRateBlock`.
                5 => {
                    self.wait += payload;
                    self.at += values(2);
                }
                // `SetValRate`.
                6 => self.at += values(2),
                // 12: track lengths only.
                12 => self.at += 4,
                // `SetInterp`: the command and a pointer.
                13 => self.at += 8,
                // 17: one value per flag after the command.
                17 => {
                    self.wait += payload;
                    self.at += values(1);
                }
                // `Jump` and `SetAnim` loop (no life); 18 and up never
                // advance (`default`).
                _ => return Err(bad(self.at)),
            }
            if self.wait > 0.0 {
                return Ok(());
            }
        }
    }
}

/// The play on which a tree whose animated `DObj`s run `scripts` (tree
/// order) first leaves `GObj::anim_frame` at or below zero, counting the
/// make's play as 1.
fn life(data: &[u8], file: u32, scripts: &[u32]) -> Result<u16> {
    let mut joints: Vec<Joint> = scripts
        .iter()
        .map(|&s| Joint {
            at: s as usize,
            wait: 0.0,
            frame: 0.0,
            changed: true,
        })
        .collect();
    // `gcAddAnimAll` sets it to the start frame.
    let mut gobj = 0.0f32;
    for play in 1..=MAX_PLAYS {
        for j in &mut joints {
            j.play(data, file, &mut gobj)?;
        }
        if play > 1 && gobj <= 0.0 {
            return Ok(play);
        }
    }
    Err(Error::BadRelocation {
        file,
        offset: scripts.first().copied().unwrap_or(0) as usize,
    })
}

/// The first `nodes` entries of `AnimJoint` table `table`, `NULL`s
/// (`AOBJ_ANIM_NULL`, never played) dropped.
fn scripts(data: &[u8], table: u32, nodes: usize) -> Result<Vec<u32>> {
    let mut out = Vec::new();
    for i in 0..nodes {
        let s = be_u32(data, table as usize + i * 4)?;
        if s != 0 {
            out.push(s);
        }
    }
    Ok(out)
}

/// `DObjDesc` entries at `at` before the terminator.
fn desc_nodes(data: &[u8], at: usize) -> Result<usize> {
    let mut n = 0;
    while be_u32(data, at + n * DOBJ_DESC_SIZE)? != DOBJ_ARRAY_MAX {
        n += 1;
        if n > DOBJ_ARRAY_MAX as usize {
            return Err(Error::BadRelocation {
                file: SHOCK_FILE,
                offset: at,
            });
        }
    }
    Ok(n)
}

fn lives(rom: &Source) -> Result<([u16; 4], [u16; 3])> {
    let mut quake = [0; 4];
    let data = rom.file(QUAKE_FILE)?.data;
    for (life_out, &table) in quake.iter_mut().zip(&QUAKE_TABLES) {
        *life_out = life(&data, QUAKE_FILE, &scripts(&data, table, 1)?)?;
    }
    let mut shock = [0; 3];
    let data = rom.file(SHOCK_FILE)?.data;
    if desc_nodes(&data, SHOCK_DESC)? != SHOCK_NODES {
        return Err(Error::BadRelocation {
            file: SHOCK_FILE,
            offset: SHOCK_DESC,
        });
    }
    for (life_out, &table) in shock.iter_mut().zip(&SHOCK_TABLES) {
        *life_out = life(&data, SHOCK_FILE, &scripts(&data, table, SHOCK_NODES)?)?;
    }
    Ok((quake, shock))
}

pub fn generate(rom: Option<&Source>) -> Result<String> {
    let (quake, shock) = match rom {
        Some(rom) => lives(rom)?,
        None => ([0; 4], [0; 3]),
    };
    let mut w = emit::header("the quakes' and the Thunder Shock spark's animation lives");
    let _ = writeln!(
        w,
        "/// `llEFCommonEffects1QuakeMag0AnimJoint` to `...Mag3...`.\n\
         pub const QUAKE: [u16; 4] = {quake:?};\n\
         /// `llPikachuSpecial2ThunderShock{{0,1,2}}AnimJoint`, each counting the\n\
         /// default animation's play at the make.\n\
         pub const THUNDER_SHOCK: [u16; 3] = {shock:?};"
    );
    Ok(w)
}

#[cfg(test)]
mod tests {
    use super::*;

    const fn cmd(op: u32, flags: u32, payload: u32) -> u32 {
        (op << 25) | (flags << 15) | payload
    }

    fn bytes(words: &[u32]) -> Vec<u8> {
        words.iter().flat_map(|w| w.to_be_bytes()).collect()
    }

    #[test]
    fn stub_lives_are_zero() {
        let text = generate(None).unwrap();
        assert!(text.contains("pub const QUAKE: [u16; 4] = [0, 0, 0, 0];"));
        assert!(text.contains("pub const THUNDER_SHOCK: [u16; 3] = [0, 0, 0];"));
    }

    /// A block of 18 frames then `End`: `End` runs on play 19 and writes
    /// its `anim_wait` of 0.
    #[test]
    fn a_block_then_end_lives_its_payload_plus_one() {
        let data = bytes(&[cmd(3, 0b101, 18), 1.0f32.to_bits(), 2.0f32.to_bits(), 0]);
        assert_eq!(life(&data, 0, &[0]).unwrap(), 19);
    }

    /// The last animated `DObj` in tree order owns the clock: a short
    /// script ended earlier reports `AOBJ_ANIM_END` only when it is last.
    #[test]
    fn the_last_joint_in_tree_order_decides() {
        let data = bytes(&[cmd(2, 0, 4), 0, cmd(2, 0, 9), 0]);
        assert_eq!(life(&data, 0, &[8, 0]).unwrap(), 5);
        assert_eq!(life(&data, 0, &[0, 8]).unwrap(), 10);
    }

    #[test]
    fn a_jump_has_no_life() {
        let data = bytes(&[cmd(2, 0, 2), cmd(1, 0, 0), 0]);
        assert!(life(&data, 0, &[0]).is_err());
    }
}
