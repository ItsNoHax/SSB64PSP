//! Test support: run a closure with the FPU exceptions a PSP traps.
//!
//! Under PSPLink a PSP thread starts with FCR31 `0x00000E00`: invalid
//! operation, divide-by-zero and overflow trap (RE-502). Rust and LLVM
//! assume the IEEE default instead (every exception masked), so a host test
//! never sees what kills the device. glibc's `feenableexcept` arms the same
//! three traps on the calling thread; a trapped operation then ends the test
//! process with SIGFPE, as RE-469's tests do. On a host without it the
//! closure runs unarmed.

/// Runs `f` with the FPU's invalid, divide-by-zero and overflow traps armed
/// on this thread, and disarms them after.
#[cfg(all(
    target_os = "linux",
    target_env = "gnu",
    any(target_arch = "x86_64", target_arch = "x86")
))]
pub(crate) fn with_traps<R>(f: impl FnOnce() -> R) -> R {
    extern "C" {
        fn feenableexcept(excepts: i32) -> i32;
        fn fedisableexcept(excepts: i32) -> i32;
    }
    const FE_INVALID: i32 = 0x01;
    const FE_DIVBYZERO: i32 = 0x04;
    const FE_OVERFLOW: i32 = 0x08;
    let traps = FE_INVALID | FE_DIVBYZERO | FE_OVERFLOW;
    // SAFETY: glibc's own functions; they only change this thread's FPU
    // control words.
    unsafe { feenableexcept(traps) };
    let r = f();
    unsafe { fedisableexcept(traps) };
    r
}

/// Without glibc's `feenableexcept` the closure runs with the traps masked.
#[cfg(not(all(
    target_os = "linux",
    target_env = "gnu",
    any(target_arch = "x86_64", target_arch = "x86")
)))]
pub(crate) fn with_traps<R>(f: impl FnOnce() -> R) -> R {
    f()
}
