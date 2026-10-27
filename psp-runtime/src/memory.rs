//! User-memory measurement, the test ballast and the out-of-memory screen
//! (RE-475).
//!
//! The PSP's user partition is smaller when a game starts from the XMB
//! than under PSPLink: RE-475 measured 47,891,712 free bytes at
//! `psp_main` from the XMB of a PSP-2000 with 6.61 ARK, 52,086,016 under
//! PSPLink. [`hold_ballast`] takes the difference away at boot so PPSSPP
//! and PSPLink runs see a launch's budget.

use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use psp::sys;

/// Free bytes in the user partition, summed over every free block.
pub fn free() -> usize {
    // SAFETY: a query with no preconditions.
    unsafe { sys::sceKernelTotalFreeMemSize() as usize }
}

/// The largest single allocation the user partition could satisfy now.
pub fn max_block() -> usize {
    // SAFETY: a query with no preconditions.
    unsafe { sys::sceKernelMaxFreeMemSize() as usize }
}

/// The lowest [`free`] seen by [`sample`] since boot.
static LOW_WATER: AtomicUsize = AtomicUsize::new(usize::MAX);
/// [`free`] at the first [`sample`].
static AT_BOOT: AtomicUsize = AtomicUsize::new(0);

/// Records [`free`] for [`low_water`]; returns it.
pub fn sample() -> usize {
    let now = free();
    if AT_BOOT.load(Ordering::Relaxed) == 0 {
        AT_BOOT.store(now, Ordering::Relaxed);
    }
    LOW_WATER.fetch_min(now, Ordering::Relaxed);
    now
}

/// `(free at the first sample, lowest free since)`.
pub fn low_water() -> (usize, usize) {
    (
        AT_BOOT.load(Ordering::Relaxed),
        LOW_WATER.load(Ordering::Relaxed),
    )
}

/// Allocates and never frees the bytes that leave `target` free, so this
/// run sees the user memory a launch with `target` free would. Returns the
/// bytes held (0 when `target` is already at or above what is free).
pub fn hold_ballast(target: usize) -> usize {
    let take = free().saturating_sub(target);
    if take == 0 {
        return 0;
    }
    // From the partition's top, as the memory a smaller launch lacks.
    // SAFETY: the name is NUL-terminated; the block is never freed.
    let id = unsafe {
        sys::sceKernelAllocPartitionMemory(
            sys::SceSysMemPartitionId::SceKernelPrimaryUserPartition,
            b"ballast\0".as_ptr(),
            sys::SceSysMemBlockTypes::High,
            take as u32,
            core::ptr::null_mut(),
        )
    };
    if id.0 < 0 {
        0
    } else {
        take
    }
}

/// The ballast target from `ballast.txt` beside the EBOOT (the capture
/// scene's search paths): free bytes, decimal.
pub fn ballast_target() -> Option<usize> {
    const PATHS: &[&str] = &["ballast.txt\0", "ms0:/PSP/GAME/ssb64/ballast.txt\0"];
    let mut buf = [0u8; 32];
    for path in PATHS {
        // SAFETY: path is a NUL-terminated literal.
        let fd = unsafe { sys::sceIoOpen(path.as_ptr(), sys::IoOpenFlags::RD_ONLY, 0o777) };
        if fd.0 < 0 {
            continue;
        }
        let n = unsafe {
            sys::sceIoRead(
                fd,
                buf.as_mut_ptr() as *mut core::ffi::c_void,
                buf.len() as u32,
            )
        };
        unsafe { sys::sceIoClose(fd) };
        let text = core::str::from_utf8(buf.get(..n.max(0) as usize)?).ok()?;
        return text.trim().parse().ok();
    }
    None
}

/// Shows `lines` on a black screen and keeps the thread waiting on vblank,
/// so HOME still quits: the screen a load or allocation failure ends on
/// instead of a hang (RE-475). Draws with the CPU into VRAM, after the GE
/// has finished, and allocates nothing.
pub fn fatal(lines: &[&str]) -> ! {
    // SAFETY: the GE is waited for before the CPU writes VRAM; the waits
    // have no preconditions.
    unsafe {
        sys::sceGuSync(sys::GuSyncMode::Finish, sys::GuSyncBehavior::Wait);
    }
    psp::dprintln!("Super Smash Bros. 64 (PSP) cannot continue.");
    psp::dprintln!("");
    for l in lines {
        psp::dprintln!("{}", l);
    }
    psp::dprintln!("");
    psp::dprintln!(
        "Free memory: {} bytes, largest block {} bytes.",
        free(),
        max_block()
    );
    psp::dprintln!("Press HOME to quit.");
    for l in lines {
        // SAFETY: the bytes outlive the write.
        unsafe {
            sys::sceIoWrite(
                sys::sceKernelStdout(),
                b"fatal: ".as_ptr() as *const core::ffi::c_void,
                7,
            );
            sys::sceIoWrite(
                sys::sceKernelStdout(),
                l.as_ptr() as *const core::ffi::c_void,
                l.len(),
            );
            sys::sceIoWrite(
                sys::sceKernelStdout(),
                b"\n".as_ptr() as *const core::ffi::c_void,
                1,
            );
        }
    }
    // SAFETY: as above. PPSSPPHeadless saves the screen; a PSP ignores the
    // emulator's devctl.
    unsafe { sys::sceDisplayWaitVblankStart() };
    crate::gu::emit_headless_screenshot();
    loop {
        // SAFETY: as above.
        unsafe { sys::sceDisplayWaitVblankStart() };
    }
}

/// Bytes behind [`frame_arena`]. Measured high water: 28 B (opening
/// movie camera order); 4 KiB keeps static RAM low for the PSP-1000.
pub const FRAME_ARENA_BYTES: usize = 4 * 1024;

/// The frame arena's storage, on its own D-cache lines like the GE
/// buffers (`gu.rs`, RE-360), though only the CPU touches it.
#[repr(C, align(64))]
struct FrameBuf([u8; FRAME_ARENA_BYTES]);

static mut FRAME_BUF: FrameBuf = FrameBuf([0; FRAME_ARENA_BYTES]);
static FRAME_TAKEN: AtomicBool = AtomicBool::new(false);

/// The per-tick scratch arena (`docs/memory.md`, Allocators) over a
/// static buffer: the game loop opens one scope a tick, so per-frame
/// lists never touch the heap. The first call gets it; later calls get
/// `None`.
pub fn frame_arena() -> Option<ssb_engine::memory::FrameArena<'static>> {
    if FRAME_TAKEN.swap(true, Ordering::AcqRel) {
        return None;
    }
    // SAFETY: the flag hands the buffer out once, so this is its only
    // reference for the rest of the program.
    Some(unsafe {
        ssb_engine::memory::Arena::from_aligned(
            core::ptr::addr_of_mut!(FRAME_BUF.0).cast::<u8>(),
            FRAME_ARENA_BYTES,
        )
    })
}
