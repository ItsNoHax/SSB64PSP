//! The stage ambient effects' flight cycles, for
//! `ssb_game::ground_effect::cycle`: per `EFGroundDesc` (`ef/efground.c`),
//! the `gcPlayAnimAll` call on which the root's child `DObj`'s script ends
//! (`child_dobj->anim_wait == AOBJ_ANIM_NULL`, counting the add's own play
//! as 1) and that child's `translate.x`/`.y` there, which
//! `efGroundCommonProcUpdate` adds to the root before it re-adds the
//! animation at frame 0.
//!
//! The child's `AObj`s outlive a re-add (`gcAddDObjAnimJoint` only resets
//! `kind`), so a cycle's first commands read the previous cycle's targets,
//! rates and `length_invert`; the end is computed for the first cycle and
//! for the cycles after it.
//!
//! Plays the child's `AObjEvent32` script as `gcParseDObjAnimJoint`
//! (`src/sys/objanim.c:268-634`) and evaluates its translate tracks as
//! `gcPlayDObjAnim` (`src/sys/objanim.c:715-822`) does, in `f32`.
//! `ssb_rom::objanim` plays the same scripts; this is a second, deliberately
//! minimal reading (`crates/ssb-rom/tests/ground_effects.rs` compares them).

use std::fmt::Write;

use crate::emit;
use crate::source::{be_u32, OverlayData, OVL2};
use crate::{Error, Result, Source};

/// `dEFGroundCastleEffectDescs`, the first `EFGroundDesc`; the seven
/// stages' arrays follow it contiguously, 24 descs in all.
pub const DESCS: u32 = 0x8012_F240;
pub const DESC_SIZE: u32 = 0x40;
pub const DESC_COUNT: usize = 24;
/// `EFGroundDesc::effect_desc.o_dobjsetup` and `.o_anim_joint`.
const O_DOBJSETUP: u32 = 0x30;
const O_ANIM_JOINT: u32 = 0x38;
/// `dEFGroundDatas`: `o_data` and `effect_descs` of each `gkind` row.
const DATAS: u32 = 0x8012_F840;
const DATA_SIZE: u32 = 16;
/// The file `sEFGroundActor.file_head` points at
/// (`gr_desc[1].dobjdesc - o_data`), by `gkind`: the stage's
/// `Stage*File2` (`refs/ssb-decomp-re/src/relocData/1NN_Stage*File2.c`,
/// whose `Layer1DObj` sits at `ll*MapDataStart`). Hyrule (4) has no
/// effects.
pub const STAGE_FILES: [u32; 8] = [106, 109, 108, 105, 0, 111, 104, 112];

/// `sizeof(DObjDesc)` and its `DOBJ_ARRAY_MAX` terminator id.
const DOBJ_DESC_SIZE: usize = 0x2C;
const DOBJ_ARRAY_MAX: u32 = 18;
/// Plays before a script that never ends is an error.
const MAX_PLAYS: u16 = 2000;
/// `nGCAnimTrackTraX - nGCAnimTrackJointStart` and `TraY`'s.
const TRA_X: usize = 4;
const TRA_Y: usize = 5;

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    None,
    Linear,
    Cubic,
    Step,
}

/// One `AObj`: only the fields `gcPlayDObjAnim` reads.
#[derive(Clone, Copy)]
struct Track {
    added: bool,
    kind: Kind,
    base: f32,
    target: f32,
    rate_base: f32,
    rate_target: f32,
    length: f32,
    length_invert: f32,
}

impl Track {
    /// `gcAddAObjForDObj`.
    const NEW: Track = Track {
        added: true,
        kind: Kind::None,
        base: 0.0,
        target: 0.0,
        rate_base: 0.0,
        rate_target: 0.0,
        length: 0.0,
        length_invert: 1.0,
    };

    /// `gcPlayDObjAnim`'s value for one track.
    fn value(&self) -> f32 {
        match self.kind {
            Kind::Linear => self.base + self.length * self.rate_base,
            Kind::Cubic => {
                let f16 = self.length_invert * self.length_invert;
                let f12 = self.length * self.length;
                let f18 = self.length_invert * f12;
                let f14 = self.length * f12 * f16;
                let f20 = 2.0 * f14 * self.length_invert;
                let f22 = 3.0 * f12 * f16;
                let f24 = f14 - f18;
                self.base * ((f20 - f22) + 1.0)
                    + self.target * (f22 - f20)
                    + self.rate_base * ((f24 - f18) + self.length)
                    + self.rate_target * f24
            }
            Kind::Step => {
                if self.length_invert <= self.length {
                    self.target
                } else {
                    self.base
                }
            }
            Kind::None => 0.0,
        }
    }
}

/// The child `DObj`: its script position, clock and ten tracks.
struct Child {
    at: usize,
    wait: f32,
    frame: f32,
    changed: bool,
    ended: bool,
    tracks: [Track; 10],
    /// `translate.x`, `.y`.
    tra: [f32; 2],
}

impl Child {
    /// `gcAddDObjAnimJoint(child, script, 0)`, keeping the `AObj`s.
    fn add(&mut self, script: u32) {
        for t in &mut self.tracks {
            t.kind = Kind::None;
        }
        self.at = script as usize;
        self.changed = true;
        self.frame = 0.0;
        self.ended = false;
    }

    /// One `gcParseDObjAnimJoint` then `gcPlayDObjAnim`.
    fn play(&mut self, data: &[u8], file: u32) -> Result<()> {
        if self.ended {
            return Ok(());
        }
        let parse = if self.changed {
            self.changed = false;
            self.wait = -self.frame;
            true
        } else {
            self.wait -= 1.0;
            self.frame += 1.0;
            self.wait <= 0.0
        };
        let mut end = false;
        if parse {
            end = self.parse(data, file)?;
        }
        for t in &mut self.tracks {
            if t.kind == Kind::None {
                continue;
            }
            if !end {
                t.length += 1.0;
            }
        }
        for (k, i) in [TRA_X, TRA_Y].into_iter().enumerate() {
            if self.tracks[i].kind != Kind::None {
                self.tra[k] = self.tracks[i].value();
            }
        }
        self.ended = end;
        Ok(())
    }

    /// The commands up to the next blocking wait; `true` on `End`.
    fn parse(&mut self, data: &[u8], file: u32) -> Result<bool> {
        let bad = |offset| Error::BadRelocation { file, offset };
        loop {
            let w = be_u32(data, self.at)?;
            let (op, mut flags, payload) = (w >> 25, (w >> 15) & 0x3FF, (w & 0x7FFF) as f32);
            self.at += 4;
            let word = |at: &mut usize| -> Result<f32> {
                let v = f32::from_bits(be_u32(data, *at)?);
                *at += 4;
                Ok(v)
            };
            match op {
                // `End`.
                0 => {
                    for t in &mut self.tracks {
                        if t.kind != Kind::None {
                            t.length += 1.0 + self.wait;
                        }
                    }
                    return Ok(true);
                }
                // `Wait`.
                2 => self.wait += payload,
                3..=11 if op != 7 => {
                    let mut i = 0;
                    while flags != 0 {
                        if flags & 1 != 0 {
                            let t = &mut self.tracks[i];
                            if !t.added {
                                *t = Track::NEW;
                            }
                            t.base = t.target;
                            t.target = word(&mut self.at)?;
                            match op {
                                // `SetValBlock`, `SetVal`.
                                3 | 4 => {
                                    t.kind = Kind::Linear;
                                    if payload != 0.0 {
                                        t.rate_base = (t.target - t.base) / payload;
                                    }
                                    t.rate_target = 0.0;
                                }
                                // `SetValRateBlock`, `SetValRate`.
                                5 | 6 => {
                                    t.rate_base = t.rate_target;
                                    t.rate_target = word(&mut self.at)?;
                                    t.kind = Kind::Cubic;
                                    if payload != 0.0 {
                                        t.length_invert = 1.0 / payload;
                                    }
                                }
                                // `SetVal0RateBlock`, `SetVal0Rate`.
                                8 | 9 => {
                                    t.rate_base = t.rate_target;
                                    t.rate_target = 0.0;
                                    t.kind = Kind::Cubic;
                                    if payload != 0.0 {
                                        t.length_invert = 1.0 / payload;
                                    }
                                }
                                // `SetValAfterBlock`, `SetValAfter`.
                                _ => {
                                    t.kind = Kind::Step;
                                    t.length_invert = payload;
                                    t.rate_target = 0.0;
                                }
                            }
                            t.length = -self.wait - 1.0;
                        }
                        flags >>= 1;
                        i += 1;
                    }
                    if matches!(op, 3 | 5 | 8 | 10) {
                        self.wait += payload;
                    }
                }
                // `SetTargetRate`.
                7 => {
                    let mut i = 0;
                    while flags != 0 {
                        if flags & 1 != 0 {
                            let t = &mut self.tracks[i];
                            if !t.added {
                                *t = Track::NEW;
                            }
                            t.rate_target = word(&mut self.at)?;
                        }
                        flags >>= 1;
                        i += 1;
                    }
                }
                // `Jump`, `SetAnim` (no end), 12, `SetInterp` and the
                // `SetFlags` family: none in these scripts.
                _ => return Err(bad(self.at - 4)),
            }
            if self.wait > 0.0 {
                return Ok(false);
            }
        }
    }
}

/// One desc's cycle: its life and the child's end `translate.x`/`.y` on
/// the first cycle and on the later ones.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cycle {
    pub life: u16,
    pub end: [[f32; 2]; 2],
}

fn cycle(data: &[u8], file: u32, script: u32) -> Result<Cycle> {
    let mut child = Child {
        at: 0,
        wait: 0.0,
        frame: 0.0,
        changed: false,
        ended: false,
        tracks: [Track {
            added: false,
            ..Track::NEW
        }; 10],
        tra: [0.0; 2],
    };
    let mut lives = [0u16; 3];
    let mut ends = [[0.0f32; 2]; 3];
    for (life, end) in lives.iter_mut().zip(ends.iter_mut()) {
        child.add(script);
        for play in 1..=MAX_PLAYS {
            child.play(data, file)?;
            if child.ended {
                *life = play;
                break;
            }
        }
        if *life == 0 {
            return Err(Error::BadRelocation {
                file,
                offset: script as usize,
            });
        }
        *end = child.tra;
        // `efGroundCommonProcUpdate` zeroes them before the re-add.
        child.tra = [0.0; 2];
    }
    let steady = ends[1].map(f32::to_bits) == ends[2].map(f32::to_bits);
    if lives[0] != lives[1] || lives[1] != lives[2] || !steady {
        return Err(Error::BadRelocation {
            file,
            offset: script as usize,
        });
    }
    Ok(Cycle {
        life: lives[0],
        end: [ends[0], ends[1]],
    })
}

/// `DObjDesc` entries at `at` before the terminator, at most `max`.
fn desc_nodes(data: &[u8], file: u32, at: usize, max: usize) -> Result<usize> {
    let mut n = 0;
    while be_u32(data, at + n * DOBJ_DESC_SIZE)? != DOBJ_ARRAY_MAX {
        n += 1;
        if n > max {
            return Err(Error::BadRelocation { file, offset: at });
        }
    }
    Ok(n)
}

/// Every desc's cycle, in desc order.
pub fn cycles(rom: &Source) -> Result<Vec<Cycle>> {
    let ovl2 = OverlayData::new(Some(rom), OVL2);
    let mut out = vec![None; DESC_COUNT];
    for (gkind, &file) in STAGE_FILES.iter().enumerate() {
        let row = DATAS + gkind as u32 * DATA_SIZE;
        let (o_data, descs) = (ovl2.u32(row + 8)?, ovl2.u32(row + 12)?);
        if file == 0 {
            if descs != 0 {
                return Err(Error::BadRelocation {
                    file,
                    offset: row as usize,
                });
            }
            continue;
        }
        let params_num = ovl2.u8(row)?;
        let params = ovl2.u32(row + 4)?;
        let data = rom.file(file)?.data;
        // `Layer1DObj` at `o_data` (a stage layer, any length): the file
        // `file_head` resolves to.
        desc_nodes(&data, file, o_data as usize, 256)?;
        // The stage's descs: those its params name (`effect_id`).
        for p in 0..u32::from(params_num) {
            let id = ovl2.u16(params + p * 12)?;
            let desc = descs + u32::from(id) * DESC_SIZE;
            let index = ((desc - DESCS) / DESC_SIZE) as usize;
            let dobjs = ovl2.u32(desc + O_DOBJSETUP)?;
            let table = ovl2.u32(desc + O_ANIM_JOINT)?;
            // `efGroundSetupEffectDObjs`'s `array_dobjs[18]`.
            desc_nodes(&data, file, dobjs as usize, DOBJ_ARRAY_MAX as usize)?;
            let script = be_u32(&data, table as usize)?;
            out[index] = Some(cycle(&data, file, script)?);
        }
    }
    out.into_iter()
        .enumerate()
        .map(|(i, c)| {
            c.ok_or(Error::BadRelocation {
                file: 0,
                offset: DESCS as usize + i * DESC_SIZE as usize,
            })
        })
        .collect()
}

pub fn generate(rom: Option<&Source>) -> Result<String> {
    let cycles = match rom {
        Some(rom) => cycles(rom)?,
        None => vec![
            Cycle {
                life: 0,
                end: [[0.0; 2]; 2],
            };
            DESC_COUNT
        ],
    };
    let mut w = emit::header("the stage ambient effects' flight cycles");
    let _ = writeln!(
        w,
        "/// The `gcPlayAnimAll` call, counting the add's as 1, on which each\n\
         /// `EFGroundDesc`'s child `DObj` ends its script.\n\
         pub const LIFE: [u16; {DESC_COUNT}] = {:?};",
        cycles.iter().map(|c| c.life).collect::<Vec<_>>()
    );
    let _ = writeln!(
        w,
        "/// The child's `translate.x`, `.y` at that play: `[first cycle, later\n\
         /// cycles]`.\n\
         pub const END: [[[f32; 2]; 2]; {DESC_COUNT}] = ["
    );
    for c in &cycles {
        let pair = |e: [f32; 2]| format!("[{}, {}]", emit::f32_lit(e[0]), emit::f32_lit(e[1]));
        let _ = writeln!(w, "    [{}, {}],", pair(c.end[0]), pair(c.end[1]));
    }
    let _ = writeln!(w, "];");
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
    fn stub_cycles_are_zero() {
        let text = generate(None).unwrap();
        assert!(text.contains("pub const LIFE: [u16; 24] = [0, 0,"));
    }

    /// `SetVal` TraX to 0 then a 10-frame linear block to -100: `End` is
    /// parsed on play 11, where the block's `length` reaches 10.
    #[test]
    fn a_linear_block_ends_at_its_target() {
        let data = bytes(&[
            cmd(4, 0x10, 0),
            0.0f32.to_bits(),
            cmd(3, 0x10, 10),
            (-100.0f32).to_bits(),
            0,
        ]);
        let c = cycle(&data, 0, 0).unwrap();
        assert_eq!(c.life, 11);
        assert_eq!(c.end, [[-100.0, 0.0], [-100.0, 0.0]]);
    }
}
