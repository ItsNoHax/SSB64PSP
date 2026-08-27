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

// ---------------------------------------------------------------------------
// Stages
// ---------------------------------------------------------------------------

/// Scratch for one collision polyline. Sized to the longest line in the
/// archive with headroom; `Align16` because the GE DMAs it.
static mut LINE_BUF: psp::Align16<[crate::gu::GuVertex; 256]> =
    psp::Align16([crate::gu::GuVertex::new(0.0, 0.0, 0.0, 0.0, 0.0, 0); 256]);

/// Colours for the four `MPLineKind`s, so the overlay is readable at a glance.
///
/// Packed ABGR, matching `Color::to_abgr`. Floors are green, ceilings red,
/// walls blue and yellow — the point is that a wall and a floor never look
/// alike, because "is this line the kind the extractor said it was" is what
/// the overlay is there to answer.
fn line_color(kind: u16, passable: bool) -> u32 {
    use ssb_rom::pack::line_kind;
    match kind {
        // A drop-through floor is drawn dimmer than a solid one: on Dream Land
        // that is immediately visible as three faint platforms over one bright
        // one, which is exactly how the stage behaves.
        line_kind::FLOOR if passable => 0xFF60_C060,
        line_kind::FLOOR => 0xFF40_FF40,
        line_kind::CEILING => 0xFF40_40FF,
        line_kind::RIGHT_WALL => 0xFFFF_A040,
        _ => 0xFF40_D0FF,
    }
}

/// Draws a stage's render layers: up to four assembled objects.
///
/// Returns the triangles submitted and how many layer slots were filled. An
/// empty slot is normal — most stages use two or three.
///
/// # Safety
///
/// Same as [`draw_object`].
pub unsafe fn draw_stage(
    pack: &Pack<'_>,
    stage: &ssb_rom::pack::StageDesc,
    base: &ScePspFMatrix4,
    st: &mut DrawState,
) -> (u32, u32) {
    let mut tris = 0;
    let mut drawn = 0;
    for slot in stage.layers {
        if slot == ssb_rom::pack::StageDesc::NO_LAYER {
            continue;
        }
        let Some(object) = pack.object(slot) else {
            continue;
        };
        tris += draw_object(pack, &object, base, st);
        drawn += 1;
    }
    (tris, drawn)
}

/// Draws a stage's collision polylines over its geometry.
///
/// This is the visual form of the check `romtool collide` does numerically: if
/// the green lines trace the platforms you can see, then the collision data,
/// the pack and the renderer agree about where the stage is. Numbers cannot
/// catch a systematic offset that happens to be consistent; this can.
///
/// Positions are divided by [`MODEL_SCALE`] so they land in the same space as
/// the mesh path's `i16` vertices under the same `base` matrix — one transform
/// for both, rather than a second one that could drift.
///
/// Returns the number of line segments drawn.
///
/// # Safety
///
/// The pack buffer must stay valid until the frame is submitted.
pub unsafe fn draw_collision(
    pack: &Pack<'_>,
    stage: &ssb_rom::pack::StageDesc,
    base: &ScePspFMatrix4,
    gpu: &mut crate::gu::Gpu,
) -> u32 {
    const PASS_BIT: u16 = 1 << 14;

    sys::sceGumMatrixMode(sys::MatrixMode::Model);
    sys::sceGumLoadMatrix(base);

    let mut segments = 0;
    for line in pack.stage_lines(stage) {
        let buf = &mut *core::ptr::addr_of_mut!(LINE_BUF);
        let mut n = 0usize;
        let mut passable = false;
        for v in pack.line_vertices(&line) {
            if n >= buf.0.len() {
                break;
            }
            passable |= v.flags & PASS_BIT != 0;
            buf.0[n] = crate::gu::GuVertex::new(
                v.x as f32 / MODEL_SCALE,
                v.y as f32 / MODEL_SCALE,
                0.0,
                0.0,
                0.0,
                0,
            );
            n += 1;
        }
        if n < 2 {
            continue;
        }
        let color = line_color(line.kind, passable);
        for v in &mut buf.0[..n] {
            v.color = color;
        }
        gpu.draw_line_strip(&buf.0[..n]);
        segments += n as u32 - 1;
    }
    segments
}

/// Draws the simulated fighter as its collision body.
///
/// Not a character model: the pack has no record naming which object is Mario,
/// and guessing one would be a fingerprint rather than a fact. What is drawn
/// instead is `MPObjectColl` — the **diamond** the collision code actually
/// tests, with points at `(0, top)`, `(±width, center)` and `(0, bottom)`.
/// Those four numbers are real extracted data (Mario's are `320, 190, 0, 150`),
/// so this is the body the game uses and not a stand-in sized by eye. Seeing
/// the waist sit at 190 of 320 rather than halfway is the visible confirmation
/// that `center` is a height and not a midpoint.
///
/// White while airborne, green once it has a floor, so landing is visible as a
/// colour change on the exact frame it happens.
///
/// Positions are divided by [`MODEL_SCALE`], the same as the collision overlay,
/// so all three of geometry, collision and fighter share one transform.
///
/// # Safety
///
/// The GE must be mid-frame; the vertex buffer is DMAed and must not be reused
/// until the list is submitted.
pub unsafe fn draw_fighter(
    pos: [f32; 3],
    coll: &ssb_game::ground::BodyColl,
    grounded: bool,
    base: &ScePspFMatrix4,
    gpu: &mut crate::gu::Gpu,
) {
    // Magenta grounded, white airborne. Not green: the overlay draws solid
    // floors in exactly `0xFF40_FF40`, so a grounded fighter standing on one
    // was the same colour as the line under its feet and disappeared into it.
    // Magenta is also the one hue the collision palette does not already use
    // -- green floors, red ceilings, blue and amber walls -- so the fighter
    // cannot be confused with any surface it happens to be touching.
    let color = if grounded { 0xFFFF_40FF } else { 0xFFFF_FFFF };
    let s = 1.0 / MODEL_SCALE;
    let (x, y, z) = (pos[0] * s, pos[1] * s, pos[2] * s);

    sys::sceGumMatrixMode(sys::MatrixMode::Model);
    sys::sceGumLoadMatrix(base);

    let v = |dx: f32, dy: f32| crate::gu::GuVertex::new(x + dx, y + dy, z, 0.0, 0.0, color);
    let buf = &mut *core::ptr::addr_of_mut!(LINE_BUF);

    let (top, center, bottom, half) = (
        coll.top * s,
        coll.center * s,
        coll.bottom * s,
        coll.width * s,
    );

    // The diamond, closed: bottom -> right -> top -> left -> bottom.
    buf.0[0] = v(0.0, bottom);
    buf.0[1] = v(half, center);
    buf.0[2] = v(0.0, top);
    buf.0[3] = v(-half, center);
    buf.0[4] = v(0.0, bottom);
    gpu.draw_line_strip(&buf.0[..5]);

    // A short tick at the origin: the floor query runs at `pos.y + bottom`,
    // and `bottom` is zero for every playable character, so this marks the
    // point that is actually tested against the surface.
    buf.0[0] = v(-half * 0.25, bottom);
    buf.0[1] = v(half * 0.25, bottom);
    gpu.draw_line_strip(&buf.0[..2]);
}

/// A stage's extent in normalised units, from its collision and its layers.
///
/// Both sources matter: collision alone misses a stage's scenery, and geometry
/// alone can be dominated by a skybox that swamps the playable area.
pub fn stage_bounds(
    pack: &Pack<'_>,
    stage: &ssb_rom::pack::StageDesc,
) -> Option<([f32; 3], [f32; 3])> {
    let mut min = [f32::MAX; 3];
    let mut max = [f32::MIN; 3];
    let mut any = false;

    for line in pack.stage_lines(stage) {
        for v in pack.line_vertices(&line) {
            let p = [v.x as f32 / MODEL_SCALE, v.y as f32 / MODEL_SCALE, 0.0];
            for axis in 0..3 {
                min[axis] = min[axis].min(p[axis]);
                max[axis] = max[axis].max(p[axis]);
            }
            any = true;
        }
    }
    for slot in stage.layers {
        if slot == ssb_rom::pack::StageDesc::NO_LAYER {
            continue;
        }
        let Some(object) = pack.object(slot) else {
            continue;
        };
        let Some((lo, hi)) = object_bounds(pack, &object) else {
            continue;
        };
        for axis in 0..3 {
            min[axis] = min[axis].min(lo[axis]);
            max[axis] = max[axis].max(hi[axis]);
        }
        any = true;
    }
    any.then_some((min, max))
}
