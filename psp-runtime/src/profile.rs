//! Frame profiler for real-hardware runs (RE-469), compiled out unless the
//! `profile` feature is on.
//!
//! Each [`Span`] accumulates the microseconds spent in it per frame;
//! [`frame_end`] closes a frame and, every [`REPORT_FRAMES`] frames, writes
//! one line of averages and maxima, frames per second and free memory to
//! `profile.log` beside the executable (`host0:` under PSPLink) and to
//! stdout. Without the feature every function here is an empty inline.

/// A timed part of the frame.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Span {
    /// The screens' logic and the world (`session_frame`).
    Update,
    /// The world's interrupt pass: input, status interrupts, animation.
    Interrupt,
    /// The world's physics pass: movement, collision, map.
    Physics,
    /// The world's hit pass.
    Hit,
    /// The effect players' catch-up before the draw.
    EffectSync,
    /// Building the frame's display list on the CPU. Includes the
    /// [`Span::Vblank`] wait in `Gpu::begin_frame` (RE-470).
    Draw,
    /// `sceGuSync`: waiting for the GE to finish the list after the CPU.
    GeSync,
    /// `Gpu::wait_presented`: waiting for the vblank that shows the last
    /// frame before drawing over the buffer it replaces (RE-470).
    Vblank,
    /// Inside the physics pass: the fighters' own physics, animation and
    /// map collision (`FighterScene::tick_fighter_physics`).
    FighterPhysics,
    /// Inside the physics pass: the items' processes.
    Items,
    /// Inside the physics pass: the weapons' processes.
    Weapons,
    /// Inside the physics pass: the battle camera.
    Camera,
    /// Inside the fighters' physics: the skeleton animation step.
    Anim,
    /// Inside the fighters' physics: movement and map collision.
    Map,
    /// Inside the fighters' physics: the joint transforms gameplay reads.
    Joints,
    /// Inside the draw: a battle's stage passes before the fighters (links
    /// 0-8).
    DrawStage,
    /// Inside the draw: a battle's fighter models.
    DrawFighters,
    /// Inside the draw: a battle's arrows, magnifiers and interface.
    DrawHud,
}

const SPANS: usize = 18;
#[cfg_attr(not(feature = "profile"), allow(dead_code))]
const NAMES: [&str; SPANS] = [
    "update", "interrupt", "physics", "hit", "effects", "draw", "ge", "vblank", "fphys", "items", "weapons", "camera", "anim",
    "map", "joints", "dstage", "dfighters", "dhud",
];

/// Frames per report: two seconds at 60 FPS.
pub const REPORT_FRAMES: u32 = 120;

#[cfg(feature = "profile")]
mod imp {
    use super::{Span, NAMES, REPORT_FRAMES, SPANS};
    use core::fmt::Write;
    use psp::sys;

    struct State {
        frame: [u32; SPANS],
        sum: [u32; SPANS],
        max: [u32; SPANS],
        frames: u32,
        frame_sum: u32,
        frame_max: u32,
        last_end: u32,
        report_start: u32,
        min_free: u32,
        reports: u32,
    }

    static mut STATE: State = State {
        frame: [0; SPANS],
        sum: [0; SPANS],
        max: [0; SPANS],
        frames: 0,
        frame_sum: 0,
        frame_max: 0,
        last_end: 0,
        report_start: 0,
        min_free: u32::MAX,
        reports: 0,
    };

    fn now() -> u32 {
        unsafe { sys::sceKernelGetSystemTimeLow() }
    }

    #[inline(always)]
    pub fn start() -> u32 {
        now()
    }

    pub fn stop(span: Span, start: u32) {
        let us = now().wrapping_sub(start);
        // SAFETY: the game is single-threaded.
        let s = unsafe { &mut *core::ptr::addr_of_mut!(STATE) };
        s.frame[span as usize] = s.frame[span as usize].wrapping_add(us);
    }

    /// A fixed-capacity line buffer, so a report allocates nothing.
    struct Line {
        buf: [u8; 768],
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

    pub fn frame_end(stack_free: usize) {
        let s = unsafe { &mut *core::ptr::addr_of_mut!(STATE) };
        let t = now();
        if s.last_end == 0 {
            s.last_end = t;
            s.report_start = t;
            s.frame = [0; SPANS];
            return;
        }
        let frame_us = t.wrapping_sub(s.last_end);
        s.last_end = t;
        for i in 0..SPANS {
            s.sum[i] = s.sum[i].wrapping_add(s.frame[i]);
            s.max[i] = s.max[i].max(s.frame[i]);
        }
        s.frame = [0; SPANS];
        s.frames += 1;
        s.frame_sum = s.frame_sum.wrapping_add(frame_us);
        s.frame_max = s.frame_max.max(frame_us);
        let free = unsafe { sys::sceKernelTotalFreeMemSize() } as u32;
        s.min_free = s.min_free.min(free);
        if s.frames < REPORT_FRAMES {
            return;
        }
        let wall = t.wrapping_sub(s.report_start).max(1);
        let n = s.frames;
        let mut line = Line { buf: [0; 768], len: 0 };
        let fps10 = (u64::from(n) * 10_000_000 / u64::from(wall)) as u32;
        let _ = write!(
            line,
            "prof n={} fps={}.{} frame={}/{}",
            s.reports,
            fps10 / 10,
            fps10 % 10,
            s.frame_sum / n,
            s.frame_max
        );
        for i in 0..SPANS {
            let _ = write!(line, " {}={}/{}", NAMES[i], s.sum[i] / n, s.max[i]);
        }
        let max_block = unsafe { sys::sceKernelMaxFreeMemSize() } as u32;
        let _ = writeln!(
            line,
            " free={} maxblock={} minfree={} stackfree={}",
            free, max_block, s.min_free, stack_free
        );
        emit(&line.buf[..line.len]);
        s.sum = [0; SPANS];
        s.max = [0; SPANS];
        s.frames = 0;
        s.frame_sum = 0;
        s.frame_max = 0;
        s.report_start = t;
        s.reports += 1;
    }

    pub fn emit(bytes: &[u8]) {
        unsafe {
            sys::sceIoWrite(sys::sceKernelStdout(), bytes.as_ptr() as *const core::ffi::c_void, bytes.len());
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
    }
}

/// The time a span starts, for [`stop`].
#[inline(always)]
pub fn start() -> u32 {
    #[cfg(feature = "profile")]
    {
        imp::start()
    }
    #[cfg(not(feature = "profile"))]
    {
        0
    }
}

/// Adds the time since `start` to `span`'s share of this frame.
#[inline(always)]
pub fn stop(span: Span, start: u32) {
    #[cfg(feature = "profile")]
    imp::stop(span, start);
    #[cfg(not(feature = "profile"))]
    let _ = (span, start);
}

/// Times `f` as `span`.
#[inline(always)]
pub fn time<R>(span: Span, f: impl FnOnce() -> R) -> R {
    let t = start();
    let r = f();
    stop(span, t);
    r
}

/// Ends a frame, reporting every [`REPORT_FRAMES`] frames. `stack_free` is
/// the main thread's untouched stack (`thread::stack_free_bytes`).
#[inline(always)]
pub fn frame_end(stack_free: impl FnOnce() -> usize) {
    #[cfg(feature = "profile")]
    imp::frame_end(stack_free());
    #[cfg(not(feature = "profile"))]
    let _ = stack_free;
}

/// Writes `line` to stdout and `profile.log` with the reports, so a
/// PSPLink run keeps it (RE-469). Nothing without the feature.
#[inline(always)]
pub fn log(line: &[u8]) {
    #[cfg(feature = "profile")]
    imp::emit(line);
    #[cfg(not(feature = "profile"))]
    let _ = line;
}

/// Whether the profiler is compiled in.
pub const ENABLED: bool = cfg!(feature = "profile");
