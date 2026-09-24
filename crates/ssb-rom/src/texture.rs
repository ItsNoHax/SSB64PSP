//! N64 texture formats.
//!
//! The RDP addresses a texture by a `(format, size)` pair rather than a single
//! enum. Smash stores images in these combinations; the decoders below turn
//! each into straight RGBA8888, which is the neutral form the PSP converter
//! then packs down (see `docs/rendering.md`).
//!
//! Paletted (`CI`) formats decode against a TLUT of RGBA5551 entries. The
//! RDP's `G_MDSFT_TEXTLUT` other-mode field, not the image format alone,
//! decides whether a texel goes through the TLUT at all ([`TextureLut`],
//! RE-313).

use alloc::vec::Vec;

/// RDP texel format (`G_IM_FMT_*`).
// Ord is derived so materials keying on a texture can be sorted, which is how
// draws get grouped to minimise GE state changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Format {
    Rgba = 0,
    Yuv = 1,
    Ci = 2,
    Ia = 3,
    I = 4,
}

impl Format {
    pub fn from_raw(v: u8) -> Option<Format> {
        Some(match v {
            0 => Format::Rgba,
            1 => Format::Yuv,
            2 => Format::Ci,
            3 => Format::Ia,
            4 => Format::I,
            _ => return None,
        })
    }
}

/// RDP texel size (`G_IM_SIZ_*`), in bits per texel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum BitSize {
    Bits4 = 0,
    Bits8 = 1,
    Bits16 = 2,
    Bits32 = 3,
}

impl BitSize {
    pub fn from_raw(v: u8) -> Option<BitSize> {
        Some(match v {
            0 => BitSize::Bits4,
            1 => BitSize::Bits8,
            2 => BitSize::Bits16,
            3 => BitSize::Bits32,
            _ => return None,
        })
    }

    pub fn bits(self) -> usize {
        match self {
            BitSize::Bits4 => 4,
            BitSize::Bits8 => 8,
            BitSize::Bits16 => 16,
            BitSize::Bits32 => 32,
        }
    }
}

/// The RDP's texture-LUT mode (`G_MDSFT_TEXTLUT`, other-mode-high bits 14-15).
///
/// Bit 15 is `en_tlut` and bit 14 `tlut_type` (`angrylion-rdp-plus`'s
/// `rdp_set_other_modes`); `gbi.h` names the three meaningful values
/// `G_TT_NONE` (0), `G_TT_RGBA16` (2) and `G_TT_IA16` (3). The value 1 sets
/// only the type bit, so the TLUT stays disabled.
///
/// With the TLUT enabled, the texel *format* no longer decides how a 4- or
/// 8-bit texel is interpreted: `fetch_texel_entlut_quadro` indexes the TLUT
/// with `palette << 4 | nibble` for every 4-bit format (CI, IA, I and RGBA
/// alike) and with the whole byte for every 8-bit one. A 16-bit texel indexes
/// with its high byte, and a 32-bit texel with its red byte. Without it, a CI
/// texel is not looked up: CI4 reads `palette << 4 | nibble` and CI8 the byte,
/// replicated into all four channels (`fetch_texel`'s `TEXEL_CI4`/`TEXEL_CI8`).
/// RE-313 has the derivation and the archive measurement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum TextureLut {
    #[default]
    None,
    Rgba16,
    Ia16,
}

impl TextureLut {
    /// Decodes the two-bit `G_MDSFT_TEXTLUT` field.
    pub fn from_field(field: u32) -> Self {
        match field & 0x3 {
            2 => TextureLut::Rgba16,
            3 => TextureLut::Ia16,
            _ => TextureLut::None,
        }
    }

    /// The other-mode-high field value this mode is written as.
    pub fn field(self) -> u32 {
        match self {
            TextureLut::None => 0,
            TextureLut::Rgba16 => 2,
            TextureLut::Ia16 => 3,
        }
    }

    /// Whether texels pass through the TLUT.
    pub fn enabled(self) -> bool {
        self != TextureLut::None
    }

    /// Expands one 16-bit TLUT entry the way this mode reads it. `None`
    /// never reads the TLUT and returns transparent black.
    pub fn entry_rgba(self, entry: u16) -> [u8; 4] {
        match self {
            TextureLut::None => [0; 4],
            TextureLut::Rgba16 => rgba5551(entry),
            TextureLut::Ia16 => {
                let i = (entry >> 8) as u8;
                [i, i, i, entry as u8]
            }
        }
    }
}

/// A decoded image: tightly packed RGBA8888, row-major, top-left origin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rgba8 {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

impl Rgba8 {
    pub fn new(width: u32, height: u32) -> Self {
        Rgba8 {
            width,
            height,
            pixels: alloc::vec![0; (width * height * 4) as usize],
        }
    }

    pub fn put(&mut self, i: usize, rgba: [u8; 4]) {
        self.pixels[i * 4..i * 4 + 4].copy_from_slice(&rgba);
    }

    pub fn get(&self, i: usize) -> [u8; 4] {
        self.pixels[i * 4..i * 4 + 4].try_into().unwrap()
    }
}

/// Pre-bakes `G_TX_MIRROR` into the pixel data, so a plain hardware wrap
/// mode reproduces real mirror addressing exactly without a programmable
/// texture stage: `sceGuTexScale` already renormalises UVs against whatever
/// width/height a texture actually reports, so a caller that just swaps in
/// this wider/taller image needs no other change.
///
/// `img` must already be exactly one repeat period on each mirrored axis
/// (`crates/ssb-rom/src/mesh.rs`'s `current_texture()` narrows a `TextureRef`
/// to `1 << mask` for this reason) -- mirroring anything else would bake in
/// whatever partial pattern happened to be visible, not the real period.
///
/// A mirrored axis with no clamp bit bakes exactly one mirrored pair (two
/// periods): a plain `Repeat` wrap over that doubled image already mirrors
/// forever, exactly, since wrapping back to the start resumes the same
/// (unflipped) phase the doubled image began with. A mirrored axis *with*
/// the clamp bit is different (`PLAN.md` R2.0/P0c, RE-220/RE-221): real
/// hardware keeps mirroring at every period boundary up to the tile's own
/// drawn-rect far edge before it clamps, not just through the first
/// mirrored pair, so this bakes every period the drawn rect spans --
/// `drawn_width`/`drawn_height`, `TextureRef`'s own fields -- instead of
/// always exactly two; a plain `sceGuTexWrap(Clamp)` on the result then
/// holds exactly the real far-edge texel forever, matching
/// `n64_addressing::address_axis`'s clamp target for every coordinate a
/// real drawn primitive can reach (see `n64_addressing::psp_lowering_axis`,
/// which models this same fold for direct comparison). An unmirrored axis
/// with the clamp bit needs the same `drawn`-wide bake whenever the mask
/// narrows the texture below the drawn rect (`PLAN.md` R2.1/T7a, RE-231):
/// real hardware keeps mask-wrapping every period up to the drawn rect's
/// far edge before it clamps there, not at the narrowed period's own last
/// texel, so an unmirrored axis with no such narrowing (`drawn == period`,
/// the overwhelmingly common case) bakes exactly `period` and this is a
/// no-op.
pub fn mirror_extend(
    img: &Rgba8,
    mirror_s: bool,
    mirror_t: bool,
    clamp_s: bool,
    clamp_t: bool,
    drawn_width: u32,
    drawn_height: u32,
) -> Rgba8 {
    let (w, h) = (img.width, img.height);
    let out_w = mirror_axis_len(w, mirror_s, clamp_s, drawn_width);
    let out_h = mirror_axis_len(h, mirror_t, clamp_t, drawn_height);
    if out_w == w && out_h == h {
        return img.clone();
    }
    let mut out = Rgba8::new(out_w, out_h);
    for y in 0..out_h {
        let sy = mirror_fold(y, h, mirror_t);
        for x in 0..out_w {
            let sx = mirror_fold(x, w, mirror_s);
            let px = img.get((sy * w + sx) as usize);
            out.put((y * out_w + x) as usize, px);
        }
    }
    out
}

/// One axis's baked output length -- see [`mirror_extend`]'s doc comment
/// for the cases this distinguishes. Clamp always bakes to `drawn` (a
/// no-op when the mask does not narrow the texture below the drawn rect,
/// since `drawn == period` then), mirror-without-clamp always bakes
/// exactly one mirrored pair, and neither bakes nothing.
pub fn mirror_axis_len(period: u32, mirror: bool, clamp: bool, drawn: u32) -> u32 {
    if clamp {
        drawn.max(1)
    } else if mirror {
        period * 2
    } else {
        period
    }
}

/// Which source texel (0..period) a baked output index `i` reads from: a
/// plain `% period` wrap for an unmirrored axis, otherwise the mask-period
/// mirror fold on top of it (unflipped on an even period index, reversed on
/// an odd one) -- `n64_addressing`'s `fold_period_mirror` transcribes the
/// same fold from the real hardware's bit-twiddled form for direct
/// comparison.
fn mirror_fold(i: u32, period: u32, mirror: bool) -> u32 {
    let phase = i % period;
    if mirror && (i / period) % 2 == 1 {
        period - 1 - phase
    } else {
        phase
    }
}

/// Softens a texture by averaging each texel with its 8 neighbours,
/// wrapping at the edges (the texture tiles, so the neighbour across a
/// border is the opposite edge, not a clamp).
///
/// RE-070: this is not general-purpose blurring, it is a named,
/// evidence-based exception. The N64 fakes extra shades on a small CI4
/// palette with ordered dithering, relying on the analog blur of a
/// composite-video CRT to read as a smooth gradient; the PSP's LCD has no
/// equivalent, and bilinear filtering alone measurably does not compensate
/// (a single dithered/non-dithered texel pair is one bilinear sample wide,
/// but the dither pattern repeats faster than that). Averaging texels
/// *before* palette quantization approximates the missing analog blur.
/// Naively requantizing the blurred result back to the same small palette
/// mostly undoes it (the blurred value usually snaps to one of the two
/// original entries again) -- callers of this function should pack the
/// result unquantized (e.g. `Psm8888`) instead, spending real VRAM for the
/// specific textures this is applied to, which is why this is opt-in per
/// texture rather than automatic for every paletted format.
pub fn box_blur_wrapped(img: &Rgba8) -> Rgba8 {
    let (w, h) = (img.width, img.height);
    let mut out = Rgba8::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let mut acc = [0u32; 4];
            for dy in [h - 1, 0, 1] {
                for dx in [w - 1, 0, 1] {
                    let sx = (x + dx) % w;
                    let sy = (y + dy) % h;
                    let px = img.get((sy * w + sx) as usize);
                    for (a, p) in acc.iter_mut().zip(px) {
                        *a += p as u32;
                    }
                }
            }
            out.put(
                (y * w + x) as usize,
                [
                    (acc[0] / 9) as u8,
                    (acc[1] / 9) as u8,
                    (acc[2] / 9) as u8,
                    (acc[3] / 9) as u8,
                ],
            );
        }
    }
    out
}

/// Applies a mild centre-weighted reconstruction filter, wrapping at texture
/// edges. Unlike [`box_blur_wrapped`], the source texel keeps three quarters
/// of the weight, so sharp authored details remain legible while single-texel
/// stair steps are softened.
pub fn mild_filter_wrapped(img: &Rgba8) -> Rgba8 {
    let (w, h) = (img.width, img.height);
    let mut out = Rgba8::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let mut acc = [0u32; 4];
            for dy in [h - 1, 0, 1] {
                for dx in [w - 1, 0, 1] {
                    let px = img.get((((y + dy) % h) * w + (x + dx) % w) as usize);
                    let weight = if dx == 0 && dy == 0 { 24 } else { 1 };
                    for (a, p) in acc.iter_mut().zip(px) {
                        *a += p as u32 * weight;
                    }
                }
            }
            out.put(
                (y * w + x) as usize,
                [
                    (acc[0] / 32) as u8,
                    (acc[1] / 32) as u8,
                    (acc[2] / 32) as u8,
                    (acc[3] / 32) as u8,
                ],
            );
        }
    }
    out
}

/// Expands an RGBA5551 texel. The single alpha bit becomes 0 or 255.
///
/// Channels are widened by bit replication (`c << 3 | c >> 2`) rather than a
/// plain shift, so full-scale input maps to full-scale output.
pub fn rgba5551(v: u16) -> [u8; 4] {
    let r = ((v >> 11) & 0x1F) as u8;
    let g = ((v >> 6) & 0x1F) as u8;
    let b = ((v >> 1) & 0x1F) as u8;
    let a = (v & 1) as u8;
    [
        (r << 3) | (r >> 2),
        (g << 3) | (g >> 2),
        (b << 3) | (b >> 2),
        if a != 0 { 255 } else { 0 },
    ]
}

/// Expands a 4-bit channel to 8 bits by nibble replication.
fn nib(v: u8) -> u8 {
    (v << 4) | v
}

/// Errors from texture decoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextureError {
    /// The source slice is too small for `width * height` texels.
    Truncated { need: usize, have: usize },
    /// A `(format, size)` combination the RDP does not define.
    UnsupportedCombination(Format, BitSize),
    /// A CI texture was decoded without a palette.
    MissingPalette,
}

/// Bytes required to hold `width * height` texels of the given size.
pub fn data_len(width: u32, height: u32, size: BitSize) -> usize {
    let texels = width as usize * height as usize;
    (texels * size.bits()).div_ceil(8)
}

/// Decodes an N64 texture into RGBA8888, as drawn with `G_TT_RGBA16`.
///
/// `tlut` is required for `Ci` formats and ignored otherwise. It is a slice of
/// big-endian RGBA5551 entries. This is [`decode_lut`] under the assumption
/// every CI texture is drawn with the RGBA16 TLUT enabled and every other
/// format with it disabled -- the common case, but a caller that knows the
/// real `G_MDSFT_TEXTLUT` state must use [`decode_lut`].
pub fn decode(
    data: &[u8],
    width: u32,
    height: u32,
    format: Format,
    size: BitSize,
    tlut: Option<&[u16]>,
) -> Result<Rgba8, TextureError> {
    if format == Format::Ci {
        let tlut = tlut.ok_or(TextureError::MissingPalette)?;
        decode_lut(
            data,
            width,
            height,
            format,
            size,
            TextureLut::Rgba16,
            tlut,
            0,
        )
    } else {
        decode_lut(data, width, height, format, size, TextureLut::None, &[], 0)
    }
}

/// The TLUT index a texel of this size would look up, when the TLUT is on.
///
/// `i` is the texel number. `palette` is the render tile's bank; it only
/// applies to 4-bit texels, as in `fetch_texel_entlut_quadro`.
fn lut_index(data: &[u8], i: usize, size: BitSize, palette: u8) -> usize {
    match size {
        BitSize::Bits4 => {
            let b = data[i / 2];
            let n = if i.is_multiple_of(2) { b >> 4 } else { b & 0xF };
            ((palette as usize & 0xF) << 4) | n as usize
        }
        BitSize::Bits8 => data[i] as usize,
        BitSize::Bits16 => data[i * 2] as usize,
        BitSize::Bits32 => data[i * 4] as usize,
    }
}

/// Decodes an N64 texture into RGBA8888 under an explicit TLUT mode.
///
/// `tlut` holds the whole loaded TLUT from index 0 (big-endian RGBA5551 or
/// IA16 words); `palette` is the render tile's CI4 bank. An index past the end
/// of `tlut` decodes as transparent black. The caller decides what that slot
/// really holds: RE-313 found the RDP loads exactly `count` entries and a
/// read past them returns stale TMEM, so a caller that knows the stale value
/// must extend `tlut` with it.
#[allow(clippy::too_many_arguments)]
pub fn decode_lut(
    data: &[u8],
    width: u32,
    height: u32,
    format: Format,
    size: BitSize,
    lut: TextureLut,
    tlut: &[u16],
    palette: u8,
) -> Result<Rgba8, TextureError> {
    let need = data_len(width, height, size);
    if data.len() < need {
        return Err(TextureError::Truncated {
            need,
            have: data.len(),
        });
    }
    if format == Format::Yuv {
        return Err(TextureError::UnsupportedCombination(format, size));
    }
    let count = (width * height) as usize;
    if lut.enabled() {
        let mut out = Rgba8::new(width, height);
        for i in 0..count {
            let entry = tlut.get(lut_index(data, i, size, palette)).copied();
            out.put(i, entry.map_or([0; 4], |e| lut.entry_rgba(e)));
        }
        return Ok(out);
    }
    if format == Format::Ci {
        // `fetch_texel`'s `TEXEL_CI4`/`TEXEL_CI8` without the TLUT: the index
        // itself (CI4 with the bank as its high nibble), in every channel.
        if !matches!(size, BitSize::Bits4 | BitSize::Bits8) {
            return Err(TextureError::UnsupportedCombination(format, size));
        }
        let mut out = Rgba8::new(width, height);
        for i in 0..count {
            let v = lut_index(data, i, size, palette) as u8;
            out.put(i, [v, v, v, v]);
        }
        return Ok(out);
    }
    decode_direct(data, width, height, format, size)
}

/// Decodes a non-CI texture with the TLUT disabled.
// The index does double duty here: it addresses the destination pixel *and*
// derives the packed source offset (`i / 2` for 4bpp, `i * 2` for 16bpp).
// Iterator adaptors would obscure that relationship rather than clarify it.
#[allow(clippy::needless_range_loop)]
fn decode_direct(
    data: &[u8],
    width: u32,
    height: u32,
    format: Format,
    size: BitSize,
) -> Result<Rgba8, TextureError> {
    let count = (width * height) as usize;
    let mut out = Rgba8::new(width, height);

    match (format, size) {
        (Format::Rgba, BitSize::Bits16) => {
            for i in 0..count {
                let v = u16::from_be_bytes([data[i * 2], data[i * 2 + 1]]);
                out.put(i, rgba5551(v));
            }
        }
        (Format::Rgba, BitSize::Bits32) => {
            for i in 0..count {
                let p = &data[i * 4..i * 4 + 4];
                out.put(i, [p[0], p[1], p[2], p[3]]);
            }
        }
        // IA16: 8-bit intensity, 8-bit alpha.
        (Format::Ia, BitSize::Bits16) => {
            for i in 0..count {
                let (v, a) = (data[i * 2], data[i * 2 + 1]);
                out.put(i, [v, v, v, a]);
            }
        }
        // IA8: 4-bit intensity, 4-bit alpha.
        (Format::Ia, BitSize::Bits8) => {
            for i in 0..count {
                let b = data[i];
                let v = nib(b >> 4);
                let a = nib(b & 0xF);
                out.put(i, [v, v, v, a]);
            }
        }
        // IA4: 3-bit intensity, 1-bit alpha.
        (Format::Ia, BitSize::Bits4) => {
            for i in 0..count {
                let b = data[i / 2];
                let n = if i % 2 == 0 { b >> 4 } else { b & 0xF };
                let iv = n >> 1;
                // Replicate 3 bits across 8: 0b_abc -> 0b_abcabcab
                let v = (iv << 5) | (iv << 2) | (iv >> 1);
                let a = if n & 1 != 0 { 255 } else { 0 };
                out.put(i, [v, v, v, a]);
            }
        }
        // I8 / I4: intensity drives all four channels, alpha included.
        (Format::I, BitSize::Bits8) => {
            for i in 0..count {
                let v = data[i];
                out.put(i, [v, v, v, v]);
            }
        }
        (Format::I, BitSize::Bits4) => {
            for i in 0..count {
                let b = data[i / 2];
                let v = nib(if i % 2 == 0 { b >> 4 } else { b & 0xF });
                out.put(i, [v, v, v, v]);
            }
        }
        (f, s) => return Err(TextureError::UnsupportedCombination(f, s)),
    }

    Ok(out)
}

/// Reads a TLUT from big-endian bytes.
pub fn parse_tlut(data: &[u8]) -> Vec<u16> {
    data.as_chunks::<2>()
        .0
        .iter()
        .map(|c| u16::from_be_bytes(*c))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rgba5551_replicates_bits_to_full_scale() {
        // All channels max, alpha set.
        assert_eq!(rgba5551(0xFFFF), [255, 255, 255, 255]);
        // All zero, alpha clear.
        assert_eq!(rgba5551(0x0000), [0, 0, 0, 0]);
        // Pure red, alpha set.
        assert_eq!(rgba5551(0xF801), [255, 0, 0, 255]);
    }

    /// A 2x1 image whose pixels are distinct enough to read positions back
    /// off the result unambiguously: (0,0)=A, (1,0)=B.
    fn ab_2x1() -> Rgba8 {
        let mut img = Rgba8::new(2, 1);
        img.put(0, [1, 0, 0, 255]);
        img.put(1, [2, 0, 0, 255]);
        img
    }

    #[test]
    fn mirror_extend_with_neither_axis_is_a_plain_copy() {
        let img = ab_2x1();
        let out = mirror_extend(&img, false, false, false, false, 0, 0);
        assert_eq!(out, img);
    }

    #[test]
    fn mirror_extend_s_only_flips_the_second_half_horizontally() {
        let out = mirror_extend(&ab_2x1(), true, false, false, false, 0, 0);
        assert_eq!((out.width, out.height), (4, 1));
        // A B | B A -- the second copy is the first one reversed, so the
        // pattern bounces smoothly across the seam at x=2 instead of
        // jumping straight back to A.
        assert_eq!(
            [out.get(0), out.get(1), out.get(2), out.get(3)],
            [
                [1, 0, 0, 255],
                [2, 0, 0, 255],
                [2, 0, 0, 255],
                [1, 0, 0, 255]
            ]
        );
    }

    #[test]
    fn mirror_extend_t_only_flips_the_second_half_vertically() {
        let mut img = Rgba8::new(1, 2);
        img.put(0, [1, 0, 0, 255]);
        img.put(1, [2, 0, 0, 255]);
        let out = mirror_extend(&img, false, true, false, false, 0, 0);
        assert_eq!((out.width, out.height), (1, 4));
        assert_eq!(
            [out.get(0), out.get(1), out.get(2), out.get(3)],
            [
                [1, 0, 0, 255],
                [2, 0, 0, 255],
                [2, 0, 0, 255],
                [1, 0, 0, 255]
            ]
        );
    }

    #[test]
    fn mirror_extend_both_axes_produces_all_four_orientations() {
        // A 2x2 source with a distinct pixel in every corner, so the four
        // quadrants of a both-axes mirror (identity, h-flip, v-flip,
        // h+v-flip) are each individually checkable.
        let mut img = Rgba8::new(2, 2);
        img.put(0, [1, 0, 0, 255]); // (0,0) top-left
        img.put(1, [2, 0, 0, 255]); // (1,0) top-right
        img.put(2, [3, 0, 0, 255]); // (0,1) bottom-left
        img.put(3, [4, 0, 0, 255]); // (1,1) bottom-right

        let out = mirror_extend(&img, true, true, false, false, 0, 0);
        assert_eq!((out.width, out.height), (4, 4));
        let px = |x: u32, y: u32| out.get((y * out.width + x) as usize);

        // Top-left quadrant: the source, unmirrored.
        assert_eq!(px(0, 0), [1, 0, 0, 255]);
        assert_eq!(px(1, 1), [4, 0, 0, 255]);
        // Top-right quadrant: horizontally mirrored (columns reversed).
        assert_eq!(px(2, 0), [2, 0, 0, 255]);
        assert_eq!(px(3, 0), [1, 0, 0, 255]);
        // Bottom-left quadrant: vertically mirrored (rows reversed).
        assert_eq!(px(0, 2), [3, 0, 0, 255]);
        assert_eq!(px(0, 3), [1, 0, 0, 255]);
        // Bottom-right quadrant: mirrored on both axes (180 degree turn).
        assert_eq!(px(2, 2), [4, 0, 0, 255]);
        assert_eq!(px(3, 3), [1, 0, 0, 255]);
    }

    /// `PLAN.md` R2.0/P0c: a mirror+clamp axis bakes every period the
    /// drawn rect spans, not just the first mirrored pair. A 2-texel
    /// period spanning a 7-texel drawn rect covers periods 0,1,2 in full
    /// plus one texel of period 3 -- unflipped, flipped, unflipped, then
    /// the start of another flipped period.
    #[test]
    fn mirror_extend_with_clamp_bakes_every_period_the_drawn_rect_spans() {
        let out = mirror_extend(&ab_2x1(), true, false, true, false, 7, 0);
        assert_eq!((out.width, out.height), (7, 1));
        let a = [1, 0, 0, 255];
        let b = [2, 0, 0, 255];
        assert_eq!(
            (0..7).map(|x| out.get(x)).collect::<alloc::vec::Vec<_>>(),
            [a, b, b, a, a, b, b],
            "AB|BA|AB|B: periods 0 and 2 unflipped, period 1 flipped, period 3 begins flipped"
        );
    }

    /// A drawn rect narrower than one mask period never reaches a second
    /// period, so the baked output is just the leading slice of the first
    /// (unflipped) period -- no mirroring visible at all.
    #[test]
    fn mirror_extend_with_clamp_and_drawn_rect_inside_first_period_is_unflipped() {
        let out = mirror_extend(&ab_2x1(), true, false, true, false, 1, 0);
        assert_eq!((out.width, out.height), (1, 1));
        assert_eq!(out.get(0), [1, 0, 0, 255]);
    }

    /// `PLAN.md` R2.1/T7a (RE-231): a clamp-without-mirror axis whose mask
    /// narrows the texture below the drawn rect must keep repeating every
    /// period up to the drawn rect's far edge before `sceGuTexWrap(Clamp)`
    /// takes over -- not clamp at the narrowed period's own last texel. A
    /// 2-texel period tiled plainly (no flip) across a 7-texel drawn rect is
    /// exactly the source pattern repeated three and a half times.
    #[test]
    fn mirror_extend_with_clamp_and_no_mirror_bakes_every_period_the_drawn_rect_spans() {
        let out = mirror_extend(&ab_2x1(), false, false, true, false, 7, 0);
        assert_eq!((out.width, out.height), (7, 1));
        let a = [1, 0, 0, 255];
        let b = [2, 0, 0, 255];
        assert_eq!(
            (0..7).map(|x| out.get(x)).collect::<alloc::vec::Vec<_>>(),
            [a, b, a, b, a, b, a],
            "AB repeated plainly, never flipped, up to the drawn rect's far edge"
        );
    }

    /// A clamp-without-mirror axis whose mask does *not* narrow the texture
    /// (`drawn == period`, the common case RE-066/RE-102 already cover) must
    /// stay a no-op bake, matching the pre-T7a behaviour exactly.
    #[test]
    fn mirror_extend_with_clamp_and_no_mirror_and_no_narrowing_is_a_plain_copy() {
        let img = ab_2x1();
        let out = mirror_extend(&img, false, false, true, false, 2, 0);
        assert_eq!(out, img);
    }

    #[test]
    fn box_blur_of_a_flat_image_is_unchanged() {
        let mut img = Rgba8::new(3, 3);
        for i in 0..9 {
            img.put(i, [42, 100, 200, 255]);
        }
        let out = box_blur_wrapped(&img);
        for i in 0..9 {
            assert_eq!(out.get(i), [42, 100, 200, 255]);
        }
    }

    #[test]
    fn mild_filter_preserves_flat_images_and_weights_the_center() {
        let mut flat = Rgba8::new(3, 3);
        for i in 0..9 {
            flat.put(i, [42, 100, 200, 255]);
        }
        assert_eq!(mild_filter_wrapped(&flat), flat);

        let mut impulse = Rgba8::new(3, 3);
        impulse.put(4, [160, 80, 40, 255]);
        let out = mild_filter_wrapped(&impulse);
        assert_eq!(out.get(4), [120, 60, 30, 191]);
        assert_eq!(out.get(1), [5, 2, 1, 7]);
        assert_eq!(out.get(0), [5, 2, 1, 7]);
    }

    #[test]
    fn box_blur_averages_a_checkerboard_toward_the_midpoint() {
        // A 2x2 checker tiled across a 4x4 image, so each texel's 8
        // wrapped neighbours are 4 distinct cells of its own colour
        // (diagonals) and 4 distinct cells of the other (orthogonal) --
        // not the degenerate double-sampling a 2x2 image would give.
        let mut img = Rgba8::new(4, 4);
        for y in 0..4u32 {
            for x in 0..4u32 {
                let on = (x % 2) ^ (y % 2) == 0;
                img.put(
                    (y * 4 + x) as usize,
                    if on { [0, 0, 0, 255] } else { [200; 4] },
                );
            }
        }
        let out = box_blur_wrapped(&img);
        // Every texel: itself + 4 diagonal same-colour + 4 orthogonal
        // opposite-colour, over 9 samples.
        let dark = 0u32;
        let light = 200u32;
        let on_avg = (dark * 5 + light * 4) / 9;
        let off_avg = (light * 5 + dark * 4) / 9;
        assert_eq!(out.get(0)[0], on_avg as u8, "(0,0) starts 'on'");
        assert_eq!(out.get(1)[0], off_avg as u8, "(1,0) starts 'off'");
    }

    #[test]
    fn box_blur_wraps_rather_than_darkening_the_edges() {
        // A single bright texel in an otherwise-black tiling image. Wrapping
        // means every texel (including the far corners) is within one
        // step of it through *some* edge, so no position is treated
        // differently just for being near a border.
        let mut img = Rgba8::new(4, 4);
        for i in 0..16 {
            img.put(i, [0, 0, 0, 255]);
        }
        img.put(0, [90, 0, 0, 255]); // corner (0,0)
        let out = box_blur_wrapped(&img);
        // (3,3) wraps to be diagonally adjacent to (0,0) through the corner.
        let corner_neighbour = out.get(3 * 4 + 3)[0];
        assert!(
            corner_neighbour > 0,
            "wrapping must reach across the border"
        );
    }

    #[test]
    fn decodes_i4_two_texels_per_byte() {
        let img = decode(&[0xF0], 2, 1, Format::I, BitSize::Bits4, None).unwrap();
        assert_eq!(img.pixels, [255, 255, 255, 255, 0, 0, 0, 0]);
    }

    #[test]
    fn decodes_ia8_split_nibbles() {
        // intensity 0xF, alpha 0x0
        let img = decode(&[0xF0], 1, 1, Format::Ia, BitSize::Bits8, None).unwrap();
        assert_eq!(img.pixels, [255, 255, 255, 0]);
    }

    #[test]
    fn decodes_ci4_through_palette() {
        let tlut = [0x0000u16, 0xF801];
        let img = decode(&[0x01], 2, 1, Format::Ci, BitSize::Bits4, Some(&tlut)).unwrap();
        assert_eq!(img.pixels, [0, 0, 0, 0, 255, 0, 0, 255]);
    }

    #[test]
    fn ci_without_palette_is_an_error() {
        assert_eq!(
            decode(&[0x01], 2, 1, Format::Ci, BitSize::Bits4, None),
            Err(TextureError::MissingPalette)
        );
    }

    #[test]
    fn rejects_truncated_input() {
        assert!(matches!(
            decode(&[0x00], 8, 8, Format::Rgba, BitSize::Bits16, None),
            Err(TextureError::Truncated { .. })
        ));
    }

    #[test]
    fn data_len_rounds_4bpp_up() {
        assert_eq!(data_len(3, 1, BitSize::Bits4), 2);
        assert_eq!(data_len(4, 1, BitSize::Bits4), 2);
        assert_eq!(data_len(4, 1, BitSize::Bits16), 8);
    }

    /// RE-313: with the TLUT off, I4/I8 are intensity in every channel; with
    /// it on, the same bytes are TLUT indices (`fetch_texel_entlut_quadro`),
    /// with the CI4 bank as the high nibble of a 4-bit index only.
    #[test]
    fn intensity_texels_read_the_tlut_only_when_it_is_enabled() {
        let tlut: Vec<u16> = (0..32).map(|i| 0xF801 ^ (i << 1)).collect();
        let i4 = [0x1F];
        let off = decode_lut(
            &i4,
            2,
            1,
            Format::I,
            BitSize::Bits4,
            TextureLut::None,
            &tlut,
            0,
        )
        .unwrap();
        assert_eq!(off.get(0), [0x11; 4]);
        assert_eq!(off.get(1), [0xFF; 4]);

        let on = decode_lut(
            &i4,
            2,
            1,
            Format::I,
            BitSize::Bits4,
            TextureLut::Rgba16,
            &tlut,
            0,
        )
        .unwrap();
        assert_eq!(on.get(0), rgba5551(tlut[1]));
        assert_eq!(on.get(1), rgba5551(tlut[15]));
        let bank = decode_lut(
            &i4,
            2,
            1,
            Format::I,
            BitSize::Bits4,
            TextureLut::Rgba16,
            &tlut,
            1,
        )
        .unwrap();
        assert_eq!(
            bank.get(0),
            rgba5551(tlut[17]),
            "4-bit index = bank << 4 | nibble"
        );

        let i8 = [0x1E];
        let on8 = decode_lut(
            &i8,
            1,
            1,
            Format::I,
            BitSize::Bits8,
            TextureLut::Rgba16,
            &tlut,
            1,
        )
        .unwrap();
        assert_eq!(
            on8.get(0),
            rgba5551(tlut[0x1E]),
            "an 8-bit index ignores the bank"
        );
        let off8 = decode_lut(
            &i8,
            1,
            1,
            Format::I,
            BitSize::Bits8,
            TextureLut::None,
            &tlut,
            0,
        )
        .unwrap();
        assert_eq!(off8.get(0), [0x1E; 4]);
    }

    /// RE-313: `G_TT_IA16` reads an entry as intensity (high byte) and alpha
    /// (low byte); `G_TT_RGBA16` as RGBA5551 with a one-bit alpha.
    #[test]
    fn tlut_type_decides_how_an_entry_expands() {
        assert_eq!(
            TextureLut::Ia16.entry_rgba(0x80C0),
            [0x80, 0x80, 0x80, 0xC0]
        );
        assert_eq!(TextureLut::Rgba16.entry_rgba(0xFFFE), [255, 255, 255, 0]);
        assert_eq!(TextureLut::Rgba16.entry_rgba(0x0001), [0, 0, 0, 255]);
        assert_eq!(TextureLut::from_field(0), TextureLut::None);
        assert_eq!(TextureLut::from_field(1), TextureLut::None);
        assert_eq!(TextureLut::from_field(2), TextureLut::Rgba16);
        assert_eq!(TextureLut::from_field(3), TextureLut::Ia16);
        let ci = decode_lut(
            &[0x01],
            2,
            1,
            Format::Ci,
            BitSize::Bits4,
            TextureLut::Ia16,
            &[0x10FF, 0x2080],
            0,
        )
        .unwrap();
        assert_eq!(ci.get(0), [0x10, 0x10, 0x10, 0xFF]);
        assert_eq!(ci.get(1), [0x20, 0x20, 0x20, 0x80]);
    }

    /// RE-313: a CI texel drawn with the TLUT off is its raw index (CI4 with
    /// the bank as high nibble) in all four channels, alpha included.
    #[test]
    fn ci_texels_with_the_tlut_off_read_the_raw_index() {
        let ci4 = decode_lut(
            &[0x3C],
            2,
            1,
            Format::Ci,
            BitSize::Bits4,
            TextureLut::None,
            &[],
            2,
        )
        .unwrap();
        assert_eq!(ci4.get(0), [0x23; 4]);
        assert_eq!(ci4.get(1), [0x2C; 4]);
        let ci8 = decode_lut(
            &[0x9A],
            1,
            1,
            Format::Ci,
            BitSize::Bits8,
            TextureLut::None,
            &[],
            2,
        )
        .unwrap();
        assert_eq!(ci8.get(0), [0x9A; 4]);
    }

    /// RE-313: `decode_lut` reads an index past the supplied TLUT as
    /// transparent black; the caller extends the TLUT with any measured
    /// stale TMEM value first. `decode` keeps its RGBA16 reading of CI.
    #[test]
    fn an_index_past_the_supplied_tlut_is_transparent_black() {
        let short = [0x294A, 0xA50D, 0x0001];
        let img = decode_lut(
            &[0x03],
            2,
            1,
            Format::Ci,
            BitSize::Bits4,
            TextureLut::Rgba16,
            &short,
            0,
        )
        .unwrap();
        assert_eq!(img.get(0), rgba5551(0x294A));
        assert_eq!(img.get(1), [0, 0, 0, 0]);
        let mut extended = short.to_vec();
        extended.push(0x1091);
        let img = decode_lut(
            &[0x03],
            2,
            1,
            Format::Ci,
            BitSize::Bits4,
            TextureLut::Rgba16,
            &extended,
            0,
        )
        .unwrap();
        assert_eq!(img.get(1), rgba5551(0x1091));
        assert_eq!(
            decode(&[0x03], 2, 1, Format::Ci, BitSize::Bits4, Some(&extended)).unwrap(),
            img
        );
        assert_eq!(
            decode(&[0x03], 2, 1, Format::Ci, BitSize::Bits4, None),
            Err(TextureError::MissingPalette)
        );
    }
}
