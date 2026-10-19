//! `mn/mnmaps/mnmaps.c`'s local arrays (see [`super`]).

use crate::d_arrays::{DArray, Elem};
use crate::source::OVL30;

pub const GAME: &[DArray] = &[
    // mn/mnmaps/mnmaps.c:457.
    DArray::local(
        "MN_MAPS_GET_GROUND_KIND_GKINDS",
        "mnMapsGetGroundKind gkinds",
        OVL30,
        0x8013_4644,
        Elem::I32,
        &[10],
    ),
    // mn/mnmaps/mnmaps.c:513.
    DArray::local(
        "MN_MAPS_MAKE_ICONS_OFFSETS",
        "mnMapsMakeIcons offsets",
        OVL30,
        0x8013_466C,
        Elem::X32,
        &[10],
    ),
    // mn/mnmaps/mnmaps.c:581.
    DArray::local(
        "MN_MAPS_MAKE_NAME_OFFSETS",
        "mnMapsMakeName offsets",
        OVL30,
        0x8013_46DC,
        Elem::X32,
        &[9],
    ),
    // mn/mnmaps/mnmaps.c:753.
    DArray::local(
        "MN_MAPS_SET_LOGO_POSITION_POSITIONS",
        "mnMapsSetLogoPosition positions",
        OVL30,
        0x8013_47E4,
        Elem::F32,
        &[10, 2],
    ),
    // mn/mnmaps/mnmaps.c:784.
    DArray::local(
        "MN_MAPS_MAKE_EMBLEM_OFFSETS",
        "mnMapsMakeEmblem offsets",
        OVL30,
        0x8013_4834,
        Elem::X32,
        &[9],
    ),
    // mn/mnmaps/mnmaps.c:996.
    DArray::local(
        "MN_MAPS_MAKE_LAYER_SCALES",
        "mnMapsMakeLayer scales",
        OVL30,
        0x8013_4858,
        Elem::F32,
        &[9],
    ),
    // mn/mnmaps/mnmaps.c:1290.
    DArray::local(
        "MN_MAPS_SET_PREVIEW_CAMERA_POSITION_POSITIONS",
        "mnMapsSetPreviewCameraPosition positions",
        OVL30,
        0x8013_487C,
        Elem::F32,
        &[9, 3],
    ),
];

pub const ROM: &[DArray] = &[];
