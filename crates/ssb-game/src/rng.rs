//! The game's shared random number generator — `sys/utils.c`'s
//! `syUtilsRandUShort`/`syUtilsRandFloat` over one global seed
//! (`sSYUtilsRandomSeed`, starting at 1).
//!
//! The original has one seed for the whole game, so every consumer advances
//! the same sequence. The atomic keeps it a plain global in `no_std`.

use core::sync::atomic::{AtomicI32, Ordering};

static SEED: AtomicI32 = AtomicI32::new(1);

fn step() -> i32 {
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
pub fn rand_ushort() -> u16 {
    (step() >> 16) as u16
}

/// `syUtilsRandFloat`: `[0, 1)`.
pub fn rand_float() -> f32 {
    ((step() >> 16) & 0xFFFF) as f32 / 65536.0
}

/// `syUtilsRandIntRange`.
pub fn rand_int_range(range: i32) -> i32 {
    (i32::from(rand_ushort()) * range) / 65536
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
