//! `gFTManagerMotionCount` and `gFTManagerStatUpdateCount` (`ftmanager.c`
//! :37, :40): the battle's two shared nonzero u16 sequences, which
//! `ftManagerAllocFighter` resets to 1 (`ftmanager.c:163-164`) before the
//! battle's fighters are made.
//!
//! The counters are the manager's globals ([`game_static!`]), as the
//! original keeps them; [`ManagerCounts`] is a handle with no state of its
//! own, so every fighter's [`Stats`], the [`ItemPool`] (and each item it
//! makes) and the [`WeaponPool`] draw from the one pair. A battle setup
//! resets it with [`ManagerCounts::new`] ([`ItemPool`]'s, as
//! `itManagerInitItems` follows `ftManagerAllocFighter`); every other
//! handle ([`Default`]) joins the pair as it stands.
//!
//! [`Stats`]: crate::spgame::live::Stats
//! [`ItemPool`]: crate::item::ItemPool
//! [`WeaponPool`]: crate::weapon::WeaponPool

use core::fmt;
use core::sync::atomic::{AtomicU16, Ordering};

game_static! {
    /// `gFTManagerMotionCount`: the next count `ftParamGetMotionCount` returns.
    static MOTION: AtomicU16 = AtomicU16::new(1);
}
game_static! {
    /// `gFTManagerStatUpdateCount`: the next count
    /// `ftParamGetStatUpdateCount` returns.
    static STAT: AtomicU16 = AtomicU16::new(1);
}

/// A handle on the battle's counters ([module docs](self)).
#[derive(Clone, Default)]
pub struct ManagerCounts;

/// Returns `counter`'s count and moves it on, skipping 0 on wrap.
fn draw(counter: &AtomicU16) -> u16 {
    let count = counter.load(Ordering::Relaxed);
    counter.store(count.wrapping_add(1).max(1), Ordering::Relaxed);
    count
}

impl ManagerCounts {
    /// `ftManagerAllocFighter`: both sequences start again at 1.
    pub fn new() -> Self {
        MOTION.with(|c| c.store(1, Ordering::Relaxed));
        STAT.with(|c| c.store(1, Ordering::Relaxed));
        ManagerCounts
    }

    /// `ftParamGetMotionCount` (`ftparam.c:1620`), skipping 0 on wrap.
    pub fn motion(&self) -> u16 {
        MOTION.with(draw)
    }

    /// `ftParamGetStatUpdateCount` (`ftparam.c:1668`), skipping 0 on wrap.
    pub fn stat(&self) -> u16 {
        STAT.with(draw)
    }

    /// The next `(motion, stat)` counts, without drawing them.
    pub fn peek(&self) -> (u16, u16) {
        (
            MOTION.with(|c| c.load(Ordering::Relaxed)),
            STAT.with(|c| c.load(Ordering::Relaxed)),
        )
    }
}

impl fmt::Debug for ManagerCounts {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (motion, stat) = self.peek();
        f.debug_struct("ManagerCounts")
            .field("motion", &motion)
            .field("stat", &stat)
            .finish()
    }
}

impl PartialEq for ManagerCounts {
    /// The counters are the battle's, not the holder's state: holders
    /// compare equal whatever handle they carry.
    fn eq(&self, _: &ManagerCounts) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_start_at_one_and_skip_zero_on_wrap() {
        let c = ManagerCounts::new();
        assert_eq!((c.motion(), c.stat(), c.stat()), (1, 1, 2));
        MOTION.with(|m| m.store(u16::MAX, Ordering::Relaxed));
        STAT.with(|s| s.store(u16::MAX, Ordering::Relaxed));
        assert_eq!((c.motion(), c.motion()), (u16::MAX, 1));
        assert_eq!((c.stat(), c.stat()), (u16::MAX, 1));
    }

    #[test]
    fn every_handle_draws_from_the_one_pair_until_the_next_battle() {
        let battle = ManagerCounts::new();
        let other = ManagerCounts;
        assert_eq!(other.stat(), 1);
        assert_eq!(battle.stat(), 2);
        for _ in 0..5 {
            other.motion();
        }
        assert_eq!(battle.peek(), (6, 3));
        // The next battle's `ftManagerAllocFighter`.
        let _ = ManagerCounts::new();
        assert_eq!(other.peek(), (1, 1));
    }
}
