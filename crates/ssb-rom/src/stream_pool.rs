//! A fixed slab that holds streamed archive files, least recently used
//! out first (RE-572 option C, RE-573).
//!
//! The N64 reads a fighter's figatree into its figatree heap on every
//! status change (`ftMainSetStatus` → `lbRelocGetForceExternHeapFile`,
//! `ftmain.c:4619-4626`), a heap sized to the largest figatree
//! (`ftManagerAllocFigatreeHeapKind`, `ftmanager.c:364`). A Memory Stick
//! read costs ~1.6 ms + 0.073 ms/KiB (RE-573), so the port keeps the last
//! few in a slab and reads only on a miss: a [`StreamPool`] does the slab's
//! bookkeeping, the platform its reads and its registry entries.
//!
//! Entries never move. A new one takes the first gap that fits; while none
//! does, the entry least recently used goes, except one used in the
//! current [`Epoch`] (its bytes may still be in use this frame).

use alloc::vec::Vec;

use crate::residency::Epoch;

/// Entry starts and sizes are whole multiples of this (a cache line).
pub const ALIGN: u32 = 64;

/// One file in the slab: `len` bytes at offset `at`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Entry {
    pub key: u32,
    pub at: u32,
    pub len: u32,
}

impl Entry {
    /// The bytes it occupies.
    pub fn size(&self) -> u32 {
        self.len.div_ceil(ALIGN) * ALIGN
    }
}

/// Counts since [`StreamPool::new`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Stats {
    /// Entries placed.
    pub placed: u32,
    /// Entries evicted to make room.
    pub evicted: u32,
    /// Placements that found no room.
    pub failed: u32,
    /// Most bytes occupied at once.
    pub peak: u32,
}

/// A slab of `capacity` bytes holding entries by key.
#[derive(Debug, Default)]
pub struct StreamPool {
    capacity: u32,
    /// Sorted by `at`.
    entries: Vec<Entry>,
    stats: Stats,
}

impl StreamPool {
    pub const fn new(capacity: u32) -> Self {
        Self {
            capacity,
            entries: Vec::new(),
            stats: Stats {
                placed: 0,
                evicted: 0,
                failed: 0,
                peak: 0,
            },
        }
    }

    pub fn capacity(&self) -> u32 {
        self.capacity
    }

    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    pub fn stats(&self) -> Stats {
        self.stats
    }

    /// Bytes the entries occupy.
    pub fn occupied(&self) -> u32 {
        self.entries.iter().map(Entry::size).sum()
    }

    pub fn get(&self, key: u32) -> Option<Entry> {
        self.entries.iter().copied().find(|e| e.key == key)
    }

    /// The first gap of `size` bytes, if any.
    fn gap(&self, size: u32) -> Option<(usize, u32)> {
        let mut end = 0;
        for (i, e) in self.entries.iter().enumerate() {
            if e.at - end >= size {
                return Some((i, end));
            }
            end = e.at + e.size();
        }
        (self.capacity.saturating_sub(end) >= size).then_some((self.entries.len(), end))
    }

    /// Places `len` bytes for `key` (not already held): in the first gap
    /// that fits, evicting the least recently used entries into `evicted`
    /// until one does. `used` gives an entry's last [`Epoch`]; entries used
    /// in `now` stay. `None` when no room can be made; what was evicted
    /// on the way stays evicted.
    pub fn place(
        &mut self,
        key: u32,
        len: u32,
        now: Epoch,
        mut used: impl FnMut(&Entry) -> Epoch,
        evicted: &mut Vec<Entry>,
    ) -> Option<Entry> {
        debug_assert!(self.get(key).is_none());
        let size = len.div_ceil(ALIGN) * ALIGN;
        loop {
            if let Some((i, at)) = self.gap(size) {
                let e = Entry { key, at, len };
                self.entries.insert(i, e);
                self.stats.placed += 1;
                self.stats.peak = self.stats.peak.max(self.occupied());
                return Some(e);
            }
            // The oldest entry not used this epoch; ties to the lowest.
            let victim = self
                .entries
                .iter()
                .enumerate()
                .map(|(i, e)| (i, now.wrapping_sub(used(e))))
                .filter(|&(_, age)| age > 0)
                .max_by_key(|&(i, age)| (age, core::cmp::Reverse(i)))
                .map(|(i, _)| i);
            let Some(i) = victim else {
                self.stats.failed += 1;
                return None;
            };
            evicted.push(self.entries.remove(i));
            self.stats.evicted += 1;
        }
    }

    /// Takes `key`'s entry out.
    pub fn remove(&mut self, key: u32) -> Option<Entry> {
        let i = self.entries.iter().position(|e| e.key == key)?;
        Some(self.entries.remove(i))
    }

    /// Takes every entry out, into `out`.
    pub fn drain_into(&mut self, out: &mut Vec<Entry>) {
        out.append(&mut self.entries);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn least_recently_used_goes_first_and_this_epoch_stays() {
        let mut pool = StreamPool::new(256);
        let mut out = Vec::new();
        let mut used = [0u32; 8];
        // Four 64-byte entries fill it; keys stamped as they land.
        for (k, now) in [(0u32, 1u32), (1, 2), (2, 3), (3, 4)] {
            used[k as usize] = now;
            let e = pool
                .place(k, 50, now, |e| used[e.key as usize], &mut out)
                .unwrap();
            assert_eq!(e.at, k * 64);
        }
        assert!(out.is_empty());
        // Key 0 used again: key 1 is the oldest and goes.
        used[0] = 5;
        let e = pool
            .place(4, 64, 5, |e| used[e.key as usize], &mut out)
            .unwrap();
        assert_eq!((e.at, out[0].key), (64, 1));
        used[4] = 5;
        // 128 bytes: evicts 2 then 3 (oldest first), which leaves a gap.
        out.clear();
        let e = pool
            .place(5, 100, 6, |e| used[e.key as usize], &mut out)
            .unwrap();
        assert_eq!(out.iter().map(|e| e.key).collect::<Vec<_>>(), [2, 3]);
        assert_eq!(e.at, 128);
        assert_eq!(pool.occupied(), 256);
        // Everything used this epoch: no room is made.
        out.clear();
        let all = |_: &Entry| 7;
        assert!(pool.place(6, 64, 7, all, &mut out).is_none());
        assert!(out.is_empty());
        assert_eq!(pool.stats().failed, 1);
        // Too large for the slab: everything evictable goes, then it fails.
        assert!(pool.place(7, 512, 8, all, &mut out).is_none());
        assert_eq!(out.len(), 3);
        assert_eq!(pool.entries().len(), 0);
        let s = pool.stats();
        assert_eq!((s.placed, s.evicted, s.peak), (6, 6, 256));
    }

    #[test]
    fn epochs_wrap() {
        let mut pool = StreamPool::new(128);
        let mut out = Vec::new();
        let stamp = [u32::MAX, 0];
        pool.place(0, 64, u32::MAX, |e| stamp[e.key as usize], &mut out)
            .unwrap();
        pool.place(1, 64, 0, |e| stamp[e.key as usize], &mut out)
            .unwrap();
        // At epoch 1, key 0 (stamped MAX, two epochs back) is older.
        pool.place(2, 64, 1, |e| stamp[e.key.min(1) as usize], &mut out)
            .unwrap();
        assert_eq!(out[0].key, 0);
    }
}
