//! `mv/mvopening/mvopeningyamabuki.c`: Pikachu in Saffron City, a
//! monster's legs stomping past and a Master Ball.

use alloc::vec;

use super::movie::{
    link, CameraKind, Fighter, Head, Joints, Model, ObjectKind, Persp, View, World, FULL,
};
use super::{Exit, Kind};
use crate::fighter::FighterKind;
use crate::menu::Piece;

/// `llMVOpeningYamabukiFileID` and its symbols.
pub const FILE: u32 = 0x47;
pub const LEGS: u32 = 0x9548;
pub const LEGS_ANIM_JOINT: u32 = 0x98C0;
pub const LEGS_SHADOW: u32 = 0xB2B0;
pub const LEGS_SHADOW_ANIM_JOINT: u32 = 0xB390;
pub const MBALL: u32 = 0xC9E0;
pub const MBALL_ANIM_JOINT: u32 = 0xCAC0;
pub const CAM_ANIM_JOINT: u32 = 0xD330;
pub const WALLPAPER: u32 = 0x3EE58;

/// The camera's baked plays (`ssb_rom::opening::camera_slot`).
pub const CAMERA_SLOT: u32 = 0x4000_0000 | (FILE << 20) | CAM_ANIM_JOINT;

/// 0x1000F: submotion row 15.
pub const ROW_OPENING1: u8 = 15;

#[derive(Debug, Clone, PartialEq)]
pub struct Yamabuki {
    tics: i32,
    pub world: World,
    camera: u16,
    fighter: u16,
    /// The legs, their shadow and the Master Ball, in creation order.
    models: [u16; 3],
}

impl Yamabuki {
    /// `mvOpeningYamabukiFuncStart`.
    pub fn new() -> Yamabuki {
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
            link(27) | link(9),
            FULL,
            CameraKind::Persp {
                persp: Persp::animated(CAMERA_SLOT, View::LookAt, 128.0, 16384.0),
                zbuffer: false,
            },
        );
        world.camera(90, link(28), FULL, CameraKind::Sprite);
        // `mvOpeningYamabukiMakeWallpaper`: on link 28
        // (`nGCMatrixKindTraRotRpyRSca`'s value).
        world.object(
            28,
            Head::Zero,
            ObjectKind::Sprites(vec![Piece::at(FILE, WALLPAPER, 0.0, 0.0)]),
        );
        let fighter = world.object(
            9,
            Head::Links,
            ObjectKind::Fighter(Fighter::new(FighterKind::Pikachu, 0, ROW_OPENING1)),
        );
        let legs = world.object(
            27,
            Head::Zero,
            ObjectKind::Model(
                Model::new(FILE, LEGS, Head::Zero).joints(Joints::table(FILE, LEGS_ANIM_JOINT)),
            ),
        );
        let shadow = world.object(
            27,
            Head::Links,
            ObjectKind::Model(
                Model::new(FILE, LEGS_SHADOW, Head::Links)
                    .joints(Joints::table(FILE, LEGS_SHADOW_ANIM_JOINT)),
            ),
        );
        let mball = world.object(
            27,
            Head::Links,
            ObjectKind::Model(
                Model::new(FILE, MBALL, Head::Links).joints(Joints::table(FILE, MBALL_ANIM_JOINT)),
            ),
        );
        world.light = [45.0, 45.0];
        Yamabuki {
            tics: 0,
            world,
            camera,
            fighter,
            models: [legs, shadow, mball],
        }
    }

    /// One frame: `mvOpeningYamabukiFuncRun`, then the processes in
    /// creation order: the camera's, the fighter's, the legs', their
    /// shadow's and the ball's.
    pub fn tick(&mut self, tapped: bool) -> Option<Exit> {
        let mut exit = None;
        self.tics += 1;
        if self.tics >= 10 {
            if tapped {
                exit = Some(Exit::Title);
            }
            if self.tics == 160 {
                exit = Some(Exit::Next(Kind::Jungle));
            }
        }
        if let Some(p) = self
            .world
            .camera_mut(self.camera)
            .and_then(|c| c.persp_mut())
        {
            p.play();
        }
        if let Some(f) = self.world.fighter_mut(self.fighter) {
            f.plays += 1;
        }
        for id in self.models {
            if let Some(m) = self.world.model_mut(id) {
                m.play();
            }
        }
        exit
    }
}

impl Default for Yamabuki {
    fn default() -> Self {
        Yamabuki::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_scene_ends_at_160_with_everything_played_each_tic() {
        let mut y = Yamabuki::new();
        let mut exit = None;
        for _ in 0..160 {
            exit = y.tick(false);
        }
        assert_eq!(exit, Some(Exit::Next(Kind::Jungle)));
        let legs = y.world.model_mut(y.models[0]).unwrap();
        assert_eq!(legs.joints.unwrap().plays, 160);
        let cam = y.world.cameras.iter().find_map(|c| c.persp()).unwrap();
        assert_eq!(cam.anim.unwrap().plays, 160);
        assert_eq!(CAMERA_SLOT, 0x4000_0000 | (0x47 << 20) | 0xD330);
    }

    #[test]
    fn a_tap_before_tic_10_does_nothing() {
        let mut y = Yamabuki::new();
        for _ in 0..9 {
            assert_eq!(y.tick(true), None);
        }
        assert_eq!(y.tick(true), Some(Exit::Title));
    }
}
