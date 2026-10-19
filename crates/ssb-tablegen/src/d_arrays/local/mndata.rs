//! `mn/mndata/mndata.c`'s local arrays (see [`super`]).

use crate::d_arrays::{DArray, Elem};
use crate::source::OVL61;

pub const GAME: &[DArray] = &[
    // mn/mndata/mndata.c:134.
    DArray::local(
        "MN_DATA_SET_OPTION_SPRITE_COLORS_SELCOLORS",
        "mnDataSetOptionSpriteColors selcolors",
        OVL61,
        0x8013_2F68,
        Elem::U8,
        &[2, 3],
    ),
    // mn/mndata/mndata.c:137.
    DArray::local(
        "MN_DATA_SET_OPTION_SPRITE_COLORS_HICOLORS",
        "mnDataSetOptionSpriteColors hicolors",
        OVL61,
        0x8013_2F70,
        Elem::U8,
        &[2, 3],
    ),
    // mn/mndata/mndata.c:140.
    DArray::local(
        "MN_DATA_SET_OPTION_SPRITE_COLORS_NOTCOLORS",
        "mnDataSetOptionSpriteColors notcolors",
        OVL61,
        0x8013_2F78,
        Elem::U8,
        &[2, 3],
    ),
];

pub const ROM: &[DArray] = &[];
