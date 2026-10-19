//! `mn/mnplayers/mnplayers1pgame.c`'s local arrays (see [`super`]).

use crate::d_arrays::{DArray, Elem};
use crate::source::OVL27;

pub const GAME: &[DArray] = &[
    // mn/mnplayers/mnplayers1pgame.c:205.
    DArray::local(
        "MN_PLAYERS_1P_GAME_MAKE_NUMBER_OFFSETS",
        "mnPlayers1PGameMakeNumber offsets",
        OVL27,
        0x8013_8690,
        Elem::X32,
        &[10],
    ),
    // mn/mnplayers/mnplayers1pgame.c:337.
    DArray::local(
        "MN_PLAYERS_1P_GAME_MAKE_STRING_OFFSETS",
        "mnPlayers1PGameMakeString offsets",
        OVL27,
        0x8013_86B8,
        Elem::X32,
        &[29],
    ),
    // mn/mnplayers/mnplayers1pgame.c:357.
    DArray::local(
        "MN_PLAYERS_1P_GAME_MAKE_STRING_WIDTHS",
        "mnPlayers1PGameMakeString widths",
        OVL27,
        0x8013_872C,
        Elem::F32,
        &[29],
    ),
    // mn/mnplayers/mnplayers1pgame.c:890.
    DArray::local(
        "MN_PLAYERS_1P_GAME_MAKE_GATE_POS_X",
        "mnPlayers1PGameMakeGate pos_x",
        OVL27,
        0x8013_8A24,
        Elem::F32,
        &[4],
    ),
    // mn/mnplayers/mnplayers1pgame.c:954.
    DArray::local(
        "MN_PLAYERS_1P_GAME_MAKE_TIME_NUMBER_WIDTHS",
        "mnPlayers1PGameMakeTimeNumber widths",
        OVL27,
        0x8013_8A5C,
        Elem::F32,
        &[10],
    ),
    // mn/mnplayers/mnplayers1pgame.c:990.
    DArray::local(
        "MN_PLAYERS_1P_GAME_MAKE_TIME_SETTING_COLORS",
        "mnPlayers1PGameMakeTimeSetting colors",
        OVL27,
        0x8013_8A84,
        Elem::U32,
        &[6],
    ),
    // mn/mnplayers/mnplayers1pgame.c:1197.
    DArray::local(
        "MN_PLAYERS_1P_GAME_MAKE_LEVEL_OFFSETS",
        "mnPlayers1PGameMakeLevel offsets",
        OVL27,
        0x8013_8A9C,
        Elem::X32,
        &[5],
    ),
    // mn/mnplayers/mnplayers1pgame.c:1205.
    DArray::local(
        "MN_PLAYERS_1P_GAME_MAKE_LEVEL_POS",
        "mnPlayers1PGameMakeLevel pos",
        OVL27,
        0x8013_8AB0,
        Elem::F32,
        &[5, 2],
    ),
    // mn/mnplayers/mnplayers1pgame.c:1213.
    DArray::local(
        "MN_PLAYERS_1P_GAME_MAKE_LEVEL_COLORS",
        "mnPlayers1PGameMakeLevel colors",
        OVL27,
        0x8013_8AD8,
        Elem::U8,
        &[5, 3],
    ),
    // mn/mnplayers/mnplayers1pgame.c:1475.
    DArray::local(
        "MN_PLAYERS_1P_GAME_MAKE_HI_SCORE_TEXT_COLORS",
        "mnPlayers1PGameMakeHiScore text_colors",
        OVL27,
        0x8013_8AE8,
        Elem::U32,
        &[3],
    ),
    // mn/mnplayers/mnplayers1pgame.c:1476.
    DArray::local(
        "MN_PLAYERS_1P_GAME_MAKE_HI_SCORE_NUMBER_COLORS",
        "mnPlayers1PGameMakeHiScore number_colors",
        OVL27,
        0x8013_8AF4,
        Elem::U32,
        &[6],
    ),
    // mn/mnplayers/mnplayers1pgame.c:1477.
    DArray::local(
        "MN_PLAYERS_1P_GAME_MAKE_HI_SCORE_DIFFICULTY_COLORS",
        "mnPlayers1PGameMakeHiScore difficulty_colors",
        OVL27,
        0x8013_8B0C,
        Elem::U8,
        &[5, 3],
    ),
    // mn/mnplayers/mnplayers1pgame.c:1530.
    DArray::local(
        "MN_PLAYERS_1P_GAME_MAKE_BONUS_COUNT_COLORS",
        "mnPlayers1PGameMakeBonusCount colors",
        OVL27,
        0x8013_8B1C,
        Elem::U32,
        &[6],
    ),
    // mn/mnplayers/mnplayers1pgame.c:1590.
    DArray::local(
        "MN_PLAYERS_1P_GAME_MAKE_TOTAL_HI_SCORE_TEXT_COLORS",
        "mnPlayers1PGameMakeTotalHiScore text_colors",
        OVL27,
        0x8013_8B34,
        Elem::U32,
        &[3],
    ),
    // mn/mnplayers/mnplayers1pgame.c:1591.
    DArray::local(
        "MN_PLAYERS_1P_GAME_MAKE_TOTAL_HI_SCORE_NUMBER_COLORS",
        "mnPlayers1PGameMakeTotalHiScore number_colors",
        OVL27,
        0x8013_8B40,
        Elem::U32,
        &[6],
    ),
    // mn/mnplayers/mnplayers1pgame.c:1618.
    DArray::local(
        "MN_PLAYERS_1P_GAME_MAKE_TOTAL_BONUS_COUNT_COLORS",
        "mnPlayers1PGameMakeTotalBonusCount colors",
        OVL27,
        0x8013_8B58,
        Elem::U32,
        &[6],
    ),
];

pub const ROM: &[DArray] = &[
    // mn/mnplayers/mnplayers1pgame.c:337.
    DArray::local(
        "MN_PLAYERS_1P_GAME_MAKE_STRING_OFFSETS",
        "mnPlayers1PGameMakeString offsets",
        OVL27,
        0x8013_86B8,
        Elem::X32,
        &[29],
    ),
    // mn/mnplayers/mnplayers1pgame.c:869.
    DArray::local(
        "MN_PLAYERS_1P_GAME_SET_GATE_LUT_OFFSETS",
        "mnPlayers1PGameSetGateLUT offsets",
        OVL27,
        0x8013_8A04,
        Elem::X32,
        &[4],
    ),
    // mn/mnplayers/mnplayers1pgame.c:1197.
    DArray::local(
        "MN_PLAYERS_1P_GAME_MAKE_LEVEL_OFFSETS",
        "mnPlayers1PGameMakeLevel offsets",
        OVL27,
        0x8013_8A9C,
        Elem::X32,
        &[5],
    ),
];
