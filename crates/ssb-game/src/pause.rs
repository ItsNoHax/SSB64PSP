//! The VS pause menu — `ifCommonBattlePause*` in `if/ifcommon.c`: START
//! during Go pauses the battle, hides the HUD, zooms the camera on the
//! pausing player (`gmCameraSetStatusPlayerZoom`) and shows the menu's white
//! border and decals from file 197 (`ssb_rom::sprite::BATTLE_PAUSE`, which
//! [`Decal::sprite`] indexes). The stick turns the view; START resumes, the
//! camera easing back for 20 ticks; A+B+R+Z resets the battle.

use crate::battle::{Battle, GameStatus};
use crate::camera::Bounds;
use ssb_engine::input::N64Buttons;
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

/// `dIFCommonBattlePauseDecalsSpriteData` (`if/ifcommon.c`): per decal
/// `sprite, x, y, prim[3], env[3]` (`TableId::IfPauseDecals`).
static DECAL_ROWS: ssb_tables::Table<[[i16; 9]; 14]> =
    ssb_tables::Table::new(ssb_tables::TableId::IfPauseDecals);

/// Decal `i` of [`DECAL_ROWS`].
fn decal(i: usize) -> Decal {
    let r = DECAL_ROWS[i];
    Decal {
        sprite: r[0] as u8,
        pos: (r[1], r[2]),
        prim: [r[3] as u8, r[4] as u8, r[5] as u8],
        env: [r[6] as u8, r[7] as u8, r[8] as u8],
    }
}

/// The decals every pause shows (`dIFCommonBattlePauseDecalsSpriteData[..12]`).
pub static DECALS: ssb_tables::Decoded<[Decal; 12]> =
    ssb_tables::Decoded::new(|| core::array::from_fn(decal));
/// L and RETRY, which bonus courses add (`...[12..14]`).
pub static BONUS_RETRY: ssb_tables::Decoded<[Decal; 2]> =
    ssb_tables::Decoded::new(|| core::array::from_fn(|i| decal(12 + i)));
/// `dIFCommonBattlePauseBorderRectangle`: `ulx, uly, lrx, lry`
/// (`TableId::IfPauseBorder`).
pub static BORDER: ssb_tables::Table<[[i16; 4]; 5]> =
    ssb_tables::Table::new(ssb_tables::TableId::IfPauseBorder);

/// `ifCommonBattlePauseMakeSObjsAll`: ten decals unless the camera zooms.
pub fn decals(kind: PauseKind) -> &'static [Decal] {
    match kind {
        PauseKind::Default => DECALS.get(),
        PauseKind::PlayerNA | PauseKind::Bonus => &DECALS.get()[..10],
    }
}

/// `ifCommonBattlePausePlayerNumMakeSObj`: "1P" to "4P" (sprites 0 to 3)
/// at (213, 191), white.
pub const PLAYER_NUM_POS: (i16, i16) = (213, 191);

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

/// The pause menu's choices at the pause (`sIFCommonBattlePause*`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PauseState {
    pub kind: PauseKind,
    /// Bonus Practice's L: RETRY (`ifCommonBattlePauseMakeSObjsAll`).
    pub retry: bool,
    /// `sIFCommonBattlePauseCameraEyeXOrigin`/`YOrigin`.
    pub origin: (f32, f32),
    /// `sIFCommonBattlePausePlayerDetail`: the zoomed player draws at high
    /// detail until the battle resumes (RE-426).
    pub detail: crate::modelpart::Detail,
    pub map_origin: Vec3,
    pub map_target: Vec3,
}

/// A, B, R and Z: the reset combo.
pub const QUIT_COMBO: u16 = N64Buttons::A | N64Buttons::B | N64Buttons::R | N64Buttons::Z;

/// `ifCommonBattlePauseUpdateInterface`'s reset: on a frame with any tap
/// (other than START, which resumes first), all four of A, B, R and Z held
/// (`ifcommon.c:3021-3044`).
pub fn quits(tapped: N64Buttons, held: N64Buttons) -> bool {
    tapped.0 != 0 && !tapped.contains(N64Buttons::START) && held.contains_all(QUIT_COMBO)
}

/// The reset from the pause menu: [`quits`], then `is_reset` and
/// `ifCommonBattleInterfaceProcSet` ([`Battle::reset`]), with the menu
/// hidden at once (`ifCommonBattlePauseSetGObjFlagsAll(GOBJ_FLAG_HIDDEN)`),
/// so it never shows again past the reset.
pub fn quit(
    battle: &mut Battle,
    menu: &mut Option<PauseState>,
    tapped: N64Buttons,
    held: N64Buttons,
) -> bool {
    if battle.status != GameStatus::Pause || !quits(tapped, held) {
        return false;
    }
    battle.reset();
    *menu = None;
    true
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

    #[test]
    fn the_reset_needs_all_four_of_a_b_r_and_z() {
        let b = |m: u16| N64Buttons(m);
        let all = QUIT_COMBO;
        assert!(quits(b(N64Buttons::Z), b(all)));
        for one in [N64Buttons::A, N64Buttons::B, N64Buttons::R, N64Buttons::Z] {
            assert!(!quits(b(one), b(one)), "{one:#x} alone");
            assert!(!quits(b(N64Buttons::A), b(all & !one)), "all but {one:#x}");
        }
        // No tap this frame, or START (it resumes instead).
        assert!(!quits(b(0), b(all)));
        assert!(!quits(b(N64Buttons::START), b(all | N64Buttons::START)));
        assert!(quits(b(N64Buttons::L), b(all | N64Buttons::L)));
    }
}
