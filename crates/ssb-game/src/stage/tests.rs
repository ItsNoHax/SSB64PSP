use super::*;
use crate::fighter::{FighterKind, Situation};
use crate::ground::Standing;
use crate::hazard;
use crate::status::{AnyStatus, Status};
use crate::weapon::{MapSurfaceKind, SurfaceTopology};
use ssb_engine::math::Vec2;

/// A clock per object: `play` restarts it at frame 0 and each `advance`
/// counts up until the clip's length, where `anim_frame` drops to zero and
/// then far below, as `gcParseDObjAnimJoint` leaves it after `End`.
#[derive(Default)]
struct Clocks {
    len: f32,
    frames: Vec<(StageObj, f32, f32)>,
    played: Vec<StageAnim>,
    barrel: Vec3,
    /// Each object's root translation as the controller last wrote it.
    placed: Vec<(StageObj, Vec3)>,
}

impl Clocks {
    fn placed(&self, obj: StageObj) -> Option<Vec3> {
        self.placed.iter().find(|(o, _)| *o == obj).map(|(_, t)| *t)
    }
    fn place(&mut self, obj: StageObj) -> &mut Vec3 {
        if self.placed(obj).is_none() {
            self.placed.push((obj, Vec3::ZERO));
        }
        &mut self.placed.iter_mut().find(|(o, _)| *o == obj).unwrap().1
    }
}

fn obj_of(anim: StageAnim) -> StageObj {
    match anim {
        StageAnim::WhispyEyes { .. } => StageObj::WhispyEyes,
        StageAnim::WhispyMouth { .. } => StageObj::WhispyMouth,
        StageAnim::FlowersBack { .. } => StageObj::FlowersBack,
        StageAnim::FlowersFront { .. } => StageObj::FlowersFront,
        StageAnim::CloudSolid(i) | StageAnim::CloudEvaporate(i) => StageObj::Cloud(i),
        StageAnim::TaruCannDefault | StageAnim::TaruCannFill | StageAnim::TaruCannShoot => {
            StageObj::TaruCann
        }
        StageAnim::ScaleRetract(i) => StageObj::Scale(i),
        StageAnim::GateOpen | StageAnim::GateClose => StageObj::Gate,
        StageAnim::CastleGround => StageObj::CastleGround,
        StageAnim::Acid => StageObj::Acid,
    }
}

impl Clocks {
    fn with_len(len: f32) -> Self {
        Clocks {
            len,
            ..Default::default()
        }
    }
    fn advance(&mut self) {
        for (_, frame, left) in self.frames.iter_mut() {
            if *left > 0.0 {
                *left -= 1.0;
                *frame = if *left > 0.0 { *frame + 1.0 } else { 0.0 };
            } else {
                *frame = -1.0e30;
            }
        }
    }
}

impl StageObjects for Clocks {
    fn play(&mut self, anim: StageAnim) {
        let obj = obj_of(anim);
        self.frames.retain(|(o, _, _)| *o != obj);
        self.frames.push((obj, 0.0, self.len));
        self.played.push(anim);
    }
    fn anim_frame(&self, obj: StageObj) -> f32 {
        self.frames
            .iter()
            .find(|(o, _, _)| *o == obj)
            .map_or(0.0, |(_, f, _)| *f)
    }
    fn mat_anim_idle(&self, obj: StageObj) -> bool {
        self.frames
            .iter()
            .find(|(o, _, _)| *o == obj)
            .is_none_or(|(_, _, left)| *left <= 0.0)
    }
    fn translate(&self, _obj: StageObj) -> Vec3 {
        self.barrel
    }
    fn set_translate(&mut self, obj: StageObj, pos: Vec3) {
        *self.place(obj) = pos;
    }
    fn set_translate_y(&mut self, obj: StageObj, y: f32) {
        self.place(obj).y = y;
    }
}

fn floor(line: u16, x1: i16, x2: i16, y: i16, flags: u16) -> MapSurface {
    MapSurface {
        kind: MapSurfaceKind::Floor,
        segment: crate::collision::Segment {
            x1,
            y1: y,
            x2,
            y2: y,
            flags,
        },
        topology: Some(SurfaceTopology {
            line,
            point: 0,
            segments: 1,
            vertex1: line * 2,
            vertex2: line * 2 + 1,
        }),
        motion: None,
    }
}

fn standing(kind: FighterKind, port: u8, pos: Vec3, line: u16, flags: u16) -> Fighter {
    let mut f = Fighter::new(kind, port, 3);
    f.pos = pos;
    f.situation = Situation::Ground;
    f.status.status = AnyStatus::Common(Status::Wait);
    f.floor = Some(Standing {
        line,
        flags,
        normal: Vec2::new(0.0, 1.0),
    });
    f
}

fn airborne(pos: Vec3) -> Fighter {
    let mut f = Fighter::new(FighterKind::Mario, 0, 3);
    f.pos = pos;
    f.status.status = AnyStatus::Common(Status::Fall);
    f
}

fn no_group(_: u16) -> Option<u8> {
    None
}

fn query<'a>(
    surfaces: &'a [MapSurface],
    line_group: &'a dyn Fn(u16) -> Option<u8>,
) -> MapQuery<'a, impl Fn() -> core::iter::Copied<core::slice::Iter<'a, MapSurface>> + 'a> {
    MapQuery {
        surfaces: move || surfaces.iter().copied(),
        line_group,
    }
}

fn init(kind: StageKind, objects: &[MapObject]) -> StageInit<'_> {
    StageInit {
        kind,
        map_objects: objects,
        bound_bottom: -3000.0,
        hazard_attack: None,
        hazard_throw: None,
        acid_surface_y: 0.0,
    }
}

// ---------------------------------------------------------------------------

#[test]
fn registry_walks_only_the_first_num_slots() {
    let mut r = Registry::default();
    assert!(r.add_obstacle(Obstacle::Twister));
    assert!(r.add_obstacle(Obstacle::TaruCann));
    assert!(!r.add_obstacle(Obstacle::Twister), "two slots");
    r.clear_obstacle(Obstacle::Twister);
    // `sFTMainGroundObstaclesNum` is 1, so slot 1 is no longer visited.
    assert_eq!(r.obstacles().count(), 0);
    assert!(r.add_obstacle(Obstacle::Twister));
    assert_eq!(r.obstacles().count(), 2);
}

#[test]
fn whispy_turns_opens_and_blows_the_fighters_away() {
    let mut w = pupupu::Pupupu::new();
    w.status = pupupu::WindStatus::Wait;
    w.wind_wait = 0;
    w.blink_wait = 30_000;
    let mut clocks = Clocks::with_len(10.0);
    // Two fighters on the left: Whispy turns to face them (lr 0).
    let mut a = airborne(Vec3::new(-1000.0, 0.0, 0.0));
    let mut b = airborne(Vec3::new(-2400.0, 0.0, 0.0));
    let mut frames = 0;
    while !w.is_blowing() {
        clocks.advance();
        w.tick(&mut [&mut a, &mut b], &mut clocks, true);
        frames += 1;
        assert!(frames < 200, "the wind never started");
    }
    assert_eq!(w.lr_players, 0);
    assert!(clocks.played.contains(&StageAnim::WhispyMouth {
        lr: 0,
        status: pupupu::MouthAnim::Turn
    }));
    assert!(clocks.played.contains(&StageAnim::WhispyMouth {
        lr: 0,
        status: pupupu::MouthAnim::Open
    }));
    // Turn (10) then Open (10); the front flowers' 22-frame lead starts on
    // the frame the wind does, then their 10-frame start clip.
    assert_eq!(frames, 1 + 10 + 10 + 21 + 10);
    // The loop pushes from the next frame.
    assert_eq!(a.hazard.vel_push, Vec3::ZERO);
    clocks.advance();
    w.tick(&mut [&mut a, &mut b], &mut clocks, true);
    // 6 - 475 * 0.0006 leftward; the second fighter is outside the box.
    let expect = -(pupupu::WIND_VEL_BASE - 475.0 * pupupu::WIND_DIST_DECAY);
    assert!((a.hazard.vel_push.x - expect).abs() < 1e-5);
    assert_eq!(b.hazard.vel_push, Vec3::ZERO);
}

#[test]
fn whispy_wind_stops_after_its_duration_and_the_loop_end() {
    let mut w = pupupu::Pupupu::new();
    w.status = pupupu::WindStatus::Blow;
    w.wind_duration = 1;
    w.flowers_front_status = pupupu::FlowerStatus::WindLoop;
    w.flowers_front_wait = 22;
    w.blink_wait = 30_000;
    let mut clocks = Clocks::with_len(5.0);
    let mut f = airborne(Vec3::new(0.0, 100.0, 0.0));
    w.tick(&mut [&mut f], &mut clocks, true);
    assert_eq!(w.status, pupupu::WindStatus::Stop);
    assert_eq!(w.flowers_front_status, pupupu::FlowerStatus::WindLoopEnd);
    // The loop end pushes on the frame it starts and 20 more.
    assert!(f.hazard.vel_push.x > 0.0);
    for _ in 0..20 {
        f.hazard.vel_push = Vec3::ZERO;
        clocks.advance();
        w.tick(&mut [&mut f], &mut clocks, true);
        assert!(f.hazard.vel_push.x > 0.0);
    }
    f.hazard.vel_push = Vec3::ZERO;
    w.tick(&mut [&mut f], &mut clocks, true);
    assert_eq!(f.hazard.vel_push, Vec3::ZERO);
    assert_eq!(w.flowers_front_status, pupupu::FlowerStatus::WindStop);
}

#[test]
fn acid_rises_to_its_next_level_and_burns() {
    let objects = [];
    let mut i = init(StageKind::Zebes, &objects);
    i.hazard_attack = Some(hazard::GroundAttack::from_words([0, 10, 90, 100, 0, 60, 1]));
    i.acid_surface_y = 100.0;
    let mut stage = Stage::new(&i, &mut [], &mut NoObjects, &mut NoItems);
    let Controller::Zebes(z) = &mut stage.controller else {
        panic!()
    };
    assert_eq!(z.level, -3000.0);
    z.status = zebes::AcidStatus::Normal;
    z.wait = 1;
    z.tick(true, &mut NoObjects);
    assert_eq!(z.status, zebes::AcidStatus::Shake);
    for _ in 0..18 {
        z.tick(true, &mut NoObjects);
    }
    assert_eq!(z.status, zebes::AcidStatus::Rise);
    let target = z.level + z.level_step * 240.0;
    for _ in 0..240 {
        z.tick(true, &mut NoObjects);
    }
    assert_eq!(z.status, zebes::AcidStatus::Normal);
    assert!((z.level - target).abs() < 0.5);
    assert!(z.level >= -3600.0 && z.level <= -3600.0 + 250.0 + 1.0);

    let level = z.level;
    let mut f = airborne(Vec3::new(0.0, level + 50.0, 0.0));
    hazard::search_ground_hit(&mut f, &stage);
    assert_eq!(f.hits.log_len, 1);
    assert_eq!(f.hazard.acid_wait, hazard::ACID_WAIT);
    let entry = f.hits.log[0].unwrap();
    assert_eq!(entry.attack_handicap, hazard::GROUND_HANDICAP);
    assert_eq!(entry.hitbox.angle, 90);
    // `acid_wait` blocks the next frame's contact.
    let before = f.hits.log_len;
    hazard::search_ground_hit(&mut f, &stage);
    assert_eq!(f.hits.log_len, before);
    // Above the surface is safe.
    let mut g = airborne(Vec3::new(0.0, level + 150.0, 0.0));
    hazard::search_ground_hit(&mut g, &stage);
    assert_eq!(g.hits.log_len, 0);
}

/// The acid object the source writes and reads: root Y is the level, the
/// surface is the root plus the child's animated Y.
#[derive(Default)]
struct AcidObject {
    played: Vec<StageAnim>,
    root_y: Vec<f32>,
    child_y: f32,
}

impl StageObjects for AcidObject {
    fn play(&mut self, anim: StageAnim) {
        self.played.push(anim);
    }
    fn set_translate_y(&mut self, obj: StageObj, y: f32) {
        assert_eq!(obj, StageObj::Acid);
        self.root_y.push(y);
    }
    fn child_translate(&self, obj: StageObj) -> Option<Vec3> {
        (obj == StageObj::Acid).then(|| Vec3::new(0.0, self.child_y, 0.0))
    }
}

#[test]
fn acid_writes_its_root_and_reads_its_animated_surface() {
    let objects = [];
    let mut i = init(StageKind::Zebes, &objects);
    i.hazard_attack = Some(hazard::GroundAttack::from_words([0, 10, 90, 100, 0, 60, 1]));
    i.acid_surface_y = 100.0;
    let mut acid = AcidObject {
        child_y: -270.0,
        ..Default::default()
    };
    let mut stage = Stage::new(&i, &mut [], &mut acid, &mut NoItems);
    assert_eq!(acid.played, [StageAnim::Acid]);
    assert_eq!(acid.root_y, [-3000.0]);
    let Controller::Zebes(z) = &mut stage.controller else {
        panic!()
    };
    assert_eq!(z.surface_y, -270.0);
    // Waiting and Normal leave the root alone; the child still moves.
    acid.child_y = 180.0;
    z.tick(false, &mut acid);
    assert_eq!(z.surface_y, 180.0);
    z.status = zebes::AcidStatus::Normal;
    z.wait = 1;
    for _ in 0..19 {
        z.tick(true, &mut acid);
    }
    assert_eq!(z.status, zebes::AcidStatus::Rise);
    assert_eq!(acid.root_y.len(), 1);
    // Each Rise frame writes the new level.
    z.tick(true, &mut acid);
    assert_eq!(acid.root_y, [-3000.0, z.level]);

    let (level, surface) = (z.level, z.surface_y);
    let mut f = airborne(Vec3::new(0.0, level + surface - 1.0, 0.0));
    hazard::search_ground_hit(&mut f, &stage);
    assert_eq!(f.hits.log_len, 1);
    let mut g = airborne(Vec3::new(0.0, level + surface + 1.0, 0.0));
    hazard::search_ground_hit(&mut g, &stage);
    assert_eq!(g.hits.log_len, 0);
}

#[test]
fn twister_rides_its_floor_catches_and_throws() {
    let surfaces = [floor(0, -2000, 2000, 0, 0)];
    let objects = [MapObject {
        kind: mapobj::TWISTER,
        pos: Vec3::new(0.0, 500.0, 0.0),
    }];
    let mut i = init(StageKind::Hyrule, &objects);
    i.hazard_throw = Some(hazard::HazardThrow::from_words([0, 15, 80, 70, 0, 60, 0]));
    let mut stage = Stage::new(&i, &mut [], &mut NoObjects, &mut NoItems);
    let map = query(&surfaces, &no_group);
    let mut f = standing(FighterKind::Mario, 0, Vec3::new(1200.0, 0.0, 0.0), 0, 0);
    {
        let Controller::Hyrule(h) = &mut stage.controller else {
            panic!()
        };
        h.status = hyrule::TwisterStatus::Wait;
        h.wait = 1;
        h.tick(&[&mut f], &mut stage.registry, &map, true);
        assert_eq!(h.status, hyrule::TwisterStatus::Summon);
        assert_eq!(
            h.twister_pos,
            Vec3::new(0.0, 0.0, 0.0),
            "projected onto the floor"
        );
        assert_eq!((h.left_edge_x, h.right_edge_x), (-2000.0, 2000.0));
        h.wait = 1;
        h.tick(&[&mut f], &mut stage.registry, &map, true);
        assert_eq!(h.status, hyrule::TwisterStatus::Move);
        assert_eq!(stage.registry.obstacles().count(), 1);
        h.twister_pos.x = 1100.0;
    }
    hazard::search_hit_hazard(&mut f, &mut stage, &mut NoObjects, &[]);
    assert_eq!(f.status.status, AnyStatus::Common(Status::Twister));
    assert!(!f.is_grounded());
    let surfaces_fn = || surfaces.iter().copied();
    for _ in 0..hazard::TWISTER_RELEASE_WAIT {
        f.tick_interrupt(&surfaces_fn);
        stage.tick(
            &mut [&mut f],
            TickInput {
                groups: &mut [],
                objects: &mut NoObjects,
                items: &mut NoItems,
                map: query(&surfaces, &no_group),
                started: true,
            },
        );
        f.tick_physics_map(&surfaces_fn);
    }
    assert_ne!(f.status.status, AnyStatus::Common(Status::Twister));
    assert_eq!(f.damage, 15);
    assert_eq!(f.hazard.twister_wait, hazard::TWISTER_PICKUP_WAIT);
    assert_eq!(f.pos.z, 0.0);
}

#[test]
fn clouds_sink_under_weight_then_evaporate_and_return() {
    let mut groups = vec![MapGroup::default(); 4];
    groups[1].translate = Vec3::new(-1000.0, 800.0, 0.0);
    let mut clocks = Clocks::with_len(1.0);
    let mut y = yoster::Yoster::new(&mut groups, &mut clocks);
    assert_eq!(groups[1].status, crate::map::GroupStatus::On);
    // Each cloud object starts at its group (RE-365).
    assert_eq!(clocks.placed(StageObj::Cloud(0)), Some(groups[1].translate));
    let line_group = |line: u16| (line == 7).then_some(1u8);
    let surfaces: [MapSurface; 0] = [];
    let map = query(&surfaces, &line_group);
    let mut f = standing(FighterKind::Mario, 0, Vec3::new(-1000.0, 800.0, 0.0), 7, 0);
    let mut frames = 0;
    while y.clouds[0].status == yoster::CloudStatus::Solid {
        clocks.advance();
        y.tick(&[&mut f], &mut groups, &mut clocks, &map);
        frames += 1;
        assert!(frames < 400);
    }
    // The timer is armed and first counted on the same frame, so it reads
    // zero on frame 120 and the cloud gives way on the next.
    assert_eq!(frames, 120 + 1);
    assert_eq!(y.clouds[0].pressure, 180.0);
    assert_eq!(groups[1].translate.y, 800.0 - 180.0);
    assert_eq!(clocks.placed(StageObj::Cloud(0)), Some(groups[1].translate));
    clocks.advance();
    y.tick(&[&mut f], &mut groups, &mut clocks, &map);
    assert_eq!(groups[1].status, crate::map::GroupStatus::Off);
    for _ in 0..180 {
        y.tick(&[], &mut groups, &mut clocks, &map);
    }
    assert_eq!(y.clouds[0].status, yoster::CloudStatus::Solid);
    assert_eq!(y.clouds[0].pressure, 0.0);
    assert!(clocks.played.contains(&StageAnim::CloudEvaporate(0)));
}

#[test]
fn scales_tip_toward_the_weight_and_fall_past_the_limit() {
    let objects = [
        MapObject {
            kind: mapobj::SCALE_L,
            pos: Vec3::new(-800.0, 1000.0, 0.0),
        },
        MapObject {
            kind: mapobj::SCALE_R,
            pos: Vec3::new(800.0, 1000.0, 0.0),
        },
    ];
    let mut groups = vec![MapGroup::default(); 3];
    let i = init(StageKind::Inishie, &objects);
    let mut stage = Stage::new(&i, &mut groups, &mut NoObjects, &mut NoItems);
    let line_group = |line: u16| (line == 1).then_some(1u8);
    let surfaces: [MapSurface; 0] = [];
    let map = query(&surfaces, &line_group);
    let mut f = standing(FighterKind::Donkey, 0, Vec3::new(-800.0, 1000.0, 0.0), 1, 0);
    f.attributes.weight = 1.0;
    let Controller::Inishie(s) = &mut stage.controller else {
        panic!()
    };
    let mut frames = 0;
    while s.status == inishie::ScaleStatus::Wait {
        s.tick(
            &[&mut f],
            &mut groups,
            &mut NoObjects,
            &mut NoItems,
            &map,
            true,
            &mut stage.registry,
        );
        frames += 1;
        assert!(frames < 2000);
        if s.status == inishie::ScaleStatus::Wait {
            assert!(s.platform[0].y <= 1000.0 && s.platform[1].y >= 1000.0);
        }
    }
    assert_eq!(s.status, inishie::ScaleStatus::Fall);
    assert_eq!(s.alt, -inishie::SCALE_ALT_MAX);
    for _ in 0..200 {
        s.tick(
            &[],
            &mut groups,
            &mut NoObjects,
            &mut NoItems,
            &map,
            true,
            &mut stage.registry,
        );
        if s.status != inishie::ScaleStatus::Fall {
            break;
        }
    }
    assert_eq!(s.status, inishie::ScaleStatus::Sleep);
    assert_eq!(groups[1].status, crate::map::GroupStatus::Off);
    assert_eq!(groups[2].status, crate::map::GroupStatus::Off);
    // The POW spawner found no position and no item: it re-arms.
    assert_eq!(s.pblock_status, inishie::PowerBlockStatus::Make);
}

#[test]
fn gate_opens_on_the_first_frame_and_closes_without_a_monster() {
    let mut groups = vec![MapGroup::default(); 4];
    groups[3].translate = Vec3::new(960.0, 240.0, 0.0);
    let mut y = yamabuki::Yamabuki::new(&mut groups, &mut NoObjects, &mut NoItems);
    y.tick(&[], &mut groups, &mut NoObjects, &mut NoItems, true);
    assert_eq!(y.status, yamabuki::GateStatus::Wait);
    y.tick(&[], &mut groups, &mut NoObjects, &mut NoItems, true);
    assert_eq!(y.gate_wait, 0);
    assert_eq!(groups[3].translate.x, yamabuki::GATE_FAR_X);
    // A fighter on the detect floor calls the monster; none can be made.
    let mut f = standing(
        FighterKind::Mario,
        0,
        Vec3::ZERO,
        0,
        hazard::material::DETECT,
    );
    y.tick(&[&mut f], &mut groups, &mut NoObjects, &mut NoItems, true);
    assert_eq!(y.status, yamabuki::GateStatus::Open);
    y.tick(&[&mut f], &mut groups, &mut NoObjects, &mut NoItems, true);
    assert_eq!(y.status, yamabuki::GateStatus::Wait);
    assert_eq!(y.gate_wait, 1000);
    assert_eq!(y.gate_pos.x, yamabuki::GATE_NEAR_X);
}

#[test]
fn barrel_captures_and_fires_on_a_button_press() {
    let objects = [];
    let mut i = init(StageKind::Jungle, &objects);
    i.hazard_throw = Some(hazard::HazardThrow::from_words([0, 0, 0, 100, 50, 60, 0]));
    let mut clocks = Clocks::with_len(3.0);
    clocks.barrel = Vec3::new(-2000.0, -800.0, 0.0);
    let mut stage = Stage::new(&i, &mut [], &mut clocks, &mut NoItems);
    assert!(clocks.played.contains(&StageAnim::TaruCannDefault));
    let mut f = airborne(Vec3::new(-2100.0, -700.0, 0.0));
    hazard::search_hit_hazard(&mut f, &mut stage, &mut clocks, &[]);
    assert_eq!(f.status.status, AnyStatus::Common(Status::TaruCann));
    assert!(f.is_invisible);
    assert!(clocks.played.contains(&StageAnim::TaruCannFill));
    // A second fighter cannot enter an occupied barrel.
    let mut g = airborne(Vec3::new(-2000.0, -800.0, 0.0));
    hazard::search_hit_hazard(&mut g, &mut stage, &mut clocks, &[f.status.status]);
    assert_eq!(g.status.status, AnyStatus::Common(Status::Fall));

    let surfaces: [MapSurface; 0] = [];
    let surfaces_fn = || surfaces.iter().copied();
    let press = ssb_engine::input::ControllerState {
        buttons: ssb_engine::input::N64Buttons(ssb_engine::input::N64Buttons::A),
        ..Default::default()
    };
    f.set_input(press, false, false);
    f.tick_interrupt(&surfaces_fn);
    assert_eq!(f.hazard.shoot_wait, hazard::TARUCANN_SHOOT_WAIT);
    stage.tick(
        &mut [&mut f],
        TickInput {
            groups: &mut [],
            objects: &mut clocks,
            items: &mut NoItems,
            map: query(&surfaces, &no_group),
            started: true,
        },
    );
    assert!(clocks.played.contains(&StageAnim::TaruCannShoot));
    f.tick_physics_map(&surfaces_fn);
    assert_eq!(
        f.pos,
        Vec3::new(-2000.0, -800.0, 0.0),
        "the fighter sits in the barrel"
    );
    for _ in 0..hazard::TARUCANN_SHOOT_WAIT {
        f.tick_interrupt(&surfaces_fn);
        f.tick_physics_map(&surfaces_fn);
    }
    assert_eq!(f.status.status, AnyStatus::Common(Status::DamageFlyRoll));
    assert_eq!(f.hazard.tarucann_wait, hazard::TARUCANN_PICKUP_WAIT);
    // Facing right with no spin, the barrel fires straight up.
    assert!(f.physics.vel_knockback.y > 0.0 || f.physics.vel_air.y > 0.0);
}

#[test]
fn power_block_quake_spares_its_hitter_and_rearms_the_spawner() {
    let objects = [];
    let mut i = init(StageKind::Inishie, &objects);
    i.hazard_attack = Some(GroundAttack::from_words([1, 20, 90, 130, 0, 30, 0]));
    let mut stage = Stage::new(&i, &mut [], &mut NoObjects, &mut NoItems);
    stage.apply_item_events([crate::item::StageItemEvent::PowerBlockDamage {
        handicap: 9,
        hitter: Some(1),
    }]);
    let hitter = standing(FighterKind::Mario, 1, Vec3::ZERO, 0, 0);
    let other = standing(FighterKind::Fox, 0, Vec3::ZERO, 0, 0);
    let flying = airborne(Vec3::ZERO);
    assert_eq!(stage.check_hazards(&hitter), None);
    assert_eq!(stage.check_hazards(&flying), None);
    let (attack, handicap) = stage.check_hazards(&other).expect("the quake");
    assert_eq!((attack.damage, handicap), (20, 9));
    let Controller::Inishie(s) = &stage.controller else {
        panic!()
    };
    assert_eq!(s.pblock_status, inishie::PowerBlockStatus::Damage);
    assert_eq!(s.pblock_wait, 2);

    stage.apply_item_events([crate::item::StageItemEvent::PowerBlockGone]);
    let Controller::Inishie(s) = &stage.controller else {
        panic!()
    };
    assert_eq!(s.pblock_status, inishie::PowerBlockStatus::Make);
    assert_eq!(s.pblock_wait, 1800);
    assert_eq!(s.pblock, None);
}

#[test]
fn castle_carries_its_bumper_with_the_ground() {
    let objects = [MapObject {
        kind: mapobj::BUMPER,
        pos: Vec3::new(100.0, 50.0, 0.0),
    }];
    let i = init(StageKind::Castle, &objects);
    let mut clocks = Clocks::with_len(3.0);
    let mut pool = crate::item::ItemPool::default();
    let mut stage = Stage::new(&i, &mut [], &mut clocks, &mut pool);
    assert_eq!(pool.active_count(), 1);
    clocks.barrel = Vec3::new(-640.0, 0.0, 0.0);
    let surfaces: [MapSurface; 0] = [];
    stage.tick(
        &mut [],
        TickInput {
            groups: &mut [],
            objects: &mut clocks,
            items: &mut pool,
            map: query(&surfaces, &no_group),
            started: true,
        },
    );
    let bumper = pool.items().next().unwrap();
    assert_eq!(bumper.kind, crate::item::ItemKind::GBumper);
    assert_eq!(bumper.pos, Vec3::new(-540.0, 50.0, 0.0));
}
