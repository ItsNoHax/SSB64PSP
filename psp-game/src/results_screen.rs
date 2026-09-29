//! The VS results screen (RE-409, RE-410): `ssb_game::results_scene`
//! places the fighters and picks their demo statuses; this poses each one
//! with its demo clip (`anim::SLOT_WIN1` to `SLOT_LOSE`) and draws it under
//! `mnVSResultsMakeFighterCamera`'s camera, between the wallpaper and the
//! text and table `ssb_game::results_layer` lays out.
//!
//! Everything here lives on the heap: a [`Skeleton`] is tens of KB and the
//! main thread's stack is 256 KB.

use alloc::boxed::Box;

use ssb_game::fighter::FighterKind;
use ssb_game::results::Results;
use ssb_game::results_layer::{self, Draw, Layer, Player, Sprite, SpriteFile};
use ssb_game::results_scene::{self, Scene};
use ssb_psp_runtime::gu::Gpu;
use ssb_psp_runtime::meshdraw;
use ssb_rom::pack::Pack;
use ssb_rom::skeleton::Skeleton;

/// One results fighter's pose.
pub struct Model {
    pub fighter: results_scene::Fighter,
    /// The object the fighter's clips drive, or `u32::MAX`.
    object: u32,
    /// The demo clip's slot, if the fighter has a status and the pack the
    /// clip.
    slot: Option<u32>,
    skeleton: Skeleton,
}

/// The scene, its fighters' poses and its 2D layer.
pub struct Fighters {
    pub scene: Scene,
    pub layer: Layer,
    models: [Option<Box<Model>>; 4],
}

/// `mnVSResultsFuncStart`: the scene's first random pick. Boxed so the
/// session holds a pointer.
#[inline(never)]
pub fn start(r: &Results) -> Box<Fighters> {
    Box::new(Fighters {
        scene: Scene::start(r),
        layer: Layer::new(),
        models: [None, None, None, None],
    })
}

/// One tic of the fighters, after [`Results::tick`]: on
/// `sMNVSResultsInitFightersAllTic` they are made and their statuses set
/// (`ftMainSetStatus` plays the clip's first frame at once); on later tics
/// each plays on (`ftMainProcUpdateMain`). A Win clip holds its last frame
/// and the claps loop, as the clips themselves do.
#[inline(never)]
pub fn tick(pack: Option<&Pack<'_>>, r: &Results, f: &mut Fighters, entrants: [Option<(FighterKind, u8)>; 4]) {
    // `mnVSResultsFuncRun` makes the wallpaper and text before the
    // fighters.
    f.layer.tick(r);
    let Some(p) = pack else {
        return;
    };
    if r.fighters_due() {
        f.scene.init_fighters_all(r, entrants);
        for (slot, fighter) in f.models.iter_mut().zip(f.scene.fighters) {
            *slot = fighter.map(|fighter| make_model(p, fighter));
        }
        return;
    }
    for m in f.models.iter_mut().flatten() {
        play(p, m);
    }
}

/// `mnVSResultsMakeFighter` and `scSubsysFighterSetStatus`.
#[inline(never)]
fn make_model(p: &Pack<'_>, fighter: results_scene::Fighter) -> Box<Model> {
    let kind = fighter.kind as u32;
    let mut m = Box::new(Model {
        fighter,
        object: ssb_psp_runtime::scene::fighter_object(p, kind).unwrap_or(u32::MAX),
        slot: None,
        skeleton: Skeleton::new(),
    });
    if let Some(status) = fighter.status {
        let slot = (ssb_rom::anim::SLOT_WIN1 + status.index()) as u32;
        if let Some(anim) = p.fighter_anim(kind, slot) {
            // `ftMainSetStatus(gobj, status, 0, 1.0F, 0.0F)`.
            m.skeleton.start(p, &anim, 0.0, 1.0);
            m.slot = Some(slot);
            play(p, &mut m);
        }
    }
    m
}

/// One frame of the fighter's clip. Luigi's own demo rows play without his
/// translation scales (`FTANIM_FLAG_TRANSLATE_SCALES`); every other row
/// plays through the fighter's scales, if it has any.
fn play(p: &Pack<'_>, m: &mut Model) {
    let Some(anim) = m.slot.and_then(|slot| p.fighter_anim(m.fighter.kind as u32, slot)) else {
        return;
    };
    let Some(script) = p.anim_script(&anim) else {
        return;
    };
    let skips = m.fighter.status.is_some_and(|s| s.skips_translate_scales(m.fighter.kind));
    let scales = if skips {
        None
    } else {
        p.fighter_translate_scales(m.fighter.kind as u32)
    };
    let first_node = p.object(m.object).map_or(0, |o| o.first_node);
    let _ = m.skeleton.tick_scaled(script, scales, first_node);
}

/// The screen back to front (`ssb_game::results_layer::Layer::visit`):
/// the wallpaper and its fades, the fighters, then the tags, text, tint
/// and table.
#[inline(never)]
pub unsafe fn draw_all(
    gpu: &mut Gpu,
    p: &Pack<'_>,
    draw_state: &mut meshdraw::DrawState,
    r: &Results,
    f: &Fighters,
    players: &[Option<Player>; 4],
) {
    f.layer.visit(r, players, |d| match d {
        Draw::Wallpaper { prim, env } => draw_wallpaper(p, draw_state, prim, env),
        Draw::Fill { rect, color } => meshdraw::fill_rect_n64(rect, color, draw_state),
        Draw::Sprite(piece) => draw_piece(p, draw_state, &piece),
        Draw::Fighters => draw(gpu, p, draw_state, f),
    });
}

/// `mnVSResultsWallpaperProcDisplay`'s `(PRIM - ENV) * TEXEL0 + ENV` over
/// the I4 wallpaper, opaque: the pack's I texture is white with the
/// intensity as alpha, so the environment is filled first and the
/// primitive blended over it by that alpha.
unsafe fn draw_wallpaper(p: &Pack<'_>, draw_state: &mut meshdraw::DrawState, prim: [u8; 3], env: [u8; 3]) {
    let f = &ssb_rom::sprite::VS_RESULTS;
    let Some(s) = f
        .offsets
        .get(usize::from(results_layer::WALLPAPER))
        .and_then(|&at| p.sprite(f.file, at))
    else {
        return;
    };
    let (x, y) = (10.0, 10.0);
    let rect = [x, y, x + f32::from(s.width), y + f32::from(s.height)];
    meshdraw::fill_rect_n64(rect, [env[0], env[1], env[2], 0xFF], draw_state);
    let d = meshdraw::SObjDraw {
        x,
        y,
        scale: 1.0,
        prim: [prim[0], prim[1], prim[2], 0xFF],
        env,
        solid: false,
        attr: s.attr | ssb_rom::sprite::SP_TRANSPARENT,
    };
    meshdraw::draw_sprite(p, &s, &d, draw_state);
}

/// The pack's sprite for a layer sprite.
fn sprite_of(p: &Pack<'_>, sprite: Sprite) -> Option<ssb_rom::pack::SpriteDesc> {
    use ssb_rom::sprite as rom;
    let (file, i) = match sprite {
        Sprite::Stock { kind, costume } => {
            return p.fighter_sprite(kind as u8, ssb_rom::pack::SpriteDesc::ROLE_STOCK, costume);
        }
        Sprite::In(file, i) => (file, i),
    };
    let f = match file {
        SpriteFile::VsResults => &rom::VS_RESULTS,
        SpriteFile::GameModes => &rom::GAME_MODES,
        SpriteFile::Digits => &rom::DIGITS,
        SpriteFile::PlayerDamage => &rom::PLAYER_DAMAGE,
        SpriteFile::Announce => &rom::ANNOUNCE_COMMON,
        SpriteFile::PlayerTags => &rom::PLAYER_TAGS,
    };
    f.offsets.get(usize::from(i)).and_then(|&at| p.sprite(f.file, at))
}

/// One `SObj` through `lbCommonDrawSObjAttr`, with `SP_FASTCOPY` cleared
/// and `SP_TRANSPARENT` set as every results sprite has them.
unsafe fn draw_piece(p: &Pack<'_>, draw_state: &mut meshdraw::DrawState, piece: &results_layer::Piece) {
    const SP_FASTCOPY: u16 = 0x0020;
    let Some(s) = sprite_of(p, piece.sprite) else {
        return;
    };
    let [r, g, b, a] = s.color;
    let prim = piece.prim.map_or([r, g, b, a], |c| [c[0], c[1], c[2], a]);
    let d = meshdraw::SObjDraw {
        x: piece.x,
        y: piece.y,
        scale: 1.0,
        prim,
        env: piece.env,
        solid: false,
        attr: (s.attr & !SP_FASTCOPY) | ssb_rom::sprite::SP_TRANSPARENT,
    };
    meshdraw::draw_sprite_xy(p, &s, &d, [piece.scale_x, 1.0], draw_state);
}

/// The fighters under `mnVSResultsMakeFighterCamera`'s camera, lit from
/// `mnVSResultsFuncLights`'s angles. A clip that leads with a runtime
/// joint (Kirby's Win1 and Win2, Pikachu's Win1, Ness's Win2) poses the
/// model as it stands: `ftMainSetStatus` detaches `TransN` from the
/// hierarchy, and no demo status moves the fighter by it.
#[inline(never)]
unsafe fn draw(gpu: &mut Gpu, p: &Pack<'_>, draw_state: &mut meshdraw::DrawState, f: &Fighters) {
    let cam = &results_scene::CAMERA;
    gpu.set_viewport_n64(cam.viewport);
    gpu.set_perspective(cam.fovy, cam.aspect, cam.near, cam.far);
    gpu.reset_modelview();
    draw_state.begin_frame();
    gpu.set_view(&ssb_engine::math::Mat4::look_at(cam.eye, cam.at, cam.up));
    for m in f.models.iter().flatten() {
        let Some(obj) = p.object(m.object) else {
            continue;
        };
        let mut posed = [ssb_rom::scene::Mat4::IDENTITY; ssb_rom::skeleton::MAX_NODES];
        let n = m.skeleton.compose(p, &obj, &mut posed);
        let pos = m.fighter.pos;
        gpu.model_transform(
            [pos.x, pos.y, pos.z],
            [0.0, m.fighter.rotate_y, 0.0],
            meshdraw::MODEL_SCALE * m.fighter.scale,
        );
        let base = gpu.model_matrix();
        draw_state.configure_fighter_light(results_scene::LIGHT_ANGLE);
        meshdraw::draw_object_posed(
            p,
            &obj,
            &base,
            &posed[..n],
            None,
            draw_state,
            None,
            None,
            u32::from(m.fighter.costume),
        );
        draw_state.finish_fighter_light();
    }
    gpu.set_viewport_fullscreen();
}
