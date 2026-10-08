use super::*;
use crate::fighter::{Facing, FighterKind};
use crate::status::{self, AnyStatus, DonkeyStatus, Status, StatusTiming};
use container::{Kind, Status as C};

fn floor() -> MapSurface {
    crate::map::floor_surface((
        0,
        crate::collision::Segment {
            x1: -4000,
            y1: 0,
            x2: 4000,
            y2: 0,
            flags: 0,
        },
    ))
}
fn waiting(pool: &mut ItemPool, kind: Kind) -> u8 {
    let slot = pool
        .spawn_container(kind, Vec3::new(75.0, 225.0, 0.0), Vec3::ZERO)
        .unwrap();
    let item = pool.get_mut(slot).unwrap();
    item.pos.y = -item.coll.bottom + 2.0;
    item.times_landed = 1;
    pool.tick(|| [floor()], None, &[], &mut NoItemAnims);
    assert_eq!(
        pool.get(slot).unwrap().status,
        ItemStatus::Container(C::Wait)
    );
    slot
}
fn pickup(pool: &mut ItemPool, kind: FighterKind) -> Fighter {
    let mut f = Fighter::new(kind, 0, 3);
    f.floor = Some(Standing {
        line: 0,
        normal: Vec2::new(0.0, 1.0),
        flags: 0,
    });
    status::set_wait(&mut f);
    pool.publish(&mut f);
    assert!(crate::item_throw::check_get(&mut f));
    assert_eq!(f.status.status, Status::HeavyGet);
    for _ in 0..100 {
        status::update(&mut f);
        pool.take_requests(&mut f, || [floor()]);
        if f.status.status != Status::HeavyGet {
            break;
        }
    }
    assert!(f.items.held.is_some());
    f
}

#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn every_fighters_heavy_get_script_reaches_its_source_carry_status() {
    for &kind in FighterKind::PLAYABLE {
        let mut pool = ItemPool::default();
        waiting(&mut pool, Kind::Crate);
        let f = pickup(&mut pool, kind);
        let expected = if kind == FighterKind::Donkey {
            AnyStatus::Donkey(DonkeyStatus::ThrowFWait)
        } else {
            Status::LiftWait.into()
        };
        assert_eq!(f.status.status, expected, "{kind:?}");
        assert_eq!(f.items.held.unwrap().weight, ItemWeight::Heavy);
    }
}
#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn lift_turn_flips_after_four_of_eight_source_steps_and_keeps_the_hold() {
    let mut pool = ItemPool::default();
    waiting(&mut pool, Kind::Crate);
    let mut f = pickup(&mut pool, FighterKind::Mario);
    f.stick.x = -40;
    status::update(&mut f);
    assert_eq!(f.status.status, Status::LiftTurn);
    assert_eq!(f.item_throw.lift_turn_tics, 7);
    for _ in 0..2 {
        status::update(&mut f);
    }
    assert_eq!(f.facing, Facing::Right);
    status::update(&mut f);
    assert_eq!(f.facing, Facing::Left);
    for _ in 0..4 {
        status::update(&mut f);
    }
    assert_eq!(f.status.status, Status::LiftWait);
    assert!(f.items.held.is_some());
}
#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn common_and_donkey_heavy_throw_scripts_release_with_the_authored_velocity() {
    for kind in [FighterKind::Mario, FighterKind::Donkey] {
        for smash in [false, true] {
            let mut pool = ItemPool::default();
            let slot = waiting(&mut pool, Kind::Crate);
            let mut f = pickup(&mut pool, kind);
            f.stick.x = 60;
            f.stick.hold_x = if smash { 0 } else { 20 };
            f.input.buttons = ssb_engine::input::N64Buttons(ssb_engine::input::N64Buttons::B);
            assert!(crate::item_throw::check_heavy_throw(&mut f));
            assert_eq!(
                crate::item_throw::common_throw(f.status.status),
                Some(if smash {
                    Status::HeavyThrowF4
                } else {
                    Status::HeavyThrowF
                })
            );
            for _ in 0..100 {
                status::update(&mut f);
                pool.take_requests(&mut f, core::iter::empty);
                if !pool.get(slot).unwrap().is_hold {
                    break;
                }
            }
            let item = pool.get(slot).unwrap();
            assert_eq!(item.status, ItemStatus::Container(C::Thrown));
            let (speed, angle) = if smash {
                (90.0, 20.0_f32)
            } else {
                (70.0, 60.0_f32)
            };
            let (sin, cos) = ssb_engine::math::sin_cos(angle.to_radians());
            assert!(
                (item.vel_air.x - cos * speed).abs() < 0.001,
                "{kind:?}, {smash}"
            );
            assert!((item.vel_air.y - sin * speed).abs() < 0.001);
        }
    }
}
#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn common_carry_drops_on_damage_and_floor_loss_but_donkey_air_throw_keeps_kinetics() {
    let mut pool = ItemPool::default();
    waiting(&mut pool, Kind::Crate);
    let mut f = pickup(&mut pool, FighterKind::Mario);
    crate::item_throw::on_damage(&mut f);
    pool.take_requests(&mut f, core::iter::empty);
    assert!(f.items.held.is_none());
    let mut pool = ItemPool::default();
    waiting(&mut pool, Kind::Crate);
    let mut f = pickup(&mut pool, FighterKind::Mario);
    f.tick_physics_map(&core::iter::empty);
    pool.take_requests(&mut f, core::iter::empty);
    assert!(f.items.held.is_none());
    assert_eq!(f.status.status, Status::Fall);
    let mut pool = ItemPool::default();
    waiting(&mut pool, Kind::Barrel);
    let mut f = pickup(&mut pool, FighterKind::Donkey);
    f.become_airborne();
    status::set_any_status(
        &mut f,
        AnyStatus::Donkey(DonkeyStatus::ThrowFFall),
        0.0,
        StatusTiming::unknown(),
    );
    f.input.buttons = ssb_engine::input::N64Buttons(ssb_engine::input::N64Buttons::A);
    assert!(crate::item_throw::check_heavy_throw(&mut f));
    assert!(!f.is_grounded());
    assert!(crate::grab::on_landing(&mut f, 0.0));
    assert!(f.is_grounded());
    assert_eq!(
        f.status.status,
        AnyStatus::Donkey(DonkeyStatus::HeavyThrowF)
    );
}
#[test]
fn heavy_container_health_thresholds_apply_only_where_a_damage_callback_exists() {
    for (kind, health) in [(Kind::Crate, 15), (Kind::Barrel, 10)] {
        let mut pool = ItemPool::default();
        let slot = waiting(&mut pool, kind);
        pool.get_mut(slot).unwrap().damage_queue = health - 1;
        pool.resolve(&[], &mut NoItemAnims, core::iter::empty);
        assert_eq!(
            pool.get(slot).unwrap().status,
            ItemStatus::Container(C::Wait)
        );
        let item = pool.get_mut(slot).unwrap();
        item.damage_queue = 1;
        item.hitlag_tics = 0;
        pool.resolve(&[], &mut NoItemAnims, core::iter::empty);
        let item = pool.get(slot).unwrap();
        assert_eq!(item.status, ItemStatus::Container(C::Explode));
        assert_eq!((item.attack.damage, item.attack.size), (20, 350.0));
        assert!(item.hidden);
        for _ in 0..7 {
            pool.get_mut(slot).unwrap().hitlag_tics = 0;
            pool.tick(core::iter::empty, None, &[], &mut NoItemAnims);
        }
        assert_eq!(pool.get(slot).unwrap().attack.damage, 10);
        pool.tick(core::iter::empty, None, &[], &mut NoItemAnims);
        assert_eq!(pool.active_count(), 0);
    }
}
#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn barrel_drop_rolls_and_uses_a_stationary_lifetime_without_pickup() {
    let mut pool = ItemPool::default();
    let slot = waiting(&mut pool, Kind::Barrel);
    let mut f = pickup(&mut pool, FighterKind::Mario);
    f.items.request(ItemRequest::Drop {
        vel: Vec3::new(20.0, -10.0, 0.0),
        throw_mul: 1.0,
    });
    pool.take_requests(&mut f, || [floor()]);
    let item = pool.get_mut(slot).unwrap();
    item.pos = Vec3::new(0.0, -item.coll.bottom + 2.0, 0.0);
    item.pos_prev = item.pos;
    pool.tick(|| [floor()], None, &[], &mut NoItemAnims);
    let item = pool.get(slot).unwrap();
    assert_eq!(item.status, ItemStatus::Container(C::Roll));
    assert!(!item.is_allow_pickup);
    assert_eq!(item.lifetime, 360);
    assert_eq!(item.owner, None);
    for _ in 0..10 {
        pool.tick(|| [floor()], None, &[], &mut NoItemAnims);
    }
    assert_eq!(pool.get(slot).unwrap().lifetime, 360);
    pool.get_mut(slot).unwrap().vel_air = Vec3::ZERO;
    for _ in 0..359 {
        pool.tick(|| [floor()], None, &[], &mut NoItemAnims);
    }
    assert_eq!(pool.get(slot).unwrap().lifetime, 1);
    pool.tick(|| [floor()], None, &[], &mut NoItemAnims);
    assert_eq!(pool.active_count(), 0);
}
