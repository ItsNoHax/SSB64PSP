//! Platform selection, parent geometry and independent child/Bumper clocks
//! compared with the user's US ROM. No ROM-derived data in source.
use ssb_rom::{
    archive::Archive,
    bonus2 as b,
    figatree::JointPose,
    objanim::StageJoint,
    pack::{AnimJoint, Pack},
    skeleton::StageAnimator,
};

fn assets() -> Option<(Vec<u8>, Vec<u8>)> {
    let rom = std::fs::read(std::env::var_os("SSB64_ROM")?).unwrap();
    let pack = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/generated/ssb64.pak"),
    )
    .unwrap();
    Some((rom, pack))
}
fn object(p: &Pack<'_>, file: u32, offset: u32) -> ssb_rom::pack::ObjectDesc {
    (0..p.object_count())
        .filter_map(|i| p.object(i))
        .find(|o| (o.source_file, o.source_offset) == (file, offset))
        .unwrap()
}

#[test]
fn all_platform_courses_have_ten_distinct_detect_groups_and_original_geometry() {
    let Some((rom, bytes)) = assets() else { return };
    let a = Archive::open(&rom, ssb_rom::rom::identify(&rom).unwrap().region).unwrap();
    let p = Pack::open(&bytes).unwrap();
    for kind in 0..12 {
        let stage = p.stage(p.stage_of_file(283 + kind).unwrap()).unwrap();
        let floors = b::platforms(&p, &stage);
        assert_eq!(floors.len(), 10, "course {kind}");
        let mut groups: Vec<_> = floors.iter().map(|f| f.group).collect();
        groups.sort();
        groups.dedup();
        assert_eq!(groups.len(), 10, "one child tree per yakumono");
        assert_eq!(
            groups,
            (1..=10).collect::<Vec<_>>(),
            "platform parents precede the course geometry"
        );
        assert!(p
            .spawn(&stage, ssb_game::spgame::setup::mapobj::PLAYER)
            .is_some());
        let raw = a.load(stage.source_file).unwrap();
        let g = ssb_rom::stage::find_ground_data(&raw, |_, _| true)
            .into_iter()
            .find(|g| g.offset == stage.source_offset)
            .unwrap();
        let (file, geometry) = g.map_geometry.unwrap();
        let map = ssb_rom::collision::read(&a.load(file).unwrap(), geometry).unwrap();
        let parent = p.object(stage.layers[1]).unwrap();
        assert_eq!(
            p.node(parent.first_node).unwrap().mesh,
            ssb_rom::pack::NodeDesc::NO_MESH
        );
        for floor in floors {
            let line = map.lines.iter().find(|l| l.id == floor.line).unwrap();
            assert_eq!(line.yakumono, floor.group);
            assert_eq!(line.points[0].flags & 0xff, 14);
            let width = (i32::from(line.points.last().unwrap().pos[0])
                - i32::from(line.points[0].pos[0]))
            .unsigned_abs() as f32;
            assert_eq!(floor.width, width);
            let node = p.node(parent.first_node + u32::from(floor.group)).unwrap();
            assert_eq!(node.mesh, ssb_rom::pack::NodeDesc::NO_MESH);
            assert_eq!(node.parent, parent.first_node);
            assert_eq!(node.rest_rotate, [0.0; 3]);
            assert_eq!(node.rest_scale, [1.0; 3]);
        }
    }
}

#[test]
fn all_six_platform_trees_replay_original_joint_streams_and_restart_when_boarded() {
    let Some((rom, bytes)) = assets() else { return };
    let a = Archive::open(&rom, ssb_rom::rom::identify(&rom).unwrap().region).unwrap();
    let raw = a.load(b::FILE).unwrap();
    let p = Pack::open(&bytes).unwrap();
    for (tree, labels) in b::TREES.iter().enumerate() {
        let obj = b::object(&p, tree).unwrap();
        assert!(obj.node_count <= 8);
        let clip = p.item_anim(b::FIRST_ANIM + tree as u32).unwrap();
        let scripts =
            ssb_rom::objanim::joint_scripts(&raw.data, labels[1], obj.node_count as usize);
        let mut anim = StageAnimator::new();
        anim.start_changed(&p, &clip);
        let mut expected: Vec<_> = scripts
            .iter()
            .enumerate()
            .filter_map(|(i, s)| {
                s.map(|s| {
                    let node = p.node(obj.first_node + i as u32).unwrap();
                    (
                        obj.first_node + i as u32,
                        StageJoint::start_changed(s, 0.0),
                        JointPose {
                            translate: node.rest_translate,
                            rotate: node.rest_rotate,
                            scale: node.rest_scale,
                        },
                    )
                })
            })
            .collect();
        assert!(!expected.is_empty());
        for tick in 0..600 {
            anim.tick(p.anim_script(&clip).unwrap()).unwrap();
            for (node, joint, pose) in &mut expected {
                joint.tick(&raw.data, 1.0, pose).unwrap();
                assert_eq!(
                    anim.node_pose(*node),
                    Some(&*pose),
                    "tree {tree} tick {tick}"
                );
            }
        }
        if tree < 3 {
            let mut visual = b::Visual::new(&p, 1, tree as u8).unwrap();
            let materials =
                ssb_rom::mobj::read_table(&raw, labels[2], obj.node_count as usize).unwrap();
            let source =
                ssb_rom::matanim::resolve_scripts(&raw, labels[3], materials.nodes.len(), |n| {
                    materials.nodes[n].len()
                });
            let pack = &p;
            let mut slots: Vec<_> = (0..obj.node_count)
                .filter_map(|i| p.node(obj.first_node + i))
                .filter_map(|n| p.mesh(n.mesh))
                .flat_map(|m| (0..m.prim_count).filter_map(move |i| pack.prim(m.first_prim + i)))
                .map(|p| p.mat_anim)
                .filter(|&i| i != u32::MAX)
                .collect();
            slots.sort();
            slots.dedup();
            assert!(!slots.is_empty(), "platform material must animate");
            let mut refs: Vec<_> = slots
                .into_iter()
                .map(|i| {
                    let script = p.mat_anim(i).unwrap().script;
                    assert!(source.iter().flatten().any(|s| *s == Some(script)));
                    (i, ssb_rom::matanim::MaterialJoint::start(script, 0.0))
                })
                .collect();
            for tick in 0..600 {
                for (slot, reference) in &mut refs {
                    reference.tick(&raw.data, 1.0).unwrap();
                    let live = visual.materials.joint_for(*slot).unwrap();
                    for track in 0..ssb_rom::matanim::TICK_TRACK_COUNT {
                        assert_eq!(
                            live.track_value(track).map(f32::to_bits),
                            reference.track_value(track).map(f32::to_bits),
                            "tree {tree} material tick {tick} track {track}"
                        );
                    }
                }
                visual.tick(&p).unwrap();
            }
            visual.board(&p);
            assert!(visual.materials.is_empty());
            let boarded = b::object(&p, tree + 3).unwrap();
            assert_eq!(visual.object, boarded);
            let mut fresh = StageAnimator::new();
            let clip = p.item_anim(b::FIRST_ANIM + tree as u32 + 3).unwrap();
            fresh.start_changed(&p, &clip);
            fresh.tick(p.anim_script(&clip).unwrap()).unwrap();
            for i in 0..fresh.joint_count() {
                assert_eq!(visual.anim.joint(i), fresh.joint(i));
            }
        }
    }
}

#[test]
fn every_course_bumper_uses_shared_item_tree_and_independent_rom_root_clock() {
    let Some((rom, bytes)) = assets() else { return };
    let a = Archive::open(&rom, ssb_rom::rom::identify(&rom).unwrap().region).unwrap();
    let p = Pack::open(&bytes).unwrap();
    let model = object(
        &p,
        ssb_rom::ground_obj::GBUMPER_SOURCE.0,
        ssb_rom::ground_obj::GBUMPER_SOURCE.1,
    );
    let mut count = 0;
    for (kind, labels) in b::BUMPERS.iter().enumerate() {
        let mut objects = ssb_rom::ground_obj::GroundObjects::new(&p, 283 + kind as u32);
        let Some((graph, table)) = labels else {
            assert_eq!(objects.iter().count(), 0);
            continue;
        };
        let raw = a.load(137 + kind as u32).unwrap();
        let placements = object(&p, raw.id, *graph);
        assert!(placements.node_count - 1 <= ssb_rom::ground_obj::MAX_STAGE_OBJECTS as u32);
        assert_eq!(objects.iter().count(), placements.node_count as usize - 1);
        let scripts =
            ssb_rom::objanim::joint_scripts(&raw.data, *table, placements.node_count as usize);
        for (i, script) in scripts.into_iter().skip(1).enumerate() {
            count += 1;
            let asset = b::FIRST_BUMPER_ASSET + kind as u8;
            objects.item_make(&p, asset, i as u8);
            assert_eq!(objects.instance(asset, i as u8).unwrap().object, model);
            let clip = p.item_anim(b::BUMPER_ANIM + kind as u32 * 10 + i as u32);
            assert_eq!(clip.is_some(), script.is_some());
            let Some(script) = script else { continue };
            let joint = p.anim_joint(clip.unwrap().first_joint).unwrap();
            assert_ne!(joint.script, AnimJoint::NO_SCRIPT);
            assert_eq!(joint.node, model.first_node + 1);
            let mut expected = StageJoint::start_changed(script, 0.0);
            let mut pose = JointPose::default();
            expected.tick(&raw.data, 1.0, &mut pose).unwrap();
            for tick in 0..600 {
                assert_eq!(
                    objects.instance(asset, i as u8).unwrap().pose(1),
                    Some(&pose),
                    "course {kind} Bumper {i} tick {tick}"
                );
                expected.tick(&raw.data, 1.0, &mut pose).unwrap();
                objects.item_play(&p, asset, i as u8);
            }
        }
    }
    assert!(count > 0);
}
