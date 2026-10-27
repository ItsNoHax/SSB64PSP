//! The allocators planned in `docs/memory.md`: scoped bump arenas, a
//! fixed-capacity object pool, and the scene loader's block arena. `no_std`.
//! [`Arena`] and [`ObjectPool`] use no `alloc`: an arena borrows its storage,
//! which the platform layer provides (a static, or one block from the PSP
//! heap).
//!
//! The original loads a scene's whole `relocData` closure into one
//! allocation sized up front (`lbRelocGetAllocSize` then
//! `lbRelocLoadFilesExtern`) and frees it in one go. [`Arena`] keeps that
//! shape for game state: allocations are pointer bumps inside
//! [`Arena::scope`], and the whole scope is released when it returns.
//!
//! | Allocator | Holds | Lifetime |
//! |---|---|---|
//! | [`AssetArena`] | a scene's archive files: one block per read run (RE-571) | a block lives while any of its files is held |
//! | [`GameArena`] | long-lived game state | one scope per scene |
//! | [`FrameArena`] | per-frame scratch, so per-frame paths never touch the heap | one scope per tick |
//!
//! [`ObjectPool`] is the fixed-capacity store for fighters, items and
//! particles, whose counts the original also fixes.
//!
//! [`AssetArena`] serves `ssb_rom::loader_plan` (the PSP's
//! `scene_files`); [`GameArena`] and [`FrameArena`] are not used yet.

use alloc::vec::Vec;
use core::mem::{align_of, size_of, MaybeUninit};

/// Default alignment: `LBRELOC_CACHE_ALIGN`'s 16 bytes, which is also what
/// the GE wants for DMA sources.
pub const ARENA_ALIGN: usize = 16;

/// A bump allocator over borrowed storage. See the module docs.
pub struct Arena<'a> {
    buf: &'a mut [u8],
    high_water: usize,
}

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

/// Where [`AssetArena`]'s blocks come from: the platform's allocator. A
/// block is freed by dropping it.
pub trait BlockAlloc {
    type Block;
    /// A block of at least `len` bytes, or `None` when memory is short.
    fn alloc(&mut self, len: usize) -> Option<Self::Block>;
    /// `block`'s first byte.
    fn ptr(block: &Self::Block) -> *mut u8;
}

/// An [`AssetArena`] block's handle.
pub type BlockId = u16;
/// No block.
pub const NO_BLOCK: BlockId = BlockId::MAX;

struct BlockSlot<B> {
    block: B,
    /// Bytes asked for.
    len: usize,
    /// Files in the block still held.
    live: u32,
    /// Its files are registered ([`AssetArena::commit`]); until then a
    /// read may be filling it.
    committed: bool,
    /// The scene scope ([`AssetArena::begin_scene`]) that allocated it.
    scene: u32,
}

/// The scene loader's blocks (RE-475, D-046): one block per read run,
/// handed out per scene scope and owned here until freed.
///
/// * [`alloc`](Self::alloc): a block for one read run, pending until its
///   files are registered ([`commit`](Self::commit), with how many).
/// * [`release`](Self::release): one of its files dropped; the last one
///   retires the block. A file kept into the next scene keeps its block
///   alive, whichever scene allocated it.
/// * Retired blocks are freed by [`collect`](Self::collect) once no
///   display list can read them (a GE safe point); a pending block no read
///   reached is freed at once ([`free`](Self::free)).
///
/// The bookkeeping is portable; the platform supplies the memory
/// ([`BlockAlloc`]: on the PSP, 64-byte aligned whole cache lines, RE-476).
pub struct AssetArena<A: BlockAlloc> {
    alloc: A,
    slots: Vec<Option<BlockSlot<A::Block>>>,
    retired: Vec<A::Block>,
    /// Bytes in slots (pending and committed), and in retired blocks.
    live_bytes: usize,
    retired_bytes: usize,
    high_water: usize,
    scene: u32,
    scene_high_water: usize,
}

impl<A: BlockAlloc> AssetArena<A> {
    pub fn new(alloc: A) -> Self {
        Self {
            alloc,
            slots: Vec::new(),
            retired: Vec::new(),
            live_bytes: 0,
            retired_bytes: 0,
            high_water: 0,
            scene: 0,
            scene_high_water: 0,
        }
    }

    /// Starts a scene scope: blocks allocated from now on are the new
    /// scene's; [`scene_high_water`](Self::scene_high_water) restarts.
    pub fn begin_scene(&mut self) {
        self.scene = self.scene.wrapping_add(1);
        self.scene_high_water = self.live_bytes + self.retired_bytes;
    }

    /// A pending block of `len` bytes, or `None` when the allocator or the
    /// handle space is exhausted. Reuses the lowest free handle.
    pub fn alloc(&mut self, len: usize) -> Option<BlockId> {
        let slot = match self.slots.iter().position(Option::is_none) {
            Some(s) => s,
            None if self.slots.len() < usize::from(NO_BLOCK) => {
                self.slots.try_reserve(1).ok()?;
                self.slots.push(None);
                self.slots.len() - 1
            }
            None => return None,
        };
        let block = self.alloc.alloc(len)?;
        self.slots[slot] = Some(BlockSlot {
            block,
            len,
            live: 0,
            committed: false,
            scene: self.scene,
        });
        self.live_bytes += len;
        self.note();
        Some(slot as BlockId)
    }

    fn note(&mut self) {
        let held = self.live_bytes + self.retired_bytes;
        self.high_water = self.high_water.max(held);
        self.scene_high_water = self.scene_high_water.max(held);
    }

    fn slot(&self, id: BlockId) -> Option<&BlockSlot<A::Block>> {
        self.slots.get(usize::from(id))?.as_ref()
    }

    /// Block `id`, while it is allocated and not retired.
    pub fn block(&self, id: BlockId) -> Option<&A::Block> {
        self.slot(id).map(|s| &s.block)
    }

    /// Block `id`'s first byte.
    pub fn ptr(&self, id: BlockId) -> Option<*mut u8> {
        self.block(id).map(A::ptr)
    }

    /// Bytes block `id` was asked for.
    pub fn len(&self, id: BlockId) -> Option<usize> {
        self.slot(id).map(|s| s.len)
    }

    /// Files still held in block `id`.
    pub fn live(&self, id: BlockId) -> Option<u32> {
        self.slot(id).map(|s| s.live)
    }

    /// Block `id` now holds `live` registered files. A block with none
    /// retires at once.
    pub fn commit(&mut self, id: BlockId, live: u32) {
        let Some(Some(s)) = self.slots.get_mut(usize::from(id)) else {
            return;
        };
        s.committed = true;
        s.live = live;
        if live == 0 {
            self.retire(id);
        }
    }

    /// One of block `id`'s files dropped. Returns whether that retired the
    /// block (its last file).
    pub fn release(&mut self, id: BlockId) -> bool {
        let Some(Some(s)) = self.slots.get_mut(usize::from(id)) else {
            return false;
        };
        s.live = s.live.saturating_sub(1);
        if s.live > 0 {
            return false;
        }
        self.retire(id);
        true
    }

    fn retire(&mut self, id: BlockId) {
        if let Some(s) = self.slots.get_mut(usize::from(id)).and_then(Option::take) {
            self.live_bytes -= s.len;
            self.retired_bytes += s.len;
            self.retired.push(s.block);
        }
    }

    /// Frees pending block `id` now: no file of it was registered, so no
    /// display list can read it (a cancelled or failed read).
    pub fn free(&mut self, id: BlockId) {
        if let Some(s) = self.slots.get_mut(usize::from(id)).and_then(Option::take) {
            debug_assert!(!s.committed, "a registered block retires instead");
            self.live_bytes -= s.len;
        }
    }

    /// Blocks retired and not yet freed.
    pub fn retired(&self) -> usize {
        self.retired.len()
    }

    /// Frees the retired blocks; call once the GE has read everything.
    /// Returns how many it freed.
    pub fn collect(&mut self) -> usize {
        let n = self.retired.len();
        self.retired.clear();
        self.retired_bytes = 0;
        n
    }

    /// Bytes and count of the blocks holding registered files.
    pub fn committed(&self) -> (usize, usize) {
        self.slots
            .iter()
            .flatten()
            .filter(|s| s.committed)
            .fold((0, 0), |(b, n), s| (b + s.len, n + 1))
    }

    /// Bytes and count of committed blocks an earlier scene scope
    /// allocated: what files kept across scenes pin.
    pub fn pinned(&self) -> (usize, usize) {
        self.slots
            .iter()
            .flatten()
            .filter(|s| s.committed && s.scene != self.scene)
            .fold((0, 0), |(b, n), s| (b + s.len, n + 1))
    }

    /// Bytes in blocks not yet freed (pending, committed and retired).
    pub fn live_bytes(&self) -> usize {
        self.live_bytes + self.retired_bytes
    }

    /// The most [`live_bytes`](Self::live_bytes) has been.
    pub fn high_water(&self) -> usize {
        self.high_water
    }

    /// The most [`live_bytes`](Self::live_bytes) has been in this scene
    /// scope.
    pub fn scene_high_water(&self) -> usize {
        self.scene_high_water
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

    /// A heap block that counts its frees.
    struct Counted(alloc::vec::Vec<u8>, std::rc::Rc<core::cell::Cell<usize>>);
    impl Drop for Counted {
        fn drop(&mut self) {
            self.1.set(self.1.get() + 1);
        }
    }

    /// `budget` bytes of fake heap.
    struct Fake {
        budget: usize,
        freed: std::rc::Rc<core::cell::Cell<usize>>,
    }
    impl BlockAlloc for Fake {
        type Block = Counted;
        fn alloc(&mut self, len: usize) -> Option<Counted> {
            self.budget = self.budget.checked_sub(len)?;
            Some(Counted(alloc::vec![0; len.max(1)], self.freed.clone()))
        }
        fn ptr(block: &Counted) -> *mut u8 {
            block.0.as_ptr() as *mut u8
        }
    }

    fn asset_arena(budget: usize) -> (AssetArena<Fake>, std::rc::Rc<core::cell::Cell<usize>>) {
        let freed = std::rc::Rc::new(core::cell::Cell::new(0));
        let arena = AssetArena::new(Fake {
            budget,
            freed: freed.clone(),
        });
        (arena, freed)
    }

    #[test]
    fn asset_blocks_retire_with_their_last_file_and_free_at_collect() {
        let (mut a, freed) = asset_arena(1 << 20);
        a.begin_scene();
        let b0 = a.alloc(100).unwrap();
        let b1 = a.alloc(300).unwrap();
        assert_eq!((b0, b1), (0, 1));
        assert_ne!(a.ptr(b0), a.ptr(b1));
        // Pending blocks are not counted as holding files.
        assert_eq!(a.committed(), (0, 0));
        a.commit(b0, 2);
        a.commit(b1, 1);
        assert_eq!(a.committed(), (400, 2));
        assert_eq!(a.live(b0), Some(2));
        assert!(!a.release(b0));
        assert!(a.release(b1));
        // Retired: gone from the table, still allocated until collected.
        assert_eq!(a.block(b1).map(|_| ()), None);
        assert_eq!(a.committed(), (100, 1));
        assert_eq!((a.retired(), a.live_bytes(), freed.get()), (1, 400, 0));
        assert_eq!(a.collect(), 1);
        assert_eq!((a.retired(), a.live_bytes(), freed.get()), (0, 100, 1));
        // The freed handle is reused first.
        assert_eq!(a.alloc(50), Some(b1));
        assert!(a.release(b0));
        a.collect();
        assert_eq!(freed.get(), 2);
        assert_eq!(a.high_water(), 400);
    }

    #[test]
    fn kept_files_pin_an_earlier_scenes_block() {
        let (mut a, _) = asset_arena(1 << 20);
        a.begin_scene();
        let old = a.alloc(1000).unwrap();
        a.commit(old, 3);
        a.begin_scene();
        assert_eq!(a.scene_high_water(), 1000);
        let new = a.alloc(200).unwrap();
        a.commit(new, 1);
        // Two of the old scene's files leave; one is kept.
        a.release(old);
        a.release(old);
        assert_eq!(a.pinned(), (1000, 1));
        assert_eq!(a.committed(), (1200, 2));
        assert!(a.release(old));
        assert_eq!(a.pinned(), (0, 0));
        assert_eq!(a.scene_high_water(), 1200);
    }

    #[test]
    fn pending_blocks_free_at_once_and_exhaustion_is_none() {
        let (mut a, freed) = asset_arena(500);
        let b = a.alloc(400).unwrap();
        assert_eq!(a.alloc(200), None, "the fake heap has 100 bytes left");
        a.free(b);
        assert_eq!((freed.get(), a.live_bytes(), a.retired()), (1, 0, 0));
        // A block whose read registered no file retires on commit.
        let (mut a, freed) = asset_arena(500);
        let b = a.alloc(10).unwrap();
        a.commit(b, 0);
        assert_eq!((a.retired(), a.collect(), freed.get()), (1, 1, 1));
    }
}
