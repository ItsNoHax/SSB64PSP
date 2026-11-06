//! The pack writer's blob held compressed in RAM (RE-588): the on-device
//! build cannot hold the raw blob, so each chunk is compressed as it is
//! pushed and decompressed when `PackWriter::finish_to` streams it out.
//! Lossless and private to the writer: the pack's bytes do not change.
//!
//! - Codec: byte-oriented LZ77 (LZ4-like sequences), one independent block
//!   per chunk, so any chunk reads back alone.
//! - Storage: 64 KiB pages (no doubling slack, no large reallocation); a
//!   chunk larger than a quarter page gets a block of its own.

use alloc::vec::Vec;

use crate::pack::align_up;

/// Compressed-stream page size.
const PAGE: usize = 64 * 1024;
const MIN_MATCH: usize = 4;
const MAX_OFFSET: usize = 0xFFFF;
const HASH_BITS: u32 = 14;

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

/// The blob: chunk `i` is the `i`-th [`BlobStore::push`].
#[derive(Default)]
pub struct BlobStore {
    /// The raw blob's length, alignment padding included.
    len: usize,
    raw_bytes: usize,
    pages: Vec<Vec<u8>>,
    /// The page small chunks fill.
    open: Option<usize>,
    chunks: Vec<Stored>,
    /// The encoder's hash table: `base + position + 1` per 4-byte hash.
    table: Vec<u32>,
    base: u32,
    scratch: Vec<u8>,
}

impl BlobStore {
    /// The raw blob's length.
    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Bytes held: pages' capacity plus the chunk records.
    pub fn held(&self) -> usize {
        self.pages.iter().map(Vec::capacity).sum::<usize>()
            + self.chunks.capacity() * core::mem::size_of::<Stored>()
            + self.table.capacity() * 4
            + self.scratch.capacity()
    }

    /// The chunks' raw bytes (no padding).
    pub fn raw_bytes(&self) -> usize {
        self.raw_bytes
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
        let (page, at_in_page) = self.reserve(data.len());
        self.pages[page].extend_from_slice(data);
        self.chunks.push(Stored {
            page: page as u32,
            at: at_in_page as u32,
            stored: data.len() as u32,
            raw: bytes.len() as u32,
            compressed,
        });
        self.scratch = scratch;
        #[cfg(debug_assertions)]
        {
            let mut back = Vec::new();
            self.read_chunk(self.chunks.len() - 1, &mut back);
            debug_assert!(back == bytes, "blob store round trip");
        }
        at as u32
    }

    /// A page with room for `n` more bytes, and where they go in it.
    fn reserve(&mut self, n: usize) -> (usize, usize) {
        if n > PAGE / 4 {
            self.pages.push(Vec::with_capacity(n));
            return (self.pages.len() - 1, 0);
        }
        if self.open.is_none_or(|p| self.pages[p].len() + n > PAGE) {
            self.pages.push(Vec::with_capacity(PAGE));
            self.open = Some(self.pages.len() - 1);
        }
        let page = self.open.expect("open page");
        (page, self.pages[page].len())
    }

    /// Chunk `i`'s raw bytes into `out` (replacing its contents).
    pub fn read_chunk(&self, i: usize, out: &mut Vec<u8>) {
        let c = self.chunks[i];
        let data = &self.pages[c.page as usize][c.at as usize..(c.at + c.stored) as usize];
        out.clear();
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
}
