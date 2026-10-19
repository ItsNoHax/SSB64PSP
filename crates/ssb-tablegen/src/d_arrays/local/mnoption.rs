//! `mn/mnoption/mnoption.c`'s local arrays (see [`super`]).

use crate::d_arrays::{DArray, Elem};
use crate::source::OVL60;

pub const GAME: &[DArray] = &[
    // mn/mnoption/mnoption.c:142.
    DArray::local(
        "MN_OPTION_SET_OPTION_SPRITE_COLORS_SELCOLORS",
        "mnOptionSetOptionSpriteColors selcolors",
        OVL60,
        0x8013_3668,
        Elem::U8,
        &[2, 3],
    ),
    // mn/mnoption/mnoption.c:145.
    DArray::local(
        "MN_OPTION_SET_OPTION_SPRITE_COLORS_HICOLORS",
        "mnOptionSetOptionSpriteColors hicolors",
        OVL60,
        0x8013_3670,
        Elem::U8,
        &[2, 3],
    ),
    // mn/mnoption/mnoption.c:148.
    DArray::local(
        "MN_OPTION_SET_OPTION_SPRITE_COLORS_NOTCOLORS",
        "mnOptionSetOptionSpriteColors notcolors",
        OVL60,
        0x8013_3678,
        Elem::U8,
        &[2, 3],
    ),
    // mn/mnoption/mnoption.c:609.
    DArray::local(
        "MN_OPTION_SOUND_UNDERLINE_PROC_DISPLAY_RECT",
        "mnOptionSoundUnderlineProcDisplay rect",
        OVL60,
        0x8013_36AC,
        Elem::I32,
        &[2, 4],
    ),
];

pub const ROM: &[DArray] = &[];
