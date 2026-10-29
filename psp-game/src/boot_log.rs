//! Boot and scene-load log for launches without PSPLink (`boot_log`
//! feature, RE-475).
//!
//! Appends lines to `ms0:/PSP/GAME/ssb64/boot.log`. Each line carries the
//! system time in microseconds and the free user memory. Formats into fixed
//! buffers: it allocates nothing, so it still writes when the heap is
//! exhausted. Compiled to nothing without the feature.
//!
//! Until [`flush`] (frame 60, or a `memory::fatal` screen) the lines stay
//! in RAM: a file open, append and close costs about 16 ms on a PSP-2000
//! stick and creating the file 0.34 s, which made the boot read about
//! 0.4 s slower than it is. A hang before the flush loses the held lines.
//! After it, each line is appended at once, opening and closing the file
//! so a hang or crash keeps every earlier line. Game thread only.

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
    let bytes = &line.buf[..end + 1];
    // SAFETY: game thread only (module docs); no other reference to `HELD`
    // is live.
    let held = unsafe { &mut *core::ptr::addr_of_mut!(HELD) };
    if !held.flushed {
        if held.len + bytes.len() <= held.data.len() {
            held.data[held.len..held.len + bytes.len()].copy_from_slice(bytes);
            held.len += bytes.len();
            return;
        }
        // Full: write what is held, then this line, as after a flush.
        flush();
    }
    write_file(bytes);
}

/// The lines held until [`flush`].
#[cfg(feature = "boot_log")]
struct Held {
    data: [u8; HELD_BYTES],
    len: usize,
    flushed: bool,
}

/// A boot's lines up to frame 60 take about 2 KiB.
#[cfg(feature = "boot_log")]
const HELD_BYTES: usize = 8 * 1024;

#[cfg(feature = "boot_log")]
static mut HELD: Held = Held {
    data: [0; HELD_BYTES],
    len: 0,
    flushed: false,
};

/// Writes the held lines in one append; every later line is appended at
/// once. Runs once; later calls do nothing.
#[cfg(feature = "boot_log")]
pub fn flush() {
    // SAFETY: as in `log_args`.
    let held = unsafe { &mut *core::ptr::addr_of_mut!(HELD) };
    if held.flushed {
        return;
    }
    held.flushed = true;
    write_file(&held.data[..held.len]);
}

#[cfg(not(feature = "boot_log"))]
#[inline(always)]
pub fn flush() {}

/// One append to the log file.
#[cfg(feature = "boot_log")]
fn write_file(bytes: &[u8]) {
    use psp::sys;
    let path = b"ms0:/PSP/GAME/ssb64/boot.log\0";
    // SAFETY: the path is NUL-terminated; the buffer outlives the write.
    unsafe {
        let fd = sys::sceIoOpen(
            path.as_ptr(),
            sys::IoOpenFlags::WR_ONLY | sys::IoOpenFlags::CREAT | sys::IoOpenFlags::APPEND,
            0o777,
        );
        if fd.0 >= 0 {
            sys::sceIoWrite(fd, bytes.as_ptr() as *const core::ffi::c_void, bytes.len());
            sys::sceIoClose(fd);
        }
    }
}

#[cfg(not(feature = "boot_log"))]
#[inline(always)]
pub fn log_args(_args: core::fmt::Arguments<'_>) {}
