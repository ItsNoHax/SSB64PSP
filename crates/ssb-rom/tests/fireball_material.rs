//! The Fireball list (file 297) starts at `WPAttributes.data` (0x1A8), six
//! commands before the decomp's `SuperJumpPunchDL` label (0x1D8): those set
//! its TLUT, alpha compare, `G_RM_AA_TEX_EDGE` render mode and combiner, so
//! the pack converts it as the RE-424 alpha-edge material, like the Charge
//! Shot (321 + 0x270) and Falcon Punch (333 + 0x760).

use ssb_rom::dl::Cmd;
use ssb_rom::pack::{flags, Pack};
use ssb_rom::Archive;

const FIREBALL: u32 = 297;
const FIREBALL_DATA: u32 = 0x1A8;
/// Pack keys: Mario's mesh by the decomp label, Luigi's by its palette.
const MESH_KEYS: [u32; 2] = [0x1D8, 0x08];

fn rom() -> Option<Vec<u8>> {
    std::fs::read(ssb_tablegen::test_rom(env!("CARGO_MANIFEST_DIR"))?).ok()
}

fn pack_bytes() -> Option<Vec<u8>> {
    ssb_tablegen::test_rom(env!("CARGO_MANIFEST_DIR"))?;
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/generated/ssb64.pak");
    std::fs::read(path).ok()
}

#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn the_fireball_list_sets_its_own_tex_edge_mode_and_combiner() {
    let Some(rom) = rom() else {
        return;
    };
    let info = ssb_rom::rom::identify(&rom).unwrap();
    let archive = Archive::open(&rom, info.region).unwrap();
    let file = archive.load(FIREBALL).unwrap();
    let cmds =
        ssb_rom::dl::decode_list_at(&file.data[FIREBALL_DATA as usize..], FIREBALL_DATA).unwrap();
    let modes: Vec<u32> = cmds
        .iter()
        .filter_map(|c| match c {
            Cmd::SetOtherModeL {
                shift: 3,
                len: 29,
                data,
            } => Some(*data),
            _ => None,
        })
        .collect();
    assert_eq!(modes, [0x0055_3048]);
    let combines: Vec<(u32, u32)> = cmds
        .iter()
        .filter_map(|c| match c {
            Cmd::SetCombine { hi, lo } => Some((*hi, *lo)),
            _ => None,
        })
        .collect();
    assert_eq!(combines, [(0x0012_7E24, 0xFFFF_F3F9)]);
}

#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn both_fireball_meshes_are_head_one_alpha_edges() {
    let Some(bytes) = pack_bytes() else {
        return;
    };
    let pack = Pack::open(&bytes).unwrap();
    for key in MESH_KEYS {
        let mesh = (0..pack.mesh_count())
            .filter_map(|i| pack.mesh(i))
            .find(|m| (m.source_file, m.source_offset) == (FIREBALL, key))
            .unwrap_or_else(|| panic!("no Fireball mesh {key:#x}"));
        assert!(mesh.prim_count > 0);
        for j in 0..mesh.prim_count {
            let f = pack.prim(mesh.first_prim + j).unwrap().flags;
            let want = flags::ALPHA_TEST | flags::ALPHA_COMPARE_THRESHOLD | flags::HEAD1;
            assert_eq!(f & want, want, "{key:#x}: {f:#010x}");
            assert_eq!(f & flags::TRANSLUCENT, 0, "{key:#x}: {f:#010x}");
        }
    }
}
