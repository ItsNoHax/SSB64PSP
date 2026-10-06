//! The opening's eight fighter scenes (`mvOpeningMario`, `...Donkey`,
//! `...Samus`, `...Fox`, `...Link`, `...Yoshi`, `...Pikachu`,
//! `...Kirby`) and the jungle (`mvOpeningJungle`): each runs a battle
//! (`game_type == nSCBattleGameTypeMovie`) on a stage, its fighters
//! driven by input scripts (`nFTPlayerKindKey`), under a movie camera.
//!
//! A fighter scene shows the fighter's name for 15 tics; at tic 15 it
//! makes the stage and the fighter (the motion window), a panel of colour
//! beside it and the posed fighter sliding into the panel, and at tic 60
//! loads the next scene. Its movie camera moves from its start to its end
//! description by a 45th per tic. The jungle's camera plays its file's
//! animation instead.
//!
//! The scenes' tables are their source's statics; all eight scenes share
//! their code, which this ports once.

use alloc::vec;
use alloc::vec::Vec;

use super::movie::{link, CameraKind, Fighter, Fill, Head, ObjectKind, Persp, View, World, FULL};
use super::{Exit, Kind};
use crate::fighter::FighterKind;
use crate::menu::Piece;
use ssb_engine::math::Vec3;

/// `llIFCommonAnnounceCommonFileID` and its letters' sprites (A to Z).
pub const FILE_ANNOUNCE: u32 = 0x25;
pub const LETTERS: [u32; 26] = [
    0x5E0, 0x9A8, 0xD80, 0x1268, 0x1628, 0x1A00, 0x1F08, 0x2408, 0x26B8, 0x2A90, 0x2F98, 0x3358,
    0x3980, 0x3E88, 0x44B0, 0x4890, 0x4F10, 0x5418, 0x57F0, 0x5BD0, 0x60D8, 0x65D8, 0x6C00, 0x7108,
    0x7608, 0x7AE8,
];

/// `llMVOpeningCommonFileID`: the posed fighters' camera animations.
pub const FILE_COMMON: u32 = 0x41;

/// `nFTDemoStatusStance`'s submotion row.
pub const ROW_STANCE: u8 = 12;

/// `GRKind`s the scenes' battles are on.
pub mod gkind {
    pub const CASTLE: u8 = 0;
    pub const SECTOR: u8 = 1;
    pub const JUNGLE: u8 = 2;
    pub const ZEBES: u8 = 3;
    pub const HYRULE: u8 = 4;
    pub const YOSTER: u8 = 5;
    pub const PUPUPU: u8 = 6;
    pub const YAMABUKI: u8 = 7;
}

/// `nMPMapObjKindMoviePlayer1` to `...3`.
pub const MOVIE_PLAYER1: u16 = 0x15;
pub const MOVIE_PLAYER2: u16 = 0x16;
pub const MOVIE_PLAYER3: u16 = 0x17;

/// A `CObjDesc`: eye, at and `up.x`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CObjDesc {
    pub eye: Vec3,
    pub at: Vec3,
    pub up_x: f32,
}

const fn v(x: f32, y: f32, z: f32) -> Vec3 {
    Vec3 { x, y, z }
}

const fn desc(eye: Vec3, at: Vec3, up_x: f32) -> CObjDesc {
    CObjDesc { eye, at, up_x }
}

/// One fighter scene's statics.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Params {
    pub kind: Kind,
    pub fighter: FighterKind,
    pub gkind: u8,
    /// `dMVOpening*CObjDescStart` and `...End`.
    pub start: CObjDesc,
    pub end: CObjDesc,
    /// `dMVOpening*KeyEvents` as halfwords.
    pub keys: &'static [u16],
    /// The name's letters (0 is A) and each letter's offset.
    pub letters: &'static [(u8, f32, f32)],
    /// The name's origin, added to each letter's offset.
    pub name_at: [f32; 2],
    /// The movie camera's viewport and `persp.aspect`.
    pub motion_viewport: [f32; 4],
    pub motion_aspect: f32,
    /// `desc.lr`.
    pub lr: i8,
    /// Added to `MoviePlayer1`'s position before the camera and fighter
    /// are placed there.
    pub spawn_offset: Vec3,
    /// The posed panel: its fill, the posed camera's viewport and aspect.
    pub fill: [f32; 4],
    pub fill_color: [u8; 3],
    pub posed_viewport: [f32; 4],
    pub posed_aspect: f32,
    /// `llMVOpeningCommon*CamAnimJoint`.
    pub posed_camera: u32,
    /// The posed fighter's start (`desc.pos`) and the axis its speed is
    /// added along (`translate += axis * speed`).
    pub posed_start: Vec3,
    pub posed_axis: Vec3,
}

/// `FTKEY_EVENT_STICK(x, y, wait)` and `FTKEY_EVENT_BUTTON(buttons, wait)`
/// as `ft/ftkey.c` reads them.
mod keys {
    pub const MARIO: [u16; 15] = [
        0x2000, 0x0000, 0x1001, 0x8000, 0x100B, 0x0000, 0x1001, 0x8000, 0x1014, 0x0000, 0x2000,
        0x00B0, 0x1001, 0x8000, 0x0000,
    ];
    pub const DONKEY: [u16; 9] = [
        0x2000, 0x00B0, 0x1001, 0x4000, 0x1001, 0x0000, 0x1001, 0x4000, 0x0000,
    ];
    pub const SAMUS: [u16; 5] = [0x1001, 0x2000, 0x1001, 0x8000, 0x0000];
    pub const FOX: [u16; 11] = [
        0x2001, 0xCE00, 0x1001, 0x4000, 0x100C, 0x0000, 0x1001, 0x4000, 0x100C, 0x0000, 0x0000,
    ];
    pub const LINK: [u16; 3] = [0x1001, 0x0020, 0x0000];
    pub const YOSHI: [u16; 7] = [0x2014, 0x5000, 0x1001, 0x2000, 0x1001, 0x8000, 0x0000];
    pub const PIKACHU: [u16; 1] = [0x0000];
    pub const KIRBY: [u16; 5] = [0x2001, 0x2D50, 0x1001, 0x8000, 0x0000];
    pub const JUNGLE_DONKEY: [u16; 35] = [
        0x2000, 0x00B0, 0x1001, 0x4000, 0x1001, 0x0000, 0x2032, 0x0000, 0x201E, 0x0F50, 0x200A,
        0x5000, 0x1001, 0x8000, 0x1001, 0x0000, 0x203C, 0x5000, 0x2003, 0xE200, 0x200A, 0x0000,
        0x2001, 0xB000, 0x1001, 0x4000, 0x1001, 0x0000, 0x1001, 0x4000, 0x1001, 0x0000, 0x2001,
        0x0000, 0x0000,
    ];
    pub const JUNGLE_SAMUS: [u16; 33] = [
        0x2014, 0xB000, 0x204B, 0x0000, 0x1001, 0x2000, 0x203C, 0xB000, 0x2001, 0x0000, 0x2005,
        0xB000, 0x2001, 0x0000, 0x2017, 0x0000, 0x2005, 0xB000, 0x1001, 0x0000, 0x2028, 0x0000,
        0x2003, 0x1E00, 0x2001, 0x0000, 0x1001, 0x4000, 0x1001, 0x0000, 0x1001, 0x4000, 0x0000,
    ];
}

/// Letter index from an ASCII capital.
const fn l(c: u8) -> u8 {
    c - b'A'
}

/// A side panel's layout (the posed fighter on the left or right of a
/// motion window 200 wide), and a band's (above or below one 140 high).
const LEFT_PANEL: [f32; 4] = [10.0, 10.0, 110.0, 230.0];
const RIGHT_PANEL: [f32; 4] = [210.0, 10.0, 310.0, 230.0];
const RIGHT_MOTION: [f32; 4] = [110.0, 10.0, 310.0, 230.0];
const LEFT_MOTION: [f32; 4] = [10.0, 10.0, 210.0, 230.0];

/// The eight scenes, in play order.
pub const SCENES: [Params; 8] = [
    Params {
        kind: Kind::Mario,
        fighter: FighterKind::Mario,
        gkind: gkind::CASTLE,
        start: desc(v(300.0, 500.0, 1700.0), v(0.0, 100.0, 0.0), 0.15),
        end: desc(v(800.0, 500.0, 1300.0), v(100.0, 100.0, 0.0), 0.15),
        keys: &keys::MARIO,
        letters: &[
            (l(b'M'), 0.0, 0.0),
            (l(b'A'), 40.0, 0.0),
            (l(b'R'), 80.0, 0.0),
            (l(b'I'), 110.0, 0.0),
            (l(b'O'), 125.0, 0.0),
        ],
        name_at: [80.0, 100.0],
        motion_viewport: RIGHT_MOTION,
        motion_aspect: 10.0 / 11.0,
        lr: 1,
        spawn_offset: v(0.0, 0.0, 0.0),
        fill: LEFT_PANEL,
        fill_color: [0xA0, 0xAA, 0xFF],
        posed_viewport: LEFT_PANEL,
        posed_aspect: 5.0 / 11.0,
        posed_camera: 0x0,
        posed_start: v(0.0, 600.0, 0.0),
        posed_axis: v(0.0, -1.0, 0.0),
    },
    Params {
        kind: Kind::Donkey,
        fighter: FighterKind::Donkey,
        gkind: gkind::JUNGLE,
        start: desc(v(-1100.0, 150.0, 400.0), v(0.0, 150.0, 0.0), 0.0),
        end: desc(v(-900.0, 500.0, 1800.0), v(0.0, 500.0, 0.0), 0.0),
        keys: &keys::DONKEY,
        // The US build's "DK".
        letters: &[(l(b'D'), 0.0, 0.0), (l(b'K'), 40.0, 0.0)],
        name_at: [120.0, 100.0],
        motion_viewport: LEFT_MOTION,
        motion_aspect: 10.0 / 11.0,
        lr: -1,
        spawn_offset: v(0.0, 0.0, 0.0),
        fill: RIGHT_PANEL,
        fill_color: [0x46, 0x5A, 0x00],
        posed_viewport: RIGHT_PANEL,
        posed_aspect: 5.0 / 11.0,
        posed_camera: 0x30,
        posed_start: v(0.0, -600.0, 0.0),
        posed_axis: v(0.0, 1.0, 0.0),
    },
    Params {
        kind: Kind::Link,
        fighter: FighterKind::Link,
        gkind: gkind::HYRULE,
        start: desc(v(-800.0, 180.0, 800.0), v(0.0, 180.0, 0.0), 0.0),
        end: desc(v(200.0, 0.0, 400.0), v(0.0, 240.0, 0.0), 0.4),
        keys: &keys::LINK,
        letters: &[
            (l(b'L'), 0.0, 0.0),
            (l(b'I'), 30.0, 0.0),
            (l(b'N'), 45.0, 0.0),
            (l(b'K'), 80.0, 0.0),
        ],
        name_at: [100.0, 100.0],
        motion_viewport: [10.0, 90.0, 310.0, 230.0],
        motion_aspect: 15.0 / 7.0,
        lr: 1,
        spawn_offset: v(0.0, 0.0, 0.0),
        fill: [10.0, 10.0, 310.0, 90.0],
        fill_color: [0x96, 0x78, 0xB4],
        posed_viewport: [10.0, 10.0, 310.0, 90.0],
        posed_aspect: 26.25 / 7.0,
        posed_camera: 0xC0,
        posed_start: v(600.0, 0.0, 0.0),
        posed_axis: v(-1.0, 0.0, 0.0),
    },
    Params {
        kind: Kind::Samus,
        fighter: FighterKind::Samus,
        gkind: gkind::ZEBES,
        start: desc(v(400.0, 1100.0, 0.0), v(0.0, 200.0, 0.0), 0.6),
        end: desc(v(1600.0, 230.0, 200.0), v(0.0, 200.0, 0.0), 0.6),
        keys: &keys::SAMUS,
        letters: &[
            (l(b'S'), 0.0, 0.0),
            (l(b'A'), 30.0, 0.0),
            (l(b'M'), 70.0, 0.0),
            (l(b'U'), 110.0, 0.0),
            (l(b'S'), 140.0, 0.0),
        ],
        name_at: [80.0, 100.0],
        motion_viewport: RIGHT_MOTION,
        motion_aspect: 10.0 / 11.0,
        lr: 1,
        spawn_offset: v(0.0, 0.0, 0.0),
        fill: LEFT_PANEL,
        fill_color: [0x00, 0x00, 0x50],
        posed_viewport: LEFT_PANEL,
        posed_aspect: 5.0 / 11.0,
        posed_camera: 0x60,
        posed_start: v(0.0, 600.0, 0.0),
        posed_axis: v(0.0, -1.0, 0.0),
    },
    Params {
        kind: Kind::Yoshi,
        fighter: FighterKind::Yoshi,
        gkind: gkind::YOSTER,
        start: desc(v(1200.0, 150.0, 1000.0), v(100.0, 200.0, 0.0), 0.0),
        end: desc(v(2000.0, 100.0, 600.0), v(1300.0, 100.0, -100.0), 0.0),
        keys: &keys::YOSHI,
        letters: &[
            (l(b'Y'), 0.0, 0.0),
            (l(b'O'), 30.0, 0.0),
            (l(b'S'), 65.0, 0.0),
            (l(b'H'), 95.0, 0.0),
            (l(b'I'), 128.0, 0.0),
        ],
        name_at: [80.0, 100.0],
        motion_viewport: [10.0, 10.0, 310.0, 150.0],
        motion_aspect: 15.0 / 7.0,
        lr: 1,
        spawn_offset: v(-1000.0, 70.0, 0.0),
        fill: [10.0, 150.0, 310.0, 230.0],
        fill_color: [0xFF, 0xBE, 0x5A],
        posed_viewport: [10.0, 150.0, 310.0, 230.0],
        posed_aspect: 26.25 / 7.0,
        posed_camera: 0xF0,
        posed_start: v(-600.0, 0.0, 0.0),
        posed_axis: v(1.0, 0.0, 0.0),
    },
    Params {
        kind: Kind::Kirby,
        fighter: FighterKind::Kirby,
        gkind: gkind::PUPUPU,
        start: desc(v(0.0, 400.0, 2000.0), v(0.0, 400.0, 0.0), 0.0),
        end: desc(v(1100.0, 400.0, 1800.0), v(1100.0, 400.0, 0.0), 0.0),
        keys: &keys::KIRBY,
        letters: &[
            (l(b'K'), 0.0, 0.0),
            (l(b'I'), 35.0, 0.0),
            (l(b'R'), 50.0, 0.0),
            (l(b'B'), 80.0, 0.0),
            (l(b'Y'), 110.0, 0.0),
        ],
        name_at: [90.0, 100.0],
        motion_viewport: LEFT_MOTION,
        motion_aspect: 10.0 / 11.0,
        lr: 1,
        spawn_offset: v(0.0, 30.0, 0.0),
        fill: RIGHT_PANEL,
        fill_color: [0x50, 0xAA, 0xFF],
        posed_viewport: RIGHT_PANEL,
        posed_aspect: 5.0 / 11.0,
        posed_camera: 0x150,
        posed_start: v(0.0, 600.0, 0.0),
        posed_axis: v(0.0, -1.0, 0.0),
    },
    Params {
        kind: Kind::Fox,
        fighter: FighterKind::Fox,
        gkind: gkind::SECTOR,
        start: desc(v(-400.0, 320.0, 100.0), v(0.0, 320.0, 0.0), 0.0),
        end: desc(v(-3000.0, 300.0, 250.0), v(0.0, 300.0, -200.0), 0.7),
        keys: &keys::FOX,
        letters: &[
            (l(b'F'), 0.0, 0.0),
            (l(b'O'), 30.0, 0.0),
            (l(b'X'), 75.0, 0.0),
        ],
        name_at: [110.0, 100.0],
        motion_viewport: LEFT_MOTION,
        motion_aspect: 10.0 / 11.0,
        lr: -1,
        spawn_offset: v(0.0, 0.0, 0.0),
        fill: RIGHT_PANEL,
        fill_color: [0x00, 0x3C, 0x28],
        posed_viewport: RIGHT_PANEL,
        posed_aspect: 5.0 / 11.0,
        posed_camera: 0x90,
        posed_start: v(0.0, 600.0, 0.0),
        posed_axis: v(0.0, -1.0, 0.0),
    },
    Params {
        kind: Kind::Pikachu,
        fighter: FighterKind::Pikachu,
        gkind: gkind::YAMABUKI,
        start: desc(v(0.0, 0.0, 20000.0), v(0.0, 0.0, 0.0), 0.0),
        end: desc(v(50.0, -1640.0, 1000.0), v(50.0, -1640.0, 0.0), 0.0),
        keys: &keys::PIKACHU,
        letters: &[
            (l(b'P'), 0.0, 0.0),
            (l(b'I'), 30.0, 0.0),
            (l(b'K'), 45.0, 0.0),
            (l(b'A'), 75.0, 0.0),
            (l(b'C'), 110.0, 0.0),
            (l(b'H'), 140.0, 0.0),
            (l(b'U'), 170.0, 0.0),
        ],
        name_at: [65.0, 100.0],
        motion_viewport: RIGHT_MOTION,
        motion_aspect: 10.0 / 11.0,
        lr: 1,
        spawn_offset: v(0.0, 0.0, 0.0),
        fill: LEFT_PANEL,
        fill_color: [0x6E, 0xAA, 0x6E],
        posed_viewport: LEFT_PANEL,
        posed_aspect: 5.0 / 11.0,
        posed_camera: 0x120,
        posed_start: v(0.0, -600.0, 0.0),
        posed_axis: v(0.0, 1.0, 0.0),
    },
];

/// The scene's statics.
pub fn params(kind: Kind) -> Option<&'static Params> {
    SCENES.iter().find(|p| p.kind == kind)
}

/// The camera's baked plays' slot (`ssb_rom::opening::camera_slot`).
pub const fn camera_slot(file: u32, offset: u32) -> u32 {
    0x4000_0000 | (file << 20) | offset
}

/// A battle fighter the motion window makes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Player {
    pub kind: FighterKind,
    /// The map object its position comes from, and what is added to it.
    pub spawn: u16,
    pub offset: Vec3,
    pub lr: i8,
    pub damage: u16,
    /// `passive_vars.donkey.charge_level` or `.samus.charge_level`.
    pub charge: Option<u8>,
    pub keys: &'static [u16],
}

/// The battle a scene makes at its motion window.
#[derive(Debug, Clone, PartialEq)]
pub struct Battle {
    pub gkind: u8,
    pub players: Vec<Player>,
    /// `ifScreenFlashMakeInterface`: the jungle's KO flash.
    pub screen_flash: bool,
}

/// The movie camera (`gmCameraMakeMovieCamera` with its process ended):
/// what the battle draws through.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MovieCamera {
    pub viewport: [f32; 4],
    pub aspect: f32,
    pub near: f32,
    pub far: f32,
    pub eye: Vec3,
    pub at: Vec3,
    pub up_x: f32,
    pub fovy: f32,
    /// The camera's baked animation and plays (the jungle's).
    pub anim: Option<super::movie::CamAnim>,
}

/// `dGMCameraPerspDefault`'s field of view and planes.
const MOVIE_FOVY: f32 = 38.0;
const MOVIE_NEAR: f32 = 256.0;
const MOVIE_FAR: f32 = 39936.0;

/// What the scene's tick asks the host for.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Tick {
    pub exit: Option<Exit>,
    /// `mvOpening*MakeMotionWindow`: make this battle now.
    pub battle: Option<Battle>,
}

/// A fighter scene.
#[derive(Debug, Clone, PartialEq)]
pub struct FighterScene {
    pub params: &'static Params,
    tics: i32,
    pub world: World,
    name: u16,
    posed: Option<u16>,
    /// `sMVOpening*PosedFighterSpeed`.
    posed_speed: f32,
    /// `sMVOpening*AdjustedStartCObjDesc` and `...End`.
    start: CObjDesc,
    end: CObjDesc,
    pub camera: Option<MovieCamera>,
}

impl FighterScene {
    /// `mvOpening*FuncStart`.
    pub fn new(params: &'static Params) -> FighterScene {
        let mut world = World::new();
        world.camera(
            100,
            0,
            [0.0, 0.0, 320.0, 240.0],
            CameraKind::Default {
                fill: Some([0, 0, 0, 0xFF]),
                zbuffer: true,
            },
        );
        // `mvOpening*MakeNameCamera`, `...PosedWallpaperCamera` and
        // `...PosedFighterCamera`.
        world.camera(80, link(27), FULL, CameraKind::Sprite);
        world.camera(
            20,
            link(28),
            params.posed_viewport,
            CameraKind::Default {
                fill: None,
                zbuffer: true,
            },
        );
        world.camera(20, link(28), params.posed_viewport, CameraKind::Sprite);
        world.camera(
            10,
            link(26),
            params.posed_viewport,
            CameraKind::Persp {
                persp: Persp {
                    aspect: params.posed_aspect,
                    ..Persp::animated(
                        camera_slot(FILE_COMMON, params.posed_camera),
                        View::LookAt,
                        100.0,
                        12800.0,
                    )
                },
                zbuffer: false,
            },
        );
        // `mvOpening*MakeName`: white letters (`envcolor` and
        // `sprite.red..blue` all 0xFF).
        let pieces = params
            .letters
            .iter()
            .map(|&(c, x, y)| {
                Piece::clear(
                    FILE_ANNOUNCE,
                    LETTERS[usize::from(c)],
                    x + params.name_at[0],
                    y + params.name_at[1],
                )
                .prim([0xFF; 3])
                .env([0xFF; 3])
            })
            .collect();
        let name = world.object(27, Head::Zero, ObjectKind::Sprites(pieces));
        FighterScene {
            params,
            tics: 0,
            world,
            name,
            posed: None,
            posed_speed: 0.0,
            start: params.start,
            end: params.end,
            camera: None,
        }
    }

    /// One frame: `mvOpening*FuncRun`, then the processes (the movie
    /// camera's and the posed fighter's). `spawn` is `MoviePlayer1`'s
    /// position, read when the motion window is made.
    pub fn tick(&mut self, tapped: bool, spawn: Vec3) -> Tick {
        let mut t = Tick::default();
        self.tics += 1;
        if tapped {
            t.exit = Some(Exit::Title);
        }
        let p = self.params;
        if self.tics == 15 {
            self.world.eject(self.name);
            let pos = spawn + p.spawn_offset;
            self.make_motion_camera(pos);
            t.battle = Some(Battle {
                gkind: p.gkind,
                players: vec![Player {
                    kind: p.fighter,
                    spawn: MOVIE_PLAYER1,
                    offset: p.spawn_offset,
                    lr: p.lr,
                    damage: 0,
                    charge: None,
                    keys: p.keys,
                }],
                screen_flash: false,
            });
            // `mvOpening*MakePosedWallpaper` and `...MakePosedFighter`.
            let c = p.fill_color;
            self.world.object(
                28,
                Head::Zero,
                ObjectKind::Fills(vec![Fill {
                    rect: p.fill,
                    rgba: [c[0], c[1], c[2], 0xFF],
                }]),
            );
            let mut f = Fighter::new(p.fighter, 0, ROW_STANCE);
            f.translate = p.posed_start;
            self.posed = Some(self.world.object(26, Head::Links, ObjectKind::Fighter(f)));
        }
        if self.tics == 60 {
            t.exit = Some(Exit::after(p.kind));
        }
        // The cameras' processes, then the fighters'.
        for c in self.world.cameras.iter_mut() {
            if let Some(persp) = c.persp_mut() {
                persp.play();
            }
        }
        self.update_motion_camera();
        self.update_posed_fighter();
        t
    }

    /// `mvOpening*MakeMotionCamera`.
    fn make_motion_camera(&mut self, mv: Vec3) {
        let p = self.params;
        self.start = p.start;
        self.end = p.end;
        for d in [&mut self.start, &mut self.end] {
            d.eye += mv;
            d.at += mv;
        }
        self.camera = Some(MovieCamera {
            viewport: p.motion_viewport,
            aspect: p.motion_aspect,
            near: MOVIE_NEAR,
            far: MOVIE_FAR,
            eye: self.start.eye,
            at: self.start.at,
            up_x: self.start.up_x,
            fovy: MOVIE_FOVY,
            anim: None,
        });
    }

    /// `mvOpening*MotionCameraProcUpdate`.
    fn update_motion_camera(&mut self) {
        let (s, e) = (self.start, self.end);
        let Some(c) = self.camera.as_mut() else {
            return;
        };
        if self.tics >= 15 {
            c.eye.x += (e.eye.x - s.eye.x) / 45.0;
            c.eye.y += (e.eye.y - s.eye.y) / 45.0;
            c.eye.z += (e.eye.z - s.eye.z) / 45.0;
            c.at.x += (e.at.x - s.at.x) / 45.0;
            c.at.y += (e.at.y - s.at.y) / 45.0;
            c.at.z += (e.at.z - s.at.z) / 45.0;
            c.up_x += (e.up_x - s.up_x) / 45.0;
        }
    }

    /// `mvOpening*PosedFighterProcUpdate`.
    fn update_posed_fighter(&mut self) {
        let Some(id) = self.posed else {
            return;
        };
        let t = self.tics;
        match t {
            15 => self.posed_speed = 17.0,
            45 => self.posed_speed = 15.0,
            60 => self.posed_speed = 0.0,
            _ => {}
        }
        if t > 15 && t < 45 {
            self.posed_speed += -1.0 / 15.0;
        }
        if t > 45 && t < 60 {
            self.posed_speed += -1.0;
        }
        let axis = self.params.posed_axis;
        let speed = self.posed_speed;
        if let Some(f) = self.world.fighter_mut(id) {
            f.translate += Vec3::new(axis.x * speed, axis.y * speed, axis.z * speed);
            f.plays += 1;
        }
    }

    pub fn tics(&self) -> i32 {
        self.tics
    }
}

/// `mvOpeningJungle`: Donkey Kong and Samus under the jungle's animated
/// camera, from tic 1 to 320.
#[derive(Debug, Clone, PartialEq)]
pub struct Jungle {
    tics: i32,
    pub world: World,
    pub camera: MovieCamera,
    battle: Option<Battle>,
}

/// `llMVOpeningJungleFileID`'s camera.
pub const JUNGLE_CAMERA_SLOT: u32 = camera_slot(0x40, 0x0);

impl Jungle {
    /// `mvOpeningJungleFuncStart`: the battle is made at once.
    pub fn new() -> Jungle {
        let mut world = World::new();
        world.camera(
            100,
            0,
            [0.0, 0.0, 320.0, 240.0],
            CameraKind::Default {
                fill: Some([0, 0, 0, 0xFF]),
                zbuffer: true,
            },
        );
        let battle = Battle {
            gkind: gkind::JUNGLE,
            players: vec![
                Player {
                    kind: FighterKind::Donkey,
                    spawn: MOVIE_PLAYER2,
                    offset: Vec3::ZERO,
                    lr: 1,
                    damage: 200,
                    charge: Some(9),
                    keys: &keys::JUNGLE_DONKEY,
                },
                Player {
                    kind: FighterKind::Samus,
                    spawn: MOVIE_PLAYER3,
                    offset: v(1100.0, 0.0, 0.0),
                    lr: -1,
                    damage: 40,
                    charge: Some(6),
                    keys: &keys::JUNGLE_SAMUS,
                },
            ],
            screen_flash: true,
        };
        // `mvOpeningJungleMakeGroundViewport`: its animation's first play
        // at once (`gcPlayCamAnim`).
        let camera = MovieCamera {
            viewport: FULL,
            aspect: 15.0 / 11.0,
            near: 50.0,
            far: 15000.0,
            eye: Vec3::ZERO,
            at: Vec3::ZERO,
            up_x: 0.0,
            fovy: MOVIE_FOVY,
            anim: Some(super::movie::CamAnim {
                slot: JUNGLE_CAMERA_SLOT,
                plays: 1,
            }),
        };
        Jungle {
            tics: 0,
            world,
            camera,
            battle: Some(battle),
        }
    }

    /// The battle, which the host makes before the first tick.
    pub fn take_battle(&mut self) -> Option<Battle> {
        self.battle.take()
    }

    /// One frame: `mvOpeningJungleFuncRun`, then the camera's process.
    pub fn tick(&mut self, tapped: bool) -> Tick {
        let mut t = Tick::default();
        self.tics += 1;
        if tapped {
            t.exit = Some(Exit::Title);
        }
        if self.tics == 320 {
            t.exit = Some(Exit::after(Kind::Jungle));
        }
        if let Some(a) = self.camera.anim.as_mut() {
            a.plays += 1;
        }
        t
    }
}

impl Default for Jungle {
    fn default() -> Self {
        Jungle::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scenes_play_in_order_and_each_names_its_own_fighter() {
        let order: Vec<Kind> = SCENES.iter().map(|p| p.kind).collect();
        assert_eq!(
            order,
            [
                Kind::Mario,
                Kind::Donkey,
                Kind::Link,
                Kind::Samus,
                Kind::Yoshi,
                Kind::Kirby,
                Kind::Fox,
                Kind::Pikachu
            ]
        );
        for p in &SCENES {
            assert_eq!(p.kind.fighter(), Some(p.fighter));
            assert_eq!(*p.keys.last().unwrap(), 0);
        }
    }

    #[test]
    fn the_motion_window_opens_at_15_and_the_scene_ends_at_60() {
        let mut s = FighterScene::new(params(Kind::Mario).unwrap());
        let spawn = v(10.0, 20.0, 0.0);
        let mut battle = None;
        let mut exit = None;
        for _ in 0..60 {
            let t = s.tick(false, spawn);
            battle = battle.or(t.battle);
            exit = t.exit.or(exit);
            if s.tics() == 15 {
                // The camera's process ran once this tic.
                let c = s.camera.unwrap();
                assert!((c.eye.x - (310.0 + 500.0 / 45.0)).abs() < 1e-3);
            }
        }
        assert_eq!(battle.unwrap().players[0].kind, FighterKind::Mario);
        assert_eq!(exit, Some(Exit::Next(Kind::Donkey)));
        let c = s.camera.unwrap();
        // 46 steps of a 45th: one past the end description.
        assert!((c.eye.x - (310.0 + 500.0 * 46.0 / 45.0)).abs() < 1e-2);
    }

    #[test]
    fn the_posed_fighter_slides_back_to_the_panel() {
        let mut s = FighterScene::new(params(Kind::Mario).unwrap());
        for _ in 0..60 {
            s.tick(false, Vec3::ZERO);
        }
        let f = match &s.world.objects.iter().find(|o| o.link == 26).unwrap().kind {
            ObjectKind::Fighter(f) => *f,
            _ => unreachable!(),
        };
        // 17, then 15 tics of -1/15 to 15, then 15 down by 1 to 0.
        assert!(f.translate.y.abs() < 30.0, "{}", f.translate.y);
    }

    #[test]
    fn the_jungle_ends_at_320() {
        let mut j = Jungle::new();
        assert!(j.take_battle().is_some());
        let mut exit = None;
        for _ in 0..320 {
            exit = j.tick(false).exit.or(exit);
        }
        assert_eq!(exit, Some(Exit::Next(Kind::Yoster)));
        assert_eq!(j.camera.anim.unwrap().plays, 321);
    }
}
