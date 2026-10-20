//! `itmap.c`: the item map callbacks, over the shared map sweeps of
//! [`crate::map`]. An airborne item takes `mpProcessUpdateMain` with
//! `itMapProcAllCheckCollisionFlag` ([`crate::map::move_air`]), a grounded
//! one `itMapProcLRWallCheckFloor` ([`crate::map::move_ground`]); the
//! masks and surface angles they leave drive the rebounds.

use ssb_engine::math::{Vec2, Vec3};

use super::{
    Ga, Item, LANDING_DESPAWN_CHECK, LANDING_NUM_MAX, THROW_DESPAWN_RANDOM, THROW_NUM_MAX,
};
use crate::ground::{BodyColl, Standing};
use crate::map::{AirOptions, MASK_CEIL, MASK_FLOOR, MASK_LWALL, MASK_RWALL};
use crate::weapon::MapSurface;

/// `MAP_FLAG_MAIN_MASK`.
pub(crate) const MASK_MAIN: u16 = MASK_LWALL | MASK_RWALL | MASK_CEIL | MASK_FLOOR;

/// `itMapSetGround`.
pub(crate) fn set_ground(item: &mut Item) {
    item.ga = Ga::Ground;
    item.vel_ground = item.vel_air.x * item.lr;
}

/// `itMapSetAir`.
pub(crate) fn set_air(item: &mut Item) {
    item.ga = Ga::Air;
}

/// `itMapTestLRWallCheckFloor`: the walls, then the floor under a grounded
/// item. Returns whether it is still on a floor.
pub(crate) fn test_lr_wall_check_floor<I, F>(item: &mut Item, surfaces: &F) -> bool
where
    F: Fn() -> I,
    I: IntoIterator<Item = MapSurface>,
{
    let Some(floor) = item.floor.standing() else {
        return false;
    };
    let (moved, contacts) = crate::map::move_ground(
        &item.coll,
        item.pos_prev,
        item.pos - crate::map::line_speed(surfaces, floor.line),
        floor.line,
        false,
        surfaces,
    );
    item.pos = moved.pos;
    if let Some(hit) = contacts.left_wall {
        item.mask_curr |= MASK_LWALL;
        item.lwall_normal = hit.normal;
        item.lwall_line = hit.line;
    }
    if let Some(hit) = contacts.right_wall {
        item.mask_curr |= MASK_RWALL;
        item.rwall_normal = hit.normal;
        item.rwall_line = hit.line;
    }
    // Standing on the same line is `mask_stat` only; `MAP_FLAG_FLOOR` in
    // `mask_curr` marks the swept change of floor line
    // (`mpProcessCheckTestFloorCollision`, mpprocess.c:1020; RE-554).
    if contacts.floor {
        item.mask_curr |= MASK_FLOOR;
    }
    match moved.floor {
        Some(f) => {
            item.floor.set(f);
            true
        }
        None => false,
    }
}

/// `itMapCheckLRWallProcNoFloor`: returns whether the item stayed on a
/// floor; otherwise the caller runs its fall callback.
pub(crate) fn check_lr_wall_proc_no_floor<I, F>(item: &mut Item, surfaces: &F) -> bool
where
    F: Fn() -> I,
    I: IntoIterator<Item = MapSurface>,
{
    test_lr_wall_check_floor(item, surfaces)
}

/// `itMapTestAllCollisionFlag`: the airborne sweep. Walls and ceilings stop
/// the item; a floor lands it (`mpProcessSetLandingFloor`). Returns whether
/// any of `flags` was touched.
pub(crate) fn test_all_collision_flag<I, F>(item: &mut Item, flags: u16, surfaces: &F) -> bool
where
    F: Fn() -> I,
    I: IntoIterator<Item = MapSurface>,
{
    let result = crate::map::move_air(
        &item.coll,
        item.pos_prev,
        item.pos,
        AirOptions::default(),
        surfaces,
    );
    item.pos = result.moved.pos;
    if let Some(hit) = result.contacts.left_wall {
        item.mask_curr |= MASK_LWALL;
        item.lwall_normal = hit.normal;
        item.lwall_line = hit.line;
    }
    if let Some(hit) = result.contacts.right_wall {
        item.mask_curr |= MASK_RWALL;
        item.rwall_normal = hit.normal;
        item.rwall_line = hit.line;
    }
    if let Some(hit) = result.contacts.ceiling {
        item.mask_curr |= MASK_CEIL;
        item.ceil_normal = hit.normal;
        item.ceil_line = hit.line;
    }
    if let Some(f) = result.moved.floor {
        item.mask_curr |= MASK_FLOOR;
        item.floor.set(f);
        crate::sound::play_fgm(crate::sound::id::nSYAudioFGMItemMapCollide);
        // The landing stops the item's spin and squares its root.
        item.spin_step = 0.0;
        item.rotate_z = 0.0;
    }
    item.mask_curr & flags != 0
}

/// `itMapTestAllCheckCollEnd`: the walls, ceiling and floor are only
/// tested ([`crate::map::move_air_check_coll_end`]); `mpProcessUpdateMain`
/// keeps the substep that met one (`is_coll_end`), and the item stays
/// airborne. A floor its bottom crossed is noted
/// (`mpProcessRunFloorCollisionAdjNewNULL`), and its spin and root roll
/// stop. Returns `is_coll_end`.
pub(crate) fn test_all_check_coll_end<I, F>(item: &mut Item, surfaces: &F) -> bool
where
    F: Fn() -> I,
    I: IntoIterator<Item = MapSurface>,
{
    let end = crate::map::move_air_check_coll_end(&item.coll, item.pos_prev, item.pos, surfaces);
    item.pos = end.pos;
    item.mask_curr |= end.mask;
    if let Some(hit) = end.floor {
        item.floor.set(Standing {
            line: hit.line,
            flags: hit.flags,
            normal: hit.normal,
        });
        crate::sound::play_fgm(crate::sound::id::nSYAudioFGMItemMapCollide);
        item.spin_step = 0.0;
        item.rotate_z = 0.0;
    }
    end.mask != 0
}

fn dot(v: Vec3, n: Vec2) -> f32 {
    v.x * n.x + v.y * n.y
}

/// `lbCommonReflect2D`.
pub(super) fn reflect(v: &mut Vec3, n: Vec2) {
    let d = dot(*v, n) * 2.0;
    v.x -= d * n.x;
    v.y -= d * n.y;
}

/// `lbCommonScale2D`.
fn scale(v: &mut Vec3, s: f32) {
    v.x *= s;
    v.y *= s;
}

/// `itMapCheckCollideAllRebound`: a surface first touched this frame that
/// the item was moving into turns its velocity, scaled by `mod_vel`. Each
/// wall or floor it turns from plays `nSYAudioFGMItemMapCollide`; a ceiling
/// is silent.
pub(crate) fn check_collide_all_rebound(item: &mut Item, check: u16, mod_vel: f32) -> bool {
    let fresh = (item.mask_prev ^ item.mask_curr) & item.mask_curr & MASK_MAIN;
    let mut hit = false;
    let floor_normal = item.floor.normal();
    for (mask, normal) in [
        (MASK_LWALL, item.lwall_normal),
        (MASK_RWALL, item.rwall_normal),
        (MASK_CEIL, item.ceil_normal),
        (MASK_FLOOR, floor_normal),
    ] {
        if fresh & check & mask != 0 && dot(item.vel_air, normal) < 0.0 {
            reflect(&mut item.vel_air, normal);
            hit = true;
            if mask != MASK_CEIL {
                crate::sound::play_fgm(crate::sound::id::nSYAudioFGMItemMapCollide);
            }
        }
    }
    if hit {
        scale(&mut item.vel_air, mod_vel);
    }
    hit
}

/// `itMapSetGroundRebound`.
fn set_ground_rebound(vel: &mut Vec3, floor: Vec2, ground_rebound: f32) {
    let mag = Vec2::new(vel.x, vel.y).length();
    if mag != 0.0 {
        let inverse = 1.0 / mag;
        let rebound = mag * ground_rebound * 0.5;
        reflect(vel, floor);
        vel.x *= inverse;
        vel.y *= inverse;
        vel.x += floor.x;
        vel.y += floor.y;
        vel.x *= rebound;
        vel.y *= rebound;
    }
}

/// What [`check_destroy_dropped`] asks of its caller.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Dropped {
    /// The source's `TRUE` return (a random despawn); every ported caller
    /// ignores it.
    pub destroy: bool,
    /// The second landing: run the status callback.
    pub goto_wait: bool,
}

/// `itMapCheckDestroyDropped`. A thrown item's first landing may despawn
/// it: always after `ITEM_THROW_NUM_MAX` throws, otherwise one time in
/// four, drawn from the shared generator.
pub(crate) fn check_destroy_dropped<I, F>(
    item: &mut Item,
    common_rebound: f32,
    ground_rebound: f32,
    surfaces: &F,
) -> Dropped
where
    F: Fn() -> I,
    I: IntoIterator<Item = MapSurface>,
{
    let mut out = Dropped {
        destroy: false,
        goto_wait: false,
    };
    let is_collide_floor = test_all_collision_flag(item, MASK_FLOOR, surfaces);
    if check_collide_all_rebound(item, MASK_CEIL | MASK_RWALL | MASK_LWALL, common_rebound) {
        item.set_spin_vel_lr();
    }
    if is_collide_floor {
        let normal = item.floor.normal();
        set_ground_rebound(&mut item.vel_air, normal, ground_rebound);
        item.set_spin_vel_lr();
        item.times_landed = (item.times_landed + 1) & 3;
        if item.times_landed == LANDING_DESPAWN_CHECK
            && !is_explain()
            && item.times_thrown != 0
            && (item.times_thrown == THROW_NUM_MAX
                || crate::rng::rand_int_range(THROW_DESPAWN_RANDOM) == 0)
        {
            out.destroy = true;
            return out;
        }
        if item.times_landed == LANDING_NUM_MAX {
            out.goto_wait = true;
        }
    }
    out
}

/// `itMapCheckLanding`: returns whether the item landed (the caller runs
/// its callback).
pub(crate) fn check_landing<I, F>(
    item: &mut Item,
    common_rebound: f32,
    ground_rebound: f32,
    surfaces: &F,
) -> bool
where
    F: Fn() -> I,
    I: IntoIterator<Item = MapSurface>,
{
    let is_collide_floor = test_all_collision_flag(item, MASK_FLOOR, surfaces);
    if check_collide_all_rebound(item, MASK_CEIL | MASK_RWALL | MASK_LWALL, common_rebound) {
        item.set_spin_vel_lr();
    }
    if is_collide_floor {
        let normal = item.floor.normal();
        reflect(&mut item.vel_air, normal);
        scale(&mut item.vel_air, ground_rebound);
        item.set_spin_vel_lr();
        return true;
    }
    false
}

/// `itMapCheckMapReboundProcAll`: returns whether any surface was touched
/// (the caller runs its callback).
pub(crate) fn check_map_rebound_proc_all<I, F>(
    item: &mut Item,
    common_rebound: f32,
    ground_rebound: f32,
    surfaces: &F,
) -> bool
where
    F: Fn() -> I,
    I: IntoIterator<Item = MapSurface>,
{
    let is_collide_any = test_all_collision_flag(item, MASK_MAIN, surfaces);
    if check_collide_all_rebound(item, MASK_CEIL | MASK_RWALL | MASK_LWALL, common_rebound) {
        item.set_spin_vel_lr();
    }
    if item.mask_curr & MASK_FLOOR != 0 {
        let normal = item.floor.normal();
        reflect(&mut item.vel_air, normal);
        scale(&mut item.vel_air, ground_rebound);
        item.set_spin_vel_lr();
    }
    is_collide_any
}

/// `itMapCheckDestroyLanding`: returns whether the item met a floor (the
/// source's `TRUE`, which destroys it).
pub(crate) fn check_destroy_landing<I, F>(
    item: &mut Item,
    common_rebound: f32,
    surfaces: &F,
) -> bool
where
    F: Fn() -> I,
    I: IntoIterator<Item = MapSurface>,
{
    let is_collide_floor = test_all_collision_flag(item, MASK_FLOOR, surfaces);
    if check_collide_all_rebound(item, MASK_CEIL | MASK_RWALL | MASK_LWALL, common_rebound) {
        item.set_spin_vel_lr();
    }
    is_collide_floor
}

/// `itMapCheckMapProcAll`: returns whether any surface was touched (the
/// caller runs its callback); the source itself always returns `FALSE`.
pub(crate) fn check_map_proc_all<I, F>(item: &mut Item, surfaces: &F) -> bool
where
    F: Fn() -> I,
    I: IntoIterator<Item = MapSurface>,
{
    test_all_collision_flag(item, MASK_MAIN, surfaces)
}

/// `itMapCheckMapReboundProcNoFloor`: `on_floor` runs on a floor contact,
/// before the walls and ceiling rebound the item.
pub(crate) fn check_map_rebound_proc_no_floor<I, F>(
    item: &mut Item,
    common_rebound: f32,
    surfaces: &F,
    on_floor: impl FnOnce(&mut Item),
) where
    F: Fn() -> I,
    I: IntoIterator<Item = MapSurface>,
{
    if test_all_collision_flag(item, MASK_FLOOR, surfaces) {
        on_floor(item);
    }
    if check_collide_all_rebound(item, MASK_CEIL | MASK_RWALL | MASK_LWALL, common_rebound) {
        item.set_spin_vel_lr();
    }
}

/// `mpCommonRunItemCollisionDefault` (mpcommon.c:970): one
/// `mpCommonRunDefaultCollision` pass from the parent's translate, probed
/// with the parent's box, to the item's, probed with its own. A floor the
/// bottom crossed puts the item on it without landing it; failing that, the
/// floor under it is only projected (`mpProcessSetCollProjectFloorID`).
pub(crate) fn run_default_collision<I, F>(
    item: &mut Item,
    parent_pos: Vec3,
    parent_coll: BodyColl,
    surfaces: &F,
) where
    F: Fn() -> I,
    I: IntoIterator<Item = MapSurface>,
{
    let result = crate::map::run_default_collision_project(
        &item.coll,
        &parent_coll,
        parent_pos,
        item.pos,
        surfaces,
    );
    item.pos = result.pos;
    item.floor.project(result.floor);
}

game_static! {
    /// `gSCManagerBattleState->game_type == nSCBattleGameTypeExplain`: How to
    /// Play's thrown items never vanish on landing (`itMapCheckCollideAllRebound`'s
    /// despawn check).
    static IS_EXPLAIN: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);
}

/// Sets the battle's game type for the landing check: How to Play's or any
/// other.
pub fn set_explain(is_explain: bool) {
    IS_EXPLAIN.with(|v| v.store(is_explain, core::sync::atomic::Ordering::Relaxed));
}

fn is_explain() -> bool {
    IS_EXPLAIN.with(|v| v.load(core::sync::atomic::Ordering::Relaxed))
}
