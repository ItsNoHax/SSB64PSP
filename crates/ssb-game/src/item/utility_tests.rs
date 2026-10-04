use super::*;
use crate::colanim::ColAnimId;
use crate::fighter::FighterKind;
use crate::status::{self, Status};
use normal::{AppearActor, Appearance, Switches};
use utility::Kind;

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
fn grounded(kind: FighterKind) -> Fighter {
    let mut f = Fighter::new(kind, 0, 3);
    f.floor = Some(Standing {
        line: 0,
        normal: Vec2::new(0.0, 1.0),
        flags: 0,
    });
    status::set_wait(&mut f);
    f
}
/// A consumable made at rest on the floor, as its second landing leaves it.
fn waiting(pool: &mut ItemPool, kind: Kind) -> u8 {
    let slot = pool
        .make_setup_common(
            kind as u8,
            None,
            Vec3::new(75.0, 400.0, 0.0),
            Vec3::ZERO,
            &|| [floor()],
        )
        .unwrap();
    for _ in 0..200 {
        pool.tick(|| [floor()], None, &[], &mut NoItemAnims);
        if pool.get(slot).unwrap().status == ItemStatus::Utility(utility::Status::Wait) {
            return slot;
        }
    }
    panic!("{kind:?} never came to rest");
}

#[test]
fn appear_actor_draws_its_wait_then_drops_a_weighted_kind_on_an_item_point() {
    let mut weights = [0; 20];
    weights[2] = 5;
    weights[4] = 10;
    weights[19] = 7;
    let points = [Vec3::new(-500.0, 800.0, 0.0), Vec3::new(500.0, 900.0, 0.0)];
    let switches = Switches {
        appearance: Appearance::Middle,
        toggles: !(1 << 19),
    };
    let seed = crate::rng::seed();
    let mut actor = AppearActor::new(switches, Some(&weights), points).unwrap();
    assert_ne!(crate::rng::seed(), seed, "the first wait is drawn at once");
    assert!((1200..1260).contains(&actor.spawn_wait));
    assert_eq!(&actor.weights.kinds[..2], &[2, 4]);
    assert_eq!(&actor.weights.blocks[..2], &[0, 5]);
    assert_eq!(actor.weights.sum, 15);
    // The countdown holds the actor; so does the wait.
    let wait = actor.spawn_wait;
    assert_eq!(actor.tick(false, true), None);
    assert_eq!(actor.spawn_wait, wait);
    for _ in 0..wait {
        assert_eq!(actor.tick(true, true), None);
    }
    let spawn = actor.tick(true, true).unwrap();
    assert!([2, 4].contains(&spawn.kind));
    assert!(points.contains(&spawn.pos));
    assert!((1200..1260).contains(&actor.spawn_wait));
    // No free struct: no draws but a new wait.
    actor.spawn_wait = 0;
    assert_eq!(actor.tick(true, false), None);
    assert!(actor.spawn_wait >= 1200);
    for (switches, weights, points) in [
        (
            Switches {
                appearance: Appearance::None,
                ..switches
            },
            Some(&weights),
            &points[..],
        ),
        (
            Switches {
                toggles: 0,
                ..switches
            },
            Some(&weights),
            &points[..],
        ),
        (switches, None, &points[..]),
        (switches, Some(&weights), &[][..]),
        (
            Switches {
                toggles: 1 << 19,
                ..switches
            },
            Some(&[0; 20]),
            &points[..],
        ),
    ] {
        let seed = crate::rng::seed();
        assert!(AppearActor::new(switches, weights, points.iter().copied()).is_none());
        assert_eq!(crate::rng::seed(), seed);
    }
}

#[test]
fn every_common_kind_fits_both_tables() {
    let weights = [1; 20];
    let actor = AppearActor::new(Switches::default(), Some(&weights), [Vec3::ZERO]).unwrap();
    assert_eq!((actor.weights.len, actor.weights.sum), (20, 20));
    let drops = normal::DropWeights::new(Switches::default(), Some(&weights));
    // 16 utilities, then the sentinel and its tenth.
    assert_eq!((drops.len, drops.sum), (17, 17));
}

#[test]
fn tomato_and_heart_are_eaten_at_the_end_of_light_get_and_heal_one_percent_a_frame() {
    for (kind, heal) in [
        (Kind::Tomato, utility::TOMATO_DAMAGE_HEAL),
        (Kind::Heart, utility::HEART_DAMAGE_HEAL),
    ] {
        let mut pool = ItemPool::default();
        let slot = waiting(&mut pool, kind);
        let item = pool.get(slot).unwrap();
        assert!(item.is_allow_pickup);
        assert_eq!(item.pos.y, -item.coll.bottom);
        let mut f = grounded(FighterKind::Mario);
        f.damage = 150;
        pool.publish(&mut f);
        assert!(crate::item_throw::check_get(&mut f));
        assert_eq!(f.status.status, Status::LightGet);
        let mut held = false;
        for _ in 0..100 {
            status::update(&mut f);
            pool.take_requests(&mut f, || [floor()]);
            held |= f.items.held.is_some();
            if f.status.status != Status::LightGet {
                break;
            }
        }
        assert!(held, "{kind:?} is held before it is eaten");
        assert_eq!(f.status.status, Status::Wait);
        assert!(f.items.held.is_none());
        assert!(pool.get(slot).is_none(), "{kind:?} is destroyed");
        assert_eq!(f.damage_heal, heal);
        assert_eq!(f.colanim.id, ColAnimId::FIGHTER_HEAL);
        // The Heart's heal stops at zero percent.
        let frames = heal.min(150) as u16;
        for _ in 1..frames {
            f.tick_timers();
            crate::colanim::run_update_interrupt(&mut f);
        }
        assert_eq!(f.damage, 150 - frames + 1);
        assert_eq!(f.colanim.id, ColAnimId::FIGHTER_HEAL);
        f.tick_timers();
        crate::colanim::run_update_interrupt(&mut f);
        assert_eq!(f.damage, 150 - frames);
        assert_eq!(f.damage_heal, 0);
        assert_ne!(f.colanim.id, ColAnimId::FIGHTER_HEAL);
    }
}

#[test]
fn damage_during_light_get_eats_the_held_tomato() {
    let mut pool = ItemPool::default();
    let slot = waiting(&mut pool, Kind::Tomato);
    let mut f = grounded(FighterKind::Fox);
    pool.publish(&mut f);
    assert!(crate::item_throw::check_get(&mut f));
    for _ in 0..100 {
        status::update(&mut f);
        pool.take_requests(&mut f, || [floor()]);
        if f.items.held.is_some() {
            break;
        }
    }
    assert_eq!(f.status.status, Status::LightGet);
    crate::item_throw::on_damage(&mut f);
    pool.take_requests(&mut f, || [floor()]);
    assert!(pool.get(slot).is_none());
    assert_eq!(f.damage_heal, utility::TOMATO_DAMAGE_HEAL);
}

#[test]
fn star_bounces_towards_the_camera_and_touching_it_makes_a_fighter_invincible() {
    // The camera looks at x = 0.
    let mut pool = ItemPool::default();
    for (x, vx) in [(-300.0, 8.0), (300.0, -8.0)] {
        let slot = pool
            .make_setup_common(6, None, Vec3::new(x, 400.0, 0.0), Vec3::ZERO, &|| [floor()])
            .unwrap();
        let star = pool.get(slot).unwrap();
        assert_eq!(star.vel_air, Vec3::new(vx, 50.0, 0.0));
        assert_eq!(star.attack.state, AttackState::Off);
        assert_eq!(star.attack.interact_mask, INTERACT_FIGHTER);
        assert!(!star.is_allow_pickup);
        pool.destroy(slot);
    }
    let slot = pool
        .make_setup_common(
            6,
            None,
            Vec3::new(0.0, 40.0, 0.0),
            Vec3::ZERO,
            &|| [floor()],
        )
        .unwrap();
    for i in 1..=16 {
        pool.tick(|| [floor()], None, &[], &mut NoItemAnims);
        let star = pool.get(slot).unwrap();
        assert_eq!(star.attack.state != AttackState::Off, i == 16, "tick {i}");
    }
    // It bounces off the floor with a fresh 50 upwards, and never rests.
    let star = pool.get_mut(slot).unwrap();
    star.pos.y = 32.0;
    star.vel_air.y = -10.0;
    pool.tick(|| [floor()], None, &[], &mut NoItemAnims);
    let star = pool.get(slot).unwrap();
    assert_eq!(star.vel_air.y, 50.0);
    assert_eq!(star.status, ItemStatus::Utility(utility::Status::Star));
    let mut f = grounded(FighterKind::Mario);
    let star = pool.get_mut(slot).unwrap();
    f.pos = star.pos;
    star.update_attack_positions();
    let damage = f.damage;
    pool.search_fighter(&mut f);
    assert_eq!(f.damage, damage);
    assert_eq!(f.star_invincible_frames, utility::STAR_INVINCIBLE_TIME);
    assert_eq!(f.colanim.id, ColAnimId::FIGHTER_STAR);
    assert!(!crate::combat::is_body_normal(&f));
    pool.resolve(&[&f], &mut NoItemAnims, || [floor()]);
    assert!(pool.get(slot).is_none(), "the Star is spent");
    for _ in 0..utility::STAR_INVINCIBLE_TIME {
        f.tick_timers();
    }
    crate::colanim::run_update_interrupt(&mut f);
    assert!(crate::combat::is_body_normal(&f));
    assert_ne!(f.colanim.id, ColAnimId::FIGHTER_STAR);
}

#[test]
fn a_crate_drops_one_to_three_utilities_from_the_drop_table() {
    let mut weights = [0; 20];
    weights[4] = 1;
    weights[5] = 1;
    let switches = Switches {
        appearance: Appearance::Middle,
        toggles: (1 << 4) | (1 << 5),
    };
    let mut counts = [0; 4];
    for _ in 0..200 {
        let mut pool = ItemPool {
            normal_switches: switches,
            normal_drops: normal::DropWeights::new(switches, Some(&weights)),
            ..ItemPool::default()
        };
        let slot = pool
            .spawn_container(
                container::Kind::Crate,
                Vec3::new(0.0, 227.0, 0.0),
                Vec3::ZERO,
            )
            .unwrap();
        pool.get_mut(slot).unwrap().times_landed = 1;
        pool.tick(|| [floor()], None, &[], &mut NoItemAnims);
        assert_eq!(
            pool.get(slot).unwrap().status,
            ItemStatus::Container(container::Status::Wait)
        );
        // Break it open.
        pool.get_mut(slot).unwrap().damage_queue = 15;
        pool.resolve(&[], &mut NoItemAnims, || [floor()]);
        let utilities = pool
            .items()
            .filter(|i| matches!(i.kind, ItemKind::Utility(Kind::Tomato | Kind::Heart)))
            .count();
        if pool.get(slot).is_none() {
            counts[utilities] += 1;
            for item in pool.items() {
                // Contents made at priority 0 run their collision callback
                // this pass, but priority 3's animation/movement has passed.
                assert_eq!(item.anim_ticks, 1);
                assert!((item.vel_air.y - 48.0).abs() < 0.3);
                // `itMainSetAppearSpin(item_gobj, FALSE)`.
                let spin = if item.kind == ItemKind::Utility(Kind::Tomato) {
                    core::f32::consts::PI / 18.0
                } else {
                    0.0
                };
                assert_eq!(item.spin_step, spin);
            }
        } else {
            // The explosion sentinel.
            assert_eq!(utilities, 0);
            counts[0] += 1;
        }
    }
    assert!(
        counts[1] > 0 && counts[2] > 0 && counts[3] > 0,
        "{counts:?}"
    );
}

#[test]
fn a_pickable_item_shows_its_arrow_thirty_of_every_forty_five_frames() {
    let mut pool = ItemPool::default();
    let slot = waiting(&mut pool, Kind::Tomato);
    let mut shown = 0;
    for _ in 0..45 {
        pool.tick(|| [floor()], None, &[], &mut NoItemAnims);
        shown += usize::from(pool.get(slot).unwrap().is_arrow_shown());
    }
    assert_eq!(shown, 30);
    // `itStarMakeItem` makes no arrow.
    assert!(!ItemKind::Utility(Kind::Star).has_arrow());
    assert!(ItemKind::Utility(Kind::Heart).has_arrow());
    assert!(!ItemKind::LinkBomb.has_arrow());
}
