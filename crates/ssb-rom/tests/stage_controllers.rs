//! Drives the portable stage controllers (`ssb_game::stage`) with the ROM's
//! own stage-object animations, played by [`StageJoint`] with the source
//! `GObj::anim_frame` semantics. Gated on `SSB64_ROM`.
//!
//! Offsets are the US `llGRPupupuMap*` labels in
//! `refs/ssb-decomp-re/symbols/reloc_data_symbols.us.txt`.

use ssb_game::fighter::{Fighter, FighterKind};
use ssb_game::stage::pupupu::{EyesAnim, MouthAnim, Pupupu, WindStatus};
use ssb_game::stage::{NoItems, StageAnim, StageObj, StageObjects};
use ssb_rom::archive::{Archive, File};
use ssb_rom::figatree::JointPose;
use ssb_rom::objanim::{joint_scripts, StageJoint};
use ssb_rom::scene::find_scene_graphs;

/// Every Saffron item uses file 159, while the gate uses file 160. Replay
/// each packed root against its ROM script, including the make-time play.
#[test]
fn saffron_item_clocks_and_texture_frames_match_the_rom() {
    use ssb_rom::ground_obj::{self as g, GroundObjects};
    use ssb_rom::pack::{NodeDesc, Pack};
    let Some((bytes, rom)) = pack_and_rom() else {
        return;
    };
    let pack = Pack::open(&bytes).unwrap();
    let archive = Archive::open(&rom, ssb_rom::rom::identify(&rom).unwrap().region).unwrap();
    let file = archive.load(159).unwrap();
    let mut objects = GroundObjects::new(&pack, g::YAMABUKI_FILE);
    assert_eq!(objects.iter().count(), 6);
    for (id, script) in [0x3F8, 0x828, 0x1A28, 0x23D8, 0xF38]
        .into_iter()
        .enumerate()
    {
        let asset = g::MONSTER_FIRST + id as u8;
        let object = objects.instance(asset, 0).unwrap().object;
        assert_eq!(object.source_file, 159);
        objects.item_make(&pack, asset, 0);
        let mut joint = StageJoint::start_changed(script, 0.0);
        let mut pose = JointPose::default();
        joint.tick(&file.data, 1.0, &mut pose).unwrap();
        let mut plays = 1;
        while !joint.ended() {
            let write = objects.item_play(&pack, asset, 0);
            joint.tick(&file.data, 1.0, &mut pose).unwrap();
            for (axis, v) in write.into_iter().enumerate() {
                if let Some(v) = v {
                    assert_eq!(
                        v.to_bits(),
                        pose.translate[axis].to_bits(),
                        "kind {id} play {plays} axis {axis}"
                    );
                }
            }
            assert_eq!(
                objects.item_root_frame(asset, 0).to_bits(),
                joint.frame().to_bits()
            );
            assert_eq!(objects.item_root_idle(asset, 0), joint.ended());
            plays += 1;
            assert!(plays < 500);
        }
        assert!(plays > 80, "kind {id}: {plays}");
        if id == 2 || id == 3 {
            let o = objects.instance_mut(asset, 0).unwrap();
            let node = pack.node(object.first_node + 1).unwrap();
            for frame in 0..2 {
                o.texture = frame;
                let draw = o.draw_node(1, node);
                assert_ne!(draw.mesh, NodeDesc::NO_MESH);
                let mesh = pack.mesh(draw.mesh).unwrap();
                assert_eq!(
                    mesh.source_offset,
                    g::MONSTER_TEXTURES[id - 2][frame as usize]
                );
                assert!(mesh.prim_count > 0);
            }
        }
    }
}

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
        // Item trees play from their item, not `advance`
        // (`packed_item_trees_play_on_the_ejected_root`).
        if asset.item {
            continue;
        }
        let mut objects = g::GroundObjects::new(&pack, asset.gr_file);
        let packed = objects.get(object_index as u8).unwrap();
        let header_file = archive.load(asset.gr_file).unwrap();
        let headers = ssb_rom::stage::find_ground_data(&header_file, |_, _| true);
        let header = headers.iter().find(|header| header.offset == 0x14).unwrap();
        let (nodes_file, map_head) = header.map_nodes.unwrap();
        assert_eq!(map_head, asset.map_head);
        assert_eq!(packed.object.source_file, nodes_file);
        let file = archive.load(nodes_file).unwrap();
        // A display-list object is one `DObj` at rest (RE-365).
        let mut poses: Vec<_> = match asset.build {
            g::Build::Desc => find_scene_graphs(&file)
                .into_iter()
                .find(|graph| graph.offset == asset.graph)
                .unwrap()
                .nodes
                .iter()
                .map(|node| JointPose {
                    rotate: node.desc.rotate,
                    translate: node.desc.translate,
                    scale: node.desc.scale,
                })
                .collect(),
            g::Build::Dl | g::Build::Empty => vec![JointPose {
                rotate: [0.0; 3],
                translate: [0.0; 3],
                scale: [1.0; 3],
            }],
        };
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
        let mut stage = Stage::new(&init, &mut groups, &mut NoObjects, &mut NoItems);
        assert_eq!(stage.attack, hazard_attack);
        assert_eq!(stage.throw, hazard_throw);
        assert!(!matches!(stage.controller, Controller::None), "{kind:?}");
        let line_group = |_: u16| None;
        for _ in 0..3600 {
            stage.tick(
                &mut [],
                TickInput {
                    groups: &mut groups,
                    objects: &mut NoObjects,
                    items: &mut NoItems,
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

/// The packed acid under the Zebes controller: the controller owns the root
/// Y (`grZebesMakeAcid`, `grZebesAcidUpdateRise`) and the surface it tests
/// is the child's `llGRZebesMapAcidAnimJoint` Y, read after the animation.
#[test]
fn packed_acid_follows_the_zebes_controller() {
    use ssb_game::stage::{Controller, Stage, StageInit, StageKind, TickInput};
    use ssb_rom::ground_obj::{self as g, GroundObjects};
    use ssb_rom::pack::Pack;
    struct Port<'a, 'p> {
        pack: &'a Pack<'p>,
        objects: &'a mut GroundObjects,
    }
    impl StageObjects for Port<'_, '_> {
        fn play(&mut self, anim: StageAnim) {
            assert_eq!(anim, StageAnim::Acid);
            self.objects.play(self.pack, g::ACID_ANIM).unwrap();
        }
        fn set_translate_y(&mut self, obj: StageObj, y: f32) {
            assert_eq!(obj, StageObj::Acid);
            self.objects.get_mut(g::ACID).unwrap().set_translate_y(y);
        }
        fn child_translate(&self, obj: StageObj) -> Option<ssb_engine::math::Vec3> {
            assert_eq!(obj, StageObj::Acid);
            let [x, y, z] = self.objects.get(g::ACID)?.child_translate(self.pack)?;
            Some(ssb_engine::math::Vec3::new(x, y, z))
        }
    }
    if std::env::var_os("SSB64_ROM").is_none() {
        return;
    }
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/generated/ssb64.pak");
    if !path.exists() {
        return;
    }
    let bytes = std::fs::read(path).unwrap();
    let pack = Pack::open(&bytes).unwrap();
    let s = pack
        .stage(pack.stage_of_file(g::ZEBES_FILE).unwrap())
        .unwrap();
    let (hazard_attack, hazard_throw) = StageKind::Zebes.hazard_descs(s.hazard);
    let init = StageInit {
        kind: StageKind::Zebes,
        map_objects: &[],
        bound_bottom: s.bounds.bottom as f32,
        hazard_attack,
        hazard_throw,
        acid_surface_y: s.hazard_surface_y,
    };
    let mut objects = GroundObjects::new(&pack, g::ZEBES_FILE);
    let mut stage = Stage::new(
        &init,
        &mut [],
        &mut Port {
            pack: &pack,
            objects: &mut objects,
        },
        &mut NoItems,
    );
    let acid = objects.get(g::ACID).expect("acid packed");
    assert_eq!(acid.node_count(), 2);
    let surface = |stage: &Stage| match &stage.controller {
        Controller::Zebes(z) => (z.level, z.surface_y),
        _ => panic!(),
    };
    // The immediate parse moves the child off its rest -282.725 to the
    // script's first key, -270.
    let (level, surface_y) = surface(&stage);
    assert_eq!(level, -3000.0);
    assert_eq!(surface_y, -270.0);
    assert_eq!(acid.translate()[1], level);
    let line_group = |_: u16| None;
    let (mut low, mut high) = (f32::MAX, f32::MIN);
    let (mut wave_low, mut wave_high) = (f32::MAX, f32::MIN);
    for _ in 0..3600 {
        objects.advance(&pack).unwrap();
        stage.tick(
            &mut [],
            TickInput {
                groups: &mut [],
                objects: &mut Port {
                    pack: &pack,
                    objects: &mut objects,
                },
                items: &mut NoItems,
                map: ssb_game::stage::MapQuery {
                    surfaces: || core::iter::empty::<ssb_game::weapon::MapSurface>(),
                    line_group: &line_group,
                },
                started: true,
            },
        );
        let (level, surface_y) = surface(&stage);
        let acid = objects.get(g::ACID).unwrap();
        assert_eq!(acid.translate()[1], level);
        assert_eq!(acid.pose(1).unwrap().translate[1], surface_y);
        low = low.min(level);
        high = high.max(level);
        wave_low = wave_low.min(surface_y);
        wave_high = wave_high.max(surface_y);
    }
    // A minute includes the first 1,200-frame wait and at least one rise,
    // and the surface script loops the whole time.
    assert!(high > low, "{low} {high}");
    assert!(wave_high - wave_low > 100.0, "{wave_low} {wave_high}");
    eprintln!("acid level {low}..{high}, surface {wave_low}..{wave_high}");
}

/// RE-364: every controller material table plays on its object's own
/// clocks. Each packed `GROUND_MAT` joint restarts the `MatAnimDesc` the
/// object's primitives carry, and its track values follow an independent
/// replay of the same script on the archive file, tick for tick. An `MObj`
/// the controller has not reached keeps no clock (its rest material).
#[test]
fn packed_ground_materials_replay_the_rom() {
    use ssb_rom::ground_obj::{self as g, GroundObjects};
    use ssb_rom::matanim::MaterialJoint;
    use ssb_rom::pack::{AnimDesc, AnimJoint, Pack};
    let Some(rom) = std::env::var_os("SSB64_ROM") else {
        return;
    };
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/generated/ssb64.pak");
    if !path.exists() {
        return;
    }
    let data = std::fs::read(rom).unwrap();
    let info = ssb_rom::rom::identify(&data).unwrap();
    let archive = Archive::open(&data, info.region).unwrap();
    let bytes = std::fs::read(path).unwrap();
    let pack = Pack::open(&bytes).unwrap();

    let descs: Vec<AnimDesc> = (0..pack.anim_count())
        .filter_map(|i| pack.anim(i))
        .filter(|a| a.fighter == AnimDesc::GROUND_MAT)
        .collect();
    assert_eq!(descs.len(), g::MAT_ANIMS.len());
    let mut checked = 0usize;
    for (anim, _) in g::ANIMS.iter().enumerate() {
        let Some(slot) = g::mat_anim_of(anim) else {
            continue;
        };
        let asset = &g::MAT_ANIMS[slot];
        let object = asset.object;
        // Item trees play from their item, not `advance`
        // (`packed_item_trees_play_on_the_ejected_root`).
        if g::OBJECTS[object as usize].item {
            continue;
        }
        let desc = descs.iter().find(|d| d.slot == slot as u32).unwrap();
        let file = archive.load(desc.source_file).unwrap();
        let joints: Vec<AnimJoint> = (0..desc.joint_count)
            .map(|j| pack.anim_joint(desc.first_joint + j).unwrap())
            .collect();
        let mut objects = GroundObjects::new(&pack, asset.gr_file);
        let obj = objects.get(object).unwrap();
        assert!(
            obj.materials().is_empty(),
            "{}: a clock before any play",
            asset.name
        );
        // The targets are the entries the object's primitives carry.
        let od = obj.object;
        let carried: Vec<u32> = (0..od.node_count)
            .filter_map(|n| pack.node(od.first_node + n))
            .filter_map(|n| pack.mesh(n.mesh))
            .flat_map(|m| (0..m.prim_count).map(move |p| m.first_prim + p))
            .filter_map(|p| pack.prim(p))
            .map(|p| p.mat_anim)
            .collect();
        for j in &joints {
            assert!(
                carried.contains(&j.node),
                "{}: target {} not drawn",
                asset.name,
                j.node
            );
        }
        objects.play(&pack, anim).unwrap();
        let mut refs: Vec<(u32, MaterialJoint)> = joints
            .iter()
            .map(|j| (j.node, MaterialJoint::start(j.script, 0.0)))
            .collect();
        for tick in 0..240 {
            if tick > 0 {
                objects.advance(&pack).unwrap();
            }
            let obj = objects.get(object).unwrap();
            for (target, r) in refs.iter_mut() {
                r.tick(&file.data, 1.0).unwrap();
                let live = obj.materials().joint_for(*target).unwrap();
                for track in 0..ssb_rom::matanim::TICK_TRACK_COUNT {
                    assert_eq!(
                        live.track_value(track).map(f32::to_bits),
                        r.track_value(track).map(f32::to_bits),
                        "{} tick {tick} track {track}",
                        asset.name
                    );
                }
                let id = r.track_value(ssb_rom::matanim::TRACK_TEXTURE_ID_CURRENT);
                if let Some(id) = id {
                    let m = pack.mat_anim(*target).unwrap();
                    assert_eq!(
                        obj.materials().resolved_texture(&pack, *target),
                        Some(m.textures[(id.max(0.0) as usize).min(m.texture_count as usize - 1)]),
                        "{} tick {tick}",
                        asset.name
                    );
                }
            }
        }
        checked += 1;
    }
    assert_eq!(checked, 11);
}

/// RE-364: a cloud's `MObj` idles until its first script, runs the fade
/// for as long as the ROM replay does, and idles again on the parse that
/// reads `End`. Only the next `gcPlayAnimAll` parses a new script.
#[test]
fn packed_cloud_fades_end_when_the_rom_replay_does() {
    use ssb_rom::ground_obj::{self as g, GroundObjects};
    use ssb_rom::matanim::{resolve_scripts, MaterialJoint};
    use ssb_rom::pack::Pack;
    let Some(rom) = std::env::var_os("SSB64_ROM") else {
        return;
    };
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/generated/ssb64.pak");
    if !path.exists() {
        return;
    }
    let data = std::fs::read(rom).unwrap();
    let info = ssb_rom::rom::identify(&data).unwrap();
    let archive = Archive::open(&data, info.region).unwrap();
    // `llGRYosterMapMapHead` (0x100) in the `map_nodes` file.
    let file = archive.load(154).unwrap();
    let bytes = std::fs::read(path).unwrap();
    let pack = Pack::open(&bytes).unwrap();
    let mut objects = GroundObjects::new(&pack, g::YOSTER_FILE);
    for mat in [g::CLOUD_SOLID_MAT, g::CLOUD_EVAPORATE_MAT] {
        assert!(objects.has_mat(mat));
        let script = resolve_scripts(&file, g::MAT_ANIMS[mat].table, 1, |_| 1)[0][0].unwrap();
        let mut r = MaterialJoint::start(script, 0.0);
        let mut rom_ticks = 0u32;
        while !r.ended() {
            r.tick(&file.data, 1.0).unwrap();
            rom_ticks += 1;
            assert!(rom_ticks < 1000);
        }
        assert!((0..g::CLOUD_COUNT).all(|c| objects.cloud_idle(&pack, c)));
        objects.play_cloud(&pack, 1, mat);
        assert!(!objects.cloud_idle(&pack, 1));
        assert!(objects.cloud_idle(&pack, 0) && objects.cloud_idle(&pack, 2));
        let mut ticks = 0u32;
        while !objects.cloud_idle(&pack, 1) {
            objects.advance(&pack).unwrap();
            ticks += 1;
            assert!(ticks < 1000);
        }
        assert_eq!(ticks, rom_ticks, "{}", g::MAT_ANIMS[mat].name);
        assert_eq!(ticks, 101, "{}", g::MAT_ANIMS[mat].name);
    }
}

/// Opens the generated pack and the ROM archive, or `None` when either is
/// missing.
fn pack_and_rom() -> Option<(Vec<u8>, Vec<u8>)> {
    let rom = std::env::var_os("SSB64_ROM")?;
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/generated/ssb64.pak");
    Some((std::fs::read(path).ok()?, std::fs::read(rom).ok()?))
}

/// The runtime port `psp-runtime::scene::StageObjectsPort` implements,
/// for the display-list objects (RE-365).
struct ListPort<'a, 'p> {
    pack: &'a ssb_rom::pack::Pack<'p>,
    objects: &'a mut ssb_rom::ground_obj::GroundObjects,
}

impl ListPort<'_, '_> {
    fn key(obj: StageObj) -> (u8, u8) {
        use ssb_rom::ground_obj as g;
        match obj {
            StageObj::Cloud(i) => (g::CLOUD, i),
            StageObj::Scale(i) => (g::SCALE_PLATFORM, i),
            StageObj::ScaleStrings => (g::SCALE_STRINGS, 0),
            StageObj::CastleGround => (g::CASTLE_GROUND, 0),
            _ => panic!("not a display-list object: {obj:?}"),
        }
    }
    fn get_mut(&mut self, obj: StageObj) -> &mut ssb_rom::ground_obj::GroundObject {
        let (a, i) = Self::key(obj);
        self.objects.instance_mut(a, i).expect("object packed")
    }
}

impl StageObjects for ListPort<'_, '_> {
    fn play(&mut self, anim: StageAnim) {
        use ssb_rom::ground_obj as g;
        match anim {
            StageAnim::CloudSolid(i) => {
                self.objects
                    .play_cloud(self.pack, i as usize, g::CLOUD_SOLID_MAT)
            }
            StageAnim::CloudEvaporate(i) => {
                self.objects
                    .play_cloud(self.pack, i as usize, g::CLOUD_EVAPORATE_MAT)
            }
            StageAnim::ScaleRetract(i) => self
                .objects
                .play_on(self.pack, g::SCALE_RETRACT, i)
                .unwrap(),
            StageAnim::CastleGround => self.objects.play(self.pack, g::CASTLE_GROUND_ANIM).unwrap(),
            other => panic!("unexpected {other:?}"),
        }
    }
    fn stop(&mut self, obj: StageObj) {
        self.get_mut(obj).stop();
    }
    fn mat_anim_idle(&self, obj: StageObj) -> bool {
        match obj {
            StageObj::Cloud(i) => self.objects.cloud_idle(self.pack, i as usize),
            _ => true,
        }
    }
    fn translate(&self, obj: StageObj) -> ssb_engine::math::Vec3 {
        let (a, i) = Self::key(obj);
        let [x, y, z] = self.objects.instance(a, i).unwrap().translate();
        ssb_engine::math::Vec3::new(x, y, z)
    }
    fn set_translate_y(&mut self, obj: StageObj, y: f32) {
        self.get_mut(obj).set_translate_y(y);
    }
    fn set_translate(&mut self, obj: StageObj, pos: ssb_engine::math::Vec3) {
        self.get_mut(obj).set_translate([pos.x, pos.y, pos.z]);
    }
    fn node_translate(&self, obj: StageObj, node: u8) -> Option<ssb_engine::math::Vec3> {
        let (a, i) = Self::key(obj);
        let [x, y, z] = self.objects.instance(a, i)?.node_translate(node as usize)?;
        Some(ssb_engine::math::Vec3::new(x, y, z))
    }
    fn set_node_translate_y(&mut self, obj: StageObj, node: u8, y: f32) {
        self.get_mut(obj).set_node_translate_y(node as usize, y);
    }
}

/// RE-365: the objects the Yoshi's Island, Mushroom Kingdom and Castle
/// controllers build from display lists pack as their controllers make
/// them. Each cloud is `llGRYosterMapMapHead` (four nodes) with a
/// `Kind48` leaf drawing `llGRYosterMapCloudDisplayList` under each of the
/// three children, and the leaf's primitives carry the `MatAnimDesc` both
/// cloud fades restart. The scale strings are the five-node
/// `llGRInishieMapScaleDObjDesc`, each platform one node drawing
/// `llGRInishieMapMapHead`, and the Castle ground one node with no mesh.
#[test]
fn packed_display_list_objects_build_as_the_controllers_do() {
    use ssb_rom::ground_obj::{self as g, GroundObjects};
    use ssb_rom::pack::{AnimDesc, NodeDesc, Pack};
    let Some((bytes, _)) = pack_and_rom() else {
        return;
    };
    let pack = Pack::open(&bytes).unwrap();
    let meshes = |o: &ssb_rom::pack::ObjectDesc| {
        (0..o.node_count)
            .filter_map(|n| pack.node(o.first_node + n))
            .filter_map(|n| pack.mesh(n.mesh))
            .count()
    };

    let clouds = GroundObjects::new(&pack, g::YOSTER_FILE);
    assert_eq!(clouds.iter().count(), g::CLOUD_COUNT);
    let fade_targets: Vec<u32> = (0..pack.anim_count())
        .filter_map(|i| pack.anim(i))
        .filter(|a| {
            a.fighter == AnimDesc::GROUND_MAT
                && (a.slot as usize == g::CLOUD_SOLID_MAT
                    || a.slot as usize == g::CLOUD_EVAPORATE_MAT)
        })
        .flat_map(|a| (0..a.joint_count).map(move |j| a.first_joint + j))
        .map(|j| pack.anim_joint(j).unwrap().node)
        .collect();
    assert_eq!(fade_targets.len(), 2);
    assert_eq!(fade_targets[0], fade_targets[1]);
    for c in 0..g::CLOUD_COUNT as u8 {
        let cloud = clouds.instance(g::CLOUD, c).expect("cloud packed");
        assert_eq!(cloud.node_count(), 4);
        assert_eq!(meshes(&cloud.object), 0);
        assert_eq!(cloud.leaf_parents(&pack).collect::<Vec<_>>(), [1, 2, 3]);
        let leaf = cloud.leaf.expect("leaf packed");
        assert_eq!(leaf.node_count, 1);
        let node = pack.node(leaf.first_node).unwrap();
        assert_ne!(node.flags & NodeDesc::FLAG_BILLBOARD, 0);
        assert_ne!(node.flags & NodeDesc::FLAG_BILLBOARD_PITCH_LOCKED, 0);
        let mesh = pack.mesh(node.mesh).expect("cloud mesh");
        let carried: Vec<u32> = (0..mesh.prim_count)
            .filter_map(|p| pack.prim(mesh.first_prim + p))
            .map(|p| p.mat_anim)
            .collect();
        assert!(carried.contains(&fade_targets[0]), "{carried:?}");
        // Nodes 2 and 3 rest at scale 1.093, but only their translation
        // reaches a matrix (`nGCMatrixKindTra`).
        assert!(cloud.pose(2).unwrap().scale[0] > 1.09);
        let mut posed = [ssb_rom::scene::Mat4::IDENTITY; 4];
        cloud.compose(&pack, &mut posed);
        for m in &posed {
            let col = |c: usize| (0..3).map(|r| m.0[c * 4 + r].powi(2)).sum::<f32>().sqrt();
            assert!((col(0) - 1.0).abs() < 1e-6 && (col(1) - 1.0).abs() < 1e-6);
        }
    }

    let scales = GroundObjects::new(&pack, g::INISHIE_FILE);
    let strings = scales.get(g::SCALE_STRINGS).expect("strings packed");
    assert_eq!(strings.node_count(), 5);
    assert_eq!(meshes(&strings.object), 5);
    for i in 0..2 {
        let platform = scales
            .instance(g::SCALE_PLATFORM, i)
            .expect("platform packed");
        assert_eq!(platform.node_count(), 1);
        assert_eq!(meshes(&platform.object), 1);
    }
    assert!(scales.has(g::SCALE_RETRACT));

    let castle = GroundObjects::new(&pack, g::CASTLE_FILE);
    let ground = castle.get(g::CASTLE_GROUND).expect("castle ground packed");
    assert_eq!(ground.node_count(), 1);
    assert_eq!(meshes(&ground.object), 0);
    assert!(castle.has(g::CASTLE_GROUND_ANIM));
}

/// RE-365: the Castle ground root follows `map_nodes`' own table, replayed
/// on the archive file: X sweeps 0, -1050, 1050, 0 and loops.
#[test]
fn packed_castle_ground_replays_the_rom() {
    use ssb_game::stage::castle::Castle;
    use ssb_game::stage::{StageInit, StageKind};
    use ssb_rom::ground_obj::{self as g, GroundObjects};
    use ssb_rom::pack::Pack;
    let Some((bytes, rom)) = pack_and_rom() else {
        return;
    };
    let pack = Pack::open(&bytes).unwrap();
    let info = ssb_rom::rom::identify(&rom).unwrap();
    let archive = Archive::open(&rom, info.region).unwrap();
    // `map_nodes` for `llGRCastleMapMapHead` (0x0).
    let file = archive.load(156).unwrap();
    let script = joint_scripts(&file.data, 0x0, 1)[0].unwrap();
    let mut r = StageJoint::start_changed(script, 0.0);
    let mut pose = JointPose::default();

    let mut objects = GroundObjects::new(&pack, g::CASTLE_FILE);
    let init = StageInit {
        kind: StageKind::Castle,
        map_objects: &[],
        bound_bottom: 0.0,
        hazard_attack: None,
        hazard_throw: None,
        acid_surface_y: 0.0,
    };
    let mut castle = Castle::new(
        &init,
        &mut ListPort {
            pack: &pack,
            objects: &mut objects,
        },
        &mut NoItems,
    );
    r.tick(&file.data, 1.0, &mut pose).unwrap();
    let (mut low, mut high) = (f32::MAX, f32::MIN);
    for tick in 0..6000 {
        if tick > 0 {
            objects.advance(&pack).unwrap();
            r.tick(&file.data, 1.0, &mut pose).unwrap();
        }
        let x = objects.get(g::CASTLE_GROUND).unwrap().translate()[0];
        assert_eq!(x.to_bits(), pose.translate[0].to_bits(), "tick {tick}");
        castle.bumper = Some(0);
        castle.tick(
            &ListPort {
                pack: &pack,
                objects: &mut objects,
            },
            &mut NoItems,
        );
        assert_eq!(castle.bumper_x, x + castle.bumper_pos.x);
        low = low.min(x);
        high = high.max(x);
    }
    assert_eq!((low, high), (-1050.0, 1050.0));
}

/// RE-365: `llGRInishieMapScaleRetractAnimJoint` blinks one platform's
/// subtree (flags 2 then 0, two ticks each) as its ROM replay does, and
/// the controller's stop clears it. The Inishie controller places both
/// platforms at their map objects and hangs each string node from its
/// platform.
#[test]
fn packed_scales_follow_the_inishie_controller() {
    use ssb_game::stage::inishie::{Inishie, STRING_NODES};
    use ssb_game::stage::{MapObject, StageInit, StageKind};
    use ssb_rom::ground_obj::{self as g, GroundObjects};
    use ssb_rom::pack::Pack;
    let Some((bytes, rom)) = pack_and_rom() else {
        return;
    };
    let pack = Pack::open(&bytes).unwrap();
    let info = ssb_rom::rom::identify(&rom).unwrap();
    let archive = Archive::open(&rom, info.region).unwrap();
    let file = archive.load(155).unwrap();

    let mut objects = GroundObjects::new(&pack, g::INISHIE_FILE);
    objects.play_on(&pack, g::SCALE_RETRACT, 1).unwrap();
    let mut r = StageJoint::start_changed(0x734, 0.0);
    let mut pose = JointPose::default();
    r.tick(&file.data, 1.0, &mut pose).unwrap();
    let mut hidden = 0;
    // Tick 40 ends hidden, so the stop below has a flag to clear.
    for tick in 0..41 {
        if tick > 0 {
            objects.advance(&pack).unwrap();
            r.tick(&file.data, 1.0, &mut pose).unwrap();
        }
        let platform = objects.instance(g::SCALE_PLATFORM, 1).unwrap();
        assert_eq!(platform.flags(0), r.flags, "tick {tick}");
        assert_eq!(platform.visible(&pack, 0), r.flags & 2 == 0);
        hidden += usize::from(!platform.visible(&pack, 0));
        assert!(objects
            .instance(g::SCALE_PLATFORM, 0)
            .unwrap()
            .visible(&pack, 0));
    }
    assert_eq!(hidden, 21);
    assert_eq!(objects.instance(g::SCALE_PLATFORM, 1).unwrap().flags(0), 2);
    objects.instance_mut(g::SCALE_PLATFORM, 1).unwrap().stop();
    for _ in 0..4 {
        objects.advance(&pack).unwrap();
        let platform = objects.instance(g::SCALE_PLATFORM, 1).unwrap();
        assert_eq!(platform.flags(0), 0);
    }

    let s = pack
        .stage(pack.stage_of_file(g::INISHIE_FILE).unwrap())
        .unwrap();
    let map_objects: Vec<MapObject> = pack
        .stage_points(&s)
        .map(|p| MapObject {
            kind: p.kind,
            pos: ssb_engine::math::Vec3::new(p.x as f32, p.y as f32, 0.0),
        })
        .collect();
    let init = StageInit {
        kind: StageKind::Inishie,
        map_objects: &map_objects,
        bound_bottom: s.bounds.bottom as f32,
        hazard_attack: None,
        hazard_throw: None,
        acid_surface_y: 0.0,
    };
    let mut objects = GroundObjects::new(&pack, g::INISHIE_FILE);
    let mut groups = vec![ssb_game::map::MapGroup::default(); 4];
    let mut inishie = Inishie::new(
        &init,
        &mut groups,
        &mut ListPort {
            pack: &pack,
            objects: &mut objects,
        },
        &mut NoItems,
    );
    // `map_dobjs[0].y + map_dobjs[3].y` and `map_dobjs[0].y + map_dobjs[1].y`.
    assert_eq!(inishie.string_length[0], 2010.0 + -57.750_09);
    assert_eq!(inishie.string_length[1], 2010.0 + -57.751_007);
    let line_group = |_: u16| None;
    inishie.tick(
        &[],
        &mut groups,
        &mut ListPort {
            pack: &pack,
            objects: &mut objects,
        },
        &mut NoItems,
        &ssb_game::stage::MapQuery {
            surfaces: || core::iter::empty::<ssb_game::weapon::MapSurface>(),
            line_group: &line_group,
        },
        false,
        &mut ssb_game::stage::Registry::default(),
    );
    let strings = objects.get(g::SCALE_STRINGS).unwrap();
    for i in 0..2u8 {
        let platform = objects.instance(g::SCALE_PLATFORM, i).unwrap();
        let t = platform.translate();
        assert_eq!(t[0], inishie.platform[i as usize].x);
        assert_eq!(t[1], inishie.platform[i as usize].y);
        assert_ne!(t[1], 0.0);
        // The string node's world Y is its platform's.
        let node = STRING_NODES[i as usize] as usize;
        let parent = if node == 4 { 3 } else { 1 };
        let world = strings.node_translate(0).unwrap()[1]
            + strings.node_translate(parent).unwrap()[1]
            + strings.node_translate(node).unwrap()[1];
        assert!((world - t[1]).abs() < 1e-2, "{world} {}", t[1]);
    }
}

/// The runtime port's Arwing half (`psp-runtime::scene::StageObjectsPort`),
/// for the pack's Sector Z Arwing (RE-428).
struct ArwingPort<'a, 'p> {
    pack: &'a ssb_rom::pack::Pack<'p>,
    objects: &'a mut ssb_rom::ground_obj::GroundObjects,
}

fn script(anim: ssb_game::stage::sector::ArwingAnim) -> ssb_rom::sector::Script {
    use ssb_game::stage::sector::ArwingAnim as A;
    use ssb_rom::sector::Script as S;
    match anim {
        A::Flight { pattern, field } => S::Flight { pattern, field },
        A::Pilot(id) => S::Pilot(id),
        A::LaserCharge => S::LaserCharge,
        A::LaserFire => S::LaserFire,
        A::Flare => S::Flare,
        A::Glow => S::Glow,
    }
}

fn v3([x, y, z]: [f32; 3]) -> ssb_engine::math::Vec3 {
    ssb_engine::math::Vec3::new(x, y, z)
}

impl ArwingPort<'_, '_> {
    fn a(&self) -> &ssb_rom::sector::Arwing {
        self.objects.arwing.as_ref().expect("arwing packed")
    }
    fn a_mut(&mut self) -> &mut ssb_rom::sector::Arwing {
        self.objects.arwing.as_mut().expect("arwing packed")
    }
}

impl ssb_game::stage::sector::ArwingObject for ArwingPort<'_, '_> {
    fn add_anim(&mut self, node: u8, anim: Option<ssb_game::stage::sector::ArwingAnim>) {
        let pack = self.pack;
        self.a_mut()
            .add_anim(pack, node as usize, anim.map(script))
            .expect("arwing script parses");
    }
    fn add_anim_joint(&mut self, node: u8, anim: ssb_game::stage::sector::ArwingAnim) {
        let pack = self.pack;
        self.a_mut()
            .add_anim_joint(pack, node as usize, script(anim));
    }
    fn play_all(&mut self) {
        let pack = self.pack;
        self.a_mut().play_all(pack).expect("arwing scripts parse");
    }
    fn anim_null(&self, node: u8) -> bool {
        self.a().anim_null(node as usize)
    }
    fn stop(&mut self, node: u8) {
        self.a_mut().stop(node as usize);
    }
    fn flags(&self, node: u8) -> u16 {
        self.a().flags(node as usize)
    }
    fn set_flags(&mut self, node: u8, flags: u16) {
        self.a_mut().set_flags(node as usize, flags);
    }
    fn set_hidden(&mut self, hidden: bool) {
        self.a_mut().hidden = hidden;
    }
    fn translate(&self, node: u8) -> ssb_engine::math::Vec3 {
        v3(self.a().translate(node as usize))
    }
    fn set_translate(&mut self, node: u8, t: ssb_engine::math::Vec3) {
        self.a_mut().set_translate(node as usize, [t.x, t.y, t.z]);
    }
    fn rotate(&self, node: u8) -> ssb_engine::math::Vec3 {
        v3(self.a().rotate(node as usize))
    }
    fn set_rotate(&mut self, node: u8, r: ssb_engine::math::Vec3) {
        self.a_mut().set_rotate(node as usize, [r.x, r.y, r.z]);
    }
    fn path_fraction(&self, node: u8) -> Option<f32> {
        self.a().path_fraction(node as usize)
    }
    fn path_tangent(&self, node: u8, t: f32) -> Option<ssb_engine::math::Vec3> {
        self.a().path_tangent(self.pack, node as usize, t).map(v3)
    }
    fn path_point(&self, node: u8, t: f32) -> Option<ssb_engine::math::Vec3> {
        self.a().path_point(self.pack, node as usize, t).map(v3)
    }
    fn set_root(&mut self, m: [[f32; 4]; 4]) {
        self.a_mut().root = ssb_rom::scene::Mat4(core::array::from_fn(|i| m[i / 4][i % 4]));
    }
}

impl StageObjects for ArwingPort<'_, '_> {
    fn arwing(&mut self) -> Option<&mut dyn ssb_game::stage::sector::ArwingObject> {
        Some(self)
    }
}

/// RE-428: the Sector Z controller flies the packed Arwing. Every pattern
/// starts visible on its path, the plane patterns pass the stage plane with
/// the wing's collision group on (never a background one, which may pass
/// as close), every fourth pattern is a background one, and every pattern
/// ends hidden with the wing off.
#[test]
fn packed_arwing_flies_the_sector_controller() {
    use ssb_game::map::{GroupStatus, MapGroup};
    use ssb_game::stage::sector::{node, ArwingStatus, WING_GROUP};
    use ssb_game::stage::{Controller, Stage, StageInit, StageKind, TickInput};
    use ssb_rom::ground_obj::GroundObjects;
    let Some((bytes, _)) = pack_and_rom() else {
        return;
    };
    let pack = ssb_rom::pack::Pack::open(&bytes).unwrap();
    let s = pack
        .stage(pack.stage_of_file(ssb_rom::sector::MAP_FILE).unwrap())
        .unwrap();
    let mut objects = GroundObjects::new(&pack, s.source_file);
    assert!(objects.arwing.is_some(), "the pack carries the Arwing");
    let init = StageInit {
        kind: StageKind::Sector,
        map_objects: &[],
        bound_bottom: s.bounds.bottom as f32,
        hazard_attack: None,
        hazard_throw: None,
        acid_surface_y: 0.0,
    };
    let mut groups = vec![MapGroup::default(); 4];
    ssb_game::rng::set_seed(0x5EC7);
    let mut stage = Stage::new(
        &init,
        &mut groups,
        &mut ArwingPort {
            pack: &pack,
            objects: &mut objects,
        },
        &mut NoItems,
    );
    assert_eq!(groups[WING_GROUP as usize].status, GroupStatus::Off);
    let line_group = |_: u16| None;
    // Ahead of the plane patterns, level with their cruise: in the 2D
    // volley's window (`grSectorArwingGetLaserAmmoCount`).
    let mut fighter = Fighter::new(FighterKind::Mario, 0, 3);
    fighter.pos = ssb_engine::math::Vec3::new(-9000.0, 2400.0, 0.0);
    let mut patterns = Vec::new();
    let mut was_patrol = false;
    let mut near_z = (f32::MAX, f32::MAX);
    let mut wing_on = 0;
    let mut lasers = 0;
    for frame in 0..60 * 60 * 6 {
        objects.advance(&pack).unwrap();
        let mut fighters = [&mut fighter];
        stage.tick(
            &mut fighters,
            TickInput {
                groups: &mut groups,
                objects: &mut ArwingPort {
                    pack: &pack,
                    objects: &mut objects,
                },
                items: &mut NoItems,
                map: ssb_game::stage::MapQuery {
                    surfaces: || core::iter::empty::<ssb_game::weapon::MapSurface>(),
                    line_group: &line_group,
                },
                started: true,
            },
        );
        for l in stage.take_lasers() {
            lasers += 1;
            if !l.three_d {
                assert_eq!(l.velocity, ssb_engine::math::Vec3::new(-230.0, 0.0, 0.0));
            }
        }
        let Controller::Sector(c) = &stage.controller else {
            panic!("Sector has its controller");
        };
        let a = objects.arwing.as_ref().unwrap();
        let patrol = c.status == ArwingStatus::Patrol;
        if patrol && !was_patrol {
            assert!(!a.hidden, "frame {frame}: a pattern starts visible");
            assert!(a.path_fraction(node::PATH as usize).is_some());
            patterns.push((frame, c.laser_count, c.target_x));
            near_z = (f32::MAX, f32::MAX);
        }
        if patrol {
            let t = a.translate(node::PATH as usize);
            assert!(t.iter().all(|v| v.is_finite()), "frame {frame}: {t:?}");
            let slot = if c.laser_count == 2 {
                &mut near_z.0
            } else {
                &mut near_z.1
            };
            *slot = slot.min(t[2].abs());
            if groups[WING_GROUP as usize].status == GroupStatus::On {
                wing_on += 1;
                assert_eq!(c.laser_count, 2, "only the plane patterns land the wing");
            }
        }
        if !patrol && was_patrol {
            assert!(a.hidden, "frame {frame}: a pattern ends hidden");
            assert_eq!(groups[WING_GROUP as usize].status, GroupStatus::Off);
            let (start, count, x) = patterns.last().copied().unwrap();
            eprintln!(
                "pattern from {start} to {frame}: laser_count {count} target_x {x} min |z| plane {} background {}",
                near_z.0, near_z.1
            );
            if count == 2 {
                assert!(near_z.0 < 200.0, "a plane pattern crosses the stage plane");
            }
        }
        was_patrol = patrol;
    }
    // Sleep ends on the first started frame, then 600 frames of wait.
    assert_eq!(patterns.first().map(|p| p.0), Some(601));
    assert!(patterns.len() >= 8, "{patterns:?}");
    for (i, p) in patterns.iter().enumerate() {
        assert_eq!(
            p.1 == 0,
            i % 4 == 3,
            "every fourth pattern sweeps in: {patterns:?}"
        );
    }
    assert!(wing_on > 0);
    assert!(
        lasers > 0 && lasers % 2 == 0,
        "the 2D volleys fire pairs: {lasers}"
    );
    eprintln!(
        "{} patterns, {wing_on} wing frames, {lasers} lasers",
        patterns.len()
    );
}

/// RE-428: node 0's path, played from the pack, against the N64. A
/// Training-on-Sector-Z warp build traced `map_dobjs[0]->translate` every
/// frame (Mupen64Plus, 9,500 frames): patterns 0, 4, 2 and 5 started at
/// frames 601, 2271, 4080 and 8032, and every traced frame of each matched
/// this playback to four decimals. These are frames `k` after the pattern
/// started (`func_ovl2_80107D50`'s parse and play is `k` 0).
#[test]
fn packed_arwing_paths_match_the_n64_trace() {
    use ssb_rom::sector::{Arwing, Script};
    let Some((bytes, _)) = pack_and_rom() else {
        return;
    };
    let pack = ssb_rom::pack::Pack::open(&bytes).unwrap();
    // The trace's printed values, to four decimals.
    let traced: [(u8, usize, [f64; 3]); 6] = [
        (0, 300, [1999.1096, 4500.0, -0.0002]),
        (4, 300, [-1037.0774, 2311.9409, 0.0]),
        (2, 1150, [-1122.0406, 2500.9065, 0.0]),
        (2, 2166, [10827.8457, -810.501, 8400.0]),
        (5, 200, [-1991.5497, 3943.3335, 3276.9719]),
        (5, 398, [-12299.9893, 13141.6191, 19200.002]),
    ];
    for (pattern, k, want) in traced {
        let mut a = Arwing::new(&pack).expect("arwing packed");
        a.add_anim(&pack, 0, Some(Script::Flight { pattern, field: 0 }))
            .unwrap();
        for _ in 0..k {
            a.play_all(&pack).unwrap();
        }
        let got = a.translate(0);
        for i in 0..3 {
            assert!(
                (f64::from(got[i]) - want[i]).abs() < 0.001,
                "pattern {pattern} frame {k}: {got:?} vs {want:?}"
            );
        }
    }
}

/// RE-429: the Castle Bumper is the exact GBumper attribute extern target,
/// not the separate horizontal tree at 0x7BE8 (RE-162).
#[test]
fn castle_bumper_graph_matches_its_rom_attribute() {
    let Some((bytes, rom)) = pack_and_rom() else {
        return;
    };
    let pack = ssb_rom::pack::Pack::open(&bytes).unwrap();
    let info = ssb_rom::rom::identify(&rom).unwrap();
    let archive = Archive::open(&rom, info.region).unwrap();
    let attr = archive.load(251).unwrap();
    let data = attr.extern_relocs.iter().find(|r| r.at == 0xCF0).unwrap();
    assert_eq!(
        (u32::from(data.target_file), data.target_offset),
        ssb_rom::ground_obj::GBUMPER_SOURCE
    );
    let pickup = attr.extern_relocs.iter().find(|r| r.at == 0x69C).unwrap();
    assert_eq!(
        (pickup.target_file, pickup.target_offset),
        (data.target_file, data.target_offset)
    );
    let materials = attr.extern_relocs.iter().find(|r| r.at == 0xCF4).unwrap();
    assert_eq!(
        (materials.target_file, materials.target_offset),
        (86, 0x7488)
    );
    let object = (0..pack.object_count())
        .filter_map(|i| pack.object(i))
        .find(|o| (o.source_file, o.source_offset) == ssb_rom::ground_obj::GBUMPER_SOURCE)
        .unwrap();
    let root = pack.node(object.first_node).unwrap();
    assert_eq!(root.mesh, ssb_rom::pack::NodeDesc::NO_MESH);
    assert_eq!(root.rest_translate, [0.0; 3]);
    assert_ne!(
        pack.node(object.first_node + 1).unwrap().mesh,
        ssb_rom::pack::NodeDesc::NO_MESH
    );
}

/// RE-429: the Mushroom Kingdom item trees. `itManagerMakeItem` ejects each
/// descriptor's empty root, so the item's root is node 1: the POW Block's
/// `anim_joints` pop-in (file 155 + 0x13B8) plays there and settles the
/// block on its 21st play, its squash (0x1288) ends on the 23rd, and the
/// Piranha Plant's rise (0xCC8) writes the root's Y as the ROM replay does
/// and ends on its 141st play. Both material scripts are packed.
#[test]
fn packed_item_trees_play_on_the_ejected_root() {
    use ssb_rom::ground_obj::{self as g, GroundObjects};
    use ssb_rom::pack::Pack;
    let Some((bytes, rom)) = pack_and_rom() else {
        return;
    };
    let pack = Pack::open(&bytes).unwrap();
    let info = ssb_rom::rom::identify(&rom).unwrap();
    let archive = Archive::open(&rom, info.region).unwrap();
    let file = archive.load(155).unwrap();
    let mut objects = GroundObjects::new(&pack, g::INISHIE_FILE);
    for (anim, mat) in [
        (g::POWER_BLOCK_APPEAR, None),
        (g::POWER_BLOCK_DAMAGE, None),
        (g::PAKKUN_APPEAR, Some(g::PAKKUN_APPEAR_MAT)),
    ] {
        assert!(objects.has(anim), "{}", g::ANIMS[anim].name);
        if let Some(m) = mat {
            assert!(objects.has_mat(m), "{}", g::MAT_ANIMS[m].name);
        }
    }
    assert!(objects.has_mat(g::PAKKUN_DAMAGED_MAT));
    for i in 0..2 {
        let p = objects.instance(g::PAKKUN, i).expect("both plants");
        assert!(p.hidden);
        assert_eq!(p.node_count(), 2);
    }

    // The pop-in: the make's play, then one per process.
    objects.item_make(&pack, g::POWER_BLOCK, 0);
    let mut plays = 1;
    while !objects.item_root_idle(g::POWER_BLOCK, 0) {
        assert_eq!(objects.item_play(&pack, g::POWER_BLOCK, 0), [None; 3]);
        plays += 1;
        assert!(plays < 100);
    }
    assert_eq!(plays, 21);
    let mut plays = 1;
    objects.item_add_play(&pack, Some(g::POWER_BLOCK_DAMAGE), None, g::POWER_BLOCK, 0);
    while !objects.item_root_idle(g::POWER_BLOCK, 0) {
        objects.item_play(&pack, g::POWER_BLOCK, 0);
        plays += 1;
        assert!(plays < 100);
    }
    assert_eq!(plays, 23);

    objects.item_make(&pack, g::PAKKUN, 1);
    assert!(objects.item_root_idle(g::PAKKUN, 1));
    let mut rom_joint = StageJoint::start_changed(0xCC8, 0.0);
    let mut pose = JointPose::default();
    rom_joint.tick(&file.data, 1.0, &mut pose).unwrap();
    let first = objects.item_add_play(
        &pack,
        Some(g::PAKKUN_APPEAR),
        Some(g::PAKKUN_APPEAR_MAT),
        g::PAKKUN,
        1,
    );
    assert_eq!(first, [None, Some(pose.translate[1]), None]);
    let mut plays = 1;
    let mut top = 0.0f32;
    while !objects.item_root_idle(g::PAKKUN, 1) {
        let w = objects.item_play(&pack, g::PAKKUN, 1);
        rom_joint.tick(&file.data, 1.0, &mut pose).unwrap();
        assert_eq!(
            w[1].map(f32::to_bits),
            Some(pose.translate[1].to_bits()),
            "play {plays}"
        );
        top = top.max(pose.translate[1]);
        plays += 1;
        assert!(plays < 400);
    }
    assert_eq!(plays, 141);
    assert_eq!(top, 596.39996);
    // Instance 0 never moved.
    assert!(objects.item_root_idle(g::PAKKUN, 0));

    // The item descriptor supplies kind 48 even if the graph has no high
    // bits. A knockout switches to custom kind 70, which persists after
    // rebirth; the spin alone resets (itPakkunDamagedProcDead).
    let draw_root = |objects: &g::GroundObjects| {
        let plant = objects.instance(g::PAKKUN, 1).unwrap();
        let mut posed = [ssb_rom::scene::Mat4::IDENTITY; g::MAX_OBJECT_NODES];
        plant.compose(&pack, &mut posed);
        let mut node = pack
            .node(plant.object.first_node + g::ITEM_ROOT as u32)
            .unwrap();
        node.world = posed[g::ITEM_ROOT].0;
        plant.draw_node(g::ITEM_ROOT, node)
    };
    use ssb_rom::pack::NodeDesc;
    assert_eq!(
        draw_root(&objects).flags,
        NodeDesc::FLAG_BILLBOARD | NodeDesc::FLAG_BILLBOARD_PITCH_LOCKED
    );
    objects.item_add_play(&pack, None, Some(g::PAKKUN_DAMAGED_MAT), g::PAKKUN, 1);
    objects.item_place(g::PAKKUN, 1, [2800.0, 1000.0, 0.0], core::f32::consts::PI);
    let node = draw_root(&objects);
    assert_eq!(
        node.flags,
        NodeDesc::FLAG_BILLBOARD | NodeDesc::FLAG_BILLBOARD_SPIN_Z
    );
    assert_eq!(node.billboard_rest_spin(), core::f32::consts::PI);
    assert_eq!([node.world[0], node.world[5], node.world[10]], [1.0; 3]);
    objects.item_stop_material(g::PAKKUN, 1);
    objects.item_place(g::PAKKUN, 1, [2800.0, 0.0, 0.0], 0.0);
    assert_eq!(draw_root(&objects).billboard_rest_spin(), 0.0);
    assert_eq!(draw_root(&objects).flags, node.flags);
}
