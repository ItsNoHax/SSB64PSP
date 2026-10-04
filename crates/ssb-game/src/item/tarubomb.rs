//! Race to the Finish's rolling bomb barrel, `ittarubomb.c`.
//! `grBonus3TaruBombProcUpdate` drops one every 180 frames
//! ([`crate::stage::bonus3`]). It falls, bounces once or twice and rolls
//! down the slopes; a wall, any hit or 10% damage smashes it into seven
//! pieces and an explosion. It has no owner, and nobody can pick it up.

use ssb_engine::math::{Vec2, Vec3};

use super::{
    map, normal::CommonItems, HitProc, Item, ItemAttributes, ItemKind, ItemStatus, ItemType,
    ItemWeight,
};
use crate::combat::{AttackState, Element, HitStatus};
use crate::effect::SmashPiece;
use crate::ground::BodyColl;
use crate::map::{MASK_CEIL, MASK_FLOOR, MASK_LWALL, MASK_RWALL};
use crate::weapon::MapSurface;
use crate::wpeffect::{Emit, WeaponEffect as Fx};

/// `ITTARUBOMB_*`.
pub const HEALTH_MAX: i32 = 10;
pub const EXPLODE_LIFETIME: u16 = 6;
pub const EXPLODE_EFFECT_SCALE: f32 = 1.4;
pub const MUL_VEL_X: f32 = 1.4;
pub const ROLL_ROTATE_MUL: f32 = 0.0045;
pub const GRAVITY: f32 = 4.0;
pub const TVEL: f32 = 90.0;
pub const MAP_REBOUND_COMMON: f32 = 0.5;

/// `llGRBonus3MapTaruBombItemAttributes` (offset 0xA8 of relocData file
/// 0x127, US): a heavy item whose model is file 162 at 0x788, a 294 by
/// 316 by 294 damage box, map box 236 up and down and 221 wide (the maker
/// squares it to 221), size 290, 10 damage at angle 70, 20 scale and 90
/// base knockback, can set off, reflectable and shieldable.
pub static ATTRIBUTES: ItemAttributes = ItemAttributes {
    is_give_hitlag: true,
    is_display_colanim: false,
    weight: ItemWeight::Heavy,
    attack_offsets: [Vec3::ZERO; 2],
    damage_coll_offset: Vec3::ZERO,
    damage_coll_size: Vec3::new(294.0, 316.0, 294.0),
    map_coll: BodyColl {
        top: 236.0,
        center: 0.0,
        bottom: -236.0,
        width: 221.0,
    },
    size: 290.0,
    angle: 70,
    kb_scale: 20,
    damage: 10,
    element: Element::Normal,
    kb_weight: 0,
    shield_damage: 0,
    attack_count: 1,
    can_setoff: true,
    priority: 1,
    can_rehit_item: false,
    can_rehit_fighter: false,
    can_hop: false,
    can_reflect: true,
    can_shield: true,
    kb_base: 90,
    ty: ItemType::Fighter,
    hitstatus: HitStatus::Normal,
    vel_scale: 100,
};

/// `llGRBonus3MapTaruBombAttackEvents` (0xF0 of file 0x127): timer,
/// angle, damage, size.
pub const ATTACK_EVENTS: [(u16, i32, i32, f32); 4] = [
    (0, 361, 16, 350.0),
    (2, 361, 11, 250.0),
    (4, 361, 8, 150.0),
    (6, 361, 1, 0.0),
];

/// `itTaruBombStatus`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Fall,
    Explode,
    Roll,
}

/// `itTaruBombMakeItem` (`ITEM_FLAG_PARENT_GROUND`: no map projection).
/// `itTaruBombCommonSetMapCollisionBox` stands the barrel on its side,
/// `rotate.x` 90° on the root, and squares its map box.
pub(super) fn make(pos: Vec3) -> Item {
    let mut item = Item::new(
        ItemKind::TaruBomb,
        &ATTRIBUTES,
        ItemStatus::TaruBomb(Status::Fall),
        AttackState::New,
        pos,
        Vec3::ZERO,
        0,
    );
    item.update_attack_positions();
    item.vars.taru_roll_step = 0.0;
    item.vars.container_root_pitch = core::f32::consts::FRAC_PI_2;
    item.coll.top = item.coll.width;
    item.coll.bottom = -item.coll.width;
    item
}

fn set(item: &mut Item, s: Status) {
    item.set_status(ItemStatus::TaruBomb(s));
}

/// `itMainUpdateAttackEvent` on `llGRBonus3MapTaruBombAttackEvents`.
fn attack_event(item: &mut Item) {
    let (timer, angle, damage, size) = ATTACK_EVENTS[usize::from(item.event_id)];
    if item.multi == timer {
        item.attack.angle = angle;
        item.attack.damage = damage;
        item.attack.size = size;
        item.event_id += 1;
        if item.event_id == 4 {
            item.event_id = 3;
        }
    }
}

/// `itTaruBombExplodeInitVars` and `itTaruBombExplodeSetStatus`.
fn explode_set_status(item: &mut Item) {
    item.multi = 0;
    item.event_id = 0;
    item.attack.can_rehit_item = true;
    item.attack.can_reflect = false;
    item.attack.throw_mul = 1.0;
    item.attack.element = Element::Fire;
    item.attack.can_setoff = false;
    item.damage_coll.hitstatus = HitStatus::None;
    item.refresh_attack_coll();
    attack_event(item);
    set(item, Status::Explode);
}

/// `itTaruBombCommonProcHit`: the pieces, then
/// `itTaruBombExplodeMakeEffectGotoSetStatus`.
fn smash(item: &mut Item, common: &mut dyn CommonItems, fx: &mut Emit) {
    common.smash_container(item.pos, SmashPiece::TaruBomb);
    item.attack.state = AttackState::Off;
    item.vel_air = Vec3::ZERO;
    fx.push(Fx::ScaledExplosion {
        pos: item.pos,
        scale: EXPLODE_EFFECT_SCALE,
    });
    fx.push(Fx::Quake(1));
    // `DObjGetStruct(item_gobj)->flags = DOBJ_FLAG_HIDDEN`.
    item.hidden = true;
    explode_set_status(item);
}

/// `itTaruBombRollSetStatus`.
fn roll(item: &mut Item) {
    item.vel_air.y = 0.0;
    set(item, Status::Roll);
}

pub(super) fn update(item: &mut Item, status: Status) -> bool {
    match status {
        // `itTaruBombFallProcUpdate`.
        Status::Fall => {
            item.apply_gravity_clamp_tvel(GRAVITY, TVEL);
            item.rotate_z += item.vars.taru_roll_step;
        }
        // `itTaruBombExplodeProcUpdate`.
        Status::Explode => {
            item.multi += 1;
            if item.multi == EXPLODE_LIFETIME {
                return false;
            }
            attack_event(item);
        }
        // `itTaruBombRollProcUpdate`: unlike the Barrel's roll, no clock.
        Status::Roll => {
            let n = item.floor.map_or(Vec2::new(0.0, 1.0), |f| f.normal);
            item.vel_air.x +=
                -(ssb_engine::math::atan2(n.y, n.x) - core::f32::consts::FRAC_PI_2) * MUL_VEL_X;
            item.lr = if item.vel_air.x >= 0.0 { 1.0 } else { -1.0 };
            let speed = Vec2::new(item.vel_air.x, item.vel_air.y).length();
            item.vars.taru_roll_step = if item.lr == -1.0 {
                ROLL_ROTATE_MUL
            } else {
                -ROLL_ROTATE_MUL
            } * speed;
            item.rotate_z += item.vars.taru_roll_step;
        }
    }
    true
}

pub(super) fn has_proc_map(status: Status) -> bool {
    status != Status::Explode
}

pub(super) fn proc_map<I, F>(
    item: &mut Item,
    status: Status,
    surfaces: &F,
    common: &mut dyn CommonItems,
    fx: &mut Emit,
) -> bool
where
    F: Fn() -> I,
    I: IntoIterator<Item = MapSurface>,
{
    match status {
        // `itTaruBombFallProcMap` through
        // `itTaruBombFallCheckCollideGround`.
        Status::Fall => {
            let floor = map::test_all_collision_flag(item, MASK_FLOOR, surfaces);
            if map::check_collide_all_rebound(
                item,
                MASK_CEIL | MASK_RWALL | MASK_LWALL,
                MAP_REBOUND_COMMON,
            ) {
                item.set_spin_vel_lr();
            }
            if floor {
                // The source's signed comparison: a landing this fast
                // smashes the barrel and the `TRUE` return then destroys
                // it, explosion and all. Gravity clamps the fall to -90,
                // so only an upward 90 reaches it.
                if item.vel_air.y >= 90.0 {
                    smash(item, common, fx);
                    return false;
                } else if item.vel_air.y < 30.0 {
                    roll(item);
                } else {
                    let n = item.floor.map_or(Vec2::new(0.0, 1.0), |f| f.normal);
                    map::reflect(&mut item.vel_air, n);
                    item.vel_air.y *= 0.2;
                    item.set_spin_vel_lr();
                }
                item.clear_owner_stats();
            }
        }
        // `itTaruBombRollProcMap`: off an edge it falls without changing
        // its kinetics; a wall smashes it.
        Status::Roll => {
            if !map::test_lr_wall_check_floor(item, surfaces) {
                set(item, Status::Fall);
            } else if item.mask_curr & (MASK_LWALL | MASK_RWALL) != 0 {
                smash(item, common, fx);
            }
        }
        Status::Explode => {}
    }
    true
}

/// `itTaruBombCommonProcHit` for hit, shield, set-off and reflection, and
/// `itTaruBombCommonProcDamage`; the explosion has none.
pub(super) fn hit_proc(
    item: &mut Item,
    status: Status,
    proc: HitProc,
    common: &mut dyn CommonItems,
    fx: &mut Emit,
) -> Option<bool> {
    if status == Status::Explode {
        return None;
    }
    match proc {
        HitProc::Hit | HitProc::Shield | HitProc::SetOff | HitProc::Reflector => {
            smash(item, common, fx)
        }
        HitProc::Damage => {
            if item.percent_damage >= HEALTH_MAX {
                smash(item, common, fx);
            }
        }
        HitProc::Hop => return None,
    }
    Some(true)
}
