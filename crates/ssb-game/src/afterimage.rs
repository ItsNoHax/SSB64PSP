//! The sword trail's geometry (`ftDisplayMainDrawAfterImage`,
//! `ft/ftdisplaymain.c`, 0x800F1020).
//!
//! `ftMainProcParams` samples the blade into three descriptors
//! ([`crate::fighter::AfterImage`], `combat::update_afterimage`); once two
//! are filled, `ftDisplayMainDrawAll` draws a ribbon of world-space `Vtx`
//! after the fighter's model: two vertices per sample (`near` and `far`
//! along the blade), plus interpolated pairs wherever consecutive samples
//! turn by 30 degrees or more. This module builds those vertices exactly as
//! the original's integer and single-precision arithmetic does; the
//! renderer only draws them.

use crate::fighter::AfterImage;
use ssb_engine::math::Vec3;

/// Vertices one trail can emit: three samples, plus at most six pairs per
/// gap (the angle between unit-length blades is at most pi, so
/// `target_angle` is at most 6 + 1). One `gSPVertex` load holds 32.
pub const TRAIL_VTX_MAX: usize = 32;

/// `F_CLC_DTOR32(30.0F)`: the turn that adds one interpolated pair. The
/// ROM's literal (0x3F060A92) is what the r4300 oracle hash pins.
pub const TRAIL_ROTATE: f32 = f32::from_bits(0x3F06_0A92);

/// The per-variant constants of `ftDisplayMainDrawAfterImage`'s switch.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TrailStyle {
    /// Distance along the blade of the first vertex of each pair.
    pub near: f32,
    /// Distance along the blade of the second vertex.
    pub far: f32,
    /// `color1`, at `near`.
    pub near_rgb: [u8; 3],
    /// `color2`, at `far`.
    pub far_rgb: [u8; 3],
}

/// Link's own sword (`is_itemswing` FALSE).
pub const TRAIL_SWORD: TrailStyle = TrailStyle {
    near: 50.0,
    far: 250.0,
    near_rgb: [0x00, 0xFF, 0xFF],
    far_rgb: [0xFF, 0xFF, 0xFF],
};

/// A held Beam Sword (`is_itemswing` TRUE).
pub const TRAIL_ITEM_SWORD: TrailStyle = TrailStyle {
    near: 80.0,
    far: 580.0,
    near_rgb: [0xFF, 0x40, 0xC0],
    far_rgb: [0xFF, 0xFF, 0xFF],
};

/// `base_alpha` and `add_alpha`: the same in both variants.
const BASE_ALPHA: i32 = 0xFF;
const ADD_ALPHA: i32 = 0;

/// One emitted `Vtx`: `ob` in world units and `cn`. `flag` and `tc` are 0.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TrailVtx {
    pub pos: [i16; 3],
    pub rgba: [u8; 4],
}

/// `fp->afterimage.drawstatus >= 2` (`ftDisplayMainDrawAll`).
pub fn trail_visible(a: &AfterImage) -> bool {
    a.drawstatus >= 2
}

pub fn trail_style(a: &AfterImage) -> &'static TrailStyle {
    if a.is_itemswing {
        &TRAIL_ITEM_SWORD
    } else {
        &TRAIL_SWORD
    }
}

/// A float-to-`s16` store: `trunc.w.s` then `sh`. Rust's `as i32`
/// saturates where the N64 would raise an invalid operation; trail
/// coordinates are stage-bounded `s16` plus at most 580, far inside both.
fn to_s16(v: f32) -> i16 {
    v as i32 as i16
}

fn push(
    out: &mut [TrailVtx; TRAIL_VTX_MAX],
    n: &mut usize,
    t: [i16; 3],
    vec: Vec3,
    dist: f32,
    rgb: [u8; 3],
    alpha: i32,
) {
    if *n >= TRAIL_VTX_MAX {
        // Unreachable for unit or near-unit blades (see TRAIL_VTX_MAX).
        return;
    }
    out[*n] = TrailVtx {
        pos: [
            to_s16(f32::from(t[0]) + vec.x * dist),
            to_s16(f32::from(t[1]) + vec.y * dist),
            to_s16(f32::from(t[2]) + vec.z * dist),
        ],
        // `cn[3] = alpha`: an `sb` of the s32.
        rgba: [rgb[0], rgb[1], rgb[2], alpha as u8],
    };
    *n += 1;
}

/// `syVectorNormCross3D`: the normalized cross product, `None` when it is
/// the zero vector.
fn norm_cross(a: Vec3, b: Vec3) -> Option<Vec3> {
    let mut c = crate::slope_contour::cross(a, b);
    (crate::slope_contour::norm3(&mut c) != 0.0).then_some(c)
}

/// The vertices `ftDisplayMainDrawAfterImage` writes for `a`, newest
/// sample first; returns how many. Call only when [`trail_visible`].
pub fn trail_vertices(a: &AfterImage, out: &mut [TrailVtx; TRAIL_VTX_MAX]) -> usize {
    let style = trail_style(a);
    let (near, far) = (style.near, style.far);
    let ds = i32::from(a.drawstatus);
    if ds < 2 {
        return 0;
    }
    let step = (BASE_ALPHA - ADD_ALPHA) / (ds - 1);
    let mut n = 0;
    let mut index = if a.desc_id != 0 {
        usize::from(a.desc_id) - 1
    } else {
        a.desc.len() - 1
    };
    let mut i = ds - 1;
    while i >= 0 {
        let d = &a.desc[index];
        let mut alpha = step * i + ADD_ALPHA;
        push(out, &mut n, d.translate, d.vec, near, style.near_rgb, alpha);
        push(out, &mut n, d.translate, d.vec, far, style.far_rgb, alpha);
        let next_index = if index != 0 {
            index - 1
        } else {
            a.desc.len() - 1
        };
        if i != 0 {
            let next = &a.desc[next_index];
            if let Some(axis) = norm_cross(d.vec, next.vec) {
                let mut angle = crate::slope_contour::angle_diff(d.vec, next.vec);
                let mut target = (angle / TRAIL_ROTATE) as i32;
                if target != 0 {
                    target += 1;
                    let scale = 1.0 / target as f32;
                    let mut t = d.translate;
                    angle *= scale;
                    // `(next - n_ai) * scale` to `s16`; the walk adds in `s16`.
                    let inc = [0, 1, 2].map(|k| {
                        to_s16((i32::from(next.translate[k]) - i32::from(t[k])) as f32 * scale)
                    });
                    let mut vec = d.vec;
                    let alpha_inc = (((step * (i - 1) + ADD_ALPHA) - alpha) as f32 * scale) as i32;
                    for _ in 0..target - 1 {
                        for k in 0..3 {
                            t[k] = t[k].wrapping_add(inc[k]);
                        }
                        vec = crate::item::rotate_about(vec, axis, angle);
                        alpha += alpha_inc;
                        push(out, &mut n, t, vec, near, style.near_rgb, alpha);
                        push(out, &mut n, t, vec, far, style.far_rgb, alpha);
                    }
                }
            }
        }
        index = next_index;
        i -= 1;
    }
    n
}

/// The triangles over `n` trail vertices (`gSP2Triangles(i, i+1, i+2;
/// i+1, i+3, i+2)` for each pair step), drawn without culling.
pub fn trail_triangles(n: usize) -> impl Iterator<Item = [usize; 3]> {
    (0..n.saturating_sub(2))
        .step_by(2)
        .flat_map(|i| [[i, i + 1, i + 2], [i + 1, i + 3, i + 2]])
}

#[cfg(test)]
mod tests;
