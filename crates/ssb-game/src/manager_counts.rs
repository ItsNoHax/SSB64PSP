//! `gFTManagerMotionCount` and `gFTManagerStatUpdateCount` (`ftmanager.c`
//! :37, :40): the battle's two shared nonzero u16 sequences, which
//! `ftManagerAllocFighter` resets to 1 (`ftmanager.c:163-164`) before the
//! battle's fighters are made.
//!
//! Every fighter, item and weapon of one battle draws from the same pair:
//! a handle ([`ManagerCounts`]) shared by the fighters' [`Stats`], the
//! [`ItemPool`] (and each item it holds) and the [`WeaponPool`]. A battle
//! setup makes one with [`ManagerCounts::new`] and hands it to the pools and
//! to each fighter before its first status; [`crate::matchtick::step`]
//! links any stray handle to the item pool's ([`ManagerCounts::adopt`]).
//!
//! [`Stats`]: crate::spgame::live::Stats
//! [`ItemPool`]: crate::item::ItemPool
//! [`WeaponPool`]: crate::weapon::WeaponPool

use alloc::rc::Rc;
use core::cell::Cell;
use core::fmt;

#[derive(Clone, Copy)]
struct Next {
    motion: u16,
    stat: u16,
}

/// A handle on one battle's counters. Clones share them.
#[derive(Clone)]
pub struct ManagerCounts(Rc<Cell<Next>>);

impl ManagerCounts {
    /// `ftManagerAllocFighter`: both sequences start at 1.
    pub fn new() -> Self {
        ManagerCounts(Rc::new(Cell::new(Next { motion: 1, stat: 1 })))
    }

    /// `ftParamGetMotionCount` (`ftparam.c:1620`), skipping 0 on wrap.
    pub fn motion(&self) -> u16 {
        let mut n = self.0.get();
        let count = n.motion;
        n.motion = n.motion.wrapping_add(1).max(1);
        self.0.set(n);
        count
    }

    /// `ftParamGetStatUpdateCount` (`ftparam.c:1668`), skipping 0 on wrap.
    pub fn stat(&self) -> u16 {
        let mut n = self.0.get();
        let count = n.stat;
        n.stat = n.stat.wrapping_add(1).max(1);
        self.0.set(n);
        count
    }

    /// The next `(motion, stat)` counts, without drawing them.
    pub fn peek(&self) -> (u16, u16) {
        let n = self.0.get();
        (n.motion, n.stat)
    }

    /// Whether both handles share one pair of counters.
    pub fn same(&self, other: &ManagerCounts) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }

    /// Joins `self` to `battle`'s counters. A handle that already drew
    /// (an object made before the battle's handle reached it) first moves
    /// the battle's counters past its own, so no count repeats.
    pub fn adopt(&mut self, battle: &ManagerCounts) {
        if self.same(battle) {
            return;
        }
        let (own, mut b) = (self.0.get(), battle.0.get());
        b.motion = b.motion.max(own.motion);
        b.stat = b.stat.max(own.stat);
        battle.0.set(b);
        *self = battle.clone();
    }
}

impl Default for ManagerCounts {
    /// A handle of its own: a fighter, item or pool outside a battle.
    fn default() -> Self {
        Self::new()
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
        c.0.set(Next {
            motion: u16::MAX,
            stat: u16::MAX,
        });
        assert_eq!((c.motion(), c.motion()), (u16::MAX, 1));
        assert_eq!((c.stat(), c.stat()), (u16::MAX, 1));
    }

    #[test]
    fn clones_share_and_adopt_moves_past_drawn_counts() {
        let battle = ManagerCounts::new();
        let shared = battle.clone();
        assert_eq!(shared.stat(), 1);
        assert_eq!(battle.stat(), 2);
        let mut stray = ManagerCounts::new();
        for _ in 0..5 {
            stray.motion();
        }
        stray.adopt(&battle);
        assert!(stray.same(&battle));
        assert_eq!(battle.peek(), (6, 3));
    }
}
