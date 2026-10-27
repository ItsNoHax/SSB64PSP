//! [`GameArena`]: one scene's general heap, the original's
//! `gSYTaskmanGeneralHeap`.
//!
//! Each scene's `syTaskmanLoadScene` hands `syTaskmanInitGeneralHeap` the
//! scene's arena (`taskman.c:1220`, `:1231`), and the scene's objects come
//! out of it by `syTaskmanMalloc` (`syMallocSet`, `malloc.c:12-33`): a
//! pointer bump that never frees, and a hang ("ml : alloc overflow") when
//! the scene asks for more than its arena holds. The next scene starts a
//! new arena.
//!
//! Here a scene's arena is one block from a [`BlockAlloc`], taken when the
//! scene begins ([`GameArena::begin`]) and freed when it ends
//! ([`GameArena::end`]), so a scene's state costs memory only while the
//! scene runs. Values come out as [`SceneBox`]es, which unlike the
//! original's plain pointers run their destructors; the block is freed only
//! once every box has been dropped, which [`GameArena::end`] checks.
//!
//! The original's object free lists (`gcGetAObjSetNextAlloc`,
//! `gcGetDObjSetNextAlloc`, `gcGetMObjSetNextAlloc`, `objman.c:602-706`)
//! take a new object from the heap whenever their list is empty, into the
//! slack the heap holds past the scene's setup. The port's block is sized
//! to the scene's setup alone, so [`GameArena::alloc_grow_uninit`] takes
//! such an object from a block of its own once the scene's block is full;
//! those blocks are the scene's too, freed by [`GameArena::end`]. Each
//! grown block starts with the link to the one grown before it, so the
//! arena holds only the last.

use core::cell::Cell;
use core::marker::PhantomData;
use core::mem::{align_of, size_of, MaybeUninit};
use core::ops::{Deref, DerefMut};
use core::ptr::NonNull;

use super::{BlockAlloc, ARENA_ALIGN};

/// The most objects [`GameArena::alloc_grow_uninit`] holds past a scene's
/// block: the effect and weapon players' pool (`psp-game`), 38 effects and
/// 32 weapons (`EFFECT_ALLOC_NUM`, `WEAPON_ALLOC_NUM`).
pub const GROW_BLOCKS: usize = 70;

/// What a [`GameArena`] call could not do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameArenaError {
    /// The scene asked for more than its arena holds (`ml : alloc
    /// overflow`): `need` more bytes with `used` of `capacity` taken.
    Full {
        scene: &'static str,
        need: usize,
        used: usize,
        capacity: usize,
    },
    /// The block allocator had no `bytes` block for `scene`.
    NoMemory { scene: &'static str, bytes: usize },
    /// An allocation with no scene begun.
    NoScene,
    /// [`GameArena::begin`] while `scene` is still running.
    Open { scene: &'static str },
    /// [`GameArena::end`] while `live` of `scene`'s boxes are alive.
    Live { scene: &'static str, live: u32 },
}

/// One scene's figures, from [`GameArena::end`] or [`GameArena::usage`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct GameArenaUsage {
    pub scene: &'static str,
    /// Bytes taken, alignment padding included.
    pub used: usize,
    pub capacity: usize,
    /// Bytes of the objects in blocks taken past the scene's block
    /// ([`GameArena::alloc_grow_uninit`]), their links not included.
    pub grown: usize,
}

/// The block's first bytes: the count of its live [`SceneBox`]es.
struct Header {
    live: Cell<u32>,
}

/// A scene's general heap over one block per scene. See the module docs.
pub struct GameArena<A: BlockAlloc> {
    alloc: A,
    block: Option<A::Block>,
    scene: &'static str,
    capacity: usize,
    used: usize,
    /// The last block taken past `block`, one object each, freed with it:
    /// each starts with the link to the one before ([`Self::grow_link`]).
    grown: Option<A::Block>,
    grown_len: usize,
    grown_bytes: usize,
}

impl<A: BlockAlloc> GameArena<A> {
    pub const fn new(alloc: A) -> Self {
        GameArena {
            alloc,
            block: None,
            scene: "",
            capacity: 0,
            used: 0,
            grown: None,
            grown_len: 0,
            grown_bytes: 0,
        }
    }

    /// `syTaskmanInitGeneralHeap`: takes a `bytes`-byte block for `scene`.
    /// The block must be [`ARENA_ALIGN`]-aligned.
    pub fn begin(&mut self, scene: &'static str, bytes: usize) -> Result<(), GameArenaError> {
        if self.block.is_some() {
            return Err(GameArenaError::Open { scene: self.scene });
        }
        let bytes = bytes.max(size_of::<Header>());
        let block = self
            .alloc
            .alloc(bytes)
            .ok_or(GameArenaError::NoMemory { scene, bytes })?;
        let base = A::ptr(&block);
        assert!(
            (base as usize).is_multiple_of(ARENA_ALIGN),
            "game arena block below ARENA_ALIGN"
        );
        // SAFETY: the block is `bytes >= size_of::<Header>()` long and
        // aligned for `Header` (`ARENA_ALIGN` >= 4).
        unsafe { base.cast::<Header>().write(Header { live: Cell::new(0) }) };
        self.block = Some(block);
        self.scene = scene;
        self.capacity = bytes;
        self.used = size_of::<Header>();
        Ok(())
    }

    /// Frees the scene's block. Refused while any of its boxes lives: the
    /// block stays, so no box ever points at freed memory.
    pub fn end(&mut self) -> Result<GameArenaUsage, GameArenaError> {
        let Some(block) = self.block.as_ref() else {
            return Ok(GameArenaUsage::default());
        };
        let live = Self::header(block).live.get();
        if live != 0 {
            return Err(GameArenaError::Live {
                scene: self.scene,
                live,
            });
        }
        let usage = self.usage();
        let mut next = self.grown.take();
        while let Some(b) = next {
            // SAFETY: `alloc_grow_uninit` wrote the link, and nothing has
            // read it out since.
            next = unsafe { Self::grow_link(&b).read() };
        }
        self.grown_len = 0;
        self.grown_bytes = 0;
        self.block = None;
        self.scene = "";
        self.capacity = 0;
        self.used = 0;
        Ok(usage)
    }

    /// Whether a scene has begun and not ended.
    pub fn is_open(&self) -> bool {
        self.block.is_some()
    }

    pub fn usage(&self) -> GameArenaUsage {
        GameArenaUsage {
            scene: self.scene,
            used: self.used,
            capacity: self.capacity,
            grown: self.grown_bytes,
        }
    }

    /// The scene's boxes still alive.
    pub fn live(&self) -> u32 {
        self.block
            .as_ref()
            .map_or(0, |b| Self::header(b).live.get())
    }

    fn header(block: &A::Block) -> &Header {
        // SAFETY: `begin` wrote the header at the block's start, and the
        // block lives as long as the borrow.
        unsafe { &*A::ptr(block).cast::<Header>() }
    }

    /// `syMallocSet`: room for one `T`, uninitialised.
    pub fn alloc_uninit<T>(&mut self) -> Result<SceneBox<MaybeUninit<T>>, GameArenaError> {
        let (ptr, header) = self.bump(size_of::<T>(), align_of::<T>())?;
        Ok(SceneBox::from_raw(ptr.cast::<MaybeUninit<T>>(), header))
    }

    /// `gcGet*SetNextAlloc` on an empty free list: room for one `T` from
    /// the scene's block while it has room, else from a block of its own
    /// (module docs), at most [`GROW_BLOCKS`] of them a scene. A grown
    /// block is never reused within its scene; the caller keeps its own
    /// free list, as the original's does.
    pub fn alloc_grow_uninit<T>(&mut self) -> Result<SceneBox<MaybeUninit<T>>, GameArenaError> {
        let full = match self.alloc_uninit::<T>() {
            Err(e @ GameArenaError::Full { .. }) => e,
            other => return other,
        };
        let Some(block) = self.block.as_ref() else {
            return Err(GameArenaError::NoScene);
        };
        if self.grown_len == GROW_BLOCKS {
            return Err(full);
        }
        let header = NonNull::new(A::ptr(block).cast::<Header>()).expect("block pointer");
        let at = size_of::<Option<A::Block>>().next_multiple_of(align_of::<T>());
        let bytes = at + size_of::<T>();
        let grown = self.alloc.alloc(bytes).ok_or(GameArenaError::NoMemory {
            scene: self.scene,
            bytes,
        })?;
        let base = A::ptr(&grown);
        assert!(
            (base as usize).is_multiple_of(align_of::<T>().max(align_of::<Option<A::Block>>())),
            "grown block below the value's alignment"
        );
        // SAFETY: the block is `bytes` long and aligned for the link and,
        // at `at`, for `T`.
        let ptr = unsafe {
            Self::grow_link(&grown).write(self.grown.take());
            base.add(at)
        };
        self.grown = Some(grown);
        self.grown_len += 1;
        self.grown_bytes += size_of::<T>();
        // SAFETY: the header lives while the scene's block does, which
        // `end` keeps until every box is gone.
        let h = unsafe { header.as_ref() };
        h.live.set(h.live.get() + 1);
        Ok(SceneBox::from_raw(ptr.cast::<MaybeUninit<T>>(), header))
    }

    /// A grown block's link to the block grown before it, at its start.
    fn grow_link(block: &A::Block) -> *mut Option<A::Block> {
        A::ptr(block).cast::<Option<A::Block>>()
    }

    /// Moves `value` into the arena.
    pub fn alloc<T>(&mut self, value: T) -> Result<SceneBox<T>, GameArenaError> {
        let mut b = self.alloc_uninit::<T>()?;
        b.write(value);
        // SAFETY: just written.
        Ok(unsafe { b.assume_init() })
    }

    /// `[f(0), .., f(len - 1)]`, contiguous.
    pub fn alloc_slice_with<T>(
        &mut self,
        len: usize,
        mut f: impl FnMut(usize) -> T,
    ) -> Result<SceneBox<[T]>, GameArenaError> {
        let bytes = size_of::<T>().saturating_mul(len);
        let (ptr, header) = self.bump(bytes, align_of::<T>())?;
        let first = ptr.cast::<T>();
        for i in 0..len {
            // SAFETY: in bounds of the `len`-element run just bumped.
            unsafe { first.add(i).write(f(i)) };
        }
        let slice = core::ptr::slice_from_raw_parts_mut(first, len);
        Ok(SceneBox::from_raw(slice, header))
    }

    /// The iterator's items, contiguous. The run is bumped once, for the
    /// iterator's exact length; a short iterator leaves the rest unused.
    pub fn alloc_from_iter<T, I>(&mut self, items: I) -> Result<SceneBox<[T]>, GameArenaError>
    where
        I: IntoIterator<Item = T>,
        I::IntoIter: ExactSizeIterator,
    {
        let it = items.into_iter();
        let len = it.len();
        let (ptr, header) = self.bump(size_of::<T>().saturating_mul(len), align_of::<T>())?;
        let first = ptr.cast::<T>();
        let mut n = 0;
        for v in it.take(len) {
            // SAFETY: `n < len`, inside the run just bumped.
            unsafe { first.add(n).write(v) };
            n += 1;
        }
        Ok(SceneBox::from_raw(
            core::ptr::slice_from_raw_parts_mut(first, n),
            header,
        ))
    }

    /// Bumps `len` bytes aligned to `align`; returns them and the block's
    /// header, with the header's count already raised for the new box.
    fn bump(
        &mut self,
        len: usize,
        align: usize,
    ) -> Result<(*mut u8, NonNull<Header>), GameArenaError> {
        let Some(block) = self.block.as_ref() else {
            return Err(GameArenaError::NoScene);
        };
        let base = A::ptr(block);
        let pad = (base as usize + self.used).wrapping_neg() & (align.max(1) - 1);
        let full = GameArenaError::Full {
            scene: self.scene,
            need: len.saturating_add(pad),
            used: self.used,
            capacity: self.capacity,
        };
        let end = self
            .used
            .checked_add(pad)
            .and_then(|o| o.checked_add(len))
            .ok_or(full)?;
        if end > self.capacity {
            return Err(full);
        }
        // SAFETY: `used + pad <= capacity`, inside the block.
        let ptr = unsafe { base.add(self.used + pad) };
        self.used = end;
        let header = NonNull::new(base.cast::<Header>()).expect("block pointer");
        // SAFETY: the header lives while the block does.
        let h = unsafe { header.as_ref() };
        h.live.set(h.live.get() + 1);
        Ok((ptr, header))
    }
}

impl<A: BlockAlloc> Drop for GameArena<A> {
    /// A block with live boxes is leaked rather than freed under them.
    fn drop(&mut self) {
        if self.live() != 0 {
            core::mem::forget(self.block.take());
            core::mem::forget(self.grown.take());
        }
    }
}

/// A value in a [`GameArena`] scene: owns it like a `Box`, and runs its
/// destructor when dropped. The arena's block outlives every box, since
/// [`GameArena::end`] refuses to free it while any box lives.
pub struct SceneBox<T: ?Sized> {
    ptr: NonNull<T>,
    header: NonNull<Header>,
    _owns: PhantomData<T>,
}

impl<T: ?Sized> SceneBox<T> {
    fn from_raw(ptr: *mut T, header: NonNull<Header>) -> Self {
        SceneBox {
            ptr: NonNull::new(ptr).expect("arena pointer"),
            header,
            _owns: PhantomData,
        }
    }
}

impl<T> SceneBox<MaybeUninit<T>> {
    /// # Safety
    ///
    /// The value is fully initialised.
    pub unsafe fn assume_init(self) -> SceneBox<T> {
        let me = core::mem::ManuallyDrop::new(self);
        SceneBox {
            ptr: me.ptr.cast::<T>(),
            header: me.header,
            _owns: PhantomData,
        }
    }

    /// The uninitialised value's address, for building it in place a field
    /// at a time (a value too large to build on the stack first).
    pub fn as_mut_ptr(&mut self) -> *mut T {
        self.ptr.as_ptr().cast::<T>()
    }
}

impl<T: ?Sized> Deref for SceneBox<T> {
    type Target = T;
    fn deref(&self) -> &T {
        // SAFETY: the box owns an initialised value in a block that
        // outlives it.
        unsafe { self.ptr.as_ref() }
    }
}

impl<T: ?Sized> DerefMut for SceneBox<T> {
    fn deref_mut(&mut self) -> &mut T {
        // SAFETY: as in `deref`; `&mut self` makes the access unique.
        unsafe { self.ptr.as_mut() }
    }
}

impl<T: ?Sized> Drop for SceneBox<T> {
    fn drop(&mut self) {
        // SAFETY: the box owns the value; the header outlives the box.
        unsafe {
            core::ptr::drop_in_place(self.ptr.as_ptr());
            let h = self.header.as_ref();
            h.live.set(h.live.get() - 1);
        }
    }
}

impl<T: ?Sized + core::fmt::Debug> core::fmt::Debug for SceneBox<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        (**self).fmt(f)
    }
}

/// The bytes a scene's arena needs for values laid out in turn: the
/// header, then each `(size, align)` at its alignment (at most
/// `align - 1` padding each). Sizes a scene's block before it begins.
pub const fn scene_bytes(items: &[(usize, usize)]) -> usize {
    let mut total = size_of::<Header>();
    let mut i = 0;
    while i < items.len() {
        let (size, align) = items[i];
        total += size + align - 1;
        i += 1;
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::rc::Rc;
    use alloc::vec::Vec;

    /// Heap blocks of `u128`s (16-byte aligned), counting frees.
    struct Blocks {
        budget: usize,
        freed: Rc<Cell<usize>>,
    }
    struct Block(Vec<u128>, Rc<Cell<usize>>);
    impl Drop for Block {
        fn drop(&mut self) {
            self.1.set(self.1.get() + 1);
        }
    }
    impl BlockAlloc for Blocks {
        type Block = Block;
        fn alloc(&mut self, len: usize) -> Option<Block> {
            self.budget = self.budget.checked_sub(len)?;
            Some(Block(alloc::vec![0; len.div_ceil(16)], self.freed.clone()))
        }
        fn ptr(block: &Block) -> *mut u8 {
            block.0.as_ptr() as *mut u8
        }
    }

    fn arena(budget: usize) -> (GameArena<Blocks>, Rc<Cell<usize>>) {
        let freed = Rc::new(Cell::new(0));
        let a = GameArena::new(Blocks {
            budget,
            freed: freed.clone(),
        });
        (a, freed)
    }

    /// Counts its drops.
    struct Noisy(Rc<Cell<u32>>, #[allow(dead_code)] Vec<u8>);
    impl Drop for Noisy {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }

    #[test]
    fn a_scene_frees_its_block_only_once_its_boxes_are_dropped() {
        let (mut a, freed) = arena(1 << 16);
        let drops = Rc::new(Cell::new(0));
        a.begin("battle", 1024).unwrap();
        let x = a.alloc(Noisy(drops.clone(), alloc::vec![1, 2, 3])).unwrap();
        let ys = a
            .alloc_slice_with(3, |_| Noisy(drops.clone(), Vec::new()))
            .unwrap();
        assert_eq!(a.live(), 2);
        assert_eq!(
            a.end(),
            Err(GameArenaError::Live {
                scene: "battle",
                live: 2
            })
        );
        assert!(a.is_open());
        drop(ys);
        assert_eq!(drops.get(), 3);
        drop(x);
        assert_eq!(drops.get(), 4);
        let usage = a.end().unwrap();
        assert_eq!((usage.scene, usage.capacity), ("battle", 1024));
        assert_eq!(freed.get(), 1);
        assert!(!a.is_open());
    }

    #[test]
    fn allocations_bump_in_order_and_overflow_names_the_scene() {
        let (mut a, _) = arena(1 << 16);
        assert_eq!(a.alloc(1u8).err(), Some(GameArenaError::NoScene));
        let need = scene_bytes(&[(1, 1), (8, 8), (100, 4)]);
        a.begin("vs", need).unwrap();
        assert_eq!(a.begin("vs", 64), Err(GameArenaError::Open { scene: "vs" }));
        let p = a.alloc(7u8).unwrap();
        let q = a.alloc(9u64).unwrap();
        let r = a.alloc_from_iter((0..25u32).map(|i| i * 2)).unwrap();
        assert_eq!((*p, *q), (7, 9));
        assert_eq!(r.len(), 25);
        assert_eq!(r[24], 48);
        assert_eq!(&*q as *const u64 as usize % 8, 0);
        assert!(a.usage().used <= need);
        let used = a.usage().used;
        let err = a.alloc([0u8; 64]).err().unwrap();
        assert_eq!(
            err,
            GameArenaError::Full {
                scene: "vs",
                need: 64,
                used,
                capacity: need
            }
        );
        drop((p, q, r));
        a.end().unwrap();
    }

    #[test]
    fn a_value_too_big_for_the_stack_is_built_in_place() {
        let (mut a, _) = arena(1 << 20);
        a.begin("big", scene_bytes(&[(size_of::<[u32; 4096]>(), 4)]))
            .unwrap();
        let mut b = a.alloc_uninit::<[u32; 4096]>().unwrap();
        let p = b.as_mut_ptr().cast::<u32>();
        for i in 0..4096 {
            // SAFETY: in bounds of the uninitialised array.
            unsafe { p.add(i).write(i as u32) };
        }
        // SAFETY: every element was written.
        let b = unsafe { b.assume_init() };
        assert_eq!(b[4095], 4095);
        drop(b);
        a.end().unwrap();
    }

    #[test]
    fn objects_past_the_block_grow_blocks_of_their_own_freed_with_the_scene() {
        let (mut a, freed) = arena(1 << 20);
        a.begin("vs", scene_bytes(&[(size_of::<u64>(), 8)]))
            .unwrap();
        // The scene's block takes the first while it has room.
        let first = a.alloc_grow_uninit::<u64>().unwrap();
        assert_eq!(a.usage().grown, 0);
        let mut more = Vec::new();
        for i in 0..GROW_BLOCKS {
            let mut b = a.alloc_grow_uninit::<[u64; 4]>().unwrap();
            b.write([i as u64; 4]);
            // SAFETY: just written.
            more.push(unsafe { b.assume_init() });
        }
        assert_eq!(a.usage().grown, GROW_BLOCKS * 32);
        assert_eq!(more[GROW_BLOCKS - 1][3], GROW_BLOCKS as u64 - 1);
        let used = a.usage().used;
        assert_eq!(
            a.alloc_grow_uninit::<[u64; 4]>().err(),
            Some(GameArenaError::Full {
                scene: "vs",
                need: 32,
                used,
                capacity: a.usage().capacity
            })
        );
        assert_eq!(a.live(), 1 + GROW_BLOCKS as u32);
        assert!(matches!(a.end(), Err(GameArenaError::Live { .. })));
        assert_eq!(freed.get(), 0);
        drop((first, more));
        let usage = a.end().unwrap();
        assert_eq!(usage.grown, GROW_BLOCKS * 32);
        assert_eq!(freed.get(), 1 + GROW_BLOCKS);
        // The next scene starts with none grown.
        a.begin("vs", 64).unwrap();
        assert_eq!(a.usage().grown, 0);
        a.end().unwrap();
    }

    #[test]
    fn a_short_block_allocator_fails_the_scene() {
        let (mut a, _) = arena(100);
        assert_eq!(
            a.begin("tiny", 200),
            Err(GameArenaError::NoMemory {
                scene: "tiny",
                bytes: 200
            })
        );
        assert!(!a.is_open());
    }
}
