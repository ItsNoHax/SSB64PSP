use super::*;
use crate::fighter::{FighterKind, Situation};
use crate::status::StatusTiming;

const BODY: BodyColl = BodyColl {
    top: 20.0,
    center: 10.0,
    bottom: 0.0,
    width: 10.0,
};

fn surface(kind: Kind, line: u16, a: (i16, i16), b: (i16, i16), flags: u16) -> MapSurface {
    MapSurface {
        motion: None,
        kind,
        segment: Segment {
            x1: a.0,
            y1: a.1,
            x2: b.0,
            y2: b.1,
            flags,
        },
        topology: Some(SurfaceTopology {
            line,
            point: 0,
            segments: 1,
            vertex1: line * 2,
            vertex2: line * 2 + 1,
        }),
    }
}

fn air(s: &[MapSurface], from: Vec3, to: Vec3) -> AirMoved {
    move_air(&BODY, from, to, AirOptions::default(), || s.iter().copied())
}

#[test]
fn both_wall_sides_stop_at_the_waist_and_keep_vertical_motion() {
    let left = surface(Kind::LeftWall, 8, (100, -100), (100, 100), 3);
    let moved = air(
        &[left],
        Vec3::new(50.0, 0.0, 0.0),
        Vec3::new(120.0, 30.0, 0.0),
    );
    assert_eq!(moved.moved.pos, Vec3::new(90.0, 30.0, 0.0));
    assert_eq!(
        moved.contacts.left_wall.unwrap(),
        Contact {
            line: 8,
            flags: 3,
            normal: Vec2::new(-1.0, 0.0)
        }
    );
    let right = surface(Kind::RightWall, 9, (-100, -100), (-100, 100), 0);
    let moved = air(
        &[right],
        Vec3::new(-50.0, 0.0, 0.0),
        Vec3::new(-120.0, -30.0, 0.0),
    );
    assert_eq!(moved.moved.pos, Vec3::new(-90.0, -30.0, 0.0));
    assert_eq!(
        moved.contacts.right_wall.unwrap().normal,
        Vec2::new(1.0, 0.0)
    );
}

#[test]
fn walls_are_one_sided_and_independent_of_vertex_winding() {
    let wall = surface(Kind::LeftWall, 1, (100, -100), (100, 100), 0);
    assert!(air(
        &[wall],
        Vec3::new(130.0, 0.0, 0.0),
        Vec3::new(50.0, 0.0, 0.0)
    )
    .contacts
    .left_wall
    .is_none());
    let reversed = surface(Kind::LeftWall, 1, (100, 100), (100, -100), 0);
    let from = Vec3::new(50.0, 0.0, 0.0);
    let to = Vec3::new(120.0, 0.0, 0.0);
    assert_eq!(air(&[wall], from, to), air(&[reversed], from, to));
}

#[test]
fn wall_vertices_inside_the_diamond_constrain_the_origin() {
    // Waist is above this wall: its upper vertex meets the lower diamond edge.
    let wall = surface(Kind::LeftWall, 1, (100, -30), (100, 5), 0);
    let moved = air(
        &[wall],
        Vec3::new(70.0, 0.0, 0.0),
        Vec3::new(110.0, 0.0, 0.0),
    );
    assert_eq!(moved.moved.pos.x, 95.0);
    assert!(moved.contacts.left_wall.is_some());
}

#[test]
fn tilted_wall_samples_all_three_diamond_points() {
    let wall = surface(Kind::LeftWall, 1, (100, -10), (70, 20), 0);
    let moved = air(
        &[wall],
        Vec3::new(40.0, 0.0, 0.0),
        Vec3::new(120.0, 0.0, 0.0),
    );
    assert_eq!(moved.moved.pos.x, 70.0);
    let n = moved.contacts.left_wall.unwrap().normal;
    assert!((n.x + 0.70710677).abs() < 0.001);
    assert!((n.y + 0.70710677).abs() < 0.001);
}

#[test]
fn ceiling_uses_the_top_tip_and_preserves_destination_x() {
    let ceil = surface(Kind::Ceiling, 2, (-500, 100), (500, 100), 0);
    let moved = air(
        &[ceil],
        Vec3::new(0.0, 50.0, 0.0),
        Vec3::new(25.0, 120.0, 0.0),
    );
    assert_eq!(moved.moved.pos, Vec3::new(25.0, 80.0, 0.0));
    assert_eq!(moved.contacts.ceiling.unwrap().normal, Vec2::new(0.0, -1.0));
    assert!(air(
        &[ceil],
        Vec3::new(0.0, 120.0, 0.0),
        Vec3::new(0.0, 50.0, 0.0)
    )
    .contacts
    .ceiling
    .is_none());
}

#[test]
fn substeps_retain_wall_contact_and_land_after_sliding() {
    let wall = surface(Kind::LeftWall, 1, (100, -1000), (100, 1000), 0);
    let floor = surface(Kind::Floor, 2, (-1000, 0), (1000, 0), 0);
    let moved = air(
        &[wall, floor],
        Vec3::new(0.0, 500.0, 0.0),
        Vec3::new(500.0, -500.0, 0.0),
    );
    assert_eq!(moved.moved.pos, Vec3::new(90.0, 0.0, 0.0));
    assert_eq!(moved.moved.floor.unwrap().line, 2);
    assert!(moved.contacts.left_wall.is_some());
}

#[test]
fn source_pass_callback_rejects_nearest_floor_without_retrying_the_probe() {
    let soft = surface(
        Kind::Floor,
        1,
        (-100, 10),
        (100, 10),
        collision::flags::PASS,
    );
    let solid = surface(Kind::Floor, 2, (-100, 0), (100, 0), 0);
    let options = AirOptions {
        skip_pass: true,
        ..AirOptions::default()
    };
    let moved = move_air(
        &BODY,
        Vec3::new(0.0, 20.0, 0.0),
        Vec3::new(0.0, -10.0, 0.0),
        options,
        || [soft, solid],
    );
    assert!(moved.moved.floor.is_none());
    let moved = move_air(
        &BODY,
        Vec3::new(0.0, 20.0, 0.0),
        Vec3::new(0.0, -10.0, 0.0),
        AirOptions::default(),
        || [soft, solid],
    );
    assert_eq!(moved.moved.floor.unwrap().line, 1);
}

#[test]
fn cliff_uses_hand_reach_and_whole_polyline_edge() {
    let mut a = surface(Kind::Floor, 3, (-200, 0), (0, 0), collision::flags::CLIFF);
    let mut b = surface(Kind::Floor, 3, (0, 0), (200, 0), collision::flags::CLIFF);
    a.topology.as_mut().unwrap().segments = 2;
    b.topology.as_mut().unwrap().point = 1;
    b.topology.as_mut().unwrap().segments = 2;
    let caught = cliff(
        || [a, b],
        -1.0,
        0,
        Vec2::new(100.0, 50.0),
        Vec3::new(250.0, -40.0, 0.0),
        Vec3::new(250.0, -60.0, 0.0),
    );
    assert_eq!(caught, Some((3, Vec2::new(200.0, 0.0))));
    assert!(cliff(
        || [a, b],
        -1.0,
        0,
        Vec2::ZERO,
        Vec3::new(250.0, -40.0, 0.0),
        Vec3::new(250.0, -60.0, 0.0)
    )
    .is_none());
}

#[test]
fn cliff_material_four_exclusion_is_left_only_and_cooldown_applies() {
    let floor = surface(
        Kind::Floor,
        3,
        (-100, 0),
        (100, 0),
        collision::flags::CLIFF | 4,
    );
    let from = Vec3::new(0.0, 10.0, 0.0);
    let to = Vec3::new(0.0, -10.0, 0.0);
    assert!(cliff(|| [floor], 1.0, 0, Vec2::ZERO, from, to).is_none());
    assert!(cliff(|| [floor], -1.0, 0, Vec2::ZERO, from, to).is_some());
    assert!(cliff(|| [floor], -1.0, 1, Vec2::ZERO, from, to).is_none());
}

#[test]
fn cliff_is_tested_per_substep_and_an_occupied_corner_is_skipped() {
    let floor = surface(Kind::Floor, 3, (-100, 0), (100, 0), collision::flags::CLIFF);
    let query = CliffQuery {
        facing: 1.0,
        wait: 0,
        reach: Vec2::new(100.0, 100.0),
        occupied: [None; 3],
    };
    let from = Vec3::new(-150.0, 100.0, 0.0);
    let to = Vec3::new(-150.0, -600.0, 0.0);
    let moved = move_air(
        &BODY,
        from,
        to,
        AirOptions {
            cliff: Some(query),
            ..AirOptions::default()
        },
        || [floor],
    );
    assert_eq!(moved.cliff, Some((3, Vec2::new(-100.0, 0.0))));
    assert!(moved.moved.pos.y > to.y);
    let moved = move_air(
        &BODY,
        from,
        to,
        AirOptions {
            cliff: Some(CliffQuery {
                occupied: [None, Some((3, 1.0)), None],
                ..query
            }),
            ..AirOptions::default()
        },
        || [floor],
    );
    assert!(moved.cliff.is_none());
    assert_eq!(moved.moved.pos, to);
}

#[test]
fn neighbor_identity_uses_original_vertex_ids_and_highest_line_id() {
    let floor = surface(Kind::Floor, 1, (-100, 0), (100, 0), 0);
    let mut wall = surface(Kind::RightWall, 2, (-100, -100), (-100, 0), 0);
    assert_eq!(neighbor(&|| [floor, wall], Kind::Floor, 1, false), None);
    wall.topology.as_mut().unwrap().vertex2 = floor.topology.unwrap().vertex1;
    let mut higher = wall;
    higher.topology.as_mut().unwrap().line = 7;
    assert_eq!(
        neighbor(&|| [higher, floor, wall], Kind::Floor, 1, false),
        Some((Kind::RightWall, 7))
    );
}

#[test]
fn edge_stops_are_selected_by_status_and_do_not_affect_walk_offs() {
    let floor = surface(Kind::Floor, 1, (-100, 0), (100, 0), 0);
    let from = Vec3::new(90.0, 0.0, 0.0);
    let to = Vec3::new(120.0, 0.0, 0.0);
    let (m, _) = move_ground(&BODY, from, to, 1, true, || [floor]);
    assert_eq!(m.pos.x, 100.0);
    assert!(m.floor.is_some());
    assert!(move_ground(&BODY, from, to, 1, false, || [floor])
        .0
        .floor
        .is_none());
    assert!(stops_at_edge(AnyStatus::Purin(PurinStatus::SpecialHi)));
    assert!(!stops_at_edge(AnyStatus::Common(Status::WalkFast)));
}

fn fighter(kind: FighterKind, s: AnyStatus, velocity: Vec3) -> Fighter {
    let mut f = Fighter::new(kind, 0, 3);
    f.coll = BODY;
    status::set_any_status(&mut f, s, 0.0, StatusTiming::unknown());
    f.physics.vel_air = velocity;
    f
}

#[test]
fn quick_attack_wall_and_ceiling_cancel_only_above_135_degrees() {
    let mut f = fighter(
        FighterKind::Pikachu,
        AnyStatus::Pikachu(PikachuStatus::SpecialAirHi),
        Vec3::new(100.0, 0.0, 0.0),
    );
    f.map_contacts.left_wall = Some(Contact {
        line: 1,
        flags: 0,
        normal: Vec2::new(-1.0, 0.0),
    });
    air_callback(&mut f);
    assert_eq!(
        f.status.status,
        AnyStatus::Pikachu(PikachuStatus::SpecialAirHiEnd)
    );
    assert_eq!(f.physics.vel_air.x, 20.0);
    let mut middle = fighter(
        FighterKind::Pikachu,
        AnyStatus::Pikachu(PikachuStatus::SpecialAirHi),
        Vec3::new(60.0, 80.0, 0.0),
    );
    middle.map_contacts = f.map_contacts;
    air_callback(&mut middle);
    assert_eq!(
        middle.status.status,
        AnyStatus::Pikachu(PikachuStatus::SpecialAirHi)
    );
    let mut shallow = fighter(
        FighterKind::Pikachu,
        AnyStatus::Pikachu(PikachuStatus::SpecialAirHi),
        Vec3::new(20.0, 100.0, 0.0),
    );
    shallow.map_contacts = f.map_contacts;
    air_callback(&mut shallow);
    assert_eq!(
        shallow.status.status,
        AnyStatus::Pikachu(PikachuStatus::SpecialAirHi)
    );
    shallow.map_contacts.ceiling = Some(Contact {
        line: 2,
        flags: 0,
        normal: Vec2::new(0.0, -1.0),
    });
    air_callback(&mut shallow);
    assert_eq!(
        shallow.status.status,
        AnyStatus::Pikachu(PikachuStatus::SpecialAirHiEnd)
    );
}

#[test]
fn fire_fox_redirects_shallow_new_contacts_and_keeps_speed() {
    let mut f = fighter(
        FighterKind::Fox,
        AnyStatus::Fox(FoxStatus::SpecialAirHi),
        Vec3::new(10.0, 100.0, 0.0),
    );
    f.map_contacts.left_wall = Some(Contact {
        line: 1,
        flags: 0,
        normal: Vec2::new(-1.0, 0.0),
    });
    let speed = f.physics.vel_air.length();
    air_callback(&mut f);
    assert_eq!(f.physics.vel_air.x, 0.0);
    assert!((f.physics.vel_air.y - speed).abs() < 0.001);
    f.map_contacts_prev = f.map_contacts;
    f.physics.vel_air.x = 10.0;
    air_callback(&mut f);
    assert_eq!(f.physics.vel_air.x, 10.0);

    let floor = surface(Kind::Floor, 3, (-1000, 0), (1000, 0), 0);
    f.status.status = AnyStatus::Fox(FoxStatus::SpecialHi);
    f.fox_special_hi.travel_frames = 30;
    f.situation = crate::fighter::Situation::Ground;
    f.floor = Some(Standing {
        line: 3,
        flags: 0,
        normal: Vec2::new(0.0, 1.0),
    });
    f.pos = Vec3::ZERO;
    f.tick_map(|| [floor]);
    assert_eq!(f.fox_special_hi.pass_timer, 1);
}

#[test]
fn ness_blast_rebounds_steep_contacts_and_slides_shallow_walls() {
    let mut f = fighter(
        FighterKind::Ness,
        AnyStatus::Ness(NessStatus::SpecialAirHiJibaku),
        Vec3::new(0.0, 200.0, 0.0),
    );
    f.map_contacts.ceiling = Some(Contact {
        line: 1,
        flags: 0,
        normal: Vec2::new(0.0, -1.0),
    });
    air_callback(&mut f);
    assert_eq!(
        f.status.status,
        AnyStatus::Ness(NessStatus::SpecialAirHiBound)
    );
    assert_eq!(f.physics.vel_air.y, -100.0);
    assert_eq!(f.facing, Facing::Left);
    let mut f = fighter(
        FighterKind::Ness,
        AnyStatus::Ness(NessStatus::SpecialAirHiJibaku),
        Vec3::new(20.0, 100.0, 0.0),
    );
    f.ness.blast_angle = ssb_engine::math::atan2(100.0, 20.0);
    f.map_contacts.left_wall = Some(Contact {
        line: 2,
        flags: 0,
        normal: Vec2::new(-1.0, 0.0),
    });
    let speed = f.physics.vel_air.length();
    air_callback(&mut f);
    assert_eq!(
        f.status.status,
        AnyStatus::Ness(NessStatus::SpecialAirHiJibaku)
    );
    assert!(f.physics.vel_air.x.abs() < 0.01);
    assert!((f.physics.vel_air.y - speed).abs() < 0.01);
}

#[test]
fn falcon_kick_wall_rebound_obeys_the_source_flag_window() {
    let mut f = fighter(
        FighterKind::Captain,
        AnyStatus::Captain(CaptainStatus::SpecialLw),
        Vec3::ZERO,
    );
    f.status.anim_frame = 12.0;
    f.map_contacts.left_wall = Some(Contact {
        line: 1,
        flags: 0,
        normal: Vec2::new(-1.0, 0.0),
    });
    assert!(ground_callback(&mut f, None));
    assert_eq!(
        f.status.status,
        AnyStatus::Captain(CaptainStatus::SpecialLwBound)
    );
    assert_eq!(f.situation, Situation::Air);
    let mut f = fighter(
        FighterKind::Captain,
        AnyStatus::Captain(CaptainStatus::SpecialLw),
        Vec3::ZERO,
    );
    f.status.anim_frame = 32.0;
    f.map_contacts.right_wall = Some(Contact {
        line: 1,
        flags: 0,
        normal: Vec2::new(1.0, 0.0),
    });
    assert!(!ground_callback(&mut f, None));
}

#[test]
fn a_live_fighter_tick_reaches_the_wall_callback() {
    let wall = surface(Kind::LeftWall, 1, (100, -100), (100, 100), 0);
    let mut f = fighter(
        FighterKind::Pikachu,
        AnyStatus::Pikachu(PikachuStatus::SpecialAirHi),
        Vec3::new(200.0, 0.0, 0.0),
    );
    f.pikachu.zip_frames = 5;
    f.tick_map(|| [wall]);
    assert_eq!(f.pos.x, 90.0);
    assert_eq!(
        f.status.status,
        AnyStatus::Pikachu(PikachuStatus::SpecialAirHiEnd)
    );
    assert!(f.map_contacts.left_wall.is_some());
}

#[test]
fn a_held_cliff_uses_its_authored_pose_without_air_gravity() {
    let mut f = fighter(
        FighterKind::Mario,
        AnyStatus::Common(Status::CliffWait),
        Vec3::ZERO,
    );
    f.cliff.fall_wait = 100;
    f.cliff.line = 3;
    f.transn = Vec3::new(0.0, -180.0, -100.0);
    let floor = surface(Kind::Floor, 3, (100, 0), (1000, 0), 0);
    f.tick_map(|| [floor]);
    assert_eq!(f.pos, Vec3::new(0.0, -180.0, 0.0));
    assert_eq!(f.physics.vel_air, Vec3::ZERO);
}

fn moving(mut s: MapSurface, offset: Vec2, speed: Vec3) -> MapSurface {
    s.motion = Some(crate::weapon::SurfaceMotion { offset, speed });
    s
}

#[test]
fn rising_fractional_floor_lands_a_stationary_body() {
    let floor = moving(
        surface(Kind::Floor, 3, (-1000, 0), (1000, 0), 0),
        Vec2::new(0.25, 100.5),
        Vec3::new(0.25, 100.5, 0.0),
    );
    let pos = Vec3::new(0.0, 50.0, 0.0);
    let out = move_air(&BodyColl::MARIO, pos, pos, AirOptions::default(), || {
        [floor]
    });
    assert_eq!(out.moved.floor.unwrap().line, 3);
    assert_eq!(out.moved.pos.y, 100.5);
}

#[test]
fn moving_wall_and_ceiling_use_relative_sweeps_and_world_correction() {
    let wall = moving(
        surface(Kind::LeftWall, 1, (500, -1000), (500, 1000), 0),
        Vec2::new(-200.5, 0.0),
        Vec3::new(-200.5, 0.0, 0.0),
    );
    let from = Vec3::new(200.0, 0.0, 0.0);
    let out = move_air(&BodyColl::MARIO, from, from, AirOptions::default(), || {
        [wall]
    });
    assert_eq!(out.contacts.left_wall.unwrap().line, 1);
    assert_eq!(out.moved.pos.x, 149.5);
    let ceil = moving(
        surface(Kind::Ceiling, 2, (-1000, 500), (1000, 500), 0),
        Vec2::new(0.0, -200.25),
        Vec3::new(0.0, -200.25, 0.0),
    );
    let out = move_air(
        &BodyColl::MARIO,
        Vec3::ZERO,
        Vec3::ZERO,
        AirOptions::default(),
        || [ceil],
    );
    assert_eq!(out.moved.pos.y, -20.25);
    assert_eq!(out.contacts.ceiling.unwrap().line, 2);
}

#[test]
fn group_carry_is_added_on_the_first_substep() {
    let floor = moving(
        surface(Kind::Floor, 3, (-2000, 0), (2000, 0), 0),
        Vec2::new(300.0, 0.0),
        Vec3::new(300.0, 0.0, 20.0),
    );
    let wall = surface(Kind::LeftWall, 4, (400, -1000), (400, 1000), 0);
    let body = BodyColl {
        top: 100.0,
        center: 50.0,
        bottom: 0.0,
        width: 20.0,
    };
    let (out, contact) = move_ground(
        &body,
        Vec3::ZERO,
        Vec3::new(1000.0, 0.0, 60.0),
        3,
        false,
        || [floor, wall],
    );
    assert_eq!(out.pos, Vec3::new(380.0, 0.0, 30.0));
    assert!(contact.left_wall.is_some());
}

#[test]
fn copied_diamond_sweeps_previous_bottom_to_current_bottom() {
    let floor = surface(Kind::Floor, 3, (-1000, -50), (1000, -50), 0);
    let previous = BodyColl::MARIO;
    let current = BodyColl {
        bottom: -100.0,
        ..previous
    };
    let out = move_air_from_shape(
        &current,
        &previous,
        Vec3::ZERO,
        Vec3::ZERO,
        AirOptions::default(),
        || [floor],
    );
    assert_eq!(out.moved.pos.y, 50.0);
    assert!(out.moved.floor.is_some());
    assert!(move_air(
        &current,
        Vec3::ZERO,
        Vec3::ZERO,
        AirOptions::default(),
        || [floor]
    )
    .moved
    .floor
    .is_none());
}

#[test]
fn group_states_gate_translation_and_collision() {
    let mut group = MapGroup {
        translate: Vec3::new(25.5, 100.25, 0.0),
        ..MapGroup::default()
    };
    assert!(group.exists());
    assert!(group.motion().is_none());
    group.status = GroupStatus::On;
    assert_eq!(group.motion().unwrap().offset, Vec2::new(25.5, 100.25));
    group.set_position(Vec3::new(30.0, 99.75, 2.0));
    assert_eq!(group.speed, Vec3::new(4.5, -0.5, 2.0));
    for status in [GroupStatus::Off, GroupStatus::Hidden] {
        group.status = status;
        assert!(!group.exists());
    }
    group.status = GroupStatus::Show;
    assert!(group.exists());
}

#[test]
fn hitlag_keeps_fighter_attached_to_moving_floor() {
    let floor = moving(
        surface(Kind::Floor, 3, (-2000, 0), (2000, 0), 0),
        Vec2::new(12.5, 20.25),
        Vec3::new(12.5, 20.25, 0.0),
    );
    let mut f = fighter(FighterKind::Mario, Status::Wait.into(), Vec3::ZERO);
    f.place_on_stage([(3, floor.segment)]);
    f.hitlag = 10;
    f.tick_map(|| [floor]);
    assert_eq!(f.pos, Vec3::new(12.5, 20.25, 0.0));
}

#[test]
fn hanging_pose_follows_group_and_releases_when_group_is_disabled() {
    let floor = moving(
        surface(Kind::Floor, 3, (100, 0), (1000, 0), 0),
        Vec2::new(10.5, 25.25),
        Vec3::new(10.5, 25.25, 0.0),
    );
    let mut f = fighter(FighterKind::Mario, Status::CliffWait.into(), Vec3::ZERO);
    f.cliff.line = 3;
    f.cliff.fall_wait = 100;
    f.attributes.size = 2.0;
    f.transn = Vec3::new(0.0, -100.0, -50.0);
    f.tick_map(|| [floor]);
    assert_eq!(f.pos, Vec3::new(10.5, -174.75, 0.0));
    f.tick_map(core::iter::empty);
    assert_eq!(f.status.status, Status::Fall);
    assert!(f.floor.is_none());
}

#[test]
fn cliff_phase_two_places_on_the_current_corner_and_follows_transn() {
    let floor = moving(
        surface(Kind::Floor, 3, (100, 0), (1000, 0), 0),
        Vec2::new(10.5, 25.25),
        Vec3::ZERO,
    );
    let mut f = fighter(
        FighterKind::Fox,
        Status::CliffClimbQuick2.into(),
        Vec3::new(-400.0, -100.0, 0.0),
    );
    f.cliff.line = 3;
    f.cliff.place_phase2 = true;
    f.set_root_motion(crate::physics::RootMotion {
        delta: Vec3::new(0.0, 0.0, 12.0),
        rotate_z: 0.0,
        ..Default::default()
    });
    f.tick_map(|| [floor]);
    assert_eq!(f.pos, Vec3::new(127.5, 25.25, 0.0));
    assert!(f.is_grounded());
    assert_eq!(f.floor.unwrap().line, 3);
}

#[test]
fn full_signed_vertex_range_survives_axis_reflection_and_widening() {
    let ceil = surface(
        Kind::Ceiling,
        1,
        (i16::MIN, i16::MIN),
        (i16::MAX, i16::MIN),
        0,
    );
    let moved = air(
        &[ceil],
        Vec3::new(0.0, -32800.0, 0.0),
        Vec3::new(0.0, -32760.0, 0.0),
    );
    assert_eq!(moved.moved.pos.y, -32788.0);
    assert_eq!(moved.contacts.ceiling.unwrap().normal, Vec2::new(0.0, -1.0));
    let floor = surface(Kind::Floor, 2, (i16::MIN, 0), (i16::MAX, 0), 0);
    let moved = air(
        &[floor],
        Vec3::new(0.0, 20.0, 0.0),
        Vec3::new(0.0, -20.0, 0.0),
    );
    assert_eq!(moved.moved.floor.unwrap().line, 2);
}

#[test]
fn a_live_fighter_catches_with_hand_reach_and_respects_ledge_hog() {
    let floor = surface(Kind::Floor, 3, (-100, 0), (100, 0), collision::flags::CLIFF);
    let mut f = fighter(
        FighterKind::Mario,
        AnyStatus::Common(Status::Fall),
        Vec3::new(0.0, -20.0, 0.0),
    );
    f.pos = Vec3::new(-150.0, -40.0, 0.0);
    f.cliff_reach = Vec2::new(100.0, 50.0);
    let mut occupied = f.clone();
    occupied.occupied_cliffs[0] = Some((3, Facing::Right));
    f.tick_map(|| [floor]);
    assert_eq!(f.status.status, AnyStatus::Common(Status::CliffCatch));
    assert_eq!(f.pos, Vec3::new(-100.0, 0.0, 0.0));
    assert!(f.floor.is_none());
    occupied.tick_map(|| [floor]);
    assert_eq!(occupied.status.status, AnyStatus::Common(Status::Fall));
}

#[test]
fn helpless_fall_pass_callback_uses_the_exact_down_stick_threshold() {
    let floor = surface(Kind::Floor, 3, (-100, 0), (100, 0), collision::flags::PASS);
    let mut f = fighter(
        FighterKind::Mario,
        AnyStatus::Common(Status::FallSpecial),
        Vec3::new(0.0, -20.0, 0.0),
    );
    f.pos.y = 10.0;
    f.fall_special.is_allow_pass = true;
    f.input.stick_y = -45;
    let mut lands = f.clone();
    lands.input.stick_y = -44;
    f.tick_map(|| [floor]);
    assert!(f.floor.is_none());
    assert!(f.pos.y < 0.0);
    lands.tick_map(|| [floor]);
    assert!(lands.floor.is_some());
    assert_eq!(lands.pos.y, 0.0);
}

#[test]
fn swallowed_star_reflects_from_the_ceiling_with_source_flag_updates() {
    let mut f = fighter(
        FighterKind::Mario,
        AnyStatus::Common(Status::ThrownKirbyStar),
        Vec3::new(0.0, 100.0, 0.0),
    );
    f.kirby_capture.flag1 = 20;
    f.map_contacts.ceiling = Some(Contact {
        line: 1,
        flags: 0,
        normal: Vec2::new(0.0, -1.0),
    });
    air_callback(&mut f);
    assert_eq!(f.physics.vel_air.y, -100.0);
    assert_eq!(f.kirby_capture.flag1, 0);
    assert_eq!(f.kirby_capture.lr, 1.0);
}

#[test]
fn aerial_jump_and_recovery_cliff_gates_follow_their_source_callbacks() {
    let mut f = fighter(
        FighterKind::Captain,
        AnyStatus::Captain(CaptainStatus::SpecialAirHi),
        Vec3::new(0.0, -1.0, 0.0),
    );
    f.captain.dive_cliff_wait = 1;
    assert!(!allows_cliff(&f));
    f.physics.vel_air.y = 1.0;
    assert!(allows_cliff(&f));
    f.captain.dive_cliff_wait = 0;
    f.physics.vel_air.y = -1.0;
    assert!(allows_cliff(&f));
    f.status.status = AnyStatus::Kirby(KirbyStatus::JumpAerialF5);
    assert!(allows_cliff(&f));
    f.status.status = AnyStatus::Purin(PurinStatus::JumpAerialF3);
    assert!(allows_cliff(&f));
    f.status.status = AnyStatus::Yoshi(status::YoshiStatus::SpecialAirLwStart);
    f.status.anim_frame = 4.0;
    assert!(!allows_cliff(&f));
    f.status.anim_frame = 5.0;
    assert!(allows_cliff(&f));
}

// ---------------------------------------------------------------------------
// Damage sweep (`mpCommonProcFighterDamage`) and the surface reactions
// ---------------------------------------------------------------------------

fn damage(s: &[MapSurface], from: Vec3, to: Vec3, prev: u16, hitlag: bool) -> DamageMoved {
    move_damage(&BODY, from, to, prev, hitlag, None, || s.iter().copied())
}

#[test]
fn a_head_on_wall_is_struck_only_above_30_units_and_once() {
    let wall = surface(Kind::LeftWall, 1, (100, -100), (100, 100), 0);
    let fast = damage(
        &[wall],
        Vec3::new(50.0, 0.0, 0.0),
        Vec3::new(131.0, 0.0, 0.0),
        0,
        false,
    );
    assert!(fast.collide);
    assert_eq!(fast.mask_curr, MASK_LWALL);
    assert_eq!(fast.moved.pos.x, 90.0);
    assert_eq!(fast.lwall_normal, Vec2::new(-1.0, 0.0));
    // `lbCommonMag2D(&pos_diff) > 30.0F`: 30 exactly only stops.
    let slow = damage(
        &[wall],
        Vec3::new(70.0, 0.0, 0.0),
        Vec3::new(100.0, 0.0, 0.0),
        0,
        false,
    );
    assert!(!slow.collide);
    assert_eq!(slow.moved.pos.x, 90.0);
    // Already struck last frame (`coll_mask_prev`): no second bounce.
    let again = damage(
        &[wall],
        Vec3::new(50.0, 0.0, 0.0),
        Vec3::new(131.0, 0.0, 0.0),
        MASK_LWALL,
        false,
    );
    assert!(!again.collide);
}

#[test]
fn a_wall_is_struck_only_beyond_110_degrees_from_its_normal() {
    let wall = surface(Kind::LeftWall, 1, (100, -500), (100, 500), 0);
    // 100 across, 250 up: 111.8 degrees from (-1, 0).
    let steep = damage(
        &[wall],
        Vec3::new(50.0, 0.0, 0.0),
        Vec3::new(150.0, 250.0, 0.0),
        0,
        false,
    );
    assert_eq!(steep.mask_curr, MASK_LWALL);
    // 100 across, 300 up: 108.4 degrees, a graze.
    let graze = damage(
        &[wall],
        Vec3::new(50.0, 0.0, 0.0),
        Vec3::new(150.0, 300.0, 0.0),
        0,
        false,
    );
    assert_eq!(graze.mask_curr, 0);
    assert_eq!(graze.moved.pos.x, 90.0);
}

#[test]
fn a_damage_floor_lands_steep_falls_and_carries_grazes_and_hitlag() {
    let floor = surface(Kind::Floor, 2, (-1000, 0), (1000, 0), 0);
    let steep = damage(
        &[floor],
        Vec3::new(0.0, 40.0, 0.0),
        Vec3::new(20.0, -40.0, 0.0),
        0,
        false,
    );
    assert!(steep.collide);
    assert_eq!(steep.mask_curr, MASK_FLOOR);
    assert_eq!(steep.moved.floor.unwrap().line, 2);
    assert_eq!(steep.moved.pos.y, 0.0);
    // 200 across, 40 down is 11 degrees below the floor: carried along it,
    // still airborne (`mpProcessSetCollideFloor`).
    let graze = damage(
        &[floor],
        Vec3::new(0.0, 20.0, 0.0),
        Vec3::new(200.0, -20.0, 0.0),
        0,
        false,
    );
    assert!(!graze.collide);
    assert!(graze.moved.floor.is_none());
    assert!(graze.contacts.floor);
    assert_eq!(graze.moved.pos, Vec3::new(200.0, 0.0, 0.0));
    let lag = damage(
        &[floor],
        Vec3::new(0.0, 40.0, 0.0),
        Vec3::new(0.0, -40.0, 0.0),
        0,
        true,
    );
    assert!(lag.moved.floor.is_none());
    assert_eq!(lag.moved.pos.y, 0.0);
}

#[test]
fn a_struck_wall_bounces_a_tumble_into_wall_damage() {
    let mut f = fighter(
        FighterKind::Mario,
        AnyStatus::Common(Status::DamageFlyN),
        Vec3::new(10.0, 0.0, 0.0),
    );
    f.physics.vel_knockback = Vec3::new(90.0, -20.0, 0.0);
    crate::reaction::set_wall_damage(&mut f, Vec2::new(-1.0, 0.0));
    assert_eq!(f.status.status, AnyStatus::Common(Status::WallDamage));
    // `lbCommonReflect2D` of (100, -20) off (-1, 0), scaled by 0.8.
    assert_eq!(f.physics.vel_knockback, Vec3::new(-80.0, -16.0, 0.0));
    assert_eq!(f.physics.vel_air, Vec3::ZERO);
    assert_eq!(f.facing, Facing::Right);
    let kb = ssb_engine::math::sqrt(80.0 * 80.0 + 16.0 * 16.0);
    assert_eq!(f.hitstun, (kb / 1.875) as u16);
    assert_eq!(f.damage_knockback_stack, kb);
    assert_eq!(f.intangible_frames, 15);
    assert_eq!(f.status.timing.anim_speed, 2.0);
}

#[test]
fn a_live_tumble_into_a_wall_bounces_and_falls_when_hitstun_ends() {
    let wall = surface(Kind::LeftWall, 1, (100, -500), (100, 500), 0);
    let mut f = fighter(
        FighterKind::Mario,
        AnyStatus::Common(Status::DamageFlyN),
        Vec3::ZERO,
    );
    f.physics.vel_knockback = Vec3::new(120.0, 0.0, 0.0);
    f.hitstun = 60;
    f.tick_map(|| [wall]);
    assert_eq!(f.status.status, AnyStatus::Common(Status::WallDamage));
    assert!(f.physics.vel_knockback.x < 0.0);
    assert_eq!(f.reaction.coll_mask_curr, MASK_LWALL);
    let hitstun = f.hitstun;
    for _ in 0..hitstun {
        f.tick_map(|| [wall]);
    }
    assert_eq!(f.status.status, AnyStatus::Common(Status::DamageFall));
}

#[test]
fn a_steep_tumble_landing_knocks_down_or_techs() {
    let floor = surface(Kind::Floor, 2, (-1000, 0), (1000, 0), 0);
    let mut f = fighter(
        FighterKind::Mario,
        AnyStatus::Common(Status::DamageFlyN),
        Vec3::ZERO,
    );
    f.pos.y = 30.0;
    f.physics.vel_knockback = Vec3::new(10.0, -60.0, 0.0);
    f.hitstun = 30;
    let mut tech = f.clone();
    f.tick_map(|| [floor]);
    assert_eq!(f.status.status, AnyStatus::Common(Status::DownBounceU));
    assert!(f.is_grounded());
    assert_eq!(f.pos.y, 0.0);
    tech.tics_since_last_z = 5;
    tech.tick_map(|| [floor]);
    assert_eq!(tech.status.status, AnyStatus::Common(Status::Passive));
}

#[test]
fn a_fast_rise_into_a_ceiling_stops_ceil_then_falls() {
    let ceil = surface(Kind::Ceiling, 4, (-1000, 100), (1000, 100), 0);
    let mut f = fighter(
        FighterKind::Mario,
        AnyStatus::Common(Status::JumpF),
        Vec3::new(5.0, 45.0, 0.0),
    );
    f.pos.y = 60.0;
    let mut slow = f.clone();
    f.tick_map(|| [ceil]);
    assert_eq!(f.status.status, AnyStatus::Common(Status::StopCeil));
    assert!(!f.is_grounded());
    assert_eq!(f.physics.vel_air.y, 0.0);
    // No `proc_physics`: the bonk neither falls nor slows.
    let (x, vx) = (f.pos.x, f.physics.vel_air.x);
    assert!(vx > 0.0);
    f.tick_map(|| [ceil]);
    assert_eq!(f.physics.vel_air, Vec3::new(vx, 0.0, 0.0));
    assert_eq!(f.pos.x, x + vx);
    let len = crate::motion::anim_length(FighterKind::Mario, Status::StopCeil.into()).unwrap();
    for _ in 0..len as usize {
        f.tick_map(|| [ceil]);
    }
    assert_eq!(f.status.status, AnyStatus::Common(Status::Fall));
    // After gravity the rise is below `CEILHEAVY_VEL_Y_MIN`.
    slow.physics.vel_air.y = 31.0;
    slow.tick_map(|| [ceil]);
    assert_eq!(slow.status.status, AnyStatus::Common(Status::JumpF));
}

#[test]
fn damage_fall_catches_a_ledge() {
    let floor = surface(Kind::Floor, 3, (-100, 0), (100, 0), collision::flags::CLIFF);
    let mut f = fighter(
        FighterKind::Mario,
        AnyStatus::Common(Status::DamageFall),
        Vec3::new(0.0, -20.0, 0.0),
    );
    f.pos = Vec3::new(-150.0, -40.0, 0.0);
    f.cliff_reach = Vec2::new(100.0, 50.0);
    f.tick_map(|| [floor]);
    assert_eq!(f.status.status, AnyStatus::Common(Status::CliffCatch));
}

#[test]
fn ness_jibaku_bounces_through_the_shared_down_bounce() {
    let mut f = fighter(
        FighterKind::Ness,
        AnyStatus::Ness(NessStatus::SpecialHiJibaku),
        Vec3::ZERO,
    );
    f.situation = Situation::Ground;
    crate::ness::map_down_bounce(&mut f);
    assert_eq!(f.status.status, AnyStatus::Common(Status::DownBounceU));
    assert_eq!(f.damage_mul, 0.5);
    let len = crate::motion::anim_length(FighterKind::Ness, Status::DownBounceU.into());
    assert_eq!(f.status.timing.anim_length, len);
}

#[test]
fn cliff_release_sweeps_from_the_outside_corner_into_the_live_pose() {
    let wall = surface(Kind::LeftWall, 1, (0, -100), (0, 100), 0);
    let mut f = fighter(
        FighterKind::Mario,
        AnyStatus::Common(Status::CliffWait),
        Vec3::new(0.0, -10.0, 0.0),
    );
    f.cliff.corner = Vec2::ZERO;
    f.cliff.line = 3;
    status::cliff_release_position(&mut f);
    assert_eq!(f.pos.x, 0.0, "the source changes pos_prev, not TopN");
    status::set_fall(&mut f);
    f.hitlag = 4;
    f.tick_map(|| [wall]);
    assert_eq!(f.pos.x, -BODY.width);
    assert!(f.cliff.release_previous.is_none());
}

/// `mpCommonRunFighterCollisionDefault` as a throw's release runs it
/// (RE-472): one pass from the catcher to a TopN dropped below the floor
/// lands it on the floor where it is, however far the sweep.
#[test]
fn a_released_fighter_dropped_below_the_floor_is_put_back_on_it() {
    let floor = surface(Kind::Floor, 0, (-1000, -6), (1000, -6), 0);
    let pos = run_default_collision(
        &BodyColl::MARIO,
        &BodyColl::MARIO,
        Vec3::new(104.91, -6.0, 0.0),
        Vec3::new(483.53, -16.81, 23.19),
        || [floor],
    );
    assert!((pos.x - 483.53).abs() < 0.01 && pos.y == -6.0, "{pos:?}");
    // Above the floor nothing moves it.
    let above = Vec3::new(483.53, 40.0, 0.0);
    assert_eq!(
        run_default_collision(
            &BodyColl::MARIO,
            &BodyColl::MARIO,
            Vec3::new(104.91, -6.0, 0.0),
            above,
            || [floor]
        ),
        above
    );
}
