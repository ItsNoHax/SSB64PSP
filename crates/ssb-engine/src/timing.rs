//! The game clock.
//!
//! Smash 64 is a fixed-60 Hz simulation: every physics constant, animation
//! length, hitlag duration and hitstun count in the decompilation is expressed
//! in frames at that rate. Decoupling the tick from the display is therefore
//! not an optimisation, it is a correctness requirement — a fighter's jump arc
//! must not change because the PSP dropped a frame.
//!
//! The PSP's display runs at ~59.94 Hz, close enough that in the steady state
//! we tick once per vblank. [`FixedClock`] exists for the cases that are not
//! steady state: a slow frame must produce catch-up ticks, not a slow-motion
//! game.

/// The simulation rate the original game runs at.
pub const TICK_HZ: u32 = 60;

/// Duration of one simulation tick, in microseconds.
pub const TICK_US: u64 = 1_000_000 / TICK_HZ as u64;

/// Upper bound on catch-up ticks per frame.
///
/// Without this, a long stall (loading, say) produces a burst of ticks that
/// takes even longer to simulate, which produces more catch-up — the classic
/// spiral of death. Past this point we accept that the game clock slips
/// relative to wall time.
pub const MAX_CATCHUP_TICKS: u32 = 5;

/// Monotonic time source. The PSP backend reads the CPU tick counter.
pub trait Clock {
    /// Microseconds since an arbitrary fixed origin. Must never go backwards.
    fn now_us(&self) -> u64;
}

/// Accumulator that converts wall-clock time into a whole number of fixed
/// simulation ticks.
#[derive(Debug, Clone)]
pub struct FixedClock {
    last_us: u64,
    accumulator_us: u64,
    /// Total ticks simulated. Doubles as the RNG-relevant frame counter.
    pub tick: u64,
    /// Ticks dropped to the catch-up limit, surfaced by the profiler.
    pub dropped: u64,
}

impl FixedClock {
    pub fn new(now_us: u64) -> Self {
        FixedClock {
            last_us: now_us,
            accumulator_us: 0,
            tick: 0,
            dropped: 0,
        }
    }

    /// Advances to `now_us` and returns how many ticks to run.
    pub fn advance(&mut self, now_us: u64) -> u32 {
        // Saturating: a backwards clock should stall, not wrap to a huge delta.
        let delta = now_us.saturating_sub(self.last_us);
        self.last_us = now_us;
        self.accumulator_us += delta;

        let mut ticks = (self.accumulator_us / TICK_US) as u32;
        self.accumulator_us %= TICK_US;

        if ticks > MAX_CATCHUP_TICKS {
            self.dropped += (ticks - MAX_CATCHUP_TICKS) as u64;
            ticks = MAX_CATCHUP_TICKS;
            // Drop the leftover rather than carrying it, or the spiral resumes
            // on the next frame.
            self.accumulator_us = 0;
        }

        self.tick += ticks as u64;
        ticks
    }

    /// How far we are between the last tick and the next, in `0.0..1.0`.
    ///
    /// Rendering can interpolate with this to stay smooth when the display
    /// rate and the tick rate are not in lockstep.
    pub fn alpha(&self) -> f32 {
        self.accumulator_us as f32 / TICK_US as f32
    }
}

/// A named span of a frame, for the on-screen profiler.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Section {
    Simulation,
    Collision,
    Animation,
    Ai,
    RenderPrep,
    GpuSubmit,
    Audio,
    Resources,
}

impl Section {
    pub const ALL: &'static [Section] = &[
        Section::Simulation,
        Section::Collision,
        Section::Animation,
        Section::Ai,
        Section::RenderPrep,
        Section::GpuSubmit,
        Section::Audio,
        Section::Resources,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Section::Simulation => "Game",
            Section::Collision => "Collision",
            Section::Animation => "Animation",
            Section::Ai => "AI",
            Section::RenderPrep => "Render",
            Section::GpuSubmit => "GPU",
            Section::Audio => "Audio",
            Section::Resources => "Resources",
        }
    }
}

/// Per-section timings for one frame, in microseconds.
#[derive(Debug, Clone, Copy, Default)]
pub struct FrameTimings {
    pub sections: [u32; 8],
    pub total_us: u32,
}

impl FrameTimings {
    pub fn get(&self, s: Section) -> u32 {
        self.sections[s as usize]
    }

    pub fn set(&mut self, s: Section, us: u32) {
        self.sections[s as usize] = us;
    }

    /// Frame time as a frames-per-second figure.
    pub fn fps(&self) -> f32 {
        if self.total_us == 0 {
            0.0
        } else {
            1_000_000.0 / self.total_us as f32
        }
    }

    /// Time not attributed to any named section.
    pub fn other_us(&self) -> u32 {
        self.total_us.saturating_sub(self.sections.iter().sum())
    }
}

/// The 16.67 ms budget one frame has at 60 FPS.
pub const FRAME_BUDGET_US: u32 = 16_667;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_tick_per_period() {
        let mut c = FixedClock::new(0);
        assert_eq!(c.advance(TICK_US), 1);
        assert_eq!(c.tick, 1);
    }

    #[test]
    fn sub_tick_deltas_accumulate() {
        let mut c = FixedClock::new(0);
        // Three-quarters of a tick twice should produce exactly one tick.
        let step = TICK_US * 3 / 4;
        assert_eq!(c.advance(step), 0);
        assert_eq!(c.advance(step * 2), 1);
        assert_eq!(c.tick, 1);
    }

    #[test]
    fn long_stall_is_capped_and_counted() {
        let mut c = FixedClock::new(0);
        // A full second of stall would be 60 ticks.
        assert_eq!(c.advance(1_000_000), MAX_CATCHUP_TICKS);
        assert_eq!(c.tick, MAX_CATCHUP_TICKS as u64);
        assert!(c.dropped > 50, "dropped {}", c.dropped);
    }

    #[test]
    fn cap_does_not_leave_residue_that_re_triggers() {
        let mut c = FixedClock::new(0);
        c.advance(1_000_000);
        // Immediately after the cap, a zero-length frame produces no ticks.
        assert_eq!(c.advance(1_000_000), 0);
    }

    #[test]
    fn backwards_clock_stalls_rather_than_wrapping() {
        let mut c = FixedClock::new(1_000_000);
        assert_eq!(c.advance(0), 0);
        assert_eq!(c.tick, 0);
    }

    #[test]
    fn alpha_reports_fractional_progress() {
        let mut c = FixedClock::new(0);
        c.advance(TICK_US / 2);
        assert!((c.alpha() - 0.5).abs() < 0.05, "alpha {}", c.alpha());
    }

    #[test]
    fn sixty_ticks_per_simulated_second() {
        let mut c = FixedClock::new(0);
        // Feed one display frame at a time so the catch-up cap never trips.
        for f in 1..=60u64 {
            c.advance(f * TICK_US);
        }
        assert_eq!(c.tick, 60);
    }

    #[test]
    fn other_us_is_the_unattributed_remainder() {
        let mut t = FrameTimings {
            total_us: 10_000,
            ..Default::default()
        };
        t.set(Section::Simulation, 4_000);
        t.set(Section::RenderPrep, 3_000);
        assert_eq!(t.other_us(), 3_000);
        assert_eq!(t.get(Section::Simulation), 4_000);
    }

    #[test]
    fn section_names_cover_every_variant() {
        assert_eq!(Section::ALL.len(), 8);
        for s in Section::ALL {
            assert!(!s.name().is_empty());
        }
    }
}
