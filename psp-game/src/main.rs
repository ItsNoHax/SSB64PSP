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
        // B at tick 20; `SpecialNStart` runs 16 frames, then the charge
        // grows a level every 20 frames. Level 2 is on the arm cannon here.
        GameScene::Samus => 80,
        // A second B at tick 60 ends the charge; the level-1 shot is in
        // flight a few frames later.
        GameScene::SamusShot => 64,
        // Down+B at tick 20; the Bomb spawns on `SpecialLw` frame 10 and
        // rests under Samus. She walks right from tick 80, uncovering it
        // with some 30 frames of its 100-frame fuse left.
        GameScene::SamusBomb => 100,
        // B at tick 20; `SpecialN` throws the Boomerang on frame 26, and it
        // has flown some 14 frames here.
        GameScene::Link => 60,
        // Up+B at tick 20 enters `SpecialHi` with the swirl; 13 swirl
        // frames later its primitive alpha is 204 (RE-324).
        GameScene::LinkSpin => 33,
        // Up+B at tick 20; Egg Throw makes the egg on frame 4 and throws it
        // on frame 23, and it has flown some 16 frames here.
        GameScene::Yoshi => 60,
        // Down+B at tick 20; the Yoshi Bomb hops on its TransN, drops and
        // lands near tick 59, and `SpecialLwLanding` frame 3 makes the two
        // stars. They are some 6 of their 16 frames old here (RE-378).
        GameScene::YoshiBomb => 68,
        // B at tick 20; `SpecialN` makes the flame on frame 42 and stops it
        // on frame 55. It has played some 7 frames here.
        GameScene::Captain => 69,
        // Down+B at tick 20; `SpecialLw` makes the flame on frame 12 and
        // stops it on frame 32. It has played some 12 frames here.
        GameScene::CaptainKick => 44,
        // Up+B at tick 20; Final Cutter rises on its TransN, lands on the
        // top platform near tick 77, and `SpecialHiLanding` frame 3 makes
        // the wave. It is some 6 of its 20 frames old here.
        GameScene::Kirby => 83,
        // B at tick 20; `SpecialN` makes the aerial jolt near tick 44, which
        // lands on its first frame as the ground jolt. Its animation is 8
        // plays into its first 15-frame push cycle here.
        GameScene::Pikachu => 52,
        // C-Up at tick 13 jumps; B at tick 24 enters `SpecialAirN`, which
        // makes the aerial jolt near tick 45. It is some 11 frames into its
        // flight here, still in the air.
        GameScene::PikachuAir => 56,
        // Up+B at tick 20; Sing makes its notes on its first update.
        GameScene::Purin => 50,
        // B at tick 20 starts the Giant Punch; it is charging here.
        GameScene::Donkey => 60,
        // B at tick 20; PK Fire's spark appears near tick 45 and is in
        // flight here.
        GameScene::Ness => 50,
        // Up+B at tick 20; PK Thunder's head rises with its four trails
        // while Ness holds `SpecialHiHold`.
        GameScene::NessThunder => 60,
        // Down+B at tick 20, held; PSI Magnet's field is up in
        // `SpecialLwHold`.
        GameScene::NessMagnet => 50,
        // Down+B at tick 20 pulls a Bomb; it is in Link's hand here.
        GameScene::LinkBomb => 55,
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
        // Hyrule Castle is confirmed at tick 30; both fighters have
        // settled on its floor.
        GameScene::StageSelect => 70,
        // Peach's Castle is confirmed at tick 115; Kirby and the dummy have
        // settled.
        GameScene::FighterSelect => 160,
        GameScene::Rebirth => 300,
        // The battle's first frame is tick 8; "3" shows at tick 128 and
        // "Go" at 398.
        GameScene::Vs => 300,
        // Time runs out at tick 3999 with no KOs, a tie; 94 frozen frames
        // later sudden death starts, says "Go" 90 ticks on, and both
        // fighters stand at 300%.
        GameScene::VsTimeUp => 4200,
        // "TIME UP" holds from tick 3999 for the 90-tick end wait.
        GameScene::VsTimeUpSign => 4040,
        // Sudden death starts at 4093 and says "GO!" 90 ticks on.
        GameScene::VsSuddenDeath => 4150,
        // Paused at tick 500; 60 ticks of the zoom.
        GameScene::VsPause => 560,
        // The VS mode menu after its four inputs.
        GameScene::VsModeMenu => 60,
        // Reset at 520; the results follow Set's three ticks.
        GameScene::VsNoContest => 560,
        // The dummy's CPU has paced for some 190 ticks, or jumped several
        // times.
        GameScene::CpuWalk => 200,
        GameScene::CpuJump => 120,
        // After "Go" the CPU closes in and lands hits (10%); at tick 690 it
        // pulls the player into a grab (`CatchPull`, RE-394).
        GameScene::VsCpu => 690,
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
    // Tick 8 opens the stage select on Peach's Castle, which reads input
    // from its tenth tick (18). D-Right at 20 and 24 moves the cursor to
    // Kongo Jungle, then Hyrule Castle; A at 30 confirms it.
    if scene == GameScene::StageSelect {
        return match tick {
            4 | 8 | 30 => N64Buttons(N64Buttons::A),
            20 | 24 => N64Buttons(N64Buttons::D_RIGHT),
            _ => N64Buttons(0),
        };
    }
    // Tick 8 opens the character select; its first tick is tick 9. The
    // stick (`scripted_stick_x`/`_y`) carries the held puck onto Kirby's
    // portrait, A at 32 places it, and START at 72 (select tick 64, past
    // its 60-tick guard) proceeds 30 ticks later, at 102, to the stage
    // select on Peach's Castle. A at 115 (its thirteenth tick) confirms.
    if scene == GameScene::FighterSelect {
        return match tick {
            4 | 8 | 32 | 115 => N64Buttons(N64Buttons::A),
            72 => N64Buttons(N64Buttons::START),
            _ => N64Buttons(0),
        };
    }
    // The VS scenes move the menu cursor down to VS at tick 6
    // (`scripted_stick_y`) and confirm it at 8.
    if matches!(
        scene,
        GameScene::Vs
            | GameScene::VsTimeUp
            | GameScene::VsTimeUpSign
            | GameScene::VsSuddenDeath
            | GameScene::VsCpu
            | GameScene::VsPause
            | GameScene::VsModeMenu
            | GameScene::VsNoContest
    ) {
        return match tick {
            4 | 8 => N64Buttons(N64Buttons::A),
            // `vsmode`: Rule, to Stock, then Time/Stock, one more stock.
            20 | 40 if scene == GameScene::VsModeMenu => N64Buttons(N64Buttons::D_DOWN),
            30 | 50 if scene == GameScene::VsModeMenu => N64Buttons(N64Buttons::D_RIGHT),
            // `vspause`: START 109 ticks after "Go".
            500 if matches!(scene, GameScene::VsPause | GameScene::VsNoContest) => {
                N64Buttons(N64Buttons::START)
            }
            // `vsnocontest`: A+B+R+Z in the pause menu resets the battle.
            520 if scene == GameScene::VsNoContest => {
                N64Buttons(N64Buttons::A | N64Buttons::B | N64Buttons::R | N64Buttons::Z)
            }
            _ => N64Buttons(0),
        };
    }
    if matches!(scene, GameScene::CpuWalk | GameScene::CpuJump) {
        return match tick {
            4 | 8 => N64Buttons(N64Buttons::A),
            _ => N64Buttons(0),
        };
    }
    if scene == GameScene::Rebirth {
        return match tick {
            4 | 8 => N64Buttons(N64Buttons::A),
            _ => N64Buttons(0),
        };
    }
    // The costume scenes only confirm into Training; their pick is preset
    // (`capture_training_scene`).
    if matches!(
        scene,
        GameScene::Costume1 | GameScene::Costume2 | GameScene::Costume3
    ) {
        return match tick {
            4 | 8 => N64Buttons(N64Buttons::A),
            _ => N64Buttons(0),
        };
    }
    // The Samus, Link, Yoshi and Captain scenes stay on the spawn floor: no
    // jump route.
    if matches!(
        scene,
        GameScene::Samus
            | GameScene::SamusShot
            | GameScene::SamusBomb
            | GameScene::Link
            | GameScene::LinkSpin
            | GameScene::Yoshi
            | GameScene::YoshiBomb
            | GameScene::Captain
            | GameScene::CaptainKick
            | GameScene::Kirby
            | GameScene::Pikachu
            | GameScene::Purin
            | GameScene::Donkey
            | GameScene::Ness
            | GameScene::NessThunder
            | GameScene::NessMagnet
            | GameScene::LinkBomb
    ) {
        return match tick {
            4 | 8 => N64Buttons(N64Buttons::A),
            20 => N64Buttons(N64Buttons::B),
            60 if scene == GameScene::SamusShot => N64Buttons(N64Buttons::B),
            // PSI Magnet lasts while B is held.
            t if scene == GameScene::NessMagnet && t > 20 => N64Buttons(N64Buttons::B),
            _ => N64Buttons(0),
        };
    }
    if scene == GameScene::PikachuAir {
        return match tick {
            4 | 8 => N64Buttons(N64Buttons::A),
            13 => N64Buttons(N64Buttons::C_UP),
            24 => N64Buttons(N64Buttons::B),
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
    // lands at x -1253, 144 units right of the dummy, still facing left
    // from his spawn at x = 0 (RE-387); tick 108 is Z held with an A edge
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
    // Eleven ticks at 80 move the cursor 44 pixels right, from x 70 to 114.
    if scene == GameScene::FighterSelect {
        return if (12..=22).contains(&tick) { 80 } else { 0 };
    }
    if scene == GameScene::Shield {
        return if (50..=60).contains(&tick) { 40 } else { 0 };
    }
    // Held left, Mario dashes off the main floor's left end.
    if scene == GameScene::Rebirth {
        return if (20..=100).contains(&tick) { -80 } else { 0 };
    }
    if matches!(
        scene,
        GameScene::Vs
            | GameScene::VsTimeUp
            | GameScene::VsTimeUpSign
            | GameScene::VsSuddenDeath
            | GameScene::VsCpu
            | GameScene::VsPause
            | GameScene::VsModeMenu
            | GameScene::VsNoContest
            | GameScene::CpuWalk
            | GameScene::CpuJump
    ) {
        return 0;
    }
    if matches!(scene, GameScene::Grab | GameScene::Jab) {
        return if (14..52).contains(&tick) { -30 } else { 0 };
    }
    if matches!(
        scene,
        GameScene::Costume1
            | GameScene::Costume2
            | GameScene::Costume3
            | GameScene::Samus
            | GameScene::SamusShot
            | GameScene::Link
            | GameScene::LinkSpin
            | GameScene::Yoshi
            | GameScene::YoshiBomb
            | GameScene::Captain
            | GameScene::CaptainKick
            | GameScene::Kirby
            | GameScene::Pikachu
            | GameScene::PikachuAir
            | GameScene::Purin
            | GameScene::Donkey
            | GameScene::Ness
            | GameScene::NessThunder
            | GameScene::NessMagnet
            | GameScene::LinkBomb
            | GameScene::StageSelect
    ) {
        return 0;
    }
    if scene == GameScene::SamusBomb {
        return if (80..100).contains(&tick) { 80 } else { 0 };
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
    if matches!(
        scene,
        GameScene::Vs
            | GameScene::VsTimeUp
            | GameScene::VsTimeUpSign
            | GameScene::VsSuddenDeath
            | GameScene::VsCpu
            | GameScene::VsPause
            | GameScene::VsModeMenu
            | GameScene::VsNoContest
    ) {
        return if tick == 6 { -80 } else { 0 };
    }
    // Seventeen ticks at 80 move the cursor 68 pixels up, from y 170 to
    // 102: the held puck's centre lands on Kirby's portrait.
    if scene == GameScene::FighterSelect {
        return if (12..=28).contains(&tick) { 80 } else { 0 };
    }
    if scene == GameScene::Shield && (50..=60).contains(&tick) {
        40
    } else if (scene == GameScene::Superjump && tick == 150)
        || (matches!(
            scene,
            GameScene::LinkSpin
                | GameScene::Yoshi
                | GameScene::Kirby
                | GameScene::Purin
                | GameScene::NessThunder
        ) && tick == 20)
    {
        80
    } else if (matches!(
        scene,
        GameScene::SamusBomb | GameScene::YoshiBomb | GameScene::CaptainKick | GameScene::LinkBomb
    ) && tick == 20)
        || (scene == GameScene::NessMagnet && tick >= 20)
    {
        // The special-low check's downward stick with the B edge.
        -80
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

/// The capture log's state lines (RE-351): whether a scripted attack landed
/// and which weapons, items and statuses a scene reached is not always
/// visible. Kept out of `run` so its code does not push `run`'s branches out
/// of MIPS range.
#[cfg(feature = "headless_capture")]
#[inline(never)]
fn log_capture_state(
    capture_scene: Option<GameScene>,
    sim_frame_index: u64,
    player: &play::FighterScene,
    dummy: &play::Dummy,
    weapons: &ssb_game::weapon::WeaponPool,
    items: &ssb_game::item::ItemPool,
    battle: Option<&ssb_game::battle::Battle>,
) {
    if let Some(b) = battle {
        let line = alloc::format!(
            "battle status={:?} end={:?} time_remain={} time_passed={} winner={:?}\n",
            b.status,
            b.end,
            b.time_remain,
            b.time_passed,
            b.winner(),
        );
        unsafe {
            psp::sys::sceIoWrite(
                psp::sys::sceKernelStdout(),
                line.as_ptr() as *const core::ffi::c_void,
                line.len(),
            );
        }
    }
    let line = alloc::format!(
        "capture tick={} player_damage={} player_status={:?} player_facing={:?} player_catch={:?} dummy_damage={} dummy_status={:?} dummy_facing={:?} dummy_capture={:?}\n",
        sim_frame_index,
        player.fighter.damage,
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
    if matches!(capture_scene, Some(GameScene::Yoshi | GameScene::YoshiBomb)) {
        let line = alloc::format!(
            "yoshi anim_frame={:.1} egg_held={} eggs={} stars={}\n",
            player.fighter.status.anim_frame,
            player.fighter.yoshi.egg_held,
            weapons.eggs().count(),
            weapons.stars().count(),
        );
        unsafe {
            psp::sys::sceIoWrite(
                psp::sys::sceKernelStdout(),
                line.as_ptr() as *const core::ffi::c_void,
                line.len(),
            );
        }
    }
    if matches!(
        capture_scene,
        Some(GameScene::Pikachu | GameScene::PikachuAir)
    ) {
        let line = alloc::format!(
            "pikachu status={:?} jolts={:?}\n",
            player.fighter.status.status,
            weapons
                .jolts()
                .map(|j| (
                    j.surface.is_some(),
                    j.anim_epoch,
                    j.anim_ticks,
                    j.position.x
                ))
                .collect::<alloc::vec::Vec<_>>(),
        );
        unsafe {
            psp::sys::sceIoWrite(
                psp::sys::sceKernelStdout(),
                line.as_ptr() as *const core::ffi::c_void,
                line.len(),
            );
        }
    }
    if matches!(
        capture_scene,
        Some(
            GameScene::Donkey
                | GameScene::Ness
                | GameScene::NessThunder
                | GameScene::NessMagnet
                | GameScene::LinkBomb
        )
    ) {
        let line = alloc::format!(
            "fighter status={:?} anim_frame={:.1} items={:?} sparks={} heads={} trails={}\n",
            player.fighter.status.status,
            player.fighter.status.anim_frame,
            items
                .items()
                .map(|i| (i.kind, i.anim_ticks, i.pos.x, i.scale.x, i.hidden))
                .collect::<alloc::vec::Vec<_>>(),
            weapons.pk_fires().count(),
            weapons.pk_thunders().count(),
            weapons.pk_trails().count(),
        );
        unsafe {
            psp::sys::sceIoWrite(
                psp::sys::sceKernelStdout(),
                line.as_ptr() as *const core::ffi::c_void,
                line.len(),
            );
        }
    }
    if capture_scene == Some(GameScene::Kirby) {
        let line = alloc::format!(
            "kirby status={:?} anim_frame={:.1} cutters={:?}\n",
            player.fighter.status.status,
            player.fighter.status.anim_frame,
            weapons
                .cutters()
                .map(|c| (c.lifetime, c.anim_ticks, c.position.x))
                .collect::<alloc::vec::Vec<_>>(),
        );
        unsafe {
            psp::sys::sceIoWrite(
                psp::sys::sceKernelStdout(),
                line.as_ptr() as *const core::ffi::c_void,
                line.len(),
            );
        }
    }
    if matches!(
        capture_scene,
        Some(GameScene::Captain | GameScene::CaptainKick)
    ) {
        let line = alloc::format!(
            "captain anim_frame={:.1} punch_effect={:?} kick_effect={:?}\n",
            player.fighter.status.anim_frame,
            ssb_game::captain::punch_effect_ticks(&player.fighter),
            ssb_game::captain::kick_effect_ticks(&player.fighter),
        );
        unsafe {
            psp::sys::sceIoWrite(
                psp::sys::sceKernelStdout(),
                line.as_ptr() as *const core::ffi::c_void,
                line.len(),
            );
        }
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

/// One Training simulation frame: the stage controllers, both fighters'
/// interrupt, physics and map passes, and the weapon and item pools. Kept
/// out of [`run`] so `run` stays inside MIPS branch range.
#[inline(never)]
#[allow(clippy::too_many_arguments)]
unsafe fn training_step(
    p: &Pack<'_>,
    stage_index: u32,
    pl: &mut play::FighterScene,
    dummy_state: &mut Option<play::Dummy>,
    weapons: &mut ssb_game::weapon::WeaponPool,
    items: &mut ssb_game::item::ItemPool,
    material_anim: &mut ssb_rom::skeleton::MaterialAnimator,
    stage_objects: &mut ssb_rom::ground_obj::GroundObjects,
    stage_map: &mut Option<alloc::boxed::Box<ssb_psp_runtime::scene::StageMap>>,
    stage_ctl: &mut ssb_game::stage::Stage,
    controller: ControllerState,
    started: bool,
) {
    material_anim.tick(p);
    if let Some(stage) = p.stage(stage_index) {
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
        // The VS countdown locks every fighter's control, the CPU's too.
        let locked = !started;
        // Priority 5: every fighter's `ftMainProcUpdateInterrupt`.
        // Grab events land before the partner's own half,
        // matching the original's direct status writes
        // (`ssb_game::grab` module docs).
        // `DeadUpFall` drops from above `gGMCameraGObj`'s eye.
        pl.fighter.dead.camera_eye = pl.camera.eye;
        if let Some(dummy) = dummy_state.as_mut() {
            dummy.fighter.dead.camera_eye = pl.camera.eye;
        }
        items.publish(&mut pl.fighter);
        pl.tick_fighter_interrupt(p, &stage, controller, jump_held, groups);
        after_interrupt(&mut pl.fighter, dummy_state.as_ref().map(|d| &d.fighter));
        if let Some(dummy) = dummy_state.as_mut() {
            ssb_game::grab::exchange(&mut pl.fighter, &mut dummy.fighter);
            items.publish(&mut dummy.fighter);
            let opponents = [ssb_game::computer::behave::opponent(&pl.fighter)];
            dummy.tick_interrupt(p, &stage, groups, &opponents, locked);
            ssb_game::grab::exchange(&mut dummy.fighter, &mut pl.fighter);
            after_interrupt(&mut dummy.fighter, Some(&pl.fighter));
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
                        objects: stage_objects,
                    },
                    // The groups are being written, so the
                    // controller sees the static map; only the
                    // Twister queries it, on a static floor.
                    map: ssb_game::stage::MapQuery {
                        surfaces: || ssb_psp_runtime::scene::MapSegments::new(p, &stage),
                        line_group: &line_group,
                    },
                    started,
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
        if core::mem::take(&mut pl.fighter.dead.died) {
            weapons.destroy_boomerang(pl.fighter.port);
        }
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
            dummy.fighter.occupied_cliff = ssb_game::map::is_cliff_hold(pl.fighter.status.status)
                .then_some((pl.fighter.cliff.line, pl.fighter.facing));
            ssb_game::grab::exchange(&mut pl.fighter, &mut dummy.fighter);
            dummy.tick_fighter_physics(p, &stage, groups);
            if core::mem::take(&mut dummy.fighter.dead.died) {
                weapons.destroy_boomerang(dummy.fighter.port);
            }
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
                stage_ctl,
                &mut ssb_psp_runtime::scene::StageObjectsPort {
                    pack: p,
                    objects: stage_objects,
                },
                &dummy_status,
            );
            ssb_game::grab::search_catch(&mut pl.fighter, &dummy.fighter);
            let pl_status = [pl.fighter.status.status];
            ssb_game::hazard::search_hit_hazard(
                &mut dummy.fighter,
                stage_ctl,
                &mut ssb_psp_runtime::scene::StageObjectsPort {
                    pack: p,
                    objects: stage_objects,
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
            items.search_hurt(&mut [&mut pl.fighter, &mut dummy.fighter], weapons);
            // `ftMainSearchGroundHit`, last of `ftMainProcSearchHitAll`.
            ssb_game::hazard::search_ground_hit(&mut pl.fighter, &stage_ctl);
            ssb_game::hazard::search_ground_hit(&mut dummy.fighter, &stage_ctl);
            ssb_game::combat::finish_frame(&mut [&mut pl.fighter, &mut dummy.fighter]);
            let map = || ssb_psp_runtime::scene::MapSegments::with_groups(p, &stage, groups);
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
            items.take_weapon_spawns(weapons, || {
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

/// The fighter Training spawns for the player: Fox for the Fox capture
/// scene, Luigi for the Luigi scene, Samus, Link, Yoshi or Captain Falcon
/// for their own scenes, Mario otherwise.
fn training_fighter_kind(capture_scene: Option<GameScene>) -> ssb_game::fighter::FighterKind {
    // The costume scenes use Fox so that no pick can collide with the Mario
    // dummy's costume.
    match capture_scene {
        Some(GameScene::Fox | GameScene::Costume1 | GameScene::Costume2 | GameScene::Costume3) => {
            ssb_game::fighter::FighterKind::Fox
        }
        Some(GameScene::Luigi) => ssb_game::fighter::FighterKind::Luigi,
        Some(GameScene::Samus | GameScene::SamusShot | GameScene::SamusBomb) => {
            ssb_game::fighter::FighterKind::Samus
        }
        Some(GameScene::Link | GameScene::LinkSpin | GameScene::LinkBomb) => {
            ssb_game::fighter::FighterKind::Link
        }
        Some(GameScene::Yoshi | GameScene::YoshiBomb) => ssb_game::fighter::FighterKind::Yoshi,
        Some(GameScene::Captain | GameScene::CaptainKick) => {
            ssb_game::fighter::FighterKind::Captain
        }
        Some(GameScene::Kirby) => ssb_game::fighter::FighterKind::Kirby,
        Some(GameScene::Pikachu | GameScene::PikachuAir) => ssb_game::fighter::FighterKind::Pikachu,
        Some(GameScene::Purin) => ssb_game::fighter::FighterKind::Purin,
        Some(GameScene::Donkey) => ssb_game::fighter::FighterKind::Donkey,
        Some(GameScene::Ness | GameScene::NessThunder | GameScene::NessMagnet) => {
            ssb_game::fighter::FighterKind::Ness
        }
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
    /// The Training character select (`mnPlayers1PTraining`,
    /// `ssb_game::fighter_select`).
    FighterSelect,
    /// The Training stage select (`mnMaps`, `ssb_game::stage_select`).
    StageSelect,
    /// A VS battle's results: the winner's slot lit (RE-389).
    Results,
    /// The VS mode menu (`mnVSMode`, `ssb_game::vs_mode`, RE-399).
    VsMode,
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
/// A VS battle against the CPU pick, which stands still: CPU AI is not
/// ported (RE-389).
const VS_ENTRY: usize = 1;

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
/// The character select's player puck (the 1P cursor's red), CPU puck and
/// cursor.
const PUCK_PLAYER: Color = Color::rgba(224, 21, 21, 255);
const PUCK_CPU: Color = Color::rgba(150, 150, 150, 255);
const CURSOR_COLOR: Color = Color::rgba(255, 255, 255, 255);

/// The stage every capture scene but `stageselect` loads, skipping the
/// stage select: Dream Land, where the Training goldens were captured.
const CAPTURE_STAGE_GKIND: u8 = ssb_game::stage_select::gkind::PUPUPU;

/// Which selects a capture scene passes through from the Training entry.
#[derive(Clone, Copy, PartialEq, Eq)]
enum CaptureRoute {
    /// Straight into Training on Dream Land with [`capture_training_scene`].
    Direct,
    /// The stage select only, with [`capture_training_scene`].
    StageSelect,
    /// Both selects, from their first-visit state.
    Selects,
}

fn capture_route(scene: GameScene) -> CaptureRoute {
    match scene {
        GameScene::StageSelect => CaptureRoute::StageSelect,
        GameScene::FighterSelect | GameScene::VsModeMenu => CaptureRoute::Selects,
        _ => CaptureRoute::Direct,
    }
}

/// The character select's scene data for a scene that skips it: the
/// scene's fighter against a Mario dummy. The dummy takes the first royal
/// costume the player's default costume leaves free, and the costume
/// scenes then pick with C-Right, C-Down or C-Left
/// (`mnPlayers1PTrainingUpdateCostume`).
#[inline(never)]
fn capture_training_scene(scene: GameScene) -> ssb_game::fighter_select::SceneData {
    use ssb_game::fighter::FighterKind;
    let kind = training_fighter_kind(Some(scene));
    let dummy = ssb_game::costume::Slot {
        kind: FighterKind::Mario,
        costume: ssb_game::costume::free_costume(
            FighterKind::Mario,
            ssb_game::costume::Slot { kind, costume: 0 },
        ),
    };
    let button = match scene {
        GameScene::Costume1 => Some(1),
        GameScene::Costume2 => Some(2),
        GameScene::Costume3 => Some(3),
        _ => None,
    };
    ssb_game::fighter_select::SceneData {
        man_kind: Some(kind),
        man_costume: button
            .and_then(|b| ssb_game::costume::pick(kind, b, dummy))
            .unwrap_or(0),
        com_kind: Some(dummy.kind),
        com_costume: dummy.costume,
    }
}

/// The CPU behaviour a capture scene gives the Training dummy.
fn capture_cpu_behavior(scene: GameScene) -> Option<ssb_game::computer::Behavior> {
    match scene {
        GameScene::CpuWalk => Some(ssb_game::computer::Behavior::Walk),
        GameScene::CpuJump => Some(ssb_game::computer::Behavior::Jump),
        // A time-up tie needs a CPU that never lands a hit.
        GameScene::VsTimeUp | GameScene::VsTimeUpSign | GameScene::VsSuddenDeath => {
            Some(ssb_game::computer::Behavior::Stand)
        }
        _ => None,
    }
}

/// `gSCManagerBackupData.fighter_mask` with no save data
/// (`dSCManagerDefaultBackupData`): Luigi, Captain Falcon, Ness and
/// Jigglypuff are locked.
const FIGHTER_MASK: u16 = 0;

/// `osGetTime() & 0xFF`: the clock's low byte.
fn clock_byte() -> u8 {
    unsafe { psp::sys::sceKernelGetSystemTimeLow() as u8 }
}

/// `syUtilsRandTimeUCharRange(9)`: the clock's low byte, scaled to 0..9.
fn stage_select_rand() -> u8 {
    (u32::from(clock_byte()) * 9 / 256) as u8
}

/// The half of `ftCommonDeadCheckRebirth` a fighter cannot do itself: the
/// rebirth takes the lowest halo the other fighter is not using.
fn after_interrupt(f: &mut ssb_game::fighter::Fighter, other: Option<&ssb_game::fighter::Fighter>) {
    if f.dead.rebirth_pending {
        let halo = ssb_game::dead::halo_number(
            other.map(|o| (o.status.status, o.dead.rebirth.halo_number)).into_iter(),
        );
        ssb_game::dead::rebirth_down(f, halo);
    }
}

/// A VS battle's rules (`gSCManagerTransferBattleState`), as the VS mode
/// menu would set them.
#[derive(Clone, Copy)]
struct VsRules {
    rule: ssb_game::battle::Rule,
    time_limit: u8,
    stocks: i8,
}

impl VsRules {
    /// `dSCManagerDefaultBattleState`: a three-minute time battle, stocks 2.
    const DEFAULT: VsRules = VsRules {
        rule: ssb_game::battle::Rule::Time,
        time_limit: 3,
        stocks: 2,
    };
}

/// A capture scene's VS rules: `vstimeup` picks one minute.
fn vs_rules(scene: GameScene) -> VsRules {
    match scene {
        GameScene::VsTimeUp | GameScene::VsTimeUpSign | GameScene::VsSuddenDeath => VsRules {
            time_limit: 1,
            ..VsRules::DEFAULT
        },
        _ => VsRules::DEFAULT,
    }
}

/// One Training or VS frame: `ifCommonBattleUpdateInterfaceAll`'s say on
/// whether the world runs and whether control is locked, then the world,
/// then the falls it scored. Returns whether the battle is over. Out of
/// [`run`] so `run` stays inside MIPS branch range.
#[inline(never)]
#[allow(clippy::too_many_arguments)]
unsafe fn training_frame(
    p: &Pack<'_>,
    stage_index: u32,
    pl: &mut play::FighterScene,
    dummy_state: &mut Option<play::Dummy>,
    weapons: &mut ssb_game::weapon::WeaponPool,
    items: &mut ssb_game::item::ItemPool,
    material_anim: &mut ssb_rom::skeleton::MaterialAnimator,
    stage_objects: &mut ssb_rom::ground_obj::GroundObjects,
    stage_map: &mut Option<alloc::boxed::Box<ssb_psp_runtime::scene::StageMap>>,
    stage_ctl: &mut ssb_game::stage::Stage,
    controller: ControllerState,
    pressed: N64Buttons,
    mut battle: Option<&mut ssb_game::battle::Battle>,
    damage_hud: &mut Hud,
) -> bool {
    use ssb_game::battle::{Frame, GameStatus};
    if let Some(b) = battle.as_deref_mut() {
        pause_frame(p, stage_index, pl, damage_hud, b, controller, pressed);
    }
    let frame = battle.as_mut().map(|b| (b.begin_frame(), b.status));
    // `ifCommonBattlePauseRestoreInterfaceAll`: the camera eases back while
    // the pause menu stays, then the turn is restored and the world runs.
    if let (Some(pause), Some((f, status))) = (damage_hud.pause, frame) {
        if status == GameStatus::Unpause && f == Frame::Frozen {
            ssb_game::pause::ease_back(&mut pl.camera.pause_eye, pause.origin);
            if let Some(stage) = p.stage(stage_index) {
                pl.tick_camera(&stage, None);
            }
        } else if status == GameStatus::Go {
            pl.camera.pause_eye = pause.origin;
            damage_hud.pause = None;
        }
    }
    let (started, locked) = match frame {
        None => (true, false),
        Some((Frame::Run, status)) => (status != GameStatus::Wait, status == GameStatus::Wait),
        Some((Frame::Frozen, _)) => return false,
        Some((Frame::Done, _)) => return true,
    };
    training_step(
        p,
        stage_index,
        pl,
        dummy_state,
        weapons,
        items,
        material_anim,
        stage_objects,
        stage_map,
        stage_ctl,
        if locked { ControllerState::default() } else { controller },
        started,
    );
    let fell = report_falls(battle.as_deref_mut(), &mut pl.fighter);
    update_damage_hud(&mut damage_hud.damage[0], &pl.fighter, fell, started);
    if let Some(dummy) = dummy_state.as_mut() {
        let fell = report_falls(battle.as_deref_mut(), &mut dummy.fighter);
        update_damage_hud(&mut damage_hud.damage[1], &dummy.fighter, fell, started);
    }
    if let Some(b) = battle.as_deref() {
        tick_countdown(p, damage_hud, b);
        entry_frame(p, pl, dummy_state, damage_hud, b);
    }
    false
}

/// `ifCommonEntryFocusThread`'s slice of the frame: each fighter's entry on
/// its tick, and for the next frame the camera's zoom on the fighter the
/// focus holds (RE-402).
#[inline(never)]
fn entry_frame(
    p: &Pack<'_>,
    pl: &mut play::FighterScene,
    dummy_state: &mut Option<play::Dummy>,
    hud: &Hud,
    b: &ssb_game::battle::Battle,
) {
    pl.entry_zoom = None;
    let Some(focus) = hud.entry_focus else { return };
    let Some(t) = b.clock().checked_sub(1 + ssb_game::battle::ENTRY_WAIT) else {
        return;
    };
    if t == focus.appear_tick(0) {
        ssb_game::appear::appear_set_status(&mut pl.fighter);
    }
    if let Some(d) = dummy_state.as_mut() {
        if t == focus.appear_tick(1) {
            ssb_game::appear::appear_set_status(&mut d.fighter);
        }
    }
    let target = |scene: &play::FighterScene| {
        let mut pos = scene.fighter.pos;
        pos.y += scene.cam_offset_y;
        let dist = p.fighter(scene.fighter.kind as u32).map_or(1000.0, |d| d.closeup_camera_zoom);
        (pos, dist)
    };
    pl.entry_zoom = match focus.zoom(t) {
        Some(0) => Some(target(pl)),
        Some(_) => dummy_state.as_ref().map(|d| target(d)),
        None => None,
    };
}

/// The battle HUD's state beside the world: the damage displays and the
/// countdown or sudden death's "GO!".
struct Hud {
    damage: [ssb_game::hud::DamageDisplay; 2],
    countdown: Option<ssb_game::countdown::Countdown>,
    pause: Option<PauseState>,
    /// `ifCommonEntryFocusThread`, from the countdown's frame.
    entry_focus: Option<ssb_game::appear::EntryFocus>,
}

/// The pause menu's choices at the pause (`sIFCommonBattlePause*`).
#[derive(Clone, Copy)]
struct PauseState {
    kind: ssb_game::pause::PauseKind,
    /// `sIFCommonBattlePauseCameraEyeXOrigin`/`YOrigin`.
    origin: (f32, f32),
}

/// `ifCommonBattleGoUpdateInterface`'s START and
/// `ifCommonBattlePauseUpdateInterface`: pause on START during Go, zooming
/// on the player when in bounds; in the menu steer the view, resume on
/// START, reset on A+B+R+Z, and run the zoom camera.
#[inline(never)]
fn pause_frame(
    p: &Pack<'_>,
    stage_index: u32,
    pl: &mut play::FighterScene,
    hud: &mut Hud,
    b: &mut ssb_game::battle::Battle,
    controller: ControllerState,
    pressed: N64Buttons,
) {
    use ssb_game::battle::GameStatus;
    use ssb_game::pause::{self, PauseKind};
    let Some(stage) = p.stage(stage_index) else {
        return;
    };
    let bounds = ssb_game::camera::Bounds {
        top: f32::from(stage.camera.top),
        bottom: f32::from(stage.camera.bottom),
        left: f32::from(stage.camera.left),
        right: f32::from(stage.camera.right),
    };
    match b.status {
        GameStatus::Go if pressed.contains(N64Buttons::START) => {
            hud.pause = Some(PauseState {
                kind: pause::kind_for(pl.fighter.pos, bounds),
                origin: pl.camera.pause_eye,
            });
            b.pause();
        }
        GameStatus::Pause => {
            let Some(state) = hud.pause else { return };
            if state.kind == PauseKind::Default {
                pause::steer(&mut pl.camera.pause_eye, controller.stick_x, controller.stick_y);
            }
            if pressed.contains(N64Buttons::START) {
                // `gmCameraSetStatusPrev`: back to the battle camera.
                b.unpause(state.kind != PauseKind::PlayerNA);
                return;
            }
            let held = controller.buttons;
            let combo = N64Buttons::A | N64Buttons::B | N64Buttons::R | N64Buttons::Z;
            if pressed.0 != 0 && held.contains(combo) {
                b.reset();
                return;
            }
            if state.kind != PauseKind::PlayerNA {
                // `gmCameraPlayerZoomFuncCamera`: the battle camera while
                // the player is out of bounds.
                if pause::kind_for(pl.fighter.pos, bounds) == PauseKind::PlayerNA {
                    pl.tick_camera(&stage, None);
                } else {
                    let mut pos = pl.fighter.pos;
                    pos.y += pl.cam_offset_y;
                    let dist = p.fighter(pl.fighter.kind as u32).map_or(1000.0, |d| d.closeup_camera_zoom);
                    pl.camera
                        .tick_player_zoom(pos, (0.0, 0.0), dist, pause::ZOOM_PAN_SCALE, pause::ZOOM_FOV);
                }
            }
        }
        _ => {}
    }
}

impl Hud {
    fn new() -> Hud {
        Hud {
            damage: [
                ssb_game::hud::DamageDisplay::new(0, 0),
                ssb_game::hud::DamageDisplay::new(1, 0),
            ],
            countdown: None,
            pause: None,
            entry_focus: None,
        }
    }
}

/// File 82's sprite sizes, for the countdown's pop-ins.
fn game_status_sizes(p: &Pack<'_>) -> [(u16, u16); 24] {
    let f = &ssb_rom::sprite::GAME_STATUS;
    core::array::from_fn(|i| {
        f.offsets
            .get(i)
            .and_then(|&at| p.sprite(f.file, at))
            .map_or((0, 0), |s| (s.width, s.height))
    })
}

/// `ifCommonEntryAllThread` makes the countdown after its 90-tick sleep,
/// drawing the entry focus's `syUtilsRandIntRange(3)` (the focus itself is
/// not drawn); `ifCommonSuddenDeathThread` shows "GO!" at the same tick.
fn tick_countdown(p: &Pack<'_>, hud: &mut Hud, b: &ssb_game::battle::Battle) {
    let entry = 1 + ssb_game::battle::ENTRY_WAIT;
    if b.clock() < entry {
        return;
    }
    if b.clock() == entry {
        match hud.countdown.as_mut() {
            Some(c) if b.is_sudden_death => c.start_go(),
            None => {
                hud.entry_focus = Some(ssb_game::appear::EntryFocus {
                    id: ssb_game::rng::rand_int_range(3) as u8,
                    count: 2,
                });
                hud.countdown = Some(ssb_game::countdown::Countdown::new());
            }
            Some(_) => {}
        }
    }
    if let Some(c) = hud.countdown.as_mut() {
        c.tick(&game_status_sizes(p));
    }
}

/// `scVSBattleStartSuddenDeath`: the tied fighters again, at 300%, under
/// the sudden-death battle.
#[inline(never)]
fn start_sudden_death(
    pack: Option<&Pack<'_>>,
    gkind: u8,
    fighters: ssb_game::fighter_select::SceneData,
    sudden: ssb_game::battle::Battle,
    battle: &mut Option<ssb_game::battle::Battle>,
    world: &mut TrainingWorld<'_>,
) -> u32 {
    let rules = VsRules {
        rule: ssb_game::battle::Rule::Stock,
        time_limit: sudden.time_limit,
        stocks: 0,
    };
    // A capture scene's CPU behaviour carries over; in play it is the VS
    // default either way.
    let cpu = world
        .dummy_state
        .as_ref()
        .map(|d| (d.computer.behavior, d.computer.trait_kind));
    let index = enter_training(pack, gkind, fighters, Some(rules), battle, world);
    // `is_skip_entry`: sudden death's fighters stand at once.
    if let Some(pl) = world.play_state.as_mut() {
        pl.fighter.damage = ssb_game::battle::SUDDEN_DEATH_DAMAGE;
        ssb_game::status::set_wait(&mut pl.fighter);
    }
    if let Some(d) = world.dummy_state.as_mut() {
        d.fighter.damage = ssb_game::battle::SUDDEN_DEATH_DAMAGE;
        ssb_game::status::set_wait(&mut d.fighter);
        if let Some((behavior, trait_kind)) = cpu {
            d.computer.behavior = behavior;
            d.computer.trait_kind = trait_kind;
        }
    }
    // `scVSBattleStartSuddenDeath` makes the damage display after the
    // fighters are at 300%, and `ifCommonSuddenDeathMakeInterface`.
    reset_damage_hud(world);
    world.damage_hud.countdown = Some(ssb_game::countdown::Countdown::sudden_death());
    *battle = Some(sudden);
    index
}

/// The battle half of `ftCommonDeadUpdateScore`. Returns whether the
/// fighter fell.
fn report_falls(battle: Option<&mut ssb_game::battle::Battle>, f: &mut ssb_game::fighter::Fighter) -> bool {
    let fell = core::mem::take(&mut f.dead.scored);
    if fell {
        if let Some(b) = battle {
            b.on_fall(f.port, f.damage_player);
        }
    }
    fell
}

/// `ifCommonPlayerDamageInitInterface` for both fighters, shown at once
/// outside a battle.
fn reset_damage_hud(world: &mut TrainingWorld<'_>) {
    let damage = |f: Option<&ssb_game::fighter::Fighter>| f.map_or(0, |f| i32::from(f.damage));
    world.damage_hud.countdown = None;
    world.damage_hud.entry_focus = None;
    world.damage_hud.damage[0] =
        ssb_game::hud::DamageDisplay::new(0, damage(world.play_state.as_ref().map(|s| &s.fighter)));
    world.damage_hud.damage[1] =
        ssb_game::hud::DamageDisplay::new(1, damage(world.dummy_state.as_ref().map(|d| &d.fighter)));
}

/// One frame of `ifCommonPlayerDamageProcUpdate` for a fighter: the break
/// on a fall (`ftCommonDeadUpdateScore`), its end at the rebirth
/// (`ftCommonRebirthDownSetStatus`), then the update.
fn update_damage_hud(hud: &mut ssb_game::hud::DamageDisplay, f: &ssb_game::fighter::Fighter, fell: bool, shown: bool) {
    use ssb_game::status::{AnyStatus, Status};
    if fell {
        hud.start_break_anim();
    }
    if hud.is_update_anim && f.status.status == AnyStatus::Common(Status::RebirthDown) {
        hud.stop_break_anim();
    }
    hud.is_show_interface |= shown;
    hud.update(i32::from(f.damage), f.dead.stock_rule && f.stocks == -1);
}

/// The results in place of `mnVSResults` (RE-400): one slot per player,
/// taller for a better place, the winner's (and a shared winner's) lit.
#[inline(never)]
fn draw_results(gpu: &mut Gpu, results: Option<&ssb_game::results::Results>) {
    let Some(r) = results else {
        return;
    };
    for i in (0..4).filter(|&i| r.present[i]) {
        let x0 = 60 + i as i32 * 100;
        let lit = r.winner == Some(i) || r.shared_winner[i];
        let color = if lit { ENTRY_SELECTED } else { ENTRY_ENABLED };
        // First place stands tallest; no contest levels everyone.
        let top = 100 + r.places[i] * 20;
        gpu.draw_rect(x0, top, x0 + 80, 180, color);
    }
}

/// `mnVSResultsInitVars` and its rankings. Out of [`run`] for branch range.
#[inline(never)]
fn make_results(b: &ssb_game::battle::Battle) -> ssb_game::results::Results {
    ssb_game::results::Results::new(b)
}

/// One frame of the results' exit check. Out of [`run`] for branch range.
#[inline(never)]
fn results_frame(results: &mut Option<ssb_game::results::Results>, pressed: N64Buttons) -> bool {
    results
        .as_mut()
        .is_some_and(|r| r.tick(pressed.contains(N64Buttons::START)))
}

/// What Training owns across frames, rebuilt on each stage entry.
struct TrainingWorld<'w> {
    play_state: &'w mut Option<play::FighterScene>,
    dummy_state: &'w mut Option<play::Dummy>,
    weapons: &'w mut ssb_game::weapon::WeaponPool,
    items: &'w mut ssb_game::item::ItemPool,
    stage_objects: &'w mut ssb_rom::ground_obj::GroundObjects,
    stage_map: &'w mut Option<alloc::boxed::Box<ssb_psp_runtime::scene::StageMap>>,
    stage_ctl: &'w mut ssb_game::stage::Stage,
    damage_hud: &'w mut Hud,
}

/// Loads VS stage `gkind` for Training and spawns both fighters on it;
/// returns the pack's stage index. Out of [`run`] so `run` stays inside
/// MIPS branch range.
#[inline(never)]
fn enter_training(
    pack: Option<&Pack<'_>>,
    gkind: u8,
    fighters: ssb_game::fighter_select::SceneData,
    vs: Option<VsRules>,
    battle: &mut Option<ssb_game::battle::Battle>,
    world: &mut TrainingWorld<'_>,
) -> u32 {
    use ssb_game::fighter::FighterKind;
    *world.weapons = ssb_game::weapon::WeaponPool::default();
    *world.items = ssb_game::item::ItemPool::default();
    let Some((p, index, stage)) = pack.and_then(|p| {
        let index = ssb_psp_runtime::scene::vs_stage_index(p, gkind)?;
        Some((p, index, p.stage(index)?))
    }) else {
        return 0;
    };
    *world.stage_map = Some(alloc::boxed::Box::new(ssb_psp_runtime::scene::StageMap::new(
        p, index, &stage,
    )));
    *world.stage_objects = ssb_rom::ground_obj::GroundObjects::new(p, stage.source_file);
    // `grMainSetupMakeGround`: any VS stage gets its controller; others run
    // an empty slot.
    *world.stage_ctl = match ssb_psp_runtime::scene::StageSetup::new(p, &stage) {
        Some(setup) => {
            let mut empty = [];
            let groups = world
                .stage_map
                .as_mut()
                .map_or(&mut empty[..], |map| map.groups.as_mut_slice());
            ssb_game::stage::Stage::new(
                &setup.init(),
                groups,
                &mut ssb_psp_runtime::scene::StageObjectsPort {
                    pack: p,
                    objects: world.stage_objects,
                },
            )
        }
        None => ssb_game::stage::Stage::none(),
    };
    let kind = fighters.man_kind.unwrap_or(FighterKind::Mario);
    let mut scene = play::FighterScene::at_spawn(p, &stage, kind, 0);
    scene.fighter.costume = fighters.man_costume;
    *world.play_state = Some(scene);
    *world.dummy_state = play::Dummy::at_spawn(
        p,
        &stage,
        fighters.com_kind.unwrap_or(FighterKind::Mario),
        fighters.com_costume,
    );
    // `ifCommonPlayerDamageInitInterface`; Training shows it at once
    // (`ifCommonPlayerDamageSetShowInterface`), VS at "Go".
    reset_damage_hud(world);
    *battle = vs.map(|rules| {
        // `scVSBattleStartBattle`: each fighter faces the nearest other
        // spawn, and a stock battle's deaths take stocks.
        let spawn_x = |i| p.spawn(&stage, i).map(|s| f32::from(s.x));
        let stock_rule = rules.rule == ssb_game::battle::Rule::Stock;
        if let Some(pl) = world.play_state.as_mut() {
            pl.fighter.facing =
                ssb_game::battle::start_facing(pl.fighter.pos.x, spawn_x(1).into_iter());
            pl.fighter.dead.stock_rule = stock_rule;
            pl.fighter.stocks = rules.stocks;
            // `ftManagerMakeFighter`: a VS fighter waits hidden for its
            // entry (`ftCommonEntrySetStatus`).
            ssb_game::appear::entry_set_status(&mut pl.fighter);
        }
        if let Some(d) = world.dummy_state.as_mut() {
            d.fighter.facing =
                ssb_game::battle::start_facing(d.fighter.pos.x, spawn_x(0).into_iter());
            d.fighter.dead.stock_rule = stock_rule;
            d.fighter.stocks = rules.stocks;
            // A VS CPU runs the default trait and behaviour: it fights.
            d.computer.trait_kind = ssb_game::computer::attack::Trait::Default;
            d.computer.behavior = ssb_game::computer::Behavior::Default;
            ssb_game::appear::entry_set_status(&mut d.fighter);
        }
        let mut players = [ssb_game::battle::Player::default(); 4];
        players[0] = ssb_game::battle::Player {
            present: true,
            is_human: true,
            team: 0,
            ..Default::default()
        };
        players[1] = ssb_game::battle::Player {
            present: world.dummy_state.is_some(),
            team: 1,
            ..Default::default()
        };
        ssb_game::battle::Battle::new(rules.rule, rules.time_limit, rules.stocks, players)
    });
    index
}

/// The frame's draw for the current screen. Out of [`run`] so `run` stays
/// inside MIPS branch range.
#[inline(never)]
unsafe fn draw_frame(
    gpu: &mut Gpu,
    s: &Session,
    pack: &Option<Pack<'_>>,
    draw_state: &mut meshdraw::DrawState,
    effect_visuals: &mut EffectVisuals,
    draw_assets: &DrawAssets,
    no_pack_color: Color,
) {
    match s.screen {
        Screen::Intro => {
            gpu.set_viewport_fullscreen();
            gpu.begin_frame(Some(BG_INTRO));
        }
        Screen::Menu => {
            gpu.set_viewport_fullscreen();
            gpu.begin_frame(Some(BG_MENU));
            draw_menu(gpu, s.cursor);
        }
        Screen::VsMode => {
            gpu.set_viewport_fullscreen();
            gpu.begin_frame(Some(BG_MENU));
            draw_vs_mode(gpu, &s.vs_mode);
        }
        Screen::FighterSelect => {
            gpu.set_viewport_fullscreen();
            gpu.begin_frame(Some(BG_MENU));
            if let Some(select) = s.fighter_select.as_ref() {
                draw_fighter_select(gpu, select);
            }
        }
        Screen::StageSelect => {
            gpu.set_viewport_fullscreen();
            gpu.begin_frame(Some(BG_MENU));
            draw_stage_select(gpu, &s.stage_select);
        }
        Screen::Results => {
            gpu.set_viewport_fullscreen();
            gpu.begin_frame(Some(BG_MENU));
            draw_results(gpu, s.vs_results.as_ref());
        }
        Screen::Training => {
            if let (Some(p), Some(pl)) = (pack.as_ref(), s.play_state.as_ref()) {
                effect_visuals.sync(p, draw_assets, &pl.fighter, &s.weapons, &s.items);
            }
            draw_training(
                gpu,
                draw_state,
                pack.as_ref(),
                s.training_stage,
                s.play_state.as_ref(),
                s.dummy_state.as_ref(),
                &s.weapons,
                &s.items,
                draw_assets,
                effect_visuals,
                Some(&s.material_anim),
                s.stage_map.as_ref().map(|map| &map.animator),
                Some(&s.stage_objects),
                no_pack_color,
                &s.damage_hud,
                s.vs_battle.as_ref(),
            );
        }
    }
}

/// One frame of the screens' logic and the world, when not frozen for a
/// capture: the menus, the selects, the battle frame and what follows a
/// battle. Out of [`run`] so `run` stays inside MIPS branch range.
#[inline(never)]
#[allow(clippy::too_many_arguments)]
unsafe fn session_frame(
    s: &mut Session,
    pack: &Option<Pack<'_>>,
    capture_scene: Option<GameScene>,
    sim_frame_index: u64,
    previous_controller: ControllerState,
    controller: ControllerState,
    pressed: N64Buttons,
) {
        match s.screen {
            Screen::Intro => {
                if pressed.contains(N64Buttons::A) || pressed.contains(N64Buttons::START) {
                    s.screen = Screen::Menu;
                }
            }
            Screen::Menu => {
                if menu_stick_down_pressed(previous_controller, controller) {
                    s.cursor = (s.cursor + 1) % MENU_ENTRIES;
                } else if menu_stick_up_pressed(previous_controller, controller) {
                    s.cursor = (s.cursor + MENU_ENTRIES - 1) % MENU_ENTRIES;
                } else if pressed.contains(N64Buttons::A)
                    && (s.cursor == TRAINING_ENTRY || s.cursor == VS_ENTRY)
                {
                    let route = capture_scene.map(capture_route);
                    if let Some(scene) = capture_scene.filter(|_| route != Some(CaptureRoute::Selects)) {
                        s.training_scene = capture_training_scene(scene);
                    }
                    if s.cursor == VS_ENTRY || s.vs {
                        // A VS battle always starts over.
                        s.play_state = None;
                        s.dummy_state = None;
                    }
                    s.vs = s.cursor == VS_ENTRY;
                    let rules = s.vs.then(|| capture_scene.map_or(VsRules::DEFAULT, vs_rules));
                    if s.play_state.is_some() {
                        s.screen = Screen::Training;
                    } else if route == Some(CaptureRoute::Direct) {
                        s.training_stage = enter_training(
                            pack.as_ref(),
                            CAPTURE_STAGE_GKIND,
                            s.training_scene,
                            rules,
                            &mut s.vs_battle,
                            &mut TrainingWorld {
                                play_state: &mut s.play_state,
                                dummy_state: &mut s.dummy_state,
                                weapons: &mut s.weapons,
                                items: &mut s.items,
                                stage_objects: &mut s.stage_objects,
                                stage_map: &mut s.stage_map,
                                stage_ctl: &mut s.stage_ctl,
                                damage_hud: &mut s.damage_hud,
                            },
                        );
                        s.scene_gkind = CAPTURE_STAGE_GKIND;
                        // Training's CPU menu (`dSC1PTrainingModeDummyBehaviors`)
                        // is not ported; these scenes pick its behaviour.
                        if let (Some(d), Some(b)) =
                            (s.dummy_state.as_mut(), capture_scene.and_then(capture_cpu_behavior))
                        {
                            d.computer.behavior = b;
                            d.computer.trait_kind = ssb_game::computer::attack::Trait::None;
                        }
                        s.screen = Screen::Training;
                    } else if route == Some(CaptureRoute::StageSelect) {
                        s.stage_select = ssb_game::stage_select::StageSelect::new(s.maps_training_gkind, 0);
                        s.screen = Screen::StageSelect;
                    } else if s.vs {
                        // `mnVSModeFuncStartVars` from the last settings.
                        s.vs_mode = vs_mode_menu(s.vs_menu_rules);
                        s.screen = Screen::VsMode;
                    } else {
                        s.fighter_select =
                            Some(new_fighter_select(s.training_scene, capture_scene.is_some(), sim_frame_index));
                        s.screen = Screen::FighterSelect;
                    }
                }
            }
            Screen::VsMode => {
                use ssb_game::vs_mode::Action;
                match vs_mode_frame(&mut s.vs_mode, controller, pressed) {
                    Action::Start => {
                        // `mnVSModeSaveSettings`.
                        s.vs_menu_rules = VsRules {
                            rule: s.vs_mode.rule.battle_rule(),
                            time_limit: s.vs_mode.time,
                            stocks: s.vs_mode.stocks(),
                        };
                        s.fighter_select =
                            Some(new_fighter_select(s.training_scene, capture_scene.is_some(), sim_frame_index));
                        s.screen = Screen::FighterSelect;
                    }
                    Action::Back => s.screen = Screen::Menu,
                    Action::Title => s.screen = Screen::Intro,
                    // VS Options is not ported.
                    Action::Options | Action::None => {}
                }
            }
            Screen::FighterSelect => {
                use ssb_game::fighter_select::Outcome;
                match s.fighter_select.as_mut().and_then(|s| s.tick(controller, pressed)) {
                    Some(Outcome::Proceed(data)) => {
                        s.training_scene = data;
                        // `mnMapsInitVars`: the s.cursor starts on the
                        // stage this mode picked last. The host has no
                        // save data, so Mushroom Kingdom stays locked.
                        let remembered = if s.vs { s.maps_vsmode_gkind } else { s.maps_training_gkind };
                        s.stage_select = ssb_game::stage_select::StageSelect::new(remembered, 0);
                        s.screen = Screen::StageSelect;
                    }
                    // The menu stands in for the 1P mode menu.
                    Some(Outcome::Back(data)) => {
                        s.training_scene = data;
                        s.screen = Screen::Menu;
                    }
                    Some(Outcome::Timeout(data)) => {
                        s.training_scene = data;
                        s.screen = Screen::Intro;
                    }
                    None => {}
                }
            }
            Screen::StageSelect => match s.stage_select.tick(controller, pressed) {
                Some(ssb_game::stage_select::Outcome::Confirm { .. }) => {
                    let saved = s.stage_select.save(s.scene_gkind, stage_select_rand);
                    if s.vs {
                        s.maps_vsmode_gkind = saved.remembered;
                    } else {
                        s.maps_training_gkind = saved.remembered;
                    }
                    s.scene_gkind = saved.gkind;
                    s.training_stage = enter_training(
                        pack.as_ref(),
                        saved.gkind,
                        s.training_scene,
                        s.vs.then_some(s.vs_menu_rules),
                        &mut s.vs_battle,
                        &mut TrainingWorld {
                            play_state: &mut s.play_state,
                            dummy_state: &mut s.dummy_state,
                            weapons: &mut s.weapons,
                            items: &mut s.items,
                            stage_objects: &mut s.stage_objects,
                            stage_map: &mut s.stage_map,
                            stage_ctl: &mut s.stage_ctl,
                            damage_hud: &mut s.damage_hud,
                        },
                    );
                    s.screen = Screen::Training;
                }
                // B returns to the character select, with the fighters
                // it saved. B and the idle return also save the scene
                // data.
                Some(ssb_game::stage_select::Outcome::Back) => {
                    let saved = s.stage_select.save(s.scene_gkind, stage_select_rand);
                    s.maps_training_gkind = saved.remembered;
                    s.scene_gkind = saved.gkind;
                    s.fighter_select = Some(ssb_game::fighter_select::FighterSelect::new(
                        s.training_scene,
                        FIGHTER_MASK,
                        clock_byte,
                    ));
                    s.screen = Screen::FighterSelect;
                }
                Some(ssb_game::stage_select::Outcome::Timeout) => {
                    let saved = s.stage_select.save(s.scene_gkind, stage_select_rand);
                    s.maps_training_gkind = saved.remembered;
                    s.scene_gkind = saved.gkind;
                    s.screen = Screen::Intro;
                }
                None => {}
            },
            Screen::Training => {
                // START is navigation-only here. B belongs to the fighter's
                // source special-input path and must reach `pl.tick` below.
                // In a VS battle START is the pause menu's (`training_frame`).
                if pressed.contains(N64Buttons::START) && s.vs_battle.is_none() {
                    s.screen = Screen::Menu;
                }
            }
            // `mnVSResultsCheckExit`: START after the wait, on to the
            // character select (`mnPlayersVS`'s stand-in).
            Screen::Results => {
                if results_frame(&mut s.vs_results, pressed) {
                    s.play_state = None;
                    s.dummy_state = None;
                    s.vs_battle = None;
                    s.fighter_select =
                        Some(new_fighter_select(s.training_scene, capture_scene.is_some(), sim_frame_index));
                    s.screen = Screen::FighterSelect;
                }
            }
        }

        let mut vs_done = false;
        if let (Screen::Training, Some(p), Some(pl)) = (s.screen, &pack, s.play_state.as_mut()) {
            vs_done = training_frame(
                p,
                s.training_stage,
                pl,
                &mut s.dummy_state,
                &mut s.weapons,
                &mut s.items,
                &mut s.material_anim,
                &mut s.stage_objects,
                &mut s.stage_map,
                &mut s.stage_ctl,
                controller,
                pressed,
                s.vs_battle.as_mut(),
                &mut s.damage_hud,
            );
        }
        // `scVSBattleStartScene`: a tied time battle goes to sudden
        // death on the same stage, then to the results.
        if vs_done {
            let sudden = s.vs_battle
                .as_ref()
                .filter(|b| !b.is_sudden_death && !b.is_reset)
                .and_then(ssb_game::battle::Battle::sudden_death_battle);
            match sudden {
                Some(battle) => {
                    s.training_stage = start_sudden_death(
                        pack.as_ref(),
                        s.scene_gkind,
                        s.training_scene,
                        battle,
                        &mut s.vs_battle,
                        &mut TrainingWorld {
                            play_state: &mut s.play_state,
                            dummy_state: &mut s.dummy_state,
                            weapons: &mut s.weapons,
                            items: &mut s.items,
                            stage_objects: &mut s.stage_objects,
                            stage_map: &mut s.stage_map,
                            stage_ctl: &mut s.stage_ctl,
                            damage_hud: &mut s.damage_hud,
                        },
                    );
                }
                // A reset from the pause menu is a no contest.
                None => {
                    s.vs_results = s.vs_battle.as_ref().map(make_results);
                    s.screen = Screen::Results;
                }
            }
        }
}

/// What `run` owns across frames: the screens' state and the world.
struct Session {
    material_anim: ssb_rom::skeleton::MaterialAnimator,
    stage_map: Option<alloc::boxed::Box<ssb_psp_runtime::scene::StageMap>>,
    training_stage: u32,
    scene_gkind: u8,
    maps_training_gkind: u8,
    stage_select: ssb_game::stage_select::StageSelect,
    damage_hud: Hud,
    play_state: Option<play::FighterScene>,
    dummy_state: Option<play::Dummy>,
    weapons: ssb_game::weapon::WeaponPool,
    items: ssb_game::item::ItemPool,
    stage_objects: ssb_rom::ground_obj::GroundObjects,
    stage_ctl: ssb_game::stage::Stage,
    screen: Screen,
    cursor: usize,
    training_scene: ssb_game::fighter_select::SceneData,
    vs: bool,
    vs_battle: Option<ssb_game::battle::Battle>,
    maps_vsmode_gkind: u8,
    vs_menu_rules: VsRules,
    vs_results: Option<ssb_game::results::Results>,
    vs_mode: ssb_game::vs_mode::VsMode,
    fighter_select: Option<ssb_game::fighter_select::FighterSelect>,
}

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
    // to draw -- distinguishes *why* (open/read failure s.vs. a rejected
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
    let mut s = Session {
        material_anim: ssb_rom::skeleton::MaterialAnimator::new(),
        stage_map: None,
        training_stage: 0,
        scene_gkind: ssb_game::stage_select::DEFAULT_GKIND,
        maps_training_gkind: ssb_game::stage_select::DEFAULT_GKIND,
        stage_select: ssb_game::stage_select::StageSelect::new(ssb_game::stage_select::DEFAULT_GKIND, 0),
        damage_hud: Hud::new(),
        play_state: None,
        dummy_state: None,
        weapons: ssb_game::weapon::WeaponPool::default(),
        items: ssb_game::item::ItemPool::default(),
        stage_objects: ssb_rom::ground_obj::GroundObjects::empty(),
        stage_ctl: ssb_game::stage::Stage::none(),
        screen: Screen::Intro,
        cursor: 0,
        training_scene: ssb_game::fighter_select::SceneData::default(),
        vs: false,
        vs_battle: None,
        maps_vsmode_gkind: ssb_game::stage_select::DEFAULT_GKIND,
        vs_menu_rules: VsRules::DEFAULT,
        vs_results: None,
        vs_mode: ssb_game::vs_mode::VsMode::new(ssb_game::vs_mode::VsRule::Time, 3, 2, false),
        fighter_select: None,
    };
    // Stage MObj material joints are process-lifetime clocks in the original
    // layer setup. Start once with this pack and advance in the same simulation
    // branch as the stage/fighter tick; draw only reads the resulting state.
    if let Some(p) = pack.as_ref() {
        s.material_anim.start(p);
    }
    // Built on each Training entry for the stage picked (`enter_training`).
    // `gSCManagerSceneData.gkind` and `s.maps_training_gkind`, both
    // `nGRKindCastle` in `dSCManagerDefaultSceneData`.

    let draw_assets = pack.as_ref().map(DrawAssets::resolve).unwrap_or_default();
    let mut effect_visuals = EffectVisuals::default();
    let mut draw_state = meshdraw::DrawState::default();
    // Created once, on first entry to Training Mode (below) -- a fighter
    // spawned on the training stage, ticked with real physics/animation/
    // camera every frame this s.screen is active (`play::FighterScene`, shared
    // with `psp-asset-viewer/` via `ssb_psp_runtime::scene`).
    // The stationary dummy target (`play::Dummy`, `psp-game`-only -- see its
    // doc comment): spawned alongside `s.play_state` at the stage's second
    // spawn point, ticked with permanently neutral input.
    // Match-owned spawned s.weapons. Fighter statuses emit portable requests;
    // Training owns the pool because it is the layer that has both fighters
    // and the stage collision iterator.
    // The stage controller slot (`grMainSetupMakeGround`), backed by the
    // packed objects' priority-5 animation clocks in Training (RE-357).
    // `gSCManagerSceneData`'s Training fighters, both `nFTKindNull` until
    // the character select saves them.
    // The VS path: the same selects, then `Battle` rules (RE-389).
    // `s.maps_vsmode_gkind` defaults to Peach's Castle.
    // `gSCManagerTransferBattleState`'s rule, time and stocks, which the VS
    // mode menu edits and every VS battle reads.
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
            session_frame(
                &mut s,
                &pack,
                capture_scene,
                sim_frame_index,
                previous_controller,
                controller,
                pressed,
            );
        }

        draw_frame(
            &mut gpu,
            &s,
            &pack,
            &mut draw_state,
            &mut effect_visuals,
            &draw_assets,
            no_pack_color,
        );
        gpu.end_frame();

        #[cfg(feature = "headless_capture")]
        if !headless_capture_sent && deterministic_capture_frozen(capture_scene, sim_frame_index) {
            emit_headless_screenshot();
            // One line for the capture log: whether the scripted attack
            // landed is not always visible (RE-351).
            if let (Some(dummy), Some(player)) = (s.dummy_state.as_ref(), s.play_state.as_ref()) {
                log_capture_state(
                    capture_scene,
                    sim_frame_index,
                    player,
                    dummy,
                    &s.weapons,
                    &s.items,
                    s.vs_battle.as_ref(),
                );
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
#[inline(never)]
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
        } else if i == TRAINING_ENTRY || i == VS_ENTRY {
            ENTRY_ENABLED
        } else {
            ENTRY_DISABLED
        };
        gpu.draw_rect(LEFT, y0, LEFT + ENTRY_WIDTH, y0 + ENTRY_HEIGHT, color);
    }
}

/// The character select from the last scene data. Captures read the frame
/// counter for the CPU's random fighter, so they stay deterministic.
#[inline(never)]
fn new_fighter_select(
    scene: ssb_game::fighter_select::SceneData,
    capture: bool,
    frame: u64,
) -> ssb_game::fighter_select::FighterSelect {
    let time_byte = frame as u8;
    ssb_game::fighter_select::FighterSelect::new(scene, FIGHTER_MASK, || {
        if capture { time_byte } else { clock_byte() }
    })
}

/// One frame of `mnVSModeMain`. Out of [`run`] so `run` stays inside MIPS
/// branch range.
#[inline(never)]
fn vs_mode_frame(
    m: &mut ssb_game::vs_mode::VsMode,
    controller: ControllerState,
    pressed: N64Buttons,
) -> ssb_game::vs_mode::Action {
    m.tick(ssb_game::vs_mode::Input {
        hold: controller.buttons.0,
        tap: pressed.0,
        stick_x: controller.stick_x,
        stick_y: controller.stick_y,
    })
}

/// `mnVSModeFuncStartVars`'s rule, time and stock from the battle settings.
fn vs_mode_menu(rules: VsRules) -> ssb_game::vs_mode::VsMode {
    use ssb_game::vs_mode::{VsMode, VsRule};
    let rule = match rules.rule {
        ssb_game::battle::Rule::Time => VsRule::Time,
        ssb_game::battle::Rule::Stock => VsRule::Stock,
    };
    VsMode::new(rule, rules.time_limit, rules.stocks.max(0) as u8, false)
}

/// The VS mode menu as plain slots: the four buttons, the cursor's lit,
/// the rule's four values with the chosen one lit, and the time or stock as
/// a bar (full for an infinite time). The menu's sprites are not drawn.
#[inline(never)]
fn draw_vs_mode(gpu: &mut Gpu, m: &ssb_game::vs_mode::VsMode) {
    use ssb_game::vs_mode::{Button, VsRule};
    const LEFT: i32 = 40;
    const TOP: i32 = 40;
    const HEIGHT: i32 = 32;
    const GAP: i32 = 16;
    const WIDTH: i32 = 160;
    let buttons = [Button::Start, Button::Rule, Button::TimeStock, Button::Options];
    for (i, b) in buttons.into_iter().enumerate() {
        let y0 = TOP + i as i32 * (HEIGHT + GAP);
        let color = if b == m.cursor { ENTRY_SELECTED } else { ENTRY_ENABLED };
        gpu.draw_rect(LEFT, y0, LEFT + WIDTH, y0 + HEIGHT, color);
    }
    let rules = [VsRule::Time, VsRule::Stock, VsRule::TimeTeam, VsRule::StockTeam];
    let y0 = TOP + HEIGHT + GAP;
    for (i, r) in rules.into_iter().enumerate() {
        let x0 = LEFT + WIDTH + 16 + i as i32 * 40;
        let color = if r == m.rule { ENTRY_SELECTED } else { ENTRY_DISABLED };
        gpu.draw_rect(x0, y0, x0 + 32, y0 + HEIGHT, color);
    }
    let y0 = TOP + 2 * (HEIGHT + GAP);
    let fraction = if m.rule.is_time() {
        if m.time == ssb_game::battle::TIMELIMIT_INFINITE {
            1.0
        } else {
            f32::from(m.time) / 99.0
        }
    } else {
        f32::from(m.stock + 1) / 99.0
    };
    let x0 = LEFT + WIDTH + 16;
    gpu.draw_rect(x0, y0, x0 + 152, y0 + HEIGHT, ENTRY_DISABLED);
    gpu.draw_rect(x0, y0, x0 + (152.0 * fraction) as i32, y0 + HEIGHT, ENTRY_SELECTED);
}

/// Draws the character select in N64 screen coordinates scaled onto the
/// PSP screen: the portrait grid (locked portraits dimmed, a placed
/// fighter's portrait lit), the pucks and the cursor. The portraits,
/// models and names are not drawn.
#[inline(never)]
fn draw_fighter_select(gpu: &mut Gpu, select: &ssb_game::fighter_select::FighterSelect) {
    use ssb_game::fighter_select as fs;
    // 320×240 onto 480×272 at 17/15, centred horizontally.
    let map = |x: f32, y: f32| ((59.0 + x * 17.0 / 15.0) as i32, (y * 17.0 / 15.0) as i32);
    let rect = |gpu: &mut Gpu, x: f32, y: f32, w: f32, h: f32, color: Color| {
        let (x0, y0) = map(x, y);
        let (x1, y1) = map(x + w, y + h);
        gpu.draw_rect(x0, y0, x1, y1, color);
    };
    for (i, kind) in fs::PORTRAIT_KINDS.iter().enumerate() {
        let x = fs::PORTRAIT_LEFT + (i % 6) as f32 * fs::PORTRAIT_WIDTH;
        let y = fs::PORTRAIT_TOP + (i / 6) as f32 * fs::PORTRAIT_HEIGHT;
        let placed = select
            .slots
            .iter()
            .any(|s| s.is_fighter_selected && s.kind == Some(*kind));
        let color = if fs::is_locked(*kind, FIGHTER_MASK) {
            ENTRY_DISABLED
        } else if placed {
            ENTRY_SELECTED
        } else {
            ENTRY_ENABLED
        };
        rect(gpu, x + 1.0, y + 1.0, fs::PORTRAIT_WIDTH - 2.0, fs::PORTRAIT_HEIGHT - 2.0, color);
    }
    for (i, slot) in select.slots.iter().enumerate() {
        if select.puck_visible(i) {
            let color = if i == fs::MAN { PUCK_PLAYER } else { PUCK_CPU };
            let (x, y) = slot.puck;
            rect(gpu, x, y, fs::PUCK_WIDTH, fs::PUCK_HEIGHT, color);
        }
    }
    let (x, y) = select.slots[fs::MAN].cursor;
    rect(gpu, x + 20.0, y, 10.0, 10.0, CURSOR_COLOR);
}

/// Draws the stage select as `mnMaps`'s two rows of five slots, the random
/// slot last. The cursor's slot is in `ENTRY_SELECTED` and a locked slot in
/// `ENTRY_DISABLED`. The names, emblems and stage previews are not drawn.
#[inline(never)]
fn draw_stage_select(gpu: &mut Gpu, select: &ssb_game::stage_select::StageSelect) {
    const SLOT_WIDTH: i32 = 64;
    const SLOT_HEIGHT: i32 = 48;
    const GAP: i32 = 16;
    const LEFT: i32 = 40;
    const TOP: i32 = 120;

    for slot in 0..=ssb_game::stage_select::RANDOM_SLOT {
        let x0 = LEFT + (slot % 5) as i32 * (SLOT_WIDTH + GAP);
        let y0 = TOP + (slot / 5) as i32 * (SLOT_HEIGHT + GAP);
        let color = if slot == select.cursor_slot {
            ENTRY_SELECTED
        } else if select.is_locked(ssb_game::stage_select::slot_gkind(slot)) {
            ENTRY_DISABLED
        } else {
            ENTRY_ENABLED
        };
        gpu.draw_rect(x0, y0, x0 + SLOT_WIDTH, y0 + SLOT_HEIGHT, color);
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
    charge_shot_mesh: Option<ssb_rom::pack::MeshDesc>,
    /// Indexed by `SamusBomb::blink_palette`.
    bomb_meshes: [Option<ssb_rom::pack::MeshDesc>; 2],
    /// Link's Boomerang tree and its `anim_joints` spin.
    boomerang: Option<(ssb_rom::pack::ObjectDesc, ssb_rom::pack::AnimDesc)>,
    /// The Spin Attack swirl and its transform animation.
    spin_effect: Option<(ssb_rom::pack::ObjectDesc, ssb_rom::pack::AnimDesc)>,
    yoshi_egg_mesh: Option<ssb_rom::pack::MeshDesc>,
    yoshi_star_mesh: Option<ssb_rom::pack::MeshDesc>,
    /// The Falcon Punch flame (material animation only).
    falcon_punch: Option<ssb_rom::pack::ObjectDesc>,
    /// Pikachu's aerial and ground Thunder Jolts with their `anim_joints`.
    jolt_air: Option<(ssb_rom::pack::ObjectDesc, ssb_rom::pack::AnimDesc)>,
    jolt_ground: Option<(ssb_rom::pack::ObjectDesc, ssb_rom::pack::AnimDesc)>,
    /// Ness's PK Fire spark, PK Thunder head (with its scale pulse) and
    /// trail.
    pk_fire: Option<ssb_rom::pack::ObjectDesc>,
    pk_thunder: Option<(ssb_rom::pack::ObjectDesc, ssb_rom::pack::AnimDesc)>,
    pk_trail: Option<ssb_rom::pack::ObjectDesc>,
    /// The PK Fire flame and Link's Bomb items and their `anim_joints`.
    pk_fire_item: Option<(ssb_rom::pack::ObjectDesc, ssb_rom::pack::AnimDesc)>,
    link_bomb_item: Option<(ssb_rom::pack::ObjectDesc, ssb_rom::pack::AnimDesc)>,
    /// The shield bubble.
    shield: Option<ssb_rom::pack::ObjectDesc>,
    /// Ness's PSI Magnet field and its transform animation.
    magnet: Option<(ssb_rom::pack::ObjectDesc, ssb_rom::pack::AnimDesc)>,
    /// Jigglypuff's Sing notes and their transform animation.
    sing: Option<(ssb_rom::pack::ObjectDesc, ssb_rom::pack::AnimDesc)>,
    /// Kirby's Final Cutter wave tree and its `anim_joints` flicker.
    cutter: Option<(ssb_rom::pack::ObjectDesc, ssb_rom::pack::AnimDesc)>,
    /// The Falcon Kick flame tree and its transform animation.
    falcon_kick: Option<(ssb_rom::pack::ObjectDesc, ssb_rom::pack::AnimDesc)>,
}

impl DrawAssets {
    fn resolve(p: &Pack<'_>) -> Self {
        DrawAssets {
            shadow_texture: meshdraw::fighter_shadow_texture(p),
            fireball_meshes: ssb_psp_runtime::scene::fireball_meshes(p),
            blaster_mesh: ssb_psp_runtime::scene::fox_blaster_mesh(p),
            reflector: ssb_psp_runtime::scene::fox_reflector_object(p),
            charge_shot_mesh: ssb_psp_runtime::scene::samus_charge_shot_mesh(p),
            bomb_meshes: ssb_psp_runtime::scene::samus_bomb_meshes(p),
            boomerang: ssb_psp_runtime::scene::link_boomerang_object(p)
                .zip(p.weapon_anim(ssb_rom::pack::AnimDesc::WEAPON_ANIM_LINK_BOOMERANG)),
            spin_effect: ssb_psp_runtime::scene::link_spin_attack_effect(p)
                .and_then(|(object, slot)| Some((object, p.effect_anim(slot)?))),
            yoshi_egg_mesh: ssb_psp_runtime::scene::yoshi_egg_mesh(p),
            yoshi_star_mesh: ssb_psp_runtime::scene::yoshi_star_mesh(p),
            falcon_punch: ssb_psp_runtime::scene::captain_falcon_punch_effect(p),
            jolt_air: ssb_psp_runtime::scene::object_keyed(
                p,
                ssb_psp_runtime::scene::PIKACHU_JOLT_AIR_SOURCE,
            )
            .zip(p.weapon_anim(ssb_rom::pack::AnimDesc::WEAPON_ANIM_PIKACHU_JOLT_AIR)),
            jolt_ground: ssb_psp_runtime::scene::object_keyed(
                p,
                ssb_psp_runtime::scene::PIKACHU_JOLT_GROUND_SOURCE,
            )
            .zip(p.weapon_anim(ssb_rom::pack::AnimDesc::WEAPON_ANIM_PIKACHU_JOLT_GROUND)),
            pk_fire: ssb_psp_runtime::scene::object_keyed(
                p,
                ssb_psp_runtime::scene::NESS_PK_FIRE_SOURCE,
            ),
            pk_thunder: ssb_psp_runtime::scene::object_keyed(
                p,
                ssb_psp_runtime::scene::NESS_PK_THUNDER_SOURCE,
            )
            .zip(p.weapon_anim(ssb_rom::pack::AnimDesc::WEAPON_ANIM_NESS_PK_THUNDER)),
            pk_trail: ssb_psp_runtime::scene::object_keyed(
                p,
                ssb_psp_runtime::scene::NESS_PK_TRAIL_SOURCE,
            ),
            pk_fire_item: ssb_psp_runtime::scene::object_keyed(
                p,
                ssb_psp_runtime::scene::NESS_PK_FIRE_ITEM_SOURCE,
            )
            .zip(p.item_anim(ssb_rom::pack::AnimDesc::ITEM_ANIM_NESS_PK_FIRE)),
            link_bomb_item: ssb_psp_runtime::scene::object_keyed(
                p,
                ssb_psp_runtime::scene::LINK_BOMB_ITEM_SOURCE,
            )
            .zip(p.item_anim(ssb_rom::pack::AnimDesc::ITEM_ANIM_LINK_BOMB)),
            shield: ssb_psp_runtime::scene::object_keyed(p, ssb_psp_runtime::scene::SHIELD_EFFECT_KEY),
            magnet: ssb_psp_runtime::scene::ness_psi_magnet_effect(p)
                .and_then(|(object, slot)| Some((object, p.effect_anim(slot)?))),
            sing: ssb_psp_runtime::scene::purin_sing_effect(p)
                .and_then(|(object, slot)| Some((object, p.effect_anim(slot)?))),
            cutter: ssb_psp_runtime::scene::kirby_cutter_object(p)
                .zip(p.weapon_anim(ssb_rom::pack::AnimDesc::WEAPON_ANIM_KIRBY_CUTTER)),
            falcon_kick: ssb_psp_runtime::scene::captain_falcon_kick_effect(p)
                .and_then(|(object, slot)| Some((object, p.effect_anim(slot)?))),
        }
    }
}

/// The animation players for Link's Boomerang and Spin Attack swirl,
/// Captain Falcon's Falcon Punch and Falcon Kick flames, Kirby's Final
/// Cutter wave, Pikachu's Thunder Jolts, Jigglypuff's Sing notes and Ness's
/// PK Fire spark, PK Thunder head and PSI Magnet field. The game state counts each one's
/// `gcPlayAnimAll` calls; [`Self::sync`] plays the players up to that
/// count, restarting when it goes back.
#[derive(Default)]
struct EffectVisuals {
    boomerang: ssb_rom::skeleton::StageAnimator,
    boomerang_ticks: Option<u16>,
    spin: ssb_rom::skeleton::StageAnimator,
    spin_materials: ssb_rom::skeleton::EffectMaterialAnimator,
    spin_ticks: Option<u16>,
    punch_materials: ssb_rom::skeleton::EffectMaterialAnimator,
    punch_ticks: Option<u16>,
    kick: ssb_rom::skeleton::StageAnimator,
    kick_materials: ssb_rom::skeleton::EffectMaterialAnimator,
    kick_ticks: Option<u16>,
    cutter: ssb_rom::skeleton::StageAnimator,
    cutter_ticks: Option<u16>,
    jolts: [JoltVisual; MAX_JOLT_VISUALS],
    sing: ssb_rom::skeleton::StageAnimator,
    sing_materials: ssb_rom::skeleton::EffectMaterialAnimator,
    sing_ticks: Option<u16>,
    pk_fire_materials: ssb_rom::skeleton::EffectMaterialAnimator,
    pk_fire_ticks: Option<u16>,
    pk_thunder: ssb_rom::skeleton::StageAnimator,
    pk_thunder_materials: ssb_rom::skeleton::EffectMaterialAnimator,
    pk_thunder_ticks: Option<u16>,
    magnet: ssb_rom::skeleton::StageAnimator,
    magnet_materials: ssb_rom::skeleton::EffectMaterialAnimator,
    magnet_ticks: Option<u16>,
    items: [ItemVisual; MAX_ITEM_VISUALS],
}

/// Items drawn at once: Training has at most one PK Fire flame and one Bomb
/// per fighter.
const MAX_ITEM_VISUALS: usize = 4;

/// One item's players, keyed by its kind and restarted when its play count
/// goes back.
#[derive(Default)]
struct ItemVisual {
    kind: Option<ssb_game::item::ItemKind>,
    ticks: u16,
    anim: ssb_rom::skeleton::StageAnimator,
    materials: ssb_rom::skeleton::EffectMaterialAnimator,
}

impl DrawAssets {
    fn item(
        &self,
        kind: ssb_game::item::ItemKind,
    ) -> Option<&(ssb_rom::pack::ObjectDesc, ssb_rom::pack::AnimDesc)> {
        match kind {
            ssb_game::item::ItemKind::NessPKFire => self.pk_fire_item.as_ref(),
            ssb_game::item::ItemKind::LinkBomb => self.link_bomb_item.as_ref(),
        }
    }
}

/// The pose a stage player has reached for `node`, if it drives it.
fn stage_pose(
    anim: &ssb_rom::skeleton::StageAnimator,
    node: u32,
) -> Option<ssb_rom::figatree::JointPose> {
    (0..anim.joint_count())
        .filter_map(|i| anim.joint(i))
        .find(|&(n, _)| n == node)
        .map(|(_, pose)| *pose)
}

/// Thunder Jolts drawn at once; `ftPikachuSpecialNProcUpdate` fires one per
/// Thunder Jolt and each lives 100 frames.
const MAX_JOLT_VISUALS: usize = 4;

/// One Thunder Jolt's players, keyed by its animation epoch.
#[derive(Default)]
struct JoltVisual {
    epoch: Option<u16>,
    ticks: u16,
    anim: ssb_rom::skeleton::StageAnimator,
    materials: ssb_rom::skeleton::EffectMaterialAnimator,
}

/// `gcAddAnimAll`'s material half: the material scripts bound to an
/// effect object's own primitives (RE-175).
fn object_mat_anims<'p>(
    p: &'p Pack<'_>,
    object: &ssb_rom::pack::ObjectDesc,
) -> impl Iterator<Item = u32> + 'p {
    let (first, count) = (object.first_node, object.node_count);
    (0..count)
        .filter_map(move |n| p.node(first + n))
        .filter(|n| n.mesh != ssb_rom::pack::NodeDesc::NO_MESH)
        .filter_map(move |n| p.mesh(n.mesh))
        .flat_map(move |m| (0..m.prim_count).filter_map(move |i| p.prim(m.first_prim + i)))
        .map(|prim| prim.mat_anim)
}

/// `(restart, ticks)` that bring a player at `clock` to `target`.
fn catch_up(clock: &mut Option<u16>, target: Option<u16>) -> Option<(bool, u16)> {
    let Some(target) = target else {
        *clock = None;
        return None;
    };
    let step = match *clock {
        Some(at) if at <= target => (false, target - at),
        _ => (true, target),
    };
    *clock = Some(target);
    Some(step)
}

impl EffectVisuals {
    #[inline(never)]
    fn sync(
        &mut self,
        p: &Pack<'_>,
        assets: &DrawAssets,
        player: &ssb_game::fighter::Fighter,
        weapons: &ssb_game::weapon::WeaponPool,
        items: &ssb_game::item::ItemPool,
    ) {
        // The items in pool order; a kind change or a play count that went
        // back restarts a player.
        let mut live = items.items();
        for visual in &mut self.items {
            let Some(item) = live.next() else {
                visual.kind = None;
                continue;
            };
            let Some((object, anim)) = assets.item(item.kind) else {
                continue;
            };
            if visual.kind != Some(item.kind) || visual.ticks > item.anim_ticks {
                visual.kind = Some(item.kind);
                visual.ticks = 0;
                visual.anim.start(p, anim);
                visual.materials.start(p, object_mat_anims(p, object));
            }
            if let Some(script) = p.anim_script(anim) {
                while visual.ticks < item.anim_ticks {
                    let _ = visual.anim.tick(script);
                    visual.materials.tick(p);
                    visual.ticks += 1;
                }
            }
        }

        // `ftLinkSpecialNProcUpdate` allows one Boomerang per Link, and
        // Training has one Link.
        let boomerang = weapons.boomerangs().next().map(|b| b.anim_ticks);
        if let (Some((restart, ticks)), Some((_, anim))) = (
            catch_up(&mut self.boomerang_ticks, boomerang),
            assets.boomerang.as_ref(),
        ) {
            if restart {
                self.boomerang.start(p, anim);
            }
            if let Some(script) = p.anim_script(anim) {
                for _ in 0..ticks {
                    let _ = self.boomerang.tick(script);
                }
            }
        }

        // `ftKirbySpecialHiLanding` makes one wave per Final Cutter, and
        // Training has one Kirby.
        let cutter = weapons.cutters().next().map(|c| c.anim_ticks);
        if let (Some((restart, ticks)), Some((_, anim))) = (
            catch_up(&mut self.cutter_ticks, cutter),
            assets.cutter.as_ref(),
        ) {
            if restart {
                self.cutter.start(p, anim);
            }
            if let Some(script) = p.anim_script(anim) {
                for _ in 0..ticks {
                    let _ = self.cutter.tick(script);
                }
            }
        }

        // The jolts in pool order. A jolt whose epoch changed (it landed, or
        // `wpPikachuThunderJoltGroundAddAnim` restarted it) replays from the
        // start; the ground form plays at `gcSetAllAnimSpeed`'s 0.5.
        let mut jolts = weapons.jolts();
        for visual in &mut self.jolts {
            let Some(jolt) = jolts.next() else {
                visual.epoch = None;
                continue;
            };
            let ground = jolt.surface.is_some();
            let Some((object, anim)) = (if ground {
                assets.jolt_ground.as_ref()
            } else {
                assets.jolt_air.as_ref()
            }) else {
                continue;
            };
            if visual.epoch != Some(jolt.anim_epoch) || visual.ticks > jolt.anim_ticks {
                visual.epoch = Some(jolt.anim_epoch);
                visual.ticks = 0;
                visual.anim.start(p, anim);
                visual.materials.start(p, object_mat_anims(p, object));
            }
            let speed = if ground {
                ssb_game::weapon::JOLT_GROUND_ANIM_SPEED
            } else {
                1.0
            };
            if let Some(script) = p.anim_script(anim) {
                while visual.ticks < jolt.anim_ticks {
                    let _ = visual.anim.tick_speed(script, speed);
                    visual.materials.tick_speed(p, speed);
                    visual.ticks += 1;
                }
            }
        }

        // One PK Fire and one PK Thunder per Ness, and Training has one Ness.
        let pk_fire = weapons.pk_fires().next().map(|w| w.anim_ticks);
        if let (Some((restart, ticks)), Some(object)) = (
            catch_up(&mut self.pk_fire_ticks, pk_fire),
            assets.pk_fire.as_ref(),
        ) {
            if restart {
                self.pk_fire_materials.start(p, object_mat_anims(p, object));
            }
            for _ in 0..ticks {
                self.pk_fire_materials.tick(p);
            }
        }
        let pk_thunder = weapons.pk_thunders().next().map(|w| w.anim_ticks);
        if let (Some((restart, ticks)), Some((object, anim))) = (
            catch_up(&mut self.pk_thunder_ticks, pk_thunder),
            assets.pk_thunder.as_ref(),
        ) {
            if restart {
                self.pk_thunder.start(p, anim);
                self.pk_thunder_materials
                    .start(p, object_mat_anims(p, object));
            }
            if let Some(script) = p.anim_script(anim) {
                for _ in 0..ticks {
                    let _ = self.pk_thunder.tick(script);
                    self.pk_thunder_materials.tick(p);
                }
            }
        }

        let magnet = ssb_game::ness::magnet_effect_ticks(player);
        if let (Some((restart, ticks)), Some((object, anim))) = (
            catch_up(&mut self.magnet_ticks, magnet),
            assets.magnet.as_ref(),
        ) {
            if restart {
                self.magnet.start(p, anim);
                self.magnet_materials.start(p, object_mat_anims(p, object));
            }
            if let Some(script) = p.anim_script(anim) {
                for _ in 0..ticks {
                    let _ = self.magnet.tick(script);
                    self.magnet_materials.tick(p);
                }
            }
        }

        let sing = ssb_game::purin::sing_effect_ticks(player);
        if let (Some((restart, ticks)), Some((object, anim))) =
            (catch_up(&mut self.sing_ticks, sing), assets.sing.as_ref())
        {
            if restart {
                self.sing.start(p, anim);
                self.sing_materials.start(p, object_mat_anims(p, object));
            }
            if let Some(script) = p.anim_script(anim) {
                for _ in 0..ticks {
                    let _ = self.sing.tick(script);
                    self.sing_materials.tick(p);
                }
            }
        }

        let spin = ssb_game::link::spin_effect_ticks(player);
        if let (Some((restart, ticks)), Some((object, anim))) = (
            catch_up(&mut self.spin_ticks, spin),
            assets.spin_effect.as_ref(),
        ) {
            if restart {
                self.spin.start(p, anim);
                self.spin_materials.start(p, object_mat_anims(p, object));
            }
            if let Some(script) = p.anim_script(anim) {
                for _ in 0..ticks {
                    let _ = self.spin.tick(script);
                    self.spin_materials.tick(p);
                }
            }
        }

        let punch = ssb_game::captain::punch_effect_ticks(player);
        if let (Some((restart, ticks)), Some(object)) = (
            catch_up(&mut self.punch_ticks, punch),
            assets.falcon_punch.as_ref(),
        ) {
            if restart {
                self.punch_materials.start(p, object_mat_anims(p, object));
            }
            for _ in 0..ticks {
                self.punch_materials.tick(p);
            }
        }

        let kick = ssb_game::captain::kick_effect_ticks(player);
        if let (Some((restart, ticks)), Some((object, anim))) = (
            catch_up(&mut self.kick_ticks, kick),
            assets.falcon_kick.as_ref(),
        ) {
            if restart {
                self.kick.start(p, anim);
                self.kick_materials.start(p, object_mat_anims(p, object));
            }
            if let Some(script) = p.anim_script(anim) {
                for _ in 0..ticks {
                    let _ = self.kick.tick(script);
                    self.kick_materials.tick(p);
                }
            }
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
#[inline(never)]
unsafe fn draw_training(
    gpu: &mut Gpu,
    draw_state: &mut meshdraw::DrawState,
    pack: Option<&Pack<'_>>,
    stage_index: u32,
    play_state: Option<&play::FighterScene>,
    dummy_state: Option<&play::Dummy>,
    weapons: &ssb_game::weapon::WeaponPool,
    items: &ssb_game::item::ItemPool,
    assets: &DrawAssets,
    effect_visuals: &EffectVisuals,
    material_anim: Option<&ssb_rom::skeleton::MaterialAnimator>,
    stage_anim: Option<&ssb_rom::skeleton::StageAnimator>,
    stage_objects: Option<&ssb_rom::ground_obj::GroundObjects>,
    no_pack_color: Color,
    damage_hud: &Hud,
    battle: Option<&ssb_game::battle::Battle>,
) {
    let scene = pack
        .zip(play_state)
        .and_then(|(p, pl)| p.stage(stage_index).map(|s| (p, pl, s)));

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

    // `FTStruct::is_invisible` (a KO, a Kirby or Yoshi capture) skips the
    // model.
    if let Some(obj) = p.object(pl.object).filter(|_| !pl.fighter.is_invisible) {
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
                [0.0, play::fighter_turn(&pl.fighter), 0.0],
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
        if let Some(obj) = p.object(dummy.object).filter(|_| !dummy.fighter.is_invisible) {
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
                    [0.0, play::fighter_turn(&dummy.fighter), 0.0],
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

    draw_items_weapons_effects(
        gpu,
        draw_state,
        p,
        pl,
        dummy_state,
        weapons,
        items,
        assets,
        effect_visuals,
        material_anim,
    );
    // `players[].color`: the human's port, a CPU's `GMCOMMON_PLAYERS_MAX`.
    let fighters = [
        Some((pl.fighter.kind, 0)),
        dummy_state.map(|d| (d.fighter.kind, ssb_game::hud::CPU_COLOR)),
    ];
    // `ifCommonBattleInterfaceProcSet` hides every interface at Set, and
    // `ifCommonBattlePauseInitInterface` while paused, when only the pause
    // menu draws.
    if battle.is_some_and(|b| b.status == ssb_game::battle::GameStatus::Set) {
        return;
    }
    if let Some(pause) = damage_hud.pause {
        draw_pause_menu(gpu, p, draw_state, pause.kind);
        return;
    }
    draw_damage_hud(p, draw_state, &damage_hud.damage, fighters, stage_index);
    if let Some(b) = battle {
        let stocks = [
            Some(&pl.fighter),
            dummy_state.map(|d| &d.fighter),
        ];
        draw_stocks(p, draw_state, b, stocks);
        draw_timer(p, draw_state, b);
    }
    if let Some(c) = damage_hud.countdown.as_ref() {
        draw_countdown(p, draw_state, c);
    }
    if let Some(end) = battle.and_then(|b| b.end) {
        draw_announce(p, draw_state, end);
    }
}

/// `ifCommonPlayerDamageProcDisplay` for each fighter, over the 3D scene.
#[inline(never)]
fn draw_damage_hud(
    p: &Pack<'_>,
    draw_state: &mut meshdraw::DrawState,
    hud: &[ssb_game::hud::DamageDisplay; 2],
    fighters: [Option<(ssb_game::fighter::FighterKind, usize)>; 2],
    stage_index: u32,
) {
    // `ifCommonPlayerDamageSetDigitAttr`.
    const ATTR: u16 = ssb_rom::sprite::SP_TEXSHUF | ssb_rom::sprite::SP_TRANSPARENT;
    let f = &ssb_rom::sprite::PLAYER_DAMAGE;
    let mut sprites = [None; 12];
    let mut sizes = [(0u16, 0u16); 12];
    for (i, &at) in f.offsets.iter().enumerate() {
        sprites[i] = p.sprite(f.file, at);
        sizes[i] = sprites[i].map_or((0, 0), |s| (s.width, s.height));
    }
    let emblem_colors = p.stage(stage_index).map(|s| s.emblem_colors);
    for (d, fighter) in hud.iter().zip(fighters) {
        let Some((kind, color)) = fighter else {
            continue;
        };
        let emblem = p.fighter_sprite(kind as u8, ssb_rom::pack::SpriteDesc::ROLE_EMBLEM, 0);
        if let (Some(sprite), Some(colors)) = (emblem, emblem_colors) {
            let (x, y) = ssb_game::hud::emblem_origin(d.player, sprite.width, sprite.height);
            let [r, g, b] = colors[color];
            unsafe {
                let d = meshdraw::SObjDraw {
                    x,
                    y,
                    scale: 1.0,
                    prim: [r, g, b, 0xFF],
                    env: [0; 3],
                    solid: false,
                    attr: ATTR,
                };
                meshdraw::draw_sprite(p, &sprite, &d, draw_state);
            }
        }
        for g in d.glyphs(false, &sizes) {
            let Some(sprite) = sprites[usize::from(g.digit)] else {
                continue;
            };
            let [r, gr, b] = g.color;
            unsafe {
                let d = meshdraw::SObjDraw {
                    x: g.x,
                    y: g.y,
                    scale: g.scale,
                    prim: [r, gr, b, 0xFF],
                    env: [0; 3],
                    solid: g.solid,
                    attr: ATTR,
                };
                meshdraw::draw_sprite(p, &sprite, &d, draw_state);
            }
        }
    }
}

/// `ifCommonBattlePauseProcDisplay`'s white border, then the "1P" and the
/// decals (`ifCommonBattlePauseMakeInterface`).
#[inline(never)]
fn draw_pause_menu(
    gpu: &mut Gpu,
    p: &Pack<'_>,
    draw_state: &mut meshdraw::DrawState,
    kind: ssb_game::pause::PauseKind,
) {
    use ssb_game::pause;
    let (vx, _, _, vh) = ssb_engine::coord::pillarboxed_viewport();
    let k = vh as f32 / ssb_engine::coord::N64_SCREEN.1 as f32;
    let px = |x: i16| (vx as f32 + f32::from(x) * k) as i32;
    let py = |y: i16| (f32::from(y) * k) as i32;
    for [ulx, uly, lrx, lry] in pause::BORDER {
        // `G_CYC_FILL` covers both corners.
        gpu.draw_rect(px(ulx), py(uly), px(lrx + 1), py(lry + 1), Color::rgba(0xFF, 0xFF, 0xFF, 0xFF));
    }
    draw_state.invalidate_all();
    let f = &ssb_rom::sprite::BATTLE_PAUSE;
    let sprite = |i: u8| f.offsets.get(usize::from(i)).and_then(|&at| p.sprite(f.file, at));
    let mut draw = |i: u8, (x, y): (i16, i16), prim: [u8; 3], env: [u8; 3]| {
        if let Some(s) = sprite(i) {
            let d = meshdraw::SObjDraw {
                x: f32::from(x),
                y: f32::from(y),
                scale: 1.0,
                prim: [prim[0], prim[1], prim[2], 0xFF],
                env,
                solid: false,
                attr: ssb_game::countdown::ATTR_TRANSPARENT,
            };
            unsafe {
                meshdraw::draw_sprite(p, &s, &d, draw_state);
            }
        }
    };
    // Port 0 paused: "1P".
    draw(0, pause::PLAYER_NUM_POS, [0xFF; 3], [0; 3]);
    for d in pause::decals(kind) {
        draw(d.sprite, d.pos, d.prim, d.env);
    }
}

/// One `SP_TEXSHUF | SP_TRANSPARENT` sprite, untinted.
fn draw_plain(p: &Pack<'_>, draw_state: &mut meshdraw::DrawState, sprite: &ssb_rom::pack::SpriteDesc, x: f32, y: f32) {
    let d = meshdraw::SObjDraw {
        x,
        y,
        scale: 1.0,
        prim: [0xFF; 4],
        env: [0; 3],
        solid: false,
        attr: ssb_game::countdown::ATTR_TRANSPARENT,
    };
    unsafe {
        meshdraw::draw_sprite(p, sprite, &d, draw_state);
    }
}

/// `ifCommonPlayerStockSingleProcDisplay` in a time battle,
/// `...MultiProcDisplay` in a stock one: each fighter's stock icon in its
/// costume's palette.
#[inline(never)]
fn draw_stocks(
    p: &Pack<'_>,
    draw_state: &mut meshdraw::DrawState,
    b: &ssb_game::battle::Battle,
    fighters: [Option<&ssb_game::fighter::Fighter>; 2],
) {
    let single = b.rule == ssb_game::battle::Rule::Time;
    for (player, f) in fighters.into_iter().enumerate() {
        let Some(f) = f else { continue };
        let Some(icon) = p.fighter_sprite(f.kind as u8, ssb_rom::pack::SpriteDesc::ROLE_STOCK, f.costume) else {
            continue;
        };
        for (x, y) in ssb_game::hud::stock_icons(player, f.stocks, single, icon.width, icon.height) {
            draw_plain(p, draw_state, &icon, x, y);
        }
    }
}

/// `ifCommonTimerProcDisplay`: `M M : S S` in a timed battle.
#[inline(never)]
fn draw_timer(p: &Pack<'_>, draw_state: &mut meshdraw::DrawState, b: &ssb_game::battle::Battle) {
    use ssb_game::hud;
    if b.rule != ssb_game::battle::Rule::Time || b.time_limit == ssb_game::battle::TIMELIMIT_INFINITE {
        return;
    }
    let f = &ssb_rom::sprite::TIMER;
    let sprite = |i: u8| f.offsets.get(usize::from(i)).and_then(|&at| p.sprite(f.file, at));
    let limit = u32::from(b.time_limit) * ssb_game::battle::TICS_PER_MINUTE;
    let digits = hud::timer_digits(b.time_remain, limit);
    for (&x, digit) in hud::TIMER_X.iter().zip(digits) {
        if let Some(s) = sprite(digit) {
            let (x, y) = hud::timer_origin(x, s.width, s.height);
            draw_plain(p, draw_state, &s, x, y);
        }
    }
    if let Some(s) = sprite(hud::TIMER_COLON) {
        let (x, y) = hud::timer_origin(hud::TIMER_COLON_X, s.width, s.height);
        draw_plain(p, draw_state, &s, x, y);
    }
}

/// "TIME UP" (`ifCommonAnnounceTimeUpMakeInterface`) or "GAME SET"
/// (`ifCommonAnnounceGameSetMakeInterface`) until the battle's Set.
#[inline(never)]
fn draw_announce(p: &Pack<'_>, draw_state: &mut meshdraw::DrawState, end: ssb_game::battle::EndKind) {
    use ssb_game::hud;
    let letters: &[(f32, f32, u8)] = match end {
        ssb_game::battle::EndKind::TimeUp => &hud::TIME_UP,
        ssb_game::battle::EndKind::GameSet => &hud::GAME_SET,
    };
    let f = &ssb_rom::sprite::GAME_STATUS;
    for &(x, y, i) in letters {
        if let Some(s) = f.offsets.get(usize::from(i)).and_then(|&at| p.sprite(f.file, at)) {
            draw_plain(p, draw_state, &s, x, y);
        }
    }
}

/// `lbCommonDrawSObjAttr` over the countdown and "GO!" `SObj`s.
#[inline(never)]
fn draw_countdown(p: &Pack<'_>, draw_state: &mut meshdraw::DrawState, c: &ssb_game::countdown::Countdown) {
    if c.sudden_death_letters {
        let letters = &ssb_rom::sprite::ANNOUNCE_COMMON;
        for &(x, y, i) in &ssb_game::countdown::SUDDEN_DEATH {
            if let Some(s) = letters.offsets.get(usize::from(i)).and_then(|&at| p.sprite(letters.file, at)) {
                // `ifCommonAnnounceSetColors`: white on black.
                let d = meshdraw::SObjDraw {
                    x,
                    y,
                    scale: 1.0,
                    prim: [0xFF; 4],
                    env: [0; 3],
                    solid: false,
                    attr: ssb_game::countdown::ATTR_TRANSPARENT,
                };
                unsafe {
                    meshdraw::draw_sprite(p, &s, &d, draw_state);
                }
            }
        }
    }
    let f = &ssb_rom::sprite::GAME_STATUS;
    for o in c.sobjs() {
        let Some(sprite) = f
            .offsets
            .get(usize::from(o.sprite))
            .and_then(|&at| p.sprite(f.file, at))
        else {
            continue;
        };
        let d = meshdraw::SObjDraw {
            x: o.pos.0,
            y: o.pos.1,
            scale: o.scale,
            prim: [o.prim[0], o.prim[1], o.prim[2], 0xFF],
            env: o.env,
            solid: false,
            attr: o.attr,
        };
        unsafe {
            meshdraw::draw_sprite(p, &sprite, &d, draw_state);
        }
    }
}

/// The item pass (DL link 11) and the weapon and effect pass (links 13 to 15)
/// that follow the fighters. Kept out of [`draw_training`] so neither
/// function outgrows MIPS branch range.
#[inline(never)]
#[allow(clippy::too_many_arguments)]
unsafe fn draw_items_weapons_effects(
    gpu: &mut Gpu,
    draw_state: &mut meshdraw::DrawState,
    p: &Pack<'_>,
    pl: &play::FighterScene,
    dummy_state: Option<&play::Dummy>,
    weapons: &ssb_game::weapon::WeaponPool,
    items: &ssb_game::item::ItemPool,
    assets: &DrawAssets,
    effect_visuals: &EffectVisuals,
    material_anim: Option<&ssb_rom::skeleton::MaterialAnimator>,
) {
    // Items (DL link 11) draw in the fighters' camera pass, under the
    // camera's default head modes. The PK Fire flame is a
    // `TraRotRpyRSca` tree scaled by its lifetime (`itNessPKFireProcUpdate`);
    // its node 3 is a ROM billboard. Link's Bomb is `Tra` then kind 46 on its
    // root and child (RE-383): the child, the drawn node, is a camera-facing
    // quad spun by its own `rotate.z` and sized by the accumulated
    // `gGCScaleX` (root scale times its own). Its position is its translate
    // under the root's modelview: the item position in world axes, or, held,
    // the item joint under `itMainSetFighterHold`'s kind-82 parent
    // (`func_ovl0_800C9F70`, the joint matrix with its scale divided out). Link's Bomb is `Tra` then kind 46 on the
    // root and its child: the child draws as a camera-facing quad at the
    // item's position plus its own translate, sized by the root's scale times
    // its own and spun by its own `rotate.z` (`gcPrepDObjMatrix` kind 46
    // rewrites only the MVP's rotation rows and carries `gGCScaleX` down).
    for (item, visual) in items.items().zip(effect_visuals.items.iter()) {
        if item.hidden {
            continue;
        }
        let Some((object, _)) = assets.item(item.kind) else {
            continue;
        };
        match item.kind {
            ssb_game::item::ItemKind::NessPKFire => {
                let mut posed = [ssb_rom::scene::Mat4::IDENTITY; 8];
                let n = visual.anim.compose(p, object, &mut posed);
                gpu.model_transform_xyz(
                    [item.pos.x, item.pos.y, item.pos.z],
                    [0.0; 3],
                    [
                        meshdraw::MODEL_SCALE * item.scale.x,
                        meshdraw::MODEL_SCALE * item.scale.y,
                        meshdraw::MODEL_SCALE * item.scale.z,
                    ],
                );
                let base = gpu.model_matrix();
                meshdraw::draw_object_posed(
                    p,
                    object,
                    &base,
                    &posed[..n],
                    None,
                    draw_state,
                    None,
                    Some(&visual.materials),
                    0,
                );
            }
            ssb_game::item::ItemKind::LinkBomb => {
                let pose_of = |index: u32| {
                    let node = object.first_node + index;
                    stage_pose(&visual.anim, node).or_else(|| {
                        p.node(node).map(|n| ssb_rom::figatree::JointPose {
                            rotate: n.rest_rotate,
                            translate: n.rest_translate,
                            scale: n.rest_scale,
                        })
                    })
                };
                let mesh_of = |index: u32| {
                    p.node(object.first_node + index)
                        .filter(|n| n.mesh != ssb_rom::pack::NodeDesc::NO_MESH)
                        .and_then(|n| p.mesh(n.mesh))
                };
                let (Some(body), Some(spark)) = (pose_of(1), pose_of(2)) else {
                    continue;
                };
                let owner = item.owner.and_then(|port| {
                    if pl.fighter.port == port {
                        Some(&pl.fighter)
                    } else {
                        dummy_state.map(|d| &d.fighter).filter(|f| f.port == port)
                    }
                });
                let held = owner.filter(|_| item.is_hold).and_then(|f| {
                    let joint = match item.weight {
                        ssb_game::item::ItemWeight::Light => {
                            Some(ssb_game::item_throw::itemlight_joint(f.kind))
                        }
                        ssb_game::item::ItemWeight::Heavy => {
                            ssb_game::grab::itemheavy_joint(f.kind)
                        }
                    }?;
                    f.joint_transforms[joint]
                });
                let v = |pose: &ssb_rom::figatree::JointPose| {
                    ssb_engine::math::Vec3::new(
                        pose.translate[0],
                        pose.translate[1],
                        pose.translate[2],
                    )
                };
                let to_world = |local: ssb_engine::math::Vec3| match held {
                    Some(joint) => {
                        let axes = ssb_game::grab::held_root_axes(joint);
                        joint.origin + axes[0] * local.x + axes[1] * local.y + axes[2] * local.z
                    }
                    None => item.pos + local,
                };
                // Held, `itMainSetFighterHold` resets the root to its desc
                // scale; loose, the bloat scales the root.
                let root_scale = if held.is_some() {
                    [1.0, 1.0]
                } else {
                    [item.scale.x, item.scale.y]
                };
                if let Some(mesh) = mesh_of(1) {
                    gpu.model_transform_billboard(
                        to_world(v(&body)),
                        pl.camera.eye,
                        pl.camera.at,
                        body.rotate[2],
                        [
                            meshdraw::MODEL_SCALE * root_scale[0] * body.scale[0],
                            meshdraw::MODEL_SCALE * root_scale[1] * body.scale[1],
                        ],
                    );
                    meshdraw::draw_mesh(p, &mesh, draw_state, None, Some(&visual.materials));
                }
                // Node 2 (the fuse spark) is plain `Tra`: no rotation or scale
                // of its own, in the parent's frame.
                if let Some(mesh) = mesh_of(2) {
                    let pos = to_world(v(&body) + v(&spark));
                    match held {
                        Some(joint) => gpu.model_transform_joint(pos, joint, meshdraw::MODEL_SCALE),
                        None => gpu.model_transform(
                            [pos.x, pos.y, pos.z],
                            [0.0; 3],
                            meshdraw::MODEL_SCALE,
                        ),
                    }
                    meshdraw::draw_mesh(p, &mesh, draw_state, None, Some(&visual.materials));
                }
            }
        }
    }

    // Weapons (DL link 14) and effects (DL link 15) draw in the camera's
    // links-13-to-15 pass, after the pass that draws the fighters (link 9)
    // and items (link 11): `gmCameraProcDisplay`'s `camera_mask` order.
    // Most of them clear `G_ZBUFFER`, so a fighter drawn later would cover
    // them (RE-379).
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

    // The Charge Shot is `gcPrepDObjMatrix` kind 46: a camera-facing quad
    // spun by `rotate.z` and scaled by its level's `gfx_size / 30`, both
    // while it charges on Samus's arm and after release.
    if let Some(shot_mesh) = assets.charge_shot_mesh.as_ref() {
        let mut draw_shot = |pos: ssb_engine::math::Vec3, spin: f32, scale: f32| {
            gpu.model_transform_billboard(
                pos,
                pl.camera.eye,
                pl.camera.at,
                spin,
                [meshdraw::MODEL_SCALE * scale; 2],
            );
            meshdraw::draw_mesh(p, shot_mesh, draw_state, None, None);
        };
        if ssb_game::samus::is_charging(&pl.fighter) {
            draw_shot(
                ssb_game::samus::charge_shot_position(&pl.fighter),
                ssb_game::samus::charge_shot_rotate_z(&pl.fighter),
                ssb_game::weapon::samus_charge_shot_scale(pl.fighter.samus.charge_level),
            );
        }
        for shot in weapons.charge_shots() {
            draw_shot(shot.position, shot.rotate_z, shot.scale());
        }
    }

    // Yoshi's egg is `TraRotRpyRSca` then kind 46. Kind 46 overwrites the
    // rotation and scale with the camera-facing spin by `rotate.z`, so only
    // the main transform's translation survives: the same billboard as the
    // Charge Shot. In hand, `ftYoshiSpecialHiUpdateEggVectors` places it at
    // the Egg Throw joint with that joint's scale (1 in the rest pose). The
    // explosion clears its display list.
    if let Some(egg_mesh) = assets.yoshi_egg_mesh.as_ref() {
        let mut draw_egg = |pos: ssb_engine::math::Vec3, spin: f32| {
            gpu.model_transform_billboard(
                pos,
                pl.camera.eye,
                pl.camera.at,
                spin,
                [meshdraw::MODEL_SCALE; 2],
            );
            meshdraw::draw_mesh(p, egg_mesh, draw_state, None, None);
        };
        if let Some(pos) = ssb_game::yoshi::held_egg_position(&pl.fighter) {
            draw_egg(pos, 0.0);
        }
        for egg in weapons.eggs().filter(|egg| !egg.exploded) {
            draw_egg(egg.position, egg.rotate_z);
        }
    }

    // Yoshi's star is a plain `TraRotRpyRSca` DObj: spun about Z and shrunk
    // in X and Y by `wpYoshiStarGetScale`.
    if let Some(star_mesh) = assets.yoshi_star_mesh.as_ref() {
        for star in weapons.stars() {
            let scale = star.scale();
            gpu.model_transform_xyz(
                [star.position.x, star.position.y, star.position.z],
                [0.0, 0.0, star.rotate_z],
                [
                    meshdraw::MODEL_SCALE * scale,
                    meshdraw::MODEL_SCALE * scale,
                    meshdraw::MODEL_SCALE,
                ],
            );
            meshdraw::draw_mesh(p, star_mesh, draw_state, None, None);
        }
    }

    // The Bomb's second transform is battle matrix function 70 - 66 = 4,
    // `func_ovl0_800CA194` (`dLBCommonFuncMatrixList`): it keeps the
    // translation and replaces rotation and scale with a spin about world Z.
    // The explosion clears the DObj's display list, so it draws nothing.
    for bomb in weapons.bombs().filter(|bomb| !bomb.exploded) {
        let Some(bomb_mesh) = assets
            .bomb_meshes
            .get(usize::from(bomb.blink_palette))
            .and_then(Option::as_ref)
        else {
            continue;
        };
        gpu.model_transform(
            [bomb.position.x, bomb.position.y, bomb.position.z],
            [0.0, 0.0, bomb.rotate_z],
            meshdraw::MODEL_SCALE,
        );
        meshdraw::draw_mesh(p, bomb_mesh, draw_state, None, None);
    }

    // Link's Boomerang is a `WEAPON_FLAG_DOBJDESC` tree. Its root takes the
    // weapon's position in place of the `DObjDesc` translate, and
    // `wpMainVelSetModelPitch`'s `rotate.y`; node 1 spins under the
    // `anim_joints` stream. The return hides node 2 (`DOBJ_FLAG_NOTEXTURE`).
    if let Some((object, _)) = assets.boomerang.as_ref() {
        for boomerang in weapons.boomerangs() {
            let mut posed = [ssb_rom::scene::Mat4::IDENTITY; 4];
            let n = effect_visuals.boomerang.compose(p, object, &mut posed);
            if let Some(root) = p.node(object.first_node) {
                let t = root.rest_translate.map(|v| -v / meshdraw::MODEL_SCALE);
                let unplace = ssb_rom::scene::Mat4::from_trs(t, [0.0; 3], [1.0; 3]);
                for m in &mut posed[..n] {
                    *m = unplace.mul(m);
                }
            }
            gpu.model_transform(
                [
                    boomerang.position.x,
                    boomerang.position.y,
                    boomerang.position.z,
                ],
                [0.0, boomerang.model_rotate_y, 0.0],
                meshdraw::MODEL_SCALE,
            );
            let base = gpu.model_matrix();
            let grandchild = object.first_node + 2;
            let hidden = |node: u32| boomerang.grandchild_hidden && node == grandchild;
            meshdraw::draw_object_posed_hiding(
                p,
                object,
                &base,
                &posed[..n],
                draw_state,
                None,
                &hidden,
            );
        }
    }

    // Pikachu's aerial Thunder Jolt is one DObj: `Tra` then kind 46, the
    // camera-facing billboard spun by the animated `rotate.z` and sized by
    // its `scale.x`/`scale.y`. The ground jolt is a `DObjDesc` tree whose
    // root takes the position and `rotate` (0, `model_rotate_y`, the line's
    // slope); nodes 2-7 blink through their `SetFlags` events.
    for (jolt, visual) in weapons.jolts().zip(effect_visuals.jolts.iter()) {
        if jolt.surface.is_none() {
            let Some((object, _)) = assets.jolt_air.as_ref() else {
                continue;
            };
            let Some(mesh) = p
                .node(object.first_node)
                .filter(|n| n.mesh != ssb_rom::pack::NodeDesc::NO_MESH)
                .and_then(|n| p.mesh(n.mesh))
            else {
                continue;
            };
            let pose = visual.anim.joint(0).map(|(_, pose)| *pose);
            let (spin, scale) = pose.map_or((0.0, [1.0, 1.0]), |pose| {
                (pose.rotate[2], [pose.scale[0], pose.scale[1]])
            });
            gpu.model_transform_billboard(
                jolt.position,
                pl.camera.eye,
                pl.camera.at,
                spin,
                [
                    meshdraw::MODEL_SCALE * scale[0],
                    meshdraw::MODEL_SCALE * scale[1],
                ],
            );
            meshdraw::draw_mesh(p, &mesh, draw_state, None, Some(&visual.materials));
        } else if let Some((object, _)) = assets.jolt_ground.as_ref() {
            let mut posed = [ssb_rom::scene::Mat4::IDENTITY; 8];
            let n = visual.anim.compose(p, object, &mut posed);
            let place = ssb_rom::scene::Mat4::from_trs(
                [0.0; 3],
                [0.0, jolt.model_rotate_y, jolt.rotate_z()],
                [1.0; 3],
            );
            for m in &mut posed[..n] {
                *m = place.mul(m);
            }
            gpu.model_transform(
                [jolt.position.x, jolt.position.y, jolt.position.z],
                [0.0; 3],
                meshdraw::MODEL_SCALE,
            );
            let base = gpu.model_matrix();
            let hidden = |node: u32| !visual.anim.visible(p, node);
            meshdraw::draw_object_posed_hiding(
                p,
                object,
                &base,
                &posed[..n],
                draw_state,
                Some(&visual.materials),
                &hidden,
            );
        }
    }

    // Ness's PK Fire spark and PK Thunder head are `Tra` then kind 46: a
    // camera-facing quad spun by `rotate.z`. The head's root pulses its
    // scale; its child, the drawn node, adds its own kind 46 at scale 1. The
    // trail is one `TraRotRpyRSca` DObj turned about Z along its path.
    let first_mesh = |object: &ssb_rom::pack::ObjectDesc, node: u32| {
        p.node(object.first_node + node)
            .filter(|n| n.mesh != ssb_rom::pack::NodeDesc::NO_MESH)
            .and_then(|n| p.mesh(n.mesh))
    };
    if let Some(mesh) = assets.pk_fire.as_ref().and_then(|o| first_mesh(o, 0)) {
        for spark in weapons.pk_fires() {
            gpu.model_transform_billboard(
                spark.position,
                pl.camera.eye,
                pl.camera.at,
                spark.rotate_z,
                [meshdraw::MODEL_SCALE; 2],
            );
            meshdraw::draw_mesh(
                p,
                &mesh,
                draw_state,
                None,
                Some(&effect_visuals.pk_fire_materials),
            );
        }
    }
    if let Some((object, _)) = assets.pk_thunder.as_ref() {
        if let Some(mesh) = first_mesh(object, 1) {
            let scale = stage_pose(&effect_visuals.pk_thunder, object.first_node)
                .map_or([1.0, 1.0], |pose| [pose.scale[0], pose.scale[1]]);
            for head in weapons.pk_thunders() {
                gpu.model_transform_billboard(
                    head.position,
                    pl.camera.eye,
                    pl.camera.at,
                    head.rotate_z(),
                    [
                        meshdraw::MODEL_SCALE * scale[0],
                        meshdraw::MODEL_SCALE * scale[1],
                    ],
                );
                meshdraw::draw_mesh(
                    p,
                    &mesh,
                    draw_state,
                    None,
                    Some(&effect_visuals.pk_thunder_materials),
                );
            }
        }
    }
    if let Some(mesh) = assets.pk_trail.as_ref().and_then(|o| first_mesh(o, 0)) {
        for trail in weapons.pk_trails() {
            gpu.model_transform(
                [trail.position.x, trail.position.y, trail.position.z],
                [0.0, 0.0, trail.rotation()],
                meshdraw::MODEL_SCALE,
            );
            meshdraw::draw_mesh(p, &mesh, draw_state, None, None);
        }
    }

    // The shield bubble (`efManagerShieldMakeEffect`, RE-384). Its root is
    // battle matrix function 79, `YRotN`'s whole matrix, which the guard
    // scales by the shield size; the drawn node adds kind 44, a camera-facing
    // quad sized by that accumulated scale. `efManagerShieldProcDisplay` sets
    // PRIM white and ENV the player's colour, both at alpha 0xC0. Yoshi's
    // egg shield is a different effect and is not drawn.
    if let Some(object) = assets.shield.as_ref() {
        let fighters = core::iter::once(&pl.fighter).chain(dummy_state.map(|d| &d.fighter));
        for f in fighters.filter(|f| f.guard.is_shield && f.kind != ssb_game::fighter::FighterKind::Yoshi) {
            let joint = ssb_game::combat::shield_transform(f);
            let size = joint.axes[0].length();
            let (prim, env) = ssb_psp_runtime::scene::SHIELD_COLORS[usize::from(f.port).min(3)];
            draw_state.color_override = Some(ssb_rom::skeleton::EffectColors {
                prim: Some([prim[0], prim[1], prim[2], 0xC0]),
                env: Some([env[0], env[1], env[2], 0xC0]),
                ..Default::default()
            });
            for n in 0..object.node_count {
                let Some(node) = p.node(object.first_node + n) else {
                    continue;
                };
                let Some(mesh) = (node.mesh != ssb_rom::pack::NodeDesc::NO_MESH)
                    .then(|| p.mesh(node.mesh))
                    .flatten()
                else {
                    continue;
                };
                let offset = ssb_engine::math::Vec3::new(
                    node.rest_translate[0],
                    node.rest_translate[1],
                    node.rest_translate[2],
                );
                gpu.model_transform_billboard(
                    joint.point(offset),
                    pl.camera.eye,
                    pl.camera.at,
                    0.0,
                    [
                        meshdraw::MODEL_SCALE * size * node.rest_scale[0],
                        meshdraw::MODEL_SCALE * size * node.rest_scale[1],
                    ],
                );
                meshdraw::draw_mesh(p, &mesh, draw_state, None, None);
            }
            draw_state.color_override = None;
        }
    }

    // Ness's PSI Magnet field. The root is battle matrix function 80, a pure
    // translation to TopN's world position; each child is `Tra` then kind
    // 46, a camera-facing quad spun by its `rotate.z` and sized by its
    // `scale.x`/`scale.y`. So each drawn node is a billboard at TopN plus
    // its composed offset, in world axes.
    if let (Some(_), Some((object, _))) = (
        ssb_game::ness::magnet_effect_ticks(&pl.fighter),
        assets.magnet.as_ref(),
    ) {
        let mut posed = [ssb_rom::scene::Mat4::IDENTITY; 8];
        let n = effect_visuals.magnet.compose(p, object, &mut posed);
        let top = pl.fighter.joint_world(0, ssb_engine::math::Vec3::ZERO);
        for (i, local) in posed[..n].iter().enumerate() {
            let node_index = object.first_node + i as u32;
            let Some(mesh) = p
                .node(node_index)
                .filter(|node| node.mesh != ssb_rom::pack::NodeDesc::NO_MESH)
                .and_then(|node| p.mesh(node.mesh))
            else {
                continue;
            };
            let pos = top
                + ssb_engine::math::Vec3::new(
                    local.0[12] * meshdraw::MODEL_SCALE,
                    local.0[13] * meshdraw::MODEL_SCALE,
                    local.0[14] * meshdraw::MODEL_SCALE,
                );
            let (spin, scale) = stage_pose(&effect_visuals.magnet, node_index)
                .map_or((0.0, [1.0, 1.0]), |pose| {
                    (pose.rotate[2], [pose.scale[0], pose.scale[1]])
                });
            gpu.model_transform_billboard(
                pos,
                pl.camera.eye,
                pl.camera.at,
                spin,
                [
                    meshdraw::MODEL_SCALE * scale[0],
                    meshdraw::MODEL_SCALE * scale[1],
                ],
            );
            meshdraw::draw_mesh(
                p,
                &mesh,
                draw_state,
                None,
                Some(&effect_visuals.magnet_materials),
            );
        }
    }

    // Jigglypuff's Sing notes. The root is battle matrix function 79 - 66 =
    // 13, `func_ovl0_800C994C`: TopN's whole world matrix. Node 1 adds kind
    // 70 (`func_ovl0_800CA194`), a camera-facing spin by `rotate.z`; nodes
    // 3-5 add kind 42, a camera-facing quad. Neither keeps any scale, so
    // each drawn node is a billboard at its composed position. Node 2's
    // `RotRpyR` swings the notes around it. `efManagerHaveStructProcUpdate`
    // ejects the effect when its animation ends.
    if let (Some(_), Some((object, _))) = (
        ssb_game::purin::sing_effect_ticks(&pl.fighter),
        assets.sing.as_ref(),
    ) {
        if !effect_visuals.sing.ended() {
            let mut posed = [ssb_rom::scene::Mat4::IDENTITY; 8];
            let n = effect_visuals.sing.compose(p, object, &mut posed);
            for (i, local) in posed[..n].iter().enumerate() {
                let node_index = object.first_node + i as u32;
                let Some(mesh) = p
                    .node(node_index)
                    .filter(|node| node.mesh != ssb_rom::pack::NodeDesc::NO_MESH)
                    .and_then(|node| p.mesh(node.mesh))
                else {
                    continue;
                };
                let offset = ssb_engine::math::Vec3::new(
                    local.0[12] * meshdraw::MODEL_SCALE,
                    local.0[13] * meshdraw::MODEL_SCALE,
                    local.0[14] * meshdraw::MODEL_SCALE,
                );
                let pos = pl.fighter.joint_world(0, offset);
                let spin = if i == 1 {
                    stage_pose(&effect_visuals.sing, node_index).map_or(0.0, |pose| pose.rotate[2])
                } else {
                    0.0
                };
                gpu.model_transform_billboard(
                    pos,
                    pl.camera.eye,
                    pl.camera.at,
                    spin,
                    [meshdraw::MODEL_SCALE; 2],
                );
                meshdraw::draw_mesh(
                    p,
                    &mesh,
                    draw_state,
                    None,
                    Some(&effect_visuals.sing_materials),
                );
            }
        }
    }

    // Kirby's Final Cutter wave is a `WEAPON_FLAG_DOBJDESC` tree. Its root
    // takes the weapon's position in place of the desc translate, and
    // `RotRpyR` of `wpMainVelSetModelPitch`'s yaw and the floor slope; node 1
    // flickers under the `anim_joints` stream.
    if let Some((object, _)) = assets.cutter.as_ref() {
        for cutter in weapons.cutters() {
            let mut posed = [ssb_rom::scene::Mat4::IDENTITY; 4];
            let n = effect_visuals.cutter.compose(p, object, &mut posed);
            if let Some(root) = p.node(object.first_node) {
                let t = root.rest_translate.map(|v| -v / meshdraw::MODEL_SCALE);
                let place = ssb_rom::scene::Mat4::from_trs(
                    [0.0; 3],
                    [0.0, cutter.model_rotate_y, cutter.rotate_z],
                    [1.0; 3],
                )
                .mul(&ssb_rom::scene::Mat4::from_trs(t, [0.0; 3], [1.0; 3]));
                for m in &mut posed[..n] {
                    *m = place.mul(m);
                }
            }
            gpu.model_transform(
                [cutter.position.x, cutter.position.y, cutter.position.z],
                [0.0; 3],
                meshdraw::MODEL_SCALE,
            );
            let base = gpu.model_matrix();
            meshdraw::draw_object_posed(
                p,
                object,
                &base,
                &posed[..n],
                None,
                draw_state,
                None,
                None,
                0,
            );
        }
    }

    // The Spin Attack swirl's root is battle matrix function 80 - 66 = 14,
    // `func_ovl0_800C99CC`: a translation to TopN's world position. Its
    // second transform, `RotRpyR`, applies the 30- or 210-degree yaw
    // `efManagerLinkSpinAttackMakeEffect` sets.
    if let (Some(_), Some((object, _))) = (
        ssb_game::link::spin_effect_ticks(&pl.fighter),
        assets.spin_effect.as_ref(),
    ) {
        let mut posed = [ssb_rom::scene::Mat4::IDENTITY; 4];
        let n = effect_visuals.spin.compose(p, object, &mut posed);
        gpu.model_transform(
            [pl.fighter.pos.x, pl.fighter.pos.y, pl.fighter.pos.z],
            [0.0, ssb_game::link::spin_effect_rotate_y(&pl.fighter), 0.0],
            meshdraw::MODEL_SCALE,
        );
        let base = gpu.model_matrix();
        meshdraw::draw_object_posed(
            p,
            object,
            &base,
            &posed[..n],
            None,
            draw_state,
            material_anim,
            Some(&effect_visuals.spin_materials),
            0,
        );
    }

    // The Falcon Punch flame is one DObj: battle matrix function 80 places
    // it at joint 16's world position, and `RotRpyR` applies the
    // `lr * -90`-degree yaw `efManagerCaptainFalconPunchMakeEffect` sets.
    if let (Some(_), Some(object)) = (
        ssb_game::captain::punch_effect_ticks(&pl.fighter),
        assets.falcon_punch.as_ref(),
    ) {
        let pos = pl.fighter.joint_world(
            ssb_game::captain::PUNCH_EFFECT_JOINT,
            ssb_engine::math::Vec3::ZERO,
        );
        gpu.model_transform(
            [pos.x, pos.y, pos.z],
            [
                0.0,
                ssb_game::captain::punch_effect_rotate_y(&pl.fighter),
                0.0,
            ],
            meshdraw::MODEL_SCALE,
        );
        let base = gpu.model_matrix();
        meshdraw::draw_object_posed(
            p,
            object,
            &base,
            &[],
            None,
            draw_state,
            material_anim,
            Some(&effect_visuals.punch_materials),
            0,
        );
    }

    // The Falcon Kick flame is a `DObjDesc` tree. Battle matrix function 80
    // replaces the root's desc translate with joint 23's world position, and
    // `RotRpyR` applies the root rotation
    // `efManagerCaptainFalconKickMakeEffect` sets. `Mat4::from_trs` is that
    // x-then-y-then-z order.
    if let (Some(_), Some((object, _))) = (
        ssb_game::captain::kick_effect_ticks(&pl.fighter),
        assets.falcon_kick.as_ref(),
    ) {
        let mut posed = [ssb_rom::scene::Mat4::IDENTITY; 4];
        let n = effect_visuals.kick.compose(p, object, &mut posed);
        if let Some(root) = p.node(object.first_node) {
            let t = root.rest_translate.map(|v| -v / meshdraw::MODEL_SCALE);
            let place = ssb_rom::scene::Mat4::from_trs(
                [0.0; 3],
                ssb_game::captain::kick_effect_rotate(&pl.fighter),
                [1.0; 3],
            )
            .mul(&ssb_rom::scene::Mat4::from_trs(t, [0.0; 3], [1.0; 3]));
            for m in &mut posed[..n] {
                *m = place.mul(m);
            }
        }
        let pos = pl.fighter.joint_world(
            ssb_game::captain::KICK_EFFECT_JOINT,
            ssb_engine::math::Vec3::ZERO,
        );
        gpu.model_transform([pos.x, pos.y, pos.z], [0.0; 3], meshdraw::MODEL_SCALE);
        let base = gpu.model_matrix();
        meshdraw::draw_object_posed(
            p,
            object,
            &base,
            &posed[..n],
            None,
            draw_state,
            material_anim,
            Some(&effect_visuals.kick_materials),
            0,
        );
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
                [0.0, play::fighter_turn(&pl.fighter), 0.0],
                meshdraw::MODEL_SCALE,
            );
            let base = gpu.model_matrix();
            meshdraw::draw_object(p, reflector, &base, draw_state, None, 0);
        }
    }
}
