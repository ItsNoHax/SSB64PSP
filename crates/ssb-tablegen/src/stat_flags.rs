//! `FTStatusDesc.sflags` (`GMStatFlags`) of the common statuses and each
//! playable fighter's special statuses, for `ssb_game::spgame::stat_flags`.

use crate::emit;
use crate::{Result, Source};

/// Bytes per `FTStatusDesc`: `mflags`, `sflags` and four callbacks.
pub const STATUS_DESC_SIZE: u32 = 0x14;

/// `dFTCommonNullStatusDescs` (statuses 0-5), directly followed by
/// `dFTCommonActionStatusDescs` (6-219): ROM offset and count.
pub const COMMON_STATUS_DESCS: (u32, usize) = (0x0A_45D8, 220);

/// `dFT<Name>SpecialStatusDescs` (`ft/ftchar/ft<name>/ft<name>status.h`):
/// constant name, ROM offset and count, from `nFTCommonStatusSpecialStart`
/// (220) on. Master Hand's (0x0A_6CAC, 33) is not carried.
pub const SPECIAL_STATUS_DESCS: [(&str, u32, usize); 13] = [
    ("MARIO", 0x0A_5708, 9),
    ("FOX", 0x0A_5A14, 26),
    ("DONKEY", 0x0A_57BC, 30),
    ("SAMUS", 0x0A_5C1C, 11),
    ("LUIGI", 0x0A_5CF8, 9),
    ("LINK", 0x0A_5DAC, 17),
    ("YOSHI", 0x0A_66F8, 14),
    ("CAPTAIN", 0x0A_657C, 19),
    ("KIRBY", 0x0A_5F00, 83),
    ("PIKACHU", 0x0A_6810, 18),
    ("PURIN", 0x0A_6978, 16),
    ("NESS", 0x0A_6AB8, 25),
    ("BOSS", 0x0A_6CAC, 33),
];

/// The status descriptors' `sflags` halfwords at `at`.
pub fn sflags(rom: &Source, at: u32, count: usize) -> Result<Vec<u16>> {
    (0..count as u32)
        .map(|i| {
            let flags = rom.u16(at + i * STATUS_DESC_SIZE + 2)?;
            // Bits 15..13 are `unused`; anything there is a misread.
            assert_eq!(flags >> 13, 0, "sflags 0x{flags:04X} at 0x{at:X} + {i}");
            Ok(flags)
        })
        .collect()
}

pub fn generate(rom: Option<&Source>) -> Result<String> {
    let mut out = emit::header("FTStatusDesc.sflags of the common and special statuses");
    let tables = std::iter::once(("COMMON", COMMON_STATUS_DESCS.0, COMMON_STATUS_DESCS.1)).chain(
        SPECIAL_STATUS_DESCS
            .iter()
            .copied()
            .filter(|(n, _, _)| *n != "BOSS"),
    );
    for (name, at, count) in tables {
        let flags = match rom {
            Some(rom) => sflags(rom, at, count)?,
            None => vec![0; count],
        };
        out.push_str(&format!(
            "#[rustfmt::skip]\npub const {name}: [u16; {count}] = [\n"
        ));
        emit::hex_rows_u16(&mut out, &flags, 12);
        out.push_str("];\n\n");
    }
    out.truncate(out.trim_end().len());
    out.push('\n');
    Ok(out)
}
