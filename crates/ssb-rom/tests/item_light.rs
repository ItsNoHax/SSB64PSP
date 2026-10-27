//! Items draw on link 11 after every fighter's `ftDisplayLightsDrawReflect`,
//! whose `gSPLight` carries only a direction: LIGHT_1's colour is stale
//! graphics-heap bytes and the ambient is the last fighter's last LIGHT_2
//! write. The port's item light (`configure_item_light`) therefore lights only
//! primitives that author both colours. This pins that every other lit item
//! primitive reads no shade, so neither machine shows the inherited light.

use ssb_rom::pack::{flags, Pack};

fn pack_bytes() -> Option<Vec<u8>> {
    ssb_tablegen::test_rom(env!("CARGO_MANIFEST_DIR"))?;
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/generated/ssb64.pak");
    std::fs::read(path).ok()
}

#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn lit_item_primitives_without_light_colours_read_no_shade() {
    let Some(bytes) = pack_bytes() else {
        return;
    };
    let pack = Pack::open(&bytes).unwrap();
    let both = flags::LIGHT1_COLOR | flags::LIGHT2_COLOR;
    let mut unlit_colour = Vec::new();
    // `llITCommonObjectFileID` (86) and `llITCommonDataFileID` (251).
    for m in (0..pack.mesh_count()).filter_map(|i| pack.mesh(i)) {
        if m.source_file != 86 && m.source_file != 251 {
            continue;
        }
        for j in 0..m.prim_count {
            let f = pack.prim(m.first_prim + j).unwrap().flags;
            if f & flags::LIT != 0 && f & both != both {
                assert!(
                    f & (flags::TEXTURE_BLEND | flags::FLAT_COLOR) != 0,
                    "{}+{:#X} prim {j} reads the inherited light",
                    m.source_file,
                    m.source_offset
                );
                unlit_colour.push((m.source_file, m.source_offset, j));
            }
        }
    }
    // The Heart's fill (`(0 - 0) * 0 + PRIM`, alpha TEXEL0) is the only one.
    assert_eq!(unlit_colour, [(86, 0xFF8, 1)]);
}
