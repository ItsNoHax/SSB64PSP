//! `mn/mnplayers/mnplayers1pbonus.c`'s local arrays (see [`super`]).

use crate::d_arrays::{DArray, Elem};
use crate::source::OVL29;

pub const GAME: &[DArray] = &[
    // mn/mnplayers/mnplayers1pbonus.c:1039.
    DArray::local(
        "MN_PLAYERS_1P_BONUS_MAKE_BEST_TIME_COLORS_2",
        "mnPlayers1PBonusMakeBestTime colors2",
        OVL29,
        0x8013_73B0,
        Elem::U32,
        &[6],
    ),
    // mn/mnplayers/mnplayers1pbonus.c:1113.
    DArray::local(
        "MN_PLAYERS_1P_BONUS_MAKE_BEST_TASK_COUNT_COLORS_2",
        "mnPlayers1PBonusMakeBestTaskCount colors2",
        OVL29,
        0x8013_73D4,
        Elem::U32,
        &[6],
    ),
    // mn/mnplayers/mnplayers1pbonus.c:1193.
    DArray::local(
        "MN_PLAYERS_1P_BONUS_MAKE_TOTAL_TIME_COLORS_2",
        "mnPlayers1PBonusMakeTotalTime colors2",
        OVL29,
        0x8013_73F8,
        Elem::U32,
        &[6],
    ),
];

pub const ROM: &[DArray] = &[];
