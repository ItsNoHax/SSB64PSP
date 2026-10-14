//! A fixed input set for [`super::sqrt`] whose output hash ([`HASH`]) the
//! host test pins; a platform runs [`run`] on the target to prove its
//! square root returns the same bits (and to time it).
//!
//! Inputs: special values (zeros, denormals, the normal boundary, powers of
//! two, the largest finite values, infinities, NaNs in both the IEEE-2008
//! and the legacy-MIPS quiet encodings, negatives), then a jittered stride
//! across every positive denormal, then one across every positive finite
//! value, then negatives (which the guard turns into zero).
//!
//! Each output is checked on the spot against exact bounds: `r` is the
//! correctly rounded square root of `x` when `x` lies strictly between the
//! squares of the midpoints to `r`'s neighbours. Those midpoints have 25
//! significant bits and their squares at most 50, so `f64` multiplication
//! computes them exactly, and no `f64` square root is needed (the PSP has
//! none in hardware). A square root is never exactly a midpoint, so the
//! bounds are strict.

/// FNV-1a over the output bits of every input, NaNs written as
/// `0x7fc0_0000` (a NaN's payload and sign differ between FPUs).
pub const HASH: u64 = 0x86fb_3ce8_27c2_930e;

/// Positive finite inputs in the main stride.
pub const SWEEP: u32 = 249_001;
/// Positive denormal inputs.
pub const DENORMALS: u32 = 4_096;
/// Negative inputs.
pub const NEGATIVES: u32 = 4_096;

const SPECIALS: [u32; 34] = [
    0x0000_0000, // +0
    0x8000_0000, // -0
    0x0000_0001, // smallest denormal
    0x0000_0002,
    0x0000_0003,
    0x0000_1234,
    0x0040_0000,
    0x007f_ffff, // largest denormal
    0x0080_0000, // smallest normal
    0x0080_0001,
    0x3e80_0000, // 0.25
    0x3f00_0000, // 0.5
    0x3f80_0000, // 1
    0x3f80_0001,
    0x3f7f_ffff,
    0x4000_0000, // 2
    0x4040_0000, // 3
    0x4080_0000, // 4
    0x4b80_0000, // 2^24
    0x7f7f_fffe,
    0x7f7f_ffff, // largest finite
    0x7f80_0000, // +inf
    0xff80_0000, // -inf
    0x7fc0_0000, // NaN, quiet in IEEE-2008, signalling in legacy MIPS
    0xffc0_0000,
    0x7fbf_ffff, // NaN, quiet in legacy MIPS (the R4300's default NaN)
    0xffbf_ffff,
    0x7f80_0001, // NaN, signalling in IEEE-2008
    0x8000_0001, // negative denormal
    0x807f_ffff,
    0x8080_0000, // negative smallest normal
    0xbf80_0000, // -1
    0xff7f_ffff, // most negative finite
    0x9e3b_8c2a, // a tiny negative, like a sum of squares' rounding residue
];

/// Number of inputs.
pub const COUNT: u32 = SPECIALS.len() as u32 + DENORMALS + SWEEP + NEGATIVES;

/// A small hash of `i`, to jitter the strides off a fixed lattice.
fn jitter(i: u32) -> u32 {
    let mut h = i.wrapping_mul(0x9e37_79b9);
    h ^= h >> 15;
    h = h.wrapping_mul(0x85eb_ca6b);
    h ^ (h >> 13)
}

/// The `i`th input's bits, for `i < COUNT`.
pub fn input(i: u32) -> u32 {
    let mut i = i;
    if (i as usize) < SPECIALS.len() {
        return SPECIALS[i as usize];
    }
    i -= SPECIALS.len() as u32;
    if i < DENORMALS {
        // (0, 0x007f_ffff] in DENORMALS equal steps.
        let step = 0x0080_0000 / DENORMALS;
        return 1 + i * step + jitter(i) % (step - 1);
    }
    i -= DENORMALS;
    if i < SWEEP {
        // [0x0080_0000, 0x7f7f_ffff]: every exponent of the normals.
        let span = 0x7f80_0000u64 - 0x0080_0000;
        let step = span / u64::from(SWEEP);
        let base = 0x0080_0000 + u64::from(i) * step;
        return (base + u64::from(jitter(i)) % step) as u32;
    }
    i -= SWEEP;
    // Negative finite values, denormals included, -0 not.
    let span = 0x7f80_0000u64;
    let step = span / u64::from(NEGATIVES);
    0x8000_0000 | (1 + u64::from(i) * step + u64::from(jitter(!i)) % (step - 1)) as u32
}

/// Whether `r` is what [`super::sqrt`] must return for `x`: zero (positive)
/// for every input at or below zero, NaN for NaN, infinity for infinity,
/// else the correctly rounded square root.
pub fn correct(x: f32, r: f32) -> bool {
    if x.is_nan() {
        return r.is_nan();
    }
    if x <= 0.0 {
        return r.to_bits() == 0;
    }
    if x == f32::INFINITY {
        return r == f32::INFINITY;
    }
    if !(r.is_finite() && r > 0.0) {
        return false;
    }
    let below = f32::from_bits(r.to_bits() - 1);
    let above = f32::from_bits(r.to_bits() + 1);
    let r = f64::from(r);
    // `above` of the largest finite result is not infinity: square roots
    // stay below 2^64.
    let lo = (r + f64::from(below)) * 0.5;
    let hi = (r + f64::from(above)) * 0.5;
    let x = f64::from(x);
    lo * lo < x && x < hi * hi
}

/// What [`run`] found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Report {
    /// Inputs run ([`COUNT`]).
    pub count: u32,
    /// Wrong results among the specials, denormals, normal sweep and
    /// negatives, in that order.
    pub bad: [u32; 4],
    /// The first wrong results, `(input bits, output bits)`; zero-filled.
    pub first_bad: [(u32, u32); 4],
    /// The output hash ([`HASH`] when every bit matches the host's).
    pub hash: u64,
}

impl Report {
    /// Every result correct and every bit as on the host.
    pub fn ok(&self) -> bool {
        self.bad == [0; 4] && self.hash == HASH
    }
}

/// Runs `sqrt` over every input. The platform passes
/// [`super::sqrt`] itself; the host test passes it too.
pub fn run(sqrt: impl Fn(f32) -> f32) -> Report {
    let mut report = Report {
        count: COUNT,
        bad: [0; 4],
        first_bad: [(0, 0); 4],
        hash: FNV_OFFSET,
    };
    let mut found = 0;
    for i in 0..COUNT {
        let x = f32::from_bits(input(i));
        let r = sqrt(x);
        if !correct(x, r) {
            let class = class(i);
            report.bad[class] += 1;
            if found < report.first_bad.len() {
                report.first_bad[found] = (x.to_bits(), r.to_bits());
                found += 1;
            }
        }
        let bits = if r.is_nan() { 0x7fc0_0000 } else { r.to_bits() };
        for b in bits.to_le_bytes() {
            report.hash = (report.hash ^ u64::from(b)).wrapping_mul(FNV_PRIME);
        }
    }
    report
}

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

fn class(i: u32) -> usize {
    let specials = SPECIALS.len() as u32;
    if i < specials {
        0
    } else if i < specials + DENORMALS {
        1
    } else if i < specials + DENORMALS + SWEEP {
        2
    } else {
        3
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The guarded `sqrt` is correct for every input and its output bits
    /// hash to [`HASH`], the value a target's self-test must report.
    #[test]
    fn sqrt_matches_the_pinned_hash() {
        let report = run(super::super::sqrt);
        assert_eq!(report.bad, [0; 4], "first wrong: {:x?}", report.first_bad);
        assert_eq!(report.hash, HASH, "hash {:#018x}", report.hash);
    }

    /// The exact bounds agree with the host's `f64` square root rounded to
    /// `f32` (correctly rounded: 53 >= 2 * 24 + 2 bits) on every input, and
    /// reject its neighbours, so the check can tell a 1-ULP error.
    #[test]
    fn the_bounds_check_is_exact() {
        for i in 0..COUNT {
            let x = f32::from_bits(input(i));
            if !(x > 0.0 && x.is_finite()) {
                continue;
            }
            let r = (f64::from(x)).sqrt() as f32;
            assert!(correct(x, r), "{:#x}", x.to_bits());
            assert!(
                !correct(x, f32::from_bits(r.to_bits() + 1)),
                "{:#x} +1",
                x.to_bits()
            );
            assert!(
                !correct(x, f32::from_bits(r.to_bits() - 1)),
                "{:#x} -1",
                x.to_bits()
            );
        }
    }

    /// The input classes hold what their names say.
    #[test]
    fn inputs_cover_each_class() {
        let specials = SPECIALS.len() as u32;
        for i in 0..COUNT {
            let x = f32::from_bits(input(i));
            match class(i) {
                1 => assert!(x > 0.0 && x.is_subnormal(), "{i}: {:#x}", x.to_bits()),
                2 => assert!(x.is_normal() && x > 0.0, "{i}: {:#x}", x.to_bits()),
                3 => assert!(x < 0.0 && x.is_finite(), "{i}: {:#x}", x.to_bits()),
                _ => assert!(i < specials),
            }
        }
    }
}
