//! Sample-buffer copies and fills for the render path.
//!
//! The PSP runtime's `memcpy`, `memmove` and `memset` (rust-psp) move one
//! byte per loop iteration, about six cycles a byte on the Allegrex, and
//! LLVM lowers every `copy_from_slice`, `copy_within`, `fill` and zeroed
//! array to them. These helpers move whole words instead on the PSP (the
//! audio buffers are word aligned); elsewhere they are the slice methods.

/// `dst.copy_from_slice(src)` (same lengths; no overlap, as slices
/// guarantee).
#[inline]
pub fn copy(dst: &mut [i16], src: &[i16]) {
    assert_eq!(dst.len(), src.len());
    #[cfg(target_arch = "mips")]
    // SAFETY: two valid, non-overlapping ranges of the same length.
    unsafe {
        imp::copy_fwd(dst.as_mut_ptr(), src.as_ptr(), dst.len());
    }
    #[cfg(not(target_arch = "mips"))]
    dst.copy_from_slice(src);
}

/// `w.copy_within(src..src + n, dst)` (memmove semantics).
#[inline]
pub fn copy_within(w: &mut [i16], src: usize, dst: usize, n: usize) {
    assert!(src.max(dst) + n <= w.len());
    #[cfg(target_arch = "mips")]
    // SAFETY: both ranges are inside `w` (checked above); the direction
    // makes overlapping moves read each sample before it is overwritten.
    unsafe {
        let p = w.as_mut_ptr();
        if dst <= src {
            imp::copy_fwd(p.add(dst), p.add(src), n);
        } else {
            imp::copy_back(p.add(dst), p.add(src), n);
        }
    }
    #[cfg(not(target_arch = "mips"))]
    w.copy_within(src..src + n, dst);
}

/// `dst.fill(0)`.
#[inline]
pub fn zero(dst: &mut [i16]) {
    #[cfg(target_arch = "mips")]
    // SAFETY: a valid range.
    unsafe {
        imp::zero(dst.as_mut_ptr(), dst.len());
    }
    #[cfg(not(target_arch = "mips"))]
    dst.fill(0);
}

#[cfg(target_arch = "mips")]
mod imp {
    use core::arch::asm;

    /// Forward copy of `n` samples (also correct for `dst <= src` overlaps).
    pub unsafe fn copy_fwd(mut d: *mut i16, mut s: *const i16, mut n: usize) {
        unsafe {
            if (d as usize | s as usize) & 3 == 0 && n >= 2 {
                let words = n / 2;
                let end = d.add(words * 2);
                // Two words per iteration, then an odd word.
                let pairs = words / 2;
                if pairs > 0 {
                    let pend = d.add(pairs * 4);
                    asm!(
                        ".set push",
                        ".set noreorder",
                        "1:",
                        "lw $8, 0($5)",
                        "lw $9, 4($5)",
                        "addiu $5, $5, 8",
                        "sw $8, 0($4)",
                        "addiu $4, $4, 8",
                        "bne $4, $6, 1b",
                        "sw $9, -4($4)",
                        ".set pop",
                        inout("$4") d => d,
                        inout("$5") s => s,
                        in("$6") pend,
                        out("$8") _,
                        out("$9") _,
                        options(nostack),
                    );
                }
                if d != end {
                    *(d as *mut u32) = *(s as *const u32);
                    d = d.add(2);
                    s = s.add(2);
                }
                n -= words * 2;
            }
            if n > 0 {
                asm!(
                    ".set push",
                    ".set noreorder",
                    "1:",
                    "lh $8, 0($5)",
                    "addiu $5, $5, 2",
                    "addiu $4, $4, 2",
                    "addiu $6, $6, -1",
                    "bnez $6, 1b",
                    "sh $8, -2($4)",
                    ".set pop",
                    inout("$4") d => _,
                    inout("$5") s => _,
                    inout("$6") n => _,
                    out("$8") _,
                    options(nostack),
                );
            }
        }
    }

    /// Backward copy of `n` samples, for `dst > src` overlaps.
    pub unsafe fn copy_back(d: *mut i16, s: *const i16, n: usize) {
        if n == 0 {
            return;
        }
        unsafe {
            asm!(
                ".set push",
                ".set noreorder",
                "1:",
                "lh $8, -2($5)",
                "addiu $5, $5, -2",
                "addiu $4, $4, -2",
                "addiu $6, $6, -1",
                "bnez $6, 1b",
                "sh $8, 0($4)",
                ".set pop",
                inout("$4") d.add(n) => _,
                inout("$5") s.add(n) => _,
                inout("$6") n => _,
                out("$8") _,
                options(nostack),
            );
        }
    }

    /// Zeroes `n` samples.
    pub unsafe fn zero(mut d: *mut i16, mut n: usize) {
        unsafe {
            if d as usize & 2 != 0 && n > 0 {
                *d = 0;
                d = d.add(1);
                n -= 1;
            }
            let words = n / 2;
            if words > 0 {
                asm!(
                    ".set push",
                    ".set noreorder",
                    "1:",
                    "addiu $4, $4, 4",
                    "bne $4, $5, 1b",
                    "sw $0, -4($4)",
                    ".set pop",
                    inout("$4") d => d,
                    in("$5") d.add(words * 2),
                    options(nostack),
                );
            }
            if n & 1 != 0 {
                *d = 0;
            }
        }
    }
}
