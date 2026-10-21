//! `ftMainProcPhysicsMap` (`ftmain.c:1739-1847`) for the statuses with
//! their own callbacks: every status runs the dead check, the altitude
//! whistle, and, when it has a `proc_map`, the mask roll, `proc_map` and
//! Kirby's map stars.

use super::*;
use crate::collision::Segment;
use crate::fteffect::FighterEffect;
use crate::map::{floor_surface, Contacts};
use crate::sound::id::nSYAudioFGMAltitudeWarn;
use crate::sound::testing::{Call, Recorder};
use crate::status::{AnyStatus, Status, StatusTiming};
use crate::weapon::{MapSurface, SurfaceMotion};

fn floor_at(y: i16) -> MapSurface {
    floor_surface((
        0,
        Segment {
            x1: -3000,
            y1: y,
            x2: 3000,
            y2: y,
            flags: 0,
        },
    ))
}

fn standing_contacts() -> Contacts {
    Contacts {
        floor: true,
        ..Contacts::default()
    }
}

fn whistled(r: &Recorder) -> bool {
    r.take()
        .iter()
        .any(|c| matches!(c, Call::PlayFgm(id) if *id == nSYAudioFGMAltitudeWarn))
}

/// `DeadUpFall` has no `proc_physics` and no `proc_map`, but
/// `ftMainProcPhysicsMap` still moves it by `vel_air` and runs the whistle
/// check: its fall to the camera's bottom crosses Peach's Castle's
/// `alt_warning` (-1900). No `proc_map`, so the masks are not rolled.
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
#[test]
fn a_falling_top_out_whistles_when_it_crosses_the_line() {
    let r = Recorder::install();
    crate::battle::start_scene(
        crate::stage_select::gkind::CASTLE,
        crate::stale::DAMAGE_RATIO_DEFAULT,
    );
    let mut f = Fighter::new(FighterKind::Mario, 0, 3);
    f.pos.y = 3001.0;
    crate::dead::set_dead_up_fall(&mut f);
    f.pos.y = -1890.0;
    f.physics.vel_air.y = -20.0;
    f.map_contacts = standing_contacts();
    r.take();
    f.tick_physics_map(&|| [floor_at(-1900)]);
    assert!(whistled(r));
    assert_eq!(f.pos.y, -1910.0, "no proc_map: the floor stops nothing");
    assert_eq!(f.map_contacts, standing_contacts());
    assert_eq!(f.map_contacts_prev, Contacts::default());
}

/// The rebirth and appear statuses have a `proc_map`, which rolls the masks
/// (`ftmain.c:1830`) and records none.
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
#[test]
fn rebirth_and_appear_roll_the_masks() {
    for status in [
        AnyStatus::Common(Status::RebirthWait),
        AnyStatus::Kirby(crate::status::KirbyStatus::AppearR),
    ] {
        let mut f = Fighter::new(FighterKind::Kirby, 0, 3);
        f.status.status = status;
        f.map_contacts = standing_contacts();
        f.tick_physics_map(&|| [floor_at(0)]);
        assert_eq!(f.map_contacts, Contacts::default(), "{status:?}");
        assert_eq!(f.map_contacts_prev, standing_contacts(), "{status:?}");
    }
}

/// A held fighter's `proc_map` (`ftCommonCaptureWaitProcMap`) rolls the
/// masks and records none.
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
#[test]
fn a_held_fighter_rolls_its_masks() {
    let (_, mut held) = held_pair(None);
    held.map_contacts = standing_contacts();
    held.tick_physics_map(&|| [floor_at(0)]);
    assert_eq!(held.status.status, AnyStatus::Common(Status::CaptureWait));
    assert_eq!(held.map_contacts, Contacts::default());
    assert_eq!(held.map_contacts_prev, standing_contacts());
}

/// A catcher on line 0 and a fighter in `CaptureWait`, grounded on that
/// line the way `ftCommonCaptureWaitProcMap` leaves it. Returns the line's
/// surface with `motion` and the held fighter.
fn held_pair(motion: Option<SurfaceMotion>) -> (MapSurface, Fighter) {
    let mut catcher = Fighter::new(FighterKind::Mario, 0, 3);
    catcher.situation = Situation::Ground;
    catcher.floor = Some(crate::ground::Standing {
        line: 0,
        flags: 0,
        normal: ssb_engine::math::Vec2::new(0.0, 1.0),
    });
    catcher.pos = Vec3::new(100.0, 0.0, 0.0);
    let mut held = Fighter::new(FighterKind::Mario, 1, 3);
    held.grab.capture = Some(0);
    held.grab.holder = Some(crate::grab::holder_of(&catcher));
    crate::status::set_status(&mut held, Status::CaptureWait, 0.0, StatusTiming::unknown());
    held.situation = Situation::Ground;
    held.floor = catcher.floor;
    let mut floor = floor_at(0);
    floor.motion = motion;
    (floor, held)
}

/// `ftmain.c:1808-1812`: a fighter grounded on a moving line rides its
/// `vel_speed` before `proc_map`, a held one included: `CaptureWait`'s
/// `proc_map` keeps TopN's x, so the held fighter is drawn and collides
/// `vel_speed` along from its catcher's hand that frame.
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
#[test]
fn a_held_fighter_rides_its_catchers_moving_floor() {
    let (still, mut a) = held_pair(None);
    a.tick_physics_map(&|| [still]);
    let speed = Vec3::new(7.0, 0.0, 0.0);
    let (moving, mut b) = held_pair(Some(SurfaceMotion {
        offset: ssb_engine::math::Vec2::new(0.0, 0.0),
        speed,
    }));
    b.tick_physics_map(&|| [moving]);
    assert_eq!(b.pos.x - a.pos.x, speed.x);
    assert_eq!(b.pos.y, a.pos.y);
}

/// In hitlag `proc_map` still runs after the mask roll
/// (`ftmain.c:1828-1842`): a damaged Kirby that Smash DIs into the floor
/// records the floor and makes a map star that frame.
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
#[test]
fn kirby_smash_di_into_the_floor_in_hitlag_makes_a_map_star() {
    let mut f = Fighter::new(FighterKind::Kirby, 0, 3);
    f.become_airborne();
    crate::status::set_status(&mut f, Status::DamageFlyN, 0.0, StatusTiming::unknown());
    f.pos = Vec3::new(0.0, 60.0, 0.0);
    f.hitlag = 5;
    f.is_smash_di = true;
    f.stick.y = -80;
    f.stick.tap_y = 0;
    f.effects.clear();
    f.tick_physics_map(&|| [floor_at(0)]);
    assert!(f.map_contacts.floor, "{:?}", f.pos);
    assert!(
        f.effects
            .iter()
            .any(|e| matches!(e, FighterEffect::KirbyStar(_))),
        "{:?}",
        f.effects
    );
}

/// `ftmain.c:1767-1801` runs for every status outside hitlag, whatever its
/// `proc_physics`: an airborne `Sleep` with damage velocity left decays it
/// by 1.7 along its angle and adds it to TopN after `vel_air`.
#[test]
fn a_status_with_no_physics_still_decays_and_adds_the_damage_velocity() {
    let mut f = Fighter::new(FighterKind::Mario, 0, 3);
    f.situation = Situation::Air;
    f.status.status = AnyStatus::Common(Status::Sleep);
    f.pos = Vec3::new(0.0, 500.0, 0.0);
    f.physics.vel_air = Vec3::new(2.0, 0.0, 0.0);
    f.physics.vel_knockback = Vec3::new(30.0, 40.0, 0.0);
    let mut expect = f.physics;
    crate::physics::update_damage_velocity(
        &mut expect,
        false,
        ssb_engine::math::Vec2::new(0.0, 1.0),
        0.0,
        0.0,
    );
    assert!(expect.vel_knockback.x < 30.0);
    f.tick_physics_map(&|| [floor_at(0)]);
    assert_eq!(f.physics.vel_knockback, expect.vel_knockback);
    assert_eq!(f.pos.x, (0.0 + 2.0) + expect.vel_knockback.x);
    assert_eq!(f.pos.y, 500.0 + expect.vel_knockback.y);
}

/// The rebirth halo's `-2` floor (`ftCommonRebirthDownSetStatus`): the
/// first ground status after it runs its `proc_physics` and the velocity
/// step on that flat, material-0 floor, then its `proc_map` finds no floor
/// and falls from the stepped TopN (`ftmain.c:1755-1801`).
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
#[test]
fn a_grounded_status_on_the_halo_floor_steps_before_it_falls() {
    let mut f = Fighter::new(FighterKind::Mario, 0, 3);
    crate::status::set_status(&mut f, Status::Wait, 0.0, StatusTiming::unknown());
    f.situation = Situation::Ground;
    f.floor = None;
    f.pos = Vec3::new(0.0, 1000.0, 0.0);
    f.physics.vel_ground.x = 10.0;
    f.tick_physics_map(&|| [floor_at(0)]);
    assert!(matches!(
        f.status.status,
        AnyStatus::Common(Status::Fall | Status::FallAerial)
    ));
    // Wait's friction on material 0, then the step.
    assert!(f.pos.x > 0.0 && f.pos.x < 10.0, "{}", f.pos.x);
    assert_eq!(f.pos.x, f.physics.vel_air.x);
    assert_eq!(f.pos.y, 1000.0);
}

/// `ftMarioSpecialLwProcPhysics` (`ftmariospeciallw.c:64-77`): the ground
/// speed transfers into `vel_air`, the B tap adds its rise, and
/// `ftMarioSpecialLwSwitchStatusAir` lifts Mario off; the same frame's
/// velocity step moves TopN by both and the aerial `proc_map` runs.
/// `mpCommonSetFighterAir` spends the ground jump.
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
#[test]
fn the_grounded_tornado_lifts_off_with_this_frames_step() {
    let mut f = Fighter::new(FighterKind::Mario, 0, 3);
    f.situation = Situation::Ground;
    f.floor = Some(crate::ground::Standing {
        line: 0,
        flags: 0,
        normal: ssb_engine::math::Vec2::new(0.0, 1.0),
    });
    crate::status::switch_mario_tornado_ground(&mut f);
    f.mario_special_lw.rise_enabled = true;
    f.physics.vel_ground.x = 10.0;
    f.physics.jumps_used = 0;
    f.prev_input.buttons = N64Buttons::default();
    f.input.buttons = N64Buttons(N64Buttons::B);
    f.tick_physics_map(&|| [floor_at(0)]);
    assert_eq!(
        f.status.status,
        AnyStatus::Mario(crate::status::MarioStatus::SpecialAirLw)
    );
    assert_eq!(f.situation, Situation::Air);
    assert_eq!(f.physics.jumps_used, 1);
    assert_eq!(f.physics.vel_air.y, crate::status::MARIO_TORNADO_VEL_Y_TAP);
    assert!(f.physics.vel_air.x > 0.0);
    assert_eq!(f.pos.x, f.physics.vel_air.x);
    assert_eq!(f.pos.y, crate::status::MARIO_TORNADO_VEL_Y_TAP);
}

/// `cliffcatch_wait` counts down in `ftMainProcPhysicsMap`, outside hitlag
/// and ahead of `proc_physics` (`ftmain.c:1755-1760`), not in the
/// interrupt pass: a wait set by a status's `proc_interrupt` (letting go,
/// `ftcommoncliffclimb.c:102`) is already 29 when that frame's `proc_map`
/// reads it (`mpcommon.c:549`).
#[test]
fn the_cliff_catch_wait_counts_in_the_physics_pass_outside_hitlag() {
    let surfaces = || [floor_at(0)];
    let mut f = Fighter::new(FighterKind::Mario, 0, 3);
    crate::status::set_status(&mut f, Status::Fall, 0.0, StatusTiming::unknown());
    f.situation = Situation::Air;
    f.pos = Vec3::new(0.0, 1000.0, 0.0);
    f.cliffcatch_wait = crate::status::CLIFF_CATCH_WAIT;
    f.tick_interrupt(&surfaces);
    assert_eq!(f.cliffcatch_wait, crate::status::CLIFF_CATCH_WAIT);
    f.tick_physics_map(&surfaces);
    assert_eq!(f.cliffcatch_wait, crate::status::CLIFF_CATCH_WAIT - 1);
    f.hitlag = 2;
    f.tick_physics_map(&surfaces);
    assert_eq!(f.cliffcatch_wait, crate::status::CLIFF_CATCH_WAIT - 1);
}

fn grounded_wait(kind: FighterKind) -> Fighter {
    let mut f = Fighter::new(kind, 0, 3);
    crate::status::set_status(&mut f, Status::Wait, 0.0, StatusTiming::unknown());
    f.situation = Situation::Ground;
    f.floor = Some(crate::ground::Standing {
        line: 0,
        flags: 0,
        normal: ssb_engine::math::Vec2::new(0.0, 1.0),
    });
    f.pos = Vec3::ZERO;
    f
}

/// In hitlag `ftMainProcPhysicsMap` skips `proc_physics` but still runs
/// the status's own `proc_map` (`ftmain.c:1829`): `Wait`'s
/// `mpCommonSetFighterFallOnGroundBreak` reaches `mpProcessUpdateMain`,
/// which adds the stage's `vel_push` (`mpprocess.c:450`).
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
#[test]
fn a_grounded_status_in_hitlag_is_pushed_by_the_stage() {
    let mut f = grounded_wait(FighterKind::Mario);
    f.hitlag = 3;
    f.hazard.vel_push = Vec3::new(5.0, 0.0, 0.0);
    f.tick_physics_map(&|| [floor_at(0)]);
    assert_eq!(f.pos.x, 5.0);
    assert_eq!(f.situation, Situation::Ground);
    assert_eq!(f.status.status, AnyStatus::Common(Status::Wait));
}

/// The same `proc_map` in hitlag on no floor (the rebirth halo's) finds
/// none and falls (`mpCommonSetFighterFallOnGroundBreak`).
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
#[test]
fn a_grounded_status_in_hitlag_on_no_floor_falls() {
    let mut f = grounded_wait(FighterKind::Mario);
    f.floor = None;
    f.pos = Vec3::new(0.0, 1000.0, 0.0);
    f.hitlag = 3;
    f.tick_physics_map(&|| [floor_at(0)]);
    assert!(matches!(
        f.status.status,
        AnyStatus::Common(Status::Fall | Status::FallAerial)
    ));
    assert_eq!(f.situation, Situation::Air);
    assert_eq!(f.pos.y, 1000.0);
}

/// `ftFoxSpecialAirHiProcMap` counts `pass_timer` first
/// (`ftfoxspecialhi.c:223`), in hitlag too: a Fire Fox frozen by its own
/// hit still nears the frame it may land on platforms.
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
#[test]
fn fire_fox_counts_its_pass_timer_in_hitlag() {
    let mut f = Fighter::new(FighterKind::Fox, 0, 3);
    f.become_airborne();
    crate::status::set_any_status(
        &mut f,
        AnyStatus::Fox(crate::status::FoxStatus::SpecialAirHi),
        0.0,
        StatusTiming::unknown(),
    );
    f.pos = Vec3::new(0.0, 500.0, 0.0);
    f.fox_special_hi.pass_timer = 4;
    f.hitlag = 3;
    f.tick_physics_map(&|| [floor_at(0)]);
    assert_eq!(f.fox_special_hi.pass_timer, 5);
    assert_eq!(f.pos, Vec3::new(0.0, 500.0, 0.0));
}

/// `ftPikachuSpecialAirHiProcMap` counts `pass_timer` too
/// (`ftpikachuspecialhi.c:216`): in `proc_map`, so in hitlag as well.
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
#[test]
fn quick_attack_counts_its_pass_timer_in_proc_map() {
    let mut f = Fighter::new(FighterKind::Pikachu, 0, 3);
    f.become_airborne();
    crate::status::set_any_status(
        &mut f,
        AnyStatus::Pikachu(crate::status::PikachuStatus::SpecialAirHi),
        0.0,
        StatusTiming::unknown(),
    );
    f.pos = Vec3::new(0.0, 500.0, 0.0);
    f.pikachu.pass_timer = 1;
    f.hitlag = 3;
    f.tick_physics_map(&|| [floor_at(0)]);
    assert_eq!(f.pikachu.pass_timer, 2);
}

/// `ftCommonCapturePulledProcMap` in hitlag (`ftmain.c:1829`): no
/// `proc_physics` places the fighter at the hand, but the `proc_map` still
/// halves its height above the catcher's floor and keeps it airborne
/// (`ftcommoncapturepulled.c:83`).
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
#[test]
fn a_pulled_fighter_in_hitlag_still_drops_toward_its_catchers_floor() {
    let (floor, mut held) = held_pair(None);
    crate::status::set_status(
        &mut held,
        Status::CapturePulled,
        0.0,
        StatusTiming::unknown(),
    );
    held.situation = Situation::Air;
    held.floor = None;
    held.pos = Vec3::new(100.0, 40.0, 0.0);
    held.hitlag = 3;
    held.tick_physics_map(&|| [floor]);
    assert_eq!(held.pos, Vec3::new(100.0, 20.0, 0.0));
    assert_eq!(held.situation, Situation::Air);
    assert_eq!(held.physics.jumps_used, 1);
}

/// `ftCommonCaptureWaitProcMap` in hitlag, past the end of the catcher's
/// line: the floor probe fails, so the fighter takes the line's edge
/// height and becomes airborne (`ftcommoncapturewait.c:36-41`).
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
#[test]
fn a_held_fighter_in_hitlag_off_its_catchers_line_takes_the_edge() {
    let (floor, mut held) = held_pair(None);
    held.pos = Vec3::new(3100.0, 0.0, 0.0);
    held.hitlag = 3;
    held.tick_physics_map(&|| [floor]);
    assert_eq!(held.status.status, AnyStatus::Common(Status::CaptureWait));
    assert_eq!(held.situation, Situation::Air);
    assert_eq!(held.floor, None);
    assert_eq!(held.physics.jumps_used, 1);
}

/// A fighter in Kirby's mouth on `holder`, 500 units from Kirby.
fn in_kirbys_mouth(holder: Vec3, facing: Facing) -> Fighter {
    let mut kirby = Fighter::new(FighterKind::Kirby, 0, 3);
    kirby.pos = holder;
    kirby.facing = facing;
    let mut f = Fighter::new(FighterKind::Mario, 1, 3);
    f.become_airborne();
    f.grab.capture = Some(0);
    f.grab.holder = Some(crate::grab::holder_of(&kirby));
    crate::status::set_status(
        &mut f,
        Status::CaptureWaitKirby,
        0.0,
        StatusTiming::unknown(),
    );
    f.pos = holder + Vec3::new(500.0, 300.0, 0.0);
    f
}

/// `ftCommonCaptureWaitKirbyProcMap` places the fighter at Kirby's side;
/// as a `proc_map` it runs in hitlag too.
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
#[test]
fn a_fighter_in_kirbys_mouth_follows_kirby_in_hitlag() {
    let mut f = in_kirbys_mouth(Vec3::new(100.0, 0.0, 0.0), Facing::Right);
    f.hitlag = 3;
    f.tick_physics_map(&|| [floor_at(0)]);
    assert_eq!(f.pos, Vec3::new(110.0, 0.0, 0.0));
}

/// The same placement runs after `ftCommonDeadCheckInterruptCommon`:
/// the check sees where the fighter stood, so Kirby's side past the blast
/// line is not a KO that frame.
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
#[test]
fn kirbys_mouth_places_the_fighter_after_the_dead_check() {
    let mut f = in_kirbys_mouth(Vec3::new(-5995.0, 0.0, 0.0), Facing::Left);
    f.dead.bounds = Some(crate::dead::StageBounds {
        map: crate::status::BlastZone {
            top: 3000.0,
            bottom: -2000.0,
            left: -6000.0,
            right: 6000.0,
        },
        camera: crate::status::BlastZone {
            top: 2000.0,
            bottom: -1000.0,
            left: -5000.0,
            right: 5000.0,
        },
        rebirth: ssb_engine::math::Vec2::new(0.0, 1200.0),
        fog_color: [0; 3],
    });
    f.tick_physics_map(&|| [floor_at(0)]);
    assert_eq!(f.status.status, AnyStatus::Common(Status::CaptureWaitKirby));
    assert_eq!(f.pos, Vec3::new(-6005.0, 0.0, 0.0));
}

/// `ftCommonDokanWaitProcMap` puts TopN on the trip's cubic from its
/// counter, in hitlag too.
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
#[test]
fn a_fighter_in_a_pipe_in_hitlag_stays_on_the_trip() {
    let mut f = Fighter::new(FighterKind::Mario, 0, 3);
    f.become_airborne();
    crate::status::set_status(&mut f, Status::DokanWait, 0.0, StatusTiming::unknown());
    f.dokan.pos_curr = Vec3::new(-1250.0, 0.0, 0.0);
    f.dokan.target_pos = Vec3::new(1250.0, 0.0, 0.0);
    f.dokan.pos_adjust_wait = 10;
    f.pos = Vec3::new(0.0, 500.0, 0.0);
    f.hitlag = 3;
    f.tick_physics_map(&|| [floor_at(-400)]);
    let x = crate::dokan::interp_cubic(10.0, -1250.0, 1250.0);
    assert_eq!(f.pos, Vec3::new(x, 0.0, 0.0));
}

/// `mpCommonProcFighterProject`, the captors' `proc_map`, goes through
/// `mpProcessUpdateMain`, which adds the stage's `vel_push`
/// (`mpprocess.c:450`), in hitlag too.
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
#[test]
fn a_fighter_in_a_barrel_in_hitlag_runs_the_project_map() {
    let mut f = Fighter::new(FighterKind::Mario, 0, 3);
    f.become_airborne();
    crate::status::set_status(&mut f, Status::TaruCann, 0.0, StatusTiming::unknown());
    f.pos = Vec3::new(0.0, 500.0, 0.0);
    f.hazard.vel_push = Vec3::new(5.0, 0.0, 0.0);
    f.hitlag = 3;
    f.tick_physics_map(&|| [floor_at(0)]);
    assert_eq!(f.pos, Vec3::new(5.0, 500.0, 0.0));
}
