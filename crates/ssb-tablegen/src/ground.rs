//! `MPGroundData::alt_warning` of every ground, for
//! `ssb_game::stage::alt_warning`.

use crate::source::be_u16;
use crate::{emit, Result, Source};

/// `dMPCollisionGroundFileInfos` (`mp/mpcollision.c`), in `GRKind` order:
/// each ground's `ll*MapFileID` and `ll*MapMapHeader`
/// (`include/reloc_data.us.h`).
#[rustfmt::skip]
pub const GROUND_FILE_INFOS: [(u32, u32); 41] = [
    (0x103, 0x14), (0x106, 0x14), (0x105, 0x14), (0x101, 0x14), (0x109, 0x14),
    (0x107, 0x14), (0x0FF, 0x14), (0x108, 0x14), (0x104, 0x14), // VS
    (0x100, 0x14), // PupupuSmall
    (0x102, 0x14), // PupupuTest
    (0x10B, 0x00), // Explain
    (0x10E, 0x14), // YosterSmall
    (0x10D, 0x14), // Metal
    (0x10C, 0x14), // Zako
    (0x127, 0x00), // Bonus3
    (0x10A, 0x00), // Last
    // Bonus1 (Break the Targets), Mario..Ness.
    (0x10F, 0), (0x110, 0), (0x111, 0), (0x112, 0), (0x113, 0), (0x114, 0),
    (0x115, 0), (0x116, 0), (0x117, 0), (0x118, 0), (0x119, 0), (0x11A, 0),
    // Bonus2 (Board the Platforms), Mario..Ness.
    (0x11B, 0), (0x11C, 0), (0x11D, 0), (0x11E, 0), (0x11F, 0), (0x120, 0),
    (0x121, 0), (0x122, 0), (0x123, 0), (0x124, 0), (0x125, 0), (0x126, 0),
];

/// `MPGroundData::alt_warning`'s offset: after `bgm_id` (0x7C),
/// `map_nodes` and `item_weights`.
pub const ALT_WARNING_OFFSET: usize = 0x88;

pub fn generate(rom: Option<&Source>) -> Result<String> {
    let mut out = emit::header("MPGroundData.alt_warning by GRKind");
    let mut alts = Vec::with_capacity(GROUND_FILE_INFOS.len());
    for (file, header) in GROUND_FILE_INFOS {
        alts.push(match rom {
            Some(rom) => {
                let f = rom.file(file)?;
                be_u16(&f.data, header as usize + ALT_WARNING_OFFSET)? as i16
            }
            // Stub tables: a height no fighter crosses.
            None => i16::MIN,
        });
    }
    let rows: Vec<String> = alts.iter().map(i16::to_string).collect();
    out.push_str(&format!(
        "#[rustfmt::skip]\npub static ALT_WARNING: [i16; {}] = [{}];\n",
        alts.len(),
        rows.join(", ")
    ));
    Ok(out)
}
