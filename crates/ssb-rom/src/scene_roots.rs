//! Each N64 scene's archive files (RE-475, D-046): the roots a scene's
//! FuncStart loads with `lbRelocLoadFilesListed` and
//! `lbRelocGetExternHeapFile`. Loading a root loads its extern closure
//! ([`crate::pack::FileDesc`]'s dependencies), as `lbRelocLoadAndRelocFile`
//! does.
//!
//! IDs are `ll*FileID` values (`include/reloc_data.us.h`).

/// `dGMCommonFileIDs` (`src/gm/gmcommon.c:11`): the battle interface
/// (`IFCommonPlayer`, `...GameStatus`, `...PlayerDamage`, `...Timer`,
/// `...Digits`, `...BattlePause`, `...PlayerTags`, `...AnnounceCommon`).
pub const GM_COMMON: [u32; 8] = [0xA6, 0x52, 0xA4, 0xA5, 0x24, 0xC5, 0x26, 0x25];
/// `efManagerInitEffects`: `EFCommonEffects1` to `3`.
pub const EF_COMMON: [u32; 3] = [0x53, 0x54, 0x55];
/// `itManagerInitItems`: `ITCommonData` (with `ITCommonObject`) and
/// `IFCommonItem`.
pub const IT_COMMON: [u32; 2] = [0xFB, 0x57];
/// `ftManagerAllocFighter`: `FTManagerCommon`, `FTCommonMoveset`.
pub const FT_MANAGER: [u32; 2] = [0xA3, 0xC9];
/// The wipes' models (`LBTransition*`, `lbTransitionSetupTransition`).
pub const TRANSITIONS: [u32; 11] = [
    0x28, 0x29, 0x2A, 0x2B, 0x2C, 0x2D, 0x2E, 0x33, 0x30, 0x31, 0x32,
];

/// `dFTManagerDataFiles[kind].file_main_id`, in `FTKind` order
/// (`src/ft/ftdata.c:127`): the twelve playable kinds, Master Hand, Metal
/// Mario, the twelve Polygons and Giant Donkey Kong.
pub const FIGHTER_MAIN: [u32; 27] = [
    0xCB, 0xD1, 0xD5, 0xD9, 0xDD, 0xE1, 0xF7, 0xEC, 0xE5, 0xF3, 0xE9, 0xEF, // playable
    0xFA, // Boss
    0xCE, // MMario
    0xCF, 0xD3, 0xD6, 0xDB, 0xDF, 0xE3, 0xF8, 0xED, 0xE7, 0xF5, 0xEA, 0xF1, // Polygons
    0xD7, // GDonkey
];

/// The `FTKind` whose figatrees `kind` plays: Metal Mario and the
/// Polygons reuse their model's, Giant Donkey Kong Donkey Kong's.
pub fn anim_kind(kind: u32) -> u32 {
    match kind {
        13 => 0,
        14..=25 => kind - 14,
        26 => 2,
        k => k,
    }
}

/// `dMPCollisionGroundFileInfos[gkind].file_id`
/// (`src/mp/mpcollision.c:26`): the stage's map file, whose closure holds
/// its models and wallpaper.
pub fn ground_map(gkind: u32) -> Option<u32> {
    match gkind {
        0..=16 => Some(crate::stage::COMMON_GROUND_FILES[gkind as usize]),
        // Break the Targets, then Board the Platforms, Mario to Ness.
        17..=40 => Some(0x10F + (gkind - 17)),
        _ => None,
    }
}

/// `sc1PTrainingModeLoadWallpaper`'s three wallpapers
/// (`GRWallpaperTraining{Black,Yellow,Blue}`) and `SC1PTrainingMode`.
pub const TRAINING: [u32; 4] = [0x1A, 0x1B, 0x1C, 0xFE];
/// Break the Targets' `ITBonus1ObjectHeader` and both bonus stages'
/// `SC1PStageClear3` (`sc1pbonusstage.c`).
pub const BONUS1: [u32; 2] = [0xFD, 0x97];
/// Board the Platforms' `Bonus2Common` and `SC1PStageClear3`.
pub const BONUS2: [u32; 2] = [0x88, 0x97];
/// How to Play's `SCExplainGraphics` and `SCExplainMain`.
pub const EXPLAIN: [u32; 2] = [0xC6, 0xFC];
/// The title demo's `CharacterNames`.
pub const AUTO_DEMO: [u32; 1] = [0x0C];
/// The Polygon stage's `FTStocksZako`.
pub const ZAKO: [u32; 1] = [0x19];
/// The Kirby team's `KirbySpecial1` (every copy's special files).
pub const KIRBY_TEAM: [u32; 1] = [0xE6];

/// The N64 logo's `N64Logo` (`mnstartup.c:232`).
pub const STARTUP: [u32; 1] = [0xC2];
/// `mnTitleLoadFiles`: `MNTitle`, `MNTitleFireAnim`.
pub const TITLE: [u32; 2] = [0xA7, 0xA8];
pub const MODE_SELECT: [u32; 2] = [0x00, 0x01];
pub const ONE_P_MODE: [u32; 2] = [0x00, 0x02];
pub const VS_MODE: [u32; 2] = [0x00, 0x06];
pub const VS_OPTIONS: [u32; 2] = [0x00, 0x07];
pub const ITEM_SWITCH: [u32; 1] = [0x08];
pub const OPTION: [u32; 2] = [0x00, 0x04];
pub const SCREEN_ADJUST: [u32; 1] = [0x0F];
pub const BACKUP_CLEAR: [u32; 3] = [0x00, 0x4D, 0x4E];
pub const DATA: [u32; 2] = [0x00, 0x05];
pub const CHARACTERS: [u32; 4] = [0x10, 0x20, 0x14, 0x23];
pub const VS_RECORD: [u32; 4] = [0x1F, 0x20, 0x13, 0x21];
pub const SOUND_TEST: [u32; 5] = [0xC5, 0xA4, 0x20, 0x00, 0xC4];
pub const MESSAGE: [u32; 2] = [0x00, 0x09];
pub const PLAYERS_VS: [u32; 7] = [0x11, 0x00, 0x14, 0x15, 0x12, 0x13, 0x16];
pub const PLAYERS_1P: [u32; 11] = [
    0x11, 0x14, 0x15, 0x12, 0x13, 0x17, 0x18, 0x19, 0x21, 0x24, 0x16,
];
pub const PLAYERS_TRAINING: [u32; 8] = [0x11, 0x17, 0x00, 0x14, 0x15, 0x12, 0x13, 0x16];
pub const PLAYERS_BONUS: [u32; 11] = PLAYERS_1P;
/// `mnMaps` (`mnmaps.c:19`).
pub const MAPS: [u32; 5] = [0x14, 0x15, 0x1E, 0x21, 0x1A];
/// `mnVSResults` (`mnvsresults.c:72`).
pub const VS_RESULTS: [u32; 8] = [0x22, 0x26, 0x12, 0xA4, 0x23, 0x24, 0x25, 0x19];
/// `sc1PIntro` (`sc1pintro.c:18`).
pub const ONE_P_INTRO: [u32; 4] = [0x0B, 0x0C, 0x0D, 0x0E];
/// `mnPlayers1PGameContinue` (`mn1pcontinue.c:37`).
pub const ONE_P_CONTINUE: [u32; 5] = [0x4F, 0x51, 0x25, 0xA4, 0x50];
/// `sc1PStageClear` (`sc1pstageclear.c:24`).
pub const ONE_P_STAGE_CLEAR: [u32; 7] = [0x50, 0x51, 0xA4, 0xA5, 0x24, 0x97, 0x1A];
/// `sc1PChallenger`.
pub const CHALLENGER: [u32; 1] = [0x0A];
/// `mvEnding`: `MVCommon`, `MVEnding`.
pub const ENDING: [u32; 2] = [0x34, 0x4C];
/// `scStaffroll`.
pub const STAFFROLL: [u32; 1] = [0xC3];

/// `dMNCongraPictures` (`mncongra.c:18`): the bottom and top pictures of
/// each playable kind, in `FTKind` order.
pub const CONGRA: [[u32; 2]; 12] = [
    [0xBA, 0xBB], // Mario
    [0xBE, 0xBF], // Fox
    [0xB8, 0xB9], // Donkey
    [0xB0, 0xB1], // Samus
    [0xBC, 0xBD], // Luigi
    [0xB2, 0xB3], // Link
    [0xAC, 0xAD], // Yoshi
    [0xB6, 0xB7], // Captain
    [0xAA, 0xAB], // Kirby
    [0xAE, 0xAF], // Pikachu
    [0xB4, 0xB5], // Purin
    [0xC0, 0xC1], // Ness
];

/// The opening's scenes' listed files (`mvOpening*FuncStart`), by
/// `ssb_game::opening::Kind` name.
pub const OPENING_ROOM: [u32; 8] = [0x34, 0x3F, 0x38, 0x39, 0x3A, 0x3B, 0x4B, 0x5A];
pub const OPENING_PORTRAITS: [u32; 2] = [0x35, 0x36];
pub const OPENING_FIGHTER: [u32; 2] = [0x25, 0x41];
pub const OPENING_RUN: [u32; 3] = [0x37, 0x3C, 0x4B];
pub const OPENING_YOSTER: [u32; 2] = [0x43, 0x5D];
pub const OPENING_CLIFF: [u32; 2] = [0x44, 0x46];
pub const OPENING_STANDOFF: [u32; 2] = [0x45, 0x46];
pub const OPENING_YAMABUKI: [u32; 1] = [0x47];
pub const OPENING_CLASH: [u32; 2] = [0x48, 0x42];
pub const OPENING_SECTOR: [u32; 3] = [0x49, 0xA1, 0x4A];
pub const OPENING_JUNGLE: [u32; 2] = [0x25, 0x40];
pub const OPENING_NEWCOMERS: [u32; 2] = [0x3D, 0x3E];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ground_maps_follow_the_ground_table() {
        assert_eq!(ground_map(0), Some(0x103));
        assert_eq!(ground_map(16), Some(0x10A));
        assert_eq!(ground_map(17), Some(0x10F));
        assert_eq!(ground_map(40), Some(0x126));
        assert_eq!(ground_map(41), None);
    }

    #[test]
    fn variant_kinds_play_their_models_figatrees() {
        assert_eq!(anim_kind(13), 0);
        assert_eq!(anim_kind(15), 1);
        assert_eq!(anim_kind(26), 2);
        assert_eq!(anim_kind(8), 8);
    }
}
