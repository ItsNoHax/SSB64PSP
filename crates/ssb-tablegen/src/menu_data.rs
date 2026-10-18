//! Menu scene data arrays (`d*` symbols in the scene overlays): the title's
//! fire animation, colours and sprite placement (`ovl10`,
//! `mn/mncommon/mntitle.c`) and the Characters scene's motion tables
//! (`ovl33`, `mn/mndata/mncharacters.c`).
//!
//! Each table's run-time address is its symbol in
//! `refs/ssb-decomp-re/symbols/symbols_us.txt`; the overlay is the one the
//! defining file's `.data` sits in (`smashbrothers.us.yaml`).

use std::fmt::Write;

use crate::emit::{self, f32_lit};
use crate::source::{OverlayData, OVL10, OVL33};
use crate::{Generated, Result, Source};

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
const MOTION_SIZE: u32 = 12;
/// The tracks of a motion row.
const TRACKS: u32 = 8;

pub fn generate(rom: Option<&Source>) -> Result<Vec<Generated>> {
    let ovl10 = OverlayData::new(rom, OVL10);
    let ovl33 = OverlayData::new(rom, OVL33);
    Ok(vec![
        Generated {
            name: "title_data.rs",
            text: title(ovl10)?,
        },
        Generated {
            name: "characters_motion.rs",
            text: characters(ovl33)?,
        },
    ])
}

fn title(d: OverlayData) -> Result<String> {
    let mut w = emit::header(
        "`dMNTitleFireSpriteOffsets`, `dMNTitleFireColors{R,G,B}` and `dMNTitleCommonSpriteDescs`",
    );
    let (at, n) = TITLE_FIRE_OFFSETS;
    let _ = writeln!(w, "pub const FIRE: [u32; {n}] = [");
    let offsets: Vec<u32> = (0..n as u32)
        .map(|i| d.u32(at + i * 4))
        .collect::<Result<_>>()?;
    for row in offsets.chunks(8) {
        let cells: Vec<String> = row.iter().map(|o| format!("0x{o:X}")).collect();
        let _ = writeln!(w, "    {},", cells.join(", "));
    }
    w.push_str("];\n");
    let _ = writeln!(
        w,
        "pub const FIRE_COLORS: [[u8; 3]; {TITLE_FIRE_COLOR_COUNT}] = ["
    );
    for i in 0..TITLE_FIRE_COLOR_COUNT {
        let c: Vec<String> = TITLE_FIRE_COLORS
            .iter()
            .map(|&a| d.u8(a + i).map(|v| format!("0x{v:02X}")))
            .collect::<Result<_>>()?;
        let _ = writeln!(w, "    [{}],", c.join(", "));
    }
    w.push_str("];\n");
    let (at, n) = TITLE_SPRITE_DESCS;
    let _ = writeln!(w, "pub const SPRITE_DESCS: [([f32; 2], u32); {n}] = [");
    for i in 0..n as u32 {
        let at = at + i * 12;
        let (x, y, offset) = (d.i32(at)?, d.i32(at + 4)?, d.u32(at + 8)?);
        let (x, y) = (f32_lit(x as f32), f32_lit(y as f32));
        let _ = writeln!(w, "    ([{x}, {y}], 0x{offset:X}),");
    }
    w.push_str("];\n");
    Ok(w)
}

/// One `MNCharactersMotion` as a `RawTrack`.
fn raw_track(d: OverlayData, at: u32) -> Result<String> {
    Ok(format!(
        "RawTrack {{ status: 0x{:X}, length: {}, flags: 0x{:X} }}",
        d.u32(at)?,
        d.i32(at + 4)?,
        d.u32(at + 8)?
    ))
}

/// `MNCharactersMotion[8]` at `at`.
fn raw_row(d: OverlayData, at: u32, indent: &str) -> Result<String> {
    let mut w = format!("{indent}[\n");
    for t in 0..TRACKS {
        let _ = writeln!(w, "{indent}    {},", raw_track(d, at + t * MOTION_SIZE)?);
    }
    let _ = writeln!(w, "{indent}],");
    Ok(w)
}

fn characters(d: OverlayData) -> Result<String> {
    let mut w = emit::header(
        "`dMNCharactersSpecialMotion*`, `...CommonMotionDescs`, `...Kirby*` and `...Purin*` \
         motion descriptors and `...Attack1MotionDescs`",
    );
    let (at, n) = SPECIAL_MOTIONS;
    let _ = writeln!(w, "pub static SPECIAL: [[[RawTrack; 8]; 3]; {n}] = [");
    for f in 0..n as u32 {
        w.push_str("    [\n");
        for motion in 0..3 {
            let at = at + f * SPECIAL_MOTION_SIZE + motion * TRACKS * MOTION_SIZE;
            w.push_str(&raw_row(d, at, "        ")?);
        }
        w.push_str("    ],\n");
    }
    w.push_str("];\n");
    for (name, (at, n)) in [("COMMON", COMMON_MOTIONS), ("ATTACK1", ATTACK1_MOTIONS)] {
        let _ = writeln!(w, "pub static {name}: [[RawTrack; 8]; {n}] = [");
        for row in 0..n as u32 {
            w.push_str(&raw_row(d, at + row * TRACKS * MOTION_SIZE, "    ")?);
        }
        w.push_str("];\n");
    }
    for (name, at) in [
        ("KIRBY_JUMP_AERIAL", KIRBY_JUMP_AERIAL),
        ("PURIN_JUMP_AERIAL", PURIN_JUMP_AERIAL),
    ] {
        let _ = writeln!(w, "pub static {name}: [RawTrack; 8] =");
        let row = raw_row(d, at, "")?;
        let _ = writeln!(w, "{};", row.trim_end().trim_end_matches(','));
    }
    for (name, at) in [("KIRBY_FALL", KIRBY_FALL), ("PURIN_FALL", PURIN_FALL)] {
        let _ = writeln!(w, "pub const {name}: RawTrack = {};", raw_track(d, at)?);
    }
    Ok(w)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stubs_have_every_row() {
        let files = generate(None).unwrap();
        assert_eq!(files.len(), 2);
        let title = &files[0].text;
        assert_eq!(title.matches("    ([0.0, 0.0], 0x0),").count(), 10);
        assert!(title.contains("pub const FIRE: [u32; 30]"));
        let motion = &files[1].text;
        // 12 x 24 + 39 x 8 + 12 x 8 + 2 x 8 tracks, plus the two falls.
        let tracks = 12 * 24 + 39 * 8 + 12 * 8 + 2 * 8 + 2;
        assert_eq!(motion.matches("RawTrack {").count(), tracks);
    }

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
