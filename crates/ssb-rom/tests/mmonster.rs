//! The Poké Ball Pokémon's and the item weapons' constants, from the US
//! ROM (RE-435): their `ITAttributes` and `WPAttributes` (file 251, and
//! file 264 for Saffron's two), and the animation values gameplay reads.
use ssb_game::item::mmonster::{Kind, ATTRIBUTES, SPEAR_SWARM_CALL_WAIT};
use ssb_game::monster_weapon::{hydro_offset_x, ShotKind, ATTRIBUTES as SHOTS, SMOG_SCALE};
use ssb_rom::{archive::Archive, figatree::JointPose, objanim::StageJoint};

fn word(d: &[u8], at: usize) -> u32 {
    u32::from_be_bytes(d[at..at + 4].try_into().unwrap())
}

#[test]
fn pokemon_status_materials_and_conditional_meshes_are_bound_in_the_pack() {
    use ssb_rom::{
        mmonster as asset,
        pack::{flags, Pack},
    };
    if std::env::var_os("SSB64_ROM").is_none() {
        return;
    }
    let bytes = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/generated/ssb64.pak"),
    )
    .unwrap();
    let p = Pack::open(&bytes).unwrap();
    let object = |graph| {
        (0..p.object_count())
            .filter_map(|i| p.object(i))
            .find(|o| (o.source_file, o.source_offset) == (86, graph))
            .unwrap()
    };
    let mesh = |key| {
        (0..p.mesh_count())
            .filter_map(|i| p.mesh(i))
            .find(|m| (m.source_file, m.source_offset) == (86, key))
            .unwrap()
    };
    // Offsets and target nodes independently read from the named status
    // callbacks and reloc_data.us.h, rather than the Visual table itself.
    for (kind, node, script) in [
        (Kind::Tosakinto, 2, 0xB90C),
        (Kind::Lizardon, 1, 0xD688),
        (Kind::Spear, 2, 0xE12C),
        (Kind::Starmie, 1, 0x11338),
    ] {
        let o = object(asset::VISUALS[kind as usize].graph);
        let m = p.mesh(p.node(o.first_node + node).unwrap().mesh).unwrap();
        let bound: Vec<_> = (0..m.prim_count)
            .filter_map(|i| p.prim(m.first_prim + i))
            .filter_map(|pr| p.mat_anim(pr.mat_anim))
            .collect();
        assert!(!bound.is_empty(), "{kind:?}");
        assert!(
            bound
                .iter()
                .all(|a| (a.source_file, a.script) == (86, script)),
            "{kind:?}"
        );
    }
    let ball = mesh(0x9520);
    let a = p
        .mat_anim(p.prim(ball.first_prim).unwrap().mat_anim)
        .unwrap();
    assert_eq!((a.source_file, a.script, a.texture_count), (86, 0x9520, 8));
    for key in [asset::KABIGON_FALL_MESH, asset::PIPPI_XLU_MESH] {
        let m = mesh(key);
        for pr in (0..m.prim_count).map(|i| p.prim(m.first_prim + i).unwrap()) {
            assert_ne!(pr.flags & flags::ALPHA_BLEND, 0);
            assert_eq!(
                pr.flags & (flags::DEPTH_TEST | flags::DEPTH_WRITE | flags::HEAD1),
                0
            );
        }
    }
    let clefairy = object(asset::PIPPI_SWARM_GRAPH);
    let normal = p
        .mesh(p.node(clefairy.first_node + 1).unwrap().mesh)
        .unwrap();
    let pr = p.prim(normal.first_prim).unwrap();
    assert_eq!(
        pr.flags & (flags::DEPTH_TEST | flags::DEPTH_WRITE | flags::ALPHA_TEST),
        flags::DEPTH_TEST | flags::DEPTH_WRITE | flags::ALPHA_TEST
    );
    let textures: Vec<_> = asset::ROCK_TEXTURE_KEYS
        .into_iter()
        .map(|key| p.prim(mesh(key).first_prim).unwrap().texture)
        .collect();
    assert!(textures.iter().all(|&i| p.texture(i).is_some()));
    assert_ne!(textures[0], textures[1]);
    assert_ne!(textures[1], textures[2]);
    assert_ne!(textures[0], textures[2]);
}
fn half(d: &[u8], at: usize) -> i16 {
    i16::from_be_bytes(d[at..at + 2].try_into().unwrap())
}
fn archive_files(ids: &[u32]) -> Option<Vec<ssb_rom::archive::File>> {
    let rom = std::fs::read(std::env::var_os("SSB64_ROM")?).unwrap();
    let archive = Archive::open(&rom, ssb_rom::rom::identify(&rom).unwrap().region).unwrap();
    Some(ids.iter().map(|&id| archive.load(id).unwrap()).collect())
}

#[test]
fn pokemon_item_attributes_match_the_rom() {
    let Some(files) = archive_files(&[251]) else {
        return;
    };
    let f = &files[0].data;
    let offsets = [
        0x72C, 0x7A8, 0x7F0, 0x880, 0x8FC, 0x98C, 0xA08, 0xA84, 0xB34, 0xBB0, 0xBF8, 0xC74, 0x838,
    ];
    for (kind, off) in Kind::ALL.into_iter().zip(offsets) {
        let attr = &ATTRIBUTES[kind as usize];
        let d = &f[off..off + 72];
        assert_eq!((word(d, 16) >> 27) & 3, 3, "{kind:?}: light, hitlag");
        for at in (18..36).step_by(2) {
            assert_eq!(half(d, at), 0);
        }
        let size = |at| half(d, at) as f32;
        assert_eq!(
            [
                attr.damage_coll_size.x,
                attr.damage_coll_size.y,
                attr.damage_coll_size.z
            ],
            [size(36), size(38), size(40)],
            "{kind:?}"
        );
        assert_eq!(
            [
                attr.map_coll.top,
                attr.map_coll.center,
                attr.map_coll.bottom,
                attr.map_coll.width
            ],
            [size(42), size(44), size(46), size(48)],
            "{kind:?}"
        );
        assert_eq!(attr.size, half(d, 50) as u16 as f32, "{kind:?}");
        let w = word(d, 52);
        assert_eq!(
            (attr.angle, attr.kb_scale, attr.damage, attr.element as u32),
            (
                (w as i32) >> 22,
                ((w >> 12) & 1023) as i32,
                ((w >> 4) & 255) as i32,
                w & 15
            ),
            "{kind:?}"
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
                ((w << 10) as i32) >> 24,
                ((w >> 12) & 3) as usize,
                w & (1 << 11) != 0
            ),
            "{kind:?}"
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
            [28, 27, 26, 25, 24].map(|b| w & (1 << b) != 0),
            "{kind:?}"
        );
        assert_eq!(attr.kb_base, ((w >> 14) & 1023) as i32, "{kind:?}");
        assert_eq!(attr.ty as u32, (w >> 10) & 15);
        assert_eq!(attr.hitstatus as u32, (w >> 6) & 15);
        assert_eq!(u32::from(attr.vel_scale), word(d, 68) >> 23);
        assert_eq!(half(d, 70), 0, "{kind:?}: no spin");
    }
}

#[test]
fn item_weapon_attributes_match_the_rom() {
    let Some(files) = archive_files(&[251, 264]) else {
        return;
    };
    for (kind, file, off) in [
        (ShotKind::HitokageFlame, 1, 0x244),
        (ShotKind::FushigibanaRazor, 1, 0x308),
        (ShotKind::IwarkRock, 0, 0x774),
        (ShotKind::NyarsCoin, 0, 0x8C8),
        (ShotKind::LizardonFlame, 0, 0x944),
        (ShotKind::SpearSwarm, 0, 0x9D4),
        (ShotKind::PippiSwarm, 0, 0xCBC),
        (ShotKind::KamexHydro, 0, 0xA50),
        (ShotKind::StarmieSwift, 0, 0xB7C),
        (ShotKind::DogasSmog, 0, 0xC40),
        (ShotKind::RayGun, 0, 0x2B0),
        (ShotKind::StarRod, 0, 0x4D4),
        (ShotKind::FireFlower, 0, 0x32C),
    ] {
        let a = &SHOTS[kind as usize];
        let d = &files[file].data[off..off + 0x34];
        for at in (16..28).step_by(2) {
            assert_eq!(
                half(d, at),
                if kind == ShotKind::RayGun && at == 22 {
                    -5
                } else {
                    0
                },
                "{kind:?}: attack offsets"
            );
        }
        let h = |at| half(d, at) as f32;
        assert_eq!(
            [
                a.map_coll.top,
                a.map_coll.center,
                a.map_coll.bottom,
                a.map_coll.width
            ],
            [h(28), h(30), h(32), h(34)],
            "{kind:?}"
        );
        let (w0, w1, w2, w3) = (word(d, 36), word(d, 40), word(d, 44), word(d, 48));
        assert_eq!(a.size, (w0 >> 16) as f32, "{kind:?}");
        assert_eq!(a.angle, ((w0 << 16) as i32) >> 22, "{kind:?}");
        assert_eq!(
            (a.kb_scale, a.damage, a.element as u32, a.kb_weight),
            (
                (w1 >> 22) as i32,
                ((w1 >> 14) & 255) as i32,
                (w1 >> 10) & 15,
                (w1 & 1023) as i32
            ),
            "{kind:?}"
        );
        assert_eq!(a.shield_damage, (w2 as i32) >> 24, "{kind:?}");
        assert_eq!((w2 >> 22) & 3, if kind == ShotKind::RayGun { 2 } else { 1 });
        assert_eq!(a.can_setoff, (w2 >> 21) & 1 != 0, "{kind:?}");
        assert_eq!((w2 >> 8) & 7, 1, "{kind:?}: priority");
        assert_eq!(
            [
                a.can_rehit_fighter,
                a.can_hop,
                a.can_reflect,
                a.can_absorb,
                a.can_shield
            ],
            [6, 5, 4, 3, 2].map(|b| w2 & (1 << b) != 0),
            "{kind:?}"
        );
        if matches!(
            kind,
            ShotKind::RayGun | ShotKind::StarRod | ShotKind::FireFlower
        ) {
            assert_eq!(a.can_rehit_item, w2 & (1 << 7) != 0, "{kind:?}");
        }
        assert_eq!(a.kb_base, (w3 >> 22) as i32, "{kind:?}");
    }
}

#[test]
fn star_rod_smash_record_and_flower_angles_match_the_rom() {
    let Some(files) = archive_files(&[251]) else {
        return;
    };
    let d = &files[0].data;
    // Smash changes damage and the hit sound in the packed attack words.
    for at in (0..0x34).step_by(4) {
        let tilt = word(d, 0x4D4 + at);
        let smash = word(d, 0x508 + at);
        if at == 40 {
            assert_eq!((smash >> 14) & 255, 12);
            assert_eq!(tilt & !(255 << 14), smash & !(255 << 14));
        } else if at == 44 {
            assert_eq!((tilt >> 11) & 1023, 38);
            assert_eq!((smash >> 11) & 1023, 37);
            assert_eq!(tilt & !(1023 << 11), smash & !(1023 << 11));
        } else {
            assert_eq!(tilt, smash, "word {at}");
        }
    }
    for i in 0..5 {
        let radians = f32::from_bits(word(d, 0x360 + i * 4));
        let expected = (i as f32 * 7.5 - 15.0) * core::f32::consts::PI / 180.0;
        assert!(
            (radians - expected).abs() < 0.000_001,
            "angle {i}: {radians}"
        );
    }
}

/// Plays `script` in file 86 from a `gcAddDObjAnimJoint` at frame 0,
/// `plays` times, from `rest`.
fn replay(data: &[u8], script: u32, rest: JointPose, plays: usize) -> Vec<(JointPose, StageJoint)> {
    let mut joint = StageJoint::start_changed(script, 0.0);
    let mut pose = rest;
    (0..plays)
        .map(|_| {
            joint.tick(data, 1.0, &mut pose).unwrap();
            (pose, joint)
        })
        .collect()
}

#[test]
fn the_animation_values_gameplay_reads_replay_the_rom() {
    let Some(files) = archive_files(&[86]) else {
        return;
    };
    let d = &files[0].data;
    // The Hydro Pump's child (file 86 + 0xF9D8, second descriptor, at
    // X 4391.25 at rest) under `llITCommonDataKamexHydro` + 4 (0xFA9C).
    let rest = JointPose {
        translate: [4391.25, 0.0, 0.0],
        scale: [1.0, 0.000_01, 1.0],
        ..JointPose::default()
    };
    for (play, (pose, _)) in replay(d, 0xFA9C, rest, 20).into_iter().enumerate() {
        assert_eq!(
            pose.translate[0],
            hydro_offset_x(play as u16),
            "play {play}"
        );
    }
    // The Smog's child (0x13100's second descriptor, 5.5 at rest), 0x13198.
    let rest = JointPose {
        scale: [5.5, 5.5, 1.0],
        ..JointPose::default()
    };
    for (play, (pose, _)) in replay(d, 0x13198, rest, 30).into_iter().enumerate() {
        assert_eq!(pose.scale[0], SMOG_SCALE[play], "play {play}");
    }
    // Beedrill's appear animation (`llITCommonDataSpearAnimJoint`, 0xDFFC)
    // writes the GObj frame 51 on its 51st play after the add's own, then
    // ends.
    let plays = replay(d, 0xDFFC, JointPose::default(), 53);
    let frame = |n: usize| plays[n].1.gobj_frame();
    assert_eq!(frame(usize::from(SPEAR_SWARM_CALL_WAIT)), Some(51.0));
    assert!((1..51).all(|n| frame(n) == Some(n as f32)));
    assert!(plays[52].1.ended());
    // The rise's scale script (`llITCommonDataMonsterAnimBankStart`,
    // 0x13624) on the Pokémon's root never writes its translation: the
    // item's position is its own.
    for (pose, _) in replay(d, 0x13624, JointPose::default(), 40) {
        assert_eq!(pose.translate, [0.0; 3]);
        assert_eq!(pose.rotate, [0.0; 3]);
    }
    // Charizard's attack script on its root turns it, but never moves it.
    for (pose, _) in replay(d, 0xD658, JointPose::default(), 120) {
        assert_eq!(pose.translate, [0.0; 3]);
    }
}
