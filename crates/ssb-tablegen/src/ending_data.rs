//! The staff roll's data arrays (`d*` symbols in `ovl59`,
//! `sc/sccommon/scstaffroll.c`): the name/job letters and the text box
//! glyphs, each a `SCStaffrollSprite` (`u8 width, height; intptr_t offset`)
//! naming a sprite or image in `llSCStaffrollFileID`.
//!
//! Each table's run-time address is its symbol in
//! `refs/ssb-decomp-re/symbols/symbols_us.txt`; the overlay is the one the
//! defining file's `.data` sits in (`smashbrothers.us.yaml`).

use alloc::vec::Vec;

use crate::source::{OverlayData, OVL57, OVL59};
use crate::tables::{le_words, Ctx};
use crate::Result;

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

/// `n` sprite infos' `width` and `height` from `at`.
fn sizes_bytes(c: &Ctx, (at, n): (u32, usize)) -> Result<Vec<u8>> {
    let d = OverlayData::new(Some(c.rom), OVL59);
    let mut out = Vec::with_capacity(n * 2);
    for i in 0..n as u32 {
        out.extend(d.bytes(at + i * SPRITE_INFO_SIZE, 2)?);
    }
    Ok(out)
}

/// `ssb-game`'s `StaffrollNameGlyphs` table: each letter quad's half width
/// and half height, by name/job font index.
pub fn name_glyphs_bytes(c: &Ctx) -> Result<Vec<u8>> {
    sizes_bytes(c, NAME_AND_JOB)
}

/// `ssb-game`'s `StaffrollTextGlyphs` table: each text box glyph's advance
/// and height, by text font index.
pub fn text_glyphs_bytes(c: &Ctx) -> Result<Vec<u8>> {
    sizes_bytes(c, TEXT_BOX)
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
    fn the_two_arrays_are_contiguous() {
        assert_eq!(
            NAME_AND_JOB.0 + NAME_AND_JOB.1 as u32 * SPRITE_INFO_SIZE,
            TEXT_BOX.0
        );
    }
}
