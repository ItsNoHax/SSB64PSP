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

const fn c(s: Status, length: i32) -> Track {
    Track {
        motion: Motion::Status(AnyStatus::Common(s)),
        length,
    }
}

const fn a(s: AnyStatus, length: i32) -> Track {
    Track {
        motion: Motion::Status(s),
        length,
    }
}

const fn d(s: DemoStatus) -> Track {
    Track {
        motion: Motion::Demo(s),
        length: UNTIL_END,
    }
}

const WAIT30: Track = c(Status::Wait, 30);

/// Eight tracks from `tracks`, the rest [`NULL`].
const fn row(tracks: &[Track]) -> [Track; 8] {
    let mut r = [NULL; 8];
    let mut i = 0;
    while i < tracks.len() {
        r[i] = tracks[i];
        i += 1;
    }
    r
}

/// A special that plays once, then waits 30 ticks.
const fn special(s: AnyStatus) -> [Track; 8] {
    row(&[a(s, UNTIL_END), WAIT30])
}

use AnyStatus as S;

/// `dMNCharactersSpecialMotion*`, by `FTKind`: Hi, N and Lw.
static SPECIAL_MOTIONS: [[[Track; 8]; 3]; 12] = [
    // Mario.
    [
        special(S::Mario(MarioStatus::SpecialHi)),
        special(S::Mario(MarioStatus::SpecialN)),
        special(S::Mario(MarioStatus::SpecialLw)),
    ],
    // Fox.
    [
        row(&[
            a(S::Fox(FoxStatus::SpecialHiStart), UNTIL_END),
            a(S::Fox(FoxStatus::SpecialHiHold), 35),
            a(S::Fox(FoxStatus::SpecialHi), 30),
            a(S::Fox(FoxStatus::SpecialHiEnd), UNTIL_END),
            WAIT30,
        ]),
        special(S::Fox(FoxStatus::SpecialN)),
        row(&[
            a(S::Fox(FoxStatus::SpecialLwStart), UNTIL_END),
            a(S::Fox(FoxStatus::SpecialLwLoop), 60),
            a(S::Fox(FoxStatus::SpecialLwEnd), UNTIL_END),
            WAIT30,
        ]),
    ],
    // Donkey Kong.
    [
        special(S::Donkey(DonkeyStatus::SpecialHi)),
        row(&[
            a(S::Donkey(DonkeyStatus::SpecialNStart), UNTIL_END),
            a(S::Donkey(DonkeyStatus::SpecialNLoop), UNTIL_END),
            a(S::Donkey(DonkeyStatus::SpecialNEnd), UNTIL_END),
            WAIT30,
        ]),
        row(&[
            a(S::Donkey(DonkeyStatus::SpecialLwStart), UNTIL_END),
            a(S::Donkey(DonkeyStatus::SpecialLwLoop), UNTIL_END),
            a(S::Donkey(DonkeyStatus::SpecialLwEnd), UNTIL_END),
            WAIT30,
        ]),
    ],
    // Samus.
    [
        special(S::Samus(SamusStatus::SpecialHi)),
        row(&[
            a(S::Samus(SamusStatus::SpecialNStart), UNTIL_END),
            a(S::Samus(SamusStatus::SpecialNLoop), 60),
            a(S::Samus(SamusStatus::SpecialNEnd), UNTIL_END),
            WAIT30,
        ]),
        special(S::Samus(SamusStatus::SpecialLw)),
    ],
    // Luigi: Mario's special statuses (`nFTLuigiStatus*`).
    [
        special(S::Mario(MarioStatus::SpecialHi)),
        special(S::Mario(MarioStatus::SpecialN)),
        special(S::Mario(MarioStatus::SpecialLw)),
    ],
    // Link.
    [
        row(&[
            a(S::Link(LinkStatus::SpecialHi), UNTIL_END),
            a(S::Link(LinkStatus::SpecialHiEnd), UNTIL_END),
            WAIT30,
        ]),
        special(S::Link(LinkStatus::SpecialN)),
        special(S::Link(LinkStatus::SpecialLw)),
    ],
    // Yoshi.
    [
        special(S::Yoshi(YoshiStatus::SpecialHi)),
        special(S::Yoshi(YoshiStatus::SpecialN)),
        row(&[
            a(S::Yoshi(YoshiStatus::SpecialLwStart), UNTIL_END),
            a(S::Yoshi(YoshiStatus::SpecialAirLwLoop), 12),
            a(S::Yoshi(YoshiStatus::SpecialLwLanding), UNTIL_END),
            WAIT30,
        ]),
    ],
    // Captain Falcon.
    [
        special(S::Captain(CaptainStatus::SpecialHi)),
        special(S::Captain(CaptainStatus::SpecialN)),
        special(S::Captain(CaptainStatus::SpecialLw)),
    ],
    // Kirby.
    [
        row(&[
            a(S::Kirby(KirbyStatus::SpecialHi), UNTIL_END),
            a(S::Kirby(KirbyStatus::SpecialAirHiFall), 12),
            a(S::Kirby(KirbyStatus::SpecialHiLanding), UNTIL_END),
            WAIT30,
        ]),
        row(&[
            a(S::Kirby(KirbyStatus::SpecialNStart), UNTIL_END),
            a(S::Kirby(KirbyStatus::SpecialNLoop), 40),
            a(S::Kirby(KirbyStatus::SpecialNEnd), UNTIL_END),
            WAIT30,
        ]),
        row(&[
            a(S::Kirby(KirbyStatus::SpecialLwStart), UNTIL_END),
            a(S::Kirby(KirbyStatus::SpecialLwHold), UNTIL_END),
            a(S::Kirby(KirbyStatus::SpecialLwEnd), UNTIL_END),
            WAIT30,
        ]),
    ],
    // Pikachu. Its Lw's last track is left zero in the source and never
    // reached.
    [
        special(S::Pikachu(PikachuStatus::SpecialHiEnd)),
        special(S::Pikachu(PikachuStatus::SpecialN)),
        row(&[
            a(S::Pikachu(PikachuStatus::SpecialLwStart), UNTIL_END),
            a(S::Pikachu(PikachuStatus::SpecialLwLoop), 60),
            a(S::Pikachu(PikachuStatus::SpecialLwHit), UNTIL_END),
            a(S::Pikachu(PikachuStatus::SpecialLwEnd), UNTIL_END),
            WAIT30,
        ]),
    ],
    // Jigglypuff.
    [
        special(S::Purin(PurinStatus::SpecialHi)),
        special(S::Purin(PurinStatus::SpecialN)),
        special(S::Purin(PurinStatus::SpecialLw)),
    ],
    // Ness: Hold 120 and End 28, which the source comments as swapped.
    [
        row(&[
            a(S::Ness(NessStatus::SpecialHiStart), UNTIL_END),
            a(S::Ness(NessStatus::SpecialHiHold), 120),
            a(S::Ness(NessStatus::SpecialHiJibaku), UNTIL_END),
            a(S::Ness(NessStatus::SpecialHiEnd), 28),
            WAIT30,
        ]),
        special(S::Ness(NessStatus::SpecialN)),
        row(&[
            a(S::Ness(NessStatus::SpecialLwStart), UNTIL_END),
            a(S::Ness(NessStatus::SpecialLwHold), 60),
            a(S::Ness(NessStatus::SpecialLwHit), UNTIL_END),
            a(S::Ness(NessStatus::SpecialLwEnd), UNTIL_END),
            WAIT30,
        ]),
    ],
];

const fn once(s: Status) -> [Track; 8] {
    row(&[c(s, UNTIL_END), WAIT30])
}

const fn aerial(s: Status) -> [Track; 8] {
    row(&[
        c(s, UNTIL_END),
        c(Status::FallAerial, 30),
        c(Status::LandingLight, UNTIL_END),
        WAIT30,
    ])
}

/// `dMNCharactersCommonMotionDescs`, by motion kind (the specials' rows
/// are empty).
static COMMON_MOTIONS: [[Track; 8]; MOTION_KINDS] = [
    [NULL; 8],
    [NULL; 8],
    [NULL; 8],
    row(&[c(Status::WalkSlow, 90), WAIT30]),
    row(&[c(Status::WalkMiddle, 90), WAIT30]),
    row(&[c(Status::WalkFast, 90), WAIT30]),
    row(&[
        c(Status::Dash, 18),
        c(Status::Run, 60),
        c(Status::RunBrake, UNTIL_END),
        WAIT30,
    ]),
    row(&[
        c(Status::JumpF, UNTIL_END),
        c(Status::JumpAerialF, UNTIL_END),
        c(Status::Fall, UNTIL_END),
        c(Status::LandingLight, UNTIL_END),
        WAIT30,
    ]),
    row(&[
        c(Status::JumpB, UNTIL_END),
        c(Status::JumpAerialB, UNTIL_END),
        c(Status::Fall, UNTIL_END),
        c(Status::LandingLight, UNTIL_END),
        WAIT30,
    ]),
    row(&[
        c(Status::JumpAerialF, UNTIL_END),
        c(Status::LandingLight, UNTIL_END),
        WAIT30,
    ]),
    row(&[
        c(Status::JumpAerialB, UNTIL_END),
        c(Status::LandingLight, UNTIL_END),
        WAIT30,
    ]),
    row(&[
        c(Status::Squat, UNTIL_END),
        c(Status::SquatWait, UNTIL_END),
        c(Status::SquatRv, UNTIL_END),
        WAIT30,
    ]),
    row(&[
        c(Status::Ottotto, UNTIL_END),
        c(Status::OttottoWait, UNTIL_END),
        WAIT30,
    ]),
    row(&[c(Status::FuraFura, 90), WAIT30]),
    row(&[c(Status::Wait, UNTIL_END)]),
    row(&[c(Status::DamageE1, 60), WAIT30]),
    once(Status::EscapeF),
    // EscapeB: EscapeF again in the source.
    once(Status::EscapeF),
    row(&[
        c(Status::Attack11, UNTIL_END),
        c(Status::Attack12, UNTIL_END),
        WAIT30,
    ]),
    once(Status::AttackDash),
    once(Status::AttackS3),
    once(Status::AttackHi3),
    row(&[
        c(Status::AttackLw3, UNTIL_END),
        c(Status::SquatRv, UNTIL_END),
        WAIT30,
    ]),
    once(Status::AttackS4),
    once(Status::AttackHi4),
    once(Status::AttackLw4),
    aerial(Status::AttackAirN),
    aerial(Status::AttackAirF),
    aerial(Status::AttackAirB),
    aerial(Status::AttackAirHi),
    aerial(Status::AttackAirLw),
    once(Status::ThrowF),
    once(Status::ThrowB),
    row(&[d(DemoStatus::Win1)]),
    row(&[d(DemoStatus::Win2)]),
    row(&[d(DemoStatus::Win3)]),
    row(&[d(DemoStatus::Win4)]),
    row(&[d(DemoStatus::Lose)]),
    once(Status::Appeal),
];

/// `dMNCharactersAttack1MotionDescs`, by `FTKind`. Donkey Kong's last
/// track is left zero in the source and never reached.
static ATTACK1_MOTIONS: [[Track; 8]; 12] = [
    row(&[
        c(Status::Attack11, UNTIL_END),
        c(Status::Attack12, UNTIL_END),
        a(S::Mario(MarioStatus::Attack13), UNTIL_END),
        WAIT30,
    ]),
    row(&[
        c(Status::Attack11, UNTIL_END),
        c(Status::Attack12, UNTIL_END),
        a(S::Fox(FoxStatus::Attack100Start), UNTIL_END),
        a(S::Fox(FoxStatus::Attack100Loop), UNTIL_END),
        a(S::Fox(FoxStatus::Attack100End), UNTIL_END),
        WAIT30,
    ]),
    row(&[
        c(Status::Attack11, UNTIL_END),
        c(Status::Attack12, UNTIL_END),
        WAIT30,
    ]),
    row(&[
        c(Status::Attack11, UNTIL_END),
        c(Status::Attack12, UNTIL_END),
        WAIT30,
    ]),
    row(&[
        c(Status::Attack11, UNTIL_END),
        c(Status::Attack12, UNTIL_END),
        a(S::Mario(MarioStatus::Attack13), UNTIL_END),
        WAIT30,
    ]),
    row(&[
        c(Status::Attack11, UNTIL_END),
        c(Status::Attack12, UNTIL_END),
        a(S::Link(LinkStatus::Attack100Start), UNTIL_END),
        a(S::Link(LinkStatus::Attack100Loop), UNTIL_END),
        a(S::Link(LinkStatus::Attack100End), UNTIL_END),
        WAIT30,
    ]),
    row(&[
        c(Status::Attack11, UNTIL_END),
        c(Status::Attack12, UNTIL_END),
        WAIT30,
    ]),
    row(&[
        c(Status::Attack11, UNTIL_END),
        c(Status::Attack12, UNTIL_END),
        a(S::Captain(CaptainStatus::Attack13), UNTIL_END),
        a(S::Captain(CaptainStatus::Attack100Start), UNTIL_END),
        a(S::Captain(CaptainStatus::Attack100Loop), UNTIL_END),
        a(S::Captain(CaptainStatus::Attack100End), UNTIL_END),
        WAIT30,
    ]),
    row(&[
        c(Status::Attack11, UNTIL_END),
        c(Status::Attack12, UNTIL_END),
        a(S::Kirby(KirbyStatus::Attack100Start), UNTIL_END),
        a(S::Kirby(KirbyStatus::Attack100Loop), UNTIL_END),
        a(S::Kirby(KirbyStatus::Attack100End), UNTIL_END),
        WAIT30,
    ]),
    row(&[c(Status::Attack11, UNTIL_END), WAIT30]),
    row(&[
        c(Status::Attack11, UNTIL_END),
        c(Status::Attack12, UNTIL_END),
        WAIT30,
    ]),
    row(&[
        c(Status::Attack11, UNTIL_END),
        c(Status::Attack12, UNTIL_END),
        a(S::Ness(NessStatus::Attack13), UNTIL_END),
        WAIT30,
    ]),
];

/// `dMNCharactersKirbyJumpAerialMotionDescs` and the Purin ones.
const KIRBY_JUMP_AERIAL: [Track; 8] =
    row(&[a(S::Kirby(KirbyStatus::JumpAerialF1), UNTIL_END), WAIT30]);
const PURIN_JUMP_AERIAL: [Track; 8] =
    row(&[a(S::Purin(PurinStatus::JumpAerialF1), UNTIL_END), WAIT30]);
/// `dMNCharactersKirbyFallMotionDesc` and `...PurinFallMotionDesc`.
const FLOATY_FALL: Track = c(Status::Fall, 30);

/// `mnCharactersGetMotion`.
pub fn motion(kind: FighterKind, motion_kind: usize, track: usize) -> Track {
    let fk = (kind as usize).min(11);
    let track = track.min(7);
    if motion_kind <= SPECIAL_LW {
        return SPECIAL_MOTIONS[fk][motion_kind][track];
    }
    if matches!(kind, FighterKind::Kirby | FighterKind::Purin) {
        if motion_kind == JUMP_AERIAL_F || motion_kind == JUMP_AERIAL_B {
            return if kind == FighterKind::Kirby {
                KIRBY_JUMP_AERIAL[track]
            } else {
                PURIN_JUMP_AERIAL[track]
            };
        }
        if (ATTACK_AIR_START..=ATTACK_AIR_END).contains(&motion_kind) && track == 1 {
            return FLOATY_FALL;
        }
    }
    if motion_kind == ATTACK1 {
        return ATTACK1_MOTIONS[fk][track];
    }
    COMMON_MOTIONS[motion_kind.min(MOTION_KINDS - 1)][track]
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
        let (sin, cos) = math::sin_cos(math::atan2(370.0, 0.0) + r);
        c.at.y = sin * 370.0;
        c.at.z = cos * 370.0;
        let (sin, cos) = math::sin_cos(math::atan2(1.0, 0.0) + r);
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
