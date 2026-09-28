use super::*;
use crate::collision::Segment;
use crate::ground::Standing;
use crate::weapon::MapSurface;

/// One floor from -2000 to 2000 at y 0, walled below its ends.
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

fn world(opponents: &[Opponent]) -> World<'_, impl Fn() -> [MapSurface; 1]> {
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
        twister: None,
        acid: None,
    }
}

fn standing_mario(x: f32) -> Fighter {
    let mut f = Fighter::new(FighterKind::Mario, 1, 3);
    f.attributes = crate::physics::PhysicsAttributes::MARIO;
    f.pos = Vec3::new(x, 0.0, 0.0);
    f.situation = Situation::Ground;
    f.floor = Some(Standing {
        line: 0,
        flags: 0,
        normal: Vec2::new(0.0, 1.0),
    });
    f
}

fn cpu(f: &Fighter, behavior: Behavior) -> Computer {
    let mut com = Computer::setup(f, 3);
    com.behavior = behavior;
    com.setup_cliffs(&world(&[]));
    com
}

fn player_at(x: f32) -> Opponent {
    Opponent {
        pos: Vec3::new(x, 0.0, 0.0),
        vel_air: Vec3::ZERO,
        status: AnyStatus::Common(Status::Wait),
        grounded: true,
        floor_line: Some(0),
        facing: Facing::Right,
        damage: 0,
        star_invincible: false,
        has_hammer: false,
    }
}

#[test]
fn the_outer_edges_are_the_recovery_aims() {
    let f = standing_mario(0.0);
    let com = cpu(&f, Behavior::Stand);
    assert_eq!(com.cliff_left, Vec2::new(-2000.0, 0.0));
    assert_eq!(com.cliff_right, Vec2::new(2000.0, 0.0));
}

#[test]
fn a_standing_cpu_keeps_the_stick_neutral() {
    let f = standing_mario(500.0);
    let mut com = cpu(&f, Behavior::Stand);
    com.stick = (60, 60);
    let w = world(&[]);
    for _ in 0..5 {
        com.process(&f, &w);
    }
    assert_eq!(com.objective, Objective::Stand);
    assert_eq!(com.stick, (0, 0));
}

#[test]
fn off_the_side_a_cpu_recovers_back_toward_the_stage() {
    let mut f = standing_mario(3000.0);
    f.situation = Situation::Air;
    f.floor = None;
    f.pos.y = -500.0;
    f.physics.vel_air.y = -10.0;
    let mut com = cpu(&f, Behavior::Stand);
    let w = world(&[]);
    com.process(&f, &w);
    assert_eq!(com.objective, Objective::Recover);
    // Past the right side: aim 1,100 units back in.
    assert_eq!(com.target_pos.x, 3000.0 - 1100.0);
    // `MoveAuto` in the air points full left.
    for _ in 0..3 {
        com.process(&f, &w);
    }
    assert!(com.stick.0 < 0, "{:?}", com.stick);
}

#[test]
fn a_walking_cpu_paces_inside_its_floor() {
    crate::rng::set_seed(7);
    let f = standing_mario(0.0);
    let mut com = cpu(&f, Behavior::Walk);
    let w = world(&[]);
    com.process(&f, &w);
    assert!(com.target_pos.x >= -2000.0 && com.target_pos.x <= 2000.0);
    if com.objective == Objective::Walk {
        for _ in 0..3 {
            com.process(&f, &w);
        }
        assert_eq!(
            com.stick.0.signum(),
            (com.target_pos.x - f.pos.x).signum() as i8
        );
    }
}

#[test]
fn a_jumping_cpu_tilts_the_stick_up_to_jump() {
    let f = standing_mario(0.0);
    let mut com = cpu(&f, Behavior::Jump);
    let w = world(&[]);
    com.process(&f, &w);
    assert_eq!(com.target_pos.y, 1100.0);
    let mut up = false;
    for _ in 0..60 {
        com.process(&f, &w);
        up |= com.stick.1 > 0;
    }
    assert!(up, "the stick never tilted up");
}

#[test]
fn an_evading_cpu_keeps_away_from_the_player() {
    let f = standing_mario(0.0);
    let mut com = cpu(&f, Behavior::Evade);
    let opponents = [player_at(-1000.0)];
    let w = world(&opponents);
    com.process(&f, &w);
    // The player is on the left, so run to 2,500 units past them on the
    // right, inside the floor.
    assert_eq!(com.target_pos.x, -1000.0 + 2500.0);
    assert_eq!(com.objective, Objective::Evade);
    // Past the floor's end the target flips to the player's other side, and
    // a jump command is queued instead.
    let mut com = cpu(&f, Behavior::Evade);
    let opponents = [player_at(-300.0)];
    let w = world(&opponents);
    com.process(&f, &w);
    assert_eq!(com.target_pos.x, -300.0 - 2500.0);
    assert_eq!(
        com.command.map(|c| c.0),
        Some(input::MOVE_AUTO_STICK_TILT_HI_RELEASE_Z)
    );
}

#[test]
fn the_nearest_player_on_stage_is_the_target() {
    let f = standing_mario(0.0);
    let mut com = cpu(&f, Behavior::Stand);
    let mut off = player_at(5000.0);
    off.pos.y = -2000.0;
    let opponents = [off, player_at(-800.0), player_at(1500.0)];
    let w = world(&opponents);
    assert!(com.find_target(&f, &w));
    assert_eq!(com.target_user, Some(1));
    assert_eq!(com.target_dist, 800.0);
    assert_eq!(com.target_line, Some(0));
}
