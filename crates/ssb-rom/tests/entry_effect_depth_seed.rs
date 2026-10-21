//! RE-567: DL link 10 effects drawn after the fighters whose lists set no
//! render mode of their own run under the fighters' head-0 state
//! (`G_RM_AA_ZB_OPA_SURF`, `G_ZBUFFER`). N64 RDRAM walks find Mario's entry
//! pipe, Samus's entry point, Donkey Kong's barrel and the rebirth halo
//! there, as RE-425 found the Arwing and the car. Without the seed the pipe's
//! lid, drawn last, covers its near wall.

use ssb_rom::pack::{flags, Pack};

/// `DObjDesc` trees: the pipe (356 + 0x608), Samus's entry point
/// (349 + 0xB90), Donkey Kong's barrel (355 + 0x7C8), the rebirth halo
/// (85 + 0x2AC0), the Arwing (161 + 0x2C30) and the car (350 + 0x5FC0).
const OBJECTS: [(u32, u32); 6] = [
    (356, 0x608),
    (349, 0xB90),
    (355, 0x7C8),
    (85, 0x2AC0),
    (161, 0x2C30),
    (350, 0x5FC0),
];

#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn link_10_effects_draw_under_the_fighters_depth_state() {
    let Some(_) = ssb_tablegen::test_rom(env!("CARGO_MANIFEST_DIR")) else {
        return;
    };
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/generated/ssb64.pak");
    let Ok(bytes) = std::fs::read(path) else {
        return;
    };
    let pack = Pack::open(&bytes).unwrap();
    for key in OBJECTS {
        let object = (0..pack.object_count())
            .filter_map(|i| pack.object(i))
            .find(|o| (o.source_file, o.source_offset) == key)
            .unwrap_or_else(|| panic!("{key:?} is packed"));
        let mut opaque = 0;
        for n in 0..object.node_count {
            let node = pack.node(object.first_node + n).unwrap();
            let Some(mesh) = pack.mesh(node.mesh) else {
                continue;
            };
            for k in 0..mesh.prim_count {
                let p = pack.prim(mesh.first_prim + k).unwrap();
                assert_ne!(p.flags & flags::DEPTH_TEST, 0, "{key:?} node {n} prim {k}");
                assert_ne!(p.flags & flags::Z_BUFFER, 0, "{key:?} node {n} prim {k}");
                if p.flags & flags::TRANSLUCENT == 0 {
                    assert_ne!(p.flags & flags::DEPTH_WRITE, 0, "{key:?} node {n} prim {k}");
                    opaque += 1;
                }
                if key == OBJECTS[0] {
                    // The pipe's lids wind outward: culled from inside.
                    assert_ne!(p.flags & flags::CULL_BACK, 0, "pipe node {n} prim {k}");
                }
            }
        }
        assert!(opaque > 0, "{key:?} has a head-0 list");
    }
}
