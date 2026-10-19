//! `mv/mvopening/mvopeningrun.c`'s local arrays (see [`super`]).

use crate::d_arrays::{DArray, Elem};
use crate::source::OVL44;

pub const GAME: &[DArray] = &[
    // mv/mvopening/mvopeningrun.c:98.
    DArray::local(
        "MV_OPENING_RUN_MAKE_FIGHTERS_FKINDS",
        "mvOpeningRunMakeFighters fkinds",
        OVL44,
        0x8013_2610,
        Elem::I32,
        &[8],
    ),
];

pub const ROM: &[DArray] = &[
    // mv/mvopening/mvopeningrun.c:111.
    DArray::local(
        "MV_OPENING_RUN_MAKE_FIGHTERS_OFFSETS",
        "mvOpeningRunMakeFighters offsets",
        OVL44,
        0x8013_2630,
        Elem::X32,
        &[8],
    ),
];
