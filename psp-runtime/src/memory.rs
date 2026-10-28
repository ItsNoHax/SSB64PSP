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
use ssb_engine::memory::{BlockAlloc, GameArena, GameArenaError, GameArenaUsage, SceneBox};

use crate::assets::CacheLineBlocks;

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

/// Bytes behind [`frame_arena`]. Largest user: the magnifier's depth-mask
/// lists, `ssb_rom::depth_mask::scratch_bytes` at the widest magnifier
/// (2,923 B; `psp-game` asserts they fit). 4 KiB keeps static RAM low for
/// the PSP-1000.
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

/// The scene arena's blocks. The scene's own block (past
/// [`ssb_engine::heap::MAX_SMALL`]) is the kernel's, on whole cache lines
/// ([`CacheLineBlocks`]). An object grown past it
/// ([`scene_alloc_grow_uninit`]) comes from the general heap like any
/// small value: its size class sits in chunks the heap has already taken,
/// where a kernel block of its own would take fresh memory
/// (`docs/memory.md`).
struct SceneBlocks;

impl BlockAlloc for SceneBlocks {
    type Block = crate::assets::AlignedBuf;
    fn alloc(&mut self, len: usize) -> Option<crate::assets::AlignedBuf> {
        if len > ssb_engine::heap::MAX_SMALL {
            CacheLineBlocks.alloc(len)
        } else {
            crate::assets::AlignedBuf::new(len)
        }
    }
    fn ptr(block: &crate::assets::AlignedBuf) -> *mut u8 {
        CacheLineBlocks::ptr(block)
    }
}

/// The scene arena (`docs/memory.md`, Allocators): `syTaskmanMalloc`'s
/// general heap, one block a scene, taken at its entry and freed at its
/// exit. Only the main thread touches it.
struct SceneArena(core::cell::UnsafeCell<GameArena<SceneBlocks>>);

// SAFETY: only the main thread calls the functions below (the game loop
// owns every scene).
unsafe impl Sync for SceneArena {}

static SCENE: SceneArena = SceneArena(core::cell::UnsafeCell::new(GameArena::new(SceneBlocks)));

/// The scene arena. Each caller below holds the reference for the call
/// only and none of them re-enters another.
#[allow(clippy::mut_from_ref)]
fn scene() -> &'static mut GameArena<SceneBlocks> {
    // SAFETY: main thread only, and no reference outlives its call (above).
    unsafe { &mut *SCENE.0.get() }
}

/// One [`fatal`] line, formatted on the stack: the heap may be what
/// failed. Truncated past its 96 bytes.
struct Line {
    buf: [u8; 96],
    len: usize,
}

impl Line {
    fn new(args: core::fmt::Arguments) -> Self {
        let mut line = Line {
            buf: [0; 96],
            len: 0,
        };
        let _ = core::fmt::Write::write_fmt(&mut line, args);
        line
    }

    fn as_str(&self) -> &str {
        // SAFETY: `write_str` copies whole characters only.
        unsafe { core::str::from_utf8_unchecked(&self.buf[..self.len]) }
    }
}

impl core::fmt::Write for Line {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let mut n = s.len().min(self.buf.len() - self.len);
        while !s.is_char_boundary(n) {
            n -= 1;
        }
        self.buf[self.len..self.len + n].copy_from_slice(&s.as_bytes()[..n]);
        self.len += n;
        Ok(())
    }
}

/// [`fatal`] for a scene arena failure, naming the scene and the sizes.
#[cold]
#[inline(never)]
fn scene_fatal(e: GameArenaError) -> ! {
    let line = match e {
        GameArenaError::Full {
            scene,
            need,
            used,
            capacity,
        } => Line::new(format_args!(
            "scene arena '{scene}' full: {need} B wanted, {used} of {capacity} B used"
        )),
        GameArenaError::NoMemory { scene, bytes } => {
            Line::new(format_args!("scene arena '{scene}': no {bytes} B block"))
        }
        GameArenaError::NoScene => Line::new(format_args!("scene arena: no scene")),
        GameArenaError::Open { scene } => {
            Line::new(format_args!("scene arena: '{scene}' still open"))
        }
        GameArenaError::Live { scene, live } => Line::new(format_args!(
            "scene arena '{scene}': {live} values outlive the scene"
        )),
    };
    fatal(&["Out of memory.", line.as_str()])
}

/// [`fatal`] for an infallible allocation that got no memory (the global
/// allocator's error handler, `crate::heap`): the size asked for, the
/// scene whose arena is open and the thread.
#[cold]
#[inline(never)]
pub fn alloc_failed(size: usize, align: usize) -> ! {
    let need = Line::new(format_args!(
        "allocation of {size} B (align {align}) failed"
    ));
    // SAFETY: read only, to name the scene; the arena may be mid-call on
    // this thread, and nothing returns to that call.
    let scene = unsafe { (*SCENE.0.get()).usage().scene };
    let mut info = core::mem::MaybeUninit::<sys::SceKernelThreadInfo>::zeroed();
    let p = info.as_mut_ptr();
    // SAFETY: `p` points at a writable record (the kernel fills it); only
    // its size and name are touched, plain integers.
    let name = unsafe {
        (*p).size = core::mem::size_of::<sys::SceKernelThreadInfo>();
        let thid = sys::SceUid(sys::sceKernelGetThreadId());
        if sys::sceKernelReferThreadStatus(thid, p) >= 0 {
            (*p).name
        } else {
            let mut unknown = [0; 32];
            unknown[0] = b'?';
            unknown
        }
    };
    let mut place = Line::new(format_args!("scene '{scene}', thread '"));
    for &b in name.iter().take_while(|&&b| b != 0) {
        let c = match b {
            b' '..=b'~' => char::from(b),
            _ => '?',
        };
        let _ = core::fmt::Write::write_char(&mut place, c);
    }
    let _ = core::fmt::Write::write_str(&mut place, "'");
    fatal(&["Out of memory.", need.as_str(), place.as_str()])
}

/// Opens scene `name`'s arena block of `bytes`, after closing the last
/// scene's ([`end_scene`]).
pub fn begin_scene(name: &'static str, bytes: usize) {
    end_scene();
    scene().begin(name, bytes).unwrap_or_else(|e| scene_fatal(e));
}

/// Frees the open scene's block, if any; every [`SceneBox`] from it must
/// be gone. Returns its use.
pub fn end_scene() -> GameArenaUsage {
    if !scene().is_open() {
        return GameArenaUsage::default();
    }
    scene().end().unwrap_or_else(|e| scene_fatal(e))
}

/// The open scene's use (`scene` empty when none is open).
pub fn scene_usage() -> GameArenaUsage {
    scene().usage()
}

/// Uninitialised room for a `T` in the open scene.
pub fn scene_alloc_uninit<T>() -> SceneBox<core::mem::MaybeUninit<T>> {
    scene().alloc_uninit().unwrap_or_else(|e| scene_fatal(e))
}

/// Uninitialised room for a `T` in the open scene, past its block when
/// that is full ([`GameArena::alloc_grow_uninit`]): an object the scene
/// makes on demand and keeps on a free list of its own.
pub fn scene_alloc_grow_uninit<T>() -> SceneBox<core::mem::MaybeUninit<T>> {
    scene().alloc_grow_uninit().unwrap_or_else(|e| scene_fatal(e))
}

/// `value` in the open scene.
pub fn scene_alloc<T>(value: T) -> SceneBox<T> {
    scene().alloc(value).unwrap_or_else(|e| scene_fatal(e))
}
