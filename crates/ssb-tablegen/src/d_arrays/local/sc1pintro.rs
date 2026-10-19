//! `sc/sc1pmode/sc1pintro.c`'s local arrays (see [`super`]).

use crate::d_arrays::{DArray, Elem};
use crate::source::OVL24;

pub const GAME: &[DArray] = &[
    // sc/sc1pmode/sc1pintro.c:709.
    DArray::local(
        "SC_1P_INTRO_GET_FIGHTER_VELOCITY_Z_VEL_Z",
        "sc1PIntroGetFighterVelocityZ vel_z",
        OVL24,
        0x8013_4F90,
        Elem::F32,
        &[6],
    ),
    // sc/sc1pmode/sc1pintro.c:744.
    DArray::local(
        "SC_1P_INTRO_GET_FIGHTER_POSITION_Z_POS_Z",
        "sc1PIntroGetFighterPositionZ pos_z",
        OVL24,
        0x8013_4FA8,
        Elem::F32,
        &[6],
    ),
    // sc/sc1pmode/sc1pintro.c:1708.
    DArray::local(
        "SC_1P_INTRO_UPDATE_ANNOUNCE_FIGHTER_VOICES",
        "sc1PIntroUpdateAnnounce fighter_voices",
        OVL24,
        0x8013_5A68,
        Elem::U32,
        &[12],
    ),
    // sc/sc1pmode/sc1pintro.c:1725.
    DArray::local(
        "SC_1P_INTRO_UPDATE_ANNOUNCE_VS_FIGHTER_VOICES",
        "sc1PIntroUpdateAnnounce vs_fighter_voices",
        OVL24,
        0x8013_5A98,
        Elem::U32,
        &[14],
    ),
    // sc/sc1pmode/sc1pintro.c:1744.
    DArray::local(
        "SC_1P_INTRO_UPDATE_ANNOUNCE_ANNOUNCE_WAIT_TICS",
        "sc1PIntroUpdateAnnounce announce_wait_tics",
        OVL24,
        0x8013_5AD0,
        Elem::U32,
        &[12],
    ),
];

pub const ROM: &[DArray] = &[
    // sc/sc1pmode/sc1pintro.c:230.
    DArray::local(
        "SC_1P_INTRO_MAKE_LABELS_NUMBER_OFFSETS",
        "sc1PIntroMakeLabels number_offsets",
        OVL24,
        0x8013_4E40,
        Elem::X32,
        &[14],
    ),
    // sc/sc1pmode/sc1pintro.c:315.
    DArray::local(
        "SC_1P_INTRO_MAKE_FIGURES_FIGURE_OFFSETS",
        "sc1PIntroMakeFigures figure_offsets",
        OVL24,
        0x8013_4E78,
        Elem::X32,
        &[15],
    ),
    // sc/sc1pmode/sc1pintro.c:335.
    DArray::local(
        "SC_1P_INTRO_MAKE_FIGURES_FIGURE_POS",
        "sc1PIntroMakeFigures figure_pos",
        OVL24,
        0x8013_4EB4,
        Elem::I32,
        &[14, 2],
    ),
    // sc/sc1pmode/sc1pintro.c:457.
    DArray::local(
        "SC_1P_INTRO_MAKE_VS_NAME_OPPONENT_OFFSETS",
        "sc1PIntroMakeVSName opponent_offsets",
        OVL24,
        0x8013_4F24,
        Elem::X32,
        &[15],
    ),
    // sc/sc1pmode/sc1pintro.c:525.
    DArray::local(
        "SC_1P_INTRO_MAKE_NAME_NAME_OFFSETS",
        "sc1PIntroMakeName name_offsets",
        OVL24,
        0x8013_4F60,
        Elem::X32,
        &[12],
    ),
    // sc/sc1pmode/sc1pintro.c:1138.
    DArray::local(
        "SC_1P_INTRO_MAKE_STAGE_CAMERA_CAMANIM_JOINTS",
        "sc1PIntroMakeStageCamera camanim_joints",
        OVL24,
        0x8013_5180,
        Elem::X32,
        &[14],
    ),
    // sc/sc1pmode/sc1pintro.c:1504.
    DArray::local(
        "SC_1P_INTRO_MAKE_FIGHTER_CAMERA_CAMANIM_JOINTS",
        "sc1PIntroMakeFighterCamera camanim_joints",
        OVL24,
        0x8013_59D0,
        Elem::X32,
        &[12],
    ),
];
