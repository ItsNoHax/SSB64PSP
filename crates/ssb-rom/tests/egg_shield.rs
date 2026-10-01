//! RE-418: Yoshi's egg shield list (file 338 + 0xA860) combines
//! `(SHADE - ENV) * TEXEL0`, and the pack marks it so a draw can subtract
//! `efManagerYoshiShieldProcDisplay`'s ENV.

use ssb_rom::pack::{flags, Pack};
use ssb_rom::Archive;

const EGG: (u32, u32) = (338, 0xA860);

fn rom() -> Option<Vec<u8>> {
    std::fs::read(std::env::var_os("SSB64_ROM")?).ok()
}

fn pack_bytes() -> Option<Vec<u8>> {
    std::env::var_os("SSB64_ROM")?;
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/generated/ssb64.pak");
    std::fs::read(path).ok()
}

/// The list's one `G_SETCOMBINE` is `(SHADE - ENV) * TEXEL0 + 0` in both
/// cycles, with alpha `TEXEL0`.
#[test]
fn the_egg_list_subtracts_env_from_the_shade() {
    let Some(rom) = rom() else {
        return;
    };
    let info = ssb_rom::rom::identify(&rom).unwrap();
    let archive = Archive::open(&rom, info.region).unwrap();
    let file = archive.load(EGG.0).unwrap();
    let cmds = ssb_rom::dl::decode_list_at(&file.data[EGG.1 as usize..], EGG.1).unwrap();
    let combines: Vec<(u32, u32)> = cmds
        .iter()
        .filter_map(|c| match c {
            ssb_rom::dl::Cmd::SetCombine { hi, lo } => Some((*hi, *lo)),
            _ => None,
        })
        .collect();
    assert_eq!(combines, [(0x0040_FE81, 0x55FF_F3F9)]);
    let (hi, lo) = combines[0];
    for cycle in [
        [
            (hi >> 20) & 0xF,
            (lo >> 28) & 0xF,
            (hi >> 15) & 0x1F,
            (lo >> 15) & 0x7,
        ],
        [
            (hi >> 5) & 0xF,
            (lo >> 24) & 0xF,
            hi & 0x1F,
            (lo >> 6) & 0x7,
        ],
    ] {
        // SHADE, ENVIRONMENT, TEXEL0, ZERO.
        assert_eq!(cycle, [4, 5, 1, 7]);
    }
}

/// Only file 338's egg meshes carry `SHADE_MINUS_ENV`, and the egg's vertex
/// shade is white, so with no ENV it draws as before.
#[test]
fn only_the_egg_carries_shade_minus_env() {
    let Some(bytes) = pack_bytes() else {
        return;
    };
    let pack = Pack::open(&bytes).unwrap();
    let mut flagged = Vec::new();
    for i in 0..pack.mesh_count() {
        let mesh = pack.mesh(i).unwrap();
        for j in 0..mesh.prim_count {
            let prim = pack.prim(mesh.first_prim + j).unwrap();
            if prim.flags & flags::SHADE_MINUS_ENV != 0 {
                flagged.push((mesh.source_file, mesh.source_offset));
            }
        }
    }
    assert!(flagged.contains(&EGG), "{flagged:x?}");
    assert!(
        flagged.iter().all(|&(file, _)| file == EGG.0),
        "{flagged:x?}"
    );
    let egg = (0..pack.mesh_count())
        .filter_map(|i| pack.mesh(i))
        .find(|m| (m.source_file, m.source_offset) == EGG)
        .unwrap();
    let verts = pack.vertices(&egg).unwrap();
    for v in verts.as_chunks::<{ ssb_rom::pack::VERTEX_SIZE }>().0 {
        assert_eq!(&v[4..7], &[0xFF, 0xFF, 0xFF]);
    }
}

/// RE-427's original-game trace at RollF/B frame 12. Matrix kind 0x4A
/// reads fighter joint 5's local rotate.x. The figatree omits three of the
/// four runtime joints, so animation index 2 drives the second model node.
#[test]
fn the_roll_egg_attachment_pose_matches_the_n64_trace() {
    let Some(bytes) = pack_bytes() else { return };
    let pack = Pack::open(&bytes).unwrap();
    for (slot, expected) in [(462, 2.9705577f32), (463, -6.3230405f32)] {
        let anim = pack.fighter_anim(6, slot).unwrap();
        let mut skeleton = ssb_rom::skeleton::Skeleton::new();
        skeleton.start(&pack, &anim, 0.0, 1.0);
        let node = skeleton.joint_node(2).unwrap();
        let object = (0..pack.object_count())
            .filter_map(|i| pack.object(i))
            .find(|o| o.first_node <= node && node < o.first_node + o.node_count)
            .unwrap();
        assert_eq!(node, object.first_node + 1);
        for _ in 0..13 {
            skeleton.tick(pack.anim_script(&anim).unwrap()).unwrap();
        }
        let angle = skeleton.node_pose(node).unwrap().rotate[0];
        assert!(
            (angle - expected).abs() < 0.0001,
            "slot {slot}: {angle} != {expected}"
        );
    }
}
