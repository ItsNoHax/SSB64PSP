//! `mn/mnvsmode/mnvsresults.c`'s local arrays (see [`super`]).

use crate::d_arrays::{DArray, Elem};
use crate::source::OVL31;

pub const GAME: &[DArray] = &[
    // mn/mnvsmode/mnvsresults.c:286.
    DArray::local(
        "MN_VS_RESULTS_ANNOUNCE_WINNER_ANNOUNCE_NAMES",
        "mnVSResultsAnnounceWinner announce_names",
        OVL31,
        0x8013_8FA8,
        Elem::U32,
        &[12],
    ),
    // mn/mnvsmode/mnvsresults.c:301.
    DArray::local(
        "MN_VS_RESULTS_ANNOUNCE_WINNER_ANNOUNCER_TEAMS",
        "mnVSResultsAnnounceWinner announcer_teams",
        OVL31,
        0x8013_8FD8,
        Elem::U32,
        &[3],
    ),
    // mn/mnvsmode/mnvsresults.c:649.
    DArray::local(
        "MN_VS_RESULTS_MAKE_EMBLEM_COLORS",
        "mnVSResultsMakeEmblem colors",
        OVL31,
        0x8013_9074,
        Elem::I32,
        &[3],
    ),
    // mn/mnvsmode/mnvsresults.c:716.
    DArray::local(
        "MN_VS_RESULTS_MAKE_WALLPAPER_TEAM_COLORS",
        "mnVSResultsMakeWallpaper team_colors",
        OVL31,
        0x8013_90A8,
        Elem::I32,
        &[3],
    ),
    // mn/mnvsmode/mnvsresults.c:718.
    DArray::local(
        "MN_VS_RESULTS_MAKE_WALLPAPER_COLORS",
        "mnVSResultsMakeWallpaper colors",
        OVL31,
        0x8013_90B4,
        Elem::U8,
        &[4, 2, 3],
    ),
    // mn/mnvsmode/mnvsresults.c:805.
    DArray::local(
        "MN_VS_RESULTS_SET_FIGHTER_POSITION_POS_X_2P",
        "mnVSResultsSetFighterPosition pos_x_2p",
        OVL31,
        0x8013_90CC,
        Elem::F32,
        &[2, 4],
    ),
    // mn/mnvsmode/mnvsresults.c:810.
    DArray::local(
        "MN_VS_RESULTS_SET_FIGHTER_POSITION_POS_X_3P",
        "mnVSResultsSetFighterPosition pos_x_3p",
        OVL31,
        0x8013_90EC,
        Elem::F32,
        &[3, 4],
    ),
    // mn/mnvsmode/mnvsresults.c:816.
    DArray::local(
        "MN_VS_RESULTS_SET_FIGHTER_POSITION_POS_X_4P",
        "mnVSResultsSetFighterPosition pos_x_4p",
        OVL31,
        0x8013_911C,
        Elem::F32,
        &[4, 4],
    ),
    // mn/mnvsmode/mnvsresults.c:823.
    DArray::local(
        "MN_VS_RESULTS_SET_FIGHTER_POSITION_POS_YZ",
        "mnVSResultsSetFighterPosition pos_yz",
        OVL31,
        0x8013_915C,
        Elem::F32,
        &[4, 2],
    ),
    // mn/mnvsmode/mnvsresults.c:871.
    DArray::local(
        "MN_VS_RESULTS_GET_STATUS_WIN_STATUS_IDS",
        "mnVSResultsGetStatusWin status_ids",
        OVL31,
        0x8013_917C,
        Elem::I32,
        &[3],
    ),
    // mn/mnvsmode/mnvsresults.c:967.
    DArray::local(
        "MN_VS_RESULTS_GET_SPOT_AHEADS",
        "mnVSResultsGetSpot aheads",
        OVL31,
        0x8013_9188,
        Elem::I32,
        &[4],
    ),
    // mn/mnvsmode/mnvsresults.c:968.
    DArray::local(
        "MN_VS_RESULTS_GET_SPOT_PLACES",
        "mnVSResultsGetSpot places",
        OVL31,
        0x8013_9198,
        Elem::I32,
        &[5],
    ),
    // mn/mnvsmode/mnvsresults.c:999.
    DArray::local(
        "MN_VS_RESULTS_SET_PLAYER_TAG_POSITION_POS_XY_2P",
        "mnVSResultsSetPlayerTagPosition pos_xy_2p",
        OVL31,
        0x8013_91AC,
        Elem::F32,
        &[2, 4, 2],
    ),
    // mn/mnvsmode/mnvsresults.c:1004.
    DArray::local(
        "MN_VS_RESULTS_SET_PLAYER_TAG_POSITION_POS_XY_3P",
        "mnVSResultsSetPlayerTagPosition pos_xy_3p",
        OVL31,
        0x8013_91EC,
        Elem::F32,
        &[3, 4, 2],
    ),
    // mn/mnvsmode/mnvsresults.c:1010.
    DArray::local(
        "MN_VS_RESULTS_SET_PLAYER_TAG_POSITION_POS_XY_4P",
        "mnVSResultsSetPlayerTagPosition pos_xy_4p",
        OVL31,
        0x8013_924C,
        Elem::F32,
        &[4, 4, 2],
    ),
    // mn/mnvsmode/mnvsresults.c:1017.
    DArray::local(
        "MN_VS_RESULTS_SET_PLAYER_TAG_POSITION_POS_Y_KINDS",
        "mnVSResultsSetPlayerTagPosition pos_y_kinds",
        OVL31,
        0x8013_92CC,
        Elem::F32,
        &[12, 4],
    ),
    // mn/mnvsmode/mnvsresults.c:1150.
    DArray::local(
        "MN_VS_RESULTS_MAKE_STRING_WIDTHS",
        "mnVSResultsMakeString widths",
        OVL31,
        0x8013_93B4,
        Elem::F32,
        &[28],
    ),
    // mn/mnvsmode/mnvsresults.c:1174.
    DArray::local(
        "MN_VS_RESULTS_MAKE_STRING_COLORS",
        "mnVSResultsMakeString colors",
        OVL31,
        0x8013_9494,
        Elem::U8,
        &[5, 2, 3],
    ),
    // mn/mnvsmode/mnvsresults.c:1234.
    DArray::local(
        "MN_VS_RESULTS_MAKE_WINNER_TEXT_X_FKINDS",
        "mnVSResultsMakeWinnerText x_fkinds",
        OVL31,
        0x8013_94B4,
        Elem::F32,
        &[12],
    ),
    // mn/mnvsmode/mnvsresults.c:1264.
    DArray::local(
        "MN_VS_RESULTS_MAKE_WINNER_TEXT_X_TEAMS",
        "mnVSResultsMakeWinnerText x_teams",
        OVL31,
        0x8013_94E4,
        Elem::F32,
        &[3],
    ),
    // mn/mnvsmode/mnvsresults.c:1296.
    DArray::local(
        "MN_VS_RESULT_MAKE_FIGHTER_NAME_NAMES",
        "mnVSResultMakeFighterName names",
        OVL31,
        0x8013_94F0,
        Elem::CStr,
        &[12],
    ),
    // mn/mnvsmode/mnvsresults.c:1319.
    DArray::local(
        "MN_VS_RESULT_MAKE_FIGHTER_NAME_POS_X",
        "mnVSResultMakeFighterName pos_x",
        OVL31,
        0x8013_9520,
        Elem::F32,
        &[12],
    ),
    // mn/mnvsmode/mnvsresults.c:1349.
    DArray::local(
        "MN_VS_RESULT_MAKE_FIGHTER_NAME_SCALES",
        "mnVSResultMakeFighterName scales",
        OVL31,
        0x8013_9550,
        Elem::F32,
        &[12],
    ),
    // mn/mnvsmode/mnvsresults.c:1380.
    DArray::local(
        "MN_VS_RESULT_MAKE_TEAM_NAME_NAMES",
        "mnVSResultMakeTeamName names",
        OVL31,
        0x8013_9580,
        Elem::CStr,
        &[9],
    ),
    // mn/mnvsmode/mnvsresults.c:1392.
    DArray::local(
        "MN_VS_RESULT_MAKE_TEAM_NAME_POS_X",
        "mnVSResultMakeTeamName pos_x",
        OVL31,
        0x8013_95A4,
        Elem::F32,
        &[9],
    ),
    // mn/mnvsmode/mnvsresults.c:1489.
    DArray::local(
        "MN_VS_RESULTS_SET_NUMBER_COLOR_COLORS",
        "mnVSResultsSetNumberColor colors",
        OVL31,
        0x8013_95C8,
        Elem::U8,
        &[5, 2, 3],
    ),
    // mn/mnvsmode/mnvsresults.c:1809.
    DArray::local(
        "MN_VS_RESULTS_GET_COLUMN_X_COLUMN_X_2P",
        "mnVSResultsGetColumnX column_x_2p",
        OVL31,
        0x8013_9678,
        Elem::F32,
        &[2],
    ),
    // mn/mnvsmode/mnvsresults.c:1810.
    DArray::local(
        "MN_VS_RESULTS_GET_COLUMN_X_COLUMN_X_3P",
        "mnVSResultsGetColumnX column_x_3p",
        OVL31,
        0x8013_9680,
        Elem::F32,
        &[3],
    ),
    // mn/mnvsmode/mnvsresults.c:1811.
    DArray::local(
        "MN_VS_RESULTS_GET_COLUMN_X_COLUMN_X_4P",
        "mnVSResultsGetColumnX column_x_4p",
        OVL31,
        0x8013_968C,
        Elem::F32,
        &[4],
    ),
    // mn/mnvsmode/mnvsresults.c:1830.
    DArray::local(
        "MN_VS_RESULTS_GET_NUMBER_COLOR_ID_COLOR_IDS",
        "mnVSResultsGetNumberColorID color_ids",
        OVL31,
        0x8013_969C,
        Elem::I32,
        &[3],
    ),
    // mn/mnvsmode/mnvsresults.c:3212.
    DArray::local(
        "MN_VS_RESULTS_MAKE_CONFETTI_POS_1",
        "mnVSResultsMakeConfetti pos1",
        OVL31,
        0x8013_96F8,
        Elem::F32,
        &[3],
    ),
    // mn/mnvsmode/mnvsresults.c:3213.
    DArray::local(
        "MN_VS_RESULTS_MAKE_CONFETTI_POS_0",
        "mnVSResultsMakeConfetti pos0",
        OVL31,
        0x8013_9704,
        Elem::F32,
        &[3],
    ),
];

/// `effect::EMBLEMS` and the `sprite` manifests.
pub const ROM: &[DArray] = &[
    // mn/mnvsmode/mnvsresults.c:622.
    DArray::local(
        "MN_VS_RESULTS_MAKE_EMBLEM_DOBJDESCS",
        "mnVSResultsMakeEmblem dobjdescs",
        OVL31,
        0x8013_8FE4,
        Elem::X32,
        &[12],
    ),
    // mn/mnvsmode/mnvsresults.c:631.
    DArray::local(
        "MN_VS_RESULTS_MAKE_EMBLEM_MOBJSUBS",
        "mnVSResultsMakeEmblem mobjsubs",
        OVL31,
        0x8013_9014,
        Elem::X32,
        &[12],
    ),
    // mn/mnvsmode/mnvsresults.c:640.
    DArray::local(
        "MN_VS_RESULTS_MAKE_EMBLEM_MATANIM_JOINTS",
        "mnVSResultsMakeEmblem matanim_joints",
        OVL31,
        0x8013_9044,
        Elem::X32,
        &[12],
    ),
    MAKE_STRING_OFFSETS,
    // mn/mnvsmode/mnvsresults.c:1859.
    DArray::local(
        "MN_VS_RESULTS_MAKE_HEADER_OFFSETS",
        "mnVSResultsMakeHeader offsets",
        OVL31,
        0x8013_96A8,
        Elem::X32,
        &[4],
    ),
    // mn/mnvsmode/mnvsresults.c:2359.
    DArray::local(
        "MN_VS_RESULTS_MAKE_LABEL_OFFSETS",
        "mnVSResultsMakeLabel offsets",
        OVL31,
        0x8013_96DC,
        Elem::X32,
        &[2],
    ),
];

/// `mnVSResultsMakeString`'s letters: `ssb-rom`'s `sprite::ANNOUNCE_COMMON`
/// manifest, which `crate::ifcommon` also indexes the announcer's letters by.
// mn/mnvsmode/mnvsresults.c:1155.
pub const MAKE_STRING_OFFSETS: DArray = DArray::local(
    "MN_VS_RESULTS_MAKE_STRING_OFFSETS",
    "mnVSResultsMakeString offsets",
    OVL31,
    0x8013_9424,
    Elem::X32,
    &[28],
);
