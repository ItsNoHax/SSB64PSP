//! Word-at-a-time `memcpy`, `memmove` and `memset` (RE-471).
//!
//! The PSP's `psp` crate defines the C memory functions one byte per loop
//! iteration, five instructions a byte, and every large Rust move or
//! zeroing calls them: a tenth to a fifth of a battle's CPU time. These
//! copy four bytes per load and store when source and destination share
//! their alignment, and are what the PSP binaries link in their place
//! (`psp_runtime::memops`).
//!
//! Every access is volatile so the compiler cannot recognise a loop here as
//! a copy and call `memcpy` from inside `memcpy`.

use core::ptr::{read_volatile, write_volatile};

/// Copies `n` bytes from `src` to `dst`, front to back.
///
/// # Safety
/// As `core::ptr::copy` front to back: valid ranges, and when they overlap
/// `dst` must not be above `src`.
pub unsafe fn copy_forward(dst: *mut u8, src: *const u8, n: usize) {
    let (mut d, mut s, mut n) = (dst, src, n);
    if n >= 8 && (d as usize ^ s as usize) & 3 == 0 {
        while d as usize & 3 != 0 {
            write_volatile(d, read_volatile(s));
            d = d.add(1);
            s = s.add(1);
            n -= 1;
        }
        let (mut dw, mut sw) = (d as *mut u32, s as *const u32);
        while n >= 16 {
            let (a, b, c, e) = (
                read_volatile(sw),
                read_volatile(sw.add(1)),
                read_volatile(sw.add(2)),
                read_volatile(sw.add(3)),
            );
            write_volatile(dw, a);
            write_volatile(dw.add(1), b);
            write_volatile(dw.add(2), c);
            write_volatile(dw.add(3), e);
            dw = dw.add(4);
            sw = sw.add(4);
            n -= 16;
        }
        while n >= 4 {
            write_volatile(dw, read_volatile(sw));
            dw = dw.add(1);
            sw = sw.add(1);
            n -= 4;
        }
        d = dw as *mut u8;
        s = sw as *const u8;
    }
    while n > 0 {
        write_volatile(d, read_volatile(s));
        d = d.add(1);
        s = s.add(1);
        n -= 1;
    }
}

/// Copies `n` bytes from `src` to `dst`, back to front.
///
/// # Safety
/// As `core::ptr::copy` back to front: valid ranges, and when they overlap
/// `dst` must not be below `src`.
pub unsafe fn copy_backward(dst: *mut u8, src: *const u8, n: usize) {
    let (mut d, mut s, mut n) = (dst.add(n), src.add(n), n);
    if n >= 8 && (d as usize ^ s as usize) & 3 == 0 {
        while d as usize & 3 != 0 {
            d = d.sub(1);
            s = s.sub(1);
            write_volatile(d, read_volatile(s));
            n -= 1;
        }
        let (mut dw, mut sw) = (d as *mut u32, s as *const u32);
        while n >= 4 {
            dw = dw.sub(1);
            sw = sw.sub(1);
            write_volatile(dw, read_volatile(sw));
            n -= 4;
        }
        d = dw as *mut u8;
        s = sw as *const u8;
    }
    while n > 0 {
        d = d.sub(1);
        s = s.sub(1);
        write_volatile(d, read_volatile(s));
        n -= 1;
    }
}

/// `memmove`: copies `n` bytes between possibly overlapping ranges.
///
/// # Safety
/// Both ranges valid for `n` bytes.
pub unsafe fn move_bytes(dst: *mut u8, src: *const u8, n: usize) {
    if (dst as usize) <= (src as usize) {
        copy_forward(dst, src, n);
    } else {
        copy_backward(dst, src, n);
    }
}

/// `memset`: fills `n` bytes at `dst` with `value`.
///
/// # Safety
/// `dst` valid for `n` bytes.
pub unsafe fn fill(dst: *mut u8, value: u8, n: usize) {
    let (mut d, mut n) = (dst, n);
    if n >= 8 {
        while d as usize & 3 != 0 {
            write_volatile(d, value);
            d = d.add(1);
            n -= 1;
        }
        let word = u32::from(value) * 0x0101_0101;
        let mut dw = d as *mut u32;
        while n >= 16 {
            write_volatile(dw, word);
            write_volatile(dw.add(1), word);
            write_volatile(dw.add(2), word);
            write_volatile(dw.add(3), word);
            dw = dw.add(4);
            n -= 16;
        }
        while n >= 4 {
            write_volatile(dw, word);
            dw = dw.add(1);
            n -= 4;
        }
        d = dw as *mut u8;
    }
    while n > 0 {
        write_volatile(d, value);
        d = d.add(1);
        n -= 1;
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    use std::vec;
    use std::vec::Vec;

    fn pattern(len: usize) -> Vec<u8> {
        (0..len).map(|i| (i * 7 + 3) as u8).collect()
    }

    #[test]
    fn copies_every_length_and_alignment() {
        for len in 0..70 {
            for so in 0..4 {
                for do_ in 0..4 {
                    let src = pattern(len + 8);
                    let mut dst = vec![0xEEu8; len + 8];
                    unsafe { copy_forward(dst.as_mut_ptr().add(do_), src.as_ptr().add(so), len) };
                    let mut want = vec![0xEEu8; len + 8];
                    want[do_..do_ + len].copy_from_slice(&src[so..so + len]);
                    assert_eq!(dst, want, "forward len {len} src+{so} dst+{do_}");
                    let mut dst = vec![0xEEu8; len + 8];
                    unsafe { copy_backward(dst.as_mut_ptr().add(do_), src.as_ptr().add(so), len) };
                    assert_eq!(dst, want, "backward len {len} src+{so} dst+{do_}");
                }
            }
        }
    }

    #[test]
    fn moves_overlapping_ranges() {
        for len in 0..70 {
            for from in 0..8 {
                for to in 0..8 {
                    let mut buf = pattern(len + 8);
                    let mut want = buf.clone();
                    want.copy_within(from..from + len, to);
                    let p = buf.as_mut_ptr();
                    unsafe { move_bytes(p.add(to), p.add(from), len) };
                    assert_eq!(buf, want, "len {len} {from} -> {to}");
                }
            }
        }
    }

    #[test]
    fn fills_every_length_and_alignment() {
        for len in 0..70 {
            for off in 0..4 {
                let mut buf = vec![0x11u8; len + 8];
                unsafe { fill(buf.as_mut_ptr().add(off), 0xA5, len) };
                let mut want = vec![0x11u8; len + 8];
                want[off..off + len].fill(0xA5);
                assert_eq!(buf, want, "len {len} +{off}");
            }
        }
    }
}
