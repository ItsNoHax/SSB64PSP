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
    0x5ce6_7f15_a071_11f1,
    0x3e70_06a2_223c_7aa0,
    0x73ea_c1cc_68ee_83f5,
    0xdb9d_797b_e79b_0a13,
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

struct Runner<C: FnMut() -> u32> {
    sys: AudioSystem,
    clock: C,
    hash: u64,
    ticks: u32,
    frames: u32,
    max_voices: u32,
    buf: [i16; FRAME_SAMPLES_MAX * 2],
}

impl<C: FnMut() -> u32> Runner<C> {
    fn frame(&mut self, n: usize) {
        let t = (self.clock)();
        self.sys.render_frame(&mut self.buf, n);
        self.ticks = self.ticks.wrapping_add((self.clock)().wrapping_sub(t));
        self.frames += 1;
        for s in &self.buf[..n * 2] {
            self.hash = (self.hash ^ (*s as u16 as u64)).wrapping_mul(0x0100_0000_01B3);
        }
        self.max_voices = self.max_voices.max(self.sys.active_voices());
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
    let mut r = Report {
        hashes: [0; SEGMENTS],
        ticks: [0; SEGMENTS],
        frames: [0; SEGMENTS],
        max_voices: 0,
    };
    let mut x = Runner {
        sys: AudioSystem::new(section)?,
        clock,
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
    Ok(r)
}
