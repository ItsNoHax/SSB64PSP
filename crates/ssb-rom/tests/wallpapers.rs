//! RE-419: the pack carries every VS stage's wallpaper, keyed by `GRKind`,
//! Training's three and the stage select's sprites, the wallpapers exact in
//! 5551.

use ssb_rom::pack::{Pack, SpriteDesc};
use ssb_rom::psp_texture::Psm;
use ssb_rom::Archive;

fn rom() -> Option<Vec<u8>> {
    std::fs::read(std::env::var_os("SSB64_ROM")?).ok()
}

fn pack_bytes() -> Option<Vec<u8>> {
    std::env::var_os("SSB64_ROM")?;
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/generated/ssb64.pak");
    std::fs::read(path).ok()
}

#[test]
fn every_stage_wallpaper_is_packed_under_its_kind() {
    let (Some(rom), Some(bytes)) = (rom(), pack_bytes()) else {
        return;
    };
    let info = ssb_rom::rom::identify(&rom).unwrap();
    let archive = Archive::open(&rom, info.region).unwrap();
    let pack = Pack::open(&bytes).unwrap();
    for (gkind, &map_id) in ssb_rom::stage::VS_GROUND_FILES.iter().enumerate() {
        let map = archive.load(map_id).unwrap();
        let (file, at) = ssb_rom::stage::wallpaper(&map, ssb_rom::stage::MAP_HEADER).unwrap();
        let s = pack.stage_wallpaper(gkind as u8).unwrap();
        assert_eq!((s.source_file, s.source_offset), (file, at), "kind {gkind}");
        assert_eq!(s.role, SpriteDesc::ROLE_WALLPAPER);
        assert_eq!((s.width, s.height), (300, 220));
        let t = pack.texture(s.texture).unwrap();
        assert_eq!(t.psm, Psm::Psm5551 as u8);
    }
    assert_eq!(pack.stage_wallpaper(9), None);
}

#[test]
fn trainings_wallpapers_and_the_stage_select_sprites_are_packed() {
    let Some(bytes) = pack_bytes() else {
        return;
    };
    let pack = Pack::open(&bytes).unwrap();
    for file in ssb_rom::sprite::TRAINING_WALLPAPER_FILES {
        let s = pack
            .sprite(file, ssb_rom::sprite::TRAINING_WALLPAPER_SPRITE)
            .unwrap();
        assert_eq!((s.width, s.height), (300, 220));
        assert_eq!(pack.texture(s.texture).unwrap().psm, Psm::Psm5551 as u8);
    }
    for &at in ssb_rom::sprite::MN_MAPS.offsets {
        assert!(
            pack.sprite(ssb_rom::sprite::MN_MAPS.file, at).is_some(),
            "{at:#x}"
        );
    }
}

/// 5551 holds an RGBA16 texel exactly: the packed wallpaper reads back as
/// the ROM's texels.
#[test]
fn a_packed_wallpaper_keeps_its_texels() {
    let (Some(rom), Some(bytes)) = (rom(), pack_bytes()) else {
        return;
    };
    let info = ssb_rom::rom::identify(&rom).unwrap();
    let archive = Archive::open(&rom, info.region).unwrap();
    let pack = Pack::open(&bytes).unwrap();
    let s = pack.stage_wallpaper(6).unwrap();
    let image = ssb_rom::sprite::decode(&archive.load(s.source_file).unwrap(), s.source_offset)
        .unwrap()
        .image;
    let t = pack.texture(s.texture).unwrap();
    let data = pack.texture_data(&t).unwrap();
    let unswizzled = if t.swizzled != 0 {
        ssb_rom::psp_texture::unswizzle(data, t.stride as usize * 2, 256)
    } else {
        data.to_vec()
    };
    for (x, y) in [(0u32, 0u32), (150, 110), (299, 219), (17, 203)] {
        let at = ((y * t.stride as u32 + x) * 2) as usize;
        let v = u16::from_le_bytes([unswizzled[at], unswizzled[at + 1]]);
        let px = &image.pixels[((y * 300 + x) * 4) as usize..][..4];
        let five = |c: u8| u16::from(c) >> 3;
        let expect =
            five(px[0]) | five(px[1]) << 5 | five(px[2]) << 10 | u16::from(px[3] >= 0x80) << 15;
        assert_eq!(v, expect, "texel ({x}, {y})");
    }
}
