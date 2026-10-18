//! RE-419: the pack carries every VS stage's wallpaper, keyed by `GRKind`,
//! Training's three and the stage select's sprites, the wallpapers exact in
//! 5551.

use ssb_rom::pack::{Pack, SpriteDesc, TextureDesc};
use ssb_rom::psp_texture::Psm;
use ssb_rom::Archive;

fn rom() -> Option<Vec<u8>> {
    std::fs::read(ssb_tablegen::test_rom(env!("CARGO_MANIFEST_DIR"))?).ok()
}

fn pack_bytes() -> Option<Vec<u8>> {
    ssb_tablegen::test_rom(env!("CARGO_MANIFEST_DIR"))?;
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/generated/ssb64.pak");
    std::fs::read(path).ok()
}

/// A wallpaper is RGBA16: `Psm5551`, or a CLUT of its own colours when it
/// has at most 256 (RE-491).
fn assert_direct_or_palettized(t: &TextureDesc) {
    if t.texels & TextureDesc::TEXELS_PALETTIZED != 0 {
        assert!(t.psm == Psm::PsmT4 as u8 || t.psm == Psm::PsmT8 as u8);
    } else {
        assert_eq!(t.psm, Psm::Psm5551 as u8);
    }
}

#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
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
        assert_direct_or_palettized(&pack.texture(s.texture).unwrap());
    }
    assert_eq!(pack.stage_wallpaper(9), None);
}

/// `grWallpaperMakeStatic`: every bonus stage (Break the Targets, Board the
/// Platforms) draws its map's one static wallpaper, the same sprite for
/// all 24 kinds (RE-524).
#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn every_bonus_stage_wallpaper_is_packed_under_its_kind() {
    let (Some(rom), Some(bytes)) = (rom(), pack_bytes()) else {
        return;
    };
    let info = ssb_rom::rom::identify(&rom).unwrap();
    let archive = Archive::open(&rom, info.region).unwrap();
    let pack = Pack::open(&bytes).unwrap();
    for gkind in ssb_rom::stage::BONUS_WALLPAPER_GROUNDS {
        let map_id = ssb_rom::scene_roots::ground_map(u32::from(gkind)).unwrap();
        let map = archive.load(map_id).unwrap();
        let target = ssb_rom::stage::wallpaper(&map, ssb_rom::stage::BONUS_MAP_HEADER).unwrap();
        assert_eq!(target, (119, 0x26C88), "kind {gkind}");
        let s = pack.stage_wallpaper(gkind).unwrap();
        assert_eq!((s.source_file, s.source_offset), target, "kind {gkind}");
        assert_eq!(s.role, SpriteDesc::ROLE_WALLPAPER);
        assert_eq!((s.width, s.height), (300, 220));
        assert_direct_or_palettized(&pack.texture(s.texture).unwrap());
    }
}

#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
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
        assert_direct_or_palettized(&pack.texture(s.texture).unwrap());
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
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
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
