//! Fixed input sets for the original's inverse trigonometry
//! ([`super::sy_arc_tan`], [`super::sy_arc_tan2`], [`super::sy_arc_sin`],
//! [`super::sy_arc_cos`]) and the hash of their output bits on the N64.
//!
//! The pinned hashes come from the ROM's own machine code, not from this
//! port: an R4300 interpreter (FCSR as `osInitialize` sets it: round to
//! nearest, denormal results flushed to zero, invalid operations trapping)
//! ran `syUtilsArcTan` (`0x80018514`), `syUtilsArcTan2` (`0x8001863c`),
//! `syUtilsArcSin` (`0x8001873c`) and `syUtilsArcCos` (`0x800187c0`) over
//! these inputs. The inputs are synthetic (a fixed hash of the index);
//! nothing read from the ROM is stored here but the four hashes. A platform
//! runs [`run`] on the target to prove it computes the same bits.
//!
//! `python3 -I tools/r4300/trig_hashes.py --rom PATH` regenerates the
//! hashes: it reads the routines from the ROM at run time and mirrors the
//! inputs below, so changing either side means changing both.
//!
//! Inputs avoid what traps the R4300 (NaN, denormal operands, `inf / inf`)
//! and, for the two-argument function, ratios that underflow, where the
//! N64's flush to zero and IEEE gradual underflow part ways.

/// FNV-1a of `sy_arc_tan`'s output bits over [`unary`]'s inputs.
pub const HASH_ARC_TAN: u64 = 0x38f6_75e0_75a1_5c0b;
/// The same for `sy_arc_sin`.
pub const HASH_ARC_SIN: u64 = 0xb91a_26e0_5467_effc;
/// The same for `sy_arc_cos`.
pub const HASH_ARC_COS: u64 = 0x197a_b366_d661_0cb3;
/// FNV-1a of `sy_arc_tan2`'s output bits over [`binary`]'s inputs.
pub const HASH_ARC_TAN2: u64 = 0xfd0a_8140_286a_588d;

/// Special single inputs: zeros, the `±1` and `±0.99999` boundaries and
/// their neighbours, infinities, the largest and smallest normals.
const UNARY_SPECIALS: [u32; 22] = [
    0x0000_0000, // +0
    0x8000_0000, // -0
    0x3f80_0000, // 1
    0x3f7f_ffff,
    0x3f80_0001,
    0xbf80_0000, // -1
    0xbf7f_ffff,
    0xbf80_0001,
    0x3f7f_ff58, // 0.99999
    0x3f7f_ff57,
    0x3f7f_ff59,
    0xbf7f_ff58,
    0xbf7f_ff57,
    0xbf7f_ff59,
    0x7f80_0000, // +inf
    0xff80_0000, // -inf
    0x7f7f_ffff, // largest finite
    0xff7f_ffff,
    0x0080_0000, // smallest normal
    0x8080_0000,
    0x3fc9_0fdb, // pi/2
    0x4049_0fdb, // pi
];

/// Hashed bit patterns over every finite exponent.
pub const UNARY_RANDOM: u32 = 65_536;
/// A uniform sweep of `[-8, 8)` in steps of `2^-12`.
pub const UNARY_SWEEP: u32 = 65_536;
/// Neighbours (±128 ULP) of `1`, `-1`, `0.99999` and `-0.99999`.
pub const UNARY_NEAR: u32 = 4 * 256;
/// Every unary input.
pub const UNARY_COUNT: u32 = UNARY_SPECIALS.len() as u32 + UNARY_RANDOM + UNARY_SWEEP + UNARY_NEAR;

/// Hashed `(y, x)` pairs, magnitudes in `[2^-40, 2^41)`, one in sixteen of
/// each axis zero.
pub const BINARY_RANDOM: u32 = 65_536;
/// A grid of every quarter-integer pair in `[-4, 4]`.
pub const BINARY_GRID: u32 = 33 * 33;
/// Every pair.
pub const BINARY_COUNT: u32 = BINARY_RANDOM + BINARY_GRID;

/// A fixed integer hash (Chris Wellons' `lowbias32`).
pub fn mix(mut x: u32) -> u32 {
    x ^= x >> 16;
    x = x.wrapping_mul(0x7feb_352d);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846c_a68b);
    x ^= x >> 16;
    x
}

/// Unary input `i` (`i < UNARY_COUNT`), as bits.
pub fn unary(i: u32) -> u32 {
    let specials = UNARY_SPECIALS.len() as u32;
    if i < specials {
        return UNARY_SPECIALS[i as usize];
    }
    let i = i - specials;
    if i < UNARY_RANDOM {
        let bits = mix(i);
        let exponent = (bits >> 23) & 0xff;
        return if exponent == 0 {
            bits & 0x8000_0000
        } else if exponent == 0xff {
            bits ^ 0x4000_0000
        } else {
            bits
        };
    }
    let i = i - UNARY_RANDOM;
    if i < UNARY_SWEEP {
        return ((i as f32) * (1.0 / 4096.0) - 8.0).to_bits();
    }
    let i = i - UNARY_SWEEP;
    let centre = [0x3f80_0000u32, 0xbf80_0000, 0x3f7f_ff58, 0xbf7f_ff58][(i / 256) as usize];
    centre.wrapping_add(i % 256).wrapping_sub(128)
}

/// A hashed finite value with magnitude in `[2^-40, 2^41)`, or a signed
/// zero one time in sixteen.
fn bounded(bits: u32) -> u32 {
    if bits & 0xf == 0 {
        return bits & 0x8000_0000;
    }
    let exponent = 87 + ((bits >> 23) & 0xff) % 81;
    (bits & 0x807f_ffff) | (exponent << 23)
}

/// Binary input `i` (`i < BINARY_COUNT`), as `(y, x)` bits.
pub fn binary(i: u32) -> (u32, u32) {
    if i < BINARY_RANDOM {
        return (bounded(mix(2 * i)), bounded(mix(2 * i + 1)));
    }
    let i = i - BINARY_RANDOM;
    let y = (i / 33) as f32 * 0.25 - 4.0;
    let x = (i % 33) as f32 * 0.25 - 4.0;
    (y.to_bits(), x.to_bits())
}

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

fn fnv(hash: u64, r: f32) -> u64 {
    let bits = if r.is_nan() { 0x7fc0_0000 } else { r.to_bits() };
    bits.to_le_bytes()
        .iter()
        .fold(hash, |h, &b| (h ^ u64::from(b)).wrapping_mul(FNV_PRIME))
}

/// The output hash of a one-argument function over [`unary`].
pub fn hash_unary(f: impl Fn(f32) -> f32) -> u64 {
    (0..UNARY_COUNT).fold(FNV_OFFSET, |h, i| fnv(h, f(f32::from_bits(unary(i)))))
}

/// The output hash of a two-argument `f(y, x)` over [`binary`].
pub fn hash_binary(f: impl Fn(f32, f32) -> f32) -> u64 {
    (0..BINARY_COUNT).fold(FNV_OFFSET, |h, i| {
        let (y, x) = binary(i);
        fnv(h, f(f32::from_bits(y), f32::from_bits(x)))
    })
}

/// What a platform reports: each function's hash, in the order
/// `[arc_tan, arc_tan2, arc_sin, arc_cos]`.
pub fn run() -> [u64; 4] {
    [
        hash_unary(super::sy_arc_tan),
        hash_binary(super::sy_arc_tan2),
        hash_unary(super::sy_arc_sin),
        hash_unary(super::sy_arc_cos),
    ]
}

/// The pinned hashes in [`run`]'s order.
pub const HASHES: [u64; 4] = [HASH_ARC_TAN, HASH_ARC_TAN2, HASH_ARC_SIN, HASH_ARC_COS];

#[cfg(test)]
mod tests {
    use super::*;

    /// The port returns the N64's bits on every input.
    #[test]
    fn the_arc_functions_match_the_rom() {
        let got = run();
        assert_eq!(got, HASHES, "got {got:#018x?}");
    }

    /// The inputs are what the interpreter could run: finite (or an
    /// infinity among the specials), never denormal.
    #[test]
    fn the_inputs_never_trap_the_r4300() {
        for i in 0..UNARY_COUNT {
            let x = f32::from_bits(unary(i));
            assert!(!x.is_nan() && !x.is_subnormal(), "{i}: {:#x}", x.to_bits());
        }
        for i in 0..BINARY_COUNT {
            let (y, x) = binary(i);
            let (y, x) = (f32::from_bits(y), f32::from_bits(x));
            assert!(y.is_finite() && !y.is_subnormal(), "{i}");
            assert!(x.is_finite() && !x.is_subnormal(), "{i}");
        }
    }
}
