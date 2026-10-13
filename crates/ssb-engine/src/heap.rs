//! Size-class free lists for small heap blocks: the core of the PSP's
//! global allocator (`ssb_psp_runtime::heap`, RE-484).
//!
//! rust-psp's own allocator takes every block from the kernel
//! (`sceKernelAllocPartitionMemory`), about 9,400 cycles an allocate-and-free
//! on a PSP-2000 (RE-483). [`SmallHeap`] serves blocks up to
//! [`MAX_SMALL`] bytes from chunks the platform takes from the kernel once
//! and never returns: an allocation pops a free list or bumps a pointer, a
//! free pushes the block back on its class's list.
//!
//! Classes are the powers of two from 16 and the midpoints between them
//! (16, 24, 32, 48, 64, 96, ...), so a block wastes at most a third of
//! itself. A class's blocks are 16-byte aligned when its size is a
//! multiple of 16, 8-byte aligned otherwise; [`class_of`] picks a class
//! that honours the request's alignment, up to 16. Larger or more aligned
//! requests are the platform's (`None`).
//!
//! No locking: the platform serialises calls.

/// The smallest block.
pub const MIN_BLOCK: usize = 16;
/// The largest block a class serves.
pub const MAX_SMALL: usize = 32 * 1024;
/// Size classes, [`MIN_BLOCK`] to [`MAX_SMALL`].
pub const CLASSES: usize = 23;
/// The largest alignment a class serves.
pub const MAX_ALIGN: usize = 16;

/// Bytes in class `c`: 16, 24, 32, 48, ..., 24,576, 32,768.
pub const fn class_size(c: usize) -> usize {
    if c.is_multiple_of(2) {
        MIN_BLOCK << (c / 2)
    } else {
        24 << (c / 2)
    }
}

/// The class serving `size` bytes aligned to `align`, or `None` for a
/// request the platform serves itself.
#[inline]
pub fn class_of(size: usize, align: usize) -> Option<usize> {
    if align > MAX_ALIGN || size > MAX_SMALL {
        return None;
    }
    let s = size.max(MIN_BLOCK);
    // 2^k >= s > 2^(k-1), k >= 4.
    let k = (usize::BITS - (s - 1).leading_zeros()) as usize;
    let pow = 2 * (k - 4);
    // The midpoint class below 2^k, 3 * 2^(k-2), when it fits.
    let mid = 3usize << (k - 2);
    if k > 4 && s <= mid && (align <= 8 || mid.is_multiple_of(16)) {
        Some(pow - 1)
    } else {
        Some(pow)
    }
}

/// A free block's link.
#[repr(C)]
struct Free {
    next: *mut Free,
}

/// Counters for profiling.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Stats {
    /// Bytes taken from the platform in chunks.
    pub chunk_bytes: usize,
    /// Chunks taken.
    pub chunks: u32,
    /// Bytes in blocks handed out and not yet freed, by class size.
    pub live_bytes: usize,
}

/// Free lists and the current chunk's unused tail.
pub struct SmallHeap {
    free: [*mut Free; CLASSES],
    bump: usize,
    end: usize,
    stats: Stats,
}

// SAFETY: the heap owns the memory its pointers reach; the platform
// serialises access.
unsafe impl Send for SmallHeap {}

impl Default for SmallHeap {
    fn default() -> Self {
        Self::new()
    }
}

impl SmallHeap {
    pub const fn new() -> Self {
        Self {
            free: [core::ptr::null_mut(); CLASSES],
            bump: 0,
            end: 0,
            stats: Stats {
                chunk_bytes: 0,
                chunks: 0,
                live_bytes: 0,
            },
        }
    }

    pub fn stats(&self) -> Stats {
        self.stats
    }

    /// A block of class `c`, or null when the list is empty, the chunk is
    /// spent and `chunk` returns none. `chunk(min)` returns a new chunk of
    /// at least `min` bytes, 16-byte aligned, which the heap keeps for
    /// good.
    ///
    /// # Safety
    ///
    /// `c < CLASSES`; every chunk returned is writable, owned by the heap
    /// from then on and 16-byte aligned.
    #[inline]
    pub unsafe fn alloc(
        &mut self,
        c: usize,
        chunk: impl FnOnce(usize) -> Option<(*mut u8, usize)>,
    ) -> *mut u8 {
        let head = self.free[c];
        if !head.is_null() {
            self.free[c] = (*head).next;
            self.stats.live_bytes += class_size(c);
            return head.cast();
        }
        self.carve(c, chunk)
    }

    #[inline(never)]
    unsafe fn carve(
        &mut self,
        c: usize,
        chunk: impl FnOnce(usize) -> Option<(*mut u8, usize)>,
    ) -> *mut u8 {
        let size = class_size(c);
        let mut at = align_for(self.bump, size);
        if at + size > self.end || self.end == 0 {
            let Some((base, len)) = chunk(size) else {
                return core::ptr::null_mut();
            };
            // The old tail goes to the free lists, largest blocks first.
            self.spill();
            self.bump = base as usize;
            self.end = self.bump + len;
            self.stats.chunk_bytes += len;
            self.stats.chunks += 1;
            at = align_for(self.bump, size);
            if at + size > self.end {
                return core::ptr::null_mut();
            }
        }
        self.bump = at + size;
        self.stats.live_bytes += size;
        at as *mut u8
    }

    /// Splits the chunk's unused tail into free blocks.
    unsafe fn spill(&mut self) {
        let mut c = CLASSES;
        while c > 0 {
            c -= 1;
            let size = class_size(c);
            loop {
                let at = align_for(self.bump, size);
                if at + size > self.end {
                    break;
                }
                let block = at as *mut Free;
                (*block).next = self.free[c];
                self.free[c] = block;
                self.bump = at + size;
            }
        }
        self.bump = self.end;
    }

    /// Returns a block to class `c`'s list.
    ///
    /// # Safety
    ///
    /// `ptr` came from [`SmallHeap::alloc`] with class `c` and is not used
    /// again.
    #[inline]
    pub unsafe fn free(&mut self, ptr: *mut u8, c: usize) {
        let block = ptr.cast::<Free>();
        (*block).next = self.free[c];
        self.free[c] = block;
        self.stats.live_bytes -= class_size(c);
    }
}

/// `at` rounded up to the alignment a block of `size` bytes promises.
#[inline]
fn align_for(at: usize, size: usize) -> usize {
    let a = if size.is_multiple_of(16) { 16 } else { 8 };
    (at + a - 1) & !(a - 1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;

    #[test]
    fn classes_grow_by_halves() {
        let sizes: Vec<usize> = (0..CLASSES).map(class_size).collect();
        assert_eq!(&sizes[..6], &[16, 24, 32, 48, 64, 96]);
        assert_eq!(sizes[CLASSES - 1], MAX_SMALL);
        for c in 1..CLASSES {
            assert!(sizes[c] > sizes[c - 1]);
            assert!(sizes[c] * 2 <= sizes[c - 1] * 3);
        }
    }

    #[test]
    fn class_of_picks_the_smallest_fitting_aligned_class() {
        for align in [1, 2, 4, 8, 16] {
            for size in 0..=MAX_SMALL {
                let c = class_of(size, align).unwrap();
                let cs = class_size(c);
                assert!(cs >= size.max(MIN_BLOCK), "{size} {align}");
                if align == 16 {
                    assert!(cs.is_multiple_of(16));
                }
                // No smaller class with the right alignment fits.
                for d in 0..c {
                    let ds = class_size(d);
                    assert!(
                        ds < size || (align == 16 && !ds.is_multiple_of(16)),
                        "{size} {align} {d}"
                    );
                }
            }
        }
        assert_eq!(class_of(MAX_SMALL + 1, 8), None);
        assert_eq!(class_of(64, 32), None);
    }

    /// Chunks from a host buffer, 16-byte aligned.
    struct Host {
        bufs: Vec<Vec<u128>>,
        len: usize,
    }

    impl Host {
        fn chunk(&mut self, min: usize) -> Option<(*mut u8, usize)> {
            let len = self.len.max(min);
            let mut v = alloc::vec![0u128; len.div_ceil(16)];
            let p = v.as_mut_ptr().cast::<u8>();
            self.bufs.push(v);
            Some((p, len))
        }
    }

    #[test]
    fn blocks_are_aligned_disjoint_and_reused() {
        let mut host = Host {
            bufs: Vec::new(),
            len: 4096,
        };
        let mut heap = SmallHeap::new();
        let mut live: Vec<(usize, usize)> = Vec::new();
        let mut seed = 1u32;
        for i in 0..20_000 {
            seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12_345);
            if !live.is_empty() && (seed >> 16).is_multiple_of(3) {
                let (p, c) = live.swap_remove((seed >> 8) as usize % live.len());
                unsafe { heap.free(p as *mut u8, c) };
                continue;
            }
            let size = 1 + (seed >> 12) as usize % 3000;
            let align = [1, 4, 8, 16][i % 4];
            let c = class_of(size, align).unwrap();
            let p = unsafe { heap.alloc(c, |m| host.chunk(m)) } as usize;
            assert_ne!(p, 0);
            assert_eq!(p % align, 0);
            let end = p + class_size(c);
            for &(q, d) in &live {
                assert!(end <= q || q + class_size(d) <= p, "overlap");
            }
            live.push((p, c));
        }
        let want: usize = live.iter().map(|&(_, c)| class_size(c)).sum();
        assert_eq!(heap.stats().live_bytes, want);
        // Freed blocks come back before a new chunk is taken.
        let chunks = heap.stats().chunks;
        for (p, c) in live.drain(..) {
            unsafe { heap.free(p as *mut u8, c) };
        }
        for _ in 0..100 {
            let c = class_of(100, 8).unwrap();
            let p = unsafe { heap.alloc(c, |m| host.chunk(m)) };
            live.push((p as usize, c));
        }
        assert_eq!(heap.stats().chunks, chunks);
    }

    #[test]
    fn a_spent_chunk_spills_its_tail_and_none_is_null() {
        let mut host = Host {
            bufs: Vec::new(),
            len: 1000,
        };
        let mut heap = SmallHeap::new();
        let big = class_of(600, 8).unwrap();
        let a = unsafe { heap.alloc(big, |m| host.chunk(m)) };
        assert!(!a.is_null());
        // The second 768-byte block does not fit: a new chunk, the old tail
        // (232 bytes) split into the free lists: a 192 and a 32.
        let b = unsafe { heap.alloc(big, |m| host.chunk(m)) };
        assert!(!b.is_null());
        assert_eq!(heap.stats().chunks, 2);
        let small = class_of(150, 8).unwrap();
        let s = unsafe { heap.alloc(small, |_| None) };
        let first = host.bufs[0].as_ptr() as usize;
        assert!(
            (s as usize) >= first && (s as usize) < first + 1000,
            "from the spilled tail"
        );
        let none = unsafe { heap.alloc(big, |_| None) };
        assert!(none.is_null());
    }
}
