//! Starting another EBOOT: the game's hand-off to the first-launch
//! installer (`psp-installer`, RE-588) when the boot finds no valid pack.
//!
//! - The installer is `INSTALL.PBP` in the game's own folder: the folder
//!   of the path the game was started with (`argv[0]`, [`exe_path`]).
//! - Launched as the XMB launches an EBOOT: ARK's and PRO's
//!   `SystemCtrlForUser` `sctrlKernelLoadExecVSHMs2` (key `game`,
//!   argv = the path), the call RE-516 ran on a PSP-2000 from a user-mode
//!   module; `sceKernelLoadExec` if that returns. The new EBOOT gets the
//!   XMB's memory and its own `argv[0]`, so its folder is the working
//!   directory as for a launch from the XMB.
//! - Neither returns on success: LoadExec ends this program.
//! - Nothing here allocates: the gate runs before the game's heap.

use core::ffi::c_void;
use psp::sys;

/// The installer's file name, in the game's folder.
pub const INSTALLER: &str = "INSTALL.PBP";

/// `struct SceKernelLoadExecVSHParam` (pspsdk `psploadexec_kernel.h`).
#[repr(C)]
pub(crate) struct LoadExecVshParam {
    pub size: u32,
    pub args: u32,
    pub argp: *const c_void,
    pub key: *const u8,
    pub vshmain_args_size: u32,
    pub vshmain_args: *const c_void,
    pub configfile: *const u8,
    pub unk4: u32,
    pub unk5: u32,
}

// The import of ARK's `SystemCtrlForUser` `sctrlKernelLoadExecVSHMs2`
// (NID 0xABA7F1B0, the SHA-1 of the name; listed by `pspsh -e modexp` on
// the PSP-2000's SystemControl, RE-516).
pub(crate) mod sctrl {
    use super::LoadExecVshParam;

    psp::psp_extern! {
        #![name = "SystemCtrlForUser"]
        #![flags = 0x4001]
        #![version = (0x00, 0x00)]

        #[psp(0xABA7F1B0)]
        pub fn sctrlKernelLoadExecVSHMs2(path: *const u8, param: *const LoadExecVshParam) -> i32;
    }
}

/// LoadExecs `path` (NUL-terminated) as the XMB would; returns the two
/// calls' results only if both fail.
pub(crate) fn load_exec(path: &[u8]) -> (i32, i32) {
    let args = path.iter().position(|&b| b == 0).map_or(0, |n| n + 1);
    let param = LoadExecVshParam {
        size: core::mem::size_of::<LoadExecVshParam>() as u32,
        args: args as u32,
        argp: path.as_ptr() as *const c_void,
        key: b"game\0".as_ptr(),
        vshmain_args_size: 0,
        vshmain_args: core::ptr::null(),
        configfile: core::ptr::null(),
        unk4: 0,
        unk5: 0,
    };
    // SAFETY: NUL-terminated path; the param outlives the call.
    let vsh = unsafe { sctrl::sctrlKernelLoadExecVSHMs2(path.as_ptr(), &param) };
    let mut fallback = sys::SceKernelLoadExecParam {
        size: core::mem::size_of::<sys::SceKernelLoadExecParam>(),
        args,
        argp: path.as_ptr() as *mut c_void,
        key: core::ptr::null(),
    };
    // SAFETY: as above.
    let user = unsafe { sys::sceKernelLoadExec(path.as_ptr(), &mut fallback) };
    (vsh, user)
}

/// The kernel's per-thread block at the top of a user thread's stack; the
/// start arguments lie right below it (`sceKernelStartThread`).
const K0_BYTES: usize = 0x100;
/// How far below that block [`exe_path`] looks for the arguments.
const ARG_WINDOW: usize = 0x200;
/// Devices an EBOOT or PRX starts from.
const DEVICES: [&[u8]; 5] = [b"ms0:/", b"ef0:/", b"host0:/", b"disc0:/", b"umd0:/"];

/// The path this program was started with (`argv[0]`, printable ASCII), read from the
/// calling thread's start arguments: call it on the main thread, which
/// `psp::module!` starts with the module's arguments. The kernel copies
/// them right below the 256-byte block at the top of the thread's stack
/// ([`first_path`]). `None` if none is found.
pub fn exe_path(out: &mut [u8; 192]) -> Option<&[u8]> {
    let mut info = core::mem::MaybeUninit::<sys::SceKernelThreadInfo>::zeroed();
    let p = info.as_mut_ptr();
    // SAFETY: `p` points at a writable record the kernel fills.
    let (base, size) = unsafe {
        (*p).size = core::mem::size_of::<sys::SceKernelThreadInfo>();
        let thid = sys::SceUid(sys::sceKernelGetThreadId());
        if sys::sceKernelReferThreadStatus(thid, p) < 0 {
            return None;
        }
        ((*p).stack as usize, (*p).stack_size as usize)
    };
    if base == 0 || size < K0_BYTES + ARG_WINDOW {
        return None;
    }
    let k0 = base + size - K0_BYTES;
    // SAFETY: inside this thread's own stack, above its live frames.
    let window = unsafe { core::slice::from_raw_parts((k0 - ARG_WINDOW) as *const u8, ARG_WINDOW) };
    let found = first_path(window)?;
    let n = found.len().min(out.len() - 1);
    out[..n].copy_from_slice(&found[..n]);
    out[n] = 0;
    Some(&out[..n])
}

/// `argv[0]` in `window`, the bytes right below the kernel's block: the
/// argument block starts 16-byte aligned, `align16(size)` bytes below the
/// window's end, and its tail padding is left as it was (PPSSPP: 0xFF).
/// So `argv[0]` is the string at the highest 16-byte boundary that starts
/// with a device prefix and ends in a NUL inside the window.
fn first_path(window: &[u8]) -> Option<&[u8]> {
    (16..=window.len()).step_by(16).find_map(|back| {
        let s = &window[window.len() - back..];
        let n = s.iter().position(|&b| b == 0)?;
        let s = &s[..n];
        (DEVICES.iter().any(|d| s.starts_with(d)) && s.iter().all(|b| (b' '..=b'~').contains(b)))
            .then_some(s)
    })
}

/// The game's hand-off when the pack is missing or unusable: LoadExecs
/// [`INSTALLER`] from the game's folder; if that is impossible, says so
/// and quits to the XMB on a button press. Never returns.
pub fn start_installer() -> ! {
    let mut arg0 = [0u8; 192];
    let mut path = [0u8; 224];
    let mut len = 0;
    let dir =
        exe_path(&mut arg0).map(|p| &p[..p.iter().rposition(|&b| b == b'/').map_or(0, |i| i + 1)]);
    let mut result = None;
    if let Some(dir) = dir.filter(|d| !d.is_empty()) {
        for &b in dir.iter().chain(INSTALLER.as_bytes()) {
            if len + 1 < path.len() {
                path[len] = b;
                len += 1;
            }
        }
        path[len] = 0;
        // SAFETY: NUL-terminated path.
        let fd = unsafe { sys::sceIoOpen(path.as_ptr(), sys::IoOpenFlags::RD_ONLY, 0) };
        if fd.0 >= 0 {
            // SAFETY: the descriptor just opened.
            unsafe { sys::sceIoClose(fd) };
            result = Some(load_exec(&path[..=len]));
        } else {
            result = Some((fd.0, fd.0));
        }
    }
    let shown = core::str::from_utf8(&path[..len]).unwrap_or("?");
    psp::dprintln!("Super Smash Bros. 64 (PSP)");
    psp::dprintln!("");
    psp::dprintln!("The game data (ssb64.pak) is missing or out of date, and the");
    psp::dprintln!("setup program that builds it from your ROM could not be started.");
    psp::dprintln!("");
    match result {
        None => psp::dprintln!("The game's folder is unknown: {INSTALLER} not found."),
        Some((a, b)) if a == b => psp::dprintln!("Not found: {shown} (0x{:08X})", a as u32),
        Some((a, b)) => psp::dprintln!(
            "Could not start {shown} (0x{:08X}, 0x{:08X}).",
            a as u32,
            b as u32
        ),
    }
    psp::dprintln!("");
    psp::dprintln!("Copy {INSTALLER} from the release into the folder of EBOOT.PBP,");
    psp::dprintln!("then start the game again.");
    psp::dprintln!("");
    psp::dprintln!("Press any button to quit.");
    wait_and_exit()
}

/// Waits for a button pressed after this call, then quits to the XMB.
fn wait_and_exit() -> ! {
    // SAFETY: plain vblank wait; the screen is drawn.
    unsafe { sys::sceDisplayWaitVblankStart() };
    crate::gu::emit_headless_screenshot();
    let mut released = false;
    loop {
        // SAFETY: plain vblank wait.
        unsafe { sys::sceDisplayWaitVblankStart() };
        let mut pad = sys::SceCtrlData::default();
        // SAFETY: `pad` outlives the call.
        unsafe { sys::sceCtrlPeekBufferPositive(&mut pad, 1) };
        let any = sys::CtrlButtons::SELECT
            | sys::CtrlButtons::START
            | sys::CtrlButtons::UP
            | sys::CtrlButtons::RIGHT
            | sys::CtrlButtons::DOWN
            | sys::CtrlButtons::LEFT
            | sys::CtrlButtons::LTRIGGER
            | sys::CtrlButtons::RTRIGGER
            | sys::CtrlButtons::TRIANGLE
            | sys::CtrlButtons::CIRCLE
            | sys::CtrlButtons::CROSS
            | sys::CtrlButtons::SQUARE;
        if !pad.buttons.intersects(any) {
            released = true;
        } else if released {
            break;
        }
    }
    // SAFETY: no preconditions; does not return.
    unsafe { sys::sceKernelExitGame() };
    loop {
        // SAFETY: plain sleep, should the exit be slow.
        unsafe { sys::sceKernelDelayThread(100_000) };
    }
}
