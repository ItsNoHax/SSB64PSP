//! The FPU control word (FCR31) of the game's threads.
//!
//! - The N64: `osInitialize` and `osCreateThread` set the FCSR to
//!   `FPCSR_FS | FPCSR_EV` (plus round-to-nearest): only an invalid
//!   operation traps; divide-by-zero, overflow, underflow and inexact give
//!   their IEEE results (`refs/ssb-decomp-re/src/libultra/os/initialize.c:37`,
//!   `createthread.c:35`). The original's `x / 0` is an infinity.
//! - A PSP thread starts with FCR31 `0x00000E00`, under PSPLink and at an
//!   XMB-style LoadExec launch alike: invalid, divide-by-zero and overflow
//!   trap (RE-201, RE-502, RE-516). [`init_thread`]
//!   records what each game thread starts with, so a `boot_log` build shows
//!   an XMB launch's value too.
//! - Rust and LLVM assume the IEEE default, every exception masked: `as`
//!   runs `trunc.w.s` before saturating, and a division may be computed
//!   before the branch that guards it (RE-469).
//!
//! Policy: the game leaves the enables as the system sets them, and a trap
//! is a bug to fix in the code: the port reproduces the original's IEEE
//! results without the trapping operation (`ssb_engine::math::div_ieee`,
//! `f32_to_i32_sat`, `div_nonzero`), and host tests arm the same traps
//! (`feenableexcept`). The `fpu_mask` feature masks every exception on the
//! game's threads instead, for comparing a launch with and without traps.

use core::sync::atomic::{AtomicU32, Ordering};

/// A thread whose starting FCR31 [`init_thread`] records.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GameThread {
    Main,
    Audio,
}

/// No value recorded yet.
const UNSET: u32 = u32::MAX;

static MAIN: AtomicU32 = AtomicU32::new(UNSET);
static AUDIO: AtomicU32 = AtomicU32::new(UNSET);

fn slot(thread: GameThread) -> &'static AtomicU32 {
    match thread {
        GameThread::Main => &MAIN,
        GameThread::Audio => &AUDIO,
    }
}

/// FCR31's cause bits (`E V Z O U I`, bits 12-17): a write with a cause bit
/// and its enable both set traps at the `ctc1` itself (RE-502).
pub const CAUSE: u32 = 0x3F << 12;
/// FCR31's enable bits (`V Z O U I`, bits 7-11).
pub const ENABLES: u32 = 0x1F << 7;

/// FCR31 with every exception masked and no cause pending: what the
/// `fpu_mask` feature writes.
pub const fn masked(fcr31: u32) -> u32 {
    fcr31 & !(CAUSE | ENABLES)
}

/// On entry to each thread that runs game code: records the FCR31 the
/// system started it with ([`launch_fcr31`]) and, with `fpu_mask`, masks
/// every exception.
pub fn init_thread(thread: GameThread) {
    let v = read();
    slot(thread).store(v, Ordering::Release);
    #[cfg(feature = "fpu_mask")]
    write(masked(v));
}

/// The FCR31 `thread` started with, once it has called [`init_thread`].
pub fn launch_fcr31(thread: GameThread) -> Option<u32> {
    let v = slot(thread).load(Ordering::Acquire);
    (v != UNSET).then_some(v)
}

#[cfg(target_arch = "mips")]
fn read() -> u32 {
    let v: u32;
    // SAFETY: `cfc1` of FCR31 only reads this thread's FPU control word.
    // A fixed register: the allocator may otherwise pick `$at`.
    unsafe { core::arch::asm!("cfc1 $8, $31", out("$8") v, options(nomem, nostack)) };
    v
}

#[cfg(all(target_arch = "mips", feature = "fpu_mask"))]
fn write(v: u32) {
    // SAFETY: `ctc1` of FCR31 only writes this thread's FPU control word;
    // `masked` leaves no cause bit set, so the write itself cannot trap.
    unsafe { core::arch::asm!("ctc1 $8, $31", in("$8") v, options(nomem, nostack)) };
}

/// Host builds have no FCR31.
#[cfg(not(target_arch = "mips"))]
fn read() -> u32 {
    0
}

#[cfg(all(not(target_arch = "mips"), feature = "fpu_mask"))]
fn write(_v: u32) {}
