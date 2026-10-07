//! Boot and scene-load log for launches without PSPLink (`boot_log`
//! feature, RE-475).
//!
//! Appends one line per call to `ms0:/PSP/GAME/ssb64/boot.log`, opening and
//! closing the file each time so a hang or crash keeps every earlier line.
//! Each line carries the system time in microseconds and the free user
//! memory. Formats into a fixed buffer: it allocates nothing, so it still
//! writes when the heap is exhausted. Compiled to nothing without the
//! feature.

/// Logs `stage`.
#[inline(always)]
pub fn log(stage: &str) {
    log_args(format_args!("{stage}"));
}

/// Logs a formatted line.
#[cfg(feature = "boot_log")]
pub fn log_args(args: core::fmt::Arguments<'_>) {
    use core::fmt::Write;
    use psp::sys;

    struct Line {
        buf: [u8; 192],
        len: usize,
    }
    impl Write for Line {
        fn write_str(&mut self, s: &str) -> core::fmt::Result {
            let n = s.len().min(self.buf.len() - self.len);
            self.buf[self.len..self.len + n].copy_from_slice(&s.as_bytes()[..n]);
            self.len += n;
            Ok(())
        }
    }
    let mut line = Line {
        buf: [0; 192],
        len: 0,
    };
    let _ = write!(
        line,
        "{} us={} free={} max_block={}",
        args,
        unsafe { sys::sceKernelGetSystemTimeLow() },
        ssb_psp_runtime::memory::free(),
        ssb_psp_runtime::memory::max_block(),
    );
    let end = line.len.min(line.buf.len() - 1);
    line.buf[end] = b'\n';
    let path = b"ms0:/PSP/GAME/ssb64/boot.log\0";
    // SAFETY: the path is NUL-terminated; the buffer outlives the write.
    unsafe {
        let fd = sys::sceIoOpen(
            path.as_ptr(),
            sys::IoOpenFlags::WR_ONLY | sys::IoOpenFlags::CREAT | sys::IoOpenFlags::APPEND,
            0o777,
        );
        if fd.0 >= 0 {
            sys::sceIoWrite(fd, line.buf.as_ptr() as *const core::ffi::c_void, end + 1);
            sys::sceIoClose(fd);
        }
    }
}

#[cfg(not(feature = "boot_log"))]
#[inline(always)]
pub fn log_args(_args: core::fmt::Arguments<'_>) {}
