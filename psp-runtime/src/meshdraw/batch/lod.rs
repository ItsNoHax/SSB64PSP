//! [`super`]'s passes for a tag with the Fall's two-tile blends
//! (`LOD_BLEND`) or float UVs (`SIGNED_CLAMP_UV`), which [`super::plan_object`]
//! leaves to [`crate::boss::BossEffects::draw`]. [`super::batch_pose`] is
//! shared; [`batch_expand_lod_tag`] and [`batch_submit_lod_tag`] are
//! [`super::batch_expand`] and [`super::batch_submit`] with those
//! primitives' [`super::super::draw_mesh_vertices`] paths added. They are
//! copies, not branches, so the comets' passes keep their code and
//! addresses (RE-585).
//!
//! The records: one per primitive at [`super::BatchMesh`]'s `record`, as
//! [`super::batch_expand`] takes them; past every primitive's, one per
//! texel-alpha blend's second pass, in primitive order
//! ([`super::super::draw_lod_texel_alpha`]), which [`plan_object_lod`]
//! counts. What else these passes read, against the per-object path:
//! - `expand_float_uv_corners` reads no `DrawState`;
//! - a texel-alpha blend's plan ([`texel_alpha_plan`]: pass alphas, both
//!   passes' textures and mappings) reads the effect's material player, its
//!   `color_override`, `lod_blend_isolate` and the pack, none of which a
//!   draw changes; [`batch_expand_lod_tag`] works it out once and
//!   [`batch_submit_lod_tag`] binds from it.
//!
//! `bind_lod_next`'s binds, `batch_material`'s and the texel-alpha blend's
//! lookups are split: the lookups ([`material_inputs`], the plan) run in a
//! loop of their own, so the per-mesh draw does not run their code
//! (RE-585). `draw_lod_blend_pass`, `apply_lod_blend_translucent_pass1` and
//! `expand_float_uv_corners` are out of line in the per-object path and
//! shared; `texel_alpha_pass_alphas` is inlined there and copied here
//! (RE-494).

use core::ffi::c_void;

use psp::sys::{self, GuState};
use ssb_rom::pack::{flags, NodeDesc, ObjectDesc, Pack, PackedVertex, PrimDesc, TextureDesc};
use ssb_rom::skeleton::{
    EffectColors, EffectMaterialAnimator, LodBlendState, MaterialUv, UvAffine,
};

use super::super::{
    animated_uv_affine, apply_fade_blend, apply_flags_state, apply_lod_blend_translucent_pass1,
    apply_texture_affine, bind_texture, bind_texture_cached, bind_texture_uncached,
    clip_submission, draw_lod_blend_pass, expand_corners, expand_float_uv_corners,
    position_outcode, ClipVertex, Draw, DrawState, FloatUvVertex, GuardClip, LodBlendIsolate,
    TextureFuncState, CLIP_CODES, CLIP_CODES_MAX, CLIP_DRAWS, CLIP_DRAWS_MAX, CLIP_VERTEX_FORMAT,
    FLOAT_UV_VERTEX_FORMAT, GUARD_BAND, INDEXED_FLOAT_UV_VERTEX_FORMAT, LIGHT_PREPASS,
    LOD_BLEND_TRANSLUCENT, VERTEX_16BIT_DIVISOR, VERTEX_FORMAT,
};
use super::{clip_transform, Batch, BatchSize};

/// [`super::UNBATCHED_FLAGS`] less the paths these passes add.
const LOD_UNBATCHED_FLAGS: u32 = flags::LIT | flags::TEXTURE_GEN | flags::TEXTURE_GEN_LINEAR;

/// [`texel_alpha_plan`]'s answer for one primitive that takes a second-pass
/// record: [`super::super::draw_lod_texel_alpha`]'s two passes, or the
/// single pass (`two_pass` false).
#[derive(Clone, Copy)]
#[repr(C)]
struct TexelPlan {
    two_pass: bool,
    /// The passes' alphas, `TEXEL0`'s then `TEXEL1`'s.
    v0: u8,
    v1: u8,
    /// The first pass's material: `batch_material`'s texture, palette and
    /// mapping.
    texture: u32,
    palette: u32,
    affine: UvAffine,
    /// `TEXEL1` and `bind_lod_next`'s mapping of it (the identity when the
    /// pack lacks it: the pass is not drawn).
    next: u32,
    next_affine: UvAffine,
}

impl TexelPlan {
    /// The single pass: only `two_pass` is read. All zero, so it is
    /// stored without a constant.
    const SINGLE: Self = Self {
        two_pass: false,
        v0: 0,
        v1: 0,
        texture: 0,
        palette: 0,
        affine: Self::UNUSED,
        next: 0,
        next_affine: Self::UNUSED,
    };
    const UNUSED: UvAffine = UvAffine {
        scale_s: 0.0,
        scale_t: 0.0,
        offset_s: 0.0,
        offset_t: 0.0,
    };
}

/// Plan `j` of the tag (its `j`th second-pass record's primitive): at the
/// top of [`CLIP_CODES`], below plan `j - 1`, which [`plan_object_lod`]
/// keeps clear of the outcodes. Byte-aligned: read and written unaligned.
#[inline(always)]
unsafe fn plan_slot(j: usize) -> *mut TexelPlan {
    let table = core::ptr::addr_of_mut!(CLIP_CODES) as *mut u8;
    table.add(CLIP_CODES_MAX - (j + 1) * core::mem::size_of::<TexelPlan>()) as *mut TexelPlan
}

/// Whether primitive flags `flags` take a second-pass record: a
/// translucent two-tile blend may draw as a texel-alpha blend's two passes.
#[inline(always)]
fn takes_spare(flags: u32) -> bool {
    flags & LOD_BLEND_TRANSLUCENT == LOD_BLEND_TRANSLUCENT
}

/// [`super::plan_object`] for these passes: [`LOD_UNBATCHED_FLAGS`] in
/// place of [`super::UNBATCHED_FLAGS`], and a second-pass record and a
/// [`TexelPlan`] (in `verts`: [`plan_slot`]) for each primitive that may
/// take one.
#[inline(never)]
pub(crate) fn plan_object_lod(pack: &Pack<'_>, object: &ObjectDesc, size: &mut BatchSize) -> bool {
    for i in 0..object.node_count {
        let Some(node) = pack.node(object.first_node + i) else {
            continue;
        };
        // Costume 0 without parts draws the node's own mesh, no accessory.
        if node.mesh == NodeDesc::NO_MESH {
            continue;
        }
        if node.flags & NodeDesc::FLAG_BILLBOARD != 0 {
            return false;
        }
        let Some(mesh) = pack.mesh(node.mesh) else {
            return false;
        };
        let Some(verts) = pack.vertices(&mesh) else {
            return false;
        };
        for j in 0..mesh.prim_count {
            let Some(p) = pack.prim(mesh.first_prim + j) else {
                continue;
            };
            if p.flags & LOD_UNBATCHED_FLAGS != 0 {
                return false;
            }
            let spare = usize::from(takes_spare(p.flags));
            size.prims += spare;
            size.verts += spare * core::mem::size_of::<TexelPlan>();
        }
        size.meshes += 1;
        size.prims += mesh.prim_count as usize;
        size.verts += verts.len() / core::mem::size_of::<PackedVertex>();
    }
    true
}

/// [`super::batch_expand`] with the `LOD_BLEND` and `SIGNED_CLAMP_UV`
/// primitives' submissions: float UVs by [`expand_float_uv_corners`] as
/// [`super::super::draw_mesh_vertices`] chooses them, or a texel-alpha
/// blend's two ([`texel_alpha_plan`]), the second in the next spare record.
/// `objects` are the posed objects in order: each one's meshes and
/// material player. First, in a loop of their own, every second-pass
/// record's [`TexelPlan`], which [`batch_submit_lod_tag`] binds from.
///
/// # Safety
///
/// As [`super::batch_expand`], for a batch [`plan_object_lod`] sized.
#[inline(never)]
pub(crate) unsafe fn batch_expand_lod_tag(
    pack: &Pack<'_>,
    st: &DrawState,
    batch: &mut Batch,
    objects: &[(usize, Option<&EffectMaterialAnimator>)],
) {
    let band = if st.clip_at_viewport { 1.0 } else { GUARD_BAND };
    let size = core::mem::size_of::<PackedVertex>();
    let mut j = 0usize;
    let mut object = 0usize;
    let mut left = 0usize;
    let mut mats = None;
    for k in 0..batch.len {
        while left == 0 {
            let Some(&(n, m)) = objects.get(object) else {
                break;
            };
            object += 1;
            left = n;
            mats = m;
        }
        left = left.saturating_sub(1);
        let m = &*batch.meshes.add(k);
        let verts = core::slice::from_raw_parts(m.verts, m.verts_len);
        // `material_colors` with `color_override` set.
        let effect_colors = Some(m.colors).filter(|c| c.prim.is_some() || c.env.is_some());
        for i in 0..m.prim_count {
            let Some(p) = pack.prim(m.first_prim + i) else {
                continue;
            };
            if !takes_spare(p.flags) {
                continue;
            }
            let plan = match pack.indices(&p) {
                Some(indices) if st.heads.admits(p.flags) && p.index_count != 0 => {
                    merged_texel_alpha_plan(pack, &p, st, indices, verts, effect_colors, mats)
                }
                _ => TexelPlan::SINGLE,
            };
            // `BatchSize::fits`: [`plan_object_lod`] counted it.
            plan_slot(j).write_unaligned(plan);
            j += 1;
        }
    }
    let table = &mut *core::ptr::addr_of_mut!(CLIP_CODES);
    let mut at = 0usize;
    let mut spare = batch.prims;
    for k in 0..batch.len {
        let m = &mut *batch.meshes.add(k);
        let verts = core::slice::from_raw_parts(m.verts, m.verts_len);
        let count = verts.len() / size;
        m.codes = at as u32;
        if let Some(vp) = st.clip_view_proj {
            let mvp = clip_transform(&vp, &m.model);
            let aligned = verts.as_ptr() as usize % core::mem::align_of::<PackedVertex>() == 0;
            let position = |bytes: &[u8]| {
                let from = bytes.as_ptr() as *const PackedVertex;
                // SAFETY: `bytes` is one whole `PackedVertex`.
                let v = unsafe {
                    if aligned {
                        from.read()
                    } else {
                        from.read_unaligned()
                    }
                };
                [v.x, v.y, v.z]
            };
            let mut union = 0u8;
            // `BatchSize::fits`: every mesh's table fits at once.
            let codes = table.get_unchecked_mut(at..at + count);
            for (code, bytes) in codes.iter_mut().zip(verts.chunks_exact(size)) {
                *code = position_outcode(&mvp, band, position(bytes));
                union |= *code;
            }
            m.clips = union != 0;
        }
        at += count;
        // `material_colors` with `color_override` set.
        let effect_colors = Some(m.colors).filter(|c| c.prim.is_some() || c.env.is_some());
        for i in 0..m.prim_count {
            let record = batch.records.add(m.record as usize + i as usize);
            record.write(Draw::EMPTY);
            let Some(p) = pack.prim(m.first_prim + i) else {
                continue;
            };
            let mut next = record;
            let mut plan = TexelPlan::SINGLE;
            if takes_spare(p.flags) {
                // `BatchSize::fits`: [`plan_object_lod`] counted it.
                plan = plan_slot(spare - batch.prims).read_unaligned();
                next = batch.records.add(spare);
                next.write(Draw::EMPTY);
                spare += 1;
            }
            if !st.heads.admits(p.flags) {
                continue;
            }
            let Some(indices) = pack.indices(&p) else {
                continue;
            };
            if p.index_count == 0 {
                continue;
            }
            let signed_clamp_uv = p.flags & flags::SIGNED_CLAMP_UV != 0;
            // RE-321: `draw_lod_texel_alpha`'s expansion of each pass, at
            // the pass's alpha; [`batch_submit_lod_tag`] draws them.
            if plan.two_pass {
                let base = effect_colors.unwrap_or_default();
                for (pass, alpha) in [(0usize, plan.v0), (1, plan.v1)] {
                    if alpha == 0 {
                        continue;
                    }
                    let rgb = base.prim.map_or([0xFF; 3], |c| [c[0], c[1], c[2]]);
                    let colors = EffectColors {
                        prim: Some([rgb[0], rgb[1], rgb[2], alpha]),
                        ..base
                    };
                    let (vtype, index, vertex) = if signed_clamp_uv {
                        expand_float_uv_corners(&p, indices, verts, Some(colors), None)
                    } else {
                        expand_corners(pack, &p, st, indices, verts, Some(colors), false, None)
                    };
                    let to = if pass == 0 { record } else { next };
                    to.write(Draw {
                        vtype: vtype.bits(),
                        index,
                        vertex,
                        count: p.index_count,
                    });
                }
                continue;
            }
            let (vtype, index, vertex) = if signed_clamp_uv {
                expand_float_uv_corners(&p, indices, verts, effect_colors, None)
            } else if effect_colors.is_some() {
                expand_corners(pack, &p, st, indices, verts, effect_colors, false, None)
            } else {
                (
                    VERTEX_FORMAT,
                    indices.as_ptr() as *const c_void,
                    verts.as_ptr() as *const c_void,
                )
            };
            record.write(Draw {
                vtype: vtype.bits(),
                index,
                vertex,
                count: p.index_count,
            });
        }
    }
}

/// [`super::batch_submit`] with the `LOD_BLEND` primitives' draws:
/// [`super::super::draw_lod_texel_alpha`] with [`batch_expand_lod_tag`]'s
/// expansions, or the single pass with the translucent pass 1 and the next
/// tile's pass, as [`super::super::draw_mesh_vertices`] sends them.
/// `spares` counts the second-pass records the objects before took.
///
/// # Safety
///
/// As [`super::batch_submit`]; after [`batch_expand_lod_tag`], the objects
/// in its order.
#[inline(never)]
pub(crate) unsafe fn batch_submit_lod_tag(
    pack: &Pack<'_>,
    st: &mut DrawState,
    batch: &Batch,
    first: usize,
    count: usize,
    effect_mat_anim: Option<&EffectMaterialAnimator>,
    spares: &mut usize,
) {
    // `light_prepass`, which returns at once without a fighter light.
    (*core::ptr::addr_of_mut!(LIGHT_PREPASS)).0 = 0;
    let band = if st.clip_at_viewport { 1.0 } else { GUARD_BAND };
    let size = core::mem::size_of::<PackedVertex>();
    for k in first..first + count {
        let m = &*batch.meshes.add(k);
        let matrix_t = crate::profile::start_fine();
        sys::sceGumUpdateMatrix();
        sys::sceGuSetMatrix(sys::MatrixMode::Model, &m.model);
        st.set_model_matrix(m.model);
        crate::profile::stop_fine(crate::profile::Span::NodeMatrix, matrix_t);
        // `Look::costume(0)`: no texture part.
        st.texture_part_ids = [0; 2];
        st.texture_part_mesh = None;
        let verts = core::slice::from_raw_parts(m.verts, m.verts_len);
        let clip = st.clip_view_proj.filter(|_| m.clips).map(|vp| GuardClip {
            mvp: clip_transform(&vp, &m.model),
            band,
            verts,
            // `batch_expand_lod_tag` wrote them inside the table
            // (`BatchSize::fits`).
            codes: Some(
                (&*core::ptr::addr_of!(CLIP_CODES))
                    .get_unchecked(m.codes as usize..m.codes as usize + verts.len() / size),
            ),
        });
        let mut tris = 0u32;
        for i in 0..m.prim_count {
            let Some(p) = pack.prim(m.first_prim + i) else {
                continue;
            };
            let record = batch.records.add(m.record as usize + i as usize);
            let mut next = record;
            let mut plan = TexelPlan::SINGLE;
            if takes_spare(p.flags) {
                plan = plan_slot(*spares).read_unaligned();
                next = batch.records.add(batch.prims + *spares);
                *spares += 1;
            }
            if !st.admits(p.flags) {
                continue;
            }
            let Some(indices) = pack.indices(&p) else {
                continue;
            };
            if p.index_count == 0 {
                continue;
            }
            let material_t = crate::profile::start_fine();
            st.texture_override = None;
            // `material_colors`: the override wins.
            let effect_colors = st.color_override;
            let (texture, palette, affine) = if plan.two_pass {
                (plan.texture, plan.palette, plan.affine)
            } else {
                material_inputs(pack, &p, effect_mat_anim)
            };
            lod_material(pack, &p, st, texture, palette, affine, effect_colors);
            st.texture_override = None;
            crate::profile::stop_fine(crate::profile::Span::Material, material_t);

            // RE-321: `draw_lod_texel_alpha` with [`batch_expand_lod_tag`]'s
            // expansions and plan.
            if plan.two_pass {
                sys::sceGuDisable(GuState::AlphaTest);
                st.last_flags = None;
                st.state_changes += 1;
                let mut n = 0;
                // The first pass's clip, once it took one: only the second
                // pass sees it.
                let mut first = None;
                for (pass, alpha) in [(0usize, plan.v0), (1, plan.v1)] {
                    if alpha == 0 {
                        continue;
                    }
                    if pass == 1 {
                        // `bind_lod_next` with the plan's mapping.
                        let Some(t) = pack.texture(plan.next) else {
                            break;
                        };
                        if st.last_texture != Some((plan.next, TextureDesc::NO_ANIM)) {
                            st.last_texture = Some((plan.next, TextureDesc::NO_ANIM));
                            st.state_changes += 1;
                            bind_texture(pack, plan.next, &t, TextureDesc::NO_ANIM);
                        }
                        apply_texture_affine(pack, &p, st, plan.next, plan.next_affine);
                        if st.last_texture_func != Some(TextureFuncState::Modulate) {
                            st.last_texture_func = Some(TextureFuncState::Modulate);
                            sys::sceGuTexFunc(
                                sys::TextureEffect::Modulate,
                                sys::TextureColorComponent::Rgba,
                            );
                        }
                        sys::sceGuDepthMask(1);
                    }
                    let whole = [if pass == 0 { *record } else { *next }];
                    let list: &[Draw] = match &clip {
                        Some(clip) => {
                            let pieces = match first {
                                Some(pieces) => reclip_second_pass(
                                    clip,
                                    indices,
                                    &*record,
                                    &whole[0],
                                    pieces,
                                    (p.flags & flags::ALPHA_NO_PRIM == 0).then_some(alpha),
                                ),
                                _ => clip_float_uv(clip, indices, &whole[0]),
                            };
                            first = Some(pieces);
                            pieces.unwrap_or(&whole)
                        }
                        None => &whole,
                    };
                    for d in list {
                        d.draw();
                    }
                    n += 1;
                }
                st.draws += n;
                tris += p.index_count / 3;
                continue;
            }

            let whole = [*record];
            let draws: &[Draw] = match &clip {
                Some(clip) => clip_submission(clip, indices, &whole[0]).unwrap_or(&whole),
                None => &whole,
            };
            if takes_spare(p.flags) {
                apply_lod_blend_translucent_pass1(pack, &p, st, None, effect_mat_anim);
            }
            let submit_t = crate::profile::start_fine();
            for d in draws {
                d.draw();
            }
            crate::profile::stop_fine(crate::profile::Span::Submit, submit_t);
            crate::profile::count(crate::profile::Span::Prims, 1);
            st.draws += 1;
            if p.flags & flags::LOD_BLEND != 0
                && draw_lod_blend_pass(pack, &p, st, None, effect_mat_anim, draws)
            {
                st.draws += 1;
            }
            tris += p.index_count / 3;
        }
        st.triangles += tris;
        st.texture_part_mesh = None;
    }
}

/// [`clip_submission`] of a texel-alpha blend's second pass, `whole`, from
/// its first pass's, `first_whole`, which drew `first` (RE-321): the two
/// expansions differ only in their alpha, the prim alpha `alpha` (`None`:
/// `ALPHA_NO_PRIM`, the same colours), the same in every corner. The same
/// positions and outcodes cross the same triangles into the same pieces, and
/// a clip point's channels are its corners' interpolated, so a constant
/// alpha stays that alpha. The first pass's pieces are re-based onto the
/// second's submission and its clipped corners copied with the second's
/// alpha into a block as `rebuild_runs` allocates it: the same vertices,
/// without decoding and clipping them again.
#[inline(never)]
unsafe fn reclip_second_pass(
    clip: &GuardClip<'_>,
    indices: &[u8],
    first_whole: &Draw,
    whole: &Draw,
    first: Option<&'static [Draw]>,
    alpha: Option<u8>,
) -> Option<&'static [Draw]> {
    // No triangle crossed, or a format `rebuild_runs` does not read.
    let count = first?.len();
    let tris = whole.count as usize / 3;
    // At most a run and a fan for each triangle, then one more run:
    // `rebuild_runs` never overflows into `rebuild_clipped`. The outcodes
    // in their table, as the first pass read them.
    let (Some(table), true) = (clip.codes, 2 * tris < CLIP_DRAWS_MAX) else {
        return clip_submission(clip, indices, whole);
    };
    // `crossing_triangles` over the corners in one loop.
    let (mut crossing, mut corner, mut any, mut all) = (0, 0, 0u8, !0u8);
    for c in 0..3 * tris {
        let i = usize::from(u16::from_le_bytes([
            *indices.get_unchecked(2 * c),
            *indices.get_unchecked(2 * c + 1),
        ]));
        let code = match table.get(i) {
            Some(&code) => code,
            None => clip.vertex_code(i),
        };
        any |= code;
        all &= code;
        corner += 1;
        if corner == 3 {
            crossing += usize::from(any != 0 && all == 0);
            (corner, any, all) = (0, 0, !0);
        }
    }
    let size = core::mem::size_of::<ClipVertex>();
    let capacity = crossing * 3 * (ssb_engine::guard_clip::MAX_CLIPPED - 2);
    let out = crate::gu::cached_vertices(capacity * size) as *mut ClipVertex;
    // The second pass's alpha over the first's, or the same colours.
    let (keep, set) = match alpha {
        Some(a) => (0x00FF_FFFF, u32::from(a) << 24),
        None => (u32::MAX, 0),
    };
    let rebase = |at: *const c_void, from: *const c_void, to: *const c_void| {
        (to as usize).wrapping_add((at as usize).wrapping_sub(from as usize)) as *const c_void
    };
    // SAFETY: only this primitive's draws read the pieces, on the one
    // thread that draws; the first pass has sent its draws.
    let draws = &mut *core::ptr::addr_of_mut!(CLIP_DRAWS);
    // The fans fill the block from its start, in order.
    let mut n = 0usize;
    let draws = draws.get_unchecked_mut(..count);
    for d in draws.iter_mut() {
        if d.vtype == CLIP_VERTEX_FORMAT.bits() {
            let fan = out.add(n);
            let count = d.count as usize;
            core::ptr::copy_nonoverlapping(d.vertex as *const ClipVertex, fan, count);
            d.vertex = fan as *const c_void;
            n += count;
            for v in core::slice::from_raw_parts_mut(fan, count) {
                v.color = v.color & keep | set;
            }
        } else {
            d.index = rebase(d.index, first_whole.index, whole.index);
            d.vertex = rebase(d.vertex, first_whole.vertex, whole.vertex);
        }
    }
    crate::gu::writeback_vertices(out as *const u8, n * size, false);
    Some(draws)
}

/// [`clip_submission`] of a float-UV submission, a texel-alpha blend's
/// first pass (the Fall's sheets, RE-321): `rebuild_runs` with
/// [`ssb_engine::guard_clip::clip_triangle_runs`], its polygon clip and
/// `decode_corner`/`encode_corner` copied in for the two float-UV formats
/// and the mesh's outcode table, so one block of code here does what runs
/// through the shared clip code (RE-585). The same float operations in the
/// same order, the same pieces in [`CLIP_DRAWS`] and the same
/// [`crate::gu::cached_vertices`] block: the same draws. Any other
/// submission, or a corner past the table, goes to [`clip_submission`]; so
/// does a polygon past `MAX_CLIPPED` corners, where `clip_polygon`'s bounds
/// check panics: the same triangle then panics there, and this function
/// holds no panic of its own (no `.rodata`: RE-584's `.rodata` offsets).
#[inline(never)]
unsafe fn clip_float_uv(
    clip: &GuardClip<'_>,
    indices: &[u8],
    whole: &Draw,
) -> Option<&'static [Draw]> {
    use ssb_engine::guard_clip::{ATTRS, MAX_CLIPPED};
    const PLANES: usize = MAX_CLIPPED - 3;
    const POOL: usize = 3 + PLANES * MAX_CLIPPED;
    const MAX_TRIS: usize = (CLIP_DRAWS_MAX - 1) / 2;
    let vtype = whole.vtype;
    let indexed = vtype == INDEXED_FLOAT_UV_VERTEX_FORMAT.bits();
    let tris = whole.count as usize / 3;
    let (Some(table), true, Some(corners)) = (
        clip.codes,
        (indexed || vtype == FLOAT_UV_VERTEX_FORMAT.bits()) && tris <= MAX_TRIS,
        indices.get(..tris * 6),
    ) else {
        return clip_submission(clip, indices, whole);
    };
    // Every corner's outcode from the table, and `crossing_triangles`.
    let (mut crossing, mut any, mut all) = (0usize, 0u8, !0u8);
    for c in 0..3 * tris {
        let i = usize::from(u16::from_le_bytes([
            *corners.get_unchecked(2 * c),
            *corners.get_unchecked(2 * c + 1),
        ]));
        let Some(&code) = table.get(i) else {
            return clip_submission(clip, indices, whole);
        };
        any |= code;
        all &= code;
        if c % 3 == 2 {
            crossing += usize::from(any != 0 && all == 0);
            (any, all) = (0, !0);
        }
    }
    if crossing == 0 {
        return None;
    }
    let band = clip.band;
    let mvp = &clip.mvp;
    let capacity = crossing * 3 * (MAX_CLIPPED - 2);
    let out = crate::gu::cached_vertices(capacity * core::mem::size_of::<ClipVertex>())
        as *mut ClipVertex;
    // SAFETY: only this primitive's draws read the pieces, on the one
    // thread that draws. At most a run and a fan per triangle, then one
    // more run: `MAX_TRIS` keeps them inside.
    let draws = &mut *core::ptr::addr_of_mut!(CLIP_DRAWS);
    let mut pieces = 0usize;
    let mut n = 0usize;
    let mut block = 0usize;
    let size = core::mem::size_of::<FloatUvVertex>();
    let stride = if indexed {
        3 * core::mem::size_of::<u16>()
    } else {
        3 * size
    };
    // Triangle `t`'s corners' outcodes, ORed and ANDed: every index is
    // inside the table (checked above).
    let tri_codes = |t: usize| {
        let mut or_and = (0u8, !0u8);
        for c in 3 * t..3 * t + 3 {
            let i = usize::from(u16::from_le_bytes([
                *corners.get_unchecked(2 * c),
                *corners.get_unchecked(2 * c + 1),
            ]));
            let code = *table.get_unchecked(i);
            or_and = (or_and.0 | code, or_and.1 & code);
        }
        or_and
    };
    let mut run: Option<usize> = None;
    for t in 0..=tris {
        // `Piece::Run`: the triangles before a crossing or dropped one,
        // and those at the end.
        let (or, and) = if t < tris { tri_codes(t) } else { (1, 0) };
        let inside = or == 0;
        if inside {
            run.get_or_insert(t);
            continue;
        }
        if let Some(first) = run.take() {
            if n > block {
                *draws.get_unchecked_mut(pieces) = Draw {
                    vtype: CLIP_VERTEX_FORMAT.bits(),
                    index: core::ptr::null(),
                    vertex: out.add(block) as *const c_void,
                    count: (n - block) as u32,
                };
                pieces += 1;
                block = n;
            }
            let skip = first * stride;
            *draws.get_unchecked_mut(pieces) = Draw {
                vtype,
                index: if indexed {
                    (whole.index as *const u8).add(skip) as *const c_void
                } else {
                    whole.index
                },
                vertex: if indexed {
                    whole.vertex
                } else {
                    (whole.vertex as *const u8).add(skip) as *const c_void
                },
                count: (3 * (t - first)) as u32,
            };
            pieces += 1;
        }
        if t == tris {
            break;
        }
        if and != 0 {
            continue;
        }
        let k = 3 * t;
        // `decode_corner` for the float-UV formats, then `clip_position`.
        let tri = [
            float_uv_corner(whole, indexed, mvp, k),
            float_uv_corner(whole, indexed, mvp, k + 1),
            float_uv_corner(whole, indexed, mvp, k + 2),
        ];
        // `clip_polygon`.
        let outcode = |c: [f32; 4]| {
            let gw = band * c[3];
            u8::from(gw - c[0] < 0.0)
                | u8::from(gw + c[0] < 0.0) << 1
                | u8::from(gw - c[1] < 0.0) << 2
                | u8::from(gw + c[1] < 0.0) << 3
                | u8::from(c[2] + c[3] < 0.0) << 4
        };
        let tc = tri.map(|v| outcode(v.clip));
        if tc[0] & tc[1] & tc[2] != 0 {
            continue;
        }
        let union = tc[0] | tc[1] | tc[2];
        let mut pool = core::mem::MaybeUninit::<[ClipCorner; POOL]>::uninit();
        let base = pool.as_mut_ptr().cast::<ClipCorner>();
        base.write(tri[0]);
        base.add(1).write(tri[1]);
        base.add(2).write(tri[2]);
        let mut used = 3usize;
        let mut polys = [[0u8; MAX_CLIPPED]; 2];
        polys[0][..3].copy_from_slice(&[0, 1, 2]);
        let mut m = 3;
        let mut src = 0;
        let mut dropped = false;
        // The planes `union` has, in order: a loop the compiler does not
        // unroll into one copy per plane.
        let mut planes = union;
        while planes != 0 {
            // SAFETY: `m` counts the writes to `b` below, each at most
            // `MAX_CLIPPED`.
            if m > MAX_CLIPPED {
                core::hint::unreachable_unchecked();
            }
            let plane = planes.trailing_zeros();
            planes &= planes - 1;
            let (first, second) = polys.split_at_mut(1);
            let (a, b) = if src == 0 {
                (&first[0], &mut second[0])
            } else {
                (&second[0], &mut first[0])
            };
            // `distance`.
            let mut d = [0.0f32; MAX_CLIPPED];
            for (d, &i) in d.iter_mut().zip(&a[..m]) {
                let c = (*base.add(usize::from(i))).clip;
                let gw = band * c[3];
                // Planes 0-3 as `x` or `y` (`plane / 2`), minus or plus
                // (`plane % 2`): no jump table.
                *d = if plane < 4 {
                    let x = c[(plane >> 1) as usize];
                    if plane & 1 == 0 {
                        gw - x
                    } else {
                        gw + x
                    }
                } else {
                    c[2] + c[3]
                };
            }
            let mut o = 0;
            for i in 0..m {
                let j = if i + 1 == m { 0 } else { i + 1 };
                // SAFETY: `i` and `j` are below `m`.
                let (dc, dn) = (*d.get_unchecked(i), *d.get_unchecked(j));
                let (ai, aj) = (*a.get_unchecked(i), *a.get_unchecked(j));
                if dc >= 0.0 {
                    if o == MAX_CLIPPED {
                        return clip_submission(clip, indices, whole);
                    }
                    *b.get_unchecked_mut(o) = ai;
                    o += 1;
                }
                if (dc >= 0.0) != (dn >= 0.0) {
                    if o == MAX_CLIPPED {
                        return clip_submission(clip, indices, whole);
                    }
                    *b.get_unchecked_mut(o) = used as u8;
                    // `lerp`.
                    let (va, vb) = (&*base.add(usize::from(ai)), &*base.add(usize::from(aj)));
                    let t = dc / (dc - dn);
                    let mut v = *va;
                    for q in 0..4 {
                        v.clip[q] = va.clip[q] + (vb.clip[q] - va.clip[q]) * t;
                    }
                    for q in 0..ATTRS {
                        v.attrs[q] = va.attrs[q] + (vb.attrs[q] - va.attrs[q]) * t;
                    }
                    base.add(used).write(v);
                    used += 1;
                    o += 1;
                }
            }
            src ^= 1;
            m = o;
            if m < 3 {
                dropped = true;
                break;
            }
        }
        if dropped {
            continue;
        }
        // SAFETY: as above.
        if m > MAX_CLIPPED {
            core::hint::unreachable_unchecked();
        }
        // `Piece::Fan`: each polygon vertex encoded once (`encode_corner`),
        // then copied into every fan triangle that uses it.
        let byte = |x: f32| (x + 0.5).clamp(0.0, 255.0) as u8;
        let normal = |x: f32| (x + if x < 0.0 { -0.5 } else { 0.5 }).clamp(-128.0, 127.0) as i8;
        let mut encoded = core::mem::MaybeUninit::<[ClipVertex; MAX_CLIPPED]>::uninit();
        let encoded = encoded.as_mut_ptr().cast::<ClipVertex>();
        for (e, &i) in polys[src][..m].iter().enumerate() {
            let a = &(*base.add(usize::from(i))).attrs;
            encoded.add(e).write(ClipVertex {
                u: a[0],
                v: a[1],
                color: u32::from_le_bytes([byte(a[2]), byte(a[3]), byte(a[4]), byte(a[5])]),
                nx: normal(a[6]),
                ny: normal(a[7]),
                nz: normal(a[8]),
                _normal_pad: 0,
                x: a[9],
                y: a[10],
                z: a[11],
            });
        }
        for i in 1..m - 1 {
            for k in [0, i, i + 1] {
                out.add(n).write(*encoded.add(k));
                n += 1;
            }
        }
    }
    if n > block {
        *draws.get_unchecked_mut(pieces) = Draw {
            vtype: CLIP_VERTEX_FORMAT.bits(),
            index: core::ptr::null(),
            vertex: out.add(block) as *const c_void,
            count: (n - block) as u32,
        };
        pieces += 1;
    }
    crate::gu::writeback_vertices(
        out as *const u8,
        n * core::mem::size_of::<ClipVertex>(),
        false,
    );
    Some(draws.get_unchecked(..pieces))
}

/// [`batch_submit_lod_tag`] for one object's `count` meshes from `first`,
/// or [`super::batch_submit`] when none of their primitives is a two-tile
/// blend (`LOD_BLEND`): for those, [`batch_submit_lod_tag`] takes
/// [`super::batch_submit`]'s steps (`material_inputs` and [`lod_material`]
/// are `batch_material`, the clip and the draws the same; no second-pass
/// record, so `spares` stays), and the comets sharing the Fall's tag then
/// run the comets' code (RE-585).
///
/// # Safety
///
/// As [`batch_submit_lod_tag`].
#[inline(never)]
pub(crate) unsafe fn batch_submit_lod_object(
    pack: &Pack<'_>,
    st: &mut DrawState,
    batch: &Batch,
    first: usize,
    count: usize,
    effect_mat_anim: Option<&EffectMaterialAnimator>,
    spares: &mut usize,
) {
    let mut lod = false;
    for k in first..first + count {
        let m = &*batch.meshes.add(k);
        for i in 0..m.prim_count {
            if let Some(p) = pack.prim(m.first_prim + i) {
                lod |= p.flags & flags::LOD_BLEND != 0;
            }
        }
    }
    if lod {
        batch_submit_lod_tag(pack, st, batch, first, count, effect_mat_anim, spares);
    } else {
        super::batch_submit(pack, st, batch, first, count, effect_mat_anim);
    }
}

/// [`clip_float_uv`]'s `guard_clip::Vertex`: clip position, then
/// attributes.
#[derive(Clone, Copy)]
struct ClipCorner {
    clip: [f32; 4],
    attrs: [f32; ssb_engine::guard_clip::ATTRS],
}

/// [`clip_float_uv`]'s corner `c` of `whole`: `decode_corner` for the
/// float-UV formats, then `clip_position`. Out of line: one copy for the
/// three corners.
#[inline(never)]
unsafe fn float_uv_corner(
    whole: &Draw,
    indexed: bool,
    mvp: &[[f32; 4]; 4],
    c: usize,
) -> ClipCorner {
    let v = if indexed {
        let index = (whole.index as *const u16).add(c).read_unaligned();
        (whole.vertex as *const FloatUvVertex)
            .wrapping_add(usize::from(index))
            .read_unaligned()
    } else {
        (whole.vertex as *const FloatUvVertex)
            .add(c)
            .read_unaligned()
    };
    let col = v.color.to_le_bytes();
    let attrs = [
        v.u,
        v.v,
        f32::from(col[0]),
        f32::from(col[1]),
        f32::from(col[2]),
        f32::from(col[3]),
        f32::from(v.nx),
        f32::from(v.ny),
        f32::from(v.nz),
        f32::from(v.x) / VERTEX_16BIT_DIVISOR,
        f32::from(v.y) / VERTEX_16BIT_DIVISOR,
        f32::from(v.z) / VERTEX_16BIT_DIVISOR,
    ];
    let p = [attrs[9], attrs[10], attrs[11]];
    let row = |r: usize| mvp[0][r] * p[0] + mvp[1][r] * p[1] + mvp[2][r] * p[2] + mvp[3][r];
    ClipCorner {
        clip: [row(0), row(1), row(2), row(3)],
        attrs,
    }
}

/// [`super::super::draw_lod_texel_alpha`]'s choice for a primitive of an
/// effect whose material player is `effect_mat_anim` (no stage player):
/// the two passes' alphas, `batch_material`'s inputs for the first and
/// `bind_lod_next`'s for `TEXEL1`, or [`TexelPlan::SINGLE`] to leave the
/// primitive to the single pass. Its text, with
/// [`ssb_rom::lod_blend::texel_alpha_pass_alphas`] copied in (its only
/// other caller inlines it, RE-494) and checked indexing.
#[inline(never)]
unsafe fn texel_alpha_plan(
    pack: &Pack<'_>,
    p: &PrimDesc,
    st: &DrawState,
    indices: &[u8],
    verts: &[u8],
    effect_colors: Option<EffectColors>,
    effect_mat_anim: Option<&EffectMaterialAnimator>,
) -> TexelPlan {
    let alphas = || {
        let lod = effect_mat_anim?.resolved_lod_blend(pack, p.mat_anim)?;
        let mean = lod.texel_alpha?;
        let base = effect_colors.unwrap_or_default();
        if base.prim.is_none() && p.flags & flags::FLAT_COLOR != 0 {
            return None;
        }
        let prim_alpha = match base.prim {
            Some(prim) => prim[3],
            None => {
                let size = core::mem::size_of::<PackedVertex>();
                let first = usize::from(u16::from_le_bytes([*indices.first()?, *indices.get(1)?]));
                let v = verts.get(first * size..(first + 1) * size)?;
                (*(v.as_ptr() as *const PackedVertex)).color.to_le_bytes()[3]
            }
        };
        let (v0, v1) = match st.lod_blend_isolate {
            Some(LodBlendIsolate::Current) => (prim_alpha, 0),
            Some(LodBlendIsolate::Next) => (0, prim_alpha),
            None => {
                // `texel_alpha_pass_alphas(prim_alpha, lod.frac, mean)`.
                let fa = ((u32::from(lod.frac) * 255 + 128) / 256) as u8;
                let fb = 255 - fa;
                let a = u32::from(prim_alpha);
                let v0 = (a * u32::from(fb) + 127) / 255;
                if fa == 0 {
                    (v0 as u8, 0)
                } else {
                    // Never 0: `fa > 0` keeps `v0` under 255.
                    let den = 65025 - v0 * u32::from(mean);
                    let v1 = (a * u32::from(fa) * 255 + den / 2)
                        .checked_div(den)
                        .unwrap_or(0);
                    (v0 as u8, v1.min(255) as u8)
                }
            }
        };
        Some((lod, v0, v1))
    };
    let Some((lod, v0, v1)) = alphas() else {
        return TexelPlan::SINGLE;
    };
    let (texture, palette, uv) = material_lookups(pack, p, effect_mat_anim);
    // Both tiles' rows from one lookup site, which keeps `Pack::texture`
    // inlined.
    let mut rows = [None; 2];
    for (row, i) in rows.iter_mut().zip([texture, lod.texture]) {
        *row = pack.texture(i);
    }
    // `apply_texture_mapping`.
    let affine = match (uv, rows[0]) {
        (Some(uv), Some(t)) => animated_uv_affine(uv, &t),
        _ => UvAffine::IDENTITY,
    };
    // RE-321: tile 1's live window, re-based from tile 0's static origin.
    let next_affine = match rows[1] {
        Some(t) => lod.ge_affine(
            ssb_rom::psp_texture::ge_texture_dims(t.width as u32, t.height as u32),
            (
                t.wrap & TextureDesc::CLAMP_S != 0,
                t.wrap & TextureDesc::CLAMP_T != 0,
            ),
        ),
        None => UvAffine::IDENTITY,
    };
    TexelPlan {
        two_pass: true,
        v0,
        v1,
        texture,
        palette,
        affine,
        next: lod.texture,
        next_affine,
    }
}

/// `batch_material`'s lookups for primitive `p` with no texture override:
/// the texture, the palette and the animated window's mapping it binds.
#[inline(always)]
fn material_inputs(
    pack: &Pack<'_>,
    p: &PrimDesc,
    effect_mat_anim: Option<&EffectMaterialAnimator>,
) -> (u32, u32, UvAffine) {
    let (texture, palette, uv) = material_lookups(pack, p, effect_mat_anim);
    // `apply_texture_mapping`.
    let affine = match uv.and_then(|uv| Some((uv, pack.texture(texture)?))) {
        Some((uv, t)) => animated_uv_affine(uv, &t),
        None => UvAffine::IDENTITY,
    };
    (texture, palette, affine)
}

/// [`material_inputs`] up to the texture row: the texture, the palette and
/// the animated window that owns tile 0, if any.
#[inline(always)]
fn material_lookups(
    pack: &Pack<'_>,
    p: &PrimDesc,
    effect_mat_anim: Option<&EffectMaterialAnimator>,
) -> (u32, u32, Option<MaterialUv>) {
    let texture = match p.image_anim() {
        Some(anim) => effect_mat_anim
            .and_then(|m| m.resolved_texture(pack, anim))
            .unwrap_or(p.texture),
        None => p.texture,
    };
    let palette = p
        .palette_anim()
        .and_then(|anim| effect_mat_anim.and_then(|m| m.resolved_palette(pack, anim)))
        .unwrap_or(TextureDesc::NO_ANIM);
    let uv = (p.mat_anim != TextureDesc::NO_ANIM)
        .then(|| effect_mat_anim.and_then(|m| m.resolved_uv(pack, p.mat_anim)))
        .flatten()
        .and_then(|uv| uv.owned_by(p.flags));
    (texture, palette, uv)
}

/// `batch_material` for [`material_inputs`]' answer: its state and binds,
/// in its order.
#[inline(always)]
unsafe fn lod_material(
    pack: &Pack<'_>,
    p: &PrimDesc,
    st: &mut DrawState,
    texture: u32,
    palette: u32,
    affine: UvAffine,
    effect_colors: Option<EffectColors>,
) {
    if let Some(on) = st.env_lerp_fog {
        let want = p.flags & flags::ENV_LERP != 0;
        if want != on {
            if want {
                sys::sceGuEnable(GuState::Fog);
            } else {
                sys::sceGuDisable(GuState::Fog);
            }
            st.env_lerp_fog = Some(want);
        }
    }

    if st.fade_alpha.is_some() {
        st.last_flags = None;
    }
    if st.last_flags != Some(p.flags) {
        apply_flags_state(p, st);
    }
    if let Some(a) = st.fade_alpha {
        apply_fade_blend(a);
    }

    if st.last_texture != Some((texture, palette)) {
        st.last_texture = Some((texture, palette));
        st.state_changes += 1;
        if !bind_texture_cached(pack, texture, palette) {
            match pack.texture(texture) {
                Some(t) => bind_texture_uncached(pack, texture, &t, palette),
                None => sys::sceGuDisable(GuState::Texture2D),
            }
        }
    }

    apply_texture_affine(pack, p, st, texture, affine);

    let blend_target = (p.flags & flags::TEXTURE_BLEND != 0).then_some(
        effect_colors
            .and_then(|c| c.packed_prim())
            .unwrap_or(p.texture_blend_target),
    );
    let desired_texture_func = match blend_target {
        Some(target) => TextureFuncState::Blend(target),
        None => TextureFuncState::Modulate,
    };
    if st.last_texture_func != Some(desired_texture_func) {
        st.last_texture_func = Some(desired_texture_func);
        st.state_changes += 1;
        match desired_texture_func {
            TextureFuncState::Blend(target) => {
                sys::sceGuTexFunc(sys::TextureEffect::Blend, sys::TextureColorComponent::Rgba);
                sys::sceGuTexEnvColor(target);
            }
            TextureFuncState::Modulate => {
                sys::sceGuTexFunc(
                    sys::TextureEffect::Modulate,
                    sys::TextureColorComponent::Rgba,
                );
            }
        }
    }
}

/// [`texel_alpha_plan`] for [`batch_expand_lod_tag`], with the material
/// player's lookups of the primitive's one `mat_anim` merged: one joint
/// search, one `MatAnimDesc` read and each track evaluated once, where
/// `resolved_lod_blend`, `resolved_texture`, `resolved_palette` and
/// `resolved_uv` each search, read and evaluate their own. The same values,
/// so the same plan. Its callees are out of line already or always-inlined
/// copies, so no other function's inlining changes (RE-585). A primitive
/// whose `mat_anim` this player does not tick takes [`texel_alpha_plan`].
#[inline(never)]
unsafe fn merged_texel_alpha_plan(
    pack: &Pack<'_>,
    p: &PrimDesc,
    st: &DrawState,
    indices: &[u8],
    verts: &[u8],
    effect_colors: Option<EffectColors>,
    effect_mat_anim: Option<&EffectMaterialAnimator>,
) -> TexelPlan {
    use ssb_rom::matanim::{
        TRACK_PALETTE_ID, TRACK_SCA_U, TRACK_SCA_V, TRACK_SCR_U, TRACK_SCR_V, TRACK_SET_LFRAC,
        TRACK_TEXTURE_ID_CURRENT, TRACK_TEXTURE_ID_NEXT, TRACK_TRA_U, TRACK_TRA_V,
    };
    let Some(m) = effect_mat_anim else {
        return TexelPlan::SINGLE;
    };
    let mat = p.mat_anim;
    let Some(j) = m.ticked_joint(mat) else {
        // An opaque player: that this call site has one must not reach
        // [`texel_alpha_plan`]'s code, which stays as it was.
        use core::hint::black_box as opaque;
        return texel_alpha_plan(
            pack,
            p,
            st,
            indices,
            verts,
            effect_colors,
            opaque(effect_mat_anim),
        );
    };
    // `resolve_lod_blend`.
    let Some(desc) = pack.lod_blend_inlined(mat) else {
        return TexelPlan::SINGLE;
    };
    let Some(a) = pack.mat_anim(mat) else {
        return TexelPlan::SINGLE;
    };
    if desc.next_count == 0 {
        return TexelPlan::SINGLE;
    }
    let mut live = [None; TRACK_SET_LFRAC + 1];
    for (t, v) in live.iter_mut().enumerate() {
        *v = j.track_value_inlined(t);
    }
    let base = |i: usize| f32::from_bits(a.base_tracks[i]);
    let lfrac = match live[TRACK_SET_LFRAC] {
        Some(v) => v,
        None => base(TRACK_SET_LFRAC),
    };
    let next = match live[TRACK_TEXTURE_ID_NEXT] {
        Some(v) => v.max(0.0) as u32,
        None => 0,
    };
    let next_texture = desc.next_textures[next.min(desc.next_count - 1) as usize];
    if next_texture == TextureDesc::NO_ANIM {
        return TexelPlan::SINGLE;
    }
    let pick = |t: usize| match live[t] {
        Some(v) => v,
        None => base(t),
    };
    let moves = live[TRACK_SCA_U].is_some()
        || live[TRACK_SCA_V].is_some()
        || live[TRACK_SCR_U].is_some()
        || live[TRACK_SCR_V].is_some();
    let lod_uv = if a.uv_mode != 0 && moves {
        Some(MaterialUv {
            trau: pick(TRACK_SCR_U),
            trav: pick(TRACK_SCR_V),
            scau: pick(TRACK_SCA_U),
            scav: pick(TRACK_SCA_V),
            base_trau: base(TRACK_SCR_U),
            base_trav: base(TRACK_SCR_V),
            base_scau: base(TRACK_SCA_U),
            base_scav: base(TRACK_SCA_V),
            mode: 1 | (a.uv_mode & (4 | u32::from(ssb_rom::mobj::UV_MODE_HALF))),
            tile_bias: desc.tile1_params[0] as f32,
            tile_width: desc.tile1_params[1] as f32,
            tile_height: desc.tile1_params[2] as f32,
            half: [f32::from_bits(a.uv_half[2]), f32::from_bits(a.uv_half[1])],
        })
    } else {
        None
    };
    let lod = LodBlendState {
        texture: next_texture,
        // `prim_lod_frac`.
        frac: ((lfrac * 255.0) as u32 & 0xFF) as u8,
        uv: lod_uv,
        origin_delta: desc.origin_delta,
        texel_alpha: desc.texel_alpha,
    };
    // [`texel_alpha_plan`]'s `alphas`.
    let Some(mean) = lod.texel_alpha else {
        return TexelPlan::SINGLE;
    };
    let prim = match effect_colors {
        Some(c) => c.prim,
        None => None,
    };
    let prim_alpha = match prim {
        Some(prim) => prim[3],
        None => {
            if p.flags & flags::FLAT_COLOR != 0 {
                return TexelPlan::SINGLE;
            }
            let size = core::mem::size_of::<PackedVertex>();
            let (Some(&i0), Some(&i1)) = (indices.first(), indices.get(1)) else {
                return TexelPlan::SINGLE;
            };
            let first = usize::from(u16::from_le_bytes([i0, i1]));
            let Some(v) = verts.get(first * size..(first + 1) * size) else {
                return TexelPlan::SINGLE;
            };
            (*(v.as_ptr() as *const PackedVertex)).color.to_le_bytes()[3]
        }
    };
    let (v0, v1) = match st.lod_blend_isolate {
        Some(LodBlendIsolate::Current) => (prim_alpha, 0),
        Some(LodBlendIsolate::Next) => (0, prim_alpha),
        None => {
            let fa = ((u32::from(lod.frac) * 255 + 128) / 256) as u8;
            let fb = 255 - fa;
            let a = u32::from(prim_alpha);
            let v0 = (a * u32::from(fb) + 127) / 255;
            if fa == 0 {
                (v0 as u8, 0)
            } else {
                let den = 65025 - v0 * u32::from(mean);
                let v1 = (a * u32::from(fa) * 255 + den / 2)
                    .checked_div(den)
                    .unwrap_or(0);
                (v0 as u8, v1.min(255) as u8)
            }
        }
    };
    // `material_lookups`: `PrimDesc::image_anim` and `resolve_texture`.
    let mut texture = p.texture;
    if p.flags & flags::IMAGE_ANIM != 0 && a.texture_count != 0 {
        if let Some(value) = live[TRACK_TEXTURE_ID_CURRENT] {
            let index = (value.max(0.0) as u32).min(a.texture_count - 1);
            let t = a.textures[index as usize];
            if t != TextureDesc::NO_ANIM {
                texture = t;
            }
        }
    }
    // `PrimDesc::palette_anim` and `resolve_palette`.
    let mut palette = TextureDesc::NO_ANIM;
    if p.flags & flags::PALETTE_ANIM != 0 && a.palette_count != 0 {
        if let Some(v) = j.track_value_inlined(TRACK_PALETTE_ID) {
            // `i32::clamp`, written out: its bounds are ordered here.
            let (x, top) = (v as i32, a.palette_count as i32 - 1);
            let index = if x < 0 {
                0
            } else if x > top {
                top as u32
            } else {
                x as u32
            };
            palette = a.first_palette + index;
        }
    }
    // `resolve_uv`, then `MaterialUv::owned_by`.
    let mut uv = None;
    if a.uv_mode != 0
        && (live[TRACK_TRA_U].is_some()
            || live[TRACK_TRA_V].is_some()
            || live[TRACK_SCA_U].is_some()
            || live[TRACK_SCA_V].is_some())
    {
        let mut mode = a.uv_mode;
        if p.flags & flags::TILE0_ANIM == 0 {
            mode &= !3;
        }
        if p.flags & flags::SCALE_ANIM == 0 {
            mode &= !4;
        }
        if mode & 7 != 0 {
            uv = Some(MaterialUv {
                trau: pick(TRACK_TRA_U),
                trav: pick(TRACK_TRA_V),
                scau: pick(TRACK_SCA_U),
                scav: pick(TRACK_SCA_V),
                base_trau: base(TRACK_TRA_U),
                base_trav: base(TRACK_TRA_V),
                base_scau: base(TRACK_SCA_U),
                base_scav: base(TRACK_SCA_V),
                mode,
                tile_bias: a.uv_tile_params[0] as f32,
                tile_width: a.uv_tile_params[1] as f32,
                tile_height: a.uv_tile_params[2] as f32,
                half: [f32::from_bits(a.uv_half[0]), f32::from_bits(a.uv_half[1])],
            });
        }
    }
    // [`texel_alpha_plan`] from here.
    let row0 = pack.texture_inlined(texture);
    let row1 = pack.texture_inlined(lod.texture);
    let affine = match (uv, row0) {
        (Some(uv), Some(t)) => animated_uv_affine(uv, &t),
        _ => UvAffine::IDENTITY,
    };
    let next_affine = match row1 {
        Some(t) => {
            // `ge_texture_dims`.
            let dim = |v: u16| {
                u32::from(v)
                    .max(1)
                    .next_power_of_two()
                    .min(ssb_rom::psp_texture::GE_MAX_TEXTURE_DIM)
            };
            lod.ge_affine_inlined(
                (dim(t.width), dim(t.height)),
                (
                    t.wrap & TextureDesc::CLAMP_S != 0,
                    t.wrap & TextureDesc::CLAMP_T != 0,
                ),
            )
        }
        None => UvAffine::IDENTITY,
    };
    TexelPlan {
        two_pass: true,
        v0,
        v1,
        texture,
        palette,
        affine,
        next: lod.texture,
        next_affine,
    }
}
