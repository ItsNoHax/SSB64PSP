//! `mn/mndata/mncharacters.c`'s local arrays (see [`super`]).

use crate::d_arrays::{DArray, Elem};
use crate::source::OVL33;

pub const GAME: &[DArray] = &[
    // mn/mndata/mncharacters.c:1347.
    DArray::local(
        "MN_CHARACTERS_GET_FIGHTER_KIND_FKINDS",
        "mnCharactersGetFighterKind fkinds",
        OVL33,
        0x8013_6250,
        Elem::I32,
        &[12],
    ),
    // mn/mndata/mncharacters.c:1368.
    DArray::local(
        "MN_CHARACTERS_GET_PAGE_PAGES",
        "mnCharactersGetPage pages",
        OVL33,
        0x8013_6280,
        Elem::I32,
        &[12],
    ),
    // mn/mndata/mncharacters.c:1387.
    DArray::local(
        "MN_CHARACTERS_MAKE_STORY_OFFSETS",
        "mnCharactersMakeStory offsets",
        OVL33,
        0x8013_62B0,
        Elem::X32,
        &[12],
    ),
    // mn/mndata/mncharacters.c:1548.
    DArray::local(
        "MN_CHARACTERS_MAKE_NAME_POS",
        "mnCharactersMakeName pos",
        OVL33,
        0x8013_6370,
        Elem::F32,
        &[12, 2],
    ),
    // mn/mndata/mncharacters.c:1567.
    DArray::local(
        "MN_CHARACTERS_MAKE_NAME_OFFSETS",
        "mnCharactersMakeName offsets",
        OVL33,
        0x8013_63D0,
        Elem::X32,
        &[12],
    ),
    // mn/mndata/mncharacters.c:1653.
    DArray::local(
        "MN_CHARACTERS_MAKE_WORKS_OFFSETS",
        "mnCharactersMakeWorks offsets",
        OVL33,
        0x8013_6400,
        Elem::X32,
        &[12],
    ),
    // mn/mndata/mncharacters.c:2061.
    DArray::local(
        "MN_CHARACTERS_UPDATE_MOTION_NAME_MOTION_NAMES",
        "mnCharactersUpdateMotionName motion_names",
        OVL33,
        0x8013_6460,
        Elem::X32,
        &[12, 3],
    ),
    // mn/mndata/mncharacters.c:2078.
    DArray::local(
        "MN_CHARACTERS_UPDATE_MOTION_NAME_MOTION_INPUTS",
        "mnCharactersUpdateMotionName motion_inputs",
        OVL33,
        0x8013_64F0,
        Elem::X32,
        &[3],
    ),
];

pub const ROM: &[DArray] = &[];
