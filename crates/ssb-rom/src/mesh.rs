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
///
/// "Where" is a *file* and an offset, not an offset alone. A stage's display
/// lists live in one archive file and its texels in another; the pointer
/// between them is an extern relocation the archive records rather than
/// applies, so it reads as zero in the list (RE-037). `data_file` is `None`
/// for the ordinary case where the texels are in the same file as the list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct TextureRef {
    /// Archive file holding the texel data, when it is not the list's own.
    pub data_file: Option<u16>,
    /// Byte offset of the texel data within that file.
    pub data_offset: u32,
    pub format: Format,
    pub size: BitSize,
    pub width: u16,
    pub height: u16,
    /// Archive file holding the palette, when it is not the list's own.
    pub palette_file: Option<u16>,
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
    /// `G_SETPRIMCOLOR`, when the list or an `MObj` set one.
    ///
    /// `None` is not the same as black: most of Mario's model is flat-shaded,
    /// its vertices carry a **greyscale shade** rather than a colour, and the
    /// colour comes entirely from here. A primitive that never set one must
    /// keep its shade unmodulated (RE-039).
    pub prim_color: Option<[u8; 4]>,
    /// `G_SETENVCOLOR`. `None` for the same reason as `prim_color`: a combiner
    /// that reads one nothing set is reading whatever the RDP had, and black
    /// is not a safe stand-in for that.
    pub env_color: Option<[u8; 4]>,
    pub blend_color: Option<[u8; 4]>,
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

/// A cached vertex plus the space it was loaded in.
///
/// `G_VTX` transforms vertices by the modelview matrix *as it stands at load
/// time*, and the cache survives across display lists. A fighter therefore
/// stitches its joints together by loading half a triangle's vertices under one
/// joint's matrix and the other half under the next joint's — the N64's version
/// of skinning. `space` records which node was current, so [`convert_sequence`]
/// can put the vertex back where it belongs.
#[derive(Debug, Clone, Copy)]
struct CacheEntry {
    vertex: MeshVertex,
    space: u16,
}

/// The file a display list is read out of, and where its pointers lead.
///
/// Carrying the relocations rather than just the bytes is what lets a texture
/// in another archive file be found at all: the pointer word itself is zero,
/// and the only record of what it meant is keyed by the slot's offset.
#[derive(Clone, Copy)]
pub struct Source<'a> {
    pub data: &'a [u8],
    /// The file's extern relocation slots, as the archive recorded them.
    pub externs: &'a [crate::archive::ExternReloc],
}

impl<'a> Source<'a> {
    /// A file whose cross-file pointers are not available.
    ///
    /// Fine for a list that has none — most do — and for tests. A list that
    /// does have them will convert with those textures missing rather than
    /// wrong, which is what the pre-RE-037 behaviour was everywhere.
    pub fn bare(data: &'a [u8]) -> Self {
        Source { data, externs: &[] }
    }

    /// A loaded archive file, relocations included.
    pub fn of(file: &'a crate::archive::File) -> Self {
        Source {
            data: &file.data,
            externs: &file.extern_relocs,
        }
    }

    /// What the pointer word at `slot` targets, if it left this file.
    fn extern_at(&self, slot: u32) -> Option<(u16, u32)> {
        self.externs
            .iter()
            .find(|r| r.at == slot)
            .map(|r| (r.target_file, r.target_offset))
    }
}

/// A colour as far as a static converter can follow it.
///
/// The RDP combiner computes `(A - B) * C + D` per cycle, with the previous
/// cycle's result available to the next. Two of its four inputs vary per
/// vertex or per texel and the rest are constants, so any result this converter
/// can use has the shape
///
/// ```text
/// k + s*SHADE + t*TEXEL + st*SHADE*TEXEL
/// ```
///
/// per channel. `SHADE` is the vertex colour and `TEXEL` is what the GE's
/// texture unit supplies, so a result of `s*SHADE` folds into the vertex and a
/// result of `st*SHADE*TEXEL` is exactly `GU_TFX_MODULATE`. Anything else —
/// an additive term, a texture in an unmodulated position — is left alone
/// rather than approximated.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Combined {
    k: [f32; 3],
    s: [f32; 3],
    t: [f32; 3],
    st: [f32; 3],
}

impl Combined {
    const ZERO: Combined = Combined {
        k: [0.0; 3],
        s: [0.0; 3],
        t: [0.0; 3],
        st: [0.0; 3],
    };

    fn constant(c: [f32; 3]) -> Combined {
        Combined {
            k: c,
            ..Combined::ZERO
        }
    }

    /// Whether every varying term is zero, so this is a plain colour.
    fn is_constant(&self) -> bool {
        self.s == [0.0; 3] && self.t == [0.0; 3] && self.st == [0.0; 3]
    }

    fn zip(&self, o: &Combined, f: impl Fn(f32, f32) -> f32) -> Combined {
        let g = |a: [f32; 3], b: [f32; 3]| [f(a[0], b[0]), f(a[1], b[1]), f(a[2], b[2])];
        Combined {
            k: g(self.k, o.k),
            s: g(self.s, o.s),
            t: g(self.t, o.t),
            st: g(self.st, o.st),
        }
    }

    fn sub(&self, o: &Combined) -> Combined {
        self.zip(o, |a, b| a - b)
    }

    fn add(&self, o: &Combined) -> Combined {
        self.zip(o, |a, b| a + b)
    }

    /// `self * o`, when one side is constant.
    ///
    /// Two varying terms multiplied — `SHADE * TEXEL` aside, which the shapes
    /// below cover — is not representable, and returning `None` is how a
    /// combiner this model cannot follow declines to be guessed at.
    fn mul(&self, o: &Combined) -> Option<Combined> {
        let (c, v) = if o.is_constant() {
            (o.k, self)
        } else if self.is_constant() {
            (self.k, o)
        } else {
            // The one mixed product worth keeping: shade times texel.
            let shade_by_texel = |a: &Combined, b: &Combined| {
                (a.s != [0.0; 3]
                    && b.t != [0.0; 3]
                    && a.k == [0.0; 3]
                    && a.t == [0.0; 3]
                    && a.st == [0.0; 3]
                    && b.k == [0.0; 3]
                    && b.s == [0.0; 3]
                    && b.st == [0.0; 3])
                    .then(|| Combined {
                        st: [a.s[0] * b.t[0], a.s[1] * b.t[1], a.s[2] * b.t[2]],
                        ..Combined::ZERO
                    })
            };
            return shade_by_texel(self, o).or_else(|| shade_by_texel(o, self));
        };
        let g = |a: [f32; 3]| [a[0] * c[0], a[1] * c[1], a[2] * c[2]];
        Some(Combined {
            k: g(v.k),
            s: g(v.s),
            t: g(v.t),
            st: g(v.st),
        })
    }
}

fn to_f(c: [u8; 4]) -> [f32; 3] {
    [
        c[0] as f32 / 255.0,
        c[1] as f32 / 255.0,
        c[2] as f32 / 255.0,
    ]
}

/// Resolves one combiner input.
///
/// The four multiplexers have different widths *and* different meanings for
/// the same code, which is why they are decoded separately rather than through
/// one table. `None` is a source this model cannot follow.
fn source(code: u32, slot: u8, prim: [f32; 3], env: [f32; 3]) -> Option<Combined> {
    const SHADE: Combined = Combined {
        s: [1.0; 3],
        ..Combined::ZERO
    };
    const TEXEL: Combined = Combined {
        t: [1.0; 3],
        ..Combined::ZERO
    };
    Some(match (code, slot) {
        (0, _) => return None, // COMBINED: substituted by the caller
        (1 | 2, _) => TEXEL,
        (3, _) => Combined::constant(prim),
        (4, _) => SHADE,
        (5, _) => Combined::constant(env),
        // A and D read 1 here; B reads CENTER and C reads SCALE, neither of
        // which this model follows.
        (6, b'a') | (6, b'd') => Combined::constant([1.0; 3]),
        // A's 7 is NOISE; D's 7 is zero; B's is K4 and C's is an alpha.
        (7, b'd') => Combined::ZERO,
        // Everything past each multiplexer's named range reads zero.
        (8..=15, b'a') | (8..=15, b'b') | (16..=31, b'c') => Combined::ZERO,
        _ => return None,
    })
}

/// Runs one combiner cycle, given the previous cycle's result.
fn cycle(
    srcs: [u32; 4],
    prev: Option<&Combined>,
    prim: [f32; 3],
    env: [f32; 3],
) -> Option<Combined> {
    let mut v = [Combined::ZERO; 4];
    for (i, (&code, slot)) in srcs.iter().zip(b"abcd").enumerate() {
        v[i] = match source(code, *slot, prim, env) {
            Some(x) => x,
            // Only `COMBINED` returns None for a code the model knows.
            None if code == 0 => *prev?,
            None => return None,
        };
    }
    v[0].sub(&v[1]).mul(&v[2]).map(|m| m.add(&v[3]))
}

/// What a primitive's colour reduces to: a constant to fold into the vertex
/// shade, or nothing when the combiner is one this model does not follow.
///
/// Both cycles are evaluated. Reading only the first is what left Mario's
/// gloves and Dream Land's platforms white — their first cycle is a bare
/// `SHADE` and everything that gives it a colour is in the second (RE-043).
fn combiner_shade_scale(
    hi: u32,
    lo: u32,
    two_cycle: bool,
    prim: Option<[u8; 4]>,
    env: Option<[u8; 4]>,
) -> Option<[f32; 3]> {
    // An unset constant is the multiplicative identity rather than black: a
    // combiner that reads one the display list never set is reading whatever
    // the RDP had, and white is the only choice that cannot darken geometry
    // that should be lit.
    let p = prim.map_or([1.0; 3], to_f);
    let e = env.map_or([1.0; 3], to_f);

    let c0 = cycle(
        [
            (hi >> 20) & 0xF,
            (lo >> 28) & 0xF,
            (hi >> 15) & 0x1F,
            (lo >> 15) & 0x7,
        ],
        None,
        p,
        e,
    )?;
    let out = if two_cycle {
        cycle(
            [
                (hi >> 5) & 0xF,
                (lo >> 24) & 0xF,
                hi & 0x1F,
                (lo >> 6) & 0x7,
            ],
            Some(&c0),
            p,
            e,
        )?
    } else {
        c0
    };

    // Usable only as a scale on the shade. A constant term would need a second
    // colour source the vertex format does not have.
    if out.k != [0.0; 3] || out.t != [0.0; 3] {
        return None;
    }
    match (out.s == [0.0; 3], out.st == [0.0; 3]) {
        // `SHADE * TEXEL` is what the GE's modulate already does.
        (true, false) => Some(out.st),
        (false, true) => Some(out.s),
        _ => None,
    }
}

/// Tracks RDP/RSP state while walking a display list./// Tracks RDP/RSP state while walking a display list.
struct State {
    cache: [Option<CacheEntry>; VTX_CACHE_SIZE as usize],
    /// The space `G_VTX` loads into and triangles are emitted in.
    space: u16,
    /// World matrix of each space, and the inverse of the current one. Empty
    /// for a standalone conversion, where every vertex is in the same space.
    spaces: Vec<crate::scene::Mat4>,
    inv_current: crate::scene::Mat4,
    material: MeshMaterial,
    /// Address of the current texture image, from `G_SETTIMG`.
    ///
    /// Deliberately separate from the format below. `G_SETTIMG`'s own format
    /// and size describe the *load*, not the render: for a CI4 texture loaded
    /// via `G_LOADBLOCK` the SETTIMG typically reads RGBA16. Letting SETTIMG
    /// set the format produced impossible pairs like `(Ci, Bits16)` and failed
    /// 294 texture conversions.
    timg_addr: Option<u32>,
    /// File the current texture image lives in, when not this one.
    timg_file: Option<u16>,
    /// Render format from `G_SETTILE` on tile 0 — the authoritative one.
    tile0_fmt: Option<(u8, u8)>,
    tile_dims: Option<(u16, u16)>,
    /// `G_SETTILE`'s `masks`/`maskt` on tile 0: the texture wraps every
    /// `1 << mask` texels, and zero means it does not wrap at all.
    tile0_mask: Option<(u8, u8)>,
    palette_offset: Option<u32>,
    palette_file: Option<u16>,
    palette_entries: u16,
    texture_enabled: bool,
    /// The `G_SETCOMBINE` words in force, if one has been set. A list that
    /// draws before setting one is a list whose colour cannot be resolved, and
    /// leaving the shade alone is the safe answer.
    combiner: Option<(u32, u32)>,
    /// Whether `G_SETOTHERMODE_H` put the RDP in two-cycle mode. Cycle 1 is
    /// only run when it did — applying it in one-cycle mode would invent a
    /// multiply the hardware never performs.
    two_cycle: bool,
    /// The current node's `MObj` chain; see `SequenceItem::mobjs`.
    mobjs: Vec<crate::mobj::MObjMaterial>,
}

impl State {
    fn new() -> Self {
        State {
            cache: [None; VTX_CACHE_SIZE as usize],
            space: 0,
            spaces: Vec::new(),
            inv_current: crate::scene::Mat4::IDENTITY,
            material: MeshMaterial::default(),
            timg_addr: None,
            timg_file: None,
            tile0_fmt: None,
            tile_dims: None,
            tile0_mask: None,
            palette_offset: None,
            palette_file: None,
            palette_entries: 0,
            texture_enabled: false,
            combiner: None,
            two_cycle: false,
            mobjs: Vec::new(),
        }
    }

    /// Moves a cached vertex into the current space.
    ///
    /// The overwhelmingly common case is that it was loaded here, and that path
    /// is returned untouched — going through `inv * world` would perturb every
    /// coordinate by a float ulp or two and, since positions round back to
    /// `i16`, could shift a vertex by a whole game unit for no reason.
    fn rebase(&self, e: CacheEntry) -> MeshVertex {
        if e.space == self.space {
            return e.vertex;
        }
        let Some(world) = self.spaces.get(e.space as usize) else {
            return e.vertex;
        };
        let p = e.vertex.pos.map(|c| c as f32);
        let local = self.inv_current.transform_point(world.transform_point(p));
        MeshVertex {
            pos: local.map(|c| {
                // `as` saturates on overflow and truncates towards zero, so
                // round explicitly first.
                let r = if c < 0.0 { c - 0.5 } else { c + 0.5 };
                r.clamp(i16::MIN as f32, i16::MAX as f32) as i16
            }),
            ..e.vertex
        }
    }

    /// Replays the commands one `MObj` contributes.
    ///
    /// The order matters and is `gcDrawMObjForDObj`'s: the palette is set as
    /// the texture image first, then loaded as a TLUT if this `MObj` does that
    /// itself, then the sprite overwrites the image address. A node whose
    /// `MObj` only supplies a palette leaves the `G_LOADTLUT` to its own
    /// display list, which is the common fighter case.
    fn apply_mobj(&mut self, m: &crate::mobj::MObjMaterial) {
        if let Some(palette) = m.palette {
            self.timg_addr = Some(palette.offset);
            self.timg_file = palette.file;
            if m.loads_tlut {
                self.palette_offset = Some(palette.offset);
                self.palette_file = palette.file;
                self.palette_entries = m.palette_entries;
                self.timg_addr = None;
            }
        }
        if let Some(sprite) = m.sprite {
            self.timg_addr = Some(sprite.offset);
            self.timg_file = sprite.file;
        }
        if let Some(c) = m.prim_color {
            self.material.prim_color = Some(c);
        }
        if m.env_color.is_some() {
            self.material.env_color = m.env_color;
        }
        if m.blend_color.is_some() {
            self.material.blend_color = m.blend_color;
        }
    }

    /// Drops the texture binding a call we cannot follow would have replaced.
    fn forget_texture(&mut self) {
        self.timg_addr = None;
        self.timg_file = None;
        self.palette_offset = None;
        self.palette_file = None;
        self.texture_enabled = false;
    }

    /// The material a primitive emitted right now would carry.
    ///
    /// `prim_color` is not the raw `G_SETPRIMCOLOR` but what the *combiner*
    /// makes of the whole state — the constant the shade is multiplied by. A
    /// combiner this model cannot follow leaves it `None`, and the shade is
    /// used unmodified, which is what the renderer did before any of this.
    fn material_now(&self) -> MeshMaterial {
        let scale = self.combiner.and_then(|(hi, lo)| {
            combiner_shade_scale(
                hi,
                lo,
                self.two_cycle,
                self.material.prim_color,
                self.material.env_color,
            )
        });
        MeshMaterial {
            texture: self.current_texture(),
            // Identity is not worth storing: it means "use the shade as it is".
            prim_color: scale.filter(|s| *s != [1.0; 3]).map(|s| {
                [
                    (s[0] * 255.0).clamp(0.0, 255.0) as u8,
                    (s[1] * 255.0).clamp(0.0, 255.0) as u8,
                    (s[2] * 255.0).clamp(0.0, 255.0) as u8,
                    255,
                ]
            }),
            ..self.material
        }
    }

    /// Assembles the current texture binding, if one is fully specified.
    fn current_texture(&self) -> Option<TextureRef> {
        if !self.texture_enabled {
            return None;
        }
        let offset = self.timg_addr?;
        let (fmt, siz) = self.tile0_fmt?;
        let (w, h) = self.tile_dims?;
        // `G_SETTILESIZE` gives the rectangle being *drawn*, which for a
        // wrapping texture is larger than the texture: Dream Land renders a
        // 64x32 tile across a 256x128 span. `masks`/`maskt` are what say how
        // big the texture really is -- it repeats every `1 << mask` texels --
        // and taking the drawn rect instead asks for 16 KiB of texels out of a
        // 12 KiB file (RE-044). A mask of zero means no wrapping, so the drawn
        // rect is the texture.
        let (w, h) = match self.tile0_mask {
            Some((ms, mt)) => (
                if ms > 0 { w.min(1 << ms) } else { w },
                if mt > 0 { h.min(1 << mt) } else { h },
            ),
            None => (w, h),
        };
        Some(TextureRef {
            data_file: self.timg_file,
            data_offset: offset,
            format: Format::from_raw(fmt)?,
            size: BitSize::from_raw(siz)?,
            width: w,
            height: h,
            palette_file: self.palette_file,
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
    /// Adds a vertex, folding the primitive colour into its shade first.
    ///
    /// The N64 combiner multiplies `PRIM * SHADE`, and for a flat-shaded model
    /// the vertex bytes are the shade: Mario's are pure greys. Doing that
    /// multiply here rather than at draw time costs nothing at runtime and
    /// needs no second colour source in the vertex format — and the dedup
    /// below turns a vertex shared by two primitives of different colours into
    /// two entries by itself, because the folded colour is part of the key.
    fn push_vertex(&mut self, mut v: MeshVertex) -> Result<u16, MeshError> {
        if let Some(c) = self.material.prim_color {
            for (shade, &prim) in v.rgba.iter_mut().zip(c.iter()).take(3) {
                *shade = ((*shade as u16 * prim as u16) / 255) as u8;
            }
            // Shade alpha is not a coverage value here — Mario's vertices are
            // all zero — so the primitive's alpha is the one that means
            // something. Multiplying would make him invisible.
            v.rgba[3] = c[3];
        }
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
pub fn convert(cmds: &[Cmd], src: Source<'_>) -> Result<Mesh, MeshError> {
    let mut state = State::new();
    let mut builder = Builder::default();
    let mut out: Vec<Primitive> = Vec::new();

    walk(cmds, src, &mut state, &mut builder, &mut out, 0)?;
    builder.flush(&mut out);

    Ok(Mesh {
        vertices: builder.vertices,
        primitives: merge_by_material(out),
    })
}

/// One node's display list, ready to be drawn in sequence.
pub struct SequenceItem<'a> {
    pub cmds: &'a [Cmd],
    /// The node's world matrix — the modelview in effect while this list runs.
    pub world: crate::scene::Mat4,
    /// The node's `MObj` chain, indexed the way its segment-`0x0E` calls index
    /// it. Empty when we could not recover one, which is not the same as the
    /// node having none: see [`crate::mobj::PartTables`].
    pub mobjs: &'a [crate::mobj::MObjMaterial],
}

/// Converts display lists that share one RSP vertex cache, in draw order.
///
/// [`convert`] treats a list as self-contained, which is wrong for every
/// fighter in the game. `gcDrawDObjTree*` walks the node tree emitting each
/// node's list into a single command stream, so the vertex cache carries over:
/// a joint's list routinely draws triangles whose other vertices a *previous*
/// joint loaded. Converting those in isolation fails with
/// [`MeshError::EmptyCacheSlot`] — 144 of the archive's node lists did.
///
/// Because `G_VTX` bakes in the modelview at load time, such a triangle spans
/// two joint spaces. Each result is still a mesh in its own node's space, with
/// borrowed vertices carried across by `inv(world_here) * world_there`. That is
/// exact for the rest pose. It cannot survive animation — under a moving joint
/// the seam would tear — but reproducing that needs the runtime to keep the
/// cache, which is a decision for when animation lands.
///
/// RDP material state is threaded too, not just the cache — the hardware keeps
/// it, so a list that draws before setting a texture is drawing with whatever
/// the previous one bound. Resetting per list instead resolves 378 textures
/// against 394, so inheritance is a real but modest win.
///
/// It is also where this gets delicate. `gcDrawMObjForDObj` injects a material
/// display list at segment `0x0E` — the runtime graphics heap — and that is
/// where a fighter's palette comes from. When `mobjs` supplies it we replay
/// exactly the commands that function would have emitted. When it does not,
/// the call still invalidates the texture binding rather than letting the
/// previous node's survive: inheriting past one bound another joint's texels
/// over Samus's torso, and cost 117 spurious textures.
///
/// Returns one result per item, in the order given; a failing item does not
/// stop the rest, since its state contribution has still been applied.
pub fn convert_sequence(items: &[SequenceItem], src: Source<'_>) -> Vec<Result<Mesh, MeshError>> {
    let mut state = State::new();
    state.spaces = items.iter().map(|i| i.world).collect();

    let mut out = Vec::with_capacity(items.len());
    for (i, item) in items.iter().enumerate() {
        state.space = i as u16;
        state.mobjs = item.mobjs.to_vec();
        // A singular node matrix (zero scale) means nothing borrowed from
        // elsewhere can be expressed here; identity keeps it in its own space
        // rather than producing infinities.
        state.inv_current = item
            .world
            .inverse_affine()
            .unwrap_or(crate::scene::Mat4::IDENTITY);

        // Seed the builder from the state carried in, not from the default.
        // RDP state persists across lists exactly as the vertex cache does, and
        // starting at the default made every triangle a list emitted *before*
        // its first state command land in a spurious untextured primitive.
        let mut builder = Builder {
            material: state.material_now(),
            ..Builder::default()
        };
        let mut prims: Vec<Primitive> = Vec::new();
        let result = walk(item.cmds, src, &mut state, &mut builder, &mut prims, 0);
        builder.flush(&mut prims);

        out.push(result.map(|()| Mesh {
            vertices: builder.vertices,
            primitives: merge_by_material(prims),
        }));
    }
    out
}

fn walk(
    cmds: &[Cmd],
    src: Source<'_>,
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
                    let raw = src
                        .data
                        .get(at..at + Vtx::SIZE)
                        .ok_or(MeshError::VertexDataOutOfBounds { offset: at as u32 })?;
                    let v = Vtx::parse(raw)
                        .map_err(|_| MeshError::VertexDataOutOfBounds { offset: at as u32 })?;
                    let slot = dest_index as usize + i;
                    if slot < state.cache.len() {
                        state.cache[slot] = Some(CacheEntry {
                            vertex: MeshVertex {
                                pos: v.pos,
                                uv: v.uv,
                                rgba: v.rgba,
                            },
                            space: state.space,
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
                if addr.segment() == crate::mobj::GRAPHICS_HEAP_SEGMENT {
                    // The heap holds one 8-byte entry point per `MObj`, so the
                    // offset names which of the node's materials to apply.
                    let index = (addr.offset() / 8) as usize;
                    match state.mobjs.get(index) {
                        Some(m) => state.apply_mobj(&m.clone()),
                        // A material we know is there and cannot supply. Its
                        // whole purpose is to replace the binding, so keeping
                        // the previous list's is worse than dropping it.
                        None => state.forget_texture(),
                    }
                } else if addr.segment() != 0 {
                    state.forget_texture();
                }
                if depth < MAX_DL_DEPTH && addr.segment() == 0 {
                    let at = addr.0 as usize;
                    if at < src.data.len() {
                        // The sub-list's own offset, so any `G_SETTIMG` inside
                        // it can still find a cross-file texture.
                        if let Ok(sub) = crate::dl::decode_list_at(&src.data[at..], at as u32) {
                            walk(&sub, src, state, builder, out, depth + 1)?;
                        }
                    }
                }
                // G_DL in branch mode does not return to the caller.
                if tail {
                    break;
                }
            }

            // ---- material state ------------------------------------------
            // Address only; the render format comes from tile 0. See
            // `State::timg_addr` for why.
            // Address only; the render format comes from tile 0. A zero
            // address is not "no texture": the archive zeroes a pointer that
            // leaves the file and records it as an extern relocation instead,
            // which is how every stage reaches its texels (RE-037).
            Cmd::SetTimg { addr, slot, .. } => match (addr.0, src.extern_at(slot)) {
                (0, Some((target_file, offset))) => {
                    state.timg_addr = Some(offset);
                    state.timg_file = Some(target_file);
                }
                _ => {
                    state.timg_addr = Some(addr.0);
                    state.timg_file = None;
                }
            },

            Cmd::SetTile {
                format,
                size,
                tile,
                mask_s,
                mask_t,
                ..
            } => {
                // Only tile 0 (G_TX_RENDERTILE) describes the texture actually
                // sampled. A display list configures several tiles — tiles 5
                // and 7 stage TLUT loads — and taking whichever came last
                // picked up the palette tile's descriptor instead, yielding
                // impossible combinations like CI at 16 bits and failing 312
                // texture conversions.
                if tile == RENDER_TILE {
                    state.tile0_fmt = Some((format, size));
                    state.tile0_mask = Some((mask_s, mask_t));
                }
            }

            Cmd::SetTileSize {
                tile,
                uls,
                ult,
                lrs,
                lrt,
            } if tile == RENDER_TILE => {
                // Bounds are 10.2 fixed point and inclusive, so the pixel count
                // is ((lr - ul) >> 2) + 1.
                let w = ((lrs.saturating_sub(uls)) >> 2) + 1;
                let h = ((lrt.saturating_sub(ult)) >> 2) + 1;
                state.tile_dims = Some((w, h));
            }

            Cmd::LoadTlut { count, .. } => {
                // A TLUT load reads from whatever image address is current, so
                // that address *is* the palette — including which file it is
                // in. The real texture follows with its own SETTIMG.
                state.palette_offset = state.timg_addr;
                state.palette_file = state.timg_file;
                state.palette_entries = count;
                state.timg_addr = None;
                state.timg_file = None;
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

            Cmd::SetCombine { hi, lo } => state.combiner = Some((hi, lo)),

            // The cycle type is two bits at shift 20 of the high other-mode
            // word: 0 one-cycle, 1 two-cycle, then copy and fill.
            Cmd::SetOtherModeH { shift, len, data } if shift == 20 && len == 2 => {
                state.two_cycle = (data >> 20) & 0x3 == 1;
            }

            Cmd::SetPrimColor { rgba, .. } => state.material.prim_color = Some(rgba),
            Cmd::SetEnvColor(c) => state.material.env_color = Some(c),
            Cmd::SetBlendColor(c) => state.material.blend_color = Some(c),

            Cmd::End => break,
            _ => continue,
        }

        // Splitting on every material change keeps each primitive homogeneous;
        // they are merged again at the end.
        let material = state.material_now();
        if material != builder.material {
            builder.flush(out);
            builder.material = material;
        }
    }

    Ok(())
}

fn emit_tri(builder: &mut Builder, state: &State, tri: [u8; 3]) -> Result<(), MeshError> {
    for slot in tri {
        let e = state
            .cache
            .get(slot as usize)
            .copied()
            .flatten()
            .ok_or(MeshError::EmptyCacheSlot(slot))?;
        let idx = builder.push_vertex(state.rebase(e))?;
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

/// The tile the RDP samples when drawing (`G_TX_RENDERTILE`). Other tiles are
/// scratch used to stage TLUT and texture loads.
const RENDER_TILE: u8 = 0;

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
        let mesh = convert(&cmds, Source::bare(&file)).unwrap();

        assert_eq!(mesh.triangle_count(), 1);
        assert_eq!(mesh.vertex_count(), 3);
        assert_eq!(mesh.primitives[0].indices, [0, 1, 2]);
    }

    #[test]
    fn a_later_list_draws_from_the_cache_an_earlier_one_filled() {
        use crate::scene::Mat4;

        // Yoshi's shape, minimally: joint A loads two vertices and draws
        // nothing with them; joint B loads one more and draws a triangle
        // spanning both. Standalone that is EmptyCacheSlot(0) -- 144 of the
        // archive's node lists failed exactly this way.
        let file = vertex_data(3);
        let a = [
            Cmd::Vtx {
                count: 2,
                dest_index: 0,
                addr: SegAddr(0),
            },
            Cmd::End,
        ];
        let b = [
            Cmd::Vtx {
                count: 1,
                dest_index: 2,
                addr: SegAddr(2 * Vtx::SIZE as u32),
            },
            Cmd::Tri1([0, 1, 2]),
            Cmd::End,
        ];

        assert_eq!(
            convert(&b, Source::bare(&file)).unwrap_err(),
            MeshError::EmptyCacheSlot(0)
        );

        // Joint B sits 100 units along x from joint A.
        let items = [
            SequenceItem {
                cmds: &a,
                world: Mat4::IDENTITY,
                mobjs: &[],
            },
            SequenceItem {
                cmds: &b,
                world: Mat4::from_trs([100.0, 0.0, 0.0], [0.0; 3], [1.0; 3]),
                mobjs: &[],
            },
        ];
        let out = convert_sequence(&items, Source::bare(&file));

        assert_eq!(out[0].as_ref().unwrap().triangle_count(), 0);
        let mesh = out[1].as_ref().unwrap();
        assert_eq!(mesh.triangle_count(), 1);

        // vertex_data puts vertex i at x = 10i. The two borrowed from joint A
        // must land 100 units earlier in joint B's space; the one loaded here
        // must be untouched.
        let xs: Vec<i16> = mesh.vertices.iter().map(|v| v.pos[0]).collect();
        assert_eq!(xs, [-100, -90, 20]);
    }

    /// A CI4 joint list the way a fighter writes one: the palette's `G_SETTIMG`
    /// is missing because `gcDrawMObjForDObj` was going to emit it.
    fn ci4_list_calling_the_heap(entry: u32) -> [Cmd; 8] {
        [
            Cmd::SetTile {
                format: Format::Ci as u8,
                size: BitSize::Bits4 as u8,
                line: 2,
                tmem: 0,
                tile: 0,
                palette: 0,
                cm_s: 2,
                cm_t: 2,
                mask_s: 5,
                mask_t: 5,
                shift_s: 0,
                shift_t: 0,
            },
            Cmd::Call(SegAddr(0x0E00_0000 + entry)),
            Cmd::LoadTlut { tile: 5, count: 16 },
            Cmd::Texture {
                level: 0,
                tile: 0,
                on: true,
                scale_s: 0xFFFF,
                scale_t: 0xFFFF,
            },
            Cmd::SetTileSize {
                tile: 0,
                uls: 0,
                ult: 0,
                lrs: 124,
                lrt: 124,
            },
            Cmd::SetTimg {
                format: 0,
                size: 2,
                width: 1,
                addr: SegAddr(0x400),
                slot: 0,
            },
            vtx(3),
            Cmd::Tri1([0, 1, 2]),
        ]
    }

    #[test]
    fn a_heap_call_supplies_the_palette_the_list_then_loads() {
        use crate::mobj::MObjMaterial;
        use crate::scene::Mat4;

        let file = vertex_data(3);
        let cmds = ci4_list_calling_the_heap(8);
        // Entry 8 is index 1: the second `MObj` in the node's chain.
        let mobjs = [
            MObjMaterial {
                palette: Some(crate::mobj::Ptr {
                    file: None,
                    offset: 0x100,
                }),
                ..MObjMaterial::default()
            },
            MObjMaterial {
                palette: Some(crate::mobj::Ptr {
                    file: None,
                    offset: 0x200,
                }),
                ..MObjMaterial::default()
            },
        ];
        let items = [SequenceItem {
            cmds: &cmds,
            world: Mat4::IDENTITY,
            mobjs: &mobjs,
        }];
        let mesh = convert_sequence(&items, Source::bare(&file))
            .pop()
            .unwrap()
            .unwrap();
        let texture = mesh.primitives[0].material.texture.expect("bound texture");
        assert_eq!(texture.palette_offset, Some(0x200));
        assert_eq!(texture.data_offset, 0x400);
        assert_eq!(texture.format, Format::Ci);
    }

    #[test]
    fn without_a_material_the_heap_call_leaves_the_texture_unbound() {
        use crate::scene::Mat4;
        // The same list with no chain recovered. The `G_LOADTLUT` has no image
        // address to read, so there is no palette and hence no texture -- which
        // is the honest outcome, not a texture with someone else's palette.
        let file = vertex_data(3);
        let cmds = ci4_list_calling_the_heap(0);
        let items = [SequenceItem {
            cmds: &cmds,
            world: Mat4::IDENTITY,
            mobjs: &[],
        }];
        let mesh = convert_sequence(&items, Source::bare(&file))
            .pop()
            .unwrap()
            .unwrap();
        assert_eq!(mesh.triangle_count(), 1);
        let texture = mesh.primitives[0].material.texture.expect("bound texture");
        assert_eq!(texture.palette_offset, None);
    }

    #[test]
    fn tri2_emits_two_triangles() {
        let file = vertex_data(6);
        let cmds = [vtx(6), Cmd::Tri2([0, 1, 2], [3, 4, 5]), Cmd::End];
        let mesh = convert(&cmds, Source::bare(&file)).unwrap();
        assert_eq!(mesh.triangle_count(), 2);
        assert_eq!(mesh.vertex_count(), 6);
    }

    #[test]
    fn shared_vertices_are_deduplicated() {
        // Two triangles sharing an edge: 4 unique vertices, not 6.
        let file = vertex_data(4);
        let cmds = [vtx(4), Cmd::Tri2([0, 1, 2], [0, 2, 3]), Cmd::End];
        let mesh = convert(&cmds, Source::bare(&file)).unwrap();
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
        let mesh = convert(&cmds, Source::bare(&file)).unwrap();
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
            // A combiner that reads PRIMITIVE, so the colour is part of the
            // material rather than ignored.
            prim_times_shade(),
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
        let mesh = convert(&cmds, Source::bare(&file)).unwrap();
        // Three runs, two distinct materials -> two draws, not three.
        assert_eq!(mesh.primitives.len(), 2, "same material must merge");
        assert_eq!(mesh.triangle_count(), 3);

        let red_prim = mesh
            .primitives
            .iter()
            .find(|p| p.material.prim_color == Some(red))
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
        let mesh = convert(&cmds, Source::bare(&file)).unwrap();
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
        let mesh = convert(&cmds, Source::bare(&file)).unwrap();
        assert!(!mesh.primitives[0].material.cull_back);
    }

    #[test]
    fn tile_size_converts_10_2_fixed_point_to_pixels() {
        let file = vertex_data(3);
        // 0..=(31<<2) inclusive in 10.2 -> 32 pixels.
        let cmds = [
            vtx(3),
            // SETTIMG carries the address. Its format/size describe the *load*
            // and are deliberately wrong here (RGBA16, as real lists have for a
            // CI4 texture) to prove they are ignored.
            Cmd::SetTimg {
                format: 0,
                size: 2,
                width: 32,
                addr: SegAddr(0x100),
                slot: 0,
            },
            // Tile 0 is authoritative for the render format.
            Cmd::SetTile {
                format: 2,
                size: 0,
                line: 0,
                tmem: 0,
                tile: 0,
                palette: 0,
                cm_s: 0,
                cm_t: 0,
                mask_s: 0,
                mask_t: 0,
                shift_s: 0,
                shift_t: 0,
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
        let mesh = convert(&cmds, Source::bare(&file)).unwrap();
        let tex = mesh.primitives[0].material.texture.expect("texture bound");
        assert_eq!((tex.width, tex.height), (32, 16));
        assert_eq!(tex.format, Format::Ci, "tile 0 wins over SETTIMG");
        assert_eq!(tex.size, BitSize::Bits4);
    }

    /// Non-render tiles configure TLUT staging and must not affect the
    /// texture binding.
    #[test]
    fn non_render_tiles_are_ignored() {
        let file = vertex_data(3);
        let settile = |tile: u8, format: u8, size: u8| Cmd::SetTile {
            format,
            size,
            line: 0,
            tmem: 0,
            tile,
            palette: 0,
            cm_s: 0,
            cm_t: 0,
            mask_s: 0,
            mask_t: 0,
            shift_s: 0,
            shift_t: 0,
        };
        let cmds = [
            vtx(3),
            Cmd::SetTimg {
                format: 0,
                size: 2,
                width: 32,
                addr: SegAddr(0x100),
                slot: 0,
            },
            settile(0, 2, 0), // render tile: CI4
            settile(7, 2, 2), // TLUT staging tile: the impossible (Ci, 16b)
            settile(5, 0, 0), // another staging tile
            Cmd::SetTileSize {
                tile: 0,
                uls: 0,
                ult: 0,
                lrs: 31 << 2,
                lrt: 31 << 2,
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
        let mesh = convert(&cmds, Source::bare(&file)).unwrap();
        let tex = mesh.primitives[0].material.texture.expect("texture bound");
        assert_eq!(tex.format, Format::Ci);
        assert_eq!(tex.size, BitSize::Bits4, "staging tiles must not leak in");
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
                slot: 0,
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
        let mesh = convert(&cmds, Source::bare(&file)).unwrap();
        assert!(mesh.primitives[0].material.texture.is_none());
    }

    #[test]
    fn triangle_referencing_an_unloaded_slot_is_an_error() {
        let file = vertex_data(3);
        let cmds = [Cmd::Tri1([0, 1, 2]), Cmd::End];
        assert!(matches!(
            convert(&cmds, Source::bare(&file)),
            Err(MeshError::EmptyCacheSlot(0))
        ));
    }

    #[test]
    fn vertex_pointer_past_end_of_file_is_an_error() {
        let file = vertex_data(1);
        let cmds = [vtx(4), Cmd::Tri1([0, 1, 2]), Cmd::End];
        assert!(matches!(
            convert(&cmds, Source::bare(&file)),
            Err(MeshError::VertexDataOutOfBounds { .. })
        ));
    }

    #[test]
    fn empty_list_produces_empty_mesh() {
        let mesh = convert(&[Cmd::End], Source::bare(&[])).unwrap();
        assert_eq!(mesh.triangle_count(), 0);
        assert!(mesh.primitives.is_empty());
    }

    /// A list whose `G_SETTIMG` and TLUT pointers both left the file.
    ///
    /// The archive zeroes such a slot and records an extern relocation for it,
    /// so the list on its own says "address 0" for both halves.
    fn cross_file_ci4_list(timg_slot: u32, tlut_slot: u32) -> [Cmd; 9] {
        [
            Cmd::SetTile {
                format: Format::Ci as u8,
                size: BitSize::Bits4 as u8,
                line: 2,
                tmem: 0,
                tile: 0,
                palette: 0,
                cm_s: 2,
                cm_t: 2,
                mask_s: 5,
                mask_t: 5,
                shift_s: 0,
                shift_t: 0,
            },
            // Palette first: a TLUT load reads whatever image is current.
            Cmd::SetTimg {
                format: 0,
                size: 2,
                width: 1,
                addr: SegAddr(0),
                slot: tlut_slot,
            },
            Cmd::LoadTlut { tile: 5, count: 16 },
            Cmd::SetTimg {
                format: 0,
                size: 2,
                width: 1,
                addr: SegAddr(0),
                slot: timg_slot,
            },
            Cmd::Texture {
                level: 0,
                tile: 0,
                on: true,
                scale_s: 0xFFFF,
                scale_t: 0xFFFF,
            },
            Cmd::SetTileSize {
                tile: 0,
                uls: 0,
                ult: 0,
                lrs: 124,
                lrt: 124,
            },
            Cmd::Vtx {
                count: 3,
                dest_index: 0,
                addr: SegAddr(0),
            },
            Cmd::Tri1([0, 1, 2]),
            Cmd::End,
        ]
    }

    #[test]
    fn a_texture_pointer_that_left_the_file_resolves_through_the_relocation() {
        use crate::archive::ExternReloc;
        // Both halves point into file 103, at different offsets -- the shape
        // every stage has: geometry in one file, texels and palette in another
        // (RE-037).
        let file = vertex_data(3);
        let cmds = cross_file_ci4_list(0x40, 0x20);
        let externs = [
            ExternReloc {
                at: 0x20,
                target_file: 103,
                target_offset: 0x900,
            },
            ExternReloc {
                at: 0x40,
                target_file: 103,
                target_offset: 0x1200,
            },
        ];
        let src = Source {
            data: &file,
            externs: &externs,
        };
        let mesh = convert(&cmds, src).unwrap();
        let t = mesh.primitives[0].material.texture.expect("bound texture");
        assert_eq!(t.data_file, Some(103));
        assert_eq!(t.data_offset, 0x1200);
        assert_eq!(t.palette_file, Some(103));
        assert_eq!(t.palette_offset, Some(0x900));
    }

    #[test]
    fn without_the_relocations_the_same_list_binds_nothing_resolvable() {
        // The pre-RE-037 behaviour, kept as the honest fallback: an address of
        // zero with no record of what it meant is not a texture at offset
        // zero. It must not silently sample the head of the file.
        let file = vertex_data(3);
        let cmds = cross_file_ci4_list(0x40, 0x20);
        let mesh = convert(&cmds, Source::bare(&file)).unwrap();
        let t = mesh.primitives[0].material.texture.expect("bound texture");
        assert_eq!(t.data_file, None);
        assert_eq!(t.data_offset, 0);
    }

    #[test]
    fn a_relocation_for_a_different_slot_does_not_resolve_this_one() {
        // The lookup is keyed by the address word's own offset. Taking any
        // extern relocation in the file would give a stage every other
        // stage's textures.
        use crate::archive::ExternReloc;
        let file = vertex_data(3);
        let cmds = cross_file_ci4_list(0x40, 0x20);
        let externs = [ExternReloc {
            at: 0x48,
            target_file: 103,
            target_offset: 0x1200,
        }];
        let src = Source {
            data: &file,
            externs: &externs,
        };
        let mesh = convert(&cmds, src).unwrap();
        let t = mesh.primitives[0].material.texture.expect("bound texture");
        assert_eq!(t.data_file, None);
    }

    #[test]
    fn a_primitive_colour_the_combiner_ignores_does_not_reach_the_material() {
        // Mario's gloves and shoes take SHADE alone. Folding in whatever
        // primitive colour was last set turns them green, which is exactly
        // what happened (RE-039).
        let file = vertex_data(3);
        let cmds = [
            vtx(3),
            // (A - B) * C + D with every source something other than PRIM.
            Cmd::SetCombine {
                hi: 0x00FF_FE05,
                lo: 0xFF16_7DFF,
            },
            Cmd::SetPrimColor {
                m: 0,
                l: 0,
                rgba: [0, 206, 0, 255],
            },
            Cmd::Tri1([0, 1, 2]),
            Cmd::End,
        ];
        let mesh = convert(&cmds, Source::bare(&file)).unwrap();
        assert_eq!(mesh.primitives[0].material.prim_color, None);
        // And the shade is left exactly as the vertex carried it.
        assert_eq!(mesh.vertices[0].rgba, [0xFF; 4], "vertex_data writes white");
    }

    #[test]
    fn a_primitive_colour_the_combiner_reads_is_folded_into_the_shade() {
        // `PRIM * SHADE` is the combiner Mario's arms and thighs use, and the
        // vertices carry a grey shade rather than a colour.
        let file = vertex_data(3);
        let cmds = [
            vtx(3),
            prim_times_shade(),
            Cmd::SetPrimColor {
                m: 0,
                l: 0,
                rgba: [255, 0, 0, 255],
            },
            Cmd::Tri1([0, 1, 2]),
            Cmd::End,
        ];
        let mesh = convert(&cmds, Source::bare(&file)).unwrap();
        let got = mesh.vertices[0].rgba;
        assert_eq!(got[0], 0xFF, "red passes through unchanged");
        assert_eq!(got[1], 0, "green is multiplied out");
        assert_eq!(got[2], 0, "and so is blue");
        assert_eq!(got[3], 255, "alpha comes from the primitive colour");
    }

    /// A `G_SETCOMBINE` computing `PRIM * SHADE`, the one Mario's arms use.
    fn prim_times_shade() -> Cmd {
        let (hi, lo) = combine(PRIM, ZERO_A, SHADE, ZERO_D, 0, 0, 0, 0);
        Cmd::SetCombine { hi, lo }
    }

    /// `(A - B) * C + D` packed the way `gDPSetCombineLERP` does.
    #[allow(clippy::too_many_arguments)] // eight multiplexer codes is the format
    const fn combine(
        a: u32,
        b: u32,
        c: u32,
        d: u32,
        a1: u32,
        b1: u32,
        c1: u32,
        d1: u32,
    ) -> (u32, u32) {
        (
            (a << 20) | (c << 15) | (a1 << 5) | c1,
            (b << 28) | (b1 << 24) | (d << 15) | (d1 << 6),
        )
    }

    const COMBINED: u32 = 0;
    const TEXEL0: u32 = 1;
    const PRIM: u32 = 3;
    const SHADE: u32 = 4;
    const ENV: u32 = 5;
    const ZERO_A: u32 = 8;
    const ZERO_C: u32 = 16;
    const ZERO_D: u32 = 7;

    #[test]
    fn prim_times_shade_reduces_to_the_primitive_colour() {
        // Mario's upper arms and thighs.
        let (hi, lo) = combine(PRIM, ZERO_A, SHADE, ZERO_D, 0, 0, 0, 0);
        let got = combiner_shade_scale(hi, lo, false, Some([255, 0, 0, 255]), None);
        assert_eq!(got, Some([1.0, 0.0, 0.0]));
    }

    #[test]
    fn shade_alone_reduces_to_the_identity() {
        let (hi, lo) = combine(ZERO_A, ZERO_A, ZERO_C, SHADE, 0, 0, 0, 0);
        assert_eq!(
            combiner_shade_scale(hi, lo, false, None, None),
            Some([1.0; 3])
        );
    }

    #[test]
    fn the_second_cycle_is_run_only_in_two_cycle_mode() {
        // Cycle 0 is a bare SHADE and cycle 1 multiplies it by ENV. Reading
        // only the first is what left Mario's gloves and Dream Land's
        // platforms white (RE-043); running the second in one-cycle mode would
        // invent a multiply the hardware never performs.
        let (hi, lo) = combine(ZERO_A, ZERO_A, ZERO_C, SHADE, COMBINED, ZERO_A, ENV, ZERO_D);
        let env = Some([128, 64, 0, 255]);
        assert_eq!(
            combiner_shade_scale(hi, lo, false, None, env),
            Some([1.0; 3])
        );
        let two = combiner_shade_scale(hi, lo, true, None, env).unwrap();
        assert!((two[0] - 128.0 / 255.0).abs() < 1e-6);
        assert!((two[1] - 64.0 / 255.0).abs() < 1e-6);
        assert_eq!(two[2], 0.0);
    }

    #[test]
    fn an_unset_constant_is_white_rather_than_black() {
        // A combiner reading a colour the list never set is reading whatever
        // the RDP had. White cannot darken geometry that should be lit; black
        // would turn every such surface into a silhouette.
        let (hi, lo) = combine(ENV, ZERO_A, SHADE, ZERO_D, 0, 0, 0, 0);
        assert_eq!(
            combiner_shade_scale(hi, lo, false, None, None),
            Some([1.0; 3])
        );
    }

    #[test]
    fn texel_times_shade_is_left_to_the_texture_unit() {
        // `GU_TFX_MODULATE` already multiplies the two, so the scale is the
        // identity rather than something to fold.
        let (hi, lo) = combine(TEXEL0, ZERO_A, SHADE, ZERO_D, 0, 0, 0, 0);
        assert_eq!(
            combiner_shade_scale(hi, lo, false, None, None),
            Some([1.0; 3])
        );
    }

    #[test]
    fn a_combiner_with_an_additive_constant_is_declined() {
        // `PRIM * SHADE + ENV` cannot be expressed as a scale on the shade,
        // and approximating it would tint geometry the hardware does not.
        let (hi, lo) = combine(PRIM, ZERO_A, SHADE, ENV, 0, 0, 0, 0);
        assert_eq!(
            combiner_shade_scale(
                hi,
                lo,
                false,
                Some([255, 0, 0, 255]),
                Some([0, 0, 255, 255])
            ),
            None
        );
    }

    #[test]
    fn marios_three_combiners_reduce_the_way_his_model_needs() {
        // The words his display lists actually set, from file 296.
        let textured = combiner_shade_scale(0x0012_7e05, 0xff17_f3ff, true, None, None);
        let arms =
            combiner_shade_scale(0x0032_7e05, 0xff17_fdff, true, Some([255, 0, 0, 255]), None);
        let gloves = combiner_shade_scale(0x00ff_fe05, 0xff16_7dff, true, None, None);
        assert_eq!(textured, Some([1.0; 3]), "TEXEL0 * SHADE, then * ENV");
        assert_eq!(arms, Some([1.0, 0.0, 0.0]), "PRIM * SHADE, then * ENV");
        assert_eq!(gloves, Some([1.0; 3]), "SHADE, then * ENV");
    }
}
