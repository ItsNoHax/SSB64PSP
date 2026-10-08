//! Per-stage render timing for platform profiling (feature
//! `audio_profile`). The platform installs a microsecond clock with
//! [`set_clock`]; the renderer adds each stage's time to a single-writer
//! accumulator the platform reads with [`read`]. Without the feature every
//! call compiles to nothing.

/// A renderer stage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(usize)]
pub enum Stage {
    /// The sequence players' client handlers (`n_alCSPVoiceHandler`).
    Seq,
    /// The sound player / FGM client handler.
    Snd,
    /// `A_ADPCM` decoding (`_decodeChunk`).
    Adpcm,
    /// `A_RESAMPLE` (or the unity-pitch `A_DMEMMOVE`).
    Resample,
    /// `A_ENVMIXER`.
    Envmix,
    /// The aux effect (`n_alFxPull` after the aux bus).
    Reverb,
    /// Bus clears and mixes, interleave and save.
    Bus,
    /// Mono pass and the per-frame `syAudio` work after rendering.
    Post,
    /// Whole `render_frame` calls.
    Total,
    /// Count of voice sub-frames pulled (not a time).
    Pulls,
}

/// Number of [`Stage`]s.
pub const STAGES: usize = 10;

#[cfg(feature = "audio_profile")]
mod imp {
    use super::{Stage, STAGES};
    use core::sync::atomic::{AtomicU32, AtomicUsize, Ordering};

    static CLOCK: AtomicUsize = AtomicUsize::new(0);
    #[allow(clippy::declare_interior_mutable_const)]
    const ZERO: AtomicU32 = AtomicU32::new(0);
    static ACC: [AtomicU32; STAGES] = [ZERO; STAGES];

    pub fn set_clock(f: fn() -> u32) {
        CLOCK.store(f as usize, Ordering::Release);
    }

    #[inline(always)]
    pub fn now() -> u32 {
        let f = CLOCK.load(Ordering::Relaxed);
        if f == 0 {
            return 0;
        }
        // SAFETY: only `set_clock` stores a non-zero value, a `fn() -> u32`.
        let f: fn() -> u32 = unsafe { core::mem::transmute(f) };
        f()
    }

    #[inline(always)]
    pub fn add(s: Stage, n: u32) {
        let a = &ACC[s as usize];
        a.store(a.load(Ordering::Relaxed).wrapping_add(n), Ordering::Release);
    }

    pub fn read() -> [u32; STAGES] {
        core::array::from_fn(|i| ACC[i].load(Ordering::Acquire))
    }
}

/// Installs the clock (microseconds) the stage timers read.
#[cfg(feature = "audio_profile")]
pub fn set_clock(f: fn() -> u32) {
    imp::set_clock(f);
}

/// A stage timer's start.
#[inline(always)]
pub fn start() -> u32 {
    #[cfg(feature = "audio_profile")]
    return imp::now();
    #[cfg(not(feature = "audio_profile"))]
    0
}

/// Adds the time since `t0` to `stage`.
#[inline(always)]
pub fn stop(stage: Stage, t0: u32) {
    #[cfg(feature = "audio_profile")]
    imp::add(stage, imp::now().wrapping_sub(t0));
    #[cfg(not(feature = "audio_profile"))]
    let _ = (stage, t0);
}

/// Adds `n` to a counting stage.
#[inline(always)]
pub fn count(stage: Stage, n: u32) {
    #[cfg(feature = "audio_profile")]
    imp::add(stage, n);
    #[cfg(not(feature = "audio_profile"))]
    let _ = (stage, n);
}

/// The running totals, indexed by [`Stage`] (zeros without the feature).
pub fn read() -> [u32; STAGES] {
    #[cfg(feature = "audio_profile")]
    return imp::read();
    #[cfg(not(feature = "audio_profile"))]
    [0; STAGES]
}
