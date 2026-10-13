//! The global allocator (RE-484): rust-psp's `external-allocator` feature
//! forwards every allocation to the `__psp_external_*` functions here.
//!
//! rust-psp's `SystemAlloc` makes every allocation and free a kernel call,
//! ~9,400 cycles an allocate-and-free on a PSP-2000 that also evicts the
//! caller's cache lines (RE-483). Here blocks up to
//! [`ssb_engine::heap::MAX_SMALL`] bytes come from
//! [`ssb_engine::heap::SmallHeap`]'s free lists over [`CHUNK_BYTES`] chunks
//! taken from `SystemAlloc` once and kept; larger or more aligned blocks
//! still go to the kernel, so the scenes' file loads and their failures
//! (`memory::fatal`) behave as before.
//!
//! The audio thread allocates too. A short lock guards the free lists; a
//! thread that finds it held sleeps a microsecond, so a higher-priority
//! thread never spins on a holder it has preempted.

use core::alloc::{GlobalAlloc, Layout};
use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use psp::{sys, SystemAlloc};
use ssb_engine::heap::{class_of, SmallHeap, Stats};

/// Bytes taken from the kernel at a time for small blocks.
pub const CHUNK_BYTES: usize = 256 * 1024;

static LOCK: AtomicBool = AtomicBool::new(false);
static mut SMALL: SmallHeap = SmallHeap::new();

/// Allocations since boot, all and the kernel's.
static ALLOCS: AtomicU32 = AtomicU32::new(0);
static KERNEL_ALLOCS: AtomicU32 = AtomicU32::new(0);

fn with_small<R>(f: impl FnOnce(&mut SmallHeap) -> R) -> R {
    while LOCK
        .compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed)
        .is_err()
    {
        // SAFETY: a plain syscall; lets the holder run.
        unsafe { sys::sceKernelDelayThread(1) };
    }
    // SAFETY: the lock gives this thread the only reference.
    let r = f(unsafe { &mut *core::ptr::addr_of_mut!(SMALL) });
    LOCK.store(false, Ordering::Release);
    r
}

fn chunk(min: usize) -> Option<(*mut u8, usize)> {
    let len = CHUNK_BYTES.max(min);
    KERNEL_ALLOCS.fetch_add(1, Ordering::Relaxed);
    // SAFETY: a valid layout; the chunk is never freed.
    let p = unsafe { SystemAlloc.alloc(Layout::from_size_align(len, 64).ok()?) };
    (!p.is_null()).then_some((p, len))
}

#[no_mangle]
unsafe fn __psp_external_alloc(size: usize, align: usize) -> *mut u8 {
    ALLOCS.fetch_add(1, Ordering::Relaxed);
    match class_of(size, align) {
        Some(c) => with_small(|h| h.alloc(c, chunk)),
        None => {
            KERNEL_ALLOCS.fetch_add(1, Ordering::Relaxed);
            // SAFETY: rust-psp forwards a valid layout.
            SystemAlloc.alloc(Layout::from_size_align_unchecked(size, align))
        }
    }
}

#[no_mangle]
unsafe fn __psp_external_dealloc(ptr: *mut u8, size: usize, align: usize) {
    match class_of(size, align) {
        Some(c) => with_small(|h| h.free(ptr, c)),
        None => SystemAlloc.dealloc(ptr, Layout::from_size_align_unchecked(size, align)),
    }
}

#[no_mangle]
unsafe fn __psp_external_realloc(
    ptr: *mut u8,
    size: usize,
    align: usize,
    new_size: usize,
) -> *mut u8 {
    let old = class_of(size, align);
    if old.is_some() && old == class_of(new_size, align) {
        return ptr;
    }
    let p = __psp_external_alloc(new_size, align);
    if !p.is_null() {
        core::ptr::copy_nonoverlapping(ptr, p, size.min(new_size));
        __psp_external_dealloc(ptr, size, align);
    }
    p
}

/// `(allocations since boot, kernel allocations since boot)`.
pub fn counts() -> (u32, u32) {
    (
        ALLOCS.load(Ordering::Relaxed),
        KERNEL_ALLOCS.load(Ordering::Relaxed),
    )
}

/// The small-block heap's counters.
pub fn stats() -> Stats {
    with_small(|h| h.stats())
}
