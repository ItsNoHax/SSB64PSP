//! A stage-major [`super::super::draw_object_posed_nodes`] for one fighter of
//! the battle's fighter pass (RE-585).
//!
//! The per-object path runs every stage of one mesh before the next mesh:
//! node matrix, light, skin, material, light shade, expansion, draw. Here
//! each stage runs over every mesh of the fighter before the next starts:
//!
//! 1. [`plan_fighter`]: which meshes draw, decided before anything is
//!    drawn or allocated; any path the stages do not copy takes the
//!    per-object path for the whole fighter;
//! 2. [`fighter_pose`]: node matrices, object-space lights and skins (no GE
//!    command);
//! 3. [`fighter_expand`]: each primitive's lit or float-UV submission (no
//!    GE command);
//! 4. [`fighter_submit`]: each node's matrix, then each primitive's
//!    material and draw, in the per-object path's order and state.
//!
//! What the stages reorder, checked against the per-object path:
//! - the node light is [`super::super::light_prepass`]'s formula of the
//!   node's model matrix, the answer `DrawState::object_light` returns;
//! - the skin reads the node's model matrix and the fighter light, both
//!   fixed before [`fighter_pose`];
//! - a lit primitive's shade reads the light registers after its own
//!   material wrote them: [`fighter_expand`] runs the same writes on a copy
//!   (`light_scope != Off`; no known scene lights, [`plan_fighter`]);
//! - `material_colors` is `None` (no `color_override`, no material
//!   animator) and `expand_corners` reads `DrawState` only for linear
//!   texgen, which no staged primitive has;
//! - `apply_material` and every other `DrawState` write run in
//!   [`fighter_submit`], in the per-object path's order.
//!
//! Only skin blocks move in the list (taken before the commands that drew
//! between them); the expansions keep their order in the vertex region, so
//! the GE draws the per-object path's pixels (D-051).
//!
//! Helpers the per-object path inlines from a single call site are copied
//! (`LitPrim::new`, the light scope); out-of-line ones are shared.

use core::ffi::c_void;

use psp::sys::{self, GuState, ScePspFMatrix4, ScePspFVector4};
use ssb_rom::pack::{flags, NodeDesc, ObjectDesc, Pack, PackedVertex, PrimDesc};

use super::super::{
    apply_fade_blend, apply_flags_state, apply_texture_affine, bind_texture_cached,
    bind_texture_uncached, compose_model_matrix, draw_object_posed_nodes, expand_corners,
    expand_float_uv_corners, fpu_sqrt, light_rows, mat4_of, mat4_rows, matrix_rows,
    node_meshes, ssb_all_costume_rows, tape_mode, texture_part_override, Draw, DrawState,
    LitPrim, Look, TapeMode, TextureFuncState, LIGHT_MARK_OFFSET, LIGHT_MARK_SET,
    VERTEX_FORMAT,
};

/// Most meshes one fighter stages: the largest fighter draws 19.
const MESHES_MAX: usize = 64;
/// Most primitives one fighter stages: the largest fighter draws 47.
const PRIMS_MAX: usize = 128;

/// [`FighterMesh::flags`]: the first mesh of its node, which loads the
/// node's matrix.
const NEW_NODE: u8 = 1;
/// [`FighterMesh::flags`]: the joint's own list under a part that skips
/// the fighter's fog (`FTPARTS_FLAG_NOFOG`, RE-453).
const OWN_NOFOG: u8 = 2;
/// [`FighterMesh::flags`]: a mesh whose texture part shows another sprite
/// (RE-426).
const TEXTURE_PART: u8 = 4;

/// One mesh of the staged fighter, in draw order.
#[derive(Clone, Copy)]
#[repr(C)]
struct FighterMesh {
    /// Its node's model matrix `base * world`, column-major.
    model: [f32; 16],
    /// The vertices it draws: the pack's, or its skin block.
    verts: *const u8,
    verts_len: u32,
    /// The mesh in the pack.
    mesh: u32,
    /// Its node's index in the object.
    node: u32,
    first_prim: u32,
    prim_count: u16,
    /// Its first primitive's [`Scratch::draws`] record.
    record: u16,
    /// The fighter light in the node's object space.
    light: [i8; 3],
    flags: u8,
}

impl FighterMesh {
    const EMPTY: Self = Self {
        model: [0.0; 16],
        verts: core::ptr::null(),
        verts_len: 0,
        mesh: 0,
        node: 0,
        first_prim: 0,
        prim_count: 0,
        record: 0,
        light: [0; 3],
        flags: 0,
    };
}

/// The records of the fighter being drawn. One static of
/// exactly one 8 KiB D-cache way, word-aligned, so every static behind it
/// keeps its cache set.
#[repr(C)]
struct Scratch {
    meshes: [FighterMesh; MESHES_MAX],
    draws: [Draw; PRIMS_MAX],
    _pad: [u8; SCRATCH_PAD],
}

const SCRATCH_SIZE: usize = 8192;
const SCRATCH_PAD: usize = SCRATCH_SIZE
    - MESHES_MAX * core::mem::size_of::<FighterMesh>()
    - PRIMS_MAX * core::mem::size_of::<Draw>();
const _: () = assert!(core::mem::size_of::<Scratch>() == SCRATCH_SIZE);
const _: () = assert!(core::mem::align_of::<Scratch>() == 4);

static mut SCRATCH: Scratch = Scratch {
    meshes: [FighterMesh::EMPTY; MESHES_MAX],
    draws: [Draw::EMPTY; PRIMS_MAX],
    _pad: [0; SCRATCH_PAD],
};

/// [`super::super::draw_fighter_posed`] (`hidden` `None`) or
/// [`super::super::draw_fighter_posed_hiding`], staged: the battle's
/// fighter pass. A fighter [`plan_fighter`] rejects takes the per-object
/// path with the same arguments.
///
/// # Safety
///
/// As [`draw_object_posed_nodes`].
#[inline(never)]
pub unsafe fn draw_fighter_posed_staged(
    pack: &Pack<'_>,
    object: &ObjectDesc,
    base: &ScePspFMatrix4,
    posed: &[ssb_rom::scene::Mat4],
    st: &mut DrawState,
    look: Look<'_>,
    hidden: Option<&dyn Fn(u32) -> bool>,
) -> u32 {
    #[cfg(feature = "texture_probe")]
    let _cat = super::super::probe_cat(st, crate::texprobe::Cat::Fighter);
    if let Some(count) = plan_fighter(pack, object, st, look, hidden) {
        fighter_pose(pack, object, base, posed, st, count);
        fighter_expand(pack, st, count);
        return fighter_submit(pack, st, look, count);
    }
    draw_object_posed_nodes(pack, object, base, posed, None, st, None, None, look, None, hidden, None)
}

/// The meshes [`draw_object_posed_nodes`] would draw for `object`, in its
/// order, as [`Scratch::meshes`] records; `None` when the fighter or `st`
/// takes a path the stages do not copy: a vertex tape, an effect colour, a
/// guard clip, known scene lights, no fighter light, more than 64 nodes, a
/// billboard, a mesh or vertex buffer the pack lacks, a texgen or two-tile
/// primitive, or more records than [`Scratch`] holds.
#[inline(never)]
unsafe fn plan_fighter(
    pack: &Pack<'_>,
    object: &ObjectDesc,
    st: &DrawState,
    look: Look<'_>,
    hidden: Option<&dyn Fn(u32) -> bool>,
) -> Option<usize> {
    if !matches!(*tape_mode(), TapeMode::Off)
        || st.color_override.is_some()
        || st.clip_view_proj.is_some()
        || st.scene_lights == ssb_rom::anim_color::SceneLights::Known
        || !st.runtime_fighter_light
        || object.node_count > 64
    {
        return None;
    }
    let scratch = &mut *core::ptr::addr_of_mut!(SCRATCH);
    let textures = look.textures.iter().any(|&id| id > 0);
    let mut count = 0usize;
    let mut prims = 0usize;
    for i in 0..object.node_count {
        let global_node = object.first_node + i;
        if hidden.is_some_and(|hidden| hidden(global_node)) {
            continue;
        }
        let Some(node) = pack.node(global_node) else {
            continue;
        };
        let part = look.parts.map(|p| {
            p.get(i as usize).copied().unwrap_or(ssb_game::modelpart::ABSENT)
        });
        let (mesh_index, accessory) = node_meshes(pack, global_node, node.mesh, look.costume, part);
        if mesh_index == NodeDesc::NO_MESH {
            continue;
        }
        if node.flags & NodeDesc::FLAG_BILLBOARD != 0 {
            return None;
        }
        let meshes = if look.accessory_before {
            [accessory, Some(mesh_index)]
        } else {
            [Some(mesh_index), accessory]
        };
        // `part_skips_fog`.
        let nofog = st.fighter_fog
            && part.is_some_and(|p| {
                p >= 0
                    && pack.modelpart_flags_in(
                        ssb_all_costume_rows(pack, global_node),
                        global_node,
                        p as u32,
                    ) & ssb_rom::pack::PART_FLAG_NOFOG
                        != 0
            });
        let mut first = NEW_NODE;
        for index in meshes.into_iter().flatten() {
            let mesh = pack.mesh(index)?;
            let verts = pack.vertices(&mesh)?;
            if count == MESHES_MAX || prims + mesh.prim_count as usize > PRIMS_MAX {
                return None;
            }
            for j in 0..mesh.prim_count {
                if pack.prim(mesh.first_prim + j).is_some_and(|p| {
                    p.flags & (flags::TEXTURE_GEN | flags::TEXTURE_GEN_LINEAR | flags::LOD_BLEND) != 0
                }) {
                    return None;
                }
            }
            let mut mesh_flags = first;
            if nofog && index == mesh_index {
                mesh_flags |= OWN_NOFOG;
            }
            if textures && pack.mesh_has_texture_part(index) {
                mesh_flags |= TEXTURE_PART;
            }
            scratch.meshes[count] = FighterMesh {
                model: [0.0; 16],
                verts: verts.as_ptr(),
                verts_len: verts.len() as u32,
                mesh: index,
                node: i,
                first_prim: mesh.first_prim,
                prim_count: mesh.prim_count as u16,
                record: prims as u16,
                light: [0; 3],
                flags: mesh_flags,
            };
            count += 1;
            prims += mesh.prim_count as usize;
            first = 0;
        }
    }
    Some(count)
}

/// Stage 1 over the planned meshes: each node's model matrix and
/// object-space light, and each bound mesh's skin, as
/// [`super::super::draw_object_posed_nodes`],
/// [`super::super::light_prepass`] and [`super::super::draw_node_mesh`]
/// compute them, without the GE writes.
#[allow(clippy::redundant_closure)]
#[inline(never)]
unsafe fn fighter_pose(
    pack: &Pack<'_>,
    object: &ObjectDesc,
    base: &ScePspFMatrix4,
    posed: &[ssb_rom::scene::Mat4],
    st: &DrawState,
    count: usize,
) {
    let scratch = &mut *core::ptr::addr_of_mut!(SCRATCH);
    let light = st.active_light();
    let mut model = ScePspFMatrix4 {
        x: ScePspFVector4 { x: 0.0, y: 0.0, z: 0.0, w: 0.0 },
        y: ScePspFVector4 { x: 0.0, y: 0.0, z: 0.0, w: 0.0 },
        z: ScePspFVector4 { x: 0.0, y: 0.0, z: 0.0, w: 0.0 },
        w: ScePspFVector4 { x: 0.0, y: 0.0, z: 0.0, w: 0.0 },
    };
    let mut world = [0.0f32; 16];
    let mut node_light = [0i8; 3];
    for k in 0..count {
        let m = &mut scratch.meshes[k];
        let i = m.node;
        if m.flags & NEW_NODE != 0 {
            let matrix_t = crate::profile::start_fine();
            world = match posed.get(i as usize) {
                Some(p) => p.0,
                None => pack.node(object.first_node + i).map_or([0.0; 16], |n| n.world),
            };
            let local = ScePspFMatrix4 {
                x: ScePspFVector4 { x: world[0], y: world[1], z: world[2], w: world[3] },
                y: ScePspFVector4 { x: world[4], y: world[5], z: world[6], w: world[7] },
                z: ScePspFVector4 { x: world[8], y: world[9], z: world[10], w: world[11] },
                w: ScePspFVector4 { x: world[12], y: world[13], z: world[14], w: world[15] },
            };
            model = compose_model_matrix(base, &local);
            node_light = light.map_or([0; 3], |dir| {
                ssb_engine::rsp_light::object_light_dir(
                    st.rsp_rows(light_rows(matrix_rows(&model))),
                    dir,
                )
            });
            crate::profile::stop_fine(crate::profile::Span::NodeMatrix, matrix_t);
        }
        m.model = mat4_of(&model).0;
        m.light = node_light;
        if posed.is_empty() {
            continue;
        }
        let Some(mesh) = pack.mesh(m.mesh) else {
            continue;
        };
        let verts = core::slice::from_raw_parts(m.verts, m.verts_len as usize);
        let (Some(bindings), Some(inverse)) = (
            pack.vertex_bindings(&mesh),
            ssb_rom::scene::Mat4(world).inverse_affine(),
        ) else {
            continue;
        };
        let skin = crate::profile::start_fine();
        let dynamic = crate::gu::cached_vertices(verts.len());
        let count = verts.len() / ssb_rom::pack::VERTEX_SIZE;
        let mut bound = bindings.chunks_exact(8);
        // The node's matrix, as `set_model_matrix` holds it.
        let model4 = mat4_of(&model);
        let mut pose: Option<(u16, ssb_rom::pack::CachedVertexPose, [i8; 3])> = None;
        for index in 0..count {
            let at = index * ssb_rom::pack::VERTEX_SIZE;
            let from = verts.as_ptr().add(at) as *const PackedVertex;
            let source_vertex = if from as usize % core::mem::align_of::<PackedVertex>() == 0 {
                from.read()
            } else {
                from.read_unaligned()
            };
            let (vertex, source_light) = match bound.next() {
                Some(binding) if u16::from_le_bytes([binding[0], binding[1]]) != u16::MAX => {
                    let source_node = u16::from_le_bytes([binding[0], binding[1]]);
                    let (_, poser, source_light) = match &pose {
                        Some(entry) if entry.0 == source_node => entry,
                        _ => {
                            let source = if source_node == u16::MAX - 1 {
                                ssb_rom::scene::Mat4::IDENTITY
                            } else {
                                posed.get(source_node as usize).copied().unwrap_or_else(|| {
                                    pack.node(object.first_node + u32::from(source_node))
                                        .map_or(ssb_rom::scene::Mat4::IDENTITY, |n| {
                                            ssb_rom::scene::Mat4(n.world)
                                        })
                                })
                            };
                            // A node matrix carries `MODEL_SCALE`.
                            let source_light = light.map_or([0; 3], |dir| {
                                let world_source = model4.mul(&inverse.mul(&source));
                                ssb_engine::rsp_light::object_light_dir(
                                    st.rsp_rows(light_rows(mat4_rows(&world_source))),
                                    dir,
                                )
                            });
                            &*pose.insert((
                                source_node,
                                ssb_rom::pack::CachedVertexPose::new(inverse, source),
                                source_light,
                            ))
                        }
                    };
                    // A closure of its own, as `batch_pose`'s: another
                    // `fpu_sqrt` instance would change `draw_node_mesh`.
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
                (dynamic.add(at + LIGHT_MARK_OFFSET) as *mut u16).write_unaligned(mark.to_le());
            }
        }
        let tail = count * ssb_rom::pack::VERTEX_SIZE;
        core::ptr::copy_nonoverlapping(verts.as_ptr().add(tail), dynamic.add(tail), verts.len() - tail);
        crate::gu::writeback_vertices(dynamic, verts.len(), true);
        m.verts = dynamic;
        crate::profile::stop_fine(crate::profile::Span::Skin, skin);
    }
}

/// [`LitPrim::new`], copied: its one caller inlines it.
#[inline(always)]
fn lit_prim(p: &PrimDesc, dir: [i8; 3], registers: [[u8; 3]; 2], env: Option<[u8; 4]>) -> LitPrim {
    let prim = (p.prim_color != 0).then(|| {
        let c = p.prim_color;
        [c as u8, (c >> 8) as u8, (c >> 16) as u8]
    });
    LitPrim {
        dir,
        shade: ssb_engine::rsp_light::LitShade::new(registers[0], registers[1], prim),
        keep_rgb: p.flags & (flags::FLAT_COLOR | flags::TEXTURE_BLEND) != 0,
        env: env.map(|[r, g, b, _]| [r, g, b]),
    }
}

/// `apply_material`'s light scope for `p` under `st`.
#[inline(always)]
fn fighter_light_scope(p: &PrimDesc, st: &DrawState) -> ssb_rom::anim_color::LightScope {
    let authored = flags::LIGHT1_COLOR | flags::LIGHT2_COLOR;
    ssb_rom::anim_color::light_scope(
        p.flags,
        st.runtime_fighter_light && (!st.authored_lights_only || p.flags & authored == authored),
        st.stage_light.is_some(),
        st.scene_lights,
    )
}

/// The `G_MW_LIGHTCOL` writes of `p`'s list (`apply_material`, without
/// effect colours) on `registers`.
#[inline(always)]
fn write_light_registers(p: &PrimDesc, registers: &mut [[u8; 3]; 2]) {
    let colors = ssb_rom::anim_color::light_registers(None, p);
    for (register, color) in registers.iter_mut().zip([colors.0, colors.1]) {
        if let Some(color) = color {
            *register = [color as u8, (color >> 8) as u8, (color >> 16) as u8];
        }
    }
}

/// Stage 2 over the planned meshes: each drawn primitive's submission, lit
/// with the light registers its own material leaves, as
/// [`super::super::draw_mesh_vertices`] chooses it.
#[inline(never)]
unsafe fn fighter_expand(pack: &Pack<'_>, st: &DrawState, count: usize) {
    let scratch = &mut *core::ptr::addr_of_mut!(SCRATCH);
    let mut registers = st.light_registers;
    for k in 0..count {
        let m = scratch.meshes[k];
        let verts = core::slice::from_raw_parts(m.verts, m.verts_len as usize);
        for j in 0..u32::from(m.prim_count) {
            let record = &mut scratch.draws[usize::from(m.record) + j as usize];
            *record = Draw::EMPTY;
            let Some(p) = pack.prim(m.first_prim + j) else {
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
            let lit = if fighter_light_scope(&p, st) == ssb_rom::anim_color::LightScope::Off {
                None
            } else {
                write_light_registers(&p, &mut registers);
                Some(lit_prim(&p, m.light, registers, st.fog_env))
            };
            let (vtype, index, vertex) = if p.flags & flags::SIGNED_CLAMP_UV != 0 {
                expand_float_uv_corners(&p, indices, verts, None, lit)
            } else if lit.is_some() {
                expand_corners(pack, &p, st, indices, verts, None, false, lit)
            } else {
                (
                    VERTEX_FORMAT,
                    indices.as_ptr() as *const c_void,
                    verts.as_ptr() as *const c_void,
                )
            };
            *record = Draw {
                vtype: vtype.bits(),
                index,
                vertex,
                count: p.index_count,
            };
        }
    }
}

/// Stage 3 over the planned meshes: each node's matrix, then each
/// primitive's material and draw, as
/// [`super::super::draw_object_posed_nodes`] and
/// [`super::super::draw_mesh_vertices`] send them. Returns the triangles.
#[inline(never)]
unsafe fn fighter_submit(pack: &Pack<'_>, st: &mut DrawState, look: Look<'_>, count: usize) -> u32 {
    let scratch = &*core::ptr::addr_of!(SCRATCH);
    let world_light = st.active_light().unwrap_or([0; 3]);
    let mut tris = 0u32;
    for k in 0..count {
        let m = &scratch.meshes[k];
        if m.flags & NEW_NODE != 0 {
            let matrix_t = crate::profile::start_fine();
            let c = &m.model;
            let model = ScePspFMatrix4 {
                x: ScePspFVector4 { x: c[0], y: c[1], z: c[2], w: c[3] },
                y: ScePspFVector4 { x: c[4], y: c[5], z: c[6], w: c[7] },
                z: ScePspFVector4 { x: c[8], y: c[9], z: c[10], w: c[11] },
                w: ScePspFVector4 { x: c[12], y: c[13], z: c[14], w: c[15] },
            };
            sys::sceGumUpdateMatrix();
            sys::sceGuSetMatrix(sys::MatrixMode::Model, &model);
            st.set_model_matrix(model);
            // `light_prepass`'s answer for the node.
            st.node_light = Some((world_light, m.light));
            crate::profile::stop_fine(crate::profile::Span::NodeMatrix, matrix_t);
        }
        let own_list = m.flags & OWN_NOFOG != 0;
        if own_list {
            sys::sceGuDisable(GuState::Fog);
        }
        st.texture_part_ids = look.textures;
        st.texture_part_mesh = (m.flags & TEXTURE_PART != 0).then_some(m.mesh);
        let mut mesh_tris = 0u32;
        for j in 0..u32::from(m.prim_count) {
            let Some(p) = pack.prim(m.first_prim + j) else {
                continue;
            };
            if !st.admits(p.flags) {
                continue;
            }
            // `fighter_expand` left the primitives without indices empty.
            let record = &scratch.draws[usize::from(m.record) + j as usize];
            if record.count == 0 {
                continue;
            }
            let material_t = crate::profile::start_fine();
            st.texture_override = st
                .texture_part_mesh
                .and_then(|mesh| texture_part_override(pack, mesh, j, st.texture_part_ids));
            fighter_material(pack, &p, st);
            st.texture_override = None;
            crate::profile::stop_fine(crate::profile::Span::Material, material_t);
            let submit_t = crate::profile::start_fine();
            record.draw();
            crate::profile::stop_fine(crate::profile::Span::Submit, submit_t);
            crate::profile::count(crate::profile::Span::Prims, 1);
            st.draws += 1;
            mesh_tris += p.index_count / 3;
        }
        st.triangles += mesh_tris;
        tris += mesh_tris;
        st.texture_part_mesh = None;
        if own_list {
            sys::sceGuEnable(GuState::Fog);
        }
    }
    tris
}

/// [`super::super::apply_material`] for a staged primitive: no material
/// animator and no effect colours, so the texture is the primitive's (or
/// its texture part's), the palette the baked one and the mapping the
/// identity. The rest is its text.
#[inline(always)]
unsafe fn fighter_material(pack: &Pack<'_>, p: &PrimDesc, st: &mut DrawState) {
    let light_scope = fighter_light_scope(p, st);
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

    if light_scope != ssb_rom::anim_color::LightScope::Off
        || st.scene_lights == ssb_rom::anim_color::SceneLights::Known
    {
        write_light_registers(p, &mut st.light_registers);
    }

    let effective_texture = st.texture_override.unwrap_or(p.texture);
    let palette = ssb_rom::pack::TextureDesc::NO_ANIM;
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

    // `apply_texture_mapping` without an animated window.
    apply_texture_affine(pack, p, st, effective_texture, ssb_rom::skeleton::UvAffine::IDENTITY);

    let desired_texture_func = if p.flags & flags::TEXTURE_BLEND != 0 {
        TextureFuncState::Blend(p.texture_blend_target)
    } else {
        TextureFuncState::Modulate
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
