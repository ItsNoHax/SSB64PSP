//! `mn/mnplayers/mnplayersvs.c`'s local arrays (see [`super`]).

use crate::d_arrays::{DArray, Elem};
use crate::source::OVL26;

pub const GAME: &[DArray] = &[
    // mn/mnplayers/mnplayersvs.c:208.
    DArray::local(
        "MN_PLAYERS_VS_GET_NEXT_PORTRAIT_X_PORTRAIT_POS_X",
        "mnPlayersVSGetNextPortraitX portrait_pos_x",
        OVL26,
        0x8013_B3F0,
        Elem::F32,
        &[12],
    ),
    // mn/mnplayers/mnplayersvs.c:214.
    DArray::local(
        "MN_PLAYERS_VS_GET_NEXT_PORTRAIT_X_PORTRAIT_VEL",
        "mnPlayersVSGetNextPortraitX portrait_vel",
        OVL26,
        0x8013_B420,
        Elem::F32,
        &[12],
    ),
    // mn/mnplayers/mnplayersvs.c:263.
    DArray::local(
        "MN_PLAYERS_VS_SET_PORTRAIT_WALLPAPER_POSITION_POS",
        "mnPlayersVSSetPortraitWallpaperPosition pos",
        OVL26,
        0x8013_B450,
        Elem::F32,
        &[12, 2],
    ),
    // mn/mnplayers/mnplayersvs.c:378.
    DArray::local(
        "MN_PLAYERS_VS_MAKE_PORTRAIT_SHADOW_OFFSETS",
        "mnPlayersVSMakePortraitShadow offsets",
        OVL26,
        0x8013_B534,
        Elem::X32,
        &[12],
    ),
    // mn/mnplayers/mnplayersvs.c:433.
    DArray::local(
        "MN_PLAYERS_VS_MAKE_PORTRAIT_OFFSETS",
        "mnPlayersVSMakePortrait offsets",
        OVL26,
        0x8013_B564,
        Elem::X32,
        &[12],
    ),
    // mn/mnplayers/mnplayersvs.c:492.
    DArray::local(
        "MN_PLAYERS_VS_MAKE_TEAM_SELECT_OFFSETS",
        "mnPlayersVSMakeTeamSelect offsets",
        OVL26,
        0x8013_B594,
        Elem::X32,
        &[3],
    ),
    // mn/mnplayers/mnplayersvs.c:561.
    DArray::local(
        "MN_PLAYERS_VS_UPDATE_PLAYER_KIND_SELECT_OFFSETS",
        "mnPlayersVSUpdatePlayerKindSelect offsets",
        OVL26,
        0x8013_B5A0,
        Elem::X32,
        &[3],
    ),
    // mn/mnplayers/mnplayersvs.c:590.
    DArray::local(
        "MN_PLAYERS_VS_MAKE_NAME_AND_EMBLEM_EMBLEM_OFFSETS",
        "mnPlayersVSMakeNameAndEmblem emblem_offsets",
        OVL26,
        0x8013_B60C,
        Elem::X32,
        &[12],
    ),
    // mn/mnplayers/mnplayersvs.c:599.
    DArray::local(
        "MN_PLAYERS_VS_MAKE_NAME_AND_EMBLEM_NAME_OFFSETS",
        "mnPlayersVSMakeNameAndEmblem name_offsets",
        OVL26,
        0x8013_B63C,
        Elem::X32,
        &[12],
    ),
    // mn/mnplayers/mnplayersvs.c:974.
    DArray::local(
        "MN_PLAYERS_VS_MAKE_PLAYER_KIND_OFFSETS",
        "mnPlayersVSMakePlayerKind offsets",
        OVL26,
        0x8013_B6A4,
        Elem::X32,
        &[4],
    ),
    // mn/mnplayers/mnplayersvs.c:979.
    DArray::local(
        "MN_PLAYERS_VS_MAKE_PLAYER_KIND_POS_X",
        "mnPlayersVSMakePlayerKind pos_x",
        OVL26,
        0x8013_B6B4,
        Elem::F32,
        &[4],
    ),
    // mn/mnplayers/mnplayersvs.c:1179.
    DArray::local(
        "MN_PLAYERS_VS_MAKE_GAME_RULE_NUMBER_OFFSETS",
        "mnPlayersVSMakeGameRuleNumber offsets",
        OVL26,
        0x8013_B704,
        Elem::X32,
        &[10],
    ),
    // mn/mnplayers/mnplayersvs.c:1223.
    DArray::local(
        "MN_PLAYERS_VS_MAKE_TIME_SETTING_COLORS",
        "mnPlayersVSMakeTimeSetting colors",
        OVL26,
        0x8013_B72C,
        Elem::U32,
        &[6],
    ),
    // mn/mnplayers/mnplayersvs.c:1291.
    DArray::local(
        "MN_PLAYERS_VS_MAKE_STOCK_SETTING_COLORS",
        "mnPlayersVSMakeStockSetting colors",
        OVL26,
        0x8013_B744,
        Elem::U32,
        &[6],
    ),
    // mn/mnplayers/mnplayersvs.c:1388.
    DArray::local(
        "MN_PLAYERS_VS_MAKE_LABELS_OFFSETS",
        "mnPlayersVSMakeLabels offsets",
        OVL26,
        0x8013_B75C,
        Elem::X32,
        &[2],
    ),
    // mn/mnplayers/mnplayersvs.c:1393.
    DArray::local(
        "MN_PLAYERS_VS_MAKE_LABELS_COLORS",
        "mnPlayersVSMakeLabels colors",
        OVL26,
        0x8013_B764,
        Elem::U8,
        &[2, 3],
    ),
    // mn/mnplayers/mnplayersvs.c:1710.
    DArray::local(
        "MN_PLAYERS_VS_UPDATE_CURSOR_COLORS",
        "mnPlayersVSUpdateCursor colors",
        OVL26,
        0x8013_B76C,
        Elem::U8,
        &[4, 2, 3],
    ),
    // mn/mnplayers/mnplayersvs.c:1717.
    DArray::local(
        "MN_PLAYERS_VS_UPDATE_CURSOR_NUM_OFFSETS",
        "mnPlayersVSUpdateCursor num_offsets",
        OVL26,
        0x8013_B784,
        Elem::X32,
        &[4],
    ),
    // mn/mnplayers/mnplayersvs.c:1724.
    DArray::local(
        "MN_PLAYERS_VS_UPDATE_CURSOR_CURSOR_OFFSETS",
        "mnPlayersVSUpdateCursor cursor_offsets",
        OVL26,
        0x8013_B794,
        Elem::X32,
        &[3],
    ),
    // mn/mnplayers/mnplayersvs.c:1730.
    DArray::local(
        "MN_PLAYERS_VS_UPDATE_CURSOR_POS",
        "mnPlayersVSUpdateCursor pos",
        OVL26,
        0x8013_B7A0,
        Elem::I32,
        &[3, 2],
    ),
    // mn/mnplayers/mnplayersvs.c:2747.
    DArray::local(
        "MN_PLAYERS_VS_MAKE_HANDICAP_LEVEL_VALUE_OFFSETS",
        "mnPlayersVSMakeHandicapLevelValue offsets",
        OVL26,
        0x8013_B818,
        Elem::X32,
        &[10],
    ),
    // mn/mnplayers/mnplayersvs.c:2848.
    DArray::local(
        "MN_PLAYERS_VS_UPDATE_CURSOR_GRAB_PRIORITIES_PRIORITIES",
        "mnPlayersVSUpdateCursorGrabPriorities priorities",
        OVL26,
        0x8013_B840,
        Elem::U32,
        &[4],
    ),
    // mn/mnplayers/mnplayersvs.c:2874.
    DArray::local(
        "MN_PLAYERS_VS_UPDATE_CURSOR_PLACEMENT_PRIORITIES_UNHELD_PRIORITIES",
        "mnPlayersVSUpdateCursorPlacementPriorities unheld_priorities",
        OVL26,
        0x8013_B860,
        Elem::I32,
        &[4],
    ),
    // mn/mnplayers/mnplayersvs.c:3432.
    DArray::local(
        "MN_PLAYERS_VS_UPDATE_PUCK_OFFSETS",
        "mnPlayersVSUpdatePuck offsets",
        OVL26,
        0x8013_B888,
        Elem::X32,
        &[5],
    ),
    // mn/mnplayers/mnplayersvs.c:3683.
    DArray::local(
        "MN_PLAYERS_VS_MAKE_CURSOR_POS",
        "mnPlayersVSMakeCursor pos",
        OVL26,
        0x8013_B8AC,
        Elem::F32,
        &[4, 2],
    ),
    // mn/mnplayers/mnplayersvs.c:3690.
    DArray::local(
        "MN_PLAYERS_VS_MAKE_CURSOR_PRIORITIES",
        "mnPlayersVSMakeCursor priorities",
        OVL26,
        0x8013_B8CC,
        Elem::U32,
        &[4],
    ),
    // mn/mnplayers/mnplayersvs.c:3756.
    DArray::local(
        "MN_PLAYERS_VS_MAKE_PUCK_DROPPED_PRIORITIES",
        "mnPlayersVSMakePuck dropped_priorities",
        OVL26,
        0x8013_B8EC,
        Elem::I32,
        &[4],
    ),
    // mn/mnplayers/mnplayersvs.c:3757.
    DArray::local(
        "MN_PLAYERS_VS_MAKE_PUCK_HELD_PRIORITIES",
        "mnPlayersVSMakePuck held_priorities",
        OVL26,
        0x8013_B8FC,
        Elem::I32,
        &[4],
    ),
    // mn/mnplayers/mnplayersvs.c:4542.
    DArray::local(
        "MN_PLAYERS_VS_GET_NEXT_TIME_VALUE_TIMER_VALUES",
        "mnPlayersVSGetNextTimeValue timer_values",
        OVL26,
        0x8013_B93C,
        Elem::I32,
        &[8],
    ),
    // mn/mnplayers/mnplayersvs.c:4562.
    DArray::local(
        "MN_PLAYERS_VS_GET_PREV_TIME_VALUE_TIMER_VALUES",
        "mnPlayersVSGetPrevTimeValue timer_values",
        OVL26,
        0x8013_B95C,
        Elem::I32,
        &[8],
    ),
    // mn/mnplayers/mnplayersvs.c:4638.
    DArray::local(
        "MN_PLAYERS_VS_RESET_PLAYER_DEFAULT_TEAMS",
        "mnPlayersVSResetPlayer default_teams",
        OVL26,
        0x8013_B97C,
        Elem::U8,
        &[4],
    ),
];

pub const ROM: &[DArray] = &[
    // mn/mnplayers/mnplayersvs.c:378.
    DArray::local(
        "MN_PLAYERS_VS_MAKE_PORTRAIT_SHADOW_OFFSETS",
        "mnPlayersVSMakePortraitShadow offsets",
        OVL26,
        0x8013_B534,
        Elem::X32,
        &[12],
    ),
    // mn/mnplayers/mnplayersvs.c:433.
    DArray::local(
        "MN_PLAYERS_VS_MAKE_PORTRAIT_OFFSETS",
        "mnPlayersVSMakePortrait offsets",
        OVL26,
        0x8013_B564,
        Elem::X32,
        &[12],
    ),
    // mn/mnplayers/mnplayersvs.c:590.
    DArray::local(
        "MN_PLAYERS_VS_MAKE_NAME_AND_EMBLEM_EMBLEM_OFFSETS",
        "mnPlayersVSMakeNameAndEmblem emblem_offsets",
        OVL26,
        0x8013_B60C,
        Elem::X32,
        &[12],
    ),
    // mn/mnplayers/mnplayersvs.c:903.
    DArray::local(
        "MN_PLAYERS_VS_SET_GATE_LUT_MAN_OFFSETS",
        "mnPlayersVSSetGateLUT man_offsets",
        OVL26,
        0x8013_B66C,
        Elem::X32,
        &[4],
    ),
    // mn/mnplayers/mnplayersvs.c:908.
    DArray::local(
        "MN_PLAYERS_VS_SET_GATE_LUT_COM_OFFSETS",
        "mnPlayersVSSetGateLUT com_offsets",
        OVL26,
        0x8013_B67C,
        Elem::X32,
        &[4],
    ),
    // mn/mnplayers/mnplayersvs.c:2747.
    DArray::local(
        "MN_PLAYERS_VS_MAKE_HANDICAP_LEVEL_VALUE_OFFSETS",
        "mnPlayersVSMakeHandicapLevelValue offsets",
        OVL26,
        0x8013_B818,
        Elem::X32,
        &[10],
    ),
];
