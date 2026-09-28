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

mod capture;
mod play;

use ssb_engine::input::{newly_pressed, ControllerState, Input, N64Buttons, SSB64_GAME_MAPPING};
use ssb_engine::renderer::Color;
use ssb_rom::pack::Pack;

use ssb_psp_runtime::assets;

use capture::GameScene;
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
const fn capture_ticks(scene: GameScene) -> u64 {
    match scene {
        // Training starts at tick 8; C-Up at 13 enters jumpsquat, and this
        // lands in the rising portion of Mario's real button jump while the
        // dummy is still on Dream Land's main floor.
        GameScene::Shadows => 22,
        GameScene::Fireball => 167,
        // B+up is pressed at tick 150; frame 2's strong opening hit has
        // resolved by this point while the source TransN launch is still
        // clearly visible.
        GameScene::Superjump => 156,
        GameScene::Fox => 46,
        // B at tick 20, as in the Fox scene; the Fireball is in flight.
        GameScene::Luigi => 46,
        GameScene::Training => 106,
        // Z+A at tick 108; the catch box is live on `Catch` frame 6, the
        // two-frame pull follows, and the dummy then hangs in `CaptureWait`.
        GameScene::Grab => 118,
        // A at tick 108 lands on tick 109 (host `romtool jumptest`); the
        // dummy's hitlag is over and it is in `DamageN1`.
        GameScene::Jab => 118,
        // Z begins at tick 40; the diagonal stick is applied after GuardOn
        // has entered Guard, then frozen with the shield still raised.
        GameScene::Shield => 60,
        // Both fighters have settled on Dream Land's main floor.
        GameScene::Costume1 | GameScene::Costume2 | GameScene::Costume3 => 40,
    }
}

/// `true` once `regression_capture`'s scripted input has run past its fixed
/// script and reached its capture tick; always `false` otherwise, so callers
/// need one guard, not a cfg per call site (mirrors `psp-asset-viewer/main.rs`'s function
/// of the same name).
#[inline]
fn deterministic_capture_frozen(scene: Option<GameScene>, sim_frame_index: u64) -> bool {
    scene.is_some_and(|scene| sim_frame_index >= capture_ticks(scene))
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
/// RE-341 found that the current build's jab no longer damages the dummy.
/// Only consulted when `deterministic_capture_frozen` reads
/// `regression_capture` as enabled; harmless to keep unconditionally. The
/// `regression_capture_fireball` variant adds a neutral-B tap at tick 150,
/// after that jab has completed, and freezes at tick 167 so the source
/// frame-16 Fireball spawn is visible. `regression_capture_superjump` uses
/// the same B edge plus an upward stick at tick 150 and freezes after its
/// opening hit window.
fn scripted_buttons(scene: GameScene, tick: u64) -> N64Buttons {
    // The costume scenes tap one C-button on the Training entry at tick 6,
    // between the Intro and Training confirms (`mnPlayers1PTrainingUpdateCostume`).
    if let Some(pick) = match scene {
        GameScene::Costume1 => Some(N64Buttons::C_RIGHT),
        GameScene::Costume2 => Some(N64Buttons::C_DOWN),
        GameScene::Costume3 => Some(N64Buttons::C_LEFT),
        _ => None,
    } {
        return match tick {
            4 | 8 => N64Buttons(N64Buttons::A),
            6 => N64Buttons(pick),
            _ => N64Buttons(0),
        };
    }
    if matches!(scene, GameScene::Fox | GameScene::Luigi) && tick == 20 {
        return N64Buttons(N64Buttons::B);
    }
    if scene == GameScene::Fireball && tick == 150 {
        return N64Buttons(N64Buttons::B);
    }
    if scene == GameScene::Superjump && tick == 150 {
        return N64Buttons(N64Buttons::B);
    }
    // The grab scene has its own route onto the dummy's platform, found
    // with `romtool jumptest --jump-tick 5 --jump2-tick 22 --stick-x -30
    // --stick-switch-tick 6 --stick-release-tick 44` (local ticks): Mario
    // lands at x -1253, 144 units right of the dummy. A left tap at tick
    // 98 turns Mario to face the dummy; tick 108 is Z held with an A edge
    // (`ftCommonCatchCheckInterruptCommon`).
    if scene == GameScene::Grab {
        return match tick {
            4 | 8 => N64Buttons(N64Buttons::A),
            13 | 30 => N64Buttons(N64Buttons::C_UP),
            108 => N64Buttons(N64Buttons::Z | N64Buttons::A),
            _ => N64Buttons(0),
        };
    }
    // The jab scene takes the grab's route and taps A where the grab
    // pressed Z+A (RE-351).
    if scene == GameScene::Jab {
        return match tick {
            4 | 8 | 108 => N64Buttons(N64Buttons::A),
            13 | 30 => N64Buttons(N64Buttons::C_UP),
            _ => N64Buttons(0),
        };
    }
    if scene == GameScene::Shield {
        return match tick {
            4 | 8 => N64Buttons(N64Buttons::A),
            40..=60 => N64Buttons(N64Buttons::Z),
            _ => N64Buttons(0),
        };
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
fn scripted_stick_x(scene: GameScene, tick: u64) -> i8 {
    if scene == GameScene::Shield {
        return if (50..=60).contains(&tick) { 40 } else { 0 };
    }
    if scene == GameScene::Grab && tick == 98 {
        // The route ends facing right. The active Catch pose reaches right,
        // away from spawn 1, unless a fresh reverse tap completes a turn.
        return -80;
    }
    if matches!(scene, GameScene::Grab | GameScene::Jab) {
        return if (14..52).contains(&tick) { -30 } else { 0 };
    }
    if matches!(
        scene,
        GameScene::Costume1 | GameScene::Costume2 | GameScene::Costume3
    ) {
        return 0;
    }
    if (34..90).contains(&tick) {
        -30
    } else {
        0
    }
}

/// The Super Jump Punch input shares the source `check_special_hi` gate with
/// live play: a B edge and an upward raw N64 stick value, not a capture-only
/// shortcut. Every other regression scene remains neutral vertically.
fn scripted_stick_y(scene: GameScene, tick: u64) -> i8 {
    if scene == GameScene::Shield && (50..=60).contains(&tick) {
        40
    } else if scene == GameScene::Superjump && tick == 150 {
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

/// The fighter Training spawns for the player: Fox for the Fox capture
/// scene, Luigi for the Luigi scene, Mario otherwise.
fn training_fighter_kind(capture_scene: Option<GameScene>) -> ssb_game::fighter::FighterKind {
    // The costume scenes use Fox so that no pick can collide with the Mario
    // dummy's costume.
    match capture_scene {
        Some(GameScene::Fox | GameScene::Costume1 | GameScene::Costume2 | GameScene::Costume3) => {
            ssb_game::fighter::FighterKind::Fox
        }
        Some(GameScene::Luigi) => ssb_game::fighter::FighterKind::Luigi,
        _ => ssb_game::fighter::FighterKind::Mario,
    }
}

fn menu_stick_down_pressed(previous: ControllerState, current: ControllerState) -> bool {
    previous.stick_y > -MENU_STICK_NAV_MIN && current.stick_y <= -MENU_STICK_NAV_MIN
}

fn menu_stick_up_pressed(previous: ControllerState, current: ControllerState) -> bool {
    previous.stick_y < MENU_STICK_NAV_MIN && current.stick_y >= MENU_STICK_NAV_MIN
}

/// Frames rendered after the screenshot request before a scene-file capture
/// exits (`psp-asset-viewer/src/main.rs` has the same constant and reason).
#[cfg(feature = "golden_capture")]
const CAPTURE_EXIT_FRAMES: u32 = 2;

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
/// `strict_render` builds only: the pack opened but `ssb_rom::strict` found
/// a reference the draw path would silently skip.
#[cfg(feature = "strict_render")]
const BG_TRAINING_STRICT_FAILED: Color = Color::rgba(200, 0, 100, 255);
const ENTRY_SELECTED: Color = Color::rgba(255, 200, 40, 255);
const ENTRY_ENABLED: Color = Color::rgba(200, 200, 200, 255);
const ENTRY_DISABLED: Color = Color::rgba(70, 70, 70, 255);

/// Which packed stage Training Mode loads. Dream Land (file 104) -- stage
/// index 0, matching every other build in this project's own default/unset
/// convention (`psp-asset-viewer/Cargo.toml`'s `regression_capture_stage_index` doc: "0
/// (Dream Land) if unset").
const TRAINING_STAGE_INDEX: u32 = 0;

unsafe fn run() -> ! {
    // The scripted scene this run captures; `None` reads the real pad.
    let capture = capture::select();
    let capture_scene = capture.map(|c| c.scene);
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
    // Strict rendering mode fails fast instead of drawing around a bad
    // reference.
    #[cfg(feature = "strict_render")]
    let (pack, no_pack_color) = match pack {
        Some(p) if ssb_rom::strict::first_issue(&p).is_some() => (None, BG_TRAINING_STRICT_FAILED),
        p => (p, no_pack_color),
    };
    // Stage MObj material joints are process-lifetime clocks in the original
    // layer setup. Start once with this pack and advance in the same simulation
    // branch as the stage/fighter tick; draw only reads the resulting state.
    let mut material_anim = ssb_rom::skeleton::MaterialAnimator::new();
    if let Some(p) = pack.as_ref() {
        material_anim.start(p);
    }
    let mut stage_map = pack.as_ref().and_then(|p| {
        p.stage(TRAINING_STAGE_INDEX).map(|stage| {
            alloc::boxed::Box::new(ssb_psp_runtime::scene::StageMap::new(
                p,
                TRAINING_STAGE_INDEX,
                &stage,
            ))
        })
    });

    let draw_assets = pack.as_ref().map(DrawAssets::resolve).unwrap_or_default();
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
    let mut items = ssb_game::item::ItemPool::default();
    // The stage controller slot (`grMainSetupMakeGround`), backed by the
    // packed objects' priority-5 animation clocks in Training (RE-357).
    let mut stage_objects = ssb_rom::ground_obj::GroundObjects::empty();
    let mut stage_ctl = ssb_game::stage::Stage::none();

    let mut screen = Screen::Intro;
    let mut cursor: usize = 0;
    // The player's costume, picked with a C-button tap on the Training
    // entry (`mnPlayers1PTrainingUpdateCostume`, `ssb_game::costume`).
    let mut player_costume: u8 = 0;
    // The dummy's costume: `mnPlayers1PTrainingInitVars` fills the player's
    // slot first, then gives the CPU slot the first royal costume the player
    // is not wearing (`mnPlayers1PTrainingGetFreeCostume`). It keeps that
    // costume while the player picks.
    let dummy_costume = ssb_game::costume::free_costume(
        ssb_game::fighter::FighterKind::Mario,
        ssb_game::costume::Slot {
            kind: training_fighter_kind(capture_scene),
            costume: player_costume,
        },
    );
    let mut sim_frame_index: u64 = 0;
    #[cfg(feature = "headless_capture")]
    let mut headless_capture_sent = false;
    #[cfg(feature = "golden_capture")]
    let mut frames_after_capture = 0u32;

    loop {
        sim_frame_index = sim_frame_index.saturating_add(1);
        pad.poll();
        let (previous_controller, controller) = if let Some(scene) = capture_scene {
            (
                ControllerState {
                    buttons: scripted_buttons(scene, sim_frame_index.saturating_sub(1)),
                    stick_x: scripted_stick_x(scene, sim_frame_index.saturating_sub(1)),
                    stick_y: scripted_stick_y(scene, sim_frame_index.saturating_sub(1)),
                    connected: true,
                },
                ControllerState {
                    buttons: scripted_buttons(scene, sim_frame_index),
                    stick_x: scripted_stick_x(scene, sim_frame_index),
                    stick_y: scripted_stick_y(scene, sim_frame_index),
                    connected: true,
                },
            )
        } else {
            (pad.previous(0), pad.state(0))
        };
        let pressed = newly_pressed(previous_controller.buttons, controller.buttons);

        if !deterministic_capture_frozen(capture_scene, sim_frame_index) {
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
                    } else if let (Some(button), true) = (
                        ssb_game::costume::select_button(pressed),
                        cursor == TRAINING_ENTRY && play_state.is_none(),
                    ) {
                        // The Training select's C-button costume pick,
                        // denied when the dummy already wears that costume.
                        let dummy = ssb_game::costume::Slot {
                            kind: ssb_game::fighter::FighterKind::Mario,
                            costume: dummy_costume,
                        };
                        if let Some(costume) = ssb_game::costume::pick(
                            training_fighter_kind(capture_scene),
                            button,
                            dummy,
                        ) {
                            player_costume = costume;
                        }
                    } else if pressed.contains(N64Buttons::A) && cursor == TRAINING_ENTRY {
                        screen = Screen::Training;
                        if play_state.is_none() {
                            weapons = ssb_game::weapon::WeaponPool::default();
                            items = ssb_game::item::ItemPool::default();
                            if let Some((p, stage)) = pack
                                .as_ref()
                                .and_then(|p| p.stage(TRAINING_STAGE_INDEX).map(|stage| (p, stage)))
                            {
                                stage_objects =
                                    ssb_rom::ground_obj::GroundObjects::new(p, stage.source_file);
                                // `grMainSetupMakeGround`: any VS stage gets its
                                // controller; others run an empty slot.
                                stage_ctl = match ssb_psp_runtime::scene::StageSetup::new(p, &stage)
                                {
                                    Some(setup) => {
                                        let mut empty = [];
                                        let groups = stage_map.as_mut().map_or(&mut empty[..], |map| {
                                            map.groups.as_mut_slice()
                                        });
                                        ssb_game::stage::Stage::new(
                                            &setup.init(),
                                            groups,
                                            &mut ssb_psp_runtime::scene::StageObjectsPort {
                                                pack: p,
                                                objects: &mut stage_objects,
                                            },
                                        )
                                    }
                                    None => ssb_game::stage::Stage::none(),
                                };
                            }
                            play_state = pack.as_ref().and_then(|p| {
                                p.stage(TRAINING_STAGE_INDEX).map(|s| {
                                    let mut scene = play::FighterScene::at_spawn(
                                        p,
                                        &s,
                                        training_fighter_kind(capture_scene),
                                        0,
                                    );
                                    scene.fighter.costume = player_costume;
                                    scene
                                })
                            });
                            dummy_state = pack.as_ref().and_then(|p| {
                                p.stage(TRAINING_STAGE_INDEX)
                                    .and_then(|s| play::Dummy::at_spawn(p, &s, dummy_costume))
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
                    // Priority 5, Ground link: `gcPlayAnimAll` precedes
                    // every fighter interrupt and the priority-4 controller.
                    let _ = stage_objects.advance(p);
                    if let Some(map) = stage_map.as_mut() {
                        let _ = map.tick(p);
                    }
                    let groups = stage_map
                        .as_ref()
                        .map_or(&[][..], |map| map.groups.as_slice());
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
                    // Priority 5: every fighter's `ftMainProcUpdateInterrupt`.
                    // Grab events land before the partner's own half,
                    // matching the original's direct status writes
                    // (`ssb_game::grab` module docs).
                    items.publish(&mut pl.fighter);
                    pl.tick_fighter_interrupt(p, &stage, controller, jump_held, groups);
                    if let Some(dummy) = dummy_state.as_mut() {
                        ssb_game::grab::exchange(&mut pl.fighter, &mut dummy.fighter);
                        items.publish(&mut dummy.fighter);
                        dummy.tick_interrupt(p, &stage, groups);
                        ssb_game::grab::exchange(&mut dummy.fighter, &mut pl.fighter);
                    }
                    // Priority 4, Ground link: the stage controller.
                    {
                        let mut empty: [ssb_game::map::MapGroup; 0] = [];
                        let groups_mut = stage_map
                            .as_mut()
                            .map_or(&mut empty[..], |map| map.groups.as_mut_slice());
                        // `mpCollisionSetDObjNoID`: a floor's original
                        // line id to its collision group.
                        let line_group = |line: u16| {
                            p.stage_lines(&stage)
                                .find(|l| l.id == line)
                                .map(|l| l.yakumono as u8)
                        };
                        let mut fighters: alloc::vec::Vec<&mut ssb_game::fighter::Fighter> =
                            alloc::vec::Vec::with_capacity(2);
                        fighters.push(&mut pl.fighter);
                        if let Some(dummy) = dummy_state.as_mut() {
                            fighters.push(&mut dummy.fighter);
                        }
                        stage_ctl.tick(
                            &mut fighters,
                            ssb_game::stage::TickInput {
                                groups: groups_mut,
                                objects: &mut ssb_psp_runtime::scene::StageObjectsPort {
                                    pack: p,
                                    objects: &mut stage_objects,
                                },
                                // The groups are being written, so the
                                // controller sees the static map; only the
                                // Twister queries it, on a static floor.
                                map: ssb_game::stage::MapQuery {
                                    surfaces: || {
                                        ssb_psp_runtime::scene::MapSegments::new(p, &stage)
                                    },
                                    line_group: &line_group,
                                },
                                started: true,
                            },
                        );
                    }
                    let groups = stage_map
                        .as_ref()
                        .map_or(&[][..], |map| map.groups.as_slice());
                    // Priority 4, Fighter link: `ftMainProcPhysicsMap`.
                    pl.fighter.occupied_cliff = dummy_state.as_ref().and_then(|dummy| {
                        ssb_game::map::is_cliff_hold(dummy.fighter.status.status)
                            .then_some((dummy.fighter.cliff.line, dummy.fighter.facing))
                    });
                    pl.tick_fighter_physics(p, &stage, groups);
                    // The Boomerang projects through the camera last drawn.
                    weapons.observe_camera(&pl.camera);
                    pl.tick_camera(&stage, None);
                    items.take_requests(&mut pl.fighter, || {
                        ssb_psp_runtime::scene::MapSegments::with_groups(p, &stage, groups)
                    });
                    if let Some(spawn) = pl.fighter.take_weapon_spawn() {
                        weapons.spawn(spawn);
                    }
                    if let Some(dummy) = dummy_state.as_mut() {
                        dummy.fighter.occupied_cliff =
                            ssb_game::map::is_cliff_hold(pl.fighter.status.status)
                                .then_some((pl.fighter.cliff.line, pl.fighter.facing));
                        ssb_game::grab::exchange(&mut pl.fighter, &mut dummy.fighter);
                        dummy.tick_fighter_physics(p, &stage, groups);
                        items.take_requests(&mut dummy.fighter, || {
                            ssb_psp_runtime::scene::MapSegments::with_groups(p, &stage, groups)
                        });
                        ssb_game::grab::exchange(&mut dummy.fighter, &mut pl.fighter);
                        if let Some(spawn) = dummy.fighter.take_weapon_spawn() {
                            weapons.spawn(spawn);
                        }
                        weapons.observe_owner(&pl.fighter);
                        weapons.observe_owner(&dummy.fighter);
                        let blast_zone = ssb_game::status::BlastZone {
                            top: stage.bounds.top as f32,
                            bottom: stage.bounds.bottom as f32,
                            left: stage.bounds.left as f32,
                            right: stage.bounds.right as f32,
                        };
                        weapons.tick(
                            || ssb_psp_runtime::scene::MapSegments::with_groups(p, &stage, groups),
                            Some(blast_zone),
                        );
                        weapons.sync_owner(&mut pl.fighter);
                        weapons.sync_owner(&mut dummy.fighter);
                        items.observe_owner(&pl.fighter);
                        items.observe_owner(&dummy.fighter);
                        items.tick(
                            || ssb_psp_runtime::scene::MapSegments::with_groups(p, &stage, groups),
                            Some(blast_zone),
                        );
                        items.sync_owner(&mut pl.fighter);
                        items.sync_owner(&mut dummy.fighter);
                        // `ftMainProcSearchCatch`, then `ftMainProcSearchHitAll`
                        // (fighters, then weapons), then `ftMainProcParams` for
                        // every fighter -- the original's process priorities.
                        // `ftMainProcSearchCatch` opens with the obstacle
                        // search (`ftMainSearchHitHazard`).
                        let dummy_status = [dummy.fighter.status.status];
                        ssb_game::hazard::search_hit_hazard(
                            &mut pl.fighter,
                            &mut stage_ctl,
                            &mut ssb_psp_runtime::scene::StageObjectsPort {
                                pack: p,
                                objects: &mut stage_objects,
                            },
                            &dummy_status,
                        );
                        ssb_game::grab::search_catch(&mut pl.fighter, &dummy.fighter);
                        let pl_status = [pl.fighter.status.status];
                        ssb_game::hazard::search_hit_hazard(
                            &mut dummy.fighter,
                            &mut stage_ctl,
                            &mut ssb_psp_runtime::scene::StageObjectsPort {
                                pack: p,
                                objects: &mut stage_objects,
                            },
                            &pl_status,
                        );
                        ssb_game::grab::search_catch(&mut dummy.fighter, &pl.fighter);
                        ssb_game::grab::exchange(&mut pl.fighter, &mut dummy.fighter);
                        ssb_game::grab::exchange(&mut dummy.fighter, &mut pl.fighter);
                        ssb_game::combat::search_all(&mut [&mut pl.fighter, &mut dummy.fighter]);
                        items.search_fighter(&mut pl.fighter);
                        items.search_fighter(&mut dummy.fighter);
                        weapons.apply_hits(&mut pl.fighter);
                        weapons.apply_hits(&mut dummy.fighter);
                        ssb_game::link::apply_spin_attack_hits(&mut pl.fighter, &mut dummy.fighter);
                        ssb_game::link::apply_spin_attack_hits(&mut dummy.fighter, &mut pl.fighter);
                        items.search_hurt(&mut [&mut pl.fighter, &mut dummy.fighter], &mut weapons);
                        // `ftMainSearchGroundHit`, last of `ftMainProcSearchHitAll`.
                        ssb_game::hazard::search_ground_hit(&mut pl.fighter, &stage_ctl);
                        ssb_game::hazard::search_ground_hit(&mut dummy.fighter, &stage_ctl);
                        ssb_game::combat::finish_frame(&mut [&mut pl.fighter, &mut dummy.fighter]);
                        let map =
                            || ssb_psp_runtime::scene::MapSegments::with_groups(p, &stage, groups);
                        pl.fighter.resolve_cliff_release(&map);
                        dummy.fighter.resolve_cliff_release(&map);
                        items.take_requests(&mut pl.fighter, || {
                            ssb_psp_runtime::scene::MapSegments::with_groups(p, &stage, groups)
                        });
                        items.take_requests(&mut dummy.fighter, || {
                            ssb_psp_runtime::scene::MapSegments::with_groups(p, &stage, groups)
                        });
                        items.resolve(&[&pl.fighter, &dummy.fighter]);
                        items.sync_owner(&mut pl.fighter);
                        items.sync_owner(&mut dummy.fighter);
                        items.take_weapon_spawns(&mut weapons, || {
                            ssb_psp_runtime::scene::MapSegments::with_groups(p, &stage, groups)
                        });
                        items.record_landed(&mut pl.fighter);
                        items.record_landed(&mut dummy.fighter);
                        weapons.record_landed(&mut pl.fighter);
                        weapons.record_landed(&mut dummy.fighter);
                        ssb_game::grab::exchange(&mut pl.fighter, &mut dummy.fighter);
                        ssb_game::grab::exchange(&mut dummy.fighter, &mut pl.fighter);
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
                    &draw_assets,
                    Some(&material_anim),
                    stage_map.as_ref().map(|map| &map.animator),
                    Some(&stage_objects),
                    no_pack_color,
                );
            }
        }
        gpu.end_frame();

        #[cfg(feature = "headless_capture")]
        if !headless_capture_sent && deterministic_capture_frozen(capture_scene, sim_frame_index) {
            emit_headless_screenshot();
            // One line for the capture log: whether the scripted attack
            // landed is not always visible (RE-351).
            if let (Some(dummy), Some(player)) = (dummy_state.as_ref(), play_state.as_ref()) {
                let line = alloc::format!(
                    "capture tick={} player_status={:?} player_facing={:?} player_catch={:?} dummy_damage={} dummy_status={:?} dummy_facing={:?} dummy_capture={:?}\n",
                    sim_frame_index,
                    player.fighter.status.status,
                    player.fighter.facing,
                    player.fighter.grab.catch,
                    dummy.fighter.damage,
                    dummy.fighter.status.status,
                    dummy.fighter.facing,
                    dummy.fighter.grab.capture,
                );
                unsafe {
                    psp::sys::sceIoWrite(
                        psp::sys::sceKernelStdout(),
                        line.as_ptr() as *const core::ffi::c_void,
                        line.len(),
                    );
                }
                if capture_scene == Some(GameScene::Shield) {
                    let joint = player.fighter.joint_transforms[3];
                    let shield = ssb_game::combat::shield_transform(&player.fighter);
                    let line = alloc::format!(
                        "shield raised={} joint_present={} angle_sector={} angle_frame={:.2} range={:.3} player=({:.2},{:.2},{:.2}) center=({:.2},{:.2},{:.2}) axis_x=({:.2},{:.2},{:.2})\n",
                        player.fighter.guard.is_shield,
                        joint.is_some(),
                        player.fighter.guard.angle_i,
                        player.fighter.guard.angle_f,
                        player.fighter.guard.shield_rotate_range,
                        player.fighter.pos.x,
                        player.fighter.pos.y,
                        player.fighter.pos.z,
                        shield.origin.x,
                        shield.origin.y,
                        shield.origin.z,
                        shield.axes[0].x,
                        shield.axes[0].y,
                        shield.axes[0].z,
                    );
                    unsafe {
                        psp::sys::sceIoWrite(
                            psp::sys::sceKernelStdout(),
                            line.as_ptr() as *const core::ffi::c_void,
                            line.len(),
                        );
                    }
                }
            }
            headless_capture_sent = true;
        }
        #[cfg(feature = "golden_capture")]
        if headless_capture_sent && capture.is_some_and(|c| c.from_file) {
            frames_after_capture += 1;
            if frames_after_capture > CAPTURE_EXIT_FRAMES {
                psp::sys::sceKernelExitGame();
            }
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

/// Pack descriptors `draw_training` needs every frame. Each lookup scans a
/// whole descriptor table, which cost the PSP-2000 about 12 ms per frame
/// when done per draw (RE-360), so `run` resolves them once.
#[derive(Default)]
struct DrawAssets {
    shadow_texture: Option<ssb_rom::pack::TextureDesc>,
    /// Indexed by `MarioFireball::index`: Mario's palette, then Luigi's.
    fireball_meshes: [Option<ssb_rom::pack::MeshDesc>; 2],
    blaster_mesh: Option<ssb_rom::pack::MeshDesc>,
    reflector: Option<ssb_rom::pack::ObjectDesc>,
}

impl DrawAssets {
    fn resolve(p: &Pack<'_>) -> Self {
        DrawAssets {
            shadow_texture: meshdraw::fighter_shadow_texture(p),
            fireball_meshes: ssb_psp_runtime::scene::fireball_meshes(p),
            blaster_mesh: ssb_psp_runtime::scene::fox_blaster_mesh(p),
            reflector: ssb_psp_runtime::scene::fox_reflector_object(p),
        }
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
    assets: &DrawAssets,
    material_anim: Option<&ssb_rom::skeleton::MaterialAnimator>,
    stage_anim: Option<&ssb_rom::skeleton::StageAnimator>,
    stage_objects: Option<&ssb_rom::ground_obj::GroundObjects>,
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

    // `sc1PGameFuncLights`: the stage light that animated stage light
    // colours are evaluated under (RE-322).
    draw_state.set_stage_light(stage.light_angle_xy);
    meshdraw::draw_stage_animated(
        p,
        &stage,
        &base,
        stage_anim,
        stage_objects,
        draw_state,
        material_anim,
    );

    // The N64 puts shadows on their own display link between the stage and
    // fighters.  Resolve each independently from its live floor/air state;
    // the fixed scratch is copied into GE memory by the renderer, so it is
    // safe to reuse for both players without a per-frame allocation.
    if let Some(shadow_texture) = assets.shadow_texture.as_ref() {
        let mut shadow_verts = [meshdraw::TexQuadVertex::default(); 18];
        let player_shadow =
            ssb_psp_runtime::scene::fighter_shadow(p, &stage, &pl.fighter, pl.shadow_size);
        meshdraw::draw_fighter_shadow(
            p,
            shadow_texture,
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
                shadow_texture,
                &dummy_shadow,
                [0x00, 0x00, 0x00, 0xA0],
                &mut shadow_verts,
                draw_state,
            );
        }
    }

    for fireball in weapons.fireballs() {
        let Some(fireball_mesh) = assets
            .fireball_meshes
            .get(usize::from(fireball.index))
            .and_then(Option::as_ref)
        else {
            continue;
        };
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
        meshdraw::draw_mesh(p, fireball_mesh, draw_state, None, None);
    }

    if let Some(blaster_mesh) = assets.blaster_mesh.as_ref() {
        for shot in weapons.blasters() {
            let pitch = ssb_engine::math::atan2(shot.velocity.y, shot.velocity.x);
            gpu.model_transform_xyz(
                [shot.position.x, shot.position.y, shot.position.z],
                [0.0, 0.0, pitch],
                [
                    meshdraw::MODEL_SCALE * shot.scale_x,
                    meshdraw::MODEL_SCALE,
                    meshdraw::MODEL_SCALE,
                ],
            );
            meshdraw::draw_mesh(p, blaster_mesh, draw_state, None, None);
        }
    }

    if matches!(
        pl.fighter.status.status,
        ssb_game::status::AnyStatus::Fox(
            ssb_game::status::FoxStatus::SpecialLwStart
                | ssb_game::status::FoxStatus::SpecialLwLoop
                | ssb_game::status::FoxStatus::SpecialLwTurn
                | ssb_game::status::FoxStatus::SpecialLwHit
                | ssb_game::status::FoxStatus::SpecialLwEnd
                | ssb_game::status::FoxStatus::SpecialAirLwStart
                | ssb_game::status::FoxStatus::SpecialAirLwLoop
                | ssb_game::status::FoxStatus::SpecialAirLwTurn
                | ssb_game::status::FoxStatus::SpecialAirLwHit
                | ssb_game::status::FoxStatus::SpecialAirLwEnd
        )
    ) {
        if let Some(reflector) = assets.reflector.as_ref() {
            gpu.model_transform(
                [pl.fighter.pos.x, pl.fighter.pos.y, pl.fighter.pos.z],
                [0.0, play::facing_turn(pl.fighter.facing), 0.0],
                meshdraw::MODEL_SCALE,
            );
            let base = gpu.model_matrix();
            meshdraw::draw_object(p, reflector, &base, draw_state, None, 0);
        }
    }

    if let Some(obj) = p.object(pl.object) {
        let mut posed = [ssb_rom::scene::Mat4::IDENTITY; ssb_rom::skeleton::MAX_NODES];
        let n = pl.compose_model(p, &obj, &mut posed);
        if let Some(joint) = pl
            .fighter
            .grab
            .holder
            .and_then(|h| h.anchor_transform)
            .filter(|_| ssb_game::grab::is_held(pl.fighter.status.status))
        {
            gpu.model_transform_joint(pl.fighter.pos, joint, meshdraw::MODEL_SCALE);
        } else {
            gpu.model_transform(
                [pl.fighter.pos.x, pl.fighter.pos.y, pl.fighter.pos.z],
                [0.0, play::facing_turn(pl.fighter.facing), 0.0],
                meshdraw::MODEL_SCALE,
            );
        }
        let m = gpu.model_matrix();
        // `ftDisplayMainProcDisplay` rebuilds the fighter's one directional
        // light from the active stage's `MPGroundData.light_angle.x/y`
        // immediately before drawing each fighter (RE-164) -- matches
        // `psp-asset-viewer/main.rs`'s own real-camera fighter draw.
        draw_state.configure_fighter_light(stage.light_angle_xy);
        meshdraw::draw_object_posed(
            p,
            &obj,
            &m,
            &posed[..n],
            None,
            draw_state,
            None,
            None,
            u32::from(pl.fighter.costume),
        );
        draw_state.finish_fighter_light();
    }

    // The stationary dummy target (`play::Dummy`), drawn the same way as the
    // player's fighter -- its own pose, its own per-fighter light rebuild
    // (RE-164) -- just with no camera interest of its own (F1's target
    // doesn't move, so it never influences framing).
    if let Some(dummy) = dummy_state {
        if let Some(obj) = p.object(dummy.object) {
            let mut posed = [ssb_rom::scene::Mat4::IDENTITY; ssb_rom::skeleton::MAX_NODES];
            let n = dummy.compose_model(p, &obj, &mut posed);
            if let Some(joint) = dummy
                .fighter
                .grab
                .holder
                .and_then(|h| h.anchor_transform)
                .filter(|_| ssb_game::grab::is_held(dummy.fighter.status.status))
            {
                gpu.model_transform_joint(dummy.fighter.pos, joint, meshdraw::MODEL_SCALE);
            } else {
                gpu.model_transform(
                    [
                        dummy.fighter.pos.x,
                        dummy.fighter.pos.y,
                        dummy.fighter.pos.z,
                    ],
                    [0.0, play::facing_turn(dummy.fighter.facing), 0.0],
                    meshdraw::MODEL_SCALE,
                );
            }
            let m = gpu.model_matrix();
            draw_state.configure_fighter_light(stage.light_angle_xy);
            meshdraw::draw_object_posed(
                p,
                &obj,
                &m,
                &posed[..n],
                None,
                draw_state,
                None,
                None,
                u32::from(dummy.fighter.costume),
            );
            draw_state.finish_fighter_light();
        }
    }
}
