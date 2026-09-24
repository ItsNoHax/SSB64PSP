//! RE-318: lowering for render tiles whose PSP bake exceeds the GE's
//! 512-texel limit.
//!
//! Every such tile in the ROM has the same shape: a small `mask` period
//! (8 to 64 texels) with `G_TX_CLAMP` set and a `G_SETTILESIZE` rect far
//! larger than the period (up to 960 texels). Real hardware repeats the
//! period across the rect and clamps only at its edges
//! ([`crate::n64_addressing::address_axis`]).
//! [`crate::texture::mirror_extend`] bakes that repeat out to the whole rect,
//! so the GE sees a texture wider or taller than it can address (RE-314).
//!
//! The rect's far edge is the only place its clamp differs from a plain
//! repeat of the period: the RDP clamps once the coordinate reaches
//! `lrs = W - 1` and then holds texel `W - 1` with a zero filter fraction.
//! Below `W - 1` the tile is exactly periodic. So each triangle of an
//! affected primitive gets one of two equivalent tiles per wide axis:
//!
//! * **Repeat**: the period alone (a mirrored pair if the axis mirrors),
//!   without the clamp bit, for triangles whose coordinates stay in
//!   `[0, W - 1)`. The GE's `Repeat` wrap then matches the RDP everywhere
//!   such a triangle samples, bilinear neighbours included.
//! * **Window**: the rect's last `W - L` texels, clamped, with the
//!   triangle's coordinates shifted down by `L`, for triangles that reach
//!   the clamp band. `L` is a multiple of the baked period, so the shift
//!   keeps the phase, and `W - L <= 512`, so the GE's `Clamp` sits exactly
//!   where the RDP's does.
//!
//! A triangle that reaches the band but also samples below `L` is split
//! along the line `coordinate == L`. Every other triangle that crosses the
//! line in the same primitive is split too, so no split edge is shared with
//! an unsplit neighbour inside the primitive.

use alloc::collections::BTreeMap;
use alloc::vec::Vec;

use crate::mesh::{Mesh, MeshVertex, Primitive, TextureRef};
use crate::pack::looks_like_unit_normal;
use crate::psp_texture::GE_MAX_TEXTURE_DIM;
use crate::texture::mirror_axis_len;

/// Why a primitive with a too-large baked axis could not be lowered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WideTileError {
    /// The wide axis is not a mask-narrowed, clamped tile with a zero origin
    /// and a period of at most 512: the only shape this lowering proves
    /// equivalent.
    UnsupportedTile { prim: usize, axis: usize },
    /// Texture coordinates are generated, not authored, so no triangle can
    /// be classified from its vertices.
    TextureGen { prim: usize },
    /// The caller could not prove the primitive's material animation leaves
    /// its UVs unchanged.
    AnimatedUv { prim: usize },
    /// A coordinate below zero reaches the rect's near clamp, which this
    /// lowering does not model. No ROM primitive does this.
    NegativeCoordinate { prim: usize, axis: usize },
    /// A split edge joins a normal-carrying vertex to a colour-carrying one.
    MixedLighting { prim: usize },
    /// More unique vertices than a `u16` index can address.
    TooManyVertices,
}

/// What the lowering did, for the pack census.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Stats {
    /// Source primitives with a wide axis.
    pub primitives: usize,
    /// Source triangles cut along a window boundary.
    pub split_triangles: usize,
    /// Primitives emitted in place of the wide ones.
    pub emitted: usize,
}

/// A mesh with every wide tile lowered.
#[derive(Debug, Clone, PartialEq)]
pub struct Lowered {
    pub mesh: Mesh,
    /// For each output primitive, the input primitive it came from.
    pub source_prim: Vec<usize>,
    pub stats: Stats,
}

/// How one axis of one triangle is addressed after lowering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AxisClass {
    /// The axis fits the GE unchanged.
    Keep,
    /// Period only, `Repeat`.
    Repeat,
    /// The rect's texels from `start` to its far edge, clamped.
    Window { start: u32 },
}

/// A wide axis: the baked period and the tile's drawn extent, in texels.
#[derive(Debug, Clone, Copy)]
struct WideAxis {
    period: u32,
    drawn: u32,
}

type Tri = [MeshVertex; 3];

/// One lowered primitive's runs: an equivalent tile and its triangles.
type Runs = Vec<(TextureRef, Vec<Tri>)>;

/// The baked extent of one axis if it exceeds the GE limit, checked for
/// the one shape this lowering is exact for.
fn wide_axis(t: &TextureRef, axis: usize) -> Result<Option<WideAxis>, ()> {
    let (period, mirror, clamp, drawn, mask, origin) = if axis == 0 {
        (
            t.width,
            t.mirror_s,
            t.clamp_s,
            t.drawn_width,
            t.mask_s,
            t.origin_s,
        )
    } else {
        (
            t.height,
            t.mirror_t,
            t.clamp_t,
            t.drawn_height,
            t.mask_t,
            t.origin_t,
        )
    };
    let (period, drawn) = (u32::from(period), u32::from(drawn));
    if mirror_axis_len(period, mirror, clamp, drawn) <= GE_MAX_TEXTURE_DIM {
        return Ok(None);
    }
    let baked_period = if mirror { period * 2 } else { period };
    let narrowed = mask > 0 && mask < 16 && period == 1 << mask;
    if !clamp
        || !narrowed
        || origin != 0
        || baked_period > GE_MAX_TEXTURE_DIM
        || drawn <= baked_period
    {
        return Err(());
    }
    Ok(Some(WideAxis {
        period: baked_period,
        drawn,
    }))
}

fn coord(v: &MeshVertex, axis: usize) -> i32 {
    i32::from(v.uv[axis])
}

fn range(tri: &Tri, axis: usize) -> (i32, i32) {
    let c = tri.map(|v| coord(&v, axis));
    (c[0].min(c[1]).min(c[2]), c[0].max(c[1]).max(c[2]))
}

/// First S10.5 coordinate the RDP clamps at the far edge: `lrs = W - 1`
/// texels, with no fractional quarter-texel (the tile-size decode floors
/// it away; every affected ROM tile is a whole number of periods).
fn band_start(w: &WideAxis) -> i32 {
    (w.drawn as i32 - 1) * 32
}

/// Window start for one axis: the multiple of the period, with a window of
/// at most 512 texels, that splits the fewest triangles. Ties go to the
/// smaller window. `None` when no triangle reaches the clamp band.
fn window_start(tris: &[Tri], axis: usize, w: &WideAxis) -> Option<u32> {
    let band = band_start(w);
    let reaching: Vec<(i32, i32)> = tris
        .iter()
        .map(|t| range(t, axis))
        .filter(|&(_, hi)| hi >= band)
        .collect();
    if reaching.is_empty() {
        return None;
    }
    let lo = w
        .drawn
        .saturating_sub(GE_MAX_TEXTURE_DIM)
        .div_ceil(w.period)
        * w.period;
    let hi = (w.drawn - 1) / w.period * w.period;
    let cost = |start: u32| {
        let x = start as i32 * 32;
        if reaching.iter().all(|&(min, _)| min >= x) {
            return 0;
        }
        tris.iter()
            .filter(|t| {
                let (min, max) = range(t, axis);
                min < x && x < max
            })
            .count()
    };
    (lo..=hi)
        .step_by(w.period as usize)
        .min_by_key(|&start| (cost(start), core::cmp::Reverse(start)))
}

/// `a + (b - a) * num / den`, rounded to nearest, ties away from zero.
fn lerp(a: i64, b: i64, num: i64, den: i64) -> i64 {
    let n = (b - a) * num;
    let q = if n >= 0 {
        (2 * n + den) / (2 * den)
    } else {
        -((-2 * n + den) / (2 * den))
    };
    a + q
}

/// Whether `pack::add_mesh` will read `rgba` as a normal.
fn carries_normal(v: &MeshVertex) -> bool {
    v.lit || looks_like_unit_normal(v.rgba)
}

/// The point on edge `a`-`b` where `axis` equals `x`.
///
/// Interpolates from the endpoint with the lower coordinate, so both
/// triangles sharing the edge get the same vertex. Positions round to the
/// nearest unit; the split coordinate is exact. A normal interpolates
/// linearly without renormalising: `pack::shade_normal` is linear in the
/// normal while `n . l >= 0`, so the baked shade at the new vertex equals
/// the Gouraud shade the unsplit edge had there.
fn intersect(a: &MeshVertex, b: &MeshVertex, axis: usize, x: i32) -> Result<MeshVertex, ()> {
    let (lo, hi) = if (coord(a, axis), a) <= (coord(b, axis), b) {
        (a, b)
    } else {
        (b, a)
    };
    if carries_normal(lo) != carries_normal(hi) {
        return Err(());
    }
    let normal = carries_normal(lo);
    let num = i64::from(x - coord(lo, axis));
    let den = i64::from(coord(hi, axis) - coord(lo, axis));
    let i16_at = |p: i16, q: i16| lerp(p.into(), q.into(), num, den) as i16;
    let mut v = *lo;
    v.pos = core::array::from_fn(|k| i16_at(lo.pos[k], hi.pos[k]));
    v.uv = core::array::from_fn(|k| {
        if k == axis {
            x as i16
        } else {
            i16_at(lo.uv[k], hi.uv[k])
        }
    });
    v.rgba = core::array::from_fn(|k| {
        if normal && k < 3 {
            lerp(
                i64::from(lo.rgba[k] as i8),
                i64::from(hi.rgba[k] as i8),
                num,
                den,
            ) as i8 as u8
        } else {
            lerp(lo.rgba[k].into(), hi.rgba[k].into(), num, den) as u8
        }
    });
    v.lit = normal;
    Ok(v)
}

/// Splits a triangle along `axis == x` into the part below and the part
/// above, each fanned back into triangles with the source winding.
fn split(tri: &Tri, axis: usize, x: i32) -> Result<(Vec<Tri>, Vec<Tri>), ()> {
    let mut below = Vec::with_capacity(4);
    let mut above = Vec::with_capacity(4);
    for i in 0..3 {
        let (p, q) = (&tri[i], &tri[(i + 1) % 3]);
        let (cp, cq) = (coord(p, axis), coord(q, axis));
        if cp <= x {
            below.push(*p);
        }
        if cp >= x {
            above.push(*p);
        }
        if (cp < x && x < cq) || (cq < x && x < cp) {
            let v = intersect(p, q, axis, x)?;
            below.push(v);
            above.push(v);
        }
    }
    let fan = |poly: Vec<MeshVertex>| -> Vec<Tri> {
        (1..poly.len().saturating_sub(1))
            .map(|i| [poly[0], poly[i], poly[i + 1]])
            .collect()
    };
    Ok((fan(below), fan(above)))
}

/// The equivalent tile for one class on one axis.
fn lowered_texture(
    mut t: TextureRef,
    axis: usize,
    class: AxisClass,
    w: Option<WideAxis>,
) -> TextureRef {
    let Some(w) = w else { return t };
    let (clamp, drawn) = if axis == 0 {
        (&mut t.clamp_s, &mut t.drawn_width)
    } else {
        (&mut t.clamp_t, &mut t.drawn_height)
    };
    match class {
        AxisClass::Keep => {}
        AxisClass::Repeat => {
            *clamp = false;
            *drawn = w.period as u16;
        }
        AxisClass::Window { start } => *drawn = (w.drawn - start) as u16,
    }
    t
}

/// Lowers one primitive's triangles, or `None` when it has no wide axis.
fn lower_primitive(
    mesh: &Mesh,
    index: usize,
    prim: &Primitive,
    uv_static: bool,
    stats: &mut Stats,
) -> Result<Option<Runs>, WideTileError> {
    let Some(t) = prim.material.texture.filter(|t| !t.framebuffer) else {
        return Ok(None);
    };
    let mut wide = [None; 2];
    for (axis, slot) in wide.iter_mut().enumerate() {
        *slot = wide_axis(&t, axis)
            .map_err(|()| WideTileError::UnsupportedTile { prim: index, axis })?;
    }
    if wide.iter().all(Option::is_none) {
        return Ok(None);
    }
    if prim.material.texgen_scale.is_some() {
        return Err(WideTileError::TextureGen { prim: index });
    }
    if !uv_static {
        return Err(WideTileError::AnimatedUv { prim: index });
    }
    stats.primitives += 1;

    let mut pieces: Vec<(Tri, [AxisClass; 2])> = prim
        .indices
        .as_chunks::<3>()
        .0
        .iter()
        .map(|i| (i.map(|i| mesh.vertices[i as usize]), [AxisClass::Keep; 2]))
        .collect();
    let mut split_sources = alloc::collections::BTreeSet::new();
    for axis in 0..2 {
        let Some(w) = wide[axis] else { continue };
        let tris: Vec<Tri> = pieces.iter().map(|p| p.0).collect();
        if tris.iter().any(|t| range(t, axis).0 < 0) {
            return Err(WideTileError::NegativeCoordinate { prim: index, axis });
        }
        let start = window_start(&tris, axis, &w);
        let band = band_start(&w);
        let x = start.map(|s| s as i32 * 32);
        let must_split = x.is_some_and(|x| {
            tris.iter()
                .any(|t| range(t, axis).1 >= band && range(t, axis).0 < x)
        });
        let mut next = Vec::with_capacity(pieces.len());
        for (source, (tri, mut class)) in pieces.into_iter().enumerate() {
            let (min, max) = range(&tri, axis);
            let parts = match x {
                Some(x) if must_split && min < x && x < max => {
                    split_sources.insert((axis, source));
                    let (below, above) = split(&tri, axis, x)
                        .map_err(|()| WideTileError::MixedLighting { prim: index })?;
                    below.into_iter().chain(above).collect()
                }
                _ => alloc::vec![tri],
            };
            for part in parts {
                let (min, max) = range(&part, axis);
                class[axis] = match start {
                    Some(start) if max >= band => {
                        debug_assert!(min >= start as i32 * 32);
                        AxisClass::Window { start }
                    }
                    _ => AxisClass::Repeat,
                };
                next.push((part, class));
            }
        }
        pieces = next;
    }
    stats.split_triangles += split_sources.len();

    // Consecutive triangles with the same classes stay one primitive, so
    // submission order within the source primitive is preserved exactly.
    let mut runs: Vec<([AxisClass; 2], Vec<Tri>)> = Vec::new();
    for (mut tri, class) in pieces {
        for (axis, c) in class.iter().enumerate() {
            if let AxisClass::Window { start } = *c {
                for v in &mut tri {
                    v.uv[axis] -= (start * 32) as i16;
                }
            }
        }
        match runs.last_mut() {
            Some((c, tris)) if *c == class => tris.push(tri),
            _ => runs.push((class, alloc::vec![tri])),
        }
    }
    Ok(Some(
        runs.into_iter()
            .map(|(class, tris)| {
                let t = lowered_texture(t, 0, class[0], wide[0]);
                (lowered_texture(t, 1, class[1], wide[1]), tris)
            })
            .collect(),
    ))
}

/// Lowers every primitive whose baked texture would exceed the GE's
/// 512-texel limit on either axis. `uv_static(i)` must say whether input
/// primitive `i` keeps its authored UVs for its whole life (no live
/// material UV animation); only then are its triangles' coordinate ranges
/// the ones it samples.
///
/// A mesh with nothing to lower comes back unchanged.
pub fn lower(mesh: &Mesh, uv_static: impl Fn(usize) -> bool) -> Result<Lowered, WideTileError> {
    let mut stats = Stats::default();
    let mut lowered = Vec::with_capacity(mesh.primitives.len());
    for (i, prim) in mesh.primitives.iter().enumerate() {
        lowered.push(lower_primitive(mesh, i, prim, uv_static(i), &mut stats)?);
    }
    if lowered.iter().all(Option::is_none) {
        return Ok(Lowered {
            mesh: mesh.clone(),
            source_prim: (0..mesh.primitives.len()).collect(),
            stats,
        });
    }

    // Rebuild the shared vertex buffer: untouched primitives keep their
    // vertices in first-use order, new ones are deduplicated as they come.
    let mut vertices: Vec<MeshVertex> = Vec::new();
    let mut seen: BTreeMap<MeshVertex, u16> = BTreeMap::new();
    let mut intern = |v: MeshVertex| -> Result<u16, WideTileError> {
        if let Some(&i) = seen.get(&v) {
            return Ok(i);
        }
        let i = u16::try_from(vertices.len()).map_err(|_| WideTileError::TooManyVertices)?;
        vertices.push(v);
        seen.insert(v, i);
        Ok(i)
    };
    let mut primitives = Vec::new();
    let mut source_prim = Vec::new();
    for (i, (prim, lowered)) in mesh.primitives.iter().zip(lowered).enumerate() {
        match lowered {
            None => {
                let indices = prim
                    .indices
                    .iter()
                    .map(|&v| intern(mesh.vertices[v as usize]))
                    .collect::<Result<_, _>>()?;
                primitives.push(Primitive {
                    material: prim.material,
                    indices,
                });
                source_prim.push(i);
            }
            Some(runs) => {
                for (texture, tris) in runs {
                    let mut indices = Vec::with_capacity(tris.len() * 3);
                    for v in tris.into_iter().flatten() {
                        indices.push(intern(v)?);
                    }
                    let mut material = prim.material;
                    material.texture = Some(texture);
                    primitives.push(Primitive { material, indices });
                    source_prim.push(i);
                    stats.emitted += 1;
                }
            }
        }
    }
    Ok(Lowered {
        mesh: Mesh {
            vertices,
            primitives,
        },
        source_prim,
        stats,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::MeshMaterial;
    use crate::n64_addressing::{address_axis, TileAxis};
    use crate::texture::{BitSize, Format, TextureLut};

    /// A 32-texel clamped period drawn across `drawn` texels on S.
    fn tile(period_log2: u8, drawn: u16, mirror: bool) -> TextureRef {
        TextureRef {
            data_file: None,
            data_offset: 0x100,
            format: Format::Ci,
            size: BitSize::Bits4,
            width: 1 << period_log2,
            height: 16,
            source_width: 1 << period_log2,
            palette_file: None,
            palette_offset: Some(0),
            palette_entries: 16,
            palette: 0,
            mirror_s: mirror,
            mirror_t: false,
            clamp_s: true,
            clamp_t: true,
            framebuffer: false,
            origin_s: 0,
            origin_t: 0,
            mask_s: period_log2,
            mask_t: 4,
            drawn_width: drawn,
            drawn_height: 16,
            tlut: TextureLut::Rgba16,
        }
    }

    fn vtx(x: i16, u: i32) -> MeshVertex {
        MeshVertex {
            pos: [x, 0, 0],
            uv: [u as i16, 0],
            rgba: [255, 255, 255, 255],
            lit: false,
        }
    }

    /// One quad spanning the whole rect on S, as two triangles.
    fn quad_mesh(t: TextureRef) -> Mesh {
        let far = i32::from(t.drawn_width) * 32 - 1;
        let v = [vtx(0, 0), vtx(1000, far), vtx(1000, far), vtx(0, 0)];
        let mut v = v;
        v[2].pos[1] = 100;
        v[2].uv[1] = 511;
        v[3].pos[1] = 100;
        v[3].uv[1] = 511;
        let material = MeshMaterial {
            texture: Some(t),
            ..MeshMaterial::default()
        };
        Mesh {
            vertices: v.to_vec(),
            primitives: alloc::vec![Primitive {
                material,
                indices: alloc::vec![0, 1, 2, 0, 2, 3]
            }],
        }
    }

    /// The texel the PSP samples for an absolute source coordinate `c`
    /// through one lowered primitive: its shift, its baked period, and its
    /// wrap mode at the texture's declared (power-of-two) size.
    fn psp_texel(t: &TextureRef, shift: i32, c: i32) -> i32 {
        let period = i32::from(t.width) * if t.mirror_s { 2 } else { 1 };
        let baked = mirror_axis_len(
            u32::from(t.width),
            t.mirror_s,
            t.clamp_s,
            u32::from(t.drawn_width),
        ) as i32;
        let local = (c - shift).div_euclid(32);
        let index = if t.clamp_s {
            local.clamp(0, baked - 1)
        } else {
            local.rem_euclid(period)
        };
        // Undo the bake: fold the baked index back to a source texel.
        let phase = index % i32::from(t.width);
        if t.mirror_s && (index / i32::from(t.width)) % 2 == 1 {
            i32::from(t.width) - 1 - phase
        } else {
            phase
        }
    }

    fn hardware_texel(t: &TextureRef, c: i32) -> i32 {
        address_axis(
            &TileAxis {
                shift: 0,
                origin_q2: 0,
                far_edge_q2: (i32::from(t.drawn_width) - 1) * 4,
                mask: t.mask_s,
                mirror: t.mirror_s,
                clamp_bit: true,
            },
            c,
        )
    }

    /// Every output primitive's baked axis fits the GE, and every
    /// coordinate each output triangle can sample -- plus the texel after
    /// it, which bilinear filtering also reads -- addresses the same source
    /// texel the RDP would, with the RDP's own far-edge clamp.
    fn check_equivalent(t: TextureRef) {
        let mesh = quad_mesh(t);
        let out = lower(&mesh, |_| true).unwrap();
        for prim in &out.mesh.primitives {
            let lt = prim.material.texture.unwrap();
            let baked = mirror_axis_len(
                u32::from(lt.width),
                lt.mirror_s,
                lt.clamp_s,
                u32::from(lt.drawn_width),
            );
            assert!(baked <= GE_MAX_TEXTURE_DIM);
            let shift = if lt.clamp_s {
                (i32::from(t.drawn_width) - i32::from(lt.drawn_width)) * 32
            } else {
                0
            };
            // The source quad maps x in 0..=1000 linearly onto 0..=far, so
            // every output vertex, shifted back, must land on that map
            // within its half-unit position rounding.
            let far = i32::from(t.drawn_width) * 32 - 1;
            for &i in &prim.indices {
                let v = out.mesh.vertices[i as usize];
                let source = i32::from(v.pos[0]) * far / 1000;
                assert!(
                    (i32::from(v.uv[0]) + shift - source).abs() <= far / 1000 + 1,
                    "vertex {v:?}"
                );
            }
            for tri in prim.indices.as_chunks::<3>().0 {
                let c: Vec<i32> = tri
                    .iter()
                    .map(|&i| i32::from(out.mesh.vertices[i as usize].uv[0]) + shift)
                    .collect();
                let (lo, hi) = (*c.iter().min().unwrap(), *c.iter().max().unwrap());
                for s in lo..=hi {
                    assert_eq!(
                        psp_texel(&lt, shift, s),
                        hardware_texel(&t, s),
                        "coordinate {s}"
                    );
                    if s < (i32::from(t.drawn_width) - 1) * 32 {
                        // Bilinear neighbour: unclamped on the RDP too.
                        assert_eq!(
                            psp_texel(&lt, shift, s + 32),
                            hardware_texel(&t, s + 32),
                            "neighbour of {s}"
                        );
                    } else {
                        // Clamp band: the RDP zeroes the fraction, the GE
                        // clamps the neighbour onto the same texel.
                        assert_eq!(psp_texel(&lt, shift, s + 32), hardware_texel(&t, s));
                    }
                }
            }
        }
        assert!(
            out.mesh.primitives.len() >= 2,
            "a rect reaching its far edge needs both tiles"
        );
    }

    #[test]
    fn a_clamped_period_across_a_wide_rect_lowers_to_equivalent_tiles() {
        // 744: 32 over 608. 797: 8-texel mirrored period over 576.
        // 424: 64 mirrored over 576 (not a whole number of mirrored pairs).
        // Unmirrored first: at a mirrored far edge texels `W - 1` and `W`
        // fold to the same source texel, so only an unmirrored rect can
        // tell a missing clamp apart.
        check_equivalent(tile(5, 608, false));
        check_equivalent(tile(3, 576, true));
        check_equivalent(tile(6, 576, true));
        check_equivalent(tile(5, 960, false));
    }

    #[test]
    fn a_tile_that_fits_is_left_alone() {
        let mesh = quad_mesh(tile(5, 512, false));
        let out = lower(&mesh, |_| true).unwrap();
        assert_eq!(out.mesh, mesh);
        assert_eq!(out.stats, Stats::default());
    }

    #[test]
    fn triangles_that_never_reach_the_far_edge_just_repeat_the_period() {
        let mut mesh = quad_mesh(tile(5, 608, false));
        for v in &mut mesh.vertices {
            v.uv[0] = v.uv[0].min(600 * 32);
        }
        let out = lower(&mesh, |_| true).unwrap();
        assert_eq!(out.mesh.primitives.len(), 1);
        let t = out.mesh.primitives[0].material.texture.unwrap();
        assert!(!t.clamp_s);
        assert_eq!(t.drawn_width, 32);
        assert_eq!(out.stats.split_triangles, 0);
    }

    #[test]
    fn a_split_keeps_winding_and_shares_its_new_vertices() {
        let mesh = quad_mesh(tile(5, 608, false));
        let out = lower(&mesh, |_| true).unwrap();
        let area = |tri: &[u16]| {
            let p = tri
                .iter()
                .map(|&i| out.mesh.vertices[i as usize].pos)
                .collect::<Vec<_>>();
            (i32::from(p[1][0]) - i32::from(p[0][0])) * (i32::from(p[2][1]) - i32::from(p[0][1]))
                - (i32::from(p[2][0]) - i32::from(p[0][0]))
                    * (i32::from(p[1][1]) - i32::from(p[0][1]))
        };
        let source = area(&[0, 1, 2]).signum();
        for prim in &out.mesh.primitives {
            for tri in prim.indices.as_chunks::<3>().0 {
                let a = area(tri);
                assert!(a == 0 || a.signum() == source);
            }
        }
        assert_eq!(out.stats.split_triangles, 2);
    }

    #[test]
    fn animated_uvs_are_refused_not_guessed() {
        let mesh = quad_mesh(tile(5, 608, false));
        assert_eq!(
            lower(&mesh, |_| false),
            Err(WideTileError::AnimatedUv { prim: 0 })
        );
    }

    #[test]
    fn an_unnarrowed_wide_tile_is_refused() {
        let mut t = tile(5, 608, false);
        t.mask_s = 0;
        t.width = 608;
        assert_eq!(
            lower(&quad_mesh(t), |_| true),
            Err(WideTileError::UnsupportedTile { prim: 0, axis: 0 })
        );
    }
}
