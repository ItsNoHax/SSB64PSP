//! Frame timer for measuring the release build on hardware (RE-500),
//! compiled out unless the `frame_timer` feature is on.
//!
//! The `profile` feature times spans inside the draw path, so a profiling
//! build's hot code is not the release build's (RE-496, RE-499). This timer
//! is called from the main loop only: at the frame's end ([`frame_end`]) and
//! around the wait for the last frame to be on screen ([`vblank`]), which
//! the main loop then makes before the draw instead of in
//! `Gpu::begin_frame`. The code it adds is those calls and functions
//! outside the searched link order; its state is a few words.
//!
//! A release-code battle mostly finishes its frame inside one vblank
//! (RE-500), so frames per second say little about the code's speed; the
//! time a frame spends outside that wait, `busy`, does. Every
//! [`REPORT_FRAMES`](crate::profile::REPORT_FRAMES) counted frames it writes
//!
//! `prof n=<report> fps=<frames per second> frame=<avg µs>/<max µs> busy=<avg µs>/<max µs>`
//!
//! to `profile.log` beside the executable and to stdout: the profiler's
//! line prefix and keys, so the same summaries read both. A capture frozen at
//! its tick is not counted. With `profile` on as well the profiler reports
//! and this timer does nothing.

/// Closes a frame. `frozen`: a capture frozen at its tick, not a frame of
/// the game.
#[inline(always)]
pub fn frame_end(frozen: bool) {
    #[cfg(all(feature = "frame_timer", not(feature = "profile")))]
    imp::frame_end(frozen);
    #[cfg(not(all(feature = "frame_timer", not(feature = "profile"))))]
    let _ = frozen;
}

/// Runs `wait`, the wait for the last frame to be on screen
/// (`Gpu::wait_presented`), and keeps its time out of the frame's `busy`.
#[inline(always)]
pub fn vblank(wait: impl FnOnce()) {
    #[cfg(all(feature = "frame_timer", not(feature = "profile")))]
    imp::vblank(wait);
    #[cfg(not(all(feature = "frame_timer", not(feature = "profile"))))]
    wait();
}

#[cfg(all(feature = "frame_timer", not(feature = "profile")))]
mod imp {
    use crate::profile::REPORT_FRAMES;
    use core::fmt::Write;
    use psp::sys;

    struct State {
        /// The last frame's end; 0 before the first.
        last_end: u32,
        frames: u32,
        sum: u32,
        max: u32,
        /// This frame's vblank wait.
        wait: u32,
        busy_sum: u32,
        busy_max: u32,
        reports: u32,
    }

    static mut STATE: State = State {
        last_end: 0,
        frames: 0,
        sum: 0,
        max: 0,
        wait: 0,
        busy_sum: 0,
        busy_max: 0,
        reports: 0,
    };

    struct Line {
        buf: [u8; 112],
        len: usize,
    }

    impl Write for Line {
        fn write_str(&mut self, text: &str) -> core::fmt::Result {
            let n = text.len().min(self.buf.len() - self.len);
            self.buf[self.len..self.len + n].copy_from_slice(&text.as_bytes()[..n]);
            self.len += n;
            Ok(())
        }
    }

    #[inline(never)]
    pub fn vblank(wait: impl FnOnce()) {
        let t = unsafe { sys::sceKernelGetSystemTimeLow() };
        wait();
        let us = unsafe { sys::sceKernelGetSystemTimeLow() }.wrapping_sub(t);
        // SAFETY: the game is single-threaded.
        let s = unsafe { &mut *core::ptr::addr_of_mut!(STATE) };
        s.wait = s.wait.wrapping_add(us);
    }

    #[inline(never)]
    pub fn frame_end(frozen: bool) {
        // A measurement run gets no input: keep the PSP from auto-sleeping
        // under it (RE-481).
        unsafe { sys::scePowerTick(sys::PowerTick::All) };
        // SAFETY: the game is single-threaded.
        let s = unsafe { &mut *core::ptr::addr_of_mut!(STATE) };
        let t = unsafe { sys::sceKernelGetSystemTimeLow() };
        let last = core::mem::replace(&mut s.last_end, t);
        let wait = core::mem::replace(&mut s.wait, 0);
        if last == 0 || frozen {
            return;
        }
        let us = t.wrapping_sub(last);
        let busy = us.saturating_sub(wait);
        s.frames += 1;
        s.sum = s.sum.wrapping_add(us);
        s.max = s.max.max(us);
        s.busy_sum = s.busy_sum.wrapping_add(busy);
        s.busy_max = s.busy_max.max(busy);
        if s.frames < REPORT_FRAMES {
            return;
        }
        s.reports += 1;
        let fps100 = (u64::from(s.frames) * 100_000_000 / u64::from(s.sum.max(1))) as u32;
        let mut line = Line {
            buf: [0; 112],
            len: 0,
        };
        let _ = writeln!(
            line,
            "prof n={} fps={}.{:02} frame={}/{} busy={}/{}",
            s.reports,
            fps100 / 100,
            fps100 % 100,
            s.sum / s.frames,
            s.max,
            s.busy_sum / s.frames,
            s.busy_max
        );
        s.frames = 0;
        s.sum = 0;
        s.max = 0;
        s.busy_sum = 0;
        s.busy_max = 0;
        let bytes = &line.buf[..line.len];
        unsafe {
            sys::sceIoWrite(
                sys::sceKernelStdout(),
                bytes.as_ptr() as *const core::ffi::c_void,
                bytes.len(),
            );
            let fd = sys::sceIoOpen(
                b"profile.log\0".as_ptr(),
                sys::IoOpenFlags::WR_ONLY | sys::IoOpenFlags::CREAT | sys::IoOpenFlags::APPEND,
                0o777,
            );
            if fd.0 >= 0 {
                sys::sceIoWrite(fd, bytes.as_ptr() as *const core::ffi::c_void, bytes.len());
                sys::sceIoClose(fd);
            }
        }
        // The report's own write is not the next frame's time.
        s.last_end = unsafe { sys::sceKernelGetSystemTimeLow() };
    }
}
