//! RE-417's packed assets: Kirby's copy hats (`FTModelPart`s of joint 6)
//! and Pikachu's four Thunder frames.

use ssb_rom::pack::{modelpart_costume, Pack};
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

fn kirby() -> ssb_rom::fighter::FighterFile {
    *ssb_rom::fighter::FIGHTER_FILES
        .iter()
        .find(|e| e.name == "Kirby")
        .unwrap()
}

/// `COPY_MODELPART_IDS` is `FTKirbyCopy[kind].copy_modelpart_id` from file
/// 228, and part 0's high-detail list is joint 6's own, graph node 2's
/// (`nFTPartsJointCommonStart` is 4).
#[test]
fn the_copy_table_and_joint_six_match_the_rom() {
    let Some(rom) = rom() else {
        return;
    };
    let info = ssb_rom::rom::identify(&rom).unwrap();
    let archive = Archive::open(&rom, info.region).unwrap();
    let motion = archive.load(228).unwrap();
    for (kind, &want) in ssb_game::kirby_copy::COPY_MODELPART_IDS.iter().enumerate() {
        let at = kind * 12;
        let copy_id = u16::from_be_bytes([motion.data[at], motion.data[at + 1]]);
        let part = i16::from_be_bytes([motion.data[at + 2], motion.data[at + 3]]);
        assert_eq!(copy_id as usize, kind);
        assert_eq!(part, i16::from(want), "kind {kind}");
    }
    let main = archive.load(kirby().file).unwrap();
    let common = ssb_rom::fighter::common_parts(&main, kirby())[0].unwrap();
    let model = archive.load(common.model_file).unwrap();
    let graph = ssb_rom::scene::find_scene_graphs(&model)
        .into_iter()
        .find(|g| g.offset == common.graph)
        .unwrap();
    let joint = u32::from(ssb_game::kirby_copy::COPY_MODELPARTS_JOINT);
    let node = (joint - ssb_rom::fighter::JOINT_COMMON_START) as usize;
    let base = ssb_rom::fighter::model_part(&main, kirby(), joint, 0, 0).unwrap();
    assert_eq!(Some(base.dl.1), graph.nodes[node].desc.dl);
    assert_eq!(base.dl.0, common.model_file);
    for &part in &ssb_game::kirby_copy::COPY_MODELPART_IDS {
        let mp = ssb_rom::fighter::model_part(&main, kirby(), joint, u32::from(part), 0).unwrap();
        assert_eq!(mp.dl.0, common.model_file, "part {part}");
        assert!(mp.mobjsubs.is_some(), "part {part}");
    }
}

/// Every copy hat has a costume-0 mesh on joint 6's node, built from its
/// own list, and no other node has one.
#[test]
fn every_copy_hat_is_packed_on_joint_six() {
    let (Some(rom), Some(bytes)) = (rom(), pack_bytes()) else {
        return;
    };
    let pack = Pack::open(&bytes).unwrap();
    let info = ssb_rom::rom::identify(&rom).unwrap();
    let archive = Archive::open(&rom, info.region).unwrap();
    let main = archive.load(kirby().file).unwrap();
    let object = pack
        .object(ssb_rom::scene_deps::fighter_object(&pack, 8).unwrap())
        .unwrap();
    let joint = u32::from(ssb_game::kirby_copy::COPY_MODELPARTS_JOINT);
    let node = object.first_node + joint - ssb_rom::fighter::JOINT_COMMON_START;
    for &part in ssb_game::kirby_copy::COPY_MODELPART_IDS
        .iter()
        .filter(|&&p| p != 0)
    {
        let mp = ssb_rom::fighter::model_part(&main, kirby(), joint, u32::from(part), 0).unwrap();
        let mesh = pack
            .costume_mesh(node, modelpart_costume(u32::from(part), 0))
            .unwrap_or_else(|| panic!("part {part} not packed"));
        let mesh = pack.mesh(mesh).unwrap();
        assert_eq!((mesh.source_file, mesh.source_offset), mp.dl, "part {part}");
        assert!(mesh.prim_count > 0);
        for other in object.first_node..object.first_node + object.node_count {
            if other != node {
                assert!(pack
                    .costume_mesh(other, modelpart_costume(u32::from(part), 0))
                    .is_none());
            }
        }
    }
    // Part 0 is the node's base mesh.
    assert!(pack.costume_mesh(node, modelpart_costume(0, 0)).is_none());
}

/// Each Thunder frame is its own mesh, keyed by its sprite, binding a
/// different texture.
#[test]
fn the_four_thunder_frames_are_packed() {
    let Some(bytes) = pack_bytes() else {
        return;
    };
    let pack = Pack::open(&bytes).unwrap();
    let textures: Vec<u32> = ssb_rom::effect::PIKACHU_THUNDER_SPRITES
        .iter()
        .map(|&sprite| {
            let mesh = (0..pack.mesh_count())
                .filter_map(|i| pack.mesh(i))
                .find(|m| {
                    (m.source_file, m.source_offset)
                        == (ssb_rom::effect::PIKACHU_THUNDER_FILE, sprite)
                })
                .unwrap_or_else(|| panic!("frame 0x{sprite:X} not packed"));
            assert_eq!(mesh.prim_count, 1);
            pack.prim(mesh.first_prim).unwrap().texture
        })
        .collect();
    for (i, a) in textures.iter().enumerate() {
        for b in &textures[i + 1..] {
            assert_ne!(a, b);
        }
    }
}
