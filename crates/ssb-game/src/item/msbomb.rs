//! The Motion-Sensor Bomb, `itmsbomb.c`. Thrown or dropped, it sticks to
//! the first surface it meets and turns to lie on it. After 100 frames
//! there, any fighter whose centre comes within 400 units sets it off, and
//! so does any damage. A bomb whose surface disappears falls and keeps
//! watching for fighters.

use ssb_engine::math::Vec3;

use super::{
    map, HitProc, Item, ItemAttributes, ItemKind, ItemStatus, ItemType, ItemWeight, OwnerView,
};
use crate::combat::{AttackState, Element, HitStatus};
use crate::ground::BodyColl;
use crate::map::{MASK_CEIL, MASK_FLOOR, MASK_LWALL, MASK_RWALL};
use crate::weapon::MapSurface;
use crate::wpeffect::{Emit, WeaponEffect as Fx};

/// `ITMSBOMB_*`.
pub const EXPLODE_LIFETIME: u16 = 16;
pub const DETECT_FIGHTER_DELAY: u16 = 100;
/// `SQUARE(400.0F)`, compared with the squared distance.
pub const DETECT_FIGHTER_RADIUS: f32 = 400.0 * 400.0;
pub const GRAVITY: f32 = 1.5;
pub const TVEL: f32 = 80.0;
pub const MAP_REBOUND_COMMON: f32 = 0.4;
pub const MAP_REBOUND_GROUND: f32 = 0.3;
pub const COLL_SIZE: f32 = 30.0;
pub const EXPLODE_SCALE: f32 = 1.2;

/// File 251, `llITCommonDataMSBombItemAttributes` (0x3BC).
pub static ATTRIBUTES: ItemAttributes = ItemAttributes {
    is_give_hitlag: true,
    weight: ItemWeight::Light,
    attack_offsets: [Vec3::ZERO; 2],
    damage_coll_offset: Vec3::ZERO,
    damage_coll_size: Vec3::new(150.0, 150.0, 150.0),
    map_coll: BodyColl {
        top: 60.0,
        center: 0.0,
        bottom: -60.0,
        width: 60.0,
    },
    size: 200.0,
    angle: 361,
    kb_scale: 100,
    damage: 5,
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
    kb_base: 10,
    ty: ItemType::Throw,
    hitstatus: HitStatus::None,
    vel_scale: 90,
};

/// File 251, `llITCommonDataMSBombAttackEvents` (0x404): timer, angle,
/// damage, size.
pub const ATTACK_EVENTS: [(u16, i32, i32, f32); 4] = [
    (0, 361, 30, 360.0),
    (4, 361, 30, 300.0),
    (8, 361, 20, 200.0),
    (16, 361, 1, 0.0),
];

/// `itMSBombStatus`, with the descriptor's procs as `Init`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Init,
    Wait,
    Fall,
    Hold,
    Thrown,
    Dropped,
    Attached,
    Detached,
    Explode,
}

/// `itMSBombMakeItem`.
pub(super) fn make(pos: Vec3, vel: Vec3) -> Item {
    let mut item = Item::new(
        ItemKind::MSBomb,
        &ATTRIBUTES,
        ItemStatus::MSBomb(Status::Init),
        AttackState::Off,
        pos,
        vel,
        0,
    );
    item.multi = 0;
    item.rotate_z = 0.0;
    item.vars.msbomb_attached = false;
    item
}

fn set(item: &mut Item, s: Status) {
    item.set_status(ItemStatus::MSBomb(s));
}

fn set_coll_size(item: &mut Item) {
    item.coll = BodyColl {
        top: COLL_SIZE,
        center: 0.0,
        bottom: -COLL_SIZE,
        width: COLL_SIZE,
    };
}

/// `itMSBombWaitSetStatus`.
fn wait(item: &mut Item) {
    item.set_ground_allow_pickup();
    set(item, Status::Wait);
}

/// `itMSBombFallSetStatus`.
fn fall(item: &mut Item) {
    item.is_allow_pickup = false;
    map::set_air(item);
    set(item, Status::Fall);
}

pub(super) fn hold(item: &mut Item) {
    set(item, Status::Hold);
}

pub(super) fn thrown(item: &mut Item) {
    set_coll_size(item);
    set(item, Status::Thrown);
}

pub(super) fn dropped(item: &mut Item) {
    set_coll_size(item);
    set(item, Status::Dropped);
}

/// `itMSBombAttachedUpdateSurface`: a floor or ceiling wins over a wall,
/// and a floor over a ceiling.
fn update_surface(item: &mut Item) {
    let mask = item.mask_curr;
    let mut angle = None;
    if mask & (MASK_CEIL | MASK_FLOOR) != 0 {
        if mask & MASK_CEIL != 0 {
            angle = Some(item.ceil_normal);
            item.attach_line = Some(item.ceil_line);
        }
        if mask & MASK_FLOOR != 0 {
            if let Some(f) = item.floor {
                angle = Some(f.normal);
                item.attach_line = Some(f.line);
            }
        }
    } else {
        if mask & MASK_LWALL != 0 {
            angle = Some(item.lwall_normal);
            item.attach_line = Some(item.lwall_line);
        }
        if mask & MASK_RWALL != 0 {
            angle = Some(item.rwall_normal);
            item.attach_line = Some(item.rwall_line);
        }
    }
    if let Some(a) = angle {
        item.rotate_z = ssb_engine::math::atan2(a.y, a.x) - core::f32::consts::FRAC_PI_2;
    }
}

/// `itMSBombAttachedSetStatus`.
fn attach(item: &mut Item) {
    set_coll_size(item);
    item.vel_air = Vec3::ZERO;
    // The armed shape shows and the ball hides.
    item.vars.msbomb_attached = true;
    update_surface(item);
    item.damage_coll.hitstatus = HitStatus::Normal;
    item.attack.state = AttackState::Off;
    item.clear_owner_stats();
    set(item, Status::Attached);
}

/// `itMSBombDetachedSetStatus`.
fn detach(item: &mut Item) {
    item.damage_coll.hitstatus = HitStatus::Normal;
    item.attack.state = AttackState::Off;
    item.clear_owner_stats();
    set(item, Status::Detached);
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

/// `itMSBombExplodeInitStatusVars`.
fn explode(item: &mut Item, is_make_effect: bool, fx: &mut Emit) {
    if is_make_effect && item.mask_curr & MASK_FLOOR != 0 {
        let mut pos = item.pos;
        pos.y += item.attr.map_coll.bottom;
        fx.push(Fx::DustHeavyDouble {
            pos,
            lr: item.lr as i8,
        });
    }
    fx.push(Fx::ScaledExplosion {
        pos: item.pos,
        scale: EXPLODE_SCALE,
    });
    fx.push(Fx::Quake(1));
    item.refresh_attack_coll();
    item.multi = 0;
    item.event_id = 0;
    item.attack.throw_mul = 1.0;
    item.damage_coll.hitstatus = HitStatus::None;
    attack_event(item);
    set(item, Status::Explode);
    item.hidden = true;
}

/// The proximity check of `itMSBombAttachedProcUpdate` and
/// `itMSBombDetachedProcUpdate`. Every fighter in range sets the bomb off
/// again, as the source's loop does not stop at the first.
fn detect(item: &mut Item, owners: &[Option<OwnerView>; 4], is_make_effect: bool, fx: &mut Emit) {
    if item.multi < DETECT_FIGHTER_DELAY {
        item.multi += 1;
        return;
    }
    let at = item.pos;
    for view in owners.iter().flatten() {
        let mut pos = view.pos;
        pos.y += view.coll.top * 0.5;
        let d = pos - at;
        if d.x * d.x + d.y * d.y + d.z * d.z < DETECT_FIGHTER_RADIUS {
            explode(item, is_make_effect, fx);
        }
    }
}

pub(super) fn update(
    item: &mut Item,
    status: Status,
    owners: &[Option<OwnerView>; 4],
    fx: &mut Emit,
) -> bool {
    match status {
        Status::Init | Status::Fall | Status::Dropped | Status::Thrown => {
            item.apply_gravity_clamp_tvel(GRAVITY, TVEL);
            item.rotate_z += item.spin_step;
        }
        Status::Wait | Status::Hold => {}
        Status::Attached => detect(item, owners, true, fx),
        Status::Detached => {
            item.apply_gravity_clamp_tvel(GRAVITY, TVEL);
            detect(item, owners, false, fx);
        }
        Status::Explode => {
            attack_event(item);
            item.multi += 1;
            if item.multi == EXPLODE_LIFETIME {
                return false;
            }
        }
    }
    true
}

pub(super) fn has_proc_map(status: Status) -> bool {
    !matches!(status, Status::Hold | Status::Explode)
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
        // `itMSBombThrownProcMap`, `itMSBombDroppedProcMap`; the Detached
        // status reuses the latter.
        Status::Thrown | Status::Dropped | Status::Detached => {
            if map::check_map_proc_all(item, surfaces) {
                attach(item);
            }
        }
        Status::Attached => {
            if !item
                .attach_line
                .is_some_and(|line| crate::map::line_exists(surfaces, line))
            {
                item.attach_line = None;
                detach(item);
            }
        }
        Status::Hold | Status::Explode => {}
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
        // `itMSBombCommonProcHit` for the hit, shield and set-off.
        (Status::Thrown | Status::Dropped, HitProc::Hit | HitProc::Shield | HitProc::SetOff) => {
            item.vel_set_rebound()
        }
        (Status::Thrown | Status::Dropped, HitProc::Hop) => item.common_proc_hop(),
        (Status::Thrown | Status::Dropped, HitProc::Reflector) => {
            item.common_proc_reflector(reflector_lr)
        }
        // `itMSBombCommonProcDamage`.
        (Status::Attached | Status::Detached, HitProc::Damage) => explode(item, false, fx),
        _ => return None,
    }
    Some(true)
}
