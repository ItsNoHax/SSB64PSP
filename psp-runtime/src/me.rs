//! The Media Engine audio executor (PLAN "Audio cost", RE-479).
//!
//! - Split: the SC audio thread runs the control pass under the audio lock
//!   and records the frame's op lists into a [`FrameList`]; the ME runs the
//!   whole list on the system's [`Dsp`] (integer reverb: the ME has no
//!   VFPU) into [`MeExec`]'s PCM; one mailbox round trip per frame.
//! - Bring-up: [`bring_up`] loads the game's own kernel module
//!   (`psp-kcall/`, built and embedded by `build.rs`) from a file it writes
//!   next to the game, and its `module_start` runs [`k_me_reset`] in
//!   kernel mode: the reset handler copied to the ME reset vector, then a
//!   reset pulse.
//! - Coherence: the ME starts each command with an empty D-cache and ends
//!   it with an index writeback-invalidate of the whole cache; everything it
//!   writes ([`Dsp`], the reverb's buffers, [`Pcm`]) is 64-byte aligned and
//!   padded. The SC writes the frame list back before a post, invalidates
//!   the PCM before reading it, and writes back and invalidates its whole
//!   D-cache after touching the [`Dsp`] ([`MeExec::sc_touched`]), so it
//!   never holds a dirty line of ME-written memory.
//! - Mailbox: one heap line, uncached on both sides, `sync` between the
//!   arguments and `SEQ`. The ME polls with a 256-nop delay between reads
//!   (a tight poll slows the SC's memory, RE-479).
//! - Fallback: an emulator, the module refused, the ME not up within 5 ms, or
//!   no ACK within 20 ms: the SC runs the same list (same PCM). A missed
//!   ACK resets the ME (the watchdog: a sleep/wake loses the ME's program;
//!   untested); three failed bring-ups in a row end ME use.
//! - Code the ME runs makes no system call: [`Dsp::set_profiled`] is off
//!   while the ME owns the DSP, and nothing on the run path allocates.

use alloc::boxed::Box;
use core::arch::{asm, global_asm};
use core::ffi::c_void;
use core::ptr::{read_volatile, write_volatile};
use core::sync::atomic::{AtomicU32, AtomicUsize, Ordering};

use psp::sys;
use ssb_engine::audio::data::AudioData;
use ssb_engine::audio::exec::{Dsp, Exec, FrameList, OpList};
use ssb_engine::audio::FRAME_SAMPLES_MAX;

/// The kernel module (`psp-kcall/`), written to [`KCALL_PATH`] for the
/// loader. Byte-aligned: a 64-byte aligned object in `.rodata` shifts that
/// segment's file offset against the first one's, and the game's PRX then
/// loads with its module info misread.
static KCALL_PRX: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/kcall.prx"));

const UNCACHED: usize = 0x4000_0000;

// Mailbox words.
const SEQ: usize = 0;
const CMD: usize = 1;
const A_LIST: usize = 2;
const A_DSP: usize = 3;
const A_DATA: usize = 4;
const A_PCM: usize = 5;
const A_LEN: usize = 6;
const ACK: usize = 7;
/// 0x10 once the ME reached its command loop.
const STAGE: usize = 9;

const C_FRAME: u32 = 1;
const STAGE_UP: u32 = 0x10;

/// Nops between the ME's mailbox reads (RE-479: at least 256).
const IDLE_NOPS: u32 = 256;
const UP_TIMEOUT_US: u32 = 5_000;
const ACK_TIMEOUT_US: u32 = 20_000;
/// Frames between bring-up attempts after a failed one.
const RETRY_FRAMES: u32 = 120;
/// Failed bring-ups in a row that end ME use.
const MAX_FAILS: u32 = 3;

/// [`stats`]' `state`.
pub const STATE_NONE: u32 = 0;
pub const STATE_UP: u32 = 1;
pub const STATE_DOWN: u32 = 2;
pub const STATE_EMULATOR: u32 = 3;
/// The kernel module did not load or start ([`Stats::kmod`] says why).
pub const STATE_NO_KMOD: u32 = 4;
pub const STATE_GAVE_UP: u32 = 5;

/// The mailbox line's cached address (0: none yet).
static MBOX: AtomicUsize = AtomicUsize::new(0);
static STATE: AtomicU32 = AtomicU32::new(STATE_NONE);
static SEQ_N: AtomicU32 = AtomicU32::new(0);
static KMOD: AtomicU32 = AtomicU32::new(0);
static FRAMES: AtomicU32 = AtomicU32::new(0);
static POLLS: AtomicU32 = AtomicU32::new(0);
static WAIT_US: AtomicU32 = AtomicU32::new(0);
static FALLBACKS: AtomicU32 = AtomicU32::new(0);
static RESETS: AtomicU32 = AtomicU32::new(0);

/// The executor's counters (`profile`).
#[derive(Clone, Copy)]
pub struct Stats {
    pub state: u32,
    /// Lists the ME ran.
    pub frames: u32,
    /// The SC's wake-ups polling for an ACK (the ME has no readable
    /// clock, RE-479: its time is the SC's wait).
    pub polls: u32,
    /// The SC's wall time waiting for them (the audio thread sleeps).
    pub wait_us: u32,
    /// Lists the SC ran instead.
    pub fallbacks: u32,
    /// ME resets after the first.
    pub resets: u32,
    /// The last bring-up's kernel-module error: 0, or the failed load's
    /// or start's result.
    pub kmod: u32,
}

impl Stats {
    pub const ZERO: Stats = Stats {
        state: STATE_NONE,
        frames: 0,
        polls: 0,
        wait_us: 0,
        fallbacks: 0,
        resets: 0,
        kmod: 0,
    };
}

pub fn stats() -> Stats {
    Stats {
        state: STATE.load(Ordering::Acquire),
        frames: FRAMES.load(Ordering::Acquire),
        polls: POLLS.load(Ordering::Acquire),
        wait_us: WAIT_US.load(Ordering::Acquire),
        fallbacks: FALLBACKS.load(Ordering::Acquire),
        resets: RESETS.load(Ordering::Acquire),
        kmod: KMOD.load(Ordering::Acquire),
    }
}

fn bump(c: &AtomicU32, n: u32) {
    c.store(c.load(Ordering::Relaxed).wrapping_add(n), Ordering::Release);
}

#[repr(C, align(64))]
struct Line([u32; 16]);

#[inline(always)]
fn mbox() -> *mut u32 {
    (MBOX.load(Ordering::Relaxed) | UNCACHED) as *mut u32
}
#[inline(always)]
fn mget(i: usize) -> u32 {
    // SAFETY: the mailbox line, uncached; set before any reader runs.
    unsafe { read_volatile(mbox().add(i)) }
}
#[inline(always)]
fn mset(i: usize, v: u32) {
    // SAFETY: as `mget`.
    unsafe { write_volatile(mbox().add(i), v) }
}
#[inline(always)]
fn sync() {
    unsafe { asm!("sync", options(nostack, preserves_flags)) }
}

fn now() -> u32 {
    unsafe { sys::sceKernelGetSystemTimeLow() }
}

/// PPSSPP answers its `kemulator:` device; a PSP has none.
fn emulator() -> bool {
    let mut out = 0u32;
    // SAFETY: a NUL-terminated device name and a live 4-byte output.
    let r = unsafe {
        sys::sceIoDevctl(
            b"kemulator:\0".as_ptr(),
            3, // EMULATOR_DEVCTL__IS_EMULATOR
            core::ptr::null_mut(),
            0,
            &mut out as *mut u32 as *mut c_void,
            4,
        )
    };
    r >= 0
}

/// The kernel module's file, next to the game: the system refuses a
/// kernel module from a user-mode memory buffer (0x80020149, RE-479) but
/// loads one from a file.
const KCALL_PATH: &[u8] = b"ssb64_kcall.prx\0";

/// Writes [`KCALL_PRX`] to [`KCALL_PATH`] unless the file already holds
/// it: `false` when it cannot be written.
fn kcall_file() -> bool {
    let mut same = false;
    // SAFETY: a NUL-terminated path; reads into a buffer of its length.
    unsafe {
        let fd = sys::sceIoOpen(KCALL_PATH.as_ptr(), sys::IoOpenFlags::RD_ONLY, 0o777);
        if fd.0 >= 0 {
            let mut old = alloc::vec![0u8; KCALL_PRX.len() + 1];
            let n = sys::sceIoRead(fd, old.as_mut_ptr() as *mut c_void, old.len() as u32);
            same = n == KCALL_PRX.len() as i32 && old[..KCALL_PRX.len()] == *KCALL_PRX;
            sys::sceIoClose(fd);
        }
    }
    if same {
        return true;
    }
    // SAFETY: as above; writes the module's bytes.
    unsafe {
        let fd = sys::sceIoOpen(
            KCALL_PATH.as_ptr(),
            sys::IoOpenFlags::WR_ONLY | sys::IoOpenFlags::CREAT | sys::IoOpenFlags::TRUNC,
            0o777,
        );
        if fd.0 < 0 {
            return false;
        }
        let n = sys::sceIoWrite(fd, KCALL_PRX.as_ptr() as *const c_void, KCALL_PRX.len());
        sys::sceIoClose(fd);
        n == KCALL_PRX.len() as i32
    }
}

/// Starts the ME's program ([`k_me_reset`]) in kernel mode: `Err` when
/// that is not possible here. Loads the kernel module from its file
/// ([`kcall_file`]) and starts it with [`k_me_reset`]'s address; it
/// unloads itself.
fn bring_up() -> Result<(), u32> {
    if !kcall_file() {
        KMOD.store(u32::MAX, Ordering::Release);
        return Err(STATE_NO_KMOD);
    }
    // SAFETY: a NUL-terminated path; no options.
    let id = unsafe { sys::sceKernelLoadModule(KCALL_PATH.as_ptr(), 0, core::ptr::null_mut()) };
    if id.0 < 0 {
        KMOD.store(id.0 as u32, Ordering::Release);
        return Err(STATE_NO_KMOD);
    }
    // The SC's caches as the user-mode system calls leave them; the
    // kernel-mode part writes the D-cache back again after its copy.
    // SAFETY: plain cache maintenance.
    unsafe {
        sys::sceKernelDcacheWritebackInvalidateAll();
        sys::sceKernelIcacheInvalidateAll();
    }
    let func = k_me_reset as *const () as usize as u32;
    let mut status = 0;
    // SAFETY: a module just loaded; its `module_start` reads the 4 bytes
    // of `func` (copied by the system) and calls it in kernel mode.
    let r = unsafe {
        sys::sceKernelStartModule(
            id,
            4,
            &func as *const u32 as *mut c_void,
            &mut status,
            core::ptr::null_mut(),
        )
    };
    if r < 0 {
        KMOD.store(r as u32, Ordering::Release);
        // SAFETY: the module loaded above, not started.
        unsafe { sys::sceKernelUnloadModule(id) };
        return Err(STATE_NO_KMOD);
    }
    KMOD.store(0, Ordering::Release);
    Ok(())
}

/// Kernel mode: copies the reset handler to the ME reset vector, writes
/// back and invalidates the SC's D-cache, and pulses the ME's reset.
/// Makes no system call: it runs on the kernel module's start thread.
extern "C" fn k_me_reset() -> i32 {
    // SAFETY: kernel mode; the handler is position-independent words.
    unsafe {
        let s = core::ptr::addr_of!(ssb_me_handler_start) as *const u32;
        let e = core::ptr::addr_of!(ssb_me_handler_end) as usize;
        let dst = 0xbfc0_0000 as *mut u32;
        for i in 0..(e - s as usize) / 4 {
            write_volatile(dst.add(i), read_volatile(s.add(i)));
        }
        dc_wbinv_all();
        write_volatile(0xbc10_004c as *mut u32, 0x14);
        write_volatile(0xbc10_004c as *mut u32, 0);
        sync();
    }
    0
}

/// Brings the ME up (again): `true` once its loop answers.
fn start_me() -> bool {
    match STATE.load(Ordering::Acquire) {
        STATE_UP => return true,
        STATE_EMULATOR | STATE_NO_KMOD | STATE_GAVE_UP => return false,
        STATE_NONE if emulator() => {
            STATE.store(STATE_EMULATOR, Ordering::Release);
            return false;
        }
        _ => {}
    }
    if MBOX.load(Ordering::Acquire) == 0 {
        let line = Box::leak(Box::new(Line([0; 16])));
        // SAFETY: the line is never touched cached again.
        unsafe { psp::dcache::writeback_invalidate_range(line as *const Line as *const u8, 64) };
        MBOX.store(line as *const Line as usize, Ordering::Release);
    }
    for i in 0..16 {
        mset(i, 0);
    }
    sync();
    SEQ_N.store(0, Ordering::Release);
    if STATE.load(Ordering::Acquire) != STATE_NONE {
        bump(&RESETS, 1);
    }
    if let Err(state) = bring_up() {
        STATE.store(state, Ordering::Release);
        return false;
    }
    let t0 = now();
    while mget(STAGE) != STAGE_UP {
        if now().wrapping_sub(t0) > UP_TIMEOUT_US {
            STATE.store(STATE_DOWN, Ordering::Release);
            return false;
        }
        unsafe { sys::sceKernelDelayThread(100) };
    }
    STATE.store(STATE_UP, Ordering::Release);
    true
}

/// One frame's PCM, written by the ME.
#[repr(C, align(64))]
struct Pcm([i16; FRAME_SAMPLES_MAX * 2]);

/// The ME executor: an [`Exec`] that records the frame and runs it on the
/// ME at [`MeExec::finish`].
pub struct MeExec {
    list: Box<FrameList>,
    pcm: Box<Pcm>,
    dsp: *mut Dsp,
    data: *const AudioData,
    /// The frame's output length, in samples (both channels).
    len: usize,
    /// The SC's wait for the last list, in microseconds.
    last_us: u32,
    /// Failed bring-ups in a row.
    fails: u32,
    /// Frames until the next bring-up attempt.
    retry_in: u32,
    /// The SC touched the DSP state since the last post.
    sc_dirty: bool,
}

impl MeExec {
    /// Brings the ME up: `None` when it is not available (the caller keeps
    /// the SC executor). `dsp`'s runs stop feeding `prof`.
    pub fn start(dsp: &mut Dsp) -> Option<Box<Self>> {
        if !start_me() {
            return None;
        }
        dsp.set_profiled(false);
        Some(Box::new(Self {
            list: FrameList::boxed(),
            pcm: Box::new(Pcm([0; FRAME_SAMPLES_MAX * 2])),
            dsp: core::ptr::null_mut(),
            data: core::ptr::null(),
            len: 0,
            last_us: 0,
            fails: 0,
            retry_in: 0,
            sc_dirty: true,
        }))
    }

    /// The ME stays unusable (the caller goes back to the SC executor).
    pub fn gave_up(&self) -> bool {
        STATE.load(Ordering::Acquire) == STATE_GAVE_UP
    }

    /// The SC wrote the DSP state (an effect change): written back before
    /// the next post.
    pub fn sc_touched(&mut self) {
        self.sc_dirty = true;
    }

    /// Starts a frame on `dsp` and `data` (`AudioSystem::dsp_mut`,
    /// `AudioSystem::data`, under the audio lock).
    pub fn begin(&mut self, dsp: *mut Dsp, data: *const AudioData) {
        self.dsp = dsp;
        self.data = data;
        self.len = 0;
        self.list.clear();
    }

    /// Runs what the frame recorded and copies its PCM to `out` (outside
    /// the audio lock is fine: nothing else touches the DSP state).
    pub fn finish(&mut self, out: &mut [i16]) {
        self.flush();
        let len = self.len;
        // SAFETY: the PCM is written back (ME) or written by this thread;
        // its lines hold nothing else.
        unsafe {
            psp::dcache::writeback_invalidate_range(self.pcm.0.as_ptr() as *const u8, len * 2)
        };
        ssb_engine::audio::mem::copy(&mut out[..len], &self.pcm.0[..len]);
    }

    /// Runs the recorded list on the ME, or on the SC when the ME fails.
    fn flush(&mut self) {
        if self.list.is_empty() {
            return;
        }
        let mut up = STATE.load(Ordering::Acquire) == STATE_UP;
        if !up && STATE.load(Ordering::Acquire) == STATE_DOWN {
            if self.retry_in == 0 {
                up = start_me();
                if up {
                    self.fails = 0;
                } else {
                    self.fails += 1;
                    self.retry_in = RETRY_FRAMES;
                    if self.fails >= MAX_FAILS {
                        STATE.store(STATE_GAVE_UP, Ordering::Release);
                    }
                }
                self.sc_dirty = true;
            } else {
                self.retry_in -= 1;
            }
        }
        if up && self.post_and_wait() {
            self.list.clear();
            return;
        }
        // The SC executor: a clean cache first, written back after, so the
        // ME (back up later) sees what it wrote.
        unsafe { sys::sceKernelDcacheWritebackInvalidateAll() };
        // SAFETY: `begin` set both under the audio lock; the ME is reset or
        // idle, so this thread is the DSP state's only user.
        unsafe { (*self.dsp).run_frame(&self.list, &*self.data, &mut self.pcm.0[..self.len]) };
        unsafe { sys::sceKernelDcacheWritebackInvalidateAll() };
        bump(&FALLBACKS, 1);
        self.list.clear();
    }

    fn post_and_wait(&mut self) -> bool {
        if core::mem::take(&mut self.sc_dirty) {
            unsafe { sys::sceKernelDcacheWritebackInvalidateAll() };
        }
        let list: &FrameList = &self.list;
        // SAFETY: the list's used ranges, read by the ME from memory.
        unsafe {
            psp::dcache::writeback_range(list as *const FrameList as *const u8, list.ops_bytes());
            psp::dcache::writeback_range(list.arena().as_ptr(), list.arena().len());
        }
        let s = SEQ_N.load(Ordering::Relaxed).wrapping_add(1);
        SEQ_N.store(s, Ordering::Relaxed);
        mset(CMD, C_FRAME);
        mset(A_LIST, list as *const FrameList as u32);
        mset(A_DSP, self.dsp as u32);
        mset(A_DATA, self.data as u32);
        mset(A_PCM, self.pcm.0.as_mut_ptr() as u32);
        mset(A_LEN, self.len as u32);
        sync();
        mset(SEQ, s);
        sync();
        let t0 = now();
        // Sleep through most of the ME's expected time, then poll.
        let mut d = self.last_us * 3 / 4;
        loop {
            unsafe { sys::sceKernelDelayThread(d.max(50)) };
            bump(&POLLS, 1);
            if mget(ACK) == s {
                break;
            }
            if now().wrapping_sub(t0) > ACK_TIMEOUT_US {
                bump(&WAIT_US, now().wrapping_sub(t0));
                // The watchdog: stop the ME now, bring it up again later.
                STATE.store(STATE_DOWN, Ordering::Release);
                let _ = bring_up();
                self.retry_in = RETRY_FRAMES;
                return false;
            }
            d = 100;
        }
        self.last_us = now().wrapping_sub(t0).min(ACK_TIMEOUT_US);
        bump(&WAIT_US, self.last_us);
        bump(&FRAMES, 1);
        true
    }
}

impl Exec for MeExec {
    fn run(&mut self, ops: &OpList, _: &AudioData, out: &mut [i16]) {
        self.len = out.len();
        if !self.list.append(ops) {
            self.flush();
            let ok = self.list.append(ops);
            debug_assert!(ok, "an empty frame list takes a whole op list");
        }
    }
}

// --- The ME side: no system calls below. ------------------------------------

macro_rules! cache_range {
    ($name:ident, $word:literal) => {
        #[inline(always)]
        unsafe fn $name(start: usize, len: usize) {
            let mut a = start & !63;
            let end = start + len;
            while a < end {
                asm!(concat!(".word ", $word), in("$4") a, options(nostack, preserves_flags));
                a += 64;
            }
            sync();
        }
    };
}
cache_range!(dc_inv, "0xbc990000"); // cache 0x19: hit invalidate D
cache_range!(ic_inv, "0xbc880000"); // cache 0x08: hit invalidate I

/// Index writeback-invalidate of the whole D-cache (`meLibDcacheWritebackInvalidateAll`).
#[inline(always)]
unsafe fn dc_wbinv_all() {
    sync();
    let mut a = 0usize;
    while a < 8192 {
        asm!(".word 0xbc940000", ".word 0xbc940000", in("$4") a, options(nostack, preserves_flags));
        a += 64;
    }
    sync();
}

#[inline(never)]
fn delay(n: u32) {
    for _ in 0..n {
        unsafe { asm!("nop", options(nostack, preserves_flags)) };
    }
}

// The reset handler, copied to the ME reset vector (me-core.h's
// `meLibHandler`): bus clocks, unlock, minimal VME config, status (CU0 |
// CU1), the stack by the witness word, then `ssb_me_entry` through the
// kernel cached alias.
global_asm!(
    ".pushsection .text.ssb_me_handler,\"ax\",@progbits",
    ".globl ssb_me_handler_start",
    ".globl ssb_me_handler_end",
    ".set push",
    ".set noreorder",
    ".set noat",
    "ssb_me_handler_start:",
    "lui $26, 0xbc10",
    "li $27, 0x0f",
    "sw $27, 0x50($26)",
    "lw $27, 0x40($26)",
    "ori $27, $27, 2",
    "sw $27, 0x40($26)",
    "li $27, -1",
    "sw $27, 0x04($26)",
    "sync",
    "lui $26, 0xbc00",
    "sw $27, 0x30($26)",
    "sw $27, 0x34($26)",
    "sw $27, 0x38($26)",
    "sw $27, 0x3c($26)",
    "sw $27, 0x40($26)",
    "sw $27, 0x44($26)",
    "sync",
    "sw $27, 0x00($26)",
    "sw $27, 0x04($26)",
    "sw $27, 0x08($26)",
    "sw $27, 0x0c($26)",
    "sw $27, 0x10($26)",
    "sw $27, 0x14($26)",
    "sw $27, 0x18($26)",
    "sw $27, 0x1c($26)",
    "sw $27, 0x20($26)",
    "sw $27, 0x24($26)",
    "sw $27, 0x28($26)",
    "sw $27, 0x2c($26)",
    "sync",
    "lui $26, 0xbcc0",
    "sw $27, 0x00($26)",
    "li $27, 1",
    "sw $27, 0x10($26)",
    "1:",
    "lw $27, 0x10($26)",
    "sync",
    "bnez $27, 1b",
    "nop",
    "sw $0, 0x70($26)",
    "li $27, -1",
    "sw $27, 0x20($26)",
    "li $27, 1",
    "sw $27, 0x30($26)",
    "li $27, 2",
    "sw $27, 0x40($26)",
    "sync",
    "lui $26, 0x3000",
    "mtc0 $26, $12",
    "sync",
    "lui $26, 0x279c",
    "ori $26, $26, 0x637c",
    "lui $27, 0x8830",
    "lw $27, 0x18($27)",
    "beq $26, $27, 2f",
    "nop",
    "lui $29, 0x8020",
    "b 3f",
    "nop",
    "2:",
    "lui $29, 0x8040",
    "3:",
    "lui $26, %hi(ssb_me_entry)",
    "addiu $26, $26, %lo(ssb_me_entry)",
    "lui $27, 0x8000",
    "or $26, $26, $27",
    ".word 0xbf480000",
    "sync",
    "jr $26",
    "nop",
    "ssb_me_handler_end:",
    ".set pop",
    ".popsection",
);

extern "C" {
    static ssb_me_handler_start: u8;
    static ssb_me_handler_end: u8;
}

/// The ME's entry (`meLibOnPreProcess`) and command loop.
#[no_mangle]
extern "C" fn ssb_me_entry() -> ! {
    // SAFETY: the ME's own start-up; the mailbox address was written back
    // before the reset (`k_me_reset`).
    unsafe {
        dc_inv(0x8830_0000, 0x90000);
        ic_inv(0x8830_0000, 0x90000);
        write_volatile(0xbc20_0000 as *mut u32, 511 << 16 | 511);
        write_volatile(0xbc20_0004 as *mut u32, 511 << 16 | 511);
        write_volatile(0xbc20_0008 as *mut u32, 511 << 16 | 511);
        // FCR31: round to nearest, every exception masked (an ME trap has
        // no handler; the SC's enables make the same code trap there).
        asm!("ctc1 $0, $31", options(nostack));
        sync();
        let mb = (read_volatile((core::ptr::addr_of!(MBOX) as usize | UNCACHED) as *const usize)
            | UNCACHED) as *mut u32;
        let get = |i: usize| read_volatile(mb.add(i));
        let set = |i: usize, v: u32| write_volatile(mb.add(i), v);
        dc_wbinv_all();
        set(STAGE, STAGE_UP);
        sync();
        let mut seen = get(SEQ);
        loop {
            let s = get(SEQ);
            if s == seen {
                delay(IDLE_NOPS);
                continue;
            }
            seen = s;
            if get(CMD) == C_FRAME {
                let list = &*(get(A_LIST) as *const FrameList);
                let dsp = &mut *(get(A_DSP) as *mut Dsp);
                let data = &*(get(A_DATA) as *const AudioData);
                let pcm =
                    core::slice::from_raw_parts_mut(get(A_PCM) as *mut i16, get(A_LEN) as usize);
                dsp.run_frame(list, data, pcm);
            }
            dc_wbinv_all();
            set(ACK, s);
            sync();
        }
    }
}
