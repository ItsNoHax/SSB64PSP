//! `mn/mnvsmode/mnvsoptions.c`'s local arrays (see [`super`]).

use crate::d_arrays::{DArray, Elem};
use crate::source::OVL20;

pub const GAME: &[DArray] = &[
    // mn/mnvsmode/mnvsoptions.c:187.
    DArray::local(
        "MN_VS_OPTIONS_MAKE_DAMAGE_DIGIT_S_OBJS_DIGIT_OFFSETS",
        "mnVSOptionsMakeDamageDigitSObjs digit_offsets",
        OVL20,
        0x8013_4708,
        Elem::X32,
        &[10],
    ),
    // mn/mnvsmode/mnvsoptions.c:254.
    DArray::local(
        "MN_VS_OPTIONS_SET_OPTION_SPRITE_COLORS_NOTCOLORS",
        "mnVSOptionsSetOptionSpriteColors notcolors",
        OVL20,
        0x8013_4758,
        Elem::U8,
        &[2, 3],
    ),
    // mn/mnvsmode/mnvsoptions.c:257.
    DArray::local(
        "MN_VS_OPTIONS_SET_OPTION_SPRITE_COLORS_HICOLORS",
        "mnVSOptionsSetOptionSpriteColors hicolors",
        OVL20,
        0x8013_4760,
        Elem::U8,
        &[2, 3],
    ),
    // mn/mnvsmode/mnvsoptions.c:443.
    DArray::local(
        "MN_VS_OPTIONS_MAKE_DAMAGE_DIGITS_COLORS",
        "mnVSOptionsMakeDamageDigits colors",
        OVL20,
        0x8013_4768,
        Elem::U32,
        &[6],
    ),
    // mn/mnvsmode/mnvsoptions.c:887.
    DArray::local(
        "MN_VS_OPTIONS_UNDERLINE_PROC_DISPLAY_HANDICAP_RECT",
        "mnVSOptionsUnderlineProcDisplay handicap_rect",
        OVL20,
        0x8013_478C,
        Elem::I32,
        &[3, 4],
    ),
    // mn/mnvsmode/mnvsoptions.c:895.
    DArray::local(
        "MN_VS_OPTIONS_UNDERLINE_PROC_DISPLAY_TEAM_ATTACK_RECT",
        "mnVSOptionsUnderlineProcDisplay team_attack_rect",
        OVL20,
        0x8013_47BC,
        Elem::I32,
        &[2, 4],
    ),
    // mn/mnvsmode/mnvsoptions.c:902.
    DArray::local(
        "MN_VS_OPTIONS_UNDERLINE_PROC_DISPLAY_STAGE_SELECT_RECT",
        "mnVSOptionsUnderlineProcDisplay stage_select_rect",
        OVL20,
        0x8013_47DC,
        Elem::I32,
        &[2, 4],
    ),
];

pub const ROM: &[DArray] = &[];
