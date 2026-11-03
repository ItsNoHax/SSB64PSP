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
//! - a texel-alpha blend's pass alphas ([`texel_alpha_plan`]) read the
//!   effect's material player, its `color_override` and
//!   `lod_blend_isolate`, none of which a draw changes.
//!
//! `bind_lod_next`, `draw_lod_blend_pass`,
//! `apply_lod_blend_translucent_pass1` and `expand_float_uv_corners` are
//! out of line in the per-object path and shared;
//! `texel_alpha_pass_alphas` is inlined there and copied here (RE-494).

use core::ffi::c_void;

use psp::sys::{self, GuState};
use ssb_rom::pack::{flags, NodeDesc, ObjectDesc, Pack, PackedVertex, PrimDesc};
use ssb_rom::skeleton::{EffectColors, EffectMaterialAnimator, LodBlendState};

use super::super::{
    apply_lod_blend_translucent_pass1, bind_lod_next, clip_submission, draw_lod_blend_pass,
    expand_corners, expand_float_uv_corners, position_outcode, Draw, DrawState, GuardClip,
    LodBlendIsolate, CLIP_CODES, GUARD_BAND, LIGHT_PREPASS, LOD_BLEND_TRANSLUCENT, VERTEX_FORMAT,
};
use super::{batch_material, clip_transform, Batch, BatchSize};

/// [`super::UNBATCHED_FLAGS`] less the paths these passes add.
const LOD_UNBATCHED_FLAGS: u32 = flags::LIT | flags::TEXTURE_GEN | flags::TEXTURE_GEN_LINEAR;

/// Whether primitive flags `flags` take a second-pass record: a
/// translucent two-tile blend may draw as a texel-alpha blend's two passes.
#[inline(always)]
fn takes_spare(flags: u32) -> bool {
    flags & LOD_BLEND_TRANSLUCENT == LOD_BLEND_TRANSLUCENT
}

/// [`super::plan_object`] for these passes: [`LOD_UNBATCHED_FLAGS`] in
/// place of [`super::UNBATCHED_FLAGS`], and a second-pass record for each
/// primitive that may take one.
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
            size.prims += usize::from(takes_spare(p.flags));
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
/// material player.
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
    let table = &mut *core::ptr::addr_of_mut!(CLIP_CODES);
    let mut at = 0usize;
    let mut spare = batch.prims;
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
            if takes_spare(p.flags) {
                // `BatchSize::fits`: [`plan_object_lod`] counted it.
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
            if takes_spare(p.flags) {
                if let Some((_, v0, v1)) =
                    texel_alpha_plan(pack, &p, st, indices, verts, effect_colors, mats)
                {
                    let base = effect_colors.unwrap_or_default();
                    for (pass, alpha) in [(0usize, v0), (1, v1)] {
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
            if takes_spare(p.flags) {
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
            batch_material(pack, &p, st, effect_mat_anim, effect_colors);
            st.texture_override = None;
            let effect_colors = effect_colors.filter(|c| c.prim.is_some() || c.env.is_some());
            crate::profile::stop_fine(crate::profile::Span::Material, material_t);

            // RE-321: `draw_lod_texel_alpha` with [`batch_expand_lod_tag`]'s
            // expansions.
            if takes_spare(p.flags) {
                if let Some((lod, v0, v1)) =
                    texel_alpha_plan(pack, &p, st, indices, verts, effect_colors, effect_mat_anim)
                {
                    sys::sceGuDisable(GuState::AlphaTest);
                    st.last_flags = None;
                    st.state_changes += 1;
                    let mut n = 0;
                    for (pass, alpha) in [(0usize, v0), (1, v1)] {
                        if alpha == 0 {
                            continue;
                        }
                        if pass == 1 {
                            if !bind_lod_next(pack, &p, st, &lod) {
                                break;
                            }
                            sys::sceGuDepthMask(1);
                        }
                        let whole = [if pass == 0 { *record } else { *next }];
                        let list: &[Draw] = match &clip {
                            Some(clip) => {
                                clip_submission(clip, indices, &whole[0]).unwrap_or(&whole)
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

/// [`super::super::draw_lod_texel_alpha`]'s choice for a primitive of an
/// effect whose material player is `effect_mat_anim` (no stage player):
/// `TEXEL1`'s tile and the two passes' alphas, or `None` to leave the
/// primitive to the single pass. Its text, with
/// [`ssb_rom::lod_blend::texel_alpha_pass_alphas`] copied in (its only
/// other caller inlines it, RE-494) and checked indexing.
/// [`batch_expand_lod_tag`] and [`batch_submit_lod_tag`] ask it on the same
/// inputs.
#[inline(never)]
unsafe fn texel_alpha_plan(
    pack: &Pack<'_>,
    p: &PrimDesc,
    st: &DrawState,
    indices: &[u8],
    verts: &[u8],
    effect_colors: Option<EffectColors>,
    effect_mat_anim: Option<&EffectMaterialAnimator>,
) -> Option<(LodBlendState, u8, u8)> {
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
}
