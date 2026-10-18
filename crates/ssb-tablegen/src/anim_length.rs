//! How long a fighter animation (a figatree archive file) runs.
//!
//! Mirrors `ftAnimParseDObjFigatree` as far as the length depends on it: the
//! `u16` words each command consumes, and which commands advance the clock.
//! `ssb_rom::figatree` plays the same scripts; this is a second, deliberately
//! minimal reading of them (`romtool anims --verify` compares the two).

use std::collections::HashMap;
use std::sync::Mutex;

use crate::source::{be_u16, be_u32};
use crate::Source;

const OP_END: u16 = 0;
const OP_TRANSLATE_INTERP: u16 = 12;
const OP_LOOP: u16 = 13;

/// Opcodes whose payload is added to the clock.
fn is_block(op: u16) -> bool {
    matches!(op, 1 | 2 | 4 | 7 | 9 | 14)
}

/// Value words per set track flag.
fn values_per_track(op: u16) -> u32 {
    match op {
        2 | 3 | 6..=10 => 1,
        4 | 5 => 2,
        _ => 0,
    }
}

/// The figatree's joint scripts, by byte offset: the pointer table that opens
/// the file runs to the first script it names, or to the first word that
/// cannot be a script pointer.
fn joint_scripts(data: &[u8]) -> Option<Vec<usize>> {
    let word = |i: usize| be_u32(data, i * 4).ok();
    let first = (0..data.len() / 4).filter_map(word).find(|&p| p != 0)?;
    if !first.is_multiple_of(4) || first as usize > data.len() {
        return None;
    }
    let mut end = first as usize / 4;
    let mut out = Vec::new();
    let mut len = 0;
    while len < end {
        let ptr = word(len)?;
        if ptr != 0 {
            if !ptr.is_multiple_of(2) || ptr as usize >= data.len() {
                break;
            }
            end = end.min(ptr as usize / 4);
            out.push(ptr as usize);
        }
        len += 1;
    }
    (len != 0 && len <= 64).then_some(out)
}

/// One joint's script: `(frames until End or the first Loop, loops)`.
/// `None` when the walk runs off the file or a `Loop` does not jump back to
/// the script's first frame.
fn script_clip(data: &[u8], start: usize) -> Option<(u32, bool)> {
    let word = |i: usize| be_u16(data, start + i * 2).ok();
    let mut total = 0u32;
    let mut i = 0usize;
    let mut frame_at = HashMap::new();
    loop {
        frame_at.insert(i, total);
        let w = word(i)?;
        let (op, flags, toggle) = (w >> 11, (w >> 1) & 0x3FF, w & 1);
        i += 1;
        match op {
            OP_END => return Some((total, false)),
            OP_LOOP => {
                let off = i64::from(word(i)? as i16);
                let target = i as i64 + off.div_euclid(2);
                if usize::try_from(target).ok().and_then(|t| frame_at.get(&t)) != Some(&0) {
                    return None;
                }
                return Some((total, true));
            }
            OP_TRANSLATE_INTERP => {
                i += 1;
                continue;
            }
            _ => {}
        }
        let mut payload = 0;
        if toggle != 0 {
            payload = word(i)?;
            i += 1;
        }
        if is_block(op) {
            total += u32::from(payload);
        }
        i += (values_per_track(op) * flags.count_ones()) as usize;
    }
}

/// One joint's script: frames until `End`, `Some(None)` when it loops.
fn script_frames(data: &[u8], start: usize) -> Option<Option<u32>> {
    let word = |i: usize| be_u16(data, start + i * 2).ok();
    let mut total = 0u32;
    let mut i = 0usize;
    loop {
        let w = word(i)?;
        let (op, flags, toggle) = (w >> 11, (w >> 1) & 0x3FF, w & 1);
        i += 1;
        match op {
            OP_END => return Some(Some(total)),
            OP_LOOP => return Some(None),
            OP_TRANSLATE_INTERP => {
                i += 1;
                continue;
            }
            _ => {}
        }
        let mut payload = 0;
        if toggle != 0 {
            payload = word(i)?;
            i += 1;
        }
        if is_block(op) {
            total += u32::from(payload);
        }
        i += (values_per_track(op) * flags.count_ones()) as usize;
    }
}

/// Every joint's value, when they all agree.
fn agreed<T: PartialEq + Copy>(data: &[u8], walk: impl Fn(&[u8], usize) -> Option<T>) -> Option<T> {
    let joints = joint_scripts(data)?;
    let mut out: Option<T> = None;
    for start in joints {
        let v = walk(data, start)?;
        match out {
            None => out = Some(v),
            Some(prev) if prev != v => return None,
            Some(_) => {}
        }
    }
    out
}

/// Lengths per animation file, computed once.
#[derive(Default)]
pub struct Lengths {
    frames: Mutex<HashMap<u32, Option<Option<u32>>>>,
    clips: Mutex<HashMap<u32, Option<(u32, bool)>>>,
}

impl Lengths {
    /// The animation's length in frames: `Some(Some(n))`; `Some(None)` when
    /// it loops; `None` when the file is not a figatree whose joints agree.
    pub fn frames(&self, rom: &Source, file: u32) -> Option<Option<u32>> {
        let mut cache = self.frames.lock().expect("lengths");
        *cache.entry(file).or_insert_with(|| {
            let data = rom.file(file).ok()?.data;
            agreed(&data, script_frames)
        })
    }

    /// The frames to the animation's `End` or first `Loop`, and whether it
    /// loops; `None` as for [`Self::frames`].
    pub fn clip(&self, rom: &Source, file: u32) -> Option<(u32, bool)> {
        let mut cache = self.clips.lock().expect("lengths");
        *cache.entry(file).or_insert_with(|| {
            let data = rom.file(file).ok()?.data;
            agreed(&data, script_clip)
        })
    }
}
