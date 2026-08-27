//! Drawing packed meshes through the GE.
//!
//! Everything here reads directly out of the loaded asset pack — no copying,
//! no per-frame conversion. The pack was laid out for exactly this
//! (`ssb_rom::pack`).
//!
//! ## Performance shape
//!
//! * **Indexed draws.** One `sceGuDrawArray` per primitive, indexing a vertex
//!   buffer shared by the whole mesh, so the GE's post-transform cache works
//!   across primitives.
//! * **16-bit position and UV.** `GU_VERTEX_16BIT | GU_TEXTURE_16BIT` — 16
//!   bytes per vertex instead of 24. The N64 data is already integral, so the
//!   narrowing is lossless (see `docs/rendering.md`).
//! * **State set only when it changes.** Primitives arrive sorted by material
//!   from the converter, so tracking the last-applied state turns a per-draw
//!   cost into a per-material one.

use core::ffi::c_void;

use psp::sys::{
    self, ClutPixelFormat, GuPrimitive, GuState, ScePspFMatrix4, ScePspFVector4,
    TexturePixelFormat, VertexType,
};

use ssb_rom::pack::{flags, MeshDesc, NodeDesc, ObjectDesc, Pack, PrimDesc, TextureDesc};

/// What the GE divides 16-bit vertex components by.
///
/// **Integer vertex formats on the PSP are normalised, not raw.** A
/// `GU_VERTEX_16BIT` coordinate is interpreted as `value / 32768`, so N64
/// coordinates in the hundreds collapse to a few hundredths of a unit and the
/// model becomes an invisible speck at the origin. The model matrix must scale
/// back up by this factor -- see [`MODEL_SCALE`].
pub const VERTEX_16BIT_DIVISOR: f32 = 32768.0;

/// Uniform model scale that undoes [`VERTEX_16BIT_DIVISOR`].
pub const MODEL_SCALE: f32 = VERTEX_16BIT_DIVISOR;

/// Vertex format of [`ssb_rom::pack::PackedVertex`].
///
/// Must describe the struct's field order exactly: texture coords, colour,
/// position. A mismatch renders garbage without any error.
const VERTEX_FORMAT: VertexType = VertexType::from_bits_truncate(
    VertexType::TEXTURE_16BIT.bits()
        | VertexType::COLOR_8888.bits()
        | VertexType::VERTEX_16BIT.bits()
        | VertexType::TRANSFORM_3D.bits()
        | VertexType::INDEX_16BIT.bits(),
);

/// Maps a packed `Psm` discriminant back to the GE enum.
///
/// The discriminants come from `ssb_rom::psp_texture::Psm`; keep in step.
fn psm_of(v: u8) -> TexturePixelFormat {
    match v {
        0 => TexturePixelFormat::Psm5650,
        1 => TexturePixelFormat::Psm5551,
        2 => TexturePixelFormat::Psm4444,
        3 => TexturePixelFormat::Psm8888,
        4 => TexturePixelFormat::PsmT4,
        _ => TexturePixelFormat::PsmT8,
    }
}

/// Tracks what state is already applied, so redundant sets are skipped.
#[derive(Default)]
pub struct DrawState {
    last_texture: Option<u32>,
    last_flags: Option<u32>,
    pub draws: u32,
    pub triangles: u32,
    pub state_changes: u32,
}

impl DrawState {
    pub fn begin_frame(&mut self) {
        self.last_texture = None;
        self.last_flags = None;
        self.draws = 0;
        self.triangles = 0;
        self.state_changes = 0;
    }
}

/// Binds a texture from the pack.
///
/// # Safety
///
/// The pack buffer must outlive the frame; the GE reads it asynchronously.
unsafe fn bind_texture(pack: &Pack<'_>, t: &TextureDesc) {
    let Some(data) = pack.texture_data(t) else {
        return;
    };

    sys::sceGuEnable(GuState::Texture2D);
    let psm = psm_of(t.psm);

    // Paletted formats need the CLUT uploaded before the image.
    if let Some(pal) = pack.palette_data(t) {
        // The mask selects which bits of the texel index address the CLUT, and
        // it must match the index width: 0x0F for 4-bit, 0xFF for 8-bit.
        // Leaving it at 0xFF for a PsmT4 texture lets the upper nibble leak in,
        // indexing past a 16-entry palette and producing coloured speckle.
        let mask: u32 = if matches!(psm, TexturePixelFormat::PsmT4) {
            0x0F
        } else {
            0xFF
        };
        sys::sceGuClutMode(ClutPixelFormat::Psm8888, 0, mask, 0);
        // sceGuClutLoad counts blocks of 8 entries, rounded up.
        let blocks = (t.palette_len as i32 + 7) / 8;
        sys::sceGuClutLoad(blocks, pal.as_ptr() as *const c_void);
    }

    sys::sceGuTexMode(psm, 0, 0, t.swizzled as i32);
    sys::sceGuTexImage(
        sys::MipmapLevel::None,
        t.stride as i32,
        // The GE wants power-of-two dimensions; the packer padded height too.
        (t.height as u32).next_power_of_two() as i32,
        t.stride as i32,
        data.as_ptr() as *const c_void,
    );
    sys::sceGuTexFunc(
        sys::TextureEffect::Modulate,
        sys::TextureColorComponent::Rgba,
    );
    sys::sceGuTexFilter(sys::TextureFilter::Linear, sys::TextureFilter::Linear);
    // Texture coordinates need the same normalisation undone, then the N64's
    // S10.5 fixed point (32 units per texel) converted to 0..1 across the
    // texture:  final = (uv / 32768) * scale  and we want  (uv / 32) / dim,
    // so scale = 32768 / (32 * dim) = 1024 / dim.
    const UV_SCALE: f32 = VERTEX_16BIT_DIVISOR / 32.0; // 1024
                                                       // Both axes normalise against the dimensions actually handed to
                                                       // sceGuTexImage -- the padded ones. Using the logical height here stretches
                                                       // V on any texture whose height is not already a power of two.
    let padded_h = (t.height as u32).next_power_of_two().max(1) as f32;
    sys::sceGuTexScale(UV_SCALE / t.stride as f32, UV_SCALE / padded_h);

    // Mesh UVs routinely run outside 0..1 (measured -55..119 texels on a
    // 64-wide texture), so the texture must tile rather than clamp.
    sys::sceGuTexWrap(sys::GuTexWrapMode::Repeat, sys::GuTexWrapMode::Repeat);
    sys::sceGuTexOffset(0.0, 0.0);
}

/// Applies a primitive's material state.
unsafe fn apply_material(pack: &Pack<'_>, p: &PrimDesc, st: &mut DrawState) {
    if st.last_flags != Some(p.flags) {
        st.last_flags = Some(p.flags);
        st.state_changes += 1;

        let cull = p.flags & (flags::CULL_BACK | flags::CULL_FRONT) != 0;
        if cull {
            sys::sceGuEnable(GuState::CullFace);
            // The N64's front-face winding is the opposite of the GE's default
            // for the same triangle order.
            sys::sceGuFrontFace(if p.flags & flags::CULL_FRONT != 0 {
                sys::FrontFaceDirection::Clockwise
            } else {
                sys::FrontFaceDirection::CounterClockwise
            });
        } else {
            sys::sceGuDisable(GuState::CullFace);
        }

        sys::sceGuShadeModel(if p.flags & flags::SMOOTH != 0 {
            sys::ShadingModel::Smooth
        } else {
            sys::ShadingModel::Flat
        });
    }

    if st.last_texture != Some(p.texture) {
        st.last_texture = Some(p.texture);
        st.state_changes += 1;
        match pack.texture(p.texture) {
            Some(t) => bind_texture(pack, &t),
            None => sys::sceGuDisable(GuState::Texture2D),
        }
    }
}

/// Draws one mesh from the pack.
///
/// Returns the number of triangles submitted.
///
/// # Safety
///
/// `pack`'s backing buffer must remain valid and cache-flushed until the frame
/// is submitted, because the GE reads it by DMA.
pub unsafe fn draw_mesh(pack: &Pack<'_>, mesh: &MeshDesc, st: &mut DrawState) -> u32 {
    let Some(verts) = pack.vertices(mesh) else {
        return 0;
    };

    let mut tris = 0u32;
    for i in 0..mesh.prim_count {
        let Some(p) = pack.prim(mesh.first_prim + i) else {
            continue;
        };
        let Some(indices) = pack.indices(&p) else {
            continue;
        };
        if p.index_count == 0 {
            continue;
        }

        apply_material(pack, &p, st);

        sys::sceGumDrawArray(
            GuPrimitive::Triangles,
            VERTEX_FORMAT,
            p.index_count as i32,
            indices.as_ptr() as *const c_void,
            verts.as_ptr() as *const c_void,
        );

        st.draws += 1;
        tris += p.index_count / 3;
    }

    st.triangles += tris;
    tris
}

/// Draws every node of one assembled object.
///
/// The per-node world matrix is baked into the pack (`ssb_rom::pack::NodeDesc`),
/// so this costs one `sceGumLoadMatrix` per node and no matrix maths at all —
/// the whole point of baking. `base` is the camera/model transform the object
/// sits under; each node's matrix is composed onto it by the GE.
///
/// Returns the number of triangles submitted.
///
/// # Safety
///
/// Same as [`draw_mesh`]: the pack buffer must stay valid and cache-flushed
/// until the frame is submitted.
pub unsafe fn draw_object(
    pack: &Pack<'_>,
    object: &ObjectDesc,
    base: &ScePspFMatrix4,
    st: &mut DrawState,
) -> u32 {
    let mut tris = 0;
    for i in 0..object.node_count {
        let Some(node) = pack.node(object.first_node + i) else {
            continue;
        };
        if node.mesh == NodeDesc::NO_MESH {
            continue; // pure transform: a joint with no geometry
        }
        let Some(mesh) = pack.mesh(node.mesh) else {
            continue;
        };

        // `world` is already the node's full ancestor-composed transform, so
        // there is nothing to push or pop -- load base * world and draw.
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
        sys::sceGumMatrixMode(sys::MatrixMode::Model);
        sys::sceGumLoadMatrix(base);
        sys::sceGumMultMatrix(&local);

        tris += draw_mesh(pack, &mesh, st);
    }
    tris
}

/// Bounding box of a whole object, in the pack's normalised units.
///
/// Node transforms place geometry that is itself in `i16` units divided by
/// [`VERTEX_16BIT_DIVISOR`], so this works in that same normalised space and
/// the caller scales once.
///
/// All eight corners of each node's box are transformed, not just its
/// translation. Translation-only was the earlier version, on the reasoning that
/// the extra tightness would not change how the camera frames anything — which
/// was wrong the moment fighters started converting. Samus's joints are rotated
/// far enough that her head sat outside the box and got clipped off the top of
/// the frame.
pub fn object_bounds(pack: &Pack<'_>, object: &ObjectDesc) -> Option<([f32; 3], [f32; 3])> {
    let mut min = [f32::MAX; 3];
    let mut max = [f32::MIN; 3];
    let mut any = false;

    for i in 0..object.node_count {
        let Some(node) = pack.node(object.first_node + i) else {
            continue;
        };
        if node.mesh == NodeDesc::NO_MESH {
            continue;
        }
        let Some(mesh) = pack.mesh(node.mesh) else {
            continue;
        };
        let Some((lo, hi)) = bounds(pack, &mesh) else {
            continue;
        };
        any = true;

        let m = &node.world;
        for corner in 0..8 {
            let p = [
                if corner & 1 == 0 { lo[0] } else { hi[0] } / VERTEX_16BIT_DIVISOR,
                if corner & 2 == 0 { lo[1] } else { hi[1] } / VERTEX_16BIT_DIVISOR,
                if corner & 4 == 0 { lo[2] } else { hi[2] } / VERTEX_16BIT_DIVISOR,
            ];
            for axis in 0..3 {
                let v = m[axis] * p[0] + m[4 + axis] * p[1] + m[8 + axis] * p[2] + m[12 + axis];
                if v < min[axis] {
                    min[axis] = v;
                }
                if v > max[axis] {
                    max[axis] = v;
                }
            }
        }
    }
    any.then_some((min, max))
}

/// Draws a texture on a screen-filling quad with known UVs.
///
/// A diagnostic that isolates the texture *upload* from everything else. If the
/// image appears here but a mesh renders as noise, the upload (format, CLUT,
/// swizzle, buffer width) is fine and the fault is in the mesh's UVs or
/// per-primitive state. Uses float positions and UVs so neither 16-bit
/// normalisation nor the S10.5 conversion is in the picture.
///
/// # Safety
///
/// The pack buffer must outlive the frame.
pub unsafe fn draw_texture_quad(pack: &Pack<'_>, index: u32, verts: &mut [TexQuadVertex; 6]) {
    let Some(t) = pack.texture(index) else { return };
    bind_texture(pack, &t);

    // Two triangles covering a square in front of the camera.
    let quad = [
        (0.0f32, 0.0f32, -1.0f32, 1.0f32),
        (1.0, 0.0, 1.0, 1.0),
        (1.0, 1.0, 1.0, -1.0),
        (0.0, 0.0, -1.0, 1.0),
        (1.0, 1.0, 1.0, -1.0),
        (0.0, 1.0, -1.0, -1.0),
    ];
    for (i, (u, v, x, y)) in quad.into_iter().enumerate() {
        verts[i] = TexQuadVertex {
            u,
            v,
            color: 0xFFFF_FFFF,
            x,
            y,
            z: 0.0,
        };
    }

    // Undo the tex scale set for 16-bit mesh UVs: these UVs are already 0..1.
    sys::sceGuTexScale(1.0, 1.0);
    sys::sceGumDrawArray(
        GuPrimitive::Triangles,
        VertexType::TEXTURE_32BITF
            | VertexType::COLOR_8888
            | VertexType::VERTEX_32BITF
            | VertexType::TRANSFORM_3D,
        6,
        core::ptr::null(),
        verts.as_ptr() as *const c_void,
    );
}

/// Vertex layout for [`draw_texture_quad`].
#[repr(C, align(4))]
#[derive(Clone, Copy, Default)]
pub struct TexQuadVertex {
    pub u: f32,
    pub v: f32,
    pub color: u32,
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

/// Computes a mesh's bounding box in game units, for framing the camera.
///
/// Smash's meshes are authored at wildly different scales — a stage is
/// hundreds of units across, a character a few dozen — so a fixed camera
/// distance would show either nothing or a single polygon filling the screen.
pub fn bounds(pack: &Pack<'_>, mesh: &MeshDesc) -> Option<([f32; 3], [f32; 3])> {
    let verts = pack.vertices(mesh)?;
    if mesh.vertex_count == 0 {
        return None;
    }

    // Position sits at byte 8 of each vertex, after u, v and the colour.
    const POS: usize = 8;
    let stride = ssb_rom::pack::VERTEX_SIZE;

    let mut min = [f32::MAX; 3];
    let mut max = [f32::MIN; 3];
    for i in 0..mesh.vertex_count as usize {
        let at = i * stride + POS;
        for axis in 0..3 {
            let o = at + axis * 2;
            let v = i16::from_le_bytes([verts[o], verts[o + 1]]) as f32;
            if v < min[axis] {
                min[axis] = v;
            }
            if v > max[axis] {
                max[axis] = v;
            }
        }
    }
    Some((min, max))
}
