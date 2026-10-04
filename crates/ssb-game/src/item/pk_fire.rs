//! Ness's PK Fire flame, `itnesspkfire.c` (US). The spark weapon's hit
//! callback makes it 160 units along the spark's travel
//! ([`crate::weapon::WeaponPool::take_item_spawns`]). It cannot be picked
//! up, falls, rests on floors, shrinks with its life and burns anyone its
//! two stacked boxes touch every 16 frames. Its damage collision takes hits:
//! each one costs three times the frame's highest damage in life.

use ssb_engine::math::Vec3;

#[cfg(test)]
mod stat_tests {
    use super::*;
    #[test]
    fn item_status_resets_statistics_except_pk_fire_ground_air_changes() {
        use crate::spgame::{
            bonus::HitAttackId,
            live::{AttackStat, Flags},
        };
        let air = core::iter::empty::<crate::weapon::MapSurface>;
        let identity = AttackStat {
            flags: Flags(HitAttackId::SpecialN as u16),
            count: 77,
        };
        let mut item = make(
            super::super::PKFireSpawn {
                owner_port: 0,
                team: 0,
                pos: Vec3::ZERO,
                weapon_pos: Vec3::ZERO,
                weapon_coll: crate::ground::BodyColl::default(),
                stale: crate::stale::WeaponStale {
                    stat: identity,
                    ..crate::stale::WeaponStale::FRESH
                },
            },
            &air,
        );
        assert_eq!(item.attack.stat, identity);
        proc_update(&mut item, Status::Init);
        assert_eq!(item.attack.stat, identity);
        item.set_status(ItemStatus::PKFire(Status::Fall));
        assert_eq!(item.attack.stat.flags.id(), HitAttackId::Null);
        assert_ne!(item.attack.stat.count, identity.count);
    }
}

use super::{map, HitProc, Item, ItemAttributes, ItemKind, ItemStatus, ItemType, ItemWeight};
use crate::combat::{AttackState, Element, HitStatus};
use crate::ground::BodyColl;
use crate::weapon::MapSurface;

/// `ITPKFIRE_*` (US).
pub const LIFETIME: i32 = 100;
pub const HURT_DAMAGE_MUL: i32 = 3;
pub const GRAVITY: f32 = 0.45;
pub const TVEL: f32 = 55.0;
pub const MAP_REBOUND_COMMON: f32 = 0.2;
pub const MAP_REBOUND_GROUND: f32 = 0.5;

/// `llNessSpecial1PKFireItemAttributes` (offset 0x34 of relocData file 240,
/// US): two boxes of size 200 at y 100 and 350, a 200×400×200 damage box
/// centred at y 200, 3 fire damage at 70° with 10/0/4 knockback, rehit on
/// fighters and items, shieldable, not reflectable, no clank.
pub static ATTRIBUTES: ItemAttributes = ItemAttributes {
    is_give_hitlag: true,
    is_display_colanim: false,
    weight: ItemWeight::Light,
    attack_offsets: [Vec3::new(0.0, 100.0, 0.0), Vec3::new(0.0, 350.0, 0.0)],
    damage_coll_offset: Vec3::new(0.0, 200.0, 0.0),
    damage_coll_size: Vec3::new(200.0, 400.0, 200.0),
    map_coll: BodyColl {
        top: 400.0,
        center: 200.0,
        bottom: 0.0,
        width: 100.0,
    },
    size: 200.0,
    angle: 70,
    kb_scale: 10,
    damage: 3,
    element: Element::Fire,
    kb_weight: 0,
    shield_damage: 0,
    attack_count: 2,
    can_setoff: false,
    priority: 1,
    can_rehit_item: true,
    can_rehit_fighter: true,
    can_hop: false,
    can_reflect: false,
    can_shield: true,
    kb_base: 4,
    ty: ItemType::Fighter,
    hitstatus: HitStatus::Normal,
    vel_scale: 100,
};

/// `ITNessPKFireStatus`, with the descriptor's own procs as `Init`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Init,
    Wait,
    Fall,
}

/// `itNessPKFireMakeItem`.
pub(super) fn make<I, F>(spawn: super::PKFireSpawn, surfaces: &F) -> Item
where
    F: Fn() -> I,
    I: IntoIterator<Item = MapSurface>,
{
    let mut item = Item::new(
        ItemKind::NessPKFire,
        &ATTRIBUTES,
        ItemStatus::PKFire(Status::Init),
        AttackState::New,
        spawn.pos,
        Vec3::ZERO,
        spawn.stale.motion_count,
    );
    // `ITEM_FLAG_COLLPROJECT | ITEM_FLAG_PARENT_WEAPON`.
    map::run_default_collision(&mut item, spawn.weapon_pos, spawn.weapon_coll, surfaces);
    item.update_attack_positions();
    item.owner = Some(spawn.owner_port);
    item.team = spawn.team;
    item.is_allow_pickup = false;
    item.is_hold = false;
    item.player = Some(spawn.owner_port);
    item.attack.can_rehit_shield = true;
    item.attack.stale = spawn.stale.stale;
    item.attack.motion_attack_id = spawn.stale.attack_id;
    item.attack.motion_count = spawn.stale.motion_count;
    item.attack.stat = spawn.stale.stat;
    map::set_air(&mut item);
    item.update_attack_positions();
    item.lifetime = LIFETIME;
    item
}

/// `itNessPKFireCommonUpdateAllCheckDestroy`: the flame's size follows its
/// life, which runs out one tick at a time. Returns whether it is spent.
fn update_all_check_destroy(item: &mut Item) -> bool {
    let half = 0.5_f32;
    let scale = ((item.lifetime as f32 * half) / 100.0) + half;
    item.scale = Vec3::new(scale, scale, scale);
    let attr = item.attr;
    item.attack.offsets[0] = attr.attack_offsets[0] * scale;
    item.attack.offsets[1] = attr.attack_offsets[1] * scale;
    item.attack.size = attr.size * 0.5 * scale;
    item.damage_coll.offset = attr.damage_coll_offset * scale;
    item.damage_coll.size = attr.damage_coll_size * 0.5 * scale;
    item.lifetime -= 1;
    item.lifetime < 0
}

/// `itNessPKFireWaitSetStatus`: preserve the incoming weapon's stat identity.
fn wait_set_status(item: &mut Item) {
    map::set_ground(item);
    item.vel_ground = 0.0;
    item.vel_air.x = 0.0;
    item.vel_air.y = 0.0;
    let stat = item.attack.stat;
    item.set_status(ItemStatus::PKFire(Status::Wait));
    item.attack.stat = stat;
}

/// `itNessPKFireFallSetStatus`.
fn fall_set_status(item: &mut Item) {
    map::set_air(item);
    item.vel_air.x = 0.0;
    item.vel_air.y = 0.0;
    let stat = item.attack.stat;
    item.set_status(ItemStatus::PKFire(Status::Fall));
    item.attack.stat = stat;
}

/// `proc_update`. Returns whether the flame lives on.
pub(super) fn proc_update(item: &mut Item, status: Status) -> bool {
    match status {
        // `itNessPKFireCommonProcUpdate`.
        Status::Init => {
            fall_set_status(item);
            true
        }
        Status::Wait => !update_all_check_destroy(item),
        Status::Fall => {
            if update_all_check_destroy(item) {
                return false;
            }
            item.apply_gravity_clamp_tvel(GRAVITY, TVEL);
            true
        }
    }
}

/// `proc_map`.
pub(super) fn proc_map<I, F>(item: &mut Item, status: Status, surfaces: &F)
where
    F: Fn() -> I,
    I: IntoIterator<Item = MapSurface>,
{
    match status {
        Status::Init => {}
        Status::Wait => {
            if !map::check_lr_wall_proc_no_floor(item, surfaces) {
                fall_set_status(item);
            }
        }
        Status::Fall => {
            if map::check_landing(item, MAP_REBOUND_COMMON, MAP_REBOUND_GROUND, surfaces) {
                wait_set_status(item);
            }
        }
    }
}

/// The flame's only hit callback is `itNessPKFireCommonProcDamage`, in
/// both statuses.
pub(super) fn hit_proc(item: &mut Item, status: Status, proc: HitProc) -> Option<bool> {
    match (status, proc) {
        (Status::Wait | Status::Fall, HitProc::Damage) => {
            if item.lifetime > 0 {
                item.lifetime -= item.damage_highest * HURT_DAMAGE_MUL;
            }
            Some(!update_all_check_destroy(item))
        }
        _ => None,
    }
}
