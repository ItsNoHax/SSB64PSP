//! Figatree animation scripts, and the state machine that plays them.
//!
//! [`anim`](crate::anim) walks these scripts far enough to learn how long an
//! animation lasts. This module reads the rest: the per-joint transform tracks,
//! and the interpolation the original runs over them.
//!
//! ## The format
//!
//! A figatree file opens with a pointer table, one script offset per model
//! joint. Each script is a stream of 16-bit commands:
//!
//! ```text
//! bit  15..11  10..1   0
//!      opcode  flags   toggle
//! ```
//!
//! `flags` is a bitmask over the ten joint tracks (`RotX`..`ScaZ`). `toggle`
//! says whether a payload word follows the command; the payload is the number
//! of frames the command interpolates over, and for the six `Block` opcodes it
//! is also how long the command holds up the clock. After the payload come the
//! value words, one or two per set track bit depending on the opcode.
//!
//! ## The state machine
//!
//! Every track carries an `AObj`: a base value, a target value, base and target
//! rates, a running length and the reciprocal of its duration. A command
//! rewrites the `AObj`s of the tracks it names — pushing the old target down to
//! the base — and the pose for a frame is then read back out of them by
//! evaluating a cubic Hermite (or a line, or a step) at the current length.
//!
//! That indirection is what lets one command block the clock for 11 frames
//! while a track it set earlier interpolates over 26: the clock and each
//! track's duration are separate numbers.
//!
//! Ported from `ftAnimParseDObjFigatree` (0x800EC238), `gcPlayDObjAnimJoint`
//! and `ftAnimGetTargetValue` (0x800EC160).

/// Number of joint tracks a command's `flags` field can name.
pub const TRACK_COUNT: usize = 10;

/// Track indices, matching `AObjTrackKind` minus `nGCAnimTrackJointStart`.
pub const TRACK_ROT_X: usize = 0;
pub const TRACK_ROT_Y: usize = 1;
pub const TRACK_ROT_Z: usize = 2;
/// Translation along a spline. Never set by a gameplay animation: applying it
/// needs the control points that only opcode 12 supplies, and no fighter
/// figatree in the ROM contains an opcode 12.
pub const TRACK_TRA_I: usize = 3;
pub const TRACK_TRA_X: usize = 4;
pub const TRACK_TRA_Y: usize = 5;
pub const TRACK_TRA_Z: usize = 6;
pub const TRACK_SCA_X: usize = 7;
pub const TRACK_SCA_Y: usize = 8;
pub const TRACK_SCA_Z: usize = 9;

const OP_END: u16 = 0;
const OP_BLOCK: u16 = 1;
const OP_SET_VAL_BLOCK: u16 = 2;
const OP_SET_VAL: u16 = 3;
const OP_SET_VAL_RATE_BLOCK: u16 = 4;
const OP_SET_VAL_RATE: u16 = 5;
const OP_SET_TARGET_RATE: u16 = 6;
const OP_SET_VAL0_RATE_BLOCK: u16 = 7;
const OP_SET_VAL0_RATE: u16 = 8;
const OP_SET_VAL_AFTER_BLOCK: u16 = 9;
const OP_SET_VAL_AFTER: u16 = 10;
const OP_ADD_LENGTH: u16 = 11;
const OP_TRANSLATE_INTERP: u16 = 12;
const OP_LOOP: u16 = 13;
const OP_SET_FLAGS: u16 = 14;

/// Commands whose payload advances the animation clock (`anim_wait += payload`).
/// The non-`Block` variants set a track's interpolation length without
/// consuming time.
pub(crate) const fn is_block(opcode: u16) -> bool {
    matches!(
        opcode,
        OP_BLOCK
            | OP_SET_VAL_BLOCK
            | OP_SET_VAL_RATE_BLOCK
            | OP_SET_VAL0_RATE_BLOCK
            | OP_SET_VAL_AFTER_BLOCK
            | OP_SET_FLAGS
    )
}

/// Value words each command reads per set track flag.
pub(crate) const fn values_per_track(opcode: u16) -> usize {
    match opcode {
        // A (value, rate) pair per track.
        OP_SET_VAL_RATE_BLOCK | OP_SET_VAL_RATE => 2,
        OP_SET_VAL_BLOCK
        | OP_SET_VAL
        | OP_SET_TARGET_RATE
        | OP_SET_VAL0_RATE_BLOCK
        | OP_SET_VAL0_RATE
        | OP_SET_VAL_AFTER_BLOCK
        | OP_SET_VAL_AFTER => 1,
        _ => 0,
    }
}

/// A script ran off the end of its file, which means the command stream
/// desynchronised — a command's word count was read wrong.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Desynchronised {
    /// Byte offset the read was attempted at.
    pub at: usize,
}

impl core::fmt::Display for Desynchronised {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "animation script ran past its end at byte {}", self.at)
    }
}

#[cfg(feature = "std")]
impl std::error::Error for Desynchronised {}

fn u16_be(data: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_be_bytes([*data.get(at)?, *data.get(at + 1)?]))
}

/// One decoded command, with the cursor left after it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Command {
    pub opcode: u16,
    /// Bitmask of the tracks the command names, bit *n* for track *n*.
    pub flags: u16,
    /// Frames the command interpolates over, and — for a `Block` opcode — how
    /// long it holds up the clock. Zero when the command carries no payload.
    pub payload: u16,
    /// Byte offset of the first value word.
    values_at: usize,
    /// Byte offset just past the command.
    pub next: usize,
}

impl Command {
    /// Value word `i` of the command's trailing values.
    pub fn value(&self, data: &[u8], i: usize) -> i16 {
        u16_be(data, self.values_at + i * 2).unwrap_or(0) as i16
    }
}

/// Decodes the command at `at`.
///
/// `Loop` and `TranslateInterp` carry a jump offset in place of track values;
/// [`loop_target`] resolves the former's.
pub fn command(data: &[u8], at: usize) -> Result<Command, Desynchronised> {
    let word = u16_be(data, at).ok_or(Desynchronised { at })?;
    let (opcode, flags, toggle) = (word >> 11, (word >> 1) & 0x3FF, word & 1);
    let mut next = at + 2;

    // Loop and TranslateInterp carry a jump offset instead of track values.
    if opcode == OP_LOOP || opcode == OP_TRANSLATE_INTERP {
        return Ok(Command {
            opcode,
            flags,
            payload: 0,
            values_at: next,
            next: next + 2,
        });
    }

    let mut payload = 0;
    if toggle == 1 {
        payload = u16_be(data, next).ok_or(Desynchronised { at: next })?;
        next += 2;
    }
    let values_at = next;
    let words = values_per_track(opcode) * flags.count_ones() as usize;
    next += words * 2;
    if next > data.len() {
        return Err(Desynchronised { at: values_at });
    }
    Ok(Command {
        opcode,
        flags,
        payload,
        values_at,
        next,
    })
}

/// Where a `Loop` command jumps to.
///
/// The offset is a signed byte displacement from the word that holds it, which
/// is the word after the command:
///
/// ```c
/// AObjAnimAdvance(event16);                  // now at the offset word
/// event16 += event16->s / 2;                 // in u16 elements
/// ```
pub fn loop_target(data: &[u8], cmd: &Command) -> Result<usize, Desynchronised> {
    let at = cmd.values_at;
    let offset = u16_be(data, at).ok_or(Desynchronised { at })? as i16;
    // `s / 2` truncates toward zero in C, then indexes u16 elements: two bytes
    // each, so the byte displacement is that product.
    let words = (offset as i32) / 2;
    let target = at as i32 + words * 2;
    if target < 0 || target as usize >= data.len() {
        return Err(Desynchronised { at });
    }
    Ok(target as usize)
}

/// How a track's value is read back between keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum Kind {
    #[default]
    None,
    Step,
    Linear,
    Cubic,
}

/// One track's interpolation state (`AObj`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Aobj {
    pub(crate) kind: Kind,
    /// Frames since the key that set this track.
    pub(crate) length: f32,
    /// Reciprocal of the track's duration — except under `Step`, where the
    /// original reuses the field for the *un*inverted switch-over frame.
    pub(crate) length_invert: f32,
    pub(crate) value_base: f32,
    pub(crate) value_target: f32,
    pub(crate) rate_base: f32,
    pub(crate) rate_target: f32,
}

impl Default for Aobj {
    /// `gcAddAObjForDObj`: everything zero but `length_invert`, which is 1.
    fn default() -> Self {
        Aobj {
            kind: Kind::None,
            length: 0.0,
            length_invert: 1.0,
            value_base: 0.0,
            value_target: 0.0,
            rate_base: 0.0,
            rate_target: 0.0,
        }
    }
}

impl Aobj {
    /// The track's value at its current length.
    pub(crate) fn value(&self) -> f32 {
        match self.kind {
            Kind::None => 0.0,
            Kind::Linear => self.value_base + self.length * self.rate_base,
            Kind::Step => {
                if self.length_invert <= self.length {
                    self.value_target
                } else {
                    self.value_base
                }
            }
            // The cubic Hermite `gcPlayDObjAnimJoint` inlines, with `length`
            // measured in frames and `length_invert` normalising it.
            Kind::Cubic => {
                let inv2 = self.length_invert * self.length_invert;
                let len2 = self.length * self.length;
                let a = self.length_invert * len2;
                let b = self.length * len2 * inv2;
                let c = 2.0 * b * self.length_invert;
                let d = 3.0 * len2 * inv2;
                let e = b - a;

                self.value_base * (c - d + 1.0)
                    + self.value_target * (d - c)
                    + self.rate_base * (e - a + self.length)
                    + self.rate_target * e
            }
        }
    }
}

/// Scale factors `ftAnimGetTargetValue` applies to a raw `s16`, by track group.
///
/// Rotations come out in radians, translations and scales in model units. The
/// two `1/16384 - 3e-12` entries are the original's constants verbatim; the
/// bias is presumably an exporter artefact.
fn target_value(raw: i16, track: usize, is_rate: bool) -> f32 {
    const VALUE: [f32; 4] = [
        1.0 / 512.0,             // rotation
        1.0 / 4.0,               // translation
        1.0 / 4096.0,            // scale
        1.0 / 16384.0 - 3.0e-12, // translation-interpolation fraction
    ];
    const RATE: [f32; 4] = [
        1.0 / 512.0,
        1.0 / 32.0,
        1.0 / 8192.0,
        1.0 / 16384.0 - 3.0e-12,
    ];
    let group = match track {
        TRACK_ROT_X | TRACK_ROT_Y | TRACK_ROT_Z => 0,
        TRACK_TRA_X | TRACK_TRA_Y | TRACK_TRA_Z => 1,
        TRACK_SCA_X | TRACK_SCA_Y | TRACK_SCA_Z => 2,
        _ => 3,
    };
    raw as f32 * if is_rate { RATE[group] } else { VALUE[group] }
}

/// A joint's local transform, as `gcPlayDObjAnimJoint` writes it.
///
/// Only the tracks an animation actually names are overwritten, so this starts
/// from the joint's rest pose and is updated in place.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct JointPose {
    /// Euler rotation in radians, applied Z then Y then X (N64 convention).
    pub rotate: [f32; 3],
    pub translate: [f32; 3],
    pub scale: [f32; 3],
}

impl Default for JointPose {
    fn default() -> Self {
        JointPose {
            rotate: [0.0; 3],
            translate: [0.0; 3],
            scale: [1.0; 3],
        }
    }
}

/// Where a joint's clock stands, mirroring the `anim_wait` sentinels
/// (`AOBJ_ANIM_NULL`, `AOBJ_ANIM_CHANGED`, `AOBJ_ANIM_END`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Clock {
    /// A new animation was just set; the first tick seeds the clock from the
    /// frame it was told to start at.
    Changed,
    Running,
    /// The script hit its terminator this tick. One more pose is read out of
    /// the tracks, and then the joint goes inert.
    Ended,
    /// Nothing is playing. The pose stays where it was left.
    Inert,
}

/// One joint's animation playback: its ten tracks and its script cursor.
///
/// A fighter needs one of these per joint. They are independent — each joint
/// has its own script and its own clock — which is what makes the format's
/// self-check possible (see [`anim`](crate::anim)).
#[derive(Debug, Clone, PartialEq)]
pub struct JointAnim {
    tracks: [Aobj; TRACK_COUNT],
    /// Frames until the next command runs. Counts down by `speed` each tick.
    anim_wait: f32,
    /// Frames elapsed. Goes `<= 0` when the script ends, which is the sentinel
    /// the status machine tests (RE-035).
    anim_frame: f32,
    /// Byte offset of the next command.
    pc: usize,
    clock: Clock,
    /// Render flags the script last set (`nGCAnimEvent16SetFlags`).
    pub flags: u16,
}

impl JointAnim {
    /// A joint with no animation. Its pose is whatever it was given.
    pub fn inert() -> Self {
        JointAnim {
            tracks: [Aobj::default(); TRACK_COUNT],
            anim_wait: 0.0,
            anim_frame: 0.0,
            pc: 0,
            clock: Clock::Inert,
            flags: 0,
        }
    }

    /// Starts the script at byte offset `script`, from frame `frame`.
    ///
    /// `gcAddDObjAnimJoint`: the tracks are cleared to `None` rather than
    /// dropped, so a track the new animation never names keeps its rest value.
    pub fn start(script: usize, frame: f32) -> Self {
        JointAnim {
            tracks: [Aobj::default(); TRACK_COUNT],
            anim_wait: 0.0,
            anim_frame: frame,
            pc: script,
            clock: Clock::Changed,
            flags: 0,
        }
    }

    /// Frames elapsed. `<= 0` once the script has ended.
    pub fn frame(&self) -> f32 {
        self.anim_frame
    }

    /// Whether the script has run out and the joint has stopped moving.
    pub fn ended(&self) -> bool {
        matches!(self.clock, Clock::Ended | Clock::Inert)
    }

    /// Advances one tick and writes the resulting pose.
    ///
    /// `speed` is the playback rate: 1.0 normally, 0.5 for a heavy landing
    /// (RE-035). Runs `ftAnimParseDObjFigatree` and then `gcPlayDObjAnimJoint`,
    /// in that order — the same order the original's per-frame walk uses.
    pub fn tick(
        &mut self,
        data: &[u8],
        speed: f32,
        pose: &mut JointPose,
    ) -> Result<(), Desynchronised> {
        self.parse(data, speed)?;
        self.play(speed, pose);
        Ok(())
    }

    /// `ftAnimParseDObjFigatree` for one joint: runs commands until one blocks.
    fn parse(&mut self, data: &[u8], speed: f32) -> Result<(), Desynchronised> {
        match self.clock {
            Clock::Inert => return Ok(()),
            // `play` always converts Ended to Inert, so a parse never sees it.
            Clock::Ended => return Ok(()),
            Clock::Changed => {
                self.anim_wait = -self.anim_frame;
                self.clock = Clock::Running;
            }
            Clock::Running => {
                self.anim_wait -= speed;
                self.anim_frame += speed;
                if self.anim_wait > 0.0 {
                    return Ok(());
                }
            }
        }

        loop {
            let cmd = command(data, self.pc)?;
            self.pc = cmd.next;

            match cmd.opcode {
                OP_END => {
                    self.finish(speed);
                    return Ok(());
                }
                OP_LOOP => {
                    self.pc = loop_target(data, &cmd)?;
                    self.anim_frame = -self.anim_wait;
                }
                OP_BLOCK => self.anim_wait += cmd.payload as f32,
                OP_SET_FLAGS => {
                    self.flags = cmd.flags;
                    self.anim_wait += cmd.payload as f32;
                }
                // Sets the control points for a spline translation. No fighter
                // figatree in the ROM contains one, so the track it would feed
                // is never applied; skipping it keeps the walk in step.
                OP_TRANSLATE_INTERP => {}
                OP_ADD_LENGTH => {
                    for track in set_tracks(cmd.flags) {
                        self.tracks[track].length += cmd.payload as f32;
                    }
                }
                _ => {
                    self.set_tracks(data, &cmd, speed);
                    if is_block(cmd.opcode) {
                        self.anim_wait += cmd.payload as f32;
                    }
                }
            }

            if self.anim_wait > 0.0 {
                return Ok(());
            }
        }
    }

    /// Applies one value-setting command to the tracks it names.
    fn set_tracks(&mut self, data: &[u8], cmd: &Command, speed: f32) {
        let payload = cmd.payload as f32;
        // The value words are consecutive across the set tracks, not indexed
        // by track number: the nth set bit reads the nth group of words.
        let mut word = 0;
        for track in set_tracks(cmd.flags) {
            let aobj = &mut self.tracks[track];
            // Every key pushes the previous target down to the base, which is
            // what makes a command a *key* rather than an absolute pose.
            let length = -self.anim_wait - speed;

            match cmd.opcode {
                OP_SET_VAL0_RATE_BLOCK | OP_SET_VAL0_RATE => {
                    aobj.value_base = aobj.value_target;
                    aobj.value_target = target_value(cmd.value(data, word), track, false);
                    aobj.rate_base = aobj.rate_target;
                    aobj.rate_target = 0.0;
                    aobj.kind = Kind::Cubic;
                    if payload != 0.0 {
                        aobj.length_invert = 1.0 / payload;
                    }
                    aobj.length = length;
                    word += 1;
                }
                OP_SET_VAL_BLOCK | OP_SET_VAL => {
                    aobj.value_base = aobj.value_target;
                    aobj.value_target = target_value(cmd.value(data, word), track, false);
                    aobj.kind = Kind::Linear;
                    if payload != 0.0 {
                        aobj.rate_base = (aobj.value_target - aobj.value_base) / payload;
                    }
                    aobj.length = length;
                    aobj.rate_target = 0.0;
                    word += 1;
                }
                OP_SET_VAL_RATE_BLOCK | OP_SET_VAL_RATE => {
                    aobj.value_base = aobj.value_target;
                    aobj.value_target = target_value(cmd.value(data, word), track, false);
                    aobj.rate_base = aobj.rate_target;
                    aobj.rate_target = target_value(cmd.value(data, word + 1), track, true);
                    aobj.kind = Kind::Cubic;
                    if payload != 0.0 {
                        aobj.length_invert = 1.0 / payload;
                    }
                    aobj.length = length;
                    word += 2;
                }
                OP_SET_TARGET_RATE => {
                    // Retargets the outgoing rate without disturbing the
                    // value, the length or the kind.
                    aobj.rate_target = target_value(cmd.value(data, word), track, true);
                    word += 1;
                }
                OP_SET_VAL_AFTER_BLOCK | OP_SET_VAL_AFTER => {
                    aobj.value_base = aobj.value_target;
                    aobj.value_target = target_value(cmd.value(data, word), track, false);
                    aobj.kind = Kind::Step;
                    // Not inverted here: under Step the field is compared
                    // against `length` directly, as the frame to switch on.
                    aobj.length_invert = payload;
                    aobj.length = length;
                    aobj.rate_target = 0.0;
                    word += 1;
                }
                _ => {}
            }
        }
    }

    /// The terminator's epilogue, shared by `End` and by running out of script.
    fn finish(&mut self, speed: f32) {
        for aobj in self.tracks.iter_mut() {
            if aobj.kind != Kind::None {
                aobj.length += speed + self.anim_wait;
            }
        }
        self.anim_frame = self.anim_wait;
        self.clock = Clock::Ended;
    }

    /// `gcPlayDObjAnimJoint`: ages every live track by one tick and reads the
    /// pose back out of it.
    fn play(&mut self, speed: f32, pose: &mut JointPose) {
        if self.clock == Clock::Inert {
            return;
        }
        for (track, aobj) in self.tracks.iter_mut().enumerate() {
            if aobj.kind == Kind::None {
                continue;
            }
            if self.clock != Clock::Ended {
                aobj.length += speed;
            }
            let value = aobj.value();
            match track {
                TRACK_ROT_X | TRACK_ROT_Y | TRACK_ROT_Z => pose.rotate[track] = value,
                TRACK_TRA_X | TRACK_TRA_Y | TRACK_TRA_Z => {
                    pose.translate[track - TRACK_TRA_X] = value
                }
                TRACK_SCA_X | TRACK_SCA_Y | TRACK_SCA_Z => pose.scale[track - TRACK_SCA_X] = value,
                // TraI needs opcode 12's control points, which no fighter
                // animation carries.
                _ => {}
            }
        }
        if self.clock == Clock::Ended {
            self.clock = Clock::Inert;
        }
    }
}

/// The track indices a command's `flags` field names, lowest bit first.
///
/// The original stops at the first zero *remaining* mask rather than scanning
/// all ten bits, which is the same set.
fn set_tracks(flags: u16) -> impl Iterator<Item = usize> {
    (0..TRACK_COUNT).filter(move |i| flags & (1 << i) != 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    const fn cmd(op: u16, flags: u16, toggle: u16) -> u16 {
        (op << 11) | (flags << 1) | toggle
    }

    const ROTX: u16 = 1 << TRACK_ROT_X;
    const TRAX: u16 = 1 << TRACK_TRA_X;
    const TRAY: u16 = 1 << TRACK_TRA_Y;

    fn script(words: &[u16]) -> alloc::vec::Vec<u8> {
        words.iter().flat_map(|w| w.to_be_bytes()).collect()
    }

    /// Runs `frames` ticks from a fresh start and returns the pose after each.
    fn run(words: &[u16], frames: usize, speed: f32) -> alloc::vec::Vec<JointPose> {
        let data = script(words);
        let mut anim = JointAnim::start(0, 0.0);
        let mut pose = JointPose::default();
        (0..frames)
            .map(|_| {
                anim.tick(&data, speed, &mut pose).unwrap();
                pose
            })
            .collect()
    }

    #[test]
    fn a_step_track_holds_its_base_until_the_switch_over_frame() {
        // SetValAfterBlockT(ROTX, 4) with target 512 raw = 1.0 rad. Step holds
        // value_base until `length` reaches the payload.
        let words = [
            cmd(OP_SET_VAL_AFTER_BLOCK, ROTX, 1),
            4,
            512,
            cmd(OP_END, 0, 0),
        ];
        let poses = run(&words, 6, 1.0);
        let rot: alloc::vec::Vec<f32> = poses.iter().map(|p| p.rotate[0]).collect();
        assert_eq!(rot, [0.0, 0.0, 0.0, 0.0, 1.0, 1.0]);
    }

    #[test]
    fn a_linear_track_walks_from_its_base_to_its_target_over_the_payload() {
        // SetValBlockT(TRAX, 4) to 16 raw = 4.0 units, from a base of 0.
        let words = [cmd(OP_SET_VAL_BLOCK, TRAX, 1), 4, 16, cmd(OP_END, 0, 0)];
        let poses = run(&words, 5, 1.0);
        let x: alloc::vec::Vec<f32> = poses.iter().map(|p| p.translate[0]).collect();
        assert_eq!(x, [0.0, 1.0, 2.0, 3.0, 4.0]);
    }

    #[test]
    fn a_cubic_track_starts_and_ends_on_its_keys_with_the_rates_it_was_given() {
        // SetValRateBlockT(TRAY, 8): value 0 -> 32 raw (8.0 units), rates zero,
        // so it must ease in and out and hit both endpoints exactly.
        let words = [
            cmd(OP_SET_VAL_RATE_BLOCK, TRAY, 1),
            8,
            0,
            0,
            cmd(OP_SET_VAL_RATE_BLOCK, TRAY, 1),
            8,
            32,
            0,
            cmd(OP_END, 0, 0),
        ];
        let poses = run(&words, 17, 1.0);
        let y: alloc::vec::Vec<f32> = poses.iter().map(|p| p.translate[1]).collect();
        assert_eq!(y[0], 0.0, "first frame sits on the first key");
        assert_eq!(y[8], 0.0, "the second key starts where the first ended");
        assert!(
            (y[16] - 8.0).abs() < 1e-4,
            "last frame reaches the target: {}",
            y[16]
        );
        // Eased, not linear: the midpoint of a Hermite with zero end rates is
        // exactly halfway, but the quarter point is well below a straight line.
        assert!((y[12] - 4.0).abs() < 1e-4, "midpoint: {}", y[12]);
        assert!(y[10] < 4.0 * 0.5, "eases in rather than ramping: {}", y[10]);
    }

    #[test]
    fn a_non_block_command_sets_a_duration_without_spending_the_clock() {
        // SetValRateT(TRAY, 20) interpolates over 20 frames; the Block after it
        // only holds the clock for 4. The track must still be mid-flight when
        // the clock has run out.
        let words = [
            cmd(OP_SET_VAL_RATE, TRAY, 1),
            20,
            80,
            0,
            cmd(OP_BLOCK, 0, 1),
            4,
            cmd(OP_END, 0, 0),
        ];
        let poses = run(&words, 5, 1.0);
        let y = poses[4].translate[1];
        assert!(y > 0.0 && y < 20.0, "still interpolating at frame 4: {y}");
    }

    #[test]
    fn half_speed_takes_twice_as_long() {
        // The heavy-landing case: the same script at speed 0.5 (RE-035).
        let words = [cmd(OP_SET_VAL_BLOCK, TRAX, 1), 4, 16, cmd(OP_END, 0, 0)];
        let full = run(&words, 5, 1.0);
        let half = run(&words, 9, 0.5);
        assert_eq!(full[4].translate[0], half[8].translate[0]);
        assert_eq!(full[2].translate[0], half[4].translate[0]);
    }

    #[test]
    fn an_ended_script_stops_advancing_and_holds_its_last_pose() {
        let words = [cmd(OP_SET_VAL_BLOCK, TRAX, 1), 2, 8, cmd(OP_END, 0, 0)];
        let data = script(&words);
        let mut anim = JointAnim::start(0, 0.0);
        let mut pose = JointPose::default();
        for _ in 0..3 {
            anim.tick(&data, 1.0, &mut pose).unwrap();
        }
        assert!(anim.ended());
        assert!(anim.frame() <= 0.0, "the status machine's end sentinel");
        let held = pose.translate[0];
        for _ in 0..10 {
            anim.tick(&data, 1.0, &mut pose).unwrap();
        }
        assert_eq!(pose.translate[0], held);
    }

    #[test]
    fn a_loop_jumps_back_and_the_frame_counter_never_goes_negative() {
        // Block 3, then loop back to the Block. -6 bytes from the offset word.
        let words = [cmd(OP_BLOCK, 0, 1), 3, cmd(OP_LOOP, 0, 0), (-6i16) as u16];
        let data = script(&words);
        let cmd0 = command(&data, 4).unwrap();
        assert_eq!(loop_target(&data, &cmd0).unwrap(), 0);

        let mut anim = JointAnim::start(0, 0.0);
        let mut pose = JointPose::default();
        for _ in 0..20 {
            anim.tick(&data, 1.0, &mut pose).unwrap();
            assert!(!anim.ended(), "a looping script never ends");
        }
    }

    #[test]
    fn the_values_of_a_multi_track_command_are_read_in_bit_order() {
        // One command setting ROTX and TRAX: the first value word belongs to
        // the lower track bit. Swapping them would put a rotation in a
        // translation, which is exactly the bug this guards.
        let words = [
            cmd(OP_SET_VAL_BLOCK, ROTX | TRAX, 1),
            1,
            512, // ROTX: 1.0 rad
            16,  // TRAX: 4.0 units
            cmd(OP_END, 0, 0),
        ];
        let poses = run(&words, 2, 1.0);
        assert_eq!(poses[1].rotate[0], 1.0);
        assert_eq!(poses[1].translate[0], 4.0);
    }

    #[test]
    fn untouched_tracks_keep_the_rest_pose_they_started_from() {
        let words = [cmd(OP_SET_VAL_BLOCK, TRAX, 1), 1, 16, cmd(OP_END, 0, 0)];
        let data = script(&words);
        let mut anim = JointAnim::start(0, 0.0);
        let mut pose = JointPose {
            rotate: [0.5, 0.5, 0.5],
            translate: [1.0, 2.0, 3.0],
            scale: [2.0, 2.0, 2.0],
        };
        anim.tick(&data, 1.0, &mut pose).unwrap();
        assert_eq!(pose.rotate, [0.5, 0.5, 0.5], "no rotation track was named");
        assert_eq!(pose.scale, [2.0, 2.0, 2.0], "no scale track was named");
        assert_eq!(pose.translate[1..], [2.0, 3.0], "only X was named");
    }

    #[test]
    fn a_truncated_script_desynchronises_rather_than_reading_zeros() {
        // A two-track command whose value words were cut off.
        let words = [cmd(OP_SET_VAL_BLOCK, ROTX | TRAX, 1), 1, 512];
        let data = script(&words);
        assert!(command(&data, 0).is_err());
    }

    #[test]
    fn rotations_translations_and_scales_use_their_own_scale_factors() {
        assert_eq!(target_value(512, TRACK_ROT_X, false), 1.0);
        assert_eq!(target_value(4, TRACK_TRA_Y, false), 1.0);
        assert_eq!(target_value(4096, TRACK_SCA_Z, false), 1.0);
        // Rates differ from values for translation and scale, and do not for
        // rotation — the detail a single table would have got wrong.
        assert_eq!(target_value(512, TRACK_ROT_X, true), 1.0);
        assert_eq!(target_value(32, TRACK_TRA_Y, true), 1.0);
        assert_eq!(target_value(8192, TRACK_SCA_Z, true), 1.0);
    }
}
