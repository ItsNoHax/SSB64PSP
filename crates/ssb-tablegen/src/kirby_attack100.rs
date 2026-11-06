//! `ftKirbyAttack100Effect[6]` (`ftkirby.h`), the rows
//! `ftCommonAttack100LoopKirbyUpdateEffect` reads for Kirby's rapid-jab
//! puffs, for `ssb_game::kirby`.
//!
//! The table is `llKirbyMainMotionftKirbyAttack100Effect`
//! (`include/reloc_data.us.h`) in file 228 (`KirbyMainMotion`): six rows of
//! `{ Vec3f offset; f32 rotate; f32 vel; f32 add; }`, big-endian `f32`s.

use alloc::vec::Vec;

use crate::source::be_u32;
use crate::tables::{le_bytes, Ctx};
use crate::Result;

/// Archive file holding the table (`KirbyMainMotion`).
pub const FILE: u32 = 228;
/// `llKirbyMainMotionftKirbyAttack100Effect`.
pub const OFFSET: usize = 0x1220;
/// Rows: `flag2` 1..5 index rows 0..4; row 5 is never read.
pub const ROWS: usize = 6;
/// `f32`s per row: offset x, y, z, rotate, vel, add.
pub const COLS: usize = 6;

/// `TableId::KirbyAttack100`: the rows' `f32`s.
pub fn effect_bytes(c: &Ctx) -> Result<Vec<u8>> {
    let data = c.rom.file(FILE)?.data;
    let v: Vec<f32> = (0..ROWS * COLS)
        .map(|i| be_u32(&data, OFFSET + 4 * i).map(f32::from_bits))
        .collect::<Result<_>>()?;
    Ok(le_bytes(&v, f32::to_le_bytes))
}
