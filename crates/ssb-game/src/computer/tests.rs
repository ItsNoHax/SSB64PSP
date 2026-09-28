use super::*;
use crate::ground::Standing;
use ssb_engine::math::Vec3;

fn mario_on_floor() -> Fighter {
    let mut f = Fighter::new(FighterKind::Mario, 1, 3);
    f.situation = Situation::Ground;
    f.floor = Some(Standing {
        line: 3,
        flags: 0,
        normal: Vec2::new(0.0, 1.0),
    });
    f
}

/// Runs the interpreter until the script ends, returning the controller after
/// every tick.
fn run(com: &mut Computer, f: &Fighter) -> std::vec::Vec<ControllerState> {
    let mut out = std::vec::Vec::new();
    for _ in 0..200 {
        com.update_inputs(f, &Senses::default());
        out.push(com.controller());
        if com.command.is_none() {
            break;
        }
    }
    out
}

#[test]
fn scripts_decode_as_the_table_the_decomp_builds() {
    assert_eq!(INPUT_SCRIPTS.len(), 49);
    // `nFTComputerInputStickN`: both axes to neutral, then the end.
    assert_eq!(
        INPUT_SCRIPTS[input::STICK_N],
        &[0xA0, 0x00, 0xB0, 0x00, 0xFF]
    );
    // Every script ends.
    for s in INPUT_SCRIPTS.iter() {
        assert_eq!(s.last(), Some(&0xFF));
    }
}

#[test]
fn a_jab_script_presses_a_for_its_wait_and_releases_it() {
    let f = mario_on_floor();
    let mut com = Computer::setup(&f, 9);
    com.stick = (40, 40);
    com.set_command_immediate(input::STICK_N_BUTTON_A);
    let ticks = run(&mut com, &f);
    // The first tick zeroes the stick and presses A.
    assert_eq!((ticks[0].stick_x, ticks[0].stick_y), (0, 0));
    assert!(ticks[0].buttons.contains(N64Buttons::A));
    // A is released before the script ends.
    assert!(!ticks.last().unwrap().buttons.contains(N64Buttons::A));
    assert!(com.command.is_none());
}

#[test]
fn stick_auto_points_at_the_target() {
    let f = mario_on_floor();
    let mut com = Computer::setup(&f, 9);
    com.target_pos = Vec2::new(-500.0, 0.0);
    com.set_command_immediate(input::STICK_TILT_AUTO_X);
    run(&mut com, &f);
    // `FTCOMPUTER_STICK_AUTOFULL` toward a target on the left.
    assert_eq!(com.stick.0, -STICK_MAX);
}

#[test]
fn move_auto_scales_the_grounded_stick_by_the_dash_reach() {
    let mut f = mario_on_floor();
    f.attributes = crate::physics::PhysicsAttributes::MARIO;
    let mut com = Computer::setup(&f, 9);
    assert!(com.dash_predict > 0.0);
    // Far away: full stick along the floor; the target's line matches the
    // floor's, so no vertical input.
    com.target_pos = Vec2::new(com.dash_predict * 2.0, 0.0);
    com.target_line = Some(3);
    com.set_command_immediate(input::MOVE_AUTO);
    run(&mut com, &f);
    assert_eq!(com.stick, (STICK_MAX, 0));
    // A low level only half-tilts.
    let mut low = Computer::setup(&f, 3);
    low.target_pos = Vec2::new(-1000.0, 0.0);
    low.target_line = Some(3);
    low.set_command_immediate(input::MOVE_AUTO);
    run(&mut low, &f);
    assert_eq!(low.stick, (-STICK_MAX / 2, 0));
}

#[test]
fn move_auto_in_the_air_never_points_down_at_a_lower_target() {
    let mut f = Fighter::new(FighterKind::Mario, 1, 3);
    f.situation = Situation::Air;
    f.pos = Vec3::new(0.0, 1000.0, 0.0);
    let mut com = Computer::setup(&f, 9);
    com.target_pos = Vec2::new(500.0, 0.0);
    com.set_command_immediate(input::MOVE_AUTO);
    run(&mut com, &f);
    assert_eq!(com.stick, (STICK_MAX, 0));
}

#[test]
fn the_wait_before_a_command_shrinks_with_the_level() {
    let f = mario_on_floor();
    let mut top = Computer::setup(&f, LEVEL_MAX);
    top.set_command_wait_short(true, input::STICK_N);
    assert_eq!(top.input_wait, 1);
    let mut low = Computer::setup(&f, 1);
    low.set_command_wait_long(true, input::STICK_N);
    // 4 × 8 + 1 at the least.
    assert!(low.input_wait >= 33);
}

#[test]
fn a_full_jump_prediction_reaches_above_the_floor() {
    let mut f = mario_on_floor();
    f.attributes = crate::physics::PhysicsAttributes::MARIO;
    let com = Computer::setup(&f, 5);
    assert!(com.jump_predict > 0.0);
    assert_eq!(com.origin_pos, Vec2::new(0.0, 0.0));
    assert_eq!(com.floor_line, Some(3));
}
