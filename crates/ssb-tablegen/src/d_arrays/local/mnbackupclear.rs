//! `mn/mnoption/mnbackupclear.c`'s local arrays (see [`super`]).

use crate::d_arrays::{DArray, Elem};
use crate::source::OVL53;

pub const GAME: &[DArray] = &[
    // mn/mnoption/mnbackupclear.c:260.
    DArray::local(
        "MN_BACKUP_CLEAR_UPDATE_OPTION_TAB_COLORS_NOTCOLORS",
        "mnBackupClearUpdateOptionTabColors notcolors",
        OVL53,
        0x8013_2F38,
        Elem::U8,
        &[3],
    ),
    // mn/mnoption/mnbackupclear.c:263.
    DArray::local(
        "MN_BACKUP_CLEAR_UPDATE_OPTION_TAB_COLORS_HICOLORS",
        "mnBackupClearUpdateOptionTabColors hicolors",
        OVL53,
        0x8013_2F3C,
        Elem::U8,
        &[3],
    ),
    // mn/mnoption/mnbackupclear.c:294.
    DArray::local(
        "MN_BACKUP_CLEAR_SET_OPTION_SPRITE_COLORS_OFFSETS",
        "mnBackupClearSetOptionSpriteColors offsets",
        OVL53,
        0x8013_2F58,
        Elem::X32,
        &[6],
    ),
    // mn/mnoption/mnbackupclear.c:305.
    DArray::local(
        "MN_BACKUP_CLEAR_SET_OPTION_SPRITE_COLORS_POS",
        "mnBackupClearSetOptionSpriteColors pos",
        OVL53,
        0x8013_2F70,
        Elem::F32,
        &[6, 2],
    ),
];

pub const ROM: &[DArray] = &[];
