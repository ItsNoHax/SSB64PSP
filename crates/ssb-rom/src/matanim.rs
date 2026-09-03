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
use crate::figatree::{Aobj, Kind};

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
    /// `PaletteID` (joint track [`TRACK_PALETTE_ID`]), cast the same way the
    /// real draw path casts it (`(s32)mobj->palette_id`) — an index into
    /// `MObjSub.palettes[]`, not a colour. RE-096: 45% (200/441) of real
    /// fighter costume scripts carry one, which `colors_at` did not read
    /// until this field existed.
    pub palette_id: Option<i32>,
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
    /// `gcPlayMObjMatAnim`'s `nGCAnimKindStep` branch: base before the step
    /// fires, target after.
    fn resolved(&self) -> u32 {
        if self.length_invert <= self.length {
            self.target
        } else {
            self.base
        }
    }

    /// The resolved word, reinterpreted as colour bytes.
    fn color(&self) -> [u8; 4] {
        self.resolved().to_be_bytes()
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
    // Only [`TRACK_PALETTE_ID`] of the ten joint tracks is read (RE-096); the
    // costume-selection use case has no need for the other nine yet.
    let mut palette = Track::default();
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
                } else if i == TRACK_PALETTE_ID {
                    palette.live = true;
                    palette.base = palette.target;
                    palette.target = value;
                    palette.step = matches!(opcode, OP_SET_VAL_AFTER_BLOCK | OP_SET_VAL_AFTER);
                    palette.length_invert = payload;
                    palette.length = -anim_wait - SPEED;
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
    let palette_id = (palette.live && palette.step).then(|| {
        let mut t = palette;
        t.length += SPEED;
        // The raw word is a genuine `f32` (RE-087), not colour bytes, then
        // cast the same way `objdisplay.c`'s `(s32)mobj->palette_id` does.
        f32::from_bits(t.resolved()) as i32
    });
    Ok(Colors {
        prim: read(TRACK_PRIM),
        env: read(TRACK_ENV),
        blend: read(TRACK_BLEND),
        palette_id,
    })
}

/// Resolves an `AObjEvent32 ***` table into one script address per
/// `(node, MObj-chain-position)`, without evaluating anything.
///
/// Both known instances of this shape — a fighter's
/// `FTCommonPart::p_costume_matanim_joints` and a stage layer's
/// `MPGroundDesc::p_matanim_joints` — are laid out identically: the outer
/// array is parallel to the `DObjDesc` array, and each present entry points
/// at a further array parallel to that node's own `MObjSub` chain, walked in
/// lockstep by the decompiled code that consumes it
/// (`lbCommonAddMObjForFighterPartsDObj` for costumes,
/// `gcAddMatAnimJointAll` for a stage layer). `chain_len(node)` is normally a
/// closure over a chain length a caller already has from resolving the
/// parallel `MObjSub ***` table (RE-086's temporary census confirmed this
/// indexing scheme against real stage data before this became a permanent
/// function).
///
/// Same-file only: `table` and every pointer it holds are read against
/// `file`'s own bytes. A stage layer whose `p_matanim_joints` targets a
/// different archive file cannot be resolved this way (RE-086 found this gap
/// but did not attempt to close it; RE-089 confirms it is rare).
pub fn resolve_scripts(
    file: &File,
    table: u32,
    nodes: usize,
    chain_len: impl Fn(usize) -> usize,
) -> alloc::vec::Vec<alloc::vec::Vec<Option<u32>>> {
    (0..nodes)
        .map(|node| {
            let per_node = u32_at(&file.data, table as usize + node * 4).unwrap_or(0);
            (0..chain_len(node))
                .map(|m| {
                    if per_node == 0 {
                        return None;
                    }
                    let script = u32_at(&file.data, per_node as usize + m * 4)?;
                    (script != 0).then_some(script)
                })
                .collect()
        })
        .collect()
}

/// `FTCommonPart::p_costume_matanim_joints`, resolved to one script per
/// `(node, material)` and evaluated at `costume`.
///
/// See [`resolve_scripts`] for the table shape; this layers [`colors_at`]'s
/// one-shot evaluation on top of it.
pub fn costume_colors(
    file: &File,
    table: u32,
    nodes: usize,
    chain_len: impl Fn(usize) -> usize,
    costume: f32,
) -> alloc::vec::Vec<alloc::vec::Vec<Option<Colors>>> {
    resolve_scripts(file, table, nodes, chain_len)
        .into_iter()
        .map(|chain| {
            chain
                .into_iter()
                .map(|script| script.and_then(|s| colors_at(&file.data, s, costume).ok()))
                .collect()
        })
        .collect()
}

/// A persistent, per-tick material-track player (`gcPlayMObjMatAnim`'s real
/// shape), as opposed to [`colors_at`]'s one-shot "evaluate at frame N"
/// reader built for fighter costume selection.
///
/// [`colors_at`] only ever reads the five colour tracks (`nGCAnimTrackMaterialSubStart`
/// onward) and declines `JUMP`, because a costume list never uses either —
/// each costume is one more key in a script that is evaluated once, not
/// played continuously. A *general* material animation (RE-086: 71% of the
/// 172 real stage scripts archive-wide cycle `PaletteID`, not colour) is a
/// different shape entirely: it runs forever, loops via `SET_ANIM`/`JUMP`,
/// and drives the ten texture/UV/palette tracks
/// (`nGCAnimTrackMaterialStart` onward) far more often than colour.
///
/// This reuses [`crate::figatree::Aobj`]/[`Kind`] — the same interpolation
/// state [`crate::objanim::StageJoint`] already plays joint tracks with —
/// over a *unified* 15-track window (`TRACK_COUNT`): the ten material tracks
/// at indices `0..10`, then the five colour tracks at `10..15`. The two
/// windows differ in one real way, not just index range: a material track's
/// raw word *is* an `f32` (`nGCAnimTrackTraU`'s rate really is a small float
/// like `-0.012`, confirmed against a real ROM script), while a colour
/// track's raw word is four RGBA bytes reinterpreted, never arithmetic. This
/// is safe to store in the same `f32` slots as long as a colour track only
/// ever uses `Kind::Step` (never `Linear`/`Cubic`, which would perform real
/// arithmetic on the bit-transmuted value and corrupt it) — [`colors_at`]'s
/// own `read` closure already declines a colour track under any other kind,
/// so this matches an existing, accepted limitation rather than introducing
/// a new one.
#[derive(Clone, Copy)]
pub struct MaterialJoint {
    tracks: [Aobj; TICK_TRACK_COUNT],
    anim_wait: f32,
    pc: usize,
    ended: bool,
    start: usize,
}

/// Ten material tracks (`nGCAnimTrackMaterialStart..`) plus five colour
/// tracks (`nGCAnimTrackMaterialSubStart..`), unified into one index space.
pub const TICK_TRACK_COUNT: usize = 15;

pub const TRACK_TEXTURE_ID_CURRENT: usize = 0;
pub const TRACK_TRA_U: usize = 1;
pub const TRACK_TRA_V: usize = 2;
pub const TRACK_SCA_U: usize = 3;
pub const TRACK_SCA_V: usize = 4;
pub const TRACK_TEXTURE_ID_NEXT: usize = 5;
pub const TRACK_SCR_U: usize = 6;
pub const TRACK_SCR_V: usize = 7;
pub const TRACK_SET_LFRAC: usize = 8;
pub const TRACK_PALETTE_ID: usize = 9;
/// Colour-track window start; `TRACK_PRIM`/`TRACK_ENV`/`TRACK_BLEND` (this
/// module's top) are relative to `0`, not this window, so a caller reading
/// both windows through [`MaterialJoint`] adds this offset itself.
pub const TICK_EXT_START: usize = 10;
pub const TRACK_PRIM_COLOR: usize = TICK_EXT_START;
pub const TRACK_ENV_COLOR: usize = TICK_EXT_START + 1;
pub const TRACK_BLEND_COLOR: usize = TICK_EXT_START + 2;
pub const TRACK_LIGHT1_COLOR: usize = TICK_EXT_START + 3;
pub const TRACK_LIGHT2_COLOR: usize = TICK_EXT_START + 4;

const OP_ADD_LENGTH: u32 = 12;
const OP_SET_INTERP: u32 = 13;
const OP_SET_ANIM: u32 = 14;
const OP_SET_FLAGS: u32 = 15;

/// How many value words a command reads per set track, for every opcode
/// [`MaterialJoint`] models (a superset of [`values_per_track`]: also the
/// zero-value control opcodes `colors_at` never needed to name because a
/// costume list never uses them).
fn tick_values_per_track(opcode: u32) -> Option<usize> {
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
        OP_END | OP_JUMP | OP_WAIT | OP_ADD_LENGTH | OP_SET_INTERP | OP_SET_ANIM
        | OP_SET_FLAGS => 0,
        _ => return None,
    })
}

impl MaterialJoint {
    pub fn start(script: u32, frame: f32) -> Self {
        MaterialJoint {
            tracks: [Aobj::default(); TICK_TRACK_COUNT],
            anim_wait: -frame,
            pc: script as usize,
            ended: false,
            start: script as usize,
        }
    }

    pub fn ended(&self) -> bool {
        self.ended
    }

    /// Whether the script has looped back to where it began.
    pub fn looped(&self) -> bool {
        self.pc == self.start
    }

    /// The current value of one of the 15 unified tracks, or `None` if the
    /// script has never set it. A colour track's value is only trustworthy
    /// (see this type's own doc comment) when [`Self::track_is_stepped`]
    /// also returns `true` for it.
    pub fn track_value(&self, track: usize) -> Option<f32> {
        let t = self.tracks.get(track)?;
        (t.kind != Kind::None).then(|| t.value())
    }

    /// Whether a track was last set by a step (`_AFTER`/`EXT_..._AFTER`)
    /// command, the only kind safe to reinterpret as raw colour bytes or a
    /// discrete index (`PaletteID`, `TextureIDCurrent`) rather than a
    /// genuinely interpolating quantity.
    pub fn track_is_stepped(&self, track: usize) -> bool {
        self.tracks.get(track).is_some_and(|t| t.kind == Kind::Step)
    }

    /// Advances one tick, parsing new commands only if the clock allows it.
    pub fn tick(&mut self, data: &[u8], speed: f32) -> Result<(), MatAnimError> {
        self.parse(data, speed)?;
        if !self.ended {
            for t in self.tracks.iter_mut() {
                if t.kind != Kind::None {
                    t.length += speed;
                }
            }
        }
        Ok(())
    }

    /// `gcParseMObjMatAnim`: run commands until one blocks past now.
    fn parse(&mut self, data: &[u8], speed: f32) -> Result<(), MatAnimError> {
        if self.ended {
            return Ok(());
        }
        self.anim_wait -= speed;
        if self.anim_wait > 0.0 {
            return Ok(());
        }

        for _ in 0..4096 {
            let at = self.pc;
            let word = u32_at(data, at).ok_or(MatAnimError::Truncated { at })?;
            let opcode = word >> 25;
            let flags = (word >> 15) & 0x3FF;
            let payload = (word & 0x7FFF) as f32;
            self.pc += 4;

            match opcode {
                OP_END => {
                    for t in self.tracks.iter_mut() {
                        if t.kind != Kind::None {
                            t.length += speed + self.anim_wait;
                        }
                    }
                    self.ended = true;
                    return Ok(());
                }
                // `SetAnim` additionally rebases `anim_frame`, which nothing
                // here reads (matches `objanim::StageJoint`'s own choice).
                OP_JUMP | OP_SET_ANIM => {
                    let target =
                        u32_at(data, self.pc).ok_or(MatAnimError::Truncated { at })?;
                    self.pc = target as usize;
                    if self.pc == at {
                        return Err(MatAnimError::TooLong);
                    }
                }
                OP_SET_FLAGS => self.anim_wait += payload,
                OP_ADD_LENGTH => {
                    for i in 0..TICK_TRACK_COUNT.min(10) {
                        if flags & (1 << i) != 0 {
                            self.tracks[i].length += payload;
                        }
                    }
                }
                OP_SET_INTERP => self.pc += 4,
                _ => {
                    let per = tick_values_per_track(opcode)
                        .ok_or(MatAnimError::UnknownOpcode { opcode, at })?;
                    self.pc = self.apply(data, opcode, flags, payload, per)?;
                    if blocks(opcode) {
                        self.anim_wait += payload;
                    }
                }
            }
            if self.anim_wait > 0.0 {
                return Ok(());
            }
        }
        Err(MatAnimError::TooLong)
    }

    /// Sets the tracks a command names, returning the new program counter.
    ///
    /// A material-window opcode (`3..=11`) addresses tracks `0..10`; its
    /// colour-window counterpart (`18..=21`) addresses the same shape of
    /// command but tracks `10..15` — [`is_ext`] already draws that line.
    fn apply(
        &mut self,
        data: &[u8],
        opcode: u32,
        flags: u32,
        payload: f32,
        per: usize,
    ) -> Result<usize, MatAnimError> {
        let mut pc = self.pc;
        let (base, count) = if is_ext(opcode) {
            (TICK_EXT_START, TRACK_COUNT)
        } else {
            (0, MAT_TRACK_COUNT)
        };
        let mut bits = flags;
        for i in 0..count {
            if bits == 0 {
                break;
            }
            if bits & 1 != 0 {
                let raw = u32_at(data, pc).ok_or(MatAnimError::Truncated { at: pc })?;
                pc += 4;
                let second = if per == 2 {
                    let v = u32_at(data, pc).ok_or(MatAnimError::Truncated { at: pc })?;
                    pc += 4;
                    Some(f32::from_bits(v))
                } else {
                    None
                };
                let value = f32::from_bits(raw);

                let t = &mut self.tracks[base + i];
                t.value_base = t.value_target;
                t.value_target = value;
                t.length = -self.anim_wait;
                if payload != 0.0 {
                    t.length_invert = 1.0 / payload;
                }

                match opcode {
                    OP_SET_VAL0_RATE_BLOCK | OP_SET_VAL0_RATE => {
                        t.rate_base = t.rate_target;
                        t.rate_target = 0.0;
                        t.kind = Kind::Cubic;
                    }
                    OP_SET_VAL_RATE_BLOCK | OP_SET_VAL_RATE => {
                        t.rate_base = t.rate_target;
                        t.rate_target = second.unwrap_or(0.0);
                        t.kind = Kind::Cubic;
                    }
                    OP_SET_TARGET_RATE => {
                        t.rate_target = value;
                        t.value_target = t.value_base;
                        t.kind = Kind::Cubic;
                    }
                    OP_SET_VAL_AFTER_BLOCK
                    | OP_SET_VAL_AFTER
                    | OP_EXT_VAL_AFTER_BLOCK
                    | OP_EXT_VAL_AFTER => {
                        t.length_invert = payload;
                        t.kind = Kind::Step;
                    }
                    _ => {
                        // `SET_VAL(_BLOCK)`/`EXT_VAL(_BLOCK)`: linear ramp.
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
            bits >>= 1;
        }
        Ok(pc)
    }
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

    #[test]
    fn a_script_that_never_sets_palette_id_leaves_it_unset() {
        // Mario's arm (colour only) must not fabricate a palette selection.
        let d = mario_arm();
        assert_eq!(colors_at(&d, 0, 0.0).unwrap().palette_id, None);
    }

    /// A costume list carrying `PaletteID` (joint track 9) instead of colour —
    /// the real archive-wide majority shape (RE-096: 45% of 441 fighter
    /// costume scripts), same one-costume-per-`Wait(1)`-block layout as
    /// `mario_arm`.
    fn palette_costume_script() -> alloc::vec::Vec<u8> {
        script(&[
            cmd(OP_SET_VAL_AFTER_BLOCK, 1 << TRACK_PALETTE_ID, 0),
            0.0f32.to_bits(),
            cmd(OP_SET_VAL_AFTER_BLOCK, 1 << TRACK_PALETTE_ID, 1),
            1.0f32.to_bits(),
            cmd(OP_SET_VAL_AFTER_BLOCK, 1 << TRACK_PALETTE_ID, 1),
            3.0f32.to_bits(),
            cmd(OP_WAIT, 0, 97), // parks the clock past every costume
            cmd(OP_END, 0, 0),
        ])
    }

    #[test]
    fn palette_id_steps_one_costume_per_frame_like_colour_does() {
        let d = palette_costume_script();
        let at = |c: f32| colors_at(&d, 0, c).unwrap().palette_id;
        assert_eq!(at(0.0), Some(0));
        assert_eq!(at(1.0), Some(1));
        assert_eq!(at(2.0), Some(3), "the raw f32 word, cast to i32, not the step index");
    }

    #[test]
    fn palette_id_is_read_from_a_real_ieee754_bit_pattern_not_an_integer() {
        // RE-087: a real script's word is `0x3F800000`-style IEEE-754, not a
        // small integer reinterpreted — reading it as anything else would
        // give nonsense for every real archive script.
        let d = script(&[
            cmd(OP_SET_VAL_AFTER_BLOCK, 1 << TRACK_PALETTE_ID, 0),
            0x3F80_0000, // 1.0f32
            cmd(OP_WAIT, 0, 97),
            cmd(OP_END, 0, 0),
        ]);
        assert_eq!(colors_at(&d, 0, 0.0).unwrap().palette_id, Some(1));
    }

    #[test]
    fn palette_id_and_colour_survive_in_the_same_script() {
        // A costume list can carry both a colour track and `PaletteID` at
        // once, each with its own independent step sequence; reading one
        // must not clobber the other.
        let d = script(&[
            cmd(OP_EXT_VAL_AFTER_BLOCK, 1 << TRACK_PRIM, 0),
            0xff00_00ff,
            cmd(OP_SET_VAL_AFTER_BLOCK, 1 << TRACK_PALETTE_ID, 0),
            0.0f32.to_bits(),
            cmd(OP_SET_VAL_AFTER_BLOCK, 1 << TRACK_PALETTE_ID, 1),
            2.0f32.to_bits(),
            cmd(OP_WAIT, 0, 97),
            cmd(OP_END, 0, 0),
        ]);
        let c = colors_at(&d, 0, 1.0).unwrap();
        assert_eq!(c.prim, Some([0xff, 0x00, 0x00, 0xff]), "unaffected by frame");
        assert_eq!(c.palette_id, Some(2), "palette's own second step");
    }

    fn file_of(data: alloc::vec::Vec<u8>) -> crate::archive::File {
        crate::archive::File {
            id: 0,
            data,
            extern_relocs: alloc::vec::Vec::new(),
            intern_relocs: alloc::vec::Vec::new(),
        }
    }

    /// A two-node table: node 0 has no script list at all, node 1's list
    /// supplies a script for chain position 0 but not position 1 — the exact
    /// "some MObjs animate, some don't" shape a real table has.
    fn resolve_fixture() -> crate::archive::File {
        const TABLE: usize = 0x00;
        const LIST_1: usize = 0x10;
        const SCRIPT_A: u32 = 0x40;

        let mut data = alloc::vec![0u8; 0x50];
        // node 0 stays a NULL entry.
        data[TABLE + 4..TABLE + 8].copy_from_slice(&(LIST_1 as u32).to_be_bytes());
        data[LIST_1..LIST_1 + 4].copy_from_slice(&SCRIPT_A.to_be_bytes());
        // LIST_1 + 4 (chain position 1) stays zero: no script for that MObj.
        file_of(data)
    }

    #[test]
    fn resolve_scripts_finds_one_entry_per_node_and_chain_position() {
        let file = resolve_fixture();
        let out = resolve_scripts(&file, 0x00, 2, |node| if node == 1 { 2 } else { 0 });
        assert_eq!(out[0], alloc::vec::Vec::new(), "node 0 has no script list");
        assert_eq!(out[1], alloc::vec![Some(0x40), None]);
    }

    #[test]
    fn costume_colors_is_unaffected_by_being_layered_on_resolve_scripts() {
        // Behaviour-preserving refactor check: costume_colors used to inline
        // this walk itself. Same fixture as the original per-frame test,
        // reached through the table-resolution path instead of a bare script
        // address.
        const TABLE: usize = 0x00;
        const LIST_0: usize = 0x10;
        const SCRIPT: u32 = 0x20;
        let mut data = alloc::vec![0u8; 0x20];
        data[TABLE..TABLE + 4].copy_from_slice(&(LIST_0 as u32).to_be_bytes());
        data[LIST_0..LIST_0 + 4].copy_from_slice(&SCRIPT.to_be_bytes());
        data.extend(mario_arm());
        let file = file_of(data);

        let out = costume_colors(&file, 0x00, 1, |_| 1, 4.0);
        assert_eq!(out[0][0].unwrap().prim, Some([0x00, 0xce, 0x00, 0xff]));
    }
}

#[cfg(test)]
mod tick_tests {
    use super::*;

    fn script(words: &[u32]) -> alloc::vec::Vec<u8> {
        words.iter().flat_map(|w| w.to_be_bytes()).collect()
    }

    const fn cmd(opcode: u32, flags: u32, payload: u32) -> u32 {
        (opcode << 25) | (flags << 15) | payload
    }

    #[test]
    fn a_palette_step_switches_after_its_payload_frames() {
        // RE-086/RE-087's real shape: PaletteID (track 9) steps to 0
        // immediately (payload 0), then to 1 after 3 more frames.
        let d = script(&[
            cmd(OP_SET_VAL_AFTER_BLOCK, 1 << TRACK_PALETTE_ID, 0),
            0.0f32.to_bits(),
            cmd(OP_SET_VAL_AFTER_BLOCK, 1 << TRACK_PALETTE_ID, 3),
            1.0f32.to_bits(),
            cmd(OP_END, 0, 0),
        ]);
        let mut j = MaterialJoint::start(0, 0.0);
        j.tick(&d, 1.0).expect("ticks");
        assert_eq!(j.track_value(TRACK_PALETTE_ID), Some(0.0), "steps immediately");
        assert!(j.track_is_stepped(TRACK_PALETTE_ID));

        j.tick(&d, 1.0).expect("ticks");
        assert_eq!(
            j.track_value(TRACK_PALETTE_ID),
            Some(1.0),
            "steps to the target once its payload has elapsed"
        );
    }

    #[test]
    fn raw_words_are_read_as_real_floats_not_integers() {
        // A real ROM script's PaletteID values are 0x3F800000 etc -- IEEE-754
        // bit patterns for small integers, not the integers' own bit
        // patterns. Reading them any other way would read "1" as a huge,
        // meaningless palette index.
        let d = script(&[
            cmd(OP_SET_VAL_AFTER_BLOCK, 1 << TRACK_PALETTE_ID, 0),
            0x3F80_0000,
            cmd(OP_END, 0, 0),
        ]);
        let mut j = MaterialJoint::start(0, 0.0);
        j.tick(&d, 1.0).expect("ticks");
        assert_eq!(j.track_value(TRACK_PALETTE_ID), Some(1.0));
    }

    #[test]
    fn set_anim_makes_the_script_cycle_forever_instead_of_ending() {
        // The real archive-wide pattern (RE-086/RE-087): a PaletteID cycle
        // ending in `SET_ANIM` back to offset 0, not a plain `END`. Ticking
        // well past the script's own length must keep producing the cycle,
        // not stop or error, which is what `Self::ended` and a working
        // `SET_ANIM` jump together are for.
        let d = script(&[
            cmd(OP_SET_VAL_AFTER_BLOCK, 1 << TRACK_PALETTE_ID, 0),
            0.0f32.to_bits(),
            cmd(OP_SET_VAL_AFTER_BLOCK, 1 << TRACK_PALETTE_ID, 2),
            1.0f32.to_bits(),
            cmd(OP_SET_ANIM, 0, 0),
            0, // jump target: offset 0, this script's own start
        ]);
        let mut j = MaterialJoint::start(0, 0.0);
        let mut values = alloc::vec::Vec::new();
        for _ in 0..12 {
            j.tick(&d, 1.0).expect("ticks");
            assert!(!j.ended(), "a loop is not the same as stopping");
            values.push(j.track_value(TRACK_PALETTE_ID).unwrap());
        }
        // Both values must actually appear -- a script that got stuck
        // reading garbage past its own end, or one that silently stayed on
        // its first value forever, would each produce a degenerate sequence
        // that still "ticks without erroring".
        assert!(values.contains(&0.0), "{values:?}");
        assert!(values.contains(&1.0), "{values:?}");
    }

    #[test]
    fn an_unknown_opcode_is_an_error_not_a_skip() {
        let d = script(&[cmd(23, 0, 0), 0]);
        let mut j = MaterialJoint::start(0, 0.0);
        assert!(matches!(
            j.tick(&d, 1.0),
            Err(MatAnimError::UnknownOpcode { opcode: 23, .. })
        ));
    }

    #[test]
    fn a_colour_track_set_by_a_ramp_is_not_trusted_as_a_step() {
        // `EXT_VAL` (a ramp, not a step) on a colour track would corrupt the
        // bit-transmuted RGBA bytes with real arithmetic if read back as a
        // colour -- `track_is_stepped` is what a caller must check first,
        // mirroring `colors_at`'s own established limitation for this exact
        // case rather than silently trusting a ramped colour.
        let d = script(&[
            cmd(OP_EXT_VAL, 1 << 0, 4), // PrimColor (ext track 0), ramp
            0x00FF_00FFu32,
            cmd(OP_END, 0, 0),
        ]);
        let mut j = MaterialJoint::start(0, 0.0);
        j.tick(&d, 1.0).expect("ticks");
        assert!(j.track_value(TRACK_PRIM_COLOR).is_some());
        assert!(!j.track_is_stepped(TRACK_PRIM_COLOR));
    }

    #[test]
    fn a_stepped_colour_track_round_trips_its_raw_bytes() {
        // The colour window's own `_AFTER` opcode (18) is the one real
        // stage/costume scripts actually use for colour -- confirms the same
        // bit-transmutation trick that works for `PaletteID` also round-trips
        // real RGBA bytes losslessly through `Kind::Step`.
        let rgba: u32 = 0x11223344;
        let d = script(&[
            cmd(OP_EXT_VAL_AFTER_BLOCK, 1 << 0, 0),
            rgba,
            cmd(OP_END, 0, 0),
        ]);
        let mut j = MaterialJoint::start(0, 0.0);
        j.tick(&d, 1.0).expect("ticks");
        assert!(j.track_is_stepped(TRACK_PRIM_COLOR));
        let got = j.track_value(TRACK_PRIM_COLOR).unwrap().to_bits();
        assert_eq!(got, rgba);
    }

    #[test]
    fn an_unset_track_reads_as_none() {
        let d = script(&[cmd(OP_END, 0, 0)]);
        let mut j = MaterialJoint::start(0, 0.0);
        j.tick(&d, 1.0).expect("ticks");
        assert_eq!(j.track_value(TRACK_PALETTE_ID), None);
    }
}
