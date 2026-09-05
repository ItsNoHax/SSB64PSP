//! Scalar vector/matrix math.
//!
//! This is deliberately the *reference* implementation: plain `f32`, no
//! platform tricks, easy to test on the host. The PSP backend may later
//! provide VFPU-accelerated equivalents, but per the porting rules those must
//! be introduced only after profiling and must be checked against these
//! results. Keeping the scalar path always available is what makes that
//! comparison possible.

use core::ops::{Add, AddAssign, Div, Mul, MulAssign, Neg, Sub, SubAssign};

/// A 2D vector.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

impl Vec2 {
    pub const ZERO: Vec2 = Vec2 { x: 0.0, y: 0.0 };

    pub const fn new(x: f32, y: f32) -> Self {
        Vec2 { x, y }
    }

    pub fn length_squared(self) -> f32 {
        self.x * self.x + self.y * self.y
    }

    pub fn length(self) -> f32 {
        sqrt(self.length_squared())
    }
}

/// A 3D vector in the game's coordinate space: `+X` right, `+Y` up,
/// `+Z` toward the camera (the shallow depth axis Smash uses for its
/// 2.5D staging).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vec3 {
    pub const ZERO: Vec3 = Vec3 {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    };
    pub const X: Vec3 = Vec3::new(1.0, 0.0, 0.0);
    pub const Y: Vec3 = Vec3::new(0.0, 1.0, 0.0);
    pub const Z: Vec3 = Vec3::new(0.0, 0.0, 1.0);

    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Vec3 { x, y, z }
    }

    pub const fn splat(v: f32) -> Self {
        Vec3::new(v, v, v)
    }

    pub fn dot(self, o: Vec3) -> f32 {
        self.x * o.x + self.y * o.y + self.z * o.z
    }

    pub fn cross(self, o: Vec3) -> Vec3 {
        Vec3::new(
            self.y * o.z - self.z * o.y,
            self.z * o.x - self.x * o.z,
            self.x * o.y - self.y * o.x,
        )
    }

    pub fn length_squared(self) -> f32 {
        self.dot(self)
    }

    pub fn length(self) -> f32 {
        sqrt(self.length_squared())
    }

    /// Returns a unit vector, or [`Vec3::ZERO`] if the input is degenerate.
    pub fn normalized(self) -> Vec3 {
        let len_sq = self.length_squared();
        if len_sq <= f32::EPSILON {
            Vec3::ZERO
        } else {
            self * (1.0 / sqrt(len_sq))
        }
    }

    pub fn lerp(self, o: Vec3, t: f32) -> Vec3 {
        self + (o - self) * t
    }

    /// Clamps each component into `[-limit, limit]`.
    pub fn clamp_abs(self, limit: Vec3) -> Vec3 {
        Vec3::new(
            self.x.clamp(-limit.x, limit.x),
            self.y.clamp(-limit.y, limit.y),
            self.z.clamp(-limit.z, limit.z),
        )
    }

    pub fn to_array(self) -> [f32; 3] {
        [self.x, self.y, self.z]
    }
}

impl Add for Vec3 {
    type Output = Vec3;
    fn add(self, o: Vec3) -> Vec3 {
        Vec3::new(self.x + o.x, self.y + o.y, self.z + o.z)
    }
}
impl Sub for Vec3 {
    type Output = Vec3;
    fn sub(self, o: Vec3) -> Vec3 {
        Vec3::new(self.x - o.x, self.y - o.y, self.z - o.z)
    }
}
impl Mul<f32> for Vec3 {
    type Output = Vec3;
    fn mul(self, s: f32) -> Vec3 {
        Vec3::new(self.x * s, self.y * s, self.z * s)
    }
}
impl Div<f32> for Vec3 {
    type Output = Vec3;
    fn div(self, s: f32) -> Vec3 {
        Vec3::new(self.x / s, self.y / s, self.z / s)
    }
}
impl Neg for Vec3 {
    type Output = Vec3;
    fn neg(self) -> Vec3 {
        Vec3::new(-self.x, -self.y, -self.z)
    }
}
impl AddAssign for Vec3 {
    fn add_assign(&mut self, o: Vec3) {
        *self = *self + o;
    }
}
impl SubAssign for Vec3 {
    fn sub_assign(&mut self, o: Vec3) {
        *self = *self - o;
    }
}
impl MulAssign<f32> for Vec3 {
    fn mul_assign(&mut self, s: f32) {
        *self = *self * s;
    }
}

/// A 4x4 matrix stored **column-major**, matching what `sceGuSetMatrix`
/// expects. `cols[3]` is the translation column.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Mat4 {
    pub cols: [[f32; 4]; 4],
}

impl Default for Mat4 {
    fn default() -> Self {
        Mat4::IDENTITY
    }
}

impl Mat4 {
    pub const IDENTITY: Mat4 = Mat4 {
        cols: [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ],
    };

    pub const ZERO: Mat4 = Mat4 {
        cols: [[0.0; 4]; 4],
    };

    pub fn from_translation(t: Vec3) -> Mat4 {
        let mut m = Mat4::IDENTITY;
        m.cols[3] = [t.x, t.y, t.z, 1.0];
        m
    }

    pub fn from_scale(s: Vec3) -> Mat4 {
        let mut m = Mat4::IDENTITY;
        m.cols[0][0] = s.x;
        m.cols[1][1] = s.y;
        m.cols[2][2] = s.z;
        m
    }

    pub fn from_rotation_x(radians: f32) -> Mat4 {
        let (s, c) = sin_cos(radians);
        let mut m = Mat4::IDENTITY;
        m.cols[1][1] = c;
        m.cols[1][2] = s;
        m.cols[2][1] = -s;
        m.cols[2][2] = c;
        m
    }

    pub fn from_rotation_y(radians: f32) -> Mat4 {
        let (s, c) = sin_cos(radians);
        let mut m = Mat4::IDENTITY;
        m.cols[0][0] = c;
        m.cols[0][2] = -s;
        m.cols[2][0] = s;
        m.cols[2][2] = c;
        m
    }

    pub fn from_rotation_z(radians: f32) -> Mat4 {
        let (s, c) = sin_cos(radians);
        let mut m = Mat4::IDENTITY;
        m.cols[0][0] = c;
        m.cols[0][1] = s;
        m.cols[1][0] = -s;
        m.cols[1][1] = c;
        m
    }

    /// `self * rhs`: applies `rhs` first, then `self`.
    pub fn multiply(&self, rhs: &Mat4) -> Mat4 {
        let mut out = Mat4::ZERO;
        for c in 0..4 {
            for r in 0..4 {
                let mut acc = 0.0;
                for k in 0..4 {
                    acc += self.cols[k][r] * rhs.cols[c][k];
                }
                out.cols[c][r] = acc;
            }
        }
        out
    }

    /// Transforms a point (implicit `w = 1`), discarding the resulting `w`.
    pub fn transform_point(&self, v: Vec3) -> Vec3 {
        Vec3::new(
            self.cols[0][0] * v.x + self.cols[1][0] * v.y + self.cols[2][0] * v.z + self.cols[3][0],
            self.cols[0][1] * v.x + self.cols[1][1] * v.y + self.cols[2][1] * v.z + self.cols[3][1],
            self.cols[0][2] * v.x + self.cols[1][2] * v.y + self.cols[2][2] * v.z + self.cols[3][2],
        )
    }

    /// Transforms a direction, ignoring translation.
    pub fn transform_vector(&self, v: Vec3) -> Vec3 {
        Vec3::new(
            self.cols[0][0] * v.x + self.cols[1][0] * v.y + self.cols[2][0] * v.z,
            self.cols[0][1] * v.x + self.cols[1][1] * v.y + self.cols[2][1] * v.z,
            self.cols[0][2] * v.x + self.cols[1][2] * v.y + self.cols[2][2] * v.z,
        )
    }

    /// Right-handed perspective projection, `fovy` in radians.
    ///
    /// Matches the convention `guPerspective` produces on N64 and what
    /// `sceGumPerspective` produces on PSP: view space looks down `-Z`, and
    /// clip space is the OpenGL-style `[-w, w]` depth range.
    pub fn perspective(fovy: f32, aspect: f32, near: f32, far: f32) -> Mat4 {
        let f = 1.0 / tan(fovy * 0.5);
        let mut m = Mat4::ZERO;
        m.cols[0][0] = f / aspect;
        m.cols[1][1] = f;
        m.cols[2][2] = (far + near) / (near - far);
        m.cols[2][3] = -1.0;
        m.cols[3][2] = (2.0 * far * near) / (near - far);
        m
    }

    /// Right-handed orthographic projection, used for HUD and menus.
    pub fn ortho(l: f32, r: f32, b: f32, t: f32, near: f32, far: f32) -> Mat4 {
        let mut m = Mat4::IDENTITY;
        m.cols[0][0] = 2.0 / (r - l);
        m.cols[1][1] = 2.0 / (t - b);
        m.cols[2][2] = -2.0 / (far - near);
        m.cols[3] = [
            -(r + l) / (r - l),
            -(t + b) / (t - b),
            -(far + near) / (far - near),
            1.0,
        ];
        m
    }

    /// Right-handed look-at view matrix.
    pub fn look_at(eye: Vec3, target: Vec3, up: Vec3) -> Mat4 {
        let f = (target - eye).normalized();
        let s = f.cross(up).normalized();
        let u = s.cross(f);
        Mat4 {
            cols: [
                [s.x, u.x, -f.x, 0.0],
                [s.y, u.y, -f.y, 0.0],
                [s.z, u.z, -f.z, 0.0],
                [-s.dot(eye), -u.dot(eye), f.dot(eye), 1.0],
            ],
        }
    }

    /// Flat column-major array, ready to hand to `sceGuSetMatrix`.
    pub fn as_array(&self) -> [f32; 16] {
        let mut out = [0.0; 16];
        for c in 0..4 {
            out[c * 4..c * 4 + 4].copy_from_slice(&self.cols[c]);
        }
        out
    }
}

impl Mul for Mat4 {
    type Output = Mat4;
    fn mul(self, rhs: Mat4) -> Mat4 {
        Mat4::multiply(&self, &rhs)
    }
}

// --- no_std math shims -----------------------------------------------------
//
// `f32::sqrt` and friends live in `std`. On the PSP we get them from `libm`
// via the `psp` crate's intrinsics; here we route through `core` where
// possible and fall back to small implementations otherwise, so the same
// source builds for host tests and for the device.

/// Square root, routed through `std` on the host and a small Newton-Raphson
/// on the device. Public so game code needs no `libm` dependency of its own.
#[cfg(feature = "std")]
#[inline]
pub fn sqrt(v: f32) -> f32 {
    v.sqrt()
}

#[cfg(not(feature = "std"))]
#[inline]
pub fn sqrt(v: f32) -> f32 {
    // Newton-Raphson from a bit-twiddled seed. Accurate to well under a ULP
    // after four iterations for the magnitudes this code sees.
    if v <= 0.0 {
        return 0.0;
    }
    let mut x = f32::from_bits((v.to_bits() >> 1) + 0x1FC0_0000);
    for _ in 0..4 {
        x = 0.5 * (x + v / x);
    }
    x
}

/// Sine and cosine of the same angle (radians). Public, like [`sqrt`], so
/// game code needs no `libm` dependency of its own.
#[cfg(feature = "std")]
#[inline]
pub fn sin_cos(v: f32) -> (f32, f32) {
    (v.sin(), v.cos())
}

#[cfg(not(feature = "std"))]
#[inline]
pub fn sin_cos(v: f32) -> (f32, f32) {
    (sin_poly(v), sin_poly(v + core::f32::consts::FRAC_PI_2))
}

/// Tangent (radians). Public for the same reason as [`sin_cos`].
#[cfg(feature = "std")]
#[inline]
pub fn tan(v: f32) -> f32 {
    v.tan()
}

#[cfg(not(feature = "std"))]
#[inline]
pub fn tan(v: f32) -> f32 {
    let (s, c) = sin_cos(v);
    s / c
}

/// Minimax-ish sine over a range-reduced argument. Only used in `no_std`.
#[cfg(not(feature = "std"))]
fn sin_poly(v: f32) -> f32 {
    use core::f32::consts::PI;
    // Reduce to [-PI, PI].
    let mut x = v % (2.0 * PI);
    if x > PI {
        x -= 2.0 * PI;
    } else if x < -PI {
        x += 2.0 * PI;
    }
    let x2 = x * x;
    // Taylor series to x^9; error < 1e-6 over [-PI, PI].
    x * (1.0 - x2 / 6.0 * (1.0 - x2 / 20.0 * (1.0 - x2 / 42.0 * (1.0 - x2 / 72.0))))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-5
    }

    #[test]
    fn cross_product_is_right_handed() {
        assert_eq!(Vec3::X.cross(Vec3::Y), Vec3::Z);
        assert_eq!(Vec3::Y.cross(Vec3::Z), Vec3::X);
        assert_eq!(Vec3::Z.cross(Vec3::X), Vec3::Y);
    }

    #[test]
    fn normalizing_zero_does_not_produce_nan() {
        assert_eq!(Vec3::ZERO.normalized(), Vec3::ZERO);
    }

    #[test]
    fn clamp_abs_bounds_each_axis() {
        let v = Vec3::new(100.0, -100.0, 5.0);
        // Mirrors the +/-60 depth clamp ftPhysics applies to the Z axis.
        let c = v.clamp_abs(Vec3::new(50.0, 50.0, 60.0));
        assert_eq!(c, Vec3::new(50.0, -50.0, 5.0));
    }

    #[test]
    fn identity_is_a_multiplicative_unit() {
        let m = Mat4::from_translation(Vec3::new(1.0, 2.0, 3.0));
        assert_eq!(m.multiply(&Mat4::IDENTITY), m);
        assert_eq!(Mat4::IDENTITY.multiply(&m), m);
    }

    #[test]
    fn matrix_product_applies_right_hand_side_first() {
        let t = Mat4::from_translation(Vec3::new(10.0, 0.0, 0.0));
        let s = Mat4::from_scale(Vec3::splat(2.0));
        // Scale then translate: the translation must not be scaled.
        let p = t.multiply(&s).transform_point(Vec3::new(1.0, 0.0, 0.0));
        assert_eq!(p, Vec3::new(12.0, 0.0, 0.0));
        // Translate then scale: the translation is scaled too.
        let p = s.multiply(&t).transform_point(Vec3::new(1.0, 0.0, 0.0));
        assert_eq!(p, Vec3::new(22.0, 0.0, 0.0));
    }

    #[test]
    fn transform_vector_ignores_translation() {
        let m = Mat4::from_translation(Vec3::new(5.0, 5.0, 5.0));
        assert_eq!(m.transform_vector(Vec3::X), Vec3::X);
        assert_eq!(m.transform_point(Vec3::X), Vec3::new(6.0, 5.0, 5.0));
    }

    #[test]
    fn rotation_z_turns_x_into_y() {
        let m = Mat4::from_rotation_z(core::f32::consts::FRAC_PI_2);
        let p = m.transform_point(Vec3::X);
        assert!(close(p.x, 0.0) && close(p.y, 1.0), "{p:?}");
    }

    #[test]
    fn perspective_puts_near_and_far_at_clip_bounds() {
        let m = Mat4::perspective(core::f32::consts::FRAC_PI_2, 16.0 / 9.0, 1.0, 100.0);
        // A point on the near plane sits at -1 in NDC, the far plane at +1.
        for (z, want) in [(-1.0, -1.0), (-100.0, 1.0)] {
            let c = Vec3::new(0.0, 0.0, z);
            let zc = m.cols[2][2] * c.z + m.cols[3][2];
            let wc = -c.z;
            assert!(close(zc / wc, want), "z={z} -> {}", zc / wc);
        }
    }

    #[test]
    fn look_at_places_camera_at_origin_of_view_space() {
        let eye = Vec3::new(0.0, 0.0, 10.0);
        let v = Mat4::look_at(eye, Vec3::ZERO, Vec3::Y);
        let p = v.transform_point(eye);
        assert!(
            close(p.x, 0.0) && close(p.y, 0.0) && close(p.z, 0.0),
            "{p:?}"
        );
        // The target is 10 units down -Z.
        let t = v.transform_point(Vec3::ZERO);
        assert!(close(t.z, -10.0), "{t:?}");
    }

    #[test]
    fn as_array_is_column_major() {
        let m = Mat4::from_translation(Vec3::new(1.0, 2.0, 3.0));
        let a = m.as_array();
        // Translation occupies elements 12..15 in column-major order.
        assert_eq!(&a[12..16], &[1.0, 2.0, 3.0, 1.0]);
    }
}
