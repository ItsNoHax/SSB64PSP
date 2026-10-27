//! A fighter's fog colour (`ftDisplayMainDrawAll`, `ftdisplaymain.c:1198`):
//! `G_RM_FOG_PRIM_A` blends every lit pixel towards the fog colour by its
//! alpha. A colour animation's `color1` mixes with the fighter's shade
//! (`ftDisplayMainCalcFogColor`); without one the shade alone is the fog
//! (`ftDisplayMainDecideFogColor`): the team-battle and Yoshi Team shades.
//!
//! No fighter's parts carry `FTPARTS_FLAG_TOGGLEFOG`, so
//! `ftDisplayMainDecideFogDraw`'s per-part swap never runs.

/// `fp->shade_color` (`ftParamSetCostumeShade`, `ftparam.c:1059`): the
/// attribute shade's colour premultiplied by its alpha.
pub fn premultiplied(shade_color: [u8; 4]) -> [i32; 3] {
    let a = i32::from(shade_color[3]);
    core::array::from_fn(|i| i32::from(shade_color[i]) * a / 0xFF)
}

/// The fog colour a fighter draws under, `None` for none (alpha 0, the
/// `gDPSetFogColor(0, 0, 0, 0)` of shade 0, blends nothing).
///
/// `shade` is `FTStruct::shade`, `color1` the colour animation's colour
/// while `is_use_color1` is set, `shade_colors` and `fog_color`
/// `FTAttributes::shade_color` and `fog_color`. The arithmetic is the
/// source's, in `s32` with truncating division and `u8` stores.
pub fn fog_color(
    shade: u8,
    color1: Option<[u8; 4]>,
    shade_colors: &[[u8; 4]; 3],
    fog_color: [u8; 4],
) -> Option<[u8; 4]> {
    let attr = usize::from(shade)
        .checked_sub(1)
        .and_then(|i| shade_colors.get(i).copied());
    let rgba = match (color1, attr) {
        // `ftDisplayMainDecideFogColor`.
        (None, None) => return None,
        (None, Some(c)) => c,
        (Some(ck), attr) => calc(ck, attr, fog_color),
    };
    (rgba[3] != 0).then_some(rgba)
}

/// `ftDisplayMainDrawDefault`'s `ENV` (`ftdisplaymain.c:1178-1190`):
/// `colanim.color2` while `is_use_color2`, else `fog_color` while
/// `is_use_fogcolor` (Master Hand's grey, alpha 0xFF), else `None` for the
/// stage light colour. The alpha is `sFTDisplayMainSkyFogAlpha`, 0xFF for
/// `G_RM_AA_ZB_OPA_SURF2`.
///
/// `color2` is the colour animation's `color2` while set; `fog_grey`
/// `BossState::fog`. The only script that sets `color2`,
/// `dGMColScriptsFighterBossYubideppou3`, sets it opaque white.
pub fn env_color(color2: Option<[u8; 4]>, fog_grey: Option<u8>) -> Option<[u8; 4]> {
    color2.or_else(|| fog_grey.map(|g| [g, g, g, 0xFF]))
}

/// `G_RM_FOG_PRIM_A`'s weights out of 32 for the fog colour and the
/// combiner's colour, `(fog, in)`: `GBL_c1(G_BL_CLR_FOG, G_BL_A_FOG,
/// G_BL_CLR_IN, G_BL_1MA)` in the first cycle of `G_CYC_2CYCLE`
/// (`ftdisplaymain.c:717`, `:1178`).
///
/// The RDP blender takes each alpha's top 5 bits and adds 1 to the second
/// weight: `(fog * (a >> 3) + in * ((~a >> 3) + 1)) >> 5`. The weights
/// always sum to 32, so an opaque fog (`0xFF`, the challenger's black)
/// still lets `in / 32` through, and an alpha under 8 blends nothing.
/// ParaLLEl-RDP (bit-exact with the hardware) shows that leak in the
/// challenger's RDRAM framebuffer: a dithered silhouette of up to one
/// 5-bit level (RE-459).
pub const fn fog_prim_a_weights(alpha: u8) -> (u8, u8) {
    let fog = alpha >> 3;
    (fog, (!alpha >> 3) + 1)
}

/// The share of the combiner's colour `G_RM_FOG_PRIM_A` keeps at fog
/// alpha `alpha` ([`fog_prim_a_weights`]); the rest is the fog colour.
pub fn fog_prim_a_keep(alpha: u8) -> f32 {
    f32::from(fog_prim_a_weights(alpha).1) / 32.0
}

/// `ftDisplayMainCalcFogColor` (`ftdisplaymain.c:600-663`).
fn calc(ck: [u8; 4], attr: Option<[u8; 4]>, fog_color: [u8; 4]) -> [u8; 4] {
    let c = ck.map(i32::from);
    let [mut red, mut green, mut blue, mut alpha] = c;
    if let Some(shade) = attr {
        let fp = premultiplied(shade);
        let shade_base = ((0xFF - i32::from(shade[3])) * (0xFF - c[3])) / 0xFF;
        if shade_base != 0xFF {
            let shade_base = 0xFF - shade_base;
            let mix = |i: usize| ((c[i] - fp[i]) * c[3]) / 0xFF + fp[i];
            let t = mix(0);
            red = (t * 0xFF) / shade_base;
            alpha = if red != 0 {
                (t * 0xFF) / red
            } else {
                ((shade_base - t) * 0xFF) / 0xFF
            };
            green = (mix(1) * 0xFF) / shade_base;
            blue = (mix(2) * 0xFF) / shade_base;
        }
    }
    if fog_color[0] != 0xFF {
        alpha = (i32::from(fog_color[0]) * alpha) / 0xFF;
    }
    // `sFTDisplayMainFogColor` is a `SYColorRGBA`: each store keeps the
    // low byte.
    [red as u8, green as u8, blue as u8, alpha as u8]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every playable fighter's `relocData` literals (`247_YoshiMain.c:342`).
    const SHADES: [[u8; 4]; 3] = [[0xFF, 0xFF, 0xFF, 0x50], [0, 0, 0, 0x50], [0; 4]];
    const FOG: [u8; 4] = [0xFF, 0, 0, 0];

    /// The RDP blender's 5-bit weights: they sum to 32 at every alpha, an
    /// opaque fog keeps `in / 32` (the challenger's black silhouette
    /// leaks a lit 255 as 7) and an alpha under 8 blends nothing. A plain
    /// `1 - a / 255` lerp gives 0 and 0.97 instead.
    #[test]
    fn fog_prim_a_keeps_a_32nd_under_opaque_fog() {
        for a in 0..=0xFF_u8 {
            let (fog, keep) = fog_prim_a_weights(a);
            assert_eq!(fog + keep, 32, "alpha {a:#04X}");
        }
        assert_eq!(fog_prim_a_weights(0xFF), (31, 1));
        assert_eq!(fog_prim_a_weights(0x80), (16, 16));
        assert_eq!(fog_prim_a_weights(7), (0, 32));
        assert_eq!(fog_prim_a_weights(8), (1, 31));
        // Black fog at 0xFF over a lit 255: `(0 * 31 + 255 * 1) >> 5`.
        let (_, keep) = fog_prim_a_weights(0xFF);
        assert_eq!((255 * u32::from(keep)) >> 5, 7);
        assert_eq!(fog_prim_a_keep(0xFF), 1.0 / 32.0);
        assert_eq!(fog_prim_a_keep(0), 1.0);
        assert_eq!(fog_prim_a_keep(7), 1.0);
    }

    /// `color2` takes `ENV` over Master Hand's fog grey; the grey is
    /// opaque; neither leaves the stage light colour.
    #[test]
    fn env_color_prefers_color2_then_fog_grey() {
        let white = [0xFF; 4];
        assert_eq!(env_color(Some(white), Some(0x40)), Some(white));
        assert_eq!(env_color(None, Some(0x40)), Some([0x40, 0x40, 0x40, 0xFF]));
        assert_eq!(env_color(Some([1, 2, 3, 4]), None), Some([1, 2, 3, 4]));
        assert_eq!(env_color(None, None), None);
    }

    #[test]
    fn shade_zero_without_a_colour_animation_draws_no_fog() {
        assert_eq!(fog_color(0, None, &SHADES, FOG), None);
    }

    #[test]
    fn a_shade_without_a_colour_animation_is_its_attribute_colour() {
        assert_eq!(
            fog_color(1, None, &SHADES, FOG),
            Some([0xFF, 0xFF, 0xFF, 0x50])
        );
        assert_eq!(fog_color(2, None, &SHADES, FOG), Some([0, 0, 0, 0x50]));
        // Shade 3's alpha is 0: nothing blends.
        assert_eq!(fog_color(3, None, &SHADES, FOG), None);
    }

    #[test]
    fn shade_zero_passes_the_animation_colour_through() {
        let ck = [0xFF, 0x00, 0x00, 0x80];
        assert_eq!(fog_color(0, Some(ck), &SHADES, FOG), Some(ck));
    }

    /// Hand-computed from `ftDisplayMainCalcFogColor` for shade 1 (white,
    /// alpha 0x50, premultiplied 0x50) and `color1` (0xFF, 0, 0, 0x80):
    /// `shade_base` = 175 * 127 / 255 = 87, then 255 - 87 = 168; red
    /// `t` = (0xFF - 0x50) * 0x80 / 0xFF + 0x50 = 87 + 80 = 167, red =
    /// 167 * 255 / 168 = 253, alpha = 167 * 255 / 253 = 168; green and blue
    /// `t` = (0 - 80) * 128 / 255 + 80 = -40 + 80 = 40, 40 * 255 / 168 = 60.
    #[test]
    fn a_shade_mixes_with_the_animation_colour() {
        let ck = [0xFF, 0x00, 0x00, 0x80];
        assert_eq!(
            fog_color(1, Some(ck), &SHADES, FOG),
            Some([253, 60, 60, 168])
        );
    }

    /// Shade 3 (alpha 0) with an opaque-alpha-0 colour: `shade_base` is
    /// 0xFF, so `color1` passes through.
    #[test]
    fn a_transparent_shade_and_colour_pass_the_colour_through() {
        let ck = [0x10, 0x20, 0x30, 0x00];
        assert_eq!(fog_color(3, Some(ck), &SHADES, FOG), None);
        assert_eq!(calc(ck, Some(SHADES[2]), FOG), ck);
    }

    /// Giant Donkey Kong's fog red 0x80 (`215_GDonkeyMain.c:279`) scales an
    /// animation's alpha: 0x80 * 0x80 / 0xFF = 64.
    #[test]
    fn the_attribute_fog_red_scales_the_animation_alpha() {
        let ck = [0xFF, 0xFF, 0xFF, 0x80];
        assert_eq!(
            fog_color(0, Some(ck), &SHADES, [0x80, 0, 0, 0]),
            Some([0xFF, 0xFF, 0xFF, 64])
        );
    }

    #[test]
    fn the_premultiplied_shade_is_colour_times_alpha() {
        assert_eq!(premultiplied([0xFF, 0xFF, 0xFF, 0x50]), [0x50; 3]);
        assert_eq!(premultiplied([0x80, 0x40, 0, 0xFF]), [0x80, 0x40, 0]);
    }
}
