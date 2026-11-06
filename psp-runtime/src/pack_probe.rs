//! The portable pack build (`ssb_pack::build`) on the PSP, timed per phase
//! (RE-588: the ROM-free release builds the pack on the device). Replaces
//! the game: [`run`] never returns.
//!
//! - Reads [`ROM`] into one allocation, builds the pack with the default
//!   [`ssb_pack::PackOptions`] (the shipped pack, as `romtool pack`) and
//!   writes it to [`PACK`] through a [`SINK_BYTES`] `.bss` buffer.
//! - Appends to [`LOG`], one open/write/close per line so a crash keeps
//!   every line before it: each build line, and at each phase the time,
//!   the sink's and the log's own time, free memory, the small-block heap
//!   and the main thread's stack low-water mark.
//! - Times are `sceKernelGetSystemTimeLow` microseconds, exact for runs
//!   under 71 minutes.
//! - A thread above the main thread's priority keeps the PSP awake
//!   (RE-481) and samples free memory every [`TICK_US`].
//! - A panic, an allocation failure (`memory::fatal`'s hook) or a build
//!   error ends in a log line; then the thread idles so PSPLink stays usable.
//! - FPU exceptions keep the system's enables (`fpu`): a trap is a PSPLink
//!   exception with the last phase in the log.

use alloc::string::String;
use alloc::vec::Vec;
use core::cell::UnsafeCell;
use core::ffi::c_void;
use core::fmt::{self, Write};
use core::sync::atomic::{AtomicU32, AtomicUsize, Ordering};

use psp::sys;

use crate::fpu::{self, GameThread};
use crate::{heap, memory, thread, timing};

/// The probe's directory (created if missing).
const DIR: &[u8] = b"ms0:/PSP/GAME/ssb64probe\0";
/// The ROM the build reads (big-endian `.z64`).
const ROM: &[u8] = b"ms0:/PSP/GAME/ssb64probe/rom.z64\0";
/// The pack the build writes (replaced each run).
const PACK: &[u8] = b"ms0:/PSP/GAME/ssb64probe/ssb64.pak\0";
/// The log, appended to: each run starts with a `start` line.
const LOG: &[u8] = b"ms0:/PSP/GAME/ssb64probe/packlog.txt\0";
/// The pack sink's buffer: one `sceIoWrite` per this many bytes.
const SINK_BYTES: usize = 256 * 1024;
/// One `sceIoRead` per this many ROM bytes.
const READ_BYTES: usize = 1024 * 1024;
/// The keep-awake thread's period.
const TICK_US: u32 = 1_000_000;
/// Above the main thread's 32, so a CPU-bound build cannot starve it.
const TICK_PRIORITY: i32 = 0x18;

/// `sceKernelGetSystemTimeLow` at [`run`]'s start.
static START: AtomicU32 = AtomicU32::new(0);
/// Time since start at the last phase.
static LAST_PHASE: AtomicU32 = AtomicU32::new(0);
/// Microseconds in the pack sink's `sceIoWrite`s, bytes and calls.
static SINK_US: AtomicU32 = AtomicU32::new(0);
static SINK_WRITTEN: AtomicUsize = AtomicUsize::new(0);
static SINK_CALLS: AtomicU32 = AtomicU32::new(0);
/// Microseconds spent writing [`LOG`].
static LOG_US: AtomicU32 = AtomicU32::new(0);
/// The last phase's name, for the fatal hook.
static PHASE: PhaseName = PhaseName(UnsafeCell::new([0; 32]), AtomicUsize::new(0));

/// The last phase's name: written by the main thread only, read by it too
/// (the fatal hook runs on the failing thread, the build's).
struct PhaseName(UnsafeCell<[u8; 32]>, AtomicUsize);

// SAFETY: only the main thread touches the bytes (see above).
unsafe impl Sync for PhaseName {}

impl PhaseName {
    fn set(&self, name: &str) {
        let n = name.len().min(32);
        // SAFETY: main thread only; nothing borrows the bytes across calls.
        unsafe { (&mut *self.0.get())[..n].copy_from_slice(&name.as_bytes()[..n]) };
        self.1.store(n, Ordering::Relaxed);
    }

    fn get(&self) -> &str {
        let n = self.1.load(Ordering::Relaxed);
        // SAFETY: as `set`; the bytes are a prefix of a `str` cut at 32
        // bytes, so fall back to `?` if the cut split a character.
        let bytes = unsafe { &(&*self.0.get())[..n] };
        core::str::from_utf8(bytes).unwrap_or("?")
    }
}

fn now() -> u32 {
    // SAFETY: a clock read with no preconditions.
    unsafe { sys::sceKernelGetSystemTimeLow() }
}

/// Microseconds since [`run`]'s start.
fn elapsed() -> u32 {
    now().wrapping_sub(START.load(Ordering::Relaxed))
}

fn tick() {
    // SAFETY: no preconditions; resets the auto-sleep timer (RE-481).
    unsafe { sys::scePowerTick(sys::PowerTick::All) };
}

/// Builds the pack from [`ROM`], logging to [`LOG`]; then idles.
pub fn run() -> ! {
    START.store(now(), Ordering::Relaxed);
    // SAFETY: NUL-terminated path; an existing directory is an error to
    // ignore.
    unsafe { sys::sceIoMkdir(DIR.as_ptr(), 0o777) };
    spawn_ticker();
    memory::on_fatal(on_fatal);
    let clock = timing::full_speed();
    // SAFETY: queries with no preconditions.
    let (cpu, bus) = unsafe {
        (
            sys::scePowerGetCpuClockFrequency(),
            sys::scePowerGetBusClockFrequency(),
        )
    };
    log(format_args!(
        "start fcr31={:#010x} clock_rc={clock} cpu={cpu} bus={bus} free={} max={} stack_free={}",
        fpu::launch_fcr31(GameThread::Main).unwrap_or(u32::MAX),
        memory::free(),
        memory::max_block(),
        thread::stack_free_bytes(),
    ));
    match psp::catch_unwind(build) {
        Ok(()) => {}
        Err(payload) => {
            let msg = payload
                .downcast_ref::<String>()
                .map(String::as_str)
                .or_else(|| payload.downcast_ref::<&str>().copied())
                .unwrap_or("(no message)");
            log(format_args!(
                "panic t={} phase={} {msg}",
                elapsed(),
                PHASE.get()
            ));
        }
    }
    log(format_args!("idle t={}", elapsed()));
    loop {
        tick();
        // SAFETY: plain sleep.
        unsafe { sys::sceKernelDelayThread(TICK_US) };
    }
}

/// Reads the ROM, runs the build and logs its end.
fn build() {
    let t0 = elapsed();
    let rom = match read_rom() {
        Ok(rom) => rom,
        Err(e) => {
            log(format_args!("error t={} rom: {e}", elapsed()));
            return;
        }
    };
    let read_us = elapsed().wrapping_sub(t0).max(1);
    log(format_args!(
        "rom bytes={} us={read_us} kbps={} free={} max={}",
        rom.len(),
        rom.len() as u64 * 1000 / read_us as u64,
        memory::free(),
        memory::max_block(),
    ));
    let probe = Probe;
    probe.phase_line("rom");

    let mut sink = match FileSink::create() {
        Ok(sink) => sink,
        Err(e) => {
            log(format_args!("error t={} pack: {e}", elapsed()));
            return;
        }
    };
    let build_t0 = elapsed();
    let opts = ssb_pack::PackOptions::default();
    let result = ssb_pack::build(&rom, &opts, &probe, &ssb_pack::NoObserver, &mut sink);
    let build_us = elapsed().wrapping_sub(build_t0);
    let flushed = sink.flush();
    sink.close();
    drop(rom);
    probe.phase_line("closed");
    match (result, flushed) {
        (Ok(stats), Ok(())) => {
            log(format_args!(
                "done t={} build_us={build_us} pack_bytes={} sink_us={} sink_writes={} log_us={}",
                elapsed(),
                SINK_WRITTEN.load(Ordering::Relaxed),
                SINK_US.load(Ordering::Relaxed),
                SINK_CALLS.load(Ordering::Relaxed),
                LOG_US.load(Ordering::Relaxed),
            ));
            log(format_args!(
                "stats meshes={} triangles={} sprites={} objects={} fighters={} \
                 fighters_failed={} packed_anims={} anims_failed={} particle_scripts={} \
                 particle_textures={}",
                stats.meshes,
                stats.triangles,
                stats.sprites,
                stats.objects,
                stats.fighters,
                stats.fighters_failed.len(),
                stats.packed_anims,
                stats.anims_failed.len(),
                stats.particle_scripts,
                stats.particle_textures,
            ));
        }
        (Err(e), _) => log(format_args!(
            "error t={} build_us={build_us} phase={}: {e}",
            elapsed(),
            PHASE.get()
        )),
        (Ok(_), Err(e)) => log(format_args!("error t={} pack flush: {e}", elapsed())),
    }
}

/// The whole ROM in one allocation.
fn read_rom() -> Result<Vec<u8>, String> {
    // SAFETY: NUL-terminated path.
    let fd = unsafe { sys::sceIoOpen(ROM.as_ptr(), sys::IoOpenFlags::RD_ONLY, 0) };
    if fd.0 < 0 {
        return Err(alloc::format!("open {:#x}", fd.0));
    }
    // SAFETY: an open descriptor; closed on every path below.
    let size = unsafe { sys::sceIoLseek(fd, 0, sys::IoWhence::End) };
    // SAFETY: as above.
    unsafe { sys::sceIoLseek(fd, 0, sys::IoWhence::Set) };
    if !(1..=64 << 20).contains(&size) {
        // SAFETY: as above.
        unsafe { sys::sceIoClose(fd) };
        return Err(alloc::format!("size {size}"));
    }
    let size = size as usize;
    let mut rom = Vec::<u8>::with_capacity(size);
    let mut done = 0;
    while done < size {
        let want = (size - done).min(READ_BYTES);
        // SAFETY: `done + want <= size` bytes of `rom`'s capacity.
        let got =
            unsafe { sys::sceIoRead(fd, rom.as_mut_ptr().add(done) as *mut c_void, want as u32) };
        if got <= 0 {
            // SAFETY: as above.
            unsafe { sys::sceIoClose(fd) };
            return Err(alloc::format!("read {got:#x} at {done}"));
        }
        done += got as usize;
        tick();
    }
    // SAFETY: as above.
    unsafe { sys::sceIoClose(fd) };
    // SAFETY: the reads filled all `size` bytes.
    unsafe { rom.set_len(size) };
    Ok(rom)
}

/// The build's [`ssb_pack::PackLog`].
struct Probe;

impl Probe {
    fn phase_line(&self, name: &str) {
        PHASE.set(name);
        let t = elapsed();
        let dt = t.wrapping_sub(LAST_PHASE.swap(t, Ordering::Relaxed));
        let small = heap::stats();
        let (allocs, kernel_allocs) = heap::counts();
        memory::sample();
        let (_, low) = memory::low_water();
        log(format_args!(
            "phase {name} t={t} dt={dt} sink_us={} sink_bytes={} log_us={} free={} max={} \
             low={low} heap_chunks={} heap_chunk_bytes={} heap_live={} allocs={allocs} \
             kernel_allocs={kernel_allocs} stack_free={}",
            SINK_US.load(Ordering::Relaxed),
            SINK_WRITTEN.load(Ordering::Relaxed),
            LOG_US.load(Ordering::Relaxed),
            memory::free(),
            memory::max_block(),
            small.chunks,
            small.chunk_bytes,
            small.live_bytes,
            thread::stack_free_bytes(),
        ));
    }
}

impl ssb_pack::PackLog for Probe {
    fn line(&self, args: fmt::Arguments<'_>) {
        log(format_args!("line t={} {args}", elapsed()));
    }

    fn diag(&self, args: fmt::Arguments<'_>) {
        log(format_args!("diag t={} {args}", elapsed()));
    }

    fn phase(&self, name: &str) {
        self.phase_line(name);
    }
}

/// The pack sink's buffer, in `.bss` and cache-line aligned. The alignment
/// also gives `.bss` the 64-byte alignment `tools/psp-link.py` needs to
/// place it (RE-584): the game's own aligned statics are dead in this build.
#[repr(C, align(64))]
struct SinkBuf(UnsafeCell<[u8; SINK_BYTES]>);

// SAFETY: one `FileSink`, on the main thread, ever uses it.
unsafe impl Sync for SinkBuf {}

static SINK_BUF: SinkBuf = SinkBuf(UnsafeCell::new([0; SINK_BYTES]));

/// [`PACK`] through [`SINK_BUF`].
struct FileSink {
    fd: sys::SceUid,
    buf: &'static mut [u8; SINK_BYTES],
    len: usize,
}

impl FileSink {
    /// Opens [`PACK`]; call once.
    fn create() -> Result<Self, String> {
        // SAFETY: NUL-terminated path.
        let fd = unsafe {
            sys::sceIoOpen(
                PACK.as_ptr(),
                sys::IoOpenFlags::WR_ONLY | sys::IoOpenFlags::CREAT | sys::IoOpenFlags::TRUNC,
                0o777,
            )
        };
        if fd.0 < 0 {
            return Err(alloc::format!("open {:#x}", fd.0));
        }
        Ok(Self {
            fd,
            // SAFETY: the only reference: `build` creates one sink, once.
            buf: unsafe { &mut *SINK_BUF.0.get() },
            len: 0,
        })
    }

    fn flush(&mut self) -> Result<(), String> {
        let r = write_all(self.fd, &self.buf[..self.len]);
        self.len = 0;
        r
    }

    fn close(&mut self) {
        let t = now();
        // SAFETY: an open descriptor, closed once.
        unsafe { sys::sceIoClose(self.fd) };
        SINK_US.fetch_add(now().wrapping_sub(t), Ordering::Relaxed);
    }
}

/// Writes all of `bytes` to `fd`, timed into [`SINK_US`].
fn write_all(fd: sys::SceUid, mut bytes: &[u8]) -> Result<(), String> {
    let t = now();
    let mut result = Ok(());
    while !bytes.is_empty() {
        // SAFETY: an open descriptor; `bytes` outlives the call.
        let n = unsafe { sys::sceIoWrite(fd, bytes.as_ptr() as *const c_void, bytes.len()) };
        if n <= 0 {
            result = Err(alloc::format!("write {n:#x}"));
            break;
        }
        SINK_WRITTEN.fetch_add(n as usize, Ordering::Relaxed);
        bytes = &bytes[n as usize..];
    }
    SINK_CALLS.fetch_add(1, Ordering::Relaxed);
    SINK_US.fetch_add(now().wrapping_sub(t), Ordering::Relaxed);
    tick();
    result
}

impl ssb_rom::pack::PackSink for FileSink {
    fn write(&mut self, bytes: &[u8]) -> Result<(), ssb_rom::pack::SinkError> {
        if self.len + bytes.len() > SINK_BYTES {
            self.flush()?;
        }
        if bytes.len() >= SINK_BYTES {
            write_all(self.fd, bytes)?;
        } else {
            self.buf[self.len..self.len + bytes.len()].copy_from_slice(bytes);
            self.len += bytes.len();
        }
        Ok(())
    }
}

/// `memory::fatal`'s hook: an allocation failed (or another fatal screen).
fn on_fatal() {
    log(format_args!(
        "fatal t={} phase={} free={} max={} (allocation failure: size on screen)",
        elapsed(),
        PHASE.get(),
        memory::free(),
        memory::max_block(),
    ));
}

/// One line appended to [`LOG`] (open, write, close: kept on a crash).
/// Formats into a fixed buffer, so it allocates nothing; a longer line is
/// cut.
fn log(args: fmt::Arguments<'_>) {
    let t = now();
    let mut line = Line {
        buf: [0; 1024],
        len: 0,
    };
    let _ = line.write_fmt(args);
    let _ = line.write_str("\n");
    if line.len == line.buf.len() {
        line.buf[line.len - 1] = b'\n';
    }
    // SAFETY: NUL-terminated path; the buffer outlives the write.
    unsafe {
        let fd = sys::sceIoOpen(
            LOG.as_ptr(),
            sys::IoOpenFlags::WR_ONLY | sys::IoOpenFlags::CREAT | sys::IoOpenFlags::APPEND,
            0o777,
        );
        if fd.0 >= 0 {
            sys::sceIoWrite(fd, line.buf.as_ptr() as *const c_void, line.len);
            sys::sceIoClose(fd);
        }
    }
    LOG_US.fetch_add(now().wrapping_sub(t), Ordering::Relaxed);
    tick();
}

/// A fixed line buffer; bytes past its end are dropped.
struct Line {
    buf: [u8; 1024],
    len: usize,
}

impl Write for Line {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let n = s.len().min(self.buf.len() - self.len);
        self.buf[self.len..self.len + n].copy_from_slice(&s.as_bytes()[..n]);
        self.len += n;
        Ok(())
    }
}

/// Keeps the PSP awake and samples free memory (`memory::low_water`).
fn spawn_ticker() {
    unsafe extern "C" fn entry(_args: usize, _argp: *mut c_void) -> i32 {
        loop {
            tick();
            memory::sample();
            // SAFETY: plain sleep.
            unsafe { sys::sceKernelDelayThread(TICK_US) };
        }
    }
    // SAFETY: NUL-terminated static name; `entry` takes no arguments.
    unsafe {
        let id = sys::sceKernelCreateThread(
            b"pack_probe_tick\0".as_ptr(),
            entry,
            TICK_PRIORITY,
            16 * 1024,
            sys::ThreadAttributes::USER,
            core::ptr::null_mut(),
        );
        if id.0 >= 0 {
            sys::sceKernelStartThread(id, 0, core::ptr::null_mut());
        }
    }
}
