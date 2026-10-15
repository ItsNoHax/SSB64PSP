//! The RSP's per-vertex directional lighting, bit-exact (RE-514).
//!
//! SSB64 lights fighters, items and animated-light stage materials with one
//! directional light plus ambient (`ftDisplayLightsDrawReflect`,
//! `gSPNumLights(1)`), evaluated by its F3DEX2 2.04H microcode at `G_VTX`
//! time. The PSP's GE lights in float and clamps only after the material
//! multiply, so the port computes the shade on the CPU with the microcode's
//! own integer arithmetic instead:
//!
//! 1. [`reflect_light_dir`]: the world direction, `vec * 100` stored in the
//!    `Light`'s signed bytes.
//! 2. [`transform_light_dir`]: `continue_light_dir_xfrm` -- the modelview's
//!    3x3 times that direction, normalised with the RSP's `VRSQ` table and
//!    stored back as signed bytes. The normalisation drops both the 100/127
//!    length and any uniform model scale.
//! 3. [`dot16`] and [`shade`]: `lights_dircoloraccum2`/`lights_finishone`
//!    with `vmulf`/`vmacf` saturation; the shade clamps at 255 *before* the
//!    colour combiner.
//! 4. [`prim_times_shade`]: the RDP combiner's `(a - b) * c + d` rounding.
//!
//! Every function was checked against SSB64's own microcode running on an
//! LLE RSP (cxd4) over 768 vertices of random matrices, directions, colours
//! and normals: all colour and alpha bytes identical.

/// `ftDisplayLightsDrawReflect`'s light direction from the stage's X/Y
/// angles (degrees), as the `Light.l.dir` signed bytes it stores.
///
/// `lbCommonSin`/`lbCommonCos` truncate their table index, so `-sin(-a)` and
/// `cos(-a)` are not `sin(a)` and `cos(a)`. `F_CLC_DTOR32` multiplies by pi
/// before dividing by 180. The float-to-`s8` store truncates toward zero.
pub fn reflect_light_dir(angles_degrees: [f32; 2]) -> [i8; 3] {
    use crate::math::{lb_cos, lb_sin};
    let pi = core::f32::consts::PI;
    let angle_y = angles_degrees[1] * pi / 180.0;
    let angle_x = angles_degrees[0] * pi / 180.0;
    let y = -lb_sin(-angle_y);
    let z = lb_cos(-angle_y);
    let x = lb_sin(angle_x) * z;
    let z = z * lb_cos(angle_x);
    [(x * 100.0) as i8, (y * 100.0) as i8, (z * 100.0) as i8]
}

/// `guMtxF2L`'s s15.16 fixed point for a 3x3 block: `(long)(f * 65536)`.
/// `rows[i]` is the image of object axis `i` (the N64 `Mtx` row and the
/// PSP's `ScePspFMatrix4` `x`/`y`/`z` vector alike).
pub fn fixed_rows(rows: [[f32; 3]; 3]) -> [[i32; 3]; 3] {
    rows.map(|r| r.map(|v| (v * 65536.0) as i32))
}

/// The RSP's `VRSQ` table (ares `RSP::power`): entry `i` holds the largest
/// `b` with `a * b^2 < 2^44`, halved, where `a` is `512 + i` (shifted
/// right once for odd `i`).
const RSQ: [u16; 512] = {
    let mut table = [0u16; 512];
    let mut index = 0;
    while index < 512 {
        let a: u64 = (index as u64 + 512) >> (index % 2);
        // ares counts b up while a * (b + 1)^2 < 2^44, so b is the largest
        // k with a * k^2 < 2^44.
        let (mut lo, mut hi) = (0u64, 1u64 << 23);
        while hi - lo > 1 {
            let mid = (lo + hi) / 2;
            if a * mid * mid < (1u64 << 44) {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        table[index] = (lo >> 1) as u16;
        index += 1;
    }
    table
};

/// `VRSQL` after `VRSQH`: the 32-bit reciprocal square root of `input`.
fn vrsq(input: i32) -> u32 {
    let mask = input >> 31;
    let mut data = input ^ mask;
    if input > -32768 {
        data = data.wrapping_sub(mask);
    }
    if data == 0 {
        return 0x7fff_ffff;
    }
    if input == -32768 {
        return 0xffff_0000;
    }
    let data = data as u32;
    let shift = data.leading_zeros();
    let index = ((u64::from(data) << shift) & 0x7fc0_0000) >> 22;
    let r = i32::from(RSQ[(index as usize & 0x1fe) | (shift as usize & 1)]);
    let r = (0x10000 | r) << 14;
    ((r >> ((31 - shift) >> 1)) ^ mask) as u32
}

/// Wraps to the vector unit's 48-bit accumulator.
fn acc48(a: i64) -> i64 {
    (a << 16) >> 16
}

/// `accumulatorSaturate`: the middle (`upper`) or low slice when the
/// accumulator fits in 32 signed bits, else the clamp value.
fn saturate(acc: i64, upper: bool, negative: u16, positive: u16) -> u16 {
    if acc < -(1i64 << 31) {
        negative
    } else if acc >= 1i64 << 31 {
        positive
    } else if upper {
        (acc >> 16) as u16
    } else {
        acc as u16
    }
}

/// F3DEX2 2.04H `continue_light_dir_xfrm`: the world light direction in the
/// object space of the modelview `rows` (s15.16, [`fixed_rows`]), normalised
/// and stored back as signed bytes. Component `k` is `rows[k] . dir`.
pub fn transform_light_dir(rows: [[i32; 3]; 3], dir: [i8; 3]) -> [i8; 3] {
    // vmudn/vmadh over y, x, z: X_k = sum(m[k][j] * (dir_j << 8)) >> 16.
    let mut lo = [0i64; 3];
    let mut hi = [0i64; 3];
    for k in 0..3 {
        let mut acc = 0i64;
        for j in [1, 0, 2] {
            let v = i64::from(dir[j]) << 8;
            let raw = rows[k][j] as u32;
            let int = i64::from((raw >> 16) as u16 as i16);
            let frac = i64::from(raw & 0xFFFF);
            acc = acc48(acc + frac * v);
            acc = acc48(acc + ((int * v) << 16));
        }
        lo[k] = (acc >> 16) & 0xFFFF;
        hi[k] = i64::from((acc >> 32) as u16 as i16);
    }
    // Squares: vmudl/vmadm/vmadn (low slice) then vmadh (middle slice).
    let mut sq_lo = [0i64; 3];
    let mut sq_hi = [0i64; 3];
    for k in 0..3 {
        let mut acc = (lo[k] * lo[k]) >> 16;
        acc = acc48(acc + hi[k] * lo[k]);
        acc = acc48(acc + lo[k] * hi[k]);
        sq_lo[k] = i64::from(saturate(acc, false, 0, 0xFFFF));
        acc = acc48(acc + ((hi[k] * hi[k]) << 16));
        sq_hi[k] = i64::from(saturate(acc, true, 0x8000, 0x7FFF) as i16);
    }
    // vaddc/vadd: the 32-bit sum of squares, carry from the low halves.
    let clamp16 = |v: i64| v.clamp(-32768, 32767);
    let sum = sq_lo[0] + sq_lo[1];
    let high = clamp16(sq_hi[0] + sq_hi[1] + (sum >> 16));
    let sum2 = (sum & 0xFFFF) + sq_lo[2];
    let high = clamp16(high + sq_hi[2] + (sum2 >> 16));
    let r = vrsq(((high as i32) << 16) | (sum2 & 0xFFFF) as i32);
    let r_lo = i64::from(r & 0xFFFF);
    let r_hi = i64::from((r >> 16) as u16 as i16);
    // Normalise (vmudl/vmadm/vmadn/vmadh), scale by 0x100, `spv` the bytes.
    let mut out = [0i8; 3];
    for k in 0..3 {
        let mut acc = (lo[k] * r_lo) >> 16;
        acc = acc48(acc + hi[k] * r_lo);
        acc = acc48(acc + lo[k] * r_hi);
        let low = i64::from(saturate(acc, false, 0, 0xFFFF));
        acc = acc48(acc + ((hi[k] * r_hi) << 16));
        let high = i64::from(saturate(acc, true, 0x8000, 0x7FFF) as i16);
        let mut acc = acc48(low * 0x100);
        acc = acc48(acc + ((high * 0x100) << 16));
        let e = saturate(acc, true, 0x8000, 0x7FFF) as i16;
        out[k] = (e >> 8) as i8;
    }
    out
}

/// [`transform_light_dir`] for a float 3x3 (`rows[i]` = image of axis `i`).
pub fn object_light_dir(rows: [[f32; 3]; 3], dir: [i8; 3]) -> [i8; 3] {
    transform_light_dir(fixed_rows(rows), dir)
}

/// `lights_dircoloraccum2`'s clamped dot product of a vertex's signed-byte
/// normal and the object-space light: `2 * (n . d)`, 0 when facing away,
/// saturated at `0x7FFF`.
pub fn dot16(normal: [i8; 3], dir: [i8; 3]) -> u16 {
    let s: i32 = (0..3)
        .map(|i| i32::from(normal[i]) * i32::from(dir[i]))
        .sum();
    if s <= 0 {
        0
    } else {
        (2 * s).min(0x7FFF) as u16
    }
}

/// One shade channel: ambient (`LIGHT_2`) plus light colour (`LIGHT_1`)
/// times [`dot16`], through `vmulf`/`vmacf` and the `suv` store. Saturates
/// at 255.
pub fn shade(ambient: u8, color: u8, dot: u16) -> u8 {
    // `(a << 7) * 0x7FFF * 2 + 0x8000 + (c << 7) * dot * 2`, `>> 16`: every
    // term is a multiple of 256, so it stays in 32 bits divided by 256.
    let acc = u32::from(ambient) * 32_767 + u32::from(color) * u32::from(dot) + 128;
    ((acc >> 8).min(0x7FFF) >> 7) as u8
}

/// A lit vertex's shade alpha: its own fourth byte through the ambient
/// path's `vmulf` (255 becomes 254).
pub fn shade_alpha(alpha: u8) -> u8 {
    shade(alpha, 0, 0)
}

/// The RDP colour combiner's `PRIM * SHADE`: `(p * s + 0x80) >> 8`.
pub fn prim_times_shade(prim: u8, shade: u8) -> u8 {
    ((u32::from(prim) * u32::from(shade) + 0x80) >> 8) as u8
}

/// A lit vertex's RGB: [`shade`] per channel from `light` (`LIGHT_1`) and
/// `ambient` (`LIGHT_2`), scaled by the combiner's `PRIM` when it has one.
pub fn lit_rgb(dot: u16, light: [u8; 3], ambient: [u8; 3], prim: Option<[u8; 3]>) -> [u8; 3] {
    let s = [0, 1, 2].map(|i| shade(ambient[i], light[i], dot));
    match prim {
        Some(p) => [0, 1, 2].map(|i| prim_times_shade(p[i], s[i])),
        None => s,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Every expected byte below is SSB64's F3DEX2 2.04H output on the cxd4
    // LLE RSP for these synthetic inputs (RE-514).

    #[test]
    fn identity_modelview_normalises_the_100_length_direction() {
        let rows = fixed_rows([[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]);
        assert_eq!(transform_light_dir(rows, [0, 0, 100]), [0, 0, 127]);
        assert_eq!(transform_light_dir(rows, [47, -10, 87]), [60, -13, 112]);
    }

    #[test]
    fn rotated_and_scaled_modelviews_match_the_rsp() {
        let half = [[6860, 0, 32041], [91, 32767, -19], [-32041, 93, 6860]];
        assert_eq!(transform_light_dir(half, [4, -85, -51]), [-64, -110, -20]);
        let big = [
            [124258, 0, 41708],
            [32096, -83700, -95623],
            [26634, 100866, -79349],
        ];
        assert_eq!(transform_light_dir(big, [-6, 29, -95]), [-47, 63, 101]);
        let mixed = [
            [-15244, 0, -130182],
            [-88220, 96386, 10330],
            [95732, 88823, -11210],
        ];
        assert_eq!(transform_light_dir(mixed, [-28, 81, -50]), [68, 96, 49]);
    }

    #[test]
    fn shade_saturates_before_the_combiner() {
        // n = d = (0,0,127): 2 * 16129 = 32258.
        assert_eq!(dot16([0, 0, 127], [0, 0, 127]), 32258);
        assert_eq!(shade(128, 255, 32258), 255);
        assert_eq!(shade(128, 255, 0), 128);
        assert_eq!(dot16([0, 0, -127], [0, 0, 127]), 0);
        // Kirby's flat highlight: shade clamps to 255, so PRIM wins.
        assert_eq!(
            lit_rgb(32258, [255; 3], [128; 3], Some([255, 164, 184])),
            [254, 163, 183]
        );
    }

    #[test]
    fn unsaturated_vertices_match_the_rsp() {
        let d = [60, -13, 112];
        let (amb, col) = ([176, 209, 179], [182, 178, 140]);
        let lit = |n: [i8; 3]| lit_rgb(dot16(n, d), col, amb, None);
        assert_eq!(lit([126, 5, 8]), [255, 255, 250]);
        assert_eq!(lit([77, 100, -5]), [206, 238, 202]);
        assert_eq!(lit([-74, 63, 18]), [175, 208, 178]);
        let d = [-47, 63, 101];
        let (amb, col) = ([71, 57, 23], [75, 7, 249]);
        let lit = |n: [i8; 3]| lit_rgb(dot16(n, d), col, amb, None);
        assert_eq!(lit([75, -100, -16]), [71, 57, 23]);
        assert_eq!(lit([37, -24, 39]), [74, 57, 33]);
        assert_eq!(lit([41, 91, -1]), [87, 58, 79]);
        assert_eq!(lit([-67, 68, -27]), [92, 59, 94]);
        let d = [68, 96, 49];
        let (amb, col) = ([160, 64, 71], [87, 6, 168]);
        let lit = |n: [i8; 3]| lit_rgb(dot16(n, d), col, amb, None);
        assert_eq!(lit([75, 100, -16]), [233, 69, 213]);
        assert_eq!(lit([79, 72, -53]), [211, 67, 170]);
        assert_eq!(lit([120, 4, -40]), [194, 66, 138]);
    }

    #[test]
    fn shade_alpha_follows_the_ambient_path() {
        assert_eq!(shade_alpha(255), 254);
        assert_eq!(shade_alpha(186), 185);
        assert_eq!(shade_alpha(64), 64);
        assert_eq!(shade_alpha(33), 33);
        assert_eq!(shade_alpha(3), 3);
    }

    #[test]
    fn combiner_rounds_prim_times_shade() {
        assert_eq!(prim_times_shade(255, 255), 254);
        assert_eq!(prim_times_shade(164, 255), 163);
        assert_eq!(prim_times_shade(0, 255), 0);
        assert_eq!(prim_times_shade(128, 128), 64);
    }
}
