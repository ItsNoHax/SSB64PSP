//! The archive files a runtime holds outside its pack buffer (RE-475,
//! D-046).
//!
//! The pack groups its blob by archive file ([`crate::pack::FileDesc`]).
//! A runtime that keeps only the tables and the shared region in its pack
//! buffer reads each scene's files into memory of its own, as
//! `lbRelocLoadFilesListed` reads them into the scene's heap, and registers
//! where each landed here. [`crate::pack::Pack::blob`] comes here for any
//! offset past its buffer.
//!
//! A host that opens the whole file never reaches this module.
//!
//! # Threading
//!
//! One registry per process, read and written by the thread that draws.
//! The PSP game has one such thread; nothing here is synchronised.

use core::cell::UnsafeCell;

use alloc::vec::Vec;

/// One archive file's bytes in memory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    /// The file's first blob offset and length (`FileDesc::blob_start`,
    /// `FileDesc::len`).
    pub start: u32,
    pub len: u32,
    /// Where `start` is in memory.
    pub ptr: *const u8,
}

/// Called for an offset no registered span holds: loads the file holding
/// it and registers it, returning whether it did.
pub type MissHook = fn(offset: u32, len: u32) -> bool;

struct Registry {
    /// The pack buffer whose offsets the spans complete: a pack opened on
    /// any other buffer (a menu pack, a host's whole file) is not served.
    owner: *const u8,
    /// Sorted by `start`; the ranges never overlap.
    spans: Vec<Span>,
    /// The span the last lookup found.
    last: usize,
    miss: Option<MissHook>,
    misses: u32,
    unresolved: u32,
}

struct Cell(UnsafeCell<Registry>);

// SAFETY: see the module's threading note: one thread uses the registry.
unsafe impl Sync for Cell {}

static REGISTRY: Cell = Cell(UnsafeCell::new(Registry {
    owner: core::ptr::null(),
    spans: Vec::new(),
    last: 0,
    miss: None,
    misses: 0,
    unresolved: 0,
}));

fn registry() -> *mut Registry {
    REGISTRY.0.get()
}

/// Serves offsets past the pack buffer at `owner`, calling `miss` for an
/// unloaded file. Clears every span.
///
/// # Safety
///
/// Single-threaded use (module docs). Every span registered later must
/// stay valid until it is removed or the registry is installed again.
pub unsafe fn install(owner: *const u8, miss: Option<MissHook>) {
    let r = unsafe { &mut *registry() };
    r.owner = owner;
    r.spans.clear();
    r.last = 0;
    r.miss = miss;
    r.misses = 0;
    r.unresolved = 0;
}

/// Registers `span`; replaces a span with the same start.
///
/// # Safety
///
/// As [`install`]: `span.ptr` must stay valid for `span.len` bytes until
/// the span is removed.
pub unsafe fn insert(span: Span) {
    let r = unsafe { &mut *registry() };
    let i = r.spans.partition_point(|s| s.start < span.start);
    if r.spans.get(i).is_some_and(|s| s.start == span.start) {
        r.spans[i] = span;
    } else {
        r.spans.insert(i, span);
    }
    r.last = i;
}

/// Registers `run`, spans in ascending order that no registered span
/// lies between (one read's adjacent files): one move of the registry
/// instead of one per span. Falls back to [`insert`] otherwise.
///
/// # Safety
///
/// As [`insert`], for every span of `run`.
pub unsafe fn insert_run(run: &[Span]) {
    let (Some(first), Some(last)) = (run.first(), run.last()) else {
        return;
    };
    let r = unsafe { &mut *registry() };
    let i = r.spans.partition_point(|s| s.start < first.start);
    let fits = run.windows(2).all(|w| w[0].start < w[1].start)
        && r.spans.get(i).is_none_or(|s| s.start > last.start);
    if !fits {
        for s in run {
            unsafe { insert(*s) };
        }
        return;
    }
    r.spans.splice(i..i, run.iter().copied());
    r.last = i;
}

/// Unregisters every span whose start `drop` names, in one pass.
///
/// # Safety
///
/// As [`remove`], for each span dropped.
pub unsafe fn remove_where(mut drop: impl FnMut(u32) -> bool) {
    let r = unsafe { &mut *registry() };
    r.spans.retain(|s| !drop(s.start));
    r.last = 0;
}

/// Unregisters the span starting at `start`, if one does.
///
/// # Safety
///
/// As [`install`]. No slice resolved from the span may be used after.
pub unsafe fn remove(start: u32) {
    let r = unsafe { &mut *registry() };
    let i = r.spans.partition_point(|s| s.start < start);
    if r.spans.get(i).is_some_and(|s| s.start == start) {
        r.spans.remove(i);
        r.last = 0;
    }
}

/// How many spans are registered.
pub fn span_count() -> usize {
    // SAFETY: module threading note.
    unsafe { (*registry()).spans.len() }
}

/// How many lookups needed the miss hook, and how many found nothing even
/// after it, since [`install`] or [`take_misses`].
pub fn misses() -> (u32, u32) {
    // SAFETY: module threading note.
    let r = unsafe { &*registry() };
    (r.misses, r.unresolved)
}

/// [`misses`], then zeroes both counts.
pub fn take_misses() -> (u32, u32) {
    // SAFETY: module threading note.
    let r = unsafe { &mut *registry() };
    let out = (r.misses, r.unresolved);
    r.misses = 0;
    r.unresolved = 0;
    out
}

fn find(offset: u32, len: usize) -> Option<&'static [u8]> {
    // SAFETY: module threading note; the reference ends with this call.
    let r = unsafe { &mut *registry() };
    let hit =
        |s: &Span| offset >= s.start && offset as usize + len <= s.start as usize + s.len as usize;
    let i = match r.spans.get(r.last) {
        Some(s) if hit(s) => r.last,
        _ => {
            let i = r
                .spans
                .partition_point(|s| s.start <= offset)
                .checked_sub(1)?;
            if !hit(&r.spans[i]) {
                return None;
            }
            r.last = i;
            i
        }
    };
    let s = r.spans[i];
    // SAFETY: `insert`'s contract keeps the span's bytes valid while it is
    // registered.
    Some(unsafe { core::slice::from_raw_parts(s.ptr.add((offset - s.start) as usize), len) })
}

/// The `len` bytes at blob `offset` of the pack opened on `owner`, from
/// the registered spans, loading the file through the miss hook if need be.
pub fn resolve(owner: *const u8, offset: u32, len: usize) -> Option<&'static [u8]> {
    // SAFETY: module threading note.
    let (ours, miss) = unsafe {
        let r = &*registry();
        (!owner.is_null() && r.owner == owner, r.miss)
    };
    if !ours {
        return None;
    }
    if let Some(bytes) = find(offset, len) {
        return Some(bytes);
    }
    // SAFETY: module threading note.
    unsafe { (*registry()).misses += 1 };
    if let Some(hook) = miss {
        if hook(offset, len as u32) {
            if let Some(bytes) = find(offset, len) {
                return Some(bytes);
            }
        }
    }
    // SAFETY: module threading note.
    unsafe { (*registry()).unresolved += 1 };
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    static mut LOADED: [u8; 32] = [0; 32];

    fn load(offset: u32, _len: u32) -> bool {
        if !(64..96).contains(&offset) {
            return false;
        }
        unsafe {
            let p = core::ptr::addr_of_mut!(LOADED) as *mut u8;
            for i in 0..32 {
                *p.add(i) = i as u8 + 1;
            }
            insert(Span {
                start: 64,
                len: 32,
                ptr: p,
            });
        }
        true
    }

    #[test]
    fn spans_resolve_and_misses_load() {
        let owner = [0u8; 4];
        let backing = [7u8; 16];
        unsafe {
            install(owner.as_ptr(), Some(load));
            insert(Span {
                start: 16,
                len: 16,
                ptr: backing.as_ptr(),
            });
        }
        assert_eq!(resolve(owner.as_ptr(), 20, 4), Some(&[7u8; 4][..]));
        // Crossing the span's end is not served.
        assert_eq!(resolve(owner.as_ptr(), 30, 4), None);
        // Another buffer's offsets are not ours.
        assert_eq!(resolve(backing.as_ptr(), 20, 4), None);
        // A miss loads the file through the hook.
        assert_eq!(resolve(owner.as_ptr(), 66, 2), Some(&[3u8, 4][..]));
        assert_eq!(take_misses(), (2, 1));
        unsafe {
            remove(16);
            remove(64);
        }
        assert_eq!(span_count(), 0);
        // A run of adjacent spans lands in order in one move; one that
        // would straddle a registered span goes in span by span.
        let bytes = [1u8, 2, 3, 4, 5, 6, 7, 8];
        let at = |start: u32, k: usize| Span {
            start,
            len: 2,
            ptr: bytes[k..].as_ptr(),
        };
        unsafe {
            insert(at(40, 0));
            insert_run(&[at(10, 2), at(12, 4)]);
            insert_run(&[at(30, 6), at(50, 0)]);
        }
        assert_eq!(span_count(), 5);
        assert_eq!(resolve(owner.as_ptr(), 10, 2), Some(&[3u8, 4][..]));
        assert_eq!(resolve(owner.as_ptr(), 12, 2), Some(&[5u8, 6][..]));
        assert_eq!(resolve(owner.as_ptr(), 30, 2), Some(&[7u8, 8][..]));
        assert_eq!(resolve(owner.as_ptr(), 40, 2), Some(&[1u8, 2][..]));
        assert_eq!(resolve(owner.as_ptr(), 50, 2), Some(&[1u8, 2][..]));
        unsafe { remove_where(|start| start != 40) };
        assert_eq!(span_count(), 1);
        assert_eq!(resolve(owner.as_ptr(), 12, 2), None);
        unsafe { remove(40) };
        unsafe { install(core::ptr::null(), None) };
    }
}
