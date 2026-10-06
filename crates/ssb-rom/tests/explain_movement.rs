//! How to Play's input scripts (`scExplain`, RE-466) replayed through the
//! portable fighter against an N64 RDRAM trace of the unmodified US ROM.
//!
//! From the end of the entry (frame 119) to frame 700 the scripts walk,
//! dash, turn, run, brake, crouch, jump and land Mario and Luigi without
//! any attack, so the fighters' common movement alone decides where they
//! stand. Each row is a frame of the N64 trace (`gSCManagerBattleState`'s
//! two players, frame 0 the first of `nSCKindExplain`): status id, x and y
//! of each fighter, as measured; nothing here is ROM data.

use ssb_engine::math::Vec3;
use ssb_game::collision::Segment;
use ssb_game::fighter::{Fighter, FighterKind, JostleBody};
use ssb_game::status::{AnimLengths, Status};
use ssb_game::weapon::{MapSurface, MapSurfaceKind, SurfaceTopology};
use ssb_rom::archive::Archive;
use ssb_rom::collision::LineKind;

/// One fighter on one frame: status id, x, y.
type Sample = (u16, f32, f32);

/// `(frame, [Mario, Luigi])` from the N64 (RE-466).
#[rustfmt::skip]
const N64: &[(u32, [Sample; 2])] = &[
    (119, [(10, 660.00, -6.00), (10, 1440.00, -6.00)]),
    (120, [(10, 660.00, -6.00), (10, 1440.00, -6.00)]),
    (130, [(12, 650.40, -6.00), (12, 1428.60, -6.00)]),
    (131, [(12, 639.60, -6.00), (15, 1378.60, -6.00)]),
    (133, [(13, 605.40, -6.00), (15, 1278.60, -6.00)]),
    (140, [(13, 447.90, -6.00), (15, 956.60, -6.00)]),
    (150, [(13, 222.90, -6.00), (15, 722.60, -6.00)]),
    (157, [(13, 65.40, -6.00), (12, 691.40, -6.00)]),
    (158, [(13, 42.90, -6.00), (12, 675.50, -6.00)]),
    (159, [(13, 20.40, -6.00), (12, 659.60, -6.00)]),
    (160, [(13, -2.10, -6.00), (13, 637.70, -6.00)]),
    (170, [(13, -227.10, -6.00), (13, 412.70, -6.00)]),
    (200, [(13, -902.10, -6.00), (13, -262.30, -6.00)]),
    (230, [(13, -1577.10, -6.00), (13, -937.30, -6.00)]),
    (254, [(18, -2111.10, -6.00), (18, -1474.50, -6.00)]),
    (255, [(18, -2121.60, -6.00), (18, -1491.40, -6.00)]),
    (256, [(18, -2126.10, -6.00), (18, -1505.50, -6.00)]),
    (258, [(18, -2126.10, -6.00), (18, -1525.30, -6.00)]),
    (259, [(15, -2072.10, -6.00), (15, -1475.30, -6.00)]),
    (260, [(15, -2018.10, -6.00), (15, -1425.30, -6.00)]),
    (265, [(15, -1750.90, -6.00), (15, -1178.10, -6.00)]),
    (272, [(16, -1458.50, -6.00), (16, -913.70, -6.00)]),
    (280, [(16, -1106.50, -6.00), (16, -593.70, -6.00)]),
    (295, [(17, -448.37, -6.00), (17, 5.42, -6.00)]),
    (300, [(17, -265.87, -6.00), (17, 187.92, -6.00)]),
    (310, [(17, -41.50, -6.00), (17, 487.30, -6.00)]),
    (318, [(10, 4.00, -6.00), (17, 663.80, -6.00)]),
    (320, [(10, 4.00, -6.00), (17, 699.17, -6.00)]),
    (327, [(10, 4.00, -6.00), (10, 793.50, -6.00)]),
    (333, [(10, 4.00, -6.00), (18, 804.30, -6.00)]),
    (338, [(10, 4.00, -6.00), (18, 804.30, -6.00)]),
    (343, [(10, 4.00, -6.00), (18, 804.30, -6.00)]),
    (344, [(10, 4.00, -6.00), (10, 804.30, -6.00)]),
    (360, [(10, 4.00, -6.00), (10, 804.30, -6.00)]),
    (403, [(10, 4.00, -6.00), (28, 804.30, -6.00)]),
    (404, [(10, 4.00, -6.00), (28, 804.30, -6.00)]),
    (409, [(10, 4.00, -6.00), (28, 804.30, -6.00)]),
    (410, [(10, 4.00, -6.00), (30, 804.30, -6.00)]),
    (415, [(10, 4.00, -6.00), (30, 804.30, -6.00)]),
    (420, [(10, 4.00, -6.00), (30, 804.30, -6.00)]),
    (421, [(10, 4.00, -6.00), (10, 804.30, -6.00)]),
    (424, [(20, 4.00, -6.00), (20, 804.30, -6.00)]),
    (426, [(20, 4.00, -6.00), (20, 804.30, -6.00)]),
    (427, [(22, 3.85, 73.60), (22, 805.55, 77.20)]),
    (428, [(22, 3.85, 150.80), (22, 806.65, 158.30)]),
    (440, [(22, 3.85, 890.00), (22, 810.15, 967.70)]),
    (460, [(22, 3.85, 1354.00), (22, 810.15, 1644.70)]),
    (467, [(26, 3.85, 1289.60), (26, 810.15, 1683.20)]),
    (480, [(26, 3.85, 862.80), (26, 810.15, 1481.70)]),
    (499, [(26, 3.85, 26.80), (26, 810.15, 335.80)]),
    (500, [(31, 3.85, -6.00), (26, 810.15, 268.30)]),
    (505, [(31, 3.85, -6.00), (32, 810.15, -6.00)]),
    (507, [(10, 3.85, -6.00), (32, 810.15, -6.00)]),
    (519, [(10, 3.85, -6.00), (10, 810.15, -6.00)]),
    (525, [(10, 3.85, -6.00), (10, 810.15, -6.00)]),
    (530, [(20, 3.85, -6.00), (20, 810.15, -6.00)]),
    (533, [(23, -21.45, 63.80), (23, 832.50, 66.00)]),
    (550, [(23, -192.82, 883.20), (23, 939.75, 968.70)]),
    (580, [(23, -224.32, 637.20), (23, 939.75, 1081.20)]),
    (581, [(24, -201.65, 708.60), (24, 917.25, 1156.50)]),
    (600, [(24, 165.85, 1609.20), (24, 583.05, 2188.20)]),
    (620, [(24, 237.35, 1621.20), (24, 606.00, 2455.20)]),
    (641, [(27, 271.55, 785.40), (27, 606.15, 1854.00)]),
    (650, [(27, 271.55, 389.40), (27, 602.55, 1272.00)]),
    (659, [(31, 271.55, -6.00), (27, 602.55, 664.50)]),
    (666, [(10, 271.55, -6.00), (27, 602.55, 192.00)]),
    (669, [(10, 271.55, -6.00), (32, 602.55, -6.00)]),
    (683, [(10, 271.55, -6.00), (10, 602.55, -6.00)]),
    (700, [(10, 271.55, -6.00), (10, 602.55, -6.00)]),
];

/// How to Play's stage (`llGRExplainMapFileID`): its `MPGroundData` is at 0
/// and `map_geometry` the word at 0x40 of it.
const EXPLAIN_MAP_FILE: u32 = 0x10B;

fn surfaces(archive: &Archive<'_>) -> (Vec<MapSurface>, Vec<[i16; 2]>) {
    let map = archive.load(EXPLAIN_MAP_FILE).unwrap();
    let geometry = map.extern_relocs.iter().find(|r| r.at == 0x40).unwrap();
    let file = archive.load(u32::from(geometry.target_file)).unwrap();
    let coll = ssb_rom::collision::read(&file, geometry.target_offset).unwrap();
    let mut out = Vec::new();
    for line in &coll.lines {
        let kind = match line.kind {
            LineKind::Floor => MapSurfaceKind::Floor,
            LineKind::Ceiling => MapSurfaceKind::Ceiling,
            LineKind::RightWall => MapSurfaceKind::RightWall,
            LineKind::LeftWall => MapSurfaceKind::LeftWall,
        };
        let segments = line.points.len().saturating_sub(1) as u16;
        for (i, w) in line.points.windows(2).enumerate() {
            out.push(MapSurface {
                kind,
                segment: Segment {
                    x1: w[0].pos[0],
                    y1: w[0].pos[1],
                    x2: w[1].pos[0],
                    y2: w[1].pos[1],
                    flags: w[0].flags,
                },
                topology: Some(SurfaceTopology {
                    line: line.id,
                    point: i as u16,
                    segments,
                    vertex1: w[0].vertex_id,
                    vertex2: w[1].vertex_id,
                }),
                motion: None,
            });
        }
    }
    let spawns = (0..2)
        .map(|p| coll.map_objects.iter().find(|o| o.kind == p).unwrap().pos)
        .collect();
    (out, spawns)
}

/// `ftManagerMakeFighter` for a How to Play fighter, from the ROM's
/// `FTAttributes` and the motion table's clip lengths.
fn fighter(
    archive: &Archive<'_>,
    kind: FighterKind,
    port: u8,
    spawn: [i16; 2],
    map: &[MapSurface],
) -> Fighter {
    let entry = ssb_rom::fighter::FIGHTER_FILES[kind as usize];
    let a = ssb_rom::fighter::decode_file(entry, &archive.load(entry.file).unwrap())
        .unwrap()
        .attributes;
    let mut f = Fighter::new(kind, port, 3);
    f.attributes = ssb_game::physics::PhysicsAttributes {
        size: a.size,
        traction: a.traction,
        dash_speed: a.dash_speed,
        dash_decel: a.dash_decel,
        run_speed: a.run_speed,
        walk_speed_mul: a.walk_speed_mul,
        jump_vel_x: a.jump_vel_x,
        jump_height_mul: a.jump_height_mul,
        jump_height_base: a.jump_height_base,
        jumpaerial_vel_x: a.jumpaerial_vel_x,
        jumpaerial_height: a.jumpaerial_height,
        air_accel: a.air_accel,
        air_speed_max_x: a.air_speed_max_x,
        air_friction: a.air_friction,
        gravity: a.gravity,
        tvel_base: a.tvel_base,
        tvel_fast: a.tvel_fast,
        jumps_max: a.jumps_max,
        weight: a.weight,
        kneebend_anim_length: a.kneebend_anim_length,
        dash_to_run: a.dash_to_run,
        walkslow_anim_length: a.walkslow_anim_length,
        walkmiddle_anim_length: a.walkmiddle_anim_length,
        walkfast_anim_length: a.walkfast_anim_length,
    };
    f.coll = ssb_game::ground::BodyColl {
        top: a.map_coll.top,
        center: a.map_coll.center,
        bottom: a.map_coll.bottom,
        width: a.map_coll.width,
    };
    let len = |s: Status| ssb_game::motion::anim_length(kind, s.into()).unwrap_or(0.0);
    f.anim = AnimLengths {
        dash: len(Status::Dash),
        turn: len(Status::Turn),
        run_brake: len(Status::RunBrake),
        squat: len(Status::Squat),
        squat_rv: len(Status::SquatRv),
        landing: len(Status::LandingLight),
        pass: len(Status::Pass),
    };
    f.jostle_width = a.jostle_width;
    f.jostle_x = a.jostle_x;
    f.pos = Vec3::new(f32::from(spawn[0]), f32::from(spawn[1]), 0.0);
    f.facing = ssb_game::fighter::Facing::at_spawn_x(f.pos.x);
    let floors = map
        .iter()
        .filter(|s| s.kind == MapSurfaceKind::Floor)
        .map(|s| (s.topology.unwrap().line, s.segment));
    assert!(f.init_floor(floors));
    // `scExplainStartBattle`.
    ssb_game::appear::appear_set_status(&mut f);
    f.interface.control_disable = false;
    f
}

#[test]
fn how_to_play_moves_the_fighters_as_the_n64_does() {
    let Some(path) = std::env::var_os("SSB64_ROM") else {
        return;
    };
    let rom = std::fs::read(path).unwrap();
    let archive = Archive::open(&rom, ssb_rom::rom::identify(&rom).unwrap().region).unwrap();
    let keys =
        ssb_rom::explain::key_scripts(&archive.load(ssb_rom::explain::FILE_MAIN).unwrap()).unwrap();
    let (map, spawns) = surfaces(&archive);
    let map_fn = || map.iter().copied();
    let mut fighters: Vec<Fighter> = ssb_game::explain::FIGHTERS
        .iter()
        .enumerate()
        .map(|(port, &kind)| fighter(&archive, kind, port as u8, spawns[port], &map))
        .collect();
    let mut scripts: Vec<ssb_game::key::Key> = (0..2)
        .map(|i| {
            ssb_game::key::Key::new(
                ssb_game::key::parse(&keys, ssb_game::explain::KEY_EVENTS[i] as usize).unwrap(),
            )
        })
        .collect();
    let mut jump_held = [false; 2];
    let last = N64.last().unwrap().0;
    let mut rows = N64.iter().peekable();
    let mut checked = 0;
    for frame in 0..=last {
        // Priority 5: each fighter's script, interrupt and jostle in link
        // order; priority 4: each fighter's physics and map.
        for i in 0..2 {
            scripts[i].process();
            let c = scripts[i].controller();
            let held = c.buttons.0
                & (ssb_engine::input::N64Buttons::C_UP
                    | ssb_engine::input::N64Buttons::C_DOWN
                    | ssb_engine::input::N64Buttons::C_LEFT
                    | ssb_engine::input::N64Buttons::C_RIGHT)
                != 0;
            let f = &mut fighters[i];
            f.set_input(c, held && !jump_held[i], !held && jump_held[i]);
            jump_held[i] = held;
            f.tick_interrupt(&map_fn);
            if !f.is_in_hitlag() {
                let other = JostleBody::of(&fighters[1 - i]);
                ssb_game::fighter::jostle(&mut fighters[i], &[(other, i == 0)]);
            }
        }
        for f in &mut fighters {
            f.tick_physics_map(&map_fn);
        }
        while let Some(&&(at, expect)) = rows.peek() {
            if at != frame {
                break;
            }
            rows.next();
            for (port, (f, (status, x, y))) in fighters.iter().zip(expect).enumerate() {
                assert_eq!(
                    f.status.status.id(),
                    status,
                    "frame {frame} port {port}: {:?} at ({:.2}, {:.2})",
                    f.status.status,
                    f.pos.x,
                    f.pos.y
                );
                assert!(
                    (f.pos.x - x).abs() < 0.01 && (f.pos.y - y).abs() < 0.01,
                    "frame {frame} port {port}: ({:.2}, {:.2}), N64 ({x}, {y})",
                    f.pos.x,
                    f.pos.y
                );
                checked += 1;
            }
        }
    }
    assert_eq!(checked, N64.len() * 2);
}
