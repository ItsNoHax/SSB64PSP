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
    // Entry holds the fighter where it is.
    assert!(tick_status(&mut f));

    appear_set_status(&mut f);
    assert_eq!(f.status.status, AnyStatus::Mario(MarioStatus::AppearR));
    assert_eq!(slot(f.status.status), Some(SLOT_APPEAR_R));
    assert!(!f.is_invisible, "the entry shows the fighter");
    assert!(f.is_shadow_hidden && f.dead.is_ghost);
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
    run(&mut f, 50);
    assert_eq!(f.status.status, AnyStatus::Ness(NessStatus::AppearREnd));
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
