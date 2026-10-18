//! `dKirbyMainMotion_0x0000[nFTKindEnumCount]`, Kirby's `FTKirbyCopy`
//! table (`llKirbyMainMotionSpecialNFTKirbyCopy`, `include/reloc_data.us.h`),
//! for `ssb_game::kirby` and `ssb_game::kirby_copy`: one 12-byte record per
//! swallowed `FTKind` at the start of file 228 (`KirbyMainMotion`).
//!
//! `FTKirbyCopy` (`ftkirby.h`): `u16 copy_id` at +0, `s16
//! copy_modelpart_id` at +2, `f32 effect_scale` at +4 (presentation; not
//! emitted), `s32 star_damage` at +8.

use std::fmt::Write;

use crate::source::{be_u16, be_u32};
use crate::{emit, Result, Source};

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

pub fn generate(rom: Option<&Source>) -> Result<String> {
    // (copy_id, copy_modelpart_id, star_damage)
    let mut rows = [(0u16, 0i16, 0i32); ROWS];
    if let Some(rom) = rom {
        let data = rom.file(FILE)?.data;
        for (r, row) in rows.iter_mut().enumerate() {
            let at = OFFSET + SIZE * r;
            *row = (
                be_u16(&data, at)?,
                be_u16(&data, at + 2)? as i16,
                be_u32(&data, at + 8)? as i32,
            );
        }
    }
    let mut w = emit::header("Kirby's copy table (`dKirbyMainMotion_0x0000`)");
    w.push_str("/// Per swallowed `FTKind`: `(copy_id, star_damage)`.\n");
    let _ = writeln!(w, "pub const COPY: [(u8, i32); {ROWS}] = [");
    for (copy_id, _, star_damage) in rows {
        let copy_id = u8::try_from(copy_id).expect("FTKirbyCopy::copy_id fits an FTKind");
        let _ = writeln!(w, "    ({copy_id}, {star_damage}),");
    }
    w.push_str("];\n");
    w.push_str(
        "/// `copy_modelpart_id` of the twelve playable kinds' rows: joint 6's part\n\
         /// while Kirby holds that copy.\n",
    );
    let _ = write!(w, "pub const COPY_MODELPART_IDS: [u8; {PLAYABLE}] = [");
    for (i, (_, part, _)) in rows[..PLAYABLE].iter().enumerate() {
        let part = u8::try_from(*part).expect("a copy hat's model part is not negative");
        let _ = write!(w, "{}{part}", if i == 0 { "" } else { ", " });
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
        assert!(text.contains("pub const COPY: [(u8, i32); 27]"));
        assert_eq!(text.matches("    (0, 0),").count(), ROWS);
        assert!(text.contains(
            "pub const COPY_MODELPART_IDS: [u8; 12] = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];"
        ));
    }
}
