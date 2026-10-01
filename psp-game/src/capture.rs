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

/// Picks this run's capture scene once at startup.
pub fn select() -> Option<Capture> {
    #[cfg(feature = "capture_scene_file")]
    {
        let mut buf = [0u8; ssb_capture::MAX_SPEC_LEN];
        if let Some(len) = ssb_psp_runtime::assets::read_capture_scene(&mut buf) {
            // `scene@tick` captures the scene at another tick, for matching
            // an N64 frame (RE-425); the goldens name none.
            let line = core::str::from_utf8(&buf[..len]).ok().and_then(ssb_capture::spec_line);
            let (name, tick) = match line.and_then(|l| l.split_once('@')) {
                Some((name, tick)) => (Some(name), tick.trim().parse::<u32>().ok()),
                None => (line, None),
            };
            if let Some(scene) = name.and_then(GameScene::parse) {
                if let Some(tick) = tick {
                    TICK_OVERRIDE.store(tick, core::sync::atomic::Ordering::Relaxed);
                }
                return Some(Capture {
                    scene,
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
