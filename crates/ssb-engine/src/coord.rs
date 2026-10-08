//! N64 → PSP coordinate and matrix conversion.
//!
//! **This is the only module allowed to convert between the two systems.**
//! Gameplay runs entirely in the original game's space; the renderer calls in
//! here on the way to the GPU. Scattering conversions through gameplay code is
//! how ports end up with mirrored characters and inside-out collision.
//!
//! ## What is actually the same
//!
//! Both the N64 (via `guPerspective`/`guLookAt`) and the PSP (via
//! `sceGumPerspective`/`sceGumLookAt`) use a **right-handed** view space
//! looking down `-Z`, with `+Y` up. Smash's world space follows suit: `+Y` is
//! up (`ftPhysicsApplyGravityClampTVel` does `vel_air.y -= gravity`) and `Z`
//! is the shallow depth axis, clamped to ±60 units.
//!
//! So positions need **no handedness flip**. [`n64_to_psp_position`] is an
//! identity today, and exists so that if a discrepancy turns up on hardware
//! there is exactly one place to fix it.
//!
//! ## What genuinely differs
//!
//! | | N64 (F3DEX2) | PSP (sceGu) |
//! |---|---|---|
//! | Matrix element type | `s16.16` fixed point, split hi/lo | `f32` |
//! | Matrix storage | row-major-ish, interleaved halves | column-major `f32[16]` |
//! | Screen | 320x240 (game renders 320x240) | 480x272 |
//! | Depth buffer | 18-bit, non-linear | 16-bit, linear |
//!
//! The fixed-point matrix layout is the sharp edge: an N64 `Mtx` is
//! **not** 16 consecutive fixed-point numbers. It is 16 `u16` high halves
//! followed by 16 `u16` low halves, so element `i` is
//! `((hi[i] as i32) << 16 | lo[i] as i32) as f32 / 65536.0`.

use crate::math::{Mat4, Vec3};

/// Native resolution the original game renders at.
pub const N64_SCREEN: (u32, u32) = (320, 240);

/// PSP display resolution.
pub const PSP_SCREEN: (u32, u32) = (480, 272);

/// An N64 `Mtx` exactly as stored in ROM: 16 high halves then 16 low halves,
/// each big-endian, forming s15.16 fixed-point values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct N64Matrix {
    pub raw: [u16; 32],
}

impl N64Matrix {
    pub const SIZE: usize = 64;

    /// Parses the 64 big-endian bytes of an `Mtx`.
    pub fn from_bytes(raw: &[u8; Self::SIZE]) -> N64Matrix {
        let mut out = [0u16; 32];
        for (i, c) in raw.as_chunks::<2>().0.iter().enumerate() {
            out[i] = u16::from_be_bytes(*c);
        }
        N64Matrix { raw: out }
    }

    /// Reassembles element `i` (row-major index) as a float.
    fn element(&self, i: usize) -> f32 {
        let hi = self.raw[i] as u32;
        let lo = self.raw[i + 16] as u32;
        (((hi << 16) | lo) as i32) as f32 / 65536.0
    }
}

/// Converts an N64 fixed-point matrix into a PSP-ready column-major `Mat4`.
///
/// **No transpose is needed**, which is worth spelling out because it looks
/// wrong at a glance.
///
/// The N64 stores elements row-major and uses the *row-vector* convention
/// (`v' = v * M`), so translation sits at `m[3][0..2]`. The PSP stores
/// column-major and uses the *column-vector* convention (`v' = M * v`), so
/// translation sits in the last column.
///
/// Working it through: N64 gives `result[i] = Σⱼ v[j] · M64[j][i]`, PSP gives
/// `result[i] = Σⱼ Mpsp[i][j] · v[j]`. Equating them, `Mpsp[i][j] = M64[j][i]`
/// — a transpose. But column-major storage means `cols[j][i] = Mpsp[i][j]`,
/// so `cols[j][i] = M64[j][i]`: the two transposes cancel and the linear
/// element order is identical.
///
/// The practical upshot is that the *only* real work is widening the s15.16
/// fixed-point elements to `f32`.
pub fn n64_to_psp_matrix(m: &N64Matrix) -> Mat4 {
    let mut out = Mat4::ZERO;
    for c in 0..4 {
        for r in 0..4 {
            out.cols[c][r] = m.element(c * 4 + r);
        }
    }
    out
}

/// Converts a world-space position from the game's space to render space.
///
/// Currently an identity: both systems are right-handed, `+Y` up. Kept as a
/// named function so the renderer never hardcodes the assumption, and so any
/// future correction lands in one place.
#[inline]
pub fn n64_to_psp_position(v: Vec3) -> Vec3 {
    v
}

/// Converts a direction vector. Same reasoning as [`n64_to_psp_position`].
#[inline]
pub fn n64_to_psp_direction(v: Vec3) -> Vec3 {
    v
}

/// Converts a texture coordinate from the N64's S10.5 fixed point to
/// normalized floats, given the texture's dimensions.
///
/// The RDP addresses texels in 1/32nds of a texel, so the raw value is divided
/// by 32 to get texels, then by the dimension to normalize.
pub fn n64_uv_to_normalized(uv: [i16; 2], width: u32, height: u32) -> (f32, f32) {
    let s = uv[0] as f32 / 32.0;
    let t = uv[1] as f32 / 32.0;
    (s / width.max(1) as f32, t / height.max(1) as f32)
}

/// The part of the N64's 320x240 frame a CRT showed: `(10, 10)` to
/// `(310, 230)`.
///
/// SSB64 draws its 3D and its interface inside this inset box
/// (`gmCameraSetViewportDimensions(10, 10, 310, 230)`, the wallpaper's
/// `gsDPFillRectangle(10, 10, 310, 230)`); the 10-pixel strip around it
/// held only background and fell into a television's overscan. The port
/// crops it the same way ([D-047](../../../docs/decisions/D-047.md)).
pub const N64_VISIBLE: [f32; 4] = [10.0, 10.0, 310.0, 230.0];

/// PSP pixels per N64 pixel, on both axes: the visible box's 220 lines
/// fill the PSP's 272.
pub const SCREEN_SCALE: f32 = PSP_SCREEN.1 as f32 / (N64_VISIBLE[3] - N64_VISIBLE[1]);

/// The one N64 → PSP screen mapping: an N64 screen x (in N64 pixels, edges
/// at integers) to a PSP screen x. The visible box's centre, `(160, 120)`,
/// lands on the PSP screen's centre; the box spans the PSP's full height
/// and about 371 of its 480 columns; the strip lands off the screen or in
/// the black bars beside the picture, outside [`visible_area`].
#[inline]
pub fn n64_to_psp_x(x: f32) -> f32 {
    let c = (N64_VISIBLE[0] + N64_VISIBLE[2]) * 0.5;
    PSP_SCREEN.0 as f32 * 0.5 + (x - c) * SCREEN_SCALE
}

/// The y half of [`n64_to_psp_x`].
#[inline]
pub fn n64_to_psp_y(y: f32) -> f32 {
    let c = (N64_VISIBLE[1] + N64_VISIBLE[3]) * 0.5;
    PSP_SCREEN.1 as f32 * 0.5 + (y - c) * SCREEN_SCALE
}

/// An N64 screen rectangle `[ulx, uly, lrx, lry]` in PSP screen
/// coordinates, unclipped.
#[inline]
pub fn n64_rect_to_psp([ulx, uly, lrx, lry]: [f32; 4]) -> [f32; 4] {
    [
        n64_to_psp_x(ulx),
        n64_to_psp_y(uly),
        n64_to_psp_x(lrx),
        n64_to_psp_y(lry),
    ]
}

/// An N64 screen rectangle `[ulx, uly, lrx, lry]` as a PSP `[x, y, w, h]`.
#[inline]
pub fn n64_rect_to_psp_xywh(rect: [f32; 4]) -> [f32; 4] {
    let [x0, y0, x1, y1] = n64_rect_to_psp(rect);
    [x0, y0, x1 - x0, y1 - y0]
}

/// A PSP-space edge as a whole-pixel edge: the pixels between two such
/// edges are those whose centres lie between the two PSP-space edges.
#[inline]
pub fn psp_pixel_edge(v: f32) -> i32 {
    // `ceil(v - 0.5)`: truncation rounds towards zero, so step up when it
    // fell below.
    let v = v - 0.5;
    let t = v as i32;
    if (t as f32) < v {
        t + 1
    } else {
        t
    }
}

/// The PSP pixel whose area holds N64 pixel `x`'s centre: where a
/// framebuffer copy samples that N64 pixel.
#[inline]
pub fn n64_pixel_to_psp_column(x: i32) -> u32 {
    (n64_to_psp_x(x as f32 + 0.5) as u32).min(PSP_SCREEN.0 - 1)
}

/// The row half of [`n64_pixel_to_psp_column`].
#[inline]
pub fn n64_pixel_to_psp_row(y: i32) -> u32 {
    (n64_to_psp_y(y as f32 + 0.5) as u32).min(PSP_SCREEN.1 - 1)
}

/// The PSP pixels showing the N64 picture, `(x, y, w, h)`: those whose
/// centres lie inside [`N64_VISIBLE`]. Everything outside is a black
/// pillarbox bar; nothing the game draws may reach it.
pub fn visible_area() -> (u32, u32, u32, u32) {
    let [x0, y0, x1, y1] = scissor_pixels(n64_rect_to_psp(N64_VISIBLE));
    (x0 as u32, y0 as u32, (x1 - x0) as u32, (y1 - y0) as u32)
}

/// The PSP scissor `[x0, y0, x1, y1]` (ends exclusive) for an N64 screen
/// rectangle: the pixels whose centres lie inside it, clipped to
/// [`visible_area`] (the RDP's scissor likewise covers the pixels whose
/// centres it holds, and the CRT shows only the visible box).
pub fn n64_scissor(rect: [f32; 4]) -> [i32; 4] {
    let [vx0, vy0, vx1, vy1] = scissor_pixels(n64_rect_to_psp(N64_VISIBLE));
    let [x0, y0, x1, y1] = scissor_pixels(n64_rect_to_psp(rect));
    let x0 = x0.clamp(vx0, vx1);
    let y0 = y0.clamp(vy0, vy1);
    [x0, y0, x1.clamp(x0, vx1), y1.clamp(y0, vy1)]
}

/// The pixels whose centres lie inside a PSP-space rectangle.
fn scissor_pixels(rect: [f32; 4]) -> [i32; 4] {
    rect.map(psp_pixel_edge)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Builds an N64Matrix from 16 row-major floats, the way `guMtxF2L` would.
    fn from_floats(m: [f32; 16]) -> N64Matrix {
        let mut raw = [0u16; 32];
        for (i, v) in m.iter().enumerate() {
            let fixed = (v * 65536.0) as i32;
            raw[i] = (fixed >> 16) as u16;
            raw[i + 16] = (fixed & 0xFFFF) as u16;
        }
        N64Matrix { raw }
    }

    #[test]
    fn decodes_fixed_point_identity() {
        #[rustfmt::skip]
        let m = from_floats([
            1.0, 0.0, 0.0, 0.0,
            0.0, 1.0, 0.0, 0.0,
            0.0, 0.0, 1.0, 0.0,
            0.0, 0.0, 0.0, 1.0,
        ]);
        assert_eq!(n64_to_psp_matrix(&m), Mat4::IDENTITY);
    }

    #[test]
    fn decodes_negative_and_fractional_elements() {
        #[rustfmt::skip]
        let m = from_floats([
            0.5, 0.0,  0.0, 0.0,
            0.0, -2.5, 0.0, 0.0,
            0.0, 0.0,  1.0, 0.0,
            0.0, 0.0,  0.0, 1.0,
        ]);
        let c = n64_to_psp_matrix(&m);
        assert_eq!(c.cols[0][0], 0.5);
        assert_eq!(c.cols[1][1], -2.5);
    }

    #[test]
    fn row_vector_translation_lands_in_the_translation_column() {
        // libultra puts translation at m[3][0..2] under the row-vector
        // convention. After conversion it must be readable as Mat4's
        // translation column, so that `transform_point` moves a point by it.
        #[rustfmt::skip]
        let m = from_floats([
            1.0, 0.0, 0.0, 0.0,
            0.0, 1.0, 0.0, 0.0,
            0.0, 0.0, 1.0, 0.0,
            7.0, 8.0, 9.0, 1.0,
        ]);
        let c = n64_to_psp_matrix(&m);
        assert_eq!(c.cols[3], [7.0, 8.0, 9.0, 1.0]);
        assert_eq!(c.transform_point(Vec3::ZERO), Vec3::new(7.0, 8.0, 9.0));
    }

    #[test]
    fn matrix_round_trips_through_bytes() {
        let m = from_floats([
            1.5, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 1.0, 2.0, 3.0, 1.0,
        ]);
        let mut bytes = [0u8; N64Matrix::SIZE];
        for (i, v) in m.raw.iter().enumerate() {
            bytes[i * 2..i * 2 + 2].copy_from_slice(&v.to_be_bytes());
        }
        assert_eq!(N64Matrix::from_bytes(&bytes), m);
    }

    #[test]
    fn uv_conversion_divides_by_32_then_normalizes() {
        // 32 in S10.5 is exactly one texel; on a 64-wide texture that is
        // 1/64th of the way across.
        let (s, t) = n64_uv_to_normalized([32, 64], 64, 32);
        assert_eq!(s, 1.0 / 64.0);
        assert_eq!(t, 2.0 / 32.0);
    }

    #[test]
    fn visible_box_fills_the_height_at_four_by_three() {
        assert_eq!(n64_to_psp_y(10.0), 0.0);
        assert_eq!(n64_to_psp_y(230.0), 272.0);
        assert_eq!(n64_to_psp_x(160.0), 240.0);
        // The same scale on both axes: no stretching.
        let [x0, y0, x1, y1] = n64_rect_to_psp(N64_VISIBLE);
        let aspect = (x1 - x0) / (y1 - y0);
        assert!((aspect - 300.0 / 220.0).abs() < 1e-5, "aspect {aspect}");
        // About 371 columns wide, centred: 55..425 hold pixel centres.
        assert_eq!(visible_area(), (55, 0, 370, 272));
    }

    #[test]
    fn the_strip_is_cropped() {
        // The whole frame scissors to the visible area.
        assert_eq!(n64_scissor([0.0, 0.0, 320.0, 240.0]), [55, 0, 425, 272]);
        // A rectangle wholly inside the strip covers nothing.
        let [x0, _, x1, _] = n64_scissor([0.0, 0.0, 10.0, 240.0]);
        assert_eq!(x0, x1);
        let [_, y0, _, y1] = n64_scissor([0.0, 230.0, 320.0, 240.0]);
        assert_eq!(y0, y1);
        // The N64 pixels just inside the box sample the visible area.
        assert_eq!(n64_pixel_to_psp_column(10), 55);
        assert_eq!(n64_pixel_to_psp_column(309), 424);
        assert_eq!(n64_pixel_to_psp_row(10), 0);
        assert_eq!(n64_pixel_to_psp_row(229), 271);
    }
}
