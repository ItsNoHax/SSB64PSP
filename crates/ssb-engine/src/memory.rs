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
//! a [`Scope`] (from [`Arena::begin`] or [`Arena::scope`]), and the whole
//! scope is released when it drops. [`Scope::checkpoint`] nests a scope
//! that rewinds its parent when dropped.
//!
//! | Allocator | Holds | Lifetime |
//! |---|---|---|
//! | [`AssetArena`] | a scene's archive files: one block per read run (RE-571) | a block lives while any of its files is held |
//! | [`GameArena`] | a scene's general heap (`syTaskmanMalloc`): its long-lived state | one block per scene |
//! | [`FrameArena`] | per-frame scratch, so per-frame paths never touch the heap | one scope per tick |
//!
//! [`ObjectPool`] is the fixed-capacity store for items and weapons
//! (`ssb_game::item::ItemPool`, `ssb_game::weapon::WeaponPool`), whose
//! counts the original also fixes. [`FreeOrder::Lifo`] reuses slots in the
//! order of `itManagerInitItems` and `wpManagerAllocWeapons`' free lists.
//!
//! [`AssetArena`] serves `ssb_rom::loader_plan` (the PSP's
//! `scene_files`); [`FrameArena`] serves the opening movie's camera draw
//! order; [`GameArena`] serves the PSP battle scenes' world.

use alloc::vec::Vec;
use core::mem::{align_of, needs_drop, size_of, MaybeUninit};

mod game;
pub use game::{scene_bytes, GameArena, GameArenaError, GameArenaUsage, SceneBox};

/// Default alignment: `LBRELOC_CACHE_ALIGN`'s 16 bytes, which is also what
/// the GE wants for DMA sources.
pub const ARENA_ALIGN: usize = 16;

/// A bump allocator over borrowed storage. See the module docs.
pub struct Arena<'a> {
    buf: &'a mut [u8],
    high_water: usize,
}

/// Per-tick scratch.
pub type FrameArena<'a> = Arena<'a>;

impl<'a> Arena<'a> {
    pub fn new(buf: &'a mut [u8]) -> Self {
        Arena { buf, high_water: 0 }
    }

    /// An arena over `len` bytes at `ptr`: a block the platform layer owns
    /// (a static, or one heap block), normally 64-byte aligned.
    ///
    /// # Safety
    ///
    /// `ptr` is valid for reads and writes of `len` bytes for `'a`, and
    /// nothing else accesses them while the arena lives.
    pub unsafe fn from_aligned(ptr: *mut u8, len: usize) -> Self {
        assert!(
            (ptr as usize).is_multiple_of(ARENA_ALIGN),
            "arena storage below ARENA_ALIGN"
        );
        // SAFETY: the caller's contract.
        Arena::new(unsafe { core::slice::from_raw_parts_mut(ptr, len) })
    }

    pub fn capacity(&self) -> usize {
        self.buf.len()
    }

    /// The most any scope has used, alignment padding included, for sizing
    /// the backing block from real workloads.
    pub fn high_water(&self) -> usize {
        self.high_water
    }

    /// Opens an empty scope that lasts until it is dropped (a scene, a
    /// tick). Its allocations borrow the arena, so the next `begin` waits
    /// until none of them is live.
    pub fn begin(&mut self) -> Scope<'_> {
        Scope {
            rest: &mut self.buf[..],
            used: 0,
            high_water: &mut self.high_water,
        }
    }

    /// Runs `f` with an empty scope. Everything allocated from it is
    /// released when `f` returns; the borrow checker keeps any allocation
    /// from outliving the scope.
    pub fn scope<R>(&mut self, f: impl for<'s> FnOnce(&mut Scope<'s>) -> R) -> R {
        f(&mut self.begin())
    }
}

/// One scope of an [`Arena`]: allocations live until the scope ends.
///
/// Scopes run no destructors, so only types without drop glue go in
/// (`T: !needs_drop`, checked when the call is compiled):
///
/// ```compile_fail,E0080
/// # use ssb_engine::memory::Arena;
/// let mut buf = [0u8; 64];
/// let mut arena = Arena::new(&mut buf);
/// let mut scope = arena.begin();
/// scope.alloc(Vec::<u8>::new()); // a `Vec` needs its destructor
/// ```
pub struct Scope<'s> {
    rest: &'s mut [u8],
    /// Bytes from the arena's start, padding included.
    used: usize,
    high_water: &'s mut usize,
}

impl Drop for Scope<'_> {
    fn drop(&mut self) {
        *self.high_water = (*self.high_water).max(self.used);
    }
}

impl<'s> Scope<'s> {
    /// Bytes used so far from the arena's start, alignment padding
    /// included (a [`checkpoint`](Self::checkpoint) scope counts its
    /// parent's too).
    pub fn used(&self) -> usize {
        self.used
    }

    pub fn remaining(&self) -> usize {
        self.rest.len()
    }

    /// A nested scope from this one's current end. Dropping it rewinds
    /// this scope to here: its allocations borrow this scope, so none can
    /// outlive the rewind.
    pub fn checkpoint(&mut self) -> Scope<'_> {
        Scope {
            rest: &mut *self.rest,
            used: self.used,
            high_water: &mut *self.high_water,
        }
    }

    /// Ends the scope; the same as dropping it.
    pub fn rewind(self) {}

    /// `len` bytes aligned to `align` (a power of two), or `None` when the
    /// arena is full. The contents are whatever an earlier scope left.
    pub fn alloc_bytes(&mut self, len: usize, align: usize) -> Option<&'s mut [u8]> {
        let pad = self.pad(align);
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

    fn pad(&self, align: usize) -> usize {
        debug_assert!(align.is_power_of_two());
        (self.rest.as_ptr() as usize).wrapping_neg() & (align - 1)
    }

    /// Moves `value` into the arena.
    pub fn alloc<T>(&mut self, value: T) -> Option<&'s mut T> {
        const { assert!(!needs_drop::<T>(), "arena values run no destructor") };
        let bytes = self.alloc_bytes(size_of::<T>(), align_of::<T>())?;
        let slot = bytes.as_mut_ptr().cast::<MaybeUninit<T>>();
        // SAFETY: `bytes` is exclusively borrowed for `'s`, `size_of::<T>()`
        // long and aligned for `T`, so it is a valid `MaybeUninit<T>`;
        // writing `value` initialises it.
        Some(unsafe { (*slot).write(value) })
    }

    /// `len` copies of `value`, contiguous.
    pub fn alloc_slice<T: Copy>(&mut self, len: usize, value: T) -> Option<&'s mut [T]> {
        self.alloc_slice_with(len, |_| value)
    }

    /// `[f(0), f(1), .., f(len - 1)]`, contiguous.
    pub fn alloc_slice_with<T>(
        &mut self,
        len: usize,
        mut f: impl FnMut(usize) -> T,
    ) -> Option<&'s mut [T]> {
        const { assert!(!needs_drop::<T>(), "arena values run no destructor") };
        let bytes = self.alloc_bytes(size_of::<T>().checked_mul(len)?, align_of::<T>())?;
        let first = bytes.as_mut_ptr().cast::<T>();
        // SAFETY: as in `alloc`, for `len` elements; every element is
        // written before the slice is formed.
        unsafe {
            for i in 0..len {
                first.add(i).write(f(i));
            }
            Some(core::slice::from_raw_parts_mut(first, len))
        }
    }

    /// The iterator's items, contiguous, or `None` when they do not all
    /// fit (the scope is then unchanged). The length need not be known up
    /// front: items are written at the scope's end as they come.
    pub fn alloc_from_iter<T>(
        &mut self,
        items: impl IntoIterator<Item = T>,
    ) -> Option<&'s mut [T]> {
        const { assert!(!needs_drop::<T>(), "arena values run no destructor") };
        let pad = self.pad(align_of::<T>());
        if pad > self.rest.len() {
            return None;
        }
        let room = self.rest.len() - pad;
        // SAFETY: `pad` is in bounds; the elements written below stay
        // inside `rest[pad..]`.
        let first = unsafe { self.rest.as_mut_ptr().add(pad) }.cast::<T>();
        let mut len = 0usize;
        for item in items {
            if size_of::<T>().checked_mul(len + 1)? > room {
                return None;
            }
            // SAFETY: in bounds by the check above and aligned by `pad`;
            // `rest` is exclusively borrowed, so nothing aliases it.
            unsafe { first.add(len).write(item) };
            len += 1;
        }
        let out = self.alloc_bytes(size_of::<T>() * len, align_of::<T>())?;
        debug_assert_eq!(out.as_mut_ptr().cast::<T>(), first);
        // SAFETY: `out` is the bytes just written: `len` initialised `T`s.
        Some(unsafe { core::slice::from_raw_parts_mut(out.as_mut_ptr().cast::<T>(), len) })
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

    /// The slot's serial when the handle was made.
    pub fn generation(self) -> u16 {
        self.generation
    }

    /// `index | generation << 16`: the item pool's `u32` handle encoding.
    pub fn to_bits(self) -> u32 {
        u32::from(self.index) | (u32::from(self.generation) << 16)
    }

    pub fn from_bits(bits: u32) -> Self {
        Handle {
            index: bits as u16,
            generation: (bits >> 16) as u16,
        }
    }
}

/// Which free slot [`ObjectPool::insert`] takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FreeOrder {
    /// The lowest free index.
    LowestFirst,
    /// The last one freed; slot 0 first from a fresh pool. The original's
    /// item and weapon free lists (`itManagerInitItems` links struct `i`
    /// to `i + 1`; `itManagerGetNextStructAlloc` pops the head,
    /// `itManagerSetPrevStructAlloc` pushes a freed struct on it; `wpmanager.c`
    /// does the same).
    Lifo,
}

/// At most `N` live objects, no heap. Iteration visits slots in index
/// order, the order the original's fixed arrays process them.
///
/// Values, serials and the free list are three arrays, so a `T` with a
/// wider alignment pays no per-slot padding for its serial.
#[derive(Debug, Clone, PartialEq)]
pub struct ObjectPool<T, const N: usize> {
    values: [Option<T>; N],
    /// Each slot's serial, bumped by every insert.
    serials: [u16; N],
    /// [`FreeOrder::Lifo`]'s free list: `free[free_len - 1]` goes next.
    free: [u16; N],
    free_len: u16,
    len: u16,
    order: FreeOrder,
}

impl<T, const N: usize> Default for ObjectPool<T, N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T, const N: usize> ObjectPool<T, N> {
    /// A pool that fills its lowest free slot first.
    pub fn new() -> Self {
        Self::with_order(FreeOrder::LowestFirst)
    }

    pub fn with_order(order: FreeOrder) -> Self {
        assert!(
            N <= usize::from(u16::MAX),
            "pool capacity exceeds u16 handles"
        );
        ObjectPool {
            values: core::array::from_fn(|_| None),
            serials: [0; N],
            free: core::array::from_fn(|i| (N - 1 - i) as u16),
            free_len: N as u16,
            len: 0,
            order,
        }
    }

    pub const fn capacity(&self) -> usize {
        N
    }

    pub fn order(&self) -> FreeOrder {
        self.order
    }

    pub fn len(&self) -> usize {
        usize::from(self.len)
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn is_full(&self) -> bool {
        usize::from(self.len) == N
    }

    /// Stores `value` in the free slot [`FreeOrder`] picks, or gives it
    /// back when full. The slot's serial goes up by one.
    pub fn insert(&mut self, value: T) -> Result<Handle, T> {
        let index = match self.order {
            FreeOrder::LowestFirst => self.values.iter().position(Option::is_none),
            FreeOrder::Lifo => self.free_len.checked_sub(1).map(|top| {
                self.free_len = top;
                usize::from(self.free[usize::from(top)])
            }),
        };
        let Some(index) = index else {
            return Err(value);
        };
        let serial = self.serials[index].wrapping_add(1);
        self.serials[index] = serial;
        self.values[index] = Some(value);
        self.len += 1;
        Ok(Handle {
            index: index as u16,
            generation: serial,
        })
    }

    fn live(&self, h: Handle) -> bool {
        self.serials.get(h.index()) == Some(&h.generation)
    }

    pub fn get(&self, h: Handle) -> Option<&T> {
        if !self.live(h) {
            return None;
        }
        self.values[h.index()].as_ref()
    }

    pub fn get_mut(&mut self, h: Handle) -> Option<&mut T> {
        if !self.live(h) {
            return None;
        }
        self.values[h.index()].as_mut()
    }

    /// Every slot in index order, empty ones as `None`.
    pub fn values(&self) -> &[Option<T>; N] {
        &self.values
    }

    /// Every slot in index order, mutably, empty ones as `None`. Filling
    /// or emptying a slot goes through [`Self::insert`] and
    /// [`Self::remove_at`].
    pub fn values_mut(&mut self) -> impl Iterator<Item = Option<&mut T>> {
        self.values.iter_mut().map(Option::as_mut)
    }

    /// The object in slot `index`, whatever its serial.
    pub fn slot_value(&self, index: usize) -> Option<&T> {
        self.values.get(index)?.as_ref()
    }

    pub fn slot_value_mut(&mut self, index: usize) -> Option<&mut T> {
        self.values.get_mut(index)?.as_mut()
    }

    /// Slot `index`'s serial: how many inserts it has taken, wrapping.
    pub fn serial(&self, index: usize) -> Option<u16> {
        self.serials.get(index).copied()
    }

    /// The handle of the object in slot `index`, if one is there.
    pub fn handle_at(&self, index: usize) -> Option<Handle> {
        self.values.get(index)?.as_ref()?;
        Some(Handle {
            index: index as u16,
            generation: self.serials[index],
        })
    }

    /// Removes the object; its handle, and every copy of it, goes stale.
    pub fn remove(&mut self, h: Handle) -> Option<T> {
        if !self.live(h) {
            return None;
        }
        self.remove_at(h.index())
    }

    /// Removes the object in slot `index`, whatever its serial.
    pub fn remove_at(&mut self, index: usize) -> Option<T> {
        let value = self.values.get_mut(index)?.take()?;
        self.len -= 1;
        if self.order == FreeOrder::Lifo {
            self.free[usize::from(self.free_len)] = index as u16;
            self.free_len += 1;
        }
        Some(value)
    }

    /// Empties the pool. Serials are kept, so old handles stay stale; the
    /// free list starts again from slot 0, as the original's is rebuilt at
    /// each battle (`itManagerInitItems`).
    pub fn clear(&mut self) {
        for value in &mut self.values {
            *value = None;
        }
        self.len = 0;
        self.free = core::array::from_fn(|i| (N - 1 - i) as u16);
        self.free_len = N as u16;
    }

    /// [`Self::clear`] and serials back to zero: the pool as
    /// [`Self::with_order`] made it, without a temporary.
    pub fn reset(&mut self) {
        self.clear();
        self.serials = [0; N];
    }

    pub fn iter(&self) -> impl Iterator<Item = (Handle, &T)> {
        let serials = &self.serials;
        self.values.iter().enumerate().filter_map(move |(i, v)| {
            v.as_ref().map(|v| {
                (
                    Handle {
                        index: i as u16,
                        generation: serials[i],
                    },
                    v,
                )
            })
        })
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = (Handle, &mut T)> {
        let serials = &self.serials;
        self.values
            .iter_mut()
            .enumerate()
            .filter_map(move |(i, v)| {
                v.as_mut().map(|v| {
                    (
                        Handle {
                            index: i as u16,
                            generation: serials[i],
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

    #[test]
    fn a_begun_scope_lives_until_dropped() {
        let mut backing = Backing([0; 256]);
        let mut arena = Arena::new(&mut backing.0);
        let first = {
            let mut s = arena.begin();
            let a = s.alloc(1u32).unwrap();
            let b = s.alloc_slice(3, 2u8).unwrap();
            *a += 1;
            assert_eq!((*a, &*b), (2, &[2u8, 2, 2][..]));
            a as *mut u32 as usize
        };
        assert_eq!(arena.high_water(), 7);
        // Dropped: the next scope starts at the same place.
        let mut s = arena.begin();
        assert_eq!(s.alloc(0u32).map(|r| r as *mut u32 as usize), Some(first));
    }

    #[test]
    fn a_checkpoint_rewinds_its_parent() {
        let mut backing = Backing([0; 256]);
        let mut arena = Arena::new(&mut backing.0);
        let mut tick = arena.begin();
        let kept = tick.alloc(7u64).unwrap();
        let at = {
            let mut scratch = tick.checkpoint();
            let tmp = scratch.alloc_slice(4, 9u32).unwrap();
            assert_eq!(scratch.used(), 24);
            tmp.as_ptr() as usize
        };
        assert_eq!(tick.used(), 8, "the checkpoint's bytes are back");
        let again = tick.alloc(1u32).unwrap();
        assert_eq!(again as *mut u32 as usize, at);
        assert_eq!(*kept, 7);
        let mut scratch = tick.checkpoint();
        scratch.alloc_bytes(100, 1).unwrap();
        scratch.rewind();
        assert_eq!(tick.remaining(), 256 - 12);
        drop(tick);
        assert_eq!(arena.high_water(), 112, "the deepest checkpoint counts");
    }

    /// Not `Copy`, but nothing to drop.
    #[derive(Debug, PartialEq)]
    struct Pair {
        id: u16,
        name: &'static str,
    }

    #[test]
    fn values_without_drop_glue_need_not_be_copy() {
        let mut backing = Backing([0; 256]);
        let mut arena = Arena::new(&mut backing.0);
        arena.scope(|s| {
            let p = s.alloc(Pair { id: 1, name: "a" }).unwrap();
            assert_eq!(p.name, "a");
            let v = s
                .alloc_slice_with(3, |i| Pair {
                    id: i as u16,
                    name: "w",
                })
                .unwrap();
            assert_eq!(v[2], Pair { id: 2, name: "w" });
            let names = ["x", "y"];
            let it = s
                .alloc_from_iter(names.iter().enumerate().map(|(i, &name)| Pair {
                    id: 10 + i as u16,
                    name,
                }))
                .unwrap();
            assert_eq!(
                it,
                &[Pair { id: 10, name: "x" }, Pair { id: 11, name: "y" }]
            );
            assert_eq!(it.as_ptr() as usize % align_of::<Pair>(), 0);
            // Borrows of locals go into a checkpoint, whose lifetime is
            // the local borrow's, not the closure scope's.
            let local = [1u8, 2];
            let mut inner = s.checkpoint();
            let refs = inner.alloc_from_iter(local.iter()).unwrap();
            assert_eq!(*refs[1], 2);
            drop(inner);
            // Too many: nothing is taken.
            let left = s.remaining();
            assert!(s.alloc_from_iter(0..u64::MAX).is_none());
            assert_eq!(s.remaining(), left);
        });
        assert!(!needs_drop::<Pair>());
    }

    #[test]
    fn an_arena_over_raw_storage() {
        let mut backing = Backing([0; 256]);
        // SAFETY: `backing` outlives the arena and is used through it only.
        let mut arena = unsafe { Arena::from_aligned(backing.0.as_mut_ptr(), 64) };
        assert_eq!(arena.capacity(), 64);
        assert!(arena.scope(|s| s.alloc_bytes(65, 1).is_none()));
    }

    #[test]
    fn a_lifo_pool_reuses_the_last_freed_slot() {
        let mut pool: ObjectPool<u8, 3> = ObjectPool::with_order(FreeOrder::Lifo);
        let h: [Handle; 3] = core::array::from_fn(|i| pool.insert(i as u8).unwrap());
        assert_eq!(
            h.map(Handle::index),
            [0, 1, 2],
            "struct 0 is handed out first"
        );
        assert_eq!(pool.insert(9), Err(9));
        pool.remove(h[0]).unwrap();
        pool.remove(h[2]).unwrap();
        let a = pool.insert(20).unwrap();
        let b = pool.insert(21).unwrap();
        assert_eq!((a.index(), b.index()), (2, 0), "last freed, first reused");
        // The other order on the same sequence.
        let mut low: ObjectPool<u8, 3> = ObjectPool::new();
        let g: [Handle; 3] = core::array::from_fn(|i| low.insert(i as u8).unwrap());
        low.remove(g[0]).unwrap();
        low.remove(g[2]).unwrap();
        assert_eq!(low.insert(20).unwrap().index(), 0);
        pool.clear();
        assert_eq!(
            pool.insert(1).unwrap().index(),
            0,
            "clear rebuilds the list"
        );
    }

    #[test]
    fn a_reused_slot_leaves_old_handles_stale() {
        for order in [FreeOrder::LowestFirst, FreeOrder::Lifo] {
            let mut pool: ObjectPool<u32, 2> = ObjectPool::with_order(order);
            let a = pool.insert(1).unwrap();
            assert_eq!(a.to_bits(), 1 << 16, "slot 0, first serial 1");
            assert_eq!(Handle::from_bits(a.to_bits()), a);
            assert_eq!(pool.serial(0), Some(1));
            pool.remove(a).unwrap();
            assert_eq!(pool.get(a), None);
            assert_eq!(pool.remove(a), None);
            let b = pool.insert(2).unwrap();
            assert_eq!(b.index(), a.index());
            assert_eq!(b.generation(), 2);
            assert_eq!(pool.get(a), None, "{order:?}: the old handle stays stale");
            assert_eq!(pool.get_mut(a), None);
            assert_eq!(pool.get(b), Some(&2));
            assert_eq!(pool.slot_value(0), Some(&2));
            *pool.slot_value_mut(0).unwrap() = 3;
            assert_eq!(pool.get(b), Some(&3));
            assert_eq!(pool.slot_value(1), None);
            assert_eq!(pool.serial(2), None);
            let copy = pool.clone();
            assert_eq!(copy, pool);
            pool.clear();
            assert_eq!(pool.get(b), None, "clear leaves handles stale");
            assert_ne!(copy, pool);
        }
    }

    #[test]
    fn a_pool_removes_by_slot_and_resets_its_serials() {
        let mut pool: ObjectPool<u8, 3> = ObjectPool::with_order(FreeOrder::Lifo);
        let a = pool.insert(1).unwrap();
        let b = pool.insert(2).unwrap();
        assert_eq!(pool.handle_at(1), Some(b));
        assert_eq!(pool.handle_at(2), None);
        assert_eq!(pool.values(), &[Some(1), Some(2), None]);
        for v in pool.values_mut().flatten() {
            *v += 10;
        }
        assert_eq!(pool.remove_at(0), Some(11));
        assert_eq!(pool.remove_at(0), None, "an empty slot frees nothing");
        assert_eq!(pool.get(a), None);
        assert_eq!(pool.len(), 1);
        assert_eq!(pool.insert(3).unwrap().index(), 0, "slot 0 was freed last");
        assert!(!pool.is_full());
        pool.insert(4).unwrap();
        assert!(pool.is_full());
        pool.reset();
        assert_eq!(pool, ObjectPool::with_order(FreeOrder::Lifo));
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
