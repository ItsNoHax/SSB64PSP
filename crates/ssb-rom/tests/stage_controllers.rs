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
