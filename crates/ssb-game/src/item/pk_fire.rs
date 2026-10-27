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
            &crate::manager_counts::ManagerCounts::new(),
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
        proc_update(&mut item, Status::Init, &mut Emit::default());
        assert_eq!(item.attack.stat, identity);
        item.set_status(ItemStatus::PKFire(Status::Fall));
        assert_eq!(item.attack.stat.flags.id(), HitAttackId::Null);
        assert_ne!(item.attack.stat.count, identity.count);
    }

    /// `itNessPKFireCommonUpdateAllCheckDestroy`: the flame's particle
    /// transform follows the item each frame; at death the item makes the
    /// small dust and ejects its particle generator (RE-382).
    #[test]
    fn pk_fire_flame_particle_follows_and_ejects_at_death() {
        let air = core::iter::empty::<crate::weapon::MapSurface>;
        let mut item = make(
            &crate::manager_counts::ManagerCounts::new(),
            super::super::PKFireSpawn {
                owner_port: 0,
                team: 0,
                pos: Vec3::new(705.4, 1528.0, 0.0),
                weapon_pos: Vec3::ZERO,
                weapon_coll: crate::ground::BodyColl::default(),
                stale: crate::stale::WeaponStale::FRESH,
            },
            &air,
        );
        item.vars.pkfire_xf = Some(3);
        item.lifetime = 1;
        let mut fx = Emit::default();
        assert!(proc_update(&mut item, Status::Wait, &mut fx));
        let pos = item.pos;
        assert_eq!(
            fx.iter().collect::<alloc::vec::Vec<_>>(),
            [WeaponEffect::PKFireFollow {
                xf: 3,
                pos,
                scale: Vec3::splat(0.505)
            }]
        );
        let mut fx = Emit::default();
        assert!(!proc_update(&mut item, Status::Wait, &mut fx));
        assert_eq!(
            fx.iter().collect::<alloc::vec::Vec<_>>(),
            [
                WeaponEffect::PKFireFollow {
                    xf: 3,
                    pos,
                    scale: Vec3::splat(0.5)
                },
                WeaponEffect::DustExpandSmall(pos),
                WeaponEffect::PKFireEject { xf: 3 },
            ]
        );
    }
}

use super::{map, HitProc, Item, ItemAttributes, ItemKind, ItemStatus};
use crate::combat::AttackState;
use crate::weapon::MapSurface;
use crate::wpeffect::{Emit, WeaponEffect};

/// `ITPKFIRE_*` (US).
pub const LIFETIME: i32 = 100;
pub const HURT_DAMAGE_MUL: i32 = 3;
pub const GRAVITY: f32 = 0.45;
pub const TVEL: f32 = 55.0;
pub const MAP_REBOUND_COMMON: f32 = 0.2;
pub const MAP_REBOUND_GROUND: f32 = 0.5;

/// `llNessSpecial1PKFireItemAttributes` (file 240, 0x34).
pub static ATTRIBUTES: ItemAttributes = super::attributes::PK_FIRE;

/// `ITNessPKFireStatus`, with the descriptor's own procs as `Init`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Init,
    Wait,
    Fall,
}

/// `itNessPKFireMakeItem`.
pub(super) fn make<I, F>(
    counts: &crate::manager_counts::ManagerCounts,
    spawn: super::PKFireSpawn,
    surfaces: &F,
) -> Item
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
        counts,
    );
    // `ITEM_FLAG_COLLPROJECT | ITEM_FLAG_PARENT_WEAPON`.
    map::run_default_collision(&mut item, spawn.weapon_pos, spawn.weapon_coll, surfaces);
    item.update_attack_positions();
    item.owner = Some(spawn.owner_port);
    item.team = spawn.team;
    item.is_allow_pickup = false;
    item.is_hold = false;
    item.player = Some(spawn.owner_port);
    // `ip->handicap = wp->handicap` (itnesspkfire.c).
    item.handicap = spawn.stale.handicap;
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
/// life, which runs out one tick at a time, and its particle's transform
/// follows the flame. Returns whether it is spent; a spent flame makes
/// small dust where it stood and ends its particle.
fn update_all_check_destroy(item: &mut Item, fx: &mut Emit) -> bool {
    let half = 0.5_f32;
    let scale = ((item.lifetime as f32 * half) / 100.0) + half;
    item.scale = Vec3::new(scale, scale, scale);
    let attr = item.attr;
    item.attack.offsets[0] = attr.attack_offsets[0] * scale;
    item.attack.offsets[1] = attr.attack_offsets[1] * scale;
    item.attack.size = attr.size * 0.5 * scale;
    item.damage_coll.offset = attr.damage_coll_offset * scale;
    item.damage_coll.size = attr.damage_coll_size * 0.5 * scale;
    if let Some(xf) = item.vars.pkfire_xf {
        fx.push(WeaponEffect::PKFireFollow {
            xf,
            pos: item.pos,
            scale: item.scale,
        });
    }
    item.lifetime -= 1;
    if item.lifetime < 0 {
        fx.push(WeaponEffect::DustExpandSmall(item.pos));
        if let Some(xf) = item.vars.pkfire_xf {
            fx.push(WeaponEffect::PKFireEject { xf });
        }
        return true;
    }
    false
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
pub(super) fn proc_update(item: &mut Item, status: Status, fx: &mut Emit) -> bool {
    match status {
        // `itNessPKFireCommonProcUpdate`.
        Status::Init => {
            fall_set_status(item);
            true
        }
        Status::Wait => !update_all_check_destroy(item, fx),
        Status::Fall => {
            if update_all_check_destroy(item, fx) {
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
pub(super) fn hit_proc(
    item: &mut Item,
    status: Status,
    proc: HitProc,
    fx: &mut Emit,
) -> Option<bool> {
    match (status, proc) {
        (Status::Wait | Status::Fall, HitProc::Damage) => {
            if item.lifetime > 0 {
                item.lifetime -= item.damage_highest * HURT_DAMAGE_MUL;
            }
            Some(!update_all_check_destroy(item, fx))
        }
        _ => None,
    }
}
