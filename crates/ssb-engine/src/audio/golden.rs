//! A fixed render scenario whose PCM hashes lock the renderer's output
//! ([`HASHES`]): the host test checks it, and a platform can run it on the
//! target to prove its build renders the same samples (and to time it).
//!
//! Segments: BGM 33 alone (10 s; sample-exact against the N64); BGM 37 with
//! 8 FGMs (10 s); 16 FGMs at once over BGM 37 (voice steals, all 16
//! voices) with 368-sample frames, a volume fade and the mono setting; the
//! tail after stopping everything.

use super::{AudioError, AudioSystem, FRAME_SAMPLES_MAX};

/// The scenario's per-segment FNV-1a hashes of the interleaved samples.
pub const HASHES: [u64; SEGMENTS] = [
    0xad7d_f1bb_282d_b6d7,
    0x6cc4_89cd_fa30_cd64,
    0x9329_7e57_4474_d5f5,
    0x2ec2_8838_c926_ef35,
];

/// Number of segments.
pub const SEGMENTS: usize = 4;

/// What [`run`] measured.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Report {
    pub hashes: [u64; SEGMENTS],
    /// Clock ticks each segment's renders took.
    pub ticks: [u32; SEGMENTS],
    /// Audio frames rendered per segment.
    pub frames: [u32; SEGMENTS],
    /// Most voices playing after any frame.
    pub max_voices: u32,
}

impl Report {
    /// Whether every hash matches [`HASHES`].
    pub fn ok(&self) -> bool {
        self.hashes == HASHES
    }
}

struct Runner<'a, C: FnMut() -> u32, H: FnMut(), R: FnMut(&mut AudioSystem, &mut [i16], usize)> {
    sys: &'a mut AudioSystem,
    clock: C,
    /// Called after every frame (a host test's streamer).
    between: H,
    /// Renders one frame ([`AudioSystem::render_frame`] unless
    /// [`run_on_by`]).
    render: R,
    hash: u64,
    ticks: u32,
    frames: u32,
    max_voices: u32,
    buf: [i16; FRAME_SAMPLES_MAX * 2],
}

impl<C: FnMut() -> u32, H: FnMut(), R: FnMut(&mut AudioSystem, &mut [i16], usize)>
    Runner<'_, C, H, R>
{
    fn frame(&mut self, n: usize) {
        let t = (self.clock)();
        (self.render)(self.sys, &mut self.buf, n);
        self.ticks = self.ticks.wrapping_add((self.clock)().wrapping_sub(t));
        self.frames += 1;
        for s in &self.buf[..n * 2] {
            self.hash = (self.hash ^ (*s as u16 as u64)).wrapping_mul(0x0100_0000_01B3);
        }
        self.max_voices = self.max_voices.max(self.sys.active_voices());
        (self.between)();
    }

    fn end(&mut self, r: &mut Report, k: usize) {
        r.hashes[k] = self.hash;
        r.ticks[k] = self.ticks;
        r.frames[k] = self.frames;
        self.hash = 0xCBF2_9CE4_8422_2325;
        self.ticks = 0;
        self.frames = 0;
    }
}

/// Runs the scenario on a fresh [`AudioSystem`] over `section`, timing each
/// render with `clock`.
pub fn run(section: &'static [u8], clock: impl FnMut() -> u32) -> Result<Report, AudioError> {
    Ok(run_with(AudioSystem::new(section)?, clock, || {}))
}

/// Runs the scenario on `sys` (fresh), calling `between` after every frame.
pub fn run_with(mut sys: AudioSystem, clock: impl FnMut() -> u32, between: impl FnMut()) -> Report {
    run_on(&mut sys, clock, between)
}

/// [`run_with`] on a borrowed system, which the caller can inspect after.
pub fn run_on(sys: &mut AudioSystem, clock: impl FnMut() -> u32, between: impl FnMut()) -> Report {
    run_on_by(sys, clock, between, |sys, out, n| sys.render_frame(out, n))
}

/// [`run_on`] with each frame rendered by `render(sys, out, samples)`,
/// which must leave what [`AudioSystem::render_frame`] would in
/// `out[..samples * 2]` (an executor on another processor).
pub fn run_on_by(
    sys: &mut AudioSystem,
    clock: impl FnMut() -> u32,
    between: impl FnMut(),
    render: impl FnMut(&mut AudioSystem, &mut [i16], usize),
) -> Report {
    let mut r = Report {
        hashes: [0; SEGMENTS],
        ticks: [0; SEGMENTS],
        frames: [0; SEGMENTS],
        max_voices: 0,
    };
    let mut x = Runner {
        sys,
        clock,
        between,
        render,
        hash: 0xCBF2_9CE4_8422_2325,
        ticks: 0,
        frames: 0,
        max_voices: 0,
        buf: [0; FRAME_SAMPLES_MAX * 2],
    };
    x.sys.play_bgm(0, 33);
    for _ in 0..580 {
        x.frame(FRAME_SAMPLES_MAX);
    }
    x.end(&mut r, 0);

    x.sys.play_bgm(0, 37);
    const FGMS: [(u32, u16); 8] = [
        (40, 0),
        (90, 1),
        (130, 2),
        (200, 3),
        (260, 30),
        (330, 60),
        (400, 100),
        (470, 150),
    ];
    for f in 0..580 {
        for &(at, id) in &FGMS {
            if f == at {
                x.sys.play_fgm(id);
            }
        }
        x.frame(FRAME_SAMPLES_MAX);
    }
    x.end(&mut r, 1);

    x.max_voices = 0;
    for f in 0..400u32 {
        if f < 64 && f % 4 == 0 {
            x.sys.play_fgm((f * 7 % 200) as u16);
        }
        if f == 100 {
            x.sys.set_bgm_volume_fade(0, 8000, 60);
        }
        if f == 200 {
            x.sys.set_quality(0);
        }
        if f == 300 {
            x.sys.set_quality(1);
        }
        let n = x.sys.frame_samples(if f % 5 == 0 { 400 } else { 0 });
        x.frame(n);
    }
    r.max_voices = x.max_voices;
    x.end(&mut r, 2);

    x.sys.stop_bgm_all();
    x.sys.stop_all_fgm();
    for _ in 0..120 {
        x.frame(FRAME_SAMPLES_MAX);
    }
    x.end(&mut r, 3);
    r
}

/// A [`run_on_by`] renderer that records each frame's ops into `list`
/// and runs the whole list at the frame's end, as an executor on another
/// processor does: the output poisoned first, so the list must write
/// every sample.
#[cfg(test)]
pub(crate) fn via_frame_list(
    list: &mut super::exec::FrameList,
) -> impl FnMut(&mut AudioSystem, &mut [i16], usize) + '_ {
    use super::data::AudioData;
    use super::exec::{Exec, FrameList, OpList};
    struct Rec<'a>(&'a mut FrameList);
    impl Exec for Rec<'_> {
        fn run(&mut self, ops: &OpList, _: &AudioData, _: &mut [i16]) {
            assert!(self.0.append(ops), "a frame fits the frame list");
        }
    }
    move |sys, out, n| {
        sys.apply_pending_fx();
        list.clear();
        let out = &mut out[..n * 2];
        out.fill(0x5A5A);
        sys.render_frame_via(out, n, Some(&mut Rec(list)));
        sys.exec_frame_list(list, out);
        sys.mono_pass(out);
    }
}
