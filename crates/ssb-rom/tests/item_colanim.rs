//! `itDisplayColAnim{OPA,XLU}` draws the Bob-omb, the Hammer and the Link
//! Bomb in two-cycle mode with ENV their colour animation's colour. Their
//! bodies' second combiner cycle is `(ENV - COMBINED) * ENV_ALPHA +
//! COMBINED`, which the pack marks `ENV_LERP`.

use ssb_rom::pack::{flags, Pack};

fn pack_bytes() -> Option<Vec<u8>> {
    std::env::var_os("SSB64_ROM")?;
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/generated/ssb64.pak");
    std::fs::read(path).ok()
}

/// Every primitive of the lists, by `(file, offset)`, and whether each
/// carries `ENV_LERP`.
fn env_lerp(pack: &Pack<'_>, key: (u32, u32)) -> Vec<bool> {
    (0..pack.mesh_count())
        .filter_map(|i| pack.mesh(i))
        .filter(|m| (m.source_file, m.source_offset) == key)
        .flat_map(|m| (0..m.prim_count).map(move |j| pack.prim(m.first_prim + j).unwrap().flags))
        .map(|f| f & flags::ENV_LERP != 0)
        .collect()
}

#[test]
fn the_colour_animated_items_blend_towards_env() {
    let Some(bytes) = pack_bytes() else {
        return;
    };
    let pack = Pack::open(&bytes).unwrap();
    // File 86: the Hammer (0x25F0) and the Bob-omb (its walk lists are not
    // packed);
    // file 353: the Link Bomb's body.
    for key in [(86, 0x25F0), (86, 0x3310), (353, 0x16F8)] {
        let prims = env_lerp(&pack, key);
        assert!(!prims.is_empty(), "{key:x?} not packed");
        assert!(prims.iter().all(|&e| e), "{key:x?}: {prims:?}");
    }
    // The Link Bomb's spark is `(PRIM - ENV) * TEXEL0 + ENV` and sets its
    // own ENV before its triangles, so the animation never reaches it.
    assert!(env_lerp(&pack, (353, 0x17E8)).iter().all(|&e| !e));
}
