//! `ftKirbyAttack100Effect[6]` (`ftkirby.h`), the rows
//! `ftCommonAttack100LoopKirbyUpdateEffect` reads for Kirby's rapid-jab
//! puffs, for `ssb_game::kirby`.
//!
//! The table is `llKirbyMainMotionftKirbyAttack100Effect`
//! (`include/reloc_data.us.h`) in file 228 (`KirbyMainMotion`): six rows of
//! `{ Vec3f offset; f32 rotate; f32 vel; f32 add; }`, big-endian `f32`s.

use std::fmt::Write;

use crate::emit;
use crate::source::be_u32;
use crate::{Result, Source};

/// Archive file holding the table (`KirbyMainMotion`).
pub const FILE: u32 = 228;
/// `llKirbyMainMotionftKirbyAttack100Effect`.
pub const OFFSET: usize = 0x1220;
/// Rows: `flag2` 1..5 index rows 0..4; row 5 is never read.
pub const ROWS: usize = 6;
/// `f32`s per row: offset x, y, z, rotate, vel, add.
pub const COLS: usize = 6;

pub fn generate(rom: Option<&Source>) -> Result<String> {
    let mut rows = [[0f32; COLS]; ROWS];
    if let Some(rom) = rom {
        let data = rom.file(FILE)?.data;
        for (r, row) in rows.iter_mut().enumerate() {
            for (c, v) in row.iter_mut().enumerate() {
                *v = f32::from_bits(be_u32(&data, OFFSET + 4 * (r * COLS + c))?);
            }
        }
    }
    let mut w = emit::header("Kirby's rapid-jab effect rows (`ftKirbyAttack100Effect[6]`)");
    let _ = writeln!(w, "pub const ATTACK100_EFFECT: [[f32; {COLS}]; {ROWS}] = [");
    for row in rows {
        let cells: Vec<String> = row.iter().map(|&v| emit::f32_lit(v)).collect();
        let _ = writeln!(w, "    [{}],", cells.join(", "));
    }
    w.push_str("];\n");
    Ok(w)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stub_rows_are_zero() {
        let text = generate(None).unwrap();
        assert!(text.contains("pub const ATTACK100_EFFECT: [[f32; 6]; 6]"));
        assert_eq!(
            text.matches("[0.0, 0.0, 0.0, 0.0, 0.0, 0.0],").count(),
            ROWS
        );
    }
}
