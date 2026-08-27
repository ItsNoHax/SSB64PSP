//! Loading the asset pack into memory the GE can read directly.
//!
//! The pack is designed so that loading is a single `read()` and nothing else
//! (see `ssb_rom::pack`). The only real constraint is **alignment**: the GE
//! DMAs vertex, index and texel data straight out of this buffer, and
//! unaligned data renders garbage silently rather than failing.
//!
//! Rust's global allocator does not guarantee 16-byte alignment on a 32-bit
//! target, so the buffer is allocated explicitly with the alignment the
//! hardware needs.

use alloc::alloc::{alloc, dealloc, Layout};
use core::slice;

use psp::sys;

use ssb_rom::pack::ALIGN;

/// A heap buffer guaranteed to start on a 16-byte boundary.
pub struct AlignedBuf {
    ptr: *mut u8,
    len: usize,
    layout: Layout,
}

impl AlignedBuf {
    /// Allocates `len` bytes aligned to [`ALIGN`].
    fn new(len: usize) -> Option<AlignedBuf> {
        // Round the size up too: some allocators are happier, and it lets the
        // whole buffer be flushed in whole cache lines.
        let size = len.max(1).div_ceil(ALIGN) * ALIGN;
        let layout = Layout::from_size_align(size, ALIGN).ok()?;
        // SAFETY: layout has non-zero size.
        let ptr = unsafe { alloc(layout) };
        if ptr.is_null() {
            return None;
        }
        Some(AlignedBuf { ptr, len, layout })
    }

    pub fn as_slice(&self) -> &[u8] {
        // SAFETY: `ptr` is valid for `len` bytes and initialised by the read
        // that filled it.
        unsafe { slice::from_raw_parts(self.ptr, self.len) }
    }

    /// Writes the buffer back from the data cache.
    ///
    /// The GE reads system memory directly and does not see the CPU's
    /// writeback cache. Skipping this produces intermittent corruption that
    /// looks like a race condition.
    pub fn flush_cache(&self) {
        unsafe {
            sys::sceKernelDcacheWritebackRange(
                self.ptr as *const core::ffi::c_void,
                self.len as u32,
            )
        }
    }
}

impl Drop for AlignedBuf {
    fn drop(&mut self) {
        // SAFETY: allocated by `alloc` with this exact layout.
        unsafe { dealloc(self.ptr, self.layout) }
    }
}

/// Why loading failed. Kept concrete so the on-screen message is useful when
/// there is no debugger attached.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadError {
    NotFound,
    Empty,
    OutOfMemory,
    ShortRead,
}

impl LoadError {
    pub fn as_str(self) -> &'static str {
        match self {
            LoadError::NotFound => "pack not found",
            LoadError::Empty => "pack is empty",
            LoadError::OutOfMemory => "out of memory",
            LoadError::ShortRead => "short read",
        }
    }
}

/// Paths tried, in order.
///
/// A loose EBOOT run from PPSSPP resolves relative paths against its own
/// directory; an installed game lives under `ms0:/PSP/GAME`. Trying both means
/// the same build works in the emulator and on a real memory stick.
const SEARCH_PATHS: &[&str] = &[
    "ssb64.pak\0",
    "ms0:/PSP/GAME/ssb64/ssb64.pak\0",
    "ms0:/ssb64.pak\0",
];

/// Human-readable form of a search path, without the C terminator.
fn display_path(p: &'static str) -> &'static str {
    p.trim_end_matches('\0')
}

/// Loads the asset pack, returning the buffer and which path worked.
pub fn load_pack() -> Result<(AlignedBuf, &'static str), LoadError> {
    for path in SEARCH_PATHS {
        // SAFETY: path is a NUL-terminated literal.
        let fd = unsafe { sys::sceIoOpen(path.as_ptr(), sys::IoOpenFlags::RD_ONLY, 0o777) };
        if fd.0 < 0 {
            continue;
        }

        let size = unsafe { sys::sceIoLseek(fd, 0, sys::IoWhence::End) };
        unsafe { sys::sceIoLseek(fd, 0, sys::IoWhence::Set) };
        if size <= 0 {
            unsafe { sys::sceIoClose(fd) };
            return Err(LoadError::Empty);
        }

        let Some(buf) = AlignedBuf::new(size as usize) else {
            unsafe { sys::sceIoClose(fd) };
            return Err(LoadError::OutOfMemory);
        };

        let read = unsafe { sys::sceIoRead(fd, buf.ptr as *mut core::ffi::c_void, size as u32) };
        unsafe { sys::sceIoClose(fd) };

        if read as i64 != size {
            return Err(LoadError::ShortRead);
        }

        // The GE will read this memory; make sure it is actually in RAM.
        buf.flush_cache();
        return Ok((buf, display_path(path)));
    }
    Err(LoadError::NotFound)
}
