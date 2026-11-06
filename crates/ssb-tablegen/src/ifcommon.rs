//! The interface code's sprite tables (`d*` symbols of `if/ifcommon.c`, in
//! `ovl2`): the VS countdown's traffic light and "GO!", the announcer's
//! letters and the pause menu's decals.
//!
//! The ROM names a sprite by its file offset (`ll*Sprite`); the port names
//! it by its place in the file's sprite manifest
//! (`ssb_rom::sprite_offsets`; the announcer's letters are the generated
//! `mnVSResultsMakeString` offsets, `ssb_rom::sprite::ANNOUNCE_COMMON`), so
//! each generator looks the offset up there.

use alloc::vec::Vec;

use crate::d_arrays::local::mnvsresults;
use crate::source::{OverlayData, OVL2};
use crate::sprite_offsets::{BATTLE_PAUSE, GAME_STATUS};
use crate::tables::{le_bytes, Ctx};
use crate::{Error, Result};

/// `dIFCommonTrafficSpriteColors{R,G,B}`: `u8[9]` each, indexed by
/// `IFTraffic::color_id`.
pub const TRAFFIC_COLORS: [u32; 3] = [0x8012_ECA8, 0x8012_ECB4, 0x8012_ECC0];
pub const TRAFFIC_COLOR_COUNT: u32 = 9;
/// `dIFCommonTrafficGoBacklight{R,G,B}` and `...GoShadow{R,G,B}`: `u8[2]`.
pub const GO_BACKLIGHT: [u32; 3] = [0x8012_ECCC, 0x8012_ECD0, 0x8012_ECD4];
pub const GO_SHADOW: [u32; 3] = [0x8012_ECD8, 0x8012_ECDC, 0x8012_ECE0];
/// `dIFCommonTrafficSpriteData`: six-byte `IFTraffic`s (`s16 x, y; u8
/// color_id`), one per lamp.
pub const TRAFFIC_DATA: (u32, usize) = (0x8012_ECE4, 15);
/// `dIFCommonTrafficSpriteOffsets`: `intptr_t[9]`, by colour id.
pub const TRAFFIC_OFFSETS: (u32, usize) = (0x8012_ED40, 9);
/// The announcer's `IFACharacter` arrays (`s16 x, y; intptr_t offset`, eight
/// bytes): `dIFCommonAnnounce{Go,SuddenDeath,TimeUp,GameSet,Failure,
/// Complete}SpriteData`.
pub const ANNOUNCE_GO: (u32, usize) = (0x8012_ED64, 3);
pub const ANNOUNCE_SUDDEN_DEATH: (u32, usize) = (0x8012_ED7C, 12);
pub const ANNOUNCE_TIME_UP: (u32, usize) = (0x8012_EDE4, 6);
pub const ANNOUNCE_GAME_SET: (u32, usize) = (0x8012_EE14, 7);
pub const ANNOUNCE_FAILURE: (u32, usize) = (0x8012_F0F4, 7);
pub const ANNOUNCE_COMPLETE: (u32, usize) = (0x8012_F134, 9);
/// `dIFCommonBattlePauseDecalsSpriteData`: 16-byte `IFPauseDecal`s
/// (`intptr_t offset; s16 x, y; u8 prim[3], env[3]`): the twelve every pause
/// shows, then the bonus courses' L and RETRY.
pub const PAUSE_DECALS: (u32, usize) = (0x8012_EFC4, 14);
/// `dIFCommonBattlePauseBorderRectangle`: 16-byte `SYRectangle`s (four `s32`).
pub const PAUSE_BORDER: (u32, usize) = (0x8012_F0A4, 5);

/// Archive files of the three manifests, for the error a missing offset
/// reports.
const GAME_STATUS_FILE: u32 = 82;
const ANNOUNCE_COMMON_FILE: u32 = 37;
const BATTLE_PAUSE_FILE: u32 = 197;

/// The manifest index of the sprite at file offset `offset`.
fn sprite_index(manifest: &[u32], file: u32, offset: u32) -> Result<u8> {
    manifest
        .iter()
        .position(|&o| o == offset)
        .map(|i| i as u8)
        .ok_or(Error::BadRelocation {
            file,
            offset: offset as usize,
        })
}

/// `ovl2`'s data.
fn ovl2<'a>(c: &Ctx<'a>) -> OverlayData<'a> {
    OverlayData::new(Some(c.rom), OVL2)
}

/// `n` halfwords as little-endian bytes.
fn le_i16(halves: &[i16]) -> Vec<u8> {
    le_bytes(halves, i16::to_le_bytes)
}

/// An `IFACharacter` array as `x, y, sprite` rows, the sprite indexing
/// `manifest`.
fn characters(
    d: OverlayData,
    (at, n): (u32, usize),
    (manifest, file): (&[u32], u32),
    out: &mut Vec<i16>,
) -> Result<()> {
    for i in 0..n as u32 {
        let at = at + i * 8;
        let (x, y) = (d.i16(at)?, d.i16(at + 2)?);
        let sprite = sprite_index(manifest, file, d.u32(at + 4)?)?;
        out.extend([x, y, i16::from(sprite)]);
    }
    Ok(())
}

/// File 37's manifest: the letters `mnVSResultsMakeString` reads, which
/// `ssb_rom::sprite::ANNOUNCE_COMMON` is generated from.
fn announce_sprites(d: OverlayData) -> Result<Vec<u32>> {
    mnvsresults::MAKE_STRING_OFFSETS.words(d.rom)
}

/// `ssb-game`'s `IfTrafficSprites` table: `dIFCommonTrafficSpriteOffsets`
/// as `GAME_STATUS` indices, by colour id.
pub fn traffic_sprites_bytes(c: &Ctx) -> Result<Vec<u8>> {
    let d = ovl2(c);
    let (at, n) = TRAFFIC_OFFSETS;
    (0..n as u32)
        .map(|i| sprite_index(GAME_STATUS, GAME_STATUS_FILE, d.u32(at + i * 4)?))
        .collect()
}

/// The `IfTrafficColors` table: `dIFCommonTrafficSpriteColorsR`, then `G`,
/// then `B`.
pub fn traffic_colors_bytes(c: &Ctx) -> Result<Vec<u8>> {
    let d = ovl2(c);
    let mut out = Vec::new();
    for at in TRAFFIC_COLORS {
        out.extend(d.bytes(at, TRAFFIC_COLOR_COUNT)?);
    }
    Ok(out)
}

/// The `IfGoColors` table: `dIFCommonTrafficGoBacklight{R,G,B}` then
/// `...GoShadow{R,G,B}`, each by state (before and at "Go") then channel.
pub fn go_colors_bytes(c: &Ctx) -> Result<Vec<u8>> {
    let d = ovl2(c);
    let mut out = Vec::new();
    for regs in [GO_BACKLIGHT, GO_SHADOW] {
        for state in 0..2 {
            for &a in &regs {
                out.push(d.u8(a + state)?);
            }
        }
    }
    Ok(out)
}

/// The `IfTraffic` table: `dIFCommonTrafficSpriteData` (`x`, `y`,
/// `color_id`).
pub fn traffic_bytes(c: &Ctx) -> Result<Vec<u8>> {
    let d = ovl2(c);
    let (at, n) = TRAFFIC_DATA;
    let mut out = Vec::new();
    for i in 0..n as u32 {
        let at = at + i * 6;
        out.extend([d.i16(at)?, d.i16(at + 2)?, i16::from(d.u8(at + 4)?)]);
    }
    Ok(le_i16(&out))
}

/// The announcer's arrays in the `IfAnnounce` table, in its order.
pub const ANNOUNCE: [(u32, usize); 6] = [
    ANNOUNCE_GO,
    ANNOUNCE_SUDDEN_DEATH,
    ANNOUNCE_TIME_UP,
    ANNOUNCE_COMPLETE,
    ANNOUNCE_GAME_SET,
    ANNOUNCE_FAILURE,
];
/// The `IfAnnounce` table's rows.
pub const ANNOUNCE_ROWS: usize = {
    let mut n = 0;
    let mut i = 0;
    while i < ANNOUNCE.len() {
        n += ANNOUNCE[i].1;
        i += 1;
    }
    n
};

/// The `IfAnnounce` table: `dIFCommonAnnounce{Go,SuddenDeath,TimeUp,
/// Complete,GameSet,Failure}SpriteData` as `x, y, sprite` rows: "GO!",
/// `TIME_UP` and `GAME_SET` index file 82's manifest, the others file 37's
/// letters.
pub fn announce_bytes(c: &Ctx) -> Result<Vec<u8>> {
    let d = ovl2(c);
    let letters = announce_sprites(d)?;
    let status = (GAME_STATUS, GAME_STATUS_FILE);
    let letters = (letters.as_slice(), ANNOUNCE_COMMON_FILE);
    let mut out = Vec::new();
    for (rows, sprites) in ANNOUNCE
        .into_iter()
        .zip([status, letters, status, letters, status, letters])
    {
        characters(d, rows, sprites, &mut out)?;
    }
    Ok(le_i16(&out))
}

/// Halfwords per `IfPauseDecals` row: `sprite` (a file 197 manifest index),
/// `x`, `y`, `prim[3]`, `env[3]`.
pub const DECAL_ROW: usize = 9;

/// The `IfPauseDecals` table: `dIFCommonBattlePauseDecalsSpriteData`
/// ([`DECAL_ROW`]).
pub fn pause_decals_bytes(c: &Ctx) -> Result<Vec<u8>> {
    let d = ovl2(c);
    let (at, n) = PAUSE_DECALS;
    let mut out = Vec::new();
    for i in 0..n as u32 {
        let at = at + i * 16;
        let sprite = sprite_index(BATTLE_PAUSE, BATTLE_PAUSE_FILE, d.u32(at)?)?;
        out.extend([i16::from(sprite), d.i16(at + 4)?, d.i16(at + 6)?]);
        out.extend(d.bytes(at + 8, 6)?.into_iter().map(i16::from));
    }
    Ok(le_i16(&out))
}

/// The `IfPauseBorder` table: `dIFCommonBattlePauseBorderRectangle` (`ulx,
/// uly, lrx, lry`).
pub fn pause_border_bytes(c: &Ctx) -> Result<Vec<u8>> {
    let d = ovl2(c);
    let (at, n) = PAUSE_BORDER;
    let mut out = Vec::new();
    for i in 0..n as u32 * 4 {
        let v = d.i32(at + i * 4)?;
        out.push(i16::try_from(v).expect("a pause border coordinate fits an s16"));
    }
    Ok(le_i16(&out))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_announcer_has_44_rows() {
        assert_eq!(ANNOUNCE_ROWS, 3 + 12 + 6 + 9 + 7 + 7);
    }

    #[test]
    fn a_sprite_missing_from_its_manifest_is_an_error() {
        // `OverlayData` with a ROM needs a real ROM; the lookup itself is
        // checked here on the manifest.
        assert_eq!(GAME_STATUS.iter().position(|&o| o == 0x21A10), Some(16));
        assert_eq!(BATTLE_PAUSE.iter().position(|&o| o == 0x18C8), Some(15));
        assert!(!GAME_STATUS.contains(&0x1234));
    }
}
