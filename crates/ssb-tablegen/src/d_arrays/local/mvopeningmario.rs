//! `mv/mvopening/mvopeningmario.c`'s local arrays (see [`super`]).

use crate::d_arrays::{DArray, Elem};
use crate::source::OVL36;

pub const GAME: &[DArray] = &[
    // mv/mvopening/mvopeningmario.c:137.
    DArray::local(
        "MV_OPENING_MARIO_MAKE_NAME_OFFSETS",
        "mvOpeningMarioMakeName offsets",
        OVL36,
        0x8018_E0F0,
        Elem::X32,
        &[6],
    ),
    // mv/mvopening/mvopeningmario.c:148.
    DArray::local(
        "MV_OPENING_MARIO_MAKE_NAME_POS_X",
        "mvOpeningMarioMakeName pos_x",
        OVL36,
        0x8018_E108,
        Elem::F32,
        &[5],
    ),
];

pub const ROM: &[DArray] = &[];
