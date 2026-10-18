//! Board the Platforms' fighter against its moving platforms, replayed on
//! the host in the runtime's tick order and compared with N64 traces of the
//! same courses (RE-524).

#[path = "common/bonus2_world.rs"]
mod bonus2_world;
#[path = "common/dream_land.rs"]
mod dream_land;

use bonus2_world::{pad, Course};
use ssb_game::fighter::FighterKind;
use ssb_rom::pack::Pack;

fn open_pack() -> Option<Vec<u8>> {
    ssb_tablegen::test_rom(env!("CARGO_MANIFEST_DIR"))?;
    std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/generated/ssb64.pak"),
    )
    .ok()
}

const KINDS: [FighterKind; 12] = [
    FighterKind::Mario,
    FighterKind::Fox,
    FighterKind::Donkey,
    FighterKind::Samus,
    FighterKind::Luigi,
    FighterKind::Link,
    FighterKind::Yoshi,
    FighterKind::Captain,
    FighterKind::Kirby,
    FighterKind::Pikachu,
    FighterKind::Purin,
    FighterKind::Ness,
];

/// A pad script: `start end buttons stick_x stick_y` per line, frames
/// inclusive, buttons as `N64Buttons` hex.
fn script(text: &str) -> Vec<(u32, u32, u16, i8, i8)> {
    text.lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
        .map(|l| {
            let w: Vec<_> = l.split_whitespace().collect();
            (
                w[0].parse().unwrap(),
                w[1].parse().unwrap(),
                u16::from_str_radix(w[2], 16).unwrap(),
                w[3].parse().unwrap(),
                w[4].parse().unwrap(),
            )
        })
        .collect()
}

fn pad_at(script: &[(u32, u32, u16, i8, i8)], frame: u32) -> ssb_engine::input::ControllerState {
    script
        .iter()
        .find(|s| (s.0..=s.1).contains(&frame))
        .map_or(pad(0, 0, 0), |s| pad(s.2, s.3, s.4))
}

/// `B2_TRACE_KIND=n B2_TRACE_FRAMES=n [B2_TRACE_SCRIPT=file]
/// B2_TRACE_OUT=file`: the course's per-frame trace, for a diff with the
/// N64's.
#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn trace_course_on_request() {
    let (Some(kind), Some(out)) = (
        std::env::var_os("B2_TRACE_KIND"),
        std::env::var_os("B2_TRACE_OUT"),
    ) else {
        return;
    };
    let Some(bytes) = open_pack() else { return };
    let p = Pack::open(&bytes).unwrap();
    let kind = KINDS[kind.to_str().unwrap().parse::<usize>().unwrap()];
    let frames: u32 = std::env::var("B2_TRACE_FRAMES").map_or(600, |v| v.parse().unwrap());
    let script = std::env::var("B2_TRACE_SCRIPT")
        .map(|f| script(&std::fs::read_to_string(f).unwrap()))
        .unwrap_or_default();
    let mut c = Course::new(&p, kind);
    let mut text = String::new();
    for frame in 0..frames {
        c.tick(pad_at(&script, frame));
        text += &c.trace(frame);
    }
    std::fs::write(out, text).unwrap();
}

/// Fox's course idle, against the N64's trace of the same frames (warp
/// boot into `nGRKindBonus2Fox`, no input): the fighter falls from the
/// spawn (`is_skip_entry`: no entry status), lands on the course's floor,
/// and the platform under it carries it up. A platform a frame late or
/// started from its file pose instead of `gcPlayAnimAll`'s puts every
/// carried frame off by one platform step.
#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn foxs_idle_ride_matches_the_n64() {
    let Some(bytes) = open_pack() else { return };
    let p = Pack::open(&bytes).unwrap();
    let mut c = Course::new(&p, FighterKind::Fox);
    // (frame, status, airborne, y) from the N64 trace.
    let expect: [(u32, u16, bool, f32); 5] = [
        (0, 26, true, -3150.0),
        (19, 31, false, -3600.0),
        (180, 10, false, -3575.2085),
        (330, 10, false, 143.5164),
        (392, 10, false, 1680.5898),
    ];
    let mut next = expect.iter().peekable();
    for frame in 0..=392 {
        c.tick(pad(0, 0, 0));
        if let Some(&&(f, status, air, y)) = next.peek().filter(|e| e.0 == frame) {
            let fighter = &c.fighter;
            assert_eq!(fighter.status.status.id(), status, "frame {f}");
            assert_eq!(!fighter.is_grounded(), air, "frame {f}");
            assert_eq!(fighter.pos.x, -6750.0, "frame {f}");
            assert!(
                (fighter.pos.y - y).abs() < 0.01,
                "frame {f}: y {} vs {y}",
                fighter.pos.y
            );
            next.next();
        }
    }
    assert!(next.next().is_none());
}

/// Fox's course under a pad script (walk, full and short hops, aerial
/// jumps, drop-throughs, turns), against the N64's trace of the same inputs
/// from Go: the whole-unit jump force, `mpCommonSetFighterAir`'s spent
/// ground jump, `ftMainSetStatus`'s `ignore_line_id` reset and
/// `mpCommonSetFighterWaitOrLanding` each move a later checkpoint.
#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn foxs_scripted_course_matches_the_n64() {
    const SCRIPT: &str = "70 85 0000 30 -80
86 87 0008 -30 0
88 97 0000 -30 0
98 99 0008 80 0
100 111 0000 80 0
112 113 0008 80 0
114 129 0000 80 0
130 164 0000 30 -80
165 200 0000 -30 0
201 202 0008 -30 0
203 214 0000 -30 0
215 216 0008 0 0
217 222 0000 0 0
223 231 0000 30 0
232 233 0008 30 0
234 270 0000 30 0
271 272 0008 -30 0
273 307 0000 -30 0
308 321 0000 -50 0
322 323 0008 -80 0
324 347 0000 -80 0
348 387 0000 0 0
388 407 0000 -30 0
408 409 0008 80 0
410 446 0000 80 0
447 484 0000 -50 0
485 486 0008 50 0
487 518 0000 50 0
519 549 0000 -80 0";
    let Some(bytes) = open_pack() else { return };
    let p = Pack::open(&bytes).unwrap();
    let script = script(SCRIPT);
    let mut c = Course::new(&p, FighterKind::Fox);
    // (frame, status, airborne, x, y) from the N64 trace.
    let expect: [(u32, u16, bool, f32, f32); 11] = [
        (60, 10, false, -6750.0, -3600.0),
        (95, 25, true, -6878.4, -3050.9995),
        (120, 25, true, -6759.6, -2518.4983),
        (165, 30, false, -6128.8965, -3600.0),
        (180, 18, false, -6128.8965, -3575.2085),
        (240, 24, true, -6452.3965, -990.2028),
        (300, 18, false, -6577.1973, -600.2283),
        (360, 22, true, -8205.301, -454.2324),
        (420, 25, true, -9079.199, -2165.3318),
        (480, 27, true, -7940.6943, -3986.3303),
        (540, 27, true, -7882.7944, -7586.33),
    ];
    let mut next = expect.iter().peekable();
    for frame in 0..=540 {
        c.tick(pad_at(&script, frame));
        if let Some(&&(f, status, air, x, y)) = next.peek().filter(|e| e.0 == frame) {
            let fighter = &c.fighter;
            assert_eq!(fighter.status.status.id(), status, "frame {f}");
            assert_eq!(!fighter.is_grounded(), air, "frame {f}");
            assert!(
                (fighter.pos.x - x).abs() < 0.01 && (fighter.pos.y - y).abs() < 0.01,
                "frame {f}: ({}, {}) vs ({x}, {y})",
                fighter.pos.x,
                fighter.pos.y
            );
            next.next();
        }
    }
    assert!(next.next().is_none());
}

/// A platform's first tick measures its speed from the pose
/// `grDisplayMakeGeometryLayer`'s `gcPlayAnimAll` gave it at creation, so
/// a moving platform starts the battle already moving, as on the N64.
#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn platforms_start_from_their_creation_pose() {
    let Some(bytes) = open_pack() else { return };
    let p = Pack::open(&bytes).unwrap();
    for kind in KINDS {
        let mut c = Course::new(&p, kind);
        let before: Vec<_> = c.groups.iter().map(|g| g.translate).collect();
        c.tick_map();
        let moved = c
            .groups
            .iter()
            .zip(&before)
            .filter(|(g, b)| g.animated && g.translate != **b)
            .count();
        let animated = c.groups.iter().filter(|g| g.animated).count();
        assert!(animated > 0, "{kind:?}");
        // Every animated group either moves on its first tick or holds a
        // key; none jumps from its file pose to the creation pose.
        for (g, b) in c.groups.iter().zip(&before).filter(|(g, _)| g.animated) {
            let step = (g.translate.x - b.x).hypot(g.translate.y - b.y);
            assert!(step < 200.0, "{kind:?}: first step {step}");
        }
        assert!(moved > 0, "{kind:?}");
    }
}

/// `TraI`: an orbiting Bumper's path writes its whole translation, so
/// `item_play` reports both axes (Fox's course, Bumper 4, against the
/// N64's first frame).
#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn a_path_bumper_moves_on_both_axes() {
    let Some(bytes) = open_pack() else { return };
    let p = Pack::open(&bytes).unwrap();
    let kind = FighterKind::Fox as u32;
    let mut g = ssb_rom::ground_obj::GroundObjects::new(&p, 283 + kind);
    let asset = ssb_rom::bonus2::FIRST_BUMPER_ASSET + kind as u8;
    g.item_make(&p, asset, 4);
    let [x, y, _] = g.item_play(&p, asset, 4);
    let (x, y) = (x.unwrap(), y.unwrap());
    assert!(
        (x - -6478.113).abs() < 0.01 && (y - 1967.6931).abs() < 0.01,
        "({x}, {y})"
    );
    let [x2, y2, _] = g.item_play(&p, asset, 4);
    assert!(x2.unwrap() != x && y2.unwrap() != y);
}
