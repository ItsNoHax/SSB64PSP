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
    /// `G_TX_MIRROR` on the render tile, per axis -- only meaningful (and
    /// only ever set) when that axis's own `mask` is nonzero: a texture with
    /// no repeat period has nothing to mirror. RE-066 measured 208/754
    /// tile-0 `G_SETTILE` commands archive-wide with this set on at least
    /// one axis; the PSP GE has no native mirror wrap mode, so this flag
    /// exists to let pack-time conversion pre-bake a mirrored copy instead
    /// (RE-067) rather than silently rendering a plain, incorrect repeat.
    pub mirror_s: bool,
    pub mirror_t: bool,
    /// `G_TX_CLAMP` on the render tile, per axis, independent of `mirror_s`/
    /// `mirror_t` (RE-102). Real hardware clamps addressing at the tile
    /// bound instead of wrapping past it; the PSP GE supports this natively
    /// (`sceGuTexWrap`'s `Clamp` mode). When an axis mirrors too (`cms`/
    /// `cmt` == 3), real hardware mirrors exactly once and then clamps --
    /// applying `Clamp` on top of `mirror_s`/`mirror_t`'s pre-baked doubled
    /// texture reproduces that precisely, since sampling past the doubled
    /// image then just holds its far edge. Skipping this made every
    /// primitive whose drawn UV rect legitimately exceeds its own tile --
    /// true for the small decal-style textures fighter faces are built
    /// from, and for several fighters' torso/head textures overflowing a
    /// mirrored pair by 2x or more -- wrap and repeat texels the RDP would
    /// have clamped, reading as a jumbled, "melted" texture instead of one
    /// coherent (if partially off-screen) image.
    pub clamp_s: bool,
    pub clamp_t: bool,
    /// `true` when this binding came from `mobj::LB_TRANSITION_SEGMENT`
    /// rather than any archive file (RE-099/RE-100). Every other field still
    /// describes the tile's real, decoded shape (`format`/`size`/`width`/
    /// `height`, taken from the same `G_SETTILE`/`G_SETTILESIZE` commands as
    /// any other texture) — only `data_file`/`data_offset`/`palette_*` are
    /// meaningless placeholders, since there is no ROM data to point at. A
    /// pack-time converter must skip decoding texels for these and emit a
    /// runtime-filled entry instead.
    pub framebuffer: bool,
    /// `G_SETTILESIZE`'s `uls`/`ult` on the render tile — the tile's own
    /// origin, still in raw S10.2 fixed point (quarter-texel units, the same
    /// form `dl::Cmd::SetTileSize` decodes). Only meaningful when
    /// `framebuffer` is set, where it is `0` otherwise (RE-108/RE-109): an
    /// ordinary ROM texture's baked vertex UV is implicitly tile-origin-
    /// relative for free, because pack-time extraction reads the source
    /// image starting at that same origin. A framebuffer-role binding
    /// instead samples a small, synthetic runtime capture that always
    /// starts at *its own* origin regardless of which absolute band of the
    /// real N64 buffer the tile originally pointed at, so the tile's own
    /// origin must be subtracted from the vertex UV at conversion time
    /// instead — the real RDP performs the equivalent subtraction in
    /// hardware when it converts a tile-relative ST into a TMEM address.
    pub origin_s: u16,
    pub origin_t: u16,
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
    /// `G_SETRENDERMODE`'s `CVG_X_ALPHA | ALPHA_CVG_SEL` -- a cutout
    /// surface (foliage, grates) whose coverage is driven by texture alpha
    /// (RE-069). The RDP resolves this through multisampled edge coverage,
    /// which the PSP GE has no equivalent for; `sf64-psp` (a real, shipped
    /// N64-to-PSP port doing this same translation at runtime) approximates
    /// it with a plain alpha test discarding fully-transparent texels
    /// (`sceGuAlphaFunc(Greater, 0, 0xFF)`), which is what this flag drives
    /// on the PSP side.
    pub alpha_test: bool,
    /// `G_SETRENDERMODE`'s cycle-1/cycle-2 blend equation actually reads
    /// back the framebuffer weighted by `1 - alpha` (`G_BL_CLR_MEM`,
    /// `G_BL_1MA`) -- real translucency, not just "the blender unit is
    /// engaged" (`FORCE_BL` alone is set even by the fully-opaque
    /// `G_RM_OPA_SURF` default, so it cannot be the signal by itself;
    /// RE-069). Drives `sceGuEnable(Blend)` with the standard
    /// source-alpha/one-minus-source-alpha equation, matching `sf64-psp`.
    pub translucent: bool,
    /// A combiner that blends from a base colour to a target colour driven
    /// by the texture, with no shade involved at all -- `(base, target)`
    /// (RE-073). Detected on several playable fighters' own models (Link,
    /// Ness, Yoshi, Pikachu), currently declined by `combiner_shade_scale`
    /// because it cannot be expressed as a single vertex-shade scale. Maps
    /// exactly to the PSP GE's native `TextureEffect::Blend` (`Cf`=base,
    /// `Cc`=target via `sceGuTexEnvColor`) -- not consumed on the device
    /// side yet, since that also needs affected primitives' vertices baked
    /// with a flat `base` colour instead of their usual shade-derived one,
    /// which touches vertex-sharing assumptions this pass didn't verify.
    pub texture_blend: Option<([u8; 4], [u8; 4])>,
    /// A combiner that reduces to a plain constant colour -- no shade, no
    /// texel, driven only by `PRIMITIVE`/`ENVIRONMENT`/literal constants
    /// (`(ZERO-ZERO)*ZERO+PRIM`, `ONE` alone, etc.) -- found archive-wide by
    /// RE-079's combiner-shape census (1,589 primitives). Distinct from
    /// `prim_color` (a *scale* on the shade) and `texture_blend` (a
    /// texture-driven blend between two colours): this shape depends on
    /// neither the shade nor the texture at all, so a primitive with this
    /// set should be drawn flat-untextured. `texture` is forced to `None`
    /// on such a primitive (`material_now`) since `TEXEL` genuinely never
    /// enters the formula, regardless of what the RDP had bound. Packed
    /// (`pack.rs`'s `flags::FLAT_COLOR`) and baked into affected vertices
    /// the same way `prim_color`'s scale and `texture_blend`'s base colour
    /// are (`push_vertex`); not yet separately verified on device.
    pub flat_color: Option<[u8; 4]>,
    /// The texture's active palette is driven by a material animation
    /// script rather than fixed at the `MObjSub`'s own `palettes[0]`
    /// (RE-089/RE-090/RE-091) — identity only (which script), not the
    /// resolved palette data, the same division [`TextureRef`] already
    /// draws between "where the texels are" and their decoded bytes.
    /// Tied to the same palette-setting `MObj` call as `texture` itself
    /// (`State::apply_mobj`): a later palette-bearing call that carries no
    /// script correctly clears this rather than leaving it attached to
    /// whatever texture ends up bound next.
    pub mat_anim: Option<MatAnimRef>,
}

/// Identifies the material animation script driving a texture's active
/// palette. See [`MeshMaterial::mat_anim`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct MatAnimRef {
    /// Archive file the driving script lives in. Always the graph's own
    /// file for now — RE-089 resolves `p_matanim_joints` same-file only.
    pub source_file: u32,
    /// Byte offset of the driving script within that file.
    pub script: u32,
}

impl MeshMaterial {
    /// The geometry mode every object actually starts from, not an
    /// all-off `Default`.
    ///
    /// `refs/ssb-decomp-re/src/sys/rdp.c`'s `sSYRdpResetDisplayList` --
    /// replayed once per frame by `syRdpResetSettings`
    /// (`taskman.c:308`), before any object's own display list runs --
    /// clears every geometry mode bit and then sets exactly
    /// `G_ZBUFFER | G_SHADE | G_CULL_BACK | G_SHADING_SMOOTH`. A node
    /// whose own list never mentions geometry mode at all is not "mode
    /// unknown", the way an absent combiner or texture bind is -- it is
    /// drawing under this baseline, the same as it would starting from a
    /// fresh frame on real hardware. `lit` is correctly excluded:
    /// `G_LIGHTING` is cleared here, matching RE-021's finding that most
    /// lit geometry sets it per-object outside any single node's list, so
    /// the existing normal-packing heuristic (not this default) is what
    /// recovers it.
    fn rdp_default() -> Self {
        MeshMaterial {
            cull_back: true,
            smooth: true,
            z_buffer: true,
            ..MeshMaterial::default()
        }
    }
}

/// A run of triangles sharing one material, indexing [`Mesh::vertices`].
#[derive(Debug, Clone, PartialEq, Default)]
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
#[derive(Debug, Clone, PartialEq, Default)]
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
/// Each term carries its coefficient *and* whether that term is structurally
/// present at all, tracked separately from the coefficient's numeric value.
///
/// `PRIMITIVE`/`ENVIRONMENT` are real colours the display list can set to
/// anything, including exactly black -- `(PRIM-ZERO)*SHADE+ZERO` with
/// `PRIM=[0,0,0]` legitimately reduces to `s=[0,0,0]`, not to "no `s` term".
/// A value-only representation cannot tell those apart: a bare zero-value
/// coefficient and an absent one look identical, and this model used to
/// treat both as "the combiner shape is unrecognised" (RE-079 measured 1118
/// primitives, always with `PRIM` set to exactly `[0,0,0,255]`, silently
/// falling back to unmodified vertex shade instead of the solid black the
/// real hardware always produces here). Carrying a `_used` flag alongside
/// each coefficient keeps "present with value zero" and "absent" distinct
/// all the way to the final shape match.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Combined {
    k: [f32; 3],
    k_used: bool,
    s: [f32; 3],
    s_used: bool,
    t: [f32; 3],
    t_used: bool,
    st: [f32; 3],
    st_used: bool,
}

impl Combined {
    const ZERO: Combined = Combined {
        k: [0.0; 3],
        k_used: false,
        s: [0.0; 3],
        s_used: false,
        t: [0.0; 3],
        t_used: false,
        st: [0.0; 3],
        st_used: false,
    };

    fn constant(c: [f32; 3]) -> Combined {
        Combined {
            k: c,
            k_used: true,
            ..Combined::ZERO
        }
    }

    /// Whether every varying term is *structurally absent*, so this is a
    /// plain colour -- regardless of what its own value happens to be.
    fn is_constant(&self) -> bool {
        !self.s_used && !self.t_used && !self.st_used
    }

    fn zip(&self, o: &Combined, f: impl Fn(f32, f32) -> f32) -> Combined {
        let g = |a: [f32; 3], b: [f32; 3]| [f(a[0], b[0]), f(a[1], b[1]), f(a[2], b[2])];
        Combined {
            k: g(self.k, o.k),
            k_used: self.k_used || o.k_used,
            s: g(self.s, o.s),
            s_used: self.s_used || o.s_used,
            t: g(self.t, o.t),
            t_used: self.t_used || o.t_used,
            st: g(self.st, o.st),
            st_used: self.st_used || o.st_used,
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
    ///
    /// Scaling by a *real* constant (one some source actually set, however
    /// its value happens to come out) never removes the other side's
    /// presence -- only its value changes. Scaling by a constant side that
    /// is itself structurally empty (`is_constant()` true and its own `k`
    /// unused, e.g. two literal-zero multiplexer reads subtracted from each
    /// other) is different: that side's real numeric value is unconditionally
    /// zero, so the whole product genuinely is nothing, not "the other side's
    /// terms, scaled to zero" -- e.g. `(ONE-ZERO)*ZERO` must collapse away
    /// entirely so `+SHADE` afterwards still reads as a bare `SHADE` term,
    /// not a declined shape (RE-079 found this distinction matters: without
    /// it, fixing the black-`PRIM`-scale case below regressed this one).
    fn mul(&self, o: &Combined) -> Option<Combined> {
        let (c, c_used, v) = if o.is_constant() {
            (o.k, o.k_used, self)
        } else if self.is_constant() {
            (self.k, self.k_used, o)
        } else {
            // The one mixed product worth keeping: shade times texel.
            let shade_by_texel = |a: &Combined, b: &Combined| {
                (a.s_used
                    && b.t_used
                    && !a.k_used
                    && !a.t_used
                    && !a.st_used
                    && !b.k_used
                    && !b.s_used
                    && !b.st_used)
                    .then(|| Combined {
                        st: [a.s[0] * b.t[0], a.s[1] * b.t[1], a.s[2] * b.t[2]],
                        st_used: true,
                        ..Combined::ZERO
                    })
            };
            return shade_by_texel(self, o).or_else(|| shade_by_texel(o, self));
        };
        if !c_used {
            // A structurally empty constant's real value is unconditionally
            // zero, so it absorbs the other side entirely.
            return Some(Combined::ZERO);
        }
        let g = |a: [f32; 3]| [a[0] * c[0], a[1] * c[1], a[2] * c[2]];
        Some(Combined {
            k: g(v.k),
            k_used: v.k_used,
            s: g(v.s),
            s_used: v.s_used,
            t: g(v.t),
            t_used: v.t_used,
            st: g(v.st),
            st_used: v.st_used,
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
        s_used: true,
        ..Combined::ZERO
    };
    const TEXEL: Combined = Combined {
        t: [1.0; 3],
        t_used: true,
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

/// Runs both combiner cycles (the second only in two-cycle mode) and
/// returns the final symbolic result, or `None` for a combiner this model
/// cannot follow at all.
///
/// Shared by [`combiner_shade_scale`] and [`combiner_texture_blend`], which
/// each recognise a different *shape* the same evaluated result can take.
fn evaluate_combiner(
    hi: u32,
    lo: u32,
    two_cycle: bool,
    prim: [f32; 3],
    env: [f32; 3],
) -> Option<Combined> {
    let c0 = cycle(
        [
            (hi >> 20) & 0xF,
            (lo >> 28) & 0xF,
            (hi >> 15) & 0x1F,
            (lo >> 15) & 0x7,
        ],
        None,
        prim,
        env,
    )?;
    if two_cycle {
        cycle(
            [
                (hi >> 5) & 0xF,
                (lo >> 24) & 0xF,
                hi & 0x1F,
                (lo >> 6) & 0x7,
            ],
            Some(&c0),
            prim,
            env,
        )
    } else {
        Some(c0)
    }
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
    let out = evaluate_combiner(hi, lo, two_cycle, p, e)?;

    // Usable only as a scale on the shade. A constant term would need a second
    // colour source the vertex format does not have. Presence, not value: a
    // combiner can legitimately scale the shade by exactly black.
    if out.k_used || out.t_used {
        return None;
    }
    match (out.s_used, out.st_used) {
        // `SHADE * TEXEL` is what the GE's modulate already does.
        (false, true) => Some(out.st),
        (true, false) => Some(out.s),
        _ => None,
    }
}

/// Whether any slot across the active cycle(s) reads `code` (`3` =
/// `PRIMITIVE`, `5` = `ENVIRONMENT`) -- independent of that source's value.
///
/// [`combiner_texture_blend`] needs this to gate on `PRIMITIVE`/`ENVIRONMENT`
/// being set *only when the combiner actually reads them*: a shape like
/// `(ONE-ENV)*TEXEL+ENV` never reads `PRIMITIVE` at all, so requiring
/// `prim_color` to be set to evaluate it declined every occurrence needlessly
/// (RE-079).
fn combiner_reads(hi: u32, lo: u32, two_cycle: bool, code: u32) -> bool {
    let c0 = [
        (hi >> 20) & 0xF,
        (lo >> 28) & 0xF,
        (hi >> 15) & 0x1F,
        (lo >> 15) & 0x7,
    ];
    if c0.contains(&code) {
        return true;
    }
    two_cycle
        && [(hi >> 5) & 0xF, (lo >> 24) & 0xF, hi & 0x1F, (lo >> 6) & 0x7].contains(&code)
}

/// Recognises `(A-B)*TEXEL+B` -- an affine blend from a base colour `B` (at
/// `TEXEL=0`) to a target colour `A` (at `TEXEL=1`), reading only constants
/// (`PRIMITIVE`/`ENVIRONMENT`) and the texture, no shade at all (RE-073).
///
/// This is not the shape [`combiner_shade_scale`] folds into a vertex-shade
/// scale -- it has a nonzero constant term (`k`) *and* a nonzero texel term
/// (`t`) with no shade dependence, which `combiner_shade_scale` correctly
/// declines. It is exactly the PSP GE's native `TextureEffect::Blend`
/// texture function: `Cv = Cf*(1-Ct) + Cc*Ct` with `Cf` = base, `Cc` =
/// `sceGuTexEnvColor`. Returns `(base, target)` in `u8` RGBA (alpha always
/// 255; the real combiner's alpha cycle is not modelled here, matching how
/// `combiner_shade_scale` only ever resolves RGB).
///
/// Unlike `combiner_shade_scale`, an unset `PRIMITIVE`/`ENVIRONMENT` that the
/// combiner actually reads declines rather than defaulting to white: white
/// is a safe *scale* identity, but here it would be baked in as a real,
/// wrong constant colour, and there is nothing safe to substitute instead.
/// One that the combiner does *not* read (e.g. `(ONE-ENV)*TEXEL+ENV`, which
/// never touches `PRIMITIVE`) is not required at all -- gating on it
/// unconditionally declined every such shape needlessly (RE-079).
fn combiner_texture_blend(
    hi: u32,
    lo: u32,
    two_cycle: bool,
    prim: Option<[u8; 4]>,
    env: Option<[u8; 4]>,
) -> Option<([u8; 4], [u8; 4])> {
    if combiner_reads(hi, lo, two_cycle, 3) && prim.is_none() {
        return None;
    }
    if combiner_reads(hi, lo, two_cycle, 5) && env.is_none() {
        return None;
    }
    let p = prim.map_or([0.0; 3], to_f);
    let e = env.map_or([0.0; 3], to_f);
    let out = evaluate_combiner(hi, lo, two_cycle, p, e)?;

    if out.s_used || out.st_used {
        return None;
    }
    if !out.k_used || !out.t_used {
        // No constant term is `combiner_shade_scale`'s job; no texel term is
        // a plain constant colour neither function needs to special-case.
        return None;
    }
    // `round()` needs `std`; this crate is `no_std` on the PSP target, so
    // this matches `material_now`'s own float-to-`u8` conversion (truncating
    // rather than rounding) instead of introducing a different one here.
    let from_f = |c: [f32; 3]| {
        [
            (c[0] * 255.0).clamp(0.0, 255.0) as u8,
            (c[1] * 255.0).clamp(0.0, 255.0) as u8,
            (c[2] * 255.0).clamp(0.0, 255.0) as u8,
            255,
        ]
    };
    let base = out.k;
    let target = [out.k[0] + out.t[0], out.k[1] + out.t[1], out.k[2] + out.t[2]];
    Some((from_f(base), from_f(target)))
}

/// Recognises a combiner that reduces to a plain constant colour: no shade,
/// no texel, in either cycle -- `(ZERO-ZERO)*ZERO+PRIM`, a bare `ONE`, or any
/// other combination of constants and literal zeros (RE-079). Mutually
/// exclusive with [`combiner_shade_scale`] (which requires a shade term) and
/// [`combiner_texture_blend`] (which requires a texel term): this is what is
/// left over when neither applies but the result is still fully resolved.
///
/// Like `combiner_texture_blend`, only requires whichever of
/// `PRIMITIVE`/`ENVIRONMENT` the shape actually reads (RE-079's
/// `combiner_reads` gate) -- a bare `ONE` needs neither.
fn combiner_flat_color(
    hi: u32,
    lo: u32,
    two_cycle: bool,
    prim: Option<[u8; 4]>,
    env: Option<[u8; 4]>,
) -> Option<[u8; 4]> {
    if combiner_reads(hi, lo, two_cycle, 3) && prim.is_none() {
        return None;
    }
    if combiner_reads(hi, lo, two_cycle, 5) && env.is_none() {
        return None;
    }
    let p = prim.map_or([0.0; 3], to_f);
    let e = env.map_or([0.0; 3], to_f);
    let out = evaluate_combiner(hi, lo, two_cycle, p, e)?;

    if !out.k_used || out.s_used || out.t_used || out.st_used {
        return None;
    }
    Some([
        (out.k[0] * 255.0).clamp(0.0, 255.0) as u8,
        (out.k[1] * 255.0).clamp(0.0, 255.0) as u8,
        (out.k[2] * 255.0).clamp(0.0, 255.0) as u8,
        255,
    ])
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
    /// The last genuinely real (non-palette) image binding: from an actual
    /// `G_SETTIMG`, or an `MObj`'s own `sprite` field (RE-093).
    ///
    /// A `G_LOADTLUT` restores `timg_addr`/`timg_file` from here rather than
    /// clearing them, because a multi-palette group sharing one already-loaded
    /// texture image legitimately has no `G_SETTIMG` of its own between its
    /// `MObj` call and its triangles — real hardware's texture-image register
    /// keeps whatever it last held, it does not go blank just because a
    /// palette load read a different address in between.
    real_timg: Option<(u32, Option<u16>)>,
    /// Render format from `G_SETTILE` on tile 0 — the authoritative one.
    tile0_fmt: Option<(u8, u8)>,
    tile_dims: Option<(u16, u16)>,
    /// `G_SETTILESIZE`'s `uls`/`ult` on tile 0, raw S10.2 fixed point. See
    /// [`TextureRef::origin_s`]/`origin_t` (RE-108/RE-109).
    tile0_origin: Option<(u16, u16)>,
    /// `G_SETTILE`'s `masks`/`maskt` on tile 0: the texture wraps every
    /// `1 << mask` texels, and zero means it does not wrap at all.
    tile0_mask: Option<(u8, u8)>,
    /// `G_SETTILE`'s `cms`/`cmt` on tile 0 (raw 2-bit fields: bit 0 mirror,
    /// bit 1 clamp). Only the mirror bit is acted on -- RE-066 found clamp
    /// is only ever requested alongside a nonzero mask, where the existing
    /// mask-narrowed `Repeat` already reproduces real hardware exactly.
    tile0_cm: Option<(u8, u8)>,
    palette_offset: Option<u32>,
    palette_file: Option<u16>,
    palette_entries: u16,
    texture_enabled: bool,
    /// `G_TEXTURE`'s `scale_s`/`scale_t` (RE-101): an unsigned Q0.16
    /// multiplier the RSP applies to a vertex's raw ST the moment `G_VTX`
    /// loads it, before it ever reaches the cache. `0xFFFF` is the SDK's "no
    /// scaling" value (true 1.0 does not fit in 16 bits). Skipping this made
    /// the face texture on every fighter -- authored at a UV scale below 1.0
    /// -- sample several texture periods wider than the artist intended,
    /// reading as a "melted" jumble of unrelated texels instead of a face.
    tex_scale: (u16, u16),
    /// Set by a `G_SETTIMG` targeting `mobj::LB_TRANSITION_SEGMENT`; cleared
    /// by any other `G_SETTIMG` or `forget_texture`. See
    /// [`TextureRef::framebuffer`].
    framebuffer_capture: bool,
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
    /// Parallel to `mobjs`; see `SequenceItem::mat_anims`.
    mat_anims: Vec<Option<MatAnimRef>>,
}

impl State {
    fn new() -> Self {
        State {
            cache: [None; VTX_CACHE_SIZE as usize],
            space: 0,
            spaces: Vec::new(),
            inv_current: crate::scene::Mat4::IDENTITY,
            material: MeshMaterial::rdp_default(),
            timg_addr: None,
            timg_file: None,
            real_timg: None,
            tile0_fmt: None,
            tile_dims: None,
            tile0_origin: None,
            tile0_mask: None,
            tile0_cm: None,
            palette_offset: None,
            palette_file: None,
            palette_entries: 0,
            texture_enabled: false,
            tex_scale: (0xFFFF, 0xFFFF),
            framebuffer_capture: false,
            combiner: None,
            two_cycle: false,
            mobjs: Vec::new(),
            mat_anims: Vec::new(),
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
    fn apply_mobj(&mut self, m: &crate::mobj::MObjMaterial, mat_anim: Option<MatAnimRef>) {
        if let Some(palette) = m.palette {
            self.timg_addr = Some(palette.offset);
            self.timg_file = palette.file;
            self.framebuffer_capture = false;
            // Tied to the same condition as the palette itself, not merely
            // "set when present": a later palette-bearing `MObj` with no
            // script must clear a previous one rather than leave it
            // attached to whatever texture ends up bound next.
            self.material.mat_anim = mat_anim;
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
            self.real_timg = Some((sprite.offset, sprite.file));
            self.framebuffer_capture = false;
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
        self.real_timg = None;
        self.palette_offset = None;
        self.palette_file = None;
        self.texture_enabled = false;
        self.framebuffer_capture = false;
        self.material.mat_anim = None;
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
        // Gated on a texture the same way `alpha_test`/`translucent` are:
        // the whole point of this shape is the texture driving the blend
        // (RE-073), so without one there is nothing for `TEXEL` to mean.
        let texture = self.current_texture();
        let texture_blend = texture.and(self.combiner).and_then(|(hi, lo)| {
            combiner_texture_blend(
                hi,
                lo,
                self.two_cycle,
                self.material.prim_color,
                self.material.env_color,
            )
        });
        let flat_color = self.combiner.and_then(|(hi, lo)| {
            combiner_flat_color(
                hi,
                lo,
                self.two_cycle,
                self.material.prim_color,
                self.material.env_color,
            )
        });
        MeshMaterial {
            // `TEXEL` genuinely never enters this shape's formula (RE-079),
            // so a bound texture would be sampled and modulated in for
            // nothing the real hardware does -- force untextured rather than
            // let the GE's default `Modulate` silently darken/tint the flat
            // colour by whatever happens to be bound.
            texture: if flat_color.is_some() { None } else { texture },
            flat_color,
            // Identity is not worth storing: it means "use the shade as it is".
            prim_color: scale.filter(|s| *s != [1.0; 3]).map(|s| {
                [
                    (s[0] * 255.0).clamp(0.0, 255.0) as u8,
                    (s[1] * 255.0).clamp(0.0, 255.0) as u8,
                    (s[2] * 255.0).clamp(0.0, 255.0) as u8,
                    255,
                ]
            }),
            // A cutout render mode is a statement about the *texture's*
            // alpha channel (`G_CC_*` combiners for `TEX_EDGE` read TEXEL0's
            // alpha, not shade alpha) -- meaningless, and actively wrong,
            // without one. Untextured lit geometry's vertex alpha byte is
            // not a coverage value at all (it is a packed normal component;
            // see `push_vertex`'s doc comment), so alpha-testing against it
            // discarded whole primitives outright until this gate was added
            // (RE-069 measured 46 of 380 `alpha_test` primitives had no
            // texture at all). `translucent` gets the same gate for the same
            // reason: this converter does not compute the combiner's actual
            // alpha output (`combiner_shade_scale` only resolves RGB), so
            // the only alpha channel available with any real fidelity is a
            // decoded texture's own -- blending untextured, possibly-lit
            // geometry against a meaningless vertex alpha risks the same
            // silent-disappearance failure, just via `SrcAlpha` instead of a
            // discard (7 of 362 `translucent` primitives had no texture).
            alpha_test: self.material.alpha_test && texture.is_some(),
            translucent: self.material.translucent && texture.is_some(),
            texture_blend,
            // Same reasoning as `alpha_test`/`translucent`'s gate: an
            // animated palette with no texture to apply it to is orphaned
            // state, not a primitive worth carrying it on.
            mat_anim: self.material.mat_anim.filter(|_| texture.is_some()),
            ..self.material
        }
    }

    /// Assembles the current texture binding, if one is fully specified.
    fn current_texture(&self) -> Option<TextureRef> {
        if !self.texture_enabled {
            return None;
        }
        let (fmt, siz) = self.tile0_fmt?;
        let (w, h) = self.tile_dims?;
        // `G_SETTILESIZE` gives the rectangle being *drawn*, which for a
        // wrapping texture is larger than the texture: Dream Land renders a
        // 64x32 tile across a 256x128 span. `masks`/`maskt` are what say how
        // big the texture really is -- it repeats every `1 << mask` texels --
        // and taking the drawn rect instead asks for 16 KiB of texels out of a
        // 12 KiB file (RE-044). A mask of zero means no wrapping, so the drawn
        // rect is the texture.
        let (mask_s, mask_t) = self.tile0_mask.unwrap_or((0, 0));
        let (nw, nh) = (
            if mask_s > 0 { w.min(1 << mask_s) } else { w },
            if mask_t > 0 { h.min(1 << mask_t) } else { h },
        );
        let (cm_s, cm_t) = self.tile0_cm.unwrap_or((0, 0));
        let (w, h) = (nw, nh);
        // `G_TX_MIRROR` is bit 0 of `cms`/`cmt`. Only meaningful with an
        // actual repeat period (RE-066: every real occurrence in this ROM
        // already has one), so gate on the mask too rather than trusting the
        // bit alone.
        let mirror_s = mask_s > 0 && cm_s & 0x1 != 0;
        let mirror_t = mask_t > 0 && cm_t & 0x1 != 0;
        // `G_TX_CLAMP` is bit 1 (RE-102). Unlike mirror it is not gated on
        // the mask being nonzero -- a mask of zero already means "the drawn
        // rect is the texture" per the comment above, so clamp is a no-op
        // there either way, and gating it out would just be redundant, not
        // wrong. Nor is it gated on mirror: `cms`/`cmt` == 3 (both bits) is
        // real hardware's "mirror once, then clamp beyond that single
        // bounce" -- not "mirror forever". The PSP GE's `Clamp` wrap mode
        // applied *on top of* RE-067's pre-baked mirrored-double texture
        // reproduces that exactly (sampling past the doubled image just
        // holds its far edge); treating a `cms == 3` axis as pure mirror
        // (plain `Repeat`, the first cut of this fix) kept tiling the
        // mirrored pair past the point real hardware clamps, which is
        // exactly what a UV overflowing several periods -- Fox's and Captain
        // Falcon's head/body textures, measured well past 2x here -- turns
        // into a jarring rainbow repeat instead of one held edge.
        let clamp_s = cm_s & 0x2 != 0;
        let clamp_t = cm_t & 0x2 != 0;

        if self.framebuffer_capture {
            // No archive location: the real content is filled in on the
            // device from a runtime framebuffer capture (RE-099/RE-100).
            // `format`/`size`/`width`/`height` still describe the tile's real
            // decoded shape, taken from the same `G_SETTILE`/`G_SETTILESIZE`
            // commands as any other texture -- a pack-time converter uses
            // these to size the runtime buffer, just skips decoding texels.
            let (origin_s, origin_t) = self.tile0_origin.unwrap_or((0, 0));
            return Some(TextureRef {
                data_file: None,
                data_offset: 0,
                format: Format::from_raw(fmt)?,
                size: BitSize::from_raw(siz)?,
                width: w,
                height: h,
                palette_file: None,
                palette_offset: None,
                palette_entries: 0,
                mirror_s,
                mirror_t,
                clamp_s,
                clamp_t,
                framebuffer: true,
                origin_s,
                origin_t,
            });
        }

        let offset = self.timg_addr?;
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
            mirror_s,
            mirror_t,
            clamp_s,
            clamp_t,
            framebuffer: false,
            // Irrelevant outside the framebuffer role -- see
            // `TextureRef::origin_s`/`origin_t`.
            origin_s: 0,
            origin_t: 0,
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
    /// `texture_blend` (RE-073) uses the same mechanism to bake a flat base
    /// colour in place of the shade instead of scaling it.
    fn push_vertex(&mut self, mut v: MeshVertex) -> Result<u16, MeshError> {
        if let Some(c) = self.material.prim_color {
            for (shade, &prim) in v.rgba.iter_mut().zip(c.iter()).take(3) {
                *shade = ((*shade as u16 * prim as u16) / 255) as u8;
            }
            // Shade alpha is not a coverage value here — Mario's vertices are
            // all zero — so the primitive's alpha is the one that means
            // something. Multiplying would make him invisible.
            v.rgba[3] = c[3];
        } else if let Some((base, _target)) = self.material.texture_blend {
            // RE-073's shape reads no shade at all, so the vertex colour this
            // primitive needs is simply the flat base colour, the same way
            // `prim_color`'s scale replaces the shade above -- and by the same
            // mechanism, baking it here rather than at draw time means the
            // dedup below cannot hand a `texture_blend` vertex's baked colour
            // to a differently-shaded primitive that happens to load the same
            // cache slot: the baked colour is part of the dedup key, so the
            // two become distinct entries automatically.
            v.rgba = base;
        } else if let Some(c) = self.material.flat_color {
            // Same reasoning as `texture_blend` above: RE-079's flat-colour
            // shape reads neither shade nor texel, so the vertex colour is
            // simply the resolved constant, baked here so the dedup key
            // keeps a shared cache-slot vertex distinct across primitives
            // that need different flat colours.
            v.rgba = c;
        }
        if let Some(t) = self.material.texture {
            if t.framebuffer {
                // Rebase by the tile's own origin (RE-108/RE-109): see
                // `TextureRef::origin_s`/`origin_t`. `origin_s`/`origin_t`
                // are raw S10.2 (quarter-texel); `v.uv` is S10.5, so align
                // scales with `* 8` before subtracting.
                v.uv[0] = (v.uv[0] as i32 - t.origin_s as i32 * 8) as i16;
                v.uv[1] = (v.uv[1] as i32 - t.origin_t as i32 * 8) as i16;
            }
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
    /// Parallel to `mobjs`: which entries are driven by a material animation
    /// script, if any were resolved for this node (RE-089/RE-091). Shorter
    /// than or the same length as `mobjs`; a missing or `None` entry means
    /// "not animated", not "unknown" — most `MObj`s never are.
    pub mat_anims: &'a [Option<MatAnimRef>],
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
        state.mat_anims = item.mat_anims.to_vec();
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
                        // Real hardware bakes the `G_TEXTURE` scale into ST at
                        // load time, not draw time, so it must use whatever
                        // `tex_scale` is current right here.
                        let (scale_s, scale_t) = state.tex_scale;
                        let scaled_uv = [
                            ((v.uv[0] as i32 * scale_s as i32) >> 16) as i16,
                            ((v.uv[1] as i32 * scale_t as i32) >> 16) as i16,
                        ];
                        state.cache[slot] = Some(CacheEntry {
                            vertex: MeshVertex {
                                pos: v.pos,
                                uv: scaled_uv,
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
                        Some(m) => {
                            let mat_anim = state.mat_anims.get(index).copied().flatten();
                            state.apply_mobj(&m.clone(), mat_anim);
                        }
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
            Cmd::SetTimg { addr, slot, .. } => {
                // A display list only reconfigures the RDP's texture-image
                // register to sample it -- so a fresh `G_SETTIMG` implies
                // texturing is active regardless of a stale inherited `off`
                // (RE-093/094). Real stage data does this: an explicit
                // `Texture{on: false}` for one node's own untextured
                // decal can precede several later, unrelated nodes that
                // reissue their own complete `G_SETTIMG`/`G_SETTILE`/
                // `G_LOADBLOCK` chain and draw textured, with no
                // `Texture{on: true}` of their own to undo the earlier
                // `off` -- `Cmd::Texture` alone cannot be the sole signal.
                state.texture_enabled = true;
                if addr.segment() == crate::mobj::LB_TRANSITION_SEGMENT {
                    // The LB "loading transition" system binds this segment to
                    // a one-time CPU-side snapshot of the framebuffer
                    // (RE-099/RE-100), not to any archive data -- there is
                    // nothing here to resolve at pack time, only a marker to
                    // carry through (`State::framebuffer_capture`,
                    // `TextureRef::framebuffer`) to the device, which fills
                    // the real pixels in at runtime.
                    state.framebuffer_capture = true;
                    state.timg_addr = None;
                    state.timg_file = None;
                } else {
                    state.framebuffer_capture = false;
                    match (addr.0, src.extern_at(slot)) {
                        (0, Some((target_file, offset))) => {
                            state.timg_addr = Some(offset);
                            state.timg_file = Some(target_file);
                            state.real_timg = Some((offset, Some(target_file)));
                        }
                        _ => {
                            state.timg_addr = Some(addr.0);
                            state.timg_file = None;
                            state.real_timg = Some((addr.0, None));
                        }
                    }
                }
            }

            Cmd::SetTile {
                format,
                size,
                tile,
                mask_s,
                mask_t,
                cm_s,
                cm_t,
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
                    state.tile0_cm = Some((cm_s, cm_t));
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
                state.tile0_origin = Some((uls, ult));
            }

            Cmd::LoadTlut { count, .. } => {
                // A TLUT load reads from whatever image address is current, so
                // that address *is* the palette — including which file it is
                // in. The real texture usually follows with its own SETTIMG,
                // but a multi-palette group that keeps drawing the same
                // already-resident texture image legitimately does not
                // reissue one (RE-093) -- restoring the last real binding
                // rather than clearing it is correct either way: if a fresh
                // SETTIMG *does* follow, it overwrites this immediately.
                state.palette_offset = state.timg_addr;
                state.palette_file = state.timg_file;
                state.palette_entries = count;
                state.timg_addr = state.real_timg.map(|(a, _)| a);
                state.timg_file = state.real_timg.and_then(|(_, f)| f);
            }

            Cmd::Texture {
                on,
                scale_s,
                scale_t,
                ..
            } => {
                state.texture_enabled = on;
                state.tex_scale = (scale_s, scale_t);
            }

            Cmd::GeometryMode { clear, set } => {
                let apply = |cur: bool, bit: u32| (cur && clear & bit == 0) || set & bit != 0;
                state.material.cull_back = apply(state.material.cull_back, G_CULL_BACK);
                state.material.cull_front = apply(state.material.cull_front, G_CULL_FRONT);
                state.material.lit = apply(state.material.lit, G_LIGHTING);
                state.material.smooth = apply(state.material.smooth, G_SHADING_SMOOTH);
                state.material.z_buffer = apply(state.material.z_buffer, G_ZBUFFER);
            }

            // `G_MW_LIGHTCOL` (RE-105): updating a light's colour has no
            // effect unless `G_LIGHTING` is (or is about to be) on for this
            // draw -- no display list would spend a command on it otherwise.
            // Real hardware sets `G_LIGHTING` itself per-object, externally
            // (RE-021), so this in-list command is the one unambiguous,
            // ROM-verified signal (not a data-shape guess) that a segment
            // relying on that external state is about to draw lit geometry.
            Cmd::MoveWord {
                index: G_MW_LIGHTCOL,
                ..
            } => state.material.lit = true,

            Cmd::SetCombine { hi, lo } => state.combiner = Some((hi, lo)),

            // The cycle type is two bits at shift 20 of the high other-mode
            // word: 0 one-cycle, 1 two-cycle, then copy and fill.
            Cmd::SetOtherModeH { shift, len, data } if shift == 20 && len == 2 => {
                state.two_cycle = (data >> 20) & 0x3 == 1;
            }

            // `G_MDSFT_RENDERMODE` is 29 bits starting at bit 3.
            Cmd::SetOtherModeL { shift: 3, len: 29, data } => {
                state.material.alpha_test = data & RENDER_MODE_TEX_EDGE == RENDER_MODE_TEX_EDGE;
                state.material.translucent = render_mode_is_translucent(data);
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

/// `G_MOVEWORD`'s `index` for a light colour update (`gbi.h`'s
/// `G_MW_LIGHTCOL`, RE-105).
const G_MW_LIGHTCOL: u8 = 0x0a;

// Render mode bits (`G_SETOTHERMODE_L` at `G_MDSFT_RENDERMODE`, from
// `gbi.h`), RE-069. `data` there is already the raw `w1` word -- the GBI's
// `GBL_c1`/`GBL_c2` macros left-shift into these absolute bit positions
// before `gsSPSetOtherMode` stores them verbatim, so no further shifting is
// needed to read them back.
const CVG_X_ALPHA: u32 = 0x0000_1000;
const ALPHA_CVG_SEL: u32 = 0x0000_2000;
/// A cutout surface: coverage is driven by the texture's own alpha
/// (`RM_..._TEX_EDGE` family). The RDP resolves this through multisampled
/// edge coverage the PSP has no equivalent for; approximated as a plain
/// alpha test (RE-069, following `sf64-psp`'s validated approach).
const RENDER_MODE_TEX_EDGE: u32 = CVG_X_ALPHA | ALPHA_CVG_SEL;

/// True when a `G_SETRENDERMODE` value's blend equation reads back the
/// framebuffer weighted by `1 - alpha` (`G_BL_CLR_MEM`, `G_BL_1MA`) in
/// either cycle -- genuine translucency, not merely `FORCE_BL` (set even by
/// the fully-opaque `G_RM_OPA_SURF` default) or a specific `ZMODE`. Checked
/// against both cycles rather than only the active one, matching
/// `refs/BattleShip`'s validated interpreter (`interpreter.cpp:3071-3074`).
fn render_mode_is_translucent(data: u32) -> bool {
    let field = |shift: u32| (data >> shift) & 0x3;
    const CLR_MEM: u32 = 1;
    const A_1MA: u32 = 0;
    (field(22) == CLR_MEM && field(18) == A_1MA) || (field(20) == CLR_MEM && field(16) == A_1MA)
}

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
                mat_anims: &[],
            },
            SequenceItem {
                cmds: &b,
                world: Mat4::from_trs([100.0, 0.0, 0.0], [0.0; 3], [1.0; 3]),
                mobjs: &[],
                mat_anims: &[],
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
            mat_anims: &[],
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

    /// RE-091: a resolved material animation script is carried onto the
    /// primitive the same `MObj` call binds a texture for -- `mat_anims` is
    /// parallel to `mobjs`, keyed by the same heap index.
    #[test]
    fn an_animated_palette_is_carried_onto_the_primitive_that_uses_it() {
        use crate::mobj::MObjMaterial;
        use crate::scene::Mat4;

        let file = vertex_data(3);
        let cmds = ci4_list_calling_the_heap(8); // index 1, as above.
        let mobjs = [
            MObjMaterial::default(),
            MObjMaterial {
                palette: Some(crate::mobj::Ptr {
                    file: None,
                    offset: 0x200,
                }),
                ..MObjMaterial::default()
            },
        ];
        let animated = MatAnimRef {
            source_file: 104,
            script: 0x3098,
        };
        let mat_anims = [None, Some(animated)];
        let items = [SequenceItem {
            cmds: &cmds,
            world: Mat4::IDENTITY,
            mobjs: &mobjs,
            mat_anims: &mat_anims,
        }];
        let mesh = convert_sequence(&items, Source::bare(&file))
            .pop()
            .unwrap()
            .unwrap();
        assert_eq!(mesh.primitives[0].material.mat_anim, Some(animated));
    }

    /// The correctness case that matters most: a *later* palette-bearing
    /// `MObj` with no script must clear a previous animation marker rather
    /// than let it leak onto an unrelated texture. Getting this wrong would
    /// silently mis-attribute an animation the same way a wrong texture/
    /// palette inheritance rule once mis-attributed a binding (RE-064).
    #[test]
    fn a_later_unanimated_palette_clears_a_previous_mat_anim() {
        use crate::mobj::MObjMaterial;
        use crate::scene::Mat4;

        let file = vertex_data(3);
        let cmds = [
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
            // MObj 0: animated palette, texture A.
            Cmd::Call(SegAddr(0x0E00_0000)),
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
            // MObj 1: a different, unanimated palette, texture B.
            Cmd::Call(SegAddr(0x0E00_0000 + 8)),
            Cmd::LoadTlut { tile: 5, count: 16 },
            Cmd::SetTimg {
                format: 0,
                size: 2,
                width: 1,
                addr: SegAddr(0x500),
                slot: 0,
            },
            vtx(3),
            Cmd::Tri1([0, 1, 2]),
        ];
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
        let animated = MatAnimRef {
            source_file: 104,
            script: 0x3098,
        };
        let mat_anims = [Some(animated), None];
        let items = [SequenceItem {
            cmds: &cmds,
            world: Mat4::IDENTITY,
            mobjs: &mobjs,
            mat_anims: &mat_anims,
        }];
        let mesh = convert_sequence(&items, Source::bare(&file))
            .pop()
            .unwrap()
            .unwrap();
        assert_eq!(mesh.primitives.len(), 2, "two distinct textures, two primitives");
        let by_texture = |offset: u32| {
            mesh.primitives
                .iter()
                .find(|p| p.material.texture.is_some_and(|t| t.data_offset == offset))
                .unwrap_or_else(|| panic!("no primitive bound to texture 0x{offset:X}"))
        };
        assert_eq!(by_texture(0x400).material.mat_anim, Some(animated));
        assert_eq!(
            by_texture(0x500).material.mat_anim,
            None,
            "the second MObj's own palette carries no script and must clear the first one's"
        );
    }

    /// RE-093: a real ROM display list (file 105's `StageZebesFile2`, node 1)
    /// draws several `MObj` entries against the *same* already-loaded texture
    /// image, reissuing only `G_LOADTLUT` for each one's own palette and never
    /// a fresh `G_SETTIMG` -- because the image itself has not changed, only
    /// the palette has. A `G_LOADTLUT` that clears the image address instead
    /// of restoring it drops the texture entirely for every entry after the
    /// first, producing untextured geometry the real hardware never draws.
    #[test]
    fn a_palette_only_mobj_keeps_the_image_a_prior_settimg_bound() {
        use crate::mobj::MObjMaterial;
        use crate::scene::Mat4;

        let file = vertex_data(3);
        let ci4_tile = || Cmd::SetTile {
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
        };
        let cmds = [
            ci4_tile(),
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
            // The one and only `G_SETTIMG`: both `MObj` entries below draw
            // this same image, differing only in which palette they load.
            Cmd::SetTimg {
                format: 0,
                size: 2,
                width: 1,
                addr: SegAddr(0x400),
                slot: 0,
            },
            // MObj 0: palette A, drawn against the image just bound.
            Cmd::Call(SegAddr(0x0E00_0000)),
            Cmd::LoadTlut { tile: 5, count: 16 },
            vtx(3),
            Cmd::Tri1([0, 1, 2]),
            // MObj 1: palette B, no `G_SETTIMG` of its own -- same image.
            Cmd::Call(SegAddr(0x0E00_0000 + 8)),
            Cmd::LoadTlut { tile: 5, count: 16 },
            vtx(3),
            Cmd::Tri1([0, 1, 2]),
        ];
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
            mat_anims: &[],
        }];
        let mesh = convert_sequence(&items, Source::bare(&file))
            .pop()
            .unwrap()
            .unwrap();
        assert_eq!(
            mesh.primitives.len(),
            2,
            "same image, two different palettes -- still two distinct materials"
        );
        let by_palette = |offset: u32| {
            mesh.primitives
                .iter()
                .find(|p| p.material.texture.is_some_and(|t| t.palette_offset == Some(offset)))
                .unwrap_or_else(|| panic!("no primitive bound to palette 0x{offset:X}"))
        };
        assert_eq!(by_palette(0x100).material.texture.unwrap().data_offset, 0x400);
        assert_eq!(
            by_palette(0x200).material.texture.unwrap().data_offset,
            0x400,
            "the second MObj's own geometry uses the same image the first one's G_SETTIMG bound"
        );
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
            mat_anims: &[],
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
    fn a_texture_binding_persists_into_a_node_that_sets_no_new_state() {
        // R0.4's open "palette inheritance/state" item: `convert_sequence`'s
        // doc comment claims RDP state carries across a node sequence the
        // way real hardware keeps it, measured archive-wide (378/394 texture
        // resolutions) but never pinned by a direct unit test. This is that
        // test: joint A fully binds a CI4 texture+palette and draws; joint B
        // sets no texture state at all and must inherit the exact same
        // binding, not merely "some" binding.
        use crate::scene::Mat4;

        let file = vertex_data(6);
        let a = [
            Cmd::SetTimg {
                format: 0,
                size: 2,
                width: 1,
                addr: SegAddr(0x100),
                slot: 0,
            },
            Cmd::LoadTlut { tile: 5, count: 16 },
            Cmd::SetTimg {
                format: 0,
                size: 2,
                width: 1,
                addr: SegAddr(0x400),
                slot: 0,
            },
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
            Cmd::SetTileSize {
                tile: 0,
                uls: 0,
                ult: 0,
                lrs: 124,
                lrt: 124,
            },
            Cmd::Texture {
                level: 0,
                tile: 0,
                on: true,
                scale_s: 0xFFFF,
                scale_t: 0xFFFF,
            },
            vtx(3),
            Cmd::Tri1([0, 1, 2]),
        ];
        let b = [
            Cmd::Vtx {
                count: 3,
                dest_index: 0,
                addr: SegAddr(3 * Vtx::SIZE as u32),
            },
            Cmd::Tri1([0, 1, 2]),
        ];
        let items = [
            SequenceItem {
                cmds: &a,
                world: Mat4::IDENTITY,
                mobjs: &[],
                mat_anims: &[],
            },
            SequenceItem {
                cmds: &b,
                world: Mat4::IDENTITY,
                mobjs: &[],
                mat_anims: &[],
            },
        ];
        let out = convert_sequence(&items, Source::bare(&file));
        let first = out[0].as_ref().unwrap().primitives[0]
            .material
            .texture
            .expect("joint A binds its own texture");
        let second = out[1].as_ref().unwrap().primitives[0]
            .material
            .texture
            .expect("joint B must inherit joint A's texture, not draw unbound");
        assert_eq!(
            second, first,
            "a node that sets no new material state must inherit the previous node's exactly"
        );
    }

    #[test]
    fn a_lb_transition_segment_bind_produces_a_marked_framebuffer_texture() {
        // RE-099/RE-100: the LB transition's `G_SETTIMG` names segment
        // `0x1` (`sLBTransitionPhotoHeap`), not any archive location. Real
        // ROM data (file 40) measured a 300x5 tile at exactly this shape.
        let file = vertex_data(3);
        let cmds = [
            Cmd::SetTimg {
                format: Format::Rgba as u8,
                size: BitSize::Bits16 as u8,
                width: 300,
                addr: SegAddr(0x0100_0000), // segment 1, offset 0
                slot: 0,
            },
            Cmd::SetTile {
                format: Format::Rgba as u8,
                size: BitSize::Bits16 as u8,
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
            // (lrs - uls) >> 2 + 1 == 300, (lrt - ult) >> 2 + 1 == 5.
            Cmd::SetTileSize {
                tile: 0,
                uls: 0,
                ult: 0,
                lrs: 1196,
                lrt: 16,
            },
            Cmd::Texture {
                level: 0,
                tile: 0,
                on: true,
                scale_s: 0xFFFF,
                scale_t: 0xFFFF,
            },
            vtx(3),
            Cmd::Tri1([0, 1, 2]),
            Cmd::End,
        ];
        let mesh = convert(&cmds, Source::bare(&file)).unwrap();
        let t = mesh.primitives[0]
            .material
            .texture
            .expect("a segment-0x1 bind must still produce a texture reference");
        assert!(t.framebuffer, "segment 0x1 must be marked as a framebuffer capture, not a ROM location");
        assert_eq!(t.data_file, None);
        assert_eq!(t.data_offset, 0);
        assert_eq!(t.width, 300);
        assert_eq!(t.height, 5);
        assert_eq!(t.format, Format::Rgba);
        assert_eq!(t.size, BitSize::Bits16);
    }

    #[test]
    fn a_real_settimg_after_a_transition_segment_clears_the_framebuffer_marker() {
        // A node that binds segment 0x1 and then rebinds an ordinary texture
        // must not still be marked as a framebuffer capture -- the marker is
        // exactly as overridable as any other `G_SETTIMG` state.
        let file = vertex_data(3);
        let cmds = [
            Cmd::SetTimg {
                format: Format::Rgba as u8,
                size: BitSize::Bits16 as u8,
                width: 1,
                addr: SegAddr(0x0100_0000),
                slot: 0,
            },
            Cmd::SetTimg {
                format: Format::Ci as u8,
                size: BitSize::Bits4 as u8,
                width: 1,
                addr: SegAddr(0x40),
                slot: 4,
            },
            Cmd::SetTile {
                format: Format::Ci as u8,
                size: BitSize::Bits4 as u8,
                line: 2,
                tmem: 0,
                tile: 0,
                palette: 0,
                cm_s: 0,
                cm_t: 0,
                mask_s: 4,
                mask_t: 4,
                shift_s: 0,
                shift_t: 0,
            },
            Cmd::SetTileSize {
                tile: 0,
                uls: 0,
                ult: 0,
                lrs: 60,
                lrt: 60,
            },
            Cmd::Texture {
                level: 0,
                tile: 0,
                on: true,
                scale_s: 0xFFFF,
                scale_t: 0xFFFF,
            },
            vtx(3),
            Cmd::Tri1([0, 1, 2]),
            Cmd::End,
        ];
        let mesh = convert(&cmds, Source::bare(&file)).unwrap();
        let t = mesh.primitives[0].material.texture.expect("real texture bound");
        assert!(!t.framebuffer, "a later real G_SETTIMG must clear the framebuffer marker");
        assert_eq!(t.data_offset, 0x40);
    }

    #[test]
    fn a_framebuffer_role_tile_not_at_the_origin_has_its_uv_rebased() {
        // RE-108/RE-109: file 45's real 300x5 "photo" tile sets
        // `ult = 860` (texel 215, the *bottom* of the real 220-texel-tall
        // N64 buffer) with vertex V baked at exactly the same absolute
        // position -- so the raw, un-rebased UV pointed at content this
        // project's small top-of-buffer runtime capture never populates,
        // reading back whatever was there instead (measured black on
        // device). A conversion-time rebase must subtract the tile's own
        // origin so the same vertex instead samples relative position 0,
        // matching a tile whose origin genuinely is 0 (the working 300x6
        // entry, RE-100).
        let mut file = Vec::new();
        // One vertex: x=0 y=0 z=0 pad=0 u=0 v=6881 rgba=opaque white.
        // `G_TEXTURE`'s default `scale_t` (0xFFFF) is the SDK's "no
        // scaling" value, not exactly 1.0 (RE-101) -- `(6881 * 0xFFFF) >>
        // 16 == 6880`, which is exactly `860` quarter-texels (this tile's
        // own `ult`) in S10.5. Using the raw value that scales to exactly
        // that keeps the assertion below an exact `0`, not off by the
        // scale rounding's own unrelated -1.
        file.extend_from_slice(&0i16.to_be_bytes());
        file.extend_from_slice(&0i16.to_be_bytes());
        file.extend_from_slice(&0i16.to_be_bytes());
        file.extend_from_slice(&0u16.to_be_bytes());
        file.extend_from_slice(&0i16.to_be_bytes());
        file.extend_from_slice(&6881i16.to_be_bytes());
        file.extend_from_slice(&[0xFF, 0xFF, 0xFF, 0xFF]);
        // Two more vertices at the same UV, for a real triangle.
        for _ in 0..2 {
            file.extend_from_slice(&10i16.to_be_bytes());
            file.extend_from_slice(&0i16.to_be_bytes());
            file.extend_from_slice(&0i16.to_be_bytes());
            file.extend_from_slice(&0u16.to_be_bytes());
            file.extend_from_slice(&0i16.to_be_bytes());
            file.extend_from_slice(&6881i16.to_be_bytes());
            file.extend_from_slice(&[0xFF, 0xFF, 0xFF, 0xFF]);
        }
        let cmds = [
            Cmd::SetTimg {
                format: Format::Rgba as u8,
                size: BitSize::Bits16 as u8,
                width: 300,
                addr: SegAddr(0x0100_0000), // segment 1, offset 0
                slot: 0,
            },
            Cmd::SetTile {
                format: Format::Rgba as u8,
                size: BitSize::Bits16 as u8,
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
            // uls=0, ult=860 (texel 215); (lrs-uls)>>2+1 == 300,
            // (lrt-ult)>>2+1 == 5, matching RE-108's real file-45 measurement.
            Cmd::SetTileSize {
                tile: 0,
                uls: 0,
                ult: 860,
                lrs: 1196,
                lrt: 876,
            },
            Cmd::Texture {
                level: 0,
                tile: 0,
                on: true,
                scale_s: 0xFFFF,
                scale_t: 0xFFFF,
            },
            vtx(3),
            Cmd::Tri1([0, 1, 2]),
            Cmd::End,
        ];
        let mesh = convert(&cmds, Source::bare(&file)).unwrap();
        let t = mesh.primitives[0]
            .material
            .texture
            .expect("a segment-0x1 bind must still produce a texture reference");
        assert!(t.framebuffer);
        assert_eq!(t.origin_t, 860, "the tile's own origin must be recorded");
        assert_eq!(
            mesh.vertices[0].uv[1], 0,
            "a vertex baked at the tile's own origin must rebase to 0, not stay at the tile's absolute position in the conceptual 220-row image"
        );
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
    fn a_list_with_no_geometry_mode_command_draws_under_the_rdp_reset_default() {
        // RE-068: `refs/ssb-decomp-re/src/sys/rdp.c`'s `sSYRdpResetDisplayList`
        // sets `G_ZBUFFER | G_SHADE | G_CULL_BACK | G_SHADING_SMOOTH` once per
        // frame, before any object's own list runs. A list that never touches
        // geometry mode at all is not "mode unknown" -- it draws under that
        // baseline, not an all-off `Default`.
        let file = vertex_data(3);
        let cmds = [vtx(3), Cmd::Tri1([0, 1, 2]), Cmd::End];
        let mesh = convert(&cmds, Source::bare(&file)).unwrap();
        let m = mesh.primitives[0].material;
        assert!(m.cull_back, "G_CULL_BACK is on in the reset default");
        assert!(m.smooth, "G_SHADING_SMOOTH is on in the reset default");
        assert!(m.z_buffer, "G_ZBUFFER is on in the reset default");
        assert!(!m.cull_front, "the reset default only sets G_CULL_BACK, not both");
        assert!(!m.lit, "G_LIGHTING is cleared in the reset default (RE-021 recovers it separately)");
    }

    /// Builds a `G_SETRENDERMODE`'s raw `data` word the way `GBL_c1`/
    /// `GBL_c2` do: `flags` are the non-equation bits (`FORCE_BL`,
    /// `CVG_X_ALPHA`, `ZMODE_*`, ...), `c1`/`c2` are each cycle's
    /// `(color_src_a, factor_a, color_src_b, factor_b)`, matching
    /// `refs/ssb-decomp-re/include/PR/gbi.h`'s macros exactly rather than a
    /// magic hex constant.
    fn render_mode(flags: u32, c1: (u32, u32, u32, u32), c2: (u32, u32, u32, u32)) -> u32 {
        let gbl = |shifts: [u32; 4], (a, b, c, d): (u32, u32, u32, u32)| {
            (a << shifts[0]) | (b << shifts[1]) | (c << shifts[2]) | (d << shifts[3])
        };
        flags | gbl([30, 26, 22, 18], c1) | gbl([28, 24, 20, 16], c2)
    }

    fn set_render_mode(data: u32) -> Cmd {
        Cmd::SetOtherModeL {
            shift: 3,
            len: 29,
            data,
        }
    }

    /// Binds a minimal CI4 texture, the way `tile_size_converts_10_2_...`
    /// does. `alpha_test`/`translucent` are gated on a texture actually
    /// being bound (RE-069), so any test exercising them needs one.
    fn bind_a_texture() -> [Cmd; 4] {
        [
            Cmd::SetTimg {
                format: 0,
                size: 2,
                width: 32,
                addr: SegAddr(0x100),
                slot: 0,
            },
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
        ]
    }

    #[test]
    fn opaque_render_mode_is_neither_alpha_tested_nor_translucent() {
        // RE-069: `G_RM_OPA_SURF`/`G_RM_OPA_SURF2` -- the RDP reset's actual
        // default (`sSYRdpResetDisplayList`) -- sets `FORCE_BL`, but its
        // equation is `(color_src_a=CLR_IN, factor_a=G_BL_0, color_src_b=CLR_IN,
        // factor_b=G_BL_1)`: 0% old plus 100% new, i.e. no real blending.
        // `FORCE_BL` alone cannot be the "is this translucent" signal.
        const FORCE_BL: u32 = 0x4000;
        const CLR_IN: u32 = 0;
        const G_BL_0: u32 = 3;
        const G_BL_1: u32 = 2;
        let data = render_mode(
            FORCE_BL,
            (CLR_IN, G_BL_0, CLR_IN, G_BL_1),
            (CLR_IN, G_BL_0, CLR_IN, G_BL_1),
        );
        let file = vertex_data(3);
        let cmds = [vtx(3), set_render_mode(data), Cmd::Tri1([0, 1, 2]), Cmd::End];
        let m = convert(&cmds, Source::bare(&file)).unwrap().primitives[0].material;
        assert!(!m.translucent, "FORCE_BL alone must not imply real blending");
        assert!(!m.alpha_test);
    }

    #[test]
    fn xlu_render_mode_is_translucent() {
        // G_RM_XLU_SURF's equation reads the framebuffer (CLR_MEM) weighted
        // by 1-alpha (G_BL_1MA) -- the real "over" blend.
        const FORCE_BL: u32 = 0x4000;
        const CLR_IN: u32 = 0;
        const A_IN: u32 = 0;
        const CLR_MEM: u32 = 1;
        const G_BL_1MA: u32 = 0;
        let data = render_mode(
            FORCE_BL,
            (CLR_IN, A_IN, CLR_MEM, G_BL_1MA),
            (CLR_IN, A_IN, CLR_MEM, G_BL_1MA),
        );
        let file = vertex_data(3);
        let mut cmds = alloc::vec![vtx(3)];
        cmds.extend(bind_a_texture());
        cmds.extend([set_render_mode(data), Cmd::Tri1([0, 1, 2]), Cmd::End]);
        let m = convert(&cmds, Source::bare(&file)).unwrap().primitives[0].material;
        assert!(m.translucent);
        assert!(!m.alpha_test);
    }

    #[test]
    fn tex_edge_render_mode_is_alpha_tested_not_translucent() {
        // G_RM_AA_ZB_TEX_EDGE sets both CVG_X_ALPHA and ALPHA_CVG_SEL, with
        // an otherwise-opaque equation -- a cutout surface, not a blended one.
        const CVG_X_ALPHA: u32 = 0x1000;
        const ALPHA_CVG_SEL: u32 = 0x2000;
        const FORCE_BL: u32 = 0x4000;
        const CLR_IN: u32 = 0;
        const G_BL_0: u32 = 3;
        const G_BL_1: u32 = 2;
        let data = render_mode(
            CVG_X_ALPHA | ALPHA_CVG_SEL | FORCE_BL,
            (CLR_IN, G_BL_0, CLR_IN, G_BL_1),
            (CLR_IN, G_BL_0, CLR_IN, G_BL_1),
        );
        let file = vertex_data(3);
        let mut cmds = alloc::vec![vtx(3)];
        cmds.extend(bind_a_texture());
        cmds.extend([set_render_mode(data), Cmd::Tri1([0, 1, 2]), Cmd::End]);
        let m = convert(&cmds, Source::bare(&file)).unwrap().primitives[0].material;
        assert!(m.alpha_test);
        assert!(!m.translucent);
    }

    #[test]
    fn alpha_test_and_translucent_are_gated_on_having_a_real_texture() {
        // RE-069: untextured lit geometry's vertex alpha byte is a packed
        // normal component, not a coverage value (see `push_vertex`'s doc
        // comment) -- alpha-testing or blending against it discarded whole
        // primitives outright (46 of 380 `alpha_test`, 7 of 362
        // `translucent` primitives archive-wide had no texture at all).
        // Both render modes must fall back to off without a texture bound.
        const CVG_X_ALPHA: u32 = 0x1000;
        const ALPHA_CVG_SEL: u32 = 0x2000;
        const CLR_IN: u32 = 0;
        const A_IN: u32 = 0;
        const CLR_MEM: u32 = 1;
        const G_BL_1MA: u32 = 0;
        let edge = render_mode(
            CVG_X_ALPHA | ALPHA_CVG_SEL,
            (CLR_IN, 3, CLR_IN, 2),
            (CLR_IN, 3, CLR_IN, 2),
        );
        let xlu = render_mode(
            0,
            (CLR_IN, A_IN, CLR_MEM, G_BL_1MA),
            (CLR_IN, A_IN, CLR_MEM, G_BL_1MA),
        );
        let file = vertex_data(3);

        let cmds = [vtx(3), set_render_mode(edge), Cmd::Tri1([0, 1, 2]), Cmd::End];
        let m = convert(&cmds, Source::bare(&file)).unwrap().primitives[0].material;
        assert!(!m.alpha_test, "no texture bound: cutout mode must not discard everything");

        let cmds = [vtx(3), set_render_mode(xlu), Cmd::Tri1([0, 1, 2]), Cmd::End];
        let m = convert(&cmds, Source::bare(&file)).unwrap().primitives[0].material;
        assert!(!m.translucent, "no texture bound: must not blend against a meaningless alpha");
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

    #[test]
    fn mirror_is_flagged_only_alongside_a_real_repeat_period() {
        // RE-067: Dream Land's canopy gradient (file 104 offset 0x798) sets
        // exactly `cm_s=3 cm_t=3 mask_s=6 mask_t=6` -- mirror+clamp on both
        // axes, 64-texel period -- reproduced verbatim here.
        let file = vertex_data(3);
        let settile = |cm_s: u8, cm_t: u8, mask_s: u8, mask_t: u8| Cmd::SetTile {
            format: 2,
            size: 0,
            line: 0,
            tmem: 0,
            tile: 0,
            palette: 0,
            cm_s,
            cm_t,
            mask_s,
            mask_t,
            shift_s: 0,
            shift_t: 0,
        };
        let mirrored = |cm_s, cm_t, mask_s, mask_t| {
            let cmds = [
                vtx(3),
                Cmd::SetTimg {
                    format: 0,
                    size: 2,
                    width: 1,
                    addr: SegAddr(0x100),
                    slot: 0,
                },
                settile(cm_s, cm_t, mask_s, mask_t),
                Cmd::SetTileSize {
                    tile: 0,
                    uls: 0,
                    ult: 0,
                    lrs: 255 << 2,
                    lrt: 255 << 2,
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
            convert(&cmds, Source::bare(&file)).unwrap().primitives[0]
                .material
                .texture
                .unwrap()
        };

        let canopy = mirrored(3, 3, 6, 6);
        assert!(canopy.mirror_s && canopy.mirror_t, "cm=3 (mirror+clamp) with a real period must flag mirror");

        let clamp_only = mirrored(2, 2, 6, 6);
        assert!(
            !clamp_only.mirror_s && !clamp_only.mirror_t,
            "cm=2 (clamp, no mirror bit) must not flag mirror"
        );

        let mirror_no_period = mirrored(3, 3, 0, 0);
        assert!(
            !mirror_no_period.mirror_s && !mirror_no_period.mirror_t,
            "mirror with mask=0 has no period to bounce at, so it must not be flagged"
        );

        let s_only = mirrored(3, 2, 6, 6);
        assert!(s_only.mirror_s && !s_only.mirror_t, "axes are independent");
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

    /// A `G_SETTIMG` implies texturing is meant to be active (RE-093/094:
    /// real stage data reissues the whole texture chain with no `Texture{on:
    /// true}` of its own), so the disabled case has to be tested with an
    /// *explicit* `Texture{on: false}` after full state is present, not by
    /// omitting `Texture{on: true}` and relying on some other field being
    /// incidentally absent too.
    #[test]
    fn texture_disabled_means_no_binding() {
        let file = vertex_data(3);
        let cmds = [
            vtx(3),
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
            Cmd::Texture {
                level: 0,
                tile: 0,
                on: false,
                scale_s: 0,
                scale_t: 0,
            },
            Cmd::Tri1([0, 1, 2]),
            Cmd::End,
        ];
        let mesh = convert(&cmds, Source::bare(&file)).unwrap();
        assert!(mesh.primitives[0].material.texture.is_none());
    }

    /// RE-093/094: real stage data (file 105's `StageZebesFile2`, nodes 21-27)
    /// calls one node with an explicit `Texture{on: false}` and no
    /// `G_SETTIMG` of its own (drawing a plain vertex-coloured triangle),
    /// immediately followed by several unrelated nodes that each reissue a
    /// complete `G_SETTIMG`/`G_SETTILE`/`G_LOADBLOCK` chain and draw
    /// genuinely textured geometry -- with no `Texture{on: true}` of their
    /// own to undo the earlier `off`. `Cmd::Texture` alone cannot be the
    /// sole signal for whether a later, unrelated node's own fresh texture
    /// setup is meant to be sampled.
    #[test]
    fn a_later_nodes_own_settimg_overrides_an_inherited_texture_off() {
        use crate::scene::Mat4;

        let file = vertex_data(3);
        let ci4_tile = || Cmd::SetTile {
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
        };
        // Node A: an untextured decal, matching the real shape exactly --
        // no `G_SETTIMG` of its own, just an explicit disable.
        let node_a = [
            Cmd::SetCombine { hi: 0x00FF_FFFF, lo: 0xFFFF_FDFC },
            Cmd::Texture {
                level: 0,
                tile: 0,
                on: false,
                scale_s: 0,
                scale_t: 0,
            },
            vtx(3),
            Cmd::Tri1([0, 1, 2]),
        ];
        // Node B: an unrelated node with its own complete texture chain and
        // no `Texture{on: true}` at all.
        let node_b = [
            ci4_tile(),
            Cmd::SetTimg {
                format: 0,
                size: 2,
                width: 1,
                addr: SegAddr(0x300),
                slot: 0,
            },
            Cmd::LoadTlut { tile: 5, count: 16 },
            Cmd::SetTileSize {
                tile: 0,
                uls: 0,
                ult: 0,
                lrs: 124,
                lrt: 124,
            },
            Cmd::SetTimg {
                format: 2,
                size: 2,
                width: 1,
                addr: SegAddr(0x400),
                slot: 0,
            },
            vtx(3),
            Cmd::Tri1([0, 1, 2]),
        ];
        let items = [
            SequenceItem {
                cmds: &node_a,
                world: Mat4::IDENTITY,
                mobjs: &[],
                mat_anims: &[],
            },
            SequenceItem {
                cmds: &node_b,
                world: Mat4::IDENTITY,
                mobjs: &[],
                mat_anims: &[],
            },
        ];
        let meshes: Vec<_> = convert_sequence(&items, Source::bare(&file))
            .into_iter()
            .map(Result::unwrap)
            .collect();
        assert!(
            meshes[0].primitives[0].material.texture.is_none(),
            "node A's own explicit disable must still hold"
        );
        let tex = meshes[1].primitives[0]
            .material
            .texture
            .expect("node B's own fresh G_SETTIMG must resolve, not inherit node A's off");
        assert_eq!(tex.data_offset, 0x400);
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

    #[test]
    fn texture_blend_reaches_the_material_only_with_a_texture_bound() {
        // Wiring `combiner_texture_blend` into `material_now` end to end
        // (RE-073): Link, Ness, Yoshi and Pikachu's own models all set this
        // shape, and always alongside a real texture -- but it is gated the
        // same way `alpha_test`/`translucent` are (RE-069), since `TEXEL`
        // means nothing without one bound.
        let file = vertex_data(3);
        let (hi, lo) = combine(PRIM, ENV, TEXEL0, ENV, PRIM, ENV, TEXEL0, ENV);
        let lerp = Cmd::SetCombine { hi, lo };
        let prim = Cmd::SetPrimColor {
            m: 0,
            l: 0,
            rgba: [200, 100, 50, 255],
        };
        let env = Cmd::SetEnvColor([10, 20, 30, 255]);
        let bind_texture = [
            Cmd::SetTimg {
                format: 0,
                size: 2,
                width: 32,
                addr: SegAddr(0x100),
                slot: 0,
            },
            Cmd::SetTile {
                format: 0,
                size: 2,
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
                lrt: 31 << 2,
            },
            Cmd::Texture {
                level: 0,
                tile: 0,
                on: true,
                scale_s: 0,
                scale_t: 0,
            },
        ];

        let mut cmds: Vec<Cmd> = Vec::new();
        cmds.extend([vtx(3), lerp, prim, env]);
        cmds.extend(bind_texture);
        cmds.push(Cmd::Tri1([0, 1, 2]));
        cmds.push(Cmd::End);
        let m = convert(&cmds, Source::bare(&file)).unwrap().primitives[0].material;
        assert!(m.texture.is_some(), "the test itself must bind a texture");
        assert_eq!(
            m.texture_blend,
            Some(([10, 20, 30, 255], [200, 100, 50, 255])),
        );

        let cmds = [vtx(3), lerp, prim, env, Cmd::Tri1([0, 1, 2]), Cmd::End];
        let m = convert(&cmds, Source::bare(&file)).unwrap().primitives[0].material;
        assert!(m.texture.is_none(), "no texture bound this time");
        assert_eq!(m.texture_blend, None, "TEXEL means nothing without one");
    }

    #[test]
    fn texture_blend_bakes_the_base_colour_into_the_vertex() {
        // RE-073's shape reads no shade at all, so `push_vertex` must replace
        // the shade with the flat base colour the same way `prim_color`'s
        // scale replaces it -- not leave `vertex_data`'s white untouched,
        // which would draw as if `ENV` were white regardless of its real
        // value.
        let file = vertex_data(3);
        let (hi, lo) = combine(PRIM, ENV, TEXEL0, ENV, PRIM, ENV, TEXEL0, ENV);
        let cmds = [
            vtx(3),
            Cmd::SetCombine { hi, lo },
            Cmd::SetPrimColor {
                m: 0,
                l: 0,
                rgba: [200, 100, 50, 255],
            },
            Cmd::SetEnvColor([10, 20, 30, 255]),
            Cmd::SetTimg {
                format: 0,
                size: 2,
                width: 32,
                addr: SegAddr(0x100),
                slot: 0,
            },
            Cmd::SetTile {
                format: 0,
                size: 2,
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
        assert_eq!(
            mesh.vertices[0].rgba,
            [10, 20, 30, 255],
            "baked to ENV (the base), not left as vertex_data's white shade"
        );
    }

    #[test]
    fn a_flat_colour_combiner_forces_the_primitive_untextured_and_bakes_the_vertex(
    ) {
        // Wiring `combiner_flat_color` into `material_now`/`push_vertex` end
        // to end (RE-079): even with a real texture bound, `TEXEL` never
        // enters this shape's formula, so the primitive must come out
        // untextured (not silently modulated by whatever happened to be
        // bound) and its vertex must carry the flat colour, not the
        // texture-mapping shade `vertex_data` seeds.
        let file = vertex_data(3);
        let (hi, lo) = combine(ZERO_A, ZERO_A, ZERO_C, PRIM, 0, 0, 0, 0);
        let cmds = [
            vtx(3),
            Cmd::SetCombine { hi, lo },
            Cmd::SetPrimColor {
                m: 0,
                l: 0,
                rgba: [10, 20, 30, 255],
            },
            Cmd::SetTimg {
                format: 0,
                size: 2,
                width: 32,
                addr: SegAddr(0x100),
                slot: 0,
            },
            Cmd::SetTile {
                format: 0,
                size: 2,
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
        assert_eq!(
            mesh.primitives[0].material.texture, None,
            "a texture was bound, but this combiner never reads TEXEL"
        );
        assert_eq!(
            mesh.primitives[0].material.flat_color,
            Some([10, 20, 30, 255])
        );
        assert_eq!(mesh.vertices[0].rgba, [10, 20, 30, 255]);
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
    const ONE: u32 = 6;
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
    fn prim_times_shade_is_recognised_even_when_the_primitive_is_black(
    ) {
        // RE-079: 1118 primitives archive-wide set exactly this shape with
        // `PRIM=[0,0,0,255]`. A value-only reading of the evaluated combiner
        // cannot tell "the shade scale is black" apart from "no shade-scale
        // term exists at all" -- both look like an all-zero `s` field -- and
        // silently fell back to unmodified (non-black) vertex shade before
        // this was fixed to track a term's presence separately from its
        // value.
        let (hi, lo) = combine(PRIM, ZERO_A, SHADE, ZERO_D, 0, 0, 0, 0);
        let got = combiner_shade_scale(hi, lo, false, Some([0, 0, 0, 255]), None);
        assert_eq!(got, Some([0.0, 0.0, 0.0]));
    }

    #[test]
    fn multiplying_by_a_true_zero_source_still_reaches_a_later_shade_term() {
        // `(ONE-ZERO)*ZERO+SHADE`: unlike the case above, this cycle's own
        // `C` slot is a literal hardware-zero read (not a real, merely-black
        // colour), so `(ONE-ZERO)*ZERO` must collapse away entirely rather
        // than being treated as "a real term worth keeping, whose value is
        // presently zero" -- otherwise the surviving `+SHADE` term would
        // wrongly carry a phantom constant alongside it and this shape
        // (27 primitives archive-wide) would regress to declined.
        let (hi, lo) = combine(ONE, ZERO_A, ZERO_C, SHADE, 0, 0, 0, 0);
        assert_eq!(
            combiner_shade_scale(hi, lo, false, None, None),
            Some([1.0; 3])
        );
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

    #[test]
    fn prim_env_lerp_is_declined_by_the_shade_scale_reading() {
        // `(PRIM-ENV)*TEXEL+ENV` -- Link, Ness, Yoshi and Pikachu's own
        // models all set this (RE-073). It has no SHADE term at all, which
        // `combiner_shade_scale` cannot fold into anything: it is a genuine
        // additive constant (ENV) plus a texel-scaled term, not a scale on
        // the vertex shade.
        let (hi, lo) = combine(PRIM, ENV, TEXEL0, ENV, PRIM, ENV, TEXEL0, ENV);
        let prim = Some([255, 255, 255, 255]);
        let env = Some([0, 0, 0, 255]);
        assert_eq!(combiner_shade_scale(hi, lo, true, prim, env), None);
    }

    #[test]
    fn prim_env_lerp_is_recognised_as_a_texture_blend() {
        // Same shape as above, read the other way: a blend from ENV (at
        // TEXEL=0) to PRIM (at TEXEL=1) -- exactly the PSP GE's native
        // `TextureEffect::Blend` (RE-073).
        let (hi, lo) = combine(PRIM, ENV, TEXEL0, ENV, PRIM, ENV, TEXEL0, ENV);
        let prim = Some([200, 100, 50, 255]);
        let env = Some([10, 20, 30, 255]);
        assert_eq!(
            combiner_texture_blend(hi, lo, true, prim, env),
            Some(([10, 20, 30, 255], [200, 100, 50, 255])),
        );
    }

    #[test]
    fn a_texture_blend_that_never_reads_primitive_does_not_need_it_set() {
        // `(ONE-ENV)*TEXEL+ENV` -- a blend from ENV to a fixed white target,
        // found archive-wide (RE-079) -- never reads PRIMITIVE at all. The
        // old unconditional `prim?`/`env?` gate declined every occurrence
        // whenever `prim_color` merely hadn't been set, even though nothing
        // in this shape depends on it.
        let (hi, lo) = combine(ONE, ENV, TEXEL0, ENV, ONE, ENV, TEXEL0, ENV);
        let env = Some([40, 60, 80, 255]);
        assert_eq!(
            combiner_texture_blend(hi, lo, true, None, env),
            Some(([40, 60, 80, 255], [255, 255, 255, 255])),
        );
    }

    #[test]
    fn links_own_model_sets_the_lerp_shape_for_real() {
        // The exact word his display list sets at offset 0x11670 (RE-073),
        // not a synthetic stand-in: both cycles are `(PRIM, ENV, TEXEL0,
        // ENV)`.
        let (hi, lo) = (0x0030_9661, 0x552e_ff7f);
        let prim = Some([255, 255, 255, 255]);
        let env = Some([128, 128, 128, 255]);
        assert_eq!(
            combiner_texture_blend(hi, lo, true, prim, env),
            Some(([128, 128, 128, 255], [255, 255, 255, 255])),
        );
    }

    #[test]
    fn a_texture_blend_needs_both_constants_set() {
        // Baking a wrong constant colour in would be a real, visible defect,
        // unlike `combiner_shade_scale`'s safe white-identity default -- so
        // an unset PRIM or ENV must decline rather than guess.
        let (hi, lo) = combine(PRIM, ENV, TEXEL0, ENV, PRIM, ENV, TEXEL0, ENV);
        let some = Some([10, 20, 30, 255]);
        assert_eq!(combiner_texture_blend(hi, lo, true, None, some), None);
        assert_eq!(combiner_texture_blend(hi, lo, true, some, None), None);
    }

    #[test]
    fn combiner_shade_scale_and_texture_blend_do_not_both_accept_the_same_shape() {
        // Every shape one of these two folds into something usable, the
        // other must decline -- overlap would mean two different, disagreeing
        // interpretations of the same bytes are both being trusted.
        let prim = Some([255, 0, 0, 255]);
        let env = Some([0, 0, 255, 255]);
        let cases = [
            combine(PRIM, ENV, TEXEL0, ENV, PRIM, ENV, TEXEL0, ENV),
            combine(PRIM, ZERO_A, SHADE, ZERO_D, 0, 0, 0, 0),
            combine(TEXEL0, ZERO_A, SHADE, ZERO_D, 0, 0, 0, 0),
            combine(ZERO_A, ZERO_A, ZERO_C, PRIM, 0, 0, 0, 0),
            combine(ONE, ZERO_A, ZERO_C, ZERO_D, 0, 0, 0, 0),
        ];
        for (hi, lo) in cases {
            let scale = combiner_shade_scale(hi, lo, true, prim, env);
            let blend = combiner_texture_blend(hi, lo, true, prim, env);
            let flat = combiner_flat_color(hi, lo, true, prim, env);
            let accepted = [scale.is_some(), blend.is_some(), flat.is_some()]
                .iter()
                .filter(|x| **x)
                .count();
            assert!(
                accepted <= 1,
                "more than one accepted hi={hi:#x} lo={lo:#x}: scale={scale:?} blend={blend:?} flat={flat:?}"
            );
        }
    }

    #[test]
    fn a_bare_primitive_colour_is_recognised_as_flat() {
        // `(ZERO-ZERO)*ZERO+PRIM`: no shade, no texel, just a constant.
        // RE-079 found 1,589 of these archive-wide, none of which
        // `combiner_shade_scale` (needs a shade term) or
        // `combiner_texture_blend` (needs a texel term) can express.
        let (hi, lo) = combine(ZERO_A, ZERO_A, ZERO_C, PRIM, 0, 0, 0, 0);
        let prim = Some([10, 20, 30, 255]);
        assert_eq!(
            combiner_flat_color(hi, lo, false, prim, None),
            Some([10, 20, 30, 255])
        );
    }

    #[test]
    fn a_bare_one_needs_neither_constant_set() {
        // `(ZERO-ZERO)*ZERO+ONE` -- 28 occurrences archive-wide (RE-079). A
        // literal `ONE` reads no named colour at all, so it must resolve
        // without either `prim_color` or `env_color` set -- the same
        // reasoning as `a_texture_blend_that_never_reads_primitive_does_not_need_it_set`,
        // applied to the flat-colour shape.
        let (hi, lo) = combine(ZERO_A, ZERO_A, ZERO_C, ONE, 0, 0, 0, 0);
        assert_eq!(
            combiner_flat_color(hi, lo, false, None, None),
            Some([255, 255, 255, 255])
        );
    }

    #[test]
    fn a_flat_colour_combiner_still_declines_an_unset_constant_it_reads() {
        // Unlike `combiner_shade_scale`'s safe white-identity default, a
        // flat colour has nothing safe to substitute for a `PRIMITIVE` the
        // shape actually reads but the display list never set.
        let (hi, lo) = combine(ZERO_A, ZERO_A, ZERO_C, PRIM, 0, 0, 0, 0);
        assert_eq!(combiner_flat_color(hi, lo, false, None, None), None);
    }
}
