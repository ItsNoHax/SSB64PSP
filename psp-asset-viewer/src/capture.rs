//! Golden-capture scene selection.
//!
//! A `golden_capture` (or `capture_scene_file`) build reads its scene from
//! `capture_scene.txt` beside the EBOOT (`ssb_capture` owns the spec format),
//! so one EBOOT renders every golden. Without the file, or in a build without
//! either feature, the old per-scene Cargo features pick a compile-time
//! default. A build with neither is the interactive viewer and has no
//! capture scene.

pub use ssb_capture::{Fighter, ViewerScene as CaptureScene};

/// The scene this run captures, and where it came from.
#[derive(Clone, Copy)]
pub struct Capture {
    pub scene: CaptureScene,
    /// `true` when `capture_scene.txt` chose the scene. Only then does a
    /// `golden_capture` build exit after its screenshot: the old per-scene
    /// builds keep running until the runner's timeout, as before.
    #[cfg_attr(not(feature = "golden_capture"), allow(dead_code))]
    pub from_file: bool,
}

/// Picks this run's capture scene once at startup.
pub fn select() -> Option<Capture> {
    #[cfg(feature = "capture_scene_file")]
    {
        let mut buf = [0u8; ssb_capture::MAX_SPEC_LEN];
        if let Some(len) = ssb_psp_runtime::assets::read_capture_scene(&mut buf) {
            // An unreadable spec is a harness error. Fall through to the
            // default scene; the runner's candidate then differs from its
            // reference and the mismatch is reported there.
            if let Some(scene) = core::str::from_utf8(&buf[..len])
                .ok()
                .and_then(ssb_capture::spec_line)
                .and_then(CaptureScene::parse)
            {
                return Some(Capture { scene, from_file: true });
            }
        }
    }
    default_scene().map(|scene| Capture { scene, from_file: false })
}

/// The scene named by the old per-scene Cargo features. Checked in the same
/// order the old `cfg!` chains used, most specific first.
fn default_scene() -> Option<CaptureScene> {
    let scene = if cfg!(feature = "regression_capture_scene2") {
        CaptureScene::OpeningRoom
    } else if cfg!(feature = "regression_capture_scene3") {
        CaptureScene::StageSector
    } else if cfg!(feature = "regression_capture_scene4") {
        CaptureScene::CatchSwirl
    } else if cfg!(feature = "regression_capture_scene5") {
        CaptureScene::SaffronGate
    } else if cfg!(feature = "regression_capture_scene6") {
        CaptureScene::MetalTexgen
    } else if cfg!(feature = "regression_capture_scene7") {
        CaptureScene::MetalTexgenRotated
    } else if cfg!(feature = "regression_capture_scene8") {
        CaptureScene::MetalTexgenLinear
    } else if cfg!(feature = "regression_capture_scene9") {
        CaptureScene::MetalTexgenCameraRotated
    } else if cfg!(feature = "regression_capture_scene10") {
        CaptureScene::PeachCastle
    } else if cfg!(feature = "regression_capture_stage_index") {
        CaptureScene::Stage(
            option_env!("SSB64_STAGE_INDEX")
                .and_then(|s| s.parse::<u32>().ok())
                .unwrap_or(0),
        )
    } else if cfg!(feature = "regression_capture_link_costume_1") {
        CaptureScene::LinkCostume1
    } else if let Some(fighter) = default_fighter() {
        CaptureScene::Fighter(fighter)
    } else if cfg!(feature = "regression_capture_mario_entry") {
        CaptureScene::MarioEntry
    } else if cfg!(feature = "regression_capture_bonus_platform") {
        CaptureScene::BonusPlatform
    } else if cfg!(feature = "depth_mask_diagnostic") {
        CaptureScene::DepthMask
    } else if cfg!(feature = "regression_capture") {
        CaptureScene::DreamLand
    } else {
        return None;
    };
    Some(scene)
}

fn default_fighter() -> Option<Fighter> {
    let fighter = if cfg!(feature = "regression_capture_mario") {
        Fighter::Mario
    } else if cfg!(feature = "regression_capture_fox") {
        Fighter::Fox
    } else if cfg!(feature = "regression_capture_donkey_kong") {
        Fighter::DonkeyKong
    } else if cfg!(feature = "regression_capture_samus") {
        Fighter::Samus
    } else if cfg!(feature = "regression_capture_luigi") {
        Fighter::Luigi
    } else if cfg!(feature = "regression_capture_link") {
        Fighter::Link
    } else if cfg!(feature = "regression_capture_yoshi") {
        Fighter::Yoshi
    } else if cfg!(feature = "regression_capture_captain_falcon") {
        Fighter::CaptainFalcon
    } else if cfg!(feature = "regression_capture_kirby") {
        Fighter::Kirby
    } else if cfg!(feature = "regression_capture_pikachu") {
        Fighter::Pikachu
    } else if cfg!(feature = "regression_capture_purin") {
        Fighter::Purin
    } else if cfg!(feature = "regression_capture_ness") {
        Fighter::Ness
    } else if cfg!(feature = "regression_capture_metal_mario") {
        Fighter::MetalMario
    } else {
        return None;
    };
    Some(fighter)
}

/// Accessors over an optional scene, so the viewer's call sites need no
/// `match` and read as the old `cfg!` checks did.
pub trait SceneExt {
    fn is(self, scene: CaptureScene) -> bool;
    fn fighter(self) -> Option<Fighter>;
    fn stage_index(self) -> Option<u32>;
    fn object_graph(self) -> Option<(u32, u32)>;
    fn object_view(self) -> bool;
    fn orbit_camera(self) -> Option<(f32, f32, f32)>;
    fn holds_spin(self) -> bool;
    /// The Mario entry pipe is a frame of the effect browser, HUD line
    /// included.
    fn effect_audit(self) -> bool;
}

impl SceneExt for Option<CaptureScene> {
    fn is(self, scene: CaptureScene) -> bool {
        self == Some(scene)
    }
    fn fighter(self) -> Option<Fighter> {
        self.and_then(CaptureScene::fighter)
    }
    fn stage_index(self) -> Option<u32> {
        self.and_then(CaptureScene::stage_index)
    }
    fn object_graph(self) -> Option<(u32, u32)> {
        self.and_then(CaptureScene::object_graph)
    }
    fn object_view(self) -> bool {
        self.is_some_and(CaptureScene::object_view)
    }
    fn orbit_camera(self) -> Option<(f32, f32, f32)> {
        self.and_then(CaptureScene::orbit_camera)
    }
    fn holds_spin(self) -> bool {
        self.is_some_and(CaptureScene::holds_spin)
    }
    fn effect_audit(self) -> bool {
        cfg!(feature = "effect_audit_capture") || self.is(CaptureScene::MarioEntry)
    }
}
