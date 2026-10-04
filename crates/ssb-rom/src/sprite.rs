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
pub const SP_FASTCOPY: u16 = 0x0020;
/// `SP_TEXSHUF`: 32-bit strips are stored shuffled.
pub const SP_TEXSHUF: u16 = 0x0200;

/// `SC1PTrainingMode`'s four tables (file 254). Sprites are keyed in the
/// pack by the pointer slot, even when it refers to another archive file.
pub const TRAINING_FILE: u32 = 254;
pub const TRAINING_TABLES: [(u32, usize, u32); 4] =
    [(0, 4, 8), (0x20, 39, 4), (0xBC, 10, 8), (0x13C, 31, 4)];
pub const TRAINING_LAYOUT_LEN: usize = 0x1B8;

pub fn training_sprite_slots() -> impl Iterator<Item = u32> {
    TRAINING_TABLES.into_iter().flat_map(|(at, count, stride)| {
        (0..count).map(move |i| at + i as u32 * stride + if stride == 8 { 4 } else { 0 })
    })
}

/// The source sprite of a Training table entry, resolved through the
/// archive relocation rather than treating a patched pointer as an offset.
pub fn training_sprite_ref(file: &File, slot: u32) -> Result<(u32, u32), SpriteError> {
    if let Some(r) = file.extern_relocs.iter().find(|r| r.at == slot) {
        return Ok((u32::from(r.target_file), r.target_offset));
    }
    pointer(file, slot).map(|at| (file.id, at))
}

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

/// `G_IM_SIZ_4c`, which the ROM's own sprite loader expands: two bits
/// per texel.
const G_IM_SIZ_4C: u8 = 4;

/// `lbCommonDecodeBitmapSiz4b`: each byte's four 2-bit texels, most
/// significant first, become 4-bit ones through
/// `lbCommonGetBitmapDecodeNibble`'s levels 0, 5, 10 and 15.
fn expand_siz_4c(packed: &[u8]) -> Vec<u8> {
    const LEVEL: [u8; 4] = [0x0, 0x5, 0xA, 0xF];
    let mut out = Vec::with_capacity(packed.len() * 2);
    for &b in packed {
        let texel = |shift: u8| LEVEL[usize::from((b >> shift) & 3)];
        out.push(texel(6) << 4 | texel(4));
        out.push(texel(2) << 4 | texel(0));
    }
    out
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
    // `G_IM_SIZ_4c`: `lbCommonMakeSObjForGObj` expands it to 4b first.
    let packed_4c = siz == G_IM_SIZ_4C;
    let size = if packed_4c {
        BitSize::Bits4
    } else {
        BitSize::from_raw(siz).ok_or(SpriteError::BadFormat(fmt, siz))?
    };
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
        let mut texels = if packed_4c {
            // `res = (width_img / 2) * actualHeight` bytes of 4b texels
            // from `res / 2` bytes of 2b ones.
            let res = (width_img / 2) * rows;
            let mut out = expand_siz_4c(d.get(buf..buf + res / 2).ok_or(SpriteError::Truncated)?);
            out.resize(row_bytes * rows, 0);
            out
        } else {
            d.get(buf..buf + row_bytes * rows)
                .ok_or(SpriteError::Truncated)?
                .to_vec()
        };
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

/// File 165, `IFCommonTimer` (`gGMCommonFiles[3]`): `Digit0` to `Digit9`,
/// then `SymbolColon`, `...Cross`, `...Underscore`, `...Sec` and `...CSec`
/// (`dIFCommonTimerDigitSpriteOffsets`).
pub const TIMER: SpriteFile = SpriteFile {
    file: 165,
    offsets: &[
        0x138, 0x228, 0x3A8, 0x528, 0x6A8, 0x828, 0x9A8, 0xB28, 0xCA8, 0xE28, 0xF08, 0x1018,
        0x1090, 0x1140, 0x1238,
    ],
};

/// File 37, `IFCommonAnnounceCommon` (`gGMCommonFiles[7]`): `LetterA` to
/// `LetterZ`, then `SymbolExclaim` and `SymbolPeriod`.
pub const ANNOUNCE_COMMON: SpriteFile = SpriteFile {
    file: 37,
    offsets: &[
        0x5E0, 0x9A8, 0xD80, 0x1268, 0x1628, 0x1A00, 0x1F08, 0x2408, 0x26B8, 0x2A90, 0x2F98,
        0x3358, 0x3980, 0x3E88, 0x44B0, 0x4890, 0x4F10, 0x5418, 0x57F0, 0x5BD0, 0x60D8, 0x65D8,
        0x6C00, 0x7108, 0x7608, 0x7AE8, 0x7D98, 0x7E50,
    ],
};

/// File 197, `IFCommonBattlePause` (`gGMCommonFiles[5]`): `PlayerNum1P` to
/// `...4P`, then the decals `Pause`, `Plus`, `Reset`, `SmashBall`, `Retry`,
/// `AButton`, `BButton`, `ZTrigger`, `RTrigger`, `Arrows`, `ControlStick`
/// and `LTrigger`.
pub const BATTLE_PAUSE: SpriteFile = SpriteFile {
    file: 197,
    offsets: &[
        0x78, 0x138, 0x1F8, 0x2B8, 0x438, 0x4D8, 0x610, 0x6D8, 0x828, 0x958, 0xA88, 0xBD8, 0xCF8,
        0x1538, 0x17A8, 0x18C8,
    ],
};

/// File 34, `MNVSResults` (`dMNVSResultsFileIDs[0]`): `TKOTextSprite`,
/// `PlaceTextSprite`, `KOsTextSprite`, `PtsTextSprite`, `1PArrowSprite`
/// to `4PArrowSprite`, `WallpaperSprite` and `WinnerSprite` (RE-410).
pub const VS_RESULTS: SpriteFile = SpriteFile {
    file: 34,
    offsets: &[
        0x358, 0x990, 0xD38, 0x10D8, 0x49E8, 0x4B08, 0x4C28, 0x4D48, 0xD5C8, 0xE2A0,
    ],
};

/// File 18, `MNPlayersGameModes` (`dMNVSResultsFileIDs[2]`):
/// `FreeForAllTextSprite` and `TeamBattleTextSprite` (RE-410), then
/// `TrainingModeTextSprite` (`mnPlayers1PTrainingMakeLabels`).
pub const GAME_MODES: SpriteFile = SpriteFile {
    file: 18,
    offsets: &[0x280, 0x4E0, 0x758],
};

/// File 36, `IFCommonDigits` (`dMNVSResultsFileIDs[5]`): `Digits0Sprite`
/// to `Digits9Sprite`, then `DigitsDashSprite` (RE-410).
pub const DIGITS: SpriteFile = SpriteFile {
    file: 36,
    offsets: &[
        0x68, 0x118, 0x1C8, 0x278, 0x328, 0x3D8, 0x488, 0x538, 0x5E8, 0x698, 0x710, 0x828, 0x8D8,
    ],
};

/// File 38, `IFCommonPlayerTags` (`dMNVSResultsFileIDs[1]`):
/// `PlayerTags1PSprite` to `...4PSprite`, then `...CPSprite` (RE-410).
pub const PLAYER_TAGS: SpriteFile = SpriteFile {
    file: 38,
    offsets: &[0x258, 0x4F8, 0x798, 0xA38, 0xCD8, 0xEB8],
};

/// File 87, `IFCommonItem` (`llIFCommonItemFileID`): `ArrowSprite`, the
/// pickup arrow `ifCommonItemArrowSetAttr` loads.
pub const ITEM_ARROW: SpriteFile = SpriteFile {
    file: 87,
    offsets: &[0x50],
};

/// File 17, `MNPlayersCommon` (`dMNPlayersVSFileIDs[0]`), the sprites the
/// VS character select draws (RE-411), in `reloc_data.us.h`'s order:
/// `1PText` to `4PText`, `CPText`, `HandicapText`, `CPLevelText`,
/// `StartText`, `PressText`, the twelve names (`MarioText` to
/// `JigglypuffText`), `InfinityDark`, `TimeSelector`, `StockSelector`,
/// `0Dark` to `9Dark`, `HmnLabel`, `CPLabel`, `NALabel`, `CursorHandPoint`,
/// `...Grab`, `...Hover`, `1PTextGradient` to `4PTextGradient`, `1PPuck`
/// to `4PPuck`, `CPPuck`, `SmashLogoCardLeft`, `...Right`, `RedLabel`,
/// `GreenLabel`, `BlueLabel`, `ArrowL`, `ArrowR`, `ReadyToFightText`,
/// `ReadyBanner` and `BackButton`. `RedCard` is packed once per gate LUT
/// ([`GATE_LUTS`]).
pub const PLAYERS_COMMON: SpriteFile = SpriteFile {
    file: 17,
    offsets: &[
        0x878, 0xA58, 0xC38, 0xE18, 0xFF8, 0x1108, 0x1218, 0x1378, 0x14D8, 0x1838, 0x1B18, 0x1FF8,
        0x2358, 0x25B8, 0x28E8, 0x2BA0, 0x2ED8, 0x32F8, 0x35B0, 0x3998, 0x3DB8, 0x3EF0, 0x48B0,
        0x5270, 0x5388, 0x5440, 0x5558, 0x5668, 0x5778, 0x5888, 0x5998, 0x5AA8, 0x5BB8, 0x5CC8,
        0x6048, 0x63C8, 0x6748, 0x6F88, 0x76E8, 0x8168, 0x8268, 0x8368, 0x8468, 0x8568, 0x9048,
        0x9B28, 0xA608, 0xB0E8, 0xBBC8, 0xCDB0, 0xDFA0, 0xE3C8, 0xE7E8, 0xEC08, 0xECE8, 0xEDC8,
        0xF448, 0xF530, 0x115C8,
    ],
};

/// File 17's `RedCardSprite`, the player panel `mnPlayersVSMakeGate` makes.
pub const GATE_CARD: u32 = 0x104B0;

/// The TLUTs `mnPlayersVSSetGateLUT` swaps into [`GATE_CARD`]:
/// `man_offsets` (`GateMan1PLUT` to `...4PLUT`), then `com_offsets`
/// (`GateCom1PLUT` to `...4PLUT`). The file stores 3P after 4P.
pub const GATE_LUTS: [u32; 8] = [
    0x103F8, 0x10420, 0x10470, 0x10448, 0x11378, 0x113A0, 0x113F0, 0x113C8,
];

/// File 23, `MNPlayers1PMode` (`dMNPlayers1PTrainingFileIDs[1]`):
/// `RedCardSprite`, the Training select's panel (`mnPlayers1PTrainingMakeGate`,
/// ). It is 82 x 91 CI4, wider than [`GATE_CARD`].
pub const PLAYERS_1P_MODE_FILE: u32 = 23;
pub const TRAINING_GATE_CARD: u32 = 0x32A8;

/// The TLUTs `mnPlayers1PTrainingSetGateLUT` and
/// `mnPlayers1PGameSetGateLUT` swap into [`TRAINING_GATE_CARD`], as
/// `(file, offset)`: the player's `MNPlayersCommon` `GateMan1PLUT` (the
/// Training man is port 0), `MNPlayers1PMode`'s `GateCPLUT`, then
/// `GateMan2PLUT` to `...4PLUT` for a 1P Game on another port.
pub const TRAINING_GATE_LUTS: [(u32, u32); 5] = [
    (17, 0x103F8),
    (23, 0x3238),
    (17, 0x10420),
    (17, 0x10470),
    (17, 0x10448),
];

/// File 23's sprites `mnPlayers1PGame*` draws besides the card, in its
/// sprite manifest's order: `1PlayerGameText`, `ClosingParenthesis`,
/// `OpeningParenthesis`, `LevelColonText`, `StockColonText`,
/// `OptionOutline`, `SmashLogo` and `OptionText`.
pub const PLAYERS_1P_MODE: SpriteFile = SpriteFile {
    file: PLAYERS_1P_MODE_FILE,
    offsets: &[0x228, 0x2C8, 0x368, 0x488, 0x5A8, 0x1208, 0x1950, 0x1EC8],
};

/// File 24, `MNPlayersDifficulty` (`dMNPlayers1PGameFileIDs[6]`):
/// `EasyText`, `HardText`, `NormalText`, `VeryEasyText`, `VeryHardText`.
pub const PLAYERS_DIFFICULTY: SpriteFile = SpriteFile {
    file: 24,
    offsets: &[0x98, 0x178, 0x2D8, 0x438, 0x598],
};

/// File 25, `FTStocksZako` (`dMNPlayers1PGameFileIDs[7]`): the Fighting
/// Polygon's stock icon, 8 x 8 CI4.
pub const STOCKS_ZAKO: SpriteFile = SpriteFile {
    file: 25,
    offsets: &[0x80],
};

/// File 33, `MNCommonFonts` (`dMNPlayers1PGameFileIDs[8]`): `LetterA` to
/// `LetterZ`, `SymbolApostrophe`, `SymbolPercent` and `SymbolPeriod`.
pub const COMMON_FONTS: SpriteFile = SpriteFile {
    file: 33,
    offsets: &[
        0x40, 0xD0, 0x160, 0x1F0, 0x280, 0x310, 0x3A0, 0x430, 0x4C0, 0x550, 0x5E0, 0x670, 0x700,
        0x790, 0x820, 0x8B0, 0x940, 0x9D0, 0xA60, 0xAF0, 0xB80, 0xC10, 0xCA0, 0xD30, 0xDC0, 0xE50,
        0xED0, 0xF60, 0xFD0,
    ],
};

/// File 0, `MNCommon` (`dMNPlayersVSFileIDs[1]`): `Digit0Sprite` to
/// `Digit9Sprite`, then `ColonSprite` (RE-411).
pub const MN_COMMON: SpriteFile = SpriteFile {
    file: 0,
    offsets: &[
        0xD310, 0xD3E0, 0xD4B0, 0xD580, 0xD650, 0xD720, 0xD7F0, 0xD8C0, 0xD990, 0xDA60, 0xDCF0,
    ],
};

/// File 19, `MNPlayersPortraits` (`dMNPlayersVSFileIDs[5]`):
/// `WhiteSquare`, `PortraitQuestionMark`, `PortraitFireBg`, the twelve
/// portraits in the file's order (`Mario`, `Luigi`, `Donkey`, `Samus`,
/// `Fox`, `Kirby`, `Link`, `Yoshi`, `Pikachu`, `Ness`, `Captain`,
/// `Purin`), then `CaptainShadow`, `LuigiShadow`, `NessShadow` and
/// `PurinShadow` (RE-411).
pub const PORTRAITS: SpriteFile = SpriteFile {
    file: 19,
    offsets: &[
        0x6F0, 0xF68, 0x24D0, 0x4728, 0x6978, 0x8BC8, 0xAE18, 0xD068, 0xF2B8, 0x11508, 0x13758,
        0x159A8, 0x17BF8, 0x19E48, 0x1C098, 0x1E2E8, 0x20538, 0x22788, 0x249D8,
    ],
};

/// File 20, `FTEmblemSprites` (`dMNPlayersVSFileIDs[2]`): `Mario`,
/// `Donkey`, `Metroid`, `Fox`, `Kirby`, `Zelda`, `Yoshi`, `FZero`,
/// `PMonsters` and `Mother` (RE-411).
pub const EMBLEM_SPRITES: SpriteFile = SpriteFile {
    file: 20,
    offsets: &[
        0x618, 0xC78, 0x12D8, 0x1938, 0x1F98, 0x25F8, 0x2C58, 0x32B8, 0x3918, 0x3F78,
    ],
};

/// File 21, `MNSelectCommon` (`dMNPlayersVSFileIDs[3]`):
/// `StoneBackgroundSprite` (RE-411).
pub const SELECT_COMMON: SpriteFile = SpriteFile {
    file: 21,
    offsets: &[0x440],
};

/// File 30, `MNMaps` (`dMNMapsFileIDs[2]`), the stage select's sprites
/// (RE-419), in `reloc_data.us.h`'s order: the nine names
/// (`PeachsCastleText`, `SectorZText`, `CongoJungleText`,
/// `PlanetZebesText`, `HyruleCastleText`, `YoshisIslandText`,
/// `SaffronCityText`, `MushroomKingdomText`, `DreamLandText`), `Cursor`,
/// `QuestionMark`, `StageSelectText`, `WoodenCircle`, `PlateRight`,
/// `PlateMiddle`, `PlateLeft`, the nine icons (`PeachsCastle`, `SectorZ`,
/// `CongoJungle`, `PlanetZebes`, `HyruleCastle`, `YoshisIsland`,
/// `SaffronCity`, `MushroomKingdom`, `DreamLand`), `Tiles`, `RandomSmall`
/// and `RandomBig`.
pub const MN_MAPS: SpriteFile = SpriteFile {
    file: 30,
    offsets: &[
        0x1F8, 0x438, 0x678, 0x8B8, 0xB10, 0xD58, 0xF98, 0x11D8, 0x1418, 0x1AB8, 0x1DD8, 0x26A0,
        0x3840, 0x3C68, 0x3D68, 0x3FA8, 0x4D88, 0x5B68, 0x6948, 0x7728, 0x8508, 0x92E8, 0xA0C8,
        0xAEA8, 0xBC88, 0xC728, 0xCB10, 0xDE30,
    ],
};

/// `llGRWallpaperTrainingBlackFileID`, `...YellowFileID` and
/// `...BlueFileID` (`dSC1PTrainingModeWallpaperDescs`): Training's three
/// wallpapers, each a 300 x 220 RGBA16 `Sprite` at
/// [`TRAINING_WALLPAPER_SPRITE`] (RE-419).
pub const TRAINING_WALLPAPER_FILES: [u32; 3] = [0x1A, 0x1B, 0x1C];

/// `llGRWallpaperTrainingBlackSprite` (the yellow and blue share it).
pub const TRAINING_WALLPAPER_SPRITE: u32 = 0x20718;

/// `SP_CLOUD`: drawn with `G_RM_CLD_SURF`, blended like `SP_TRANSPARENT`.
pub const SP_CLOUD: u16 = 0x1000;

/// Every sprite file the pack converts.
pub const FILES: &[SpriteFile] = &[
    crate::campaign::INTRO,
    crate::campaign::NAMES,
    crate::campaign::PICTURES,
    crate::campaign::PLATFORM_PICTURE,
    crate::campaign::CONTINUE,
    crate::campaign::CLEAR,
    crate::campaign::SCORE,
    crate::campaign::OBJECTIVES,
    PLAYER_DAMAGE,
    GAME_STATUS,
    TIMER,
    ANNOUNCE_COMMON,
    BATTLE_PAUSE,
    VS_RESULTS,
    GAME_MODES,
    DIGITS,
    PLAYER_TAGS,
    ITEM_ARROW,
    PLAYERS_COMMON,
    MN_COMMON,
    PORTRAITS,
    EMBLEM_SPRITES,
    SELECT_COMMON,
    MN_MAPS,
    PLAYERS_1P_MODE,
    PLAYERS_DIFFICULTY,
    STOCKS_ZAKO,
    COMMON_FONTS,
];

/// [`GATE_CARD`] decoded through `GATE_LUTS[lut]`.
pub fn decode_gate(file: &File, lut: usize) -> Result<Sprite, SpriteError> {
    let at = GATE_LUTS[lut] as usize;
    let bytes = file.data.get(at..at + 32).ok_or(SpriteError::Truncated)?;
    decode_with_tlut(file, GATE_CARD, &texture::parse_tlut(bytes))
}

/// [`TRAINING_GATE_CARD`] of `card` (file 23) decoded through
/// `TRAINING_GATE_LUTS[lut]`, read from `lut_file`.
pub fn decode_training_gate(
    card: &File,
    lut_file: &File,
    lut: usize,
) -> Result<Sprite, SpriteError> {
    let at = TRAINING_GATE_LUTS[lut].1 as usize;
    let bytes = lut_file
        .data
        .get(at..at + 32)
        .ok_or(SpriteError::Truncated)?;
    decode_with_tlut(card, TRAINING_GATE_CARD, &texture::parse_tlut(bytes))
}

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
    fn siz_4c_expands_like_lb_common_decode_bitmap_siz_4b() {
        // 0b11_10_01_00: texels 15, 10, 5, 0.
        assert_eq!(expand_siz_4c(&[0xE4, 0x1B]), [0xFA, 0x50, 0x05, 0xAF]);
    }

    #[test]
    fn the_1p_select_sprites_decode() {
        let Some(path) = std::env::var_os("SSB64_ROM") else {
            return;
        };
        let data = std::fs::read(path).unwrap();
        let info = crate::rom::identify(&data).unwrap();
        let archive = crate::archive::Archive::open(&data, info.region).unwrap();
        let sizes = |f: &SpriteFile| -> Vec<_> {
            let file = archive.load(f.file).unwrap();
            decode_all(&file, f)
                .unwrap()
                .iter()
                .map(|s| (s.width, s.height, s.format, s.size))
                .collect()
        };
        use BitSize::{Bits4, Bits8};
        use Format::{Ci, Ia, I};
        assert_eq!(
            sizes(&PLAYERS_1P_MODE),
            [
                (86, 11, I, Bits4),
                (3, 8, I, Bits4),
                (3, 8, I, Bits4),
                (45, 8, I, Bits4),
                (46, 8, I, Bits4),
                (192, 32, I, Bits4),
                (16, 11, I, Bits4),
                (72, 18, Ia, Bits8),
            ]
        );
        assert_eq!(
            sizes(&PLAYERS_DIFFICULTY),
            [
                (31, 8, I, Bits4),
                (32, 8, I, Bits4),
                (50, 8, I, Bits4),
                (61, 8, I, Bits4),
                (62, 8, I, Bits4),
            ]
        );
        assert_eq!(sizes(&STOCKS_ZAKO), [(8, 8, Ci, Bits4)]);
        let fonts = sizes(&COMMON_FONTS);
        assert_eq!(fonts.len(), 29);
        assert!(fonts.iter().all(|&(_, _, f, b)| (f, b) == (I, Bits4)));
        assert_eq!((fonts[0].0, fonts[0].1), (5, 5), "LetterA");
        assert_eq!((fonts[27].0, fonts[27].1), (7, 5), "SymbolPercent");
    }

    #[test]
    fn the_results_screen_sprites_decode() {
        let Some(path) = std::env::var_os("SSB64_ROM") else {
            return;
        };
        let data = std::fs::read(path).unwrap();
        let info = crate::rom::identify(&data).unwrap();
        let archive = crate::archive::Archive::open(&data, info.region).unwrap();
        let formats = |f: &SpriteFile| {
            let file = archive.load(f.file).unwrap();
            decode_all(&file, f)
                .unwrap()
                .iter()
                .map(|s| (s.width, s.height, s.format, s.size))
                .collect::<Vec<_>>()
        };
        let results = formats(&VS_RESULTS);
        // The row labels and arrows are IA8, the wallpaper I4, the
        // first-place "winner" plate RGBA16.
        assert_eq!(results[0], (62, 13, Format::Ia, BitSize::Bits8));
        assert_eq!(results[1], (83, 17, Format::Ia, BitSize::Bits8));
        assert_eq!(results[4], (15, 12, Format::Ia, BitSize::Bits8));
        assert_eq!(results[8], (300, 220, Format::I, BitSize::Bits4));
        assert_eq!(results[9], (42, 35, Format::Rgba, BitSize::Bits16));
        let modes = formats(&GAME_MODES);
        assert_eq!(modes[0], (112, 11, Format::I, BitSize::Bits4));
        assert_eq!(modes[1], (110, 9, Format::I, BitSize::Bits4));
        let digits = formats(&DIGITS);
        assert_eq!(digits[0], (8, 10, Format::Ia, BitSize::Bits8));
        assert_eq!(digits[1].0, 5, "the 1 is narrow");
        assert_eq!(digits[10], (6, 3, Format::Ia, BitSize::Bits8));
        let tags = formats(&PLAYER_TAGS);
        assert!(tags.iter().all(|t| t.1 == 24 && t.2 == Format::Ia));
        assert_eq!(formats(&ITEM_ARROW), [(9, 7, Format::I, BitSize::Bits4)]);
    }

    #[test]
    fn the_vs_select_sprites_decode() {
        let Some(path) = std::env::var_os("SSB64_ROM") else {
            return;
        };
        let data = std::fs::read(path).unwrap();
        let info = crate::rom::identify(&data).unwrap();
        let archive = crate::archive::Archive::open(&data, info.region).unwrap();
        let formats = |f: &SpriteFile| {
            let file = archive.load(f.file).unwrap();
            decode_all(&file, f)
                .unwrap()
                .iter()
                .map(|s| (s.width, s.height, s.format, s.size))
                .collect::<Vec<_>>()
        };
        let common = formats(&PLAYERS_COMMON);
        assert_eq!(common.len(), 59);
        // The "1P" card text is I4, the names IA8, the selector RGBA16,
        // the HMN label RGBA32, the hand IA16, the puck RGBA32, the doors
        // IA8, the arrows and back button CI4, the banner an 8-texel IA8
        // strip.
        assert_eq!(common[0], (39, 16, Format::I, BitSize::Bits4));
        assert_eq!(common[9], (47, 16, Format::Ia, BitSize::Bits8));
        assert_eq!(common[22], (90, 13, Format::Rgba, BitSize::Bits16));
        assert_eq!(common[34], (20, 10, Format::Rgba, BitSize::Bits32));
        assert_eq!(common[37], (27, 36, Format::Ia, BitSize::Bits16));
        assert_eq!(common[44], (26, 24, Format::Rgba, BitSize::Bits32));
        assert_eq!(common[49], (41, 92, Format::Ia, BitSize::Bits8));
        assert_eq!(common[54], (7, 11, Format::Ci, BitSize::Bits4));
        assert_eq!(common[57], (8, 17, Format::Ia, BitSize::Bits8));
        assert_eq!(common[58], (48, 11, Format::Ci, BitSize::Bits4));
        assert_eq!(formats(&MN_COMMON)[10], (3, 9, Format::I, BitSize::Bits4));
        let portraits = formats(&PORTRAITS);
        assert!(portraits[3..]
            .iter()
            .all(|p| *p == (45, 43, Format::Rgba, BitSize::Bits32)));
        assert_eq!(portraits[2], (45, 43, Format::Rgba, BitSize::Bits16));
        assert!(formats(&EMBLEM_SPRITES)
            .iter()
            .all(|e| *e == (64, 48, Format::I, BitSize::Bits4)));
        // The stone tile `mnPlayersVSMakeWallpaper` wraps with masks 6 and 5.
        assert_eq!(
            formats(&SELECT_COMMON)[0],
            (64, 32, Format::Ci, BitSize::Bits4)
        );
        // Each gate LUT gives the card its port's colour: red, blue, yellow
        // and green for a human, paler for a CPU.
        let file = archive.load(PLAYERS_COMMON.file).unwrap();
        let mean = |lut: usize| {
            let s = decode_gate(&file, lut).unwrap();
            assert_eq!((s.width, s.height), (66, 91));
            let mut sum = [0u32; 3];
            let mut n = 0;
            for px in s
                .image
                .pixels
                .as_chunks::<4>()
                .0
                .iter()
                .filter(|p| p[3] > 0)
            {
                for i in 0..3 {
                    sum[i] += u32::from(px[i]);
                }
                n += 1;
            }
            sum.map(|c| c / n)
        };
        let [r, g, b] = mean(0);
        assert!(r > 2 * g && r > 2 * b, "1P red");
        let [r, _, b] = mean(1);
        assert!(b > 2 * r, "2P blue");
        let [r, g, b] = mean(2);
        assert!(r > 10 * b && g > 10 * b, "3P yellow");
        let [r, g, _] = mean(3);
        assert!(g > 2 * r, "4P green");
        let [r, g, _] = mean(4);
        assert!(r > g && r - g < 40, "the CPU's red is pale");

        // The Training select's own card: wider, red for the
        // player through `GateMan1PLUT`, grey for the CPU through
        // `MNPlayers1PMode`'s `GateCPLUT`.
        let card = archive.load(PLAYERS_1P_MODE_FILE).unwrap();
        let lut_file = |lut: usize| archive.load(TRAINING_GATE_LUTS[lut].0).unwrap();
        let mean = |lut: usize| {
            let s = decode_training_gate(&card, &lut_file(lut), lut).unwrap();
            assert_eq!((s.width, s.height), (82, 91));
            assert_eq!((s.format, s.size), (Format::Ci, BitSize::Bits4));
            let opaque: Vec<_> = s
                .image
                .pixels
                .as_chunks::<4>()
                .0
                .iter()
                .filter(|p| p[3] > 0)
                .copied()
                .collect();
            let n = opaque.len() as u32;
            [0, 1, 2].map(|i| opaque.iter().map(|p| u32::from(p[i])).sum::<u32>() / n)
        };
        let [r, g, b] = mean(0);
        assert!(r > 2 * g && r > 2 * b, "the player's card is red");
        let [r, g, b] = mean(1);
        assert!(
            r.abs_diff(g) < 8 && g.abs_diff(b) < 8 && r > 0x60,
            "the CPU's is grey"
        );
        // The 1P Game's card on ports 2 to 4: blue, yellow, green.
        let [r, _, b] = mean(2);
        assert!(b > 2 * r, "2P blue");
        let [r, g, b] = mean(3);
        assert!(r > 10 * b && g > 10 * b, "3P yellow");
        let [r, g, _] = mean(4);
        assert!(g > 2 * r, "4P green");
        assert_eq!(
            formats(&GAME_MODES)[2],
            (88, 11, Format::I, BitSize::Bits4),
            "TrainingModeText"
        );
    }

    #[test]
    fn the_stage_select_sprites_and_wallpapers_decode() {
        let Some(path) = std::env::var_os("SSB64_ROM") else {
            return;
        };
        let data = std::fs::read(path).unwrap();
        let info = crate::rom::identify(&data).unwrap();
        let archive = crate::archive::Archive::open(&data, info.region).unwrap();
        let file = archive.load(MN_MAPS.file).unwrap();
        let maps: Vec<_> = decode_all(&file, &MN_MAPS)
            .unwrap()
            .iter()
            .map(|s| (s.width, s.height, s.format, s.size))
            .collect();
        // The names and cursor are I4, "Stage Select" IA8, the wooden
        // circle and random pictures CI4, the plates, icons and tiles
        // RGBA16.
        assert!(maps[..9]
            .iter()
            .all(|m| *m == (96, 10, Format::I, BitSize::Bits4)));
        assert_eq!(maps[9], (62, 50, Format::I, BitSize::Bits4));
        assert_eq!(maps[10], (24, 44, Format::I, BitSize::Bits4));
        assert_eq!(maps[11], (112, 19, Format::Ia, BitSize::Bits8));
        assert_eq!(maps[12], (84, 85, Format::Ci, BitSize::Bits4));
        assert_eq!(maps[14], (4, 20, Format::Rgba, BitSize::Bits16));
        assert!(maps[16..25]
            .iter()
            .all(|m| *m == (48, 36, Format::Rgba, BitSize::Bits16)));
        assert_eq!(maps[25], (16, 82, Format::Rgba, BitSize::Bits16));
        assert_eq!(maps[26], (48, 36, Format::Ci, BitSize::Bits4));
        assert_eq!(maps[27], (110, 82, Format::Ci, BitSize::Bits4));

        // Training's three and every VS stage's wallpaper are 300 x 220
        // RGBA16. Each stage's sits at 0x26C88 of its own file, the offset
        // `dMNMapsWallpaperOffsets` and `dSC1PTrainingModeWallpaperHeapOffsets`
        // subtract to find the file's heap.
        for id in TRAINING_WALLPAPER_FILES {
            let s = decode(&archive.load(id).unwrap(), TRAINING_WALLPAPER_SPRITE).unwrap();
            assert_eq!(
                (s.width, s.height, s.format, s.size),
                (300, 220, Format::Rgba, BitSize::Bits16)
            );
        }
        for (gkind, header) in crate::stage::ONE_P_WALLPAPER_GROUNDS {
            let id = crate::stage::COMMON_GROUND_FILES[usize::from(gkind)];
            let map = archive.load(id).unwrap();
            let (file, at) = crate::stage::wallpaper(&map, header).unwrap();
            let s = decode(&archive.load(file).unwrap(), at).unwrap();
            assert_eq!((s.width, s.height), (300, 220), "file {id:#x}");
        }
        let mut files = Vec::new();
        for id in crate::stage::VS_GROUND_FILES {
            let map = archive.load(id).unwrap();
            let (file, at) = crate::stage::wallpaper(&map, crate::stage::MAP_HEADER).unwrap();
            assert_eq!(at, 0x26C88, "file {id:#x}");
            let s = decode(&archive.load(file).unwrap(), at).unwrap();
            assert_eq!(
                (s.width, s.height, s.format, s.size),
                (300, 220, Format::Rgba, BitSize::Bits16)
            );
            assert!(
                s.image
                    .pixels
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .all(|p| p[3] == 0xFF),
                "opaque"
            );
            files.push(file);
        }
        files.sort_unstable();
        files.dedup();
        assert_eq!(files.len(), 9, "one file per stage");
    }

    #[test]
    fn the_announce_letters_decode() {
        let Some(path) = std::env::var_os("SSB64_ROM") else {
            return;
        };
        let data = std::fs::read(path).unwrap();
        let info = crate::rom::identify(&data).unwrap();
        let archive = crate::archive::Archive::open(&data, info.region).unwrap();
        let file = archive.load(ANNOUNCE_COMMON.file).unwrap();
        let sprites = decode_all(&file, &ANNOUNCE_COMMON).unwrap();
        assert_eq!(sprites.len(), 28);
        for (i, s) in sprites.iter().enumerate() {
            let alpha: Vec<u8> = s
                .image
                .pixels
                .as_chunks::<4>()
                .0
                .iter()
                .map(|p| p[3])
                .collect();
            assert!(alpha.iter().any(|&a| a > 0), "letter {i} is empty");
        }
        // IA capitals 36 to 39 texels tall (Q's tail is the 39); the period
        // is the short one.
        for (i, s) in sprites.iter().enumerate() {
            assert_eq!(s.format, Format::Ia, "letter {i}");
            let tall = if i == 27 { 11..=11 } else { 36..=39 };
            assert!(tall.contains(&s.height), "letter {i}: {}", s.height);
        }
        assert_eq!((sprites[16].width, sprites[16].height), (37, 39));
    }

    #[test]
    fn training_resolves_external_sprites_and_the_pack_preserves_table_identity() {
        let Some(path) = std::env::var_os("SSB64_ROM") else {
            return;
        };
        let data = std::fs::read(path).unwrap();
        let info = crate::rom::identify(&data).unwrap();
        let archive = crate::archive::Archive::open(&data, info.region).unwrap();
        let table = archive.load(TRAINING_FILE).unwrap();
        let slots: Vec<_> = training_sprite_slots().collect();
        assert_eq!(slots.len(), 84);
        let mut writer = crate::pack::PackWriter::new();
        for slot in slots {
            let (id, at) = training_sprite_ref(&table, slot).unwrap();
            assert_eq!(id, 29);
            let source = archive.load(id).unwrap();
            let s = decode(&source, at).unwrap();
            assert!(s.width > 0 && s.height > 0);
            writer.add_sprite(crate::pack::SpriteDesc {
                source_file: TRAINING_FILE,
                source_offset: slot,
                texture: 0,
                width: s.width,
                height: s.height,
                color: s.color,
                attr: s.attr,
                flags: 0,
                fighter: 0,
                role: 0,
                costume: 0,
                _pad: 0,
            });
        }
        writer.add_anim(
            crate::pack::AnimDesc::TRAINING_LAYOUT,
            0,
            TRAINING_FILE,
            0,
            &table.data[..TRAINING_LAYOUT_LEN],
            &[],
        );
        let bytes = writer.finish();
        let p = crate::pack::Pack::open(&bytes).unwrap();
        assert_eq!(
            p.training_layout(),
            Some(&table.data[..TRAINING_LAYOUT_LEN])
        );
        for slot in training_sprite_slots() {
            assert!(p.sprite(TRAINING_FILE, slot).is_some());
        }
        // Both View options are reached with the callback's ordinal,
        // independently of the decomp's names for the sprite enum.
        assert_ne!(
            p.sprite(TRAINING_FILE, 0x13C + 26 * 4).unwrap().width,
            p.sprite(TRAINING_FILE, 0x13C + 27 * 4).unwrap().width
        );
    }

    #[test]
    fn the_pause_menu_sprites_decode() {
        let Some(path) = std::env::var_os("SSB64_ROM") else {
            return;
        };
        let data = std::fs::read(path).unwrap();
        let info = crate::rom::identify(&data).unwrap();
        let archive = crate::archive::Archive::open(&data, info.region).unwrap();
        let file = archive.load(BATTLE_PAUSE.file).unwrap();
        let sprites = decode_all(&file, &BATTLE_PAUSE).unwrap();
        for (i, s) in sprites.iter().enumerate() {
            let alpha: Vec<u8> = s
                .image
                .pixels
                .as_chunks::<4>()
                .0
                .iter()
                .map(|p| p[3])
                .collect();
            assert!(alpha.iter().any(|&a| a > 0), "pause sprite {i} is empty");
            assert!(
                s.width <= 128 && s.height <= 64,
                "pause sprite {i}: {}x{}",
                s.width,
                s.height
            );
        }
    }

    #[test]
    fn the_timer_digits_decode() {
        let Some(path) = std::env::var_os("SSB64_ROM") else {
            return;
        };
        let data = std::fs::read(path).unwrap();
        let info = crate::rom::identify(&data).unwrap();
        let archive = crate::archive::Archive::open(&data, info.region).unwrap();
        let file = archive.load(TIMER.file).unwrap();
        let sprites = decode_all(&file, &TIMER).unwrap();
        for (i, s) in sprites.iter().enumerate() {
            let alpha: Vec<u8> = s
                .image
                .pixels
                .as_chunks::<4>()
                .0
                .iter()
                .map(|p| p[3])
                .collect();
            assert!(alpha.iter().any(|&a| a > 0), "timer sprite {i} is empty");
            assert!(s.width < 64 && s.height < 64, "timer sprite {i}");
        }
        // The ten digits share one height.
        assert!(sprites[..10].iter().all(|s| s.height == sprites[0].height));
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
