//! `lb/lbcommon.c`'s local arrays (see [`super`]).

use crate::d_arrays::{DArray, Elem};
use crate::source::OVL0;

pub const GAME: &[DArray] = &[];

pub const ROM: &[DArray] = &[
    // lb/lbcommon.c:2170.
    DArray::local(
        "LB_COMMON_GET_BITMAP_DECODE_NIBBLE_ARRAY",
        "lbCommonGetBitmapDecodeNibble array",
        OVL0,
        0x800D_5D44,
        Elem::U8,
        &[4],
    ),
];
