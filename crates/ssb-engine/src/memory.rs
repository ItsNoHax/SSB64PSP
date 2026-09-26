//! The allocators planned in `docs/memory.md`: scoped bump arenas and a
//! fixed-capacity object pool. `no_std`, no `alloc`: an arena borrows its
//! storage, which the platform layer provides (a static, or one block from
//! the PSP heap).
//!
//! The original loads a scene's whole `relocData` closure into one
//! allocation sized up front (`lbRelocGetAllocSize` then
//! `lbRelocLoadFilesExtern`) and frees it in one go. [`Arena`] keeps that
//! shape: allocations are pointer bumps inside [`Arena::scope`], and the
//! whole scope is released when it returns. The three planned arenas are the
//! same type with different scope lifetimes:
//!
//! | Arena | One scope per |
//! |---|---|
//! | [`AssetArena`] | scene: its asset closure (`ssb_rom::reloc_link::Layout::size` sizes it) |
//! | [`GameArena`] | scene: long-lived game state |
//! | [`FrameArena`] | tick: per-frame scratch, so per-frame paths never touch the heap |
//!
//! [`ObjectPool`] is the fixed-capacity store for fighters, items and
//! particles, whose counts the original also fixes.
//!
//! Nothing uses these yet; `psp-runtime` still allocates through `alloc`.

use core::mem::{align_of, size_of, MaybeUninit};

/// Default alignment: `LBRELOC_CACHE_ALIGN`'s 16 bytes, which is also what
/// the GE wants for DMA sources.
pub const ARENA_ALIGN: usize = 16;

/// A bump allocator over borrowed storage. See the module docs.
pub struct Arena<'a> {
    buf: &'a mut [u8],
    high_water: usize,
}

/// Per-scene asset block.
pub type AssetArena<'a> = Arena<'a>;
/// Per-scene game state.
pub type GameArena<'a> = Arena<'a>;
/// Per-tick scratch.
pub type FrameArena<'a> = Arena<'a>;

impl<'a> Arena<'a> {
    pub fn new(buf: &'a mut [u8]) -> Self {
        Arena { buf, high_water: 0 }
    }

    pub fn capacity(&self) -> usize {
        self.buf.len()
    }

    /// The most any scope has used, alignment padding included, for sizing
    /// the backing block from real workloads.
    pub fn high_water(&self) -> usize {
        self.high_water
    }

    /// Runs `f` with an empty scope. Everything allocated from it is
    /// released when `f` returns; the borrow checker keeps any allocation
    /// from outliving the scope.
    pub fn scope<R>(&mut self, f: impl for<'s> FnOnce(&mut Scope<'s>) -> R) -> R {
        let mut scope = Scope {
            rest: &mut self.buf[..],
            used: 0,
        };
        let result = f(&mut scope);
        self.high_water = self.high_water.max(scope.used);
        result
    }
}

/// One scope of an [`Arena`]: allocations live until the scope ends.
pub struct Scope<'s> {
    rest: &'s mut [u8],
    used: usize,
}

impl<'s> Scope<'s> {
    /// Bytes used so far, alignment padding included.
    pub fn used(&self) -> usize {
        self.used
    }

    pub fn remaining(&self) -> usize {
        self.rest.len()
    }

    /// `len` bytes aligned to `align` (a power of two), or `None` when the
    /// arena is full. The contents are whatever an earlier scope left.
    pub fn alloc_bytes(&mut self, len: usize, align: usize) -> Option<&'s mut [u8]> {
        debug_assert!(align.is_power_of_two());
        let addr = self.rest.as_ptr() as usize;
        let pad = addr.wrapping_neg() & (align - 1);
        if pad.checked_add(len)? > self.rest.len() {
            return None;
        }
        let rest = core::mem::take(&mut self.rest);
        let (_, rest) = rest.split_at_mut(pad);
        let (out, rest) = rest.split_at_mut(len);
        self.rest = rest;
        self.used += pad + len;
        Some(out)
    }

    /// Moves `value` into the arena.
    pub fn alloc<T: Copy>(&mut self, value: T) -> Option<&'s mut T> {
        let bytes = self.alloc_bytes(size_of::<T>(), align_of::<T>())?;
        let slot = bytes.as_mut_ptr().cast::<MaybeUninit<T>>();
        // SAFETY: `bytes` is exclusively borrowed for `'s`, `size_of::<T>()`
        // long and aligned for `T`, so it is a valid `MaybeUninit<T>`;
        // writing `value` initialises it.
        Some(unsafe { (*slot).write(value) })
    }

    /// `len` copies of `value`, contiguous.
    pub fn alloc_slice<T: Copy>(&mut self, len: usize, value: T) -> Option<&'s mut [T]> {
        let bytes = self.alloc_bytes(size_of::<T>().checked_mul(len)?, align_of::<T>())?;
        let first = bytes.as_mut_ptr().cast::<T>();
        // SAFETY: as in `alloc`, for `len` elements; every element is
        // written before the slice is formed.
        unsafe {
            for i in 0..len {
                first.add(i).write(value);
            }
            Some(core::slice::from_raw_parts_mut(first, len))
        }
    }
}

/// A handle into an [`ObjectPool`]. The generation makes a handle to a
/// removed object stay invalid after its slot is reused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Handle {
    index: u16,
    generation: u16,
}

impl Handle {
    pub fn index(self) -> usize {
        usize::from(self.index)
    }
}

struct Slot<T> {
    generation: u16,
    value: Option<T>,
}

/// At most `N` live objects, no heap. Iteration visits slots in index
/// order, the order the original's fixed arrays process them.
pub struct ObjectPool<T, const N: usize> {
    slots: [Slot<T>; N],
    len: usize,
}

impl<T, const N: usize> Default for ObjectPool<T, N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T, const N: usize> ObjectPool<T, N> {
    pub fn new() -> Self {
        assert!(
            N <= usize::from(u16::MAX),
            "pool capacity exceeds u16 handles"
        );
        ObjectPool {
            slots: core::array::from_fn(|_| Slot {
                generation: 0,
                value: None,
            }),
            len: 0,
        }
    }

    pub const fn capacity(&self) -> usize {
        N
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Stores `value` in the lowest free slot, or gives it back when full.
    pub fn insert(&mut self, value: T) -> Result<Handle, T> {
        let Some(index) = self.slots.iter().position(|s| s.value.is_none()) else {
            return Err(value);
        };
        let slot = &mut self.slots[index];
        slot.value = Some(value);
        self.len += 1;
        Ok(Handle {
            index: index as u16,
            generation: slot.generation,
        })
    }

    fn slot(&self, h: Handle) -> Option<&Slot<T>> {
        self.slots
            .get(h.index())
            .filter(|s| s.generation == h.generation)
    }

    pub fn get(&self, h: Handle) -> Option<&T> {
        self.slot(h)?.value.as_ref()
    }

    pub fn get_mut(&mut self, h: Handle) -> Option<&mut T> {
        self.slots
            .get_mut(h.index())
            .filter(|s| s.generation == h.generation)?
            .value
            .as_mut()
    }

    /// Removes the object; its handle, and every copy of it, goes stale.
    pub fn remove(&mut self, h: Handle) -> Option<T> {
        let slot = self
            .slots
            .get_mut(h.index())
            .filter(|s| s.generation == h.generation)?;
        let value = slot.value.take()?;
        slot.generation = slot.generation.wrapping_add(1);
        self.len -= 1;
        Some(value)
    }

    pub fn clear(&mut self) {
        for slot in &mut self.slots {
            if slot.value.take().is_some() {
                slot.generation = slot.generation.wrapping_add(1);
            }
        }
        self.len = 0;
    }

    pub fn iter(&self) -> impl Iterator<Item = (Handle, &T)> {
        self.slots.iter().enumerate().filter_map(|(i, s)| {
            s.value.as_ref().map(|v| {
                (
                    Handle {
                        index: i as u16,
                        generation: s.generation,
                    },
                    v,
                )
            })
        })
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = (Handle, &mut T)> {
        self.slots.iter_mut().enumerate().filter_map(|(i, s)| {
            let generation = s.generation;
            s.value.as_mut().map(|v| {
                (
                    Handle {
                        index: i as u16,
                        generation,
                    },
                    v,
                )
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[repr(align(16))]
    struct Backing([u8; 256]);

    #[test]
    fn scopes_bump_align_and_release() {
        let mut backing = Backing([0; 256]);
        let mut arena = Arena::new(&mut backing.0);
        arena.scope(|s| {
            let a = s.alloc_bytes(3, 1).unwrap();
            a.copy_from_slice(&[1, 2, 3]);
            let b = s.alloc(0x1122_3344_u32).unwrap();
            assert_eq!(b as *mut u32 as usize % 4, 0);
            assert_eq!(*b, 0x1122_3344);
            assert_eq!(s.used(), 8, "one byte of padding before the u32");
            let v = s.alloc_slice(4, 7u16).unwrap();
            assert_eq!(v, &[7, 7, 7, 7]);
            let big = s.alloc_bytes(16, ARENA_ALIGN).unwrap();
            assert_eq!(big.as_ptr() as usize % ARENA_ALIGN, 0);
            assert_eq!(a, &[1, 2, 3], "earlier allocations stay live");
        });
        assert_eq!(arena.high_water(), 32);
        // The next scope starts from empty again.
        let fits = arena.scope(|s| s.alloc_bytes(256, 1).is_some());
        assert!(fits);
        assert_eq!(arena.high_water(), 256);
    }

    #[test]
    fn a_full_arena_returns_none() {
        let mut backing = Backing([0; 256]);
        let mut arena = Arena::new(&mut backing.0[..10]);
        arena.scope(|s| {
            assert!(s.alloc_bytes(8, 1).is_some());
            assert!(s.alloc_bytes(3, 1).is_none());
            assert!(s.alloc(0u64).is_none());
            assert_eq!(s.remaining(), 2);
        });
    }

    #[test]
    fn pool_fills_reuses_and_invalidates_stale_handles() {
        let mut pool: ObjectPool<&str, 2> = ObjectPool::new();
        let a = pool.insert("a").unwrap();
        let b = pool.insert("b").unwrap();
        assert_eq!(pool.insert("c"), Err("c"));
        assert_eq!(pool.remove(a), Some("a"));
        assert_eq!(pool.get(a), None);
        let c = pool.insert("c").unwrap();
        assert_eq!(c.index(), a.index(), "the lowest free slot is reused");
        assert_eq!(pool.get(a), None, "the old handle stays stale");
        assert_eq!(pool.get(c), Some(&"c"));
        *pool.get_mut(b).unwrap() = "B";
        let order: [&str; 2] = {
            let mut it = pool.iter().map(|(_, v)| *v);
            [it.next().unwrap(), it.next().unwrap()]
        };
        assert_eq!(order, ["c", "B"]);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.get(b), None);
    }
}
