//! Link's Bomb, `itlinkbomb.c`. Link's down special pulls it into his hand
//! (`ftLinkSpecialLwMakeBomb`); from there it is thrown or dropped like any
//! light item. Its 300-frame fuse runs in the hand too: the last 96 frames
//! bloat it, and a spent fuse in the hand drops it and explodes it where it
//! is. It explodes when a throw meets a target or a surface fast enough,
//! or when a single frame's hits add up to 7 damage; lighter hits knock it
//! away. The explosion clears its owner, so it can hit Link too.

use ssb_engine::math::Vec3;

use super::{
    map, Effects, HitProc, Item, ItemAttributes, ItemKind, ItemStatus, ItemType, ItemWeight,
    OwnerView,
};
use crate::colanim::ColAnimId;
use crate::combat::{AttackState, Element, HitStatus};
use crate::ground::BodyColl;
use crate::weapon::MapSurface;

/// `ITLINKBOMB_*`.
pub const HEALTH: i32 = 7;
pub const LIFETIME: i32 = 300;
pub const EXPLODE_LIFETIME: u16 = 6;
const SCALE_INDEX_MAX: i32 = 10;
const SCALE_INDEX_REWIND: i32 = SCALE_INDEX_MAX / 2;
const SCALE_INT: i32 = 4;
pub const DAMAGE_RECOIL_VEL_X: f32 = 20.0;
pub const DAMAGE_RECOIL_VEL_Y: f32 = 18.0;
pub const EXPLODE_THRESHOLD_VEL_X: f32 = 36.0;
pub const EXPLODE_THRESHOLD_VEL_Y: f32 = 25.0;
/// Compared against the integer fuse as a float in the source.
pub const BLOAT_BEGIN: i32 = 96;
/// `ITLINKBOMB_BLOAT_COLANIM_LENGTH`.
pub const BLOAT_COLANIM_LENGTH: i32 = 96;
pub const HIT_RECOIL_VEL_X: f32 = 8.0;
pub const HIT_RECOIL_VEL_Y: f32 = 20.0;
pub const GRAVITY: f32 = 1.2;
pub const TVEL: f32 = 100.0;
pub const MAP_REBOUND_COMMON: f32 = 0.4;
pub const MAP_REBOUND_GROUND: f32 = 0.3;
/// `itLinkBombThrownProcMap`'s literal rebounds.
const THROWN_REBOUND_COMMON: f32 = 0.4;
const THROWN_REBOUND_GROUND: f32 = 0.3;
/// `itLinkBombDroppedSetStatus`.
const DROP_UPDATE_WAIT: u16 = 10;

/// `llLinkMainBombItemAttributes` (offset 0x40 of relocData file 225, US):
/// light, a 100-unit damage cube, map box 113 either side of the centre,
/// size 220, 2 damage at 80° with 20/0/60 knockback, a throw item that
/// hops, reflects and shields but never clanks, hit status none until it
/// leaves the hand, 60% throw speed.
pub static ATTRIBUTES: ItemAttributes = ItemAttributes {
    is_give_hitlag: true,
    is_display_colanim: true,
    weight: ItemWeight::Light,
    attack_offsets: [Vec3::ZERO, Vec3::ZERO],
    damage_coll_offset: Vec3::ZERO,
    damage_coll_size: Vec3::new(100.0, 100.0, 100.0),
    map_coll: BodyColl {
        top: 113.0,
        center: 0.0,
        bottom: -113.0,
        width: 113.0,
    },
    size: 220.0,
    angle: 80,
    kb_scale: 20,
    damage: 2,
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
    vel_scale: 60,
};

/// `ITAttackEvent`: from `timer` on, the explosion's angle, damage and
/// radius.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AttackEvent {
    pub timer: u16,
    pub angle: i32,
    pub damage: i32,
    pub size: i32,
}

/// `llLinkMainBombAttackEvents` (offset 0x88 of file 225, US). The fourth
/// entry is never reached: the explosion ends at six frames.
pub const ATTACK_EVENTS: [AttackEvent; 4] = [
    AttackEvent {
        timer: 0,
        angle: 361,
        damage: 5,
        size: 300,
    },
    AttackEvent {
        timer: 2,
        angle: 361,
        damage: 5,
        size: 230,
    },
    AttackEvent {
        timer: 4,
        angle: 361,
        damage: 5,
        size: 150,
    },
    AttackEvent {
        timer: 6,
        angle: 361,
        damage: 5,
        size: 0,
    },
];

/// `llLinkMainBombBloatScales` (offset 0xA8 of file 225): the index runs up
/// to 5 and back down.
pub const BLOAT_SCALES: [f32; 6] = [0.8, 1.0, 1.2, 1.4, 1.6, 1.8];

/// `itLinkBombStatus`, with the descriptor's own procs as `Init`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Init,
    Wait,
    Fall,
    Hold,
    Thrown,
    Dropped,
    Explode,
}

/// `itLinkBombMakeItem` up to `itMainSetFighterHold`, which the pool runs.
pub(super) fn make(pos: Vec3, motion_count: u16) -> Item {
    let mut item = Item::new(
        ItemKind::LinkBomb,
        &ATTRIBUTES,
        ItemStatus::LinkBomb(Status::Init),
        AttackState::Off,
        pos,
        Vec3::ZERO,
        motion_count,
    );
    item.update_attack_positions();
    item.multi = 0;
    item.lifetime = LIFETIME;
    item.vars.bomb_scale_id = 0;
    item.vars.bomb_scale_int = SCALE_INT;
    item.attack.can_rehit_shield = true;
    item.vel_air = Vec3::ZERO;
    item
}

/// `itLinkBombExplodeWaitUpdateScale`: in the hand the bloat scales the
/// model's item root under the attach joint (descriptor 1).
fn update_scale(item: &mut Item) {
    let vars = &mut item.vars;
    if vars.bomb_scale_int == 0 {
        let id = if vars.bomb_scale_id > SCALE_INDEX_REWIND {
            SCALE_INDEX_MAX - vars.bomb_scale_id
        } else {
            vars.bomb_scale_id
        };
        let scale = BLOAT_SCALES[id.clamp(0, 5) as usize];
        item.scale.x = scale;
        item.scale.y = scale;
        vars.bomb_scale_int = SCALE_INT;
        if vars.bomb_scale_id >= SCALE_INDEX_MAX {
            vars.bomb_scale_id = 0;
        } else {
            vars.bomb_scale_id += 1;
        }
    }
    item.vars.bomb_scale_int -= 1;
}

/// The fuse half shared by every live status's update.
fn fuse(item: &mut Item) {
    if item.lifetime == BLOAT_BEGIN {
        item.check_set_colanim(ColAnimId::ITEM_LINK_BOMB_CRITICAL, BLOAT_COLANIM_LENGTH);
        item.vars.bomb_scale_id = 1;
    }
    if item.lifetime < BLOAT_BEGIN {
        update_scale(item);
    }
    item.lifetime -= 1;
}

/// `itLinkBombCommonSetHitStatusNormal` / `...None`.
fn set_hitstatus(item: &mut Item, status: HitStatus) {
    item.damage_coll.hitstatus = status;
}

/// `itLinkBombWaitSetStatus`.
fn wait_set_status(item: &mut Item) {
    item.attack.state = AttackState::Off;
    item.is_allow_pickup = true;
    item.times_landed = 0;
    item.vel_air = Vec3::ZERO;
    map::set_ground(item);
    set_hitstatus(item, HitStatus::Normal);
    item.set_status(ItemStatus::LinkBomb(Status::Wait));
}

/// `itLinkBombFallSetStatus`.
fn fall_set_status(item: &mut Item) {
    item.is_allow_pickup = false;
    map::set_air(item);
    set_hitstatus(item, HitStatus::Normal);
    item.set_status(ItemStatus::LinkBomb(Status::Fall));
}

/// `itLinkBombHoldSetStatus`.
pub(super) fn hold_set_status(item: &mut Item) {
    // itMainSetFighterHold resets the promoted body's descriptor scale.
    item.scale = Vec3::new(1.0, 1.0, 1.0);
    set_hitstatus(item, HitStatus::None);
    item.set_status(ItemStatus::LinkBomb(Status::Hold));
}

/// `itLinkBombThrownSetStatus`.
pub(super) fn thrown_set_status(item: &mut Item) {
    set_hitstatus(item, HitStatus::Normal);
    item.is_damage_all = true;
    item.set_status(ItemStatus::LinkBomb(Status::Thrown));
}

/// `itLinkBombDroppedSetStatus`.
pub(super) fn dropped_set_status(item: &mut Item) {
    set_hitstatus(item, HitStatus::Normal);
    item.vars.bomb_drop_update_wait = DROP_UPDATE_WAIT;
    item.is_damage_all = true;
    item.set_status(ItemStatus::LinkBomb(Status::Dropped));
}

/// `itLinkBombExplodeUpdateAttackEvent`.
fn update_attack_event(item: &mut Item) {
    let ev = ATTACK_EVENTS[usize::from(item.event_id)];
    if item.multi == ev.timer {
        item.attack.angle = ev.angle;
        item.attack.damage = ev.damage;
        item.attack.size = ev.size as f32;
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

/// `itLinkBombExplodeInitVars`: `itLinkBombExplodeMakeEffectGotoSetStatus`
/// and `itLinkBombExplodeSetStatus` with it.
fn explode(item: &mut Item) {
    item.vel_air = Vec3::ZERO;
    item.clear_owner_stats();
    set_hitstatus(item, HitStatus::None);
    item.hidden = true;
    item.refresh_attack_coll();
    item.multi = 0;
    item.event_id = 0;
    item.attack.throw_mul = 1.0;
    update_attack_event(item);
    item.set_status(ItemStatus::LinkBomb(Status::Explode));
}

/// `itLinkBombFallProcUpdate`.
fn fall_update(item: &mut Item) {
    item.apply_gravity_clamp_tvel(GRAVITY, TVEL);
    if item.lifetime == 0 {
        explode(item);
    }
    fuse(item);
}

/// `proc_update`. Returns whether the Bomb lives on.
pub(super) fn proc_update<I, F>(
    item: &mut Item,
    status: Status,
    owners: &[Option<OwnerView>; 4],
    surfaces: &F,
    effects: &mut Effects,
) -> bool
where
    F: Fn() -> I,
    I: IntoIterator<Item = MapSurface>,
{
    match status {
        // `itLinkBombHoldProcUpdate`. The pipe wait that pauses it is not
        // ported.
        Status::Init | Status::Hold => {
            if item.lifetime == 0 {
                // `itMainSetFighterRelease` with the zero hand velocity.
                let owner = item.owner;
                let view = owner.and_then(|p| owners.get(usize::from(p)).copied().flatten());
                if let Some(view) = view {
                    super::ItemPool::set_fighter_release(item, &view, item.vel_air, 1.0, surfaces);
                } else {
                    item.is_hold = false;
                }
                effects.release_owner = owner;
                item.clear_owner_stats();
                explode(item);
            }
            fuse(item);
        }
        // `itLinkBombWaitProcUpdate`.
        Status::Wait => {
            if item.vel_air.x != 0.0 {
                item.vel_air.x += -item.lr;
            }
            if item.vel_air.x.abs() < 1.0 {
                item.vel_air.x = 0.0;
            }
            if item.lifetime == 0 {
                explode(item);
            }
            fuse(item);
        }
        Status::Fall | Status::Thrown => fall_update(item),
        // `itLinkBombDroppedProcUpdate`.
        Status::Dropped => {
            if item.vars.bomb_drop_update_wait != 0 {
                item.vars.bomb_drop_update_wait -= 1;
            } else {
                fall_update(item);
            }
        }
        // `itLinkBombExplodeProcUpdate`.
        Status::Explode => {
            update_attack_event(item);
            item.multi += 1;
            if item.multi == EXPLODE_LIFETIME {
                return false;
            }
        }
    }
    true
}

/// `proc_map`.
pub(super) fn proc_map<I, F>(item: &mut Item, status: Status, surfaces: &F)
where
    F: Fn() -> I,
    I: IntoIterator<Item = MapSurface>,
{
    match status {
        // `itLinkBombWaitProcMap`.
        Status::Wait => {
            if !map::check_lr_wall_proc_no_floor(item, surfaces) {
                fall_set_status(item);
            }
        }
        // `itLinkBombFallProcMap`: the random despawn's result is ignored.
        Status::Fall => {
            let dropped =
                map::check_destroy_dropped(item, MAP_REBOUND_COMMON, MAP_REBOUND_GROUND, surfaces);
            if dropped.goto_wait {
                wait_set_status(item);
            }
        }
        // `itLinkBombThrownProcMap`: the speed before the rebound decides.
        Status::Thrown | Status::Dropped => {
            let vel = item.vel_air;
            if map::check_map_rebound_proc_all(
                item,
                THROWN_REBOUND_COMMON,
                THROWN_REBOUND_GROUND,
                surfaces,
            ) {
                fall_set_status(item);
                if vel.x.abs() > EXPLODE_THRESHOLD_VEL_X || vel.y.abs() > EXPLODE_THRESHOLD_VEL_Y {
                    explode(item);
                }
            }
        }
        Status::Init | Status::Hold | Status::Explode => {}
    }
}

/// `itLinkBombCommonProcDamage`.
fn common_proc_damage(item: &mut Item) {
    if item.damage_queue >= HEALTH {
        explode(item);
    } else {
        item.lr = -item.damage_lr;
        item.vel_air.x = -item.damage_lr * DAMAGE_RECOIL_VEL_X;
        item.vel_air.y = -item.damage_lr * DAMAGE_RECOIL_VEL_Y;
    }
}

/// `itLinkBombThrownProcHit`.
fn thrown_proc_hit(item: &mut Item) {
    if item.vel_air.x.abs() > EXPLODE_THRESHOLD_VEL_X
        || item.vel_air.y.abs() > EXPLODE_THRESHOLD_VEL_Y
    {
        explode(item);
    } else {
        item.lr = -item.hit_lr;
        item.vel_air.x = -item.hit_lr * HIT_RECOIL_VEL_X;
        item.vel_air.y = HIT_RECOIL_VEL_Y;
        fall_set_status(item);
    }
}

/// The status's hit callbacks. Every one returns `FALSE`: the Bomb only
/// ends through its explosion.
pub(super) fn hit_proc(
    item: &mut Item,
    status: Status,
    proc: HitProc,
    reflector_lr: f32,
) -> Option<bool> {
    let dropped_wait = item.vars.bomb_drop_update_wait != 0;
    match (status, proc) {
        (Status::Wait | Status::Fall | Status::Thrown, HitProc::Damage) => common_proc_damage(item),
        (Status::Dropped, HitProc::Damage) => {
            if !dropped_wait {
                common_proc_damage(item);
            }
        }
        (Status::Thrown, HitProc::Hit) => thrown_proc_hit(item),
        (Status::Dropped, HitProc::Hit) => {
            if !dropped_wait {
                thrown_proc_hit(item);
            }
        }
        (Status::Thrown | Status::Dropped, HitProc::Shield) => item.vel_set_rebound(),
        (Status::Thrown | Status::Dropped, HitProc::Hop) => item.common_proc_hop(),
        (Status::Thrown | Status::Dropped, HitProc::Reflector) => {
            item.common_proc_reflector(reflector_lr)
        }
        _ => return None,
    }
    Some(true)
}
