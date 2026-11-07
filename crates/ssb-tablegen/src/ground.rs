//! `MPGroundData::alt_warning` of every ground, for
//! `ssb_game::stage::alt_warning`.

use alloc::vec::Vec;

use crate::d_arrays::rom_words;
use crate::source::be_u16;
use crate::tables::{le_bytes, Ctx};
use crate::Result;

/// `GRKind`'s grounds: the rows of `dMPCollisionGroundFileInfos`
/// (`mp/mpcollision.c`), each ground's `ll*MapFileID` and `ll*MapMapHeader`,
/// read from the ROM (`MP_COLLISION_GROUND_FILE_INFOS_*`, D-048).
pub const GROUNDS: usize = 41;

/// `MPGroundData::alt_warning`'s offset: after `bgm_id` (0x7C),
/// `map_nodes` and `item_weights`.
pub const ALT_WARNING_OFFSET: usize = 0x88;

/// `TableId::AltWarning`: `MPGroundData::alt_warning` by `GRKind`.
pub fn alt_warning_bytes(c: &Ctx) -> Result<Vec<u8>> {
    let files = rom_words("MP_COLLISION_GROUND_FILE_INFOS_FILE_ID", c.rom)?;
    let headers = rom_words("MP_COLLISION_GROUND_FILE_INFOS_OFFSET", c.rom)?;
    assert_eq!(files.len(), GROUNDS);
    let mut alts = Vec::with_capacity(GROUNDS);
    for (file, header) in files.into_iter().zip(headers) {
        let f = c.rom.file(file)?;
        alts.push(be_u16(&f.data, header as usize + ALT_WARNING_OFFSET)? as i16);
    }
    Ok(le_bytes(&alts, i16::to_le_bytes))
}
