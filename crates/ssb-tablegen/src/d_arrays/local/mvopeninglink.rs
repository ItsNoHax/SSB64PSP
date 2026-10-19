//! `mv/mvopening/mvopeninglink.c`'s local arrays (see [`super`]).

use crate::d_arrays::{DArray, Elem};
use crate::source::OVL40;

pub const GAME: &[DArray] = &[
    // mv/mvopening/mvopeninglink.c:130.
    DArray::local(
        "MV_OPENING_LINK_MAKE_NAME_OFFSETS",
        "mvOpeningLinkMakeName offsets",
        OVL40,
        0x8018_E0B8,
        Elem::X32,
        &[5],
    ),
    // mv/mvopening/mvopeninglink.c:138.
    DArray::local(
        "MV_OPENING_LINK_MAKE_NAME_POS_X",
        "mvOpeningLinkMakeName pos_x",
        OVL40,
        0x8018_E0CC,
        Elem::F32,
        &[4],
    ),
];

pub const ROM: &[DArray] = &[];
