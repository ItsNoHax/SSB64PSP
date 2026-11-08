//! Mesh packing: draw order, texture keys and filter coverage, and the
//! per-primitive conversion into the pack.

use crate::*;

/// One display list a scene graph draws, and the space it draws it in.
#[derive(Clone, Copy)]
pub struct PlannedList {
    /// Index of the owning node within its graph.
    pub node: usize,
    /// File-relative offset of the display list.
    pub dl: u32,
    /// Modelview in effect while the list runs: the index of the node whose
    /// matrix is current, or `None` for the object root.
    ///
    /// Usually the node's own. A `Gfx *dls[2]` pair's first list draws *before*
    /// the node's matrix is pushed, so it runs in the parent's space instead.
    pub space: Option<usize>,
    pub world: ssb_rom::scene::Mat4,
    /// Which task-list command head (`gSYTaskmanDLHeads[N]`) a `DObjDLLink`
    /// entry targets (RE-245/RE-250); `None` for a `Direct`/`Pair` list, which
    /// has no such concept -- `gcDrawDObjTreeForGObj`'s own tree walk always
    /// runs on list 0.
    pub list_id: Option<u32>,
}

impl PlannedList {
    /// Whether this list can be placed on the node itself. A node holds one
    /// mesh, so anything else has to become an extra leaf; see
    /// `PackWriter::add_object`.
    pub fn own_space(&self) -> bool {
        self.space == Some(self.node)
    }
}

/// A node with no display list still occupies a slot in the draw sequence, so
/// that vertex-cache state lines up with the game's own walk.
pub const NO_LIST: u32 = u32::MAX;

/// Flattens a graph into the order `gcDrawDObjTree*` would draw it.
///
/// That order is simply the `DObjDesc` array order: `gcAddChildForDObj` appends
/// each new node to the tail of its parent's sibling list, and the draw walk is
/// node-then-child-then-siblings, so the pre-order flattening the array already
/// is round-trips exactly. Nothing needs sorting.
pub fn plan_draw_order(
    file: &ssb_rom::archive::File,
    graph: &ssb_rom::scene::SceneGraph,
    resolver: &ssb_rom::scene::DlResolver,
) -> Vec<PlannedList> {
    use ssb_rom::scene::{Mat4, NodeDl};

    let worlds = graph.world_transforms();
    let mut out = Vec::with_capacity(graph.nodes.len());

    for (i, node) in graph.nodes.iter().enumerate() {
        let Some(node_dl) = node.desc.dl else {
            continue;
        };
        let own = |dl, list_id| PlannedList {
            node: i,
            dl,
            space: Some(i),
            world: worlds[i],
            list_id,
        };
        match resolver.resolve(node_dl) {
            NodeDl::Direct(dl) => out.push(own(
                dl,
                // A manager effect or weapon whose display draws its whole
                // object on gSYTaskmanDLHeads[1] (`ssb_rom::task_head`).
                (whole_object_stream((file.id, graph.offset)) == 1).then_some(1),
            )),
            NodeDl::Links(links) => out.extend(
                links
                    .iter()
                    .filter_map(|l| l.dl.map(|dl| own(dl, Some(l.list_id)))),
            ),
            NodeDl::Pair { pre, post } => {
                if let Some(dl) = pre {
                    out.push(PlannedList {
                        node: i,
                        dl,
                        space: node.parent,
                        world: node.parent.map_or(Mat4::IDENTITY, |p| worlds[p]),
                        list_id: None,
                    });
                }
                // The node's matrix is pushed between the two, so even when
                // `post` is NULL the node still occupies a step in the walk.
                out.push(own(post.unwrap_or(NO_LIST), None));
            }
        }
    }
    out
}

/// Joint entries in a built pack that name both a script and a node.
///
/// The gap between this and the total is the two ways a joint can be inert: an
/// animation that does not move it, and a spare runtime-joint entry a table
/// carries for one of `TransN`, `XRotN`, or `YRotN` (RE-036).
pub fn pack_anim_joints_bound(pack: &ssb_rom::pack::Pack<'_>) -> usize {
    (0..pack.anim_joint_count())
        .filter_map(|i| pack.anim_joint(i))
        .filter(|j| {
            j.script != ssb_rom::pack::AnimJoint::NO_SCRIPT
                && j.node != ssb_rom::pack::AnimJoint::NO_NODE
        })
        .count()
}

/// Where a primitive's texels may be found.
///
/// Two files rather than one, because a texture need not live in the file that
/// draws it: every stage reaches its texels through a pointer into a separate
/// file (RE-037).
#[derive(Clone, Copy)]
pub struct Texels<'a> {
    /// The file the display list came from.
    pub home: &'a ssb_rom::archive::File,
    /// The whole archive, for a reference that names another file.
    pub all: &'a FileStore<'a>,
}

/// A file's bytes: borrowed from the home file, or held from the store.
pub enum FileData<'a> {
    Home(&'a [u8]),
    Other(alloc::rc::Rc<ssb_rom::archive::File>),
}

impl core::ops::Deref for FileData<'_> {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        match self {
            Self::Home(b) => b,
            Self::Other(f) => &f.data,
        }
    }
}

impl<'a> Texels<'a> {
    /// The bytes of the file a reference's half names, or of `home`.
    pub fn bytes(&self, which: Option<u16>) -> Option<FileData<'a>> {
        match which {
            None => Some(FileData::Home(&self.home.data[..])),
            Some(id) => self.all.get(id as usize).map(FileData::Other),
        }
    }
}

/// Adds a converted mesh to the pack, uploading any textures it samples.
///
/// `files` is the whole archive because a texture need not live in the file
/// that draws it: every stage reaches its texels through a pointer into a
/// separate file (RE-037). The cache key is therefore the file the *texels*
/// are in, not the one the display list came from — otherwise the four stages
/// sharing a texture file would each upload their own copy of it.
///
/// `mat_anim_data` is looked up whenever a primitive's material carries a
/// [`ssb_rom::mesh::MatAnimRef`] (RE-091): converts every resolved palette
/// variant the same way [`convert_texture`] converts the static one, and
/// points the texture at the resulting [`ssb_rom::pack::MatAnimDesc`] --
/// deduplicated via `mat_anim_index` the same way textures already are, so
/// many primitives sharing one script do not upload its palettes twice.
/// Keys the texture cache on every [`ssb_rom::mesh::TextureRef`] field
/// [`convert_texture`] actually reads to produce its baked bytes (RE-253),
/// plus the texel/palette file each `Option<u16>` resolves to. R0.16/RE-122
/// already moved this key from a bare `(image_file, image_offset,
/// palette_file, palette_offset)` 4-tuple to an 8-tuple adding `mirror_s`/
/// `mirror_t`/`clamp_s`/`clamp_t`, on exactly this reasoning -- state
/// `convert_texture` bakes into different bytes must be in the key, or
/// whichever primitive converts first silently donates its own bytes to
/// every other primitive sharing the narrower key.
///
/// RE-253 measured the 8-tuple still missing every other field
/// `convert_texture` reads: `width`/`height` (the loaded/cropped tile size),
/// `drawn_width`/`drawn_height` (the mirror-bake rect, RE-067), `format`/
/// `size`, and `palette_entries`/`palette` (the CI4 bank, RE-223). Two
/// primitives sharing one base image+palette+wrap but drawing a *different*
/// crop of it (an ordinary pattern for a shared character texture sheet)
/// silently shared one cache entry. Found while investigating why RE-252's
/// submission-order fix changed 11 of 15 golden scenes far more than its own
/// primitive-reorder effect could explain -- the reorder was simply picking
/// a different arbitrary "first" among already-colliding keys, not
/// introducing a new problem.
///
/// `format`/`size`/`palette_entries`/`palette` were first assumed safe to
/// leave out, on the theory that reading the same ROM address always decodes
/// it the same way -- an assumption, not yet a measurement, so
/// `texture_key_fields_never_vary_for_a_fixed_data_and_palette_location`
/// below was written to check it before shipping that gap. It measured the
/// assumption **false**: 1 real `(data_file, data_offset)` pair with two
/// different format/size, and 46 real `(palette_file, palette_offset)` pairs
/// with two different `palette_entries`/bank values, archive-wide. All four
/// fields are in the key below.
///
/// A first attempt keyed on the *entire* `TextureRef` instead of this
/// specific field set, reasoning from `merge_by_material`'s own whole-struct
/// `MeshMaterial` key (RE-252). That overcorrected: `origin_s`/`origin_t`/
/// `mask_s`/`mask_t`/`framebuffer` are real `TextureRef` fields but
/// `convert_texture` never reads any of them (they are draw-time UV/clamp
/// inputs consumed elsewhere, not baking inputs) — including them fragmented
/// legitimately-identical baked textures across every primitive with a
/// merely different absolute tile origin, inflating the archive-wide texture
/// count from 1,345 to 2,011 (+49.5%) instead of closing RE-253's measured
/// 45-texture gap. `MeshMaterial`'s whole-struct key is safe because every
/// one of its fields *is* real render state; `TextureRef` mixes baking
/// inputs with draw-time-only ones, so the correct key is the exhaustive
/// subset `convert_texture` depends on, not the whole struct.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct TexKey {
    pub data_file: u32,
    pub data_offset: u32,
    pub palette_file: u32,
    pub palette_offset: Option<u32>,
    pub format: ssb_rom::texture::Format,
    pub size: ssb_rom::texture::BitSize,
    pub width: u16,
    pub height: u16,
    pub source_width: u16,
    pub drawn_width: u16,
    pub drawn_height: u16,
    pub palette_entries: u16,
    pub palette: u8,
    /// `G_MDSFT_TEXTLUT` changes what the same bytes decode to (RE-313).
    pub tlut: ssb_rom::texture::TextureLut,
    pub mirror_s: bool,
    pub mirror_t: bool,
    pub clamp_s: bool,
    pub clamp_t: bool,
    pub rgb_unused: bool,
    pub rgb_lerp: Option<[[u8; 3]; 2]>,
    pub compensation_version: u8,
    pub coverage_hash: u64,
    pub animated_relationship: bool,
    pub animated_source: Option<ssb_rom::mesh::MatAnimRef>,
    pub alpha_policy: ssb_rom::filter_compensation::AlphaPolicy,
    /// Exact RE-312 dense variant; zero is the ordinary conversion.
    pub residual_variant: u16,
}

#[derive(Default)]
pub struct MatAnimData {
    pub source_offset: u32,
    pub palette_entries: u16,
    pub palettes: Vec<ssb_rom::mobj::Ptr>,
    pub sprites: Vec<ssb_rom::mobj::Ptr>,
    pub base_tracks: [u32; 10],
    pub uv_mode: u8,
    pub uv_tile_params: [u16; 3],
    /// RE-327: `unk24`/`unk28`/`unk44` bits for `unk10 == 1`.
    pub uv_half: [u32; 3],
    /// RE-318: the script can never move `TraU`/`TraV`/`ScaU`/`ScaV` off
    /// their rest values, so the primitive samples exactly its authored UVs.
    pub uv_static: bool,
    /// RE-321: the script drives `SetLFrac` or `TextureIDNext`, the two
    /// inputs of the two-tile fractional blend.
    pub drives_lod: bool,
    /// RE-321: the highest `sprites[]` index `TextureIDCurrent` reaches in the
    /// replay (0 when the track never moves: `gcAddMObjForDObj` zeroes it).
    /// `sprites` also covers `TextureIDNext`, which only tile 1 samples.
    pub max_current: usize,
    /// The replay of every script on this `MObj` ended or entered its
    /// cycle, so `max_current` bounds every `TextureIDCurrent` they write:
    /// a two-tile blend's `sprites` past it are tile-1 images only, and
    /// their tile-0 slots are left [`ssb_rom::pack::TextureDesc::NO_ANIM`]
    /// (RE-364).
    pub current_closed: bool,
    /// RE-424: the highest `PRIM_LOD_FRAC` byte the replay reaches, from the
    /// `SetLFrac` track or its rest value.
    pub max_lod_frac: u8,
    /// Other scripts the runtime restarts this `MObj` on (the boss
    /// wallpaper's shared tree, [`resolve_boss_mat_anims`]): their palette
    /// states join the base script's in [`reachable_palettes`].
    pub extra_scripts: Vec<u32>,
}

pub fn texture_cache_key(
    id: u32,
    t: &ssb_rom::mesh::TextureRef,
    coverage_hash: u64,
    animated_relationship: Option<ssb_rom::mesh::MatAnimRef>,
    alpha_policy: ssb_rom::filter_compensation::AlphaPolicy,
) -> TexKey {
    TexKey {
        data_file: t.data_file.map_or(id, u32::from),
        data_offset: t.data_offset,
        palette_file: t.palette_file.map_or(id, u32::from),
        palette_offset: t.palette_offset,
        format: t.format,
        size: t.size,
        width: t.width,
        height: t.height,
        source_width: t.source_width,
        drawn_width: t.drawn_width,
        drawn_height: t.drawn_height,
        palette_entries: t.palette_entries,
        palette: t.palette,
        tlut: t.tlut,
        mirror_s: t.mirror_s,
        mirror_t: t.mirror_t,
        clamp_s: t.clamp_s,
        clamp_t: t.clamp_t,
        rgb_unused: t.rgb_unused,
        rgb_lerp: t.rgb_lerp,
        compensation_version: ssb_rom::filter_compensation::ALGORITHM_VERSION,
        coverage_hash,
        animated_relationship: animated_relationship.is_some(),
        animated_source: animated_relationship,
        alpha_policy,
        residual_variant: 0,
    }
}

/// [`primitive_filter_coverage`] when a texture the primitive converts is
/// compensated ([`ssb_rom::filter_compensation::compensated`]) or the
/// residuals audit runs; else empty, since only compensation reads it
/// (RE-588: the on-PSP build skips it).
pub fn coverage_if_compensated(
    ctx: &PackCtx<'_>,
    compensated: bool,
    m: &ssb_rom::mesh::Mesh,
    prim: &ssb_rom::mesh::Primitive,
) -> filter_coverage::Coverage {
    if compensated || ctx.observer.enabled() {
        primitive_filter_coverage(ctx, m, prim)
    } else {
        filter_coverage::Coverage::default()
    }
}

/// Whether one of `sprites`, converted from `home`, is compensated.
pub fn any_compensated(ctx: &PackCtx<'_>, home: u32, sprites: &[ssb_rom::mobj::Ptr]) -> bool {
    sprites.iter().any(|p| listed(ctx, home, p.file, p.offset))
}

/// [`ssb_rom::filter_compensation::compensated`]; also every texture with
/// [`crate::PackOptions::list_all`] (tests of the solve itself).
pub fn listed(ctx: &PackCtx<'_>, home: u32, data_file: Option<u16>, data_offset: u32) -> bool {
    if ctx.opts.list_all {
        return true;
    }
    ssb_rom::filter_compensation::compensated(home, data_file, data_offset)
}

pub fn primitive_filter_coverage(
    ctx: &PackCtx<'_>,
    m: &ssb_rom::mesh::Mesh,
    prim: &ssb_rom::mesh::Primitive,
) -> filter_coverage::Coverage {
    // Authored UVs: the N=8 barycentric base plus bounded, cell-local filter
    // boundaries and a separate denser holdout.
    if let Some(scale) = prim.material.texgen_scale {
        return texgen_filter_coverage(ctx, m, prim, scale);
    }
    let triangles: Vec<_> = prim
        .indices
        .as_chunks::<3>()
        .0
        .iter()
        .map(|tri| [tri[0], tri[1], tri[2]].map(|i| m.vertices[i as usize].uv.map(i32::from)))
        .collect();
    filter_coverage::build(&triangles)
}

/// Texgen coverage from the primitive's real normals (RE-311).
///
/// Generated coordinates depend on the live look-at basis and node
/// transform. Neither is statically enumerable: `gmCameraLookAtFuncMatrix`
/// rebuilds the look-at from the camera's live eye/at every frame, and a
/// fighter's joint animation rotates the node freely. The real normals, the
/// `/127` dot, `gSPTexture` scale, tile origin/clamp and S10.5 truncation
/// are all static, so coverage runs them through an orientation quadrature
/// (`filter_coverage::build_texgen`) and bounds every pose by
/// `filter_coverage::TexgenBound`.
///
/// Falls back to the pre-RE-311 full-tile coverage when the generated
/// footprint cannot be bounded from static data: an animated material may
/// apply a live UV affine (`MaterialUv`) on top of the generated coordinates.
pub fn texgen_filter_coverage(
    ctx: &PackCtx<'_>,
    m: &ssb_rom::mesh::Mesh,
    prim: &ssb_rom::mesh::Primitive,
    scale: (u16, u16),
) -> filter_coverage::Coverage {
    let Some(t) = prim.material.texture else {
        return filter_coverage::Coverage::default();
    };
    if prim.material.mat_anim.is_some() || prim.indices.is_empty() {
        return filter_coverage::Coverage::default();
    }
    let normals: Vec<[[i8; 3]; 3]> = prim
        .indices
        .as_chunks::<3>()
        .0
        .iter()
        .map(|tri| {
            [tri[0], tri[1], tri[2]].map(|i| {
                let rgba = m.vertices[i as usize].rgba;
                [rgba[0] as i8, rgba[1] as i8, rgba[2] as i8]
            })
        })
        .collect();
    let input = filter_coverage::TexgenPrimitive {
        linear: prim.material.texture_gen == ssb_rom::mesh::TextureGen::Linear,
        normals: &normals,
        scale: [scale.0, scale.1],
        origin: [t.origin_s, t.origin_t],
        clamp: [t.clamp_s, t.clamp_t],
    };
    let coverage = filter_coverage::build_texgen(&input);
    let mode = ctx.opts.texgen_coverage;
    if mode != crate::TexgenCoverage::Exact {
        // Measurement only: reproduce the pre-RE-311 optimisation and gate
        // exactly, keeping the real sets for the report line. `full-tile`
        // reproduces the pre-RE-311 pack byte for byte.
        return filter_coverage::Coverage {
            train: Vec::new(),
            validation: Vec::new(),
            texgen: coverage.texgen.map(|meta| filter_coverage::TexgenMeta {
                legacy: true,
                per_primitive: mode == crate::TexgenCoverage::FullTilePerPrimitive,
                ..meta
            }),
        };
    }
    coverage
}

pub fn coverage_hash(coverage: &[[i32; 2]]) -> u64 {
    coverage
        .iter()
        .flatten()
        .fold(0xcbf29ce484222325u64, |h, &v| {
            (h ^ v as u64).wrapping_mul(0x100000001b3)
        })
}

pub fn validation_preserved(
    baseline: ssb_rom::filter_compensation::Metrics,
    candidate: ssb_rom::filter_compensation::Metrics,
    policy: ssb_rom::filter_compensation::AlphaPolicy,
) -> bool {
    baseline.samples > 0
        && candidate.samples == baseline.samples
        && candidate.squared_error <= baseline.squared_error
        && candidate.above_8 <= baseline.above_8
        && candidate.above_32 <= baseline.above_32
        && candidate.max <= baseline.max.saturating_add(8)
        && (policy != ssb_rom::filter_compensation::AlphaPolicy::Translucent
            || candidate.visible_squared_error <= baseline.visible_squared_error)
}

/// RE-321: whether every texel `t` can sample is fully opaque, read from the
/// source. Only CI through a TLUT is measured; anything else declines.
pub fn source_fully_opaque(src: Texels<'_>, t: &ssb_rom::mesh::TextureRef) -> Option<bool> {
    use ssb_rom::texture;
    if !t.tlut.enabled() || t.format != texture::Format::Ci {
        return None;
    }
    let file = src.bytes(t.data_file)?;
    let indices = decode_index_field(&file, t)?;
    let bank = if t.size == texture::BitSize::Bits4 {
        t.palette
    } else {
        0
    };
    let (off, n) = palette_bank_offset(t.palette_offset?, t.palette_entries, bank);
    let tlut = src
        .bytes(t.palette_file)?
        .get(off as usize..off as usize + n * 2)
        .map(texture::parse_tlut)?;
    Some(indices.iter().all(|&i| {
        tlut.get(usize::from(i))
            .is_some_and(|&e| t.tlut.entry_rgba(e)[3] == 255)
    }))
}

/// The RGBA of every texel of `t`'s window: a CI image through its TLUT (as
/// [`source_fully_opaque`]), or a direct image with the TLUT off. `None` for
/// any other pairing.
pub fn source_texels_rgba(src: Texels<'_>, t: &ssb_rom::mesh::TextureRef) -> Option<Vec<[u8; 4]>> {
    use ssb_rom::texture;
    let file = src.bytes(t.data_file)?;
    if t.format != texture::Format::Ci && !t.tlut.enabled() {
        let (w, h) = (u32::from(t.width), u32::from(t.height));
        let row = texture::data_len(u32::from(t.source_width.max(t.width)), 1, t.size);
        let used = texture::data_len(w, 1, t.size);
        let mut data = Vec::with_capacity(used * h as usize);
        for y in 0..h as usize {
            let at = t.data_offset as usize + y * row;
            data.extend_from_slice(file.get(at..at + used)?);
        }
        let img = texture::decode_lut(
            &data,
            w,
            h,
            t.format,
            t.size,
            texture::TextureLut::None,
            &[],
            0,
        )
        .ok()?;
        return Some((0..(w * h) as usize).map(|i| img.get(i)).collect());
    }
    if !t.tlut.enabled() || t.format != texture::Format::Ci {
        return None;
    }
    let indices = decode_index_field(&file, t)?;
    let bank = if t.size == texture::BitSize::Bits4 {
        t.palette
    } else {
        0
    };
    let (off, n) = palette_bank_offset(t.palette_offset?, t.palette_entries, bank);
    let tlut = src
        .bytes(t.palette_file)?
        .get(off as usize..off as usize + n * 2)
        .map(texture::parse_tlut)?;
    indices
        .iter()
        .map(|&i| tlut.get(usize::from(i)).map(|&e| t.tlut.entry_rgba(e)))
        .collect()
}

/// RE-321: the texel-alpha translucent lowering's proof for a two-tile blend
/// whose alpha keeps its texels (`lerp(T0a, T1a, f) * PRIM_ALPHA` under a
/// blending mode; Master Hand's Fall wallpaper): every texel of every
/// `TEXEL0` and `TEXEL1` image has one RGB, so both passes add the same
/// colour and only their alphas weight it
/// ([`ssb_rom::lod_blend::texel_alpha_pass_alphas`]). Returns the mean
/// `TEXEL0` alpha over the images the script reaches, rounded.
pub fn lod_texel_alpha_mean(
    src: Texels<'_>,
    current: impl Iterator<Item = ssb_rom::mesh::TextureRef>,
    next: impl Iterator<Item = ssb_rom::mesh::TextureRef>,
) -> Result<u8, &'static str> {
    let mut rgb = None;
    let mut same_rgb = |texels: &[[u8; 4]]| {
        texels.iter().all(|c| {
            let c = [c[0], c[1], c[2]];
            *rgb.get_or_insert(c) == c
        })
    };
    let (mut sum, mut count) = (0u64, 0u64);
    for t in current {
        let texels = source_texels_rgba(src, &t).ok_or("a TEXEL0 image did not decode")?;
        if !same_rgb(&texels) {
            return Err("the texel colours differ, so the passes' alphas would weight them");
        }
        sum += texels.iter().map(|c| u64::from(c[3])).sum::<u64>();
        count += texels.len() as u64;
    }
    for t in next {
        let texels = source_texels_rgba(src, &t).ok_or("a TEXEL1 image did not decode")?;
        if !same_rgb(&texels) {
            return Err("the texel colours differ, so the passes' alphas would weight them");
        }
    }
    if count == 0 {
        return Err("no TEXEL0 texel");
    }
    Ok(((sum + count / 2) / count) as u8)
}

/// RE-321: converts a two-tile blend primitive's `TEXEL1` images and checks
/// that the two-pass GE lowering reproduces its RDP output, returning the
/// record to pack or the reason to keep the single-texture path.
///
/// The lowering is exact when pass 1's pixel replaces the framebuffer and the
/// alpha gate cannot tell pass 1's `TEXEL0` alpha from the blended alpha:
///
/// * no GE blending (`translucent` with a classified alpha formula);
/// * no gate, or a gate with every `TEXEL0` image opaque and `PRIM_ALPHA`
///   255 whose threshold the lerp from 255 towards 0 at the script's largest
///   fraction still passes -- then the blended alpha always passes and pass
///   1's alpha is 255 (RE-424);
/// * no palette animation and no texgen, which the second pass does not
///   carry.
///
/// A translucent primitive whose second cycle multiplies the alpha by
/// `SHADE` (the Zebes acid) is exact under the premultiplied lowering
/// ([`ssb_rom::lod_blend`]) when every `TEXEL0` and `TEXEL1` image is opaque
/// (the combined alpha is then the shade alpha in both passes and on the
/// RDP, so the gate agrees too) and the primitive's vertices share one RGB
/// and no other primitive reads them ([`lod_premultiply_vertices`]).
///
/// `TEXEL1` images convert through the ordinary texture pipeline, shaped by
/// tile 1's own descriptor, under the opaque alpha policy: pass 2 blends by
/// fixed factors, and the RDP interpolates `TEXEL1`'s colour even where its
/// alpha is zero, so that colour must survive compensation.
#[allow(clippy::too_many_arguments)]
pub fn lod_blend_desc(
    ctx: &PackCtx<'_>,
    writer: &mut ssb_rom::pack::PackWriter,
    src: Texels<'_>,
    id: u32,
    offset: u32,
    prim_index: usize,
    packed_index: usize,
    m: &ssb_rom::mesh::Mesh,
    prim: &ssb_rom::mesh::Primitive,
    lod: &ssb_rom::mesh::LodBlend,
    mat_anim: u32,
    data: &MatAnimData,
    swizzle: bool,
    phase: [i16; 2],
) -> Result<ssb_rom::pack::LodBlendDesc, &'static str> {
    use ssb_rom::pack::AlphaGate;
    if !data.drives_lod {
        return Err("script drives neither SetLFrac nor TextureIDNext");
    }
    if data.sprites.is_empty() {
        return Err("no sprite table for TextureIDNext");
    }
    if !data.palettes.is_empty() {
        return Err("PaletteID animation is not carried by the second pass");
    }
    if prim.material.texture_gen != ssb_rom::mesh::TextureGen::None {
        return Err("texgen is not carried by the second pass");
    }
    let base = prim.material.texture.ok_or("no TEXEL0 texture")?;
    let (gate, translucent_blend) = ssb_rom::pack::material_alpha_state(&prim.material);
    let reached_current = || {
        let reached = data.sprites.iter().take(data.max_current + 1);
        core::iter::once(base).chain(reached.map(|p| ssb_rom::mesh::TextureRef {
            data_file: p.file,
            data_offset: p.offset,
            ..base
        }))
    };
    let mut texel_alpha = None;
    if translucent_blend
        && !lod.shade_alpha
        && matches!(
            prim.material.alpha_blend,
            Some(ssb_rom::mesh::AlphaBlend::Prim(_))
        )
    {
        // The runtime draws both passes with the alpha test off: the RDP
        // gates on the blended alpha, which no single pass carries. Only a
        // gate low enough that what it drops is invisible is accepted
        // (Master Hand's Fall: `G_AC_THRESHOLD` against 8).
        let floor = match gate {
            AlphaGate::Off => 0,
            AlphaGate::Greater(t) => u16::from(t) + 1,
            AlphaGate::GreaterOrEqual(t) => u16::from(t),
        };
        if floor > TEXEL_ALPHA_GATE_MAX {
            return Err("the alpha gate is too high to drop for the texel-alpha passes");
        }
        let next = data.sprites.iter().map(|p| ssb_rom::mesh::TextureRef {
            data_file: p.file,
            data_offset: p.offset,
            ..lod.next
        });
        let mean = lod_texel_alpha_mean(src, reached_current(), next)?;
        pack_log!(ctx,
            "lod texel-alpha file {id} list 0x{offset:X} prim {prim_index}: TEXEL0 0x{:X} sprites [{}] tile 1 origin ({},{}) tile 0 ({},{}) mean alpha {mean}",
            base.data_offset,
            data.sprites
                .iter()
                .map(|p| format!("{:?}+0x{:X}", p.file, p.offset))
                .collect::<Vec<_>>()
                .join(" "),
            lod.next.origin_s,
            lod.next.origin_t,
            base.origin_s,
            base.origin_t,
        );
        texel_alpha = Some(mean);
    } else if translucent_blend {
        if !lod.shade_alpha || prim.material.alpha_blend != Some(ssb_rom::mesh::AlphaBlend::Shade) {
            return Err("GE blending would weight pass 1 by its own alpha");
        }
        let next = data.sprites.iter().map(|p| ssb_rom::mesh::TextureRef {
            data_file: p.file,
            data_offset: p.offset,
            ..lod.next
        });
        for t in reached_current().chain(next) {
            if source_fully_opaque(src, &t) != Some(true) {
                return Err("a texel is not provably opaque under the shade-alpha blend");
            }
        }
        lod_premultiply_vertices(m, packed_index)?;
    } else if lod.shade_alpha && gate != AlphaGate::Off {
        return Err("the shade alpha can fall below the alpha gate");
    }
    // The least alpha the gate passes, and whether one is set.
    let floor = match gate {
        AlphaGate::Off => None,
        AlphaGate::Greater(t) => Some(u16::from(t) + 1),
        AlphaGate::GreaterOrEqual(t) => Some(u16::from(t)),
    };
    if let Some(floor) = floor.filter(|_| !translucent_blend && texel_alpha.is_none()) {
        // With every `TEXEL0` opaque and `PRIM_ALPHA` 255 the blended alpha
        // is at least the lerp from 255 towards 0 at the largest fraction
        // the script reaches; the gate must pass that (RE-424: the cutout
        // gate is 32, and Dream Land's pond reaches 114).
        let least = ssb_rom::lod_blend::rdp_combine_channel(0, 255, data.max_lod_frac, 255);
        if u16::from(least) < floor {
            return Err("the blended alpha can fall below the alpha gate");
        }
        if lod.prim_alpha != 255 {
            return Err("PRIM_ALPHA below 255 under an alpha gate");
        }
        for t in reached_current() {
            if source_fully_opaque(src, &t) != Some(true) {
                return Err("a TEXEL0 image is not provably opaque under an alpha gate");
            }
        }
    }
    let coverage = coverage_if_compensated(
        ctx,
        any_compensated(ctx, src.home.id, &data.sprites),
        m,
        prim,
    );
    let policy = ssb_rom::filter_compensation::AlphaPolicy::Opaque;
    let mut next_textures =
        [ssb_rom::pack::TextureDesc::NO_ANIM; ssb_rom::pack::MatAnimDesc::MAX_TEXTURES];
    for (slot, p) in data.sprites.iter().enumerate() {
        ctx.observer.discard_conversion();
        let (tex, variant, compensated) =
            convert_mat_anim_sprite(ctx, src, *p, &lod.next, &coverage, None, swizzle, policy)
                .ok_or("a TEXEL1 image did not convert")?;
        let i = writer.add_texture(&tex, lod.next.clamp_s, lod.next.clamp_t);
        record_texture_source(writer, i, src, &variant, compensated);
        ctx.observer
            .record_variant(i, &tex, lod.next.clamp_s, lod.next.clamp_t);
        if ctx.observer.enabled() {
            ctx.observer.record_site(crate::UseSite {
                variant: i,
                file: id,
                dl: offset,
                prim: prim_index,
                sprite_slot: Some(slot),
                policy,
                texgen: None,
                mat_anim: true,
                triangles: prim.triangle_count(),
                coverage: coverage.clone(),
                phase,
            });
        }
        next_textures[slot] = i;
    }
    Ok(ssb_rom::pack::LodBlendDesc {
        mat_anim,
        next_count: data.sprites.len() as u32,
        next_textures,
        tile1_params: lod.tile1_params.map(u32::from),
        origin_delta: lod.origin_delta,
        texel_alpha,
    })
}

/// The highest alpha-gate floor the texel-alpha translucent lowering drops
/// (RE-321): a pixel the RDP gates has alpha below it, so drawing it anyway
/// moves the framebuffer by under `8/255` of its distance to the colour.
pub const TEXEL_ALPHA_GATE_MAX: u16 = 8;

/// RE-321: the vertices primitive `packed` of `m` draws, when the translucent
/// two-tile lowering may premultiply them by their alpha
/// ([`ssb_rom::lod_blend::premultiply_shade`]): one RGB over all of them, so
/// the GE's interpolation of `S * a` equals the RDP's `S * a` per pixel, and
/// no other primitive reading them.
pub fn lod_premultiply_vertices(
    m: &ssb_rom::mesh::Mesh,
    packed: usize,
) -> Result<alloc::collections::BTreeSet<u16>, &'static str> {
    let used: alloc::collections::BTreeSet<u16> =
        m.primitives[packed].indices.iter().copied().collect();
    let mut rgb = used.iter().map(|&i| {
        let v = &m.vertices[usize::from(i)];
        (!v.lit).then_some([v.rgba[0], v.rgba[1], v.rgba[2]])
    });
    let first = rgb
        .next()
        .flatten()
        .ok_or("no unlit vertex colour to premultiply")?;
    if !rgb.all(|c| c == Some(first)) {
        return Err("the shade colour varies over the primitive");
    }
    let shared = m
        .primitives
        .iter()
        .enumerate()
        .any(|(k, p)| k != packed && p.indices.iter().any(|i| used.contains(i)));
    if shared {
        return Err("another primitive reads the vertices to premultiply");
    }
    Ok(used)
}

#[allow(clippy::too_many_arguments)]
pub fn pack_mesh(
    ctx: &PackCtx<'_>,
    writer: &mut ssb_rom::pack::PackWriter,
    tex_index: &mut BTreeMap<TexKey, u32>,
    mat_anim_index: &mut BTreeMap<ssb_rom::mesh::MatAnimRef, u32>,
    mat_anim_data: &BTreeMap<ssb_rom::mesh::MatAnimRef, MatAnimData>,
    src: Texels<'_>,
    id: u32,
    offset: u32,
    source: &ssb_rom::mesh::Mesh,
    swizzle: bool,
) -> u32 {
    // RE-318: split tiles whose bake would exceed the GE's 512-texel limit
    // into equivalent repeat/window tiles before anything is converted.
    // `source_prim` keeps residual fixes and report sites keyed by the
    // display list's own primitive order.
    let lowered = ssb_rom::wide_tile::lower(source, |i| {
        source.primitives[i]
            .material
            .mat_anim
            .is_none_or(|a| mat_anim_data.get(&a).is_none_or(|d| d.uv_static))
    })
    .unwrap_or_else(|e| panic!("RE-318 wide tile in file {id} list 0x{offset:X}: {e:?}"));
    if lowered.stats.primitives > 0 {
        let mut s = ctx.wide_tiles.get();
        s.0 += lowered.stats.primitives;
        s.1 += lowered.stats.split_triangles;
        s.2 += lowered.stats.emitted;
        ctx.wide_tiles.set(s);
    }
    let mut lowered = lowered;
    if ctx.opts.hide_wide_tiles {
        for (k, prim) in lowered.mesh.primitives.iter_mut().enumerate() {
            // A lowered piece always differs from its source in texture.
            if prim.material.texture != source.primitives[lowered.source_prim[k]].material.texture {
                prim.indices.clear();
            }
        }
    }
    let m = &lowered.mesh;
    let mut per_prim: Vec<Option<u32>> = Vec::with_capacity(m.primitives.len());
    let mut per_prim_mat_anim: Vec<Option<u32>> = Vec::with_capacity(m.primitives.len());
    let mut per_prim_phase = Vec::with_capacity(m.primitives.len());
    let mut per_prim_lod = Vec::with_capacity(m.primitives.len());
    let mut per_prim_premul: Vec<bool> = Vec::with_capacity(m.primitives.len());
    let mut texture_parts: Vec<(u32, u32, [u32; ssb_rom::pack::TexturePartDesc::MAX_IDS])> =
        Vec::new();
    for (packed_index, prim) in m.primitives.iter().enumerate() {
        let prim_index = lowered.source_prim[packed_index];
        let fix = prim.material.texture.as_ref().and_then(|t| {
            residual_fixes::find(&ctx.opts.residual_candidates, id, offset, prim_index, t)
        });
        let phase = match fix.map(|f| f.kind) {
            Some(residual_fixes::Kind::Phase(p)) => p,
            _ => [0, 0],
        };
        per_prim_phase.push(phase);
        let texgen_site = prim
            .material
            .texgen_scale
            .map(|_| prim.material.texture_gen == ssb_rom::mesh::TextureGen::Linear);
        let texture_index = match prim.material.texture {
            None => None,
            // RE-099/RE-100: no ROM bytes to convert -- the device fills
            // this in at run time (`Gpu::request_transition_capture`). Keyed
            // on width/height alone, under a `(u32::MAX, u32::MAX)` file
            // prefix no real resolved file id can ever equal: unlike a real
            // texture, a framebuffer capture has no baked crop/mirror bytes
            // to disambiguate (there is no pixel data to bake, RE-253), so
            // every other field is pinned to a fixed sentinel value rather
            // than the real primitive's own -- otherwise this would fragment
            // one runtime capture into needless duplicate pack entries
            // across primitives that legitimately share it at different tile
            // origins.
            Some(t) if t.framebuffer => {
                let key = TexKey {
                    data_file: u32::MAX,
                    data_offset: 0,
                    palette_file: u32::MAX,
                    palette_offset: None,
                    format: ssb_rom::texture::Format::Rgba,
                    size: ssb_rom::texture::BitSize::Bits4,
                    width: t.width,
                    height: t.height,
                    source_width: t.width,
                    drawn_width: 0,
                    drawn_height: 0,
                    palette_entries: 0,
                    palette: 0,
                    tlut: ssb_rom::texture::TextureLut::None,
                    mirror_s: false,
                    mirror_t: false,
                    clamp_s: false,
                    clamp_t: false,
                    rgb_unused: false,
                    rgb_lerp: None,
                    compensation_version: 0,
                    coverage_hash: 0,
                    animated_relationship: false,
                    animated_source: None,
                    alpha_policy: ssb_rom::filter_compensation::AlphaPolicy::Opaque,
                    residual_variant: 0,
                };
                Some(
                    *tex_index
                        .entry(key)
                        .or_insert_with(|| writer.add_framebuffer_texture(t.width, t.height)),
                )
            }
            Some(t) => {
                let original_coverage = coverage_if_compensated(
                    ctx,
                    fix.is_some() || listed(ctx, src.home.id, t.data_file, t.data_offset),
                    m,
                    prim,
                );
                let coverage = if matches!(fix.map(|f| f.kind), Some(residual_fixes::Kind::Dense)) {
                    residual_fixes::dense_coverage(&original_coverage)
                } else {
                    original_coverage
                };
                let animated = prim.material.mat_anim;
                count_rgb_lerp_swaps(ctx, &t, animated.and_then(|key| mat_anim_data.get(&key)));
                let (gate, translucent_blend) = ssb_rom::pack::material_alpha_state(&prim.material);
                let alpha_policy =
                    ssb_rom::filter_compensation::AlphaPolicy::classify(gate, translucent_blend);
                let mut key = texture_cache_key(
                    id,
                    &t,
                    coverage_hash(&coverage.train)
                        ^ coverage_hash(&coverage.validation).rotate_left(1)
                        ^ coverage
                            .texgen
                            .as_ref()
                            .filter(|meta| meta.per_primitive)
                            .map_or(0, |meta| coverage_hash(&meta.holdout).rotate_left(2)),
                    animated,
                    alpha_policy,
                );
                let direct_fit = match fix.map(|f| f.kind) {
                    Some(residual_fixes::Kind::Direct(mode)) => Some(mode),
                    _ => None,
                };
                if matches!(
                    fix.map(|f| f.kind),
                    Some(residual_fixes::Kind::Dense | residual_fixes::Kind::Direct(_))
                ) {
                    key.residual_variant = fix.unwrap().variant;
                }
                // The key an uncompensated result is cached under. A texture
                // that cannot be compensated (RE-588) looks it up first,
                // instead of converting and then finding it packed.
                let plain_key = || {
                    let mut key = texture_cache_key(
                        id,
                        &t,
                        0,
                        animated,
                        ssb_rom::filter_compensation::AlphaPolicy::Opaque,
                    );
                    key.animated_source = None;
                    key.residual_variant = 0;
                    key
                };
                let plain = fix.is_none()
                    && !ctx.observer.enabled()
                    && !listed(ctx, src.home.id, t.data_file, t.data_offset);
                let resolved = if let Some(&i) = tex_index.get(&key) {
                    Some(i)
                } else if let Some(&i) = tex_index.get(&plain_key()).filter(|_| plain) {
                    Some(i)
                } else {
                    ctx.observer.discard_conversion();
                    let mut compensated = false;
                    let animated_palettes = animated.and_then(|key| {
                        mat_anim_data
                            .get(&key)
                            .and_then(|data| reachable_palettes(src, key, data, None))
                    });
                    convert_texture(
                        ctx,
                        src,
                        &t,
                        &coverage,
                        swizzle,
                        animated_palettes.as_deref(),
                        alpha_policy,
                        Some(&mut compensated),
                        direct_fit,
                    )
                    .map(|tex| {
                        let final_key = if compensated { key } else { plain_key() };
                        if let Some(&i) = tex_index.get(&final_key) {
                            ctx.observer.discard_conversion();
                            return i;
                        }
                        let i = writer.add_texture(&tex, t.clamp_s, t.clamp_t);
                        record_texture_source(writer, i, src, &t, compensated);
                        ctx.observer.record_variant(i, &tex, t.clamp_s, t.clamp_t);
                        tex_index.insert(final_key, i);
                        i
                    })
                };
                // Report-only (`romtool residuals`): every primitive that
                // binds a variant, with its real filter coverage.
                if let (Some(i), true) = (resolved, ctx.observer.enabled()) {
                    ctx.observer.record_site(crate::UseSite {
                        variant: i,
                        file: id,
                        dl: offset,
                        prim: prim_index,
                        sprite_slot: None,
                        policy: alpha_policy,
                        texgen: texgen_site,
                        mat_anim: animated.is_some(),
                        triangles: prim.triangle_count(),
                        coverage,
                        phase,
                    });
                }
                resolved
            }
        };
        // Unlike `texture_index`, not gated on the primitive having a bound
        // texture at all: an effect script can drive untextured primitive/
        // environment/blend colour with no palette or sprite involved
        // (RE-175, `mesh.rs`'s `apply_mobj`/`material_now`).
        let mat_anim_index_resolved = prim.material.mat_anim.and_then(|anim| {
            let key = anim;
            match mat_anim_index.get(&key) {
                Some(&i) => Some(i),
                None => mat_anim_data.get(&key).and_then(|anim_data| {
                    // The build packs the file's script window later
                    // (`PackWriter::place_mat_anim_file`): decline here when
                    // its bytes are missing.
                    src.bytes(if anim.source_file == id {
                        None
                    } else {
                        Some(anim.source_file as u16)
                    })?;
                    let palettes: Vec<Vec<u32>> = anim_data
                        .palettes
                        .iter()
                        .filter_map(|p| {
                            convert_mat_anim_palette(src, *p, anim_data.palette_entries)
                        })
                        .collect();
                    // A partial conversion is a real problem worth declining
                    // outright, not shipping a script that cycles through
                    // fewer palettes than it actually names.
                    if palettes.len() != anim_data.palettes.len() {
                        return None;
                    }
                    // A texture-id track needs the primitive's own bound
                    // texture to know the sprite's format/dimensions/wrap
                    // (`convert_mat_anim_sprite`) -- an effect script whose
                    // primitive resolved no texture at all cannot be
                    // converted this way, same "decline rather than guess"
                    // shape as the palette check above. `texture_shape`
                    // (RE-177) covers the real gap this left: several
                    // manager-effect sprite primitives issue `G_SETTILE`/
                    // `G_SETTILESIZE` for their sprite's shape but never a
                    // static `G_SETTIMG` for it at all, because real hardware
                    // only supplies that address at runtime through a
                    // graphics-heap `Call` this converter cannot follow --
                    // the shape is real and known even though `texture`
                    // itself correctly stays `None`.
                    let sprites: Vec<u32> = if anim_data.sprites.is_empty() {
                        Vec::new()
                    } else {
                        // RE-326: only a primitive drawing the `MObj`'s own
                        // image has the tile shape its sprites load into. A
                        // later one that loaded its own image leaves the
                        // entry to that primitive.
                        if !prim.material.anim_texture.image {
                            ctx.non_owner_first.set(ctx.non_owner_first.get() + 1);
                            return None;
                        }
                        let base = prim.material.texture.or(prim.material.texture_shape)?;
                        let (gate, translucent_blend) =
                            ssb_rom::pack::material_alpha_state(&prim.material);
                        let alpha_policy = ssb_rom::filter_compensation::AlphaPolicy::classify(
                            gate,
                            translucent_blend,
                        );
                        let coverage = coverage_if_compensated(ctx,
                            any_compensated(ctx, src.home.id, &anim_data.sprites),
                            m,
                            prim,
                        );
                        let converted: Vec<u32> = anim_data
                            .sprites
                            .iter()
                            .enumerate()
                            .filter_map(|(slot, p)| {
                                // RE-364: tile 0 never selects a slot past
                                // the replay's highest `TextureIDCurrent`,
                                // and tile 1 owns its own images
                                // (`lod_blend_desc`), so a two-tile blend's
                                // later slots are not converted at all.
                                if anim_data.drives_lod
                                    && anim_data.current_closed
                                    && slot > anim_data.max_current
                                {
                                    pack_diag!(ctx,
                                        "mat-anim file {} script 0x{:X} MObjSub 0x{:X}: tile-0 slot {slot} past TextureIDCurrent {} not packed",
                                        anim.source_file,
                                        anim.script,
                                        anim_data.source_offset,
                                        anim_data.max_current
                                    );
                                    return Some(ssb_rom::pack::TextureDesc::NO_ANIM);
                                }
                                let states = reachable_palettes(src, anim, anim_data, Some(slot));
                                ctx.observer.discard_conversion();
                                convert_mat_anim_sprite(ctx,
                                    src,
                                    *p,
                                    &base,
                                    &coverage,
                                    states.as_deref(),
                                    swizzle,
                                    alpha_policy,
                                )
                                .map(
                                    |(tex, variant, compensated)| {
                                        let i =
                                            writer.add_texture(&tex, base.clamp_s, base.clamp_t);
                                        record_texture_source(
                                            writer,
                                            i,
                                            src,
                                            &variant,
                                            compensated,
                                        );
                                        ctx.observer.record_variant(
                                            i,
                                            &tex,
                                            base.clamp_s,
                                            base.clamp_t,
                                        );
                                        if ctx.observer.enabled() {
                                            ctx.observer.record_site(crate::UseSite {
                                                variant: i,
                                                file: id,
                                                dl: offset,
                                                prim: prim_index,
                                                sprite_slot: Some(slot),
                                                policy: alpha_policy,
                                                texgen: texgen_site,
                                                mat_anim: true,
                                                triangles: prim.triangle_count(),
                                                coverage: coverage.clone(),
                                                phase,
                                            });
                                        }
                                        i
                                    },
                                )
                            })
                            .collect();
                        if converted.len() != anim_data.sprites.len() {
                            return None;
                        }
                        converted
                    };
                    let i = writer.add_mat_anim(
                        anim.source_file,
                        anim.script,
                        anim_data.source_offset,
                        &palettes,
                        &sprites,
                        anim_data.base_tracks,
                        anim_data.uv_mode,
                        anim_data.uv_tile_params,
                    );
                    writer.set_mat_anim_uv_half(i, anim_data.uv_half);
                    for &script in &anim_data.extra_scripts {
                        writer.add_mat_anim_root(anim.source_file, script);
                    }
                    mat_anim_index.insert(key, i);
                    Some(i)
                }),
            }
        });
        if let (Some(texture), Some(mat_anim), true) = (
            texture_index,
            mat_anim_index_resolved,
            prim.material.anim_texture.palette,
        ) {
            // Kept alongside `PrimDesc.mat_anim` for the texture-only
            // palette-cycling case (RE-089/RE-090/RE-091): `bind_texture`
            // (`psp-asset-viewer/src/meshdraw.rs`) still reads a texture's own resolved
            // palette off `TextureDesc.mat_anim`, independent of which
            // primitive happens to be drawing it.
            writer.set_texture_mat_anim(texture, mat_anim);
        }
        // RE-321: a classified two-tile blend gets its second pass only
        // once the lowering is proven exact for it; otherwise it keeps the
        // single-texture path.
        // A texel-alpha blend keeps its vertex colours: its passes take
        // their alpha from `PrimDesc` effect colours, not premultiplied shade.
        let mut texel_alpha_lod = false;
        let lod = prim.material.lod_blend.map(|lod| {
            let outcome = if prim.material.texture != source.primitives[prim_index].material.texture
            {
                Err("split by the wide-tile lowering")
            } else if texture_index.is_none() {
                Err("TEXEL0 did not convert")
            } else {
                match (prim.material.mat_anim, mat_anim_index_resolved) {
                    (Some(key), Some(anim)) => match mat_anim_data.get(&key) {
                        Some(data) => lod_blend_desc(
                            ctx,
                            writer,
                            src,
                            id,
                            offset,
                            prim_index,
                            packed_index,
                            m,
                            prim,
                            &lod,
                            anim,
                            data,
                            swizzle,
                            phase,
                        )
                        .and_then(|desc| {
                            texel_alpha_lod = desc.texel_alpha.is_some();
                            if writer.add_lod_blend(desc) {
                                Ok(desc.next_count as usize)
                            } else {
                                Err("one script drives two different tile-1 shapes")
                            }
                        }),
                        None => Err("material animation did not resolve"),
                    },
                    _ => Err("material animation did not pack"),
                }
            };
            ctx.lod_blend_sites.borrow_mut().push(crate::LodBlendSite {
                file: id,
                dl: offset,
                prim: prim_index,
                triangles: prim.triangle_count(),
                outcome,
            });
            outcome.is_ok()
        });
        // RE-426: a texture part's `MObj` sprite, each script-set id
        // converted through this primitive's own tile and palette.
        if let (Some(tag), Some(t), Some(own)) = (
            prim.material.texture_part,
            prim.material.texture,
            texture_index,
        ) {
            if prim.material.texture != source.primitives[prim_index].material.texture {
                ctx.texture_part(|s| s.lowered += 1);
            } else {
                let textures = texture_part_textures(
                    ctx, writer, tex_index, src, id, m, prim, &t, own, tag, swizzle,
                );
                texture_parts.push((packed_index as u32, u32::from(tag.part), textures));
            }
        }
        per_prim.push(texture_index);
        per_prim_mat_anim.push(mat_anim_index_resolved);
        per_prim_lod.push(lod == Some(true));
        per_prim_premul.push(lod == Some(true) && !texel_alpha_lod);
    }
    // RE-321: a translucent two-tile blend draws `S * a` from its vertices
    // (`lod_blend_desc` proved them its own and of one RGB).
    let premultiplied;
    let m = if per_prim_premul
        .iter()
        .zip(&m.primitives)
        .any(|(&lod, p)| lod && ssb_rom::pack::material_alpha_state(&p.material).1)
    {
        let mut copy = m.clone();
        for (k, p) in m.primitives.iter().enumerate() {
            if !per_prim_premul[k] || !ssb_rom::pack::material_alpha_state(&p.material).1 {
                continue;
            }
            let used = lod_premultiply_vertices(m, k).expect("checked by lod_blend_desc");
            for i in used {
                let v = &mut copy.vertices[usize::from(i)];
                v.rgba = ssb_rom::lod_blend::premultiply_shade(v.rgba);
            }
        }
        premultiplied = copy;
        &premultiplied
    } else {
        m
    };
    let mesh_index = writer.add_mesh(m, id, offset, |i| per_prim[i], |i| per_prim_mat_anim[i]);
    for (prim, part, textures) in texture_parts {
        writer.add_texture_part(mesh_index, prim, part, textures);
        ctx.texture_part(|s| s.prims += 1);
    }
    for (i, &phase) in per_prim_phase.iter().enumerate() {
        if phase != [0, 0] {
            writer.set_mesh_prim_phase(mesh_index, i, phase);
        }
    }
    for (i, &lod) in per_prim_lod.iter().enumerate() {
        if lod {
            writer.set_mesh_prim_lod_blend(mesh_index, i);
        }
    }
    mesh_index
}

pub fn count_rgb_lerp_swaps(
    ctx: &PackCtx<'_>,
    t: &ssb_rom::mesh::TextureRef,
    anim: Option<&MatAnimData>,
) {
    let mut s = ctx.rgb_lerp.get();
    {
        let palettes = anim.is_some_and(|a| !a.palettes.is_empty());
        if t.rgb_lerp.is_some() {
            s.lerped += 1;
            s.lerped_sprite_anim += usize::from(anim.is_some_and(|a| !a.sprites.is_empty()));
            s.lerped_palette_anim += usize::from(palettes);
        }
        s.unused_palette_anim += usize::from(t.rgb_unused && palettes);
    }
    ctx.rgb_lerp.set(s);
}

/// `sprites[id]` of a texture part's `MObj` for every id the scripts set,
/// converted with `t`'s tile and palette (`gcDrawMObjForDObj` swaps only
/// the image address); id 0 is the primitive's own texture `own`.
#[allow(clippy::too_many_arguments)]
pub fn texture_part_textures(
    ctx: &PackCtx<'_>,
    writer: &mut ssb_rom::pack::PackWriter,
    tex_index: &mut BTreeMap<TexKey, u32>,
    src: Texels<'_>,
    id: u32,
    m: &ssb_rom::mesh::Mesh,
    prim: &ssb_rom::mesh::Primitive,
    t: &ssb_rom::mesh::TextureRef,
    own: u32,
    tag: ssb_rom::mobj::TexturePartTag,
    swizzle: bool,
) -> [u32; ssb_rom::pack::TexturePartDesc::MAX_IDS] {
    let mut out = [ssb_rom::pack::TextureDesc::NO_ANIM; ssb_rom::pack::TexturePartDesc::MAX_IDS];
    out[0] = own;
    let (gate, translucent_blend) = ssb_rom::pack::material_alpha_state(&prim.material);
    let alpha_policy = ssb_rom::filter_compensation::AlphaPolicy::classify(gate, translucent_blend);
    let parts: Vec<ssb_rom::mobj::Ptr> = (1..out.len())
        .filter(|k| tag.ids >> k & 1 != 0)
        .filter_map(|k| ssb_rom::mobj::read_sprite_at(src.home, tag.at, k))
        .collect();
    let coverage = coverage_if_compensated(ctx, any_compensated(ctx, src.home.id, &parts), m, prim);
    for (k, slot) in out.iter_mut().enumerate().skip(1) {
        if tag.ids >> k & 1 == 0 {
            continue;
        }
        let Some(p) = ssb_rom::mobj::read_sprite_at(src.home, tag.at, k) else {
            ctx.texture_part(|s| s.unresolved += 1);
            continue;
        };
        let variant = ssb_rom::mesh::TextureRef {
            data_file: p.file,
            data_offset: p.offset,
            ..*t
        };
        let key = texture_cache_key(id, &variant, 0, None, alpha_policy);
        if let Some(&i) = tex_index.get(&key) {
            *slot = i;
            continue;
        }
        ctx.observer.discard_conversion();
        let mut compensated = false;
        let Some(tex) = convert_texture(
            ctx,
            src,
            &variant,
            &coverage,
            swizzle,
            None,
            alpha_policy,
            Some(&mut compensated),
            None,
        ) else {
            ctx.texture_part(|s| s.unresolved += 1);
            continue;
        };
        let i = writer.add_texture(&tex, t.clamp_s, t.clamp_t);
        record_texture_source(writer, i, src, &variant, compensated);
        tex_index.insert(key, i);
        ctx.texture_part(|s| s.sprites += 1);
        *slot = i;
    }
    out
}

/// Converts one resolved palette variant to the GE's ABGR8888 CLUT format,
/// the same conversion [`convert_texture`] already applies to the static
/// (index-0) palette every texture bakes at pack time.
pub fn convert_mat_anim_palette(
    src: Texels<'_>,
    p: ssb_rom::mobj::Ptr,
    entries: u16,
) -> Option<Vec<u32>> {
    use ssb_rom::psp_texture as psp;
    use ssb_rom::texture;

    let n = entries.max(1) as usize;
    let file = src.bytes(p.file)?;
    let bytes = file.get(p.offset as usize..p.offset as usize + n * 2)?;
    let tlut = texture::parse_tlut(bytes);
    Some(
        tlut.iter()
            .map(|&e| psp::pack_abgr(texture::rgba5551(e)))
            .collect(),
    )
}

/// Replay the real script to retain only PaletteID slots that can coincide
/// with this texture selection. `sprite_slot == None` means the primitive's
/// static texture before a TextureIDCurrent track takes over.
pub fn reachable_palettes(
    src: Texels<'_>,
    key: ssb_rom::mesh::MatAnimRef,
    data: &MatAnimData,
    sprite_slot: Option<usize>,
) -> Option<Vec<Vec<u32>>> {
    if data.palettes.is_empty() {
        return None;
    }
    let file = src.bytes(if key.source_file == src.home.id {
        None
    } else {
        Some(key.source_file as u16)
    })?;
    let mut reached = alloc::collections::BTreeSet::new();
    for &script in core::iter::once(&key.script).chain(&data.extra_scripts) {
        let mut joint = ssb_rom::matanim::MaterialJoint::start(script, 0.0);
        for _ in 0..MAT_ANIM_REPLAY_FRAMES {
            joint.tick(&file, 1.0).ok()?;
            let texture = joint
                .track_value(ssb_rom::matanim::TRACK_TEXTURE_ID_CURRENT)
                .map(|v| (v.max(0.0) as usize).min(data.sprites.len().saturating_sub(1)));
            // `palettes[(s32)palette_id]` for any live kind (RE-325/RE-326).
            if texture == sprite_slot {
                if let Some(value) = joint.track_value(ssb_rom::matanim::TRACK_PALETTE_ID) {
                    reached.insert((value.max(0.0) as usize).min(data.palettes.len() - 1));
                }
            }
            if joint.ended() || joint.looped() {
                break;
            }
        }
    }
    reached
        .into_iter()
        .map(|id| convert_mat_anim_palette(src, data.palettes[id], data.palette_entries))
        .collect()
}

/// Converts one resolved sprite (texture-id) variant, reusing a primitive's
/// own authored format/dimensions/wrap (`base`) -- a material animation's
/// `TextureIDCurrent`/`TextureIDNext` track only ever swaps *which* texel
/// block loads, never the tile's shape, the same convention
/// [`convert_mat_anim_palette`] already follows for a palette variant's
/// entry count.
#[allow(clippy::too_many_arguments)]
pub fn convert_mat_anim_sprite(
    ctx: &PackCtx<'_>,
    src: Texels<'_>,
    p: ssb_rom::mobj::Ptr,
    base: &ssb_rom::mesh::TextureRef,
    coverage: &filter_coverage::Coverage,
    palettes: Option<&[Vec<u32>]>,
    swizzle: bool,
    alpha_policy: ssb_rom::filter_compensation::AlphaPolicy,
) -> Option<(
    ssb_rom::psp_texture::PspTexture,
    ssb_rom::mesh::TextureRef,
    bool,
)> {
    let variant = ssb_rom::mesh::TextureRef {
        data_file: p.file,
        data_offset: p.offset,
        ..*base
    };
    let mut compensated = false;
    let tex = convert_texture(
        ctx,
        src,
        &variant,
        coverage,
        swizzle,
        palettes,
        alpha_policy,
        Some(&mut compensated),
        None,
    )?;
    Some((tex, variant, compensated))
}

/// Records the render tile texture `i` was converted through (RE-327) and
/// the source tile bytes it came from, with whether its level 0 is their
/// filter compensation (RE-336). The bytes are the tile's period rows of
/// `source_width` texels, as `romtool matcolors` reads a sprite.
pub fn record_texture_source(
    writer: &mut ssb_rom::pack::PackWriter,
    i: u32,
    src: Texels<'_>,
    t: &ssb_rom::mesh::TextureRef,
    compensated: bool,
) {
    writer.set_texture_tile(i, t);
    let row = ssb_rom::texture::data_len(u32::from(t.source_width.max(t.width)), 1, t.size);
    let at = t.data_offset as usize;
    if let Some(file) = src.bytes(t.data_file) {
        if let Some(bytes) = file.get(at..at + row * usize::from(t.height)) {
            writer.set_texture_source(i, bytes, compensated);
        }
    }
}

/// Read the source CI field before colour decoding so equal RGB entries keep
/// their distinct PaletteID meanings. Mirror/clamp expansion follows the
/// same path as `decode_mirrored` in `convert_texture`.
pub fn decode_index_field(file: &[u8], t: &ssb_rom::mesh::TextureRef) -> Option<Vec<u8>> {
    use ssb_rom::texture::{self, BitSize};
    let source_width = u32::from(t.source_width.max(t.width));
    let width = u32::from(t.width);
    let height = u32::from(t.height);
    let row = texture::data_len(source_width, 1, t.size);
    let bytes = file.get(t.data_offset as usize..t.data_offset as usize + row * height as usize)?;
    let mut img = texture::Rgba8::new(width, height);
    for y in 0..height as usize {
        for x in 0..width as usize {
            let at = y * row
                + match t.size {
                    BitSize::Bits4 => x / 2,
                    _ => x,
                };
            let index = match t.size {
                BitSize::Bits4 => {
                    if x & 1 == 0 {
                        bytes[at] >> 4
                    } else {
                        bytes[at] & 0xf
                    }
                }
                _ => bytes[at],
            };
            img.put(y * width as usize + x, [index, 0, 0, 255]);
        }
    }
    let mirrored = texture::mirror_extend(
        &img,
        t.mirror_s,
        t.mirror_t,
        t.clamp_s,
        t.clamp_t,
        t.drawn_width as u32,
        t.drawn_height as u32,
    );
    Some(
        (0..(mirrored.width * mirrored.height) as usize)
            .map(|i| mirrored.get(i)[0])
            .collect(),
    )
}
