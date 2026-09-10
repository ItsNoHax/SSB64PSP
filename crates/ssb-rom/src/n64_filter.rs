//! Host-side reference samplers for `PLAN.md` R2.0/P0a.
//!
//! RE-218 found that RE-124's filtering conclusion only ever compared the
//! N64 RDP's `G_MDSFT_TEXTFILT` *mode selector* against PSP's
//! `sceGuTexFilter` mode selector, never the two hardwares' actual pixel
//! reconstruction formulas. The RDP's `G_TF_BILERP` is a 3-point
//! (triangular) filter: it picks one of the two triangles a 2x2 texel
//! quad splits into (by which side of the diagonal the fractional
//! coordinate falls on) and linearly blends only the three texels on that
//! triangle. The PSP GE's `Linear` filter is a symmetric four-tap
//! bilinear blend of all four texels. These are different formulas that
//! happen to share the name "bilinear"/"filtered".
//!
//! [`sample_3point`] is transcribed bit-for-bit from
//! `angrylion-rdp-plus`'s `texture_pipeline_cycle`
//! (`src/core/n64video/rdp/tex.c`), a cycle-accurate, hardware-validated
//! RDP low-level emulator used as the accuracy reference throughout the
//! N64 emulation community — not from a reference port
//! (`BattleShip`/`sf64-psp`/`oot-PSP`/`n64psp`), per `AGENTS.md` §6 and
//! `DECISIONS.md` D-037, and not from Nintendo's own programming manual,
//! which describes the *behaviour* ("selects three of four texels
//! depending on where the sample point lies in the 2x2 grid") but not the
//! exact blend weights. Both samplers operate on the RDP's own native
//! 1/32-texel fixed-point coordinate precision (`G_MDSFT_TEXTFILT`'s
//! `sfrac`/`tfrac`, 5 fraction bits) so the comparison isolates the
//! reconstruction-formula difference from any UV-precision difference.
//!
//! Address clamping here is a simple edge clamp, not the real N64
//! mirror/mask/clamp tile-addressing model — that model is `PLAN.md`
//! R2.0/P0b's job. These samplers are only valid for measuring interior
//! (non-boundary) reconstruction error, which is what P0a asks for.

use crate::texture::Rgba8;

/// Splits a 1/32-texel fixed-point coordinate into its integer texel index
/// and its 5-bit fraction (`0..32`), matching the RDP's own `sss1 & 0x1f`.
fn split_q5(coord: i32) -> (i32, i32) {
    (coord.div_euclid(32), coord.rem_euclid(32))
}

/// Fetches a texel with simple edge-clamp addressing (not the real N64
/// tile-addressing model; see the module docs).
fn fetch(img: &Rgba8, x: i32, y: i32) -> [i32; 4] {
    let cx = x.clamp(0, img.width as i32 - 1) as u32;
    let cy = y.clamp(0, img.height as i32 - 1) as u32;
    let idx = (cy * img.width + cx) as usize;
    let p = img.get(idx);
    [p[0] as i32, p[1] as i32, p[2] as i32, p[3] as i32]
}

/// N64 RDP `G_TF_BILERP` 3-point (triangular) reconstruction.
///
/// `s_q5`/`t_q5` are texel-space coordinates in 1/32-texel fixed point
/// (the RDP's own `TC` precision).
pub fn sample_3point(img: &Rgba8, s_q5: i32, t_q5: i32) -> [u8; 4] {
    let (s0, sfrac) = split_q5(s_q5);
    let (t0, tfrac) = split_q5(t_q5);

    // t0/t1/t2/t3 in `angrylion-rdp-plus`'s own naming (`tex.c`,
    // `fetch_texel_quadro`): t0 = (s0,t0), t1 = (s0+1,t0), t2 =
    // (s0,t0+1), t3 = (s0+1,t0+1).
    let c00 = fetch(img, s0, t0);
    let c10 = fetch(img, s0 + 1, t0);
    let c01 = fetch(img, s0, t0 + 1);
    let c11 = fetch(img, s0 + 1, t0 + 1);

    // `upper = (sfrac + tfrac) & 0x20` in the real hardware: the lower/
    // right triangle (anchored at the diagonal texel c11) is picked once
    // the fractional coordinate crosses the diagonal.
    let upper = sfrac + tfrac >= 32;

    let mut out = [0u8; 4];
    for c in 0..4 {
        let v = if !upper {
            c00[c] + ((sfrac * (c10[c] - c00[c]) + tfrac * (c01[c] - c00[c]) + 0x10) >> 5)
        } else {
            let invs = 32 - sfrac;
            let invt = 32 - tfrac;
            c11[c] + ((invs * (c01[c] - c11[c]) + invt * (c10[c] - c11[c]) + 0x10) >> 5)
        };
        out[c] = v.clamp(0, 255) as u8;
    }
    out
}

/// PSP GE `Linear` filter: a symmetric four-tap bilinear blend of all four
/// texels, at the same 1/32-texel fixed-point precision as
/// [`sample_3point`] so the two can be compared directly.
pub fn sample_bilinear(img: &Rgba8, s_q5: i32, t_q5: i32) -> [u8; 4] {
    let (s0, sfrac) = split_q5(s_q5);
    let (t0, tfrac) = split_q5(t_q5);

    let c00 = fetch(img, s0, t0);
    let c10 = fetch(img, s0 + 1, t0);
    let c01 = fetch(img, s0, t0 + 1);
    let c11 = fetch(img, s0 + 1, t0 + 1);

    let invs = 32 - sfrac;
    let invt = 32 - tfrac;

    let mut out = [0u8; 4];
    for c in 0..4 {
        let top = c00[c] * invs + c10[c] * sfrac;
        let bot = c01[c] * invs + c11[c] * sfrac;
        let v = (top * invt + bot * tfrac + 512) >> 10; // /(32*32)
        out[c] = v.clamp(0, 255) as u8;
    }
    out
}

/// Max per-channel absolute difference between two RGBA8 texels.
pub fn max_abs_diff(a: [u8; 4], b: [u8; 4]) -> u8 {
    a.iter()
        .zip(b.iter())
        .map(|(&x, &y)| x.abs_diff(y))
        .max()
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flat(color: [u8; 4]) -> Rgba8 {
        let mut img = Rgba8::new(2, 2);
        for i in 0..4 {
            img.put(i, color);
        }
        img
    }

    #[test]
    fn flat_texture_is_identical_under_both_filters_at_every_fraction() {
        let img = flat([12, 34, 56, 78]);
        for s in 0..32 {
            for t in 0..32 {
                assert_eq!(sample_3point(&img, s, t), [12, 34, 56, 78]);
                assert_eq!(sample_bilinear(&img, s, t), [12, 34, 56, 78]);
            }
        }
    }

    #[test]
    fn pure_one_axis_gradient_agrees_between_filters() {
        // Varying only along s: both formulas degrade to the same 1D
        // linear interpolation regardless of which triangle is picked.
        let mut img = Rgba8::new(2, 2);
        img.put(0, [0, 0, 0, 255]); // (0,0)
        img.put(1, [255, 255, 255, 255]); // (1,0)
        img.put(2, [0, 0, 0, 255]); // (0,1)
        img.put(3, [255, 255, 255, 255]); // (1,1)

        for s in 0..32 {
            for t in 0..32 {
                assert_eq!(
                    sample_3point(&img, s, t),
                    sample_bilinear(&img, s, t),
                    "s={s} t={t}"
                );
            }
        }
    }

    #[test]
    fn diagonal_saddle_pattern_diverges_sharply_at_the_center() {
        // The textbook N64 "triangulation bias" case: opposite corners
        // share a color, the other two share the opposite color. At the
        // exact center the 3-point filter reads only the picked
        // triangle's flat color, while symmetric bilinear averages all
        // four texels down to grey.
        let mut img = Rgba8::new(2, 2);
        img.put(0, [0, 0, 0, 255]); // (0,0) = c00, dark
        img.put(1, [255, 255, 255, 255]); // (1,0) = c10, bright
        img.put(2, [255, 255, 255, 255]); // (0,1) = c01, bright
        img.put(3, [0, 0, 0, 255]); // (1,1) = c11, dark

        let three_point = sample_3point(&img, 16, 16);
        let bilinear = sample_bilinear(&img, 16, 16);

        assert_eq!(three_point, [255, 255, 255, 255]);
        assert_eq!(bilinear, [128, 128, 128, 255]);
        assert!(max_abs_diff(three_point, bilinear) >= 127);
    }

    #[test]
    fn triangle_boundary_switches_exactly_at_sfrac_plus_tfrac_32() {
        let mut img = Rgba8::new(2, 2);
        img.put(0, [0, 0, 0, 255]);
        img.put(1, [255, 255, 255, 255]);
        img.put(2, [255, 255, 255, 255]);
        img.put(3, [0, 0, 0, 255]);

        // sfrac=15,tfrac=16 -> sum=31, lower triangle (t0-anchored).
        let below = sample_3point(&img, 15, 16);
        // sfrac=16,tfrac=16 -> sum=32, upper triangle (t3-anchored).
        let at = sample_3point(&img, 16, 16);
        assert_ne!(below, at);
    }

    #[test]
    fn edge_clamp_does_not_panic_at_texture_boundary() {
        let img = flat([1, 2, 3, 4]);
        assert_eq!(sample_3point(&img, 63, 63), [1, 2, 3, 4]);
        assert_eq!(sample_bilinear(&img, -5, -5), [1, 2, 3, 4]);
    }
}
