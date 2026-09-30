//! The character selects' drawing: VS (RE-411) and Training.
//! `ssb_game::players_vs::layer` and `ssb_game::fighter_select::layer` lay
//! out the sprites and track each slot's fighter; this looks the sprites up
//! in the pack, poses each fighter with its demo clip (`Wait` for
//! `nFTDemoStatusNull`, `anim::SLOT_WIN1` to `SLOT_WIN4` once placed) and
//! draws it under the fighter camera (`mnPlayersVSMakeFighterCamera` and
//! `mnPlayers1PTrainingMakeFighterCamera` are the same), between the
//! panels and the pucks.
//!
//! The poses live on the heap: a [`Skeleton`] is tens of KB and the main
//! thread's stack is 256 KB.

use alloc::boxed::Box;

use ssb_engine::math::Vec3;
use ssb_game::fighter_select::FighterSelect;
use ssb_game::players_vs::layer::{self, Draw, Piece};
use ssb_game::players_vs::PlayersVs;
use ssb_game::results_scene::Camera;
use ssb_psp_runtime::gu::Gpu;
use ssb_psp_runtime::meshdraw;
use ssb_rom::pack::Pack;
use ssb_rom::skeleton::Skeleton;

/// `mnPlayersVSMakeFighterCamera`: eye (0, 0, 5000) on the origin, with
/// `dGCPerspDefault`'s projection as every camera (RE-409).
const CAMERA: Camera = Camera {
    eye: Vec3 {
        x: 0.0,
        y: 0.0,
        z: 5000.0,
    },
    at: Vec3 {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    },
    up: Vec3 {
        x: 0.0,
        y: 1.0,
        z: 0.0,
    },
    fovy: 30.0,
    aspect: 4.0 / 3.0,
    near: 100.0,
    far: 12800.0,
    viewport: layer::VIEWPORT,
};

/// `mnPlayersVSFuncStart`'s `scSubsysFighterSetLightParams(45.0F, 45.0F,
/// ...)`.
const LIGHT_ANGLE: [f32; 2] = [45.0, 45.0];

/// One slot's pose.
pub struct Model {
    /// The layer's fighter the pose was started for.
    serial: u32,
    kind: ssb_game::fighter::FighterKind,
    object: u32,
    slot: Option<u32>,
    skips_scales: bool,
    skeleton: Skeleton,
}

/// The four slots' poses.
pub struct Fighters {
    models: [Option<Box<Model>>; 4],
}

/// Boxed so the session holds a pointer.
#[inline(never)]
pub fn start() -> Box<Fighters> {
    Box::new(Fighters {
        models: [None, None, None, None],
    })
}

/// One tick of the fighters, after [`PlayersVs::tick`].
pub fn tick(pack: Option<&Pack<'_>>, select: &PlayersVs, f: &mut Fighters) {
    let fighters: [Option<layer::Fighter>; 4] = core::array::from_fn(|i| select.view.slots[i].fighter);
    tick_models(pack, &fighters, f);
}

/// One tick of the Training select's two fighters, after
/// [`FighterSelect::tick`].
pub fn tick_training(pack: Option<&Pack<'_>>, select: &FighterSelect, f: &mut Fighters) {
    let v = &select.view.slots;
    tick_models(pack, &[v[0].fighter, v[1].fighter, None, None], f);
}

/// A fighter made again or given its status starts its clip over
/// (`ftMainSetStatus` plays the first frame at once); otherwise it plays on.
#[inline(never)]
fn tick_models(pack: Option<&Pack<'_>>, fighters: &[Option<layer::Fighter>; 4], f: &mut Fighters) {
    let Some(p) = pack else {
        return;
    };
    for (model, fighter) in f.models.iter_mut().zip(fighters.iter()) {
        let Some(fighter) = *fighter else {
            *model = None;
            continue;
        };
        let fresh = model
            .as_ref()
            .is_none_or(|m| m.serial != fighter.serial || m.kind != fighter.kind);
        if fresh {
            *model = Some(make_model(p, fighter));
        } else if let Some(m) = model.as_deref_mut() {
            play(p, m);
        }
    }
}

/// The fighter's clip from its first frame: `Wait` for
/// `nFTDemoStatusNull` (submotion row 0 is each fighter's `Wait` figatree),
/// else the demo status's.
#[inline(never)]
fn make_model(p: &Pack<'_>, fighter: layer::Fighter) -> Box<Model> {
    let kind = fighter.kind as u32;
    let slot = match fighter.status {
        None => ssb_rom::anim::SLOT_WAIT,
        Some(status) => ssb_rom::anim::SLOT_WIN1 + status.index(),
    } as u32;
    let mut m = Box::new(Model {
        serial: fighter.serial,
        kind: fighter.kind,
        object: ssb_psp_runtime::scene::fighter_object(p, kind).unwrap_or(u32::MAX),
        slot: None,
        skips_scales: fighter
            .status
            .is_some_and(|s| s.skips_translate_scales(fighter.kind)),
        skeleton: Skeleton::new(),
    });
    if let Some(anim) = p.fighter_anim(kind, slot) {
        m.skeleton.start(p, &anim, 0.0, 1.0);
        m.slot = Some(slot);
        play(p, &mut m);
    }
    m
}

/// One frame of the clip, through the fighter's translation scales unless
/// its row skips them (Luigi's own demo rows).
fn play(p: &Pack<'_>, m: &mut Model) {
    let Some(anim) = m.slot.and_then(|slot| p.fighter_anim(m.kind as u32, slot)) else {
        return;
    };
    let Some(script) = p.anim_script(&anim) else {
        return;
    };
    let scales = if m.skips_scales {
        None
    } else {
        p.fighter_translate_scales(m.kind as u32)
    };
    let first_node = p.object(m.object).map_or(0, |o| o.first_node);
    let _ = m.skeleton.tick_scaled(script, scales, first_node);
}

/// How one slot's fighter draws: where, in which costume, and the colour
/// animation's `color1` blend, if any.
#[derive(Clone, Copy)]
pub struct Shown {
    pub fighter: layer::Fighter,
    pub position: [f32; 3],
    pub costume: u8,
    pub tint: Option<[u8; 4]>,
}

/// The VS screen back to front (`PlayersVs::visit`). The CPU's colour is
/// not drawn here (RE-411).
pub unsafe fn draw_all(
    gpu: &mut Gpu,
    p: &Pack<'_>,
    draw_state: &mut meshdraw::DrawState,
    select: &PlayersVs,
    f: &Fighters,
) {
    let shown: [Option<Shown>; 4] = core::array::from_fn(|i| {
        select.view.slots[i].fighter.map(|fighter| Shown {
            fighter,
            position: layer::Fighter::position(i),
            costume: select.slots[i].costume,
            tint: None,
        })
    });
    draw_screen(gpu, p, draw_state, &|g| select.visit(g), &shown, Some(f));
}

/// `dGMColScriptsFighterComPlayer`'s `SetColor1`: white at 0x30, which
/// `ftParamCheckSetFighterColAnimID(..., nGMColAnimFighterComPlayer, 0)`
/// gives the Training select's CPU.
const COM_PLAYER_TINT: [u8; 4] = [0xFF, 0xFF, 0xFF, 0x30];

/// The Training screen back to front (`FighterSelect::visit`).
pub unsafe fn draw_training(
    gpu: &mut Gpu,
    p: &Pack<'_>,
    draw_state: &mut meshdraw::DrawState,
    select: &FighterSelect,
    f: Option<&Fighters>,
) {
    use ssb_game::fighter_select::{layer as training, COM, MAN};
    let shown = |slot: usize| {
        select.view.slots[slot].fighter.map(|fighter| Shown {
            fighter,
            position: training::fighter_position(slot),
            costume: select.slots[slot].costume,
            tint: (slot == COM).then_some(COM_PLAYER_TINT),
        })
    };
    let shown = [shown(MAN), shown(COM), None, None];
    draw_screen(gpu, p, draw_state, &|g| select.visit(g), &shown, f);
}

/// A select's pieces back to front, with the fighters at `Draw::Fighters`.
#[inline(never)]
unsafe fn draw_screen(
    gpu: &mut Gpu,
    p: &Pack<'_>,
    draw_state: &mut meshdraw::DrawState,
    visit: &dyn Fn(&mut dyn FnMut(Draw)),
    shown: &[Option<Shown>; 4],
    f: Option<&Fighters>,
) {
    visit(&mut |d| match d {
        Draw::Sprite(piece) | Draw::Shadow(piece) => draw_piece(p, draw_state, &piece, None),
        Draw::Tiled { piece, size } => draw_piece(p, draw_state, &piece, Some(size)),
        Draw::Puck { piece, glow } => {
            if let Some(s) = p.sprite(piece.file, piece.offset) {
                meshdraw::draw_sprite_glow(p, &s, piece.x, piece.y, glow, draw_state);
            }
        }
        Draw::Scissor(rect) => gpu.set_viewport_n64(rect),
        Draw::Fighters => {
            if let Some(f) = f {
                draw_fighters(gpu, p, draw_state, shown, f);
            }
        }
    });
    gpu.set_viewport_fullscreen();
}

/// One `SObj` through `lbCommonDrawSObjAttr`: `SP_FASTCOPY` cleared and
/// `SP_TRANSPARENT` set where the source does; a wrapping one over `size`.
unsafe fn draw_piece(
    p: &Pack<'_>,
    draw_state: &mut meshdraw::DrawState,
    piece: &Piece,
    size: Option<[f32; 2]>,
) {
    const SP_FASTCOPY: u16 = 0x0020;
    let s = match piece.lut {
        Some(lut) => p.sprite_lut(piece.file, piece.offset, lut),
        None => p.sprite(piece.file, piece.offset),
    };
    let Some(s) = s else {
        return;
    };
    let [r, g, b, a] = s.color;
    let prim = piece.prim.map_or([r, g, b, a], |c| [c[0], c[1], c[2], a]);
    let attr = if piece.transparent {
        (s.attr & !SP_FASTCOPY) | ssb_rom::sprite::SP_TRANSPARENT
    } else {
        s.attr & !ssb_rom::sprite::SP_TRANSPARENT
    };
    let d = meshdraw::SObjDraw {
        x: piece.x,
        y: piece.y,
        scale: 1.0,
        prim,
        env: piece.env,
        solid: false,
        attr,
    };
    match size {
        Some(size) => meshdraw::draw_sprite_tiled(p, &s, &d, size, draw_state),
        None => meshdraw::draw_sprite(p, &s, &d, draw_state),
    }
}

/// The fighters under the fighter camera: each at its slot's position,
/// turned by its `rotate.y`, at `dSCSubsysFighterScales` and in the slot's
/// costume. A hidden fighter (an NA slot, or one with nothing under its
/// puck) is not drawn. A tint is `G_RM_FOG_PRIM_A`'s blend towards
/// `color1` (`ftDisplayMainCalcFogColor`), as in battle.
#[inline(never)]
unsafe fn draw_fighters(
    gpu: &mut Gpu,
    p: &Pack<'_>,
    draw_state: &mut meshdraw::DrawState,
    shown: &[Option<Shown>; 4],
    f: &Fighters,
) {
    let cam = &CAMERA;
    gpu.set_viewport_n64(cam.viewport);
    gpu.set_perspective(cam.fovy, cam.aspect, cam.near, cam.far);
    gpu.reset_modelview();
    draw_state.begin_frame();
    gpu.set_view(&ssb_engine::math::Mat4::look_at(cam.eye, cam.at, cam.up));
    for (m, shown) in f.models.iter().zip(shown.iter()) {
        let (Some(m), Some(shown)) = (m, shown) else {
            continue;
        };
        let fighter = shown.fighter;
        if fighter.hidden {
            continue;
        }
        let Some(obj) = p.object(m.object) else {
            continue;
        };
        let mut posed = [ssb_rom::scene::Mat4::IDENTITY; ssb_rom::skeleton::MAX_NODES];
        let n = m.skeleton.compose(p, &obj, &mut posed);
        gpu.model_transform(
            shown.position,
            [0.0, fighter.rotate_y, 0.0],
            meshdraw::MODEL_SCALE * fighter.scale(),
        );
        let base = gpu.model_matrix();
        draw_state.configure_fighter_light(LIGHT_ANGLE);
        if let Some(rgba) = shown.tint {
            // The view looks down -z from `CAMERA.eye`.
            gpu.set_constant_fog(cam.eye.z - shown.position[2], rgba);
        }
        meshdraw::draw_object_posed(
            p,
            &obj,
            &base,
            &posed[..n],
            None,
            draw_state,
            None,
            None,
            u32::from(shown.costume),
        );
        if shown.tint.is_some() {
            gpu.clear_fog();
        }
        draw_state.finish_fighter_light();
    }
    gpu.set_viewport_n64(layer::VIEWPORT);
}
