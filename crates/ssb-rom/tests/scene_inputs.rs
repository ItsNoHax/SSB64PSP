//! The golden scenes' input scripts replayed through the portable fighter,
//! asserting that each fighter reaches the move its scene is named for.
//!
//! An A at tick 8 starts a jab; a later B inside that jab is swallowed, the
//! fighter never starts the special, and the golden shows a fighter standing
//! still next to the move's leftovers (RE-516 found `samus`, `samusshot` and
//! `donkey` this way). The scripts live in `ssb_capture::script`, shared
//! with `psp-game`, so this test reads the very schedule the capture runs.
//!
//! The replay is the fighter alone on Dream Land's floors (the slope
//! scenes': their stage's, [`scene_stage`]): no dummy, items,
//! camera or skeleton (TransN root motion is not applied, so statuses that
//! travel on their animation, like Final Cutter, are only checked for being
//! entered). Scenes that need the dummy or items stay out of it: `kirbyhat`
//! and the item scenes have the golden's capture log (`player_status`) as
//! their witness, and `yoshiegg` its own replay, which poses Yoshi and the
//! dummy from the pack as the runtime does, since the catch reads the
//! tongue's posed joint. Set `SCENE_TRACE=1` to print every scene's status
//! timeline.

use ssb_capture::script::{self, button};
use ssb_capture::GameScene;
use ssb_engine::input::{ControllerState, N64Buttons};
use ssb_game::fighter::FighterKind;
use ssb_game::status::Status;

#[path = "common/posed.rs"]
mod posed;
use posed::{tick_pair, Pair, Posed};

#[path = "common/dream_land.rs"]
mod dream_land;
use dream_land::{dream_land, dream_land_map, stage_floors, training_fighter};

/// The pack stage a scene's Training runs on (`capture_stage_gkind` in
/// `psp-game`): Dream Land but for the slope scenes.
fn scene_stage(scene: GameScene) -> u32 {
    match scene {
        // Kongo Jungle (`gkind::JUNGLE`).
        GameScene::KirbyStoneSlope => 6,
        // Peach's Castle (`gkind::CASTLE`).
        GameScene::CaptainKickRoll | GameScene::KirbyCutter => 4,
        _ => 0,
    }
}

/// The first tick the fighter sees input: Training's first battle frame
/// (the menu's A at tick 8 has confirmed the mode on its own frame).
const FIRST_TICK: u64 = 8;

/// What a scene's fighter must do.
struct Intent {
    scene: GameScene,
    kind: FighterKind,
    /// Debug names (substrings) of statuses; one of them must be entered
    /// at or after `since` and at or before the capture tick.
    reaches: &'static [&'static str],
    since: u64,
    /// When set, the status at the capture tick must contain one of these.
    at_capture: &'static [&'static str],
}

const fn intent(
    scene: GameScene,
    kind: FighterKind,
    reaches: &'static [&'static str],
    since: u64,
    at_capture: &'static [&'static str],
) -> Intent {
    Intent {
        scene,
        kind,
        reaches,
        since,
        at_capture,
    }
}

fn intents() -> Vec<Intent> {
    use FighterKind as K;
    use GameScene as S;
    vec![
        intent(S::Fireball, K::Mario, &["SpecialN"], 150, &["SpecialN"]),
        intent(S::Superjump, K::Mario, &["SpecialHi"], 150, &[]),
        intent(S::Fox, K::Fox, &["SpecialN", "SpecialAirN"], 40, &[]),
        intent(S::Luigi, K::Luigi, &["SpecialN", "SpecialAirN"], 40, &[]),
        intent(S::Samus, K::Samus, &["SpecialN"], 30, &["SpecialNLoop"]),
        intent(
            S::SamusShot,
            K::Samus,
            &["SpecialNEnd"],
            60,
            &["SpecialNEnd"],
        ),
        intent(S::SamusSpark, K::Samus, &["AttackHi4"], 50, &["AttackHi4"]),
        intent(
            S::SamusBomb,
            K::Samus,
            &["SpecialLw", "SpecialAirLw"],
            45,
            &[],
        ),
        intent(S::Link, K::Link, &["SpecialN", "SpecialAirN"], 45, &[]),
        intent(S::LinkSpin, K::Link, &["SpecialHi"], 45, &[]),
        intent(S::LinkSword, K::Link, &["Attack11"], 50, &["Attack11"]),
        intent(S::Yoshi, K::Yoshi, &["SpecialHi"], 45, &[]),
        intent(S::YoshiBomb, K::Yoshi, &["SpecialLw"], 45, &[]),
        intent(S::Captain, K::Captain, &["SpecialN"], 45, &[]),
        intent(S::CaptainKick, K::Captain, &["SpecialLw"], 45, &[]),
        intent(S::Kirby, K::Kirby, &["SpecialHi"], 45, &[]),
        intent(S::KirbyJab, K::Kirby, &["Attack100"], 50, &["Attack100"]),
        intent(S::Pikachu, K::Pikachu, &["SpecialN"], 45, &[]),
        intent(S::PikachuAir, K::Pikachu, &["SpecialAirN"], 56, &[]),
        intent(
            S::PikachuSmash,
            K::Pikachu,
            &["AttackS4"],
            50,
            &["AttackS4"],
        ),
        intent(
            S::PikachuThunder,
            K::Pikachu,
            &["SpecialLw", "SpecialAirLw"],
            90,
            &[],
        ),
        intent(S::Purin, K::Purin, &["SpecialHi"], 45, &[]),
        intent(S::Donkey, K::Donkey, &["SpecialN"], 30, &["SpecialN"]),
        intent(S::Ness, K::Ness, &["SpecialN"], 45, &[]),
        intent(S::NessThunder, K::Ness, &["SpecialHi"], 45, &["SpecialHi"]),
        // No PK Thunder here, so no jibaku: the golden's capture log has
        // `SpecialAirHiJibaku` at 227.
        intent(S::NessJibaku, K::Ness, &["SpecialHi"], 140, &["SpecialHi"]),
        intent(
            S::KirbyStoneSlope,
            K::Kirby,
            &["SpecialLwHold"],
            120,
            &["SpecialLwHold"],
        ),
        // The kick runs on its TransN, not applied here: the golden's
        // capture log has `SpecialLwAir` at 100.
        intent(S::CaptainKickRoll, K::Captain, &["SpecialLw"], 60, &[]),
        // Final Cutter rises on its TransN, not applied here: the golden's
        // capture log has `SpecialHi` at the capture.
        intent(S::KirbyCutter, K::Kirby, &["SpecialHi"], 95, &[]),
        intent(S::NessMagnet, K::Ness, &["SpecialLw"], 45, &["SpecialLw"]),
        intent(S::LinkBomb, K::Link, &["SpecialLw"], 45, &[]),
        intent(S::Grab, K::Mario, &["Catch"], 125, &["Catch"]),
        intent(S::Jab, K::Mario, &["Attack11"], 125, &["Attack11"]),
        intent(S::Shadows, K::Mario, &["KneeBend", "Jump"], 30, &["Jump"]),
        intent(S::Shield, K::Mario, &["Guard"], 30, &["Guard"]),
        intent(S::YoshiShield, K::Yoshi, &["Guard"], 30, &["Guard"]),
        intent(S::YoshiRollF, K::Yoshi, &["EscapeF"], 55, &["EscapeF"]),
        intent(S::YoshiRollB, K::Yoshi, &["EscapeB"], 55, &["EscapeB"]),
        intent(S::YoshiShieldBreak, K::Yoshi, &["Guard"], 30, &[]),
        intent(S::FoxFire, K::Fox, &["SpecialAirHi"], 45, &["SpecialAirHi"]),
        intent(
            S::PikachuQuick,
            K::Pikachu,
            &["SpecialAirHi"],
            45,
            &["SpecialAirHi"],
        ),
    ]
}

/// One scene's replay: `(tick, status)` at every status change, and the
/// status at the capture tick.
fn replay(
    pack: &ssb_rom::pack::Pack<'_>,
    scene: GameScene,
    kind: FighterKind,
) -> (Vec<(u64, String)>, String, f32) {
    let (stage, segments) = stage_floors(pack, scene_stage(scene));
    let floors = || segments.iter().copied();
    let mut f = training_fighter(pack, &stage, kind, 0, &segments);

    let capture = script::capture_tick(scene).expect("a fighter scene has a capture tick");
    let mut timeline = Vec::new();
    let mut last = String::new();
    let mut jump_was_held = false;
    for tick in FIRST_TICK..capture {
        let pad = script::fighter_pad(scene, tick).expect("a fighter scene has a pad");
        let jump_held = pad.buttons
            & (N64Buttons::C_UP | N64Buttons::C_DOWN | N64Buttons::C_LEFT | N64Buttons::C_RIGHT)
            != 0;
        let tapped = jump_held && !jump_was_held;
        let released = !jump_held && jump_was_held;
        jump_was_held = jump_held;
        f.set_input(
            ControllerState {
                buttons: N64Buttons(pad.buttons),
                stick_x: pad.stick_x,
                stick_y: pad.stick_y,
                connected: true,
            },
            tapped,
            released,
        );
        f.tick(floors);
        let status = format!("{:?}", f.status.status);
        if status != last {
            timeline.push((tick, status.clone()));
            last = status;
        }
    }
    if std::env::var_os("SCENE_TRACE").is_some() {
        let dummy = pack.spawn(&stage, 1).map_or(f32::NAN, |p| f32::from(p.x));
        println!(
            "{:14} ends at x {:.1}, y {:.1} (dummy spawn x {dummy})",
            scene.name(),
            f.pos.x,
            f.pos.y
        );
    }
    (timeline, last, f.pos.x)
}

fn open_pack() -> Option<Vec<u8>> {
    ssb_tablegen::test_rom(env!("CARGO_MANIFEST_DIR"))?;
    std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/generated/ssb64.pak"),
    )
    .ok()
}

#[test]
fn script_buttons_match_the_engine() {
    assert_eq!(button::A, N64Buttons::A);
    assert_eq!(button::B, N64Buttons::B);
    assert_eq!(button::Z, N64Buttons::Z);
    assert_eq!(button::C_UP, N64Buttons::C_UP);
    assert_eq!(button::L, N64Buttons::L);
}

#[test]
fn every_fighter_scene_has_an_intent() {
    let covered: Vec<GameScene> = intents().iter().map(|i| i.scene).collect();
    for scene in GameScene::ALL {
        if script::is_fighter_scene(scene) {
            // `kirbyhat`, `kirbyspit`, `kirbylose` and `yoshiegg` need the
            // dummy: out of this replay (`yoshiegg` has its own).
            let outside = matches!(
                scene,
                GameScene::KirbyHat
                    | GameScene::KirbySpit
                    | GameScene::KirbyLose
                    | GameScene::KirbyWiggle
                    | GameScene::YoshiEgg
                    | GameScene::PurinThrow
            );
            assert_eq!(
                covered.contains(&scene),
                !outside,
                "{} must have exactly one intent unless it needs the dummy",
                scene.name()
            );
        }
    }
}

#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
#[test]
fn fighter_scenes_reach_the_move_they_are_named_for() {
    let Some(bytes) = open_pack() else {
        return;
    };
    let pack = ssb_rom::pack::Pack::open(&bytes).unwrap();
    let trace = std::env::var_os("SCENE_TRACE").is_some();
    let mut failures = Vec::new();
    for i in intents() {
        let (timeline, at_capture, x) = replay(&pack, i.scene, i.kind);
        let capture = script::capture_tick(i.scene).unwrap();
        if trace {
            let line: Vec<String> = timeline.iter().map(|(t, s)| format!("{t}:{s}")).collect();
            println!(
                "{:14} capture {capture}: {}",
                i.scene.name(),
                line.join(" ")
            );
        }
        // A status entered at `since` or later, or still held from before
        // `since` into a tick at or after it, does not count: the press
        // has to have started it.
        let entered = timeline
            .iter()
            .any(|(t, s)| *t >= i.since && i.reaches.iter().any(|r| s.contains(r)));
        if !entered {
            failures.push(format!(
                "{}: never enters {:?} from tick {} (timeline {:?})",
                i.scene.name(),
                i.reaches,
                i.since,
                timeline
            ));
        }
        // The grab and the jab take a route onto the dummy's platform that
        // `romtool jumptest` found: Mario lands at x -1253, 144 units right
        // of the dummy. A swallowed jump lands him 370 units short.
        if matches!(i.scene, GameScene::Grab | GameScene::Jab) && !(-1260.0..-1245.0).contains(&x) {
            failures.push(format!(
                "{}: ends at x {x}, want about -1253",
                i.scene.name()
            ));
        }
        if !i.at_capture.is_empty() && !i.at_capture.iter().any(|r| at_capture.contains(r)) {
            failures.push(format!(
                "{}: is {at_capture} at its capture tick {capture}, want {:?}",
                i.scene.name(),
                i.at_capture
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "swallowed inputs:\n{}",
        failures.join("\n")
    );
}

/// `scene`'s fighter pad as `(buttons, stick x, stick y)`.
fn scene_pad(scene: GameScene) -> impl Fn(u64) -> (u16, i8, i8) {
    move |tick| {
        let pad = script::fighter_pad(scene, tick).unwrap();
        (pad.buttons, pad.stick_x, pad.stick_y)
    }
}

/// `yoshiegg`: Yoshi walks under the left platform, jumps onto it and
/// tongues the dummy standing there; the dummy is swallowed
/// (`CaptureYoshi`), laid in an egg (`YoshiEgg`) and is still in it at the
/// capture tick. The catch box is the tongue tip, joint 31, so both
/// fighters are posed from the pack as the runtime does, in the match
/// loop's order (`common/posed.rs`, after `explain_movement`). The dummy is
/// Training's standing CPU (level 3), whose pad stays neutral until it
/// wiggles the egg 90 ticks in (`ftComputer`'s `YoshiEgg` wiggle), after
/// the capture; this replay holds it neutral throughout. The PPSSPP capture
/// log of the same script catches at tick 267 and lays the egg at 293.
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
#[test]
fn yoshi_egg_lays_the_dummy_in_an_egg() {
    let Some(bytes) = open_pack() else {
        return;
    };
    let pack = ssb_rom::pack::Pack::open(&bytes).unwrap();
    let (stage, segments) = dream_land(&pack);
    let map = dream_land_map(&pack, &stage);
    let mut fighters: Vec<Posed> = [(FighterKind::Yoshi, 0), (FighterKind::Mario, 1)]
        .into_iter()
        .map(|(kind, port)| {
            Posed::new(
                &pack,
                training_fighter(&pack, &stage, kind, port, &segments),
            )
        })
        .collect();
    let scene = GameScene::YoshiEgg;
    let capture = script::capture_tick(scene).unwrap();
    let mut timeline: Vec<(u64, String, String)> = Vec::new();
    let mut pair = Pair::default();
    for tick in FIRST_TICK..capture {
        let _ = tick_pair(
            &pack,
            &map,
            &mut fighters,
            &scene_pad(scene),
            tick,
            &mut pair,
        );
        let now = (
            format!("{:?}", fighters[0].fighter.status.status),
            format!("{:?}", fighters[1].fighter.status.status),
        );
        if timeline
            .last()
            .is_none_or(|(_, y, d)| (y, d) != (&now.0, &now.1))
        {
            timeline.push((tick, now.0, now.1));
        }
    }
    let line: Vec<String> = timeline
        .iter()
        .map(|(t, y, d)| format!("{t}:{y}/{d}"))
        .collect();
    let line = line.join(" ");
    if std::env::var_os("SCENE_TRACE").is_some() {
        let (y, d) = (&fighters[0].fighter, &fighters[1].fighter);
        println!(
            "yoshiegg capture {capture}: {line}\n  Yoshi ({:.1}, {:.1}), dummy ({:.1}, {:.1}) damage {}",
            y.pos.x, y.pos.y, d.pos.x, d.pos.y, d.damage
        );
    }
    let entered = |status: &str| {
        timeline
            .iter()
            .find(|(_, _, d)| d.contains(status))
            .map(|(t, _, _)| *t)
    };
    let caught = entered("CaptureYoshi").unwrap_or_else(|| panic!("never caught: {line}"));
    let laid = entered("YoshiEgg").unwrap_or_else(|| panic!("never laid: {line}"));
    // The tongue pressed at 250 catches, not a later one.
    assert!((250..330).contains(&caught), "caught at {caught}: {line}");
    assert!(laid > caught, "{line}");
    // Training's dummy is a level-3 CPU (`sc1PTrainingModeInitVars`): it
    // wiggles from its `(LEVEL_MAX - level) * 15 + 1`th frame in the egg
    // and soon breaks it open (out at 458 in the PPSSPP log of this
    // script). This replay holds its pad neutral, so the capture has to
    // come before the wiggle to stand for the scene.
    const TRAINING_CPU_LEVEL: u64 = 3;
    let wiggle = laid + (u64::from(ssb_game::computer::LEVEL_MAX) - TRAINING_CPU_LEVEL) * 15 + 1;
    assert!(
        capture <= wiggle,
        "capture {capture} after the dummy starts to wiggle out at {wiggle}: {line}"
    );
    let dummy = &fighters[1].fighter;
    assert_eq!(
        dummy.status.status,
        ssb_game::status::AnyStatus::from(Status::YoshiEgg),
        "{line}"
    );
    assert!(
        ssb_game::capture_yoshi::egg_effect(dummy).is_some(),
        "no egg effect: {line}"
    );
}

/// Kirby's spit on the N64's Training Castle (warp ROM
/// `warp_kirbyvssamus`, P1 route `9:8:x=127;49:40:b;129:3:a`, Samus the
/// standing dummy): Kirby dashes to x 498, inhales Samus and spits her
/// right at N64 frame 129. The star leaves Kirby's mouth at x 508 on the
/// Castle's floor and climbs the rise ahead in its first frame, so its
/// first `ftCommonThrownCommonStarProcMap` lands it: N64 frame 135 makes
/// the star, the magnitude-2 quake (2 GObjs) and the impact wave at
/// (624, 1398.15), TopN's translation before the collision moved it to
/// (624, 1400.65) (`~/ppsspp-test/batch9/s.txt`, `g4.txt`). N64 row `t`
/// is replay tick `t`: a route step starting at `s` is recorded at
/// `t = s - 1` and acts on frame `s`.
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
#[test]
fn kirby_spit_star_lands_with_a_quake_and_an_impact_wave_on_its_first_frame() {
    use ssb_game::fteffect::FighterEffect as E;
    let Some(bytes) = open_pack() else {
        return;
    };
    let pack = ssb_rom::pack::Pack::open(&bytes).unwrap();
    const CASTLE: u32 = 4;
    let (stage, segments) = stage_floors(&pack, CASTLE);
    let map = dream_land_map(&pack, &stage);
    let mut fighters: Vec<Posed> = [(FighterKind::Kirby, 0), (FighterKind::Samus, 1)]
        .into_iter()
        .map(|(kind, port)| {
            Posed::new(
                &pack,
                training_fighter(&pack, &stage, kind, port, &segments),
            )
        })
        .collect();
    let pad = |t: u64| match t {
        9..=16 => (0, 127, 0),
        49..=88 => (button::B, 0, 0),
        129..=131 => (button::A, 0, 0),
        _ => (0, 0, 0),
    };
    let mut pair = Pair::default();
    let mut star: Option<(u64, Vec<E>, [f32; 2])> = None;
    let mut log = vec![format!(
        "spawn Kirby ({:.2},{:.2}) Samus ({:.2},{:.2})",
        fighters[0].fighter.pos.x,
        fighters[0].fighter.pos.y,
        fighters[1].fighter.pos.x,
        fighters[1].fighter.pos.y
    )];
    for tick in 1..=150 {
        let [_, queued] = tick_pair(&pack, &map, &mut fighters, &pad, tick, &mut pair);
        let (k, dummy) = (&fighters[0].fighter, &fighters[1].fighter);
        let impacts: Vec<E> = queued
            .into_iter()
            .filter(|e| matches!(e, E::ImpactWave { .. } | E::Quake(_)))
            .collect();
        let status = format!("{:?}", dummy.status.status);
        log.push(format!(
            "{tick}: K {:?} ({:.2},{:.2}) S {status} ({:.2},{:.2}) vel ({:.2},{:.2}) {impacts:?}",
            k.status.status,
            k.pos.x,
            k.pos.y,
            dummy.pos.x,
            dummy.pos.y,
            dummy.physics.vel_air.x,
            dummy.physics.vel_air.y
        ));
        if star.is_none() && status.contains("ThrownKirbyStar") {
            star = Some((tick, impacts, [dummy.pos.x, dummy.pos.y]));
        }
        assert_eq!(dummy.effects.dropped, 0);
    }
    let log = log.join("\n");
    if std::env::var_os("SCENE_TRACE").is_some() {
        println!("kirby spit on the Castle:\n{log}");
    }
    let (tick, impacts, pos) = star.unwrap_or_else(|| panic!("never spat: {log}"));
    assert_eq!(tick, 135, "{log}");
    // The trace prints two decimals.
    let near =
        |a: [f32; 2], b: [f32; 2]| (a[0] - b[0]).abs() < 0.005 && (a[1] - b[1]).abs() < 0.005;
    assert!(near(pos, [624.0, 1400.65]), "{pos:?}\n{log}");
    let [E::Quake(2), E::ImpactWave { pos: wave, .. }] = impacts.as_slice() else {
        panic!("star tick {tick}: {impacts:?}\n{log}");
    };
    assert!(near([wave.x, wave.y], [624.0, 1398.15]), "{wave:?}\n{log}");
}

/// `purinthrow`'s replay from `seed`: Jigglypuff on the left platform's
/// spawn (1), the dummy Mario on the main floor's (0), as `psp-game`'s
/// roster swaps them. Returns the status timeline (`tick:Purin/dummy`),
/// the dummy at the capture tick, and whether the replay drew from the
/// RNG.
fn purin_throw_replay(
    pack: &ssb_rom::pack::Pack<'_>,
    seed: i32,
) -> (String, ssb_game::fighter::Fighter, bool) {
    ssb_game::rng::set_seed(seed);
    let (stage, segments) = dream_land(pack);
    let map = dream_land_map(pack, &stage);
    let mut fighters: Vec<Posed> = [(FighterKind::Purin, 0, 1), (FighterKind::Mario, 1, 0)]
        .into_iter()
        .map(|(kind, port, spawn)| {
            Posed::new(
                pack,
                dream_land::training_fighter_at(pack, &stage, kind, port, spawn, &segments),
            )
        })
        .collect();
    let scene = GameScene::PurinThrow;
    let capture = script::capture_tick(scene).unwrap();
    let mut timeline: Vec<(u64, String, String)> = Vec::new();
    let mut pair = Pair::default();
    let trace = std::env::var_os("SCENE_TRACE").is_some();
    for tick in FIRST_TICK..capture {
        let _ = tick_pair(
            pack,
            &map,
            &mut fighters,
            &scene_pad(scene),
            tick,
            &mut pair,
        );
        let (p, d) = (&fighters[0].fighter, &fighters[1].fighter);
        if trace && tick % 5 == 0 {
            println!(
                "purinthrow {tick}: Purin {:?} ({:.1}, {:.1}) dummy {:?} ({:.1}, {:.1})",
                p.status.status, p.pos.x, p.pos.y, d.status.status, d.pos.x, d.pos.y
            );
        }
        let now = (
            format!("{:?}", p.status.status),
            format!("{:?}", d.status.status),
        );
        if timeline
            .last()
            .is_none_or(|(_, y, d)| (y, d) != (&now.0, &now.1))
        {
            timeline.push((tick, now.0, now.1));
        }
    }
    let line: Vec<String> = timeline
        .iter()
        .map(|(t, y, d)| format!("{t}:{y}/{d}"))
        .collect();
    let [_, dummy] = <[Posed; 2]>::try_from(fighters).ok().unwrap();
    let drew = ssb_game::rng::seed() != seed;
    (line.join(" "), dummy.fighter, drew)
}

/// `purinthrow`: Jigglypuff's forward throw releases the dummy into the
/// `DamageFlyRoll` its throw description names
/// (`dPurinMainMotion_ThrowF`'s `FTThrowHitDesc`), not the random roll of
/// `ftCommonDamageUpdateMain`, and the dummy is still in it, airborne, at
/// the capture tick. A second replay from another RNG seed ends in the
/// very same dummy: nothing the golden shows of it hangs on the shared
/// RNG, and the dummy's Training CPU stands until it is caught.
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
#[test]
fn purin_throw_puts_the_dummy_in_damage_fly_roll() {
    let Some(bytes) = open_pack() else {
        return;
    };
    let pack = ssb_rom::pack::Pack::open(&bytes).unwrap();
    let (line, dummy, drew) = purin_throw_replay(&pack, 1);
    if std::env::var_os("SCENE_TRACE").is_some() {
        println!("purinthrow: {line}");
    }
    assert!(line.contains("(ThrowF)/"), "never throws forward: {line}");
    assert_eq!(
        dummy.status.status,
        ssb_game::status::AnyStatus::from(Status::DamageFlyRoll),
        "{line}"
    );
    assert!(!dummy.is_grounded(), "{line}");
    // The fighters draw from the RNG on the way, so the reseeded run
    // below checks that the throw's outcome does not depend on them.
    assert!(drew, "{line}");
    let (other, reseeded, _) = purin_throw_replay(&pack, 0x1234_5678);
    assert_eq!(line, other);
    assert_eq!(
        (
            dummy.pos.x,
            dummy.pos.y,
            dummy.damage,
            dummy.status.anim_frame
        ),
        (
            reseeded.pos.x,
            reseeded.pos.y,
            reseeded.damage,
            reseeded.status.anim_frame
        )
    );
}

/// Pack stage index of Peach's Castle (`gkind::CASTLE`).
const CASTLE: u32 = 4;

/// `vsmagnify`'s fighters at its capture tick, replayed from `seed` on
/// Peach's Castle from Training's start (tick 8): the player's Kirby
/// (port 0, spawn 0) on [`script::vs_magnify`]'s pad, the CPU Mario
/// (port 1, spawn 1) standing with a neutral pad, as `Behavior::Stand`'s
/// `STICK_N` holds it on the stage; and the battle camera framing both
/// (`gmCameraUpdateInterests`, Kirby's camera). Returns Kirby's timeline,
/// Kirby and the camera at the capture tick, and whether the replay drew
/// from the RNG.
fn vs_magnify_replay(
    pack: &ssb_rom::pack::Pack<'_>,
    seed: i32,
) -> (
    String,
    ssb_game::fighter::Fighter,
    ssb_game::camera::Camera,
    bool,
) {
    use ssb_game::camera::{BonusFollow, Bounds, CameraRig, CameraStage, Framing, StageCamera};
    ssb_game::rng::set_seed(seed);
    let (stage, segments) = stage_floors(pack, CASTLE);
    let map = dream_land_map(pack, &stage);
    let kinds = [FighterKind::Kirby, FighterKind::Mario];
    let mut fighters: Vec<Posed> = kinds
        .into_iter()
        .zip(0u8..)
        .map(|(kind, port)| Posed::new(pack, training_fighter(pack, &stage, kind, port, &segments)))
        .collect();
    let framing = kinds.map(|kind| {
        let d = pack.fighter(kind as u32).unwrap();
        Framing {
            cam_offset_y: d.cam_offset_y,
            zoom_frame: d.camera_zoom,
        }
    });
    let bounds = |e: &ssb_rom::pack::Extent| Bounds {
        top: f32::from(e.top),
        bottom: f32::from(e.bottom),
        left: f32::from(e.left),
        right: f32::from(e.right),
    };
    let camera_stage = CameraStage {
        camera: bounds(&stage.camera),
        team_camera: bounds(&stage.team_camera),
        map_top: f32::from(stage.bounds.top),
        light_angle_z: stage.camera_light_angle_z,
        stage_camera: StageCamera::Default,
        follow: BonusFollow::Other,
    };
    let mut rig = CameraRig::default();
    let pad = |tick: u64| {
        (
            script::vs_magnify::buttons(tick),
            script::vs_magnify::stick_x(tick),
            0,
        )
    };
    let mut pair = Pair::default();
    let mut timeline: Vec<(u64, String)> = Vec::new();
    let trace = std::env::var_os("SCENE_TRACE").is_some();
    for tick in FIRST_TICK..script::vs_magnify::CAPTURE_TICK {
        let _ = tick_pair(pack, &map, &mut fighters, &pad, tick, &mut pair);
        let interests: Vec<_> = fighters
            .iter()
            .zip(framing)
            .filter_map(|(p, fr)| ssb_game::camera::interest_of(&p.fighter, fr, &camera_stage))
            .collect();
        let k = &fighters[0].fighter;
        rig.tick(
            k.pos,
            framing[0].cam_offset_y,
            &interests,
            None,
            &camera_stage,
        );
        let view = ssb_game::player_interface::view(k, &rig.camera, framing[0].cam_offset_y);
        if trace && tick % 10 == 0 {
            println!(
                "vsmagnify {tick}: Kirby {:?} ({:.1}, {:.1}) offscreen {} eligible {} dir {:?} raw scale {:.3}; Mario ({:.1}, {:.1})",
                k.status.status,
                k.pos.x,
                k.pos.y,
                view.offscreen,
                view.eligible,
                view.direction,
                raw_magnify_scale(&rig.camera),
                fighters[1].fighter.pos.x,
                fighters[1].fighter.pos.y
            );
        }
        let now = format!("{:?}", k.status.status);
        if timeline.last().is_none_or(|(_, s)| *s != now) {
            timeline.push((tick, now));
        }
    }
    let line: Vec<String> = timeline.iter().map(|(t, s)| format!("{t}:{s}")).collect();
    let drew = ssb_game::rng::seed() != seed;
    let [kirby, _] = <[Posed; 2]>::try_from(fighters).ok().unwrap();
    (line.join(" "), kirby.fighter, rig.camera, drew)
}

/// [`ssb_game::player_interface::magnify_scale`] before its cap of 3.
fn raw_magnify_scale(camera: &ssb_game::camera::Camera) -> f32 {
    use ssb_engine::math::Vec3;
    let dist = (camera.eye - camera.at).length();
    let mut mini = *camera;
    mini.eye = Vec3::new(0.0, 300.0, dist);
    mini.at = Vec3::new(0.0, 300.0, 0.0);
    mini.project(Vec3::new(0.0, 900.0, 0.0)).1 / 18.0
}

/// `vsmagnify`: at the capture tick Kirby, the player, is out of the
/// battle camera's view and eligible for his magnifier
/// (`ifCommonPlayerMagnifyUpdateRender`'s test, [`player_interface::view`]),
/// falling left of it, magnifier on the screen's left edge, short of the
/// left blast zone. The camera frames the standing
/// CPU too, so the magnifier's scale is the projected one, under its cap
/// of 3, as at the `vs4` frame this scene replaces (RE-508's raw 2.397).
/// Kirby is on the pad alone: the CPU stands, his run, jumps and fall
/// never read the RNG, so no other fighter's or the stage's draws reach him,
/// and a second replay from another RNG seed ends in the same Kirby and
/// camera.
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
#[test]
fn vs_magnify_shows_kirby_below_the_camera() {
    let Some(bytes) = open_pack() else {
        return;
    };
    let pack = ssb_rom::pack::Pack::open(&bytes).unwrap();
    let (line, kirby, camera, drew) = vs_magnify_replay(&pack, 1);
    if std::env::var_os("SCENE_TRACE").is_some() {
        println!("vsmagnify: {line}");
    }
    let cam_offset_y = pack
        .fighter(FighterKind::Kirby as u32)
        .unwrap()
        .cam_offset_y;
    let view = ssb_game::player_interface::view(&kirby, &camera, cam_offset_y);
    assert!(view.offscreen && view.eligible, "{view:?}: {line}");
    let stage = pack.stage(CASTLE).unwrap();
    assert!(view.direction.0 < 0.0, "{view:?}: {line}");
    assert!(kirby.pos.x > f32::from(stage.bounds.left), "{line}");
    assert!(raw_magnify_scale(&camera) < 3.0, "{line}");
    assert!(!drew, "{line}");
    let (other, reseeded, camera2, _) = vs_magnify_replay(&pack, 0x1234_5678);
    assert_eq!(line, other);
    assert_eq!(
        (
            kirby.pos.x,
            kirby.pos.y,
            kirby.status.anim_frame,
            camera.eye.x,
            camera.eye.y
        ),
        (
            reseeded.pos.x,
            reseeded.pos.y,
            reseeded.status.anim_frame,
            camera2.eye.x,
            camera2.eye.y
        )
    );
}
