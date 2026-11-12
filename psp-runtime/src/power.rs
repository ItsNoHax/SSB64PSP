//! Suspend and resume: a power callback that parks the audio thread before
//! the PSP sleeps and lets it reopen its output after the wake (RE-591).
//!
//! - The callback runs on its own thread ([`start`]), above the audio
//!   thread's priority, sleeping in `sceKernelSleepThreadCB`.
//! - Suspend (`SUSPENDING`, or the power switch outside a resume): sets
//!   [`suspended`], then waits up to [`PARK_TIMEOUT_US`] for the audio
//!   thread to report [`set_parked`] (between blocks, its SRC channel
//!   released).
//! - Resume (`RESUMING` or `RESUME_COMPLETE`): clears [`suspended`]; the
//!   audio thread reserves its channel again and restarts the ME.

use core::ffi::c_void;
use core::sync::atomic::{AtomicBool, AtomicI32, AtomicU32, Ordering};
use psp::sys::{self, PowerInfo, ThreadAttributes};

/// The callback thread's priority: above the audio thread (0x1C), so a
/// suspend is seen while the audio thread waits on its output.
const PRIORITY: i32 = 0x11;

/// How long the suspend callback waits for the audio thread to park: a
/// block (16 ms) plus the hardware queue, with margin.
const PARK_TIMEOUT_US: u32 = 100_000;

static THREAD: AtomicI32 = AtomicI32::new(-1);
static SUSPENDED: AtomicBool = AtomicBool::new(false);
static PARKED: AtomicBool = AtomicBool::new(false);
static SUSPENDS: AtomicU32 = AtomicU32::new(0);
static RESUMES: AtomicU32 = AtomicU32::new(0);
static LAST_INFO: AtomicU32 = AtomicU32::new(0);

/// A suspend is in progress: the audio thread parks.
pub fn suspended() -> bool {
    SUSPENDED.load(Ordering::Acquire)
}

/// The audio thread is parked (`true`) or running again (`false`).
pub fn set_parked(parked: bool) {
    PARKED.store(parked, Ordering::Release);
}

/// Suspends seen, resumes seen and the last callback's flags.
pub fn counts() -> (u32, u32, u32) {
    (
        SUSPENDS.load(Ordering::Acquire),
        RESUMES.load(Ordering::Acquire),
        LAST_INFO.load(Ordering::Acquire),
    )
}

unsafe extern "C" fn callback(_count: i32, info: i32, _arg: *mut c_void) -> i32 {
    let info = PowerInfo::from_bits_truncate(info as u32);
    LAST_INFO.store(info.bits(), Ordering::Release);
    let resume = info.intersects(PowerInfo::RESUMING | PowerInfo::RESUME_COMPLETE);
    let suspend = !resume && info.intersects(PowerInfo::SUSPENDING | PowerInfo::POWER_SWITCH);
    if suspend && !SUSPENDED.swap(true, Ordering::AcqRel) {
        SUSPENDS.fetch_add(1, Ordering::AcqRel);
        if crate::audio::running() {
            let t0 = unsafe { sys::sceKernelGetSystemTimeLow() };
            while !PARKED.load(Ordering::Acquire)
                && unsafe { sys::sceKernelGetSystemTimeLow() }.wrapping_sub(t0) < PARK_TIMEOUT_US
            {
                unsafe { sys::sceKernelDelayThread(1_000) };
            }
        }
    } else if resume && SUSPENDED.swap(false, Ordering::AcqRel) {
        RESUMES.fetch_add(1, Ordering::AcqRel);
    }
    0
}

unsafe extern "C" fn thread_main(_args: usize, _argp: *mut c_void) -> i32 {
    // SAFETY: a NUL-terminated name and an `extern "C"` callback.
    let cb = unsafe {
        sys::sceKernelCreateCallback(b"ssb_power_cb\0".as_ptr(), callback, core::ptr::null_mut())
    };
    if cb.0 < 0 || unsafe { sys::scePowerRegisterCallback(-1, cb) } < 0 {
        return 0;
    }
    // Runs the callback each time it is notified; never returns.
    unsafe { sys::sceKernelSleepThreadCB() };
    0
}

/// Starts the callback thread once. `false` when it could not start (the
/// game runs on; a suspend then finds the audio thread unparked).
pub fn start() -> bool {
    if THREAD.load(Ordering::Acquire) >= 0 {
        return true;
    }
    // SAFETY: a NUL-terminated name and an `extern "C"` entry.
    let id = unsafe {
        sys::sceKernelCreateThread(
            b"ssb_power\0".as_ptr(),
            thread_main,
            PRIORITY,
            0x1000,
            ThreadAttributes::USER,
            core::ptr::null_mut(),
        )
    };
    if id.0 < 0 || unsafe { sys::sceKernelStartThread(id, 0, core::ptr::null_mut()) } < 0 {
        return false;
    }
    THREAD.store(id.0, Ordering::Release);
    true
}
