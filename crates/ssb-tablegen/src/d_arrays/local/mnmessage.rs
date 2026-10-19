//! `mn/mncommon/mnmessage.c`'s local arrays (see [`super`]).

use crate::d_arrays::{DArray, Elem};
use crate::source::OVL22;

pub const GAME: &[DArray] = &[
    // mn/mncommon/mnmessage.c:140.
    DArray::local(
        "MN_MESSAGE_MAKE_MESSAGE_MESSAGE_POS",
        "mnMessageMakeMessage message_pos",
        OVL22,
        0x8013_2564,
        Elem::I32,
        &[7, 2],
    ),
    // mn/mncommon/mnmessage.c:287.
    DArray::local(
        "MN_MESSAGE_APPLY_UNLOCK_FKINDS",
        "mnMessageApplyUnlock fkinds",
        OVL22,
        0x8013_259C,
        Elem::U8,
        &[4],
    ),
];

pub const ROM: &[DArray] = &[];
