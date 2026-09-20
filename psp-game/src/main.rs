//! Layer C: the PSP front-end/Training-Mode executable (F1).
//!
//! A second, independent PSP application alongside the existing debug asset
//! viewer in `psp-asset-viewer/` -- its own crate, its own EBOOT, sharing the
//! portable `crates/ssb-engine`/`ssb-rom`/`ssb-game` libraries and the shared
//! `psp-runtime` platform/rendering layer (`AGENTS.md`'s F1 carve-out,
//! `plans/gameplay/F1.md`). `psp-asset-viewer/`'s own build and EBOOT are
//! unmodified by this crate's existence.
//!
//! Intro screen and main menu still draw flat coloured rectangles
//! (`ssb_psp_runtime::gu`), proving criterion 1 (a real, independently
//! booting second EBOOT) and the
//! intro/menu navigation shape of criteria 2-3. Training Mode now loads the
//! real pack, spawns a real fighter on a real stage, and draws both through
//! `meshdraw`'s 3D pipeline and `play::FighterScene`'s real physics/animation/camera
//! (`plans/gameplay/F1.md`'s "Scene loading" section) -- real hitbox/damage/
//! knockback combat (criterion 5) and `sceFont` menu/select text are still
//! outstanding, tracked there and in `TODO.md`.

#![no_std]
#![no_main]

// The asset pack is loaded into a heap buffer; `psp` provides the allocator.
extern crate alloc;

mod play;

use ssb_engine::input::{newly_pressed, ControllerState, Input, N64Buttons, SSB64_GAME_MAPPING};
use ssb_engine::renderer::Color;
use ssb_rom::pack::Pack;

use ssb_psp_runtime::assets;
#[cfg(feature = "headless_capture")]
use ssb_psp_runtime::gu::emit_headless_screenshot;
use ssb_psp_runtime::gu::Gpu;
use ssb_psp_runtime::input::PspInput;
use ssb_psp_runtime::meshdraw;

/// Loop-iteration count since boot. `psp-game` has no fixed-timestep sim
/// accumulator yet (unlike `psp-asset-viewer/`'s `Clock`/`FixedClock`), so this is simply
/// the draw-loop tick -- deterministic regardless of host wall-clock speed,
/// which is what `regression_capture`/`headless_capture` need. Only
/// consulted by `deterministic_capture_frozen`/`scripted_buttons`; harmless
/// to maintain unconditionally (`psp-asset-viewer/main.rs`'s own `sim_frame_index`
/// comment).
const DETERMINISTIC_CAPTURE_TICKS: u64 = if cfg!(feature = "regression_capture_shadows") {
    // Training starts at tick 8; C-Up at 13 enters jumpsquat, and this lands
    // in the rising portion of Mario's real button jump while the dummy is
    // still on Dream Land's main floor.
    22
} else if cfg!(feature = "regression_capture_fireball") {
    167
} else if cfg!(feature = "regression_capture_superjump") {
    // B+up is pressed at tick 150; frame 2's strong opening hit has resolved
    // by this point while the source TransN launch is still clearly visible.
    156
} else {
    106
};

/// `true` once `regression_capture`'s scripted input has run past its fixed
/// script and reached its capture tick; always `false` otherwise, so callers
/// need one guard, not a cfg per call site (mirrors `psp-asset-viewer/main.rs`'s function
/// of the same name).
#[inline]
fn deterministic_capture_frozen(sim_frame_index: u64) -> bool {
    sim_frame_index >= DETERMINISTIC_CAPTURE_TICKS && cfg!(feature = "regression_capture")
}

/// A fixed, tick-indexed button script standing in for real `sceCtrl` input
/// under `regression_capture`. `psp-asset-viewer/` has no precedent for this (its own
/// deterministic-capture features only ever freeze *output* -- physics,
/// animation, camera -- never override input, because its viewer has no
/// input-driven state machine to script); `psp-game`'s Intro -> Menu ->
/// Training navigation does, so this is what actually lets
/// PPSSPPHeadless drive and pixel-confirm the Menu -> Training confirm
/// transition deterministically. That transition was RE-289's one open item:
/// synthetic X11 key injection into a windowed PPSSPP instance (tried with
/// both the project's Xlib fallback and `xdotool`) raced this desktop's
/// Wayland/XWayland compositor focus arbitration and could not reliably
/// deliver the key, independent of which injection tool sent it -- and it
/// also takes over real keyboard focus on the developer's desktop while it
/// runs. Headless capture has neither problem.
///
/// Tick 4 confirms past the Intro screen. Tick 8 confirms Training: the menu
/// cursor starts on `TRAINING_ENTRY` (`cursor: usize = 0` below), so no
/// D-pad navigation is needed first. Training's first fighter tick is that
/// same tick 8 (`play_state`/`dummy_state` are created and ticked once
/// within the same loop iteration as the confirm), so every tick below this
/// point is expressed relative to that: "local tick N" (from
/// `tools/romtool`'s `jumptest` subcommand, run against the real
/// pack's Dream Land floor data, RE-295) is real tick `8 + N`.
///
/// Ticks 13/33 (local 5/25) are C-button jump taps (`ftCommonKneeBendCheck
/// ButtonTap`'s `R_CBUTTONS|L_CBUTTONS|D_CBUTTONS|U_CBUTTONS`, real bitwise
/// C-buttons, not a debug stand-in -- see [`JUMP_BUTTON_MASK`]): the first is
/// a vertical button jump (jumpsquat only, no stick) so it clears the real
/// second spawn point's 660-unit single-jump ceiling by height alone; the
/// second is a midair jump timed to reset onto the platform's line rather
/// than overshoot it. Ticks 34-89 hold the stick left (toward spawn 1,
/// `-30`) for the horizontal carry a button jump's own velocity formula
/// (`ftCommonJumpGetJumpForceButton`) does not supply; releasing at 90 stops
/// Mario dashing off the platform's far edge before he can act. Tick 98 is
/// the jab's own A tap, timed to land once `LandingLight`'s lag has cleared
/// (real tick 89) and the jab's hitbox window (`anim_frame` 2..4) has swept
/// past the dummy while grounded next to it -- `jumptest`'s trace confirmed
/// the hit with `ssb_game::attack::spheres_overlap` at ticks 100-101 against
/// the real pack's Mario collision width and spawn-1 position, not a guess.
/// Only consulted when `deterministic_capture_frozen` reads
/// `regression_capture` as enabled; harmless to keep unconditionally. The
/// `regression_capture_fireball` variant adds a neutral-B tap at tick 150,
/// after that jab has completed, and freezes at tick 167 so the source
/// frame-16 Fireball spawn is visible. `regression_capture_superjump` uses
/// the same B edge plus an upward stick at tick 150 and freezes after its
/// opening hit window.
fn scripted_buttons(tick: u64) -> N64Buttons {
    if cfg!(feature = "regression_capture_fireball") && tick == 150 {
        return N64Buttons(N64Buttons::B);
    }
    if cfg!(feature = "regression_capture_superjump") && tick == 150 {
        return N64Buttons(N64Buttons::B);
    }

    match tick {
        4 | 8 => N64Buttons(N64Buttons::A),
        13 | 33 => N64Buttons(N64Buttons::C_UP),
        98 => N64Buttons(N64Buttons::A),
        _ => N64Buttons(0),
    }
}

/// The scripted stick under `regression_capture`, alongside
/// [`scripted_buttons`] -- see that function's docs for the tick schedule's
/// derivation. Held left (toward the dummy at spawn 1) only for the carry
/// phase of the scripted jump; neutral otherwise, including during both
/// jumpsquats, so a button jump's height is not traded away for horizontal
/// distance it does not need yet (`ftCommonJumpGetJumpForceButton`'s
/// full-deflection-trades-height-for-distance curve).
fn scripted_stick_x(tick: u64) -> i8 {
    if (34..90).contains(&tick) {
        -30
    } else {
        0
    }
}

/// The Super Jump Punch input shares the source `check_special_hi` gate with
/// live play: a B edge and an upward raw N64 stick value, not a capture-only
/// shortcut. Every other regression scene remains neutral vertically.
fn scripted_stick_y(tick: u64) -> i8 {
    if cfg!(feature = "regression_capture_superjump") && tick == 150 {
        80
    } else {
        0
    }
}

/// Any N64 C-button, real `FTCOMMON_KNEEBEND` jump-by-button input
/// (`ftCommonKneeBendCheckButtonTap`'s `R_CBUTTONS|L_CBUTTONS|D_CBUTTONS|
/// U_CBUTTONS`) -- not a debug stand-in. `SSB64_GAME_MAPPING` maps each PSP
/// D-pad direction to one of these raw N64 C-button bits.
const JUMP_BUTTON_MASK: u16 =
    N64Buttons::C_UP | N64Buttons::C_DOWN | N64Buttons::C_LEFT | N64Buttons::C_RIGHT;

/// N64-stick deflection used by the temporary front end for a single menu
/// move.  This is deliberately a raw N64-stick threshold: the PSP layer has
/// already converted the nub and neither this screen nor gameplay knows PSP
/// button identities.
const MENU_STICK_NAV_MIN: i8 = 40;

fn menu_stick_down_pressed(previous: ControllerState, current: ControllerState) -> bool {
    previous.stick_y > -MENU_STICK_NAV_MIN && current.stick_y <= -MENU_STICK_NAV_MIN
}

fn menu_stick_up_pressed(previous: ControllerState, current: ControllerState) -> bool {
    previous.stick_y < MENU_STICK_NAV_MIN && current.stick_y >= MENU_STICK_NAV_MIN
}

psp::module!("ssb64_psp_game", 1, 0);

fn psp_main() {
    psp::enable_home_button();
    unsafe { run() }
}

/// Which screen is active. Deliberately just these three: nothing here may
/// grow stocks, a match timer, CPU AI, or items without widening `AGENTS.md`'s
/// F1 carve-out first.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Screen {
    Intro,
    Menu,
    /// Training Mode: a real stage and a real, physics-ticked fighter now
    /// draw here (`draw_training`) -- no combat yet, see
    /// `plans/gameplay/F1.md` acceptance criteria 5-7 for what still has to
    /// land.
    Training,
}

/// Main-menu entries. Only `Training` is selectable; the others are visible,
/// inert placeholders (`plans/gameplay/F1.md` allows this explicitly).
const MENU_ENTRIES: usize = 3;
const TRAINING_ENTRY: usize = 0;

const BG_INTRO: Color = Color::rgba(24, 32, 64, 255);
const BG_MENU: Color = Color::rgba(16, 16, 24, 255);
const BG_TRAINING: Color = Color::rgba(20, 48, 24, 255);
/// Training background when the asset pack failed to load or parse. Distinct
/// from `BG_TRAINING` so pack status is pixel-provable under PPSSPPHeadless
/// without `sceFont` text (`plans/gameplay/F1.md`'s "Scene loading" section
/// -- real on-screen text is later F1 work, not this increment).
const BG_TRAINING_NO_PACK: Color = Color::rgba(80, 16, 16, 255);
/// `assets::LoadError::Empty` -- the file opened but reported zero length.
const BG_TRAINING_PACK_EMPTY: Color = Color::rgba(160, 100, 0, 255);
/// `assets::LoadError::OutOfMemory` -- the heap allocation for the pack
/// buffer failed.
const BG_TRAINING_OUT_OF_MEMORY: Color = Color::rgba(120, 0, 160, 255);
/// `assets::LoadError::ShortRead` -- `sceIoRead` returned fewer bytes than
/// the file's reported size (seen over PSPLink's `host0:`, RE-296).
const BG_TRAINING_SHORT_READ: Color = Color::rgba(200, 200, 0, 255);
/// The pack opened and read fully but `ssb_rom::pack::Pack::open` rejected
/// its contents (bad magic/version/bounds).
const BG_TRAINING_PARSE_FAILED: Color = Color::rgba(0, 90, 170, 255);
const ENTRY_SELECTED: Color = Color::rgba(255, 200, 40, 255);
const ENTRY_ENABLED: Color = Color::rgba(200, 200, 200, 255);
const ENTRY_DISABLED: Color = Color::rgba(70, 70, 70, 255);

/// Which packed stage Training Mode loads. Dream Land (file 104) -- stage
/// index 0, matching every other build in this project's own default/unset
/// convention (`psp-asset-viewer/Cargo.toml`'s `regression_capture_stage_index` doc: "0
/// (Dream Land) if unset").
const TRAINING_STAGE_INDEX: u32 = 0;

unsafe fn run() -> ! {
    let mut gpu = Gpu::init();
    // Select this application’s layout at the PSP backend boundary.  The
    // asset viewer keeps PspInput::init() and therefore its legacy controls.
    let mut pad = PspInput::init_with_mapping(SSB64_GAME_MAPPING);

    // Load the converted asset pack. Held for the whole program: the GE
    // reads vertex and texture data out of it by DMA once the training
    // scene draws real meshes below.
    let loaded = assets::load_pack();
    let pack_buf = loaded.as_ref().ok().map(|(b, _)| b);
    let opened = pack_buf.map(|b| Pack::open(b.as_slice()));
    // Which flat colour `draw_training` falls back to when there is no scene
    // to draw -- distinguishes *why* (open/read failure vs. a rejected
    // header) without needing `sceFont` text, extending the pixel-provable
    // convention `plans/gameplay/F1.md`'s "Scene loading" section already
    // established for the plain not-loaded case.
    let no_pack_color = match (&loaded, &opened) {
        (Err(assets::LoadError::NotFound), _) => BG_TRAINING_NO_PACK,
        (Err(assets::LoadError::Empty), _) => BG_TRAINING_PACK_EMPTY,
        (Err(assets::LoadError::OutOfMemory), _) => BG_TRAINING_OUT_OF_MEMORY,
        (Err(assets::LoadError::ShortRead), _) => BG_TRAINING_SHORT_READ,
        (Ok(_), Some(Err(_))) => BG_TRAINING_PARSE_FAILED,
        (Ok(_), _) => BG_TRAINING_NO_PACK,
    };
    let pack: Option<Pack<'_>> = opened.and_then(|r| r.ok());
    // Stage MObj material joints are process-lifetime clocks in the original
    // layer setup. Start once with this pack and advance in the same simulation
    // branch as the stage/fighter tick; draw only reads the resulting state.
    let mut material_anim = ssb_rom::skeleton::MaterialAnimator::new();
    if let Some(p) = pack.as_ref() {
        material_anim.start(p);
    }

    let mut draw_state = meshdraw::DrawState::default();
    // Created once, on first entry to Training Mode (below) -- a fighter
    // spawned on the training stage, ticked with real physics/animation/
    // camera every frame this screen is active (`play::FighterScene`, shared
    // with `psp-asset-viewer/` via `ssb_psp_runtime::scene`).
    let mut play_state: Option<play::FighterScene> = None;
    // The stationary dummy target (`play::Dummy`, `psp-game`-only -- see its
    // doc comment): spawned alongside `play_state` at the stage's second
    // spawn point, ticked with permanently neutral input.
    let mut dummy_state: Option<play::Dummy> = None;
    // Match-owned spawned weapons. Fighter statuses emit portable requests;
    // Training owns the pool because it is the layer that has both fighters
    // and the stage collision iterator.
    let mut weapons = ssb_game::weapon::WeaponPool::default();

    let mut screen = Screen::Intro;
    let mut cursor: usize = 0;
    let mut sim_frame_index: u64 = 0;
    #[cfg(feature = "headless_capture")]
    let mut headless_capture_sent = false;

    loop {
        sim_frame_index = sim_frame_index.saturating_add(1);
        pad.poll();
        let (previous_controller, controller) = if cfg!(feature = "regression_capture") {
            (
                ControllerState {
                    buttons: scripted_buttons(sim_frame_index.saturating_sub(1)),
                    stick_x: scripted_stick_x(sim_frame_index.saturating_sub(1)),
                    stick_y: scripted_stick_y(sim_frame_index.saturating_sub(1)),
                    connected: true,
                },
                ControllerState {
                    buttons: scripted_buttons(sim_frame_index),
                    stick_x: scripted_stick_x(sim_frame_index),
                    stick_y: scripted_stick_y(sim_frame_index),
                    connected: true,
                },
            )
        } else {
            (pad.previous(0), pad.state(0))
        };
        let pressed = newly_pressed(previous_controller.buttons, controller.buttons);

        if !deterministic_capture_frozen(sim_frame_index) {
            match screen {
                Screen::Intro => {
                    if pressed.contains(N64Buttons::A) || pressed.contains(N64Buttons::START) {
                        screen = Screen::Menu;
                    }
                }
                Screen::Menu => {
                    if menu_stick_down_pressed(previous_controller, controller) {
                        cursor = (cursor + 1) % MENU_ENTRIES;
                    } else if menu_stick_up_pressed(previous_controller, controller) {
                        cursor = (cursor + MENU_ENTRIES - 1) % MENU_ENTRIES;
                    } else if pressed.contains(N64Buttons::A) && cursor == TRAINING_ENTRY {
                        screen = Screen::Training;
                        if play_state.is_none() {
                            weapons = ssb_game::weapon::WeaponPool::default();
                            play_state = pack.as_ref().and_then(|p| {
                                p.stage(TRAINING_STAGE_INDEX).map(|s| {
                                    play::FighterScene::at_spawn(
                                        p,
                                        &s,
                                        ssb_game::fighter::FighterKind::Mario,
                                        0,
                                    )
                                })
                            });
                            dummy_state = pack.as_ref().and_then(|p| {
                                p.stage(TRAINING_STAGE_INDEX)
                                    .and_then(|s| play::Dummy::at_spawn(p, &s))
                            });
                        }
                    }
                }
                Screen::Training => {
                    // START is navigation-only here. B belongs to the fighter's
                    // source special-input path and must reach `pl.tick` below.
                    if pressed.contains(N64Buttons::START) {
                        screen = Screen::Menu;
                    }
                }
            }

            if let (Screen::Training, Some(p), Some(pl)) = (screen, &pack, play_state.as_mut()) {
                material_anim.tick(p);
                if let Some(stage) = p.stage(TRAINING_STAGE_INDEX) {
                    // Real `sceCtrl` stick input drives real movement/physics/
                    // animation against the real stage collision, the same
                    // `Play::tick` `psp-asset-viewer/`'s own gameplay slice uses. Under
                    // `regression_capture`, real pad state is replaced by the
                    // scripted script (RE-295) rather than zeroed -- a
                    // deterministic capture of gameplay input (the jab, now
                    // the jump) needs to actually *drive* that input, not
                    // discard it; only the source is scripted, not the game
                    // logic it feeds.
                    // Real jump binding (RE-295): any N64 C-button tap is a
                    // real `FTCOMMON_KNEEBEND` button-jump input
                    // (`ftCommonKneeBendCheckButtonTap`). An upward stick
                    // flick is the game's other real jump input and needs no
                    // separate wiring here: `Fighter::tick`'s own status
                    // machine reads `stick_y` directly.
                    let jump_held = controller.buttons.contains(JUMP_BUTTON_MASK);
                    pl.tick(p, &stage, controller, jump_held, None);
                    if let Some(spawn) = pl.fighter.take_weapon_spawn() {
                        weapons.spawn(spawn);
                    }
                    if let Some(dummy) = dummy_state.as_mut() {
                        dummy.tick(p, &stage);
                        if let Some(spawn) = dummy.fighter.take_weapon_spawn() {
                            weapons.spawn(spawn);
                        }
                        weapons.tick(|| ssb_psp_runtime::scene::MapSegments::new(p, &stage));
                        weapons.apply_hits(&mut pl.fighter);
                        weapons.apply_hits(&mut dummy.fighter);
                        dummy.apply_hit_from(&pl.fighter);
                    }
                }
            }
        }

        match screen {
            Screen::Intro => {
                gpu.set_viewport_fullscreen();
                gpu.begin_frame(Some(BG_INTRO));
            }
            Screen::Menu => {
                gpu.set_viewport_fullscreen();
                gpu.begin_frame(Some(BG_MENU));
                draw_menu(&mut gpu, cursor);
            }
            Screen::Training => {
                draw_training(
                    &mut gpu,
                    &mut draw_state,
                    pack.as_ref(),
                    play_state.as_ref(),
                    dummy_state.as_ref(),
                    &weapons,
                    Some(&material_anim),
                    no_pack_color,
                );
            }
        }
        gpu.end_frame();

        #[cfg(feature = "headless_capture")]
        if !headless_capture_sent && deterministic_capture_frozen(sim_frame_index) {
            emit_headless_screenshot();
            headless_capture_sent = true;
        }
    }
}

/// Draws the menu entries as a vertical stack of rectangles: one bar per
/// entry, the selected one in `ENTRY_SELECTED`, `Training` in
/// `ENTRY_ENABLED` when not selected, and the stubbed entries dimmed. No text
/// yet (`gu.rs`'s module doc explains why), so entries are distinguished by
/// screen position and enabled/disabled colour rather than a label.
fn draw_menu(gpu: &mut Gpu, cursor: usize) {
    const ENTRY_HEIGHT: i32 = 32;
    const ENTRY_GAP: i32 = 16;
    const ENTRY_WIDTH: i32 = 200;
    const LEFT: i32 = 40;
    const TOP: i32 = 60;

    for i in 0..MENU_ENTRIES {
        let y0 = TOP + i as i32 * (ENTRY_HEIGHT + ENTRY_GAP);
        let color = if i == cursor {
            ENTRY_SELECTED
        } else if i == TRAINING_ENTRY {
            ENTRY_ENABLED
        } else {
            ENTRY_DISABLED
        };
        gpu.draw_rect(LEFT, y0, LEFT + ENTRY_WIDTH, y0 + ENTRY_HEIGHT, color);
    }
}

/// Draws the training scene: the real stage and the real spawned fighter,
/// through the real battle camera (`play::FighterScene::camera`) -- the first
/// `psp-game` content built from `meshdraw`'s 3D pipeline rather than
/// `gu::Gpu::draw_rect`'s flat placeholder rectangles.
///
/// Falls back to a flat colour keyed to *why* (see the `BG_TRAINING_*`
/// consts and `no_pack_color`'s computation in `run`) when the pack failed
/// to load/parse or the stage isn't in it -- still the pixel-provable
/// signal `plans/gameplay/F1.md`'s "Scene loading" section established (no
/// `sceFont` text exists yet to say so in words), now distinguishing the
/// failure reason too (RE-296).
unsafe fn draw_training(
    gpu: &mut Gpu,
    draw_state: &mut meshdraw::DrawState,
    pack: Option<&Pack<'_>>,
    play_state: Option<&play::FighterScene>,
    dummy_state: Option<&play::Dummy>,
    weapons: &ssb_game::weapon::WeaponPool,
    material_anim: Option<&ssb_rom::skeleton::MaterialAnimator>,
    no_pack_color: Color,
) {
    let scene = pack
        .zip(play_state)
        .and_then(|(p, pl)| p.stage(TRAINING_STAGE_INDEX).map(|s| (p, pl, s)));

    let Some((p, pl, stage)) = scene else {
        gpu.set_viewport_fullscreen();
        gpu.begin_frame(Some(no_pack_color));
        return;
    };

    gpu.begin_frame(Some(BG_TRAINING));
    gpu.set_viewport_pillarboxed();
    let (_, _, vw, vh) = ssb_engine::coord::pillarboxed_viewport();
    // 38 degrees: the real battle camera's own default FOV
    // (`refs/ssb-decomp-re/src/gm/gmcamera.c:1191`, matching `psp-asset-viewer/main.rs`'s
    // own sourced value). Far plane fixed rather than bounds-fitted like the
    // debug viewer's `dbg_cam`: Training has one known stage, not an
    // arbitrary archive entry to frame sight-unseen.
    gpu.set_perspective(38.0, vw as f32 / vh as f32, 1.0, 10_000.0);
    gpu.reset_modelview();
    draw_state.begin_frame();

    gpu.set_view(&ssb_engine::math::Mat4::look_at(
        pl.camera.eye,
        pl.camera.at,
        ssb_engine::math::Vec3::Y,
    ));
    gpu.model_transform([0.0, 0.0, 0.0], [0.0, 0.0, 0.0], meshdraw::MODEL_SCALE);
    let base = gpu.model_matrix();

    meshdraw::draw_stage(p, &stage, &base, draw_state, material_anim);

    // The N64 puts shadows on their own display link between the stage and
    // fighters.  Resolve each independently from its live floor/air state;
    // the fixed scratch is copied into GE memory by the renderer, so it is
    // safe to reuse for both players without a per-frame allocation.
    let mut shadow_verts = [meshdraw::TexQuadVertex::default(); 18];
    let player_shadow = ssb_psp_runtime::scene::fighter_shadow(
        p,
        &stage,
        &pl.fighter,
        pl.shadow_size,
    );
    meshdraw::draw_fighter_shadow(
        p,
        &player_shadow,
        [0x00, 0x00, 0x00, 0xA0],
        &mut shadow_verts,
        draw_state,
    );
    if let Some(dummy) = dummy_state {
    let dummy_shadow = ssb_psp_runtime::scene::fighter_shadow(
        p,
        &stage,
        &dummy.fighter,
        dummy.shadow_size,
    );
        meshdraw::draw_fighter_shadow(
            p,
            &dummy_shadow,
            [0x00, 0x00, 0x00, 0xA0],
            &mut shadow_verts,
            draw_state,
        );
    }

    if let Some(fireball_mesh) = ssb_psp_runtime::scene::mario_fireball_mesh(p) {
        for fireball in weapons.fireballs() {
            // `wpMainVelSetModelPitch` uses +/-90 degrees around Y from
            // horizontal velocity; the packed direct-display-list mesh gets
            // that same model orientation here.
            let yaw = if fireball.velocity.x >= 0.0 {
                core::f32::consts::FRAC_PI_2
            } else {
                -core::f32::consts::FRAC_PI_2
            };
            gpu.model_transform(
                [
                    fireball.position.x,
                    fireball.position.y,
                    fireball.position.z,
                ],
                [0.0, yaw, 0.0],
                meshdraw::MODEL_SCALE,
            );
            meshdraw::draw_mesh(p, &fireball_mesh, draw_state, None, None);
        }
    }

    if let Some(obj) = p.object(pl.object) {
        let mut posed = [ssb_rom::scene::Mat4::IDENTITY; ssb_rom::skeleton::MAX_NODES];
        let n = pl.skeleton.compose(p, &obj, &mut posed);
        gpu.model_transform(
            [pl.fighter.pos.x, pl.fighter.pos.y, pl.fighter.pos.z],
            [0.0, play::facing_turn(pl.fighter.facing), 0.0],
            meshdraw::MODEL_SCALE,
        );
        let m = gpu.model_matrix();
        // `ftDisplayMainProcDisplay` rebuilds the fighter's one directional
        // light from the active stage's `MPGroundData.light_angle.x/y`
        // immediately before drawing each fighter (RE-164) -- matches
        // `psp-asset-viewer/main.rs`'s own real-camera fighter draw.
        draw_state.configure_fighter_light(stage.light_angle_xy);
        meshdraw::draw_object_posed(p, &obj, &m, &posed[..n], None, draw_state, None, None, 0);
        draw_state.finish_fighter_light();
    }

    // The stationary dummy target (`play::Dummy`), drawn the same way as the
    // player's fighter -- its own pose, its own per-fighter light rebuild
    // (RE-164) -- just with no camera interest of its own (F1's target
    // doesn't move, so it never influences framing).
    if let Some(dummy) = dummy_state {
        if let Some(obj) = p.object(dummy.object) {
            let mut posed = [ssb_rom::scene::Mat4::IDENTITY; ssb_rom::skeleton::MAX_NODES];
            let n = dummy.skeleton.compose(p, &obj, &mut posed);
            gpu.model_transform(
                [
                    dummy.fighter.pos.x,
                    dummy.fighter.pos.y,
                    dummy.fighter.pos.z,
                ],
                [0.0, play::facing_turn(dummy.fighter.facing), 0.0],
                meshdraw::MODEL_SCALE,
            );
            let m = gpu.model_matrix();
            draw_state.configure_fighter_light(stage.light_angle_xy);
            meshdraw::draw_object_posed(p, &obj, &m, &posed[..n], None, draw_state, None, None, 0);
            draw_state.finish_fighter_light();
        }
    }
}
