//! Menu scene data arrays (`d*` symbols in the scene overlays): the title's
//! fire animation, colours and sprite placement (`ovl10`,
//! `mn/mncommon/mntitle.c`) and the Characters scene's motion tables
//! (`ovl33`, `mn/mndata/mncharacters.c`).
//!
//! Each table's run-time address is its symbol in
//! `refs/ssb-decomp-re/symbols/symbols_us.txt`; the overlay is the one the
//! defining file's `.data` sits in (`smashbrothers.us.yaml`).

use alloc::vec::Vec;

use crate::source::{be_u16, OverlayData, OVL10, OVL26, OVL33};
use crate::tables::{le_words, Ctx};
use crate::Result;

/// `dMNTitleFireSpriteOffsets` (`intptr_t[30]`: `llMNTitleFireAnimFrame1Sprite`
/// to `...Frame30Sprite`, offsets into file `llMNTitleFireAnimFileID`).
pub const TITLE_FIRE_OFFSETS: (u32, usize) = (0x8013_41F0, 30);
/// `dMNTitleFireColorsR`, `...G` and `...B`: `u8[7]` each.
pub const TITLE_FIRE_COLORS: [u32; 3] = [0x8013_4318, 0x8013_4320, 0x8013_4328];
pub const TITLE_FIRE_COLOR_COUNT: u32 = 7;
/// `dMNTitleCommonSpriteDescs` (US): 12-byte `MNTitleSpriteDesc`s (`s32 x, y;
/// intptr_t offset`), the sprite's centre and its offset in `llMNTitleFileID`.
pub const TITLE_SPRITE_DESCS: (u32, usize) = (0x8013_4268, 10);

/// `dMNCharactersSpecialMotion{Mario,Fox,Donkey,Samus,Luigi,Link,Yoshi,
/// Captain,Kirby,Pikachu,Purin,Ness}`: twelve consecutive 288-byte
/// `MNCharactersSpecialMotion`s (`MNCharactersMotion[3][8]`: Hi, N, Lw), by
/// `FTKind`.
pub const SPECIAL_MOTIONS: (u32, usize) = (0x8013_40B0, 12);
const SPECIAL_MOTION_SIZE: u32 = 3 * 8 * MOTION_SIZE;
/// `dMNCharactersCommonMotionDescs`: `MNCharactersMotion[39][8]`, by motion
/// kind.
pub const COMMON_MOTIONS: (u32, usize) = (0x8013_4E30, 39);
/// `dMNCharactersKirbyJumpAerialMotionDescs` and the Purin ones
/// (`MNCharactersMotion[8]`), then `dMNCharactersKirbyFallMotionDesc` and
/// `...PurinFallMotionDesc` (one `MNCharactersMotion` each).
pub const KIRBY_JUMP_AERIAL: u32 = 0x8013_5CD0;
pub const KIRBY_FALL: u32 = 0x8013_5D30;
pub const PURIN_JUMP_AERIAL: u32 = 0x8013_5D3C;
pub const PURIN_FALL: u32 = 0x8013_5D9C;
/// `dMNCharactersAttack1MotionDescs`: `MNCharactersMotion[12][8]`, by `FTKind`.
pub const ATTACK1_MOTIONS: (u32, usize) = (0x8013_5DA8, 12);
/// `MNCharactersMotion`: `s32 status_id, anim_length; u32 flags`.
pub const MOTION_SIZE: u32 = 12;
/// The tracks of a motion row.
pub const TRACKS: u32 = 8;
/// `mnPlayersVSMakeGameRuleNumber`'s `offsets` (`intptr_t[10]`:
/// `llMNPlayersCommon0DarkSprite` to `...9DarkSprite`, `ovl26`).
pub const DARK_DIGIT_OFFSETS: (u32, usize) = (0x8013_B704, 10);
/// `llMNPlayersCommonFileID`.
pub const PLAYERS_COMMON_FILE: u32 = 17;
/// `Sprite.width` (`s16`) after `s16 x, y`.
const SPRITE_WIDTH: usize = 4;

/// `ssb-game`'s `TitleFire` table: `dMNTitleFireSpriteOffsets`.
pub fn title_fire_bytes(c: &Ctx) -> Result<Vec<u8>> {
    let d = OverlayData::new(Some(c.rom), OVL10);
    let (at, n) = TITLE_FIRE_OFFSETS;
    let words = (0..n as u32)
        .map(|i| d.u32(at + i * 4))
        .collect::<Result<Vec<_>>>()?;
    Ok(le_words(&words))
}

/// The `TitleFireColors` table: `dMNTitleFireColors{R,G,B}`, by colour then
/// channel.
pub fn title_fire_colors_bytes(c: &Ctx) -> Result<Vec<u8>> {
    let d = OverlayData::new(Some(c.rom), OVL10);
    let mut out = Vec::new();
    for i in 0..TITLE_FIRE_COLOR_COUNT {
        for &a in &TITLE_FIRE_COLORS {
            out.push(d.u8(a + i)?);
        }
    }
    Ok(out)
}

/// The `TitleSprites` table: `dMNTitleCommonSpriteDescs` (`x`, `y`,
/// `offset`).
pub fn title_sprites_bytes(c: &Ctx) -> Result<Vec<u8>> {
    let d = OverlayData::new(Some(c.rom), OVL10);
    let (at, n) = TITLE_SPRITE_DESCS;
    let words = (0..n as u32 * 3)
        .map(|i| d.u32(at + i * 4))
        .collect::<Result<Vec<_>>>()?;
    Ok(le_words(&words))
}

/// `rows` rows of [`TRACKS`] `MNCharactersMotion`s from `at`, each as its
/// three words.
fn motion_words(d: OverlayData, at: u32, rows: u32, out: &mut Vec<u32>) -> Result<()> {
    for i in 0..rows * TRACKS * 3 {
        out.push(d.u32(at + i * 4)?);
    }
    Ok(())
}

/// The `CharactersSpecial` table: `dMNCharactersSpecialMotion*`, by
/// `FTKind`, motion (Hi, N, Lw), track.
pub fn characters_special_bytes(c: &Ctx) -> Result<Vec<u8>> {
    let d = OverlayData::new(Some(c.rom), OVL33);
    let (at, n) = SPECIAL_MOTIONS;
    let mut words = Vec::new();
    for f in 0..n as u32 {
        motion_words(d, at + f * SPECIAL_MOTION_SIZE, 3, &mut words)?;
    }
    Ok(le_words(&words))
}

/// The `CharactersCommon` table: `dMNCharactersCommonMotionDescs`.
pub fn characters_common_bytes(c: &Ctx) -> Result<Vec<u8>> {
    let d = OverlayData::new(Some(c.rom), OVL33);
    let (at, n) = COMMON_MOTIONS;
    let mut words = Vec::new();
    motion_words(d, at, n as u32, &mut words)?;
    Ok(le_words(&words))
}

/// The `CharactersAttack1` table: `dMNCharactersAttack1MotionDescs`.
pub fn characters_attack1_bytes(c: &Ctx) -> Result<Vec<u8>> {
    let d = OverlayData::new(Some(c.rom), OVL33);
    let (at, n) = ATTACK1_MOTIONS;
    let mut words = Vec::new();
    motion_words(d, at, n as u32, &mut words)?;
    Ok(le_words(&words))
}

/// The `CharactersAerial` table: Kirby's eight jump-aerial tracks and his
/// fall track, then Purin's.
pub fn characters_aerial_bytes(c: &Ctx) -> Result<Vec<u8>> {
    let d = OverlayData::new(Some(c.rom), OVL33);
    let mut words = Vec::new();
    for (jump, fall) in [
        (KIRBY_JUMP_AERIAL, KIRBY_FALL),
        (PURIN_JUMP_AERIAL, PURIN_FALL),
    ] {
        motion_words(d, jump, 1, &mut words)?;
        for k in 0..3 {
            words.push(d.u32(fall + k * 4)?);
        }
    }
    Ok(le_words(&words))
}

/// The `PlayersDarkDigitWidths` table: the `Sprite.width` of each sprite
/// `mnPlayersVSMakeGameRuleNumber` steps by, as `f32`.
pub fn dark_digit_widths_bytes(c: &Ctx) -> Result<Vec<u8>> {
    let (at, n) = DARK_DIGIT_OFFSETS;
    let d = OverlayData::new(Some(c.rom), OVL26);
    let data = c.rom.file(PLAYERS_COMMON_FILE)?.data;
    let mut words = Vec::with_capacity(n);
    for i in 0..n as u32 {
        let offset = d.u32(at + i * 4)? as usize;
        let width = be_u16(&data, offset + SPRITE_WIDTH)?;
        words.push(f32::from(width as i16).to_bits());
    }
    Ok(le_words(&words))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_layout_tables_are_contiguous() {
        // The twelve specials end where the common table starts, and the
        // common table where Kirby's jump row starts.
        assert_eq!(
            SPECIAL_MOTIONS.0 + SPECIAL_MOTIONS.1 as u32 * SPECIAL_MOTION_SIZE,
            COMMON_MOTIONS.0
        );
        assert_eq!(
            COMMON_MOTIONS.0 + COMMON_MOTIONS.1 as u32 * TRACKS * MOTION_SIZE,
            KIRBY_JUMP_AERIAL
        );
        assert_eq!(KIRBY_JUMP_AERIAL + TRACKS * MOTION_SIZE, KIRBY_FALL);
        assert_eq!(PURIN_JUMP_AERIAL + TRACKS * MOTION_SIZE, PURIN_FALL);
    }
}
