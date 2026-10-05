//! Race to the Finish's ground data (`grbonus3.c`, `ittarubomb.c`) from
//! the US ROM: the bomb barrel's attributes and explosion, the Bumpers'
//! descriptors and scripts, the barrel point and the finish gate.
use ssb_game::item::{gbumper, tarubomb, ItemType, ItemWeight};
use ssb_game::stage::bonus3;
use ssb_rom::archive::Archive;

fn word(d: &[u8], at: usize) -> u32 {
    u32::from_be_bytes(d[at..at + 4].try_into().unwrap())
}
fn half(d: &[u8], at: usize) -> i16 {
    i16::from_be_bytes(d[at..at + 2].try_into().unwrap())
}
fn float(d: &[u8], at: usize) -> f32 {
    f32::from_bits(word(d, at))
}

/// `llGRBonus3MapFileID`, `llGRBonus3File2FileID` and
/// `llGRBonus3File3FileID`.
const MAP_FILE: u32 = 0x127;
const GEOMETRY_FILE: u32 = 0x95;
const NODES_FILE: u32 = 0xA2;

#[test]
fn packed_race_ground_barrel_and_bumper_roots_match_original_scripts() {
    use ssb_rom::{figatree::JointPose, ground_obj as g, objanim::StageJoint, pack::Pack};
    let Some(path) = std::env::var_os("SSB64_ROM") else {
        return;
    };
    let rom = std::fs::read(path).unwrap();
    let archive = Archive::open(&rom, ssb_rom::rom::identify(&rom).unwrap().region).unwrap();
    let nodes = archive.load(NODES_FILE).unwrap();
    let bytes = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/generated/ssb64.pak"),
    )
    .unwrap();
    let pack = Pack::open(&bytes).unwrap();
    for kind in 14..=25 {
        let entry = ssb_rom::fighter::FIGHTER_FILES[kind as usize];
        let main = archive.load(entry.file).unwrap();
        let attributes = pack.fighter(kind).expect("Polygon attributes");
        assert_eq!(
            (attributes.source_file, attributes.source_offset),
            (entry.file, entry.offset)
        );
        let object = pack
            .object(ssb_rom::scene_deps::fighter_object(&pack, kind).expect("Polygon model"))
            .unwrap();
        let part = ssb_rom::fighter::common_parts(&main, entry)[0].unwrap();
        assert_eq!(
            (object.source_file, object.source_offset),
            (part.model_file, part.graph)
        );
        for slot in [
            ssb_rom::anim::SLOT_WAIT,
            ssb_rom::anim::SLOT_WALK_SLOW,
            ssb_rom::anim::SLOT_RUN,
        ] {
            assert!(
                pack.fighter_anim(kind, slot as u32).is_some(),
                "Polygon {kind} movement {slot}"
            );
        }
    }
    assert!(
        (0..pack.mesh_count())
            .filter_map(|i| pack.mesh(i))
            .any(|m| (m.source_file, m.source_offset) == (NODES_FILE, 0x8A0)),
        "barrel smash pieces"
    );
    let index = pack.stage_of_file(MAP_FILE).expect("Race ground packed");
    let stage = pack.stage(index).unwrap();
    assert_eq!(stage.line_count, 34);
    for layer in [0, 1] {
        assert_ne!(stage.layers[layer], ssb_rom::pack::StageDesc::NO_LAYER);
    }
    let object = |key: (u32, u32)| {
        (0..pack.object_count())
            .filter_map(|i| pack.object(i))
            .find(|o| (o.source_file, o.source_offset) == key)
            .unwrap()
    };
    assert!(object(g::TARUBOMB_SOURCE).node_count >= 2);
    let descriptors = object((NODES_FILE, 0));
    for i in 0..4 {
        let packed = pack.node(descriptors.first_node + i + 1).unwrap();
        let desc = 0x2C * (i + 1) as usize;
        assert_eq!(
            packed.rest_translate,
            [
                float(&nodes.data, desc + 8),
                float(&nodes.data, desc + 12),
                float(&nodes.data, desc + 16)
            ]
        );
    }
    let mut objects = g::GroundObjects::new(&pack, MAP_FILE);
    assert_eq!(objects.iter().count(), 4);
    for i in 0..4 {
        let asset = g::BONUS3_BUMPER_FIRST + i;
        objects.item_make(&pack, asset, 0);
        let script = word(&nodes.data, 0x114 + usize::from(i) * 4);
        let mut joint = StageJoint::start_changed(script, 0.0);
        let mut pose = JointPose::default();
        joint.tick(&nodes.data, 1.0, &mut pose).unwrap();
        for tick in 0..600 {
            assert_eq!(
                objects
                    .get(asset)
                    .unwrap()
                    .pose(g::ITEM_ROOT)
                    .unwrap()
                    .translate,
                pose.translate,
                "Bumper {i} at {tick}"
            );
            objects.item_play(&pack, asset, 0);
            joint.tick(&nodes.data, 1.0, &mut pose).unwrap();
        }
    }
}

#[test]
fn bonus3_ground_data_matches_the_rom() {
    let Some(path) = std::env::var_os("SSB64_ROM") else {
        return;
    };
    let rom = std::fs::read(path).unwrap();
    let archive = Archive::open(&rom, ssb_rom::rom::identify(&rom).unwrap().region).unwrap();
    let map = archive.load(MAP_FILE).unwrap();

    // `MPGroundData::map_nodes` (0x80) and the barrel's `ITAttributes::data`
    // (0xA8): `grBonus3InitHeaders`' two heads.
    let reloc = |at| {
        map.extern_relocs
            .iter()
            .find(|r| r.at == at)
            .map(|r| (u32::from(r.target_file), r.target_offset))
    };
    assert_eq!(reloc(0x80), Some((NODES_FILE, 0)));
    assert_eq!(reloc(0xA8), Some((NODES_FILE, 0x788)));
    assert_eq!(reloc(0x40), Some((GEOMETRY_FILE, 0x65A8)));

    let attr = &tarubomb::ATTRIBUTES;
    let d = &map.data[0xA8..0xA8 + 72];
    assert_eq!(attr.weight, ItemWeight::Heavy);
    assert_eq!((word(d, 16) >> 27) & 3, 2);
    assert_eq!(word(d, 16) >> 31, u32::from(attr.is_give_hitlag));
    assert_eq!((word(d, 16) >> 29) & 1 == 1, attr.is_display_colanim);
    for at in (18..36).step_by(2) {
        assert_eq!(half(d, at), 0);
    }
    assert_eq!(
        attr.damage_coll_size.to_array(),
        [36, 38, 40].map(|at| half(d, at) as f32)
    );
    let m = attr.map_coll;
    assert_eq!(
        [m.top, m.center, m.bottom, m.width],
        [42, 44, 46, 48].map(|at| half(d, at) as f32)
    );
    assert_eq!(attr.size, half(d, 50) as f32);
    let w = word(d, 52);
    assert_eq!(
        (attr.angle, attr.kb_scale, attr.damage, attr.element as u32),
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
    assert_eq!(attr.ty, ItemType::Fighter);
    assert_eq!(attr.ty as u32, (w >> 10) & 15);
    assert_eq!(attr.hitstatus as u32, (w >> 6) & 15);
    assert_eq!(attr.vel_scale, (word(d, 68) >> 23) as u16);
    // `spin_speed`: none.
    assert_eq!(half(d, 70), 0);

    // `llGRBonus3MapTaruBombAttackEvents`.
    for (i, e) in tarubomb::ATTACK_EVENTS.into_iter().enumerate() {
        let d = &map.data[0xF0 + i * 8..];
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

    // `llGRBonus3MapBumpersDObjDesc` past descriptor 0, to the
    // `DOBJ_ARRAY_MAX` terminator, and `llGRBonus3MapBumpersAnimJoint`.
    let nodes = archive.load(NODES_FILE).unwrap();
    let n = &nodes.data;
    let mut descs = Vec::new();
    let mut i = 1;
    while word(n, i * 0x2C) != 18 {
        let at = i * 0x2C;
        let joint = 0x110 + i * 4;
        let animated = nodes.intern_relocs.iter().any(|r| r.at as usize == joint);
        assert_eq!(animated, word(n, joint) != 0);
        descs.push(bonus3::BumperDesc {
            translate: ssb_engine::math::Vec3::new(
                float(n, at + 8),
                float(n, at + 12),
                float(n, at + 16),
            ),
            animated,
        });
        i += 1;
    }
    assert_eq!(descs.len(), bonus3::BUMPERS_MAX);
    assert!(descs.iter().all(|d| d.animated));
    assert_eq!(descs[0].translate.to_array(), [900.0, -2550.0, 0.0]);
    assert_eq!(gbumper::ATTRIBUTES.kb_weight, 200);

    // Exactly one barrel point, and a Detect floor for the finish.
    let geometry = archive.load(GEOMETRY_FILE).unwrap();
    let coll = ssb_rom::collision::read(&geometry, 0x65A8).unwrap();
    let points: Vec<_> = coll
        .map_objects
        .iter()
        .filter(|o| o.kind == bonus3::MAPOBJ_TARUBOMB)
        .collect();
    assert_eq!(points.len(), 1);
    assert!(coll
        .lines_of(ssb_rom::collision::LineKind::Floor)
        .flat_map(|l| &l.points)
        .any(|p| p.flags & 0xFF == bonus3::MATERIAL_DETECT));
}
