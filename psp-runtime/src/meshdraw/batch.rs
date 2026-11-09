//! A stage-major [`super::draw_object_posed`] for many small objects under
//! one camera: the boss wallpaper's comets
//! ([`crate::boss::BossEffects::draw_batched`]).
//!
//! The per-object path runs every stage of one mesh before the next mesh:
//! node matrix, skin, outcodes, material, recolour, clip, draw. Its code is
//! larger than the PSP's 16 KB instruction cache, so 24 comets of three
//! meshes refill it for every mesh (RE-585). Here each stage runs over every
//! mesh of the camera before the next stage starts:
//!
//! 1. [`batch_pose`]: node matrices and skins (no GE command);
//! 2. [`batch_expand`]: outcodes and recoloured corners (no GE command);
//! 3. [`batch_submit`]: each mesh's matrix, materials, clips and draws, in
//!    the per-object path's order and state.
//!
//! The GE runs the per-object path's commands in its order; only the list
//! blocks the skins, corners and records come from are taken earlier, so
//! the jumps over them and the addresses the draws name differ, not the
//! pixels (D-051).
//!
//! What the passes reorder, checked against the per-object path:
//! - the skin reads the node's model matrix and the scene's light
//!   (`active_light`, `rsp_rows`), both fixed before [`batch_pose`];
//! - the outcodes read `clip_view_proj` and the node's
//!   model matrix, the value `set_model_matrix` gives `texgen_model`;
//! - `expand_corners` reads `DrawState` only for linear texgen
//!   (`texgen_object_basis`), which no batched primitive has;
//! - `material_colors` returns the object's `color_override`, which the
//!   boss sets for every effect, and reads no other state;
//! - `apply_material` and every other `DrawState` write run in
//!   [`batch_submit`], in the per-object path's order.
//!
//! Anything else takes the per-object path for the whole frame, decided
//! before anything is drawn or allocated ([`batch_ready`], [`plan_object`],
//! [`BatchSize::fits`]); a tag with the Fall's two-tile blends or float UVs
//! takes [`lod`]'s copies of the passes instead.
//!
//! The helpers the per-object path inlines from a single call site are
//! copied here, not called: a second caller would change how LLVM inlines
//! them into [`super::draw_mesh_vertices`] and move the ordered block
//! (RE-494). Only out-of-line helpers (`expand_corners`,
//! `clip_submission`, `bind_texture_uncached`, `apply_texture_affine`) are
//! shared.

pub(super) mod display;
pub(super) mod fighter;
mod lod;

use core::ffi::c_void;

use psp::sys::{self, GuState, ScePspFMatrix4, ScePspFVector4};
use ssb_rom::pack::{flags, NodeDesc, ObjectDesc, Pack, PackedVertex, PrimDesc, TextureDesc};
use ssb_rom::skeleton::{EffectColors, EffectMaterialAnimator};

use super::{
    animated_uv_affine, apply_fade_blend, apply_flags_state, apply_texture_affine,
    bind_texture_cached, bind_texture_uncached, clip_submission, compose_model_matrix,
    expand_corners, fpu_sqrt, light_rows, mat4_of, mat4_rows, tape_mode, Draw, DrawState,
    GuardClip, TapeMode, TextureFuncState, CLIP_CODES, CLIP_CODES_MAX, CLIP_RATIO,
    LIGHT_MARK_OFFSET, LIGHT_MARK_SET, LIGHT_PREPASS, VERTEX_FORMAT,
};

pub(crate) use lod::{batch_expand_lod_tag, batch_submit_lod_object, plan_object_lod};

/// Primitive flags whose paths the batch does not copy: lighting reads the
/// node's light ([`DrawState::object_light`]), the two-tile blends draw
/// more passes, texgen reads the model matrix and float UVs another format.
const UNBATCHED_FLAGS: u32 = flags::LIT
    | flags::LOD_BLEND
    | flags::TEXTURE_GEN
    | flags::TEXTURE_GEN_LINEAR
    | flags::SIGNED_CLAMP_UV;

/// Most bytes of mesh and primitive records one batch takes from the list:
/// 24 comets need about 10 KB (72 meshes of 128 bytes, 72 primitives of 16).
const RECORDS_MAX: usize = 16 * 1024;

/// What a batch holds, counted before anything is drawn.
#[derive(Clone, Copy, Default)]
pub(crate) struct BatchSize {
    meshes: usize,
    prims: usize,
    verts: usize,
}

impl BatchSize {
    fn bytes(&self) -> usize {
        self.meshes * core::mem::size_of::<BatchMesh>() + self.prims * core::mem::size_of::<Draw>()
    }

    /// Whether the batch fits its scratch: every mesh's outcodes in
    /// [`CLIP_CODES`] at once, and the records in [`RECORDS_MAX`].
    pub(crate) fn fits(&self) -> bool {
        self.meshes > 0 && self.verts <= CLIP_CODES_MAX && self.bytes() <= RECORDS_MAX
    }
}

/// Whether `st` lets a batch draw as the per-object path: no vertex tape
/// (a recording or replay takes its own submissions), no fighter light
/// (its prepass sets `node_light`), and no known scene lights (every
/// primitive would rewrite the light registers).
pub(crate) fn batch_ready(st: &DrawState) -> bool {
    matches!(*tape_mode(), TapeMode::Off)
        && !st.runtime_fighter_light
        && st.scene_lights != ssb_rom::anim_color::SceneLights::Known
}

/// Adds `object`, drawn as [`super::draw_object_posed`] draws it with
/// costume 0, to `size`; `false` when a node or primitive takes a path the
/// batch does not copy: a billboard, a mesh or vertex buffer the pack lacks,
/// or a primitive with [`UNBATCHED_FLAGS`].
pub(crate) fn plan_object(pack: &Pack<'_>, object: &ObjectDesc, size: &mut BatchSize) -> bool {
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
            if pack
                .prim(mesh.first_prim + j)
                .is_some_and(|p| p.flags & UNBATCHED_FLAGS != 0)
            {
                return false;
            }
        }
        size.meshes += 1;
        size.prims += mesh.prim_count as usize;
        size.verts += verts.len() / core::mem::size_of::<PackedVertex>();
    }
    true
}

/// One mesh of a batch: its node's model matrix and the vertices it draws.
#[repr(C)]
struct BatchMesh {
    model: ScePspFMatrix4,
    verts: *const u8,
    verts_len: usize,
    /// The object's `color_override`.
    colors: EffectColors,
    /// The mesh's primitives in the pack.
    first_prim: u32,
    prim_count: u32,
    /// Its first primitive's record.
    record: u32,
    /// Its first vertex's outcode in [`CLIP_CODES`].
    codes: u32,
    /// Whether a vertex lies outside the clip band ([`super::guard_clip`]).
    clips: bool,
}

/// A batch's records, in one block of list memory written through the
/// D-cache: a [`BatchMesh`] per mesh, then a [`Draw`] per primitive.
pub(crate) struct Batch {
    meshes: *mut BatchMesh,
    records: *mut Draw,
    len: usize,
    prims: usize,
    bytes: usize,
}

impl Batch {
    /// The records' block for `size`.
    ///
    /// # Safety
    ///
    /// Between `sceGuStart` and `sceGuFinish` of the direct list.
    pub(crate) unsafe fn new(size: &BatchSize) -> Self {
        let bytes = size.bytes();
        let block = crate::gu::cached_vertices(bytes);
        Batch {
            meshes: block as *mut BatchMesh,
            records: block.add(size.meshes * core::mem::size_of::<BatchMesh>()) as *mut Draw,
            len: 0,
            prims: 0,
            bytes,
        }
    }

    /// Writes the records' lines back and drops them from the D-cache: the
    /// next list writes this memory through the uncached alias, and a dirty
    /// line evicted later would overwrite its commands (RE-360).
    ///
    /// # Safety
    ///
    /// As [`Self::new`], in the same list.
    pub(crate) unsafe fn finish(self) {
        crate::gu::writeback_vertices(self.meshes as *const u8, self.bytes, false);
    }
}

/// Pass 1 for one object: each mesh node's model matrix `base * world`
/// and its skinned vertices, appended to `batch`; the number of meshes.
/// [`super::draw_object_posed_nodes`]' node loop and
/// [`super::draw_node_mesh`]'s skin without the GE writes, for a node
/// [`plan_object`] admitted.
///
/// # Safety
///
/// As [`Batch::new`]; `batch` sized by [`plan_object`] for this object.
#[allow(clippy::redundant_closure)]
#[inline(never)]
pub(crate) unsafe fn batch_pose(
    pack: &Pack<'_>,
    object: &ObjectDesc,
    base: &ScePspFMatrix4,
    posed: &[ssb_rom::scene::Mat4],
    st: &DrawState,
    colors: EffectColors,
    batch: &mut Batch,
) -> usize {
    let first = batch.len;
    let light = st.active_light();
    for i in 0..object.node_count {
        let global_node = object.first_node + i;
        let Some(node) = pack.node(global_node) else {
            continue;
        };
        if node.mesh == NodeDesc::NO_MESH {
            continue;
        }
        let node = match posed.get(i as usize) {
            Some(m) => NodeDesc { world: m.0, ..node },
            None => node,
        };
        let local = ScePspFMatrix4 {
            x: ScePspFVector4 {
                x: node.world[0],
                y: node.world[1],
                z: node.world[2],
                w: node.world[3],
            },
            y: ScePspFVector4 {
                x: node.world[4],
                y: node.world[5],
                z: node.world[6],
                w: node.world[7],
            },
            z: ScePspFVector4 {
                x: node.world[8],
                y: node.world[9],
                z: node.world[10],
                w: node.world[11],
            },
            w: ScePspFVector4 {
                x: node.world[12],
                y: node.world[13],
                z: node.world[14],
                w: node.world[15],
            },
        };
        let matrix_t = crate::profile::start_fine();
        let model = compose_model_matrix(base, &local);
        crate::profile::stop_fine(crate::profile::Span::NodeMatrix, matrix_t);
        let Some(mesh) = pack.mesh(node.mesh) else {
            continue;
        };
        let Some(mut verts) = pack.vertices(&mesh) else {
            continue;
        };
        if !posed.is_empty() {
            if let (Some(bindings), Some(inverse)) = (
                pack.vertex_bindings(&mesh),
                ssb_rom::scene::Mat4(node.world).inverse_affine(),
            ) {
                let skin = crate::profile::start_fine();
                let dynamic = crate::gu::cached_vertices(verts.len());
                let count = verts.len() / ssb_rom::pack::VERTEX_SIZE;
                let mut bound = bindings.chunks_exact(8);
                // The node's matrix, as `set_model_matrix` would hold it.
                let model4 = mat4_of(&model);
                let mut pose: Option<(u16, ssb_rom::pack::CachedVertexPose, [i8; 3])> = None;
                for index in 0..count {
                    let at = index * ssb_rom::pack::VERTEX_SIZE;
                    let from = verts.as_ptr().add(at) as *const PackedVertex;
                    let source_vertex =
                        if from as usize % core::mem::align_of::<PackedVertex>() == 0 {
                            from.read()
                        } else {
                            from.read_unaligned()
                        };
                    let (vertex, source_light) = match bound.next() {
                        Some(binding)
                            if u16::from_le_bytes([binding[0], binding[1]]) != u16::MAX =>
                        {
                            let source_node = u16::from_le_bytes([binding[0], binding[1]]);
                            let (_, poser, source_light) = match &pose {
                                Some(entry) if entry.0 == source_node => entry,
                                _ => {
                                    let source = if source_node == u16::MAX - 1 {
                                        ssb_rom::scene::Mat4::IDENTITY
                                    } else {
                                        posed.get(source_node as usize).copied().unwrap_or_else(
                                            || {
                                                pack.node(
                                                    object.first_node + u32::from(source_node),
                                                )
                                                .map_or(ssb_rom::scene::Mat4::IDENTITY, |n| {
                                                    ssb_rom::scene::Mat4(n.world)
                                                })
                                            },
                                        )
                                    };
                                    // A node matrix carries `MODEL_SCALE`.
                                    let source_light = light.map_or([0; 3], |world| {
                                        let world_source = model4.mul(&inverse.mul(&source));
                                        ssb_engine::rsp_light::object_light_dir(
                                            st.rsp_rows(light_rows(mat4_rows(&world_source))),
                                            world,
                                        )
                                    });
                                    &*pose.insert((
                                        source_node,
                                        ssb_rom::pack::CachedVertexPose::new(inverse, source),
                                        source_light,
                                    ))
                                }
                            };
                            // The same square root through a closure of its
                            // own: a second `fpu_sqrt` instance of the pose's
                            // array maps would take them out of line in
                            // `draw_node_mesh`.
                            (
                                poser.pose_with_sqrt(source_vertex, binding, |v| fpu_sqrt(v)),
                                Some(*source_light),
                            )
                        }
                        _ => (source_vertex, None),
                    };
                    (dynamic.add(at) as *mut PackedVertex).write(vertex);
                    if light.is_some() {
                        let mark = source_light.map_or(0, |dir| {
                            LIGHT_MARK_SET
                                | ssb_engine::rsp_light::dot16(
                                    [source_vertex.nx, source_vertex.ny, source_vertex.nz],
                                    dir,
                                )
                        });
                        (dynamic.add(at + LIGHT_MARK_OFFSET) as *mut u16)
                            .write_unaligned(mark.to_le());
                    }
                }
                let tail = count * ssb_rom::pack::VERTEX_SIZE;
                core::ptr::copy_nonoverlapping(
                    verts.as_ptr().add(tail),
                    dynamic.add(tail),
                    verts.len() - tail,
                );
                crate::gu::writeback_vertices(dynamic, verts.len(), true);
                verts = core::slice::from_raw_parts(dynamic, verts.len());
                crate::profile::stop_fine(crate::profile::Span::Skin, skin);
            }
        }
        batch.meshes.add(batch.len).write(BatchMesh {
            model,
            verts: verts.as_ptr(),
            verts_len: verts.len(),
            colors,
            first_prim: mesh.first_prim,
            prim_count: mesh.prim_count,
            record: batch.prims as u32,
            codes: 0,
            clips: false,
        });
        batch.len += 1;
        batch.prims += mesh.prim_count as usize;
    }
    batch.len - first
}

/// [`super::guard_clip`]'s clip transform: `vp * model`, in its order.
#[inline(always)]
fn clip_transform(vp: &[[f32; 4]; 4], model: &ScePspFMatrix4) -> [[f32; 4]; 4] {
    let cols = [model.x, model.y, model.z, model.w].map(|c| [c.x, c.y, c.z, c.w]);
    let mut mvp = [[0.0f32; 4]; 4];
    for (c, out) in mvp.iter_mut().enumerate() {
        for (r, o) in out.iter_mut().enumerate() {
            *o = (0..4).map(|k| vp[k][r] * cols[c][k]).sum();
        }
    }
    mvp
}

/// Pass 2 over every mesh of `batch`: [`super::guard_clip`]'s outcodes,
/// each mesh's at its own offset in [`CLIP_CODES`], and each drawn
/// primitive's submission, recoloured by [`expand_corners`] as
/// [`super::draw_mesh_vertices`] chooses it.
///
/// # Safety
///
/// As [`Batch::new`]; after [`batch_pose`] for every object and before any
/// [`batch_submit`]: nothing else may write [`CLIP_CODES`] until the last.
#[inline(never)]
pub(crate) unsafe fn batch_expand(pack: &Pack<'_>, st: &DrawState, batch: &mut Batch) {
    let band = CLIP_RATIO;
    let size = core::mem::size_of::<PackedVertex>();
    let table = &mut *core::ptr::addr_of_mut!(CLIP_CODES);
    let mut at = 0usize;
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
                *code = super::position_outcode(&mvp, band, position(bytes));
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
            if !st.heads.admits(p.flags) {
                continue;
            }
            let Some(indices) = pack.indices(&p) else {
                continue;
            };
            if p.index_count == 0 {
                continue;
            }
            let (vtype, index, vertex) = if effect_colors.is_some() {
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

/// Pass 3 for one object, the `count` meshes from `first`: each node's
/// matrix, then each primitive's material, clip and draw, as
/// [`super::draw_object_posed_nodes`] and [`super::draw_mesh_vertices`]
/// send them. `st.color_override` is the object's, as [`batch_pose`] took
/// it.
///
/// # Safety
///
/// As [`Batch::new`]; after [`batch_expand`].
#[inline(never)]
pub(crate) unsafe fn batch_submit(
    pack: &Pack<'_>,
    st: &mut DrawState,
    batch: &Batch,
    first: usize,
    count: usize,
    effect_mat_anim: Option<&EffectMaterialAnimator>,
) {
    // `light_prepass`, which returns at once without a fighter light.
    (*core::ptr::addr_of_mut!(LIGHT_PREPASS)).0 = 0;
    let band = CLIP_RATIO;
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
            // `batch_expand` wrote them inside the table (`BatchSize::fits`).
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
            crate::profile::stop_fine(crate::profile::Span::Material, material_t);
            let whole = [*batch.records.add(m.record as usize + i as usize)];
            let draws: &[Draw] = match &clip {
                Some(clip) => clip_submission(clip, indices, &whole[0]).unwrap_or(&whole),
                None => &whole,
            };
            let submit_t = crate::profile::start_fine();
            for d in draws {
                d.draw();
            }
            crate::profile::stop_fine(crate::profile::Span::Submit, submit_t);
            crate::profile::count(crate::profile::Span::Prims, 1);
            st.draws += 1;
            tris += p.index_count / 3;
        }
        st.triangles += tris;
        st.texture_part_mesh = None;
    }
}

/// [`super::apply_material`] for a batched primitive: no stage material
/// player, and `light_scope` is `Off` (no `LIT` primitive, [`plan_object`];
/// no known scene lights, [`batch_ready`]), so the light registers keep
/// their values. The rest is its text.
#[inline(always)]
unsafe fn batch_material(
    pack: &Pack<'_>,
    p: &PrimDesc,
    st: &mut DrawState,
    effect_mat_anim: Option<&EffectMaterialAnimator>,
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

    let effective_texture = match p.image_anim() {
        _ if st.texture_override.is_some() => st.texture_override.unwrap_or(p.texture),
        Some(anim) => effect_mat_anim
            .and_then(|m| m.resolved_texture(pack, anim))
            .unwrap_or(p.texture),
        None => p.texture,
    };

    let palette = p
        .palette_anim()
        .and_then(|anim| effect_mat_anim.and_then(|m| m.resolved_palette(pack, anim)))
        .unwrap_or(TextureDesc::NO_ANIM);
    if st.last_texture != Some((effective_texture, palette)) {
        st.last_texture = Some((effective_texture, palette));
        st.state_changes += 1;
        if !bind_texture_cached(pack, effective_texture, palette) {
            match pack.texture(effective_texture) {
                Some(t) => bind_texture_uncached(pack, effective_texture, &t, palette),
                None => sys::sceGuDisable(GuState::Texture2D),
            }
        }
    }

    let uv = (p.mat_anim != TextureDesc::NO_ANIM)
        .then(|| effect_mat_anim.and_then(|m| m.resolved_uv(pack, p.mat_anim)))
        .flatten()
        .and_then(|uv| uv.owned_by(p.flags));
    // `apply_texture_mapping`.
    let affine = match uv.and_then(|uv| Some((uv, pack.texture(effective_texture)?))) {
        Some((uv, t)) => animated_uv_affine(uv, &t),
        None => ssb_rom::skeleton::UvAffine::IDENTITY,
    };
    apply_texture_affine(pack, p, st, effective_texture, affine);

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
