//! Golden-capture scene specs.
//!
//! A `golden_capture` EBOOT reads one line from `capture_scene.txt` beside
//! it and renders that scene, so one build covers every golden instead of one
//! Cargo feature (and one build) per scene. The same one-line spec is the
//! `scene_spec` column of `tests/golden/scenes.tsv`.
//!
//! This crate is `no_std`, allocation-free and free of PSP types so that the
//! parser runs in host tests. The scene behaviour itself (which object, which
//! stage, which light) stays in each PSP binary.

#![no_std]

use core::fmt;

/// Longest spec either binary accepts. `capture_scene.txt` is read into a
/// fixed buffer of this size.
pub const MAX_SPEC_LEN: usize = 64;

/// The file name both binaries look for next to their EBOOT.
pub const SCENE_FILE: &str = "capture_scene.txt";

/// The first non-empty, non-comment line of a scene file, trimmed.
pub fn spec_line(text: &str) -> Option<&str> {
    text.lines()
        .map(str::trim)
        .find(|line| !line.is_empty() && !line.starts_with('#'))
}

/// A fighter with a neutral-model golden in `psp-asset-viewer`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fighter {
    Mario,
    Fox,
    DonkeyKong,
    Samus,
    Luigi,
    Link,
    Yoshi,
    CaptainFalcon,
    Kirby,
    Pikachu,
    Purin,
    Ness,
    MetalMario,
}

impl Fighter {
    pub const ALL: [Fighter; 13] = [
        Fighter::Mario,
        Fighter::Fox,
        Fighter::DonkeyKong,
        Fighter::Samus,
        Fighter::Luigi,
        Fighter::Link,
        Fighter::Yoshi,
        Fighter::CaptainFalcon,
        Fighter::Kirby,
        Fighter::Pikachu,
        Fighter::Purin,
        Fighter::Ness,
        Fighter::MetalMario,
    ];

    /// The spec name, which is also the old `regression_capture_<name>`
    /// feature suffix.
    pub const fn name(self) -> &'static str {
        match self {
            Fighter::Mario => "mario",
            Fighter::Fox => "fox",
            Fighter::DonkeyKong => "donkey_kong",
            Fighter::Samus => "samus",
            Fighter::Luigi => "luigi",
            Fighter::Link => "link",
            Fighter::Yoshi => "yoshi",
            Fighter::CaptainFalcon => "captain_falcon",
            Fighter::Kirby => "kirby",
            Fighter::Pikachu => "pikachu",
            Fighter::Purin => "purin",
            Fighter::Ness => "ness",
            Fighter::MetalMario => "metal_mario",
        }
    }

    pub fn from_name(name: &str) -> Option<Fighter> {
        Fighter::ALL.into_iter().find(|f| f.name() == name)
    }
}

/// One `psp-asset-viewer` golden scene.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ViewerScene {
    /// `default`: Dream Land stage view, Mario at spawn 0.
    DreamLand,
    /// `scene2`: file 52 opening-movie graph.
    OpeningRoom,
    /// `scene3`: file 109 graph 0x44C8.
    StageSector,
    /// `scene4`: file 84 graph 0x2760.
    CatchSwirl,
    /// `scene5`: stage 9, Saffron City gate.
    SaffronGate,
    /// `scene6`: file 117 graph 0x1B10.
    MetalTexgen,
    /// `scene7`: scene 6 a quarter turn round.
    MetalTexgenRotated,
    /// `scene8`: file 117 graph 0x2EE0.
    MetalTexgenLinear,
    /// `scene9`: scene 6 under a rotated camera.
    MetalTexgenCameraRotated,
    /// `scene10`: stage 4, Peach's Castle.
    PeachCastle,
    /// `stage N`: whole-stage view of stage index N.
    Stage(u32),
    /// `fighter NAME`: neutral high-detail model.
    Fighter(Fighter),
    /// `link_costume_1`: Link with costume 1.
    LinkCostume1,
    /// `mario_entry`: file 356 graph 0x608 in the effect browser.
    MarioEntry,
    /// `bonus_platform`: file 136 graph 0x3DA8.
    BonusPlatform,
    /// `depth_mask`: synthetic depth-write ON/OFF/ON quads.
    DepthMask,
    /// `dream_land_water`: file 104 graph 0x2450, Dream Land's water layer,
    /// seen from above: both two-tile fractional blend ponds (RE-321).
    DreamLandWater,
}

impl ViewerScene {
    /// Parses one spec, e.g. `stage 17`, `fighter fox`, `scene3`.
    pub fn parse(spec: &str) -> Option<ViewerScene> {
        let mut words = spec.split_whitespace();
        let head = words.next()?;
        let arg = words.next();
        if words.next().is_some() {
            return None;
        }
        let scene = match (head, arg) {
            ("default", None) => ViewerScene::DreamLand,
            ("scene2", None) => ViewerScene::OpeningRoom,
            ("scene3", None) => ViewerScene::StageSector,
            ("scene4", None) => ViewerScene::CatchSwirl,
            ("scene5", None) => ViewerScene::SaffronGate,
            ("scene6", None) => ViewerScene::MetalTexgen,
            ("scene7", None) => ViewerScene::MetalTexgenRotated,
            ("scene8", None) => ViewerScene::MetalTexgenLinear,
            ("scene9", None) => ViewerScene::MetalTexgenCameraRotated,
            ("scene10", None) => ViewerScene::PeachCastle,
            ("stage", Some(n)) => ViewerScene::Stage(parse_decimal(n)?),
            ("fighter", Some(name)) => ViewerScene::Fighter(Fighter::from_name(name)?),
            ("link_costume_1", None) => ViewerScene::LinkCostume1,
            ("mario_entry", None) => ViewerScene::MarioEntry,
            ("bonus_platform", None) => ViewerScene::BonusPlatform,
            ("depth_mask", None) => ViewerScene::DepthMask,
            ("dream_land_water", None) => ViewerScene::DreamLandWater,
            _ => return None,
        };
        Some(scene)
    }

    /// The fighter whose model, `Wait` animation and light this scene shows.
    pub const fn fighter(self) -> Option<Fighter> {
        match self {
            ViewerScene::Fighter(f) => Some(f),
            ViewerScene::LinkCostume1 => Some(Fighter::Link),
            _ => None,
        }
    }

    /// The stage shown by a whole-stage scene. `None` keeps the viewer's
    /// default stage 0.
    pub const fn stage_index(self) -> Option<u32> {
        match self {
            ViewerScene::SaffronGate => Some(9),
            ViewerScene::PeachCastle => Some(4),
            ViewerScene::Stage(n) => Some(n),
            _ => None,
        }
    }

    /// The `(source file, graph offset)` of the object a scene selects by
    /// exact graph. Fighters come from their own table in the viewer.
    pub const fn object_graph(self) -> Option<(u32, u32)> {
        match self {
            ViewerScene::StageSector => Some((109, 0x44C8)),
            ViewerScene::CatchSwirl => Some((84, 0x2760)),
            ViewerScene::MetalTexgen
            | ViewerScene::MetalTexgenRotated
            | ViewerScene::MetalTexgenCameraRotated => Some((117, 0x1B10)),
            ViewerScene::MetalTexgenLinear => Some((117, 0x2EE0)),
            ViewerScene::BonusPlatform => Some((136, 0x3DA8)),
            ViewerScene::DreamLandWater => Some((104, 0x2450)),
            _ => None,
        }
    }

    /// `(yaw, pitch, distance scale)`, angles in radians, of a real camera
    /// orbiting the object's centre, for scenes that need a view the object
    /// viewer's spin cannot give. `None` keeps the identity view.
    pub const fn orbit_camera(self) -> Option<(f32, f32, f32)> {
        match self {
            // 35 and 20 degrees.
            ViewerScene::MetalTexgenCameraRotated => Some((0.610_865_2, 0.349_065_85, 1.0)),
            // 60 degrees down onto the ponds, facing the stage front, close
            // enough that each pond spans a few hundred pixels.
            ViewerScene::DreamLandWater => Some((0.0, core::f32::consts::FRAC_PI_3, 0.4)),
            _ => None,
        }
    }

    /// Scenes that start in the object viewer instead of the stage view.
    pub const fn object_view(self) -> bool {
        matches!(
            self,
            ViewerScene::OpeningRoom
                | ViewerScene::StageSector
                | ViewerScene::CatchSwirl
                | ViewerScene::MetalTexgen
                | ViewerScene::MetalTexgenRotated
                | ViewerScene::MetalTexgenLinear
                | ViewerScene::MetalTexgenCameraRotated
                | ViewerScene::Fighter(_)
                | ViewerScene::LinkCostume1
                | ViewerScene::MarioEntry
                | ViewerScene::BonusPlatform
                | ViewerScene::DreamLandWater
        )
    }

    /// Scenes whose object does not drift round. The bonus platform and the
    /// entry pipe predate this list and drift until the freeze; their goldens
    /// pin that angle.
    pub const fn holds_spin(self) -> bool {
        matches!(
            self,
            ViewerScene::OpeningRoom
                | ViewerScene::StageSector
                | ViewerScene::CatchSwirl
                | ViewerScene::MetalTexgen
                | ViewerScene::MetalTexgenRotated
                | ViewerScene::MetalTexgenLinear
                | ViewerScene::MetalTexgenCameraRotated
                | ViewerScene::Fighter(_)
                | ViewerScene::LinkCostume1
                | ViewerScene::DreamLandWater
        )
    }
}

impl fmt::Display for ViewerScene {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ViewerScene::DreamLand => f.write_str("default"),
            ViewerScene::OpeningRoom => f.write_str("scene2"),
            ViewerScene::StageSector => f.write_str("scene3"),
            ViewerScene::CatchSwirl => f.write_str("scene4"),
            ViewerScene::SaffronGate => f.write_str("scene5"),
            ViewerScene::MetalTexgen => f.write_str("scene6"),
            ViewerScene::MetalTexgenRotated => f.write_str("scene7"),
            ViewerScene::MetalTexgenLinear => f.write_str("scene8"),
            ViewerScene::MetalTexgenCameraRotated => f.write_str("scene9"),
            ViewerScene::PeachCastle => f.write_str("scene10"),
            ViewerScene::Stage(n) => write!(f, "stage {n}"),
            ViewerScene::Fighter(fighter) => write!(f, "fighter {}", fighter.name()),
            ViewerScene::LinkCostume1 => f.write_str("link_costume_1"),
            ViewerScene::MarioEntry => f.write_str("mario_entry"),
            ViewerScene::BonusPlatform => f.write_str("bonus_platform"),
            ViewerScene::DepthMask => f.write_str("depth_mask"),
            ViewerScene::DreamLandWater => f.write_str("dream_land_water"),
        }
    }
}

/// One `psp-game` scripted Training scene.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GameScene {
    /// `training`: jump, jab, freeze.
    Training,
    /// `fireball`: Training plus Mario's neutral B.
    Fireball,
    /// `superjump`: Training plus Mario's up B.
    Superjump,
    /// `fox`: Fox in Training, one Blaster shot.
    Fox,
    /// `shadows`: first jump's apex, player airborne.
    Shadows,
    /// `grab`: Training plus a Z+A grab of the dummy, frozen while held.
    Grab,
}

impl GameScene {
    pub const ALL: [GameScene; 6] = [
        GameScene::Training,
        GameScene::Fireball,
        GameScene::Superjump,
        GameScene::Fox,
        GameScene::Shadows,
        GameScene::Grab,
    ];

    pub const fn name(self) -> &'static str {
        match self {
            GameScene::Training => "training",
            GameScene::Fireball => "fireball",
            GameScene::Superjump => "superjump",
            GameScene::Fox => "fox",
            GameScene::Shadows => "shadows",
            GameScene::Grab => "grab",
        }
    }

    pub fn parse(spec: &str) -> Option<GameScene> {
        let spec = spec.trim();
        GameScene::ALL.into_iter().find(|s| s.name() == spec)
    }
}

impl fmt::Display for GameScene {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// Unsigned decimal without sign, prefix or leading `+`.
fn parse_decimal(s: &str) -> Option<u32> {
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    s.parse().ok()
}

#[cfg(test)]
mod tests;
