//! The campaign's last scenes, drawn: the ending movie (`mvending.c`), the
//! staff roll (`scstaffroll.c`), the congratulations picture
//! (`mncongra.c`), the challenger warning (`sc1pchallenger.c`) and the
//! unlock message (`mnmessage.c`). Their clocks, input and state live in
//! `ssb_game::spgame`; this draws what they show, in the order their
//! cameras draw (highest priority first).
//!
//! The staff roll runs at 640 x 480 on the N64 (`dSCStaffrollVideoSetup`).
//! The PSP draws it in the same pillarboxed picture as every other scene:
//! its 2D positions are halved onto the 320 x 240 grid, and its sprites at
//! half their size.
use super::{
    draw_model_ex, make_model, sprite, DrawState, Gpu, ModelView, Pack, Presentation,
};
use alloc::boxed::Box;
use ssb_game::spgame::{
    challenger, congra, ending as movie, frontend::Screen, message, staffroll,
};
use ssb_psp_runtime::{ending as visual, meshdraw};
use ssb_rom::ending as asset;

/// The ending's room and camera and the staff roll's textures, made when
/// their scene starts.
#[derive(Default)]
pub(super) struct LastScenes {
    room: Option<Box<visual::Room>>,
    camera: Option<ssb_psp_runtime::scene::CameraAnim>,
    staffroll: Option<Box<visual::StaffrollTextures>>,
}

/// `gcMakeCameraGObj`'s default projection (`dGCPerspDefault`).
const ASPECT: f32 = 4.0 / 3.0;
const VIEWPORT: [f32; 4] = [10.0, 10.0, 310.0, 230.0];

/// Makes a new scene's models and visuals. `false` for the other scenes.
pub(super) fn make(pres: &mut Presentation, p: &Pack<'_>, screen: &Screen) -> bool {
    pres.last = LastScenes::default();
    match screen {
        Screen::Ending(e) => {
            // `mvEndingMakeFighter`: the figure drops from its status's
            // first frame; the operator camera fills `camera` per frame.
            let mut model = make_model(
                p,
                e.fkind,
                e.costume,
                ssb_rom::anim::SLOT_FIGURE_DROPPED,
                9,
                0.0,
                false,
                [0.0; 10],
                ssb_game::spgame::intro::Entrance::ally(0),
            );
            model.pos = movie::FIGHTER_POSITION;
            pres.models.push(model);
            pres.last.room = visual::Room::new(p).map(Box::new);
            pres.last.camera = visual::ending_camera(p);
            true
        }
        Screen::Challenger(c) => {
            // A demo fighter in `nFTDemoStatusNull`: its `Wait` clip.
            let mut camera = [0.0; 10];
            camera[..3].copy_from_slice(&challenger::CAMERA_EYE);
            camera[9] = 30.0;
            let mut model = make_model(
                p,
                c.fkind,
                ssb_game::costume::costume_common_id(c.fkind, 0),
                ssb_rom::anim::SLOT_WAIT,
                0,
                0.0,
                false,
                camera,
                ssb_game::spgame::intro::Entrance::ally(0),
            );
            model.pos = challenger::FIGHTER_POSITION;
            model.scale = ssb_game::results_scene::FIGHTER_SCALES[c.fkind as usize];
            pres.models.push(model);
            true
        }
        Screen::Staffroll(_) => {
            pres.last.staffroll = Some(Box::new(visual::StaffrollTextures::new(p)));
            true
        }
        Screen::Congra(_) | Screen::Message(_) => true,
        _ => false,
    }
}

/// N64 screen black, before the scene's own cameras.
unsafe fn clear_black(st: &mut DrawState) {
    meshdraw::fill_rect_n64([0.0, 0.0, 320.0, 240.0], [0, 0, 0, 0xFF], st);
}

pub(super) unsafe fn draw(
    pres: &mut Presentation,
    gpu: &mut Gpu,
    p: &Pack<'_>,
    st: &mut DrawState,
    screen: &mut Screen,
) {
    match screen {
        Screen::Ending(e) => {
            e.draw();
            draw_ending(pres, gpu, p, st, e);
        }
        Screen::Staffroll(s) => draw_staffroll(pres, gpu, p, st, s),
        Screen::Congra(c) => draw_congra(p, st, c),
        Screen::Challenger(c) => draw_challenger(pres, gpu, p, st, c),
        Screen::Message(m) => draw_message(p, st, m),
        _ => {}
    }
}

/// The operator camera's play this tick: `[eye, at, fovy]`.
fn ending_camera(pres: &Presentation, total_tics: u32) -> Option<[f32; 7]> {
    let frames = &pres.last.camera.as_ref()?.frames;
    let index = (total_tics as usize).saturating_sub(1).min(frames.len().checked_sub(1)?);
    Some(frames[index])
}

/// `mvEndingFuncStart`'s cameras: the room (80), the fade-in (60), the
/// figure (40, sharing the room's depth) and the light (30), over a white
/// fill.
unsafe fn draw_ending(
    pres: &Presentation,
    gpu: &mut Gpu,
    p: &Pack<'_>,
    st: &mut DrawState,
    e: &movie::Ending,
) {
    use ssb_engine::math::{Mat4, Vec3};
    clear_black(st);
    meshdraw::fill_rect_n64(VIEWPORT, [0xFF; 4], st);
    let Some(c) = ending_camera(pres, e.total_tics) else {
        return;
    };
    let (eye, at) = (Vec3::new(c[0], c[1], c[2]), Vec3::new(c[3], c[4], c[5]));
    if let Some(room) = pres.last.room.as_ref() {
        gpu.set_viewport_n64(VIEWPORT);
        gpu.set_perspective(c[6], ASPECT, movie::CAMERA_NEAR, movie::CAMERA_FAR);
        gpu.reset_modelview();
        st.begin_frame();
        gpu.set_view(&Mat4::look_at(eye, at, Vec3::Y));
        room.draw(gpu, p, st, e.props_shown);
    }
    if e.fade_in_alpha > 0 {
        meshdraw::fill_rect_n64(VIEWPORT, [0, 0, 0, e.fade_in_alpha.clamp(0, 0xFF) as u8], st);
    }
    if e.fighter_shown {
        if let Some(m) = pres.models.first() {
            draw_model_ex(
                gpu,
                p,
                st,
                m,
                m.pos,
                1.0,
                (movie::CAMERA_NEAR, movie::CAMERA_FAR),
                movie::LIGHT_ANGLE,
                ModelView {
                    camera: [c[0], c[1], c[2], 0.0, c[3], c[4], c[5], 0.0, 0.0, c[6]],
                    aspect: ASPECT,
                    yaw: 0.0,
                    fog: None,
                },
            );
        }
    }
    if let Some(alpha) = e.light_prim_alpha() {
        meshdraw::fill_rect_n64(VIEWPORT, [0xFF, 0xFF, 0xFF, alpha], st);
    }
}

/// `scStaffrollMakeCamera`'s 3D camera (50: names, jobs, the frame, the
/// lock-on corners and the text box), then its sprite camera (30: the
/// crosshair, the role and company texts and the brackets).
unsafe fn draw_staffroll(
    pres: &Presentation,
    gpu: &mut Gpu,
    p: &Pack<'_>,
    st: &mut DrawState,
    s: &staffroll::Staffroll,
) {
    use ssb_engine::math::{Mat4, Vec3};
    clear_black(st);
    if s.blackout {
        return;
    }
    let Some(textures) = pres.last.staffroll.as_ref() else {
        return;
    };
    let [ex, ey, ez] = staffroll::CAMERA_EYE;
    let [ax, ay, az] = s.camera_at;
    gpu.set_viewport_n64(staffroll::VIEWPORT.map(|v| v * 0.5));
    gpu.set_perspective(
        staffroll::CAMERA_FOVY,
        staffroll::CAMERA_ASPECT,
        staffroll::CAMERA_NEAR,
        staffroll::CAMERA_FAR,
    );
    gpu.reset_modelview();
    st.begin_frame();
    gpu.set_view(&Mat4::look_at(
        Vec3::new(ex, ey, ez),
        Vec3::new(ax, ay, az),
        Vec3::Y,
    ));
    for (is_name, color) in [(true, staffroll::NAME_COLOR), (false, staffroll::JOB_COLOR)] {
        for text in s.texts.iter().filter(|t| t.is_name == is_name && !t.hidden) {
            textures.draw_text(gpu, p, st, text, color);
        }
    }
    if let Some(frame) = s.frame.as_ref() {
        textures.draw_frame(gpu, p, st, frame);
    }
    gpu.set_viewport_n64(VIEWPORT);
    // `G_CYC_FILL` rectangles include both corners.
    let fill = |st: &mut DrawState, r: [i32; 4], c: [u8; 3]| {
        meshdraw::fill_rect_n64(
            [
                r[0] as f32 * 0.5,
                r[1] as f32 * 0.5,
                (r[2] + 1) as f32 * 0.5,
                (r[3] + 1) as f32 * 0.5,
            ],
            [c[0], c[1], c[2], 0xFF],
            st,
        );
    };
    if let Some(h) = s.highlight {
        for r in h.rects() {
            fill(st, r, staffroll::HIGHLIGHT_COLOR);
        }
    }
    if s.text_box {
        for r in staffroll::TEXT_BOX_RECTS {
            fill(st, r, staffroll::TEXT_BOX_COLOR);
        }
    }
    let glyph = |st: &mut DrawState, at: u32, pos: [f32; 2], scale: [f32; 2], color: [u8; 3]| {
        if let Some(sp) = p.sprite(asset::STAFFROLL_FILE, at) {
            meshdraw::draw_sprite_xy(
                p,
                &sp,
                &meshdraw::SObjDraw {
                    x: pos[0] * 0.5,
                    y: pos[1] * 0.5,
                    scale: 1.0,
                    prim: [color[0], color[1], color[2], 0xFF],
                    env: [0; 3],
                    solid: false,
                    attr: ssb_rom::sprite::SP_TRANSPARENT,
                },
                [scale[0] * 0.5, scale[1] * 0.5],
                st,
            );
        }
    };
    glyph(
        st,
        asset::CROSSHAIR,
        s.crosshair,
        [staffroll::CROSSHAIR_SCALE; 2],
        staffroll::CROSSHAIR_COLOR,
    );
    for (text, color) in [
        (s.role_text.as_ref(), staffroll::ROLE_COLOR),
        (s.company_text.as_ref(), staffroll::COMPANY_COLOR),
    ] {
        for g in text.into_iter().flatten() {
            glyph(
                st,
                asset::TEXT_SPRITES[usize::from(g.glyph)],
                [g.x, g.y],
                [1.0; 2],
                color,
            );
        }
    }
    if s.text_box {
        for (at, pos) in [
            (asset::BRACKET_LEFT, staffroll::BRACKET_LEFT),
            (asset::BRACKET_RIGHT, staffroll::BRACKET_RIGHT),
        ] {
            glyph(st, at, pos, staffroll::BRACKET_SCALE, staffroll::BRACKET_COLOR);
        }
    }
}

/// `mnCongraFuncStart`'s picture over black, then the fade.
unsafe fn draw_congra(p: &Pack<'_>, st: &mut DrawState, c: &congra::Congra) {
    clear_black(st);
    if c.blackout {
        return;
    }
    let [bottom, top] = asset::CONGRA_FILES[c.fkind as usize];
    for (file, pos) in [(bottom, congra::BOTTOM_POSITION), (top, congra::TOP_POSITION)] {
        // The pack cuts each picture at column 256 (`ssb_rom::ending::congra_part`).
        let split = f32::from(asset::CONGRA_SPLIT);
        sprite(p, st, file, asset::CONGRA_SPRITE, pos[0], pos[1], [0xFF; 3], [0; 3], 1.0);
        sprite(p, st, file, asset::CONGRA_SPRITE_RIGHT, pos[0] + split, pos[1], [0xFF; 3], [0; 3], 1.0);
    }
    if let Some(f) = c.fade {
        meshdraw::fill_rect_n64(VIEWPORT, [0, 0, 0, f.alpha()], st);
    }
}

/// `sc1PChallengerFuncStart`'s decals (70) then the turning silhouette (40).
unsafe fn draw_challenger(
    pres: &Presentation,
    gpu: &mut Gpu,
    p: &Pack<'_>,
    st: &mut DrawState,
    c: &challenger::Challenger,
) {
    clear_black(st);
    meshdraw::fill_rect_n64(challenger::PANEL_RECT, challenger::PANEL_COLOR, st);
    for d in challenger::DECALS {
        let at = match d.sprite {
            challenger::DecalSprite::Exclaim => asset::CHALLENGER_EXCLAIM,
            challenger::DecalSprite::Warning => asset::WARNING_TEXT,
            challenger::DecalSprite::Challenger => asset::CHALLENGER_TEXT,
            challenger::DecalSprite::Approaching => asset::APPROACHING_TEXT,
        };
        let color = d.color.unwrap_or([0xFF; 3]);
        sprite(p, st, asset::CHALLENGER.file, at, d.pos[0], d.pos[1], color, [0; 3], 1.0);
    }
    if let Some(m) = pres.models.first() {
        draw_model_ex(
            gpu,
            p,
            st,
            m,
            m.pos,
            1.0,
            (100.0, 12800.0),
            [-20.0, 30.0],
            ModelView {
                camera: m.camera,
                aspect: ASPECT,
                yaw: c.rotate_y,
                // `dGMColScriptsFighterChallenger`: black at full alpha.
                fog: Some([0, 0, 0, 0xFF]),
            },
        );
    }
}

/// `mnMessageFuncStart`'s cameras: the collage (80), the blue tint (70),
/// the exclamation mark (60) and the message (40).
pub(super) unsafe fn draw_message(p: &Pack<'_>, st: &mut DrawState, m: &message::Message) {
    clear_black(st);
    let [wx, wy] = message::WALLPAPER_POSITION;
    sprite(p, st, asset::MESSAGE_COLLAGE.file, asset::COLLAGE, wx, wy, [0xFF; 3], [0; 3], 1.0);
    meshdraw::fill_rect_n64(message::TINT_RECT, message::TINT, st);
    let [ex, ey] = message::EXCLAIM_POSITION;
    sprite(p, st, asset::MESSAGE.file, asset::MESSAGE_EXCLAIM, ex, ey, [0xFF; 3], [0; 3], 1.0);
    let [x, y] = m.position();
    let at = asset::MESSAGE.offsets[m.unlock as usize];
    sprite(p, st, asset::MESSAGE.file, at, x, y, [0xFF; 3], [0; 3], 1.0);
}
