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

/// Aspect-ratio correction for showing a 320x240 game on a 480x272 screen.
///
/// The PSP is 16:9-ish (1.76) against the N64's 4:3 (1.33). Stretching to fill
/// would distort every character. Returns the viewport that preserves the
/// original aspect ratio, pillarboxed horizontally.
pub fn pillarboxed_viewport() -> (u32, u32, u32, u32) {
    let (sw, sh) = PSP_SCREEN;
    let (nw, nh) = N64_SCREEN;
    // Scale to fit height, since 272/240 < 480/320.
    let scale = sh as f32 / nh as f32;
    let w = (nw as f32 * scale) as u32;
    let x = (sw - w) / 2;
    (x, 0, w, sh)
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
    fn pillarbox_preserves_four_by_three() {
        let (x, _, w, h) = pillarboxed_viewport();
        assert_eq!((w, h), (362, 272));
        // Centred, and never wider than the screen.
        assert_eq!(x, (480 - 362) / 2);
        assert!(w <= PSP_SCREEN.0);
        let aspect = w as f32 / h as f32;
        assert!((aspect - 4.0 / 3.0).abs() < 0.01, "aspect {aspect}");
    }
}
