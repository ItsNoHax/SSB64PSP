//! `mn/mndata/mnvsrecord.c`'s local arrays (see [`super`]).

use crate::d_arrays::{DArray, Elem};
use crate::source::OVL32;

pub const GAME: &[DArray] = &[
    // mn/mndata/mnvsrecord.c:122.
    DArray::local(
        "MN_VS_RECORD_GET_FIGHTER_KIND_BY_INDEX_FKINDS",
        "mnVSRecordGetFighterKindByIndex fkinds",
        OVL32,
        0x8013_66A0,
        Elem::I32,
        &[12],
    ),
    // mn/mndata/mnvsrecord.c:257.
    DArray::local(
        "MN_VS_RECORD_MAKE_DIGITS_OFFSETS",
        "mnVSRecordMakeDigits offsets",
        OVL32,
        0x8013_66D0,
        Elem::X32,
        &[10],
    ),
    // mn/mndata/mnvsrecord.c:436.
    DArray::local(
        "MN_VS_RECORD_MAKE_STRING_OFFSETS",
        "mnVSRecordMakeString offsets",
        OVL32,
        0x8013_66F8,
        Elem::X32,
        &[29],
    ),
    // mn/mndata/mnvsrecord.c:565.
    DArray::local(
        "MN_VS_RECORD_SUBTITLE_PROC_UPDATE_OFFSETS",
        "mnVSRecordSubtitleProcUpdate offsets",
        OVL32,
        0x8013_676C,
        Elem::X32,
        &[4],
    ),
    // mn/mndata/mnvsrecord.c:839.
    DArray::local(
        "MN_VS_RECORD_SET_ICON_POSITION_FOR_COLUMN_OFFSETS",
        "mnVSRecordSetIconPositionForColumn offsets",
        OVL32,
        0x8013_677C,
        Elem::F32,
        &[12, 2],
    ),
    // mn/mndata/mnvsrecord.c:897.
    DArray::local(
        "MN_VS_RECORD_MAKE_COLUMN_ICONS_OFFSETS",
        "mnVSRecordMakeColumnIcons offsets",
        OVL32,
        0x8013_67DC,
        Elem::X32,
        &[12],
    ),
    // mn/mndata/mnvsrecord.c:941.
    DArray::local(
        "MN_VS_RECORD_SET_ROW_ICON_POSITION_OFFSETS",
        "mnVSRecordSetRowIconPosition offsets",
        OVL32,
        0x8013_680C,
        Elem::F32,
        &[12, 2],
    ),
    // mn/mndata/mnvsrecord.c:976.
    DArray::local(
        "MN_VS_RECORD_MAKE_ROW_ICONS_OFFSETS",
        "mnVSRecordMakeRowIcons offsets",
        OVL32,
        0x8013_686C,
        Elem::X32,
        &[12],
    ),
    // mn/mndata/mnvsrecord.c:1071.
    DArray::local(
        "MN_VS_RECORD_MAKE_PORTRAIT_STATS_OFFSETS",
        "mnVSRecordMakePortraitStats offsets",
        OVL32,
        0x8013_689C,
        Elem::X32,
        &[12],
    ),
    // mn/mndata/mnvsrecord.c:1080.
    DArray::local(
        "MN_VS_RECORD_MAKE_PORTRAIT_STATS_STRING_COLORS",
        "mnVSRecordMakePortraitStats string_colors",
        OVL32,
        0x8013_68CC,
        Elem::U32,
        &[3],
    ),
    // mn/mndata/mnvsrecord.c:1081.
    DArray::local(
        "MN_VS_RECORD_MAKE_PORTRAIT_STATS_DIGIT_COLORS",
        "mnVSRecordMakePortraitStats digit_colors",
        OVL32,
        0x8013_68D8,
        Elem::U32,
        &[6],
    ),
    // mn/mndata/mnvsrecord.c:1241.
    DArray::local(
        "MN_VS_RECORD_MAKE_BATTLE_SCORE_TABLE_VALUES_COLORS",
        "mnVSRecordMakeBattleScoreTableValues colors",
        OVL32,
        0x8013_68F0,
        Elem::U32,
        &[6],
    ),
    // mn/mndata/mnvsrecord.c:1391.
    DArray::local(
        "MN_VS_RECORD_MAKE_RANKING_TABLE_VALUES_COL_WIDTHS",
        "mnVSRecordMakeRankingTableValues col_widths",
        OVL32,
        0x8013_6908,
        Elem::I32,
        &[7],
    ),
    // mn/mndata/mnvsrecord.c:1393.
    DArray::local(
        "MN_VS_RECORD_MAKE_RANKING_TABLE_VALUES_COLORS",
        "mnVSRecordMakeRankingTableValues colors",
        OVL32,
        0x8013_6924,
        Elem::U32,
        &[6],
    ),
    // mn/mndata/mnvsrecord.c:1559.
    DArray::local(
        "MN_VS_RECORD_MAKE_RANKING_TABLE_HEADERS_OFFSETS",
        "mnVSRecordMakeRankingTableHeaders offsets",
        OVL32,
        0x8013_693C,
        Elem::X32,
        &[7],
    ),
    // mn/mndata/mnvsrecord.c:1570.
    DArray::local(
        "MN_VS_RECORD_MAKE_RANKING_TABLE_HEADERS_X_PADDING",
        "mnVSRecordMakeRankingTableHeaders x_padding",
        OVL32,
        0x8013_6958,
        Elem::I32,
        &[7],
    ),
    // mn/mndata/mnvsrecord.c:1651.
    DArray::local(
        "MN_VS_RECORD_MAKE_INDIV_TABLE_VALUES_Y",
        "mnVSRecordMakeIndivTableValues y",
        OVL32,
        0x8013_6974,
        Elem::F32,
        &[4],
    ),
    // mn/mndata/mnvsrecord.c:1652.
    DArray::local(
        "MN_VS_RECORD_MAKE_INDIV_TABLE_VALUES_COLORS",
        "mnVSRecordMakeIndivTableValues colors",
        OVL32,
        0x8013_6984,
        Elem::U32,
        &[6],
    ),
    // mn/mndata/mnvsrecord.c:1723.
    DArray::local(
        "MN_VS_RECORD_MAKE_INDIV_PORTRAIT_ALL_OFFSETS",
        "mnVSRecordMakeIndivPortraitAll offsets",
        OVL32,
        0x8013_699C,
        Elem::X32,
        &[4],
    ),
    // mn/mndata/mnvsrecord.c:1730.
    DArray::local(
        "MN_VS_RECORD_MAKE_INDIV_PORTRAIT_ALL_POSITIONS",
        "mnVSRecordMakeIndivPortraitAll positions",
        OVL32,
        0x8013_69AC,
        Elem::F32,
        &[4, 2],
    ),
];

pub const ROM: &[DArray] = &[];
