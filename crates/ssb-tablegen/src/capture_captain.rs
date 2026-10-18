//! `dCaptainMainMotion_0x0000[nFTKindEnumCount]`, the per-victim offsets
//! `ftCommonCaptureCaptainUpdatePositions` reads through
//! `llCaptainMainMotionSpecialHiVec2h` (`include/reloc_data.us.h`), for
//! `ssb_game::grab`: one `Vec2h` (two big-endian `s16`s) per `FTKind` at
//! the start of file 235 (`CaptainMainMotion`).

use std::fmt::Write;

use crate::source::be_u16;
use crate::{emit, Result, Source};

/// Archive file holding the table (`llCaptainMainMotionFileID`).
pub const FILE: u32 = 235;
/// `llCaptainMainMotionSpecialHiVec2h`.
pub const OFFSET: usize = 0x0;
/// `nFTKindEnumCount`.
pub const ROWS: usize = 27;

pub fn generate(rom: Option<&Source>) -> Result<String> {
    let mut rows = [[0i16; 2]; ROWS];
    if let Some(rom) = rom {
        let data = rom.file(FILE)?.data;
        for (r, row) in rows.iter_mut().enumerate() {
            for (c, v) in row.iter_mut().enumerate() {
                *v = be_u16(&data, OFFSET + 4 * r + 2 * c)? as i16;
            }
        }
    }
    let mut w =
        emit::header("Falcon Dive's per-victim capture offsets (`dCaptainMainMotion_0x0000`)");
    w.push_str(
        "/// Per victim `FTKind`: the offset from its TopN to the Captain's joint, `(x, y)`.\n",
    );
    let _ = writeln!(w, "pub const CAPTURE_OFFSETS: [(i16, i16); {ROWS}] = [");
    for [x, y] in rows {
        let _ = writeln!(w, "    ({x}, {y}),");
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
        assert!(text.contains("pub const CAPTURE_OFFSETS: [(i16, i16); 27]"));
        assert_eq!(text.matches("    (0, 0),").count(), ROWS);
    }
}
