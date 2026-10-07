//! The C memory functions every large move and zeroing calls, word at a
//! time (`ssb_engine::memops`, RE-471). The `psp` crate defines `memcpy`,
//! `memmove` and `memset` a byte per loop iteration; each PSP binary's
//! `build.rs` links with `--wrap` for the three, so every call, the
//! compiler's own included, comes here instead.

/// `memcpy`.
///
/// # Safety
/// The C contract: valid, non-overlapping ranges.
#[no_mangle]
pub unsafe extern "C" fn __wrap_memcpy(dst: *mut u8, src: *const u8, n: usize) -> *mut u8 {
    ssb_engine::memops::copy_forward(dst, src, n);
    dst
}

/// `memmove`.
///
/// # Safety
/// The C contract: valid ranges.
#[no_mangle]
pub unsafe extern "C" fn __wrap_memmove(dst: *mut u8, src: *const u8, n: usize) -> *mut u8 {
    ssb_engine::memops::move_bytes(dst, src, n);
    dst
}

/// `memset`.
///
/// # Safety
/// The C contract: a valid range.
#[no_mangle]
pub unsafe extern "C" fn __wrap_memset(dst: *mut u8, value: i32, n: usize) -> *mut u8 {
    ssb_engine::memops::fill(dst, value as u8, n);
    dst
}
