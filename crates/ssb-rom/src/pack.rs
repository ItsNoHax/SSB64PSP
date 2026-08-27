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
//! ---- 16-byte aligned blob region ----
//! vertex data | index data | texel data | palette data
//! ```
//!
//! Descriptors carry offsets into the blob region, so the whole file can be
//! relocated freely: nothing stores an absolute address.

use alloc::vec::Vec;

/// Magic at the start of every pack: `SSBP`.
pub const MAGIC: u32 = 0x5342_5350;

/// Bumped whenever the layout changes incompatibly.
///
/// 2 added the object and node tables.
pub const VERSION: u32 = 2;

/// Alignment for every blob the GE reads.
pub const ALIGN: usize = 16;

/// Bytes per packed vertex. Must equal `size_of::<PackedVertex>()`.
///
/// 16, not 14: the `u32` colour forces 4-byte alignment, so `repr(C)` inserts
/// tail padding. The GE also requires the vertex stride to be a multiple of the
/// largest component size, so 16 is what the hardware wants anyway.
pub const VERTEX_SIZE: usize = 16;

/// Header, 64 bytes so the descriptor tables start 16-byte aligned.
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
    pub _pad: [u32; 7],
}

impl Header {
    pub const SIZE: usize = 64;
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
}

impl PrimDesc {
    pub const SIZE: usize = 24;
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
}

impl TextureDesc {
    /// 24, not 20: `u16 * 3 + u8 * 2` is 8 bytes, plus four `u32` is 16.
    ///
    /// This was declared as 20 and the writer emitted 24, so every descriptor
    /// after the first was read from the wrong offset and textures came out as
    /// coloured noise on device. The size guard test below exists because the
    /// original round-trip test only checked texture 0, where the offset is
    /// correct no matter what the stride says.
    pub const SIZE: usize = 24;
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
    pub world: [f32; 16],
}

impl NodeDesc {
    /// `4 + 4 + 64`.
    pub const SIZE: usize = 72;
    pub const NO_MESH: u32 = u32::MAX;
    pub const NO_PARENT: u32 = u32::MAX;
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
/// Smash's real lighting comes from per-material `MObj` light colours, which
/// are not extracted yet. Until then a single neutral key light gives shape
/// definition instead of the psychedelic look you get from drawing packed
/// normals as vertex colours.
const LIGHT_DIR: [f32; 3] = [0.372, 0.743, 0.557]; // normalised (2, 4, 3)

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

/// Fraction of a primitive's vertices that must look like normals before the
/// whole primitive is treated as lit.
///
/// Decided per-primitive rather than per-vertex so a single surface is not
/// split into shaded and unshaded halves by a few ambiguous vertices.
const NORMAL_MAJORITY: f32 = 0.8;

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

    /// Adds a texture, returning its index.
    pub fn add_texture(&mut self, tex: &crate::psp_texture::PspTexture) -> u32 {
        let data_offset = self.push_blob(&tex.data);
        let palette_bytes: Vec<u8> = tex.palette.iter().flat_map(|c| c.to_le_bytes()).collect();
        let palette_offset = if palette_bytes.is_empty() {
            0
        } else {
            self.push_blob(&palette_bytes)
        };

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
        });
        (self.textures.len() - 1) as u32
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
        // Which vertices belong to a lit primitive. The IR keeps the raw bytes
        // (it is deliberately lossless); interpreting them is this lowering
        // step's job, and it has to be per-vertex because a mesh can mix lit
        // and unlit materials over one shared vertex buffer.
        let mut lit = alloc::vec![false; mesh.vertices.len()];
        for p in &mesh.primitives {
            // Trust the geometry mode when it says lit; otherwise fall back to
            // inspecting the data, because the mode is often inherited from
            // outside the list. See `looks_like_unit_normal`.
            let treat_as_lit = p.material.lit || {
                let mut seen = 0usize;
                let mut normals = 0usize;
                for &i in &p.indices {
                    if let Some(v) = mesh.vertices.get(i as usize) {
                        seen += 1;
                        normals += looks_like_unit_normal(v.rgba) as usize;
                    }
                }
                seen > 0 && (normals as f32 / seen as f32) >= NORMAL_MAJORITY
            };

            if treat_as_lit {
                for &i in &p.indices {
                    if let Some(slot) = lit.get_mut(i as usize) {
                        *slot = true;
                    }
                }
            }
        }

        // Vertices, converted to the GE layout.
        let mut verts = Vec::with_capacity(mesh.vertices.len() * VERTEX_SIZE);
        for (i, v) in mesh.vertices.iter().enumerate() {
            let rgba = if lit[i] { shade_normal(v.rgba) } else { v.rgba };
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

            self.prims.push(PrimDesc {
                texture: texture_for(i).unwrap_or(PrimDesc::NO_TEXTURE),
                flags: f,
                prim_color: crate::psp_texture::pack_abgr(m.prim_color),
                env_color: crate::psp_texture::pack_abgr(m.env_color),
                index_offset,
                index_count: p.indices.len() as u32,
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
    /// `mesh_for` maps a node's display-list offset to a mesh index already
    /// added via [`PackWriter::add_mesh`]; nodes whose display list did not
    /// convert (or which have none) become pure transforms.
    pub fn add_object(
        &mut self,
        graph: &crate::scene::SceneGraph,
        source_file: u32,
        mesh_for: impl Fn(u32) -> Option<u32>,
    ) -> u32 {
        let first_node = self.nodes.len() as u32;

        for (i, node) in graph.nodes.iter().enumerate() {
            // `world_transforms` is O(n) over the whole graph, so hoisting it
            // out of the loop matters for the 33-node fighter hierarchies.
            let mesh = node
                .desc
                .dl
                .and_then(&mesh_for)
                .unwrap_or(NodeDesc::NO_MESH);
            let parent = match node.parent {
                Some(p) => first_node + p as u32,
                None => NodeDesc::NO_PARENT,
            };
            self.nodes.push(NodeDesc {
                mesh,
                parent,
                world: [0.0; 16],
            });
            debug_assert_eq!(self.nodes.len() - 1, first_node as usize + i);
        }

        for (i, m) in graph.world_transforms().into_iter().enumerate() {
            let mut world = m.0;
            // Bring the translation into the same normalised space as the
            // `i16` vertex positions; see `MODEL_SCALE`.
            world[12] /= MODEL_SCALE;
            world[13] /= MODEL_SCALE;
            world[14] /= MODEL_SCALE;
            self.nodes[first_node as usize + i].world = world;
        }

        self.objects.push(ObjectDesc {
            first_node,
            node_count: graph.nodes.len() as u32,
            source_file,
            source_offset: graph.offset,
        });
        (self.objects.len() - 1) as u32
    }

    /// Serialises the pack.
    pub fn finish(self) -> Vec<u8> {
        let table_bytes = self.meshes.len() * MeshDesc::SIZE
            + self.prims.len() * PrimDesc::SIZE
            + self.textures.len() * TextureDesc::SIZE
            + self.objects.len() * ObjectDesc::SIZE
            + self.nodes.len() * NodeDesc::SIZE;
        let blob_offset = align_up(Header::SIZE + table_bytes);

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
    blob_offset: usize,
    blob_len: usize,
}

fn u32_at(d: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([d[at], d[at + 1], d[at + 2], d[at + 3]])
}
fn u16_at(d: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([d[at], d[at + 1]])
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

        let tables_end = Header::SIZE
            + mesh_count as usize * MeshDesc::SIZE
            + prim_count as usize * PrimDesc::SIZE
            + texture_count as usize * TextureDesc::SIZE
            + object_count as usize * ObjectDesc::SIZE
            + node_count as usize * NodeDesc::SIZE;

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
        Some(NodeDesc {
            mesh: u32_at(self.data, at),
            parent: u32_at(self.data, at + 4),
            world,
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
        })
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
                    prim_color: [1, 2, 3, 4],
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
        };
        w.add_texture(&tex);
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
        };
        for n in 0..4 {
            w.add_texture(&tex(n));
        }
        w.add_mesh(&sample_mesh(), 1, 2, |_| Some(3));
        let bytes = w.finish();
        let pack = Pack::open(&bytes).unwrap();

        // The blob must start exactly after all five tables.
        let expected_tables = Header::SIZE
            + pack.mesh_count as usize * MeshDesc::SIZE
            + pack.prim_count as usize * PrimDesc::SIZE
            + pack.texture_count as usize * TextureDesc::SIZE
            + pack.object_count as usize * ObjectDesc::SIZE
            + pack.node_count as usize * NodeDesc::SIZE;
        assert!(
            pack.blob_offset >= expected_tables,
            "blob overlaps the descriptor tables: {} < {}",
            pack.blob_offset,
            expected_tables
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
        w.add_object(&chain_graph(3), 11, |dl| Some(dl / 0x100));
        w.add_object(&chain_graph(2), 22, |_| None);

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
    fn node_transforms_accumulate_and_are_scaled_into_vertex_space() {
        let mut w = PackWriter::new();
        w.add_object(&chain_graph(3), 0, |_| None);
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
        };
        w.add_texture(&tex);
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
        let mut w = PackWriter::new();
        w.add_mesh(&m, 0, 0, |_| None);
        let bytes = w.finish();
        let pack = Pack::open(&bytes).unwrap();
        let v = pack.vertices(&pack.mesh(0).unwrap()).unwrap();
        assert_eq!(u32_at(v, VERTEX_SIZE + 4), 0xFFFF_FFFF, "unlit unchanged");

        // The same mesh marked lit must have that vertex replaced by a shade.
        let mut m = sample_mesh();
        m.primitives[0].material.lit = true;
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
}
