//! N64 textures → PSP texture formats.
//!
//! The measured inventory (`docs/rendering.md`) says the dominant case by a
//! wide margin is **CI4 with a 16-entry palette** — 1192 of ~1500 tile setups.
//! The PSP supports 4-bit paletted textures natively, so that case converts
//! almost 1:1 and stays at 4 bits per texel. That matters: after the two
//! framebuffers and the depth buffer, only ~700 KiB of VRAM is left.
//!
//! ## Swizzling
//!
//! The GE reads textures through a cache organised in 16-byte-wide,
//! 8-row blocks. A linearly stored texture makes each cache line span a single
//! row, so vertical locality is lost and any non-axis-aligned sampling thrashes
//! the cache. **Swizzling** reorders texels so each 16x8-byte block is
//! contiguous, which is what `sceGuTexMode`'s swizzle flag expects.
//!
//! This is a large win on PSP and costs nothing at runtime because it is done
//! here, at build time.

use alloc::vec::Vec;

use crate::texture::{BitSize, Format, Rgba8, TextureError};

/// PSP texture storage format (`TexturePixelFormat`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Psm {
    /// 16-bit 5:6:5, no alpha.
    Psm5650,
    /// 16-bit 5:5:5:1 — the natural home for N64 RGBA16.
    Psm5551,
    /// 16-bit 4:4:4:4.
    Psm4444,
    /// 32-bit RGBA.
    Psm8888,
    /// 4-bit palette index.
    PsmT4,
    /// 8-bit palette index.
    PsmT8,
}

impl Psm {
    /// Bits per texel.
    pub fn bits(self) -> usize {
        match self {
            Psm::PsmT4 => 4,
            Psm::PsmT8 => 8,
            Psm::Psm5650 | Psm::Psm5551 | Psm::Psm4444 => 16,
            Psm::Psm8888 => 32,
        }
    }

    /// Whether the format indexes a CLUT.
    pub fn is_paletted(self) -> bool {
        matches!(self, Psm::PsmT4 | Psm::PsmT8)
    }
}

/// A texture ready to hand to `sceGuTexImage`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PspTexture {
    pub width: u32,
    pub height: u32,
    /// Row stride in texels. The GE requires a power of two, so this may exceed
    /// `width` (see [`pad_to_power_of_two`]).
    pub stride: u32,
    pub format: Psm,
    /// Texel data, swizzled if `swizzled` is set.
    pub data: Vec<u8>,
    pub swizzled: bool,
    /// CLUT entries as 32-bit ABGR, for paletted formats.
    pub palette: Vec<u32>,
    /// Mip levels held in `data`, level 0 first. Always at least 1.
    pub levels: u32,
}

impl PspTexture {
    /// Bytes of texel data (excluding the palette), across every mip level.
    pub fn data_size(&self) -> usize {
        self.data.len()
    }

    /// Total VRAM footprint including the CLUT.
    pub fn vram_size(&self) -> usize {
        self.data_size() + self.palette.len() * 4
    }
}

/// Packs an RGBA8888 colour into the PSP's 32-bit ABGR word order.
///
/// The PSP stores colour as `0xAABBGGRR` — the byte order is the reverse of
/// what "RGBA" suggests, and getting it wrong swaps red and blue in a way that
/// is easy to miss on greyscale test data.
pub fn pack_abgr(rgba: [u8; 4]) -> u32 {
    (rgba[3] as u32) << 24 | (rgba[2] as u32) << 16 | (rgba[1] as u32) << 8 | rgba[0] as u32
}

/// Packs RGBA8888 into 16-bit 5:5:5:1, in the PSP's bit order (`ABGR1555`).
pub fn pack_5551(rgba: [u8; 4]) -> u16 {
    let r = (rgba[0] >> 3) as u16;
    let g = (rgba[1] >> 3) as u16;
    let b = (rgba[2] >> 3) as u16;
    let a = if rgba[3] >= 128 { 1u16 } else { 0 };
    (a << 15) | (b << 10) | (g << 5) | r
}

/// The GE requires texture dimensions to be powers of two.
pub fn pad_to_power_of_two(v: u32) -> u32 {
    v.max(1).next_power_of_two()
}

/// Divisor a packed 16-bit vertex texture coordinate is normalised by before
/// `sceGuTexScale` is applied (`GU_TEXTURE_16BIT`).
const VERTEX_16BIT_DIVISOR: f32 = 32768.0;

/// `sceGuTexScale` factor for one axis of an **authored-UV** primitive.
///
/// A packed UV is the N64's S10.5 fixed point (32 units per texel) carried in
/// a `GU_TEXTURE_16BIT` field, which the GE divides by 32768 before scaling.
/// The wanted result is `(uv / 32) / dim`, so the factor is
/// `32768 / (32 * dim)` = `1024 / dim`.
///
/// `uploaded_dim` must be the dimension actually handed to `sceGuTexImage`
/// (the padded/strided one), not the logical tile size.
pub fn authored_uv_tex_scale(uploaded_dim: u32) -> f32 {
    (VERTEX_16BIT_DIVISOR / 32.0) / uploaded_dim.max(1) as f32
}

/// `sceGuTexScale` factor for one axis of a `G_TEXTURE_GEN` primitive drawn
/// through the GE's environment-map coordinate generator.
///
/// The two pipelines produce coordinates on different scales, so the
/// authored-UV factor above is simply wrong here — using it multiplies an
/// already-normalised generated coordinate by ~1024/dim and repeats the
/// reflection many times over.
///
/// F3DEX (`refs/BattleShip`'s interpreter, `GfxSpVertex`'s `G_TEXTURE_GEN`
/// branch) generates, for a unit vertex normal `n` and unit look-at basis
/// vector `l`:
///
/// ```text
/// dot = clamp(n · l, -1, 1)
/// S10.5 coordinate = ((dot + 1) / 4) * gSPTexture_scale
/// texels           = S10.5 / 32 = (dot + 1) * gSPTexture_scale / 128
/// ```
///
/// The PSP GE's `GU_ENVIRONMENT_MAP` generates a *normalised* coordinate from
/// the same dot product:
///
/// ```text
/// u          = (1 + n · l) / 2
/// texels     = u * sceGuTexScale * uploaded_dim
/// ```
///
/// Equating the two gives `sceGuTexScale = gSPTexture_scale / (64 * dim)`.
///
/// This is corroborated independently by the ROM: every one of the five
/// distinct `G_TEXTURE` scales measured at a real texgen draw (`romtool
/// texgen`) makes the generated span exactly one period of that draw's own
/// tile — e.g. `0x07C0` on a 32x32 tile yields `31/32`, `0x0BC0`/`0x0A40` on
/// 48x42 yield `47/48` and `41/42`.
pub fn env_map_tex_scale(gsp_texture_scale: u16, uploaded_dim: u32) -> f32 {
    gsp_texture_scale as f32 / (64.0 * uploaded_dim.max(1) as f32)
}

/// `1 / (2*pi)`, the constant `G_TEXTURE_GEN_LINEAR`'s curve scales `acos` by.
const INV_TWO_PI: f32 = 0.5 / core::f32::consts::PI;

/// `acos`, routed through `std` on the host and a minimax polynomial on the
/// device -- same convention as `ssb-engine::math::sqrt`/`sin_cos`: no `libm`
/// dependency for game code. The polynomial is a widely used minimax
/// approximation (NVIDIA's Cg `acos`); measured against `std`'s `acos`
/// across the full `[-1, 1]` domain in
/// `acos_matches_std_within_measured_error` below, max error ~6.6e-5 rad.
///
/// Not a lookup table: a LUT is only exact if its *input* domain is finite,
/// and [`texgen_dot`]'s input is not -- only the vertex normal is quantised
/// (`i8`), but the look-at basis it is dotted against is a continuous float
/// that changes with camera orientation every frame. A LUT keyed on the
/// normal alone would therefore have to interpolate or accept error just
/// like this polynomial does, for no accuracy gain, while adding a build-time
/// table and a runtime gather. 257 triangles archive-wide (`RE-214`'s census)
/// also means the call count this replaces is small enough that a polynomial
/// evaluation is very unlikely to be measurable against everything else one
/// frame does.
#[cfg(feature = "std")]
fn acos(x: f32) -> f32 {
    x.acos()
}

#[cfg(not(feature = "std"))]
fn acos(x: f32) -> f32 {
    acos_poly(x)
}

/// The polynomial itself, kept compiled under `std` too (behind `cfg(test)`)
/// so `acos_matches_std_within_measured_error` can check it against `std`'s
/// `acos` directly rather than trusting the approximation blind.
#[cfg(any(not(feature = "std"), test))]
fn acos_poly(x: f32) -> f32 {
    let negate = if x < 0.0 { 1.0 } else { 0.0 };
    let x = if x < 0.0 { -x } else { x };
    let mut ret = -0.0187293f32;
    ret = ret * x + 0.0742610;
    ret = ret * x - 0.2121144;
    ret = ret * x + 1.5707288;
    ret *= sqrt_approx(1.0 - x);
    ret -= 2.0 * negate * ret;
    negate * core::f32::consts::PI + ret
}

/// Newton-Raphson square root for [`acos_poly`]. Not exported --
/// `ssb-engine::math::sqrt` is the public equivalent, but this crate has no
/// dependency on `ssb-engine` and the domain here (`[0, 2]`) is narrow enough
/// not to need one.
#[cfg(any(not(feature = "std"), test))]
fn sqrt_approx(v: f32) -> f32 {
    if v <= 0.0 {
        return 0.0;
    }
    let mut x = f32::from_bits((v.to_bits() >> 1) + 0x1FC0_0000);
    for _ in 0..4 {
        x = 0.5 * (x + v / x);
    }
    x
}

/// `G_TEXTURE_GEN_LINEAR`'s curve, mapping `dot` in `[-1, 1]` onto `u` in
/// `[0, 0.5]` -- the same output range as the ordinary `(dot + 1) / 4` curve
/// [`env_map_tex_scale`] documents, differing only in shape. Cross-checked
/// against two independent implementations that agree exactly:
/// `refs/BattleShip`'s `GfxSpVertex` (`acosf(-dotx) * 0.159155f`, its own
/// comment noting that constant is `1/(2*pi)`) and `refs/n64psp`'s
/// `n64psp_texgen_snorm8_batch_scalar` (`acosf(-dot_s) * inverse_two_pi`).
pub fn linear_texgen_curve(dot: f32) -> f32 {
    let dot = dot.clamp(-1.0, 1.0);
    acos(-dot) * INV_TWO_PI
}

/// The RSP's own dot product feeding [`linear_texgen_curve`] (and the
/// ordinary curve): the raw, **un-renormalised** `i8` vertex normal against a
/// unit look-at basis vector, divided by 127. Not a square-root
/// renormalisation -- `refs/BattleShip`'s `GfxSpVertex` and `refs/n64psp`'s
/// `n64psp_texgen_snorm8_batch_scalar` both divide by the constant 127
/// rather than the normal's real length, and this project's GE-hardware
/// ordinary-texgen path already accepts that hardware normalises instead
/// (D-038) as a *separate* deviation from this one.
pub fn texgen_dot(normal: [i8; 3], basis: [f32; 3]) -> f32 {
    let dot =
        normal[0] as f32 * basis[0] + normal[1] as f32 * basis[1] + normal[2] as f32 * basis[2];
    dot / 127.0
}

/// The ordinary (non-`LINEAR`) `G_TEXTURE_GEN` curve: `(dot + 1) / 4`,
/// `env_map_tex_scale`'s own derivation. The *only* difference from
/// [`linear_texgen_curve`] (`PLAN.md` R2.1/T4's own wording) -- both take the
/// same [`texgen_dot`] input, cover the same `[0, 0.5]` output range, and feed
/// the same [`texgen_s10_5_addressed`] scale-and-addressing step below; they
/// differ only in shape (affine here, `acos` there).
pub fn regular_texgen_curve(dot: f32) -> f32 {
    (dot.clamp(-1.0, 1.0) + 1.0) / 4.0
}

/// One axis of a texgen curve (either [`linear_texgen_curve`] or
/// [`regular_texgen_curve`]) converted into the pack's raw S10.5 fixed-point
/// unit (`PackedVertex::u`/`v`, 32 units per texel) -- the same unit
/// `S10.5 = u * gSPTexture_scale` `env_map_tex_scale`'s doc comment uses --
/// and addressed against the render tile's origin on a clamped axis, exactly
/// parallel to `mesh::Builder::push_vertex`'s authored-UV bake
/// (`v.uv[0] -= origin_s * 8`). The `* 8` there and here is the same
/// S10.2-to-S10.5 scale alignment; see `push_vertex`'s own comment for the
/// quarter-texel origin unit. Shared by both curves: "the only curve
/// difference... followed by common scale and addressing" (`PLAN.md`
/// R2.1/T4).
///
/// Truncates, does not round (`PLAN.md` R2.1/T5, RE-229): two independent
/// reference implementations of this exact conversion both cast straight to
/// an integer with no `+ 0.5` -- `refs/n64psp`'s `n64psp_texgen_to_s10_5`
/// (`tnl_scalar.c`, `(int16_t)scaled`) and `refs/BattleShip`'s
/// `Interpreter::GfxSpTexture`-fed conversion (`interpreter.cpp`,
/// `(int32_t)(dotx * texture_scaling_factor.s)`). An earlier version of this
/// function added `0.5` before casting (round-half-up); T5's boundary tests
/// below pin the corrected truncating behavior.
fn texgen_s10_5_addressed(curve: f32, gsp_texture_scale: u16, origin: u16, clamp: bool) -> i16 {
    let s10_5 = (curve * gsp_texture_scale as f32) as i32;
    let addressed = s10_5 - if clamp { origin as i32 * 8 } else { 0 };
    addressed.clamp(i16::MIN as i32, i16::MAX as i32) as i16
}

/// Generates one linear-texgen vertex's `(u, v)` in the pack's raw S10.5
/// unit, so a linear-texgen primitive can be drawn through the ordinary
/// authored-UV pipeline rather than needing a second GE mode (D-040).
#[allow(clippy::too_many_arguments)]
pub fn linear_texgen_uv(
    normal: [i8; 3],
    basis_s: [f32; 3],
    basis_t: [f32; 3],
    scale_s: u16,
    scale_t: u16,
    origin_s: u16,
    origin_t: u16,
    clamp_s: bool,
    clamp_t: bool,
) -> (i16, i16) {
    (
        texgen_s10_5_addressed(
            linear_texgen_curve(texgen_dot(normal, basis_s)),
            scale_s,
            origin_s,
            clamp_s,
        ),
        texgen_s10_5_addressed(
            linear_texgen_curve(texgen_dot(normal, basis_t)),
            scale_t,
            origin_t,
            clamp_t,
        ),
    )
}

/// Generates one *ordinary* (non-`LINEAR`) texgen vertex's `(u, v)` in the
/// pack's raw S10.5 unit -- the source-formula reference this project's real
/// rendering path (the PSP GE's texture-matrix generator,
/// `meshdraw::apply_texture_mapping`/`regular_texgen_matrix_coeffs`) is
/// proven against, not something this project draws through directly (the GE
/// generates its own coordinates per vertex in hardware). `PLAN.md` R2.1/T4,
/// RE-228.
#[allow(clippy::too_many_arguments)]
pub fn regular_texgen_uv(
    normal: [i8; 3],
    basis_s: [f32; 3],
    basis_t: [f32; 3],
    scale_s: u16,
    scale_t: u16,
    origin_s: u16,
    origin_t: u16,
    clamp_s: bool,
    clamp_t: bool,
) -> (i16, i16) {
    (
        texgen_s10_5_addressed(
            regular_texgen_curve(texgen_dot(normal, basis_s)),
            scale_s,
            origin_s,
            clamp_s,
        ),
        texgen_s10_5_addressed(
            regular_texgen_curve(texgen_dot(normal, basis_t)),
            scale_t,
            origin_t,
            clamp_t,
        ),
    )
}

/// The affine texture-matrix coefficients `meshdraw::apply_texture_mapping`
/// installs for one axis of the GE's raw-`Normal`-projection `G_TEXTURE_GEN`
/// path: the GE computes `u = a * dot(normal_raw / 128, basis) + b` per
/// vertex in hardware (`normal_raw / 128` is the GE's own measured read,
/// RE-226 -- not this project's choice).
///
/// `a` and `b` need *different* corrections for that `/128` against the
/// original hardware's own `/127` (RE-226's `NORMAL_SCALE_COMPENSATION`):
/// `a` multiplies the normal-dependent dot product, which *is* read through
/// the GE's `/128` divisor, so it must carry the `128.0/127.0` compensation.
/// `b` is the curve's zero-crossing constant (`dot = -1 -> u = 0`, half the
/// normalised span) plus the tile's origin shift -- neither passes through
/// the GE's per-component normal decoder, so compensating it by the same
/// factor overcorrects. RE-228 (`PLAN.md` R2.1/T4) found and fixed exactly
/// this: an earlier version used the same compensated value for both,
/// producing a real (if sub-texel: `scale/127 - scale/128`, `<= 1` S10.5 unit
/// for every real archive scale) systematic offset against
/// [`regular_texgen_uv`]'s independent reference derivation.
pub fn regular_texgen_matrix_coeffs(
    gsp_texture_scale: u16,
    origin: u16,
    clamp: bool,
    uploaded_dim: u32,
) -> (f32, f32) {
    const NORMAL_SCALE_COMPENSATION: f32 = 128.0 / 127.0;
    let half_scale = 0.5 * env_map_tex_scale(gsp_texture_scale, uploaded_dim);
    let a = half_scale * NORMAL_SCALE_COMPENSATION;
    let shift = if clamp {
        // `origin` is quarter-texel S10.2; normalise against the uploaded
        // dimension the coordinate is expressed in.
        -(origin as f32 / 4.0) / uploaded_dim.max(1) as f32
    } else {
        0.0
    };
    let b = half_scale + shift;
    (a, b)
}

/// Swizzles texel data for the GE's texture cache.
///
/// Operates on raw bytes: the GE swizzles in units of **16 bytes by 8 rows**
/// regardless of the texel format, so a 4-bit texture's block covers 32 texels
/// horizontally while a 32-bit texture's covers 4.
///
/// `stride_bytes` is the source row length in bytes and must be a multiple of
/// 16; `height` must be a multiple of 8. Callers should pad first.
pub fn swizzle(src: &[u8], stride_bytes: usize, height: usize) -> Vec<u8> {
    const BLOCK_W: usize = 16;
    const BLOCK_H: usize = 8;

    let mut out = alloc::vec![0u8; stride_bytes * height];
    let blocks_x = stride_bytes / BLOCK_W;
    let blocks_y = height / BLOCK_H;

    let mut dst = 0usize;
    for by in 0..blocks_y {
        for bx in 0..blocks_x {
            // Copy one 16x8 block, row by row, into contiguous output.
            for row in 0..BLOCK_H {
                let s = (by * BLOCK_H + row) * stride_bytes + bx * BLOCK_W;
                out[dst..dst + BLOCK_W].copy_from_slice(&src[s..s + BLOCK_W]);
                dst += BLOCK_W;
            }
        }
    }
    out
}

/// Reverses [`swizzle`]. Used only by tests, to prove the transform is
/// lossless — a swizzler that silently drops texels is hard to spot by eye.
pub fn unswizzle(src: &[u8], stride_bytes: usize, height: usize) -> Vec<u8> {
    const BLOCK_W: usize = 16;
    const BLOCK_H: usize = 8;

    let mut out = alloc::vec![0u8; stride_bytes * height];
    let blocks_x = stride_bytes / BLOCK_W;
    let blocks_y = height / BLOCK_H;

    let mut s = 0usize;
    for by in 0..blocks_y {
        for bx in 0..blocks_x {
            for row in 0..BLOCK_H {
                let d = (by * BLOCK_H + row) * stride_bytes + bx * BLOCK_W;
                out[d..d + BLOCK_W].copy_from_slice(&src[s..s + BLOCK_W]);
                s += BLOCK_W;
            }
        }
    }
    out
}

/// Chooses the PSP format for an N64 `(format, size)` pair.
///
/// Driven by the measured inventory rather than by covering every possibility:
/// paletted N64 formats stay paletted (cheapest in VRAM), RGBA16 maps to the
/// equivalent 16-bit format, and everything else expands to 8888 because the
/// PSP has no intensity/alpha format.
pub fn choose_psm(format: Format, size: BitSize) -> Psm {
    match (format, size) {
        (Format::Ci, BitSize::Bits4) => Psm::PsmT4,
        (Format::Ci, BitSize::Bits8) => Psm::PsmT8,
        // I4/I8 are greyscale ramps; a CLUT keeps them at 4/8 bits instead of
        // expanding 8x to 8888.
        (Format::I, BitSize::Bits4) => Psm::PsmT4,
        (Format::I, BitSize::Bits8) => Psm::PsmT8,
        (Format::Rgba, BitSize::Bits16) => Psm::Psm5551,
        // IA and RGBA32 have no direct PSP equivalent.
        _ => Psm::Psm8888,
    }
}

/// Converts a decoded RGBA8888 image into a PSP texture.
///
/// Non-paletted path. Pads to power-of-two dimensions and swizzles.
pub fn pack_rgba(img: &Rgba8, format: Psm, swizzle_it: bool) -> PspTexture {
    let stride = pad_to_power_of_two(img.width);
    let padded_h = pad_to_power_of_two(img.height);

    let mut data: Vec<u8> = match format {
        Psm::Psm8888 => {
            let mut d = alloc::vec![0u8; (stride * padded_h * 4) as usize];
            for y in 0..img.height {
                for x in 0..img.width {
                    let s = ((y * img.width + x) * 4) as usize;
                    let px = [
                        img.pixels[s],
                        img.pixels[s + 1],
                        img.pixels[s + 2],
                        img.pixels[s + 3],
                    ];
                    let o = ((y * stride + x) * 4) as usize;
                    d[o..o + 4].copy_from_slice(&pack_abgr(px).to_le_bytes());
                }
            }
            d
        }
        Psm::Psm5551 => {
            let mut d = alloc::vec![0u8; (stride * padded_h * 2) as usize];
            for y in 0..img.height {
                for x in 0..img.width {
                    let s = ((y * img.width + x) * 4) as usize;
                    let px = [
                        img.pixels[s],
                        img.pixels[s + 1],
                        img.pixels[s + 2],
                        img.pixels[s + 3],
                    ];
                    let o = ((y * stride + x) * 2) as usize;
                    d[o..o + 2].copy_from_slice(&pack_5551(px).to_le_bytes());
                }
            }
            d
        }
        // Other 16-bit formats are not produced by `choose_psm` today.
        _ => alloc::vec![0u8; (stride * padded_h * format.bits() as u32 / 8) as usize],
    };
    // `PLAN.md` R2.0/P0d: see `pad_edge_repeat`'s doc comment.
    pad_edge_repeat(
        &mut data,
        stride,
        padded_h,
        img.width,
        img.height,
        format.bits() / 8,
    );

    let stride_bytes = (stride as usize * format.bits()).div_ceil(8);
    let swizzled = swizzle_it && can_swizzle(stride_bytes, padded_h as usize);
    if swizzled {
        data = swizzle(&data, stride_bytes, padded_h as usize);
    }

    PspTexture {
        width: img.width,
        height: img.height,
        stride,
        format,
        data,
        swizzled,
        palette: Vec::new(),
        levels: 1,
    }
}

/// Whether a texture's dimensions permit swizzling.
///
/// Small textures whose rows are under 16 bytes cannot be swizzled; the GE
/// wants whole 16x8-byte blocks. Returning false rather than padding further
/// keeps tiny textures small.
pub fn can_swizzle(stride_bytes: usize, height: usize) -> bool {
    stride_bytes >= 16 && stride_bytes.is_multiple_of(16) && height.is_multiple_of(8)
}

/// Fills a packed texture's power-of-two padding region (columns
/// `width..stride`, rows `height..padded_h`) with the repeated real edge
/// row/column, in place of the zero fill every caller starts from
/// (`PLAN.md` R2.0/P0d, RE-220/RE-222): real hardware clamps addressing to
/// the *logical* edge, so `sceGuTexFilter(Linear, Linear)`'s bilinear blend
/// near a clamped, non-power-of-two edge should read real edge data, not a
/// synthetic zero the RDP never produces.
///
/// A no-op when `width == stride` and `height == padded_h` (already
/// power-of-two) — true for every mirrored axis, since a mask period
/// doubled by [`crate::texture::mirror_extend`] is always itself a power of
/// two (RE-220), so this never touches mirrored-axis data. Must run
/// *before* swizzling: it addresses the still-linear row-major buffer every
/// caller builds first.
///
/// `bytes_per_texel` must be a whole number of bytes; `PsmT4`'s 4-bit texels
/// use [`pad_edge_repeat_nibbles`] instead, since a padding boundary can
/// fall mid-byte.
fn pad_edge_repeat(
    data: &mut [u8],
    stride: u32,
    padded_h: u32,
    width: u32,
    height: u32,
    bytes_per_texel: usize,
) {
    let (stride, width, height, padded_h) = (
        stride as usize,
        width as usize,
        height as usize,
        padded_h as usize,
    );
    if width == 0 || height == 0 {
        return;
    }
    let row_bytes = stride * bytes_per_texel;
    if width < stride {
        for y in 0..height {
            let row = y * row_bytes;
            let edge_start = row + (width - 1) * bytes_per_texel;
            let edge: Vec<u8> = data[edge_start..edge_start + bytes_per_texel].to_vec();
            for x in width..stride {
                let o = row + x * bytes_per_texel;
                data[o..o + bytes_per_texel].copy_from_slice(&edge);
            }
        }
    }
    if height < padded_h {
        let last_row = (height - 1) * row_bytes;
        let edge_row: Vec<u8> = data[last_row..last_row + row_bytes].to_vec();
        for y in height..padded_h {
            let o = y * row_bytes;
            data[o..o + row_bytes].copy_from_slice(&edge_row);
        }
    }
}

/// [`pad_edge_repeat`]'s counterpart for `PsmT4`'s two texels per byte, high
/// nibble first (this crate's own convention -- see [`pack_indexed`]'s doc
/// comment) -- a padding boundary can fall mid-byte, so the byte-level copy
/// above cannot address a single padding texel there.
fn pad_edge_repeat_nibbles(data: &mut [u8], stride: u32, padded_h: u32, width: u32, height: u32) {
    let (stride, width, height, padded_h) = (
        stride as usize,
        width as usize,
        height as usize,
        padded_h as usize,
    );
    if width == 0 || height == 0 {
        return;
    }
    let stride_bytes = stride.div_ceil(2);
    let get = |data: &[u8], x: usize, y: usize| -> u8 {
        let byte = data[y * stride_bytes + x / 2];
        if x.is_multiple_of(2) {
            byte >> 4
        } else {
            byte & 0x0F
        }
    };
    let set = |data: &mut [u8], x: usize, y: usize, v: u8| {
        let idx = y * stride_bytes + x / 2;
        if x.is_multiple_of(2) {
            data[idx] = (data[idx] & 0x0F) | (v << 4);
        } else {
            data[idx] = (data[idx] & 0xF0) | (v & 0x0F);
        }
    };
    if width < stride {
        for y in 0..height {
            let edge = get(data, width - 1, y);
            for x in width..stride {
                set(data, x, y, edge);
            }
        }
    }
    if height < padded_h {
        for x in 0..stride {
            let edge = get(data, x, height - 1);
            for y in height..padded_h {
                set(data, x, y, edge);
            }
        }
    }
}

/// Converts a paletted N64 texture, keeping it paletted.
///
/// `indices` are the raw N64 texel indices; `tlut` holds RGBA5551 palette
/// entries. Output is `PsmT4`/`PsmT8` with a 32-bit CLUT.
pub fn pack_paletted(
    indices: &[u8],
    width: u32,
    height: u32,
    size: BitSize,
    tlut: &[u16],
    swizzle_it: bool,
) -> Result<PspTexture, TextureError> {
    let palette: Vec<u32> = tlut
        .iter()
        .map(|&e| pack_abgr(crate::texture::rgba5551(e)))
        .collect();
    pack_indexed(indices, width, height, size, &palette, swizzle_it)
}

/// The CLUT an I4 or I8 texture amounts to.
///
/// `choose_psm` maps intensity formats to `PsmT4`/`PsmT8` so they stay 4 or 8
/// bits per texel rather than expanding eightfold to `Psm8888`. Nothing in the
/// ROM supplies a palette for them, though, because on the N64 they need none:
/// the texel *is* the intensity, driving all four channels including alpha.
/// So the palette is generated, matching `texture::decode`'s expansion exactly
/// — `(v << 4) | v` for the 4-bit ramp.
///
/// Alpha is why this cannot go through `pack_paletted`: an RGBA5551 entry has
/// one alpha bit, and an intensity texture's alpha is its full range.
pub fn intensity_palette(size: BitSize) -> Vec<u32> {
    match size {
        BitSize::Bits4 => (0u8..16).map(|i| ramp((i << 4) | i)).collect(),
        _ => (0u8..=255).map(ramp).collect(),
    }
}

fn ramp(v: u8) -> u32 {
    pack_abgr([v, v, v, v])
}

/// Packs already-indexed texels against a ready RGBA8888 palette.
pub fn pack_indexed(
    indices: &[u8],
    width: u32,
    height: u32,
    size: BitSize,
    palette: &[u32],
    swizzle_it: bool,
) -> Result<PspTexture, TextureError> {
    let format = match size {
        BitSize::Bits4 => Psm::PsmT4,
        BitSize::Bits8 => Psm::PsmT8,
        _ => return Err(TextureError::UnsupportedCombination(Format::Ci, size)),
    };

    let need = crate::texture::data_len(width, height, size);
    if indices.len() < need {
        return Err(TextureError::Truncated {
            need,
            have: indices.len(),
        });
    }

    let stride = pad_to_power_of_two(width);
    let padded_h = pad_to_power_of_two(height);
    let stride_bytes = (stride as usize * format.bits()).div_ceil(8);
    let src_row_bytes = (width as usize * format.bits()).div_ceil(8);

    // Copy row by row into the padded stride.
    let mut data = alloc::vec![0u8; stride_bytes * padded_h as usize];
    for y in 0..height as usize {
        let s = y * src_row_bytes;
        let d = y * stride_bytes;
        data[d..d + src_row_bytes].copy_from_slice(&indices[s..s + src_row_bytes]);
    }

    // The N64 stores the high nibble first within a byte, which is also what
    // the PSP expects for PsmT4, so 4-bit data copies through unchanged.
    let palette = palette.to_vec();

    // `PLAN.md` R2.0/P0d: see `pad_edge_repeat`'s doc comment.
    if format == Psm::PsmT4 {
        pad_edge_repeat_nibbles(&mut data, stride, padded_h, width, height);
    } else {
        pad_edge_repeat(
            &mut data,
            stride,
            padded_h,
            width,
            height,
            format.bits() / 8,
        );
    }

    let swizzled = swizzle_it && can_swizzle(stride_bytes, padded_h as usize);
    if swizzled {
        data = swizzle(&data, stride_bytes, padded_h as usize);
    }

    Ok(PspTexture {
        width,
        height,
        stride,
        format,
        data,
        swizzled,
        palette,
        levels: 1,
    })
}

#[cfg(test)]
mod mip_tests {
    use super::*;

    fn ramp_palette() -> Vec<u32> {
        (0..16u8)
            .map(|i| pack_abgr([i * 17, i * 17, i * 17, 255]))
            .collect()
    }

    /// Level 0 must survive the round trip through RGBA and back to indices.
    /// It is regenerated rather than copied, and that is only safe because the
    /// nearest palette entry to a decoded texel is the entry it came from.
    #[test]
    fn level_zero_is_unchanged_by_regenerating_it() {
        let pal = ramp_palette();
        let mut img = Rgba8::new(16, 16);
        for i in 0..16 * 16 {
            let v = ((i % 16) as u8) * 17;
            img.put(i, [v, v, v, 255]);
        }
        let tex = pack_mipped(&img, Psm::PsmT4, &pal, false);
        // Row 0 cycles through all sixteen entries, high nibble first.
        assert_eq!(tex.data[0], 0x01);
        assert_eq!(tex.data[7], 0xEF);
    }

    /// The point of the chain: averaging a dithered pair lands between palette
    /// entries and snaps to the shade between them, not back onto the dither.
    #[test]
    fn a_dithered_pair_averages_to_the_shade_between() {
        let pal = ramp_palette();
        // A 2x2 checker of entries 0 and 2 should reduce to entry 1.
        let mut img = Rgba8::new(2, 2);
        for (i, v) in [0u8, 34, 34, 0].into_iter().enumerate() {
            img.put(i, [v, v, v, 255]);
        }
        let half = halve(&img);
        assert_eq!(
            nearest_entry(&pal, {
                let p = &half.pixels;
                [p[0], p[1], p[2], p[3]]
            }),
            1
        );
    }

    #[test]
    fn a_chain_stops_rather_than_lose_swizzling() {
        let pal = ramp_palette();
        let img = Rgba8::new(64, 64);
        let swizzled = pack_mipped(&img, Psm::PsmT4, &pal, true);
        assert!(swizzled.swizzled, "64x64 CI4 must still swizzle");
        // 16x16 at 4bpp is an 8-byte stride, below the swizzler's minimum, so
        // the chain stops at 32x32.
        assert_eq!(swizzled.levels, 2);
        // Unswizzled, nothing constrains it and the chain runs to 1x1.
        let full = pack_mipped(&img, Psm::PsmT4, &pal, false);
        assert!(full.levels > 2);
    }

    /// `PLAN.md` R2.0/P0d, end-to-end through the actual production path
    /// (`convert_texture`'s `mipped` closure calls `pack_mipped`, not
    /// `pack_rgba`/`pack_indexed` directly): a non-power-of-two CI4 level 0
    /// pads its padding column with the repeated edge texel, not index 0.
    #[test]
    fn pack_mipped_pads_level_zero_with_the_repeated_edge_texel() {
        let pal = ramp_palette();
        // 3 wide, 1 tall: texels decode to entries 0, 5, 10 (unambiguous
        // under `nearest_entry`, spaced well apart on the 16-entry ramp).
        let mut img = Rgba8::new(3, 1);
        for (i, entry) in [0u8, 5, 10].into_iter().enumerate() {
            let v = entry * 17;
            img.put(i, [v, v, v, 255]);
        }
        let tex = pack_mipped(&img, Psm::PsmT4, &pal, false);
        assert_eq!(tex.stride, 4, "3 pads to a 4-texel stride");
        // Level 0: byte 0 = texels (0,5), byte 1 = texels (10, padding).
        assert_eq!(tex.data[0], 0x05, "texels 0 and 1 unchanged");
        assert_eq!(
            tex.data[1], 0xAA,
            "padding texel (low nibble) repeats the edge texel (entry 10 = 0xA), not index 0"
        );
    }
}

#[cfg(test)]
mod intensity_tests {
    use super::*;

    /// I4 has no ROM palette because on the N64 it needs none; the generated
    /// ramp is what keeps it at 4 bits instead of expanding to 8888 (RE-047).
    #[test]
    fn the_i4_ramp_matches_the_decoder() {
        let pal = intensity_palette(BitSize::Bits4);
        assert_eq!(pal.len(), 16);
        // `texture::decode` expands a nibble as `(v << 4) | v`, and intensity
        // drives alpha too, so index 15 is opaque white and 0 is transparent.
        assert_eq!(pal[0], pack_abgr([0x00, 0x00, 0x00, 0x00]));
        assert_eq!(pal[15], pack_abgr([0xFF, 0xFF, 0xFF, 0xFF]));
        assert_eq!(pal[5], pack_abgr([0x55, 0x55, 0x55, 0x55]));
    }

    #[test]
    fn the_i8_ramp_is_the_identity() {
        let pal = intensity_palette(BitSize::Bits8);
        assert_eq!(pal.len(), 256);
        assert_eq!(pal[200], pack_abgr([200, 200, 200, 200]));
    }

    /// The point of the ramp: an I4 texture stays 4 bits per texel.
    #[test]
    fn an_intensity_texture_packs_at_four_bits() {
        let texels = alloc::vec![0x0Fu8; 32 * 32 / 2];
        let tex = pack_indexed(
            &texels,
            32,
            32,
            BitSize::Bits4,
            &intensity_palette(BitSize::Bits4),
            false,
        )
        .expect("packs");
        assert_eq!(tex.format, Psm::PsmT4);
        // 32x32 at 4bpp is 512 bytes, against 4096 as RGBA8888.
        assert_eq!(tex.data_size(), 512);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::texture::rgba5551;

    #[test]
    fn abgr_packing_is_byte_reversed() {
        // R=0x11 must land in the low byte, not the high one.
        assert_eq!(pack_abgr([0x11, 0x22, 0x33, 0x44]), 0x4433_2211);
        assert_eq!(pack_abgr([255, 0, 0, 255]), 0xFF00_00FF);
    }

    /// Texels one axis spans on the PSP for a full sweep of the dot product,
    /// under the environment-map factor.
    fn env_map_span_texels(scale: u16, dim: u32) -> f32 {
        // dot = -1 -> u = 0; dot = +1 -> u = 1.
        1.0 * env_map_tex_scale(scale, dim) * dim as f32
    }

    #[test]
    fn authored_uv_scale_converts_s10_5_to_normalised_texture_space() {
        // 1024 / dim, for the padded dimension actually uploaded.
        assert_eq!(authored_uv_tex_scale(32), 32.0);
        assert_eq!(authored_uv_tex_scale(64), 16.0);
        assert_eq!(authored_uv_tex_scale(1024), 1.0);
        // One texel (32 in S10.5) on a 64-wide texture must land at 1/64.
        let u = (32.0 / VERTEX_16BIT_DIVISOR) * authored_uv_tex_scale(64);
        assert!((u - 1.0 / 64.0).abs() < 1e-6, "{u}");
        // Never divides by zero.
        assert!(authored_uv_tex_scale(0).is_finite());
    }

    /// Every real `G_TEXTURE` scale/tile pairing `romtool texgen` measured in
    /// the ROM must sweep exactly one period of its own tile — the property
    /// that corroborates the derived formula against real data rather than
    /// against the derivation it came from.
    #[test]
    fn env_map_scale_sweeps_one_tile_period_for_every_real_rom_pairing() {
        // (scale, uploaded dim, expected texels swept)
        for &(scale, dim, want) in &[
            // StageMetalFile2's 32x32 reflection, 2403 triangles.
            (0x07C0u16, 32u32, 31.0f32),
            // 48x42 tile, 356 triangles.
            (0x0BC0, 48, 47.0),
            (0x0A40, 42, 41.0),
            // 64x32 tile, 18 triangles.
            (0x0FC0, 64, 63.0),
            (0x07C0, 32, 31.0),
            // 8x8 tile, 40 triangles.
            (0x01C0, 8, 7.0),
            // Mask-narrowed 16x8 period, 175 triangles.
            (0x0400, 16, 16.0),
            (0x0200, 8, 8.0),
        ] {
            let got = env_map_span_texels(scale, dim);
            assert!(
                (got - want).abs() < 1e-3,
                "scale {scale:#06x} on dim {dim}: swept {got} texels, expected {want}"
            );
        }
    }

    #[test]
    fn env_map_scale_is_native_at_full_gsp_texture_scale() {
        // `0xFFFF` is the SDK's "no scaling" value: the RSP's own generated
        // range is 1024 texels regardless of the texture's size, so a
        // 1024-wide upload maps 1:1 and a 32-wide one repeats 32 times.
        assert!((env_map_span_texels(0xFFFF, 1024) - 1023.98).abs() < 0.05);
        assert!((env_map_span_texels(0xFFFF, 32) - 1023.98).abs() < 0.05);
        // Reduced scale shrinks the span proportionally, independent of dim.
        assert!((env_map_span_texels(0x8000, 32) - 512.0).abs() < 0.01);
    }

    #[test]
    fn env_map_scale_accounts_for_padded_height_separately_from_logical() {
        // A 48x42 tile uploads at height 64 (padded). The span in *texels*
        // must not change, so the normalised factor must shrink to match.
        let logical = env_map_tex_scale(0x0A40, 42);
        let padded = env_map_tex_scale(0x0A40, pad_to_power_of_two(42));
        assert!(padded < logical);
        assert!((1.0 * padded * 64.0 - 41.0).abs() < 1e-3);
    }

    #[test]
    fn env_map_and_authored_scales_differ_by_more_than_a_constant() {
        // Guards against the pre-RE-214 bug of leaving the authored-UV factor
        // active under environment mapping: on a 32x32 reflection texture it
        // was 32x too large, so the reflection repeated 32 times.
        let authored = authored_uv_tex_scale(32);
        let env = env_map_tex_scale(0x07C0, 32);
        assert!((authored / env - 33.03).abs() < 0.05, "{authored} {env}");
    }

    #[test]
    fn linear_curve_hits_its_endpoints_and_shared_midpoint() {
        // Both curves map [-1, 1] onto [0, 0.5]; dot = -1 -> u = 0,
        // dot = +1 -> u = 0.5, and both curves happen to agree at dot = 0
        // too (acos(0)/(2*pi) == (0+1)/4 == 0.25) -- the difference is only
        // in the interior shape, checked below.
        assert!((linear_texgen_curve(-1.0) - 0.0).abs() < 1e-6);
        assert!((linear_texgen_curve(1.0) - 0.5).abs() < 1e-6);
        assert!((linear_texgen_curve(0.0) - 0.25).abs() < 1e-6);
    }

    #[test]
    fn linear_curve_is_complementary_about_its_midpoint() {
        // acos(x) + acos(-x) == pi for every x, so u(dot) + u(-dot) == 0.5
        // for every dot -- the curve's actual symmetry, distinct from being
        // an odd/even function.
        for dot in [-0.9, -0.5, -0.1, 0.3, 0.7, 0.95] {
            let sum = linear_texgen_curve(dot) + linear_texgen_curve(-dot);
            assert!((sum - 0.5).abs() < 1e-5, "dot {dot}: sum {sum}");
        }
    }

    /// A check that cannot fail is not evidence: this must fail if the
    /// linear branch is deleted and both modes fall back to the ordinary
    /// curve. Endpoints and dot = 0 coincide (see the test above), so the
    /// comparison has to use an interior point.
    #[test]
    fn linear_curve_differs_from_the_ordinary_curve_away_from_shared_points() {
        let dot = 0.5;
        let linear = linear_texgen_curve(dot);
        let ordinary = (dot + 1.0) / 4.0;
        assert!(
            (linear - ordinary).abs() > 0.03,
            "linear {linear} too close to ordinary {ordinary}"
        );
    }

    #[test]
    fn acos_matches_std_within_measured_error() {
        // Sweeps the full domain at fine resolution and asserts a bound
        // rather than trusting the approximation blind (`AGENTS.md` #9).
        let mut max_error = 0.0f32;
        let mut x = -1.0f32;
        while x <= 1.0 {
            let error = (acos_poly(x) - x.acos()).abs();
            if error > max_error {
                max_error = error;
            }
            x += 1.0 / 4096.0;
        }
        assert!(max_error < 1.0e-4, "measured max error {max_error} rad");
    }

    #[test]
    fn texgen_dot_divides_by_the_constant_127_not_the_normals_real_length() {
        // [90, 90, 0] has real length ~127.28, not 127 -- if this divided by
        // the real length the two results below would match; the RSP (and
        // both reference implementations) divide by the constant instead, so
        // they must not.
        let by_constant = texgen_dot([90, 90, 0], [1.0, 0.0, 0.0]);
        assert!((by_constant - 90.0 / 127.0).abs() < 1e-6, "{by_constant}");
        let by_real_length = 90.0 / (90.0f32 * 90.0 + 90.0 * 90.0).sqrt();
        assert!((by_constant - by_real_length).abs() > 1e-4);
    }

    /// `PLAN.md` R2.1/T10's texgen test minimum names the zero normal as its
    /// own case, distinct from `dot = 0.0` above (which feeds the curves
    /// directly): a degenerate `[0, 0, 0]` vertex normal must produce
    /// `dot = 0` through `texgen_dot` itself for *any* basis, not divide by
    /// zero or otherwise diverge from the already-proven dot = 0 curve
    /// values.
    #[test]
    fn texgen_dot_of_the_zero_normal_is_zero_for_any_basis() {
        for basis in [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.3, -0.6, 0.74]] {
            let dot = texgen_dot([0, 0, 0], basis);
            assert_eq!(dot, 0.0, "basis {basis:?}: dot {dot}");
        }
        assert!((regular_texgen_curve(0.0) - 0.25).abs() < 1e-6);
        assert!((linear_texgen_curve(0.0) - 0.25).abs() < 1e-6);
    }

    #[test]
    fn linear_texgen_s10_5_matches_the_ordinary_curves_endpoint_for_every_real_rom_scale() {
        // At dot = 1 both curves reach the same S10.5 maximum
        // (`scale / 2`), which is the fixed point the whole derivation in
        // `env_map_tex_scale` rests on -- same five real scales that test
        // checks, this time through the linear path.
        for &scale in &[0x07C0u16, 0x0BC0, 0x0A40, 0x0FC0, 0x01C0, 0x0400, 0x0200] {
            let got = texgen_s10_5_addressed(linear_texgen_curve(1.0), scale, 0, false) as i32;
            let want = (scale as f32 / 2.0).round() as i32;
            assert!(
                (got - want).abs() <= 1,
                "scale {scale:#06x}: got {got}, want {want}"
            );
        }
    }

    #[test]
    fn texgen_s10_5_addressed_truncates_rather_than_rounds_at_half_unit_boundaries() {
        // `PLAN.md` R2.1/T5: N+0.49/0.50/0.51 boundary cases at every real
        // ROM texgen scale (RE-214's census). Truncation means all three
        // land on `n` -- a round-to-nearest implementation (this function's
        // prior, incorrect behavior) would instead land N+0.50/N+0.51 on
        // `n + 1`.
        for &scale in &[0x07C0u16, 0x0BC0, 0x0A40, 0x0FC0, 0x01C0, 0x0400, 0x0200] {
            for &n in &[0i32, 1, 10, 100] {
                for &frac in &[0.49f32, 0.50, 0.51] {
                    let target = n as f32 + frac;
                    let curve = target / scale as f32;
                    let got = texgen_s10_5_addressed(curve, scale, 0, false);
                    assert_eq!(
                        got as i32, n,
                        "scale {scale:#06x} n {n} frac {frac}: got {got}, want {n}"
                    );
                }
            }
        }
    }

    #[test]
    fn linear_texgen_uv_reproduces_file_117_prim_3542() {
        // RE-214's census: file 117, 16x8 tile, scale 0x0400/0x0200, tile
        // origin 6/3, both axes clamped (the only axis kind the census's
        // `env_map_scale_sweeps_one_tile_period_for_every_real_rom_pairing`
        // shift formula applies to).
        let (scale_s, scale_t) = (0x0400u16, 0x0200u16);
        let (origin_s, origin_t) = (6u16, 3u16);

        // A normal aligned with the S basis and perpendicular to T: dot_s = 1,
        // dot_t = 0.
        let (u, v) = linear_texgen_uv(
            [127, 0, 0],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            scale_s,
            scale_t,
            origin_s,
            origin_t,
            true,
            true,
        );
        let want_u = (scale_s as f32 / 2.0).round() as i32 - origin_s as i32 * 8;
        let want_v = (scale_t as f32 / 4.0).round() as i32 - origin_t as i32 * 8;
        assert_eq!(u as i32, want_u);
        assert_eq!(v as i32, want_v);

        // Unclamped: the origin shift must not apply.
        let (u_unclamped, _) = linear_texgen_uv(
            [127, 0, 0],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            scale_s,
            scale_t,
            origin_s,
            origin_t,
            false,
            true,
        );
        assert_eq!(u_unclamped as i32, (scale_s as f32 / 2.0).round() as i32);
    }

    #[test]
    fn packs_5551_with_alpha_threshold() {
        // Opaque white.
        assert_eq!(pack_5551([255, 255, 255, 255]), 0xFFFF);
        // Transparent black.
        assert_eq!(pack_5551([0, 0, 0, 0]), 0x0000);
        // Pure red, opaque: r in low 5 bits, alpha in bit 15.
        assert_eq!(pack_5551([255, 0, 0, 255]), 0x801F);
        // Pure blue, opaque.
        assert_eq!(pack_5551([0, 0, 255, 255]), 0xFC00);
    }

    #[test]
    fn alpha_below_half_becomes_transparent() {
        assert_eq!(pack_5551([0, 0, 0, 127]) >> 15, 0);
        assert_eq!(pack_5551([0, 0, 0, 128]) >> 15, 1);
    }

    #[test]
    fn power_of_two_padding() {
        assert_eq!(pad_to_power_of_two(1), 1);
        assert_eq!(pad_to_power_of_two(8), 8);
        assert_eq!(pad_to_power_of_two(9), 16);
        assert_eq!(pad_to_power_of_two(33), 64);
    }

    /// `PLAN.md` R2.0/P0d: a non-power-of-two 3-wide, 1-tall single-byte-texel
    /// image pads out to a 4-wide stride with the last real column (`2`)
    /// repeated, not left zero.
    #[test]
    fn pad_edge_repeat_fills_column_padding_with_the_last_real_column() {
        let mut data = alloc::vec![9u8, 8, 7, 0]; // stride=4, texels 0..3 real, col 3 is padding
        pad_edge_repeat(&mut data, 4, 1, 3, 1, 1);
        assert_eq!(
            data,
            [9, 8, 7, 7],
            "padding column repeats the edge column (7)"
        );
    }

    /// The row-padding counterpart: a 2-wide, 3-tall image padded to 4 rows
    /// repeats the last real row, and does so *after* that row's own column
    /// padding has already been filled in.
    #[test]
    fn pad_edge_repeat_fills_row_padding_with_the_last_real_row_including_its_own_column_padding() {
        // stride=2 (already power-of-two on this axis), so only row padding
        // applies: rows 0/1/2 real, row 3 padding.
        let mut data = alloc::vec![1u8, 2, 3, 4, 5, 6, 0, 0];
        pad_edge_repeat(&mut data, 2, 4, 2, 3, 1);
        assert_eq!(
            data,
            [1, 2, 3, 4, 5, 6, 5, 6],
            "row padding repeats the last real row (5, 6)"
        );
    }

    /// A power-of-two image needs no padding on either axis: a no-op.
    #[test]
    fn pad_edge_repeat_is_a_no_op_for_an_already_power_of_two_image() {
        let mut data: Vec<u8> = (0u8..16).collect();
        let before = data.clone();
        pad_edge_repeat(&mut data, 4, 4, 4, 4, 1);
        assert_eq!(data, before);
    }

    /// The 4-bit counterpart: a 3-wide, 1-tall CI4 row (two texels per byte,
    /// high nibble first) padded to a 4-texel stride repeats the last real
    /// texel's nibble into the padding nibble, not the neighbouring byte's
    /// unrelated data.
    #[test]
    fn pad_edge_repeat_nibbles_fills_column_padding_with_the_last_real_texel() {
        // Texel 0=0x9 (byte0 high), 1=0x8 (byte0 low), 2=0x7 (byte1 high);
        // byte1 low nibble (texel 3) is padding, currently zero.
        let mut data = alloc::vec![0x98u8, 0x70];
        pad_edge_repeat_nibbles(&mut data, 4, 1, 3, 1);
        assert_eq!(
            data,
            [0x98, 0x77],
            "padding texel 3 repeats texel 2's value (0x7)"
        );
    }

    /// A power-of-two CI4 image needs no padding: a no-op.
    #[test]
    fn pad_edge_repeat_nibbles_is_a_no_op_for_an_already_power_of_two_image() {
        let mut data = alloc::vec![0x12u8, 0x34];
        let before = data.clone();
        pad_edge_repeat_nibbles(&mut data, 4, 1, 4, 1);
        assert_eq!(data, before);
    }

    /// End-to-end through `pack_paletted` (`PLAN.md` R2.0/P0d): a
    /// non-power-of-two CI8 texture's padded columns sample the same
    /// palette index as the real edge column, not index 0.
    #[test]
    fn pack_paletted_pads_a_non_power_of_two_texture_with_the_repeated_edge() {
        // 3 wide, 1 tall, CI8: indices 10, 20, 30.
        let indices = [10u8, 20, 30];
        let tlut: Vec<u16> = alloc::vec![0; 256];
        let tex = pack_paletted(&indices, 3, 1, BitSize::Bits8, &tlut, false).unwrap();
        assert_eq!(tex.stride, 4);
        assert_eq!(
            &tex.data[..],
            [10, 20, 30, 30],
            "padding texel repeats the edge index (30)"
        );
    }

    #[test]
    fn swizzle_round_trips() {
        // A swizzler that drops or duplicates texels is hard to see by eye, so
        // prove it is a bijection.
        let stride = 32usize;
        let height = 16usize;
        let src: Vec<u8> = (0..stride * height).map(|i| (i % 251) as u8).collect();

        let sw = swizzle(&src, stride, height);
        assert_eq!(sw.len(), src.len(), "swizzle must preserve size");
        assert_ne!(sw, src, "swizzle should actually reorder");
        assert_eq!(unswizzle(&sw, stride, height), src, "must round-trip");
    }

    #[test]
    fn swizzle_moves_the_first_block_contiguously() {
        // Row 0 bytes 0..16 then row 1 bytes 0..16 must become adjacent.
        let stride = 32usize;
        let height = 8usize;
        let mut src = alloc::vec![0u8; stride * height];
        for row in 0..height {
            for b in 0..stride {
                src[row * stride + b] = (row * 16 + b / 16) as u8;
            }
        }
        let sw = swizzle(&src, stride, height);
        // First 16 bytes: row 0, block 0. Next 16: row 1, block 0.
        assert_eq!(sw[0], 0);
        assert_eq!(sw[16], 16);
    }

    #[test]
    fn swizzle_requires_whole_blocks() {
        assert!(can_swizzle(32, 8));
        assert!(can_swizzle(16, 16));
        assert!(!can_swizzle(8, 8), "row under 16 bytes");
        assert!(!can_swizzle(32, 4), "height not a multiple of 8");
        assert!(!can_swizzle(24, 8), "stride not a multiple of 16");
    }

    #[test]
    fn format_choice_follows_the_measured_inventory() {
        // The dominant case must stay at 4 bits per texel.
        assert_eq!(choose_psm(Format::Ci, BitSize::Bits4), Psm::PsmT4);
        assert_eq!(choose_psm(Format::Ci, BitSize::Bits8), Psm::PsmT8);
        // Intensity ramps stay paletted rather than expanding 8x.
        assert_eq!(choose_psm(Format::I, BitSize::Bits4), Psm::PsmT4);
        assert_eq!(choose_psm(Format::Rgba, BitSize::Bits16), Psm::Psm5551);
        // No PSP equivalent: expand.
        assert_eq!(choose_psm(Format::Ia, BitSize::Bits16), Psm::Psm8888);
        assert_eq!(choose_psm(Format::Rgba, BitSize::Bits32), Psm::Psm8888);
    }

    /// Expands a packed CI4 texture back to RGBA, reversing swizzle and CLUT.
    ///
    /// Exists to answer one question precisely: is the *packing* correct, or is
    /// a bad on-device image the GE's fault? Without this, a texture that
    /// renders as noise gives no way to tell those apart.
    fn unpack_ci4(tex: &PspTexture) -> Vec<[u8; 4]> {
        let stride_bytes = (tex.stride as usize * 4).div_ceil(8);
        let padded_h = tex.data.len() / stride_bytes.max(1);
        let linear = if tex.swizzled {
            unswizzle(&tex.data, stride_bytes, padded_h)
        } else {
            tex.data.clone()
        };

        let mut out = Vec::new();
        for y in 0..tex.height as usize {
            for x in 0..tex.width as usize {
                let byte = linear[y * stride_bytes + x / 2];
                // The N64 stores the first texel in the high nibble.
                let idx = if x % 2 == 0 { byte >> 4 } else { byte & 0x0F } as usize;
                let entry = tex.palette[idx];
                out.push([
                    entry as u8,
                    (entry >> 8) as u8,
                    (entry >> 16) as u8,
                    (entry >> 24) as u8,
                ]);
            }
        }
        out
    }

    /// The packed texture must reproduce the source image exactly.
    #[test]
    fn ci4_packing_round_trips_through_swizzle_and_clut() {
        let w = 64u32;
        let h = 32u32;

        // A recognisable pattern: index varies with both axes, so a transposed
        // or block-shuffled result cannot pass by coincidence.
        let mut indices = alloc::vec![0u8; (w * h / 2) as usize];
        for y in 0..h as usize {
            for x in 0..w as usize {
                let idx = ((x / 4 + y / 2) % 16) as u8;
                let at = y * (w as usize / 2) + x / 2;
                if x % 2 == 0 {
                    indices[at] = (indices[at] & 0x0F) | (idx << 4);
                } else {
                    indices[at] = (indices[at] & 0xF0) | idx;
                }
            }
        }
        // A palette where every entry is distinguishable.
        let tlut: Vec<u16> = (0..16u16)
            .map(|i| (i << 11) | (i << 6) | (i << 1) | 1)
            .collect();

        for swizzled in [false, true] {
            let tex = pack_paletted(&indices, w, h, BitSize::Bits4, &tlut, swizzled).unwrap();
            assert_eq!(tex.swizzled, swizzled, "swizzle flag must be honoured");

            let got = unpack_ci4(&tex);
            assert_eq!(got.len(), (w * h) as usize);

            for y in 0..h as usize {
                for x in 0..w as usize {
                    let expect_idx = (x / 4 + y / 2) % 16;
                    let expect = rgba5551(tlut[expect_idx]);
                    let actual = got[y * w as usize + x];
                    assert_eq!(
                        actual, expect,
                        "texel ({x},{y}) wrong (swizzled={swizzled})"
                    );
                }
            }
        }
    }

    #[test]
    fn ci4_stays_four_bits_per_texel() {
        // 32x8 CI4 with a 16-entry palette: the dominant real-world case.
        let indices = alloc::vec![0x01u8; 32 * 8 / 2];
        let tlut: Vec<u16> = (0..16).map(|i| (i as u16) << 11 | 1).collect();

        let tex = pack_paletted(&indices, 32, 8, BitSize::Bits4, &tlut, true).unwrap();

        assert_eq!(tex.format, Psm::PsmT4);
        assert_eq!(tex.palette.len(), 16);
        // 32 texels * 4 bits = 16 bytes per row, 8 rows.
        assert_eq!(tex.data_size(), 32 * 8 / 2);
        assert!(tex.swizzled, "16-byte rows and 8 rows can be swizzled");
        // 128 bytes of texels + 64 bytes of CLUT.
        assert_eq!(tex.vram_size(), 128 + 64);
    }

    #[test]
    fn ci4_is_eight_times_smaller_than_expanding_to_8888() {
        let indices = alloc::vec![0u8; 64 * 64 / 2];
        let tlut: Vec<u16> = alloc::vec![0xFFFF; 16];
        let tex = pack_paletted(&indices, 64, 64, BitSize::Bits4, &tlut, true).unwrap();

        let expanded_8888 = 64 * 64 * 4;
        assert_eq!(tex.data_size(), 64 * 64 / 2);
        assert_eq!(expanded_8888 / tex.data_size(), 8);
    }

    #[test]
    fn non_power_of_two_width_is_padded() {
        let indices = alloc::vec![0u8; 24 * 8]; // 24 wide, CI8
        let tlut: Vec<u16> = alloc::vec![0; 256];
        let tex = pack_paletted(&indices, 24, 8, BitSize::Bits8, &tlut, false).unwrap();

        assert_eq!(tex.width, 24, "logical width preserved");
        assert_eq!(tex.stride, 32, "stride padded to a power of two");
        assert_eq!(tex.data_size(), 32 * 8);
    }

    #[test]
    fn truncated_index_data_is_rejected() {
        let tlut: Vec<u16> = alloc::vec![0; 16];
        assert!(matches!(
            pack_paletted(&[0u8; 4], 64, 64, BitSize::Bits4, &tlut, false),
            Err(TextureError::Truncated { .. })
        ));
    }

    #[test]
    fn rgba16_packs_to_5551_at_two_bytes_per_texel() {
        let mut img = Rgba8::new(16, 8);
        for i in 0..16 * 8 {
            img.pixels[i * 4] = 255; // red
            img.pixels[i * 4 + 3] = 255; // opaque
        }
        let tex = pack_rgba(&img, Psm::Psm5551, true);

        assert_eq!(tex.format, Psm::Psm5551);
        assert_eq!(tex.data_size(), 16 * 8 * 2);
        assert!(tex.swizzled);
        assert!(tex.palette.is_empty());
    }

    #[test]
    fn tiny_textures_are_left_unswizzled() {
        // 4x8 at 8 bits = 4-byte rows, below the 16-byte block width.
        let img = Rgba8::new(4, 8);
        let tex = pack_rgba(&img, Psm::Psm8888, true);
        // 4 texels * 4 bytes = 16 bytes/row, which *is* swizzlable.
        assert!(tex.swizzled);

        // But a 2-wide 8888 texture has 8-byte rows.
        let img = Rgba8::new(2, 8);
        let tex = pack_rgba(&img, Psm::Psm8888, true);
        assert!(!tex.swizzled, "rows under 16 bytes cannot swizzle");
    }

    /// Pure-math transcription of what the GE's texture-matrix multiply
    /// computes for one axis of `G_TEXTURE_GEN`, given
    /// [`regular_texgen_matrix_coeffs`]'s `(a, b)`: `a * dot(normal/128,
    /// basis) + b`. Not a hardware measurement -- RE-226 already pinned the
    /// GE's `/128` normal read, and a 4x4 matrix multiply with a fixed
    /// translation column is ordinary linear algebra, not a new thing to
    /// measure -- so this can run on the host for thousands of cases instead
    /// of needing real `sceGu` for every one of them (`PLAN.md` R2.1/T4).
    fn simulate_ge_matrix_output(normal: [i8; 3], basis: [f32; 3], a: f32, b: f32) -> f32 {
        let dot128 = normal[0] as f32 / 128.0 * basis[0]
            + normal[1] as f32 / 128.0 * basis[1]
            + normal[2] as f32 / 128.0 * basis[2];
        a * dot128 + b
    }

    /// One axis, end to end: builds the GE's matrix coefficients, simulates
    /// the hardware multiply, and converts the normalised `[0, 1]`-ish result
    /// into the same S10.5 unit [`regular_texgen_uv`] returns (`texels * 32`,
    /// `texels = normalised * uploaded_dim`), including the clamped-origin
    /// shift already folded into `b`.
    fn simulate_ge_lowering_s10_5(
        normal: [i8; 3],
        basis: [f32; 3],
        gsp_texture_scale: u16,
        origin: u16,
        clamp: bool,
        uploaded_dim: u32,
    ) -> f32 {
        let (a, b) = regular_texgen_matrix_coeffs(gsp_texture_scale, origin, clamp, uploaded_dim);
        simulate_ge_matrix_output(normal, basis, a, b) * uploaded_dim as f32 * 32.0
    }

    /// RE-228 (`PLAN.md` R2.1/T4): thousands of random normals, bases,
    /// scales, origins and dimensions, comparing the GE-matrix simulation
    /// above against [`regular_texgen_uv`]'s independent source-formula
    /// reference. Exact `i16` S10.5 equality isn't claimed or required here
    /// (the two paths round at different points in the computation); the
    /// assertion instead bounds the *measured* maximum error, which this test
    /// prints on failure -- the "otherwise document maximum error" half of
    /// the plan's own acceptance text.
    /// A random point on the unit sphere, by rejection sampling the unit
    /// cube -- good enough for property testing without a Box-Muller
    /// dependency. Both the real vertex normal and the real look-at basis
    /// [`texgen_object_basis`] feeds this generator are unit-length (the
    /// normal by quantized-byte convention, the basis by
    /// `texgen_object_basis`'s own explicit `normalize` step), so a
    /// realistic test input must be too -- a non-unit basis or a
    /// normal with independently-random axes (magnitude up to `sqrt(3)`x
    /// real) can push `dot` past `[-1, 1]` for reasons that have nothing to
    /// do with the formula under test.
    fn random_unit_vector(rng: &mut crate::particle::Rng) -> [f32; 3] {
        loop {
            let v = [
                rng.next_float() * 2.0 - 1.0,
                rng.next_float() * 2.0 - 1.0,
                rng.next_float() * 2.0 - 1.0,
            ];
            let len_sq = v[0] * v[0] + v[1] * v[1] + v[2] * v[2];
            if len_sq > 0.01 {
                let len = len_sq.sqrt();
                return [v[0] / len, v[1] / len, v[2] / len];
            }
        }
    }

    #[test]
    fn regular_texgen_matrix_lowering_matches_the_reference_curve() {
        let mut rng = crate::particle::Rng::new(0x5EED);
        let mut max_error = 0.0f32;
        for _ in 0..20_000 {
            let unit_normal = random_unit_vector(&mut rng);
            let normal = [
                (unit_normal[0] * 127.0).round() as i8,
                (unit_normal[1] * 127.0).round() as i8,
                (unit_normal[2] * 127.0).round() as i8,
            ];
            let basis = random_unit_vector(&mut rng);
            let scale = 1 + (rng.next_float() * 4000.0) as u16;
            let dim = 1u32 << (1 + (rng.next_float() * 7.0) as u32); // 2..=256
                                                                     // `origin` is a quarter-texel offset *within* the tile (real
                                                                     // content never names an origin past the tile it addresses), so
                                                                     // it is bounded by the tile's own dimension, not independent of
                                                                     // it -- `dim * 4` quarter-texels is the full tile width/height.
            let origin = (rng.next_float() * dim as f32 * 4.0) as u16;
            let clamp = rng.next_float() < 0.5;

            let (reference, _) =
                regular_texgen_uv(normal, basis, [0.0; 3], scale, 1, origin, 0, clamp, false);
            let simulated = simulate_ge_lowering_s10_5(normal, basis, scale, origin, clamp, dim);

            let error = (reference as f32 - simulated).abs();
            if error > max_error {
                max_error = error;
            }
        }
        // Measured max ~1.78 S10.5 units (< 0.06 texels) across 20,000 random
        // unit normals/bases/scales/origins/dims -- not zero, but not the
        // `a`/`b` coefficient bug RE-228 fixed either (that produced errors
        // in the hundreds to thousands, `uncorrected_b_coefficient_
        // measurably_overcorrects` below). The residual comes from
        // `regular_texgen_curve`'s `.clamp(-1, 1)` of `dot`: an i8-quantized
        // normal is only ever *approximately* unit length (e.g. `[-106, 62,
        // -34]` has real magnitude ~127.4, not exactly 127), which can push
        // `dot` a hair past +-1. The reference formula clamps there (matching
        // the original hardware's own documented `dot = clamp(n*l, -1, 1)`);
        // the GE's real affine matrix multiply has no such clamp and simply
        // keeps going linearly. Accepted as a sub-texel PSP deviation, not
        // fixed -- clamping the GE's output would need an extra per-vertex
        // branch for a discrepancy below the padding `pad_edge_repeat`
        // already puts at every tile edge.
        assert!(
            max_error < 2.0,
            "measured max error {max_error} S10.5 units"
        );
    }

    /// The same comparison, but with `b` wrongly carrying the dot term's
    /// `NORMAL_SCALE_COMPENSATION` -- the formula this project shipped before
    /// RE-228. `a` is correct (it multiplies the normal-dependent dot
    /// product, which *is* read through the GE's measured `/128` divisor);
    /// `b` is the curve's zero-crossing constant and the origin shift, which
    /// are not, so compensating it the same way overcorrects. Kept as a
    /// regression record, not exercised by the real rendering path.
    #[test]
    fn uncorrected_b_coefficient_measurably_overcorrects() {
        let mut rng = crate::particle::Rng::new(0x5EED);
        let mut max_error = 0.0f32;
        for _ in 0..20_000 {
            let unit_normal = random_unit_vector(&mut rng);
            let normal = [
                (unit_normal[0] * 127.0).round() as i8,
                (unit_normal[1] * 127.0).round() as i8,
                (unit_normal[2] * 127.0).round() as i8,
            ];
            let basis = random_unit_vector(&mut rng);
            let scale = 1 + (rng.next_float() * 4000.0) as u16;
            let dim = 1u32 << (1 + (rng.next_float() * 7.0) as u32);
            let origin = (rng.next_float() * dim as f32 * 4.0) as u16;
            let clamp = rng.next_float() < 0.5;

            let (a, _) = regular_texgen_matrix_coeffs(scale, origin, clamp, dim);
            // The pre-RE-228 formula: `b = a + shift` instead of
            // `half_scale + shift`.
            let shift = if clamp {
                -(origin as f32 / 4.0) / dim.max(1) as f32
            } else {
                0.0
            };
            let uncorrected_b = a + shift;

            let (reference, _) =
                regular_texgen_uv(normal, basis, [0.0; 3], scale, 1, origin, 0, clamp, false);
            let simulated =
                simulate_ge_matrix_output(normal, basis, a, uncorrected_b) * dim as f32 * 32.0;

            let error = (reference as f32 - simulated).abs();
            if error > max_error {
                max_error = error;
            }
        }
        // This is the bug RE-228 fixed: a measurable, systematic error the
        // corrected version above does not have. Scale/127 vs scale/128
        // predicts up to roughly `scale / (127*128) * 32` S10.5 units.
        assert!(
            max_error > 1.0,
            "expected the uncorrected formula to measurably diverge; measured max error {max_error} S10.5 units"
        );
    }
}

/// Most mip levels the GE accepts.
pub const MAX_MIP_LEVELS: usize = 8;

/// Box-filters an image to half size, rounding dimensions up so a 1-wide image
/// stays 1 wide rather than vanishing.
fn halve(img: &Rgba8) -> Rgba8 {
    let w = (img.width / 2).max(1);
    let h = (img.height / 2).max(1);
    let mut out = Rgba8::new(w, h);
    for y in 0..h {
        for x in 0..w {
            // The source footprint, clamped for an odd or already-1 dimension.
            let (x0, y0) = (x * 2, y * 2);
            let x1 = (x0 + 1).min(img.width - 1);
            let y1 = (y0 + 1).min(img.height - 1);
            let mut acc = [0u32; 4];
            for (sx, sy) in [(x0, y0), (x1, y0), (x0, y1), (x1, y1)] {
                let at = ((sy * img.width + sx) * 4) as usize;
                for (c, a) in acc.iter_mut().enumerate() {
                    *a += img.pixels[at + c] as u32;
                }
            }
            out.put(
                (y * w + x) as usize,
                [
                    (acc[0] / 4) as u8,
                    (acc[1] / 4) as u8,
                    (acc[2] / 4) as u8,
                    (acc[3] / 4) as u8,
                ],
            );
        }
    }
    out
}

/// The palette entry closest to `rgba`, by squared distance over all four
/// channels.
///
/// Averaging four dithered texels lands between palette entries, and snapping
/// back to the nearest is what turns the dither into shading rather than into
/// a different dither: on a gradient ramp the nearest entry to a local average
/// *is* the shade that region represents.
fn nearest_entry(palette: &[u32], rgba: [u8; 4]) -> u8 {
    let mut best = (u32::MAX, 0usize);
    for (i, &e) in palette.iter().enumerate() {
        let c = [
            (e & 0xFF) as i32,
            ((e >> 8) & 0xFF) as i32,
            ((e >> 16) & 0xFF) as i32,
            ((e >> 24) & 0xFF) as i32,
        ];
        let d: u32 = (0..4)
            .map(|k| {
                let v = c[k] - rgba[k] as i32;
                (v * v) as u32
            })
            .sum();
        if d < best.0 {
            best = (d, i);
        }
    }
    best.1 as u8
}

/// Encodes one level into the GE's padded stride, unswizzled.
fn encode_level(img: &Rgba8, format: Psm, palette: &[u32]) -> (Vec<u8>, u32) {
    let stride = pad_to_power_of_two(img.width);
    let padded_h = pad_to_power_of_two(img.height);
    let stride_bytes = (stride as usize * format.bits()).div_ceil(8);
    let mut data = alloc::vec![0u8; stride_bytes * padded_h as usize];

    for y in 0..img.height as usize {
        for x in 0..img.width as usize {
            let s = (y * img.width as usize + x) * 4;
            let px = [
                img.pixels[s],
                img.pixels[s + 1],
                img.pixels[s + 2],
                img.pixels[s + 3],
            ];
            match format {
                Psm::PsmT4 => {
                    let i = nearest_entry(palette, px) & 0xF;
                    let at = y * stride_bytes + x / 2;
                    // High nibble first, matching the N64 order the
                    // straight-copy path relies on.
                    if x % 2 == 0 {
                        data[at] = (data[at] & 0x0F) | (i << 4);
                    } else {
                        data[at] = (data[at] & 0xF0) | i;
                    }
                }
                Psm::PsmT8 => data[y * stride_bytes + x] = nearest_entry(palette, px),
                Psm::Psm5551 => {
                    let v = pack_5551(px).to_le_bytes();
                    let at = y * stride_bytes + x * 2;
                    data[at..at + 2].copy_from_slice(&v);
                }
                _ => {
                    let at = y * stride_bytes + x * 4;
                    data[at..at + 4].copy_from_slice(&px);
                }
            }
        }
    }
    // `PLAN.md` R2.0/P0d: real hardware clamps to the logical edge, so a
    // bilinear sample near a clamped, non-power-of-two edge should read
    // real edge data rather than the zero fill above.
    if format == Psm::PsmT4 {
        pad_edge_repeat_nibbles(&mut data, stride, padded_h, img.width, img.height);
    } else {
        pad_edge_repeat(
            &mut data,
            stride,
            padded_h,
            img.width,
            img.height,
            format.bits() / 8,
        );
    }
    (data, stride)
}

/// Packs a texture with a full mip chain, every level generated from the
/// decoded image.
///
/// The N64's textures are frequently *dithered* — a CI4 gradient fakes a
/// smooth ramp out of sixteen colours — and the console resolves that with its
/// own filtering into shading. Sampled at around one texel per pixel on a sharp
/// display the dither instead aliases into moiré, which is what made Dream
/// Land's tree canopy read as green noise (RE-053).
///
/// Level 0 is regenerated from `rgba` rather than copied. For a paletted
/// texture that is lossless: `rgba` was decoded through this same palette, so
/// the nearest entry to each texel is the entry it came from.
///
/// Swizzling is all-or-nothing across the chain, because the GE's swizzle flag
/// is per texture and not per level.
pub fn pack_mipped(rgba: &Rgba8, format: Psm, palette: &[u32], swizzle_it: bool) -> PspTexture {
    let mut levels: Vec<(Vec<u8>, u32, u32)> = Vec::new(); // data, stride, padded height
    let mut img = rgba.clone();
    loop {
        let (data, stride) = encode_level(&img, format, palette);
        let padded_h = pad_to_power_of_two(img.height);
        let stride_bytes = data.len() / (padded_h as usize).max(1);
        // The GE's swizzle flag is per texture, not per level, so a chain
        // containing one level too small to swizzle would cost the whole
        // texture its swizzling. Small levels are also the ones that matter
        // least — the dither is resolved by the first level or two — so the
        // chain stops where swizzling would, rather than the other way round.
        if !levels.is_empty() && swizzle_it && !can_swizzle(stride_bytes, padded_h as usize) {
            break;
        }
        levels.push((data, stride, padded_h));
        if levels.len() == MAX_MIP_LEVELS || (img.width == 1 && img.height == 1) {
            break;
        }
        img = halve(&img);
    }

    let swizzled = swizzle_it
        && levels.iter().all(|(d, _, h)| {
            let sb = d.len() / (*h as usize).max(1);
            can_swizzle(sb, *h as usize)
        });

    let mut data = Vec::new();
    for (d, _, h) in &levels {
        let sb = d.len() / (*h as usize).max(1);
        if swizzled {
            data.extend_from_slice(&swizzle(d, sb, *h as usize));
        } else {
            data.extend_from_slice(d);
        }
    }

    PspTexture {
        width: rgba.width,
        height: rgba.height,
        stride: levels[0].1,
        format,
        data,
        swizzled,
        palette: palette.to_vec(),
        levels: levels.len() as u32,
    }
}
