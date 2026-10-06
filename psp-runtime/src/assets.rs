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

/// `ssb_rom::menu_pack::FILE_NAME` beside each of [`SEARCH_PATHS`].
const MENU_PATHS: &[(&str, &str)] = &[
    ("ssb64.pak", "ssb64-menus.pak\0"),
    ("ms0:/PSP/GAME/ssb64/ssb64.pak", "ms0:/PSP/GAME/ssb64/ssb64-menus.pak\0"),
    ("ms0:/ssb64.pak", "ms0:/ssb64-menus.pak\0"),
];

/// One options or data menu scene's sprite pack from `ssb64-menus.pak`
/// beside the pack at `pack_path` (`ssb_rom::menu_pack`): the index, then
/// only that scene's bytes. The caller drops the buffer when the scene
/// ends, as `lbRelocInitSetup` drops the original's files.
pub fn load_menu_pack(pack_path: &str, scene: ssb_rom::menu_pack::MenuScene) -> Result<AlignedBuf, LoadError> {
    let path = MENU_PATHS
        .iter()
        .find(|(pack, _)| *pack == pack_path)
        .map_or(MENU_PATHS[0].1, |(_, menus)| *menus);
    // SAFETY: the path is a NUL-terminated literal.
    let fd = unsafe { sys::sceIoOpen(path.as_ptr(), sys::IoOpenFlags::RD_ONLY, 0o777) };
    if fd.0 < 0 {
        return Err(LoadError::NotFound);
    }
    let result = (|| {
        let mut index = [0u8; 256];
        let n = ssb_rom::menu_pack::index_len(ssb_rom::menu_pack::MenuScene::ALL.len());
        let read = unsafe { sys::sceIoRead(fd, index.as_mut_ptr() as *mut core::ffi::c_void, n as u32) };
        if read as usize != n {
            return Err(LoadError::ShortRead);
        }
        let (at, len) = ssb_rom::menu_pack::locate(&index[..n], scene).ok_or(LoadError::Empty)?;
        if len == 0 {
            return Err(LoadError::Empty);
        }
        let buf = AlignedBuf::new(len as usize).ok_or(LoadError::OutOfMemory)?;
        unsafe { sys::sceIoLseek(fd, i64::from(at), sys::IoWhence::Set) };
        let read = unsafe { sys::sceIoRead(fd, buf.ptr as *mut core::ffi::c_void, len) };
        if read as i64 != i64::from(len) {
            return Err(LoadError::ShortRead);
        }
        buf.flush_cache();
        Ok(buf)
    })();
    unsafe { sys::sceIoClose(fd) };
    result
}
