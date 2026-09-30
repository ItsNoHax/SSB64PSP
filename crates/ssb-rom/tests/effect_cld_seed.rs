//! RE-421: the shield bubble (file 163's list at 0x248) and the other
//! `DObjDLLink` effects on DL links 15 and 18 draw their head-1 lists after
//! `efDisplayCLDProcDisplay` (`G_RM_CLD_SURF`, `G_AC_THRESHOLD`,
//! `G_ZBUFFER` cleared). A list that sets no render mode of its own is
//! packed with no depth test, so it covers its fighter and the ground in
//! front, as on the N64.

use ssb_rom::pack::{flags, Pack};

/// The shield, Jigglypuff's Sing (351 + 0x1F38) and Ness's PSI Magnet
/// (352 + 0x8E0).
const LISTS: [(u32, u32); 3] = [(163, 0x248), (351, 0x1F38), (352, 0x8E0)];

#[test]
fn the_link_15_effect_lists_are_packed_with_no_depth_test() {
    let Some(_) = std::env::var_os("SSB64_ROM") else {
        return;
    };
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/generated/ssb64.pak");
    let Ok(bytes) = std::fs::read(path) else {
        return;
    };
    let pack = Pack::open(&bytes).unwrap();
    for key in LISTS {
        let mesh = (0..pack.mesh_count())
            .filter_map(|i| pack.mesh(i))
            .find(|m| (m.source_file, m.source_offset) == key)
            .unwrap_or_else(|| panic!("{key:?} is packed"));
        for k in 0..mesh.prim_count {
            let p = pack.prim(mesh.first_prim + k).unwrap();
            assert_eq!(p.flags & flags::DEPTH_TEST, 0, "{key:?} prim {k}");
            assert_eq!(p.flags & flags::DEPTH_WRITE, 0, "{key:?} prim {k}");
            assert_eq!(p.flags & flags::Z_BUFFER, 0, "{key:?} prim {k}");
            assert_ne!(p.flags & flags::TRANSLUCENT, 0, "{key:?} prim {k}");
        }
    }
    // The shield sets `G_AC_THRESHOLD` itself; the seed agrees.
    let shield = (0..pack.mesh_count())
        .filter_map(|i| pack.mesh(i))
        .find(|m| (m.source_file, m.source_offset) == LISTS[0])
        .unwrap();
    let p = pack.prim(shield.first_prim).unwrap();
    assert_ne!(p.flags & flags::ALPHA_COMPARE_THRESHOLD, 0);
}
