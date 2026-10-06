//! The save file: `lbBackupWrite`'s SRAM image (`ssb_game::backup`) kept as
//! a plain memory-stick file beside the asset pack (D-045).
//!
//! The file holds the N64's two copies at their SRAM offsets. A write opens
//! the file without truncating it and writes the first copy, then the
//! second, as `lbBackupWrite`'s two DMAs do, so a write cut short leaves the
//! other copy for `lbBackupIsSramValid` to fall back on.

use psp::sys;
use ssb_game::backup::{self, Backup, Loaded, COPY_OFFSET, IMAGE_SIZE, SIZE};

/// The save beside each of `assets`' pack paths, in the same order.
const PATHS: [(&str, &str); 3] = [
    ("ssb64.pak", "ssb64.sav\0"),
    ("ms0:/PSP/GAME/ssb64/ssb64.pak", "ms0:/PSP/GAME/ssb64/ssb64.sav\0"),
    ("ms0:/ssb64.pak", "ms0:/ssb64.sav\0"),
];

/// The save file beside the pack [`crate::assets::load_pack`] found.
pub fn path_for(pack_path: &str) -> &'static str {
    PATHS
        .iter()
        .find(|(pack, _)| *pack == pack_path)
        .map_or(PATHS[0].1, |(_, save)| save)
}

/// `lbBackupIsSramValid` over the file at `path`: a missing or short file
/// reads as invalid copies, which fall back to the defaults. The caller
/// saves when the returned backup's write count says the source wrote.
pub fn load(path: &'static str) -> (Backup, Loaded) {
    let mut image = [0u8; IMAGE_SIZE];
    // SAFETY: path is a NUL-terminated literal.
    let fd = unsafe { sys::sceIoOpen(path.as_ptr(), sys::IoOpenFlags::RD_ONLY, 0o777) };
    if fd.0 < 0 {
        return backup::load(None);
    }
    let read = unsafe {
        sys::sceIoRead(
            fd,
            image.as_mut_ptr() as *mut core::ffi::c_void,
            IMAGE_SIZE as _,
        )
    };
    unsafe { sys::sceIoClose(fd) };
    let len = usize::try_from(read).unwrap_or(0).min(IMAGE_SIZE);
    backup::load(Some(&image[..len]))
}

/// Why [`save`] failed: the `sceIo*` error code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SaveError(pub i32);

/// `lbBackupWrite`: both copies of `b`, the first then the second.
pub fn save(path: &'static str, b: &Backup) -> Result<(), SaveError> {
    let copy = b.encode();
    let flags = sys::IoOpenFlags::WR_ONLY | sys::IoOpenFlags::CREAT;
    // SAFETY: path is a NUL-terminated literal.
    let fd = unsafe { sys::sceIoOpen(path.as_ptr(), flags, 0o777) };
    if fd.0 < 0 {
        return Err(SaveError(fd.0));
    }
    let mut result = Ok(());
    for offset in [0, COPY_OFFSET] {
        let at = unsafe { sys::sceIoLseek(fd, offset as i64, sys::IoWhence::Set) };
        let wrote = unsafe {
            sys::sceIoWrite(fd, copy.as_ptr() as *const core::ffi::c_void, SIZE as _)
        };
        if at != offset as i64 || wrote != SIZE as i32 {
            result = Err(SaveError(if wrote < 0 { wrote } else { -1 }));
            break;
        }
    }
    unsafe { sys::sceIoClose(fd) };
    result
}
