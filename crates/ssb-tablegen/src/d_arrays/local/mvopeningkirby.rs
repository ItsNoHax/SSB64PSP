//! `mv/mvopening/mvopeningkirby.c`'s local arrays (see [`super`]).

use crate::d_arrays::{DArray, Elem};
use crate::source::OVL43;

pub const GAME: &[DArray] = &[
    // mv/mvopening/mvopeningkirby.c:132.
    DArray::local(
        "MV_OPENING_KIRBY_MAKE_NAME_OFFSETS",
        "mvOpeningKirbyMakeName offsets",
        OVL43,
        0x8018_E0FC,
        Elem::X32,
        &[6],
    ),
    // mv/mvopening/mvopeningkirby.c:143.
    DArray::local(
        "MV_OPENING_KIRBY_MAKE_NAME_POS",
        "mvOpeningKirbyMakeName pos",
        OVL43,
        0x8018_E114,
        Elem::F32,
        &[5, 2],
    ),
];

pub const ROM: &[DArray] = &[];
