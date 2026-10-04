//! The throwable utilities: Motion-Sensor Bomb, Bob-omb, Bumper, both
//! Shells and the Poké Ball.
use super::*;
use crate::fighter::FighterKind;
use crate::status;

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
fn short_floor() -> MapSurface {
    crate::map::floor_surface((
        0,
        crate::collision::Segment {
            x1: -1000,
            y1: 0,
            x2: 1000,
            y2: 0,
            flags: 0,
        },
    ))
}
fn fighter(kind: FighterKind, port: u8, x: f32) -> Fighter {
    let mut f = Fighter::new(kind, port, 3);
    f.pos = Vec3::new(x, 0.0, 0.0);
    f.floor = Some(Standing {
        line: 0,
        normal: Vec2::new(0.0, 1.0),
        flags: 0,
    });
    status::set_wait(&mut f);
    f
}
fn tick(pool: &mut ItemPool) {
    pool.tick(|| [floor()], None, &[], &mut NoItemAnims);
}
/// A common item made above the floor and left to come to rest.
fn resting(pool: &mut ItemPool, index: u8, x: f32) -> u8 {
    let slot = pool
        .make_setup_common(index, None, Vec3::new(x, 400.0, 0.0), Vec3::ZERO, &|| {
            [floor()]
        })
        .unwrap();
    for _ in 0..300 {
        tick(pool);
        if pool.get(slot).unwrap().is_allow_pickup {
            return slot;
        }
    }
    panic!("item {index} never came to rest");
}
/// `f` picks the item up and throws it with `vel` from 500 units above
/// the floor (a host fighter holds at its own position).
fn throw(pool: &mut ItemPool, f: &mut Fighter, slot: u8, vel: Vec3) {
    let at = f.pos;
    f.pos.y = 500.0;
    f.items.request(ItemRequest::Hold { slot });
    pool.take_requests(f, || [floor()]);
    assert!(pool.get(slot).unwrap().is_hold);
    pool.observe_owner(f);
    f.items.request(ItemRequest::Throw {
        vel,
        throw_mul: 1.0,
        is_smash: false,
    });
    pool.take_requests(f, || [floor()]);
    assert!(!pool.get(slot).unwrap().is_hold);
    f.pos = at;
}
/// Ticks out the hitlag a hit left, up to the tick whose update runs.
fn ride_hitlag(pool: &mut ItemPool, slot: u8) {
    while pool.get(slot).unwrap().hitlag_tics > 1 {
        tick(pool);
    }
}
fn status_of(pool: &ItemPool, slot: u8) -> ItemStatus {
    pool.get(slot).unwrap().status
}

#[test]
fn the_six_kinds_are_made_by_their_item_ids_with_rom_attributes() {
    let mut pool = ItemPool::default();
    for (index, kind, ty) in [
        (14, ItemKind::MSBomb, ItemType::Throw),
        (15, ItemKind::BombHei, ItemType::Throw),
        (16, ItemKind::NBumper, ItemType::Throw),
        (17, ItemKind::Shell(shell::Kind::Green), ItemType::Throw),
        (18, ItemKind::Shell(shell::Kind::Red), ItemType::Throw),
        (19, ItemKind::MBall, ItemType::Throw),
    ] {
        let slot = pool
            .make_setup_common(index, None, Vec3::new(0.0, 300.0, 0.0), Vec3::ZERO, &|| {
                [floor()]
            })
            .unwrap();
        let item = pool.get(slot).unwrap();
        assert_eq!((item.kind, item.ty), (kind, ty));
        assert_eq!(item.damage_coll.hitstatus, HitStatus::None);
        pool.destroy(slot);
    }
    // The Bumper hits fighters only.
    let slot = resting(&mut pool, 16, 0.0);
    assert_eq!(
        pool.get(slot).unwrap().attack.interact_mask,
        INTERACT_FIGHTER
    );
}

#[test]
fn motion_sensor_bomb_sticks_arms_after_100_frames_and_explodes_near_a_fighter() {
    let mut pool = ItemPool::default();
    let slot = resting(&mut pool, 14, 0.0);
    let mut mario = fighter(FighterKind::Mario, 0, 0.0);
    throw(&mut pool, &mut mario, slot, Vec3::new(20.0, 10.0, 0.0));
    assert_eq!(pool.get(slot).unwrap().coll.width, msbomb::COLL_SIZE);
    // A far-away fighter does not set it off.
    mario.pos.x = 3000.0;
    pool.observe_owner(&mario);
    let mut stuck = None;
    for i in 0..200 {
        tick(&mut pool);
        if status_of(&pool, slot) == ItemStatus::MSBomb(msbomb::Status::Attached) {
            stuck = Some(i);
            break;
        }
    }
    assert!(stuck.is_some(), "the thrown bomb sticks to the floor");
    let bomb = *pool.get(slot).unwrap();
    assert_eq!(bomb.vel_air, Vec3::ZERO);
    assert_eq!(bomb.owner, None);
    assert!(bomb.is_damage_all);
    assert_eq!(bomb.damage_coll.hitstatus, HitStatus::Normal);
    assert_eq!(bomb.attach_line, Some(0));
    assert_eq!(bomb.rotate_z, 0.0, "it lies flat on a level floor");
    // Mario walks up during the arming delay: nothing happens.
    mario.pos.x = bomb.pos.x + 100.0;
    pool.observe_owner(&mario);
    for _ in 0..msbomb::DETECT_FIGHTER_DELAY {
        tick(&mut pool);
        assert_eq!(
            status_of(&pool, slot),
            ItemStatus::MSBomb(msbomb::Status::Attached)
        );
    }
    tick(&mut pool);
    let bomb = pool.get(slot).unwrap();
    assert_eq!(bomb.status, ItemStatus::MSBomb(msbomb::Status::Explode));
    assert!(bomb.hidden);
    assert_eq!((bomb.attack.damage, bomb.attack.size), (30, 360.0));
    assert_eq!(bomb.attack.element, Element::Fire);
    for _ in 1..msbomb::EXPLODE_LIFETIME {
        tick(&mut pool);
        assert!(pool.get(slot).is_some());
    }
    tick(&mut pool);
    assert!(pool.get(slot).is_none(), "the explosion lasts 16 frames");
}

#[test]
fn an_attached_motion_sensor_bomb_explodes_on_damage_and_falls_when_its_line_goes() {
    let mut pool = ItemPool::default();
    let slot = resting(&mut pool, 14, 0.0);
    let mut mario = fighter(FighterKind::Mario, 0, 0.0);
    throw(&mut pool, &mut mario, slot, Vec3::new(0.0, -10.0, 0.0));
    for _ in 0..50 {
        tick(&mut pool);
    }
    assert_eq!(
        status_of(&pool, slot),
        ItemStatus::MSBomb(msbomb::Status::Attached)
    );
    // Its floor disappears: it detaches, falls and still watches.
    pool.tick(core::iter::empty, None, &[], &mut NoItemAnims);
    let bomb = pool.get(slot).unwrap();
    assert_eq!(bomb.status, ItemStatus::MSBomb(msbomb::Status::Detached));
    assert_eq!(bomb.attach_line, None);
    pool.tick(core::iter::empty, None, &[], &mut NoItemAnims);
    assert!(pool.get(slot).unwrap().vel_air.y < 0.0);
    // Any damage sets it off.
    pool.get_mut(slot).unwrap().damage_queue = 1;
    pool.resolve(&[], &mut NoItemAnims, || [floor()]);
    assert_eq!(
        status_of(&pool, slot),
        ItemStatus::MSBomb(msbomb::Status::Explode)
    );
}

#[test]
fn bob_omb_walks_towards_the_fighters_after_180_frames_and_explodes_480_plus_90_later() {
    let mut pool = ItemPool::default();
    let slot = resting(&mut pool, 15, 0.0);
    assert_eq!(
        status_of(&pool, slot),
        ItemStatus::BombHei(bombhei::Status::Wait)
    );
    let multi = pool.get(slot).unwrap().multi;
    // Two fighters to the left, one to the right.
    for (port, x) in [(0, -800.0), (1, -600.0), (2, 900.0)] {
        pool.observe_owner(&fighter(FighterKind::Mario, port, x));
    }
    for _ in multi..bombhei::WALK_WAIT {
        tick(&mut pool);
        assert_eq!(
            status_of(&pool, slot),
            ItemStatus::BombHei(bombhei::Status::Wait)
        );
    }
    tick(&mut pool);
    let bob = pool.get(slot).unwrap();
    assert_eq!(bob.status, ItemStatus::BombHei(bombhei::Status::Walk));
    assert_eq!((bob.lr, bob.vel_air.x), (-1.0, -bombhei::WALK_VEL_X));
    assert!(!bob.is_allow_pickup);
    assert!(bob.attack.state != AttackState::Off);
    let start = bob.pos.x;
    tick(&mut pool);
    assert_eq!(pool.get(slot).unwrap().pos.x, start - bombhei::WALK_VEL_X);
    for _ in 1..bombhei::FLASH_WAIT {
        tick(&mut pool);
    }
    let bob = pool.get(slot).unwrap();
    assert_eq!(
        bob.status,
        ItemStatus::BombHei(bombhei::Status::ExplodeWait)
    );
    assert_eq!(bob.vel_air, Vec3::ZERO);
    // `itBombHeiExplodeWaitInitVars`' critical flash runs its first frame
    // in the same process: yellow blending towards dark red over 8.
    assert_eq!(
        bob.colanim.id,
        crate::colanim::ColAnimId::ITEM_BOMB_HEI_CRITICAL
    );
    assert_eq!(bob.colanim.color(), Some([0xF0, 0xE0, 0x00, 0x8C]));
    for _ in 0..bombhei::EXPLODE_WAIT {
        tick(&mut pool);
    }
    let bob = pool.get(slot).unwrap();
    assert_eq!(bob.status, ItemStatus::BombHei(bombhei::Status::Explode));
    // Its 90-frame length runs out with the wait.
    assert_eq!(bob.colanim.id, crate::colanim::ColAnimId::NONE);
    assert_eq!((bob.attack.damage, bob.attack.size), (30, 350.0));
}

#[test]
fn a_walking_bob_omb_turns_at_the_floor_edge() {
    let mut pool = ItemPool::default();
    let slot = pool
        .make_setup_common(15, None, Vec3::new(700.0, 400.0, 0.0), Vec3::ZERO, &|| {
            [short_floor()]
        })
        .unwrap();
    pool.observe_owner(&fighter(FighterKind::Mario, 0, 2000.0));
    let mut turned = false;
    for _ in 0..400 {
        pool.tick(|| [short_floor()], None, &[], &mut NoItemAnims);
        let bob = pool.get(slot).unwrap();
        if bob.status == ItemStatus::BombHei(bombhei::Status::Walk) && bob.lr == -1.0 {
            turned = true;
            assert!(bob.pos.x + bob.attr.map_coll.width >= 1000.0 - bombhei::WALK_VEL_X);
            break;
        }
    }
    assert!(turned, "it turns back before walking off");
}

#[test]
fn a_thrown_bob_omb_explodes_on_the_first_surface() {
    let mut pool = ItemPool::default();
    let slot = resting(&mut pool, 15, 0.0);
    let mut mario = fighter(FighterKind::Mario, 0, 0.0);
    throw(&mut pool, &mut mario, slot, Vec3::new(10.0, 5.0, 0.0));
    let mut statuses = Vec::new();
    for _ in 0..100 {
        tick(&mut pool);
        let Some(bob) = pool.get(slot) else { break };
        if statuses.last() != Some(&bob.status) {
            statuses.push(bob.status);
        }
    }
    assert_eq!(
        statuses,
        [
            ItemStatus::BombHei(bombhei::Status::Thrown),
            ItemStatus::BombHei(bombhei::Status::ExplodeMap),
            ItemStatus::BombHei(bombhei::Status::Explode),
        ]
    );
    assert!(pool.get(slot).is_none());
}

#[test]
fn a_hit_launches_a_green_shell_that_takes_its_attacker_as_owner() {
    let mut pool = ItemPool::default();
    let slot = resting(&mut pool, 17, 0.0);
    let shell = pool.get_mut(slot).unwrap();
    assert_eq!(shell.damage_coll.hitstatus, HitStatus::Normal);
    // A 10-damage hit from the left (`damage_lr` -1) by port 2.
    shell.damage_queue = 10;
    shell.damage_lr = -1.0;
    shell.damage_by = Some(2);
    shell.damage_port = Some(2);
    shell.damage_team = 2;
    pool.resolve(&[], &mut NoItemAnims, || [floor()]);
    let shell = pool.get(slot).unwrap();
    assert_eq!(shell.status, ItemStatus::Shell(shell::Status::Spin));
    // 10 * 8 = 80 to the right, inside the 90 clamp.
    assert_eq!(shell.vel_air.x, 80.0);
    assert_eq!(shell.owner, Some(2));
    assert!(!shell.is_damage_all);
    assert_eq!(shell.damage_coll.hitstatus, HitStatus::None);
    assert!(shell.attack.state != AttackState::Off);
    // It hurts its owner again 32 frames on.
    ride_hitlag(&mut pool, slot);
    for _ in 0..shell::G_DAMAGE_ALL_WAIT {
        tick(&mut pool);
        assert!(!pool.get(slot).unwrap().is_damage_all);
    }
    tick(&mut pool);
    assert!(pool.get(slot).unwrap().is_damage_all);
}

#[test]
fn a_sliding_green_shell_ends_after_its_lifetime() {
    let mut pool = ItemPool::default();
    let slot = resting(&mut pool, 17, -3000.0);
    let shell = pool.get_mut(slot).unwrap();
    shell.damage_queue = 2;
    shell.damage_lr = -1.0;
    pool.resolve(&[], &mut NoItemAnims, || [floor()]);
    assert_eq!(
        status_of(&pool, slot),
        ItemStatus::Shell(shell::Status::Spin)
    );
    assert_eq!(pool.get(slot).unwrap().vel_air.x, 16.0);
    ride_hitlag(&mut pool, slot);
    for _ in 0..=shell::G_LIFETIME {
        tick(&mut pool);
    }
    assert!(pool.get(slot).is_none());
}

#[test]
fn a_thrown_red_shell_steers_towards_the_nearest_fighter() {
    let mut pool = ItemPool::default();
    let slot = resting(&mut pool, 18, 0.0);
    let mut mario = fighter(FighterKind::Mario, 0, 0.0);
    throw(&mut pool, &mut mario, slot, Vec3::new(30.0, 0.0, 0.0));
    // The thrower stays far right; Fox waits at the left.
    mario.pos.x = 3000.0;
    pool.observe_owner(&mario);
    pool.observe_owner(&fighter(FighterKind::Fox, 1, -1500.0));
    let mut spin = false;
    for _ in 0..60 {
        tick(&mut pool);
        if status_of(&pool, slot) == ItemStatus::Shell(shell::Status::Spin) {
            spin = true;
            break;
        }
    }
    assert!(spin, "it slides once it lands");
    let shell = *pool.get(slot).unwrap();
    assert_eq!(shell.lifetime, shell::R_LIFETIME);
    assert_eq!(shell.vars.shell_interact, shell::R_INTERACT_MAX);
    assert_eq!(shell.owner, None);
    let mut vx = shell.vel_air.x;
    for _ in 0..20 {
        tick(&mut pool);
        let shell = pool.get(slot).unwrap();
        if shell.pos.x > -1500.0 {
            assert!(shell.vel_air.x <= vx, "it accelerates leftwards");
        }
        vx = shell.vel_air.x;
        assert!(vx.abs() <= shell::R_CLAMP_VEL_X);
    }
    assert!(vx < 0.0);
}

#[test]
fn a_red_shell_breaks_on_its_24th_interaction() {
    let mut pool = ItemPool::default();
    let slot = resting(&mut pool, 18, 0.0);
    pool.observe_owner(&fighter(FighterKind::Mario, 0, -2000.0));
    let shell = pool.get_mut(slot).unwrap();
    shell.damage_queue = 5;
    shell.damage_lr = -1.0;
    pool.resolve(&[], &mut NoItemAnims, || [floor()]);
    assert_eq!(
        status_of(&pool, slot),
        ItemStatus::Shell(shell::Status::Spin)
    );
    for n in 1..shell::R_INTERACT_MAX {
        let shell = pool.get_mut(slot).unwrap();
        shell.hit_normal_damage = 1;
        shell.hit_lr = 1.0;
        pool.resolve(&[], &mut NoItemAnims, || [floor()]);
        assert_eq!(
            pool.get(slot).unwrap().vars.shell_interact,
            shell::R_INTERACT_MAX - n
        );
    }
    pool.get_mut(slot).unwrap().hit_normal_damage = 1;
    pool.resolve(&[], &mut NoItemAnims, || [floor()]);
    assert!(pool.get(slot).is_none());
}

#[test]
fn a_thrown_bumper_settles_on_the_floor_for_360_frames_then_blinks_out() {
    let mut pool = ItemPool::default();
    let slot = resting(&mut pool, 16, 0.0);
    let mut mario = fighter(FighterKind::Mario, 0, 0.0);
    throw(&mut pool, &mut mario, slot, Vec3::new(0.0, -5.0, 0.0));
    let bumper = pool.get(slot).unwrap();
    assert_eq!(bumper.coll.top, nbumper::COLL_SIZE);
    let mut attached = false;
    for _ in 0..60 {
        tick(&mut pool);
        if status_of(&pool, slot) == ItemStatus::NBumper(nbumper::Status::Attached) {
            attached = true;
            break;
        }
    }
    assert!(attached);
    let bumper = pool.get(slot).unwrap();
    assert_eq!(bumper.lifetime, nbumper::LIFETIME);
    assert!(bumper.attack.state != AttackState::Off);
    assert_eq!(bumper.owner, None);
    // A hit swells it in X and Z and slides it back.
    let bumper = pool.get_mut(slot).unwrap();
    bumper.hit_normal_damage = 1;
    bumper.hit_lr = 1.0;
    pool.resolve(&[], &mut NoItemAnims, || [floor()]);
    let bumper = pool.get(slot).unwrap();
    assert_eq!((bumper.scale.x, bumper.scale.y), (2.0, 1.0));
    assert_eq!(
        (bumper.palette, bumper.vel_air.x, bumper.lr),
        (1, -100.0, -1.0)
    );
    for _ in 0..=nbumper::LIFETIME {
        tick(&mut pool);
    }
    let bumper = pool.get(slot).unwrap();
    assert_eq!(
        bumper.status,
        ItemStatus::NBumper(nbumper::Status::GDisappear)
    );
    assert_eq!(bumper.attack.state, AttackState::Off);
    for _ in 0..nbumper::DESPAWN_TIMER {
        tick(&mut pool);
    }
    tick(&mut pool);
    assert!(pool.get(slot).is_none());
}

#[test]
fn a_bumper_hit_in_the_air_flies_back() {
    let mut pool = ItemPool::default();
    let slot = resting(&mut pool, 16, 0.0);
    let mut mario = fighter(FighterKind::Mario, 0, 0.0);
    throw(&mut pool, &mut mario, slot, Vec3::new(30.0, 30.0, 0.0));
    tick(&mut pool);
    let bumper = pool.get_mut(slot).unwrap();
    bumper.hit_normal_damage = 1;
    bumper.hit_lr = 1.0;
    pool.resolve(&[], &mut NoItemAnims, || [floor()]);
    let bumper = pool.get(slot).unwrap();
    assert_eq!(bumper.status, ItemStatus::NBumper(nbumper::Status::HitAir));
    assert_eq!(bumper.vel_air, Vec3::new(-400.0, 200.0, bumper.vel_air.z));
    assert_eq!(
        bumper.vars.bumper_damage_all_delay,
        nbumper::DAMAGE_ALL_WAIT
    );
}

#[test]
fn a_poke_ball_opens_on_its_second_landing_and_releases_after_30_frames() {
    let mut pool = ItemPool::default();
    let slot = resting(&mut pool, 19, 0.0);
    let mut mario = fighter(FighterKind::Mario, 0, 0.0);
    throw(&mut pool, &mut mario, slot, Vec3::new(10.0, 40.0, 0.0));
    let mut opened = false;
    for _ in 0..200 {
        tick(&mut pool);
        if status_of(&pool, slot) == ItemStatus::MBall(mball::Status::Open) {
            opened = true;
            break;
        }
    }
    assert!(opened);
    let ball = pool.get(slot).unwrap();
    assert_eq!(ball.times_landed, 2);
    assert_eq!(ball.vel_air, Vec3::ZERO);
    assert_eq!(ball.attack.state, AttackState::Off);
    assert!(!ball.attack.can_reflect);
    assert_eq!(pool.monster_data, MonsterData::default());
    for _ in 0..mball::SPAWN_WAIT {
        tick(&mut pool);
        assert!(pool.get(slot).is_some());
    }
    tick(&mut pool);
    assert!(pool.get(slot).is_none());
    let data = pool.monster_data;
    assert!((MBALL_MONSTER_START..=MBALL_COMMON_END).contains(&data.monster_curr));
    assert_eq!((data.monster_prev, data.monsters_num), (u8::MAX, 11));
}

#[test]
fn a_reflected_poke_ball_stays_its_throwers() {
    let mut pool = ItemPool::default();
    let slot = resting(&mut pool, 19, 0.0);
    let mut mario = fighter(FighterKind::Mario, 2, 0.0);
    mario.team = 2;
    throw(&mut pool, &mut mario, slot, Vec3::new(30.0, 10.0, 0.0));
    tick(&mut pool);
    let mut fox = fighter(FighterKind::Fox, 1, 300.0);
    fox.team = 1;
    pool.get_mut(slot).unwrap().reflect_by = Some(1);
    pool.resolve(&[&fox], &mut NoItemAnims, || [floor()]);
    let ball = pool.get(slot).unwrap();
    assert_eq!((ball.owner, ball.team), (Some(2), 2));
    assert!(ball.vars.mball_is_rebound);
    assert_eq!(ball.attack.state, AttackState::Off);
}

#[test]
fn the_released_pokemon_never_repeats_either_of_the_last_two() {
    let mut data = MonsterData::default();
    let mut last = [u8::MAX; 2];
    for n in 0..500 {
        let kind = data.choose(false);
        assert!((MBALL_MONSTER_START..=MBALL_COMMON_END).contains(&kind));
        assert!(!last.contains(&kind), "draw {n}");
        last = [last[1], kind];
    }
    assert_eq!(data.monsters_num, 10);
    // With a newcomer unlocked, Mew's 1-in-151 chance takes a draw first.
    let seed = crate::rng::seed();
    data.clone().choose(false);
    let locked = crate::rng::seed();
    crate::rng::set_seed(seed);
    data.clone().choose(true);
    assert_ne!(crate::rng::seed(), locked);
}
