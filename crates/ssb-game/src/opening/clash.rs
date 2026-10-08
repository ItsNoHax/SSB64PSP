//! `mv/mvopening/mvopeningclash.c`: the eight fighters clashing before a
//! four-cornered animated backdrop, then the screen washing out to white.

use alloc::vec;
use alloc::vec::Vec;

use super::movie::{
    link, CameraKind, Fighter, Fill, Head, Joints, Model, ObjectKind, Persp, View, World, FULL,
};
use super::{Exit, Kind};
use crate::fighter::FighterKind;

/// `llMVOpeningClashFightersFileID` and its camera animation.
pub const FILE_FIGHTERS: u32 = 0x48;
pub const FIGHTERS_CAM_ANIM_JOINT: u32 = 0x1440;
/// `llMVOpeningClashWallpaperFileID` and its symbols.
pub const FILE_WALLPAPER: u32 = 0x42;
pub const WALLPAPER_CAM_ANIM_JOINT: u32 = 0x4AB0;
/// The four corners, lower left, lower right, upper left, upper right:
/// `(display list, AnimJoint)` (their `MObjSub`s and `MatAnimJoint`s are
/// paired in the pack, `ssb_rom::opening::MAT_ANIM_JOINTS`).
pub const CORNERS: [(u32, u32); 4] = [
    (0x32A8, 0x36D0),
    (0x3938, 0x3D60),
    (0x3FC8, 0x4400),
    (0x4668, 0x4AA0),
];

/// The cameras' baked plays (`ssb_rom::opening::camera_slot`).
pub const FIGHTERS_CAMERA_SLOT: u32 = 0x4000_0000 | (FILE_FIGHTERS << 20) | FIGHTERS_CAM_ANIM_JOINT;
pub const WALLPAPER_CAMERA_SLOT: u32 =
    0x4000_0000 | (FILE_WALLPAPER << 20) | WALLPAPER_CAM_ANIM_JOINT;

/// `mvOpeningClashMakeFighters`' `fkinds`.
pub const KINDS: [FighterKind; 8] = [
    FighterKind::Mario,
    FighterKind::Kirby,
    FighterKind::Link,
    FighterKind::Yoshi,
    FighterKind::Fox,
    FighterKind::Donkey,
    FighterKind::Samus,
    FighterKind::Pikachu,
];

/// `nFTDemoStatusClash` (0x1000B): submotion row 11.
pub const ROW_CLASH: u8 = 11;

#[derive(Debug, Clone, PartialEq)]
pub struct Clash {
    tics: i32,
    pub world: World,
    /// The fighters' camera and the wallpaper's.
    cameras: [u16; 2],
    fighters: [u16; 8],
    corners: [u16; 4],
    /// `mvOpeningClashMakeVoid`'s display and `sMVOpeningClashVoidAlpha`.
    void: Option<(u16, i32)>,
}

impl Clash {
    /// `mvOpeningClashFuncStart`.
    pub fn new() -> Clash {
        let mut world = World::new();
        world.camera(
            100,
            0,
            FULL,
            CameraKind::Default {
                fill: Some([0, 0, 0, 0xFF]),
                zbuffer: true,
            },
        );
        // `mvOpeningClashMakeFightersCamera`: `COBJ_FLAG_ZBUFFER` added.
        let fighters_camera = world.camera(
            60,
            link(27) | link(9),
            FULL,
            CameraKind::Persp {
                persp: Persp::animated(FIGHTERS_CAMERA_SLOT, View::LookAt, 128.0, 16384.0),
                zbuffer: true,
            },
        );
        // `mvOpeningClashMakeVoidCamera`: its `COBJ_FLAG_ZBUFFER` does
        // nothing for `lbCommonDrawSprite`.
        world.camera(40, link(26), FULL, CameraKind::Sprite);
        // `mvOpeningClashMakeWallpaperCamera`: `func_80017DBC` (head 0
        // only), at the default near and far. Its display's
        // `G_RM_AA_OPA_SURF` is dead: `func_8001663C` sets the camera's
        // `G_RM_AA_ZB_OPA_SURF` over it before the corners draw (the pack
        // seeds them so, `ssb_rom::opening::CAMERA_DEFAULT_GRAPHS`).
        let wallpaper_camera = world.camera(
            90,
            link(29),
            FULL,
            CameraKind::Persp {
                persp: Persp::animated(WALLPAPER_CAMERA_SLOT, View::LookAt, 100.0, 12800.0),
                zbuffer: false,
            },
        );
        let mut fighters = [0; 8];
        for (i, &kind) in KINDS.iter().enumerate() {
            fighters[i] = world.object(
                9,
                Head::Links,
                ObjectKind::Fighter(Fighter::new(kind, 0, ROW_CLASH)),
            );
        }
        // `mvOpeningClashMakeWallpaper`: each corner one `DObj` on its list
        // (`gcDrawDObjDLHead0`), with its materials and joint script.
        let mut corners = [0; 4];
        for (i, &(dl, anim)) in CORNERS.iter().enumerate() {
            corners[i] = world.object(
                29,
                Head::Zero,
                ObjectKind::Model(
                    Model::new(FILE_WALLPAPER, dl, Head::Zero)
                        .joints(Joints::table(FILE_WALLPAPER, anim))
                        .mats(),
                ),
            );
        }
        world.light = [45.0, 45.0];
        Clash {
            tics: 0,
            world,
            cameras: [fighters_camera, wallpaper_camera],
            fighters,
            corners,
            void: None,
        }
    }

    /// One frame: `mvOpeningClashFuncRun`, then the processes in creation
    /// order: the cameras', the fighters' and the corners'; then the void
    /// display's fade.
    pub fn tick(&mut self, tapped: bool) -> Option<Exit> {
        let mut exit = None;
        self.tics += 1;
        if self.tics >= 10 {
            if tapped {
                exit = Some(Exit::Title);
            }
            if self.tics == 144 {
                // `mvOpeningClashMakeVoid`: link 26, head 0.
                let id = self
                    .world
                    .object(26, Head::Zero, ObjectKind::Fills(Vec::new()));
                self.void = Some((id, 0));
            }
            if self.tics == 160 {
                exit = Some(Exit::Next(Kind::Newcomers));
            }
            if matches!(self.tics, 15 | 75 | 90 | 105) {
                crate::sound::play_fgm(crate::sound::id::nSYAudioFGMOpeningClash);
            }
        }
        for id in self.cameras {
            if let Some(p) = self.world.camera_mut(id).and_then(|c| c.persp_mut()) {
                p.play();
            }
        }
        for id in self.fighters {
            if let Some(f) = self.world.fighter_mut(id) {
                f.plays += 1;
            }
        }
        for id in self.corners {
            if let Some(m) = self.world.model_mut(id) {
                m.play();
            }
        }
        if let Some((id, alpha)) = self.void.as_mut() {
            // `mvOpeningClashVoidProcDisplay` raises the alpha once a drawn
            // frame, then fills the viewport white.
            if *alpha < 0xFF {
                *alpha += 0x1E;
                if *alpha > 0xFF {
                    *alpha = 0xFF;
                }
            }
            let a = *alpha as u8;
            let id = *id;
            if let Some(f) = self.world.fills_mut(id) {
                *f = vec![Fill {
                    rect: FULL,
                    rgba: [0xFF, 0xFF, 0xFF, a],
                }];
            }
        }
        exit
    }

    /// The void's alpha this frame (0 before it is made).
    pub fn void_alpha(&self) -> u8 {
        self.void.map_or(0, |(_, a)| a as u8)
    }
}

impl Default for Clash {
    fn default() -> Self {
        Clash::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_void_washes_in_from_144_and_the_scene_ends_at_160() {
        let mut c = Clash::new();
        for _ in 0..143 {
            c.tick(false);
        }
        assert_eq!(c.void_alpha(), 0);
        c.tick(false);
        assert_eq!(c.void_alpha(), 0x1E);
        let mut exit = None;
        for _ in 144..160 {
            exit = c.tick(false);
        }
        assert_eq!(exit, Some(Exit::Next(Kind::Newcomers)));
        assert_eq!(c.void_alpha(), 0xFF);
        let corner = c.corners[0];
        let m = c.world.model_mut(corner).unwrap();
        assert_eq!((m.joints.unwrap().plays, m.mats), (160, Some(160)));
    }

    #[test]
    fn a_tap_before_tic_10_does_nothing() {
        let mut c = Clash::new();
        for _ in 0..9 {
            assert_eq!(c.tick(true), None);
        }
        assert_eq!(c.tick(true), Some(Exit::Title));
    }
}
