//! A material-animation file's scripts, packed apart from the rest of the
//! file (pack `VERSION` 149).
//!
//! [`MaterialJoint`](super::MaterialJoint) reads only the words its script
//! reaches: each command and its value words, and through `Jump`/`SetAnim`
//! the word naming the next command. [`script_window`] walks every root the
//! same way `gcParseMObjMatAnimJoint` does ([`MaterialJoint::parse`]), keeps
//! only the reached words, closes each run of them up against the last, and
//! rewrites every jump-target word to the target's new place. A script then
//! plays from the window exactly as from the whole file.
//!
//! [`MaterialJoint::parse`]: super::MaterialJoint

use alloc::vec;
use alloc::vec::Vec;

use super::{
    is_ext, tick_values_per_track, u32_at, MatAnimError, CMD_22_FLAGS, MAT_TRACK_COUNT,
    OP_ADD_LENGTH, OP_CMD_22, OP_END, OP_JUMP, OP_SET_ANIM, OP_SET_FLAGS, OP_SET_INTERP,
    TRACK_COUNT,
};

/// Why a file's scripts cannot be windowed. Each is a pack-build failure:
/// the whole file would play, the window would not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowError {
    /// `root`'s script does not decode (as [`MaterialJoint::tick`] would
    /// fail on it).
    ///
    /// [`MaterialJoint::tick`]: super::MaterialJoint::tick
    Script { root: u32, error: MatAnimError },
    /// A command or jump target off a word boundary.
    Unaligned { at: u32 },
    /// A word one script reads as a jump target and another as a command or
    /// a value: rebasing it would change the other's reading.
    Overlap { at: u32 },
}

/// One file's reached script words, closed up.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScriptWindow {
    /// The words, big-endian as in the file, with every jump target
    /// rebased to its packed offset ([`Self::translate`]).
    pub bytes: Vec<u8>,
    /// `(source offset, length, packed offset)` per run of reached words,
    /// ascending; packed offsets count from the window's `base`.
    pub ranges: Vec<(u32, u32, u32)>,
}

impl ScriptWindow {
    /// The packed offset of the reached word at file offset `source`.
    pub fn translate(&self, source: u32) -> Option<u32> {
        let i = self
            .ranges
            .partition_point(|&(start, _, _)| start <= source);
        let (start, len, packed) = *self.ranges.get(i.checked_sub(1)?)?;
        (source - start < len).then_some(packed + (source - start))
    }
}

/// A word read as part of a command (its opcode or a value).
const PLAIN: u8 = 1;
/// A word read as a `Jump`/`SetAnim` target.
const TARGET: u8 = 2;
/// A word a command starts at: the walk from it is already recorded.
const COMMAND: u8 = 4;

/// Marks `words` words from `at` with `kind`.
fn mark(uses: &mut [u8], at: usize, words: usize, kind: u8) -> Result<(), WindowError> {
    for w in at / 4..at / 4 + words {
        let u = uses.get_mut(w).ok_or(WindowError::Script {
            root: 0,
            error: MatAnimError::Truncated { at: w * 4 },
        })?;
        *u |= kind;
        if *u & PLAIN != 0 && *u & TARGET != 0 {
            return Err(WindowError::Overlap { at: (w * 4) as u32 });
        }
    }
    Ok(())
}

/// Records every word [`super::MaterialJoint`] reads playing `root`
/// forever: the command sequence is fixed (no command branches on state),
/// so it is the static walk to `End` or back to a command already walked.
fn reach(file: &[u8], root: u32, uses: &mut [u8]) -> Result<(), WindowError> {
    let script = |error| WindowError::Script { root, error };
    let mut pc = root as usize;
    loop {
        if !pc.is_multiple_of(4) {
            return Err(WindowError::Unaligned { at: pc as u32 });
        }
        if uses.get(pc / 4).is_some_and(|&u| u & COMMAND != 0) {
            return Ok(());
        }
        let word = u32_at(file, pc).ok_or(script(MatAnimError::Truncated { at: pc }))?;
        let opcode = word >> 25;
        let flags = (word >> 15) & 0x3FF;
        // Words after the opcode word, as `parse`/`apply` step `pc`.
        let values = match opcode {
            OP_END => {
                mark(uses, pc, 1, PLAIN | COMMAND)?;
                return Ok(());
            }
            OP_JUMP | OP_SET_ANIM => {
                let target =
                    u32_at(file, pc + 4).ok_or(script(MatAnimError::Truncated { at: pc }))?;
                if target as usize == pc {
                    return Err(script(MatAnimError::TooLong));
                }
                mark(uses, pc, 1, PLAIN | COMMAND)?;
                mark(uses, pc + 4, 1, TARGET)?;
                pc = target as usize;
                continue;
            }
            OP_SET_FLAGS | OP_ADD_LENGTH => 0,
            OP_CMD_22 => (flags & CMD_22_FLAGS).count_ones() as usize,
            OP_SET_INTERP => 1,
            _ => {
                let per = tick_values_per_track(opcode)
                    .ok_or(script(MatAnimError::UnknownOpcode { opcode, at: pc }))?;
                let count = if is_ext(opcode) {
                    TRACK_COUNT
                } else {
                    MAT_TRACK_COUNT
                };
                // `apply` reads one word per set track, and a second for a
                // two-value command.
                let set = (flags & ((1 << count) - 1)).count_ones() as usize;
                set * if per == 2 { 2 } else { 1 }
            }
        };
        if u32_at(file, pc + values * 4).is_none() && values > 0 {
            return Err(script(MatAnimError::Truncated {
                at: pc + values * 4,
            }));
        }
        mark(uses, pc, 1, PLAIN | COMMAND)?;
        mark(uses, pc + 4, values, PLAIN)?;
        pc += 4 + values * 4;
    }
}

/// `file`'s words every script in `roots` reaches, closed up into one
/// window whose offsets start at `base` (the bytes a caller puts first).
pub fn script_window(
    file: &[u8],
    roots: impl IntoIterator<Item = u32>,
    base: u32,
) -> Result<ScriptWindow, WindowError> {
    let mut uses = vec![0u8; file.len() / 4];
    for root in roots {
        reach(file, root, &mut uses).map_err(|e| match e {
            WindowError::Script { error, .. } => WindowError::Script { root, error },
            e => e,
        })?;
    }
    let mut ranges: Vec<(u32, u32, u32)> = Vec::new();
    let mut bytes = Vec::new();
    for (w, &u) in uses.iter().enumerate() {
        if u == 0 {
            continue;
        }
        let at = (w * 4) as u32;
        match ranges.last_mut() {
            Some((start, len, _)) if *start + *len == at => *len += 4,
            _ => ranges.push((at, 4, base + bytes.len() as u32)),
        }
        bytes.extend_from_slice(&file[w * 4..w * 4 + 4]);
    }
    let mut window = ScriptWindow { bytes, ranges };
    for (w, &u) in uses.iter().enumerate() {
        if u & TARGET == 0 {
            continue;
        }
        let at = (w * 4) as u32;
        let target = u32_at(file, w * 4).expect("a reached word");
        // Every target was walked, so it starts a reached command.
        let packed = window
            .translate(target)
            .filter(|_| {
                uses.get(target as usize / 4)
                    .is_some_and(|&t| t & COMMAND != 0)
            })
            .ok_or(WindowError::Overlap { at })?;
        let slot = (window.translate(at).expect("a reached word") - base) as usize;
        window.bytes[slot..slot + 4].copy_from_slice(&packed.to_be_bytes());
    }
    Ok(window)
}

#[cfg(test)]
mod tests {
    use super::super::{MaterialJoint, OP_SET_VAL_AFTER_BLOCK, TICK_TRACK_COUNT};
    use super::*;

    const fn cmd(opcode: u32, flags: u32, payload: u32) -> u32 {
        (opcode << 25) | (flags << 15) | payload
    }

    fn bytes(words: &[u32]) -> Vec<u8> {
        words.iter().flat_map(|w| w.to_be_bytes()).collect()
    }

    /// Two scripts with filler between and around them; the second loops
    /// through a jump into the first's tail.
    fn file() -> Vec<u8> {
        bytes(&[
            0xDEAD_0000,
            0xDEAD_0001,
            // 0x08: script A: two keys, then End.
            cmd(OP_SET_VAL_AFTER_BLOCK, 1 << 9, 3),
            0x3F80_0000,
            cmd(OP_SET_VAL_AFTER_BLOCK, 1 << 9, 2),
            0x4000_0000,
            cmd(OP_END, 0, 0),
            0xDEAD_0002,
            0xDEAD_0003,
            // 0x24: script B: one key, then a jump to A's second key.
            cmd(OP_SET_VAL_AFTER_BLOCK, 1 << 9, 5),
            0x4040_0000,
            cmd(OP_JUMP, 0, 0),
            0x10,
            0xDEAD_0004,
        ])
    }

    #[test]
    fn a_window_keeps_only_reached_words_and_rebases_jumps() {
        let f = file();
        let w = script_window(&f, [0x24, 0x08], 8).unwrap();
        assert_eq!(w.ranges, vec![(0x08, 20, 8), (0x24, 16, 28)]);
        assert_eq!(w.bytes.len(), 36);
        assert_eq!(w.translate(0x08), Some(8));
        assert_eq!(w.translate(0x24), Some(28));
        assert_eq!(w.translate(0x20), None);
        // B's jump now names A's second key at its packed place.
        assert_eq!(&w.bytes[32..36], &16u32.to_be_bytes());
    }

    #[test]
    fn a_windowed_script_plays_as_the_whole_file() {
        let f = file();
        let base = 8;
        let w = script_window(&f, [0x08, 0x24], base).unwrap();
        let mut packed = vec![0xEE; base as usize];
        packed.extend_from_slice(&w.bytes);
        for root in [0x08, 0x24] {
            let mut whole = MaterialJoint::start(root, 0.0);
            let mut win = MaterialJoint::start(w.translate(root).unwrap(), 0.0);
            for _ in 0..40 {
                whole.tick(&f, 1.0).unwrap();
                win.tick(&packed, 1.0).unwrap();
                for t in 0..TICK_TRACK_COUNT {
                    assert_eq!(whole.track_value(t), win.track_value(t));
                }
                assert_eq!(whole.ended(), win.ended());
            }
        }
    }

    #[test]
    fn a_word_read_two_ways_is_refused() {
        // Root 0x0C starts inside the jump at 0x08: its opcode word is the
        // jump's target word.
        let f = bytes(&[
            cmd(OP_SET_FLAGS, 0, 1),
            cmd(OP_END, 0, 0),
            cmd(OP_JUMP, 0, 0),
            0x04,
        ]);
        assert_eq!(
            script_window(&f, [0x08, 0x0C], 0),
            Err(WindowError::Overlap { at: 0x0C })
        );
    }

    #[test]
    fn a_truncated_root_is_refused() {
        let f = bytes(&[cmd(OP_SET_VAL_AFTER_BLOCK, 1 << 9, 3)]);
        assert!(matches!(
            script_window(&f, [0], 0),
            Err(WindowError::Script { root: 0, .. })
        ));
    }
}
