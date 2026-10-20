//! The fighter selects' floor spotlight (`mn*MakeSpotlight`, file 22
//! `MNPlayersSpotlight`, `DObjDesc` at 0x568) is a packed two-node object:
//! a transform root and one quad on display-list head 1 (`list_id 1`).
//! The texture is white (`rgb_unused`: SHADE colour, TEXEL0 alpha).

use ssb_rom::pack::{flags, Pack};

const SPOTLIGHT: (u32, u32) = (22, 0x568);

fn pack_bytes() -> Option<Vec<u8>> {
    ssb_tablegen::test_rom(env!("CARGO_MANIFEST_DIR"))?;
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/generated/ssb64.pak");
    std::fs::read(path).ok()
}

#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn the_select_spotlight_is_a_packed_head1_pool() {
    let Some(bytes) = pack_bytes() else {
        return;
    };
    let pack = Pack::open(&bytes).unwrap();
    let object = (0..pack.object_count())
        .filter_map(|i| pack.object(i))
        .find(|o| (o.source_file, o.source_offset) == SPOTLIGHT)
        .expect("file 22's spotlight DObjDesc is packed");
    assert_eq!(object.node_count, 2);
    let root = pack.node(object.first_node).unwrap();
    assert_eq!(root.mesh, ssb_rom::pack::NodeDesc::NO_MESH);
    let child = pack.node(object.first_node + 1).unwrap();
    assert_eq!(child.parent, object.first_node);
    let mesh = pack.mesh(child.mesh).unwrap();
    assert_eq!(mesh.prim_count, 1);
    let prim = pack.prim(mesh.first_prim).unwrap();
    assert_ne!(prim.flags & flags::HEAD1, 0, "list_id 1");
    // Head 1 opens with `G_RM_AA_ZB_XLU_SURF` (objdisplay.c `func_80016338`)
    // and the list sets no render mode: depth-compared against the
    // fighters' head-0 depth, never written, blended.
    assert_ne!(prim.flags & flags::DEPTH_TEST, 0);
    assert_eq!(prim.flags & flags::DEPTH_WRITE, 0);
    assert_ne!(prim.flags & flags::TRANSLUCENT, 0);
    assert_eq!(prim.flags & flags::LIT, 0, "G_GEOMETRYMODE clears lighting");
    let texture = pack.texture(prim.texture).unwrap();
    assert_eq!((texture.width, texture.height), (32, 32));
}
