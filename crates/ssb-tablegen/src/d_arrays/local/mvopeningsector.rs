//! `mv/mvopening/mvopeningsector.c`'s local arrays (see [`super`]).

use crate::d_arrays::{DArray, Elem};
use crate::source::OVL50;

pub const GAME: &[DArray] = &[
    // mv/mvopening/mvopeningsector.c:352.
    DArray::local(
        "MV_OPENING_SECTOR_MAKE_ARWINGS_ANIM_JOINTS",
        "mvOpeningSectorMakeArwings anim_joints",
        OVL50,
        0x8013_2930,
        Elem::X32,
        &[3],
    ),
];

pub const ROM: &[DArray] = &[
    // mv/mvopening/mvopeningsector.c:352.
    DArray::local(
        "MV_OPENING_SECTOR_MAKE_ARWINGS_ANIM_JOINTS",
        "mvOpeningSectorMakeArwings anim_joints",
        OVL50,
        0x8013_2930,
        Elem::X32,
        &[3],
    ),
];
