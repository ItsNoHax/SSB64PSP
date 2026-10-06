//! `mv/mvopening/mvopeningstandoff.c`: Mario and Kirby facing off on a
//! dark plain, the sky racing past, lightning striking three times.

use alloc::vec;
use alloc::vec::Vec;

use super::movie::{
    link, CameraKind, Fighter, Fill, Head, Joints, Model, ObjectKind, Persp, View, World, FULL,
};
use super::{Exit, Kind};
use crate::fighter::FighterKind;
use crate::menu::Piece;

/// `llMVOpeningStandoffFileID` and its symbols.
pub const FILE: u32 = 0x45;
pub const GROUND_DL: u32 = 0x1C10;
pub const LIGHTNING: u32 = 0x6950;
pub const LIGHTNING_ANIM_JOINT: u32 = 0x6D60;
pub const CAM_ANIM_JOINT: u32 = 0x7250;
/// `llMVOpeningStandoffWallpaperFileID` and its sprite.
pub const FILE_WALLPAPER: u32 = 0x46;
pub const WALLPAPER: u32 = 0xB500;

/// The camera's baked plays (`ssb_rom::opening::camera_slot`).
pub const CAMERA_SLOT: u32 = 0x4000_0000 | (FILE << 20) | CAM_ANIM_JOINT;

/// 0x1000F: submotion row 15.
pub const ROW_OPENING1: u8 = 15;

#[derive(Debug, Clone, PartialEq)]
pub struct Standoff {
    tics: i32,
    pub world: World,
    camera: u16,
    wallpaper: u16,
    /// The first `SObj`'s position and scale.
    wallpaper_x: f32,
    wallpaper_y: f32,
    wallpaper_scale: f32,
    /// `sMVOpeningStandoffWallpaperScrollSpeed`.
    speed: f32,
    fighters: [u16; 2],
    lightning: u16,
    flash: u16,
}

impl Standoff {
    /// `mvOpeningStandoffFuncStart`.
    pub fn new() -> Standoff {
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
        let camera = world.camera(
            80,
            link(26) | link(9),
            FULL,
            CameraKind::Persp {
                persp: Persp::animated(CAMERA_SLOT, View::LookAt, 128.0, 16384.0),
                zbuffer: false,
            },
        );
        world.camera(90, link(27), FULL, CameraKind::Sprite);
        world.camera(20, link(28), FULL, CameraKind::Sprite);
        // `mvOpeningStandoffMakeWallpaper`: two copies at twice the size.
        let piece = |x: f32| {
            let mut p = Piece::at(FILE_WALLPAPER, WALLPAPER, x, 0.0);
            p.scale = [2.0, 2.0];
            p
        };
        let wallpaper = world.object(
            27,
            Head::Zero,
            ObjectKind::Sprites(vec![piece(0.0), piece(320.0)]),
        );
        let mut fighters = [0; 2];
        for (i, kind) in [FighterKind::Mario, FighterKind::Kirby]
            .into_iter()
            .enumerate()
        {
            fighters[i] = world.object(
                9,
                Head::Links,
                ObjectKind::Fighter(Fighter::new(kind, 0, ROW_OPENING1)),
            );
        }
        // `mvOpeningStandoffMakeGround`: one `DObj` on the list, head 0.
        world.object(
            26,
            Head::Zero,
            ObjectKind::Model(Model::new(FILE, GROUND_DL, Head::Zero)),
        );
        // `mvOpeningStandoffMakeLightning`: its materials' scripts switch
        // the bolt's textures in as the flashes come.
        let lightning = world.object(
            26,
            Head::Links,
            ObjectKind::Model(
                Model::new(FILE, LIGHTNING, Head::Links)
                    .joints(Joints::table(FILE, LIGHTNING_ANIM_JOINT))
                    .mats(),
            ),
        );
        // `mvOpeningStandoffMakeLightningFlash`: head 1, link 28.
        let flash = world.object(28, Head::One, ObjectKind::Fills(Vec::new()));
        world.light = [45.0, 45.0];
        let mut s = Standoff {
            tics: 0,
            world,
            camera,
            wallpaper,
            wallpaper_x: 0.0,
            wallpaper_y: 0.0,
            wallpaper_scale: 2.0,
            speed: 0.0,
            fighters,
            lightning,
            flash,
        };
        s.update_flash();
        s
    }

    /// One frame: `mvOpeningStandoffFuncRun`, then the processes in
    /// creation order: the camera's, the wallpaper's, the fighters' and the
    /// lightning's.
    pub fn tick(&mut self, tapped: bool) -> Option<Exit> {
        let mut exit = None;
        self.tics += 1;
        if self.tics >= 10 {
            if tapped {
                exit = Some(Exit::Title);
            }
            if self.tics == 320 {
                exit = Some(Exit::Next(Kind::Clash));
            }
        }
        if let Some(p) = self
            .world
            .camera_mut(self.camera)
            .and_then(|c| c.persp_mut())
        {
            p.play();
        }
        self.scroll_wallpaper();
        for id in self.fighters {
            if let Some(f) = self.world.fighter_mut(id) {
                f.plays += 1;
            }
        }
        if let Some(m) = self.world.model_mut(self.lightning) {
            m.play();
        }
        self.update_flash();
        exit
    }

    /// `mvOpeningStandoffWallpaperProcUpdate`.
    fn scroll_wallpaper(&mut self) {
        let t = self.tics;
        match t {
            1 | 105 | 195 | 280 => self.speed = 2.0,
            90 | 180 => self.speed = 8.0,
            300 => self.speed = 7.0,
            _ => {}
        }
        if (281..300).contains(&t) {
            self.speed += -0.05;
        }
        if (301..320).contains(&t) {
            self.speed += 0.4;
        }
        if t >= 301 {
            self.wallpaper_y += self.speed;
        } else {
            self.wallpaper_x += self.speed;
            if self.wallpaper_x > 320.0 {
                self.wallpaper_x -= 320.0;
            }
        }
        // The second `SObj` copies the first's y before this tic rewrites
        // it to -240 or 0: a tic behind on the zoom's first and last tics.
        let next_x;
        let next_y = self.wallpaper_y;
        if (t > 90 && t < 105) || (t > 180 && t < 195) {
            next_x = self.wallpaper_x - 640.0;
            self.wallpaper_scale = 4.0;
            self.wallpaper_y = -240.0;
        } else {
            next_x = self.wallpaper_x - 320.0;
            self.wallpaper_scale = 2.0;
            if t < 300 {
                self.wallpaper_y = 0.0;
            }
        }
        let (x, y, k) = (self.wallpaper_x, self.wallpaper_y, self.wallpaper_scale);
        if let Some(s) = self.world.sprites_mut(self.wallpaper) {
            s[0].x = x;
            s[0].y = y;
            s[0].scale = [k, k];
            s[1].x = next_x;
            s[1].y = next_y;
            s[1].scale = [k, k];
        }
    }

    /// `mvOpeningStandoffLightningFlashProcDisplay`: white at 0x40 over
    /// the frame in the three flashes, else nothing (alpha 0).
    fn update_flash(&mut self) {
        let t = self.tics;
        let on = (t > 19 && t < 23) || (t > 149 && t < 153) || (t > 260 && t < 264);
        let fills = if on {
            vec![Fill {
                rect: FULL,
                rgba: [0xFF, 0xFF, 0xFF, 0x40],
            }]
        } else {
            Vec::new()
        };
        if let Some(f) = self.world.fills_mut(self.flash) {
            *f = fills;
        }
    }
}

impl Default for Standoff {
    fn default() -> Self {
        Standoff::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flash(s: &mut Standoff) -> usize {
        let id = s.flash;
        s.world.fills_mut(id).unwrap().len()
    }

    #[test]
    fn lightning_flashes_three_times_and_the_scene_ends_at_320() {
        let mut s = Standoff::new();
        let mut lit = vec![];
        let mut exit = None;
        for _ in 0..320 {
            exit = s.tick(false);
            if flash(&mut s) > 0 {
                lit.push(s.tics);
            }
        }
        assert_eq!(lit, [20, 21, 22, 150, 151, 152, 261, 262, 263]);
        assert_eq!(exit, Some(Exit::Next(Kind::Clash)));
        let lightning = s.lightning;
        assert_eq!(s.world.model_mut(lightning).unwrap().mats, Some(320));
    }

    #[test]
    fn the_sky_zooms_in_the_rushes_and_rises_at_the_end() {
        let mut s = Standoff::new();
        for _ in 0..91 {
            s.tick(false);
        }
        assert_eq!(s.wallpaper_scale, 4.0);
        assert_eq!(s.wallpaper_y, -240.0);
        for _ in 91..105 {
            s.tick(false);
        }
        assert_eq!((s.wallpaper_scale, s.wallpaper_y), (2.0, 0.0));
        for _ in 105..301 {
            s.tick(false);
        }
        // Tic 301: 7 + 0.4 added to y.
        assert!((s.wallpaper_y - 7.4).abs() < 1e-5);
    }

    #[test]
    fn a_tap_before_tic_10_does_nothing() {
        let mut s = Standoff::new();
        for _ in 0..9 {
            assert_eq!(s.tick(true), None);
        }
        assert_eq!(s.tick(true), Some(Exit::Title));
    }
}
