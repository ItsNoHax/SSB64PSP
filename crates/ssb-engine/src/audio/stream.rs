//! Sample streaming: the banks' wave data (the tbl files) read from the
//! pack through a page cache instead of held resident (PSP-1000 memory,
//! RE-572 option A; Memory Stick latency RE-573).
//!
//! The N64 keeps the tbl files in ROM and the synthesizer pulls every ADPCM
//! chunk through `syAudioDma` (`src/sys/audio.c:441-484`), called only from
//! `_decodeChunk` (n_env.c:744-745): per physical voice
//! (`alN_PVoiceNew`, n_env.c:442-446; 16 voices, audio.c:96), 4 rotating
//! 1 KiB buffers (`NUM_DMA_BUFFERS`, `MAX_BUFFER_LENGTH`, audio.h:9-11).
//! A chunk inside the voice's current buffer is served from it; any other
//! starts a 1 KiB PI DMA from the chunk's address (`addr & ~1`) into the
//! next buffer. The audio thread never waits for it (the drain is
//! `OS_MESG_NOBLOCK`, audio.c:1082-1085): the DMA ends long before the RSP
//! task that reads it runs, and a late one has no handling (the RSP would
//! decode the buffer's previous bytes).
//!
//! A Memory Stick read costs about 1.6 ms + 0.073 ms/KiB and queues behind
//! any loader read (RE-573), so the PSP cannot fetch per chunk on the audio
//! thread. Instead (PSP deviation):
//!
//! - [`PageCache`]: [`PAGE`]-byte pages of the pack's streamed region (both
//!   tbl parts, page-aligned in the file) in a fixed set of slots with LRU
//!   eviction. The audio thread only copies from mapped slots and queues
//!   page requests; a streamer thread reads them ([`PageCache::service`]).
//! - Heads: each wave's first [`HEAD`] bytes stay resident (the section's
//!   `Heads` part), so a voice starts before any of its pages is in RAM.
//! - Prefetch: every chunk read queues the pages holding its wave's next
//!   [`LOOKAHEAD`] bytes; a looping voice also queues its loop start's.
//! - Miss (a page not in RAM when its chunk is decoded): the missing bytes
//!   decode as zeros, counted in [`CacheStats::misses`]. The N64 has no
//!   miss case to copy; zeros are deterministic where its stale buffer
//!   would not be.
//!
//! Every streamed chunk is copied into a [`FETCH_MAX`]-byte buffer, the
//! N64's `MAX_BUFFER_LENGTH` (a chunk is at most 23 ADPCM frames, 207
//! bytes: `MAX_RATIO` x 184 samples).

use alloc::boxed::Box;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicU16, AtomicU32, Ordering};

use super::data::{Bank, ParseError, WaveRef, WaveTable};

/// The page: one sector- and cluster-aligned Memory Stick read (16 KiB:
/// 2.5 ms median, 4.4 ms max, RE-573).
pub const PAGE: usize = 16 * 1024;

/// Resident bytes at the start of every wave: 2 KiB is 3.3 N64 frames of
/// audio at the highest pitch (`MAX_RATIO`, 2), 6.6 at pitch 1 (sized by
/// `tests::head_sizing`).
pub const HEAD: usize = 2048;

/// How far ahead of a chunk its wave's pages are queued: half a page
/// (0.47 s of audio at pitch 2).
pub const LOOKAHEAD: usize = PAGE / 2;

/// `MAX_BUFFER_LENGTH`: the largest chunk one read returns.
pub const FETCH_MAX: usize = 1024;

/// Pending page requests (a power of two).
const QUEUE: usize = 64;

const NONE: u16 = u16::MAX;

/// Reads the streamed region for the streamer thread.
pub trait PageSource {
    /// Fills `buf` from region offset `at`; `false` on an I/O error.
    fn read(&mut self, at: u32, buf: &mut [u8]) -> bool;
}

/// What [`PageCache`] counted. Totals since it was built.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CacheStats {
    /// Chunk reads that found a page missing (audio thread).
    pub misses: u32,
    /// Bytes those reads decoded as zeros.
    pub miss_bytes: u32,
    /// Pages read (streamer).
    pub fetched: u32,
    /// Pages dropped to make room (streamer).
    pub evicted: u32,
    /// Page reads that failed (streamer).
    pub errors: u32,
    /// Requests dropped because the queue was full (audio thread; asked
    /// again on the next read).
    pub queue_full: u32,
    /// Most requests waiting at once.
    pub queue_max: u32,
}

/// The page cache over the streamed region.
///
/// Threads: one reader (the audio thread: [`Self::copy`], [`Self::want`],
/// [`Self::frame`]) and one streamer ([`Self::service`]). Each atomic has
/// one writer, except the per-page `queued` flag (set by the reader, cleared
/// by the streamer after the page is mapped). The streamer unmaps a slot
/// before it reads into it and maps it after, so the reader only copies
/// from a slot that holds the page it looked up.
pub struct PageCache {
    region_len: usize,
    mem: *mut u8,
    slots: usize,
    page_slot: Box<[AtomicU16]>,
    queued: Box<[AtomicBool]>,
    slot_page: Box<[AtomicU16]>,
    slot_used: Box<[AtomicU32]>,
    ring: [AtomicU16; QUEUE],
    head: AtomicU32,
    tail: AtomicU32,
    tick: AtomicU32,
    wake: fn(),
    misses: AtomicU32,
    miss_bytes: AtomicU32,
    fetched: AtomicU32,
    evicted: AtomicU32,
    errors: AtomicU32,
    queue_full: AtomicU32,
    queue_max: AtomicU32,
}

// SAFETY: `mem` is written only by the streamer, into an unmapped slot, and
// read only by the reader, from a mapped one (see the type's docs).
unsafe impl Sync for PageCache {}
unsafe impl Send for PageCache {}

/// Adds `n` to a counter only the calling thread writes.
#[inline]
fn bump(c: &AtomicU32, n: u32) {
    c.store(c.load(Ordering::Relaxed).wrapping_add(n), Ordering::Release);
}

impl PageCache {
    /// A cache of `mem.len() / PAGE` slots over a region of `region_len`
    /// bytes. `wake` is called (on the reader's thread) after each queued
    /// request, to wake the streamer.
    pub fn new(region_len: usize, mem: &'static mut [u8], wake: fn()) -> Self {
        let slots = (mem.len() / PAGE).min(NONE as usize - 1);
        let pages = region_len.div_ceil(PAGE).min(NONE as usize - 1);
        assert!(slots > 0, "page cache without slots");
        Self {
            region_len,
            mem: mem.as_mut_ptr(),
            slots,
            page_slot: (0..pages).map(|_| AtomicU16::new(NONE)).collect(),
            queued: (0..pages).map(|_| AtomicBool::new(false)).collect(),
            slot_page: (0..slots).map(|_| AtomicU16::new(NONE)).collect(),
            slot_used: (0..slots).map(|_| AtomicU32::new(0)).collect(),
            ring: [const { AtomicU16::new(NONE) }; QUEUE],
            head: AtomicU32::new(0),
            tail: AtomicU32::new(0),
            tick: AtomicU32::new(0),
            wake,
            misses: AtomicU32::new(0),
            miss_bytes: AtomicU32::new(0),
            fetched: AtomicU32::new(0),
            evicted: AtomicU32::new(0),
            errors: AtomicU32::new(0),
            queue_full: AtomicU32::new(0),
            queue_max: AtomicU32::new(0),
        }
    }

    /// Pages in the region.
    pub fn pages(&self) -> usize {
        self.page_slot.len()
    }

    /// Slots (pages the cache holds at once).
    pub fn slots(&self) -> usize {
        self.slots
    }

    /// The slot holding `page`, if it is mapped.
    #[inline]
    fn mapped(&self, page: usize) -> Option<usize> {
        let slot = self.page_slot.get(page)?.load(Ordering::Acquire);
        (slot != NONE && self.slot_page[slot as usize].load(Ordering::Acquire) as usize == page)
            .then_some(slot as usize)
    }

    /// Reader: starts a new frame (the LRU clock).
    pub fn frame(&self) {
        let t = self.tick.load(Ordering::Relaxed);
        self.tick.store(t.wrapping_add(1), Ordering::Release);
    }

    /// Reader: copies region bytes `[at, at + out.len())` into `out`. Bytes
    /// of a page not in RAM are zeros and the page is queued. Returns the
    /// count of such bytes.
    pub fn copy(&self, at: usize, out: &mut [u8]) -> usize {
        let tick = self.tick.load(Ordering::Relaxed);
        let mut missing = 0;
        let mut pos = at;
        let mut o = 0;
        while o < out.len() {
            let page = pos / PAGE;
            let off = pos % PAGE;
            let k = (PAGE - off).min(out.len() - o);
            if let Some(slot) = self.mapped(page) {
                self.slot_used[slot].store(tick, Ordering::Relaxed);
                // SAFETY: a mapped slot is not written until the streamer
                // unmaps it, which cannot happen while this thread (the
                // more urgent one) copies (see the type's docs).
                let src =
                    unsafe { core::slice::from_raw_parts(self.mem.add(slot * PAGE + off), k) };
                out[o..o + k].copy_from_slice(src);
            } else {
                out[o..o + k].fill(0);
                if page < self.pages() {
                    missing += k;
                    self.request(page);
                }
            }
            o += k;
            pos += k;
        }
        if missing > 0 {
            bump(&self.misses, 1);
            bump(&self.miss_bytes, missing as u32);
        }
        missing
    }

    /// Reader: queues every page of region bytes `[at, at + len)` not in
    /// RAM, and marks the others used.
    pub fn want(&self, at: usize, len: usize) {
        if len == 0 || at >= self.region_len {
            return;
        }
        let tick = self.tick.load(Ordering::Relaxed);
        let last = (at + len - 1).min(self.region_len - 1) / PAGE;
        for page in at / PAGE..=last {
            match self.mapped(page) {
                Some(slot) => self.slot_used[slot].store(tick, Ordering::Relaxed),
                None => self.request(page),
            }
        }
    }

    fn request(&self, page: usize) {
        if self.queued[page].load(Ordering::Acquire) {
            return;
        }
        let h = self.head.load(Ordering::Relaxed);
        let t = self.tail.load(Ordering::Acquire);
        let waiting = h.wrapping_sub(t);
        if waiting as usize >= QUEUE {
            bump(&self.queue_full, 1);
            return;
        }
        self.ring[h as usize % QUEUE].store(page as u16, Ordering::Relaxed);
        self.queued[page].store(true, Ordering::Release);
        self.head.store(h.wrapping_add(1), Ordering::Release);
        if waiting + 1 > self.queue_max.load(Ordering::Relaxed) {
            self.queue_max.store(waiting + 1, Ordering::Release);
        }
        (self.wake)();
    }

    /// Streamer: whether requests are waiting.
    pub fn pending(&self) -> bool {
        self.head.load(Ordering::Acquire) != self.tail.load(Ordering::Relaxed)
    }

    /// Streamer: reads up to `max` queued pages from `src` into the least
    /// recently used slots. Returns the pages read.
    pub fn service(&self, src: &mut dyn PageSource, max: usize) -> usize {
        let mut n = 0;
        while n < max {
            let t = self.tail.load(Ordering::Relaxed);
            if t == self.head.load(Ordering::Acquire) {
                break;
            }
            let page = self.ring[t as usize % QUEUE].load(Ordering::Relaxed) as usize;
            if self.mapped(page).is_none() {
                self.fetch(src, page);
                n += 1;
            }
            self.queued[page].store(false, Ordering::Release);
            self.tail.store(t.wrapping_add(1), Ordering::Release);
        }
        n
    }

    fn fetch(&self, src: &mut dyn PageSource, page: usize) {
        let slot = self.victim();
        let old = self.slot_page[slot].load(Ordering::Relaxed);
        self.slot_page[slot].store(NONE, Ordering::Release);
        if old != NONE {
            self.page_slot[old as usize].store(NONE, Ordering::Release);
            bump(&self.evicted, 1);
        }
        let len = PAGE.min(self.region_len - page * PAGE);
        // SAFETY: the slot is unmapped: the reader does not touch it.
        let buf = unsafe { core::slice::from_raw_parts_mut(self.mem.add(slot * PAGE), len) };
        if src.read((page * PAGE) as u32, buf) {
            self.slot_used[slot].store(self.tick.load(Ordering::Acquire), Ordering::Relaxed);
            self.slot_page[slot].store(page as u16, Ordering::Release);
            self.page_slot[page].store(slot as u16, Ordering::Release);
            bump(&self.fetched, 1);
        } else {
            bump(&self.errors, 1);
        }
    }

    /// A free slot, or the least recently used one.
    fn victim(&self) -> usize {
        let tick = self.tick.load(Ordering::Acquire);
        let mut best = 0;
        let mut best_age = 0;
        for s in 0..self.slots {
            if self.slot_page[s].load(Ordering::Relaxed) == NONE {
                return s;
            }
            let age = tick.wrapping_sub(self.slot_used[s].load(Ordering::Relaxed));
            if age > best_age {
                best = s;
                best_age = age;
            }
        }
        best
    }

    /// Pages in RAM now.
    pub fn resident_pages(&self) -> usize {
        (0..self.slots)
            .filter(|&s| self.slot_page[s].load(Ordering::Relaxed) != NONE)
            .count()
    }

    pub fn stats(&self) -> CacheStats {
        let l = |c: &AtomicU32| c.load(Ordering::Acquire);
        CacheStats {
            misses: l(&self.misses),
            miss_bytes: l(&self.miss_bytes),
            fetched: l(&self.fetched),
            evicted: l(&self.evicted),
            errors: l(&self.errors),
            queue_full: l(&self.queue_full),
            queue_max: l(&self.queue_max),
        }
    }
}

/// A wave's resident head length.
fn head_len(wt: &WaveTable, head: usize) -> usize {
    (wt.len.max(0) as usize).min(head)
}

/// The section's `Heads` part: bank 0's waves in index order, then bank
/// 1's, each its first `min(len, HEAD)` bytes. [`Streamed::new`] walks the
/// waves in the same order.
pub fn build_heads(banks: [&Bank; 2], tbls: [&[u8]; 2]) -> Vec<u8> {
    build_heads_with(banks, tbls, HEAD)
}

/// [`build_heads`] with another head length (for sizing).
pub fn build_heads_with(banks: [&Bank; 2], tbls: [&[u8]; 2], head: usize) -> Vec<u8> {
    let mut out = Vec::new();
    for (bank, tbl) in banks.into_iter().zip(tbls) {
        for wt in &bank.wavetables {
            let base = wt.base as usize;
            out.extend_from_slice(&tbl[base..base + head_len(wt, head)]);
        }
    }
    out
}

/// Streamed samples: the resident heads and the page cache.
pub struct Streamed {
    cache: &'static PageCache,
    heads: &'static [u8],
    /// Per bank, per wave: `(offset into heads, len)`.
    index: [Vec<(u32, u32)>; 2],
    /// Each bank's tbl in the region: offset, len.
    tbl_at: [u32; 2],
    tbl_len: [u32; 2],
}

impl Streamed {
    pub fn new(
        heads: &'static [u8],
        banks: [&Bank; 2],
        tbl_at: [u32; 2],
        tbl_len: [u32; 2],
        cache: &'static PageCache,
    ) -> Result<Self, ParseError> {
        Self::with_head(heads, banks, tbl_at, tbl_len, cache, HEAD)
    }

    /// [`Self::new`] for heads built with another length.
    pub fn with_head(
        heads: &'static [u8],
        banks: [&Bank; 2],
        tbl_at: [u32; 2],
        tbl_len: [u32; 2],
        cache: &'static PageCache,
        head: usize,
    ) -> Result<Self, ParseError> {
        let mut at = 0u32;
        let index = banks.map(|b| {
            b.wavetables
                .iter()
                .map(|wt| {
                    let e = (at, head_len(wt, head) as u32);
                    at += e.1;
                    e
                })
                .collect::<Vec<_>>()
        });
        if at as usize != heads.len() {
            return Err(ParseError("heads part does not match the banks"));
        }
        Ok(Self {
            cache,
            heads,
            index,
            tbl_at,
            tbl_len,
        })
    }

    pub fn cache(&self) -> &'static PageCache {
        self.cache
    }

    /// Resident head bytes.
    pub fn heads_len(&self) -> usize {
        self.heads.len()
    }

    /// Bytes `[start, end)` of the wave's bank tbl into `scratch`, and the
    /// wave's next [`LOOKAHEAD`] bytes queued.
    fn read<'a>(
        &self,
        w: WaveRef,
        wt: &WaveTable,
        start: usize,
        end: usize,
        scratch: &'a mut [u8],
    ) -> &'a [u8] {
        let b = (w.bank != 0) as usize;
        debug_assert!(end - start <= scratch.len(), "chunk over FETCH_MAX");
        let end = end.min(start + scratch.len());
        let out = &mut scratch[..end - start];
        let (ho, hl) = self.index[b][w.index as usize];
        let base = wt.base as usize;
        let head_end = base + hl as usize;
        let mut pos = start;
        if pos >= base && pos < head_end {
            let k = head_end.min(end) - pos;
            let h = ho as usize + pos - base;
            out[..k].copy_from_slice(&self.heads[h..h + k]);
            pos += k;
        }
        if pos < end {
            self.cache
                .copy(self.tbl_at[b] as usize + pos, &mut out[pos - start..]);
        }
        self.want(b, wt, head_end, start, end + LOOKAHEAD);
        out
    }

    /// Queues the wave's bytes `[from, to)` past its head.
    fn want(&self, b: usize, wt: &WaveTable, head_end: usize, from: usize, to: usize) {
        let wave_end = (wt.base as usize + wt.len.max(0) as usize).min(self.tbl_len[b] as usize);
        let from = from.max(head_end);
        let to = to.min(wave_end);
        if from < to {
            self.cache.want(self.tbl_at[b] as usize + from, to - from);
        }
    }

    fn prefetch(&self, w: WaveRef, wt: &WaveTable, at: usize) {
        let b = (w.bank != 0) as usize;
        let (_, hl) = self.index[b][w.index as usize];
        let head_end = wt.base as usize + hl as usize;
        self.want(b, wt, head_end, at, at + LOOKAHEAD);
    }
}

/// Both banks' wave data, where the synthesizer reads it.
pub enum Samples {
    /// The tbl parts in RAM: `[music, sfx]`.
    Resident([&'static [u8]; 2]),
    Streamed(Streamed),
}

impl Samples {
    /// A bank's tbl length.
    pub fn tbl_len(&self, bank: u8) -> usize {
        let b = (bank != 0) as usize;
        match self {
            Self::Resident(t) => t[b].len(),
            Self::Streamed(s) => s.tbl_len[b] as usize,
        }
    }

    /// `syAudioDma`'s role for `_decodeChunk`: the bytes `[start, start +
    /// nbytes)` of the wave's bank tbl, clamped to the tbl.
    #[inline]
    pub fn read<'a>(
        &'a self,
        w: WaveRef,
        wt: &WaveTable,
        start: i32,
        nbytes: i32,
        scratch: &'a mut [u8; FETCH_MAX],
    ) -> &'a [u8] {
        let len = self.tbl_len(w.bank);
        let start = (start.max(0) as usize).min(len);
        let end = (start + nbytes.max(0) as usize).min(len);
        match self {
            Self::Resident(t) => &t[(w.bank != 0) as usize][start..end],
            Self::Streamed(s) => s.read(w, wt, start, end, scratch),
        }
    }

    /// Queues the pages for a voice's upcoming bytes at `at` (a loop's
    /// start). A no-op for resident samples.
    #[inline]
    pub fn prefetch(&self, w: WaveRef, wt: &WaveTable, at: i32) {
        if let Self::Streamed(s) = self {
            s.prefetch(w, wt, at.max(0) as usize);
        }
    }

    /// Starts a frame (the cache's LRU clock).
    #[inline]
    pub fn frame(&self) {
        if let Self::Streamed(s) = self {
            s.cache.frame();
        }
    }

    /// The streamed samples, if any.
    pub fn streamed(&self) -> Option<&Streamed> {
        match self {
            Self::Streamed(s) => Some(s),
            Self::Resident(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;
    use crate::audio::data::{AudioData, Part, Section};
    use crate::audio::{golden, testdata, AudioSystem, FRAME_SAMPLES_MAX};
    use alloc::vec;
    use std::println;

    fn no_wake() {}

    fn cache(region_len: usize, slots: usize) -> &'static PageCache {
        let mem = Box::leak(vec![0u8; slots * PAGE].into_boxed_slice());
        Box::leak(Box::new(PageCache::new(region_len, mem, no_wake)))
    }

    struct Mem(&'static [u8]);

    impl PageSource for Mem {
        fn read(&mut self, at: u32, buf: &mut [u8]) -> bool {
            let at = at as usize;
            buf.copy_from_slice(&self.0[at..at + buf.len()]);
            true
        }
    }

    fn region(len: usize) -> &'static [u8] {
        let mut x = 0x2545_F491u32;
        let v: Vec<u8> = (0..len)
            .map(|_| {
                x ^= x << 13;
                x ^= x >> 17;
                x ^= x << 5;
                x as u8
            })
            .collect();
        Box::leak(v.into_boxed_slice())
    }

    #[test]
    fn a_miss_is_zeros_then_the_page_arrives() {
        let r = region(5 * PAGE + 300);
        let c = cache(r.len(), 2);
        let mut out = [0xAAu8; 50];
        assert_eq!(c.copy(100, &mut out), 50);
        assert!(out.iter().all(|&b| b == 0));
        assert_eq!(c.stats().misses, 1);
        assert!(c.pending());
        assert_eq!(c.service(&mut Mem(r), usize::MAX), 1);
        assert_eq!(c.copy(100, &mut out), 0);
        assert_eq!(&out[..], &r[100..150]);
        // A read across a page boundary, the last page short.
        let at = 5 * PAGE - 10;
        let mut out = [0u8; 310];
        assert_eq!(c.copy(at, &mut out[..200]), 200);
        c.service(&mut Mem(r), usize::MAX);
        assert_eq!(c.copy(at, &mut out[..310]), 0);
        assert_eq!(&out[..310], &r[at..at + 310]);
        assert_eq!(c.stats().evicted, 1);
    }

    #[test]
    fn the_least_recently_used_page_goes() {
        let r = region(8 * PAGE);
        let c = cache(r.len(), 2);
        let mut src = Mem(r);
        let mut b = [0u8; 4];
        c.want(0, 1);
        c.service(&mut src, 9);
        c.frame();
        c.want(PAGE, 1);
        c.service(&mut src, 9);
        c.frame();
        assert_eq!(c.copy(0, &mut b), 0); // page 0 used last
        c.frame();
        c.want(2 * PAGE, 1);
        c.service(&mut src, 9);
        assert_eq!(c.copy(0, &mut b), 0);
        assert_eq!(c.copy(2 * PAGE, &mut b), 0);
        assert_eq!(c.copy(PAGE, &mut b), 4);
        assert_eq!(c.resident_pages(), 2);
    }

    #[test]
    fn a_full_queue_drops_and_a_page_is_queued_once() {
        let r = region(80 * PAGE);
        let c = cache(r.len(), 4);
        c.want(0, 80 * PAGE);
        c.want(0, 80 * PAGE);
        let s = c.stats();
        assert_eq!(s.queue_max as usize, QUEUE);
        assert_eq!(s.queue_full as usize, 2 * (80 - QUEUE));
        assert_eq!(c.service(&mut Mem(r), usize::MAX), QUEUE);
        assert!(!c.pending());
    }

    #[test]
    fn random_reads_match_the_region() {
        let r = region(40 * PAGE + 77);
        let c = cache(r.len(), 8);
        let mut src = Mem(r);
        let mut x = 7u32;
        let mut out = [0u8; FETCH_MAX];
        for _ in 0..20_000 {
            x = x.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let at = (x >> 8) as usize % r.len();
            let n = (1 + (x as usize & 0x3FF)).min(r.len() - at);
            let missing = c.copy(at, &mut out[..n]);
            for (k, &b) in out[..n].iter().enumerate() {
                assert!(b == r[at + k] || b == 0);
            }
            if missing > 0 {
                c.service(&mut src, usize::MAX);
                assert_eq!(c.copy(at, &mut out[..n]), 0);
            }
            assert_eq!(&out[..n], &r[at..at + n]);
            c.frame();
        }
    }

    // --- Against the ROM's banks -------------------------------------------

    /// The section's resident prefix and streamed region.
    fn split(section: &'static [u8]) -> (&'static [u8], &'static [u8]) {
        let l = Section::stream_layout(section).expect("streamable section");
        (
            &section[..l.resident_len],
            &section[l.region_offset..l.region_offset + l.region_len],
        )
    }

    /// The section with heads of `head` bytes.
    fn with_heads(section: &'static [u8], head: usize) -> &'static [u8] {
        let s = Section::open(section).unwrap();
        let d = AudioData::new(section).unwrap();
        let tbls = [s.part(Part::MusicTbl), s.part(Part::SfxTbl)];
        let heads = build_heads_with([&d.music, &d.sfx], tbls, head);
        let mut parts = [&[][..]; crate::audio::data::PART_COUNT];
        for (i, p) in parts.iter_mut().enumerate() {
            // SAFETY: `i` < PART_COUNT, the enum's discriminant range.
            let part: Part = unsafe { core::mem::transmute::<usize, Part>(i) };
            *p = s.part(part);
        }
        parts[Part::Heads as usize] = &heads;
        Box::leak(Section::build(&parts).into_boxed_slice())
    }

    /// Every chunk of every wave, and random chunks, read through the
    /// cache equal the resident tbl's bytes once their pages are in.
    #[test]
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
    fn streamed_reads_equal_resident_reads() {
        let Some(section) = testdata::section() else {
            return;
        };
        let (prefix, reg) = split(section);
        let c = cache(reg.len(), 48);
        let mut src = Mem(reg);
        let resident = AudioData::new(section).unwrap();
        let streamed = AudioData::new_streamed(prefix, c).unwrap();
        let mut a = [0u8; FETCH_MAX];
        let mut b = [0u8; FETCH_MAX];
        let mut check = |w: WaveRef, start: i32, n: i32| {
            let (wt, _) = resident.wave(w);
            let want = resident.samples.read(w, wt, start, n, &mut a).to_vec();
            let m0 = c.stats().misses;
            let mut got = streamed.samples.read(w, wt, start, n, &mut b).to_vec();
            if c.stats().misses != m0 {
                c.service(&mut src, usize::MAX);
                let m1 = c.stats().misses;
                got = streamed.samples.read(w, wt, start, n, &mut b).to_vec();
                assert_eq!(c.stats().misses, m1, "page still missing");
            }
            assert_eq!(got, want, "{w:?} at {start} len {n}");
            c.frame();
        };
        let mut chunks = 0;
        for (bank, d) in [(0u8, &resident.music), (1, &resident.sfx)] {
            for (i, wt) in d.wavetables.iter().enumerate() {
                let w = WaveRef {
                    bank,
                    index: i as u16,
                };
                let mut at = wt.base as i32;
                while at < wt.base as i32 + wt.len {
                    check(w, at, 207);
                    at += 207;
                    chunks += 1;
                }
            }
        }
        let mut x = 99u32;
        for _ in 0..20_000 {
            x = x.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let bank = (x >> 31) as u8;
            let waves = if bank == 0 {
                &resident.music
            } else {
                &resident.sfx
            };
            let i = (x >> 8) as usize % waves.wavetables.len();
            let wt = waves.wavetables[i];
            let start = wt.base as i32 - 32 + ((x >> 3) as i32 % (wt.len + 64));
            let n = 1 + (x % FETCH_MAX as u32) as i32;
            check(
                WaveRef {
                    bank,
                    index: i as u16,
                },
                start,
                n,
            );
        }
        let s = c.stats();
        println!("chunks {chunks}, cache {s:?}");
        assert_eq!(s.errors, 0);
    }

    /// The golden scenario over streamed samples, the streamer reading one
    /// page per audio frame (a 16 KiB read behind a 64 KiB loader read,
    /// 15 ms, RE-573), renders the resident hashes with no miss.
    #[test]
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
    fn streamed_golden_matches_resident() {
        let Some(section) = testdata::section() else {
            return;
        };
        let (prefix, reg) = split(section);
        let c = cache(reg.len(), CACHE_SLOTS_FOR_TESTS);
        let mut src = Mem(reg);
        let sys = AudioSystem::new_streamed(prefix, c).unwrap();
        let r = golden::run_with(
            sys,
            || 0,
            || {
                c.service(&mut src, 1);
            },
        );
        let s = c.stats();
        println!("golden over the cache: {s:?}");
        assert_eq!(s.misses, 0);
        assert_eq!(r.hashes, golden::HASHES);
    }

    /// The runtime's slot count (`psp_runtime::audio`, 768 KiB).
    const CACHE_SLOTS_FOR_TESTS: usize = 48;

    /// Misses per head length: every BGM for 1.5 s with two FGMs started
    /// every 6 frames (cycling through all of them), the streamer reading
    /// one page per frame, or one per two frames (twice RE-573's worst).
    #[test]
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
    fn head_sizing() {
        let Some(section) = testdata::section() else {
            return;
        };
        let mut buf = [0i16; FRAME_SAMPLES_MAX * 2];
        for every in [1u32, 2] {
            for head in [0usize, 512, 1024, 1536, HEAD] {
                let sec = with_heads(section, head);
                let (prefix, reg) = split(sec);
                let c = cache(reg.len(), CACHE_SLOTS_FOR_TESTS);
                let mut src = Mem(reg);
                let data = AudioData::new_streamed_with_head(prefix, c, head).unwrap();
                let heads = data.samples.streamed().unwrap().heads_len();
                let mut sys = AudioSystem::with_data(data);
                let count = sys.fgm_count();
                let mut next = 0u16;
                let mut frames = 0u32;
                for bgm in 0..47 {
                    sys.play_bgm(0, bgm);
                    for f in 0..90 {
                        if f % 6 == 0 {
                            for _ in 0..2 {
                                sys.play_fgm(next);
                                next = (next + 1) % count;
                            }
                        }
                        sys.render_frame(&mut buf, FRAME_SAMPLES_MAX);
                        frames += 1;
                        if frames.is_multiple_of(every) {
                            c.service(&mut src, 1);
                        }
                    }
                }
                let s = c.stats();
                println!(
                    "page every {every} frame(s), head {head}: heads {heads} B, misses {} ({} B), fetched {}, queue max {}",
                    s.misses, s.miss_bytes, s.fetched, s.queue_max
                );
                if every == 1 && head == HEAD {
                    assert_eq!(s.misses, 0);
                }
            }
        }
    }
}
