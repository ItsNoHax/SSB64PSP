//! `convert_texture` on synthetic files: palette banks, format choice,
//! compensation entry, stale TLUT entries, determinism.

use std::cell::RefCell;

use ssb_rom::psp_texture::Psm;

use crate::*;

/// The pending conversion's `(method, attempted, natural, final)`, as the
/// `romtool residuals` recorder would file it.
#[derive(Default)]
struct Pending(RefCell<Option<(&'static str, bool, Psm, Psm)>>);

impl PackObserver for Pending {
    fn enabled(&self) -> bool {
        true
    }

    fn record_conversion(&self, c: Conversion) {
        *self.0.borrow_mut() = Some((c.method, c.attempted, c.source_psm, c.final_psm));
    }

    fn discard_conversion(&self) {
        *self.0.borrow_mut() = None;
    }
}

/// RE-223/`R2.0`/P2: `G_SETTILE.palette` selects a 16-entry bank within
/// a multi-bank loaded TLUT. Three distinct-coloured 16-entry banks,
/// matching the real file 86 `ITCommonObject` shape RE-223 measured
/// (three banks, real instances requesting bank 1) -- picking the wrong
/// bank cannot pass this by accident.
#[test]
fn convert_texture_resolves_the_requested_palette_bank() {
    let opts = PackOptions::default();
    let ctx = &PackCtx::new(&opts, &NoLog, &NoObserver);
    use ssb_rom::mesh::TextureRef;
    use ssb_rom::texture::{BitSize, Format};

    let mut data = vec![0u8; 5];
    data[4] = 0x00; // CI4 texel: high nibble -> palette index 0.
    for bank in 0..3u16 {
        let v: u16 = match bank {
            0 => 0x0001,
            1 => 0xFFFF,
            _ => 0x8000,
        };
        for _ in 0..16 {
            data.extend_from_slice(&v.to_be_bytes());
        }
    }
    let file = ssb_rom::archive::File {
        id: 0,
        data,
        extern_relocs: Vec::new(),
        intern_relocs: Vec::new(),
    };
    let texels = crate::Texels {
        home: &file,
        all: &[],
    };

    let base = TextureRef {
        data_file: None,
        data_offset: 4,
        format: Format::Ci,
        size: BitSize::Bits4,
        width: 1,
        height: 1,
        source_width: 1,
        palette_file: None,
        palette_offset: Some(5),
        palette_entries: 48,
        palette: 1,
        mirror_s: false,
        mirror_t: false,
        clamp_s: false,
        clamp_t: false,
        framebuffer: false,
        rgb_unused: false,
        rgb_lerp: None,
        origin_s: 0,
        origin_t: 0,
        mask_s: 0,
        mask_t: 0,
        drawn_width: 1,
        drawn_height: 1,
        tlut: ssb_rom::texture::TextureLut::Rgba16,
    };
    let expect_abgr = |v: u16| ssb_rom::psp_texture::pack_abgr(ssb_rom::texture::rgba5551(v));

    let packed = crate::convert_texture(
        ctx,
        texels,
        &base,
        &crate::filter_coverage::Coverage::default(),
        false,
        None,
        ssb_rom::filter_compensation::AlphaPolicy::Opaque,
        None,
        None,
    )
    .expect("CI4 texture converts");
    assert_eq!(
        packed.palette[0],
        expect_abgr(0xFFFF),
        "palette == 1 must read bank 1's colours, not bank 0's"
    );

    let bank0 = TextureRef { palette: 0, ..base };
    let packed0 = crate::convert_texture(
        ctx,
        texels,
        &bank0,
        &crate::filter_coverage::Coverage::default(),
        false,
        None,
        ssb_rom::filter_compensation::AlphaPolicy::Opaque,
        None,
        None,
    )
    .expect("CI4 texture converts");
    assert_eq!(
        packed0.palette[0],
        expect_abgr(0x0001),
        "palette == 0 (the overwhelming common case) must stay bank 0 -- a no-op"
    );

    // Guard: a bank beyond the loaded TLUT should not occur on real
    // content (RE-223's own measurement), but must not panic or index
    // out of bounds -- falls back to bank 0 rather than losing the
    // palette entirely.
    let out_of_range = TextureRef { palette: 5, ..base };
    let packed_guard = crate::convert_texture(
        ctx,
        texels,
        &out_of_range,
        &crate::filter_coverage::Coverage::default(),
        false,
        None,
        ssb_rom::filter_compensation::AlphaPolicy::Opaque,
        None,
        None,
    )
    .expect("an out-of-range bank must not panic");
    assert_eq!(packed_guard.palette[0], expect_abgr(0x0001));
}

/// One `convert_texture` call on a synthetic file, with the residual
/// recorder on and every texture listed for compensation: the packed texture, whether a compensated result was
/// kept, and the recorder's `(method, attempted, natural, final)`.
fn convert_recorded(
    home: u32,
    data: Vec<u8>,
    t: &ssb_rom::mesh::TextureRef,
) -> (
    ssb_rom::psp_texture::PspTexture,
    bool,
    (
        &'static str,
        bool,
        ssb_rom::psp_texture::Psm,
        ssb_rom::psp_texture::Psm,
    ),
) {
    let file = ssb_rom::archive::File {
        id: home,
        data,
        extern_relocs: Vec::new(),
        intern_relocs: Vec::new(),
    };
    let texels = crate::Texels {
        home: &file,
        all: &[],
    };
    let opts = PackOptions {
        list_all: true,
        ..PackOptions::default()
    };
    let observer = Pending::default();
    let ctx = &PackCtx::new(&opts, &NoLog, &observer);
    let mut compensated = false;
    let packed = crate::convert_texture(
        ctx,
        texels,
        t,
        &crate::filter_coverage::Coverage::default(),
        false,
        None,
        ssb_rom::filter_compensation::AlphaPolicy::Opaque,
        Some(&mut compensated),
        None,
    )
    .expect("texture converts");
    let recorded = observer.0.take().expect("conversion recorded");
    (packed, compensated, recorded)
}

/// A `width` x `height` tile-0 texture at `data_offset`, drawn with the
/// given TLUT mode and no palette.
fn synthetic_texture(
    format: ssb_rom::texture::Format,
    size: ssb_rom::texture::BitSize,
    width: u16,
    height: u16,
    data_offset: u32,
) -> ssb_rom::mesh::TextureRef {
    ssb_rom::mesh::TextureRef {
        data_file: None,
        data_offset,
        format,
        size,
        width,
        height,
        source_width: width,
        palette_file: None,
        palette_offset: None,
        palette_entries: 0,
        palette: 0,
        mirror_s: false,
        mirror_t: false,
        clamp_s: false,
        clamp_t: false,
        framebuffer: false,
        rgb_unused: false,
        rgb_lerp: None,
        origin_s: 0,
        origin_t: 0,
        mask_s: 0,
        mask_t: 0,
        drawn_width: width,
        drawn_height: height,
        tlut: ssb_rom::texture::TextureLut::None,
    }
}

/// A 5-bit level per texel of a `width`-wide diagonal ridge: a
/// smooth shape whose diagonal slope is where 3-point and bilinear
/// filtering disagree, and where the compensator measurably helps.
fn ridge(width: usize, height: usize) -> Vec<u8> {
    (0..width * height)
        .map(|i| {
            let (x, y) = ((i % width) as i32, (i / width) as i32);
            ((x - y).abs() * 5).min(31) as u8
        })
        .collect()
}

fn widen5(l: u8) -> u8 {
    (l << 3) | (l >> 2)
}

/// RE-313: an RGBA16 source enters the compensator and keeps its natural
/// `Psm5551` format when the 5551-quantized result passes the gates.
#[test]
fn rgba16_compensation_is_kept_as_psm5551() {
    use ssb_rom::psp_texture::Psm;
    use ssb_rom::texture::{BitSize, Format};
    let data: Vec<u8> = ridge(8, 8)
        .into_iter()
        .map(|l| u16::from(l) << 11 | u16::from(l) << 6 | u16::from(l) << 1 | 1)
        .flat_map(u16::to_be_bytes)
        .collect();
    let data = [vec![0; 8], data].concat();
    let t = synthetic_texture(Format::Rgba, BitSize::Bits16, 8, 8, 8);
    let (packed, compensated, (method, attempted, natural, final_psm)) =
        convert_recorded(1, data, &t);
    assert!(attempted, "{method}");
    assert_eq!(natural, Psm::Psm5551);
    assert!(compensated, "{method}");
    assert_eq!(method, "direct-rgba5551");
    assert_eq!(final_psm, Psm::Psm5551);
    assert_eq!(packed.format, Psm::Psm5551);
}

/// RE-313: an RGBA16 texture the compensator cannot improve keeps its
/// natural 5551 format, bit-exact at level 0, instead of the old forced
/// RGBA8888 expansion.
#[test]
fn uncompensated_rgba16_is_not_promoted() {
    use ssb_rom::psp_texture::Psm;
    use ssb_rom::texture::{BitSize, Format};
    let data: Vec<u8> = [0u16; 4]
        .into_iter()
        .chain([0x7BC1u16; 64])
        .flat_map(u16::to_be_bytes)
        .collect();
    let t = synthetic_texture(Format::Rgba, BitSize::Bits16, 8, 8, 8);
    let (packed, compensated, (_, attempted, _, final_psm)) = convert_recorded(1, data, &t);
    assert!(attempted);
    assert!(!compensated);
    assert_eq!(final_psm, Psm::Psm5551);
    assert_eq!(packed.format, Psm::Psm5551);
    assert_eq!(
        u16::from_le_bytes([packed.data[0], packed.data[1]]),
        ssb_rom::psp_texture::pack_5551(ssb_rom::texture::rgba5551(0x7BC1))
    );
}

/// RE-313: RGBA32 and IA16/IA8 sources enter the same compensator and
/// stay `Psm8888`, their natural PSP format.
#[test]
fn rgba32_and_ia_sources_enter_compensation() {
    use ssb_rom::psp_texture::Psm;
    use ssb_rom::texture::{BitSize, Format};
    let rgba32: Vec<u8> = ridge(8, 8)
        .into_iter()
        .flat_map(|l| [widen5(l), widen5(l), widen5(l), 255])
        .collect();
    let ia16: Vec<u8> = ridge(8, 8)
        .into_iter()
        .flat_map(|l| [widen5(l), 255])
        .collect();
    let ia8: Vec<u8> = ridge(8, 8)
        .into_iter()
        .map(|l| (l >> 1) << 4 | 0xF)
        .collect();
    for (format, size, data) in [
        (Format::Rgba, BitSize::Bits32, rgba32),
        (Format::Ia, BitSize::Bits16, ia16),
        (Format::Ia, BitSize::Bits8, ia8),
    ] {
        let t = synthetic_texture(format, size, 8, 8, 8);
        let data = [vec![0; 8], data].concat();
        let (packed, compensated, (method, attempted, natural, _)) = convert_recorded(1, data, &t);
        assert!(attempted, "{format:?}/{size:?}: {method}");
        assert!(compensated, "{format:?}/{size:?}: {method}");
        assert_eq!(natural, Psm::Psm8888);
        assert_eq!(packed.format, Psm::Psm8888);
    }
}

/// RE-313: the RE-283 bypass still avoids the paletted transport, but its
/// texels now go through the compensator instead of around it.
#[test]
fn re283_bypass_textures_are_compensated() {
    use ssb_rom::psp_texture::Psm;
    use ssb_rom::texture::{BitSize, Format};
    const TEXELS: usize = 0xCF18;
    const PALETTE: usize = TEXELS + 128;
    let mut data = vec![0u8; PALETTE + 32];
    for (i, l) in ridge(16, 16).into_iter().enumerate() {
        let v = l >> 1;
        data[TEXELS + i / 2] |= if i % 2 == 0 { v << 4 } else { v };
    }
    for i in 0..16u16 {
        let l = i * 2 + 1;
        let at = PALETTE + i as usize * 2;
        data[at..at + 2].copy_from_slice(&(l << 11 | l << 6 | l << 1 | 1).to_be_bytes());
    }
    let t = ssb_rom::mesh::TextureRef {
        palette_offset: Some(PALETTE as u32),
        palette_entries: 16,
        tlut: ssb_rom::texture::TextureLut::Rgba16,
        ..synthetic_texture(Format::Ci, BitSize::Bits4, 16, 16, TEXELS as u32)
    };
    // File 324, offset 0xCF18: Link's boot, RE-283's first bypass.
    let (packed, compensated, (method, attempted, natural, final_psm)) =
        convert_recorded(324, data, &t);
    assert_eq!(
        natural,
        Psm::PsmT4,
        "the report still names the natural format"
    );
    assert_eq!(final_psm, Psm::Psm8888, "the bypass still avoids PsmT4");
    assert_eq!(packed.format, Psm::Psm8888);
    assert!(packed.palette.is_empty());
    assert!(attempted, "{method}");
    assert!(compensated, "{method}");
}

/// RE-313: an I4 texture drawn with the TLUT off packs the intensity ramp
/// even when a TLUT is loaded; drawn with it on, it packs the TLUT.
#[test]
fn intensity_textures_pack_the_tlut_only_when_it_is_enabled() {
    use ssb_rom::texture::{BitSize, Format, TextureLut};
    // Texels at 8, TLUT at 16.
    let mut data = vec![0u8; 16 + 32];
    data[8..16].fill(0x0F);
    for i in 0..16u16 {
        let at = 16 + i as usize * 2;
        data[at..at + 2].copy_from_slice(&(0x0801 + i).to_be_bytes());
    }
    let off = synthetic_texture(Format::I, BitSize::Bits4, 4, 4, 8);
    let (packed_off, _, _) = convert_recorded(1, data.clone(), &off);
    assert_eq!(
        packed_off.palette,
        ssb_rom::psp_texture::intensity_palette(BitSize::Bits4)
    );

    let on = ssb_rom::mesh::TextureRef {
        palette_offset: Some(16),
        palette_entries: 16,
        tlut: TextureLut::Rgba16,
        ..off
    };
    let (packed_on, _, _) = convert_recorded(1, data, &on);
    assert_eq!(
        packed_on.palette[15],
        ssb_rom::psp_texture::pack_abgr(ssb_rom::texture::rgba5551(0x0801 + 15))
    );
}

/// RE-313: an index past a short TLUT reads the TMEM slot's measured stale
/// value (`STALE_TLUT_ENTRIES`) in both the decoded reference and the
/// packed palette. The level-0 index field then matches the reference.
#[test]
fn short_tlut_indices_use_the_measured_stale_entry() {
    use ssb_rom::texture::{BitSize, Format, TextureLut};
    // File 120's Break the Targets arrow: 3 entries loaded from 0x598.
    let mut data = vec![0u8; 0x5A8 + 8];
    for (i, e) in [0x294Au16, 0xA50D, 0x0001, 0x7BC1].into_iter().enumerate() {
        data[0x598 + i * 2..0x59A + i * 2].copy_from_slice(&e.to_be_bytes());
    }
    data[0x5A8..0x5A8 + 8].copy_from_slice(&[0x01, 0x23, 0x32, 0x10, 0x01, 0x23, 0x32, 0x10]);
    let t = ssb_rom::mesh::TextureRef {
        palette_offset: Some(0x598),
        palette_entries: 3,
        tlut: TextureLut::Rgba16,
        ..synthetic_texture(Format::Ci, BitSize::Bits4, 4, 4, 0x5A8)
    };
    let (packed, _, _) = convert_recorded(120, data.clone(), &t);
    assert_eq!(packed.palette.len(), 4);
    assert_eq!(
        packed.palette[3],
        ssb_rom::psp_texture::pack_abgr(ssb_rom::texture::rgba5551(0x1091)),
        "slot 3 is the measured stale TMEM entry, not the unloaded 0x7BC1"
    );
    // The same bytes under any other palette address have no measured
    // value: the slot stays transparent black, matching the decoder.
    let (unknown, _, _) = convert_recorded(7, data, &t);
    assert_eq!(unknown.palette[3], 0);
}

/// Converting the same input twice packs identical bytes.
#[test]
fn texture_conversion_is_deterministic() {
    use ssb_rom::texture::{BitSize, Format};
    let data: Vec<u8> = ridge(8, 8)
        .into_iter()
        .map(|l| u16::from(l) << 11 | u16::from(l) << 6 | u16::from(l) << 1 | 1)
        .flat_map(u16::to_be_bytes)
        .collect();
    let data = [vec![0; 8], data].concat();
    let t = synthetic_texture(Format::Rgba, BitSize::Bits16, 8, 8, 8);
    let (a, _, _) = convert_recorded(1, data.clone(), &t);
    let (b, _, _) = convert_recorded(1, data, &t);
    assert_eq!(a.data, b.data);
    assert_eq!(a.palette, b.palette);
    assert_eq!(a.format, b.format);
}

#[test]
fn padded_source_rows_do_not_apply_the_tile_origin_twice() {
    use ssb_rom::mesh::TextureRef;
    use ssb_rom::texture::{BitSize, Format};

    // Two 4-texel RGBA16 source rows, rendered through a 2-wide tile.
    // The nonzero origin is already represented by the packed UVs; it
    // must not also skip the first two pixels in this source buffer.
    let source = [
        0xF801u16, 0x07C1, 0x003F, 0xFFFF, // red, green, blue, white
        0xFFFF, 0x003F, 0x07C1, 0xF801, // white, blue, green, red
    ]
    .into_iter()
    .flat_map(u16::to_be_bytes)
    .collect::<Vec<_>>();
    let texture = TextureRef {
        data_file: None,
        data_offset: 0,
        format: Format::Rgba,
        size: BitSize::Bits16,
        width: 2,
        height: 2,
        source_width: 4,
        palette_file: None,
        palette_offset: None,
        palette_entries: 0,
        palette: 0,
        mirror_s: false,
        mirror_t: false,
        clamp_s: true,
        clamp_t: true,
        framebuffer: false,
        rgb_unused: false,
        rgb_lerp: None,
        origin_s: 2 << 2,
        origin_t: 0,
        mask_s: 0,
        mask_t: 0,
        drawn_width: 2,
        drawn_height: 2,
        tlut: ssb_rom::texture::TextureLut::None,
    };

    let image = crate::decode_texture(&source, &texture, &[]).expect("texture decodes");
    assert_eq!(
        image.pixels,
        vec![
            255, 0, 0, 255, 0, 255, 0, 255, // source row 0, columns 0..2
            255, 255, 255, 255, 0, 0, 255, 255, // source row 1, columns 0..2
        ],
        "the visible tile starts at the source buffer's top-left"
    );
}
