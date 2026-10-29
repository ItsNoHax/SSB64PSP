//! Master Hand's Fall wallpaper (file 114, graph 0xA188,
//! `SC1PGameBossWallpaper1ProcDisplay`: `G_CYC_2CYCLE`, `G_RM_PASS,
//! G_RM_AA_XLU_SURF2`) crossfades two tiles (RE-321) whose static windows
//! differ: tile 0 at `uls`/`ult` (256,128), tile 1 at (256,0), both clamped
//! (list 0x9D38, `MObjSub` 0x9B60). The packed UVs carry tile 0's origin
//! removed, so the second pass re-bases by the difference.

use ssb_rom::dl::Cmd;
use ssb_rom::lod_blend::{ge_texel_alpha_two_pass_channel, rdp_texel_alpha_channel};
use ssb_rom::mesh::{
    convert_sequence, AlphaBlend, InitialMaterial, MatAnimRef, SequenceItem, Source, TextureRef,
};
use ssb_rom::texture::{BitSize, Format, TextureLut};
use ssb_rom::Archive;

const BOSS: u32 = 114;
const FALL_DL: u32 = 0x9D38;
const FALL_MOBJ_SUB: u32 = 0x9B60;
const FALL_SCRIPT: u32 = 0xB3E8;

fn rom() -> Option<Vec<u8>> {
    std::fs::read(ssb_tablegen::test_rom(env!("CARGO_MANIFEST_DIR"))?).ok()
}

#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn the_fall_pairs_tiles_whose_origins_differ() {
    let Some(rom) = rom() else {
        return;
    };
    let info = ssb_rom::rom::identify(&rom).unwrap();
    let archive = Archive::open(&rom, info.region).unwrap();
    let file = archive.load(BOSS).unwrap();
    let mut cmds = ssb_rom::dl::decode_list_at(&file.data[FALL_DL as usize..], FALL_DL).unwrap();
    // The display proc's state before the tree's lists.
    cmds.splice(
        0..0,
        [
            Cmd::SetOtherModeH {
                shift: 20,
                len: 2,
                data: 0x0010_0000,
            },
            Cmd::SetOtherModeL {
                shift: 3,
                len: 29,
                data: 0x0C18_41C8,
            },
            Cmd::SetPrimColor {
                m: 0,
                l: 0,
                rgba: [0, 0, 0, 0xFF],
            },
        ],
    );
    let mobj = ssb_rom::mobj::read_sub(&file, FALL_MOBJ_SUB).expect("MObjSub");
    assert!(mobj.loads_next_block && mobj.prim_lod.is_some());
    let mobjs = [mobj];
    let anims = [Some(MatAnimRef {
        source_file: BOSS,
        script: FALL_SCRIPT,
        source_mobj: FALL_MOBJ_SUB,
    })];
    let items = [SequenceItem {
        cmds: &cmds,
        world: ssb_rom::scene::Mat4::IDENTITY,
        mobjs: &mobjs,
        mat_anims: &anims,
        depth_seed: None,
        stream: 0,
    }];
    let mesh = convert_sequence(&items, Source::of(&file), InitialMaterial::SCENE)
        .remove(0)
        .unwrap();
    let m = mesh.primitives[0].material;
    let t0 = m.texture.expect("TEXEL0");
    let lod = m.lod_blend.expect("two-tile blend");
    assert_eq!((t0.origin_s, t0.origin_t), (256, 128));
    assert_eq!((lod.next.origin_s, lod.next.origin_t), (256, 0));
    assert!(t0.clamp_s && t0.clamp_t && lod.next.clamp_s && lod.next.clamp_t);
    assert_eq!(lod.origin_delta, [0, 128]);
    assert_eq!(lod.tile1_params, [64, 160, 64]);
    assert!(m.translucent);
    // Each pass is `TEXELn_ALPHA * PRIM_ALPHA`: the vertices carry the
    // display proc's PRIM alpha, which the renderer replaces per pass.
    assert_eq!(m.alpha_blend, Some(AlphaBlend::Prim(0xFF)));
    assert_eq!(lod.prim_alpha, 0xFF);
    assert!(!lod.shade_alpha);
}

/// A tile's image: its texels' RGBA over the window, with the TLUT off.
fn image_rgba(file: &[u8], t: &TextureRef) -> Vec<[u8; 4]> {
    assert!(!t.tlut.enabled());
    let (w, h) = (u32::from(t.width), u32::from(t.height));
    let row = ssb_rom::texture::data_len(u32::from(t.source_width.max(t.width)), 1, t.size);
    let used = ssb_rom::texture::data_len(w, 1, t.size);
    let data: Vec<u8> = (0..h as usize)
        .flat_map(|y| {
            let at = t.data_offset as usize + y * row;
            file[at..at + used].iter().copied()
        })
        .collect();
    let img = ssb_rom::texture::decode_lut(&data, w, h, t.format, t.size, TextureLut::None, &[], 0)
        .unwrap();
    (0..(w * h) as usize).map(|i| img.get(i)).collect()
}

/// RE-321: the texel-alpha lowering's assumptions, on the ROM's image.
///
/// * One RGB over every texel: the passes add the same colour.
/// * Tile 1 samples `32 - ScrV` rows below tile 0 (origins (256,128) and
///   (256,0) quarter texels, both clamped), from TEXEL0's own image or the
///   second sprite the script's `TextureIDNext` selects, so the two alphas
///   can be correlated by the image's own structure. The compensated two
///   passes' error against the RDP is measured for each tile-1 sprite over
///   the window and every row offset the scroll reaches.
#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn the_fall_texel_alpha_passes_track_the_rdp_over_the_image() {
    let Some(rom) = rom() else {
        return;
    };
    let info = ssb_rom::rom::identify(&rom).unwrap();
    let archive = Archive::open(&rom, info.region).unwrap();
    let file = archive.load(BOSS).unwrap();
    let mobj = ssb_rom::mobj::read_sub(&file, FALL_MOBJ_SUB).expect("MObjSub");
    let mut cmds = ssb_rom::dl::decode_list_at(&file.data[FALL_DL as usize..], FALL_DL).unwrap();
    cmds.splice(
        0..0,
        [
            Cmd::SetOtherModeH {
                shift: 20,
                len: 2,
                data: 0x0010_0000,
            },
            Cmd::SetOtherModeL {
                shift: 3,
                len: 29,
                data: 0x0C18_41C8,
            },
        ],
    );
    let mobjs = [mobj];
    let anims = [Some(MatAnimRef {
        source_file: BOSS,
        script: FALL_SCRIPT,
        source_mobj: FALL_MOBJ_SUB,
    })];
    let items = [SequenceItem {
        cmds: &cmds,
        world: ssb_rom::scene::Mat4::IDENTITY,
        mobjs: &mobjs,
        mat_anims: &anims,
        depth_seed: None,
        stream: 0,
    }];
    let mesh = convert_sequence(&items, Source::of(&file), InitialMaterial::SCENE)
        .remove(0)
        .unwrap();
    let m = mesh.primitives[0].material;
    let t0 = m.texture.unwrap();
    let t1 = m.lod_blend.unwrap().next;
    // IA8: four bits of intensity, four of alpha.
    assert_eq!((t0.format, t0.size), (Format::Ia, BitSize::Bits8));
    let a = image_rgba(&file.data, &t0);
    assert_eq!(
        a,
        image_rgba(&file.data, &t1),
        "both tiles start on one image"
    );
    // `TextureIDNext` swaps tile 1's image (`gcDrawMObjForDObj`): every
    // sprite the Fall's own script selects.
    let mut next_ids = std::collections::BTreeSet::new();
    let mut j = ssb_rom::matanim::MaterialJoint::start(FALL_SCRIPT, 0.0);
    for _ in 0..2000 {
        j.tick(&file.data, 1.0).unwrap();
        if let Some(v) = j.track_value(ssb_rom::matanim::TRACK_TEXTURE_ID_NEXT) {
            next_ids.insert(v.max(0.0) as usize);
        }
        if j.ended() || j.looped() {
            break;
        }
    }
    next_ids.insert(0);
    let sprites =
        ssb_rom::mobj::read_sprites(&file, FALL_MOBJ_SUB, next_ids.last().unwrap() + 1).unwrap();
    assert_eq!(
        sprites[0].offset, t0.data_offset,
        "sprite 0 is TEXEL0's image"
    );
    let images: Vec<Vec<[u8; 4]>> = next_ids
        .iter()
        .map(|&i| {
            assert_eq!(sprites[i].file, None);
            image_rgba(
                &file.data,
                &TextureRef {
                    data_offset: sprites[i].offset,
                    ..t1
                },
            )
        })
        .collect();
    let rgb = [a[0][0], a[0][1], a[0][2]];
    assert!(
        images.iter().flatten().all(|c| c[..3] == rgb),
        "one texel colour over both tiles' images"
    );
    let mean = ((a.iter().map(|c| u32::from(c[3])).sum::<u32>() + a.len() as u32 / 2)
        / a.len() as u32) as u8;
    let (w, h) = (usize::from(t0.width), usize::from(t0.height));
    let mut report = vec![format!(
        "next sprites {:?} at {:X?}",
        next_ids,
        next_ids
            .iter()
            .map(|&i| sprites[i].offset)
            .collect::<Vec<_>>()
    )];
    for (b, id) in images.iter().zip(&next_ids) {
        for prim_alpha in [58u8, 255] {
            for frac in [51u8, 128, 204] {
                let (mut signed, mut n, mut worst) = (0i64, 0i64, 0u8);
                for scroll in 0..h {
                    for y in 0..h {
                        let y1 = (y + 32 + h - scroll) % h;
                        for x in 0..w {
                            let (ta, tb) = (a[y * w + x][3], b[y1 * w + x][3]);
                            // The widest colour step: white over black.
                            let rdp = rdp_texel_alpha_channel(ta, tb, prim_alpha, frac, 8, 255, 0);
                            let ge = ge_texel_alpha_two_pass_channel(
                                ta, tb, prim_alpha, frac, mean, 255, 0,
                            );
                            signed += i64::from(ge) - i64::from(rdp);
                            n += 1;
                            worst = worst.max(ge.abs_diff(rdp));
                        }
                    }
                }
                let bias = signed as f64 / n as f64;
                report.push(format!(
                    "T1 {id} A {prim_alpha} f {frac}: bias {bias:.2} worst {worst}"
                ));
                // Uncompensated (`mean` 0) measures bias -2.6 and worst 4 at
                // A 58, bias -29.8 and worst 63 at A 255.
                assert!(bias.abs() <= BIAS_MAX, "{report:?}");
                // The worst pixel is the cross term `a0 * a1 * (M - C)` the
                // compensation spreads over the mean (measured 34 at A 255).
                assert!(
                    worst <= if prim_alpha == 255 { 36 } else { 3 },
                    "{report:?}"
                );
            }
        }
    }
    println!("mean alpha {mean}; {report:?}");
}

/// The mean error bound of
/// [`the_fall_texel_alpha_passes_track_the_rdp_over_the_image`], in 8-bit
/// steps of a white-over-black pixel (measured -1.76 at worst).
const BIAS_MAX: f64 = 2.0;
