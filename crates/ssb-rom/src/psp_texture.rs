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
