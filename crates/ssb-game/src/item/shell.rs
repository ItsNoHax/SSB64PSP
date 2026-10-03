//! The Green Shell (`itgshell.c`) and the Red Shell (`itrshell.c`). A hit
//! or a throw sets a shell sliding along the ground; it takes its attacker
//! as its owner and hurts everyone else. The Green Shell slides straight and
//! stops after 240 frames of sliding. The Red Shell steers towards the
//! nearest fighter, turns at floor edges and walls, and breaks after 24
//! interactions or 480 frames.
//!
//! `damage_all_delay` is unsigned in both, and the source compares it with
//! -1, which never matches: after the delay expires the countdown wraps and
//! runs again, so the delayed callback repeats every 256 frames.

use ssb_engine::math::Vec3;

use super::{
    map, HitProc, Item, ItemAttributes, ItemKind, ItemStatus, ItemType, ItemWeight, OwnerView,
};
use crate::combat::{AttackState, Element, HitStatus};
use crate::ground::BodyColl;
use crate::map::{MASK_CEIL, MASK_LWALL, MASK_RWALL};
use crate::weapon::MapSurface;
use crate::wpeffect::{Emit, WeaponEffect as Fx};

/// `ITKind` order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Kind {
    Green = 17,
    Red = 18,
}

/// `ITGSHELL_*`.
pub const G_LIFETIME: i32 = 240;
pub const G_HEALTH_MAX: i32 = 4;
pub const G_EFFECT_SPAWN_INT: u8 = 8;
pub const G_DAMAGE_ALL_WAIT: u8 = 32;
pub const G_CLAMP_VEL_X: f32 = 90.0;
pub const G_REBOUND_MUL_X: f32 = 0.125;
pub const G_REBOUND_VEL_Y: f32 = 38.0;
pub const G_STOP_VEL_X: f32 = 12.0;
pub const G_DAMAGE_MUL_NORMAL: f32 = 8.0;
pub const G_DAMAGE_MUL_ADD: f32 = 3.0;
pub const G_GRAVITY: f32 = 1.2;
pub const G_TVEL: f32 = 100.0;
pub const G_MAP_REBOUND_COMMON: f32 = 0.2;
pub const G_MAP_REBOUND_GROUND: f32 = 0.5;

/// `ITRSHELL_*`.
pub const R_INTERACT_MAX: u8 = 24;
pub const R_LIFETIME: i32 = 480;
pub const R_HEALTH_MAX: i32 = 4;
pub const R_EFFECT_SPAWN_INT: u8 = 8;
pub const R_DAMAGE_ALL_WAIT: u8 = 16;
pub const R_CLAMP_VEL_X: f32 = 70.0;
pub const R_CLAMP_AIR_X: f32 = 90.0;
pub const R_HIT_INITVEL_X: f32 = 8.0;
pub const R_MUL_VEL_X: f32 = 1.2;
pub const R_STOP_VEL_X: f32 = 8.0;
pub const R_ADD_VEL_X: f32 = 60.0;
pub const R_RECOIL_VEL_X: f32 = -8.0;
pub const R_RECOIL_MUL_X: f32 = 0.7;
pub const R_DAMAGE_MUL_NORMAL: f32 = 10.0;
pub const R_GRAVITY: f32 = 1.2;
pub const R_TVEL: f32 = 100.0;
pub const R_MAP_REBOUND_COMMON: f32 = 0.25;
pub const R_MAP_REBOUND_GROUND: f32 = 0.5;

/// File 251, `llITCommonDataGShellItemAttributes` (0x53C).
pub static GREEN_ATTRIBUTES: ItemAttributes = ItemAttributes {
    is_give_hitlag: true,
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
    kb_scale: 80,
    damage: 18,
    element: Element::Normal,
    kb_weight: 0,
    shield_damage: 0,
    attack_count: 1,
    can_setoff: false,
    priority: 1,
    can_rehit_item: false,
    can_rehit_fighter: false,
    can_hop: true,
    can_reflect: true,
    can_shield: true,
    kb_base: 60,
    ty: ItemType::Throw,
    hitstatus: HitStatus::None,
    vel_scale: 100,
};
/// File 251, `llITCommonDataRShellItemAttributes` (0x584).
pub static RED_ATTRIBUTES: ItemAttributes = ItemAttributes {
    damage: 10,
    kb_weight: 90,
    can_rehit_fighter: true,
    ..GREEN_ATTRIBUTES
};

/// `itGShellStatus` / `itRShellStatus`, with the descriptor's procs as
/// `Init`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Init,
    Wait,
    Fall,
    Hold,
    Thrown,
    Dropped,
    Spin,
    SpinAir,
}

/// `it{G,R}ShellMakeItem`. The maker's `rotate.y` of 90° is lost: the
/// `TraRotRpyR` it adds next resets the root's rotation, and the US build
/// restores only the translation (RE-434). The palette picks the colour.
pub(super) fn make(kind: Kind, pos: Vec3, vel: Vec3) -> Item {
    let attr = match kind {
        Kind::Green => &GREEN_ATTRIBUTES,
        Kind::Red => &RED_ATTRIBUTES,
    };
    let mut item = Item::new(
        ItemKind::Shell(kind),
        attr,
        ItemStatus::Shell(Status::Init),
        AttackState::Off,
        pos,
        vel,
        0,
    );
    item.vars.shell_rotate_y = 0.0;
    item.palette = match kind {
        Kind::Green => 1,
        Kind::Red => 0,
    };
    item.attack.can_rehit_shield = true;
    item.vars.shell_health = 1;
    item.vars.shell_is_damage = false;
    match kind {
        Kind::Green => item.lifetime = G_LIFETIME,
        Kind::Red => {
            item.vars.shell_is_setup = false;
            item.vars.shell_damage_all_delay = u8::MAX;
            item.vars.shell_vel_x = 0.0;
        }
    }
    item
}

fn kind(item: &Item) -> Kind {
    let ItemKind::Shell(k) = item.kind else {
        unreachable!()
    };
    k
}
fn set(item: &mut Item, s: Status) {
    item.set_status(ItemStatus::Shell(s));
}

/// The `damage_all_delay` countdown of `itGShellSpinProcUpdate` and
/// `itRShellFallProcUpdate`; returns whether it expired this frame.
fn damage_all_delay(item: &mut Item) -> bool {
    let expired = item.vars.shell_damage_all_delay == 0;
    if expired {
        item.vars.shell_damage_all_delay = u8::MAX;
    }
    // `!= -1` on a `u8` always holds.
    item.vars.shell_damage_all_delay = item.vars.shell_damage_all_delay.wrapping_sub(1);
    expired
}

/// `it{G,R}ShellCommonClearAnim`: the slide's spin animation stops.
fn clear_anim(item: &mut Item) {
    item.vars.shell_spin_anim = false;
}

/// `itGShellSpinUpdateEffect` / `itRShellSpinUpdateGFX`.
fn dust(item: &mut Item, fx: &mut Emit) {
    if item.vars.shell_dust_int == 0 {
        let mut pos = item.pos;
        pos.y += item.attr.map_coll.bottom;
        fx.push(Fx::DustLight {
            pos,
            lr: item.lr as i8,
        });
        item.vars.shell_dust_int = match kind(item) {
            Kind::Green => G_EFFECT_SPAWN_INT,
            Kind::Red => R_EFFECT_SPAWN_INT,
        };
    }
    item.vars.shell_dust_int = item.vars.shell_dust_int.wrapping_sub(1);
}

fn lr_of(x: f32) -> f32 {
    if x < 0.0 {
        -1.0
    } else {
        1.0
    }
}

/// `itGShellWaitInitVars` / `itRShellCommonSetStatusWaitOrSpin`: a slow
/// shell rests, and a fast one that was hit slides on.
fn wait_or_spin(item: &mut Item) {
    map::set_ground(item);
    let stop = match kind(item) {
        Kind::Green => G_STOP_VEL_X,
        Kind::Red => R_STOP_VEL_X,
    };
    if item.vel_air.x.abs() >= stop && item.vars.shell_is_damage {
        item.attack.state = AttackState::New;
        item.update_attack_positions();
        spin(item);
        return;
    }
    let slow = item.vel_air.x.abs() < stop;
    item.set_ground_allow_pickup();
    if slow {
        item.vars.shell_is_damage = false;
    }
    match kind(item) {
        Kind::Green => item.is_damage_all = true,
        Kind::Red => {
            item.vel_air.x = 0.0;
            item.clear_owner_stats();
        }
    }
    item.damage_coll.hitstatus = HitStatus::Normal;
    item.attack.state = AttackState::Off;
    item.vel_air.x = 0.0;
    clear_anim(item);
    set(item, Status::Wait);
}

/// `it{G,R}ShellFallSetStatus`.
fn fall(item: &mut Item) {
    if kind(item) == Kind::Green {
        item.damage_coll.hitstatus = HitStatus::None;
        item.attack.state = AttackState::Off;
    }
    item.is_allow_pickup = false;
    if kind(item) == Kind::Red {
        item.damage_coll.hitstatus = HitStatus::Normal;
    }
    map::set_air(item);
    set(item, Status::Fall);
}

pub(super) fn hold(item: &mut Item) {
    item.vars.shell_rotate_y = 0.0;
    set(item, Status::Hold);
}

fn release(item: &mut Item, s: Status) {
    item.vars.shell_health = 1;
    item.vars.shell_is_damage = true;
    if kind(item) == Kind::Red {
        item.vars.shell_damage_all_delay = R_DAMAGE_ALL_WAIT;
        item.times_thrown = 0;
        map::set_air(item);
    }
    set(item, s);
}
pub(super) fn thrown(item: &mut Item) {
    release(item, Status::Thrown);
}
pub(super) fn dropped(item: &mut Item) {
    release(item, Status::Dropped);
}

/// `it{G,R}ShellSpinSetStatus`.
fn spin(item: &mut Item) {
    item.is_allow_pickup = false;
    item.pickup_wait = super::PICKUP_WAIT_DEFAULT;
    let clamp = match kind(item) {
        Kind::Green => G_CLAMP_VEL_X,
        Kind::Red => R_CLAMP_VEL_X,
    };
    item.vel_air.x = item.vel_air.x.clamp(-clamp, clamp);
    item.vel_air.y = 0.0;
    item.lr = lr_of(item.vel_air.x);
    match kind(item) {
        Kind::Green => {
            item.vars.shell_dust_int = G_EFFECT_SPAWN_INT;
            item.vars.shell_damage_all_delay = G_DAMAGE_ALL_WAIT;
            item.vars.shell_spin_anim = true;
            item.is_damage_all = false;
            item.refresh_attack_coll();
        }
        Kind::Red => {
            if !item.vars.shell_is_setup {
                item.lifetime = R_LIFETIME;
                item.vars.shell_is_setup = true;
                item.vars.shell_interact = R_INTERACT_MAX;
            }
            item.vars.shell_dust_int = R_EFFECT_SPAWN_INT;
            item.vars.shell_spin_anim = true;
            item.clear_owner_stats();
            map::set_ground(item);
        }
    }
    set(item, Status::Spin);
}

/// `it{G,R}ShellSpinAirSetStatus`.
fn spin_air(item: &mut Item) {
    let clamp = match kind(item) {
        Kind::Green => G_CLAMP_VEL_X,
        Kind::Red => {
            item.is_allow_pickup = false;
            R_CLAMP_AIR_X
        }
    };
    item.vel_air.x = item.vel_air.x.clamp(-clamp, clamp);
    item.lr = lr_of(item.vel_air.x);
    match kind(item) {
        Kind::Green => {
            item.is_damage_all = false;
            item.refresh_attack_coll();
        }
        Kind::Red => {
            item.clear_owner_stats();
            map::set_air(item);
        }
    }
    set(item, Status::SpinAir);
}

/// `itMainCopyDamageStats`, then the slide its hit asks for.
fn launch(item: &mut Item) {
    item.attack.state = AttackState::New;
    item.update_attack_positions();
    item.copy_damage_stats();
    if item.ga != super::Ga::Ground {
        spin_air(item);
    } else {
        spin(item);
    }
}

/// `itRShellSpinUpdateFollowPlayer` towards the nearest fighter in link
/// order (`itRShellSpinSearchFollowPlayer`; a later fighter at the same
/// distance wins).
fn follow(item: &mut Item, owners: &[Option<OwnerView>; 4]) {
    let mut nearest = None;
    let mut nearest_dist = 0.0;
    for (n, view) in owners.iter().flatten().enumerate() {
        let d = view.pos - item.pos;
        let dist = d.x * d.x + d.y * d.y;
        if n == 0 {
            nearest_dist = dist;
        }
        if nearest_dist >= dist {
            nearest_dist = dist;
            nearest = Some(view.pos);
        }
    }
    let Some(target) = nearest else {
        return;
    };
    if item.ga != super::Ga::Ground {
        return;
    }
    let vel_x = lr_of(target.x - item.pos.x) * R_MUL_VEL_X;
    item.vars.shell_vel_x = vel_x;
    item.vel_air.x += vel_x;
    if lr_of(item.vel_air.x) == lr_of(item.vars.shell_vel_x) && item.vel_air.x.abs() > R_CLAMP_VEL_X
    {
        item.vel_air.x = item.lr * R_CLAMP_VEL_X;
    }
    if item.attack.state == AttackState::Off && item.vel_air.x.abs() <= R_HIT_INITVEL_X {
        item.attack.state = AttackState::New;
        item.update_attack_positions();
    }
    item.lr = lr_of(item.vel_air.x);
}

/// `itRShellSpinEdgeInvertVelLR`.
fn invert(item: &mut Item, right: bool) {
    item.vel_air.x = -item.vel_air.x;
    item.vars.shell_vel_x = -item.vars.shell_vel_x;
    item.lr = if right { 1.0 } else { -1.0 };
}

/// `itRShellSpinCheckCollisionEdge`.
fn check_edge<I, F>(item: &mut Item, surfaces: &F)
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
            invert(item, true);
        }
    } else if crate::map::floor_edge(surfaces, line, true)
        .is_some_and(|e| e.x <= item.pos.x + width)
    {
        invert(item, false);
    }
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
    match (kind(item), status) {
        (_, Status::Wait | Status::Hold) => {}
        (
            Kind::Green,
            Status::Init | Status::Fall | Status::Dropped | Status::Thrown | Status::SpinAir,
        ) => {
            item.apply_gravity_clamp_tvel(G_GRAVITY, G_TVEL);
        }
        (Kind::Green, Status::Spin) => {
            dust(item, fx);
            if damage_all_delay(item) {
                item.is_damage_all = true;
            }
            if item.lifetime == 0 {
                return false;
            }
            item.lifetime -= 1;
        }
        (
            Kind::Red,
            Status::Init | Status::Fall | Status::Dropped | Status::Thrown | Status::SpinAir,
        ) => {
            item.apply_gravity_clamp_tvel(R_GRAVITY, R_TVEL);
            if damage_all_delay(item) {
                item.clear_owner_stats();
            }
        }
        (Kind::Red, Status::Spin) => {
            dust(item, fx);
            follow(item, owners);
            check_edge(item, surfaces);
            if item.lifetime == 0 {
                return false;
            }
            item.lifetime -= 1;
        }
    }
    true
}

pub(super) fn has_proc_map(status: Status) -> bool {
    status != Status::Hold
}

pub(super) fn proc_map<I, F>(item: &mut Item, status: Status, surfaces: &F) -> bool
where
    F: Fn() -> I,
    I: IntoIterator<Item = MapSurface>,
{
    let k = kind(item);
    match status {
        Status::Wait => {
            if !map::check_lr_wall_proc_no_floor(item, surfaces) {
                fall(item);
            }
        }
        Status::Init | Status::Fall => {
            let (common, ground) = match k {
                Kind::Green => (G_MAP_REBOUND_COMMON, G_MAP_REBOUND_GROUND),
                Kind::Red => (R_MAP_REBOUND_COMMON, R_MAP_REBOUND_GROUND),
            };
            if item.vars.shell_health == 0 {
                return !map::check_destroy_landing(item, common, surfaces);
            }
            // Both shells drop the random despawn's result.
            let out = map::check_destroy_dropped(item, common, ground, surfaces);
            if out.goto_wait {
                wait_or_spin(item);
            }
        }
        Status::Thrown | Status::Dropped | Status::SpinAir => match k {
            // `itGShellThrownProcMap`.
            Kind::Green => {
                if map::check_landing(item, G_MAP_REBOUND_COMMON, G_MAP_REBOUND_GROUND, surfaces) {
                    wait_or_spin(item);
                }
            }
            // `itRShellThrownProcMap`: the landing's slide starts against
            // the travel, scaled.
            Kind::Red => {
                if map::check_landing(item, 0.25, 0.5, surfaces) {
                    spin(item);
                    item.lr = lr_of(item.vel_air.x);
                    item.vel_air.x = (item.lr * -8.0 + -10.0) * 0.7;
                }
            }
        },
        Status::Spin => match k {
            // `itGShellSpinProcMap`: leaving the floor falls (an oversight
            // the source keeps), and walls and ceilings rebound it.
            Kind::Green => {
                if !map::check_lr_wall_proc_no_floor(item, surfaces) {
                    fall(item);
                }
                if map::check_collide_all_rebound(item, MASK_CEIL | MASK_RWALL | MASK_LWALL, 0.2) {
                    item.set_spin_vel_lr();
                    item.clear_owner_stats();
                }
            }
            Kind::Red => {
                if map::check_lr_wall_proc_no_floor(item, surfaces) {
                    if item.mask_curr & (MASK_RWALL | MASK_LWALL) != 0 {
                        item.vel_air.x = -item.vel_air.x;
                        item.set_spin_vel_lr();
                        item.clear_owner_stats();
                        item.vars.shell_vel_x = -item.vars.shell_vel_x;
                    }
                } else {
                    spin_air(item);
                }
            }
        },
        Status::Hold => {}
    }
    true
}

/// `itGShellCommonProcHit`.
fn green_hit(item: &mut Item) {
    item.damage_coll.hitstatus = HitStatus::Normal;
    item.vars.shell_health = crate::rng::rand_int_range(G_HEALTH_MAX) as u8;
    item.vel_air.y = G_REBOUND_VEL_Y;
    item.vel_air.x = crate::rng::rand_float() * (-item.vel_air.x * G_REBOUND_MUL_X);
    item.clear_owner_stats();
    clear_anim(item);
    fall(item);
}

/// `itRShellCommonProcHit`: returns whether the shell breaks.
fn red_hit(item: &mut Item) -> bool {
    item.vars.shell_interact = item.vars.shell_interact.wrapping_sub(1);
    if item.vars.shell_interact == 0 {
        return true;
    }
    item.damage_coll.hitstatus = HitStatus::Normal;
    item.vars.shell_health = crate::rng::rand_int_range(R_HEALTH_MAX) as u8;
    item.vel_air.x = (-item.vel_air.x + R_RECOIL_VEL_X * item.hit_lr) * R_RECOIL_MUL_X;
    clear_anim(item);
    if item.ga != super::Ga::Ground {
        spin_air(item);
    } else {
        spin(item);
    }
    false
}

/// `itRShellCommonProcReflector`. `reflector_x` is the new owner's
/// position.
fn red_reflector(item: &mut Item, reflector_x: f32) -> bool {
    item.vars.shell_interact = item.vars.shell_interact.wrapping_sub(1);
    if item.vars.shell_interact == 0 {
        return true;
    }
    if item.pos.x < reflector_x {
        item.lr = -1.0;
        if item.vel_air.x >= 0.0 {
            item.vel_air.x = -item.vel_air.x;
            item.vars.shell_vel_x = -item.vars.shell_vel_x;
        }
    } else {
        item.lr = 1.0;
        if item.vel_air.x < 0.0 {
            item.vel_air.x = -item.vel_air.x;
            item.vars.shell_vel_x = -item.vars.shell_vel_x;
        }
    }
    item.vel_air.x += R_ADD_VEL_X * item.lr;
    item.clear_owner_stats();
    false
}

pub(super) fn hit_proc(
    item: &mut Item,
    status: Status,
    proc: HitProc,
    reflector: (f32, f32),
) -> Option<bool> {
    let (reflector_lr, reflector_x) = reflector;
    let thrown = matches!(status, Status::Thrown | Status::Dropped);
    let spinning = matches!(status, Status::Spin | Status::SpinAir);
    match kind(item) {
        Kind::Green => match proc {
            HitProc::Hit if thrown || spinning => green_hit(item),
            HitProc::Shield if spinning => green_hit(item),
            HitProc::Shield | HitProc::SetOff if thrown => item.vel_set_rebound(),
            HitProc::Hop if thrown => item.common_proc_hop(),
            HitProc::Reflector if thrown || spinning => item.common_proc_reflector(reflector_lr),
            // `itGShellCommonProcDamage`.
            HitProc::Damage if thrown || status == Status::Wait => {
                item.vel_air.x = item.damage_queue as f32 * G_DAMAGE_MUL_NORMAL * -item.damage_lr;
                if item.vel_air.x.abs() > G_STOP_VEL_X {
                    item.vars.shell_is_damage = true;
                    item.damage_coll.hitstatus = HitStatus::None;
                    launch(item);
                } else {
                    item.vel_air.x = 0.0;
                    if item.ga != super::Ga::Ground {
                        fall(item);
                    } else {
                        wait_or_spin(item);
                    }
                }
            }
            // `itGShellSpinProcDamage`.
            HitProc::Damage if spinning => {
                item.vel_air.x += item.damage_queue as f32 * G_DAMAGE_MUL_ADD * -item.damage_lr;
                if item.vel_air.x.abs() > G_STOP_VEL_X {
                    launch(item);
                } else {
                    item.vel_air.x = 0.0;
                    if item.ga != super::Ga::Ground {
                        fall(item);
                    } else {
                        wait_or_spin(item);
                    }
                }
            }
            _ => return None,
        },
        Kind::Red => match proc {
            HitProc::Hit if thrown || spinning => {
                if red_hit(item) {
                    return Some(false);
                }
            }
            HitProc::Shield if spinning => {
                if red_hit(item) {
                    return Some(false);
                }
            }
            HitProc::Shield | HitProc::SetOff if thrown => item.vel_set_rebound(),
            HitProc::Hop if thrown => item.common_proc_hop(),
            HitProc::Reflector if thrown || spinning => {
                if red_reflector(item, reflector_x) {
                    return Some(false);
                }
            }
            // `itRShellCommonProcDamage`, which the descriptor and the
            // airborne slide share.
            HitProc::Damage
                if matches!(
                    status,
                    Status::Init
                        | Status::Wait
                        | Status::Thrown
                        | Status::Dropped
                        | Status::SpinAir
                ) =>
            {
                item.vel_air.x = item.damage_queue as f32 * R_DAMAGE_MUL_NORMAL * -item.damage_lr;
                if item.vel_air.x.abs() > R_STOP_VEL_X {
                    item.vars.shell_is_damage = true;
                    launch(item);
                } else {
                    item.vel_air.x = 0.0;
                    item.attack.state = AttackState::Off;
                }
            }
            // `itRShellSpinProcDamage`.
            HitProc::Damage if status == Status::Spin => {
                item.vars.shell_interact = item.vars.shell_interact.wrapping_sub(1);
                if item.vars.shell_interact == 0 {
                    return Some(false);
                }
                item.vel_air.x += item.damage_queue as f32 * 2.0 * -item.damage_lr;
                if item.vel_air.x.abs() > R_STOP_VEL_X {
                    item.attack.state = AttackState::New;
                    item.update_attack_positions();
                    item.copy_damage_stats();
                    spin(item);
                } else {
                    item.attack.state = AttackState::Off;
                }
            }
            _ => return None,
        },
    }
    Some(true)
}
