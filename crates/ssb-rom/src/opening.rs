//! The N64 logo (`mn/mncommon/mnstartup.c`) and the opening movie's
//! assets (`mv/mvopening/*.c`, US). Offsets are from
//! `include/reloc_data.us.h`; no ROM data lives here.
//!
//! The scenes' models (their `DObjDesc` trees and display lists) are in the
//! resident pack like every other graph; what each scene's pack in
//! `ssb64-menus.pak` adds is its sprites, its camera animations baked one
//! play per frame ([`crate::camanim`]) and the whole files its joint
//! animations play from ([`blob_slot`]).

// The source's own float literals, kept digit for digit.
#![allow(clippy::excessive_precision)]

use crate::camanim::CamInit;
use crate::sprite::SpriteFile;

/// `llN64LogoFileID` and `llN64LogoSprite`.
pub const FILE_N64_LOGO: u32 = 0xC2;
pub const N64_LOGO: u32 = 0x73C0;

/// `llIFCommonAnnounceCommonFileID`: the fighter scenes' name letters
/// (resident; [`LETTERS`]).
pub const FILE_ANNOUNCE: u32 = 0x25;

/// The opening's relocData files (`ll*FileID`).
pub mod file {
    pub const MV_COMMON: u32 = 0x34;
    pub const PORTRAITS_SET1: u32 = 0x35;
    pub const PORTRAITS_SET2: u32 = 0x36;
    pub const RUN: u32 = 0x37;
    pub const ROOM_SCENE1: u32 = 0x38;
    pub const ROOM_SCENE2: u32 = 0x39;
    pub const ROOM_SCENE3: u32 = 0x3A;
    pub const ROOM_SCENE4: u32 = 0x3B;
    pub const RUN_MAIN: u32 = 0x3C;
    pub const NEWCOMERS1: u32 = 0x3D;
    pub const NEWCOMERS2: u32 = 0x3E;
    pub const ROOM_TRANSITION: u32 = 0x3F;
    pub const JUNGLE: u32 = 0x40;
    pub const COMMON: u32 = 0x41;
    pub const CLASH_WALLPAPER: u32 = 0x42;
    pub const YOSTER: u32 = 0x43;
    pub const CLIFF: u32 = 0x44;
    pub const STANDOFF: u32 = 0x45;
    pub const STANDOFF_WALLPAPER: u32 = 0x46;
    pub const YAMABUKI: u32 = 0x47;
    pub const CLASH_FIGHTERS: u32 = 0x48;
    pub const SECTOR: u32 = 0x49;
    pub const SECTOR_WALLPAPER: u32 = 0x4A;
    pub const RUN_CRASH: u32 = 0x4B;
    pub const ROOM_WALLPAPER: u32 = 0x5A;
    /// `llStageYoshiFileID`: Yoshi's Island's wallpaper (resident).
    pub const STAGE_YOSHI: u32 = 0x5D;
    /// `llFoxSpecial3FileID`: the entry Arwing's tree (resident).
    pub const FOX_SPECIAL3: u32 = 0xA1;
}

/// The files only the opening reads (`llMVOpeningPortraitsSet1FileID` to
/// `llMVOpeningRunCrashFileID`): their models go to the opening's own pack
/// (`MenuScene::OpeningModels`) rather than the resident one. `MVCommon`
/// (0x34) stays resident, since the ending's room draws from it too.
pub fn is_model_file(id: u32) -> bool {
    (file::PORTRAITS_SET1..=file::RUN_CRASH).contains(&id)
}

/// `llIFCommonAnnounceCommonLetter{A..Z}Sprite`.
pub const LETTERS: [u32; 26] = [
    0x5E0, 0x9A8, 0xD80, 0x1268, 0x1628, 0x1A00, 0x1F08, 0x2408, 0x26B8, 0x2A90, 0x2F98, 0x3358,
    0x3980, 0x3E88, 0x44B0, 0x4890, 0x4F10, 0x5418, 0x57F0, 0x5BD0, 0x60D8, 0x65D8, 0x6C00, 0x7108,
    0x7608, 0x7AE8,
];

/// `llMVCommon*` (file 0x34), the room's models and scripts.
pub mod room {
    pub const BACKGROUND_MOBJSUB: u32 = 0x42F8;
    pub const BACKGROUND: u32 = 0x7E98;
    pub const BACKGROUND_MAT_ANIM_JOINT: u32 = 0x8788;
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
    pub const LOGO_MOBJSUB: u32 = 0x1BC60;
    pub const LOGO: u32 = 0x1C4A8;
    pub const LOGO_MAT_ANIM_JOINT: u32 = 0x1C52C;
    pub const SNAP: u32 = 0x1CA68;
    pub const SNAP_ANIM_JOINT: u32 = 0x1CAF0;
    pub const CLOSE_UP_AIR_MOBJSUB: u32 = 0x1DCA0;
    pub const CLOSE_UP_AIR: u32 = 0x1DF28;
    pub const CLOSE_UP_AIR_MAT_ANIM_JOINT: u32 = 0x1DFD8;
    pub const CLOSE_UP_AIR_ANIM_JOINT: u32 = 0x1E010;
    pub const CLOSE_UP_GROUND_MOBJSUB: u32 = 0x1F0F8;
    pub const CLOSE_UP_GROUND: u32 = 0x1F270;
    pub const CLOSE_UP_GROUND_MAT_ANIM_JOINT: u32 = 0x1F2F4;
    pub const CLOSE_UP_GROUND_ANIM_JOINT: u32 = 0x1F330;
    pub const BOSS_SHADOW_DL: u32 = 0x1F790;
    pub const BOSS_SHADOW_ANIM_JOINT: u32 = 0x1F924;
    pub const DESK_GROUND_MOBJSUB: u32 = 0x20480;
    pub const DESK_GROUND: u32 = 0x22440;
    pub const DESK_GROUND_MAT_ANIM_JOINT: u32 = 0x225CC;
    pub const SPOTLIGHT_MOBJSUB: u32 = 0x22C90;
    pub const SPOTLIGHT_DL: u32 = 0x22E18;
    pub const SPOTLIGHT_MAT_ANIM_JOINT: u32 = 0x22F10;
    pub const OUTSIDE_DL: u32 = 0x24200;
    pub const SUNLIGHT_DL: u32 = 0x24708;
    /// `llMVOpeningRoomTransition*` (file 0x3F).
    pub const TRANSITION_OVERLAY_DL: u32 = 0x5A0;
    pub const TRANSITION_OVERLAY_ANIM_JOINT: u32 = 0x714;
    pub const TRANSITION_OUTLINE_DL: u32 = 0xF40;
    pub const TRANSITION_OUTLINE_ANIM_JOINT: u32 = 0x11C4;
    /// `llMVOpeningRoomWallpaperSprite` (file 0x5A, resident).
    pub const WALLPAPER: u32 = 0x26C88;
}

/// `llMVOpeningPortraitsSet{1,2}*Sprite`: Samus, Mario, Fox, Pikachu
/// (set 1) and Link, Kirby, Donkey Kong, Yoshi (set 2), the four rows
/// top to bottom, then set 1's cover.
pub mod portraits {
    pub const ROWS: [u32; 4] = [0x9960, 0x13310, 0x1CCC0, 0x26670];
    pub const COVER: u32 = 0x2B2D0;
}

/// `llMVOpeningRun*` (file 0x37) and `llMVOpeningRunCrash*` (0x4B).
pub mod run {
    /// The eight fighters' proxy `AnimJoint`s, in `mvOpeningRunMakeFighters`'
    /// order: Mario, Fox, Donkey Kong, Samus, Link, Yoshi, Kirby, Pikachu.
    pub const FIGHTER_ANIM_JOINTS: [u32; 8] = [0x4, 0xB4, 0x124, 0x184, 0x224, 0x334, 0x3A4, 0x484];
    pub const WALLPAPER: u32 = 0x58A0;
    pub const CRASH_MOBJSUB: u32 = 0x2AA8;
    pub const CRASH: u32 = 0x35F8;
    pub const CRASH_MAT_ANIM_JOINT: u32 = 0x3700;
}

/// `llMVOpeningNewcomers{1,2}*` (files 0x3D, 0x3E): each newcomer's shown
/// and hidden display lists and its `AnimJoint`.
pub mod newcomers {
    pub const PURIN_SHOW: u32 = 0x5C28;
    pub const PURIN_ANIM_JOINT: u32 = 0x5E44;
    pub const LUIGI_SHOW: u32 = 0x1C838;
    pub const LUIGI_ANIM_JOINT: u32 = 0x1CE94;
    pub const PURIN_HIDDEN: u32 = 0x203A8;
    pub const LUIGI_HIDDEN: u32 = 0x28C28;
    pub const CAPTAIN_SHOW: u32 = 0x1C238;
    pub const CAPTAIN_ANIM_JOINT: u32 = 0x1C9D4;
    pub const NESS_SHOW: u32 = 0x2A448;
    pub const NESS_ANIM_JOINT: u32 = 0x2A864;
    pub const CAPTAIN_HIDDEN: u32 = 0x355C0;
    pub const NESS_HIDDEN: u32 = 0x3BAF8;
}

/// `llMVOpeningCommon*CamAnimJoint` (file 0x41): the fighter scenes' posed
/// fighter cameras, in `FTKind` order of the eight (Mario, Donkey Kong,
/// Samus, Fox, Link, Yoshi, Pikachu, Kirby).
pub mod common {
    pub const MARIO: u32 = 0x0;
    pub const DONKEY: u32 = 0x30;
    pub const SAMUS: u32 = 0x60;
    pub const FOX: u32 = 0x90;
    pub const LINK: u32 = 0xC0;
    pub const YOSHI: u32 = 0xF0;
    pub const PIKACHU: u32 = 0x120;
    pub const KIRBY: u32 = 0x150;
}

/// `llMVOpeningClashWallpaper*` (file 0x42): the four corners' display
/// lists, materials and scripts, lower left, lower right, upper left,
/// upper right.
pub mod clash {
    pub const CAM_ANIM_JOINT: u32 = 0x4AB0;
    /// `(MObjSub, display list, MatAnimJoint, AnimJoint)`.
    pub const CORNERS: [[u32; 4]; 4] = [
        [0x3050, 0x32A8, 0x3458, 0x36D0],
        [0x36E0, 0x3938, 0x3AE8, 0x3D60],
        [0x3D70, 0x3FC8, 0x4180, 0x4400],
        [0x4410, 0x4668, 0x4820, 0x4AA0],
    ];
    /// `llMVOpeningClashFightersCamAnimJoint` (file 0x48).
    pub const FIGHTERS_CAM_ANIM_JOINT: u32 = 0x1440;
}

/// `llMVOpeningYoster*` (file 0x43).
pub mod yoster {
    pub const NEST: u32 = 0x9808;
    pub const GROUND: u32 = 0xB990;
    pub const GROUND_ANIM_JOINT: u32 = 0xBF70;
    pub const CAM_ANIM_JOINT: u32 = 0xC940;
    /// `llStageYoshiSprite` (file 0x5D, resident).
    pub const WALLPAPER: u32 = 0x26C88;
}

/// `llMVOpeningCliff*` (file 0x44).
pub mod cliff {
    pub const HILLS: u32 = 0x37A0;
    pub const OCARINA: u32 = 0x67A0;
    pub const OCARINA_ANIM_JOINT: u32 = 0x6850;
    pub const CAM_ANIM_JOINT: u32 = 0x8910;
}

/// `llMVOpeningStandoff*` (file 0x45) and its wallpaper (0x46).
pub mod standoff {
    pub const GROUND_DL: u32 = 0x1C10;
    pub const LIGHTNING_MOBJSUB: u32 = 0x6140;
    pub const LIGHTNING: u32 = 0x6950;
    pub const LIGHTNING_MAT_ANIM_JOINT: u32 = 0x6BB8;
    pub const LIGHTNING_ANIM_JOINT: u32 = 0x6D60;
    pub const CAM_ANIM_JOINT: u32 = 0x7250;
    /// `llMVOpeningStandoffWallpaperSprite` (file 0x46), which Cliff shares.
    pub const WALLPAPER: u32 = 0xB500;
}

/// `llMVOpeningYamabuki*` (file 0x47).
pub mod yamabuki {
    pub const LEGS: u32 = 0x9548;
    pub const LEGS_ANIM_JOINT: u32 = 0x98C0;
    pub const LEGS_SHADOW: u32 = 0xB2B0;
    pub const LEGS_SHADOW_ANIM_JOINT: u32 = 0xB390;
    pub const MBALL: u32 = 0xC9E0;
    pub const MBALL_ANIM_JOINT: u32 = 0xCAC0;
    pub const CAM_ANIM_JOINT: u32 = 0xD330;
    pub const WALLPAPER: u32 = 0x3EE58;
}

/// `llMVOpeningSector*` (file 0x49) and its wallpaper (0x4A).
pub mod sector {
    pub const GREAT_FOX: u32 = 0xD820;
    pub const GREAT_FOX_ANIM_JOINT: u32 = 0xDA10;
    pub const ARWING_ANIM_JOINTS: [u32; 3] = [0xE110, 0xE910, 0xF1C0];
    pub const CAM_ANIM_JOINT: u32 = 0xF9A0;
    pub const COCKPIT: u32 = 0x3CC90;
    /// `llMVOpeningSectorWallpaperSprite` (file 0x4A).
    pub const WALLPAPER: u32 = 0x26C88;
    /// `llFoxSpecial3EntryArwingDObjDesc` (file 0xA1).
    pub const ARWING: u32 = 0x2C30;
}

/// `llMVOpeningJungleCamAnimJoint` (file 0x40).
pub const JUNGLE_CAM_ANIM_JOINT: u32 = 0x0;
/// `llMVOpeningRunMainCamAnimJoint` (file 0x3C) and the room's four scene
/// cameras' (files 0x38 to 0x3B), each at its file's start.
pub const FILE_START_CAM_ANIM_JOINT: u32 = 0x0;

/// A camera animation one scene plays: its script and the camera's values
/// before it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CamAnim {
    pub file: u32,
    pub offset: u32,
    pub init: CamInit,
}

impl CamAnim {
    const fn new(file: u32, offset: u32) -> CamAnim {
        CamAnim {
            file,
            offset,
            init: CamInit::DEFAULT,
        }
    }

    /// The baked plays' slot in the scene's pack.
    pub const fn slot(&self) -> u32 {
        camera_slot(self.file, self.offset)
    }
}

/// `gmCameraMakeMovieCamera`'s camera before Jungle's script:
/// `dGMCameraPerspDefault`'s 38 degrees and the eye
/// `gmCameraMakeDefaultCamera` puts 10000 out from (0, 300, 0).
pub const MOVIE_CAMERA_INIT: CamInit = CamInit {
    eye: [0.0, 300.0, 10000.0],
    at: [0.0, 300.0, 0.0],
    up_x: 0.0,
    fovy: 38.0,
};

/// `mvOpeningRoomInitScene3Cameras`' camera before its script.
pub const ROOM_SCENE3_INIT: CamInit = CamInit {
    eye: [9.2993, 3880.389404, 4077.981689],
    at: [0.9915789962, 2995.681396, -388.9534302],
    up_x: 0.0,
    fovy: 18.60718727,
};

/// `mvOpeningRoomInitScene4Cameras`' camera before its script.
pub const ROOM_SCENE4_INIT: CamInit = CamInit {
    eye: [-1039.880615, 3199.215576, -1235.168823],
    at: [-1162.40979, 2127.824463, -3853.073242],
    up_x: 0.0,
    fovy: 11.98226547,
};

/// The room's scene cameras (its logo camera plays scene 1's).
pub const ROOM_CAMERAS: [CamAnim; 4] = [
    CamAnim::new(file::ROOM_SCENE1, 0),
    CamAnim::new(file::ROOM_SCENE2, 0),
    CamAnim {
        init: ROOM_SCENE3_INIT,
        ..CamAnim::new(file::ROOM_SCENE3, 0)
    },
    CamAnim {
        init: ROOM_SCENE4_INIT,
        ..CamAnim::new(file::ROOM_SCENE4, 0)
    },
];

/// The fighter scenes' posed fighter cameras, in [`FIGHTER_SCENE_KINDS`]
/// order.
pub const COMMON_CAMERAS: [CamAnim; 8] = [
    CamAnim::new(file::COMMON, common::MARIO),
    CamAnim::new(file::COMMON, common::DONKEY),
    CamAnim::new(file::COMMON, common::SAMUS),
    CamAnim::new(file::COMMON, common::FOX),
    CamAnim::new(file::COMMON, common::LINK),
    CamAnim::new(file::COMMON, common::YOSHI),
    CamAnim::new(file::COMMON, common::PIKACHU),
    CamAnim::new(file::COMMON, common::KIRBY),
];

pub const RUN_CAMERA: CamAnim = CamAnim::new(file::RUN_MAIN, 0);
pub const CLIFF_CAMERA: CamAnim = CamAnim::new(file::CLIFF, cliff::CAM_ANIM_JOINT);
pub const YAMABUKI_CAMERA: CamAnim = CamAnim::new(file::YAMABUKI, yamabuki::CAM_ANIM_JOINT);
pub const JUNGLE_CAMERA: CamAnim = CamAnim {
    init: MOVIE_CAMERA_INIT,
    ..CamAnim::new(file::JUNGLE, JUNGLE_CAM_ANIM_JOINT)
};
pub const YOSTER_CAMERA: CamAnim = CamAnim::new(file::YOSTER, yoster::CAM_ANIM_JOINT);
pub const SECTOR_CAMERA: CamAnim = CamAnim::new(file::SECTOR, sector::CAM_ANIM_JOINT);
pub const STANDOFF_CAMERA: CamAnim = CamAnim::new(file::STANDOFF, standoff::CAM_ANIM_JOINT);
pub const CLASH_FIGHTERS_CAMERA: CamAnim =
    CamAnim::new(file::CLASH_FIGHTERS, clash::FIGHTERS_CAM_ANIM_JOINT);
pub const CLASH_WALLPAPER_CAMERA: CamAnim =
    CamAnim::new(file::CLASH_WALLPAPER, clash::CAM_ANIM_JOINT);

/// A camera's baked plays' slot (`AnimDesc::EFFECT`) in a scene pack.
pub const fn camera_slot(file: u32, offset: u32) -> u32 {
    0x4000_0000 | (file << 20) | offset
}

/// A whole file's slot (`AnimDesc::EFFECT`) in a scene pack: the bytes
/// its joint scripts play from.
pub const fn blob_slot(file: u32) -> u32 {
    0x2000_0000 | file
}

/// The opening graphs' `MatAnimJoint` tables (`gcAddMatAnimJointAll`), by
/// `(file, DObjDesc or display list)`. Their `MObjSub` pairings are
/// romtool's hand-entered table.
pub const MAT_ANIM_JOINTS: [((u32, u32), u32); 13] = [
    (
        (file::MV_COMMON, room::BACKGROUND),
        room::BACKGROUND_MAT_ANIM_JOINT,
    ),
    ((file::MV_COMMON, room::LOGO), room::LOGO_MAT_ANIM_JOINT),
    (
        (file::MV_COMMON, room::CLOSE_UP_AIR),
        room::CLOSE_UP_AIR_MAT_ANIM_JOINT,
    ),
    (
        (file::MV_COMMON, room::CLOSE_UP_GROUND),
        room::CLOSE_UP_GROUND_MAT_ANIM_JOINT,
    ),
    (
        (file::MV_COMMON, room::DESK_GROUND),
        room::DESK_GROUND_MAT_ANIM_JOINT,
    ),
    (
        (file::MV_COMMON, room::SPOTLIGHT_DL),
        room::SPOTLIGHT_MAT_ANIM_JOINT,
    ),
    (
        (file::STANDOFF, standoff::LIGHTNING),
        standoff::LIGHTNING_MAT_ANIM_JOINT,
    ),
    ((file::RUN_CRASH, run::CRASH), run::CRASH_MAT_ANIM_JOINT),
    (
        (file::CLASH_WALLPAPER, clash::CORNERS[0][1]),
        clash::CORNERS[0][2],
    ),
    (
        (file::CLASH_WALLPAPER, clash::CORNERS[1][1]),
        clash::CORNERS[1][2],
    ),
    (
        (file::CLASH_WALLPAPER, clash::CORNERS[2][1]),
        clash::CORNERS[2][2],
    ),
    (
        (file::CLASH_WALLPAPER, clash::CORNERS[3][1]),
        clash::CORNERS[3][2],
    ),
    // `mnTitleMakeSlash`: `llMNTitleSlashDObjDesc`, `...SlashMatAnimJoint`.
    ((crate::title::FILE, 0x28DA8), 0x25F60),
];

/// The single display lists the scenes hang on one `DObj` each
/// (`gcAddDObjForGObj(gobj, dl)`), which romtool packs as one-node graphs:
/// `(file, list, MObjSub or 0)`.
pub const DL_GRAPHS: [(u32, u32, u32); 21] = [
    (file::MV_COMMON, room::HAZE_DL, 0),
    (file::MV_COMMON, room::BOSS_SHADOW_DL, 0),
    (file::MV_COMMON, room::SPOTLIGHT_DL, room::SPOTLIGHT_MOBJSUB),
    (file::MV_COMMON, room::OUTSIDE_DL, 0),
    (file::MV_COMMON, room::SUNLIGHT_DL, 0),
    (file::ROOM_TRANSITION, room::TRANSITION_OVERLAY_DL, 0),
    (file::ROOM_TRANSITION, room::TRANSITION_OUTLINE_DL, 0),
    (file::NEWCOMERS1, newcomers::PURIN_SHOW, 0),
    (file::NEWCOMERS1, newcomers::PURIN_HIDDEN, 0),
    (file::NEWCOMERS1, newcomers::LUIGI_SHOW, 0),
    (file::NEWCOMERS1, newcomers::LUIGI_HIDDEN, 0),
    (file::NEWCOMERS2, newcomers::CAPTAIN_SHOW, 0),
    (file::NEWCOMERS2, newcomers::CAPTAIN_HIDDEN, 0),
    (file::NEWCOMERS2, newcomers::NESS_SHOW, 0),
    (file::NEWCOMERS2, newcomers::NESS_HIDDEN, 0),
    (file::STANDOFF, standoff::GROUND_DL, 0),
    (
        file::CLASH_WALLPAPER,
        clash::CORNERS[0][1],
        clash::CORNERS[0][0],
    ),
    (
        file::CLASH_WALLPAPER,
        clash::CORNERS[1][1],
        clash::CORNERS[1][0],
    ),
    (
        file::CLASH_WALLPAPER,
        clash::CORNERS[2][1],
        clash::CORNERS[2][0],
    ),
    (
        file::CLASH_WALLPAPER,
        clash::CORNERS[3][1],
        clash::CORNERS[3][0],
    ),
    (file::MV_COMMON, room::TISSUES_DL, 0),
];

/// The opening graphs drawn on head 0 of a `func_80017EC0` camera with no
/// render mode of their own, as `(file, DObjDesc or display list)`. Their
/// lists inherit the camera's `G_RM_AA_ZB_OPA_SURF` (`func_8001663C`; the
/// fighters' displays restore it after them), so romtool seeds them with
/// the camera's untouched default rather than the RDP reset's no-depth
/// state: the run's crash covers the fighters behind it, as on the N64.
/// The clash's corners are here too: their camera's display sets
/// `G_RM_AA_OPA_SURF`, but `func_80017DBC`'s `func_8001663C` sets the
/// camera's default over it before they draw. The cliff's hills draw under
/// their own object display's `G_RM_AA_OPA_SURF` (no depth), and the
/// newcomers on head 1, so neither is here.
pub const CAMERA_DEFAULT_GRAPHS: [(u32, u32); 14] = [
    // The room's desk, pencils, lamp, outside and the castle desk: their
    // lists set no render mode under the scene cameras' `func_80017EC0`.
    (file::MV_COMMON, room::DESK),
    (file::MV_COMMON, room::PENCILS),
    (file::MV_COMMON, room::LAMP),
    (file::MV_COMMON, room::OUTSIDE_DL),
    (file::MV_COMMON, room::DESK_GROUND),
    (file::RUN_CRASH, run::CRASH),
    (file::YAMABUKI, yamabuki::LEGS),
    (file::YAMABUKI, yamabuki::MBALL),
    (file::SECTOR, sector::GREAT_FOX),
    (file::STANDOFF, standoff::GROUND_DL),
    (file::CLASH_WALLPAPER, clash::CORNERS[0][1]),
    (file::CLASH_WALLPAPER, clash::CORNERS[1][1]),
    (file::CLASH_WALLPAPER, clash::CORNERS[2][1]),
    (file::CLASH_WALLPAPER, clash::CORNERS[3][1]),
];

/// The newcomers' lists (`gcDrawDObjDLHead1`), which set no render mode and
/// so draw under the camera's head-1 default, `G_RM_AA_ZB_XLU_SURF`
/// (`func_80016338`): their I8 silhouettes' soft alpha blends, with the
/// depth test and no depth write. romtool seeds them so (RE-467).
pub const HEAD1_DL_GRAPHS: [(u32, u32); 10] = [
    // The room's Master Hand shadow and spotlight, drawn on head 1 too.
    (file::MV_COMMON, room::BOSS_SHADOW_DL),
    (file::MV_COMMON, room::SPOTLIGHT_DL),
    (file::NEWCOMERS1, newcomers::PURIN_SHOW),
    (file::NEWCOMERS1, newcomers::PURIN_HIDDEN),
    (file::NEWCOMERS1, newcomers::LUIGI_SHOW),
    (file::NEWCOMERS1, newcomers::LUIGI_HIDDEN),
    (file::NEWCOMERS2, newcomers::CAPTAIN_SHOW),
    (file::NEWCOMERS2, newcomers::CAPTAIN_HIDDEN),
    (file::NEWCOMERS2, newcomers::NESS_SHOW),
    (file::NEWCOMERS2, newcomers::NESS_HIDDEN),
];

/// `mnStartup`'s sprite.
pub const STARTUP_SPRITES: SpriteFile = SpriteFile {
    file: FILE_N64_LOGO,
    offsets: &[N64_LOGO],
};

pub const PORTRAITS_SET1_SPRITES: SpriteFile = SpriteFile {
    file: file::PORTRAITS_SET1,
    offsets: &[
        portraits::ROWS[0],
        portraits::ROWS[1],
        portraits::ROWS[2],
        portraits::ROWS[3],
        portraits::COVER,
    ],
};

pub const PORTRAITS_SET2_SPRITES: SpriteFile = SpriteFile {
    file: file::PORTRAITS_SET2,
    offsets: &portraits::ROWS,
};

pub const RUN_SPRITES: SpriteFile = SpriteFile {
    file: file::RUN,
    offsets: &[run::WALLPAPER],
};

pub const STANDOFF_WALLPAPER_SPRITES: SpriteFile = SpriteFile {
    file: file::STANDOFF_WALLPAPER,
    offsets: &[standoff::WALLPAPER],
};

pub const YAMABUKI_SPRITES: SpriteFile = SpriteFile {
    file: file::YAMABUKI,
    offsets: &[yamabuki::WALLPAPER],
};

pub const SECTOR_SPRITES: SpriteFile = SpriteFile {
    file: file::SECTOR,
    offsets: &[sector::COCKPIT],
};

pub const SECTOR_WALLPAPER_SPRITES: SpriteFile = SpriteFile {
    file: file::SECTOR_WALLPAPER,
    offsets: &[sector::WALLPAPER],
};

/// The eight fighter scenes (`mvOpening{Mario,..}`) in the order their
/// [`COMMON_CAMERAS`] run, as `FTKind` ordinals.
pub const FIGHTER_SCENE_KINDS: [u8; 8] = [0, 2, 3, 1, 5, 6, 9, 8];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slots_do_not_collide() {
        let mut slots: alloc::vec::Vec<u32> = ROOM_CAMERAS
            .iter()
            .chain(&COMMON_CAMERAS)
            .chain(&[
                RUN_CAMERA,
                CLIFF_CAMERA,
                YAMABUKI_CAMERA,
                JUNGLE_CAMERA,
                YOSTER_CAMERA,
                SECTOR_CAMERA,
                STANDOFF_CAMERA,
                CLASH_FIGHTERS_CAMERA,
                CLASH_WALLPAPER_CAMERA,
            ])
            .map(CamAnim::slot)
            .collect();
        let n = slots.len();
        slots.sort_unstable();
        slots.dedup();
        assert_eq!(slots.len(), n);
        assert_ne!(camera_slot(0x34, 0), blob_slot(0x34));
    }
}
