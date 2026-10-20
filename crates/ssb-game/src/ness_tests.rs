use super::*;
use crate::attack;
use crate::item::{ItemPool, NoItemAnims};
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
        team: f.team,
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
    // The setter played frame 1 (RE-474).
    steps(&mut f, 18);
    assert!(f.weapon_spawn.is_none());
    assert!(on_ground_lost(&mut f));
    assert_eq!(f.status.anim_frame, 19.0);
    steps(&mut f, 1);
    // `ftNessSpecialNProcAccessory` makes it after the map step.
    crate::item_use::accessory(&mut f);
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
    crate::item_use::accessory(&mut f);
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
    // Expectations below are in unit size.
    f.attributes.size = 1.0;
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
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
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
    // `ftCommonAttackLw3SetStatus` plays frame 1 at once; motion flag 1 at
    // frame 11 restarts the buffered tilt, at its frame 1 again.
    steps(&mut f, 8);
    assert_eq!(f.status.anim_frame, 10.0);
    steps(&mut f, 1);
    assert_eq!(f.status.anim_frame, 1.0);
}
#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
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
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn thunder_launch_waits30_hold_frames_then_runs28_with_source_deceleration() {
    let mut f = fighter(FighterKind::Ness, 0, false);
    set_special_hi(&mut f);
    // The start lasts 23 frames after its setter's play (RE-474).
    steps(&mut f, 23);
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
    pool.tick(core::iter::empty, None);
    let head = pool.pk_thunders().next().unwrap();
    close(
        head.angle,
        core::f32::consts::FRAC_PI_2 - 6.0 * core::f32::consts::PI / 180.0,
    );
    assert!(head.velocity.x > 0.0);
    for _ in 0..12 {
        pool.tick(core::iter::empty, None);
    }
    assert_eq!(pool.pk_trails().count(), 4);
    status::set_fall(&mut f);
    pool.observe_owner(&f);
    pool.tick(core::iter::empty, None);
    assert_eq!(pool.pk_thunders().count(), 0);
    assert_eq!(pool.pk_trails().count(), 0);
}
#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn thunder_reflection_cleans_trails_and_repeated_reflection_is_safe() {
    let mut f = fighter(FighterKind::Ness, 0, false);
    set_special_hi(&mut f);
    steps(&mut f, 24);
    let mut pool = WeaponPool::default();
    pool.spawn(f.take_weapon_spawn().unwrap());
    pool.observe_owner(&f);
    for _ in 0..12 {
        pool.tick(core::iter::empty, None);
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
            pool.tick(core::iter::empty, None);
        }
    }
    pool.sync_owner(&mut f);
    assert!(f.ness.thunder_destroyed);
}
#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn magnet_absorbs_staled_energy_heals_and_retains_timers_through_hit() {
    let mut f = fighter(FighterKind::Ness, 0, false);
    f.damage = 70;
    input(&mut f, N64Buttons::B, 0, -80);
    set_special_lw(&mut f);
    // The start lasts 14 frames after its setter's play (RE-474).
    steps(&mut f, 14);
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
    // The hit's setter played frame 1: flag 1 at 14 is 13 frames on.
    steps(&mut f, 13);
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
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
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
    items.tick(core::iter::empty, None, &[], &mut NoItemAnims);
    let falling = items.items().next().unwrap();
    assert_eq!(falling.pos, initial.pos);
    assert_eq!(falling.vel_air, Vec3::ZERO);
    assert_eq!(falling.lifetime, 100);
    items.tick(core::iter::empty, None, &[], &mut NoItemAnims);
    close(items.items().next().unwrap().vel_air.y, -0.45);
}
#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
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
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
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
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
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
        items.tick(core::iter::empty, None, &[], &mut NoItemAnims);
    }
    let p = items.items().next().unwrap();
    assert!(p.scale.x < 1.0);
    target.pos = p.pos + Vec3::new(0.0, 100.0 * p.scale.x, 0.0);
    items.search_fighter(&mut target);
    crate::combat::resolve(&mut target);
    assert_eq!(target.damage, 7);
    items.tick(core::iter::empty, None, &[], &mut NoItemAnims);
    let p = items.items().next().unwrap();
    target.pos = p.pos + Vec3::new(0.0, 100.0 * p.scale.x, 0.0);
    items.search_fighter(&mut target);
    crate::combat::resolve(&mut target);
    assert_eq!(target.damage, 10);
    for _ in 0..101 {
        items.tick(core::iter::empty, None, &[], &mut NoItemAnims);
    }
    assert_eq!(items.active_count(), 0);
}
#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
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
#[test]
fn magnet_field_starts_on_hold_and_ends_with_the_end_status() {
    let mut f = fighter(FighterKind::Ness, 0, false);
    input(&mut f, N64Buttons::B, 0, -80);
    set_special_lw(&mut f);
    let mut first = None;
    for frame in 1..=40 {
        status::update(&mut f);
        if first.is_none() && absorbing(&f) {
            first = Some(frame);
            // Made on the Hold entry, with one play.
            assert_eq!(magnet_effect_ticks(&f), Some(1));
        }
    }
    let first = first.expect("reaches Hold");
    assert_eq!(magnet_effect_ticks(&f), Some(1 + (40 - first) as u16));
    // A Hit keeps the one field; the return to Hold does not make another.
    proc_absorb(&mut f, 1.0);
    let before = magnet_effect_ticks(&f).unwrap();
    steps(&mut f, 13);
    assert_eq!(f.status.status, AnyStatus::Ness(N::SpecialAirLwHold));
    assert_eq!(magnet_effect_ticks(&f), Some(before + 13));
    input(&mut f, 0, 0, 0);
    while absorbing(&f) {
        status::update(&mut f);
    }
    assert_eq!(f.status.status, AnyStatus::Ness(N::SpecialAirLwEnd));
    assert_eq!(magnet_effect_ticks(&f), None);
}
/// `dEFManagerNessPsychicMagnetEffectDesc`'s process is `gcPlayAnimAll`,
/// which never reads `is_pause_effect`, and `ftnessspeciallw.c` sets no
/// `proc_lagstart`: the field plays through Ness's hitlag.
#[test]
fn magnet_field_keeps_playing_through_hitlag() {
    let surfaces = || core::iter::empty::<crate::weapon::MapSurface>();
    let mut f = fighter(FighterKind::Ness, 0, false);
    input(&mut f, N64Buttons::B, 0, -80);
    set_special_lw(&mut f);
    while !absorbing(&f) {
        status::update(&mut f);
    }
    steps(&mut f, 2);
    assert_eq!(magnet_effect_ticks(&f), Some(3));
    let frame = f.status.anim_frame;
    f.hitlag = 5;
    for t in 1..=4 {
        f.tick_interrupt(&surfaces);
        assert!(f.is_in_hitlag());
        assert_eq!(magnet_effect_ticks(&f), Some(3 + t), "hitlag tick {t}");
    }
    assert_eq!(f.status.anim_frame, frame, "the fighter is held");
    // The last hitlag tick runs the full update: one tick, not two.
    f.tick_interrupt(&surfaces);
    assert!(!f.is_in_hitlag());
    assert_eq!(magnet_effect_ticks(&f), Some(8));
}
/// Every Ness ground/air switch passes `FTSTATUS_PRESERVE_HITSTATUS`
/// (`FTNESS_SPECIAL*_STATUS_FLAGS`): a fighter made invincible keeps it, and
/// the hit-status flash, both ways. A setter (`FTSTATUS_PRESERVE_NONE`)
/// drops it.
#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn ground_air_switches_keep_the_hit_status() {
    use crate::combat::HitStatus;
    for s in [
        N::SpecialN,
        N::SpecialHiStart,
        N::SpecialHiHold,
        N::SpecialHiEnd,
        N::SpecialHiJibaku,
        N::SpecialLwStart,
        N::SpecialLwHold,
        N::SpecialLwHit,
        N::SpecialLwEnd,
    ] {
        let mut f = fighter(FighterKind::Ness, 0, true);
        set(&mut f, s);
        crate::hurtbox::set_hit_status_all(&mut f, HitStatus::Invincible);
        let colanim = f.colanim.id;
        assert!(on_ground_lost(&mut f), "{s:?}");
        assert_eq!(f.hitstatus, HitStatus::Invincible, "{s:?} to the air");
        assert_eq!(f.colanim.id, colanim, "{s:?}");
        // A level blast lands rather than bouncing.
        f.physics.vel_air = Vec3::new(50.0, 0.0, 0.0);
        assert!(on_landing(&mut f, 0.0, Vec2::new(0.0, 1.0)));
        assert_eq!(f.status.status, AnyStatus::Ness(s), "{s:?}");
        assert_eq!(f.hitstatus, HitStatus::Invincible, "{s:?} to the ground");
    }
    let mut f = fighter(FighterKind::Ness, 0, true);
    crate::hurtbox::set_hit_status_all(&mut f, HitStatus::Invincible);
    set_special_n(&mut f);
    assert_eq!(f.hitstatus, HitStatus::Normal);
}
/// The blast's switches pass `FTSTATUS_PRESERVE_HIT`
/// (`FTNESS_SPECIALHI_STATUS_FLAGS`): the switch's script replay skips the
/// attack events, so a dropped hitbox would stay gone for the slide.
#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn a_landing_blast_keeps_its_hitbox() {
    let mut f = fighter(FighterKind::Ness, 0, false);
    set(&mut f, N::SpecialAirHiJibaku);
    f.ness.blast_frames = 28;
    f.physics.vel_air = Vec3::new(50.0, 0.0, 0.0);
    assert!(on_landing(&mut f, 0.0, Vec2::new(0.0, 1.0)));
    assert_eq!(f.status.status, AnyStatus::Ness(N::SpecialHiJibaku));
    let mut target = fighter(FighterKind::Mario, 1, true);
    target.pos = f.pos + Vec3::new(0.0, 100.0, 0.0);
    assert!(attack::apply_hit_from(&mut f, &mut target));
    assert_eq!(target.damage, 30);
}
/// `ftNessSpecialHiHoldInitStatusVars` makes the PK Thunder wave
/// (`efManagerNessPKThunderWaveMakeEffect`) on the Hold entry, with one
/// play, and sets `is_effect_attach`; the Hold ground/air switches keep
/// it (`FTNESS_SPECIALHIHOLD_STATUS_FLAGS`); the Jibaku stops it.
#[test]
fn pk_wave_starts_on_hold_and_ends_with_the_blast() {
    let mut f = fighter(FighterKind::Ness, 0, false);
    set_special_hi(&mut f);
    assert_eq!(pk_wave_effect_ticks(&f), None);
    steps(&mut f, 22);
    assert!(!thunder_controlling(f.status.status));
    assert_eq!(pk_wave_effect_ticks(&f), None);
    steps(&mut f, 1);
    assert_eq!(f.status.status, AnyStatus::Ness(N::SpecialAirHiHold));
    assert_eq!(pk_wave_effect_ticks(&f), Some(1));
    assert!(f.is_effect_attach);
    steps(&mut f, 4);
    assert_eq!(pk_wave_effect_ticks(&f), Some(5));
    // Landing switches to the grounded Hold: the wave survives.
    assert!(on_landing(&mut f, 0.0, Vec2::new(0.0, 1.0)));
    assert_eq!(f.status.status, AnyStatus::Ness(N::SpecialHiHold));
    assert!(f.is_effect_attach);
    steps(&mut f, 1);
    assert_eq!(pk_wave_effect_ticks(&f), Some(6));
    assert!(on_ground_lost(&mut f));
    assert_eq!(f.status.status, AnyStatus::Ness(N::SpecialAirHiHold));
    steps(&mut f, 1);
    assert_eq!(pk_wave_effect_ticks(&f), Some(7));
    // The blast's setter (`FTSTATUS_PRESERVE_NONE`) stops it.
    f.ness.thunder_position = Some(f.pos + Vec3::new(-100.0, 150.0, 0.0));
    while thunder_controlling(f.status.status) {
        status::update(&mut f);
    }
    assert_eq!(f.status.status, AnyStatus::Ness(N::SpecialAirHiJibaku));
    assert_eq!(pk_wave_effect_ticks(&f), None);
    assert!(!f.is_effect_attach);
}

/// The wave's process is bare `gcPlayAnimAll`: it plays through Ness's
/// hitlag.
#[test]
fn pk_wave_keeps_playing_through_hitlag() {
    let surfaces = || core::iter::empty::<crate::weapon::MapSurface>();
    let mut f = fighter(FighterKind::Ness, 0, false);
    set_special_hi(&mut f);
    while !thunder_controlling(f.status.status) {
        status::update(&mut f);
    }
    steps(&mut f, 2);
    assert_eq!(pk_wave_effect_ticks(&f), Some(3));
    let frame = f.status.anim_frame;
    f.hitlag = 5;
    for t in 1..=4 {
        f.tick_interrupt(&surfaces);
        assert!(f.is_in_hitlag());
        assert_eq!(pk_wave_effect_ticks(&f), Some(3 + t), "hitlag tick {t}");
    }
    assert_eq!(f.status.anim_frame, frame, "the fighter is held");
    f.tick_interrupt(&surfaces);
    assert!(!f.is_in_hitlag());
    assert_eq!(pk_wave_effect_ticks(&f), Some(8));
}

/// One match frame for the PK Thunder trail effect: link 0 and the effect
/// link's `func_run`s, the weapons' main processes and their effects, then
/// the effect processes (priority 3, after the weapon link).
fn pk_frame(pool: &mut WeaponPool, rt: &mut crate::effect::EffectRuntime<'_>) {
    rt.run();
    pool.tick(core::iter::empty, None);
    pool.flush_effects(rt);
    rt.effects.process(rt.particles, rt.banks);
}

fn pk_trail_effects(e: &crate::effect::Effects) -> std::vec::Vec<crate::effect::Display> {
    use crate::effect::DisplayKind as K;
    e.displays()
        .filter(|d| matches!(d.kind, K::NessPKThunderTrail | K::NessPKReflectTrail))
        .copied()
        .collect()
}

/// `efManagerNessPKThunderTrailMakeEffect`: trail 3 makes the effect at
/// its lifetime 158, and the maker runs its process: the owner's ring
/// sample ten frames back, two behind trail 3's own (`- 8`), at z 0, turned
/// by the step into it less 90 degrees. Each later frame's process (after
/// the weapons) follows the ring; the head's end in the weapon pass
/// (`wpNessPKThunderHeadProcUpdate`'s status test) sets `Destroy` before
/// the effect's process, which ejects it unseen that frame.
#[test]
fn pk_trail_effect_follows_the_ring_ten_back_and_ends_with_the_head() {
    let bank = crate::particle::tests::TestBank::new(&[]);
    let mut particles = std::boxed::Box::new(crate::particle::Particles::new());
    let mut effects = crate::effect::Effects::new(0);
    let mut rt = crate::effect::EffectRuntime {
        particles: &mut particles,
        effects: &mut effects,
        banks: &bank,
    };
    let mut f = fighter(FighterKind::Ness, 0, false);
    set_special_hi(&mut f);
    steps(&mut f, 24);
    let mut pool = WeaponPool::default();
    assert!(pool.spawn(f.take_weapon_spawn().unwrap()));
    // Steer, so that the ring's steps turn.
    input(&mut f, 0, 80, 0);
    pool.observe_owner(&f);
    let mut trail3 = std::vec::Vec::new();
    let mut made = None;
    for frame in 1..=20 {
        pk_frame(&mut pool, &mut rt);
        if let Some(t) = pool.pk_trails().find(|t| t.id == 3) {
            trail3.push((frame, t.position, t.rotation()));
        }
        let fx = pk_trail_effects(rt.effects);
        if made.is_none() && !fx.is_empty() {
            made = Some(frame);
            // Made by trail 3 on its third update (lifetime 158), the
            // first on its own make's frame.
            assert_eq!(trail3.len(), 3, "trail 3 frames before the make");
        }
        if let Some(m) = made {
            assert_eq!(fx.len(), 1, "frame {frame}");
            let d = fx[0];
            assert_eq!(d.kind, crate::effect::DisplayKind::NessPKThunderTrail);
            assert_eq!(d.translate.z, 0.0);
            // Two frames behind trail 3's sample.
            let back = trail3.iter().find(|&&(t, _, _)| t + 2 == frame);
            if let Some(&(_, pos, rot)) = back {
                close(d.translate.x, pos.x);
                close(d.translate.y, pos.y);
                close(d.rotate.z, rot);
            } else {
                assert!(frame < m + 2 || trail3.is_empty());
            }
        }
    }
    assert_eq!(made, Some(11), "trail 3 makes the effect");
    // The head ends in the weapon pass: the effect goes the same frame.
    status::set_fall(&mut f);
    pool.observe_owner(&f);
    pk_frame(&mut pool, &mut rt);
    assert_eq!(pool.pk_thunders().count(), 0);
    assert!(pk_trail_effects(rt.effects).is_empty());
}

/// A head ended after the effect processes (a hit, priority 0) leaves the
/// trail effect `Destroy`ed but drawn that frame; the next frame's process
/// ejects it. A head the weapon pass still saw keeps it.
#[test]
fn pk_trail_effect_outlives_a_late_head_end_by_one_frame() {
    let bank = crate::particle::tests::TestBank::new(&[]);
    let mut particles = std::boxed::Box::new(crate::particle::Particles::new());
    let mut effects = crate::effect::Effects::new(0);
    let mut rt = crate::effect::EffectRuntime {
        particles: &mut particles,
        effects: &mut effects,
        banks: &bank,
    };
    use crate::effect::HitEffectSink;
    rt.run();
    assert!(rt.effects.pk_trail(7, false, Vec3::new(1.0, 2.0, 3.0), 0.5));
    // Not started on its make's frame.
    rt.process();
    assert_eq!(pk_trail_effects(rt.effects).len(), 1);
    // A live head writes the pose before the process.
    rt.run();
    rt.effects
        .pk_trail_pose(7, Vec3::new(4.0, 5.0, 9.0), Some(0.25));
    rt.process();
    let d = pk_trail_effects(rt.effects)[0];
    assert_eq!(
        (d.translate.x, d.translate.y, d.translate.z),
        (4.0, 5.0, 0.0)
    );
    assert_eq!(d.rotate.z, 0.25);
    // Another head's pose is not this one's.
    rt.run();
    rt.effects.pk_trail_pose(8, Vec3::ZERO, None);
    rt.process();
    assert!(pk_trail_effects(rt.effects).is_empty());
}

/// `wpNessPKThunderHeadProcReflector` (priority 0, after the effect
/// processes) destroys the trails: the trail effect is drawn that frame and
/// ejected by the next frame's process. The reflected head's trail makes
/// `efManagerNessPKReflectTrailMakeEffect` (link 18) at its lifetime 158,
/// turned by the head's own angle less 90 degrees.
#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn reflected_thunder_swaps_the_trail_effect_for_the_reflect_trail() {
    use crate::effect::DisplayKind as K;
    let bank = crate::particle::tests::TestBank::new(&[]);
    let mut particles = std::boxed::Box::new(crate::particle::Particles::new());
    let mut effects = crate::effect::Effects::new(0);
    let mut rt = crate::effect::EffectRuntime {
        particles: &mut particles,
        effects: &mut effects,
        banks: &bank,
    };
    let mut f = fighter(FighterKind::Ness, 0, false);
    set_special_hi(&mut f);
    steps(&mut f, 24);
    let mut pool = WeaponPool::default();
    pool.spawn(f.take_weapon_spawn().unwrap());
    pool.observe_owner(&f);
    for _ in 0..16 {
        pk_frame(&mut pool, &mut rt);
    }
    let fx = pk_trail_effects(rt.effects);
    assert_eq!(fx.len(), 1);
    assert_eq!(fx[0].kind, K::NessPKThunderTrail);
    let head = pool.pk_thunders().next().unwrap();
    let mut fox = fighter(FighterKind::Fox, 1, false);
    fox.pos = head.position - Vec3::new(100.0, 250.0, 0.0);
    status::set_any_status(
        &mut fox,
        AnyStatus::Fox(crate::status::FoxStatus::SpecialAirLwLoop),
        0.0,
        StatusTiming::unknown(),
    );
    pool.apply_hits(&mut fox);
    crate::combat::resolve(&mut fox);
    pool.flush_effects(&mut rt);
    assert!(pool.pk_thunders().next().unwrap().reflected);
    // Drawn on the reflection's frame, gone by the next process.
    assert_eq!(pk_trail_effects(rt.effects).len(), 1);
    pk_frame(&mut pool, &mut rt);
    assert!(pk_trail_effects(rt.effects).is_empty());
    let mut made = None;
    for frame in 2..=24 {
        pk_frame(&mut pool, &mut rt);
        let fx = pk_trail_effects(rt.effects);
        if let Some(d) = fx.first() {
            assert_eq!(fx.len(), 1);
            assert_eq!(d.kind, K::NessPKReflectTrail);
            assert_eq!(d.kind.dl_link(), 18);
            let h = pool.pk_thunders().next().unwrap();
            close(d.rotate.z, h.rotate_z() - core::f32::consts::FRAC_PI_2);
            made.get_or_insert(frame);
        }
    }
    assert!(made.is_some(), "the reflect trail makes its effect");
}

/// `wpDisplayPKThunderProcDisplay` (`wp/wpdisplay.c`): each trail draws
/// under `dWPDisplayPKThunder{Prim,Env}Colors[trail_id]`, trail 0 the
/// bluest, trail 3 the palest.
#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn pk_trails_take_prim_and_env_by_trail_id() {
    use crate::weapon::{PK_THUNDER_TRAIL_ENV, PK_THUNDER_TRAIL_PRIM};
    assert_eq!(
        PK_THUNDER_TRAIL_PRIM,
        [
            [0x5E, 0xA3, 0xFF],
            [0x98, 0xBD, 0xFF],
            [0xC2, 0xD9, 0xFF],
            [0xB3, 0xF1, 0xFF]
        ]
    );
    assert_eq!(
        PK_THUNDER_TRAIL_ENV,
        [
            [0x3A, 0x00, 0x83],
            [0x5B, 0x00, 0xB2],
            [0x86, 0x33, 0xD9],
            [0xA7, 0x74, 0xF8]
        ]
    );
    let mut f = fighter(FighterKind::Ness, 0, false);
    set_special_hi(&mut f);
    steps(&mut f, 24);
    let mut pool = WeaponPool::default();
    assert!(pool.spawn(f.take_weapon_spawn().unwrap()));
    pool.observe_owner(&f);
    for _ in 0..12 {
        pool.tick(core::iter::empty, None);
    }
    let mut seen = 0u8;
    for t in pool.pk_trails() {
        let i = usize::from(t.id);
        assert_eq!(
            t.colors(),
            (PK_THUNDER_TRAIL_PRIM[i], PK_THUNDER_TRAIL_ENV[i])
        );
        seen |= 1 << t.id;
    }
    assert_eq!(seen, 0b1111);
}

/// `wpNessPKThunderTrailProcUpdate` ends with `texture_id_curr =
/// syUtilsRandIntRange(WPPKTHUNDER_TEXTURES_NUM - 1)`: each live trail
/// draws one of three frames per update, in link order, after its trail
/// spawn and effect (RE-550). The flush keeps the frame on the trail and
/// draws exactly one random number per trail, so the shared sequence is the
/// same as before the frame was kept.
#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn pk_trails_keep_the_frame_their_update_draws() {
    let mut f = fighter(FighterKind::Ness, 0, false);
    set_special_hi(&mut f);
    steps(&mut f, 24);
    let mut pool = WeaponPool::default();
    assert!(pool.spawn(f.take_weapon_spawn().unwrap()));
    pool.observe_owner(&f);
    // Trail 0 is made on its first update, at `gcAddMObjForDObj`'s 0.
    let mut seen = [false; 3];
    crate::rng::set_seed(29);
    for _ in 0..40 {
        let before = crate::rng::seed();
        pool.tick(core::iter::empty, None);
        pool.flush_effects(&mut crate::effect::NoEffects);
        let after = crate::rng::seed();
        let mut trails: std::vec::Vec<_> = pool.pk_trails().collect();
        trails.sort_by_key(|t| t.id);
        crate::rng::set_seed(before);
        for t in &trails {
            let want = crate::rng::rand_int_range(3) as u8;
            assert_eq!(t.texture, want, "trail {}", t.id);
            seen[usize::from(t.texture)] = true;
        }
        assert_eq!(crate::rng::seed(), after, "one draw per live trail");
    }
    assert_eq!(seen, [true; 3]);
}
/// N64 (warp ROM, grounded Up+B, trace of links 5 and 6): the head's first
/// update is on its make's frame (t 68), and each trail runs its first
/// update on the frame it is made, so trails 0-3 come at t 70, 72, 74 and
/// 76, two frames apart, and trail 3 makes the trail effect at t 78 at the
/// head's position from t 68.
#[test]
fn pk_trails_come_two_frames_apart_and_the_effect_ten_after_the_head() {
    let bank = crate::particle::tests::TestBank::new(&[]);
    let mut particles = std::boxed::Box::new(crate::particle::Particles::new());
    let mut effects = crate::effect::Effects::new(0);
    let mut rt = crate::effect::EffectRuntime {
        particles: &mut particles,
        effects: &mut effects,
        banks: &bank,
    };
    let mut f = fighter(FighterKind::Ness, 0, false);
    set_special_hi(&mut f);
    steps(&mut f, 24);
    let mut pool = WeaponPool::default();
    assert!(pool.spawn(f.take_weapon_spawn().unwrap()));
    pool.observe_owner(&f);
    let mut first = [None; 4];
    let mut made = None;
    let mut head_first = None;
    for frame in 1..=12u32 {
        pk_frame(&mut pool, &mut rt);
        let head = pool.pk_thunders().next().unwrap().position;
        head_first.get_or_insert(head);
        for t in pool.pk_trails() {
            first[usize::from(t.id)].get_or_insert(frame);
        }
        if made.is_none() {
            if let Some(d) = pk_trail_effects(rt.effects).first() {
                made = Some(frame);
                let at = head_first.unwrap();
                close(d.translate.x, at.x);
                close(d.translate.y, at.y);
            }
        }
    }
    assert_eq!(first, [Some(3), Some(5), Some(7), Some(9)]);
    assert_eq!(made, Some(11));
}
