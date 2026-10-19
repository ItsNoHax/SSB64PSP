//! `mv/mvopening/mvopeningyoshi.c`'s local arrays (see [`super`]).

use crate::d_arrays::{DArray, Elem};
use crate::source::OVL41;

pub const GAME: &[DArray] = &[
    // mv/mvopening/mvopeningyoshi.c:132.
    DArray::local(
        "MV_OPENING_YOSHI_MAKE_NAME_OFFSETS",
        "mvOpeningYoshiMakeName offsets",
        OVL41,
        0x8018_E110,
        Elem::X32,
        &[6],
    ),
    // mv/mvopening/mvopeningyoshi.c:141.
    DArray::local(
        "MV_OPENING_YOSHI_MAKE_NAME_POS_X",
        "mvOpeningYoshiMakeName pos_x",
        OVL41,
        0x8018_E128,
        Elem::F32,
        &[5],
    ),
];

pub const ROM: &[DArray] = &[];
