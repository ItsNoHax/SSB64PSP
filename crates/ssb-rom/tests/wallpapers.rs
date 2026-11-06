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

/// Every wallpaper's packed texture, once each: the `ROLE_WALLPAPER`
/// sprites (the bonus stages' share one image) and Training's three.
fn wallpaper_textures(pack: &Pack<'_>) -> Vec<(SpriteDesc, TextureDesc)> {
    let mut seen = std::collections::BTreeSet::new();
    let training = ssb_rom::sprite::TRAINING_WALLPAPER_FILES
        .into_iter()
        .map(|file| {
            pack.sprite(file, ssb_rom::sprite::TRAINING_WALLPAPER_SPRITE)
                .unwrap()
        });
    (0..pack.sprite_count())
        .filter_map(|i| pack.sprite_at(i))
        .filter(|s| s.role == SpriteDesc::ROLE_WALLPAPER)
        .chain(training)
        .filter(|s| seen.insert(s.texture))
        .map(|s| (s, pack.texture(s.texture).unwrap()))
        .collect()
}

/// RE-419's trimmed layout keeps one column past the image, in whole
/// 16-byte rows: every packed wallpaper stores its 256 rows at
/// `trimmed_stride` for its format, and its first 224 read back as
/// the ROM's 300 x 220 RGBA16 texels (exact in 5551, or through a CLUT of
/// its own colours, RE-491) with the edge texel repeated into the rest.
#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn every_packed_wallpaper_keeps_its_texels_in_the_trimmed_layout() {
    let (Some(rom), Some(bytes)) = (rom(), pack_bytes()) else {
        return;
    };
    let info = ssb_rom::rom::identify(&rom).unwrap();
    let archive = Archive::open(&rom, info.region).unwrap();
    let pack = Pack::open(&bytes).unwrap();
    let wallpapers = wallpaper_textures(&pack);
    // Fifteen `ROLE_WALLPAPER` images and Training's three.
    assert_eq!(wallpapers.len(), 18);
    for (s, t) in wallpapers {
        let at = (s.source_file, s.source_offset);
        assert_direct_or_palettized(&t);
        let bits = match t.psm {
            4 => 4,
            5 => 8,
            _ => 16,
        };
        let stride = ssb_rom::psp_texture::trimmed_stride(300, bits);
        assert_eq!(
            (t.width, t.height, t.stride),
            (300, 220, stride as u16),
            "{at:?}"
        );
        assert_eq!(t.swizzled, 1, "{at:?}");
        let row = stride as usize * bits / 8;
        let data = pack.texture_data(&t).unwrap();
        assert_eq!(data.len(), row * 256, "{at:?}");
        let palette: Vec<u32> = pack
            .palette_data(&t)
            .unwrap_or_default()
            .as_chunks::<4>()
            .0
            .iter()
            .map(|&c| u32::from_le_bytes(c))
            .collect();
        let linear = ssb_rom::psp_texture::unswizzle(data, row, 256);
        let image = ssb_rom::sprite::decode(&archive.load(s.source_file).unwrap(), s.source_offset)
            .unwrap()
            .image;
        for y in 0..224 {
            for x in 0..stride as usize {
                let v = match bits {
                    16 => {
                        u16::from_le_bytes([linear[y * row + x * 2], linear[y * row + x * 2 + 1]])
                    }
                    _ => {
                        let i = if bits == 4 {
                            (linear[y * row + x / 2] >> ((x & 1) * 4)) & 0xF
                        } else {
                            linear[y * row + x]
                        };
                        // The CLUT holds the 5551 colour widened to ABGR8888.
                        let c = palette[i as usize];
                        let five = |shift: u32| ((c >> shift) & 0xFF) as u16 >> 3;
                        five(0) | five(8) << 5 | five(16) << 10 | ((c >> 31) as u16) << 15
                    }
                };
                let (sx, sy) = (x.min(299), y.min(219));
                let px = &image.pixels[(sy * 300 + sx) * 4..][..4];
                let five = |c: u8| u16::from(c) >> 3;
                let expect = five(px[0])
                    | five(px[1]) << 5
                    | five(px[2]) << 10
                    | u16::from(px[3] >= 0x80) << 15;
                assert_eq!(v, expect, "{at:?} texel ({x}, {y})");
            }
        }
    }
}

/// The trimmed stride is the wallpapers' alone, and every texture still
/// holds its power-of-two rows (the runtime's `texture_image` size).
#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn only_the_wallpapers_have_a_trimmed_stride() {
    let Some(bytes) = pack_bytes() else {
        return;
    };
    let pack = Pack::open(&bytes).unwrap();
    let wallpapers: std::collections::BTreeSet<u32> = wallpaper_textures(&pack)
        .into_iter()
        .map(|(s, _)| s.texture)
        .collect();
    let mut trimmed = std::collections::BTreeSet::new();
    for i in 0..pack.texture_count() {
        let t = pack.texture(i).unwrap();
        if t.role != TextureDesc::ROLE_NORMAL {
            continue;
        }
        let Some(data) = pack.texture_data(&t) else {
            continue;
        };
        let bits = match t.psm {
            0..=2 => 16,
            3 => 32,
            4 => 4,
            _ => 8,
        };
        let row = (t.stride as usize * bits).div_ceil(8);
        let pow2 = u32::from(t.height).max(1).next_power_of_two() as usize;
        assert!(
            data.len() >= row * pow2,
            "texture {i}: fewer rows than the power of two"
        );
        if u32::from(t.stride) < ssb_rom::psp_texture::ge_buffer_stride(u32::from(t.width), bits) {
            trimmed.insert(i);
        }
    }
    assert_eq!(trimmed, wallpapers);
}
