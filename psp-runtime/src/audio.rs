//! The PSP audio backend ([D-049](../../docs/decisions/D-049.md)): one
//! [`AudioSystem`] behind a kernel-semaphore lock, rendered by a dedicated
//! thread into the 32 kHz stereo SRC channel.
//!
//! The N64's `syAudioThreadMain` (`src/sys/audio.c:990-1140`) runs one tic
//! per vertical retrace: it picks the frame length from the AI queue
//! (552 or 368 samples), builds the frame, hands the previous one to the AI
//! (`osAiSetNextBuffer`) and runs its client handlers. On the PSP the output
//! clock is the master instead:
//!
//! - The audio thread renders whole N64 frames into a FIFO until it holds at
//!   least one hardware block ([`BLOCK`] = 512 frames, 16 ms), copies that
//!   block into one of [`BUFFERS`] rotating 64-byte-aligned buffers, writes
//!   it back from the data cache and submits it with
//!   `sceAudioSRCOutputBlocking`, which sleeps until the previous block has
//!   played.
//! - The frame length comes from a model of the N64's AI ([`VirtualAi`]),
//!   drained at the N64 rate once per NTSC retrace, so the 552/368 pattern
//!   and the tic rate (59.94 per second, the FGM engine's and client
//!   handlers' clock) match the N64. The real PSP queue cannot stand in for
//!   `AI_LEN`: it always holds a whole block, which would pick 368 every
//!   time and run the tics 46 % fast.
//! - The game calls [`AudioApi`] synchronously ([`GameAudio`]); the audio
//!   thread takes the same lock around each frame (DESIGN decision 3). It
//!   runs at [`PRIORITY`], above the game's main thread (32): the PSP's
//!   scheduler is strictly by priority, so a thread at or below the game's
//!   would only run when the game blocks, and CPU-bound frames would starve
//!   it (R4 §2, pitfall 3). It sleeps in the blocking output call, so it
//!   takes the CPU only for its render bursts.
//!
//! Latency: at a submit the FIFO keeps under one N64 frame (< 552 frames),
//! one block is queued and one plays: at most about 1,575 frames, 49 ms.
//! The N64 holds one frame being built plus one or two in the AI, 35-50 ms.
//!
//! Nothing on the audio thread allocates, does I/O or logs. Its statistics
//! are single-writer atomics ([`stats`]); the `profile` feature reports
//! them, and the `audio_dump` feature copies every submitted block into a
//! ring the game thread writes to `audio.raw` ([`dump_flush`]).
//!
//! Fallback: if the SRC channel cannot be reserved, [`start`] fails and the
//! game runs without an audio system installed (every sound call is then a
//! no-op). The 32 kHz stream would otherwise need our own resampler to
//! 44.1 kHz for a normal channel, and the SRC channel is available on every
//! firmware the port targets.

use alloc::boxed::Box;
use core::ffi::c_void;
use core::sync::atomic::{AtomicBool, AtomicI32, AtomicPtr, AtomicU32, Ordering};

use psp::sys::{self, SceUid};
use ssb_engine::audio::{
    AudioApi, AudioError, AudioSystem, FgmHandle, RawLock, SharedAudio, FRAME_SAMPLES_MAX,
    N64_OUTPUT_RATE,
};

use crate::assets::{AlignedBuf, LoadError};
use crate::profile::{self, Span};

/// Sample frames per hardware block: 16 ms at 32 kHz, a multiple of 64 (so a
/// normal channel would accept it too) and within the SRC channel's
/// 17..=4111 (R4 §1).
pub const BLOCK: usize = 512;

/// Output buffers in rotation. The block passed to a blocking output keeps
/// playing after the call returns, so it is not touched until the next
/// call has returned; the third is margin for hardware that holds one more
/// (PPSSPP copies at the call and would hide such a bug, R4 pitfall 2).
pub const BUFFERS: usize = 3;

/// The audio thread's priority (lower is more urgent). Above the game's
/// main thread (32, `thread.rs`), so it preempts CPU-bound frames the
/// moment a block finishes; at or below the system's own threads' range
/// (0x10-0x18; sf64-psp's output thread runs at 0x18, oot-PSP's at 0x1E).
pub const PRIORITY: i32 = 0x1C;

/// The audio thread's stack: the synth's per-frame state lives in the
/// [`AudioSystem`], so the stack holds only call frames. The profile line
/// reports the unused part ([`stack_free`]): PPSSPP never detects an
/// overflow (RE-469).
pub const STACK_BYTES: i32 = 64 * 1024;

/// `PSP_AUDIO_VOLUME_MAX`.
const VOLUME: i32 = 0x8000;

/// The FIFO between N64 frames and hardware blocks: it is refilled while it
/// holds under one block, by at most one N64 frame.
const FIFO_FRAMES: usize = BLOCK + FRAME_SAMPLES_MAX;

/// The kernel semaphore the [`SharedAudio`] lock is built on.
pub struct SemaLock(SceUid);

impl SemaLock {
    /// A binary semaphore, initially free. `Err` with the kernel's error.
    pub fn new() -> Result<Self, i32> {
        // rust-psp declares `sceKernelCreateSema` (5 arguments) without its
        // `i5` ABI mapper, so a plain call leaves the option pointer in `$t0`
        // unset (PPSSPP logs `deadbeef`, "invalid options parameter"). The
        // kernel takes arguments 5-8 in `$t0-$t3` (MIPS EABI); rust-psp's
        // global `i5` trampoline moves the fifth O32 stack argument there.
        extern "C" {
            fn i5(
                a: u32,
                b: u32,
                c: u32,
                d: u32,
                e: u32,
                f: extern "C" fn(u32, u32, u32, u32, u32) -> u32,
            ) -> u32;
            fn __sceKernelCreateSema_stub(
                name: u32,
                attr: u32,
                init: u32,
                max: u32,
                option: u32,
            ) -> u32;
        }
        // SAFETY: the stub is the kernel import rust-psp links; the name is
        // NUL-terminated and the option pointer is null. The transmute only
        // changes the declared ABI tag (both are `extern "C"`), from an
        // `unsafe` to a safe fn pointer as `i5` expects.
        let id = unsafe {
            let stub: extern "C" fn(u32, u32, u32, u32, u32) -> u32 = core::mem::transmute(
                __sceKernelCreateSema_stub as unsafe extern "C" fn(u32, u32, u32, u32, u32) -> u32,
            );
            i5(b"ssb_audio_lock\0".as_ptr() as u32, 0, 1, 1, 0, stub) as i32
        };
        if id < 0 {
            Err(id)
        } else {
            Ok(Self(SceUid(id)))
        }
    }
}

impl RawLock for SemaLock {
    fn lock(&self) {
        // SAFETY: a semaphore this lock created; no timeout.
        unsafe { sys::sceKernelWaitSema(self.0, 1, core::ptr::null_mut()) };
    }
    fn unlock(&self) {
        // SAFETY: as above.
        unsafe { sys::sceKernelSignalSema(self.0, 1) };
    }
}

/// The game's [`AudioApi`]: the shared system, with every call's cost (lock
/// wait included) added to [`Span::Sound`] and counted in
/// [`Span::SoundCalls`] when the `profile` feature is on.
pub struct GameAudio {
    shared: SharedAudio<SemaLock>,
}

macro_rules! timed_api {
    ($(fn $name:ident(&self $(, $arg:ident: $ty:ty)*) $(-> $ret:ty)?;)*) => {
        impl AudioApi for GameAudio {
            $(
                fn $name(&self $(, $arg: $ty)*) $(-> $ret)? {
                    let t = profile::start();
                    let r = self.shared.$name($($arg),*);
                    profile::stop(Span::Sound, t);
                    profile::count(Span::SoundCalls, 1);
                    r
                }
            )*
        }
    };
}

timed_api! {
    fn play_fgm(&self, id: u16) -> Option<FgmHandle>;
    fn play_fgm_balance(&self, id: u16, balance: u8) -> Option<FgmHandle>;
    fn stop_fgm(&self, handle: FgmHandle);
    fn stop_all_fgm(&self);
    fn pause_fgm(&self);
    fn resume_fgm(&self);
    fn fgm_alive(&self, handle: FgmHandle) -> bool;
    fn fgm_count(&self) -> u16;
    fn set_fgm_count(&self, count: u16);
    fn set_fgm_master_volume(&self, volume: u8);
    fn set_fgm_volume(&self, handle: FgmHandle, volume: u8);
    fn set_fgm_pan(&self, handle: FgmHandle, pan: u8);
    fn set_fgm_fx(&self, handle: FgmHandle, fx: u8);
    fn play_bgm(&self, player: u32, bgm: u32) -> i32;
    fn stop_bgm(&self, player: u32);
    fn stop_bgm_all(&self);
    fn set_bgm_volume(&self, player: u32, volume: u32);
    fn set_bgm_volume_fade(&self, player: u32, volume: u32, time: u32);
    fn set_bgm_reverb(&self, player: u32, reverb: u32);
    fn set_bgm_priority(&self, player: u32, priority: u8);
    fn bgm_playing(&self, player: u32) -> bool;
    fn sy_play_fgm(&self, fgm: u32) -> i32;
    fn sy_stop_fgm(&self, slot: i32);
    fn set_quality(&self, quality: u32);
    fn set_fx_type(&self, fx_type: i32);
}

/// The N64 AI as `syAudioThreadMain` sees it: a playing DMA and one queued
/// (`osAiSetNextBuffer`), drained at [`N64_OUTPUT_RATE`] once per NTSC
/// retrace. `AI_LEN` reads the playing one's remaining length.
struct VirtualAi {
    /// Sample frames left in the playing buffer (`AI_LEN >> 2`).
    cur: u32,
    /// The queued buffer's frames; 0 when none.
    next: u32,
    /// The drain's remainder, in 1/[`VI_DEN`] frames.
    frac: u32,
    /// The frame built last tic, submitted at this tic's retrace.
    pending: u32,
    /// `ai_len`: what the last retrace read.
    ai_len: u32,
}

/// Frames drained per retrace, as a fraction: 32006 Hz at 60000/1001 Hz.
const VI_NUM: u32 = N64_OUTPUT_RATE as u32 * 1001;
const VI_DEN: u32 = 60_000;

impl VirtualAi {
    const fn new() -> Self {
        Self {
            cur: 0,
            next: 0,
            frac: 0,
            pending: 0,
            ai_len: 0,
        }
    }

    /// The rest of one loop of `syAudioThreadMain` after the frame of `n`
    /// samples is built: wait for the retrace and read `AI_LEN`
    /// (audio.c:1064-1065), then queue the previous tic's frame (:1080).
    fn tic(&mut self, n: u32) {
        self.frac += VI_NUM;
        let mut drain = self.frac / VI_DEN;
        self.frac %= VI_DEN;
        let take = drain.min(self.cur);
        self.cur -= take;
        drain -= take;
        if self.cur == 0 {
            self.cur = self.next.saturating_sub(drain);
            self.next = 0;
        }
        self.ai_len = self.cur;
        let frame = core::mem::replace(&mut self.pending, n);
        if frame == 0 {
            return;
        }
        if self.cur == 0 {
            self.cur = frame;
        } else if self.next == 0 {
            self.next = frame;
        }
        // Otherwise the AI's queue is full and `osAiSetNextBuffer` drops
        // the frame; the N64's frame lengths never let that happen.
    }
}

/// A 64-byte-aligned, cache-line-padded array (RE-360: a DMA'd buffer must
/// not share a line with anything the CPU dirties).
#[repr(C, align(64))]
struct Lines<const N: usize>([i16; N]);

/// Everything only the audio thread touches.
struct ThreadState {
    out: [Lines<{ BLOCK * 2 }>; BUFFERS],
    fifo: Lines<{ FIFO_FRAMES * 2 }>,
    fifo_len: usize,
    next_out: usize,
    ai: VirtualAi,
    /// [`EPOCH`] as last seen; a change restarts the windowed maxima.
    epoch: u32,
    max_us: u32,
    rest_min: u32,
    latency_max: u32,
}

impl ThreadState {
    /// Allocated on the heap by [`start`] (game thread, at boot): 10 KB the
    /// image does not carry when audio is off.
    fn boxed() -> Box<Self> {
        Box::new(Self {
            out: [
                Lines([0; BLOCK * 2]),
                Lines([0; BLOCK * 2]),
                Lines([0; BLOCK * 2]),
            ],
            fifo: Lines([0; FIFO_FRAMES * 2]),
            fifo_len: 0,
            next_out: 0,
            ai: VirtualAi::new(),
            epoch: 0,
            max_us: 0,
            rest_min: u32::MAX,
            latency_max: 0,
        })
    }
}

/// The audio thread's state, set by [`start`] before the thread starts and
/// then touched only by that thread.
static THREAD_STATE: AtomicPtr<ThreadState> = AtomicPtr::new(core::ptr::null_mut());

/// The running system, set by [`start`] before the thread starts.
static mut SHARED: Option<&'static GameAudio> = None;
static THREAD: AtomicI32 = AtomicI32::new(-1);
static STOP: AtomicBool = AtomicBool::new(false);

// Statistics: each written only by the audio thread (plain stores, no
// read-modify-write across threads, R4 §1), except EPOCH, written only by
// the game thread.
static BLOCKS: AtomicU32 = AtomicU32::new(0);
static TICS: AtomicU32 = AtomicU32::new(0);
static BUSY_US: AtomicU32 = AtomicU32::new(0);
static UNDERRUNS: AtomicU32 = AtomicU32::new(0);
static OUTPUT_ERRORS: AtomicU32 = AtomicU32::new(0);
static MAX_US: AtomicU32 = AtomicU32::new(0);
static REST_MIN: AtomicU32 = AtomicU32::new(u32::MAX);
static LATENCY_MAX: AtomicU32 = AtomicU32::new(0);
static EPOCH: AtomicU32 = AtomicU32::new(0);

/// A snapshot of the audio thread's counters. The totals only grow
/// (wrapping); the maxima and minimum cover the window since the last
/// [`new_window`].
#[derive(Clone, Copy)]
pub struct Stats {
    /// Hardware blocks submitted.
    pub blocks: u32,
    /// N64 audio frames (tics) rendered.
    pub tics: u32,
    /// Microseconds spent filling the FIFO (lock waits included).
    pub busy_us: u32,
    /// Blocks submitted after the hardware had run dry.
    pub underruns: u32,
    /// Failed output calls.
    pub errors: u32,
    /// The window's longest FIFO fill, in microseconds.
    pub max_us: u32,
    /// The window's smallest hardware queue at a submit, in sample frames.
    pub rest_min: u32,
    /// The window's largest render-to-output latency at a submit, in
    /// sample frames (hardware queue + FIFO + the block submitted).
    pub latency_max: u32,
}

impl Stats {
    pub const ZERO: Stats = Stats {
        blocks: 0,
        tics: 0,
        busy_us: 0,
        underruns: 0,
        errors: 0,
        max_us: 0,
        rest_min: u32::MAX,
        latency_max: 0,
    };
}

/// Why [`start`] did not start audio.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartError {
    System(AudioError),
    Sema(i32),
    /// `sceAudioSRCChReserve` failed: run without audio.
    Channel(i32),
    Thread(i32),
}

/// Reads the pack's audio section (`ssb_rom::pack::audio_range`, from the
/// header in `head`) from `path` (NUL-terminated, as `assets::c_path`
/// returns) into one 64-byte-aligned buffer that is never freed: the audio
/// thread reads it for the life of the process. `Ok(None)` when the pack
/// has no audio section (a pack built before it existed).
pub fn load_section(path: &'static str, head: &[u8]) -> Result<Option<&'static [u8]>, LoadError> {
    let Some((offset, len)) = ssb_rom::pack::audio_range(head) else {
        return Ok(None);
    };
    if len == 0 {
        return Ok(None);
    }
    // SAFETY: path is NUL-terminated.
    let fd = unsafe { sys::sceIoOpen(path.as_ptr(), sys::IoOpenFlags::RD_ONLY, 0o777) };
    if fd.0 < 0 {
        return Err(LoadError::NotFound);
    }
    let Some(buf) = AlignedBuf::cache_lines(len) else {
        unsafe { sys::sceIoClose(fd) };
        return Err(LoadError::OutOfMemory);
    };
    let read = unsafe {
        sys::sceIoLseek(fd, offset as i64, sys::IoWhence::Set);
        let n = sys::sceIoRead(fd, buf.as_mut_ptr() as *mut c_void, len as u32);
        sys::sceIoClose(fd);
        n
    };
    if read < 0 || read as usize != len {
        return Err(LoadError::ShortRead);
    }
    // SAFETY: the buffer is leaked below, so the slice lives forever.
    let bytes = unsafe { core::slice::from_raw_parts(buf.as_ptr(), len) };
    core::mem::forget(buf);
    Ok(Some(bytes))
}

/// Builds the audio system over `section`, reserves the SRC channel and
/// starts the audio thread. Returns the [`AudioApi`] to install with
/// `ssb_game::sound::install`. Called once, on the game thread, at boot.
pub fn start(section: &'static [u8]) -> Result<&'static GameAudio, StartError> {
    let sys_ = AudioSystem::new(section).map_err(StartError::System)?;
    let lock = SemaLock::new().map_err(StartError::Sema)?;
    // SAFETY: plain syscall; 2 = stereo, the only SRC format.
    let ch =
        unsafe { sys::sceAudioSRCChReserve(BLOCK as i32, sys::AudioOutputFrequency::Khz32, 2) };
    if ch < 0 {
        return Err(StartError::Channel(ch));
    }
    let api: &'static GameAudio = Box::leak(Box::new(GameAudio {
        shared: SharedAudio::new(lock, sys_),
    }));
    // SAFETY: written before the thread that reads it starts.
    unsafe { SHARED = Some(api) };
    if THREAD_STATE.load(Ordering::Acquire).is_null() {
        THREAD_STATE.store(Box::into_raw(ThreadState::boxed()), Ordering::Release);
    }
    STOP.store(false, Ordering::Release);
    // SAFETY: a NUL-terminated name and a `extern "C"` entry. VFPU: the
    // synth may reach rust-psp's VFPU libm (R4 §1).
    let id = unsafe {
        sys::sceKernelCreateThread(
            b"ssb_audio\0".as_ptr(),
            thread_main,
            PRIORITY,
            STACK_BYTES,
            sys::ThreadAttributes::USER | sys::ThreadAttributes::VFPU,
            core::ptr::null_mut(),
        )
    };
    if id.0 < 0 {
        unsafe { sys::sceAudioSRCChRelease() };
        return Err(StartError::Thread(id.0));
    }
    THREAD.store(id.0, Ordering::Release);
    let r = unsafe { sys::sceKernelStartThread(id, 0, core::ptr::null_mut()) };
    if r < 0 {
        unsafe {
            sys::sceKernelDeleteThread(id);
            sys::sceAudioSRCChRelease();
        }
        THREAD.store(-1, Ordering::Release);
        return Err(StartError::Thread(r));
    }
    Ok(api)
}

/// Stops the audio thread after its current block and releases the SRC
/// channel. The installed [`GameAudio`] keeps answering calls; nothing
/// renders. Safe to call when audio never started.
pub fn stop() {
    let id = THREAD.swap(-1, Ordering::AcqRel);
    if id < 0 {
        return;
    }
    STOP.store(true, Ordering::Release);
    let mut timeout: u32 = 200_000;
    unsafe {
        sys::sceKernelWaitThreadEnd(SceUid(id), &mut timeout);
        sys::sceKernelDeleteThread(SceUid(id));
        sys::sceAudioSRCChRelease();
    }
}

/// Whether the audio thread is running.
pub fn running() -> bool {
    THREAD.load(Ordering::Acquire) >= 0
}

/// The audio thread's untouched stack bytes, or 0 when it is not running.
pub fn stack_free() -> usize {
    let id = THREAD.load(Ordering::Acquire);
    if id < 0 {
        return 0;
    }
    unsafe { sys::sceKernelGetThreadStackFreeSize(SceUid(id)) }.max(0) as usize
}

/// The audio thread's counters.
pub fn stats() -> Stats {
    Stats {
        blocks: BLOCKS.load(Ordering::Acquire),
        tics: TICS.load(Ordering::Acquire),
        busy_us: BUSY_US.load(Ordering::Acquire),
        underruns: UNDERRUNS.load(Ordering::Acquire),
        errors: OUTPUT_ERRORS.load(Ordering::Acquire),
        max_us: MAX_US.load(Ordering::Acquire),
        rest_min: REST_MIN.load(Ordering::Acquire),
        latency_max: LATENCY_MAX.load(Ordering::Acquire),
    }
}

/// Starts a new window for [`Stats`]' maxima (game thread).
pub fn new_window() {
    EPOCH.store(
        EPOCH.load(Ordering::Relaxed).wrapping_add(1),
        Ordering::Release,
    );
}

fn now() -> u32 {
    unsafe { sys::sceKernelGetSystemTimeLow() }
}

/// Adds `n` to a counter only this thread writes.
#[inline]
fn bump(counter: &AtomicU32, n: u32) {
    counter.store(
        counter.load(Ordering::Relaxed).wrapping_add(n),
        Ordering::Release,
    );
}

unsafe extern "C" fn thread_main(_args: usize, _argp: *mut c_void) -> i32 {
    // SAFETY: set by `start` before this thread began.
    let Some(api) = (unsafe { *core::ptr::addr_of!(SHARED) }) else {
        return 0;
    };
    let st = THREAD_STATE.load(Ordering::Acquire);
    if st.is_null() {
        return 0;
    }
    // SAFETY: leaked by `start`; only this thread touches it.
    let st = unsafe { &mut *st };
    let mut first = true;
    while !STOP.load(Ordering::Acquire) {
        let t0 = now();
        while st.fifo_len < BLOCK {
            let ai_len = st.ai.ai_len;
            let off = st.fifo_len * 2;
            let fifo = &mut st.fifo.0[off..];
            let n = api.shared.with(|s| {
                let n = s.frame_samples(ai_len).min(FRAME_SAMPLES_MAX);
                s.render_frame(fifo, n);
                n
            });
            st.fifo_len += n;
            st.ai.tic(n as u32);
            bump(&TICS, 1);
        }
        let us = now().wrapping_sub(t0);
        bump(&BUSY_US, us);

        let epoch = EPOCH.load(Ordering::Acquire);
        if epoch != st.epoch {
            st.epoch = epoch;
            st.max_us = 0;
            st.rest_min = u32::MAX;
            st.latency_max = 0;
        }
        if us > st.max_us {
            st.max_us = us;
            MAX_US.store(us, Ordering::Release);
        }

        let out = &mut st.out[st.next_out].0;
        out.copy_from_slice(&st.fifo.0[..BLOCK * 2]);
        st.fifo.0.copy_within(BLOCK * 2..st.fifo_len * 2, 0);
        st.fifo_len -= BLOCK;
        #[cfg(feature = "audio_dump")]
        dump::push(out);
        unsafe {
            sys::sceKernelDcacheWritebackRange(out.as_ptr() as *const c_void, (BLOCK * 4) as u32)
        };

        let rest = unsafe { sys::sceAudioOutput2GetRestSample() }.max(0) as u32;
        if rest == 0 && !first {
            bump(&UNDERRUNS, 1);
        }
        if !first && rest < st.rest_min {
            st.rest_min = rest;
            REST_MIN.store(rest, Ordering::Release);
        }
        let latency = rest + (st.fifo_len + BLOCK) as u32;
        if latency > st.latency_max {
            st.latency_max = latency;
            LATENCY_MAX.store(latency, Ordering::Release);
        }
        first = false;

        // SAFETY: a block-sized, written-back buffer that stays untouched
        // until two more blocks have been submitted.
        if unsafe { sys::sceAudioSRCOutputBlocking(VOLUME, out.as_mut_ptr() as *mut c_void) } < 0 {
            bump(&OUTPUT_ERRORS, 1);
        }
        st.next_out = (st.next_out + 1) % BUFFERS;
        bump(&BLOCKS, 1);
    }
    0
}

/// The `audio_dump` feature: every submitted block, copied into a ring the
/// game thread drains to `audio.raw` beside the executable (s16le stereo,
/// 32000 Hz). PPSSPPHeadless cannot record audio (R4 §6), and this is the
/// exact stream, before any emulator or firmware resampling.
#[cfg(feature = "audio_dump")]
mod dump {
    use super::*;
    use core::cell::UnsafeCell;

    /// Ring capacity in sample frames: 8 s (1 MB), a whole number of
    /// blocks so a block never wraps.
    const FRAMES: usize = 32_000 / BLOCK * BLOCK * 8;

    struct Ring(UnsafeCell<[i16; FRAMES * 2]>);
    // SAFETY: the audio thread writes only frames past HEAD, the game thread
    // reads only frames before it.
    unsafe impl Sync for Ring {}
    static RING: Ring = Ring(UnsafeCell::new([0; FRAMES * 2]));

    /// Frames written (audio thread) and frames drained (game thread),
    /// both wrapping counters.
    static HEAD: AtomicU32 = AtomicU32::new(0);
    static TAIL: AtomicU32 = AtomicU32::new(0);
    static DROPPED: AtomicU32 = AtomicU32::new(0);
    static mut OPENED: bool = false;

    pub(super) fn push(block: &[i16]) {
        let head = HEAD.load(Ordering::Relaxed);
        let tail = TAIL.load(Ordering::Acquire);
        if head.wrapping_sub(tail) as usize + BLOCK > FRAMES {
            bump(&DROPPED, 1);
            return;
        }
        let at = head as usize % FRAMES * 2;
        // SAFETY: frames [head, head + BLOCK) are not readable by the game
        // until HEAD moves past them.
        unsafe { (&mut *RING.0.get())[at..at + BLOCK * 2].copy_from_slice(block) };
        HEAD.store(head.wrapping_add(BLOCK as u32), Ordering::Release);
    }

    pub fn flush(force: bool) {
        let head = HEAD.load(Ordering::Acquire);
        let mut tail = TAIL.load(Ordering::Relaxed);
        let pending = head.wrapping_sub(tail) as usize;
        if pending == 0 || (!force && pending < FRAMES / 4) {
            return;
        }
        // SAFETY: game thread only.
        let flags = if unsafe { OPENED } {
            sys::IoOpenFlags::WR_ONLY | sys::IoOpenFlags::APPEND
        } else {
            sys::IoOpenFlags::WR_ONLY | sys::IoOpenFlags::CREAT | sys::IoOpenFlags::TRUNC
        };
        let fd = unsafe { sys::sceIoOpen(b"audio.raw\0".as_ptr(), flags, 0o777) };
        unsafe { OPENED = true };
        while tail != head {
            let at = tail as usize % FRAMES;
            let n = (head.wrapping_sub(tail) as usize).min(FRAMES - at);
            if fd.0 >= 0 {
                // SAFETY: frames [tail, head) are complete (HEAD Acquire).
                let ring = unsafe { &*RING.0.get() };
                let bytes = &ring[at * 2..(at + n) * 2];
                unsafe { sys::sceIoWrite(fd, bytes.as_ptr() as *const c_void, bytes.len() * 2) };
            }
            tail = tail.wrapping_add(n as u32);
            TAIL.store(tail, Ordering::Release);
        }
        if fd.0 >= 0 {
            unsafe { sys::sceIoClose(fd) };
        }
    }

    pub fn dropped() -> u32 {
        DROPPED.load(Ordering::Acquire)
    }
}

/// Writes the `audio_dump` ring to `audio.raw` (game thread, at a safe
/// point): when a quarter of the ring is pending, or everything when
/// `force` (before exiting). A no-op without the feature.
pub fn dump_flush(force: bool) {
    #[cfg(feature = "audio_dump")]
    dump::flush(force);
    #[cfg(not(feature = "audio_dump"))]
    let _ = force;
}

/// Blocks the `audio_dump` ring dropped because the game thread fell
/// behind (0 without the feature).
pub fn dump_dropped() -> u32 {
    #[cfg(feature = "audio_dump")]
    return dump::dropped();
    #[cfg(not(feature = "audio_dump"))]
    0
}
