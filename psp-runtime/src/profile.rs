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
    /// Inside the interface: the off-screen fighters' magnifiers.
    DrawMagnifiers,
    /// Inside the magnifiers: their depth masks.
    MagnifyMask,
    /// Inside the magnifiers: their fighters' models.
    MagnifyModel,
    /// Reading a scene's archive files (`scene_files`), in whatever span
    /// waited for them (RE-476).
    Io,
    /// Of [`Span::Io`]: files loaded on demand, missing from the scene's
    /// list.
    Demand,
    /// `scene_files::safe_point` after the frame: background reads
    /// completed and registered, the last scene's files dropped. Counted
    /// in the frame's CPU time (RE-476).
    Files,
    /// The opening's movie draw (`movie::Runtime::draw`, RE-476): posing
    /// models and fighters.
    MoviePose,
    /// The movie draw's mesh lists (`meshdraw`), skinning included.
    MovieMesh,
    /// Inside mesh lists: vertices re-posed on the CPU (`draw_node_mesh`).
    Skin,
    /// The movie draw's sprite pieces and fills.
    MovieSprites,
    /// Mesh lists: a node's matrix (`sceGum*`) before its meshes.
    NodeMatrix,
    /// Mesh lists: each primitive's material and texture state.
    Material,
    /// Mesh lists: each primitive's `sceGumDrawArray`.
    Submit,
    /// Mesh lists: primitives drawn (a count, not microseconds).
    Prims,
    /// The game thread's sound calls (`audio::GameAudio`): the lock, its
    /// wait for an audio frame in progress, and the call.
    Sound,
    /// Sound calls made (a count, not microseconds).
    SoundCalls,
}

const SPANS: usize = 34;
#[cfg_attr(not(feature = "profile"), allow(dead_code))]
const NAMES: [&str; SPANS] = [
    "update", "interrupt", "physics", "hit", "effects", "draw", "ge", "vblank", "fphys", "items", "weapons", "camera", "anim",
    "map", "joints", "dstage", "dfighters", "dhud", "dmagnify", "mmask", "mmodel", "io", "demand", "files", "mpose", "mmesh", "skin", "msprites", "nodemat", "material", "submit", "prims", "sound", "sndcalls",
];

/// Frames per report: two seconds at 60 FPS.
pub const REPORT_FRAMES: u32 = 120;

/// A frame whose CPU time (update plus draw, less the vblank wait) passes
/// this is logged on its own line with its tick and spans (RE-471): 12 ms,
/// the average budget RE-470 leaves for the PSP's slower frames.
#[cfg_attr(not(feature = "profile"), allow(dead_code))]
const SPIKE_US: u32 = 12_000;
/// Spike lines a run writes at most.
#[cfg_attr(not(feature = "profile"), allow(dead_code))]
const SPIKE_LINES: u32 = 400;
/// The whole-run CPU-time histogram's bucket width and count: 0.1 ms
/// buckets to 100 ms, the last holding everything slower.
#[cfg_attr(not(feature = "profile"), allow(dead_code))]
const BUCKET_US: u32 = 100;
#[cfg_attr(not(feature = "profile"), allow(dead_code))]
const BUCKETS: usize = 1000;

#[cfg(feature = "profile")]
mod imp {
    use super::{Span, BUCKETS, BUCKET_US, NAMES, REPORT_FRAMES, SPANS, SPIKE_LINES, SPIKE_US};
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
        tick: u32,
        cpu_sum: u32,
        cpu_max: u32,
        cpu_max_tick: u32,
        spikes: u32,
        /// Every frame's CPU time since the start of the run, by
        /// [`BUCKET_US`].
        hist: [u32; BUCKETS],
        hist_frames: u32,
        run_max: u32,
        run_max_tick: u32,
        /// The audio thread's counters at the last report.
        audio_last: crate::audio::Stats,
        /// The renderer's stage totals at the last report.
        stages_last: [u32; ssb_engine::audio::prof::STAGES],
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
        tick: 0,
        cpu_sum: 0,
        cpu_max: 0,
        cpu_max_tick: 0,
        spikes: 0,
        hist: [0; BUCKETS],
        hist_frames: 0,
        run_max: 0,
        run_max_tick: 0,
        audio_last: crate::audio::Stats::ZERO,
        stages_last: [0; ssb_engine::audio::prof::STAGES],
    };

    pub fn set_tick(tick: u32) {
        // SAFETY: the game is single-threaded.
        unsafe { (*core::ptr::addr_of_mut!(STATE)).tick = tick };
    }

    pub fn tick() -> u32 {
        // SAFETY: the game is single-threaded.
        unsafe { (*core::ptr::addr_of!(STATE)).tick }
    }

    /// The CPU time below which `per_mille` of the run's frames fall, in
    /// microseconds (the bucket's upper edge).
    fn percentile(s: &State, per_mille: u32) -> u32 {
        let want = (u64::from(s.hist_frames) * u64::from(per_mille)).div_ceil(1000) as u32;
        let mut seen = 0;
        for (i, &n) in s.hist.iter().enumerate() {
            seen += n;
            if seen >= want.max(1) {
                return (i as u32 + 1) * BUCKET_US;
            }
        }
        BUCKETS as u32 * BUCKET_US
    }

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

    pub fn count(span: Span, n: u32) {
        // SAFETY: the game is single-threaded.
        let s = unsafe { &mut *core::ptr::addr_of_mut!(STATE) };
        s.frame[span as usize] = s.frame[span as usize].wrapping_add(n);
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

    pub fn frame_end(_stack_free: usize) {
        flush();
        let s = unsafe { &mut *core::ptr::addr_of_mut!(STATE) };
        let t = now();
        if s.last_end == 0 {
            s.last_end = t;
            s.report_start = t;
            s.frame = [0; SPANS];
            return;
        }
        if s.tick == u32::MAX {
            // A capture frozen at its tick: not a frame of the game.
            s.last_end = t;
            s.frame = [0; SPANS];
            return;
        }
        let frame_us = t.wrapping_sub(s.last_end);
        s.last_end = t;
        let cpu = (s.frame[Span::Update as usize]
            + s.frame[Span::Draw as usize]
            + s.frame[Span::Files as usize])
            .saturating_sub(s.frame[Span::Vblank as usize]);
        s.cpu_sum = s.cpu_sum.wrapping_add(cpu);
        if cpu > s.cpu_max {
            s.cpu_max = cpu;
            s.cpu_max_tick = s.tick;
        }
        if cpu > s.run_max {
            s.run_max = cpu;
            s.run_max_tick = s.tick;
        }
        s.hist[((cpu / BUCKET_US) as usize).min(BUCKETS - 1)] += 1;
        s.hist_frames += 1;
        if cpu > SPIKE_US && s.spikes < SPIKE_LINES {
            s.spikes += 1;
            let mut line = Line { buf: [0; 768], len: 0 };
            let _ = write!(
                line,
                "spike tick={} scene={} cpu={}",
                s.tick,
                crate::scene_files::report().scene,
                cpu
            );
            for i in 0..SPANS {
                if s.frame[i] >= 200 {
                    let _ = write!(line, " {}={}", NAMES[i], s.frame[i]);
                }
            }
            let _ = writeln!(line);
            emit(&line.buf[..line.len]);
        }
        for i in 0..SPANS {
            s.sum[i] = s.sum[i].wrapping_add(s.frame[i]);
            s.max[i] = s.max[i].max(s.frame[i]);
        }
        s.frame = [0; SPANS];
        s.frames += 1;
        s.frame_sum = s.frame_sum.wrapping_add(frame_us);
        s.frame_max = s.frame_max.max(frame_us);
        s.min_free = s.min_free.min(unsafe { sys::sceKernelTotalFreeMemSize() } as u32);
        if s.frames < REPORT_FRAMES {
            return;
        }
        report(s, t);
    }

    /// Writes the last, partial report.
    pub fn finish() {
        flush();
        let s = unsafe { &mut *core::ptr::addr_of_mut!(STATE) };
        if s.frames > 0 {
            report(s, now());
        }
    }

    fn report(s: &mut State, t: u32) {
        let free = unsafe { sys::sceKernelTotalFreeMemSize() } as u32;
        let stack_free = crate::thread::stack_free_bytes();
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
        let _ = write!(
            line,
            " cpu={}/{}@{} run_p50={} run_p99={} run_max={}@{} run_over12={} run_frames={}",
            s.cpu_sum / n,
            s.cpu_max,
            s.cpu_max_tick,
            percentile(s, 500),
            percentile(s, 990),
            s.run_max,
            s.run_max_tick,
            s.hist[(SPIKE_US / BUCKET_US) as usize..].iter().sum::<u32>(),
            s.hist_frames
        );
        for i in 0..SPANS {
            let _ = write!(line, " {}={}/{}", NAMES[i], s.sum[i] / n, s.max[i]);
        }
        let max_block = unsafe { sys::sceKernelMaxFreeMemSize() } as u32;
        let _ = writeln!(
            line,
            " free={} maxblock={} minfree={} stackfree={} mhz={}/{}",
            free,
            max_block,
            s.min_free,
            stack_free,
            unsafe { sys::scePowerGetCpuClockFrequency() },
            unsafe { sys::scePowerGetBusClockFrequency() }
        );
        emit(&line.buf[..line.len]);
        audio_report(s, wall, n);
        s.sum = [0; SPANS];
        s.max = [0; SPANS];
        s.frames = 0;
        s.frame_sum = 0;
        s.frame_max = 0;
        s.cpu_sum = 0;
        s.cpu_max = 0;
        s.report_start = t;
        s.reports += 1;
    }

    /// The audio thread's line: per-second rates over the window, the
    /// window's worst render and queue, and its stack. `busy` and `per_frame`
    /// are what the audio thread took from the CPU, which the game thread's
    /// wall-time spans above include whenever it preempted them.
    fn audio_report(s: &mut State, wall: u32, frames: u32) {
        let mut line = Line { buf: [0; 768], len: 0 };
        if !crate::audio::running() {
            let _ = writeln!(line, "audio n={} off", s.reports);
            emit(&line.buf[..line.len]);
            return;
        }
        let a = crate::audio::stats();
        let l = s.audio_last;
        let per_s = |d: u32| (u64::from(d) * 10_000_000 / u64::from(wall)) as u32;
        let blk10 = per_s(a.blocks.wrapping_sub(l.blocks));
        let tic10 = per_s(a.tics.wrapping_sub(l.tics));
        let busy = a.busy_us.wrapping_sub(l.busy_us);
        let _ = writeln!(
            line,
            "audio n={} blk/s={}.{} tic/s={}.{} busy_us/s={} per_frame={} max={} under={} under_total={} err={} rest_min={} lat_max={} voices={}/{} steals={} drops={} osc_drops={} stk={} dump_drop={}",
            s.reports,
            blk10 / 10,
            blk10 % 10,
            tic10 / 10,
            tic10 % 10,
            (u64::from(busy) * 1_000_000 / u64::from(wall)) as u32,
            busy / frames.max(1),
            a.max_us,
            a.underruns.wrapping_sub(l.underruns),
            a.underruns,
            a.errors.wrapping_sub(l.errors),
            if a.rest_min == u32::MAX { 0 } else { a.rest_min },
            a.latency_max,
            a.voices,
            a.voices_max,
            a.steals,
            a.drops,
            a.osc_drops,
            crate::audio::stack_free(),
            crate::audio::dump_dropped(),
        );
        emit(&line.buf[..line.len]);
        // Renderer stages in microseconds per second (pulls: voice
        // sub-frames per second; ns_pull: the per-voice stages' cost per
        // pull).
        let st = ssb_engine::audio::prof::read();
        let d: [u32; ssb_engine::audio::prof::STAGES] =
            core::array::from_fn(|i| per_s(st[i].wrapping_sub(s.stages_last[i])) / 10);
        let per_voice = d[2] + d[3] + d[4];
        let mut line = Line { buf: [0; 768], len: 0 };
        let _ = writeln!(
            line,
            "astage n={} seq={} snd={} adpcm={} resample={} envmix={} reverb={} bus={} post={} total={} pulls/s={} ns_pull={}",
            s.reports,
            d[0],
            d[1],
            d[2],
            d[3],
            d[4],
            d[5],
            d[6],
            d[7],
            d[8],
            d[9],
            (u64::from(per_voice) * 1000 / u64::from(d[9].max(1))) as u32,
        );
        emit(&line.buf[..line.len]);
        s.stages_last = st;
        s.audio_last = a;
        crate::audio::new_window();
    }

    /// Lines [`log`] took during a frame, written at its end, outside the
    /// timed spans: a write opens `profile.log` (RE-476).
    static mut PENDING: [u8; 2048] = [0; 2048];
    static mut PENDING_LEN: usize = 0;

    pub fn log(bytes: &[u8]) {
        // SAFETY: the game is single-threaded.
        unsafe {
            let buf = &mut *core::ptr::addr_of_mut!(PENDING);
            let len = &mut *core::ptr::addr_of_mut!(PENDING_LEN);
            if *len + bytes.len() > buf.len() {
                flush();
            }
            if bytes.len() > buf.len() {
                emit(bytes);
                return;
            }
            buf[*len..*len + bytes.len()].copy_from_slice(bytes);
            *len += bytes.len();
        }
    }

    pub fn flush() {
        // SAFETY: the game is single-threaded.
        unsafe {
            let len = &mut *core::ptr::addr_of_mut!(PENDING_LEN);
            if *len > 0 {
                let buf = &*core::ptr::addr_of!(PENDING);
                emit(&buf[..*len]);
                *len = 0;
            }
        }
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

/// Adds `n` to `span`'s share of this frame: a count rather than a time.
#[inline(always)]
pub fn count(span: Span, n: u32) {
    #[cfg(feature = "profile")]
    imp::count(span, n);
    #[cfg(not(feature = "profile"))]
    let _ = (span, n);
}

/// Times `f` as `span`.
#[inline(always)]
pub fn time<R>(span: Span, f: impl FnOnce() -> R) -> R {
    let t = start();
    let r = f();
    stop(span, t);
    r
}

/// Names the simulation tick the frame being timed runs, for the spike
/// lines and the reports' worst-frame ticks; `u32::MAX` leaves the frame
/// out (a capture frozen at its tick).
#[inline(always)]
pub fn set_tick(tick: u32) {
    #[cfg(feature = "profile")]
    imp::set_tick(tick);
    #[cfg(not(feature = "profile"))]
    let _ = tick;
}

/// The tick [`set_tick`] named; 0 without the feature.
#[inline(always)]
pub fn tick() -> u32 {
    #[cfg(feature = "profile")]
    {
        imp::tick()
    }
    #[cfg(not(feature = "profile"))]
    {
        0
    }
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

/// Writes the last, partial report, before a profiling run exits.
#[inline(always)]
pub fn finish() {
    #[cfg(feature = "profile")]
    imp::finish();
}

/// Writes `line` to stdout and `profile.log` with the reports, so a
/// PSPLink run keeps it (RE-469), at the frame's end, outside its timed
/// spans (RE-476). Nothing without the feature.
#[inline(always)]
pub fn log(line: &[u8]) {
    #[cfg(feature = "profile")]
    imp::log(line);
    #[cfg(not(feature = "profile"))]
    let _ = line;
}

/// Whether the profiler is compiled in.
pub const ENABLED: bool = cfg!(feature = "profile");
