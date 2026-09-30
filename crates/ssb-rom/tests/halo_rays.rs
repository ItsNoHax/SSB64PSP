//! RE-420: the rebirth halo's rays (file 85 + 0x2A88, calling 0x2890)
//! combine `PRIMITIVE` colour with `TEXEL0` alpha under a blending mode.
//! The pack keeps their I8 texture as a `TEXTURE_BLEND` from the white
//! primitive to itself, so the GE draws white at the texel's alpha.

use ssb_rom::pack::{flags, Pack};
use ssb_rom::Archive;

const RAYS: (u32, u32) = (85, 0x2A88);

#[test]
fn the_rays_list_is_primitive_colour_and_texel_alpha() {
    let Some(rom) = std::env::var_os("SSB64_ROM").and_then(|p| std::fs::read(p).ok()) else {
        return;
    };
    let info = ssb_rom::rom::identify(&rom).unwrap();
    let archive = Archive::open(&rom, info.region).unwrap();
    let file = archive.load(RAYS.0).unwrap();
    let cmds = ssb_rom::dl::decode_list_at(&file.data[0x2890..], 0x2890).unwrap();
    let combines: Vec<(u32, u32)> = cmds
        .iter()
        .filter_map(|c| match c {
            ssb_rom::dl::Cmd::SetCombine { hi, lo } => Some((*hi, *lo)),
            _ => None,
        })
        .collect();
    assert_eq!(combines, [(0x00FF_FFFF, 0xFFFD_F2F9)]);
    let (hi, lo) = combines[0];
    // Cycle 1: RGB `(0 - 0) * 0 + PRIMITIVE`, alpha `(0 - 0) * 0 + TEXEL0`.
    assert_eq!((lo >> 15) & 0x7, 3);
    assert_eq!(((hi >> 9) & 0x7, (lo >> 9) & 0x7), (7, 1));
}

#[test]
fn the_packed_rays_keep_their_texture_and_blend_by_its_alpha() {
    let Some(_) = std::env::var_os("SSB64_ROM") else {
        return;
    };
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/generated/ssb64.pak");
    let Ok(bytes) = std::fs::read(path) else {
        return;
    };
    let pack = Pack::open(&bytes).unwrap();
    let rays = (0..pack.mesh_count())
        .filter_map(|i| pack.mesh(i))
        .find(|m| (m.source_file, m.source_offset) == RAYS)
        .unwrap();
    assert_eq!(rays.prim_count, 1);
    let p = pack.prim(rays.first_prim).unwrap();
    assert_ne!(p.texture, u32::MAX);
    assert_eq!(p.flags & flags::FLAT_COLOR, 0);
    assert_ne!(p.flags & flags::TEXTURE_BLEND, 0);
    assert_ne!(p.flags & flags::TRANSLUCENT, 0);
    assert_eq!(
        (p.texture_blend_base, p.texture_blend_target),
        (0xFFFF_FFFF, 0xFFFF_FFFF)
    );
    // The vertex colour is the primitive, fully opaque (`TEXEL0` alpha alone).
    let verts = pack.vertices(&rays).unwrap();
    for v in verts.as_chunks::<{ ssb_rom::pack::VERTEX_SIZE }>().0 {
        assert_eq!(&v[4..8], &[0xFF; 4]);
    }
}
