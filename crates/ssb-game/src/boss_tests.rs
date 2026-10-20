use super::*;
use crate::collision::Segment;
use crate::fighter::FighterKind;
use crate::ground::BodyColl;

/// A Final Destination-like floor: one line from -3000 to 3000 at y 0.
fn floor() -> [MapSurface; 1] {
    [crate::map::floor_surface((
        0,
        Segment {
            x1: -3000,
            y1: 0,
            x2: 3000,
            y2: 0,
            flags: 0,
        },
    ))]
}

fn boss() -> Fighter {
    let mut f = Fighter::new(FighterKind::Boss, 1, 0);
    // `dBossMain_attr.map_coll`.
    f.coll = BodyColl {
        top: 200.0,
        center: 100.0,
        bottom: 0.0,
        width: 120.0,
    };
    f.pos = Vec3::new(0.0, 1500.0, 0.0);
    init(&mut f, &floor, 9, false);
    f.boss.target = Some(Target {
        port: 0,
        pos: Vec3::new(-800.0, 0.0, 0.0),
        floor_line: Some(0),
    });
    f
}

fn step(f: &mut Fighter) {
    f.tick_map(floor);
}

#[test]
fn init_takes_the_first_floor_line_and_a_level_wait() {
    let f = boss();
    assert_eq!(f.boss.default_line, Some(0));
    assert_eq!(f.boss.wait_div, 1.0);
    assert_eq!(f.boss.status_id, -1);
    // `syUtilsRandIntRange(120) + 100 / 9` at the helper's level 9.
    assert!((11..131).contains(&f.boss.wait_timer));
}

/// The 1P boss at the default difficulty is level 2 (`sc1pgame.c` Final
/// Stage row); from seed 1 its first draw is 0, so it waits the minimum
/// `0 + 100 / 2` ticks (N64 trace, RE-554: `wait_timer` 50).
#[test]
fn a_level_two_boss_from_seed_one_waits_fifty_ticks() {
    let mut f = boss();
    crate::rng::set_seed(1);
    init(&mut f, &floor, 2, false);
    assert_eq!(f.boss.wait_timer, 50);
}

/// `ftMainProcUpdateInterrupt` reads `proc_interrupt` after `proc_update`:
/// the tick Appear ends in Wait already counts the wait down. Wait eases
/// toward its spot without `ftBossMoveProcPhysics`'s snap under 5 units
/// (N64 trace, RE-554: Wait from boss tick 601, Move at 650).
#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn wait_counts_down_from_the_tick_appear_ends_and_never_snaps() {
    let mut f = boss();
    let target = f.boss.target;
    f.pos = Vec3::ZERO;
    init(&mut f, &floor, 9, false);
    f.boss.target = target;
    f.boss.wait_timer = 50;
    appear_set_status(&mut f);
    for _ in 0..APPEAR_FRAMES as usize {
        step(&mut f);
    }
    assert_eq!(f.status.status, AnyStatus::Boss(B::Wait));
    assert_eq!(f.boss.wait_timer, 49);
    for _ in 0..48 {
        step(&mut f);
    }
    assert_eq!(f.status.status, AnyStatus::Boss(B::Wait));
    let gap = (f.boss.pos - f.pos).length();
    assert!(gap > 0.0 && gap < 5.0, "{gap}");
    step(&mut f);
    assert_ne!(f.status.status, AnyStatus::Boss(B::Wait));
}

#[test]
fn the_entry_lasts_its_clip_then_waits_above_the_floor() {
    // Spawned on the floor's plane, as the boss stage's spawn point is.
    let mut f = boss();
    let target = f.boss.target;
    f.pos = Vec3::ZERO;
    init(&mut f, &floor, 9, false);
    f.boss.target = target;
    let spawn = f.pos;
    appear_set_status(&mut f);
    assert_eq!(f.status.status, AnyStatus::Boss(B::Appear));
    assert!(f.boss.lr_zero);
    assert_eq!(model_yaw(&f), Some(0.0));
    for _ in 0..APPEAR_FRAMES as usize {
        step(&mut f);
    }
    assert_eq!(f.status.status, AnyStatus::Boss(B::Wait));
    assert!(!f.boss.lr_zero);
    assert_eq!(f.pos.x, spawn.x);
    // `ftBossWaitSetStatus` from the spawn: 400 above the floor below it.
    assert_eq!(f.boss.pos, Vec3::new(0.0, 400.0, 0.0));
}

#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn an_entry_over_the_floor_waits_above_its_own_spawn_point() {
    // `ftManagerMakeFighter` keeps the floor line under the spawn point,
    // so `ftBossWaitSetStatus` stays over it instead of the line's middle
    // (the N64 boss trace: x 900 through the first Wait).
    let mut f = Fighter::new(FighterKind::Boss, 1, 0);
    f.coll = boss().coll;
    f.pos = Vec3::new(900.0, 0.0, 0.0);
    // The spawn's probe; Appear's own map proc probes from the clip's pose.
    init(&mut f, &floor, 9, false);
    assert_eq!(f.boss.floor_line, Some(0));
    f.boss.target = boss().boss.target;
    appear_set_status(&mut f);
    for _ in 0..APPEAR_FRAMES as usize {
        step(&mut f);
    }
    assert_eq!(f.status.status, AnyStatus::Boss(B::Wait));
    assert_eq!(f.boss.pos, Vec3::new(900.0, 400.0, 0.0));
    // Off the stage there is no line: the middle of the target's.
    let mut f = Fighter::new(FighterKind::Boss, 1, 0);
    f.pos = Vec3::new(5000.0, 0.0, 0.0);
    init(&mut f, &floor, 9, false);
    assert_eq!(f.boss.floor_line, None);
}

#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn every_attack_returns_to_wait_on_a_flat_floor() {
    let mut f = boss();
    set_wait(&mut f, &floor);
    let mut seen = std::collections::BTreeSet::new();
    for _ in 0..20_000 {
        step(&mut f);
        // The finger gun's shots are the host's to make.
        f.weapon_spawn = None;
        f.boss.camera = None;
        // The slam's clip carries the hand back to the front plane through
        // TransN, which a host test does not sample.
        if f.status.status == AnyStatus::Boss(B::Wait) {
            f.pos.z = 0.0;
        }
        if let AnyStatus::Boss(s) = f.status.status {
            seen.insert(s);
        }
        assert!(f.pos.x.is_finite() && f.pos.y.is_finite() && f.pos.z.is_finite());
    }
    for s in [
        B::Move,
        B::Hippataku,
        B::Harau,
        B::Okuhikouki1,
        B::Okuhikouki2,
        B::Okuhikouki3,
        B::Walk,
        B::WalkLoop,
        B::GootsubusuUp,
        B::GootsubusuWait,
        B::GootsubusuDown,
        B::GootsubusuEnd,
        B::Tsutsuku1,
        B::Tsutsuku2,
        B::Tsutsuku3,
        B::Drill,
        B::Okukouki,
        B::Yubideppou1,
        B::Yubideppou2,
        B::Yubideppou3,
        B::Okupunch1,
        B::Okupunch2,
        B::Okupunch3,
        B::OkutsubushiStart,
        B::Okutsubushi,
    ] {
        assert!(seen.contains(&s), "never entered {s:?}: {seen:?}");
    }
}

#[test]
fn the_background_attacks_request_the_map_zoom_and_fog() {
    let mut f = boss();
    f.boss.pos = Vec3::new(0.0, 0.0, 0.0);
    set_okuhikouki2(&mut f);
    assert!(f.boss.lr_zero);
    assert_eq!(f.pos, Vec3::new(-9000.0, 6000.0, BACKGROUND_Z));
    assert_eq!(
        f.boss.camera,
        Some(CameraRequest::MapZoom {
            at: Vec3::ZERO,
            eye: BACKGROUND_ZOOM_EYE
        })
    );
    // At z -15000 the fog is `0x80`.
    update_fog_color(&mut f);
    assert_eq!(f.boss.fog, Some(0x80));
    f.pos.z = 0.0;
    update_fog_color(&mut f);
    assert_eq!(f.boss.fog, Some(0xFF));
}

#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn three_hundred_damage_defeats_and_two_hundred_enrages() {
    let mut f = boss();
    set_wait(&mut f, &floor);
    f.damage = 200;
    update_damage_stats(&mut f);
    assert_eq!(f.boss.wait_div, ENRAGED_WAIT_DIV);
    assert!(!f.boss.defeated);
    f.facing = Facing::Left;
    f.damage = 300;
    update_damage_stats(&mut f);
    assert!(f.boss.defeated);
    assert_eq!(f.status.status, AnyStatus::Boss(B::DeadLeft));
    // Dead Left runs its clip, then flies off for 200 ticks and drifts on.
    for _ in 0..20 {
        step(&mut f);
    }
    assert_eq!(f.status.status, AnyStatus::Boss(B::DeadCenter));
    let (sin, cos) = sin_cos(45.0_f32.to_radians());
    assert_eq!(f.physics.vel_air, Vec3::new(0.0, sin * 100.0, -cos * 100.0));
    // Damage taken while dying changes nothing.
    f.damage = 400;
    update_damage_stats(&mut f);
    assert_eq!(f.status.status, AnyStatus::Boss(B::DeadCenter));
}

#[test]
fn facing_right_falls_through_dead_right() {
    let mut f = boss();
    set_wait(&mut f, &floor);
    f.facing = Facing::Right;
    f.damage = 300;
    update_damage_stats(&mut f);
    assert_eq!(f.status.status, AnyStatus::Boss(B::DeadRight));
    assert_eq!(f.facing, Facing::Left);
}

#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn the_first_attack_choice_reads_before_the_group_table() {
    // `dFTBossWaitRandomArrayLookup[-3..-1]`: the tail of
    // `dFTBossWaitRandomNoGround`.
    assert_eq!(
        [
            wait_byte(ARRAY_LOOKUP - 3),
            wait_byte(ARRAY_LOOKUP - 2),
            wait_byte(ARRAY_LOOKUP - 1)
        ],
        [3, 10, 1]
    );
    // Group 10's rows lie past both tables.
    assert_eq!(
        [wait_byte(RANDOM_GROUND + 20), wait_byte(RANDOM_GROUND + 21)],
        [2, 3]
    );
    assert_eq!(
        [
            wait_byte(RANDOM_NO_GROUND + 20),
            wait_byte(RANDOM_NO_GROUND + 21)
        ],
        [0, 0]
    );
    // From group 10, every roll reads zero: group 0.
    for r in 0..3 {
        assert_eq!(wait_byte(ARRAY_LOOKUP + 30 + r), 0);
    }
    // `dFTBossWaitRandomStatusIDs` holds the status ids.
    assert_eq!(wait_byte(STATUS_IDS), B::Hippataku as u8);
    assert_eq!(wait_byte(STATUS_IDS + 10), B::Yubideppou1 as u8);
}

#[test]
fn the_finger_gun_fires_from_its_joints() {
    let mut f = boss();
    set_wait(&mut f, &floor);
    f.facing = Facing::Left;
    set_yubideppou3(&mut f, false);
    let spawn = f.weapon_spawn.take().expect("first shot");
    assert_eq!(spawn.kind, WeaponKind::BossBullet { hard: false });
    assert_eq!(spawn.facing, -1.0);
    for _ in 0..3 {
        step(&mut f);
        assert!(f.weapon_spawn.is_none());
    }
    step(&mut f);
    assert!(
        f.weapon_spawn.take().is_some(),
        "the follow-up shot on tick 4"
    );
    // Enraged, the first two of three are hard.
    f.boss.wait_div = ENRAGED_WAIT_DIV;
    f.boss.bullet_count = 0;
    set_yubideppou3(&mut f, false);
    assert_eq!(
        f.weapon_spawn.take().map(|s| s.kind),
        Some(WeaponKind::BossBullet { hard: true })
    );
}

#[test]
fn the_slot_order_follows_the_motion_enum() {
    assert_eq!(anim_slot(B::Wait), SLOT_DEFAULT);
    assert_eq!(anim_slot(B::OkutsubushiStart), anim_slot(B::Okupunch1));
    assert_eq!(anim_slot(B::Appear), SLOT_DEFAULT + 29);
}

/// `ftBossDeadCenterSetStatus` takes `F_CST_DTOR32(45.0F)`; at 45 degrees
/// the rounded constant and the calculated one give the same angle bits,
/// so the launch velocity is the decomp's to the bit.
#[test]
fn dead_center_launch_uses_the_cst_angle() {
    use ssb_engine::math::{cosf, f_clc_dtor32, sinf};
    let a = f_cst_dtor32(45.0);
    assert_eq!(a.to_bits(), f_clc_dtor32(45.0).to_bits());
    assert_eq!(a.to_bits(), 45.0_f32.to_radians().to_bits());
    let mut f = Fighter::new(FighterKind::Boss, 0, 1);
    set_dead_center(&mut f);
    assert_eq!(f.physics.vel_air.y.to_bits(), (sinf(a) * 100.0).to_bits());
    assert_eq!(
        f.physics.vel_air.z.to_bits(),
        (-(cosf(a) * 100.0)).to_bits()
    );
    assert_eq!(f.physics.vel_air.x, 0.0);
}

/// `ftBossYubideppou3ProcUpdate` re-enters the status and then queries
/// joint 15 (ftbossyubideppou3.c:10-76): `ftMainSetStatus` plays nothing,
/// so the query reads the play the replaced status had this frame.
/// `ftBossYubideppou2ProcPhysics` and `ftBossYubideppou3ProcPhysics`
/// query in `proc_physics`, from this frame's play: the host samples it
/// before the physics step on exactly those frames.
#[test]
fn the_finger_gun_reads_the_play_its_maker_sees() {
    use crate::fighter::JointTransform;
    let at = |origin| {
        Some(JointTransform {
            axes: [Vec3::X, Vec3::Y, Vec3::Z],
            origin,
        })
    };
    let mut f = boss();
    set_wait(&mut f, &floor);
    let replaced = f.status;
    set_yubideppou3(&mut f, true);
    assert_eq!(crate::spawn_pose::played_status(&f), Some(replaced));
    let queued = f.weapon_spawn.unwrap().position;
    f.joint_transforms[usize::from(BULLET_JOINT_FIRST)] = at(Vec3::new(10.0, 20.0, 30.0));
    crate::spawn_pose::place_update(&mut f);
    assert_eq!(f.weapon_spawn.unwrap().position, queued);
    crate::spawn_pose::place_played(&mut f);
    assert_eq!(
        f.weapon_spawn.take().unwrap().position,
        Vec3::new(10.0, 20.0, 30.0)
    );
    // The follow-up shot from `proc_physics`.
    for tick in 0..6 {
        let queries = physics_queries_joints(&f);
        step(&mut f);
        assert_eq!(f.weapon_spawn.take().is_some(), queries, "tick {tick}");
    }
    // `ftBossYubideppou2ProcPhysics`'s set and shot, then the follow-up.
    set_yubideppou2(&mut f);
    f.boss.timer = 3;
    let mut made = 0;
    for tick in 0..10 {
        let queries = physics_queries_joints(&f);
        step(&mut f);
        let shot = f.weapon_spawn.take().is_some();
        assert_eq!(shot, queries, "tick {tick}");
        made += usize::from(shot);
    }
    assert_eq!(made, 2, "the set's shot and the follow-up");
}
