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
use crate::tables::{le_words, Ctx};
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

/// `n` sprite infos' `offset`s from `at`.
fn offsets_bytes(c: &Ctx, (at, n): (u32, usize)) -> Result<Vec<u8>> {
    let d = OverlayData::new(Some(c.rom), OVL59);
    let words = (0..n as u32)
        .map(|i| d.u32(at + i * SPRITE_INFO_SIZE + 4))
        .collect::<Result<Vec<_>>>()?;
    Ok(le_words(&words))
}

/// The `StaffrollNameImages` table: `dSCStaffrollNameAndJobSpriteInfo`'s
/// `offset`s.
pub fn name_images_bytes(c: &Ctx) -> Result<Vec<u8>> {
    offsets_bytes(c, NAME_AND_JOB)
}

/// The `StaffrollTextSprites` table: `dSCStaffrollTextBoxSpriteInfo`'s
/// `offset`s.
pub fn text_sprites_bytes(c: &Ctx) -> Result<Vec<u8>> {
    offsets_bytes(c, TEXT_BOX)
}

/// The `EndingCongra` table: `dMNCongraPictures` (bottom file, offset, top
/// file, offset), by `FTKind`.
pub fn congra_bytes(c: &Ctx) -> Result<Vec<u8>> {
    let d = OverlayData::new(Some(c.rom), OVL57);
    let (at, n) = CONGRA_PICTURES;
    let words = (0..n as u32 * 4)
        .map(|i| d.u32(at + (i / 4) * CONGRA_PICTURE_SIZE + (i % 4) * 4))
        .collect::<Result<Vec<_>>>()?;
    Ok(le_words(&words))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stubs_have_every_row() {
        let files = generate(None).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].text.matches("    [0, 0],").count(), 56 + 74);
    }

    #[test]
    fn the_two_arrays_are_contiguous() {
        assert_eq!(
            NAME_AND_JOB.0 + NAME_AND_JOB.1 as u32 * SPRITE_INFO_SIZE,
            TEXT_BOX.0
        );
    }
}
