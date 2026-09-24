//! The RDP two-tile fractional image blend driven by `SetLFrac` and
//! `TextureIDNext` (RE-321).
//!
//! `gcDrawMObjForDObj` writes an `MObj`'s `lfrac` into the `PRIM_LOD_FRAC`
//! byte of `gDPSetPrimColor` (`objdisplay.c:1218-1245`) and stages
//! `sprites[texture_id_next]` into TMEM for tile 1. A two-cycle combiner whose
//! first cycle is `(TEXEL1 - TEXEL0) * PRIM_LOD_FRAC + TEXEL0` in both RGB and
//! alpha then crossfades the tile-0 and tile-1 images by that byte.
//!
//! The GE samples one texture per draw, so the renderer draws such a
//! primitive twice over identical geometry. Pass 1 is the ordinary draw of
//! `TEXEL0`; pass 2 draws `TEXEL1` over it with the fixed blend factors
//! [`ge_fix_weights`] returns:
//!
//! ```text
//! RDP:  C = T0 + (T1 - T0) * f / 256            (then * SHADE in cycle 2)
//! GE:   C = T1*S * fa/255 + (T0*S) * fb/255,     fa = round(f * 255/256), fb = 255 - fa
//! ```
//!
//! Cycle 2 multiplies by `SHADE`, which distributes over the lerp, so each
//! pass modulates its own texel by the vertex colour. The decomposition is
//! exact only when pass 1's pixel replaces the framebuffer: the packer
//! declines the path for a material the GE would blend (RE-321).
//!
//! This module holds the arithmetic both the packer and the renderer use, and
//! a host reference of the RDP equation the decomposition is tested against.

/// The `PRIM_LOD_FRAC` byte `gcDrawMObjForDObj` emits for an `MObj` without
/// `MOBJ_FLAG_FRAC`: `gDPSetPrimColor(prim_m, mobj->lfrac * 255.0F, ...)`.
///
/// The product is single precision, and `gbi.h`'s `_SHIFTL` converts it to
/// `u32` (truncating) and keeps the low eight bits. `lfrac` starts at
/// `prim_l / 255.0F` (`objman.c:1321`) and is replaced by the `SetLFrac`
/// track once a script drives it.
pub fn prim_lod_frac(lfrac: f32) -> u8 {
    ((lfrac * 255.0) as u32 & 0xFF) as u8
}

/// One colour-combiner channel of `(A - B) * C + D`, as the RDP computes it:
/// `((A - B) * C + (D << 8) + 0x80) >> 8`.
///
/// With every input in `0..=255` and `C` a non-negative `PRIM_LOD_FRAC`, the
/// result cannot leave `0..=255`, so the 9-bit sign-extension and clamp
/// stages of the real unit never engage here.
pub fn rdp_combine_channel(a: u8, b: u8, c: u8, d: u8) -> u8 {
    let v = ((i32::from(a) - i32::from(b)) * i32::from(c) + (i32::from(d) << 8) + 0x80) >> 8;
    v.clamp(0, 255) as u8
}

/// Host reference of the classified first cycle,
/// `(TEXEL1 - TEXEL0) * PRIM_LOD_FRAC + TEXEL0`, on all four channels.
pub fn rdp_lod_lerp(t0: [u8; 4], t1: [u8; 4], frac: u8) -> [u8; 4] {
    core::array::from_fn(|i| rdp_combine_channel(t1[i], t0[i], frac, t0[i]))
}

/// `sceGuBlendFunc(Add, Fix, Fix, fa, fb)` weights for pass 2, as a
/// per-channel `(fa, fb)` pair.
///
/// The RDP weight is `f / 256`; the GE's fixed factors are read as `/ 255`.
/// `fa + fb == 255` keeps the two weights summing to one, so a region where
/// both textures agree is left unchanged.
pub fn ge_fix_weights(frac: u8) -> (u8, u8) {
    let fa = ((u32::from(frac) * 255 + 128) / 256) as u8;
    (fa, 255 - fa)
}

/// The same weights as packed `0x00BBGGRR` colours for `sceGuBlendFunc`.
pub fn ge_fix_colors(frac: u8) -> (u32, u32) {
    let (fa, fb) = ge_fix_weights(frac);
    let splat = |w: u8| u32::from(w) * 0x0001_0101;
    (splat(fa), splat(fb))
}

/// Model of the GE's two-pass result on one colour channel, for host tests:
/// both passes modulate their texel by the vertex colour `shade`, then pass 2
/// is blended over pass 1 with [`ge_fix_weights`]. Pass 2 is skipped at
/// `frac == 0`, as the renderer does.
pub fn ge_two_pass_channel(t0: u8, t1: u8, shade: u8, frac: u8) -> u8 {
    let modulate = |t: u8| (u32::from(t) * u32::from(shade) / 255) as u8;
    let pass1 = modulate(t0);
    if frac == 0 {
        return pass1;
    }
    let (fa, fb) = ge_fix_weights(frac);
    let pass2 = modulate(t1);
    ((u32::from(pass2) * u32::from(fa) + u32::from(pass1) * u32::from(fb)) / 255).min(255) as u8
}

/// The N64 reference for the same channel: the lerp, then cycle 2's
/// `COMBINED * SHADE` (with the port's standing convention that a shade of
/// 255 is the identity, matching the GE's `Modulate`).
pub fn rdp_two_cycle_channel(t0: u8, t1: u8, shade: u8, frac: u8) -> u8 {
    let lerp = rdp_combine_channel(t1, t0, frac, t0);
    (u32::from(lerp) * u32::from(shade) / 255) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prim_lod_frac_truncates_the_single_precision_product() {
        // Dream Land's water sways between 0.45 and 0.6 (RE-321).
        assert_eq!(prim_lod_frac(0.45), 114);
        assert_eq!(prim_lod_frac(0.6), 153);
        // `lfrac` initialises to `prim_l / 255.0F`; the round trip through
        // single precision must give the byte back where C does.
        for prim_l in 0..=255u8 {
            let lfrac = f32::from(prim_l) / 255.0;
            let product = lfrac * 255.0;
            assert_eq!(
                prim_lod_frac(lfrac),
                product as u32 as u8,
                "prim_l {prim_l}"
            );
        }
        assert_eq!(prim_lod_frac(0.0), 0);
        assert_eq!(prim_lod_frac(1.0), 255);
    }

    #[test]
    fn frac_zero_selects_the_current_texture() {
        for t0 in (0..=255u8).step_by(5) {
            for t1 in (0..=255u8).step_by(7) {
                assert_eq!(rdp_lod_lerp([t0; 4], [t1; 4], 0), [t0; 4]);
                assert_eq!(ge_two_pass_channel(t0, t1, 255, 0), t0);
            }
        }
    }

    #[test]
    fn frac_max_selects_the_next_texture_within_one_step() {
        // 255/256 is the largest weight PRIM_LOD_FRAC can express.
        for t0 in 0..=255u8 {
            for t1 in (0..=255u8).step_by(3) {
                let rdp = rdp_lod_lerp([t0; 4], [t1; 4], 255)[0];
                assert!(rdp.abs_diff(t1) <= 1, "rdp {rdp} t1 {t1}");
                let ge = ge_two_pass_channel(t0, t1, 255, 255);
                assert!(ge.abs_diff(rdp) <= 1, "t0 {t0} t1 {t1}: ge {ge} rdp {rdp}");
            }
        }
    }

    #[test]
    fn midpoint_blend_matches_the_rdp_equation() {
        for t0 in 0..=255u8 {
            for t1 in 0..=255u8 {
                let rdp = rdp_combine_channel(t1, t0, 128, t0);
                let exact = (u32::from(t0) + u32::from(t1)) as f32 / 2.0;
                assert!((f32::from(rdp) - exact).abs() <= 0.5);
                let ge = ge_two_pass_channel(t0, t1, 255, 128);
                assert!(ge.abs_diff(rdp) <= 1, "t0 {t0} t1 {t1}: ge {ge} rdp {rdp}");
            }
        }
    }

    #[test]
    fn every_fraction_and_shade_stays_within_two_steps_of_the_rdp() {
        // A shade below 255 rounds twice on the GE (each pass modulates) and
        // once on the RDP, so this bound is one step wider than the others.
        let mut worst = 0u8;
        for frac in 0..=255u8 {
            for t0 in (0..=255u8).step_by(17) {
                for t1 in (0..=255u8).step_by(15) {
                    for shade in [255u8, 200, 128, 64] {
                        let ge = ge_two_pass_channel(t0, t1, shade, frac);
                        let rdp = rdp_two_cycle_channel(t0, t1, shade, frac);
                        worst = worst.max(ge.abs_diff(rdp));
                    }
                }
            }
        }
        assert!(worst <= 2, "worst two-pass error {worst}");
    }

    #[test]
    fn fix_weights_sum_to_one_and_track_the_rdp_weight() {
        for frac in 0..=255u8 {
            let (fa, fb) = ge_fix_weights(frac);
            assert_eq!(u16::from(fa) + u16::from(fb), 255);
            let rdp = f32::from(frac) / 256.0;
            assert!((f32::from(fa) / 255.0 - rdp).abs() <= 0.5 / 255.0 + 1e-6);
        }
        assert_eq!(ge_fix_weights(0), (0, 255));
        assert_eq!(ge_fix_colors(128), (0x0080_8080, 0x007F_7F7F));
    }
}
