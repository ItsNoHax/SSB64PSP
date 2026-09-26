//! Independent ROM geometry input to the portable fighter map solver.
use ssb_engine::math::{Vec2, Vec3};
use ssb_game::collision::Segment;
use ssb_game::ground::BodyColl;
use ssb_game::map::{self, AirOptions};
use ssb_game::weapon::{MapSurface, MapSurfaceKind, SurfaceTopology};
use ssb_rom::archive::Archive;
use ssb_rom::collision::LineKind;

#[test]
fn dream_land_rom_geometry_drives_floor_wall_ceiling_and_cliff_queries() {
    let Ok(path) = std::env::var("SSB64_ROM") else {
        return;
    };
    let rom = std::fs::read(path).expect("SSB64_ROM readable");
    let info = ssb_rom::rom::identify(&rom).expect("verified ROM");
    let archive = Archive::open(&rom, info.region).unwrap();
    // Dream Land's decoded extern geometry (RE-345), independent of the pack.
    let file = archive.load(104).unwrap();
    let geometry = ssb_rom::collision::read(&file, 0x1F34).unwrap();
    let mut surfaces = Vec::new();
    for line in &geometry.lines {
        let kind = match line.kind {
            LineKind::Floor => MapSurfaceKind::Floor,
            LineKind::Ceiling => MapSurfaceKind::Ceiling,
            LineKind::LeftWall => MapSurfaceKind::LeftWall,
            LineKind::RightWall => MapSurfaceKind::RightWall,
        };
        for (point, pair) in line.points.windows(2).enumerate() {
            surfaces.push(MapSurface {
                kind,
                segment: Segment {
                    x1: pair[0].pos[0],
                    y1: pair[0].pos[1],
                    x2: pair[1].pos[0],
                    y2: pair[1].pos[1],
                    flags: pair[0].flags,
                },
                topology: Some(SurfaceTopology {
                    line: line.id,
                    point: point as u16,
                    segments: line.points.len() as u16 - 1,
                    vertex1: pair[0].vertex_id,
                    vertex2: pair[1].vertex_id,
                }),
            });
        }
    }
    let input = || surfaces.iter().copied();
    let floor = map::move_air(
        &BodyColl::MARIO,
        Vec3::new(0.0, 500.0, 0.0),
        Vec3::new(0.0, -200.0, 0.0),
        AirOptions::default(),
        input,
    );
    assert_eq!(floor.moved.floor.unwrap().line, 3);
    assert_eq!(floor.moved.pos.y, 0.0);
    let wall = map::move_air(
        &BodyColl::MARIO,
        Vec3::new(2500.0, -500.0, 0.0),
        Vec3::new(2000.0, -500.0, 0.0),
        AirOptions::default(),
        input,
    );
    assert_eq!(wall.contacts.right_wall.unwrap().line, 5);
    assert!(wall.moved.pos.x > 2000.0);
    let left = map::move_air(
        &BodyColl::MARIO,
        Vec3::new(-2500.0, -500.0, 0.0),
        Vec3::new(-2000.0, -500.0, 0.0),
        AirOptions::default(),
        input,
    );
    assert_eq!(left.contacts.left_wall.unwrap().line, 6);
    assert!(left.moved.pos.x < -2000.0);
    let ceiling = geometry.lines.iter().find(|line| line.id == 4).unwrap();
    let y = ceiling.points[0].pos[1] as f32;
    let ceil = map::move_air(
        &BodyColl::MARIO,
        Vec3::new(0.0, y - 400.0, 0.0),
        Vec3::new(0.0, y - 200.0, 0.0),
        AirOptions::default(),
        input,
    );
    assert_eq!(ceil.contacts.ceiling.unwrap().line, 4);
    assert_eq!(ceil.moved.pos.y, y - BodyColl::MARIO.top);
    let cliff = map::cliff(
        input,
        -1.0,
        0,
        Vec2::new(400.0, 360.0),
        Vec3::new(2518.0, -350.0, 0.0),
        Vec3::new(2518.0, -370.0, 0.0),
    );
    assert_eq!(cliff, Some((3, Vec2::new(2318.0, 0.0))));
}
