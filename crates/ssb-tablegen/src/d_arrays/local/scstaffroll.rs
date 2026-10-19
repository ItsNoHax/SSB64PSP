//! `sc/sccommon/scstaffroll.c`'s local arrays (see [`super`]).

use crate::d_arrays::{DArray, Elem};
use crate::source::OVL59;

pub const GAME: &[DArray] = &[
    // sc/sccommon/scstaffroll.c:908.
    DArray::local(
        "SC_STAFFROLL_TRY_HIDE_UNLOCKS_LUIGI",
        "scStaffrollTryHideUnlocks luigi",
        OVL59,
        0x8013_A5E0,
        Elem::I32,
        &[5],
    ),
    // sc/sccommon/scstaffroll.c:918.
    DArray::local(
        "SC_STAFFROLL_TRY_HIDE_UNLOCKS_PURIN",
        "scStaffrollTryHideUnlocks purin",
        OVL59,
        0x8013_A5F4,
        Elem::I32,
        &[10],
    ),
    // sc/sccommon/scstaffroll.c:941.
    DArray::local(
        "SC_STAFFROLL_TRY_HIDE_UNLOCKS_CAPTAIN",
        "scStaffrollTryHideUnlocks captain",
        OVL59,
        0x8013_A61C,
        Elem::I32,
        &[8],
    ),
    // sc/sccommon/scstaffroll.c:954.
    DArray::local(
        "SC_STAFFROLL_TRY_HIDE_UNLOCKS_NESS",
        "scStaffrollTryHideUnlocks ness",
        OVL59,
        0x8013_A63C,
        Elem::I32,
        &[4],
    ),
    // sc/sccommon/scstaffroll.c:963.
    DArray::local(
        "SC_STAFFROLL_TRY_HIDE_UNLOCKS_EARTHBOUND",
        "scStaffrollTryHideUnlocks earthbound",
        OVL59,
        0x8013_A64C,
        Elem::I32,
        &[10],
    ),
    // sc/sccommon/scstaffroll.c:988.
    DArray::local(
        "SC_STAFFROLL_TRY_HIDE_UNLOCKS_FZERO",
        "scStaffrollTryHideUnlocks fzero",
        OVL59,
        0x8013_A674,
        Elem::I32,
        &[8],
    ),
    // sc/sccommon/scstaffroll.c:1001.
    DArray::local(
        "SC_STAFFROLL_TRY_HIDE_UNLOCKS_CLASSICMARIO",
        "scStaffrollTryHideUnlocks classicmario",
        OVL59,
        0x8013_A694,
        Elem::I32,
        &[12],
    ),
];

pub const ROM: &[DArray] = &[];
