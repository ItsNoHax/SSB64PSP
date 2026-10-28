//! The audio samples' streamer thread (RE-572 A, RE-573): reads the pages
//! the audio thread queues in the [`PageCache`] from the pack, so the audio
//! thread never touches the Memory Stick ([`ssb_engine::audio::stream`]).
//!
//! - Priority [`PRIORITY`] (0x1E): below the audio thread (0x1C), which
//!   must preempt it mid-read to copy from mapped slots, and above the
//!   game's main thread (0x20), whose busy frames would otherwise starve
//!   it (RE-573: a waiting I/O at the main thread's level makes no
//!   progress while it runs).
//! - Sync reads on its own descriptor, one page each: a 16 KiB page-aligned
//!   read is 2.5 ms median, 4.4 ms max alone, plus at most one 64 KiB
//!   loader chunk in flight ([`crate::scene_files`]).
//! - Woken by a counting semaphore the audio thread signals per request;
//!   it drains the queue, then waits again.

use alloc::boxed::Box;
use core::ffi::c_void;
use core::sync::atomic::{AtomicBool, AtomicI32, AtomicPtr, Ordering};

use psp::sys::{self, SceUid};
use ssb_engine::audio::stream::{PageCache, PageSource};

/// The streamer's priority (lower is more urgent).
pub const PRIORITY: i32 = 0x1E;

/// Its stack: `sceIoRead` frames only.
const STACK_BYTES: i32 = 16 * 1024;

static SEMA: AtomicI32 = AtomicI32::new(-1);
static THREAD: AtomicI32 = AtomicI32::new(-1);
static STOP: AtomicBool = AtomicBool::new(false);
static CACHE: AtomicPtr<PageCache> = AtomicPtr::new(core::ptr::null_mut());
/// The last failed page read's kernel result (`sceIoOpen`, `sceIoLseek` or
/// `sceIoRead`), 0 before any failure.
static LAST_ERROR: AtomicI32 = AtomicI32::new(0);

/// The last failed page read's kernel result, 0 if none failed.
pub fn last_error() -> i32 {
    LAST_ERROR.load(Ordering::Relaxed)
}

/// What the thread reads: the NUL-terminated pack path, the streamed
/// region's file offset and the pack opened by [`start`]. Set before the
/// thread starts.
///
/// The descriptor is opened on the caller's thread: a thread this module
/// creates has no current directory, so the relative pack path fails there
/// with `SCE_KERNEL_ERROR_NOCWD` (0x8002032C, PPSSPP). A reopen after a
/// failed read uses the path, which works only for an absolute one.
static mut SOURCE: (&str, i64, i32) = ("", 0, -1);

/// [`PageCache`]'s wake hook: one signal per queued page.
pub fn wake() {
    let s = SEMA.load(Ordering::Acquire);
    if s >= 0 {
        // SAFETY: a semaphore `start` created; never blocks.
        unsafe { sys::sceKernelSignalSema(SceUid(s), 1) };
    }
}

/// The pack's streamed region on the Memory Stick.
struct PackPages {
    path: &'static str,
    base: i64,
    fd: Option<SceUid>,
}

impl PageSource for PackPages {
    fn read(&mut self, at: u32, buf: &mut [u8]) -> bool {
        let pos = self.base + i64::from(at);
        for attempt in 0..2 {
            if attempt == 1 {
                if let Some(f) = self.fd.take() {
                    // SAFETY: no read in flight on it.
                    unsafe { sys::sceIoClose(f) };
                }
            }
            if self.fd.is_none() {
                // SAFETY: `path` is NUL-terminated.
                let f =
                    unsafe { sys::sceIoOpen(self.path.as_ptr(), sys::IoOpenFlags::RD_ONLY, 0o777) };
                if f.0 < 0 {
                    LAST_ERROR.store(f.0, Ordering::Relaxed);
                    continue;
                }
                self.fd = Some(f);
            }
            let Some(fd) = self.fd else { continue };
            // SAFETY: `buf` holds `buf.len()` bytes; this thread is its only
            // writer while the slot is unmapped.
            let seek = unsafe { sys::sceIoLseek(fd, pos, sys::IoWhence::Set) };
            if seek != pos {
                LAST_ERROR.store(seek as i32, Ordering::Relaxed);
                continue;
            }
            let got =
                unsafe { sys::sceIoRead(fd, buf.as_mut_ptr() as *mut c_void, buf.len() as u32) };
            if got as usize == buf.len() {
                return true;
            }
            LAST_ERROR.store(got, Ordering::Relaxed);
        }
        false
    }
}

/// Starts the streamer over `cache`, reading the pack at `path`
/// (NUL-terminated) with the region at file offset `base`. `Err` with the
/// kernel's error.
pub fn start(path: &'static str, base: i64, cache: &'static PageCache) -> Result<(), i32> {
    if THREAD.load(Ordering::Acquire) >= 0 {
        return Ok(());
    }
    let sema = crate::audio::create_sema(b"ssb_audio_pages\0", 0, 0x7FFF_FFFF)?;
    SEMA.store(sema.0, Ordering::Release);
    CACHE.store(
        cache as *const PageCache as *mut PageCache,
        Ordering::Release,
    );
    // SAFETY: `path` is NUL-terminated.
    let fd = unsafe { sys::sceIoOpen(path.as_ptr(), sys::IoOpenFlags::RD_ONLY, 0o777) };
    if fd.0 < 0 {
        return Err(fd.0);
    }
    // SAFETY: written before the thread that reads it starts.
    unsafe { SOURCE = (path, base, fd.0) };
    STOP.store(false, Ordering::Release);
    // SAFETY: a NUL-terminated name and an `extern "C"` entry.
    let id = unsafe {
        sys::sceKernelCreateThread(
            b"ssb_audio_pages\0".as_ptr(),
            thread_main,
            PRIORITY,
            STACK_BYTES,
            sys::ThreadAttributes::USER,
            core::ptr::null_mut(),
        )
    };
    if id.0 < 0 {
        unsafe { sys::sceIoClose(fd) };
        return Err(id.0);
    }
    let r = unsafe { sys::sceKernelStartThread(id, 0, core::ptr::null_mut()) };
    if r < 0 {
        unsafe {
            sys::sceKernelDeleteThread(id);
            sys::sceIoClose(fd);
        }
        return Err(r);
    }
    THREAD.store(id.0, Ordering::Release);
    Ok(())
}

/// Stops the streamer after its current read.
pub fn stop() {
    let id = THREAD.swap(-1, Ordering::AcqRel);
    if id < 0 {
        return;
    }
    STOP.store(true, Ordering::Release);
    wake();
    let mut timeout: u32 = 200_000;
    unsafe {
        sys::sceKernelWaitThreadEnd(SceUid(id), &mut timeout);
        sys::sceKernelDeleteThread(SceUid(id));
    }
}

/// The cache the streamer serves, once started.
pub fn cache() -> Option<&'static PageCache> {
    // SAFETY: leaked by the caller of `start`; never freed.
    unsafe { CACHE.load(Ordering::Acquire).as_ref() }
}

unsafe extern "C" fn thread_main(_args: usize, _argp: *mut c_void) -> i32 {
    let Some(cache) = cache() else {
        return 0;
    };
    // SAFETY: set by `start` before this thread began.
    let (path, base, fd) = unsafe { *core::ptr::addr_of!(SOURCE) };
    let mut src = PackPages {
        path,
        base,
        fd: Some(SceUid(fd)),
    };
    let sema = SceUid(SEMA.load(Ordering::Acquire));
    while !STOP.load(Ordering::Acquire) {
        // SAFETY: the semaphore `start` created; no timeout.
        unsafe { sys::sceKernelWaitSema(sema, 1, core::ptr::null_mut()) };
        while cache.pending() && !STOP.load(Ordering::Acquire) {
            cache.service(&mut src, 1);
        }
    }
    if let Some(f) = src.fd.take() {
        unsafe { sys::sceIoClose(f) };
    }
    0
}

/// Leaks `len` zeroed bytes on whole cache lines for the cache's slots.
pub(crate) fn slots(len: usize) -> Option<&'static mut [u8]> {
    let buf = crate::assets::AlignedBuf::cache_lines(len)?;
    // SAFETY: the buffer is leaked below, so the slice lives forever; it is
    // `len` bytes, written only through this slice.
    let mem = unsafe { core::slice::from_raw_parts_mut(buf.as_mut_ptr(), len) };
    core::mem::forget(buf);
    Some(mem)
}

/// Builds and leaks a cache of `slots` pages over a region of `region_len`
/// bytes.
pub(crate) fn new_cache(region_len: usize, slot_bytes: &'static mut [u8]) -> &'static PageCache {
    Box::leak(Box::new(PageCache::new(region_len, slot_bytes, wake)))
}
