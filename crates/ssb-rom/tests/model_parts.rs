//! RE-425's packed assets: every fighter's model parts, the headgear
//! accessories (`FTAttributes::accesspart`) and the entry vehicles'
//! animations.

use ssb_rom::fighter::{self, FighterFile, JOINT_COMMON_START};
use ssb_rom::pack::{accessory_costume, modelpart_costume, NodeDesc, Pack};
use ssb_rom::Archive;

fn rom() -> Option<Vec<u8>> {
    std::fs::read(std::env::var_os("SSB64_ROM")?).ok()
}

fn pack_bytes() -> Option<Vec<u8>> {
    std::env::var_os("SSB64_ROM")?;
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/generated/ssb64.pak");
    std::fs::read(path).ok()
}

/// Every fighter but Master Hand: the twelve, Metal Mario, the Polygons
/// and Giant Donkey Kong (RE-458).
fn playable() -> impl Iterator<Item = (FighterFile, ssb_game::fighter::FighterKind)> {
    (0..27)
        .filter(|&k| k != 12)
        .filter_map(ssb_game::fighter::FighterKind::from_ordinal)
        .map(|kind| {
            let entry = *fighter::FIGHTER_FILES
                .iter()
                .find(|e| e.kind == kind as u8)
                .unwrap();
            (entry, kind)
        })
}

/// `ssb_game::modelpart`'s joint table is each fighter's `setup_parts` and
/// its high-detail descriptors' lists.
#[test]
fn the_joint_masks_are_the_roms() {
    let Some(rom) = rom() else {
        return;
    };
    let info = ssb_rom::rom::identify(&rom).unwrap();
    let archive = Archive::open(&rom, info.region).unwrap();
    for (entry, kind) in playable() {
        let main = archive.load(entry.file).unwrap();
        let common = fighter::common_parts(&main, entry)[0].unwrap();
        let model = archive.load(common.model_file).unwrap();
        let graph = ssb_rom::scene::find_scene_graphs(&model)
            .into_iter()
            .find(|g| g.offset == common.graph)
            .unwrap();
        let mask = fighter::setup_parts(&main, entry).unwrap();
        let dl = graph
            .nodes
            .iter()
            .enumerate()
            .filter(|(_, n)| n.desc.dl.is_some())
            .fold(0u64, |m, (i, _)| m | 1 << i);
        assert_eq!(
            ssb_game::modelpart::joint_masks(kind),
            Some((mask, dl & mask)),
            "{}",
            entry.name
        );
    }
}

/// Pikachu wears his hat on joint 11 and Jigglypuff her bow on joint 6,
/// each from its model file; nobody else has an accessory.
#[test]
fn only_pikachu_and_jigglypuff_have_an_accessory() {
    let Some(rom) = rom() else {
        return;
    };
    let info = ssb_rom::rom::identify(&rom).unwrap();
    let archive = Archive::open(&rom, info.region).unwrap();
    for (entry, kind) in playable() {
        let main = archive.load(entry.file).unwrap();
        let access = fighter::access_part(&main, entry);
        let model = fighter::common_parts(&main, entry)[0].unwrap().model_file;
        match kind {
            ssb_game::fighter::FighterKind::Pikachu => {
                let a = access.unwrap();
                assert_eq!((a.joint, a.dl), (11, (model, 0x63F0)));
                assert_eq!(a.mobjsubs, Some((model, 0x6350)));
                assert_eq!(a.costume_matanim_joints, Some((model, 0x654C)));
            }
            ssb_game::fighter::FighterKind::Purin => {
                let a = access.unwrap();
                assert_eq!((a.joint, a.dl), (6, (model, 0x4A60)));
                assert_eq!(a.mobjsubs, Some((model, 0x49D0)));
                assert_eq!(a.costume_matanim_joints, Some((model, 0x4BE4)));
            }
            _ => assert_eq!(access, None, "{}", entry.name),
        }
    }
}

/// Every costume but 0 of Pikachu and Jigglypuff has an accessory mesh on
/// its joint's node, from the accessory's list, and no other node has one.
#[test]
fn every_costume_but_the_first_packs_its_accessory() {
    let (Some(rom), Some(bytes)) = (rom(), pack_bytes()) else {
        return;
    };
    let pack = Pack::open(&bytes).unwrap();
    let info = ssb_rom::rom::identify(&rom).unwrap();
    let archive = Archive::open(&rom, info.region).unwrap();
    for (entry, kind) in playable() {
        let object = pack
            .object(ssb_rom::scene_deps::fighter_object(&pack, kind as u32).unwrap())
            .unwrap();
        let main = archive.load(entry.file).unwrap();
        let access = fighter::access_part(&main, entry);
        let nodes = object.first_node..object.first_node + object.node_count;
        for costume in 0..8 {
            for node in nodes.clone() {
                let mesh = pack.costume_mesh(node, accessory_costume(costume));
                let joint_node = access.map(|a| object.first_node + a.joint - JOINT_COMMON_START);
                if costume == 0 || Some(node) != joint_node {
                    assert!(
                        mesh.is_none(),
                        "{} costume {costume} node {node}",
                        entry.name
                    );
                    continue;
                }
                if costume >= 4 {
                    continue;
                }
                let mesh = pack.mesh(mesh.unwrap()).unwrap();
                assert_eq!(mesh.source_offset, access.unwrap().dl.1);
                assert!(mesh.prim_count > 0);
            }
        }
    }
}

/// Every part a playable fighter's scripts or code can set on a joint it
/// makes draws: its costume-0 mesh is packed on the joint's node from the
/// part's list, unless it is exactly the node's own mesh.
#[test]
fn every_reachable_model_part_is_packed() {
    let (Some(rom), Some(bytes)) = (rom(), pack_bytes()) else {
        return;
    };
    let pack = Pack::open(&bytes).unwrap();
    let info = ssb_rom::rom::identify(&rom).unwrap();
    let archive = Archive::open(&rom, info.region).unwrap();
    let mut packed = 0;
    for (entry, kind) in playable() {
        let object = pack
            .object(ssb_rom::scene_deps::fighter_object(&pack, kind as u32).unwrap())
            .unwrap();
        let main = archive.load(entry.file).unwrap();
        let (present, _) = ssb_game::modelpart::joint_masks(kind).unwrap();
        let mut wanted = ssb_game::motion::model_part_events(kind);
        match kind {
            ssb_game::fighter::FighterKind::Link | ssb_game::fighter::FighterKind::PolyLink => {
                wanted.extend([(19, 0), (21, 0)])
            }
            ssb_game::fighter::FighterKind::Yoshi => {
                wanted.insert((7, 1));
            }
            ssb_game::fighter::FighterKind::Kirby => wanted.extend(
                ssb_game::kirby_copy::COPY_MODELPART_IDS
                    .iter()
                    .filter(|&&p| p != 0)
                    .map(|&p| (6, i32::from(p))),
            ),
            _ => {}
        }
        for (joint, part) in wanted {
            let (Ok(joint), Ok(part)) = (u32::try_from(joint), u32::try_from(part)) else {
                continue;
            };
            let n = joint - JOINT_COMMON_START;
            if n >= object.node_count || present >> n & 1 == 0 {
                continue;
            }
            let node = object.first_node + n;
            // A joint without a `modelparts_desc` keeps its own list for any
            // part (`ftParamSetModelPartID`): the Polygons' simpler models.
            let Some(mp) = fighter::model_part(&main, entry, joint, part, 0) else {
                assert_eq!(
                    pack.costume_mesh(node, modelpart_costume(part, 0)),
                    None,
                    "{} joint {joint} part {part}",
                    entry.name
                );
                continue;
            };
            match pack.costume_mesh(node, modelpart_costume(part, 0)) {
                Some(mesh) => {
                    let mesh = pack.mesh(mesh).unwrap();
                    assert_eq!(
                        (mesh.source_file, mesh.source_offset),
                        mp.dl,
                        "{} joint {joint} part {part}",
                        entry.name
                    );
                    packed += 1;
                }
                None => {
                    let own = pack.node(node).unwrap().mesh;
                    assert_ne!(
                        own,
                        NodeDesc::NO_MESH,
                        "{} joint {joint} part {part}",
                        entry.name
                    );
                    let own = pack.mesh(own).unwrap();
                    assert_eq!(
                        (own.source_file, own.source_offset),
                        mp.dl,
                        "{} joint {joint} part {part}",
                        entry.name
                    );
                }
            }
        }
    }
    // Fox's Blaster, Link's shield in hand, the hands and faces.
    assert!(packed >= 30, "{packed}");
}

/// The entry makers' tables are packed on their trees' nodes, and the
/// car's exhaust sprites are billboards.
#[test]
fn the_entry_vehicles_animations_are_packed() {
    let Some(bytes) = pack_bytes() else {
        return;
    };
    let pack = Pack::open(&bytes).unwrap();
    let object = |key: (u32, u32)| {
        (0..pack.object_count())
            .filter_map(|i| pack.object(i))
            .find(|o| (o.source_file, o.source_offset) == key)
            .unwrap()
    };
    for &(slot, key, file, _, singles) in &ssb_rom::effect::ENTRY_ANIMS {
        let o = object(key);
        let anim = pack
            .effect_anim(slot)
            .unwrap_or_else(|| panic!("slot {slot:#X}"));
        assert_eq!(anim.source_file, file);
        assert!(anim.joint_count as usize > singles.len());
        for j in 0..anim.joint_count {
            let joint = pack.anim_joint(anim.first_joint + j).unwrap();
            assert!((o.first_node..o.first_node + o.node_count).contains(&joint.node));
        }
    }
    let car = object(ssb_rom::effect::ENTRY_CAR_KEY);
    for n in 0..car.node_count {
        let billboard =
            pack.node(car.first_node + n).unwrap().flags & NodeDesc::FLAG_BILLBOARD != 0;
        assert_eq!(
            billboard,
            ssb_rom::effect::ENTRY_CAR_BILLBOARD_NODES.contains(&n),
            "car node {n}"
        );
    }
}
