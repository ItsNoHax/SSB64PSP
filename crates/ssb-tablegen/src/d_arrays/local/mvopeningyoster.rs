//! `mv/mvopening/mvopeningyoster.c`'s local arrays (see [`super`]).

use crate::d_arrays::{DArray, Elem};
use crate::source::OVL45;

pub const GAME: &[DArray] = &[
    // mv/mvopening/mvopeningyoster.c:95.
    DArray::local(
        "MV_OPENING_YOSTER_MAKE_FIGHTERS_STATUS_IDS",
        "mvOpeningYosterMakeFighters status_ids",
        OVL45,
        0x8013_2368,
        Elem::I32,
        &[4],
    ),
];

pub const ROM: &[DArray] = &[];
