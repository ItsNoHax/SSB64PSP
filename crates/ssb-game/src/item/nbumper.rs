//! The Bumper item, `itnbumper.c`. Thrown or dropped, it hits fighters
//! only, bounces off walls and ceilings, and settles on the first floor it
//! lands on. There it knocks back whoever touches it for 360 frames, then
//! flashes for 60 frames and vanishes. A hit in the air sends it flying
//! back with heavier gravity.
//!
//! `damage_all_delay` is a `u16` that the source compares with -1, which
//! never matches, so after the delay the countdown wraps and the owner is
//! cleared again every 65,536 frames.

use ssb_engine::math::Vec3;

use super::{map, HitProc, Item, ItemAttributes, ItemKind, ItemStatus, ItemType, ItemWeight};
use crate::combat::{AttackState, Element, HitStatus};
use crate::ground::BodyColl;
use crate::weapon::MapSurface;

/// `ITBUMPER_*`.
pub const LIFETIME: i32 = 360;
pub const DESPAWN_TIMER: i32 = 60;
pub const STOPVEL_WAIT: u16 = 4;
pub const DAMAGE_ALL_WAIT: u16 = 16;
pub const HIT_SCALE: u16 = 10;
pub const HIT_ANIM_LENGTH: u16 = 3;
pub const COLL_SIZE: f32 = 120.0;
pub const REBOUND_VEL_X: f32 = -100.0;
pub const REBOUND_AIR_X: f32 = -400.0;
pub const REBOUND_AIR_Y: f32 = 200.0;
pub const GRAVITY_NORMAL: f32 = 1.4;
pub const GRAVITY_HIT: f32 = 4.0;
pub const TVEL: f32 = 80.0;
pub const MAP_REBOUND_COMMON: f32 = 0.8;
pub const MAP_REBOUND_GROUND: f32 = 0.8;

/// File 251, `llITCommonDataNBumperItemAttributes` (0x69C).
pub static ATTRIBUTES: ItemAttributes = ItemAttributes {
    sounds: crate::item_sounds::item::NBUMPER,
    is_give_hitlag: true,
    is_display_colanim: false,
    weight: ItemWeight::Light,
    attack_offsets: [Vec3::ZERO; 2],
    damage_coll_offset: Vec3::ZERO,
    damage_coll_size: Vec3::new(150.0, 150.0, 150.0),
    map_coll: BodyColl {
        top: 180.0,
        center: 0.0,
        bottom: -180.0,
        width: 180.0,
    },
    size: 200.0,
    angle: 362,
    kb_scale: 50,
    damage: 1,
    element: Element::Normal,
    kb_weight: 250,
    shield_damage: 0,
    attack_count: 1,
    can_setoff: false,
    priority: 1,
    can_rehit_item: false,
    can_rehit_fighter: true,
    can_hop: true,
    can_reflect: true,
    can_shield: true,
    kb_base: 0,
    ty: ItemType::Throw,
    hitstatus: HitStatus::None,
    vel_scale: 100,
};

/// `itNBumperStatus`, with the descriptor's procs as `Init`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Init,
    Wait,
    Fall,
    Hold,
    Thrown,
    Dropped,
    Attached,
    HitAir,
    GDisappear,
}

/// `itNBumperMakeItem`.
pub(super) fn make(pos: Vec3, vel: Vec3) -> Item {
    let mut item = Item::new(
        ItemKind::NBumper,
        &ATTRIBUTES,
        ItemStatus::NBumper(Status::Init),
        AttackState::Off,
        pos,
        vel,
        0,
    );
    item.multi = 0;
    item.attack.interact_mask = super::INTERACT_FIGHTER;
    item.attack.can_rehit_shield = true;
    item.palette = 0;
    item.rotate_z = 0.0;
    item
}

fn set(item: &mut Item, s: Status) {
    item.set_status(ItemStatus::NBumper(s));
}

fn set_scale(item: &mut Item, s: f32) {
    item.scale = Vec3::new(s, s, s);
}

/// The swell after a hit: `2 - (10 - multi) / 10`, down to 1.
fn swell(item: &mut Item) {
    if item.multi != 0 {
        set_scale(item, 2.0 - (10 - i32::from(item.multi)) as f32 * 0.1);
        item.multi -= 1;
    } else {
        set_scale(item, 1.0);
    }
}

/// The owner countdown of the airborne updates.
fn damage_all_delay(item: &mut Item) {
    if item.vars.bumper_damage_all_delay == 0 {
        item.clear_owner_stats();
        item.vars.bumper_damage_all_delay = u16::MAX;
    }
    item.vars.bumper_damage_all_delay = item.vars.bumper_damage_all_delay.wrapping_sub(1);
}

/// The lit palette lasts `hit_anim_length` frames.
fn unlight(item: &mut Item) {
    if item.vars.bumper_hit_anim_length == 0 && item.palette == 1 {
        item.palette = 0;
    } else {
        item.vars.bumper_hit_anim_length = item.vars.bumper_hit_anim_length.wrapping_sub(1);
    }
}

/// `itNBumperWaitSetStatus`.
fn wait(item: &mut Item) {
    item.set_ground_allow_pickup();
    set(item, Status::Wait);
}

/// `itNBumperFallSetStatus`.
fn fall(item: &mut Item) {
    item.is_allow_pickup = false;
    map::set_air(item);
    set(item, Status::Fall);
}

pub(super) fn hold(item: &mut Item) {
    set(item, Status::Hold);
}

fn release(item: &mut Item, s: Status) {
    item.vars.bumper_damage_all_delay = DAMAGE_ALL_WAIT;
    item.coll.top = COLL_SIZE;
    item.coll.bottom = -COLL_SIZE;
    set(item, s);
}
pub(super) fn thrown(item: &mut Item) {
    release(item, Status::Thrown);
}
pub(super) fn dropped(item: &mut Item) {
    release(item, Status::Dropped);
}

/// `itNBumperAttachedSetModelPitch`: the model lies along the floor.
fn set_model_pitch(item: &mut Item) {
    if let Some(f) = item.floor {
        item.attach_line = Some(f.line);
        item.rotate_z =
            ssb_engine::math::atan2(f.normal.y, f.normal.x) - core::f32::consts::FRAC_PI_2;
    }
}

/// `itNBumperAttachedSetStatus`. The flat model and its material swap in.
fn attach(item: &mut Item) {
    item.vel_air = Vec3::ZERO;
    item.vars.bumper_attached = true;
    set_scale(item, 1.0);
    item.coll.top = COLL_SIZE;
    item.coll.bottom = -COLL_SIZE;
    set_model_pitch(item);
    item.lifetime = LIFETIME;
    item.clear_owner_stats();
    set(item, Status::Attached);
}

/// `itNBumperHitAirSetStatus`.
fn hit_air(item: &mut Item) {
    item.vars.bumper_damage_all_delay = DAMAGE_ALL_WAIT;
    set(item, Status::HitAir);
}

/// `itNBumperGDisappearSetStatus`.
fn disappear(item: &mut Item) {
    item.palette = 0;
    set_scale(item, 1.0);
    item.lifetime = DESPAWN_TIMER;
    item.hidden = false;
    item.attack.state = AttackState::Off;
    item.vel_air = Vec3::ZERO;
    set(item, Status::GDisappear);
}

pub(super) fn update<I, F>(item: &mut Item, status: Status, surfaces: &F) -> bool
where
    F: Fn() -> I,
    I: IntoIterator<Item = MapSurface>,
{
    match status {
        Status::Wait | Status::Hold => {}
        // `itNBumperFallProcUpdate`.
        Status::Init | Status::Fall => {
            item.apply_gravity_clamp_tvel(GRAVITY_NORMAL, TVEL);
            swell(item);
            damage_all_delay(item);
            item.rotate_z += item.spin_step;
        }
        // `itNBumperThrownProcUpdate`.
        Status::Thrown | Status::Dropped => {
            item.apply_gravity_clamp_tvel(GRAVITY_NORMAL, TVEL);
            damage_all_delay(item);
            item.rotate_z += item.spin_step;
        }
        // `itNBumperAttachedProcUpdate`: it slides back from a hit, but
        // stops at the floor's edges and when the swell is nearly over.
        Status::Attached => {
            unlight(item);
            if let Some(line) = item.floor_line() {
                if crate::map::line_exists(surfaces, line) {
                    let width = item.attr.map_coll.width;
                    if item.lr == -1.0 {
                        if crate::map::floor_edge(surfaces, line, false)
                            .is_some_and(|e| e.x >= item.pos.x - width)
                        {
                            item.vel_air.x = 0.0;
                        }
                    } else if crate::map::floor_edge(surfaces, line, true)
                        .is_some_and(|e| e.x <= item.pos.x + width)
                    {
                        item.vel_air.x = 0.0;
                    }
                }
            }
            if item.multi < STOPVEL_WAIT {
                item.vel_air.x = 0.0;
            }
            // The grounded swell leaves Y alone.
            if item.multi != 0 {
                let s = 2.0 - (10 - i32::from(item.multi)) as f32 * 0.1;
                item.scale.x = s;
                item.scale.z = s;
                item.multi -= 1;
            } else {
                set_scale(item, 1.0);
            }
            if item.lifetime == 0 {
                disappear(item);
            }
            item.lifetime -= 1;
        }
        // `itNBumperHitAirProcUpdate`.
        Status::HitAir => {
            unlight(item);
            item.apply_gravity_clamp_tvel(GRAVITY_HIT, TVEL);
            swell(item);
            damage_all_delay(item);
        }
        // `itNBumperGDisappearProcUpdate`: the model blinks.
        Status::GDisappear => {
            if item.lifetime == 0 {
                return false;
            }
            if item.lifetime % 2 != 0 {
                item.hidden = !item.hidden;
            }
            item.lifetime -= 1;
        }
    }
    true
}

pub(super) fn has_proc_map(status: Status) -> bool {
    !matches!(status, Status::Hold | Status::GDisappear)
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
        // `itNBumperThrownProcMap`.
        Status::Thrown | Status::Dropped | Status::HitAir => {
            map::check_map_rebound_proc_no_floor(item, 0.8, surfaces, attach);
        }
        // `itNBumperAttachedProcMap`.
        Status::Attached => {
            if map::check_lr_wall_proc_no_floor(item, surfaces) {
                if !item
                    .attach_line
                    .is_some_and(|line| crate::map::line_exists(surfaces, line))
                {
                    item.attach_line = None;
                    dropped(item);
                    // US only: the swell and the light end.
                    set_scale(item, 1.0);
                    item.palette = 0;
                } else if item.multi == 0 {
                    set_model_pitch(item);
                }
            } else {
                dropped(item);
            }
        }
        Status::Hold | Status::GDisappear => {}
    }
    true
}

/// `itNBumperThrownProcHit`.
fn thrown_hit(item: &mut Item) {
    set_scale(item, 2.0);
    item.vars.bumper_hit_anim_length = HIT_ANIM_LENGTH;
    item.palette = 1;
    item.vel_air.x = REBOUND_AIR_X * item.hit_lr;
    item.vel_air.y = REBOUND_AIR_Y;
    item.multi = HIT_SCALE;
    hit_air(item);
}

pub(super) fn hit_proc(
    item: &mut Item,
    status: Status,
    proc: HitProc,
    reflector_lr: f32,
) -> Option<bool> {
    let airborne = matches!(status, Status::Thrown | Status::Dropped | Status::HitAir);
    match (status, proc) {
        (_, HitProc::Hit) if airborne => thrown_hit(item),
        // `itNBumperThrownProcShield`.
        (_, HitProc::Shield) if airborne => {
            item.vel_set_rebound();
            item.clear_owner_stats();
        }
        (Status::Thrown | Status::Dropped, HitProc::Hop) => item.common_proc_hop(),
        // `itNBumperThrownProcReflector`.
        (_, HitProc::Reflector) if airborne => {
            if item.vel_air.x * reflector_lr < 0.0 {
                item.vel_air.x = -item.vel_air.x;
            }
            item.clear_owner_stats();
        }
        // `itNBumperAttachedProcHit`: the Y scale is left as it was.
        (Status::Attached, HitProc::Hit) => {
            item.scale.x = 2.0;
            item.scale.z = 2.0;
            item.vars.bumper_hit_anim_length = HIT_ANIM_LENGTH;
            item.palette = 1;
            item.lr = -item.hit_lr;
            item.vel_air.x = item.hit_lr * REBOUND_VEL_X;
            item.multi = HIT_SCALE;
        }
        // `itNBumperAttachedProcReflector`.
        (Status::Attached, HitProc::Reflector) => {
            item.scale.x = 2.0;
            item.scale.z = 2.0;
            item.vars.bumper_hit_anim_length = 3;
            item.palette = 1;
            item.vel_air.x = -reflector_lr * REBOUND_VEL_X;
            item.lr = reflector_lr;
            item.multi = HIT_SCALE;
            item.clear_owner_stats();
        }
        _ => return None,
    }
    Some(true)
}
