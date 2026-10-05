//! The VS pause menu — `ifCommonBattlePause*` in `if/ifcommon.c`: START
//! during Go pauses the battle, hides the HUD, zooms the camera on the
//! pausing player (`gmCameraSetStatusPlayerZoom`) and shows the menu's white
//! border and decals from file 197 (`ssb_rom::sprite::BATTLE_PAUSE`, which
//! [`Decal::sprite`] indexes). The stick turns the view; START resumes, the
//! camera easing back for 20 ticks; A+B+R+Z resets the battle.

use crate::camera::Bounds;
use ssb_engine::math::Vec3;

/// `nIFPauseKind*`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PauseKind {
    /// The camera zooms on the player; all twelve decals.
    Default,
    /// The player is out of the camera bounds: no zoom, no camera decals.
    PlayerNA,
    /// A bonus course zooms to its authored map points and adds L: RETRY.
    Bonus,
}

/// `IFPauseDecal`: a file 197 sprite, its top-left corner and colours.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Decal {
    pub sprite: u8,
    pub pos: (i16, i16),
    pub prim: [u8; 3],
    pub env: [u8; 3],
}

const fn decal(sprite: u8, x: i16, y: i16, prim: [u8; 3], env: [u8; 3]) -> Decal {
    Decal {
        sprite,
        pos: (x, y),
        prim,
        env,
    }
}

const WHITE: [u8; 3] = [0xFF; 3];
const BLACK: [u8; 3] = [0; 3];
const GREY: [u8; 3] = [0x80; 3];
const DARK: [u8; 3] = [0x21; 3];

/// The common pause-menu decals used by the VS presentation. Bonus courses
/// add their L: RETRY labels separately from rows 12 and 13 of source data.
pub const DECALS: [Decal; 12] = [
    decal(4, 232, 191, WHITE, BLACK),
    decal(9, 99, 203, [0x00, 0x95, 0xFF], [0x00, 0x05, 0xC7]),
    decal(10, 122, 203, [0x36, 0xBF, 0x00], [0x00, 0x30, 0x00]),
    decal(11, 145, 202, GREY, DARK),
    decal(12, 164, 203, GREY, DARK),
    decal(5, 113, 206, WHITE, BLACK),
    decal(5, 136, 206, WHITE, BLACK),
    decal(5, 155, 206, WHITE, BLACK),
    decal(6, 182, 205, WHITE, BLACK),
    decal(7, 198, 191, WHITE, BLACK),
    decal(13, 21, 19, [0xFF, 0x00, 0x00], BLACK),
    decal(14, 31, 29, WHITE, [0x14, 0x18, 0x11]),
];

/// `ifCommonBattlePauseMakeSObjsAll`: ten decals unless the camera zooms.
pub fn decals(kind: PauseKind) -> &'static [Decal] {
    match kind {
        PauseKind::Default => &DECALS,
        PauseKind::PlayerNA | PauseKind::Bonus => &DECALS[..10],
    }
}

/// `dIFCommonBattlePauseDecalsSpriteData[12..14]`: L and RETRY on bonus courses.
pub const BONUS_RETRY: [Decal; 2] = [
    decal(15, 34, 203, GREY, DARK),
    decal(8, 51, 205, WHITE, BLACK),
];

/// `ifCommonBattlePausePlayerNumMakeSObj`: "1P" to "4P" (sprites 0 to 3)
/// at (213, 191), white.
pub const PLAYER_NUM_POS: (i16, i16) = (213, 191);

/// `dIFCommonBattlePauseBorderRectangle`: `ulx, uly, lrx, lry`, filled in
/// white (`G_CYC_FILL`, both corners inclusive).
pub const BORDER: [[i16; 4]; 5] = [
    [26, 24, 294, 26],
    [26, 24, 28, 199],
    [26, 197, 190, 199],
    [292, 24, 294, 199],
    [279, 197, 294, 199],
];

/// `ifCommonBattleGoUpdateInterface`'s choice:
/// `gmCameraCheckPausePlayerOutBounds`.
pub fn kind_for(pos: Vec3, camera: Bounds) -> PauseKind {
    let out = pos.x < camera.left
        || pos.x > camera.right
        || pos.y < camera.bottom
        || pos.y > camera.top
        || pos.z < -1000.0
        || pos.z > 1000.0;
    if out {
        PauseKind::PlayerNA
    } else {
        PauseKind::Default
    }
}

/// `gmCameraSetStatusPlayerZoom(fighter, 0, 0, closeup_camera_zoom, 0.1, 29)`.
pub const ZOOM_PAN_SCALE: f32 = 0.1;
pub const ZOOM_FOV: f32 = 29.0;

/// `ifCommonBattlePauseUpdateInterface`'s stick: past 8 on an axis, turn
/// the view by `stick * 0.000333` radians, within ±50° across and ±20°
/// up and down (Y inverted).
pub fn steer(eye: &mut (f32, f32), stick_x: i8, stick_y: i8) {
    let (x, y) = (f32::from(stick_x), f32::from(stick_y));
    if x.abs() > 8.0 {
        eye.0 = (eye.0 + x * 0.000333).clamp((-50.0f32).to_radians(), 50.0f32.to_radians());
    }
    if y.abs() > 8.0 {
        eye.1 = (eye.1 - y * 0.000333).clamp((-20.0f32).to_radians(), 20.0f32.to_radians());
    }
}

/// `ifCommonBattlePauseRestoreInterfaceAll`: each restore tick eases the
/// turn 10% back toward where it was at the pause.
pub fn ease_back(eye: &mut (f32, f32), origin: (f32, f32)) {
    eye.0 += (origin.0 - eye.0) * 0.1;
    eye.1 += (origin.1 - eye.1) * 0.1;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bounds() -> Bounds {
        Bounds {
            top: 3000.0,
            bottom: -1000.0,
            left: -4000.0,
            right: 4000.0,
        }
    }

    #[test]
    fn a_player_in_bounds_gets_the_zoom_and_all_decals() {
        let k = kind_for(Vec3::new(100.0, 0.0, 0.0), bounds());
        assert_eq!(k, PauseKind::Default);
        assert_eq!(decals(k).len(), 12);
        let k = kind_for(Vec3::new(5000.0, 0.0, 0.0), bounds());
        assert_eq!(k, PauseKind::PlayerNA);
        assert_eq!(decals(k).len(), 10);
    }

    #[test]
    fn the_stick_turns_the_view_within_limits() {
        let mut eye = (0.0, 0.0);
        steer(&mut eye, 8, -8);
        assert_eq!(eye, (0.0, 0.0), "the dead zone");
        steer(&mut eye, 80, 80);
        assert!((eye.0 - 80.0 * 0.000333).abs() < 1e-7);
        assert!((eye.1 + 80.0 * 0.000333).abs() < 1e-7);
        for _ in 0..1000 {
            steer(&mut eye, 80, -80);
        }
        assert_eq!(eye, (50.0f32.to_radians(), 20.0f32.to_radians()));
        for _ in 0..60 {
            ease_back(&mut eye, (0.0, 0.0));
        }
        assert!(eye.0.abs() < 0.01 && eye.1.abs() < 0.01);
    }
}
