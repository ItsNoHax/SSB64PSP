//! The stage wallpaper, from `gr/grwallpaper.c` and Training's
//! `sc1PTrainingModeLoadWallpaper` (RE-419).
//!
//! `grWallpaperMakeDecideKind` makes one `SObj` behind the stage, drawn by
//! `gmCameraMakeWallpaperCamera`'s camera (priority 80, before the stage
//! camera's 50) over the battle viewport `(10, 10)` to `(310, 230)`. The
//! sprite is `MPGroundData.wallpaper`, a 300 x 220 RGBA16 image; Training
//! swaps in one of three of its own. How it moves depends on the kind:
//!
//! - [`Kind::Common`] (`grWallpaperMakeCommon`, most stages): each update
//!   (`grWallpaperCalcPersp`) scales it by the camera's distance and pans it
//!   by the camera's angle, clamped so it always covers the viewport.
//! - [`Kind::Static`] (`grWallpaperMakeStatic`: Training, Yoshi's Island
//!   and the bonus stages): fixed at `(10, 10)`, scale 1.
//! - [`Kind::Sector`] (`grWallpaperMakeSector`, Sector Z): scaled by the
//!   camera's distance about its centre, never panned.
//! - [`Kind::Bonus3`] (`grWallpaperMakeBonus3`, Race to the Finish): no
//!   sprite; a black fill of `(10, 10)` to `(310, 230)`.
//!
//! The update runs at process priority 3, after the battle camera's (also 3,
//! made first), so it reads the camera of the same tick. The pause flag
//! `grWallpaperPausePerspUpdate` sets is never read; while paused
//! `ifCommonBattlePauseUpdateInterface` runs the camera and then the
//! wallpaper's process by hand, so the wallpaper follows the pause camera.

use ssb_engine::math::{self, Vec3};

/// `nGRKind` values the decision reads (`gr/grdef.h`).
pub mod gkind {
    pub const SECTOR: u8 = 1;
    pub const YOSTER: u8 = 5;
    pub const YOSTER_SMALL: u8 = 12;
    pub const BONUS3: u8 = 15;
    pub const LAST: u8 = 16;
    /// `nGRKindBonusStageStart`: the Break the Targets and Board the
    /// Platforms stages from here on.
    pub const BONUS_STAGE_START: u8 = 17;
}

/// `SP_TEXSHUF`.
pub const SP_TEXSHUF: u16 = 0x0200;
/// `SP_FASTCOPY`.
pub const SP_FASTCOPY: u16 = 0x0020;

/// The image's size (`grWallpaperCalcPersp`'s 300 and 220).
pub const WIDTH: f32 = 300.0;
pub const HEIGHT: f32 = 220.0;

/// How the wallpaper is made and moved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Common,
    Static,
    Sector,
    Bonus3,
}

/// `grWallpaperMakeDecideKind`. Final Destination
/// (`sc1PGameBossInitWallpaper`) swaps its sprite first and is otherwise
/// [`Kind::Common`].
pub fn decide_kind(is_training: bool, gkind: u8) -> Kind {
    if is_training || gkind >= gkind::BONUS_STAGE_START {
        return Kind::Static;
    }
    match gkind {
        gkind::YOSTER | gkind::YOSTER_SMALL => Kind::Static,
        gkind::SECTOR => Kind::Sector,
        gkind::BONUS3 => Kind::Bonus3,
        _ => Kind::Common,
    }
}

/// `dSC1PTrainingModeWallpaperIDs`: which of Training's wallpapers
/// ([`TRAINING_WALLPAPERS`]) each VS stage shows. `mnMapsPreviewWallpaper`'s
/// copy, `dMNMapsTrainingModeWallpaperIDs`, is the same with a tenth 0.
const TRAINING_WALLPAPER_IDS: [u8; 9] = [2, 0, 0, 0, 2, 1, 2, 2, 2];

/// Training's wallpapers (`dSC1PTrainingModeWallpaperDescs`): the file
/// (`llGRWallpaperTraining*FileID`) and the fog colour
/// `sc1PTrainingModeInitDisplayVars` writes over `MPGroundData.fog_color`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrainingWallpaper {
    pub file: u32,
    pub fog_color: [u8; 3],
}

/// Black, yellow and blue.
pub const TRAINING_WALLPAPERS: [TrainingWallpaper; 3] = [
    TrainingWallpaper {
        file: 0x1A,
        fog_color: [0x00, 0x00, 0x00],
    },
    TrainingWallpaper {
        file: 0x1B,
        fog_color: [0xEE, 0x9E, 0x06],
    },
    TrainingWallpaper {
        file: 0x1C,
        fog_color: [0xAF, 0xF5, 0xFF],
    },
];

/// `llGRWallpaperTraining*Sprite`: the sprite's offset in each file.
pub const TRAINING_WALLPAPER_SPRITE: u32 = 0x20718;

/// The wallpaper Training shows on the VS stage `gkind`, or `None` for any
/// other kind.
pub fn training_wallpaper(gkind: u8) -> Option<TrainingWallpaper> {
    TRAINING_WALLPAPER_IDS
        .get(usize::from(gkind))
        .map(|&id| TRAINING_WALLPAPERS[usize::from(id)])
}

/// The wallpaper `SObj`: its top-left corner on the 320 x 240 screen, its
/// `sprite.scalex` (= `scaley`) and `sprite.attr`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Wallpaper {
    pub kind: Kind,
    pub x: f32,
    pub y: f32,
    pub scale: f32,
    pub attr: u16,
}

impl Wallpaper {
    /// `grWallpaperMake*`: the `SObj` as made, before any update.
    pub fn make(kind: Kind) -> Self {
        let (scale, attr) = match kind {
            Kind::Common => (1.004, SP_TEXSHUF),
            Kind::Static => (1.0, SP_TEXSHUF | SP_FASTCOPY),
            // `grWallpaperMakeSector` keeps the sprite's own scale (1) and
            // sets only `SP_TEXSHUF`.
            Kind::Sector => (1.0, SP_TEXSHUF),
            Kind::Bonus3 => (1.0, 0),
        };
        Wallpaper {
            kind,
            x: 10.0,
            y: 10.0,
            scale,
            attr,
        }
    }

    /// One tick of the wallpaper's process from the battle camera's `eye`
    /// and `at`: `grWallpaperCommonProcUpdate` or
    /// `grWallpaperSectorProcUpdate`. The static kinds have none.
    pub fn update(&mut self, eye: Vec3, at: Vec3) {
        match self.kind {
            Kind::Common => self.calc_persp(eye, at),
            Kind::Sector => self.sector_update(eye, at),
            Kind::Static | Kind::Bonus3 => {}
        }
    }

    /// `grWallpaperCalcPersp`.
    fn calc_persp(&mut self, eye: Vec3, at: Vec3) {
        let dist = Vec3::new(eye.x - at.x, eye.y - at.y, eye.z - at.z);
        let mag = math::sqrt(dist.x * dist.x + dist.y * dist.y + dist.z * dist.z);
        let (angle_x, angle_y) = if dist.z < 0.0 {
            (0.0, 0.0)
        } else {
            (arc_tan2(dist.y, dist.z), arc_tan2(dist.x, dist.z))
        };
        let scale = (20000.0 / (mag + 8000.0)).clamp(1.004, 2.0);
        self.scale = scale;
        let width = WIDTH * scale;
        let height = HEIGHT * scale;
        let pi = core::f32::consts::PI;
        let bak_x = (angle_y / pi) * width - (width - 320.0) * 0.5;
        let bak_y = (-angle_x / pi) * height - (height - 240.0) * 0.5;
        self.x = if bak_x > 10.0 {
            10.0
        } else {
            bak_x.max(-width - 10.0 + 320.0)
        };
        self.y = if bak_y > 10.0 {
            10.0
        } else {
            bak_y.max(-height - 10.0 + 240.0)
        };
    }

    /// `grWallpaperSectorProcUpdate`.
    fn sector_update(&mut self, eye: Vec3, at: Vec3) {
        let dist = Vec3::new(eye.x - at.x, eye.y - at.y, eye.z - at.z);
        let len = math::sqrt(dist.x * dist.x + dist.y * dist.y + dist.z * dist.z);
        if len > 0.0 {
            let scale = (20000.0 / (len + 10000.0)).clamp(1.004, 2.0);
            let grow = (scale - 1.0) * 0.5;
            self.scale = scale;
            self.x = 10.0 - WIDTH * grow;
            self.y = 10.0 - HEIGHT * grow;
        }
    }
}

/// `syUtilsArcTan2`: `atan(y / x)`, mirrored for a negative `x`, and 0 at
/// the origin.
fn arc_tan2(y: f32, x: f32) -> f32 {
    if x == 0.0 && y == 0.0 {
        0.0
    } else {
        math::atan2(y, x)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_kinds_follow_the_mode_and_stage() {
        assert_eq!(decide_kind(true, gkind::SECTOR), Kind::Static);
        assert_eq!(decide_kind(false, 0), Kind::Common);
        assert_eq!(decide_kind(false, gkind::SECTOR), Kind::Sector);
        assert_eq!(decide_kind(false, gkind::YOSTER), Kind::Static);
        assert_eq!(decide_kind(false, gkind::YOSTER_SMALL), Kind::Static);
        assert_eq!(decide_kind(false, gkind::BONUS3), Kind::Bonus3);
        assert_eq!(decide_kind(false, gkind::LAST), Kind::Common);
        assert_eq!(decide_kind(false, gkind::BONUS_STAGE_START), Kind::Static);
        assert_eq!(decide_kind(false, 40), Kind::Static);
    }

    #[test]
    fn training_picks_its_own_wallpaper_and_fog() {
        // Peach's Castle, Hyrule, Dream Land, Saffron and Mushroom Kingdom
        // blue; Sector Z, Kongo Jungle and Zebes black; Yoshi's Island
        // yellow.
        let files: [u32; 9] = core::array::from_fn(|k| training_wallpaper(k as u8).unwrap().file);
        assert_eq!(
            files,
            [0x1C, 0x1A, 0x1A, 0x1A, 0x1C, 0x1B, 0x1C, 0x1C, 0x1C]
        );
        assert_eq!(training_wallpaper(6).unwrap().fog_color, [0xAF, 0xF5, 0xFF]);
        assert_eq!(training_wallpaper(5).unwrap().fog_color, [0xEE, 0x9E, 0x06]);
        assert_eq!(training_wallpaper(9), None);
    }

    #[test]
    fn the_made_sobjs_keep_their_sources_fields() {
        let c = Wallpaper::make(Kind::Common);
        assert_eq!((c.x, c.y, c.scale, c.attr), (10.0, 10.0, 1.004, SP_TEXSHUF));
        let s = Wallpaper::make(Kind::Static);
        assert_eq!((s.scale, s.attr), (1.0, SP_TEXSHUF | SP_FASTCOPY));
        let mut st = s;
        st.update(Vec3::new(500.0, 900.0, 3000.0), Vec3::new(0.0, 0.0, 0.0));
        assert_eq!(st, s, "a static wallpaper has no process");
    }

    #[test]
    fn a_camera_straight_on_centres_the_common_wallpaper() {
        let mut w = Wallpaper::make(Kind::Common);
        // 12,000 away: 20000 / 20000 = 1, clamped up to 1.004.
        w.update(Vec3::new(0.0, 0.0, 12000.0), Vec3::new(0.0, 0.0, 0.0));
        assert_eq!(w.scale, 1.004);
        let width = 300.0 * 1.004;
        let height = 220.0 * 1.004;
        assert_eq!(w.x, -(width - 320.0) * 0.5);
        assert_eq!(w.y, -(height - 240.0) * 0.5);
    }

    #[test]
    fn a_close_camera_zooms_and_an_angle_pans_within_the_viewport() {
        let mut w = Wallpaper::make(Kind::Common);
        // 2,000 away: 20000 / 10000 = 2, the cap.
        w.update(Vec3::new(0.0, 0.0, 2000.0), Vec3::new(0.0, 0.0, 0.0));
        assert_eq!(w.scale, 2.0);
        assert_eq!(w.x, -(600.0 - 320.0) * 0.5);
        // Far to the right: the pan clamps at the left edge, 10 in.
        w.update(Vec3::new(1_000_000.0, 0.0, 1.0), Vec3::new(0.0, 0.0, 0.0));
        assert_eq!(w.x, 10.0);
        // Far to the left: the right edge stays at 310.
        w.update(Vec3::new(-1_000_000.0, 0.0, 1.0), Vec3::new(0.0, 0.0, 0.0));
        assert_eq!(w.x, -300.0 * w.scale - 10.0 + 320.0);
        // Looking down from above pans the image up (y falls): the clamp
        // holds its bottom edge at 230.
        w.update(Vec3::new(0.0, 1_000_000.0, 1.0), Vec3::new(0.0, 0.0, 0.0));
        assert_eq!(w.y, -220.0 * w.scale - 10.0 + 240.0);
        // A camera behind the target (dist.z < 0) reads as no angle.
        w.update(Vec3::new(5000.0, 5000.0, -3000.0), Vec3::new(0.0, 0.0, 0.0));
        let width = 300.0 * w.scale;
        assert_eq!(w.x, -(width - 320.0) * 0.5);
    }

    #[test]
    fn a_small_pan_follows_the_angle_linearly() {
        let mut w = Wallpaper::make(Kind::Common);
        let eye = Vec3::new(1000.0, -500.0, 6000.0);
        w.update(eye, Vec3::new(0.0, 0.0, 0.0));
        let scale =
            20000.0 / (math::sqrt(1000.0f32 * 1000.0 + 500.0 * 500.0 + 6000.0 * 6000.0) + 8000.0);
        assert!((w.scale - scale).abs() < 1e-6);
        let width = 300.0 * scale;
        let height = 220.0 * scale;
        let pi = core::f32::consts::PI;
        let x = math::atan2(1000.0, 6000.0) / pi * width - (width - 320.0) * 0.5;
        let y = -math::atan2(-500.0, 6000.0) / pi * height - (height - 240.0) * 0.5;
        assert!((w.x - x).abs() < 1e-4);
        assert!((w.y - y).abs() < 1e-4);
    }

    #[test]
    fn sector_z_scales_about_its_centre() {
        let mut w = Wallpaper::make(Kind::Sector);
        w.update(Vec3::new(0.0, 0.0, 5000.0), Vec3::new(0.0, 0.0, 0.0));
        let scale = 20000.0f32 / 15000.0;
        assert_eq!(w.scale, scale);
        assert_eq!(w.x, 10.0 - 300.0 * (scale - 1.0) * 0.5);
        assert_eq!(w.y, 10.0 - 220.0 * (scale - 1.0) * 0.5);
        // The camera on its target leaves the last update standing.
        let before = w;
        w.update(Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.0, 0.0, 0.0));
        assert_eq!(w, before);
    }
}
