//! N64 texture formats.
//!
//! The RDP addresses a texture by a `(format, size)` pair rather than a single
//! enum. Smash stores images in these combinations; the decoders below turn
//! each into straight RGBA8888, which is the neutral form the PSP converter
//! then packs down (see `docs/rendering.md`).
//!
//! Paletted (`CI`) formats decode against a TLUT of RGBA5551 entries.

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

/// Pre-bakes `G_TX_MIRROR` into the pixel data by doubling the affected
/// axes, so a plain hardware `Repeat` wrap reproduces a real mirror-repeat
/// exactly (RE-067): `sceGuTexScale` already renormalises UVs against
/// whatever width/height a texture actually reports, so a caller that just
/// swaps in this wider/taller image needs no other change.
///
/// `img` must already be exactly one repeat period on each mirrored axis
/// (`crates/ssb-rom/src/mesh.rs`'s `current_texture()` narrows a `TextureRef`
/// to `1 << mask` for this reason) -- mirroring anything else would bake in
/// whatever partial pattern happened to be visible, not the real period.
pub fn mirror_extend(img: &Rgba8, mirror_s: bool, mirror_t: bool) -> Rgba8 {
    if !mirror_s && !mirror_t {
        return img.clone();
    }
    let (w, h) = (img.width, img.height);
    let out_w = if mirror_s { w * 2 } else { w };
    let out_h = if mirror_t { h * 2 } else { h };
    let mut out = Rgba8::new(out_w, out_h);
    for y in 0..out_h {
        let sy = if y < h { y } else { 2 * h - 1 - y };
        for x in 0..out_w {
            let sx = if x < w { x } else { 2 * w - 1 - x };
            let px = img.get((sy * w + sx) as usize);
            out.put((y * out_w + x) as usize, px);
        }
    }
    out
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

/// Decodes an N64 texture into RGBA8888.
///
/// `tlut` is required for `Ci` formats and ignored otherwise. It is a slice of
/// big-endian RGBA5551 entries.
// The index does double duty here: it addresses the destination pixel *and*
// derives the packed source offset (`i / 2` for 4bpp, `i * 2` for 16bpp).
// Iterator adaptors would obscure that relationship rather than clarify it.
#[allow(clippy::needless_range_loop)]
pub fn decode(
    data: &[u8],
    width: u32,
    height: u32,
    format: Format,
    size: BitSize,
    tlut: Option<&[u16]>,
) -> Result<Rgba8, TextureError> {
    let need = data_len(width, height, size);
    if data.len() < need {
        return Err(TextureError::Truncated {
            need,
            have: data.len(),
        });
    }

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
        (Format::Ci, BitSize::Bits8) => {
            let tlut = tlut.ok_or(TextureError::MissingPalette)?;
            for i in 0..count {
                let idx = data[i] as usize;
                out.put(i, rgba5551(tlut.get(idx).copied().unwrap_or(0)));
            }
        }
        (Format::Ci, BitSize::Bits4) => {
            let tlut = tlut.ok_or(TextureError::MissingPalette)?;
            for i in 0..count {
                let b = data[i / 2];
                let idx = (if i % 2 == 0 { b >> 4 } else { b & 0xF }) as usize;
                out.put(i, rgba5551(tlut.get(idx).copied().unwrap_or(0)));
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
        let out = mirror_extend(&img, false, false);
        assert_eq!(out, img);
    }

    #[test]
    fn mirror_extend_s_only_flips_the_second_half_horizontally() {
        let out = mirror_extend(&ab_2x1(), true, false);
        assert_eq!((out.width, out.height), (4, 1));
        // A B | B A -- the second copy is the first one reversed, so the
        // pattern bounces smoothly across the seam at x=2 instead of
        // jumping straight back to A.
        assert_eq!(
            [out.get(0), out.get(1), out.get(2), out.get(3)],
            [[1, 0, 0, 255], [2, 0, 0, 255], [2, 0, 0, 255], [1, 0, 0, 255]]
        );
    }

    #[test]
    fn mirror_extend_t_only_flips_the_second_half_vertically() {
        let mut img = Rgba8::new(1, 2);
        img.put(0, [1, 0, 0, 255]);
        img.put(1, [2, 0, 0, 255]);
        let out = mirror_extend(&img, false, true);
        assert_eq!((out.width, out.height), (1, 4));
        assert_eq!(
            [out.get(0), out.get(1), out.get(2), out.get(3)],
            [[1, 0, 0, 255], [2, 0, 0, 255], [2, 0, 0, 255], [1, 0, 0, 255]]
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

        let out = mirror_extend(&img, true, true);
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
    fn box_blur_averages_a_checkerboard_toward_the_midpoint() {
        // A 2x2 checker tiled across a 4x4 image, so each texel's 8
        // wrapped neighbours are 4 distinct cells of its own colour
        // (diagonals) and 4 distinct cells of the other (orthogonal) --
        // not the degenerate double-sampling a 2x2 image would give.
        let mut img = Rgba8::new(4, 4);
        for y in 0..4u32 {
            for x in 0..4u32 {
                let on = (x % 2) ^ (y % 2) == 0;
                img.put((y * 4 + x) as usize, if on { [0, 0, 0, 255] } else { [200; 4] });
            }
        }
        let out = box_blur_wrapped(&img);
        // Every texel: itself + 4 diagonal same-colour + 4 orthogonal
        // opposite-colour, over 9 samples.
        let on_avg = (0 * 5 + 200 * 4) / 9;
        let off_avg = (200 * 5 + 0 * 4) / 9;
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
        assert!(corner_neighbour > 0, "wrapping must reach across the border");
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
}
