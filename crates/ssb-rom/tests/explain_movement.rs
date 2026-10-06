//! How to Play's input scripts (`scExplain`, RE-466) replayed through the
//! portable fighter against an N64 RDRAM trace of the unmodified US ROM.
//!
//! From the end of the entry (frame 119) to frame 700 the scripts walk,
//! dash, turn, run, brake, crouch, jump and land Mario and Luigi without
//! any attack, so the fighters' common movement alone decides where they
//! stand. Each row is a frame of the N64 trace (`gSCManagerBattleState`'s
//! two players, frame 0 the first of `nSCKindExplain`): status id, x and y
//! of each fighter, as measured; nothing here is ROM data.
//!
//! From frame 701 to 1964 the scripts attack: jabs, tilts, smashes, rolls
//! and aerials hit and launch. That replay poses both skeletons from the
//! built pack, places the hit and hurt collisions on them as
//! `gmCollisionGetFighterPartsWorldPosition` does (RE-468) and runs the
//! hit search and `ftMainProcParams`, then checks status, position and
//! damage against the N64.

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

/// Status id, x, y and damage of one fighter on one frame.
type HitRow = (u16, f32, f32, u16);

/// `(frame, [Mario, Luigi])` from the N64, 701 to 1964 (RE-468): status id,
/// x, y and damage, at every change of status or damage and every 30th
/// frame.
#[rustfmt::skip]
const N64_HITS: &[(u32, [HitRow; 2])] = &[
    (701, [(10, 271.55, -6.00, 0), (10, 602.55, -6.00, 0)]),
    (710, [(190, 271.55, -6.00, 0), (10, 602.55, -6.00, 0)]),
    (711, [(190, 271.55, -6.00, 0), (40, 602.55, -6.00, 2)]),
    (720, [(190, 271.55, -6.00, 0), (40, 678.45, -6.00, 2)]),
    (723, [(191, 271.55, -6.00, 0), (40, 715.59, -6.00, 2)]),
    (725, [(191, 271.55, -6.00, 0), (40, 736.85, -6.00, 4)]),
    (734, [(220, 271.55, -6.00, 0), (40, 588.38, -6.00, 4)]),
    (737, [(220, 271.55, -6.00, 0), (38, 591.70, -6.00, 8)]),
    (750, [(220, 258.05, -6.00, 0), (38, 669.18, -6.00, 8)]),
    (760, [(220, 258.05, -6.00, 0), (15, 838.28, -6.00, 8)]),
    (764, [(10, 258.05, -6.00, 0), (15, 706.32, -6.00, 8)]),
    (768, [(190, 258.05, -6.00, 0), (15, 579.96, -6.00, 8)]),
    (769, [(190, 258.05, -6.00, 0), (40, 553.62, -6.00, 10)]),
    (780, [(190, 258.05, -6.00, 0), (40, 662.82, -6.00, 10)]),
    (781, [(191, 258.05, -6.00, 0), (40, 675.62, -6.00, 10)]),
    (783, [(191, 258.05, -6.00, 0), (40, 699.12, -6.00, 12)]),
    (792, [(220, 258.05, -6.00, 0), (40, 722.34, 26.31, 12)]),
    (796, [(220, 258.05, -6.00, 0), (38, 730.45, -6.00, 16)]),
    (810, [(220, 258.05, -6.00, 0), (38, 749.04, 105.13, 16)]),
    (822, [(10, 258.05, -6.00, 0), (38, 831.39, -6.00, 16)]),
    (824, [(10, 258.05, -6.00, 0), (11, 827.68, -6.00, 16)]),
    (825, [(10, 258.05, -6.00, 0), (12, 819.88, -6.00, 16)]),
    (836, [(10, 258.05, -6.00, 0), (10, 691.58, -6.00, 16)]),
    (840, [(10, 258.05, -6.00, 0), (10, 679.88, -6.00, 16)]),
    (842, [(190, 258.05, -6.00, 0), (10, 679.88, -6.00, 16)]),
    (843, [(190, 258.05, -6.00, 0), (40, 679.88, -6.00, 18)]),
    (857, [(190, 258.05, -6.00, 0), (12, 825.48, -6.00, 18)]),
    (860, [(190, 258.05, -6.00, 0), (10, 828.34, -6.00, 18)]),
    (863, [(10, 258.05, -6.00, 0), (12, 833.40, -6.00, 18)]),
    (870, [(10, 258.05, -6.00, 0), (12, 761.34, -6.00, 18)]),
    (875, [(10, 258.05, -6.00, 0), (10, 688.32, -6.00, 18)]),
    (896, [(199, 258.05, -6.00, 0), (10, 661.52, -6.00, 18)]),
    (900, [(199, 392.45, -6.00, 0), (54, 661.52, -6.00, 28)]),
    (930, [(199, 258.05, -6.00, 0), (54, 728.71, 387.89, 28)]),
    (943, [(199, 258.05, -6.00, 0), (68, 738.94, -6.00, 28)]),
    (944, [(11, 261.05, -6.00, 0), (68, 738.94, -6.00, 28)]),
    (946, [(190, 265.55, -6.00, 0), (68, 738.94, -6.00, 28)]),
    (947, [(190, 265.55, -6.00, 0), (40, 738.94, -6.00, 29)]),
    (960, [(190, 265.55, -6.00, 0), (40, 896.98, -6.00, 29)]),
    (962, [(190, 265.55, -6.00, 0), (10, 924.40, -6.00, 29)]),
    (967, [(11, 270.65, -6.00, 0), (10, 980.70, -6.00, 29)]),
    (968, [(195, 270.65, -6.00, 0), (10, 989.86, -6.00, 29)]),
    (974, [(195, 270.65, -6.00, 0), (15, 980.12, -6.00, 29)]),
    (976, [(195, 270.65, -6.00, 0), (51, 887.94, -6.00, 42)]),
    (990, [(195, 270.65, -6.00, 0), (51, 1180.56, 218.04, 42)]),
    (1017, [(195, 270.65, -6.00, 0), (68, 1939.00, -6.00, 42)]),
    (1018, [(28, 270.65, -6.00, 0), (68, 1950.10, -6.00, 42)]),
    (1020, [(28, 270.65, -6.00, 0), (68, 1970.19, -6.00, 42)]),
    (1021, [(201, 309.46, -6.00, 0), (68, 1979.19, -6.00, 42)]),
    (1043, [(201, 242.93, -6.00, 0), (76, 2030.53, -6.00, 42)]),
    (1050, [(201, 267.05, -6.00, 0), (76, 1669.79, -6.00, 42)]),
    (1060, [(30, 272.05, -6.00, 0), (76, 1008.15, -6.00, 42)]),
    (1071, [(10, 272.05, -6.00, 0), (76, 856.54, -6.00, 42)]),
    (1078, [(10, 272.05, -6.00, 0), (10, 856.54, -6.00, 42)]),
    (1080, [(10, 272.05, -6.00, 0), (10, 856.54, -6.00, 42)]),
    (1090, [(207, 272.05, -6.00, 0), (10, 856.54, -6.00, 42)]),
    (1092, [(207, 272.05, -6.00, 0), (11, 853.24, -6.00, 42)]),
    (1097, [(207, 272.05, -6.00, 0), (12, 819.94, -6.00, 42)]),
    (1098, [(207, 272.05, -6.00, 0), (54, 769.94, -6.00, 61)]),
    (1110, [(207, 272.05, -6.00, 0), (54, 796.55, 291.85, 61)]),
    (1140, [(207, 272.05, -6.00, 0), (54, 1124.57, 3102.45, 61)]),
    (1159, [(10, 272.05, -6.00, 0), (54, 1263.35, 3890.67, 61)]),
    (1167, [(208, 272.05, -6.00, 0), (54, 1305.78, 4039.65, 61)]),
    (1170, [(208, 272.05, -6.00, 0), (54, 1319.25, 4067.57, 61)]),
    (1190, [(208, 272.05, -6.00, 0), (57, 1374.95, 3864.22, 61)]),
    (1200, [(208, 272.05, -6.00, 0), (57, 1344.98, 3254.71, 61)]),
    (1211, [(15, 326.05, -6.00, 0), (57, 1257.80, 2512.21, 61)]),
    (1213, [(156, 380.05, -6.00, 0), (57, 1243.90, 2377.21, 61)]),
    (1230, [(156, 1073.52, -6.00, 0), (57, 1149.98, 1229.71, 61)]),
    (1244, [(10, 1388.05, -6.00, 0), (57, 982.50, 284.71, 61)]),
    (1245, [(204, 1388.05, -6.00, 0), (57, 961.48, 217.21, 61)]),
    (1249, [(204, 1388.05, -6.00, 0), (68, 878.88, -6.00, 61)]),
    (1260, [(204, 1388.05, -6.00, 0), (52, 814.30, -6.00, 70)]),
    (1290, [(204, 1388.05, -6.00, 0), (52, -1232.45, 1302.51, 70)]),
    (1305, [(13, 1365.25, -6.00, 0), (52, -2210.08, 1568.34, 70)]),
    (1306, [(10, 1348.45, -6.00, 0), (52, -2265.23, 1576.87, 70)]),
    (1307, [(15, 1294.45, -6.00, 0), (52, -2319.13, 1584.26, 70)]),
    (1320, [(16, 680.85, -6.00, 0), (52, -2905.70, 1575.76, 70)]),
    (1333, [(17, 110.73, -6.00, 0), (52, -3280.46, 1373.16, 70)]),
    (1342, [(17, -184.02, -6.00, 0), (57, -3414.15, 1119.20, 70)]),
    (1350, [(17, -318.52, -6.00, 0), (57, -3375.02, 815.35, 70)]),
    (1356, [(10, -341.65, -6.00, 0), (57, -3252.42, 563.35, 70)]),
    (1357, [(10, -341.65, -6.00, 0), (24, -3229.92, 638.65, 70)]),
    (1380, [(10, -341.65, -6.00, 0), (24, -2715.87, 1790.95, 70)]),
    (1410, [(10, -341.65, -6.00, 0), (24, -2045.36, 1485.85, 70)]),
    (1417, [(10, -341.65, -6.00, 0), (27, -1888.91, 1013.35, 70)]),
    (1433, [(10, -341.65, -6.00, 0), (32, -1531.46, -6.00, 70)]),
    (1440, [(10, -341.65, -6.00, 0), (32, -1453.41, -6.00, 70)]),
    (1443, [(10, -341.65, -6.00, 0), (15, -1403.41, -6.00, 70)]),
    (1453, [(18, -341.65, -6.00, 0), (15, -945.41, -6.00, 70)]),
    (1456, [(18, -341.65, -6.00, 0), (16, -841.81, -6.00, 70)]),
    (1457, [(18, -341.65, -6.00, 0), (17, -802.69, -6.00, 70)]),
    (1464, [(10, -341.65, -6.00, 0), (17, -553.31, -6.00, 70)]),
    (1470, [(10, -301.15, -6.00, 0), (17, -414.19, -6.00, 70)]),
    (1471, [(199, -294.40, -6.00, 0), (17, -394.06, -6.00, 70)]),
    (1480, [(199, -122.28, -6.00, 0), (54, -252.31, -6.00, 80)]),
    (1500, [(199, -233.65, -6.00, 0), (54, -357.42, 1305.93, 80)]),
    (1519, [(10, -233.65, -6.00, 0), (54, -474.98, 2233.22, 80)]),
    (1530, [(10, -233.65, -6.00, 0), (54, -523.47, 2464.73, 80)]),
    (1555, [(10, -233.65, -6.00, 0), (57, -578.60, 2227.74, 80)]),
    (1560, [(10, -233.65, -6.00, 0), (57, -546.57, 2053.15, 80)]),
    (1579, [(199, -233.65, -6.00, 0), (57, -167.21, 772.95, 80)]),
    (1584, [(199, -99.25, -6.00, 0), (54, -55.46, 435.45, 88)]),
    (1590, [(199, -99.25, -6.00, 0), (54, -55.46, 435.45, 88)]),
    (1620, [(199, -233.65, -6.00, 0), (54, 139.27, 2359.15, 88)]),
    (1626, [(10, -233.65, -6.00, 0), (54, 165.40, 2480.93, 88)]),
    (1634, [(11, -230.05, -6.00, 0), (54, 193.61, 2548.35, 88)]),
    (1644, [(10, -197.65, -6.00, 0), (54, 218.20, 2479.99, 88)]),
    (1650, [(10, -197.65, -6.00, 0), (54, 227.26, 2357.57, 88)]),
    (1653, [(10, -197.65, -6.00, 0), (57, 230.19, 2273.47, 88)]),
    (1678, [(199, -197.65, -6.00, 0), (57, 232.88, 649.87, 88)]),
    (1680, [(199, -127.65, -6.00, 0), (57, 232.88, 514.87, 88)]),
    (1682, [(199, -63.25, -6.00, 0), (54, 232.88, 379.87, 96)]),
    (1710, [(199, -197.65, -6.00, 0), (54, 399.00, 2230.57, 96)]),
    (1725, [(12, -187.45, -6.00, 0), (54, 479.36, 2749.76, 96)]),
    (1740, [(12, -30.55, -6.00, 0), (54, 533.04, 2887.39, 96)]),
    (1755, [(10, 120.95, -6.00, 0), (54, 560.04, 2643.44, 96)]),
    (1756, [(10, 120.95, -6.00, 0), (57, 560.36, 2588.11, 96)]),
    (1770, [(10, 120.95, -6.00, 0), (57, 560.30, 1680.82, 96)]),
    (1786, [(199, 120.95, -6.00, 0), (57, 560.30, 600.82, 96)]),
    (1790, [(199, 255.35, -6.00, 0), (54, 560.30, 330.82, 104)]),
    (1800, [(199, 244.73, -6.00, 0), (54, 597.12, 836.34, 104)]),
    (1830, [(199, 120.95, -6.00, 0), (54, 812.77, 2912.30, 104)]),
    (1833, [(10, 120.95, -6.00, 0), (54, 828.46, 3010.76, 104)]),
    (1834, [(12, 130.25, -6.00, 0), (54, 833.46, 3040.18, 104)]),
    (1860, [(12, 487.55, -6.00, 0), (54, 921.69, 3210.00, 104)]),
    (1868, [(10, 591.95, -6.00, 0), (57, 932.82, 3006.11, 104)]),
    (1870, [(20, 593.75, -6.00, 0), (57, 934.39, 2893.56, 104)]),
    (1873, [(22, 594.25, 71.50, 0), (57, 935.85, 2712.01, 104)]),
    (1888, [(210, 609.10, 946.00, 0), (57, 927.32, 1705.19, 104)]),
    (1890, [(210, 622.00, 1021.80, 0), (57, 920.82, 1570.19, 104)]),
    (1898, [(210, 697.90, 1229.00, 0), (57, 900.82, 1030.19, 104)]),
    (1899, [(210, 706.70, 1244.10, 0), (55, 902.77, 1105.49, 120)]),
    (1920, [(210, 796.70, 1238.10, 0), (55, 2378.50, 2317.83, 120)]),
    (1936, [(26, 834.97, 710.50, 0), (55, 4067.63, 3279.77, 120)]),
    (1950, [(26, 625.15, 94.50, 0), (55, 5284.53, 3826.55, 120)]),
    (1953, [(31, 544.23, -6.00, 0), (55, 5513.59, 3914.15, 120)]),
    (1960, [(10, 497.13, -6.00, 0), (55, 6004.55, 4077.97, 120)]),
    (1964, [(10, 497.13, -6.00, 0), (55, 6257.74, 4146.08, 120)]),
];

/// `dSCExplainRandomSeed1` as the N64's knockback draw at frame 1899 finds
/// it (RE-468): the frame-1898 state (674066467, measured) one draw on, the
/// draw the jump's effect makes first. The replay runs no effects, so it
/// takes the generator here; `DamageFlyRoll` then follows from the ported
/// `ftCommonDamageInitDamageVars` draw (0.4795 < 0.5).
const N64_SEED_BEFORE_FLYROLL: (u32, i32) = (1899, -372_204_966);

/// Frame 736, Mario's `Attack13` kick on its first frame: the N64's
/// `attack_colls[0..2].pos_curr` and Luigi's joints' world positions
/// (`FTParts::mtx_translate` row 3). The collision matrices read the
/// `lbCommonSin` table; exact sine puts these 0.2 to 0.7 units off.
const N64_KICK: [Vec3; 2] = [
    Vec3::new(335.22, 246.07, 24.48),
    Vec3::new(325.11, 180.27, 35.86),
];
const N64_LUIGI_JOINTS: [(usize, Vec3); 4] = [
    (9, Vec3::new(587.53, 210.92, -0.32)),
    (12, Vec3::new(671.91, 257.50, -27.52)),
    (15, Vec3::new(781.61, 257.18, 1.01)),
    (25, Vec3::new(708.61, 73.47, -59.72)),
];

/// One fighter as `psp_runtime::scene::FighterScene` runs it: the
/// skeleton the gameplay joints are sampled from, with its clip state.
struct Posed {
    fighter: Fighter,
    skeleton: ssb_rom::skeleton::Skeleton,
    object: u32,
    started: Option<(ssb_game::status::AnyStatus, u32)>,
    root_before: Option<ssb_rom::figatree::JointPose>,
}

impl Posed {
    /// `tick_skeleton_animation`: a new status starts its clip on its
    /// current frame, then the clip advances one frame.
    fn tick_animation(&mut self, pack: &ssb_rom::pack::Pack<'_>) {
        let kind = self.fighter.kind as u32;
        let first_node = pack.object(self.object).map_or(0, |o| o.first_node);
        let status = self.fighter.status;
        let slot = status.status.anim_slot() as u32;
        let setter_played = status.anim_frame > status.anim_frame_begin;
        let speed = ssb_game::status::clip_speed(&self.fighter);
        let mut restarted = false;
        if self.started != Some((status.status, status.entry)) {
            self.started = Some((status.status, status.entry));
            restarted = !status.status.keeps_motion();
            if status.status.keeps_motion() {
                self.skeleton.speed = ssb_game::status::clip_speed(&self.fighter);
            } else if let Some(anim) = pack.fighter_anim(kind, slot) {
                let frame = if setter_played {
                    status.anim_frame - speed
                } else {
                    status.anim_frame
                };
                self.skeleton.start(pack, &anim, frame, speed);
                self.skeleton.lead_xrotn =
                    ssb_game::motion::leads_with_xrotn(self.fighter.kind, status.status);
            }
        }
        self.root_before = if restarted && setter_played {
            if let Some(anim) = pack.fighter_anim(kind, slot) {
                if let Some(script) = pack.anim_script(&anim) {
                    let _ = self.skeleton.tick_scaled(
                        script,
                        pack.fighter_translate_scales(kind),
                        first_node,
                    );
                }
            }
            self.skeleton.pose(0).copied()
        } else if restarted {
            self.skeleton.pose(0).map(|p| ssb_rom::figatree::JointPose {
                translate: [0.0; 3],
                ..*p
            })
        } else {
            self.skeleton.pose(0).copied()
        };
        if let Some(anim) = pack.fighter_anim(kind, slot) {
            if let Some(script) = pack.anim_script(&anim) {
                let _ = self.skeleton.tick_scaled(
                    script,
                    pack.fighter_translate_scales(kind),
                    first_node,
                );
            }
        }
    }

    /// TransN's step since the last parse, for the status's physics.
    fn set_root_motion(&mut self, rotate_z: Option<f32>) {
        if let (Some(before), Some(current)) = (self.root_before, self.skeleton.pose(0)) {
            let t = current.translate;
            self.fighter.set_root_motion(ssb_game::physics::RootMotion {
                delta: Vec3::new(
                    t[0] - before.translate[0],
                    t[1] - before.translate[1],
                    t[2] - before.translate[2],
                ),
                rotate_z: rotate_z.unwrap_or(current.rotate[2]),
                translate: Vec3::new(t[0], t[1], t[2]),
            });
        }
    }

    /// `sample_gameplay_joints`: TopN, then every model joint from the
    /// collision matrices.
    fn sample_joints(&mut self, pack: &ssb_rom::pack::Pack<'_>) {
        use ssb_game::fighter::{JointTransform, FIGHTER_JOINTS};
        let f = &mut self.fighter;
        f.joint_transforms = [None; FIGHTER_JOINTS];
        let size = f.attributes.size;
        let axes = ssb_game::item_throw::collision_axes(f).map(|a| a * size);
        let world = |v: Vec3| axes[0] * v.x + axes[1] * v.y + axes[2] * v.z;
        f.joint_transforms[0] = Some(JointTransform {
            axes,
            origin: f.pos,
        });
        let Some(object) = pack.object(self.object) else {
            return;
        };
        let mut posed = [ssb_rom::scene::Mat4::IDENTITY; ssb_rom::skeleton::MAX_NODES];
        let count = self
            .skeleton
            .compose_collision(pack, &object, &mut posed)
            .min(FIGHTER_JOINTS - 4);
        let scale = ssb_rom::pack::MODEL_SCALE;
        for (i, m) in posed[..count].iter().enumerate() {
            let axis = |c: usize| world(Vec3::new(m.0[c * 4], m.0[c * 4 + 1], m.0[c * 4 + 2]));
            let t = m.translation();
            f.joint_transforms[i + 4] = Some(JointTransform {
                axes: [axis(0), axis(1), axis(2)],
                origin: f.pos + world(Vec3::new(t[0] * scale, t[1] * scale, t[2] * scale)),
            });
        }
    }
}

#[test]
fn how_to_play_hits_as_the_n64_does() {
    let Some(path) = std::env::var_os("SSB64_ROM") else {
        return;
    };
    let pack_path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/generated/ssb64.pak");
    let Ok(pack_bytes) = std::fs::read(pack_path) else {
        return;
    };
    let pack = ssb_rom::pack::Pack::open(&pack_bytes).unwrap();
    let rom = std::fs::read(path).unwrap();
    let archive = Archive::open(&rom, ssb_rom::rom::identify(&rom).unwrap().region).unwrap();
    let keys =
        ssb_rom::explain::key_scripts(&archive.load(ssb_rom::explain::FILE_MAIN).unwrap()).unwrap();
    let (map, spawns) = surfaces(&archive);
    let map_fn = || map.iter().copied();
    ssb_game::rng::set_seed(ssb_game::explain::RANDOM_SEED);
    let mut fighters: Vec<Posed> = ssb_game::explain::FIGHTERS
        .iter()
        .enumerate()
        .map(|(port, &kind)| Posed {
            fighter: fighter(&archive, kind, port as u8, spawns[port], &map),
            skeleton: ssb_rom::skeleton::Skeleton::new(),
            object: ssb_rom::scene_deps::fighter_object(&pack, kind as u32).unwrap(),
            started: None,
            root_before: None,
        })
        .collect();
    let mut scripts: Vec<ssb_game::key::Key> = (0..2)
        .map(|i| {
            ssb_game::key::Key::new(
                ssb_game::key::parse(&keys, ssb_game::explain::KEY_EVENTS[i] as usize).unwrap(),
            )
        })
        .collect();
    let mut jump_held = [false; 2];
    let last = N64_HITS.last().unwrap().0;
    let mut rows = N64_HITS.iter().peekable();
    let mut checked = 0;
    for frame in 0..=last {
        if frame == N64_SEED_BEFORE_FLYROLL.0 {
            ssb_game::rng::set_seed(N64_SEED_BEFORE_FLYROLL.1);
        }
        // Priority 5: input, TransN's sample, the interrupt and jostle.
        for i in 0..2 {
            scripts[i].process();
            let c = scripts[i].controller();
            let held = c.buttons.0
                & (ssb_engine::input::N64Buttons::C_UP
                    | ssb_engine::input::N64Buttons::C_DOWN
                    | ssb_engine::input::N64Buttons::C_LEFT
                    | ssb_engine::input::N64Buttons::C_RIGHT)
                != 0;
            let p = &mut fighters[i];
            p.fighter
                .set_input(c, held && !jump_held[i], !held && jump_held[i]);
            jump_held[i] = held;
            p.set_root_motion(None);
            p.fighter.tick_interrupt(&map_fn);
            if !p.fighter.is_in_hitlag() {
                let other = JostleBody::of(&fighters[1 - i].fighter);
                ssb_game::fighter::jostle(&mut fighters[i].fighter, &[(other, i == 0)]);
            }
        }
        // Priority 4: the clip, the physics and map, the joint samples.
        for p in &mut fighters {
            if !p.fighter.is_in_hitlag() {
                p.tick_animation(&pack);
                let rotate_z = p.fighter.root_motion.rotate_z;
                p.set_root_motion(Some(rotate_z));
            } else {
                p.root_before = p.skeleton.pose(0).copied();
            }
            p.fighter.tick_physics_map_before_accessory(&map_fn);
            if !p.fighter.is_in_hitlag()
                && p.started != Some((p.fighter.status.status, p.fighter.status.entry))
            {
                p.tick_animation(&pack);
            }
            p.sample_joints(&pack);
            ssb_game::item_use::accessory(&mut p.fighter);
        }
        // Priorities 1 and 0: the hit search, then `ftMainProcParams`.
        {
            let [a, b] = &mut fighters[..] else {
                unreachable!()
            };
            let mut both = [&mut a.fighter, &mut b.fighter];
            ssb_game::combat::resolve_frame(&mut both);
        }
        if frame == 736 {
            let close = |a: Vec3, b: Vec3| (a - b).length() < 0.02;
            let kick = &fighters[0].fighter.attack_colls;
            for (i, n64) in N64_KICK.iter().enumerate() {
                assert!(
                    close(kick[i].pos_curr, *n64),
                    "kick {i}: {:?}",
                    kick[i].pos_curr
                );
            }
            for (joint, n64) in N64_LUIGI_JOINTS {
                let t = fighters[1].fighter.joint_transforms[joint].unwrap();
                assert!(close(t.origin, n64), "Luigi joint {joint}: {:?}", t.origin);
            }
            checked += 1;
        }
        while let Some(&&(at, expect)) = rows.peek() {
            if at != frame {
                break;
            }
            rows.next();
            for (port, (p, (status, x, y, damage))) in fighters.iter().zip(expect).enumerate() {
                let f = &p.fighter;
                assert_eq!(
                    (f.status.status.id(), f.damage),
                    (status, damage),
                    "frame {frame} port {port}: {:?} at ({:.2}, {:.2})",
                    f.status.status,
                    f.pos.x,
                    f.pos.y
                );
                assert!(
                    (f.pos.x - x).abs() < 0.05 && (f.pos.y - y).abs() < 0.05,
                    "frame {frame} port {port}: ({:.2}, {:.2}), N64 ({x}, {y})",
                    f.pos.x,
                    f.pos.y
                );
                checked += 1;
            }
        }
    }
    assert_eq!(checked, N64_HITS.len() * 2 + 1);
}
