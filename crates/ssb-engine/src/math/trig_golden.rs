//! Fixed input sets for the original's trigonometry ([`super::sy_arc_tan`],
//! [`super::sy_arc_tan2`], [`super::sy_arc_sin`], [`super::sy_arc_cos`],
//! [`super::sinf`], [`super::cosf`], [`super::sy_tan`]) and the hash of
//! their output bits on the N64.
//!
//! The pinned hashes come from the ROM's own machine code, not from this
//! port: an R4300 interpreter (FCSR as `osInitialize` sets it: round to
//! nearest, denormal results flushed to zero, invalid operations trapping)
//! ran `syUtilsArcTan` (`0x80018514`), `syUtilsArcTan2` (`0x8001863c`),
//! `syUtilsArcSin` (`0x8001873c`), `syUtilsArcCos` (`0x800187c0`),
//! `__sinf` (`0x800303f0`), `__cosf` (`0x80035cd0`) and `syUtilsTan`
//! (`0x800184e0`) over these inputs. The inputs are synthetic (a fixed hash
//! of the index); nothing read from the ROM is stored here but the hashes.
//! A platform runs [`run`] on the target to prove it computes the same bits.
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
/// The same for `sinf` over [`unary`].
pub const HASH_SINF: u64 = 0x0ae6_4c80_ba5c_8ee7;
/// The same for `cosf` over [`unary`].
pub const HASH_COSF: u64 = 0x037f_84ec_89ad_d89b;
/// FNV-1a of `sinf`'s output bits over [`angle`]'s inputs.
pub const HASH_SINF_ANGLE: u64 = 0x126e_1fdb_4372_c505;
/// The same for `cosf`.
pub const HASH_COSF_ANGLE: u64 = 0x4b4c_37df_1393_b4c6;
/// The same for `sy_tan`, over the angles below `2^28` (above it both
/// `sinf` and `cosf` give 0, and the R4300 traps the `0 / 0`).
pub const HASH_TAN_ANGLE: u64 = 0x17dd_b219_f595_dccc;

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

/// Special angles: `fsin`'s and `fcos`'s branch boundaries (`2^-12`, `1.5`,
/// `2^28`) and their neighbours, both signs.
const ANGLE_SPECIALS: [u32; 18] = [
    0x3980_0000, // 2^-12
    0x397f_ffff,
    0x3980_0001,
    0x3fc0_0000, // 1.5
    0x3fbf_ffff,
    0x3fc0_0001,
    0x4d80_0000, // 2^28
    0x4d7f_ffff,
    0x4d80_0001,
    0xb980_0000,
    0xb97f_ffff,
    0xb980_0001,
    0xbfc0_0000,
    0xbfbf_ffff,
    0xbfc0_0001,
    0xcd80_0000,
    0xcd7f_ffff,
    0xcd80_0001,
];

/// Hashed angles with magnitudes in `[2^-15, 2^30)`, both signs: every path
/// of `fsin` and `fcos` but the NaN and infinity ones.
pub const ANGLE_RANDOM: u32 = 65_536;
/// Neighbours (±16 ULP) of the `f32` nearest `kπ/2`, `k` in `-64..=64`,
/// where the reduction cancels most.
pub const ANGLE_NEAR: u32 = 129 * 33;
/// Every angle.
pub const ANGLE_COUNT: u32 = ANGLE_SPECIALS.len() as u32 + ANGLE_RANDOM + ANGLE_NEAR;

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

/// Angle input `i` (`i < ANGLE_COUNT`), as bits.
pub fn angle(i: u32) -> u32 {
    let specials = ANGLE_SPECIALS.len() as u32;
    if i < specials {
        return ANGLE_SPECIALS[i as usize];
    }
    let i = i - specials;
    if i < ANGLE_RANDOM {
        let bits = mix(i ^ 0x5eed_0000);
        let exponent = 112 + ((bits >> 23) & 0xff) % 45;
        return (bits & 0x807f_ffff) | (exponent << 23);
    }
    let i = i - ANGLE_RANDOM;
    let k = (i / 33) as i32 - 64;
    let centre = (k as f32 * core::f32::consts::FRAC_PI_2).to_bits();
    // `k = 0` walks the neighbours of 1 instead (zero's are denormal).
    let step = (i % 33) as i32 - 16;
    if k == 0 {
        return 0x3f80_0000u32.wrapping_add_signed(step);
    }
    centre.wrapping_add_signed(step)
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

/// The output hash of a one-argument function over [`angle`], skipping
/// the angles `keep` rejects.
pub fn hash_angle(f: impl Fn(f32) -> f32, keep: impl Fn(f32) -> bool) -> u64 {
    (0..ANGLE_COUNT).fold(FNV_OFFSET, |h, i| {
        let x = f32::from_bits(angle(i));
        if keep(x) {
            fnv(h, f(x))
        } else {
            h
        }
    })
}

/// [`hash_angle`]'s filter for `sy_tan`: `|x| < 2^28`.
pub fn tan_keeps(x: f32) -> bool {
    x.to_bits() & 0x7fff_ffff < 0x4d80_0000
}

/// The output hash of a two-argument `f(y, x)` over [`binary`].
pub fn hash_binary(f: impl Fn(f32, f32) -> f32) -> u64 {
    (0..BINARY_COUNT).fold(FNV_OFFSET, |h, i| {
        let (y, x) = binary(i);
        fnv(h, f(f32::from_bits(y), f32::from_bits(x)))
    })
}

/// The names of [`run`]'s results, in order.
pub const NAMES: [&str; 9] = [
    "arc_tan",
    "arc_tan2",
    "arc_sin",
    "arc_cos",
    "sinf",
    "cosf",
    "sinf_angle",
    "cosf_angle",
    "tan_angle",
];

/// What a platform reports: each function's hash, in [`NAMES`]' order.
pub fn run() -> [u64; 9] {
    let all = |_: f32| true;
    [
        hash_unary(super::sy_arc_tan),
        hash_binary(super::sy_arc_tan2),
        hash_unary(super::sy_arc_sin),
        hash_unary(super::sy_arc_cos),
        hash_unary(super::sinf),
        hash_unary(super::cosf),
        hash_angle(super::sinf, all),
        hash_angle(super::cosf, all),
        hash_angle(super::sy_tan, tan_keeps),
    ]
}

/// The pinned hashes in [`run`]'s order.
pub const HASHES: [u64; 9] = [
    HASH_ARC_TAN,
    HASH_ARC_TAN2,
    HASH_ARC_SIN,
    HASH_ARC_COS,
    HASH_SINF,
    HASH_COSF,
    HASH_SINF_ANGLE,
    HASH_COSF_ANGLE,
    HASH_TAN_ANGLE,
];

/// FNV-1a of `y / x`, `y * x` and `y + x` in `f32` over [`binary`]'s
/// pairs: IEEE single precision, round to nearest even (the host's). The
/// inverse functions are chains of these; a platform that matches them is
/// IEEE for every pair here (zeros included: `y / 0` is an infinity,
/// `0 / 0` a NaN). The N64 would differ only where a result is denormal,
/// which no pair here produces.
pub const HASHES_ARITH: [u64; 3] = [
    0xb968_dc04_9cb7_fa53,
    0x1f67_ae85_9cd6_2fb1,
    0x1014_cac4_e77d_a8f6,
];

/// What a platform's `div.s`, `mul.s` and `add.s` give over [`binary`], in
/// [`HASHES_ARITH`]' order.
pub fn run_arith() -> [u64; 3] {
    let op = |f: fn(f32, f32) -> f32| {
        hash_binary(|y, x| f(core::hint::black_box(y), core::hint::black_box(x)))
    };
    [op(|y, x| y / x), op(|y, x| y * x), op(|y, x| y + x)]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The port returns the N64's bits on every input.
    #[test]
    fn the_trig_functions_match_the_rom() {
        let got = run();
        assert_eq!(got, HASHES, "got {got:#018x?}");
    }

    /// The host's `f32` arithmetic is IEEE, so its hashes are the
    /// reference a platform's self-test compares with.
    #[test]
    fn the_arithmetic_hashes_are_the_hosts() {
        let got = run_arith();
        assert_eq!(got, HASHES_ARITH, "got {got:#018x?}");
    }

    /// No pair gives a denormal, where the N64 (flush to zero) and IEEE
    /// part ways.
    #[test]
    fn no_arithmetic_result_is_denormal() {
        for i in 0..BINARY_COUNT {
            let (y, x) = binary(i);
            let (y, x) = (f32::from_bits(y), f32::from_bits(x));
            for r in [y / x, y * x, y + x] {
                assert!(!r.is_subnormal(), "{i}: {y:e} {x:e}");
            }
        }
    }

    /// The inputs are what the interpreter could run: finite (or an
    /// infinity among the specials), never denormal.
    #[test]
    fn the_inputs_never_trap_the_r4300() {
        for i in 0..UNARY_COUNT {
            let x = f32::from_bits(unary(i));
            assert!(!x.is_nan() && !x.is_subnormal(), "{i}: {:#x}", x.to_bits());
        }
        for i in 0..ANGLE_COUNT {
            let x = f32::from_bits(angle(i));
            assert!(x.is_finite() && x.is_normal(), "{i}: {:#x}", x.to_bits());
        }
        for i in 0..BINARY_COUNT {
            let (y, x) = binary(i);
            let (y, x) = (f32::from_bits(y), f32::from_bits(x));
            assert!(y.is_finite() && !y.is_subnormal(), "{i}");
            assert!(x.is_finite() && !x.is_subnormal(), "{i}");
        }
    }
}
