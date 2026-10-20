//! The character selects' drawing: VS (RE-411), Training, the 1P Game and
//! the Bonus Practice select.
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
use ssb_game::players_1p::Players1P;
use ssb_game::players_1p_bonus::Players1PBonus;
use ssb_game::players_vs::layer::{self, Draw, Piece, SpotlightDraw};
use ssb_game::players_vs::PlayersVs;
use ssb_game::results_scene::Camera;
use ssb_psp_runtime::gu::Gpu;
use ssb_psp_runtime::meshdraw;
use ssb_rom::pack::Pack;
use ssb_rom::skeleton::Skeleton;

/// `mnPlayersVSMakeFighterCamera`: eye (0, 0, 5000) on the origin, with
/// every camera's default projection (`Camera::persp_default`, RE-409).
const CAMERA: Camera = Camera::persp_default(
    Vec3 {
        x: 0.0,
        y: 0.0,
        z: 5000.0,
    },
    Vec3::ZERO,
    layer::VIEWPORT,
);

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
    /// The demo fighter: its status script's parts (RE-426), sounds and
    /// effects.
    demo: ssb_game::demo_fighter::DemoFighter,
}

/// The four slots' poses, the scene's particle world
/// (`efParticleInitAll`, `efManagerInitEffects`) and the spotlight model.
pub struct Fighters {
    models: [Option<Box<Model>>; 4],
    world: crate::demo_world::DemoWorld,
    /// `llMNPlayersSpotlightDObjDesc`'s packed object, looked up on the
    /// first tick with a pack (`object_keyed` scans every object).
    spotlight: Option<Option<ssb_rom::pack::ObjectDesc>>,
}

/// Boxed so the session holds a pointer.
#[inline(never)]
pub fn start() -> Box<Fighters> {
    Box::new(Fighters {
        models: [None, None, None, None],
        world: crate::demo_world::DemoWorld::new(),
        spotlight: None,
    })
}

/// One tick of the fighters, after [`PlayersVs::tick`].
pub fn tick(pack: Option<&Pack<'_>>, select: &PlayersVs, f: &mut Fighters) {
    let fighters: [Option<Placed>; 4] =
        core::array::from_fn(|i| {
            select.view.slots[i]
                .fighter
                .map(|f| (f, layer::Fighter::position(i), select.slots[i].costume))
        });
    tick_models(pack, &fighters, f);
}

/// One tick of the Training select's two fighters, after
/// [`FighterSelect::tick`].
pub fn tick_training(pack: Option<&Pack<'_>>, select: &FighterSelect, f: &mut Fighters) {
    use ssb_game::fighter_select::{layer as training, COM, MAN};
    let v = &select.view.slots;
    let placed = |slot: usize| {
        v[slot]
            .fighter
            .map(|f| (f, training::fighter_position(slot), select.slots[slot].costume))
    };
    tick_models(pack, &[placed(MAN), placed(COM), None, None], f);
}

/// One tick of the 1P select's fighter, after [`Players1P::tick`].
pub fn tick_1p(pack: Option<&Pack<'_>>, select: &Players1P, f: &mut Fighters) {
    let placed = select
        .view
        .fighter
        .map(|f| (f, ssb_game::players_1p::layer::FIGHTER_POSITION, select.slot.costume));
    tick_models(pack, &[placed, None, None, None], f);
}

/// One tick of the Bonus Practice select's fighter, after
/// [`Players1PBonus::tick`]. The poses start from [`start`].
pub fn tick_bonus(pack: Option<&Pack<'_>>, select: &Players1PBonus, f: &mut Fighters) {
    let placed = select
        .view
        .fighter
        .map(|f| (f, ssb_game::players_1p_bonus::layer::FIGHTER_POSITION, select.slot.costume));
    tick_models(pack, &[placed, None, None, None], f);
}

/// A slot's fighter, where its TopN stands and its costume
/// (`desc.costume`: the slot's, already a costume ID).
type Placed = (layer::Fighter, [f32; 3], u8);

/// Link 0's particle `func_run`s, then each fighter's
/// `scSubsysFighterProcUpdate` (priority 5): a fighter made again or given
/// its status starts its clip over (`ftMainSetStatus` plays the first
/// frame at once and runs the script's time-zero events); otherwise the
/// clip plays on and the script and colour animation run. The effects the
/// scripts queued are made on their joints; then the effect processes
/// (priority 3).
#[inline(never)]
fn tick_models(pack: Option<&Pack<'_>>, fighters: &[Option<Placed>; 4], f: &mut Fighters) {
    let Some(p) = pack else {
        return;
    };
    if f.spotlight.is_none() {
        f.spotlight = Some(ssb_psp_runtime::scene::object_keyed(p, layer::SPOTLIGHT_DESC));
    }
    f.world.run(p);
    for (port, (model, placed)) in f.models.iter_mut().zip(fighters.iter()).enumerate() {
        let Some((fighter, pos, costume)) = *placed else {
            if let Some(mut m) = model.take() {
                f.world.destroy(p, &mut m.demo);
            }
            continue;
        };
        let fresh = model
            .as_ref()
            .is_none_or(|m| m.serial != fighter.serial || m.kind != fighter.kind);
        if fresh {
            // `ftManagerDestroyFighter` before `ftManagerMakeFighter` (and
            // `ftMainSetStatus`'s stop, which a status set here remakes).
            if let Some(old) = model.as_deref_mut() {
                f.world.destroy(p, &mut old.demo);
            }
            *model = Some(make_model(p, fighter, port as u8, costume));
        } else if let Some(m) = model.as_deref_mut() {
            play(p, m);
            m.demo.tick(m.skeleton.frame());
        }
        if let Some(m) = model.as_deref_mut() {
            let stand = crate::demo_world::Stand {
                pos,
                yaw: fighter.rotate_y,
                scale: fighter.scale(),
            };
            f.world.make(p, m.object, &m.skeleton, stand, &mut m.demo);
        }
    }
    f.world.process(p);
}

/// The fighter's clip from its first frame: `Wait` for
/// `nFTDemoStatusNull` (submotion row 0 is each fighter's `Wait` figatree),
/// else the demo status's.
#[inline(never)]
fn make_model(p: &Pack<'_>, fighter: layer::Fighter, port: u8, costume: u8) -> Box<Model> {
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
        // The select motions are unflagged: the `animlock` parts stay
        // frozen (RE-509).
        skeleton: ssb_psp_runtime::scene::fighter_skeleton(fighter.kind, 1.0),
        // `desc.costume`; only the draw reads it, from the select.
        demo: ssb_game::demo_fighter::DemoFighter::new(fighter.kind, port, costume),
    });
    if let Some(anim) = p.fighter_anim(kind, slot) {
        m.skeleton.start(p, &anim, 0.0, 1.0);
        m.slot = Some(slot);
        play(p, &mut m);
    }
    // `ftMainSetStatus` runs the script's time-zero events after the
    // clip's first frame.
    m.demo.set_status(fighter.status.map_or(0, |s| 1 + s.index()));
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
    let spots = select.spotlights();
    draw_screen(gpu, p, draw_state, &|g| select.visit(g), &shown, &spots, Some(f));
}

/// `dGMColScriptsFighterComPlayer`'s `SetColor1`: white at 0x30, which
/// `ftParamCheckSetFighterColAnimID(..., nGMColAnimFighterComPlayer, 0)`
/// gives the Training select's CPU.
fn com_player_tint() -> Option<[u8; 4]> {
    ssb_game::colanim::start_color(ssb_game::colanim::ColAnimId::FIGHTER_COM_PLAYER)
}

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
            tint: if slot == COM { com_player_tint() } else { None },
        })
    };
    let shown = [shown(MAN), shown(COM), None, None];
    let spots = select.spotlights();
    draw_screen(gpu, p, draw_state, &|g| select.visit(g), &shown, &spots, f);
}

/// The 1P select back to front (`Players1P::visit`), with the backup's
/// records for the fighter under the puck.
pub unsafe fn draw_1p(
    gpu: &mut Gpu,
    p: &Pack<'_>,
    draw_state: &mut meshdraw::DrawState,
    select: &Players1P,
    backup: &ssb_game::spgame::Backup,
    f: Option<&Fighters>,
) {
    use ssb_game::players_1p::layer::Draw as One;
    let shown = select.fighter_draw().map(|(fighter, position, _, costume)| Shown {
        fighter,
        position,
        costume,
        tint: None,
    });
    let shown = [shown, None, None, None];
    let spots = select.spotlights();
    let records = Players1P::records(backup, select.force_puck_fighter_kind());
    select.visit(&records, |d| match d {
        One::Select(d) => draw_select_piece(gpu, p, draw_state, d, &shown, &spots, f),
        One::Fill { rect, color } => meshdraw::fill_rect_n64(rect, color, draw_state),
        One::MirroredT { piece, size } => {
            if let Some((s, d)) = sobj_draw(p, &piece) {
                meshdraw::draw_sprite_mirror_t(p, &s, &d, size, draw_state);
            }
        }
        One::Stock { kind, costume, x, y } => {
            // `fp->attr->sprites->stock_sprite` through `stock_luts`.
            if let Some(s) = p.fighter_sprite(kind as u8, ssb_rom::pack::SpriteDesc::ROLE_STOCK, costume) {
                let d = meshdraw::SObjDraw {
                    x,
                    y,
                    scale: 1.0,
                    prim: s.color,
                    env: [0; 3],
                    solid: false,
                    attr: s.attr | ssb_rom::sprite::SP_TRANSPARENT,
                };
                meshdraw::draw_sprite(p, &s, &d, draw_state);
            }
        }
    });
    gpu.set_viewport_pillarboxed();
}

/// The Bonus Practice select back to front (`Players1PBonus::visit`), with
/// the backup's records for the practice shown. A sprite is looked up in
/// `menu`, the scene's menu pack (the titles and records' labels the
/// resident pack lacks, `players_1p_bonus::layer::MENU_SPRITES`), first,
/// then in the resident `p`.
pub unsafe fn draw_bonus(
    gpu: &mut Gpu,
    p: &Pack<'_>,
    menu: Option<&Pack<'_>>,
    draw_state: &mut meshdraw::DrawState,
    select: &Players1PBonus,
    backup: &ssb_game::spgame::Backup,
    f: Option<&Fighters>,
) {
    let shown = select.fighter_draw().map(|(fighter, position, _, costume)| Shown {
        fighter,
        position,
        costume,
        tint: None,
    });
    let shown = [shown, None, None, None];
    let spots = select.spotlights();
    let records = select.records(backup);
    select.visit(&records, |d| {
        let (piece, size) = match d {
            Draw::Sprite(piece) | Draw::Shadow(piece) => (piece, None),
            Draw::Tiled { piece, size } => (piece, Some(size)),
            _ => return draw_select_piece(gpu, p, draw_state, d, &shown, &spots, f),
        };
        match menu.filter(|m| has_sprite(m, &piece)) {
            Some(m) => draw_piece(m, draw_state, &piece, size),
            None => draw_piece(p, draw_state, &piece, size),
        }
    });
    gpu.set_viewport_pillarboxed();
}

/// Whether `p` holds `piece`'s sprite (through its LUT, if it names one).
fn has_sprite(p: &Pack<'_>, piece: &Piece) -> bool {
    match piece.lut {
        Some(lut) => p.sprite_lut(piece.file, piece.offset, lut).is_some(),
        None => p.sprite(piece.file, piece.offset).is_some(),
    }
}

/// A select's pieces back to front, with the fighters at `Draw::Fighters`.
#[inline(never)]
unsafe fn draw_screen(
    gpu: &mut Gpu,
    p: &Pack<'_>,
    draw_state: &mut meshdraw::DrawState,
    visit: &dyn Fn(&mut dyn FnMut(Draw)),
    shown: &[Option<Shown>; 4],
    spots: &[Option<SpotlightDraw>; 4],
    f: Option<&Fighters>,
) {
    visit(&mut |d| draw_select_piece(gpu, p, draw_state, d, shown, spots, f));
    gpu.set_viewport_pillarboxed();
}

/// One of a select's pieces.
unsafe fn draw_select_piece(
    gpu: &mut Gpu,
    p: &Pack<'_>,
    draw_state: &mut meshdraw::DrawState,
    d: Draw,
    shown: &[Option<Shown>; 4],
    spots: &[Option<SpotlightDraw>; 4],
    f: Option<&Fighters>,
) {
    match d {
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
                draw_fighters(gpu, p, draw_state, shown, spots, f);
            }
        }
    }
}

/// One `SObj` through `lbCommonDrawSObjAttr`: `SP_FASTCOPY` cleared and
/// `SP_TRANSPARENT` set where the source does; a wrapping one over `size`.
unsafe fn draw_piece(
    p: &Pack<'_>,
    draw_state: &mut meshdraw::DrawState,
    piece: &Piece,
    size: Option<[f32; 2]>,
) {
    let Some((s, d)) = sobj_draw(p, piece) else {
        return;
    };
    match size {
        Some(size) => meshdraw::draw_sprite_tiled(p, &s, &d, size, draw_state),
        None => meshdraw::draw_sprite(p, &s, &d, draw_state),
    }
}

/// A piece's pack sprite and its `SObj` state.
fn sobj_draw(p: &Pack<'_>, piece: &Piece) -> Option<(ssb_rom::pack::SpriteDesc, meshdraw::SObjDraw)> {
    const SP_FASTCOPY: u16 = 0x0020;
    let s = match piece.lut {
        Some(lut) => p.sprite_lut(piece.file, piece.offset, lut),
        None => p.sprite(piece.file, piece.offset),
    }?;
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
    Some((s, d))
}

/// The fighters under the fighter camera: each at its slot's position,
/// turned by its `rotate.y`, at `dSCSubsysFighterScales` and in the slot's
/// costume. A hidden fighter (an NA slot, or one with nothing under its
/// puck) is not drawn. A tint is `G_RM_FOG_PRIM_A`'s blend towards
/// `color1` (`ftDisplayMainCalcFogColor`), as in battle. Then the shown
/// spotlights ([`draw_spotlights`]).
#[inline(never)]
unsafe fn draw_fighters(
    gpu: &mut Gpu,
    p: &Pack<'_>,
    draw_state: &mut meshdraw::DrawState,
    shown: &[Option<Shown>; 4],
    spots: &[Option<SpotlightDraw>; 4],
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
        // The parts an effect's joint query marked this frame draw from
        // their collision locals (RE-509).
        let queried = m.demo.fighter.queried_joints.get();
        let lead = m.skeleton.lead_xrotn_pose().is_some() && queried & 1 << 2 != 0;
        let n = m.skeleton.compose_queried(p, &obj, &mut posed, queried >> 4, lead);
        let base = crate::topn_base(shown.position, fighter.rotate_y, fighter.scale(), m.skeleton.locks.flagged);
        draw_state.configure_fighter_light(LIGHT_ANGLE);
        if let Some(rgba) = shown.tint {
            // The view looks down -z from `CAMERA.eye`.
            gpu.set_constant_fog(cam.eye.z - shown.position[2], rgba);
        }
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
                costume: u32::from(shown.costume),
                textures: m.demo.parts().draw_textures(),
                parts: parts.as_ref().map(|p| &p[..]),
                accessory_before: fighter.kind == ssb_game::fighter::FighterKind::Purin,
            },
        );
        if shown.tint.is_some() {
            gpu.clear_fog();
        }
        draw_state.finish_fighter_light();
    }
    if let Some(Some(object)) = f.spotlight {
        draw_spotlights(gpu, p, draw_state, &object, spots);
    }
    // DL links 10, 15 and 18 of the same camera.
    f.world.draw(p, cam, draw_state);
    gpu.set_viewport_n64(layer::VIEWPORT);
}

/// The spotlights (`mn*MakeSpotlight`, DL link 9 under the fighter camera,
/// `gcDrawDObjTreeDLLinksForGObj`), in GObj order. Their one list is on
/// task head 1 (`list_id 1`), and a camera pass runs every head-0 list
/// before its head-1 lists (RE-422), so the floor pool draws after the
/// fighters' head-0 lists whatever their GObj order. Head 1 opens with
/// `G_RM_AA_ZB_XLU_SURF` and the list sets no render mode: the pool is
/// depth-compared against the fighters' depth, writes none, and blends.
/// Deviation: links 10, 15 and 18's head-0 lists (the particles) draw
/// after it here.
unsafe fn draw_spotlights(
    gpu: &mut Gpu,
    p: &Pack<'_>,
    draw_state: &mut meshdraw::DrawState,
    object: &ssb_rom::pack::ObjectDesc,
    spots: &[Option<SpotlightDraw>; 4],
) {
    let k = meshdraw::MODEL_SCALE;
    let posed = crate::results_screen::rooted_rest_pose(p, object);
    let n = (object.node_count as usize).min(posed.len());
    for spot in spots.iter().flatten() {
        let s = spot.scale;
        // `mn*SpotlightProcUpdate` stores `scale.y` twice: z stays 1.
        gpu.model_transform_xyz(spot.translate, [0.0; 3], [s * k, s * k, k]);
        let base = gpu.model_matrix();
        meshdraw::draw_object_posed(p, object, &base, &posed[..n], None, draw_state, None, None, 0);
    }
}
