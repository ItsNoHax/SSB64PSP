//! RE-423: Yoshi's Island's fruit panel and platforms, and Saffron City's
//! gate, as the N64 draws them.

use ssb_rom::pack::{flags, Pack, VERTEX_SIZE};

fn pack_bytes() -> Option<Vec<u8>> {
    std::env::var_os("SSB64_ROM")?;
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/generated/ssb64.pak");
    std::fs::read(path).ok()
}

fn mesh(pack: &Pack<'_>, key: (u32, u32)) -> ssb_rom::pack::MeshDesc {
    (0..pack.mesh_count())
        .filter_map(|i| pack.mesh(i))
        .find(|m| (m.source_file, m.source_offset) == key)
        .unwrap_or_else(|| panic!("{key:?} is packed"))
}

/// The fruit panel (file 111 + 0x640) draws its 66-texel RGBA32 image in
/// five `G_LOADTILE` strips; each binds its own rows, so no two strips
/// share a texture.
#[test]
fn the_fruit_panel_strips_bind_their_own_rows() {
    let Some(bytes) = pack_bytes() else {
        return;
    };
    let pack = Pack::open(&bytes).unwrap();
    let m = mesh(&pack, (111, 0x640));
    let mut strips: Vec<u32> = (0..m.prim_count)
        .map(|k| pack.prim(m.first_prim + k).unwrap().texture)
        .filter(|&t| t != u32::MAX)
        .filter(|&t| pack.texture(t).unwrap().tile_source_width == 66)
        .collect();
    strips.sort_unstable();
    strips.dedup();
    assert_eq!(strips.len(), 5, "{strips:?}");
}

/// Yoshi's Island's layer 1 (file 111 + 0x49A0) clears `G_LIGHTING` first,
/// so the platform tops keep their vertex colours: the right one (227, 227,
/// 132), the left one (191, 170, 156), the top one (180, 180, 205).
#[test]
fn the_platform_tops_keep_their_vertex_colours() {
    let Some(bytes) = pack_bytes() else {
        return;
    };
    let pack = Pack::open(&bytes).unwrap();
    let m = mesh(&pack, (111, 0x49A0));
    let v = pack.vertices(&m).unwrap();
    let colours: Vec<u32> = v
        .chunks(VERTEX_SIZE)
        .map(|c| u32::from_le_bytes(c[4..8].try_into().unwrap()))
        .collect();
    for abgr in [0xFF84_E3E3, 0xFF9C_AABF, 0xFFCD_B4B4] {
        assert!(colours.contains(&abgr), "{abgr:08x} in {colours:08x?}");
    }
}

/// Saffron City's gate (file 160) draws on DL link 6 after layer 1, under
/// its `G_ZBUFFER` and `G_RM_AA_ZB_OPA_SURF`: the three head-0 panels test
/// and write depth, so the door frame's lights show past them.
#[test]
fn the_saffron_gate_tests_and_writes_depth() {
    let Some(bytes) = pack_bytes() else {
        return;
    };
    let pack = Pack::open(&bytes).unwrap();
    for dl in [0x420, 0x4F0, 0x5D0] {
        let m = mesh(&pack, (160, dl));
        for k in 0..m.prim_count {
            let f = pack.prim(m.first_prim + k).unwrap().flags;
            assert_eq!(
                f & (flags::DEPTH_TEST | flags::DEPTH_WRITE),
                flags::DEPTH_TEST | flags::DEPTH_WRITE,
                "{dl:#x} {f:#x}"
            );
        }
    }
}
