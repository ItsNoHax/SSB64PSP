//! FCR31 at an XMB-equivalent launch, readable without a person at the PSP
//! (RE-516). Replaces the game: [`run`] never returns.
//!
//! The XMB starts a Memory Stick EBOOT with `sceKernelLoadExecVSHMs2`
//! (apitype 0x141), which reboots user mode. ARK exports the same call to
//! user mode as `SystemCtrlForUser`'s `sctrlKernelLoadExecVSHMs2`, so a
//! PRX started under PSPLink can launch an EBOOT exactly as the XMB does,
//! and that EBOOT can relaunch PSPLink the same way.
//!
//! One build, two roles, chosen by [`LAUNCH_FLAG`] beside the module (on
//! `host0:` only):
//!
//! 1. `ldstart host0:/.../ssb64-psp-game.prx` with the flag: appends this
//!    PSPLink launch's line to [`LOG`], then LoadExecs [`PROBE_EBOOT`].
//! 2. [`PROBE_EBOOT`] (no flag): appends its line, then LoadExecs
//!    [`PSPLINK_EBOOT`].
//!
//! Each line: `fcr31 launch=<ldstart|loadexec> main=<hex> thread=<hex>`,
//! `main` as the main thread started (`fpu::init_thread`), `thread` from a
//! new thread with the audio thread's priority, stack and attributes. A
//! watchdog thread relaunches PSPLink after [`WATCHDOG_US`] whatever the
//! main thread does, so a failure cannot leave the PSP outside PSPLink.
//! Nothing here allocates or panics: a panic would `sceKernelExitGame` to
//! the XMB.

use core::ffi::c_void;
use psp::sys;

use crate::fpu::{self, GameThread};

/// The probe's output, appended to by both roles.
const LOG: &[u8] = b"ms0:/PSP/GAME/ssb64probe/fcr31.txt\0";
/// The probe EBOOT the PSPLink role launches.
const PROBE_EBOOT: &[u8] = b"ms0:/PSP/GAME/ssb64probe/EBOOT.PBP\0";
/// PSPLink's EBOOT (`pspsh -e config`: `ms0:/PSP/GAME/psplink/psplink.ini`).
const PSPLINK_EBOOT: &[u8] = b"ms0:/PSP/GAME/psplink/EBOOT.PBP\0";
/// Present beside the PRX on `host0:` only: selects the PSPLink role.
const LAUNCH_FLAG: &[u8] = b"fcr31_launch.txt\0";
/// The watchdog's delay before it relaunches PSPLink.
const WATCHDOG_US: u32 = 10_000_000;

/// `struct SceKernelLoadExecVSHParam` (pspsdk `psploadexec_kernel.h`).
#[repr(C)]
struct LoadExecVshParam {
    size: u32,
    args: u32,
    argp: *const c_void,
    key: *const u8,
    vshmain_args_size: u32,
    vshmain_args: *const c_void,
    configfile: *const u8,
    unk4: u32,
    unk5: u32,
}

/// Records the FCR31 values, appends them to [`LOG`] and LoadExecs the
/// next role's EBOOT; retries until the watchdog relaunches PSPLink.
pub fn run() -> ! {
    spawn(b"fcr31_watchdog\0", watchdog, 0x10, 16 * 1024);
    let thread = probe_thread();
    let main = fpu::launch_fcr31(GameThread::Main).unwrap_or(u32::MAX);
    let ldstart = exists(LAUNCH_FLAG);
    let mut line = Line::new();
    line.push(b"fcr31 launch=");
    line.push(if ldstart { b"ldstart" } else { b"loadexec" });
    line.push(b" main=");
    line.hex(main);
    line.push(b" thread=");
    line.hex(thread);
    line.push(b"\n");
    append(&line);
    let target = if ldstart { PROBE_EBOOT } else { PSPLINK_EBOOT };
    load_exec(target);
    loop {
        // SAFETY: plain sleep; the watchdog takes over from here.
        unsafe { sys::sceKernelDelayThread(1_000_000) };
    }
}

/// The FCR31 a new thread with the audio thread's priority, stack and
/// attributes starts with; `u32::MAX` if it did not run.
fn probe_thread() -> u32 {
    unsafe extern "C" fn entry(_args: usize, _argp: *mut c_void) -> i32 {
        fpu::init_thread(GameThread::Audio);
        0
    }
    let id = spawn(b"ssb_audio\0", entry, crate::audio::PRIORITY, crate::audio::STACK_BYTES);
    if id.0 >= 0 {
        let mut timeout: u32 = 1_000_000;
        // SAFETY: waits on the thread just started, with a timeout.
        unsafe {
            sys::sceKernelWaitThreadEnd(id, &mut timeout);
            sys::sceKernelDeleteThread(id);
        }
    }
    fpu::launch_fcr31(GameThread::Audio).unwrap_or(u32::MAX)
}

unsafe extern "C" fn watchdog(_args: usize, _argp: *mut c_void) -> i32 {
    // SAFETY: plain sleep.
    unsafe { sys::sceKernelDelayThread(WATCHDOG_US) };
    let mut line = Line::new();
    line.push(b"fcr31 watchdog fired\n");
    append(&line);
    loop {
        load_exec(PSPLINK_EBOOT);
        // SAFETY: plain sleep between attempts.
        unsafe { sys::sceKernelDelayThread(2_000_000) };
    }
}

fn spawn(
    name: &[u8],
    entry: unsafe extern "C" fn(usize, *mut c_void) -> i32,
    priority: i32,
    stack: i32,
) -> sys::SceUid {
    // SAFETY: `name` is NUL-terminated and static; `entry` takes no args.
    unsafe {
        let id = sys::sceKernelCreateThread(
            name.as_ptr(),
            entry,
            priority,
            stack,
            sys::ThreadAttributes::USER | sys::ThreadAttributes::VFPU,
            core::ptr::null_mut(),
        );
        if id.0 >= 0 {
            sys::sceKernelStartThread(id, 0, core::ptr::null_mut());
        }
        id
    }
}

/// `sctrlKernelLoadExecVSHMs2(path)` as the XMB launches an EBOOT (key
/// `game`, argv = the path); `sceKernelLoadExec` if that returns. Logs
/// each failure; returns only if both fail.
fn load_exec(path: &'static [u8]) {
    let param = LoadExecVshParam {
        size: core::mem::size_of::<LoadExecVshParam>() as u32,
        args: path.len() as u32,
        argp: path.as_ptr() as *const c_void,
        key: b"game\0".as_ptr(),
        vshmain_args_size: 0,
        vshmain_args: core::ptr::null(),
        configfile: core::ptr::null(),
        unk4: 0,
        unk5: 0,
    };
    // SAFETY: NUL-terminated static path; the param outlives the call.
    let rc = unsafe { sctrl_load_exec_vsh_ms2(path.as_ptr(), &param) };
    let mut line = Line::new();
    line.push(b"fcr31 vshms2 failed rc=");
    line.hex(rc as u32);
    line.push(b"\n");
    append(&line);
    let mut fallback = sys::SceKernelLoadExecParam {
        size: core::mem::size_of::<sys::SceKernelLoadExecParam>(),
        args: path.len(),
        argp: path.as_ptr() as *mut c_void,
        key: core::ptr::null(),
    };
    // SAFETY: as above.
    let rc2 = unsafe { sys::sceKernelLoadExec(path.as_ptr(), &mut fallback) };
    let mut line = Line::new();
    line.push(b"fcr31 loadexec failed rc=");
    line.hex(rc2 as u32);
    line.push(b"\n");
    append(&line);
}

fn exists(path: &[u8]) -> bool {
    // SAFETY: NUL-terminated path.
    unsafe {
        let fd = sys::sceIoOpen(path.as_ptr(), sys::IoOpenFlags::RD_ONLY, 0);
        if fd.0 >= 0 {
            sys::sceIoClose(fd);
        }
        fd.0 >= 0
    }
}

fn append(line: &Line) {
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
}

/// A fixed line buffer; bytes past its end are dropped.
struct Line {
    buf: [u8; 96],
    len: usize,
}

impl Line {
    fn new() -> Self {
        Self { buf: [0; 96], len: 0 }
    }

    fn push(&mut self, s: &[u8]) {
        for &b in s {
            if self.len < self.buf.len() {
                self.buf[self.len] = b;
                self.len += 1;
            }
        }
    }

    fn hex(&mut self, v: u32) {
        const DIGITS: &[u8; 16] = b"0123456789abcdef";
        self.push(b"0x");
        for shift in (0..8).rev() {
            self.push(&[DIGITS[((v >> (shift * 4)) & 0xF) as usize]]);
        }
    }
}

// The import of ARK's `SystemCtrlForUser` `sctrlKernelLoadExecVSHMs2`
// (NID 0xABA7F1B0, the SHA-1 of the name; listed by `pspsh -e modexp` on
// the PSP-2000's SystemControl). Stub tables as in `hwcount.rs`.

#[repr(C)]
struct Stub {
    lib: &'static sys::SceStubLibraryEntry,
    nid: &'static u32,
}

// SAFETY: read-only addresses, replaced by the loader with `jr ra; syscall`.
unsafe impl Sync for Stub {}

#[link_section = ".rodata.sceResident.SystemCtrlForUser"]
static SCTRL_NAME: [u8; 18] = *b"SystemCtrlForUser\0";

#[link_section = ".rodata.sceNid.SystemCtrlForUser"]
static SCTRL_NID_START: () = ();

#[link_section = ".sceStub.text.SystemCtrlForUser"]
static SCTRL_STUB_START: () = ();

#[link_section = ".lib.stub.entry.SystemCtrlForUser"]
static SCTRL_STUB: sys::SceStubLibraryEntry = sys::SceStubLibraryEntry {
    name: &SCTRL_NAME[0],
    version: [0, 0],
    flags: 0x4001,
    len: 5,
    v_stub_count: 0,
    // Patched by cargo-psp, as for rust-psp's own imports.
    stub_count: 0,
    nid_table: &SCTRL_NID_START as *const () as *const _,
    stub_table: &SCTRL_STUB_START as *const () as *const _,
};

#[link_section = ".rodata.sceNid.SystemCtrlForUser.sctrlKernelLoadExecVSHMs2"]
static SCTRL_LOAD_EXEC_VSH_MS2_NID: u32 = 0xABA7_F1B0;

#[link_section = ".sceStub.text.SystemCtrlForUser.sctrlKernelLoadExecVSHMs2"]
#[no_mangle]
static __ssb_sctrlKernelLoadExecVSHMs2_stub: Stub =
    Stub { lib: &SCTRL_STUB, nid: &SCTRL_LOAD_EXEC_VSH_MS2_NID };

unsafe fn sctrl_load_exec_vsh_ms2(path: *const u8, param: *const LoadExecVshParam) -> i32 {
    extern "C" {
        fn __ssb_sctrlKernelLoadExecVSHMs2_stub(path: *const u8, param: *const LoadExecVshParam) -> i32;
    }
    __ssb_sctrlKernelLoadExecVSHMs2_stub(path, param)
}
