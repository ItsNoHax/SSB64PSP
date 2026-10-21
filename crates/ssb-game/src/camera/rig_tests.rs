//! The camera process's status precedence (`gGMCameraStruct.status`):
//! movie, map zoom, animation, bonus follow, entry focus, demo focus,
//! Close-Up, then the stage's battle camera.

use super::*;
use crate::camera::MovieView;
use crate::fighter::FighterKind;

fn bounds(half: f32) -> Bounds {
    Bounds {
        top: half,
        bottom: -half,
        left: -half,
        right: half,
    }
}

fn stage(follow: BonusFollow) -> CameraStage {
    CameraStage {
        camera: bounds(3000.0),
        team_camera: bounds(1500.0),
        map_top: 6000.0,
        light_angle_z: 0.1,
        stage_camera: StageCamera::Zebes,
        follow,
    }
}

const POS: Vec3 = Vec3::new(400.0, 200.0, 0.0);

fn interest(x: f32) -> Interest {
    Interest {
        target_pos: Vec3::new(x, 100.0, 0.0),
        facing_left: false,
        zoom_frame: 1.0,
        zoom_range: 1.0,
        idle_zoomed_out: false,
        dead_up: false,
        team_bounds: None,
    }
}

/// The battle camera on `interests`, as the stage's default status runs it.
fn battle(mut camera: Camera, interests: &[Interest], s: &CameraStage) -> Camera {
    camera.stage_camera = s.stage_camera;
    let (w, h) = camera.viewport_size();
    camera.tick_interests(interests, s.camera, s.light_angle_z, w / h);
    camera
}

fn tick(rig: &mut CameraRig, entry: Option<EntryFocusTarget>, s: &CameraStage) {
    rig.tick(POS, 50.0, &[interest(0.0), interest(500.0)], entry, s);
}

#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn a_movie_camera_is_left_to_its_scene() {
    let mut rig = CameraRig {
        camera: Camera {
            movie: Some(MovieView {
                viewport: [0.0; 4],
                aspect: 1.0,
                near: 1.0,
                far: 2.0,
                up_x: 0.0,
            }),
            ..Camera::default()
        },
        player_zoom: Some(1000.0),
        ..CameraRig::default()
    };
    let before = rig;
    tick(&mut rig, None, &stage(BonusFollow::Other));
    assert_eq!(rig, before);
}

#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn map_zoom_runs_before_every_other_status() {
    let (origin, target) = (Vec3::new(0.0, 500.0, 0.0), Vec3::new(0.0, 500.0, 4000.0));
    let mut rig = CameraRig {
        bonus_follow: true,
        player_zoom: Some(1000.0),
        ..CameraRig::default()
    };
    rig.apply_boss(CameraRequest::MapZoom {
        at: origin,
        eye: target,
    });
    assert_eq!(rig.status, Some(CameraStatus::MapZoom { origin, target }));
    let mut expect = Camera::default();
    expect.begin_map_zoom(origin, target);
    expect.tick_map_zoom(origin, target);
    tick(&mut rig, None, &stage(BonusFollow::Other));
    assert_eq!(rig.camera, expect);
    rig.apply_boss(CameraRequest::Default);
    assert_eq!(rig.status, None);
}

/// `gmCameraAnimFuncCamera`: a playing animation holds the camera; an
/// ended one hands it to the battle camera in the same tick.
#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn an_ended_animation_falls_through_to_the_battle_camera() {
    let s = stage(BonusFollow::Other);
    let anim = |ended| CameraStatus::Anim {
        frame: 3,
        vel: Vec3::ZERO,
        ended,
    };
    let mut rig = CameraRig {
        status: Some(anim(false)),
        ..CameraRig::default()
    };
    let before = rig;
    tick(&mut rig, None, &s);
    assert_eq!(rig, before);
    rig.status = Some(anim(true));
    tick(&mut rig, None, &s);
    assert_eq!(rig.status, None);
    assert_eq!(
        rig.camera,
        battle(Camera::default(), &[interest(0.0), interest(500.0)], &s)
    );
}

#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn bonus_follow_pitches_and_distances_by_stage() {
    for (follow, pitch, dist) in [
        (BonusFollow::Race, -15.0f32, 7000.0),
        (BonusFollow::Platforms, -15.0, 9000.0),
        (BonusFollow::Other, -9.0, 9000.0),
    ] {
        let s = stage(follow);
        let mut rig = CameraRig {
            bonus_follow: true,
            player_zoom: Some(1000.0),
            ..CameraRig::default()
        };
        let mut expect = Camera::default();
        expect.tick_player_zoom(
            s.camera.clamp(Vec3::new(POS.x, POS.y + 50.0, 0.0)),
            (0.0, pitch * core::f32::consts::PI / 180.0),
            dist,
            0.3,
            31.5,
        );
        tick(&mut rig, None, &s);
        assert_eq!(rig.camera, expect, "{follow:?}");
    }
}

/// The entry focus, then the demo focus, then Close-Up; each only while
/// its fighter is in the camera bounds, the battle camera otherwise.
#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn player_zooms_take_precedence_in_order_while_in_bounds() {
    let s = stage(BonusFollow::Other);
    let entry = EntryFocusTarget {
        pos: Vec3::new(-300.0, 0.0, 0.0),
        cam_offset_y: 120.0,
        dist: 1500.0,
    };
    let demo = DemoZoom {
        port: 1,
        eye: (0.2, -0.1),
        dist: 1800.0,
        pan_scale: 0.3,
        fov: 30.0,
        pos: Vec3::new(700.0, 100.0, 0.0),
        cam_offset_y: 80.0,
    };
    let zoomed = |pos: Vec3, off: f32, eye, dist, pan, fov| {
        let mut c = Camera::default();
        assert!(c.tick_player_zoom_in_bounds(pos, off, s.camera, eye, dist, pan, fov));
        c
    };
    let all = CameraRig {
        demo_zoom: Some(demo),
        player_zoom: Some(1200.0),
        ..CameraRig::default()
    };
    let mut rig = all;
    tick(&mut rig, Some(entry), &s);
    assert_eq!(
        rig.camera,
        zoomed(
            entry.pos,
            120.0,
            (0.0, 0.0),
            1500.0,
            crate::pause::ZOOM_PAN_SCALE,
            crate::appear::FOCUS_ZOOM_FOV
        )
    );
    // The entry's fighter out of bounds: the demo focus.
    let out = EntryFocusTarget {
        pos: Vec3::new(9000.0, 0.0, 0.0),
        ..entry
    };
    let mut rig = all;
    tick(&mut rig, Some(out), &s);
    assert_eq!(
        rig.camera,
        zoomed(demo.pos, 80.0, demo.eye, 1800.0, 0.3, 30.0)
    );
    // No demo focus: Close-Up on the lead.
    let mut rig = CameraRig {
        demo_zoom: None,
        ..all
    };
    tick(&mut rig, Some(out), &s);
    assert_eq!(
        rig.camera,
        zoomed(
            POS,
            50.0,
            (0.0, 0.0),
            1200.0,
            crate::training::CLOSE_UP_PAN_SCALE,
            crate::training::CLOSE_UP_FOV
        )
    );
    // Every zoomed fighter out of bounds: the battle camera.
    let mut rig = CameraRig {
        demo_zoom: Some(DemoZoom {
            pos: Vec3::new(0.0, -9000.0, 0.0),
            ..demo
        }),
        player_zoom: None,
        ..all
    };
    tick(&mut rig, Some(out), &s);
    assert_eq!(
        rig.camera,
        battle(Camera::default(), &[interest(0.0), interest(500.0)], &s)
    );
}

/// `gmCameraUpdateInterests` reads at most four fighters.
#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn the_battle_camera_frames_at_most_four_interests() {
    let s = stage(BonusFollow::Other);
    let five: Vec<Interest> = (0..5).map(|i| interest(i as f32 * 400.0)).collect();
    let mut rig = CameraRig::default();
    rig.tick(POS, 0.0, &five, None, &s);
    assert_eq!(rig.camera, battle(Camera::default(), &five[..4], &s));
    assert_eq!(rig.stage_camera, StageCamera::Zebes);
}

#[test]
fn interest_follows_the_fighters_camera_mode() {
    let s = stage(BonusFollow::Other);
    let framing = Framing {
        cam_offset_y: 60.0,
        zoom_frame: 1.25,
    };
    let mut f = Fighter::new(FighterKind::Mario, 0, 3);
    f.pos = Vec3::new(100.0, 50.0, 0.0);
    f.facing = Facing::Left;
    let i = interest_of(&f, framing, &s).unwrap();
    assert_eq!(i.target_pos, Vec3::new(100.0, 110.0, 0.0));
    assert!(i.facing_left && !i.dead_up && i.team_bounds.is_none());
    assert_eq!((i.zoom_frame, i.zoom_range), (1.25, 1.0));
    // Entering: the entry's position and facing.
    f.dead.camera_mode = CameraMode::Entry;
    f.entry.pos = Vec3::new(-200.0, 900.0, 0.0);
    f.entry.lr = Some(Facing::Right);
    let i = interest_of(&f, framing, &s).unwrap();
    assert_eq!(i.target_pos, Vec3::new(-200.0, 960.0, 0.0));
    assert!(!i.facing_left);
    // A ghost is no interest.
    f.dead.camera_mode = CameraMode::Ghost;
    assert_eq!(interest_of(&f, framing, &s), None);
}

/// `gmCameraSetDeadUpStarPosition`: the top-out point scaled onto the
/// camera's top; a 1P enemy keeps its point under the team bounds.
#[test]
fn dead_up_scales_onto_the_camera_top_unless_team_bounded() {
    let s = stage(BonusFollow::Other);
    let mut f = Fighter::new(FighterKind::Mario, 0, 3);
    f.dead.camera_mode = CameraMode::DeadUp;
    f.dead.up_pos = Vec3::new(1200.0, 7000.0, 0.0);
    let i = interest_of(&f, Framing::default(), &s).unwrap();
    assert!(i.dead_up);
    assert_eq!(
        i.target_pos,
        Vec3::new(3000.0 * 1200.0 / 6000.0, 3000.0, 0.0)
    );
    f.dead.team_bounds = Some(crate::status::BlastZone {
        top: 1.0,
        bottom: -1.0,
        left: -1.0,
        right: 1.0,
    });
    let i = interest_of(&f, Framing::default(), &s).unwrap();
    assert!(!i.dead_up);
    assert_eq!(i.team_bounds, Some(s.team_camera));
    assert_eq!(i.target_pos, Vec3::new(1200.0, 7000.0, 0.0));
}

/// `gmCameraCalcFighterZoomRange`: a reborn fighter's
/// `camera_zoom_range` (0.6) shrinks its box.
#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn a_reborn_fighters_interest_carries_its_zoom_range() {
    use crate::dead::{CameraZoomRange, REBIRTH_CAMERA_ZOOM_RANGE};
    let s = stage(BonusFollow::Other);
    let framing = Framing {
        cam_offset_y: 0.0,
        zoom_frame: 1.5,
    };
    let mut f = Fighter::new(FighterKind::Mario, 0, 3);
    assert_eq!(interest_of(&f, framing, &s).unwrap().zoom_range, 1.0);
    f.dead.camera_zoom_range = CameraZoomRange(REBIRTH_CAMERA_ZOOM_RANGE);
    let i = interest_of(&f, framing, &s).unwrap();
    assert_eq!(i.zoom_range, 0.6);
    // One fighter: the distance stays at its 2500 floor, but the
    // asymmetric box (700 behind, 1000 ahead, times the one-player zoom
    // and 0.6) moves the interest's centre.
    let normal = crate::camera::calculate_interest_scaled(&[interest(0.0)], &[], s.camera, 1.0);
    let reborn = crate::camera::calculate_interest_scaled(
        &[Interest {
            zoom_range: 0.6,
            ..interest(0.0)
        }],
        &[],
        s.camera,
        1.0,
    );
    let adjust = super::super::ONE_PLAYER_ZOOM;
    assert_eq!(normal.1, 850.0 * adjust);
    assert_eq!(reborn.1, 850.0 * adjust * 0.6);
    assert!(reborn.0.x < normal.0.x);
}

/// Four fighters: one reborn fighter on the right edge narrows the box,
/// and the camera settles closer.
#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn a_reborn_fighter_pulls_a_four_player_camera_in() {
    let s = CameraStage {
        stage_camera: StageCamera::Default,
        ..stage(BonusFollow::Other)
    };
    let framing = Framing {
        cam_offset_y: 0.0,
        zoom_frame: 1.0,
    };
    let interests = |reborn: bool| -> Vec<Interest> {
        [-2000.0, -600.0, 600.0, 2000.0]
            .iter()
            .enumerate()
            .map(|(port, &x)| {
                let mut f = Fighter::new(FighterKind::Mario, port as u8, 3);
                f.pos = Vec3::new(x, 100.0, 0.0);
                f.facing = Facing::Right;
                if reborn && port == 3 {
                    f.dead.camera_zoom_range =
                        crate::dead::CameraZoomRange(crate::dead::REBIRTH_CAMERA_ZOOM_RANGE);
                }
                interest_of(&f, framing, &s).unwrap()
            })
            .collect()
    };
    let (all, one) = (interests(false), interests(true));
    let (_, hz_all, _) = crate::camera::calculate_interest_scaled(&all, &[], s.camera, 1.0);
    let (_, hz_one, _) = crate::camera::calculate_interest_scaled(&one, &[], s.camera, 1.0);
    // 1000 ahead of the right-most fighter becomes 600.
    assert_eq!((hz_all, hz_one), (2850.0, 2650.0));
    let (mut a, mut b) = (Camera::default(), Camera::default());
    for _ in 0..200 {
        a = battle(a, &all, &s);
        b = battle(b, &one, &s);
    }
    let dist = |c: &Camera| (c.eye - c.at).length();
    assert!(dist(&b) < dist(&a) - 100.0, "{} vs {}", dist(&b), dist(&a));
}
