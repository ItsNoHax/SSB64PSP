//! The synthesizer's DSP half. [`super::synth::Synth`]'s control pass
//! (sequencer and FGM clients, parameter updates, voice allocation,
//! envelope targets, loop bookkeeping, wave addresses) records what the
//! N64 would put in its RSP command list as an [`OpList`]: one entry per
//! `n_a*` microcode command, per 184-sample chunk, in `n_alAudioFrame`'s
//! order. One executor, [`Dsp`], runs a list: it alone owns the workspace
//! ([`Dmem`]), each physical voice's DSP state (ADPCM and resampler
//! history, [`EnvMixState`]) and the aux effect ([`Fx`]), and the control
//! pass never reads any of them. So recording a whole sub-frame and running
//! it afterwards renders the same samples as running each command as it is
//! recorded (the original order, [`OpList::set_limit`] 1).
//!
//! Plain data with fixed capacity: an op names resident data by index
//! (books and loop states in [`AudioData`]'s banks, resident sample bytes
//! by `'static` slice) and streamed sample bytes by offset into the list's
//! own arena, so a list can be copied to another processor's buffer.

use alloc::boxed::Box;

use super::data::{AudioData, Bank, BOOK_MAX};
use super::dsp::{self, Dmem, EnvMixInit, EnvMixState, State16};
use super::prof;
use super::reverb::Fx;
use super::synth::MAX_PVOICES;

/// Ops a list holds before the control pass runs it early. The list runs at
/// each 184-sample sub-frame's end; a sub-frame with 16 voices records at
/// most 61 (the golden scenario; every BGM with FGMs,
/// `stream::tests::head_sizing`).
pub const OPS_CAP: usize = 128;

/// Bytes of streamed ADPCM input a list holds before the control pass runs
/// it early: the busiest sub-frame copies 1,638 in the streamed golden
/// scenario and 2,826 in `head_sizing`'s every-BGM sweep. Running early
/// changes no sample.
pub const ARENA_BYTES: usize = 4096;

/// A resident book or loop state: the bank ([`super::data::WaveRef::bank`])
/// and the index into its table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BankRef {
    pub bank: u8,
    pub index: u16,
}

/// `A_ADPCM`'s input bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Input {
    /// Resident sample bytes.
    Resident(&'static [u8]),
    /// `len` bytes at `at` in the list's arena (a streamed chunk).
    Arena { at: u16, len: u16 },
}

/// One microcode command. Addresses are workspace byte offsets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    /// `A_CLEARBUFF`.
    Clear { addr: u16, nbytes: u16 },
    /// `A_DMEMMOVE`.
    Move { src: u16, dst: u16, nbytes: u16 },
    /// `A_LOADADPCM` + `A_ADPCM` for physical voice `voice`: `nbytes` as
    /// `_decodeChunk` computes it (`tsam << 1`); `book` `None` is the zero
    /// book, `loop_state` `None` the zero state.
    Adpcm {
        voice: u8,
        flags: u8,
        out: u16,
        nbytes: i32,
        input: Input,
        book: Option<BankRef>,
        loop_state: Option<BankRef>,
    },
    /// `A_RESAMPLE` for `voice`.
    Resample {
        voice: u8,
        flags: u8,
        pitch: u16,
        input: u16,
        out: u16,
    },
    /// `A_SETVOL`s (when `init`) + `A_ENVMIXER` for `voice`.
    EnvMix { voice: u8, init: Option<EnvMixInit> },
    /// `A_MIXER`.
    Mix { gain: i16, input: u16, out: u16 },
    /// `n_alFxPull`'s effect (nothing without one).
    Reverb,
    /// `A_INTERLEAVE`.
    Interleave,
    /// `A_SAVEBUFF` of [`dsp::TEMP_0`] to the frame's output at sample
    /// `at`, `len` samples.
    Save { at: u16, len: u16 },
}

/// A workspace address as an op field. Every in-range address fits; one
/// that does not saturates, so the executor's bounds check fails as the
/// direct call's did.
#[inline]
pub(crate) fn addr(a: usize) -> u16 {
    u16::try_from(a).unwrap_or(u16::MAX)
}

/// The recorded commands and the streamed bytes they read. 64-byte
/// aligned for a later shared buffer.
#[repr(C, align(64))]
pub struct OpList {
    ops: [Op; OPS_CAP],
    len: u16,
    /// Ops before the recorder runs the list ([`Self::set_limit`]).
    limit: u16,
    arena_len: u16,
    /// Most ops and arena bytes a run has held (diagnostic).
    pub peak_ops: u16,
    pub peak_arena: u16,
    arena: [u8; ARENA_BYTES],
}

impl OpList {
    /// An empty list on the heap, written in place: never on a stack, and
    /// out of line in [`super::synth::Synth`] so the synthesizer's other
    /// fields keep offsets a MIPS load immediate reaches.
    pub fn boxed() -> Box<Self> {
        let mut b = Box::<Self>::new_uninit();
        let p = b.as_mut_ptr();
        // SAFETY: `p` is valid for writes and aligned; every field is
        // written before `assume_init`.
        unsafe {
            let ops = core::ptr::addr_of_mut!((*p).ops).cast::<Op>();
            for i in 0..OPS_CAP {
                ops.add(i).write(Op::Interleave);
            }
            core::ptr::addr_of_mut!((*p).len).write(0);
            core::ptr::addr_of_mut!((*p).limit).write(OPS_CAP as u16);
            core::ptr::addr_of_mut!((*p).arena_len).write(0);
            core::ptr::addr_of_mut!((*p).peak_ops).write(0);
            core::ptr::addr_of_mut!((*p).peak_arena).write(0);
            core::ptr::addr_of_mut!((*p).arena)
                .cast::<u8>()
                .write_bytes(0, ARENA_BYTES);
            b.assume_init()
        }
    }

    /// The recorded ops, in order.
    pub fn ops(&self) -> &[Op] {
        &self.ops[..self.len as usize]
    }

    /// The arena's used bytes.
    pub fn arena(&self) -> &[u8] {
        &self.arena[..self.arena_len as usize]
    }

    /// Runs the list after at most `n` ops (1: each op as it is recorded,
    /// the unsplit order). Clamped to `1..=OPS_CAP`.
    pub fn set_limit(&mut self, n: usize) {
        self.limit = n.clamp(1, OPS_CAP) as u16;
    }

    /// Whether `ops` more ops and `bytes` more arena bytes fit.
    #[inline]
    pub fn room(&self, ops: usize, bytes: usize) -> bool {
        self.len as usize + ops <= self.limit as usize
            && self.arena_len as usize + bytes <= ARENA_BYTES
    }

    /// Appends `op`; the caller has checked [`Self::room`].
    #[inline]
    pub fn push(&mut self, op: Op) {
        self.ops[self.len as usize] = op;
        self.len += 1;
    }

    /// `n` arena bytes to fill, and their offset; the caller has checked
    /// [`Self::room`].
    #[inline]
    pub fn alloc(&mut self, n: usize) -> (u16, &mut [u8]) {
        let at = self.arena_len as usize;
        self.arena_len = (at + n) as u16;
        (at as u16, &mut self.arena[at..at + n])
    }

    /// Empties the list after a run.
    #[inline]
    pub fn clear(&mut self) {
        self.peak_ops = self.peak_ops.max(self.len);
        self.peak_arena = self.peak_arena.max(self.arena_len);
        self.len = 0;
        self.arena_len = 0;
    }
}

/// Ops a [`FrameList`] holds: a frame's three sub-frames at the busiest
/// sub-frame's 61, with room to spare.
pub const FRAME_OPS: usize = 256;

/// Arena bytes a [`FrameList`] holds: three of the busiest sub-frames'
/// 2,826.
pub const FRAME_ARENA: usize = 12 * 1024;

/// A whole frame's [`OpList`]s, appended in order, for an executor on
/// another processor: one hand-over per frame instead of one per
/// sub-frame. Plain data; 64-byte aligned and padded, the counts first so
/// the used part is two ranges ([`Self::ops_bytes`], [`Self::arena`]).
#[repr(C, align(64))]
pub struct FrameList {
    len: u16,
    arena_len: u16,
    ops: [Op; FRAME_OPS],
    arena: [u8; FRAME_ARENA],
}

impl FrameList {
    /// An empty list on the heap, written in place.
    pub fn boxed() -> Box<Self> {
        let mut b = Box::<Self>::new_uninit();
        let p = b.as_mut_ptr();
        // SAFETY: `p` is valid for writes and aligned; every field is
        // written before `assume_init`.
        unsafe {
            core::ptr::addr_of_mut!((*p).len).write(0);
            core::ptr::addr_of_mut!((*p).arena_len).write(0);
            let ops = core::ptr::addr_of_mut!((*p).ops).cast::<Op>();
            for i in 0..FRAME_OPS {
                ops.add(i).write(Op::Interleave);
            }
            core::ptr::addr_of_mut!((*p).arena)
                .cast::<u8>()
                .write_bytes(0, FRAME_ARENA);
            b.assume_init()
        }
    }

    /// The appended ops, in order.
    pub fn ops(&self) -> &[Op] {
        &self.ops[..self.len as usize]
    }

    /// The arena's used bytes.
    pub fn arena(&self) -> &[u8] {
        &self.arena[..self.arena_len as usize]
    }

    /// Bytes from the list's start through its last used op (the counts
    /// and the ops another processor reads).
    pub fn ops_bytes(&self) -> usize {
        core::mem::offset_of!(Self, ops) + self.len as usize * core::mem::size_of::<Op>()
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn clear(&mut self) {
        self.len = 0;
        self.arena_len = 0;
    }

    /// Appends `list`'s ops and arena bytes, its [`Input::Arena`] offsets
    /// moved to where its bytes land. `false`, and nothing appended, when
    /// they do not fit; an empty `FrameList` always takes a whole
    /// [`OpList`].
    pub fn append(&mut self, list: &OpList) -> bool {
        let (n, a) = (list.len as usize, list.arena_len as usize);
        let (len, base) = (self.len as usize, self.arena_len as usize);
        if len + n > FRAME_OPS || base + a > FRAME_ARENA {
            return false;
        }
        self.arena[base..base + a].copy_from_slice(&list.arena[..a]);
        for (dst, op) in self.ops[len..len + n].iter_mut().zip(list.ops()) {
            *dst = match *op {
                Op::Adpcm {
                    voice,
                    flags,
                    out,
                    nbytes,
                    input: Input::Arena { at, len },
                    book,
                    loop_state,
                } => Op::Adpcm {
                    voice,
                    flags,
                    out,
                    nbytes,
                    input: Input::Arena {
                        at: at + base as u16,
                        len,
                    },
                    book,
                    loop_state,
                },
                op => op,
            };
        }
        self.len = (len + n) as u16;
        self.arena_len = (base + a) as u16;
        true
    }
}

const _: () = assert!(OPS_CAP <= FRAME_OPS && ARENA_BYTES <= FRAME_ARENA);
const _: () = assert!(FRAME_ARENA <= u16::MAX as usize);

/// What runs an [`OpList`]: [`Dsp`] here, a remote processor later.
pub trait Exec {
    /// Runs `ops` in order; [`Op::Save`] writes into `out` (the frame's
    /// interleaved output).
    fn run(&mut self, ops: &OpList, data: &AudioData, out: &mut [i16]);
}

/// One physical voice's DSP state (`N_PVoice`'s `dc_state`, `rs_state`,
/// `em_state`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct VoiceDsp {
    pub adpcm: State16,
    pub resample: State16,
    pub envmix: EnvMixState,
}

/// The executor: the workspace, the voices' DSP state and the aux effect
/// (`n_syn->auxBus->fx`, `None` for `AL_FX_NONE`).
///
/// 64-byte aligned and padded, so another processor that runs it (the
/// PSP's Media Engine) and writes back its data-cache lines never writes
/// a line holding anything else; the effect's heap buffers are padded the
/// same way ([`Fx::new`]).
#[repr(C, align(64))]
pub struct Dsp {
    dmem: Dmem,
    voices: [VoiceDsp; MAX_PVOICES],
    pub fx: Option<Fx>,
    /// Whether runs feed [`prof`] (off on a processor that cannot read
    /// the clock or write the counters).
    prof: bool,
}

static ZERO_BOOK: [i16; BOOK_MAX] = [0; BOOK_MAX];
static ZERO_STATE: State16 = [0; 16];

#[inline]
fn bank(data: &AudioData, b: u8) -> &Bank {
    if b == 0 {
        &data.music
    } else {
        &data.sfx
    }
}

impl Dsp {
    pub fn new(fx: Option<Fx>) -> Self {
        Self {
            dmem: Dmem::default(),
            voices: [VoiceDsp::default(); MAX_PVOICES],
            fx,
            prof: true,
        }
    }

    /// Whether runs feed [`prof`] (on by default). A run on another
    /// processor turns it off for its duration.
    pub fn set_profiled(&mut self, on: bool) {
        self.prof = on;
    }

    /// Runs a whole frame's recorded lists ([`FrameList`]).
    pub fn run_frame(&mut self, list: &FrameList, data: &AudioData, out: &mut [i16]) {
        self.run_ops(list.ops(), list.arena(), data, out);
    }

    /// The workspace (tests).
    pub fn dmem(&self) -> &Dmem {
        &self.dmem
    }

    /// The voices' DSP state (tests).
    pub fn voices(&self) -> &[VoiceDsp; MAX_PVOICES] {
        &self.voices
    }
}

impl Exec for Dsp {
    fn run(&mut self, ops: &OpList, data: &AudioData, out: &mut [i16]) {
        self.run_ops(ops.ops(), &ops.arena, data, out);
    }
}

impl Dsp {
    /// Runs `ops` in order, [`Input::Arena`] reading `arena`.
    pub fn run_ops(&mut self, ops: &[Op], arena: &[u8], data: &AudioData, out: &mut [i16]) {
        let on = self.prof;
        let d = &mut self.dmem;
        // `prof` only when profiled: another processor must not call the
        // clock or write the counters.
        macro_rules! stop {
            ($s:expr, $t:expr) => {
                if on {
                    prof::stop($s, $t)
                }
            };
        }
        for op in ops {
            let t = if on { prof::start() } else { 0 };
            match *op {
                Op::Clear { addr, nbytes } => {
                    d.clear(addr as usize, nbytes as usize);
                    stop!(prof::Stage::Bus, t);
                }
                Op::Move { src, dst, nbytes } => {
                    d.dmem_move(src as usize, dst as usize, nbytes as usize);
                    stop!(prof::Stage::Resample, t);
                }
                Op::Adpcm {
                    voice,
                    flags,
                    out: o,
                    nbytes,
                    input,
                    book,
                    loop_state,
                } => {
                    let input = match input {
                        Input::Resident(s) => s,
                        Input::Arena { at, len } => &arena[at as usize..at as usize + len as usize],
                    };
                    let book = match book {
                        Some(r) => &bank(data, r.bank).books[r.index as usize].book,
                        None => &ZERO_BOOK,
                    };
                    let loop_state = match loop_state {
                        Some(r) => &bank(data, r.bank).loops[r.index as usize].state,
                        None => &ZERO_STATE,
                    };
                    d.adpcm(
                        flags as u32,
                        &mut self.voices[voice as usize].adpcm,
                        loop_state,
                        book,
                        input,
                        o as usize,
                        nbytes as usize,
                    );
                    stop!(prof::Stage::Adpcm, t);
                }
                Op::Resample {
                    voice,
                    flags,
                    pitch,
                    input,
                    out: o,
                } => {
                    d.resample(
                        flags as u32,
                        pitch,
                        &mut self.voices[voice as usize].resample,
                        input as usize,
                        o as usize,
                        &data.resample,
                    );
                    stop!(prof::Stage::Resample, t);
                }
                Op::EnvMix { voice, init } => {
                    d.envmix(init.as_ref(), &mut self.voices[voice as usize].envmix);
                    stop!(prof::Stage::Envmix, t);
                    if on {
                        prof::count(prof::Stage::Pulls, 1);
                    }
                }
                Op::Mix {
                    gain,
                    input,
                    out: o,
                } => {
                    d.mix(gain, input as usize, o as usize);
                    stop!(prof::Stage::Bus, t);
                }
                Op::Reverb => {
                    if let Some(fx) = &mut self.fx {
                        fx.pull(d, &data.resample);
                        stop!(prof::Stage::Reverb, t);
                    }
                }
                Op::Interleave => {
                    d.interleave();
                    stop!(prof::Stage::Bus, t);
                }
                Op::Save { at, len } => {
                    let at = at as usize;
                    d.save(dsp::TEMP_0, &mut out[at..at + len as usize]);
                    stop!(prof::Stage::Bus, t);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Op stays small: the list is copied per frame to the executor.
    #[test]
    fn op_size() {
        assert!(
            core::mem::size_of::<Op>() <= 48,
            "{}",
            core::mem::size_of::<Op>()
        );
        assert_eq!(core::mem::align_of::<OpList>(), 64);
        // Another processor writes the DSP state back by whole lines, and
        // reads the frame list's counts with its ops.
        assert_eq!(core::mem::align_of::<Dsp>(), 64);
        assert_eq!(core::mem::align_of::<FrameList>(), 64);
        assert_eq!(core::mem::offset_of!(FrameList, len), 0);
    }

    /// Appending rebases arena offsets; a list that does not fit is
    /// refused whole.
    #[test]
    fn frame_list_append() {
        let mut a = OpList::boxed();
        a.arena[..4].copy_from_slice(&[1, 2, 3, 4]);
        a.arena_len = 4;
        let adpcm = |at| Op::Adpcm {
            voice: 0,
            flags: 0,
            out: 0,
            nbytes: 4,
            input: Input::Arena { at, len: 4 },
            book: None,
            loop_state: None,
        };
        a.ops[0] = adpcm(0);
        a.len = 1;
        let mut f = FrameList::boxed();
        assert!(f.append(&a));
        assert!(f.append(&a));
        assert_eq!(f.ops(), &[adpcm(0), adpcm(4)]);
        assert_eq!(f.arena(), &[1, 2, 3, 4, 1, 2, 3, 4]);
        a.arena_len = FRAME_ARENA as u16;
        assert!(!f.append(&a));
        assert_eq!(f.ops().len(), 2);
        f.clear();
        assert!(f.is_empty() && f.arena().is_empty());
    }
}
