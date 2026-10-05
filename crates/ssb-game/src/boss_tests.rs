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
    // `syUtilsRandIntRange(120) + 100 / 9`.
    assert!((11..131).contains(&f.boss.wait_timer));
}

#[test]
fn the_entry_lasts_its_clip_then_waits_above_the_floor() {
    let mut f = boss();
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
    set_yubideppou3(&mut f);
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
    set_yubideppou3(&mut f);
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
