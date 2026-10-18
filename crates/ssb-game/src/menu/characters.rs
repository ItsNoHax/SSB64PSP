//! `mn/mndata/mncharacters.c`: Characters. One page per fighter, its
//! name, series emblem, story and works, and the fighter playing random
//! motions (`dMNCharacters*MotionDescs`) while it turns; holding Z turns it
//! and tilts the camera with the stick. Left and right change the page
//! over the unlocked fighters; B saves the page's fighter
//! (`characters_fkind`) and goes back to Data.
//!
//! The fighter is a demo fighter: `ftMainSetStatus` plays a status's
//! figatree and motion script without its update, physics or map
//! processes. The host plays [`Fighter::motion`]'s clip and reports when
//! `anim_frame` reads 0 ([`CharactersMenu::tick_fighter`]).
//!
//! The title's attract demo enters this scene too ([`CharactersMenu::demo`]),
//! showing two fighters for 300 ticks each.

use super::{
    data_common, Draw, Pad, Piece, Repeat, Scene, FILE_CHARACTERS, FILE_DATA_COMMON, LEFT, RIGHT,
};
use crate::backup::Backup;
use crate::fighter::FighterKind;
use crate::results_scene::{Camera, DemoStatus, FIGHTER_SCALES};
use crate::status::{
    AnyStatus, CaptainStatus, DonkeyStatus, FoxStatus, KirbyStatus, LinkStatus, MarioStatus,
    NessStatus, PikachuStatus, PurinStatus, SamusStatus, Status, YoshiStatus,
};
use ssb_engine::input::N64Buttons;
use ssb_engine::math::{self, Vec3};

/// `llMNCharacters*Sprite` (`reloc_data.us.h`).
pub mod sprite {
    pub const LABEL: u32 = 0x630;
    pub const NAME_TAG_DEFAULT: u32 = 0x1230;
    pub const NAME_TAG_TALL: u32 = 0x28F0;
    /// `*NameSprite`, by `FTKind`.
    pub const NAMES: [u32; 12] = [
        0x2F98, 0x33A0, 0x4290, 0x4910, 0x4F78, 0x5398, 0x58F8, 0x6828, 0x6E48, 0x7628, 0x82E0,
        0x8828,
    ];
    /// `*StorySprite`, by `FTKind`.
    pub const STORIES: [u32; 12] = [
        0xACA8, 0xD128, 0xF5A8, 0x11A28, 0x13EA8, 0x16328, 0x187A8, 0x1AC28, 0x1D0A8, 0x1F528,
        0x219A8, 0x23E28,
    ];
    pub const WORKS_WALLPAPER: u32 = 0x25058;
    /// `*WorksSprite`, by `FTKind`.
    pub const WORKS: [u32; 12] = [
        0x25AB8, 0x26518, 0x26F78, 0x279D8, 0x28438, 0x28E98, 0x298F8, 0x2A358, 0x2ADB8, 0x2B818,
        0x2C278, 0x2CCD8,
    ];
    /// `MotionSpecial{Hi,N,Lw}InputSprite`.
    pub const MOTION_INPUTS: [u32; 3] = [0x2CDA8, 0x2CE78, 0x2CF48];
    /// `mnCharactersUpdateMotionName`'s `motion_names`, by `FTKind`, then
    /// Hi, N and Lw. Luigi's up and neutral specials are Mario's names.
    pub const MOTION_NAMES: [[u32; 3]; 12] = [
        [0x2D088, 0x2DE48, 0x2EC48],
        [0x2D1C8, 0x2DF88, 0x2ED88],
        [0x2D308, 0x2E0C8, 0x2EEC8],
        [0x2D448, 0x2E208, 0x2F008],
        [0x2D088, 0x2DE48, 0x2F148],
        [0x2D588, 0x2E348, 0x2F288],
        [0x2D6C8, 0x2E488, 0x2F3C8],
        [0x2D808, 0x2E5C8, 0x2F508],
        [0x2D948, 0x2E740, 0x2F648],
        [0x2DA88, 0x2E888, 0x2F788],
        [0x2DBC8, 0x2E9C8, 0x2F8C8],
        [0x2DD08, 0x2EB08, 0x2FA08],
    ];
    pub const STORY_WALLPAPER: u32 = 0x30888;
}

/// `mnCharactersGetFighterKind`: the pages' order.
pub const PAGES: [FighterKind; 12] = [
    FighterKind::Mario,
    FighterKind::Luigi,
    FighterKind::Donkey,
    FighterKind::Link,
    FighterKind::Samus,
    FighterKind::Yoshi,
    FighterKind::Kirby,
    FighterKind::Fox,
    FighterKind::Pikachu,
    FighterKind::Purin,
    FighterKind::Captain,
    FighterKind::Ness,
];

/// `mnCharactersGetPage`.
pub fn page_of(kind: FighterKind) -> usize {
    PAGES.iter().position(|&k| k == kind).unwrap_or(0)
}

/// `nMNCharactersMotionKindEnumCount`.
pub const MOTION_KINDS: usize = 39;
pub const SPECIAL_HI: usize = 0;
pub const SPECIAL_N: usize = 1;
pub const SPECIAL_LW: usize = 2;
const COMMON_START: usize = 3;
const JUMP_AERIAL_F: usize = 9;
const JUMP_AERIAL_B: usize = 10;
const ATTACK1: usize = 18;
const ATTACK_AIR_START: usize = 26;
const ATTACK_AIR_END: usize = 30;

/// A track's `anim_length` that waits for the clip's end.
pub const UNTIL_END: i32 = 666;

/// What a motion track plays (`MNCharactersMotion::status_id`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Motion {
    /// `FTSTATUS_CHARACTERS_NULL`: the motion has ended.
    Null,
    /// `FTSTATUS_CHARACTERS_DEMO(status)`: a battle status's figatree.
    Status(AnyStatus),
    /// `nFTDemoStatusWin1` to `...Lose`.
    Demo(DemoStatus),
}

/// `MNCharactersMotion`: what plays and for how many ticks, or
/// [`UNTIL_END`]. The `FTSTATUS_PRESERVE_*` flags only keep a colour
/// animation, effect or model part across the change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Track {
    pub motion: Motion,
    pub length: i32,
}

const NULL: Track = Track {
    motion: Motion::Null,
    length: 1,
};

use AnyStatus as S;

/// `MNCharactersMotion` as the ROM holds it (`s32 status_id, anim_length;
/// u32 flags`). The `FTSTATUS_PRESERVE_*` `flags` only keep a colour
/// animation, effect or model part across a status change, which the demo
/// fighter's [`Track`] does not carry.
#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
struct RawTrack {
    status: u32,
    length: i32,
    flags: u32,
}

/// The motion tables (`dMNCharactersSpecialMotion*`, `...CommonMotionDescs`,
/// `...Kirby*`, `...Purin*` and `...Attack1MotionDescs`), generated at build
/// time from the ROM by `crates/ssb-tablegen` (`menu_data.rs`).
mod generated {
    use super::RawTrack;
    include!(concat!(env!("OUT_DIR"), "/characters_motion.rs"));
}

/// `FTSTATUS_CHARACTERS_NULL`.
const STATUS_NULL: u32 = 0xA2C2A;
/// `FTSTATUS_CHARACTERS_DEMO(status_id)` is `0x20000 + status_id`.
const STATUS_DEMO: u32 = 0x20000;
/// `nFTDemoStatusNull`: `nFTDemoStatusWin1` to `...Lose` follow.
const DEMO_STATUS_NULL: u32 = 0x10000;
/// `nFTCommonStatusSpecialStart`: the status IDs from here are a fighter's
/// own.
const SPECIAL_START: u32 = 220;

/// The common statuses the Characters scene plays.
const COMMON_STATUSES: [Status; 39] = [
    Status::Wait,
    Status::WalkSlow,
    Status::WalkMiddle,
    Status::WalkFast,
    Status::Dash,
    Status::Run,
    Status::RunBrake,
    Status::JumpF,
    Status::JumpB,
    Status::JumpAerialF,
    Status::JumpAerialB,
    Status::Fall,
    Status::FallAerial,
    Status::LandingLight,
    Status::Squat,
    Status::SquatWait,
    Status::SquatRv,
    Status::Ottotto,
    Status::OttottoWait,
    Status::FuraFura,
    Status::DamageE1,
    Status::EscapeF,
    Status::Attack11,
    Status::Attack12,
    Status::AttackDash,
    Status::AttackS3,
    Status::AttackHi3,
    Status::AttackLw3,
    Status::AttackS4,
    Status::AttackHi4,
    Status::AttackLw4,
    Status::AttackAirN,
    Status::AttackAirF,
    Status::AttackAirB,
    Status::AttackAirHi,
    Status::AttackAirLw,
    Status::ThrowF,
    Status::ThrowB,
    Status::Appeal,
];

/// The fighter-specific statuses the Characters scene plays, by `FTKind`
/// (Luigi's are Mario's: same IDs).
fn fighter_statuses(fk: usize) -> &'static [AnyStatus] {
    match fk {
        0 | 4 => &[
            S::Mario(MarioStatus::SpecialHi),
            S::Mario(MarioStatus::SpecialN),
            S::Mario(MarioStatus::SpecialLw),
            S::Mario(MarioStatus::Attack13),
        ],
        1 => &[
            S::Fox(FoxStatus::SpecialHiStart),
            S::Fox(FoxStatus::SpecialHiHold),
            S::Fox(FoxStatus::SpecialHi),
            S::Fox(FoxStatus::SpecialHiEnd),
            S::Fox(FoxStatus::SpecialN),
            S::Fox(FoxStatus::SpecialLwStart),
            S::Fox(FoxStatus::SpecialLwLoop),
            S::Fox(FoxStatus::SpecialLwEnd),
            S::Fox(FoxStatus::Attack100Start),
            S::Fox(FoxStatus::Attack100Loop),
            S::Fox(FoxStatus::Attack100End),
        ],
        2 => &[
            S::Donkey(DonkeyStatus::SpecialHi),
            S::Donkey(DonkeyStatus::SpecialNStart),
            S::Donkey(DonkeyStatus::SpecialNLoop),
            S::Donkey(DonkeyStatus::SpecialNEnd),
            S::Donkey(DonkeyStatus::SpecialLwStart),
            S::Donkey(DonkeyStatus::SpecialLwLoop),
            S::Donkey(DonkeyStatus::SpecialLwEnd),
        ],
        3 => &[
            S::Samus(SamusStatus::SpecialHi),
            S::Samus(SamusStatus::SpecialNStart),
            S::Samus(SamusStatus::SpecialNLoop),
            S::Samus(SamusStatus::SpecialNEnd),
            S::Samus(SamusStatus::SpecialLw),
        ],
        5 => &[
            S::Link(LinkStatus::SpecialHi),
            S::Link(LinkStatus::SpecialHiEnd),
            S::Link(LinkStatus::SpecialN),
            S::Link(LinkStatus::SpecialLw),
            S::Link(LinkStatus::Attack100Start),
            S::Link(LinkStatus::Attack100Loop),
            S::Link(LinkStatus::Attack100End),
        ],
        6 => &[
            S::Yoshi(YoshiStatus::SpecialHi),
            S::Yoshi(YoshiStatus::SpecialN),
            S::Yoshi(YoshiStatus::SpecialLwStart),
            S::Yoshi(YoshiStatus::SpecialAirLwLoop),
            S::Yoshi(YoshiStatus::SpecialLwLanding),
        ],
        7 => &[
            S::Captain(CaptainStatus::SpecialHi),
            S::Captain(CaptainStatus::SpecialN),
            S::Captain(CaptainStatus::SpecialLw),
            S::Captain(CaptainStatus::Attack13),
            S::Captain(CaptainStatus::Attack100Start),
            S::Captain(CaptainStatus::Attack100Loop),
            S::Captain(CaptainStatus::Attack100End),
        ],
        8 => &[
            S::Kirby(KirbyStatus::SpecialHi),
            S::Kirby(KirbyStatus::SpecialAirHiFall),
            S::Kirby(KirbyStatus::SpecialHiLanding),
            S::Kirby(KirbyStatus::SpecialNStart),
            S::Kirby(KirbyStatus::SpecialNLoop),
            S::Kirby(KirbyStatus::SpecialNEnd),
            S::Kirby(KirbyStatus::SpecialLwStart),
            S::Kirby(KirbyStatus::SpecialLwHold),
            S::Kirby(KirbyStatus::SpecialLwEnd),
            S::Kirby(KirbyStatus::Attack100Start),
            S::Kirby(KirbyStatus::Attack100Loop),
            S::Kirby(KirbyStatus::Attack100End),
            S::Kirby(KirbyStatus::JumpAerialF1),
        ],
        9 => &[
            S::Pikachu(PikachuStatus::SpecialHiEnd),
            S::Pikachu(PikachuStatus::SpecialN),
            S::Pikachu(PikachuStatus::SpecialLwStart),
            S::Pikachu(PikachuStatus::SpecialLwLoop),
            S::Pikachu(PikachuStatus::SpecialLwHit),
            S::Pikachu(PikachuStatus::SpecialLwEnd),
        ],
        10 => &[
            S::Purin(PurinStatus::SpecialHi),
            S::Purin(PurinStatus::SpecialN),
            S::Purin(PurinStatus::SpecialLw),
            S::Purin(PurinStatus::JumpAerialF1),
        ],
        _ => &[
            S::Ness(NessStatus::SpecialHiStart),
            S::Ness(NessStatus::SpecialHiHold),
            S::Ness(NessStatus::SpecialHiJibaku),
            S::Ness(NessStatus::SpecialHiEnd),
            S::Ness(NessStatus::SpecialN),
            S::Ness(NessStatus::SpecialLwStart),
            S::Ness(NessStatus::SpecialLwHold),
            S::Ness(NessStatus::SpecialLwHit),
            S::Ness(NessStatus::SpecialLwEnd),
            S::Ness(NessStatus::Attack13),
        ],
    }
}

/// A ROM `MNCharactersMotion` as a [`Track`], its fighter-specific status
/// IDs read as `fk`'s (`FTKind`).
///
/// An all-zero record (the source leaves Donkey Kong's last Attack1 track
/// and Pikachu's last Lw track undefined, and never reaches them) reads as
/// [`NULL`]. Any other status the port has no name for is a bug in the
/// candidate lists above, except in a stub build, which has no ROM data.
fn track_of(fk: usize, raw: RawTrack) -> Track {
    let length = raw.length;
    let motion = if raw.status == 0 && raw.length == 0 {
        return NULL;
    } else if raw.status == STATUS_NULL {
        Motion::Null
    } else {
        let id = raw.status.wrapping_sub(STATUS_DEMO);
        // The demo statuses are stored as their own IDs, not DEMO()-wrapped.
        let demo = raw.status.wrapping_sub(DEMO_STATUS_NULL);
        const DEMOS: [DemoStatus; 5] = [
            DemoStatus::Win1,
            DemoStatus::Win2,
            DemoStatus::Win3,
            DemoStatus::Win4,
            DemoStatus::Lose,
        ];
        if (1..=DEMOS.len() as u32).contains(&demo) {
            Motion::Demo(DEMOS[demo as usize - 1])
        } else if id < SPECIAL_START {
            match COMMON_STATUSES.iter().find(|s| **s as u32 == id) {
                Some(&s) => Motion::Status(AnyStatus::Common(s)),
                None => unknown(raw),
            }
        } else {
            match fighter_statuses(fk)
                .iter()
                .find(|s| u32::from(s.id()) == id)
            {
                Some(&s) => Motion::Status(s),
                None => unknown(raw),
            }
        }
    };
    Track { motion, length }
}

/// A status the candidate lists lack: an error with the ROM's tables, an
/// empty track with the stub's zeroes.
fn unknown(raw: RawTrack) -> Motion {
    if cfg!(ssb64_stub_tables) {
        return Motion::Null;
    }
    panic!(
        "Characters motion status 0x{:X} has no port name",
        raw.status
    )
}

/// `mnCharactersGetMotion`, from the ROM's tables.
pub fn motion(kind: FighterKind, motion_kind: usize, track: usize) -> Track {
    let fk = (kind as usize).min(11);
    let track = track.min(7);
    if motion_kind <= SPECIAL_LW {
        return track_of(fk, generated::SPECIAL[fk][motion_kind][track]);
    }
    if matches!(kind, FighterKind::Kirby | FighterKind::Purin) {
        if motion_kind == JUMP_AERIAL_F || motion_kind == JUMP_AERIAL_B {
            return if kind == FighterKind::Kirby {
                track_of(fk, generated::KIRBY_JUMP_AERIAL[track])
            } else {
                track_of(fk, generated::PURIN_JUMP_AERIAL[track])
            };
        }
        if (ATTACK_AIR_START..=ATTACK_AIR_END).contains(&motion_kind) && track == 1 {
            return track_of(
                fk,
                if kind == FighterKind::Kirby {
                    generated::KIRBY_FALL
                } else {
                    generated::PURIN_FALL
                },
            );
        }
    }
    if motion_kind == ATTACK1 {
        return track_of(fk, generated::ATTACK1[fk][track]);
    }
    track_of(
        fk,
        generated::COMMON[motion_kind.min(MOTION_KINDS - 1)][track],
    )
}

/// `mnCharactersMakeFighterCamera`: eye (0, 0, 3000) looking at
/// (700, 370, 0), with every camera's projection.
pub const FIGHTER_CAMERA: Camera = Camera {
    eye: Vec3 {
        x: 0.0,
        y: 0.0,
        z: 3000.0,
    },
    at: Vec3 {
        x: 700.0,
        y: 370.0,
        z: 0.0,
    },
    up: Vec3 {
        x: 0.0,
        y: 1.0,
        z: 0.0,
    },
    fovy: 30.0,
    aspect: 4.0 / 3.0,
    near: 100.0,
    far: 12800.0,
    viewport: super::VIEWPORT,
};

/// `mnCharactersMakeEmblemCamera`: eye (0, 0, 1800) on the origin.
pub const EMBLEM_CAMERA: Camera = Camera {
    eye: Vec3 {
        x: 0.0,
        y: 0.0,
        z: 1800.0,
    },
    at: Vec3 {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    },
    ..FIGHTER_CAMERA
};

/// `mnCharactersMakeEmblem`'s DObj: translate, and scale in x and y.
pub const EMBLEM_TRANSLATE: Vec3 = Vec3 {
    x: -350.0,
    y: 200.0,
    z: 0.0,
};
pub const EMBLEM_SCALE: f32 = 1.7;
/// `gcAddMatAnimJointAll(gobj, ..., 4.0F)`: the emblem's colour frame.
pub const EMBLEM_COLOR_FRAME: f32 = 4.0;

/// `mnCharactersFuncStart`'s `scSubsysFighterSetLightParams(45.0F, 10.0F,
/// ...)`.
pub const LIGHT_ANGLE: [f32; 2] = [45.0, 10.0];

/// `mnCharactersSetFighterPosition`.
pub const FIGHTER_POSITION: [f32; 3] = [0.0, -100.0, 0.0];

/// The page's fighter (`sMNCharactersFighterGObj`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Fighter {
    pub kind: FighterKind,
    /// The status `ftMainSetStatus` last set.
    pub motion: Motion,
    /// Bumped on every `ftMainSetStatus` and every new fighter: the clip
    /// starts over from frame 0.
    pub serial: u32,
    /// `DObjGetStruct(fighter_gobj)->rotate.vec.f.y`, radians.
    pub rotate_y: f32,
}

impl Fighter {
    /// `dSCSubsysFighterScales[fkind]`.
    pub fn scale(&self) -> f32 {
        FIGHTER_SCALES[(self.kind as usize).min(11)]
    }
}

/// The menu's state (`sMNCharacters*`).
#[derive(Debug, Clone, PartialEq)]
pub struct CharactersMenu {
    pub page: usize,
    pub fighter: Fighter,
    /// The fighter camera as `mnCharactersMoveFighterCamera` left it.
    pub camera: Camera,
    motion_kind: usize,
    anim_frames_remain: i32,
    is_use_anim_frames_remain: bool,
    is_auto_rotate: bool,
    held_stick_angle: f32,
    is_demo: bool,
    demo_fkinds: [FighterKind; 2],
    current_track: usize,
    fighter_mask: u16,
    /// `sMNCharactersCurrentAnimFrame`: how often the motion has ended.
    current_anim_frame: i32,
    recent: [usize; 3],
    recent_id: usize,
    change_wait: i32,
    total_tics: i32,
}

impl CharactersMenu {
    /// `mnCharactersFuncStart` from Data: the backup's fighter
    /// (`characters_fkind`). `rand` is `syUtilsRandTimeUChar`.
    pub fn new(backup: &Backup, rand: &mut impl FnMut() -> u8) -> CharactersMenu {
        let m = Self::start(backup, false, [backup.characters_fkind; 2], rand);
        // `scene_prev == nSCKindData`; from the attract demo the Explain
        // BGM plays on.
        crate::sound::play_bgm(0, crate::sound::id::nSYAudioBGMData);
        m
    }

    /// `mnCharactersFuncStart` from the title's attract demo:
    /// `gSCManagerSceneData.demo_fkind`.
    pub fn demo(
        backup: &Backup,
        demo_fkinds: [FighterKind; 2],
        rand: &mut impl FnMut() -> u8,
    ) -> CharactersMenu {
        Self::start(backup, true, demo_fkinds, rand)
    }

    fn start(
        backup: &Backup,
        is_demo: bool,
        demo_fkinds: [FighterKind; 2],
        rand: &mut impl FnMut() -> u8,
    ) -> CharactersMenu {
        let page = page_of(if is_demo {
            demo_fkinds[0]
        } else {
            backup.characters_fkind
        });
        let mut m = CharactersMenu {
            page,
            fighter: Fighter {
                kind: PAGES[page],
                motion: Motion::Null,
                serial: 0,
                rotate_y: 0.0,
            },
            camera: FIGHTER_CAMERA,
            motion_kind: 0,
            anim_frames_remain: 0,
            is_use_anim_frames_remain: false,
            is_auto_rotate: true,
            held_stick_angle: 0.0,
            is_demo,
            demo_fkinds,
            current_track: 0,
            fighter_mask: backup.fighter_mask,
            current_anim_frame: 0,
            recent: [MOTION_KINDS; 3],
            recent_id: 0,
            change_wait: 0,
            total_tics: 0,
        };
        m.make_fighter(rand);
        m
    }

    /// The page's fighter kind.
    pub fn kind(&self) -> FighterKind {
        PAGES[self.page]
    }

    /// `mnCharactersCheckHaveFighterKind`.
    fn have(&self, kind: FighterKind) -> bool {
        match kind {
            FighterKind::Luigi | FighterKind::Captain | FighterKind::Purin | FighterKind::Ness => {
                self.fighter_mask & (1 << kind as u16) != 0
            }
            _ => true,
        }
    }

    /// `mnCharactersRandMotionKind`: a kind none of the last three was.
    fn rand_motion_kind(&mut self, rand: &mut impl FnMut() -> u8) -> usize {
        let mut kind;
        loop {
            // `syUtilsRandTimeUCharRange`.
            kind = ((f32::from(rand()) * MOTION_KINDS as f32) / 256.0) as usize;
            if !self.recent.contains(&kind) {
                break;
            }
        }
        self.recent[self.recent_id] = kind;
        self.recent_id = if self.recent_id >= 2 {
            0
        } else {
            self.recent_id + 1
        };
        kind
    }

    /// `mnCharactersSetMotion`: the current track, or a new random motion
    /// once this one has ended.
    fn set_motion(&mut self, rand: &mut impl FnMut() -> u8) -> Track {
        let kind = self.kind();
        let t = motion(kind, self.motion_kind, self.current_track);
        if t.motion != Motion::Null {
            return t;
        }
        self.current_anim_frame += 1;
        if t.length == self.current_anim_frame {
            self.motion_kind = self.rand_motion_kind(rand);
            self.current_anim_frame = 0;
        }
        self.current_track = 0;
        motion(kind, self.motion_kind, 0)
    }

    fn set_status(&mut self, m: Motion) {
        self.fighter.motion = m;
        self.fighter.serial = self.fighter.serial.wrapping_add(1);
    }

    /// `mnCharactersMakeFighter`.
    fn make_fighter(&mut self, rand: &mut impl FnMut() -> u8) {
        self.fighter.kind = self.kind();
        // `ftManagerMakeFighter`: `lr` right, a quarter turn.
        self.fighter.rotate_y = core::f32::consts::FRAC_PI_2;
        self.recent = [MOTION_KINDS; 3];
        self.recent_id = 0;
        self.motion_kind = self.rand_motion_kind(rand);
        self.current_track = 0;
        self.current_anim_frame = 0;
        let t = self.set_motion(rand);
        self.set_status(t.motion);
        if t.length != UNTIL_END {
            self.is_use_anim_frames_remain = true;
            self.anim_frames_remain = t.length;
        } else {
            self.is_use_anim_frames_remain = false;
            self.anim_frames_remain = 0;
        }
        self.is_auto_rotate = true;
    }

    /// `mnCharactersGetMotionKind`: the special the name shows, if any.
    pub fn shown_special(&self) -> Option<usize> {
        (self.motion_kind <= SPECIAL_LW).then_some(self.motion_kind)
    }

    /// `mnCharactersFighterProcUpdate`, after the fighter's own processes
    /// have played its clip: `anim_end` is `fighter_gobj->anim_frame == 0`.
    pub fn tick_fighter(&mut self, anim_end: bool, rand: &mut impl FnMut() -> u8) {
        let kind = self.kind();
        if self.is_use_anim_frames_remain {
            if self.anim_frames_remain != 0 {
                self.anim_frames_remain -= 1;
            } else {
                // `mnCharactersAdvanceTrack`.
                self.current_track += 1;
                let new = self.set_motion(rand);
                let next = motion(kind, self.motion_kind, self.current_track + 1);
                self.set_status(new.motion);
                if new.length != UNTIL_END {
                    self.anim_frames_remain = if next.motion == Motion::Null {
                        new.length + 20
                    } else {
                        new.length
                    };
                    self.is_use_anim_frames_remain = true;
                } else {
                    self.anim_frames_remain = 0;
                    self.is_use_anim_frames_remain = false;
                }
            }
        } else if anim_end {
            let next = motion(kind, self.motion_kind, self.current_track + 1);
            self.anim_frames_remain = if next.motion == Motion::Null { 20 } else { 0 };
            self.is_use_anim_frames_remain = true;
        }
        if self.is_auto_rotate {
            let full = core::f32::consts::TAU;
            self.fighter.rotate_y += core::f32::consts::PI / 360.0;
            if self.fighter.rotate_y > full {
                self.fighter.rotate_y -= full;
            }
        }
    }

    /// `mnCharactersChangeFighter`: the page's emblem, name, story, works
    /// and fighter are remade.
    fn change_fighter(&mut self, rand: &mut impl FnMut() -> u8) {
        self.make_fighter(rand);
    }

    /// `mnCharactersResetFighterCamera`.
    fn reset_camera(&mut self) {
        self.camera = FIGHTER_CAMERA;
        self.held_stick_angle = 0.0;
    }

    /// `mnCharactersMoveFighterCamera`: the camera turned `angle` degrees
    /// about x, at a distance of 3000.
    fn move_camera(&mut self, angle: f32) {
        let r = angle.to_radians();
        let c = &mut self.camera;
        let (sin, cos) = math::sin_cos(r);
        c.eye.y = sin * 3000.0;
        c.eye.z = cos * 3000.0;
        // `syUtilsArcTan2(370.0F, 0.0F)` and `(1.0F, 0.0F)`: a quarter turn.
        let (sin, cos) = math::sin_cos(math::sy_arc_tan2(370.0, 0.0) + r);
        c.at.y = sin * 370.0;
        c.at.z = cos * 370.0;
        let (sin, cos) = math::sin_cos(math::sy_arc_tan2(1.0, 0.0) + r);
        c.up.y = sin;
        c.up.z = cos;
    }

    /// `mnCharactersFuncRun`. Returns the scene it loads; `backup` gets
    /// the page's fighter when B leaves.
    pub fn tick(
        &mut self,
        pad: &Pad,
        backup: &mut Backup,
        rand: &mut impl FnMut() -> u8,
    ) -> Option<Scene> {
        self.total_tics += 1;
        if self.total_tics < 10 {
            return None;
        }
        if self.change_wait != 0 {
            self.change_wait -= 1;
        }
        if pad.released(RIGHT | LEFT) {
            self.change_wait = 0;
        }
        if self.is_demo {
            self.update_demo(pad, rand)
        } else {
            self.update(pad, backup, rand)
        }
    }

    /// `mnCharactersUpdateScene`.
    fn update(
        &mut self,
        pad: &Pad,
        backup: &mut Backup,
        rand: &mut impl FnMut() -> u8,
    ) -> Option<Scene> {
        let mut load = None;
        if pad.tap(N64Buttons::B) {
            // `mnCharactersBackupFighterKind`.
            backup.characters_fkind = self.kind();
            backup.write();
            load = Some(Scene::Data);
        }
        if pad.hold(N64Buttons::Z) {
            let x = f32::from(pad.stick_x);
            let y = f32::from(pad.stick_y);
            let full = core::f32::consts::TAU;
            if pad.stick_x < -20 || pad.stick_x > 20 {
                self.fighter.rotate_y -= (x / 60.0).to_radians();
                if self.fighter.rotate_y > full {
                    self.fighter.rotate_y -= full;
                }
                self.is_auto_rotate = false;
            }
            if pad.stick_y > 20 && self.held_stick_angle < 45.0 {
                self.held_stick_angle += y / 60.0;
                self.move_camera(self.held_stick_angle);
                self.is_auto_rotate = false;
            }
            if pad.stick_y < -20 && self.held_stick_angle > -45.0 {
                self.held_stick_angle += y / 60.0;
                self.move_camera(self.held_stick_angle);
                self.is_auto_rotate = false;
            }
        } else {
            let mut r = Repeat::default();
            if r.check(self.change_wait, pad, LEFT, false, -20, false) {
                loop {
                    self.page = if self.page == 0 { 11 } else { self.page - 1 };
                    if self.have(PAGES[self.page]) {
                        break;
                    }
                }
                self.change_fighter(rand);
                self.change_wait = r.wait_n(7);
                self.reset_camera();
            }
            if r.check(self.change_wait, pad, RIGHT, false, 20, true) {
                loop {
                    self.page = if self.page == 11 { 0 } else { self.page + 1 };
                    if self.have(PAGES[self.page]) {
                        break;
                    }
                }
                self.change_fighter(rand);
                self.change_wait = r.wait_p(7);
                self.reset_camera();
            }
        }
        load
    }

    /// `mnCharactersUpdateSceneDemo`.
    fn update_demo(&mut self, pad: &Pad, rand: &mut impl FnMut() -> u8) -> Option<Scene> {
        let mut load = None;
        if pad.tap(N64Buttons::START | N64Buttons::A | N64Buttons::B) {
            load = Some(Scene::Title);
        }
        if self.total_tics == 300 {
            self.page = page_of(self.demo_fkinds[1]);
            self.change_fighter(rand);
        }
        if self.total_tics == 600 {
            load = Some(Scene::AutoDemo);
        }
        load
    }

    /// The cameras back to front: the emblem (90), the name (80), the story
    /// (70), the decals (60), the works' wallpaper (50), the works and the
    /// motion's name (40), then the fighter (30).
    pub fn visit(&self, f: &mut impl FnMut(Draw)) {
        let kind = self.kind() as usize;
        f(Draw::Emblem);
        // `mnCharactersMakeName`.
        let brown = [0x7D, 0x45, 0x07];
        if matches!(self.kind(), FighterKind::Purin | FighterKind::Captain) {
            f(Draw::Sprite(
                Piece::clear(FILE_CHARACTERS, sprite::NAME_TAG_TALL, 10.0, 44.0).prim(brown),
            ));
        } else {
            f(Draw::Sprite(
                Piece::clear(FILE_CHARACTERS, sprite::NAME_TAG_DEFAULT, 10.0, 45.0).prim(brown),
            ));
        }
        const NAME_AT: [(f32, f32); 12] = [
            (33.0, 50.0),
            (46.0, 51.0),
            (24.0, 51.0),
            (24.0, 51.0),
            (38.0, 50.0),
            (44.0, 49.0),
            (32.0, 49.0),
            (24.0, 48.0),
            (34.0, 49.0),
            (23.0, 50.0),
            (34.0, 49.0),
            (42.0, 52.0),
        ];
        let (x, y) = NAME_AT[kind];
        f(Draw::Sprite(
            Piece::clear(FILE_CHARACTERS, sprite::NAMES[kind], x, y).prim(brown),
        ));
        // `mnCharactersMakeStory`.
        f(Draw::Sprite(
            Piece::clear(FILE_CHARACTERS, sprite::STORY_WALLPAPER, 126.0, 54.0).prim([0; 3]),
        ));
        f(Draw::Sprite(
            Piece::clear(FILE_CHARACTERS, sprite::STORIES[kind], 126.0, 54.0).prim([0xFF; 3]),
        ));
        // `mnCharactersMakeDecals`.
        f(Draw::Sprite(
            Piece::clear(FILE_DATA_COMMON, data_common::DATA_HEADER, 23.0, 17.0)
                .prim([0x5F, 0x58, 0x46]),
        ));
        f(Draw::Sprite(
            Piece::clear(FILE_CHARACTERS, sprite::LABEL, 157.0, 23.0)
                .prim([0xF2, 0xC7, 0x0D])
                .env([0; 3]),
        ));
        let orange = [0xE3, 0x7D, 0x0C];
        f(Draw::Sprite(
            Piece::clear(FILE_DATA_COMMON, data_common::ARROW_L, 257.0, 40.0).prim(orange),
        ));
        f(Draw::Sprite(
            Piece::clear(FILE_DATA_COMMON, data_common::ARROW_R, 275.0, 40.0).prim(orange),
        ));
        // `mnCharactersMakeWorksWallpaper` and `mnCharactersMakeWorks`.
        f(Draw::Sprite(
            Piece::clear(FILE_CHARACTERS, sprite::WORKS_WALLPAPER, 116.0, 173.0)
                .prim([0xCF, 0xCF, 0xAE]),
        ));
        f(Draw::Sprite(
            Piece::clear(FILE_CHARACTERS, sprite::WORKS[kind], 139.0, 180.0)
                .prim([0xBC, 0xBF, 0xFF]),
        ));
        // `mnCharactersUpdateMotionName`.
        if let Some(special) = self.shown_special() {
            f(Draw::Sprite(
                Piece::clear(FILE_CHARACTERS, sprite::MOTION_INPUTS[special], 24.0, 199.0)
                    .prim(orange),
            ));
            f(Draw::Sprite(
                Piece::clear(
                    FILE_CHARACTERS,
                    sprite::MOTION_NAMES[kind][special],
                    24.0,
                    210.0,
                )
                .prim(orange),
            ));
        }
        f(Draw::Fighter);
    }
}

/// [`COMMON_START`] is the first motion kind the name hides.
const _: () = assert!(COMMON_START == 3);
