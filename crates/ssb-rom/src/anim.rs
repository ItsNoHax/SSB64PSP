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

/// First of Donkey Kong's eleven cargo-carry slots, `ThrowFWait` through
/// `ThrowAirFF` in `ftDonkeyStatus` order.
pub const SLOT_DONKEY_THROWF_WAIT: usize = 99;
/// First of the thirteen shared grab slots: `Catch`, `CatchPull`, `ThrowF`,
/// `ThrowB`, `CapturePulled`, then the thrown statuses 181..=188 in
/// `ftCommonStatus` order. Packed for Mario, Fox, Donkey Kong, Samus, Luigi
/// Link, Yoshi, Captain Falcon and Kirby.
pub const SLOT_CATCH: usize = 110;
/// First of Samus's 22 common attack slots, `Attack11` through
/// `AttackAirLw` in `ftCommonStatus` order.
pub const SLOT_SAMUS_ATTACK11: usize = 123;
/// First of Samus's nine special slots, `SpecialNStart` through
/// `SpecialAirLw` in `ftSamusStatus` order.
pub const SLOT_SAMUS_SPECIAL_N_START: usize = 145;
/// First of Luigi's 21 attack slots: `Attack11`, `Attack12`, `Attack13`,
/// then `AttackDash` through `AttackAirLw` without the two mid-angle
/// forward tilts he lacks. His specials use the Mario special slots.
pub const SLOT_LUIGI_ATTACK11: usize = 154;
/// First of Link's 14 common attack slots: `Attack11`, `Attack12`,
/// `AttackDash`, then one forward tilt, the up and down tilts, one forward
/// smash, the other smashes and the five aerials.
pub const SLOT_LINK_ATTACK11: usize = 175;
/// First of Link's 15 own slots, `Attack13` through `SpecialAirLw` in
/// `ftLinkStatus` order without the two Appear statuses.
pub const SLOT_LINK_ATTACK13: usize = 189;
/// First of Yoshi's 18 common attack slots, `Attack11` through `AttackAirLw`.
pub const SLOT_YOSHI_ATTACK11: usize = 204;
/// First of Yoshi's 11 special slots, `SpecialHi` through
/// `SpecialAirNRelease` in `ftYoshiStatus` order without Appear.
pub const SLOT_YOSHI_SPECIAL_HI: usize = 222;
/// First of Captain Falcon's 20 common attack slots.
pub const SLOT_CAPTAIN_ATTACK11: usize = 233;
/// First of his 15 extended attack and special slots, without Appear.
pub const SLOT_CAPTAIN_ATTACK13: usize = 253;
/// First of Kirby's 16 common attack slots (three forward tilts, one
/// forward smash), followed by his `LandingAirF` and `LandingAirB`.
pub const SLOT_KIRBY_ATTACK11: usize = 268;
/// First of Kirby's 31 own slots, `Attack100Start` through `SpecialNCopy`
/// in `ftKirbyStatus` order without Appear, the copy abilities,
/// `SpecialNCatch`, `SpecialAirLwFall` and the aerial inhale statuses, which
/// reuse the grounded figatrees.
pub const SLOT_KIRBY_ATTACK100_START: usize = 286;
/// First of Kirby's 25 copy-ability slots for the ported fighters, one per
/// figatree in `dFTKirbyMotionDescs` order, from `CopyMarioSpecialN` to
/// `CopyYoshiSpecialAirNRelease`.
pub const SLOT_KIRBY_COPY_MARIO_SPECIAL_N: usize = 317;

pub const SLOT_PIKACHU_ATTACK11: usize = 342;
pub const SLOT_PIKACHU_SPECIAL_N: usize = 359;
pub const SLOT_KIRBY_COPY_PIKACHU_SPECIAL_N: usize = 373;
/// First of Jigglypuff's 18 common attack slots (three forward tilts, one
/// forward smash), followed by her `LandingAirF` (Kirby's figatree) and
/// `LandingAirB`.
pub const SLOT_PURIN_ATTACK11: usize = 375;
/// First of Jigglypuff's nine own slots: `JumpAerialF1` through
/// `JumpAerialF5`, then Pound, Pound in the air, Sing and Rest. Sing and Rest
/// have one figatree each for the grounded and aerial statuses.
pub const SLOT_PURIN_JUMP_AERIAL_F1: usize = 393;
pub const SLOT_KIRBY_COPY_PURIN_SPECIAL_N: usize = 402;
/// `FuraSleep`, the shared status Sing puts its targets in.
pub const SLOT_FURA_SLEEP: usize = 404;

pub const SLOT_NESS_ATTACK11: usize = 405;
pub const SLOT_NESS_ATTACK13: usize = 424;
pub const SLOT_KIRBY_COPY_NESS_SPECIAL_N: usize = 444;
/// First of the 25 shared reaction slots: `WallDamage`, `StopCeil`,
/// `DownBounceD` through `Rebound` without the two `DownWait`s and
/// `ReboundWait` (which keep the previous motion), then `EscapeF` through
/// `FuraFura`, in `ftCommonStatus` order.
pub const SLOT_WALL_DAMAGE: usize = 446;
/// First of the 16 shared cliff slots, `CliffCatch` through
/// `CliffEscapeSlow2`.
pub const SLOT_CLIFF_CATCH: usize = 471;
/// First of the 22 shared damage slots: `DamageHi1` through `DamageFlyRoll`,
/// then `DamageFall`, `FallSpecial` and `LandingFallSpecial`.
pub const SLOT_DAMAGE_HI1: usize = 487;
/// First of the 31 shared attack slots, `Appeal` through `LandingAirNull` in
/// `ftCommonStatus` order. A fighter without a motion (a mid-angle tilt it
/// does not have, a null `LandingAirX`) has no file in that slot.
pub const SLOT_APPEAL: usize = 509;
/// Mario's jab finisher (`MarioStatus::Attack13`).
pub const SLOT_MARIO_ATTACK13: usize = 540;
/// First of the 66 remaining shared slots: `RebirthDown` through
/// `HeavyGet`, the item throws, swings and shots, the two hammer motions,
/// `GuardOn`, `GuardOff`, then the five thrown statuses the grab slots do not
/// cover, in `ftCommonStatus` order.
pub const SLOT_REBIRTH_DOWN: usize = 541;
/// First of the 42 item slots, `LightThrowDrop` through `FireFlowerShootAir`.
pub const SLOT_LIGHT_THROW_DROP: usize = 556;
/// `GuardOn`, which `Guard` and `GuardSetOff` keep playing.
pub const SLOT_GUARD_ON: usize = 600;
/// First of the seven battle-entry slots (RE-401): `AppearR`, `AppearL`,
/// then Captain Falcon's and Ness's phases `AppearRStart`, `AppearLStart`,
/// `AppearREnd`, `AppearLEnd` and Ness's `AppearWait`. These are 32-bit
/// `AnimJoint` clips (`FTANIM_FLAG_ANIMJOINT`), not figatrees.
pub const SLOT_APPEAR_R: usize = 607;
pub const SLOT_APPEAR_L: usize = 608;
pub const SLOT_APPEAR_R_START: usize = 609;
pub const SLOT_APPEAR_L_START: usize = 610;
pub const SLOT_APPEAR_R_END: usize = 611;
pub const SLOT_APPEAR_L_END: usize = 612;
pub const SLOT_APPEAR_WAIT: usize = 613;
/// First of the five demo slots (RE-408): `Win1` to `Win4` and `Lose`, the
/// clips `nFTDemoStatusWin1` to `nFTDemoStatusLose` play on the VS results
/// screen and the character selects. They come from the fighter's
/// `dFT<Name>SubMotionDescs` rows 1 to 5 and are figatrees.
pub const SLOT_WIN1: usize = 614;
pub const SLOT_WIN2: usize = 615;
pub const SLOT_WIN3: usize = 616;
pub const SLOT_WIN4: usize = 617;
pub const SLOT_LOSE: usize = 618;
/// Continue and campaign intro demo rows 9, 10, 13 and 14 (RE-451).
pub const SLOT_FIGURE_DROPPED: usize = 619;
pub const SLOT_FIGURE_STAND: usize = 620;
pub const SLOT_INTRO_L: usize = 621;
pub const SLOT_INTRO_R: usize = 622;
/// First of Master Hand's 30 slots, `ftBossMotion` order from
/// `nFTBossMotionDefault` to `nFTBossMotionAppear`.
pub const SLOT_BOSS_DEFAULT: usize = 623;
/// Number of Master Hand's own motion slots.
pub const BOSS_SLOTS: usize = 30;
/// The opening movie's demo clips (`mvOpening*`): `nFTDemoStatusRun`,
/// `...Jump`, `...FigurePulled`, `...Clash` and `...Stance` (submotion
/// rows 6, 7, 8, 11 and 12), then the nine fighter-specific statuses from
/// `nFTDemoStatusSpecialStart` (0x1000F, rows 15 to 23). Figatrees.
pub const SLOT_DEMO_RUN: usize = 653;
pub const SLOT_DEMO_JUMP: usize = 654;
pub const SLOT_FIGURE_PULLED: usize = 655;
pub const SLOT_CLASH: usize = 656;
pub const SLOT_STANCE: usize = 657;
pub const SLOT_OPENING1: usize = 658;
/// Number of fighter-specific opening slots from [`SLOT_OPENING1`].
pub const OPENING_SLOTS: usize = 9;
/// Number of statuses [`FIGHTER_ANIMS`] carries an animation for.
pub const SLOT_COUNT: usize = 667;

/// Whether a slot holds a 32-bit `AnimJoint` clip rather than a figatree:
/// the seven entry slots, and Master Hand's motions whose descriptor sets
/// `FTANIM_FLAG_ANIMJOINT`.
pub const fn is_anim_joint_slot(slot: usize) -> bool {
    (slot >= SLOT_APPEAR_R && slot < SLOT_WIN1)
        || (slot >= SLOT_BOSS_DEFAULT
            && slot < SLOT_BOSS_DEFAULT + BOSS_SLOTS
            && BOSS_ANIM_JOINT[slot - SLOT_BOSS_DEFAULT])
}

/// Slot index of each status, matching [`SLOT_NAMES`].
///
/// The first seven are the original movement statuses whose animation length
/// controls their exit (RE-035). Later grab, throw and cargo slots also carry
/// finite lengths for their status callbacks; other slots provide poses and
/// loop or leave when interrupted.
pub const SLOT_DASH: usize = 0;
pub const SLOT_TURN: usize = 1;
pub const SLOT_RUN_BRAKE: usize = 2;
pub const SLOT_SQUAT: usize = 3;
pub const SLOT_SQUAT_RV: usize = 4;
pub const SLOT_LANDING: usize = 5;
pub const SLOT_PASS: usize = 6;
pub const SLOT_WAIT: usize = 7;
pub const SLOT_WALK_SLOW: usize = 8;
pub const SLOT_WALK_MIDDLE: usize = 9;
pub const SLOT_WALK_FAST: usize = 10;
pub const SLOT_RUN: usize = 11;
pub const SLOT_KNEE_BEND: usize = 12;
pub const SLOT_JUMP_F: usize = 13;
pub const SLOT_JUMP_B: usize = 14;
pub const SLOT_JUMP_AERIAL_F: usize = 15;
pub const SLOT_JUMP_AERIAL_B: usize = 16;
pub const SLOT_FALL: usize = 17;
pub const SLOT_FALL_AERIAL: usize = 18;
pub const SLOT_SQUAT_WAIT: usize = 19;
/// Mario's grounded `SpecialN` / Fireball pose.
pub const SLOT_MARIO_SPECIAL_N: usize = 20;
/// Mario's aerial `SpecialN` / Fireball pose.
pub const SLOT_MARIO_SPECIAL_AIR_N: usize = 21;
/// Mario's `SpecialHi` / Super Jump Punch pose.
pub const SLOT_MARIO_SPECIAL_HI: usize = 22;
/// Mario's aerial `SpecialHi` / Super Jump Punch pose.
pub const SLOT_MARIO_SPECIAL_AIR_HI: usize = 23;
/// Mario's grounded `SpecialLw` / Tornado pose.
pub const SLOT_MARIO_SPECIAL_LW: usize = 24;
/// Mario's aerial `SpecialLw` / Tornado pose.
pub const SLOT_MARIO_SPECIAL_AIR_LW: usize = 25;

/// Slots whose animation ends on its own, so a length means something.
pub const TIMED_SLOTS: usize = 7;

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

fn first_pointer(data: &[u8]) -> u32 {
    (0..data.len() / 4)
        .map(|i| u32_be(data, i * 4))
        .find(|&p| p != 0)
        .unwrap_or(0)
}

/// How many joint pointers open a figatree file.
///
/// The table normally runs up to the first script it points at. A few files
/// (Samus's `RollF`/`RollB`) keep unreferenced bytes between the table and
/// that script, so the table also ends at the first word that cannot be a
/// script pointer: odd (scripts are `u16` streams), or past the end of the
/// file. The game's
/// own parser never needs the length -- it reads one pointer per skeleton
/// joint.
pub fn joint_table_len(data: &[u8]) -> Option<usize> {
    let first = first_pointer(data);
    if first == 0 || !first.is_multiple_of(4) || first as usize > data.len() {
        return None;
    }
    let mut end = first as usize / 4;
    let mut len = 0;
    while len < end {
        let ptr = u32_be(data, len * 4);
        if ptr != 0 {
            if !ptr.is_multiple_of(2) || ptr as usize >= data.len() {
                break;
            }
            end = end.min(ptr as usize / 4);
        }
        len += 1;
    }
    (len != 0 && len <= MAX_JOINTS).then_some(len)
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

    let Some(joints) = joint_table_len(data) else {
        return Err(AnimError::BadTable {
            file: file_id,
            first: first_pointer(data),
        });
    };

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
        // Zero means the fighter has no animation for that status -- Kirby and
        // Jigglypuff have no aerial jump, and the motion table says so with a
        // null placeholder (RE-035).
        if id == 0 {
            continue;
        }
        let file = archive.load(id as u32).map_err(|_| AnimError::TooShort {
            file: id as u32,
            len: 0,
        })?;
        // The entry slots hold 32-bit `AnimJoint` clips, not figatrees
        // (RE-401): their length is the tick the first joint's stream ends
        // on, less one. A malformed one is left at 0 rather than failing
        // the fighter.
        if is_anim_joint_slot(slot) {
            let first = joint_table_len(&file.data).and_then(|len| {
                crate::objanim::joint_scripts(&file.data, 0, len)
                    .into_iter()
                    .flatten()
                    .next()
            });
            frames[slot] = first
                .and_then(|at| crate::objanim::script_end_tick(&file.data, at, 4096))
                .map_or(0, |t| t.saturating_sub(1) as u16);
            continue;
        }
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
    fn unreferenced_bytes_after_the_table_do_not_lengthen_it() {
        // Samus's `RollF`: a gap between the table and the first script. The
        // first gap word is a command, not a pointer into the file.
        let script = [cmd(1, 0, 1), 7, cmd(0, 0, 0)];
        let gap = [cmd(8, 0x3FF, 1), 1234, 0, 0];
        let mut data = Vec::new();
        let table = 2 * 4;
        let first = (table + gap.len() * 2) as u32;
        let second = first + (script.len() * 2) as u32;
        data.extend_from_slice(&first.to_be_bytes());
        data.extend_from_slice(&second.to_be_bytes());
        for w in gap.iter().chain(&script).chain(&script) {
            data.extend_from_slice(&w.to_be_bytes());
        }
        assert_eq!(joint_table_len(&data), Some(2));
        let f = File {
            id: 1,
            data,
            extern_relocs: Vec::new(),
            intern_relocs: Vec::new(),
        };
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
            // Every fighter has every *timed* slot; the looping ones may be
            // absent where the character lacks the move.
            assert!(anims.files[..TIMED_SLOTS].iter().all(|&f| f != 0));
        }
    }

    #[test]
    fn the_slot_names_are_in_slot_order_and_the_timed_ones_come_first() {
        // `ssb_game::status::Status::anim_slot` repeats this numbering, because
        // Layer A must not depend on the pack format. If the generator ever
        // reorders `SLOTS`, this is what says so before a fighter starts
        // walking with a crouch animation.
        assert_eq!(SLOT_NAMES.len(), SLOT_COUNT);
        assert_eq!(
            &SLOT_NAMES[..TIMED_SLOTS],
            &["Dash", "Turn", "RunBrake", "Squat", "SquatRv", "Landing", "Pass"]
        );
        assert_eq!(SLOT_NAMES[SLOT_WAIT], "Wait");
        assert_eq!(SLOT_NAMES[SLOT_WALK_SLOW], "WalkSlow");
        assert_eq!(SLOT_NAMES[SLOT_RUN], "Run");
        assert_eq!(SLOT_NAMES[SLOT_KNEE_BEND], "KneeBend");
        assert_eq!(SLOT_NAMES[SLOT_JUMP_F], "JumpF");
        assert_eq!(SLOT_NAMES[SLOT_FALL], "Fall");
        assert_eq!(SLOT_NAMES[SLOT_SQUAT_WAIT], "SquatWait");
        assert_eq!(SLOT_NAMES[SLOT_MARIO_SPECIAL_N], "MarioSpecialN");
        assert_eq!(SLOT_NAMES[SLOT_MARIO_SPECIAL_AIR_N], "MarioSpecialAirN");
        assert_eq!(SLOT_NAMES[SLOT_MARIO_SPECIAL_HI], "MarioSpecialHi");
        assert_eq!(SLOT_NAMES[SLOT_MARIO_SPECIAL_AIR_HI], "MarioSpecialAirHi");
        assert_eq!(SLOT_NAMES[SLOT_MARIO_SPECIAL_LW], "MarioSpecialLw");
        assert_eq!(SLOT_NAMES[SLOT_MARIO_SPECIAL_AIR_LW], "MarioSpecialAirLw");
    }

    #[test]
    fn only_the_moves_a_character_lacks_are_absent() {
        // Kirby and Jigglypuff have no aerial jump; nobody is missing a timed
        // slot, which is what `decode_fighter` would trip over.
        for a in FIGHTER_ANIMS {
            for (slot, &file) in a.files.iter().enumerate() {
                if slot < TIMED_SLOTS {
                    assert_ne!(file, 0, "{} has no {}", a.name, SLOT_NAMES[slot]);
                }
            }
        }
        let missing: usize = FIGHTER_ANIMS[..13]
            .iter()
            .map(|a| a.files.iter().filter(|&&f| f == 0).count())
            .sum();
        // The attack slots lack the 106 motions the twelve do not have
        // (mid-angle tilts and smashes, `AttackHi3F`/`B`, null
        // `LandingAirX`), and only Mario and Luigi have `MarioAttack13`.
        // The remaining shared slots exist for all twelve, except that
        // Pikachu has no `YoshiEgg` motion. Of the seven entry slots, two
        // exist for each one-part entry, four for Captain Falcon's and five
        // for Ness's. The five demo slots exist for the twelve and Master
        // Hand (his one default clip) (RE-408). Of the campaign's four, the
        // twelve lack only IntroR for Captain Falcon, Jigglypuff and Ness,
        // who are never a card's enemy; Master Hand lacks IntroL. Master
        // Hand's 30 own slots exist for him alone; his common statuses all
        // play one idle. The opening's 14 slots (RE-467) exist 61 times:
        // DemoRun for the eight starters, Captain Falcon, Ness and Master
        // Hand; DemoJump for Link and Master Hand; FigurePulled for the
        // starters and Master Hand; Clash and Stance for the starters; and
        // the fighters' own rows as far as each table goes (Fox's nine).
        assert_eq!(missing, 5807, "Twelve fighters and Master Hand");
        // Metal Mario, the Polygons and Giant Donkey Kong carry every slot
        // their base fighter has (their `dFT<Name>MotionDescs` index the
        // base's motion enum), except: the demo and continue rows and
        // IntroL their submotion tables lack (only IntroR exists, and not
        // for Poly Luigi, Poly Jigglypuff and Giant Donkey Kong; Poly
        // Captain Falcon and Poly Ness have the IntroR their bases lack);
        // Giant Donkey Kong's null `YoshiEgg` (Poly Pikachu has one, Poly
        // Luigi's plays Mario's Wait); and the specials and copies of Poly
        // Kirby and Poly Pikachu, whose tables drop rows among their own
        // motions and who have no specials (`is_have_special*` 0).
        let base = |kind: usize| match kind {
            13 | 14 => 0,
            26 => 2,
            k => k - 14,
        };
        for (kind, a) in FIGHTER_ANIMS.iter().enumerate().skip(13) {
            let b = &FIGHTER_ANIMS[base(kind)];
            for (slot, name) in SLOT_NAMES.iter().enumerate() {
                let (have, base_has) = (a.files[slot] != 0, b.files[slot] != 0);
                // The opening's slots come from the twelve's own submotion
                // rows, which the variants' tables do not carry (RE-467).
                if have == base_has
                    || (SLOT_WIN1..=SLOT_INTRO_R).contains(&slot)
                    || slot >= SLOT_DEMO_RUN
                    || (*name == "YoshiEgg" && matches!(kind, 23 | 26))
                {
                    continue;
                }
                let special = name.starts_with("KirbySpecial")
                    || name.starts_with("KirbyCopy")
                    || name.starts_with("PikachuSpecial");
                assert!(
                    matches!(kind, 22 | 23) && special && !have,
                    "{} {} differs from {}",
                    a.name,
                    name,
                    b.name
                );
            }
        }
        for a in &FIGHTER_ANIMS[..12] {
            for (file, name) in a.files[SLOT_WALL_DAMAGE..SLOT_APPEAL]
                .iter()
                .zip(&SLOT_NAMES[SLOT_WALL_DAMAGE..SLOT_APPEAL])
            {
                assert_ne!(*file, 0, "{} has no {}", a.name, name);
            }
            // Everyone has a jab, a neutral air and a taunt.
            for slot in [SLOT_APPEAL, SLOT_APPEAL + 1, SLOT_APPEAL + 20] {
                assert_ne!(a.files[slot], 0, "{} has no {}", a.name, SLOT_NAMES[slot]);
            }
        }
        assert_eq!(SLOT_NAMES[SLOT_WALL_DAMAGE], "WallDamage");
        assert_eq!(SLOT_NAMES[SLOT_CLIFF_CATCH - 1], "FuraFura");
        assert_eq!(SLOT_NAMES[SLOT_DAMAGE_HI1 - 1], "CliffEscapeSlow2");
        assert_eq!(SLOT_NAMES[SLOT_DAMAGE_HI1], "DamageHi1");
        assert_eq!(SLOT_NAMES[SLOT_DAMAGE_HI1 + 18], "DamageFlyRoll");
        assert_eq!(SLOT_NAMES[SLOT_APPEAL - 1], "LandingFallSpecial");
        assert_eq!(SLOT_NAMES[SLOT_APPEAL], "Appeal");
        assert_eq!(SLOT_NAMES[SLOT_APPEAL + 1], "Attack11");
        assert_eq!(SLOT_NAMES[SLOT_MARIO_ATTACK13 - 1], "LandingAirNull");
        assert_eq!(SLOT_NAMES[SLOT_MARIO_ATTACK13], "MarioAttack13");
        assert_eq!(SLOT_NAMES[SLOT_REBIRTH_DOWN - 1], "MarioAttack13");
        assert_eq!(SLOT_NAMES[SLOT_REBIRTH_DOWN], "RebirthDown");
        assert_eq!(SLOT_NAMES[SLOT_LIGHT_THROW_DROP - 1], "HeavyGet");
        assert_eq!(SLOT_NAMES[SLOT_LIGHT_THROW_DROP], "LightThrowDrop");
        assert_eq!(SLOT_NAMES[SLOT_LIGHT_THROW_DROP + 41], "FireFlowerShootAir");
        assert_eq!(SLOT_NAMES[SLOT_GUARD_ON - 1], "HammerWalk");
        assert_eq!(SLOT_NAMES[SLOT_GUARD_ON], "GuardOn");
        assert_eq!(SLOT_NAMES[SLOT_GUARD_ON + 1], "GuardOff");
        assert_eq!(SLOT_NAMES[SLOT_APPEAR_R - 1], "ThrownDonkeyUnk");
        assert_eq!(SLOT_NAMES[SLOT_APPEAR_R], "AppearR");
        assert_eq!(SLOT_NAMES[SLOT_WIN1 - 1], "AppearWait");
        assert_eq!(SLOT_NAMES[SLOT_WIN1], "Win1");
        assert_eq!(SLOT_NAMES[SLOT_WIN4], "Win4");
        assert_eq!(SLOT_LOSE, SLOT_FIGURE_DROPPED - 1);
        assert_eq!(SLOT_NAMES[SLOT_LOSE], "Lose");
        assert_eq!(SLOT_NAMES[SLOT_FIGURE_DROPPED], "FigureDropped");
        assert_eq!(SLOT_NAMES[SLOT_FIGURE_STAND], "FigureStand");
        assert_eq!(SLOT_NAMES[SLOT_INTRO_L], "IntroL");
        assert_eq!(SLOT_NAMES[SLOT_INTRO_R], "IntroR");
        assert_eq!(SLOT_BOSS_DEFAULT, SLOT_INTRO_R + 1);
        assert_eq!(SLOT_NAMES[SLOT_BOSS_DEFAULT], "BossDefault");
        assert_eq!(SLOT_NAMES[SLOT_DEMO_RUN - 1], "BossAppear");
        assert_eq!(SLOT_BOSS_DEFAULT + BOSS_SLOTS, SLOT_DEMO_RUN);
        assert_eq!(SLOT_NAMES[SLOT_DEMO_RUN], "DemoRun");
        assert_eq!(SLOT_NAMES[SLOT_DEMO_JUMP], "DemoJump");
        assert_eq!(SLOT_NAMES[SLOT_FIGURE_PULLED], "FigurePulled");
        assert_eq!(SLOT_NAMES[SLOT_CLASH], "Clash");
        assert_eq!(SLOT_NAMES[SLOT_STANCE], "Stance");
        assert_eq!(SLOT_NAMES[SLOT_OPENING1], "Opening1");
        assert_eq!(SLOT_OPENING1 + OPENING_SLOTS, SLOT_COUNT);
        let mario = FIGHTER_ANIMS
            .iter()
            .find(|fighter| fighter.name == "Mario")
            .expect("Mario is in FTKind order");
        assert_eq!(mario.files[SLOT_MARIO_SPECIAL_N], 635);
        assert_eq!(mario.files[SLOT_MARIO_SPECIAL_AIR_N], 636);
        assert_eq!(mario.files[SLOT_MARIO_SPECIAL_HI], 637);
        assert_eq!(mario.files[SLOT_MARIO_SPECIAL_AIR_HI], 637);
        let donkey = FIGHTER_ANIMS
            .iter()
            .find(|fighter| fighter.name == "Donkey")
            .expect("Donkey is in FTKind order");
        assert_eq!(donkey.files[68], 907); // Jab1
        assert_eq!(donkey.files[94], 940); // Spinning Kong ground
        let samus = FIGHTER_ANIMS
            .iter()
            .find(|fighter| fighter.name == "Samus")
            .expect("Samus is in FTKind order");
        assert_eq!(samus.files[SLOT_SAMUS_ATTACK11], 1063); // Jab1
        assert_eq!(samus.files[SLOT_SAMUS_SPECIAL_N_START + 5], 1097); // Screw Attack
        assert_eq!(samus.files[SLOT_CATCH], 1015);
        let luigi = FIGHTER_ANIMS
            .iter()
            .find(|fighter| fighter.name == "Luigi")
            .expect("Luigi is in FTKind order");
        assert_eq!(luigi.files[SLOT_LUIGI_ATTACK11], 606); // Mario's Jab1
        assert_eq!(luigi.files[SLOT_LUIGI_ATTACK11 + 3], 1108); // DashAttack
        assert_eq!(luigi.files[SLOT_LUIGI_ATTACK11 + 11], 1112); // FSmash
        assert_eq!(luigi.files[SLOT_MARIO_SPECIAL_HI], 637);
        assert_eq!(luigi.files[SLOT_CATCH], 561);
        // Mario's own attack slots name the files Luigi borrows.
        assert_eq!(mario.files[SLOT_APPEAL + 1], 606); // Jab1
        assert_eq!(
            mario.files[SLOT_MARIO_ATTACK13],
            luigi.files[SLOT_LUIGI_ATTACK11 + 2]
        );
        assert_eq!(luigi.files[SLOT_APPEAL + 1], 606);
        let link = FIGHTER_ANIMS
            .iter()
            .find(|fighter| fighter.name == "Link")
            .expect("Link is in FTKind order");
        assert_eq!(link.files[SLOT_LINK_ATTACK11], 1223); // Jab1
        assert_eq!(link.files[SLOT_LINK_ATTACK13], 1225); // Jab3
                                                          // The Boomerang throw and its empty-handed variant share a figatree.
        assert_eq!(link.files[SLOT_LINK_ATTACK13 + 7], 1251);
        assert_eq!(link.files[SLOT_LINK_ATTACK13 + 9], 1251);
        assert_eq!(link.files[SLOT_CATCH], 1177);
        let yoshi = FIGHTER_ANIMS
            .iter()
            .find(|fighter| fighter.name == "Yoshi")
            .expect("Yoshi is in FTKind order");
        assert_ne!(yoshi.files[SLOT_YOSHI_ATTACK11], 0);
        assert_ne!(yoshi.files[SLOT_YOSHI_SPECIAL_HI], 0);
        assert_ne!(yoshi.files[SLOT_CATCH], 0);
        let captain = FIGHTER_ANIMS
            .iter()
            .find(|fighter| fighter.name == "Captain")
            .expect("Captain is in FTKind order");
        assert_eq!(captain.files[SLOT_CAPTAIN_ATTACK11], 1619);
        assert_eq!(captain.files[SLOT_CAPTAIN_ATTACK13], 1621);
        assert_eq!(captain.files[SLOT_CAPTAIN_ATTACK13 + 11], 1658);
        let kirby = FIGHTER_ANIMS
            .iter()
            .find(|fighter| fighter.name == "Kirby")
            .expect("Kirby is in FTKind order");
        assert_eq!(kirby.files[SLOT_KIRBY_ATTACK11], 1373);
        assert_eq!(kirby.files[SLOT_KIRBY_ATTACK100_START], 1375);
        assert_eq!(kirby.files[SLOT_KIRBY_ATTACK100_START + 30], 1430);
        assert_eq!(kirby.files[SLOT_KIRBY_COPY_MARIO_SPECIAL_N], 1397);
        assert_eq!(kirby.files[SLOT_KIRBY_COPY_MARIO_SPECIAL_N + 24], 1442);
        assert_ne!(kirby.files[SLOT_CATCH], 0);
        assert_ne!(captain.files[SLOT_CATCH], 0);
        let purin = FIGHTER_ANIMS
            .iter()
            .find(|fighter| fighter.name == "Purin")
            .expect("Jigglypuff is in FTKind order");
        assert_eq!(purin.files[SLOT_PURIN_ATTACK11], 1446); // Jab1
        assert_eq!(purin.files[SLOT_PURIN_ATTACK11 + 16], 1392); // Kirby's LandingAirF
        assert_eq!(purin.files[SLOT_PURIN_JUMP_AERIAL_F1], 1471); // Jump2
        assert_eq!(purin.files[SLOT_PURIN_JUMP_AERIAL_F1 + 8], 1501); // Rest
        assert_eq!(kirby.files[SLOT_KIRBY_COPY_PURIN_SPECIAL_N], 1502); // Pound
        assert_eq!(purin.files[SLOT_FURA_SLEEP], kirby.files[SLOT_FURA_SLEEP]); // Kirby's Sleep
        assert_eq!(mario.files[SLOT_FURA_SLEEP], 522);

        // Cargo reuses one held-pose file for five statuses, and both cargo
        // throws share one figatree (`dFTDonkeyMotionDescs`).
        assert_eq!(donkey.files[SLOT_DONKEY_THROWF_WAIT], 946);
        assert_eq!(donkey.files[SLOT_DONKEY_THROWF_WAIT + 8], 946);
        assert_eq!(donkey.files[SLOT_DONKEY_THROWF_WAIT + 9], 945);
        assert_eq!(mario.files[SLOT_CATCH], 561);
        assert_eq!(donkey.files[SLOT_CATCH + 3], 865); // ThrowB

        // Only Donkey Kong's own table has a `ThrownFoxFStart` motion; Mario
        // and Fox carry null placeholders there.
        assert_eq!(mario.files[SLOT_CATCH + 7], 0);
        assert_eq!(donkey.files[SLOT_CATCH + 7], 868);
        assert_eq!(EXPECTED_FRAMES[0][SLOT_CATCH], 16);
        assert_eq!(EXPECTED_FRAMES[2][SLOT_CATCH + 2], 20); // Donkey ThrowF
        assert_eq!(mario.files[SLOT_MARIO_SPECIAL_LW], 638);
        assert_eq!(mario.files[SLOT_MARIO_SPECIAL_AIR_LW], 639);
        assert_eq!(
            EXPECTED_FRAMES[0][SLOT_MARIO_SPECIAL_HI], 40,
            "Super Jump Punch is a 40-frame figatree"
        );
        assert_eq!(EXPECTED_FRAMES[0][SLOT_MARIO_SPECIAL_LW], 87);
        assert_eq!(EXPECTED_FRAMES[0][SLOT_MARIO_SPECIAL_AIR_LW], 83);
        for fighter in FIGHTER_ANIMS {
            let has_mario_specials = matches!(
                fighter.name,
                "Mario" | "Luigi" | "MMario" | "NMario" | "NLuigi"
            );
            assert_eq!(
                fighter.files[SLOT_MARIO_SPECIAL_HI] != 0,
                has_mario_specials
            );
            assert_eq!(
                fighter.files[SLOT_MARIO_SPECIAL_AIR_HI] != 0,
                has_mario_specials
            );
            assert_eq!(
                fighter.files[SLOT_MARIO_SPECIAL_LW] != 0,
                has_mario_specials
            );
            assert_eq!(
                fighter.files[SLOT_MARIO_SPECIAL_AIR_LW] != 0,
                has_mario_specials
            );
        }
    }
}
