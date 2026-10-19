//! `mv/mvopening/mvopeningclash.c`'s local arrays (see [`super`]).

use crate::d_arrays::{DArray, Elem};
use crate::source::OVL49;

pub const GAME: &[DArray] = &[
    // mv/mvopening/mvopeningclash.c:86.
    DArray::local(
        "MV_OPENING_CLASH_MAKE_FIGHTERS_FKINDS",
        "mvOpeningClashMakeFighters fkinds",
        OVL49,
        0x8013_2908,
        Elem::I32,
        &[8],
    ),
];

pub const ROM: &[DArray] = &[];
