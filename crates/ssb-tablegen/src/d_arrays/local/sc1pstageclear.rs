//! `sc/sc1pmode/sc1pstageclear.c`'s local arrays (see [`super`]).

use crate::d_arrays::{DArray, Elem};
use crate::source::OVL56;

pub const GAME: &[DArray] = &[
    // sc/sc1pmode/sc1pstageclear.c:1301.
    DArray::local(
        "SC_1P_STAGE_CLEAR_GET_NO_MISS_MULTIPLIER_NOMISS",
        "sc1PStageClearGetNoMissMultiplier nomiss",
        OVL56,
        0x8013_5198,
        Elem::I32,
        &[14],
    ),
];

pub const ROM: &[DArray] = &[
    // sc/sc1pmode/sc1pstageclear.c:601.
    DArray::local(
        "SC_1P_STAGE_CLEAR_SET_DIGIT_SPRITE_COLORS_COLORS_ALL",
        "sc1PStageClearSetDigitSpriteColors colors_all",
        OVL56,
        0x8013_5100,
        Elem::U8,
        &[3, 2, 3],
    ),
    // sc/sc1pmode/sc1pstageclear.c:668.
    DArray::local(
        "SC_1P_STAGE_CLEAR_GET_SCORE_DIGIT_SPRITE_FILE_ARRAY_IDS",
        "sc1PStageClearGetScoreDigitSprite file_array_ids",
        OVL56,
        0x8013_5114,
        Elem::I32,
        &[3],
    ),
    // sc/sc1pmode/sc1pstageclear.c:671.
    DArray::local(
        "SC_1P_STAGE_CLEAR_GET_SCORE_DIGIT_SPRITE_OFFSETS",
        "sc1PStageClearGetScoreDigitSprite offsets",
        OVL56,
        0x8013_5120,
        Elem::X32,
        &[10, 3],
    ),
    // sc/sc1pmode/sc1pstageclear.c:1324.
    DArray::local(
        "SC_1P_STAGE_CLEAR_GET_APPEND_BONUS_STAT_POINTS_OFFSETS",
        "sc1PStageClearGetAppendBonusStatPoints offsets",
        OVL56,
        0x8013_51D0,
        Elem::X32,
        &[5],
    ),
    // sc/sc1pmode/sc1pstageclear.c:1334.
    DArray::local(
        "SC_1P_STAGE_CLEAR_GET_APPEND_BONUS_STAT_POINTS_COLORS",
        "sc1PStageClearGetAppendBonusStatPoints colors",
        OVL56,
        0x8013_51E4,
        Elem::U8,
        &[2, 3],
    ),
];
