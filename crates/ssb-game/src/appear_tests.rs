use super::*;
use crate::fighter::Situation;
use crate::ground::Standing;
use ssb_engine::math::Vec2;

fn standing(kind: FighterKind, facing: Facing) -> Fighter {
    let mut f = Fighter::new(kind, 0, 2);
    f.pos = Vec3::new(-800.0, 0.0, 0.0);
    f.facing = facing;
    f.situation = Situation::Ground;
    f.floor = Some(Standing {
        line: 3,
        flags: 0,
        normal: Vec2::new(0.0, 1.0),
    });
    f
}

fn run(f: &mut Fighter, frames: u32) {
    for _ in 0..frames {
        status::update(f);
    }
}

#[test]
fn a_vs_fighter_waits_hidden_then_enters_facing_the_camera() {
    let mut f = standing(FighterKind::Mario, Facing::Right);
    entry_set_status(&mut f);
    assert_eq!(f.status.status, AnyStatus::Common(Status::Entry));
    assert!(f.is_invisible && f.is_shadow_hidden && f.dead.is_ghost);
    assert!(f.interface.tag_hide);
    // Entry holds the fighter where it is.
    assert!(tick_status(&mut f));

    appear_set_status(&mut f);
    assert_eq!(f.status.status, AnyStatus::Mario(MarioStatus::AppearR));
    assert_eq!(slot(f.status.status), Some(SLOT_APPEAR_R));
    assert!(!f.is_invisible, "the entry shows the fighter");
    assert!(f.is_shadow_hidden && f.dead.is_ghost);
    assert!(f.interface.tag_hide);
    assert_eq!(f.situation, Situation::Air);
    assert_eq!(model_yaw(&f), Some(0.0));
    assert_eq!(f.entry.floor_line, Some(3));

    // TransN places it: the spawn point plus the clip's translation.
    f.root_motion.translate = Vec3::new(10.0, -300.0, 5.0);
    assert!(tick_status(&mut f));
    assert_eq!(f.pos, Vec3::new(-790.0, -300.0, 5.0));

    // 120 frames on, it stands at its spawn facing right.
    run(&mut f, 120);
    assert_eq!(f.status.status, AnyStatus::Common(Status::Wait));
    assert_eq!(f.pos, Vec3::new(-800.0, 0.0, 0.0));
    assert_eq!(f.facing, Facing::Right);
    assert_eq!(f.situation, Situation::Ground);
    assert_eq!(model_yaw(&f), None);
    assert!(!f.interface.tag_hide);
}

#[test]
fn a_leftward_captain_turns_around_and_has_two_phases() {
    let mut f = standing(FighterKind::Captain, Facing::Left);
    appear_set_status(&mut f);
    assert_eq!(
        f.status.status,
        AnyStatus::Captain(CaptainStatus::AppearLStart)
    );
    assert!(f.entry.is_rotate);
    assert_eq!(model_yaw(&f), Some(core::f32::consts::PI));
    f.root_motion.translate = Vec3::new(100.0, 0.0, 50.0);
    tick_status(&mut f);
    assert_eq!(f.pos, Vec3::new(-900.0, 0.0, -50.0));
    run(&mut f, 90);
    assert_eq!(
        f.status.status,
        AnyStatus::Captain(CaptainStatus::AppearLEnd)
    );
    assert!(!f.is_shadow_hidden);
    assert!(f.interface.tag_hide);
    run(&mut f, 30);
    assert_eq!(f.status.status, AnyStatus::Common(Status::Wait));
    assert_eq!(f.facing, Facing::Left);
}

#[test]
fn ness_enters_in_three_phases() {
    let mut f = standing(FighterKind::Ness, Facing::Right);
    appear_set_status(&mut f);
    assert_eq!(f.status.status, AnyStatus::Ness(NessStatus::AppearRStart));
    run(&mut f, 40);
    assert_eq!(f.status.status, AnyStatus::Ness(NessStatus::AppearWait));
    assert!(f.interface.tag_hide);
    run(&mut f, 50);
    assert_eq!(f.status.status, AnyStatus::Ness(NessStatus::AppearREnd));
    assert!(f.interface.tag_hide);
    run(&mut f, 30);
    assert_eq!(f.status.status, AnyStatus::Common(Status::Wait));
}

#[test]
fn every_playable_fighter_has_an_entry() {
    use FighterKind::*;
    for kind in [
        Mario, Fox, Donkey, Samus, Luigi, Link, Yoshi, Captain, Kirby, Pikachu, Purin, Ness,
    ] {
        for facing in [Facing::Right, Facing::Left] {
            let mut f = standing(kind, facing);
            appear_set_status(&mut f);
            assert!(slot(f.status.status).is_some(), "{kind:?} {facing:?}");
            assert_eq!(f.status.status.anim_slot(), slot(f.status.status).unwrap());
        }
    }
}

#[test]
fn the_entry_focus_staggers_two_fighters() {
    let f = |id| EntryFocus { id, count: 2 };
    assert_eq!((f(0).appear_tick(0), f(0).appear_tick(1)), (22, 44));
    assert_eq!((f(1).appear_tick(0), f(1).appear_tick(1)), (105, 120));
    assert_eq!((f(2).appear_tick(0), f(2).appear_tick(1)), (60, 120));
    // Only id 2 zooms: on the first fighter 90..150, the second 150..210.
    assert_eq!(f(0).zoom(100), None);
    assert_eq!(f(2).zoom(89), None);
    assert_eq!(f(2).zoom(90), Some(0));
    assert_eq!(f(2).zoom(149), Some(0));
    assert_eq!(f(2).zoom(150), Some(1));
    assert_eq!(f(2).zoom(209), Some(1));
    assert_eq!(f(2).zoom(210), None);
}

#[test]
fn the_entry_effect_clock_starts_with_the_entry() {
    let mut f = standing(FighterKind::Mario, Facing::Right);
    assert_eq!(f.entry.effect_ticks, None);
    appear_set_status(&mut f);
    assert_eq!(f.entry.effect_ticks, Some(0));
    tick_effect_clock(&mut f);
    tick_effect_clock(&mut f);
    assert_eq!(f.entry.effect_ticks, Some(2));
    let mut n = standing(FighterKind::Ness, Facing::Right);
    appear_set_status(&mut n);
    assert_eq!(n.entry.effect_ticks, None, "Ness makes no effect");
}

#[test]
fn the_entry_camera_mode_lasts_through_the_statuses_until_go() {
    let mut f = standing(FighterKind::Mario, Facing::Right);
    entry_set_status(&mut f);
    assert_eq!(f.dead.camera_mode, crate::dead::CameraMode::Default);
    appear_set_status(&mut f);
    assert_eq!(f.dead.camera_mode, crate::dead::CameraMode::Entry);
    // `ftMainSetStatus` keeps it: the fighter stands in Wait still framed
    // at its entry.
    run(&mut f, 130);
    assert_eq!(f.status.status, AnyStatus::Common(Status::Wait));
    assert_eq!(f.dead.camera_mode, crate::dead::CameraMode::Entry);
    on_go(&mut f);
    assert_eq!(f.dead.camera_mode, crate::dead::CameraMode::Default);
}

/// The Poké Ball opens on Appear frame 40: the motion script sets flag 1
/// and `ftCommonAppearUpdateEffects` makes the rays (RE-425).
#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn the_poke_balls_rays_start_on_the_scripts_flag() {
    for kind in [FighterKind::Pikachu, FighterKind::Purin] {
        let mut f = standing(kind, Facing::Right);
        appear_set_status(&mut f);
        let mut started = None;
        for frame in 1..=60 {
            status::update(&mut f);
            if f.entry.rays_ticks.is_some() && started.is_none() {
                started = Some(frame);
            }
            tick_effect_clock(&mut f);
        }
        assert_eq!(started, Some(40), "{kind:?}");
        assert_eq!(f.entry.rays_ticks, Some(21));
        assert_eq!(f.motion_script.flags[1], 0);
    }
    let mut m = standing(FighterKind::Mario, Facing::Right);
    appear_set_status(&mut m);
    run(&mut m, 60);
    assert_eq!(m.entry.rays_ticks, None);
}

/// Captain Falcon's leftward entry draws him on DL link 1 until his TopN
/// is nearer than z -1000; a rightward one never moves him (RE-425).
#[test]
fn falcon_draws_before_the_stage_while_far_on_a_leftward_entry() {
    let mut f = standing(FighterKind::Captain, Facing::Left);
    appear_set_status(&mut f);
    assert!(f.entry.is_link_1);
    f.pos.z = -5000.0;
    status::update(&mut f);
    assert!(f.entry.is_link_1);
    f.pos.z = -900.0;
    status::update(&mut f);
    assert!(!f.entry.is_link_1);
    let mut r = standing(FighterKind::Captain, Facing::Right);
    appear_set_status(&mut r);
    assert!(!r.entry.is_link_1);
}

#[test]
fn a_team_member_drops_in_from_between_the_camera_and_blast_tops() {
    let mut f = standing(FighterKind::Yoshi, Facing::Left);
    entry_set_status(&mut f);
    let start = f.pos;
    appear_set_position(&mut f, 3500.0, 7500.0);
    assert_eq!(f.status.status, AnyStatus::Common(Status::Fall));
    assert_eq!(f.pos.y, 5500.0);
    assert_eq!(f.entry.pos, start);
    assert_eq!(f.dead.camera_mode, crate::dead::CameraMode::Entry);
    assert!(!f.is_invisible && !f.dead.is_ghost);
}

#[test]
fn a_team_member_drops_in_from_halfway_up_framed_at_its_start() {
    let mut f = standing(FighterKind::Yoshi, Facing::Left);
    entry_set_status(&mut f);
    appear_set_position(&mut f, 3000.0, 5000.0);
    assert_eq!(f.status.status, AnyStatus::Common(Status::Fall));
    assert_eq!(f.pos.y, 4000.0);
    assert_eq!(f.entry.pos, Vec3::new(-800.0, 0.0, 0.0));
    assert_eq!(f.dead.camera_mode, crate::dead::CameraMode::Entry);
    assert!(!f.is_invisible && !f.dead.is_ghost);
    on_go(&mut f);
    assert_eq!(f.dead.camera_mode, crate::dead::CameraMode::Default);
}
