//! The staff roll's data arrays (`d*` symbols in `ovl59`,
//! `sc/sccommon/scstaffroll.c`): the name/job letters and the text box
//! glyphs, each a `SCStaffrollSprite` (`u8 width, height; intptr_t offset`)
//! naming a sprite or image in `llSCStaffrollFileID`.
//!
//! Each table's run-time address is its symbol in
//! `refs/ssb-decomp-re/symbols/symbols_us.txt`; the overlay is the one the
//! defining file's `.data` sits in (`smashbrothers.us.yaml`).

use std::fmt::Write;

use crate::emit;
use crate::source::{OverlayData, OVL57, OVL59};
use crate::{Generated, Result, Source};

/// `dSCStaffrollNameAndJobSpriteInfo`, by name/job font index.
pub const NAME_AND_JOB: (u32, usize) = (0x8013_A188, 56);
/// `dSCStaffrollTextBoxSpriteInfo`, by text font index.
pub const TEXT_BOX: (u32, usize) = (0x8013_A348, 74);
const SPRITE_INFO_SIZE: u32 = 8;
/// `dMNCongraPictures` (`MNCongraPicture[12]`, 16 bytes: `u32 bottom_file_id;
/// intptr_t bottom_offset; u32 top_file_id; intptr_t top_offset`), by
/// `FTKind` Mario to Ness (`ovl57`, `mn/mncommon/mncongra.c`).
pub const CONGRA_PICTURES: (u32, usize) = (0x8013_2100, 12);
const CONGRA_PICTURE_SIZE: u32 = 16;

/// `ssb-game`'s tables: each letter's width and height.
pub fn generate(rom: Option<&Source>) -> Result<Vec<Generated>> {
    let d = OverlayData::new(rom, OVL59);
    let mut w = emit::header(
        "`dSCStaffrollNameAndJobSpriteInfo` and `dSCStaffrollTextBoxSpriteInfo`'s `width` and `height`",
    );
    for (name, (at, n), doc) in [
        (
            "NAME_GLYPHS",
            NAME_AND_JOB,
            "each letter quad's half width and half height, by name/job font index",
        ),
        (
            "TEXT_GLYPHS",
            TEXT_BOX,
            "each text box glyph's advance and height, by text font index",
        ),
    ] {
        let _ = writeln!(w, "/// `{name}`: {doc}.");
        let _ = writeln!(w, "pub const {name}: [[u8; 2]; {n}] = [");
        for i in 0..n as u32 {
            let b = d.bytes(at + i * SPRITE_INFO_SIZE, 2)?;
            let _ = writeln!(w, "    [{}, {}],", b[0], b[1]);
        }
        w.push_str("];\n");
    }
    Ok(vec![Generated {
        name: "staffroll_glyphs.rs",
        text: w,
    }])
}

/// `ssb-rom`'s tables from the same arrays: the images' and sprites'
/// offsets in the staff roll's file.
pub fn generate_rom(rom: Option<&Source>) -> Result<Vec<Generated>> {
    let d = OverlayData::new(rom, OVL59);
    let mut w = emit::header(
        "`dSCStaffrollNameAndJobSpriteInfo` and `dSCStaffrollTextBoxSpriteInfo`'s `offset`s",
    );
    for (name, (at, n)) in [("NAME_IMAGES", NAME_AND_JOB), ("TEXT_SPRITES", TEXT_BOX)] {
        let offsets = (0..n as u32)
            .map(|i| d.u32(at + i * SPRITE_INFO_SIZE + 4))
            .collect::<Result<Vec<_>>>()?;
        let _ = writeln!(w, "pub const {name}: [u32; {n}] = [");
        emit::hex_rows_u32(&mut w, &offsets);
        w.push_str("];\n");
    }
    Ok(vec![
        Generated {
            name: "ending_staffroll_offsets.rs",
            text: w,
        },
        congra(rom)?,
    ])
}

/// `dMNCongraPictures`: the files by kind (`CONGRA_FILES`) and as sprite
/// files, bottom then top for each kind (`CONGRA`).
fn congra(rom: Option<&Source>) -> Result<Generated> {
    let d = OverlayData::new(rom, OVL57);
    let (at, n) = CONGRA_PICTURES;
    let mut files = String::new();
    let mut sprites = String::new();
    for i in 0..n as u32 {
        let at = at + i * CONGRA_PICTURE_SIZE;
        let (bottom, bottom_offset, top, top_offset) =
            (d.u32(at)?, d.u32(at + 4)?, d.u32(at + 8)?, d.u32(at + 12)?);
        let _ = writeln!(files, "    [0x{bottom:X}, 0x{top:X}],");
        for (file, offset) in [(bottom, bottom_offset), (top, top_offset)] {
            let _ = writeln!(
                sprites,
                "    SpriteFile {{\n        file: 0x{file:X},\n        offsets: &[0x{offset:X}],\n    }},"
            );
        }
    }
    let mut w = emit::header("`dMNCongraPictures`");
    let _ = writeln!(w, "pub const CONGRA_FILES: [[u32; 2]; {n}] = [\n{files}];");
    let _ = writeln!(
        w,
        "pub const CONGRA: [SpriteFile; {}] = [\n{sprites}];",
        n * 2
    );
    Ok(Generated {
        name: "ending_congra.rs",
        text: w,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stubs_have_every_row() {
        let files = generate(None).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].text.matches("    [0, 0],").count(), 56 + 74);
        let rom = generate_rom(None).unwrap();
        assert!(rom[0].text.contains("NAME_IMAGES: [u32; 56]"));
        assert!(rom[0].text.contains("TEXT_SPRITES: [u32; 74]"));
    }

    #[test]
    fn the_two_arrays_are_contiguous() {
        assert_eq!(
            NAME_AND_JOB.0 + NAME_AND_JOB.1 as u32 * SPRITE_INFO_SIZE,
            TEXT_BOX.0
        );
    }
}
