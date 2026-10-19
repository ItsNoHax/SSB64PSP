//! `mv/mvopening/mvopeningroom.c`'s local arrays (see [`super`]).

use crate::d_arrays::{DArray, Elem};
use crate::source::OVL34;

pub const GAME: &[DArray] = &[
    // mv/mvopening/mvopeningroom.c:628.
    DArray::local(
        "MV_OPENING_ROOM_SET_SPOTLIGHT_POSITION_TRANSLATES",
        "mvOpeningRoomSetSpotlightPosition translates",
        OVL34,
        0x8013_4A70,
        Elem::F32,
        &[12, 3],
    ),
    // mv/mvopening/mvopeningroom.c:643.
    DArray::local(
        "MV_OPENING_ROOM_SET_SPOTLIGHT_POSITION_SCALES",
        "mvOpeningRoomSetSpotlightPosition scales",
        OVL34,
        0x8013_4B00,
        Elem::F32,
        &[12, 3],
    ),
    // mv/mvopening/mvopeningroom.c:1114.
    DArray::local(
        "MV_OPENING_ROOM_GET_DROPPED_FIGHTER_KIND_FKINDS",
        "mvOpeningRoomGetDroppedFighterKind fkinds",
        OVL34,
        0x8013_4B90,
        Elem::I32,
        &[8],
    ),
    // mv/mvopening/mvopeningroom.c:1136.
    DArray::local(
        "MV_OPENING_ROOM_GET_PULLED_FIGHTER_KIND_FKINDS",
        "mvOpeningRoomGetPulledFighterKind fkinds",
        OVL34,
        0x8013_4BB0,
        Elem::I32,
        &[8],
    ),
];

pub const ROM: &[DArray] = &[];
