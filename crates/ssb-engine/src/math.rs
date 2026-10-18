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
            // Never a zero divisor, even if LLVM divides before the guard
            // (D-050).
            self * div_nonzero(1.0, sqrt(len_sq))
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

    /// `syMatrixPerspFastF` (`sys/matrix.c:575`) with `scale` 1: the
    /// projection every perspective camera but the cliff's builds, its
    /// cotangent from `gSYSinTable` ([`sy_persp_cot`], RE-505).
    pub fn sy_persp_fast(fovy_degrees: f32, aspect: f32, near: f32, far: f32) -> Mat4 {
        Self::persp_from_cot(sy_persp_cot(fovy_degrees), aspect, near, far)
    }

    /// `syMatrixPerspF` (`sys/matrix.c:645`) with `scale` 1: the cliff
    /// cameras' exact projection ([`sy_persp_cot_exact`]).
    pub fn sy_persp_f(fovy_degrees: f32, aspect: f32, near: f32, far: f32) -> Mat4 {
        Self::persp_from_cot(sy_persp_cot_exact(fovy_degrees), aspect, near, far)
    }

    /// The matrix both perspective builders write from a cotangent, in
    /// their order of operations (`scale` 1 drops out exactly). `mf[i][j]`
    /// is `cols[i][j]`.
    fn persp_from_cot(cot: f32, aspect: f32, near: f32, far: f32) -> Mat4 {
        let mut m = Mat4::ZERO;
        m.cols[0][0] = cot / aspect;
        m.cols[1][1] = cot;
        m.cols[2][2] = (near + far) / (near - far);
        m.cols[2][3] = -1.0;
        m.cols[3][2] = (2.0 * near * far) / (near - far);
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

// --- The original's sine and cosine (libultra `__sinf`/`__cosf`) -----------
//
// The game's `__sinf`, `__cosf`, `sinf` and `cosf` (the `syVectorRotate*`,
// `syMatrix*F` and particle code all call them) are libultra's
// `fsin`/`fcos` (`refs/ssb-decomp-re/src/libultra/gu/sinf.c`, `cosf.c`):
// a Cody-Waite reduction by π and a degree-9 odd polynomial, both in
// DOUBLE precision, rounded to `f32` once at the end. They are not
// correctly rounded (about one input in twenty differs from a correctly
// rounded sine), so only the same operations in the same order give the
// N64's bits. Ported operation for operation in `f64`: the R4300's
// `add.d`/`mul.d`/`cvt` are IEEE round-to-nearest with no fused
// multiply-add, as are the host's SSE2 and the PSP's software `f64`
// (compiler-builtins), and Rust never reassociates or contracts. The
// ROM's own machine code is the test oracle (`trig_golden`). No VFPU: its
// `vsin`/`vcos` work in quarter turns with their own single-precision
// approximation.

/// `fsin`/`fcos`'s polynomial `P[1..=4]` (`sinf.c:39-49`, the ROM's rodata
/// `0x8003f160`, identical); `P[0] = 1` is never read.
const SINF_P: [f64; 4] = [
    f64::from_bits(0xbfc5_5554_bc83_656d),
    f64::from_bits(0x3f81_10ed_3804_c2a0),
    f64::from_bits(0xbf29_f6ff_eea5_6814),
    f64::from_bits(0x3ec5_dbdf_0e31_4bfe),
];
/// `rpi`: 1/π.
const SINF_RPI: f64 = f64::from_bits(0x3fd4_5f30_6dc9_c883);
/// `pihi`: π's leading 25 significant bits, so `n * pihi` is exact.
const SINF_PIHI: f64 = f64::from_bits(0x4009_21fb_5000_0000);
/// `pilo`: the rest of π.
const SINF_PILO: f64 = f64::from_bits(0x3e61_10b4_611a_6263);
/// `__libm_qnan_f`, what both return for a NaN.
const SINF_QNAN: u32 = 0x7f81_0000;

/// `xpt` of `sinf.c`/`cosf.c`: the exponent and the leading mantissa bit,
/// sign dropped. `0xff` is `|x| = 1.5`, `0xe6` is `2^-12`, `0x136` is `2^28`.
#[inline(always)]
fn sinf_xpt(x: f32) -> u32 {
    (x.to_bits() >> 22) & 0x1ff
}

/// `x` is a NaN, tested on the bits: a compare would raise the invalid
/// operation a PSP traps on a signalling NaN (RE-469).
#[inline(always)]
fn sinf_is_nan(x: f32) -> bool {
    x.to_bits() & 0x7fff_ffff > 0x7f80_0000
}

/// `dx + (dx * xsq) * poly`, the sine of a reduced argument, as `fsin` and
/// `fcos` both write it (Horner from `P[4]`).
#[inline(always)]
fn sinf_poly(dx: f64) -> f64 {
    let [p1, p2, p3, p4] = SINF_P;
    let xsq = dx * dx;
    let poly = ((p4 * xsq + p3) * xsq + p2) * xsq + p1;
    dx + (dx * xsq) * poly
}

/// `ROUND` (`PR/guint.h:39`): half away from zero, then C's truncating
/// `(int)`. `|d| < 2^27` here, so the cast never saturates.
#[inline(always)]
fn sinf_round(d: f64) -> i32 {
    (if d >= 0.0 { d + 0.5 } else { d - 0.5 }) as i32
}

/// libultra's `__sinf` (`fsin`) in `f64`, the reference [`sinf`] falls
/// back to.
#[cold]
#[inline(never)]
fn sinf_f64(x: f32) -> f32 {
    let xpt = sinf_xpt(x);
    if xpt < 0xff {
        if xpt >= 0xe6 {
            return sinf_poly(x as f64) as f32;
        }
        return x;
    }
    if xpt < 0x136 {
        let dx = x as f64;
        let n = sinf_round(dx * SINF_RPI);
        let dn = n as f64;
        let dx = dx - dn * SINF_PIHI;
        let dx = dx - dn * SINF_PILO;
        let result = sinf_poly(dx) as f32;
        return if n & 1 == 0 { result } else { -result };
    }
    if sinf_is_nan(x) {
        return f32::from_bits(SINF_QNAN);
    }
    0.0
}

/// libultra's `__cosf` (`fcos`) in `f64`, the reference [`cosf`] falls
/// back to. Not `sin(x + π/2)`: it reduces `|x|` by `(n - 1/2)π`.
#[cold]
#[inline(never)]
fn cosf_f64(x: f32) -> f32 {
    if sinf_xpt(x) < 0x136 {
        // `ABS(x)`: `x > 0 ? x : -x`, in `f32` (so `+0` becomes `-0`, which
        // the reduction below cannot tell apart).
        let absx = if x > 0.0 { x } else { -x };
        let dx = absx as f64;
        let n = sinf_round(dx * SINF_RPI + 0.5);
        let dn = n as f64 - 0.5;
        let dx = dx - dn * SINF_PIHI;
        let dx = dx - dn * SINF_PILO;
        let result = sinf_poly(dx) as f32;
        return if n & 1 == 0 { result } else { -result };
    }
    if sinf_is_nan(x) {
        return f32::from_bits(SINF_QNAN);
    }
    0.0
}

// --- The same bits, faster: double-single arithmetic ------------------------
//
// The PSP has no `f64` hardware: [`sinf_f64`] costs about 8 us there
// against about 1 us for a single-precision sine. The fast path below
// evaluates the same reduction and polynomial in double-single arithmetic
// (each value an unevaluated sum `hi + lo` of two `f32`s, about 48
// significant bits; Dekker's exact product and Knuth's exact sum), then
// rounds to `f32` only when the result is clear of a rounding boundary by a
// margin of 2^-40 relative, and of every other decision `f64` makes (the
// quotient `n`, the small-argument cut) by 2^-20. Otherwise it returns
// `None` and the caller takes the `f64` path, about 1 call in 20,000 (or
// `|x| >= 65536`, which no game angle reaches). `f32` adds and multiplies
// are IEEE round-to-nearest on the host, the PSP's FPU and every other
// target (`trig_golden::HASHES_ARITH`); none is fused. Proof: an
// exhaustive test over all 2^32 inputs (`sinf_ds_matches_f64_everywhere`,
// run on demand) finds no input where a `Some` differs from the `f64`
// result, also at margins 16 times smaller.

/// A double-single number: the unevaluated sum `.0 + .1`, `|.1| <= ulp(.0)/2`.
#[derive(Clone, Copy)]
struct Ds(f32, f32);

/// `a + b` exactly, as a double-single (Knuth's two-sum).
#[inline(always)]
fn ds_two_sum(a: f32, b: f32) -> Ds {
    let s = a + b;
    let bb = s - a;
    Ds(s, (a - (s - bb)) + (b - bb))
}

/// `a + b` exactly when `|a| >= |b|` (Dekker's fast two-sum).
#[inline(always)]
fn ds_fast_two_sum(a: f32, b: f32) -> Ds {
    let s = a + b;
    Ds(s, b - (s - a))
}

/// `a` as `hi + lo`, each with at most 12 significant bits (Veltkamp).
#[inline(always)]
fn ds_split(a: f32) -> (f32, f32) {
    let c = 4097.0 * a;
    let h = c - (c - a);
    (h, a - h)
}

/// `a * b` exactly, as a double-single (Dekker's two-product).
#[inline(always)]
fn ds_two_prod(a: f32, b: f32) -> Ds {
    let p = a * b;
    let (ah, al) = ds_split(a);
    let (bh, bl) = ds_split(b);
    Ds(p, ((ah * bh - p) + ah * bl + al * bh) + al * bl)
}

#[inline(always)]
fn ds_mul(a: Ds, b: Ds) -> Ds {
    let p = ds_two_prod(a.0, b.0);
    ds_fast_two_sum(p.0, p.1 + (a.0 * b.1 + a.1 * b.0))
}

#[inline(always)]
fn ds_add(a: Ds, b: Ds) -> Ds {
    let s = ds_two_sum(a.0, b.0);
    ds_fast_two_sum(s.0, s.1 + (a.1 + b.1))
}

#[inline(always)]
fn ds_add_f(a: Ds, b: f32) -> Ds {
    let s = ds_two_sum(a.0, b);
    ds_fast_two_sum(s.0, s.1 + a.1)
}

/// The double-single nearest `v`.
const fn ds_of(v: f64) -> Ds {
    let h = v as f32;
    Ds(h, (v - h as f64) as f32)
}

const DS_P: [Ds; 4] = [
    ds_of(SINF_P[0]),
    ds_of(SINF_P[1]),
    ds_of(SINF_P[2]),
    ds_of(SINF_P[3]),
];
const DS_RPI: Ds = ds_of(SINF_RPI);
/// `pihi` in two `f32`s of 24 and 1 significant bits: `dn * DS_PI_A` is
/// exact as a two-product, `dn * DS_PI_B` exact in `f32` (`|dn| < 2^15`).
const DS_PI_A: f32 = SINF_PIHI as f32;
const DS_PI_B: f32 = (SINF_PIHI - DS_PI_A as f64) as f32;
const DS_PILO: Ds = ds_of(SINF_PILO);
/// `|x| < 65536`: the fast path's domain.
const DS_LIMIT_BITS: u32 = 0x4780_0000;
/// How close the fraction of `x/π` may come to where `f64`'s rounding of
/// `n` could go either way.
const DS_MARGIN_N: f32 = 1.0 / 1_048_576.0;
/// How close (relative) the result may come to an `f32` rounding boundary.
const DS_MARGIN: f32 = 1.0 / 1_099_511_627_776.0;

/// `sinf_poly` in double-single.
#[inline(always)]
fn ds_poly(d: Ds) -> Ds {
    let xsq = ds_mul(d, d);
    let mut p = ds_add(ds_mul(DS_P[3], xsq), DS_P[2]);
    p = ds_add(ds_mul(p, xsq), DS_P[1]);
    p = ds_add(ds_mul(p, xsq), DS_P[0]);
    ds_add(d, ds_mul(ds_mul(d, xsq), p))
}

/// `r` rounded to `f32`, or `None` when `r` is within [`DS_MARGIN`] of a
/// rounding boundary (where `r`'s own error could decide the rounding).
#[inline(always)]
fn ds_round(r: Ds) -> Option<f32> {
    let v = r.0 + r.1;
    let e = (r.0 - v) + r.1;
    let bits = v.to_bits() & 0x7fff_ffff;
    // Half an ulp of `v`; toward zero from a power of two, a quarter.
    let half = f32::from_bits((bits & 0x7f80_0000) - (24 << 23));
    let toward_zero = (e < 0.0) == (v > 0.0);
    let boundary = if bits & 0x007f_ffff == 0 && toward_zero {
        half * 0.5
    } else {
        half
    };
    if (e.abs() - boundary).abs() <= f32::from_bits(bits) * DS_MARGIN {
        return None;
    }
    Some(v)
}

/// `x - dn * pihi - dn * pilo` as a double-single (`dn` is an integer or a
/// half-integer, exact in `f32`).
#[inline(always)]
fn ds_reduce(x: f32, dn: f32) -> Ds {
    let h = ds_two_prod(dn, DS_PI_A);
    let s = ds_two_sum(x, -h.0);
    let s = ds_add_f(s, -h.1);
    let s = ds_add_f(s, -(dn * DS_PI_B));
    let lo = ds_mul(DS_PILO, Ds(dn, 0.0));
    ds_add(s, Ds(-lo.0, -lo.1))
}

/// `(floor(p), p - floor(p))` of a double-single `p`, `|p| < 2^21`.
#[inline(always)]
fn ds_floor_frac(p: Ds) -> (f32, f32) {
    let k = (p.0 as i32 - (p.0 < 0.0) as i32) as f32;
    let k = if k > p.0 { k - 1.0 } else { k };
    (k, (p.0 - k) + p.1)
}

/// [`sinf_f64`]'s bits, or `None` (see above).
#[inline(always)]
fn sinf_ds(x: f32) -> Option<f32> {
    let xpt = sinf_xpt(x);
    if xpt < 0xff {
        if xpt >= 0xe6 {
            return ds_round(ds_poly(Ds(x, 0.0)));
        }
        return Some(x);
    }
    if x.to_bits() & 0x7fff_ffff >= DS_LIMIT_BITS {
        return None;
    }
    // `n = ROUND(x/π)`, half away from zero; the margin keeps clear of halves.
    let (k, t) = ds_floor_frac(ds_mul(Ds(x, 0.0), DS_RPI));
    if (t - 0.5).abs() <= DS_MARGIN_N {
        return None;
    }
    let n = if t > 0.5 { k + 1.0 } else { k };
    let d = ds_reduce(x, n);
    if d.0.abs() < 1.0 / 4096.0 {
        return None;
    }
    let r = ds_round(ds_poly(d))?;
    Some(if n as i32 & 1 == 0 { r } else { -r })
}

/// [`cosf_f64`]'s bits, or `None` (see above).
#[inline(always)]
fn cosf_ds(x: f32) -> Option<f32> {
    if x.to_bits() & 0x7fff_ffff >= DS_LIMIT_BITS {
        return None;
    }
    let a = if x > 0.0 { x } else { -x };
    // `n = ROUND(|x|/π + 1/2)`: clear of integers in `|x|/π`, except that
    // `|x|/π < 1` always gives `n = 1`.
    let (k, t) = ds_floor_frac(ds_mul(Ds(a, 0.0), DS_RPI));
    if (t <= DS_MARGIN_N && k >= 1.0) || t >= 1.0 - DS_MARGIN_N {
        return None;
    }
    let n = k + 1.0;
    let d = ds_reduce(a, n - 0.5);
    if d.0.abs() < 1.0 / 4096.0 {
        return None;
    }
    let r = ds_round(ds_poly(d))?;
    Some(if n as i32 & 1 == 0 { r } else { -r })
}

/// libultra's `__sinf` (= `sinf`, `fsin`; `sinf.c:69-156`, ROM
/// `0x800303f0`), bit for bit. `|x| < 2^-12` returns `x` itself (signed
/// zeros and denormals included); `|x| >= 2^28` and infinities return `+0`;
/// a NaN returns the quiet NaN `0x7f810000`.
#[inline(never)]
pub fn sinf(x: f32) -> f32 {
    match sinf_ds(x) {
        Some(v) => v,
        None => sinf_f64(x),
    }
}

/// libultra's `__cosf` (= `cosf`, `fcos`; `cosf.c:61-126`, ROM
/// `0x80035cd0`), bit for bit. `|x| >= 2^28` and infinities return `+0`; a
/// NaN returns `0x7f810000`.
#[inline(never)]
pub fn cosf(x: f32) -> f32 {
    match cosf_ds(x) {
        Some(v) => v,
        None => cosf_f64(x),
    }
}

/// `(__sinf(v), __cosf(v))`: libultra's sine and cosine of the same angle
/// (radians), the N64's bits on every target. Public, like [`sqrt`], so
/// game code needs no `libm` dependency of its own.
#[inline]
pub fn sin_cos(v: f32) -> (f32, f32) {
    (sinf(v), cosf(v))
}

/// Whether `opt` holds a value, and that value or `default`, with every
/// float written before any use.
///
/// A PSP traps float operations on a signalling NaN (D-050), and LLVM may
/// compute a compare or a sum on an `Option`'s payload before the tag test
/// that guards it. A `None`'s payload is unwritten stack or a copy of it,
/// so that garbage can trap (RE-531). A plain `unwrap_or(default)` is not
/// enough: when the operation folds on a constant `default`, LLVM moves it
/// back onto the raw payload. The barrier keeps the chosen value opaque.
#[inline(always)]
pub fn defined_or<T: Copy>(opt: Option<T>, default: T) -> (bool, T) {
    (opt.is_some(), core::hint::black_box(opt.unwrap_or(default)))
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

/// `n / d` with the IEEE 754 result for a zero `d` -- `±inf` by the signs
/// of `n` and `d`, NaN for `0 / 0` or a NaN `n` -- without ever dividing by
/// zero.
///
/// The N64 masks divide-by-zero (`osInitialize` and `osCreateThread` set
/// the FCSR to `FPCSR_FS | FPCSR_EV`: only invalid operations trap), so the
/// original's `x / 0` is the IEEE infinity. A PSP under PSPLink traps the
/// division instead (FCR31 `0x00000E00`, RE-502), so the port reproduces the
/// value without executing it. The division only ever sees a nonzero
/// divisor, so LLVM may hoist it past the branch freely. Only `d == 0` is
/// special: `inf / inf` still divides.
#[inline]
pub fn div_ieee(n: f32, d: f32) -> f32 {
    let q = n / if d == 0.0 { 1.0 } else { d };
    if d != 0.0 {
        q
    } else if n == 0.0 || n.is_nan() {
        f32::NAN
    } else if n.is_sign_negative() != d.is_sign_negative() {
        f32::NEG_INFINITY
    } else {
        f32::INFINITY
    }
}

/// `x as i32` -- truncation toward zero, saturating at `i32::MIN`/`i32::MAX`,
/// NaN to 0 -- computed on the bits, so no FPU conversion ever sees an
/// out-of-range value.
///
/// Rust's `as` gives the same result, but on MIPS it runs `trunc.w.s` first
/// and fixes the result up after: `trunc.w.s` of NaN, `±inf` or anything
/// outside `[-2^31, 2^31)` raises the FPU's invalid operation, which a PSP
/// under PSPLink traps (RE-469). The N64 never converts such a value either:
/// the R4300 raises its unmaskable unimplemented-operation exception there.
/// For every in-range input the result is `trunc.w.s`'s.
#[inline]
pub fn f32_to_i32_sat(x: f32) -> i32 {
    let bits = x.to_bits();
    let exponent = ((bits >> 23) & 0xFF) as i32;
    if exponent == 0xFF && bits & 0x007F_FFFF != 0 {
        return 0;
    }
    if exponent < 127 {
        // |x| < 1, zeros and denormals included.
        return 0;
    }
    let negative = bits >> 31 != 0;
    let e = exponent - 127;
    if e >= 31 {
        // |x| >= 2^31, infinities included; -2^31 itself is i32::MIN.
        return if negative { i32::MIN } else { i32::MAX };
    }
    let mantissa = (bits & 0x007F_FFFF) | 0x0080_0000;
    let magnitude = if e >= 23 {
        mantissa << (e - 23)
    } else {
        mantissa >> (23 - e)
    } as i32;
    if negative {
        -magnitude
    } else {
        magnitude
    }
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

/// `gSYSinTable` (`sys/sintable.c`): `u16[2048]`, the sine over `[0, PI)`
/// scaled by 32768, read from the user's ROM at build time (`build.rs`,
/// `ssb-tablegen`). Hand-authored data, not a formula: rounding
/// `sin(i * PI / 2048) * 32768` misses most entries (RE-505).
mod sy_sin_table {
    include!(concat!(env!("OUT_DIR"), "/sin_table.rs"));
}

/// `gSYSinTable` by entry (0..2048).
pub fn sy_sin_table_entry(index: usize) -> u16 {
    sy_sin_table::SY_SIN_TABLE[index]
}

/// `syGetSinCosUShort` (`lb/lbdef.h:85`): the sine and cosine words of a
/// radian angle, as the original's integers (`-32768..=32768`, the table's
/// own scale, not 1.0).
///
/// The angle becomes `(s32)(angle * (2048 / PI)) & 0xFFF` (truncated toward
/// zero, a negative angle wraps by the mask); the sine is
/// `gSYSinTable[id & 0x7FF]`, negated when bit 0x800 is set, and the cosine
/// the same read at `id + 0x400`.
pub fn sy_sin_cos_ushort(angle: f32) -> (i32, i32) {
    let id = (angle * 651.898_6) as i32 as u32 & 0xFFF;
    let word = |id: u32| {
        let v = i32::from(sy_sin_table::SY_SIN_TABLE[(id & 0x7FF) as usize]);
        if id & 0x800 != 0 {
            -v
        } else {
            v
        }
    };
    (word(id), word(id + 0x400))
}

/// `syMatrixPerspFastF`'s cotangent (`sys/matrix.c:575`) of a vertical field
/// of view in degrees: `fovy *= 0.008726646F`, then the table's cosine
/// over its sine, both converted to `f32` first. This is the original's
/// `1 / tan(fovy / 2)` quantised to 4096 steps a turn (0.088 degrees).
///
/// The original divides by a sine of zero (a zero field of view) to
/// infinity; a PSP traps that (RE-469), so this divides by 1 there. The
/// camera never has one.
pub fn sy_persp_cot(fovy_degrees: f32) -> f32 {
    let (sin, cos) = sy_sin_cos_ushort(fovy_degrees * 0.008_726_646);
    div_nonzero(cos as f32, sin as f32)
}

/// `DTOR32` (`include/macros.h`): `(float)(PI64 / 180)`.
const DTOR32: f32 = (core::f64::consts::PI / 180.0) as f32;

/// `syMatrixPerspF`'s cotangent (`sys/matrix.c:645`): `fovy *= DTOR32`,
/// then libultra's `cosf(fovy / 2) / sinf(fovy / 2)`, unquantised. Only
/// the opening's cliff cameras (`mvOpeningCliffMakeMainCamera`) have this
/// kind; every other perspective camera is `PerspFastF` ([`sy_persp_cot`]).
/// A zero field of view divides by 1, as [`sy_persp_cot`] does.
pub fn sy_persp_cot_exact(fovy_degrees: f32) -> f32 {
    let fovy = fovy_degrees * DTOR32;
    div_nonzero(cosf(fovy / 2.0), sinf(fovy / 2.0))
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
/// angle) rounded to six decimals, for the 1024 steps of a quarter turn.
/// The table is generated at build time from the ROM by `ssb-tablegen`
/// (D-048) and never committed. `lb_sin_index` folds every angle into
/// `0..1024`, so no caller indexes past the table; an index past its end
/// reads 0.0.
pub fn lb_sine_sample(index: u16) -> f32 {
    LB_SINE.get(index as usize).copied().unwrap_or(0.0)
}

/// The generated table, kept in one place in `.rodata`.
static LB_SINE: [f32; 1024] = crate::d_arrays::LB_COMMON_SIN_LOOKUP;

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
        // Out of line: LLVM may divide before the branch (RE-469).
        r.map(|c| div_nonzero(c, len))
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
    /// `fsin`/`fcos` outside the ROM oracle's inputs (`trig_golden`): the
    /// paths the R4300 would trap on or never computes.
    #[test]
    fn libultra_sine_and_cosine_edge_cases() {
        use super::{cosf, sinf};
        let qnan = 0x7f81_0000;
        for nan in [0x7fc0_0000u32, 0x7f80_0001, 0xffc0_0000, 0xff80_0001] {
            assert_eq!(sinf(f32::from_bits(nan)).to_bits(), qnan);
            assert_eq!(cosf(f32::from_bits(nan)).to_bits(), qnan);
        }
        for big in [
            f32::INFINITY,
            f32::NEG_INFINITY,
            f32::MAX,
            268_435_456.0,
            -268_435_456.0,
        ] {
            assert_eq!(sinf(big).to_bits(), 0);
            assert_eq!(cosf(big).to_bits(), 0);
        }
        // Tiny inputs pass straight through `fsin`, sign and denormals too.
        for tiny in [0u32, 0x8000_0000, 1, 0x8000_0001, 0x007f_ffff, 0x397f_ffff] {
            assert_eq!(sinf(f32::from_bits(tiny)).to_bits(), tiny);
        }
        assert_eq!(cosf(0.0), 1.0);
        assert_eq!(cosf(-0.0), 1.0);
        assert_eq!(cosf(f32::from_bits(1)), 1.0);
        // Even in `x`, as `fcos` takes `|x|` first.
        for i in 0..4096 {
            let x = i as f32 * 0.37;
            assert_eq!(cosf(x).to_bits(), cosf(-x).to_bits(), "{x}");
            assert_eq!(sinf(x).to_bits(), (-sinf(-x)).to_bits(), "{x}");
        }
        // Not correctly rounded: some sweep inputs differ from the host's
        // sine (about 5% of them), which the ROM hashes pin.
        let differ = (0..4096)
            .filter(|&i| {
                let x = i as f32 * 0.37;
                sinf(x) != x.sin()
            })
            .count();
        assert!(differ > 50, "{differ}");
    }

    /// Whether the fast path's answer for `bits`, if it has one, is the
    /// `f64` port's: `(sin ok, sin fell back, cos ok, cos fell back)`.
    fn ds_check(bits: u32) -> (bool, bool, bool, bool) {
        use super::{cosf_ds, cosf_f64, sinf_ds, sinf_f64};
        let x = f32::from_bits(bits);
        let s = sinf_ds(x);
        let c = cosf_ds(x);
        (
            s.is_none_or(|v| v.to_bits() == sinf_f64(x).to_bits()),
            s.is_none(),
            c.is_none_or(|v| v.to_bits() == cosf_f64(x).to_bits()),
            c.is_none(),
        )
    }

    #[test]
    fn sinf_ds_matches_f64_on_a_sample() {
        // Every 4093rd bit pattern, and the dense neighbourhoods of the
        // multiples of π/2 the reduction is hardest at.
        let near = (-200i32..=200).flat_map(|k| {
            let c = (k as f32 * core::f32::consts::FRAC_PI_2).to_bits();
            (0..512u32).map(move |d| c.wrapping_add(d).wrapping_sub(256))
        });
        for bits in near {
            let (s_ok, _, c_ok, _) = ds_check(bits);
            assert!(s_ok && c_ok, "{bits:#010x}");
        }
        // Over the stride, which is not concentrated where `sin` or `cos`
        // is near 0, the fast path almost never gives up.
        let mut fell_back = 0;
        let mut n = 0;
        for bits in (0..=u32::MAX).step_by(4093) {
            let (s_ok, s_fb, c_ok, c_fb) = ds_check(bits);
            assert!(s_ok && c_ok, "{bits:#010x}");
            fell_back += usize::from(s_fb && bits & 0x7fff_ffff < super::DS_LIMIT_BITS);
            fell_back += usize::from(c_fb && bits & 0x7fff_ffff < super::DS_LIMIT_BITS);
            n += 2;
        }
        assert!(fell_back * 1000 < n, "{fell_back} of {n}");
    }

    /// All 2^32 inputs; about 30 s on 24 threads in release:
    /// `cargo test --release -p ssb-engine -- --ignored sinf_ds_matches_f64_everywhere --nocapture`.
    #[test]
    #[ignore = "exhaustive: run on demand, in release"]
    fn sinf_ds_matches_f64_everywhere() {
        let threads = std::thread::available_parallelism().map_or(4, |n| n.get()) as u64;
        let total = 1u64 << 32;
        let counts = std::thread::scope(|sc| {
            let handles: std::vec::Vec<_> = (0..threads)
                .map(|t| {
                    sc.spawn(move || {
                        let mut c = [0u64; 4];
                        for bits in total * t / threads..total * (t + 1) / threads {
                            let bits = bits as u32;
                            let in_range = bits & 0x7fff_ffff < super::DS_LIMIT_BITS;
                            let (s_ok, s_fb, c_ok, c_fb) = ds_check(bits);
                            c[0] += u64::from(!s_ok);
                            c[1] += u64::from(s_fb && in_range);
                            c[2] += u64::from(!c_ok);
                            c[3] += u64::from(c_fb && in_range);
                        }
                        c
                    })
                })
                .collect();
            handles.into_iter().fold([0u64; 4], |mut a, h| {
                for (a, c) in a.iter_mut().zip(h.join().unwrap()) {
                    *a += c;
                }
                a
            })
        });
        std::println!(
            "sinf: {} wrong, {} fell back; cosf: {} wrong, {} fell back (|x| < 65536)",
            counts[0],
            counts[1],
            counts[2],
            counts[3]
        );
        assert_eq!((counts[0], counts[2]), (0, 0));
    }

    #[test]
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
    fn the_table_sine_is_exact_at_a_quarter_turn() {
        assert_eq!(super::lb_sin(core::f32::consts::FRAC_PI_2), 1.0);
        assert_eq!(super::lb_cos(core::f32::consts::FRAC_PI_2), 0.0);
    }
}

#[cfg(test)]
mod sy_sin_tests {
    use super::*;

    /// `gSYSinTable` as `ssb-rom` reads it for the audio section.
    #[test]
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
    fn the_table_is_the_roms_gsysintable() {
        let Some(path) = ssb_tablegen::test_rom(env!("CARGO_MANIFEST_DIR")) else {
            return;
        };
        let rom = std::fs::read(path).unwrap();
        let (at, len) = ssb_rom::audio::SIN_TABLE;
        assert_eq!(len, 4096);
        for i in 0..2048usize {
            let word = u16::from_be_bytes([rom[at + i * 2], rom[at + i * 2 + 1]]);
            assert_eq!(sy_sin_table_entry(i), word, "entry {i}");
        }
    }

    #[test]
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
    fn the_table_has_the_shape_the_macro_relies_on() {
        assert_eq!(sy_sin_table_entry(0), 0);
        assert_eq!(sy_sin_table_entry(1), 50);
        assert_eq!(sy_sin_table_entry(1024), 0x8000);
        assert_eq!(sy_sin_table_entry(2047), 0);
        // Not a formula: round(sin * 32768) misses most entries (RE-505).
        let misses = (0..2048usize)
            .filter(|&i| {
                let v = (f64::from(i as u32) * core::f64::consts::PI / 2048.0).sin() * 32768.0;
                sy_sin_table_entry(i) != v.round() as u16
            })
            .count();
        assert!(misses > 1000, "{misses}");
    }

    #[test]
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
    fn sin_cos_words_follow_the_macro() {
        let t = |i: usize| i32::from(sy_sin_table_entry(i));
        // Index 0 and the angle just under the first step.
        assert_eq!(sy_sin_cos_ushort(0.0), (0, t(1024)));
        assert_eq!(sy_sin_cos_ushort(0.001), (0, t(1024)));
        // Index 216 (fovy 38 degrees, the battle camera).
        let a = 38.0f32 * 0.008_726_646;
        assert_eq!(sy_sin_cos_ushort(a), (t(216), t(216 + 1024)));
        // The angle in the middle of step `id`, so truncation picks `id`.
        let step = |id: f32| (id + 0.5) / 651.898_6;
        // Past a half turn the sine is negated (bit 0x800); the cosine's
        // index `id + 0x400` has the bit set too, at 0xC02.
        assert_eq!(sy_sin_cos_ushort(step(2050.0)), (-t(2), -t(1026)));
        // Id 1500: sine entry 1500 positive; cosine at 0x9DC has the bit set.
        assert_eq!(
            sy_sin_cos_ushort(step(1500.0)),
            (t(1500), -t(1500 + 1024 - 2048))
        );
        // A negative angle truncates toward zero, then wraps by the mask:
        // -1.5 steps is -1, id 0xFFF, the sine of entry 2047 negated.
        let m = sy_sin_cos_ushort(-1.5 / 651.898_6);
        assert_eq!(m.0, -t(2047));
        assert_eq!(m.1, t(1023));
    }

    #[test]
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
    fn the_cotangent_is_the_tables_cos_over_sin() {
        let (s, c) = (
            sy_sin_table_entry(216) as i32 as f32,
            sy_sin_table_entry(216 + 1024) as i32 as f32,
        );
        assert_eq!(sy_persp_cot(38.0).to_bits(), (c / s).to_bits());
        // Quantised but close to the exact cotangent.
        let exact = 1.0 / (38.0f32.to_radians() * 0.5).tan();
        assert!((sy_persp_cot(38.0) - exact).abs() / exact < 2e-3);
        // fovy 0 would divide by zero on the PSP.
        assert!(sy_persp_cot(0.0).is_finite());
    }

    #[test]
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
    fn the_fast_projection_is_sy_matrix_persp_fast_f() {
        // The battle camera: 38 degrees, near 256, far 39,936.
        let (aspect, near, far) = (300.0f32 / 220.0, 256.0f32, 39_936.0f32);
        for fovy in [38.0f32, 30.0, 18.607_187, 11.982_265, 60.0, 0.0] {
            let m = Mat4::sy_persp_fast(fovy, aspect, near, far);
            let cot = sy_persp_cot(fovy);
            assert_eq!(m.cols[0][0].to_bits(), (cot / aspect).to_bits());
            assert_eq!(m.cols[1][1].to_bits(), cot.to_bits());
            assert_eq!(
                m.cols[2][2].to_bits(),
                ((near + far) / (near - far)).to_bits()
            );
            assert_eq!(m.cols[2][3], -1.0);
            assert_eq!(
                m.cols[3][2].to_bits(),
                ((2.0 * near * far) / (near - far)).to_bits()
            );
            assert_eq!(m.cols[3][3], 0.0);
            assert!(m.as_array().iter().all(|v| v.is_finite()));
        }
    }
}

#[cfg(test)]
mod persp_f_tests {
    use super::*;

    #[test]
    fn the_exact_projection_uses_libultra_cosf_and_sinf() {
        // The cliff camera: `syMatrixPerspF`.
        let half = 38.0f32 * DTOR32 / 2.0;
        let cot = cosf(half) / sinf(half);
        assert_eq!(sy_persp_cot_exact(38.0).to_bits(), cot.to_bits());
        let m = Mat4::sy_persp_f(38.0, 4.0 / 3.0, 128.0, 16_384.0);
        assert_eq!(m.cols[1][1].to_bits(), cot.to_bits());
        assert_eq!(m.cols[0][0].to_bits(), (cot / (4.0 / 3.0)).to_bits());
        // Unquantised: within float rounding of 1 / tan(19 degrees).
        let exact = (1.0 / (19.0f64.to_radians()).tan()) as f32;
        assert!((cot - exact).abs() / exact < 1e-6);
        assert!(sy_persp_cot_exact(0.0).is_finite());
        assert_eq!(DTOR32.to_bits(), 0.017_453_292_f32.to_bits());
    }

    /// `tools/r4300/matrix_hashes.py`'s `persp_inputs`: the game's cameras,
    /// then a field-of-view sweep from 0.5 to 170 degrees in 0.37 steps.
    fn persp_inputs() -> Vec<[f32; 4]> {
        let a = (300.0f64 / 220.0) as f32;
        let mut out = vec![
            [38.0, a, 256.0, 39_936.0],
            [30.0, a, 256.0, 39_936.0],
            [18.607_187, a, 128.0, 16_384.0],
            [11.982_265, (4.0f64 / 3.0) as f32, 100.0, 15_000.0],
            [60.0, (4.0f64 / 3.0) as f32, 10.0, 12_800.0],
            [45.0, 1.0, 1.0, 4_000.0],
        ];
        let aspects = [a, (4.0f64 / 3.0) as f32, 1.0, (16.0f64 / 9.0) as f32];
        let planes = [
            (256.0, 39_936.0),
            (128.0, 16_384.0),
            (32.0, 6_400.0),
            (1.0, 1.5),
        ];
        let (mut k, mut fov) = (0usize, 0.5f64);
        while fov < 170.0 {
            let (n, f) = planes[(k / 4) % planes.len()];
            out.push([fov as f32, aspects[k % aspects.len()], n, f]);
            fov += 0.37;
            k += 1;
        }
        out
    }

    fn persp_hash(f: impl Fn(&[f32; 4]) -> Mat4) -> u64 {
        persp_inputs().iter().fold(0xcbf2_9ce4_8422_2325, |h, i| {
            let m = f(i);
            m.cols.iter().flatten().fold(h, |h, v| {
                v.to_bits().to_le_bytes().iter().fold(h, |h, &b| {
                    (h ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3)
                })
            })
        })
    }

    #[test]
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
    fn the_projections_match_the_rom() {
        // Hashes of the ROM's own `syMatrixPerspFastF` (0x8001B248) and
        // `syMatrixPerspF` (0x8001B4CC), scale 1, over 465 inputs, run by
        // `tools/r4300/matrix_hashes.py`: every f32 of `mf[i][j]`, which
        // is `cols[i][j]`.
        assert_eq!(persp_inputs().len(), 465);
        let fast = persp_hash(|i| Mat4::sy_persp_fast(i[0], i[1], i[2], i[3]));
        assert_eq!(fast, 0xec27_1c0f_303d_dfb5);
        let exact = persp_hash(|i| Mat4::sy_persp_f(i[0], i[1], i[2], i[3]));
        assert_eq!(exact, 0x4696_e72a_0b3f_7d6f);
        // A sample the script prints: the battle camera.
        let m = Mat4::sy_persp_fast(38.0, (300.0f64 / 220.0) as f32, 256.0, 39_936.0);
        assert_eq!(m.cols[0][0].to_bits(), 0x4008_5c92);
        assert_eq!(m.cols[1][1].to_bits(), 0x4039_f299);
        assert_eq!(m.cols[3][2].to_bits(), 0xc400_d368);
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

    #[test]
    fn div_ieee_gives_the_ieee_quotient_without_dividing_by_zero() {
        crate::fpu_traps::with_traps(|| {
            let z = core::hint::black_box(0.0f32);
            assert_eq!(div_ieee(255.0, z), f32::INFINITY);
            assert_eq!(div_ieee(-255.0, z), f32::NEG_INFINITY);
            assert_eq!(div_ieee(255.0, -z), f32::NEG_INFINITY);
            assert_eq!(div_ieee(-255.0, -z), f32::INFINITY);
            assert!(div_ieee(0.0, z).is_nan());
            assert!(div_ieee(-0.0, -z).is_nan());
            assert!(div_ieee(f32::NAN, z).is_nan());
            assert_eq!(div_ieee(f32::INFINITY, z), f32::INFINITY);
            assert_eq!(div_ieee(255.0 * 3.0, 4.0), 191.25);
            assert_eq!(div_ieee(-7.0, 2.0), -3.5);
        });
        // Masked, the host's own division agrees.
        for (n, d) in [(255.0f32, 0.0f32), (-1.0, 0.0), (1.0, -0.0), (3.0, 0.75)] {
            let (n, d) = core::hint::black_box((n, d));
            assert_eq!(div_ieee(n, d).to_bits(), (n / d).to_bits());
        }
    }

    #[test]
    fn f32_to_i32_sat_is_rusts_cast_without_a_trapping_conversion() {
        crate::fpu_traps::with_traps(|| {
            assert_eq!(f32_to_i32_sat(2_147_483_648.0), i32::MAX);
            assert_eq!(f32_to_i32_sat(-2_147_483_648.0), i32::MIN);
            assert_eq!(f32_to_i32_sat(-2_147_483_904.0), i32::MIN);
            assert_eq!(f32_to_i32_sat(f32::INFINITY), i32::MAX);
            assert_eq!(f32_to_i32_sat(f32::NEG_INFINITY), i32::MIN);
            assert_eq!(f32_to_i32_sat(f32::NAN), 0);
            assert_eq!(f32_to_i32_sat(-f32::NAN), 0);
            assert_eq!(f32_to_i32_sat(2_147_483_520.0), 2_147_483_520);
            assert_eq!(f32_to_i32_sat(-1.5), -1);
            assert_eq!(f32_to_i32_sat(0.99999994), 0);
            assert_eq!(f32_to_i32_sat(-0.0), 0);
        });
        // Every exponent, with a sweep of mantissas, against `as` (masked).
        let mut checked = 0u32;
        for bits in (0..=u32::MAX).step_by(4093) {
            let x = f32::from_bits(bits);
            assert_eq!(f32_to_i32_sat(x), x as i32, "{bits:#010x}");
            checked += 1;
        }
        for e in 0..=255u32 {
            for m in [0u32, 1, 0x40_0000, 0x7F_FFFF] {
                for s in [0u32, 1] {
                    let x = f32::from_bits(s << 31 | e << 23 | m);
                    assert_eq!(f32_to_i32_sat(x), x as i32, "{x:e}");
                }
            }
        }
        assert!(checked > 1_000_000);
    }
}
