//! The `trig_selftest` feature: runs the original's trigonometry
//! (`ssb_engine::math::{sinf, cosf, sy_tan, sy_arc_*}`) and plain `f32`
//! `div.s`/`mul.s`/`add.s` over [`trig_golden`]'s fixed inputs once at boot,
//! times one call of each sine and cosine path, and writes
//!
//! `trig_selftest ok=<bool> bad=[names] arith_ok=<bool> us=<run time> ns=sinf_small/sinf/cosf/sin_cos fcr31=<before>/<flags after>`
//!
//! then one `trig_hash` line per function, to stdout and appended to
//! `selftest.log` beside the executable. `ok` means every function's output
//! hash is the one the ROM's own machine code gives (`trig_golden::HASHES`):
//! the PSP computes the N64's bits. `arith_ok` means the FPU's single
//! precision divide, multiply and add give the host's IEEE bits
//! (`trig_golden::HASHES_ARITH`). `ns` is the net cost of one call (100,000
//! calls, less an empty loop's time): `sinf_small` takes `|x| < 1.5`, the
//! others sweep `[-8, 8)`.
//!
//! FPU exception enables are cleared for the run and restored after, as in
//! `sqrt_selftest`: the inputs include infinities and `0 / 0`.

use core::ffi::c_void;
use core::fmt::Write;
use core::hint::black_box;

use psp::fpu::{fcr31, restore_fcr31, set_fcr31, ENABLES, FLAGS};
use psp::sys;
use ssb_engine::math::{self, trig_golden};

const BENCH_CALLS: u32 = 100_000;

/// Runs the self-test and writes its lines. Called once, at boot.
pub fn run() {
    let before = fcr31();
    // SAFETY: masks every exception for the run; `restore_fcr31` below
    // puts the thread's value back.
    unsafe { set_fcr31(before & !(ENABLES | FLAGS)) };
    let start = now();
    let hashes = trig_golden::run();
    let arith = trig_golden::run_arith();
    let end = now();
    let empty = bench(|x| x);
    let ns = [
        bench_ns(|x| math::sinf(x * (1.0 / 8.0)), empty),
        bench_ns(math::sinf, empty),
        bench_ns(math::cosf, empty),
        bench_ns(
            |x| {
                let (s, c) = math::sin_cos(x);
                s + c
            },
            empty,
        ),
    ];
    let flags = fcr31() & FLAGS;
    // SAFETY: `before` is the value this thread had.
    unsafe { restore_fcr31(before) };

    let ok = hashes == trig_golden::HASHES;
    let arith_ok = arith == trig_golden::HASHES_ARITH;
    let mut b = Buf([0; 1024], 0);
    let _ = write!(b, "trig_selftest ok={ok} bad=[");
    for (i, name) in trig_golden::NAMES.iter().enumerate() {
        if hashes[i] != trig_golden::HASHES[i] {
            let _ = write!(b, "{name} ");
        }
    }
    let _ = writeln!(
        b,
        "] arith_ok={arith_ok} us={} ns={}/{}/{}/{} fcr31={before:08x}/{flags:02x}",
        end - start,
        ns[0],
        ns[1],
        ns[2],
        ns[3],
    );
    for (i, name) in trig_golden::NAMES.iter().enumerate() {
        let _ = writeln!(
            b,
            "trig_hash {name} got={:016x} want={:016x}",
            hashes[i],
            trig_golden::HASHES[i]
        );
    }
    for (i, name) in ["div", "mul", "add"].iter().enumerate() {
        let _ = writeln!(
            b,
            "trig_hash {name} got={:016x} want={:016x}",
            arith[i],
            trig_golden::HASHES_ARITH[i]
        );
    }
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

struct Buf([u8; 1024], usize);

impl Write for Buf {
    fn write_str(&mut self, t: &str) -> core::fmt::Result {
        let n = t.len().min(self.0.len() - self.1);
        self.0[self.1..self.1 + n].copy_from_slice(&t.as_bytes()[..n]);
        self.1 += n;
        Ok(())
    }
}

fn now() -> u64 {
    // SAFETY: plain syscall.
    unsafe { sys::sceKernelGetSystemTimeWide() as u64 }
}

/// Microseconds for [`BENCH_CALLS`] calls of `f` over `[-8, 8)`.
fn bench(f: impl Fn(f32) -> f32) -> u64 {
    let start = now();
    let mut acc = 0.0f32;
    for i in 0..BENCH_CALLS {
        let x = black_box(i as f32 * (16.0 / BENCH_CALLS as f32) - 8.0);
        acc += black_box(f(x));
    }
    black_box(acc);
    now() - start
}

/// Nanoseconds per call of `f`, less the empty loop's `empty` microseconds.
fn bench_ns(f: impl Fn(f32) -> f32, empty: u64) -> u64 {
    bench(f).saturating_sub(empty) * 1000 / u64::from(BENCH_CALLS)
}
