//! `mn/mnplayers/mnplayers1ptraining.c`'s local arrays (see [`super`]).

use crate::d_arrays::{DArray, Elem};
use crate::source::OVL28;

pub const GAME: &[DArray] = &[
    // mn/mnplayers/mnplayers1ptraining.c:283.
    DArray::local(
        "MN_PLAYERS_1P_TRAINING_GET_FIGHTER_KIND_FKINDS",
        "mnPlayers1PTrainingGetFighterKind fkinds",
        OVL28,
        0x8013_8094,
        Elem::I32,
        &[12],
    ),
    // mn/mnplayers/mnplayers1ptraining.c:295.
    DArray::local(
        "MN_PLAYERS_1P_TRAINING_GET_PORTRAIT_PORTRAITS",
        "mnPlayers1PTrainingGetPortrait portraits",
        OVL28,
        0x8013_80C4,
        Elem::I32,
        &[12],
    ),
    // mn/mnplayers/mnplayers1ptraining.c:1411.
    DArray::local(
        "MN_PLAYERS_1P_TRAINING_ANNOUNCE_FIGHTER_ANNOUNCE_NAMES",
        "mnPlayers1PTrainingAnnounceFighter announce_names",
        OVL28,
        0x8013_8310,
        Elem::U16,
        &[12],
    ),
    // mn/mnplayers/mnplayers1ptraining.c:1663.
    DArray::local(
        "MN_PLAYERS_1P_TRAINING_UPDATE_CURSOR_GRAB_PRIORITIES_PRIORITIES",
        "mnPlayers1PTrainingUpdateCursorGrabPriorities priorities",
        OVL28,
        0x8013_8350,
        Elem::U32,
        &[4],
    ),
    // mn/mnplayers/mnplayers1ptraining.c:1689.
    DArray::local(
        "MN_PLAYERS_1P_TRAINING_UPDATE_CURSOR_PLACEMENT_PRIORITIES_UNHELD_PRIORITIES",
        "mnPlayers1PTrainingUpdateCursorPlacementPriorities unheld_priorities",
        OVL28,
        0x8013_8370,
        Elem::I32,
        &[4],
    ),
    // mn/mnplayers/mnplayers1ptraining.c:2400.
    DArray::local(
        "MN_PLAYERS_1P_TRAINING_MAKE_CURSOR_PRIORITIES",
        "mnPlayers1PTrainingMakeCursor priorities",
        OVL28,
        0x8013_83BC,
        Elem::U32,
        &[4],
    ),
    // mn/mnplayers/mnplayers1ptraining.c:2466.
    DArray::local(
        "MN_PLAYERS_1P_TRAINING_MAKE_PUCK_DROPPED_PRIORITIES",
        "mnPlayers1PTrainingMakePuck dropped_priorities",
        OVL28,
        0x8013_83DC,
        Elem::I32,
        &[4],
    ),
    // mn/mnplayers/mnplayers1ptraining.c:2467.
    DArray::local(
        "MN_PLAYERS_1P_TRAINING_MAKE_PUCK_HELD_PRIORITIES",
        "mnPlayers1PTrainingMakePuck held_priorities",
        OVL28,
        0x8013_83EC,
        Elem::I32,
        &[4],
    ),
];

pub const ROM: &[DArray] = &[
    // mn/mnplayers/mnplayers1ptraining.c:681.
    DArray::local(
        "MN_PLAYERS_1P_TRAINING_SET_GATE_LUT_OFFSETS",
        "mnPlayers1PTrainingSetGateLUT offsets",
        OVL28,
        0x8013_8214,
        Elem::X32,
        &[4],
    ),
];
