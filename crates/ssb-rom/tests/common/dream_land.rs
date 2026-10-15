//! Dream Land for the host replays: its collision from the pack, a
//! Training fighter at a stage spawn, and Whispy's packed animations
//! (`scene_inputs`, `stage_controllers`).

#![allow(dead_code)]

use ssb_game::fighter::{Fighter, FighterKind};
use ssb_game::stage::pupupu::{EyesAnim, MouthAnim};
use ssb_game::stage::{StageAnim, StageObj, StageObjects};
use ssb_game::weapon::{MapSurface, MapSurfaceKind, SurfaceTopology};

/// Dream Land and its floor segments, `(line id, segment)`.
pub fn dream_land<'p>(
    pack: &ssb_rom::pack::Pack<'p>,
) -> (
    ssb_rom::pack::StageDesc,
    Vec<(u16, ssb_game::collision::Segment)>,
) {
    let stage = pack.stage(0).expect("Dream Land");
    let mut segments = Vec::new();
    for line in pack.stage_lines(&stage) {
        if line.kind != ssb_rom::pack::line_kind::FLOOR {
            continue;
        }
        let points: Vec<_> = pack.line_vertices(&line).collect();
        for pair in points.windows(2) {
            segments.push((
                line.id,
                ssb_game::collision::Segment {
                    x1: pair[0].x,
                    y1: pair[0].y,
                    x2: pair[1].x,
                    y2: pair[1].y,
                    flags: pair[0].flags,
                },
            ));
        }
    }
    (stage, segments)
}

/// Training's fighter on `port` at the stage's spawn `port`
/// (`FighterScene::at_position`), standing (or falling) from its first
/// battle frame.
pub fn training_fighter(
    pack: &ssb_rom::pack::Pack<'_>,
    stage: &ssb_rom::pack::StageDesc,
    kind: FighterKind,
    port: u8,
    floors: &[(u16, ssb_game::collision::Segment)],
) -> Fighter {
    let spawn = pack.spawn(stage, u16::from(port)).expect("spawn point");
    let mut f = Fighter::new(kind, port, 3);
    let d = pack.fighter(kind as u32).expect("fighter in pack");
    f.attributes = ssb_game::physics::PhysicsAttributes {
        size: d.size,
        traction: d.traction,
        dash_speed: d.dash_speed,
        dash_decel: d.dash_decel,
        run_speed: d.run_speed,
        walk_speed_mul: d.walk_speed_mul,
        jump_vel_x: d.jump_vel_x,
        jump_height_mul: d.jump_height_mul,
        jump_height_base: d.jump_height_base,
        jumpaerial_vel_x: d.jumpaerial_vel_x,
        jumpaerial_height: d.jumpaerial_height,
        air_accel: d.air_accel,
        air_speed_max_x: d.air_speed_max_x,
        air_friction: d.air_friction,
        gravity: d.gravity,
        tvel_base: d.tvel_base,
        tvel_fast: d.tvel_fast,
        jumps_max: d.jumps_max,
        weight: d.weight,
        kneebend_anim_length: d.kneebend_anim_length,
        dash_to_run: d.dash_to_run,
        walkslow_anim_length: d.walkslow_anim_length,
        walkmiddle_anim_length: d.walkmiddle_anim_length,
        walkfast_anim_length: d.walkfast_anim_length,
    };
    f.coll = ssb_game::ground::BodyColl {
        top: d.coll_top,
        center: d.coll_center,
        bottom: d.coll_bottom,
        width: d.coll_width,
    };
    f.cliff_reach = ssb_engine::math::Vec2::new(d.cliffcatch_width, d.cliffcatch_height);
    f.cliff_air_mask = d.cliff_air_mask;
    f.anim = ssb_game::status::AnimLengths {
        dash: d.dash_anim_length,
        turn: d.turn_anim_length,
        run_brake: d.runbrake_anim_length,
        squat: d.squat_anim_length,
        squat_rv: d.squatrv_anim_length,
        landing: d.landing_anim_length,
        pass: d.pass_anim_length,
    };
    f.jostle_width = d.jostle_width;
    f.jostle_x = d.jostle_x;
    f.pos = ssb_engine::math::Vec3::new(f32::from(spawn.x), f32::from(spawn.y), 0.0);
    f.facing = ssb_game::fighter::Facing::at_spawn_x(f.pos.x);
    f.topn_lr = f.facing.sign();
    f.init_floor(floors.iter().copied());
    ssb_game::status::set_wait_or_fall(&mut f);
    f
}

/// Dream Land's collision lines as the runtime's `FloorSegments` gives
/// them to the map: every kind, with its topology.
pub fn dream_land_map(
    pack: &ssb_rom::pack::Pack<'_>,
    stage: &ssb_rom::pack::StageDesc,
) -> Vec<MapSurface> {
    use ssb_rom::pack::line_kind;
    let mut out = Vec::new();
    for line in pack.stage_lines(stage) {
        let kind = match line.kind {
            line_kind::FLOOR => MapSurfaceKind::Floor,
            line_kind::CEILING => MapSurfaceKind::Ceiling,
            line_kind::RIGHT_WALL => MapSurfaceKind::RightWall,
            line_kind::LEFT_WALL => MapSurfaceKind::LeftWall,
            _ => continue,
        };
        let points: Vec<_> = pack.line_vertices(&line).collect();
        for (n, pair) in points.windows(2).enumerate() {
            out.push(MapSurface {
                kind,
                segment: ssb_game::collision::Segment {
                    x1: pair[0].x,
                    y1: pair[0].y,
                    x2: pair[1].x,
                    y2: pair[1].y,
                    flags: pair[0].flags,
                },
                topology: Some(SurfaceTopology {
                    line: line.id,
                    point: n as u16,
                    segments: line.vertex_count - 1,
                    vertex1: pair[0].vertex_id,
                    vertex2: pair[1].vertex_id,
                }),
                motion: None,
            });
        }
    }
    out
}

/// Whispy's eyes and mouth, then the back and front flowers: the order of
/// `gGRCommonStruct.pupupu.map_gobj`.
pub fn whispy_index(obj: StageObj) -> usize {
    match obj {
        StageObj::WhispyEyes => 0,
        StageObj::WhispyMouth => 1,
        StageObj::FlowersBack => 2,
        StageObj::FlowersFront => 3,
        _ => panic!("not a Dream Land object"),
    }
}

/// Whispy and the flowers played from the pack's ground objects.
pub struct PackWhispy<'a, 'p> {
    pub pack: &'a ssb_rom::pack::Pack<'p>,
    pub objects: ssb_rom::ground_obj::GroundObjects,
}

impl StageObjects for PackWhispy<'_, '_> {
    fn play(&mut self, anim: StageAnim) {
        use ssb_rom::ground_obj as g;
        let slot = match anim {
            StageAnim::WhispyEyes { lr, status } => g::whispy_eyes(lr, status == EyesAnim::Blink),
            StageAnim::WhispyMouth { lr, status } => g::whispy_mouth(
                lr,
                match status {
                    MouthAnim::Stretch => 0,
                    MouthAnim::Turn => 1,
                    MouthAnim::Open => 2,
                    MouthAnim::Close => 3,
                },
            ),
            StageAnim::FlowersBack { lr, phase } => g::flowers_back(lr, phase),
            StageAnim::FlowersFront { lr, phase } => g::flowers_front(lr, phase),
            _ => panic!("not a Dream Land animation"),
        };
        assert!(self.objects.has(slot));
        self.objects.play(self.pack, slot).unwrap();
    }

    fn anim_frame(&self, obj: StageObj) -> f32 {
        self.objects.get(whispy_index(obj) as u8).unwrap().frame
    }
}
