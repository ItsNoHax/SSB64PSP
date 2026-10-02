//! Independent ROM geometry input to the portable fighter map solver.
use ssb_engine::math::{Vec2, Vec3};
use ssb_game::collision::Segment;
use ssb_game::ground::BodyColl;
use ssb_game::map::{self, AirOptions};
use ssb_game::weapon::{MapSurface, MapSurfaceKind, SurfaceTopology};
use ssb_rom::archive::Archive;
use ssb_rom::collision::LineKind;

#[test]
fn cliff_rom_joint_inventory() {
    let Ok(path) = std::env::var("SSB64_ROM") else {
        return;
    };
    let rom = std::fs::read(path).unwrap();
    let info = ssb_rom::rom::identify(&rom).unwrap();
    let archive = Archive::open(&rom, info.region).unwrap();
    for kind in 0..12 {
        let entry_file = ssb_rom::fighter::FIGHTER_FILES[kind as usize];
        let main = archive.load(entry_file.file).unwrap();
        let fighter = ssb_rom::fighter::decode_file(entry_file, &main).unwrap();
        let mask = fighter.setup_parts;
        let entry = &ssb_rom::anim::FIGHTER_ANIMS[kind as usize];
        assert_eq!(
            fighter.attributes.cliff_air_mask,
            if matches!(kind, 0 | 3 | 4 | 11) { 2 } else { 0 }
        );
        for slot in ssb_rom::anim::SLOT_CLIFF_CATCH..ssb_rom::anim::SLOT_DAMAGE_HI1 {
            let file = archive.load(entry.files[slot] as u32).unwrap();
            let count = ssb_rom::anim::joint_table_len(&file.data).unwrap();
            assert!(ssb_rom::anim::LEADING_RUNTIME_JOINT[kind as usize][slot]);
            assert!(
                count as u32 > mask.count_ones(),
                "fighter {kind}, {}",
                ssb_rom::anim::SLOT_NAMES[slot]
            );
        }
    }
}

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
                motion: None,
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

/// RE-432: the simulated figatree walk reproduces the mask-order binding for
/// motions without hidden model parts, fits every motion that has them, and
/// ends HeavyGet on each fighter's `joint_itemheavy_id` descriptor.
#[test]
fn hidden_part_walk_matches_mask_order_and_reaches_the_item_heavy_joint() {
    let Ok(path) = std::env::var("SSB64_ROM") else {
        return;
    };
    let rom = std::fs::read(path).unwrap();
    let info = ssb_rom::rom::identify(&rom).unwrap();
    let archive = Archive::open(&rom, info.region).unwrap();
    let mut walked = 0;
    for kind in 0..12usize {
        let entry_file = ssb_rom::fighter::FIGHTER_FILES[kind];
        let main = archive.load(entry_file.file).unwrap();
        let mask = ssb_rom::fighter::setup_parts(&main, entry_file).unwrap();
        let common = ssb_rom::fighter::common_parts(&main, entry_file)[0].unwrap();
        let model = archive.load(common.model_file).unwrap();
        let graph = ssb_rom::scene::find_scene_graphs(&model)
            .into_iter()
            .find(|g| g.offset == common.graph)
            .unwrap();
        let depths: Vec<u32> = graph.nodes.iter().map(|n| n.desc.depth()).collect();
        let hidden: Vec<_> = (0..4)
            .map(|i| ssb_rom::fighter::hidden_part(&main, entry_file, i).unwrap())
            .collect();
        let plain = ssb_rom::fighter::figatree_order(&depths, mask, &hidden, 0).unwrap();
        let enabled: Vec<_> = (0..depths.len() as u32)
            .filter(|i| mask >> i & 1 != 0)
            .map(ssb_rom::fighter::TreeJoint::Desc)
            .collect();
        assert_eq!(plain, enabled, "fighter {kind}");
        let entry = &ssb_rom::anim::FIGHTER_ANIMS[kind];
        for (slot, &id) in entry.files.iter().enumerate() {
            let parts = ssb_rom::anim::HIDDEN_PARTS[kind][slot];
            if id == 0 || parts & 0x1FFF_FFE0 == 0 || ssb_rom::anim::is_anim_joint_slot(slot) {
                continue;
            }
            let all: Vec<_> = (0..27)
                .take_while(|&i| ssb_rom::fighter::hidden_part(&main, entry_file, i).is_some())
                .map(|i| ssb_rom::fighter::hidden_part(&main, entry_file, i).unwrap())
                .collect();
            let order = ssb_rom::fighter::figatree_order(&depths, mask, &all, parts)
                .unwrap_or_else(|| panic!("fighter {kind} slot {slot}"));
            let file = archive.load(id as u32).unwrap();
            let len = ssb_rom::anim::joint_table_len(&file.data).unwrap();
            assert!(
                len >= order.len(),
                "fighter {kind} {}",
                ssb_rom::anim::SLOT_NAMES[slot]
            );
            walked += 1;
        }
        let heavy = ssb_rom::fighter::figatree_order(
            &depths,
            mask,
            &hidden,
            ssb_rom::anim::HIDDEN_PARTS[kind][ssb_rom::anim::SLOT_LIGHT_THROW_DROP - 1],
        )
        .unwrap();
        assert_eq!(
            heavy.last(),
            Some(&ssb_rom::fighter::TreeJoint::Desc(
                hidden[3].root_joint_id - 4
            )),
            "fighter {kind}"
        );
    }
    assert!(walked > 100, "{walked}");
}
