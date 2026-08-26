//! Display list → indexed mesh conversion.
//!
//! Walks an F3DEX2 command stream and produces a neutral, **lossless**
//! intermediate mesh. Packing into a PSP vertex format is a separate step
//! (`psp_vtx`), so this stage can be tested without reference to hardware.
//!
//! ## Why this shape
//!
//! The RSP model is a 32-entry vertex cache plus triangles that index into it.
//! `G_VTX` refills part of the cache; `G_TRI1`/`G_TRI2` reference slots. The
//! same vertex is re-uploaded every time a new batch needs it, because the
//! cache is tiny.
//!
//! The PSP has no such limit, so the conversion *undoes* the batching: build
//! one vertex buffer per material run and index into it. This is both smaller
//! and faster — the GE reads each vertex once instead of once per batch.
//!
//! ## Performance decisions baked in here
//!
//! * **Vertices are deduplicated** within a material run. Smash's display lists
//!   re-upload shared vertices constantly; collapsing them cuts both memory and
//!   GE vertex-fetch bandwidth.
//! * **Primitives are split on material change, then merged by material.**
//!   State changes are the expensive thing on the GE, so the output is sorted
//!   to minimise them rather than preserving submission order.
//! * **Positions and UVs stay integral.** The N64 stores `i16` positions and
//!   S10.5 UVs, which map onto the PSP's 16-bit vertex components directly.
//!   Converting to `f32` here would double vertex size for no gain.

use alloc::collections::BTreeMap;
use alloc::vec::Vec;

use crate::dl::{Cmd, Vtx};
use crate::scan::VTX_CACHE_SIZE;
use crate::texture::{BitSize, Format};

/// A vertex in the intermediate mesh, kept in the original integral form.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct MeshVertex {
    /// Object-space position, game units.
    pub pos: [i16; 3],
    /// Texture coordinates, S10.5 fixed point (32 = one texel).
    pub uv: [i16; 2],
    /// Vertex colour, or a packed normal when the material is lit.
    pub rgba: [u8; 4],
}

/// Which texture a primitive samples, identified by where it lives.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct TextureRef {
    /// Byte offset of the texel data within the source file.
    pub data_offset: u32,
    pub format: Format,
    pub size: BitSize,
    pub width: u16,
    pub height: u16,
    /// Byte offset of the palette, for `Ci` formats.
    pub palette_offset: Option<u32>,
    pub palette_entries: u16,
}

/// Render state a primitive is drawn under.
///
/// Ordering matters: primitives are grouped by this key, so cheap-to-compare
/// fields come first and the sort naturally clusters same-texture draws.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct MeshMaterial {
    pub texture: Option<TextureRef>,
    pub cull_back: bool,
    pub cull_front: bool,
    pub lit: bool,
    pub smooth: bool,
    pub z_buffer: bool,
    pub prim_color: [u8; 4],
    pub env_color: [u8; 4],
    pub blend_color: [u8; 4],
}

/// A run of triangles sharing one material, indexing [`Mesh::vertices`].
#[derive(Debug, Clone, Default)]
pub struct Primitive {
    pub material: MeshMaterial,
    /// Triangle list, three indices per triangle, into the mesh-wide buffer.
    pub indices: Vec<u16>,
}

impl Primitive {
    pub fn triangle_count(&self) -> usize {
        self.indices.len() / 3
    }
}

/// The converted result.
///
/// Vertices live in **one buffer shared by every primitive**, rather than one
/// buffer per primitive. A vertex used under two different materials would
/// otherwise be stored twice; measured over the whole archive that inflated
/// the vertex count by 25% *above* what the N64 uploads. A single buffer also
/// lets the GE's post-transform cache do its job across draws.
#[derive(Debug, Clone, Default)]
pub struct Mesh {
    pub vertices: Vec<MeshVertex>,
    pub primitives: Vec<Primitive>,
}

impl Mesh {
    pub fn triangle_count(&self) -> usize {
        self.primitives.iter().map(|p| p.triangle_count()).sum()
    }

    pub fn vertex_count(&self) -> usize {
        self.vertices.len()
    }
}

/// Errors from mesh conversion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeshError {
    /// A triangle referenced a cache slot no `G_VTX` had filled. Indicates the
    /// command stream was entered mid-way, or is not really a display list.
    EmptyCacheSlot(u8),
    /// A `G_VTX` pointed outside the file.
    VertexDataOutOfBounds { offset: u32 },
    /// More unique vertices in one material run than a `u16` index can address.
    TooManyVertices,
}

/// Tracks RDP/RSP state while walking a display list.
struct State {
    cache: [Option<MeshVertex>; VTX_CACHE_SIZE as usize],
    material: MeshMaterial,
    /// Pending texture image set by `G_SETTIMG`, completed by `G_SETTILE`.
    timg: Option<(u32, u8, u8)>,
    tile_dims: Option<(u16, u16)>,
    palette_offset: Option<u32>,
    palette_entries: u16,
    texture_enabled: bool,
}

impl State {
    fn new() -> Self {
        State {
            cache: [None; VTX_CACHE_SIZE as usize],
            material: MeshMaterial::default(),
            timg: None,
            tile_dims: None,
            palette_offset: None,
            palette_entries: 0,
            texture_enabled: false,
        }
    }

    /// Assembles the current texture binding, if one is fully specified.
    fn current_texture(&self) -> Option<TextureRef> {
        if !self.texture_enabled {
            return None;
        }
        let (offset, fmt, siz) = self.timg?;
        let (w, h) = self.tile_dims?;
        Some(TextureRef {
            data_offset: offset,
            format: Format::from_raw(fmt)?,
            size: BitSize::from_raw(siz)?,
            width: w,
            height: h,
            palette_offset: self.palette_offset,
            palette_entries: self.palette_entries,
        })
    }
}

/// Accumulates the mesh-wide vertex buffer plus the current material run.
#[derive(Default)]
struct Builder {
    material: MeshMaterial,
    /// Shared across every primitive in the mesh.
    vertices: Vec<MeshVertex>,
    /// Maps a vertex to its index, so repeated cache uploads collapse.
    seen: BTreeMap<MeshVertex, u16>,
    /// Indices for the material run currently being accumulated.
    indices: Vec<u16>,
}

impl Builder {
    fn push_vertex(&mut self, v: MeshVertex) -> Result<u16, MeshError> {
        if let Some(&i) = self.seen.get(&v) {
            return Ok(i);
        }
        let i = u16::try_from(self.vertices.len()).map_err(|_| MeshError::TooManyVertices)?;
        self.vertices.push(v);
        self.seen.insert(v, i);
        Ok(i)
    }

    /// Closes the current material run, keeping the shared vertex buffer.
    fn flush(&mut self, out: &mut Vec<Primitive>) {
        if self.indices.is_empty() {
            return;
        }
        out.push(Primitive {
            material: self.material,
            indices: core::mem::take(&mut self.indices),
        });
    }
}

/// Maximum `G_DL` nesting followed during conversion.
///
/// The RSP's own display list stack is 18 deep, so anything beyond that could
/// not have run on hardware and indicates a cycle in mis-identified data.
const MAX_DL_DEPTH: u32 = 18;

/// Converts one display list into a mesh.
///
/// `file` is the containing archive file's decompressed bytes; `G_VTX`
/// addresses are file-relative offsets after relocation.
///
/// `G_DL` calls are followed and **inlined**, sharing vertex-cache and material
/// state with the caller. That is not an optimisation, it is required for
/// correctness: Smash uses continuation lists that draw triangles from a cache
/// their *caller* filled, so converting such a list standalone fails with
/// [`MeshError::EmptyCacheSlot`].
pub fn convert(cmds: &[Cmd], file: &[u8]) -> Result<Mesh, MeshError> {
    let mut state = State::new();
    let mut builder = Builder::default();
    let mut out: Vec<Primitive> = Vec::new();

    walk(cmds, file, &mut state, &mut builder, &mut out, 0)?;
    builder.flush(&mut out);

    Ok(Mesh {
        vertices: builder.vertices,
        primitives: merge_by_material(out),
    })
}

fn walk(
    cmds: &[Cmd],
    file: &[u8],
    state: &mut State,
    builder: &mut Builder,
    out: &mut Vec<Primitive>,
    depth: u32,
) -> Result<(), MeshError> {
    let state = &mut *state;
    let builder = &mut *builder;

    for cmd in cmds {
        match *cmd {
            Cmd::Vtx {
                count,
                dest_index,
                addr,
            } => {
                let base = addr.0 as usize;
                for i in 0..count as usize {
                    let at = base + i * Vtx::SIZE;
                    let raw = file
                        .get(at..at + Vtx::SIZE)
                        .ok_or(MeshError::VertexDataOutOfBounds { offset: at as u32 })?;
                    let v = Vtx::parse(raw)
                        .map_err(|_| MeshError::VertexDataOutOfBounds { offset: at as u32 })?;
                    let slot = dest_index as usize + i;
                    if slot < state.cache.len() {
                        state.cache[slot] = Some(MeshVertex {
                            pos: v.pos,
                            uv: v.uv,
                            rgba: v.rgba,
                        });
                    }
                }
            }

            Cmd::Tri1(t) => emit_tri(builder, state, t)?,
            Cmd::Tri2(a, b) => {
                emit_tri(builder, state, a)?;
                emit_tri(builder, state, b)?;
            }

            // Inline the callee, sharing cache and material state.
            Cmd::Call(addr) | Cmd::Branch(addr) => {
                let tail = matches!(cmd, Cmd::Branch(_));
                // Segmented targets (e.g. segment 0x0E, the runtime graphics
                // heap) are resolved by the RSP at draw time and simply do not
                // exist in the file; skip them rather than treating them as
                // offsets.
                if depth < MAX_DL_DEPTH && addr.segment() == 0 {
                    let at = addr.0 as usize;
                    if at < file.len() {
                        if let Ok(sub) = crate::dl::decode_list(&file[at..]) {
                            walk(&sub, file, state, builder, out, depth + 1)?;
                        }
                    }
                }
                // G_DL in branch mode does not return to the caller.
                if tail {
                    break;
                }
            }

            // ---- material state ------------------------------------------
            Cmd::SetTimg {
                format, size, addr, ..
            } => state.timg = Some((addr.0, format, size)),

            Cmd::SetTile { format, size, .. } => {
                // A SETTILE for the texture itself refines the pending image's
                // format; the TLUT-loading tile is distinguished by arriving
                // after a LoadTlut, which we handle there.
                if let Some((off, _, _)) = state.timg {
                    state.timg = Some((off, format, size));
                }
            }

            Cmd::SetTileSize {
                uls, ult, lrs, lrt, ..
            } => {
                // Bounds are 10.2 fixed point and inclusive, so the pixel count
                // is ((lr - ul) >> 2) + 1.
                let w = ((lrs.saturating_sub(uls)) >> 2) + 1;
                let h = ((lrt.saturating_sub(ult)) >> 2) + 1;
                state.tile_dims = Some((w, h));
            }

            Cmd::LoadTlut { count, .. } => {
                // The palette is whatever texture image is currently set.
                state.palette_offset = state.timg.map(|(off, _, _)| off);
                state.palette_entries = count;
                // The TLUT load consumes the pending image; the real texture
                // follows with its own SETTIMG.
                state.timg = None;
            }

            Cmd::Texture { on, .. } => state.texture_enabled = on,

            Cmd::GeometryMode { clear, set } => {
                let apply = |cur: bool, bit: u32| (cur && clear & bit == 0) || set & bit != 0;
                state.material.cull_back = apply(state.material.cull_back, G_CULL_BACK);
                state.material.cull_front = apply(state.material.cull_front, G_CULL_FRONT);
                state.material.lit = apply(state.material.lit, G_LIGHTING);
                state.material.smooth = apply(state.material.smooth, G_SHADING_SMOOTH);
                state.material.z_buffer = apply(state.material.z_buffer, G_ZBUFFER);
            }

            Cmd::SetPrimColor { rgba, .. } => state.material.prim_color = rgba,
            Cmd::SetEnvColor(c) => state.material.env_color = c,
            Cmd::SetBlendColor(c) => state.material.blend_color = c,

            Cmd::End => break,
            _ => continue,
        }

        // Splitting on every material change keeps each primitive homogeneous;
        // they are merged again at the end.
        let material = MeshMaterial {
            texture: state.current_texture(),
            ..state.material
        };
        if material != builder.material {
            builder.flush(out);
            builder.material = material;
        }
    }

    Ok(())
}

fn emit_tri(builder: &mut Builder, state: &State, tri: [u8; 3]) -> Result<(), MeshError> {
    for slot in tri {
        let v = state
            .cache
            .get(slot as usize)
            .copied()
            .flatten()
            .ok_or(MeshError::EmptyCacheSlot(slot))?;
        let idx = builder.push_vertex(v)?;
        builder.indices.push(idx);
    }
    Ok(())
}

/// Merges primitives that share a material into single draws.
///
/// This is the state-sorting step: on the GE, a draw call is cheap but a state
/// change is not, so collapsing N same-material runs into one draw is the
/// highest-value optimisation available at conversion time — and it costs
/// nothing at runtime.
///
/// With a shared vertex buffer this is a pure index-list concatenation -- no
/// re-indexing needed, which is both simpler and much faster than the
/// per-primitive-buffer version it replaced.
fn merge_by_material(prims: Vec<Primitive>) -> Vec<Primitive> {
    let mut by_material: BTreeMap<MeshMaterial, Vec<u16>> = BTreeMap::new();
    for p in prims {
        by_material
            .entry(p.material)
            .or_default()
            .extend_from_slice(&p.indices);
    }
    by_material
        .into_iter()
        .map(|(material, indices)| Primitive { material, indices })
        .collect()
}

// Geometry mode bits (F3DEX2, from `gbi.h`).
const G_ZBUFFER: u32 = 0x0000_0001;
const G_CULL_FRONT: u32 = 0x0000_0200;
const G_CULL_BACK: u32 = 0x0000_0400;
const G_LIGHTING: u32 = 0x0002_0000;
const G_SHADING_SMOOTH: u32 = 0x0020_0000;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dl::SegAddr;

    /// Builds `n` vertices at file offset 0, each at a distinct position.
    fn vertex_data(n: usize) -> Vec<u8> {
        let mut d = Vec::new();
        for i in 0..n {
            let x = (i as i16) * 10;
            d.extend_from_slice(&x.to_be_bytes()); // x
            d.extend_from_slice(&0i16.to_be_bytes()); // y
            d.extend_from_slice(&0i16.to_be_bytes()); // z
            d.extend_from_slice(&0u16.to_be_bytes()); // pad
            d.extend_from_slice(&0i16.to_be_bytes()); // u
            d.extend_from_slice(&0i16.to_be_bytes()); // v
            d.extend_from_slice(&[0xFF, 0xFF, 0xFF, 0xFF]); // rgba
        }
        d
    }

    fn vtx(count: u8) -> Cmd {
        Cmd::Vtx {
            count,
            dest_index: 0,
            addr: SegAddr(0),
        }
    }

    #[test]
    fn converts_a_single_triangle() {
        let file = vertex_data(3);
        let cmds = [vtx(3), Cmd::Tri1([0, 1, 2]), Cmd::End];
        let mesh = convert(&cmds, &file).unwrap();

        assert_eq!(mesh.triangle_count(), 1);
        assert_eq!(mesh.vertex_count(), 3);
        assert_eq!(mesh.primitives[0].indices, [0, 1, 2]);
    }

    #[test]
    fn tri2_emits_two_triangles() {
        let file = vertex_data(6);
        let cmds = [vtx(6), Cmd::Tri2([0, 1, 2], [3, 4, 5]), Cmd::End];
        let mesh = convert(&cmds, &file).unwrap();
        assert_eq!(mesh.triangle_count(), 2);
        assert_eq!(mesh.vertex_count(), 6);
    }

    #[test]
    fn shared_vertices_are_deduplicated() {
        // Two triangles sharing an edge: 4 unique vertices, not 6.
        let file = vertex_data(4);
        let cmds = [vtx(4), Cmd::Tri2([0, 1, 2], [0, 2, 3]), Cmd::End];
        let mesh = convert(&cmds, &file).unwrap();
        assert_eq!(mesh.triangle_count(), 2);
        assert_eq!(mesh.vertex_count(), 4, "shared vertices must collapse");
        assert_eq!(mesh.primitives[0].indices, [0, 1, 2, 0, 2, 3]);
    }

    #[test]
    fn re_uploading_the_same_vertex_does_not_duplicate_it() {
        // The RSP cache is small, so lists re-upload vertices constantly. Two
        // identical G_VTX batches must not double the vertex count.
        let file = vertex_data(3);
        let cmds = [
            vtx(3),
            Cmd::Tri1([0, 1, 2]),
            vtx(3),
            Cmd::Tri1([0, 1, 2]),
            Cmd::End,
        ];
        let mesh = convert(&cmds, &file).unwrap();
        assert_eq!(mesh.triangle_count(), 2);
        assert_eq!(mesh.vertex_count(), 3, "re-upload must not duplicate");
    }

    #[test]
    fn material_change_splits_then_merges_back() {
        let file = vertex_data(3);
        let red = [255, 0, 0, 255];
        let blue = [0, 0, 255, 255];
        let cmds = [
            vtx(3),
            Cmd::SetPrimColor {
                m: 0,
                l: 0,
                rgba: red,
            },
            Cmd::Tri1([0, 1, 2]),
            Cmd::SetPrimColor {
                m: 0,
                l: 0,
                rgba: blue,
            },
            Cmd::Tri1([0, 1, 2]),
            Cmd::SetPrimColor {
                m: 0,
                l: 0,
                rgba: red,
            },
            Cmd::Tri1([0, 1, 2]),
            Cmd::End,
        ];
        let mesh = convert(&cmds, &file).unwrap();
        // Three runs, two distinct materials -> two draws, not three.
        assert_eq!(mesh.primitives.len(), 2, "same material must merge");
        assert_eq!(mesh.triangle_count(), 3);

        let red_prim = mesh
            .primitives
            .iter()
            .find(|p| p.material.prim_color == red)
            .expect("red primitive");
        assert_eq!(red_prim.triangle_count(), 2, "both red runs merged");
    }

    #[test]
    fn geometry_mode_sets_and_clears() {
        let file = vertex_data(3);
        let cmds = [
            vtx(3),
            Cmd::GeometryMode {
                clear: 0,
                set: G_CULL_BACK | G_LIGHTING,
            },
            Cmd::Tri1([0, 1, 2]),
            Cmd::End,
        ];
        let mesh = convert(&cmds, &file).unwrap();
        let m = mesh.primitives[0].material;
        assert!(m.cull_back);
        assert!(m.lit);
        assert!(!m.cull_front);

        // Now clear culling.
        let cmds = [
            vtx(3),
            Cmd::GeometryMode {
                clear: 0,
                set: G_CULL_BACK,
            },
            Cmd::GeometryMode {
                clear: G_CULL_BACK,
                set: 0,
            },
            Cmd::Tri1([0, 1, 2]),
            Cmd::End,
        ];
        let mesh = convert(&cmds, &file).unwrap();
        assert!(!mesh.primitives[0].material.cull_back);
    }

    #[test]
    fn tile_size_converts_10_2_fixed_point_to_pixels() {
        let file = vertex_data(3);
        // 0..=(31<<2) inclusive in 10.2 -> 32 pixels.
        let cmds = [
            vtx(3),
            Cmd::SetTimg {
                format: 2,
                size: 0,
                width: 32,
                addr: SegAddr(0x100),
            },
            Cmd::SetTileSize {
                tile: 0,
                uls: 0,
                ult: 0,
                lrs: 31 << 2,
                lrt: 15 << 2,
            },
            Cmd::Texture {
                level: 0,
                tile: 0,
                on: true,
                scale_s: 0,
                scale_t: 0,
            },
            Cmd::Tri1([0, 1, 2]),
            Cmd::End,
        ];
        let mesh = convert(&cmds, &file).unwrap();
        let tex = mesh.primitives[0].material.texture.expect("texture bound");
        assert_eq!((tex.width, tex.height), (32, 16));
        assert_eq!(tex.format, Format::Ci);
        assert_eq!(tex.size, BitSize::Bits4);
    }

    #[test]
    fn texture_disabled_means_no_binding() {
        let file = vertex_data(3);
        let cmds = [
            vtx(3),
            Cmd::SetTimg {
                format: 0,
                size: 2,
                width: 8,
                addr: SegAddr(0x100),
            },
            Cmd::SetTileSize {
                tile: 0,
                uls: 0,
                ult: 0,
                lrs: 7 << 2,
                lrt: 7 << 2,
            },
            Cmd::Tri1([0, 1, 2]),
            Cmd::End,
        ];
        let mesh = convert(&cmds, &file).unwrap();
        assert!(mesh.primitives[0].material.texture.is_none());
    }

    #[test]
    fn triangle_referencing_an_unloaded_slot_is_an_error() {
        let file = vertex_data(3);
        let cmds = [Cmd::Tri1([0, 1, 2]), Cmd::End];
        assert!(matches!(
            convert(&cmds, &file),
            Err(MeshError::EmptyCacheSlot(0))
        ));
    }

    #[test]
    fn vertex_pointer_past_end_of_file_is_an_error() {
        let file = vertex_data(1);
        let cmds = [vtx(4), Cmd::Tri1([0, 1, 2]), Cmd::End];
        assert!(matches!(
            convert(&cmds, &file),
            Err(MeshError::VertexDataOutOfBounds { .. })
        ));
    }

    #[test]
    fn empty_list_produces_empty_mesh() {
        let mesh = convert(&[Cmd::End], &[]).unwrap();
        assert_eq!(mesh.triangle_count(), 0);
        assert!(mesh.primitives.is_empty());
    }
}
