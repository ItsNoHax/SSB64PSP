//! Fighter hurtboxes — `FTDamageColl`.
//!
//! Each fighter carries up to eleven boxes bound to model joints
//! (`FTAttributes::damage_coll_descs`, transcribed below from the ported
//! fighters' `relocData/*Main.c`, US). A hit tests its sphere against them in
//! order (`ftMainSearchHitFighter`) and the first box it touches decides the
//! hurtbox `placement`: the low/middle/high column of the damage status
//! tables ([`crate::attack::damage_status`]).
//!
//! `gmCollisionCheckFighterAttackDamageCollide` moves the hit position into
//! the joint's space with the inverse world matrix and compares it with an
//! axis-aligned box `size` either side of `offset`, grown by the hit radius
//! divided by the joint's scale on each axis (`gmCollisionTestRectangle`).
//! The port tests the current position only: hitboxes carry no previous
//! position, so the swept half of the test does not apply.
//!
//! Joint poses come from the runtime ([`Fighter::joint_transforms`]). A
//! caller with no posed joints (host tests) keeps the former stand-in: one
//! sphere at the fighter's root, read as a middle hit.

use ssb_engine::math::Vec3;

use crate::attack::{DAMAGE_INDEX_N, MARIO_HURTBOX_RADIUS};
use crate::combat::{AttackState, HitStatus};
use crate::fighter::{Fighter, FighterKind, JointTransform};
use crate::status::Status;

/// `FTDamageCollDesc`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DamageCollDesc {
    /// `joint_id`, an `FTStruct::joints` index.
    pub joint: u8,
    /// `placement`: 0 low, 1 middle, 2 high.
    pub placement: u8,
    pub is_grabbable: bool,
    pub offset: Vec3,
    /// Half extents in joint space.
    pub size: Vec3,
}

const fn hurt(
    joint: u8,
    placement: u8,
    is_grabbable: bool,
    offset: Vec3,
    size: Vec3,
) -> DamageCollDesc {
    DamageCollDesc {
        joint,
        placement,
        is_grabbable,
        offset,
        size,
    }
}

/// `dMarioMain_attr.damage_coll_descs` (`203_MarioMain.c`, US).
const MARIO: &[DamageCollDesc] = &[
    hurt(
        6,
        1,
        true,
        Vec3::new(0.0, 10.0, 4.0),
        Vec3::new(103.0, 112.0, 95.0),
    ),
    hurt(
        12,
        2,
        true,
        Vec3::new(0.0, 68.0, 8.0),
        Vec3::new(148.0, 140.0, 138.0),
    ),
    hurt(
        14,
        1,
        false,
        Vec3::new(15.0, 0.0, 0.0),
        Vec3::new(36.0, 50.0, 50.0),
    ),
    hurt(
        8,
        1,
        false,
        Vec3::new(15.0, 0.0, 0.0),
        Vec3::new(36.0, 50.0, 50.0),
    ),
    hurt(
        15,
        1,
        false,
        Vec3::new(30.0, 0.0, 0.0),
        Vec3::new(58.0, 54.0, 54.0),
    ),
    hurt(
        9,
        1,
        false,
        Vec3::new(30.0, 0.0, 0.0),
        Vec3::new(58.0, 54.0, 54.0),
    ),
    hurt(
        24,
        0,
        false,
        Vec3::new(22.0, 0.0, 0.0),
        Vec3::new(58.0, 67.0, 67.0),
    ),
    hurt(
        19,
        0,
        false,
        Vec3::new(22.0, 0.0, 0.0),
        Vec3::new(58.0, 67.0, 67.0),
    ),
    hurt(
        25,
        0,
        false,
        Vec3::new(28.0, 0.0, 0.0),
        Vec3::new(66.0, 78.0, 76.0),
    ),
    hurt(
        20,
        0,
        false,
        Vec3::new(28.0, 0.0, 0.0),
        Vec3::new(66.0, 78.0, 76.0),
    ),
];

/// `dFoxMain_attr.damage_coll_descs` (`209_FoxMain.c`, US).
const FOX: &[DamageCollDesc] = &[
    hurt(
        5,
        1,
        true,
        Vec3::new(0.0, -32.0, 0.0),
        Vec3::new(102.0, 52.0, 45.0),
    ),
    hurt(
        6,
        1,
        true,
        Vec3::new(0.0, 48.0, 0.0),
        Vec3::new(136.0, 90.0, 82.0),
    ),
    hurt(
        12,
        2,
        true,
        Vec3::new(0.0, 74.0, 10.0),
        Vec3::new(136.0, 144.0, 122.0),
    ),
    hurt(
        14,
        1,
        false,
        Vec3::new(15.0, 0.0, 0.0),
        Vec3::new(63.0, 38.0, 34.0),
    ),
    hurt(
        8,
        1,
        false,
        Vec3::new(15.0, 0.0, 0.0),
        Vec3::new(63.0, 38.0, 34.0),
    ),
    hurt(
        15,
        1,
        false,
        Vec3::new(42.0, 0.0, 0.0),
        Vec3::new(92.0, 38.0, 34.0),
    ),
    hurt(
        9,
        1,
        false,
        Vec3::new(42.0, 0.0, 0.0),
        Vec3::new(92.0, 38.0, 34.0),
    ),
    hurt(
        24,
        0,
        true,
        Vec3::new(54.0, 0.0, 0.0),
        Vec3::new(102.0, 60.0, 52.0),
    ),
    hurt(
        19,
        0,
        true,
        Vec3::new(54.0, 0.0, 0.0),
        Vec3::new(102.0, 60.0, 52.0),
    ),
    hurt(
        25,
        0,
        false,
        Vec3::new(36.0, 0.0, 0.0),
        Vec3::new(113.0, 44.0, 44.0),
    ),
    hurt(
        20,
        0,
        false,
        Vec3::new(36.0, 0.0, 0.0),
        Vec3::new(113.0, 44.0, 44.0),
    ),
];

/// `dDonkeyMain_attr.damage_coll_descs` (`213_DonkeyMain.c`, US).
const DONKEY: &[DamageCollDesc] = &[
    hurt(
        5,
        1,
        true,
        Vec3::new(0.0, 0.0, 0.0),
        Vec3::new(143.0, 106.0, 116.0),
    ),
    hurt(
        6,
        1,
        true,
        Vec3::new(0.0, 146.0, 120.0),
        Vec3::new(195.0, 260.0, 260.0),
    ),
    hurt(
        12,
        2,
        true,
        Vec3::new(0.0, 0.0, 15.0),
        Vec3::new(170.0, 200.0, 210.0),
    ),
    hurt(
        14,
        1,
        true,
        Vec3::new(45.0, 0.0, 0.0),
        Vec3::new(170.0, 88.0, 92.0),
    ),
    hurt(
        8,
        1,
        true,
        Vec3::new(45.0, 0.0, 0.0),
        Vec3::new(170.0, 88.0, 92.0),
    ),
    hurt(
        15,
        1,
        false,
        Vec3::new(94.0, 0.0, 0.0),
        Vec3::new(207.0, 96.0, 108.0),
    ),
    hurt(
        9,
        1,
        false,
        Vec3::new(94.0, 0.0, 0.0),
        Vec3::new(207.0, 96.0, 108.0),
    ),
    hurt(
        25,
        0,
        false,
        Vec3::new(57.0, 0.0, 0.0),
        Vec3::new(133.0, 60.0, 64.0),
    ),
    hurt(
        20,
        0,
        false,
        Vec3::new(57.0, 0.0, 0.0),
        Vec3::new(133.0, 60.0, 64.0),
    ),
    hurt(
        26,
        0,
        false,
        Vec3::new(64.0, 8.0, 0.0),
        Vec3::new(174.0, 70.0, 64.0),
    ),
    hurt(
        21,
        0,
        false,
        Vec3::new(64.0, 8.0, 0.0),
        Vec3::new(174.0, 70.0, 64.0),
    ),
];

/// `dSamusMain_attr.damage_coll_descs` (`217_SamusMain.c`, US).
const SAMUS: &[DamageCollDesc] = &[
    hurt(
        5,
        1,
        true,
        Vec3::new(0.0, -69.0, -6.0),
        Vec3::new(88.0, 64.0, 86.0),
    ),
    hurt(
        6,
        1,
        true,
        Vec3::new(0.0, 25.0, -8.0),
        Vec3::new(120.0, 132.0, 110.0),
    ),
    hurt(
        13,
        2,
        true,
        Vec3::new(0.0, 0.0, 0.0),
        Vec3::new(69.0, 49.0, 68.0),
    ),
    hurt(
        15,
        1,
        false,
        Vec3::new(39.0, 0.0, 0.0),
        Vec3::new(147.0, 56.0, 56.0),
    ),
    hurt(
        8,
        1,
        false,
        Vec3::new(39.0, 0.0, 0.0),
        Vec3::new(147.0, 56.0, 56.0),
    ),
    hurt(
        16,
        1,
        false,
        Vec3::new(84.0, 0.0, 0.0),
        Vec3::new(147.0, 60.0, 60.0),
    ),
    hurt(
        9,
        1,
        false,
        Vec3::new(84.0, 0.0, 0.0),
        Vec3::new(147.0, 47.0, 44.0),
    ),
    hurt(
        32,
        0,
        true,
        Vec3::new(70.0, 8.0, 0.0),
        Vec3::new(153.0, 64.0, 74.0),
    ),
    hurt(
        27,
        0,
        true,
        Vec3::new(70.0, 8.0, 0.0),
        Vec3::new(153.0, 64.0, 74.0),
    ),
    hurt(
        33,
        0,
        false,
        Vec3::new(80.0, 0.0, 0.0),
        Vec3::new(203.0, 56.0, 64.0),
    ),
    hurt(
        28,
        0,
        false,
        Vec3::new(80.0, 0.0, 0.0),
        Vec3::new(203.0, 56.0, 64.0),
    ),
];

/// `dLuigiMain_attr.damage_coll_descs` (`221_LuigiMain.c`, US).
const LUIGI: &[DamageCollDesc] = &[
    hurt(
        6,
        1,
        true,
        Vec3::new(0.0, 28.0, 9.0),
        Vec3::new(110.0, 130.0, 98.0),
    ),
    hurt(
        12,
        2,
        true,
        Vec3::new(0.0, 81.0, 14.0),
        Vec3::new(155.0, 200.0, 150.0),
    ),
    hurt(
        14,
        1,
        false,
        Vec3::new(0.0, 0.0, 0.0),
        Vec3::new(51.0, 50.0, 50.0),
    ),
    hurt(
        8,
        1,
        false,
        Vec3::new(0.0, 0.0, 0.0),
        Vec3::new(51.0, 50.0, 50.0),
    ),
    hurt(
        15,
        1,
        false,
        Vec3::new(20.0, 0.0, 0.0),
        Vec3::new(74.0, 54.0, 54.0),
    ),
    hurt(
        9,
        1,
        false,
        Vec3::new(20.0, 0.0, 0.0),
        Vec3::new(74.0, 54.0, 54.0),
    ),
    hurt(
        24,
        0,
        false,
        Vec3::new(18.0, 0.0, 0.0),
        Vec3::new(62.0, 67.0, 67.0),
    ),
    hurt(
        19,
        0,
        false,
        Vec3::new(18.0, 0.0, 0.0),
        Vec3::new(62.0, 67.0, 67.0),
    ),
    hurt(
        25,
        0,
        false,
        Vec3::new(34.0, 0.0, 0.0),
        Vec3::new(67.0, 78.0, 76.0),
    ),
    hurt(
        20,
        0,
        false,
        Vec3::new(34.0, 0.0, 0.0),
        Vec3::new(67.0, 78.0, 76.0),
    ),
];

/// `dLinkMain_attr.damage_coll_descs` (`225_LinkMain.c`, US).
const LINK: &[DamageCollDesc] = &[
    hurt(
        5,
        1,
        true,
        Vec3::new(0.0, 0.0, 0.0),
        Vec3::new(90.0, 60.0, 78.0),
    ),
    hurt(
        6,
        1,
        true,
        Vec3::new(0.0, 40.0, 0.0),
        Vec3::new(98.0, 111.0, 78.0),
    ),
    hurt(
        23,
        2,
        true,
        Vec3::new(0.0, 30.0, 0.0),
        Vec3::new(68.0, 84.0, 88.0),
    ),
    hurt(
        13,
        1,
        false,
        Vec3::new(24.0, 0.0, 0.0),
        Vec3::new(85.0, 41.0, 41.0),
    ),
    hurt(
        8,
        1,
        false,
        Vec3::new(24.0, 0.0, 0.0),
        Vec3::new(85.0, 41.0, 41.0),
    ),
    hurt(
        14,
        1,
        false,
        Vec3::new(33.0, 0.0, 0.0),
        Vec3::new(99.0, 33.0, 33.0),
    ),
    hurt(
        9,
        1,
        false,
        Vec3::new(33.0, 0.0, 0.0),
        Vec3::new(99.0, 33.0, 33.0),
    ),
    hurt(
        31,
        0,
        true,
        Vec3::new(45.0, 3.0, 0.0),
        Vec3::new(86.0, 38.0, 46.0),
    ),
    hurt(
        26,
        0,
        true,
        Vec3::new(45.0, -3.0, 0.0),
        Vec3::new(86.0, 38.0, 46.0),
    ),
    hurt(
        32,
        0,
        false,
        Vec3::new(43.0, 3.0, 0.0),
        Vec3::new(121.0, 43.0, 51.0),
    ),
    hurt(
        27,
        0,
        false,
        Vec3::new(43.0, -3.0, 0.0),
        Vec3::new(121.0, 43.0, 51.0),
    ),
];

/// `dYoshiMain_attr.damage_coll_descs` (`247_YoshiMain.c`, US).
const YOSHI: &[DamageCollDesc] = &[
    hurt(
        5,
        1,
        true,
        Vec3::new(0.0, 3.0, -9.0),
        Vec3::new(117.0, 141.0, 237.0),
    ),
    hurt(
        6,
        1,
        true,
        Vec3::new(0.0, 81.0, 60.0),
        Vec3::new(72.0, 72.0, 72.0),
    ),
    hurt(
        7,
        2,
        true,
        Vec3::new(0.0, 45.0, 48.0),
        Vec3::new(150.0, 201.0, 240.0),
    ),
    hurt(
        15,
        1,
        false,
        Vec3::new(0.0, 0.0, 0.0),
        Vec3::new(0.0, 0.0, 0.0),
    ),
    hurt(
        11,
        1,
        false,
        Vec3::new(0.0, 0.0, 0.0),
        Vec3::new(0.0, 0.0, 0.0),
    ),
    hurt(
        16,
        1,
        false,
        Vec3::new(3.0, 6.0, 0.0),
        Vec3::new(51.0, 51.0, 51.0),
    ),
    hurt(
        12,
        1,
        false,
        Vec3::new(3.0, -6.0, 0.0),
        Vec3::new(51.0, 51.0, 51.0),
    ),
    hurt(
        27,
        0,
        false,
        Vec3::new(12.0, 15.0, 0.0),
        Vec3::new(75.0, 72.0, 48.0),
    ),
    hurt(
        22,
        0,
        false,
        Vec3::new(12.0, -15.0, 0.0),
        Vec3::new(75.0, 72.0, 48.0),
    ),
    hurt(
        28,
        0,
        false,
        Vec3::new(36.0, 0.0, 0.0),
        Vec3::new(90.0, 45.0, 51.0),
    ),
    hurt(
        23,
        0,
        false,
        Vec3::new(36.0, 0.0, 0.0),
        Vec3::new(90.0, 45.0, 51.0),
    ),
];

/// `dCaptainMain_attr.damage_coll_descs` (`236_CaptainMain.c`, US).
const CAPTAIN: &[DamageCollDesc] = &[
    hurt(
        5,
        1,
        true,
        Vec3::new(0.0, -70.0, -20.0),
        Vec3::new(108.0, 63.0, 78.0),
    ),
    hurt(
        6,
        1,
        true,
        Vec3::new(0.0, 20.0, -20.0),
        Vec3::new(150.0, 156.0, 114.0),
    ),
    hurt(
        12,
        2,
        true,
        Vec3::new(0.0, 25.0, 0.0),
        Vec3::new(75.0, 75.0, 75.0),
    ),
    hurt(
        14,
        1,
        false,
        Vec3::new(45.0, 0.0, 0.0),
        Vec3::new(138.0, 54.0, 54.0),
    ),
    hurt(
        8,
        1,
        false,
        Vec3::new(45.0, 0.0, 0.0),
        Vec3::new(138.0, 54.0, 54.0),
    ),
    hurt(
        15,
        1,
        false,
        Vec3::new(54.0, 0.0, 0.0),
        Vec3::new(138.0, 50.0, 50.0),
    ),
    hurt(
        9,
        1,
        false,
        Vec3::new(54.0, 0.0, 0.0),
        Vec3::new(138.0, 50.0, 50.0),
    ),
    hurt(
        25,
        0,
        true,
        Vec3::new(78.0, 0.0, 3.0),
        Vec3::new(129.0, 74.0, 66.0),
    ),
    hurt(
        20,
        0,
        true,
        Vec3::new(78.0, 0.0, 3.0),
        Vec3::new(129.0, 74.0, 66.0),
    ),
    hurt(
        26,
        0,
        false,
        Vec3::new(93.0, 3.0, 0.0),
        Vec3::new(180.0, 70.0, 64.0),
    ),
    hurt(
        21,
        0,
        false,
        Vec3::new(93.0, -3.0, 0.0),
        Vec3::new(180.0, 70.0, 64.0),
    ),
];

/// `dKirbyMain_attr.damage_coll_descs` (`229_KirbyMain.c`, US).
const KIRBY: &[DamageCollDesc] = &[
    hurt(
        5,
        1,
        true,
        Vec3::new(0.0, 0.0, 0.0),
        Vec3::new(0.0, 0.0, 0.0),
    ),
    hurt(
        6,
        2,
        true,
        Vec3::new(0.0, 30.0, 0.0),
        Vec3::new(260.0, 260.0, 260.0),
    ),
    hurt(
        15,
        1,
        false,
        Vec3::new(0.0, 0.0, 0.0),
        Vec3::new(45.0, 40.0, 40.0),
    ),
    hurt(
        10,
        1,
        false,
        Vec3::new(0.0, 0.0, 0.0),
        Vec3::new(90.0, 40.0, 40.0),
    ),
    hurt(
        16,
        1,
        false,
        Vec3::new(0.0, 0.0, 0.0),
        Vec3::new(90.0, 80.0, 80.0),
    ),
    hurt(
        11,
        1,
        false,
        Vec3::new(0.0, 0.0, 0.0),
        Vec3::new(90.0, 80.0, 80.0),
    ),
    hurt(
        29,
        0,
        false,
        Vec3::new(45.0, 20.0, 0.0),
        Vec3::new(160.0, 50.0, 110.0),
    ),
    hurt(
        24,
        0,
        false,
        Vec3::new(45.0, 20.0, 0.0),
        Vec3::new(160.0, 50.0, 110.0),
    ),
];

/// `dPikachuMain_attr.damage_coll_descs` (`243_PikachuMain.c`, US).
const PIKACHU: &[DamageCollDesc] = &[
    hurt(
        5,
        1,
        true,
        Vec3::new(0.0, -3.0, 0.0),
        Vec3::new(200.0, 96.0, 165.0),
    ),
    hurt(
        6,
        1,
        true,
        Vec3::new(0.0, 0.0, 10.0),
        Vec3::new(212.0, 129.0, 178.0),
    ),
    hurt(
        11,
        2,
        true,
        Vec3::new(0.0, 46.0, 12.0),
        Vec3::new(191.0, 149.0, 203.0),
    ),
    hurt(
        17,
        1,
        false,
        Vec3::new(0.0, 0.0, -6.0),
        Vec3::new(38.0, 38.0, 38.0),
    ),
    hurt(
        9,
        1,
        false,
        Vec3::new(0.0, 0.0, 6.0),
        Vec3::new(38.0, 38.0, 38.0),
    ),
    hurt(
        18,
        1,
        false,
        Vec3::new(0.0, 0.0, 0.0),
        Vec3::new(31.0, 24.0, 24.0),
    ),
    hurt(
        18,
        1,
        false,
        Vec3::new(0.0, 0.0, 0.0),
        Vec3::new(31.0, 24.0, 24.0),
    ),
    hurt(
        25,
        0,
        false,
        Vec3::new(0.0, 9.0, -6.0),
        Vec3::new(96.0, 69.0, 60.0),
    ),
    hurt(
        20,
        0,
        false,
        Vec3::new(0.0, 9.0, 6.0),
        Vec3::new(96.0, 69.0, 60.0),
    ),
    hurt(
        26,
        0,
        false,
        Vec3::new(43.0, 3.0, 0.0),
        Vec3::new(69.0, 45.0, 45.0),
    ),
    hurt(
        21,
        0,
        false,
        Vec3::new(43.0, 3.0, 0.0),
        Vec3::new(69.0, 45.0, 45.0),
    ),
];

/// `dPurinMain_attr.damage_coll_descs` (`233_PurinMain.c`, US).
const PURIN: &[DamageCollDesc] = &[
    hurt(
        5,
        1,
        true,
        Vec3::new(0.0, 0.0, 0.0),
        Vec3::new(0.0, 0.0, 0.0),
    ),
    hurt(
        6,
        2,
        true,
        Vec3::new(0.0, 30.0, 0.0),
        Vec3::new(260.0, 260.0, 260.0),
    ),
    hurt(
        14,
        1,
        false,
        Vec3::new(0.0, 0.0, 0.0),
        Vec3::new(45.0, 40.0, 40.0),
    ),
    hurt(
        10,
        1,
        false,
        Vec3::new(0.0, 0.0, 0.0),
        Vec3::new(90.0, 40.0, 40.0),
    ),
    hurt(
        15,
        1,
        false,
        Vec3::new(0.0, 0.0, 0.0),
        Vec3::new(90.0, 80.0, 80.0),
    ),
    hurt(
        11,
        1,
        false,
        Vec3::new(0.0, 0.0, 0.0),
        Vec3::new(90.0, 80.0, 80.0),
    ),
    hurt(
        28,
        0,
        false,
        Vec3::new(45.0, 20.0, 0.0),
        Vec3::new(160.0, 50.0, 110.0),
    ),
    hurt(
        23,
        0,
        false,
        Vec3::new(45.0, 20.0, 0.0),
        Vec3::new(160.0, 50.0, 110.0),
    ),
];

/// `dNessMain_attr.damage_coll_descs` (`239_NessMain.c`, US).
const NESS: &[DamageCollDesc] = &[
    hurt(
        5,
        1,
        true,
        Vec3::new(0.0, -30.0, 0.0),
        Vec3::new(117.0, 73.0, 105.0),
    ),
    hurt(
        6,
        1,
        true,
        Vec3::new(0.0, 28.0, 0.0),
        Vec3::new(118.0, 100.0, 112.0),
    ),
    hurt(
        12,
        2,
        true,
        Vec3::new(0.0, 91.0, 6.0),
        Vec3::new(143.0, 153.0, 147.0),
    ),
    hurt(
        14,
        1,
        false,
        Vec3::new(15.0, 0.0, 0.0),
        Vec3::new(59.0, 32.0, 32.0),
    ),
    hurt(
        8,
        1,
        false,
        Vec3::new(15.0, 0.0, 0.0),
        Vec3::new(59.0, 32.0, 32.0),
    ),
    hurt(
        15,
        1,
        false,
        Vec3::new(25.0, 0.0, 3.0),
        Vec3::new(51.0, 32.0, 32.0),
    ),
    hurt(
        9,
        1,
        false,
        Vec3::new(25.0, 0.0, 3.0),
        Vec3::new(51.0, 32.0, 32.0),
    ),
    hurt(
        25,
        0,
        false,
        Vec3::new(33.0, 0.0, 0.0),
        Vec3::new(59.0, 65.0, 58.0),
    ),
    hurt(
        19,
        0,
        false,
        Vec3::new(33.0, 0.0, 0.0),
        Vec3::new(59.0, 65.0, 58.0),
    ),
    hurt(
        26,
        0,
        false,
        Vec3::new(26.0, 0.0, -2.0),
        Vec3::new(68.0, 47.0, 42.0),
    ),
    hurt(
        20,
        0,
        false,
        Vec3::new(26.0, 0.0, -2.0),
        Vec3::new(68.0, 47.0, 42.0),
    ),
];

/// A fighter's hurtbox table, or `None` for an unported fighter.
pub fn damage_colls(kind: FighterKind) -> Option<&'static [DamageCollDesc]> {
    Some(match kind {
        FighterKind::Mario => MARIO,
        FighterKind::Fox => FOX,
        FighterKind::Donkey => DONKEY,
        FighterKind::Samus => SAMUS,
        FighterKind::Luigi => LUIGI,
        FighterKind::Link => LINK,
        FighterKind::Yoshi => YOSHI,
        FighterKind::Captain => CAPTAIN,
        FighterKind::Kirby => KIRBY,
        FighterKind::Pikachu => PIKACHU,
        FighterKind::Purin => PURIN,
        FighterKind::Ness => NESS,
        _ => return None,
    })
}

/// `dFTCommonYoshiEggDamageCollDescs` (`ftcommoncaptureyoshi.c:11`): the one
/// TopN box `ftCommonYoshiEggSetDamageCollCollisions` leaves live while the
/// fighter is inside an egg; the rest turn intangible.
pub fn yoshi_egg_coll(kind: FighterKind) -> Option<DamageCollDesc> {
    let (y, size) = match kind {
        FighterKind::Mario => (157.0, 180.0),
        FighterKind::Fox => (155.0, 171.0),
        FighterKind::Donkey => (230.0, 245.0),
        FighterKind::Samus => (163.0, 198.0),
        FighterKind::Luigi => (160.0, 188.0),
        FighterKind::Link => (133.0, 148.0),
        FighterKind::Yoshi => (175.0, 210.0),
        FighterKind::Captain => (156.0, 198.0),
        FighterKind::Kirby => (132.0, 164.0),
        FighterKind::Pikachu => (144.0, 165.0),
        FighterKind::Purin => (144.0, 162.0),
        FighterKind::Ness => (160.0, 168.0),
        _ => return None,
    };
    Some(hurt(
        0,
        1,
        false,
        Vec3::new(0.0, y, 0.0),
        Vec3::new(size, size, size),
    ))
}

/// `gmCollisionSetInvertMatrix` applied to a point: the joint-space position
/// of `world`, or `None` for a degenerate matrix.
fn to_joint_space(t: &JointTransform, world: Vec3) -> Option<Vec3> {
    let [a, b, c] = t.axes;
    let det = a.x * (b.y * c.z - b.z * c.y) - b.x * (a.y * c.z - a.z * c.y)
        + c.x * (a.y * b.z - a.z * b.y);
    if det == 0.0 {
        return None;
    }
    let d = world - t.origin;
    // Cramer's rule for a * u + b * v + c * w = d.
    let solve = |col: Vec3, i: usize| {
        let m = match i {
            0 => [col, b, c],
            1 => [a, col, c],
            _ => [a, b, col],
        };
        (m[0].x * (m[1].y * m[2].z - m[1].z * m[2].y)
            - m[1].x * (m[0].y * m[2].z - m[0].z * m[2].y)
            + m[2].x * (m[0].y * m[1].z - m[0].z * m[1].y))
            / det
    };
    Some(Vec3::new(solve(d, 0), solve(d, 1), solve(d, 2)))
}

/// `gmCollisionTestRectangle`'s stationary case (`opkind == 2`).
pub fn box_contains(t: &JointTransform, desc: &DamageCollDesc, pos: Vec3, radius: f32) -> bool {
    let Some(local) = to_joint_space(t, pos) else {
        return false;
    };
    let scale = |axis: Vec3| axis.length();
    let reach = |size: f32, s: f32| size + radius / s;
    let d = local - desc.offset;
    d.x.abs() <= reach(desc.size.x, scale(t.axes[0]))
        && d.y.abs() <= reach(desc.size.y, scale(t.axes[1]))
        && d.z.abs() <= reach(desc.size.z, scale(t.axes[2]))
}

/// The first of `descs` a sphere at `pos` touches, as its placement column.
/// `None` when a posed box exists and none is touched; `Some(None)` when no
/// box could be posed at all.
fn search(
    f: &Fighter,
    descs: &mut dyn Iterator<Item = DamageCollDesc>,
    pos: Vec3,
    radius: f32,
) -> Option<Option<usize>> {
    let mut posed = false;
    for desc in descs {
        let Some(t) = f
            .joint_transforms
            .get(desc.joint as usize)
            .copied()
            .flatten()
        else {
            continue;
        };
        posed = true;
        if box_contains(&t, &desc, pos, radius) {
            return Some(Some(usize::from(desc.placement)));
        }
    }
    if posed {
        None
    } else {
        Some(None)
    }
}

/// `FTStruct::damage_colls`' per-frame state over the attribute table:
/// each box's hit status and any motion-script resize.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DamageColls {
    pub hitstatus: [HitStatus; 11],
    /// `SetDamageCollPartID`: offset and half extents.
    pub overrides: [Option<(Vec3, Vec3)>; 11],
    /// `is_hitstatus_nodamage`.
    pub is_hitstatus_nodamage: bool,
    /// `is_damage_coll_modify`.
    pub is_modify: bool,
}

impl Default for DamageColls {
    fn default() -> Self {
        DamageColls {
            hitstatus: [HitStatus::Normal; 11],
            overrides: [None; 11],
            is_hitstatus_nodamage: false,
            is_modify: false,
        }
    }
}

/// `ftParamSetHitStatusPartAll`.
pub fn set_hit_status_part_all(f: &mut Fighter, status: HitStatus) {
    let n = damage_colls(f.kind).map_or(0, |d| d.len());
    for slot in f.damage_colls.hitstatus.iter_mut().take(n) {
        *slot = status;
    }
    f.damage_colls.is_hitstatus_nodamage = status != HitStatus::Normal;
}

/// `ftParamSetHitStatusPartID`: the first box on `joint` only.
pub fn set_hit_status_part_id(f: &mut Fighter, joint: i32, status: HitStatus) {
    let Some(descs) = damage_colls(f.kind) else {
        return;
    };
    if let Some(i) = descs.iter().position(|d| i32::from(d.joint) == joint) {
        f.damage_colls.hitstatus[i] = status;
        if status != HitStatus::Normal {
            f.damage_colls.is_hitstatus_nodamage = true;
        }
    }
}

/// `ftParamResetFighterDamageCollsAll`.
pub fn reset_damage_colls(f: &mut Fighter) {
    f.damage_colls.overrides = [None; 11];
    f.damage_colls.is_modify = false;
}

/// `ftParamModifyDamageCollID`: the first box on `joint` takes the new
/// offset and (halved) size.
pub fn modify_damage_coll(f: &mut Fighter, joint: i32, offset: Vec3, size: Vec3) {
    let Some(descs) = damage_colls(f.kind) else {
        return;
    };
    if let Some(i) = descs.iter().position(|d| i32::from(d.joint) == joint) {
        f.damage_colls.overrides[i] = Some((offset, size * 0.5));
        f.damage_colls.is_modify = true;
    }
}

/// `ftMainSetStatus`'s damage-collision half: hit statuses back to normal
/// (unless preserved) and script resizes undone.
pub fn on_set_status(f: &mut Fighter, preserve_hitstatus: bool) {
    if !preserve_hitstatus {
        if f.damage_colls.is_hitstatus_nodamage {
            set_hit_status_part_all(f, HitStatus::Normal);
        }
        f.hitstatus = HitStatus::Normal;
    }
    if f.damage_colls.is_modify {
        reset_damage_colls(f);
    }
}

/// A hurtbox an attack touched.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HurtHit {
    /// The `placement` column.
    pub placement: usize,
    pub hitstatus: HitStatus,
}

/// `ftMainSearchHitFighter`'s damage-collision loop for one attack: the
/// first non-intangible box it touches, swept from `pos_prev` unless the
/// attack is new this frame.
pub fn search_attack(
    f: &Fighter,
    pos_curr: Vec3,
    pos_prev: Vec3,
    radius: f32,
    state: AttackState,
) -> Option<HurtHit> {
    let mut posed = false;
    let egg = f.status.status == Status::YoshiEgg;
    let descs: &[DamageCollDesc] = if egg {
        &[]
    } else {
        damage_colls(f.kind).unwrap_or(&[])
    };
    let egg_desc = if egg { yoshi_egg_coll(f.kind) } else { None };
    let iter = descs
        .iter()
        .copied()
        .enumerate()
        .map(|(i, d)| (Some(i), d))
        .chain(egg_desc.map(|d| (None, d)));
    for (slot, desc) in iter {
        let hitstatus = slot.map_or(HitStatus::Normal, |i| f.damage_colls.hitstatus[i]);
        if hitstatus == HitStatus::None {
            break;
        }
        let Some(t) = f
            .joint_transforms
            .get(desc.joint as usize)
            .copied()
            .flatten()
        else {
            continue;
        };
        posed = true;
        if hitstatus == HitStatus::Intangible {
            continue;
        }
        let (offset, size) = slot
            .and_then(|i| f.damage_colls.overrides[i])
            .unwrap_or((desc.offset, desc.size));
        if test_rectangle(&t, pos_curr, pos_prev, radius, state, offset, size) {
            return Some(HurtHit {
                placement: usize::from(desc.placement),
                hitstatus,
            });
        }
    }
    if posed {
        return None;
    }
    // No pose (host tests): the root-sphere stand-in.
    let touches = crate::attack::spheres_overlap(pos_curr, radius, f.pos, MARIO_HURTBOX_RADIUS)
        || (state == AttackState::Interpolate
            && crate::attack::spheres_overlap(pos_prev, radius, f.pos, MARIO_HURTBOX_RADIUS));
    touches.then_some(HurtHit {
        placement: DAMAGE_INDEX_N,
        hitstatus: f.damage_colls.hitstatus[0],
    })
}

/// `func_ovl2_800EE24C`.
fn outcode_xy(p: Vec3, c: Vec3) -> u32 {
    let mut flags = 0;
    if p.x < -c.x {
        flags |= 1;
    }
    if p.x > c.x {
        flags |= 2;
    }
    if p.y < -c.y {
        flags |= 4;
    }
    if p.y > c.y {
        flags |= 8;
    }
    flags
}

/// `func_ovl2_800EE2C0`.
fn outcode_z(p: Vec3, c: Vec3) -> u32 {
    let mut flags = 0;
    if p.z < -c.z {
        flags |= 1;
    }
    if p.z > c.z {
        flags |= 2;
    }
    flags
}

/// The segment clip shared by `gmCollisionTestRectangle` and
/// `func_ovl2_800EEEAC`: clips `a`-`b` against the X/Y slabs, then tests Z.
fn clip_segment(mut a: Vec3, mut b: Vec3, c: Vec3, dist: Vec3) -> bool {
    let mut fa = outcode_xy(a, c);
    let mut fb = outcode_xy(b, c);
    // Each pass moves one endpoint onto a slab face, so four suffice; the
    // cap guards against a NaN from a zero-length axis.
    for _ in 0..16 {
        if fa == 0 && fb == 0 {
            break;
        }
        if fa & fb != 0 {
            return false;
        }
        let main = if fa != 0 { fa } else { fb };
        let mut p = Vec3::ZERO;
        if main & 1 != 0 {
            p.x = -c.x;
            p.y = ((p.x - a.x) / dist.x) * dist.y + a.y;
            p.z = ((p.x - a.x) / dist.x) * dist.z + a.z;
        } else if main & 2 != 0 {
            p.x = c.x;
            p.y = ((p.x - a.x) / dist.x) * dist.y + a.y;
            p.z = ((p.x - a.x) / dist.x) * dist.z + a.z;
        } else if main & 4 != 0 {
            p.y = -c.y;
            p.x = ((p.y - a.y) / dist.y) * dist.x + a.x;
            p.z = ((p.y - a.y) / dist.y) * dist.z + a.z;
        } else if main & 8 != 0 {
            p.y = c.y;
            p.x = ((p.y - a.y) / dist.y) * dist.x + a.x;
            p.z = ((p.y - a.y) / dist.y) * dist.z + a.z;
        }
        if main == fa {
            a = p;
            fa = outcode_xy(a, c);
        } else {
            b = p;
            fb = outcode_xy(b, c);
        }
    }
    if fa != 0 || fb != 0 {
        return false;
    }
    outcode_z(a, c) & outcode_z(b, c) == 0
}

/// `gmCollisionTestRectangle`: a box `size` either side of `offset` in the
/// joint's space, grown by the attack radius over the joint's scale. A new
/// attack (`Transfer`) tests its position; an older one its whole swept
/// segment.
pub fn test_rectangle(
    t: &JointTransform,
    pos_curr: Vec3,
    pos_prev: Vec3,
    radius: f32,
    state: AttackState,
    offset: Vec3,
    size: Vec3,
) -> bool {
    let scale = [t.axes[0].length(), t.axes[1].length(), t.axes[2].length()];
    if scale.contains(&0.0) {
        return false;
    }
    let c = Vec3::new(
        size.x + radius / scale[0],
        size.y + radius / scale[1],
        size.z + radius / scale[2],
    );
    let Some(curr) = to_joint_space(t, pos_curr) else {
        return false;
    };
    let curr = curr - offset;
    if state != AttackState::Interpolate {
        return -c.x <= curr.x
            && curr.x <= c.x
            && -c.y <= curr.y
            && curr.y <= c.y
            && -c.z <= curr.z
            && curr.z <= c.z;
    }
    let Some(prev) = to_joint_space(t, pos_prev) else {
        return false;
    };
    let prev = prev - offset;
    clip_segment(curr, prev, c, prev - curr)
}

/// `gmCollisionTestSphere` (`sphit_kind` 2): an ellipsoid `size` about
/// `offset` in the joint's space, grown by the attack radius over the
/// joint's scale, against the attack's position or swept segment.
pub fn test_sphere(
    t: &JointTransform,
    pos_curr: Vec3,
    pos_prev: Vec3,
    radius: f32,
    state: AttackState,
    offset: Vec3,
    size: Vec3,
) -> bool {
    let scale = [t.axes[0].length(), t.axes[1].length(), t.axes[2].length()];
    if scale.contains(&0.0) {
        return false;
    }
    let c = Vec3::new(
        size.x + radius / scale[0],
        size.y + radius / scale[1],
        size.z + radius / scale[2],
    );
    let norm = |p: Vec3| {
        let p = p - offset;
        Vec3::new(p.x / c.x, p.y / c.y, p.z / c.z)
    };
    let Some(curr) = to_joint_space(t, pos_curr) else {
        return false;
    };
    let c1 = norm(curr);
    if state == AttackState::Transfer || pos_curr == pos_prev {
        return c1.length() <= 1.0;
    }
    let Some(prev) = to_joint_space(t, pos_prev) else {
        return false;
    };
    let c2 = norm(prev);
    let d = c1 - c2;
    let a = d.x * d.x + d.y * d.y + d.z * d.z;
    if a == 0.0 {
        return c1.length() <= 1.0;
    }
    let b = d.x * c2.x + d.y * c2.y + d.z * c2.z;
    let k = a * ((c2.x * c2.x + c2.y * c2.y + c2.z * c2.z) - 1.0);
    let disc = b * b - k;
    if b * b < k {
        return false;
    }
    if disc == 0.0 {
        let t0 = -b / a;
        return (0.0..=1.0).contains(&t0);
    }
    let root = ssb_engine::math::sqrt(disc);
    let t1 = (root - b) / a;
    let t2 = (-b - root) / a;
    (0.0..=1.0).contains(&t1) || (0.0..=1.0).contains(&t2) || t1 * t2 < 0.0
}

/// `gmCollisionTestSphere` with `sphit_kind` 1 (the shield): on contact, the
/// angle between the swept segment and the entry point, both in the joint's
/// Y/Z plane, and their normalised cross product (`shield_collide_angle`
/// and the hop axis). A still or new attack reports 180° about +X.
pub fn test_sphere_angle(
    t: &JointTransform,
    pos_curr: Vec3,
    pos_prev: Vec3,
    radius: f32,
    state: AttackState,
    offset: Vec3,
    size: Vec3,
) -> Option<(f32, Vec3)> {
    const STILL: (f32, Vec3) = (core::f32::consts::PI, Vec3::new(1.0, 0.0, 0.0));
    if !test_sphere(t, pos_curr, pos_prev, radius, state, offset, size) {
        return None;
    }
    if state == AttackState::Transfer || pos_curr == pos_prev {
        return Some(STILL);
    }
    let scale = [t.axes[0].length(), t.axes[1].length(), t.axes[2].length()];
    let c = Vec3::new(
        size.x + radius / scale[0],
        size.y + radius / scale[1],
        size.z + radius / scale[2],
    );
    let norm = |p: Vec3| {
        let p = p - offset;
        Vec3::new(p.x / c.x, p.y / c.y, p.z / c.z)
    };
    let c1 = norm(to_joint_space(t, pos_curr)?);
    let c2 = norm(to_joint_space(t, pos_prev)?);
    let d = c1 - c2;
    let a = d.x * d.x + d.y * d.y + d.z * d.z;
    if a == 0.0 {
        return Some(STILL);
    }
    let b = d.x * c2.x + d.y * c2.y + d.z * c2.z;
    let k = a * ((c2.x * c2.x + c2.y * c2.y + c2.z * c2.z) - 1.0);
    let disc = b * b - k;
    let t_hit = if disc == 0.0 {
        -b / a
    } else {
        let root = ssb_engine::math::sqrt(disc);
        ((root - b) / a).min((-b - root) / a)
    };
    let sub = Vec3::new(0.0, d.y, d.z);
    let at = Vec3::new(0.0, d.y * t_hit + c2.y, d.z * t_hit + c2.z);
    if at.y == 0.0 && at.z == 0.0 {
        return Some(STILL);
    }
    let (ls, la) = (sub.length(), at.length());
    if ls == 0.0 {
        return Some(STILL);
    }
    // `syVectorAngleDiff3D`.
    let cos = ((sub.x * at.x + sub.y * at.y + sub.z * at.z) / (ls * la)).clamp(-1.0, 1.0);
    let angle = ssb_engine::math::atan2(ssb_engine::math::sqrt(1.0 - cos * cos), cos);
    if angle == core::f32::consts::PI {
        return Some(STILL);
    }
    // `syVectorNormCross3D`.
    let cross = Vec3::new(
        sub.y * at.z - sub.z * at.y,
        sub.z * at.x - sub.x * at.z,
        sub.x * at.y - sub.y * at.x,
    );
    let len = cross.length();
    let axis = if len == 0.0 {
        cross
    } else {
        cross * (1.0 / len)
    };
    Some((angle, axis))
}

/// `gmCollisionCheckFighterAttacksCollide`: a broad box test, then the
/// first attack against the second's swept capsule in the second's frame.
pub fn attacks_collide(
    (a_curr, a_prev, a_size, a_state): (Vec3, Vec3, f32, AttackState),
    (b_curr, b_prev, b_size, b_state): (Vec3, Vec3, f32, AttackState),
) -> bool {
    if !attacks_overlap_broad(
        a_curr, a_prev, a_size, a_state, b_curr, b_prev, b_size, b_state,
    ) {
        return false;
    }
    // `func_ovl2_800EE050`: the second attack's segment frame.
    let mut half = 0.0;
    let mut frame: Option<[[f32; 3]; 4]> = None;
    if b_state == AttackState::Interpolate {
        let d = b_prev - b_curr;
        let len = d.length();
        if len != 0.0 {
            let d = d * (1.0 / len);
            let mut m = [[0.0f32; 3]; 4];
            if d.x * d.x == 1.0 {
                let s = if d.x >= 0.0 { 1.0 } else { -1.0 };
                m[0][0] = s;
                m[1][1] = s;
                m[2][2] = 1.0;
            } else {
                m[0][0] = d.x;
                m[1][2] = (d.z * d.y) / (1.0 + d.x);
                m[2][1] = (d.z * d.y) / (1.0 + d.x);
                let inv = 1.0 / (1.0 - d.x * d.x);
                m[1][1] = ((1.0 - (d.z * d.z * inv)) * d.x) + (d.z * d.z * inv);
                m[2][0] = d.z;
                m[0][2] = -d.z;
                m[2][2] = ((1.0 - (d.y * d.y * inv)) * d.x) + (d.y * d.y * inv);
                m[0][1] = d.y;
                m[1][0] = -d.y;
            }
            half = len * 0.5;
            m[3] = [half, 0.0, 0.0];
            frame = Some(m);
        }
    }
    // `func_ovl2_800EEEAC`.
    let c = Vec3::new(half + b_size + a_size, b_size + a_size, b_size + a_size);
    let to_frame = |p: Vec3| -> Vec3 {
        let p = p - b_curr;
        match frame {
            Some(m) => Vec3::new(
                m[0][0] * p.x + m[1][0] * p.y + m[2][0] * p.z + m[3][0],
                m[0][1] * p.x + m[1][1] * p.y + m[2][1] * p.z + m[3][1],
                m[0][2] * p.x + m[1][2] * p.y + m[2][2] * p.z + m[3][2],
            ),
            None => p,
        }
    };
    let p0 = to_frame(a_curr);
    if a_state != AttackState::Interpolate {
        return -c.x <= p0.x
            && p0.x <= c.x
            && -c.y <= p0.y
            && p0.y <= c.y
            && -c.z <= p0.z
            && p0.z <= c.z;
    }
    let p1 = to_frame(a_prev);
    clip_segment(p0, p1, c, p1 - p0)
}

/// `func_ovl2_800EF5D4`: whether two attacks' swept boxes can meet.
#[allow(clippy::too_many_arguments)]
fn attacks_overlap_broad(
    a0: Vec3,
    a1: Vec3,
    a_size: f32,
    a_state: AttackState,
    b0: Vec3,
    b1: Vec3,
    b_size: f32,
    b_state: AttackState,
) -> bool {
    let r = b_size + a_size;
    let lo = |p: f32, q: f32, pad: f32| {
        if p < q {
            (p - pad, q + pad)
        } else {
            (q - pad, p + pad)
        }
    };
    if b_state == AttackState::Transfer {
        if a_state == AttackState::Transfer {
            return !(a0.x < b0.x - r || b0.x + r < a0.x || a0.y < b0.y - r || b0.y + r < a0.y);
        }
        let (x0, x1) = lo(a0.x, a1.x, r);
        if b0.x < x0 || x1 < b0.x {
            return false;
        }
        let (y0, y1) = lo(a0.y, a1.y, r);
        return !(b0.y < y0 || y1 < b0.y);
    }
    if a_state == AttackState::Transfer {
        let (x0, x1) = lo(b0.x, b1.x, r);
        if a0.x < x0 || x1 < a0.x {
            return false;
        }
        let (y0, y1) = lo(b0.y, b1.y, r);
        return !(a0.y < y0 || y1 < a0.y);
    }
    let span_x = (a0.x - a1.x).abs();
    let (x0, x1) = lo(b0.x, b1.x, span_x * 0.5 + r);
    if (a0.x < x0 && a1.x < x0) || (x1 < a0.x && x1 < a1.x) {
        return false;
    }
    let span_y = (a0.y - a1.y).abs();
    let (y0, y1) = lo(b0.y, b1.y, span_y * 0.5 + r);
    !((a0.y < y0 && a1.y < y0) || (y1 < a0.y && y1 < a1.y))
}

/// The damage-table column a hit sphere at `pos` lands in, or `None` when it
/// misses every hurtbox (`ftMainSearchHitFighter`'s damage-coll loop).
pub fn hit_index(f: &Fighter, pos: Vec3, radius: f32) -> Option<usize> {
    let result = if f.status.status == Status::YoshiEgg {
        match yoshi_egg_coll(f.kind) {
            Some(desc) => search(f, &mut core::iter::once(desc), pos, radius),
            None => Some(None),
        }
    } else {
        match damage_colls(f.kind) {
            Some(descs) => search(f, &mut descs.iter().copied(), pos, radius),
            None => Some(None),
        }
    };
    match result {
        Some(Some(index)) => Some(index),
        None => None,
        Some(None) => crate::attack::spheres_overlap(pos, radius, f.pos, MARIO_HURTBOX_RADIUS)
            .then_some(DAMAGE_INDEX_N),
    }
}

/// Whether a catch box at `pos` touches one of the target's grabbable
/// hurtboxes (`ftMainSearchFighterCatch`'s `is_grabbable` test), with the
/// same root-sphere stand-in when the target has no posed joints.
pub fn catch_touches(f: &Fighter, pos: Vec3, radius: f32) -> bool {
    // `ftMainSearchFighterCatch`: parts end at `nGMHitStatusNone`, and
    // intangible or invincible parts cannot be grabbed.
    let mut grabbable = damage_colls(f.kind)
        .unwrap_or(&[])
        .iter()
        .copied()
        .enumerate()
        .take_while(|&(i, _)| f.damage_colls.hitstatus[i] != HitStatus::None)
        .filter(|&(i, desc)| {
            desc.is_grabbable
                && !matches!(
                    f.damage_colls.hitstatus[i],
                    HitStatus::Intangible | HitStatus::Invincible
                )
        })
        .map(|(_, desc)| desc);
    match search(f, &mut grabbable, pos, radius) {
        Some(Some(_)) => true,
        None => false,
        Some(None) => crate::attack::spheres_overlap(pos, radius, f.pos, MARIO_HURTBOX_RADIUS),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn posed(f: &mut Fighter, joint: usize, origin: Vec3, scale: f32) {
        f.joint_transforms[joint] = Some(JointTransform {
            axes: [
                Vec3::new(scale, 0.0, 0.0),
                Vec3::new(0.0, scale, 0.0),
                Vec3::new(0.0, 0.0, scale),
            ],
            origin,
        });
    }

    #[test]
    fn every_ported_fighter_has_a_table() {
        for kind in [
            FighterKind::Mario,
            FighterKind::Fox,
            FighterKind::Donkey,
            FighterKind::Samus,
            FighterKind::Luigi,
            FighterKind::Link,
            FighterKind::Yoshi,
            FighterKind::Captain,
        ] {
            let descs = damage_colls(kind).unwrap();
            assert!(!descs.is_empty() && descs.len() <= 11, "{kind:?}");
            assert!(descs.iter().all(|d| d.placement <= 2));
        }
        assert_eq!(MARIO.len(), 10);
        // US Mario's head box is 140 tall (JP: 160).
        assert_eq!(MARIO[1].size.y, 140.0);
    }

    #[test]
    fn the_box_grows_by_the_radius_over_the_joint_scale() {
        let desc = MARIO[0];
        let mut f = Fighter::new(FighterKind::Mario, 0, 3);
        posed(&mut f, 6, Vec3::new(1000.0, 0.0, 0.0), 2.0);
        // Joint-space X reach is 103 + 20 / 2 = 113, world 226.
        let t = f.joint_transforms[6].unwrap();
        assert!(box_contains(
            &t,
            &desc,
            Vec3::new(1000.0 + 225.0, 20.0, 8.0),
            20.0
        ));
        assert!(!box_contains(
            &t,
            &desc,
            Vec3::new(1000.0 + 227.0, 20.0, 8.0),
            20.0
        ));
    }

    #[test]
    fn the_first_box_touched_decides_the_placement() {
        let mut f = Fighter::new(FighterKind::Mario, 0, 3);
        posed(&mut f, 6, Vec3::new(0.0, 100.0, 0.0), 1.0);
        posed(&mut f, 12, Vec3::new(0.0, 300.0, 0.0), 1.0);
        posed(&mut f, 24, Vec3::new(0.0, -200.0, 0.0), 1.0);
        assert_eq!(hit_index(&f, Vec3::new(0.0, 110.0, 0.0), 10.0), Some(1));
        assert_eq!(hit_index(&f, Vec3::new(0.0, 400.0, 0.0), 10.0), Some(2));
        assert_eq!(hit_index(&f, Vec3::new(0.0, -200.0, 0.0), 10.0), Some(0));
        assert_eq!(hit_index(&f, Vec3::new(2000.0, 0.0, 0.0), 10.0), None);
    }

    #[test]
    fn a_rotated_joint_is_tested_in_its_own_space() {
        let mut f = Fighter::new(FighterKind::Mario, 0, 3);
        // Joint X runs along world Y.
        f.joint_transforms[14] = Some(JointTransform {
            axes: [
                Vec3::new(0.0, 1.0, 0.0),
                Vec3::new(-1.0, 0.0, 0.0),
                Vec3::new(0.0, 0.0, 1.0),
            ],
            origin: Vec3::ZERO,
        });
        let desc = MARIO[2];
        let t = f.joint_transforms[14].unwrap();
        // Offset 15 along joint X, half length 36: world Y from -21 to 51.
        assert!(box_contains(&t, &desc, Vec3::new(0.0, 50.0, 0.0), 0.0));
        assert!(!box_contains(&t, &desc, Vec3::new(0.0, 52.0, 0.0), 0.0));
        assert!(!box_contains(&t, &desc, Vec3::new(60.0, 0.0, 0.0), 0.0));
    }

    #[test]
    fn without_posed_joints_the_root_sphere_stands_in() {
        let f = Fighter::new(FighterKind::Mario, 0, 3);
        assert_eq!(
            hit_index(&f, Vec3::new(100.0, 0.0, 0.0), 30.0),
            Some(DAMAGE_INDEX_N)
        );
        assert_eq!(hit_index(&f, Vec3::new(200.0, 0.0, 0.0), 30.0), None);
    }

    #[test]
    fn only_grabbable_boxes_catch() {
        let mut f = Fighter::new(FighterKind::Mario, 0, 3);
        posed(&mut f, 6, Vec3::new(0.0, 100.0, 0.0), 1.0);
        posed(&mut f, 24, Vec3::new(0.0, -300.0, 0.0), 1.0);
        assert!(catch_touches(&f, Vec3::new(0.0, 100.0, 0.0), 10.0));
        // Mario's feet are hurtboxes but not grabbable.
        assert!(!catch_touches(&f, Vec3::new(0.0, -300.0, 0.0), 10.0));
    }
}
