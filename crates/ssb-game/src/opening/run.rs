//! `mv/mvopening/mvopeningrun.c`: the eight fighters running across the
//! screen before a scrolling sky, Link jumping at tic 45 and the crash at
//! 190.

use alloc::vec;

use super::movie::{
    link, CameraKind, Fighter, Head, Joints, Model, ObjectKind, Persp, View, World, FULL,
};
use super::{Exit, Kind};
use crate::fighter::FighterKind;
use crate::menu::Piece;
use ssb_engine::math::Vec3;

/// `llMVOpeningRunFileID` and its symbols.
pub const FILE: u32 = 0x37;
pub const WALLPAPER: u32 = 0x58A0;
/// The fighters' proxy `AnimJoint`s, in [`KINDS`] order.
pub const FIGHTER_ANIM_JOINTS: [u32; 8] = [0x4, 0xB4, 0x124, 0x184, 0x224, 0x334, 0x3A4, 0x484];
/// `llMVOpeningRunMainFileID`: its camera animation is at the file's start.
pub const FILE_MAIN: u32 = 0x3C;
/// `llMVOpeningRunCrashFileID` and `llMVOpeningRunCrashDObjDesc`.
pub const FILE_CRASH: u32 = 0x4B;
pub const CRASH: u32 = 0x35F8;

/// The camera's baked plays (`ssb_rom::opening::camera_slot`).
pub const CAMERA_SLOT: u32 = 0x4000_0000 | (FILE_MAIN << 20);

/// `mvOpeningRunMakeFighters`' `fkinds`.
pub const KINDS: [FighterKind; 8] = [
    FighterKind::Mario,
    FighterKind::Fox,
    FighterKind::Donkey,
    FighterKind::Samus,
    FighterKind::Link,
    FighterKind::Yoshi,
    FighterKind::Kirby,
    FighterKind::Pikachu,
];

/// `nFTDemoStatusRun` (0x10006) and 0x10007: submotion rows 6 and 7.
pub const ROW_RUN: u8 = 6;
pub const ROW_JUMP: u8 = 7;

#[derive(Debug, Clone, PartialEq)]
pub struct Run {
    tics: i32,
    pub world: World,
    camera: u16,
    wallpaper: u16,
    /// The left `SObj`'s x.
    wallpaper_x: f32,
    fighters: [u16; 8],
    /// `sMVOpeningRunLinkFighterGObj`.
    link_fighter: u16,
    crash: Option<u16>,
}

impl Run {
    /// `mvOpeningRunFuncStart`.
    pub fn new() -> Run {
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
        // `mvOpeningRunMakeMainCamera`: `PerspFastF` and `LookAt` at their
        // defaults, the scene's camera animation.
        let camera = world.camera(
            60,
            link(18) | link(15) | link(10) | link(9) | link(6),
            FULL,
            CameraKind::Persp {
                persp: Persp::animated(CAMERA_SLOT, View::LookAt, 100.0, 12800.0),
                zbuffer: false,
            },
        );
        world.camera(80, link(28), FULL, CameraKind::Sprite);
        // `mvOpeningRunMakeWallpaper`: two copies at twice the size.
        let piece = |x: f32| {
            let mut p = Piece::at(FILE, WALLPAPER, x, 0.0);
            p.scale = [2.0, 2.0];
            p
        };
        let wallpaper = world.object(
            28,
            Head::Zero,
            ObjectKind::Sprites(vec![piece(-320.0), piece(0.0)]),
        );
        // `mvOpeningRunMakeFighters`: each fighter running at the origin,
        // its root following a proxy `DObj`'s animation, which plays once
        // here (`gcPlayAnimAll`) and every tic after.
        let mut fighters = [0; 8];
        let mut link_fighter = 0;
        for (i, &kind) in KINDS.iter().enumerate() {
            let mut f = Fighter::new(kind, 0, ROW_RUN);
            // `ftManagerMakeFighter` scales the root by `attr->size`, which
            // this scene, unlike the later ones, leaves.
            let size = crate::motion::combat_attrs(kind).map_or(1.0, |a| a.size);
            f.scale = Vec3::new(size, size, size);
            let mut proxy = Joints::root(FILE, FIGHTER_ANIM_JOINTS[i]);
            proxy.plays = 1;
            f.proxy = Some(proxy);
            fighters[i] = world.object(9, Head::Links, ObjectKind::Fighter(f));
            if kind == FighterKind::Link {
                link_fighter = fighters[i];
            }
        }
        world.light = [45.0, 10.0];
        Run {
            tics: 0,
            world,
            camera,
            wallpaper,
            wallpaper_x: -320.0,
            fighters,
            link_fighter,
            crash: None,
        }
    }

    /// One frame: `mvOpeningRunFuncRun`, then the processes in creation
    /// order: the camera's, the wallpaper's, each fighter's and its
    /// proxy's (`mvOpeningRunFighterProcUpdate`), the crash's.
    pub fn tick(&mut self, tapped: bool) -> Option<Exit> {
        let mut exit = None;
        self.tics += 1;
        if self.tics >= 10 {
            if tapped {
                exit = Some(Exit::Title);
            }
            if self.tics == 45 {
                if let Some(f) = self.world.fighter_mut(self.link_fighter) {
                    f.set_status(ROW_JUMP);
                }
            }
            if self.tics == 190 {
                crate::sound::play_fgm(crate::sound::id::nSYAudioFGMExplodeL);
                self.make_crash();
            }
            if self.tics == 220 {
                exit = Some(Exit::Next(Kind::Cliff));
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
                if let Some(j) = f.proxy.as_mut() {
                    j.plays += 1;
                }
            }
        }
        if let Some(m) = self.crash.and_then(|id| self.world.model_mut(id)) {
            m.play();
        }
        exit
    }

    /// `mvOpeningRunWallpaperProcUpdate`.
    fn scroll_wallpaper(&mut self) {
        self.wallpaper_x += 30.0;
        if self.wallpaper_x > 0.0 {
            self.wallpaper_x += -320.0;
        }
        let x = self.wallpaper_x;
        if let Some(s) = self.world.sprites_mut(self.wallpaper) {
            s[0].x = x;
            s[1].x = x + 320.0;
        }
    }

    /// `mvOpeningRunMakeCrash`: on link 6, its materials played once here
    /// and by its process from this tic.
    fn make_crash(&mut self) {
        let mut m = Model::new(FILE_CRASH, CRASH, Head::Zero).mats();
        m.translate = Some(Vec3::new(960.0, 360.0, -13.5));
        m.rotate = Some(Vec3::new(0.0, 90.0 * (core::f32::consts::PI / 180.0), 0.0));
        m.scale = Some(Vec3::new(0.9, 0.9, 0.9));
        m.play();
        self.crash = Some(self.world.object(6, Head::Zero, ObjectKind::Model(m)));
    }
}

impl Default for Run {
    fn default() -> Self {
        Run::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn link_jumps_at_45_the_crash_lands_at_190_and_the_scene_ends_at_220() {
        let mut r = Run::new();
        let mut exit = None;
        for _ in 0..44 {
            r.tick(false);
        }
        let link = r.link_fighter;
        assert_eq!(r.world.fighter_mut(link).unwrap().row, ROW_RUN);
        r.tick(false);
        let f = *r.world.fighter_mut(link).unwrap();
        assert_eq!((f.row, f.plays), (ROW_JUMP, 1));
        // The proxies played once at creation and once a tic since.
        let mario = *r.world.fighter_mut(r.fighters[0]).unwrap();
        assert_eq!(mario.proxy.unwrap().plays, 46);
        for _ in 45..190 {
            r.tick(false);
        }
        let crash = r.crash.expect("the crash is made at 190");
        assert_eq!(r.world.model_mut(crash).unwrap().mats, Some(2));
        for _ in 190..220 {
            exit = r.tick(false);
        }
        assert_eq!(exit, Some(Exit::Next(Kind::Cliff)));
        assert_eq!(CAMERA_SLOT, 0x4000_0000 | (0x3C << 20));
    }

    #[test]
    fn a_tap_before_tic_10_does_nothing() {
        let mut r = Run::new();
        for _ in 0..9 {
            assert_eq!(r.tick(true), None);
        }
        assert_eq!(r.tick(true), Some(Exit::Title));
    }

    #[test]
    fn the_wallpaper_wraps_every_320() {
        let mut r = Run::new();
        r.tick(false);
        assert_eq!(r.wallpaper_x, -290.0);
        for _ in 1..11 {
            r.tick(false);
        }
        // -290 + 300 = 10 > 0 wraps to -310.
        assert_eq!(r.wallpaper_x, -310.0);
    }
}
