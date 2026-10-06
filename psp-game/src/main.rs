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

mod campaign;
mod capture;
mod demo_screen;
mod menus_screen;
mod opening_screen;
mod play;
mod player_screen;
mod players_screen;
mod results_screen;
mod save;
mod stage_screen;
mod title_opening;
mod training_screen;

use ssb_engine::input::{newly_pressed, ControllerState, Input, N64Buttons, SSB64_GAME_MAPPING};
use ssb_engine::renderer::Color;
use ssb_game::menu::Scene as MScene;
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
/// `vsresults`' capture tick (RE-409). At tick 760 the results are up with
/// no fighters yet, so they started after 640 and the fighters come after
/// 760; Kirby's 161-frame Win clips have ended by 1041.
const VS_RESULTS_CAPTURE_TICK: u64 = 1100;
/// `vsteamsteal`'s stick-left windows, the START tap that steals and the
/// capture: the icon in flight (RE-464).
const VS_TEAM_STEAL_RUNS: [core::ops::RangeInclusive<u64>; 2] = [398..=520, 800..=900];
const VS_TEAM_STEAL_START_TICK: u64 = 1060;
const VS_TEAM_STEAL_CAPTURE_TICK: u64 = 1075;
/// The capture tick on which the `rebirth` scenes' Mario falls past Dream
/// Land's bottom blast line (`ftCommonDeadDownSetStatus`), logged from a
/// capture build (RE-412).
const REBIRTH_KO_TICK: u64 = 160;
/// `starko`'s tick: Mario, settled on Dream Land's floor, is put in
/// `DeadUpStar` (`ftCommonDeadUpStarSetStatus`) as a top-out picks it,
/// without the random draw between the star and the fall (RE-420).
const STAR_KO_TICK: u64 = 60;
/// `vsresultsemblem`'s capture tick: results tic 100. A capture build's
/// results log shows tic 0 before tick 643's update (RE-420).
const VS_RESULTS_EMBLEM_CAPTURE_TICK: u64 = 742;
/// `vsshield`'s capture tick (RE-418, RE-438). A capture build's log shows
/// the CPU's hit setting the player's shield off
/// (`ftCommonGuardSetOffSetStatus`) in tick 884's update, the only update
/// that leaves the grey row selected. A scene freezes before its capture
/// tick's update, so freezing at 885 draws that state.
const VS_SHIELD_SET_OFF_TICK: u64 = 885;

/// `saveunlock`'s A: the message's two-second input wait, from its
/// fixture at tick 105, has passed.
const SAVE_UNLOCK_TICK: u64 = 240;

const fn capture_ticks(scene: GameScene) -> u64 {
    match scene {
        GameScene::OnePGame => 600,
        GameScene::OnePIntro => 150,
        GameScene::OnePBonus => 150,
        GameScene::OnePTargetClear => 850,
        GameScene::OnePTargetFall => 670,
        GameScene::OnePPlatforms => 600,
        GameScene::OnePPlatformClear => 950,
        GameScene::OnePPlatformFall => 750,
        GameScene::OnePRace => 600,
        GameScene::OnePRaceClear => 750,
        GameScene::OnePRaceFall => 750,
        GameScene::OnePRaceHazards => 660,
        GameScene::OnePBoss => 1500,
        GameScene::OnePBossDefeat => 1500,
        GameScene::OnePMetal | GameScene::OnePGiant | GameScene::OnePZako => 1200,
        GameScene::OnePContinue => 240,
        GameScene::OnePRetry => 320,
        GameScene::OnePClear => 270,
        GameScene::OnePEnding => 400,
        GameScene::OnePStaffroll => 900,
        GameScene::OnePCongra => 200,
        GameScene::OnePChallenger => 150,
        GameScene::OnePMessage => 150,
        // `onepmessage`'s A at [`SAVE_UNLOCK_TICK`], then the next scene.
        GameScene::SaveUnlock => 260,
        GameScene::SavePlayers => 85,
        // The menus read input from their tenth tic (RE-461).
        GameScene::Option | GameScene::DataMenu => 40,
        GameScene::ScreenAdjust => 30,
        GameScene::BackupClear => 50,
        GameScene::VsRecord => 40,
        GameScene::Characters => 90,
        // The title from tic 169: "Press Start" shows at tic 280 (RE-462).
        GameScene::Title => 140,
        // The title's demos (RE-465): How to Play's tap-the-stick phase,
        // the auto demo's focus on player 1.
        GameScene::Explain => 300,
        GameScene::AutoDemo => 420,
        // The N64 logo at rest (RE-467); an opening scene's 30th tic.
        GameScene::Opening => 30,
        GameScene::OpeningRoom
        | GameScene::OpeningPortraits
        | GameScene::OpeningMario
        | GameScene::OpeningDonkey
        | GameScene::OpeningLink
        | GameScene::OpeningSamus
        | GameScene::OpeningYoshi
        | GameScene::OpeningKirby
        | GameScene::OpeningFox
        | GameScene::OpeningPikachu
        | GameScene::OpeningRun
        | GameScene::OpeningCliff
        | GameScene::OpeningYamabuki
        | GameScene::OpeningJungle
        | GameScene::OpeningYoster
        | GameScene::OpeningSector
        | GameScene::OpeningStandoff
        | GameScene::OpeningClash
        | GameScene::OpeningNewcomers => 30,
        GameScene::ModeSelect | GameScene::OnePMode => 40,
        GameScene::VsOptions | GameScene::ItemSwitch => 40,
        // The bonus select places a fighter at tick 40.
        GameScene::BonusSelect => 70,
        // The course from tick 51; its timer has run some 200 tics.
        GameScene::BonusPractice => 300,
        GameScene::OnePFinale => 2200,
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
        // The KO below Dream Land is at tick REBIRTH_KO_TICK; a few ticks
        // on, the blast and the screen flash are up.
        GameScene::RebirthBlast => REBIRTH_KO_TICK + 8,
        // 150 ticks into the flight: some 12,400 units behind the floor,
        // past the old 10,000-unit far plane (RE-420).
        GameScene::StarKo => STAR_KO_TICK + 150,
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
        // The VS character select with Yoshi placed and a CPU opened.
        GameScene::VsPlayers => 85,
        // Reset at 520; the results follow Set's three ticks. By 640 the
        // no-contest table has its KOs (tic 60) and falls (tic 80) rows.
        GameScene::VsNoContest => 640,
        // The dummy's CPU has paced for some 190 ticks, or jumped several
        // times.
        GameScene::CpuWalk => 200,
        GameScene::CpuJump => 120,
        // After "Go" the CPU closes in; at tick 900 its fiery hit lands
        // (7%). Locked fighters run no CPU during the countdown (RE-438).
        GameScene::VsCpu => 900,
        // "Go" at 398. The camera frames the player alone, so this is a
        // tick where the CPUs' fight has drawn all four into its view.
        GameScene::Vs4 => 870,
        // "Go" at 398. From tick 935 Kirby's attack overlaps Mario, his
        // teammate, for 14 ticks and passes through (Team Attack off).
        GameScene::VsTeam => 940,
        // The player's second fall, then START asleep (RE-464).
        GameScene::VsTeamSteal => VS_TEAM_STEAL_CAPTURE_TICK,
        // "Go" at 398; Luigi, held left, falls off Dream Land and the
        // one-stock battle ends. The results make the fighters 120 tics
        // in; this is past the end of Kirby's Win clip.
        GameScene::VsResults => VS_RESULTS_CAPTURE_TICK,
        // Results tic 100: the emblem has shrunk and risen for 61 tics; the
        // text and confetti come at 120 (RE-420).
        GameScene::VsResultsEmblem => VS_RESULTS_EMBLEM_CAPTURE_TICK,
        GameScene::PikachuThunder => 92,
        GameScene::KirbyHat => 260,
        GameScene::YoshiEgg => 460,
        // Z from tick 40: the egg is up from the end of `GuardOn`, and 16
        // ticks a point of the shield's 55 decays, darkening it.
        GameScene::YoshiShield => 600,
        GameScene::YoshiRollF | GameScene::YoshiRollB => 69,
        GameScene::YoshiShieldBreak => 84,
        // "Go" at 398 with Z held; the CPU's first hit on the shield sets
        // it off on this tick (the capture log's `re418` lines).
        GameScene::VsShield => VS_SHIELD_SET_OFF_TICK,
        // The select opens at tick 8 and moves at 20 (and 24); by 60 the
        // preview camera has bobbed and the model played 36 ticks.
        GameScene::StageSelectView | GameScene::StageSelectYoshi => 60,
        GameScene::VsSector | GameScene::VsYoshi => 300,
        // Tick 400, the frame of the N64 warp-boot captures RE-422 compares:
        // both fighters stand at their spawns and the camera has settled.
        GameScene::TrainingJungle
        | GameScene::TrainingZebes
        | GameScene::TrainingSaffron
        | GameScene::TrainingInishie
        | GameScene::TrainingYoster
        | GameScene::TrainingSector
        | GameScene::TrainingCastle
        | GameScene::TrainingHyrule
        | GameScene::TrainingPupupu => 400,
        // RE-428: the first pattern's low pass, its wing in the top right.
        GameScene::TrainingArwing => 1800,
        GameScene::TrainingBumper | GameScene::TrainingPlants => 240,
        // The CPU has jumped to the Bat's platform and holds it.
        GameScene::TrainingCpuItem => 200,
        // The menu closes at 34; the CPU has walked and the Tomato landed.
        GameScene::TrainingMenu => 150,
        GameScene::TrainingInterface => 50,
        GameScene::TrainingCapsule
        | GameScene::TrainingCrate
        | GameScene::TrainingBarrel
        | GameScene::TrainingHeavy
        | GameScene::TrainingUtility
        | GameScene::TrainingThrowable
        | GameScene::TrainingSlide
        | GameScene::TrainingBumperThrow
        | GameScene::TrainingPokemonA
        | GameScene::TrainingPokemonB
        | GameScene::TrainingPokemonC
        | GameScene::TrainingPokemonD
        | GameScene::TrainingEquipment
        | GameScene::TrainingRayGun
        | GameScene::TrainingFlower
        | GameScene::TrainingStarRod
        | GameScene::TrainingHammer
        | GameScene::TrainingChansey
        | GameScene::TrainingElectrode
        | GameScene::TrainingCharmander
        | GameScene::TrainingVenusaur
        | GameScene::TrainingPorygon => 90,
        // The select opens at tick 8; at its tick 60 the portraits are in
        // and the CPU's puck shows.
        GameScene::TrainingSelect => 68,
        // The CPU's puck is placed again at tick 80; the banner shows 15 of
        // every 20 ticks from there, and by 110 Mario has turned back to
        // the front and plays his Win3 clip.
        GameScene::TrainingSelectPicked => 110,
        // Both fighters have settled on Dream Land's main floor.
        GameScene::PikachuHat | GameScene::PurinBow => 40,
        // The entry focus (`ifCommonEntryFocusThread`) starts from battle
        // clock 91; these ticks put each vehicle mid-flight (RE-425).
        GameScene::VsArwing => VS_ARWING_CAPTURE_TICK,
        GameScene::VsCar => VS_CAR_CAPTURE_TICK,
        GameScene::VsBall => VS_BALL_CAPTURE_TICK,
        GameScene::VsRays => VS_RAYS_CAPTURE_TICK,
    }
}

/// RE-425's entry captures. The capture RNG gives the entry focus id 1, so
/// the first fighter enters at tick 204 and the second at 219: the Arwing
/// is 50 frames into its flight, the car 45 into its drive, the ball 26
/// into its throw, and the ball 50 frames in has opened under its rays.
const VS_ARWING_CAPTURE_TICK: u64 = 254;
const VS_CAR_CAPTURE_TICK: u64 = 264;
const VS_BALL_CAPTURE_TICK: u64 = 230;
const VS_RAYS_CAPTURE_TICK: u64 = 254;

/// `true` once `regression_capture`'s scripted input has run past its fixed
/// script and reached its capture tick; always `false` otherwise, so callers
/// need one guard, not a cfg per call site (mirrors `psp-asset-viewer/main.rs`'s function
/// of the same name).
#[inline]
fn deterministic_capture_frozen(scene: Option<GameScene>, sim_frame_index: u64) -> bool {
    scene.is_some_and(|scene| sim_frame_index >= capture::tick_override().unwrap_or(capture_ticks(scene)))
}

/// The `training<stage>` scenes (RE-422): Training on one stage with no
/// input after the menu confirm.
fn is_training_stage_scene(scene: GameScene) -> bool {
    matches!(
        scene,
        GameScene::TrainingJungle
            | GameScene::TrainingZebes
            | GameScene::TrainingSaffron
            | GameScene::TrainingInishie
            | GameScene::TrainingYoster
            | GameScene::TrainingSector
            | GameScene::TrainingArwing
            | GameScene::TrainingCastle
            | GameScene::TrainingBumper
            | GameScene::TrainingPlants
            | GameScene::TrainingCapsule
            | GameScene::TrainingCrate
            | GameScene::TrainingBarrel
            | GameScene::TrainingHeavy
            | GameScene::TrainingUtility
            | GameScene::TrainingThrowable
            | GameScene::TrainingSlide
            | GameScene::TrainingBumperThrow
            | GameScene::TrainingPokemonA
            | GameScene::TrainingPokemonB
            | GameScene::TrainingPokemonC
            | GameScene::TrainingPokemonD
            | GameScene::TrainingEquipment
            | GameScene::TrainingRayGun
            | GameScene::TrainingFlower
            | GameScene::TrainingStarRod
            | GameScene::TrainingHammer
            | GameScene::TrainingCpuItem
            | GameScene::TrainingChansey
            | GameScene::TrainingElectrode
            | GameScene::TrainingCharmander
            | GameScene::TrainingVenusaur
            | GameScene::TrainingPorygon
            | GameScene::TrainingHyrule
            | GameScene::TrainingPupupu
    )
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
/// same tick 8 (`play_state`/`dummies` are created and ticked once
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
    // Backup Clear: down three times to VS Record, then A for its confirm.
    match (scene, tick) {
        (GameScene::VsTeamSteal, VS_TEAM_STEAL_START_TICK) => return N64Buttons(N64Buttons::START),
        (GameScene::BackupClear, 15 | 30 | 45) => return N64Buttons(N64Buttons::D_DOWN),
        (GameScene::BackupClear, 47) => return N64Buttons(N64Buttons::A),
        // VS Record and Characters start on Data, so the capture also
        // swaps the scenes' sprite packs.
        (GameScene::VsRecord, 14) => return N64Buttons(N64Buttons::D_DOWN),
        (GameScene::VsRecord, 16 | 30) | (GameScene::Characters, 15) => return N64Buttons(N64Buttons::A),
        // The front end's menus (RE-462): down once on the mode select and
        // twice on the 1P mode menu; the damage ratio up on VS Options
        // (down three rows, right twice); the Sword off on the Item
        // Switch (down a row, A).
        (GameScene::ModeSelect, 15) | (GameScene::OnePMode, 15 | 30) => return N64Buttons(N64Buttons::D_DOWN),
        (GameScene::VsOptions, 12 | 15 | 18) | (GameScene::ItemSwitch, 15) => return N64Buttons(N64Buttons::D_DOWN),
        (GameScene::VsOptions, 22 | 26) => return N64Buttons(N64Buttons::D_RIGHT),
        (GameScene::ItemSwitch, 22) => return N64Buttons(N64Buttons::A),
        // The bonus select: the held puck dropped on the portrait the
        // hand reached.
        (GameScene::BonusSelect | GameScene::BonusPractice, 40) => return N64Buttons(N64Buttons::A),
        (GameScene::BonusPractice, 50) => return N64Buttons(N64Buttons::START),
        (GameScene::Option | GameScene::ScreenAdjust | GameScene::BackupClear | GameScene::DataMenu | GameScene::VsRecord | GameScene::Characters | GameScene::Title | GameScene::ModeSelect | GameScene::OnePMode | GameScene::VsOptions | GameScene::ItemSwitch | GameScene::BonusSelect | GameScene::BonusPractice | GameScene::Explain | GameScene::AutoDemo | GameScene::Opening, _) => {
            return N64Buttons(0)
        }
        (g, _) if opening_capture_kind(g).is_some() => return N64Buttons(0),
        _ => {}
    }
    if matches!(scene, GameScene::OnePGame | GameScene::OnePIntro | GameScene::OnePBonus | GameScene::OnePTargetClear | GameScene::OnePTargetFall | GameScene::OnePPlatforms | GameScene::OnePPlatformClear | GameScene::OnePPlatformFall | GameScene::OnePRace | GameScene::OnePRaceClear | GameScene::OnePRaceFall | GameScene::OnePRaceHazards | GameScene::OnePBoss | GameScene::OnePBossDefeat | GameScene::OnePMetal | GameScene::OnePGiant | GameScene::OnePZako | GameScene::OnePContinue | GameScene::OnePRetry | GameScene::OnePClear | GameScene::OnePEnding | GameScene::OnePStaffroll | GameScene::OnePCongra | GameScene::OnePChallenger | GameScene::OnePMessage | GameScene::OnePFinale) {
        return match tick {
            4 | 10 | 34 => N64Buttons(N64Buttons::A),
            166 if scene == GameScene::OnePGame => N64Buttons(N64Buttons::A),
            270 if scene == GameScene::OnePRetry => N64Buttons(N64Buttons::A),
            t if scene == GameScene::OnePStaffroll && t >= 400 && t % 20 == 0 => N64Buttons(N64Buttons::A),
            930 if scene == GameScene::OnePFinale => N64Buttons(N64Buttons::START),
            t if scene == GameScene::OnePFinale && t >= 150 && t % 150 == 0 => N64Buttons(N64Buttons::A),
            74 => N64Buttons(N64Buttons::START),
            _ => N64Buttons(0),
        };
    }
    if scene == GameScene::TrainingInterface {
        return match tick {
            4 | 8 => N64Buttons(N64Buttons::A),
            60 | 70 | 80 | 84 => N64Buttons(N64Buttons::START),
            _ => N64Buttons(0),
        };
    }
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
    // The same select left open on Hyrule Castle, or moved down once to
    // Yoshi's Island.
    if scene == GameScene::StageSelectView {
        return match tick {
            4 | 8 => N64Buttons(N64Buttons::A),
            20 | 24 => N64Buttons(N64Buttons::D_RIGHT),
            _ => N64Buttons(0),
        };
    }
    if scene == GameScene::StageSelectYoshi {
        return match tick {
            4 | 8 => N64Buttons(N64Buttons::A),
            20 => N64Buttons(N64Buttons::D_DOWN),
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
    // `trainingselect` only opens the select. In
    // `trainingselectpicked` the stick carries the held puck onto Kirby as
    // in `fighterselect` and C-Down at 32 places it in his third costume;
    // the cursor then moves onto the CPU's puck on Mario, A at 66 (past the
    // 30-tick grab wait) grabs it and C-Right at 80 places it back in
    // Mario's second costume.
    if scene == GameScene::TrainingSelect {
        return match tick {
            4 | 8 => N64Buttons(N64Buttons::A),
            _ => N64Buttons(0),
        };
    }
    if scene == GameScene::TrainingSelectPicked {
        return match tick {
            4 | 8 | 66 => N64Buttons(N64Buttons::A),
            32 => N64Buttons(N64Buttons::C_DOWN),
            80 => N64Buttons(N64Buttons::C_RIGHT),
            _ => N64Buttons(0),
        };
    }
    // The VS scenes move the menu cursor down to VS at tick 6
    // (`scripted_stick_y`) and confirm it at 8.
    if matches!(
        scene,
        GameScene::Vs
            | GameScene::VsSector
            | GameScene::VsYoshi
            | GameScene::VsTimeUp
            | GameScene::VsTimeUpSign
            | GameScene::VsSuddenDeath
            | GameScene::VsCpu
            | GameScene::VsPause
            | GameScene::VsModeMenu
            | GameScene::VsNoContest
            | GameScene::VsPlayers
            | GameScene::Vs4
            | GameScene::VsTeam
            | GameScene::VsTeamSteal
            | GameScene::VsResults
            | GameScene::VsResultsEmblem
            | GameScene::VsShield
            | GameScene::VsArwing
            | GameScene::VsCar
            | GameScene::VsBall
            | GameScene::VsRays
    ) {
        return match tick {
            4 | 8 => N64Buttons(N64Buttons::A),
            // `vsshield`: the shield is held from "Go".
            t if scene == GameScene::VsShield && t >= 398 => N64Buttons(N64Buttons::Z),
            // `vsplayers`: A on the VS mode menu's Start at 19; the
            // select's first tick is 21. A at 46 places the held puck on
            // Yoshi, and A at 62 on port 2's NA button opens a CPU.
            // VS Start loads the select the tick after its A
            // (`mnVSModeMain`'s exit interrupt).
            19 | 46 | 62 if scene == GameScene::VsPlayers => N64Buttons(N64Buttons::A),
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
    // `trainingmenu`: START at 20 opens Training's menu; with the stick
    // (`scripted_stick_x`/`_y`) CP goes to Walk at 22, the cursor down to
    // Item at 25 and Item to Maxim Tomato at 27; A at 29 drops it and
    // START at 34 closes the menu.
    if scene == GameScene::TrainingMenu {
        return match tick {
            4 | 8 | 29 => N64Buttons(N64Buttons::A),
            20 | 34 => N64Buttons(N64Buttons::START),
            _ => N64Buttons(0),
        };
    }
    if matches!(scene, GameScene::CpuWalk | GameScene::CpuJump) {
        return match tick {
            4 | 8 => N64Buttons(N64Buttons::A),
            _ => N64Buttons(0),
        };
    }
    if matches!(scene, GameScene::Rebirth | GameScene::RebirthBlast | GameScene::StarKo) {
        return match tick {
            4 | 8 => N64Buttons(N64Buttons::A),
            _ => N64Buttons(0),
        };
    }
    // The costume scenes only confirm into Training; their pick is preset
    // (`capture_training_scene`). The stage scenes stand still.
    if matches!(
        scene,
        GameScene::Costume1
            | GameScene::Costume2
            | GameScene::Costume3
            | GameScene::PikachuHat
            | GameScene::PurinBow
            | GameScene::TrainingJungle
            | GameScene::TrainingZebes
            | GameScene::TrainingSaffron
            | GameScene::TrainingInishie
            | GameScene::TrainingYoster
            | GameScene::TrainingSector
            | GameScene::TrainingArwing
            | GameScene::TrainingCastle
            | GameScene::TrainingBumper
            | GameScene::TrainingPlants
            | GameScene::TrainingCapsule
            | GameScene::TrainingCrate
            | GameScene::TrainingBarrel
            | GameScene::TrainingHeavy
            | GameScene::TrainingUtility
            | GameScene::TrainingThrowable
            | GameScene::TrainingSlide
            | GameScene::TrainingBumperThrow
            | GameScene::TrainingPokemonA
            | GameScene::TrainingPokemonB
            | GameScene::TrainingPokemonC
            | GameScene::TrainingPokemonD
            | GameScene::TrainingEquipment
            | GameScene::TrainingRayGun
            | GameScene::TrainingFlower
            | GameScene::TrainingStarRod
            | GameScene::TrainingHammer
            | GameScene::TrainingCpuItem
            | GameScene::TrainingChansey
            | GameScene::TrainingElectrode
            | GameScene::TrainingCharmander
            | GameScene::TrainingVenusaur
            | GameScene::TrainingPorygon
            | GameScene::TrainingHyrule
            | GameScene::TrainingPupupu
    ) {
        return match tick {
            4 | 8 => N64Buttons(N64Buttons::A),
            60 if matches!(
                scene,
                GameScene::TrainingHeavy | GameScene::TrainingUtility | GameScene::TrainingThrowable
                    | GameScene::TrainingSlide | GameScene::TrainingBumperThrow
                    | GameScene::TrainingRayGun
                    | GameScene::TrainingFlower
                    | GameScene::TrainingStarRod
                    | GameScene::TrainingHammer
            ) =>
            {
                N64Buttons(N64Buttons::A)
            }
            100 if matches!(
                scene,
                GameScene::TrainingThrowable
                    | GameScene::TrainingSlide
                    | GameScene::TrainingBumperThrow
                    | GameScene::TrainingRayGun
                    | GameScene::TrainingStarRod
            ) =>
            {
                N64Buttons(N64Buttons::A)
            }
            100..=150 if scene == GameScene::TrainingFlower => N64Buttons(N64Buttons::A),
            110 if scene == GameScene::TrainingHeavy => N64Buttons(N64Buttons::B),
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
            // Extended controls hold through the critical bloat, then throw
            // backward. The original Link Bomb capture stays at tick 55.
            300 if scene == GameScene::LinkBomb => N64Buttons(N64Buttons::A),
            // PSI Magnet lasts while B is held.
            t if scene == GameScene::NessMagnet && t > 20 => N64Buttons(N64Buttons::B),
            _ => N64Buttons(0),
        };
    }
    // `kirbyhat` and `yoshiegg` take the grab scene's route onto the
    // dummy's platform, then B: Kirby inhales and, with a second B from
    // `SpecialNWait`, copies (`ftKirbySpecialNCopyCheckGotoCopy`); Yoshi's
    // tongue swallows and lays the dummy.
    if matches!(scene, GameScene::KirbyHat | GameScene::YoshiEgg) {
        return match tick {
            4 | 8 => N64Buttons(N64Buttons::A),
            13 | 30 if scene == GameScene::KirbyHat => N64Buttons(N64Buttons::C_UP),
            108 | 170 if scene == GameScene::KirbyHat => N64Buttons(N64Buttons::B),
            // Yoshi walks under the platform and jumps twice straight up.
            // The aerial tongue at 250 drops him onto the platform beside
            // the dummy; joint 31, the tongue tip, reaches 340-785 units
            // ahead in the catch window, so he backs off, turns and tongues
            // the dummy at 330 (RE-432).
            130 | 148 if scene == GameScene::YoshiEgg => N64Buttons(N64Buttons::C_UP),
            250 | 330 if scene == GameScene::YoshiEgg => N64Buttons(N64Buttons::B),
            _ => N64Buttons(0),
        };
    }
    // `pikachuthunder`: Pikachu dashes right, out from under the top
    // platform that would stop the head, then Down+B.
    if scene == GameScene::PikachuThunder {
        return match tick {
            4 | 8 => N64Buttons(N64Buttons::A),
            50 => N64Buttons(N64Buttons::B),
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
    if matches!(scene, GameScene::YoshiShield | GameScene::YoshiRollF | GameScene::YoshiRollB | GameScene::YoshiShieldBreak) {
        return match tick {
            4 | 8 => N64Buttons(N64Buttons::A),
            t if t >= 40 && (scene != GameScene::YoshiShieldBreak || t < 80) => {
                N64Buttons(N64Buttons::Z)
            }
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
    if capture_menu(scene).is_some() {
        return 0;
    }
    if matches!(scene, GameScene::OnePGame | GameScene::OnePIntro | GameScene::OnePBonus | GameScene::OnePTargetClear | GameScene::OnePTargetFall | GameScene::OnePPlatforms | GameScene::OnePPlatformClear | GameScene::OnePPlatformFall | GameScene::OnePRace | GameScene::OnePRaceClear | GameScene::OnePRaceFall | GameScene::OnePRaceHazards | GameScene::OnePBoss | GameScene::OnePBossDefeat | GameScene::OnePMetal | GameScene::OnePGiant | GameScene::OnePZako | GameScene::OnePContinue | GameScene::OnePRetry | GameScene::OnePClear | GameScene::OnePEnding | GameScene::OnePStaffroll | GameScene::OnePCongra | GameScene::OnePChallenger | GameScene::OnePMessage | GameScene::OnePFinale) {
        return if (14..=24).contains(&tick) { 80 } else { 0 };
    }
    if scene == GameScene::LinkBomb && tick == 300 {
        return 80;
    }
    if scene == GameScene::TrainingInterface {
        return if tick == 68 || tick == 82 { 80 } else { 0 };
    }
    if is_training_stage_scene(scene) {
        return 0;
    }
    if scene == GameScene::TrainingMenu {
        return if tick == 22 || tick == 27 { 80 } else { 0 };
    }
    // The cursor moves 4 pixels a tick at 80: from (40, 170) to (84, 102)
    // over Yoshi, then to (116, 134) on port 2's NA button.
    if scene == GameScene::VsPlayers {
        return if (24..=34).contains(&tick) || (50..=57).contains(&tick) { 80 } else { 0 };
    }
    // Eleven ticks at 80 move the cursor 44 pixels right, from x 70 to 114.
    if scene == GameScene::FighterSelect {
        return if (12..=22).contains(&tick) { 80 } else { 0 };
    }
    // Then 44 back left to x 70, over the CPU's puck.
    if scene == GameScene::TrainingSelect {
        return 0;
    }
    if scene == GameScene::TrainingSelectPicked {
        return match tick {
            12..=22 => 80,
            36..=46 => -80,
            _ => 0,
        };
    }
    if scene == GameScene::Shield {
        return if (50..=60).contains(&tick) { 40 } else { 0 };
    }
    // Held left, Mario dashes off the main floor's left end.
    if matches!(scene, GameScene::Rebirth | GameScene::RebirthBlast) {
        return if (20..=100).contains(&tick) { -80 } else { 0 };
    }
    if matches!(
        scene,
        GameScene::Vs
            | GameScene::VsSector
            | GameScene::VsYoshi
            | GameScene::VsTimeUp
            | GameScene::VsTimeUpSign
            | GameScene::VsSuddenDeath
            | GameScene::VsCpu
            | GameScene::VsPause
            | GameScene::VsModeMenu
            | GameScene::VsNoContest
            | GameScene::Vs4
            | GameScene::VsTeam
            | GameScene::VsShield
            | GameScene::VsArwing
            | GameScene::VsCar
            | GameScene::VsBall
            | GameScene::VsRays
            | GameScene::CpuWalk
            | GameScene::CpuJump
    ) {
        return 0;
    }
    // `vsteamsteal`: off the stage from "Go", and again once reborn.
    if scene == GameScene::VsTeamSteal {
        return if VS_TEAM_STEAL_RUNS.iter().any(|r| r.contains(&tick)) { -80 } else { 0 };
    }
    // `vsresults`: held left from "Go", the player runs off the stage.
    if matches!(scene, GameScene::VsResults | GameScene::VsResultsEmblem) {
        return if (398..=520).contains(&tick) { -80 } else { 0 };
    }
    if matches!(scene, GameScene::Grab | GameScene::Jab | GameScene::KirbyHat) {
        return if (14..52).contains(&tick) { -30 } else { 0 };
    }
    if scene == GameScene::YoshiEgg {
        // Then backs off right and turns to face the dummy (RE-432).
        return match tick {
            14..123 => -30,
            296..308 => 40,
            318..320 => -30,
            _ => 0,
        };
    }
    if matches!(scene, GameScene::YoshiRollF | GameScene::YoshiRollB) {
        return if (60..62).contains(&tick) {
            // Dream Land spawn 0 is at x=0 and faces left.
            if scene == GameScene::YoshiRollF { -80 } else { 80 }
        } else { 0 };
    }
    if matches!(
        scene,
        GameScene::Costume1
            | GameScene::Costume2
            | GameScene::Costume3
            | GameScene::PikachuHat
            | GameScene::PurinBow
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
            | GameScene::StageSelectView
            | GameScene::StageSelectYoshi
            | GameScene::YoshiShield
            | GameScene::YoshiShieldBreak
    ) {
        return 0;
    }
    if scene == GameScene::SamusBomb {
        return if (80..100).contains(&tick) { 80 } else { 0 };
    }
    if scene == GameScene::PikachuThunder {
        return if (10..=34).contains(&tick) { 80 } else { 0 };
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
    // `bonusselect`: the hand and its puck up onto the portraits.
    if matches!(scene, GameScene::BonusSelect | GameScene::BonusPractice) {
        return if (12..=32).contains(&tick) { 80 } else { 0 };
    }
    if capture_menu(scene).is_some() {
        return 0;
    }
    if matches!(scene, GameScene::OnePGame | GameScene::OnePIntro | GameScene::OnePBonus | GameScene::OnePTargetClear | GameScene::OnePTargetFall | GameScene::OnePPlatforms | GameScene::OnePPlatformClear | GameScene::OnePPlatformFall | GameScene::OnePRace | GameScene::OnePRaceClear | GameScene::OnePRaceFall | GameScene::OnePRaceHazards | GameScene::OnePBoss | GameScene::OnePBossDefeat | GameScene::OnePMetal | GameScene::OnePGiant | GameScene::OnePZako | GameScene::OnePContinue | GameScene::OnePRetry | GameScene::OnePClear | GameScene::OnePEnding | GameScene::OnePStaffroll | GameScene::OnePCongra | GameScene::OnePChallenger | GameScene::OnePMessage | GameScene::OnePFinale) {
        // Two separate menu-down edges, then carry the puck to Kirby.
        return match tick {
            6 | 8 => -80,
            14..=30 => 80,
            _ => 0,
        };
    }
    if scene == GameScene::TrainingInterface {
        return if tick == 62 || tick == 64 || tick == 66 {
            -80
        } else {
            0
        };
    }
    if is_training_stage_scene(scene) {
        return 0;
    }
    if scene == GameScene::TrainingMenu {
        return if tick == 25 { -80 } else { 0 };
    }
    if scene == GameScene::VsPlayers {
        return match tick {
            6 => -80,
            24..=40 => 80,
            50..=57 => -80,
            _ => 0,
        };
    }
    if matches!(
        scene,
        GameScene::Vs
            | GameScene::VsSector
            | GameScene::VsYoshi
            | GameScene::VsTimeUp
            | GameScene::VsTimeUpSign
            | GameScene::VsSuddenDeath
            | GameScene::VsCpu
            | GameScene::VsPause
            | GameScene::VsModeMenu
            | GameScene::VsNoContest
            | GameScene::Vs4
            | GameScene::VsTeam
            | GameScene::VsTeamSteal
            | GameScene::VsResults
            | GameScene::VsResultsEmblem
            | GameScene::VsShield
            | GameScene::VsArwing
            | GameScene::VsCar
            | GameScene::VsBall
            | GameScene::VsRays
    ) {
        return if tick == 6 { -80 } else { 0 };
    }
    // Seventeen ticks at 80 move the cursor 68 pixels up, from y 170 to
    // 102: the held puck's centre lands on Kirby's portrait.
    if scene == GameScene::FighterSelect {
        return if (12..=28).contains(&tick) { 80 } else { 0 };
    }
    // Then 48 up to y 54, onto the CPU's puck on Mario.
    if scene == GameScene::TrainingSelect {
        return 0;
    }
    if scene == GameScene::TrainingSelectPicked {
        return match tick {
            12..=28 => 80,
            36..=47 => 80,
            _ => 0,
        };
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
        GameScene::SamusBomb
            | GameScene::YoshiBomb
            | GameScene::CaptainKick
            | GameScene::LinkBomb
    ) && tick == 20)
        || (scene == GameScene::PikachuThunder && tick == 50)
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
pub(crate) const JUMP_BUTTON_MASK: u16 =
    N64Buttons::C_UP | N64Buttons::C_DOWN | N64Buttons::C_LEFT | N64Buttons::C_RIGHT;

/// N64-stick deflection used by the temporary front end for a single menu
/// move.  This is deliberately a raw N64-stick threshold: the PSP layer has
/// already converted the nub and neither this screen nor gameplay knows PSP
/// button identities.
const MENU_STICK_NAV_MIN: i8 = 40;

/// The capture log's opening line every 30 ticks of an opening scene:
/// the user partition's free memory, for the opening packs' headroom
/// (RE-467).
#[cfg(feature = "headless_capture")]
#[inline(never)]
fn log_opening_memory(sim_frame_index: u64, scene: MScene) {
    let MScene::Opening(kind) = scene else {
        return;
    };
    if sim_frame_index % 30 != 0 {
        return;
    }
    let line = alloc::format!(
        "opening tick={} scene={:?} free={} max_free={}\n",
        sim_frame_index,
        kind,
        unsafe { psp::sys::sceKernelTotalFreeMemSize() },
        unsafe { psp::sys::sceKernelMaxFreeMemSize() },
    );
    unsafe {
        psp::sys::sceIoWrite(
            psp::sys::sceKernelStdout(),
            line.as_ptr() as *const core::ffi::c_void,
            line.len(),
        );
    }
}

/// The capture log's results line: the tick and the results tic before
/// this frame's (RE-420).
#[cfg(feature = "headless_capture")]
#[inline(never)]
fn log_results_tic(sim_frame_index: u64, r: Option<&ssb_game::results::Results>) {
    let Some(r) = r else {
        return;
    };
    let line = alloc::format!("results tick={} tic={}\n", sim_frame_index, r.total_tics);
    unsafe {
        psp::sys::sceIoWrite(
            psp::sys::sceKernelStdout(),
            line.as_ptr() as *const core::ffi::c_void,
            line.len(),
        );
    }
}

/// The capture log's entry lines (RE-425), every frame of the entry
/// scenes: each fighter's status, entry clocks and facing.
#[cfg(feature = "headless_capture")]
#[inline(never)]
fn log_entry_state(
    capture_scene: Option<GameScene>,
    sim_frame_index: u64,
    player: Option<&play::FighterScene>,
    dummy: Option<&play::Dummy>,
) {
    // `explain`'s fighters each frame, against an N64 trace (RE-465).
    if capture_scene == Some(GameScene::Explain) {
        for f in [player.map(|p| &p.fighter), dummy.map(|d| &d.fighter)].into_iter().flatten() {
            let line = alloc::format!(
                "re465 tick={} port={} status={:?} damage={} x={:.2} y={:.2} lr={:?} af={:.2} vg={:.3} va={:.3},{:.3} lag={} id={} seed={}\n",
                sim_frame_index,
                f.port,
                f.status.status,
                f.damage,
                f.pos.x,
                f.pos.y,
                f.facing,
                f.status.anim_frame,
                f.physics.vel_ground.x,
                f.physics.vel_air.x,
                f.physics.vel_air.y,
                f.hitlag,
                f.status.status.id(),
                ssb_game::rng::seed(),
            );
            unsafe {
                psp::sys::sceIoWrite(
                    psp::sys::sceKernelStdout(),
                    line.as_ptr() as *const core::ffi::c_void,
                    line.len(),
                );
            }
        }
        return;
    }
    if !matches!(
        capture_scene,
        Some(GameScene::VsArwing | GameScene::VsCar | GameScene::VsBall | GameScene::VsRays)
    ) {
        return;
    }
    for f in [player.map(|p| &p.fighter), dummy.map(|d| &d.fighter)].into_iter().flatten() {
        let line = alloc::format!(
            "re425 tick={} port={} status={:?} fx={:?} rays={:?} lr={:?} link1={} z={:.0}\n",
            sim_frame_index,
            f.port,
            f.status.status,
            f.entry.effect_ticks,
            f.entry.rays_ticks,
            f.entry.lr,
            f.entry.is_link_1,
            f.pos.z,
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
    if matches!(capture_scene, Some(GameScene::YoshiRollF | GameScene::YoshiRollB | GameScene::YoshiShieldBreak)) {
        let f = &player.fighter;
        let line = alloc::format!(
            "re427 tick={} anim_frame={} roll_egg={} joint5={:?} rotate_x={} positive={} health={}\n",
            sim_frame_index,
            f.status.anim_frame,
            f.yoshi.egg_escape_active,
            f.joint_world(5, ssb_engine::math::Vec3::ZERO),
            // The current roll animation drives joint 5 at slot 2.
            player.skeleton.pose(2).map_or(0.0, |p| p.rotate[0]),
            f.joint_transforms[5].is_some_and(|j| j.axes[0].z > 0.0),
            f.guard.shield_health,
        );
        unsafe {
            psp::sys::sceIoWrite(psp::sys::sceKernelStdout(), line.as_ptr() as *const core::ffi::c_void, line.len());
        }
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
    if matches!(
        capture_scene,
        Some(GameScene::PikachuThunder | GameScene::KirbyHat | GameScene::YoshiEgg)
    ) {
        let line = alloc::format!(
            "re417 player=({:.0},{:.0}) status={:?} copy={:?} hat={:?} dummy=({:.0},{:.0}) egg={:?} heads={:?} trails={:?}\n",
            player.fighter.pos.x,
            player.fighter.pos.y,
            player.fighter.status.status,
            player.fighter.kirby.copy_id,
            ssb_game::kirby_copy::copy_hat(&player.fighter),
            dummy.fighter.pos.x,
            dummy.fighter.pos.y,
            ssb_game::capture_yoshi::egg_effect(&dummy.fighter).map(|e| (e.index, e.plays)),
            weapons.thunder_heads().map(|h| h.position.y as i32).collect::<alloc::vec::Vec<_>>(),
            weapons
                .thunder_trails()
                .map(|t| (t.position.y as i32, t.texture))
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
    if matches!(capture_scene, Some(GameScene::YoshiShield | GameScene::VsShield)) {
        let line = alloc::format!(
            "re418 tick={} status={:?} is_shield={} damage_shield={} row={} health={} egg={} env={:?}\n",
            sim_frame_index,
            player.fighter.status.status,
            player.fighter.guard.is_shield,
            player.fighter.guard.is_damage_shield,
            ssb_game::combat::shield_color_row(&player.fighter),
            player.fighter.guard.shield_health,
            ssb_game::combat::is_yoshi_egg_shield(&player.fighter),
            ssb_game::combat::yoshi_shield_env(&player.fighter),
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

/// The CPU fighters beside the player's: `dummies[i]` is port `i + 1`.
/// Boxed, since each [`play::Dummy`] is large and [`Session`] lives on
/// `run`'s stack.
type Dummies = [Option<alloc::boxed::Box<play::Dummy>>; 3];

/// Every fighter's scene in port order (`gGCCommonLinks[nGCCommonLinkIDFighter]`,
/// where `scVSBattleStartBattle` makes the fighters by player): the player's
/// on port 0, then the CPUs'.
fn scenes<'a>(pl: &'a mut play::FighterScene, dummies: &'a mut Dummies) -> [Option<&'a mut play::FighterScene>; 4] {
    let [a, b, c] = dummies.each_mut();
    let scene = |d: &'a mut Option<alloc::boxed::Box<play::Dummy>>| d.as_deref_mut().map(|d| &mut **d);
    [Some(pl), scene(a), scene(b), scene(c)]
}

/// [`scenes`] for reading.
fn scenes_ref<'a>(pl: &'a play::FighterScene, dummies: &'a Dummies) -> [Option<&'a play::FighterScene>; 4] {
    let scene = |i: usize| dummies[i].as_deref().map(|d| &**d);
    [Some(pl), scene(0), scene(1), scene(2)]
}

/// Fighters `a` and `b` of `s` at once, in that order.
fn pair<'b>(
    s: &'b mut [Option<&mut play::FighterScene>; 4],
    a: usize,
    b: usize,
) -> Option<(&'b mut ssb_game::fighter::Fighter, &'b mut ssb_game::fighter::Fighter)> {
    if a == b || a.max(b) >= s.len() {
        return None;
    }
    let (lo, hi) = s.split_at_mut(a.max(b));
    let low = &mut lo[a.min(b)].as_deref_mut()?.fighter;
    let high = &mut hi[0].as_deref_mut()?.fighter;
    Some(if a < b { (low, high) } else { (high, low) })
}

/// The index in `s` of the fighter on `port`.
fn index_of(s: &[Option<&mut play::FighterScene>; 4], port: u8) -> Option<usize> {
    s.iter().position(|x| x.as_deref().is_some_and(|x| x.fighter.port == port))
}

/// [`ssb_game::grab::exchange`] from fighter `from` to its grab partner
/// (`ssb_game::grab::partner`, the fighter the original's direct writes
/// reach through `catch_gobj` or `capture_gobj`), or to the first other
/// fighter in port order when it has never been linked.
fn exchange_from(s: &mut [Option<&mut play::FighterScene>; 4], from: usize) {
    let Some(f) = s[from].as_deref() else { return };
    let to = ssb_game::grab::partner(&f.fighter)
        .and_then(|port| index_of(s, port))
        .filter(|&to| to != from)
        .or_else(|| (0..s.len()).find(|&i| i != from && s[i].is_some()));
    if let Some((a, b)) = to.and_then(|to| pair(s, from, to)) {
        ssb_game::grab::exchange(a, b);
    }
}

/// Every fighter in `s` in port order, for the passes that take them all.
fn fighters_mut<'b>(s: &'b mut [Option<&mut play::FighterScene>; 4]) -> alloc::vec::Vec<&'b mut ssb_game::fighter::Fighter> {
    s.iter_mut().flatten().map(|x| &mut x.fighter).collect()
}

/// One Training simulation frame: the stage controllers, every fighter's
/// interrupt, physics and map passes in port order, and the weapon and item
/// pools. Kept out of [`run`] so `run` stays inside MIPS branch range.
#[inline(never)]
#[allow(clippy::too_many_arguments)]
unsafe fn training_step(
    p: &Pack<'_>,
    stage_index: u32,
    pl: &mut play::FighterScene,
    dummies: &mut Dummies,
    weapons: &mut ssb_game::weapon::WeaponPool,
    items: &mut ssb_game::item::ItemPool,
    material_anim: &mut ssb_rom::skeleton::MaterialAnimator,
    stage_objects: &mut ssb_rom::ground_obj::GroundObjects,
    stage_map: &mut Option<alloc::boxed::Box<ssb_psp_runtime::scene::StageMap>>,
    stage_ctl: &mut ssb_game::stage::Stage,
    controller: ControllerState,
    started: bool,
    locked: bool,
    effects: &mut dyn ssb_game::effect::HitEffectSink,
    bonus: Option<&mut ssb_game::spgame::bonus_stage::BonusStage>,
    mut battle: Option<&mut ssb_game::battle::Battle>,
    lead: &mut play::Lead,
) {
    material_anim.tick(p);
    let Some(stage) = p.stage(stage_index) else {
        return;
    };
    // Priority 5, Ground link: `gcPlayAnimAll` precedes
    // every fighter interrupt and the priority-4 controller.
    let _ = stage_objects.advance(p);
    if let Some(map) = stage_map.as_mut() {
        let _ = map.tick(p);
    }
    let groups = stage_map
        .as_ref()
        .map_or(&[][..], |map| map.groups.as_slice());
    interrupt_pass(
        p,
        &stage,
        groups,
        pl,
        dummies,
        items,
        weapons,
        stage_ctl,
        controller,
        locked,
        effects,
        lead,
    );
    // Bonus2's priority-4 process observes the fighter after interrupts,
    // before priority-3 movement, item hits and death scoring.
    if let (Some(bonus), Some(battle), Some(map)) = (bonus, battle.as_deref_mut(), stage_map.as_mut()) {
        if let Some(group) = pl.fighter.floor.and_then(|s| p.stage_lines(&stage).find(|l| l.id == s.line)).map(|l| l.yakumono) {
            if let Some(i) = bonus.board(&pl.fighter, group, battle) {
                map.platforms[i].board(p);
            }
        }
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
        let mut s = scenes(pl, dummies);
        let mut fighters = fighters_mut(&mut s);
        stage_ctl.tick(
            &mut fighters,
            ssb_game::stage::TickInput {
                groups: groups_mut,
                objects: &mut ssb_psp_runtime::scene::StageObjectsPort {
                    pack: p,
                    objects: stage_objects,
                },
                items: &mut *items,
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
    // The finish process runs at priority 4, before fighter physics and
    // death scoring. A timer callback already dispatched keeps its end.
    if let ssb_game::stage::Controller::Bonus3(race) = &stage_ctl.controller {
        if race.complete {
            if let Some(battle) = battle.as_deref_mut() { battle.announce_complete(); }
        }
    }
    // The vapor and sparkles the stage process made.
    stage_ctl.flush_effects(effects);
    // Sector Z's Arwing lasers: `wpManagerMakeWeapon` inside the
    // controller; a pair's second shot only after the first.
    for laser in stage_ctl.take_lasers() {
        if !weapons.spawn_arwing_laser(laser) {
            break;
        }
    }
    if let Some(a) = stage_objects.arwing.as_ref() {
        weapons.observe_arwing_roll(a.rotate(1)[2]);
    }
    let groups = stage_map
        .as_ref()
        .map_or(&[][..], |map| map.groups.as_slice());
    let mut s = scenes(pl, dummies);
    physics_pass(p, &stage, groups, &mut s, weapons, items,
        stage_ctl,
        stage_objects,
        started,
        effects);
    // Priority 3, after the fighters', weapons' and items': the effects'
    // processes.
    effects.process();
    // A quake's `gmCameraSetVelAt`, which the next camera update applies.
    if let Some((magnitude, ticks)) = effects.take_quake() {
        if let Some(f) = s[0].as_deref_mut() {
            f.camera.quake = quake_translate(p, magnitude, ticks).or(f.camera.quake);
        }
    }
    hit_pass(p, &stage, groups, &mut s, weapons, items, stage_objects, stage_ctl, effects,
    );
    // The stage calls the items made (`grInishiePowerBlockSetDamage` in
    // their hit collisions, `grInishiePowerBlockSetWait` in their main
    // process): no stage process runs between them and here.
    stage_ctl.apply_item_events(
        items.take_stage_events(),
        groups,
        &mut ssb_psp_runtime::scene::StageObjectsPort {
            pack: p,
            objects: stage_objects,
        },
    );
    ssb_psp_runtime::scene::place_item_trees(stage_objects, items);
}

/// A new weapon pool on the heap, built in this frame rather than `run`'s.
#[inline(never)]
fn new_weapon_pool() -> alloc::boxed::Box<ssb_game::weapon::WeaponPool> {
    alloc::boxed::Box::default()
}

/// `efManagerQuakeProcUpdate`'s `DObj` translation `(y, z)` after `ticks`
/// plays of the quake's packed animation (RE-416).
#[inline(never)]
fn quake_translate(p: &Pack<'_>, magnitude: u8, ticks: u16) -> Option<(f32, f32)> {
    let anim = p.effect_anim(ssb_rom::effect::QUAKE_ANIM_SLOT + u32::from(magnitude))?;
    let joint = p.anim_joint(anim.first_joint)?;
    let data = p.anim_script(&anim)?;
    let mut j = ssb_rom::objanim::StageJoint::start_changed(joint.script, 0.0);
    let mut pose = ssb_rom::figatree::JointPose {
        rotate: [0.0; 3],
        translate: [0.0; 3],
        scale: [1.0; 3],
    };
    for _ in 0..ticks {
        j.tick(data, 1.0, &mut pose).ok()?;
    }
    Some((pose.translate[1], pose.translate[2]))
}

/// Makes every fighter's queued effects (`ssb_game::fteffect`), in port
/// order, where the process that asked for them ends.
#[inline(never)]
fn flush_fighter_effects(s: &mut [Option<&mut play::FighterScene>; 4], effects: &mut dyn ssb_game::effect::HitEffectSink) {
    for f in s.iter_mut().flatten() {
        effects.fighter(&mut f.fighter);
    }
}

/// Priority 5: every fighter's `ftMainProcUpdateInterrupt`, in port order,
/// the player's from the pad and each CPU's from `ftComputerProcessAll`
/// against every other fighter as it stands. Each fighter's grab events
/// land before the next one's half, matching the original's direct status
/// writes (`ssb_game::grab` module docs).
#[inline(never)]
#[allow(clippy::too_many_arguments)]
fn interrupt_pass(
    p: &Pack<'_>,
    stage: &ssb_rom::pack::StageDesc,
    groups: &[ssb_game::map::MapGroup],
    pl: &mut play::FighterScene,
    dummies: &mut Dummies,
    items: &mut ssb_game::item::ItemPool,
    weapons: &ssb_game::weapon::WeaponPool,
    stage_ctl: &ssb_game::stage::Stage,
    controller: ControllerState,
    // The VS countdown locks every fighter's control, the CPUs' too.
    locked: bool,
    effects: &mut dyn ssb_game::effect::HitEffectSink,
    lead: &mut play::Lead,
) {
    // Real jump binding (RE-295): any N64 C-button tap is a
    // real `FTCOMMON_KNEEBEND` button-jump input
    // (`ftCommonKneeBendCheckButtonTap`). An upward stick
    // flick is the game's other real jump input and needs no
    // separate wiring here: `Fighter::tick`'s own status
    // machine reads `stick_y` directly.
    let jump_held = controller.buttons.contains(JUMP_BUTTON_MASK);
    // `DeadUpFall` drops from above `gGMCameraGObj`'s eye.
    let eye = pl.camera.eye;
    for f in scenes(pl, dummies).into_iter().flatten() {
        f.fighter.dead.camera_eye = eye;
        f.fighter.interface.control_disable = locked;
    }
    for i in 0..4 {
        // Master Hand reads his target as it stands (`target_gobj`).
        campaign::refresh_boss_targets(&mut scenes(pl, dummies));
        if i == 0 {
            items.publish(&mut pl.fighter);
            let (input, jump) = match lead {
                play::Lead::Pad => (controller, jump_held),
                _ if locked => (ControllerState::default(), false),
                play::Lead::Key(key) => play::key_input(key),
                play::Lead::Computer(computer) => {
                    let all = scenes_ref(pl, dummies);
                    let opponents: alloc::vec::Vec<_> = all
                        .into_iter()
                        .flatten()
                        .filter(|x| ssb_game::computer::behave::is_opponent(&pl.fighter, &x.fighter))
                        .map(|x| ssb_game::computer::behave::opponent(&x.fighter))
                        .collect();
                    let sight = play::CpuSight::observe(pl.fighter.port, items, weapons, stage_ctl);
                    let input = play::computer_input(computer, pl, p, stage, groups, &opponents, &sight, false);
                    (input, false)
                }
            };
            pl.tick_fighter_interrupt(p, stage, input, jump, groups);
        } else {
            // The CPU's fighter walks skip itself and its team.
            let all = scenes_ref(pl, dummies);
            let opponents: alloc::vec::Vec<_> = all[i].map_or_else(alloc::vec::Vec::new, |me| {
                all.into_iter()
                    .flatten()
                    .filter(|x| ssb_game::computer::behave::is_opponent(&me.fighter, &x.fighter))
                    .map(|x| ssb_game::computer::behave::opponent(&x.fighter))
                    .collect()
            });
            let Some(d) = dummies[i - 1].as_deref_mut() else {
                continue;
            };
            items.publish(&mut d.fighter);
            // The items as the fighters before it left them this pass.
            let sight = play::CpuSight::observe(d.fighter.port, items, weapons, stage_ctl);
            d.tick_interrupt(p, stage, groups, &opponents, &sight, locked);
        }
        let mut s = scenes(pl, dummies);
        after_interrupt(&mut s, i);
        jostle_from(&mut s, i);
        exchange_from(&mut s, i);
        flush_fighter_effects(&mut s, effects);
    }
}

/// Priority 4, Fighter link: every fighter's `ftMainProcPhysicsMap` in port
/// order, then the weapon and item pools.
#[inline(never)]
#[allow(clippy::too_many_arguments)]
fn physics_pass(
    p: &Pack<'_>,
    stage: &ssb_rom::pack::StageDesc,
    groups: &[ssb_game::map::MapGroup],
    s: &mut [Option<&mut play::FighterScene>; 4],
    weapons: &mut ssb_game::weapon::WeaponPool,
    items: &mut ssb_game::item::ItemPool,
    stage_ctl: &mut ssb_game::stage::Stage,
    stage_objects: &mut ssb_rom::ground_obj::GroundObjects,
    started: bool,
    effects: &mut dyn ssb_game::effect::HitEffectSink,
) {
    let map = || ssb_psp_runtime::scene::MapSegments::with_groups(p, stage, groups);
    for i in 0..s.len() {
        let mut held = [None; 3];
        let others = s.iter().enumerate().filter(|&(j, _)| j != i).filter_map(|(_, x)| x.as_deref());
        for (slot, o) in held.iter_mut().zip(others) {
            *slot = ssb_game::map::is_cliff_hold(o.fighter.status.status).then_some((o.fighter.cliff.line, o.fighter.facing));
        }
        campaign::refresh_boss_targets(s);
        let Some(f) = s[i].as_deref_mut() else {
            continue;
        };
        f.fighter.occupied_cliffs = held;
        f.tick_fighter_physics(p, stage, groups);
        if core::mem::take(&mut f.fighter.dead.died) {
            weapons.destroy_boomerang(f.fighter.port);
        }
        if i == 0 {
            // The Boomerang projects through the camera last drawn.
            weapons.observe_camera(&f.camera);
        }
        items.take_requests(&mut f.fighter, map);
        let spawn = f.fighter.take_weapon_spawn();
        exchange_from(s, i);
        // The motion scripts' effects, made at the end of this fighter's
        // `ftMainProcPhysicsMap`.
        flush_fighter_effects(s, effects);
        if let Some(spawn) = spawn {
            weapons.spawn(spawn);
            // A weapon's making effect (the Blaster's glow), in the
            // fighter's process.
            weapons.flush_effects(effects);
        }
    }
    // Master Hand's camera requests reach the camera before its process.
    campaign::take_boss_camera(s);
    tick_battle_camera(stage, s);
    for f in s.iter().flatten() {
        weapons.observe_owner(&f.fighter);
    }
    let blast_zone = ssb_game::status::BlastZone {
        top: stage.bounds.top as f32,
        bottom: stage.bounds.bottom as f32,
        left: stage.bounds.left as f32,
        right: stage.bounds.right as f32,
    };
    for f in s.iter().flatten() {
        items.observe_owner(&f.fighter);
    }
    // `itPakkunCommonCheckNoFighter` walks the fighter link's `TopN`s.
    let mut tops = [ssb_engine::math::Vec3::ZERO; 4];
    let mut top_count = 0;
    for f in s.iter().flatten() {
        tops[top_count] = ssb_game::item::pakkun::fighter_top(&f.fighter);
        top_count += 1;
    }
    // Priority 3, Item actor link (2), before the item link: the
    // appearance actor. The Star's maker reads the camera's look-at.
    if let Some(pl) = s[0].as_deref() {
        items.camera_at_x = pl.camera.at.x;
    }
    // The weapon link's last pass: free structs for the items' weapon
    // makers, and Onix's rock events.
    items.observe_weapons(weapons);
    items.tick_appear(started, map);
    items.tick_with_effects(map, Some(blast_zone),
        &tops[..top_count],
        &mut ssb_psp_runtime::scene::ItemAnimsPort {
            pack: p,
            objects: stage_objects,
        },
        effects,
    );
    // Monster completion calls the gate from the item main process,
    // before the weapon link and its particle RNG draws.
    stage_ctl.apply_item_events(
        items.take_stage_events(),
        groups,
        &mut ssb_psp_runtime::scene::StageObjectsPort {
            pack: p,
            objects: stage_objects,
        },
    );
    for f in s.iter_mut().flatten() {
        items.sync_owner(&mut f.fighter);
    }
    items.flush_effects(effects);
    items.flush_monster_shots(weapons);
    weapons.tick(map, Some(blast_zone));
    for f in s.iter_mut().flatten() {
        weapons.sync_owner(&mut f.fighter);
    }
    // The weapons' main processes' effects: the weapon link runs after the
    // item link (RE-416).
    weapons.flush_effects(effects);
}

/// The battle camera's process (priority 3, after every fighter's
/// physics): `gmCameraUpdateInterests` over the fighters in link order,
/// through the player's camera.
fn tick_battle_camera(stage: &ssb_rom::pack::StageDesc, s: &mut [Option<&mut play::FighterScene>; 4]) {
    let mut others = [ssb_game::camera::Interest::default(); 3];
    let mut count = 0;
    for f in s[1..].iter().flatten() {
        if let Some(interest) = f.camera_interest(stage) {
            others[count] = interest;
            count += 1;
        }
    }
    // The auto demo's focus follows its fighter as this tick left it.
    let focus = s[0].as_deref().and_then(|pl| pl.demo_zoom).and_then(|z| {
        let f = s.iter().flatten().find(|f| f.fighter.port == z.port)?;
        Some((f.fighter.pos, f.cam_offset_y))
    });
    if let Some(pl) = s[0].as_deref_mut() {
        if let (Some(z), Some((pos, cam_offset_y))) = (pl.demo_zoom.as_mut(), focus) {
            z.pos = pos;
            z.cam_offset_y = cam_offset_y;
        }
        pl.tick_camera(stage, &others[..count]);
    }
}

/// `ftMainProcSearchCatch`, then `ftMainProcSearchHitAll` (fighters, then
/// weapons), then `ftMainProcParams` for every fighter -- the original's
/// process priorities, each over the fighters in port order.
#[inline(never)]
#[allow(clippy::too_many_arguments)]
fn hit_pass(
    p: &Pack<'_>,
    stage: &ssb_rom::pack::StageDesc,
    groups: &[ssb_game::map::MapGroup],
    s: &mut [Option<&mut play::FighterScene>; 4],
    weapons: &mut ssb_game::weapon::WeaponPool,
    items: &mut ssb_game::item::ItemPool,
    stage_objects: &mut ssb_rom::ground_obj::GroundObjects,
    stage_ctl: &mut ssb_game::stage::Stage,
    effects: &mut dyn ssb_game::effect::HitEffectSink,
) {
    let map = || ssb_psp_runtime::scene::MapSegments::with_groups(p, stage, groups);
    // The battle's team rule, which `enter_training` gave the pools.
    let rules = weapons.team_rules;
    for i in 0..s.len() {
        // `ftMainProcSearchCatch` opens with the obstacle search
        // (`ftMainSearchHitHazard`), which reads the others' statuses.
        let mut statuses = [ssb_game::status::AnyStatus::Common(ssb_game::status::Status::Wait); 3];
        let mut count = 0;
        for o in s.iter().enumerate().filter(|&(j, _)| j != i).filter_map(|(_, x)| x.as_deref()) {
            statuses[count] = o.fighter.status.status;
            count += 1;
        }
        let Some(f) = s[i].as_deref_mut() else {
            continue;
        };
        ssb_game::hazard::search_hit_hazard(
            &mut f.fighter,
            stage_ctl,
            &mut ssb_psp_runtime::scene::StageObjectsPort {
                pack: p,
                objects: stage_objects,
            },
            &statuses[..count],
        );
        // `ftMainSearchFighterCatch` over the fighter link, then the
        // catch of the nearest.
        let others = s.iter().enumerate().filter(|&(j, _)| j != i).filter_map(|(_, x)| x.as_deref());
        let caught = s[i]
            .as_deref()
            .and_then(|f| {
                ssb_game::grab::nearest_catch(&f.fighter, others.map(|o| &o.fighter), rules)
            })
            .and_then(|port| index_of(s, port));
        if let Some((catcher, other)) = caught.and_then(|j| pair(s, i, j)) {
            ssb_game::grab::search_catch(catcher, other, rules);
        }
    }
    for i in 0..s.len() {
        exchange_from(s, i);
    }
    ssb_game::combat::search_all(&mut fighters_mut(s), rules);
    for f in s.iter_mut().flatten() {
        items.search_fighter(&mut f.fighter);
    }
    // `wpProcessProcSearchHitWeapon` reads the weapons as they stand before
    // any hit reacts (RE-416).
    weapons.search_weapons();
    for f in s.iter_mut().flatten() {
        weapons.apply_hits(&mut f.fighter);
    }
    for i in 0..s.len() {
        for j in 0..s.len() {
            if let Some((attacker, defender)) = pair(s, i, j) {
                ssb_game::link::apply_spin_attack_hits(attacker, defender, rules);
            }
        }
    }
    items.search_hurt(&mut fighters_mut(s), weapons);
    // The clashes' `proc_setoff`s, after every `proc_hit`.
    weapons.finish_clashes();
    // `ftMainSearchGroundHit`, last of `ftMainProcSearchHitAll`.
    for f in s.iter_mut().flatten() {
        ssb_game::hazard::search_ground_hit(&mut f.fighter, stage_ctl);
    }
    // The hit sparks, made in each fighter's `ftMainProcSearchHitAll`, then
    // the clash search's set-offs (weapon link, priority 1).
    ssb_game::combat::finish_frame_between(&mut fighters_mut(s), effects, &mut |fx| {
        weapons.flush_clash_effects(fx)
    });
    // The weapons' hit collisions (priority 0, after every
    // `ftMainProcParams`).
    weapons.flush_effects(effects);
    for f in s.iter_mut().flatten() {
        f.fighter.resolve_cliff_release(&map);
    }
    for f in s.iter_mut().flatten() {
        items.take_requests(&mut f.fighter, map);
    }
    let all: alloc::vec::Vec<&ssb_game::fighter::Fighter> = s.iter().flatten().map(|x| &x.fighter).collect();
    items.resolve_with_effects(&all,
        &mut ssb_psp_runtime::scene::ItemAnimsPort {
            pack: p,
            objects: stage_objects,
        },
        map,
        effects,
    );
    items.flush_effects(effects);
    for f in s.iter_mut().flatten() {
        items.sync_owner(&mut f.fighter);
    }
    items.take_weapon_spawns(weapons, map);
    for f in s.iter_mut().flatten() {
        items.record_landed(&mut f.fighter);
    }
    for f in s.iter_mut().flatten() {
        weapons.record_landed(&mut f.fighter);
    }
    for i in 0..s.len() {
        exchange_from(s, i);
    }
    flush_fighter_effects(s, effects);
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
        Some(GameScene::Yoshi | GameScene::YoshiBomb | GameScene::YoshiEgg | GameScene::YoshiShield | GameScene::YoshiRollF | GameScene::YoshiRollB | GameScene::YoshiShieldBreak) => {
            ssb_game::fighter::FighterKind::Yoshi
        }
        Some(GameScene::Captain | GameScene::CaptainKick) => {
            ssb_game::fighter::FighterKind::Captain
        }
        Some(GameScene::Kirby | GameScene::KirbyHat) => ssb_game::fighter::FighterKind::Kirby,
        Some(GameScene::Pikachu | GameScene::PikachuAir | GameScene::PikachuThunder) => {
            ssb_game::fighter::FighterKind::Pikachu
        }
        Some(GameScene::Purin | GameScene::PurinBow) => ssb_game::fighter::FighterKind::Purin,
        Some(GameScene::PikachuHat) => ssb_game::fighter::FighterKind::Pikachu,
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

/// Which screen is active.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Screen {
    /// A capture build's scripted route in (RE-462): the two confirms its
    /// scenes' input scripts were timed against (tick 4 past this, tick 8
    /// on [`Screen::CaptureMenu`]'s entry), drawing nothing. A production
    /// build boots the title ([`Screen::Menus`]).
    CaptureIntro,
    /// The capture route's entries: Training, VS, the 1P Game's select,
    /// Option and Data.
    CaptureMenu,
    /// The Training character select (`mnPlayers1PTraining`,
    /// `ssb_game::fighter_select`).
    FighterSelect,
    /// The Training stage select (`mnMaps`, `ssb_game::stage_select`).
    StageSelect,
    /// A VS battle's results: the winner's slot lit (RE-389).
    Results,
    /// The unlock message the VS results queued (`mnMessage`), then the
    /// VS character select.
    Message,
    /// The VS character select (`mnPlayersVS`, `ssb_game::players_vs`,
    /// RE-404).
    PlayersVs,
    /// The 1P Game character select (`mnPlayers1PGame`,
    /// `ssb_game::players_1p`).
    Players1P,
    /// The Bonus Practice select (`mnPlayers1PBonus`,
    /// `ssb_game::players_1p_bonus`, RE-462).
    Players1PBonus,
    /// The 1P Game's own scenes between battles: intro, continue and stage
    /// clear (`campaign`, RE-450). Its battles run on [`Screen::Training`].
    Campaign,
    /// The options and data menus (`ssb_game::menu`, RE-461).
    Menus,
    /// The N64 logo and the opening movie's scenes that run without a
    /// battle (`opening_screen`, RE-467).
    Opening,
    /// Training Mode: a real stage and a real, physics-ticked fighter now
    /// draw here (`draw_training`) -- no combat yet, see
    /// `plans/gameplay/F1.md` acceptance criteria 5-7 for what still has to
    /// land.
    Training,
}

/// The capture route's entries: Training, VS, the 1P Game's select, Option
/// and Data.
const MENU_ENTRIES: usize = 5;
const TRAINING_ENTRY: usize = 0;
/// A VS battle against the CPU pick, which stands still: CPU AI is not
/// ported (RE-389).
const VS_ENTRY: usize = 1;
/// The 1P Game's character select. Its campaign scenes are not wired yet,
/// so START comes back to this menu with the choice saved.
const ONE_P_ENTRY: usize = 2;
/// `mnOption` (RE-461).
const OPTION_ENTRY: usize = 3;
/// `mnData` (RE-461).
const DATA_ENTRY: usize = 4;

const BG_MENU: Color = Color::rgba(16, 16, 24, 255);
/// `mnVSResultsFuncStart`'s default camera fill,
/// `GPACK_RGBA8888(0x00, 0x00, 0x00, 0xFF)`.
const BG_RESULTS: Color = Color::rgba(0, 0, 0, 255);
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

/// The stage a scene that skips the select loads: [`CAPTURE_STAGE_GKIND`],
/// the stage whose wallpaper kind `vssector` and `vsyoshi` show, or the
/// stage a `training<stage>` scene shows (RE-422).
fn capture_stage_gkind(scene: Option<GameScene>) -> u8 {
    match scene {
        Some(GameScene::VsSector) => ssb_game::stage_select::gkind::SECTOR,
        Some(GameScene::VsYoshi) => ssb_game::stage_select::gkind::YOSTER,
        Some(GameScene::TrainingJungle) => ssb_game::stage_select::gkind::JUNGLE,
        Some(GameScene::TrainingZebes) => ssb_game::stage_select::gkind::ZEBES,
        Some(
            GameScene::TrainingSaffron | GameScene::TrainingChansey
            | GameScene::TrainingElectrode
            | GameScene::TrainingCharmander
            | GameScene::TrainingVenusaur
            | GameScene::TrainingPorygon,
        ) => ssb_game::stage_select::gkind::YAMABUKI,
        Some(GameScene::TrainingInishie | GameScene::TrainingPlants) => {
            ssb_game::stage_select::gkind::INISHIE
        }
        Some(GameScene::TrainingYoster) => ssb_game::stage_select::gkind::YOSTER,
        Some(GameScene::TrainingSector | GameScene::TrainingArwing) => {
            ssb_game::stage_select::gkind::SECTOR
        }
        Some(GameScene::TrainingCastle | GameScene::TrainingBumper) => {
            ssb_game::stage_select::gkind::CASTLE
        }
        Some(GameScene::TrainingHyrule) => ssb_game::stage_select::gkind::HYRULE,
        _ => CAPTURE_STAGE_GKIND,
    }
}

fn capture_monster(scene: Option<GameScene>) -> Option<u8> {
    Some(match scene {
        Some(GameScene::TrainingChansey) => 0,
        Some(GameScene::TrainingElectrode) => 1,
        Some(GameScene::TrainingCharmander) => 2,
        Some(GameScene::TrainingVenusaur) => 3,
        Some(GameScene::TrainingPorygon) => 4,
        _ => return None,
    })
}

#[inline(never)]
fn prepare_monster_capture(scene: Option<GameScene>, pack: Option<&Pack<'_>>, s: &mut Session) {
    use ssb_game::stage::StageItems;
    if let Some(kinds) = match scene {
        Some(GameScene::TrainingPokemonA) => Some([
            (ssb_game::item::mmonster::Kind::Nyars, -900.0),
            (ssb_game::item::mmonster::Kind::MLucky, -400.0),
            (ssb_game::item::mmonster::Kind::Kamex, 600.0),
        ]),
        Some(GameScene::TrainingPokemonB) => Some([
            (ssb_game::item::mmonster::Kind::Lizardon, -900.0),
            (ssb_game::item::mmonster::Kind::Starmie, -400.0),
            (ssb_game::item::mmonster::Kind::Dogas, 600.0),
        ]),
        Some(GameScene::TrainingPokemonC) => Some([
            (ssb_game::item::mmonster::Kind::Iwark, -900.0),
            (ssb_game::item::mmonster::Kind::Kabigon, -400.0),
            (ssb_game::item::mmonster::Kind::Spear, 600.0),
        ]),
        Some(GameScene::TrainingPokemonD) => Some([
            (ssb_game::item::mmonster::Kind::Tosakinto, -900.0),
            (ssb_game::item::mmonster::Kind::Sawamura, -400.0),
            (ssb_game::item::mmonster::Kind::Pippi, 600.0),
        ]),
        _ => None,
    } {
        if let Some(pl) = s.play_state.as_ref() {
            // Each from a ball resting on the floor (its map box's top
            // above it) that Mario (port 0) threw.
            // Blastoise's maker reads every fighter.
            s.items.observe_owner(&pl.fighter);
            for d in s.dummies.iter().flatten() {
                s.items.observe_owner(&d.fighter);
            }
            let at = pl.fighter.pos;
            let ball = ssb_game::item::mball::ATTRIBUTES.map_coll.top;
            for (kind, x) in kinds {
                let pos = at + ssb_engine::math::Vec3::new(x, ball, 0.0);
                s.items.spawn_mmonster(
                    kind,
                    pos,
                    Some(pl.fighter.port),
                    pl.fighter.team,
                    &core::iter::empty,
                );
            }
        }
        return;
    }
    if matches!(
        scene,
        Some(
            GameScene::TrainingEquipment
                | GameScene::TrainingRayGun
                | GameScene::TrainingFlower
                | GameScene::TrainingStarRod
                | GameScene::TrainingHammer
        )
    ) {
        if let Some(pl) = s.play_state.as_ref() {
            let at = pl.fighter.pos;
            s.items.camera_at_x = pl.camera.at.x;
            let selected = match scene {
                Some(GameScene::TrainingRayGun) => Some(11),
                Some(GameScene::TrainingFlower) => Some(12),
                Some(GameScene::TrainingStarRod) => Some(10),
                Some(GameScene::TrainingHammer) => Some(13),
                _ => None,
            };
            for index in 7..=13 {
                if selected.is_some_and(|k| k != index) {
                    continue;
                }
                let x = if selected.is_some() {
                    75.0
                } else {
                    (f32::from(index) - 10.0) * 350.0
                };
                let pos = at + ssb_engine::math::Vec3::new(x, 500.0, 0.0);
                s.items.make_setup_common(
                    index,
                    None,
                    pos,
                    ssb_engine::math::Vec3::ZERO,
                    &core::iter::empty,
                );
            }
        }
        return;
    }
    if scene == Some(GameScene::TrainingCpuItem) {
        // A Bat dropped beside the CPU, on the side away from the player.
        if let (Some(pl), Some(d)) = (s.play_state.as_ref(), s.dummies[0].as_deref()) {
            let side = if d.fighter.pos.x < pl.fighter.pos.x {
                -1.0
            } else {
                1.0
            };
            let pos = d.fighter.pos + ssb_engine::math::Vec3::new(side * 400.0, 300.0, 0.0);
            s.items.camera_at_x = pl.camera.at.x;
            s.items.make_setup_common(
                8,
                None,
                pos,
                ssb_engine::math::Vec3::ZERO,
                &core::iter::empty,
            );
        }
        return;
    }
    if scene == Some(GameScene::TrainingThrowable) {
        if let Some(pl) = s.play_state.as_ref() {
            // Motion-Sensor Bomb, Bob-omb, Bumper and the two Shells across
            // the stage; the Poké Ball in reach.
            let at = pl.fighter.pos;
            s.items.camera_at_x = pl.camera.at.x;
            for (index, offset) in [
                (14, (-1100.0, 500.0)),
                (15, (-750.0, 500.0)),
                (16, (-400.0, 500.0)),
                (17, (450.0, 500.0)),
                (18, (800.0, 500.0)),
                (19, (75.0, 300.0)),
            ] {
                let pos = at + ssb_engine::math::Vec3::new(offset.0, offset.1, 0.0);
                s.items.make_setup_common(
                    index,
                    None,
                    pos,
                    ssb_engine::math::Vec3::ZERO,
                    &core::iter::empty,
                );
            }
        }
        return;
    }
    if matches!(
        scene,
        Some(GameScene::TrainingSlide | GameScene::TrainingBumperThrow)
    ) {
        if let Some(pl) = s.play_state.as_ref() {
            // The thrown item in reach; the slide adds a Bob-omb that walks.
            let at = pl.fighter.pos;
            s.items.camera_at_x = pl.camera.at.x;
            let items: &[(u8, (f32, f32))] = if scene == Some(GameScene::TrainingSlide) {
                &[(17, (75.0, 300.0)), (15, (750.0, 500.0))]
            } else {
                &[(16, (75.0, 300.0))]
            };
            for &(index, offset) in items {
                let pos = at + ssb_engine::math::Vec3::new(offset.0, offset.1, 0.0);
                s.items.make_setup_common(
                    index,
                    None,
                    pos,
                    ssb_engine::math::Vec3::ZERO,
                    &core::iter::empty,
                );
            }
        }
        return;
    }
    if scene == Some(GameScene::TrainingUtility) {
        if let Some(pl) = s.play_state.as_ref() {
            // Tomato in reach, Heart and Star in view; the Star bounces
            // towards the camera's look-at. No parent: no map projection.
            let at = pl.fighter.pos;
            s.items.camera_at_x = pl.camera.at.x;
            for (index, offset) in [
                (4, (75.0, 300.0)),
                (5, (-500.0, 600.0)),
                (6, (500.0, 300.0)),
            ] {
                let pos = at + ssb_engine::math::Vec3::new(offset.0, offset.1, 0.0);
                s.items.make_setup_common(
                    index,
                    None,
                    pos,
                    ssb_engine::math::Vec3::ZERO,
                    &core::iter::empty,
                );
            }
        }
        return;
    }
    if matches!(
        scene,
        Some(
            GameScene::TrainingCapsule
                | GameScene::TrainingCrate
                | GameScene::TrainingBarrel
                | GameScene::TrainingHeavy
        )
    ) {
        if let Some(pl) = s.play_state.as_ref() {
            let kind = match scene {
                Some(GameScene::TrainingBarrel) => ssb_game::item::container::Kind::Barrel,
                Some(GameScene::TrainingCrate | GameScene::TrainingHeavy) => {
                    ssb_game::item::container::Kind::Crate
                }
                _ => ssb_game::item::container::Kind::Capsule,
            };
            let offset = if scene == Some(GameScene::TrainingHeavy) {
                ssb_engine::math::Vec3::new(75.0, 500.0, 0.0)
            } else {
                ssb_engine::math::Vec3::new(500.0, 500.0, 0.0)
            };
            s.items
                .spawn_container(kind, pl.fighter.pos + offset, ssb_engine::math::Vec3::ZERO);
        }
        return;
    }
    let (Some(id), Some(pack)) = (capture_monster(scene), pack) else {
        return;
    };
    let ssb_game::stage::Controller::Yamabuki(gate) = &mut s.stage_ctl.controller else {
        return;
    };
    gate.status = ssb_game::stage::yamabuki::GateStatus::Open;
    gate.monster_prev = id;
    gate.monster = s
        .items
        .make_item(ssb_game::stage::StageItem::Monster(id), gate.monster_pos);
    // Force the source's WAIT firing pattern, to show the weapon and the
    // active texture frame at this diagnostic scene's capture tick.
    for slot in 0..ssb_game::item::ITEM_ALLOC_MAX as u8 {
        if let Some(item) = s.items.get_mut(slot) {
            item.vars.monster_flags = 1;
        }
    }
    let _ = s.stage_objects.play(pack, ssb_rom::ground_obj::GATE_OPEN);
}

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
        GameScene::StageSelect | GameScene::StageSelectView | GameScene::StageSelectYoshi => {
            CaptureRoute::StageSelect
        }
        GameScene::OnePGame
        | GameScene::OnePIntro | GameScene::OnePBonus | GameScene::OnePTargetClear | GameScene::OnePTargetFall | GameScene::OnePPlatforms | GameScene::OnePPlatformClear | GameScene::OnePPlatformFall | GameScene::OnePRace | GameScene::OnePRaceClear | GameScene::OnePRaceFall | GameScene::OnePRaceHazards | GameScene::OnePBoss | GameScene::OnePBossDefeat | GameScene::OnePMetal | GameScene::OnePGiant | GameScene::OnePZako | GameScene::OnePContinue | GameScene::OnePRetry | GameScene::OnePClear | GameScene::OnePEnding | GameScene::OnePStaffroll | GameScene::OnePCongra | GameScene::OnePChallenger | GameScene::OnePMessage | GameScene::OnePFinale
        | GameScene::FighterSelect
        | GameScene::VsModeMenu
        | GameScene::VsPlayers
        | GameScene::TrainingSelect
        | GameScene::TrainingSelectPicked => CaptureRoute::Selects,
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
    // RE-425: both accessory wearers, in the costumes the scenes name.
    match scene {
        GameScene::PikachuHat => {
            return ssb_game::fighter_select::SceneData {
                man_kind: Some(FighterKind::Pikachu),
                man_costume: 1,
                com_kind: Some(FighterKind::Purin),
                com_costume: 2,
            }
        }
        GameScene::PurinBow => {
            return ssb_game::fighter_select::SceneData {
                man_kind: Some(FighterKind::Purin),
                man_costume: 3,
                com_kind: Some(FighterKind::Pikachu),
                com_costume: 3,
            }
        }
        _ => {}
    }
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
        GameScene::TrainingCpuItem => Some(ssb_game::computer::Behavior::Default),
        // A time-up tie needs a CPU that never lands a hit.
        // A time-up tie needs a CPU that never lands a hit; `vsresults` a
        // winner that lets the player fall.
        GameScene::VsTimeUp
        | GameScene::VsTimeUpSign
        | GameScene::VsSuddenDeath
        | GameScene::VsResults
        | GameScene::VsResultsEmblem => {
            Some(ssb_game::computer::Behavior::Stand)
        }
        _ => None,
    }
}

/// `osGetTime() & 0xFF`: the clock's low byte.
fn clock_byte() -> u8 {
    unsafe { psp::sys::sceKernelGetSystemTimeLow() as u8 }
}

/// `syUtilsRandTimeUCharRange(9)`: the clock's low byte, scaled to 0..9.
fn stage_select_rand() -> u8 {
    (u32::from(clock_byte()) * 9 / 256) as u8
}

/// `ftMainProcUpdateInterrupt`'s jostle for fighter `i`, outside hitlag,
/// against the others as their interrupts have left them (RE-466).
fn jostle_from(s: &mut [Option<&mut play::FighterScene>; 4], i: usize) {
    let Some(me) = s[i].as_deref() else { return };
    if me.fighter.is_in_hitlag() {
        return;
    }
    let mut others = [(ssb_game::fighter::JostleBody::of(&me.fighter), false); 3];
    let mut count = 0;
    for (j, o) in s.iter().enumerate() {
        if j == i {
            continue;
        }
        if let Some(o) = o.as_deref() {
            others[count] = (ssb_game::fighter::JostleBody::of(&o.fighter), j > i);
            count += 1;
        }
    }
    if let Some(me) = s[i].as_deref_mut() {
        ssb_game::fighter::jostle(&mut me.fighter, &others[..count]);
    }
}

/// The half of `ftCommonDeadCheckRebirth` fighter `i` cannot do itself:
/// the rebirth takes the lowest halo no other fighter is using.
fn after_interrupt(s: &mut [Option<&mut play::FighterScene>; 4], i: usize) {
    let mut others = [(ssb_game::status::AnyStatus::Common(ssb_game::status::Status::Wait), 0u8); 3];
    let mut count = 0;
    for o in s.iter().enumerate().filter(|&(j, _)| j != i).filter_map(|(_, x)| x.as_deref()) {
        others[count] = (o.fighter.status.status, o.fighter.dead.rebirth.halo_number);
        count += 1;
    }
    let Some(f) = s[i].as_deref_mut().map(|x| &mut x.fighter) else {
        return;
    };
    if f.dead.rebirth_pending {
        let halo = ssb_game::dead::halo_number(others[..count].iter().copied());
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
    team_rules: ssb_game::team::TeamRules,
    /// The item switches and appearance rate (`mnVSItemSwitch`).
    items: ssb_game::item::normal::Switches,
    /// VS Options' damage ratio (percent).
    damage_ratio: u8,
    /// How to Play or the auto demo: `dSCManagerDefaultBattleState`'s
    /// rules with no battle interface ([`ssb_game::battle::Battle::new_demo`]),
    /// each fighter facing away from its side of the stage (`desc.lr`) and
    /// not entering.
    demo: bool,
}

impl VsRules {
    /// `dSCManagerDefaultBattleState`: a three-minute free-for-all time
    /// battle, stocks 2, Team Attack off.
    pub(crate) const DEFAULT: VsRules = VsRules {
        rule: ssb_game::battle::Rule::Time,
        time_limit: 3,
        stocks: 2,
        team_rules: ssb_game::team::TeamRules::FREE_FOR_ALL,
        items: ssb_game::item::normal::Switches {
            appearance: ssb_game::item::normal::Appearance::Middle,
            toggles: !0,
        },
        damage_ratio: ssb_game::stale::DAMAGE_RATIO_DEFAULT,
        demo: false,
    };

    /// The rules `mnPlayersVSSetSceneData` left in the battle state.
    fn of(state: &ssb_game::players_vs::BattleState) -> VsRules {
        VsRules {
            rule: state.rule,
            time_limit: state.time_limit,
            stocks: state.stocks as i8,
            team_rules: ssb_game::team::TeamRules {
                is_team_battle: state.is_team_battle,
                is_team_attack: state.is_team_attack,
            },
            items: state.item_switches(),
            damage_ratio: state.damage_ratio,
            demo: false,
        }
    }
}

/// A capture scene's VS rules: `vstimeup` picks one minute, `vsteam` a
/// team battle.
fn vs_rules(scene: GameScene) -> VsRules {
    match scene {
        GameScene::VsTimeUp | GameScene::VsTimeUpSign | GameScene::VsSuddenDeath => VsRules {
            time_limit: 1,
            ..VsRules::DEFAULT
        },
        GameScene::VsTeam => VsRules {
            team_rules: ssb_game::team::TeamRules::TEAMS,
            ..VsRules::DEFAULT
        },
        // Two lives each (`stock_setting` 1).
        GameScene::VsTeamSteal => VsRules {
            rule: ssb_game::battle::Rule::Stock,
            stocks: 1,
            team_rules: ssb_game::team::TeamRules::TEAMS,
            ..VsRules::DEFAULT
        },
        // One stock (`stock_setting` 0).
        GameScene::VsResults | GameScene::VsResultsEmblem => VsRules {
            rule: ssb_game::battle::Rule::Stock,
            stocks: 0,
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
    dummies: &mut Dummies,
    weapons: &mut ssb_game::weapon::WeaponPool,
    items: &mut ssb_game::item::ItemPool,
    material_anim: &mut ssb_rom::skeleton::MaterialAnimator,
    stage_objects: &mut ssb_rom::ground_obj::GroundObjects,
    stage_map: &mut Option<alloc::boxed::Box<ssb_psp_runtime::scene::StageMap>>,
    stage_ctl: &mut ssb_game::stage::Stage,
    controller: ControllerState,
    pressed: N64Buttons,
    battle: &mut Option<ssb_game::battle::Battle>,
    damage_hud: &mut Hud,
    // Training's menu locks both fighters' control
    // (`ftParamLockPlayerControl`) while the world runs on.
    menu_locked: bool,
    // A 1P Game battle's campaign: it collects the falls and replaces the
    // enemies (RE-450).
    mut campaign: Option<&mut ssb_game::spgame::session::Session>,
    mut bonus: Option<&mut ssb_game::spgame::bonus_stage::BonusStage>,
    // Master Hand's stage (`sc1pgameboss.c`).
    mut boss: Option<&mut campaign::BossScene>,
    // What drives port 0: the pad, or a title demo's script or CPU.
    lead: &mut play::Lead,
) -> bool {
    use ssb_game::battle::{Frame, GameStatus};
    // How to Play and the auto demo make no battle interface: no pause.
    if let Some(b) = battle.as_mut().filter(|b| !b.is_demo) {
        pause_frame(p, stage_index, pl, dummies, damage_hud, b, controller, pressed, bonus.as_deref_mut());
        if bonus.as_ref().is_some_and(|stage| stage.retry_requested) {
            return true;
        }
    }
    let frame = battle.as_mut().map(|b| (b.begin_frame(), b.status));
    if let (Some(bonus), Some((Frame::Run, _))) = (bonus.as_deref_mut(), frame) {
        bonus.fade_ticks = bonus.fade_ticks.saturating_add(1);
        damage_hud.bonus_fade_alpha = bonus.fade_alpha();
    }
    if let (Some(c), Some((_, status))) = (campaign.as_deref_mut(), frame) {
        campaign::update_game(c, status, &pl.fighter);
    }
    if let (Some(boss), Some(b)) = (boss.as_deref_mut(), battle.as_ref()) {
        campaign::boss_frame_start(boss, pl, b);
    }
    // `ifCommonBattlePauseRestoreInterfaceAll`: the camera eases back while
    // the pause menu stays, then the turn is restored and the world runs.
    if let (Some(pause), Some((f, status))) = (damage_hud.pause, frame) {
        if status == GameStatus::Unpause && f == Frame::Frozen {
            ssb_game::pause::ease_back(&mut pl.camera.pause_eye, pause.origin);
            if let Some(stage) = p.stage(stage_index) {
                tick_battle_camera(&stage, &mut scenes(pl, dummies));
            }
        } else if status == GameStatus::Go {
            pl.camera.pause_eye = pause.origin;
            if pause.kind == ssb_game::pause::PauseKind::Default {
                pl.fighter.model_parts.set_detail_all(pause.detail);
            }
            damage_hud.pause = None;
        }
    }
    // `ifCommonAnnounceGoSetStatus`: the entry camera mode ends at "Go".
    if matches!(frame, Some((_, GameStatus::Go))) {
        for f in scenes(pl, dummies).into_iter().flatten() {
            ssb_game::appear::on_go(&mut f.fighter);
        }
    }
    let banks = ssb_psp_runtime::particles::PackBanks::new(p);
    let (started, locked) = match frame {
        None => (true, false),
        Some((Frame::Run, status)) => (status != GameStatus::Wait, status == GameStatus::Wait),
        Some((Frame::Frozen, status)) => {
            // `ifCommonBattleInterfaceProcUpdate`: at the battle's end the
            // particles run on, lists 2 and 3 only.
            if matches!(status, GameStatus::End | GameStatus::BossDefeat | GameStatus::Set) {
                if let Some(b) = banks.as_ref() {
                    damage_hud.particles.skip = !((1 << 2) | (1 << 3));
                    ssb_game::particle::run(&mut damage_hud.particles, b, &mut damage_hud.effects);
                }
            }
            if let (Some(boss), Some(b)) = (boss.as_deref_mut(), battle.as_ref()) {
                campaign::boss_frozen_frame(p, stage_index, boss, pl, dummies, b);
                // The damage display's break animation runs on.
                if let Some(h) = damage_hud.damage.get_mut(usize::from(boss.port)) {
                    h.update(i32::from(h.damage), false);
                }
            }
            return false;
        }
        Some((Frame::Done, _)) => return true,
    };
    // `sc1PGameBossLockPlayerControl`: Master Hand's defeat locks every
    // fighter.
    let locked = locked || menu_locked || battle.as_ref().is_some_and(|b| b.boss_defeat.is_some());
    let mut none = ssb_game::effect::NoEffects;
    let mut rt = banks.as_ref().map(|b| ssb_game::effect::EffectRuntime {
        particles: &mut damage_hud.particles,
        effects: &mut damage_hud.effects,
        banks: b,
    });
    // Link 0's `func_run`s, before every process: the particles.
    if let Some(rt) = rt.as_mut() {
        rt.run();
    }
    let sink: &mut dyn ssb_game::effect::HitEffectSink = match rt.as_mut() {
        Some(rt) => rt,
        None => &mut none,
    };
    training_step(
        p,
        stage_index,
        pl,
        dummies,
        weapons,
        items,
        material_anim,
        stage_objects,
        stage_map,
        stage_ctl,
        if locked { ControllerState::default() } else { controller },
        started,
        locked,
        sink,
        bonus.as_deref_mut(),
        battle.as_mut(),
        lead,
    );
    // The effect and interface processes after the fighters': the KO
    // explosions (with their particles) and the screen flash.
    for f in scenes(pl, dummies).into_iter().flatten() {
        match rt.as_mut() {
            Some(rt) => damage_hud.ko.observe_with(&mut f.fighter, rt),
            None => damage_hud.ko.observe(&mut f.fighter),
        }
    }
    damage_hud.ko.tick();
    for f in scenes(pl, dummies).into_iter().flatten() {
        let fell = if let (Some(bonus), Some(b)) = (bonus.as_deref_mut(), battle.as_mut()) {
            bonus.observe(&mut f.fighter, b, stage_ctl.take_target_breaks())
        } else { match campaign.as_deref_mut() {
            Some(c) => campaign::collect_fall(c, battle, &mut f.fighter),
            None => report_falls(battle.as_mut(), &mut f.fighter),
        }};
        if let Some(hud) = damage_hud.damage.get_mut(usize::from(f.fighter.port)) {
            update_damage_hud(hud, &f.fighter, fell, started);
        }
        if let (true, Some(rt)) = (fell, rt.as_mut()) {
            fall_effects(rt, &damage_hud.damage, battle.as_ref(), &f.fighter);
        }
    }
    if let Some(b) = battle.as_mut() {
        stock_steal_frame(p, &mut scenes(pl, dummies), b, &damage_hud.damage, &mut damage_hud.steals, rt.as_mut());
    }
    if let (Some(bonus), Some(b)) = (bonus, battle.as_ref()) {
        campaign::bonus_frame(p, damage_hud, b, bonus);
        return false;
    }
    if let (Some(boss), Some(b)) = (boss.as_deref_mut(), battle.as_mut()) {
        campaign::boss_after_world(boss, pl, dummies, b, damage_hud);
    }
    match (campaign, p.stage(stage_index)) {
        (Some(c), Some(stage)) => {
            campaign::replace_enemies(p, &stage, dummies, c, battle, damage_hud);
            if let (Some(b), Some(wait)) = (battle.as_ref(), c.wait.as_ref()) {
                campaign::entry_frame(p, &stage, pl, dummies, damage_hud, b, wait, boss.as_deref());
            }
        }
        _ => {
            if let Some(b) = battle.as_ref().filter(|b| !b.is_demo) {
                tick_countdown(p, damage_hud, b);
                entry_frame(p, pl, dummies, damage_hud, b);
            }
        }
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
    dummies: &mut Dummies,
    hud: &Hud,
    b: &ssb_game::battle::Battle,
) {
    pl.entry_zoom = None;
    let Some(focus) = hud.entry_focus else { return };
    let Some(t) = b.clock().checked_sub(1 + ssb_game::battle::ENTRY_WAIT) else {
        return;
    };
    // The focus counts the fighters in link order.
    for (i, f) in scenes(pl, dummies).into_iter().flatten().enumerate() {
        if t == focus.appear_tick(i) {
            ssb_game::appear::appear_set_status(&mut f.fighter);
        }
    }
    let target = |scene: &play::FighterScene| {
        let mut pos = scene.fighter.pos;
        pos.y += scene.cam_offset_y;
        let dist = p.fighter(scene.fighter.kind as u32).map_or(1000.0, |d| d.closeup_camera_zoom);
        (pos, dist)
    };
    let zoom = focus
        .zoom(t)
        .and_then(|i| scenes_ref(pl, dummies).into_iter().flatten().nth(i).map(target));
    pl.entry_zoom = zoom;
}

/// The battle HUD's state beside the world: the damage displays and the
/// countdown or sudden death's "GO!".
struct Hud {
    bonus_tasks: Option<u8>,
    /// Bonus Practice's time passed (`sc1PBonusStageMakeTimer`).
    bonus_timer: Option<ssb_game::spgame::bonus_stage::PracticeTimer>,
    /// Black scene-entry fade alpha for a campaign bonus course.
    bonus_fade_alpha: u8,
    /// The title demos' fade in from black (`lbFadeMakeActor`, display
    /// link 10 of the battle camera, RE-465).
    demo_fade_alpha: u8,
    /// The boss wallpaper's closing fade this frame
    /// (`sc1PGameBossProcDisplayFadeAlpha`/`...FadeColor`): RGBA over
    /// the battle viewport.
    boss_fade: Option<[u8; 4]>,
    /// By port.
    damage: [ssb_game::hud::DamageDisplay; 4],
    countdown: Option<ssb_game::countdown::Countdown>,
    pause: Option<PauseState>,
    /// `ifCommonEntryFocusThread`, from the countdown's frame.
    entry_focus: Option<ssb_game::appear::EntryFocus>,
    /// `players[].color` by port, the stage emblem colour each damage
    /// display takes (`ifCommonPlayerDamageInitInterface`).
    colors: [u8; 4],
    players: ssb_game::player_interface::Interface,
    /// The KO explosions and the screen flash (RE-412).
    ko: ssb_game::ko::KoEffects,
    /// The match's particles (RE-413), on the heap: some 30 KB.
    particles: alloc::boxed::Box<ssb_game::particle::Particles>,
    /// The effect manager's structs (`efManagerInitEffects`).
    effects: ssb_game::effect::Effects,
    /// The 1P Game's `players[].is_single_stockicon`; `None` follows the
    /// VS battle's rule.
    single_stock: Option<[bool; 4]>,
    /// `sIFCommonPlayerStealInterface` by thief: a stolen stock's icon in
    /// flight (RE-464).
    steals: [Option<ssb_game::hud::StockSteal>; 4],
    /// The 1P Game's team stock icons and their stage
    /// (`sc1PGameTeamStockDisplayProcDisplay`).
    team_stocks: Option<(ssb_game::spgame::Stage, alloc::vec::Vec<ssb_game::spgame::setup::StockIcon>)>,
    /// The display effects' players ([`draw_display_effects`]).
    display_scratch: alloc::boxed::Box<DisplayScratch>,
    /// An opening battle (`nSCBattleGameTypeMovie`, RE-467): no interface,
    /// tags or magnifiers.
    movie: bool,
    /// Samus's opening scene draws Zebes' kind-48 billboards flat
    /// (`mvOpeningSamusMakeMotionWindow`).
    movie_flat48: bool,
}

/// The pause menu's choices at the pause (`sIFCommonBattlePause*`).
#[derive(Clone, Copy)]
struct PauseState {
    kind: ssb_game::pause::PauseKind,
    /// Bonus Practice's L: RETRY (`ifCommonBattlePauseMakeSObjsAll`).
    retry: bool,
    /// `sIFCommonBattlePauseCameraEyeXOrigin`/`YOrigin`.
    origin: (f32, f32),
    /// `sIFCommonBattlePausePlayerDetail`: the zoomed player draws at high
    /// detail until the battle resumes (RE-426).
    detail: ssb_game::modelpart::Detail,
    map_origin: ssb_engine::math::Vec3,
    map_target: ssb_engine::math::Vec3,
}

/// `ifCommonBattleGoUpdateInterface`'s START and
/// `ifCommonBattlePauseUpdateInterface`: pause on START during Go, zooming
/// on the player when in bounds; in the menu steer the view, resume on
/// START, reset on A+B+R+Z, and run the zoom camera.
#[inline(never)]
#[allow(clippy::too_many_arguments)]
fn pause_frame(
    p: &Pack<'_>,
    stage_index: u32,
    pl: &mut play::FighterScene,
    dummies: &mut Dummies,
    hud: &mut Hud,
    b: &mut ssb_game::battle::Battle,
    controller: ControllerState,
    pressed: N64Buttons,
    mut bonus: Option<&mut ssb_game::spgame::bonus_stage::BonusStage>,
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
        GameStatus::Go if pressed.contains(N64Buttons::START) && !pl.fighter.dead.is_menu_ignore => {
            let kind = if bonus.is_some() { PauseKind::Bonus } else { pause::kind_for(pl.fighter.pos, bounds) };
            hud.pause = Some(PauseState {
                retry: bonus.as_deref().is_some_and(|b| b.practice),
                kind,
                origin: pl.camera.pause_eye,
                detail: pl.fighter.model_parts.detail_curr,
                map_origin: ssb_engine::math::Vec3::new(stage.zoom_start[0] as f32, stage.zoom_start[1] as f32, stage.zoom_start[2] as f32),
                map_target: ssb_engine::math::Vec3::new(stage.zoom_end[0] as f32, stage.zoom_end[1] as f32, stage.zoom_end[2] as f32),
            });
            if kind == PauseKind::Bonus {
                pl.camera.begin_map_zoom(
                    ssb_engine::math::Vec3::new(stage.zoom_start[0] as f32, stage.zoom_start[1] as f32, stage.zoom_start[2] as f32),
                    ssb_engine::math::Vec3::new(stage.zoom_end[0] as f32, stage.zoom_end[1] as f32, stage.zoom_end[2] as f32),
                );
            }
            if kind == PauseKind::Default {
                pl.fighter
                    .model_parts
                    .set_detail_all(ssb_game::modelpart::Detail::High);
            }
            b.pause();
        }
        GameStatus::Pause => {
            let Some(state) = hud.pause else { return };
            if state.retry && pressed.contains(N64Buttons::L) {
                if let Some(bonus) = bonus.as_deref_mut() {
                    bonus.retry_requested = true;
                }
                return;
            }
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
            if state.kind == PauseKind::Bonus {
                pl.camera.tick_map_zoom(state.map_origin, state.map_target);
            } else if state.kind != PauseKind::PlayerNA {
                // `gmCameraPlayerZoomFuncCamera`: the battle camera while
                // the player is out of bounds.
                if pause::kind_for(pl.fighter.pos, bounds) == PauseKind::PlayerNA {
                    tick_battle_camera(&stage, &mut scenes(pl, dummies));
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
    #[inline(never)]
    fn new() -> Hud {
        Hud {
            bonus_tasks: None,
            bonus_timer: None,
            bonus_fade_alpha: 0,
            demo_fade_alpha: 0,
            boss_fade: None,
            damage: core::array::from_fn(|port| ssb_game::hud::DamageDisplay::new(port, 0)),
            countdown: None,
            pause: None,
            entry_focus: None,
            colors: [0, 1, 2, 3],
            players: ssb_game::player_interface::Interface::default(),
            ko: ssb_game::ko::KoEffects::default(),
            particles: new_particles(),
            effects: ssb_game::effect::Effects::new(0),
            single_stock: None,
            steals: [None; 4],
            team_stocks: None,
            display_scratch: new_display_scratch(),
            movie: false,
            movie_flat48: false,
        }
    }
}

/// `efParticleInitAll`'s pools, built in place on the heap.
#[inline(never)]
fn new_particles() -> alloc::boxed::Box<ssb_game::particle::Particles> {
    let mut b = alloc::boxed::Box::<ssb_game::particle::Particles>::new_uninit();
    // SAFETY: `write_new` initialises every field.
    unsafe {
        ssb_game::particle::Particles::write_new(b.as_mut_ptr());
        b.assume_init()
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
                    // `pl_count + cp_count`.
                    count: b.players.iter().filter(|p| p.present).count() as u8,
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
/// the sudden-death battle. `None`, entering nothing, when the player on
/// port 0 is not among them: the world needs the pad's fighter and its
/// camera.
#[inline(never)]
fn start_sudden_death(
    pack: Option<&Pack<'_>>,
    gkind: u8,
    roster: Roster,
    sudden: ssb_game::battle::Battle,
    battle: &mut Option<ssb_game::battle::Battle>,
    world: &mut TrainingWorld<'_>,
) -> Option<u32> {
    if !sudden.players[0].present {
        return None;
    }
    let rules = VsRules {
        rule: ssb_game::battle::Rule::Stock,
        time_limit: sudden.time_limit,
        stocks: 0,
        team_rules: sudden.team_rules(),
        // `gSCManagerVSBattleState` keeps the battle's items and ratio.
        items: world.items.normal_switches,
        damage_ratio: ssb_game::stale::damage_ratio(),
        demo: false,
    };
    // `gSCManagerVSBattleState`: only the tied players.
    let tied: Roster = core::array::from_fn(|port| roster[port].filter(|_| sudden.players[port].present));
    // A capture scene's CPU behaviour carries over; in play it is the VS
    // default either way.
    let cpu = world
        .dummies
        .each_ref()
        .map(|d| d.as_ref().map(|d| (d.computer.behavior, d.computer.trait_kind)));
    let index = enter_training(pack, gkind, tied, Some(rules), battle, world);
    // `is_skip_entry`: sudden death's fighters stand at once.
    if let Some(pl) = world.play_state.as_mut() {
        pl.fighter.damage = ssb_game::battle::SUDDEN_DEATH_DAMAGE;
        ssb_game::status::set_wait(&mut pl.fighter);
    }
    for (d, cpu) in world.dummies.iter_mut().zip(cpu) {
        let Some(d) = d.as_deref_mut() else { continue };
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
    Some(index)
}

/// The battle half of `ftCommonDeadUpdateScore`. Returns whether the
/// fighter fell.
fn report_falls(battle: Option<&mut ssb_game::battle::Battle>, f: &mut ssb_game::fighter::Fighter) -> bool {
    let fell = core::mem::take(&mut f.dead.scored);
    if let Some(b) = battle {
        b.collect_damage(f);
        if fell {
            b.on_fall(f.port, f.damage_player);
        }
    }
    fell
}

/// The interface half of `ftCommonDeadUpdateScore` (RE-464): the lost
/// stock's snap (`ifCommonPlayerStockMakeStockSnap`), then where the battle
/// shows scores, "-1" over the fallen player and "+1" over the player
/// credited with the fall (`ifCommonPlayerScoreMakeEffect`).
fn fall_effects(
    rt: &mut ssb_game::effect::EffectRuntime<'_>,
    displays: &[ssb_game::hud::DamageDisplay; 4],
    battle: Option<&ssb_game::battle::Battle>,
    f: &ssb_game::fighter::Fighter,
) {
    let Some(own) = displays.get(usize::from(f.port)) else {
        return;
    };
    let (x, y) = ssb_game::hud::stock_snap_position_at(own.pos_x, own.pos_y);
    rt.effects.stock_snap(rt.particles, rt.banks, x, y);
    if !battle.is_some_and(|b| b.is_show_score()) {
        return;
    }
    rt.effects
        .battle_score(rt.particles, rt.banks, ssb_game::hud::score_position_at(own.pos_x, own.pos_y), -1);
    if let Some(scorer) = f.damage_player.and_then(|k| displays.get(usize::from(k))) {
        rt.effects
            .battle_score(rt.particles, rt.banks, ssb_game::hud::score_position_at(scorer.pos_x, scorer.pos_y), 1);
    }
}

/// `ftCommonSleepProcUpdate`'s battle half and the steals' interface
/// (RE-464): a sleeping thief's START steals a teammate's stock
/// (`ifCommonPlayerStockStealMakeInterface` and its start burst); a landed
/// steal gives the thief one life; each icon in flight moves
/// (`ifCommonPlayerStockStealProcUpdate`) and bursts at the thief's stocks
/// when it lands. The fighters' halves run in their own updates
/// (`ssb_game::dead::update_sleep`); the battle's run here, after every
/// fighter's, in the same frame.
fn stock_steal_frame(
    p: &Pack<'_>,
    s: &mut [Option<&mut play::FighterScene>; 4],
    b: &mut ssb_game::battle::Battle,
    displays: &[ssb_game::hud::DamageDisplay; 4],
    steals: &mut [Option<ssb_game::hud::StockSteal>; 4],
    mut rt: Option<&mut ssb_game::effect::EffectRuntime<'_>>,
) {
    use ssb_game::hud::{steal_effect_position_at, StockSteal};
    for thief in 0..4 {
        let Some(f) = s[thief].as_deref_mut().map(|x| &mut x.fighter) else {
            continue;
        };
        if core::mem::take(&mut f.dead.steal_landed) {
            b.land_stolen_stock(f.port);
        }
        if !core::mem::take(&mut f.dead.steal_request) {
            continue;
        }
        let Some(stolen) = b.steal_stock(f.port) else {
            continue;
        };
        ssb_game::dead::start_stock_steal(f);
        let Some(victim) = s[usize::from(stolen)].as_deref_mut().map(|x| &mut x.fighter) else {
            continue;
        };
        victim.stocks -= 1;
        let (kind, costume) = (victim.kind, victim.costume);
        let (Some(from), Some(to)) = (displays.get(usize::from(stolen)), displays.get(thief)) else {
            continue;
        };
        let Some(icon) = p.fighter_sprite(kind as u8, ssb_rom::pack::SpriteDesc::ROLE_STOCK, costume) else {
            continue;
        };
        steals[thief] = Some(StockSteal::make_at(stolen, from.pos_x, to.pos_x, from.pos_y, icon.width, icon.height));
        if let Some(rt) = rt.as_deref_mut() {
            let (x, y) = steal_effect_position_at(from.pos_x, from.pos_y);
            rt.effects.stock_steal_start(rt.particles, rt.banks, x, y);
        }
    }
    for (thief, slot) in steals.iter_mut().enumerate() {
        if slot.as_mut().is_some_and(|steal| !steal.update()) {
            *slot = None;
            if let (Some(rt), Some(to)) = (rt.as_deref_mut(), displays.get(thief)) {
                let (x, y) = steal_effect_position_at(to.pos_x, to.pos_y);
                rt.effects.stock_steal_end(rt.particles, rt.banks, x, y);
            }
        }
    }
}

/// `ifCommonPlayerDamageInitInterface` for every port, shown at once
/// outside a battle.
fn reset_damage_hud(world: &mut TrainingWorld<'_>) {
    world.damage_hud.bonus_tasks = None;
    world.damage_hud.bonus_timer = None;
    world.damage_hud.bonus_fade_alpha = 0;
    world.damage_hud.demo_fade_alpha = 0;
    world.damage_hud.boss_fade = None;
    world.damage_hud.countdown = None;
    world.damage_hud.ko = ssb_game::ko::KoEffects::default();
    // `efParticleInitAll` and `efManagerInitEffects`: a new battle scene.
    world.damage_hud.particles.reset();
    world.damage_hud.effects = ssb_game::effect::Effects::new(0);
    world.damage_hud.entry_focus = None;
    world.damage_hud.single_stock = None;
    world.damage_hud.steals = [None; 4];
    world.damage_hud.players = ssb_game::player_interface::Interface::default();
    let mut damage = [0; 4];
    if let Some(pl) = world.play_state.as_ref() {
        damage[0] = i32::from(pl.fighter.damage);
    }
    for (d, damage) in world.dummies.iter().zip(&mut damage[1..]) {
        *damage = d.as_ref().map_or(0, |d| i32::from(d.fighter.damage));
    }
    world.damage_hud.damage = core::array::from_fn(|port| ssb_game::hud::DamageDisplay::new(port, damage[port]));
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

/// `mnVSResults`' frame over the black of `mnVSResultsFuncStart`'s default
/// camera: the wallpaper, the fighters (RE-409), then the text and table
/// (RE-410), each player's kind, costume, tag colour and humanity from the
/// battle's `roster`.
#[inline(never)]
unsafe fn draw_results(
    gpu: &mut Gpu,
    pack: Option<&Pack<'_>>,
    draw_state: &mut meshdraw::DrawState,
    results: Option<&ssb_game::results::Results>,
    fighters: Option<&mut results_screen::Fighters>,
    roster: &Roster,
) {
    // `lbTransitionSetupTransition`: the battle's last frame, still on
    // display, before the results' first frame opens (RE-464).
    let mut fighters = fighters;
    if let Some(f) = fighters.as_deref_mut().filter(|f| f.photo_pending) {
        gpu.capture_transition_photo_now();
        f.photo_pending = false;
    }
    gpu.set_viewport_fullscreen();
    gpu.begin_frame(Some(BG_RESULTS));
    let (Some(p), Some(r), Some(f)) = (pack, results, fighters) else {
        return;
    };
    let players = roster.map(|e| {
        e.map(|e| ssb_game::results_layer::Player {
            kind: e.kind,
            costume: e.costume,
            human: e.human,
            color: e.color,
        })
    });
    results_screen::draw_all(gpu, p, draw_state, r, f, &players);
}

/// `mnVSResultsFuncStart`: the rankings (`mnVSResultsInitVars` and
/// `mnVSResultsInitRankings`), then the scene's random pick. Out of [`run`]
/// for branch range.
#[inline(never)]
fn make_results(
    pack: Option<&Pack<'_>>,
    b: &ssb_game::battle::Battle,
    roster: &Roster,
) -> (ssb_game::results::Results, alloc::boxed::Box<results_screen::Fighters>) {
    let r = ssb_game::results::Results::new(b);
    let entrants = roster.map(|e| e.map(|e| (e.kind, e.costume)));
    let f = results_screen::start(pack, &r, entrants);
    (r, f)
}

/// One frame of `mnVSResultsFuncRun`: the tic, the fighters (made from the
/// battle's `roster`, each with its costume) and the exit check. Out of
/// [`run`] for branch range.
#[inline(never)]
fn results_frame(
    pack: Option<&Pack<'_>>,
    results: &mut Option<ssb_game::results::Results>,
    fighters: &mut Option<alloc::boxed::Box<results_screen::Fighters>>,
    roster: &Roster,
    pressed: N64Buttons,
) -> bool {
    let Some(r) = results.as_mut() else {
        return false;
    };
    let leave = r.tick(pressed.contains(N64Buttons::START));
    if let Some(f) = fighters.as_deref_mut() {
        let entrants = roster.map(|e| e.map(|e| (e.kind, e.costume)));
        results_screen::tick(pack, r, f, entrants);
    }
    leave
}

/// What Training owns across frames, rebuilt on each stage entry.
struct TrainingWorld<'w> {
    play_state: &'w mut Option<play::FighterScene>,
    dummies: &'w mut Dummies,
    weapons: &'w mut ssb_game::weapon::WeaponPool,
    items: &'w mut ssb_game::item::ItemPool,
    stage_objects: &'w mut ssb_rom::ground_obj::GroundObjects,
    stage_map: &'w mut Option<alloc::boxed::Box<ssb_psp_runtime::scene::StageMap>>,
    stage_ctl: &'w mut ssb_game::stage::Stage,
    damage_hud: &'w mut Hud,
}

/// One fighter a battle makes (`FTDesc`): its fighter and costume, its CPU
/// level and handicap, its spawn point
/// (`mpCollisionGetPlayerMapObjPosition(player)`), its `team` (`FTDesc::team`:
/// the port, or the team in a team battle), its `players[].color` and
/// whether it is a human's.
#[derive(Clone, Copy)]
struct Entrant {
    kind: ssb_game::fighter::FighterKind,
    costume: u8,
    level: u8,
    handicap: u8,
    spawn: u16,
    team: u8,
    color: u8,
    human: bool,
}

/// A battle's fighters by port. The pad drives port 0: the host has one
/// controller.
type Roster = [Option<Entrant>; 4];

/// Training's two fighters from the character select's scene data: the
/// player on spawn 0 and the CPU dummy on spawn 1, at Training's level.
fn training_roster(scene: ssb_game::fighter_select::SceneData) -> Roster {
    let entrant = |kind: Option<ssb_game::fighter::FighterKind>, costume, port: u8| Entrant {
        kind: kind.unwrap_or(ssb_game::fighter::FighterKind::Mario),
        costume,
        level: play::TRAINING_CPU_LEVEL,
        handicap: ssb_game::stale::HANDICAP_DEFAULT,
        spawn: u16::from(port),
        team: port,
        // The player's port colour; the dummy's CPU grey.
        color: if port == 0 { 0 } else { ssb_game::hud::CPU_COLOR as u8 },
        human: port == 0,
    };
    [
        Some(entrant(scene.man_kind, scene.man_costume, 0)),
        Some(entrant(scene.com_kind, scene.com_costume, 1)),
        None,
        None,
    ]
}

/// `scVSBattleStartBattle`'s fighters from the VS battle state: every
/// present player with a fighter, on the spawn of its own player index.
/// The first human -- or, with none, the first fighter -- trades ports with
/// player 0 so the pad drives it, keeping its own spawn; any other human
/// runs as a CPU at its slot's level.
fn vs_roster(state: &ssb_game::players_vs::BattleState) -> Roster {
    use ssb_game::players_vs::PlayerKind;
    let mut roster: Roster = [None; 4];
    for (i, (entrant, p)) in roster.iter_mut().zip(&state.players).enumerate() {
        let Some(kind) = p.fkind.filter(|_| p.pkind != PlayerKind::Not) else {
            continue;
        };
        *entrant = Some(Entrant {
            kind,
            costume: p.costume,
            level: p.level,
            handicap: p.handicap,
            spawn: i as u16,
            team: p.player,
            color: p.color,
            human: p.pkind == PlayerKind::Man,
        });
    }
    let lead = roster
        .iter()
        .position(|e| e.is_some_and(|e| e.human))
        .or_else(|| roster.iter().position(Option::is_some));
    if let Some(i) = lead {
        roster.swap(0, i);
    }
    for e in roster.iter_mut().skip(1).flatten() {
        e.human = false;
    }
    roster
}

/// The fighters a capture scene that skips the selects starts with:
/// `vs4`'s Mario against a Fox, a Donkey Kong and a Kirby at level 3, each
/// in its first royal costume (`mnPlayersVSGetFreeCostumeRoyal` with no
/// fighter repeated); `vsteam`'s Mario and Kirby on red against Fox and
/// Donkey Kong on blue, in team costumes (`costume_team_id`) and team
/// colours; and otherwise [`capture_training_scene`]'s pair.
fn capture_roster(scene: GameScene, training: ssb_game::fighter_select::SceneData) -> Roster {
    use ssb_game::fighter::FighterKind;
    let (kinds, teams) = match scene {
        GameScene::Vs4 => (
            [FighterKind::Mario, FighterKind::Fox, FighterKind::Donkey, FighterKind::Kirby],
            None,
        ),
        GameScene::VsTeam | GameScene::VsTeamSteal => (
            [FighterKind::Mario, FighterKind::Kirby, FighterKind::Fox, FighterKind::Donkey],
            Some([0, 0, 1, 1]),
        ),
        GameScene::VsResults | GameScene::VsResultsEmblem => return vs_results_roster(),
        GameScene::VsArwing | GameScene::VsCar => return vs_pair_roster([FighterKind::Fox, FighterKind::Captain]),
        GameScene::VsBall | GameScene::VsRays => return vs_pair_roster([FighterKind::Pikachu, FighterKind::Purin]),
        _ => return training_roster(training),
    };
    core::array::from_fn(|port| {
        let (costume, team, color) = match teams {
            Some(teams) => {
                let team = teams[port];
                (
                    ssb_game::costume::costume_team_id(kinds[port], team),
                    team,
                    ssb_game::players_vs::TEAM_COLOR_IDS[usize::from(team)],
                )
            }
            None => (
                ssb_game::costume::costume_common_id(kinds[port], 0),
                port as u8,
                if port == 0 { 0 } else { ssb_game::hud::CPU_COLOR as u8 },
            ),
        };
        Some(Entrant {
            kind: kinds[port],
            costume,
            level: 3,
            handicap: ssb_game::stale::HANDICAP_DEFAULT,
            spawn: port as u16,
            team,
            color,
            human: port == 0,
        })
    })
}

/// RE-425's entry scenes: the player's first fighter against a CPU's
/// second, each in its first royal costume.
fn vs_pair_roster(kinds: [ssb_game::fighter::FighterKind; 2]) -> Roster {
    core::array::from_fn(|port| {
        kinds.get(port).map(|&kind| Entrant {
            kind,
            costume: ssb_game::costume::costume_common_id(kind, 0),
            level: 3,
            handicap: ssb_game::stale::HANDICAP_DEFAULT,
            spawn: port as u16,
            team: port as u8,
            color: if port == 0 { 0 } else { ssb_game::hud::CPU_COLOR as u8 },
            human: port == 0,
        })
    })
}

/// `vsresults`' fighters: the player's Luigi against a Kirby CPU, each in
/// its first royal costume. Luigi's claps are Mario's, played through his
/// translation scales; Kirby's Win clips lead with a runtime joint.
fn vs_results_roster() -> Roster {
    use ssb_game::fighter::FighterKind;
    let kinds = [FighterKind::Luigi, FighterKind::Kirby];
    core::array::from_fn(|port| {
        kinds.get(port).map(|&kind| Entrant {
            kind,
            costume: ssb_game::costume::costume_common_id(kind, 0),
            level: 3,
            handicap: ssb_game::stale::HANDICAP_DEFAULT,
            spawn: port as u16,
            team: port as u8,
            color: if port == 0 { 0 } else { ssb_game::hud::CPU_COLOR as u8 },
            human: port == 0,
        })
    })
}

/// Loads VS stage `gkind` for Training and spawns the roster's fighters on
/// it; returns the pack's stage index. Out of [`run`] so `run` stays inside
/// MIPS branch range.
#[inline(never)]
fn enter_training(
    pack: Option<&Pack<'_>>,
    gkind: u8,
    roster: Roster,
    vs: Option<VsRules>,
    battle: &mut Option<ssb_game::battle::Battle>,
    world: &mut TrainingWorld<'_>,
) -> u32 {
    world.weapons.reset();
    *world.items = ssb_game::item::ItemPool::default();
    // `gSCManagerBattleState`'s team rule, which every hit search reads;
    // Training is a free-for-all.
    ssb_game::spgame::live::reset_count();
    let team_rules = vs.map_or(ssb_game::team::TeamRules::FREE_FOR_ALL, |r| r.team_rules);
    world.weapons.team_rules = team_rules;
    world.items.team_rules = team_rules;
    let Some((p, index, stage)) = pack.and_then(|p| {
        let index = ssb_psp_runtime::scene::common_stage_index(p, gkind)?;
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
    // `gSCManagerBattleState`'s item switches: Training clears them
    // (`sc1PTrainingModeFuncStart`); VS takes Item Switch's
    // (`dSCManagerDefaultBattleState`'s until it is changed: every item at
    // middle appearance). `itManagerInitItems` builds the container drop
    // table before the ground exists.
    match vs {
        Some(rules) => world.items.normal_switches = rules.items,
        None => world.items.normal_switches.toggles = 0,
    }
    // `gSCManagerBattleState->damage_ratio`, which knockback reads.
    ssb_game::stale::set_damage_ratio(vs.map_or(ssb_game::stale::DAMAGE_RATIO_DEFAULT, |r| r.damage_ratio));
    world.items.normal_drops = ssb_game::item::normal::DropWeights::new(
        world.items.normal_switches,
        stage.item_weights.as_ref(),
    );
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
                world.items,
            )
        }
        None => ssb_game::stage::Stage::none(),
    };
    // `grCommonSetupInitAll`: the appearance actor follows the ground.
    world.items.make_appear_actor(
        stage.item_weights.as_ref(),
        p.stage_points(&stage)
            .filter(|point| point.kind == ssb_game::stage::mapobj::ITEM)
            .map(|point| ssb_engine::math::Vec3::new(point.x as f32, point.y as f32, 0.0)),
    );
    // `ftManagerMakeFighter` for each player in turn.
    let lead = roster[0].unwrap_or(training_roster(Default::default())[0].unwrap());
    let mut scene = play::FighterScene::at_spawn(p, &stage, lead.kind, lead.spawn);
    scene.fighter.port = 0;
    scene.fighter.team = lead.team;
    scene.fighter.costume = lead.costume;
    scene.fighter.handicap = lead.handicap;
    *world.play_state = Some(scene);
    *world.dummies = core::array::from_fn(|i| {
        let e = roster[i + 1]?;
        let port = i as u8 + 1;
        let mut d = play::Dummy::at_spawn(p, &stage, e.kind, e.costume, e.level, e.spawn, port)?;
        d.fighter.team = e.team;
        d.fighter.handicap = e.handicap;
        Some(alloc::boxed::Box::new(d))
    });
    // `ifCommonPlayerDamageInitInterface`; Training shows it at once
    // (`ifCommonPlayerDamageSetShowInterface`), VS at "Go".
    reset_damage_hud(world);
    world.damage_hud.colors = core::array::from_fn(|port| roster[port].map_or(port as u8, |e| e.color));
    let Some(pl) = world.play_state.as_mut() else {
        return index;
    };
    // `desc.detail` (`scvsbattle.c:188`, `sc1ptrainingmode.c:1809`): low
    // detail with three or four fighters (RE-426).
    let detail = ssb_game::modelpart::Detail::for_fighters(roster.iter().flatten().count());
    for f in scenes(pl, world.dummies).into_iter().flatten() {
        f.fighter.model_parts.detail_curr = detail;
        f.fighter.model_parts.detail_base = detail;
    }
    *battle = vs.map(|rules| {
        // `scVSBattleStartBattle`: each fighter faces the nearest other
        // player's spawn (`scVSBattleGetStartPlayerLR`), and a stock
        // battle's deaths take stocks.
        let spawn_x = |e: &Entrant| p.spawn(&stage, e.spawn).map(|s| f32::from(s.x));
        let stock_rule = rules.rule == ssb_game::battle::Rule::Stock;
        let mut players = [ssb_game::battle::Player::default(); 4];
        for (port, f) in scenes(pl, world.dummies).into_iter().enumerate() {
            let Some(f) = f else { continue };
            let me = roster[port].unwrap_or(lead);
            let others = roster
                .iter()
                .enumerate()
                .filter(|&(j, e)| j != port && e.is_some_and(|e| e.team != me.team))
                .filter_map(|(_, e)| e.as_ref().and_then(spawn_x));
            f.fighter.facing = if rules.demo {
                // `desc.lr = (desc.pos.x >= 0.0F) ? -1 : +1`.
                if f.fighter.pos.x >= 0.0 {
                    ssb_game::fighter::Facing::Left
                } else {
                    ssb_game::fighter::Facing::Right
                }
            } else {
                ssb_game::battle::start_facing(f.fighter.pos.x, others)
            };
            f.fighter.dead.stock_rule = stock_rule;
            f.fighter.dead.team_battle = rules.team_rules.is_team_battle;
            f.fighter.stocks = rules.stocks;
            // `ftParamUpdateDamage`'s and `ftParamUpdatePlayerBattleStats`'
            // totals, which the results save.
            f.fighter.stats.enable();
            // `ftManagerMakeFighter`: a VS fighter waits hidden for its
            // entry (`ftCommonEntrySetStatus`); the demos' fighters are set
            // up by their scenes.
            if !rules.demo {
                ssb_game::appear::entry_set_status(&mut f.fighter);
            }
            players[port] = ssb_game::battle::Player {
                present: true,
                is_human: port == 0 && me.human,
                team: me.team,
                ..Default::default()
            };
        }
        // A VS CPU runs the default trait and behaviour: it fights.
        for d in world.dummies.iter_mut().flatten() {
            d.computer.trait_kind = ssb_game::computer::attack::Trait::Default;
            d.computer.behavior = ssb_game::computer::Behavior::Default;
        }
        if rules.demo {
            return ssb_game::battle::Battle::new_demo(players);
        }
        ssb_game::battle::Battle::new(rules.rule, rules.time_limit, rules.stocks, players).with_teams(
            rules.team_rules.is_team_battle,
            rules.team_rules.is_team_attack,
        )
    });
    index
}

/// The frame's draw for the current screen. Out of [`run`] so `run` stays
/// inside MIPS branch range.
#[inline(never)]
unsafe fn draw_frame(
    gpu: &mut Gpu,
    s: &mut Session,
    pack: &Option<Pack<'_>>,
    draw_state: &mut meshdraw::DrawState,
    effect_visuals: &mut EffectVisuals,
    draw_assets: &DrawAssets,
    no_pack_color: Color,
) {
    match s.screen {
        Screen::CaptureIntro | Screen::CaptureMenu => {
            gpu.set_viewport_fullscreen();
            gpu.begin_frame(Some(BG_RESULTS));
        }
        Screen::Menus => s.menus.draw(gpu, pack.as_ref(), draw_state, &s.backup),
        Screen::Opening => opening_screen::draw(&s.opening, gpu, pack.as_ref(), draw_state),
        Screen::FighterSelect => draw_training_select(
            gpu,
            pack.as_ref(),
            draw_state,
            s.fighter_select.as_ref(),
            s.fighter_select_fighters.as_deref(),
        ),
        Screen::PlayersVs => draw_players_vs(
            gpu,
            pack.as_ref(),
            draw_state,
            s.players_vs.as_ref(),
            s.players_vs_fighters.as_deref(),
        ),
        Screen::Players1P => draw_players_1p(gpu, pack.as_ref(), draw_state, s),
        Screen::Players1PBonus => draw_players_1p_bonus(gpu, pack.as_ref(), draw_state, s),
        Screen::Campaign => campaign::draw(gpu, pack.as_ref(), draw_state, s),
        Screen::StageSelect => match (pack.as_ref(), s.stage_select_layer.as_ref()) {
            // `gcMakeDefaultCameraGObj`'s black clear, then the cameras.
            (Some(p), Some(layer)) => {
                gpu.set_viewport_fullscreen();
                gpu.begin_frame(Some(Color::rgba(0, 0, 0, 0xFF)));
                stage_screen::draw_all(gpu, p, draw_state, &s.stage_select, layer, &s.stage_preview);
            }
            _ => {
                gpu.set_viewport_fullscreen();
                gpu.begin_frame(Some(BG_MENU));
                draw_stage_select(gpu, &s.stage_select);
            }
        },
        Screen::Message => {
            gpu.set_viewport_fullscreen();
            gpu.begin_frame(Some(BG_RESULTS));
            if let (Some(p), Some(m)) = (pack.as_ref(), s.vs_message.as_ref()) {
                campaign::draw_message(gpu, p, draw_state, m);
            }
        }
        Screen::Results => {
            draw_results(
                gpu,
                pack.as_ref(),
                draw_state,
                s.vs_results.as_ref(),
                s.vs_results_fighters.as_deref_mut(),
                &s.roster,
            );
        }
        // An opening battle scene before its battle exists (RE-467).
        Screen::Training if s.play_state.is_none() && matches!(s.demo, Some(demo_screen::Demo::Movie(_))) => {
            if let Some(demo) = s.demo.as_ref() {
                demo_screen::draw_movie_only(gpu, pack.as_ref(), draw_state, demo);
            }
        }
        Screen::Training => {
            if let (Some(p), Some(pl)) = (pack.as_ref(), s.play_state.as_ref()) {
                effect_visuals.sync(p, draw_assets, &pl.fighter, &s.weapons, &s.items);
                effect_visuals.sync_entry(
                    p,
                    draw_assets,
                    scenes_ref(pl, &s.dummies).map(|x| x.map(|x| &x.fighter)),
                );
                effect_visuals.sync_ko(
                    p,
                    draw_assets,
                    &s.damage_hud.ko,
                    scenes_ref(pl, &s.dummies).map(|x| x.map(|x| &x.fighter)),
                );
                effect_visuals.sync_eggs(
                    p,
                    draw_assets,
                    scenes_ref(pl, &s.dummies).map(|x| x.map(|x| &x.fighter)),
                );
            }
            // `grWallpaperCommonProcUpdate` or `grWallpaperSectorProcUpdate`:
            // process priority 3, after the battle camera's, so from this
            // tick's camera. Recomputed from the same camera, it is the same
            // on a frame drawn without a tick.
            if let Some(pl) = s.play_state.as_ref() {
                s.wallpaper.update(pl.camera.eye, pl.camera.at);
            }
            if let (Some(p), Some(b)) = (pack.as_ref(), s.campaign.as_mut().and_then(|c| c.boss.as_deref_mut())) {
                if let Some(effects) = b.effects.as_mut() {
                    effects.sync(p, &b.wallpaper);
                }
            }
            let magnify_hidden = boss_magnify_hidden(s);
            // The boss wallpaper's fades step once per drawn frame.
            s.damage_hud.boss_fade = s
                .campaign
                .as_mut()
                .and_then(|c| c.boss.as_mut())
                .and_then(|b| b.wallpaper.display())
                .map(|(d, v)| match d {
                    ssb_game::spgame::boss::Display::FadeAlpha => [0xFF, 0xFF, 0xFF, v],
                    _ => [v, v, v, 0xFF],
                });
            draw_training(
                gpu,
                draw_state,
                pack.as_ref(),
                s.training_stage,
                s.play_state.as_ref(),
                &s.dummies,
                &s.weapons,
                &s.items,
                draw_assets,
                effect_visuals,
                Some(&s.material_anim),
                s.stage_map.as_deref(),
                Some(&s.stage_objects),
                no_pack_color,
                &mut s.damage_hud,
                s.vs_battle.as_ref(),
                s.wallpaper_sprite.as_ref().map(|sprite| (sprite, &s.wallpaper)),
                s.training_paused,
                s.training_menu.as_ref().is_none_or(|m| m.magnify_display)
                    && s.demo.as_ref().is_none_or(demo_screen::magnify_display)
                    && !magnify_hidden,
                // The demos tag each player by its number (`players[].tag
                // = player`).
                if s.demo.is_some() { [false; 4] } else { s.roster.map(|x| x.is_some_and(|x| !x.human)) },
                s.campaign
                    .as_ref()
                    .and_then(|c| c.boss.as_deref())
                    .and_then(|b| b.effects.as_ref().map(|e| (e, &b.wallpaper))),
            );
            if let Some(demo) = s.demo.as_ref() {
                demo_screen::draw(gpu, pack.as_ref(), draw_state, demo);
            }
            if let (Some(p), Some(menu), Some(pl)) = (pack.as_ref(), s.training_menu.as_mut(), s.play_state.as_ref()) {
                let dummy = s.dummies[0].as_ref().map(|x| &x.fighter);
                if !s.training_paused {
                    menu.stats.observe(dummy.map_or(0, |f| f.combo_damage_foe), dummy.map_or(0, |f| f.combo_count_foe));
                }
                let held = pl.fighter.items.held.and_then(|h| s.items.get(h.slot)).map(|it| it.kind);
                training_screen::draw(p, draw_state, menu, s.training_paused, ssb_game::training::held_item_option(held));
            }
        }
    }
}

/// Master Hand's stage shows the magnifiers only from its "Go"
/// (`sc1PGameWaitStageBossUpdate` sets `is_magnify_display`).
fn boss_magnify_hidden(s: &Session) -> bool {
    s.campaign.as_ref().is_some_and(|c| c.boss.is_some())
        && s.vs_battle
            .as_ref()
            .is_some_and(|b| b.status == ssb_game::battle::GameStatus::Wait)
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
        let mut lag_tic = false;
        // An opening scene, or the title after it, waiting for its tic:
        // the picture holds.
        if s.opening_pending.is_some() || s.title_pending {
            s.opening.clock.retrace();
            if let Some(kind) = s.opening_pending {
                if s.opening.clock.ready(kind) {
                    s.opening_pending = None;
                    begin_opening(s, pack.as_ref(), kind, capture_scene.is_some());
                }
            } else if s.opening.clock.tic >= ssb_game::opening::TITLE_START_TIC {
                s.title_pending = false;
                let newcomers = MScene::Opening(ssb_game::opening::Kind::Newcomers);
                go_scene(s, pack.as_ref(), MScene::Title, newcomers, capture_scene.is_some());
            }
            return;
        }
        #[cfg(feature = "headless_capture")]
        log_opening_memory(sim_frame_index, s.menus.scene);
        match s.screen {
            Screen::CaptureIntro => {
                if let Some((scene, prev)) = capture_scene.and_then(capture_menu) {
                    // The title picks the demo fighters on its way out.
                    if scene == MScene::AutoDemo {
                        s.menus.demo.set_demo_fighter_kinds(&s.backup, &mut ssb_game::rng::rand_int_range);
                    }
                    go_scene(s, pack.as_ref(), scene, prev, true);
                } else if pressed.contains(N64Buttons::A) || pressed.contains(N64Buttons::START) {
                    s.screen = Screen::CaptureMenu;
                }
            }
            Screen::CaptureMenu => {
                if pressed.contains(N64Buttons::A) && (s.cursor == OPTION_ENTRY || s.cursor == DATA_ENTRY) {
                    let scene = if s.cursor == OPTION_ENTRY {
                        ssb_game::menu::Scene::Option
                    } else {
                        ssb_game::menu::Scene::Data
                    };
                    enter_menus(s, pack.as_ref(), scene, ssb_game::menu::Scene::ModeSelect, capture_scene.is_some());
                } else if pressed.contains(N64Buttons::A) && s.cursor == ONE_P_ENTRY {
                    s.players_1p = Some(ssb_game::players_1p::Players1P::new(s.one_p_scene, &s.backup));
                    s.players_1p_fighters = None;
                    s.screen = Screen::Players1P;
                } else if menu_stick_down_pressed(previous_controller, controller) {
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
                        s.dummies = Default::default();
                    }
                    s.vs = s.cursor == VS_ENTRY;
                    let rules = s.vs.then(|| capture_scene.map_or(VsRules::DEFAULT, vs_rules));
                    if s.play_state.is_some() {
                        s.screen = Screen::Training;
                    } else if route == Some(CaptureRoute::Direct) {
                        let roster = capture_scene.map_or(training_roster(s.training_scene), |scene| {
                            capture_roster(scene, s.training_scene)
                        });
                        let gkind = capture_stage_gkind(capture_scene);
                        s.enter(pack.as_ref(), gkind, roster, rules);
                        prepare_monster_capture(capture_scene, pack.as_ref(), s);
                        s.scene_gkind = gkind;
                        // These scenes set the behaviour directly rather than
                        // through Training's CP option (`trainingmenu` does).
                        if let Some(b) = capture_scene.and_then(capture_cpu_behavior) {
                            for d in s.dummies.iter_mut().flatten() {
                                d.computer.behavior = b;
                                d.computer.trait_kind = ssb_game::computer::attack::Trait::None;
                                // Level 9 tracks an item without waiting.
                                if capture_scene == Some(GameScene::TrainingCpuItem) {
                                    d.computer.level = ssb_game::computer::LEVEL_MAX;
                                }
                            }
                        }
                        s.screen = Screen::Training;
                    } else if route == Some(CaptureRoute::StageSelect) {
                        s.open_stage_select(s.maps_training_gkind);
                    } else if s.vs {
                        go_scene(s, pack.as_ref(), MScene::VsMode, MScene::ModeSelect, true);
                    } else {
                        s.fighter_select =
                            Some(new_fighter_select(s.training_scene, s.backup.fighter_mask, capture_scene.is_some(), sim_frame_index));
                        s.fighter_select_fighters = None;
                        s.screen = Screen::FighterSelect;
                    }
                }
            }
            Screen::Menus => menus_frame(s, pack.as_ref(), controller, pressed, capture_scene.is_some()),
            Screen::Opening => {
                // `scSubsysControllerGetPlayerTapButtons(A_BUTTON | B_BUTTON | START_BUTTON)`.
                let tapped = pressed.contains(N64Buttons::A | N64Buttons::B | N64Buttons::START);
                if let Some(next) = opening_screen::frame(&mut s.opening, pack.as_ref(), s.pack_path, tapped) {
                    let from = s.menus.scene;
                    go_scene(s, pack.as_ref(), next, from, capture_scene.is_some());
                }
            }
            Screen::PlayersVs => {
                players_vs_frame(s, pack, capture_scene.is_some(), sim_frame_index, controller, pressed);
            }
            Screen::Players1P => players_1p_frame(s, pack.as_ref(), controller, pressed),
            Screen::Players1PBonus => players_1p_bonus_frame(s, pack.as_ref(), controller, pressed),
            Screen::Campaign => campaign::frame(s, pack.as_ref(), controller, pressed),
            Screen::FighterSelect => {
                use ssb_game::fighter_select::Outcome;
                match fighter_select_frame(s, pack.as_ref(), controller, pressed) {
                    Some(Outcome::Proceed(data)) => {
                        s.training_scene = data;
                        // `mnMapsInitVars`: the s.cursor starts on the
                        // stage this mode picked last; Mushroom Kingdom
                        // needs the backup's unlock.
                        let remembered = if s.vs { s.maps_vsmode_gkind } else { s.maps_training_gkind };
                        s.open_stage_select(remembered);
                    }
                    Some(Outcome::Back(data)) => {
                        s.training_scene = data;
                        go_scene(s, pack.as_ref(), MScene::OnePMode, MScene::Players1PTraining, capture_scene.is_some());
                    }
                    Some(Outcome::Timeout(data)) => {
                        s.training_scene = data;
                        go_scene(s, pack.as_ref(), MScene::Title, MScene::Players1PTraining, capture_scene.is_some());
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
                    let roster = if s.vs {
                        vs_roster(&s.vs_state)
                    } else {
                        training_roster(s.training_scene)
                    };
                    s.enter(pack.as_ref(), saved.gkind, roster, s.vs.then_some(s.vs_menu_rules));
                    s.screen = Screen::Training;
                }
                // B returns to the character select, with the fighters
                // it saved. B and the idle return also save the scene
                // data.
                Some(ssb_game::stage_select::Outcome::Back) => {
                    let saved = s.stage_select.save(s.scene_gkind, stage_select_rand);
                    s.scene_gkind = saved.gkind;
                    // `mnMapsFuncRun`: back to the select this mode came
                    // from.
                    if s.vs {
                        s.maps_vsmode_gkind = saved.remembered;
                        s.players_vs = Some(new_players_vs(s.vs_state, s.scene_gkind, &s.backup));
                        s.players_vs_fighters = None;
                        s.screen = Screen::PlayersVs;
                    } else {
                        s.maps_training_gkind = saved.remembered;
                        s.fighter_select = Some(ssb_game::fighter_select::FighterSelect::new(
                            s.training_scene,
                            s.backup.fighter_mask,
                            clock_byte,
                        ));
                        s.fighter_select_fighters = None;
                        s.screen = Screen::FighterSelect;
                    }
                }
                Some(ssb_game::stage_select::Outcome::Timeout) => {
                    let saved = s.stage_select.save(s.scene_gkind, stage_select_rand);
                    if s.vs {
                        s.maps_vsmode_gkind = saved.remembered;
                    } else {
                        s.maps_training_gkind = saved.remembered;
                    }
                    s.scene_gkind = saved.gkind;
                    go_scene(s, pack.as_ref(), MScene::Title, MScene::Maps, capture_scene.is_some());
                }
                None => tick_stage_select_layer(pack.as_ref(), s),
            },
            Screen::Training => {
                #[cfg(feature = "headless_capture")]
                log_entry_state(capture_scene, sim_frame_index, s.play_state.as_ref(), s.dummies[0].as_deref());
                // In a VS battle START is the pause menu's (`training_frame`).
                if s.vs_battle.is_none() && s.demo.is_none() {
                    lag_tic = training_menu_frame(pack.as_ref(), s, controller, pressed, capture_scene.is_some(), sim_frame_index);
                }
            }
            // `mnVSResultsCheckExit`: START after the wait, on to the
            // VS character select.
            Screen::Results => {
                #[cfg(feature = "headless_capture")]
                log_results_tic(sim_frame_index, s.vs_results.as_ref());
                if results_frame(pack.as_ref(), &mut s.vs_results, &mut s.vs_results_fighters, &s.roster, pressed) {
                    s.vs_results_fighters = None;
                    s.play_state = None;
                    s.dummies = Default::default();
                    s.vs_battle = None;
                    // Any unlock goes through the message scene first.
                    let [first, next] = ssb_game::results::unlocks(&s.backup);
                    s.vs_message = first.map(ssb_game::spgame::message::Message::new);
                    s.vs_message_next = next;
                    s.message_after = MScene::PlayersVs;
                    if s.vs_message.is_some() {
                        s.screen = Screen::Message;
                    } else {
                        s.players_vs = Some(new_players_vs(s.vs_state, s.scene_gkind, &s.backup));
                        s.players_vs_fighters = None;
                        s.screen = Screen::PlayersVs;
                    }
                }
            }
            // `mnMessageFuncRun`, one task per queued unlock, then
            // `nSCKindPlayersVS`.
            Screen::Message => {
                let done = s.vs_message.as_mut().is_none_or(|m| m.tick(pressed, &mut s.backup));
                if done {
                    s.vs_message = s.vs_message_next.take().map(ssb_game::spgame::message::Message::new);
                    // `mnMessageFuncRun`'s last task: the VS select after
                    // the VS results, else the N64 logo (skipped to the
                    // title).
                    if s.vs_message.is_none() {
                        if s.message_after == MScene::PlayersVs {
                            s.players_vs = Some(new_players_vs(s.vs_state, s.scene_gkind, &s.backup));
                            s.players_vs_fighters = None;
                            s.screen = Screen::PlayersVs;
                        } else {
                            go_scene(s, pack.as_ref(), MScene::Startup, MScene::Message, capture_scene.is_some());
                        }
                    }
                }
            }
        }

        let mut vs_done = false;
        if lag_tic {
            // `sc1PTrainingModeCheckLagTic`: a slowed tick runs only the
            // camera.
            if let (Some(p), Some(pl)) = (&pack, s.play_state.as_mut()) {
                if let Some(stage) = p.stage(s.training_stage) {
                    tick_battle_camera(&stage, &mut scenes(pl, &mut s.dummies));
                }
            }
        }
        if capture_scene == Some(GameScene::StarKo) && sim_frame_index == STAR_KO_TICK {
            if let Some(pl) = s.play_state.as_mut() {
                ssb_game::dead::set_dead_up_star(&mut pl.fighter);
            }
        }
        // The title demos' exit: A, B or START on the pad (any
        // controller's `button_tap`).
        let demo_tapped = pressed.contains(N64Buttons::A | N64Buttons::B | N64Buttons::START);
        let mut demo_load = None;
        if s.screen == Screen::Training && s.demo.is_some() {
            demo_load = demo_screen::before_world(s, pack.as_ref(), demo_tapped);
        }
        if let (Screen::Training, false, Some(p), Some(pl)) = (s.screen, lag_tic, &pack, s.play_state.as_mut()) {
            let (campaign_session, bonus_stage, boss_scene) = s.campaign.as_mut().map_or((None, None, None), |c|
                (c.frontend.session.as_deref_mut(), c.bonus.as_deref_mut(), c.boss.as_deref_mut()));
            vs_done = training_frame(
                p,
                s.training_stage,
                pl,
                &mut s.dummies,
                &mut s.weapons,
                &mut s.items,
                &mut s.material_anim,
                &mut s.stage_objects,
                &mut s.stage_map,
                &mut s.stage_ctl,
                controller,
                pressed,
                &mut s.vs_battle,
                &mut s.damage_hud,
                s.training_paused,
                campaign_session,
                bonus_stage,
                boss_scene,
                &mut s.lead,
            );
            if let Some(m) = s.training_menu.as_mut() {
                m.tick_processes();
            }
            if s.demo.is_some() {
                demo_load = demo_load.or(demo_screen::after_world(s, demo_tapped));
            }
            let magnify_display = s.training_menu.as_ref().is_none_or(|m| m.magnify_display)
                && s.demo.as_ref().is_none_or(demo_screen::magnify_display)
                && !boss_magnify_hidden(s)
                && s.vs_battle.as_ref().is_none_or(|b| {
                    matches!(
                        b.status,
                        ssb_game::battle::GameStatus::Wait | ssb_game::battle::GameStatus::Go
                    )
                });
            s.damage_hud.players.tick(magnify_display);
            #[cfg(feature = "headless_capture")]
            if capture_scene == Some(GameScene::TrainingInterface) && sim_frame_index >= 40 {
                for (i, f) in scenes(s.play_state.as_mut().unwrap(), &mut s.dummies)
                    .into_iter()
                    .enumerate()
                {
                    let Some(f) = f else { continue };
                    ssb_game::status::set_fall(&mut f.fighter);
                    f.fighter.pos =
                        ssb_engine::math::Vec3::new(if i == 0 { -6500.0 } else { 6500.0 }, 1500.0, 0.0);
                    f.fighter.physics.vel_air = ssb_engine::math::Vec3::ZERO;
                }
            }
        }
        // An opening battle's movie camera and posed panel (RE-467).
        if s.screen == Screen::Training && matches!(s.demo, Some(demo_screen::Demo::Movie(_))) {
            demo_screen::movie_after_world(s, pack.as_ref());
        }
        // A title demo's `syTaskmanSetLoadScene`.
        if let Some(next) = demo_load {
            let from = s.menus.scene;
            // An opening battle stays up until the next scene starts.
            if !(matches!(next, MScene::Opening(_)) && matches!(s.demo, Some(demo_screen::Demo::Movie(_)))) {
                demo_screen::leave(s);
            }
            go_scene(s, pack.as_ref(), next, from, capture_scene.is_some());
            return;
        }
        // `scVSBattleStartScene`: a tied time battle goes to sudden
        // death on the same stage, then to the results.
        if vs_done && s.campaign.is_some() {
            campaign::finish_battle(s, pack.as_ref());
        } else if vs_done {
            if s.vs_battle.as_ref().is_some_and(|b| !b.is_sudden_death) {
                s.vs_transfer = s.vs_battle.clone();
            }
            let sudden = s.vs_battle
                .as_ref()
                .filter(|b| !b.is_sudden_death && !b.is_reset)
                .and_then(ssb_game::battle::Battle::sudden_death_battle);
            let entered = sudden.and_then(|battle| {
                start_sudden_death(
                    pack.as_ref(),
                    s.scene_gkind,
                    s.roster,
                    battle,
                    &mut s.vs_battle,
                    &mut TrainingWorld {
                        play_state: &mut s.play_state,
                        dummies: &mut s.dummies,
                        weapons: &mut s.weapons,
                        items: &mut s.items,
                        stage_objects: &mut s.stage_objects,
                        stage_map: &mut s.stage_map,
                        stage_ctl: &mut s.stage_ctl,
                        damage_hud: &mut s.damage_hud,
                    },
                )
            });
            match entered {
                Some(index) => {
                    s.training_stage = index;
                    s.apply_backup_options();
                }
                // A reset from the pause menu is a no contest.
                None => {
                    // `mnVSResultsFuncStart`'s `mnVSResultsSaveBackup`.
                    if let Some(transfer) = s.vs_transfer.take() {
                        let kinds = s.roster.map(|e| e.map(|e| e.kind));
                        ssb_game::results::save_backup(&mut s.backup, &transfer, kinds, s.scene_gkind);
                    }
                    let (results, fighters) = s
                        .vs_battle
                        .as_ref()
                        .map(|b| make_results(pack.as_ref(), b, &s.roster))
                        .unzip();
                    s.vs_results = results;
                    s.vs_results_fighters = fighters;
                    s.screen = Screen::Results;
                }
            }
        }
}

/// `sc1PTrainingModeUpdateAll`'s menu half: START during Go opens
/// Training's menu, which then runs its options until B or START leaves
/// it, or Reset or Exit loads the next scene. Returns whether this tick is
/// a slowed speed's skipped one (`sc1PTrainingModeCheckLagTic`).
#[inline(never)]
fn training_menu_frame(
    pack: Option<&Pack<'_>>,
    s: &mut Session,
    controller: ControllerState,
    pressed: N64Buttons,
    is_capture: bool,
    sim_frame_index: u64,
) -> bool {
    use ssb_game::training;
    let (Some(menu), Some(pl)) = (s.training_menu.as_mut(), s.play_state.as_mut()) else {
        return false;
    };
    if !s.training_paused {
        // `sc1PTrainingModeCheckEnterMenu`.
        if menu.check_enter(pressed, pl.fighter.dead.is_menu_ignore) {
            s.training_paused = true;
        }
        return menu.check_lag_tic();
    }
    let item_count = s
        .items
        .items()
        .filter(|it| training::counts_toward_limit(it.kind))
        .count();
    let frame = menu.update(&controller, pressed, item_count);
    if frame.cp_changed {
        // `sc1PTrainingModeUpdateDummyBehavior`.
        let behavior = menu.dummy_behavior();
        if let Some(d) = s.dummies[usize::from(menu.dummy) - 1].as_deref_mut() {
            d.computer.behavior = behavior;
            d.computer.trait_kind = ssb_game::computer::attack::Trait::None;
        }
    }
    if let Some(kind) = frame.spawn_item {
        let mut pos = pl.fighter.pos;
        pos.y += 200.0;
        pos.z = 0.0;
        let vel = ssb_engine::math::Vec3::new(0.0, 30.0, 0.0);
        s.items.make_setup_common(kind, None, pos, vel, &core::iter::empty);
    }
    match frame.view_changed {
        Some(training::VIEW_NORMAL) => pl.player_zoom = None,
        Some(_) => {
            pl.player_zoom =
                Some(pack.and_then(|p| p.fighter(pl.fighter.kind as u32)).map_or(1000.0, |d| d.closeup_camera_zoom));
        }
        None => {}
    }
    if let Some(b_held) = frame.leave {
        // `sc1PTrainingModeCheckLeaveMenu`: Go, both fighters unlocked;
        // B stays held so it is no special.
        s.training_paused = false;
        if b_held {
            pl.fighter.input.buttons.0 |= N64Buttons::B;
        }
    }
    if frame.load_scene {
        // `sc1PTrainingModeStartScene`: Reset runs Training again; Exit
        // goes to its character select (`nSCKindPlayers1PTraining`).
        if menu.exit_or_reset {
            let roster = s.roster;
            s.enter(pack, s.scene_gkind, roster, None);
        } else {
            s.play_state = None;
            s.dummies = Default::default();
            s.training_menu = None;
            s.training_paused = false;
            s.fighter_select = Some(new_fighter_select(s.training_scene, s.backup.fighter_mask, is_capture, sim_frame_index));
            s.fighter_select_fighters = None;
            s.screen = Screen::FighterSelect;
        }
        return false;
    }
    menu.check_lag_tic()
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
    dummies: Dummies,
    /// Boxed: the pool holds its effect queues (RE-416), which building the
    /// session would otherwise copy through `run`'s frame.
    weapons: alloc::boxed::Box<ssb_game::weapon::WeaponPool>,
    items: ssb_game::item::ItemPool,
    stage_objects: ssb_rom::ground_obj::GroundObjects,
    stage_ctl: ssb_game::stage::Stage,
    screen: Screen,
    cursor: usize,
    training_scene: ssb_game::fighter_select::SceneData,
    /// The last battle's fighters, for its sudden death.
    roster: Roster,
    vs: bool,
    vs_battle: Option<ssb_game::battle::Battle>,
    /// Training's menu (`sSC1PTrainingModeMenu`); `None` in a VS battle.
    training_menu: Option<ssb_game::training::TrainingMenu>,
    /// `game_status == nSCBattleGameStatusPause` in Training: the menu is
    /// open.
    training_paused: bool,
    maps_vsmode_gkind: u8,
    vs_menu_rules: VsRules,
    vs_results: Option<ssb_game::results::Results>,
    /// `gSCManagerTransferBattleState`: the battle as it ended before any
    /// sudden death, which `mnVSResultsSaveBackup` records.
    vs_transfer: Option<ssb_game::battle::Battle>,
    /// The VS results' unlock message and the one queued after it.
    vs_message: Option<ssb_game::spgame::message::Message>,
    vs_message_next: Option<ssb_game::spgame::Unlock>,
    /// The scene after the message: the VS select after the VS results'
    /// unlocks, the N64 logo after Bonus Practice's (`mnMessage`).
    message_after: MScene,
    /// The Bonus Practice select (`mnPlayers1PBonus`), its fighter's pose
    /// and its sprite pack.
    players_1p_bonus: Option<alloc::boxed::Box<ssb_game::players_1p_bonus::Players1PBonus>>,
    players_1p_bonus_fighters: Option<alloc::boxed::Box<players_screen::Fighters>>,
    players_1p_bonus_sprites: Option<assets::AlignedBuf>,
    /// `gSCManagerSceneData.player`, `.bonus_fkind` and `.bonus_costume`.
    bonus_saved: ssb_game::players_1p_bonus::Saved,
    /// The results' fighters (RE-409), on the heap.
    vs_results_fighters: Option<alloc::boxed::Box<results_screen::Fighters>>,
    fighter_select: Option<ssb_game::fighter_select::FighterSelect>,
    /// The Training select's fighter poses, on the heap.
    fighter_select_fighters: Option<alloc::boxed::Box<players_screen::Fighters>>,
    /// `gSCManagerTransferBattleState` between the VS menus.
    vs_state: ssb_game::players_vs::BattleState,
    players_vs: Option<ssb_game::players_vs::PlayersVs>,
    /// The select's fighter poses (RE-411), on the heap.
    players_vs_fighters: Option<alloc::boxed::Box<players_screen::Fighters>>,
    players_1p: Option<ssb_game::players_1p::Players1P>,
    players_1p_fighters: Option<alloc::boxed::Box<players_screen::Fighters>>,
    /// What the 1P select left in `gSCManagerSceneData` for its next visit.
    one_p_scene: ssb_game::players_1p::SceneData,
    /// The 1P Game's part of `gSCManagerSceneData`.
    spgame_scene: ssb_game::spgame::SceneData,
    /// `gSCManagerBackupData`, loaded at boot and saved on every write.
    backup: ssb_game::backup::Backup,
    save: save::Save,
    /// Where the pack loaded from: the menus' sprite packs sit beside it.
    pack_path: Option<&'static str>,
    /// The options and data menus (RE-461).
    menus: menus_screen::Menus,
    /// `dSYAudioSoundQuality`, which `lbBackupApplyOptions` and Option set:
    /// 1 stereo, 0 mono. The port's audio has no mono mix (RE-461).
    sound_quality: u8,
    /// `syVideoSetCenterOffsets`' horizontal and vertical offsets. The PSP's
    /// picture does not move (RE-461).
    video_offsets: (i16, i16),
    /// The running 1P Game, between the select's START and its end.
    campaign: Option<campaign::Campaign>,
    /// The stage select's presentation (RE-419), made with the select.
    stage_select_layer: Option<ssb_game::stage_select_layer::Layer>,
    /// The select's preview model and its clocks, on the heap.
    stage_preview: alloc::boxed::Box<stage_screen::Preview>,
    /// The battle's wallpaper `SObj` and its sprite (RE-419).
    wallpaper: ssb_game::wallpaper::Wallpaper,
    wallpaper_sprite: Option<ssb_rom::pack::SpriteDesc>,
    /// What drives port 0: the pad, or in the title's demos How to Play's
    /// input script or the auto demo's CPU.
    lead: play::Lead,
    /// The running title demo (`scExplain`, `scAutoDemo`, RE-465).
    demo: Option<demo_screen::Demo>,
    /// The N64 logo and the opening movie (RE-467): its music clock and
    /// running scene.
    opening: alloc::boxed::Box<opening_screen::Opening>,
    /// An opening scene whose start waits for the music's tic: nothing
    /// ticks and the last picture holds until then.
    opening_pending: Option<ssb_game::opening::Kind>,
    /// The title after the newcomers, waiting for
    /// [`ssb_game::opening::TITLE_START_TIC`].
    title_pending: bool,
}

impl Session {
    /// [`enter_training`] into this session's world with `roster`,
    /// remembering it for a sudden death.
    fn enter(&mut self, pack: Option<&Pack<'_>>, gkind: u8, roster: Roster, rules: Option<VsRules>) {
        self.roster = roster;
        self.training_stage = enter_training(
            pack,
            gkind,
            roster,
            rules,
            &mut self.vs_battle,
            &mut TrainingWorld {
                play_state: &mut self.play_state,
                dummies: &mut self.dummies,
                weapons: &mut self.weapons,
                items: &mut self.items,
                stage_objects: &mut self.stage_objects,
                stage_map: &mut self.stage_map,
                stage_ctl: &mut self.stage_ctl,
                damage_hud: &mut self.damage_hud,
            },
        );
        self.make_wallpaper(pack, gkind, rules.is_none());
        self.apply_backup_options();
        // `sc1PTrainingModeInitVars`; the player is on port 0.
        self.training_menu = rules.is_none().then(|| ssb_game::training::TrainingMenu::new(0));
        self.training_paused = false;
    }

    /// The backup's battle settings: `ifScreenFlashMakeInterface`'s
    /// `is_allow_screenflash` and Mew's newcomer gate (`itMainMakeMonster`).
    fn apply_backup_options(&mut self) {
        self.damage_hud.ko.flash_disabled = !self.backup.is_allow_screenflash;
        self.items.unlock_newcomers =
            self.backup.unlock_mask & ssb_game::backup::UNLOCK_MASK_NEWCOMERS != 0;
    }

    /// `grWallpaperMakeDecideKind`, and for Training
    /// `sc1PTrainingModeLoadWallpaper`'s sprite and
    /// `sc1PTrainingModeInitDisplayVars`' fog colour, which a star KO fades
    /// towards (RE-419).
    #[inline(never)]
    fn make_wallpaper(&mut self, pack: Option<&Pack<'_>>, gkind: u8, is_training: bool) {
        use ssb_game::wallpaper;
        self.wallpaper = wallpaper::Wallpaper::make(wallpaper::decide_kind(is_training, gkind));
        let training = wallpaper::training_wallpaper(gkind).filter(|_| is_training);
        self.wallpaper_sprite = pack.and_then(|p| match training {
            Some(t) => p.sprite(t.file, wallpaper::TRAINING_WALLPAPER_SPRITE),
            None => p.stage_wallpaper(gkind),
        });
        if let (Some(t), Some(pl)) = (training, self.play_state.as_mut()) {
            for f in scenes(pl, &mut self.dummies).into_iter().flatten() {
                if let Some(b) = f.fighter.dead.bounds.as_mut() {
                    b.fog_color = t.fog_color;
                }
            }
        }
    }

    /// `nSCKindMaps`: the stage select on the kind this mode picked last,
    /// with its presentation (`mnMapsFuncStart`).
    fn open_stage_select(&mut self, remembered: u8) {
        // `mnMapsFuncStart`: `sMNMapsUnlockedMask` from the backup.
        self.stage_select = ssb_game::stage_select::StageSelect::new(remembered, self.backup.unlock_mask);
        self.stage_select_layer = Some(ssb_game::stage_select_layer::Layer::new(&self.stage_select, !self.vs));
        self.screen = Screen::StageSelect;
    }
}

unsafe fn run() -> ! {
    // The scripted scene this run captures; `None` reads the real pad.
    let capture = capture::select();
    // The save scenes run another scene's route and script.
    let capture_spec = capture.map(|c| c.scene);
    let capture_scene = capture_spec.map(GameScene::script);
    let mut gpu = Gpu::init();
    // Select this application’s layout at the PSP backend boundary.  The
    // asset viewer keeps PspInput::init() and therefore its legacy controls.
    let mut pad = PspInput::init_with_mapping(SSB64_GAME_MAPPING);

    // Load the converted asset pack. Held for the whole program: the GE
    // reads vertex and texture data out of it by DMA once the training
    // scene draws real meshes below.
    let loaded = assets::load_pack();
    // `lbBackupIsSramValid` and `lbBackupApplyOptions`, before the first
    // scene.
    let (backup, save) = save::boot(loaded.as_ref().ok().map(|(_, p)| *p), capture_spec);
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
    // On the heap: the main thread's stack is 256 KB (`psp::module!`), and
    // the session alone is some 100 KB.
    let mut s = alloc::boxed::Box::new(Session {
        material_anim: ssb_rom::skeleton::MaterialAnimator::new(),
        stage_map: None,
        training_stage: 0,
        scene_gkind: ssb_game::stage_select::DEFAULT_GKIND,
        maps_training_gkind: ssb_game::stage_select::DEFAULT_GKIND,
        stage_select: ssb_game::stage_select::StageSelect::new(ssb_game::stage_select::DEFAULT_GKIND, backup.unlock_mask),
        damage_hud: Hud::new(),
        play_state: None,
        dummies: Default::default(),
        weapons: new_weapon_pool(),
        items: ssb_game::item::ItemPool::default(),
        stage_objects: ssb_rom::ground_obj::GroundObjects::empty(),
        stage_ctl: ssb_game::stage::Stage::none(),
        screen: Screen::CaptureIntro,
        cursor: 0,
        training_scene: ssb_game::fighter_select::SceneData::default(),
        roster: [None; 4],
        vs: false,
        vs_battle: None,
        training_menu: None,
        training_paused: false,
        maps_vsmode_gkind: ssb_game::stage_select::DEFAULT_GKIND,
        vs_menu_rules: VsRules::DEFAULT,
        vs_results: None,
        vs_transfer: None,
        vs_message: None,
        vs_message_next: None,
        message_after: MScene::PlayersVs,
        players_1p_bonus: None,
        players_1p_bonus_fighters: None,
        players_1p_bonus_sprites: None,
        bonus_saved: ssb_game::players_1p_bonus::Saved {
            player: 0,
            bonus_fkind: None,
            bonus_costume: 0,
        },
        vs_results_fighters: None,
        fighter_select: None,
        fighter_select_fighters: None,
        vs_state: ssb_game::players_vs::BattleState::default(),
        players_vs: None,
        players_vs_fighters: None,
        players_1p: None,
        players_1p_fighters: None,
        one_p_scene: ssb_game::players_1p::SceneData::default(),
        spgame_scene: ssb_game::spgame::SceneData::default(),
        pack_path: loaded.as_ref().ok().map(|(_, p)| *p),
        menus: menus_screen::Menus::new(),
        // `lbBackupApplyOptions`.
        sound_quality: backup.sound_mono_or_stereo,
        video_offsets: (backup.screen_adjust_h, backup.screen_adjust_v),
        backup,
        save,
        campaign: None,
        stage_select_layer: None,
        stage_preview: stage_screen::start(),
        wallpaper: ssb_game::wallpaper::Wallpaper::make(ssb_game::wallpaper::Kind::Static),
        wallpaper_sprite: None,
        lead: play::Lead::Pad,
        demo: None,
        opening: alloc::boxed::Box::new(opening_screen::Opening::new()),
        opening_pending: None,
        title_pending: false,
    });
    // Stage MObj material joints are process-lifetime clocks in the original
    // layer setup. Start once with this pack and advance in the same simulation
    // branch as the stage/fighter tick; draw only reads the resulting state.
    if let Some(p) = pack.as_ref() {
        s.material_anim.start(p);
    }
    // `scManagerRunLoop`'s first scene: the N64 logo (`mnStartup`), then
    // the opening movie and the title (RE-467). A capture takes its
    // scripted route.
    if capture_scene.is_none() {
        go_scene(&mut s, pack.as_ref(), MScene::Startup, MScene::Startup, false);
    }
    // Built on each Training entry for the stage picked (`enter_training`).
    // `gSCManagerSceneData.gkind` and `s.maps_training_gkind`, both
    // `nGRKindCastle` in `dSCManagerDefaultSceneData`.

    let draw_assets = pack.as_ref().map(DrawAssets::resolve).unwrap_or_default();
    // On the heap for the same reason (`EffectVisuals::new_boxed`).
    let mut effect_visuals = EffectVisuals::new_boxed();
    let mut draw_state = meshdraw::DrawState::default();
    // Created once, on first entry to Training Mode (below) -- a fighter
    // spawned on the training stage, ticked with real physics/animation/
    // camera every frame this s.screen is active (`play::FighterScene`, shared
    // with `psp-asset-viewer/` via `ssb_psp_runtime::scene`).
    // The stationary dummy target (`play::Dummy`, `psp-game`-only -- see its
    // doc comment): spawned alongside `s.play_state` at the stage's second
    // spawn point, ticked with permanently neutral input.
    // Match-owned spawned s.weapons. Fighter statuses emit portable requests;
    // Training owns the pool because it is the layer that has every fighter
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
        let mut controller = controller;
        if capture_spec == Some(GameScene::SaveUnlock) && sim_frame_index == SAVE_UNLOCK_TICK {
            controller.buttons = N64Buttons(N64Buttons::A);
        }
        let pressed = newly_pressed(previous_controller.buttons, controller.buttons);

        if !deterministic_capture_frozen(capture_spec, sim_frame_index) {
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
        save::persist(&mut s.save, &s.backup);

        #[cfg(feature = "headless_capture")]
        if sim_frame_index == 105 {
            if let Some(scene) = capture_scene {
                campaign::capture_fixture(&mut s, scene);
            }
        }
        #[cfg(feature = "headless_capture")]
        if let Some(scene) = capture_scene {
            campaign::capture_objectives(&mut s, pack.as_ref(), scene, sim_frame_index);
        }

        // RE-429: a diagnostic camera makes the otherwise offscreen Bumper
        // visible without moving it or the fighters.
        if matches!(capture_scene, Some(GameScene::TrainingBumper | GameScene::TrainingPlants)) {
            if let (Some(pl), Some(item)) = (
                s.play_state.as_mut(),
                s.items
                    .items()
                    .find(|i| i.kind == if capture_scene == Some(GameScene::TrainingBumper) { ssb_game::item::ItemKind::GBumper } else { ssb_game::item::ItemKind::Pakkun }),
            ) {
                let at = if item.kind == ssb_game::item::ItemKind::Pakkun {
                    item.vars.pakkun_pos + ssb_engine::math::Vec3::new(0.0, 400.0, 0.0)
                } else { item.pos };
                pl.camera.at = at;
                pl.camera.eye = at + ssb_engine::math::Vec3::new(0.0, 0.0, 5000.0);
            }
        }

        if capture_monster(capture_scene).is_some() {
            if let Some(pl) = s.play_state.as_mut() {
                if let ssb_game::stage::Controller::Yamabuki(gate) = &s.stage_ctl.controller {
                    let at = gate.monster_pos + ssb_engine::math::Vec3::new(-600.0, 0.0, 0.0);
                    pl.camera.at = at;
                    pl.camera.eye = at + ssb_engine::math::Vec3::new(0.0, 0.0, 5000.0);
                }
            }
        }

        draw_frame(
            &mut gpu,
            &mut s,
            &pack,
            &mut draw_state,
            &mut effect_visuals,
            &draw_assets,
            no_pack_color,
        );
        gpu.end_frame();

        #[cfg(feature = "headless_capture")]
        if !headless_capture_sent && deterministic_capture_frozen(capture_spec, sim_frame_index) {
            emit_headless_screenshot();
            if matches!(capture_scene, Some(GameScene::OnePGame | GameScene::OnePBonus | GameScene::OnePTargetClear | GameScene::OnePTargetFall | GameScene::OnePPlatforms | GameScene::OnePPlatformClear | GameScene::OnePPlatformFall | GameScene::OnePRace | GameScene::OnePRaceClear | GameScene::OnePRaceFall | GameScene::OnePRaceHazards | GameScene::OnePBoss | GameScene::OnePBossDefeat | GameScene::OnePMetal | GameScene::OnePGiant | GameScene::OnePZako | GameScene::OnePEnding | GameScene::OnePStaffroll | GameScene::OnePCongra | GameScene::OnePChallenger | GameScene::OnePMessage | GameScene::OnePFinale)) {
                campaign::log_capture(&s, sim_frame_index);
            }
            // One line for the capture log: whether the scripted attack
            // landed is not always visible (RE-351).
            if let (Some(dummy), Some(player)) = (s.dummies[0].as_deref(), s.play_state.as_ref()) {
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

/// `gSCManagerSceneData` and `gSCManagerTransferBattleState`'s selections,
/// for `lbBackupCorrectErrors`.
fn selections(s: &Session) -> ssb_game::backup::Selections {
    use ssb_game::backup::{SelectedPlayer, Selections};
    Selections {
        fkind: s.one_p_scene.kind,
        training_man_fkind: s.training_scene.man_kind,
        training_com_fkind: s.training_scene.com_kind,
        players: s.vs_state.players.map(|p| SelectedPlayer {
            fkind: p.fkind,
            is_man: p.pkind == ssb_game::players_vs::PlayerKind::Man,
        }),
        maps_vsmode_gkind: s.maps_vsmode_gkind,
        maps_training_gkind: s.maps_training_gkind,
        items_reset: false,
    }
}

/// The corrected selections back into the session.
fn apply_selections(s: &mut Session, c: ssb_game::backup::Selections) {
    s.one_p_scene.kind = c.fkind;
    s.training_scene.man_kind = c.training_man_fkind;
    s.training_scene.com_kind = c.training_com_fkind;
    for (p, c) in s.vs_state.players.iter_mut().zip(c.players) {
        p.fkind = c.fkind;
        if c.is_man {
            p.pkind = ssb_game::players_vs::PlayerKind::Man;
        }
    }
    s.maps_vsmode_gkind = c.maps_vsmode_gkind;
    s.maps_training_gkind = c.maps_training_gkind;
}

/// The menu scene a capture starts in, and the scene before it.
fn capture_menu(scene: GameScene) -> Option<(ssb_game::menu::Scene, ssb_game::menu::Scene)> {
    use ssb_game::menu::Scene;
    Some(match scene {
        GameScene::Option => (Scene::Option, Scene::ModeSelect),
        GameScene::ScreenAdjust => (Scene::ScreenAdjust, Scene::Option),
        GameScene::BackupClear => (Scene::BackupClear, Scene::Option),
        GameScene::DataMenu => (Scene::Data, Scene::ModeSelect),
        GameScene::VsRecord | GameScene::Characters => (Scene::Data, Scene::ModeSelect),
        GameScene::Title => (Scene::Title, Scene::Startup),
        GameScene::Explain => (Scene::Explain, Scene::Title),
        GameScene::AutoDemo => (Scene::AutoDemo, Scene::Characters),
        GameScene::Opening => (Scene::Startup, Scene::Startup),
        GameScene::ModeSelect => (Scene::ModeSelect, Scene::Title),
        GameScene::OnePMode => (Scene::OnePMode, Scene::ModeSelect),
        GameScene::VsOptions => (Scene::VsOptions, Scene::VsMode),
        GameScene::ItemSwitch => (Scene::VsItemSwitch, Scene::VsOptions),
        GameScene::BonusSelect | GameScene::BonusPractice => (Scene::Players1PBonus1, Scene::OnePMode),
        g => match opening_capture_kind(g) {
            Some(kind) => (Scene::Opening(kind), Scene::Startup),
            None => return None,
        },
    })
}

/// The opening scene an `op-*` capture starts at once (RE-467).
fn opening_capture_kind(scene: GameScene) -> Option<ssb_game::opening::Kind> {
    use ssb_game::opening::Kind;
    Some(match scene {
        GameScene::OpeningRoom => Kind::Room,
        GameScene::OpeningPortraits => Kind::Portraits,
        GameScene::OpeningMario => Kind::Mario,
        GameScene::OpeningDonkey => Kind::Donkey,
        GameScene::OpeningLink => Kind::Link,
        GameScene::OpeningSamus => Kind::Samus,
        GameScene::OpeningYoshi => Kind::Yoshi,
        GameScene::OpeningKirby => Kind::Kirby,
        GameScene::OpeningFox => Kind::Fox,
        GameScene::OpeningPikachu => Kind::Pikachu,
        GameScene::OpeningRun => Kind::Run,
        GameScene::OpeningCliff => Kind::Cliff,
        GameScene::OpeningYamabuki => Kind::Yamabuki,
        GameScene::OpeningJungle => Kind::Jungle,
        GameScene::OpeningYoster => Kind::Yoster,
        GameScene::OpeningSector => Kind::Sector,
        GameScene::OpeningStandoff => Kind::Standoff,
        GameScene::OpeningClash => Kind::Clash,
        GameScene::OpeningNewcomers => Kind::Newcomers,
        _ => return None,
    })
}

/// The options and data menus' host state.
macro_rules! menus_host {
    ($s:expr, $pack:expr, $selections:expr, $capture:expr) => {
        menus_screen::Host {
            pack: $pack,
            pack_path: $s.pack_path,
            backup: &mut $s.backup,
            selections: $selections,
            sound_quality: &mut $s.sound_quality,
            video_offsets: &mut $s.video_offsets,
            vs_state: &mut $s.vs_state,
            capture: $capture,
        }
    };
}

/// `mnModeSelect`'s Option or Data: the menus' first scene.
#[inline(never)]
fn enter_menus(s: &mut Session, pack: Option<&Pack<'_>>, scene: ssb_game::menu::Scene, prev: ssb_game::menu::Scene, capture: bool) {
    let mut selections = selections(s);
    let mut menus = core::mem::replace(&mut s.menus, menus_screen::Menus::new());
    menus.enter(scene, prev, &mut menus_host!(s, pack, &mut selections, capture));
    s.menus = menus;
    s.screen = Screen::Menus;
}

/// One frame of the options and data menus.
#[inline(never)]
fn menus_frame(s: &mut Session, pack: Option<&Pack<'_>>, controller: ControllerState, pressed: N64Buttons, capture: bool) {
    let pad = ssb_game::menu::Pad {
        hold: controller.buttons.0,
        tap: pressed.0,
        stick_x: controller.stick_x,
        stick_y: controller.stick_y,
    };
    let mut selections = selections(s);
    let mut menus = core::mem::replace(&mut s.menus, menus_screen::Menus::new());
    let exit = menus.frame(&pad, &mut menus_host!(s, pack, &mut selections, capture));
    s.menus = menus;
    apply_selections(s, selections);
    if let Some(scene) = exit {
        let prev = s.menus.scene_prev;
        start_scene(s, pack, scene, prev, capture);
    }
}

/// `syTaskmanSetLoadScene` from a host scene: the menus take their own
/// scenes, the host the rest.
#[inline(never)]
fn go_scene(s: &mut Session, pack: Option<&Pack<'_>>, scene: MScene, prev: MScene, capture: bool) {
    // `mnTitleFuncStart` after the newcomers waits for the music's tic
    // (RE-467).
    if scene == MScene::Title
        && prev == MScene::Opening(ssb_game::opening::Kind::Newcomers)
        && s.opening.clock.tic < ssb_game::opening::TITLE_START_TIC
    {
        s.title_pending = true;
        return;
    }
    // The opening's models stay loaded across its scenes (RE-467).
    ssb_psp_runtime::movie::hold_models(s.pack_path, matches!(scene, MScene::Opening(_)));
    if menus_screen::Menus::is_menu(scene) || matches!(scene, MScene::Explain | MScene::AutoDemo | MScene::SoundTest) {
        let mut selections = selections(s);
        let mut menus = core::mem::replace(&mut s.menus, menus_screen::Menus::new());
        let next = menus.go(scene, prev, &mut menus_host!(s, pack, &mut selections, capture));
        s.menus = menus;
        apply_selections(s, selections);
        match next {
            Some(scene) => start_scene(s, pack, scene, prev, capture),
            None => s.screen = Screen::Menus,
        }
    } else {
        start_scene(s, pack, scene, prev, capture);
    }
}

/// Starts a scene the host runs, entered from `prev`.
#[inline(never)]
fn start_scene(s: &mut Session, pack: Option<&Pack<'_>>, scene: MScene, prev: MScene, capture: bool) {
    ssb_psp_runtime::movie::hold_models(s.pack_path, matches!(scene, MScene::Opening(_)));
    s.menus.scene = scene;
    s.menus.scene_prev = prev;
    match scene {
        MScene::Players1PGame => {
            s.players_1p = Some(ssb_game::players_1p::Players1P::new(s.one_p_scene, &s.backup));
            s.players_1p_fighters = None;
            s.screen = Screen::Players1P;
        }
        MScene::Players1PTraining => {
            // Training always starts over from its select.
            s.play_state = None;
            s.dummies = Default::default();
            s.vs = false;
            s.fighter_select = Some(new_fighter_select(s.training_scene, s.backup.fighter_mask, capture, 0));
            s.fighter_select_fighters = None;
            s.screen = Screen::FighterSelect;
        }
        MScene::Players1PBonus1 | MScene::Players1PBonus2 => {
            use ssb_game::players_1p_bonus::{BonusKind, Players1PBonus, SceneData};
            let bonus = if scene == MScene::Players1PBonus1 { BonusKind::Targets } else { BonusKind::Platforms };
            let data = SceneData { player: s.bonus_saved.player, bonus };
            s.players_1p_bonus = Some(alloc::boxed::Box::new(Players1PBonus::new(data, &s.backup)));
            s.players_1p_bonus_fighters = None;
            s.players_1p_bonus_sprites = s
                .pack_path
                .and_then(|path| assets::load_menu_pack(path, ssb_rom::menu_pack::MenuScene::Players1PBonus).ok());
            s.screen = Screen::Players1PBonus;
        }
        MScene::PlayersVs => {
            // A VS battle always starts over.
            s.play_state = None;
            s.dummies = Default::default();
            s.vs = true;
            s.vs_menu_rules = VsRules::of(&s.vs_state);
            s.players_vs = Some(new_players_vs(s.vs_state, s.scene_gkind, &s.backup));
            s.players_vs_fighters = None;
            s.screen = Screen::PlayersVs;
        }
        // The title's demos (RE-465); without their data each is skipped
        // along its own exit.
        MScene::Explain => {
            if !demo_screen::start_explain(s, pack) {
                go_scene(s, pack, MScene::Characters, MScene::Explain, capture);
            }
        }
        MScene::AutoDemo => {
            if !demo_screen::start_auto_demo(s, pack) {
                go_scene(s, pack, MScene::Startup, MScene::AutoDemo, capture);
            }
        }
        // `mnStartup` and the opening movie (RE-467).
        MScene::Startup => {
            s.menus.leave();
            opening_screen::start(&mut s.opening, s.pack_path, None);
            s.screen = Screen::Opening;
        }
        MScene::Opening(kind) => start_opening(s, pack, kind, prev, capture),
        // Not reached yet: the menus run every other scene they name, and
        // the bonus practices' select is wired below.
        _ => go_scene(s, pack, MScene::ModeSelect, scene, capture),
    }
}


/// `syTaskmanSetLoadScene` into an opening scene (RE-467). A scene whose
/// start tic has not come yet holds the last picture until it has; a
/// scene not yet ported passes on along its own exit.
#[inline(never)]
fn start_opening(s: &mut Session, pack: Option<&Pack<'_>>, kind: ssb_game::opening::Kind, prev: MScene, capture: bool) {
    // `mvOpeningNewcomersFuncStart`'s unlocked characters.
    s.opening.fighter_mask = s.backup.fighter_mask;
    if kind == ssb_game::opening::Kind::Room {
        // `mvOpeningRoomFuncStart`'s `sySchedulerSetTicCount(0)`.
        s.opening.clock.reset();
    }
    // An `op-*` capture starts its scene at once, its clock at the
    // scene's start tic.
    if prev == MScene::Startup && s.screen == Screen::CaptureIntro {
        s.opening.clock.tic = kind.start_tic().unwrap_or(0);
    }
    if !s.opening.clock.ready(kind) {
        if s.screen == Screen::Opening && opening_screen::hold(&mut s.opening, kind) {
            return;
        }
        let movie = matches!(s.demo, Some(demo_screen::Demo::Movie(_)));
        if s.screen != Screen::Opening && !movie {
            // Nothing of the opening's to hold: black until the start.
            opening_screen::clear(&mut s.opening);
            s.menus.leave();
            s.screen = Screen::Opening;
        }
        s.opening_pending = Some(kind);
        return;
    }
    begin_opening(s, pack, kind, capture);
}

/// An opening scene's start, its tic come.
#[inline(never)]
fn begin_opening(s: &mut Session, pack: Option<&Pack<'_>>, kind: ssb_game::opening::Kind, capture: bool) {
    s.menus.scene = MScene::Opening(kind);
    if kind.is_battle() {
        if demo_screen::start_movie(s, pack, kind) {
            s.menus.leave();
            return;
        }
    } else if opening_screen::runs(Some(kind)) {
        if s.demo.is_some() {
            demo_screen::leave(s);
        }
        s.menus.leave();
        opening_screen::start(&mut s.opening, s.pack_path, Some(kind));
        s.screen = Screen::Opening;
        return;
    }
    if s.demo.is_some() {
        demo_screen::leave(s);
    }
    let next = kind.next().map_or(MScene::Title, MScene::Opening);
    go_scene(s, pack, next, MScene::Opening(kind), capture);
}

/// One frame of `mnPlayers1PTraining`: the select's tick, then its
/// fighters'. Out of [`run`] for branch range.
#[inline(never)]
fn fighter_select_frame(
    s: &mut Session,
    pack: Option<&Pack<'_>>,
    controller: ControllerState,
    pressed: N64Buttons,
) -> Option<ssb_game::fighter_select::Outcome> {
    let select = s.fighter_select.as_mut()?;
    let outcome = select.tick(controller, pressed);
    let fighters = s.fighter_select_fighters.get_or_insert_with(players_screen::start);
    players_screen::tick_training(pack, select, fighters);
    outcome
}

/// `mnPlayers1PTraining`'s frame over the black of
/// `mnPlayers1PTrainingFuncStart`'s default camera:
/// `players_screen` draws the select's sprites and fighters. Without a
/// pack it falls back to plain slots.
#[inline(never)]
unsafe fn draw_training_select(
    gpu: &mut Gpu,
    pack: Option<&Pack<'_>>,
    draw_state: &mut meshdraw::DrawState,
    select: Option<&ssb_game::fighter_select::FighterSelect>,
    fighters: Option<&players_screen::Fighters>,
) {
    gpu.set_viewport_fullscreen();
    let Some(select) = select else {
        gpu.begin_frame(Some(BG_MENU));
        return;
    };
    match pack {
        // The fighters' poses are made on the select's first tick; a draw
        // before it shows none.
        Some(p) => {
            gpu.begin_frame(Some(BG_RESULTS));
            players_screen::draw_training(gpu, p, draw_state, select, fighters);
        }
        None => {
            gpu.begin_frame(Some(BG_MENU));
            draw_fighter_select(gpu, select);
        }
    }
}

/// The character select from the last scene data. Captures read the frame
/// counter for the CPU's random fighter, so they stay deterministic.
#[inline(never)]
fn new_fighter_select(
    scene: ssb_game::fighter_select::SceneData,
    fighter_mask: u16,
    capture: bool,
    frame: u64,
) -> ssb_game::fighter_select::FighterSelect {
    let time_byte = frame as u8;
    ssb_game::fighter_select::FighterSelect::new(scene, fighter_mask, || {
        if capture { time_byte } else { clock_byte() }
    })
}



/// `mnPlayersVSStartScene` from the battle state, with one controller
/// plugged into port 1.
fn new_players_vs(
    state: ssb_game::players_vs::BattleState,
    gkind: u8,
    backup: &ssb_game::backup::Backup,
) -> ssb_game::players_vs::PlayersVs {
    ssb_game::players_vs::PlayersVs::new(
        state,
        ssb_game::players_vs::SceneContext {
            fighter_mask: backup.fighter_mask,
            unlock_mask: backup.unlock_mask,
            gkind,
        },
        [true, false, false, false],
    )
}

/// One frame of `mnPlayersVS` and where it leads. Out of [`run`] for
/// branch range.
#[inline(never)]
fn players_vs_frame(
    s: &mut Session,
    pack: &Option<Pack<'_>>,
    capture: bool,
    frame: u64,
    controller: ControllerState,
    pressed: N64Buttons,
) {
    use ssb_game::players_vs::{Outcome, Pad};
    let Some(select) = s.players_vs.as_mut() else {
        return;
    };
    let pads = [
        Pad {
            state: controller,
            taps: pressed,
        },
        Pad::default(),
        Pad::default(),
        Pad::default(),
    ];
    let time_byte = frame as u8;
    let outcome = select.tick(&pads, &mut || if capture { time_byte } else { clock_byte() });
    let fighters = s.players_vs_fighters.get_or_insert_with(players_screen::start);
    players_screen::tick(pack.as_ref(), select, fighters);
    let (state, next) = match outcome {
        None => return,
        Some(Outcome::Maps(state)) => (state, None),
        Some(Outcome::Battle { state, gkind }) => (state, Some(gkind)),
        Some(Outcome::VsMode(state)) => {
            s.vs_state = state;
            go_scene(s, pack.as_ref(), MScene::VsMode, MScene::PlayersVs, capture);
            return;
        }
        Some(Outcome::Title(state)) => {
            s.vs_state = state;
            go_scene(s, pack.as_ref(), MScene::Title, MScene::PlayersVs, capture);
            return;
        }
    };
    s.vs_state = state;
    s.vs_menu_rules = VsRules::of(&state);
    match next {
        // `nSCKindMaps`: the cursor starts on the stage VS picked last.
        None => {
            s.open_stage_select(s.maps_vsmode_gkind);
        }
        // `nSCKindVSBattle` on the random stage.
        Some(gkind) => {
            s.scene_gkind = gkind;
            s.enter(pack.as_ref(), gkind, vs_roster(&state), Some(s.vs_menu_rules));
            s.screen = Screen::Training;
        }
    }
}

/// One frame of `mnPlayers1PGame`: the select's tick, then its fighter's.
/// The select's way out saves its scene and backup data
/// (`mnPlayers1PGameSetSceneData`). Out of [`run`] for branch range.
#[inline(never)]
fn players_1p_frame(s: &mut Session, pack: Option<&Pack<'_>>, controller: ControllerState, pressed: N64Buttons) {
    use ssb_game::players_1p::Outcome;
    let Some(select) = s.players_1p.as_mut() else {
        return;
    };
    let outcome = select.tick(controller, pressed);
    let fighters = s.players_1p_fighters.get_or_insert_with(players_screen::start);
    players_screen::tick_1p(pack, select, fighters);
    let (saved, next) = match outcome {
        None => return,
        // `nSCKind1PGame`: the campaign (RE-450).
        Some(Outcome::Proceed(saved)) => (saved, None),
        Some(Outcome::Back(saved)) => (saved, Some(MScene::OnePMode)),
        Some(Outcome::Timeout(saved)) => (saved, Some(MScene::Title)),
    };
    saved.apply(&mut s.spgame_scene, &mut s.backup);
    s.one_p_scene = saved.scene;
    s.players_1p = None;
    s.players_1p_fighters = None;
    match next {
        Some(scene) => go_scene(s, pack, scene, MScene::Players1PGame, false),
        None => campaign::start(s, pack),
    }
}

/// One frame of the Bonus Practice select (`mnPlayers1PBonus`).
#[inline(never)]
fn players_1p_bonus_frame(s: &mut Session, pack: Option<&Pack<'_>>, controller: ControllerState, pressed: N64Buttons) {
    use ssb_game::players_1p_bonus::{BonusKind, Outcome};
    let Some(select) = s.players_1p_bonus.as_mut() else {
        return;
    };
    // `scene_curr` as the select was entered.
    let entered = s.menus.scene;
    let outcome = select.tick(controller, pressed);
    let fighters = s.players_1p_bonus_fighters.get_or_insert_with(players_screen::start);
    players_screen::tick_bonus(pack, select, fighters);
    let Some(outcome) = outcome else {
        return;
    };
    let of = |b: BonusKind| if b == BonusKind::Targets { MScene::Players1PBonus1 } else { MScene::Players1PBonus2 };
    let (saved, next, prev) = match outcome {
        Outcome::Proceed { saved, bonus } => (saved, None, of(bonus)),
        Outcome::Back { saved, bonus } => (saved, Some(MScene::OnePMode), of(bonus)),
        Outcome::Timeout(saved) => (saved, Some(MScene::Title), entered),
    };
    // `mnPlayers1PBonusSetSceneData`.
    s.bonus_saved = saved;
    s.players_1p_bonus = None;
    s.players_1p_bonus_fighters = None;
    s.players_1p_bonus_sprites = None;
    match (next, outcome) {
        (Some(scene), _) => go_scene(s, pack, scene, prev, false),
        (None, Outcome::Proceed { bonus, .. }) => {
            s.menus.scene = MScene::BonusStage;
            s.menus.scene_prev = prev;
            campaign::start_practice(s, pack, saved, bonus);
        }
        (None, _) => {}
    }
}

/// `mnPlayers1PBonus`'s frame over its default camera's black.
#[inline(never)]
unsafe fn draw_players_1p_bonus(gpu: &mut Gpu, pack: Option<&Pack<'_>>, draw_state: &mut meshdraw::DrawState, s: &Session) {
    gpu.set_viewport_fullscreen();
    gpu.begin_frame(Some(BG_RESULTS));
    if let (Some(p), Some(select)) = (pack, s.players_1p_bonus.as_deref()) {
        let menu = s.players_1p_bonus_sprites.as_ref().and_then(|b| Pack::open(b.as_slice()).ok());
        players_screen::draw_bonus(gpu, p, menu.as_ref(), draw_state, select, &s.backup, s.players_1p_bonus_fighters.as_deref());
    }
}

/// `mnPlayers1PGame`'s frame over the black of its default camera:
/// `players_screen` draws the select's sprites and fighter. Without a
/// pack it draws nothing over the menu colour.
#[inline(never)]
unsafe fn draw_players_1p(gpu: &mut Gpu, pack: Option<&Pack<'_>>, draw_state: &mut meshdraw::DrawState, s: &Session) {
    gpu.set_viewport_fullscreen();
    match (pack, s.players_1p.as_ref()) {
        (Some(p), Some(select)) => {
            gpu.begin_frame(Some(BG_RESULTS));
            players_screen::draw_1p(gpu, p, draw_state, select, &s.backup, s.players_1p_fighters.as_deref());
        }
        _ => gpu.begin_frame(Some(BG_MENU)),
    }
}

/// `mnPlayersVS`'s frame over the black of `mnPlayersVSFuncStart`'s
/// default camera (RE-411): `players_screen` draws the select's sprites
/// and fighters. Without a pack it falls back to plain slots.
#[inline(never)]
unsafe fn draw_players_vs(
    gpu: &mut Gpu,
    pack: Option<&Pack<'_>>,
    draw_state: &mut meshdraw::DrawState,
    select: Option<&ssb_game::players_vs::PlayersVs>,
    fighters: Option<&players_screen::Fighters>,
) {
    gpu.set_viewport_fullscreen();
    let Some(select) = select else {
        gpu.begin_frame(Some(BG_MENU));
        return;
    };
    match (pack, fighters) {
        (Some(p), Some(f)) => {
            gpu.begin_frame(Some(BG_RESULTS));
            players_screen::draw_all(gpu, p, draw_state, select, f);
        }
        _ => {
            gpu.begin_frame(Some(BG_MENU));
            draw_players_vs_slots(gpu, select);
        }
    }
}

/// Draws the VS character select as plain slots in N64 screen coordinates
/// scaled onto the PSP screen, for a run with no pack: the portrait grid
/// (locked portraits dimmed, a placed fighter's portrait lit), the
/// game-mode, time and back buttons along the top, the four player panels
/// with their HMN/CP/NA buttons (lit for a human, grey for a CPU, dark when
/// closed), the pucks and the cursors.
#[inline(never)]
fn draw_players_vs_slots(gpu: &mut Gpu, select: &ssb_game::players_vs::PlayersVs) {
    use ssb_game::fighter_select as fs;
    use ssb_game::players_vs::PlayerKind;
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
            .any(|s| s.is_fighter_selected && s.fkind == Some(*kind));
        let color = if fs::is_locked(*kind, select.fighter_mask()) {
            ENTRY_DISABLED
        } else if placed {
            ENTRY_SELECTED
        } else {
            ENTRY_ENABLED
        };
        rect(gpu, x + 1.0, y + 1.0, fs::PORTRAIT_WIDTH - 2.0, fs::PORTRAIT_HEIGHT - 2.0, color);
    }
    // `mnPlayersVSCheckGameModeInRange`, the time arrows and the back
    // button's boxes.
    let mode = if select.is_team_battle { ENTRY_SELECTED } else { ENTRY_ENABLED };
    rect(gpu, 27.0, 14.0, 110.0, 21.0, mode);
    rect(gpu, 140.0, 12.0, 20.0, 23.0, ENTRY_ENABLED);
    rect(gpu, 210.0, 12.0, 20.0, 23.0, ENTRY_ENABLED);
    rect(gpu, 244.0, 13.0, 48.0, 21.0, ENTRY_ENABLED);
    for (i, slot) in select.slots.iter().enumerate() {
        let x = (i * 69) as f32;
        let (panel, button) = match slot.pkind {
            PlayerKind::Man => (PUCK_PLAYER, ENTRY_SELECTED),
            PlayerKind::Com => (PUCK_CPU, ENTRY_ENABLED),
            PlayerKind::Not => (ENTRY_DISABLED, ENTRY_DISABLED),
        };
        rect(gpu, x + 22.0, 126.0, 64.0, 94.0, panel);
        // `mnPlayersVSCheckPlayerKindSelectInRange`.
        rect(gpu, x + 60.0, 127.0, 28.0, 18.0, button);
    }
    for (i, slot) in select.slots.iter().enumerate() {
        if select.puck_visible(i) {
            let color = if slot.pkind == PlayerKind::Man { PUCK_PLAYER } else { PUCK_CPU };
            let (x, y) = slot.puck;
            rect(gpu, x, y, fs::PUCK_WIDTH, fs::PUCK_HEIGHT, color);
        }
    }
    for slot in &select.slots {
        if let Some((x, y)) = slot.cursor {
            rect(gpu, x + 20.0, y, 10.0, 10.0, CURSOR_COLOR);
        }
    }
}


/// Draws the character select in N64 screen coordinates scaled onto the
/// PSP screen, for a run with no pack: the portrait grid (locked portraits
/// dimmed, a placed fighter's portrait lit), the pucks and the cursor.
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
        let color = if fs::is_locked(*kind, select.fighter_mask()) {
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

/// The select's presentation after its `mnMapsFuncRun` tick: the layer's
/// preview and camera, then the preview model's clocks (RE-419).
#[inline(never)]
fn tick_stage_select_layer(pack: Option<&Pack<'_>>, s: &mut Session) {
    if let Some(layer) = s.stage_select_layer.as_mut() {
        layer.tick(&s.stage_select);
        stage_screen::tick(pack, layer, &mut s.stage_preview);
    }
}

/// Pack descriptors `draw_training` needs every frame. Each lookup scans a
/// whole descriptor table, which cost the PSP-2000 about 12 ms per frame
/// when done per draw (RE-360), so `run` resolves them once.
#[derive(Default)]
struct DrawAssets {
    shadow_texture: Option<ssb_rom::pack::TextureDesc>,
    player_frame: Option<ssb_rom::texture::Rgba8>,
    /// `ssb_psp_runtime::scene::ENTRY_PARTS`' objects and animations.
    entry_effects: [Option<(ssb_rom::pack::ObjectDesc, ssb_rom::pack::AnimDesc)>; 14],
    /// Indexed by `MarioFireball::index`: Mario's palette, then Luigi's.
    fireball_meshes: [Option<ssb_rom::pack::MeshDesc>; 2],
    blaster_mesh: Option<ssb_rom::pack::MeshDesc>,
    reflector: Option<ssb_rom::pack::ObjectDesc>,
    /// Master Hand's bullet tree.
    boss_bullet: Option<ssb_rom::pack::ObjectDesc>,
    charge_shot_mesh: Option<ssb_rom::pack::MeshDesc>,
    /// Indexed by `SamusBomb::blink_palette`.
    bomb_meshes: [Option<ssb_rom::pack::MeshDesc>; 2],
    /// Link's Boomerang tree and its `anim_joints` spin.
    boomerang: Option<(ssb_rom::pack::ObjectDesc, ssb_rom::pack::AnimDesc)>,
    /// The Spin Attack swirl and its transform animation.
    spin_effect: Option<(ssb_rom::pack::ObjectDesc, ssb_rom::pack::AnimDesc)>,
    yoshi_egg_mesh: Option<ssb_rom::pack::MeshDesc>,
    yoshi_star_mesh: Option<ssb_rom::pack::MeshDesc>,
    arwing_laser_mesh: Option<ssb_rom::pack::MeshDesc>,
    monster_razor: Option<ssb_rom::pack::ObjectDesc>,
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
    /// Peach's Castle's Bumper: the NBumper tree `ITCommonData`'s
    /// `GBumperItemAttributes` names (file 86 + 0x7648). It has no scripts.
    gbumper_item: Option<ssb_rom::pack::ObjectDesc>,
    capsule_item: Option<ssb_rom::pack::ObjectDesc>,
    heavy_items: [Option<ssb_rom::pack::ObjectDesc>; 2],
    tarubomb_item: Option<ssb_rom::pack::ObjectDesc>,
    tarubomb_piece: Option<ssb_rom::pack::MeshDesc>,
    container_piece: Option<ssb_rom::pack::MeshDesc>,
    egg_item: Option<(ssb_rom::pack::ObjectDesc, ssb_rom::pack::AnimDesc)>,
    /// Tomato, Heart and Star: file 86 + 0xAB0, 0x1158, 0x1560 (RE-433).
    utility_items: [Option<ssb_rom::pack::ObjectDesc>; 3],
    /// Motion-Sensor Bomb, Bob-omb, Bumper, Shell and Poké Ball: file 86 +
    /// 0x39A0, 0x33F8, 0x7648, 0x5F88, 0x9430.
    throwable_items: [Option<ssb_rom::pack::ObjectDesc>; 5],
    equipment_items: [Option<ssb_rom::pack::ObjectDesc>; 7],
    ray_ammo: Option<ssb_rom::pack::MeshDesc>,
    /// The Green Shell's list under `palettes[1]`, keyed (86, 0x5578).
    green_shell: Option<ssb_rom::pack::MeshDesc>,
    /// The Shells' slide spin on their root (RE-442).
    shell_spin: Option<ssb_rom::pack::AnimDesc>,
    /// The Bob-omb's left-walking list (86 + 0x34C0, RE-442).
    bombhei_left: Option<ssb_rom::pack::MeshDesc>,
    /// The Poké Ball's closed ball under `llITCommonDataMBallMatAnimJoint`
    /// (RE-443).
    mball_ball: Option<ssb_rom::pack::MeshDesc>,
    /// The Bumper's root lists by `[attached][palette_id]`: the tree's own,
    /// lit (keyed by `palettes[1]`, 0x7238), then the attached Wait list
    /// under the Wait `MObjSub` (0x7A38) and lit (0x76D8) (RE-442).
    bumper_lit: Option<ssb_rom::pack::MeshDesc>,
    bumper_wait: [Option<ssb_rom::pack::MeshDesc>; 2],
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
    /// The KO blast and the respawn halo, with their transform animations
    /// (RE-412).
    dead_explode: Option<(ssb_rom::pack::ObjectDesc, ssb_rom::pack::AnimDesc)>,
    rebirth_halo: Option<(ssb_rom::pack::ObjectDesc, ssb_rom::pack::AnimDesc)>,
    /// The display effects' objects and transform animations (RE-415), by
    /// [`display_asset`].
    displays: [Option<(ssb_rom::pack::ObjectDesc, Option<ssb_rom::pack::AnimDesc>)>; 7],
    /// Pikachu's Thunder frames by `texture_id_curr` (RE-417).
    thunder_frames: [Option<ssb_rom::pack::MeshDesc>; 4],
    /// The Egg Lay egg and its Wait, Break and Throw animations (RE-417).
    egg_lay: Option<(ssb_rom::pack::ObjectDesc, [ssb_rom::pack::AnimDesc; 3])>,
    /// The Poké Ball Pokémon by kind (RE-435).
    mmonsters: [Option<MonsterAsset>; 13],
    /// Their weapons' trees: Onix's rock, Meowth's coin, Starmie's Swift,
    /// Clefairy's swarm (Clefairy's own tree), and the Hydro Pump, Smog and
    /// Beedrill swarm with their `anim_joints`.
    monster_rock: [Option<ssb_rom::pack::MeshDesc>; 3],
    monster_kabigon_fall: Option<ssb_rom::pack::MeshDesc>,
    monster_pippi_xlu: Option<ssb_rom::pack::MeshDesc>,
    monster_coin: Option<ssb_rom::pack::ObjectDesc>,
    monster_swift: Option<ssb_rom::pack::ObjectDesc>,
    monster_pippi_swarm: Option<ssb_rom::pack::ObjectDesc>,
    monster_hydro: Option<(ssb_rom::pack::ObjectDesc, ssb_rom::pack::AnimDesc)>,
    monster_smog: Option<(ssb_rom::pack::ObjectDesc, ssb_rom::pack::AnimDesc)>,
    monster_swarm: Option<ssb_rom::pack::ObjectDesc>,
}

/// One Poké Ball Pokémon's tree (`ssb_rom::mmonster::VISUALS`): its attack
/// display list, and its appear and status scripts.
#[derive(Clone, Copy)]
struct MonsterAsset {
    object: ssb_rom::pack::ObjectDesc,
    attack_mesh: Option<ssb_rom::pack::MeshDesc>,
    appear: Option<ssb_rom::pack::AnimDesc>,
    status: Option<ssb_rom::pack::AnimDesc>,
}

impl MonsterAsset {
    fn resolve(p: &Pack<'_>, kind: usize) -> Option<Self> {
        use ssb_rom::mmonster::{FILE, VISUALS};
        let v = VISUALS[kind];
        let object = ssb_psp_runtime::scene::object_keyed(p, (FILE, v.graph))?;
        let attack_mesh = v.attack_dl_graph.and_then(|g| {
            let o = ssb_psp_runtime::scene::object_keyed(p, (FILE, g))?;
            let node = p.node(o.first_node + 1)?;
            (node.mesh != ssb_rom::pack::NodeDesc::NO_MESH)
                .then(|| p.mesh(node.mesh))
                .flatten()
        });
        Some(MonsterAsset {
            object,
            attack_mesh,
            appear: p.item_anim(ssb_rom::pack::AnimDesc::ITEM_ANIM_MMONSTER_APPEAR + kind as u32),
            status: v.status.and_then(|_| {
                p.item_anim(ssb_rom::pack::AnimDesc::ITEM_ANIM_MMONSTER_STATUS + kind as u32)
            }),
        })
    }
}

impl DrawAssets {
    fn resolve(p: &Pack<'_>) -> Self {
        DrawAssets {
            shadow_texture: meshdraw::fighter_shadow_texture(p),
            player_frame: player_screen::frame_image(p),
            entry_effects: ssb_psp_runtime::scene::ENTRY_PARTS
                .map(|part| ssb_psp_runtime::scene::entry_part(p, &part)),
            fireball_meshes: ssb_psp_runtime::scene::fireball_meshes(p),
            blaster_mesh: ssb_psp_runtime::scene::fox_blaster_mesh(p),
            reflector: ssb_psp_runtime::scene::fox_reflector_object(p),
            boss_bullet: ssb_psp_runtime::scene::boss_bullet_object(p),
            charge_shot_mesh: ssb_psp_runtime::scene::samus_charge_shot_mesh(p),
            bomb_meshes: ssb_psp_runtime::scene::samus_bomb_meshes(p),
            boomerang: ssb_psp_runtime::scene::link_boomerang_object(p)
                .zip(p.weapon_anim(ssb_rom::pack::AnimDesc::WEAPON_ANIM_LINK_BOOMERANG)),
            spin_effect: ssb_psp_runtime::scene::link_spin_attack_effect(p)
                .and_then(|(object, slot)| Some((object, p.effect_anim(slot)?))),
            yoshi_egg_mesh: ssb_psp_runtime::scene::yoshi_egg_mesh(p),
            yoshi_star_mesh: ssb_psp_runtime::scene::yoshi_star_mesh(p),
            arwing_laser_mesh: ssb_psp_runtime::scene::arwing_laser_mesh(p),
            monster_razor: ssb_psp_runtime::scene::object_keyed(p, (159, 0x2A50)),
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
            capsule_item: ssb_psp_runtime::scene::object_keyed(p, (86, 0x670)),
            heavy_items: [0x6778, 0x71A8]
                .map(|offset| ssb_psp_runtime::scene::object_keyed(p, (86, offset))),
            tarubomb_item: ssb_psp_runtime::scene::object_keyed(p, ssb_rom::ground_obj::TARUBOMB_SOURCE),
            tarubomb_piece: mesh_keyed(p, (ssb_rom::ground_obj::BONUS3_NODES_FILE, 0x8A0)),
            container_piece: (0..p.mesh_count())
                .filter_map(|i| p.mesh(i))
                .find(|m| m.source_file == 86 && m.source_offset == 0x68F0),
            egg_item: ssb_psp_runtime::scene::object_keyed(p, (86, 0x104A0))
                .zip(p.item_anim(ssb_rom::pack::AnimDesc::ITEM_ANIM_EGG)),
            utility_items: [0xAB0, 0x1158, 0x1560]
                .map(|offset| ssb_psp_runtime::scene::object_keyed(p, (86, offset))),
            throwable_items: [0x39A0, 0x33F8, 0x7648, 0x5F88, 0x9430]
                .map(|offset| ssb_psp_runtime::scene::object_keyed(p, (86, offset))),
            equipment_items: [0x1918, 0x1E00, 0x2198, 0x4B60, 0x3F50, 0x46B0, 0x2750]
                .map(|offset| ssb_psp_runtime::scene::object_keyed(p, (86, offset))),
            ray_ammo: (0..p.mesh_count())
                .filter_map(|i| p.mesh(i))
                .find(|m| m.source_file == 251 && m.source_offset == 0x2B0),
            green_shell: mesh_keyed(p, (86, 0x5578)),
            shell_spin: p.item_anim(ssb_rom::pack::AnimDesc::ITEM_ANIM_SHELL_SPIN),
            bombhei_left: mesh_keyed(p, (86, 0x34C0)),
            mball_ball: mesh_keyed(p, (86, 0x9520)),
            bumper_lit: mesh_keyed(p, (86, 0x7238)),
            bumper_wait: [0x7A38, 0x76D8].map(|offset| mesh_keyed(p, (86, offset))),
            gbumper_item: ssb_psp_runtime::scene::object_keyed(
                p,
                ssb_psp_runtime::scene::GBUMPER_ITEM_SOURCE,
            ),
            link_bomb_item: ssb_psp_runtime::scene::object_keyed(
                p,
                ssb_psp_runtime::scene::LINK_BOMB_ITEM_SOURCE,
            )
            .zip(p.item_anim(ssb_rom::pack::AnimDesc::ITEM_ANIM_LINK_BOMB)),
            shield: ssb_psp_runtime::scene::object_keyed(
                p,
                ssb_psp_runtime::scene::SHIELD_EFFECT_KEY,
            ),
            magnet: ssb_psp_runtime::scene::ness_psi_magnet_effect(p)
                .and_then(|(object, slot)| Some((object, p.effect_anim(slot)?))),
            sing: ssb_psp_runtime::scene::purin_sing_effect(p)
                .and_then(|(object, slot)| Some((object, p.effect_anim(slot)?))),
            cutter: ssb_psp_runtime::scene::kirby_cutter_object(p)
                .zip(p.weapon_anim(ssb_rom::pack::AnimDesc::WEAPON_ANIM_KIRBY_CUTTER)),
            falcon_kick: ssb_psp_runtime::scene::captain_falcon_kick_effect(p)
                .and_then(|(object, slot)| Some((object, p.effect_anim(slot)?))),
            dead_explode: ssb_psp_runtime::scene::manager_effect(
                p,
                ssb_psp_runtime::scene::DEAD_EXPLODE_EFFECT_KEY,
            ),
            rebirth_halo: ssb_psp_runtime::scene::manager_effect(
                p,
                ssb_psp_runtime::scene::REBIRTH_HALO_EFFECT_KEY,
            ),
            displays: display_assets(p),
            thunder_frames: ssb_psp_runtime::scene::pikachu_thunder_meshes(p),
            egg_lay: ssb_psp_runtime::scene::yoshi_egg_lay_effect(p),
            mmonsters: core::array::from_fn(|kind| MonsterAsset::resolve(p, kind)),
            monster_rock: ssb_rom::mmonster::ROCK_TEXTURE_KEYS.map(|key| mesh_keyed(p, (86, key))),
            monster_kabigon_fall: mesh_keyed(p, (86, ssb_rom::mmonster::KABIGON_FALL_MESH)),
            monster_pippi_xlu: mesh_keyed(p, (86, ssb_rom::mmonster::PIPPI_XLU_MESH)),
            monster_coin: ssb_psp_runtime::scene::object_keyed(
                p,
                (86, ssb_rom::mmonster::COIN_GRAPH),
            ),
            monster_swift: ssb_psp_runtime::scene::object_keyed(
                p,
                (86, ssb_rom::mmonster::SWIFT_GRAPH),
            ),
            monster_pippi_swarm: ssb_psp_runtime::scene::object_keyed(
                p,
                (86, ssb_rom::mmonster::PIPPI_SWARM_GRAPH),
            ),
            monster_hydro: ssb_psp_runtime::scene::object_keyed(
                p,
                (86, ssb_rom::mmonster::HYDRO_GRAPH),
            )
            .zip(p.weapon_anim(ssb_rom::pack::AnimDesc::WEAPON_ANIM_KAMEX_HYDRO)),
            monster_smog: ssb_psp_runtime::scene::object_keyed(
                p,
                (86, ssb_rom::mmonster::SMOG_GRAPH),
            )
            .zip(p.weapon_anim(ssb_rom::pack::AnimDesc::WEAPON_ANIM_DOGAS_SMOG)),
            monster_swarm: ssb_psp_runtime::scene::object_keyed(
                p,
                (86, ssb_rom::mmonster::SPEAR_SWARM_GRAPH),
            ),
        }
    }
}

/// `DObjGetStruct(weapon_gobj)->scale = 0.5` in
/// `wpPikachuThunderHeadMakeWeapon` and `wpPikachuThunderTrailMakeWeapon`,
/// and the segment's in `efManagerPikachuThunderTrailMakeEffect`.
const THUNDER_SCALE: f32 = 0.5;
/// `mobj->texture_id_curr = 3` in `wpPikachuThunderHeadMakeWeapon`.
const THUNDER_HEAD_FRAME: u8 = 3;

/// `efManagerPikachuThunderTrailProcDisplay` (RE-417): each fading segment
/// is a `TraRotRpyRSca` DObj at its translation, turned 180 degrees on its
/// last frame and scaled by half, drawing `sprites[texture_id_curr]` under
/// `G_RM_AA_XLU_SURF` with no alpha compare. Its frame is the only thing
/// that changes: the source sets no colour or alpha as it fades.
#[inline(never)]
unsafe fn draw_thunder_segments(
    gpu: &mut Gpu,
    p: &Pack<'_>,
    draw_state: &mut meshdraw::DrawState,
    assets: &DrawAssets,
    effects: &ssb_game::effect::Effects,
) {
    for d in effects
        .displays()
        .filter(|d| d.kind == ssb_game::effect::DisplayKind::ThunderTrail)
    {
        let Some(mesh) = assets.thunder_frames.get(usize::from(d.index)).and_then(Option::as_ref) else {
            continue;
        };
        gpu.model_transform_xyz(
            [d.translate.x, d.translate.y, d.translate.z],
            [d.rotate.x, d.rotate.y, d.rotate.z],
            [
                meshdraw::MODEL_SCALE * d.scale.x,
                meshdraw::MODEL_SCALE * d.scale.y,
                meshdraw::MODEL_SCALE * d.scale.z,
            ],
        );
        meshdraw::draw_mesh(p, mesh, draw_state, None, None);
    }
}

/// The display effects' `EFDesc` objects (RE-415): the slash
/// (`llEFCommonEffects1DamageSlashDObjDesc`), the flying orbs, the impact
/// wave, the common spark (the flying sparks and the Star Rod spark), the
/// metal dust, the small shock (`llEFCommonEffects2ShockSmallDObjDesc`) and
/// the Poké Ball's rays (`llEFCommonEffects3MBallRaysDObjDesc`, RE-443).
const DISPLAY_EFFECT_KEYS: [(u32, u32); 7] = [
    (83, 0x7750),
    (83, 0x7E80),
    (83, 0x7C28),
    (83, 0x8FA0),
    (83, 0xCAC8),
    (84, 0x1500),
    (85, 0x0628),
];

/// [`DISPLAY_EFFECT_KEYS`]' objects and transform animations.
#[inline(never)]
fn display_assets(
    p: &Pack<'_>,
) -> [Option<(ssb_rom::pack::ObjectDesc, Option<ssb_rom::pack::AnimDesc>)>; 7] {
    DISPLAY_EFFECT_KEYS.map(|key| {
        let slot = ssb_rom::effect::MANAGER_EFFECT_KEYS
            .iter()
            .position(|&k| k == key)?;
        Some((
            ssb_psp_runtime::scene::object_keyed(p, key)?,
            p.effect_anim(slot as u32),
        ))
    })
}

/// A display effect's slot in [`DISPLAY_EFFECT_KEYS`], or `None` for one
/// that draws nothing.
fn display_asset(kind: ssb_game::effect::DisplayKind) -> Option<usize> {
    use ssb_game::effect::DisplayKind as K;
    Some(match kind {
        K::Slash => 0,
        K::FlyOrbs => 1,
        K::ImpactWave => 2,
        K::FlySparks | K::StarRodSpark => 3,
        K::FlyMDust => 4,
        K::ShockSmall => 5,
        K::MBallRays => 6,
        K::SpawnOrbs
        | K::SpawnSparks
        | K::SpawnMDust
        | K::Quake { .. }
        | K::FireSpark
        | K::ThunderTrail
        | K::YoshiEggEscape
        | K::ContainerSmash => return None,
    })
}

/// A display effect's players, replayed from the start to its clock each
/// time it is drawn (RE-415): one set on the heap serves every effect.
struct DisplayScratch {
    anim: ssb_rom::skeleton::StageAnimator,
    materials: ssb_rom::skeleton::EffectMaterialAnimator,
}

/// [`DisplayScratch`], built in place on the heap.
#[inline(never)]
fn new_display_scratch() -> alloc::boxed::Box<DisplayScratch> {
    use core::ptr;
    let mut b = alloc::boxed::Box::<DisplayScratch>::new_uninit();
    let p = b.as_mut_ptr();
    // SAFETY: both fields are written before `assume_init`.
    unsafe {
        ptr::addr_of_mut!((*p).anim).write(Default::default());
        ptr::addr_of_mut!((*p).materials).write(Default::default());
        b.assume_init()
    }
}

/// `func_80010918(.., TRUE)` (matrix kind 40): the effect's `DObj` turned to
/// face the camera's eye, at its translation. Its X axis is level, its Z
/// axis points back along the line from the eye.
fn eye_billboard(pos: ssb_engine::math::Vec3, eye: ssb_engine::math::Vec3) -> ssb_rom::scene::Mat4 {
    let mut d = pos - eye;
    let len = d.length();
    if len > 0.0 {
        d = d * (1.0 / len);
    }
    let r = ssb_engine::math::sqrt(d.x * d.x + d.z * d.z);
    let (x, y, z) = if r != 0.0 {
        let inv = 1.0 / r;
        (
            [-d.z * inv, 0.0, d.x * inv],
            [-d.y * d.x * inv, r, -d.y * d.z * inv],
            [-d.x, -d.y, -d.z],
        )
    } else {
        ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0])
    };
    ssb_rom::scene::Mat4([
        x[0], x[1], x[2], 0.0, //
        y[0], y[1], y[2], 0.0, //
        z[0], z[1], z[2], 0.0, //
        pos.x, pos.y, pos.z, 1.0,
    ])
}

fn psp_matrix(m: &ssb_rom::scene::Mat4) -> psp::sys::ScePspFMatrix4 {
    let v = |c: usize| psp::sys::ScePspFVector4 {
        x: m.0[c * 4],
        y: m.0[c * 4 + 1],
        z: m.0[c * 4 + 2],
        w: m.0[c * 4 + 3],
    };
    psp::sys::ScePspFMatrix4 {
        x: v(0),
        y: v(1),
        z: v(2),
        w: v(3),
    }
}

/// Draws the display effects (RE-415) through the battle camera: each
/// replays its animations to its clock, then draws under its `EFDesc`'s
/// matrix kinds. The root of all but the impact wave is kind 40
/// ([`eye_billboard`]) with its second kind (`RotSca` 0x45, `Sca` or
/// `RotRpyR`); a flag-1 desc's own `DObj` hangs under it with its kind
/// (`Sca`, `RotSca` 0x45 or the scale-and-translate 0x44). The impact wave
/// is a `TraRotRpyRSca` `DObj` in its index's primitive colour. Only the
/// effects on display link `link` draw (RE-422).
#[inline(never)]
fn draw_display_effects(
    p: &Pack<'_>,
    draw_state: &mut meshdraw::DrawState,
    assets: &DrawAssets,
    scratch: &mut DisplayScratch,
    effects: &ssb_game::effect::Effects,
    eye: ssb_engine::math::Vec3,
    link: u8,
) {
    use ssb_game::effect::DisplayKind as K;
    use ssb_rom::scene::Mat4;
    let ms = meshdraw::MODEL_SCALE;
    for d in effects.displays().filter(|d| d.kind.dl_link() == link) {
        let Some((object, anim)) = display_asset(d.kind).and_then(|i| assets.displays[i].as_ref())
        else {
            continue;
        };
        match anim {
            Some(anim) => {
                scratch.anim.start(p, anim);
                if let Some(script) = p.anim_script(anim) {
                    for _ in 0..d.ticks {
                        let _ = scratch.anim.tick(script);
                    }
                }
            }
            None => scratch.anim = ssb_rom::skeleton::StageAnimator::new(),
        }
        scratch.materials.start(p, object_mat_anims(p, object));
        for _ in 0..d.ticks {
            scratch.materials.tick(p);
        }
        let pose = stage_pose(&scratch.anim, object.first_node);
        let rot = [d.rotate.x, d.rotate.y, d.rotate.z];
        let scale = [d.scale.x, d.scale.y, d.scale.z];
        let world = |x: [f32; 3]| x.map(|v| v / ms);
        let child = |kind: u8| -> Mat4 {
            let Some(pose) = pose else {
                return Mat4::IDENTITY;
            };
            match kind {
                // `Sca`.
                0 => Mat4::from_trs([0.0; 3], [0.0; 3], pose.scale),
                // 0x45 `lbCommonRotScaFuncMatrix`.
                1 => Mat4::from_trs([0.0; 3], pose.rotate, pose.scale),
                // 0x44 `func_ovl0_800CA024`: scale and translate.
                _ => Mat4::from_trs(world(pose.translate), [0.0; 3], pose.scale),
            }
        };
        let units = Mat4::from_trs([0.0; 3], [0.0; 3], [ms; 3]);
        if d.kind == K::MBallRays {
            draw_mball_rays(p, draw_state, scratch, object, d.translate);
            continue;
        }
        let bill = eye_billboard(d.translate, eye);
        let mut posed = [Mat4::IDENTITY; 8];
        let (root, n) = match d.kind {
            K::ImpactWave => {
                let s = pose.map_or([1.0; 3], |p| p.scale);
                let t = [d.translate.x, d.translate.y, d.translate.z];
                (Mat4::from_trs(t, rot, s).mul(&units), 1)
            }
            K::ShockSmall => (
                bill.mul(&Mat4::from_trs([0.0; 3], rot, scale)).mul(&units),
                1,
            ),
            K::Slash => {
                let n = scratch.anim.compose(p, object, &mut posed);
                (
                    bill.mul(&Mat4::from_trs([0.0; 3], rot, scale)).mul(&units),
                    n,
                )
            }
            K::FlyOrbs => {
                posed[0] = child(0);
                (
                    bill.mul(&Mat4::from_trs([0.0; 3], [0.0; 3], scale))
                        .mul(&units),
                    1,
                )
            }
            K::FlySparks => {
                posed[0] = child(1);
                (
                    bill.mul(&Mat4::from_trs([0.0; 3], rot, [1.0; 3]))
                        .mul(&units),
                    1,
                )
            }
            K::StarRodSpark => {
                posed[0] = child(1);
                (
                    bill.mul(&Mat4::from_trs([0.0; 3], rot, scale)).mul(&units),
                    1,
                )
            }
            K::FlyMDust => {
                posed[0] = child(2);
                (
                    bill.mul(&Mat4::from_trs([0.0; 3], rot, [1.0; 3]))
                        .mul(&units),
                    1,
                )
            }
            _ => continue,
        };
        let n = n.min(object.node_count as usize).max(1);
        if d.kind == K::ImpactWave {
            let [r, g, b] = ssb_game::effect::IMPACT_WAVE_PRIM[usize::from(d.index.min(4))];
            draw_state.color_override = Some(ssb_rom::skeleton::EffectColors {
                prim: Some([r, g, b, d.alpha as u8]),
                env: Some([0, 0, 0, 0xFF]),
                ..Default::default()
            });
        }
        let base = psp_matrix(&root);
        unsafe {
            meshdraw::draw_object_posed(
                p,
                object,
                &base,
                &posed[..n],
                None,
                draw_state,
                None,
                Some(&scratch.materials),
                0,
            );
        }
        draw_state.color_override = None;
    }
}

/// The opened Poké Ball's rays (RE-443), drawn as the entry's rays are
/// (RE-425): the root's translation is the ball's position (`dobj->translate
/// .vec.f = *pos`), under the scripts' pose, billboard scales and
/// visibility.
fn draw_mball_rays(
    p: &Pack<'_>,
    draw_state: &mut meshdraw::DrawState,
    scratch: &DisplayScratch,
    object: &ssb_rom::pack::ObjectDesc,
    pos: ssb_engine::math::Vec3,
) {
    use ssb_rom::scene::Mat4;
    let ms = meshdraw::MODEL_SCALE;
    let mut posed = [Mat4::IDENTITY; 8];
    let n = scratch.anim.compose(p, object, &mut posed);
    if let Some(root) = p.node(object.first_node) {
        let place = Mat4::from_trs(root.rest_translate.map(|x| -x / ms), [0.0; 3], [1.0; 3]);
        for m in &mut posed[..n] {
            *m = place.mul(m);
        }
    }
    let mut scales = [[1.0f32; 2]; 8];
    let count = scratch.anim.billboard_scales(p, object, &mut scales).min(n);
    let base = psp_matrix(&Mat4::from_trs([pos.x, pos.y, pos.z], [0.0; 3], [ms; 3]));
    let hidden = |node: u32| !scratch.anim.visible(p, node);
    unsafe {
        meshdraw::draw_object_posed_scaled_hiding(
            p,
            object,
            &base,
            &posed[..n],
            &scales[..count],
            draw_state,
            None,
            Some(&scratch.materials),
            &hidden,
        );
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
    /// Each port's entry effect parts (RE-403), made on the first
    /// [`Self::sync_entry`]: on the heap, since each part's players are
    /// some 25 KB and `run` keeps this on its stack.
    entry: alloc::vec::Vec<[EntryVisual; 2]>,
    /// Each port's KO blast and respawn halo (RE-412), made on the first
    /// [`Self::sync_ko`], on the heap for the same reason.
    ko: alloc::vec::Vec<KoVisual>,
    /// Each port's Egg Lay egg (RE-417), made on the first
    /// [`Self::sync_eggs`].
    eggs: alloc::vec::Vec<EggVisual>,
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

impl EffectVisuals {
    /// The players, built in place on the heap one field at a time: the
    /// whole is some 380 KB, past the main thread's 256 KB stack
    /// (`psp::module!`), and `Box::default` builds it on the stack first.
    #[inline(never)]
    fn new_boxed() -> alloc::boxed::Box<EffectVisuals> {
        use core::ptr;
        let mut b = alloc::boxed::Box::<EffectVisuals>::new_uninit();
        let p = b.as_mut_ptr();
        // SAFETY: every field is written exactly once below before
        // `assume_init`, and none is read before then.
        unsafe {
            ptr::addr_of_mut!((*p).entry).write(Default::default());
            ptr::addr_of_mut!((*p).ko).write(Default::default());
            ptr::addr_of_mut!((*p).eggs).write(Default::default());
            ptr::addr_of_mut!((*p).boomerang).write(Default::default());
            ptr::addr_of_mut!((*p).boomerang_ticks).write(Default::default());
            ptr::addr_of_mut!((*p).spin).write(Default::default());
            ptr::addr_of_mut!((*p).spin_materials).write(Default::default());
            ptr::addr_of_mut!((*p).spin_ticks).write(Default::default());
            ptr::addr_of_mut!((*p).punch_materials).write(Default::default());
            ptr::addr_of_mut!((*p).punch_ticks).write(Default::default());
            ptr::addr_of_mut!((*p).kick).write(Default::default());
            ptr::addr_of_mut!((*p).kick_materials).write(Default::default());
            ptr::addr_of_mut!((*p).kick_ticks).write(Default::default());
            ptr::addr_of_mut!((*p).cutter).write(Default::default());
            ptr::addr_of_mut!((*p).cutter_ticks).write(Default::default());
            for i in 0..MAX_JOLT_VISUALS {
                ptr::addr_of_mut!((*p).jolts[i]).write(Default::default());
            }
            ptr::addr_of_mut!((*p).sing).write(Default::default());
            ptr::addr_of_mut!((*p).sing_materials).write(Default::default());
            ptr::addr_of_mut!((*p).sing_ticks).write(Default::default());
            ptr::addr_of_mut!((*p).pk_fire_materials).write(Default::default());
            ptr::addr_of_mut!((*p).pk_fire_ticks).write(Default::default());
            ptr::addr_of_mut!((*p).pk_thunder).write(Default::default());
            ptr::addr_of_mut!((*p).pk_thunder_materials).write(Default::default());
            ptr::addr_of_mut!((*p).pk_thunder_ticks).write(Default::default());
            ptr::addr_of_mut!((*p).magnet).write(Default::default());
            ptr::addr_of_mut!((*p).magnet_materials).write(Default::default());
            ptr::addr_of_mut!((*p).magnet_ticks).write(Default::default());
            for i in 0..MAX_ITEM_VISUALS {
                ptr::addr_of_mut!((*p).items[i]).write(Default::default());
            }
            b.assume_init()
        }
    }
}

/// One visual for every live item in the shared pool, including containers.
const MAX_ITEM_VISUALS: usize = ssb_game::item::ITEM_ALLOC_MAX;

/// One item's players, keyed by its kind and restarted when its play count
/// goes back.
#[derive(Default)]
struct ItemVisual {
    kind: Option<ssb_game::item::ItemKind>,
    ticks: u16,
    anim: ssb_rom::skeleton::StageAnimator,
    materials: ssb_rom::skeleton::EffectMaterialAnimator,
    /// A Pokémon's: the status script it plays since that tick, and the
    /// script playing.
    status: Option<u16>,
    playing: Option<ssb_rom::pack::AnimDesc>,
    /// The root script ([`ssb_game::item::RootScript`]) these players
    /// follow, by the tick it was added, and whether it has been cleared.
    root_added: Option<u16>,
    root_cleared: bool,
}

impl DrawAssets {
    fn item(
        &self,
        kind: ssb_game::item::ItemKind,
    ) -> Option<&(ssb_rom::pack::ObjectDesc, ssb_rom::pack::AnimDesc)> {
        match kind {
            ssb_game::item::ItemKind::NessPKFire => self.pk_fire_item.as_ref(),
            ssb_game::item::ItemKind::LinkBomb => self.link_bomb_item.as_ref(),
            ssb_game::item::ItemKind::Container(ssb_game::item::container::Kind::Egg) => self.egg_item.as_ref(),
            ssb_game::item::ItemKind::Container(_)
            | ssb_game::item::ItemKind::Equipment(_)
            | ssb_game::item::ItemKind::Utility(_)
            | ssb_game::item::ItemKind::MSBomb
            | ssb_game::item::ItemKind::BombHei
            | ssb_game::item::ItemKind::NBumper
            | ssb_game::item::ItemKind::Shell(_)
            | ssb_game::item::ItemKind::MBall => None,
            // No scripts, or trees the stage's ground objects draw.
            ssb_game::item::ItemKind::GBumper
            | ssb_game::item::ItemKind::Target
            | ssb_game::item::ItemKind::PowerBlock
            | ssb_game::item::ItemKind::Pakkun
            | ssb_game::item::ItemKind::Monster(_) => None,
            // Static tree drawn with the container roots below.
            ssb_game::item::ItemKind::TaruBomb => None,
            // [`MonsterAsset`].
            ssb_game::item::ItemKind::MMonster(_) => None,
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

/// One entry effect part's players, the clock they have caught up to and
/// the DL link it draws on this frame (RE-425).
#[derive(Default)]
struct EntryVisual {
    ticks: Option<u16>,
    anim: ssb_rom::skeleton::StageAnimator,
    materials: ssb_rom::skeleton::EffectMaterialAnimator,
    link: u8,
}

/// A node's local z in an entry part's animation, as the sort helpers
/// read `dobj->translate.vec.f.z`.
fn entry_node_z(p: &Pack<'_>, object: &ssb_rom::pack::ObjectDesc, anim: &ssb_rom::skeleton::StageAnimator, node: usize) -> f32 {
    let global = object.first_node + node as u32;
    anim.node_pose(global)
        .map(|pose| pose.translate[2])
        .or_else(|| p.node(global).map(|n| n.rest_translate[2]))
        .unwrap_or(0.0)
}

/// `efManagerSortZNeg`.
fn sort_z_neg(z: f32) -> u8 {
    if z < -1000.0 {
        2
    } else {
        20
    }
}

/// `efManagerSortZPos`.
fn sort_z_pos(z: f32) -> u8 {
    if z > 1000.0 {
        2
    } else {
        20
    }
}

impl EffectVisuals {
    /// Plays each fighter's entry effect up to its clock
    /// (`Entry::effect_ticks`, or `rays_ticks` for the Poké Ball's rays),
    /// restarting on a new entry, and sorts each part onto its link as its
    /// update does (RE-425).
    #[inline(never)]
    fn sync_entry(&mut self, p: &Pack<'_>, assets: &DrawAssets, fighters: [Option<&ssb_game::fighter::Fighter>; 4]) {
        use ssb_psp_runtime::scene::{EntryClock, EntryLink, ENTRY_PARTS};
        if self.entry.len() < fighters.len() {
            self.entry.resize_with(fighters.len(), Default::default);
        }
        for (visuals, f) in self.entry.iter_mut().zip(fighters) {
            let parts = f.map_or(&[][..], ssb_psp_runtime::scene::entry_effect_parts);
            for (i, v) in visuals.iter_mut().enumerate() {
                let Some(&k) = parts.get(i) else {
                    v.ticks = None;
                    continue;
                };
                let part = &ENTRY_PARTS[k];
                let clock = f.and_then(|f| match part.clock {
                    EntryClock::Effect => f.entry.effect_ticks,
                    EntryClock::Rays => f.entry.rays_ticks,
                });
                let asset = assets.entry_effects[k].as_ref();
                let (Some((restart, ticks)), Some((object, anim))) = (catch_up(&mut v.ticks, clock), asset) else {
                    continue;
                };
                let turned = part.turns_left && f.is_some_and(|f| f.entry.lr == Some(ssb_game::fighter::Facing::Left));
                if restart {
                    v.anim.start(p, anim);
                    v.materials.start(p, object_mat_anims(p, object));
                    v.link = match part.link {
                        EntryLink::Fixed(link) => link,
                        // The maker's own sort, on the rest pose until the
                        // first play.
                        EntryLink::SortZNeg(n) | EntryLink::Ball(n) => sort_z_neg(entry_node_z(p, object, &v.anim, n)),
                        EntryLink::SortCar(n) if turned => sort_z_pos(entry_node_z(p, object, &v.anim, n)),
                        EntryLink::SortCar(n) => sort_z_neg(entry_node_z(p, object, &v.anim, n)),
                    };
                }
                let Some(script) = p.anim_script(anim) else {
                    continue;
                };
                for t in 0..ticks {
                    let first = restart && t == 0;
                    // `efManagerMBallThrownProcUpdate` sorts on the pose
                    // before it plays.
                    if let EntryLink::Ball(n) = part.link {
                        if !first {
                            v.link = if entry_node_z(p, object, &v.anim, n) > 1000.0 { 20 } else { 10 };
                        }
                    }
                    let _ = v.anim.tick(script);
                    v.materials.tick(p);
                    match part.link {
                        EntryLink::Ball(n) if first => v.link = sort_z_neg(entry_node_z(p, object, &v.anim, n)),
                        EntryLink::SortZNeg(n) => v.link = sort_z_neg(entry_node_z(p, object, &v.anim, n)),
                        EntryLink::SortCar(n) if turned => v.link = sort_z_pos(entry_node_z(p, object, &v.anim, n)),
                        EntryLink::SortCar(n) => v.link = sort_z_neg(entry_node_z(p, object, &v.anim, n)),
                        _ => {}
                    }
                }
            }
        }
    }
}

/// Whether an entry part has been ejected: its end node's script, or every
/// script, has ended.
fn entry_part_ended(part: &ssb_psp_runtime::scene::EntryPart, object: &ssb_rom::pack::ObjectDesc, v: &EntryVisual) -> bool {
    match part.end_node {
        Some(n) => v.anim.node_ended(object.first_node + n as u32).unwrap_or(true),
        None => v.anim.ended(),
    }
}

/// Draws each fighter's entry effect parts on DL link `link`: the root (or
/// the Arwing's outer `DObj`) takes the entry position (`dobj->translate
/// .vec.f = *pos`), the car turns for a leftward entry, the visibility
/// scripts hide their nodes, and a part is gone once its animation ends
/// (RE-403, RE-425).
#[inline(never)]
fn draw_entry_effects(
    gpu: &mut Gpu,
    p: &Pack<'_>,
    draw_state: &mut meshdraw::DrawState,
    assets: &DrawAssets,
    visuals: &EffectVisuals,
    fighters: [Option<&ssb_game::fighter::Fighter>; 4],
    material_anim: Option<&ssb_rom::skeleton::MaterialAnimator>,
    link: u8,
) {
    use ssb_psp_runtime::scene::{EntryClock, EntryRoot, ENTRY_PARTS};
    for (vs, f) in visuals.entry.iter().zip(fighters) {
        let Some(f) = f else { continue };
        let parts = ssb_psp_runtime::scene::entry_effect_parts(f);
        for (v, &k) in vs.iter().zip(parts) {
            let part = &ENTRY_PARTS[k];
            let clock = match part.clock {
                EntryClock::Effect => f.entry.effect_ticks,
                EntryClock::Rays => f.entry.rays_ticks,
            };
            if clock.is_none() || v.ticks.is_none() || v.link != link {
                continue;
            }
            let Some((object, _)) = assets.entry_effects[k].as_ref() else {
                continue;
            };
            if entry_part_ended(part, object, v) {
                continue;
            }
            let mut posed = [ssb_rom::scene::Mat4::IDENTITY; 16];
            let n = v.anim.compose(p, object, &mut posed);
            if part.root == EntryRoot::Replace {
                if let Some(root) = p.node(object.first_node) {
                    let t = root.rest_translate.map(|x| -x / meshdraw::MODEL_SCALE);
                    let place = ssb_rom::scene::Mat4::from_trs(t, [0.0; 3], [1.0; 3]);
                    for m in &mut posed[..n] {
                        *m = place.mul(m);
                    }
                }
            }
            let mut scales = [[1.0f32; 2]; 16];
            let count = v.anim.billboard_scales(p, object, &mut scales).min(n);
            for &b in part.extra_billboards {
                let node = object.first_node + b as u32;
                let sx = v
                    .anim
                    .node_pose(node)
                    .map(|pose| pose.scale[0])
                    .or_else(|| p.node(node).map(|n| n.rest_scale[0]))
                    .unwrap_or(1.0);
                if let Some(s) = scales.get_mut(b) {
                    *s = [s[0] * sx, s[1] * sx];
                }
            }
            let turned = part.turns_left && f.entry.lr == Some(ssb_game::fighter::Facing::Left);
            let yaw = if turned { core::f32::consts::PI } else { 0.0 };
            let pos = f.entry.pos;
            gpu.model_transform([pos.x, pos.y, pos.z], [0.0, yaw, 0.0], meshdraw::MODEL_SCALE);
            let base = gpu.model_matrix();
            let hidden = |node: u32| !v.anim.visible(p, node);
            unsafe {
                meshdraw::draw_object_posed_scaled_hiding(
                    p,
                    object,
                    &base,
                    &posed[..n],
                    &scales[..count],
                    draw_state,
                    material_anim,
                    Some(&v.materials),
                    &hidden,
                );
            }
        }
    }
}

/// One port's KO blast and respawn halo players, and the clocks they have
/// caught up to.
#[derive(Default)]
struct KoVisual {
    explode_ticks: Option<u16>,
    explode: ssb_rom::skeleton::StageAnimator,
    explode_materials: ssb_rom::skeleton::EffectMaterialAnimator,
    halo_ticks: Option<u16>,
    halo: ssb_rom::skeleton::StageAnimator,
}

/// One port's Egg Lay egg player: the animation epoch it plays and the
/// plays it has caught up to.
#[derive(Default)]
struct EggVisual {
    epoch: Option<u8>,
    plays: u16,
    anim: ssb_rom::skeleton::StageAnimator,
}

impl EffectVisuals {
    /// Plays each fighter's egg up to its play count, restarting on a new
    /// animation (`efManagerYoshiEggLaySetAnim`), at the effect's
    /// `gcSetAnimSpeed`.
    #[inline(never)]
    fn sync_eggs(&mut self, p: &Pack<'_>, assets: &DrawAssets, fighters: [Option<&ssb_game::fighter::Fighter>; 4]) {
        if self.eggs.len() < 4 {
            self.eggs.resize_with(4, Default::default);
        }
        let Some((_, anims)) = assets.egg_lay.as_ref() else {
            return;
        };
        for (v, f) in self.eggs.iter_mut().zip(fighters) {
            let Some(e) = f.and_then(ssb_game::capture_yoshi::egg_effect) else {
                v.epoch = None;
                continue;
            };
            let Some(anim) = anims.get(usize::from(e.index)) else {
                continue;
            };
            if v.epoch != Some(e.epoch) || v.plays > e.plays {
                v.epoch = Some(e.epoch);
                v.plays = 0;
                v.anim.start(p, anim);
            }
            if let Some(script) = p.anim_script(anim) {
                while v.plays < e.plays {
                    let _ = v.anim.tick_speed(script, e.speed);
                    v.plays += 1;
                }
            }
        }
    }
}

/// Draws the egg each trapped fighter sits in (`dEFManagerYoshiEggLayEffectDesc`,
/// RE-417). The root is kind 0x50, a translation to the fighter's TopN,
/// then `Sca` by `effect_size` in X and Y. The tree's root (node 0) is
/// `TraRotRpyRSca`, its translation the stick's wiggle; its child, the egg,
/// is `Tra` then kind 46: a camera-facing quad at its composed position,
/// spun by its own `rotate.z` and sized by `gGCScaleX` (the root's scale
/// times node 0's) times its own scale.
#[inline(never)]
fn draw_egg_effects(
    gpu: &mut Gpu,
    p: &Pack<'_>,
    draw_state: &mut meshdraw::DrawState,
    assets: &DrawAssets,
    visuals: &EffectVisuals,
    fighters: [Option<&ssb_game::fighter::Fighter>; 4],
    camera: &ssb_game::camera::Camera,
) {
    use ssb_rom::scene::Mat4;
    let Some((object, _)) = assets.egg_lay.as_ref() else {
        return;
    };
    let pose_of = |v: &EggVisual, index: u32| {
        let node = object.first_node + index;
        stage_pose(&v.anim, node).or_else(|| {
            p.node(node).map(|n| ssb_rom::figatree::JointPose {
                rotate: n.rest_rotate,
                translate: n.rest_translate,
                scale: n.rest_scale,
            })
        })
    };
    let Some(mesh) = p
        .node(object.first_node + 1)
        .filter(|n| n.mesh != ssb_rom::pack::NodeDesc::NO_MESH)
        .and_then(|n| p.mesh(n.mesh))
    else {
        return;
    };
    for (v, f) in visuals.eggs.iter().zip(fighters) {
        let Some(f) = f else { continue };
        let Some(e) = ssb_game::capture_yoshi::egg_effect(f) else {
            continue;
        };
        let (Some(root), Some(egg)) = (pose_of(v, 0), pose_of(v, 1)) else {
            continue;
        };
        let size = ssb_game::capture_yoshi::EGG_EFFECT_SIZES
            .get(f.kind as usize)
            .copied()
            .unwrap_or(1.0);
        let top = f.joint_world(0, ssb_engine::math::Vec3::ZERO);
        let t0 = [root.translate[0] + e.wiggle[0], root.translate[1] + e.wiggle[1], root.translate[2]];
        let place = Mat4::from_trs([top.x, top.y, top.z], [0.0; 3], [size, size, 1.0])
            .mul(&Mat4::from_trs(t0, root.rotate, root.scale));
        let [x, y, z] = place.transform_point(egg.translate);
        let scale = size * root.scale[0];
        gpu.model_transform_billboard(
            ssb_engine::math::Vec3::new(x, y, z),
            camera.eye,
            camera.at,
            egg.rotate[2],
            [
                meshdraw::MODEL_SCALE * scale * egg.scale[0],
                meshdraw::MODEL_SCALE * scale * egg.scale[1],
            ],
        );
        unsafe {
            meshdraw::draw_mesh(p, &mesh, draw_state, None, None);
        }
    }
}

/// The halo's `gcPlayAnimAll` calls: one on the frame
/// `ftCommonRebirthDownSetStatus` makes it, then one a frame while the
/// despawn wait counts down from 390.
fn halo_ticks(f: &ssb_game::fighter::Fighter) -> Option<u16> {
    ssb_game::dead::halo_scale(f)?;
    let wait = f.dead.rebirth.halo_despawn_wait.clamp(0, ssb_game::dead::HALO_DESPAWN_WAIT);
    Some((ssb_game::dead::HALO_DESPAWN_WAIT + 1 - wait) as u16)
}

impl EffectVisuals {
    /// Plays each port's blast up to its clock (`Explosion::ticks`), on the
    /// player's material scripts, and each halo up to [`halo_ticks`].
    #[inline(never)]
    fn sync_ko(
        &mut self,
        p: &Pack<'_>,
        assets: &DrawAssets,
        ko: &ssb_game::ko::KoEffects,
        fighters: [Option<&ssb_game::fighter::Fighter>; 4],
    ) {
        if self.ko.len() < 4 {
            self.ko.resize_with(4, Default::default);
        }
        for (i, v) in self.ko.iter_mut().enumerate() {
            let blast = ko.explosions[i];
            if let (Some((restart, ticks)), Some((object, anim))) = (
                catch_up(&mut v.explode_ticks, blast.map(|e| e.ticks)),
                assets.dead_explode.as_ref(),
            ) {
                if restart {
                    v.explode.start(p, anim);
                    v.explode_materials.start(p, object_mat_anims(p, object));
                    // `dEFManagerDeadExplodeEffectDesc.o_matanim_joint =
                    // dEFManagerDeadExplodeMatAnimJoints[player]`.
                    let player = usize::from(blast.map_or(0, |e| e.player).min(3));
                    let scripts = ssb_psp_runtime::scene::DEAD_EXPLODE_MAT_SCRIPTS[player];
                    for (mat, &script) in object_mat_anims(p, object).zip(&scripts) {
                        v.explode_materials.restart(mat, script);
                    }
                }
                if let Some(script) = p.anim_script(anim) {
                    for _ in 0..ticks {
                        let _ = v.explode.tick(script);
                        v.explode_materials.tick(p);
                    }
                }
            }
            let halo = fighters[i].and_then(halo_ticks);
            if let (Some((restart, ticks)), Some((_, anim))) =
                (catch_up(&mut v.halo_ticks, halo), assets.rebirth_halo.as_ref())
            {
                if restart {
                    v.halo.start(p, anim);
                }
                if let Some(script) = p.anim_script(anim) {
                    for _ in 0..ticks {
                        let _ = v.halo.tick(script);
                    }
                }
            }
        }
    }
}

/// `dEFManagerRebirthHaloEffectDesc.dl_link`.
const REBIRTH_HALO_LINK: u8 = 10;
/// `dEFManagerDeadExplodeEffectDesc.dl_link`.
const DEAD_EXPLODE_LINK: u8 = 18;

/// Draws each port's respawn halo and KO blast (RE-412).
///
/// The halo's root is matrix kind 0x50 (`func_ovl0_800C99CC`), a
/// translation to TopN's world position, and
/// `efManagerRebirthHaloMakeEffect` scales its child by the fighter's
/// `halo_size`. Its rays (DLLink 1) draw blended, their colour the primitive
/// and their alpha the texture's (RE-420). The blast's root takes the clamped death point and its
/// `rotate.z` by `ExplodeKind`; its first and third DObjs take the player's
/// ENV colours, and all three the player's material scripts.
///
/// The halo is on display link 10 and the blast on 18
/// (`dEFManagerRebirthHaloEffectDesc`, `dEFManagerDeadExplodeEffectDesc`);
/// only the one on `link` draws (RE-422).
#[inline(never)]
fn draw_ko_effects(
    gpu: &mut Gpu,
    p: &Pack<'_>,
    draw_state: &mut meshdraw::DrawState,
    assets: &DrawAssets,
    visuals: &EffectVisuals,
    ko: &ssb_game::ko::KoEffects,
    fighters: [Option<&ssb_game::fighter::Fighter>; 4],
    link: u8,
) {
    use ssb_rom::scene::Mat4;
    for (i, v) in visuals.ko.iter().enumerate() {
        let halo = assets.rebirth_halo.as_ref().filter(|_| link == REBIRTH_HALO_LINK);
        if let (Some(f), Some((object, _))) = (fighters[i], halo) {
            if let (Some(scale), Some(_)) = (ssb_game::dead::halo_scale(f), v.halo_ticks) {
                let mut posed = [Mat4::IDENTITY; 8];
                let n = v.halo.compose(p, object, &mut posed);
                // `child->scale = scale`, about the child's own origin.
                if let Some(child) = p.node(object.first_node + 1) {
                    let t = child.rest_translate.map(|x| x / meshdraw::MODEL_SCALE);
                    let k = Mat4::from_trs(t, [0.0; 3], [1.0; 3])
                        .mul(&Mat4::from_trs([0.0; 3], [0.0; 3], [scale; 3]))
                        .mul(&Mat4::from_trs(t.map(|x| -x), [0.0; 3], [1.0; 3]));
                    for m in posed.iter_mut().take(n).skip(1) {
                        *m = k.mul(m);
                    }
                }
                let top = f.joint_world(0, ssb_engine::math::Vec3::ZERO);
                gpu.model_transform([top.x, top.y, top.z], [0.0; 3], meshdraw::MODEL_SCALE);
                let base = gpu.model_matrix();
                unsafe {
                    meshdraw::draw_object_posed(p, object, &base, &posed[..n], None, draw_state, None, None, 0);
                }
            }
        }
        let explode = assets.dead_explode.as_ref().filter(|_| link == DEAD_EXPLODE_LINK);
        let (Some(e), Some((object, _))) = (ko.explosions[i], explode) else {
            continue;
        };
        if v.explode_ticks.is_none() || v.explode.ended() {
            continue;
        }
        let mut posed = [Mat4::IDENTITY; 8];
        let n = v.explode.compose(p, object, &mut posed);
        gpu.model_transform(
            [e.pos.x, e.pos.y, e.pos.z],
            [0.0, 0.0, e.kind.rotate_z_degrees().to_radians()],
            meshdraw::MODEL_SCALE,
        );
        let base = gpu.model_matrix();
        let player = usize::from(e.player.min(3));
        for node in 1..object.node_count {
            let global = object.first_node + node;
            let env = match node {
                1 => Some(ssb_psp_runtime::scene::DEAD_EXPLODE_ENV_CHILD[player]),
                3 => Some(ssb_psp_runtime::scene::DEAD_EXPLODE_ENV_SIBLING[player]),
                _ => None,
            };
            let prim = p
                .node(global)
                .filter(|n| n.mesh != ssb_rom::pack::NodeDesc::NO_MESH)
                .and_then(|n| p.mesh(n.mesh))
                .and_then(|m| p.prim(m.first_prim))
                .and_then(|prim| v.explode_materials.resolved_colors(prim.mat_anim))
                .and_then(|c| c.prim);
            draw_state.color_override = Some(ssb_rom::skeleton::EffectColors {
                prim,
                env: env.map(|[r, g, b]| [r, g, b, 0xFF]),
                ..Default::default()
            });
            unsafe {
                meshdraw::draw_object_posed_hiding(
                    p,
                    object,
                    &base,
                    &posed[..n],
                    draw_state,
                    Some(&v.explode_materials),
                    &|g| g != global,
                );
            }
        }
        draw_state.color_override = None;
    }
}

/// `lbParticleDrawTextures` through the battle camera (RE-413), with the
/// projection `draw_training` sets, for the particle lists `lists` (one
/// display link's, RE-422).
#[inline(never)]
fn draw_particles(
    p: &Pack<'_>,
    pl: &play::FighterScene,
    hud: &mut Hud,
    draw_state: &mut meshdraw::DrawState,
    lists: &[usize],
) {
    let Some(banks) = ssb_psp_runtime::particles::PackBanks::new(p) else {
        return;
    };
    let view = ssb_engine::math::Mat4::look_at(pl.camera.eye, pl.camera.at, ssb_engine::math::Vec3::Y);
    let (w, h) = pl.camera.viewport_size();
    let proj = ssb_engine::math::Mat4::perspective(
        pl.camera.fovy_degrees.to_radians(),
        w / h,
        ssb_game::camera::DEFAULT_NEAR,
        ssb_game::camera::DEFAULT_FAR,
    );
    unsafe {
        ssb_psp_runtime::particles::draw(&banks, &mut hud.particles, &view, &proj, lists, draw_state);
    }
}

/// `ifScreenFlashProcDisplay`: `color1` over `(10, 10)`–`(310, 230)` of the
/// 320 x 240 screen, blended (`G_RM_AA_XLU_SURF`).
#[inline(never)]
fn draw_screen_flash(gpu: &mut Gpu, draw_state: &mut meshdraw::DrawState, ko: &ssb_game::ko::KoEffects) {
    let Some([r, g, b, a]) = ko.flash_color() else {
        return;
    };
    let (vx, _, _, vh) = ssb_engine::coord::pillarboxed_viewport();
    let k = vh as f32 / ssb_engine::coord::N64_SCREEN.1 as f32;
    let px = |x: i16| (vx as f32 + f32::from(x) * k) as i32;
    let py = |y: i16| (f32::from(y) * k) as i32;
    // A one-cycle fill leaves out the lower-right edge.
    gpu.draw_rect_translucent(px(10), py(10), px(310), py(230), Color::rgba(r, g, b, a));
    draw_state.invalidate_all();
}

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

/// The items whose players are material scripts, and the Shells' spin
/// (RE-442): the Star's and the Ray Gun's `ITAttributes` tables from the
/// make, and the root script a Bob-omb's walk or a Shell's slide adds and
/// a later status clears. Returns whether `item` is one of them.
fn sync_item_scripts(
    p: &Pack<'_>,
    assets: &DrawAssets,
    v: &mut ItemVisual,
    item: &ssb_game::item::Item,
) -> bool {
    use ssb_game::item::ItemKind;
    let table = match item.kind {
        ItemKind::Utility(ssb_game::item::utility::Kind::Star) => {
            Some(assets.utility_items[2].as_ref())
        }
        ItemKind::Equipment(kind) => Some(assets.equipment_items[kind as usize].as_ref()),
        ItemKind::BombHei | ItemKind::Shell(_) | ItemKind::MBall => None,
        _ => return false,
    };
    if let Some(object) = table {
        let Some(object) = object else {
            return true;
        };
        if v.kind != Some(item.kind) || v.ticks > item.anim_ticks {
            v.kind = Some(item.kind);
            v.ticks = 0;
            v.materials.start(p, object_mat_anims(p, object));
        }
        while v.ticks < item.anim_ticks {
            v.materials.tick(p);
            v.ticks += 1;
        }
        return true;
    }
    let Some(script) = item.root_script else {
        if v.kind != Some(item.kind) || v.root_added.is_some() {
            v.kind = Some(item.kind);
            v.root_added = None;
            v.materials = Default::default();
            v.anim = Default::default();
        }
        return true;
    };
    let (plays, cleared) = script.plays(item.anim_ticks);
    if v.kind != Some(item.kind) || v.root_added != Some(script.added) || v.ticks > plays {
        v.kind = Some(item.kind);
        v.root_added = Some(script.added);
        v.root_cleared = false;
        v.ticks = 0;
        let index = match item.kind {
            ItemKind::BombHei => 1,
            ItemKind::MBall => 4,
            _ => 3,
        };
        let Some(object) = assets.throwable_items[index].as_ref() else {
            return true;
        };
        // The extra lists run the root `MObj`'s script too: the left walk,
        // and the Green Shell's, under a key of its own. The Poké Ball's
        // closed ball list carries the throw's script (RE-443).
        let extra = match item.kind {
            ItemKind::BombHei => assets.bombhei_left,
            ItemKind::MBall => assets.mball_ball,
            _ => assets.green_shell,
        };
        let extra = extra
            .into_iter()
            .flat_map(|m| (0..m.prim_count).filter_map(move |i| p.prim(m.first_prim + i)))
            .map(|prim| prim.mat_anim);
        v.materials
            .start(p, object_mat_anims(p, object).chain(extra));
        v.anim = Default::default();
        if let (ItemKind::Shell(_), Some(anim)) = (item.kind, assets.shell_spin.as_ref()) {
            v.anim.start_changed(p, anim);
        }
    }
    let spin = match item.kind {
        ItemKind::Shell(_) => assets
            .shell_spin
            .as_ref()
            .and_then(|anim| p.anim_script(anim)),
        _ => None,
    };
    loop {
        // The status clears `event32` after the play it ran on.
        if cleared == Some(v.ticks) && !v.root_cleared {
            v.root_cleared = true;
            v.materials.clear_scripts();
            v.anim.clear_scripts();
        }
        if v.ticks >= plays {
            break;
        }
        v.materials.tick(p);
        if let Some(script) = spin {
            let _ = v.anim.tick(script);
        }
        v.ticks += 1;
    }
    true
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

/// A Pokémon's players: the appear script from its first process
/// (`gcAddDObjAnimJoint` in the maker, played by `itProcessProcItemMain`),
/// then, from the tick its status added one, the status script, whose own
/// init plays it once at frame zero. Beedrill's fly status stops it.
fn sync_mmonster(
    p: &Pack<'_>,
    assets: &DrawAssets,
    v: &mut ItemVisual,
    item: &ssb_game::item::Item,
    kind: ssb_game::item::mmonster::Kind,
) {
    let Some(asset) = assets.mmonsters[kind as usize].as_ref() else {
        return;
    };
    let vars = &item.vars.mmonster;
    // A status's material script starts with that status (RE-443).
    let status_mat = ssb_rom::mmonster::VISUALS[kind as usize]
        .status_mat
        .is_some();
    if v.kind != Some(item.kind) || v.ticks > item.anim_ticks {
        v.kind = Some(item.kind);
        v.status = None;
        v.playing = asset.appear;
        v.anim = Default::default();
        if let Some(anim) = asset.appear.as_ref() {
            v.anim.start_changed(p, anim);
        }
        // The make's own play ran before the maker added the script.
        v.ticks = 1;
        v.materials = Default::default();
        if !status_mat {
            v.materials.start(p, object_mat_anims(p, &asset.object));
        }
    }
    let advance = |v: &mut ItemVisual, until: u16| {
        let script = v.playing.as_ref().and_then(|a| p.anim_script(a));
        while v.ticks < until {
            if let (Some(script), false) = (script, vars.anim_frozen) {
                let _ = v.anim.tick(script);
            }
            v.materials.tick(p);
            v.ticks += 1;
        }
    };
    if vars.status_anim != v.status {
        if let Some(at) = vars.status_anim {
            advance(v, at);
            if let Some(anim) = asset.status {
                v.anim.start_changed(p, &anim);
                if let Some(script) = p.anim_script(&anim) {
                    let _ = v.anim.tick(script);
                }
                v.playing = Some(anim);
            }
            if status_mat {
                v.materials.start(p, object_mat_anims(p, &asset.object));
                v.materials.tick(p);
            }
        }
        v.status = vars.status_anim;
    }
    advance(v, item.anim_ticks);
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
            if let ssb_game::item::ItemKind::MMonster(kind) = item.kind {
                sync_mmonster(p, assets, visual, item, kind);
                continue;
            }
            if sync_item_scripts(p, assets, visual, item) {
                continue;
            }
            let Some((object, anim)) = assets.item(item.kind) else {
                continue;
            };
            if visual.kind != Some(item.kind) || visual.ticks > item.anim_ticks {
                visual.kind = Some(item.kind);
                visual.ticks = 0;
                if item.kind == ssb_game::item::ItemKind::Container(ssb_game::item::container::Kind::Egg) {
                    visual.anim.start_changed(p, anim);
                } else {
                    visual.anim.start(p, anim);
                }
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
    dummies: &Dummies,
    weapons: &ssb_game::weapon::WeaponPool,
    items: &ssb_game::item::ItemPool,
    assets: &DrawAssets,
    effect_visuals: &EffectVisuals,
    material_anim: Option<&ssb_rom::skeleton::MaterialAnimator>,
    stage_map: Option<&ssb_psp_runtime::scene::StageMap>,
    stage_objects: Option<&ssb_rom::ground_obj::GroundObjects>,
    no_pack_color: Color,
    damage_hud: &mut Hud,
    battle: Option<&ssb_game::battle::Battle>,
    wallpaper: Option<(&ssb_rom::pack::SpriteDesc, &ssb_game::wallpaper::Wallpaper)>,
    training_paused: bool,
    magnify_display: bool,
    cpu_ports: [bool; 4],
    // Final Destination's boss wallpaper effects and their controller.
    boss: Option<(&ssb_psp_runtime::boss::BossEffects, &ssb_game::spgame::boss::BossWallpaper)>,
) {
    let scene = pack
        .zip(play_state)
        .and_then(|(p, pl)| p.stage(stage_index).map(|s| (p, pl, s)));

    let Some((p, pl, stage)) = scene else {
        gpu.set_viewport_fullscreen();
        gpu.begin_frame(Some(no_pack_color));
        return;
    };

    // The opening's default camera fills black (RE-467).
    gpu.begin_frame(Some(if damage_hud.movie { BG_RESULTS } else { BG_TRAINING }));
    draw_state.kind48_flat = damage_hud.movie_flat48;
    gpu.set_viewport_pillarboxed();
    // `gmCameraSetViewportDimensions`: How to Play's battle cameras (the
    // wallpaper's among them) stop above its window.
    let viewport = pl.camera.viewport;
    if viewport != ssb_game::camera::BATTLE_VIEWPORT {
        gpu.set_viewport_n64(viewport);
    }
    if stage.source_file == ssb_rom::ground_obj::BONUS3_FILE {
        meshdraw::fill_rect_n64([10.0, 10.0, 310.0, 230.0], [0, 0, 0, 255], draw_state);
    }
    // `gmCameraMakeWallpaperCamera` (priority 80) draws before the stage
    // camera (50). Race uses a black viewport without a sprite.
    if let Some((sprite, w)) = wallpaper.filter(|(_, w)| w.kind != ssb_game::wallpaper::Kind::Bonus3) {
        if damage_hud.movie {
            // The opening's wallpaper camera keeps its border (RE-467).
            gpu.set_viewport_n64(viewport);
            meshdraw::draw_wallpaper_n64(p, sprite, w.x, w.y, w.scale, draw_state);
        } else {
            meshdraw::draw_wallpaper(p, sprite, w.x, w.y, w.scale, draw_state);
        }
    }
    // `sc1PGameBossMakeCamera`'s second camera (priority 60, tag 2): the
    // boss wallpaper's far effects, between the wallpaper and the stage.
    if let Some((effects, controller)) = boss {
        draw_state.begin_frame();
        effects.draw(gpu, p, draw_state, controller, 2);
    }
    gpu.set_viewport_n64(viewport);
    // Tags, culling and magnifier scale use this camera's live projection.
    // Its default is 38 degrees; entry/pause zooms ease to their source FOV.
    // `dGMCameraPerspDefault` supplies near 256 and far 39,936 (RE-421/440).
    let (viewport_w, viewport_h) = pl.camera.viewport_size();
    if let Some(m) = pl.camera.movie {
        // An opening scene's movie camera (RE-467): its own viewport,
        // aspect and planes, its view rolled by `up.x`.
        gpu.set_viewport_n64(m.viewport);
        gpu.set_perspective(pl.camera.fovy_degrees, m.aspect, m.near, m.far);
        gpu.reset_modelview();
        draw_state.begin_frame();
        gpu.set_view(&ssb_psp_runtime::movie::view_matrix(
            pl.camera.eye,
            pl.camera.at,
            m.up_x,
            ssb_game::opening::movie::View::Roll,
        ));
    } else {
        gpu.set_perspective(
            pl.camera.fovy_degrees,
            viewport_w / viewport_h,
            ssb_game::camera::DEFAULT_NEAR,
            ssb_game::camera::DEFAULT_FAR,
        );
        gpu.reset_modelview();
        draw_state.begin_frame();

        gpu.set_view(&ssb_engine::math::Mat4::look_at(
            pl.camera.eye,
            pl.camera.at,
            ssb_engine::math::Vec3::Y,
        ));
    }
    gpu.model_transform([0.0, 0.0, 0.0], [0.0, 0.0, 0.0], meshdraw::MODEL_SCALE);
    let base = gpu.model_matrix();

    // `sc1PGameFuncLights`: the stage light that animated stage light
    // colours are evaluated under (RE-322).
    draw_state.set_stage_light(stage.light_angle_xy);
    // `gmCameraDefaultProcDisplay` draws the battle in passes of display
    // links, each running every head-0 list of its links before any head-1
    // list (RE-422): link 4 (layer 0); links 6-12 (layer 1, the shadows,
    // the fighters, the link-10 effects, the items and the acid); links
    // 13-15 (layer 2, the weapons and the effects); links 16-18 (Dream
    // Land's front flowers, layer 3 and the link-18 effects). Layers 0, 2
    // and 3 and the front flowers clear `G_ZBUFFER`, so what a later pass
    // draws covers them and they cover what an earlier pass drew.
    let stage_pass = |links: core::ops::RangeInclusive<u8>, heads, draw_state: &mut meshdraw::DrawState| {
        draw_state.heads = heads;
        // All twelve courses put empty yakumono parents 1..10 before
        // their other layer-1 nodes. Draw their inserted children there.
        if links.contains(&6) {
            if let Some(map) = stage_map {
                map.draw_platforms(p, &base, draw_state);
            }
        }
        meshdraw::draw_stage_links(
            p,
            &stage,
            &base,
            stage_map.map(|m| &m.animator),
            stage_objects,
            draw_state,
            material_anim,
            links.clone(),
        );
        draw_state.heads = meshdraw::Heads::Both;
    };
    use meshdraw::Heads::{Head0, Head1};
    let fighters = scenes_ref(pl, dummies);
    let fighter_refs = fighters.map(|x| x.map(|x| &x.fighter));
    let views = fighters.map(|f| {
        f.map_or_else(ssb_game::player_interface::View::default, |f| {
            ssb_game::player_interface::view(&f.fighter, &pl.camera, f.cam_offset_y)
        })
    });
    damage_hud.players.views = views;
    // Links 1-2, before layer 0 (which clears `G_ZBUFFER` and covers
    // them): Captain Falcon on a leftward entry (`ftParamMoveDLLink`) and
    // the entry vehicles `efManagerSortZNeg` puts behind z -1000 (RE-425).
    for f in fighters
        .iter()
        .flatten()
        .filter(|f| f.fighter.entry.is_link_1)
    {
        if !views[usize::from(f.fighter.port)].offscreen {
            draw_fighter_model(gpu, p, &stage, draw_state, f, &pl.camera, false);
        }
    }
    draw_entry_effects(
        gpu,
        p,
        draw_state,
        assets,
        effect_visuals,
        fighter_refs,
        material_anim,
        2,
    );
    stage_pass(0..=5, Head0, draw_state);
    stage_pass(0..=5, Head1, draw_state);
    stage_pass(6..=8, Head0, draw_state);

    // The N64 puts shadows on their own display link between the stage and
    // fighters.  Resolve each independently from its live floor/air state;
    // the fixed scratch is copied into GE memory by the renderer, so it is
    // safe to reuse for every fighter without a per-frame allocation.
    if let Some(shadow_texture) = assets.shadow_texture.as_ref() {
        let mut shadow_verts = [meshdraw::TexQuadVertex::default(); 18];
        for f in fighters.iter().flatten() {
            let shadow =
                ssb_psp_runtime::scene::fighter_shadow(p, &stage, &f.fighter, f.shadow_size);
            meshdraw::draw_fighter_shadow(
                p,
                shadow_texture,
                &shadow,
                [0x00, 0x00, 0x00, 0xA0],
                &mut shadow_verts,
                draw_state,
            );
        }
    }

    // Each fighter in port order, the CPUs drawn the same way as the
    // player's -- their own pose, their own per-fighter light rebuild
    // (RE-164).
    for f in fighters
        .iter()
        .flatten()
        .filter(|f| !f.fighter.entry.is_link_1)
    {
        if !views[usize::from(f.fighter.port)].offscreen {
            draw_fighter_model(gpu, p, &stage, draw_state, f, &pl.camera, false);
        }
    }
    // Link 10: the title demos' fade (made before any effect), the entry
    // effects, the trapping egg, the halo, the impact wave and particle
    // list 4.
    if damage_hud.demo_fade_alpha != 0 {
        meshdraw::fill_rect_n64(viewport, [0, 0, 0, damage_hud.demo_fade_alpha], draw_state);
    }
    draw_egg_effects(
        gpu,
        p,
        draw_state,
        assets,
        effect_visuals,
        fighter_refs,
        &pl.camera,
    );
    draw_entry_effects(
        gpu,
        p,
        draw_state,
        assets,
        effect_visuals,
        fighter_refs,
        material_anim,
        10,
    );
    draw_ko_effects(
        gpu,
        p,
        draw_state,
        assets,
        effect_visuals,
        &damage_hud.ko,
        fighter_refs,
        REBIRTH_HALO_LINK,
    );
    draw_display_effects(
        p,
        draw_state,
        assets,
        &mut damage_hud.display_scratch,
        &damage_hud.effects,
        pl.camera.eye,
        10,
    );
    draw_particles(
        p,
        pl,
        damage_hud,
        draw_state,
        &ssb_psp_runtime::particles::LINK10_LISTS,
    );
    let battle_part = |part, draw_state: &mut meshdraw::DrawState, gpu: &mut Gpu| {
        draw_items_weapons_effects(
            gpu,
            draw_state,
            p,
            pl,
            dummies,
            weapons,
            items,
            assets,
            effect_visuals,
            material_anim,
            part,
        );
    };
    // Items draw after the fighters, each of which restores the stage's
    // light (`ftDisplayMainProcDisplay`'s closing
    // `ftDisplayLightsDrawReflect`) under the pass's `G_LIGHTING`: a lit
    // item primitive takes that direction and its own `G_MW_LIGHTCOL`.
    draw_state.configure_item_light(stage.light_angle_xy);
    battle_part(BattlePart::Items, draw_state, gpu);
    draw_state.finish_fighter_light();
    {
        for display in damage_hud
            .effects
            .displays()
            .filter(|d| d.kind == ssb_game::effect::DisplayKind::ContainerSmash)
        {
            if let Some((kind, pieces)) = damage_hud.effects.container_pieces(display) {
                let mesh = match kind {
                    ssb_game::effect::SmashPiece::Box => assets.container_piece.as_ref(),
                    ssb_game::effect::SmashPiece::TaruBomb => assets.tarubomb_piece.as_ref(),
                };
                let Some(mesh) = mesh else { continue; };
                for piece in pieces {
                    gpu.model_transform_xyz(
                        [piece.pos.x, piece.pos.y, piece.pos.z],
                        [piece.rotate.x, piece.rotate.y, piece.rotate.z],
                        [meshdraw::MODEL_SCALE; 3],
                    );
                    meshdraw::draw_mesh(p, mesh, draw_state, None, None);
                }
            }
        }
    }
    stage_pass(9..=12, Head0, draw_state);
    stage_pass(6..=12, Head1, draw_state);

    // Links 13-15: layer 2 (no head-1 lists on any stage), the weapons, then
    // the link-15 effects and particle list 1.
    stage_pass(13..=15, meshdraw::Heads::Both, draw_state);
    battle_part(BattlePart::Weapons, draw_state, gpu);
    battle_part(BattlePart::Effects15, draw_state, gpu);
    draw_thunder_segments(gpu, p, draw_state, assets, &damage_hud.effects);
    draw_display_effects(
        p,
        draw_state,
        assets,
        &mut damage_hud.display_scratch,
        &damage_hud.effects,
        pl.camera.eye,
        15,
    );
    draw_particles(
        p,
        pl,
        damage_hud,
        draw_state,
        &ssb_psp_runtime::particles::LINK15_LISTS,
    );

    // Links 16-18: the front flowers and layer 3, then the link-18 effects
    // and particle lists 0 and 2, then the stage's head-1 lists.
    stage_pass(16..=17, Head0, draw_state);
    draw_state.configure_item_light(stage.light_angle_xy);
    draw_link18_monsters(gpu, p, draw_state, assets, pl, items, effect_visuals);
    draw_state.finish_fighter_light();
    draw_ko_effects(
        gpu,
        p,
        draw_state,
        assets,
        effect_visuals,
        &damage_hud.ko,
        fighter_refs,
        DEAD_EXPLODE_LINK,
    );
    draw_display_effects(
        p,
        draw_state,
        assets,
        &mut damage_hud.display_scratch,
        &damage_hud.effects,
        pl.camera.eye,
        18,
    );
    draw_particles(
        p,
        pl,
        damage_hud,
        draw_state,
        &ssb_psp_runtime::particles::LINK18_LISTS,
    );
    stage_pass(16..=18, Head1, draw_state);
    // Links 19-20: the entry vehicles sorted in front (RE-425).
    draw_entry_effects(
        gpu,
        p,
        draw_state,
        assets,
        effect_visuals,
        fighter_refs,
        material_anim,
        20,
    );
    // The boss's first camera (priority 40, tag 1): the near comets and
    // the closing effect, whose fades cover the battle.
    if let Some((effects, controller)) = boss {
        effects.draw(gpu, p, draw_state, controller, 1);
        if let Some(color) = damage_hud.boss_fade {
            meshdraw::fill_rect_n64([10.0, 10.0, 310.0, 230.0], color, draw_state);
        }
    }
    // Arrow camera 35, magnifier camera 30, then the interface camera 20.
    let show_magnify = magnify_display
        && damage_hud.pause.is_none()
        && battle.is_none_or(|b| {
            matches!(
                b.status,
                ssb_game::battle::GameStatus::Wait | ssb_game::battle::GameStatus::Go
            )
        });
    if show_magnify {
        let flags = views
            .iter()
            .filter(|v| v.offscreen && v.eligible)
            .fold(0, |flags, v| flags | v.arrow);
        if !training_paused && stage.source_file != ssb_rom::ground_obj::BONUS3_FILE {
            player_screen::arrows(gpu, p, draw_state, &damage_hud.players, flags);
        }
        draw_magnifiers(
            gpu,
            p,
            &stage,
            draw_state,
            fighters,
            &pl.camera,
            views,
            damage_hud.colors,
            assets.player_frame.as_ref(),
        );
    }
    gpu.set_viewport_pillarboxed();
    // The interface's link 25, under `gmCameraMakeEffectCamera`'s camera:
    // the stock snaps and steals and the scores (RE-464).
    if let Some(banks) = ssb_psp_runtime::particles::PackBanks::new(p) {
        ssb_psp_runtime::particles::draw_screen(
            &banks,
            &mut damage_hud.particles,
            viewport,
            &ssb_psp_runtime::particles::LINK25_LISTS,
            draw_state,
        );
    }
    draw_screen_flash(gpu, draw_state, &damage_hud.ko);
    // The opening's battles (`nSCBattleGameTypeMovie`) make no interface.
    if damage_hud.movie {
        gpu.set_viewport_pillarboxed();
        return;
    }
    // `players[].color`: in a free-for-all the human's port and a CPU's
    // `GMCOMMON_PLAYERS_MAX`, in a team battle the team's colour.
    let emblems = fighters.map(|x| {
        x.map(|x| {
            let color = damage_hud.colors[usize::from(x.fighter.port).min(3)];
            (x.fighter.kind, usize::from(color))
        })
    });
    // `ifCommonBattleInterfaceProcSet` hides every interface at Set, and
    // `ifCommonBattlePauseInitInterface` while paused, when only the pause
    // menu draws.
    if battle.is_some_and(|b| b.status == ssb_game::battle::GameStatus::Set) {
        return;
    }
    if let Some(pause) = damage_hud.pause {
        draw_pause_menu(gpu, p, draw_state, pause.kind, pause.retry);
        return;
    }
    // Training hides every Interface-link GObj while its PauseMenu link
    // draws, including the battle damage and Training stat displays.
    if training_paused {
        return;
    }
    player_screen::tags(
        gpu,
        p,
        draw_state,
        fighters,
        &pl.camera,
        damage_hud.colors,
        cpu_ports,
    );
    // `gmCameraMakeInterfaceCamera`'s scissor is the battle viewport.
    if viewport != ssb_game::camera::BATTLE_VIEWPORT {
        gpu.set_viewport_n64(viewport);
    }
    draw_damage_hud(p, draw_state, &damage_hud.damage, emblems, stage_index);
    if let Some(count) = damage_hud.bonus_tasks {
        let offset = if ssb_rom::bonus2::kind(stage.source_file).is_some() { ssb_rom::campaign::OBJECTIVES_PLATFORM } else { ssb_rom::campaign::OBJECTIVES_TARGET };
        if let Some(icon) = p.sprite(ssb_rom::campaign::OBJECTIVES.file, offset) {
            for i in 0..count {
                let x = 30 + (i32::from(icon.width) + 3) * i32::from(i) - i32::from(icon.width) / 2;
                draw_plain(p, draw_state, &icon, x as f32, (30 - i32::from(icon.height) / 2) as f32);
            }
        }
    }
    if let Some(t) = damage_hud.bonus_timer {
        draw_practice_timer(p, draw_state, &t);
    }
    if let Some(b) = battle {
        draw_stocks(p, draw_state, b, damage_hud, fighters.map(|x| x.map(|x| &x.fighter)));
        draw_team_stocks(p, draw_state, damage_hud);
        draw_stock_steals(p, draw_state, damage_hud, fighters.map(|x| x.map(|x| &x.fighter)));
        draw_timer(p, draw_state, b);
    }
    if let Some(c) = damage_hud.countdown.as_ref() {
        draw_countdown(p, draw_state, c);
    }
    // Each pickup arrow is made with its item, after the battle's own
    // interface GObjs.
    player_screen::item_arrows(gpu, p, draw_state, items, &pl.camera);
    if let Some(end) = battle.and_then(|b| b.end) {
        draw_announce(p, draw_state, end);
    }
    if damage_hud.bonus_fade_alpha != 0 {
        meshdraw::fill_rect_n64(viewport, [0, 0, 0, damage_hud.bonus_fade_alpha], draw_state);
    }
    gpu.set_viewport_pillarboxed();
}

#[inline(never)]
unsafe fn draw_magnifiers(
    gpu: &mut Gpu,
    p: &Pack<'_>,
    stage: &ssb_rom::pack::StageDesc,
    st: &mut meshdraw::DrawState,
    fighters: [Option<&play::FighterScene>; 4],
    camera: &ssb_game::camera::Camera,
    views: [ssb_game::player_interface::View; 4],
    colors: [u8; 4],
    frame_image: Option<&ssb_rom::texture::Rgba8>,
) {
    use ssb_game::player_interface as logic;
    let Some(frame) = p.sprite(
        ssb_rom::player_interface::FILE,
        ssb_rom::player_interface::FRAME,
    ) else {
        return;
    };
    let Some(image) = frame_image else { return };
    let scale = logic::magnify_scale(camera);
    let distance = (camera.eye - camera.at).length();
    let mut mini_camera = *camera;
    mini_camera.eye = ssb_engine::math::Vec3::new(0.0, 300.0, distance);
    mini_camera.at = ssb_engine::math::Vec3::new(0.0, 300.0, 0.0);
    for f in fighters.into_iter().flatten() {
        let port = usize::from(f.fighter.port).min(3);
        let v = views[port];
        if !v.offscreen || !v.eligible {
            continue;
        }
        let (w, h) = camera.viewport_size();
        let xy = logic::magnify_position_in(v.direction, scale, ((w * 0.5) as i32 as f32, (h * 0.5) as i32 as f32));
        let (cx, cy) = camera.viewport_center();
        let (x, y) = (cx + xy.0, cy - xy.1);
        let color = logic::MAGNIFY_COLORS[usize::from(colors[port]).min(4)];
        let fog = f
            .fighter
            .dead
            .bounds
            .map_or(stage.fog_color, |b| b.fog_color);
        gpu.set_viewport_n64([10.0, 10.0, 310.0, 230.0]);
        let rect = [
            x - 16.0 * scale,
            y - 16.0 * scale,
            32.0 * scale,
            32.0 * scale,
        ];
        meshdraw::draw_depth_image(image, rect, st);
        meshdraw::draw_sprite(
            p,
            &frame,
            &meshdraw::SObjDraw {
                x: rect[0],
                y: rect[1],
                scale,
                prim: [fog[0], fog[1], fog[2], 255],
                env: [color[0], color[1], color[2]],
                solid: false,
                attr: ssb_rom::sprite::SP_TRANSPARENT,
            },
            st,
        );
        let half = 9.0 * scale;
        gpu.set_viewport_n64([x - half, y - half, x + half, y + half]);
        gpu.set_ortho([-450.0, 450.0, -450.0, 450.0], 256.0, 39936.0);
        gpu.set_view(&ssb_engine::math::Mat4::look_at(
            mini_camera.eye,
            mini_camera.at,
            ssb_engine::math::Vec3::Y,
        ));
        draw_fighter_model(gpu, p, stage, st, f, &mini_camera, true);
        player_screen::pointer(gpu, p, st, xy, v.direction, scale, color);
    }
}

/// One fighter's model: posed, placed on its catcher's joint while held,
/// and lit by `ftDisplayMainProcDisplay`'s rebuild of its one directional
/// light from the active stage's `MPGroundData.light_angle.x/y` immediately
/// before drawing it (RE-164) -- matches `psp-asset-viewer/main.rs`'s own
/// real-camera fighter draw. `FTStruct::is_invisible` (a KO, a Kirby or
/// Yoshi capture) and Yoshi's egg shield skip the model.
#[inline(never)]
unsafe fn draw_fighter_model(
    gpu: &mut Gpu,
    p: &Pack<'_>,
    stage: &ssb_rom::pack::StageDesc,
    draw_state: &mut meshdraw::DrawState,
    f: &play::FighterScene,
    camera: &ssb_game::camera::Camera,
    miniature: bool,
) {
    // Yoshi's egg shield hides every part (`ftParamHideModelPartAll`, RE-418).
    let hidden = f.fighter.is_invisible || ssb_game::combat::is_yoshi_egg_shield(&f.fighter);
    let Some(obj) = p.object(f.object).filter(|_| !hidden) else {
        return;
    };
    let mut posed = [ssb_rom::scene::Mat4::IDENTITY; ssb_rom::skeleton::MAX_NODES];
    let n = f.compose_model(p, &obj, &mut posed);
    // RE-426: the low-detail model, posed by the high-detail one's nodes.
    let drawn = ssb_psp_runtime::scene::fighter_draw_object(
        f.object,
        f.object_low,
        f.fighter.model_parts.detail_curr,
    );
    let obj = p.object(drawn).unwrap_or(obj);
    // TopN's scale is `attr->size` (`ftManagerMakeFighter`): Giant Donkey
    // Kong twice the size of his model, Mario 1.12, Kirby 0.91 (RE-458).
    let scale = meshdraw::MODEL_SCALE * f.fighter.attributes.size;
    if miniature {
        gpu.model_transform(
            [0.0; 3],
            [0.0, play::fighter_turn(&f.fighter), 0.0],
            scale,
        );
    } else if let Some(joint) = f
        .fighter
        .grab
        .holder
        .and_then(|h| h.anchor_transform)
        .filter(|_| ssb_game::grab::is_held(f.fighter.status.status))
    {
        gpu.model_transform_joint(f.fighter.pos, joint, scale);
    } else {
        gpu.model_transform(
            [f.fighter.pos.x, f.fighter.pos.y, f.fighter.pos.z],
            [0.0, play::fighter_turn(&f.fighter), 0.0],
            scale,
        );
    }
    let m = gpu.model_matrix();
    // `ftDisplayLightsDrawReflect`: a colour animation's light turns with
    // the fighter (`lr * light_angle_x`).
    let light = f.fighter.colanim.light.map_or(stage.light_angle_xy, |(x, y)| [f.fighter.facing.sign() * x, y]);
    draw_state.configure_fighter_light(light);
    // `ftDisplayMainCalcFogColor` with no shade: `G_RM_FOG_PRIM_A` blends
    // the fighter towards `color1` by its alpha.
    let fog = f.fighter.colanim.color();
    if let Some(rgba) = fog {
        let (eye, at) = (camera.eye, camera.at);
        let view = (at - eye).normalized();
        gpu.set_constant_fog((f.fighter.pos - eye).dot(view), rgba);
    }
    // Its model parts and headgear accessory (RE-425).
    let parts = f.fighter.model_parts.draw_parts();
    meshdraw::draw_fighter_posed(
        p,
        &obj,
        &m,
        &posed[..n],
        draw_state,
        meshdraw::Look {
            costume: fighter_draw_costume(p, &obj, &f.fighter),
            textures: f.fighter.model_parts.draw_textures(),
            parts: parts.as_ref().map(|p| &p[..]),
            accessory_before: f.fighter.kind == ssb_game::fighter::FighterKind::Purin,
        },
    );
    if fog.is_some() {
        gpu.clear_fog();
    }
    draw_state.finish_fighter_light();
}

/// The costume a fighter's model draws in: its own, or, while its colour
/// animation shows a skeleton (`ftDisplayMainDrawAll`), that skeleton set's
/// parts (RE-414) when the fighter has one.
#[inline(never)]
fn fighter_draw_costume(
    p: &Pack<'_>,
    obj: &ssb_rom::pack::ObjectDesc,
    f: &ssb_game::fighter::Fighter,
) -> u32 {
    let id = u32::from(f.colanim.skeleton_id);
    let nodes = obj.first_node..obj.first_node + obj.node_count;
    if id != 0 {
        let key = ssb_rom::pack::SKELETON_COSTUME_BASE + id;
        if nodes.clone().any(|n| p.costume_mesh(n, key).is_some()) {
            return key;
        }
    }
    // Kirby's copy hat (RE-417) is joint 6's model part (RE-425).
    u32::from(f.costume)
}

/// `ifCommonPlayerDamageProcDisplay` for each fighter, over the 3D scene.
#[inline(never)]
fn draw_damage_hud(
    p: &Pack<'_>,
    draw_state: &mut meshdraw::DrawState,
    hud: &[ssb_game::hud::DamageDisplay; 4],
    fighters: [Option<(ssb_game::fighter::FighterKind, usize)>; 4],
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
            let (x, y) = ssb_game::hud::emblem_origin_at(d.pos_x, d.pos_y, sprite.width, sprite.height);
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
    retry: bool,
) {
    use ssb_game::pause;
    let (vx, _, _, vh) = ssb_engine::coord::pillarboxed_viewport();
    let k = vh as f32 / ssb_engine::coord::N64_SCREEN.1 as f32;
    let px = |x: i16| (vx as f32 + f32::from(x) * k) as i32;
    let py = |y: i16| (f32::from(y) * k) as i32;
    for [ulx, uly, lrx, lry] in pause::BORDER {
        // `G_CYC_FILL` covers both corners.
        gpu.draw_rect_fill(px(ulx), py(uly), px(lrx + 1), py(lry + 1), Color::rgba(0xFF, 0xFF, 0xFF, 0xFF));
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
    if retry {
        for d in pause::BONUS_RETRY {
            draw(d.sprite, d.pos, d.prim, d.env);
        }
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
    hud: &Hud,
    fighters: [Option<&ssb_game::fighter::Fighter>; 4],
) {
    for f in fighters.into_iter().flatten() {
        let player = usize::from(f.port);
        // `players[].is_single_stockicon`: a VS time battle's, or the 1P
        // Game's per player.
        let single = hud
            .single_stock
            .map_or(b.rule == ssb_game::battle::Rule::Time, |s| s[player]);
        let (pos_x, pos_y) = (hud.damage[player].pos_x, hud.damage[player].pos_y);
        let Some(icon) = p.fighter_sprite(f.kind as u8, ssb_rom::pack::SpriteDesc::ROLE_STOCK, f.costume) else {
            continue;
        };
        for (x, y) in ssb_game::hud::stock_icons_at(pos_x, pos_y, f.stocks, single, icon.width, icon.height) {
            draw_plain(p, draw_state, &icon, x, y);
        }
    }
}

/// `ifCommonPlayerStockStealMakeInterface`'s icons in flight: the stolen
/// player's stock icon in its costume (RE-464).
fn draw_stock_steals(
    p: &Pack<'_>,
    draw_state: &mut meshdraw::DrawState,
    hud: &Hud,
    fighters: [Option<&ssb_game::fighter::Fighter>; 4],
) {
    for steal in hud.steals.iter().flatten() {
        let Some(f) = fighters.get(usize::from(steal.stolen)).copied().flatten() else {
            continue;
        };
        if let Some(icon) = p.fighter_sprite(f.kind as u8, ssb_rom::pack::SpriteDesc::ROLE_STOCK, f.costume) {
            draw_plain(p, draw_state, &icon, steal.x, steal.y);
        }
    }
}

/// `sc1PGameTeamStockDisplayProcDisplay`: the enemy team's remaining
/// members, ten to a row from (20, 20): Yoshi's stock icon in each
/// member's costume, the Kirbys' in theirs, the Polygons' own
/// (`FTStocksZako`).
#[inline(never)]
fn draw_team_stocks(p: &Pack<'_>, draw_state: &mut meshdraw::DrawState, hud: &Hud) {
    use ssb_game::{fighter::FighterKind, spgame::Stage};
    let Some((stage, icons)) = hud.team_stocks.as_ref() else {
        return;
    };
    let zako = || {
        let f = &ssb_rom::sprite::STOCKS_ZAKO;
        p.sprite(f.file, f.offsets[0])
    };
    for icon in icons {
        let sprite = match (stage, icon.costume) {
            (Stage::Yoshi, Some(c)) => p.fighter_sprite(FighterKind::Yoshi as u8, ssb_rom::pack::SpriteDesc::ROLE_STOCK, c),
            (Stage::Kirby, Some(c)) => p.fighter_sprite(FighterKind::Kirby as u8, ssb_rom::pack::SpriteDesc::ROLE_STOCK, c),
            _ => zako(),
        };
        if let Some(sprite) = sprite {
            draw_plain(p, draw_state, &sprite, icon.x as f32, icon.y as f32);
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

/// Bonus Practice's time passed: six `IFCommonTimer` digits centred at
/// y 30, the tens of minutes hidden until they first change, and the
/// seconds and hundredths marks (`sc1PBonusStageMakeTimer`).
fn draw_practice_timer(p: &Pack<'_>, draw_state: &mut meshdraw::DrawState, t: &ssb_game::spgame::bonus_stage::PracticeTimer) {
    use ssb_game::spgame::bonus_stage as bs;
    let f = &ssb_rom::sprite::TIMER;
    let sprite = |i: usize| f.offsets.get(i).and_then(|&at| p.sprite(f.file, at));
    for (i, (&x, &digit)) in bs::TIMER_DIGIT_X.iter().zip(&t.digits).enumerate() {
        if i == 0 && !t.tens_shown {
            continue;
        }
        if let Some(s) = sprite(usize::from(digit)) {
            let x0 = x - f32::from(s.width) * 0.5;
            let y0 = 30.0 - f32::from(s.height) * 0.5;
            draw_plain(p, draw_state, &s, x0 as i32 as f32, y0 as i32 as f32);
        }
    }
    // `SymbolSec` and `SymbolCSec`, the list's last two.
    for (i, at) in [(13, bs::TIMER_SEC_AT), (14, bs::TIMER_CSEC_AT)] {
        if let Some(s) = sprite(i) {
            let x0 = (at[0] - f32::from(s.width) * 0.5) as i32;
            let y0 = (at[1] - f32::from(s.height) * 0.5) as i32;
            draw_plain(p, draw_state, &s, x0 as f32, y0 as f32);
        }
    }
}

/// "TIME UP" (`ifCommonAnnounceTimeUpMakeInterface`) or "GAME SET"
/// (`ifCommonAnnounceGameSetMakeInterface`) until the battle's Set.
#[inline(never)]
fn draw_announce(p: &Pack<'_>, draw_state: &mut meshdraw::DrawState, end: ssb_game::battle::EndKind) {
    use ssb_game::hud;
    let (letters, f): (&[(f32, f32, u8)], _) = match end {
        ssb_game::battle::EndKind::TimeUp => (&hud::TIME_UP, &ssb_rom::sprite::GAME_STATUS),
        ssb_game::battle::EndKind::GameSet => (&hud::GAME_SET, &ssb_rom::sprite::GAME_STATUS),
        ssb_game::battle::EndKind::Complete => (&hud::COMPLETE, &ssb_rom::sprite::ANNOUNCE_COMMON),
        ssb_game::battle::EndKind::Failure => (&hud::FAILURE, &ssb_rom::sprite::ANNOUNCE_COMMON),
        // Master Hand's defeat announces nothing.
        ssb_game::battle::EndKind::BossDefeat => return,
    };
    for &(x, y, i) in letters {
        if let Some(s) = f.offsets.get(usize::from(i)).and_then(|&at| p.sprite(f.file, at)) {
            let env = match end {
                ssb_game::battle::EndKind::Complete => [0xFF, 0, 0],
                ssb_game::battle::EndKind::Failure => [0, 0, 0xFF],
                _ => [0; 3],
            };
            let d = meshdraw::SObjDraw { x, y, scale: 1.0, prim: [0xFF; 4], env,
                solid: false, attr: ssb_game::countdown::ATTR_TRANSPARENT };
            unsafe { meshdraw::draw_sprite(p, &s, &d, draw_state); }
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

/// The part of [`draw_items_weapons_effects`] a battle camera pass draws
/// (RE-422): items on DL link 11 (the fighters' pass), weapons on 14 and the
/// effects on 15 (the pass after it, with stage layer 2 on 13 first).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BattlePart {
    Items,
    Weapons,
    Effects15,
}

/// The item pass (DL link 11) and the weapon and effect pass (links 13 to 15)
/// that follow the fighters. Kept out of [`draw_training`] so neither
/// function outgrows MIPS branch range.
#[inline(never)]
#[allow(clippy::too_many_arguments)]
/// The throwable utilities (RE-434). The Green Shell binds palette 1. The
/// Bob-omb and the Bumper put kind 0x2E on their root (node 1 once
/// `itManagerMakeItem` ejects descriptor 0): a camera-facing quad spun by
/// `rotate.z` and sized by the root's scale. The Shells put `TraRotRpyR`
/// then kind 0x48 (`func_ovl0_800CAB48`) on theirs: `rotate.x`/`rotate.y`
/// relative to the camera. The Motion-Sensor Bomb and the Poké Ball turn
/// their root by `rotate.z`; its first child is the armed or open shape,
/// and its second, the ball, is kind 0x46 (`func_ovl0_800CA194`): a
/// camera-facing quad spun by the root's `rotate.z`, unscaled. Held, each
/// draws at the item position the pool keeps at the hold joint. Returns
/// whether `item` was one of them.
unsafe fn draw_throwable(
    p: &Pack<'_>,
    gpu: &mut Gpu,
    assets: &DrawAssets,
    pl: &play::FighterScene,
    item: &ssb_game::item::Item,
    visual: &ItemVisual,
    draw_state: &mut meshdraw::DrawState,
) -> bool {
    use ssb_game::item::ItemKind;
    let index = match item.kind {
        ItemKind::MSBomb => 0,
        ItemKind::BombHei => 1,
        ItemKind::NBumper => 2,
        ItemKind::Shell(_) => 3,
        ItemKind::MBall => 4,
        _ => return false,
    };
    let Some(object) = assets.throwable_items[index].as_ref() else {
        return true;
    };
    let node_of = |i: u32| p.node(object.first_node + i);
    let mesh_of = |i: u32| {
        node_of(i)
            .filter(|n| n.mesh != ssb_rom::pack::NodeDesc::NO_MESH)
            .and_then(|n| p.mesh(n.mesh))
    };
    let (eye, at) = (pl.camera.eye, pl.camera.at);
    let scale = meshdraw::MODEL_SCALE;
    match item.kind {
        ItemKind::BombHei | ItemKind::NBumper => {
            // `itBombHeiCommonSetWalkLR`'s list, and the Bumper's lit
            // palette and attached Wait list (RE-442).
            let mesh = match item.kind {
                ItemKind::BombHei if !item.vars.bombhei_walk_right => assets.bombhei_left,
                ItemKind::NBumper if item.vars.bumper_attached => {
                    assets.bumper_wait[usize::from(item.palette == 1)]
                }
                ItemKind::NBumper if item.palette == 1 => assets.bumper_lit,
                _ => mesh_of(1),
            };
            if let Some(mesh) = mesh {
                gpu.model_transform_billboard(
                    item.pos,
                    eye,
                    at,
                    item.rotate_z,
                    [scale * item.scale.x, scale * item.scale.y],
                );
                meshdraw::draw_mesh(p, &mesh, draw_state, None, Some(&visual.materials));
            }
        }
        ItemKind::Shell(kind) => {
            let green = (kind == ssb_game::item::shell::Kind::Green)
                .then_some(assets.green_shell)
                .flatten();
            // The slide's spin writes `rotate.y` from its first play on,
            // frozen once its script ends (RE-442).
            let rotate_y = item
                .root_script
                .and_then(|_| visual.anim.node_pose(object.first_node + 1))
                .map_or(item.vars.shell_rotate_y, |pose| pose.rotate[1]);
            if let Some(mesh) = green.or_else(|| mesh_of(1)) {
                gpu.model_transform_camera_rotated(
                    item.pos,
                    eye,
                    at,
                    [0.0, rotate_y],
                    scale * item.scale.x,
                );
                meshdraw::draw_mesh(p, &mesh, draw_state, None, Some(&visual.materials));
            }
        }
        _ => {
            let open = match item.kind {
                ItemKind::MSBomb => item.vars.msbomb_attached,
                _ => item.vars.mball_open,
            };
            if open {
                if let (Some(node), Some(mesh)) = (node_of(2), mesh_of(2)) {
                    let (s, c) = ssb_engine::math::sin_cos(item.rotate_z);
                    let [tx, ty, tz] = node.rest_translate;
                    gpu.model_transform(
                        [
                            item.pos.x + c * tx - s * ty,
                            item.pos.y + s * tx + c * ty,
                            item.pos.z + tz,
                        ],
                        [0.0, 0.0, item.rotate_z],
                        scale,
                    );
                    meshdraw::draw_mesh(p, &mesh, draw_state, None, None);
                }
            } else if item.kind == ItemKind::MBall {
                // The throw's texture cycle (RE-443).
                if let Some(mesh) = assets.mball_ball.or_else(|| mesh_of(3)) {
                    gpu.model_transform_billboard(item.pos, eye, at, item.rotate_z, [scale, scale]);
                    meshdraw::draw_mesh(p, &mesh, draw_state, None, Some(&visual.materials));
                }
            } else if let Some(mesh) = mesh_of(3) {
                gpu.model_transform_billboard(item.pos, eye, at, item.rotate_z, [scale, scale]);
                meshdraw::draw_mesh(p, &mesh, draw_state, None, None);
            }
        }
    }
    true
}

/// The mesh romtool packed under `key`.
fn mesh_keyed(p: &Pack<'_>, key: (u32, u32)) -> Option<ssb_rom::pack::MeshDesc> {
    (0..p.mesh_count())
        .filter_map(|i| p.mesh(i))
        .find(|m| (m.source_file, m.source_offset) == key)
}

/// A tree node's mesh, if it has one.
fn node_mesh(
    p: &Pack<'_>,
    object: &ssb_rom::pack::ObjectDesc,
    node: u32,
) -> Option<ssb_rom::pack::MeshDesc> {
    p.node(object.first_node + node)
        .filter(|n| n.mesh != ssb_rom::pack::NodeDesc::NO_MESH)
        .and_then(|n| p.mesh(n.mesh))
}

/// The pose a player has reached for a tree node, or its rest pose.
fn node_pose_or_rest(
    p: &Pack<'_>,
    anim: &ssb_rom::skeleton::StageAnimator,
    object: &ssb_rom::pack::ObjectDesc,
    node: u32,
) -> ssb_rom::figatree::JointPose {
    let index = object.first_node + node;
    anim.node_pose(index).copied().unwrap_or_else(|| {
        p.node(index)
            .map_or_else(Default::default, |n| ssb_rom::figatree::JointPose {
                rotate: n.rest_rotate,
                translate: n.rest_translate,
                scale: n.rest_scale,
            })
    })
}

/// `node`'s pose after `plays` plays of the packed script driving it
/// (`gcAddAnimAll` then one `gcPlayAnimAll` per update, the first at frame
/// zero), from its rest pose, or the rest pose with no script. A weapon
/// replays its own scripts from its play count rather than keep a player.
fn replay_node(
    p: &Pack<'_>,
    anim: &ssb_rom::pack::AnimDesc,
    object: &ssb_rom::pack::ObjectDesc,
    node: u32,
    plays: u16,
) -> ssb_rom::figatree::JointPose {
    let index = object.first_node + node;
    let mut pose = p
        .node(index)
        .map_or_else(Default::default, |n| ssb_rom::figatree::JointPose {
            rotate: n.rest_rotate,
            translate: n.rest_translate,
            scale: n.rest_scale,
        });
    let joint = (0..anim.joint_count)
        .filter_map(|i| p.anim_joint(anim.first_joint + i))
        .find(|j| j.node == index && j.script != ssb_rom::pack::AnimJoint::NO_SCRIPT);
    if let (Some(joint), Some(script)) = (joint, p.anim_script(anim)) {
        let mut player = ssb_rom::objanim::StageJoint::start_changed(joint.script, 0.0);
        for _ in 0..plays {
            if player.tick(script, 1.0, &mut pose).is_err() {
                break;
            }
        }
    }
    pose
}

/// A weapon tree's material scripts (`WPAttributes::p_matanim_joints`)
/// after `plays` plays, replayed rather than kept.
fn replay_materials(
    p: &Pack<'_>,
    object: &ssb_rom::pack::ObjectDesc,
    plays: u16,
) -> ssb_rom::skeleton::EffectMaterialAnimator {
    let mut mats = ssb_rom::skeleton::EffectMaterialAnimator::default();
    mats.start(p, object_mat_anims(p, object));
    for _ in 0..plays {
        mats.tick(p);
    }
    mats
}

fn local_trs(pose: &ssb_rom::figatree::JointPose) -> ssb_rom::scene::Mat4 {
    ssb_rom::scene::Mat4::from_trs(
        pose.translate.map(|t| t / meshdraw::MODEL_SCALE),
        pose.rotate,
        pose.scale,
    )
}

fn v3(a: [f32; 3]) -> ssb_engine::math::Vec3 {
    ssb_engine::math::Vec3::new(a[0], a[1], a[2])
}

/// The Poké Ball Pokémon (RE-435), each under its makers' matrix kinds.
/// Most roots are `TraRotRpyR` then kind 0x48 (`func_ovl0_800CAB48`):
/// rotation relative to the camera, scaled by the `DObj` (the appear
/// script's squash); Goldeen's child has no matrix of its own and draws
/// under it. Snorlax and Mew are plain `TraRotRpyRSca` trees in world axes.
/// Koffing's root is kind 40, facing the eye, over a `TraRotRpyRSca` child;
/// Chansey's child is kind 44 and Beedrill's kind 0x48, each placed by its
/// translation under an unrotated root. The attack display lists replace
/// Onix's, Blastoise's and Hitmonlee's models (RE-435, RE-443).
/// The Pokémon a status moved to the head of DL link 18
/// (`gcMoveGObjDLHead(item_gobj, 18, ...)`: Snorlax's fall, Starmie,
/// Hitmonlee and Clefairy's imitations of them), drawn before the link's
/// other objects (RE-443).
#[inline(never)]
fn draw_link18_monsters(
    gpu: &mut Gpu,
    p: &Pack<'_>,
    draw_state: &mut meshdraw::DrawState,
    assets: &DrawAssets,
    pl: &play::FighterScene,
    items: &ssb_game::item::ItemPool,
    effect_visuals: &EffectVisuals,
) {
    let mut ordered = [None; ssb_game::item::ITEM_ALLOC_MAX];
    let mut count = 0;
    for (index, (item, visual)) in items.items().zip(effect_visuals.items.iter()).enumerate() {
        if let ssb_game::item::ItemKind::MMonster(kind) = item.kind {
            if item.vars.mmonster.xlu && !item.hidden {
                ordered[count] = Some((
                    item.anim_ticks.wrapping_sub(item.vars.mmonster.link18_at),
                    index,
                    item,
                    kind,
                    visual,
                ));
                count += 1;
            }
        }
    }
    // Head insertion at equal priority: most recent move first; moves in
    // the same item pass run in reverse item-link order.
    ordered[..count].sort_unstable_by_key(|e| {
        let e = e.as_ref().unwrap();
        (e.0, core::cmp::Reverse(e.1))
    });
    for (_, _, item, kind, visual) in ordered[..count].iter().flatten() {
        unsafe { draw_mmonster(p, gpu, assets, pl, item, *kind, visual, draw_state) };
    }
}

#[allow(clippy::too_many_arguments)]
unsafe fn draw_mmonster(
    p: &Pack<'_>,
    gpu: &mut Gpu,
    assets: &DrawAssets,
    pl: &play::FighterScene,
    item: &ssb_game::item::Item,
    kind: ssb_game::item::mmonster::Kind,
    visual: &ItemVisual,
    draw_state: &mut meshdraw::DrawState,
) {
    use ssb_game::item::mmonster::Kind as K;
    let Some(asset) = assets.mmonsters[kind as usize].as_ref() else {
        return;
    };
    let object = &asset.object;
    let ms = meshdraw::MODEL_SCALE;
    let (eye, at) = (pl.camera.eye, pl.camera.at);
    let vars = &item.vars.mmonster;
    let pose = |n: u32| node_pose_or_rest(p, &visual.anim, object, n);
    let mats = Some(&visual.materials);
    match kind {
        K::Kabigon | K::Mew => {
            let r = pose(1);
            gpu.model_transform_xyz(
                [item.pos.x, item.pos.y, item.pos.z],
                r.rotate,
                [
                    ms * item.scale.x * r.scale[0],
                    ms * item.scale.y * r.scale[1],
                    ms * item.scale.z * r.scale[2],
                ],
            );
            if let Some(mesh) = assets
                .monster_kabigon_fall
                .filter(|_| kind == K::Kabigon && vars.xlu)
                .or_else(|| node_mesh(p, object, 1))
            {
                meshdraw::draw_mesh(p, &mesh, draw_state, None, mats);
            }
        }
        K::Dogas => {
            // The maker's `TraRotRpyRSca` resets the child before its
            // scripts drive it.
            let c = visual
                .anim
                .node_pose(object.first_node + 2)
                .copied()
                .unwrap_or_default();
            let mut base = eye_billboard(item.pos, eye);
            for v in &mut base.0[..12] {
                *v *= ms;
            }
            let mut posed = [ssb_rom::scene::Mat4::IDENTITY; 3];
            posed[2] = local_trs(&c);
            meshdraw::draw_object_posed(
                p,
                object,
                &psp_matrix(&base),
                &posed[..(object.node_count as usize).min(3)],
                None,
                draw_state,
                None,
                mats,
                0,
            );
        }
        K::MLucky => {
            let c = pose(2);
            gpu.model_transform_billboard(
                item.pos + v3(c.translate),
                eye,
                at,
                0.0,
                [ms * c.scale[0], ms * c.scale[1]],
            );
            if let Some(mesh) = node_mesh(p, object, 2) {
                meshdraw::draw_mesh(p, &mesh, draw_state, None, mats);
            }
        }
        K::Spear => {
            let c = pose(2);
            gpu.model_transform_camera_rotated_scaled(
                item.pos + v3(c.translate),
                eye,
                at,
                [c.rotate[0], vars.rotate_y],
                [ms * c.scale[0], ms * c.scale[1]],
            );
            if let Some(mesh) = node_mesh(p, object, 2) {
                meshdraw::draw_mesh(p, &mesh, draw_state, None, mats);
            }
        }
        _ => {
            let r = pose(1);
            // Charizard's attack script turns its root; the rest turn by
            // their own `rotate.y`.
            let yaw = if kind == K::Lizardon && visual.status.is_some() {
                r.rotate[1]
            } else {
                vars.rotate_y
            };
            // Clefairy's Snorlax imitation writes 5.2 to the root's
            // scale when it enters Fall, retaining its camera matrix.
            gpu.model_transform_camera_rotated_scaled(
                item.pos,
                eye,
                at,
                [0.0, yaw],
                [ms * item.scale.x * r.scale[0], ms * item.scale.y * r.scale[1]],
            );
            let mesh = assets
                .monster_pippi_xlu
                .filter(|_| kind == K::Pippi && vars.xlu)
                .or_else(|| {
                    asset
                        .attack_mesh
                        .filter(|_| vars.attack_dl)
                        .or_else(|| node_mesh(p, object, 1))
                        .or_else(|| node_mesh(p, object, 2))
                });
            if let Some(mesh) = mesh {
                meshdraw::draw_mesh(p, &mesh, draw_state, None, mats);
            }
        }
    }
}

/// The Poké Ball Pokémon's weapons (RE-435). Onix's rock and Meowth's coin
/// are `TraRotRpyR(Sca)` then kind 0x46 at their root: a camera-facing
/// quad spun by `rotate.z`, its mesh child drawn under it. Swift and the
/// Hydro Pump are `TraRotRpyRSca` trees turned by `rotate.y`; the Hydro
/// Pump's child reaches along its script. The Smog's child is kind 44,
/// sized by its script; the swarms' drawn node is kind 0x48, turned by
/// `rotate.y`. Charizard's flame is particles only. The rock selects its
/// original sprite by texture_id_curr; Clefairy's swarm uses its
/// AA_XLU_SURF variant (RE-443).
unsafe fn draw_monster_weapons(
    p: &Pack<'_>,
    gpu: &mut Gpu,
    assets: &DrawAssets,
    pl: &play::FighterScene,
    weapons: &ssb_game::weapon::WeaponPool,
    draw_state: &mut meshdraw::DrawState,
) {
    use ssb_game::monster_weapon::ShotKind as S;
    let ms = meshdraw::MODEL_SCALE;
    let (eye, at) = (pl.camera.eye, pl.camera.at);
    for shot in weapons.monster_shots() {
        let pos = shot.position;
        match shot.kind {
            S::IwarkRock | S::NyarsCoin => {
                let mesh = if shot.kind == S::IwarkRock {
                    assets
                        .monster_rock
                        .get(usize::from(shot.texture))
                        .copied()
                        .flatten()
                } else {
                    assets
                        .monster_coin
                        .as_ref()
                        .and_then(|o| node_mesh(p, o, 1))
                };
                if let Some(mesh) = mesh {
                    gpu.model_transform_billboard(pos, eye, at, shot.rotate_z, [ms, ms]);
                    meshdraw::draw_mesh(p, &mesh, draw_state, None, None);
                }
            }
            S::StarmieSwift => {
                if let Some(object) = assets.monster_swift.as_ref() {
                    gpu.model_transform(
                        [pos.x, pos.y, pos.z],
                        [0.0, shot.rotate_y, shot.rotate_z],
                        ms,
                    );
                    meshdraw::draw_object(p, object, &gpu.model_matrix(), draw_state, None, 0);
                }
            }
            S::KamexHydro => {
                if let Some((object, anim)) = assets.monster_hydro.as_ref() {
                    let mut posed = [ssb_rom::scene::Mat4::IDENTITY; 3];
                    posed[1] = local_trs(&replay_node(p, anim, object, 1, shot.plays));
                    posed[2] =
                        posed[1].mul(&local_trs(&replay_node(p, anim, object, 2, shot.plays)));
                    let mats = replay_materials(p, object, shot.plays);
                    gpu.model_transform([pos.x, pos.y, pos.z], [0.0, shot.rotate_y, 0.0], ms);
                    meshdraw::draw_object_posed(
                        p,
                        object,
                        &gpu.model_matrix(),
                        &posed[..(object.node_count as usize).min(3)],
                        None,
                        draw_state,
                        None,
                        Some(&mats),
                        0,
                    );
                }
            }
            S::DogasSmog => {
                if let Some((object, anim)) = assets.monster_smog.as_ref() {
                    let c = replay_node(p, anim, object, 1, shot.plays);
                    let mats = replay_materials(p, object, shot.plays);
                    if let Some(mesh) = node_mesh(p, object, 1) {
                        gpu.model_transform_billboard(
                            pos + v3(c.translate),
                            eye,
                            at,
                            0.0,
                            [ms * c.scale[0], ms * c.scale[1]],
                        );
                        meshdraw::draw_mesh(p, &mesh, draw_state, None, Some(&mats));
                    }
                }
            }
            S::SpearSwarm => {
                if let Some(object) = assets.monster_swarm.as_ref() {
                    let (Some(a), Some(b)) =
                        (p.node(object.first_node + 1), p.node(object.first_node + 2))
                    else {
                        continue;
                    };
                    let mats = replay_materials(p, object, shot.plays);
                    if let Some(mesh) = node_mesh(p, object, 2) {
                        gpu.model_transform_camera_rotated_scaled(
                            pos + v3(a.rest_translate) + v3(b.rest_translate),
                            eye,
                            at,
                            [b.rest_rotate[0], shot.rotate_y],
                            [ms * b.rest_scale[0], ms * b.rest_scale[1]],
                        );
                        meshdraw::draw_mesh(p, &mesh, draw_state, None, Some(&mats));
                    }
                }
            }
            S::PippiSwarm => {
                if let Some(object) = assets.monster_pippi_swarm.as_ref() {
                    if let Some(node) = p.node(object.first_node + 1) {
                        if let Some(mesh) = assets.monster_pippi_xlu {
                            gpu.model_transform_camera_rotated_scaled(
                                pos + v3(node.rest_translate),
                                eye,
                                at,
                                [node.rest_rotate[0], shot.rotate_y],
                                [ms * node.rest_scale[0], ms * node.rest_scale[1]],
                            );
                            meshdraw::draw_mesh(p, &mesh, draw_state, None, None);
                        }
                    }
                }
            }
            S::RayGun => {
                if let Some(mesh) = assets.ray_ammo.as_ref() {
                    gpu.model_transform_xyz(
                        [pos.x, pos.y, pos.z],
                        [0.0, 0.0, shot.rotate_z],
                        [ms * shot.scale_x, ms, ms],
                    );
                    meshdraw::draw_mesh(p, mesh, draw_state, None, None);
                }
            }
            S::StarRod => {
                if let Some(mesh) = assets.yoshi_star_mesh.as_ref() {
                    gpu.model_transform_billboard(pos, eye, at, shot.rotate_z, [ms, ms]);
                    meshdraw::draw_mesh(p, mesh, draw_state, None, None);
                }
            }
            S::HitokageFlame | S::FushigibanaRazor | S::LizardonFlame | S::FireFlower => {}
        }
    }
}

unsafe fn draw_items_weapons_effects(
    gpu: &mut Gpu,
    draw_state: &mut meshdraw::DrawState,
    p: &Pack<'_>,
    pl: &play::FighterScene,
    dummies: &Dummies,
    weapons: &ssb_game::weapon::WeaponPool,
    items: &ssb_game::item::ItemPool,
    assets: &DrawAssets,
    effect_visuals: &EffectVisuals,
    material_anim: Option<&ssb_rom::skeleton::MaterialAnimator>,
    part: BattlePart,
) {
    // Items (DL link 11) draw in the fighters' camera pass, under the
    // camera's default head modes. The PK Fire flame is a
    // `TraRotRpyRSca` tree scaled by its lifetime (`itNessPKFireProcUpdate`);
    // its node 3 is a ROM billboard. Link's Bomb is `Tra` then kind 46 on its
    // promoted body/root and fuse child. Loose, the body's translation is
    // the gameplay position; held, its descriptor offset sits under the
    // normalized hand joint. The fuse inherits the body's billboard frame
    // and then applies its own kind-46 spin and scale.
    if part == BattlePart::Items {
    // `itDisplayColAnim{OPA,XLU}ProcDisplay`: an item whose attributes set
    // `is_display_colanim` draws with ENV its colour animation's `color1`
    // (or clear), which its `ENV_LERP` primitives blend towards by its
    // alpha: the GE's constant fog, as the fighters' `G_RM_FOG_PRIM_A`.
    let mut env_fog = false;
    let (eye, view) = (pl.camera.eye, (pl.camera.at - pl.camera.eye).normalized());
    for (item, visual) in items.items().zip(effect_visuals.items.iter()) {
        if env_fog {
            draw_state.end_env_lerp();
            gpu.clear_fog();
            env_fog = false;
        }
        if item.hidden {
            continue;
        }
        if let Some(rgba) = item.colanim.color().filter(|c| item.attr.is_display_colanim && c[3] != 0) {
            gpu.set_constant_fog((item.pos - eye).dot(view), rgba);
            draw_state.begin_env_lerp();
            env_fog = true;
        }
            // The Bumper: its root `TraRotRpyRSca` at the item, scaled in X and Y
            // by its swell, under its lit palette (`palette_id` 1) after a hit
            // (RE-442). Its tree's node 0 and root rest at identity.
            if item.kind == ssb_game::item::ItemKind::GBumper {
                let mesh = if item.palette == 1 {
                    assets.bumper_lit
                } else {
                    assets.gbumper_item.as_ref().and_then(|object| node_mesh(p, object, 1))
                };
                if let Some(mesh) = mesh {
                    gpu.model_transform_xyz(
                        [item.pos.x, item.pos.y, item.pos.z],
                        [0.0; 3],
                        [
                            meshdraw::MODEL_SCALE * item.scale.x,
                            meshdraw::MODEL_SCALE * item.scale.y,
                            meshdraw::MODEL_SCALE * item.scale.z,
                        ],
                    );
                    meshdraw::draw_mesh(p, &mesh, draw_state, material_anim, None);
                }
                continue;
        }
        // Tomato, Heart and Star: `gcAddXObjForDObjFixed(root, 0x2E)` makes the
        // item root, node 1, a camera-facing quad spun by its `rotate.z`. Held,
        // it draws at the item-light joint, where the pool keeps it (RE-433).
        if let ssb_game::item::ItemKind::Utility(kind) = item.kind {
            let object = assets.utility_items[kind as usize - ssb_game::item::utility::Kind::Tomato as usize].as_ref();
            if let Some(mesh) = object
                .and_then(|o| p.node(o.first_node + 1))
                .filter(|n| n.mesh != ssb_rom::pack::NodeDesc::NO_MESH)
                .and_then(|n| p.mesh(n.mesh))
            {
                gpu.model_transform_billboard(item.pos, pl.camera.eye, pl.camera.at, item.rotate_z,
                    [meshdraw::MODEL_SCALE, meshdraw::MODEL_SCALE]);
                // The Star's palette flicker (RE-442).
                meshdraw::draw_mesh(p, &mesh, draw_state, None, Some(&visual.materials));
            }
            continue;
        }
        if draw_throwable(p, gpu, assets, pl, item, visual, draw_state) {
                continue;
            }
            if let ssb_game::item::ItemKind::MMonster(kind) = item.kind {
                // Moved to link 18 (draw_link18_monsters).
                if !item.vars.mmonster.xlu {
                    draw_mmonster(p, gpu, assets, pl, item, kind, visual, draw_state);
                }
                continue;
            }
            if let ssb_game::item::ItemKind::Equipment(kind) = item.kind {
                if let Some(object) = assets.equipment_items[kind as usize].as_ref() {
                    let held = item.owner.filter(|_| item.is_hold).and_then(|port| {
                        scenes_ref(pl, dummies)
                            .into_iter()
                            .flatten()
                            .find(|s| s.fighter.port == port)
                            .and_then(|s| {
                                s.fighter.joint_transforms
                                    [ssb_game::item_throw::itemlight_joint(s.fighter.kind)]
                            })
                    });
                    match held {
                        Some(joint) => {
                            gpu.model_transform_joint(joint.origin, joint, meshdraw::MODEL_SCALE)
                        }
                        None => gpu.model_transform(
                            [item.pos.x, item.pos.y, item.pos.z],
                            [0.0; 3],
                            meshdraw::MODEL_SCALE,
                        ),
                    }
                    let mut posed = [ssb_rom::scene::Mat4::IDENTITY; 4];
                    for i in 1..object.node_count as usize {
                        let Some(node) = p.node(object.first_node + i as u32) else {
                            continue;
                        };
                        let t = if i == 1 && held.is_none() {
                            [0.0; 3]
                        } else {
                            node.rest_translate.map(|x| x / meshdraw::MODEL_SCALE)
                        };
                        let r = if i == 1 {
                            [0.0, item.vars.container_root_yaw, item.rotate_z]
                        } else {
                            [
                                node.rest_rotate[0],
                                node.rest_rotate[1] + item.vars.equipment_child_yaw,
                                node.rest_rotate[2],
                            ]
                        };
                        let scale = if i == 1 && kind == ssb_game::item::equipment::Kind::Fan {
                            [
                                node.rest_scale[0] * item.scale.x,
                                node.rest_scale[1] * item.scale.y,
                                node.rest_scale[2] * item.scale.z,
                            ]
                        } else {
                            node.rest_scale
                        };
                        let local = ssb_rom::scene::Mat4::from_trs(t, r, scale);
                        posed[i] = node
                            .parent
                            .checked_sub(object.first_node)
                            .filter(|&parent| parent >= 1 && parent < i as u32)
                            .map_or(local, |parent| posed[parent as usize].mul(&local));
                    }
                    meshdraw::draw_object_posed(
                        p,
                        object,
                        &gpu.model_matrix(),
                        &posed[..object.node_count as usize],
                        None,
                        draw_state,
                        material_anim,
                        Some(&visual.materials),
                        0,
                    );
                }
                continue;
        }
        if matches!(item.kind, ssb_game::item::ItemKind::Container(_) | ssb_game::item::ItemKind::TaruBomb) {
          let kind = match item.kind {
            ssb_game::item::ItemKind::Container(kind) => kind,
            _ => ssb_game::item::container::Kind::Barrel,
          };
          if kind != ssb_game::item::container::Kind::Egg {
            let object = if item.kind == ssb_game::item::ItemKind::TaruBomb { assets.tarubomb_item.as_ref() } else { match kind {
                ssb_game::item::container::Kind::Capsule => assets.capsule_item.as_ref(),
                ssb_game::item::container::Kind::Crate => assets.heavy_items[0].as_ref(),
                ssb_game::item::container::Kind::Barrel => assets.heavy_items[1].as_ref(),
                _ => None,
            }};
            if let Some(object) = object {
                // The manager replaces root translation with the item's position.
                // Relative descendant transforms stay from the descriptor.
                let held = item.owner.filter(|_| item.is_hold).and_then(|port| {
                    scenes_ref(pl, dummies).into_iter().flatten().find(|s| s.fighter.port == port)
                        .and_then(|s| {
                            let joint = if kind.heavy() { ssb_game::grab::itemheavy_joint(s.fighter.kind) }
                                else { Some(ssb_game::item_throw::itemlight_joint(s.fighter.kind)) }?;
                            s.fighter.joint_transforms[joint]
                        })
                });
                match held {
                    Some(joint) => gpu.model_transform_joint(joint.origin, joint, meshdraw::MODEL_SCALE),
                    None => gpu.model_transform([item.pos.x, item.pos.y, item.pos.z], [0.0; 3], meshdraw::MODEL_SCALE),
                }
                // `itManagerMakeItem` ejects descriptor 0's placeholder, so the
                // item root is node 1: at the item position loose, at its own
                // descriptor translation under the attach joint (RE-432).
                let mut posed = [ssb_rom::scene::Mat4::IDENTITY; 4];
                for i in 1..object.node_count as usize {
                    let Some(node) = p.node(object.first_node + i as u32) else { continue; };
                    let local = if i == 1 {
                        let t = if held.is_some() { node.rest_translate.map(|x| x / meshdraw::MODEL_SCALE) } else { [0.0; 3] };
                        ssb_rom::scene::Mat4::from_trs(t, [item.vars.container_root_pitch, item.vars.container_root_yaw, item.rotate_z], node.rest_scale)
                    } else {
                        ssb_rom::scene::Mat4::from_trs(node.rest_translate.map(|x| x / meshdraw::MODEL_SCALE), node.rest_rotate, node.rest_scale)
                    };
                    posed[i] = node.parent.checked_sub(object.first_node).filter(|&parent| parent >= 1 && parent < i as u32).map_or(local, |parent| posed[parent as usize].mul(&local));
                }
                let base = gpu.model_matrix();
                meshdraw::draw_object_posed(p, object, &base, &posed[..object.node_count as usize], None, draw_state, material_anim, None, 0);
            }
            continue;
          }
        }
        let Some((object, _)) = assets.item(item.kind) else {
            continue;
        };
        match item.kind {
            ssb_game::item::ItemKind::Container(ssb_game::item::container::Kind::Egg) => {
                // Maker inserts kind 46 on child 1; its child 2 carries the
                // mesh and ROM scale script. Egg copies root spin to child 1.
                let Some(node) = p.node(object.first_node + 2) else { continue; };
                let Some(mesh) = p.mesh(node.mesh) else { continue; };
                let pose = stage_pose(&visual.anim, object.first_node + 2).unwrap_or(ssb_rom::figatree::JointPose {
                    rotate: node.rest_rotate, translate: node.rest_translate, scale: node.rest_scale,
                });
                gpu.model_transform_billboard(item.pos, pl.camera.eye, pl.camera.at, item.rotate_z,
                    [meshdraw::MODEL_SCALE * item.scale.x * pose.scale[0], meshdraw::MODEL_SCALE * item.scale.y * pose.scale[1]]);
                meshdraw::draw_mesh(p, &mesh, draw_state, None, Some(&visual.materials));
            }
            ssb_game::item::ItemKind::NessPKFire => {
                let mut posed = [ssb_rom::scene::Mat4::IDENTITY; 8];
                let mut root = node_pose_or_rest(p, &visual.anim, object, 1);
                root.translate = [0.0; 3];
                let n = visual.anim.compose_item(p, object, root, &mut posed);
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
                    scenes_ref(pl, dummies)
                        .into_iter()
                        .flatten()
                        .map(|x| &x.fighter)
                        .find(|f| f.port == port)
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
                // Descriptor 0 is gone. Node 1 is the body/root, retaining
                // its hand offset only under the attach joint. Bloat writes
                // this same node loose and held (root->child when held).
                let root_pos = if held.is_some() { to_world(v(&body)) } else { item.pos };
                let root_scale = [item.scale.x * body.scale[0], item.scale.y * body.scale[1]];
                gpu.model_transform_billboard(
                    root_pos, pl.camera.eye, pl.camera.at, body.rotate[2],
                    root_scale.map(|s| meshdraw::MODEL_SCALE * s),
                );
                let parent = gpu.model_matrix();
                if let Some(mesh) = mesh_of(1) {
                    meshdraw::draw_mesh(p, &mesh, draw_state, None, Some(&visual.materials));
                }
                // The maker adds kind 46 to root->child too: its position
                // inherits the body's billboard matrix; its axes then reset
                // to the camera, with its own spin/scale and gGCScaleX.
                if let Some(mesh) = mesh_of(2) {
                    let [x, y, z] = spark.translate.map(|v| v / meshdraw::MODEL_SCALE);
                    let pos = ssb_engine::math::Vec3::new(
                        parent.w.x + parent.x.x * x + parent.y.x * y + parent.z.x * z,
                        parent.w.y + parent.x.y * x + parent.y.y * y + parent.z.y * z,
                        parent.w.z + parent.x.z * x + parent.y.z * y + parent.z.z * z,
                    );
                    gpu.model_transform_billboard(pos, pl.camera.eye, pl.camera.at, spark.rotate[2],
                        [meshdraw::MODEL_SCALE * root_scale[0] * spark.scale[0],
                         meshdraw::MODEL_SCALE * root_scale[0] * spark.scale[1]]);
                    meshdraw::draw_mesh(p, &mesh, draw_state, None, Some(&visual.materials));
                }
                }
                // The Bumper drew above; the POW Block and the Piranha Plants are
                // stage ground objects.
                _ => {}
        }
    }
    if env_fog {
        draw_state.end_env_lerp();
        gpu.clear_fog();
    }
    }

    if part == BattlePart::Weapons {
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
        // `wpMainVelSetModelPitch` sets `rotate.y` to +/-90 degrees from
        // the horizontal velocity and `wpMarioFireballProcUpdate` spins
        // `rotate.x`; the DObj's second transform, battle matrix function
        // 0x47, turns both relative to the camera, so the Fireball rolls in
        // the screen plane (RE-418).
        let yaw = if fireball.velocity.x >= 0.0 {
            core::f32::consts::FRAC_PI_2
        } else {
            -core::f32::consts::FRAC_PI_2
        };
        gpu.model_transform_camera_rotated(
            fireball.position,
            pl.camera.eye,
            pl.camera.at,
            [fireball.rotate_x, yaw],
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

    // Master Hand's bullets: `TraRotRpyRSca` turned along the velocity by
    // `wpMainReflectorRotateWeaponModel`; the burst has no display list.
    if let Some(bullet) = assets.boss_bullet.as_ref() {
        for shot in weapons.boss_bullets().filter(|b| !b.exploded) {
            // `syUtilsArcTan2` of the cross product is scale-free.
            let (vx, vy) = (shot.velocity.x, shot.velocity.y);
            let (yaw, pitch) = if vx > 0.0 {
                (core::f32::consts::FRAC_PI_2, ssb_engine::math::atan2(-vy, vx))
            } else {
                (-core::f32::consts::FRAC_PI_2, ssb_engine::math::atan2(-vy, -vx))
            };
            gpu.model_transform_xyz(
                [shot.position.x, shot.position.y, shot.position.z],
                [pitch, yaw, 0.0],
                [meshdraw::MODEL_SCALE; 3],
            );
            let base = gpu.model_matrix();
            meshdraw::draw_object(p, bullet, &base, draw_state, None, 0);
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

    // Sector Z's Arwing lasers (RE-428): a `TraRotRpyR` DObj drawing file
    // 153's list, faced along the shot. The 3D shot's burst clears it.
    if let Some(laser_mesh) = assets.arwing_laser_mesh.as_ref() {
        for laser in weapons.arwing_lasers().filter(|l| !l.exploded) {
            gpu.model_transform(
                [laser.position.x, laser.position.y, laser.position.z],
                [laser.rotate.x, laser.rotate.y, laser.rotate.z],
                meshdraw::MODEL_SCALE,
            );
            meshdraw::draw_mesh(p, laser_mesh, draw_state, None, None);
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
    if let Some(object) = assets.monster_razor.as_ref() {
        for shot in weapons.monster_shots().filter(|m| m.is_razor()) {
            gpu.model_transform_xyz(
                [shot.position.x, shot.position.y, shot.position.z],
                [0.0, 0.0, shot.rotate_z],
                [meshdraw::MODEL_SCALE; 3],
            );
            meshdraw::draw_object(p, object, &gpu.model_matrix(), draw_state, None, 0);
        }
    }
        draw_monster_weapons(p, gpu, assets, pl, weapons, draw_state);
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

    // Pikachu's Thunder head and trails (RE-417): `TraRotRpyRSca` DObjs at
    // half scale, never turned, drawing `sprites[texture_id_curr]`: 3 for
    // the head, the frame each update draws for a trail.
    let thunder = |gpu: &mut Gpu, draw_state: &mut meshdraw::DrawState, pos: ssb_engine::math::Vec3, frame: u8| {
        let Some(mesh) = assets.thunder_frames.get(usize::from(frame)).and_then(Option::as_ref) else {
            return;
        };
        gpu.model_transform([pos.x, pos.y, pos.z], [0.0; 3], meshdraw::MODEL_SCALE * THUNDER_SCALE);
        meshdraw::draw_mesh(p, mesh, draw_state, None, None);
    };
    for head in weapons.thunder_heads() {
        thunder(gpu, draw_state, head.position, THUNDER_HEAD_FRAME);
    }
    for trail in weapons.thunder_trails() {
        thunder(gpu, draw_state, trail.position, trail.texture);
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
    }

    if part != BattlePart::Effects15 {
        return;
    }

    // The shield bubble (`efManagerShieldMakeEffect`, RE-384). Its root is
    // battle matrix function 79, `YRotN`'s whole matrix, which the guard
    // scales by the shield size; the drawn node adds kind 44, a camera-facing
    // quad sized by that accumulated scale. `efManagerShieldProcDisplay` sets
    // PRIM white and ENV the player's colour, both at alpha 0xC0, or the
    // grey damage row on a set-off's frame (RE-418). Yoshi's egg shield is
    // a different effect, drawn below.
    if let Some(object) = assets.shield.as_ref() {
        let fighters = scenes_ref(pl, dummies).into_iter().flatten().map(|x| &x.fighter);
        for f in fighters.filter(|f| f.guard.is_shield && f.kind != ssb_game::fighter::FighterKind::Yoshi) {
            let joint = ssb_game::combat::shield_transform(f);
            let size = joint.axes[0].length();
            let (prim, env) = ssb_psp_runtime::scene::SHIELD_COLORS[ssb_game::combat::shield_color_row(f)];
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

    // Yoshi's egg shield (`dEFManagerYoshiShieldEffectDesc`, RE-418): file
    // 338's direct list at 0xA860, the Egg Throw's egg, under a kind-0x50
    // root (a translation to `YRotN`'s world position) then kind 44, a
    // camera-facing quad scaled 1.5 in X and Y. `efManagerYoshiShieldProcDisplay`
    // sets ENV from the shield's health; the list's `(SHADE - ENV) * TEXEL0`
    // subtracts it, so the egg darkens (green and blue faster than red)
    // as the shield wears.
    if let Some(egg_mesh) = assets.yoshi_egg_mesh.as_ref() {
        let fighters = scenes_ref(pl, dummies).into_iter().flatten();
        for scene in fighters.filter(|s| {
            ssb_game::combat::is_yoshi_egg_shield(&s.fighter) || s.fighter.yoshi.egg_escape_active
        }) {
            let f = &scene.fighter;
            let [r, g, b] = ssb_game::combat::yoshi_shield_env(f);
            draw_state.color_override = Some(ssb_rom::skeleton::EffectColors {
                env: Some([r, g, b, 0]),
                ..Default::default()
            });
            let scale = meshdraw::MODEL_SCALE * ssb_game::combat::YOSHI_SHIELD_SCALE;
            // Roll egg: kind 0x50 translates to joint 5; kind 0x4A
            // (`func_ovl0_800CB2F0`) copies signed local rotate.x into z;
            // kind 0x2E then builds the scaled, spun billboard.
            let (origin, spin) = if f.yoshi.egg_escape_active {
                let angle = p.object(scene.object)
                    .and_then(|o| scene.skeleton.node_pose(o.first_node + 1))
                    .map_or(0.0, |p| p.rotate[0]);
                let positive = f.joint_transforms[5].is_some_and(|j| j.axes[0].z > 0.0);
                (f.joint_world(5, ssb_engine::math::Vec3::ZERO), if positive { angle } else { -angle })
            } else {
                (ssb_game::combat::shield_transform(f).origin, 0.0)
            };
            gpu.model_transform_billboard(
                origin,
                pl.camera.eye,
                pl.camera.at,
                spin,
                [scale, scale],
            );
            meshdraw::draw_mesh(p, egg_mesh, draw_state, None, None);
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
