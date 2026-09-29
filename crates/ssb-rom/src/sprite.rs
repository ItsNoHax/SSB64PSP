//! libultra `Sprite`s: the 2D images an `SObj` draws (`PR/sp.h`,
//! `lbCommonDrawSObjBitmap`).
//!
//! A `Sprite` is 68 bytes: its size, scale, attribute flags, a primitive
//! colour, an optional TLUT and a list of `Bitmap` strips. Each 16-byte
//! `Bitmap` is one horizontal band of the image, `bmheight` rows apart,
//! `actualHeight` rows tall, `width_img` texels per stored row. The strips
//! are loaded with `gDPLoadBlock` and no line step, so the ROM stores every
//! odd row already interleaved the way TMEM wants it: the two 32-bit halves
//! of each 64-bit word swapped for 4-, 8- and 16-bit texels, and, when
//! `SP_TEXSHUF` is set, the two 64-bit halves of each 128-bit unit for
//! 32-bit texels (`tools/relocSpriteTool.py`'s `unswizzle_n64_texture` and
//! `deshuffle_texshuf_rows`).

use alloc::vec::Vec;

use crate::archive::File;
use crate::texture::{self, BitSize, Format, Rgba8};

/// `sizeof(Sprite)`.
pub const SPRITE_SIZE: usize = 68;
/// `sizeof(Bitmap)`.
pub const BITMAP_SIZE: usize = 16;

/// `SP_TRANSPARENT`: drawn with `G_RM_XLU_SURF`.
pub const SP_TRANSPARENT: u16 = 0x0001;
/// `SP_TEXSHUF`: 32-bit strips are stored shuffled.
pub const SP_TEXSHUF: u16 = 0x0200;

/// A decoded sprite.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sprite {
    pub width: u16,
    pub height: u16,
    pub attr: u16,
    /// `red`, `green`, `blue`, `alpha`: the primitive colour.
    pub color: [u8; 4],
    pub format: Format,
    pub size: BitSize,
    /// The image, `width` by `height`, texels decoded without any combiner.
    pub image: Rgba8,
}

/// Why a sprite did not decode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpriteError {
    Truncated,
    /// A pointer field holds no relocation.
    MissingPointer(u32),
    BadFormat(u8, u8),
    Texture(texture::TextureError),
}

fn be16(d: &[u8], at: usize) -> Result<i16, SpriteError> {
    d.get(at..at + 2)
        .map(|b| i16::from_be_bytes([b[0], b[1]]))
        .ok_or(SpriteError::Truncated)
}

/// The in-file target of the pointer slot at `at`.
fn pointer(file: &File, at: u32) -> Result<u32, SpriteError> {
    file.intern_relocs
        .iter()
        .find(|r| r.at == at)
        .map(|r| r.target)
        .ok_or(SpriteError::MissingPointer(at))
}

/// Undoes the odd-row interleave of one strip, `row_bytes` per row.
fn deinterleave(data: &mut [u8], row_bytes: usize, rows: usize, unit: usize) {
    let half = unit / 2;
    for row in (1..rows).step_by(2) {
        let start = row * row_bytes;
        let end = (start + row_bytes).min(data.len());
        let mut i = start;
        while i + unit <= end {
            let (a, b) = data[i..i + unit].split_at_mut(half);
            a.swap_with_slice(b);
            i += unit;
        }
    }
}

/// Decodes the `Sprite` at byte offset `at` of `file`.
pub fn decode(file: &File, at: u32) -> Result<Sprite, SpriteError> {
    decode_with(file, at, None)
}

/// [`decode`] with the TLUT replaced, as `ifCommonPlayerStockSetLUT` swaps
/// a stock icon's `LUT` for the costume's.
pub fn decode_with_tlut(file: &File, at: u32, tlut: &[u16]) -> Result<Sprite, SpriteError> {
    decode_with(file, at, Some(tlut))
}

fn decode_with(file: &File, at: u32, tlut_override: Option<&[u16]>) -> Result<Sprite, SpriteError> {
    let d = &file.data;
    let base = at as usize;
    if base + SPRITE_SIZE > d.len() {
        return Err(SpriteError::Truncated);
    }
    let width = be16(d, base + 4)? as u16;
    let height = be16(d, base + 6)? as u16;
    let attr = be16(d, base + 20)? as u16;
    let color = [d[base + 24], d[base + 25], d[base + 26], d[base + 27]];
    let nbitmaps = be16(d, base + 40)?.max(0) as usize;
    let bmheight = be16(d, base + 44)?.max(0) as usize;
    let (fmt, siz) = (d[base + 48], d[base + 49]);
    let format = Format::from_raw(fmt).ok_or(SpriteError::BadFormat(fmt, siz))?;
    let size = BitSize::from_raw(siz).ok_or(SpriteError::BadFormat(fmt, siz))?;
    let tlut = if let Some(t) = tlut_override {
        Some(t.to_vec())
    } else if format == Format::Ci {
        let lut = pointer(file, at + 32)? as usize;
        let len = if size == BitSize::Bits4 { 32 } else { 512 };
        Some(texture::parse_tlut(
            d.get(lut..lut + len).ok_or(SpriteError::Truncated)?,
        ))
    } else {
        None
    };
    let bitmaps = pointer(file, at + 52)?;

    let mut image = Rgba8::new(u32::from(width), u32::from(height));
    for strip in 0..nbitmaps {
        let bm = bitmaps as usize + strip * BITMAP_SIZE;
        let bm_width = be16(d, bm)?.max(0) as usize;
        let width_img = be16(d, bm + 2)?.max(0) as usize;
        let rows = be16(d, bm + 12)?.max(0) as usize;
        let buf = pointer(file, (bm + 8) as u32)? as usize;
        let row_bytes = (width_img * size.bits()).div_ceil(8);
        let mut texels = d
            .get(buf..buf + row_bytes * rows)
            .ok_or(SpriteError::Truncated)?
            .to_vec();
        match size {
            BitSize::Bits32 if attr & SP_TEXSHUF != 0 => {
                deinterleave(&mut texels, row_bytes, rows, 16)
            }
            BitSize::Bits32 => {}
            _ => deinterleave(&mut texels, row_bytes, rows, 8),
        }
        let strip_image = texture::decode(
            &texels,
            width_img as u32,
            rows as u32,
            format,
            size,
            tlut.as_deref(),
        )
        .map_err(SpriteError::Texture)?;
        let top = strip * bmheight;
        for y in 0..rows {
            let dy = top + y;
            if dy >= usize::from(height) {
                break;
            }
            for x in 0..bm_width.min(usize::from(width)).min(width_img) {
                let px = strip_image.get(y * width_img + x);
                image.put(dy * usize::from(width) + x, px);
            }
        }
    }
    Ok(Sprite {
        width,
        height,
        attr,
        color,
        format,
        size,
        image,
    })
}

/// The image the PSP draws with a `Modulate` texture function against the
/// primitive colour, following `lbCommonPrepSObjAttr`'s combiner per
/// format:
/// - I: the primitive's colour, the texel's intensity as alpha;
/// - IA: `PRIMITIVE * TEXEL0` in colour and alpha, with the environment
///   black;
/// - RGBA and CI: `G_CC_DECALRGBA`, the texel as it is (drawn white).
pub fn combined_image(s: &Sprite) -> Rgba8 {
    let mut out = s.image.clone();
    if s.format == Format::I {
        for px in out.pixels.as_chunks_mut::<4>().0 {
            *px = [0xFF, 0xFF, 0xFF, px[0]];
        }
    }
    out
}

/// Whether the draw is tinted by the primitive colour.
pub fn uses_prim_color(format: Format) -> bool {
    matches!(format, Format::I | Format::Ia)
}

/// The `Sprite` offsets `lbRelocGetFileData` reads in one file, from
/// `reloc_data.us.h`.
pub struct SpriteFile {
    pub file: u32,
    pub offsets: &'static [u32],
}

/// File 164, `IFCommonPlayerDamage`: `llIFCommonPlayerDamageDigit0Sprite`
/// to `...Digit9Sprite`, then `...SymbolPercentSprite` and
/// `...SymbolHPSprite` (`dIFCommonPlayerDamageDigitSpriteOffsets`).
pub const PLAYER_DAMAGE: SpriteFile = SpriteFile {
    file: 164,
    offsets: &[
        0x148, 0x2D8, 0x500, 0x698, 0x8C0, 0xA58, 0xC80, 0xE18, 0x1040, 0x1270, 0x1458, 0x15D8,
    ],
};

/// `FTAttributes.sprites`, counted on from `translate_scales` at 0x324
/// through `modelparts_container`, `accesspart`, `textureparts_container`,
/// `joint_itemheavy_id`, `thrown_status` and `joint_itemlight_id`.
pub const ATTR_SPRITES_OFFSET: u32 = 0x340;

/// A place in the archive: file and byte offset.
pub type Place = (u32, u32);

/// `FTSprites`: a fighter's stock icon, its per-costume palettes and its
/// series emblem, as places in the archive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FighterSprites {
    pub stock: Place,
    /// `stock_luts[costume]`.
    pub stock_luts: Vec<Place>,
    pub emblem: Place,
}

/// Where the pointer slot at `at` of `file` leads, in this file or another.
pub fn pointer_place(file: &File, at: u32) -> Option<Place> {
    if let Some(r) = file.intern_relocs.iter().find(|r| r.at == at) {
        return Some((file.id, r.target));
    }
    file.extern_relocs
        .iter()
        .find(|r| r.at == at)
        .map(|r| (u32::from(r.target_file), r.target_offset))
}

/// Follows `FTAttributes.sprites` in a fighter's main file. `stock_luts`
/// ends at the first slot without a relocation (at most 8, `nFTCostume`'s
/// room).
pub fn fighter_sprites(main: &File, attributes: u32) -> Option<FighterSprites> {
    let (file, sprites) = pointer_place(main, attributes + ATTR_SPRITES_OFFSET)?;
    if file != main.id {
        return None;
    }
    let stock = pointer_place(main, sprites)?;
    let emblem = pointer_place(main, sprites + 8)?;
    let (lut_file, luts) = pointer_place(main, sprites + 4)?;
    let stock_luts = if lut_file == main.id {
        (0..8)
            .map_while(|i| pointer_place(main, luts + i * 4))
            .collect()
    } else {
        Vec::new()
    };
    Some(FighterSprites {
        stock,
        stock_luts,
        emblem,
    })
}

/// File 82, `IFCommonGameStatus` (`gGMCommonFiles[1]`), in its sprite
/// manifest's order: `OrangeLetterG`, `...O`, `OrangeExclamationMark`, the
/// blue letters `T I M E U P S A G`, `Rod`, `Frame`, `RodShadow`, then the
/// lamps `{Red,Yellow,Blue}Dim`, `...Light` and `...Contour`.
pub const GAME_STATUS: SpriteFile = SpriteFile {
    file: 82,
    offsets: &[
        0x4D78, 0xA730, 0xC370, 0xE4A8, 0xF740, 0x127E0, 0x144E0, 0x16EB8, 0x18FE8, 0x1B5F8,
        0x1DE68, 0x20788, 0x20990, 0x21760, 0x21878, 0x21950, 0x21A10, 0x21BA8, 0x22128, 0x22588,
        0x22F18, 0x23A28, 0x24620, 0x25290,
    ],
};

/// `SP_CLOUD`: drawn with `G_RM_CLD_SURF`, blended like `SP_TRANSPARENT`.
pub const SP_CLOUD: u16 = 0x1000;

/// Every sprite file the pack converts.
pub const FILES: &[SpriteFile] = &[PLAYER_DAMAGE, GAME_STATUS];

/// Decodes every sprite of `f`.
pub fn decode_all(file: &File, f: &SpriteFile) -> Result<Vec<Sprite>, SpriteError> {
    f.offsets.iter().map(|&at| decode(file, at)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn odd_rows_swap_their_halves() {
        let mut d: Vec<u8> = (0..32).collect();
        deinterleave(&mut d, 16, 2, 8);
        assert_eq!(&d[..16], &(0..16).collect::<Vec<u8>>()[..]);
        assert_eq!(
            &d[16..],
            &[20, 21, 22, 23, 16, 17, 18, 19, 28, 29, 30, 31, 24, 25, 26, 27]
        );
    }

    #[test]
    fn every_fighter_has_an_emblem_and_stock_icons() {
        let Some(path) = std::env::var_os("SSB64_ROM") else {
            return;
        };
        let data = std::fs::read(path).unwrap();
        let info = crate::rom::identify(&data).unwrap();
        let archive = crate::archive::Archive::open(&data, info.region).unwrap();
        for e in crate::fighter::FIGHTER_FILES.iter().take(12) {
            let main = archive.load(e.file).unwrap();
            let s = fighter_sprites(&main, e.offset).unwrap();
            let emblem = decode(&archive.load(s.emblem.0).unwrap(), s.emblem.1).unwrap();
            assert_eq!(
                (emblem.format, emblem.size),
                (Format::I, BitSize::Bits4),
                "{}",
                e.name
            );
            let model = archive.load(s.stock.0).unwrap();
            let stock = decode(&model, s.stock.1).unwrap();
            assert_eq!((stock.width, stock.height), (8, 10), "{}", e.name);
            assert_eq!((stock.format, stock.size), (Format::Ci, BitSize::Bits4));
            assert!((7..=8).contains(&s.stock_luts.len()), "{}", e.name);
        }
        // Mario's, named in `reloc_data.us.h`.
        let mario = archive.load(203).unwrap();
        let s = fighter_sprites(&mario, crate::fighter::FIGHTER_FILES[0].offset).unwrap();
        assert_eq!(s.stock, (296, 0x72D0));
        assert_eq!(s.emblem, (296, 0x74C8));
    }

    #[test]
    fn the_game_status_sprites_decode() {
        let Some(path) = std::env::var_os("SSB64_ROM") else {
            return;
        };
        let data = std::fs::read(path).unwrap();
        let info = crate::rom::identify(&data).unwrap();
        let archive = crate::archive::Archive::open(&data, info.region).unwrap();
        let file = archive.load(GAME_STATUS.file).unwrap();
        let sprites = decode_all(&file, &GAME_STATUS).unwrap();
        for (i, s) in sprites.iter().enumerate() {
            let rgba32 = (s.format, s.size) == (Format::Rgba, BitSize::Bits32);
            // The letters are RGBA32; the rod, frame, shadow IA8; the lamps I.
            match i {
                0..=11 => assert!(rgba32, "sprite {i}"),
                12..=14 => assert_eq!((s.format, s.size), (Format::Ia, BitSize::Bits8)),
                _ => assert_eq!(s.format, Format::I),
            }
        }
    }

    #[test]
    fn the_damage_digits_decode() {
        let Some(path) = std::env::var_os("SSB64_ROM") else {
            return;
        };
        let data = std::fs::read(path).unwrap();
        let info = crate::rom::identify(&data).unwrap();
        let archive = crate::archive::Archive::open(&data, info.region).unwrap();
        let file = archive.load(PLAYER_DAMAGE.file).unwrap();
        let sprites = decode_all(&file, &PLAYER_DAMAGE).unwrap();
        assert_eq!(sprites.len(), 12);
        // Each digit is two texels wider than its advance in
        // `dIFCommonPlayerDamageDigitWidths` (14, 9, 15, ...): the outline.
        assert_eq!(sprites[0].width, 16);
        assert_eq!(sprites[1].width, 11);
        for (i, s) in sprites.iter().enumerate() {
            assert!(s.width > 0 && s.width < 64 && s.height > 0 && s.height < 64);
            // Some texel is drawn, and the corners are clear.
            let alpha: Vec<u8> = s
                .image
                .pixels
                .as_chunks::<4>()
                .0
                .iter()
                .map(|p| p[3])
                .collect();
            assert!(alpha.iter().any(|&a| a > 0), "digit {i} is empty");
            // The digits and `%` have clear corners; `H.P` is boxed.
            if i < 11 {
                assert_eq!(alpha[0], 0, "digit {i}'s corner is drawn");
            }
            assert_eq!((s.format, s.size), (Format::Ia, BitSize::Bits8));
            assert_eq!(
                s.height,
                [19, 19, 19, 19, 19, 19, 19, 19, 19, 19, 16, 12][i]
            );
        }
    }
}
