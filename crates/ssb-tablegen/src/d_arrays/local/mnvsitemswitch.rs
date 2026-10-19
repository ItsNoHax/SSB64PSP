//! `mn/mnvsmode/mnvsitemswitch.c`'s local arrays (see [`super`]).

use crate::d_arrays::{DArray, Elem};
use crate::source::OVL21;

pub const GAME: &[DArray] = &[
    // mn/mnvsmode/mnvsitemswitch.c:434.
    DArray::local(
        "MN_VS_ITEM_SWITCH_MAKE_APPEARANCE_POS_X",
        "mnVSItemSwitchMakeAppearance pos_x",
        OVL21,
        0x8013_32F0,
        Elem::I32,
        &[6],
    ),
    // mn/mnvsmode/mnvsitemswitch.c:445.
    DArray::local(
        "MN_VS_ITEM_SWITCH_MAKE_APPEARANCE_RATE_OFFSETS",
        "mnVSItemSwitchMakeAppearance rate_offsets",
        OVL21,
        0x8013_3308,
        Elem::X32,
        &[6],
    ),
];

pub const ROM: &[DArray] = &[];
