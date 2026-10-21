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
