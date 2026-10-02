//! Utility items whose use needs no fighter status of its own:
//! `ittomato.c`, `itheart.c` (consumed at the end of `LightGet`,
//! `ftCommonLightGetProcDamage`) and `itstar.c` (touched,
//! `ftMainUpdateDamageStatItem`).
use super::{map, HitProc, Item, ItemAttributes, ItemKind, ItemStatus, ItemType, ItemWeight};
use crate::{
    combat::{AttackState, Element, HitStatus},
    ground::BodyColl,
    map::{MASK_CEIL, MASK_FLOOR, MASK_LWALL, MASK_RWALL},
    weapon::MapSurface,
};
use ssb_engine::math::Vec3;

/// `ITKind` order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Kind {
    Tomato = 4,
    Heart = 5,
    Star = 6,
}
impl Kind {
    /// The `ITKind` an item manager index names, if this module ports it.
    pub fn from_index(index: u8) -> Option<Self> {
        match index {
            4 => Some(Self::Tomato),
            5 => Some(Self::Heart),
            6 => Some(Self::Star),
            _ => None,
        }
    }
    /// `ITAttributes::spin_speed`, percent.
    pub fn spin_speed(self) -> f32 {
        match self {
            Self::Tomato => 100.0 * 0.01,
            Self::Heart | Self::Star => 0.0,
        }
    }
}

/// `itTomatoStatus` / `itHeartStatus`; the Star keeps its descriptor's
/// procs for life.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Wait,
    Fall,
    Dropped,
    Star,
}

/// `ITTOMATO_*`, `ITHEART_*`, `ITSTAR_*`.
pub const TOMATO_DAMAGE_HEAL: i32 = 100;
pub const HEART_DAMAGE_HEAL: i32 = 999;
pub const STAR_INVINCIBLE_TIME: u16 = 600;
const STAR_INTERACT_DELAY: u16 = 16;
const STAR_GRAVITY: f32 = 1.2;
const STAR_TVEL: f32 = 100.0;
const STAR_MAP_REBOUND_COMMON: f32 = 1.0;
const STAR_VEL_X: f32 = 8.0;
const STAR_BOUNCE_Y: f32 = 50.0;

/// File 251, `llITCommonDataTomatoItemAttributes` (0xB8).
pub static TOMATO_ATTRIBUTES: ItemAttributes = ItemAttributes {
    is_give_hitlag: true,
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
    angle: 361,
    kb_scale: 100,
    damage: 1,
    element: Element::Normal,
    kb_weight: 0,
    shield_damage: 0,
    attack_count: 1,
    can_setoff: true,
    priority: 1,
    can_rehit_item: true,
    can_rehit_fighter: false,
    can_hop: true,
    can_reflect: true,
    can_shield: true,
    kb_base: 0,
    ty: ItemType::Consume,
    hitstatus: HitStatus::None,
    vel_scale: 100,
};
/// File 251, `llITCommonDataHeartItemAttributes` (0x100).
pub static HEART_ATTRIBUTES: ItemAttributes = ItemAttributes {
    map_coll: BodyColl {
        top: 165.0,
        center: 0.0,
        bottom: -165.0,
        width: 180.0,
    },
    ..TOMATO_ATTRIBUTES
};
/// File 251, `llITCommonDataStarItemAttributes` (0x148).
pub static STAR_ATTRIBUTES: ItemAttributes = ItemAttributes {
    map_coll: BodyColl {
        top: 30.0,
        center: 0.0,
        bottom: -30.0,
        width: 30.0,
    },
    can_setoff: false,
    can_hop: false,
    can_reflect: false,
    can_shield: false,
    ty: ItemType::Touch,
    ..TOMATO_ATTRIBUTES
};

/// `it{Tomato,Heart,Star}MakeItem`. `camera_at_x` is the battle camera's
/// look-at X, which turns the Star towards the middle of the screen.
pub(super) fn make(kind: Kind, pos: Vec3, vel: Vec3, camera_at_x: f32) -> Item {
    let (attr, status, vel) = match kind {
        Kind::Tomato => (&TOMATO_ATTRIBUTES, Status::Fall, vel),
        Kind::Heart => (&HEART_ATTRIBUTES, Status::Fall, vel),
        Kind::Star => (
            &STAR_ATTRIBUTES,
            Status::Star,
            Vec3::new(
                if pos.x < camera_at_x {
                    STAR_VEL_X
                } else {
                    -STAR_VEL_X
                },
                STAR_BOUNCE_Y,
                0.0,
            ),
        ),
    };
    let mut item = Item::new(
        ItemKind::Utility(kind),
        attr,
        ItemStatus::Utility(status),
        AttackState::Off,
        pos,
        vel,
        0,
    );
    if kind == Kind::Star {
        // Star Man can only interact with fighters.
        item.attack.interact_mask = super::INTERACT_FIGHTER;
        item.multi = STAR_INTERACT_DELAY;
    }
    item
}

fn kind(item: &Item) -> Kind {
    let ItemKind::Utility(k) = item.kind else {
        unreachable!()
    };
    k
}
fn set(item: &mut Item, s: Status) {
    item.set_status(ItemStatus::Utility(s));
}

/// `it{Tomato,Heart}WaitSetStatus`: `itMainSetGroundAllowPickup`.
fn wait(item: &mut Item) {
    item.attack.state = AttackState::Off;
    item.vel_air = Vec3::ZERO;
    item.is_allow_pickup = true;
    item.times_landed = 0;
    item.owner = None;
    item.player = None;
    item.team = crate::team::TEAM_DEFAULT;
    item.handicap = crate::stale::HANDICAP_DEFAULT;
    item.attack.throw_mul = 1.0;
    map::set_ground(item);
    set(item, Status::Wait);
}
/// `it{Tomato,Heart}FallSetStatus`.
fn fall(item: &mut Item) {
    item.is_allow_pickup = false;
    map::set_air(item);
    set(item, Status::Fall);
}
/// `it{Tomato,Heart}DroppedSetStatus`: a fighter let go of it before it
/// was eaten (`ftSetupDropItem`).
pub(super) fn dropped(item: &mut Item) {
    set(item, Status::Dropped);
}

pub(super) fn update(item: &mut Item, status: Status) -> bool {
    match status {
        Status::Wait => {}
        Status::Fall | Status::Dropped => {
            match kind(item) {
                Kind::Heart => item.apply_gravity_clamp_tvel(0.25, 30.0),
                _ => item.apply_gravity_clamp_tvel(1.2, 100.0),
            }
            item.rotate_z += item.spin_step;
        }
        Status::Star => {
            item.apply_gravity_clamp_tvel(STAR_GRAVITY, STAR_TVEL);
            item.multi = item.multi.wrapping_sub(1);
            if item.multi == 0 {
                item.refresh_attack_coll();
            }
            item.rotate_z += item.spin_step;
        }
    }
    true
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
        Status::Fall | Status::Dropped => {
            let (common, ground) = match kind(item) {
                Kind::Heart => (0.1, 0.0),
                _ => (0.3, 0.5),
            };
            let out = map::check_destroy_dropped(item, common, ground, surfaces);
            if out.destroy {
                return false;
            }
            if out.goto_wait {
                wait(item);
            }
        }
        Status::Star => {
            let is_collide_floor = map::test_all_collision_flag(item, MASK_FLOOR, surfaces);
            if map::check_collide_all_rebound(
                item,
                MASK_CEIL | MASK_RWALL | MASK_LWALL,
                STAR_MAP_REBOUND_COMMON,
            ) {
                item.set_spin_vel_lr();
            }
            if is_collide_floor {
                item.vel_air.y = STAR_BOUNCE_Y;
            }
        }
    }
    true
}

/// `itStarCommonProcHit`: the Star is spent on the fighter it touched.
pub(super) fn hit_proc(_item: &mut Item, status: Status, proc: HitProc) -> Option<bool> {
    (status == Status::Star && proc == HitProc::Hit).then_some(false)
}
