//! The game's shared random number generator — `sys/utils.c`'s
//! `syUtilsRandUShort`/`syUtilsRandFloat` over one global seed
//! (`sSYUtilsRandomSeed`, starting at 1).
//!
//! The original has one seed for the whole game, so every consumer advances
//! the same sequence. The atomic keeps it a plain global in `no_std`.

use core::sync::atomic::{AtomicI32, Ordering};

static SEED: AtomicI32 = AtomicI32::new(1);

#[cfg_attr(feature = "rng_trace", track_caller)]
fn step() -> i32 {
    #[cfg(feature = "rng_trace")]
    trace::record(core::panic::Location::caller());
    let next = SEED
        .load(Ordering::Relaxed)
        .wrapping_mul(214013)
        .wrapping_add(2531011);
    SEED.store(next, Ordering::Relaxed);
    next
}

/// `syUtilsSetRandomSeed`.
pub fn set_seed(seed: i32) {
    SEED.store(seed, Ordering::Relaxed);
}

/// `syUtilsRandSeed`.
pub fn seed() -> i32 {
    SEED.load(Ordering::Relaxed)
}

/// `syUtilsRandUShort`.
#[cfg_attr(feature = "rng_trace", track_caller)]
pub fn rand_ushort() -> u16 {
    (step() >> 16) as u16
}

/// `syUtilsRandFloat`: `[0, 1)`.
#[cfg_attr(feature = "rng_trace", track_caller)]
pub fn rand_float() -> f32 {
    ((step() >> 16) & 0xFFFF) as f32 / 65536.0
}

/// `syUtilsRandIntRange`.
#[cfg_attr(feature = "rng_trace", track_caller)]
pub fn rand_int_range(range: i32) -> i32 {
    (i32::from(rand_ushort()) * range) / 65536
}

/// Diagnostic: where each draw since the last `take` came from (the
/// caller of `rand_*`), for comparing a capture's draws with an N64 trace.
#[cfg(feature = "rng_trace")]
pub mod trace {
    use core::panic::Location;
    use core::sync::atomic::{AtomicUsize, Ordering};

    const CAP: usize = 64;
    static mut LOG: [Option<(&'static Location<'static>, i32)>; CAP] = [None; CAP];
    static LEN: AtomicUsize = AtomicUsize::new(0);

    pub(super) fn record(at: &'static Location<'static>) {
        mark(at, -1);
    }

    /// Records a non-drawing event (`value` >= 0) in the draw log.
    pub fn mark(at: &'static Location<'static>, value: i32) {
        let n = LEN.fetch_add(1, Ordering::Relaxed);
        if n < CAP {
            // SAFETY: single-threaded game loop; diagnostic only.
            unsafe { LOG[n] = Some((at, value)) };
        }
    }

    /// Calls `f` with each recorded site (and mark value, -1 for a draw)
    /// and clears the log.
    pub fn take(mut f: impl FnMut(&'static str, u32, i32)) -> usize {
        let n = LEN.swap(0, Ordering::Relaxed);
        // SAFETY: as above.
        let log = unsafe { *core::ptr::addr_of!(LOG) };
        for (at, v) in log.iter().take(n.min(CAP)).flatten() {
            f(at.file(), at.line(), *v);
        }
        n
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sequence_is_the_msvc_lcg() {
        // From seed 1: (1 * 214013 + 2531011) >> 16 = 41.
        set_seed(1);
        assert_eq!(rand_ushort(), 41);
        assert_eq!(seed(), 2745024);
    }
}
