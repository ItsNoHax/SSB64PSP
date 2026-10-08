//! `mv/mvopening/mvopeningsector.c`: the Great Fox and three Arwings in
//! Sector Z over a scrolling starfield, then Fox's cockpit growing in over
//! them.

use alloc::vec;
use alloc::vec::Vec;

use super::movie::{link, CameraKind, Head, Joints, Model, ObjectKind, Persp, View, World, FULL};
use super::{Exit, Kind};
use crate::menu::Piece;

/// `llMVOpeningSectorFileID` and its symbols.
pub const FILE: u32 = 0x49;
pub const GREAT_FOX: u32 = 0xD820;
pub const GREAT_FOX_ANIM_JOINT: u32 = 0xDA10;
pub const ARWING_ANIM_JOINTS: [u32; 3] = [0xE110, 0xE910, 0xF1C0];
pub const CAM_ANIM_JOINT: u32 = 0xF9A0;
pub const COCKPIT: u32 = 0x3CC90;
/// `llFoxSpecial3FileID` and `llFoxSpecial3EntryArwingDObjDesc`.
pub const FILE_ARWING: u32 = 0xA1;
pub const ARWING: u32 = 0x2C30;
/// `llMVOpeningSectorWallpaperFileID` and its sprite.
pub const FILE_WALLPAPER: u32 = 0x4A;
pub const WALLPAPER: u32 = 0x26C88;

/// The camera's baked plays (`ssb_rom::opening::camera_slot`).
pub const CAMERA_SLOT: u32 = 0x4000_0000 | (FILE << 20) | CAM_ANIM_JOINT;

/// The cockpit's display (`mvOpeningSectorCockpitProcDisplay`), drawn by
/// the host: [`Sector::cockpit`] at [`Sector::cockpit_alpha`] through
/// `G_CC(0, 0, 0, TEXEL0, 0, 0, 0, PRIMITIVE)` and `G_RM_AA_XLU_SURF`.
pub const HOST_COCKPIT: u16 = 1;

/// `mvOpeningSectorMakeCockpit`'s `SObj` and its display's alpha.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Cockpit {
    id: u16,
    scale: f32,
    x: f32,
    y: f32,
    /// `sMVOpeningSectorCockpitAlpha`.
    alpha: i32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Sector {
    tics: i32,
    pub world: World,
    camera: u16,
    wallpaper: u16,
    /// The first `SObj`'s position.
    wallpaper_x: f32,
    wallpaper_y: f32,
    /// `sMVOpeningSectorWallpaperScrollSpeedX` and `...Y`.
    speed_x: f32,
    speed_y: f32,
    /// The Great Fox and the three Arwings, in creation order.
    models: Vec<u16>,
    cockpit: Option<Cockpit>,
}

impl Sector {
    /// `mvOpeningSectorFuncStart`.
    pub fn new() -> Sector {
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
        let camera = world.camera(
            40,
            link(27),
            FULL,
            CameraKind::Persp {
                persp: Persp::animated(CAMERA_SLOT, View::LookAt, 128.0, 30000.0),
                zbuffer: false,
            },
        );
        world.camera(90, link(28), FULL, CameraKind::Sprite);
        world.camera(20, link(29), FULL, CameraKind::Sprite);
        // `mvOpeningSectorMakeWallpaper`: four copies, 300 by 220 apart.
        let piece = |x: f32, y: f32| Piece::at(FILE_WALLPAPER, WALLPAPER, x, y);
        let wallpaper = world.object(
            28,
            Head::Zero,
            ObjectKind::Sprites(vec![
                piece(10.0, 10.0),
                piece(310.0, 10.0),
                piece(10.0, 230.0),
                piece(310.0, 230.0),
            ]),
        );
        let mut models = Vec::with_capacity(4);
        models.push(
            world.object(
                27,
                Head::Links,
                ObjectKind::Model(
                    Model::new(FILE, GREAT_FOX, Head::Links)
                        .joints(Joints::table(FILE, GREAT_FOX_ANIM_JOINT)),
                ),
            ),
        );
        // `mvOpeningSectorMakeArwings`: Fox's entry Arwing, three times.
        for &anim in &ARWING_ANIM_JOINTS {
            models.push(world.object(
                27,
                Head::Links,
                ObjectKind::Model(
                    Model::new(FILE_ARWING, ARWING, Head::Links).joints(Joints::table(FILE, anim)),
                ),
            ));
        }
        world.light = [45.0, 45.0];
        crate::sound::play_fgm(crate::sound::id::nSYAudioFGMOpeningSectorAmbient);
        Sector {
            tics: 0,
            world,
            camera,
            wallpaper,
            wallpaper_x: 10.0,
            wallpaper_y: 10.0,
            speed_x: 0.0,
            speed_y: 0.0,
            models,
            cockpit: None,
        }
    }

    /// One frame: `mvOpeningSectorFuncRun`, then the processes in creation
    /// order: the camera's, the wallpaper's, the Great Fox's, the Arwings'
    /// and the cockpit's; then the cockpit display's fade.
    pub fn tick(&mut self, tapped: bool) -> Option<Exit> {
        let mut exit = None;
        self.tics += 1;
        if self.tics >= 10 {
            if tapped {
                exit = Some(Exit::Title);
            }
            if self.tics == 120 {
                self.make_cockpit();
            }
            if self.tics == 160 {
                exit = Some(Exit::Next(Kind::Standoff));
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
        for &id in &self.models {
            if let Some(m) = self.world.model_mut(id) {
                m.play();
            }
        }
        if let Some(c) = self.cockpit.as_mut() {
            // `mvOpeningSectorCockpitProcUpdate`.
            let mut scale = c.scale;
            scale += 0.025;
            if scale > 1.0 {
                scale = 1.0;
            }
            c.scale = scale;
            c.x = 160.0 - ((320.0 * scale) / 2.0);
            c.y = 120.0 - ((240.0 * scale) / 2.0);
            // `mvOpeningSectorCockpitProcDisplay` raises the alpha once a
            // drawn frame.
            if c.alpha < 0xFF {
                c.alpha += 0x09;
                if c.alpha > 0xFF {
                    c.alpha = 0xFF;
                }
            }
        }
        exit
    }

    /// `mvOpeningSectorWallpaperProcUpdate`. The y speed is kept but never
    /// moves the `SObj`s, as in the source.
    fn scroll_wallpaper(&mut self) {
        let t = self.tics;
        match t {
            1 | 120 => self.speed_x = 6.0,
            140 => self.speed_x = 2.0,
            160 => self.speed_x = 1.0,
            _ => {}
        }
        match t {
            1 | 120 => self.speed_y = 1.0,
            160 => self.speed_y = 3.0,
            _ => {}
        }
        if t > 1 && t < 120 {
            self.speed_x += 0.0;
        }
        if t > 120 && t < 140 {
            self.speed_x += -0.2;
        }
        if t > 140 && t < 160 {
            self.speed_x += -0.05;
        }
        if t > 1 && t < 120 {
            self.speed_y += 0.0;
        }
        if t > 120 && t < 160 {
            self.speed_y += 0.05;
        }
        self.wallpaper_x += self.speed_x;
        if self.wallpaper_x > 10.0 {
            self.wallpaper_x -= 300.0;
        }
        if self.wallpaper_y < -220.0 {
            self.wallpaper_y += 220.0;
        }
        let (x, y) = (self.wallpaper_x, self.wallpaper_y);
        if let Some(s) = self.world.sprites_mut(self.wallpaper) {
            s[0].x = x;
            s[0].y = y;
            s[1].x = x + 300.0;
            s[1].y = y;
            s[2].x = x;
            s[2].y = y + 220.0;
            s[3].x = x + 300.0;
            s[3].y = y + 220.0;
        }
    }

    /// `mvOpeningSectorMakeCockpit`: link 29, a quarter size, transparent.
    fn make_cockpit(&mut self) {
        let id = self
            .world
            .object(29, Head::Zero, ObjectKind::Host(HOST_COCKPIT));
        self.cockpit = Some(Cockpit {
            id,
            scale: 0.25,
            x: 0.0,
            y: 0.0,
            alpha: 0,
        });
    }

    /// The cockpit display's object, once made.
    pub fn cockpit_id(&self) -> Option<u16> {
        self.cockpit.map(|c| c.id)
    }

    /// The cockpit `SObj` (`SP_TRANSPARENT`, `SP_FASTCOPY` cleared) where
    /// its process put it.
    pub fn cockpit(&self) -> Option<Piece> {
        let c = self.cockpit?;
        let mut p = Piece::clear(FILE, COCKPIT, c.x, c.y);
        p.scale = [c.scale, c.scale];
        Some(p)
    }

    /// The cockpit display's primitive alpha.
    pub fn cockpit_alpha(&self) -> u8 {
        self.cockpit.map_or(0, |c| c.alpha as u8)
    }
}

impl Default for Sector {
    fn default() -> Self {
        Sector::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_cockpit_grows_in_from_120_and_the_scene_ends_at_160() {
        let mut s = Sector::new();
        for _ in 0..119 {
            s.tick(false);
        }
        assert!(s.cockpit.is_none());
        s.tick(false);
        let p = s.cockpit().unwrap();
        assert_eq!(p.scale[0], 0.275);
        assert_eq!(s.cockpit_alpha(), 9);
        let mut exit = None;
        for _ in 120..160 {
            exit = s.tick(false);
        }
        assert_eq!(exit, Some(Exit::Next(Kind::Standoff)));
        assert_eq!(s.cockpit().unwrap().scale[0], 1.0);
        assert_eq!(s.cockpit_alpha(), 0xFF);
        assert_eq!((s.cockpit().unwrap().x, s.cockpit().unwrap().y), (0.0, 0.0));
    }

    #[test]
    fn the_stars_scroll_right_and_wrap_every_300() {
        let mut s = Sector::new();
        s.tick(false);
        // 10 + 6 = 16 > 10.
        assert_eq!(s.wallpaper_x, -284.0);
        assert_eq!(s.wallpaper_y, 10.0);
    }

    #[test]
    fn a_tap_before_tic_10_does_nothing() {
        let mut s = Sector::new();
        for _ in 0..9 {
            assert_eq!(s.tick(true), None);
        }
        assert_eq!(s.tick(true), Some(Exit::Title));
    }
}
