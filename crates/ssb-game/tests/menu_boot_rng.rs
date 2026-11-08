//! The seed a real boot carries into a VS battle (PLAN "real menu boot").
//!
//! N64 fixture (Mupen64Plus HLE, stock US ROM from reset, P1 route below;
//! `sSYUtilsRandomSeed` 0x8003B940 and `scene_curr` 0x800A4AD0 read every
//! frame; identical with `RandomizeInterrupt` on and off): the N64 logo,
//! the opening room (START at frame 200), the title (START at 502), the
//! mode select (down at 564, A at 596), the VS menu (A at 678), the VS
//! character select (Mario, then port 2 to CPU; START at 990) and the
//! stage select (A on the default Castle at 1142) draw nothing: the seed
//! is still 1 when the battle starts. Its first two draws give 0xC823F683,
//! as on the Castle warp ROM (RE-568), so the warp-ROM traces start from
//! the real boot's seed.
//!
//! The port's scenes take the same inputs, at the same frames after each
//! N64 scene change, and must draw nothing either. Button presses are held
//! two frames, as the N64 route held them.

use ssb_engine::input::{ControllerState, N64Buttons};
use ssb_game::backup::Backup;
use ssb_game::demo_fighter::DemoFighter;
use ssb_game::fighter::FighterKind;
use ssb_game::menu::mode_select::ModeSelect;
use ssb_game::menu::title::{DemoData, Title};
use ssb_game::menu::{Pad, Scene};
use ssb_game::players_vs::{self, BattleState, PlayersVs, SceneContext, PLAYERS};
use ssb_game::stage_select::{self, StageSelect};
use ssb_game::vs_mode::VsMode;

/// `syUtilsRandomSeed`'s start: nothing has drawn.
const BOOT_SEED: i32 = 1;

/// One route step: hold `buttons` and the stick for `frames` frames.
#[derive(Clone, Copy)]
struct Step {
    frames: u32,
    buttons: u16,
    x: i8,
    y: i8,
}

const fn idle(frames: u32) -> Step {
    Step {
        frames,
        buttons: 0,
        x: 0,
        y: 0,
    }
}

const fn press(buttons: u16) -> Step {
    Step {
        frames: 2,
        buttons,
        x: 0,
        y: 0,
    }
}

const fn stick(frames: u32, x: i8, y: i8) -> Step {
    Step {
        frames,
        buttons: 0,
        x,
        y,
    }
}

/// The route's frames as menu pads, a press tapping on its first frame.
fn pads(steps: &[Step]) -> Vec<Pad> {
    let mut out = Vec::new();
    let mut prev = 0u16;
    for s in steps {
        for _ in 0..s.frames {
            out.push(Pad {
                hold: s.buttons,
                tap: s.buttons & !prev,
                stick_x: s.x,
                stick_y: s.y,
            });
            prev = s.buttons;
        }
    }
    out
}

/// Runs `tick` over the pads, then idle, until it returns a scene; the
/// tick it did, counted from 0.
fn run_until<T>(pads: &[Pad], limit: usize, mut tick: impl FnMut(&Pad) -> Option<T>) -> (usize, T) {
    for i in 0..limit {
        let pad = pads.get(i).copied().unwrap_or_default();
        if let Some(out) = tick(&pad) {
            assert_eq!(ssb_game::rng::seed(), BOOT_SEED, "a draw by tick {i}");
            return (i, out);
        }
        assert_eq!(ssb_game::rng::seed(), BOOT_SEED, "a draw at tick {i}");
    }
    panic!("no scene change in {limit} ticks");
}

/// A menu pad as the controller the select scenes read.
fn controller(p: &Pad) -> ControllerState {
    ControllerState {
        buttons: N64Buttons(p.hold),
        stick_x: p.stick_x,
        stick_y: p.stick_y,
        connected: true,
    }
}

const START: u16 = N64Buttons::START;
const A: u16 = N64Buttons::A;
const DOWN: u16 = N64Buttons::D_DOWN;

#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn the_menus_to_a_vs_battle_draw_nothing_as_on_the_n64() {
    assert_eq!(ssb_game::rng::seed(), BOOT_SEED);
    let mut backup = Backup::default();

    // The title, entered from the skipped opening room (N64 frame 200):
    // START 302 frames on.
    let mut demo = DemoData::default();
    ssb_game::menu::title::count_boot(&demo, &mut backup);
    let mut title = Title::new(0);
    let route = pads(&[idle(302), press(START)]);
    let mut range = |r: i32| ssb_game::rng::rand_int_range(r);
    let (_, next) = run_until(&route, 2000, |p| {
        title.tick(
            p,
            Scene::Opening(ssb_game::opening::Kind::Room),
            &mut demo,
            &backup,
            0,
            &mut range,
        )
    });
    assert_eq!(next, Scene::ModeSelect);

    // The mode select (N64 502): down at 62, A at 94.
    let mut mode = ModeSelect::new(Scene::Title);
    let route = pads(&[idle(62), press(DOWN), idle(30), press(A)]);
    let (_, next) = run_until(&route, 600, |p| mode.tick(p));
    assert_eq!(next, Scene::VsMode);

    // The VS menu (N64 596): A on "VS Start" at 82.
    let mut state = BattleState::default();
    let mut vs = VsMode::new(Scene::ModeSelect, &state);
    let route = pads(&[idle(82), press(A)]);
    let (_, next) = run_until(&route, 600, |p| vs.tick(p, &mut state));
    assert_eq!(next, Scene::PlayersVs);

    // The VS character select (N64 678), one controller (the PSP's): the
    // cursor up onto Mario, A; down onto port 2's button, A (N64: HMN to
    // CP; here N/A to CP); START. The preview fighters run their scripts.
    let ctx = SceneContext {
        fighter_mask: backup.fighter_mask,
        unlock_mask: backup.unlock_mask,
        gkind: stage_select::DEFAULT_GKIND,
        from_maps: false,
    };
    let mut select = PlayersVs::new(state, ctx, [true, false, false, false]);
    let route = pads(&[
        idle(152),
        stick(28, 0, 80),
        stick(16, 40, 0),
        idle(10),
        press(A),
        idle(20),
        stick(19, 0, -80),
        stick(13, 80, 0),
        idle(10),
        press(A),
        idle(40),
        press(START),
    ]);
    let mut fighters: [Option<(u32, DemoFighter)>; PLAYERS] = Default::default();
    let mut effects = 0;
    let mut shown = Vec::new();
    // `osGetTime() & 0xFF` for the CPU's fighter: Kirby, as on the N64 run.
    let (at, out) = run_until(&route, 2000, |p| {
        let port0 = players_vs::Pad {
            state: controller(p),
            taps: N64Buttons(p.tap),
        };
        let quads = [
            port0,
            players_vs::Pad::default(),
            players_vs::Pad::default(),
            players_vs::Pad::default(),
        ];
        let out = select.tick(&quads, &mut || 180u8);
        for (i, slot) in fighters.iter_mut().enumerate() {
            let Some(f) = select.view.slots[i].fighter else {
                *slot = None;
                continue;
            };
            let row = f.status.map_or(0, |s| 1 + s.index());
            match slot {
                Some((serial, d)) if *serial == f.serial && d.fighter.kind == f.kind => {
                    let frame = d.fighter.status.anim_frame + 1.0;
                    d.tick(frame);
                }
                _ => {
                    shown.push((i, f.kind, row));
                    *slot = Some((f.serial, DemoFighter::start(f.kind, i as u8, 0, row)));
                }
            }
            if let Some((_, d)) = slot {
                effects += d.fighter.effects.len();
                d.fighter.effects.clear();
            }
        }
        out
    });
    // N64: START at 312, `scene_curr` to the stage select at 342.
    assert_eq!(at, 342);
    assert!(shown
        .iter()
        .any(|&(i, k, row)| i == 0 && k == FighterKind::Mario && row > 0));
    assert!(shown
        .iter()
        .any(|&(i, k, row)| i == 1 && k == FighterKind::Kirby && row > 0));
    assert_eq!(effects, 0, "the preview fighters made effects");
    let players_vs::Outcome::Maps(state) = out else {
        panic!("not the stage select: {out:?}");
    };
    assert_eq!(state.players[0].fkind, Some(FighterKind::Mario));
    assert_eq!(state.players[1].fkind, Some(FighterKind::Kirby));

    // The stage select (N64 1020): A on the default stage at 122.
    let mut maps = StageSelect::new(stage_select::DEFAULT_GKIND, backup.unlock_mask);
    let route = pads(&[idle(122), press(A)]);
    let (_, out) = run_until(&route, 600, |p| maps.tick(controller(p), N64Buttons(p.tap)));
    assert_eq!(
        out,
        stage_select::Outcome::Confirm {
            gkind: stage_select::gkind::CASTLE
        }
    );

    // The battle starts from the boot seed, as on the N64.
    assert_eq!(ssb_game::rng::seed(), BOOT_SEED);
}

/// N64 fixture (same harness): START in the opening room at frame 200,
/// then no input. The title's demo pick (`mnTitleProceedDemoNext`, two
/// `syUtilsRandIntRange` draws) changes the seed to 0xC823F683 at frame
/// 1220, 1020 frames on (tic 1190: a boot starts with the extended wait),
/// and How to Play follows.
#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn an_idle_title_after_a_skipped_opening_picks_its_demo_at_tic_1190_as_on_the_n64() {
    let mut backup = Backup::default();
    let mut demo = DemoData::default();
    ssb_game::menu::title::count_boot(&demo, &mut backup);
    let mut title = Title::new(0);
    let mut range = |r: i32| ssb_game::rng::rand_int_range(r);
    let mut first_draw = None;
    let mut next = None;
    for tick in 0..1100 {
        let out = title.tick(
            &Pad::default(),
            Scene::Opening(ssb_game::opening::Kind::Room),
            &mut demo,
            &backup,
            0,
            &mut range,
        );
        if first_draw.is_none() && ssb_game::rng::seed() != BOOT_SEED {
            first_draw = Some((tick, ssb_game::rng::seed() as u32));
        }
        next = next.or(out);
    }
    assert_eq!(first_draw, Some((1020, 0xC823_F683)));
    assert_eq!(next, Some(Scene::Explain));
}
