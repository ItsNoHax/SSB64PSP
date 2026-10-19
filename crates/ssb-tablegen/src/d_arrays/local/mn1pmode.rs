//! `mn/mn1pmode/mn1pmode.c`'s local arrays (see [`super`]).

use crate::d_arrays::{DArray, Elem};
use crate::source::OVL18;

pub const GAME: &[DArray] = &[
    // mn/mn1pmode/mn1pmode.c:104.
    DArray::local(
        "MN_1P_MODE_SET_OPTION_SPRITE_COLORS_SELCOLORS",
        "mn1PModeSetOptionSpriteColors selcolors",
        OVL18,
        0x8013_30C8,
        Elem::U8,
        &[2, 3],
    ),
    // mn/mn1pmode/mn1pmode.c:107.
    DArray::local(
        "MN_1P_MODE_SET_OPTION_SPRITE_COLORS_HICOLORS",
        "mn1PModeSetOptionSpriteColors hicolors",
        OVL18,
        0x8013_30D0,
        Elem::U8,
        &[2, 3],
    ),
    // mn/mn1pmode/mn1pmode.c:110.
    DArray::local(
        "MN_1P_MODE_SET_OPTION_SPRITE_COLORS_NOTCOLORS",
        "mn1PModeSetOptionSpriteColors notcolors",
        OVL18,
        0x8013_30D8,
        Elem::U8,
        &[2, 3],
    ),
];

pub const ROM: &[DArray] = &[];
