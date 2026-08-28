//! Stage joint animation (`AObjEvent32` animation joints).
//!
//! A fighter's joints are driven by the 16-bit figatree stream
//! [`figatree`](crate::figatree) decodes. A *stage's* are driven by the 32-bit
//! `AObjEvent32` stream instead — the same encoding
//! [`matanim`](crate::matanim) reads for costume colours, pointed at the ten
//! joint tracks rather than the material ones:
//!
//! ```c
//! struct MPGroundDesc {
//!     DObjDesc *dobjdesc;
//!     AObjEvent32 **anim_joints;   // one script per node
//!     ...
//! };
//! ```
//!
//! `gcAddAnimJointAll` walks the layer's `DObj` tree and hands node `i` script
//! `i`, and `gcParseDObjAnimJoint` then runs it into the same `AObj` tracks
//! `gcPlayDObjAnimJoint` reads. So the *state machine* here is shared with
//! `figatree` — only the instruction encoding differs, and one thing about the
//! values:
//!
//! **A 32-bit event stores a real `f32`.** The 16-bit stream stores an `s16`
//! that `ftAnimGetTargetValue` scales by 1/512 for rotations, 1/4 for
//! translations and so on. Here the word following a command *is* the value,
//! already in radians or model units, and applying figatree's scale factors to
//! it would be wrong by three orders of magnitude.
//!
//! ## Encoding
//!
//! One `u32` per command, as in [`matanim`](crate::matanim):
//!
//! ```text
//! bits  31..25   24..15   14..0
//!       opcode   flags    payload
//! ```
//!
//! `flags` is a bitmask over the ten joint tracks and `payload` is a duration.
//! Each named track then reads one or two `f32` words, depending on the
//! opcode.

use crate::figatree::{Aobj, JointPose, Kind, TRACK_COUNT};

/// Opcodes, `AObjEvent32Kind`. Shared with [`matanim`](crate::matanim), which
/// reads the material half of the same instruction set.
const OP_END: u32 = 0;
const OP_JUMP: u32 = 1;
const OP_WAIT: u32 = 2;
const OP_SET_VAL_BLOCK: u32 = 3;
const OP_SET_VAL: u32 = 4;
const OP_SET_VAL_RATE_BLOCK: u32 = 5;
const OP_SET_VAL_RATE: u32 = 6;
const OP_SET_TARGET_RATE: u32 = 7;
const OP_SET_VAL0_RATE_BLOCK: u32 = 8;
const OP_SET_VAL0_RATE: u32 = 9;
const OP_SET_VAL_AFTER_BLOCK: u32 = 10;
const OP_SET_VAL_AFTER: u32 = 11;
const OP_ADD_LENGTH: u32 = 12;
const OP_SET_INTERP: u32 = 13;
const OP_SET_ANIM: u32 = 14;
const OP_SET_FLAGS: u32 = 15;

/// What went wrong running a stage animation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimError {
    /// The script ran past the end of its file.
    Truncated { at: usize },
    /// A command this player does not model. An error rather than a skip: the
    /// value words are counted per opcode, so guessing desynchronises the
    /// stream and every later key is read out of the middle of a float.
    UnknownOpcode { opcode: u32, at: usize },
    /// The script did not terminate within a sane number of commands.
    TooLong,
}

impl core::fmt::Display for AnimError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            AnimError::Truncated { at } => write!(f, "stage animation ran past its end at {at}"),
            AnimError::UnknownOpcode { opcode, at } => {
                write!(f, "stage animation opcode {opcode} at {at} is not modelled")
            }
            AnimError::TooLong => write!(f, "stage animation did not terminate"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for AnimError {}

fn u32_at(data: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes(data.get(at..at + 4)?.try_into().ok()?))
}

fn f32_at(data: &[u8], at: usize) -> Option<f32> {
    u32_at(data, at).map(f32::from_bits)
}

/// How many value words each named track reads.
fn values_per_track(opcode: u32) -> Option<usize> {
    Some(match opcode {
        OP_SET_VAL_RATE_BLOCK | OP_SET_VAL_RATE => 2,
        OP_SET_VAL_BLOCK
        | OP_SET_VAL
        | OP_SET_TARGET_RATE
        | OP_SET_VAL0_RATE_BLOCK
        | OP_SET_VAL0_RATE
        | OP_SET_VAL_AFTER_BLOCK
        | OP_SET_VAL_AFTER => 1,
        OP_END | OP_JUMP | OP_WAIT | OP_SET_FLAGS | OP_ADD_LENGTH | OP_SET_INTERP | OP_SET_ANIM => {
            0
        }
        _ => return None,
    })
}

/// Whether the command's payload advances the clock.
fn blocks(opcode: u32) -> bool {
    matches!(
        opcode,
        OP_WAIT
            | OP_SET_VAL_BLOCK
            | OP_SET_VAL_RATE_BLOCK
            | OP_SET_VAL0_RATE_BLOCK
            | OP_SET_VAL_AFTER_BLOCK
    )
}

/// One node's animation clock and ten tracks.
#[derive(Clone)]
pub struct StageJoint {
    tracks: [Aobj; TRACK_COUNT],
    anim_wait: f32,
    pc: usize,
    ended: bool,
    /// Where the script started, so a `Jump` that returns here is a loop.
    start: usize,
}

impl StageJoint {
    pub fn start(script: u32, frame: f32) -> Self {
        StageJoint {
            tracks: [Aobj::default(); TRACK_COUNT],
            anim_wait: -frame,
            pc: script as usize,
            ended: false,
            start: script as usize,
        }
    }

    pub fn ended(&self) -> bool {
        self.ended
    }

    /// Advances one tick and writes the tracks it names into `pose`.
    ///
    /// `pose` starts at the node's rest transform and is updated in place, so
    /// a track the script never mentions keeps its modelled value — the same
    /// contract [`figatree::JointAnim::tick`](crate::figatree::JointAnim::tick)
    /// has.
    pub fn tick(&mut self, data: &[u8], speed: f32, pose: &mut JointPose) -> Result<(), AnimError> {
        self.parse(data, speed)?;
        self.play(speed, pose);
        Ok(())
    }

    /// `gcParseDObjAnimJoint`: run commands until one blocks past now.
    fn parse(&mut self, data: &[u8], speed: f32) -> Result<(), AnimError> {
        if self.ended {
            return Ok(());
        }
        self.anim_wait -= speed;
        if self.anim_wait > 0.0 {
            return Ok(());
        }

        for _ in 0..4096 {
            let at = self.pc;
            let word = u32_at(data, at).ok_or(AnimError::Truncated { at })?;
            let opcode = word >> 25;
            let flags = ((word >> 15) & 0x3FF) as u16;
            let payload = (word & 0x7FFF) as f32;
            self.pc += 4;

            match opcode {
                OP_END => {
                    // `gcParseDObjAnimJoint`'s end path credits every live
                    // track with the time left in this tick, which is what
                    // carries a ramp all the way to its target on the frame
                    // the script stops rather than leaving it one short.
                    for t in self.tracks.iter_mut() {
                        if t.kind != Kind::None {
                            t.length += speed + self.anim_wait;
                        }
                    }
                    self.ended = true;
                    return Ok(());
                }
                // Both read the following word as a script pointer and
                // continue there. `SetAnim` additionally rebases `anim_frame`,
                // which nothing here reads.
                OP_JUMP | OP_SET_ANIM => {
                    let target = u32_at(data, self.pc).ok_or(AnimError::Truncated { at })?;
                    self.pc = target as usize;
                    // A jump straight back to itself would spin this loop.
                    if self.pc == at {
                        return Err(AnimError::TooLong);
                    }
                }
                // `dobj->flags = command.flags`, then the command's *own*
                // payload is the wait — `AObjAnimAdvance` post-increments, so
                // the read is from this word, not the next one.
                OP_SET_FLAGS => self.anim_wait += payload,
                // Ages the named tracks without setting a key.
                OP_ADD_LENGTH => {
                    for i in 0..TRACK_COUNT {
                        if flags & (1 << i) != 0 {
                            self.tracks[i].length += payload;
                        }
                    }
                }
                // Hands the `TraI` track a pointer to spline control points.
                // The pointer word is consumed; nothing reads `TraI` yet.
                OP_SET_INTERP => self.pc += 4,
                _ => {
                    let per =
                        values_per_track(opcode).ok_or(AnimError::UnknownOpcode { opcode, at })?;
                    self.pc = self.apply(data, opcode, flags, payload, per, speed)?;
                    if blocks(opcode) {
                        self.anim_wait += payload;
                    }
                }
            }
            if self.anim_wait > 0.0 {
                return Ok(());
            }
        }
        Err(AnimError::TooLong)
    }

    /// Sets the tracks a command names, returning the new program counter.
    fn apply(
        &mut self,
        data: &[u8],
        opcode: u32,
        flags: u16,
        payload: f32,
        per: usize,
        speed: f32,
    ) -> Result<usize, AnimError> {
        let mut pc = self.pc;
        for i in 0..TRACK_COUNT {
            if flags & (1 << i) == 0 {
                continue;
            }
            let value = f32_at(data, pc).ok_or(AnimError::Truncated { at: pc })?;
            pc += 4;
            let second = if per == 2 {
                let v = f32_at(data, pc).ok_or(AnimError::Truncated { at: pc })?;
                pc += 4;
                Some(v)
            } else {
                None
            };

            let t = &mut self.tracks[i];
            t.value_base = t.value_target;
            t.value_target = value;
            t.length = -self.anim_wait - speed;
            if payload != 0.0 {
                t.length_invert = 1.0 / payload;
            }

            match opcode {
                // Cubic with a zero outgoing rate.
                OP_SET_VAL0_RATE_BLOCK | OP_SET_VAL0_RATE => {
                    t.rate_base = t.rate_target;
                    t.rate_target = 0.0;
                    t.kind = Kind::Cubic;
                }
                // Cubic with both rates given.
                OP_SET_VAL_RATE_BLOCK | OP_SET_VAL_RATE => {
                    t.rate_base = t.rate_target;
                    t.rate_target = second.unwrap_or(0.0);
                    t.kind = Kind::Cubic;
                }
                // Only the outgoing rate changes; the value is a target.
                OP_SET_TARGET_RATE => {
                    t.rate_target = value;
                    t.value_target = t.value_base;
                    t.kind = Kind::Cubic;
                }
                // Steps to the value once `payload` frames have passed —
                // `length_invert` holds the switch-over frame uninverted here,
                // exactly as the 16-bit `Step` does.
                OP_SET_VAL_AFTER_BLOCK | OP_SET_VAL_AFTER => {
                    t.length_invert = payload;
                    t.kind = Kind::Step;
                }
                // Linear ramp over `payload` frames.
                _ => {
                    t.rate_base = if payload != 0.0 {
                        (t.value_target - t.value_base) / payload
                    } else {
                        0.0
                    };
                    t.rate_target = 0.0;
                    t.kind = Kind::Linear;
                }
            }
        }
        Ok(pc)
    }

    /// `gcPlayDObjAnimJoint`: advance every live track and write the pose.
    fn play(&mut self, speed: f32, pose: &mut JointPose) {
        for (track, aobj) in self.tracks.iter_mut().enumerate() {
            if aobj.kind == Kind::None {
                continue;
            }
            if !self.ended {
                aobj.length += speed;
            }
            let value = aobj.value();
            match track {
                0..=2 => pose.rotate[track] = value,
                4..=6 => pose.translate[track - 4] = value,
                7..=9 => pose.scale[track - 7] = value,
                // TraI, the spline-translation fraction, needs the control
                // points opcode 12 would set; nothing reads it here.
                _ => {}
            }
        }
    }

    /// Whether the script has looped back to where it began.
    pub fn looped(&self) -> bool {
        self.pc == self.start
    }
}

/// Reads the `AObjEvent32 *anim_joints[]` table for a layer of `nodes` nodes.
///
/// Entry `i` belongs to graph node `i`, the way `gcAddAnimJointAll` hands them
/// out. A NULL entry means that node is not animated.
pub fn joint_scripts(data: &[u8], table: u32, nodes: usize) -> alloc::vec::Vec<Option<u32>> {
    (0..nodes)
        .map(|i| u32_at(data, table as usize + i * 4).filter(|&s| s != 0))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;

    /// Builds a script: `(opcode, flags, payload)` commands with `f32` values.
    fn script(words: &[u32]) -> Vec<u8> {
        words.iter().flat_map(|w| w.to_be_bytes()).collect()
    }

    const fn cmd(op: u32, flags: u32, payload: u32) -> u32 {
        (op << 25) | (flags << 15) | payload
    }

    #[test]
    fn a_linear_ramp_reaches_its_target_after_its_payload() {
        // SetValBlock on TraY over 4 frames, to 100.0, then End.
        let data = script(&[
            cmd(OP_SET_VAL_BLOCK, 1 << 5, 4),
            100.0f32.to_bits(),
            cmd(OP_END, 0, 0),
        ]);
        let mut j = StageJoint::start(0, 0.0);
        let mut pose = JointPose {
            rotate: [0.0; 3],
            translate: [0.0; 3],
            scale: [1.0; 3],
        };
        for _ in 0..4 {
            j.tick(&data, 1.0, &mut pose).expect("ticks");
        }
        // Base is 0 and the rate is target/payload, so four frames arrive.
        assert!(
            (pose.translate[1] - 100.0).abs() < 0.01,
            "got {}",
            pose.translate[1]
        );
    }

    /// The 32-bit stream stores real floats. Reading them through figatree's
    /// `s16` scale factors would divide a rotation by 512.
    #[test]
    fn values_are_read_as_floats_not_fixed_point() {
        let data = script(&[
            cmd(OP_SET_VAL_BLOCK, 1, 1),
            core::f32::consts::FRAC_PI_2.to_bits(),
            cmd(OP_END, 0, 0),
        ]);
        let mut j = StageJoint::start(0, 0.0);
        let mut pose = JointPose {
            rotate: [0.0; 3],
            translate: [0.0; 3],
            scale: [1.0; 3],
        };
        j.tick(&data, 1.0, &mut pose).expect("ticks");
        assert!(
            (pose.rotate[0] - core::f32::consts::FRAC_PI_2).abs() < 0.001,
            "got {}",
            pose.rotate[0]
        );
    }

    #[test]
    fn an_unknown_opcode_is_an_error_not_a_skip() {
        let data = script(&[cmd(23, 0, 0), 0]);
        let mut j = StageJoint::start(0, 0.0);
        let mut pose = JointPose {
            rotate: [0.0; 3],
            translate: [0.0; 3],
            scale: [1.0; 3],
        };
        assert!(matches!(
            j.tick(&data, 1.0, &mut pose),
            Err(AnimError::UnknownOpcode { opcode: 23, .. })
        ));
    }

    #[test]
    fn a_null_table_entry_means_an_unanimated_node() {
        let data = script(&[0, 0x40, 0]);
        assert_eq!(
            joint_scripts(&data, 0, 3),
            alloc::vec![None, Some(0x40), None]
        );
    }
}
