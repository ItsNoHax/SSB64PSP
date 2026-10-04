//! Packed presentation adapter for sc1pintro, mn1pcontinue and sc1pstageclear.
//! All scene/input/score clocks remain in ssb-game. Demo poses live on the heap.
use alloc::{boxed::Box, vec::Vec};
use ssb_game::{
    fighter::FighterKind,
    spgame::{self, frontend::Screen, Stage},
};
use ssb_psp_runtime::{
    gu::Gpu,
    meshdraw::{self, DrawState, SObjDraw},
};
use ssb_rom::{campaign as a, pack::Pack, skeleton::Skeleton};

#[derive(Default)]
pub struct Presentation {
    scene: Option<(u8, u8)>,
    models: Vec<Box<Model>>,
    snapshot_pending: bool,
}
struct Model {
    kind: FighterKind,
    costume: u8,
    slot: u32,
    object: u32,
    skeleton: Skeleton,
    demo: ssb_game::modelpart::DemoParts,
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
            _ => (3, data.stage),
        };
        if self.scene == Some(id) {
            return false;
        }
        self.scene = Some(id);
        self.snapshot_pending = id.0 == 2;
        self.models.clear();
        if let Screen::Intro(intro) = screen {
            let allies = match intro.stage {
                Stage::Mario => 1,
                Stage::Donkey => 2,
                _ => 0,
            };
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
                self.models.push(model);
            }
            let mut camera = a::packed_camera(p, data.fkind as usize).expect("packed intro camera");
            let override_camera = a::ALLY_CAMERAS[data.fkind as usize][player_card];
            camera[..3].copy_from_slice(&override_camera[..3]);
            camera[4..7].copy_from_slice(&override_camera[3..]);
            self.models.push(make_model(
                p,
                data.fkind,
                data.costume,
                ssb_rom::anim::SLOT_INTRO_L,
                13,
                0.0,
                false,
                camera,
                spgame::intro::Entrance::ally(player_card),
            ));
            let (kind, index, count) = match intro.stage {
                Stage::Link => (FighterKind::Link, 12, 1),
                Stage::Yoshi => (FighterKind::Yoshi, 13, 18),
                Stage::Fox => (FighterKind::Fox, 14, 1),
                Stage::Mario => (FighterKind::Mario, 15, 1),
                Stage::Donkey => (FighterKind::Donkey, 17, 1),
                Stage::Pikachu => (FighterKind::Pikachu, 16, 1),
                Stage::Kirby => (FighterKind::Kirby, 18, 8),
                Stage::Samus => (FighterKind::Samus, 19, 1),
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
                    model
                        .demo
                        .parts
                        .set_detail_all(ssb_game::modelpart::Detail::Low);
                }
                if intro.stage == Stage::Kirby {
                    let part = [13, 5, 10, 4, 7, 12, 6, 8][card as usize];
                    model.demo.parts.set(
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
                        m.demo = ssb_game::modelpart::DemoParts::start(m.kind, 10);
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
                m.demo.tick();
            }
            if matches!(screen, Screen::Continue(_))
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
                self.draw_intro_models(gpu, p, st, 70);
                intro_text(p, st, s, session);
                for priority in [60, 50, 40] {
                    self.draw_intro_models(gpu, p, st, priority);
                }
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
                    draw_model(gpu, p, st, m, pos, scale, (100.0, 15000.0), [45.0, 45.0]);
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
            _ => {}
        }
        gpu.set_viewport_fullscreen();
    }

    unsafe fn draw_intro_models(
        &self,
        gpu: &mut Gpu,
        p: &Pack<'_>,
        st: &mut DrawState,
        priority: u8,
    ) {
        for m in &self.models {
            if m.priority == priority && m.entrance.shown {
                draw_model(
                    gpu,
                    p,
                    st,
                    m,
                    [0.0, 0.0, m.entrance.z],
                    1.0,
                    (128.0, 16384.0),
                    [-20.0, 30.0],
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
    let mut m = Box::new(Model {
        kind,
        costume,
        slot: slot as u32,
        object: ssb_psp_runtime::scene::fighter_object(p, kind as u32)
            .expect("campaign fighter object"),
        skeleton: Skeleton::new(),
        demo: ssb_game::modelpart::DemoParts::start(kind, row),
        entrance,
        camera,
        pos: [0.0; 3],
        scale: 1.0,
        transn: [0.0; 3],
        frozen,
        priority: if frozen { 40 } else { 50 },
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
    planes: (f32, f32),
    light: [f32; 2],
) {
    use ssb_engine::math::{Mat4, Vec3};
    let Some(object) = p.object(m.object) else {
        return;
    };
    let c = m.camera;
    gpu.set_viewport_n64([10.0, 10.0, 310.0, 230.0]);
    gpu.set_perspective(c[9], 300.0 / 220.0, planes.0, planes.1);
    gpu.reset_modelview();
    st.begin_frame();
    gpu.set_view(&Mat4::look_at(
        Vec3::new(c[0], c[1], c[2]),
        Vec3::new(c[4], c[5], c[6]),
        Vec3::new(0.0, 1.0, 0.0),
    ));
    let mut posed = [ssb_rom::scene::Mat4::IDENTITY; ssb_rom::skeleton::MAX_NODES];
    let n = m.skeleton.compose(p, &object, &mut posed);
    gpu.model_transform(pos, [0.0; 3], meshdraw::MODEL_SCALE * m.scale * scale);
    let base = gpu.model_matrix();
    st.configure_fighter_light(light);
    let parts = m.demo.parts.draw_parts();
    meshdraw::draw_fighter_posed(
        p,
        &object,
        &base,
        &posed[..n],
        st,
        meshdraw::Look {
            costume: m.costume as u32,
            textures: m.demo.parts.draw_textures(),
            parts: parts.as_ref().map(|p| &p[..]),
            accessory_before: m.kind == FighterKind::Purin,
        },
    );
    st.finish_fighter_light();
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
        let (file, at, color) = match kind {
            0 => (80, 0xb808 + digit as u32 * 0x160, [200, 203, 211]),
            1 => (36, ssb_rom::sprite::DIGITS.offsets[digit], [255; 3]),
            3 => (36, ssb_rom::sprite::DIGITS.offsets[digit], [255, 255, 0]),
            _ => (
                ssb_rom::sprite::PLAYER_DAMAGE.file,
                ssb_rom::sprite::PLAYER_DAMAGE.offsets[digit],
                [255, 236, 0],
            ),
        };
        if let Some(s) = p.sprite(file, at) {
            x -= if kind == 2 {
                16.0
            } else {
                f32::from(s.width) + f32::from(kind == 0)
            };
            sprite(p, st, file, at, x, y, color, [0; 3], 1.0);
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
    let number = [1, 2, 3, 1, 4, 5, 6, 2, 7, 8, 9, 3, 10, 0][stage];
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
    if number != 0 {
        sprite(
            p,
            st,
            11,
            a::INTRO.offsets[number],
            80.0,
            33.0,
            [255; 3],
            [0; 3],
            1.0,
        );
    }
    let figures = [
        a::INTRO_LINK_MARKER,
        a::INTRO_YOSHI_MARKER,
        a::INTRO_FOX_MARKER,
        a::INTRO_BONUS_MARKER,
        a::INTRO_MARIO_BROS_MARKER,
        a::INTRO_PIKACHU_MARKER,
        a::INTRO_DKMARKER,
        a::INTRO_BONUS_MARKER,
        a::INTRO_KIRBY_MARKER,
        a::INTRO_SAMUS_MARKER,
        a::INTRO_MARIO_MARKER,
        a::INTRO_BONUS_MARKER,
        a::INTRO_EXCLAMATION_MARK,
        a::INTRO_BOSS_MARKER,
    ];
    let positions = [
        (99.0, 18.0),
        (115.0, 26.0),
        (131.0, 18.0),
        (148.0, 33.0),
        (156.0, 26.0),
        (172.0, 18.0),
        (189.0, 26.0),
        (205.0, 33.0),
        (212.0, 18.0),
        (230.0, 26.0),
        (246.0, 18.0),
        (261.0, 33.0),
        (270.0, 34.0),
        (280.0, 23.0),
    ];
    for i in stage..14 {
        sprite(
            p,
            st,
            11,
            figures[i],
            positions[i].0,
            positions[i].1,
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
    let (file, opponent) = match s.stage {
        Stage::Link => (12, a::NAMES_LINK),
        Stage::Yoshi => (11, a::INTRO_YOSHI_TEAM_VS18_TEXT),
        Stage::Fox => (11, a::INTRO_FOX_MC_CLOUD_TEXT),
        Stage::Mario => (11, a::INTRO_MARIO_BROS_TEXT),
        Stage::Pikachu => (12, a::NAMES_PIKACHU),
        Stage::Donkey => (11, a::INTRO_GIANT_DKTEXT),
        Stage::Kirby => (11, a::INTRO_KIRBY_TEAM_VS8_TEXT),
        Stage::Samus => (11, a::INTRO_SAMUS_ARAN_TEXT),
        Stage::MMario => (11, a::INTRO_METAL_MARIO_TEXT),
        Stage::Zako => (11, a::INTRO_FIGHTING_POLYGON_TEAM_VS30_TEXT),
        Stage::Boss => (11, a::INTRO_MASTER_HAND_TEXT),
        _ => return,
    };
    let width = |file, at| p.sprite(file, at).map_or(0, |s| i32::from(s.width));
    let allies = match s.stage {
        Stage::Mario => 1,
        Stage::Donkey => 2,
        _ => 0,
    };
    let ally_names = core::array::from_fn::<_, 2, _>(|i| {
        a::NAMES.offsets[session.state.players[data.ally_players[i] as usize].fkind as usize]
    });
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
                a::CLEAR_VERY_EASY_CLEAR_TEXT + (s.difficulty as u32) * 0x1e0
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
