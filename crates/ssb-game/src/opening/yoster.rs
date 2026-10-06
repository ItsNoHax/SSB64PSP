//! `mv/mvopening/mvopeningyoster.c`: four Yoshis in their nest on Yoshi's
//! Island.

use alloc::vec;

use super::movie::{
    link, CameraKind, Fighter, Head, Joints, Model, ObjectKind, Persp, View, World, FULL,
};
use super::{Exit, Kind};
use crate::fighter::FighterKind;
use crate::menu::Piece;

/// `llMVOpeningYosterFileID` and its symbols.
pub const FILE: u32 = 0x43;
pub const NEST: u32 = 0x9808;
pub const GROUND: u32 = 0xB990;
pub const GROUND_ANIM_JOINT: u32 = 0xBF70;
pub const CAM_ANIM_JOINT: u32 = 0xC940;
/// `llStageYoshiFileID` and `llStageYoshiSprite`.
pub const FILE_WALLPAPER: u32 = 0x5D;
pub const WALLPAPER: u32 = 0x26C88;

/// The camera's baked plays (`ssb_rom::opening::camera_slot`).
pub const CAMERA_SLOT: u32 = 0x4000_0000 | (FILE << 20) | CAM_ANIM_JOINT;

/// `mvOpeningYosterMakeFighters`' `status_ids` (0x1000F to 0x10012) as
/// submotion rows, one per Yoshi costume 0 to 3.
pub const ROWS: [u8; 4] = [15, 16, 17, 18];

#[derive(Debug, Clone, PartialEq)]
pub struct Yoster {
    tics: i32,
    pub world: World,
    camera: u16,
    ground: u16,
    fighters: [u16; 4],
}

impl Yoster {
    /// `mvOpeningYosterFuncStart`.
    pub fn new() -> Yoster {
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
        // `mvOpeningYosterMakeMainCamera`: its animation played once at
        // creation (`gcPlayCamAnim`) as well as by its process.
        let mut persp = Persp::animated(CAMERA_SLOT, View::LookAt, 128.0, 16384.0);
        persp.play();
        let camera = world.camera(
            80,
            link(18) | link(15) | link(10) | link(9) | link(6),
            FULL,
            CameraKind::Persp {
                persp,
                zbuffer: false,
            },
        );
        world.camera(90, link(28), FULL, CameraKind::Sprite);
        world.object(
            28,
            Head::Zero,
            ObjectKind::Sprites(vec![Piece::at(FILE_WALLPAPER, WALLPAPER, 10.0, 10.0)]),
        );
        world.object(
            6,
            Head::Links,
            ObjectKind::Model(Model::new(FILE, NEST, Head::Links)),
        );
        let ground = world.object(
            6,
            Head::Links,
            ObjectKind::Model(
                Model::new(FILE, GROUND, Head::Links)
                    .joints(Joints::table(FILE, GROUND_ANIM_JOINT)),
            ),
        );
        let mut fighters = [0; 4];
        for (i, &row) in ROWS.iter().enumerate() {
            fighters[i] = world.object(
                9,
                Head::Links,
                ObjectKind::Fighter(Fighter::new(FighterKind::Yoshi, i, row)),
            );
        }
        world.light = [45.0, 45.0];
        Yoster {
            tics: 0,
            world,
            camera,
            ground,
            fighters,
        }
    }

    /// One frame: `mvOpeningYosterMainProc`, then the processes in
    /// creation order: the camera's, the ground's and the fighters'.
    pub fn tick(&mut self, tapped: bool) -> Option<Exit> {
        let mut exit = None;
        self.tics += 1;
        if self.tics >= 10 {
            if tapped {
                exit = Some(Exit::Title);
            }
            if self.tics == 160 {
                exit = Some(Exit::Next(Kind::Sector));
            }
        }
        if let Some(p) = self
            .world
            .camera_mut(self.camera)
            .and_then(|c| c.persp_mut())
        {
            p.play();
        }
        if let Some(m) = self.world.model_mut(self.ground) {
            m.play();
        }
        for id in self.fighters {
            if let Some(f) = self.world.fighter_mut(id) {
                f.plays += 1;
            }
        }
        exit
    }
}

impl Default for Yoster {
    fn default() -> Self {
        Yoster::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_camera_leads_by_its_creation_play_and_the_scene_ends_at_160() {
        let mut y = Yoster::new();
        let mut exit = None;
        for _ in 0..160 {
            exit = y.tick(false);
        }
        assert_eq!(exit, Some(Exit::Next(Kind::Sector)));
        let cam = y.world.cameras.iter().find_map(|c| c.persp()).unwrap();
        assert_eq!(cam.anim.unwrap().plays, 161);
        let ids = y.fighters;
        let rows: alloc::vec::Vec<u8> = ids
            .iter()
            .map(|&id| y.world.fighter_mut(id).unwrap().row)
            .collect();
        assert_eq!(rows, ROWS);
    }

    #[test]
    fn a_tap_before_tic_10_does_nothing() {
        let mut y = Yoster::new();
        for _ in 0..9 {
            assert_eq!(y.tick(true), None);
        }
        assert_eq!(y.tick(true), Some(Exit::Title));
    }
}
