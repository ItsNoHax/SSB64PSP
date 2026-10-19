//! `mn/mnvsmode/mnvsmode.c`'s local arrays (see [`super`]).

use crate::d_arrays::{DArray, Elem};
use crate::source::OVL19;

pub const GAME: &[DArray] = &[
    // mn/mnvsmode/mnvsmode.c:176.
    DArray::local(
        "MN_VS_MODE_MAKE_NUMBER_NUMBER_OFFSETS",
        "mnVSModeMakeNumber number_offsets",
        OVL19,
        0x8013_47F8,
        Elem::X32,
        &[10],
    ),
    // mn/mnvsmode/mnvsmode.c:235.
    DArray::local(
        "MN_VS_MODE_UPDATE_BUTTON_SELCOLORS",
        "mnVSModeUpdateButton selcolors",
        OVL19,
        0x8013_4848,
        Elem::U8,
        &[2, 3],
    ),
    // mn/mnvsmode/mnvsmode.c:238.
    DArray::local(
        "MN_VS_MODE_UPDATE_BUTTON_HICOLORS",
        "mnVSModeUpdateButton hicolors",
        OVL19,
        0x8013_4850,
        Elem::U8,
        &[2, 3],
    ),
    // mn/mnvsmode/mnvsmode.c:241.
    DArray::local(
        "MN_VS_MODE_UPDATE_BUTTON_NOTCOLORS",
        "mnVSModeUpdateButton notcolors",
        OVL19,
        0x8013_4858,
        Elem::U8,
        &[2, 3],
    ),
    // mn/mnvsmode/mnvsmode.c:340.
    DArray::local(
        "MN_VS_MODE_MAKE_RULE_VALUE_COLOR",
        "mnVSModeMakeRuleValue color",
        OVL19,
        0x8013_4860,
        Elem::U8,
        &[3],
    ),
    // mn/mnvsmode/mnvsmode.c:646.
    DArray::local(
        "MN_VS_MODE_MAKE_TIME_STOCK_VALUE_COLORS",
        "mnVSModeMakeTimeStockValue colors",
        OVL19,
        0x8013_4864,
        Elem::U32,
        &[3],
    ),
];

pub const ROM: &[DArray] = &[];
