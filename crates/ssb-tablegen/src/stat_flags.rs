//! `FTStatusDesc.sflags` (`GMStatFlags`) of the common statuses and each
//! playable fighter's special statuses, for `ssb_game::spgame::stat_flags`.

use alloc::vec::Vec;

use crate::tables::{le_bytes, Ctx};
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

/// `TableId::StatFlags`: [`COMMON_STATUS_DESCS`]' `sflags`, then each
/// [`SPECIAL_STATUS_DESCS`] table's but Master Hand's, in that order.
pub fn flags_bytes(c: &Ctx) -> Result<Vec<u8>> {
    let mut out = sflags(c.rom, COMMON_STATUS_DESCS.0, COMMON_STATUS_DESCS.1)?;
    for &(name, at, count) in &SPECIAL_STATUS_DESCS {
        if name != "BOSS" {
            out.extend(sflags(c.rom, at, count)?);
        }
    }
    Ok(le_bytes(&out, u16::to_le_bytes))
}

/// `TableId::StatFlags`' halfwords.
pub const FLAG_COUNT: usize = {
    let mut n = COMMON_STATUS_DESCS.1;
    let mut i = 0;
    while i < SPECIAL_STATUS_DESCS.len() - 1 {
        n += SPECIAL_STATUS_DESCS[i].2;
        i += 1;
    }
    n
};
