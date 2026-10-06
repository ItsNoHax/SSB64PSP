//! How to Play's data (`sc/sccommon/scexplain.c`): the phase table and the
//! fighters' input scripts in `SCExplainMain` (file 0xFC), and the window's
//! sprites, the control stick's, tap spark's and colour overlay's textures
//! and their material animations in `SCExplainGraphics` (file 0xC6).
//!
//! `romtool pack` puts them in the scene's menu pack
//! ([`crate::menu_pack::MenuScene::Explain`]): the sprites as sprites, the
//! raw textures as sprites keyed by their texture's offset, and three byte
//! blobs as `AnimDesc::EFFECT` slots ([`PHASES_SLOT`], [`KEYS_SLOT`],
//! [`ANIMS_SLOT`]), as the title's baked plays are.

use alloc::vec::Vec;

use crate::archive::File;
use crate::sprite::SpriteFile;
use crate::texture::{self, BitSize, Format, Rgba8};

/// `llSCExplainGraphicsFileID` and `llSCExplainMainFileID`.
pub const FILE_GRAPHICS: u32 = 0xC6;
pub const FILE_MAIN: u32 = 0xFC;

/// `llSCExplainMainExplainPhase`, 22 entries of `sizeof(SCExplainPhase)`.
pub const PHASES: u32 = 0x1404;
pub const PHASE_SIZE: u32 = 0x2C;
pub const PHASE_COUNT: u32 = 22;
/// `SCExplainPhase.sprite`: a pointer into the graphics file.
const PHASE_SPRITE: u32 = 0x8;

/// The menu pack's blobs: the phase table with each sprite pointer
/// replaced by its offset in the graphics file; `SCExplainMain` up to the
/// table (the four input scripts, `llSCExplainMain{0..3}KeyEvent`); and
/// `SCExplainGraphics` up to [`ANIM_END`] (the stick's and spark's
/// material animations, which point within it).
pub const PHASES_SLOT: u32 = 0xF210;
pub const KEYS_SLOT: u32 = 0xF211;
pub const ANIMS_SLOT: u32 = 0xF212;
/// The end of the last material animation (`TapSparkMatAnimJoint`'s
/// script).
pub const ANIM_END: u32 = 0x5C48;

/// Every caption a phase shows (`reloc_data.us.h`), the first sprite
/// `scExplainSetPhaseSObjs` makes, then the button and marker sprites:
/// `A`, `B`, `Z`, "Here" and the plus sign.
pub const SPRITES: SpriteFile = SpriteFile {
    file: FILE_GRAPHICS,
    offsets: &[
        0x6C58, 0x72D8, 0x7C38, 0x8218, 0x8C78, 0x91A8, 0x10260, 0x11F60, 0x12B60, 0x13658,
        0x139F0, 0x14448, 0x14E30, 0x15C40, 0x17FE0, 0x1A440, 0x1AA10, 0x1B468, 0x1B950, 0x1BEB0,
        0x1CD20, 0x9628, 0x1D338, 0x1D948, 0x1DF58, 0x1E018,
    ],
};

/// The stick's five IA8 64 x 64 frames, the spark's three I4 32 x 32 and
/// the colour overlay's CI4 16 x 48 with its palette.
pub const STICK_TEXTURES: [u32; 5] = [0x4028, 0x3020, 0x2018, 0x1010, 0x8];
pub const SPARK_TEXTURES: [u32; 3] = [0x5898, 0x5690, 0x5488];
pub const RGB_TEXTURE: u32 = 0x5C58;
pub const RGB_PALETTE: u32 = 0x5DE0;
/// The overlay's quad samples s 0..60 under clamp: the packed texture is
/// its 16 columns then the last repeated out to this width.
pub const RGB_WIDTH: u32 = 60;

/// The phase table, each sprite pointer replaced by its offset in the
/// graphics file (big-endian, as the rest of the table).
pub fn phase_table(main: &File) -> Option<Vec<u8>> {
    let start = PHASES as usize;
    let end = start + (PHASE_SIZE * PHASE_COUNT) as usize;
    let mut out = main.data.get(start..end)?.to_vec();
    for i in 0..PHASE_COUNT {
        let slot = PHASES + i * PHASE_SIZE + PHASE_SPRITE;
        let r = main.extern_relocs.iter().find(|r| r.at == slot)?;
        if r.target_file as u32 != FILE_GRAPHICS {
            return None;
        }
        let at = (slot - PHASES) as usize;
        out[at..at + 4].copy_from_slice(&r.target_offset.to_be_bytes());
    }
    Some(out)
}

/// The input scripts: the main file up to the phase table.
pub fn key_scripts(main: &File) -> Option<Vec<u8>> {
    Some(main.data.get(..PHASES as usize)?.to_vec())
}

/// The graphics file up to [`ANIM_END`], whose intern pointers are already
/// offsets within it.
pub fn anim_bytes(graphics: &File) -> Option<Vec<u8>> {
    Some(graphics.data.get(..ANIM_END as usize)?.to_vec())
}

/// A stick frame: IA8, drawn `TEXEL0 * SHADE` with white shade.
pub fn stick_texture(graphics: &File, at: u32) -> Option<Rgba8> {
    let data = graphics.data.get(at as usize..)?;
    texture::decode(data, 64, 64, Format::Ia, BitSize::Bits8, None).ok()
}

/// A spark frame: I4, drawn with the shade's colour through `TEXEL0`'s
/// alpha. Baked white with alpha I, for the host to tint.
pub fn spark_texture(graphics: &File, at: u32) -> Option<Rgba8> {
    let data = graphics.data.get(at as usize..)?;
    let mut img = texture::decode(data, 32, 32, Format::I, BitSize::Bits4, None).ok()?;
    for p in img.pixels.as_chunks_mut::<4>().0 {
        *p = [0xFF, 0xFF, 0xFF, p[3]];
    }
    Some(img)
}

/// The colour overlay: CI4 through its RGBA16 palette, its last column
/// held out to [`RGB_WIDTH`].
pub fn rgb_texture(graphics: &File) -> Option<Rgba8> {
    let pal = graphics
        .data
        .get(RGB_PALETTE as usize..RGB_PALETTE as usize + 32)?;
    let data = graphics.data.get(RGB_TEXTURE as usize..)?;
    let src = texture::decode(
        data,
        16,
        48,
        Format::Ci,
        BitSize::Bits4,
        Some(&texture::parse_tlut(pal)),
    )
    .ok()?;
    let mut out = Rgba8::new(RGB_WIDTH, 48);
    for y in 0..48 {
        for x in 0..RGB_WIDTH {
            let sx = x.min(15);
            out.put(
                (y * RGB_WIDTH + x) as usize,
                src.get((y * 16 + sx) as usize),
            );
        }
    }
    Some(out)
}
