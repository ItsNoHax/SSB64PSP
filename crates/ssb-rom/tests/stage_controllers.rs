//! Drives the portable stage controllers (`ssb_game::stage`) with the ROM's
//! own stage-object animations, played by [`StageJoint`] with the source
//! `GObj::anim_frame` semantics. Gated on `SSB64_ROM`.
//!
//! Offsets are the US `llGRPupupuMap*` labels in
//! `refs/ssb-decomp-re/symbols/reloc_data_symbols.us.txt`.

use ssb_game::fighter::{Fighter, FighterKind};
use ssb_game::stage::pupupu::{EyesAnim, MouthAnim, Pupupu, WindStatus};
use ssb_game::stage::{StageAnim, StageObj, StageObjects};
use ssb_rom::archive::{Archive, File};
use ssb_rom::figatree::JointPose;
use ssb_rom::objanim::{joint_scripts, StageJoint};
use ssb_rom::scene::find_scene_graphs;

const EYES: u32 = 0x10F0;
const MOUTH: u32 = 0x1770;
const FLOWERS_BACK: u32 = 0x2A80;
const FLOWERS_FRONT: u32 = 0x31F8;

/// `dGRPupupuWhispyEyesAnims[lr][status][0]`.
const EYES_ANIMS: [[u32; 2]; 2] = [[0x11A0, 0x12B0], [0x1220, 0x1330]];
/// `dGRPupupuWhispyMouthAnims[lr][status][0]`.
const MOUTH_ANIMS: [[u32; 4]; 2] = [
    [0x18B0, 0x1BE0, 0x1E80, 0x2100],
    [0x1A40, 0x1D30, 0x22F0, 0x2590],
];
/// `dGRPupupuWhispyMouthTextures` / `dGRPupupuWhispyEyesTextures`.
const BACK_ANIMS: [[u32; 3]; 2] = [[0x2BE0, 0x2C30, 0x2C80], [0x2CD0, 0x2D20, 0x2D70]];
const FRONT_ANIMS: [[u32; 3]; 2] = [[0x33E0, 0x3450, 0x34B0], [0x3510, 0x35C0, 0x3660]];

struct GObjAnim {
    graph: u32,
    nodes: usize,
    joints: Vec<Option<StageJoint>>,
    frame: f32,
}

impl GObjAnim {
    fn tick(&mut self, data: &[u8]) {
        let mut pose = JointPose::default();
        for j in self.joints.iter_mut().flatten() {
            j.tick(data, 1.0, &mut pose)
                .expect("stage animation parses");
            if let Some(f) = j.gobj_frame() {
                self.frame = f;
            }
        }
    }
}

struct RomWhispy<'a> {
    file: &'a File,
    objs: [GObjAnim; 4],
}

fn index(obj: StageObj) -> usize {
    match obj {
        StageObj::WhispyEyes => 0,
        StageObj::WhispyMouth => 1,
        StageObj::FlowersBack => 2,
        StageObj::FlowersFront => 3,
        _ => panic!("not a Dream Land object"),
    }
}

impl RomWhispy<'_> {
    /// `gcPlayAnimAll` at priority 5 on every object.
    fn advance(&mut self) {
        for o in self.objs.iter_mut() {
            o.tick(&self.file.data);
        }
    }
}

impl StageObjects for RomWhispy<'_> {
    fn play(&mut self, anim: StageAnim) {
        let (obj, table) = match anim {
            StageAnim::WhispyEyes { lr, status } => (
                0,
                EYES_ANIMS[lr as usize][(status == EyesAnim::Blink) as usize],
            ),
            StageAnim::WhispyMouth { lr, status } => {
                let i = match status {
                    MouthAnim::Stretch => 0,
                    MouthAnim::Turn => 1,
                    MouthAnim::Open => 2,
                    MouthAnim::Close => 3,
                };
                (1, MOUTH_ANIMS[lr as usize][i])
            }
            StageAnim::FlowersBack { lr, phase } => (2, BACK_ANIMS[lr as usize][phase as usize]),
            StageAnim::FlowersFront { lr, phase } => (3, FRONT_ANIMS[lr as usize][phase as usize]),
            _ => panic!("not a Dream Land animation"),
        };
        let o = &mut self.objs[obj];
        // `gcAddAnimAll` zeroes the GObj clock; the immediate
        // `gcPlayAnimAll` parses every node from its start.
        o.frame = 0.0;
        o.joints = joint_scripts(&self.file.data, table, o.nodes)
            .into_iter()
            .map(|s| s.map(|s| StageJoint::start_changed(s, 0.0)))
            .collect();
        o.tick(&self.file.data);
    }
    fn anim_frame(&self, obj: StageObj) -> f32 {
        self.objs[index(obj)].frame
    }
}

struct PackWhispy<'a, 'p> {
    pack: &'a ssb_rom::pack::Pack<'p>,
    objects: ssb_rom::ground_obj::GroundObjects,
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
        self.objects.get(index(obj) as u8).unwrap().frame
    }
}

#[test]
fn whispy_blows_on_the_packed_animation_clocks() {
    if std::env::var_os("SSB64_ROM").is_none() {
        return;
    }
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/generated/ssb64.pak");
    if !path.exists() {
        return;
    }
    let bytes = std::fs::read(path).unwrap();
    let pack = ssb_rom::pack::Pack::open(&bytes).unwrap();
    let mut objects = PackWhispy {
        pack: &pack,
        objects: ssb_rom::ground_obj::GroundObjects::new(&pack, ssb_rom::ground_obj::PUPUPU_FILE),
    };
    assert_eq!(objects.objects.iter().count(), 4);
    assert!(objects.objects.iter().all(|o| o.object.source_file == 152));
    objects.play(StageAnim::FlowersFront { lr: 0, phase: 0 });
    let mut start_clip = 0u32;
    while objects.anim_frame(StageObj::FlowersFront) > 0.0 || start_clip == 0 {
        objects.objects.advance(&pack).unwrap();
        start_clip += 1;
        assert!(start_clip < 1000, "the packed start clip never ends");
    }
    assert_eq!(start_clip, 37);

    ssb_game::rng::set_seed(1);
    let mut w = Pupupu::new();
    let mut a = Fighter::new(FighterKind::Mario, 0, 3);
    a.pos.x = -1200.0;
    let mut b = Fighter::new(FighterKind::Mario, 1, 3);
    b.pos.x = -900.0;
    let mut blows = Vec::new();
    let mut blowing_since = None;
    let mut duration = 0u32;
    let mut status = w.status;
    for frame in 0..20_000u32 {
        objects.objects.advance(&pack).unwrap();
        a.hazard.vel_push = Default::default();
        w.tick(&mut [&mut a, &mut b], &mut objects, true);
        if w.status == WindStatus::Blow && status != WindStatus::Blow {
            duration = u32::from(w.wind_duration);
        }
        status = w.status;
        match (blowing_since, a.hazard.vel_push.x != 0.0) {
            (None, true) => blowing_since = Some(frame),
            (Some(start), false) => {
                blows.push((start, frame - start, duration));
                blowing_since = None;
            }
            _ => {}
        }
    }
    eprintln!("packed front start clip {start_clip}; pushes: {blows:?}");
    assert!(blows.len() >= 3, "{blows:?}");
    for &(_, len, duration) in &blows {
        assert_eq!(len, duration - start_clip - 1, "{blows:?}");
    }
    assert_eq!(w.lr_players, 0);
    assert!(a.hazard.vel_push.x <= 0.0);
}

/// Check all controller clips against the source file, including NULL table
/// entries and the barrel's single-child replacement. Poses and flags persist
/// across changes; only table replacements restart the owning GObj clock.
#[test]
fn packed_controller_poses_match_the_rom() {
    let Some(path) = std::env::var_os("SSB64_ROM") else {
        return;
    };
    let pack_path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/generated/ssb64.pak");
    if !pack_path.exists() {
        return;
    }
    let bytes = std::fs::read(pack_path).unwrap();
    let pack = ssb_rom::pack::Pack::open(&bytes).unwrap();
    let rom = std::fs::read(path).unwrap();
    let info = ssb_rom::rom::identify(&rom).unwrap();
    let archive = Archive::open(&rom, info.region).unwrap();
    use ssb_rom::ground_obj::{self as g, AnimTarget};
    for (object_index, asset) in g::OBJECTS.iter().enumerate() {
        let mut objects = g::GroundObjects::new(&pack, asset.gr_file);
        let packed = objects.get(object_index as u8).unwrap();
        let header_file = archive.load(asset.gr_file).unwrap();
        let headers = ssb_rom::stage::find_ground_data(&header_file, |_, _| true);
        let header = headers.iter().find(|header| header.offset == 0x14).unwrap();
        let (nodes_file, map_head) = header.map_nodes.unwrap();
        assert_eq!(map_head, asset.map_head);
        assert_eq!(packed.object.source_file, nodes_file);
        let file = archive.load(nodes_file).unwrap();
        let graph = find_scene_graphs(&file)
            .into_iter()
            .find(|graph| graph.offset == asset.graph)
            .unwrap();
        let mut poses: Vec<_> = graph
            .nodes
            .iter()
            .map(|node| JointPose {
                rotate: node.desc.rotate,
                translate: node.desc.translate,
                scale: node.desc.scale,
            })
            .collect();
        let mut joints = vec![None::<StageJoint>; poses.len()];
        let mut flags = vec![0u16; poses.len()];
        let mut clock = 0.0;
        for (slot, anim) in g::ANIMS
            .iter()
            .enumerate()
            .filter(|(_, anim)| anim.object as usize == object_index)
        {
            let scripts = match anim.target {
                AnimTarget::Table => joint_scripts(&file.data, anim.script, poses.len()),
                AnimTarget::Node(node) => {
                    let mut scripts = vec![None; poses.len()];
                    scripts[node as usize] = Some(anim.script);
                    scripts
                }
            };
            if anim.target == AnimTarget::Table {
                clock = 0.0;
            }
            for (i, script) in scripts.into_iter().enumerate() {
                if anim.target == AnimTarget::Table || anim.target == AnimTarget::Node(i as u8) {
                    joints[i] = script.map(|script| {
                        let mut joint = StageJoint::start_changed(script, 0.0);
                        joint.flags = flags[i];
                        joint
                    });
                }
            }
            objects.play(&pack, slot).unwrap();
            for tick in 0..120 {
                for (i, joint) in joints.iter_mut().enumerate() {
                    if tick == 0
                        && anim.target != AnimTarget::Table
                        && anim.target != AnimTarget::Node(i as u8)
                    {
                        continue;
                    }
                    if let Some(joint) = joint {
                        joint.tick(&file.data, 1.0, &mut poses[i]).unwrap();
                        flags[i] = joint.flags;
                        if let Some(frame) = joint.gobj_frame() {
                            clock = frame;
                        }
                    }
                }
                if tick != 0 {
                    objects.advance(&pack).unwrap();
                }
                let packed = objects.get(object_index as u8).unwrap();
                assert_eq!(packed.frame, clock, "{} tick {tick} clock", anim.name);
                for (i, pose) in poses.iter().enumerate() {
                    assert_eq!(
                        packed.pose(i),
                        Some(pose),
                        "{} tick {tick} node {i}",
                        anim.name
                    );
                    assert_eq!(
                        packed.flags(i),
                        flags[i],
                        "{} tick {tick} flags {i}",
                        anim.name
                    );
                }
            }
        }
    }
}

fn whispy_file(archive: &Archive<'_>) -> File {
    // `map_head` is `map_nodes - llGRPupupuMapMapHead`: whichever file holds
    // all four object graphs at their labels.
    for id in 0..archive.len() as u32 {
        let Ok(file) = archive.load(id) else { continue };
        let graphs = find_scene_graphs(&file);
        if [EYES, MOUTH, FLOWERS_BACK, FLOWERS_FRONT]
            .iter()
            .all(|at| graphs.iter().any(|g| g.offset == *at))
        {
            return file;
        }
    }
    panic!("no file holds Whispy's graphs");
}

#[test]
fn whispy_blows_on_the_rom_animation_clocks() {
    let Some(path) = std::env::var_os("SSB64_ROM") else {
        return;
    };
    let data = std::fs::read(path).unwrap();
    let info = ssb_rom::rom::identify(&data).unwrap();
    let archive = Archive::open(&data, info.region).unwrap();
    let file = whispy_file(&archive);
    let graphs = find_scene_graphs(&file);
    let nodes = |at: u32| graphs.iter().find(|g| g.offset == at).unwrap().nodes.len();
    let obj = |graph: u32| GObjAnim {
        graph,
        nodes: nodes(graph),
        joints: Vec::new(),
        frame: 0.0,
    };
    let mut objects = RomWhispy {
        file: &file,
        objs: [obj(EYES), obj(MOUTH), obj(FLOWERS_BACK), obj(FLOWERS_FRONT)],
    };
    assert!(objects.objs.iter().all(|o| o.graph != 0 && o.nodes > 0));

    ssb_game::rng::set_seed(1);
    let mut w = Pupupu::new();
    let mut a = Fighter::new(FighterKind::Mario, 0, 3);
    a.pos.x = -1200.0;
    let mut b = Fighter::new(FighterKind::Mario, 1, 3);
    b.pos.x = -900.0;

    // The left front-flower start clip's length: the frame its GObj clock
    // first reads zero or less after `play`.
    objects.play(StageAnim::FlowersFront { lr: 0, phase: 0 });
    let mut start_clip = 0u32;
    while objects.anim_frame(StageObj::FlowersFront) > 0.0 || start_clip == 0 {
        objects.advance();
        start_clip += 1;
        assert!(start_clip < 1000, "the start clip never ends");
    }

    let mut blows = Vec::new();
    let mut blowing_since = None;
    let mut duration = 0u32;
    let mut status = w.status;
    for frame in 0..20_000u32 {
        objects.advance();
        a.hazard.vel_push = Default::default();
        w.tick(&mut [&mut a, &mut b], &mut objects, true);
        if w.status == WindStatus::Blow && status != WindStatus::Blow {
            duration = u32::from(w.wind_duration);
        }
        status = w.status;
        match (blowing_since, a.hazard.vel_push.x != 0.0) {
            (None, true) => blowing_since = Some(frame),
            (Some(start), false) => {
                blows.push((start, frame - start, duration));
                blowing_since = None;
            }
            _ => {}
        }
    }
    eprintln!("front start clip {start_clip}; pushes (start, frames, duration): {blows:?}");
    // Every clip Whispy waits on ends: the wind comes round several times.
    assert!(blows.len() >= 3, "{blows:?}");
    for &(_, len, duration) in &blows {
        // Blow runs `duration` frames from the frame after it starts; the
        // front flowers push from the frame after their 22-frame lead and
        // start clip, until 21 frames past the Blow's end.
        assert_eq!(len, duration - start_clip - 1, "{blows:?}");
    }
    // The fighters are left of Whispy, so it blows left.
    assert_eq!(w.lr_players, 0);
    assert!(a.hazard.vel_push.x <= 0.0);
}

/// Every `GRAttackColl` / `FTThrowHitDesc` a controller reads sits at
/// `llGR*Map*` offset 0xBC of its `GRxxxMap` file, whose `MPGroundData`
/// header is at `llGR*MapMapHeader` (0x14); the acid surface is the second
/// node of `llGRZebesMapAcidDObjDesc` in the `map_nodes` file.
#[test]
fn hazard_descriptors_sit_where_the_controllers_read_them() {
    let Some(path) = std::env::var_os("SSB64_ROM") else {
        return;
    };
    let data = std::fs::read(path).unwrap();
    let info = ssb_rom::rom::identify(&data).unwrap();
    let archive = Archive::open(&data, info.region).unwrap();
    let words = |file: &File| -> [i32; 7] {
        core::array::from_fn(|i| {
            i32::from_be_bytes(file.data[0xBC + i * 4..0xC0 + i * 4].try_into().unwrap())
        })
    };
    // (GRxxxMap file, map_nodes target, words at 0xBC).
    #[rustfmt::skip]
    let expected: [(u32, (u32, u32), [i32; 7]); 4] = [
        (0x101, (157, 0xB08), [0, 16, 80, 130, 0, 30, 1]),  // Zebes acid
        (0x104, (155, 0x5F0), [1, 20, 90, 130, 0, 30, 0]),  // Inishie POW
        (0x105, (158, 0xA98), [3, 0, 90, 0, 0, 180, 0]),    // Jungle barrel
        (0x109, (0, 0), [2, 14, 90, 60, 0, 115, 0]),        // Hyrule twister
    ];
    for (id, nodes, want) in expected {
        let file = archive.load(id).unwrap();
        let heads = ssb_rom::stage::find_ground_data(&file, |_, _| true);
        let head = heads
            .iter()
            .find(|h| h.offset == 0x14)
            .expect("MPGroundData at 0x14");
        if nodes != (0, 0) {
            assert_eq!(head.map_nodes, Some(nodes), "file {id:#x}");
        } else {
            assert_eq!(head.map_nodes, None, "file {id:#x}");
        }
        assert_eq!(words(&file), want, "file {id:#x}");
    }
    let acid = archive.load(157).unwrap();
    let graph = find_scene_graphs(&acid)
        .into_iter()
        .find(|g| g.offset == 0xB08)
        .expect("llGRZebesMapAcidDObjDesc");
    assert_eq!(graph.nodes[1].parent, Some(0));
    eprintln!("acid surface y {}", graph.nodes[1].desc.translate[1]);
}

/// The match loader's path: every VS `GR*Map` file maps to its `GRKind`,
/// and only the four hazard stages yield a descriptor, of the struct their
/// controller reads.
#[test]
fn vs_ground_files_yield_their_kind_and_hazard() {
    use ssb_game::stage::StageKind;
    let Some(path) = std::env::var_os("SSB64_ROM") else {
        return;
    };
    let data = std::fs::read(path).unwrap();
    let info = ssb_rom::rom::identify(&data).unwrap();
    let archive = Archive::open(&data, info.region).unwrap();
    let mut kinds = Vec::new();
    for (gkind, &id) in ssb_rom::stage::VS_GROUND_FILES.iter().enumerate() {
        assert_eq!(ssb_rom::stage::vs_ground_kind(id), Some(gkind as u8));
        let file = archive.load(id).unwrap();
        let heads = ssb_rom::stage::find_ground_data(&file, |_, _| true);
        assert!(heads.iter().any(|h| h.offset == 0x14), "file {id:#x}");
        let kind = StageKind::from_gkind(gkind as u8).unwrap();
        let (attack, throw) =
            ssb_rom::stage::hazard_words(&file).map_or((None, None), |w| kind.hazard_descs(w));
        kinds.push((kind, attack.map(|a| a.damage), throw.map(|t| t.angle)));
    }
    assert_eq!(
        kinds,
        [
            (StageKind::Castle, None, None),
            (StageKind::Sector, None, None),
            (StageKind::Jungle, None, Some(90)),
            (StageKind::Zebes, Some(16), None),
            (StageKind::Hyrule, None, Some(90)),
            (StageKind::Yoster, None, None),
            (StageKind::Pupupu, None, None),
            (StageKind::Yamabuki, None, None),
            (StageKind::Inishie, Some(20), None),
        ]
    );
    assert_eq!(ssb_rom::stage::vs_ground_kind(0x10A), None);
}

/// The pack carries each VS stage's hazard data where the loader reads it.
#[test]
fn packed_vs_stages_carry_their_hazard_data() {
    if std::env::var_os("SSB64_ROM").is_none() {
        return;
    }
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/generated/ssb64.pak");
    if !path.exists() {
        return;
    }
    let bytes = std::fs::read(path).unwrap();
    let pack = ssb_rom::pack::Pack::open(&bytes).unwrap();
    #[rustfmt::skip]
    let expected: [[i32; 7]; 9] = [
        [0; 7],
        [0; 7],
        [3, 0, 90, 0, 0, 180, 0],   // Jungle barrel
        [0, 16, 80, 130, 0, 30, 1], // Zebes acid
        [2, 14, 90, 60, 0, 115, 0], // Hyrule twister
        [0; 7],
        [0; 7],
        [0; 7],
        [1, 20, 90, 130, 0, 30, 0], // Inishie POW
    ];
    for (gkind, &file) in ssb_rom::stage::VS_GROUND_FILES.iter().enumerate() {
        let i = pack.stage_of_file(file).expect("VS stage packed");
        let s = pack.stage(i).unwrap();
        assert_eq!(s.source_offset, 0x14, "file {file:#x}");
        assert_eq!(s.hazard, expected[gkind], "file {file:#x}");
        if gkind == 3 {
            assert!(
                (s.hazard_surface_y - -282.725).abs() < 1e-3,
                "{}",
                s.hazard_surface_y
            );
        } else {
            assert_eq!(s.hazard_surface_y, 0.0, "file {file:#x}");
        }
    }
}

/// Every VS stage builds its controller from packed data alone and runs a
/// minute of ticks with no fighters and no object runtime: the loader path
/// `psp-runtime::scene::StageSetup` feeds `Stage::new`.
#[test]
fn every_packed_vs_stage_builds_and_runs_its_controller() {
    use ssb_game::stage::{
        Controller, MapObject, NoObjects, Stage, StageInit, StageKind, TickInput,
    };
    if std::env::var_os("SSB64_ROM").is_none() {
        return;
    }
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/generated/ssb64.pak");
    if !path.exists() {
        return;
    }
    let bytes = std::fs::read(path).unwrap();
    let pack = ssb_rom::pack::Pack::open(&bytes).unwrap();
    for (gkind, &file) in ssb_rom::stage::VS_GROUND_FILES.iter().enumerate() {
        let s = pack.stage(pack.stage_of_file(file).unwrap()).unwrap();
        let kind = StageKind::from_gkind(gkind as u8).unwrap();
        let map_objects: Vec<MapObject> = pack
            .stage_points(&s)
            .map(|p| MapObject {
                kind: p.kind,
                pos: ssb_engine::math::Vec3::new(p.x as f32, p.y as f32, 0.0),
            })
            .collect();
        let (hazard_attack, hazard_throw) = kind.hazard_descs(s.hazard);
        let init = StageInit {
            kind,
            map_objects: &map_objects,
            bound_bottom: s.bounds.bottom as f32,
            hazard_attack,
            hazard_throw,
            acid_surface_y: s.hazard_surface_y,
        };
        let count = pack
            .stage_lines(&s)
            .map(|l| l.yakumono as usize + 1)
            .max()
            .unwrap_or(0);
        let mut groups = vec![ssb_game::map::MapGroup::default(); count];
        let mut stage = Stage::new(&init, &mut groups, &mut NoObjects);
        assert_eq!(stage.attack, hazard_attack);
        assert_eq!(stage.throw, hazard_throw);
        assert_eq!(
            matches!(stage.controller, Controller::None),
            kind == StageKind::Sector,
            "{kind:?}"
        );
        let line_group = |_: u16| None;
        for _ in 0..3600 {
            stage.tick(
                &mut [],
                TickInput {
                    groups: &mut groups,
                    objects: &mut NoObjects,
                    map: ssb_game::stage::MapQuery {
                        surfaces: || core::iter::empty::<ssb_game::weapon::MapSurface>(),
                        line_group: &line_group,
                    },
                    started: true,
                },
            );
        }
    }
}
