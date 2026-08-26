//! VPK0 decompression.
//!
//! VPK0 is the LZ+Huffman container used for the compressed entries of the
//! `relocData` archive. This is a direct port of `syDmaDecodeVpk0`
//! (`src/sys/dma.c` in the decompilation), which is the authority on the
//! format — in particular on the two quirks below, which are easy to get
//! wrong from a format description alone:
//!
//! * The Huffman trees are stored in **postfix** order. A `0` bit introduces a
//!   leaf carrying an 8-bit *width*, and a `1` bit pops the top two nodes off
//!   the stack and pushes an internal node. A `1` bit seen while fewer than
//!   two nodes are on the stack terminates the tree.
//! * A decoded leaf's value is not the offset/length itself, it is the *number
//!   of bits to read next* to obtain that value.
//!
//! Back-references may overlap the write cursor (`copy_src` can be less than
//! 8 bytes behind `out_ptr`), so the copy has to be byte-at-a-time.

use alloc::vec::Vec;

use crate::{Error, Result};

/// Number of Huffman nodes the original allocates on the stack (`sp14C[64]`).
/// A stream needing more than this would have overflowed on real hardware, so
/// treating it as a hard limit keeps us bug-compatible.
const MAX_NODES: usize = 64;

/// The original walks trees with a 20-deep explicit stack.
const MAX_TREE_STACK: usize = 20;

#[derive(Clone, Copy, Default)]
struct Node {
    left: u16,
    right: u16,
    value: u32,
}

const NIL: u16 = u16::MAX;

/// A bit reader over a big-endian `u16` stream.
///
/// The original refills 16 bits at a time from a DMA window; we have the whole
/// ROM mapped, so we just walk the slice, but the bit order has to match
/// exactly.
struct BitReader<'a> {
    data: &'a [u8],
    pos: usize,
    /// Bit accumulator. Only the low `bits` bits are meaningful.
    acc: u32,
    bits: i32,
}

impl<'a> BitReader<'a> {
    fn new(data: &'a [u8]) -> Self {
        BitReader {
            data,
            pos: 0,
            acc: 0,
            bits: 0,
        }
    }

    fn refill(&mut self) -> Result<()> {
        let b = self
            .data
            .get(self.pos..self.pos + 2)
            .ok_or(Error::Vpk0("stream ended mid-symbol"))?;
        self.acc = (self.acc << 16) | u32::from(u16::from_be_bytes([b[0], b[1]]));
        self.bits += 16;
        self.pos += 2;
        Ok(())
    }

    /// Reads `n` bits (0..=32), MSB first.
    fn get(&mut self, n: u32) -> Result<u32> {
        if n == 0 {
            return Ok(0);
        }
        if n > 32 {
            return Err(Error::Vpk0("symbol width above 32 bits"));
        }
        while self.bits < n as i32 {
            self.refill()?;
        }
        self.bits -= n as i32;
        // Mirrors VPK0_GET_BITS: shift the wanted field up to the top of the
        // word, then back down, which also masks off everything above it.
        let shifted = self.acc << (32 - n - self.bits as u32);
        Ok(shifted >> (32 - n))
    }
}

struct Arena {
    nodes: [Node; MAX_NODES],
    len: u16,
}

impl Arena {
    fn new() -> Self {
        Arena {
            nodes: [Node {
                left: NIL,
                right: NIL,
                value: 0,
            }; MAX_NODES],
            len: 0,
        }
    }

    fn alloc(&mut self) -> Result<u16> {
        if self.len as usize >= MAX_NODES {
            return Err(Error::Vpk0("Huffman tree exceeds 64 nodes"));
        }
        let i = self.len;
        self.nodes[i as usize] = Node {
            left: NIL,
            right: NIL,
            value: 0,
        };
        self.len += 1;
        Ok(i)
    }
}

/// Reads one postfix-encoded Huffman tree; returns its root.
fn read_tree(r: &mut BitReader<'_>, arena: &mut Arena) -> Result<u16> {
    let mut stack = [NIL; MAX_TREE_STACK];
    let mut sp = 0usize;

    loop {
        let is_internal = r.get(1)? != 0;

        if is_internal && sp < 2 {
            break;
        }
        if is_internal {
            let n = arena.alloc()?;
            arena.nodes[n as usize].left = stack[sp - 2];
            arena.nodes[n as usize].right = stack[sp - 1];
            stack[sp - 2] = n;
            sp -= 1;
        } else {
            if sp >= MAX_TREE_STACK {
                return Err(Error::Vpk0("Huffman tree stack overflow"));
            }
            let n = arena.alloc()?;
            arena.nodes[n as usize].value = r.get(8)?;
            stack[sp] = n;
            sp += 1;
        }
    }

    if sp == 0 {
        return Err(Error::Vpk0("empty Huffman tree"));
    }
    Ok(stack[0])
}

/// Walks a Huffman tree to a leaf, then reads that leaf's width in bits.
fn decode_symbol(r: &mut BitReader<'_>, arena: &Arena, root: u16) -> Result<u32> {
    let mut n = root;
    while arena.nodes[n as usize].left != NIL {
        n = if r.get(1)? == 0 {
            arena.nodes[n as usize].left
        } else {
            arena.nodes[n as usize].right
        };
    }
    r.get(arena.nodes[n as usize].value)
}

/// Decompresses a VPK0 stream starting at the beginning of `data`.
///
/// `data` may extend past the end of the compressed stream; decoding stops
/// once the decompressed size recorded in the header has been produced.
pub fn decompress(data: &[u8]) -> Result<Vec<u8>> {
    if data.get(..4) != Some(b"vpk0") {
        return Err(Error::Vpk0("missing 'vpk0' magic"));
    }

    let mut r = BitReader::new(data);
    // Header: magic (32 bits) then decompressed size (32 bits), both consumed
    // through the same bit reader so the accumulator stays aligned.
    let _magic = r.get(32)?;
    let out_len = r.get(32)? as usize;
    let sample_method = r.get(8)?;

    let mut arena = Arena::new();
    let offsets_tree = read_tree(&mut r, &mut arena)?;
    let lengths_tree = read_tree(&mut r, &mut arena)?;

    let mut out: Vec<u8> = Vec::with_capacity(out_len);

    while out.len() < out_len {
        if r.get(1)? == 0 {
            out.push(r.get(8)? as u8);
            continue;
        }

        // Back-reference. `sample_method` selects how the distance is encoded.
        let distance = if sample_method != 0 {
            // Two-sample form: a coarse word-aligned distance plus an optional
            // byte-level correction.
            let mut correction = 0u32;
            let mut value = decode_symbol(&mut r, &arena, offsets_tree)?;
            if value <= 2 {
                correction = value + 1;
                value = decode_symbol(&mut r, &arena, offsets_tree)?;
            }
            // `copy_src = out_ptr - value * 4 - correction + 8`
            let back = value
                .checked_mul(4)
                .and_then(|v| v.checked_add(correction))
                .ok_or(Error::Vpk0("distance overflow"))?;
            (back as i64) - 8
        } else {
            decode_symbol(&mut r, &arena, offsets_tree)? as i64
        };

        let length = decode_symbol(&mut r, &arena, lengths_tree)? as usize;

        if distance <= 0 || (distance as usize) > out.len() {
            return Err(Error::Vpk0("back-reference before start of output"));
        }
        // Byte-at-a-time, and it has to stay that way: a run may legitimately
        // overlap the write cursor (distance < length), which is how VPK0
        // encodes repeats. `extend_from_within` would be wrong here because it
        // reserves the source range up front rather than re-reading bytes as
        // they are produced.
        let start = out.len() - distance as usize;
        for src in start..start + length {
            let b = out[src];
            out.push(b);
        }
    }

    // A final run may overshoot the declared size; the original stops checking
    // once past the end, leaving the extra bytes in the caller's buffer. We
    // truncate so the output length is exactly what the header promised.
    out.truncate(out_len);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_bad_magic() {
        assert!(matches!(
            decompress(b"nope............"),
            Err(Error::Vpk0("missing 'vpk0' magic"))
        ));
    }

    #[test]
    fn rejects_truncated_stream() {
        assert!(decompress(b"vpk0").is_err());
    }

    #[test]
    fn bit_reader_is_msb_first() {
        // 0b1010_1100_0011_0101
        let data = [0xACu8, 0x35];
        let mut r = BitReader::new(&data);
        assert_eq!(r.get(1).unwrap(), 1);
        assert_eq!(r.get(3).unwrap(), 0b010);
        assert_eq!(r.get(4).unwrap(), 0b1100);
        assert_eq!(r.get(8).unwrap(), 0x35);
    }
}
