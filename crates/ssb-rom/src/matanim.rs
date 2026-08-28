//! Per-costume material colours (`AObjEvent32` material animation joints).
//!
//! A fighter's flat-shaded parts get their colour from `G_SETPRIMCOLOR`, and
//! the value baked into `MObjSub` is a placeholder: the last costume's, as it
//! happens. The real one is written over it at setup time from a third pointer
//! in `FTCommonPart`, next to the two [`mobj`](crate::mobj) already reads:
//!
//! ```c
//! gcAddMObjMatAnimJoint(mobj, costume_matanim_joint, anim_frame);
//! gcParseMObjMatAnimJoint(mobj);
//! gcPlayMObjMatAnim(mobj);
//! gcRemoveAObjFromMObj(mobj);          // and then thrown away
//! ```
//!
//! `anim_frame` there is `fp->costume`, which is the whole trick: **one script
//! per material holds every costume, one per frame.** Mario's upper arm reads
//!
//! ```text
//! SetExtValAfterBlock(PrimColor, 0)  ff0000ff   costume 0 — red
//! SetExtValAfterBlock(PrimColor, 1)  ffe700ff   costume 1 — yellow
//! SetExtValAfterBlock(PrimColor, 1)  f7e78cff   costume 2
//! SetExtValAfterBlock(PrimColor, 1)  5242ffff   costume 3 — blue
//! SetExtValAfter(PrimColor, 1)       00ce00ff   costume 4 — green
//! ```
//!
//! and it is that last entry which was ending up on his sleeves.
//!
//! ## The 32-bit encoding
//!
//! Unrelated to the 16-bit figatree stream [`figatree`](crate::figatree)
//! plays. A command is one `u32`:
//!
//! ```text
//! bits  31..25   24..15   14..0
//!       opcode   flags    payload
//! ```
//!
//! `flags` is a bitmask over tracks and `payload` is the duration — both in the
//! command word, with no `toggle`. Each set track is then followed by one
//! `u32` of value. For a colour track those four bytes *are* the colour:
//! `gcPlayMObjMatAnim` reinterprets the slot rather than converting it.

use crate::archive::File;

/// Colour tracks, `nGCAnimTrackMaterialSubStart` onwards.
pub const TRACK_PRIM: usize = 0;
pub const TRACK_ENV: usize = 1;
pub const TRACK_BLEND: usize = 2;
const TRACK_COUNT: usize = 5;

/// Joint tracks a command can also name (`nGCAnimTrackMaterialStart`
/// onwards): texture ids, UV translate/scale, scroll, palette id. Decoded only
/// far enough to step over their values.
const MAT_TRACK_COUNT: usize = 10;

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
const OP_EXT_VAL_AFTER_BLOCK: u32 = 18;
const OP_EXT_VAL_AFTER: u32 = 19;
const OP_EXT_VAL_BLOCK: u32 = 20;
const OP_EXT_VAL: u32 = 21;

/// What went wrong reading a material animation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatAnimError {
    /// The script ran past the end of its file.
    Truncated { at: usize },
    /// A command this decoder does not model. Returned rather than skipped:
    /// guessing a word count desynchronises the stream, and a colour read from
    /// a desynchronised stream looks like a colour.
    UnknownOpcode { opcode: u32, at: usize },
    /// The script looped for longer than any costume list plausibly runs.
    TooLong,
}

impl core::fmt::Display for MatAnimError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            MatAnimError::Truncated { at } => {
                write!(f, "material animation ran past its end at {at}")
            }
            MatAnimError::UnknownOpcode { opcode, at } => {
                write!(
                    f,
                    "material animation opcode {opcode} at {at} is not modelled"
                )
            }
            MatAnimError::TooLong => write!(f, "material animation did not terminate"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for MatAnimError {}

/// The colours one material ends up with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Colors {
    pub prim: Option<[u8; 4]>,
    pub env: Option<[u8; 4]>,
    pub blend: Option<[u8; 4]>,
}

/// One colour track's interpolation state, holding raw `SYColorPack` words.
#[derive(Clone, Copy, Default)]
struct Track {
    live: bool,
    /// `nGCAnimKindStep`, the only kind a costume list uses.
    step: bool,
    base: u32,
    target: u32,
    length: f32,
    length_invert: f32,
}

impl Track {
    /// `gcPlayMObjMatAnim`'s `nGCAnimKindStep` branch, on the raw bytes.
    fn color(&self) -> [u8; 4] {
        let v = if self.length_invert <= self.length {
            self.target
        } else {
            self.base
        };
        v.to_be_bytes()
    }
}

fn u32_at(data: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes(data.get(at..at + 4)?.try_into().ok()?))
}

/// How many value words a command reads per set track.
fn values_per_track(opcode: u32) -> Option<usize> {
    Some(match opcode {
        OP_SET_VAL_RATE_BLOCK | OP_SET_VAL_RATE => 2,
        OP_SET_VAL_BLOCK
        | OP_SET_VAL
        | OP_SET_TARGET_RATE
        | OP_SET_VAL0_RATE_BLOCK
        | OP_SET_VAL0_RATE
        | OP_SET_VAL_AFTER_BLOCK
        | OP_SET_VAL_AFTER
        | OP_EXT_VAL_AFTER_BLOCK
        | OP_EXT_VAL_AFTER
        | OP_EXT_VAL_BLOCK
        | OP_EXT_VAL => 1,
        OP_END | OP_JUMP | OP_WAIT => 0,
        _ => return None,
    })
}

/// Whether a command's payload advances the clock.
fn blocks(opcode: u32) -> bool {
    matches!(
        opcode,
        OP_WAIT
            | OP_SET_VAL_BLOCK
            | OP_SET_VAL_RATE_BLOCK
            | OP_SET_VAL0_RATE_BLOCK
            | OP_SET_VAL_AFTER_BLOCK
            | OP_EXT_VAL_AFTER_BLOCK
            | OP_EXT_VAL_BLOCK
    )
}

/// Whether a command addresses the colour tracks rather than the joint ones.
fn is_ext(opcode: u32) -> bool {
    matches!(
        opcode,
        OP_EXT_VAL_AFTER_BLOCK | OP_EXT_VAL_AFTER | OP_EXT_VAL_BLOCK | OP_EXT_VAL
    )
}

/// Runs a material animation to `frame` and reports the colours it leaves.
///
/// `frame` is the costume index — see the module note. Faithful to
/// `gcParseMObjMatAnimJoint` followed by `gcPlayMObjMatAnim`, for the subset a
/// costume list uses; anything else is an error rather than a guess.
pub fn colors_at(data: &[u8], script: u32, frame: f32) -> Result<Colors, MatAnimError> {
    let mut tracks = [Track::default(); TRACK_COUNT];
    let mut pc = script as usize;
    // `anim_wait = -anim_frame` on the first parse, then commands run until one
    // blocks past it.
    let mut anim_wait = -frame;
    const SPEED: f32 = 1.0;

    for _ in 0..4096 {
        let word = u32_at(data, pc).ok_or(MatAnimError::Truncated { at: pc })?;
        let opcode = word >> 25;
        let flags = (word >> 15) & 0x3FF;
        let payload = (word & 0x7FFF) as f32;
        pc += 4;

        if opcode == OP_END {
            break;
        }
        if opcode == OP_JUMP {
            // The target is the next word. A costume list has no reason to
            // jump, so rather than follow it into unknown territory, stop.
            return Err(MatAnimError::UnknownOpcode { opcode, at: pc - 4 });
        }
        let per_track =
            values_per_track(opcode).ok_or(MatAnimError::UnknownOpcode { opcode, at: pc - 4 })?;

        let count = if is_ext(opcode) {
            TRACK_COUNT
        } else {
            MAT_TRACK_COUNT
        };
        let mut bits = flags;
        #[allow(clippy::needless_range_loop)] // `count` differs from tracks.len()
        for i in 0..count {
            if bits == 0 {
                break;
            }
            if bits & 1 != 0 {
                let value = u32_at(data, pc).ok_or(MatAnimError::Truncated { at: pc })?;
                if is_ext(opcode) {
                    let t = &mut tracks[i];
                    t.live = true;
                    t.base = t.target;
                    t.target = value;
                    // Only the `After` pair is a step; the others interpolate,
                    // which a costume list never does.
                    t.step = matches!(opcode, OP_EXT_VAL_AFTER_BLOCK | OP_EXT_VAL_AFTER);
                    t.length_invert = payload;
                    t.length = -anim_wait - SPEED;
                }
                pc += 4 * per_track;
            }
            bits >>= 1;
        }

        if blocks(opcode) {
            anim_wait += payload;
            if anim_wait > 0.0 {
                break;
            }
        }
    }

    // `gcPlayMObjMatAnim` ages each live track by one tick before reading it.
    let read = |i: usize| -> Option<[u8; 4]> {
        let mut t = tracks[i];
        if !t.live || !t.step {
            return None;
        }
        t.length += SPEED;
        Some(t.color())
    };
    Ok(Colors {
        prim: read(TRACK_PRIM),
        env: read(TRACK_ENV),
        blend: read(TRACK_BLEND),
    })
}

/// `FTCommonPart::p_costume_matanim_joints`, resolved to one script per
/// `(node, material)`.
///
/// The outer array is parallel to the `DObjDesc` array; each entry points at a
/// list of scripts parallel to that node's `MObjSub` chain, which is how
/// `lbCommonAddMObjForFighterPartsDObj` walks the two together.
pub fn costume_colors(
    file: &File,
    table: u32,
    nodes: usize,
    chain_len: impl Fn(usize) -> usize,
    costume: f32,
) -> alloc::vec::Vec<alloc::vec::Vec<Option<Colors>>> {
    (0..nodes)
        .map(|node| {
            let per_node = u32_at(&file.data, table as usize + node * 4).unwrap_or(0);
            (0..chain_len(node))
                .map(|m| {
                    if per_node == 0 {
                        return None;
                    }
                    let script = u32_at(&file.data, per_node as usize + m * 4)?;
                    if script == 0 {
                        return None;
                    }
                    colors_at(&file.data, script, costume).ok()
                })
                .collect()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn script(words: &[u32]) -> alloc::vec::Vec<u8> {
        words.iter().flat_map(|w| w.to_be_bytes()).collect()
    }

    const fn cmd(opcode: u32, flags: u32, payload: u32) -> u32 {
        (opcode << 25) | (flags << 15) | payload
    }

    /// Mario's upper arm, verbatim from file 296 at `0x2744`.
    fn mario_arm() -> alloc::vec::Vec<u8> {
        script(&[
            0x2400_8000,
            0xff00_00ff,
            0x2400_8001,
            0xffe7_00ff,
            0x2400_8001,
            0xf7e7_8cff,
            0x2400_8001,
            0x5242_ffff,
            0x2600_8001,
            0x00ce_00ff,
            0x0400_0061, // Wait(97) — parks the clock past every costume
            0x0000_0000, // End
        ])
    }

    #[test]
    fn a_costume_list_gives_a_different_colour_per_frame() {
        // One script, one costume per frame. Mario's default is red; the green
        // that was reaching his sleeves is costume 4, the last entry, which is
        // also the one baked into `MObjSub`.
        let d = mario_arm();
        let at = |c: f32| colors_at(&d, 0, c).unwrap().prim.unwrap();
        assert_eq!(at(0.0), [0xff, 0x00, 0x00, 0xff], "red");
        assert_eq!(at(1.0), [0xff, 0xe7, 0x00, 0xff], "yellow");
        assert_eq!(at(2.0), [0xf7, 0xe7, 0x8c, 0xff]);
        assert_eq!(at(3.0), [0x52, 0x42, 0xff, 0xff], "blue");
        assert_eq!(at(4.0), [0x00, 0xce, 0x00, 0xff], "green");
    }

    #[test]
    fn the_command_word_carries_its_own_flags_and_payload() {
        // The 32-bit encoding has no `toggle`: the duration is in the command,
        // not in a word after it. Reading it the 16-bit way would take the
        // colour as the payload and the next command as the colour.
        let w = 0x2400_8001u32;
        assert_eq!(w >> 25, OP_EXT_VAL_AFTER_BLOCK);
        assert_eq!((w >> 15) & 0x3FF, 1, "PrimColor");
        assert_eq!(w & 0x7FFF, 1, "one frame");
    }

    #[test]
    fn an_opcode_that_is_not_modelled_is_an_error_rather_than_a_skip() {
        // Skipping means guessing a word count, and a colour read from a
        // desynchronised stream still looks like a colour.
        let d = script(&[cmd(17, 0x3F0, 0), 0, cmd(OP_END, 0, 0)]);
        assert!(matches!(
            colors_at(&d, 0, 0.0),
            Err(MatAnimError::UnknownOpcode { opcode: 17, .. })
        ));
    }

    #[test]
    fn a_truncated_script_is_an_error_rather_than_a_black_colour() {
        let d = script(&[cmd(OP_EXT_VAL_AFTER_BLOCK, 1, 0)]);
        assert!(matches!(
            colors_at(&d, 0, 0.0),
            Err(MatAnimError::Truncated { .. })
        ));
    }

    #[test]
    fn tracks_the_script_never_names_stay_unset() {
        // A material that only ever sets a primitive colour must not be given
        // a black environment colour by omission.
        let d = mario_arm();
        let c = colors_at(&d, 0, 0.0).unwrap();
        assert!(c.prim.is_some());
        assert_eq!(c.env, None);
        assert_eq!(c.blend, None);
    }
}
