//! `ssb-engine`'s small `d*` arrays (see [`super`]).

use super::{DArray, Elem};
use crate::source::OVL0;

pub const LIST: &[DArray] = &[
    // `lb/lbcommon.c`: `sinf(index / 651.8986)` for the first quarter turn.
    DArray::new(
        "LB_COMMON_SIN_LOOKUP",
        "dLBCommonSinLookup",
        OVL0,
        0x800D_4CA0,
        Elem::F32,
        &[1024],
    ),
];
