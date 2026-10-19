//! `mv/mvopening/mvopeningsamus.c`'s local arrays (see [`super`]).

use crate::d_arrays::{DArray, Elem};
use crate::source::OVL38;

pub const GAME: &[DArray] = &[
    // mv/mvopening/mvopeningsamus.c:131.
    DArray::local(
        "MV_OPENING_SAMUS_MAKE_NAME_OFFSETS",
        "mvOpeningSamusMakeName offsets",
        OVL38,
        0x8018_E16C,
        Elem::X32,
        &[6],
    ),
    // mv/mvopening/mvopeningsamus.c:140.
    DArray::local(
        "MV_OPENING_SAMUS_MAKE_NAME_POS_X",
        "mvOpeningSamusMakeName pos_x",
        OVL38,
        0x8018_E184,
        Elem::F32,
        &[5],
    ),
];

pub const ROM: &[DArray] = &[];
