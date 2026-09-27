use super::*;
use crate::attack;
use crate::item::ItemPool;
use crate::weapon::{PKFire, WeaponPool};
use ssb_engine::input::ControllerState;

fn fighter(kind: FighterKind, port: u8, ground: bool) -> Fighter {
    let mut f = Fighter::new(kind, port, 3);
    if ground {
        status::set_wait(&mut f);
    } else {
        f.physics.jumps_used = 1;
        status::set_fall(&mut f);
    }
    f
}
fn steps(f: &mut Fighter, count: usize) {
    for _ in 0..count {
        status::update(f);
    }
}
fn input(f: &mut Fighter, buttons: u16, x: i8, y: i8) {
    f.set_input(
        ControllerState {
            buttons: N64Buttons(buttons),
            stick_x: x,
            stick_y: y,
            ..Default::default()
        },
        false,
        false,
    );
}
fn request(f: &Fighter, kind: WeaponKind, pos: Vec3) -> WeaponSpawn {
    WeaponSpawn {
        kind,
        owner_port: f.port,
        position: pos,
        facing: f.facing.sign(),
        stale: crate::stale::WeaponStale::of(f),
    }
}
fn close(a: f32, b: f32) {
    assert!((a - b).abs() < 0.001, "{a} != {b}");
}

#[test]
fn specials_route_to_the_source_statuses_and_ordinals() {
    for ground in [true, false] {
        for (y, expected) in [
            (0, if ground { N::SpecialN } else { N::SpecialAirN }),
            (
                80,
                if ground {
                    N::SpecialHiStart
                } else {
                    N::SpecialAirHiStart
                },
            ),
            (
                -80,
                if ground {
                    N::SpecialLwStart
                } else {
                    N::SpecialAirLwStart
                },
            ),
        ] {
            let mut f = fighter(FighterKind::Ness, 0, ground);
            input(&mut f, N64Buttons::B, 0, y);
            assert!(if y == 0 {
                status::check_special_n(&mut f)
            } else if y > 0 {
                status::check_special_hi(&mut f)
            } else {
                status::check_special_lw(&mut f)
            });
            assert_eq!(f.status.status, AnyStatus::Ness(expected));
        }
    }
    assert_eq!(N::SpecialN as u16, 226);
    assert_eq!(N::SpecialAirLwEnd as u16, 244);
    assert_eq!(crate::status::KirbyStatus::CopyNessSpecialN as u16, 254);
}
#[test]
fn pk_fire_accessory_reads_situation_at_frame20_and_never_repeats() {
    let mut f = fighter(FighterKind::Ness, 0, true);
    f.facing = Facing::Left;
    set_special_n(&mut f);
    steps(&mut f, 19);
    assert!(f.weapon_spawn.is_none());
    assert!(on_ground_lost(&mut f));
    assert_eq!(f.status.anim_frame, 19.0);
    steps(&mut f, 1);
    let spawn = f.take_weapon_spawn().unwrap();
    assert_eq!(spawn.kind, WeaponKind::NessPKFire { grounded: false });
    close(spawn.position.x, f.pos.x - 100.0);
    close(spawn.position.y, f.pos.y + 180.0);
    let spark = PKFire::new(spawn, false);
    let (sin, cos) = sin_cos(-38.0 * core::f32::consts::PI / 180.0);
    close(spark.velocity.x, -95.0 * cos);
    close(spark.velocity.y, 95.0 * sin);
    steps(&mut f, 30);
    assert!(f.weapon_spawn.is_none());
}
#[test]
fn copied_pk_fire_uses_kirbys_offsets_slots_and_air_length() {
    let mut f = fighter(FighterKind::Kirby, 0, true);
    f.kirby.copy_id = FighterKind::Ness;
    assert!(crate::kirby_copy::set_special_n(&mut f));
    assert_eq!(f.status.status.anim_slot(), 444);
    steps(&mut f, 19);
    assert!(crate::kirby_copy::on_ground_lost(&mut f));
    assert_eq!(f.status.status.anim_slot(), 445);
    assert_eq!(f.status.timing.anim_length, Some(60.0));
    steps(&mut f, 1);
    let s = f.take_weapon_spawn().unwrap();
    close(s.position.x, f.pos.x + 240.0);
    close(s.position.y, f.pos.y + 190.0);
    assert_eq!(
        f.motion.attack_id,
        crate::stale::MotionAttackId::SpecialNCopyNess
    );
}
#[test]
fn double_jump_keeps_input_velocity_separate_from_animation_drift_and_ends() {
    let mut f = fighter(FighterKind::Ness, 0, false);
    input(&mut f, 0, 40, 0);
    f.physics.is_fastfall = true;
    status::set_jump_aerial(&mut f);
    assert!(!f.physics.is_fastfall);
    let initial = f.ness.jump_velocity_x;
    f.set_root_motion(physics::RootMotion {
        delta: Vec3::new(2.0, 14.0, 5.0),
        ..Default::default()
    });
    assert!(apply_air_physics(&mut f));
    close(f.physics.vel_air.x, f.ness.jump_velocity_x + 5.0);
    close(f.physics.vel_air.y, 14.0);
    close(f.physics.vel_air.z, -2.0);
    assert!(f.ness.jump_velocity_x >= initial);
    f.physics.vel_air.y = -1.0;
    input(&mut f, 0, 0, -80);
    f.tick(core::iter::empty);
    assert!(!f.physics.is_fastfall);
    steps(&mut f, 74);
    assert_eq!(f.status.status, AnyStatus::Common(Status::FallAerial));
}
#[test]
fn jab_flags_chain_to_jab3_and_down_tilt_repeats_at_frame11() {
    let mut f = fighter(FighterKind::Ness, 0, true);
    status::set_attack11(&mut f);
    steps(&mut f, 4);
    input(&mut f, N64Buttons::A, 0, 0);
    steps(&mut f, 1);
    input(&mut f, 0, 0, 0);
    steps(&mut f, 5);
    assert_eq!(f.status.status, AnyStatus::Common(Status::Attack12));
    input(&mut f, N64Buttons::A, 0, 0);
    steps(&mut f, 1);
    input(&mut f, 0, 0, 0);
    steps(&mut f, 7);
    assert_eq!(f.status.status, AnyStatus::Ness(N::Attack13));
    status::set_dtilt(&mut f);
    input(&mut f, N64Buttons::A, 0, -80);
    steps(&mut f, 1);
    input(&mut f, 0, 0, -80);
    steps(&mut f, 9);
    assert_eq!(f.status.anim_frame, 10.0);
    steps(&mut f, 1);
    assert_eq!(f.status.anim_frame, 0.0);
}
#[test]
fn down_air_and_fox_drill_have_signed_downward_angles() {
    // The first live collisions each script makes for its down air.
    fn down_air(kind: FighterKind) -> [crate::combat::AttackColl; 4] {
        let mut f = fighter(kind, 0, false);
        crate::status::set_status(
            &mut f,
            Status::AttackAirLw,
            0.0,
            crate::status::StatusTiming::frames(60.0),
        );
        for _ in 0..60 {
            if f.attack_colls
                .iter()
                .any(|c| c.state != crate::combat::AttackState::Off)
            {
                break;
            }
            crate::status::update(&mut f);
        }
        f.attack_colls
    }
    let ness = down_air(FighterKind::Ness);
    assert_eq!(ness[0].angle, -90);
    let fox = down_air(FighterKind::Fox);
    assert!(fox
        .iter()
        .filter(|c| c.state != crate::combat::AttackState::Off)
        .all(|c| c.angle == -70));
    let hit = attack::resolve_hit(
        &ness[0].hitbox(),
        Vec3::ZERO,
        Vec3::ZERO,
        0,
        1.0,
        true,
        9,
        9,
    );
    assert!(hit.knockback_vel.y < 0.0);
}
#[test]
fn thunder_launch_waits30_hold_frames_then_runs28_with_source_deceleration() {
    let mut f = fighter(FighterKind::Ness, 0, false);
    set_special_hi(&mut f);
    steps(&mut f, 24);
    assert!(thunder_controlling(f.status.status));
    assert_eq!(f.physics.jumps_used, f.attributes.jumps_max);
    f.ness.thunder_position = Some(f.pos + Vec3::new(-100.0, 150.0, 0.0));
    steps(&mut f, 29);
    assert!(thunder_controlling(f.status.status));
    steps(&mut f, 1);
    assert_eq!(f.status.status, AnyStatus::Ness(N::SpecialAirHiJibaku));
    assert!(f.ness.thunder_collide);
    close(f.physics.vel_air.x, 200.0);
    apply_air_physics(&mut f);
    close(f.physics.vel_air.x, 200.0 - 43.0 / 7.0);
    assert!(!crate::combat::is_body_normal(&f));
    steps(&mut f, 10);
    assert!(crate::combat::is_body_normal(&f));
    steps(&mut f, 18);
    assert_eq!(f.status.status, AnyStatus::Ness(N::SpecialAirHiEnd));
    f.physics.vel_air.y = 0.0;
    // `steps` runs updates only; consume the separately clocked physics delay.
    for _ in 0..f.ness.gravity_delay {
        apply_air_physics(&mut f);
    }
    close(f.physics.vel_air.y, 0.0);
    apply_air_physics(&mut f);
    close(f.physics.vel_air.y, -0.5);
    steps(&mut f, 31);
    assert_eq!(f.status.status, AnyStatus::Common(Status::FallSpecial));
    close(f.fall_special.landing_lag, 0.17);
}
#[test]
fn missing_thunder_head_exits_after30_frames_and_a_full_pool_does_not_retry() {
    let mut f = fighter(FighterKind::Ness, 0, false);
    let mut pool = WeaponPool::default();
    for _ in 0..crate::weapon::MAX_WEAPONS {
        assert!(pool.spawn(request(&f, WeaponKind::FoxBlaster, Vec3::ZERO)));
    }
    set_special_hi(&mut f);
    steps(&mut f, 24);
    assert!(!pool.spawn(f.take_weapon_spawn().unwrap()));
    pool.sync_owner(&mut f);
    assert!(f.ness.thunder_destroyed);
    steps(&mut f, 29);
    assert!(thunder_controlling(f.status.status));
    steps(&mut f, 1);
    assert_eq!(f.status.status, AnyStatus::Ness(N::SpecialAirHiEnd));
    assert!(f.weapon_spawn.is_none());
}
#[test]
fn thunder_steers_six_degrees_and_damage_removes_head_and_trails() {
    let mut f = fighter(FighterKind::Ness, 0, false);
    set_special_hi(&mut f);
    steps(&mut f, 24);
    let mut pool = WeaponPool::default();
    assert!(pool.spawn(f.take_weapon_spawn().unwrap()));
    input(&mut f, 0, 80, 0);
    pool.observe_owner(&f);
    pool.tick(core::iter::empty);
    let head = pool.pk_thunders().next().unwrap();
    close(
        head.angle,
        core::f32::consts::FRAC_PI_2 - 6.0 * core::f32::consts::PI / 180.0,
    );
    assert!(head.velocity.x > 0.0);
    for _ in 0..12 {
        pool.tick(core::iter::empty);
    }
    assert_eq!(pool.pk_trails().count(), 4);
    status::set_fall(&mut f);
    pool.observe_owner(&f);
    pool.tick(core::iter::empty);
    assert_eq!(pool.pk_thunders().count(), 0);
    assert_eq!(pool.pk_trails().count(), 0);
}
#[test]
fn thunder_reflection_cleans_trails_and_repeated_reflection_is_safe() {
    let mut f = fighter(FighterKind::Ness, 0, false);
    set_special_hi(&mut f);
    steps(&mut f, 24);
    let mut pool = WeaponPool::default();
    pool.spawn(f.take_weapon_spawn().unwrap());
    pool.observe_owner(&f);
    for _ in 0..12 {
        pool.tick(core::iter::empty);
    }
    assert!(pool.pk_trails().count() > 0);
    for port in [1, 2] {
        let head = pool.pk_thunders().next().unwrap();
        let mut fox = fighter(FighterKind::Fox, port, false);
        fox.pos = head.position - Vec3::new(100.0, 250.0, 0.0);
        status::set_any_status(
            &mut fox,
            AnyStatus::Fox(crate::status::FoxStatus::SpecialAirLwLoop),
            0.0,
            StatusTiming::unknown(),
        );
        pool.apply_hits(&mut fox);
        crate::combat::resolve(&mut fox);
        let h = pool.pk_thunders().next().unwrap();
        assert!(h.reflected);
        assert_eq!(h.owner_port, port);
        assert_eq!(h.lifetime, 160);
        assert_eq!(pool.pk_trails().count(), 0);
        for _ in 0..4 {
            pool.tick(core::iter::empty);
        }
    }
    pool.sync_owner(&mut f);
    assert!(f.ness.thunder_destroyed);
}
#[test]
fn magnet_absorbs_staled_energy_heals_and_retains_timers_through_hit() {
    let mut f = fighter(FighterKind::Ness, 0, false);
    f.damage = 70;
    input(&mut f, N64Buttons::B, 0, -80);
    set_special_lw(&mut f);
    steps(&mut f, 15);
    assert!(absorbing(&f));
    let center = f.joint_world(0, Vec3::new(300.0, 195.0, 0.0));
    let mut owner = fighter(FighterKind::Mario, 1, true);
    status::set_mario_special_n(&mut owner);
    owner.stale.push(
        crate::stale::MotionAttackId::SpecialN,
        owner.motion.count.wrapping_sub(1),
    );
    let spawn = request(&owner, WeaponKind::MarioFireball, center);
    let healed = spawn.stale.damage(7) * 2;
    let mut pool = WeaponPool::default();
    pool.spawn(spawn);
    pool.apply_hits(&mut f);
    crate::combat::resolve(&mut f);
    assert_eq!(f.damage, 70 - healed as u16);
    assert_eq!(pool.active_count(), 0);
    assert_eq!(f.status.status, AnyStatus::Ness(N::SpecialAirLwHit));
    assert_eq!(f.ness.release_lag, 30);
    steps(&mut f, 14);
    assert_eq!(f.status.status, AnyStatus::Ness(N::SpecialAirLwHold));
    assert_eq!(f.ness.release_lag, 30);
    input(&mut f, 0, 0, 0);
    steps(&mut f, 29);
    assert!(absorbing(&f));
    steps(&mut f, 1);
    assert_eq!(f.status.status, AnyStatus::Ness(N::SpecialAirLwEnd));
    steps(&mut f, 11);
    assert_eq!(f.status.status, AnyStatus::Common(Status::Fall));
}

#[test]
fn first_thunder_reflection_consumes_the_head_when_replacement_allocation_fails() {
    let owner = fighter(FighterKind::Ness, 0, false);
    let mut pool = WeaponPool::default();
    assert!(pool.spawn(request(&owner, WeaponKind::NessPKThunder, Vec3::ZERO)));
    for _ in 1..crate::weapon::MAX_WEAPONS {
        assert!(pool.spawn(request(
            &owner,
            WeaponKind::FoxBlaster,
            Vec3::new(10000.0, 0.0, 0.0),
        )));
    }
    let mut fox = fighter(FighterKind::Fox, 1, false);
    fox.pos = Vec3::new(-100.0, -250.0, 0.0);
    status::set_any_status(
        &mut fox,
        AnyStatus::Fox(crate::status::FoxStatus::SpecialAirLwLoop),
        0.0,
        StatusTiming::unknown(),
    );
    pool.apply_hits(&mut fox);
    crate::combat::resolve(&mut fox);
    assert_eq!(pool.pk_thunders().count(), 0);
    assert_eq!(pool.active_count(), crate::weapon::MAX_WEAPONS - 1);
    assert_eq!(fox.damage, 0);
}

#[test]
fn fire_pillar_allocation_and_initial_fall_are_independent_of_a_full_weapon_pool() {
    let owner = fighter(FighterKind::Ness, 0, true);
    let mut pool = WeaponPool::default();
    assert!(pool.spawn(request(
        &owner,
        WeaponKind::NessPKFire { grounded: true },
        Vec3::ZERO,
    )));
    let far = request(&owner, WeaponKind::FoxBlaster, Vec3::new(10000.0, 0.0, 0.0));
    for _ in 1..crate::weapon::MAX_WEAPONS {
        assert!(pool.spawn(far));
    }
    let mut target = fighter(FighterKind::Mario, 1, true);
    pool.apply_hits(&mut target);
    crate::combat::resolve(&mut target);
    let mut items = ItemPool::default();
    items.take_weapon_spawns(&mut pool, core::iter::empty);
    assert_eq!(items.active_count(), 1);
    assert!(pool.spawn(far));
    assert_eq!(pool.active_count(), crate::weapon::MAX_WEAPONS);
    let initial = *items.items().next().unwrap();
    items.tick(core::iter::empty, None);
    let falling = items.items().next().unwrap();
    assert_eq!(falling.pos, initial.pos);
    assert_eq!(falling.vel_air, Vec3::ZERO);
    assert_eq!(falling.lifetime, 100);
    items.tick(core::iter::empty, None);
    close(items.items().next().unwrap().vel_air.y, -0.45);
}
#[test]
fn magnet_delay_and_ground_air_switches_preserve_state() {
    let mut f = fighter(FighterKind::Ness, 0, false);
    f.physics.vel_air = Vec3::new(10.0, 20.0, 0.0);
    set_special_lw(&mut f);
    close(f.physics.vel_air.x, 5.0);
    close(f.physics.vel_air.y, 0.0);
    for _ in 0..4 {
        apply_air_physics(&mut f);
        close(f.physics.vel_air.y, 0.0);
    }
    apply_air_physics(&mut f);
    close(f.physics.vel_air.y, -0.8);
    steps(&mut f, 15);
    assert!(on_landing(&mut f, 0.0, Vec2::new(0.0, 1.0)));
    steps(&mut f, 5);
    let lag = f.ness.release_lag;
    assert!(on_ground_lost(&mut f));
    assert_eq!(f.ness.release_lag, lag);
    assert!(absorbing(&f));
}
#[test]
fn bat_reflects_projectile_only_in_the_flag_window_without_changing_status() {
    let mut f = fighter(FighterKind::Ness, 0, true);
    status::set_status(&mut f, Status::AttackS4, 0.0, StatusTiming::frames(50.0));
    // `ftMotionCommandSetFlag1(1)` at frame 16, `(0)` at 22.
    for _ in 0..15 {
        status::update(&mut f);
        assert!(crate::combat::reflector(&f).is_none());
    }
    status::update(&mut f);
    assert_eq!(f.status.anim_frame, 16.0);
    let owner = fighter(FighterKind::Mario, 1, true);
    let mut pool = WeaponPool::default();
    pool.spawn(request(
        &owner,
        WeaponKind::MarioFireball,
        f.pos + Vec3::new(0.0, 150.0, 0.0),
    ));
    pool.apply_hits(&mut f);
    crate::combat::resolve(&mut f);
    assert_eq!(pool.first_fireball().unwrap().owner_port, 0);
    assert_eq!(f.status.status, AnyStatus::Common(Status::AttackS4));
    assert!(crate::combat::reflector(&f).is_some());
    for _ in 0..6 {
        status::update(&mut f);
    }
    assert!(crate::combat::reflector(&f).is_none());
}
#[test]
fn pk_fire_hit_creates_independent_shrinking_pillar_with16_frame_rehit() {
    let owner = fighter(FighterKind::Ness, 0, true);
    let spawn = request(
        &owner,
        WeaponKind::NessPKFire { grounded: true },
        Vec3::ZERO,
    );
    let mut pool = WeaponPool::default();
    pool.spawn(spawn);
    let mut target = fighter(FighterKind::Mario, 1, true);
    pool.apply_hits(&mut target);
    crate::combat::resolve(&mut target);
    assert_eq!(target.damage, 4);
    assert_eq!(pool.pk_fires().count(), 0);
    let mut items = ItemPool::default();
    items.take_weapon_spawns(&mut pool, core::iter::empty);
    let p = items.items().next().unwrap();
    close((p.pos - spawn.position).length(), 160.0);
    target.pos = p.pos + Vec3::new(0.0, 100.0, 0.0);
    items.search_fighter(&mut target);
    crate::combat::resolve(&mut target);
    assert_eq!(target.damage, 7);
    items.search_fighter(&mut target);
    crate::combat::resolve(&mut target);
    assert_eq!(target.damage, 7);
    for _ in 0..15 {
        items.tick(core::iter::empty, None);
    }
    let p = items.items().next().unwrap();
    assert!(p.scale.x < 1.0);
    target.pos = p.pos + Vec3::new(0.0, 100.0 * p.scale.x, 0.0);
    items.search_fighter(&mut target);
    crate::combat::resolve(&mut target);
    assert_eq!(target.damage, 7);
    items.tick(core::iter::empty, None);
    let p = items.items().next().unwrap();
    target.pos = p.pos + Vec3::new(0.0, 100.0 * p.scale.x, 0.0);
    items.search_fighter(&mut target);
    crate::combat::resolve(&mut target);
    assert_eq!(target.damage, 10);
    for _ in 0..101 {
        items.tick(core::iter::empty, None);
    }
    assert_eq!(items.active_count(), 0);
}
#[test]
fn blast_hit_record_survives_floor_switch_and_invincibility_ends_at10() {
    let mut f = fighter(FighterKind::Ness, 0, false);
    set(&mut f, N::SpecialAirHiJibaku);
    f.ness.blast_frames = 28;
    let mut target = fighter(FighterKind::Mario, 1, true);
    target.pos = f.pos + Vec3::new(0.0, 100.0, 0.0);
    assert!(attack::apply_hit_from(&mut f, &mut target));
    assert_eq!(target.damage, 30);
    on_landing(&mut f, 0.0, Vec2::new(0.0, 1.0));
    assert!(!attack::apply_hit_from(&mut f, &mut target));
    assert_eq!(target.damage, 30);
    assert!(!crate::combat::is_body_normal(&f));
    steps(&mut f, 10);
    assert!(crate::combat::is_body_normal(&f));
}
