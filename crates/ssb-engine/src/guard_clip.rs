//! Homogeneous triangle clipping against a guard band and the near plane.
//!
//! The N64's RSP clips every triangle to the view volume before the RDP
//! rasterises it. The PSP's GE does not: it drops a whole triangle when a
//! vertex in front of the near plane projects outside its 4096-pixel screen
//! range. A floor quad that runs under the camera (the opening room's floor
//! and rug, the standoff's ground grid) therefore vanishes on the PSP where
//! the N64 draws its visible part. A renderer clips such triangles itself
//! before submission, to planes the GE's range cannot reach: `|x| <= g * w`,
//! `|y| <= g * w` and the near plane `z >= -w` of an OpenGL-style
//! projection. The clipped edges lie off screen or on the near plane, so
//! what reaches the screen is what the RSP's own clip draws.
//!
//! Clip coordinates and every attribute are interpolated linearly along the
//! clipped edges, which is exact in homogeneous space: the rasteriser's
//! perspective-correct interpolation then reproduces the unclipped
//! triangle's values at every pixel.

/// Attributes carried through a clip: any per-vertex quantity a caller
/// needs linearly interpolated (texture coordinates, colour, normal and the
/// object-space position the rasteriser transforms again).
pub const ATTRS: usize = 12;

/// The most vertices one triangle can become: one more per clipping plane.
pub const MAX_CLIPPED: usize = 3 + PLANES;

const PLANES: usize = 5;

/// A vertex to clip: its clip-space position and its attributes.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vertex {
    pub clip: [f32; 4],
    pub attrs: [f32; ATTRS],
}

/// Which clipping planes `clip` lies outside, one bit per plane:
/// `x > g w`, `x < -g w`, `y > g w`, `y < -g w`, `z < -w`.
#[inline]
pub fn outcode(clip: [f32; 4], guard: f32) -> u8 {
    let mut code = 0;
    for plane in 0..PLANES {
        if distance(plane, clip, guard) < 0.0 {
            code |= 1 << plane;
        }
    }
    code
}

/// Signed distance of `c` from `plane`, non-negative inside.
#[inline]
fn distance(plane: usize, c: [f32; 4], guard: f32) -> f32 {
    let gw = guard * c[3];
    match plane {
        0 => gw - c[0],
        1 => gw + c[0],
        2 => gw - c[1],
        3 => gw + c[1],
        _ => c[2] + c[3],
    }
}

fn lerp(a: &Vertex, b: &Vertex, t: f32) -> Vertex {
    let mut v = *a;
    for i in 0..4 {
        v.clip[i] = a.clip[i] + (b.clip[i] - a.clip[i]) * t;
    }
    for i in 0..ATTRS {
        v.attrs[i] = a.attrs[i] + (b.attrs[i] - a.attrs[i]) * t;
    }
    v
}

/// Clips triangle `tri` to the guard band `guard` (in units of `w`) and the
/// near plane, writing the resulting convex polygon to `out` in `tri`'s
/// winding. Returns its vertex count: 0 when nothing of the triangle is
/// inside, otherwise at least 3, to draw as the fan `(0, i, i + 1)`.
pub fn clip_triangle(tri: &[Vertex; 3], guard: f32, out: &mut [Vertex; MAX_CLIPPED]) -> usize {
    let codes = tri.map(|v| outcode(v.clip, guard));
    if codes[0] & codes[1] & codes[2] != 0 {
        return 0;
    }
    let union = codes[0] | codes[1] | codes[2];
    let mut a = [Vertex::default(); MAX_CLIPPED];
    a[..3].copy_from_slice(tri);
    let mut n = 3;
    let mut b = [Vertex::default(); MAX_CLIPPED];
    for plane in 0..PLANES {
        if union & (1 << plane) == 0 {
            continue;
        }
        let mut m = 0;
        for i in 0..n {
            let cur = &a[i];
            let next = &a[(i + 1) % n];
            let dc = distance(plane, cur.clip, guard);
            let dn = distance(plane, next.clip, guard);
            if dc >= 0.0 {
                b[m] = *cur;
                m += 1;
            }
            if (dc >= 0.0) != (dn >= 0.0) {
                b[m] = lerp(cur, next, dc / (dc - dn));
                m += 1;
            }
        }
        a = b;
        n = m;
        if n < 3 {
            return 0;
        }
    }
    out[..n].copy_from_slice(&a[..n]);
    n
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(clip: [f32; 4], u: f32) -> Vertex {
        let mut attrs = [0.0; ATTRS];
        attrs[0] = u;
        Vertex { clip, attrs }
    }

    #[test]
    fn a_triangle_inside_the_guard_band_is_unchanged() {
        let tri = [
            v([0.0, 0.0, 0.0, 1.0], 0.0),
            v([1.0, 0.0, 0.0, 1.0], 1.0),
            v([0.0, 1.0, 0.5, 2.0], 2.0),
        ];
        let mut out = [Vertex::default(); MAX_CLIPPED];
        assert_eq!(clip_triangle(&tri, 2.0, &mut out), 3);
        assert_eq!(out[..3], tri);
    }

    #[test]
    fn a_triangle_wholly_past_one_plane_is_dropped() {
        let tri = [
            v([5.0, 0.0, 0.0, 1.0], 0.0),
            v([6.0, 1.0, 0.0, 1.0], 0.0),
            v([7.0, -1.0, 0.0, 1.0], 0.0),
        ];
        let mut out = [Vertex::default(); MAX_CLIPPED];
        assert_eq!(clip_triangle(&tri, 2.0, &mut out), 0);
    }

    #[test]
    fn the_near_plane_cuts_a_triangle_running_behind_the_camera() {
        // One vertex in front (w 10), two behind the near plane (z < -w).
        let tri = [
            v([0.0, 0.0, 8.0, 10.0], 0.0),
            v([0.0, -1.0, -3.0, 1.0], 1.0),
            v([1.0, -1.0, -3.0, 1.0], 1.0),
        ];
        let mut out = [Vertex::default(); MAX_CLIPPED];
        let n = clip_triangle(&tri, 2.0, &mut out);
        assert_eq!(n, 3);
        for p in &out[..n] {
            assert!(p.clip[2] + p.clip[3] >= -1e-4, "{:?}", p.clip);
            assert!(p.clip[0].abs() <= 2.0 * p.clip[3] + 1e-4);
        }
        // The first new vertex is (18 / 20) of the way to vertex 1.
        let t = 18.0 / 20.0;
        assert!((out[1].attrs[0] - t).abs() < 1e-5);
        assert!((out[1].clip[2] + out[1].clip[3]).abs() < 1e-4);
    }

    #[test]
    fn a_side_vertex_past_the_guard_band_becomes_two() {
        let tri = [
            v([0.0, 0.0, 0.0, 1.0], 0.0),
            v([10.0, 0.0, 0.0, 1.0], 10.0),
            v([0.0, 1.0, 0.0, 1.0], 0.0),
        ];
        let mut out = [Vertex::default(); MAX_CLIPPED];
        let n = clip_triangle(&tri, 2.0, &mut out);
        assert_eq!(n, 4);
        assert!(out[..n].iter().all(|p| p.clip[0] <= 2.0 + 1e-5));
        // `u` equals `x` along all three edges, so it must after the clip.
        assert!(out[..n]
            .iter()
            .all(|p| (p.attrs[0] - p.clip[0]).abs() < 1e-5));
    }
}
