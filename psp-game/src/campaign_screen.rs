//! Packed presentation adapter for sc1pintro, mn1pcontinue and sc1pstageclear,
//! and (in `campaign_ending_screen.rs`) mvending, scstaffroll, mncongra,
//! sc1pchallenger and mnmessage. All scene/input/score clocks remain in
//! ssb-game. Demo poses live on the heap.
use alloc::{boxed::Box, vec::Vec};
use ssb_game::{
    fighter::FighterKind,
    spgame::{self, frontend::Screen, intro::FighterLight, Stage},
};
use ssb_psp_runtime::{
    gu::Gpu,
    meshdraw::{self, DrawState, SObjDraw},
};
use ssb_rom::{campaign as a, pack::Pack, skeleton::Skeleton};

#[path = "campaign_ending_screen.rs"]
mod ending;

/// The unlock message outside the 1P Game: the VS results' (`mnMessage`).
pub unsafe fn draw_message(gpu: &mut Gpu, p: &Pack<'_>, st: &mut DrawState, m: &spgame::message::Message) {
    gpu.set_viewport_n64([10.0, 10.0, 310.0, 230.0]);
    ending::draw_message(p, st, m);
    gpu.set_viewport_pillarboxed();
}

#[derive(Default)]
pub struct Presentation {
    scene: Option<(u8, u8)>,
    models: Vec<Box<Model>>,
    snapshot_pending: bool,
    /// The ending's room and camera, and the staff roll's textures.
    last: ending::LastScenes,
}
struct Model {
    kind: FighterKind,
    costume: u8,
    /// `FTDesc::shade` ([`ssb_game::shade`]).
    shade: u8,
    slot: u32,
    object: u32,
    skeleton: Skeleton,
    demo: ssb_game::demo_fighter::DemoFighter,
    entrance: spgame::intro::Entrance,
    camera: [f32; 10],
    pos: [f32; 3],
    scale: f32,
    transn: [f32; 3],
    frozen: bool,
    priority: u8,
}

impl Presentation {
    pub fn prepare_draw(
        &mut self,
        gpu: &mut Gpu,
        p: &Pack<'_>,
        screen: &Screen,
        session: &spgame::session::Session,
    ) {
        self.sync(p, screen, session);
        if self.snapshot_pending {
            gpu.capture_campaign_wallpaper();
            self.snapshot_pending = false;
        }
    }
    fn sync(&mut self, p: &Pack<'_>, screen: &Screen, session: &spgame::session::Session) -> bool {
        let data = &session.data;
        let id = match screen {
            Screen::Intro(s) => (0, s.stage as u8),
            Screen::Continue(_) => (1, data.stage),
            Screen::StageClear(_) => (2, data.stage),
            Screen::Ending(_) => (4, data.stage),
            Screen::Staffroll(_) => (5, data.stage),
            Screen::Congra(_) => (6, data.stage),
            Screen::Challenger(_) => (7, data.stage),
            // Consecutive messages are separate tasks.
            Screen::Message(m) => (8, m.unlock as u8),
            _ => (3, data.stage),
        };
        if self.scene == Some(id) {
            return false;
        }
        self.scene = Some(id);
        self.snapshot_pending = id.0 == 2;
        crate::scene_load::campaign(id.0, data.fkind as u32);
        self.models.clear();
        if ending::make(self, p, screen) {
            return true;
        }
        if let Screen::Intro(intro) = screen {
            let allies = spgame::intro::allies_num(intro.stage);
            let player_card = match allies {
                1 => 1,
                2 => 3,
                _ => 0,
            };
            for i in (0..allies).rev() {
                let ally = session.state.players[data.ally_players[i] as usize];
                let card = if allies == 1 { 2 } else { 4 + i };
                let mut camera = a::packed_camera(p, ally.fkind as usize).expect("ally camera");
                let ov = a::ALLY_CAMERAS[ally.fkind as usize][card];
                camera[..3].copy_from_slice(&ov[..3]);
                camera[4..7].copy_from_slice(&ov[3..]);
                let mut model = make_model(
                    p,
                    ally.fkind,
                    ally.costume,
                    ssb_rom::anim::SLOT_INTRO_L,
                    13,
                    0.0,
                    false,
                    camera,
                    spgame::intro::Entrance::ally(card),
                );
                model.priority = if card == 5 { 70 } else { 60 };
                // `sc1PIntroFuncStart`: `sSC1PIntroAlly*FighterDemoDesc.shade`.
                model.shade = ally.shade;
                self.models.push(model);
            }
            let mut camera = a::packed_camera(p, data.fkind as usize).expect("packed intro camera");
            let override_camera = a::ALLY_CAMERAS[data.fkind as usize][player_card];
            camera[..3].copy_from_slice(&override_camera[..3]);
            camera[4..7].copy_from_slice(&override_camera[3..]);
            let mut model = make_model(
                p,
                data.fkind,
                data.costume,
                ssb_rom::anim::SLOT_INTRO_L,
                13,
                0.0,
                false,
                camera,
                spgame::intro::Entrance::ally(player_card),
            );
            // `sSC1PIntroPlayerFighterDemoDesc.shade`.
            model.shade = session.state.players[usize::from(data.player)].shade;
            self.models.push(model);
            let (kind, index, count) = match intro.stage {
                Stage::Link => (FighterKind::Link, 12, 1),
                Stage::Yoshi => (FighterKind::Yoshi, 13, 18),
                Stage::Fox => (FighterKind::Fox, 14, 1),
                Stage::Mario => (FighterKind::Mario, 15, 1),
                Stage::Donkey => (FighterKind::Donkey, 17, 1),
                Stage::Pikachu => (FighterKind::Pikachu, 16, 1),
                Stage::Kirby => (FighterKind::Kirby, 18, 8),
                Stage::Samus => (FighterKind::Samus, 19, 1),
                Stage::MMario => (FighterKind::MetalMario, 20, 1),
                Stage::Boss => (FighterKind::Boss, 22, 1),
                Stage::Zako => {
                    // `sc1PIntroInitVSFighters`: every Polygon but Luigi
                    // and Jigglypuff, low detail, each drawn at its IntroR
                    // frames 0, 1 and 2 as the clock passes them.
                    let camera = a::packed_camera(p, 21).expect("packed opponent camera");
                    for kind in (14..=25).filter_map(FighterKind::from_ordinal) {
                        if matches!(kind, FighterKind::PolyLuigi | FighterKind::PolyPurin) {
                            continue;
                        }
                        for frame in 0..3 {
                            let mut model = make_model(
                                p,
                                kind,
                                ssb_game::costume::costume_common_id(kind, 0),
                                ssb_rom::anim::SLOT_INTRO_R,
                                14,
                                frame as f32,
                                true,
                                camera,
                                spgame::intro::Entrance::polygon(kind, frame),
                            );
                            model
                                .demo
                                .fighter
                                .model_parts
                                .set_detail_all(ssb_game::modelpart::Detail::Low);
                            self.models.push(model);
                        }
                    }
                    return true;
                }
                _ => return true,
            };
            let mut camera = a::packed_camera(p, index).expect("packed opponent camera");
            if intro.stage == Stage::Mario {
                camera[..3].copy_from_slice(&[1053.95, 404.97, 364.82]);
                camera[4..7].copy_from_slice(&[-311.21, 309.63, 332.14]);
            }
            for card in 0..count {
                let costume = if intro.stage == Stage::Yoshi {
                    card % 6
                } else {
                    ssb_game::costume::costume_common_id(
                        kind,
                        usize::from(
                            intro.stage != Stage::Donkey
                                && (kind == data.fkind
                                    && data.costume
                                        == ssb_game::costume::costume_common_id(kind, 0)
                                    || intro.stage == Stage::Mario
                                        && session.state.players[data.ally_players[0] as usize]
                                            .fkind
                                            == kind
                                        && session.state.players[data.ally_players[0] as usize]
                                            .costume
                                            == ssb_game::costume::costume_common_id(kind, 0)),
                        ),
                    )
                };
                let mut model = make_model(
                    p,
                    kind,
                    costume,
                    ssb_rom::anim::SLOT_INTRO_R,
                    14,
                    card as f32,
                    true,
                    camera,
                    spgame::intro::Entrance::opponent(intro.stage, kind, card),
                );
                if intro.stage == Stage::Yoshi {
                    // `ftParamInitAllParts(.., i % 6, shade)`: the Yoshi
                    // in the player's own colour wears shade 1
                    // (`sc1pintro.c:1276-1280`).
                    model.shade =
                        u8::from(data.fkind == FighterKind::Yoshi && data.costume == costume);
                    model
                        .demo
                        .fighter
                        .model_parts
                        .set_detail_all(ssb_game::modelpart::Detail::Low);
                }
                if intro.stage == Stage::Kirby {
                    let part = [13, 5, 10, 4, 7, 12, 6, 8][card as usize];
                    model.demo.fighter.model_parts.set(
                        6,
                        if part == 13 {
                            session.manager.kirby_model_part as i8
                        } else {
                            part
                        },
                    );
                }
                self.models.push(model);
            }
            if intro.stage == Stage::Mario {
                let mut camera = a::packed_camera(p, 15).expect("Mario Bros camera");
                camera[..3].copy_from_slice(&[1053.95, 404.97, 364.82]);
                camera[4..7].copy_from_slice(&[-311.21, 309.63, 332.14]);
                let ally = session.state.players[data.ally_players[0] as usize];
                let costume = ssb_game::costume::costume_common_id(
                    FighterKind::Luigi,
                    usize::from(
                        data.fkind == FighterKind::Luigi && data.costume == 0
                            || ally.fkind == FighterKind::Luigi && ally.costume == 0,
                    ),
                );
                self.models.push(make_model(
                    p,
                    FighterKind::Luigi,
                    costume,
                    ssb_rom::anim::SLOT_INTRO_R,
                    14,
                    0.0,
                    true,
                    camera,
                    spgame::intro::Entrance::opponent(Stage::Mario, FighterKind::Luigi, 0),
                ));
            }
        } else if matches!(screen, Screen::Continue(_)) {
            let camera = [0.0, 1000.0, 2000.0, 0.0, 0.0, 400.0, 0.0, 0.0, 0.0, 30.0];
            let mut model = make_model(
                p,
                data.fkind,
                data.costume,
                ssb_rom::anim::SLOT_FIGURE_DROPPED,
                9,
                0.0,
                false,
                camera,
                spgame::intro::Entrance::ally(0),
            );
            model.pos = [90.0, 2070.0, 0.0];
            model.scale = [
                1.25, 1.15, 1.0, 1.03, 1.21, 1.33, 1.05, 1.07, 1.22, 1.2, 1.26, 1.3,
            ][data.fkind as usize];
            self.models.push(model);
        }
        true
    }

    pub fn tick(&mut self, p: &Pack<'_>, screen: &Screen, session: &spgame::session::Session) {
        let fresh = self.sync(p, screen, session);
        for m in &mut self.models {
            let mut changed = fresh;
            match screen {
                Screen::Intro(s) => m.entrance.tick(s.total_tics),
                Screen::Continue(s) => {
                    if s.status == spgame::continue_scene::Status::Retry
                        && m.slot != ssb_rom::anim::SLOT_FIGURE_STAND as u32
                    {
                        m.slot = ssb_rom::anim::SLOT_FIGURE_STAND as u32;
                        m.demo.set_status(10);
                        m.transn = [0.0; 3];
                        if let Some(anim) = p.fighter_anim(m.kind as u32, m.slot) {
                            m.skeleton.start(p, &anim, 0.0, 1.0);
                            play(p, m);
                            m.transn = m.skeleton.pose(0).map_or([0.0; 3], |p| p.translate);
                        }
                        changed = true;
                    }
                }
                _ => {}
            }
            if !changed && !m.frozen {
                play(p, m);
                m.demo.tick(m.skeleton.frame());
                // No particle world here. These rows make no effects
                // (`demo_fighter::tests::the_campaign_rows_make_no_effects`),
                // so clearing the queue drops nothing.
                m.demo.fighter.effects.clear();
            }
            if matches!(screen, Screen::Continue(_) | Screen::Ending(_))
                && ssb_rom::anim::LEADING_RUNTIME_JOINT[m.kind as usize][m.slot as usize]
            {
                if let Some(pose) = m.skeleton.pose(0) {
                    let current = pose.translate;
                    for (i, value) in current.iter().enumerate() {
                        m.pos[i] += value - m.transn[i];
                    }
                    m.transn = current;
                }
            }
        }
    }

    pub unsafe fn draw(
        &mut self,
        gpu: &mut Gpu,
        p: &Pack<'_>,
        st: &mut DrawState,
        screen: &mut Screen,
        session: &spgame::session::Session,
    ) {
        self.sync(p, screen, session);
        gpu.set_viewport_n64([10.0, 10.0, 310.0, 230.0]);
        match screen {
            Screen::Intro(s) => {
                sprite(p, st, 11, a::INTRO_SKY, 10.0, 59.0, [255; 3], [0; 3], 1.0);
                match s.stage {
                    Stage::Bonus1 => sprite(
                        p,
                        st,
                        13,
                        a::PICTURES_TARGET,
                        103.0,
                        63.0,
                        [255; 3],
                        [0; 3],
                        1.0,
                    ),
                    Stage::Bonus2 => sprite(
                        p,
                        st,
                        14,
                        a::PLATFORM_PICTURE_SPRITE,
                        45.0,
                        55.0,
                        [255; 3],
                        [0; 3],
                        1.0,
                    ),
                    Stage::Bonus3 => sprite(
                        p,
                        st,
                        13,
                        a::PICTURES_RACE,
                        10.0,
                        55.0,
                        [255; 3],
                        [0; 3],
                        1.0,
                    ),
                    _ => {}
                }
                // `gcDrawAll` draws the cameras from the highest priority
                // down: the sky's (80), the fighters' (70 to 40), the
                // banners' (30), then the decals' and text's (20). Every
                // fighter is cut off by the banners (N64 reference, RE-458).
                let mut lights = spgame::intro::FighterLights::default();
                for priority in [70, 60, 50, 40] {
                    self.draw_intro_models(gpu, p, st, priority, &mut lights);
                }
                sprite(
                    p,
                    st,
                    11,
                    a::INTRO_BANNER_TOP,
                    10.0,
                    10.0,
                    [255; 3],
                    [0; 3],
                    1.0,
                );
                sprite(
                    p,
                    st,
                    11,
                    a::INTRO_BANNER_BOTTOM,
                    10.0,
                    182.0,
                    [255; 3],
                    [0; 3],
                    1.0,
                );
                intro_text(p, st, s, session);
            }
            Screen::Continue(s) => {
                s.draw();
                let scale = s.game_over_scale;
                if s.room_shown {
                    sprite(
                        p,
                        st,
                        79,
                        a::CONTINUE_ROOM,
                        160.0 - 130.0 * scale,
                        120.0 - 92.0 * scale,
                        [255; 3],
                        [0; 3],
                        scale,
                    );
                }
                fade(st, s.room_fade_in);
                if s.spotlight_shown {
                    sprite(
                        p,
                        st,
                        79,
                        a::CONTINUE_SHADOW,
                        80.0,
                        156.0,
                        [190, 190, 255],
                        [0; 3],
                        1.0,
                    );
                    sprite(
                        p,
                        st,
                        79,
                        a::CONTINUE_SPOTLIGHT,
                        80.0,
                        28.0,
                        [190, 190, 255],
                        [0; 3],
                        1.0,
                    );
                }
                fade(st, s.spotlight_fade);
                for m in &self.models {
                    let mut pos = m.pos;
                    pos[1] += s.fighter_y_offset;
                    draw_model(
                        gpu,
                        p,
                        st,
                        m,
                        pos,
                        scale,
                        spgame::continue_scene::CAMERA_PROJECTION,
                        FighterLight::Reflect([45.0, 45.0]),
                        RspCamera::Projection,
                    );
                }
                fade(st, s.room_fade_out);
                if s.prompt_shown {
                    sprite(
                        p,
                        st,
                        79,
                        a::CONTINUE_CONTINUE_TEXT,
                        64.0,
                        64.0,
                        [255; 3],
                        [0; 3],
                        1.0,
                    );
                }
                if s.options_shown {
                    for (selected, at, x) in [
                        (s.yes, a::CONTINUE_YES_TEXT, 84.0),
                        (!s.yes, a::CONTINUE_NO_TEXT, 189.0),
                    ] {
                        sprite(
                            p,
                            st,
                            79,
                            at,
                            x,
                            129.0,
                            if selected { [255, 0, 0] } else { [76, 71, 95] },
                            [0; 3],
                            1.0,
                        );
                    }
                    sprite(
                        p,
                        st,
                        79,
                        a::CONTINUE_CURSOR,
                        if s.yes { 76.0 } else { 177.0 },
                        120.0,
                        [255, 0, 0],
                        [0; 3],
                        1.0,
                    );
                }
                if s.status == spgame::continue_scene::Status::GameOver {
                    for (letter, x) in [
                        ('G', 30.0),
                        ('A', 60.0),
                        ('M', 95.0),
                        ('E', 133.0),
                        ('O', 166.0),
                        ('V', 200.0),
                        ('E', 230.0),
                        ('R', 254.0),
                    ] {
                        let at = ssb_rom::sprite::ANNOUNCE_COMMON.offsets
                            [(letter as u8 - b'A') as usize];
                        let v = (255.0 * s.game_over_color_step) as u8;
                        sprite(
                            p,
                            st,
                            ssb_rom::sprite::ANNOUNCE_COMMON.file,
                            at,
                            x,
                            50.0,
                            [v; 3],
                            [
                                (26.0 * s.game_over_color_step) as u8,
                                0,
                                (230.0 * s.game_over_color_step) as u8,
                            ],
                            1.0,
                        );
                    }
                }
                score(p, st, s.score);
            }
            Screen::StageClear(s) => {
                gpu.draw_campaign_wallpaper();
                st.invalidate_all();
                clear_text(p, st, s);
            }
            _ => ending::draw(self, gpu, p, st, screen),
        }
        gpu.set_viewport_pillarboxed();
    }

    unsafe fn draw_intro_models(
        &self,
        gpu: &mut Gpu,
        p: &Pack<'_>,
        st: &mut DrawState,
        priority: u8,
        lights: &mut spgame::intro::FighterLights,
    ) {
        for m in &self.models {
            if m.priority == priority && m.entrance.shown {
                let light = lights.next(m.kind);
                draw_model(
                    gpu,
                    p,
                    st,
                    m,
                    [0.0, 0.0, m.entrance.z],
                    1.0,
                    spgame::intro::CAMERA_PROJECTION,
                    light,
                    // `sc1PIntroMakeStageCamera` (kind 15) draws the frozen
                    // opponents; `sc1PIntroMakeFighterCamera` (kind 7) the
                    // player and allies.
                    if m.frozen {
                        RspCamera::ModelViewReflect
                    } else {
                        RspCamera::ModelView
                    },
                );
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn make_model(
    p: &Pack<'_>,
    kind: FighterKind,
    costume: u8,
    slot: usize,
    row: usize,
    frame: f32,
    frozen: bool,
    camera: [f32; 10],
    entrance: spgame::intro::Entrance,
) -> Box<Model> {
    let mut m = ssb_engine::box_in_place!(Model {
        kind: kind,
        costume: costume,
        shade: 0,
        slot: slot as u32,
        object: ssb_psp_runtime::scene::fighter_object(p, kind as u32)
            .expect("campaign fighter object"),
        demo: ssb_game::demo_fighter::DemoFighter::start(kind, 0, costume, row),
        entrance: entrance,
        camera: camera,
        pos: [0.0; 3],
        scale: 1.0,
        transn: [0.0; 3],
        frozen: frozen,
        priority: if frozen { 40 } else { 50 };
        // The intro and figure motions are unflagged: the `animlock` parts
        // stay frozen (RE-509).
        // SAFETY: `ptr` is the new box's skeleton field.
        skeleton => |ptr| unsafe { ssb_psp_runtime::scene::write_fighter_skeleton(ptr, kind, 1.0) },
    });
    if let Some(anim) = p.fighter_anim(kind as u32, slot as u32) {
        m.skeleton
            .start(p, &anim, frame, if frozen { 0.0 } else { 1.0 });
        play(p, &mut m);
        m.transn = m.skeleton.pose(0).map_or([0.0; 3], |p| p.translate);
    }
    m
}
fn play(p: &Pack<'_>, m: &mut Model) {
    if let Some(anim) = p.fighter_anim(m.kind as u32, m.slot) {
        if let Some(script) = p.anim_script(&anim) {
            let first = p.object(m.object).map_or(0, |o| o.first_node);
            // Demo descriptors do not set TRANSLATE_SCALES for Luigi.
            let scales = if m.kind == FighterKind::Luigi {
                None
            } else {
                p.fighter_translate_scales(m.kind as u32)
            };
            let _ = m.skeleton.tick_scaled(script, scales, first);
        }
    }
}
#[allow(clippy::too_many_arguments)]
unsafe fn draw_model(
    gpu: &mut Gpu,
    p: &Pack<'_>,
    st: &mut DrawState,
    m: &Model,
    pos: [f32; 3],
    scale: f32,
    (aspect, near, far): (f32, f32, f32),
    light: FighterLight,
    rsp: RspCamera,
) {
    draw_model_ex(
        gpu,
        p,
        st,
        m,
        pos,
        scale,
        (near, far),
        light,
        ModelView {
            camera: m.camera,
            aspect,
            yaw: 0.0,
            fog: None,
            rsp,
        },
    );
}

/// How [`draw_model_ex`] projects and shades a demo fighter.
#[derive(Clone, Copy)]
pub(crate) struct ModelView {
    /// The camera's eye (0..3), look-at point (4..7) and field of view (9).
    pub camera: [f32; 10],
    /// The camera's `persp.aspect`.
    pub aspect: f32,
    /// The fighter `DObj`'s yaw.
    pub yaw: f32,
    /// `ftDisplayMainCalcFogColor`'s blend towards a colour animation's
    /// `color1`.
    pub fog: Option<[u8; 4]>,
    /// Where the camera's matrix kind puts the view on the RSP.
    pub rsp: RspCamera,
}

/// The RSP state a camera's matrix kind leaves for a fighter it draws
/// (`gcPrepCameraMatrix`).
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum RspCamera {
    /// Kinds 6 and 8: the view multiplies the projection; lights and the
    /// look-at stay in world space, as in battle.
    Projection,
    /// Kind 7 (the 1P intro's fighter cameras): the view loads the
    /// modelview, so lights act in eye space, and no `LookAt` is loaded:
    /// texgen reads F3DEX2's zeroed default, one texel (N64 ParaLLEl trace,
    /// batch 40).
    ModelView,
    /// Kind 15 (the intro's stage camera, which draws the opponents): as
    /// [`Self::ModelView`], plus `syMatrixModLookAtReflect`'s `LookAt`, the
    /// camera's world right and up with world up (0, 1, 0) and no roll.
    ModelViewReflect,
}

#[allow(clippy::too_many_arguments)]
unsafe fn draw_model_ex(
    gpu: &mut Gpu,
    p: &Pack<'_>,
    st: &mut DrawState,
    m: &Model,
    pos: [f32; 3],
    scale: f32,
    planes: (f32, f32),
    light: FighterLight,
    view: ModelView,
) {
    use ssb_engine::math::{Mat4, Vec3};
    let Some(object) = p.object(m.object) else {
        return;
    };
    let c = view.camera;
    gpu.set_viewport_n64([10.0, 10.0, 310.0, 230.0]);
    gpu.set_perspective(c[9], view.aspect, planes.0, planes.1);
    gpu.reset_modelview();
    st.begin_frame();
    let (eye, at) = (Vec3::new(c[0], c[1], c[2]), Vec3::new(c[4], c[5], c[6]));
    gpu.set_view(&Mat4::look_at(eye, at, Vec3::new(0.0, 1.0, 0.0)));
    let texgen_basis = st.texgen_basis;
    if view.rsp != RspCamera::Projection {
        let basis =
            ssb_engine::math::camera_basis([c[0], c[1], c[2]], [c[4], c[5], c[6]], [0.0, 1.0, 0.0]);
        st.modelview_camera = Some(basis);
        st.texgen_basis = Some(if view.rsp == RspCamera::ModelViewReflect {
            (basis[0], basis[1])
        } else {
            ([0.0; 3], [0.0; 3])
        });
    }
    let mut posed = [ssb_rom::scene::Mat4::IDENTITY; ssb_rom::skeleton::MAX_NODES];
    let n = m.skeleton.compose(p, &object, &mut posed);
    let base = crate::topn_base(pos, view.yaw, m.scale * scale, m.skeleton.locks.flagged);
    match light {
        FighterLight::Reflect(angles) => st.configure_fighter_light(angles),
        FighterLight::Scene(dir) => st.configure_fighter_light_dir(dir),
    }
    // `ftDisplayMainDrawAll`'s fog: the colour animation's mixed with the
    // shade, or the shade's own.
    let fog = p
        .fighter_shade(m.kind as u32)
        .map_or(view.fog, |[s0, s1, s2, fog]| {
            ssb_game::shade::fog_color(m.shade, view.fog, &[s0, s1, s2], fog)
        });
    if let Some(rgba) = fog {
        let forward = (at - eye).normalized();
        gpu.set_fog_prim_a((Vec3::new(pos[0], pos[1], pos[2]) - eye).dot(forward), rgba);
    }
    st.fighter_fog = fog.is_some();
    let parts = m.demo.parts().draw_parts();
    meshdraw::draw_fighter_posed(
        p,
        &object,
        &base,
        &posed[..n],
        st,
        meshdraw::Look {
            costume: m.costume as u32,
            textures: m.demo.parts().draw_textures(),
            parts: parts.as_ref().map(|p| &p[..]),
            accessory_before: m.kind == FighterKind::Purin,
        },
    );
    if fog.is_some() {
        gpu.clear_fog();
    }
    st.finish_fighter_light();
    st.texgen_basis = texgen_basis;
    st.modelview_camera = None;
}
#[allow(clippy::too_many_arguments)]
unsafe fn sprite(
    p: &Pack<'_>,
    st: &mut DrawState,
    file: u32,
    at: u32,
    x: f32,
    y: f32,
    prim: [u8; 3],
    env: [u8; 3],
    scale: f32,
) {
    if let Some(s) = p.sprite(file, at) {
        meshdraw::draw_sprite(
            p,
            &s,
            &SObjDraw {
                x,
                y,
                scale,
                prim: [prim[0], prim[1], prim[2], 255],
                env,
                solid: false,
                attr: (s.attr & !ssb_rom::sprite::SP_FASTCOPY) | ssb_rom::sprite::SP_TRANSPARENT,
            },
            st,
        );
    }
}
unsafe fn fade(st: &mut DrawState, alpha: Option<u8>) {
    if let Some(a) = alpha {
        meshdraw::fill_rect_n64([10.0, 10.0, 310.0, 230.0], [0, 0, 0, a], st);
    }
}
unsafe fn score(p: &Pack<'_>, st: &mut DrawState, points: u32) {
    sprite(
        p,
        st,
        81,
        a::SCORE_SCORE_TEXT,
        90.0,
        200.0,
        [255, 200, 0],
        [255, 0, 0],
        1.0,
    );
    digits(p, st, points as i64, 295.0, 197.0, 2, 8, true);
}
unsafe fn digits(
    p: &Pack<'_>,
    st: &mut DrawState,
    value: i64,
    mut x: f32,
    y: f32,
    kind: u8,
    max: usize,
    fixed: bool,
) {
    let mut v = value.unsigned_abs();
    for i in 0..max {
        let digit = (v % 10) as usize;
        v /= 10;
        // Kinds 0 to 2 are the decomp's digit kinds; 3 is kind 1 with the
        // No Miss multiplier's colours.
        let digit_kind = if kind == 3 { 1 } else { usize::from(kind) };
        let file = ssb_rom::scene_roots::ONE_P_STAGE_CLEAR[a::CLEAR_DIGIT_FILES[digit_kind] as usize];
        let at = a::CLEAR_DIGIT_OFFSETS[digit][digit_kind];
        let [env, color] = if kind == 3 {
            a::CLEAR_NO_MISS_COLORS
        } else {
            a::CLEAR_DIGIT_COLORS[digit_kind]
        };
        if let Some(s) = p.sprite(file, at) {
            x -= if kind == 2 {
                16.0
            } else {
                f32::from(s.width) + f32::from(kind == 0)
            };
            sprite(p, st, file, at, x, y, color, env, 1.0);
        }
        if !fixed && v == 0 {
            break;
        }
        if i + 1 == max {
            break;
        }
    }
    if value < 0 && kind == 1 {
        let at = ssb_rom::sprite::DIGITS.offsets[10];
        if let Some(s) = p.sprite(36, at) {
            sprite(
                p,
                st,
                36,
                at,
                x - f32::from(s.width),
                y + 3.0,
                [255; 3],
                [0; 3],
                1.0,
            );
        }
    }
}

unsafe fn intro_text(
    p: &Pack<'_>,
    st: &mut DrawState,
    s: &spgame::intro::Intro,
    session: &spgame::session::Session,
) {
    let data = &session.data;
    let stage = s.stage as usize;
    if matches!(s.stage, Stage::Bonus1 | Stage::Bonus2 | Stage::Bonus3) {
        sprite(
            p,
            st,
            11,
            a::INTRO_BONUS_TEXT,
            15.0,
            18.0,
            [255; 3],
            [0; 3],
            1.0,
        );
    }
    if s.stage == Stage::Boss {
        sprite(
            p,
            st,
            11,
            a::INTRO_FINAL_TEXT,
            15.0,
            17.0,
            [255; 3],
            [0; 3],
            1.0,
        );
    }
    sprite(
        p,
        st,
        11,
        a::INTRO_STAGE_TEXT,
        23.0,
        33.0,
        [255; 3],
        [0; 3],
        1.0,
    );
    if s.stage != Stage::Boss {
        sprite(
            p,
            st,
            11,
            a::INTRO_STAGE_NUMBERS[stage],
            80.0,
            33.0,
            [255; 3],
            [0; 3],
            1.0,
        );
    }
    for i in stage..14 {
        sprite(
            p,
            st,
            11,
            a::INTRO_FIGURES[i],
            a::INTRO_FIGURE_POS[i][0] as f32,
            a::INTRO_FIGURE_POS[i][1] as f32,
            [255; 3],
            [0; 3],
            1.0,
        );
    }
    let task = match s.stage {
        Stage::Bonus1 => Some(a::INTRO_BREAK_THE_TARGETS_TEXT),
        Stage::Bonus2 => Some(a::INTRO_BOARD_THE_PLATFORMS_TEXT),
        Stage::Bonus3 => Some(a::INTRO_RACE_TO_THE_FINISH_TEXT),
        _ => None,
    };
    if let Some(at) = task {
        let width = p.sprite(11, at).map_or(0, |s| s.width);
        sprite(
            p,
            st,
            11,
            at,
            160.0 - f32::from(width / 2),
            197.0,
            [255; 3],
            [0; 3],
            1.0,
        );
        return;
    }
    sprite(
        p,
        st,
        11,
        a::INTRO_VSDECAL,
        135.0,
        104.0,
        [255; 3],
        [0; 3],
        1.0,
    );
    let name = a::NAMES.offsets[data.fkind as usize];
    // The bonus stages returned above.
    let opponent = a::INTRO_OPPONENTS[stage];
    let file = if matches!(s.stage, Stage::Link | Stage::Pikachu) {
        a::NAMES.file
    } else {
        a::INTRO.file
    };
    let width = |file, at| p.sprite(file, at).map_or(0, |s| i32::from(s.width));
    let allies = spgame::intro::allies_num(s.stage);
    // Only the stage's own allies: on other stages the ally ports hold the
    // last battle's Polygons, which have no name (RE-478).
    let mut ally_names = [0; 2];
    let named = spgame::intro::named_allies(s.stage, data, &session.state);
    for (name, kind) in ally_names.iter_mut().zip(named) {
        *name = a::NAMES.offsets[kind as usize];
    }
    let ally_width = if allies == 0 {
        0
    } else {
        width(11, a::INTRO_DASH)
            + if allies == 1 {
                width(12, ally_names[0])
            } else {
                width(12, ally_names[0]).max(width(12, ally_names[1]))
            }
    };
    let player_width = width(12, name) + ally_width + 10;
    let vs_width = width(11, a::INTRO_VSTEXT);
    let total = player_width + vs_width + width(file, opponent) + 10;
    let x = (160 - total / 2) as f32;
    sprite(p, st, 12, name, x, 196.0, [255; 3], [0; 3], 1.0);
    if allies != 0 {
        let dash_x = x + width(12, name) as f32;
        sprite(
            p,
            st,
            11,
            a::INTRO_DASH,
            dash_x,
            196.0,
            [255; 3],
            [0; 3],
            1.0,
        );
        for (i, &at) in ally_names.iter().take(allies).enumerate() {
            sprite(
                p,
                st,
                12,
                at,
                dash_x + width(11, a::INTRO_DASH) as f32,
                if allies == 1 {
                    196.0
                } else {
                    188.0 + i as f32 * 16.0
                },
                [255; 3],
                [0; 3],
                1.0,
            );
        }
        if allies == 1 {
            sprite(
                p,
                st,
                11,
                a::INTRO_ALLY_TEXT,
                80.0,
                80.0,
                [255, 0, 0],
                [0; 3],
                1.0,
            );
        } else {
            sprite(
                p,
                st,
                11,
                a::INTRO_ALLY_TEXT2,
                80.0,
                70.0,
                [255, 0, 0],
                [0; 3],
                1.0,
            );
            sprite(
                p,
                st,
                11,
                a::INTRO_ALLY_TEXT,
                90.0,
                100.0,
                [255, 0, 0],
                [0; 3],
                1.0,
            );
        }
    }
    let vx = x + player_width as f32;
    sprite(p, st, 11, a::INTRO_VSTEXT, vx, 196.0, [255; 3], [0; 3], 1.0);
    sprite(
        p,
        st,
        file,
        opponent,
        vx + vs_width as f32 + 10.0,
        if s.stage == Stage::Zako { 190.0 } else { 196.0 },
        [255; 3],
        [0; 3],
        1.0,
    );
}

unsafe fn clear_text(p: &Pack<'_>, st: &mut DrawState, s: &spgame::stage_clear::StageClear) {
    use spgame::stage_clear::Kind;
    sprite(
        p,
        st,
        80,
        a::CLEAR_TEXT_SHADOW,
        33.0,
        23.0,
        [0; 3],
        [0; 3],
        1.0,
    );
    if s.kind == Kind::Result {
        sprite(
            p,
            st,
            80,
            a::CLEAR_RESULT_TEXT,
            104.0,
            24.0,
            [255, 200, 0],
            [255, 0, 0],
            1.0,
        );
    } else {
        sprite(
            p,
            st,
            80,
            if s.kind == Kind::Stage {
                a::CLEAR_STAGE_TEXT
            } else {
                a::CLEAR_GAME_TEXT
            },
            53.0,
            24.0,
            [255, 200, 0],
            [255, 0, 0],
            1.0,
        );
        sprite(
            p,
            st,
            80,
            a::CLEAR_CLEAR_TEXT,
            166.0,
            24.0,
            [255, 200, 0],
            [255, 0, 0],
            1.0,
        );
    }
    score(p, st, s.shown_score);
    if s.bonus_table {
        sprite(
            p,
            st,
            80,
            a::CLEAR_BONUS_BORDER,
            52.0,
            62.0,
            [250, 226, 181],
            [0; 3],
            1.0,
        );
        sprite(
            p,
            st,
            80,
            a::CLEAR_SPECIAL_BONUS_TEXT,
            91.0,
            72.0,
            [255; 3],
            [255, 255, 0],
            1.0,
        );
        for (i, row) in s.rows.iter().enumerate() {
            let Some(row) = row.filter(|r| r.reveal_tic < s.total_tics) else {
                continue;
            };
            let at = if row.bonus_id == spgame::bonus::Bonus::StageClear as usize {
                a::CLEAR_DIFFICULTY_TEXT[s.difficulty as usize]
            } else {
                a::BONUS_LABELS[row.bonus_id]
            };
            let y = 86.0 + i as f32 * 11.0;
            let game_clear = matches!(row.bonus_id, 22..=26);
            sprite(
                p,
                st,
                80,
                at,
                80.0,
                y,
                if game_clear {
                    [255, 0, 0]
                } else {
                    [255, 255, 0]
                },
                [0; 3],
                1.0,
            );
            if row.bonus_id == spgame::bonus::Bonus::NoMiss as usize {
                sprite(p, st, 36, 0x828, 120.0, y - 1.0, [255, 255, 0], [0; 3], 1.0);
                // The multiplier uses the same small bonus digits with yellow.
                digits(
                    p,
                    st,
                    spgame::results::NO_MISS_MULTIPLIERS[s.stage as usize] as i64,
                    146.0,
                    y - 1.0,
                    3,
                    2,
                    false,
                );
            }
            sprite(p, st, 36, 0x8d8, 183.0, y, [255, 255, 0], [0; 3], 1.0);
            digits(p, st, row.points as i64, 241.0, y - 1.0, 1, 6, false);
        }
        if s.page_arrow_tic.is_some_and(|t| t < s.total_tics) {
            sprite(
                p,
                st,
                80,
                a::CLEAR_BONUS_PAGE_ARROW,
                249.0,
                176.0,
                [255; 3],
                [0; 3],
                1.0,
            );
        }
        return;
    }
    sprite(
        p,
        st,
        80,
        a::CLEAR_BONUS_TEXT,
        121.0,
        67.0,
        [255, 40, 10],
        [0; 3],
        1.0,
    );
    if s.target_text {
        sprite(
            p,
            st,
            80,
            a::CLEAR_TARGET_TEXT,
            42.0,
            94.0,
            [183, 228, 255],
            [0; 3],
            1.0,
        );
        sprite(
            p,
            st,
            80,
            a::CLEAR_COLON_TEXT,
            118.0,
            96.0,
            [183, 228, 255],
            [0; 3],
            1.0,
        );
        for i in 0..s.objectives_shown {
            sprite(
                p,
                st,
                151,
                if s.stage == Stage::Bonus1 {
                    a::OBJECTIVES_TARGET
                } else {
                    a::OBJECTIVES_PLATFORM
                },
                130.0 + 16.0 * f32::from(i),
                93.0,
                [255; 3],
                [0; 3],
                1.0,
            );
        }
    }
    let timer_y = if s.kind == Kind::Result && s.stage != Stage::Bonus3 {
        126.0
    } else {
        94.0
    };
    if s.timer_text {
        sprite(
            p,
            st,
            80,
            a::CLEAR_TIMER_TEXT,
            42.0,
            timer_y,
            [183, 228, 255],
            [0; 3],
            1.0,
        );
        sprite(
            p,
            st,
            80,
            a::CLEAR_COLON_TEXT,
            118.0,
            timer_y + 1.0,
            [183, 228, 255],
            [0; 3],
            1.0,
        );
    }
    if s.timer_digits {
        let mult = match s.stage {
            Stage::Bonus1 | Stage::Bonus2 => 200,
            Stage::Bonus3 => 500,
            _ => 50,
        };
        if s.timer_multiplied {
            digits(
                p,
                st,
                i64::from(s.seconds) * mult,
                200.0,
                timer_y - 1.0,
                0,
                5,
                false,
            );
        } else {
            digits(p, st, s.seconds as i64, 171.0, timer_y - 1.0, 0, 3, false);
            sprite(
                p,
                st,
                165,
                0x1018,
                181.0,
                timer_y + 2.0,
                [255; 3],
                [0; 3],
                1.0,
            );
            digits(
                p,
                st,
                mult,
                if mult == 50 { 233.0 } else { 246.0 },
                timer_y - 1.0,
                0,
                4,
                false,
            );
        }
    }
    let dy = if s.no_timer { 94.0 } else { 126.0 };
    if s.damage_text {
        sprite(
            p,
            st,
            80,
            a::CLEAR_DAMAGE_TEXT,
            42.0,
            dy,
            [183, 228, 255],
            [0; 3],
            1.0,
        );
        sprite(
            p,
            st,
            80,
            a::CLEAR_COLON_TEXT,
            118.0,
            dy + 2.0,
            [183, 228, 255],
            [0; 3],
            1.0,
        );
    }
    if s.damage_digits {
        if s.damage_multiplied {
            digits(
                p,
                st,
                i64::from(s.damage) * 10,
                200.0,
                dy - 1.0,
                0,
                5,
                false,
            );
        } else {
            let x = if s.damage > 1000 { 184.0 } else { 171.0 };
            digits(p, st, s.damage as i64, x, dy - 1.0, 0, 4, false);
            sprite(
                p,
                st,
                165,
                0x1018,
                x + 10.0,
                dy + 2.0,
                [255; 3],
                [0; 3],
                1.0,
            );
            digits(p, st, 10, x + 55.0, dy - 1.0, 0, 2, true);
        }
    }
}
