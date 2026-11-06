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
pub use crate::d_arrays::GM_COMMON_FILE_IDS as GM_COMMON;
/// `efManagerInitEffects`: `EFCommonEffects1` to `3`.
pub const EF_COMMON: [u32; 3] = [0x53, 0x54, 0x55];
/// `itManagerInitItems`: `ITCommonData` (with `ITCommonObject`) and
/// `IFCommonItem`.
pub const IT_COMMON: [u32; 2] = [0xFB, 0x57];
/// `ftManagerAllocFighter`: `FTManagerCommon`, `FTCommonMoveset`.
pub const FT_MANAGER: [u32; 2] = [0xA3, 0xC9];
/// The wipes' models (`LBTransition*`, `lbTransitionSetupTransition`).
pub static TRANSITIONS: ssb_tables::Decoded<[u32; 11]> =
    ssb_tables::Decoded::new(|| crate::transition::ASSETS.map(|a| a.file));

/// `dFTManagerDataFiles[kind].file_main_id`, in `FTKind` order
/// (`src/ft/ftdata.c:127`): the twelve playable kinds, Master Hand, Metal
/// Mario, the twelve Polygons and Giant Donkey Kong.
pub static FIGHTER_MAIN: ssb_tables::Decoded<[u32; 27]> =
    ssb_tables::Decoded::new(|| crate::fighter::FIGHTER_FILES.map(|f| f.file));

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
    (*crate::d_arrays::MP_COLLISION_GROUND_FILE_INFOS_FILE_ID)
        .get(gkind as usize)
        .copied()
}

/// `sc1PTrainingModeLoadWallpaper`'s three wallpapers
/// (`GRWallpaperTraining{Black,Yellow,Blue}`) and `SC1PTrainingMode`.
pub static TRAINING: ssb_tables::Decoded<[u32; 4]> = ssb_tables::Decoded::new(|| {
    let wallpapers = *crate::d_arrays::SC_1P_TRAINING_MODE_WALLPAPER_DESCS_FILE_ID;
    [wallpapers[0], wallpapers[1], wallpapers[2], 0xFE]
});
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
pub use crate::d_arrays::MN_1P_MODE_FILE_IDS as ONE_P_MODE;
pub use crate::d_arrays::MN_BACKUP_CLEAR_FILE_IDS as BACKUP_CLEAR;
pub use crate::d_arrays::MN_CHARACTERS_FILE_IDS as CHARACTERS;
pub use crate::d_arrays::MN_DATA_FILE_IDS as DATA;
/// `mnMaps` (`mnmaps.c:19`).
pub use crate::d_arrays::MN_MAPS_FILE_IDS as MAPS;
pub use crate::d_arrays::MN_MESSAGE_FILE_IDS as MESSAGE;
pub use crate::d_arrays::MN_MODE_SELECT_FILE_IDS as MODE_SELECT;
pub use crate::d_arrays::MN_OPTION_FILE_IDS as OPTION;
pub use crate::d_arrays::MN_PLAYERS_1P_BONUS_FILE_IDS as PLAYERS_BONUS;
pub use crate::d_arrays::MN_PLAYERS_1P_GAME_FILE_IDS as PLAYERS_1P;
pub use crate::d_arrays::MN_PLAYERS_1P_TRAINING_FILE_IDS as PLAYERS_TRAINING;
pub use crate::d_arrays::MN_PLAYERS_VS_FILE_IDS as PLAYERS_VS;
pub use crate::d_arrays::MN_SCREEN_ADJUST_FILE_IDS as SCREEN_ADJUST;
pub use crate::d_arrays::MN_SOUND_TEST_FILE_IDS as SOUND_TEST;
/// `mnTitleLoadFiles`: `MNTitle`, `MNTitleFireAnim`.
pub use crate::d_arrays::MN_TITLE_FILE_IDS as TITLE;
pub use crate::d_arrays::MN_VS_ITEM_SWITCH_FILE_IDS as ITEM_SWITCH;
pub use crate::d_arrays::MN_VS_MODE_FILE_IDS as VS_MODE;
pub use crate::d_arrays::MN_VS_OPTIONS_FILE_IDS as VS_OPTIONS;
pub use crate::d_arrays::MN_VS_RECORD_FILE_IDS as VS_RECORD;
/// The stage model file (`llStage<Name>File2`) each VS stage's preview
/// draws, by `gkind`: the file of the pack stage's layer objects, one file
/// of the map closure `mnMapsLoadMapFile` reads into a model heap each time
/// the cursor lands on the stage (`mnMapsMakePreview`).
pub const STAGE_PREVIEW_MODELS: [u32; 9] = [0x6A, 0x6D, 0x6C, 0x69, 0x71, 0x6F, 0x68, 0x70, 0x6B];
/// The file of each VS stage's `MPGroundData.wallpaper`, by `gkind`, which
/// a VS stage select's preview shows (`mnMapsMakePreviewWallpaper`): the
/// pack's stage wallpaper sprite's source, in the stage's map closure.
pub const STAGE_WALLPAPERS: [u32; 9] = [0x5A, 0x63, 0x5C, 0x59, 0x5F, 0x5D, 0x58, 0x5E, 0x5B];

/// `mnMapsMakePreview`'s files for `gkind`: what the select reads when the
/// scene starts and each time the cursor lands on a stage, into one of its
/// two model heaps (`mnMapsAllocModelHeaps`), the last preview's staying
/// until the next one is made. The stage's model ([`STAGE_PREVIEW_MODELS`])
/// and the wallpaper it shows: the stage's ([`STAGE_WALLPAPERS`]), or in
/// Training the one of `dMNMapsTrainingModeFileInfos`' three (`...Black`,
/// `...Yellow`, `...Blue`) `dMNMapsTrainingModeWallpaperIDs` picks, which the N64
/// reads over the stage's in the same heap. None for the random slot
/// (`gkind` past the nine VS stages), whose picture is in [`MAPS`]'
/// `MNMaps`.
pub fn maps_preview(gkind: u8, training: bool) -> Option<[u32; 2]> {
    let g = usize::from(gkind);
    let model = *STAGE_PREVIEW_MODELS.get(g)?;
    let wallpaper = if training {
        let id = *(*crate::d_arrays::MN_MAPS_TRAINING_MODE_WALLPAPER_IDS).get(g)?;
        *(*crate::d_arrays::MN_MAPS_TRAINING_MODE_FILE_INFOS_FILE_ID)
            .get(usize::try_from(id).ok()?)?
    } else {
        STAGE_WALLPAPERS[g]
    };
    Some([model, wallpaper])
}

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

/// The figatrees a battle holds for each fighter from its start: the
/// movement statuses ([`crate::anim::SLOT_DASH`] to
/// [`crate::anim::SLOT_SQUAT_WAIT`]), which change every few frames. The N64 reads every status's figatree as the
/// status starts (`ftMainSetStatus`, `ftmain.c:4619-4626`); the port reads
/// the others on first use and keeps the recent ones (RE-572 option C,
/// RE-573).
pub const PINNED_CLIP_SLOTS: [usize; 20] = {
    let mut slots = [0; 20];
    let mut i = 0;
    while i < slots.len() {
        slots[i] = crate::anim::SLOT_DASH + i;
        i += 1;
    }
    slots
};

/// Calls `$f` with entries `$i` of the clip table `$t`, each a constant.
macro_rules! each_clip {
    ($f:ident, $t:ident; $($i:literal)*) => {
        $($f(const { $t[$i].0 }, const { $t[$i].1 });)*
    };
}

/// The opening jungle battle's clips beyond [`PINNED_CLIP_SLOTS`] (`FTKind`,
/// slot): every status its scripted Donkey Kong and Samus enter, as PSP
/// read traces of the scene list them (RE-575; `DonkeySpecialNLoop` and
/// `SamusSpecialNEnd` in some runs only). Read with the scene, so no status
/// waits on a read.
pub const JUNGLE_CLIPS: [(u32, usize); 18] = {
    use crate::anim::slot_named as s;
    [
        (2, s("AttackAirF")),
        (2, s("DonkeySpecialNStart")),
        (2, s("DonkeySpecialNLoop")),
        (2, s("DonkeySpecialNEnd")),
        (2, s("DonkeySpecialLwStart")),
        (2, s("DonkeySpecialLwLoop")),
        (2, s("DonkeySpecialLwEnd")),
        (2, s("DamageE1")),
        (2, s("DamageFlyRoll")),
        (3, s("DamageFlyN")),
        (3, s("DamageFlyTop")),
        (3, s("DamageFall")),
        (3, s("DownBounceD")),
        (3, s("DownForwardD")),
        (3, s("GuardOn")),
        (3, s("EscapeF")),
        (3, s("SamusSpecialNStart")),
        (3, s("SamusSpecialNEnd")),
    ]
};

/// Calls `f` with each of [`JUNGLE_CLIPS`], in order, as constants in the
/// code: a loop over the table would add it to `.rodata`.
pub fn jungle_clips(mut f: impl FnMut(u32, usize)) {
    each_clip!(f, JUNGLE_CLIPS; 0 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17);
}

/// Adds [`JUNGLE_CLIPS`]' files to `v` (as [`clip_files`]). No table and
/// nothing to drop, so `.rodata` and `.gcc_except_table` keep their size:
/// growing either moves the PSP's data a cache line (RE-575).
pub fn jungle_files(pack: &crate::pack::Pack<'_>, v: &mut alloc::vec::Vec<u32>) {
    jungle_clips(|kind, slot| push_clip(pack, kind, slot, v));
}

/// The opening fighter scenes' clips beyond [`PINNED_CLIP_SLOTS`]
/// (`FTKind`, slot), each scene's fighter its own kind: the posed
/// fighter's `Stance` (`nFTDemoStatusStance`) and every status the scene's
/// input script makes its fighter enter, as PSP read traces of the scenes
/// list them (RE-575). Read with the scene, so no status waits on a read.
pub const FIGHTER_SCENE_CLIPS: [(u32, usize); 21] = {
    use crate::anim::slot_named as s;
    [
        (0, s("Stance")),
        (0, s("Attack11")),
        (0, s("Attack12")),
        (0, s("AttackLw4")),
        (1, s("Stance")),
        (1, s("FoxSpecialN")),
        (2, s("Stance")),
        (2, s("DonkeySpecialLwStart")),
        (2, s("DonkeySpecialLwLoop")),
        (3, s("Stance")),
        (3, s("GuardOn")),
        (3, s("Catch")),
        (5, s("Stance")),
        (5, s("Appeal")),
        (6, s("Stance")),
        (6, s("GuardOn")),
        (6, s("Catch")),
        (8, s("Stance")),
        (8, s("KirbyJumpAerialF1")),
        (8, s("KirbyAttackAirHi")),
        (9, s("Stance")),
    ]
};

/// Calls `f` with each of [`FIGHTER_SCENE_CLIPS`], in order, as constants
/// in the code (as [`jungle_clips`]).
pub fn fighter_scene_clips(mut f: impl FnMut(u32, usize)) {
    each_clip!(f, FIGHTER_SCENE_CLIPS; 0 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20);
}

/// Adds the clips of the opening fight whose fighters are `kinds` to `v`:
/// [`JUNGLE_CLIPS`]' files for the jungle (two kinds), those of the one
/// kind's [`FIGHTER_SCENE_CLIPS`] for a fighter scene. As
/// [`jungle_files`], no table and nothing to drop; each kind passes
/// `black_box`, or the compares become a jump table in `.rodata`. Not
/// inlined: in its caller, each call gains a landing pad in
/// `.gcc_except_table` (RE-575).
#[inline(never)]
pub fn opening_fight_files(
    pack: &crate::pack::Pack<'_>,
    kinds: &[u32],
    v: &mut alloc::vec::Vec<u32>,
) {
    match *kinds {
        [kind] => fighter_scene_clips(|k, slot| {
            if core::hint::black_box(k) == kind {
                push_clip(pack, k, slot, v);
            }
        }),
        _ => jungle_files(pack, v),
    }
}

fn push_clip(pack: &crate::pack::Pack<'_>, kind: u32, slot: usize, v: &mut alloc::vec::Vec<u32>) {
    if let Some(a) = pack.fighter_anim(kind, slot as u32) {
        if a.script_len != 0 && !v.contains(&a.source_file) {
            v.push(a.source_file);
        }
    }
}

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
/// `mnPlayers1PGameContinue` (`mn1pcontinue.c:37`).
pub use crate::d_arrays::MN_1P_CONTINUE_FILE_IDS as ONE_P_CONTINUE;
/// `mnVSResults` (`mnvsresults.c:72`).
pub use crate::d_arrays::MN_VS_RESULTS_FILE_IDS as VS_RESULTS;
/// `mvEnding`: `MVCommon`, `MVEnding`.
pub use crate::d_arrays::MV_ENDING_FILE_IDS as ENDING;
/// `sc1PChallenger`.
pub use crate::d_arrays::SC_1P_CHALLENGER_FILE_IDS as CHALLENGER;
/// `sc1PIntro` (`sc1pintro.c:18`).
pub use crate::d_arrays::SC_1P_INTRO_FILE_IDS as ONE_P_INTRO;
/// `sc1PStageClear` (`sc1pstageclear.c:24`).
pub use crate::d_arrays::SC_1P_STAGE_CLEAR_FILE_IDS as ONE_P_STAGE_CLEAR;
/// `scStaffroll`.
pub use crate::d_arrays::SC_STAFFROLL_FILE_IDS as STAFFROLL;

/// `dMNCongraPictures` (`mncongra.c:18`): the bottom and top pictures of
/// each playable kind, in `FTKind` order.
pub use crate::ending::CONGRA_FILES as CONGRA;

pub use crate::d_arrays::MV_OPENING_CLASH_FILE_IDS as OPENING_CLASH;
pub use crate::d_arrays::MV_OPENING_CLIFF_FILE_IDS as OPENING_CLIFF;
pub use crate::d_arrays::MV_OPENING_FOX_FILE_IDS as OPENING_FIGHTER;
pub use crate::d_arrays::MV_OPENING_JUNGLE_FILE_IDS as OPENING_JUNGLE;
pub use crate::d_arrays::MV_OPENING_NEWCOMERS_FILE_IDS as OPENING_NEWCOMERS;
pub use crate::d_arrays::MV_OPENING_PORTRAITS_FILE_IDS as OPENING_PORTRAITS;
/// The opening's scenes' listed files (`mvOpening*FuncStart`), by
/// `ssb_game::opening::Kind` name.
pub use crate::d_arrays::MV_OPENING_ROOM_FILE_IDS as OPENING_ROOM;
pub use crate::d_arrays::MV_OPENING_RUN_FILE_IDS as OPENING_RUN;
pub use crate::d_arrays::MV_OPENING_SECTOR_FILE_IDS as OPENING_SECTOR;
pub use crate::d_arrays::MV_OPENING_STANDOFF_FILE_IDS as OPENING_STANDOFF;
pub use crate::d_arrays::MV_OPENING_YAMABUKI_FILE_IDS as OPENING_YAMABUKI;
pub use crate::d_arrays::MV_OPENING_YOSTER_FILE_IDS as OPENING_YOSTER;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opening_clips_name_their_slots() {
        use crate::anim::{slot_named, SLOT_DAMAGE_HI1, SLOT_NAMES};
        assert_eq!(slot_named("DamageHi1"), SLOT_DAMAGE_HI1);
        for (_, slot) in JUNGLE_CLIPS {
            assert_eq!(slot_named(SLOT_NAMES[slot]), slot);
        }
        let mut seen = alloc::vec::Vec::new();
        jungle_clips(|kind, slot| seen.push((kind, slot)));
        assert_eq!(seen, JUNGLE_CLIPS);
        let mut seen = alloc::vec::Vec::new();
        fighter_scene_clips(|kind, slot| seen.push((kind, slot)));
        assert_eq!(seen, FIGHTER_SCENE_CLIPS);
    }

    #[test]
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
    fn ground_maps_follow_the_ground_table() {
        assert_eq!(ground_map(0), Some(0x103));
        assert_eq!(ground_map(16), Some(0x10A));
        assert_eq!(ground_map(17), Some(0x10F));
        assert_eq!(ground_map(40), Some(0x126));
        assert_eq!(ground_map(41), None);
    }

    /// A preview holds its stage's model and one wallpaper: the stage's in
    /// VS, one of Training's three in Training; the random slot none.
    #[test]
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
    fn a_preview_is_its_model_and_one_wallpaper() {
        let training = *crate::d_arrays::MN_MAPS_TRAINING_MODE_FILE_INFOS_FILE_ID;
        // `GRWallpaperTrainingBlack` is also one of the scene's own files.
        assert!(MAPS.contains(&training[0]));
        let mut shown = [false; 3];
        for gkind in 0..9u8 {
            let g = usize::from(gkind);
            assert_eq!(
                maps_preview(gkind, false),
                Some([STAGE_PREVIEW_MODELS[g], STAGE_WALLPAPERS[g]])
            );
            let [model, wallpaper] = maps_preview(gkind, true).unwrap();
            assert_eq!(model, STAGE_PREVIEW_MODELS[g]);
            let k = training.iter().position(|&f| f == wallpaper).unwrap();
            shown[k] = true;
        }
        assert_eq!(shown, [true; 3], "every Training wallpaper is shown");
        assert_eq!(maps_preview(0xDE, false), None);
        assert_eq!(maps_preview(9, true), None);
    }

    #[test]
    fn variant_kinds_play_their_models_figatrees() {
        assert_eq!(anim_kind(13), 0);
        assert_eq!(anim_kind(15), 1);
        assert_eq!(anim_kind(26), 2);
        assert_eq!(anim_kind(8), 8);
    }
}
