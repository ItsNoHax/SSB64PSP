//! The current thread's stack. The main thread's size is `psp::module!`'s
//! fourth argument (RE-469).

use psp::sys;

/// Bytes of the current thread's stack never written since it started:
/// the kernel fills a new stack with `0xFF` and this counts the untouched
/// bytes above its base (`sceKernelGetThreadStackFreeSize`). PPSSPP
/// implements the same scan.
pub fn stack_free_bytes() -> usize {
    let free = unsafe { sys::sceKernelGetThreadStackFreeSize(sys::SceUid(0)) };
    free.max(0) as usize
}
