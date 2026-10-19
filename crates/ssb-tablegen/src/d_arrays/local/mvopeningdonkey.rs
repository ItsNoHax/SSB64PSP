//! `mv/mvopening/mvopeningdonkey.c`'s local arrays (see [`super`]).

use crate::d_arrays::{DArray, Elem};
use crate::source::OVL37;

pub const GAME: &[DArray] = &[
    // mv/mvopening/mvopeningdonkey.c:134.
    DArray::local(
        "MV_OPENING_DONKEY_MAKE_NAME_OFFSETS",
        "mvOpeningDonkeyMakeName offsets",
        OVL37,
        0x8018_E0C4,
        Elem::X32,
        &[3],
    ),
    // mv/mvopening/mvopeningdonkey.c:140.
    DArray::local(
        "MV_OPENING_DONKEY_MAKE_NAME_POS",
        "mvOpeningDonkeyMakeName pos",
        OVL37,
        0x8018_E0D0,
        Elem::F32,
        &[2, 2],
    ),
];

pub const ROM: &[DArray] = &[];
