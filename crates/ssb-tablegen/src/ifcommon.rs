//! The interface code's sprite tables (`d*` symbols of `if/ifcommon.c`, in
//! `ovl2`): the VS countdown's traffic light and "GO!", the announcer's
//! letters and the pause menu's decals.
//!
//! The ROM names a sprite by its file offset (`ll*Sprite`); the port names
//! it by its place in the file's sprite manifest
//! (`ssb_rom::sprite_offsets`; the announcer's letters are the generated
//! `mnVSResultsMakeString` offsets, `ssb_rom::sprite::ANNOUNCE_COMMON`), so
//! each generator looks the offset up there.

use std::fmt::Write;

use crate::d_arrays::local::mnvsresults;
use crate::emit::{self, f32_lit};
use crate::source::{OverlayData, OVL2};
use crate::sprite_offsets::{BATTLE_PAUSE, GAME_STATUS};
use crate::{Error, Generated, Result, Source};

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

pub fn generate(rom: Option<&Source>) -> Result<Vec<Generated>> {
    let d = OverlayData::new(rom, OVL2);
    Ok(vec![
        Generated {
            name: "if_countdown.rs",
            text: countdown(d)?,
        },
        Generated {
            name: "if_announce.rs",
            text: announce(d)?,
        },
        Generated {
            name: "if_pause.rs",
            text: pause(d)?,
        },
    ])
}

/// The manifest index of the sprite at file offset `offset`. A stub has no
/// offsets to look up.
fn sprite_index(d: OverlayData, manifest: &[u32], file: u32, offset: u32) -> Result<u8> {
    if d.rom.is_none() {
        return Ok(0);
    }
    manifest
        .iter()
        .position(|&o| o == offset)
        .map(|i| i as u8)
        .ok_or(Error::BadRelocation {
            file,
            offset: offset as usize,
        })
}

/// An `IFACharacter` array as `(x, y, sprite)` rows, the sprite indexing
/// `manifest`.
fn characters(
    d: OverlayData,
    (at, n): (u32, usize),
    name: &str,
    (manifest, file): (&[u32], u32),
) -> Result<String> {
    let mut w = format!("pub const {name}: [(f32, f32, u8); {n}] = [\n");
    for i in 0..n as u32 {
        let at = at + i * 8;
        let (x, y) = (d.i16(at)?, d.i16(at + 2)?);
        let sprite = sprite_index(d, manifest, file, d.u32(at + 4)?)?;
        let (x, y) = (f32_lit(f32::from(x)), f32_lit(f32::from(y)));
        let _ = writeln!(w, "    ({x}, {y}, {sprite}),");
    }
    w.push_str("];\n");
    Ok(w)
}

/// File 82's manifest with its file id.
#[allow(non_snake_case)]
fn GAME_STATUS_SPRITES() -> (&'static [u32], u32) {
    (GAME_STATUS, GAME_STATUS_FILE)
}

/// File 37's manifest: the letters `mnVSResultsMakeString` reads, which
/// `ssb_rom::sprite::ANNOUNCE_COMMON` is generated from.
fn announce_sprites(d: OverlayData) -> Result<Vec<u32>> {
    mnvsresults::MAKE_STRING_OFFSETS.words(d.rom)
}

fn hex_bytes(bytes: &[u8]) -> String {
    let cells: Vec<String> = bytes.iter().map(|v| format!("0x{v:02X}")).collect();
    cells.join(", ")
}

fn countdown(d: OverlayData) -> Result<String> {
    let mut w = emit::header(
        "`dIFCommonTrafficSpriteData`, `...Offsets`, `...Colors{R,G,B}`, `...GoBacklight*`, \
         `...GoShadow*`, `dIFCommonAnnounceGoSpriteData` and `...SuddenDeathSpriteData`",
    );
    let (at, n) = TRAFFIC_OFFSETS;
    let (manifest, file) = GAME_STATUS_SPRITES();
    let sprites = (0..n as u32)
        .map(|i| sprite_index(d, manifest, file, d.u32(at + i * 4)?).map(|s| s.to_string()))
        .collect::<Result<Vec<_>>>()?;
    let _ = writeln!(
        w,
        "pub const TRAFFIC_SPRITES: [u8; {n}] = [{}];",
        sprites.join(", ")
    );
    for (name, at) in ["TRAFFIC_R", "TRAFFIC_G", "TRAFFIC_B"]
        .iter()
        .zip(TRAFFIC_COLORS)
    {
        let bytes = d.bytes(at, TRAFFIC_COLOR_COUNT)?;
        let _ = writeln!(
            w,
            "pub const {name}: [u8; {TRAFFIC_COLOR_COUNT}] = [{}];",
            hex_bytes(&bytes)
        );
    }
    for (name, regs) in [("BACKLIGHT", GO_BACKLIGHT), ("SHADOW", GO_SHADOW)] {
        let _ = writeln!(w, "pub const {name}: [[u8; 3]; 2] = [");
        for state in 0..2 {
            let c = regs
                .iter()
                .map(|&a| d.u8(a + state))
                .collect::<Result<Vec<_>>>()?;
            let _ = writeln!(w, "    [{}],", hex_bytes(&c));
        }
        w.push_str("];\n");
    }
    let (at, n) = TRAFFIC_DATA;
    let _ = writeln!(w, "pub const TRAFFIC: [(f32, f32, u8); {n}] = [");
    for i in 0..n as u32 {
        let at = at + i * 6;
        let (x, y, color) = (d.i16(at)?, d.i16(at + 2)?, d.u8(at + 4)?);
        let (x, y) = (f32_lit(f32::from(x)), f32_lit(f32::from(y)));
        let _ = writeln!(w, "    ({x}, {y}, {color}),");
    }
    w.push_str("];\n");
    w.push_str(&characters(d, ANNOUNCE_GO, "GO", GAME_STATUS_SPRITES())?);
    let letters = announce_sprites(d)?;
    w.push_str(&characters(
        d,
        ANNOUNCE_SUDDEN_DEATH,
        "SUDDEN_DEATH",
        (&letters, ANNOUNCE_COMMON_FILE),
    )?);
    Ok(w)
}

fn announce(d: OverlayData) -> Result<String> {
    let mut w = emit::header("`dIFCommonAnnounce{TimeUp,GameSet,Complete,Failure}SpriteData`");
    let letters = announce_sprites(d)?;
    let letters = (letters.as_slice(), ANNOUNCE_COMMON_FILE);
    for (rows, name, sprites) in [
        (ANNOUNCE_TIME_UP, "TIME_UP", GAME_STATUS_SPRITES()),
        (ANNOUNCE_COMPLETE, "COMPLETE", letters),
        (ANNOUNCE_GAME_SET, "GAME_SET", GAME_STATUS_SPRITES()),
        (ANNOUNCE_FAILURE, "FAILURE", letters),
    ] {
        w.push_str(&characters(d, rows, name, sprites)?);
    }
    Ok(w)
}

/// The first twelve decals every pause shows.
const PAUSE_COMMON: usize = 12;

fn pause(d: OverlayData) -> Result<String> {
    let mut w = emit::header("`dIFCommonBattlePauseDecalsSpriteData` and `...BorderRectangle`");
    let (at, n) = PAUSE_DECALS;
    let mut rows = Vec::new();
    for i in 0..n as u32 {
        let at = at + i * 16;
        let sprite = sprite_index(d, BATTLE_PAUSE, BATTLE_PAUSE_FILE, d.u32(at)?)?;
        let (x, y) = (d.i16(at + 4)?, d.i16(at + 6)?);
        let c = d.bytes(at + 8, 6)?;
        let (prim, env) = (hex_bytes(&c[..3]), hex_bytes(&c[3..]));
        rows.push(format!(
            "    Decal {{ sprite: {sprite}, pos: ({x}, {y}), prim: [{prim}], env: [{env}] }},\n"
        ));
    }
    let (common, bonus) = rows.split_at(PAUSE_COMMON);
    let _ = write!(
        w,
        "/// The decals every pause shows (`dIFCommonBattlePauseDecalsSpriteData[..12]`).\n\
         pub const DECALS: [Decal; {}] = [\n{}];\n\n\
         /// L and RETRY, which bonus courses add (`...[12..14]`).\n\
         pub const BONUS_RETRY: [Decal; {}] = [\n{}];\n\n",
        common.len(),
        common.concat(),
        bonus.len(),
        bonus.concat()
    );
    let (at, n) = PAUSE_BORDER;
    let _ = writeln!(
        w,
        "/// `dIFCommonBattlePauseBorderRectangle`: `ulx, uly, lrx, lry`.\n\
         pub const BORDER: [[i16; 4]; {n}] = ["
    );
    for i in 0..n as u32 {
        let r = (0..4)
            .map(|k| d.i32(at + i * 16 + k * 4))
            .collect::<Result<Vec<_>>>()?;
        let _ = writeln!(w, "    [{}, {}, {}, {}],", r[0], r[1], r[2], r[3]);
    }
    w.push_str("];\n");
    Ok(w)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stub_has_every_row() {
        let files = generate(None).unwrap();
        assert_eq!(files.len(), 3);
        assert_eq!(files[0].text.matches("(0.0, 0.0, 0),").count(), 15 + 3 + 12);
        assert_eq!(
            files[1].text.matches("(0.0, 0.0, 0),").count(),
            6 + 9 + 7 + 7
        );
        assert_eq!(files[2].text.matches("Decal {").count(), PAUSE_DECALS.1);
        assert!(files[2].text.contains("pub const BONUS_RETRY: [Decal; 2]"));
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
