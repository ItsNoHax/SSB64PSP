//! `mv/mvopening/mvopeningcliff.c`: Link on a dark hill playing the
//! ocarina, the sky scrolling behind him and slowing to a stop.

use alloc::vec;

use super::movie::{
    link, CameraKind, Fighter, Head, Joints, Model, ObjectKind, Persp, View, World, FULL,
};
use super::{Exit, Kind};
use crate::fighter::FighterKind;
use crate::menu::Piece;

/// `llMVOpeningCliffFileID` and its symbols.
pub const FILE: u32 = 0x44;
pub const HILLS: u32 = 0x37A0;
pub const OCARINA: u32 = 0x67A0;
pub const OCARINA_ANIM_JOINT: u32 = 0x6850;
pub const CAM_ANIM_JOINT: u32 = 0x8910;
/// `llMVOpeningStandoffWallpaperFileID` and `...WallpaperSprite`.
pub const FILE_WALLPAPER: u32 = 0x46;
pub const WALLPAPER: u32 = 0xB500;

/// The camera's baked plays (`ssb_rom::opening::camera_slot`).
pub const CAMERA_SLOT: u32 = 0x4000_0000 | (FILE << 20) | CAM_ANIM_JOINT;

/// `nFTDemoStatusSpecialStart` (0x1000F): submotion row 15.
pub const ROW_OPENING1: u8 = 15;

/// The hills' display (`mvOpeningCliffHillsProcDisplay`): no depth test.
pub const HOST_HILLS: u16 = 1;

#[derive(Debug, Clone, PartialEq)]
pub struct Cliff {
    tics: i32,
    /// `sMVOpeningCliffWallpaperScrollSpeed`.
    speed: f32,
    wallpaper_x: f32,
    pub world: World,
    cameras: [u16; 2],
    wallpaper: u16,
    hills: u16,
    fighter: u16,
    ocarina: u16,
}

impl Cliff {
    /// `mvOpeningCliffFuncStart`.
    pub fn new() -> Cliff {
        let mut world = World::new();
        world.camera(
            100,
            0,
            FULL,
            CameraKind::Default {
                fill: None,
                zbuffer: true,
            },
        );
        // `mvOpeningCliffMakeMainCamera`: two cameras on one script, links
        // 26 and 28, perspective `PerspF` and `LookAt`.
        let cam = |world: &mut World, priority, links| {
            world.camera(
                priority,
                links,
                FULL,
                CameraKind::Persp {
                    persp: Persp::animated(CAMERA_SLOT, View::LookAt, 128.0, 16384.0),
                    zbuffer: false,
                },
            )
        };
        let cameras = [cam(&mut world, 80, link(26)), cam(&mut world, 70, link(28))];
        world.camera(90, link(27), FULL, CameraKind::Sprite);
        // `mvOpeningCliffMakeWallpaper`: two copies at twice the size.
        let piece = |x: f32| {
            let mut p = Piece::at(FILE_WALLPAPER, WALLPAPER, x, 0.0);
            p.transparent = false;
            p.scale = [2.0, 2.0];
            p
        };
        let wallpaper = world.object(
            27,
            Head::Zero,
            ObjectKind::Sprites(vec![piece(0.0), piece(320.0)]),
        );
        let hills = world.object(26, Head::Zero, ObjectKind::Host(HOST_HILLS));
        // `mvOpeningCliffMakeFighter`: Link in his opening status on link 28.
        let fighter = world.object(
            28,
            Head::Links,
            ObjectKind::Fighter(Fighter::new(FighterKind::Link, 0, ROW_OPENING1)),
        );
        let ocarina = world.object(
            26,
            Head::Zero,
            ObjectKind::Model(
                Model::new(FILE, OCARINA, Head::Zero)
                    .joints(Joints::table(FILE, OCARINA_ANIM_JOINT)),
            ),
        );
        world.light = [-45.0, 25.0];
        Cliff {
            tics: 0,
            speed: 0.0,
            wallpaper_x: 0.0,
            world,
            cameras,
            wallpaper,
            hills,
            fighter,
            ocarina,
        }
    }

    /// One frame: `mvOpeningCliffFuncRun`, then the processes in creation
    /// order: the cameras', the wallpaper's scroll, the fighter's and the
    /// ocarina's.
    pub fn tick(&mut self, tapped: bool) -> Option<Exit> {
        let mut exit = None;
        self.tics += 1;
        if self.tics >= 10 {
            if tapped {
                exit = Some(Exit::Title);
            }
            if self.tics == 160 {
                exit = Some(Exit::Next(Kind::Yamabuki));
            }
        }
        for id in self.cameras {
            if let Some(p) = self.world.camera_mut(id).and_then(|c| c.persp_mut()) {
                p.play();
            }
        }
        self.scroll_wallpaper();
        if let Some(f) = self.world.fighter_mut(self.fighter) {
            f.plays += 1;
        }
        if let Some(m) = self.world.model_mut(self.ocarina) {
            m.play();
        }
        exit
    }

    /// `mvOpeningCliffWallpaperProcDisplay` (a process despite its name).
    fn scroll_wallpaper(&mut self) {
        let t = self.tics;
        match t {
            1 => self.speed = 15.0,
            80 => self.speed = 10.0,
            90 => self.speed = 6.0,
            120 => self.speed = 2.0,
            180 => self.speed = 0.0,
            _ => {}
        }
        if t > 1 && t < 80 {
            self.speed += -5.0 / 79.0;
        }
        if t > 80 && t < 90 {
            self.speed += -2.0 / 5.0;
        }
        if t > 90 && t < 120 {
            self.speed += -2.0 / 15.0;
        }
        if t > 120 && t < 180 {
            self.speed += -1.0 / 30.0;
        }
        self.wallpaper_x -= self.speed;
        if self.wallpaper_x < -320.0 {
            self.wallpaper_x += 320.0;
        }
        let x = self.wallpaper_x;
        if let Some(s) = self.world.sprites_mut(self.wallpaper) {
            s[0].x = x;
            s[1].x = x + 320.0;
        }
    }

    /// The hills' tree (`llMVOpeningCliffHillsDObjDesc`), drawn by the host
    /// for [`HOST_HILLS`] without depth (`G_RM_AA_OPA_SURF`).
    pub const fn hills() -> Model {
        Model::new(FILE, HILLS, Head::Zero)
    }

    pub fn hills_id(&self) -> u16 {
        self.hills
    }
}

impl Default for Cliff {
    fn default() -> Self {
        Cliff::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sky_slows_to_a_stop_and_the_scene_ends_at_160() {
        let mut c = Cliff::new();
        let mut exit = None;
        for _ in 0..160 {
            exit = c.tick(false);
        }
        assert_eq!(exit, Some(Exit::Next(Kind::Yamabuki)));
        assert!(c.speed > 0.0 && c.speed < 2.0);
        let cam = c.world.cameras.iter().find_map(|c| c.persp()).unwrap();
        assert_eq!(cam.anim.unwrap().plays, 160);
        assert_eq!(CAMERA_SLOT, 0x4000_0000 | (0x44 << 20) | 0x8910);
    }
}
