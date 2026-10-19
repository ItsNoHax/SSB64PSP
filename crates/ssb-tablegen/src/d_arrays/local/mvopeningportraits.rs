//! `mv/mvopening/mvopeningportraits.c`'s local arrays (see [`super`]).

use crate::d_arrays::{DArray, Elem};
use crate::source::OVL35;

pub const GAME: &[DArray] = &[
    // mv/mvopening/mvopeningportraits.c:75.
    DArray::local(
        "MV_OPENING_PORTRAITS_MAKE_SET_1_OFFSETS",
        "mvOpeningPortraitsMakeSet1 offsets",
        OVL35,
        0x8013_28D8,
        Elem::X32,
        &[4],
    ),
    // mv/mvopening/mvopeningportraits.c:82.
    DArray::local(
        "MV_OPENING_PORTRAITS_MAKE_SET_1_POS",
        "mvOpeningPortraitsMakeSet1 pos",
        OVL35,
        0x8013_28E8,
        Elem::F32,
        &[4, 2],
    ),
    // mv/mvopening/mvopeningportraits.c:111.
    DArray::local(
        "MV_OPENING_PORTRAITS_MAKE_SET_2_OFFSETS",
        "mvOpeningPortraitsMakeSet2 offsets",
        OVL35,
        0x8013_2908,
        Elem::X32,
        &[4],
    ),
    // mv/mvopening/mvopeningportraits.c:118.
    DArray::local(
        "MV_OPENING_PORTRAITS_MAKE_SET_2_POS",
        "mvOpeningPortraitsMakeSet2 pos",
        OVL35,
        0x8013_2918,
        Elem::F32,
        &[4, 2],
    ),
];

pub const ROM: &[DArray] = &[
    // mv/mvopening/mvopeningportraits.c:75.
    DArray::local(
        "MV_OPENING_PORTRAITS_MAKE_SET_1_OFFSETS",
        "mvOpeningPortraitsMakeSet1 offsets",
        OVL35,
        0x8013_28D8,
        Elem::X32,
        &[4],
    ),
    // mv/mvopening/mvopeningportraits.c:111.
    DArray::local(
        "MV_OPENING_PORTRAITS_MAKE_SET_2_OFFSETS",
        "mvOpeningPortraitsMakeSet2 offsets",
        OVL35,
        0x8013_2908,
        Elem::X32,
        &[4],
    ),
];
