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
        // Port-only (the draw's projection, not a decomp function).
        let f = 1.0 / sy_tan(fovy * 0.5);
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
// `f32::sqrt` and friends live in `std`. The device has no `std`: square
// root uses the FPU's `sqrt.s` and the rest fall back to small
// implementations, so the same source builds for host tests and the device.

pub mod sqrt_golden;
pub mod trig_golden;

/// Correctly rounded single-precision square root, the N64's own: libultra's
/// `sqrtf` is a leaf around `sqrt.s` (`refs/ssb-decomp-re/src/libultra/gu/sqrtf.s`),
/// IEEE round-to-nearest-even on the R4300. The PSP's Allegrex `sqrt.s` rounds
/// the same way on hardware (RE-497), so the device, the host and the original
/// agree bit for bit.
///
/// No guard: a negative input is NaN on the host and a quiet NaN on the N64,
/// but traps on a PSP with FPU exceptions enabled (RE-469). Use [`sqrt`]
/// unless the input is known non-negative.
#[cfg(feature = "std")]
#[inline(always)]
pub fn sqrt_exact(v: f32) -> f32 {
    v.sqrt()
}

#[cfg(all(not(feature = "std"), target_arch = "mips"))]
#[inline(always)]
pub fn sqrt_exact(v: f32) -> f32 {
    let r: f32;
    // SAFETY: `sqrt.s` only reads and writes FPU registers.
    unsafe {
        core::arch::asm!("sqrt.s {0}, {1}", out(freg) r, in(freg) v, options(pure, nomem, nostack));
    }
    r
}

/// Other `no_std` targets (CI's `thumbv7em-none-eabi` portability build) have
/// no FPU square root to call; `libm`'s software one rounds correctly too.
#[cfg(all(not(feature = "std"), not(target_arch = "mips")))]
#[inline(always)]
pub fn sqrt_exact(v: f32) -> f32 {
    libm::sqrtf(v)
}

/// [`sqrt_exact`] with zero for zero, negative and `-0.0` inputs, on every
/// target. The guard keeps a negative rounding residue from trapping on the
/// PSP (RE-469) and gives `1 / sqrt` callers a positive zero to divide by.
/// NaN passes through as NaN. Public so game code needs no `libm` dependency
/// of its own.
#[inline(always)]
pub fn sqrt(v: f32) -> f32 {
    if v <= 0.0 {
        0.0
    } else {
        sqrt_exact(v)
    }
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

/// `n / d` where the caller's own branch has already excluded `d == 0`.
///
/// A PSP traps the FPU's divide-by-zero and invalid operations (PSPLink
/// enables them; RE-201, RE-469), and LLVM may compute a division before
/// the branch that guards it, since in its default floating-point model a
/// division has no side effects. Out of line, the denominator a speculated
/// call sees is never zero. A zero `d` divides by 1.
#[inline(never)]
pub fn div_nonzero(n: f32, d: f32) -> f32 {
    n / if d == 0.0 { 1.0 } else { d }
}

// --- The original's inverse trigonometry (`sys/utils.c`) -------------------
//
// The N64 game links no `atan2f`, `acosf`, `asinf` or `tanf`: every angle it
// takes back from a ratio comes from `syUtilsArcTan` and the four functions
// built on it, a five-level continued fraction in single precision. Ported
// operation for operation (one IEEE `f32` operation per step, never
// reassociated or fused: the R4300 has no fused multiply-add, Rust never
// contracts), so the host, the PSP and the N64 return the same bits; the
// ROM's own machine code is the test oracle (`trig_golden`). No VFPU: its
// `vasin`/`vrcp` approximations round differently. The N64 runs with
// denormal results flushed to zero (`osInitialize`'s FCSR); these functions
// only meet a denormal when an argument is beyond any game value, and give
// the same result either way except where `sy_arc_tan2`'s ratio itself
// underflows (`|y / x| < 2^-126`).

/// `syUtilsArcTan`'s continued-fraction coefficients
/// (`refs/ssb-decomp-re/src/sys/utils.c:47-55`).
const ARC_TAN_K: [f32; 6] = [
    f32::from_bits(0xbddd_6712), // -0.10810675
    f32::from_bits(0xc232_49a5), // -44.57192
    f32::from_bits(0xbe25_cb3d), // -0.1619081
    f32::from_bits(0xc17c_6261), // -15.774018
    f32::from_bits(0xbf0e_39d2), // -0.55556977
    f32::from_bits(0xc040_000d), // -3.000003
];

/// `M_DTOR_F(90)` and `M_PI_F / 2.0F`: both fold to the `f32` nearest π/2.
const HALF_PI: f32 = core::f32::consts::FRAC_PI_2;
/// `M_PI_F`.
const PI_F: f32 = core::f32::consts::PI;
/// `syUtilsArcSin`/`syUtilsArcCos`'s saturation bound, `0.99999F`.
const ARC_SAT: f32 = 0.99999;

/// `syUtilsArcTan` (`sys/utils.c:25`): the arctangent of a ratio, in
/// `(-π/2, π/2]`. Outside `[-1, 1]` it takes the reciprocal and subtracts
/// from `±π/2`.
pub fn sy_arc_tan(div: f32) -> f32 {
    if div == 0.0 {
        return 0.0;
    }
    // The reciprocal stays out of line: LLVM may compute it before the
    // `div == 0` branch, and a PSP traps `1 / 0` (RE-469).
    let inv = div_nonzero(1.0, div);
    let (d, kind) = if div > 1.0 {
        (inv, 1)
    } else if div < -1.0 {
        (inv, 2)
    } else {
        (div, 0)
    };
    let [k0, k1, k2, k3, k4, k5] = ARC_TAN_K;
    let c = d * d;
    let a = c / k0 + k1;
    let b = c / a + k2;
    let e = c / b + k3;
    let f = c / e + k4;
    let g = c / f + k5;
    let result = (c / g + 1.0) * d;
    match kind {
        1 => HALF_PI - result,
        2 => -HALF_PI - result,
        _ => result,
    }
}

/// `syUtilsArcTan2` (`sys/utils.c:79`): the angle of `(x, y)`, in
/// `[-π, π]`. Not libm's `atan2f`: `x < 0` takes `π - atan|y / x|` and
/// signs it, `x == 0` gives exactly `±π/2`, and the origin gives 0.
pub fn sy_arc_tan2(y: f32, x: f32) -> f32 {
    // The ratio keeps a nonzero denominator out of line: the KO explosion's
    // generators have no velocity, and a PSP traps the `0 / 0` LLVM
    // computes before the `x == 0` branch (RE-469).
    let div = div_nonzero(y, x);
    if x > 0.0 {
        sy_arc_tan(div)
    } else if x < 0.0 {
        let sign = if y < 0.0 { -1.0 } else { 1.0 };
        // `ABSF`, which keeps a `-0.0` (and `sy_arc_tan` maps both zeros to
        // `+0.0`).
        let abs = if div < 0.0 { -div } else { div };
        (PI_F - sy_arc_tan(abs)) * sign
    } else if y != 0.0 {
        (if y < 0.0 { -1.0 } else { 1.0 }) * HALF_PI
    } else {
        0.0
    }
}

/// `x / sqrtf(1 - x * x)`, the tangent of `asin(x)`, for `|x| <= 0.99999`
/// (the root's argument is then at least `2e-5`). Guarded like a
/// speculated division: LLVM may evaluate it for any `x`, and a PSP traps
/// the square root of a negative number (RE-469); in range the guards
/// change nothing.
#[inline(never)]
fn arc_sin_ratio(x: f32) -> f32 {
    div_nonzero(x, sqrt(1.0 - x * x))
}

/// `syUtilsArcSin` (`sys/utils.c:102`): `±π/2` beyond `±0.99999`, else
/// `syUtilsArcTan(x / sqrtf(1 - x²))`.
pub fn sy_arc_sin(x: f32) -> f32 {
    if x > ARC_SAT {
        HALF_PI
    } else if x < -ARC_SAT {
        -HALF_PI
    } else {
        sy_arc_tan(arc_sin_ratio(x))
    }
}

/// `syUtilsArcCos` (`sys/utils.c:115`): 0 above `0.99999`, π below
/// `-0.99999`, else `π/2 - syUtilsArcTan(x / sqrtf(1 - x²))`.
pub fn sy_arc_cos(x: f32) -> f32 {
    if x > ARC_SAT {
        0.0
    } else if x < -ARC_SAT {
        PI_F
    } else {
        HALF_PI - sy_arc_tan(arc_sin_ratio(x))
    }
}

/// `syUtilsTan` (`sys/utils.c:20`): `__sinf(x) / __cosf(x)`, one `f32`
/// division of libultra's sine and cosine (see [`sin_cos`]).
pub fn sy_tan(x: f32) -> f32 {
    let (s, c) = sin_cos(x);
    s / c
}

/// `lbCommonSin`: the original's gameplay sine. It does not call the
/// platform's trigonometry: it truncates the radian angle into 4096 steps a
/// turn and reads a 1024-entry quarter-wave table of six-decimal words.
/// The fighters' collision matrices (`gmCollisionTransformMatrixAll`), the
/// battle camera and others read it; its truncation moves a joint chain's
/// end by up to a unit against exact sine (RE-468).
pub fn lb_sin(angle: f32) -> f32 {
    lb_sin_index(lb_angle_index(angle))
}

/// `lbCommonCos`: [`lb_sin`] of the angle plus a quarter turn, added in
/// radians before the truncation, as the original does.
pub fn lb_cos(angle: f32) -> f32 {
    lb_sin_index(lb_angle_index(angle + core::f32::consts::FRAC_PI_2))
}

/// `lbCommonSin` and `lbCommonCos` of one angle.
pub fn lb_sin_cos(angle: f32) -> (f32, f32) {
    (lb_sin(angle), lb_cos(angle))
}

/// `lbCommonTan`: both table words of one truncated index.
pub fn lb_tan(angle: f32) -> f32 {
    let index = lb_angle_index(angle);
    lb_sin_index(index) / lb_sin_index(index.wrapping_add(0x0400))
}

fn lb_angle_index(angle: f32) -> u16 {
    ((angle * 651.898_6) as i32 as u16) & 0x0fff
}

fn lb_sin_index(index: u16) -> f32 {
    let index = index & 0x0fff;
    let low = index & 0x03ff;
    let sample = if index & 0x0400 != 0 {
        lb_sine_sample(0x03ff - low)
    } else {
        lb_sine_sample(low)
    };
    if index & 0x0800 != 0 {
        -sample
    } else {
        sample
    }
}

/// `dLBCommonSinLookup[index]`: the sine of `index / 651.8986` (an `f32`
/// angle) rounded to six decimals, which 1010 of the 1024 words are. The
/// sine is taken in `f64`, whose rounding margin (5.7e-10 at worst) no
/// platform's error reaches, so the PSP builds the same words as the host;
/// the 14 authored exceptions are kept explicitly. The table is not copied
/// from the ROM (4 KiB of ROM data): [`LB_SINE`] is computed by the compiler
/// from this formula, because the PSP has no `f64` unit and summing the
/// series at run time cost about 2 ms per fighter a frame in the collision
/// matrices (RE-470). `lbcommon_sine_table_matches_the_rom` checks every
/// word against the ROM.
pub fn lb_sine_sample(index: u16) -> f32 {
    match LB_SINE.get(index as usize) {
        Some(&v) => v,
        None => lb_sine_compute(index),
    }
}

/// [`lb_sine_sample`] for every index of the table, evaluated at compile
/// time.
static LB_SINE: [f32; 1024] = {
    let mut table = [0.0f32; 1024];
    let mut i = 0;
    while i < 1024 {
        table[i] = lb_sine_compute(i as u16);
        i += 1;
    }
    table
};

const fn lb_sine_compute(index: u16) -> f32 {
    let exception = match index {
        355 => 0x3f04_9e99,
        372 => 0x3f0a_48b6,
        420 => 0x3f19_c1f8,
        440 => 0x3f1f_f6d3,
        500 => 0x3f31_a826,
        503 => 0x3f32_80bf,
        598 => 0x3f4b_41f2,
        628 => 0x3f52_33be,
        663 => 0x3f59_bda5,
        677 => 0x3f5c_94d5,
        722 => 0x3f65_0471,
        804 => 0x3f71_8f60,
        842 => 0x3f76_1672,
        1023 => 0x3f80_0000,
        _ => 0,
    };
    if exception != 0 {
        return f32::from_bits(exception);
    }
    let value = sin_f64((index as f32 / 651.898_6) as f64);
    (((value * 1_000_000.0 + 0.5) as u64) as f64 / 1_000_000.0) as f32
}

/// Sine of an angle in `[0, π/2]` in `f64`, summed to its 25th power
/// (the remainder is below 1e-22 there).
const fn sin_f64(x: f64) -> f64 {
    let x2 = x * x;
    let mut term = x;
    let mut sum = x;
    let mut n = 1.0;
    while n < 25.0 {
        term = -term * x2 / ((n + 1.0) * (n + 2.0));
        sum += term;
        n += 2.0;
    }
    sum
}

/// Sine without `std`: reduced to `[-π/2, π/2]` by `sin(π − x) = sin(x)`
/// and summed to its 13th power, within a few ULP of `f32::sin` (the
/// earlier `[-π, π]` series was 2e-3 out near ±π, which moved knockback,
/// RE-466).
#[cfg_attr(feature = "std", allow(dead_code))]
fn sin_poly(v: f32) -> f32 {
    use core::f32::consts::{FRAC_PI_2, PI};
    // Reduce to [-PI, PI], then fold into [-PI/2, PI/2].
    let mut x = v % (2.0 * PI);
    if x > PI {
        x -= 2.0 * PI;
    } else if x < -PI {
        x += 2.0 * PI;
    }
    if x > FRAC_PI_2 {
        x = PI - x;
    } else if x < -FRAC_PI_2 {
        x = -PI - x;
    }
    let x2 = x * x;
    x * (1.0
        - x2 / 6.0
            * (1.0
                - x2 / 20.0
                    * (1.0
                        - x2 / 42.0 * (1.0 - x2 / 72.0 * (1.0 - x2 / 110.0 * (1.0 - x2 / 156.0))))))
}

// --- LookAt quantization (FTOFRAC8) ----------------------------------------
//
// `R2.1`/T3 (RE-227). The original RSP microcode never sees a full-precision
// camera basis: the host CPU quantizes the camera's look-at `right`/`up`
// vectors to signed bytes once per frame, before any per-object model
// transform runs (`syMatrixLookAtReflectF`,
// `refs/ssb-decomp-re/src/sys/matrix.c:337-342`, via the `FTOFRAC8` macro,
// `refs/ssb-decomp-re/include/PR/gu.h:37`), and every object's own normal
// dot-products against the *quantized, then transformed* basis
// (`Interpreter::CalculateNormalDir`,
// `refs/BattleShip/libultraship/src/fast/interpreter.cpp:2610-2616`: dequantize
// by `/127`, then multiply by the modelview, then normalize).

/// Bit-exact port of the original `FTOFRAC8` macro:
/// `((int)MIN(((x) * (128.0f)), 127.0f) & 0xff)`.
///
/// Saturates positive overflow at 127 (so `+1.0` lands at 127, not the
/// `128 & 0xff` wraparound-to-zero a naive port would produce) but applies no
/// matching negative floor, so the quantized range is the asymmetric
/// `-128..=127` a single byte actually holds: `-1.0` quantizes to `-128`, not
/// `-127`. Returns the raw byte pattern the original stores; reinterpret as
/// `i8` for the signed N64 value. Only exact for inputs within the `i32`
/// range after the `* 128.0` scale (true for every normalized basis
/// component this project feeds it); Rust's `as i32` float cast saturates
/// instead of C's undefined behavior far outside that range, which this
/// project never reaches.
pub fn ftofrac8(x: f32) -> u8 {
    let y = (x * 128.0_f32).min(127.0_f32);
    (y as i32 & 0xff) as u8
}

/// Quantizes one look-at basis component through the original hardware's
/// signed-byte round trip: [`ftofrac8`] to quantize, then divide by 127 (the
/// raw-byte reconstruction RE-226 established, `(normal · LookAt) / 127`) to
/// get back a float the rest of the pipeline can use.
///
/// The `/127` reconstruction is itself asymmetric because the quantized range
/// is: `0.0` and `+1.0` round-trip exactly (`0/127`, `127/127`), but `-1.0`
/// quantizes to `-128` and reconstructs to `-128.0 / 127.0 ≈ -1.00787`, not
/// `-1.0`. This project's identity (camera-less) texgen basis only ever uses
/// `0.0`/`+1.0` components, so it is unaffected; a real rotated camera basis
/// with a negative component is not.
pub fn quantize_lookat_component(x: f32) -> f32 {
    (ftofrac8(x) as i8) as f32 / 127.0
}

/// Quantizes a full look-at basis vector component-wise. Call this *before*
/// transforming the basis by an object's model matrix, matching the source
/// order: the camera basis is quantized once, then each object's own
/// transform is applied to the already-quantized value.
pub fn quantize_lookat_basis(v: [f32; 3]) -> [f32; 3] {
    [
        quantize_lookat_component(v[0]),
        quantize_lookat_component(v[1]),
        quantize_lookat_component(v[2]),
    ]
}

/// The RSP's own `M^T * v`, normalised -- the look-at basis transform texgen
/// uses to carry a world-space look-at direction into an object's local
/// space. `columns[c]` is the object's model matrix's column `c`, upper 3x3
/// only (the translation/`w` row and column never apply to a direction).
/// `refs/BattleShip`'s `Interpreter::CalculateNormalDir`
/// (`libultraship/src/fast/interpreter.cpp:2610-2616`) is the reference:
/// dequantize, multiply by the transposed modelview, normalise. The
/// normalisation is also what makes a uniform model scale drop out, since
/// the transpose of a scaled rotation scales every row by the same factor.
/// Returns `[0.0; 3]` for a singular matrix rather than producing
/// infinities. R2.1/T4: pulled out of `meshdraw::DrawState::
/// texgen_object_basis` so it is host-testable and shared by both texgen
/// paths that call it, rather than a PSP-only inline closure.
pub fn transform_lookat_basis(columns: [[f32; 3]; 3], v: [f32; 3]) -> [f32; 3] {
    let dot = |c: [f32; 3]| c[0] * v[0] + c[1] * v[1] + c[2] * v[2];
    let r = [dot(columns[0]), dot(columns[1]), dot(columns[2])];
    let len = sqrt(r[0] * r[0] + r[1] * r[1] + r[2] * r[2]);
    if len > 1.0e-6 {
        [r[0] / len, r[1] / len, r[2] / len]
    } else {
        [0.0; 3]
    }
}

/// A billboard node's model matrix (`psp-runtime`'s billboard arm):
/// `T(pos) · B · Rz · S(sx, sy, sx)`, the chain that arm once built with
/// `sceGumTranslate`, `sceGumMultMatrix`, `sceGumRotateZ` and
/// `sceGumScale`, written out (RE-498). `B`'s columns are `right`, `up` and
/// `right × up` (the identity when `basis` is `None`), and `cos_sin` is the
/// spin's cosine and sine as `sceGumRotateZ`'s `vrot` gives them.
///
/// Every term the chain multiplies by an exact 0 or 1 is dropped, which
/// leaves each entry with the roundings the chain gives it on a `vmmul.q`
/// that sums its products in order (PPSSPP's, RE-497): one product and sum
/// per rotated column, one product per scaled entry. Only the sign of a
/// zero entry can differ.
pub fn billboard_matrix(
    pos: [f32; 3],
    basis: Option<([f32; 3], [f32; 3])>,
    cos_sin: (f32, f32),
    sx: f32,
    sy: f32,
) -> Mat4 {
    let (right, up) = basis.unwrap_or(([1.0, 0.0, 0.0], [0.0, 1.0, 0.0]));
    let forward = match basis {
        Some(_) => [
            right[1] * up[2] - right[2] * up[1],
            right[2] * up[0] - right[0] * up[2],
            right[0] * up[1] - right[1] * up[0],
        ],
        None => [0.0, 0.0, 1.0],
    };
    let (c, s) = cos_sin;
    let x = |i: usize| (right[i] * c + up[i] * s) * sx;
    let y = |i: usize| (right[i] * -s + up[i] * c) * sy;
    Mat4 {
        cols: [
            [x(0), x(1), x(2), 0.0],
            [y(0), y(1), y(2), 0.0],
            [forward[0] * sx, forward[1] * sx, forward[2] * sx, 0.0],
            [pos[0], pos[1], pos[2], 1.0],
        ],
    }
}

#[cfg(test)]
mod trig_tests {
    use super::sin_poly;

    /// RE-470: the compile-time sine table holds what the series gives at
    /// run time, word for word.
    #[test]
    fn the_compiled_sine_table_matches_the_series() {
        for i in 0..1024u16 {
            let runtime = super::lb_sine_compute(core::hint::black_box(i));
            assert_eq!(
                super::LB_SINE[i as usize].to_bits(),
                runtime.to_bits(),
                "entry {i}"
            );
        }
    }

    #[test]
    fn the_device_trig_matches_libm() {
        let mut worst_sin = 0.0f32;
        let mut worst_cos = 0.0f32;
        let mut a = -10.0f32;
        while a < 10.0 {
            worst_sin = worst_sin.max((sin_poly(a) - a.sin()).abs());
            worst_cos = worst_cos.max((sin_poly(a + core::f32::consts::FRAC_PI_2) - a.cos()).abs());
            a += 0.001;
        }
        assert!(worst_sin < 2e-6, "sin off by {worst_sin}");
        assert!(worst_cos < 2e-6, "cos off by {worst_cos}");
    }

    #[test]
    fn the_table_sine_is_exact_where_it_must_be() {
        for i in 0..1024u16 {
            let a = f64::from(i as f32 / 651.898_6);
            assert!((super::sin_f64(a) - a.sin()).abs() < 1e-14, "entry {i}");
        }
        assert_eq!(super::lb_sin(core::f32::consts::FRAC_PI_2), 1.0);
        assert_eq!(super::lb_cos(core::f32::consts::FRAC_PI_2), 0.0);
    }
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

    #[test]
    fn ftofrac8_zero_is_zero() {
        assert_eq!(ftofrac8(0.0), 0);
    }

    #[test]
    fn ftofrac8_positive_saturates_at_127() {
        // Exactly +1.0 would scale to 128, which wraps to -128 through the
        // `& 0xff` mask without the `min(127.0)` saturation -- the case the
        // macro's `MIN` exists to prevent.
        assert_eq!(ftofrac8(1.0), 127);
        assert_eq!(ftofrac8(2.0), 127);
        assert_eq!(ftofrac8(10.0), 127);
    }

    #[test]
    fn ftofrac8_negative_range_is_asymmetric() {
        // No matching negative floor: -1.0 reaches the full -128, one past
        // positive's +127 ceiling -- a real signed byte's actual range.
        assert_eq!(ftofrac8(-1.0) as i8, -128);
    }

    #[test]
    fn ftofrac8_negative_overflow_wraps_instead_of_saturating() {
        // The macro's `MIN` only guards positive overflow. A magnitude far
        // outside any real normalized-basis component (never reached by this
        // project, but the macro's own documented behavior) truncate-casts
        // to a value whose low byte happens to mask to zero rather than
        // saturating to -128 -- the "negative cast/range behavior" the
        // acceptance text calls out, reproduced exactly rather than guarded
        // against.
        assert_eq!(ftofrac8(-2.0) as i8, 0);
    }

    #[test]
    fn ftofrac8_quantum_boundaries() {
        // +-1/128 is exactly one quantization step.
        assert_eq!(ftofrac8(1.0 / 128.0), 1);
        assert_eq!(ftofrac8(-1.0 / 128.0) as i8, -1);
        // Half a step short of the next integer truncates toward zero, not
        // round-to-nearest -- `(int)` cast semantics, not `round()`.
        assert_eq!(ftofrac8(1.49 / 128.0), 1);
        assert_eq!(ftofrac8(-1.49 / 128.0) as i8, -1);
    }

    #[test]
    fn quantize_lookat_component_round_trips_zero_and_positive_one_exactly() {
        // `0.0` and `+1.0` survive the round trip losslessly (`0/127`,
        // `127/127`). This is what keeps the identity (camera-less) texgen
        // basis -- `([1,0,0], [0,1,0])`, no negative components -- bit-
        // identical after this fix.
        assert_eq!(quantize_lookat_component(1.0), 1.0);
        assert_eq!(quantize_lookat_component(0.0), 0.0);
    }

    #[test]
    fn quantize_lookat_component_negative_one_does_not_round_trip_exactly() {
        // The asymmetric quantized range (-128..=127) means -1.0 quantizes
        // to -128, not -127, so reconstruction (`/127`) overshoots to
        // slightly past -1.0. A real rotated camera basis can carry negative
        // components and is not bit-identical through this quantization --
        // only the identity default (never negative) is.
        let q = quantize_lookat_component(-1.0);
        assert_ne!(q, -1.0);
        assert!(close(q, -128.0 / 127.0));
    }

    #[test]
    fn quantize_lookat_basis_diverges_from_full_float_at_a_realistic_angle() {
        // A camera basis rotated 45 degrees: components of magnitude
        // 1/sqrt(2), a normal angle a real rotating camera reaches every
        // frame. The quantized value visibly differs from the full-float
        // input -- evidence the two paths are not accidentally identical --
        // while staying within one quantization step (1/127).
        let c = core::f32::consts::FRAC_1_SQRT_2;
        let basis = [c, c, 0.0];
        let quantized = quantize_lookat_basis(basis);
        assert_ne!(quantized[0], basis[0]);
        assert_ne!(quantized[1], basis[1]);
        assert_eq!(quantized[2], 0.0);
        assert!((quantized[0] - basis[0]).abs() < 1.0 / 127.0);
        assert!((quantized[1] - basis[1]).abs() < 1.0 / 127.0);
    }

    const IDENTITY_COLUMNS: [[f32; 3]; 3] = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

    #[test]
    fn transform_lookat_basis_identity_is_a_no_op() {
        let v = [0.6, -0.8, 0.0];
        let r = transform_lookat_basis(IDENTITY_COLUMNS, v);
        assert!(
            close(r[0], 0.6) && close(r[1], -0.8) && close(r[2], 0.0),
            "{r:?}"
        );
    }

    #[test]
    fn transform_lookat_basis_rotation_carries_the_direction_along() {
        // A node rotated 90 degrees about Z: object-space X maps onto
        // world-space Y, so the world-space X look-at direction should come
        // back as the node's own -Y (M^T, not M) in object space.
        let columns = [[0.0, 1.0, 0.0], [-1.0, 0.0, 0.0], [0.0, 0.0, 1.0]];
        let r = transform_lookat_basis(columns, [1.0, 0.0, 0.0]);
        assert!(
            close(r[0], 0.0) && close(r[1], -1.0) && close(r[2], 0.0),
            "{r:?}"
        );
    }

    #[test]
    fn transform_lookat_basis_uniform_scale_drops_out() {
        let scaled = [[5.0, 0.0, 0.0], [0.0, 5.0, 0.0], [0.0, 0.0, 5.0]];
        let r = transform_lookat_basis(scaled, [1.0, 0.0, 0.0]);
        assert!(
            close(r[0], 1.0) && close(r[1], 0.0) && close(r[2], 0.0),
            "{r:?}"
        );
    }

    #[test]
    fn transform_lookat_basis_singular_matrix_is_zero_not_nan() {
        let singular = [[0.0; 3]; 3];
        let r = transform_lookat_basis(singular, [1.0, 0.0, 0.0]);
        assert_eq!(r, [0.0; 3]);
    }

    /// RE-498: [`billboard_matrix`] equals the `sceGum*` chain it replaced,
    /// replayed as PPSSPP's `vmmul.q` and `vscl.t` compute it (products
    /// summed in order from 0, which [`Mat4::multiply`] does), bit for bit
    /// apart from the sign of a zero.
    #[test]
    fn billboard_matrix_matches_the_gum_chain() {
        fn gum_chain(
            pos: [f32; 3],
            basis: Option<([f32; 3], [f32; 3])>,
            (c, s): (f32, f32),
            sx: f32,
            sy: f32,
        ) -> Mat4 {
            // sceGumLoadIdentity, sceGumTranslate.
            let mut m =
                Mat4::IDENTITY.multiply(&Mat4::from_translation(Vec3::new(pos[0], pos[1], pos[2])));
            // sceGumMultMatrix with the camera basis.
            if let Some((r, u)) = basis {
                let f = [
                    r[1] * u[2] - r[2] * u[1],
                    r[2] * u[0] - r[0] * u[2],
                    r[0] * u[1] - r[1] * u[0],
                ];
                let b = Mat4 {
                    cols: [
                        [r[0], r[1], r[2], 0.0],
                        [u[0], u[1], u[2], 0.0],
                        [f[0], f[1], f[2], 0.0],
                        [0.0, 0.0, 0.0, 1.0],
                    ],
                };
                m = m.multiply(&b);
            }
            // sceGumRotateZ: `vrot` columns [C, S, 0, 0] and [-S, C, 0, 0].
            let rz = Mat4 {
                cols: [
                    [c, s, 0.0, 0.0],
                    [-s, c, 0.0, 0.0],
                    [0.0, 0.0, 1.0, 0.0],
                    [0.0, 0.0, 0.0, 1.0],
                ],
            };
            m = m.multiply(&rz);
            // sceGumScale: `vscl.t` on the first three columns.
            for (col, k) in [(0, sx), (1, sy), (2, sx)] {
                for r in 0..3 {
                    m.cols[col][r] *= k;
                }
            }
            m
        }
        // A fixed xorshift, so a failure reproduces.
        let mut state = 0x9E37_79B9u32;
        let mut next = move || {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            state
        };
        let mut unit = move || (next() >> 8) as f32 / (1u32 << 24) as f32 * 2.0 - 1.0;
        let mut checked = 0;
        for i in 0..200_000 {
            let pos = [unit() * 4000.0, unit() * 4000.0, unit() * 4000.0];
            let basis = match i % 3 {
                0 => None,
                _ => {
                    let r = Vec3::new(unit(), unit(), unit()).normalized();
                    let u = Vec3::new(unit(), unit(), unit()).normalized();
                    Some((r.to_array(), u.to_array()))
                }
            };
            let cos_sin = if i % 4 == 0 {
                (1.0, 0.0)
            } else {
                let a = unit() * 7.0;
                (a.cos(), a.sin())
            };
            let (sx, sy) = (unit() * 30.0, unit() * 30.0);
            let want = gum_chain(pos, basis, cos_sin, sx, sy);
            let got = billboard_matrix(pos, basis, cos_sin, sx, sy);
            for c in 0..4 {
                for r in 0..4 {
                    let (w, g) = (want.cols[c][r], got.cols[c][r]);
                    assert!(
                        w == g && !w.is_nan(),
                        "case {i} col {c} row {r}: chain {w:e} ({:#x}), direct {g:e} ({:#x})",
                        w.to_bits(),
                        g.to_bits()
                    );
                    checked += 1;
                }
            }
        }
        assert_eq!(checked, 200_000 * 16);
    }
}
