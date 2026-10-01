//! Path splines (`src/sys/interp.c`).
//!
//! An `AObjEvent32SetInterp` command hands a joint's `TraI` track a
//! `SYInterpDesc`, and from then on the track's value is a fraction of the
//! way along that path: `gcPlayDObjAnimJoint` clamps it to `[0, 1]` and
//! writes `syInterpCubic(desc, value)` to the joint's translation. Sector
//! Z's Arwing flies every one of its patterns this way (file 153), and its
//! controller reads the path's tangent (`syInterpQuad`) to face along it.
//!
//! ```c
//! typedef struct SYInterpDesc {
//!     u8 kind;            // 0x00: SYInterpKind
//!     s16 points_num;     // 0x02
//!     f32 unk04;          // 0x04: the Catmull-Rom tension
//!     Vec3f *points;      // 0x08
//!     f32 length;         // 0x0C: the path's arc length
//!     f32 *keyframes;     // 0x10: each segment's start, as a fraction
//!     f32 *quartics;      // 0x14: five coefficients per segment
//! } SYInterpDesc;
//! ```
//!
//! The value is an arc-length fraction: `syInterpGetFracFrame` finds the
//! segment whose keyframe range holds it, then bisects for the parameter
//! whose Simpson-rule arc length (the square root of the segment's speed
//! quartic) matches. Every expression below keeps the source's order of
//! single-precision operations.

/// `SYInterpKind`.
pub const KIND_LINEAR: u8 = 0;
pub const KIND_BEZIER_S3: u8 = 1;
pub const KIND_BEZIER: u8 = 2;
pub const KIND_CATROM: u8 = 3;

type V3 = [f32; 3];

fn u32_at(data: &[u8], at: usize) -> Option<u32> {
    let b = data.get(at..at + 4)?;
    Some(u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
}

fn f32_at(data: &[u8], at: usize) -> Option<f32> {
    u32_at(data, at).map(f32::from_bits)
}

/// One `SYInterpDesc`, its pointers already file offsets.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Spline {
    pub kind: u8,
    pub points_num: i16,
    /// `unk04`.
    pub tension: f32,
    pub points: u32,
    pub length: f32,
    pub keyframes: u32,
    pub quartics: u32,
}

impl Spline {
    /// Reads the descriptor at `at` in `data`.
    pub fn read(data: &[u8], at: u32) -> Option<Self> {
        let at = at as usize;
        let kind = *data.get(at)?;
        let n = data.get(at + 2..at + 4)?;
        Some(Spline {
            kind,
            points_num: i16::from_be_bytes([n[0], n[1]]),
            tension: f32_at(data, at + 4)?,
            points: u32_at(data, at + 8)?,
            length: f32_at(data, at + 12)?,
            keyframes: u32_at(data, at + 16)?,
            quartics: u32_at(data, at + 20)?,
        })
    }

    fn point(&self, data: &[u8], i: i32) -> Option<V3> {
        let at = (self.points as i64 + i64::from(i) * 12) as usize;
        Some([
            f32_at(data, at)?,
            f32_at(data, at + 4)?,
            f32_at(data, at + 8)?,
        ])
    }

    fn points4(&self, data: &[u8], first: i32) -> Option<[V3; 4]> {
        Some([
            self.point(data, first)?,
            self.point(data, first + 1)?,
            self.point(data, first + 2)?,
            self.point(data, first + 3)?,
        ])
    }

    fn keyframe(&self, data: &[u8], i: usize) -> Option<f32> {
        f32_at(data, self.keyframes as usize + i * 4)
    }

    fn quartic(&self, data: &[u8], segment: usize) -> Option<[f32; 5]> {
        let at = self.quartics as usize + segment * 20;
        Some([
            f32_at(data, at)?,
            f32_at(data, at + 4)?,
            f32_at(data, at + 8)?,
            f32_at(data, at + 12)?,
            f32_at(data, at + 16)?,
        ])
    }

    /// `syInterpCubic`: the point a fraction `t` of the way along the
    /// path. `None` for a fraction outside `[0, 1]`, which leaves the
    /// source's output untouched, or for a descriptor that leaves the file.
    pub fn cubic(&self, data: &[u8], t: f32) -> Option<V3> {
        let t = self.frac_frame(data, t)?;
        self.cubic_time_frac(data, t)
    }

    /// `syInterpQuad`: the path's derivative at fraction `t` (unnormalised).
    pub fn quad(&self, data: &[u8], t: f32) -> Option<V3> {
        let t = self.frac_frame(data, t)?;
        self.quad_time_frac(data, t)
    }

    /// `syInterpCubicSplineTimeFrac`.
    fn cubic_time_frac(&self, data: &[u8], t: f32) -> Option<V3> {
        if !(0.0..=1.0).contains(&t) {
            return None;
        }
        if t < 1.0 {
            let t = t * f32::from(self.points_num - 1);
            let frame = t as i16 as i32;
            let t = t - frame as f32;
            match self.kind {
                KIND_LINEAR => {
                    let p0 = self.point(data, frame)?;
                    let p1 = self.point(data, frame + 1)?;
                    Some(core::array::from_fn(|k| (p1[k] - p0[k]) * t + p0[k]))
                }
                KIND_BEZIER_S3 => Some(cubic_bezier_scale(&self.points4(data, frame * 3)?, t)),
                KIND_BEZIER => Some(bezier3_points(&self.points4(data, frame)?, t)),
                KIND_CATROM => Some(catrom_cubic(&self.points4(data, frame)?, self.tension, t)),
                _ => None,
            }
        } else {
            let frame = i32::from(self.points_num - 1);
            match self.kind {
                KIND_LINEAR => self.point(data, frame),
                KIND_BEZIER_S3 => self.point(data, frame * 3),
                KIND_BEZIER => Some(bezier3_points(&self.points4(data, frame - 1)?, 1.0)),
                KIND_CATROM => self.point(data, frame + 1),
                _ => None,
            }
        }
    }

    /// `syInterpQuadSplineTimeFrac`.
    fn quad_time_frac(&self, data: &[u8], t: f32) -> Option<V3> {
        if !(0.0..=1.0).contains(&t) {
            return None;
        }
        let t_origin = t;
        let t = t * f32::from(self.points_num - 1);
        let mut frame = t as i16 as i32;
        let t = t - frame as f32;
        match self.kind {
            KIND_LINEAR => {
                if t_origin == 1.0 {
                    frame -= 1;
                }
                let p0 = self.point(data, frame)?;
                let p1 = self.point(data, frame + 1)?;
                Some(core::array::from_fn(|k| p1[k] - p0[k]))
            }
            KIND_BEZIER_S3 => Some(quad_bezier4_points(&self.points4(data, frame * 3)?, t)),
            KIND_BEZIER => Some(bezier4_points(&self.points4(data, frame)?, t)),
            KIND_CATROM => Some(quad_spline(&self.points4(data, frame)?, self.tension, t)),
            _ => None,
        }
    }

    /// `syInterpGetFracFrame`: the spline parameter (over the whole path)
    /// at arc-length fraction `t`.
    fn frac_frame(&self, data: &[u8], t: f32) -> Option<f32> {
        // The source walks the keyframes with no bound; a fraction past the
        // last keyframe would read past the table. The clamped `TraI` value
        // never is, so the walk stops at the last segment instead.
        let segments = (self.points_num as usize).checked_sub(1)?;
        let mut id = 0usize;
        while id + 1 < segments && self.keyframe(data, id + 1)? < t {
            id += 1;
        }
        let k0 = self.keyframe(data, id)?;
        let frac = match self.kind {
            KIND_LINEAR => (t - k0) / (self.keyframe(data, id + 1)? - k0),
            KIND_BEZIER_S3 | KIND_BEZIER | KIND_CATROM => {
                let cof = self.quartic(data, id)?;
                let mut time_scale = (t - k0) * self.length;
                let mut min = 0.0f32;
                let mut max = 1.0f32;
                let mut frac;
                // Each pass halves [min, max], so the 1e-5 width break ends
                // it within 17; the cap only guards a non-finite input.
                let mut passes = 0;
                loop {
                    frac = (min + max) / 2.0;
                    let res = cubic_integral_approx(min, frac, &cof);
                    if time_scale < res + 0.00001 {
                        max = frac;
                    } else {
                        min = frac;
                        time_scale -= res;
                    }
                    let diff = if min < max { -(min - max) } else { min - max };
                    passes += 1;
                    if diff < 0.00001 || passes >= 64 {
                        break;
                    }
                    if !((res + 0.00001) < time_scale || time_scale < (res - 0.00001)) {
                        break;
                    }
                }
                frac
            }
            _ => return None,
        };
        Some((id as f32 + frac) / (f32::from(self.points_num) - 1.0))
    }
}

/// `syInterpGetQuartSum`.
fn quart_sum(x: f32, cof: &[f32; 5]) -> f32 {
    let mut sum =
        cof[0] * (x * x * x * x) + cof[1] * (x * x * x) + cof[2] * (x * x) + cof[3] * x + cof[4];
    if sum < 0.0 && sum > -0.001 {
        sum = 0.0;
    }
    libm::sqrtf(sum)
}

/// `syInterpGetCubicIntegralApprox`: Simpson's rule over eight intervals.
fn cubic_integral_approx(t: f32, f: f32, cof: &[f32; 5]) -> f32 {
    let factor = (f - t) / 8.0;
    let mut sum = 0.0f32;
    let mut time_scale = t + factor;
    for i in 2..9 {
        if i & 1 == 0 {
            sum += 4.0 * quart_sum(time_scale, cof);
        } else {
            sum += 2.0 * quart_sum(time_scale, cof);
        }
        time_scale += factor;
    }
    ((quart_sum(t, cof) + sum + quart_sum(f, cof)) * factor) / 3.0
}

fn weigh(c: &[V3; 4], w: [f32; 4]) -> V3 {
    core::array::from_fn(|k| c[0][k] * w[0] + c[1][k] * w[1] + c[2][k] * w[2] + c[3][k] * w[3])
}

/// `syInterpCatromCubicSpline`.
fn catrom_cubic(c: &[V3; 4], s: f32, t: f32) -> V3 {
    let sqt = t * t;
    let cbt = sqt * t;
    let w0 = (2.0 * sqt - cbt - t) * s;
    let w1 = (2.0 - s) * cbt + (s - 3.0) * sqt + 1.0;
    let w2 = (s - 2.0) * cbt + (3.0 - 2.0 * s) * sqt + s * t;
    let w3 = (cbt - sqt) * s;
    weigh(c, [w0, w1, w2, w3])
}

/// `syInterpQuadSpline`: the Catmull-Rom spline's derivative.
fn quad_spline(c: &[V3; 4], s: f32, t: f32) -> V3 {
    let sqt = t * t;
    let w0 = (((-3.0) * sqt + 4.0 * t) - 1.0) * s;
    let temp = s - 3.0;
    let w1 = ((2.0 - s) * 3.0) * sqt + (2.0 * temp) * t;
    let temp = 3.0 - 2.0 * s;
    let w2 = (((s - 2.0) * 3.0) * sqt + (2.0 * temp) * t) + s;
    let w3 = (3.0 * sqt - 2.0 * t) * s;
    weigh(c, [w0, w1, w2, w3])
}

/// `syInterpBezier3Points`: the uniform cubic B-spline.
fn bezier3_points(c: &[V3; 4], t: f32) -> V3 {
    let subt = 1.0 - t;
    let sqt = t * t;
    let cbt = sqt * t;
    let w0 = (1.0 / 6.0) * subt * subt * subt;
    let w1 = (1.0 / 6.0) * (3.0 * cbt - 6.0 * sqt + 4.0);
    let w2 = (1.0 / 6.0) * (3.0 * (sqt - cbt + t) + 1.0);
    let w3 = (1.0 / 6.0) * cbt;
    weigh(c, [w0, w1, w2, w3])
}

/// `syInterpBezier4Points`: the B-spline's derivative.
fn bezier4_points(c: &[V3; 4], t: f32) -> V3 {
    let sqt = t * t;
    let w0 = 1.0 - t;
    let w3 = -0.5 * w0 * w0;
    let mt = (3.0 * sqt - 4.0 * t) * 0.5;
    let w1 = ((-3.0 * sqt) + 2.0 * t + 1.0) * 0.5;
    let w2 = 0.5 * sqt;
    weigh(c, [w3, mt, w1, w2])
}

/// `syInterpCubicBezierScale`.
fn cubic_bezier_scale(c: &[V3; 4], t: f32) -> V3 {
    let subt = 1.0 - t;
    let sqt = t * t;
    let sqsubt = subt * subt;
    weigh(
        c,
        [sqsubt * subt, 3.0 * t * sqsubt, 3.0 * sqt * subt, sqt * t],
    )
}

/// `syInterpQuadBezier4Points`: the Bezier's derivative.
fn quad_bezier4_points(c: &[V3; 4], t: f32) -> V3 {
    let mt = t - 1.0;
    let w3 = -3.0 * mt * mt;
    let w0 = 3.0 * (t * t);
    let w1 = ((1.0 - 4.0 * t) + w0) * 3.0;
    let w2 = (2.0 * t - w0) * 3.0;
    weigh(c, [w3, w1, w2, w0])
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A straight Catmull-Rom path along X through 0, 100, 200 with the
    /// two end handles, one segment's quartic each.
    fn line() -> (alloc::vec::Vec<u8>, Spline) {
        let mut data = alloc::vec::Vec::new();
        let push = |d: &mut alloc::vec::Vec<u8>, v: f32| d.extend_from_slice(&v.to_be_bytes());
        // Points at 0x00: -100, 0, 100, 200, 300 on X.
        for x in [-100.0, 0.0, 100.0, 200.0, 300.0] {
            push(&mut data, x);
            push(&mut data, 0.0);
            push(&mut data, 0.0);
        }
        // Keyframes at 0x3C.
        for k in [0.0, 0.5, 1.0] {
            push(&mut data, k);
        }
        // Quartics at 0x48: a constant speed of 100 (tension 0.5 makes the
        // uniform Catmull-Rom derivative exactly the point spacing).
        for _ in 0..2 {
            for c in [0.0, 0.0, 0.0, 0.0, 10000.0] {
                push(&mut data, c);
            }
        }
        let spline = Spline {
            kind: KIND_CATROM,
            points_num: 3,
            tension: 0.5,
            points: 0,
            length: 200.0,
            keyframes: 0x3C,
            quartics: 0x48,
        };
        (data, spline)
    }

    #[test]
    fn a_uniform_path_maps_fraction_to_distance() {
        let (data, s) = line();
        for (t, x) in [
            (0.0, 0.0),
            (0.1, 20.0),
            (0.4, 80.0),
            (0.6, 120.0),
            (0.9, 180.0),
        ] {
            let p = s.cubic(&data, t).unwrap();
            assert!((p[0] - x).abs() < 0.01, "t {t}: {p:?}");
        }
        let d = s.quad(&data, 0.3).unwrap();
        assert!((d[0] - 100.0).abs() < 0.01 && d[1] == 0.0, "{d:?}");
    }

    /// The bisection's `do ... while` tests the length left *after* taking
    /// the half it just measured. On a uniform segment, a fraction at the
    /// segment's end leaves exactly that half again, and the loop exits at
    /// the midpoint: the source's quirk, kept.
    #[test]
    fn a_tie_after_the_first_half_stops_at_the_midpoint() {
        let (data, s) = line();
        let p = s.cubic(&data, 0.5).unwrap();
        assert!((p[0] - 50.0).abs() < 0.01, "{p:?}");
        let p = s.cubic(&data, 1.0).unwrap();
        assert!((p[0] - 150.0).abs() < 0.01, "{p:?}");
    }

    #[test]
    fn a_fraction_outside_the_path_writes_nothing() {
        let (data, s) = line();
        assert_eq!(s.cubic_time_frac(&data, 1.5), None);
        assert_eq!(s.cubic_time_frac(&data, -0.1), None);
    }

    #[test]
    fn the_descriptor_reads_its_six_fields() {
        let mut d = alloc::vec![3, 0, 0, 19];
        for w in [0.5f32.to_bits(), 0x700, 2647.66f32.to_bits(), 0x7FC, 0x848] {
            d.extend_from_slice(&w.to_be_bytes());
        }
        let s = Spline::read(&d, 0).unwrap();
        assert_eq!((s.kind, s.points_num, s.points), (3, 19, 0x700));
        assert_eq!((s.keyframes, s.quartics, s.tension), (0x7FC, 0x848, 0.5));
    }
}
