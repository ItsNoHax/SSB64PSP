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
    self, ClutPixelFormat, GuPrimitive, GuState, ScePspFMatrix4, ScePspFVector3, ScePspFVector4,
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
    /// `Some(target)` while `TEXTURE_BLEND` is the active texture function,
    /// `None` while it's the ordinary `Modulate` (RE-073). Tracked
    /// independently of `last_flags`/`last_texture`: two primitives can share
    /// identical `flags` (both `TEXTURE_BLEND`) but different target colours,
    /// or identical flags but a texture change that would otherwise silently
    /// reset the texture function back to `Modulate` — either alone would
    /// under-count a real state change if this piggybacked on those fields.
    last_texture_blend: Option<u32>,
    pub draws: u32,
    pub triangles: u32,
    pub state_changes: u32,
    /// Debug-viewer-only override: suppresses culling entirely regardless
    /// of each primitive's own `CULL_BACK`/`CULL_FRONT` flags.
    ///
    /// A one-sided authored plane (several LB-transition "screen wipe"
    /// objects, RE-115) shows its front face only from whichever side the
    /// ROM's own game code always views it from; the debug viewer's
    /// free-roaming inspection camera has no such guarantee and can land on
    /// the back side, rendering nothing despite correct geometry, UV and
    /// texture data (measured on files 41/43/50: `object_bounds` computes a
    /// sane non-degenerate radius and centre, `draws` is non-zero, yet
    /// nothing appears — disabling culling entirely made the geometry
    /// visible immediately). Real gameplay rendering must never set this:
    /// `apply_material`'s ordinary per-primitive culling already reproduces
    /// the ROM's own `CULL_BACK`/`CULL_FRONT` state faithfully (RE-068), and
    /// a real camera always views authored geometry from its intended side.
    pub force_no_cull: bool,
    /// The real camera's own basis vectors this frame, or `None` while the
    /// active view matrix is identity (RE-131: the debug viewer's
    /// whole-stage overview and every other mode besides the zoomed-in
    /// fighter-follow one).
    ///
    /// `billboard_place` (`Kind46`'s screen-aligned transform, RE-048/049)
    /// used to load the model matrix as pure identity and rely on the *view*
    /// matrix always being identity too, so "the object's own X/Y axes"
    /// happened to already equal "the screen's X/Y axes" -- true only for a
    /// camera that never rotates. RE-131 gave the debug viewer a real,
    /// rotating camera, which silently broke that assumption for any
    /// billboard rendered under it (found while wiring the camera in, not
    /// by a separate audit): with `None` here, behaviour for every existing
    /// caller is bit-for-bit unchanged; a caller that has set up a real view
    /// matrix must also set this so `Kind46`'s billboards keep facing it.
    pub billboard_camera: Option<BillboardCamera>,
}

/// The real camera's basis, resolved once per frame into the two shapes
/// this project's billboard code can reproduce (RE-132).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BillboardCamera {
    /// `Kind46`: fully screen-aligned, `(camera.right, camera.up)`.
    pub screen: ([f32; 3], [f32; 3]),
    /// `Kind48`: camera-pitch-locked (`objdisplay.c` case 48,
    /// `sGCMatrixMod1F`) -- `right` is always a world horizontal axis
    /// (invariant to the camera's yaw), while `up`/the implied forward tilt
    /// with the camera's real vertical angle. See
    /// `NodeDesc::FLAG_BILLBOARD_PITCH_LOCKED`'s own doc comment for the
    /// decomp evidence this is the real, reachable branch during normal
    /// SSB64 gameplay, not a guess.
    pub pitch_locked: ([f32; 3], [f32; 3]),
}

impl DrawState {
    pub fn begin_frame(&mut self) {
        self.last_texture = None;
        self.last_flags = None;
        self.last_texture_blend = None;
        self.draws = 0;
        self.triangles = 0;
        self.state_changes = 0;
        // Not reset here: `force_no_cull` is set once per frame by the
        // caller (main.rs), based on which debug-viewer mode is active, and
        // must survive `begin_frame`'s reset of everything else.
    }

    /// Forgets the cached texture binding, forcing the next primitive to
    /// rebind (and re-enable `GuState::Texture2D`) unconditionally.
    ///
    /// R0.15: `Gpu::draw_triangles`/`draw_line_strip` (used by the collision
    /// overlay and simulated-fighter marker, `draw_collision`/`draw_fighter`
    /// below) call `sceGuDisable(GuState::Texture2D)` directly, bypassing
    /// this cache entirely. `apply_material`'s own texture-change check
    /// (`last_texture != Some(p.texture)`) has no way to know that happened
    /// -- if the next real primitive drawn afterward happens to name the
    /// *same* texture index as whatever was bound before the overlay ran
    /// (plausible: the pack dedups textures by content, so two unrelated
    /// objects sharing one small/common texture is a real, if not
    /// guaranteed, case), the cache wrongly concludes nothing changed and
    /// leaves texturing disabled. Callers of `draw_triangles`/
    /// `draw_line_strip` that run between two cached mesh draws in the same
    /// frame must call this afterward so the next one always rebinds for
    /// real, rather than trusting a comparison a side channel already
    /// invalidated.
    pub fn forget_texture(&mut self) {
        self.last_texture = None;
    }
}

/// The GE accepts eight mip levels.
const MAX_GE_MIP_LEVELS: usize = 8;

fn mip_level(level: usize) -> sys::MipmapLevel {
    match level {
        0 => sys::MipmapLevel::None,
        1 => sys::MipmapLevel::Level1,
        2 => sys::MipmapLevel::Level2,
        3 => sys::MipmapLevel::Level3,
        4 => sys::MipmapLevel::Level4,
        5 => sys::MipmapLevel::Level5,
        6 => sys::MipmapLevel::Level6,
        _ => sys::MipmapLevel::Level7,
    }
}

/// Bits per texel, matching `ssb_rom::psp_texture::Psm::bits`.
fn psm_bits(psm: TexturePixelFormat) -> usize {
    match psm {
        TexturePixelFormat::PsmT4 => 4,
        TexturePixelFormat::PsmT8 => 8,
        TexturePixelFormat::Psm8888 => 32,
        _ => 16,
    }
}

/// Binds a texture from the pack.
///
/// `mat_anim` overrides the baked CLUT with the current frame's resolved
/// `PaletteID` variant when `t.mat_anim` names one (RE-089–095) — issued
/// *after* the static load above so it always wins, but only when there is
/// a live animator and it actually has a value (a texture whose animation
/// has not started ticking yet, or a device build with no animator at all,
/// keeps its baked palette rather than showing nothing).
///
/// # Safety
///
/// The pack buffer must outlive the frame; the GE reads it asynchronously.
unsafe fn bind_texture(
    pack: &Pack<'_>,
    t: &TextureDesc,
    mat_anim: Option<&ssb_rom::skeleton::MaterialAnimator>,
) {
    // A `ROLE_FRAMEBUFFER` texture has no baked bytes at all (RE-099/RE-100)
    // -- `pack.texture_data` would return an empty slice, not `None`, so it
    // must be intercepted here rather than falling into the ordinary path
    // below. The real pixels live in `crate::gu`'s transition-photo capture,
    // filled in by `Gpu::request_transition_capture` the first time the LB
    // "loading transition" system starts.
    if t.role == TextureDesc::ROLE_FRAMEBUFFER {
        sys::sceGuEnable(GuState::Texture2D);
        let psm = psm_of(t.psm);
        sys::sceGuTexMode(psm, 0, 0, 0);
        let data = crate::gu::transition_photo_data();
        // Matches the general path below: the GE addresses `t.stride`
        // (padded power of two), never `t.width`, and `sceGuTexScale`'s
        // denominators must be exactly what was handed to `sceGuTexImage`.
        let w = t.stride as i32;
        let h = (t.height as u32).next_power_of_two() as i32;
        sys::sceGuTexImage(mip_level(0), w, h, w, data.as_ptr() as *const c_void);
        sys::sceGuTexFilter(sys::TextureFilter::Linear, sys::TextureFilter::Linear);
        const UV_SCALE: f32 = VERTEX_16BIT_DIVISOR / 32.0;
        sys::sceGuTexScale(UV_SCALE / w as f32, UV_SCALE / h as f32);
        sys::sceGuTexWrap(sys::GuTexWrapMode::Repeat, sys::GuTexWrapMode::Repeat);
        sys::sceGuTexOffset(0.0, 0.0);
        return;
    }

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

    if t.mat_anim != TextureDesc::NO_ANIM {
        if let Some(animated) = mat_anim
            .and_then(|m| m.resolved_palette(pack, t.mat_anim))
            .and_then(|i| pack.mat_anim_palette(i))
            .and_then(|p| pack.mat_anim_palette_data(&p))
        {
            let blocks = (animated.len() as i32 / 4 + 7) / 8;
            sys::sceGuClutLoad(blocks, animated.as_ptr() as *const c_void);
        }
    }

    // Mip levels sit back to back after level 0, each half the size of the one
    // before. Uploading them is what stops a dithered N64 gradient aliasing
    // into moire when a surface samples it at around one texel per pixel — see
    // RE-053 and Dream Land's tree.
    let top = (t.levels as usize).clamp(1, MAX_GE_MIP_LEVELS) - 1;
    sys::sceGuTexMode(psm, top as i32, 0, t.swizzled as i32);
    let mut offset = 0usize;
    let mut w = t.stride as u32;
    let mut h = (t.height as u32).next_power_of_two();
    for level in 0..=top {
        let stride_bytes = (w as usize * psm_bits(psm)).div_ceil(8);
        let size = stride_bytes * h as usize;
        let Some(slice) = data.get(offset..offset + size) else {
            break;
        };
        sys::sceGuTexImage(
            mip_level(level),
            w as i32,
            h as i32,
            w as i32,
            slice.as_ptr() as *const c_void,
        );
        offset += size;
        w = (w / 2).max(1);
        h = (h / 2).max(1);
    }
    // The texture function is *not* set here (it used to be, always
    // `Modulate`): `apply_material`'s per-primitive `TEXTURE_BLEND` handling
    // (RE-073) needs to survive a texture change without being silently
    // reset back to `Modulate` by this function running in between, so it is
    // the sole place that sets it for the mesh-drawing path now. Callers
    // outside that path (`draw_texture_quad`) set their own.
    if top > 0 {
        // Trilinear: the level below is what carries the averaged-out dither,
        // and blending between levels stops the switch-over being visible as a
        // band across the surface.
        sys::sceGuTexLevelMode(sys::TextureLevelMode::Auto, 0.0);
        sys::sceGuTexFilter(
            sys::TextureFilter::LinearMipmapLinear,
            sys::TextureFilter::Linear,
        );
    } else {
        sys::sceGuTexFilter(sys::TextureFilter::Linear, sys::TextureFilter::Linear);
    }
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
    // 64-wide texture), so most textures must tile rather than clamp -- RE-066
    // measured every tile-0 `G_SETTILE` in the ROM (754, archive-wide) and
    // found clamp/mirror is *only ever* requested on an axis whose own mask
    // is also nonzero. RE-066 read that as "clamp is always redundant with
    // mask-based narrowing" and always used `Repeat`, but RE-102 found a
    // counter-example on fighter face textures: `current_texture()`'s
    // `RE-044` narrowing only *shrinks* width/height when the mask period is
    // smaller than the drawn rect, and on these it is not (mask 32, drawn
    // rect 24) -- so narrowing is a no-op, real hardware clamps at that
    // undisturbed 24, and a UV that runs well past it (measured up to 110
    // texels on one) must not tile. `t.wrap` (`TextureDesc::CLAMP_S`/
    // `CLAMP_T`) carries the real `cms`/`cmt` clamp bit per axis for exactly
    // this case, independent of mirroring (RE-067's pre-baked flipped copy):
    // `cms`/`cmt` == 3 (both bits) is real hardware's "mirror once, then
    // clamp", and clamping past RE-067's doubled texture reproduces that
    // exactly, since the last sampled row/column of the doubled image *is*
    // the held edge. Several fighters' torso/head textures are exactly this
    // combination with UVs overflowing the mirrored pair by 2x or more
    // (Fox, Captain Falcon, Kirby) -- see `mesh::TextureRef::clamp_s`/
    // `clamp_t`.
    let wrap_of = |clamp: bool| {
        if clamp {
            sys::GuTexWrapMode::Clamp
        } else {
            sys::GuTexWrapMode::Repeat
        }
    };
    sys::sceGuTexWrap(
        wrap_of(t.wrap & TextureDesc::CLAMP_S != 0),
        wrap_of(t.wrap & TextureDesc::CLAMP_T != 0),
    );
    sys::sceGuTexOffset(0.0, 0.0);
}

/// Applies a primitive's material state.
unsafe fn apply_material(
    pack: &Pack<'_>,
    p: &PrimDesc,
    st: &mut DrawState,
    mat_anim: Option<&ssb_rom::skeleton::MaterialAnimator>,
) {
    if st.last_flags != Some(p.flags) {
        st.last_flags = Some(p.flags);
        st.state_changes += 1;

        let cull = !st.force_no_cull && p.flags & (flags::CULL_BACK | flags::CULL_FRONT) != 0;
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

        // `Z_BUFFER` is the real per-primitive signal (RE-068): the RDP's
        // per-frame reset (`refs/ssb-decomp-re/src/sys/rdp.c`'s
        // `sSYRdpResetDisplayList`) turns depth testing on by default, so a
        // node whose own list never mentions it is not "unknown", it is
        // z-buffered like everything else -- 98.3% of packed primitives
        // carry the flag. The 1.7% that clear it (an always-on-top overlay,
        // typically) must not be depth-tested against geometry drawn under
        // the default.
        if p.flags & flags::Z_BUFFER != 0 {
            sys::sceGuEnable(GuState::DepthTest);
        } else {
            sys::sceGuDisable(GuState::DepthTest);
        }

        // A cutout surface (foliage, grates): the RDP resolves
        // `CVG_X_ALPHA | ALPHA_CVG_SEL` through multisampled edge coverage
        // the GE has no equivalent for. `sf64-psp` -- a real, shipped
        // N64-to-PSP port making this same translation at runtime --
        // approximates it with a plain alpha test discarding
        // fully-transparent texels (RE-069); matched here rather than
        // invented.
        if p.flags & flags::ALPHA_TEST != 0 {
            sys::sceGuEnable(GuState::AlphaTest);
            sys::sceGuAlphaFunc(sys::AlphaFunc::Greater, 0, 0xFF);
        } else {
            sys::sceGuDisable(GuState::AlphaTest);
        }

        // `TRANSLUCENT` alone was deliberately not wired to `GuState::Blend`
        // for a long time (RE-069 through RE-071): enabling it unconditionally
        // on Dream Land's own translucent highlight surface (file 104's list
        // at 0x708/0x820/0xA78) produced a harsh checkerboard, and re-testing
        // after RE-070's dither fix made it *worse* (blown-out highlights).
        // RE-129 found the real cause: this project never decoded
        // `G_SETCOMBINE`'s *alpha* formula at all (only the colour one), so
        // "enable blend, let the GE's existing `Modulate` do the rest" was
        // silently multiplying texture alpha by whatever a vertex's own alpha
        // byte happened to hold -- meaningful `SHADE_ALPHA` for this one
        // surface, but garbage for most `TRANSLUCENT` primitives archive-wide
        // (`push_vertex`'s own long-standing note: "Mario's vertices are all
        // zero"). RE-130 classified the real shapes and `flags::ALPHA_BLEND`
        // (`crates/ssb-rom/src/pack.rs`) now only sets alongside `TRANSLUCENT`
        // when a primitive's vertices were actually baked for one of them --
        // gating on both together is what makes this finally safe.
        if p.flags & (flags::TRANSLUCENT | flags::ALPHA_BLEND)
            == (flags::TRANSLUCENT | flags::ALPHA_BLEND)
        {
            sys::sceGuEnable(GuState::Blend);
            sys::sceGuBlendFunc(
                sys::BlendOp::Add,
                sys::BlendFactor::SrcAlpha,
                sys::BlendFactor::OneMinusSrcAlpha,
                0,
                0,
            );
        } else {
            sys::sceGuDisable(GuState::Blend);
        }
    }

    if st.last_texture != Some(p.texture) {
        st.last_texture = Some(p.texture);
        st.state_changes += 1;
        match pack.texture(p.texture) {
            Some(t) => bind_texture(pack, &t, mat_anim),
            None => sys::sceGuDisable(GuState::Texture2D),
        }
    }

    // `TEXTURE_BLEND` (RE-073): `(PRIM-ENV)*TEXEL+ENV`, a texture-driven
    // blend from a base colour (ENV, baked into the vertex by
    // `crates/ssb-rom/src/mesh.rs`'s `push_vertex`) to a target colour
    // (PRIM, carried here as `texture_blend_target`) with no shade
    // dependence at all -- measured on 72 of 79 `SetCombine` commands
    // archive-wide that read `ENVIRONMENT`, including Link, Ness and
    // Pikachu's own base models. Maps exactly onto the GE's native
    // `TextureEffect::Blend` (`Cv = Cf*(1-Ct) + Cc*Ct`) with `Cf` the vertex
    // colour and `Cc` set via `sceGuTexEnvColor`, at no VRAM cost.
    //
    // Tracked independently of `last_flags`/`last_texture` above: two
    // primitives can share identical `flags` (both `TEXTURE_BLEND`) with
    // different target colours, and `bind_texture` no longer sets the
    // texture function itself for exactly this reason -- either omission
    // would silently leave a stale `Blend`/`Modulate` state or a stale
    // `sceGuTexEnvColor` active on a primitive that needed a different one.
    let blend_target = (p.flags & flags::TEXTURE_BLEND != 0).then_some(p.texture_blend_target);
    if st.last_texture_blend != blend_target {
        st.last_texture_blend = blend_target;
        st.state_changes += 1;
        match blend_target {
            Some(target) => {
                sys::sceGuTexFunc(sys::TextureEffect::Blend, sys::TextureColorComponent::Rgba);
                sys::sceGuTexEnvColor(target);
            }
            None => {
                sys::sceGuTexFunc(
                    sys::TextureEffect::Modulate,
                    sys::TextureColorComponent::Rgba,
                );
            }
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
pub unsafe fn draw_mesh(
    pack: &Pack<'_>,
    mesh: &MeshDesc,
    st: &mut DrawState,
    mat_anim: Option<&ssb_rom::skeleton::MaterialAnimator>,
) -> u32 {
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

        apply_material(pack, &p, st, mat_anim);

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
    mat_anim: Option<&ssb_rom::skeleton::MaterialAnimator>,
    costume: u32,
) -> u32 {
    draw_object_posed(pack, object, base, &[], st, mat_anim, costume)
}

/// Places a screen-aligned sprite, and returns its composed position and scale.
///
/// `gcPrepDObjMatrix`'s kinds 45-48 never multiply the node's rotation into the
/// MVP: they write it straight from the projection basis with every cross term
/// zeroed, so object X and Y land on screen X and Y and `rotate.x` only spins
/// the sprite within that plane (RE-048).
///
/// This build keeps the view matrix at identity and puts the whole camera into
/// `base`, so "aligned with the eye" is "unrotated in world space". The sprite
/// therefore wants the *composed* position and scale with an identity
/// orientation — which is `base * local` with its rotation discarded.
///
/// Scale comes from the length of the composed basis vectors rather than from
/// `rest_scale`, because a node inherits its ancestors' scale and only the
/// composed matrix knows the product.
fn billboard_place(base: &ScePspFMatrix4, local: &ScePspFMatrix4) -> ([f32; 3], f32, f32) {
    let b = [
        [base.x.x, base.x.y, base.x.z],
        [base.y.x, base.y.y, base.y.z],
        [base.z.x, base.z.y, base.z.z],
    ];
    let l = [
        [local.x.x, local.x.y, local.x.z],
        [local.y.x, local.y.y, local.y.z],
        [local.z.x, local.z.y, local.z.z],
    ];
    // Column `c` of `base * local`; its length is that axis' composed scale.
    let col = |c: usize| {
        [
            b[0][c] * l[0][0] + b[1][c] * l[0][1] + b[2][c] * l[0][2],
            b[0][c] * l[1][0] + b[1][c] * l[1][1] + b[2][c] * l[1][2],
            b[0][c] * l[2][0] + b[1][c] * l[2][1] + b[2][c] * l[2][2],
        ]
    };
    let len = |v: [f32; 3]| ssb_engine::math::sqrt(v[0] * v[0] + v[1] * v[1] + v[2] * v[2]);
    let t = [local.w.x, local.w.y, local.w.z];
    let pos = [
        b[0][0] * t[0] + b[1][0] * t[1] + b[2][0] * t[2] + base.w.x,
        b[0][1] * t[0] + b[1][1] * t[1] + b[2][1] * t[2] + base.w.y,
        b[0][2] * t[0] + b[1][2] * t[1] + b[2][2] * t[2] + base.w.z,
    ];
    (pos, len(col(0)), len(col(1)))
}

/// Draws an object under per-node matrices supplied by the caller.
///
/// `posed[i]` replaces node `first_node + i`'s baked matrix. A short slice
/// falls back to the baked one per node, so passing `&[]` is exactly
/// [`draw_object`] — the animated and static paths are the same code, which is
/// what keeps a bug in one from being invisible in the other.
///
/// `costume` looks up [`Pack::costume_mesh`] (RE-098) for every node before
/// falling back to its own baked mesh; `0` is exactly "no substitution",
/// since costume 0 is never stored as an override, so every existing caller
/// that has no notion of costume selection can pass `0` for identical output
/// to before this parameter existed.
///
/// # Safety
///
/// Same as [`draw_mesh`].
pub unsafe fn draw_object_posed(
    pack: &Pack<'_>,
    object: &ObjectDesc,
    base: &ScePspFMatrix4,
    posed: &[ssb_rom::scene::Mat4],
    st: &mut DrawState,
    mat_anim: Option<&ssb_rom::skeleton::MaterialAnimator>,
    costume: u32,
) -> u32 {
    let mut tris = 0;
    for i in 0..object.node_count {
        let global_node = object.first_node + i;
        let Some(node) = pack.node(global_node) else {
            continue;
        };
        let mesh_index = pack
            .costume_mesh(global_node, costume)
            .unwrap_or(node.mesh);
        if mesh_index == NodeDesc::NO_MESH {
            continue; // pure transform: a joint with no geometry
        }
        let Some(mesh) = pack.mesh(mesh_index) else {
            continue;
        };

        let node = match posed.get(i as usize) {
            Some(m) => NodeDesc {
                world: m.0,
                ..node
            },
            None => node,
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
        if node.flags & NodeDesc::FLAG_BILLBOARD != 0 {
            let (pos, sx, sy) = billboard_place(base, &local);
            sys::sceGumLoadIdentity();
            sys::sceGumTranslate(&ScePspFVector3 { x: pos[0], y: pos[1], z: pos[2] });
            // RE-131: under a real (non-identity) view matrix, the object's
            // own axes must be set to the camera's own right/up/forward for
            // this to still be screen-aligned -- see `DrawState::
            // billboard_camera`'s own doc comment for why this was not
            // needed before. `None` (every mode except the real camera's
            // own) leaves the basis at whatever `sceGumLoadIdentity` just
            // set, identical to this code before RE-131.
            //
            // RE-132: `Kind48` gets the pitch-locked basis instead of the
            // fully screen-aligned one -- see `NodeDesc::
            // FLAG_BILLBOARD_PITCH_LOCKED`'s own doc comment for why this
            // is the real, reachable transform, not `Kind46`'s.
            if let Some((right, up)) = st.billboard_camera.map(|bc| {
                if node.flags & NodeDesc::FLAG_BILLBOARD_PITCH_LOCKED != 0 {
                    bc.pitch_locked
                } else {
                    bc.screen
                }
            }) {
                let forward = [
                    right[1] * up[2] - right[2] * up[1],
                    right[2] * up[0] - right[0] * up[2],
                    right[0] * up[1] - right[1] * up[0],
                ];
                sys::sceGumMultMatrix(&ScePspFMatrix4 {
                    x: ScePspFVector4 { x: right[0], y: right[1], z: right[2], w: 0.0 },
                    y: ScePspFVector4 { x: up[0], y: up[1], z: up[2], w: 0.0 },
                    z: ScePspFVector4 { x: forward[0], y: forward[1], z: forward[2], w: 0.0 },
                    w: ScePspFVector4 { x: 0.0, y: 0.0, z: 0.0, w: 1.0 },
                });
            }
            // `rotate.x` is the in-plane spin; with the eye axes aligned to
            // world axes (or, now, the real camera's own axes) that is a
            // rotation about local Z.
            sys::sceGumRotateZ(node.rest_rotate[0]);
            sys::sceGumScale(&ScePspFVector3 { x: sx, y: sy, z: 1.0 });
        } else {
            sys::sceGumLoadMatrix(base);
            sys::sceGumMultMatrix(&local);
        }

        tris += draw_mesh(pack, &mesh, st, mat_anim);
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
    bind_texture(pack, &t, None);
    // `bind_texture` no longer sets this itself (RE-073); this diagnostic
    // always wants the plain, unblended sample.
    sys::sceGuTexFunc(
        sys::TextureEffect::Modulate,
        sys::TextureColorComponent::Rgba,
    );

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
    mat_anim: Option<&ssb_rom::skeleton::MaterialAnimator>,
) -> (u32, u32) {
    draw_stage_animated(pack, stage, base, None, st, mat_anim)
}

/// Draws a stage, optionally posed by its scenery animation.
///
/// `anim` recomposes each layer's node matrices before drawing it. A node the
/// animation does not drive keeps its packed rest matrix, so passing `None` is
/// exactly [`draw_stage`] — the still and moving paths are one piece of code,
/// which is what stops a bug in one hiding in the other (RE-051).
///
/// # Safety
///
/// Same as [`draw_mesh`].
pub unsafe fn draw_stage_animated(
    pack: &Pack<'_>,
    stage: &ssb_rom::pack::StageDesc,
    base: &ScePspFMatrix4,
    anim: Option<&ssb_rom::skeleton::StageAnimator>,
    st: &mut DrawState,
    mat_anim: Option<&ssb_rom::skeleton::MaterialAnimator>,
) -> (u32, u32) {
    let mut tris = 0;
    let mut drawn = 0;
    let mut posed = [ssb_rom::scene::Mat4::IDENTITY; ssb_rom::skeleton::MAX_NODES];
    for slot in stage.layers {
        if slot == ssb_rom::pack::StageDesc::NO_LAYER {
            continue;
        }
        let Some(object) = pack.object(slot) else {
            continue;
        };
        tris += match anim {
            Some(a) => {
                let n = a.compose(pack, &object, &mut posed);
                draw_object_posed(pack, &object, base, &posed[..n], st, mat_anim, 0)
            }
            None => draw_object(pack, &object, base, st, mat_anim, 0),
        };
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
    draw_state: &mut DrawState,
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
    // R0.15/RE-118: `draw_line_strip` disabled `Texture2D` directly above,
    // bypassing `draw_state`'s own cache -- the next real primitive drawn
    // must not trust a stale `last_texture` that no longer reflects the
    // GE's actual state.
    if segments > 0 {
        draw_state.forget_texture();
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
    draw_state: &mut DrawState,
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

    // R0.15/RE-118: see `draw_collision`'s own comment -- `draw_line_strip`
    // just disabled `Texture2D` directly, bypassing `draw_state`'s cache.
    draw_state.forget_texture();
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
