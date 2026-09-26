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
    let mut grabbable = damage_colls(f.kind)
        .unwrap_or(&[])
        .iter()
        .copied()
        .filter(|desc| desc.is_grabbable);
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
