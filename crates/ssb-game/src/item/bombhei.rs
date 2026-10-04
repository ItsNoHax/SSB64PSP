//! The Bob-omb, `itbombhei.c`. Left on the ground for 180 frames, it starts
//! walking towards the side with more fighters, turning at walls and floor
//! edges. 480 frames later it stops, flashes for 90 frames and explodes.
//! Thrown or dropped, it explodes on the first surface or target it meets,
//! and any hit sets it off.

use ssb_engine::math::Vec3;

use super::{
    map, HitProc, Item, ItemAttributes, ItemKind, ItemStatus, ItemType, ItemWeight, OwnerView,
};
use crate::colanim::ColAnimId;
use crate::combat::{AttackState, Element, HitStatus};
use crate::ground::BodyColl;
use crate::map::{MASK_FLOOR, MASK_LWALL, MASK_RWALL};
use crate::weapon::MapSurface;
use crate::wpeffect::{Emit, WeaponEffect as Fx};

/// `ITBOMBHEI_*`.
pub const EXPLODE_LIFETIME: u16 = 6;
pub const WALK_WAIT: u16 = 180;
pub const FLASH_WAIT: u16 = 480;
pub const SMOKE_WAIT: u16 = 4;
/// A float in the source, compared with the integer `multi`.
pub const EXPLODE_WAIT: u16 = 90;
pub const WALK_VEL_X: f32 = 24.0;
pub const GRAVITY: f32 = 1.2;
pub const TVEL: f32 = 100.0;
pub const MAP_REBOUND_COMMON: f32 = 0.4;
pub const MAP_REBOUND_GROUND: f32 = 0.3;
pub const EXPLODE_SCALE: f32 = 1.4;
/// `ITBOMBHEI_EXPLODE_COLANIM_DURATION`.
pub const EXPLODE_COLANIM_DURATION: i32 = 90;

/// File 251, `llITCommonDataBombHeiItemAttributes` (0x424).
pub static ATTRIBUTES: ItemAttributes = ItemAttributes {
    is_give_hitlag: true,
    is_display_colanim: true,
    weight: ItemWeight::Light,
    attack_offsets: [Vec3::ZERO; 2],
    damage_coll_offset: Vec3::ZERO,
    damage_coll_size: Vec3::new(150.0, 150.0, 150.0),
    map_coll: BodyColl {
        top: 150.0,
        center: 0.0,
        bottom: -150.0,
        width: 150.0,
    },
    size: 200.0,
    angle: 361,
    kb_scale: 100,
    damage: 1,
    element: Element::Normal,
    kb_weight: 0,
    shield_damage: 0,
    attack_count: 1,
    can_setoff: true,
    priority: 1,
    can_rehit_item: false,
    can_rehit_fighter: false,
    can_hop: true,
    can_reflect: true,
    can_shield: true,
    kb_base: 30,
    ty: ItemType::Throw,
    hitstatus: HitStatus::None,
    vel_scale: 100,
};

/// File 251, `llITCommonDataBombHeiAttackEvents` (0x46C): timer, angle,
/// damage, size.
pub const ATTACK_EVENTS: [(u16, i32, i32, f32); 4] = [
    (0, 361, 30, 350.0),
    (2, 361, 30, 250.0),
    (4, 361, 20, 150.0),
    (6, 361, 1, 0.0),
];

/// `itBombHeiStatus`, with the descriptor's procs as `Init`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Init,
    Wait,
    Fall,
    Hold,
    Thrown,
    Dropped,
    Walk,
    ExplodeMap,
    Explode,
    ExplodeWait,
}

/// `itBombHeiMakeItem`.
pub(super) fn make(pos: Vec3, vel: Vec3) -> Item {
    let mut item = Item::new(
        ItemKind::BombHei,
        &ATTRIBUTES,
        ItemStatus::BombHei(Status::Init),
        AttackState::Off,
        pos,
        vel,
        0,
    );
    item.multi = 0;
    item.clear_owner_stats();
    item.rotate_z = 0.0;
    // The tree's own list walks right.
    item.vars.bombhei_walk_right = true;
    item
}

fn set(item: &mut Item, s: Status) {
    item.set_status(ItemStatus::BombHei(s));
}

/// `itBombHeiCommonSetWalkLR`: `right` also picks the right-walking
/// display list.
fn set_walk_lr(item: &mut Item, right: bool) {
    if right {
        item.lr = 1.0;
        item.vel_air.x = WALK_VEL_X;
    } else {
        item.lr = -1.0;
        item.vel_air.x = -WALK_VEL_X;
    }
    item.vars.bombhei_walk_right = right;
}

/// `itBombHeiCommonCheckMakeDustEffect`.
fn dust(item: &Item, force: bool, fx: &mut Emit) {
    if item.mask_curr & MASK_FLOOR != 0 || force {
        let mut pos = item.pos;
        pos.y += item.attr.map_coll.bottom;
        fx.push(Fx::DustHeavyDouble {
            pos,
            lr: item.lr as i8,
        });
    }
}

/// `itBombHeiWalkGetLR`: the fighters to the left less those to the right.
fn walk_lr(item: &Item, owners: &[Option<OwnerView>; 4]) -> i32 {
    owners
        .iter()
        .flatten()
        .map(|v| if item.pos.x - v.pos.x < 0.0 { -1 } else { 1 })
        .sum()
}

/// `itBombHeiWaitSetStatus`.
fn wait(item: &mut Item) {
    item.set_ground_allow_pickup();
    item.damage_coll.hitstatus = HitStatus::Normal;
    set(item, Status::Wait);
}

/// `itBombHeiFallSetStatus`.
fn fall(item: &mut Item) {
    item.is_allow_pickup = false;
    map::set_air(item);
    item.damage_coll.hitstatus = HitStatus::Normal;
    set(item, Status::Fall);
}

pub(super) fn hold(item: &mut Item) {
    item.damage_coll.hitstatus = HitStatus::None;
    set(item, Status::Hold);
}

pub(super) fn thrown(item: &mut Item) {
    item.damage_coll.hitstatus = HitStatus::Normal;
    set(item, Status::Thrown);
}

pub(super) fn dropped(item: &mut Item) {
    item.damage_coll.hitstatus = HitStatus::Normal;
    set(item, Status::Dropped);
}

/// The floor-edge turn of `itBombHeiWalkProcUpdate`, and with
/// `left_only` that of `itBombHeiWalkInitVars`, which tests only a
/// left-walking Bob-omb against its line's left edge.
fn check_edge<I, F>(item: &mut Item, surfaces: &F, left_only: bool)
where
    F: Fn() -> I,
    I: IntoIterator<Item = MapSurface>,
{
    let Some(line) = item.floor_line() else {
        return;
    };
    if !crate::map::line_exists(surfaces, line) {
        return;
    }
    let width = item.attr.map_coll.width;
    if item.lr == -1.0 {
        if crate::map::floor_edge(surfaces, line, false).is_some_and(|e| e.x >= item.pos.x - width)
        {
            set_walk_lr(item, true);
        }
    } else if !left_only
        && crate::map::floor_edge(surfaces, line, true).is_some_and(|e| e.x <= item.pos.x + width)
    {
        set_walk_lr(item, false);
    }
}

/// `itBombHeiWalkSetStatus`. The walk's material script
/// (`llITCommonDataBombHeiWalkMatAnimJoint`) is display only.
fn walk<I, F>(item: &mut Item, surfaces: &F)
where
    F: Fn() -> I,
    I: IntoIterator<Item = MapSurface>,
{
    item.damage_coll.hitstatus = HitStatus::Normal;
    item.is_allow_pickup = false;
    item.multi = 0;
    item.vars.bombhei_smoke_delay = SMOKE_WAIT;
    item.refresh_attack_coll();
    item.add_root_script();
    check_edge(item, surfaces, true);
    item.clear_owner_stats();
    set(item, Status::Walk);
}

/// `itBombHeiWalkUpdateEffect`.
fn smoke(item: &mut Item, fx: &mut Emit) {
    if item.vars.bombhei_smoke_delay == 0 {
        let mut pos = item.pos;
        pos.y += 120.0;
        fx.push(Fx::DustLight {
            pos,
            lr: item.lr as i8,
        });
        item.vars.bombhei_smoke_delay = SMOKE_WAIT;
    }
    item.vars.bombhei_smoke_delay -= 1;
}

fn attack_event(item: &mut Item) {
    let (timer, angle, damage, size) = ATTACK_EVENTS[usize::from(item.event_id)];
    if item.multi == timer {
        item.attack.angle = angle;
        item.attack.damage = damage;
        item.attack.size = size;
        item.attack.can_rehit_item = true;
        item.attack.can_hop = false;
        item.attack.can_reflect = false;
        item.attack.can_setoff = false;
        item.attack.element = Element::Fire;
        item.event_id += 1;
        if item.event_id == 4 {
            item.event_id = 3;
        }
    }
}

/// `itBombHeiCommonClearVelSetExplode`.
fn explode(item: &mut Item, fx: &mut Emit) {
    item.vel_air = Vec3::ZERO;
    item.damage_coll.hitstatus = HitStatus::None;
    fx.push(Fx::ScaledExplosion {
        pos: item.pos,
        scale: EXPLODE_SCALE,
    });
    fx.push(Fx::Quake(1));
    item.hidden = true;
    item.refresh_attack_coll();
    item.clear_owner_stats();
    item.multi = 0;
    item.attack.throw_mul = 1.0;
    item.event_id = 0;
    attack_event(item);
    set(item, Status::Explode);
}

/// `itBombHeiExplodeWaitSetStatus`.
fn explode_wait(item: &mut Item) {
    item.damage_coll.hitstatus = HitStatus::Normal;
    item.multi = 0;
    // `dobj->mobj->matanim_joint.event32 = NULL`.
    item.clear_root_script();
    item.check_set_colanim(ColAnimId::ITEM_BOMB_HEI_CRITICAL, EXPLODE_COLANIM_DURATION);
    set(item, Status::ExplodeWait);
}

pub(super) fn update<I, F>(
    item: &mut Item,
    status: Status,
    owners: &[Option<OwnerView>; 4],
    surfaces: &F,
    fx: &mut Emit,
) -> bool
where
    F: Fn() -> I,
    I: IntoIterator<Item = MapSurface>,
{
    match status {
        Status::Init | Status::Fall | Status::Dropped | Status::Thrown => {
            item.apply_gravity_clamp_tvel(GRAVITY, TVEL);
            item.rotate_z += item.spin_step;
        }
        Status::Hold => {}
        // `itBombHeiWaitProcUpdate`: a tie draws the direction.
        Status::Wait => {
            if item.multi == WALK_WAIT {
                let mut lr = walk_lr(item, owners);
                if lr == 0 {
                    lr = crate::rng::rand_int_range(2) - 1;
                }
                if lr < 0 {
                    item.lr = 1.0;
                    item.vel_air.x = WALK_VEL_X;
                } else {
                    item.vel_air.x = -WALK_VEL_X;
                    item.vars.bombhei_walk_right = false;
                    item.lr = -1.0;
                }
                walk(item, surfaces);
            }
            item.multi += 1;
        }
        Status::Walk => {
            smoke(item, fx);
            check_edge(item, surfaces, false);
            if item.multi == FLASH_WAIT {
                item.vel_air = Vec3::ZERO;
                explode_wait(item);
            }
            item.multi += 1;
        }
        // `itBombHeiExplodeMapProcUpdate`.
        Status::ExplodeMap => {
            dust(item, false, fx);
            explode(item, fx);
        }
        Status::Explode => {
            attack_event(item);
            item.multi += 1;
            if item.multi == EXPLODE_LIFETIME {
                return false;
            }
        }
        Status::ExplodeWait => {
            smoke(item, fx);
            if item.multi == EXPLODE_WAIT {
                dust(item, true, fx);
                explode(item, fx);
            }
            item.multi += 1;
        }
    }
    true
}

pub(super) fn has_proc_map(status: Status) -> bool {
    !matches!(status, Status::Hold | Status::ExplodeMap | Status::Explode)
}

pub(super) fn proc_map<I, F>(item: &mut Item, status: Status, surfaces: &F) -> bool
where
    F: Fn() -> I,
    I: IntoIterator<Item = MapSurface>,
{
    match status {
        Status::Wait => {
            if !map::check_lr_wall_proc_no_floor(item, surfaces) {
                fall(item);
            }
        }
        Status::Init | Status::Fall => {
            let out =
                map::check_destroy_dropped(item, MAP_REBOUND_COMMON, MAP_REBOUND_GROUND, surfaces);
            if out.destroy {
                return false;
            }
            if out.goto_wait {
                wait(item);
            }
        }
        Status::Thrown | Status::Dropped => {
            if map::check_map_proc_all(item, surfaces) {
                // `itBombHeiExplodeMapSetStatus`.
                item.damage_coll.hitstatus = HitStatus::Normal;
                set(item, Status::ExplodeMap);
            }
        }
        // `itBombHeiWalkProcMap`: the walls turn it.
        Status::Walk => {
            if !map::check_lr_wall_proc_no_floor(item, surfaces) {
                dropped(item);
            }
            if item.mask_curr & MASK_LWALL != 0 {
                set_walk_lr(item, false);
            }
            if item.mask_curr & MASK_RWALL != 0 {
                set_walk_lr(item, true);
            }
        }
        Status::ExplodeWait => {
            if !map::check_lr_wall_proc_no_floor(item, surfaces) {
                dropped(item);
            }
        }
        Status::Hold | Status::ExplodeMap | Status::Explode => {}
    }
    true
}

pub(super) fn hit_proc(
    item: &mut Item,
    status: Status,
    proc: HitProc,
    reflector_lr: f32,
    fx: &mut Emit,
) -> Option<bool> {
    match (status, proc) {
        // `itBombHeiCommonProcHit`.
        (Status::Wait, HitProc::Hit | HitProc::Damage)
        | (Status::Fall, HitProc::Damage)
        | (
            Status::Thrown | Status::Dropped,
            HitProc::Hit | HitProc::Shield | HitProc::SetOff | HitProc::Damage,
        ) => explode(item, fx),
        (Status::Thrown | Status::Dropped, HitProc::Hop) => item.common_proc_hop(),
        (Status::Thrown | Status::Dropped, HitProc::Reflector) => {
            item.common_proc_reflector(reflector_lr)
        }
        // `itBombHeiExplodeCommonProcHit`.
        (
            Status::Walk | Status::ExplodeWait,
            HitProc::Hit | HitProc::Shield | HitProc::SetOff | HitProc::Reflector | HitProc::Damage,
        ) => {
            dust(item, true, fx);
            explode(item, fx);
        }
        // `itBombHeiExplodeMapProcUpdate` as a hit callback.
        (
            Status::ExplodeMap,
            HitProc::Hit | HitProc::Shield | HitProc::Reflector | HitProc::Damage,
        ) => {
            dust(item, false, fx);
            explode(item, fx);
        }
        _ => return None,
    }
    Some(true)
}
