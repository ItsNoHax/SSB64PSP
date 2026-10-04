//! Normal-container constants and the Egg's packed animation, from the US ROM.
use ssb_game::item::container::{
    BARREL_ATTRIBUTES, CAPSULE_ATTRIBUTES, CAPSULE_EVENTS, CRATE_ATTRIBUTES, EGG_ATTRIBUTES,
    EGG_EVENTS, HEAVY_EVENTS,
};
use ssb_game::item::utility::{HEART_ATTRIBUTES, STAR_ATTRIBUTES, TOMATO_ATTRIBUTES};
use ssb_game::item::{bombhei, mball, msbomb, nbumper, shell, ItemKind};
use ssb_rom::{
    archive::Archive,
    figatree::JointPose,
    objanim::StageJoint,
    pack::{AnimDesc, Pack},
    skeleton::StageAnimator,
};
fn word(d: &[u8], at: usize) -> u32 {
    u32::from_be_bytes(d[at..at + 4].try_into().unwrap())
}
fn half(d: &[u8], at: usize) -> i16 {
    i16::from_be_bytes(d[at..at + 2].try_into().unwrap())
}
fn rom() -> Option<Vec<u8>> {
    std::env::var_os("SSB64_ROM").map(|p| std::fs::read(p).unwrap())
}

#[test]
fn utility_models_are_present_in_the_pack() {
    let Some(rom) = rom() else {
        return;
    };
    let archive = Archive::open(&rom, ssb_rom::rom::identify(&rom).unwrap().region).unwrap();
    let file = archive.load(251).unwrap();
    let bytes = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/generated/ssb64.pak"),
    )
    .unwrap();
    let p = Pack::open(&bytes).unwrap();
    // Tomato, Heart, Star: `ITAttributes::data` in file 86.
    for (offset, graph) in [(0xB8, 0xAB0), (0x100, 0x1158), (0x148, 0x1560)] {
        let data = file.extern_relocs.iter().find(|r| r.at == offset).unwrap();
        assert_eq!((data.target_file, data.target_offset), (86, graph));
        let object = (0..p.object_count())
            .filter_map(|i| p.object(i))
            .find(|o| (o.source_file, o.source_offset) == (86, graph))
            .unwrap();
        // Descriptor 0 is the placeholder `itManagerMakeItem` ejects.
        assert_eq!(object.node_count, 2);
    }
}

#[test]
fn container_attributes_and_explosion_tables_match_the_rom() {
    let Some(rom) = rom() else {
        return;
    };
    let archive = Archive::open(&rom, ssb_rom::rom::identify(&rom).unwrap().region).unwrap();
    let file = archive.load(251).unwrap();
    for (offset, attr, spin) in [
        (0x50, &CAPSULE_ATTRIBUTES, 120),
        (0xACC, &EGG_ATTRIBUTES, 100),
        (0x5CC, &CRATE_ATTRIBUTES, 40),
        (0x634, &BARREL_ATTRIBUTES, 0),
        (0xB8, &TOMATO_ATTRIBUTES, 100),
        (0x100, &HEART_ATTRIBUTES, 0),
        (0x148, &STAR_ATTRIBUTES, 0),
        (0x3BC, &msbomb::ATTRIBUTES, 120),
        (0x424, &bombhei::ATTRIBUTES, 0),
        (0x53C, &shell::GREEN_ATTRIBUTES, 0),
        (0x584, &shell::RED_ATTRIBUTES, 0),
        (0x69C, &nbumper::ATTRIBUTES, 70),
        (0x6E4, &mball::ATTRIBUTES, 20),
        (0x190, &ssb_game::item::equipment::ATTRIBUTES[0], 100),
        (0x1D8, &ssb_game::item::equipment::ATTRIBUTES[1], 100),
        (0x220, &ssb_game::item::equipment::ATTRIBUTES[2], 50),
        (0x48C, &ssb_game::item::equipment::ATTRIBUTES[3], 110),
        (0x268, &ssb_game::item::equipment::ATTRIBUTES[4], 140),
        (0x2E4, &ssb_game::item::equipment::ATTRIBUTES[5], 0),
        (0x374, &ssb_game::item::equipment::ATTRIBUTES[6], 0),
    ] {
        let d = &file.data[offset..offset + 72];
        let light = attr.weight == ssb_game::item::ItemWeight::Light;
        assert_eq!((word(d, 16) >> 27) & 3, if light { 3 } else { 2 });
        assert!(attr.is_give_hitlag);
        assert_eq!((word(d, 16) >> 29) & 1 == 1, attr.is_display_colanim);
        for at in (18..36).step_by(2) {
            assert_eq!(half(d, at), 0);
        }
        assert_eq!(
            attr.damage_coll_size,
            ssb_engine::math::Vec3::new(half(d, 36) as f32, half(d, 38) as f32, half(d, 40) as f32)
        );
        assert_eq!(
            [
                attr.map_coll.top,
                attr.map_coll.center,
                attr.map_coll.bottom,
                attr.map_coll.width
            ],
            [
                half(d, 42) as f32,
                half(d, 44) as f32,
                half(d, 46) as f32,
                half(d, 48) as f32
            ]
        );
        assert_eq!(attr.size, half(d, 50) as f32);
        let w = word(d, 52);
        assert_eq!(
            (attr.angle, attr.kb_scale, attr.damage, attr.element as u8),
            (
                ((w >> 22) & 1023) as i32,
                ((w >> 12) & 1023) as i32,
                ((w >> 4) & 255) as i32,
                (w & 15) as u8
            )
        );
        let w = word(d, 56);
        assert_eq!(
            (
                attr.kb_weight,
                attr.shield_damage,
                attr.attack_count,
                attr.can_setoff
            ),
            (
                (w >> 22) as i32,
                ((w >> 14) & 255) as i32,
                ((w >> 12) & 3) as usize,
                w & (1 << 11) != 0
            )
        );
        let w = word(d, 60);
        assert_eq!(attr.priority, (w >> 29) as i32);
        assert_eq!(
            [
                attr.can_rehit_item,
                attr.can_rehit_fighter,
                attr.can_hop,
                attr.can_reflect,
                attr.can_shield
            ],
            [28, 27, 26, 25, 24].map(|b| w & (1 << b) != 0)
        );
        assert_eq!(attr.kb_base, ((w >> 14) & 1023) as i32);
        assert_eq!(attr.ty as u8, ((w >> 10) & 15) as u8);
        assert_eq!(attr.hitstatus as u8, ((w >> 6) & 15) as u8);
        assert_eq!(attr.vel_scale, (word(d, 68) >> 23) as u16);
        assert_eq!(half(d, 70), spin);
    }
    // The pool's spin speeds are the attributes' percentages.
    for (kind, spin) in [
        (ItemKind::MSBomb, 120),
        (ItemKind::BombHei, 0),
        (ItemKind::Shell(shell::Kind::Green), 0),
        (ItemKind::Shell(shell::Kind::Red), 0),
        (ItemKind::NBumper, 70),
        (ItemKind::MBall, 20),
    ] {
        assert_eq!(kind.spin_speed(), Some(spin as f32 * 0.01));
    }
    for (offset, events) in [
        (0x404, msbomb::ATTACK_EVENTS),
        (0x46C, bombhei::ATTACK_EVENTS),
    ] {
        for (i, e) in events.into_iter().enumerate() {
            let d = &file.data[offset + i * 8..];
            let w = word(d, 0);
            assert_eq!(
                e,
                (
                    (w >> 24) as u16,
                    ((w >> 14) & 1023) as i32,
                    ((w >> 6) & 255) as i32,
                    half(d, 4) as f32
                )
            );
        }
    }
    for (offset, events) in [
        (0x98, CAPSULE_EVENTS),
        (0xB14, EGG_EVENTS),
        (0x614, HEAVY_EVENTS),
        (0x67C, HEAVY_EVENTS),
    ] {
        for (i, e) in events.into_iter().enumerate() {
            let d = &file.data[offset + i * 8..];
            let w = word(d, 0);
            assert_eq!(
                e,
                (
                    (w >> 24) as u16,
                    ((w >> 14) & 1023) as i32,
                    ((w >> 6) & 255) as i32,
                    half(d, 4) as f32
                )
            );
        }
    }
    for (offset, graph, script) in [
        (0x50, 0x670, None),
        (0xACC, 0x104A0, Some(0x10550)),
        (0x5CC, 0x6778, None),
        (0x634, 0x71A8, None),
    ] {
        let data = file.extern_relocs.iter().find(|r| r.at == offset).unwrap();
        assert_eq!((data.target_file, data.target_offset), (86, graph));
        if let Some(script) = script {
            let a = file
                .extern_relocs
                .iter()
                .find(|r| r.at == offset + 8)
                .unwrap();
            assert_eq!((a.target_file, a.target_offset), (86, script));
        }
    }
}

#[test]
fn link_bomb_promotes_its_body_and_animates_the_billboard_fuse_child() {
    let Some(rom) = rom() else {
        return;
    };
    let archive = Archive::open(&rom, ssb_rom::rom::identify(&rom).unwrap().region).unwrap();
    let file = archive.load(353).unwrap();
    // ITAttributes' script table: neither the ejected placeholder nor the
    // promoted body has a script; the fuse child does.
    assert_eq!(word(&file.data, 0x1990), 0);
    assert_eq!(word(&file.data, 0x1994), 0);
    assert_eq!(word(&file.data, 0x1998), 0x199C);
    let bytes = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/generated/ssb64.pak"),
    )
    .unwrap();
    let p = Pack::open(&bytes).unwrap();
    let object = (0..p.object_count())
        .filter_map(|i| p.object(i))
        .find(|o| (o.source_file, o.source_offset) == (353, 0x18D8))
        .unwrap();
    let body = p.node(object.first_node + 1).unwrap();
    assert_eq!(body.rest_translate, [30.0, 90.0, 15.0]);
    assert_eq!(body.parent, object.first_node);
    let fuse = p.node(object.first_node + 2).unwrap();
    assert_eq!(fuse.parent, object.first_node + 1);
    let anim = p.item_anim(AnimDesc::ITEM_ANIM_LINK_BOMB).unwrap();
    let mut packed = StageAnimator::new();
    packed.start_changed(&p, &anim);
    let mut raw = StageJoint::start_changed(0x199C, 0.0);
    let mut pose = JointPose {
        translate: fuse.rest_translate,
        rotate: fuse.rest_rotate,
        scale: fuse.rest_scale,
    };
    for _ in 0..300 {
        packed.tick(p.anim_script(&anim).unwrap()).unwrap();
        raw.tick(&file.data, 1.0, &mut pose).unwrap();
        assert_eq!(packed.node_pose(object.first_node + 2).unwrap(), &pose);
        assert!(packed.node_pose(object.first_node + 1).is_none());
    }
    let mut out = [ssb_rom::scene::Mat4::IDENTITY; 3];
    packed.compose_item(&p, &object, JointPose::default(), &mut out);
    assert_eq!(
        out[1],
        ssb_rom::scene::Mat4::IDENTITY,
        "loose body has no hand offset"
    );
}

#[test]
fn egg_scale_animation_replays_the_rom_for_two_hundred_plays() {
    let Some(rom) = rom() else {
        return;
    };
    let archive = Archive::open(&rom, ssb_rom::rom::identify(&rom).unwrap().region).unwrap();
    let file = archive.load(86).unwrap();
    let bytes = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/generated/ssb64.pak"),
    )
    .unwrap();
    let p = Pack::open(&bytes).unwrap();
    let object = (0..p.object_count())
        .filter_map(|i| p.object(i))
        .find(|o| o.source_file == 86 && o.source_offset == 0x104A0)
        .unwrap();
    assert_eq!(object.node_count, 3);
    let anim = p.item_anim(AnimDesc::ITEM_ANIM_EGG).unwrap();
    let mut packed = StageAnimator::new();
    packed.start_changed(&p, &anim);
    let mut raw = StageJoint::start_changed(0x1055C, 0.0);
    let mut pose = JointPose::default();
    let mut changed = false;
    for _ in 0..200 {
        packed.tick(p.anim_script(&anim).unwrap()).unwrap();
        raw.tick(&file.data, 1.0, &mut pose).unwrap();
        let (_, packed_pose) = packed.joint(0).unwrap();
        assert_eq!(
            packed_pose.scale.map(f32::to_bits),
            pose.scale.map(f32::to_bits)
        );
        assert_eq!(packed_pose.translate, pose.translate);
        changed |= pose.scale != [1.0; 3];
    }
    assert!(changed);
}

#[test]
fn heavy_models_and_smash_piece_are_present_in_the_pack() {
    if rom().is_none() {
        return;
    }
    let bytes = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/generated/ssb64.pak"),
    )
    .unwrap();
    let p = Pack::open(&bytes).unwrap();
    for key in [(86, 0x6778), (86, 0x71A8)] {
        let object = (0..p.object_count())
            .filter_map(|i| p.object(i))
            .find(|o| (o.source_file, o.source_offset) == key)
            .unwrap();
        assert_eq!(object.node_count, 2);
    }
    assert!((0..p.mesh_count())
        .filter_map(|i| p.mesh(i))
        .any(|m| m.source_file == 86 && m.source_offset == 0x68F0));
}

#[test]
fn vs_stages_have_item_points_and_weights() {
    if rom().is_none() {
        return;
    }
    let bytes = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/generated/ssb64.pak"),
    )
    .unwrap();
    let p = Pack::open(&bytes).unwrap();
    for &file in &ssb_rom::stage::VS_GROUND_FILES {
        let i = p.stage_of_file(file).unwrap();
        let stage = p.stage(i).unwrap();
        // `nMPMapObjKindItem`; more than 30 halts `itManagerMakeAppearActor`.
        let points = p.stage_points(&stage).filter(|pt| pt.kind == 4).count();
        assert!((1..=30).contains(&points), "file {file:#x}: {points}");
        assert!(stage.item_weights.is_some(), "file {file:#x}");
    }
}

/// The items' material scripts and root lists (RE-442).
#[test]
fn item_scripts_and_root_lists_are_packed() {
    if rom().is_none() {
        return;
    }
    let bytes = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/generated/ssb64.pak"),
    )
    .unwrap();
    let p = Pack::open(&bytes).unwrap();
    let object = |offset: u32| {
        (0..p.object_count())
            .filter_map(|i| p.object(i))
            .find(|o| (o.source_file, o.source_offset) == (86, offset))
            .unwrap()
    };
    let mesh = |offset: u32| {
        (0..p.mesh_count())
            .filter_map(|i| p.mesh(i))
            .find(|m| (m.source_file, m.source_offset) == (86, offset))
            .unwrap_or_else(|| panic!("mesh 86 + {offset:#X}"))
    };
    let prims = |m: ssb_rom::pack::MeshDesc| {
        (0..m.prim_count)
            .map(|i| p.prim(m.first_prim + i).unwrap())
            .collect::<Vec<_>>()
    };
    let root = |offset: u32| {
        let o = object(offset);
        p.mesh(p.node(o.first_node + 1).unwrap().mesh).unwrap()
    };
    let script = |prim: &ssb_rom::pack::PrimDesc| p.mat_anim(prim.mat_anim).unwrap().script;

    // The Star's two root `MObj`s flicker by `ITAttributes` table 0x15F0.
    let star = prims(root(0x1560));
    assert_eq!(
        star.iter().map(script).collect::<Vec<_>>(),
        [0x15F8, 0x1628]
    );
    // `itManagerMakeItem`'s play then one per process: palettes 0, 0, 1,
    // 1, 0 on the packed player.
    let mut mats = ssb_rom::skeleton::EffectMaterialAnimator::default();
    mats.start(&p, star.iter().map(|prim| prim.mat_anim));
    let palettes: Vec<_> = (0..5)
        .map(|_| {
            mats.tick(&p);
            mats.resolved_palette(&p, star[0].mat_anim)
        })
        .collect();
    assert_eq!(palettes[0], palettes[1]);
    assert_ne!(palettes[1], palettes[2]);
    assert_eq!(palettes[2], palettes[3]);
    assert_eq!(palettes[0], palettes[4]);
    // The Ray Gun's barrel (node 2) by table 0x4760.
    let lgun = object(0x46B0);
    let barrel = p.mesh(p.node(lgun.first_node + 2).unwrap().mesh).unwrap();
    assert_eq!(
        prims(barrel).iter().map(script).collect::<Vec<_>>(),
        [0x476C]
    );

    // Both Bob-omb walk lists run the walk script on the same entry.
    let right = prims(root(0x33F8));
    let left = prims(mesh(0x34C0));
    assert_eq!(script(&right[0]), 0x35B8);
    assert_eq!(right[0].mat_anim, left[0].mat_anim);

    // The Shells' spin frames: red under the tree, green under its own
    // entry and palette.
    let red = prims(root(0x5F88));
    let green = prims(mesh(0x5578));
    assert_eq!((script(&red[0]), script(&green[0])), (0x6048, 0x6048));
    assert_ne!(red[0].mat_anim, green[0].mat_anim);
    let red_frames = p.mat_anim(red[0].mat_anim).unwrap();
    let green_frames = p.mat_anim(green[0].mat_anim).unwrap();
    assert_eq!(red_frames.texture_count, 4);
    assert_eq!(green_frames.texture_count, 4);
    assert_ne!(red_frames.textures[1], green_frames.textures[1]);
    // Two plays per sprite: 0, 0, 1, 1, 2, 2, 3, 3, 2.
    let mut mats = ssb_rom::skeleton::EffectMaterialAnimator::default();
    mats.start(&p, core::iter::once(green[0].mat_anim));
    let frames: Vec<_> = (0..9)
        .map(|_| {
            mats.tick(&p);
            mats.resolved_texture(&p, green[0].mat_anim)
        })
        .collect();
    let t = |i: usize| Some(green_frames.textures[i]);
    assert_eq!(
        frames,
        [t(0), t(0), t(1), t(1), t(2), t(2), t(3), t(3), t(2)]
    );
    // And the root's `rotate.y` spin.
    let spin = p.item_anim(AnimDesc::ITEM_ANIM_SHELL_SPIN).unwrap();
    let joint = p.anim_joint(spin.first_joint).unwrap();
    assert_eq!((spin.joint_count, joint.script), (1, 0x6018));
    assert_eq!(joint.node, object(0x5F88).first_node + 1);

    // The Bumper's lit, attached and attached-lit lists bind their own
    // palettes.
    let unlit = prims(root(0x7648))[0].texture;
    let lit = prims(mesh(0x7238))[0].texture;
    let wait = prims(mesh(0x7A38))[0].texture;
    let wait_lit = prims(mesh(0x76D8))[0].texture;
    let all = [unlit, lit, wait, wait_lit];
    for (i, a) in all.iter().enumerate() {
        assert!(all[i + 1..].iter().all(|b| b != a), "{all:?}");
    }
}
