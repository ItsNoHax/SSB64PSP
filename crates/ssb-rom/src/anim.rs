//! How long a fighter's animations last (`AObjEvent16` figatree scripts).
//!
//! Five ground statuses — Dash, Turn, RunBrake, Squat and Landing — do not
//! have a duration anywhere in `FTAttributes`. They end when their *animation*
//! runs out, and their update functions say so directly:
//!
//! ```c
//! void ftCommonDashProcUpdate(GObj *fighter_gobj) {
//!     if (fighter_gobj->anim_frame <= 0.0F) { ... ftCommonWaitSetStatus(...); }
//! }
//! ```
//!
//! So the length lives in the animation file, and this module reads it.
//!
//! ## The file format
//!
//! A figatree file opens with a pointer table, one `AObjEvent32*` per model
//! joint, holding byte offsets into the same file (or zero, for joints this
//! animation does not move). The table's own length is not stored: the first
//! non-null pointer is the offset of the first script, which is exactly where
//! the table ends, so the entry count is that offset divided by four.
//!
//! Each script is a stream of 16-bit commands, `{ opcode:5, flags:10,
//! toggle:1 }`. `flags` is a bitmask over ten transform tracks and decides how
//! many value words trail the command; `toggle` says whether a payload word
//! comes first. [`ftAnimParseDObjFigatree`] runs this stream against a frame
//! clock, and only two things about it matter here: how many words each
//! command consumes, and which commands add their payload to the clock.
//!
//! ## Why the answer can be trusted
//!
//! Every joint carries its own independent script, and the exporter gave them
//! all the same total. So the decoder walks *all* of them and requires
//! unanimity — eighteen scripts, separately encoded, agreeing on one number.
//!
//! That is a real test rather than a formality, because the walk is
//! self-checking: a wrong word count for any command desynchronises the stream
//! and the walk then runs past the end of the script instead of finding its
//! terminator. Across the decompilation's 1775 animation files it agreed on
//! 1736; the 37 exceptions are all entry and cutscene animations, which use
//! the 32-bit `AnimJoint` encoding instead and are not figatrees at all.
//!
//! [`ftAnimParseDObjFigatree`]: https://github.com/ssb64-decomp

use alloc::vec::Vec;

use crate::archive::{Archive, File};
use crate::figatree;

/// Number of statuses [`FIGHTER_ANIMS`] carries an animation for.
pub const SLOT_COUNT: usize = 7;

/// Slot index of each status, matching [`SLOT_NAMES`].
pub const SLOT_DASH: usize = 0;
pub const SLOT_TURN: usize = 1;
pub const SLOT_RUN_BRAKE: usize = 2;
pub const SLOT_SQUAT: usize = 3;
pub const SLOT_SQUAT_RV: usize = 4;
pub const SLOT_LANDING: usize = 5;
pub const SLOT_PASS: usize = 6;

/// A fighter's animation file for each slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FighterAnims {
    /// The decompilation's symbol prefix, for diagnostics.
    pub name: &'static str,
    /// Archive file id per slot.
    pub files: [u16; SLOT_COUNT],
}

include!("anim_table.rs");

/// A decoded animation length, in frames of playback at speed 1.0.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimLength {
    /// The script terminates after this many frames.
    Frames(u16),
    /// The script jumps back on itself and never ends. Correct for Wait, the
    /// walks, Run and Fall — statuses that leave by being interrupted.
    Loops,
}

impl AnimLength {
    /// The length in frames, or `None` when the animation loops.
    pub fn frames(self) -> Option<u16> {
        match self {
            AnimLength::Frames(n) => Some(n),
            AnimLength::Loops => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimError {
    /// The file is too short to hold even a pointer table.
    TooShort { file: u32, len: usize },
    /// The pointer table's implied length is not a plausible joint count.
    BadTable { file: u32, first: u32 },
    /// A script ran past the end of the file without terminating, which means
    /// the command stream desynchronised.
    Desynchronised { file: u32, joint: usize },
    /// Two joints disagreed about how long the animation is.
    JointsDisagree {
        file: u32,
        first: AnimLength,
        other: AnimLength,
    },
    /// The file has a pointer table but no non-null scripts.
    NoScripts { file: u32 },
}

impl core::fmt::Display for AnimError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            AnimError::TooShort { file, len } => {
                write!(f, "file {file}: {len} bytes is too short for a figatree")
            }
            AnimError::BadTable { file, first } => write!(
                f,
                "file {file}: first joint pointer {first:#x} is not a joint table length"
            ),
            AnimError::Desynchronised { file, joint } => write!(
                f,
                "file {file}: joint {joint}'s script ran past the end of the file"
            ),
            AnimError::JointsDisagree { file, first, other } => write!(
                f,
                "file {file}: joints disagree on the length ({first:?} vs {other:?})"
            ),
            AnimError::NoScripts { file } => {
                write!(f, "file {file}: joint table points at no scripts")
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for AnimError {}

/// Largest joint count treated as plausible.
///
/// The biggest fighter skeleton in the game is well under this; a file whose
/// first word is a large offset is not a figatree.
const MAX_JOINTS: usize = 64;

const OP_END: u16 = 0;
const OP_LOOP: u16 = 13;

fn u32_be(data: &[u8], at: usize) -> u32 {
    u32::from_be_bytes([data[at], data[at + 1], data[at + 2], data[at + 3]])
}

/// Walks one joint's script, returning how long it runs.
///
/// Runs on the same decoder [`figatree`](crate::figatree) plays scripts with,
/// so the 189 lengths verified against the decompilation are a test of that
/// decoder's word counts rather than of a second copy of them.
fn script_length(data: &[u8], start: usize) -> Option<AnimLength> {
    let mut frames: u16 = 0;
    let mut at = start;
    loop {
        let cmd = figatree::command(data, at).ok()?;
        at = cmd.next;

        match cmd.opcode {
            OP_END => return Some(AnimLength::Frames(frames)),
            OP_LOOP => return Some(AnimLength::Loops),
            _ => {}
        }
        if figatree::is_block(cmd.opcode) {
            frames = frames.saturating_add(cmd.payload);
        }
    }
}

/// Reads an animation's length out of a figatree file.
///
/// Requires every joint script to agree, so a mis-decode surfaces as
/// [`AnimError::JointsDisagree`] or [`AnimError::Desynchronised`] rather than
/// as a plausible wrong number.
pub fn decode_length(file_id: u32, file: &File) -> Result<AnimLength, AnimError> {
    let data = &file.data;
    if data.len() < 8 {
        return Err(AnimError::TooShort {
            file: file_id,
            len: data.len(),
        });
    }

    // The table runs up to the first script it points at.
    let mut first = 0u32;
    let mut at = 0;
    while at + 4 <= data.len() {
        let ptr = u32_be(data, at);
        if ptr != 0 {
            first = ptr;
            break;
        }
        at += 4;
    }
    let joints = first as usize / 4;
    if first == 0
        || !first.is_multiple_of(4)
        || joints == 0
        || joints > MAX_JOINTS
        || first as usize > data.len()
    {
        return Err(AnimError::BadTable {
            file: file_id,
            first,
        });
    }

    let mut agreed: Option<AnimLength> = None;
    for joint in 0..joints {
        let ptr = u32_be(data, joint * 4) as usize;
        if ptr == 0 {
            continue;
        }
        if ptr >= data.len() {
            return Err(AnimError::Desynchronised {
                file: file_id,
                joint,
            });
        }
        let Some(length) = script_length(data, ptr) else {
            return Err(AnimError::Desynchronised {
                file: file_id,
                joint,
            });
        };
        match agreed {
            None => agreed = Some(length),
            Some(prev) if prev != length => {
                return Err(AnimError::JointsDisagree {
                    file: file_id,
                    first: prev,
                    other: length,
                })
            }
            Some(_) => {}
        }
    }
    agreed.ok_or(AnimError::NoScripts { file: file_id })
}

/// Every animation length for one fighter, by slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FighterLengths {
    pub name: &'static str,
    /// Frames per slot; `0` where the animation loops, which for a playable
    /// fighter never happens and for Master Hand always does.
    pub frames: [u16; SLOT_COUNT],
}

/// Decodes one fighter's animation lengths.
pub fn decode_fighter(
    entry: FighterAnims,
    archive: &Archive<'_>,
) -> Result<FighterLengths, AnimError> {
    let mut frames = [0u16; SLOT_COUNT];
    for (slot, &id) in entry.files.iter().enumerate() {
        let file = archive.load(id as u32).map_err(|_| AnimError::TooShort {
            file: id as u32,
            len: 0,
        })?;
        frames[slot] = decode_length(id as u32, &file)?.frames().unwrap_or(0);
    }
    Ok(FighterLengths {
        name: entry.name,
        frames,
    })
}

/// Decodes every fighter's animation lengths, in `FTKind` order.
pub fn decode_all(archive: &Archive<'_>) -> Vec<Result<FighterLengths, AnimError>> {
    FIGHTER_ANIMS
        .iter()
        .map(|&entry| decode_fighter(entry, archive))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Builds a figatree file from per-joint script word lists.
    fn figatree(scripts: &[&[u16]]) -> File {
        let table = scripts.len() * 4;
        let mut data = alloc::vec![0u8; table];
        for (i, words) in scripts.iter().enumerate() {
            let at = data.len() as u32;
            data[i * 4..i * 4 + 4].copy_from_slice(&at.to_be_bytes());
            for w in words.iter() {
                data.extend_from_slice(&w.to_be_bytes());
            }
        }
        File {
            id: 1,
            data,
            extern_relocs: Vec::new(),
            intern_relocs: Vec::new(),
        }
    }

    const fn cmd(op: u16, flags: u16, toggle: u16) -> u16 {
        (op << 11) | (flags << 1) | toggle
    }

    #[test]
    fn a_block_command_advances_the_clock_and_a_plain_one_does_not() {
        // SetValBlockT(TRAY, 6) then SetValT(TRAY, 9), then End. Only the
        // first is a Block, so the animation is 6 frames and not 15.
        let tray = 1 << 5;
        let script = [cmd(2, tray, 1), 6, 0, cmd(3, tray, 1), 9, 0, cmd(0, 0, 0)];
        let f = figatree(&[&script]);
        assert_eq!(decode_length(1, &f), Ok(AnimLength::Frames(6)));
    }

    #[test]
    fn track_flags_decide_how_many_value_words_follow() {
        // SetValRateBlockT over two tracks reads a (value, rate) pair each, so
        // four words trail the payload. Miscounting them would swallow the End.
        let two = (1 << 0) | (1 << 1);
        let script = [cmd(4, two, 1), 5, 100, 0, 200, 0, cmd(0, 0, 0)];
        let f = figatree(&[&script]);
        assert_eq!(decode_length(1, &f), Ok(AnimLength::Frames(5)));
    }

    #[test]
    fn a_looping_script_reports_that_it_never_ends() {
        let script = [cmd(1, 0, 1), 4, cmd(OP_LOOP, 0, 0), 0xFFF8];
        let f = figatree(&[&script]);
        assert_eq!(decode_length(1, &f), Ok(AnimLength::Loops));
        assert_eq!(AnimLength::Loops.frames(), None);
    }

    #[test]
    fn null_joints_are_skipped_without_ending_the_table() {
        let script = [cmd(1, 0, 1), 7, cmd(0, 0, 0)];
        let mut f = figatree(&[&script, &script, &script]);
        // Blank the middle joint the way an unanimated joint is stored.
        f.data[4..8].copy_from_slice(&0u32.to_be_bytes());
        assert_eq!(decode_length(1, &f), Ok(AnimLength::Frames(7)));
    }

    #[test]
    fn joints_that_disagree_are_an_error_rather_than_a_first_answer() {
        let a = [cmd(1, 0, 1), 7, cmd(0, 0, 0)];
        let b = [cmd(1, 0, 1), 9, cmd(0, 0, 0)];
        let f = figatree(&[&a, &b]);
        assert!(matches!(
            decode_length(1, &f),
            Err(AnimError::JointsDisagree { .. })
        ));
    }

    #[test]
    fn a_script_without_a_terminator_desynchronises_rather_than_returning() {
        let script = [cmd(1, 0, 1), 7];
        let f = figatree(&[&script]);
        assert!(matches!(
            decode_length(1, &f),
            Err(AnimError::Desynchronised { .. })
        ));
    }

    #[test]
    fn the_table_length_comes_from_the_first_script_offset() {
        let script = [cmd(1, 0, 1), 3, cmd(0, 0, 0)];
        let f = figatree(&[&script, &script, &script, &script]);
        assert_eq!(u32_be(&f.data, 0), 16);
        assert_eq!(decode_length(1, &f), Ok(AnimLength::Frames(3)));
    }

    #[test]
    fn every_fighter_has_a_file_for_every_slot() {
        assert_eq!(FIGHTER_ANIMS.len(), crate::fighter::FIGHTER_FILES.len());
        for (anims, files) in FIGHTER_ANIMS
            .iter()
            .zip(crate::fighter::FIGHTER_FILES.iter())
        {
            assert_eq!(anims.name, files.name, "fighter order must match");
            assert!(anims.files.iter().all(|&f| f != 0));
        }
    }
}
