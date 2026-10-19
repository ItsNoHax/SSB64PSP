//! `mv/mvopening/mvopeningfox.c`'s local arrays (see [`super`]).

use crate::d_arrays::{DArray, Elem};
use crate::source::OVL39;

pub const GAME: &[DArray] = &[
    // mv/mvopening/mvopeningfox.c:136.
    DArray::local(
        "MV_OPENING_FOX_MAKE_NAME_OFFSETS",
        "mvOpeningFoxMakeName offsets",
        OVL39,
        0x8018_E0E8,
        Elem::X32,
        &[4],
    ),
    // mv/mvopening/mvopeningfox.c:143.
    DArray::local(
        "MV_OPENING_FOX_MAKE_NAME_POS",
        "mvOpeningFoxMakeName pos",
        OVL39,
        0x8018_E0F8,
        Elem::F32,
        &[3, 2],
    ),
];

pub const ROM: &[DArray] = &[];
