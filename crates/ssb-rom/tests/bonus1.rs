//! All US target courses: placement, model, hitbox and independent root
//! script replay against the user's ROM. No derived assets are embedded.
use ssb_rom::{
    archive::Archive, bonus1 as b, figatree::JointPose, objanim::StageJoint, pack::Pack,
};

fn word(d: &[u8], at: usize) -> u32 {
    u32::from_be_bytes(d[at..at + 4].try_into().unwrap())
}
fn half(d: &[u8], at: usize) -> i16 {
    i16::from_be_bytes(d[at..at + 2].try_into().unwrap())
}
fn float(d: &[u8], at: usize) -> f32 {
    f32::from_bits(word(d, at))
}

#[test]
fn every_target_course_placement_and_animation_matches_rom() {
    let Some(path) = std::env::var_os("SSB64_ROM") else {
        return;
    };
    let rom = std::fs::read(path).unwrap();
    let a = Archive::open(&rom, ssb_rom::rom::identify(&rom).unwrap().region).unwrap();
    let bytes = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/generated/ssb64.pak"),
    )
    .unwrap();
    let p = Pack::open(&bytes).unwrap();
    let object = |key| {
        (0..p.object_count())
            .filter_map(|i| p.object(i))
            .find(|o| (o.source_file, o.source_offset) == key)
            .unwrap()
    };
    assert_eq!(object(b::MODEL).node_count, 2);
    let mut animated = 0;
    for (kind, c) in b::COURSES.iter().enumerate() {
        let header = a.load(271 + kind as u32).unwrap();
        assert!(header.extern_relocs.iter().any(|r| r.at == 16
            && (u32::from(r.target_file), r.target_offset) == (124 + kind as u32, c.start)));
        let raw = a.load(124 + kind as u32).unwrap();
        let stage = p.stage(p.stage_of_file(header.id).unwrap()).unwrap();
        assert!(p
            .spawn(&stage, ssb_game::spgame::setup::mapobj::PLAYER)
            .is_some());
        let placements = object((raw.id, c.placements));
        assert_eq!(placements.node_count, 11);
        assert_eq!(word(&raw.data, c.placements as usize + 11 * 44), 18);
        let mut objs = ssb_rom::ground_obj::GroundObjects::new(&p, header.id);
        assert_eq!(objs.iter().count(), 10);
        for i in 0..10 {
            let node = p.node(placements.first_node + 1 + i).unwrap();
            let at = c.placements as usize + (1 + i as usize) * 44;
            assert_eq!(
                node.rest_translate,
                [8, 12, 16].map(|off| float(&raw.data, at + off))
            );
            let asset = b::FIRST_ASSET + kind as u8;
            objs.item_make(&p, asset, i as u8);
            let script = word(&raw.data, c.scripts as usize + 4 * (1 + i as usize));
            assert_eq!(
                p.item_anim(b::anim(kind as u8, i as u8)).is_some(),
                script != 0
            );
            if script == 0 {
                continue;
            }
            animated += 1;
            let mut joint = StageJoint::start_changed(script, 0.0);
            let mut pose = JointPose::default();
            joint.tick(&raw.data, 1.0, &mut pose).unwrap();
            for tick in 0..600 {
                assert_eq!(
                    *objs
                        .instance(asset, i as u8)
                        .unwrap()
                        .pose(ssb_rom::ground_obj::ITEM_ROOT)
                        .unwrap(),
                    pose,
                    "fighter {kind} target {i} tick {tick}"
                );
                objs.item_play(&p, asset, i as u8);
                joint.tick(&raw.data, 1.0, &mut pose).unwrap();
            }
        }
    }
    assert_eq!(animated, 15);
}

#[test]
fn target_attributes_match_the_rom() {
    use ssb_game::item::{target, ItemType, ItemWeight};
    let Some(path) = std::env::var_os("SSB64_ROM") else {
        return;
    };
    let rom = std::fs::read(path).unwrap();
    let a = Archive::open(&rom, ssb_rom::rom::identify(&rom).unwrap().region).unwrap();
    let h = a.load(253).unwrap();
    assert!(h
        .extern_relocs
        .iter()
        .any(|r| r.at == 0 && (u32::from(r.target_file), r.target_offset) == b::MODEL));
    let d = &h.data;
    let at = &target::ATTRIBUTES;
    assert_eq!((word(d, 16) >> 30) & 1, 1);
    assert_eq!(at.weight, ItemWeight::Light);
    assert_eq!(word(d, 16) >> 31, u32::from(at.is_give_hitlag));
    assert_eq!(word(d, 16) & (1 << 29) != 0, at.is_display_colanim);
    assert!((18..36).step_by(2).all(|i| half(d, i) == 0));
    assert_eq!(
        at.damage_coll_size.to_array(),
        [36, 38, 40].map(|i| half(d, i) as f32)
    );
    let m = at.map_coll;
    assert_eq!(
        [m.top, m.center, m.bottom, m.width, at.size],
        [42, 44, 46, 48, 50].map(|i| half(d, i) as f32)
    );
    let w = word(d, 52);
    assert_eq!(
        (at.angle, at.kb_scale, at.damage, at.element as u32),
        (
            ((w >> 22) & 1023) as i32,
            ((w >> 12) & 1023) as i32,
            ((w >> 4) & 255) as i32,
            w & 15
        )
    );
    let w = word(d, 56);
    assert_eq!(
        (
            at.kb_weight,
            at.shield_damage,
            at.attack_count,
            at.can_setoff
        ),
        (
            (w >> 22) as i32,
            ((w >> 14) & 255) as i32,
            ((w >> 12) & 3) as usize,
            w & (1 << 11) != 0
        )
    );
    let w = word(d, 60);
    assert_eq!(at.priority, (w >> 29) as i32);
    assert_eq!(
        [
            at.can_rehit_item,
            at.can_rehit_fighter,
            at.can_hop,
            at.can_reflect,
            at.can_shield
        ],
        [28, 27, 26, 25, 24].map(|i| w & (1 << i) != 0)
    );
    assert_eq!(at.kb_base, ((w >> 14) & 1023) as i32);
    assert_eq!(at.ty, ItemType::Fighter);
    assert_eq!(at.ty as u32, (w >> 10) & 15);
    assert_eq!(at.hitstatus as u32, (w >> 6) & 15);
    assert_eq!(at.vel_scale, (word(d, 68) >> 23) as u16);
}
