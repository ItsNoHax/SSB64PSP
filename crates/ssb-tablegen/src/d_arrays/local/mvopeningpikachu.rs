//! `mv/mvopening/mvopeningpikachu.c`'s local arrays (see [`super`]).

use crate::d_arrays::{DArray, Elem};
use crate::source::OVL42;

pub const GAME: &[DArray] = &[
    // mv/mvopening/mvopeningpikachu.c:129.
    DArray::local(
        "MV_OPENING_PIKACHU_MAKE_NAME_OFFSETS",
        "mvOpeningPikachuMakeName offsets",
        OVL42,
        0x8018_E104,
        Elem::X32,
        &[8],
    ),
    // mv/mvopening/mvopeningpikachu.c:140.
    DArray::local(
        "MV_OPENING_PIKACHU_MAKE_NAME_POS_X",
        "mvOpeningPikachuMakeName pos_x",
        OVL42,
        0x8018_E124,
        Elem::F32,
        &[7],
    ),
];

pub const ROM: &[DArray] = &[];
