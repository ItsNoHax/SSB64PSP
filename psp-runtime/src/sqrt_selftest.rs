//! The `sqrt_selftest` feature: runs `ssb_engine::math::sqrt` over
//! [`sqrt_golden`]'s fixed inputs once at boot and writes
//!
//! `sqrt_selftest ok=<bool> n=<inputs> bad=<specials>/<denormals>/<normals>/<negatives> hash=<hex> want=<hex> us=<run time> fcr31=<before>/<flags after> first_bad=[(input, output) ...]`
//!
//! to stdout and appends it to `selftest.log` beside the executable. `ok`
//! means every result is correctly rounded and its bits hash as on the host
//! (`sqrt_golden::HASH`): the PSP's square root is the host's, and the
//! N64's.
//!
//! FPU exception enables are cleared for the run and restored after: the
//! inputs include NaNs and denormals, and a PSPLink session enables the
//! invalid, divide-by-zero and overflow traps (RE-469). The sticky flags the
//! run raised are reported instead (bits 2-6: inexact, underflow, overflow,
//! divide-by-zero, invalid).

use core::ffi::c_void;
use core::fmt::Write;

use psp::fpu::{fcr31, restore_fcr31, set_fcr31, ENABLES, FLAGS};
use psp::sys;
use ssb_engine::math::sqrt_golden;

/// Runs the self-test and writes its line. Called once, at boot.
pub fn run() {
    struct Buf([u8; 256], usize);
    impl Write for Buf {
        fn write_str(&mut self, t: &str) -> core::fmt::Result {
            let n = t.len().min(self.0.len() - self.1);
            self.0[self.1..self.1 + n].copy_from_slice(&t.as_bytes()[..n]);
            self.1 += n;
            Ok(())
        }
    }
    let before = fcr31();
    // SAFETY: masks every exception for the run; `restore_fcr31` below
    // puts the thread's value back.
    unsafe { set_fcr31(before & !(ENABLES | FLAGS)) };
    // SAFETY: plain syscall.
    let start = unsafe { sys::sceKernelGetSystemTimeWide() };
    let r = sqrt_golden::run(ssb_engine::math::sqrt);
    let end = unsafe { sys::sceKernelGetSystemTimeWide() };
    let flags = fcr31() & FLAGS;
    // SAFETY: `before` is the value this thread had.
    unsafe { restore_fcr31(before) };
    let mut b = Buf([0; 256], 0);
    let _ = writeln!(
        b,
        "sqrt_selftest ok={} n={} bad={}/{}/{}/{} hash={:016x} want={:016x} us={} fcr31={:08x}/{:02x} first_bad={:x?}",
        r.ok(),
        r.count,
        r.bad[0],
        r.bad[1],
        r.bad[2],
        r.bad[3],
        r.hash,
        sqrt_golden::HASH,
        end - start,
        before,
        flags,
        r.first_bad,
    );
    // SAFETY: plain file syscalls on a NUL-terminated name and a live buffer.
    unsafe {
        sys::sceIoWrite(sys::sceKernelStdout(), b.0.as_ptr() as *const c_void, b.1);
        let fd = sys::sceIoOpen(
            b"selftest.log\0".as_ptr(),
            sys::IoOpenFlags::WR_ONLY | sys::IoOpenFlags::CREAT | sys::IoOpenFlags::APPEND,
            0o777,
        );
        if fd.0 >= 0 {
            sys::sceIoWrite(fd, b.0.as_ptr() as *const c_void, b.1);
            sys::sceIoClose(fd);
        }
    }
}
