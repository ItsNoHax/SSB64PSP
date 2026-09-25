//! What a material animation's primitives sample, against the ROM (RE-326).
//!
//! `romtool matcolors` proves the resolvers pick the texture, palette and
//! tile window `gcDrawMObjForDObj` would. This module checks the two steps
//! after that:
//!
//! * [`check_texels`]: each packed texture a `MatAnimDesc` can bind holds
//!   the texels of the `MObjSub.sprites[]` image it stands for, and each
//!   packed CLUT the colours of the `palettes[]` entry it stands for.
//! * [`UvSampler`]: `ssb_rom::skeleton::MaterialUv::ge_affine`, the GE
//!   scale and offset the renderer installs, lands every packed vertex on
//!   the texel the RDP samples for the live window.

use std::collections::BTreeSet;

use ssb_rom::archive::{Archive, File};
use ssb_rom::mobj::Ptr;
use ssb_rom::pack::{Pack, TextureDesc};
use ssb_rom::psp_texture as psp;
use ssb_rom::texture::{self, BitSize, Format};

/// Level 0 of a packed texture, `width` x `height` texels from the top
/// left, as palette indices (`T4`/`T8`) or RGBA8.
pub(crate) enum Texels {
    Indices(Vec<u8>),
    Rgba(Vec<[u8; 4]>),
}

/// Decodes level 0 of a packed texture over its logical size.
pub(crate) fn packed_texels(pack: &Pack<'_>, t: &TextureDesc) -> Option<Texels> {
    let bits = match t.psm {
        0..=2 => 16,
        3 => 32,
        4 => 4,
        _ => 8,
    };
    let stride_bytes = (t.stride as usize * bits).div_ceil(8);
    let data = pack.texture_data(t)?;
    let rows = if t.swizzled != 0 {
        (psp::pad_to_power_of_two(t.height as u32) as usize).max(8)
    } else {
        t.height as usize
    };
    let raw = data.get(..stride_bytes * rows)?;
    let bytes = if t.swizzled != 0 {
        psp::unswizzle(raw, stride_bytes, rows)
    } else {
        raw.to_vec()
    };
    let (w, h) = (t.width as usize, t.height as usize);
    let u16_at = |o: usize| u16::from_le_bytes([bytes[o], bytes[o + 1]]);
    let expand = |v: u16, n: u32| {
        let max = (1u16 << n) - 1;
        ((v as u32 * 255 + max as u32 / 2) / max as u32) as u8
    };
    Some(match t.psm {
        4 | 5 => Texels::Indices(
            (0..h)
                .flat_map(|y| (0..w).map(move |x| (x, y)))
                .map(|(x, y)| {
                    if t.psm == 4 {
                        let b = bytes[y * stride_bytes + x / 2];
                        if x & 1 == 0 {
                            b & 0x0F
                        } else {
                            b >> 4
                        }
                    } else {
                        bytes[y * stride_bytes + x]
                    }
                })
                .collect(),
        ),
        psm => Texels::Rgba(
            (0..h)
                .flat_map(|y| (0..w).map(move |x| (x, y)))
                .map(|(x, y)| match psm {
                    3 => {
                        let o = y * stride_bytes + x * 4;
                        [bytes[o], bytes[o + 1], bytes[o + 2], bytes[o + 3]]
                    }
                    0 => {
                        let v = u16_at(y * stride_bytes + x * 2);
                        [
                            expand(v & 0x1F, 5),
                            expand((v >> 5) & 0x3F, 6),
                            expand(v >> 11, 5),
                            255,
                        ]
                    }
                    1 => {
                        let v = u16_at(y * stride_bytes + x * 2);
                        [
                            expand(v & 0x1F, 5),
                            expand((v >> 5) & 0x1F, 5),
                            expand((v >> 10) & 0x1F, 5),
                            if v & 0x8000 != 0 { 255 } else { 0 },
                        ]
                    }
                    _ => {
                        let v = u16_at(y * stride_bytes + x * 2);
                        [
                            expand(v & 0xF, 4),
                            expand((v >> 4) & 0xF, 4),
                            expand((v >> 8) & 0xF, 4),
                            expand(v >> 12, 4),
                        ]
                    }
                })
                .collect(),
        ),
    })
}

/// The largest per-channel error a packed format can add to an exact
/// RGBA8 source texel.
fn tolerance(psm: u8) -> u8 {
    match psm {
        0 | 1 => 8,
        2 => 17,
        _ => 0,
    }
}

/// Where `ptr` points, as a file and a byte offset in it.
fn home(archive: &Archive, rom_file: &File, ptr: Ptr) -> Result<(File, usize), String> {
    let file = match ptr.file {
        Some(id) => archive.load(u32::from(id)).map_err(|e| format!("{e:?}"))?,
        None => rom_file.clone(),
    };
    Ok((file, ptr.offset as usize))
}

/// One texture's comparison.
#[derive(Default)]
pub(crate) struct TexelResult {
    pub texels: usize,
    pub bad: usize,
    /// First differing texel: `(x, y, rom, packed)`.
    pub first: Option<(usize, usize, String, String)>,
    /// Packed size over source period, per axis: positive where the axis
    /// mirrors past the period, negative where it repeats.
    pub layout: (isize, isize),
    /// The render `(fmt, siz)` compared.
    pub format: (u8, u8),
}

/// Compares a packed texture with the `fmt`/`siz` image at `ptr`. An
/// index texture compares indices; its CLUT is checked apart
/// ([`palette_matches`]). A direct-colour texture compares RGBA to the
/// packed format's precision.
///
/// The packer bakes a mirrored axis into a texture twice the source size
/// (RE-067), so each axis is tried at full and half size with the second
/// half mirrored; the layout with the fewest differences is reported.
/// `G_LOADBLOCK` source rows are whole 64-bit words.
pub(crate) fn compare_texture(
    archive: &Archive,
    rom_file: &File,
    pack: &Pack<'_>,
    t: &TextureDesc,
    ptr: Ptr,
    fmt: u8,
    siz: u8,
) -> Result<TexelResult, String> {
    let mut format = Format::from_raw(fmt).ok_or("MObjSub.fmt is not a format")?;
    let mut size = BitSize::from_raw(siz).ok_or("MObjSub.siz is not a size")?;
    // `fmt`/`siz` describe the image `gDPSetTextureImage` loads, often as
    // 16-bit words for `G_LOADBLOCK`; the render tile reads it as the
    // display list's own `G_SETTILE` format. An index texture keeps that
    // format as its PSM.
    match t.psm {
        4 => (format, size) = (Format::Ci, BitSize::Bits4),
        5 => (format, size) = (Format::Ci, BitSize::Bits8),
        _ => {}
    }
    let (file, at) = home(archive, rom_file, ptr)?;
    let packed = packed_texels(pack, t).ok_or("packed texture unreadable")?;
    let (w, h) = (t.width as usize, t.height as usize);
    // A direct-colour render tile's format is the display list's
    // `G_SETTILE`, which the pack does not keep: try each the PSP format
    // can hold, starting from the `MObjSub`'s load format.
    let candidates: Vec<(Format, BitSize)> = if format == Format::Ci {
        vec![(format, size)]
    } else {
        let mut c = vec![(format, size)];
        for f in [
            (Format::Rgba, BitSize::Bits16),
            (Format::Rgba, BitSize::Bits32),
            (Format::Ia, BitSize::Bits4),
            (Format::Ia, BitSize::Bits8),
            (Format::Ia, BitSize::Bits16),
            (Format::I, BitSize::Bits4),
            (Format::I, BitSize::Bits8),
        ] {
            if !c.contains(&f) {
                c.push(f);
            }
        }
        c
    };
    let mut best: Option<TexelResult> = None;
    for (format, size) in candidates {
    // Per axis: the source period `n` and whether the packed axis mirrors
    // (`2n - 1 - x` on odd periods) or repeats (`x mod n`) past it.
    let axis_layouts = |dim: usize| -> Vec<(usize, bool)> {
        (0..4)
            .map(|j| dim >> j)
            .filter(|&n| n > 0 && dim.is_multiple_of(n))
            .flat_map(|n| [(n, false), (n, true)])
            .filter(|&(n, m)| !(n == dim && m))
            .collect()
    };
    for (sw, mirror_x) in axis_layouts(w) {
    for (sh, mirror_y) in axis_layouts(h) {
        let (mx, my) = (w / sw, h / sh);
        let row = texture::data_len(sw as u32, 1, size).div_ceil(8) * 8;
        let Some(src) = file.data.get(at..at + row * sh) else {
            continue;
        };
        let texel_width = (row * 8 / size.bits()) as u32;
        let fold = |x: usize, n: usize, mirrored: bool| {
            let p = x % (2 * n);
            if mirrored && p >= n {
                2 * n - 1 - p
            } else {
                x % n
            }
        };
        let fx = |x: usize| fold(x, sw, mirror_x);
        let fy = |y: usize| fold(y, sh, mirror_y);
        let mut r = TexelResult::default();
        let note = |r: &mut TexelResult, x: usize, y: usize, rom: String, got: String| {
            r.bad += 1;
            if r.first.is_none() {
                r.first = Some((x, y, rom, got));
            }
        };
        match (&packed, format) {
            (Texels::Indices(got), Format::Ci) => {
                let index = |x: usize, y: usize| {
                    let k = y * texel_width as usize + x;
                    match size {
                        BitSize::Bits4 => {
                            let b = src[k / 2];
                            if k & 1 == 0 {
                                b >> 4
                            } else {
                                b & 0x0F
                            }
                        }
                        _ => src[k],
                    }
                };
                for y in 0..h {
                    for x in 0..w {
                        let rom = index(fx(x), fy(y));
                        let g = got[y * w + x];
                        r.texels += 1;
                        if rom != g {
                            note(&mut r, x, y, rom.to_string(), g.to_string());
                        }
                    }
                }
            }
            (Texels::Rgba(got), _) if format != Format::Ci => {
                let image = texture::decode(src, texel_width, sh as u32, format, size, None)
                    .map_err(|e| format!("{e:?}"))?;
                let tol = tolerance(t.psm);
                for y in 0..h {
                    for x in 0..w {
                        let rom = image.get(fy(y) * texel_width as usize + fx(x));
                        let g = got[y * w + x];
                        r.texels += 1;
                        let close = (0..4).all(|c| rom[c].abs_diff(g[c]) <= tol)
                            // 5551 keeps one alpha bit: the RDP's `a >= 128`.
                            && (t.psm != 1 || (rom[3] >= 128) == (g[3] == 255));
                        if !close {
                            note(&mut r, x, y, format!("{rom:?}"), format!("{g:?}"));
                        }
                    }
                }
            }
            _ => return Err(format!("packed psm {} does not carry fmt {fmt}", t.psm)),
        }
        r.layout = (
            if mirror_x { mx as isize } else { -(mx as isize) },
            if mirror_y { my as isize } else { -(my as isize) },
        );
        r.format = (format as u8, size as u8);
        if best.as_ref().is_none_or(|b| r.bad < b.bad) {
            best = Some(r);
        }
    }
    }
    }
    best.ok_or_else(|| "sprite image runs off its file".to_string())
}

/// Whether a packed CLUT holds the RGBA5551 palette at `ptr`: the top five
/// bits of each colour channel and the alpha bit.
pub(crate) fn palette_matches(
    archive: &Archive,
    rom_file: &File,
    words: &[u8],
    ptr: Ptr,
) -> Result<bool, String> {
    let (file, at) = home(archive, rom_file, ptr)?;
    let n = words.len() / 4;
    let rom = file
        .data
        .get(at..at + 2 * n)
        .ok_or("palette runs off its file")?;
    Ok((0..n).all(|e| {
        let v = u16::from_be_bytes([rom[2 * e], rom[2 * e + 1]]);
        let w = u32::from_le_bytes(words[4 * e..4 * e + 4].try_into().unwrap());
        let top = |shift: u32| ((w >> shift) as u16 & 0xF8) >> 3;
        top(0) == (v >> 11) & 0x1F
            && top(8) == (v >> 6) & 0x1F
            && top(16) == (v >> 1) & 0x1F
            && (w >> 24 != 0) == (v & 1 != 0)
    }))
}

/// The `MObjSub` fields `gcDrawMObjForDObj` builds tile 0 and `gSPTexture`
/// from.
pub(crate) struct SubWindow {
    /// `sub.flags`, with `MOBJ_FLAG_NONE` already read as `0xA1`.
    pub flags: u16,
    pub unk08: u16,
    pub unk0a: u16,
    pub unk0c: u16,
    pub unk0e: u16,
    pub unk10: i32,
    /// `trau`, `trav`, `scau`, `scav` at rest.
    pub rest: [f32; 4],
}

const TILE0: u16 = 0x20;
const TEXTURE: u16 = 0x80;
const SCALE_EPS: f32 = 1.0 / 65535.0;

/// What `gcDrawMObjForDObj` hands the RDP for a window: `uls`/`ult` as the
/// `s32` it computes (quarter texels), and `gSPTexture`'s `s`/`t`.
/// `objdisplay.c:1353-1420`; `unk10 == 1` is not modelled.
fn rdp_window(w: &SubWindow, uv: [f32; 4]) -> (Option<[i32; 2]>, Option<[u32; 2]>) {
    let [trau, trav, scau, scav] = uv;
    let (c, e, a) = (w.unk0c as f32, w.unk0e as f32, w.unk0a as f32);
    let origin = (w.flags & TILE0 != 0).then(|| {
        if w.unk10 == 2 {
            let uls = if scau.abs() > SCALE_EPS { ((c * trau) / scau) * 4.0 } else { 0.0 };
            let ult = if scav.abs() > SCALE_EPS { ((e * trav) / scav) * 4.0 } else { 0.0 };
            [(uls as i32).max(0), (ult as i32).max(0)]
        } else {
            let uls = if scau.abs() > SCALE_EPS { (((c * trau) + a) / scau) * 4.0 } else { 0.0 };
            let ult = if scav.abs() > SCALE_EPS {
                (((((1.0 - scav) - trav) * e) + a) / scav) * 4.0
            } else {
                0.0
            };
            [uls as i32, ult as i32]
        }
    });
    let scale = (w.flags & TEXTURE != 0).then(|| {
        let (s, t) = if w.unk10 == 2 {
            (
                if scau.abs() > SCALE_EPS { (c * 64.0) / scau } else { 0.0 },
                if scav.abs() > SCALE_EPS { (e * 64.0) / scav } else { 0.0 },
            )
        } else {
            let k = 2097152.0 / w.unk08 as f32;
            (
                if scau.abs() > SCALE_EPS { k / scau } else { 0.0 },
                if scav.abs() > SCALE_EPS { k / scav } else { 0.0 },
            )
        };
        [(s as i32).min(0xFFFF) as u32, (t as i32).min(0xFFFF) as u32]
    });
    (origin, scale)
}

/// Texel-space errors between the GE and the RDP, per primitive and frame.
#[derive(Default)]
pub(crate) struct UvTally {
    pub prims: usize,
    /// Primitives on the texgen path, which [`check_uv`] does not model.
    pub texgen: usize,
    /// Primitives owning only the scale, over a list's own tile origin.
    pub dl_origin: usize,
    /// Vertex-axis samples on frames the window differs from rest.
    pub samples: usize,
    pub bad: usize,
    pub max_err: f64,
    /// The same at rest, where the runtime draws the packed mapping.
    pub rest_samples: usize,
    pub rest_bad: usize,
    pub rest_max: f64,
    /// Two-tile blend second passes, every frame they draw.
    pub tile1_prims: usize,
    pub tile1_samples: usize,
    pub tile1_bad: usize,
    pub tile1_max: f64,
}

/// One texel, the tolerance: finer than a 10.5 vertex coordinate.
const TOLERANCE: f64 = 1.0 / 64.0;

/// Compares, for every packed vertex of every primitive whose tile 0 or
/// `G_TEXTURE` scale entry `i`'s `MObj` owns, the texel the GE samples
/// under [`ssb_rom::skeleton::MaterialUv::ge_affine`] with the texel the
/// RDP samples under `gcDrawMObjForDObj`'s live window.
///
/// The RDP side starts from the packed coordinate: the packer loaded it
/// under the rest `gSPTexture` scale and, on a clamped axis, subtracted the
/// rest tile origin (`mesh::Builder::push_vertex`). A live frame loads the
/// same vertex under the live scale and subtracts the live origin, which
/// `gDPSetTileSize` holds in a 12-bit field. A repeating axis compares
/// modulo the texture period; a clamped one after clamping to the tile.
pub(crate) fn check_uv(
    pack: &Pack<'_>,
    i: u32,
    sub: &SubWindow,
    live: &[[f32; 4]],
    got: &[Option<ssb_rom::skeleton::MaterialUv>],
    tally: &mut UvTally,
    notes: &mut Vec<String>,
) {
    use ssb_rom::pack::flags;
    let (rest_origin, rest_scale) = rdp_window(sub, sub.rest);
    let moving: Vec<usize> = (0..live.len())
        .filter(|&n| live[n].map(f32::to_bits) != sub.rest.map(f32::to_bits))
        .collect();
    for m in 0..pack.mesh_count() {
        let Some(md) = pack.mesh(m) else { continue };
        let Some(verts) = pack.vertices(&md) else { continue };
        for pi in md.first_prim..md.first_prim + md.prim_count {
            let Some(p) = pack.prim(pi) else { continue };
            let owns = p.flags & (flags::TILE0_ANIM | flags::SCALE_ANIM);
            if p.mat_anim != i || owns == 0 {
                continue;
            }
            let Some(t) = pack.texture(p.texture) else { continue };
            let texgen =
                p.flags & flags::TEXTURE_GEN != 0 && p.flags & flags::TEXTURE_GEN_LINEAR == 0;
            let owns_tile0 = p.flags & flags::TILE0_ANIM != 0;
            if !owns_tile0 {
                tally.dl_origin += 1;
                continue;
            }
            let Some(rest_origin) = rest_origin else { continue };
            tally.prims += 1;
            let dims = psp::ge_texture_dims(t.width as u32, t.height as u32);
            let dim = [dims.0 as f64, dims.1 as f64];
            let tile = [t.width as f64, t.height as f64];
            let clamp = [
                t.wrap & TextureDesc::CLAMP_S != 0,
                t.wrap & TextureDesc::CLAMP_T != 0,
            ];
            // The origin the packer baked out of a clamped axis: the rest
            // `uls`/`ult`, as `mobj::read` clamps it.
            let baked = rest_origin.map(|o| o.clamp(0, 0xFFFF) as f64 / 4.0);
            let idx = pack.indices(&p).unwrap_or(&[]);
            let mut coords: BTreeSet<(i16, i16)> = idx
                .as_chunks::<2>()
                .0
                .iter()
                .map(|&b| u16::from_le_bytes(b) as usize * ssb_rom::pack::VERTEX_SIZE)
                .filter_map(|at| verts.get(at..at + 4))
                .map(|v| (i16::from_le_bytes([v[0], v[1]]), i16::from_le_bytes([v[2], v[3]])))
                .collect();
            if texgen {
                // `G_TEXTURE_GEN` replaces the vertex coordinate with
                // `((dot + 1) / 4) * scale` (S10.5) before the same tile
                // origin and window apply; sample its range, less the
                // origin the texture matrix bakes out of a clamped axis.
                tally.texgen += 1;
                let origin = [p.texgen_origin_s, p.texgen_origin_t];
                let gscale = [p.texgen_scale_s, p.texgen_scale_t];
                coords = (0..=8)
                    .map(|q| {
                        let g = |axis: usize| {
                            let s10_5 = (q as f64 / 8.0) * 0.5 * gscale[axis] as f64;
                            let c = if clamp[axis] { origin[axis] as f64 * 8.0 } else { 0.0 };
                            (s10_5 - c) as i16
                        };
                        (g(0), g(1))
                    })
                    .collect();
            }
            let diff = |a: f64, b: f64, axis: usize| {
                if clamp[axis] {
                    (a.clamp(0.0, tile[axis] - 1.0) - b.clamp(0.0, tile[axis] - 1.0)).abs()
                } else {
                    let d = (a - b).rem_euclid(dim[axis]);
                    d.min(dim[axis] - d)
                }
            };
            // N64: the vertex under `scale`, less `origin`'s 12-bit field.
            let n64 = |x: f64, axis: usize, origin: [i32; 2], k: f64| {
                let c = if clamp[axis] { baked[axis] } else { 0.0 };
                (x + c) * k - (origin[axis] & 0xFFF) as f64 / 4.0
            };
            let mut first: Option<String> = None;
            // Rest: the runtime draws the packed coordinate.
            for &(u, v) in &coords {
                for (axis, q) in [(0, u), (1, v)] {
                    let x = q as f64 / 32.0;
                    let e = diff(x, n64(x, axis, rest_origin, 1.0), axis);
                    tally.rest_samples += 1;
                    tally.rest_max = tally.rest_max.max(e);
                    if e > TOLERANCE {
                        tally.rest_bad += 1;
                    }
                }
            }
            for &n in &moving {
                let (origin, scale) = rdp_window(sub, live[n]);
                let origin = origin.unwrap_or(rest_origin);
                let k = match (scale, rest_scale) {
                    (Some(s), Some(r)) if p.flags & flags::SCALE_ANIM != 0 => {
                        [s[0] as f64 / r[0] as f64, s[1] as f64 / r[1] as f64]
                    }
                    _ => [1.0, 1.0],
                };
                let affine = got[n]
                    .and_then(|uv| uv.owned_by(p.flags))
                    .map_or(ssb_rom::skeleton::UvAffine::IDENTITY, |uv| {
                        uv.ge_affine(dims, (clamp[0], clamp[1]))
                    });
                let a = [affine.scale_s as f64, affine.scale_t as f64];
                let b = [affine.offset_s as f64, affine.offset_t as f64];
                for &(u, v) in &coords {
                    for (axis, q) in [(0, u), (1, v)] {
                        let x = q as f64 / 32.0;
                        let ge = x * a[axis] + dim[axis] * b[axis];
                        let want = n64(x, axis, origin, k[axis]);
                        let e = diff(ge, want, axis);
                        tally.samples += 1;
                        tally.max_err = tally.max_err.max(e);
                        if e > TOLERANCE {
                            tally.bad += 1;
                            if first.is_none() {
                                first = Some(format!(
                                    "frame {}: prim {pi} texture {}x{} {} axis {} coordinate {x}: \
                                     GE {ge:.4}, RDP {want:.4} (origin {:?}, k {:.5})",
                                    n + 1,
                                    t.width,
                                    t.height,
                                    if clamp[axis] { "clamped" } else { "repeating" },
                                    ["S", "T"][axis],
                                    origin,
                                    k[axis]
                                ));
                            }
                        }
                    }
                }
            }
            if let Some(f) = first {
                notes.push(format!("sampling: {f}"));
            }
        }
    }
}

/// Tile 1's size inputs for [`check_uv_tile1`]: `unk38`/`unk3A`.
pub(crate) struct Tile1 {
    pub unk38: u16,
    pub unk3a: u16,
}

/// `gDPSetTileSize(1, ...)`'s `uls`/`ult` (`objdisplay.c:1386-1397`).
fn rdp_tile1(w: &SubWindow, t: &Tile1, sca: [f32; 2], scroll: [f32; 2]) -> [i32; 2] {
    let [scau, scav] = sca;
    let [scrollu, scrollv] = scroll;
    let (a, c, e) = (w.unk0a as f32, t.unk38 as f32, t.unk3a as f32);
    let uls = if scau.abs() > SCALE_EPS { (((c * scrollu) + a) / scau) * 4.0 } else { 0.0 };
    let ult = if scav.abs() > SCALE_EPS {
        (((((1.0 - scav) - scrollv) * e) + a) / scav) * 4.0
    } else {
        0.0
    };
    [uls as i32, ult as i32]
}

/// [`check_uv`] for the two-tile blend's second pass (RE-321): the same
/// packed coordinates, sampled through tile 1's window in the next image.
/// The packed coordinate carries tile 0's baked origin, so a tile-1 origin
/// that differs from it at rest shows here too.
#[allow(clippy::too_many_arguments)]
pub(crate) fn check_uv_tile1(
    pack: &Pack<'_>,
    i: u32,
    sub: &SubWindow,
    tile1: &Tile1,
    live: &[[f32; 4]],
    scroll: &[[f32; 2]],
    got: &[Option<ssb_rom::skeleton::LodBlendState>],
    tally: &mut UvTally,
    notes: &mut Vec<String>,
) {
    use ssb_rom::pack::flags;
    let (rest_origin, rest_scale) = rdp_window(sub, sub.rest);
    for m in 0..pack.mesh_count() {
        let Some(md) = pack.mesh(m) else { continue };
        let Some(verts) = pack.vertices(&md) else { continue };
        for pi in md.first_prim..md.first_prim + md.prim_count {
            let Some(p) = pack.prim(pi) else { continue };
            if p.mat_anim != i || p.flags & flags::LOD_BLEND == 0 {
                continue;
            }
            let Some(t0) = pack.texture(p.texture) else { continue };
            tally.tile1_prims += 1;
            let clamp0 = [
                t0.wrap & TextureDesc::CLAMP_S != 0,
                t0.wrap & TextureDesc::CLAMP_T != 0,
            ];
            let baked = rest_origin.unwrap_or([0, 0]).map(|o| o.clamp(0, 0xFFFF) as f64 / 4.0);
            let idx = pack.indices(&p).unwrap_or(&[]);
            let coords: BTreeSet<(i16, i16)> = idx
                .as_chunks::<2>()
                .0
                .iter()
                .map(|&b| u16::from_le_bytes(b) as usize * ssb_rom::pack::VERTEX_SIZE)
                .filter_map(|at| verts.get(at..at + 4))
                .map(|v| (i16::from_le_bytes([v[0], v[1]]), i16::from_le_bytes([v[2], v[3]])))
                .collect();
            let mut first: Option<String> = None;
            for (n, g) in got.iter().enumerate() {
                let Some(g) = g else { continue };
                let Some(t1) = pack.texture(g.texture) else { continue };
                let dims = psp::ge_texture_dims(t1.width as u32, t1.height as u32);
                let dim = [dims.0 as f64, dims.1 as f64];
                let tile = [t1.width as f64, t1.height as f64];
                let clamp1 = [
                    t1.wrap & TextureDesc::CLAMP_S != 0,
                    t1.wrap & TextureDesc::CLAMP_T != 0,
                ];
                let origin = rdp_tile1(sub, tile1, [live[n][2], live[n][3]], scroll[n]);
                let (_, scale) = rdp_window(sub, live[n]);
                let k = match (scale, rest_scale) {
                    (Some(s), Some(r)) => [s[0] as f64 / r[0] as f64, s[1] as f64 / r[1] as f64],
                    _ => [1.0, 1.0],
                };
                let affine = g
                    .uv
                    .map_or(ssb_rom::skeleton::UvAffine::IDENTITY, |uv| {
                        uv.ge_affine(dims, (clamp1[0], clamp1[1]))
                    });
                let a = [affine.scale_s as f64, affine.scale_t as f64];
                let b = [affine.offset_s as f64, affine.offset_t as f64];
                for &(u, v) in &coords {
                    for (axis, q) in [(0, u), (1, v)] {
                        let x = q as f64 / 32.0;
                        let c = if clamp0[axis] { baked[axis] } else { 0.0 };
                        let want = (x + c) * k[axis] - (origin[axis] & 0xFFF) as f64 / 4.0;
                        let ge = x * a[axis] + dim[axis] * b[axis];
                        let e = if clamp1[axis] {
                            (ge.clamp(0.0, tile[axis] - 1.0) - want.clamp(0.0, tile[axis] - 1.0)).abs()
                        } else {
                            let d = (ge - want).rem_euclid(dim[axis]);
                            d.min(dim[axis] - d)
                        };
                        tally.tile1_samples += 1;
                        tally.tile1_max = tally.tile1_max.max(e);
                        if e > TOLERANCE {
                            tally.tile1_bad += 1;
                            if first.is_none() {
                                first = Some(format!(
                                    "frame {}: prim {pi} tile 1 {} axis {} coordinate {x}: GE {ge:.4}, RDP {want:.4} (origin {origin:?}, tile-0 baked {c})",
                                    n + 1,
                                    if clamp1[axis] { "clamped" } else { "repeating" },
                                    ["S", "T"][axis],
                                ));
                            }
                        }
                    }
                }
            }
            if let Some(f) = first {
                notes.push(format!("sampling: {f}"));
            }
        }
    }
}
