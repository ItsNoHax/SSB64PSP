//! RE-422: the battle camera draws each pass's head-0 lists before its
//! head-1 lists, so the pack marks every head-1 prim (`flags::HEAD1`). Stage
//! layers 0, 2 and 3 draw with no depth test on both heads
//! (`grDisplayLayer{0,2,3}*ProcDisplay`), and the unlit `G_CC_SHADE` glows
//! on head 1 blend by their vertex alpha.

use ssb_rom::pack::{flags, Pack, StageDesc};

fn pack_bytes() -> Option<Vec<u8>> {
    std::env::var_os("SSB64_ROM")?;
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/generated/ssb64.pak");
    std::fs::read(path).ok()
}

/// Every prim of the mesh packed from `key`'s display list.
fn prims(pack: &Pack<'_>, key: (u32, u32)) -> Vec<u32> {
    let mesh = (0..pack.mesh_count())
        .filter_map(|i| pack.mesh(i))
        .find(|m| (m.source_file, m.source_offset) == key)
        .unwrap_or_else(|| panic!("{key:?} is packed"));
    (0..mesh.prim_count)
        .map(|k| pack.prim(mesh.first_prim + k).unwrap().flags)
        .collect()
}

#[test]
fn head1_glows_blend_and_layer_head1_lists_have_no_depth_test() {
    let Some(bytes) = pack_bytes() else {
        return;
    };
    let pack = Pack::open(&bytes).unwrap();
    let blends = flags::HEAD1 | flags::TRANSLUCENT | flags::ALPHA_BLEND;
    // Saffron City's layer-3 haze (file 112 + 0x8688): head 1 under
    // `G_RM_AA_XLU_SURF`, `G_ZBUFFER` cleared.
    for f in prims(&pack, (112, 0x8688)) {
        assert_eq!(f & blends, blends, "{f:#x}");
        assert_eq!(
            f & (flags::DEPTH_TEST | flags::DEPTH_WRITE | flags::Z_BUFFER),
            0,
            "{f:#x}"
        );
    }
    // Planet Zebes's beam (105 + 0x58B8) and Saffron City's door glow
    // (112 + 0x68B8): layer 1's head 1, `G_RM_AA_ZB_XLU_SURF`.
    for key in [(105, 0x58B8), (112, 0x68B8)] {
        for f in prims(&pack, key) {
            assert_eq!(f & blends, blends, "{key:?} {f:#x}");
            assert_ne!(f & flags::DEPTH_TEST, 0, "{key:?} {f:#x}");
            assert_eq!(f & flags::DEPTH_WRITE, 0, "{key:?} {f:#x}");
        }
    }
    // Hyrule Castle's ledge shadows (113 + 0x44C8): layer 0's head 1.
    for f in prims(&pack, (113, 0x44C8)) {
        assert_eq!(f & blends, blends, "{f:#x}");
        assert_eq!(f & flags::DEPTH_TEST, 0, "{f:#x}");
    }
}

/// No VS stage's layer 0, 2 or 3 prim tests or writes depth, and only the
/// layers that `layer_mask` draws with `SecProcDisplay` have head-1 prims:
/// layer 1 of Zebes, Sector Z and Saffron City, layer 0 of Mushroom Kingdom,
/// Sector Z and Hyrule Castle, and layer 3 of Saffron City.
#[test]
fn vs_stage_layers_keep_their_depth_state_and_heads() {
    let Some(bytes) = pack_bytes() else {
        return;
    };
    let pack = Pack::open(&bytes).unwrap();
    let mut with_head1 = Vec::new();
    for s in 0..pack.stage_count() {
        let stage = pack.stage(s).unwrap();
        let Some(gkind) = ssb_rom::stage::vs_ground_kind(stage.source_file) else {
            continue;
        };
        for (layer, &slot) in stage.layers.iter().enumerate() {
            if slot == StageDesc::NO_LAYER {
                continue;
            }
            let object = pack.object(slot).unwrap();
            let mut head1 = false;
            for n in object.first_node..object.first_node + object.node_count {
                let node = pack.node(n).unwrap();
                let Some(mesh) = pack.mesh(node.mesh) else {
                    continue;
                };
                for k in 0..mesh.prim_count {
                    let f = pack.prim(mesh.first_prim + k).unwrap().flags;
                    head1 |= f & flags::HEAD1 != 0;
                    if layer != 1 {
                        assert_eq!(
                            f & (flags::DEPTH_TEST | flags::DEPTH_WRITE),
                            0,
                            "gkind {gkind} layer {layer} {f:#x}"
                        );
                    }
                }
            }
            if head1 {
                with_head1.push((gkind, layer));
            }
        }
    }
    with_head1.sort();
    // gkind: 1 Sector, 3 Zebes, 4 Hyrule, 7 Saffron, 8 Mushroom Kingdom.
    assert_eq!(
        with_head1,
        [(1, 0), (1, 1), (3, 1), (4, 0), (7, 1), (7, 3), (8, 0)]
    );
}
