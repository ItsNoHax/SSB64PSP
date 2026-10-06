//! The title's attract demos (RE-465): How to Play (`scExplain`,
//! `ssb_game::explain`) and the auto demo (`scAutoDemo`,
//! `ssb_game::auto_demo`), each a battle on the session's VS world
//! ([`crate::Screen::Training`]) with its scene's own interface.
//!
//! How to Play's fighters run their input scripts ([`play::Lead::Key`] and
//! [`play::Dummy::key`]) on its own random seed; its window, captions,
//! stick, spark and overlay come from the scene's menu pack, whose material
//! animations the stick and spark play here. The auto demo's four CPUs (port
//! 0's through [`play::Lead::Computer`]) fight on the next stage of its
//! order while its focus script closes the camera in on players 1 and 2.

use alloc::boxed::Box;

use ssb_game::auto_demo::{self, AutoDemo};
use ssb_game::explain::{self, Explain};
use ssb_game::fighter::FighterKind;
use ssb_game::menu::Scene as MScene;
use ssb_game::spgame::congra::Fade;
use ssb_psp_runtime::assets::{self, AlignedBuf};
use ssb_psp_runtime::gu::Gpu;
use ssb_psp_runtime::meshdraw;
use ssb_rom::matanim::{MaterialJoint, TRACK_TEXTURE_ID_CURRENT};
use ssb_rom::menu_pack::MenuScene;
use ssb_rom::pack::Pack;

use crate::{play, Entrant, Roster, Session, VsRules};

/// The running demo.
pub(crate) enum Demo {
    Explain(Box<ExplainScene>),
    AutoDemo(Box<AutoDemoScene>),
    /// An opening scene with a battle (`ssb_game::opening::fighters`,
    /// RE-467).
    Movie(Box<MovieScene>),
}

/// An opening battle scene's logic.
pub(crate) enum MovieLogic {
    Fighter(Box<ssb_game::opening::fighters::FighterScene>),
    Jungle(Box<ssb_game::opening::fighters::Jungle>),
}

/// An opening battle scene: its logic, its scene pack (the posed
/// fighters' and the jungle's cameras) and its own cameras' state.
pub(crate) struct MovieScene {
    logic: MovieLogic,
    sprites: Option<AlignedBuf>,
    runtime: ssb_psp_runtime::movie::Runtime,
}

impl MovieScene {
    fn world(&self) -> &ssb_game::opening::movie::World {
        match &self.logic {
            MovieLogic::Fighter(f) => &f.world,
            MovieLogic::Jungle(j) => &j.world,
        }
    }

    fn camera(&self) -> Option<ssb_game::opening::fighters::MovieCamera> {
        match &self.logic {
            MovieLogic::Fighter(f) => f.camera,
            MovieLogic::Jungle(j) => Some(j.camera),
        }
    }

    fn scene_pack(&self) -> Option<Pack<'_>> {
        Pack::open(self.sprites.as_ref()?.as_slice()).ok()
    }
}

/// `scExplain`'s interface and its sprite pack.
pub(crate) struct ExplainScene {
    logic: Explain,
    sprites: AlignedBuf,
    stick: Option<MaterialJoint>,
    spark: Option<MaterialJoint>,
    /// `lbFadeMakeActor`'s fade until it ejects itself.
    fade: Option<Fade>,
    /// The game's seed, which the scene leaves alone
    /// (`syUtilsSetRandomSeedPtr(NULL)` on the way out).
    seed_outside: i32,
}

/// `scAutoDemo`'s focus and its fighters' names.
pub(crate) struct AutoDemoScene {
    logic: AutoDemo,
    fade: Option<Fade>,
    names: [FighterKind; 2],
}

/// `*(AObjEvent32***)joint`: DObj 0's MObj 0 script.
fn script(data: &[u8], joint: u32) -> u32 {
    let word = |at: u32| {
        data.get(at as usize..at as usize + 4)
            .map_or(0, |b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
    };
    word(word(joint))
}

/// The stick's and spark's `MObj` animations over the pack's copy of the
/// graphics file.
struct Anims<'a> {
    data: &'a [u8],
    stick: &'a mut Option<MaterialJoint>,
    spark: &'a mut Option<MaterialJoint>,
}

impl Anims<'_> {
    /// `gcAddAnimAll(gobj, NULL, joint, 0.0F)` and `gcPlayAnimAll`.
    fn start(&self, joint: u32) -> Option<MaterialJoint> {
        let mut j = MaterialJoint::start(script(self.data, joint), 0.0);
        j.tick(self.data, 1.0).ok()?;
        Some(j)
    }
}

impl explain::Anims for Anims<'_> {
    fn start_stick(&mut self, status: u8) {
        let joint = explain::graphics::STICK_MAT_ANIM_JOINTS[usize::from(status).min(6)];
        *self.stick = self.start(joint);
    }

    fn play_stick(&mut self) -> Option<f32> {
        let j = self.stick.as_mut()?;
        let _ = j.tick(self.data, 1.0);
        Some(j.anim_frame())
    }

    fn start_spark(&mut self) {
        *self.spark = self.start(explain::graphics::TAP_SPARK_MAT_ANIM_JOINT);
    }

    fn play_spark(&mut self) -> bool {
        let Some(j) = self.spark.as_mut() else {
            return false;
        };
        if !j.ended() {
            let _ = j.tick(self.data, 1.0);
        }
        !j.ended()
    }
}

impl ExplainScene {
    fn pack(&self) -> Option<Pack<'_>> {
        Pack::open(self.sprites.as_slice()).ok()
    }

    /// One frame of the scene's processes, after the battle's.
    fn tick(&mut self, tapped: bool) -> explain::Tick {
        let Ok(pack) = Pack::open(self.sprites.as_slice()) else {
            return explain::Tick::default();
        };
        let data =
            ssb_rom::title::packed_frames(&pack, ssb_rom::explain::ANIMS_SLOT).unwrap_or(&[]);
        let mut anims = Anims {
            data,
            stick: &mut self.stick,
            spark: &mut self.spark,
        };
        self.logic.tick(tapped, &mut anims)
    }
}

/// `lbFadeProcUpdate`, then `lbFadeProcDisplay`'s alpha for a fade from
/// black (`color.a == 0`); 0 once the fade has ejected itself.
fn fade_alpha(fade: &mut Option<Fade>) -> u8 {
    let Some(f) = fade.as_mut() else {
        return 0;
    };
    if f.update() {
        *fade = None;
        return 0;
    }
    0xFF - f.alpha()
}

/// The roster's entrant on `port`, a fighter on its scene's spawn.
fn entrant(kind: FighterKind, spawn: u16, level: u8, port: u8) -> Entrant {
    Entrant {
        kind,
        // `players[].costume`: `dSCManagerDefaultBattleState`'s 0.
        costume: 0,
        level,
        handicap: ssb_game::stale::HANDICAP_DEFAULT,
        spawn,
        team: port,
        // `players[].color = player`.
        color: port,
        human: false,
    }
}

/// The demos' battle state: `dSCManagerDefaultBattleState`'s rules (every
/// item at middle appearance), no interface.
const RULES: VsRules = VsRules {
    demo: true,
    ..VsRules::DEFAULT
};

/// `scExplainStartScene` and `scExplainFuncStart`. `false` when the pack
/// or the scene's sprites are missing.
pub(crate) fn start_explain(s: &mut Session, pack: Option<&Pack<'_>>) -> bool {
    if pack.is_none() {
        return false;
    }
    let Some(sprites) = s
        .pack_path
        .and_then(|path| assets::load_menu_pack(path, MenuScene::Explain).ok())
    else {
        return false;
    };
    let (phases, keys) = {
        let Ok(ep) = Pack::open(sprites.as_slice()) else {
            return false;
        };
        let phases = ssb_rom::title::packed_frames(&ep, ssb_rom::explain::PHASES_SLOT)
            .and_then(explain::Phase::read_all);
        let keys = ssb_rom::title::packed_frames(&ep, ssb_rom::explain::KEYS_SLOT).map(|k| {
            [0, 1].map(|i| {
                ssb_game::key::parse(k, explain::KEY_EVENTS[i] as usize).unwrap_or_default()
            })
        });
        (phases, keys)
    };
    let (Some(phases), Some(keys)) = (phases, keys) else {
        return false;
    };
    // `syUtilsSetRandomSeedPtr(&dSCExplainRandomSeed1)`: the overlay's
    // seed, 1 on every load.
    let seed_outside = ssb_game::rng::seed();
    ssb_game::rng::set_seed(explain::RANDOM_SEED);
    // `scExplainSetBattleState`: Mario and Luigi on players 1 and 2's
    // spawns.
    let mut roster: Roster = [None; 4];
    for (port, kind) in explain::FIGHTERS.into_iter().enumerate() {
        roster[port] = Some(entrant(kind, port as u16, 3, port as u8));
    }
    ssb_game::item::set_explain(true);
    s.enter(pack, explain::GKIND, roster, Some(RULES));
    s.scene_gkind = explain::GKIND;
    let Some(pl) = s.play_state.as_mut() else {
        ssb_game::item::set_explain(false);
        ssb_game::rng::set_seed(seed_outside);
        return false;
    };
    // `gmCameraSetViewportDimensions(10, 10, 310, 160)`.
    pl.camera.viewport = explain::BATTLE_VIEWPORT;
    pl.camera.is_explain = true;
    let [lead_key, other_key] = keys;
    // `ftParamSetKey`.
    s.lead = play::Lead::Key(ssb_game::key::Key::new(lead_key));
    if let Some(d) = s.dummies[0].as_deref_mut() {
        d.key = Some(ssb_game::key::Key::new(other_key));
    }
    // `scExplainStartBattle`: each fighter appears where it stands, its
    // camera on the appearance, and takes control.
    for f in crate::scenes(pl, &mut s.dummies).into_iter().flatten() {
        ssb_game::appear::entry_set_status(&mut f.fighter);
        ssb_game::appear::appear_set_status(&mut f.fighter);
        f.fighter.dead.camera_mode = ssb_game::dead::CameraMode::Explain;
        f.fighter.interface.control_disable = false;
    }
    // `scExplainSetPlayerInterfacePositions`.
    for (port, d) in s.damage_hud.damage.iter_mut().enumerate() {
        *d = ssb_game::hud::DamageDisplay::at_place(
            port,
            0,
            explain::INTERFACE_POSITIONS_X[port],
            explain::INTERFACE_POSITION_Y,
            false,
        );
    }
    let mut stick = None;
    let mut spark = None;
    let logic = {
        let Ok(ep) = Pack::open(sprites.as_slice()) else {
            return false;
        };
        let data = ssb_rom::title::packed_frames(&ep, ssb_rom::explain::ANIMS_SLOT).unwrap_or(&[]);
        let mut anims = Anims {
            data,
            stick: &mut stick,
            spark: &mut spark,
        };
        Explain::new(phases, &mut anims).0
    };
    s.demo = Some(Demo::Explain(Box::new(ExplainScene {
        logic,
        sprites,
        stick,
        spark,
        fade: Some(Fade::new(explain::FADE_LENGTH)),
        seed_outside,
    })));
    s.screen = crate::Screen::Training;
    true
}

/// What the auto demo's focus script sees and moves.
struct World<'a> {
    s: &'a mut Session,
    pack: &'a Pack<'a>,
    fade: &'a mut Option<Fade>,
}

impl World<'_> {
    fn fighter(&mut self, player: usize) -> Option<&mut play::FighterScene> {
        let pl = self.s.play_state.as_mut()?;
        crate::scenes(pl, &mut self.s.dummies)
            .into_iter()
            .flatten()
            .find(|f| usize::from(f.fighter.port) == player)
    }
}

impl auto_demo::World for World<'_> {
    fn is_dead(&self, player: usize) -> bool {
        use ssb_game::status::{AnyStatus, Status};
        let Some(pl) = self.s.play_state.as_ref() else {
            return false;
        };
        crate::scenes_ref(pl, &self.s.dummies)
            .into_iter()
            .flatten()
            .find(|f| usize::from(f.fighter.port) == player)
            .is_some_and(|f| {
                matches!(
                    f.fighter.status.status,
                    AnyStatus::Common(
                        Status::DeadDown | Status::DeadLeftRight | Status::DeadUpStar
                    )
                )
            })
    }

    fn set_level(&mut self, player: usize, level: u8) {
        if player == 0 {
            if let play::Lead::Computer(c) = &mut self.s.lead {
                c.level = level;
            }
        } else if let Some(d) = self
            .s
            .dummies
            .get_mut(player - 1)
            .and_then(|d| d.as_deref_mut())
        {
            d.computer.level = level;
        }
    }

    fn zoom(&mut self, player: usize, eye: (f32, f32)) {
        let pack = self.pack;
        let Some((kind, pos, cam_offset_y)) = self
            .fighter(player)
            .map(|f| (f.fighter.kind, f.fighter.pos, f.cam_offset_y))
        else {
            return;
        };
        let dist = pack
            .fighter(kind as u32)
            .map_or(1000.0, |d| d.closeup_camera_zoom);
        if let Some(pl) = self.s.play_state.as_mut() {
            pl.demo_zoom = Some(ssb_psp_runtime::scene::DemoZoom {
                port: player as u8,
                eye,
                dist,
                pan_scale: auto_demo::ZOOM_PAN_SCALE,
                fov: auto_demo::ZOOM_FOV,
                pos,
                cam_offset_y,
            });
        }
    }

    fn reset_camera(&mut self) {
        if let Some(pl) = self.s.play_state.as_mut() {
            pl.demo_zoom = None;
        }
    }

    fn set_detail_high(&mut self, player: usize, high: bool) {
        use ssb_game::modelpart::Detail;
        let detail = if high { Detail::High } else { Detail::Low };
        if let Some(f) = self.fighter(player) {
            f.fighter.model_parts.set_detail_all(detail);
            f.fighter.model_parts.detail_base = detail;
        }
    }

    fn fade(&mut self) {
        *self.fade = Some(Fade::new(auto_demo::FADE_LENGTH));
    }

    fn rand(&mut self, range: i32) -> i32 {
        ssb_game::rng::rand_int_range(range)
    }
}

/// `scAutoDemoStartScene` and `scAutoDemoFuncStart`. `false` when the pack
/// is missing.
pub(crate) fn start_auto_demo(s: &mut Session, pack: Option<&Pack<'_>>) -> bool {
    let Some(p) = pack else { return false };
    // `scAutoDemoInitDemo`, then each player's start point in turn.
    let mut rand = ssb_game::rng::rand_int_range;
    let setup = auto_demo::init(&mut s.menus.demo, &s.backup, &mut rand);
    let points = auto_demo::start_points(&mut rand);
    let mut roster: Roster = [None; 4];
    for port in 0..4 {
        roster[port] = Some(entrant(
            setup.fkinds[port],
            points[port],
            auto_demo::LEVEL,
            port as u8,
        ));
    }
    s.enter(pack, setup.gkind, roster, Some(RULES));
    s.scene_gkind = setup.gkind;
    let Some(stage) = p.stage(s.training_stage) else {
        return false;
    };
    let Some(pl) = s.play_state.as_mut() else {
        return false;
    };
    // `desc.damage = players[].stock_damage_all`.
    for f in crate::scenes(pl, &mut s.dummies).into_iter().flatten() {
        let port = usize::from(f.fighter.port);
        f.fighter.damage = setup.damage[port];
        s.damage_hud.damage[port] =
            ssb_game::hud::DamageDisplay::new(port, i32::from(setup.damage[port]));
    }
    // A VS CPU on port 0 too: level 9, the default behaviour and trait.
    s.lead = play::Lead::Computer(Box::new(play::lead_computer(
        p,
        &stage,
        pl,
        auto_demo::LEVEL,
        ssb_game::computer::Behavior::Default,
        ssb_game::computer::attack::Trait::Default,
    )));
    let names = [setup.fkinds[0], setup.fkinds[1]];
    let mut fade = None;
    let logic = {
        let mut world = World {
            s,
            pack: p,
            fade: &mut fade,
        };
        AutoDemo::new(&mut world).0
    };
    s.demo = Some(Demo::AutoDemo(Box::new(AutoDemoScene {
        logic,
        fade,
        names,
    })));
    s.screen = crate::Screen::Training;
    true
}

/// `scAutoDemoFuncRun`, at the start of the frame's `gcRunAll`. Returns
/// the scene the demo leaves for.
pub(crate) fn before_world(
    s: &mut Session,
    pack: Option<&Pack<'_>>,
    tapped: bool,
) -> Option<MScene> {
    if matches!(s.demo, Some(Demo::Movie(_))) {
        return movie_before_world(s, pack, tapped);
    }
    let p = pack?;
    if !matches!(s.demo, Some(Demo::AutoDemo(_))) {
        return None;
    }
    let Some(Demo::AutoDemo(mut d)) = s.demo.take() else {
        return None;
    };
    let load = {
        let mut world = World {
            s,
            pack: p,
            fade: &mut d.fade,
        };
        d.logic.tick(tapped, &mut world)
    };
    s.demo = Some(Demo::AutoDemo(d));
    load
}

/// The scene's processes after the battle's: How to Play's stick, spark
/// and phases, then either demo's fade. Returns the scene the demo leaves
/// for.
pub(crate) fn after_world(s: &mut Session, tapped: bool) -> Option<MScene> {
    let mut load = None;
    let alpha = match s.demo.as_mut()? {
        Demo::Explain(e) => {
            let tick = e.tick(tapped);
            if tick.fire_flower {
                // `scExplainTryMakeFireFlower`.
                s.items.make_setup_common(
                    explain::FIRE_FLOWER_KIND,
                    None,
                    explain::FIRE_FLOWER_POS,
                    explain::FIRE_FLOWER_VEL,
                    &core::iter::empty,
                );
            }
            load = tick.load;
            fade_alpha(&mut e.fade)
        }
        Demo::AutoDemo(d) => fade_alpha(&mut d.fade),
        Demo::Movie(_) => 0,
    };
    s.damage_hud.demo_fade_alpha = alpha;
    load
}

/// Whether the demo's interface shows the magnifiers
/// (`gIFCommonPlayerInterface.is_magnify_display`).
pub(crate) fn magnify_display(demo: &Demo) -> bool {
    match demo {
        Demo::Explain(_) => true,
        Demo::AutoDemo(d) => d.logic.magnify_display,
        Demo::Movie(_) => false,
    }
}

/// Leaves the demo: the seed and game type it set are put back, and the
/// battle goes with it.
pub(crate) fn leave(s: &mut Session) {
    if let Some(Demo::Explain(e)) = s.demo.take() {
        ssb_game::rng::set_seed(e.seed_outside);
    }
    ssb_game::item::set_explain(false);
    s.damage_hud.movie = false;
    s.damage_hud.movie_flat48 = false;
    s.lead = play::Lead::Pad;
    s.play_state = None;
    s.dummies = Default::default();
    s.vs_battle = None;
}

/// A rectangle in N64 screen pixels as the PSP's 2D rectangle.
fn psp_rect([x0, y0, x1, y1]: [f32; 4]) -> [f32; 4] {
    let (vx, _, _, vh) = ssb_engine::coord::pillarboxed_viewport();
    let k = vh as f32 / ssb_engine::coord::N64_SCREEN.1 as f32;
    [vx as f32 + x0 * k, y0 * k, vx as f32 + x1 * k, y1 * k]
}

/// `lbCommonDrawSObjAttr` for one of the scene's `SObj`s.
unsafe fn draw_sobj(
    p: &Pack<'_>,
    draw_state: &mut meshdraw::DrawState,
    file: u32,
    at: u32,
    x: f32,
    y: f32,
    attr: Option<u16>,
) {
    let Some(sprite) = p.sprite(file, at) else {
        return;
    };
    let d = meshdraw::SObjDraw {
        x,
        y,
        scale: 1.0,
        prim: sprite.color,
        env: [0; 3],
        solid: false,
        attr: attr.unwrap_or(sprite.attr & !ssb_rom::sprite::SP_HIDDEN),
    };
    meshdraw::draw_sprite(p, &sprite, &d, draw_state);
}

/// A material's texture over `rect` (N64 pixels), modulated by `prim`.
unsafe fn draw_quad(
    p: &Pack<'_>,
    draw_state: &mut meshdraw::DrawState,
    at: u32,
    rect: [f32; 4],
    prim: [u8; 4],
    mirror: bool,
) {
    let Some(sprite) = p.sprite(explain::FILE_GRAPHICS, at) else {
        return;
    };
    let [x0, y0, x1, y1] = psp_rect(rect);
    meshdraw::draw_particle_rect(
        p,
        sprite.texture,
        &meshdraw::ParticleRect {
            x0,
            y0,
            x1,
            y1,
            flip_s: false,
            flip_t: false,
            mirror_s: mirror,
            mirror_t: mirror,
            prim,
            env: None,
            alpha_ref: None,
            depth: None,
        },
        draw_state,
    );
}

/// The frame of a texture-id track.
fn texture_id(j: Option<&MaterialJoint>) -> usize {
    j.and_then(|j| j.track_value(TRACK_TEXTURE_ID_CURRENT))
        .map_or(0, |v| v as usize)
}

/// The demo's own cameras over the battle: How to Play's window
/// (`scExplainWindowProcDisplay`), text camera (link 26) and stick camera
/// (link 27); the auto demo's names (link 23).
pub(crate) unsafe fn draw(
    gpu: &mut Gpu,
    pack: Option<&Pack<'_>>,
    draw_state: &mut meshdraw::DrawState,
    demo: &Demo,
) {
    use ssb_rom::sprite::{SP_TEXSHUF, SP_TRANSPARENT};
    draw_state.kind48_flat = false;
    gpu.set_viewport_pillarboxed();
    match demo {
        Demo::Explain(e) => {
            let Some(p) = e.pack() else { return };
            let l = &e.logic;
            gpu.set_viewport_n64(explain::WINDOW);
            meshdraw::fill_rect_n64(explain::WINDOW, [0, 0, 0, 0xFF], draw_state);
            // `scExplainUpdateTextBoxSprite`'s attributes.
            if let Some((at, x, y)) = l.textbox {
                draw_sobj(
                    &p,
                    draw_state,
                    explain::FILE_GRAPHICS,
                    at,
                    x,
                    y,
                    Some(SP_TEXSHUF | SP_TRANSPARENT),
                );
            }
            for (&at, sobj) in explain::sprite::PHASE.iter().zip(l.sobjs) {
                if let Some((x, y)) = sobj {
                    draw_sobj(&p, draw_state, explain::FILE_GRAPHICS, at, x, y, None);
                }
            }
            gpu.set_viewport_n64(ssb_game::camera::BATTLE_VIEWPORT);
            use explain::graphics as g;
            if l.stick.shown {
                let (x, y) = (l.stick.x, l.stick.y);
                let id = texture_id(e.stick.as_ref()).min(g::STICK_TEXTURES.len() - 1);
                let h = g::STICK_HALF;
                draw_quad(
                    &p,
                    draw_state,
                    g::STICK_TEXTURES[id],
                    [x - h, y - h, x + h, y + h],
                    [0xFF; 4],
                    false,
                );
                if l.stick.arrows {
                    // The child's triangles, y up in the stick camera.
                    meshdraw::fill_triangles_n64(
                        &g::ARROWS.map(|t| t.map(|[ax, ay]| [x + ax, y - ay])),
                        g::ARROW_COLOR,
                        draw_state,
                    );
                }
            }
            if let Some((x, y)) = l.spark {
                let id = texture_id(e.spark.as_ref()).min(g::SPARK_TEXTURES.len() - 1);
                let h = g::SPARK_HALF;
                draw_quad(
                    &p,
                    draw_state,
                    g::SPARK_TEXTURES[id],
                    [x - h, y - h, x + h, y + h],
                    g::SPARK_COLOR,
                    true,
                );
            }
            if let Some((x, y)) = l.rgb {
                let [x0, y0, x1, y1] = g::RGB_RECT;
                // The quad's y is up: its top edge is +18.
                draw_quad(
                    &p,
                    draw_state,
                    ssb_rom::explain::RGB_TEXTURE,
                    [x + x0, y - y1, x + x1, y - y0],
                    [0xFF, 0xFF, 0xFF, g::RGB_ALPHA],
                    false,
                );
            }
        }
        Demo::Movie(m) => {
            if let Some(p) = pack {
                // The cameras after the battle camera (priority 50): the
                // posed panel's.
                draw_movie_cameras(gpu, p, draw_state, m, |priority| priority < 50);
            }
        }
        Demo::AutoDemo(d) => {
            let Some(p) = pack else { return };
            gpu.set_viewport_n64(ssb_game::camera::BATTLE_VIEWPORT);
            for (shown, kind) in d.logic.names.iter().zip(d.names) {
                if !shown {
                    continue;
                }
                let Some(&at) = ssb_rom::campaign::NAMES.offsets.get(kind as usize) else {
                    continue;
                };
                let Some(sprite) = p.sprite(ssb_rom::campaign::NAMES.file, at) else {
                    continue;
                };
                // `scAutoDemoInitSObjs`: white, centred on (160, 50).
                let [cx, cy] = auto_demo::NAME_CENTRE;
                let x = (cx - f32::from(sprite.width) * 0.5) as i32 as f32;
                let y = (cy - f32::from(sprite.height) * 0.5) as i32 as f32;
                let mut s = sprite;
                s.color = [0xFF, 0xFF, 0xFF, sprite.color[3]];
                let draw = meshdraw::SObjDraw {
                    x,
                    y,
                    scale: 1.0,
                    prim: s.color,
                    env: [0; 3],
                    solid: false,
                    attr: SP_TEXSHUF | SP_TRANSPARENT,
                };
                meshdraw::draw_sprite(p, &s, &draw, draw_state);
            }
        }
    }
    gpu.set_viewport_pillarboxed();
}

/// The movie scene's own cameras whose priority `keep` admits.
///
/// # Safety
///
/// Between `begin_frame` and `end_frame`.
pub(crate) unsafe fn draw_movie_cameras(
    gpu: &mut Gpu,
    p: &Pack<'_>,
    draw_state: &mut meshdraw::DrawState,
    m: &MovieScene,
    keep: impl Fn(u32) -> bool,
) {
    let mut world = m.world().clone();
    world.cameras.retain(|c| keep(c.priority));
    let scene = m.scene_pack();
    let models = ssb_psp_runtime::movie::models();
    let a = ssb_psp_runtime::movie::Assets {
        main: p,
        scene: scene.as_ref(),
        models: models.as_ref(),
    };
    m.runtime
        .draw(&world, gpu, draw_state, &a, &mut |_, _, _, _, _| {});
}

/// The movie scene's picture before its battle exists: its own cameras
/// (the name's).
///
/// # Safety
///
/// Between `begin_frame` and `end_frame`.
pub(crate) unsafe fn draw_movie_only(
    gpu: &mut Gpu,
    pack: Option<&Pack<'_>>,
    draw_state: &mut meshdraw::DrawState,
    demo: &Demo,
) {
    gpu.set_viewport_fullscreen();
    gpu.begin_frame(Some(ssb_engine::renderer::Color::rgba(0, 0, 0, 0xFF)));
    gpu.set_viewport_pillarboxed();
    if let (Some(p), Demo::Movie(m)) = (pack, demo) {
        draw_movie_cameras(gpu, p, draw_state, m, |_| true);
    }
    gpu.set_viewport_fullscreen();
}

/// `mvOpening*StartScene`: an opening battle scene. `false` when its data
/// is missing.
pub(crate) fn start_movie(
    s: &mut Session,
    pack: Option<&Pack<'_>>,
    kind: ssb_game::opening::Kind,
) -> bool {
    use ssb_game::opening::fighters::{params, FighterScene, Jungle};
    let Some(p) = pack else { return false };
    let logic = match kind {
        ssb_game::opening::Kind::Jungle => MovieLogic::Jungle(Box::new(Jungle::new())),
        _ => match params(kind) {
            Some(params) => MovieLogic::Fighter(Box::new(FighterScene::new(params))),
            None => return false,
        },
    };
    let scene = if kind == ssb_game::opening::Kind::Jungle {
        MenuScene::OpeningJungle
    } else {
        MenuScene::OpeningFighters
    };
    let sprites = s
        .pack_path
        .and_then(|path| assets::load_menu_pack(path, scene).ok());
    leave(s);
    let mut m = Box::new(MovieScene {
        logic,
        sprites,
        runtime: ssb_psp_runtime::movie::Runtime::new(),
    });
    // `mvOpeningJungleFuncStart` makes its battle at once.
    let battle = match &mut m.logic {
        MovieLogic::Jungle(j) => j.take_battle(),
        MovieLogic::Fighter(_) => None,
    };
    s.demo = Some(Demo::Movie(m));
    s.screen = crate::Screen::Training;
    if let Some(b) = battle {
        make_battle(s, p, &b);
        movie_after_world(s, Some(p));
    }
    true
}

/// `mvOpening*MakeMotionWindow` / `mvOpeningJungleMakeFighters`: the stage,
/// the fighters on their map objects with their input scripts, and the
/// movie camera.
fn make_battle(s: &mut Session, p: &Pack<'_>, b: &ssb_game::opening::fighters::Battle) {
    let mut roster: Roster = [None; 4];
    for (port, pl) in b.players.iter().enumerate() {
        // `dSCManagerDefaultBattleState`'s players: costume 0, level 3.
        roster[port] = Some(entrant(pl.kind, pl.spawn, 3, port as u8));
    }
    s.enter(Some(p), b.gkind, roster, Some(RULES));
    s.scene_gkind = b.gkind;
    s.damage_hud.movie = true;
    // `mvOpeningSamusMakeMotionWindow`: Zebes' layer-1 kind-48 matrices
    // become kind 37 (its only opening scene).
    s.damage_hud.movie_flat48 = b.gkind == ssb_game::opening::fighters::gkind::ZEBES;
    s.damage_hud.ko.flash_disabled = !b.screen_flash || !s.backup.is_allow_screenflash;
    let Some(stage) = p.stage(s.training_stage) else {
        return;
    };
    let Some(pl) = s.play_state.as_mut() else {
        return;
    };
    for (port, f) in crate::scenes(pl, &mut s.dummies).into_iter().enumerate() {
        let (Some(f), Some(desc)) = (f, b.players.get(port)) else {
            continue;
        };
        // `mpCollisionGetMapObjPositionID`, then the scene's offset;
        // `nFTPlayerKindKey` stands where it is made
        // (`mpCommonSetFighterWaitOrFall`).
        if let Some(pt) = p.spawn(&stage, desc.spawn) {
            f.fighter.pos =
                ssb_engine::math::Vec3::new(f32::from(pt.x), f32::from(pt.y), 0.0) + desc.offset;
        }
        f.fighter
            .init_floor(ssb_psp_runtime::scene::FloorSegments::new(p, &stage));
        ssb_game::status::set_wait_or_fall(&mut f.fighter);
        f.fighter.facing = if desc.lr < 0 {
            ssb_game::fighter::Facing::Left
        } else {
            ssb_game::fighter::Facing::Right
        };
        f.fighter.damage = desc.damage;
        s.damage_hud.damage[port] = ssb_game::hud::DamageDisplay::new(port, i32::from(desc.damage));
        match (desc.kind, desc.charge) {
            (FighterKind::Donkey, Some(c)) => f.fighter.donkey_special_n.charge_level = c,
            (FighterKind::Samus, Some(c)) => f.fighter.samus.charge_level = c,
            _ => {}
        }
        f.fighter.interface.control_disable = false;
    }
    for (port, desc) in b.players.iter().enumerate() {
        let key = ssb_game::key::Key::new(desc.keys.to_vec());
        if port == 0 {
            s.lead = play::Lead::Key(key);
        } else if let Some(d) = s.dummies.get_mut(port - 1).and_then(|d| d.as_deref_mut()) {
            d.key = Some(key);
        }
    }
}

/// The scene's `func_run`, before the battle's processes: the music
/// clock's tic, then the scene's tick. Returns the scene to load.
fn movie_before_world(s: &mut Session, pack: Option<&Pack<'_>>, tapped: bool) -> Option<MScene> {
    let p = pack?;
    s.opening.clock.retrace();
    let Some(Demo::Movie(mut m)) = s.demo.take() else {
        return None;
    };
    let tick = match &mut m.logic {
        MovieLogic::Fighter(f) => {
            // `MoviePlayer1`'s position on the scene's stage.
            let spawn = ssb_psp_runtime::scene::common_stage_index(p, f.params.gkind)
                .and_then(|i| p.stage(i))
                .and_then(|stage| p.spawn(&stage, ssb_game::opening::fighters::MOVIE_PLAYER1))
                .map_or(ssb_engine::math::Vec3::ZERO, |pt| {
                    ssb_engine::math::Vec3::new(f32::from(pt.x), f32::from(pt.y), 0.0)
                });
            f.tick(tapped, spawn)
        }
        MovieLogic::Jungle(j) => j.tick(tapped),
    };
    s.demo = Some(Demo::Movie(m));
    if let Some(b) = tick.battle.as_ref() {
        make_battle(s, p, b);
    }
    tick.exit.map(|e| match e {
        ssb_game::opening::Exit::Title => MScene::Title,
        ssb_game::opening::Exit::Next(k) => MScene::Opening(k),
    })
}

/// After the battle's processes: the movie camera into the battle camera
/// and the posed panel's state.
pub(crate) fn movie_after_world(s: &mut Session, pack: Option<&Pack<'_>>) {
    let Some(p) = pack else { return };
    let Some(Demo::Movie(m)) = s.demo.as_mut() else {
        return;
    };
    let light = p.stage(s.training_stage).map(|st| st.light_angle_xy);
    if let MovieLogic::Fighter(f) = &mut m.logic {
        if let Some(l) = light {
            f.world.light = l;
        }
    }
    let cam = m.camera();
    {
        let scene = m
            .sprites
            .as_ref()
            .and_then(|b| Pack::open(b.as_slice()).ok());
        let models = ssb_psp_runtime::movie::models();
        let a = ssb_psp_runtime::movie::Assets {
            main: p,
            scene: scene.as_ref(),
            models: models.as_ref(),
        };
        let world = match &m.logic {
            MovieLogic::Fighter(f) => &f.world,
            MovieLogic::Jungle(j) => &j.world,
        };
        m.runtime.sync(world, &a);
        if let (Some(c), Some(pl)) = (cam, s.play_state.as_mut()) {
            let (mut eye, mut at, mut up_x, mut fovy) = (c.eye, c.at, c.up_x, c.fovy);
            if let Some(anim) = c.anim {
                if let Some(fr) = a.camera(anim.slot).and_then(|b| {
                    ssb_rom::camanim::frame_at(b, (anim.plays as usize).saturating_sub(1))
                }) {
                    eye = ssb_engine::math::Vec3::new(fr[0], fr[1], fr[2]);
                    at = ssb_engine::math::Vec3::new(fr[3], fr[4], fr[5]);
                    up_x = fr[6];
                    fovy = fr[7];
                }
            }
            pl.camera.eye = eye;
            pl.camera.at = at;
            pl.camera.fovy_degrees = fovy;
            pl.camera.movie = Some(ssb_game::camera::MovieView {
                viewport: c.viewport,
                aspect: c.aspect,
                near: c.near,
                far: c.far,
                up_x,
            });
        }
    }
}
