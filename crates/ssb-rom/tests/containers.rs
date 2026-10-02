//! Normal-container constants and the Egg's packed animation, from the US ROM.
use ssb_game::item::container::{
    BARREL_ATTRIBUTES, CAPSULE_ATTRIBUTES, CAPSULE_EVENTS, CRATE_ATTRIBUTES, EGG_ATTRIBUTES,
    EGG_EVENTS, HEAVY_EVENTS,
};
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
    ] {
        let d = &file.data[offset..offset + 72];
        let light = attr.weight == ssb_game::item::ItemWeight::Light;
        assert_eq!(word(d, 16) >> 27, if light { 3 } else { 2 });
        assert!(attr.is_give_hitlag);
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
