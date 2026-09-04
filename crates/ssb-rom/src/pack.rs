//! The runtime asset pack: a zero-copy container for converted geometry and
//! textures.
//!
//! ## Design goal: no parsing on the PSP
//!
//! The PSP has a 333 MHz CPU and no memory to spare for a second copy of the
//! asset set. So the pack is laid out exactly as the hardware wants it:
//!
//! * Vertex, index, texel and palette blocks are **16-byte aligned**, because
//!   the GE DMAs them directly and unaligned data silently renders garbage.
//! * Everything is **little-endian**, matching the PSP, so loading is a
//!   `read()` into an aligned buffer and nothing else. No byte swapping, no
//!   struct decoding, no allocation per asset.
//! * Textures are stored **already swizzled and already in their PSM**, so
//!   `sceGuTexImage` can point straight at the mapped bytes.
//!
//! The reader therefore hands out `&[u8]` slices into the loaded buffer, and
//! the renderer passes those pointers to the GE unchanged.
//!
//! ## Layout
//!
//! ```text
//! Header (64 bytes, 16-byte aligned)
//! MeshDesc[mesh_count]
//! PrimDesc[prim_count]
//! TextureDesc[texture_count]
//! ObjectDesc[object_count]
//! NodeDesc[node_count]
//! StageDesc[stage_count]
//! LineDesc[line_count]
//! CollisionVertex[coll_vertex_count]
//! MapPoint[point_count]
//! FighterDesc[fighter_count]
//! AnimDesc[anim_count]
//! AnimJoint[anim_joint_count]
//! MatAnimDesc[mat_anim_count]
//! MatAnimPalette[mat_anim_palette_count]
//! CostumeOverride[costume_override_count]
//! ---- 16-byte aligned blob region ----
//! vertex data | index data | texel data | palette data | animation scripts
//! ```
//!
//! The four stage tables are descriptors, not blobs: the CPU walks them, the
//! GE never sees them, so they sit with the other tables rather than in the
//! DMA region.
//!
//! Descriptors carry offsets into the blob region, so the whole file can be
//! relocated freely: nothing stores an absolute address.

use alloc::vec::Vec;

/// Magic at the start of every pack: `SSBP`.
pub const MAGIC: u32 = 0x5342_5350;

/// Bumped whenever the layout changes incompatibly.
///
/// 2 added the object and node tables.
/// 3 added the stage tables: collision lines, their vertices, and map points.
/// 4 added the fighter table.
/// 5 added the figatree animation lengths to the fighter table.
/// 6 added the animation tables, and each node's local rest transform.
/// 9 added `ALPHA_TEST`/`TRANSLUCENT` to `PrimDesc::flags` (RE-069).
/// 10 added `TEXTURE_BLEND` and its base/target colours to `PrimDesc`
///    (RE-073).
/// 11 added `FLAT_COLOR` and its colour to `PrimDesc` (RE-079).
/// 12 added the `MatAnimDesc`/`MatAnimPalette` tables and `TextureDesc::
///    mat_anim` (RE-091), so a `PaletteID`-cycling material animation's
///    resolved palette variants (RE-089/RE-090) travel with the pack. Filled
///    `TextureDesc`'s existing 4 bytes of tail padding, so `TextureDesc::SIZE`
///    itself is unchanged.
/// 13 added the `CostumeOverride` table and `Header::costume_override_count`
///    (RE-098): a fighter's alternate costumes share one baked object/node
///    table and only the (node, costume) pairs whose colour or palette
///    genuinely differs from costume 0 (RE-098 measured this at roughly a
///    third to two-thirds of a fighter's nodes, never all of them) get their
///    own substitute mesh, looked up by the node's *global* index so no
///    per-object bookkeeping is needed at draw time.
/// 14 added `TextureDesc::role` (RE-099/RE-100): the LB "loading transition"
///    system's `G_SETTIMG` names segment `0x1`
///    (`mobj::LB_TRANSITION_SEGMENT`), a one-time framebuffer snapshot the
///    device fills in at run time, not any archive location. Such a texture
///    has no baked bytes at all (`data_len`/`palette_len` are 0), so a reader
///    needs a way to tell it apart from an ordinary texture that simply
///    failed to convert -- `role` is that marker. Grows `TextureDesc::SIZE`
///    32 -> 36, the same shape `mat_anim` (`VERSION` 12) would have needed
///    had it not found spare tail padding to fill instead.
/// 15 added `TextureDesc::wrap` (RE-102): `G_SETTILE`'s `cms`/`cmt` clamp bit
///    per axis, threaded through so the device can call `sceGuTexWrap` with
///    `Clamp` instead of always `Repeat`. A primitive whose drawn UV rect
///    legitimately exceeds a texture's own tile -- true of the small
///    decal-style textures fighter faces are built from -- was wrapping and
///    repeating texels real hardware would have clamped at the edge, the
///    remaining cause (after RE-101's `G_TEXTURE` UV-scale fix) of fighter
///    faces reading as a jumbled, "melted" texture. Grows `TextureDesc::SIZE`
///    36 -> 40.
pub const VERSION: u32 = 15;

/// Alignment for every blob the GE reads.
pub const ALIGN: usize = 16;

/// Bytes per packed vertex. Must equal `size_of::<PackedVertex>()`.
///
/// 16, not 14: the `u32` colour forces 4-byte alignment, so `repr(C)` inserts
/// tail padding. The GE also requires the vertex stride to be a multiple of the
/// largest component size, so 16 is what the hardware wants anyway.
pub const VERTEX_SIZE: usize = 16;

/// Header. 64 bytes through `VERSION` 11; `mat_anim_count`/
/// `mat_anim_palette_count` (`VERSION` 12) extend it to 72, and
/// `costume_override_count` (`VERSION` 13) to 76 — the original 64 was a
/// coincidence of having exactly 16 `u32` fields, not a hard alignment
/// requirement (only the blob region, computed separately via `blob_offset`,
/// needs 16-byte alignment for the GE's DMA).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Header {
    pub magic: u32,
    pub version: u32,
    pub mesh_count: u32,
    pub prim_count: u32,
    pub texture_count: u32,
    /// Byte offset of the blob region.
    pub blob_offset: u32,
    pub blob_len: u32,
    pub object_count: u32,
    pub node_count: u32,
    pub stage_count: u32,
    /// Collision polylines, summed over every stage.
    pub line_count: u32,
    pub coll_vertex_count: u32,
    pub point_count: u32,
    /// Characters in the fighter table. 27 when built from a full ROM.
    pub fighter_count: u32,
    /// Animations: one per `(fighter, movement status)` pair.
    pub anim_count: u32,
    /// Joint entries, summed over every animation.
    pub anim_joint_count: u32,
    /// Animated palette tables, one per `PaletteID`-cycling material
    /// animation script found (RE-089/RE-090).
    pub mat_anim_count: u32,
    /// Palette variants, summed over every `MatAnimDesc`.
    pub mat_anim_palette_count: u32,
    /// Per-(node, costume) mesh substitutions (RE-098).
    pub costume_override_count: u32,
}

impl Header {
    pub const SIZE: usize = 76;
}

/// A vertex in the GE's expected layout.
///
/// Field order is dictated by hardware: the GE reads texture coordinates,
/// then colour, then position, and the `VertexType` flags must describe
/// exactly that order. Reordering renders garbage with no error.
///
/// 16 bytes, against 24 with float position and UV — a third less vertex
/// bandwidth. N64 data is already `i16` positions and S10.5 UVs, so the
/// narrowing is lossless.
///
/// Colour stays 8888 rather than dropping to 5551 (which would fit in 12
/// bytes): when a material is lit these bytes carry a packed *normal*, and
/// quantising a normal to 5 bits per axis bands the lighting visibly.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PackedVertex {
    pub u: i16,
    pub v: i16,
    /// Packed ABGR, matching what `sceGuColor` and vertex colours expect.
    pub color: u32,
    pub x: i16,
    pub y: i16,
    pub z: i16,
}

/// Material flags, kept as a bitfield so a primitive's state fits in one word.
pub mod flags {
    pub const CULL_BACK: u32 = 1 << 0;
    pub const CULL_FRONT: u32 = 1 << 1;
    pub const LIT: u32 = 1 << 2;
    pub const SMOOTH: u32 = 1 << 3;
    pub const Z_BUFFER: u32 = 1 << 4;
    /// RE-069: `G_SETRENDERMODE`'s `CVG_X_ALPHA | ALPHA_CVG_SEL` -- a cutout
    /// surface, approximated as a plain PSP alpha test.
    pub const ALPHA_TEST: u32 = 1 << 5;
    /// RE-069: `G_SETRENDERMODE`'s blend equation genuinely reads back the
    /// framebuffer weighted by `1 - alpha` -- real translucency.
    pub const TRANSLUCENT: u32 = 1 << 6;
    /// RE-073: a combiner blending from a base colour to a target colour
    /// driven by the texture, with no shade involved -- `PrimDesc`'s
    /// `texture_blend_base`/`texture_blend_target` carry the two colours.
    /// Not yet consumed on the device side; see `MeshMaterial::texture_blend`.
    pub const TEXTURE_BLEND: u32 = 1 << 7;
    /// RE-079: a combiner that reduces to a plain constant colour -- no
    /// shade, no texel. `PrimDesc::flat_color` carries it; already baked
    /// into the primitive's vertices (`push_vertex`) and `texture` is
    /// already forced to `PrimDesc::NO_TEXTURE`, so this flag exists for
    /// inspection, matching `TEXTURE_BLEND`'s precedent, not because the
    /// device needs it to render correctly.
    pub const FLAT_COLOR: u32 = 1 << 8;
}

/// One draw: a range of indices plus the state to draw them under.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PrimDesc {
    /// Index into the texture table, or `u32::MAX` for untextured.
    pub texture: u32,
    pub flags: u32,
    pub prim_color: u32,
    pub env_color: u32,
    /// Byte offset into the blob of the first index.
    pub index_offset: u32,
    pub index_count: u32,
    /// `flags::TEXTURE_BLEND`'s base colour (packed ABGR), zero otherwise
    /// (RE-073).
    pub texture_blend_base: u32,
    /// `flags::TEXTURE_BLEND`'s target colour (packed ABGR), zero otherwise
    /// (RE-073).
    pub texture_blend_target: u32,
    /// `flags::FLAT_COLOR`'s colour (packed ABGR), zero otherwise (RE-079).
    pub flat_color: u32,
}

impl PrimDesc {
    pub const SIZE: usize = 36;
    pub const NO_TEXTURE: u32 = u32::MAX;
}

/// A mesh: one shared vertex buffer and a run of primitives.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MeshDesc {
    /// Byte offset into the blob of the vertex array.
    pub vertex_offset: u32,
    pub vertex_count: u32,
    /// Index of the first primitive in the primitive table.
    pub first_prim: u32,
    pub prim_count: u32,
    /// Which archive file this came from, for debugging.
    pub source_file: u32,
    /// Byte offset of the source display list, for debugging.
    pub source_offset: u32,
}

impl MeshDesc {
    pub const SIZE: usize = 24;
}

/// A texture, stored ready for `sceGuTexImage`.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextureDesc {
    pub width: u16,
    pub height: u16,
    /// Row stride in texels; a power of two, may exceed `width`.
    pub stride: u16,
    /// `Psm` discriminant.
    pub psm: u8,
    /// Non-zero if the texel data is swizzled.
    pub swizzled: u8,
    pub data_offset: u32,
    pub data_len: u32,
    pub palette_offset: u32,
    /// CLUT entries; zero for non-paletted formats.
    pub palette_len: u32,
    /// Mip levels stored back to back in `data_offset`, level 0 first. Always
    /// at least 1; a pack written before mipmaps existed reads back 0, which
    /// the reader normalises to 1 (RE-053).
    pub levels: u32,
    /// Index into the `MatAnimDesc` table if this texture's palette is
    /// cycled by a material animation script, or [`TextureDesc::NO_ANIM`]
    /// (RE-089/RE-090/RE-091). `palette_offset`/`palette_len` above still
    /// name this texture's palette *at pack time* (costume 0's, matching
    /// every other baked colour) — a device-side player swaps the active
    /// CLUT for one of `MatAnimDesc`'s resolved variants instead of reading
    /// these two fields, but they stay meaningful as the frame-0 fallback.
    pub mat_anim: u32,
    /// [`TextureDesc::ROLE_NORMAL`] or [`TextureDesc::ROLE_FRAMEBUFFER`]
    /// (RE-099/RE-100, `VERSION` 14). A framebuffer-role entry has
    /// `data_len`/`palette_len` of 0 by construction — there is no ROM data
    /// to bake, only `width`/`height`/`psm` describing the small buffer the
    /// device must fill at run time before this texture is first drawn.
    pub role: u32,
    /// [`TextureDesc::CLAMP_S`] / [`TextureDesc::CLAMP_T`] (RE-102,
    /// `VERSION` 15): `G_SETTILE`'s `cms`/`cmt` clamp bit per axis, from
    /// [`crate::mesh::TextureRef::clamp_s`]/`clamp_t`. Unset for every axis
    /// that mirrors instead (mirroring is pre-baked into the pixel data at
    /// pack time, RE-067) or that never wraps at all -- both read identically
    /// to `Repeat` on the PSP GE, so leaving the bit off for them costs
    /// nothing and keeps this purely additive over `VERSION` 14 packs.
    pub wrap: u8,
}

impl TextureDesc {
    /// 40: `u16 * 3 + u8 * 2` is 8 bytes, plus seven `u32` is 28 (36 total),
    /// plus `wrap`'s 1 byte padded to a 4-byte multiple (there is no
    /// requirement each descriptor itself be 16-byte aligned, only the blob
    /// region as a whole -- `PrimDesc::SIZE` is 36 for a similar reason).
    /// `wrap` (`VERSION` 15) is the second field to grow this struct, after
    /// `role` (`VERSION` 14) did the same to 36; neither found spare tail
    /// padding to fill instead.
    ///
    /// This was declared as 20 and the writer emitted 24, so every descriptor
    /// after the first was read from the wrong offset and textures came out as
    /// coloured noise on device. The size guard test below exists because the
    /// original round-trip test only checked texture 0, where the offset is
    /// correct no matter what the stride says.
    pub const SIZE: usize = 40;
    pub const NO_ANIM: u32 = u32::MAX;
    /// Baked from real ROM texel data at pack time -- every texture before
    /// `VERSION` 14.
    pub const ROLE_NORMAL: u32 = 0;
    /// No baked bytes: filled in on the PSP device from a runtime framebuffer
    /// capture the first time it is needed (RE-099/RE-100). `data_offset`/
    /// `data_len`/`palette_offset`/`palette_len` are all 0 for this role.
    pub const ROLE_FRAMEBUFFER: u32 = 1;
    /// `wrap` bit for `G_TX_CLAMP` on the S axis (RE-102).
    pub const CLAMP_S: u8 = 1 << 0;
    /// `wrap` bit for `G_TX_CLAMP` on the T axis (RE-102).
    pub const CLAMP_T: u8 = 1 << 1;
}

/// An assembled object: a run of nodes forming one `DObjDesc` hierarchy.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ObjectDesc {
    /// Index of the first node in the node table.
    pub first_node: u32,
    pub node_count: u32,
    /// Which archive file this came from, for debugging.
    pub source_file: u32,
    /// Byte offset of the source `DObjDesc` array, for debugging.
    pub source_offset: u32,
}

impl ObjectDesc {
    pub const SIZE: usize = 16;
}

/// One node of an object: an optional mesh under a baked world transform.
///
/// The matrix is **baked at build time**, not composed on device. A static
/// stage then costs one `sceGumLoadMatrix` per node and no trigonometry at all,
/// which matters because the PSP has no fast `sin`/`cos` outside the VFPU.
/// Animation will need the local TRS back, but only for the nodes that actually
/// move, so paying 64 bytes a node for every joint now would be premature.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NodeDesc {
    /// Index into the mesh table, or [`NodeDesc::NO_MESH`]. Just over half the
    /// nodes in the archive are pure transforms with no geometry of their own.
    pub mesh: u32,
    /// Index into the node table, or [`NodeDesc::NO_PARENT`] for a root.
    /// Absolute, not object-relative, so it can be followed without the object.
    pub parent: u32,
    /// Column-major world matrix, laid out for `sceGumLoadMatrix`.
    ///
    /// The translation column is pre-divided by [`MODEL_SCALE`] so it lives in
    /// the same normalised space as the `i16` vertex positions the GE reads.
    ///
    /// This is the **rest pose**, baked. An animated object recomposes its own
    /// from [`NodeDesc::rest`] each tick; a static one uses this as it stands,
    /// which is every stage and every object in the viewer.
    pub world: [f32; 16],
    /// The node's own local transform, before its parent's is applied:
    /// translate, rotate (radians, ZYX), scale.
    ///
    /// `world` is derivable from these and the parent chain, and is stored
    /// anyway because the static path reads it every frame and should not pay
    /// to rebuild it. What it is *not* derivable from is `world` alone —
    /// decomposing a matrix back into a rotation and a scale is lossy, and an
    /// animation needs to overwrite individual tracks of exactly these numbers
    /// while leaving the others at rest (RE-036).
    pub rest_translate: [f32; 3],
    pub rest_rotate: [f32; 3],
    pub rest_scale: [f32; 3],
    /// `DObjDesc.id & 0xF000`, the matrix kind the original asks for.
    ///
    /// Only [`NodeDesc::FLAG_BILLBOARD`] is acted on. It lives in what used to
    /// be this struct's tail padding, so a pack written before it existed reads
    /// back as zero — no billboards, which is exactly the old behaviour.
    pub flags: u32,
}

impl NodeDesc {
    /// `4 + 4 + 64 + 36`, padded to a 16-byte stride.
    pub const SIZE: usize = 112;
    pub const NO_MESH: u32 = u32::MAX;
    pub const NO_PARENT: u32 = u32::MAX;
    /// The node is a screen-aligned sprite: `gcPrepDObjMatrix` kinds 45-48
    /// build its matrix from the projection basis rather than its own rotation,
    /// so it always faces the camera (RE-048).
    pub const FLAG_BILLBOARD: u32 = 1 << 0;
}

/// One fighter's animation for one movement status.
///
/// Everything the runtime needs to play it is either here or in the joint
/// entries this points at — deliberately, so driving an animation does not
/// require first resolving which object a fighter is, which mask says which of
/// its descriptors became joints, or which archive file any of it came from.
/// That resolution is build-time work (RE-036) and it is already done.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AnimDesc {
    /// `FTKind` ordinal.
    pub fighter: u32,
    /// Which movement status, matching `crate::anim`'s `SLOT_*`.
    pub slot: u32,
    /// Archive file the scripts came from. Stored so a wrong pose can be
    /// traced back to the bytes it was read from, as meshes store theirs.
    pub source_file: u32,
    /// Length in frames at playback speed 1.0, or 0 when the animation loops.
    pub frames: u32,
    /// First entry in the joint table, and how many.
    pub first_joint: u32,
    pub joint_count: u32,
    /// Byte offset into the blob of the animation file's bytes. Script offsets
    /// in the joint entries are relative to this.
    pub script_offset: u32,
    pub script_len: u32,
}

impl AnimDesc {
    /// `fighter` value marking a *stage* animation rather than a fighter's.
    /// Stage entries sit after the dense fighter block, and `slot` is the
    /// stage index (RE-051).
    pub const STAGE: u32 = u32::MAX;

    pub const SIZE: usize = 32;
}

/// One joint of one animation: its script, and the node it drives.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AnimJoint {
    /// Byte offset of this joint's script within the animation's bytes, or
    /// [`AnimJoint::NO_SCRIPT`]. Roughly a fifth of joints are not animated by
    /// any given animation and keep their rest pose.
    pub script: u32,
    /// Absolute node index this joint drives, or [`AnimJoint::NO_NODE`].
    ///
    /// Absolute rather than object-relative for the same reason `NodeDesc`'s
    /// parent is: it can be followed without first knowing the object.
    pub node: u32,
}

impl AnimJoint {
    pub const SIZE: usize = 8;
    pub const NO_SCRIPT: u32 = u32::MAX;
    pub const NO_NODE: u32 = u32::MAX;
}

/// A `PaletteID`-cycling material animation: the driving script, and where
/// its resolved palette variants (RE-089/RE-090) sit in [`MatAnimPalette`].
///
/// One entry per animated *texture*, not per primitive or per `MObjSub`:
/// `pack_mesh`'s existing texture cache already dedupes by texel location
/// (`(data_file, data_offset)`), so every primitive drawing this texture
/// already shares one [`TextureDesc`] — [`TextureDesc::mat_anim`] points
/// here, and a device-side player ticks one [`crate::matanim::MaterialJoint`]
/// per entry, not one per primitive.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MatAnimDesc {
    /// Byte offset into the blob of the driving script's *whole source file*
    /// bytes — deduplicated per archive file, the same convention
    /// `AnimDesc::script_offset` uses for joint animation. `script` below is
    /// an offset within this, not within the blob directly, matching how
    /// [`crate::matanim::MaterialJoint::tick`] is already built to run
    /// against a whole file's bytes rather than a pre-sliced script.
    pub file_offset: u32,
    pub file_len: u32,
    /// Byte offset of the driving script itself, within the file bytes named
    /// by `file_offset`/`file_len` above (RE-087/RE-089).
    pub script: u32,
    /// First entry in the [`MatAnimPalette`] table, and how many follow —
    /// the exact count [`crate::mobj::read_palettes`]'s bound produced, not
    /// a guess (RE-088 found no sound local bound exists; RE-089 supplies
    /// one from this same script).
    pub first_palette: u32,
    pub palette_count: u32,
    /// Archive file/offset of the driving `MObjSub`, for debugging.
    pub source_file: u32,
    pub source_offset: u32,
}

impl MatAnimDesc {
    pub const SIZE: usize = 28;
}

/// One resolved palette variant of an animated texture — the same shape as
/// [`TextureDesc::palette_offset`]/[`TextureDesc::palette_len`], stored
/// separately since an animated texture has several instead of one.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MatAnimPalette {
    pub palette_offset: u32,
    pub palette_len: u32,
}

impl MatAnimPalette {
    pub const SIZE: usize = 8;
}

/// A per-costume mesh substitution for one node (RE-098).
///
/// A fighter's alternate costumes share one baked `ObjectDesc`/`NodeDesc`
/// run; most nodes draw identically at every costume (RE-098 measured this
/// archive-wide: a third to two-thirds of a fighter's nodes actually differ,
/// never all of them), so only the (node, costume) pairs that genuinely need
/// a different baked colour or palette get an entry here, keyed by the
/// node's *global* index (`ObjectDesc::first_node + local`) rather than by
/// object, so a lookup needs no object context — just the node already being
/// drawn and the costume the caller wants.
///
/// Sorted by `(node, costume)` at build time so [`Pack::costume_mesh`] can
/// binary-search instead of scanning; costume 0 is never stored here, since
/// it is exactly the node's own baked `NodeDesc::mesh`.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CostumeOverride {
    pub node: u32,
    pub costume: u32,
    pub mesh: u32,
}

impl CostumeOverride {
    pub const SIZE: usize = 12;
}

/// A rectangular extent in game units, as `MPGroundData` stores it.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Extent {
    pub top: i16,
    pub bottom: i16,
    pub right: i16,
    pub left: i16,
}

impl Extent {
    pub const SIZE: usize = 8;
}

/// A stage: its render layers, its collision, and the extents a match needs.
///
/// Everything here comes from one `MPGroundData` (RE-028) and the
/// `MPGeometryData` it points at (RE-029). The four `layers` are indices into
/// the object table — resolved at build time from the layer's `DObjDesc`
/// address, so the runtime never searches for them.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StageDesc {
    /// First entry in the line table, and how many follow.
    pub first_line: u32,
    pub line_count: u32,
    /// First entry in the map-point table (spawns, item drops).
    pub first_point: u32,
    pub point_count: u32,
    /// Object index per render-layer slot, or [`StageDesc::NO_LAYER`].
    pub layers: [u32; 4],
    /// How far the camera may travel.
    pub camera: Extent,
    /// The blast zone: outside it a fighter is KO'd.
    pub bounds: Extent,
    pub bgm_id: u32,
    /// Archive file holding the `MPGroundData`, for debugging.
    pub source_file: u32,
    pub source_offset: u32,
    pub _pad: u32,
}

impl StageDesc {
    /// `16 + 16 + 8 + 8 + 16`.
    pub const SIZE: usize = 64;
    pub const NO_LAYER: u32 = u32::MAX;
}

/// Which side of a surface a collision line acts on, matching `MPLineKind`.
pub mod line_kind {
    pub const FLOOR: u16 = 0;
    pub const CEILING: u16 = 1;
    pub const RIGHT_WALL: u16 = 2;
    pub const LEFT_WALL: u16 = 3;
}

/// One collision polyline: a run of [`CollisionVertex`] joined end to end.
///
/// Not a single segment — `MPVertexLinks::vertex2` is a point *count*, so a
/// line is `vertex_count - 1` segments (RE-029).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LineDesc {
    /// First entry in the collision-vertex table.
    pub first_vertex: u32,
    pub vertex_count: u16,
    /// One of [`line_kind`].
    pub kind: u16,
    /// The movable group that owns this line. Group 0 never moves, so its
    /// points are already in world space; the rest are in the group's space.
    pub yakumono: u16,
    /// The line's id in the original flat array, which is what the game's
    /// `stand_line_id` refers to.
    pub id: u16,
}

impl LineDesc {
    pub const SIZE: usize = 12;
}

/// A point on a collision polyline.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CollisionVertex {
    pub x: i16,
    pub y: i16,
    /// Upper byte surface flags (drop-through, cliff), lower byte the material
    /// that sets friction.
    pub flags: u16,
    pub _pad: u16,
}

impl CollisionVertex {
    /// 8, not 6: the N64 struct is 6 bytes, but a power of two turns indexing
    /// into a shift and keeps every entry within one cache line.
    pub const SIZE: usize = 8;
}

/// A point of interest on a stage: `MPMapObjKind` 0..=3 are the four players'
/// start positions, 4 is where items drop.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MapPoint {
    pub kind: u16,
    pub x: i16,
    pub y: i16,
    pub _pad: u16,
}

impl MapPoint {
    pub const SIZE: usize = 8;
}

/// One character's constants, extracted from its `FTAttributes`.
///
/// The full struct (`ssb_rom::fighter::FighterAttributes`) is carried across
/// verbatim rather than pared down to the fields physics reads today: the
/// camera, shadow and shield numbers are already correct in the ROM, and
/// storing them now costs 5 KB for the whole roster and saves a format bump
/// when those subsystems land.
///
/// 192 bytes exactly — 48 words, so the table stays 16-byte aligned and a
/// fighter's constants land in a whole number of cache lines.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FighterDesc {
    /// `FTKind` ordinal. The table is dense and in this order, so it can be
    /// indexed directly, but the field is stored so a reader can assert it.
    pub kind: u32,
    /// Archive file the attributes came from, and where in it. Kept for the
    /// same reason meshes keep theirs: so a wrong number can be traced back to
    /// the bytes it was read from.
    pub source_file: u32,
    pub source_offset: u32,
    pub jumps_max: i32,
    pub is_metallic: u32,
    pub size: f32,
    pub walkslow_anim_length: f32,
    pub walkmiddle_anim_length: f32,
    pub walkfast_anim_length: f32,
    pub throw_walkslow_anim_length: f32,
    pub throw_walkmiddle_anim_length: f32,
    pub throw_walkfast_anim_length: f32,
    pub rebound_anim_length: f32,
    pub walk_speed_mul: f32,
    pub traction: f32,
    pub dash_speed: f32,
    pub dash_decel: f32,
    pub run_speed: f32,
    pub kneebend_anim_length: f32,
    pub jump_vel_x: f32,
    pub jump_height_mul: f32,
    pub jump_height_base: f32,
    pub jumpaerial_vel_x: f32,
    pub jumpaerial_height: f32,
    pub air_accel: f32,
    pub air_speed_max_x: f32,
    pub air_friction: f32,
    pub gravity: f32,
    pub tvel_base: f32,
    pub tvel_fast: f32,
    pub weight: f32,
    pub attack1_followup_frames: f32,
    pub dash_to_run: f32,
    pub shield_size: f32,
    pub shield_break_vel_y: f32,
    pub shadow_size: f32,
    pub jostle_width: f32,
    pub jostle_x: f32,
    pub cam_offset_y: f32,
    pub closeup_camera_zoom: f32,
    pub camera_zoom: f32,
    pub camera_zoom_base: f32,
    /// The collision diamond: `(0, top)`, `(±width, center)`, `(0, bottom)`.
    pub coll_top: f32,
    pub coll_center: f32,
    pub coll_bottom: f32,
    pub coll_width: f32,
    pub cliffcatch_width: f32,
    pub cliffcatch_height: f32,
    /// How long the statuses that end when their animation runs out last, in
    /// frames at playback speed 1.0. Unlike everything above these do not come
    /// from `FTAttributes` at all — they are the lengths of the figatree
    /// scripts themselves, read by [`crate::anim`].
    pub dash_anim_length: f32,
    pub turn_anim_length: f32,
    pub runbrake_anim_length: f32,
    pub squat_anim_length: f32,
    pub squatrv_anim_length: f32,
    pub landing_anim_length: f32,
    pub pass_anim_length: f32,
}

/// Float fields carried per fighter: 43 from `FTAttributes` plus the seven
/// figatree animation lengths.
const SCALARS: usize = 50;

impl FighterDesc {
    /// 56 words: 5 integers, then the 50 floats [`fighter_scalars`] lists,
    /// then one word of padding to keep the stride 16-byte aligned.
    ///
    /// The floats are the 43 scalars of the original `FTAttributes` head
    /// (`jumps_max` and `is_metallic` moved up into the integer group so no
    /// consumer has to bit-cast) followed by the seven figatree lengths.
    pub const SIZE: usize = 224;
}

/// The float fields of a [`FighterDesc`], in the order they are stored.
///
/// Writer and reader both go through this, so a field added in one place
/// cannot silently disagree with the other.
fn fighter_scalars(d: &FighterDesc) -> [f32; SCALARS] {
    [
        d.size,
        d.walkslow_anim_length,
        d.walkmiddle_anim_length,
        d.walkfast_anim_length,
        d.throw_walkslow_anim_length,
        d.throw_walkmiddle_anim_length,
        d.throw_walkfast_anim_length,
        d.rebound_anim_length,
        d.walk_speed_mul,
        d.traction,
        d.dash_speed,
        d.dash_decel,
        d.run_speed,
        d.kneebend_anim_length,
        d.jump_vel_x,
        d.jump_height_mul,
        d.jump_height_base,
        d.jumpaerial_vel_x,
        d.jumpaerial_height,
        d.air_accel,
        d.air_speed_max_x,
        d.air_friction,
        d.gravity,
        d.tvel_base,
        d.tvel_fast,
        d.weight,
        d.attack1_followup_frames,
        d.dash_to_run,
        d.shield_size,
        d.shield_break_vel_y,
        d.shadow_size,
        d.jostle_width,
        d.jostle_x,
        d.cam_offset_y,
        d.closeup_camera_zoom,
        d.camera_zoom,
        d.camera_zoom_base,
        d.coll_top,
        d.coll_center,
        d.coll_bottom,
        d.coll_width,
        d.cliffcatch_width,
        d.cliffcatch_height,
        d.dash_anim_length,
        d.turn_anim_length,
        d.runbrake_anim_length,
        d.squat_anim_length,
        d.squatrv_anim_length,
        d.landing_anim_length,
        d.pass_anim_length,
    ]
}

/// Rebuilds a [`FighterDesc`] from the integer head and [`fighter_scalars`].
fn fighter_from_parts(
    kind: u32,
    source_file: u32,
    source_offset: u32,
    jumps_max: i32,
    is_metallic: u32,
    s: [f32; SCALARS],
) -> FighterDesc {
    FighterDesc {
        kind,
        source_file,
        source_offset,
        jumps_max,
        is_metallic,
        size: s[0],
        walkslow_anim_length: s[1],
        walkmiddle_anim_length: s[2],
        walkfast_anim_length: s[3],
        throw_walkslow_anim_length: s[4],
        throw_walkmiddle_anim_length: s[5],
        throw_walkfast_anim_length: s[6],
        rebound_anim_length: s[7],
        walk_speed_mul: s[8],
        traction: s[9],
        dash_speed: s[10],
        dash_decel: s[11],
        run_speed: s[12],
        kneebend_anim_length: s[13],
        jump_vel_x: s[14],
        jump_height_mul: s[15],
        jump_height_base: s[16],
        jumpaerial_vel_x: s[17],
        jumpaerial_height: s[18],
        air_accel: s[19],
        air_speed_max_x: s[20],
        air_friction: s[21],
        gravity: s[22],
        tvel_base: s[23],
        tvel_fast: s[24],
        weight: s[25],
        attack1_followup_frames: s[26],
        dash_to_run: s[27],
        shield_size: s[28],
        shield_break_vel_y: s[29],
        shadow_size: s[30],
        jostle_width: s[31],
        jostle_x: s[32],
        cam_offset_y: s[33],
        closeup_camera_zoom: s[34],
        camera_zoom: s[35],
        camera_zoom_base: s[36],
        coll_top: s[37],
        coll_center: s[38],
        coll_bottom: s[39],
        coll_width: s[40],
        cliffcatch_width: s[41],
        cliffcatch_height: s[42],
        dash_anim_length: s[43],
        turn_anim_length: s[44],
        runbrake_anim_length: s[45],
        squat_anim_length: s[46],
        squatrv_anim_length: s[47],
        landing_anim_length: s[48],
        pass_anim_length: s[49],
    }
}

/// Divisor the GE applies to `GU_VERTEX_16BIT` positions.
///
/// Vertex positions arrive as `i16` and the hardware normalises them by 32768
/// (RE-020). A node's translation is in raw N64 world units — up to 23364 —
/// so it has to be divided by the same factor or the hierarchy would be
/// assembled 32768x too large relative to the geometry inside it.
pub const MODEL_SCALE: f32 = 32768.0;

/// Direction the baked key light comes from, normalised.
///
/// This project bakes shading into vertex colour at pack time rather than
/// lighting at draw time on the PSP, so it cannot vary the light per stage
/// the way `ftDisplayLightsDrawReflect`
/// (`refs/ssb-decomp-re/src/ft/ftdisplaylights.c`) does on real hardware —
/// doing that would need runtime `sceGuLight` and per-stage context wired
/// through the whole material pipeline, out of this task's scope. What this
/// constant *can* do is be the right single direction rather than an
/// arbitrary one: RE-065 read every stage's real `MPGroundData.light_angle`
/// (`crates/ssb-rom/src/stage.rs`) and found **33 of 41 stages (80%) use
/// exactly `(20.0, 45.0)` degrees** — the game's actual default key light,
/// not a guess. This is that angle's direction, replacing an arbitrary
/// `(2, 4, 3)` placeholder that happened to measure only 9.9 degrees away
/// from it. The remaining 8 stages (Brinstar, Sector Z, Hyrule, Final
/// Destination, Metal Mario's stage, and others) use their own angle, up to
/// 111 degrees away from this one — an accepted, measured deviation until
/// lighting moves to runtime (see RE-065).
// The y component is sin(45 deg), which coincides with 1/sqrt(2) -- that is
// the actual measured direction, not a stand-in for the named constant.
#[allow(clippy::approx_constant)]
const LIGHT_DIR: [f32; 3] = [0.2419, 0.7071, 0.6645]; // (20, 45) degrees, RE-065

/// Floor brightness, so surfaces facing away are shaded rather than black.
const AMBIENT: f32 = 0.35;

/// Converts an N64 lit vertex's packed normal into a shaded colour.
///
/// When `G_LIGHTING` is set, the four bytes the RDP would read as colour are
/// actually `signed char n[3]` plus an unsigned alpha (`Vtx_tn` in `gbi.h`).
/// Drawing them as a colour directly is what produced the saturated red/blue
/// noise in the first on-device render.
///
/// The dot product needs no square root: N64 normals are already unit length
/// scaled to `i8`, and the light direction is normalised above.
fn shade_normal(raw: [u8; 4]) -> [u8; 4] {
    let n = [
        raw[0] as i8 as f32 / 127.0,
        raw[1] as i8 as f32 / 127.0,
        raw[2] as i8 as f32 / 127.0,
    ];
    let ndotl = n[0] * LIGHT_DIR[0] + n[1] * LIGHT_DIR[1] + n[2] * LIGHT_DIR[2];
    let diffuse = if ndotl > 0.0 { ndotl } else { 0.0 };
    let i = AMBIENT + (1.0 - AMBIENT) * diffuse;
    let v = (i * 255.0) as u8;
    [v, v, v, raw[3]]
}

/// Whether a vertex's colour field actually holds a unit normal.
///
/// N64 normals are `i8` components of a unit vector, so `x² + y² + z²` lands
/// near `127² = 16129`. Arbitrary colours have no reason to.
///
/// This exists because **the display list alone cannot tell us**. `G_LIGHTING`
/// is set per-object by `objdisplay.c` before the list runs, so a list that
/// relies on inherited state carries no geometry-mode command of its own.
/// Measured over the whole archive: of the vertices whose list *did* set
/// `G_LIGHTING`, 100% look like unit normals — the test has no false positives
/// on known-lit data — while 69.4% of the supposedly unlit vertices look like
/// normals too. Drawing those as colours is what produced the saturated
/// red/green/cyan polygons in the first textured render.
pub fn looks_like_unit_normal(c: [u8; 4]) -> bool {
    let x = c[0] as i8 as i32;
    let y = c[1] as i8 as i32;
    let z = c[2] as i8 as i32;
    let m = x * x + y * y + z * z;
    // Generous band around 127²: N64 normals are quantised and not exactly unit.
    (11_000..=21_000).contains(&m)
}

/// Rounds `v` up to the next multiple of [`ALIGN`].
pub fn align_up(v: usize) -> usize {
    v.div_ceil(ALIGN) * ALIGN
}

// ---------------------------------------------------------------------------
// Writer
// ---------------------------------------------------------------------------

/// Builds a pack file.
#[derive(Default)]
pub struct PackWriter {
    meshes: Vec<MeshDesc>,
    prims: Vec<PrimDesc>,
    textures: Vec<TextureDesc>,
    objects: Vec<ObjectDesc>,
    nodes: Vec<NodeDesc>,
    anims: Vec<AnimDesc>,
    anim_joints: Vec<AnimJoint>,
    /// Animation files already in the blob, by archive file id. Kirby and
    /// Jigglypuff share three of theirs outright, and every polygon variant
    /// shares all seven with the character it copies.
    anim_files: alloc::collections::BTreeMap<u32, (u32, u32)>,
    stages: Vec<StageDesc>,
    lines: Vec<LineDesc>,
    coll_vertices: Vec<CollisionVertex>,
    points: Vec<MapPoint>,
    fighters: Vec<FighterDesc>,
    mat_anims: Vec<MatAnimDesc>,
    mat_anim_palettes: Vec<MatAnimPalette>,
    /// Source files already in the blob for [`Self::add_mat_anim`], the same
    /// dedup shape as `anim_files`.
    mat_anim_files: alloc::collections::BTreeMap<u32, (u32, u32)>,
    costume_overrides: Vec<CostumeOverride>,
    blob: Vec<u8>,
}

impl PackWriter {
    pub fn new() -> Self {
        Self::default()
    }

    /// Appends bytes to the blob, aligned, returning their offset.
    fn push_blob(&mut self, bytes: &[u8]) -> u32 {
        let at = align_up(self.blob.len());
        self.blob.resize(at, 0);
        self.blob.extend_from_slice(bytes);
        at as u32
    }

    /// Adds a texture, returning its index. `clamp_s`/`clamp_t` are
    /// `mesh::TextureRef::clamp_s`/`clamp_t` (RE-102): whether the device
    /// must sample this axis with `Clamp` rather than the default `Repeat`.
    pub fn add_texture(
        &mut self,
        tex: &crate::psp_texture::PspTexture,
        clamp_s: bool,
        clamp_t: bool,
    ) -> u32 {
        let data_offset = self.push_blob(&tex.data);
        let palette_bytes: Vec<u8> = tex.palette.iter().flat_map(|c| c.to_le_bytes()).collect();
        let palette_offset = if palette_bytes.is_empty() {
            0
        } else {
            self.push_blob(&palette_bytes)
        };
        let wrap = (clamp_s as u8 * TextureDesc::CLAMP_S) | (clamp_t as u8 * TextureDesc::CLAMP_T);

        self.textures.push(TextureDesc {
            width: tex.width as u16,
            height: tex.height as u16,
            stride: tex.stride as u16,
            psm: tex.format as u8,
            swizzled: tex.swizzled as u8,
            data_offset,
            data_len: tex.data.len() as u32,
            palette_offset,
            palette_len: tex.palette.len() as u32,
            levels: tex.levels.max(1),
            mat_anim: TextureDesc::NO_ANIM,
            role: TextureDesc::ROLE_NORMAL,
            wrap,
        });
        (self.textures.len() - 1) as u32
    }

    /// Adds a "framebuffer capture" texture entry: no baked bytes at all, a
    /// small buffer the PSP device fills from the just-rendered screen the
    /// first time the LB "loading transition" system starts (RE-099/RE-100,
    /// `mobj::LB_TRANSITION_SEGMENT`). `width`/`height` come from the real
    /// display list's own `G_SETTILESIZE` (`mesh::TextureRef::framebuffer`),
    /// since that is what the primitive's baked UVs were authored against.
    pub fn add_framebuffer_texture(&mut self, width: u16, height: u16) -> u32 {
        self.textures.push(TextureDesc {
            width,
            height,
            // Matches every other `TextureDesc`'s convention: `stride` is the
            // power-of-two the GE actually addresses (`sceGuTexImage`'s
            // `bufferwidth`), `width` is metadata only.
            stride: width.next_power_of_two(),
            psm: crate::psp_texture::Psm::Psm8888 as u8,
            swizzled: 0,
            data_offset: 0,
            data_len: 0,
            palette_offset: 0,
            palette_len: 0,
            levels: 1,
            mat_anim: TextureDesc::NO_ANIM,
            role: TextureDesc::ROLE_FRAMEBUFFER,
            // Always a single full-frame quad (RE-099/RE-100): its UVs never
            // exceed the tile, so `Repeat` vs `Clamp` cannot be told apart.
            wrap: 0,
        });
        (self.textures.len() - 1) as u32
    }

    /// Adds one animated palette table: the driving script plus every
    /// palette variant it can cycle to (RE-089/RE-090). Returns the index a
    /// [`PackWriter::set_texture_mat_anim`] call should give the texture
    /// this animates.
    ///
    /// `source_file`/`file_bytes` are the *whole* source archive file, not
    /// just the script — deduplicated per file the same way [`Self::add_anim`]
    /// deduplicates joint-animation files, since [`crate::matanim::
    /// MaterialJoint::tick`] is already built to run against a whole file's
    /// bytes rather than a pre-sliced script, and a script's own end is not
    /// knowable without decoding it.
    pub fn add_mat_anim(
        &mut self,
        source_file: u32,
        file_bytes: &[u8],
        script: u32,
        source_offset: u32,
        palettes: &[Vec<u32>],
    ) -> u32 {
        let (file_offset, file_len) = match self.mat_anim_files.get(&source_file) {
            Some(&at) => at,
            None => {
                let at = (self.push_blob(file_bytes), file_bytes.len() as u32);
                self.mat_anim_files.insert(source_file, at);
                at
            }
        };
        let first_palette = self.mat_anim_palettes.len() as u32;
        for p in palettes {
            let bytes: Vec<u8> = p.iter().flat_map(|c| c.to_le_bytes()).collect();
            let palette_offset = self.push_blob(&bytes);
            self.mat_anim_palettes.push(MatAnimPalette {
                palette_offset,
                palette_len: p.len() as u32,
            });
        }
        self.mat_anims.push(MatAnimDesc {
            file_offset,
            file_len,
            script,
            first_palette,
            palette_count: palettes.len() as u32,
            source_file,
            source_offset,
        });
        (self.mat_anims.len() - 1) as u32
    }

    /// Points an already-added texture at an animated palette table added
    /// via [`PackWriter::add_mat_anim`].
    pub fn set_texture_mat_anim(&mut self, texture: u32, mat_anim: u32) {
        if let Some(t) = self.textures.get_mut(texture as usize) {
            t.mat_anim = mat_anim;
        }
    }

    /// Adds a converted mesh. `texture_for` maps a primitive index to a texture
    /// index already added via [`PackWriter::add_texture`].
    pub fn add_mesh(
        &mut self,
        mesh: &crate::mesh::Mesh,
        source_file: u32,
        source_offset: u32,
        texture_for: impl Fn(usize) -> Option<u32>,
    ) -> u32 {
        // Which vertices are shaded from a normal rather than drawn as a
        // literal colour. The IR keeps the raw bytes (it is deliberately
        // lossless); interpreting them is this lowering step's job.
        //
        // Decided per-vertex, not per-primitive (RE-103): a fighter's mixed
        // material -- decal highlights drawn as literal colour alongside a
        // lit body sharing the same vertex buffer -- routinely lands at a
        // 20-80% split within one primitive, nowhere near unanimous. Voting
        // by majority forces every vertex on the losing side to the wrong
        // interpretation: shaded when it should have stayed a literal
        // colour, or (what RE-103 actually found, on Fox/Falcon/Kirby/Ness)
        // left as a raw normal read straight into RGB when it should have
        // been shaded -- normals are small, near-zero-centred bytes, and
        // painting them as colour directly is exactly what a "melted",
        // rainbow-noise surface looks like. `p.material.lit` (trusted when
        // the geometry mode says so) still applies to every vertex the
        // primitive touches, since real hardware computes lighting for the
        // whole draw once G_LIGHTING is on; `looks_like_unit_normal`'s
        // per-vertex, data-driven fallback is what makes the two kinds of
        // vertex coexist correctly within one primitive.
        let mut lit = alloc::vec![false; mesh.vertices.len()];
        for p in &mesh.primitives {
            for &i in &p.indices {
                let Some(slot) = lit.get_mut(i as usize) else {
                    continue;
                };
                if *slot {
                    continue;
                }
                *slot = p.material.lit
                    || mesh
                        .vertices
                        .get(i as usize)
                        .is_some_and(|v| looks_like_unit_normal(v.rgba));
            }
        }

        // RE-106: `MeshMaterial::prim_color` is not a literal colour despite
        // the name -- `material_now()` (mesh.rs) overwrites it with
        // `combiner_shade_scale`'s result whenever the combiner reads
        // `PRIMITIVE`/`ENVIRONMENT` in a `SHADE * constant` shape (RE-043).
        // Nothing downstream ever multiplied it back in: the device has no
        // fixed-function stage to scale an untextured vertex colour by a
        // constant, so unlike `TEXTURE_BLEND`'s baseline colour (also baked
        // at vertex-assembly time, but into `push_vertex`) this one is
        // cheapest to fold in here instead. First primitive to touch a
        // vertex wins, mirroring `lit` just above -- a vertex shared across
        // primitives with different scales is already the rare case
        // `merge_by_material` keeps as separate primitives to begin with.
        let mut prim_scale: alloc::vec::Vec<Option<[u8; 4]>> =
            alloc::vec![None; mesh.vertices.len()];
        for p in &mesh.primitives {
            let Some(s) = p.material.prim_color else {
                continue;
            };
            for &i in &p.indices {
                if let Some(slot @ None) = prim_scale.get_mut(i as usize) {
                    *slot = Some(s);
                }
            }
        }

        // Vertices, converted to the GE layout.
        let mut verts = Vec::with_capacity(mesh.vertices.len() * VERTEX_SIZE);
        for (i, v) in mesh.vertices.iter().enumerate() {
            let rgba = if lit[i] { shade_normal(v.rgba) } else { v.rgba };
            let rgba = match prim_scale[i] {
                Some(s) => [
                    ((rgba[0] as u32 * s[0] as u32) / 255) as u8,
                    ((rgba[1] as u32 * s[1] as u32) / 255) as u8,
                    ((rgba[2] as u32 * s[2] as u32) / 255) as u8,
                    rgba[3],
                ],
                None => rgba,
            };
            let packed = PackedVertex {
                u: v.uv[0],
                v: v.uv[1],
                color: crate::psp_texture::pack_abgr(rgba),
                x: v.pos[0],
                y: v.pos[1],
                z: v.pos[2],
            };
            verts.extend_from_slice(&packed.u.to_le_bytes());
            verts.extend_from_slice(&packed.v.to_le_bytes());
            verts.extend_from_slice(&packed.color.to_le_bytes());
            verts.extend_from_slice(&packed.x.to_le_bytes());
            verts.extend_from_slice(&packed.y.to_le_bytes());
            verts.extend_from_slice(&packed.z.to_le_bytes());
            // Tail padding, so the stride matches `size_of::<PackedVertex>()`
            // and the GE's stride requirement.
            verts.extend_from_slice(&[0u8; 2]);
        }
        let vertex_offset = self.push_blob(&verts);

        let first_prim = self.prims.len() as u32;
        for (i, p) in mesh.primitives.iter().enumerate() {
            let indices: Vec<u8> = p.indices.iter().flat_map(|i| i.to_le_bytes()).collect();
            let index_offset = self.push_blob(&indices);

            let m = &p.material;
            let mut f = 0u32;
            if m.cull_back {
                f |= flags::CULL_BACK;
            }
            if m.cull_front {
                f |= flags::CULL_FRONT;
            }
            if m.lit {
                f |= flags::LIT;
            }
            if m.smooth {
                f |= flags::SMOOTH;
            }
            if m.z_buffer {
                f |= flags::Z_BUFFER;
            }
            if m.alpha_test {
                f |= flags::ALPHA_TEST;
            }
            if m.translucent {
                f |= flags::TRANSLUCENT;
            }
            if m.texture_blend.is_some() {
                f |= flags::TEXTURE_BLEND;
            }
            if m.flat_color.is_some() {
                f |= flags::FLAT_COLOR;
            }
            let (blend_base, blend_target) = m.texture_blend.map_or((0, 0), |(base, target)| {
                (
                    crate::psp_texture::pack_abgr(base),
                    crate::psp_texture::pack_abgr(target),
                )
            });

            self.prims.push(PrimDesc {
                texture: texture_for(i).unwrap_or(PrimDesc::NO_TEXTURE),
                flags: f,
                prim_color: m.prim_color.map_or(0, crate::psp_texture::pack_abgr),
                env_color: m.env_color.map_or(0, crate::psp_texture::pack_abgr),
                index_offset,
                index_count: p.indices.len() as u32,
                texture_blend_base: blend_base,
                texture_blend_target: blend_target,
                flat_color: m.flat_color.map_or(0, crate::psp_texture::pack_abgr),
            });
        }

        self.meshes.push(MeshDesc {
            vertex_offset,
            vertex_count: mesh.vertices.len() as u32,
            first_prim,
            prim_count: mesh.primitives.len() as u32,
            source_file,
            source_offset,
        });
        (self.meshes.len() - 1) as u32
    }

    /// Adds a scene graph, returning the object's index.
    ///
    /// `mesh_for` maps a node's index within `graph` to a mesh index already
    /// added via [`PackWriter::add_mesh`]; nodes whose display list did not
    /// convert (or which have none) become pure transforms.
    ///
    /// `extra` carries geometry a node cannot hold, since a node has room for
    /// exactly one mesh: `(space, mesh)`, where `space` is the node index whose
    /// world matrix the mesh draws under, or `None` for the object root. Each
    /// becomes an extra leaf node sharing that matrix.
    ///
    /// Across the archive these are 20 lists, all of them second and later
    /// entries of a `DObjDLLink` array — the same joint drawn again into a
    /// different task display-list head, which is how the game splits a joint
    /// across render layers. The `space` indirection exists for the other
    /// source, a `Gfx *dls[2]` pair's first list, which runs *before* the
    /// node's matrix is pushed and so belongs to the parent; no shipped pre
    /// list turns out to carry triangles, but the geometry is placed correctly
    /// if one ever does.
    pub fn add_object(
        &mut self,
        graph: &crate::scene::SceneGraph,
        source_file: u32,
        mesh_for: impl Fn(usize) -> Option<u32>,
        extra: &[(Option<usize>, u32)],
    ) -> u32 {
        let first_node = self.nodes.len() as u32;
        // `world_transforms` is O(n) over the whole graph, so it is computed
        // once here rather than per node.
        let worlds = graph.world_transforms();
        let normalised = |m: &crate::scene::Mat4| {
            let mut world = m.0;
            // Bring the translation into the same normalised space as the
            // `i16` vertex positions; see `MODEL_SCALE`.
            world[12] /= MODEL_SCALE;
            world[13] /= MODEL_SCALE;
            world[14] /= MODEL_SCALE;
            world
        };

        for (i, node) in graph.nodes.iter().enumerate() {
            self.nodes.push(NodeDesc {
                mesh: mesh_for(i).unwrap_or(NodeDesc::NO_MESH),
                parent: match node.parent {
                    Some(p) => first_node + p as u32,
                    None => NodeDesc::NO_PARENT,
                },
                world: normalised(&worlds[i]),
                rest_translate: node.desc.translate,
                rest_rotate: node.desc.rotate,
                rest_scale: node.desc.scale,
                // Kinds 45-50 and `0x8000` are all camera-relative: none of
                // them multiply the node's own rotation into the MVP
                // (`gcPrepDObjMatrix` cases 44-50 in `objdisplay.c`; 44 is
                // `0x8000`/`RecalcRotRpyRSca`, the only one of this group that
                // skips the sin/cos spin term entirely). Every shipped
                // `0x8000` node's `rotate` is `[0, 0, 0]` (checked across the
                // whole archive, RE-062), so treating it as a spin-0
                // `FLAG_BILLBOARD` node reuses the already-verified 46/48
                // path (RE-048, RE-049) exactly. `0x1000`/`Kind50` (case 50)
                // is structurally identical to `Kind48` (case 48) -- same
                // move-word layout, same per-node scale math -- just built
                // from `sGCMatrixMod2F` (locked to the camera's yaw) instead
                // of `sGCMatrixMod1F` (locked to the camera's pitch), so it
                // gets the same treatment for the same reason. No shipped
                // node uses it (RE-063: 0/3117 archive-wide), so this is
                // fidelity for a kind the ROM never actually exercises, not a
                // measured fix.
                flags: match node.desc.transform_kind() {
                    crate::scene::TransformKind::Kind46
                    | crate::scene::TransformKind::Kind48
                    | crate::scene::TransformKind::Kind50
                    | crate::scene::TransformKind::RecalcRotRpyRSca => NodeDesc::FLAG_BILLBOARD,
                    _ => 0,
                },
            });
            debug_assert_eq!(self.nodes.len() - 1, first_node as usize + i);
        }

        // An extra leaf carries a mesh that could not sit on the node whose
        // space it runs in, so it *is* that space: identity of its own.
        for &(space, mesh) in extra {
            self.nodes.push(NodeDesc {
                mesh,
                parent: match space {
                    Some(p) => first_node + p as u32,
                    None => NodeDesc::NO_PARENT,
                },
                world: match space {
                    Some(p) => normalised(&worlds[p]),
                    None => normalised(&crate::scene::Mat4::IDENTITY),
                },
                rest_translate: [0.0; 3],
                rest_rotate: [0.0; 3],
                rest_scale: [1.0; 3],
                flags: 0,
            });
        }

        self.objects.push(ObjectDesc {
            first_node,
            node_count: (graph.nodes.len() + extra.len()) as u32,
            source_file,
            source_offset: graph.offset,
        });
        (self.objects.len() - 1) as u32
    }

    /// Adds one fighter's animation for one movement status.
    ///
    /// `joints` gives, per animation joint in table order, the byte offset of
    /// its script within `script` (or `None`) and the node it drives (or
    /// `None`). Working out that pairing is the build-time job described in
    /// RE-036; by the time it reaches here it is a list.
    ///
    /// The script bytes are deduplicated by `source_file`, so a shared
    /// animation is stored once however many fighters point at it.
    pub fn add_anim(
        &mut self,
        fighter: u32,
        slot: u32,
        source_file: u32,
        frames: u32,
        script: &[u8],
        joints: &[(Option<u32>, Option<u32>)],
    ) -> u32 {
        let (script_offset, script_len) = match self.anim_files.get(&source_file) {
            Some(&at) => at,
            None => {
                let at = (self.push_blob(script), script.len() as u32);
                self.anim_files.insert(source_file, at);
                at
            }
        };
        let first_joint = self.anim_joints.len() as u32;
        for &(script_at, node) in joints {
            self.anim_joints.push(AnimJoint {
                script: script_at.unwrap_or(AnimJoint::NO_SCRIPT),
                node: node.unwrap_or(AnimJoint::NO_NODE),
            });
        }
        self.anims.push(AnimDesc {
            fighter,
            slot,
            source_file,
            frames,
            first_joint,
            joint_count: joints.len() as u32,
            script_offset,
            script_len,
        });
        (self.anims.len() - 1) as u32
    }

    /// Adds a stage: its four render layers, its collision lines and its map
    /// points, returning its index.
    ///
    /// `object_for(file, offset)` maps a layer's `DObjDesc` address to the
    /// object index [`Self::add_object`] gave it. That is an exact lookup, not
    /// a search — `ObjectDesc` records the very address `MPGroundDesc` names —
    /// so a layer either resolves or is honestly reported as absent.
    ///
    /// `map` is optional because the header and the collision come from
    /// different structs in different files; a stage with unreadable geometry
    /// still contributes its bounds and its layers.
    /// Where an object's nodes begin in the node table, for a caller that has
    /// only the object index. Stage animation needs it to turn a graph-local
    /// node number into the absolute one a joint entry stores.
    pub fn object_first_node(&self, object: u32) -> Option<u32> {
        self.objects.get(object as usize).map(|o| o.first_node)
    }

    pub fn object_node_count(&self, object: u32) -> Option<u32> {
        self.objects.get(object as usize).map(|o| o.node_count)
    }

    pub fn add_stage(
        &mut self,
        ground: &crate::stage::GroundData,
        map: Option<&crate::collision::CollisionMap>,
        object_for: impl Fn(u32, u32) -> Option<u32>,
    ) -> u32 {
        let first_line = self.lines.len() as u32;
        let first_point = self.points.len() as u32;

        if let Some(map) = map {
            for line in &map.lines {
                let first_vertex = self.coll_vertices.len() as u32;
                self.coll_vertices
                    .extend(line.points.iter().map(|p| CollisionVertex {
                        x: p.pos[0],
                        y: p.pos[1],
                        flags: p.flags,
                        _pad: 0,
                    }));
                self.lines.push(LineDesc {
                    first_vertex,
                    vertex_count: line.points.len() as u16,
                    kind: match line.kind {
                        crate::collision::LineKind::Floor => line_kind::FLOOR,
                        crate::collision::LineKind::Ceiling => line_kind::CEILING,
                        crate::collision::LineKind::RightWall => line_kind::RIGHT_WALL,
                        crate::collision::LineKind::LeftWall => line_kind::LEFT_WALL,
                    },
                    yakumono: line.yakumono,
                    id: line.id,
                });
            }
            self.points.extend(map.map_objects.iter().map(|o| MapPoint {
                kind: o.kind,
                x: o.pos[0],
                y: o.pos[1],
                _pad: 0,
            }));
        }

        let mut layers = [StageDesc::NO_LAYER; 4];
        for layer in &ground.layers {
            let Some(slot) = layers.get_mut(layer.index as usize) else {
                continue;
            };
            if let Some(object) = object_for(layer.graph.0, layer.graph.1) {
                *slot = object;
            }
        }

        let extent = |b: crate::stage::Bounds| Extent {
            top: b.top,
            bottom: b.bottom,
            right: b.right,
            left: b.left,
        };
        self.stages.push(StageDesc {
            first_line,
            line_count: self.lines.len() as u32 - first_line,
            first_point,
            point_count: self.points.len() as u32 - first_point,
            layers,
            camera: extent(ground.camera_bounds),
            bounds: extent(ground.map_bounds),
            bgm_id: ground.bgm_id,
            source_file: ground.file,
            source_offset: ground.offset,
            _pad: 0,
        });
        (self.stages.len() - 1) as u32
    }

    /// Serialises the pack.
    /// Adds one character's constants. Call in `FTKind` order.
    ///
    /// `anims` comes from a different place than `f` — the figatree files
    /// rather than `FTAttributes` — so it is passed separately rather than
    /// folded into the fighter, which knows nothing about animation.
    pub fn add_fighter(
        &mut self,
        f: &crate::fighter::Fighter,
        anims: &crate::anim::FighterLengths,
    ) {
        use crate::anim;
        let a = &f.attributes;
        let frames = |slot: usize| anims.frames[slot] as f32;
        self.fighters.push(FighterDesc {
            kind: f.file.kind as u32,
            source_file: f.file.file,
            source_offset: f.file.offset,
            jumps_max: a.jumps_max,
            is_metallic: a.is_metallic as u32,
            size: a.size,
            walkslow_anim_length: a.walkslow_anim_length,
            walkmiddle_anim_length: a.walkmiddle_anim_length,
            walkfast_anim_length: a.walkfast_anim_length,
            throw_walkslow_anim_length: a.throw_walkslow_anim_length,
            throw_walkmiddle_anim_length: a.throw_walkmiddle_anim_length,
            throw_walkfast_anim_length: a.throw_walkfast_anim_length,
            rebound_anim_length: a.rebound_anim_length,
            walk_speed_mul: a.walk_speed_mul,
            traction: a.traction,
            dash_speed: a.dash_speed,
            dash_decel: a.dash_decel,
            run_speed: a.run_speed,
            kneebend_anim_length: a.kneebend_anim_length,
            jump_vel_x: a.jump_vel_x,
            jump_height_mul: a.jump_height_mul,
            jump_height_base: a.jump_height_base,
            jumpaerial_vel_x: a.jumpaerial_vel_x,
            jumpaerial_height: a.jumpaerial_height,
            air_accel: a.air_accel,
            air_speed_max_x: a.air_speed_max_x,
            air_friction: a.air_friction,
            gravity: a.gravity,
            tvel_base: a.tvel_base,
            tvel_fast: a.tvel_fast,
            weight: a.weight,
            attack1_followup_frames: a.attack1_followup_frames,
            dash_to_run: a.dash_to_run,
            shield_size: a.shield_size,
            shield_break_vel_y: a.shield_break_vel_y,
            shadow_size: a.shadow_size,
            jostle_width: a.jostle_width,
            jostle_x: a.jostle_x,
            cam_offset_y: a.cam_offset_y,
            closeup_camera_zoom: a.closeup_camera_zoom,
            camera_zoom: a.camera_zoom,
            camera_zoom_base: a.camera_zoom_base,
            coll_top: a.map_coll.top,
            coll_center: a.map_coll.center,
            coll_bottom: a.map_coll.bottom,
            coll_width: a.map_coll.width,
            cliffcatch_width: a.cliffcatch_coll.0,
            cliffcatch_height: a.cliffcatch_coll.1,
            dash_anim_length: frames(anim::SLOT_DASH),
            turn_anim_length: frames(anim::SLOT_TURN),
            runbrake_anim_length: frames(anim::SLOT_RUN_BRAKE),
            squat_anim_length: frames(anim::SLOT_SQUAT),
            squatrv_anim_length: frames(anim::SLOT_SQUAT_RV),
            landing_anim_length: frames(anim::SLOT_LANDING),
            pass_anim_length: frames(anim::SLOT_PASS),
        });
    }

    pub fn fighter_count(&self) -> usize {
        self.fighters.len()
    }

    /// An object already added, so a later step can name its nodes.
    pub fn object(&self, i: u32) -> Option<ObjectDesc> {
        self.objects.get(i as usize).copied()
    }

    /// Registers a per-costume mesh substitution (RE-098). `node` is a
    /// *global* node index — `object(o).first_node + local` for whichever
    /// local node within object `o` the substitution belongs to — and `mesh`
    /// an already-added mesh index carrying that costume's own baked colour
    /// or palette. Never call this for costume 0: it is the node's own
    /// `NodeDesc::mesh`, already the fallback [`Pack::costume_mesh`] leaves
    /// callers to use when no override exists.
    pub fn add_costume_override(&mut self, node: u32, costume: u32, mesh: u32) {
        debug_assert_ne!(costume, 0, "costume 0 is the node's own baked mesh");
        self.costume_overrides.push(CostumeOverride {
            node,
            costume,
            mesh,
        });
    }

    pub fn finish(self) -> Vec<u8> {
        let table_bytes = self.meshes.len() * MeshDesc::SIZE
            + self.prims.len() * PrimDesc::SIZE
            + self.textures.len() * TextureDesc::SIZE
            + self.objects.len() * ObjectDesc::SIZE
            + self.nodes.len() * NodeDesc::SIZE
            + self.stages.len() * StageDesc::SIZE
            + self.lines.len() * LineDesc::SIZE
            + self.coll_vertices.len() * CollisionVertex::SIZE
            + self.points.len() * MapPoint::SIZE
            + self.fighters.len() * FighterDesc::SIZE
            + self.anims.len() * AnimDesc::SIZE
            + self.anim_joints.len() * AnimJoint::SIZE
            + self.mat_anims.len() * MatAnimDesc::SIZE
            + self.mat_anim_palettes.len() * MatAnimPalette::SIZE
            + self.costume_overrides.len() * CostumeOverride::SIZE;
        let blob_offset = align_up(Header::SIZE + table_bytes);

        // Sorted by (node, costume) so the reader can binary-search rather
        // than scan; `finish` takes `self` by value, so this is the one
        // place that can still mutate the list before it is written.
        let mut costume_overrides = self.costume_overrides;
        costume_overrides.sort_unstable_by_key(|o| (o.node, o.costume));

        let mut out = Vec::with_capacity(blob_offset + self.blob.len());

        // Header.
        out.extend_from_slice(&MAGIC.to_le_bytes());
        out.extend_from_slice(&VERSION.to_le_bytes());
        out.extend_from_slice(&(self.meshes.len() as u32).to_le_bytes());
        out.extend_from_slice(&(self.prims.len() as u32).to_le_bytes());
        out.extend_from_slice(&(self.textures.len() as u32).to_le_bytes());
        out.extend_from_slice(&(blob_offset as u32).to_le_bytes());
        out.extend_from_slice(&(self.blob.len() as u32).to_le_bytes());
        out.extend_from_slice(&(self.objects.len() as u32).to_le_bytes());
        out.extend_from_slice(&(self.nodes.len() as u32).to_le_bytes());
        out.extend_from_slice(&(self.stages.len() as u32).to_le_bytes());
        out.extend_from_slice(&(self.lines.len() as u32).to_le_bytes());
        out.extend_from_slice(&(self.coll_vertices.len() as u32).to_le_bytes());
        out.extend_from_slice(&(self.points.len() as u32).to_le_bytes());
        out.extend_from_slice(&(self.fighters.len() as u32).to_le_bytes());
        out.extend_from_slice(&(self.anims.len() as u32).to_le_bytes());
        out.extend_from_slice(&(self.anim_joints.len() as u32).to_le_bytes());
        out.extend_from_slice(&(self.mat_anims.len() as u32).to_le_bytes());
        out.extend_from_slice(&(self.mat_anim_palettes.len() as u32).to_le_bytes());
        out.extend_from_slice(&(costume_overrides.len() as u32).to_le_bytes());
        out.resize(Header::SIZE, 0);

        for m in &self.meshes {
            for v in [
                m.vertex_offset,
                m.vertex_count,
                m.first_prim,
                m.prim_count,
                m.source_file,
                m.source_offset,
            ] {
                out.extend_from_slice(&v.to_le_bytes());
            }
        }
        for p in &self.prims {
            for v in [
                p.texture,
                p.flags,
                p.prim_color,
                p.env_color,
                p.index_offset,
                p.index_count,
                p.texture_blend_base,
                p.texture_blend_target,
                p.flat_color,
            ] {
                out.extend_from_slice(&v.to_le_bytes());
            }
        }
        for t in &self.textures {
            out.extend_from_slice(&t.width.to_le_bytes());
            out.extend_from_slice(&t.height.to_le_bytes());
            out.extend_from_slice(&t.stride.to_le_bytes());
            out.push(t.psm);
            out.push(t.swizzled);
            out.extend_from_slice(&t.data_offset.to_le_bytes());
            out.extend_from_slice(&t.data_len.to_le_bytes());
            out.extend_from_slice(&t.palette_offset.to_le_bytes());
            out.extend_from_slice(&t.palette_len.to_le_bytes());
            out.extend_from_slice(&t.levels.to_le_bytes());
            out.extend_from_slice(&t.mat_anim.to_le_bytes());
            out.extend_from_slice(&t.role.to_le_bytes());
            out.push(t.wrap);
            out.extend_from_slice(&[0u8; 3]);
        }
        for o in &self.objects {
            for v in [o.first_node, o.node_count, o.source_file, o.source_offset] {
                out.extend_from_slice(&v.to_le_bytes());
            }
        }
        for n in &self.nodes {
            out.extend_from_slice(&n.mesh.to_le_bytes());
            out.extend_from_slice(&n.parent.to_le_bytes());
            for f in n.world {
                out.extend_from_slice(&f.to_le_bytes());
            }
            for f in n
                .rest_translate
                .iter()
                .chain(&n.rest_rotate)
                .chain(&n.rest_scale)
            {
                out.extend_from_slice(&f.to_le_bytes());
            }
            out.extend_from_slice(&n.flags.to_le_bytes());
            // Pad to NodeDesc::SIZE so the stride stays 16-byte aligned.
            out.extend_from_slice(&[0u8; NodeDesc::SIZE - 112]);
        }
        for s in &self.stages {
            for v in [s.first_line, s.line_count, s.first_point, s.point_count] {
                out.extend_from_slice(&v.to_le_bytes());
            }
            for v in s.layers {
                out.extend_from_slice(&v.to_le_bytes());
            }
            for e in [s.camera, s.bounds] {
                for v in [e.top, e.bottom, e.right, e.left] {
                    out.extend_from_slice(&v.to_le_bytes());
                }
            }
            for v in [s.bgm_id, s.source_file, s.source_offset, s._pad] {
                out.extend_from_slice(&v.to_le_bytes());
            }
        }
        for l in &self.lines {
            out.extend_from_slice(&l.first_vertex.to_le_bytes());
            for v in [l.vertex_count, l.kind, l.yakumono, l.id] {
                out.extend_from_slice(&v.to_le_bytes());
            }
        }
        for v in &self.coll_vertices {
            out.extend_from_slice(&v.x.to_le_bytes());
            out.extend_from_slice(&v.y.to_le_bytes());
            out.extend_from_slice(&v.flags.to_le_bytes());
            out.extend_from_slice(&v._pad.to_le_bytes());
        }
        for p in &self.points {
            out.extend_from_slice(&p.kind.to_le_bytes());
            out.extend_from_slice(&p.x.to_le_bytes());
            out.extend_from_slice(&p.y.to_le_bytes());
            out.extend_from_slice(&p._pad.to_le_bytes());
        }
        for d in &self.fighters {
            for v in [d.kind, d.source_file, d.source_offset] {
                out.extend_from_slice(&v.to_le_bytes());
            }
            out.extend_from_slice(&d.jumps_max.to_le_bytes());
            out.extend_from_slice(&d.is_metallic.to_le_bytes());
            for v in fighter_scalars(d) {
                out.extend_from_slice(&v.to_le_bytes());
            }
            out.extend_from_slice(&0u32.to_le_bytes());
        }

        for a in &self.anims {
            for v in [
                a.fighter,
                a.slot,
                a.source_file,
                a.frames,
                a.first_joint,
                a.joint_count,
                a.script_offset,
                a.script_len,
            ] {
                out.extend_from_slice(&v.to_le_bytes());
            }
        }
        for j in &self.anim_joints {
            out.extend_from_slice(&j.script.to_le_bytes());
            out.extend_from_slice(&j.node.to_le_bytes());
        }
        for a in &self.mat_anims {
            for v in [
                a.file_offset,
                a.file_len,
                a.script,
                a.first_palette,
                a.palette_count,
                a.source_file,
                a.source_offset,
            ] {
                out.extend_from_slice(&v.to_le_bytes());
            }
        }
        for p in &self.mat_anim_palettes {
            out.extend_from_slice(&p.palette_offset.to_le_bytes());
            out.extend_from_slice(&p.palette_len.to_le_bytes());
        }
        for o in &costume_overrides {
            for v in [o.node, o.costume, o.mesh] {
                out.extend_from_slice(&v.to_le_bytes());
            }
        }

        out.resize(blob_offset, 0);
        out.extend_from_slice(&self.blob);
        out
    }

    pub fn mesh_count(&self) -> usize {
        self.meshes.len()
    }
    pub fn texture_count(&self) -> usize {
        self.textures.len()
    }
    pub fn stage_count(&self) -> usize {
        self.stages.len()
    }
    pub fn mat_anim_count(&self) -> usize {
        self.mat_anims.len()
    }
    pub fn blob_len(&self) -> usize {
        self.blob.len()
    }
}

// ---------------------------------------------------------------------------
// Reader (no_std, zero-copy)
// ---------------------------------------------------------------------------

/// Why a pack could not be read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackError {
    TooSmall,
    BadMagic(u32),
    BadVersion(u32),
    /// A descriptor pointed outside the blob region.
    OutOfBounds,
}

/// Zero-copy view over a loaded pack.
///
/// Borrows the buffer; every accessor returns slices into it, so the renderer
/// can hand pointers straight to the GE.
pub struct Pack<'a> {
    data: &'a [u8],
    mesh_count: u32,
    prim_count: u32,
    texture_count: u32,
    object_count: u32,
    node_count: u32,
    stage_count: u32,
    line_count: u32,
    coll_vertex_count: u32,
    point_count: u32,
    fighter_count: u32,
    anim_count: u32,
    anim_joint_count: u32,
    mat_anim_count: u32,
    mat_anim_palette_count: u32,
    costume_override_count: u32,
    blob_offset: usize,
    blob_len: usize,
}

fn u32_at(d: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([d[at], d[at + 1], d[at + 2], d[at + 3]])
}
fn u16_at(d: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([d[at], d[at + 1]])
}
fn i16_at(d: &[u8], at: usize) -> i16 {
    u16_at(d, at) as i16
}
fn extent_at(d: &[u8], at: usize) -> Extent {
    Extent {
        top: i16_at(d, at),
        bottom: i16_at(d, at + 2),
        right: i16_at(d, at + 4),
        left: i16_at(d, at + 6),
    }
}
fn f32_at(d: &[u8], at: usize) -> f32 {
    f32::from_bits(u32_at(d, at))
}

impl<'a> Pack<'a> {
    /// Validates the header and table bounds. Cheap: no per-asset work.
    pub fn open(data: &'a [u8]) -> Result<Pack<'a>, PackError> {
        if data.len() < Header::SIZE {
            return Err(PackError::TooSmall);
        }
        let magic = u32_at(data, 0);
        if magic != MAGIC {
            return Err(PackError::BadMagic(magic));
        }
        let version = u32_at(data, 4);
        if version != VERSION {
            return Err(PackError::BadVersion(version));
        }

        let mesh_count = u32_at(data, 8);
        let prim_count = u32_at(data, 12);
        let texture_count = u32_at(data, 16);
        let blob_offset = u32_at(data, 20) as usize;
        let blob_len = u32_at(data, 24) as usize;
        let object_count = u32_at(data, 28);
        let node_count = u32_at(data, 32);
        let stage_count = u32_at(data, 36);
        let line_count = u32_at(data, 40);
        let coll_vertex_count = u32_at(data, 44);
        let point_count = u32_at(data, 48);
        let fighter_count = u32_at(data, 52);
        let anim_count = u32_at(data, 56);
        let anim_joint_count = u32_at(data, 60);
        let mat_anim_count = u32_at(data, 64);
        let mat_anim_palette_count = u32_at(data, 68);
        let costume_override_count = u32_at(data, 72);

        let tables_end = Header::SIZE
            + mesh_count as usize * MeshDesc::SIZE
            + prim_count as usize * PrimDesc::SIZE
            + texture_count as usize * TextureDesc::SIZE
            + object_count as usize * ObjectDesc::SIZE
            + node_count as usize * NodeDesc::SIZE
            + stage_count as usize * StageDesc::SIZE
            + line_count as usize * LineDesc::SIZE
            + coll_vertex_count as usize * CollisionVertex::SIZE
            + point_count as usize * MapPoint::SIZE
            + fighter_count as usize * FighterDesc::SIZE
            + anim_count as usize * AnimDesc::SIZE
            + anim_joint_count as usize * AnimJoint::SIZE
            + mat_anim_count as usize * MatAnimDesc::SIZE
            + mat_anim_palette_count as usize * MatAnimPalette::SIZE
            + costume_override_count as usize * CostumeOverride::SIZE;

        if blob_offset < tables_end || blob_offset.saturating_add(blob_len) > data.len() {
            return Err(PackError::OutOfBounds);
        }

        Ok(Pack {
            data,
            mesh_count,
            prim_count,
            texture_count,
            object_count,
            node_count,
            stage_count,
            line_count,
            coll_vertex_count,
            point_count,
            fighter_count,
            anim_count,
            anim_joint_count,
            mat_anim_count,
            mat_anim_palette_count,
            costume_override_count,
            blob_offset,
            blob_len,
        })
    }

    pub fn mesh_count(&self) -> u32 {
        self.mesh_count
    }
    pub fn texture_count(&self) -> u32 {
        self.texture_count
    }
    pub fn object_count(&self) -> u32 {
        self.object_count
    }
    pub fn node_count(&self) -> u32 {
        self.node_count
    }
    pub fn stage_count(&self) -> u32 {
        self.stage_count
    }
    /// Collision polylines across every stage.
    pub fn line_count(&self) -> u32 {
        self.line_count
    }
    pub fn coll_vertex_count(&self) -> u32 {
        self.coll_vertex_count
    }
    pub fn point_count(&self) -> u32 {
        self.point_count
    }
    /// Characters whose constants are in the pack.
    pub fn fighter_count(&self) -> u32 {
        self.fighter_count
    }
    /// Animations in the pack: 189 from a full ROM, seven per fighter.
    pub fn anim_count(&self) -> u32 {
        self.anim_count
    }
    pub fn anim_joint_count(&self) -> u32 {
        self.anim_joint_count
    }
    /// Animated palette tables: one per `PaletteID`-cycling material
    /// animation script found (RE-089/RE-090/RE-091).
    pub fn mat_anim_count(&self) -> u32 {
        self.mat_anim_count
    }
    pub fn mat_anim_palette_count(&self) -> u32 {
        self.mat_anim_palette_count
    }
    /// Per-(node, costume) mesh substitutions (RE-098).
    pub fn costume_override_count(&self) -> u32 {
        self.costume_override_count
    }
    /// Total primitives: one GE draw call each, so this is the pack's draw-call
    /// budget if every mesh were on screen at once.
    pub fn prim_count(&self) -> u32 {
        self.prim_count
    }

    fn mesh_table(&self) -> usize {
        Header::SIZE
    }
    fn prim_table(&self) -> usize {
        self.mesh_table() + self.mesh_count as usize * MeshDesc::SIZE
    }
    fn texture_table(&self) -> usize {
        self.prim_table() + self.prim_count as usize * PrimDesc::SIZE
    }
    fn object_table(&self) -> usize {
        self.texture_table() + self.texture_count as usize * TextureDesc::SIZE
    }
    fn node_table(&self) -> usize {
        self.object_table() + self.object_count as usize * ObjectDesc::SIZE
    }
    fn stage_table(&self) -> usize {
        self.node_table() + self.node_count as usize * NodeDesc::SIZE
    }
    fn anim_table(&self) -> usize {
        self.fighter_table() + self.fighter_count as usize * FighterDesc::SIZE
    }
    fn anim_joint_table(&self) -> usize {
        self.anim_table() + self.anim_count as usize * AnimDesc::SIZE
    }
    fn mat_anim_table(&self) -> usize {
        self.anim_joint_table() + self.anim_joint_count as usize * AnimJoint::SIZE
    }
    fn mat_anim_palette_table(&self) -> usize {
        self.mat_anim_table() + self.mat_anim_count as usize * MatAnimDesc::SIZE
    }
    fn costume_override_table(&self) -> usize {
        self.mat_anim_palette_table() + self.mat_anim_palette_count as usize * MatAnimPalette::SIZE
    }
    fn line_table(&self) -> usize {
        self.stage_table() + self.stage_count as usize * StageDesc::SIZE
    }
    fn coll_vertex_table(&self) -> usize {
        self.line_table() + self.line_count as usize * LineDesc::SIZE
    }
    fn point_table(&self) -> usize {
        self.coll_vertex_table() + self.coll_vertex_count as usize * CollisionVertex::SIZE
    }
    fn fighter_table(&self) -> usize {
        self.point_table() + self.point_count as usize * MapPoint::SIZE
    }

    /// One character's constants, by `FTKind` ordinal.
    pub fn fighter(&self, i: u32) -> Option<FighterDesc> {
        if i >= self.fighter_count {
            return None;
        }
        let at = self.fighter_table() + i as usize * FighterDesc::SIZE;
        let mut s = [0.0f32; SCALARS];
        for (k, v) in s.iter_mut().enumerate() {
            *v = f32_at(self.data, at + 20 + k * 4);
        }
        Some(fighter_from_parts(
            u32_at(self.data, at),
            u32_at(self.data, at + 4),
            u32_at(self.data, at + 8),
            u32_at(self.data, at + 12) as i32,
            u32_at(self.data, at + 16),
            s,
        ))
    }

    /// One animation, by index into the animation table.
    pub fn anim(&self, i: u32) -> Option<AnimDesc> {
        if i >= self.anim_count {
            return None;
        }
        let at = self.anim_table() + i as usize * AnimDesc::SIZE;
        Some(AnimDesc {
            fighter: u32_at(self.data, at),
            slot: u32_at(self.data, at + 4),
            source_file: u32_at(self.data, at + 8),
            frames: u32_at(self.data, at + 12),
            first_joint: u32_at(self.data, at + 16),
            joint_count: u32_at(self.data, at + 20),
            script_offset: u32_at(self.data, at + 24),
            script_len: u32_at(self.data, at + 28),
        })
    }

    /// The animation a fighter plays for a movement status, if the pack has it.
    ///
    /// The table is dense and ordered `(fighter, slot)`, so this is arithmetic
    /// rather than a search — but the entry's own `fighter` and `slot` are
    /// checked, so a pack built some other way cannot quietly return the wrong
    /// animation.
    pub fn fighter_anim(&self, fighter: u32, slot: u32) -> Option<AnimDesc> {
        let slots = crate::anim::SLOT_COUNT as u32;
        if slot >= slots {
            return None;
        }
        let a = self.anim(fighter * slots + slot)?;
        (a.fighter == fighter && a.slot == slot).then_some(a)
    }

    /// A stage's joint animation, if it has one.
    ///
    /// Scanned rather than indexed: fighter animations occupy a dense block
    /// keyed by `fighter * SLOT_COUNT + slot`, and stage entries are appended
    /// after it, so there is no arithmetic that finds them.
    pub fn stage_anim(&self, stage: u32) -> Option<AnimDesc> {
        (0..self.anim_count)
            .filter_map(|i| self.anim(i))
            .find(|a| a.fighter == AnimDesc::STAGE && a.slot == stage)
    }

    /// One joint entry, by absolute index.
    pub fn anim_joint(&self, i: u32) -> Option<AnimJoint> {
        if i >= self.anim_joint_count {
            return None;
        }
        let at = self.anim_joint_table() + i as usize * AnimJoint::SIZE;
        Some(AnimJoint {
            script: u32_at(self.data, at),
            node: u32_at(self.data, at + 4),
        })
    }

    /// The bytes of an animation's scripts, as `figatree` wants them.
    ///
    /// [`AnimJoint::script`] offsets index into this slice, so the caller can
    /// hand the two straight to [`crate::figatree::JointAnim::start`].
    pub fn anim_script(&self, a: &AnimDesc) -> Option<&'a [u8]> {
        self.blob(a.script_offset, a.script_len as usize)
    }

    /// One animated palette table, by index into [`Pack::mat_anim_count`].
    pub fn mat_anim(&self, i: u32) -> Option<MatAnimDesc> {
        if i >= self.mat_anim_count {
            return None;
        }
        let at = self.mat_anim_table() + i as usize * MatAnimDesc::SIZE;
        Some(MatAnimDesc {
            file_offset: u32_at(self.data, at),
            file_len: u32_at(self.data, at + 4),
            script: u32_at(self.data, at + 8),
            first_palette: u32_at(self.data, at + 12),
            palette_count: u32_at(self.data, at + 16),
            source_file: u32_at(self.data, at + 20),
            source_offset: u32_at(self.data, at + 24),
        })
    }

    /// The bytes of a `MatAnimDesc`'s *whole source file* — [`MatAnimDesc::
    /// script`] is a byte offset into this slice, matching how
    /// [`crate::matanim::MaterialJoint::tick`] wants to be called.
    pub fn mat_anim_file(&self, a: &MatAnimDesc) -> Option<&'a [u8]> {
        self.blob(a.file_offset, a.file_len as usize)
    }

    /// One resolved palette variant, by absolute index (see
    /// [`MatAnimDesc::first_palette`]/[`MatAnimDesc::palette_count`]).
    pub fn mat_anim_palette(&self, i: u32) -> Option<MatAnimPalette> {
        if i >= self.mat_anim_palette_count {
            return None;
        }
        let at = self.mat_anim_palette_table() + i as usize * MatAnimPalette::SIZE;
        Some(MatAnimPalette {
            palette_offset: u32_at(self.data, at),
            palette_len: u32_at(self.data, at + 4),
        })
    }

    /// A resolved palette variant's own CLUT bytes, ready for
    /// `sceGuClutLoad`.
    pub fn mat_anim_palette_data(&self, p: &MatAnimPalette) -> Option<&'a [u8]> {
        self.blob(p.palette_offset, p.palette_len as usize * 4)
    }

    /// One costume-override entry, by index (RE-098).
    pub fn costume_override(&self, i: u32) -> Option<CostumeOverride> {
        if i >= self.costume_override_count {
            return None;
        }
        let at = self.costume_override_table() + i as usize * CostumeOverride::SIZE;
        Some(CostumeOverride {
            node: u32_at(self.data, at),
            costume: u32_at(self.data, at + 4),
            mesh: u32_at(self.data, at + 8),
        })
    }

    /// The mesh a *global* node index should draw at a given costume
    /// (RE-098): the node's own substitute mesh if one was baked for that
    /// costume, or `None` when the node draws identically at every costume
    /// (the common case) or `costume` is 0 (never stored — see
    /// [`PackWriter::add_costume_override`]).
    ///
    /// Binary search: [`PackWriter::finish`] sorts the table by
    /// `(node, costume)` before writing it, so this needs no linear scan even
    /// though the table itself is unindexed by object.
    pub fn costume_mesh(&self, node: u32, costume: u32) -> Option<u32> {
        if costume == 0 || self.costume_override_count == 0 {
            return None;
        }
        let mut lo = 0u32;
        let mut hi = self.costume_override_count;
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let o = self.costume_override(mid)?;
            match (o.node, o.costume).cmp(&(node, costume)) {
                core::cmp::Ordering::Less => lo = mid + 1,
                core::cmp::Ordering::Greater => hi = mid,
                core::cmp::Ordering::Equal => return Some(o.mesh),
            }
        }
        None
    }

    /// How many distinct costumes `object` has (RE-098): one plus the
    /// largest `costume` any [`CostumeOverride`] names for one of its nodes,
    /// or `1` (costume 0 only, the ordinary case) if it has none. Meant for
    /// a debug viewer to know how far a costume-cycle control should wrap,
    /// not a per-frame hot path — it walks the (small) run of overrides that
    /// fall within this object's own node range, found by binary-searching
    /// the table's sorted-by-node ordering for its start.
    pub fn object_costume_count(&self, object: &ObjectDesc) -> u32 {
        if self.costume_override_count == 0 {
            return 1;
        }
        let node_end = object.first_node + object.node_count;
        // First index whose node is >= first_node: the table is sorted by
        // (node, costume), so this is the run's own start.
        let mut lo = 0u32;
        let mut hi = self.costume_override_count;
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let Some(o) = self.costume_override(mid) else {
                return 1;
            };
            if o.node < object.first_node {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        let mut max_costume = 0u32;
        let mut i = lo;
        while i < self.costume_override_count {
            let Some(o) = self.costume_override(i) else {
                break;
            };
            if o.node >= node_end {
                break;
            }
            max_costume = max_costume.max(o.costume);
            i += 1;
        }
        max_costume + 1
    }

    pub fn object(&self, i: u32) -> Option<ObjectDesc> {
        if i >= self.object_count {
            return None;
        }
        let at = self.object_table() + i as usize * ObjectDesc::SIZE;
        Some(ObjectDesc {
            first_node: u32_at(self.data, at),
            node_count: u32_at(self.data, at + 4),
            source_file: u32_at(self.data, at + 8),
            source_offset: u32_at(self.data, at + 12),
        })
    }

    pub fn node(&self, i: u32) -> Option<NodeDesc> {
        if i >= self.node_count {
            return None;
        }
        let at = self.node_table() + i as usize * NodeDesc::SIZE;
        let mut world = [0f32; 16];
        for (k, w) in world.iter_mut().enumerate() {
            *w = f32_at(self.data, at + 8 + k * 4);
        }
        let vec3 = |base: usize| {
            [
                f32_at(self.data, base),
                f32_at(self.data, base + 4),
                f32_at(self.data, base + 8),
            ]
        };
        Some(NodeDesc {
            mesh: u32_at(self.data, at),
            parent: u32_at(self.data, at + 4),
            world,
            rest_translate: vec3(at + 72),
            rest_rotate: vec3(at + 84),
            rest_scale: vec3(at + 96),
            flags: u32_at(self.data, at + 108),
        })
    }

    pub fn mesh(&self, i: u32) -> Option<MeshDesc> {
        if i >= self.mesh_count {
            return None;
        }
        let at = self.mesh_table() + i as usize * MeshDesc::SIZE;
        Some(MeshDesc {
            vertex_offset: u32_at(self.data, at),
            vertex_count: u32_at(self.data, at + 4),
            first_prim: u32_at(self.data, at + 8),
            prim_count: u32_at(self.data, at + 12),
            source_file: u32_at(self.data, at + 16),
            source_offset: u32_at(self.data, at + 20),
        })
    }

    pub fn prim(&self, i: u32) -> Option<PrimDesc> {
        if i >= self.prim_count {
            return None;
        }
        let at = self.prim_table() + i as usize * PrimDesc::SIZE;
        Some(PrimDesc {
            texture: u32_at(self.data, at),
            flags: u32_at(self.data, at + 4),
            prim_color: u32_at(self.data, at + 8),
            env_color: u32_at(self.data, at + 12),
            index_offset: u32_at(self.data, at + 16),
            index_count: u32_at(self.data, at + 20),
            texture_blend_base: u32_at(self.data, at + 24),
            texture_blend_target: u32_at(self.data, at + 28),
            flat_color: u32_at(self.data, at + 32),
        })
    }

    pub fn texture(&self, i: u32) -> Option<TextureDesc> {
        if i >= self.texture_count {
            return None;
        }
        let at = self.texture_table() + i as usize * TextureDesc::SIZE;
        Some(TextureDesc {
            width: u16_at(self.data, at),
            height: u16_at(self.data, at + 2),
            stride: u16_at(self.data, at + 4),
            psm: self.data[at + 6],
            swizzled: self.data[at + 7],
            data_offset: u32_at(self.data, at + 8),
            data_len: u32_at(self.data, at + 12),
            palette_offset: u32_at(self.data, at + 16),
            palette_len: u32_at(self.data, at + 20),
            levels: u32_at(self.data, at + 24).max(1),
            mat_anim: u32_at(self.data, at + 28),
            role: u32_at(self.data, at + 32),
            wrap: self.data[at + 36],
        })
    }

    pub fn stage(&self, i: u32) -> Option<StageDesc> {
        if i >= self.stage_count {
            return None;
        }
        let at = self.stage_table() + i as usize * StageDesc::SIZE;
        let mut layers = [StageDesc::NO_LAYER; 4];
        for (k, l) in layers.iter_mut().enumerate() {
            *l = u32_at(self.data, at + 16 + k * 4);
        }
        Some(StageDesc {
            first_line: u32_at(self.data, at),
            line_count: u32_at(self.data, at + 4),
            first_point: u32_at(self.data, at + 8),
            point_count: u32_at(self.data, at + 12),
            layers,
            camera: extent_at(self.data, at + 32),
            bounds: extent_at(self.data, at + 40),
            bgm_id: u32_at(self.data, at + 48),
            source_file: u32_at(self.data, at + 52),
            source_offset: u32_at(self.data, at + 56),
            _pad: 0,
        })
    }

    pub fn line(&self, i: u32) -> Option<LineDesc> {
        if i >= self.line_count {
            return None;
        }
        let at = self.line_table() + i as usize * LineDesc::SIZE;
        Some(LineDesc {
            first_vertex: u32_at(self.data, at),
            vertex_count: u16_at(self.data, at + 4),
            kind: u16_at(self.data, at + 6),
            yakumono: u16_at(self.data, at + 8),
            id: u16_at(self.data, at + 10),
        })
    }

    pub fn coll_vertex(&self, i: u32) -> Option<CollisionVertex> {
        if i >= self.coll_vertex_count {
            return None;
        }
        let at = self.coll_vertex_table() + i as usize * CollisionVertex::SIZE;
        Some(CollisionVertex {
            x: i16_at(self.data, at),
            y: i16_at(self.data, at + 2),
            flags: u16_at(self.data, at + 4),
            _pad: 0,
        })
    }

    pub fn map_point(&self, i: u32) -> Option<MapPoint> {
        if i >= self.point_count {
            return None;
        }
        let at = self.point_table() + i as usize * MapPoint::SIZE;
        Some(MapPoint {
            kind: u16_at(self.data, at),
            x: i16_at(self.data, at + 2),
            y: i16_at(self.data, at + 4),
            _pad: 0,
        })
    }

    /// A stage's collision lines, in table order.
    pub fn stage_lines(&self, s: &StageDesc) -> impl Iterator<Item = LineDesc> + '_ {
        let range = s.first_line..s.first_line + s.line_count;
        range.filter_map(move |i| self.line(i))
    }

    /// The points of one collision polyline.
    pub fn line_vertices(&self, l: &LineDesc) -> impl Iterator<Item = CollisionVertex> + '_ {
        let range = l.first_vertex..l.first_vertex + l.vertex_count as u32;
        range.filter_map(move |i| self.coll_vertex(i))
    }

    /// A stage's map points, in table order.
    pub fn stage_points(&self, s: &StageDesc) -> impl Iterator<Item = MapPoint> + '_ {
        let range = s.first_point..s.first_point + s.point_count;
        range.filter_map(move |i| self.map_point(i))
    }

    /// Where player `n` starts. `MPMapObjKind` 0..=3 are the four spawns.
    pub fn spawn(&self, s: &StageDesc, player: u16) -> Option<MapPoint> {
        self.stage_points(s).find(|p| p.kind == player)
    }

    /// A slice of the blob region. Bounds-checked once, here, so the render
    /// loop can be free of checks.
    pub fn blob(&self, offset: u32, len: usize) -> Option<&'a [u8]> {
        let at = self.blob_offset.checked_add(offset as usize)?;
        if offset as usize + len > self.blob_len {
            return None;
        }
        self.data.get(at..at + len)
    }

    /// Raw vertex bytes for a mesh, ready to hand to the GE.
    pub fn vertices(&self, m: &MeshDesc) -> Option<&'a [u8]> {
        self.blob(m.vertex_offset, m.vertex_count as usize * VERTEX_SIZE)
    }

    /// Raw index bytes for a primitive.
    pub fn indices(&self, p: &PrimDesc) -> Option<&'a [u8]> {
        self.blob(p.index_offset, p.index_count as usize * 2)
    }

    pub fn texture_data(&self, t: &TextureDesc) -> Option<&'a [u8]> {
        self.blob(t.data_offset, t.data_len as usize)
    }

    pub fn palette_data(&self, t: &TextureDesc) -> Option<&'a [u8]> {
        if t.palette_len == 0 {
            return None;
        }
        self.blob(t.palette_offset, t.palette_len as usize * 4)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::{Mesh, MeshMaterial, MeshVertex, Primitive};
    use crate::psp_texture::{Psm, PspTexture};

    fn sample_mesh() -> Mesh {
        Mesh {
            vertices: alloc::vec![
                MeshVertex {
                    pos: [1, 2, 3],
                    uv: [32, 64],
                    rgba: [0x11, 0x22, 0x33, 0x44],
                },
                MeshVertex {
                    pos: [4, 5, 6],
                    uv: [0, 0],
                    rgba: [255, 255, 255, 255],
                },
                MeshVertex {
                    pos: [7, 8, 9],
                    uv: [1, 2],
                    rgba: [0, 0, 0, 255],
                },
            ],
            primitives: alloc::vec![Primitive {
                material: MeshMaterial {
                    cull_back: true,
                    lit: true,
                    prim_color: Some([1, 2, 3, 4]),
                    ..Default::default()
                },
                indices: alloc::vec![0, 1, 2],
            }],
        }
    }

    #[test]
    fn round_trips_a_mesh() {
        let mut w = PackWriter::new();
        w.add_mesh(&sample_mesh(), 42, 0x1000, |_| None);
        let bytes = w.finish();

        let pack = Pack::open(&bytes).unwrap();
        assert_eq!(pack.mesh_count(), 1);

        let m = pack.mesh(0).unwrap();
        assert_eq!(m.vertex_count, 3);
        assert_eq!(m.prim_count, 1);
        assert_eq!(m.source_file, 42);
        assert_eq!(m.source_offset, 0x1000);

        let p = pack.prim(m.first_prim).unwrap();
        assert_eq!(p.index_count, 3);
        assert_eq!(p.texture, PrimDesc::NO_TEXTURE);
        assert!(p.flags & flags::CULL_BACK != 0);
        assert!(p.flags & flags::LIT != 0);
        assert!(p.flags & flags::CULL_FRONT == 0);

        let idx = pack.indices(&p).unwrap();
        assert_eq!(idx, &[0, 0, 1, 0, 2, 0]); // little-endian u16
    }

    /// The declared stride must match the struct the GE is told to read.
    #[test]
    fn vertex_size_matches_the_struct() {
        assert_eq!(VERTEX_SIZE, core::mem::size_of::<PackedVertex>());
    }

    #[test]
    fn vertices_are_packed_at_the_declared_stride() {
        // Unlit, so the colour bytes pass through untouched and this test
        // isolates stride rather than also depending on the lighting bake.
        let mut m = sample_mesh();
        m.primitives[0].material.lit = false;
        // `sample_mesh`'s `prim_color` is a `combiner_shade_scale` result
        // (RE-106), not a literal colour: it would multiply into every
        // vertex here too, defeating this test's own point.
        m.primitives[0].material.prim_color = None;
        let mut w = PackWriter::new();
        w.add_mesh(&m, 0, 0, |_| None);
        let bytes = w.finish();
        let pack = Pack::open(&bytes).unwrap();
        let m = pack.mesh(0).unwrap();

        let v = pack.vertices(&m).unwrap();
        assert_eq!(v.len(), 3 * VERTEX_SIZE);

        // First vertex: u=32, v=64, colour ABGR, then x,y,z.
        assert_eq!(i16::from_le_bytes([v[0], v[1]]), 32);
        assert_eq!(i16::from_le_bytes([v[2], v[3]]), 64);
        assert_eq!(u32_at(v, 4), 0x4433_2211);
        assert_eq!(i16::from_le_bytes([v[8], v[9]]), 1);
        assert_eq!(i16::from_le_bytes([v[10], v[11]]), 2);
        assert_eq!(i16::from_le_bytes([v[12], v[13]]), 3);

        // Check the SECOND vertex too. Only checking the first cannot detect a
        // wrong stride, which is exactly the bug this test missed once already.
        let s = VERTEX_SIZE;
        assert_eq!(i16::from_le_bytes([v[s + 8], v[s + 9]]), 4, "vertex 1 x");
        assert_eq!(i16::from_le_bytes([v[s + 10], v[s + 11]]), 5, "vertex 1 y");
        assert_eq!(i16::from_le_bytes([v[s + 12], v[s + 13]]), 6, "vertex 1 z");
        assert_eq!(u32_at(v, s + 4), 0xFFFF_FFFF, "vertex 1 colour");

        // And the third, so an off-by-one stride cannot slip through either.
        let t = 2 * VERTEX_SIZE;
        assert_eq!(i16::from_le_bytes([v[t + 8], v[t + 9]]), 7, "vertex 2 x");
    }

    #[test]
    fn every_ge_blob_is_sixteen_byte_aligned() {
        // Unaligned vertex or texture data renders garbage on the GE with no
        // error, so this is the invariant most worth asserting.
        let mut w = PackWriter::new();
        let tex = PspTexture {
            width: 32,
            height: 8,
            stride: 32,
            format: Psm::PsmT4,
            data: alloc::vec![0xABu8; 128],
            swizzled: true,
            palette: alloc::vec![0xFF00_00FFu32; 16],
            levels: 1,
        };
        w.add_texture(&tex, false, false);
        w.add_mesh(&sample_mesh(), 0, 0, |_| Some(0));
        let bytes = w.finish();

        let pack = Pack::open(&bytes).unwrap();
        assert!(
            pack.blob_offset.is_multiple_of(ALIGN),
            "blob region must be aligned"
        );

        let m = pack.mesh(0).unwrap();
        assert!(m.vertex_offset.is_multiple_of(ALIGN as u32));
        let p = pack.prim(0).unwrap();
        assert!(p.index_offset.is_multiple_of(ALIGN as u32));
        let t = pack.texture(0).unwrap();
        assert!(t.data_offset.is_multiple_of(ALIGN as u32));
        assert!(t.palette_offset.is_multiple_of(ALIGN as u32));

        // And the absolute positions within the file, which is what actually
        // reaches the hardware.
        assert!((pack.blob_offset + m.vertex_offset as usize).is_multiple_of(ALIGN));
        assert!((pack.blob_offset + t.data_offset as usize).is_multiple_of(ALIGN));
    }

    /// Descriptor sizes must match exactly what the writer emits. A mismatch
    /// silently misreads every entry after the first.
    #[test]
    fn descriptor_sizes_match_the_serialised_layout() {
        let mut w = PackWriter::new();
        let tex = |n: u8| PspTexture {
            width: 16 + n as u32,
            height: 8,
            stride: 32,
            format: Psm::PsmT4,
            data: alloc::vec![n; 128],
            swizzled: false,
            palette: alloc::vec![0u32; 16],
            levels: 1,
        };
        for n in 0..4 {
            w.add_texture(&tex(n), false, false);
        }
        w.add_mesh(&sample_mesh(), 1, 2, |_| Some(3));
        w.add_object(&chain_graph(3), 11, |_| None, &[]);
        let (ground, map) = sample_stage();
        w.add_stage(&ground, Some(&map), |_, _| None);
        let bytes = w.finish();
        let pack = Pack::open(&bytes).unwrap();

        // Every table is populated, so a wrong `SIZE` anywhere moves the blob.
        let expected_tables = Header::SIZE
            + pack.mesh_count as usize * MeshDesc::SIZE
            + pack.prim_count as usize * PrimDesc::SIZE
            + pack.texture_count as usize * TextureDesc::SIZE
            + pack.object_count as usize * ObjectDesc::SIZE
            + pack.node_count as usize * NodeDesc::SIZE
            + pack.stage_count as usize * StageDesc::SIZE
            + pack.line_count as usize * LineDesc::SIZE
            + pack.coll_vertex_count as usize * CollisionVertex::SIZE
            + pack.point_count as usize * MapPoint::SIZE;
        // Exactly, not merely "at least": a `SIZE` that overstates its
        // descriptor still satisfies `>=` while every reader offset is wrong.
        assert_eq!(
            pack.blob_offset,
            align_up(expected_tables),
            "a descriptor SIZE disagrees with what the writer emitted"
        );

        // Every texture must read back with the width it was written with --
        // checking only index 0 cannot detect a wrong stride.
        for n in 0..4u32 {
            let t = pack.texture(n).expect("texture present");
            assert_eq!(t.width, 16 + n as u16, "texture {n} misread");
            assert_eq!(t.palette_len, 16, "texture {n} palette misread");
            assert_eq!(
                pack.texture_data(&t).unwrap()[0],
                n as u8,
                "texture {n} points at the wrong bytes"
            );
        }
    }

    /// Builds a graph of `n` nodes: a root plus a descending chain, each node
    /// translated 100 units along +x and carrying display list `0x100 * i`.
    fn chain_graph(n: usize) -> crate::scene::SceneGraph {
        use crate::scene::{DObjDesc, DObjNode, SceneGraph};
        let nodes = (0..n)
            .map(|i| DObjNode {
                desc: DObjDesc {
                    id: i as u32,
                    dl: Some(0x100 * i as u32),
                    translate: [100.0, 0.0, 0.0],
                    rotate: [0.0; 3],
                    scale: [1.0; 3],
                },
                parent: i.checked_sub(1),
            })
            .collect();
        SceneGraph {
            offset: 0x2000,
            nodes,
        }
    }

    #[test]
    fn objects_and_nodes_round_trip() {
        let mut w = PackWriter::new();
        // Two objects, so a wrong ObjectDesc::SIZE shows up as a misread second
        // entry rather than passing by luck -- the trap that made every texture
        // after the first read from the wrong offset.
        // Node index -> mesh index, the mapping the packer supplies.
        w.add_object(&chain_graph(3), 11, |n| Some(n as u32), &[]);
        w.add_object(&chain_graph(2), 22, |_| None, &[]);

        let bytes = w.finish();
        let pack = Pack::open(&bytes).unwrap();
        assert_eq!(pack.object_count(), 2);
        assert_eq!(pack.node_count(), 5);

        let a = pack.object(0).unwrap();
        let b = pack.object(1).unwrap();
        assert_eq!((a.first_node, a.node_count, a.source_file), (0, 3, 11));
        assert_eq!((b.first_node, b.node_count, b.source_file), (3, 2, 22));
        assert_eq!(a.source_offset, 0x2000);

        // Parent indices are absolute, so object 1's root must still be a root
        // and its child must point at node 3, not node 0.
        assert_eq!(pack.node(0).unwrap().parent, NodeDesc::NO_PARENT);
        assert_eq!(pack.node(1).unwrap().parent, 0);
        assert_eq!(pack.node(2).unwrap().parent, 1);
        assert_eq!(pack.node(3).unwrap().parent, NodeDesc::NO_PARENT);
        assert_eq!(pack.node(4).unwrap().parent, 3);

        // Nodes without a resolvable display list become pure transforms.
        assert_eq!(pack.node(2).unwrap().mesh, 2);
        assert_eq!(pack.node(4).unwrap().mesh, NodeDesc::NO_MESH);
    }

    #[test]
    fn costume_mesh_falls_back_to_the_nodes_own_mesh_when_untouched() {
        // RE-098: most nodes of a costume-bearing object draw identically at
        // every costume, so `costume_mesh` returning `None` (fall back to the
        // node's own baked `NodeDesc::mesh`) has to be the common case, not
        // an error path.
        let mut w = PackWriter::new();
        let object = w.add_object(&chain_graph(3), 11, |n| Some(n as u32), &[]);
        assert_eq!(w.object(object).unwrap().first_node, 0);
        let bytes = w.finish();
        let pack = Pack::open(&bytes).unwrap();

        assert_eq!(pack.costume_override_count(), 0);
        assert_eq!(pack.costume_mesh(1, 2), None);
        // Costume 0 is never stored -- it is exactly the node's own mesh --
        // so asking for it must also miss, even on a pack with real entries.
        assert_eq!(pack.costume_mesh(1, 0), None);
    }

    #[test]
    fn costume_mesh_returns_the_overriding_meshes_own_index() {
        let mut w = PackWriter::new();
        let object = w.add_object(&chain_graph(3), 11, |n| Some(n as u32), &[]);
        let first_node = w.object(object).unwrap().first_node;
        // Added out of (node, costume) order on purpose -- `finish` has to
        // sort before writing, not merely accept already-sorted input. Node
        // 1 changes at costumes 1 and 2; node 2 never changes -- the real
        // archive shape (RE-098), not every node overridden uniformly.
        w.add_costume_override(first_node + 1, 2, 91);
        w.add_costume_override(first_node, 1, 80);
        w.add_costume_override(first_node + 1, 1, 90);
        let bytes = w.finish();
        let pack = Pack::open(&bytes).unwrap();

        assert_eq!(pack.costume_override_count(), 3);
        assert_eq!(pack.costume_mesh(first_node, 1), Some(80));
        assert_eq!(pack.costume_mesh(first_node + 1, 1), Some(90));
        assert_eq!(pack.costume_mesh(first_node + 1, 2), Some(91));
        // Verified capable of failing: a lookup for a node/costume pair that
        // was never added must miss, not return a neighbouring entry -- the
        // binary search's exact-match `Ordering::Equal` arm is what this
        // pins, not just "the table has two rows now".
        assert_eq!(pack.costume_mesh(first_node + 1, 3), None);
        assert_eq!(pack.costume_mesh(first_node + 2, 1), None);
        assert_eq!(pack.costume_mesh(first_node, 2), None);
        // Costume 0 must never be readable back even if somehow present in
        // the raw table -- `costume_mesh` short-circuits on it before ever
        // reaching the search.
        assert_eq!(pack.costume_mesh(first_node + 1, 0), None);
    }

    #[test]
    fn object_costume_count_finds_the_highest_costume_within_its_own_node_range() {
        let mut w = PackWriter::new();
        // Two objects, so a wrong node-range bound could leak the second
        // object's overrides into the first's count instead of stopping at
        // its own `node_count`.
        let a = w.add_object(&chain_graph(3), 11, |n| Some(n as u32), &[]);
        let b = w.add_object(&chain_graph(2), 22, |n| Some(n as u32), &[]);
        let a_first = w.object(a).unwrap().first_node;
        let b_first = w.object(b).unwrap().first_node;
        w.add_costume_override(a_first + 1, 1, 90);
        w.add_costume_override(a_first + 2, 3, 91);
        w.add_costume_override(b_first, 1, 92);
        let bytes = w.finish();
        let pack = Pack::open(&bytes).unwrap();

        assert_eq!(
            pack.object_costume_count(&pack.object(a).unwrap()),
            4,
            "highest costume named for object a is 3, so it has costumes 0..=3"
        );
        assert_eq!(
            pack.object_costume_count(&pack.object(b).unwrap()),
            2,
            "object b's own override must not see object a's costume-3 entry"
        );
    }

    #[test]
    fn object_costume_count_is_one_when_the_object_has_no_overrides() {
        let mut w = PackWriter::new();
        let a = w.add_object(&chain_graph(3), 11, |n| Some(n as u32), &[]);
        // Another object *does* have overrides, so this pins that an empty
        // table isn't the only way to read back "just costume 0".
        let b = w.add_object(&chain_graph(2), 22, |n| Some(n as u32), &[]);
        let b_first = w.object(b).unwrap().first_node;
        w.add_costume_override(b_first, 5, 90);
        let bytes = w.finish();
        let pack = Pack::open(&bytes).unwrap();

        assert_eq!(pack.object_costume_count(&pack.object(a).unwrap()), 1);
    }

    #[test]
    fn node_transforms_accumulate_and_are_scaled_into_vertex_space() {
        let mut w = PackWriter::new();
        w.add_object(&chain_graph(3), 0, |_| None, &[]);
        let bytes = w.finish();
        let pack = Pack::open(&bytes).unwrap();

        // Each link adds 100 world units, and the translation column is divided
        // by MODEL_SCALE so it matches the i16 vertex positions the GE
        // normalises by the same factor (RE-020). Without this the hierarchy
        // would assemble 32768x too large relative to its own geometry.
        for (i, expect) in [100.0f32, 200.0, 300.0].into_iter().enumerate() {
            let n = pack.node(i as u32).unwrap();
            let got = n.world[12] * MODEL_SCALE;
            assert!((got - expect).abs() < 1e-2, "node {i}: {got} != {expect}");
        }
    }

    #[test]
    fn texture_round_trips_with_palette() {
        let mut w = PackWriter::new();
        let tex = PspTexture {
            width: 32,
            height: 8,
            stride: 32,
            format: Psm::PsmT4,
            data: alloc::vec![0xABu8; 128],
            swizzled: true,
            palette: alloc::vec![0xFF00_00FFu32; 16],
            levels: 1,
        };
        w.add_texture(&tex, false, false);
        let bytes = w.finish();

        let pack = Pack::open(&bytes).unwrap();
        let t = pack.texture(0).unwrap();
        assert_eq!((t.width, t.height, t.stride), (32, 8, 32));
        assert_eq!(t.psm, Psm::PsmT4 as u8);
        assert_eq!(t.swizzled, 1);
        assert_eq!(t.palette_len, 16);

        assert_eq!(pack.texture_data(&t).unwrap(), &[0xABu8; 128]);
        let pal = pack.palette_data(&t).unwrap();
        assert_eq!(pal.len(), 64);
        assert_eq!(u32_at(pal, 0), 0xFF00_00FF);
    }

    #[test]
    fn a_framebuffer_texture_round_trips_with_no_baked_bytes() {
        // RE-099/RE-100, VERSION 14: added after an ordinary texture, so a
        // wrong `TextureDesc::SIZE` (the exact bug the doc comment on
        // `TextureDesc::SIZE` describes) would misread this one's offset,
        // not just its own -- the same guard `texture_round_trips_with_palette`
        // exercises for `mat_anim`.
        let mut w = PackWriter::new();
        let tex = PspTexture {
            width: 32,
            height: 8,
            stride: 32,
            format: Psm::PsmT4,
            data: alloc::vec![0xABu8; 128],
            swizzled: true,
            palette: alloc::vec![0xFF00_00FFu32; 16],
            levels: 1,
        };
        w.add_texture(&tex, false, false);
        w.add_framebuffer_texture(300, 6);
        let bytes = w.finish();

        let pack = Pack::open(&bytes).unwrap();

        let normal = pack.texture(0).unwrap();
        assert_eq!(normal.role, TextureDesc::ROLE_NORMAL);

        let fb = pack.texture(1).unwrap();
        assert_eq!(fb.role, TextureDesc::ROLE_FRAMEBUFFER);
        assert_eq!((fb.width, fb.height), (300, 6));
        assert_eq!(fb.stride, 512, "stride must pad to a power of two like every other texture");
        assert_eq!(fb.data_len, 0, "a framebuffer entry has no baked bytes");
        assert_eq!(fb.palette_len, 0);
        assert_eq!(fb.mat_anim, TextureDesc::NO_ANIM);
    }

    #[test]
    fn lit_vertices_are_shaded_from_their_normal() {
        // A normal pointing straight at the light must be brightest; one facing
        // away must fall back to ambient, not go black.
        let toward = [
            (LIGHT_DIR[0] * 127.0) as i8 as u8,
            (LIGHT_DIR[1] * 127.0) as i8 as u8,
            (LIGHT_DIR[2] * 127.0) as i8 as u8,
            0xAB,
        ];
        let away = [
            (-LIGHT_DIR[0] * 127.0) as i8 as u8,
            (-LIGHT_DIR[1] * 127.0) as i8 as u8,
            (-LIGHT_DIR[2] * 127.0) as i8 as u8,
            0xAB,
        ];

        let bright = shade_normal(toward);
        let dark = shade_normal(away);

        assert!(bright[0] > 240, "facing the light should be near-white");
        assert_eq!(dark[0], (AMBIENT * 255.0) as u8, "away = ambient floor");
        assert!(dark[0] > 0, "must not be pure black");
        // Grey, and alpha preserved.
        assert_eq!(bright[0], bright[1]);
        assert_eq!(bright[1], bright[2]);
        assert_eq!(bright[3], 0xAB);
        assert_eq!(dark[3], 0xAB);
    }

    #[test]
    fn only_lit_primitives_get_their_vertices_shaded() {
        let mut m = sample_mesh();
        // Vertex 1 is white; unlit, it must survive untouched.
        m.primitives[0].material.lit = false;
        // Not a literal colour (RE-106); it would multiply into the "must
        // survive untouched" vertex below too.
        m.primitives[0].material.prim_color = None;
        let mut w = PackWriter::new();
        w.add_mesh(&m, 0, 0, |_| None);
        let bytes = w.finish();
        let pack = Pack::open(&bytes).unwrap();
        let v = pack.vertices(&pack.mesh(0).unwrap()).unwrap();
        assert_eq!(u32_at(v, VERTEX_SIZE + 4), 0xFFFF_FFFF, "unlit unchanged");

        // The same mesh marked lit must have that vertex replaced by a shade.
        let mut m = sample_mesh();
        m.primitives[0].material.lit = true;
        m.primitives[0].material.prim_color = None;
        let mut w = PackWriter::new();
        w.add_mesh(&m, 0, 0, |_| None);
        let bytes = w.finish();
        let pack = Pack::open(&bytes).unwrap();
        let v = pack.vertices(&pack.mesh(0).unwrap()).unwrap();
        assert_ne!(u32_at(v, VERTEX_SIZE + 4), 0xFFFF_FFFF, "lit was shaded");
    }

    #[test]
    fn unit_normals_are_distinguished_from_colours() {
        // A unit normal along an axis.
        assert!(looks_like_unit_normal([127, 0, 0, 255]));
        // A diagonal unit normal: 73² * 3 ≈ 15987.
        assert!(looks_like_unit_normal([73, 73, 73, 255]));
        // Opaque white is a colour, not a normal: (-1)² * 3 = 3.
        assert!(!looks_like_unit_normal([255, 255, 255, 255]));
        // Black likewise.
        assert!(!looks_like_unit_normal([0, 0, 0, 255]));
    }

    #[test]
    fn primitives_carrying_normals_are_shaded_even_without_the_geometry_mode() {
        // The display list never said G_LIGHTING, but the data is normals --
        // the common case, since the mode is usually inherited.
        let mut m = sample_mesh();
        m.primitives[0].material.lit = false;
        m.primitives[0].material.prim_color = None; // not a literal colour (RE-106)
        for v in &mut m.vertices {
            v.rgba = [127, 0, 0, 255];
        }

        let mut w = PackWriter::new();
        w.add_mesh(&m, 0, 0, |_| None);
        let bytes = w.finish();
        let pack = Pack::open(&bytes).unwrap();
        let v = pack.vertices(&pack.mesh(0).unwrap()).unwrap();

        // Shaded to grey rather than drawn as saturated red.
        let c = u32_at(v, 4);
        let (r, g, b) = (c as u8, (c >> 8) as u8, (c >> 16) as u8);
        assert_eq!(r, g, "shaded output must be grey");
        assert_eq!(g, b);
    }

    #[test]
    fn genuine_vertex_colours_are_left_alone() {
        let mut m = sample_mesh();
        m.primitives[0].material.lit = false;
        m.primitives[0].material.prim_color = None; // not a literal colour (RE-106)
        for v in &mut m.vertices {
            v.rgba = [255, 255, 255, 255]; // clearly a colour
        }
        let mut w = PackWriter::new();
        w.add_mesh(&m, 0, 0, |_| None);
        let bytes = w.finish();
        let pack = Pack::open(&bytes).unwrap();
        let v = pack.vertices(&pack.mesh(0).unwrap()).unwrap();
        assert_eq!(u32_at(v, 4), 0xFFFF_FFFF, "colours must survive untouched");
    }

    #[test]
    fn a_mixed_primitive_shades_only_its_normal_looking_vertices() {
        // RE-103: a fighter's decal highlights (literal colour) and lit body
        // (normals) routinely share one primitive's vertex buffer, nowhere
        // near the old 80% per-primitive majority either way -- Fox's,
        // Falcon's, and Kirby's models measured splits as even as 20/80.
        // Voting sent every vertex on the minority side to the wrong
        // interpretation; deciding per-vertex must not.
        let mut m = sample_mesh();
        m.primitives[0].material.lit = false;
        m.primitives[0].material.prim_color = None; // not a literal colour (RE-106)
        m.vertices[0].rgba = [127, 0, 0, 255]; // a unit normal along x
        m.vertices[1].rgba = [255, 255, 255, 255]; // a genuine colour
        m.primitives[0].indices = alloc::vec![0, 1, 0]; // both in one primitive

        let mut w = PackWriter::new();
        w.add_mesh(&m, 0, 0, |_| None);
        let bytes = w.finish();
        let pack = Pack::open(&bytes).unwrap();
        let v = pack.vertices(&pack.mesh(0).unwrap()).unwrap();

        let normal_shaded = u32_at(v, 4);
        let c = normal_shaded;
        let (r, g, b) = (c as u8, (c >> 8) as u8, (c >> 16) as u8);
        assert_eq!(r, g, "the normal-looking vertex must be shaded to grey");
        assert_eq!(g, b);

        assert_eq!(
            u32_at(v, VERTEX_SIZE + 4),
            0xFFFF_FFFF,
            "the colour-looking vertex in the same primitive must survive untouched"
        );
    }

    #[test]
    fn prim_color_scale_is_baked_into_the_vertex_at_pack_time() {
        // RE-106: `material.prim_color` is `combiner_shade_scale`'s result
        // (RE-043) whenever the combiner is a `SHADE * PRIMITIVE` shape --
        // e.g. Mario's own hat (file 296, offset 0x1E80). Nothing on the PSP
        // side ever multiplied it back in (no `prim_color` reference anywhere
        // in `psp/src/meshdraw.rs`), so a correctly-lit, grey-shaded surface
        // that should have read red stayed plain grey. The device has no
        // fixed-function stage to scale an untextured vertex colour by a
        // constant, so this is folded in here instead, the same time
        // `TEXTURE_BLEND`'s baseline colour already is.
        let mut m = sample_mesh();
        m.primitives[0].material.lit = false;
        m.primitives[0].material.prim_color = Some([255, 0, 0, 255]); // pure red scale
        for v in &mut m.vertices {
            v.rgba = [128, 128, 128, 255]; // a mid-grey literal colour
        }

        let mut w = PackWriter::new();
        w.add_mesh(&m, 0, 0, |_| None);
        let bytes = w.finish();
        let pack = Pack::open(&bytes).unwrap();
        let v = pack.vertices(&pack.mesh(0).unwrap()).unwrap();

        let c = u32_at(v, 4);
        let (r, g, b) = (c as u8, (c >> 8) as u8, (c >> 16) as u8);
        assert_eq!(r, 128, "the scale's red channel is full-strength, so red passes through");
        assert_eq!(g, 0, "the scale's green channel is zero, so it must be zeroed");
        assert_eq!(b, 0, "the scale's blue channel is zero, so it must be zeroed");
    }

    #[test]
    fn untextured_primitive_has_no_palette() {
        let mut w = PackWriter::new();
        w.add_mesh(&sample_mesh(), 0, 0, |_| None);
        let bytes = w.finish();
        let pack = Pack::open(&bytes).unwrap();
        assert_eq!(pack.texture_count(), 0);
        assert!(pack.texture(0).is_none());
    }

    #[test]
    fn rejects_garbage() {
        assert!(matches!(Pack::open(&[]), Err(PackError::TooSmall)));
        assert!(matches!(Pack::open(&[0u8; 8]), Err(PackError::TooSmall)));
    }

    #[test]
    fn rejects_bad_magic_and_version() {
        let mut w = PackWriter::new();
        w.add_mesh(&sample_mesh(), 0, 0, |_| None);
        let mut bytes = w.finish();

        let good = bytes.clone();
        bytes[0] = 0xFF;
        assert!(matches!(Pack::open(&bytes), Err(PackError::BadMagic(_))));

        let mut bytes = good.clone();
        bytes[4] = 0xEE;
        assert!(matches!(Pack::open(&bytes), Err(PackError::BadVersion(_))));

        assert!(Pack::open(&good).is_ok());
    }

    #[test]
    fn rejects_truncated_file() {
        let mut w = PackWriter::new();
        w.add_mesh(&sample_mesh(), 0, 0, |_| None);
        let bytes = w.finish();
        let truncated = &bytes[..bytes.len() - 8];
        assert!(matches!(Pack::open(truncated), Err(PackError::OutOfBounds)));
    }

    #[test]
    fn many_meshes_keep_their_own_vertex_buffers() {
        let mut w = PackWriter::new();
        for i in 0..5u32 {
            w.add_mesh(&sample_mesh(), i, i * 16, |_| None);
        }
        let bytes = w.finish();
        let pack = Pack::open(&bytes).unwrap();
        assert_eq!(pack.mesh_count(), 5);

        let mut offsets = alloc::vec![];
        for i in 0..5 {
            let m = pack.mesh(i).unwrap();
            assert_eq!(m.source_file, i);
            assert_eq!(pack.vertices(&m).unwrap().len(), 3 * VERTEX_SIZE);
            offsets.push(m.vertex_offset);
        }
        offsets.dedup();
        assert_eq!(offsets.len(), 5, "each mesh gets distinct storage");
    }

    // -----------------------------------------------------------------------
    // Stages
    // -----------------------------------------------------------------------

    /// Dream Land in miniature: a wide main platform, one floating platform,
    /// and two player spawns. Layer 0 and layer 2 are occupied, 1 and 3 are
    /// not — real stages leave slots empty, and a packer that just appended
    /// layers in order would silently shift them.
    fn sample_stage() -> (crate::stage::GroundData, crate::collision::CollisionMap) {
        use crate::collision::{CollisionLine, CollisionMap, CollisionVertex as V, LineKind};
        use crate::stage::{Bounds, GroundData, GroundLayer};

        let ground = GroundData {
            file: 104,
            offset: 0x14,
            layers: alloc::vec![
                GroundLayer {
                    index: 0,
                    graph: (33, 0x2000),
                    mobjsub_table: None,
                    anim_joints: None,
                    matanim_joints: None,
                },
                GroundLayer {
                    index: 2,
                    graph: (44, 0x2000),
                    mobjsub_table: None,
                    anim_joints: None,
                    matanim_joints: None,
                },
            ],
            map_geometry: Some((104, 0x400)),
            map_nodes: None,
            camera_bounds: Bounds {
                top: 1600,
                bottom: -600,
                right: 2400,
                left: -2400,
            },
            map_bounds: Bounds {
                top: 2500,
                bottom: -1500,
                right: 3500,
                left: -3500,
            },
            bgm_id: 0x11,
            light_angle: [0.0, 0.0],
        };

        let v = |x, y, flags| V { pos: [x, y], flags };
        let map = CollisionMap {
            lines: alloc::vec![
                CollisionLine {
                    yakumono: 0,
                    kind: LineKind::Floor,
                    id: 3,
                    points: alloc::vec![v(-2318, 0, 0x8000), v(2318, 0, 0)],
                },
                CollisionLine {
                    yakumono: 0,
                    kind: LineKind::Floor,
                    id: 1,
                    points: alloc::vec![v(951, 907, 1), v(1421, 907, 1), v(1892, 907, 1)],
                },
                CollisionLine {
                    yakumono: 1,
                    kind: LineKind::RightWall,
                    id: 5,
                    points: alloc::vec![v(2318, 0, 0), v(2290, -331, 0)],
                },
            ],
            map_objects: alloc::vec![
                crate::collision::MapObject {
                    kind: 0,
                    pos: [0, 6],
                },
                crate::collision::MapObject {
                    kind: 1,
                    pos: [-1397, 906],
                },
            ],
        };
        (ground, map)
    }

    #[test]
    fn a_stage_round_trips_with_its_collision() {
        let (ground, map) = sample_stage();
        let mut w = PackWriter::new();
        w.add_stage(&ground, Some(&map), |_, _| None);
        let bytes = w.finish();

        let pack = Pack::open(&bytes).unwrap();
        assert_eq!(pack.stage_count(), 1);
        let s = pack.stage(0).unwrap();
        assert_eq!((s.source_file, s.source_offset), (104, 0x14));
        assert_eq!(s.bgm_id, 0x11);
        assert_eq!(s.camera.top, 1600);
        assert_eq!(s.camera.left, -2400);
        assert_eq!(s.bounds.bottom, -1500);

        let lines: alloc::vec::Vec<LineDesc> = pack.stage_lines(&s).collect();
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0].kind, line_kind::FLOOR);
        assert_eq!(lines[0].id, 3);
        assert_eq!(lines[2].kind, line_kind::RIGHT_WALL);
        assert_eq!(lines[2].yakumono, 1);

        // The main platform: two points, symmetric about the origin, with the
        // first carrying its surface flags.
        let pts: alloc::vec::Vec<CollisionVertex> = pack.line_vertices(&lines[0]).collect();
        assert_eq!(pts.len(), 2);
        assert_eq!((pts[0].x, pts[0].y), (-2318, 0));
        assert_eq!((pts[1].x, pts[1].y), (2318, 0));
        assert_eq!(pts[0].flags, 0x8000);

        // A three-point polyline stays three points, not collapsed to a
        // segment: `vertex_count` is a point count (RE-029).
        assert_eq!(lines[1].vertex_count, 3);
        let pts: alloc::vec::Vec<CollisionVertex> = pack.line_vertices(&lines[1]).collect();
        assert_eq!(pts.len(), 3);
        assert_eq!(pts[1].x, 1421);
    }

    #[test]
    fn empty_layer_slots_stay_empty() {
        let (ground, map) = sample_stage();
        let mut w = PackWriter::new();
        // Both named graphs resolve, but they belong in slots 0 and 2.
        w.add_stage(&ground, Some(&map), |file, offset| match (file, offset) {
            (33, 0x2000) => Some(7),
            (44, 0x2000) => Some(9),
            _ => None,
        });
        let bytes = w.finish();
        let pack = Pack::open(&bytes).unwrap();
        let s = pack.stage(0).unwrap();
        assert_eq!(s.layers, [7, StageDesc::NO_LAYER, 9, StageDesc::NO_LAYER]);
    }

    #[test]
    fn a_layer_whose_object_was_never_packed_is_reported_as_absent() {
        let (ground, map) = sample_stage();
        let mut w = PackWriter::new();
        w.add_stage(&ground, Some(&map), |file, _| (file == 33).then_some(7));
        let bytes = w.finish();
        let pack = Pack::open(&bytes).unwrap();
        assert_eq!(pack.stage(0).unwrap().layers[2], StageDesc::NO_LAYER);
    }

    #[test]
    fn spawns_are_found_by_player_number() {
        let (ground, map) = sample_stage();
        let mut w = PackWriter::new();
        w.add_stage(&ground, Some(&map), |_, _| None);
        let bytes = w.finish();
        let pack = Pack::open(&bytes).unwrap();
        let s = pack.stage(0).unwrap();

        let p1 = pack.spawn(&s, 0).expect("player 1 start");
        assert_eq!((p1.x, p1.y), (0, 6));
        let p2 = pack.spawn(&s, 1).expect("player 2 start");
        assert_eq!((p2.x, p2.y), (-1397, 906));
        assert!(pack.spawn(&s, 3).is_none(), "this fixture has only two");
    }

    #[test]
    fn a_second_stage_does_not_read_the_first_ones_bytes() {
        // The trap `TextureDesc::SIZE` fell into: a wrong descriptor size still
        // reads entry 0 correctly and mangles every entry after it.
        let (ground, map) = sample_stage();
        let mut other = ground.clone();
        other.file = 200;
        other.bgm_id = 0x22;
        other.camera_bounds.top = 999;

        let mut w = PackWriter::new();
        w.add_stage(&ground, Some(&map), |_, _| None);
        w.add_stage(&other, None, |_, _| None);

        let bytes = w.finish();
        let pack = Pack::open(&bytes).unwrap();
        assert_eq!(pack.stage_count(), 2);

        let b = pack.stage(1).unwrap();
        assert_eq!((b.source_file, b.bgm_id, b.camera.top), (200, 0x22, 999));
        // A stage whose geometry could not be read still carries its bounds.
        assert_eq!(b.line_count, 0);
        assert_eq!(b.point_count, 0);
        // And it must not borrow the first stage's lines.
        assert_eq!(b.first_line, 3);
        assert_eq!(pack.stage_lines(&b).count(), 0);
    }

    #[test]
    fn stage_tables_do_not_disturb_the_geometry_tables() {
        // Stages sit after the node table, so a wrong size here would silently
        // move the blob and corrupt vertices rather than fail to load.
        let (ground, map) = sample_stage();
        let mut w = PackWriter::new();
        w.add_mesh(&sample_mesh(), 42, 0x1000, |_| None);
        w.add_object(&chain_graph(3), 11, |n| Some(n as u32), &[]);
        w.add_stage(&ground, Some(&map), |_, _| None);

        let bytes = w.finish();
        let pack = Pack::open(&bytes).unwrap();
        let m = pack.mesh(0).unwrap();
        assert_eq!(m.source_file, 42);
        assert_eq!(pack.vertices(&m).unwrap().len(), 3 * VERTEX_SIZE);
        assert_eq!(pack.node(2).unwrap().mesh, 2);
        assert_eq!(pack.stage(0).unwrap().bgm_id, 0x11);
        assert_eq!(pack.coll_vertex_count(), 7);
        assert_eq!(pack.point_count(), 2);
    }

    #[test]
    fn a_pack_with_no_stages_still_loads() {
        let mut w = PackWriter::new();
        w.add_mesh(&sample_mesh(), 0, 0, |_| None);
        let bytes = w.finish();
        let pack = Pack::open(&bytes).unwrap();
        assert_eq!(pack.stage_count(), 0);
        assert!(pack.stage(0).is_none());
        assert!(pack.line(0).is_none());
        assert!(pack.coll_vertex(0).is_none());
        assert!(pack.map_point(0).is_none());
        assert_eq!(pack.fighter_count(), 0);
        assert!(pack.fighter(0).is_none());
    }

    /// Mario's real animation lengths, as `romtool anims` reads them.
    fn sample_anims() -> crate::anim::FighterLengths {
        crate::anim::FighterLengths {
            name: "Mario",
            frames: {
                // Mario's seven timed lengths; the looping slots have none.
                let mut f = [0u16; crate::anim::SLOT_COUNT];
                f[..7].copy_from_slice(&[23, 12, 23, 8, 12, 7, 25]);
                f
            },
        }
    }

    fn sample_fighter(kind: u8) -> crate::fighter::Fighter {
        use crate::fighter::{FighterAttributes, FighterFile, ObjectColl};
        crate::fighter::Fighter {
            file: FighterFile {
                kind,
                name: "Mario",
                file: 203,
                offset: 0x0428,
            },
            setup_parts: 0x00FF_FFFF,
            animlock: 0,
            attributes: FighterAttributes {
                size: 1.12,
                walkslow_anim_length: 90.0,
                walkmiddle_anim_length: 60.0,
                walkfast_anim_length: 40.0,
                throw_walkslow_anim_length: 0.0,
                throw_walkmiddle_anim_length: 0.0,
                throw_walkfast_anim_length: 0.0,
                rebound_anim_length: 16.0,
                walk_speed_mul: 0.3,
                traction: 1.5,
                dash_speed: 54.0,
                dash_decel: 2.8,
                run_speed: 44.0,
                kneebend_anim_length: 3.0,
                jump_vel_x: 0.35,
                jump_height_mul: 0.7,
                jump_height_base: 26.0,
                jumpaerial_vel_x: 0.35,
                jumpaerial_height: 0.9,
                air_accel: 0.025,
                air_speed_max_x: 30.0,
                air_friction: 0.2,
                gravity: 2.4,
                tvel_base: 44.0,
                tvel_fast: 70.0,
                jumps_max: 2,
                weight: 1.0,
                attack1_followup_frames: 24.0,
                dash_to_run: 14.0,
                shield_size: 260.0,
                shield_break_vel_y: 70.0,
                shadow_size: 200.0,
                jostle_width: 112.5,
                jostle_x: 0.0,
                is_metallic: false,
                cam_offset_y: 250.0,
                closeup_camera_zoom: 1600.0,
                camera_zoom: 1.0,
                camera_zoom_base: 500.0,
                map_coll: ObjectColl {
                    top: 320.0,
                    center: 190.0,
                    bottom: 0.0,
                    width: 150.0,
                },
                cliffcatch_coll: (400.0, 360.0),
            },
        }
    }

    #[test]
    fn a_fighter_round_trips_through_the_pack() {
        let mut w = PackWriter::new();
        w.add_fighter(&sample_fighter(0), &sample_anims());
        let bytes = w.finish();
        let pack = Pack::open(&bytes).unwrap();

        assert_eq!(pack.fighter_count(), 1);
        let f = pack.fighter(0).unwrap();
        assert_eq!(f.kind, 0);
        assert_eq!(f.source_file, 203);
        assert_eq!(f.source_offset, 0x0428);
        // One field from each group, so a mis-ordered writer shows up.
        assert_eq!(f.walk_speed_mul, 0.3);
        assert_eq!(f.gravity, 2.4);
        assert_eq!(f.jumps_max, 2);
        assert_eq!(f.is_metallic, 0);
        assert_eq!(f.coll_top, 320.0);
        assert_eq!(f.coll_width, 150.0);
        assert_eq!(f.cliffcatch_height, 360.0);
        // The animation lengths land after the attribute scalars, so a stride
        // or ordering slip shows up here first.
        assert_eq!(f.dash_anim_length, 23.0);
        assert_eq!(f.turn_anim_length, 12.0);
        assert_eq!(f.pass_anim_length, 25.0);
    }

    #[test]
    fn the_fighter_stride_matches_what_the_writer_emits() {
        // The reader indexes by `FighterDesc::SIZE`, so a struct that does not
        // serialise to exactly that many bytes reads the next fighter's data
        // rather than failing.
        let mut w = PackWriter::new();
        for k in 0..3 {
            w.add_fighter(&sample_fighter(k), &sample_anims());
        }
        let bytes = w.finish();
        let pack = Pack::open(&bytes).unwrap();
        assert_eq!(pack.fighter_count(), 3);
        for k in 0..3 {
            assert_eq!(pack.fighter(k).unwrap().kind, k);
            assert_eq!(pack.fighter(k).unwrap().gravity, 2.4);
            assert_eq!(pack.fighter(k).unwrap().pass_anim_length, 25.0);
        }
        assert!(pack.fighter(3).is_none());
    }

    #[test]
    fn fighters_do_not_disturb_the_tables_before_them() {
        let (ground, map) = sample_stage();
        let mut w = PackWriter::new();
        w.add_mesh(&sample_mesh(), 42, 0x1000, |_| None);
        w.add_stage(&ground, Some(&map), |_, _| None);
        w.add_fighter(&sample_fighter(0), &sample_anims());

        let bytes = w.finish();
        let pack = Pack::open(&bytes).unwrap();
        let m = pack.mesh(0).unwrap();
        assert_eq!(pack.vertices(&m).unwrap().len(), 3 * VERTEX_SIZE);
        assert_eq!(pack.stage(0).unwrap().bgm_id, 0x11);
        assert_eq!(pack.point_count(), 2);
        assert_eq!(pack.fighter(0).unwrap().coll_top, 320.0);
    }

    #[test]
    fn an_animation_round_trips_with_its_joints_and_its_script() {
        let mut w = PackWriter::new();
        // Three joints: two animated, one not. The unanimated one is the
        // common case -- roughly a fifth of joints are left at rest by any
        // given animation -- and it must come back as NO_SCRIPT rather than
        // as a script at offset zero.
        let script = alloc::vec![0xABu8; 96];
        let i = w.add_anim(
            0,
            crate::anim::SLOT_DASH as u32,
            504,
            23,
            &script,
            &[(Some(0x40), Some(7)), (None, Some(8)), (Some(0x60), None)],
        );
        assert_eq!(i, 0);
        let bytes = w.finish();

        let pack = Pack::open(&bytes).unwrap();
        assert_eq!(pack.anim_count(), 1);
        assert_eq!(pack.anim_joint_count(), 3);

        let a = pack.fighter_anim(0, crate::anim::SLOT_DASH as u32).unwrap();
        assert_eq!(a.source_file, 504);
        assert_eq!(a.frames, 23);
        assert_eq!(a.joint_count, 3);
        assert_eq!(pack.anim_script(&a), Some(&script[..]));

        let j: alloc::vec::Vec<AnimJoint> = (0..a.joint_count)
            .map(|k| pack.anim_joint(a.first_joint + k).unwrap())
            .collect();
        assert_eq!(
            j[0],
            AnimJoint {
                script: 0x40,
                node: 7
            }
        );
        assert_eq!(
            j[1],
            AnimJoint {
                script: AnimJoint::NO_SCRIPT,
                node: 8
            }
        );
        assert_eq!(
            j[2],
            AnimJoint {
                script: 0x60,
                node: AnimJoint::NO_NODE
            }
        );
    }

    #[test]
    fn a_shared_animation_file_is_stored_once() {
        // Kirby and Jigglypuff share three animation files outright, and every
        // polygon variant shares all seven with the character it copies. Nine
        // copies of the same 2 KB would be most of a megabyte across the
        // roster.
        let mut w = PackWriter::new();
        let script = alloc::vec![0x5Au8; 1024];
        w.add_anim(8, 4, 1280, 10, &script, &[]);
        w.add_anim(10, 4, 1280, 10, &script, &[]);
        let bytes = w.finish();

        let pack = Pack::open(&bytes).unwrap();
        let a = pack.anim(0).unwrap();
        let b = pack.anim(1).unwrap();
        assert_eq!(a.script_offset, b.script_offset, "same file, same bytes");
        assert_eq!(pack.anim_script(&a), pack.anim_script(&b));
        // Both must still be findable under their own fighter.
        assert_eq!(pack.anim(0).unwrap().fighter, 8);
        assert_eq!(pack.anim(1).unwrap().fighter, 10);
    }

    /// RE-089/RE-090/RE-091: a `PaletteID`-cycling material animation's
    /// script and every resolved palette variant, round-tripped and pointed
    /// at from the texture it animates.
    #[test]
    fn a_mat_anim_round_trips_with_its_script_and_palettes() {
        let mut w = PackWriter::new();
        let tex = PspTexture {
            width: 32,
            height: 8,
            stride: 32,
            format: Psm::PsmT4,
            data: alloc::vec![0xCDu8; 128],
            swizzled: true,
            palette: alloc::vec![0xFF00_00FFu32; 16],
            levels: 1,
        };
        let texture = w.add_texture(&tex, false, false);

        let file_bytes = alloc::vec![0x11u8; 200];
        // File 117's real shape (RE-089/RE-090): a script cycling through
        // several distinct palettes.
        let palettes = alloc::vec![
            alloc::vec![0x1111_1111u32; 16],
            alloc::vec![0x2222_2222u32; 16],
            alloc::vec![0x3333_3333u32; 16],
        ];
        let mat_anim = w.add_mat_anim(117, &file_bytes, 0x30, 0x2AA8, &palettes);
        w.set_texture_mat_anim(texture, mat_anim);
        let bytes = w.finish();

        let pack = Pack::open(&bytes).unwrap();
        assert_eq!(pack.mat_anim_count(), 1);
        assert_eq!(pack.mat_anim_palette_count(), 3);

        let t = pack.texture(0).unwrap();
        assert_eq!(t.mat_anim, mat_anim);

        let a = pack.mat_anim(mat_anim).unwrap();
        assert_eq!(a.script, 0x30);
        assert_eq!(a.palette_count, 3);
        assert_eq!(a.source_file, 117);
        assert_eq!(a.source_offset, 0x2AA8);
        assert_eq!(pack.mat_anim_file(&a), Some(&file_bytes[..]));

        let got: alloc::vec::Vec<u32> = (0..a.palette_count)
            .map(|k| {
                let p = pack.mat_anim_palette(a.first_palette + k).unwrap();
                let data = pack.mat_anim_palette_data(&p).unwrap();
                u32_at(data, 0)
            })
            .collect();
        assert_eq!(got, alloc::vec![0x1111_1111, 0x2222_2222, 0x3333_3333]);
    }

    /// A texture nothing animates must read back `NO_ANIM`, not index 0 --
    /// the same "absent, not accidentally valid" shape `AnimJoint::NO_SCRIPT`
    /// already guards against.
    #[test]
    fn a_texture_with_no_mat_anim_reads_back_no_anim() {
        let mut w = PackWriter::new();
        let tex = PspTexture {
            width: 8,
            height: 8,
            stride: 8,
            format: Psm::Psm8888,
            data: alloc::vec![0u8; 256],
            swizzled: false,
            palette: alloc::vec::Vec::new(),
            levels: 1,
        };
        w.add_texture(&tex, false, false);
        let bytes = w.finish();

        let pack = Pack::open(&bytes).unwrap();
        assert_eq!(pack.texture(0).unwrap().mat_anim, TextureDesc::NO_ANIM);
    }

    /// Mirrors `a_shared_animation_file_is_stored_once`: two animated
    /// textures whose driving scripts live in the same archive file must not
    /// duplicate that file's bytes in the blob.
    #[test]
    fn a_shared_mat_anim_file_is_stored_once() {
        let mut w = PackWriter::new();
        let file_bytes = alloc::vec![0x22u8; 512];
        let one = alloc::vec![alloc::vec![0xAAAA_AAAAu32; 16]];
        w.add_mat_anim(114, &file_bytes, 0x100, 0x4F54, &one);
        w.add_mat_anim(114, &file_bytes, 0x200, 0x5098, &one);
        let bytes = w.finish();

        let pack = Pack::open(&bytes).unwrap();
        let a = pack.mat_anim(0).unwrap();
        let b = pack.mat_anim(1).unwrap();
        assert_eq!(a.file_offset, b.file_offset, "same file, same bytes");
        assert_eq!(pack.mat_anim_file(&a), pack.mat_anim_file(&b));
        assert_eq!(a.script, 0x100);
        assert_eq!(b.script, 0x200);
    }

    /// `fighter_anim` finds a row by arithmetic, so the fighter entries must
    /// be a dense block starting at index 0. Stage animations share the table
    /// and are appended after it; writing one first shifts every fighter
    /// animation by a row and the fighter silently gets someone else's
    /// skeleton — which showed up only as a triangle count (RE-051).
    #[test]
    fn a_stage_animation_does_not_displace_the_fighter_block() {
        let mut w = PackWriter::new();
        let slots = crate::anim::SLOT_COUNT as u32;
        for slot in 0..slots {
            w.add_anim(0, slot, 300, 10, &[0u8; 8], &[(Some(0), Some(0))]);
        }
        w.add_anim(AnimDesc::STAGE, 7, 104, 0, &[0u8; 8], &[(Some(0), Some(0))]);
        let bytes = w.finish();
        let pack = Pack::open(&bytes).unwrap();

        for slot in 0..slots {
            let a = pack
                .fighter_anim(0, slot)
                .unwrap_or_else(|| panic!("fighter 0 slot {slot} must resolve"));
            assert_eq!(a.fighter, 0);
            assert_eq!(a.slot, slot);
        }
        let s = pack
            .stage_anim(7)
            .expect("the stage entry is still findable");
        assert_eq!(s.fighter, AnimDesc::STAGE);
        assert_eq!(s.source_file, 104);
        assert_eq!(pack.stage_anim(8), None);
    }

    #[test]
    fn a_billboard_node_is_flagged_through_the_pack() {
        // `DObjDesc.id & 0xF000` selects a camera-relative matrix kind, and it
        // has to survive serialisation or the device never sees it. Dream
        // Land's canopy sprites use 0x4001: depth 1, kind 0x4000 (RE-048).
        use crate::scene::{DObjDesc, DObjNode, SceneGraph};
        let sprite = DObjDesc {
            id: 0x4001,
            dl: None,
            translate: [0.0; 3],
            rotate: [0.0; 3],
            scale: [1.0; 3],
        };
        let plain = DObjDesc { id: 1, ..sprite };
        let graph = SceneGraph {
            offset: 0x100,
            nodes: alloc::vec![
                DObjNode {
                    desc: plain,
                    parent: None
                },
                DObjNode {
                    desc: sprite,
                    parent: Some(0)
                },
            ],
        };
        let mut w = PackWriter::new();
        w.add_object(&graph, 104, |_| None, &[]);
        let bytes = w.finish();

        let pack = Pack::open(&bytes).unwrap();
        assert_eq!(pack.node(0).unwrap().flags & NodeDesc::FLAG_BILLBOARD, 0);
        assert_eq!(
            pack.node(1).unwrap().flags & NodeDesc::FLAG_BILLBOARD,
            NodeDesc::FLAG_BILLBOARD,
            "a 0x4000 node must reach the device flagged"
        );
    }

    #[test]
    fn a_recalc_node_is_flagged_as_a_spin_free_billboard() {
        // `gcPrepDObjMatrix` case 44 (`0x8000`/`RecalcRotRpyRSca`,
        // `objdisplay.c`) never touches `dobj->rotate` at all -- it is the
        // same camera-relative MVP replacement as kinds 46/48, just with the
        // sin/cos spin term dropped. Every shipped `0x8000` node's `rotate`
        // is `[0, 0, 0]` (RE-061), so reusing `FLAG_BILLBOARD`'s spin-from-
        // `rest_rotate[0]` path is exact, not an approximation.
        use crate::scene::{DObjDesc, DObjNode, SceneGraph};
        let sprite = DObjDesc {
            id: 0x8001,
            dl: None,
            translate: [0.0; 3],
            rotate: [0.0; 3],
            scale: [1.0; 3],
        };
        let graph = SceneGraph {
            offset: 0x100,
            nodes: alloc::vec![DObjNode {
                desc: sprite,
                parent: None
            }],
        };
        let mut w = PackWriter::new();
        w.add_object(&graph, 104, |_| None, &[]);
        let bytes = w.finish();

        let pack = Pack::open(&bytes).unwrap();
        assert_eq!(
            pack.node(0).unwrap().flags & NodeDesc::FLAG_BILLBOARD,
            NodeDesc::FLAG_BILLBOARD,
            "a 0x8000 node must reach the device flagged"
        );
    }

    #[test]
    fn a_kind_50_node_is_flagged_as_a_billboard_like_kind_48() {
        // `gcPrepDObjMatrix` case 50 (`0x1000`/`Kind50`, `objdisplay.c`) is
        // the same move-word layout and per-node scale math as case 48
        // (`Kind48`, already `FLAG_BILLBOARD`), just sourced from
        // `sGCMatrixMod2F` instead of `sGCMatrixMod1F`. No shipped node uses
        // this bit (RE-063: 0/3117 archive-wide), so this is fidelity with
        // the decomp's case structure, not a fix for an observed bug.
        use crate::scene::{DObjDesc, DObjNode, SceneGraph};
        let sprite = DObjDesc {
            id: 0x1001,
            dl: None,
            translate: [0.0; 3],
            rotate: [0.0; 3],
            scale: [1.0; 3],
        };
        let graph = SceneGraph {
            offset: 0x100,
            nodes: alloc::vec![DObjNode {
                desc: sprite,
                parent: None
            }],
        };
        let mut w = PackWriter::new();
        w.add_object(&graph, 104, |_| None, &[]);
        let bytes = w.finish();

        let pack = Pack::open(&bytes).unwrap();
        assert_eq!(
            pack.node(0).unwrap().flags & NodeDesc::FLAG_BILLBOARD,
            NodeDesc::FLAG_BILLBOARD,
            "a 0x1000 node must reach the device flagged"
        );
    }

    #[test]
    fn a_node_carries_the_local_transform_an_animation_starts_from() {
        // The rest pose has to survive the pack: an animation overwrites only
        // the tracks it names and leaves the rest of the joint where the model
        // put it (RE-036). A world matrix alone cannot supply that -- pulling
        // a rotation and a scale back out of one is lossy.
        use crate::scene::{DObjDesc, DObjNode, SceneGraph};
        let desc = DObjDesc {
            id: 0,
            dl: None,
            translate: [1.5, -2.5, 3.0],
            rotate: [0.25, 0.5, -0.75],
            scale: [1.0, 2.0, 0.5],
        };
        let graph = SceneGraph {
            offset: 0x100,
            nodes: alloc::vec![DObjNode { desc, parent: None }],
        };
        let mut w = PackWriter::new();
        w.add_object(&graph, 296, |_| None, &[]);
        let bytes = w.finish();

        let pack = Pack::open(&bytes).unwrap();
        let n = pack.node(0).unwrap();
        assert_eq!(n.rest_translate, [1.5, -2.5, 3.0]);
        assert_eq!(n.rest_rotate, [0.25, 0.5, -0.75]);
        assert_eq!(n.rest_scale, [1.0, 2.0, 0.5]);
        // And the baked world matrix is still there for the static path.
        assert_eq!(n.world[12], 1.5 / MODEL_SCALE);
    }
}
