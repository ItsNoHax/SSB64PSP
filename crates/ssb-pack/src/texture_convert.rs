//! Texture conversion: N64 texels to the PSP formats, filter compensation.

use crate::*;

/// Extracts the 16x16 I4 frame `ftShadowProcDisplay` loads from
/// `dEFCommonEffects2_Shadow_TextureImage` (file 84, `0x3A68`).  The trailing
/// 5,152 bytes are other frames in the shared effect pool; the fighter path
/// names the base address and `gDPLoadTextureBlock_4b` reads only 128 bytes.
pub fn fighter_shadow_texture(
    file: &[u8],
    swizzle: bool,
) -> Result<ssb_rom::psp_texture::PspTexture, PackError> {
    use ssb_rom::psp_texture::{self, Psm};
    use ssb_rom::texture::{self, Rgba8};

    const OFFSET: usize = 0x3A68;
    const WIDTH: u32 = 16;
    const HEIGHT: u32 = 16;
    let bytes = file
        .get(OFFSET..OFFSET + (WIDTH * HEIGHT / 2) as usize)
        .ok_or("fighter shadow image is truncated")?;
    let mut image = Rgba8::new(WIDTH, HEIGHT);
    for (byte_index, &byte) in bytes.iter().enumerate() {
        for (nibble_index, nibble) in [byte >> 4, byte & 0x0F].into_iter().enumerate() {
            let intensity = nibble * 17;
            image.put(
                byte_index * 2 + nibble_index,
                [intensity, intensity, intensity, intensity],
            );
        }
    }
    let mirrored = texture::mirror_extend(&image, true, true, false, false, WIDTH, HEIGHT);
    Ok(psp_texture::pack_rgba(&mirrored, Psm::Psm8888, swizzle))
}

pub fn convert_particle_frame(
    texture: &ssb_rom::particle::Texture<'_>,
    frame: usize,
    swizzle: bool,
) -> Result<ssb_rom::psp_texture::PspTexture, String> {
    use ssb_rom::psp_texture as psp;
    use ssb_rom::texture::{self, Format};

    let image = texture
        .images
        .get(frame)
        .ok_or_else(|| "frame out of range".to_string())?;
    let palette_index = if texture.flags & 1 != 0 { 0 } else { frame };
    let palette: Vec<u16> = texture
        .palettes
        .get(palette_index)
        .map(|bytes| {
            bytes
                .as_chunks::<2>()
                .0
                .iter()
                .map(|word| u16::from_be_bytes(*word))
                .collect()
        })
        .unwrap_or_default();

    if texture.format == Format::Ci {
        return psp::pack_paletted(
            image,
            texture.width,
            texture.height,
            texture.size,
            &palette,
            swizzle,
        )
        .map_err(|error| format!("{error:?}"));
    }
    if texture.format == Format::I {
        return psp::pack_indexed(
            image,
            texture.width,
            texture.height,
            texture.size,
            &psp::intensity_palette(texture.size),
            swizzle,
        )
        .map_err(|error| format!("{error:?}"));
    }
    let rgba = texture::decode(
        image,
        texture.width,
        texture.height,
        texture.format,
        texture.size,
        None,
    )
    .map_err(|error| format!("{error:?}"))?;
    Ok(psp::pack_rgba(
        &rgba,
        psp::choose_psm(texture.format, texture.size),
        swizzle,
    ))
}

/// Resolves `G_SETTILE.palette`'s raw bank index (RE-223/`R2.0`/P2) into a
/// byte offset/entry-count to read a loaded TLUT from: bank `n` starts
/// `n * 16` entries into the chunk `G_LOADTLUT` loaded. A bank beyond the
/// loaded chunk should not occur on real content (RE-223's own archive-wide
/// measurement), but is guarded here rather than panicking or silently
/// returning an empty palette: falls back to bank 0 (`palette_offset`/
/// `palette_entries` unshifted). A `palette` of 0 (the overwhelming common
/// case) is always a no-op by construction.
pub fn palette_bank_offset(palette_offset: u32, palette_entries: u16, palette: u8) -> (u32, usize) {
    let bank = palette as usize * 16;
    let entries = palette_entries.max(1) as usize;
    if bank < entries {
        (palette_offset + (bank * 2) as u32, entries - bank)
    } else {
        (palette_offset, entries)
    }
}

/// Decodes and packs one texture referenced by a primitive.
///
/// The texels and the palette are looked up independently, because they need
/// not be in the same file: a fighter's palette comes from its own file while
/// a stage's texels come from a shared one.
#[allow(clippy::too_many_arguments)]
pub fn convert_texture(
    ctx: &PackCtx<'_>,
    src: Texels<'_>,
    t: &ssb_rom::mesh::TextureRef,
    coverage: &filter_coverage::Coverage,
    swizzle: bool,
    animated_palettes: Option<&[Vec<u32>]>,
    alpha_policy: ssb_rom::filter_compensation::AlphaPolicy,
    mut compensated_out: Option<&mut bool>,
    direct_fit: Option<crate::solver::CrossFit>,
) -> Option<ssb_rom::psp_texture::PspTexture> {
    use ssb_rom::psp_texture as psp;
    use ssb_rom::texture;

    let file = src.bytes(t.data_file)?;
    if (t.data_offset >> 24) != 0 || (t.data_offset == 0 && t.data_file.is_none()) {
        return None; // segmented, or a pointer nothing resolved
    }
    // RE-283: file 324's boot texture (Link's, offset 0xCF18, 16x16 CI4)
    // reproduces a genuine PSP GE rendering defect (a purple/black patch) on
    // both PPSSPP and physical hardware, given a proven byte-correct
    // texture/CLUT bind -- the same class of defect RE-267 already found and
    // fixed for Donkey Kong's file 317 by bypassing PSP's paletted-texture
    // transport entirely. Widening the row (RE-283's own stride experiment,
    // rejected: the defect persisted byte-identical even once this texture
    // was widened enough to actually swizzle) ruled out stride/swizzle as the
    // mechanism, matching RE-267's own "swizzling... did not explain" finding.
    let is_link_boot = src.home.id == 324 && t.data_file.is_none() && t.data_offset == 0xCF18;
    // RE-283 (fifteenth follow-up): file 324's rear-leg shin-cuff texture
    // (Link's, offset 0xB4E0, 16x32 CI4, same 8-byte-row shape as the boot
    // texture above) node-isolated to node 23 only -- its mirrored twin
    // (node 28, same texture, `G_SETTILE.cms` mirror instead of wrap)
    // renders clean. Repeat-wrapped sampling of this exact packed shape
    // reproduces the same purple/black speckle class as the boot defect;
    // the same paletted-transport bypass that fixed it applies here too.
    let is_link_shin = src.home.id == 324 && t.data_file.is_none() && t.data_offset == 0xB4E0;
    // RE-283 (Yoshi follow-up): file 338's head node (node 3) binds two small
    // CI4 textures under `Clamp`/`Clamp` addressing (no mirror) -- the mouth-
    // corner mark (offset 0x9518, 8x8) and both eyes (offset 0x9BD0, 16x16,
    // bound twice). Node-isolating (`RE283_ONLY_LOCAL_NODE=3`) reproduced the
    // user's "heavy scrambled multicolour noise" and "smaller patch near the
    // nostril" exactly; disabling each texture in turn localized the ridge
    // noise to the eye texture and the mouth-corner mark to this one, and
    // disabling both together left the head perfectly clean. Both are the
    // same small-paletted packed shape (8-byte-row-or-narrower `PsmT4`) as
    // Link's boot/shin-cuff defect, this time reached through `Clamp`
    // addressing rather than `Wrap` -- the same GE quirk is not gated on
    // wrap mode specifically, just this texture-size class. Same fix.
    let is_yoshi_head = src.home.id == 338
        && t.data_file.is_none()
        && (t.data_offset == 0x9518 || t.data_offset == 0x9BD0);
    // RE-283 (Captain Falcon follow-up): file 332's shoulder-pad node (node
    // 10, graph 0x3BE0) binds an 8x8 CI4 dither texture at offset 0xC508 --
    // a prior session misidentified this node's content via a desynced raw
    // `dlraw` command dump (offsets 0xC128/0xBBB0, neither real) and ruled
    // out this same bypass class against those wrong textures. Node-isolating
    // the *correct* offset (recovered from the authoritative pack()/
    // plan_draw_order pipeline) reproduces the user's "heavily scrambled
    // yellow/orange/black patch" exactly, and this bypass alone clears it
    // completely -- same small-paletted-texture (8-byte-row-or-narrower
    // PsmT4) GE quirk as Link's boot/shin-cuff and Yoshi's head.
    let is_falcon_shoulder = src.home.id == 332 && t.data_file.is_none() && t.data_offset == 0xC508;
    // RE-283 (Ness follow-up): file 335's node 4 (right shoulder/sleeve, graph
    // 0x26B0) binds an 8x8 CI4 texture at offset 0xBDE8, mirror-narrowed to
    // 8x16 -- the same small-paletted packed shape (8-byte-row-or-narrower
    // `PsmT4`) as Link's boot/shin-cuff, Yoshi's head and Falcon's shoulder.
    // A raw `dlraw` decode of this node's list first desynced onto the wrong
    // offsets entirely (0xAD00/0xAE30/0xAB20, all clean 16x32 shirt-stripe
    // textures unaffected by bypassing them); the authoritative
    // `pack()`/`plan_draw_order` pipeline (`romtool scene --dump-node 4`)
    // recovered the real bound texture. Node-isolating (`RE283_ONLY_LOCAL_NODE=4`)
    // reproduced the user's "right shoulder strap area shows scrambled pixel
    // noise" exactly; this bypass alone clears it completely.
    let is_ness_shoulder = src.home.id == 335 && t.data_file.is_none() && t.data_offset == 0xBDE8;
    // The natural PSP format. A 4- or 8-bit texel drawn with the TLUT on is a
    // palette index whatever its N64 format says (RE-313), so it stays
    // indexed; everything else follows `choose_psm`.
    let natural = match (t.tlut.enabled(), t.size) {
        (true, texture::BitSize::Bits4) => psp::Psm::PsmT4,
        (true, texture::BitSize::Bits8) => psp::Psm::PsmT8,
        _ => psp::choose_psm(t.format, t.size),
    };
    // The RE-283 bypass replaces only the paletted *transport*. The texels
    // still enter the same compensation pipeline below as RGBA8888.
    let psm = if (src.home.id == 317
        || is_link_boot
        || is_link_shin
        || is_yoshi_head
        || is_falcon_shoulder
        || is_ness_shoulder)
        && natural.is_paletted()
    {
        psp::Psm::Psm8888
    } else {
        natural
    };

    // A CI4 bank is a 16-entry window of the loaded TLUT; an 8-bit index
    // ignores the bank (`fetch_texel_entlut_quadro`), and RE-223 found a
    // nonzero bank only on CI4.
    let bank = if t.size == texture::BitSize::Bits4 {
        t.palette
    } else {
        0
    };
    let mut tlut: Vec<u16> = match t.palette_offset {
        Some(off) => {
            let (off, n) = palette_bank_offset(off, t.palette_entries, bank);
            src.bytes(t.palette_file)
                .and_then(|f| {
                    f.get(off as usize..off as usize + n * 2)
                        .map(texture::parse_tlut)
                })
                .unwrap_or_default()
        }
        None => Vec::new(),
    };
    if t.tlut.enabled() && tlut.is_empty() {
        // No TLUT the converter can resolve: nothing defined to sample.
        return None;
    }
    if t.tlut.enabled() {
        extend_short_tlut(ctx, src, &file, t, &mut tlut);
    }
    // The colour each index value yields, for an indexed natural format.
    let index_colours: Vec<u32> = if t.tlut.enabled() {
        tlut.iter()
            .map(|&e| psp::pack_abgr(t.tlut.entry_rgba(e)))
            .collect()
    } else if natural.is_paletted() && t.format == texture::Format::Ci {
        // CI with the TLUT off reads the raw index as intensity (RE-313).
        let n = if t.size == texture::BitSize::Bits4 {
            16
        } else {
            256
        };
        (0..n)
            .map(|i| {
                let v = if n == 16 {
                    (t.palette << 4) | i as u8
                } else {
                    i as u8
                };
                psp::pack_abgr([v, v, v, v])
            })
            .collect()
    } else if natural.is_paletted() && t.format == texture::Format::I {
        // An intensity texture carries no palette because on the N64 it needs
        // none. Generating the ramp keeps it at 4 or 8 bits instead of letting
        // it fall through to the 8888 path `choose_psm` picked `PsmT4` to
        // avoid (RE-047).
        psp::intensity_palette(t.size)
    } else {
        Vec::new()
    };
    // `TextureRef::rgb_unused`: only the texel's alpha reaches the pixel, so
    // its colour is white and the GE's `Modulate` leaves the shade as it is.
    let index_colours: Vec<u32> = if t.rgb_unused {
        index_colours.into_iter().map(|c| c | 0x00FF_FFFF).collect()
    } else {
        index_colours
    };
    // `TextureRef::rgb_lerp`: the combiner blends two constants by the
    // texel before the shade multiplies it in.
    let index_colours: Vec<u32> = match t.rgb_lerp {
        Some(lerp) => index_colours
            .into_iter()
            .map(|c| psp::pack_abgr(ssb_rom::mesh::rgb_lerp_texel(lerp, c.to_le_bytes())))
            .collect(),
        None => index_colours,
    };
    if natural.is_paletted() && index_colours.is_empty() {
        return None;
    }

    // Mipmapped, because the N64's textures are frequently dithered and a
    // dithered gradient sampled near one texel per pixel aliases into moire
    // instead of reading as shading (RE-053). Every level is generated from the
    // decoded image; for a paletted texture level 0 comes back bit-identical,
    // since each texel's nearest palette entry is the one it was decoded from.
    // `t.width`/`t.height` are already narrowed to one repeat period
    // (`mesh.rs`'s `current_texture()`, RE-044), so a mirrored axis here
    // pre-bakes exactly the real hardware's mirror-repeat into the pixel
    // data -- a plain `Repeat` wrap over the resulting wider/taller image
    // reproduces it exactly, since `sceGuTexScale` renormalises UVs against
    // whatever dimensions the packed texture actually reports (RE-067).
    let decode_mirrored = |tlut: &[u16]| {
        let mut img = decode_texture(&file, t, tlut)?;
        if t.rgb_unused {
            for px in img.pixels.as_chunks_mut::<4>().0 {
                px[..3].fill(0xFF);
            }
        }
        if let Some(lerp) = t.rgb_lerp {
            for px in img.pixels.as_chunks_mut::<4>().0 {
                *px = ssb_rom::mesh::rgb_lerp_texel(lerp, *px);
            }
        }
        Some(texture::mirror_extend(
            &img,
            t.mirror_s,
            t.mirror_t,
            t.clamp_s,
            t.clamp_t,
            t.drawn_width as u32,
            t.drawn_height as u32,
        ))
    };

    // RE-588: only listed textures are compensated; every other one packs
    // as a rejected compensation leaves it, without the work. The residuals
    // audit takes the full path, whose records it reads.
    let plain = direct_fit.is_none()
        && !ctx.observer.enabled()
        && !listed(ctx, src.home.id, t.data_file, t.data_offset);
    let mut packed = |palette: Vec<u32>| {
        let img = decode_mirrored(&tlut)?;
        if plain {
            if psm.is_paletted() && t.format == texture::Format::Ci {
                if let Some(states) = animated_palettes.filter(|states| !states.is_empty()) {
                    let source = decode_index_field(&file, t)?;
                    let limit = if psm == psp::Psm::PsmT4 {
                        16
                    } else {
                        palette.len()
                    };
                    // The source indices, under every palette state when
                    // each holds the indices' range.
                    let states = if states.iter().all(|p| p.len() >= limit) {
                        states
                    } else {
                        core::slice::from_ref(&palette)
                    };
                    return Some(psp::pack_level0_indices(
                        &img, psm, &palette, swizzle, &source, states,
                    ));
                }
                // A static CLUT: still the ROM's own indices. Re-quantizing
                // from colour maps an entry to the first equal one (files
                // 111/112: ROM index 2 packed as 1), which `matcolors` reads
                // as a texel mismatch (RE-364).
                if let Some(source) = decode_index_field(&file, t) {
                    if source.iter().all(|&i| usize::from(i) < palette.len()) {
                        return Some(psp::pack_level0_indices(
                            &img,
                            psm,
                            &palette,
                            swizzle,
                            &source,
                            core::slice::from_ref(&palette),
                        ));
                    }
                }
            }
            return Some(psp::pack_level0(&img, psm, &palette, swizzle));
        }
        // RE-311: a bounded texgen primitive trains on its real-normal pose
        // coverage plus a one-point-per-texel regularization over the whole
        // reachable box, and must pass two holdouts: the disjoint-pose
        // real-normal set and the conservative old-density lattice over that
        // box (every point of which some pose can generate).
        let texgen_meta = coverage.texgen.as_ref();
        let active_texgen = texgen_meta.filter(|meta| !meta.legacy);
        let merged_train: Vec<[i32; 2]>;
        let train_samples: &[[i32; 2]] = match active_texgen {
            Some(meta) => {
                let mut merged = coverage.train.clone();
                merged.extend(meta.bound.regularization());
                merged.sort_unstable();
                merged.dedup();
                merged_train = merged;
                &merged_train
            }
            None => &coverage.train,
        };
        let conservative: Vec<[i32; 2]> = texgen_meta
            .map(|meta| meta.bound.lattice(4))
            .unwrap_or_default();
        let conservative_gate = |candidate: &ssb_rom::texture::Rgba8| {
            active_texgen.is_none() || {
                let before = ssb_rom::filter_compensation::measure_samples_policy(
                    &img,
                    &img,
                    t.clamp_s,
                    t.clamp_t,
                    &conservative,
                    alpha_policy,
                );
                let after = ssb_rom::filter_compensation::measure_samples_policy(
                    &img,
                    candidate,
                    t.clamp_s,
                    t.clamp_t,
                    &conservative,
                    alpha_policy,
                );
                validation_preserved(before, after, alpha_policy)
            }
        };
        let texgen_report = |candidate: &ssb_rom::texture::Rgba8,
                             accepted: bool,
                             outcome: &str,
                             solve_sse: Option<(u64, u64)>| {
            let Some(meta) = texgen_meta else { return };
            let full_tile: Vec<[i32; 2]> = (4..img.height as i32 * 32)
                .step_by(ssb_rom::filter_compensation::SAMPLE_STEP_Q5 as usize)
                .flat_map(|v| {
                    (4..img.width as i32 * 32)
                        .step_by(ssb_rom::filter_compensation::SAMPLE_STEP_Q5 as usize)
                        .map(move |u| [u, v])
                })
                .collect();
            let mut fields = String::new();
            for (name, set) in [
                ("real", meta.holdout.as_slice()),
                ("box", conservative.as_slice()),
                ("tile", full_tile.as_slice()),
            ] {
                let a = ssb_rom::filter_compensation::measure_samples_policy(
                    &img,
                    &img,
                    t.clamp_s,
                    t.clamp_t,
                    set,
                    alpha_policy,
                );
                let b = ssb_rom::filter_compensation::measure_samples_policy(
                    &img,
                    candidate,
                    t.clamp_s,
                    t.clamp_t,
                    set,
                    alpha_policy,
                );
                fields.push_str(&format!(
                    " {name}_samples={} {name}_source_sse={} {name}_final_sse={} {name}_source_above32={} {name}_final_above32={} {name}_source_max={} {name}_final_max={}",
                    a.samples, a.squared_error, b.squared_error, a.above_32, b.above_32, a.max, b.max
                ));
            }
            pack_diag!(ctx,
                "texgen-filter-comp file={} offset={:#X} {}x{} mode={} linear={} box={:?}..{:?} train={} regularization={} accepted={} outcome={} solve_baseline_sse={} solve_compensated_sse={}{}",
                t.data_file.map_or(src.home.id, u32::from),
                t.data_offset,
                img.width,
                img.height,
                if meta.legacy { "full-tile" } else { "real" },
                meta.linear,
                meta.bound.min,
                meta.bound.max,
                coverage.train.len(),
                if meta.legacy { 0 } else { meta.bound.regularization().len() },
                accepted,
                outcome,
                solve_sse.map_or(-1, |s| s.0 as i64),
                solve_sse.map_or(-1, |s| s.1 as i64),
                fields
            );
        };
        // A shifted full-tile grid is the holdout for unbounded texgen (the
        // fallback). Authored UVs use the separate cell-local holdout built
        // from their actual triangles.
        let validation_dense: Vec<[i32; 2]> = if train_samples.is_empty() {
            (4..img.height as i32 * 32)
                .step_by(ssb_rom::filter_compensation::SAMPLE_STEP_Q5 as usize)
                .flat_map(|v| {
                    (4..img.width as i32 * 32)
                        .step_by(ssb_rom::filter_compensation::SAMPLE_STEP_Q5 as usize)
                        .map(move |u| [u, v])
                })
                .collect()
        } else {
            Vec::new()
        };
        let validation_samples = if !coverage.validation.is_empty() {
            coverage.validation.as_slice()
        } else if !validation_dense.is_empty() {
            validation_dense.as_slice()
        } else {
            &[]
        };
        // Report-only (`romtool residuals`): clones what this conversion
        // packs; it never changes the result.
        let record = |final_img: &ssb_rom::texture::Rgba8,
                      final_psm: psp::Psm,
                      method: &'static str,
                      attempted: bool,
                      animated: Option<crate::Animated>| {
            if ctx.observer.enabled() {
                ctx.observer.record_conversion(crate::Conversion {
                    texture: *t,
                    home: src.home.id,
                    source: img.clone(),
                    final_level0: final_img.clone(),
                    source_psm: natural,
                    final_psm,
                    palette: index_colours.clone(),
                    animated,
                    method,
                    attempted,
                    policy: alpha_policy,
                });
            }
        };
        // RE-312 exact-use-site override (`residual_fixes::Kind::Direct`):
        // the section 11 cross-phase fit on this site's own phase-uniform
        // set, shipped as an immutable RGBA8888 variant. Only listed
        // authored-UV, single-palette sites reach here.
        if let Some(mode) = direct_fit {
            assert!(
                animated_palettes.is_none() && coverage.texgen.is_none(),
                "direct override needs a static authored-UV site"
            );
            assert!(
                !coverage.validation.is_empty(),
                "direct override needs a holdout"
            );
            let mut eval = coverage.validation.clone();
            eval.sort_unstable_by_key(|p| (p[1], p[0]));
            eval.dedup();
            let u1 = crate::solver::phase_uniform_set(&eval);
            let mut fitted = crate::solver::cross_phase_fit(
                &img,
                [t.clamp_s, t.clamp_t],
                alpha_policy,
                &u1,
                mode,
            );
            if ctx.opts.residual_hide_probe {
                assert!(matches!(
                    alpha_policy,
                    ssb_rom::filter_compensation::AlphaPolicy::Cutout { .. }
                ));
                fitted
                    .pixels
                    .as_chunks_mut::<4>()
                    .0
                    .iter_mut()
                    .for_each(|p| p[3] = 0);
            }
            record(
                &fitted,
                psp::Psm::Psm8888,
                "re312-direct-override",
                true,
                None,
            );
            if let Some(out) = compensated_out.as_deref_mut() {
                *out = true;
            }
            return Some(psp::pack_level0(&fitted, psp::Psm::Psm8888, &[], swizzle));
        }
        if psm.is_paletted() && t.format == texture::Format::Ci {
            if let Some(states) = animated_palettes.filter(|states| !states.is_empty()) {
                let source = decode_index_field(&file, t)?;
                let limit = if psm == psp::Psm::PsmT4 {
                    16
                } else {
                    palette.len()
                };
                if states.iter().all(|p| p.len() >= limit) {
                    let dense: Vec<[i32; 2]>;
                    let samples = if train_samples.is_empty() {
                        dense = (0..img.height as i32 * 32)
                            .step_by(ssb_rom::filter_compensation::SAMPLE_STEP_Q5 as usize)
                            .flat_map(|v| {
                                (0..img.width as i32 * 32)
                                    .step_by(ssb_rom::filter_compensation::SAMPLE_STEP_Q5 as usize)
                                    .map(move |u| [u, v])
                            })
                            .collect();
                        dense.as_slice()
                    } else {
                        train_samples
                    };
                    let optimized = ssb_rom::filter_compensation::optimize_animated_indices(
                        &source, img.width, img.height, states, limit, t.clamp_s, t.clamp_t,
                        samples,
                    );
                    let mut before = 0u64;
                    let mut after = 0u64;
                    let mut better_large = false;
                    let mut nonregressing = true;
                    let mut baseline_large = 0u64;
                    let mut baseline_samples = 0u64;
                    let mut baseline_max = 0u8;
                    let mut validation_before = 0u64;
                    let mut validation_after = 0u64;
                    let mut validation_before_32 = 0u64;
                    let mut validation_after_32 = 0u64;
                    let mut validation_before_max = 0u8;
                    let mut validation_after_max = 0u8;
                    for state in states {
                        let original = ssb_rom::filter_compensation::indices_to_rgba(
                            img.width, img.height, &source, state,
                        );
                        let candidate = ssb_rom::filter_compensation::indices_to_rgba(
                            img.width, img.height, &optimized, state,
                        );
                        let a = ssb_rom::filter_compensation::measure_samples(
                            &original, &original, t.clamp_s, t.clamp_t, samples,
                        );
                        let b = ssb_rom::filter_compensation::measure_samples(
                            &original, &candidate, t.clamp_s, t.clamp_t, samples,
                        );
                        before += a.squared_error;
                        after += b.squared_error;
                        baseline_large += a.above_8;
                        baseline_samples += a.samples;
                        baseline_max = baseline_max.max(a.max);
                        better_large |= b.above_8 < a.above_8;
                        nonregressing &=
                            b.max <= a.max.saturating_add(8) && b.above_32 <= a.above_32;
                        let va = ssb_rom::filter_compensation::measure_samples_policy(
                            &original,
                            &original,
                            t.clamp_s,
                            t.clamp_t,
                            validation_samples,
                            alpha_policy,
                        );
                        let vb = ssb_rom::filter_compensation::measure_samples_policy(
                            &original,
                            &candidate,
                            t.clamp_s,
                            t.clamp_t,
                            validation_samples,
                            alpha_policy,
                        );
                        validation_before += va.squared_error;
                        validation_after += vb.squared_error;
                        validation_before_32 += va.above_32;
                        validation_after_32 += vb.above_32;
                        validation_before_max = validation_before_max.max(va.max);
                        validation_after_max = validation_after_max.max(vb.max);
                        nonregressing &= validation_preserved(va, vb, alpha_policy);
                    }
                    let accepted = !validation_samples.is_empty()
                        && before > 0
                        && after * 100 <= before * 95
                        && better_large
                        && nonregressing;
                    let high_error = baseline_samples > 0
                        && baseline_large * 100 >= baseline_samples
                        && baseline_max >= 16;
                    let direct_sse = if !accepted && high_error {
                        let mut sum = 0u64;
                        for state in states {
                            let original = ssb_rom::filter_compensation::indices_to_rgba(
                                img.width, img.height, &source, state,
                            );
                            let baseline = ssb_rom::filter_compensation::measure_samples(
                                &original, &original, t.clamp_s, t.clamp_t, samples,
                            )
                            .squared_error;
                            sum += ssb_rom::filter_compensation::solve_samples(
                                &original,
                                t.clamp_s,
                                t.clamp_t,
                                samples,
                                alpha_policy,
                            )
                            .map_or(baseline, |s| s.compensated.squared_error.min(baseline));
                        }
                        Some(sum)
                    } else {
                        None
                    };
                    let pixels = img.width as i64 * img.height as i64;
                    let direct_extra_level0 = pixels * 4 * states.len() as i64
                        - (pixels * psm.bits() as i64).div_euclid(8);
                    pack_diag!(ctx, "animated-filter-comp file={} offset={:#X} palettes={} baseline_sse={} indexed_sse={} accepted={} high_error={} direct_sse={:?} direct_extra_level0={} format={:?} memory_delta=0 train_samples={} validation_samples={} validation_baseline_sse={} validation_candidate_sse={} validation_baseline_above32={} validation_candidate_above32={} validation_baseline_max={} validation_candidate_max={}", t.data_file.map_or(src.home.id,u32::from), t.data_offset, states.len(), before, after, accepted, high_error, direct_sse, direct_extra_level0, psm, samples.len(), validation_samples.len(), validation_before, validation_after, validation_before_32, validation_after_32, validation_before_max, validation_after_max);
                    let indices = if accepted { &optimized } else { &source };
                    if ctx.observer.enabled() {
                        record(
                            &ssb_rom::filter_compensation::indices_to_rgba(
                                img.width, img.height, indices, &palette,
                            ),
                            psm,
                            if accepted {
                                "animated-joint-index"
                            } else {
                                "animated-rejected"
                            },
                            true,
                            Some(crate::Animated {
                                source_indices: source.clone(),
                                final_indices: indices.clone(),
                                states: states.to_vec(),
                            }),
                        );
                    }
                    if accepted {
                        if let Some(out) = compensated_out.as_deref_mut() {
                            *out = true;
                        }
                    }
                    return Some(psp::pack_level0_indices(
                        &img, psm, &palette, swizzle, indices, states,
                    ));
                } else {
                    // An incomplete table cannot define a safe joint legal
                    // index set. Retain the source indices exactly instead
                    // of falling through to frame-zero RGB quantization.
                    record(
                        &img,
                        psm,
                        "animated-incomplete-table",
                        false,
                        Some(crate::Animated {
                            source_indices: source.clone(),
                            final_indices: source.clone(),
                            states: vec![palette.clone()],
                        }),
                    );
                    return Some(psp::pack_level0_indices(
                        &img,
                        psm,
                        &palette,
                        swizzle,
                        &source,
                        core::slice::from_ref(&palette),
                    ));
                }
            }
        }
        // RE-588: only the listed textures run the solve (PSP build budget).
        let solved = if !listed(ctx, src.home.id, t.data_file, t.data_offset) {
            None
        } else if train_samples.is_empty() {
            ssb_rom::filter_compensation::solve(&img, t.clamp_s, t.clamp_t, alpha_policy)
        } else {
            ssb_rom::filter_compensation::solve_samples(
                &img,
                t.clamp_s,
                t.clamp_t,
                train_samples,
                alpha_policy,
            )
        };
        let Some(solved) = solved else {
            texgen_report(&img, false, "no-solve", None);
            record(&img, psm, "no-solve", true, None);
            return Some(psp::pack_level0(&img, psm, &palette, swizzle));
        };
        // Requirement 4: quantizing to the nearest existing palette/5551
        // entry can move a texel's alpha byte again (to whatever the
        // nearest immutable entry happens to carry) even though
        // `solve_samples` already proved the pre-quantization candidate
        // silhouette-safe -- so a cutout texture needs this checked again
        // against the actual final representation, not just the float solve.
        let cutout_gate = match alpha_policy {
            ssb_rom::filter_compensation::AlphaPolicy::Cutout {
                greater_or_equal,
                threshold,
            } => Some((greater_or_equal, threshold)),
            _ => None,
        };
        // Texgen falls back to dense full-tile coverage the same way
        // `solve`/`measure` do internally when given no explicit coverage.
        let dense_coverage: Vec<[i32; 2]>;
        let mismatch_coverage: &[[i32; 2]] = if train_samples.is_empty() {
            dense_coverage = (0..img.height as i32 * 32)
                .step_by(ssb_rom::filter_compensation::SAMPLE_STEP_Q5 as usize)
                .flat_map(|tq| {
                    (0..img.width as i32 * 32)
                        .step_by(ssb_rom::filter_compensation::SAMPLE_STEP_Q5 as usize)
                        .map(move |sq| [sq, tq])
                })
                .collect();
            &dense_coverage
        } else {
            train_samples
        };
        let silhouette_preserved = |candidate: &ssb_rom::texture::Rgba8| {
            cutout_gate.is_none_or(|(greater_or_equal, threshold)| {
                ssb_rom::filter_compensation::count_cutout_mismatches(
                    &img,
                    candidate,
                    t.clamp_s,
                    t.clamp_t,
                    mismatch_coverage,
                    greater_or_equal,
                    threshold,
                ) == 0
            })
        };
        let mut direct_refined = (psm == psp::Psm::Psm8888).then(|| {
            ssb_rom::filter_compensation::refine_integer(
                &img,
                &solved.rgba,
                ssb_rom::filter_compensation::IntegerFormat::Rgba8888,
                t.clamp_s,
                t.clamp_t,
                mismatch_coverage,
                alpha_policy,
            )
        });
        let mut chosen = direct_refined.as_ref().unwrap_or(&solved.rgba);
        let mut format = psm;
        let quantized;
        let mut integer_before_metrics = None;
        let mut integer_after_metrics = None;
        let mut used_index_optimization = false;
        let mut nearest_sse_dbg: i64 = -1;
        let mut optimized_sse_dbg: i64 = -1;
        let mut nearest_max_dbg: i64 = -1;
        let mut optimized_max_dbg: i64 = -1;
        let mut nearest_above8_dbg: i64 = -1;
        let mut optimized_above8_dbg: i64 = -1;
        let mut nearest_above32_dbg: i64 = -1;
        let mut optimized_above32_dbg: i64 = -1;
        if psm.is_paletted() {
            // Nearest-palette-color quantization minimizes per-texel color
            // distance, not the actual filtered output error: neighbouring
            // texels participate in the GE's bilinear reconstruction, so the
            // nearest entry in isolation is not necessarily the index that
            // minimizes what a sample actually reads. `optimize_palette_indices`
            // coordinate-descends over the exact measured objective instead,
            // seeded from this same nearest assignment; it is only kept if it
            // measures no worse on every metric (requirement 10 -- never trade
            // away a metric the plain nearest quantization already had).
            let nearest = ssb_rom::filter_compensation::quantize_to_palette(&solved.rgba, &palette);
            let nearest_metrics = ssb_rom::filter_compensation::measure_samples(
                &img,
                &nearest,
                t.clamp_s,
                t.clamp_t,
                mismatch_coverage,
            );
            let max_index = if psm == psp::Psm::PsmT4 {
                16
            } else {
                palette.len()
            };
            let optimized = ssb_rom::filter_compensation::optimize_palette_indices(
                &img,
                &solved.rgba,
                &palette,
                max_index,
                t.clamp_s,
                t.clamp_t,
                mismatch_coverage,
            );
            let optimized_metrics = ssb_rom::filter_compensation::measure_samples(
                &img,
                &optimized,
                t.clamp_s,
                t.clamp_t,
                mismatch_coverage,
            );
            let nearest_validation = ssb_rom::filter_compensation::measure_samples_policy(
                &img,
                &nearest,
                t.clamp_s,
                t.clamp_t,
                validation_samples,
                alpha_policy,
            );
            let optimized_validation = ssb_rom::filter_compensation::measure_samples_policy(
                &img,
                &optimized,
                t.clamp_s,
                t.clamp_t,
                validation_samples,
                alpha_policy,
            );
            nearest_sse_dbg = nearest_metrics.squared_error as i64;
            optimized_sse_dbg = optimized_metrics.squared_error as i64;
            nearest_max_dbg = nearest_metrics.max as i64;
            optimized_max_dbg = optimized_metrics.max as i64;
            nearest_above8_dbg = nearest_metrics.above_8 as i64;
            optimized_above8_dbg = optimized_metrics.above_8 as i64;
            nearest_above32_dbg = nearest_metrics.above_32 as i64;
            optimized_above32_dbg = optimized_metrics.above_32 as i64;
            // Requirement 5 (RE-306), carried into index optimization: raw
            // RGBA SSE treats alpha and RGB as equally-weighted channels, so
            // a Translucent candidate can hold or improve raw SSE while
            // making the alpha-weighted *visible* result worse -- e.g.
            // trading a solid opaque-looking edge texel for one whose alpha
            // drops, fragmenting a small sprite's silhouette into holes
            // against the background even though nothing in the raw metric
            // caught it. `solve_samples` already gates its own continuous
            // solve on this; the index-optimization stage needs the same
            // gate, since it is a second, independent place alpha can move.
            let visible_ok = match alpha_policy {
                ssb_rom::filter_compensation::AlphaPolicy::Translucent => {
                    optimized_metrics.visible_squared_error <= nearest_metrics.visible_squared_error
                }
                _ => true,
            };
            let optimized_not_worse = optimized_metrics.squared_error
                <= nearest_metrics.squared_error
                && optimized_metrics.max <= nearest_metrics.max
                && optimized_metrics.above_8 <= nearest_metrics.above_8
                && optimized_metrics.above_32 <= nearest_metrics.above_32
                && validation_preserved(nearest_validation, optimized_validation, alpha_policy)
                && visible_ok;
            let (qm, candidate) = if optimized_not_worse {
                used_index_optimization = true;
                (optimized_metrics, optimized)
            } else {
                (nearest_metrics, nearest)
            };
            quantized = candidate;
            if qm.squared_error * 100 <= solved.baseline.squared_error * 95
                && qm.above_8 < solved.baseline.above_8
                && silhouette_preserved(&quantized)
            {
                chosen = &quantized;
            } else {
                // Promotion requires >=25% measured SSE improvement and is
                // capped at 64 KiB extra per immutable variant.
                let old_bytes = (img.width * img.height * psm.bits() as u32 / 8) as i64
                    + (palette.len() * 4) as i64;
                let new_bytes = (img.width * img.height * 4) as i64;
                if solved.compensated.squared_error * 4 > solved.baseline.squared_error * 3
                    || new_bytes - old_bytes > 65536
                {
                    texgen_report(
                        &img,
                        false,
                        "no-promotion",
                        Some((
                            solved.baseline.squared_error,
                            solved.compensated.squared_error,
                        )),
                    );
                    record(&img, psm, "no-promotion", true, None);
                    return Some(psp::pack_level0(&img, psm, &palette, swizzle));
                }
                format = psp::Psm::Psm8888;
                direct_refined = Some(ssb_rom::filter_compensation::refine_integer(
                    &img,
                    &solved.rgba,
                    ssb_rom::filter_compensation::IntegerFormat::Rgba8888,
                    t.clamp_s,
                    t.clamp_t,
                    mismatch_coverage,
                    alpha_policy,
                ));
                chosen = direct_refined.as_ref().unwrap();
            }
        } else if psm == psp::Psm::Psm5551 {
            let rounded = ssb_rom::filter_compensation::quantize_rgba5551(&solved.rgba);
            quantized = ssb_rom::filter_compensation::refine_integer(
                &img,
                &rounded,
                ssb_rom::filter_compensation::IntegerFormat::Rgba5551,
                t.clamp_s,
                t.clamp_t,
                mismatch_coverage,
                alpha_policy,
            );
            integer_before_metrics = Some(ssb_rom::filter_compensation::measure_samples(
                &img,
                &rounded,
                t.clamp_s,
                t.clamp_t,
                mismatch_coverage,
            ));
            let qm = if train_samples.is_empty() {
                ssb_rom::filter_compensation::measure(&img, &quantized, t.clamp_s, t.clamp_t)
            } else {
                ssb_rom::filter_compensation::measure_samples(
                    &img,
                    &quantized,
                    t.clamp_s,
                    t.clamp_t,
                    train_samples,
                )
            };
            integer_after_metrics = Some(qm);
            if qm.squared_error * 100 <= solved.baseline.squared_error * 95
                && qm.above_8 < solved.baseline.above_8
                && qm.max <= solved.baseline.max.saturating_add(8)
                && silhouette_preserved(&quantized)
            {
                chosen = &quantized;
            } else {
                let delta = (img.width * img.height * 2) as i64;
                if solved.compensated.squared_error * 4 > solved.baseline.squared_error * 3
                    || delta > 65536
                {
                    texgen_report(
                        &img,
                        false,
                        "no-promotion",
                        Some((
                            solved.baseline.squared_error,
                            solved.compensated.squared_error,
                        )),
                    );
                    record(&img, psm, "no-promotion", true, None);
                    return Some(psp::pack_level0(&img, psm, &palette, swizzle));
                }
                format = psp::Psm::Psm8888;
                direct_refined = Some(ssb_rom::filter_compensation::refine_integer(
                    &img,
                    &solved.rgba,
                    ssb_rom::filter_compensation::IntegerFormat::Rgba8888,
                    t.clamp_s,
                    t.clamp_t,
                    mismatch_coverage,
                    alpha_policy,
                ));
                chosen = direct_refined.as_ref().unwrap();
            }
        }
        if format == psp::Psm::Psm8888 {
            integer_before_metrics = Some(solved.compensated);
            integer_after_metrics = Some(ssb_rom::filter_compensation::measure_samples(
                &img,
                chosen,
                t.clamp_s,
                t.clamp_t,
                mismatch_coverage,
            ));
        }
        let integer_fields = |m: Option<ssb_rom::filter_compensation::Metrics>| {
            m.map_or([-1; 4], |m| {
                [
                    m.squared_error as i64,
                    m.above_32 as i64,
                    m.above_8 as i64,
                    m.max as i64,
                ]
            })
        };
        let integer_before = integer_fields(integer_before_metrics);
        let integer_after = integer_fields(integer_after_metrics);
        let train_before = ssb_rom::filter_compensation::measure_samples_policy(
            &img,
            &img,
            t.clamp_s,
            t.clamp_t,
            mismatch_coverage,
            alpha_policy,
        );
        let train_after = ssb_rom::filter_compensation::measure_samples_policy(
            &img,
            chosen,
            t.clamp_s,
            t.clamp_t,
            mismatch_coverage,
            alpha_policy,
        );
        let validation_before = ssb_rom::filter_compensation::measure_samples_policy(
            &img,
            &img,
            t.clamp_s,
            t.clamp_t,
            validation_samples,
            alpha_policy,
        );
        let validation_after = ssb_rom::filter_compensation::measure_samples_policy(
            &img,
            chosen,
            t.clamp_s,
            t.clamp_t,
            validation_samples,
            alpha_policy,
        );
        let validation_accepted = !validation_samples.is_empty()
            && validation_preserved(validation_before, validation_after, alpha_policy)
            && cutout_gate.is_none_or(|(greater_or_equal, threshold)| {
                ssb_rom::filter_compensation::count_cutout_mismatches(
                    &img,
                    chosen,
                    t.clamp_s,
                    t.clamp_t,
                    validation_samples,
                    greater_or_equal,
                    threshold,
                ) == 0
            })
            && conservative_gate(chosen);
        texgen_report(
            chosen,
            validation_accepted,
            "holdout-gate",
            Some((
                solved.baseline.squared_error,
                solved.compensated.squared_error,
            )),
        );
        pack_diag!(ctx, "filter-comp file={} offset={:#X} {}x{} policy={:?} baseline mean={:.3} max={} >=8={:.2}% >=32={:.2}% visible_mean={:.3} visible_max={} compensated mean={:.3} max={} >=8={:.2}% >=32={:.2}% visible_mean={:.3} visible_max={} format={:?} memory_delta={} index_opt={} nearest_sse={} optimized_sse={} nearest_max={} optimized_max={} nearest_above8={} optimized_above8={} nearest_above32={} optimized_above32={} integer_before_sse={} integer_after_sse={} integer_before_above32={} integer_after_above32={} integer_before_above8={} integer_after_above8={} integer_before_max={} integer_after_max={} train_samples={} train_baseline_sse={} train_candidate_sse={} train_baseline_above32={} train_candidate_above32={} train_baseline_max={} train_candidate_max={} validation_samples={} validation_baseline_sse={} validation_candidate_sse={} validation_baseline_above32={} validation_candidate_above32={} validation_baseline_max={} validation_candidate_max={} validation_accepted={}",
            t.data_file.map_or(src.home.id,u32::from),t.data_offset,img.width,img.height,alpha_policy,
            solved.baseline.mean,solved.baseline.max,solved.baseline.percent_above_8(),solved.baseline.percent_above_32(),
            solved.baseline.visible_mean,solved.baseline.visible_max,
            solved.compensated.mean,solved.compensated.max,solved.compensated.percent_above_8(),solved.compensated.percent_above_32(),
            solved.compensated.visible_mean,solved.compensated.visible_max,format,
            (img.width*img.height*format.bits() as u32/8) as i64-(img.width*img.height*psm.bits() as u32/8) as i64,
            used_index_optimization, nearest_sse_dbg, optimized_sse_dbg,
            nearest_max_dbg, optimized_max_dbg, nearest_above8_dbg, optimized_above8_dbg,
            nearest_above32_dbg, optimized_above32_dbg,
            integer_before[0], integer_after[0], integer_before[1], integer_after[1],
            integer_before[2], integer_after[2], integer_before[3], integer_after[3],
            train_before.samples, train_before.squared_error, train_after.squared_error,
            train_before.above_32, train_after.above_32, train_before.max, train_after.max,
            validation_before.samples, validation_before.squared_error, validation_after.squared_error,
            validation_before.above_32, validation_after.above_32, validation_before.max,
            validation_after.max, validation_accepted);
        if !validation_accepted {
            record(&img, psm, "holdout-rejected", true, None);
            return Some(psp::pack_level0(&img, psm, &palette, swizzle));
        }
        record(
            chosen,
            format,
            if format != psm {
                "promoted-rgba8888"
            } else if psm.is_paletted() && used_index_optimization {
                "indexed-optimized"
            } else if psm.is_paletted() {
                "indexed-nearest"
            } else if psm == psp::Psm::Psm5551 {
                "direct-rgba5551"
            } else {
                "direct-rgba8888"
            },
            true,
            None,
        );
        if let Some(out) = compensated_out.as_deref_mut() {
            *out = true;
        }
        Some(psp::pack_level0(
            chosen,
            format,
            if format.is_paletted() { &palette } else { &[] },
            swizzle,
        ))
    };

    // Every format enters the same compensation pipeline (RE-313): indexed
    // formats with their palette, direct ones -- RGBA16 as `Psm5551`, RGBA32
    // and IA as `Psm8888`, and the RE-283 bypass as `Psm8888` -- without.
    if psm.is_paletted() {
        packed(index_colours.clone())
    } else {
        packed(Vec::new())
    }
}

/// TMEM contents past a short `G_LOADTLUT`, measured on the original game
/// (RE-313).
///
/// `G_LOADTLUT` writes exactly `count` entries (`angrylion-rdp-plus`'s
/// `loading_pipeline` advances one entry per span step). An index past them
/// reads whatever an earlier load left in that TMEM slot. That depends on
/// draw order, so no static rule gives it; RE-313 replayed the real frame
/// display lists through `angrylion-rdp-plus`'s TMEM loader instead. Each
/// row is `(palette file, palette offset, loaded entries, &[(slot, entry)])`.
/// The entry is copy 0 of the slot's four TMEM copies: the RDP samples the
/// other copies for other bilinear taps, which a per-texel palette cannot
/// express.
pub const STALE_TLUT_ENTRIES: &[(u32, u32, u16, &[(u8, u16)])] = &[
    // Break the Targets direction arrows (Donkey Kong, Fox, Pikachu, Kirby):
    // slot 3 holds background file 119's last strip, `1091 1091 114F 114F`,
    // in every measured frame and camera position.
    (120, 0x598, 3, &[(3, 0x1091)]),
    (121, 0x6E0, 3, &[(3, 0x1091)]),
    (122, 0x2C0, 3, &[(3, 0x1091)]),
    (123, 0xAB8, 3, &[(3, 0x1091)]),
    // Opening-room book: slot 255 holds file 52's `0x42F0` wall texture,
    // `FE59 A391 FF9B FE59`, in every measured frame.
    (52, 0x9958, 255, &[(255, 0xFE59)]),
];

/// Extends a TLUT shorter than the indices its texture uses (RE-313).
///
/// A slot [`STALE_TLUT_ENTRIES`] records gets its measured value. Any other
/// slot is unresolved stale TMEM: it is filled with transparent black, the
/// value the decoder already gives it, so the packed texels match the
/// reference either way, and it is reported.
pub fn extend_short_tlut(
    ctx: &PackCtx<'_>,
    src: Texels<'_>,
    file: &[u8],
    t: &ssb_rom::mesh::TextureRef,
    tlut: &mut Vec<u16>,
) {
    if !matches!(
        t.size,
        ssb_rom::texture::BitSize::Bits4 | ssb_rom::texture::BitSize::Bits8
    ) {
        return;
    }
    let Some(max) = decode_index_field(file, t).and_then(|i| i.into_iter().max()) else {
        return;
    };
    let need = max as usize + 1;
    if need <= tlut.len() {
        return;
    }
    let palette_file = t.palette_file.map_or(src.home.id, u32::from);
    let known = STALE_TLUT_ENTRIES
        .iter()
        .find(|(f, off, n, _)| {
            *f == palette_file && Some(*off) == t.palette_offset && *n == t.palette_entries
        })
        .map_or(&[][..], |row| row.3);
    let bank_base = if t.size == ssb_rom::texture::BitSize::Bits4 {
        u32::from(t.palette) * 16
    } else {
        0
    };
    for slot in tlut.len()..need {
        let absolute = bank_base + slot as u32;
        match known.iter().find(|(s, _)| u32::from(*s) == absolute) {
            Some(&(_, entry)) => tlut.push(entry),
            None => {
                pack_diag!(ctx,
                    "stale-tlut-unresolved file={} offset={:#X} format={:?}/{:?} tlut={:?} palette={}:{:#X} entries={} slot={}",
                    t.data_file.map_or(src.home.id, u32::from),
                    t.data_offset,
                    t.format,
                    t.size,
                    t.tlut,
                    palette_file,
                    t.palette_offset.unwrap_or(0),
                    t.palette_entries,
                    absolute
                );
                tlut.push(0);
            }
        }
    }
}

/// Decodes the visible tile from a possibly padded ROM image buffer.
///
/// `G_LOADBLOCK` records source rows in 64-bit-word strides. The renderer
/// samples only `TextureRef::width` texels. The render tile's origin belongs
/// to texture-coordinate addressing, not the source image: `mesh::Builder`
/// has already rebased authored UVs for clamped tiles. Therefore this lowers
/// the physical source buffer from its top-left corner without applying that
/// origin a second time.
pub fn decode_texture(
    file: &[u8],
    t: &ssb_rom::mesh::TextureRef,
    tlut: &[u16],
) -> Option<ssb_rom::texture::Rgba8> {
    use ssb_rom::texture;

    let source_width = u32::from(t.source_width.max(t.width));
    let width = u32::from(t.width);
    let height = u32::from(t.height);
    let source_row = texture::data_len(source_width, 1, t.size);
    let need = source_row.checked_mul(height as usize)?;
    let source = file.get(t.data_offset as usize..t.data_offset as usize + need)?;
    // `tlut` already starts at the CI4 bank; the bank only reaches the
    // decoder for a CI texel read with the TLUT off (RE-313).
    let palette = if t.tlut.enabled() { 0 } else { t.palette };
    let image = texture::decode_lut(
        source,
        source_width,
        height,
        t.format,
        t.size,
        t.tlut,
        tlut,
        palette,
    )
    .ok()?;
    if source_width == width {
        return Some(image);
    }
    let mut tile = texture::Rgba8::new(width, height);
    for y in 0..height as usize {
        for x in 0..width as usize {
            tile.put(
                y * width as usize + x,
                image.get(y * source_width as usize + x),
            );
        }
    }
    Some(tile)
}
