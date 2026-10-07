//! `mv/mvopening/mvopeningroom.c`: the room. The Nintendo and HAL page
//! peels away from a child's room, Master Hand plucks a figure from the
//! desk and drops it, the light falls on it as a second figure drops
//! beside it, the scene closes in, and a transition opens on the figure
//! standing up as a fighter. It resets the music's tic count and loads the
//! portraits at 22 seconds.
//!
//! The figures are random: the pulled one any of the eight starters, the
//! dropped one any other (`syUtilsRandTimeUCharRange`).

// The source's own float literals, kept digit for digit.
#![allow(clippy::excessive_precision)]

use alloc::vec;

use super::movie::{
    link, CameraKind, Fighter, Fill, Head, Joints, Model, ObjectKind, Persp, View, World, FULL,
};
use super::{Exit, Kind};
use crate::fighter::FighterKind;
use crate::menu::Piece;
use ssb_engine::math::Vec3;

/// `llMVCommonFileID` and `llMVOpeningRoomTransitionFileID`.
pub const FILE_COMMON: u32 = 0x34;
pub const FILE_TRANSITION: u32 = 0x3F;
/// `llMVOpeningRoomWallpaperFileID` and its sprite.
pub const FILE_WALLPAPER: u32 = 0x5A;
pub const WALLPAPER: u32 = 0x26C88;

/// `llMVCommonRoom*` (file 0x34), as `ssb_rom::opening::room`.
pub mod at {
    pub const BACKGROUND: u32 = 0x7E98;
    pub const DESK: u32 = 0x8DF8;
    pub const HAZE_DL: u32 = 0x98F8;
    pub const BOOKS: u32 = 0xA6F8;
    pub const BOOKS_ANIM_JOINT: u32 = 0xA7B0;
    pub const PENCILS: u32 = 0xAEB8;
    pub const PENCILS_ANIM_JOINT: u32 = 0xAF70;
    pub const LAMP: u32 = 0xBDC0;
    pub const LAMP_ANIM_JOINT: u32 = 0xBEA0;
    pub const TISSUES_DL: u32 = 0xC690;
    pub const TISSUES_ANIM_JOINT: u32 = 0xC884;
    pub const LOGO: u32 = 0x1C4A8;
    pub const SNAP: u32 = 0x1CA68;
    pub const SNAP_ANIM_JOINT: u32 = 0x1CAF0;
    pub const CLOSE_UP_AIR: u32 = 0x1DF28;
    pub const CLOSE_UP_AIR_ANIM_JOINT: u32 = 0x1E010;
    pub const CLOSE_UP_GROUND: u32 = 0x1F270;
    pub const CLOSE_UP_GROUND_ANIM_JOINT: u32 = 0x1F330;
    pub const BOSS_SHADOW_DL: u32 = 0x1F790;
    pub const BOSS_SHADOW_ANIM_JOINT: u32 = 0x1F924;
    pub const DESK_GROUND: u32 = 0x22440;
    pub const SPOTLIGHT_DL: u32 = 0x22E18;
    pub const OUTSIDE_DL: u32 = 0x24200;
    pub const SUNLIGHT_DL: u32 = 0x24708;
    /// File 0x3F's transition lists and scripts.
    pub const TRANSITION_OVERLAY_DL: u32 = 0x5A0;
    pub const TRANSITION_OVERLAY_ANIM_JOINT: u32 = 0x714;
    pub const TRANSITION_OUTLINE_DL: u32 = 0xF40;
    pub const TRANSITION_OUTLINE_ANIM_JOINT: u32 = 0x11C4;
}

/// The four scene cameras' baked plays (`ssb_rom::opening::ROOM_CAMERAS`).
pub const fn camera_slot(file: u32) -> u32 {
    0x4000_0000 | (file << 20)
}
pub const SCENE_CAMERA_FILES: [u32; 4] = [0x38, 0x39, 0x3A, 0x3B];

/// `I_SEC_TO_TICS`.
const fn secs(s: i32) -> i32 {
    s * 60
}

/// Submotion rows: `nFTDemoStatusFigurePulled`, `...FigureDropped`,
/// `...FigureStand`; Master Hand's opening statuses 0x1000F to 0x10011.
pub const ROW_PULLED: u8 = 8;
pub const ROW_DROPPED: u8 = 9;
pub const ROW_STAND: u8 = 10;
pub const ROW_BOSS: [u8; 3] = [15, 16, 17];

/// `mvOpeningRoomGetPulledFighterKind`'s eight starters.
pub const FIGURES: [FighterKind; 8] = [
    FighterKind::Mario,
    FighterKind::Fox,
    FighterKind::Donkey,
    FighterKind::Samus,
    FighterKind::Link,
    FighterKind::Yoshi,
    FighterKind::Kirby,
    FighterKind::Pikachu,
];

/// `mvOpeningRoomSetSpotlightPosition`'s translations (times 30) and
/// scales by `FTKind`.
const SPOTLIGHT_TRANSLATES: [[f32; 3]; 12] = [
    [-38.310, 74.904, -122.733],
    [-38.870, 74.904, -121.776],
    [-38.870, 74.904, -119.480],
    [-38.870, 74.904, -119.480],
    [0.0, 0.0, 0.0],
    [-37.040, 74.904, -119.600],
    [-39.390, 74.904, -118.380],
    [0.0, 0.0, 0.0],
    [-38.310, 74.904, -122.733],
    [-39.390, 74.904, -118.380],
    [0.0, 0.0, 0.0],
    [0.0, 0.0, 0.0],
];
const SPOTLIGHT_SCALES: [[f32; 3]; 12] = [
    [1.00, 1.00, 1.00],
    [1.10, 1.00, 1.10],
    [1.30, 1.00, 1.30],
    [1.30, 1.00, 1.30],
    [0.00, 0.00, 0.00],
    [1.00, 1.00, 1.00],
    [1.20, 1.00, 1.20],
    [0.00, 0.00, 0.00],
    [1.17, 1.00, 1.17],
    [1.20, 1.00, 1.20],
    [0.00, 0.00, 0.00],
    [0.00, 0.00, 0.00],
];

/// The host's displays.
pub const HOST_TRANSITION_OUTLINE: u16 = 1;
pub const HOST_TRANSITION_OVERLAY: u16 = 2;
pub const HOST_WALLPAPER: u16 = 3;

/// `syUtilsRandTimeUCharRange(range)` from a time byte.
fn time_range(byte: u8, range: i32) -> i32 {
    ((f32::from(byte) * range as f32) / 256.0) as i32
}

/// The room's GObjs it ejects by name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct Ids {
    main_camera: u16,
    fighter_camera: u16,
    default_camera: u16,
    background: u16,
    sunlight: u16,
    outside: u16,
    haze: u16,
    books: u16,
    pencils: u16,
    lamp: u16,
    tissues: u16,
    boss: u16,
    boss_shadow: u16,
    logo: u16,
    overlay: u16,
    spotlight: u16,
    pulled: u16,
    dropped: u16,
    snap: u16,
    desk_ground: u16,
    outline: u16,
    transition_overlay: u16,
    close_up_air: u16,
    close_up_ground: u16,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Room {
    tics: i32,
    pub pulled_kind: FighterKind,
    pub dropped_kind: FighterKind,
    /// `sMVOpeningRoomOverlayAlpha`: the logo page's black, then the
    /// close-up's.
    overlay_alpha: i32,
    overlay_is_close_up: bool,
    pub world: World,
    ids: Ids,
}

/// A scene camera's `Persp`.
fn scene_persp(scene: usize, view: View, near: f32, far: f32) -> Persp {
    Persp::animated(camera_slot(SCENE_CAMERA_FILES[scene]), view, near, far)
}

impl Room {
    /// `mvOpeningRoomFuncStart`. `time` is `osGetTime`'s low byte, read
    /// once per draw (`syUtilsRandTimeUCharRange`).
    pub fn new(time: &mut impl FnMut() -> u8) -> Room {
        let (pulled_kind, dropped_kind) = Room::pick_figures(time);
        Room::with_figures(pulled_kind, dropped_kind)
    }

    /// `mvOpeningRoomInitVars`' random figures: the pulled one, then a
    /// different dropped one.
    pub fn pick_figures(time: &mut impl FnMut() -> u8) -> (FighterKind, FighterKind) {
        let pulled_kind = FIGURES[time_range(time(), 8) as usize];
        let dropped_kind = loop {
            let k = FIGURES[time_range(time(), 8) as usize];
            if k != pulled_kind {
                break k;
            }
        };
        (pulled_kind, dropped_kind)
    }

    /// `mvOpeningRoomFuncStart` with the figures [`Room::pick_figures`]
    /// gave. The host picks them before the scene starts, to read their
    /// files ahead (RE-476).
    pub fn with_figures(pulled_kind: FighterKind, dropped_kind: FighterKind) -> Room {
        let mut world = World::new();
        let ids = Ids {
            default_camera: world.camera(
                100,
                0,
                [0.0, 0.0, 320.0, 240.0],
                CameraKind::Default {
                    fill: Some([0, 0, 0, 0xFF]),
                    zbuffer: true,
                },
            ),
            ..Ids::default()
        };
        let mut room = Room {
            tics: 0,
            pulled_kind,
            dropped_kind,
            overlay_alpha: 0,
            overlay_is_close_up: false,
            world,
            ids,
        };
        room.make_scene_cameras(0);
        let w = &mut room.world;
        // `mvOpeningRoomMakeCloseUpOverlayCamera`, `...WallpaperCamera`.
        w.camera(60, link(26), FULL, CameraKind::Sprite);
        w.camera(90, link(28), FULL, CameraKind::Sprite);
        // `mvOpeningRoomMakeLogoCamera`: the page, under scene 1's script.
        w.camera(
            50,
            link(29),
            FULL,
            CameraKind::Persp {
                persp: scene_persp(0, View::LookAt, 100.0, 12800.0),
                zbuffer: true,
            },
        );
        let model = |graph, head| Model::new(FILE_COMMON, graph, head);
        let mut ids = room.ids;
        ids.outside = w.object(
            6,
            Head::Links,
            ObjectKind::Model(model(at::OUTSIDE_DL, Head::Links)),
        );
        ids.haze = w.object(
            6,
            Head::Links,
            ObjectKind::Model(model(at::HAZE_DL, Head::Links)),
        );
        // `mvOpeningRoomMakeBackground`: its material scripts played once.
        let mut bg = model(at::BACKGROUND, Head::Links).mats();
        bg.play();
        ids.background = w.object(6, Head::Links, ObjectKind::Model(bg));
        ids.sunlight = w.object(
            6,
            Head::Links,
            ObjectKind::Model(model(at::SUNLIGHT_DL, Head::Links)),
        );
        w.object(
            6,
            Head::Zero,
            ObjectKind::Model(model(at::DESK, Head::Zero)),
        );
        // `mvOpeningRoomMakeLogoWallpaper`: black over the page, on head 1.
        room.overlay_alpha = 0xFF;
        ids.overlay = w.object(26, Head::One, ObjectKind::Fills(vec![]));
        let mut logo = model(at::LOGO, Head::Links).mats();
        logo.play();
        ids.logo = w.object(29, Head::Links, ObjectKind::Model(logo));
        let played = |graph, script| {
            let mut m = model(graph, Head::Zero).joints(Joints::table(FILE_COMMON, script));
            m.play();
            m
        };
        ids.books = w.object(
            6,
            Head::Zero,
            ObjectKind::Model(played(at::BOOKS, at::BOOKS_ANIM_JOINT)),
        );
        ids.lamp = w.object(
            6,
            Head::Zero,
            ObjectKind::Model(played(at::LAMP, at::LAMP_ANIM_JOINT)),
        );
        let mut tissues = model(at::TISSUES_DL, Head::Zero)
            .joints(Joints::root(FILE_COMMON, at::TISSUES_ANIM_JOINT));
        tissues.play();
        ids.tissues = w.object(6, Head::Zero, ObjectKind::Model(tissues));
        // `mvOpeningRoomMakeBoss`: Master Hand in his first opening status.
        ids.boss = w.object(
            9,
            Head::Links,
            ObjectKind::Fighter(Fighter::new(FighterKind::Boss, 0, ROW_BOSS[0])),
        );
        ids.boss_shadow = w.object(
            9,
            Head::One,
            ObjectKind::Model(
                model(at::BOSS_SHADOW_DL, Head::One)
                    .joints(Joints::root(FILE_COMMON, at::BOSS_SHADOW_ANIM_JOINT)),
            ),
        );
        w.light = [45.0, 45.0];
        room.ids = ids;
        room.update_overlay_display();
        room
    }

    /// `mvOpeningRoomMakeScene{1,2,3,4}Cameras`.
    fn make_scene_cameras(&mut self, scene: usize) {
        let (main_view, fighter_links, near, far) = match scene {
            // Scene 1: `PerspFastF` and the reflecting roll (14); near 80.
            0 => (View::Roll, link(27) | link(9), 80.0, 15000.0),
            1 => (View::Roll, link(27) | link(9), 100.0, 12800.0),
            2 => (View::Roll, link(27) | link(9), 128.0, 16384.0),
            // Scene 4's main camera takes the default look-at; its fighter
            // camera draws link 9 only.
            _ => (View::LookAt, link(9), 128.0, 16384.0),
        };
        let main = scene_persp(scene, main_view, near, far);
        let fighter = scene_persp(scene, View::LookAt, near, far);
        self.ids.main_camera = self.world.camera(
            80,
            link(6),
            FULL,
            CameraKind::Persp {
                persp: main,
                zbuffer: false,
            },
        );
        // `argD` TRUE in scene 4's main camera only fills no colour: none
        // of these clears.
        self.ids.fighter_camera = self.world.camera(
            40,
            fighter_links,
            FULL,
            CameraKind::Persp {
                persp: fighter,
                zbuffer: false,
            },
        );
    }

    fn eject_cameras(&mut self) {
        self.world.eject(self.ids.main_camera);
        self.world.eject(self.ids.fighter_camera);
    }

    /// One frame: `mvOpeningRoomFuncRun`, then the processes.
    pub fn tick(&mut self, tapped: bool) -> Option<Exit> {
        let mut exit = None;
        self.tics += 1;
        let t = self.tics;
        if t >= 10 {
            if tapped {
                exit = Some(Exit::Title);
            }
            if t == 280 {
                self.make_pulled_fighter();
                let mut pencils = Model::new(FILE_COMMON, at::PENCILS, Head::Zero)
                    .joints(Joints::table(FILE_COMMON, at::PENCILS_ANIM_JOINT));
                pencils.play();
                self.ids.pencils = self.world.object(6, Head::Zero, ObjectKind::Model(pencils));
                self.world.eject(self.ids.logo);
                self.world.eject(self.ids.overlay);
                self.world.eject(self.ids.boss_shadow);
            }
            if t == 695 {
                let mut f = Fighter::new(self.dropped_kind, 0, ROW_DROPPED);
                f.translate = Vec3::new(872.3249512, 4038.864014, -4734.600098);
                self.ids.dropped = self.world.object(6, Head::Links, ObjectKind::Fighter(f));
            }
            if t == 380 {
                if let Some(f) = self.world.fighter_mut(self.ids.pulled) {
                    f.set_status(ROW_DROPPED);
                    f.attach = None;
                    f.rotate = Vec3::ZERO;
                }
            }
            if t == 450 {
                // `mvOpeningRoomMakeCloseUpOverlay`.
                self.overlay_alpha = 0;
                self.overlay_is_close_up = true;
                self.ids.overlay = self.world.object(26, Head::Zero, ObjectKind::Fills(vec![]));
                self.world.eject(self.ids.sunlight);
            }
            if t == 560 {
                self.eject_cameras();
                self.make_scene_cameras(1);
                if let Some(f) = self.world.fighter_mut(self.ids.boss) {
                    f.set_status(ROW_BOSS[1]);
                }
            }
            if t == 500 {
                if let Some(o) = self.world.get_mut(self.ids.pulled) {
                    o.link = 9;
                }
                self.make_spotlight();
            }
            if t == 860 {
                self.eject_cameras();
                self.make_scene_cameras(2);
                if let Some(f) = self.world.fighter_mut(self.ids.boss) {
                    f.set_status(ROW_BOSS[2]);
                }
                let mut snap = Model::new(FILE_COMMON, at::SNAP, Head::Links)
                    .joints(Joints::table(FILE_COMMON, at::SNAP_ANIM_JOINT));
                snap.play();
                self.ids.snap = self.world.object(27, Head::Links, ObjectKind::Model(snap));
            }
            if t == 1037 {
                // The default camera stops filling: the transition opens
                // over the last picture.
                if let Some(c) = self.world.camera_mut(self.ids.default_camera) {
                    c.kind = CameraKind::Default {
                        fill: None,
                        zbuffer: true,
                    };
                }
            }
            if t == 1040 {
                self.world.eject(self.ids.overlay);
                self.world.eject(self.ids.spotlight);
                self.world.eject(self.ids.boss);
                self.make_transition();
                for id in [
                    self.ids.outside,
                    self.ids.haze,
                    self.ids.books,
                    self.ids.pencils,
                    self.ids.lamp,
                    self.ids.tissues,
                ] {
                    self.world.eject(id);
                }
                let mut dg = Model::new(FILE_COMMON, at::DESK_GROUND, Head::Links).mats();
                dg.play();
                self.ids.desk_ground = self.world.object(6, Head::Links, ObjectKind::Model(dg));
                self.world
                    .object(28, Head::Zero, ObjectKind::Host(HOST_WALLPAPER));
            }
            if t == secs(19) {
                self.eject_cameras();
                self.make_close_up_effects();
                if let Some(f) = self.world.fighter_mut(self.ids.pulled) {
                    f.rotate = Vec3::ZERO;
                    f.set_status(ROW_STAND);
                }
                self.make_scene_cameras(3);
            }
            if t == secs(18) {
                self.world.eject(self.ids.transition_overlay);
                self.world.eject(self.ids.outline);
            }
            if t == secs(22) {
                exit = Some(Exit::Next(Kind::Portraits));
            }
        }
        self.run_processes();
        self.update_overlay_display();
        exit
    }

    /// The processes, in creation order of their `GObj`s.
    fn run_processes(&mut self) {
        let t = self.tics;
        for c in self.world.cameras.iter_mut() {
            if let Some(p) = c.persp_mut() {
                p.play();
            }
        }
        let ids = self.ids;
        let mut play = |id: u16, on: bool| {
            if on {
                if let Some(m) = self.world.model_mut(id) {
                    m.play();
                }
            }
        };
        // `mvOpeningRoomBackgroundProcUpdate`: from 18 seconds; ejected at 19.
        play(ids.background, t > secs(18));
        if t == secs(19) {
            self.world.eject(ids.background);
        }
        let mut play = |id: u16, on: bool| {
            if on {
                if let Some(m) = self.world.model_mut(id) {
                    m.play();
                }
            }
        };
        play(ids.logo, true);
        for id in [ids.books, ids.lamp, ids.tissues, ids.pencils] {
            play(id, t >= 560);
        }
        play(ids.boss_shadow, true);
        play(ids.spotlight, true);
        play(ids.snap, true);
        play(ids.desk_ground, t > 1060);
        play(ids.outline, true);
        play(ids.transition_overlay, true);
        play(ids.close_up_air, true);
        play(ids.close_up_ground, true);
        for id in [ids.boss, ids.pulled, ids.dropped] {
            if let Some(f) = self.world.fighter_mut(id) {
                f.plays += 1;
            }
        }
    }

    /// `mvOpeningRoomLogoWallpaperProcDisplay` and
    /// `mvOpeningRoomCloseUpOverlayProcDisplay`: their alpha steps once per
    /// draw.
    fn update_overlay_display(&mut self) {
        if self.overlay_is_close_up {
            if self.overlay_alpha < 0xA0 {
                self.overlay_alpha = (self.overlay_alpha + 0x09).min(0xA0);
            }
        } else if self.tics >= 60 && self.overlay_alpha > 0 {
            self.overlay_alpha = (self.overlay_alpha - 0x0D).max(0);
        }
        let rgba = [0, 0, 0, self.overlay_alpha as u8];
        if let Some(f) = self.world.fills_mut(self.ids.overlay) {
            *f = vec![Fill { rect: FULL, rgba }];
        }
    }

    /// `mvOpeningRoomMakePulledFighter`: on Master Hand's hand.
    fn make_pulled_fighter(&mut self) {
        let mut f = Fighter::new(self.pulled_kind, 0, ROW_PULLED);
        f.attach = Some(self.ids.boss);
        self.ids.pulled = self.world.object(6, Head::Links, ObjectKind::Fighter(f));
    }

    /// `mvOpeningRoomMakeSpotlight`: over the pulled figure.
    fn make_spotlight(&mut self) {
        let k = self.pulled_kind as usize;
        let t = SPOTLIGHT_TRANSLATES[k];
        let s = SPOTLIGHT_SCALES[k];
        let mut m = Model::new(FILE_COMMON, at::SPOTLIGHT_DL, Head::One).mats();
        m.translate = Some(Vec3::new(t[0] * 30.0, t[1] * 30.0, t[2] * 30.0));
        m.scale = Some(Vec3::new(s[0], s[1], s[2]));
        m.play();
        self.ids.spotlight = self.world.object(27, Head::One, ObjectKind::Model(m));
    }

    /// `mvOpeningRoomMakeTransitionCamera` and `...MakeTransition`.
    fn make_transition(&mut self) {
        self.world.camera(
            95,
            link(30),
            FULL,
            CameraKind::Persp {
                persp: Persp {
                    eye: Vec3::new(0.0, 0.0, 1000.0),
                    at: Vec3::ZERO,
                    fovy: 39.56115341,
                    near: 128.0,
                    far: 16384.0,
                    ..Persp::DEFAULT
                },
                zbuffer: false,
            },
        );
        // The outline, then the overlay: each list on one `DObj`, its
        // script played at its making and once a tic.
        let m = |dl, script| {
            let mut m = Model::new(FILE_TRANSITION, dl, Head::Zero)
                .joints(Joints::root(FILE_TRANSITION, script));
            m.play();
            m
        };
        self.ids.outline = self.world.object(
            30,
            Head::Zero,
            ObjectKind::HostModel(
                HOST_TRANSITION_OUTLINE,
                m(at::TRANSITION_OUTLINE_DL, at::TRANSITION_OUTLINE_ANIM_JOINT),
            ),
        );
        self.ids.transition_overlay = self.world.object(
            30,
            Head::Zero,
            ObjectKind::HostModel(
                HOST_TRANSITION_OVERLAY,
                m(at::TRANSITION_OVERLAY_DL, at::TRANSITION_OVERLAY_ANIM_JOINT),
            ),
        );
    }

    /// `mvOpeningRoomMakeCloseUpEffect`: the air's and the ground's trees
    /// at the origin, their materials and joints played at once.
    fn make_close_up_effects(&mut self) {
        let m = |graph, script| {
            let mut m = Model::new(FILE_COMMON, graph, Head::Links)
                .mats()
                .joints(Joints::table(FILE_COMMON, script))
                .at(Vec3::ZERO);
            m.play();
            m
        };
        self.ids.close_up_air = self.world.object(
            6,
            Head::Links,
            ObjectKind::Model(m(at::CLOSE_UP_AIR, at::CLOSE_UP_AIR_ANIM_JOINT)),
        );
        self.ids.close_up_ground = self.world.object(
            6,
            Head::Links,
            ObjectKind::Model(m(at::CLOSE_UP_GROUND, at::CLOSE_UP_GROUND_ANIM_JOINT)),
        );
    }

    /// `mvOpeningRoomMakeWallpaper`'s sprite: at (10, 10), drawn by the
    /// host through the depth the transition leaves (`G_ZS_PRIM` at
    /// 36863).
    pub fn wallpaper() -> Piece {
        let mut p = Piece::at(FILE_WALLPAPER, WALLPAPER, 10.0, 10.0);
        p.transparent = false;
        p
    }

    pub fn tics(&self) -> i32 {
        self.tics
    }

    /// The pulled figure's object id.
    pub fn pulled_id(&self) -> u16 {
        self.ids.pulled
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn counter() -> impl FnMut() -> u8 {
        let mut c = 0u8;
        move || {
            c = c.wrapping_add(97);
            c
        }
    }

    #[test]
    fn the_figures_differ_and_the_room_loads_the_portraits_at_22_seconds() {
        let mut r = Room::new(&mut counter());
        assert_ne!(r.pulled_kind, r.dropped_kind);
        let mut exit = None;
        for _ in 0..1320 {
            exit = r.tick(false).or(exit);
        }
        assert_eq!(exit, Some(Exit::Next(Kind::Portraits)));
    }

    #[test]
    fn master_hand_changes_status_with_the_cameras_and_leaves_at_1040() {
        let mut r = Room::new(&mut counter());
        let boss = r.ids.boss;
        for _ in 0..560 {
            r.tick(false);
        }
        assert_eq!(r.world.fighter_mut(boss).unwrap().row, ROW_BOSS[1]);
        for _ in 560..1040 {
            r.tick(false);
        }
        assert!(r.world.get(boss).is_none());
        let pulled = r.world.fighter_mut(r.ids.pulled).unwrap();
        assert_eq!(pulled.row, ROW_DROPPED);
    }

    #[test]
    fn the_page_overlay_fades_from_tic_60_and_goes_at_280() {
        let mut r = Room::new(&mut counter());
        for _ in 0..59 {
            r.tick(false);
        }
        assert_eq!(r.overlay_alpha, 0xFF);
        r.tick(false);
        assert_eq!(r.overlay_alpha, 0xFF - 0x0D);
        for _ in 60..280 {
            r.tick(false);
        }
        assert!(r.world.get(r.ids.overlay).is_none());
    }
}
