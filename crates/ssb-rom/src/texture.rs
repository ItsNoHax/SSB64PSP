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

    fn put(&mut self, i: usize, rgba: [u8; 4]) {
        self.pixels[i * 4..i * 4 + 4].copy_from_slice(&rgba);
    }
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
