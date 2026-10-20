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
use ssb_game::results_emblem;
use ssb_game::results_scene::{self, Scene};
use ssb_psp_runtime::gu::Gpu;
use ssb_psp_runtime::meshdraw;
use ssb_psp_runtime::transition::Transition;
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
    /// The demo fighter: its status script's parts (RE-426), sounds and
    /// effects.
    demo: ssb_game::demo_fighter::DemoFighter,
}

/// The scene, its fighters' poses, its emblem, its particles and its 2D
/// layer.
pub struct Fighters {
    pub scene: Scene,
    pub layer: Layer,
    models: [Option<Box<Model>>; 4],
    /// `mnVSResultsMakeEmblem`'s DObj, after a contest (RE-420).
    pub emblem: Option<Emblem>,
    /// `efParticleInitAll` and `efManagerInitEffects`: the confetti's
    /// and the fighters' effects.
    world: crate::demo_world::DemoWorld,
    /// `lbTransitionMakeTransition`'s wipe over the battle's last frame,
    /// after a contest (RE-464).
    wipe: Option<Transition>,
    /// `lbTransitionSetupTransition`'s photo, still to be copied from the
    /// screen before the scene's first frame opens.
    pub photo_pending: bool,
}

/// The emblem and its material animation, started at the player's colour.
pub struct Emblem {
    pub state: results_emblem::Emblem,
    /// `(object, material player)`, if the pack has the tree.
    model: Option<(ssb_rom::pack::ObjectDesc, ssb_rom::skeleton::EffectMaterialAnimator)>,
}

/// `mnVSResultsFuncStart`: the scene's first random pick, the wipe it
/// names (`lbTransitionSetupTransition`, `lbTransitionMakeTransition`), then
/// the emblem (`entrants` gives the winner's kind). Boxed so the session
/// holds a pointer.
#[inline(never)]
pub fn start(pack: Option<&Pack<'_>>, r: &Results, entrants: [Option<(FighterKind, u8)>; 4]) -> Box<Fighters> {
    let scene = Scene::start(r);
    let wipe = scene
        .transition
        .and_then(|id| pack.and_then(|p| Transition::make(p, id as u32)));
    let winner_kind = r
        .winner
        .and_then(|w| entrants.get(w).copied().flatten())
        .map(|(kind, _)| kind);
    let emblem = winner_kind
        .and_then(|kind| results_emblem::Emblem::make(r, kind))
        .map(|state| Emblem {
            state,
            model: pack.and_then(|p| make_emblem(p, &state)),
        });
    Box::new(Fighters {
        scene,
        layer: Layer::new(),
        models: [None, None, None, None],
        emblem,
        world: crate::demo_world::DemoWorld::new(),
        photo_pending: wipe.is_some(),
        wipe,
    })
}

/// `gcSetupCommonDObjs`, `gcAddMObjAll`, `gcAddMatAnimJointAll(..., color)`
/// and the one `gcPlayAnimAll`: the winner's series tree with its
/// material scripts started at frame `color` and played once, which
/// leaves its light colours at the player's (RE-420).
fn make_emblem(
    p: &Pack<'_>,
    e: &results_emblem::Emblem,
) -> Option<(ssb_rom::pack::ObjectDesc, ssb_rom::skeleton::EffectMaterialAnimator)> {
    let series = ssb_rom::effect::EMBLEM_BY_FIGHTER.get(e.fighter as usize)?;
    let desc = ssb_rom::effect::EMBLEMS.get(usize::from(*series))?;
    let object = ssb_psp_runtime::scene::object_keyed(p, (ssb_rom::effect::EMBLEM_FILE, desc.dobjdesc))?;
    let mut mats = ssb_rom::skeleton::EffectMaterialAnimator::new();
    let prims = (0..object.node_count)
        .filter_map(|n| p.node(object.first_node + n))
        .filter_map(|node| p.mesh(node.mesh))
        .flat_map(|m| (0..m.prim_count).map(move |j| m.first_prim + j))
        .filter_map(|i| p.prim(i))
        .map(|prim| prim.mat_anim);
    mats.start_at(p, prims, f32::from(e.color));
    mats.tick(p);
    Some((object, mats))
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
    // `lbTransitionProcUpdate`, a link-0 process made after
    // `mnVSResultsFuncRun`'s: the wipe ends when its clock does.
    if f.wipe.as_mut().is_some_and(|w| !w.update(p)) {
        f.wipe = None;
    }
    // Link 0 again: `mnVSResultsFuncRun`'s confetti, then the particles'
    // `func_run`s, made after it (RE-420).
    f.world.with(p, |rt| {
        if results_emblem::confetti_due(r) {
            for (pos, is_genlink_mask) in results_emblem::CONFETTI {
                rt.effects.confetti(rt.particles, rt.banks, pos, is_genlink_mask);
            }
        }
        rt.run();
    });
    // `mnVSResultsEmblemProcUpdate`.
    if let Some(e) = f.emblem.as_mut() {
        e.state.update(r);
    }
    if r.fighters_due() {
        f.scene.init_fighters_all(r, entrants);
        for (slot, fighter) in f.models.iter_mut().zip(f.scene.fighters) {
            *slot = fighter.map(|fighter| make_model(p, fighter));
        }
    } else {
        // `scSubsysFighterProcUpdate`: the clip, the status script's
        // events, the colour animation.
        for m in f.models.iter_mut().flatten() {
            play(p, m);
            m.demo.tick(m.skeleton.frame());
        }
    }
    // `ftParamMakeEffect` from the scripts' events, each placed on its
    // joint as the fighter now stands; then the effect processes
    // (priority 3, after the fighters' 5).
    for m in f.models.iter_mut().flatten() {
        let stand = crate::demo_world::Stand {
            pos: [m.fighter.pos.x, m.fighter.pos.y, m.fighter.pos.z],
            yaw: m.fighter.rotate_y,
            scale: m.fighter.scale,
        };
        f.world.make(p, m.object, &m.skeleton, stand, &mut m.demo);
    }
    f.world.process(p);
}

/// `mnVSResultsMakeFighter` and `scSubsysFighterSetStatus`.
#[inline(never)]
fn make_model(p: &Pack<'_>, fighter: results_scene::Fighter) -> Box<Model> {
    let kind = fighter.kind as u32;
    let mut m = Box::new(Model {
        fighter,
        object: ssb_psp_runtime::scene::fighter_object(p, kind).unwrap_or(u32::MAX),
        slot: None,
        // Win and lose motions are unflagged: the `animlock` parts stay
        // frozen (RE-509).
        skeleton: ssb_psp_runtime::scene::fighter_skeleton(fighter.kind, 1.0),
        demo: ssb_game::demo_fighter::DemoFighter::new(fighter.kind, fighter.player as u8, fighter.costume),
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
    // `ftMainSetStatus` runs the script's time-zero events after the
    // clip's first frame.
    m.demo.set_status(fighter.status.map_or(0, |s| 1 + s.index()));
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
/// and table; last the wipe (`lbTransitionMakeCamera`'s priority 10).
#[inline(never)]
pub unsafe fn draw_all(
    gpu: &mut Gpu,
    p: &Pack<'_>,
    draw_state: &mut meshdraw::DrawState,
    r: &Results,
    f: &mut Fighters,
    players: &[Option<Player>; 4],
) {
    let Fighters {
        layer,
        models,
        emblem,
        world,
        wipe,
        ..
    } = f;
    let alpha = r.character_alpha.clamp(0, 0xFF) as u8;
    layer.visit(r, players, |d| match d {
        Draw::Wallpaper { prim, env } => draw_wallpaper(p, draw_state, prim, env),
        Draw::Fill { rect, color } => meshdraw::fill_rect_n64(rect, color, draw_state),
        Draw::Sprite(piece) => draw_piece(p, draw_state, &piece),
        Draw::Emblem => draw_emblem(gpu, p, draw_state, emblem.as_ref()),
        Draw::Fighters => draw(gpu, p, draw_state, models, world, alpha),
    });
    if let Some(w) = wipe.as_ref() {
        w.draw(p, gpu, draw_state);
    }
}

/// `mnVSResultsMakeEmblemCamera`'s camera over the emblem: its root at
/// `translate`, scaled by `scale` in x and y, lit by the scene's fighter
/// light in the colours its material scripts hold (RE-420).
#[inline(never)]
unsafe fn draw_emblem(gpu: &mut Gpu, p: &Pack<'_>, draw_state: &mut meshdraw::DrawState, emblem: Option<&Emblem>) {
    let Some((e, (object, mats))) = emblem.and_then(|e| Some((&e.state, e.model.as_ref()?))) else {
        return;
    };
    let cam = &results_emblem::CAMERA;
    gpu.set_viewport_n64(cam.viewport);
    gpu.set_perspective(cam.fovy, cam.aspect, cam.near, cam.far);
    gpu.reset_modelview();
    draw_state.begin_frame();
    gpu.set_view(&ssb_engine::math::Mat4::look_at(cam.eye, cam.at, cam.up));
    let t = e.translate;
    let k = meshdraw::MODEL_SCALE;
    gpu.model_transform_xyz([t.x, t.y, t.z], [0.0; 3], [e.scale * k, e.scale * k, k]);
    let base = gpu.model_matrix();
    let posed = rooted_rest_pose(p, object);
    draw_state.configure_fighter_light(results_scene::LIGHT_ANGLE);
    meshdraw::draw_object_posed(p, object, &base, &posed[..object.node_count as usize], None, draw_state, None, Some(mats), 0);
    draw_state.finish_fighter_light();
    gpu.set_viewport_pillarboxed();
}

/// Every node's rest transform below a root the caller places: the root
/// is the identity (`mnVSResultsMakeEmblem` sets its translation and
/// scale), each child its parent's matrix times its own rest transform.
pub(crate) fn rooted_rest_pose(p: &Pack<'_>, object: &ssb_rom::pack::ObjectDesc) -> [ssb_rom::scene::Mat4; 8] {
    use ssb_rom::scene::Mat4;
    let mut posed = [Mat4::IDENTITY; 8];
    for n in 1..(object.node_count as usize).min(posed.len()) {
        let Some(node) = p.node(object.first_node + n as u32) else {
            continue;
        };
        let local = Mat4::from_trs_display(
            node.rest_translate.map(|x| x / meshdraw::MODEL_SCALE),
            node.rest_rotate,
            node.rest_scale,
        );
        let parent = node
            .parent
            .checked_sub(object.first_node)
            .map_or(0, |i| (i as usize).min(n - 1));
        posed[n] = posed[parent].mul(&local);
    }
    posed
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
/// `mnVSResultsFuncLights`'s angles and faded in at `alpha`
/// (`scSubsysFighterSetLightParams`' `sMNVSResultsCharacterAlpha`, RE-464). A clip that leads with a runtime
/// joint (Kirby's Win1 and Win2, Pikachu's Win1, Ness's Win2) poses the
/// model as it stands: `ftMainSetStatus` detaches `TransN` from the
/// hierarchy, and no demo status moves the fighter by it.
#[inline(never)]
unsafe fn draw(
    gpu: &mut Gpu,
    p: &Pack<'_>,
    draw_state: &mut meshdraw::DrawState,
    models: &[Option<Box<Model>>; 4],
    world: &crate::demo_world::DemoWorld,
    alpha: u8,
) {
    let cam = &results_scene::CAMERA;
    gpu.set_viewport_n64(cam.viewport);
    gpu.set_perspective(cam.fovy, cam.aspect, cam.near, cam.far);
    gpu.reset_modelview();
    draw_state.begin_frame();
    gpu.set_view(&ssb_engine::math::Mat4::look_at(cam.eye, cam.at, cam.up));
    for m in models.iter().flatten() {
        let Some(obj) = p.object(m.object) else {
            continue;
        };
        let mut posed = [ssb_rom::scene::Mat4::IDENTITY; ssb_rom::skeleton::MAX_NODES];
        // The parts an effect's joint query marked this frame draw from
        // their collision locals (RE-509).
        let queried = m.demo.fighter.queried_joints.get();
        let lead = m.skeleton.lead_xrotn_pose().is_some() && queried & 1 << 2 != 0;
        let n = m.skeleton.compose_queried(p, &obj, &mut posed, queried >> 4, lead);
        let pos = m.fighter.pos;
        let base = crate::topn_base([pos.x, pos.y, pos.z], m.fighter.rotate_y, m.fighter.scale, m.skeleton.locks.flagged);
        draw_state.configure_fighter_light(results_scene::LIGHT_ANGLE);
        draw_state.set_fade_alpha(Some(alpha));
        // A demo fighter's model and texture parts (RE-426) and accessory
        // (RE-425).
        let parts = m.demo.parts().draw_parts();
        meshdraw::draw_fighter_posed(
            p,
            &obj,
            &base,
            &posed[..n],
            draw_state,
            meshdraw::Look {
                costume: u32::from(m.fighter.costume),
                textures: m.demo.parts().draw_textures(),
                parts: parts.as_ref().map(|p| &p[..]),
                accessory_before: m.fighter.kind == ssb_game::fighter::FighterKind::Purin,
            },
        );
        draw_state.finish_fighter_light();
    }
    draw_state.set_fade_alpha(None);
    // DL links 10, 15 and 18 of the same camera.
    world.draw(p, cam, draw_state);
    gpu.set_viewport_pillarboxed();
}
