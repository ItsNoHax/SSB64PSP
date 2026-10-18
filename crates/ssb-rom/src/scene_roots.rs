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
pub const GM_COMMON: [u32; 8] = crate::d_arrays::GM_COMMON_FILE_IDS;
/// `efManagerInitEffects`: `EFCommonEffects1` to `3`.
pub const EF_COMMON: [u32; 3] = [0x53, 0x54, 0x55];
/// `itManagerInitItems`: `ITCommonData` (with `ITCommonObject`) and
/// `IFCommonItem`.
pub const IT_COMMON: [u32; 2] = [0xFB, 0x57];
/// `ftManagerAllocFighter`: `FTManagerCommon`, `FTCommonMoveset`.
pub const FT_MANAGER: [u32; 2] = [0xA3, 0xC9];
/// The wipes' models (`LBTransition*`, `lbTransitionSetupTransition`).
pub const TRANSITIONS: [u32; 11] = {
    let mut files = [0; 11];
    let mut i = 0;
    while i < 11 {
        files[i] = crate::transition::ASSETS[i].file;
        i += 1;
    }
    files
};

/// `dFTManagerDataFiles[kind].file_main_id`, in `FTKind` order
/// (`src/ft/ftdata.c:127`): the twelve playable kinds, Master Hand, Metal
/// Mario, the twelve Polygons and Giant Donkey Kong.
pub const FIGHTER_MAIN: [u32; 27] = {
    let mut main = [0; 27];
    let mut kind = 0;
    while kind < 27 {
        main[kind] = crate::fighter::FIGHTER_FILES[kind].file;
        kind += 1;
    }
    main
};

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
/// (`src/mp/mpcollision.c:26`), generated from the ROM: the stage's map
/// file, whose closure holds its models and wallpaper.
pub fn ground_map(gkind: u32) -> Option<u32> {
    crate::d_arrays::MP_COLLISION_GROUND_FILE_INFOS_FILE_ID
        .get(gkind as usize)
        .copied()
}

/// `sc1PTrainingModeLoadWallpaper`'s three wallpapers
/// (`GRWallpaperTraining{Black,Yellow,Blue}`) and `SC1PTrainingMode`.
pub const TRAINING: [u32; 4] = {
    let wallpapers = crate::d_arrays::SC_1P_TRAINING_MODE_WALLPAPER_DESCS_FILE_ID;
    [wallpapers[0], wallpapers[1], wallpapers[2], 0xFE]
};
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
pub const TITLE: [u32; 2] = crate::d_arrays::MN_TITLE_FILE_IDS;
pub const MODE_SELECT: [u32; 2] = crate::d_arrays::MN_MODE_SELECT_FILE_IDS;
pub const ONE_P_MODE: [u32; 2] = crate::d_arrays::MN_1P_MODE_FILE_IDS;
pub const VS_MODE: [u32; 2] = crate::d_arrays::MN_VS_MODE_FILE_IDS;
pub const VS_OPTIONS: [u32; 2] = crate::d_arrays::MN_VS_OPTIONS_FILE_IDS;
pub const ITEM_SWITCH: [u32; 1] = crate::d_arrays::MN_VS_ITEM_SWITCH_FILE_IDS;
pub const OPTION: [u32; 2] = crate::d_arrays::MN_OPTION_FILE_IDS;
pub const SCREEN_ADJUST: [u32; 1] = crate::d_arrays::MN_SCREEN_ADJUST_FILE_IDS;
pub const BACKUP_CLEAR: [u32; 3] = crate::d_arrays::MN_BACKUP_CLEAR_FILE_IDS;
pub const DATA: [u32; 2] = crate::d_arrays::MN_DATA_FILE_IDS;
pub const CHARACTERS: [u32; 4] = crate::d_arrays::MN_CHARACTERS_FILE_IDS;
pub const VS_RECORD: [u32; 4] = crate::d_arrays::MN_VS_RECORD_FILE_IDS;
pub const SOUND_TEST: [u32; 5] = crate::d_arrays::MN_SOUND_TEST_FILE_IDS;
pub const MESSAGE: [u32; 2] = crate::d_arrays::MN_MESSAGE_FILE_IDS;
pub const PLAYERS_VS: [u32; 7] = crate::d_arrays::MN_PLAYERS_VS_FILE_IDS;
pub const PLAYERS_1P: [u32; 11] = crate::d_arrays::MN_PLAYERS_1P_GAME_FILE_IDS;
pub const PLAYERS_TRAINING: [u32; 8] = crate::d_arrays::MN_PLAYERS_1P_TRAINING_FILE_IDS;
pub const PLAYERS_BONUS: [u32; 11] = crate::d_arrays::MN_PLAYERS_1P_BONUS_FILE_IDS;
/// `mnMaps` (`mnmaps.c:19`).
pub const MAPS: [u32; 5] = crate::d_arrays::MN_MAPS_FILE_IDS;
/// The stage model file (`llStage<Name>File2`) each VS stage's preview
/// draws, by `gkind`: the file of the pack stage's layer objects, one file
/// of the map closure `mnMapsLoadMapFile` reads into a model heap each time
/// the cursor lands on the stage (`mnMapsMakePreview`).
pub const STAGE_PREVIEW_MODELS: [u32; 9] = [0x6A, 0x6D, 0x6C, 0x69, 0x71, 0x6F, 0x68, 0x70, 0x6B];
/// The file of each VS stage's `MPGroundData.wallpaper`, by `gkind`, which
/// a VS stage select's preview shows (`mnMapsMakePreviewWallpaper`): the
/// pack's stage wallpaper sprite's source, in the stage's map closure.
pub const STAGE_WALLPAPERS: [u32; 9] = [0x5A, 0x63, 0x5C, 0x59, 0x5F, 0x5D, 0x58, 0x5E, 0x5B];
/// The Training stage select's preview wallpapers past [`MAPS`]' black one
/// (`dMNMapsTrainingModeFileInfos`): `GRWallpaperTrainingYellow`, `...Blue`.
pub const MAPS_TRAINING_WALLPAPERS: [u32; 2] = {
    let files = crate::d_arrays::MN_MAPS_TRAINING_MODE_FILE_INFOS_FILE_ID;
    [files[1], files[2]]
};

/// The demo clips a character select poses its fighters with: `Wait`
/// before a pick, `Win1` to `Win4` after (`mnPlayersVSGetStatusSelected`
/// and the 1P selects').
pub const SELECT_CLIP_SLOTS: [usize; 5] = [
    crate::anim::SLOT_WAIT,
    crate::anim::SLOT_WIN1,
    crate::anim::SLOT_WIN2,
    crate::anim::SLOT_WIN3,
    crate::anim::SLOT_WIN4,
];
/// The demo clips the VS results screen poses its fighters with
/// (`mnVSResultsMakeFighter`): `Win1` to `Win4` and `Lose`.
pub const RESULTS_CLIP_SLOTS: [usize; 5] = [
    crate::anim::SLOT_WIN1,
    crate::anim::SLOT_WIN2,
    crate::anim::SLOT_WIN3,
    crate::anim::SLOT_WIN4,
    crate::anim::SLOT_LOSE,
];
/// The 1P continue's clips (`mnPlayers1PGameContinue`): the dropped
/// figure, then its standing pose.
pub const CONTINUE_CLIP_SLOTS: [usize; 2] = [
    crate::anim::SLOT_FIGURE_DROPPED,
    crate::anim::SLOT_FIGURE_STAND,
];
/// The ending's clips (`mvEnding`): the dropped figure, then `Wait`.
pub const ENDING_CLIP_SLOTS: [usize; 2] =
    [crate::anim::SLOT_FIGURE_DROPPED, crate::anim::SLOT_WAIT];

/// The archive files of `kinds`' clips in `slots` (the figatrees a scene's
/// demo poses play), without the rest of each kind's set: the N64 reads
/// each into the fighter's figatree heap as the status starts.
pub fn clip_files(
    pack: &crate::pack::Pack<'_>,
    kinds: impl IntoIterator<Item = u32>,
    slots: &[usize],
) -> alloc::vec::Vec<u32> {
    let mut v: alloc::vec::Vec<u32> = alloc::vec::Vec::new();
    for kind in kinds {
        for &slot in slots {
            if let Some(a) = pack.fighter_anim(kind, slot as u32) {
                if a.script_len != 0 && !v.contains(&a.source_file) {
                    v.push(a.source_file);
                }
            }
        }
    }
    v
}
/// `mnVSResults` (`mnvsresults.c:72`).
pub const VS_RESULTS: [u32; 8] = crate::d_arrays::MN_VS_RESULTS_FILE_IDS;
/// `sc1PIntro` (`sc1pintro.c:18`).
pub const ONE_P_INTRO: [u32; 4] = crate::d_arrays::SC_1P_INTRO_FILE_IDS;
/// `mnPlayers1PGameContinue` (`mn1pcontinue.c:37`).
pub const ONE_P_CONTINUE: [u32; 5] = crate::d_arrays::MN_1P_CONTINUE_FILE_IDS;
/// `sc1PStageClear` (`sc1pstageclear.c:24`).
pub const ONE_P_STAGE_CLEAR: [u32; 7] = crate::d_arrays::SC_1P_STAGE_CLEAR_FILE_IDS;
/// `sc1PChallenger`.
pub const CHALLENGER: [u32; 1] = crate::d_arrays::SC_1P_CHALLENGER_FILE_IDS;
/// `mvEnding`: `MVCommon`, `MVEnding`.
pub const ENDING: [u32; 2] = crate::d_arrays::MV_ENDING_FILE_IDS;
/// `scStaffroll`.
pub const STAFFROLL: [u32; 1] = crate::d_arrays::SC_STAFFROLL_FILE_IDS;

/// `dMNCongraPictures` (`mncongra.c:18`): the bottom and top pictures of
/// each playable kind, in `FTKind` order.
pub const CONGRA: [[u32; 2]; 12] = crate::ending::CONGRA_FILES;

/// The opening's scenes' listed files (`mvOpening*FuncStart`), by
/// `ssb_game::opening::Kind` name.
pub const OPENING_ROOM: [u32; 8] = crate::d_arrays::MV_OPENING_ROOM_FILE_IDS;
pub const OPENING_PORTRAITS: [u32; 2] = crate::d_arrays::MV_OPENING_PORTRAITS_FILE_IDS;
pub const OPENING_FIGHTER: [u32; 2] = crate::d_arrays::MV_OPENING_FOX_FILE_IDS;
pub const OPENING_RUN: [u32; 3] = crate::d_arrays::MV_OPENING_RUN_FILE_IDS;
pub const OPENING_YOSTER: [u32; 2] = crate::d_arrays::MV_OPENING_YOSTER_FILE_IDS;
pub const OPENING_CLIFF: [u32; 2] = crate::d_arrays::MV_OPENING_CLIFF_FILE_IDS;
pub const OPENING_STANDOFF: [u32; 2] = crate::d_arrays::MV_OPENING_STANDOFF_FILE_IDS;
pub const OPENING_YAMABUKI: [u32; 1] = crate::d_arrays::MV_OPENING_YAMABUKI_FILE_IDS;
pub const OPENING_CLASH: [u32; 2] = crate::d_arrays::MV_OPENING_CLASH_FILE_IDS;
pub const OPENING_SECTOR: [u32; 3] = crate::d_arrays::MV_OPENING_SECTOR_FILE_IDS;
pub const OPENING_JUNGLE: [u32; 2] = crate::d_arrays::MV_OPENING_JUNGLE_FILE_IDS;
pub const OPENING_NEWCOMERS: [u32; 2] = crate::d_arrays::MV_OPENING_NEWCOMERS_FILE_IDS;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
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
