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

/// The PSP's data-cache line.
const CACHE_LINE: usize = 64;

/// A heap buffer guaranteed to start on a 16-byte boundary.
pub struct AlignedBuf {
    ptr: *mut u8,
    len: usize,
    layout: Layout,
}

impl AlignedBuf {
    /// Allocates `len` bytes aligned to [`ALIGN`].
    pub(crate) fn new(len: usize) -> Option<AlignedBuf> {
        Self::with_align(len, ALIGN)
    }

    /// Allocates `len` bytes on whole 64-byte data-cache lines: no other
    /// allocation shares a line with it, so a line another owner dirties
    /// while a device writes this buffer cannot be written back over it
    /// (RE-476).
    pub(crate) fn cache_lines(len: usize) -> Option<AlignedBuf> {
        Self::with_align(len, CACHE_LINE)
    }

    fn with_align(len: usize, align: usize) -> Option<AlignedBuf> {
        // Round the size up too: some allocators are happier, and it lets the
        // whole buffer be flushed in whole cache lines.
        let size = len.max(1).div_ceil(align) * align;
        let layout = Layout::from_size_align(size, align).ok()?;
        // SAFETY: layout has non-zero size.
        let ptr = unsafe { alloc(layout) };
        if ptr.is_null() {
            return None;
        }
        Some(AlignedBuf { ptr, len, layout })
    }

    pub fn as_ptr(&self) -> *const u8 {
        self.ptr
    }

    pub(crate) fn as_mut_ptr(&self) -> *mut u8 {
        self.ptr
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

/// Where [`read_capture_scene`] looks for `capture_scene.txt`: the same
/// three locations, in the same order, as the pack.
const SCENE_SEARCH_PATHS: &[&str] = &[
    "capture_scene.txt\0",
    "ms0:/PSP/GAME/ssb64/capture_scene.txt\0",
    "ms0:/capture_scene.txt\0",
];

/// Reads the golden-capture scene file into `buf`, returning the bytes read.
/// `None` when no file exists or it is empty. A file longer than `buf` is
/// truncated, which the spec parser then rejects or reads as its first line.
pub fn read_capture_scene(buf: &mut [u8]) -> Option<usize> {
    for path in SCENE_SEARCH_PATHS {
        // SAFETY: path is a NUL-terminated literal.
        let fd = unsafe { sys::sceIoOpen(path.as_ptr(), sys::IoOpenFlags::RD_ONLY, 0o777) };
        if fd.0 < 0 {
            continue;
        }
        let read = unsafe {
            sys::sceIoRead(
                fd,
                buf.as_mut_ptr() as *mut core::ffi::c_void,
                buf.len() as u32,
            )
        };
        unsafe { sys::sceIoClose(fd) };
        return (read > 0).then_some(read as usize);
    }
    None
}

/// The NUL-terminated search path [`load_pack`] reported as `display`.
pub fn c_path(display: &str) -> Option<&'static str> {
    SEARCH_PATHS.iter().copied().find(|p| display_path(p) == display)
}

/// Human-readable form of a search path, without the C terminator.
fn display_path(p: &'static str) -> &'static str {
    p.trim_end_matches('\0')
}

/// Loads the asset pack's resident part (RE-475): its header, tables and
/// shared blob region. The archive files after them load per scene
/// ([`crate::scene_files`]). Returns the buffer and which path worked.
///
/// A pack of another version reads only its header, which
/// `Pack::open` then rejects.
pub fn load_pack() -> Result<(AlignedBuf, &'static str), LoadError> {
    for path in SEARCH_PATHS {
        // SAFETY: path is a NUL-terminated literal.
        let fd = unsafe { sys::sceIoOpen(path.as_ptr(), sys::IoOpenFlags::RD_ONLY, 0o777) };
        if fd.0 < 0 {
            continue;
        }

        let mut size = unsafe { sys::sceIoLseek(fd, 0, sys::IoWhence::End) };
        unsafe { sys::sceIoLseek(fd, 0, sys::IoWhence::Set) };
        if size <= 0 {
            unsafe { sys::sceIoClose(fd) };
            return Err(LoadError::Empty);
        }
        let mut head = [0u8; ssb_rom::pack::Header::SIZE];
        let n = unsafe { sys::sceIoRead(fd, head.as_mut_ptr() as *mut core::ffi::c_void, head.len() as u32) };
        unsafe { sys::sceIoLseek(fd, 0, sys::IoWhence::Set) };
        if n as usize != head.len() {
            unsafe { sys::sceIoClose(fd) };
            return Err(LoadError::ShortRead);
        }
        let resident = ssb_rom::pack::resident_len(&head).unwrap_or(head.len());
        size = size.min(resident as i64);

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
