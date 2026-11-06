//! `dCaptainMainMotion_0x0000[nFTKindEnumCount]`, the per-victim offsets
//! `ftCommonCaptureCaptainUpdatePositions` reads through
//! `llCaptainMainMotionSpecialHiVec2h` (`include/reloc_data.us.h`), for
//! `ssb_game::grab`: one `Vec2h` (two big-endian `s16`s) per `FTKind` at
//! the start of file 235 (`CaptainMainMotion`).

use alloc::vec::Vec;

use crate::source::be_u16;
use crate::tables::{le_bytes, Ctx};
use crate::Result;

/// Archive file holding the table (`llCaptainMainMotionFileID`).
pub const FILE: u32 = 235;
/// `llCaptainMainMotionSpecialHiVec2h`.
pub const OFFSET: usize = 0x0;
/// `nFTKindEnumCount`.
pub const ROWS: usize = 27;

/// `TableId::CaptureCaptain`: `(x, y)` per victim `FTKind`.
pub fn offsets_bytes(c: &Ctx) -> Result<Vec<u8>> {
    let data = c.rom.file(FILE)?.data;
    let v: Vec<i16> = (0..2 * ROWS)
        .map(|i| be_u16(&data, OFFSET + 2 * i).map(|h| h as i16))
        .collect::<Result<_>>()?;
    Ok(le_bytes(&v, i16::to_le_bytes))
}
