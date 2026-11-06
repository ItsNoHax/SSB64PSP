//! Menu-scene sprites, plays and cameras (RE-475).

use crate::*;

/// A sprite's image through its combiner, its transparent edge bled
/// (RE-517), in 8888, clamped like a texture rectangle (RE-392).
pub fn add_sprite_texture(
    writer: &mut ssb_rom::pack::PackWriter,
    s: &ssb_rom::sprite::Sprite,
    swizzle: bool,
) -> u32 {
    let image = ssb_rom::sprite::psp_image(s);
    let tex = ssb_rom::psp_texture::pack_rgba(&image, ssb_rom::psp_texture::Psm::Psm8888, swizzle);
    writer.add_texture(&tex, true, true)
}

/// A menu sprite's texture: 5551 when that holds its image exactly (the
/// RGBA16-palette pictures; the bleed copies colours the image already
/// holds, so it keeps a 5551 image 5551), else 8888.
pub fn add_menu_sprite_texture(
    writer: &mut ssb_rom::pack::PackWriter,
    s: &ssb_rom::sprite::Sprite,
    swizzle: bool,
) -> u32 {
    use ssb_rom::psp_texture::{pack_rgba, Psm};
    let image = ssb_rom::sprite::psp_image(s);
    let exact_5551 = image.pixels.as_chunks::<4>().0.iter().all(|p| {
        let c = |v: u8| (v >> 3) << 3 | v >> 5;
        c(p[0]) == p[0] && c(p[1]) == p[1] && c(p[2]) == p[2] && (p[3] == 0 || p[3] == 0xFF)
    });
    let psm = if exact_5551 {
        Psm::Psm5551
    } else {
        Psm::Psm8888
    };
    let tex = pack_rgba(&image, psm, swizzle);
    writer.add_texture(&tex, true, true)
}

/// The menu, title, How to Play and opening scenes' sprites, baked plays
/// and cameras (`ssb_rom::menu_pack::MenuScene`), into the pack itself
/// (RE-475): each belongs to its archive file and loads with the scene
/// that lists it. Sprites the pack already holds are not added again.
pub fn add_menu_scenes(
    writer: &mut ssb_rom::pack::PackWriter,
    files: &FileStore<'_>,
    swizzle: bool,
) -> Result<usize, PackError> {
    use ssb_rom::menu_pack::MenuScene;
    use ssb_rom::pack::{AnimDesc, SpriteDesc};
    let file = |id: u32| {
        files
            .get(id as usize)
            .ok_or_else(|| format!("menu sprite file {id:#x} missing"))
    };
    let mut sprites = 0usize;
    for scene in MenuScene::ALL {
        for f in scene.sprites() {
            let data = file(f.file)?;
            for &at in f.offsets {
                if writer.has_sprite(f.file, at, SpriteDesc::ROLE_NONE, 0) {
                    continue;
                }
                let s = ssb_rom::sprite::decode(&data, at)
                    .map_err(|e| format!("menu sprite {:#x}+{at:#x}: {e:?}", f.file))?;
                let texture = add_menu_sprite_texture(writer, &s, swizzle);
                writer.add_sprite(sprite_desc(f.file, at, &s, texture, 0, 0, 0));
                sprites += 1;
            }
        }
        for &(id, at, luts) in scene.lut_sprites() {
            let data = file(id)?;
            for (i, &lut) in luts.iter().enumerate() {
                if writer.has_sprite(id, at, SpriteDesc::ROLE_LUT, i as u8) {
                    continue;
                }
                let bytes = data
                    .data
                    .get(lut as usize..lut as usize + 32)
                    .ok_or_else(|| format!("menu palette {id:#x}+{lut:#x} missing"))?;
                let s = ssb_rom::sprite::decode_with_tlut(
                    &data,
                    at,
                    &ssb_rom::texture::parse_tlut(bytes),
                )
                .map_err(|e| format!("menu sprite {id:#x}+{at:#x} LUT {i}: {e:?}"))?;
                let texture = add_menu_sprite_texture(writer, &s, swizzle);
                writer.add_sprite(sprite_desc(
                    id,
                    at,
                    &s,
                    texture,
                    0,
                    SpriteDesc::ROLE_LUT,
                    i as u8,
                ));
                sprites += 1;
            }
        }
        if scene == MenuScene::Title
            && !writer.has_anim(AnimDesc::EFFECT, ssb_rom::title::LABELS_SLOT)
        {
            add_title_anims(writer, &*file(ssb_rom::title::FILE)?)?;
            for slot in [
                ssb_rom::title::LABELS_SLOT,
                ssb_rom::title::PRESS_START_SLOT,
                ssb_rom::title::LOGO_SLOT,
            ] {
                writer.set_owner(slot, ssb_rom::title::FILE);
            }
        }
        for &id in scene.blobs() {
            let slot = ssb_rom::opening::blob_slot(id);
            if writer.has_anim(AnimDesc::EFFECT, slot) {
                continue;
            }
            writer.add_anim(AnimDesc::EFFECT, slot, id, 0, &file(id)?.data, &[]);
        }
        for cam in scene.cameras() {
            if writer.has_anim(AnimDesc::EFFECT, cam.slot()) {
                continue;
            }
            let frames = ssb_rom::camanim::bake(&file(cam.file)?.data, cam.offset, cam.init, 8192)
                .map_err(|e| format!("camera {:#x}+{:#x}: {e:?}", cam.file, cam.offset))?;
            if frames.is_empty() || frames.len() >= 8192 {
                return Err(format!(
                    "camera {:#x}+{:#x}: {} plays",
                    cam.file,
                    cam.offset,
                    frames.len()
                )
                .into());
            }
            writer.add_anim(
                AnimDesc::EFFECT,
                cam.slot(),
                cam.slot(),
                frames.len() as u32,
                &ssb_rom::camanim::to_bytes(&frames),
                &[],
            );
            writer.set_owner(cam.slot(), cam.file);
        }
        if scene == MenuScene::Explain
            && !writer.has_anim(AnimDesc::EFFECT, ssb_rom::explain::PHASES_SLOT)
        {
            sprites += add_explain(
                writer,
                &*file(ssb_rom::explain::FILE_MAIN)?,
                &*file(ssb_rom::explain::FILE_GRAPHICS)?,
                swizzle,
            )?;
            for slot in [ssb_rom::explain::PHASES_SLOT, ssb_rom::explain::KEYS_SLOT] {
                writer.set_owner(slot, ssb_rom::explain::FILE_MAIN);
            }
            writer.set_owner(
                ssb_rom::explain::ANIMS_SLOT,
                ssb_rom::explain::FILE_GRAPHICS,
            );
        }
    }
    Ok(sprites)
}

/// How to Play's raw textures as sprites keyed by their texture's offset,
/// and its three data blobs (`ssb_rom::explain`). Returns the sprites
/// added.
pub fn add_explain(
    writer: &mut ssb_rom::pack::PackWriter,
    main: &ssb_rom::archive::File,
    graphics: &ssb_rom::archive::File,
    swizzle: bool,
) -> Result<usize, PackError> {
    use ssb_rom::explain as ex;
    use ssb_rom::psp_texture::{pack_rgba, Psm};
    let mut images = Vec::new();
    for &at in &ex::STICK_TEXTURES {
        images.push((
            at,
            ex::stick_texture(graphics, at).ok_or(format!("explain stick {at:#x}"))?,
        ));
    }
    for &at in &ex::SPARK_TEXTURES {
        images.push((
            at,
            ex::spark_texture(graphics, at).ok_or(format!("explain spark {at:#x}"))?,
        ));
    }
    images.push((
        ex::RGB_TEXTURE,
        ex::rgb_texture(graphics).ok_or("explain overlay")?,
    ));
    let count = images.len();
    for (at, mut image) in images {
        // Drawn as sprites: bled like the others (RE-517).
        ssb_rom::sprite::bleed_transparent_rgb(&mut image);
        let texture = writer.add_texture(&pack_rgba(&image, Psm::Psm8888, swizzle), true, true);
        writer.add_sprite(ssb_rom::pack::SpriteDesc {
            source_file: ex::FILE_GRAPHICS,
            source_offset: at,
            texture,
            width: image.width as u16,
            height: image.height as u16,
            color: [0xFF; 4],
            attr: 0,
            flags: 0,
            fighter: 0,
            role: ssb_rom::pack::SpriteDesc::ROLE_NONE,
            costume: 0,
            _pad: 0,
        });
    }
    let blobs = [
        (
            ex::PHASES_SLOT,
            ex::phase_table(main).ok_or("explain phase table")?,
        ),
        (
            ex::KEYS_SLOT,
            ex::key_scripts(main).ok_or("explain input scripts")?,
        ),
        (
            ex::ANIMS_SLOT,
            ex::anim_bytes(graphics).ok_or("explain animations")?,
        ),
    ];
    for (slot, bytes) in blobs {
        writer.add_anim(ssb_rom::pack::AnimDesc::EFFECT, slot, slot, 1, &bytes, &[]);
    }
    Ok(count)
}

/// `mnTitle`'s labels and "Press Start" trees played ahead
/// (`ssb_rom::title`): the labels' 51 plays, and one creation play plus a
/// loop of "Press Start"'s.
pub fn add_title_anims(
    writer: &mut ssb_rom::pack::PackWriter,
    title: &ssb_rom::archive::File,
) -> PackResult {
    use ssb_rom::title;
    let pins: Vec<[f32; 2]> = title::LABEL_CENTRES
        .iter()
        .map(|c| title::pin(c[0], c[1]))
        .collect();
    let labels = title::bake(
        &title.data,
        title::LABELS_DOBJDESC,
        title::LABELS_ANIM_JOINT,
        &pins,
        title::LABEL_PLAYS,
    )
    .map_err(|e| format!("title labels: {e:?}"))?;
    let c = title::PRESS_START_CENTRE;
    let press = title::bake(
        &title.data,
        title::PRESS_START_DOBJDESC,
        title::PRESS_START_ANIM_JOINT,
        &[title::pin(c[0], c[1])],
        1 + title::PRESS_START_PERIOD,
    )
    .map_err(|e| format!("title press start: {e:?}"))?;
    // The opening layout's logo (RE-467): `mnTitleMakeLogo` zeroes each
    // child's translation after the creation's play.
    let logo = title::bake(
        &title.data,
        title::LOGO_DOBJDESC,
        title::LOGO_ANIM_JOINT,
        &[[0.0; 2]; title::LOGO_CHILDREN],
        title::LOGO_PLAYS,
    )
    .map_err(|e| format!("title logo: {e:?}"))?;
    for (slot, frames) in [
        (title::LABELS_SLOT, &labels),
        (title::PRESS_START_SLOT, &press),
        (title::LOGO_SLOT, &logo),
    ] {
        writer.add_anim(
            ssb_rom::pack::AnimDesc::EFFECT,
            slot,
            slot,
            frames.len() as u32,
            &title::to_bytes(frames),
            &[],
        );
    }
    Ok(())
}

pub fn sprite_desc(
    file: u32,
    at: u32,
    s: &ssb_rom::sprite::Sprite,
    texture: u32,
    fighter: u8,
    role: u8,
    costume: u8,
) -> ssb_rom::pack::SpriteDesc {
    ssb_rom::pack::SpriteDesc {
        source_file: file,
        source_offset: at,
        texture,
        width: s.width,
        height: s.height,
        color: s.color,
        attr: s.attr,
        flags: if ssb_rom::sprite::uses_prim_color(s.format) {
            ssb_rom::pack::SpriteDesc::TINTED
        } else {
            0
        },
        fighter,
        role,
        costume,
        _pad: 0,
    }
}
