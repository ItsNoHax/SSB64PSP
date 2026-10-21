use super::*;
use crate::fighter::{Facing, FighterKind, Situation};
use crate::ground::Standing;
use crate::status::{self, CaptainStatus, KirbyStatus, StatusTiming};
use crate::weapon::MapSurface;
use ssb_engine::math::Vec2;

fn surface(y1: i16, y2: i16) -> MapSurface {
    crate::map::floor_surface((
        0,
        crate::collision::Segment {
            x1: -4000,
            y1,
            x2: 4000,
            y2,
            flags: 0,
        },
    ))
}

fn set(f: &mut Fighter, status: AnyStatus) {
    status::set_any_status(f, status, 0.0, StatusTiming::unknown());
}

fn airborne(kind: FighterKind, status: AnyStatus) -> Fighter {
    let mut f = Fighter::new(kind, 0, 3);
    f.pos = Vec3::new(0.0, 2000.0, 0.0);
    f.become_airborne();
    set(&mut f, status);
    f
}

fn grounded(kind: FighterKind, normal: Vec2) -> Fighter {
    let mut f = Fighter::new(kind, 0, 3);
    f.situation = Situation::Ground;
    f.floor = Some(Standing {
        line: 0,
        normal,
        flags: 0,
    });
    f
}

#[test]
fn straight_up_is_a_quarter_turn_back() {
    let up = Vec3::new(0.0, 115.0, 0.0);
    assert_eq!(
        special_hi_pitch(up, 1.0).to_bits(),
        (-f_cst_dtor32(90.0)).to_bits()
    );
    assert_eq!(
        special_hi_pitch(up, -1.0).to_bits(),
        (-f_cst_dtor32(90.0)).to_bits()
    );
    // The facing turns the travel angle, not the quarter turn.
    let v = Vec3::new(60.0, 80.0, 0.0);
    let a = sy_arc_tan2(60.0, 80.0);
    assert_eq!(
        special_hi_pitch(v, 1.0).to_bits(),
        (a - f_cst_dtor32(90.0)).to_bits()
    );
    assert_eq!(
        special_hi_pitch(v, -1.0).to_bits(),
        (-a - f_cst_dtor32(90.0)).to_bits()
    );
}

#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn fly_roll_adds_the_knockback_and_keeps_the_quarter_turn() {
    let mut f = Fighter::new(FighterKind::Mario, 0, 3);
    f.facing = Facing::Left;
    f.physics.vel_air = Vec3::new(3.0, 4.0, 0.0);
    f.physics.vel_knockback = Vec3::new(1.0, -2.0, 0.0);
    fly_roll(&mut f);
    let w = f.pose_writes.joint4.unwrap();
    assert_eq!(w.rotate_x.to_bits(), (-sy_arc_tan2(4.0, 2.0)).to_bits());
    assert_eq!(w.scale, None);
}

#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn a_status_with_a_figatree_drops_a_queued_write() {
    let mut f = airborne(FighterKind::Fox, AnyStatus::Common(Status::Fall));
    f.physics.vel_air = Vec3::new(0.0, 100.0, 0.0);
    special_hi(&mut f);
    f.pose_writes.transn_rotate_z = Some(0.25);
    set(&mut f, AnyStatus::Common(Status::Fall));
    assert_eq!(f.pose_writes, PoseWrites::NONE);
}

/// One tick of the air procs on `f`, high over a floor, and the pitch
/// `proc_physics` left.
fn air_tick(f: &mut Fighter) -> ModelPitch {
    f.pose_writes = PoseWrites::NONE;
    f.tick_physics_map(&|| [surface(0, 0)]);
    f.pose_writes.joint4.expect("a pitch from proc_physics")
}

#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn the_air_special_hi_procs_pitch_by_the_travel() {
    for (kind, status, scale) in [
        (
            FighterKind::Fox,
            AnyStatus::Fox(FoxStatus::SpecialAirHi),
            None,
        ),
        (
            FighterKind::Ness,
            AnyStatus::Ness(NessStatus::SpecialAirHiJibaku),
            None,
        ),
        (
            FighterKind::Pikachu,
            AnyStatus::Pikachu(PikachuStatus::SpecialAirHi),
            Some(QUICK_ATTACK_SCALE),
        ),
    ] {
        for facing in [Facing::Right, Facing::Left] {
            let mut f = airborne(kind, status);
            f.facing = facing;
            f.physics.vel_air = Vec3::new(70.0 * facing.sign(), 90.0, 0.0);
            let w = air_tick(&mut f);
            assert_eq!(f.status.status, status, "{kind:?}");
            let want = special_hi_pitch(f.physics.vel_air, facing.sign());
            assert_eq!(w.rotate_x.to_bits(), want.to_bits(), "{kind:?} {facing:?}");
            assert_eq!(w.scale, scale, "{kind:?}");
        }
    }
}

#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn an_air_fly_roll_pitches_before_the_knockback_decays() {
    let mut f = airborne(FighterKind::Mario, AnyStatus::Common(Status::DamageFlyRoll));
    f.facing = Facing::Right;
    f.physics.vel_air = Vec3::ZERO;
    f.physics.vel_knockback = Vec3::new(-30.0, 40.0, 0.0);
    let knockback = f.physics.vel_knockback;
    let w = air_tick(&mut f);
    let v = f.physics.vel_air;
    let want = sy_arc_tan2(v.x + knockback.x, v.y + knockback.y);
    assert_eq!(w.rotate_x.to_bits(), want.to_bits());
}

#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn a_grounded_quick_attack_pitches_by_the_transferred_speed() {
    let mut f = grounded(FighterKind::Pikachu, Vec2::new(0.0, 1.0));
    set(&mut f, AnyStatus::Pikachu(PikachuStatus::SpecialHi));
    f.physics.vel_ground.x = 80.0;
    f.tick_physics_map(&|| [surface(0, 0)]);
    let w = f.pose_writes.joint4.expect("a pitch from proc_physics");
    let want = special_hi_pitch(f.physics.vel_air, f.facing.sign());
    assert_eq!(w.rotate_x.to_bits(), want.to_bits());
    assert_eq!(w.scale, Some(QUICK_ATTACK_SCALE));
}

/// A floor rising 20 degrees to the right: `(-sin, cos)`.
fn slope_normal() -> Vec2 {
    let (s, c) = ssb_engine::math::sin_cos(ssb_engine::math::f_clc_dtor32(20.0));
    Vec2::new(-s, c)
}

#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn kirby_stone_rolls_topn_by_the_floor() {
    let n = slope_normal();
    let mut f = grounded(FighterKind::Kirby, n);
    set(&mut f, AnyStatus::Kirby(KirbyStatus::SpecialLwHold));
    assert!(crate::kirby::apply_ground_physics(&mut f));
    let roll = -sy_arc_tan2(n.x, n.y);
    assert_eq!(f.topn_rotate_z.to_bits(), roll.to_bits());
    // Rolled counterclockwise, up onto the normal.
    assert!(roll > 0.0);
    // TopN's display and collision axes take it.
    let r = crate::slope_contour::topn_rotate(&f);
    assert_eq!(r[2].to_bits(), roll.to_bits());
    let rolled = crate::item_throw::collision_axes(&f);
    f.topn_rotate_z = 0.0;
    assert_ne!(rolled, crate::item_throw::collision_axes(&f));
}

#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn set_status_clears_the_roll() {
    let mut f = grounded(FighterKind::Kirby, slope_normal());
    f.topn_rotate_z = 0.3;
    status::set_wait(&mut f);
    assert_eq!(f.topn_rotate_z.to_bits(), 0.0_f32.to_bits());
}

#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn the_falcon_kick_keeps_its_roll_into_the_air() {
    let n = slope_normal();
    let mut f = grounded(FighterKind::Captain, n);
    set(&mut f, AnyStatus::Captain(CaptainStatus::SpecialLw));
    assert!(crate::captain::apply_ground_physics(&mut f));
    let roll = -sy_arc_tan2(n.x, n.y);
    assert_eq!(f.topn_rotate_z.to_bits(), roll.to_bits());
    // Off the edge on the flag-1 pulse: `ftCaptainSpecialLwAirSetStatus`.
    f.become_airborne();
    f.motion_script.flags[1] = 2;
    assert!(crate::captain::special_lw_proc_map(&mut f));
    assert_eq!(
        f.status.status,
        AnyStatus::Captain(CaptainStatus::SpecialLwAir)
    );
    assert_eq!(f.topn_rotate_z.to_bits(), roll.to_bits());
    assert_eq!(f.root_motion.rotate_z.to_bits(), roll.to_bits());
    assert_eq!(f.pose_writes.transn_rotate_z, Some(roll));
    assert_eq!(f.pose_writes.joint4, None);
}

/// `ftCommonDamageCommonProcPhysics` (`ftcommondamage.c:228-230`) writes
/// the tumble's pitch onto joint 4 in `proc_physics`, before the same
/// `ftMainProcPhysicsMap`'s `proc_map` lands it, so the landing's
/// `ftCommonDownBounceCheckUpOrDown` reads the written pitch, not the
/// pose the frame's play left.
#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn a_tumble_lands_on_the_pitch_its_physics_wrote() {
    let mut f = Fighter::new(FighterKind::Mario, 0, 3);
    // Face up as played (-90 degrees), face down as written (+90).
    f.joint4_rotate_x = Some(-f_cst_dtor32(90.0));
    f.pose_writes.joint4 = Some(ModelPitch {
        rotate_x: f_cst_dtor32(90.0),
        scale: None,
    });
    assert!(crate::reaction::is_face_down(&f));
    f.pose_writes = PoseWrites::NONE;
    assert!(!crate::reaction::is_face_down(&f));
}
