//! `gSYSinTable` (`sys/sintable.c`), for `ssb_engine::math`: the half-wave
//! sine table behind `syGetSinCosUShort`.
//!
//! It is hand-authored ROM data, not a formula (`round(sin(i * PI / 2048) *
//! 32768)` misses most of its entries; RE-505), so it is read from the ROM.

use crate::emit;
use crate::{Result, Source};

/// `gSYSinTable`: ROM offset (`smashbrothers.us.yaml`, `sys/sintable`) and
/// entry count (`u16[0x800]`, RAM 0x8003B950). `ssb-rom`'s audio section
/// reads the same bytes (`audio::SIN_TABLE`).
pub const SIN_TABLE: (u32, usize) = (0x3_C550, 2048);

pub fn generate(rom: Option<&Source>) -> Result<String> {
    let (at, count) = SIN_TABLE;
    let words: Vec<u16> = match rom {
        Some(rom) => (0..count as u32)
            .map(|i| rom.u16(at + i * 2))
            .collect::<Result<_>>()?,
        None => vec![0; count],
    };
    let mut out = emit::header("gSYSinTable, the half-wave sine of syGetSinCosUShort");
    out.push_str(&format!(
        "#[rustfmt::skip]\npub const SY_SIN_TABLE: [u16; {count}] = [\n"
    ));
    emit::hex_rows_u16(&mut out, &words, 12);
    out.push_str("];\n");
    Ok(out)
}
