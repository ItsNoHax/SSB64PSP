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

#[cfg(feature = "fpu_mask")]
use psp::fpu::{masked, set_fcr31};

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

/// On entry to each thread that runs game code: records the FCR31 the
/// system started it with ([`launch_fcr31`]) and, with `fpu_mask`, masks
/// every exception.
pub fn init_thread(thread: GameThread) {
    let v = psp::fpu::fcr31();
    slot(thread).store(v, Ordering::Release);
    // SAFETY: `masked` leaves no cause bit set, so the write cannot trap,
    // and the game's code is compiled for the masked IEEE default.
    #[cfg(feature = "fpu_mask")]
    unsafe {
        set_fcr31(masked(v))
    };
}

/// The FCR31 `thread` started with, once it has called [`init_thread`].
pub fn launch_fcr31(thread: GameThread) -> Option<u32> {
    let v = slot(thread).load(Ordering::Acquire);
    (v != UNSET).then_some(v)
}
