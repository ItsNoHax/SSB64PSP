//! `dKirbyMainMotion_0x0000[nFTKindEnumCount]`, Kirby's `FTKirbyCopy`
//! table (`llKirbyMainMotionSpecialNFTKirbyCopy`, `include/reloc_data.us.h`),
//! for `ssb_game::kirby` and `ssb_game::kirby_copy`: one 12-byte record per
//! swallowed `FTKind` at the start of file 228 (`KirbyMainMotion`).
//!
//! `FTKirbyCopy` (`ftkirby.h`): `u16 copy_id` at +0, `s16
//! copy_modelpart_id` at +2, `f32 effect_scale` at +4 (the Kirby stars'
//! size and spark scatter), `s32 star_damage` at +8.

use alloc::vec::Vec;

use crate::source::{be_u16, be_u32};
use crate::tables::{le_words, Ctx};
use crate::{Error, Result};

/// Archive file holding the table (`llKirbyMainMotionFileID`).
pub const FILE: u32 = 228;
/// `llKirbyMainMotionSpecialNFTKirbyCopy`.
pub const OFFSET: usize = 0x0;
/// `sizeof(FTKirbyCopy)`.
pub const SIZE: usize = 12;
/// `nFTKindEnumCount`.
pub const ROWS: usize = 27;
/// The playable kinds, whose rows give the copy hats' model parts.
pub const PLAYABLE: usize = 12;

/// `TableId::KirbyCopy`: per swallowed `FTKind`, `copy_id`,
/// `copy_modelpart_id`, `star_damage`, `effect_scale` (f32 bits).
pub fn copy_bytes(c: &Ctx) -> Result<Vec<u8>> {
    let data = c.rom.file(FILE)?.data;
    let mut out = Vec::with_capacity(ROWS * 4);
    for r in 0..ROWS {
        let at = OFFSET + SIZE * r;
        let copy_id = be_u16(&data, at)?;
        let part = be_u16(&data, at + 2)? as i16;
        let bad = |offset| Error::BadRelocation { file: FILE, offset };
        // `copy_id` is an `FTKind`; a playable row's part is a model part.
        if copy_id > 0xFF {
            return Err(bad(at));
        }
        if r < PLAYABLE && !(0..=0xFF).contains(&part) {
            return Err(bad(at + 2));
        }
        out.extend([
            copy_id.into(),
            part as i32 as u32,
            be_u32(&data, at + 8)?,
            be_u32(&data, at + 4)?,
        ]);
    }
    Ok(le_words(&out))
}
