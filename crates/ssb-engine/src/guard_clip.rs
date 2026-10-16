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
/// `x > g w`, `x < -g w`, `y > g w`, `y < -g w`, `z < -w`. Each bit is
/// [`distance`]'s own subtraction or sum compared with zero, written out
/// once with `g w` shared, so the result is the per-plane test's bit for bit.
#[inline]
pub fn outcode(clip: [f32; 4], guard: f32) -> u8 {
    let gw = guard * clip[3];
    u8::from(gw - clip[0] < 0.0)
        | u8::from(gw + clip[0] < 0.0) << 1
        | u8::from(gw - clip[1] < 0.0) << 2
        | u8::from(gw + clip[1] < 0.0) << 3
        | u8::from(clip[2] + clip[3] < 0.0) << 4
}

/// `mvp * (p, 1)` for a column-major `mvp` (`mvp[column][row]`): the clip
/// position every outcode and clip of a submission is taken from.
#[inline(always)]
pub fn clip_position(mvp: &[[f32; 4]; 4], p: [f32; 3]) -> [f32; 4] {
    core::array::from_fn(|r| mvp[0][r] * p[0] + mvp[1][r] * p[1] + mvp[2][r] * p[2] + mvp[3][r])
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
    // Two polygons, each plane reading one and writing the other: only the
    // live vertices are ever read, so nothing is copied between planes.
    let mut polys = [[Vertex::default(); MAX_CLIPPED]; 2];
    polys[0][..3].copy_from_slice(tri);
    let mut n = 3;
    let mut src = 0;
    for plane in 0..PLANES {
        if union & (1 << plane) == 0 {
            continue;
        }
        let (first, second) = polys.split_at_mut(1);
        let (a, b) = if src == 0 {
            (&first[0], &mut second[0])
        } else {
            (&second[0], &mut first[0])
        };
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
        src ^= 1;
        n = m;
        if n < 3 {
            return 0;
        }
    }
    out[..n].copy_from_slice(&polys[src][..n]);
    n
}

/// How many of a list's `tris` triangles cross the guard band or the near
/// plane: some corner outside a plane, but not all three outside the same
/// one. `code(c)` is corner `c`'s [`outcode`]. A list with none draws as it
/// is; [`clip_triangles`] rebuilds one with any.
#[inline]
pub fn crossing_triangles(tris: usize, code: impl Fn(usize) -> u8) -> usize {
    let mut crossing = 0;
    for t in 0..tris {
        let (a, b, c) = (code(t * 3), code(t * 3 + 1), code(t * 3 + 2));
        if a | b | c != 0 && a & b & c == 0 {
            crossing += 1;
        }
    }
    crossing
}

/// A triangle list's `tris` triangles clipped in order, each emitted as
/// the fan of [`clip_triangle`]'s polygon: what clipping every triangle
/// with [`clip_triangle`] gives, without its work where the result is
/// already known. A triangle inside every plane is its own three corners,
/// one wholly outside a plane is dropped, and only a crossing one is
/// clipped.
///
/// `code(c)` is corner `c`'s [`outcode`] of `clip(&corner(c))`;
/// `corner(c)` decodes its attributes, called once per corner kept;
/// `emit` receives each output vertex's attributes. Returns the number
/// emitted.
pub fn clip_triangles(
    tris: usize,
    guard: f32,
    code: impl Fn(usize) -> u8,
    mut corner: impl FnMut(usize) -> [f32; ATTRS],
    clip: impl Fn(&[f32; ATTRS]) -> [f32; 4],
    mut emit: impl FnMut(&[f32; ATTRS]),
) -> usize {
    let mut poly = [Vertex::default(); MAX_CLIPPED];
    let mut emitted = 0;
    for t in 0..tris {
        let codes = [code(t * 3), code(t * 3 + 1), code(t * 3 + 2)];
        if codes[0] & codes[1] & codes[2] != 0 {
            continue;
        }
        let attrs = [corner(t * 3), corner(t * 3 + 1), corner(t * 3 + 2)];
        if codes[0] | codes[1] | codes[2] == 0 {
            attrs.iter().for_each(&mut emit);
            emitted += 3;
            continue;
        }
        let tri = attrs.map(|attrs| Vertex {
            clip: clip(&attrs),
            attrs,
        });
        let count = clip_triangle(&tri, guard, &mut poly);
        for i in 1..count.saturating_sub(1) {
            for v in [&poly[0], &poly[i], &poly[i + 1]] {
                emit(&v.attrs);
            }
            emitted += 3;
        }
    }
    emitted
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(clip: [f32; 4], u: f32) -> Vertex {
        let mut attrs = [0.0; ATTRS];
        attrs[0] = u;
        Vertex { clip, attrs }
    }

    /// The per-plane outcode `clip_submission` used before the fast path.
    fn outcode_reference(clip: [f32; 4], guard: f32) -> u8 {
        let mut code = 0;
        for plane in 0..PLANES {
            if distance(plane, clip, guard) < 0.0 {
                code |= 1 << plane;
            }
        }
        code
    }

    /// `clip_triangle` before the fast path: whole polygons copied per plane.
    fn clip_triangle_reference(
        tri: &[Vertex; 3],
        guard: f32,
        out: &mut [Vertex; MAX_CLIPPED],
    ) -> usize {
        let codes = tri.map(|v| outcode_reference(v.clip, guard));
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

    /// `psp-runtime`'s `clip_submission` before the fast path: every corner
    /// decoded to take its outcode, then every triangle of a list with a
    /// crossing one re-emitted through `clip_triangle`. `None` when no
    /// triangle crosses.
    fn clip_submission_reference(
        mvp: &[[f32; 4]; 4],
        corner: impl Fn(usize) -> [f32; ATTRS],
        count: usize,
    ) -> Option<Vec<[f32; ATTRS]>> {
        let corner = |i: usize| {
            let attrs = corner(i);
            let clip = clip_position(mvp, [attrs[9], attrs[10], attrs[11]]);
            Vertex { clip, attrs }
        };
        let tris = count / 3;
        let mut clipped = 0usize;
        for t in 0..tris {
            let mut union = 0;
            let mut inter = 0xFF;
            for k in 0..3 {
                let code = outcode_reference(corner(t * 3 + k).clip, GUARD);
                union |= code;
                inter &= code;
            }
            if union != 0 && inter == 0 {
                clipped += 1;
            }
        }
        if clipped == 0 {
            return None;
        }
        let mut out = Vec::new();
        let mut poly = [Vertex::default(); MAX_CLIPPED];
        for t in 0..tris {
            let tri = [corner(t * 3), corner(t * 3 + 1), corner(t * 3 + 2)];
            let count = clip_triangle_reference(&tri, GUARD, &mut poly);
            for i in 1..count.saturating_sub(1) {
                for v in [&poly[0], &poly[i], &poly[i + 1]] {
                    out.push(v.attrs);
                }
            }
        }
        Some(out)
    }

    const GUARD: f32 = 2.0;
    const DIVISOR: f32 = 32768.0;

    /// xorshift32: deterministic test data without a dependency.
    struct Rng(u32);

    impl Rng {
        fn next(&mut self) -> u32 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 17;
            self.0 ^= self.0 << 5;
            self.0
        }

        fn unit(&mut self) -> f32 {
            (self.next() >> 8) as f32 / (1u32 << 24) as f32 * 2.0 - 1.0
        }
    }

    /// One packed vertex: `u`, `v`, colour bytes, normal, position.
    #[derive(Clone, Copy)]
    struct Packed {
        uv: [u16; 2],
        color: [u8; 4],
        normal: [i8; 3],
        pos: [i16; 3],
    }

    /// The attributes `decode_corner` reads from a packed vertex.
    fn decode(v: &Packed) -> [f32; ATTRS] {
        [
            f32::from(v.uv[0]) / DIVISOR,
            f32::from(v.uv[1]) / DIVISOR,
            f32::from(v.color[0]),
            f32::from(v.color[1]),
            f32::from(v.color[2]),
            f32::from(v.color[3]),
            f32::from(v.normal[0]),
            f32::from(v.normal[1]),
            f32::from(v.normal[2]),
            f32::from(v.pos[0]) / DIVISOR,
            f32::from(v.pos[1]) / DIVISOR,
            f32::from(v.pos[2]) / DIVISOR,
        ]
    }

    /// A camera-like `projection * view * model`: perspective with `w` the
    /// depth in front of the eye, so vertices fall behind the near plane and
    /// past the guard band as well as inside.
    fn random_mvp(rng: &mut Rng) -> [[f32; 4]; 4] {
        let mut m = [[0.0f32; 4]; 4];
        for column in m.iter_mut() {
            for e in column.iter_mut() {
                *e = rng.unit() * 4.0;
            }
        }
        // Keep `w` mostly positive near the mesh: an eye in front of it.
        m[3][3] = rng.unit().abs() * 2.0 + 0.1;
        m
    }

    /// A projection whose planes meet 16-bit positions exactly: `x`, `y` at
    /// `4 p`, `w` 1, so `p = 0.5` lies on the guard band, and `z` at `p`, so
    /// `p = -1` would lie on the near plane (`-0.5` with `z = 2 p`).
    fn exact_mvp() -> [[f32; 4]; 4] {
        [
            [4.0, 0.0, 0.0, 0.0],
            [0.0, 4.0, 0.0, 0.0],
            [0.0, 0.0, 2.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ]
    }

    fn random_vertex(rng: &mut Rng, exact: bool) -> Packed {
        let pos = if exact {
            // On, just inside and just outside each plane.
            const EDGE: [i16; 7] = [16384, 16383, 16385, -16384, -16383, -16385, 0];
            [0, 1, 2].map(|_| EDGE[rng.next() as usize % EDGE.len()])
        } else {
            [0, 1, 2].map(|_| rng.next() as u16 as i16)
        };
        Packed {
            uv: [rng.next() as u16, rng.next() as u16],
            color: rng.next().to_le_bytes(),
            normal: [rng.next() as i8, rng.next() as i8, rng.next() as i8],
            pos,
        }
    }

    /// The fast path (`crossing_triangles` and `clip_triangles` over a
    /// per-vertex outcode table read through the index list) emits exactly
    /// the vertices the old `clip_submission` did, bit for bit, and makes
    /// the same rebuild decision, on random and plane-exact meshes.
    #[test]
    fn the_outcode_table_path_matches_the_old_clip_submission_bit_for_bit() {
        let mut rng = Rng(0x5EED_1234);
        let mut rebuilt = 0;
        let mut drawn = 0;
        for case in 0..2000 {
            let exact = case % 4 == 3;
            let mvp = if exact {
                exact_mvp()
            } else {
                random_mvp(&mut rng)
            };
            let pool: Vec<Packed> = (0..1 + rng.next() as usize % 48)
                .map(|_| random_vertex(&mut rng, exact))
                .collect();
            let tris = 1 + rng.next() as usize % 24;
            let indices: Vec<usize> = (0..tris * 3)
                .map(|_| rng.next() as usize % pool.len())
                .collect();

            // The per-vertex table `draw_mesh_vertices` builds once per mesh.
            let table: Vec<u8> = pool
                .iter()
                .map(|v| {
                    outcode(
                        clip_position(&mvp, v.pos.map(|c| f32::from(c) / DIVISOR)),
                        GUARD,
                    )
                })
                .collect();
            for v in &pool {
                let clip = clip_position(&mvp, v.pos.map(|c| f32::from(c) / DIVISOR));
                assert_eq!(outcode(clip, GUARD), outcode_reference(clip, GUARD));
            }

            let corner = |c: usize| decode(&pool[indices[c]]);
            let old = clip_submission_reference(&mvp, corner, tris * 3);
            let code = |c: usize| table[indices[c]];
            let new = (crossing_triangles(tris, code) != 0).then(|| {
                let mut out = Vec::new();
                let n = clip_triangles(
                    tris,
                    GUARD,
                    code,
                    corner,
                    |a| clip_position(&mvp, [a[9], a[10], a[11]]),
                    |a| out.push(*a),
                );
                assert_eq!(n, out.len());
                out
            });
            let bits = |o: &Option<Vec<[f32; ATTRS]>>| {
                o.as_ref()
                    .map(|v| v.iter().map(|a| a.map(f32::to_bits)).collect::<Vec<_>>())
            };
            assert_eq!(bits(&old), bits(&new), "case {case}");
            if let Some(v) = &old {
                rebuilt += 1;
                drawn += v.len();
            }
        }
        // The cases exercise the rebuild, not only the packed fallback.
        assert!(rebuilt > 500 && drawn > 10_000, "{rebuilt} {drawn}");
    }

    #[test]
    fn the_ping_pong_clip_matches_the_copying_clip_bit_for_bit() {
        let mut rng = Rng(0xC11F_0001);
        for _ in 0..20_000 {
            let tri = [0, 1, 2].map(|_| Vertex {
                clip: [0, 1, 2, 3].map(|_| rng.unit() * 8.0),
                attrs: [0; ATTRS].map(|_| rng.unit() * 100.0),
            });
            let mut a = [Vertex::default(); MAX_CLIPPED];
            let mut b = [Vertex::default(); MAX_CLIPPED];
            let n = clip_triangle(&tri, GUARD, &mut a);
            assert_eq!(n, clip_triangle_reference(&tri, GUARD, &mut b));
            let bits = |p: &[Vertex]| {
                p.iter()
                    .map(|v| (v.clip.map(f32::to_bits), v.attrs.map(f32::to_bits)))
                    .collect::<Vec<_>>()
            };
            assert_eq!(bits(&a[..n]), bits(&b[..n]));
        }
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
