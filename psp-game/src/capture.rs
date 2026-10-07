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
/// `profile` build runs a battle scene on any stage (RE-471).
static STAGE_OVERRIDE: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);

/// The stage a `stage=GKIND` line asked for.
pub fn stage_override() -> Option<u8> {
    match STAGE_OVERRIDE.load(core::sync::atomic::Ordering::Relaxed) {
        0 => None,
        g => Some((g - 1) as u8),
    }
}

/// A `hold` line: a `profile` build keeps drawing its frozen tick rather
/// than exiting, for the sampling profiler (RE-471).
static HOLD: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);

/// Whether a `hold` line asked to keep drawing the frozen tick.
pub fn hold() -> bool {
    HOLD.load(core::sync::atomic::Ordering::Relaxed)
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
            #[cfg(feature = "profile")]
            if let Some(g) = text
                .into_iter()
                .flat_map(str::lines)
                .find_map(|l| l.trim().strip_prefix("stage="))
                .and_then(|g| g.parse::<u8>().ok())
            {
                STAGE_OVERRIDE.store(u32::from(g) + 1, core::sync::atomic::Ordering::Relaxed);
            }
            #[cfg(feature = "profile")]
            if text.into_iter().flat_map(str::lines).any(|l| l.trim() == "hold") {
                HOLD.store(true, core::sync::atomic::Ordering::Relaxed);
            }
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
