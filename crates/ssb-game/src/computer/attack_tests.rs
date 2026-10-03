use super::*;
use crate::collision::Segment;
use crate::dead::StageBounds;
use crate::ground::Standing;
use crate::status::BlastZone;
use crate::weapon::MapSurface;
use ssb_engine::math::Vec3;

use super::super::behave::{ItemSight, Opponent, WeaponThreat};
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
            fog_color: [0; 3],
        },
        gkind: None,
        opponents,
        items: &[],
        weapon_threats,
        team_rules: crate::team::TeamRules::FREE_FOR_ALL,
        is_1p_game: false,
        pk_thunder_trail: None,
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
    com.process_trait(&f);
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
    com.process_trait(&f);
    assert_eq!(com.behavior, Behavior::Stand);
}

#[test]
fn the_1p_traits_set_their_behaviours() {
    let mut f = standing_mario(0.0);
    let mut com = vs_cpu(&f, 3);
    for (t, b) in [
        (Trait::YoshiTeam, Behavior::YoshiTeam),
        (Trait::KirbyTeam, Behavior::KirbyTeam),
        (Trait::PolyTeam, Behavior::PolyTeam),
        (Trait::GiantDonkey, Behavior::Default),
        (Trait::Unk1, Behavior::Unk2),
        (Trait::Bonus3, Behavior::Bonus3),
        (Trait::Ally, Behavior::Ally),
        (Trait::MarioBros, Behavior::Default),
    ] {
        com.trait_kind = t;
        com.process_trait(&f);
        assert_eq!(com.behavior, b, "{t:?}");
    }
    assert_eq!(com.objective_base, Objective::Attack);
    com.trait_kind = Trait::Bonus3;
    com.process_trait(&f);
    assert_eq!(com.objective_base, Objective::Rush);
    // Luigi backs Mario up.
    f.kind = FighterKind::Luigi;
    com.trait_kind = Trait::MarioBros;
    com.process_trait(&f);
    assert_eq!(com.behavior, Behavior::Ally);
    assert_eq!(com.objective_base, Objective::Ally);
}

#[test]
fn the_link_trait_stands_until_hurt_or_the_wait_runs_out() {
    let mut f = standing_mario(0.0);
    let mut com = vs_cpu(&f, 3);
    com.trait_kind = Trait::Link;
    com.process_trait(&f);
    assert_eq!(com.behavior, Behavior::Stand);
    f.damage = 13;
    com.process_trait(&f);
    assert_eq!(com.behavior, Behavior::Stand);
    f.damage = 14;
    com.process_trait(&f);
    assert_eq!(com.behavior, Behavior::Default);
    f.damage = 0;
    com.behavior_change_wait = 0;
    com.process_trait(&f);
    assert_eq!(com.behavior, Behavior::Default);
}

#[test]
fn the_rush_objective_walks_at_a_far_target_and_attacks_a_near_one() {
    crate::rng::set_seed(3);
    let f = standing_mario(0.0);
    let mut com = vs_cpu(&f, 9);
    com.trait_kind = Trait::Bonus3;
    com.process_trait(&f);
    // Far: walk toward it.
    let opponents = [player_at(1900.0)];
    let w = world(&opponents, &[]);
    com.walk_stop_wait = 7;
    com.follow_rush(&f, &w);
    assert_eq!(com.walk_stop_wait, 0);
    assert!(com.command.is_some());
    assert!(com.target_pos.x > 1000.0, "{}", com.target_pos.x);
    // Close: an attack script.
    let mut com = vs_cpu(&f, 9);
    let opponents = [player_at(250.0)];
    let w = world(&opponents, &[]);
    com.follow_rush(&f, &w);
    assert!(com.input_kind.is_some());
    // No target: wander, counting the stop wait.
    let mut com = vs_cpu(&f, 9);
    let w = world(&[], &[]);
    com.follow_rush(&f, &w);
    assert_eq!(com.walk_stop_wait, 1);
}

#[test]
fn a_close_opponent_gives_the_attack_objective() {
    let f = standing_mario(0.0);
    let mut com = vs_cpu(&f, 3);
    let opponents = [player_at(300.0)];
    let w = world(&opponents, &[]);
    com.process_trait(&f);
    assert_eq!(com.proc_default(&f, &w), 1);
    assert_eq!(com.objective, Objective::Attack);
    // Far away, the behaviour's base objective.
    let opponents = [player_at(1500.0)];
    let w = world(&opponents, &[]);
    com.behavior = Behavior::Captain;
    com.process_trait(&f);
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
        owner: 9,
        team: 9,
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

fn item_world<'a>(
    opponents: &'a [Opponent],
    items: &'a [ItemSight],
) -> World<'a, impl Fn() -> [MapSurface; 1]> {
    World {
        items,
        ..world(opponents, &[])
    }
}

/// A light item resting on the floor at `x`, free to pick up.
fn item_at(x: f32) -> ItemSight {
    ItemSight {
        pos: Vec3::new(x, 0.0, 0.0),
        owner: None,
        team: crate::team::TEAM_DEFAULT,
        kind: ItemKind::Equipment(equipment::Kind::Bat),
        weight: ItemWeight::Light,
        is_allow_pickup: true,
        is_damage_all: false,
        floor_line: Some(0),
        coll: crate::ground::BodyColl {
            top: 100.0,
            center: 50.0,
            bottom: 0.0,
            width: 100.0,
        },
        vel_x: 0.0,
        lr: 1.0,
        attack_live: false,
        attack_size: 0.0,
        attack_count: 0,
        attack_pos: [Vec2::ZERO; crate::item::ATTACK_COLLS],
    }
}

fn holding(f: &mut Fighter, kind: ItemKind, ty: ItemType, multi: u16) {
    f.items.held = Some(crate::item::HeldItem {
        slot: 0,
        kind,
        ty,
        weight: ItemWeight::Light,
    });
    f.items.held_multi = multi;
}

#[test]
fn the_nearest_free_item_is_the_target() {
    let f = standing_mario(0.0);
    let mut own = item_at(100.0);
    own.owner = Some(f.port);
    let mut held = item_at(150.0);
    held.is_allow_pickup = false;
    let mut flying = item_at(-300.0);
    flying.floor_line = None;
    let items = [own, held, item_at(900.0), flying, item_at(-600.0)];
    let w = item_world(&[], &items);
    let mut com = vs_cpu(&f, 3);
    com.target_user = Some(0);
    com.stop_at_ledged_target = true;
    assert!(com.find_item(&f, &w));
    // Its own item and the one held are passed by; the airborne one is
    // nearest of the rest and has no floor line.
    assert_eq!(com.target_item, Some(3));
    assert_eq!(com.target_user, None);
    assert_eq!(com.target_line, None);
    assert_eq!(com.target_dist, 300.0);
    assert!(!com.stop_at_ledged_target);
    // A held item stops the search.
    let mut f = f;
    holding(&mut f, ItemKind::MBall, ItemType::Throw, 0);
    assert!(!com.find_item(&f, &w));
    // Off the stage there is nothing to find.
    let off = [item_at(3000.0)];
    let w = item_world(&[], &off);
    let f = standing_mario(0.0);
    assert!(!com.find_item(&f, &w));
}

#[test]
fn a_teammates_item_is_passed_by_without_team_attack() {
    let mut f = standing_mario(0.0);
    f.team = 0;
    let mut mate = item_at(100.0);
    mate.team = 0;
    let items = [mate, item_at(500.0)];
    let mut w = item_world(&[], &items);
    w.team_rules = crate::team::TeamRules::TEAMS;
    let mut com = vs_cpu(&f, 3);
    assert!(com.find_item(&f, &w));
    assert_eq!(com.target_item, Some(1));
    w.team_rules = crate::team::TeamRules::FREE_FOR_ALL;
    assert!(com.find_item(&f, &w));
    assert_eq!(com.target_item, Some(0));
}

#[test]
fn the_cpu_tracks_an_item_after_its_level_wait() {
    // Level 3 in VS waits 225 - 75 = 150 ticks before tracking; the
    // opponent is too far to attack.
    let f = standing_mario(0.0);
    let opponents = [player_at(1900.0)];
    let items = [item_at(800.0)];
    let w = item_world(&opponents, &items);
    let mut com = vs_cpu(&f, 3);
    com.process_trait(&f);
    for tick in 1..=150 {
        assert_eq!(com.proc_default(&f, &w), 1);
        assert_ne!(com.objective, Objective::TrackItem, "tick {tick}");
    }
    assert_eq!(com.proc_default(&f, &w), 1);
    assert_eq!(com.objective, Objective::TrackItem);
    assert_eq!(com.target_pos, Vec2::new(800.0, 0.0));
    // In 1P the wait is 315 - 105 = 210.
    let mut w = item_world(&opponents, &items);
    w.is_1p_game = true;
    let mut com = vs_cpu(&f, 3);
    com.process_trait(&f);
    for _ in 0..210 {
        com.proc_default(&f, &w);
        assert_ne!(com.objective, Objective::TrackItem);
    }
    com.proc_default(&f, &w);
    assert_eq!(com.objective, Objective::TrackItem);
    // Beyond 400 * (level + 3) the count stands still; with no item it
    // resets.
    let far = [item_at(1950.0)];
    let w = item_world(&opponents, &far);
    let mut com = vs_cpu(&f, 1);
    com.process_trait(&f);
    com.item_track_wait = 7;
    com.proc_default(&f, &w);
    assert_eq!(com.item_track_wait, 7);
    let w = item_world(&opponents, &[]);
    com.proc_default(&f, &w);
    assert_eq!(com.item_track_wait, 0);
}

#[test]
fn a_tracked_item_in_reach_is_picked_up() {
    let mut f = standing_mario(0.0);
    f.facing = Facing::Right;
    f.status.status = AnyStatus::Common(Status::Wait);
    // Mario's light reach: 105 ahead, 378 + the item's 100 either side.
    let items = [item_at(500.0)];
    let w = item_world(&[], &items);
    let mut com = vs_cpu(&f, 9);
    assert!(com.find_item(&f, &w));
    assert!(com.target_item_in_range(&f, &w));
    com.follow_track_item(&f, &w);
    assert_eq!(
        com.command.map(|c| c.0),
        Some(input::STICK_N_BUTTON_B_Z_RELEASE_A_PRESS)
    );
    // Out of reach it walks there.
    let items = [item_at(700.0)];
    let w = item_world(&[], &items);
    let mut com = vs_cpu(&f, 9);
    assert!(com.find_item(&f, &w));
    assert!(!com.target_item_in_range(&f, &w));
    com.follow_track_item(&f, &w);
    assert_eq!(com.command.map(|c| c.0), Some(input::MOVE_AUTO));
    // Running, it does not stop to pick up.
    f.status.status = AnyStatus::Common(Status::Run);
    let items = [item_at(500.0)];
    let w = item_world(&[], &items);
    let mut com = vs_cpu(&f, 9);
    assert!(com.find_item(&f, &w));
    com.follow_track_item(&f, &w);
    assert_eq!(com.command.map(|c| c.0), Some(input::MOVE_AUTO));
}

#[test]
fn a_held_item_chooses_use_or_attack() {
    let opponents = [player_at(1900.0)];
    let w = item_world(&opponents, &[]);
    for (ty, objective) in [
        (ItemType::Throw, Objective::UseItem),
        (ItemType::Shoot, Objective::UseItem),
        (ItemType::Damage, Objective::UseItem),
        (ItemType::Swing, Objective::Attack),
    ] {
        let mut f = standing_mario(0.0);
        holding(&mut f, ItemKind::MBall, ty, 0);
        let mut com = vs_cpu(&f, 3);
        com.behavior = Behavior::Captain;
        com.process_trait(&f);
        assert_eq!(com.proc_default(&f, &w), 1);
        assert_eq!(com.objective, objective, "{ty:?}");
    }
}

#[test]
fn a_throwing_item_is_thrown() {
    let opponents = [player_at(1000.0)];
    let w = item_world(&opponents, &[]);
    // A Poké Ball goes after the wait.
    let mut f = standing_mario(0.0);
    holding(&mut f, ItemKind::MBall, ItemType::Throw, 0);
    let mut com = vs_cpu(&f, 3);
    com.follow_use_item(&f, &w);
    assert_eq!(com.command.map(|c| c.0), Some(input::THROW_ITEM_WAIT));
    // Others go at once until the third empty shot.
    let mut f = standing_mario(0.0);
    holding(&mut f, ItemKind::BombHei, ItemType::Throw, 0);
    let mut com = vs_cpu(&f, 3);
    com.follow_use_item(&f, &w);
    assert_eq!(com.command.map(|c| c.0), Some(input::THROW_ITEM_IMMEDIATE));
    assert_eq!(com.item_throw_wait, 0);
    let gun = ItemKind::Equipment(equipment::Kind::RayGun);
    holding(&mut f, gun, ItemType::Shoot, 0);
    for wait in 1..=2 {
        com.follow_use_item(&f, &w);
        assert_eq!(com.item_throw_wait, wait);
        assert_eq!(com.command.map(|c| c.0), Some(input::THROW_ITEM_IMMEDIATE));
    }
    com.follow_use_item(&f, &w);
    assert_eq!(com.item_throw_wait, 0);
    assert_eq!(com.command.map(|c| c.0), Some(input::THROW_ITEM_WAIT));
}

#[test]
fn a_loaded_gun_fires_at_a_target_on_its_level() {
    let mut f = standing_mario(0.0);
    f.facing = Facing::Right;
    let gun = ItemKind::Equipment(equipment::Kind::RayGun);
    holding(&mut f, gun, ItemType::Shoot, 5);
    let opponents = [player_at(1000.0)];
    let w = item_world(&opponents, &[]);
    let mut com = vs_cpu(&f, 3);
    com.item_throw_wait = 2;
    com.follow_use_item(&f, &w);
    assert_eq!(
        com.command.map(|c| c.0),
        Some(input::STICK_TILT_AUTO_X_BUTTON_B_Z_RELEASE_A_PRESS)
    );
    assert_eq!(com.item_throw_wait, 0);
    // The Fire Flower's 1,500 reach: a farther target is walked to.
    let flower = ItemKind::Equipment(equipment::Kind::FireFlower);
    holding(&mut f, flower, ItemType::Shoot, 5);
    let opponents = [player_at(1800.0)];
    let w = item_world(&opponents, &[]);
    let mut com = vs_cpu(&f, 3);
    com.follow_use_item(&f, &w);
    assert_eq!(com.command.map(|c| c.0), Some(input::MOVE_AUTO));
    // From level 5 a reflector user gets the gun thrown at it.
    let mut fox = player_at(1000.0);
    fox.kind = FighterKind::Fox;
    let opponents = [fox];
    let w = item_world(&opponents, &[]);
    holding(&mut f, gun, ItemType::Shoot, 5);
    let mut com = vs_cpu(&f, 5);
    com.follow_use_item(&f, &w);
    assert_eq!(com.command.map(|c| c.0), Some(input::THROW_ITEM_IMMEDIATE));
}

#[test]
fn an_incoming_item_attack_is_answered_and_fox_reflects() {
    let mut f = standing_mario(0.0);
    f.kind = FighterKind::Fox;
    let opponents = [player_at(1500.0)];
    let mut shell = item_at(-400.0);
    shell.pos.y = 100.0;
    shell.is_allow_pickup = false;
    shell.vel_x = 60.0;
    shell.attack_live = true;
    shell.attack_size = 100.0;
    shell.attack_count = 1;
    shell.attack_pos[0] = Vec2::new(-400.0, 100.0);
    let items = [shell];
    let w = item_world(&opponents, &items);
    let mut com = vs_cpu(&f, 9);
    assert_eq!(com.proc_default(&f, &w), 1);
    assert_eq!(com.objective, Objective::CounterAttack);
    assert!(com.is_opponent_ra);
    // Its own shell is no threat.
    let mut own = shell;
    own.owner = Some(f.port);
    let items = [own];
    let w = item_world(&opponents, &items);
    let mut com = vs_cpu(&f, 9);
    com.proc_default(&f, &w);
    assert_ne!(com.objective, Objective::CounterAttack);
}

#[test]
fn a_held_bat_reaches_further_with_the_a_attacks() {
    // The forward tilt's box ends at 560, plus the target's 150: a target
    // at 800 is out of its reach until a Bat widens it by 1.3.
    let mut f = standing_mario(0.0);
    f.facing = Facing::Right;
    let opponents = [player_at(800.0)];
    let w = world(&opponents, &[]);
    let bat = ItemKind::Equipment(equipment::Kind::Bat);
    let mut tilts = [0u32; 2];
    for (n, held) in [None, Some(bat)].into_iter().enumerate() {
        if let Some(kind) = held {
            holding(&mut f, kind, ItemType::Swing, 0);
        }
        for seed in 0..200 {
            crate::rng::set_seed(seed);
            let mut com = vs_cpu(&f, 9);
            com.find_target(&f, &w);
            if com.detect_target(&f, &w, 0.0)
                && com.input_kind == Some(input::STICK_TILT_AUTO_X_BUTTON_A)
            {
                tilts[n] += 1;
            }
        }
    }
    assert_eq!(tilts[0], 0);
    assert!(tilts[1] > 0, "{tilts:?}");
}
