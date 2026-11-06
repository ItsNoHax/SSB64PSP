//! `gSYSinTable` (`sys/sintable.c`), for `ssb_engine::math`: the half-wave
//! sine table behind `syGetSinCosUShort`.
//!
//! It is hand-authored ROM data, not a formula (`round(sin(i * PI / 2048) *
//! 32768)` misses most of its entries; RE-505), so it is read from the ROM.

use crate::{Result, Source};

/// `gSYSinTable`: ROM offset (`smashbrothers.us.yaml`, `sys/sintable`) and
/// entry count (`u16[0x800]`, RAM 0x8003B950). `ssb-rom`'s audio section
/// reads the same bytes (`audio::SIN_TABLE`).
pub const SIN_TABLE: (u32, usize) = (0x3_C550, 2048);

/// The table's entries (the pack's `TABLES` section, [`crate::tables`]);
/// zeros for a stub.
pub fn values(rom: Option<&Source>) -> Result<Vec<u16>> {
    let (at, count) = SIN_TABLE;
    match rom {
        Some(rom) => (0..count as u32).map(|i| rom.u16(at + i * 2)).collect(),
        None => Ok(vec![0; count]),
    }
}
