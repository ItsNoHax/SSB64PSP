//! The pack writer's blob held compressed (RE-588): the on-device build
//! cannot hold the raw blob, so each chunk is compressed as it is pushed
//! and decompressed when `PackWriter::finish_to` streams it out. Lossless
//! and private to the writer: the pack's bytes do not change.
//!
//! - Codec: byte-oriented LZ77 (LZ4-like sequences), one independent block
//!   per chunk, so any chunk reads back alone.
//! - Storage: 64 KiB pages (no doubling slack, no large reallocation); a
//!   chunk larger than a quarter page gets a block of its own.
//! - Spill ([`BlobSpill`]): with one set, every page but the open one goes
//!   to it (the PSP: a scratch file on the Memory Stick, so the compressed
//!   blob is not held either); reads come back through a small page cache.
//!   Without one, every page stays in RAM (`romtool pack`).

use alloc::boxed::Box;
use alloc::vec::Vec;

use crate::pack::{align_up, SinkError};

/// Compressed-stream page size.
const PAGE: usize = 64 * 1024;
const MIN_MATCH: usize = 4;
const MAX_OFFSET: usize = 0xFFFF;
const HASH_BITS: u32 = 14;
/// Spilled shared pages kept after a read ([`BlobStore::read_chunk`]).
const CACHE_PAGES: usize = 8;

/// Where a [`BlobStore`] puts its full pages: append-only bytes read back
/// at the offsets [`BlobSpill::append`] returned.
pub trait BlobSpill {
    /// Appends `bytes`, returning their offset in the spill.
    fn append(&mut self, bytes: &[u8]) -> Result<u64, SinkError>;
    /// Reads `out.len()` bytes at offset `at`.
    fn read_at(&mut self, at: u64, out: &mut [u8]) -> Result<(), SinkError>;
}

/// A [`BlobSpill`] in RAM (tests: the spilled store's code path).
#[derive(Default)]
pub struct MemSpill(pub Vec<u8>);

impl BlobSpill for MemSpill {
    fn append(&mut self, bytes: &[u8]) -> Result<u64, SinkError> {
        let at = self.0.len() as u64;
        self.0.extend_from_slice(bytes);
        Ok(at)
    }

    fn read_at(&mut self, at: u64, out: &mut [u8]) -> Result<(), SinkError> {
        let at = at as usize;
        let src = self
            .0
            .get(at..at + out.len())
            .ok_or("blob spill read out of range")?;
        out.copy_from_slice(src);
        Ok(())
    }
}

/// A [`BlobSpill`] over a seekable file (host tests and tools).
#[cfg(feature = "std")]
pub struct FileSpill<F: std::io::Read + std::io::Write + std::io::Seek> {
    file: F,
    len: u64,
}

#[cfg(feature = "std")]
impl<F: std::io::Read + std::io::Write + std::io::Seek> FileSpill<F> {
    /// Spills to `file` from its start (its old bytes are overwritten).
    pub fn new(file: F) -> Self {
        Self { file, len: 0 }
    }
}

#[cfg(feature = "std")]
impl<F: std::io::Read + std::io::Write + std::io::Seek> BlobSpill for FileSpill<F> {
    fn append(&mut self, bytes: &[u8]) -> Result<u64, SinkError> {
        let at = self.len;
        self.file.seek(std::io::SeekFrom::Start(at))?;
        self.file.write_all(bytes)?;
        self.len += bytes.len() as u64;
        Ok(at)
    }

    fn read_at(&mut self, at: u64, out: &mut [u8]) -> Result<(), SinkError> {
        self.file.seek(std::io::SeekFrom::Start(at))?;
        Ok(self.file.read_exact(out)?)
    }
}

/// A spilled store's traffic (RE-588: the PSP's Memory Stick time).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SpillStats {
    pub appends: usize,
    pub bytes_written: usize,
    pub reads: usize,
    pub bytes_read: usize,
}

/// One stored chunk.
#[derive(Clone, Copy)]
struct Stored {
    page: u32,
    at: u32,
    /// Bytes in the page.
    stored: u32,
    raw: u32,
    compressed: bool,
}

/// One page: in RAM, or in the spill at `(offset, len)`.
enum Page {
    Ram(Vec<u8>),
    Spilled(u64, u32),
}

/// The blob: chunk `i` is the `i`-th [`BlobStore::push`].
#[derive(Default)]
pub struct BlobStore {
    /// The raw blob's length, alignment padding included.
    len: usize,
    raw_bytes: usize,
    pages: Vec<Page>,
    /// The page small chunks fill.
    open: Option<usize>,
    chunks: Vec<Stored>,
    /// The encoder's hash table: `base + position + 1` per 4-byte hash.
    table: Vec<u32>,
    base: u32,
    scratch: Vec<u8>,
    spill: Option<Box<dyn BlobSpill>>,
    /// Spilled shared pages read back, least recently used first.
    cache: Vec<(u32, Vec<u8>)>,
    /// A spilled solo chunk read back.
    solo: Vec<u8>,
    /// The first spill failure: later pages are dropped and reads come
    /// back empty; [`Self::take_error`] reports it.
    error: Option<SinkError>,
    stats: SpillStats,
}

impl BlobStore {
    /// A store that spills its full pages to `spill`.
    pub fn with_spill(spill: Box<dyn BlobSpill>) -> Self {
        Self {
            spill: Some(spill),
            ..Self::default()
        }
    }

    /// The raw blob's length.
    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Bytes held in RAM: pages' and caches' capacity plus the chunk
    /// records.
    pub fn held(&self) -> usize {
        self.pages
            .iter()
            .map(|p| match p {
                Page::Ram(v) => v.capacity(),
                Page::Spilled(..) => 0,
            })
            .sum::<usize>()
            + self.pages.capacity() * core::mem::size_of::<Page>()
            + self.chunks.capacity() * core::mem::size_of::<Stored>()
            + self.table.capacity() * 4
            + self.scratch.capacity()
            + self.cache.iter().map(|(_, v)| v.capacity()).sum::<usize>()
            + self.solo.capacity()
    }

    /// The chunks' raw bytes (no padding).
    pub fn raw_bytes(&self) -> usize {
        self.raw_bytes
    }

    /// The spill's traffic so far.
    pub fn spill_stats(&self) -> SpillStats {
        self.stats
    }

    /// The first spill failure, if any.
    pub fn take_error(&mut self) -> Option<SinkError> {
        self.error.take()
    }

    /// Appends `bytes` at the next aligned offset, returning it.
    pub fn push(&mut self, bytes: &[u8]) -> u32 {
        let at = align_up(self.len);
        self.len = at + bytes.len();
        self.raw_bytes += bytes.len();
        if self.table.is_empty() {
            self.table = alloc::vec![0; TABLE_LEN];
        }
        if self.base as usize + bytes.len() + 1 >= u32::MAX as usize {
            self.table.fill(0);
            self.base = 0;
        }
        let mut scratch = core::mem::take(&mut self.scratch);
        scratch.clear();
        compress(bytes, &mut scratch, &mut self.table, self.base);
        self.base += bytes.len() as u32 + 1;
        let compressed = scratch.len() < bytes.len();
        let data = if compressed { &scratch[..] } else { bytes };
        let (page, at_in_page) = self.store(data);
        self.chunks.push(Stored {
            page: page as u32,
            at: at_in_page as u32,
            stored: data.len() as u32,
            raw: bytes.len() as u32,
            compressed,
        });
        self.scratch = scratch;
        #[cfg(debug_assertions)]
        if self.error.is_none() {
            let mut back = Vec::new();
            self.read_chunk(self.chunks.len() - 1, &mut back);
            debug_assert!(back == bytes, "blob store round trip");
        }
        at as u32
    }

    /// `bytes` to the spill as one page (or dropped after a failure).
    fn spill_page(&mut self, bytes: &[u8]) -> Page {
        let spill = self.spill.as_mut().expect("spilling store");
        if self.error.is_none() {
            match spill.append(bytes) {
                Ok(at) => {
                    self.stats.appends += 1;
                    self.stats.bytes_written += bytes.len();
                    return Page::Spilled(at, bytes.len() as u32);
                }
                Err(e) => self.error = Some(e),
            }
        }
        Page::Spilled(u64::MAX, bytes.len() as u32)
    }

    /// Stores `data` in a page, returning the page and where in it.
    fn store(&mut self, data: &[u8]) -> (usize, usize) {
        let n = data.len();
        if n > PAGE / 4 {
            let page = if self.spill.is_some() {
                self.spill_page(data)
            } else {
                Page::Ram(data.to_vec())
            };
            self.pages.push(page);
            return (self.pages.len() - 1, 0);
        }
        let full = self.open.is_none_or(|p| match &self.pages[p] {
            Page::Ram(v) => v.len() + n > PAGE,
            Page::Spilled(..) => true,
        });
        if full {
            let mut buf = Vec::new();
            if let Some(p) = self.open.filter(|_| self.spill.is_some()) {
                // Spill the full page and fill its buffer again.
                let Page::Ram(old) = core::mem::replace(&mut self.pages[p], Page::Spilled(0, 0))
                else {
                    unreachable!("the open page is in RAM");
                };
                self.pages[p] = self.spill_page(&old);
                buf = old;
                buf.clear();
            }
            if buf.capacity() < PAGE {
                buf = Vec::with_capacity(PAGE);
            }
            self.pages.push(Page::Ram(buf));
            self.open = Some(self.pages.len() - 1);
        }
        let page = self.open.expect("open page");
        let Page::Ram(v) = &mut self.pages[page] else {
            unreachable!("the open page is in RAM");
        };
        let at = v.len();
        v.extend_from_slice(data);
        (page, at)
    }

    /// A spilled shared page's index in [`Self::cache`], read if absent.
    fn cached(&mut self, page: u32, at: u64, len: u32) -> Result<usize, SinkError> {
        if let Some(k) = self.cache.iter().position(|(p, _)| *p == page) {
            let entry = self.cache.remove(k);
            self.cache.push(entry);
            return Ok(self.cache.len() - 1);
        }
        let mut buf = if self.cache.len() >= CACHE_PAGES {
            self.cache.remove(0).1
        } else {
            Vec::with_capacity(PAGE)
        };
        buf.clear();
        buf.resize(len as usize, 0);
        self.read_spill(at, &mut buf)?;
        self.cache.push((page, buf));
        Ok(self.cache.len() - 1)
    }

    fn read_spill(&mut self, at: u64, out: &mut [u8]) -> Result<(), SinkError> {
        if at == u64::MAX {
            return Err("blob spill page was not written".into());
        }
        let spill = self.spill.as_mut().expect("spilling store");
        spill.read_at(at, out)?;
        self.stats.reads += 1;
        self.stats.bytes_read += out.len();
        Ok(())
    }

    /// Chunk `c`'s stored bytes, reading a spilled page back if need be.
    fn stored_bytes(&mut self, c: Stored) -> Result<&[u8], SinkError> {
        let (from, to) = (c.at as usize, (c.at + c.stored) as usize);
        let spilled = match &self.pages[c.page as usize] {
            Page::Ram(_) => None,
            &Page::Spilled(at, len) => Some((at, len)),
        };
        match spilled {
            None => {
                let Page::Ram(v) = &self.pages[c.page as usize] else {
                    unreachable!("checked above");
                };
                Ok(&v[from..to])
            }
            // A solo page: read just this chunk.
            Some((at, len)) if len as usize > PAGE / 4 && c.at == 0 && c.stored == len => {
                let mut solo = core::mem::take(&mut self.solo);
                solo.clear();
                solo.resize(len as usize, 0);
                let read = self.read_spill(at, &mut solo);
                self.solo = solo;
                read?;
                Ok(&self.solo[..])
            }
            Some((at, len)) => {
                let k = self.cached(c.page, at, len)?;
                Ok(&self.cache[k].1[from..to])
            }
        }
    }

    /// Chunk `i`'s raw bytes into `out` (replacing its contents). After a
    /// spill failure ([`Self::take_error`]) `out` may come back empty.
    pub fn read_chunk(&mut self, i: usize, out: &mut Vec<u8>) {
        let c = self.chunks[i];
        out.clear();
        let data = match self.stored_bytes(c) {
            Ok(data) => data,
            Err(e) => {
                self.error.get_or_insert(e);
                return;
            }
        };
        if c.compressed {
            decompress(data, c.raw as usize, out).expect("blob store chunk corrupt");
        } else {
            out.extend_from_slice(data);
        }
    }
}

fn u32_le(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

fn hash(v: u32) -> usize {
    (v.wrapping_mul(2_654_435_761) >> (32 - HASH_BITS)) as usize
}

/// A length's extension bytes past the token's nibble.
fn put_len(dst: &mut Vec<u8>, mut n: usize) {
    while n >= 255 {
        dst.push(255);
        n -= 255;
    }
    dst.push(n as u8);
}

/// One sequence: literals, then (unless `last`) a match.
fn put_sequence(dst: &mut Vec<u8>, lit: &[u8], m: Option<(usize, usize)>) {
    let ml = m.map_or(0, |(_, len)| len - MIN_MATCH);
    let token = ((lit.len().min(15) as u8) << 4) | ml.min(15) as u8;
    dst.push(token);
    if lit.len() >= 15 {
        put_len(dst, lit.len() - 15);
    }
    dst.extend_from_slice(lit);
    if let Some((offset, _)) = m {
        dst.extend_from_slice(&(offset as u16).to_le_bytes());
        if ml >= 15 {
            put_len(dst, ml - 15);
        }
    }
}

/// Candidates kept per hash bucket, newest first.
const WAYS: usize = 4;

/// The encoder's hash table length (`u32` entries).
pub const TABLE_LEN: usize = WAYS << HASH_BITS;

/// Records position `i` in its bucket.
fn insert(src: &[u8], table: &mut [u32], base: u32, i: usize) {
    let b = hash(u32_le(src, i)) * WAYS;
    table.copy_within(b..b + WAYS - 1, b + 1);
    table[b] = base + i as u32 + 1;
}

/// The longest earlier match for position `i` (offset, length), nearest
/// first on a tie.
fn best_match(src: &[u8], table: &[u32], base: u32, i: usize) -> Option<(usize, usize)> {
    let v = u32_le(src, i);
    let b = hash(v) * WAYS;
    let mut best: Option<(usize, usize)> = None;
    for &e in &table[b..b + WAYS] {
        if e <= base {
            break;
        }
        let c = (e - base - 1) as usize;
        if c >= i || i - c > MAX_OFFSET || u32_le(src, c) != v {
            continue;
        }
        let mut len = MIN_MATCH;
        while i + len < src.len() && src[c + len] == src[i + len] {
            len += 1;
        }
        if best.is_none_or(|(_, l)| len > l) {
            best = Some((i - c, len));
        }
    }
    best
}

/// Compresses `src` as one block onto `dst`. `table` ([`TABLE_LEN`]
/// entries) holds positions as `base + position + 1`, so entries older than
/// `base` (earlier blocks) never match and the table needs no clearing
/// between blocks. Greedy with one step of lazy matching.
pub fn compress(src: &[u8], dst: &mut Vec<u8>, table: &mut [u32], base: u32) {
    let (mut i, mut anchor) = (0usize, 0usize);
    while i + MIN_MATCH <= src.len() {
        let Some(mut m) = best_match(src, table, base, i) else {
            insert(src, table, base, i);
            // Skip faster through incompressible runs.
            let step = 1 + ((i - anchor) >> 6);
            i += step;
            continue;
        };
        insert(src, table, base, i);
        // A longer match one byte on wins over this one.
        if i + 1 + MIN_MATCH <= src.len() {
            if let Some(next) = best_match(src, table, base, i + 1).filter(|n| n.1 > m.1 + 1) {
                i += 1;
                insert(src, table, base, i);
                m = next;
            }
        }
        put_sequence(dst, &src[anchor..i], Some(m));
        let end = i + m.1;
        // Index inside the match so later matches can reach it.
        let mut t = i + 1;
        while t + MIN_MATCH <= end.min(src.len()) {
            insert(src, table, base, t);
            t += 1 + (m.1 >> 5);
        }
        if end + 2 <= src.len() && end >= 2 {
            insert(src, table, base, end - 2);
        }
        i = end;
        anchor = i;
    }
    put_sequence(dst, &src[anchor..], None);
}

/// Decompresses one [`compress`] block of `raw_len` bytes onto `out`;
/// `None` if the block is malformed.
pub fn decompress(src: &[u8], raw_len: usize, out: &mut Vec<u8>) -> Option<()> {
    let start = out.len();
    out.reserve(raw_len);
    let mut i = 0usize;
    let get_len = |i: &mut usize, nibble: usize| -> Option<usize> {
        let mut n = nibble;
        if nibble == 15 {
            loop {
                let b = *src.get(*i)?;
                *i += 1;
                n += b as usize;
                if b != 255 {
                    break;
                }
            }
        }
        Some(n)
    };
    loop {
        let token = *src.get(i)?;
        i += 1;
        let lit = get_len(&mut i, (token >> 4) as usize)?;
        out.extend_from_slice(src.get(i..i + lit)?);
        i += lit;
        if i == src.len() {
            break;
        }
        let offset = u16::from_le_bytes([*src.get(i)?, *src.get(i + 1)?]) as usize;
        i += 2;
        let len = get_len(&mut i, (token & 15) as usize)? + MIN_MATCH;
        let from = (out.len() - start)
            .checked_sub(offset)
            .filter(|_| offset > 0)?
            + start;
        if out.len() - start + len > raw_len {
            return None;
        }
        if offset >= len {
            out.extend_from_within(from..from + len);
        } else {
            for k in 0..len {
                let b = out[from + k];
                out.push(b);
            }
        }
    }
    (out.len() - start == raw_len).then_some(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip(data: &[u8]) -> usize {
        let mut table = alloc::vec![0u32; TABLE_LEN];
        let mut c = Vec::new();
        compress(data, &mut c, &mut table, 7);
        let mut d = alloc::vec![0xAA];
        decompress(&c, data.len(), &mut d).expect("decodes");
        assert_eq!(&d[1..], data);
        c.len()
    }

    #[test]
    fn codec_round_trips() {
        round_trip(&[]);
        round_trip(&[1]);
        round_trip(&[1, 2, 3, 4, 5]);
        let zeros = alloc::vec![0u8; 100_000];
        assert!(round_trip(&zeros) < 1_000);
        let mut x = 0x1234_5678u32;
        let noise: Vec<u8> = (0..70_000)
            .map(|_| {
                x ^= x << 13;
                x ^= x >> 17;
                x ^= x << 5;
                x as u8
            })
            .collect();
        round_trip(&noise);
        // Repeats past the 64 KiB window, overlapping copies, long literals.
        let mut mixed = noise[..300].to_vec();
        mixed.extend_from_slice(&noise[..300]);
        mixed.extend(core::iter::repeat_n(9u8, 1_000));
        mixed.extend_from_slice(&noise);
        mixed.extend_from_slice(&noise[..5_000]);
        mixed.extend_from_slice(&[1, 2, 1, 2, 1, 2, 1, 2, 1, 2, 3]);
        round_trip(&mixed);
    }

    #[test]
    fn store_reads_back_every_chunk() {
        let mut s = BlobStore::default();
        let mut x = 99u32;
        let mut want = Vec::new();
        for n in [
            0usize, 3, 17, 4_000, 20_000, 0, 70_000, 5, 16_385, 64, 300_000,
        ] {
            let chunk: Vec<u8> = (0..n)
                .map(|k| {
                    x = x.wrapping_mul(1_103_515_245).wrapping_add(12_345);
                    if k % 7 < 3 {
                        (x >> 24) as u8
                    } else {
                        (k / 13) as u8
                    }
                })
                .collect();
            let at = s.push(&chunk);
            assert_eq!(at as usize % crate::pack::ALIGN, 0);
            want.push(chunk);
        }
        let mut out = Vec::new();
        for (i, w) in want.iter().enumerate() {
            s.read_chunk(i, &mut out);
            assert_eq!(&out, w, "chunk {i}");
        }
    }

    /// Chunks of a spilled store read back the same, in any order, and the
    /// RAM it holds stays near one page and the cache.
    #[test]
    fn spilled_store_reads_back_every_chunk() {
        let (mut ram, mut spilled) = (
            BlobStore::default(),
            BlobStore::with_spill(Box::new(MemSpill::default())),
        );
        let mut x = 7u32;
        let mut want = Vec::new();
        for k in 0..3_000usize {
            let n = [0usize, 3, 40, 900, 5_000, 20_000, 70_000][k % 7] + k % 13;
            let chunk: Vec<u8> = (0..n)
                .map(|j| {
                    x = x.wrapping_mul(1_103_515_245).wrapping_add(12_345);
                    if j % 5 < 2 {
                        (x >> 24) as u8
                    } else {
                        (j / 7) as u8
                    }
                })
                .collect();
            assert_eq!(ram.push(&chunk), spilled.push(&chunk));
            want.push(chunk);
        }
        assert!(spilled.take_error().is_none());
        assert!(
            spilled.held() < ram.held() / 4,
            "spilled store holds {} of {}",
            spilled.held(),
            ram.held()
        );
        let mut out = Vec::new();
        // Forward, backward and strided: cache hits, misses and evictions.
        let order = (0..want.len())
            .chain((0..want.len()).rev())
            .chain((0..want.len()).map(|i| i * 37 % want.len()));
        for i in order {
            spilled.read_chunk(i, &mut out);
            assert_eq!(out, want[i], "chunk {i}");
        }
        assert!(spilled.take_error().is_none());
        let stats = spilled.spill_stats();
        assert!(stats.appends > 0 && stats.reads > 0, "{stats:?}");
    }
}
