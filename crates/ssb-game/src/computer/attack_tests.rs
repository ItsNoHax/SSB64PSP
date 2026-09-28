use super::*;
use crate::collision::Segment;
use crate::dead::StageBounds;
use crate::ground::Standing;
use crate::status::BlastZone;
use crate::weapon::MapSurface;
use ssb_engine::math::Vec3;

use super::super::behave::{Opponent, WeaponThreat};
use super::super::INPUT_SCRIPTS;

/// One floor from -2000 to 2000 at y 0.
fn flat() -> [MapSurface; 1] {
    [MapSurface {
        kind: MapSurfaceKind::Floor,
        segment: Segment {
            x1: -2000,
            y1: 0,
            x2: 2000,
            y2: 0,
            flags: 0,
        },
        topology: None,
        motion: None,
    }]
}

fn zone(top: f32, bottom: f32, left: f32, right: f32) -> BlastZone {
    BlastZone {
        top,
        bottom,
        left,
        right,
    }
}

fn world<'a>(
    opponents: &'a [Opponent],
    weapon_threats: &'a [WeaponThreat],
) -> World<'a, impl Fn() -> [MapSurface; 1]> {
    World {
        surfaces: flat,
        geometry: zone(0.0, 0.0, -2000.0, 2000.0),
        stage: StageBounds {
            map: zone(6000.0, -3000.0, -6000.0, 6000.0),
            camera: zone(4000.0, -1000.0, -4000.0, 4000.0),
            rebirth: Vec2::new(0.0, 2000.0),
        },
        gkind: None,
        opponents,
        item_attacks: &[],
        weapon_threats,
        twister: None,
        acid: None,
    }
}

fn standing_mario(x: f32) -> Fighter {
    let mut f = Fighter::new(FighterKind::Mario, 1, 3);
    f.attributes = crate::physics::PhysicsAttributes::MARIO;
    f.coll = crate::ground::BodyColl::MARIO;
    f.pos = Vec3::new(x, 0.0, 0.0);
    f.situation = Situation::Ground;
    f.floor = Some(Standing {
        line: 0,
        flags: 0,
        normal: Vec2::new(0.0, 1.0),
    });
    f
}

fn player_at(x: f32) -> Opponent {
    Opponent {
        pos: Vec3::new(x, 0.0, 0.0),
        vel_air: Vec3::ZERO,
        status: AnyStatus::Common(Status::Wait),
        grounded: true,
        floor_line: Some(0),
        facing: Facing::Left,
        damage: 0,
        star_invincible: false,
        has_hammer: false,
        kind: FighterKind::Mario,
        damage_size: Vec2::new(150.0, 300.0),
        tvel_base: 80.0,
        gravity: 3.5,
    }
}

fn vs_cpu(f: &Fighter, level: u8) -> Computer {
    let mut com = Computer::setup(f, level);
    com.setup_cliffs(&world(&[], &[]));
    com
}

#[test]
fn every_attack_names_a_script() {
    for (ground, air) in ATTACKS.iter() {
        for a in ground.iter().chain(air.iter()) {
            assert!(a.input < INPUT_SCRIPTS.len());
            assert!(a.detect_near_x <= a.detect_far_x);
            assert!(a.hit_start_frame <= a.hit_end_frame || a.hit_end_frame == 0);
        }
    }
    // Mario's ground table starts with the jab, 2 frames to its hit.
    assert_eq!(ATTACKS[0].0[0].input, input::STICK_N_BUTTON_A);
    assert_eq!(ATTACKS[0].0[0].hit_start_frame, 2);
}

#[test]
fn the_first_behaviour_change_waits_by_level() {
    let f = standing_mario(0.0);
    assert_eq!(Computer::setup(&f, 3).behavior_change_wait, 720);
    assert_eq!(Computer::setup(&f, 9).behavior_change_wait, 0);
}

#[test]
fn the_default_trait_changes_behaviour_when_the_wait_runs_out() {
    crate::rng::set_seed(1);
    let f = standing_mario(0.0);
    let mut com = vs_cpu(&f, 9);
    com.process_trait();
    assert!(matches!(
        com.behavior,
        Behavior::Default | Behavior::Unk2 | Behavior::Ally | Behavior::Captain
    ));
    let range = if com.behavior == Behavior::Ally {
        225..450
    } else {
        900..1800
    };
    assert!(
        range.contains(&com.behavior_change_wait),
        "{}",
        com.behavior_change_wait
    );
    // No trait keeps the behaviour.
    let mut com = vs_cpu(&f, 9);
    com.trait_kind = Trait::None;
    com.behavior = Behavior::Stand;
    com.process_trait();
    assert_eq!(com.behavior, Behavior::Stand);
}

#[test]
fn a_close_opponent_gives_the_attack_objective() {
    let f = standing_mario(0.0);
    let mut com = vs_cpu(&f, 3);
    let opponents = [player_at(300.0)];
    let w = world(&opponents, &[]);
    com.process_trait();
    assert_eq!(com.proc_default(&f, &w), 1);
    assert_eq!(com.objective, Objective::Attack);
    // Far away, the behaviour's base objective.
    let opponents = [player_at(1500.0)];
    let w = world(&opponents, &[]);
    com.behavior = Behavior::Captain;
    com.process_trait();
    assert_eq!(com.proc_default(&f, &w), 1);
    assert_eq!(com.objective, Objective::Patrol);
}

#[test]
fn a_target_in_reach_picks_an_attack_from_the_table() {
    let f = standing_mario(0.0);
    let opponents = [player_at(250.0)];
    let w = world(&opponents, &[]);
    let ground: Vec<usize> = ATTACKS[0].0.iter().map(|a| a.input).collect();
    for seed in 0..20 {
        crate::rng::set_seed(seed);
        let mut com = vs_cpu(&f, 9);
        assert!(com.find_target(&f, &w));
        assert!(com.detect_target(&f, &w, 0.0), "seed {seed}");
        let script = com.command.map(|c| c.0).unwrap();
        assert!(
            ground.contains(&script) || script == input::MOVE_AUTO_STICK_TILT_HI_RELEASE_Z,
            "seed {seed}: script {script}"
        );
        assert_eq!(com.input_kind, Some(script));
    }
    // Out of every box, nothing.
    let opponents = [player_at(1900.0)];
    let w = world(&opponents, &[]);
    let mut com = vs_cpu(&f, 9);
    assert!(com.find_target(&f, &w));
    assert!(!com.detect_target(&f, &w, 0.0));
    assert_eq!(com.command, None);
}

#[test]
fn a_target_behind_is_out_of_the_forward_boxes() {
    // Facing right with the target 600 behind: only the moves reaching
    // backward (none reach that far) could fire.
    let mut f = standing_mario(0.0);
    f.facing = Facing::Right;
    let opponents = [player_at(-600.0)];
    let w = world(&opponents, &[]);
    let mut com = vs_cpu(&f, 9);
    assert!(com.find_target(&f, &w));
    assert!(!com.detect_target(&f, &w, 0.0));
}

#[test]
fn a_fourth_repeat_of_an_attack_turns_into_a_jump() {
    let f = standing_mario(0.0);
    let opponents = [player_at(250.0)];
    let w = world(&opponents, &[]);
    let mut jumped = 0;
    for seed in 0..100 {
        crate::rng::set_seed(seed);
        let mut com = vs_cpu(&f, 9);
        com.find_target(&f, &w);
        com.input_kind = Some(input::STICK_N_BUTTON_A);
        com.input_repeat_count = 3;
        assert!(com.detect_target(&f, &w, 0.0));
        if com.input_kind == Some(input::STICK_N_BUTTON_A) {
            // The jab again: its fourth repeat jumps instead.
            assert_eq!(
                com.command.map(|c| c.0),
                Some(input::MOVE_AUTO_STICK_TILT_HI_RELEASE_Z)
            );
            jumped += 1;
        } else {
            assert_eq!(com.input_repeat_count, 0);
        }
    }
    assert!(jumped > 0, "the jab was never picked again");
}

#[test]
fn a_shielding_cpu_lets_go_of_the_shield() {
    let mut f = standing_mario(0.0);
    f.status.status = AnyStatus::Common(Status::Guard);
    let mut com = vs_cpu(&f, 3);
    let opponents = [player_at(1000.0)];
    let w = world(&opponents, &[]);
    assert_eq!(com.proc_default(&f, &w), 0);
    assert_eq!(com.command.map(|c| c.0), Some(input::BUTTON_Z_RELEASE));
}

#[test]
fn an_incoming_weapon_is_answered() {
    let f = standing_mario(0.0);
    let opponents = [player_at(1500.0)];
    // A shot 400 to the left flying right at 60: it arrives in about five
    // frames at the fighter's height.
    let threats = [WeaponThreat {
        pos: Vec2::new(-400.0, 100.0),
        vel_x: 60.0,
        lr: 1.0,
        size: 100.0,
    }];
    let w = world(&opponents, &threats);
    let mut answered = 0;
    for seed in 0..50 {
        crate::rng::set_seed(seed);
        let mut com = vs_cpu(&f, 9);
        if com.proc_default(&f, &w) == 1 && com.objective == Objective::CounterAttack {
            assert!(com.hit_predict > 0.0 && com.hit_predict < 15.0);
            answered += 1;
        }
    }
    // Level 9 answers every time: (9 + 2) / 9 is above 1.
    assert_eq!(answered, 50);
    // A shot flying away is ignored.
    let threats = [WeaponThreat {
        lr: -1.0,
        vel_x: -60.0,
        ..threats[0]
    }];
    let w = world(&opponents, &threats);
    let mut com = vs_cpu(&f, 9);
    com.proc_default(&f, &w);
    assert_ne!(com.objective, Objective::CounterAttack);
}

#[test]
fn a_vs_cpu_fights_a_standing_player() {
    // Run the whole loop: a level-3 CPU next to a player waits out its
    // follow delay (96 to 120 ticks), then presses A or B. Level 3 does not
    // roll past a target in front of it; this fighter never moves, so a
    // roll would repeat forever.
    let f = standing_mario(0.0);
    let opponents = [player_at(300.0)];
    let w = world(&opponents, &[]);
    crate::rng::set_seed(3);
    let mut com = vs_cpu(&f, 3);
    let mut pressed = false;
    for _ in 0..240 {
        com.process(&f, &w);
        pressed |= com.buttons.contains(ssb_engine::input::N64Buttons::A)
            || com.buttons.contains(ssb_engine::input::N64Buttons::B);
    }
    assert!(pressed, "the CPU never attacked");
}
