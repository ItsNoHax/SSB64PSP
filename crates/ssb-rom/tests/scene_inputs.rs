//! The golden scenes' input scripts replayed through the portable fighter,
//! asserting that each fighter reaches the move its scene is named for.
//!
//! An A at tick 8 starts a jab; a later B inside that jab is swallowed, the
//! fighter never starts the special, and the golden shows a fighter standing
//! still next to the move's leftovers (RE-516 found `samus`, `samusshot` and
//! `donkey` this way). The scripts live in `ssb_capture::script`, shared
//! with `psp-game`, so this test reads the very schedule the capture runs.
//!
//! The replay is the fighter alone on Dream Land's floors: no dummy, items,
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
use ssb_game::fighter::{Fighter, FighterKind, JostleBody};
use ssb_game::status::Status;
use ssb_game::weapon::{MapSurface, MapSurfaceKind, SurfaceTopology};

#[path = "common/posed.rs"]
mod posed;
use posed::{exchange, Posed};

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
    ]
}

/// Dream Land and its floor segments, `(line id, segment)`.
fn dream_land<'p>(
    pack: &ssb_rom::pack::Pack<'p>,
) -> (
    ssb_rom::pack::StageDesc,
    Vec<(u16, ssb_game::collision::Segment)>,
) {
    let stage = pack.stage(0).expect("Dream Land");
    let mut segments = Vec::new();
    for line in pack.stage_lines(&stage) {
        if line.kind != ssb_rom::pack::line_kind::FLOOR {
            continue;
        }
        let points: Vec<_> = pack.line_vertices(&line).collect();
        for pair in points.windows(2) {
            segments.push((
                line.id,
                ssb_game::collision::Segment {
                    x1: pair[0].x,
                    y1: pair[0].y,
                    x2: pair[1].x,
                    y2: pair[1].y,
                    flags: pair[0].flags,
                },
            ));
        }
    }
    (stage, segments)
}

/// Training's fighter on `port` at the stage's spawn `port`
/// (`FighterScene::at_position`), standing (or falling) from its first
/// battle frame.
fn training_fighter(
    pack: &ssb_rom::pack::Pack<'_>,
    stage: &ssb_rom::pack::StageDesc,
    kind: FighterKind,
    port: u8,
    floors: &[(u16, ssb_game::collision::Segment)],
) -> Fighter {
    let spawn = pack.spawn(stage, u16::from(port)).expect("spawn point");
    let mut f = Fighter::new(kind, port, 3);
    let d = pack.fighter(kind as u32).expect("fighter in pack");
    f.attributes = ssb_game::physics::PhysicsAttributes {
        size: d.size,
        traction: d.traction,
        dash_speed: d.dash_speed,
        dash_decel: d.dash_decel,
        run_speed: d.run_speed,
        walk_speed_mul: d.walk_speed_mul,
        jump_vel_x: d.jump_vel_x,
        jump_height_mul: d.jump_height_mul,
        jump_height_base: d.jump_height_base,
        jumpaerial_vel_x: d.jumpaerial_vel_x,
        jumpaerial_height: d.jumpaerial_height,
        air_accel: d.air_accel,
        air_speed_max_x: d.air_speed_max_x,
        air_friction: d.air_friction,
        gravity: d.gravity,
        tvel_base: d.tvel_base,
        tvel_fast: d.tvel_fast,
        jumps_max: d.jumps_max,
        weight: d.weight,
        kneebend_anim_length: d.kneebend_anim_length,
        dash_to_run: d.dash_to_run,
        walkslow_anim_length: d.walkslow_anim_length,
        walkmiddle_anim_length: d.walkmiddle_anim_length,
        walkfast_anim_length: d.walkfast_anim_length,
    };
    f.coll = ssb_game::ground::BodyColl {
        top: d.coll_top,
        center: d.coll_center,
        bottom: d.coll_bottom,
        width: d.coll_width,
    };
    f.cliff_reach = ssb_engine::math::Vec2::new(d.cliffcatch_width, d.cliffcatch_height);
    f.cliff_air_mask = d.cliff_air_mask;
    f.anim = ssb_game::status::AnimLengths {
        dash: d.dash_anim_length,
        turn: d.turn_anim_length,
        run_brake: d.runbrake_anim_length,
        squat: d.squat_anim_length,
        squat_rv: d.squatrv_anim_length,
        landing: d.landing_anim_length,
        pass: d.pass_anim_length,
    };
    f.jostle_width = d.jostle_width;
    f.jostle_x = d.jostle_x;
    f.pos = ssb_engine::math::Vec3::new(f32::from(spawn.x), f32::from(spawn.y), 0.0);
    f.facing = ssb_game::fighter::Facing::at_spawn_x(f.pos.x);
    f.topn_lr = f.facing.sign();
    f.init_floor(floors.iter().copied());
    ssb_game::status::set_wait_or_fall(&mut f);
    f
}

/// Dream Land's collision lines as the runtime's `FloorSegments` gives
/// them to the map: every kind, with its topology.
fn dream_land_map(
    pack: &ssb_rom::pack::Pack<'_>,
    stage: &ssb_rom::pack::StageDesc,
) -> Vec<MapSurface> {
    use ssb_rom::pack::line_kind;
    let mut out = Vec::new();
    for line in pack.stage_lines(stage) {
        let kind = match line.kind {
            line_kind::FLOOR => MapSurfaceKind::Floor,
            line_kind::CEILING => MapSurfaceKind::Ceiling,
            line_kind::RIGHT_WALL => MapSurfaceKind::RightWall,
            line_kind::LEFT_WALL => MapSurfaceKind::LeftWall,
            _ => continue,
        };
        let points: Vec<_> = pack.line_vertices(&line).collect();
        for (n, pair) in points.windows(2).enumerate() {
            out.push(MapSurface {
                kind,
                segment: ssb_game::collision::Segment {
                    x1: pair[0].x,
                    y1: pair[0].y,
                    x2: pair[1].x,
                    y2: pair[1].y,
                    flags: pair[0].flags,
                },
                topology: Some(SurfaceTopology {
                    line: line.id,
                    point: n as u16,
                    segments: line.vertex_count - 1,
                    vertex1: pair[0].vertex_id,
                    vertex2: pair[1].vertex_id,
                }),
                motion: None,
            });
        }
    }
    out
}

/// One scene's replay: `(tick, status)` at every status change, and the
/// status at the capture tick.
fn replay(
    pack: &ssb_rom::pack::Pack<'_>,
    scene: GameScene,
    kind: FighterKind,
) -> (Vec<(u64, String)>, String, f32) {
    let (stage, segments) = dream_land(pack);
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
    std::env::var_os("SSB64_ROM")?;
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
}

#[test]
fn every_fighter_scene_has_an_intent() {
    let covered: Vec<GameScene> = intents().iter().map(|i| i.scene).collect();
    for scene in GameScene::ALL {
        if script::is_fighter_scene(scene) {
            // `kirbyhat` and `yoshiegg` need the dummy: out of this replay
            // (`yoshiegg` has its own).
            let outside = matches!(scene, GameScene::KirbyHat | GameScene::YoshiEgg);
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
    let map_fn = || map.iter().copied();
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
    let mut jump_was_held = false;
    for tick in FIRST_TICK..capture {
        // Priority 5: input, TransN's sample, the interrupt, jostle and the
        // grab exchange.
        for i in 0..2 {
            let (c, tapped, released) = if i == 0 {
                let pad = script::fighter_pad(scene, tick).unwrap();
                let held = pad.buttons
                    & (N64Buttons::C_UP
                        | N64Buttons::C_DOWN
                        | N64Buttons::C_LEFT
                        | N64Buttons::C_RIGHT)
                    != 0;
                let edges = (held && !jump_was_held, !held && jump_was_held);
                jump_was_held = held;
                (
                    ControllerState {
                        buttons: N64Buttons(pad.buttons),
                        stick_x: pad.stick_x,
                        stick_y: pad.stick_y,
                        connected: true,
                    },
                    edges.0,
                    edges.1,
                )
            } else {
                (
                    ControllerState {
                        connected: true,
                        ..ControllerState::default()
                    },
                    false,
                    false,
                )
            };
            let p = &mut fighters[i];
            p.fighter.set_input(c, tapped, released);
            p.sample_held_child_offset();
            p.set_root_motion(None);
            p.fighter.tick_interrupt(&map_fn);
            p.after_interrupt(&pack);
            if !fighters[i].fighter.is_in_hitlag() {
                let other = JostleBody::of(&fighters[1 - i].fighter);
                ssb_game::fighter::jostle(&mut fighters[i].fighter, &[(other, i == 0)]);
            }
            exchange(&mut fighters, i);
        }
        // Priority 4: the clip, the physics and map, the joint samples.
        for i in 0..2 {
            fighters[i].physics(&pack, &map_fn);
            let _ = fighters[i].fighter.take_weapon_spawn();
            exchange(&mut fighters, i);
        }
        // Priorities 1 and 0: the catch search, the hit search, then
        // `ftMainProcParams`.
        for i in 0..2 {
            let [a, b] = &mut fighters[..] else {
                unreachable!()
            };
            let (catcher, other) = if i == 0 { (a, b) } else { (b, a) };
            if ssb_game::grab::nearest_catch(
                &catcher.fighter,
                core::iter::once(&other.fighter),
                ssb_game::team::TeamRules::FREE_FOR_ALL,
            )
            .is_some()
            {
                ssb_game::grab::search_catch(
                    &mut catcher.fighter,
                    &other.fighter,
                    ssb_game::team::TeamRules::FREE_FOR_ALL,
                );
            }
        }
        for i in 0..2 {
            exchange(&mut fighters, i);
        }
        for p in &mut fighters {
            p.settle_capture(&pack, &map_fn);
        }
        {
            let [a, b] = &mut fighters[..] else {
                unreachable!()
            };
            let mut both = [&mut a.fighter, &mut b.fighter];
            ssb_game::combat::search_all(&mut both, ssb_game::team::TeamRules::FREE_FOR_ALL);
            ssb_game::combat::finish_frame(&mut both);
        }
        for p in &mut fighters {
            p.fighter.resolve_cliff_release(&map_fn);
        }
        for i in 0..2 {
            exchange(&mut fighters, i);
        }
        for p in &mut fighters {
            p.settle_status(&pack);
        }
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
