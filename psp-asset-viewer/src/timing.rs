//! High-resolution timing backed by the PSP's hardware tick counter.
//!
//! `sceKernelGetSystemTimeLow` counts microseconds directly, which is exactly
//! the resolution the frame budget is measured in. Using vblank counts instead
//! would quantise every measurement to 16.67 ms and make the profiler useless.

use psp::sys;

use ssb_engine::timing::Clock;

pub struct PspClock;

impl Clock for PspClock {
    fn now_us(&self) -> u64 {
        // Returns the low 32 bits of the system clock, in microseconds. It
        // wraps roughly every 71 minutes; `FixedClock` uses saturating deltas,
        // so a wrap costs one stalled frame rather than a huge time jump.
        unsafe { sys::sceKernelGetSystemTimeLow() as u64 }
    }
}

/// Measures a span of microseconds.
pub struct Stopwatch {
    start: u32,
}

impl Stopwatch {
    pub fn start() -> Stopwatch {
        Stopwatch {
            start: unsafe { sys::sceKernelGetSystemTimeLow() },
        }
    }

    /// Microseconds since [`Stopwatch::start`], correct across a single wrap.
    pub fn elapsed_us(&self) -> u32 {
        let now = unsafe { sys::sceKernelGetSystemTimeLow() };
        now.wrapping_sub(self.start)
    }
}
