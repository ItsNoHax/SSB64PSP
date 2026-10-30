//! Golden-capture scene selection, the `psp-game` counterpart of
//! `psp-asset-viewer/src/capture.rs`.
//!
//! A `golden_capture` (or `capture_scene_file`) build reads its scene from
//! `capture_scene.txt` beside the EBOOT (`ssb_capture` owns the spec format).
//! Without the file, or in a build without either feature, the old
//! per-scene Cargo features pick a compile-time default. A build with neither
//! reads the real pad and has no capture scene. The scene only chooses the
//! script: input stays tick-driven.

pub use ssb_capture::GameScene;

/// The scene this run captures, and where it came from.
#[derive(Clone, Copy)]
pub struct Capture {
    pub scene: GameScene,
    /// `true` when `capture_scene.txt` chose the scene. Only then does a
    /// `golden_capture` build exit after its screenshot.
    #[cfg_attr(not(feature = "golden_capture"), allow(dead_code))]
    pub from_file: bool,
}

/// A `scene@tick` line's tick, or 0 for the scene's own.
static TICK_OVERRIDE: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);

/// The capture tick a `scene@tick` line asked for.
pub fn tick_override() -> Option<u64> {
    match TICK_OVERRIDE.load(core::sync::atomic::Ordering::Relaxed) {
        0 => None,
        t => Some(u64::from(t)),
    }
}

/// A `stage=GKIND` line's stage, plus one, or 0 for the scene's own: a
/// `capture_scene_file` build (the `profile` and `frame_timer` hardware
/// runs) runs a battle scene on any stage (RE-471, RE-500).
static STAGE_OVERRIDE: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);

/// The stage a `stage=GKIND` line asked for.
pub fn stage_override() -> Option<u8> {
    match STAGE_OVERRIDE.load(core::sync::atomic::Ordering::Relaxed) {
        0 => None,
        g => Some((g - 1) as u8),
    }
}

/// Diagnostic battle overrides for `capture_scene_file` hardware sweeps
/// (`profile`, `frame_timer`; RE-531's `stack_poison` runs), each stored
/// plus one, 0 for the scene's own: an `items=N` line's VS item appearance (0 none .. 5 very
/// high), a `level=N` line's CPU level (1..=9), and a `fighters=a,b,c,d`
/// line's fighter ordinals for the VS roster's four ports (the first also
/// the 1P campaign's fighter).
static ITEMS_OVERRIDE: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);
/// A `onepstage=N` line's 1P stage (`nSC1PGameStage*`, plus one): the
/// stage the `onepmetal`, `onepgiant` and `onepzako` fixtures start on
/// instead of their own, so a hardware run reaches any 1P stage.
#[cfg_attr(not(any(feature = "headless_capture", feature = "capture_scene_file")), allow(dead_code))]
static ONEP_STAGE_OVERRIDE: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);
static LEVEL_OVERRIDE: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);
static FIGHTERS_OVERRIDE: [core::sync::atomic::AtomicU32; 4] =
    [const { core::sync::atomic::AtomicU32::new(0) }; 4];

fn load_plus_one(a: &core::sync::atomic::AtomicU32) -> Option<u8> {
    match a.load(core::sync::atomic::Ordering::Relaxed) {
        0 => None,
        v => Some((v - 1) as u8),
    }
}

/// The VS item appearance an `items=N` line asked for.
pub fn items_override() -> Option<ssb_game::item::normal::Appearance> {
    use ssb_game::item::normal::Appearance;
    Some(match load_plus_one(&ITEMS_OVERRIDE)? {
        0 => Appearance::None,
        1 => Appearance::VeryLow,
        2 => Appearance::Low,
        3 => Appearance::Middle,
        4 => Appearance::High,
        _ => Appearance::VeryHigh,
    })
}

/// The 1P stage a `onepstage=N` line asked for.
#[cfg_attr(not(any(feature = "headless_capture", feature = "capture_scene_file")), allow(dead_code))]
pub fn onep_stage_override() -> Option<u8> {
    load_plus_one(&ONEP_STAGE_OVERRIDE)
}

/// The CPU level a `level=N` line asked for.
pub fn level_override() -> Option<u8> {
    load_plus_one(&LEVEL_OVERRIDE)
}

/// The fighter a `fighters=` line put on `port`.
pub fn fighter_override(port: usize) -> Option<ssb_game::fighter::FighterKind> {
    ssb_game::fighter::FighterKind::from_ordinal(load_plus_one(FIGHTERS_OVERRIDE.get(port)?)?)
}

/// A `hold` line: a `profile` build keeps drawing its frozen tick rather
/// than exiting, for the sampling profiler (RE-471).
#[cfg_attr(not(feature = "headless_capture"), allow(dead_code))]
static HOLD: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);

/// Whether a `hold` line asked to keep drawing the frozen tick.
#[cfg_attr(not(feature = "headless_capture"), allow(dead_code))]
pub fn hold() -> bool {
    HOLD.load(core::sync::atomic::Ordering::Relaxed)
}

/// Set for `vsground`: the ground-effect actor's first wait becomes 60
/// frames after its RNG draw, as the N64 warp ROM's patched
/// efground.c:1590 (`syUtilsRandIntRange(10000) * 0 + 60`) does.
static GROUND_WAIT_CUT: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);

/// `vsground`'s first ground-effect wait in frames, if this run cuts it.
pub fn ground_wait_cut() -> Option<i32> {
    GROUND_WAIT_CUT
        .load(core::sync::atomic::Ordering::Relaxed)
        .then_some(60)
}

/// Cuts the first ground-effect wait for the rest of the run.
pub fn cut_ground_wait() {
    GROUND_WAIT_CUT.store(true, core::sync::atomic::Ordering::Relaxed);
}

/// Picks this run's capture scene once at startup.
pub fn select() -> Option<Capture> {
    #[cfg(feature = "capture_scene_file")]
    {
        let mut buf = [0u8; ssb_capture::MAX_SPEC_LEN];
        if let Some(len) = ssb_psp_runtime::assets::read_capture_scene(&mut buf) {
            // `scene@tick` captures the scene at another tick, for matching
            // an N64 frame (RE-425); the goldens name none.
            let text = core::str::from_utf8(&buf[..len]).ok();
            apply_overrides(text);
            let line = text.and_then(ssb_capture::spec_line);
            if let Some(spec) = line.and_then(ssb_capture::GameSpec::parse) {
                if let Some(tick) = spec.tick {
                    TICK_OVERRIDE.store(tick, core::sync::atomic::Ordering::Relaxed);
                }
                return Some(Capture {
                    scene: spec.scene,
                    from_file: true,
                });
            }
        }
    }
    default_scene().map(|scene| Capture {
        scene,
        from_file: false,
    })
}

/// Stores a capture file's diagnostic lines (`stage=`, `items=`,
/// `onepstage=`, `level=`, `fighters=`, `hold`). In every
/// `capture_scene_file` build, out of line and cold: a measurement build
/// (`frame_timer`) then compiles `run` and the frame code exactly as the
/// trace build does (RE-578).
#[cfg(feature = "capture_scene_file")]
#[cold]
#[inline(never)]
fn apply_overrides(text: Option<&str>) {
    if let Some(g) = text
        .into_iter()
        .flat_map(str::lines)
        .find_map(|l| l.trim().strip_prefix("stage="))
        .and_then(|g| g.parse::<u8>().ok())
    {
        STAGE_OVERRIDE.store(u32::from(g) + 1, core::sync::atomic::Ordering::Relaxed);
    }
    for l in text.into_iter().flat_map(str::lines).map(str::trim) {
        let store = |a: &core::sync::atomic::AtomicU32, v: &str| {
            if let Ok(v) = v.trim().parse::<u8>() {
                a.store(u32::from(v) + 1, core::sync::atomic::Ordering::Relaxed);
            }
        };
        if let Some(v) = l.strip_prefix("items=") {
            store(&ITEMS_OVERRIDE, v);
        } else if let Some(v) = l.strip_prefix("onepstage=") {
            store(&ONEP_STAGE_OVERRIDE, v);
        } else if let Some(v) = l.strip_prefix("level=") {
            store(&LEVEL_OVERRIDE, v);
        } else if let Some(v) = l.strip_prefix("fighters=") {
            for (a, v) in FIGHTERS_OVERRIDE.iter().zip(v.split(',')) {
                store(a, v);
            }
        }
    }
    if text.into_iter().flat_map(str::lines).any(|l| l.trim() == "hold") {
        HOLD.store(true, core::sync::atomic::Ordering::Relaxed);
    }
}

/// The scene named by the old per-scene Cargo features, in the order the old
/// capture-tick `cfg!` chain checked them.
fn default_scene() -> Option<GameScene> {
    let scene = if cfg!(feature = "regression_capture_shadows") {
        GameScene::Shadows
    } else if cfg!(feature = "regression_capture_fireball") {
        GameScene::Fireball
    } else if cfg!(feature = "regression_capture_superjump") {
        GameScene::Superjump
    } else if cfg!(feature = "regression_capture_fox") {
        GameScene::Fox
    } else if cfg!(feature = "regression_capture") {
        GameScene::Training
    } else {
        return None;
    };
    Some(scene)
}
