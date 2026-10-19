//! Authored US campaign assets. Offsets from reloc_data.us.h; no ROM data. `BONUS_LABELS` and
//! the `sc1PIntro*`/`sc1PStageClear*` function-local tables are read from the ROM at build time
//! (`crate::d_arrays`).
use crate::sprite::SpriteFile;
// `BONUS_LABELS`: `dSC1PStageClearBonusData`'s label offsets in bonus ID
// order (StageClear uses difficulty instead), generated at build time from
// the user's ROM by `crates/ssb-tablegen` (never committed).
include!(concat!(env!("OUT_DIR"), "/campaign_bonus_labels.rs"));
pub const INTRO: SpriteFile = SpriteFile {
    file: 11,
    offsets: &[
        0x1f10, 0x2018, 0x2118, 0x2218, 0x2318, 0x2418, 0x2518, 0x2618, 0x2718, 0x2818, 0x29b8,
        0x2b58, 0x2e38, 0x30f8, 0x3320, 0x3b08, 0x4388, 0x4ac8, 0x5028, 0x50e8, 0x5328, 0x5568,
        0x5748, 0x5988, 0x5c88, 0x5ec8, 0x63f8, 0x6638, 0x6938, 0x69f8, 0x6b18, 0x6c38, 0x71d0,
        0x7320, 0x7470, 0x75c0, 0x7710, 0x7860, 0x79b0, 0x7b00, 0x7c50, 0x7d60, 0x7e70, 0x7f40,
        0xc898, 0xed00, 0x14bf0,
    ],
};
pub const INTRO_VSDECAL: u32 = 0x1f10;
pub const INTRO_NUMBER1: u32 = 0x2018;
pub const INTRO_NUMBER2: u32 = 0x2118;
pub const INTRO_NUMBER3: u32 = 0x2218;
pub const INTRO_NUMBER4: u32 = 0x2318;
pub const INTRO_NUMBER5: u32 = 0x2418;
pub const INTRO_NUMBER6: u32 = 0x2518;
pub const INTRO_NUMBER7: u32 = 0x2618;
pub const INTRO_NUMBER8: u32 = 0x2718;
pub const INTRO_NUMBER9: u32 = 0x2818;
pub const INTRO_NUMBER10: u32 = 0x29b8;
pub const INTRO_CROSS: u32 = 0x2b58;
pub const INTRO_STAGE_TEXT: u32 = 0x2e38;
pub const INTRO_BONUS_TEXT: u32 = 0x30f8;
pub const INTRO_FINAL_TEXT: u32 = 0x3320;
pub const INTRO_BREAK_THE_TARGETS_TEXT: u32 = 0x3b08;
pub const INTRO_BOARD_THE_PLATFORMS_TEXT: u32 = 0x4388;
pub const INTRO_RACE_TO_THE_FINISH_TEXT: u32 = 0x4ac8;
pub const INTRO_0X5028: u32 = 0x5028;
pub const INTRO_DASH: u32 = 0x50e8;
pub const INTRO_METAL_MARIO_TEXT: u32 = 0x5328;
pub const INTRO_MASTER_HAND_TEXT: u32 = 0x5568;
pub const INTRO_GIANT_DKTEXT: u32 = 0x5748;
pub const INTRO_FOX_MC_CLOUD_TEXT: u32 = 0x5988;
pub const INTRO_KIRBY_TEAM_VS8_TEXT: u32 = 0x5c88;
pub const INTRO_MARIO_BROS_TEXT: u32 = 0x5ec8;
pub const INTRO_FIGHTING_POLYGON_TEAM_VS30_TEXT: u32 = 0x63f8;
pub const INTRO_SAMUS_ARAN_TEXT: u32 = 0x6638;
pub const INTRO_YOSHI_TEAM_VS18_TEXT: u32 = 0x6938;
pub const INTRO_VSTEXT: u32 = 0x69f8;
pub const INTRO_ALLY_TEXT: u32 = 0x6b18;
pub const INTRO_ALLY_TEXT2: u32 = 0x6c38;
pub const INTRO_LINK_MARKER: u32 = 0x71d0;
pub const INTRO_YOSHI_MARKER: u32 = 0x7320;
pub const INTRO_FOX_MARKER: u32 = 0x7470;
pub const INTRO_MARIO_BROS_MARKER: u32 = 0x75c0;
pub const INTRO_PIKACHU_MARKER: u32 = 0x7710;
pub const INTRO_DKMARKER: u32 = 0x7860;
pub const INTRO_KIRBY_MARKER: u32 = 0x79b0;
pub const INTRO_SAMUS_MARKER: u32 = 0x7b00;
pub const INTRO_MARIO_MARKER: u32 = 0x7c50;
pub const INTRO_EXCLAMATION_MARK: u32 = 0x7d60;
pub const INTRO_BOSS_MARKER: u32 = 0x7e70;
pub const INTRO_BONUS_MARKER: u32 = 0x7f40;
pub const INTRO_BANNER_TOP: u32 = 0xc898;
pub const INTRO_BANNER_BOTTOM: u32 = 0xed00;
pub const INTRO_SKY: u32 = 0x14bf0;
/// File 12, `SC1PIntroNames`: one name per fighter kind,
/// `sc1PIntroMakeName`'s `name_offsets`.
pub const NAMES: SpriteFile = SpriteFile {
    file: 12,
    offsets: &crate::d_arrays::SC_1P_INTRO_MAKE_NAME_NAME_OFFSETS,
};
/// `sc1PIntroMakeLabels`' `number_offsets`: [`INTRO`]'s stage number by 1P
/// stage; the boss stage's entry is 0 (it draws none).
pub const INTRO_STAGE_NUMBERS: [u32; 14] = crate::d_arrays::SC_1P_INTRO_MAKE_LABELS_NUMBER_OFFSETS;
/// `sc1PIntroMakeFigures`' `figure_offsets`: [`INTRO`]'s map marker by 1P
/// stage.
pub const INTRO_FIGURES: [u32; 15] = crate::d_arrays::SC_1P_INTRO_MAKE_FIGURES_FIGURE_OFFSETS;
/// `sc1PIntroMakeFigures`' `figure_pos`: each marker's `(x, y)`.
pub const INTRO_FIGURE_POS: [[i32; 2]; 14] = crate::d_arrays::SC_1P_INTRO_MAKE_FIGURES_FIGURE_POS;
/// `sc1PIntroMakeVSName`'s `opponent_offsets` by 1P stage: in [`NAMES`] for
/// the Link and Pikachu stages, in [`INTRO`] otherwise; 0 for the bonus
/// stages.
pub const INTRO_OPPONENTS: [u32; 15] = crate::d_arrays::SC_1P_INTRO_MAKE_VS_NAME_OPPONENT_OFFSETS;
pub const NAMES_MARIO: u32 = 0x138;
pub const NAMES_FOX: u32 = 0x258;
pub const NAMES_DONKEY: u32 = 0x378;
pub const NAMES_SAMUS: u32 = 0x4f8;
pub const NAMES_LUIGI: u32 = 0x618;
pub const NAMES_LINK: u32 = 0x738;
pub const NAMES_YOSHI: u32 = 0x858;
pub const NAMES_CAPTAIN: u32 = 0xa38;
pub const NAMES_KIRBY: u32 = 0xbb8;
pub const NAMES_PIKACHU: u32 = 0xd38;
pub const NAMES_PURIN: u32 = 0xf78;
pub const NAMES_NESS: u32 = 0x1098;
pub const PICTURES: SpriteFile = SpriteFile {
    file: 13,
    offsets: &[0xe980, 0x1a658],
};
pub const PICTURES_TARGET: u32 = 0xe980;
pub const PICTURES_RACE: u32 = 0x1a658;
pub const PLATFORM_PICTURE: SpriteFile = SpriteFile {
    file: 14,
    offsets: &[0x27388],
};
pub const PLATFORM_PICTURE_SPRITE: u32 = 0x27388;
pub const CONTINUE: SpriteFile = SpriteFile {
    file: 79,
    offsets: &[0x18f0, 0x1e08, 0x2318, 0x2df8, 0x1e3d8, 0x21900, 0x224f8],
};
pub const CONTINUE_CONTINUE_TEXT: u32 = 0x18f0;
pub const CONTINUE_YES_TEXT: u32 = 0x1e08;
pub const CONTINUE_NO_TEXT: u32 = 0x2318;
pub const CONTINUE_CURSOR: u32 = 0x2df8;
pub const CONTINUE_ROOM: u32 = 0x1e3d8;
pub const CONTINUE_SPOTLIGHT: u32 = 0x21900;
pub const CONTINUE_SHADOW: u32 = 0x224f8;
pub const CLEAR: SpriteFile = SpriteFile {
    file: 80,
    offsets: &[
        0x9d8, 0x1338, 0x1d58, 0x2060, 0x2120, 0x25e8, 0x2b48, 0x3028, 0x30f8, 0x31c8, 0xa4b8,
        0xaf98, 0xb4f8, 0xb6a8, 0xb808, 0xb968, 0xbac8, 0xbc28, 0xbd88, 0xbee8, 0xc048, 0xc1a8,
        0xc308, 0xc468, 0xd1c8, 0xd340, 0xd528, 0xd708, 0xd8e8, 0xdac8, 0xdca8, 0xde88, 0xe068,
        0xe248, 0xe428, 0xe608, 0xe7e8, 0xe9c8, 0xeba8, 0xed88, 0xef68, 0xf148, 0xf328, 0xf508,
        0xf6e8, 0xf8c8, 0xfaa8, 0xfc88, 0xfe68, 0x10048, 0x10228, 0x10408, 0x105e8, 0x107c8,
        0x109a8, 0x10b88, 0x10d68, 0x10f48, 0x11128, 0x11308, 0x114e8, 0x116c8, 0x118a8, 0x11a88,
        0x11c68, 0x11e48, 0x12028, 0x12208, 0x123e8, 0x125c8, 0x127a8, 0x12988, 0x12b68, 0x12d48,
        0x12f28, 0x13108, 0x132e8, 0x134c8, 0x136a8, 0x13888, 0x13a68, 0x13c48, 0x13e28, 0x14008,
        0x141e8, 0x143c8, 0x145a8, 0x14788,
    ],
};
pub const CLEAR_STAGE_TEXT: u32 = 0x9d8;
pub const CLEAR_GAME_TEXT: u32 = 0x1338;
pub const CLEAR_CLEAR_TEXT: u32 = 0x1d58;
pub const CLEAR_SPECIAL_BONUS_TEXT: u32 = 0x2060;
pub const CLEAR_COLON_TEXT: u32 = 0x2120;
pub const CLEAR_TIMER_TEXT: u32 = 0x25e8;
pub const CLEAR_DAMAGE_TEXT: u32 = 0x2b48;
pub const CLEAR_0X3028: u32 = 0x3028;
pub const CLEAR_0X30F8: u32 = 0x30f8;
pub const CLEAR_0X31C8: u32 = 0x31c8;
pub const CLEAR_BONUS_BORDER: u32 = 0xa4b8;
pub const CLEAR_RESULT_TEXT: u32 = 0xaf98;
pub const CLEAR_TARGET_TEXT: u32 = 0xb4f8;
pub const CLEAR_BONUS_PAGE_ARROW: u32 = 0xb6a8;
/// `sc1PStageClearSetDigitSpriteColors`' `colors_all`: the `{prim, env}`
/// pair of each digit kind (timer and damage, bonus stat, score).
pub const CLEAR_DIGIT_COLORS: [[[u8; 3]; 2]; 3] =
    crate::d_arrays::SC_1P_STAGE_CLEAR_SET_DIGIT_SPRITE_COLORS_COLORS_ALL;
/// `sc1PStageClearGetScoreDigitSprite`'s `file_array_ids`: each digit
/// kind's index into the scene's files (`crate::scene_roots::ONE_P_STAGE_CLEAR`).
pub const CLEAR_DIGIT_FILES: [i32; 3] =
    crate::d_arrays::SC_1P_STAGE_CLEAR_GET_SCORE_DIGIT_SPRITE_FILE_ARRAY_IDS;
/// `sc1PStageClearGetScoreDigitSprite`'s `offsets`, `[digit][digit kind]`.
pub const CLEAR_DIGIT_OFFSETS: [[u32; 3]; 10] =
    crate::d_arrays::SC_1P_STAGE_CLEAR_GET_SCORE_DIGIT_SPRITE_OFFSETS;
/// `sc1PStageClearGetAppendBonusStatPoints`' `offsets`: the Stage Clear
/// bonus's text by difficulty.
pub const CLEAR_DIFFICULTY_TEXT: [u32; 5] =
    crate::d_arrays::SC_1P_STAGE_CLEAR_GET_APPEND_BONUS_STAT_POINTS_OFFSETS;
/// `sc1PStageClearGetAppendBonusStatPoints`' `colors`: the No Miss
/// multiplier digits' `{prim, env}`.
pub const CLEAR_NO_MISS_COLORS: [[u8; 3]; 2] =
    crate::d_arrays::SC_1P_STAGE_CLEAR_GET_APPEND_BONUS_STAT_POINTS_COLORS;
pub const CLEAR_TIMER_DAMAGE_DIGIT0: u32 = 0xb808;
pub const CLEAR_TIMER_DAMAGE_DIGIT1: u32 = 0xb968;
pub const CLEAR_TIMER_DAMAGE_DIGIT2: u32 = 0xbac8;
pub const CLEAR_TIMER_DAMAGE_DIGIT3: u32 = 0xbc28;
pub const CLEAR_TIMER_DAMAGE_DIGIT4: u32 = 0xbd88;
pub const CLEAR_TIMER_DAMAGE_DIGIT5: u32 = 0xbee8;
pub const CLEAR_TIMER_DAMAGE_DIGIT6: u32 = 0xc048;
pub const CLEAR_TIMER_DAMAGE_DIGIT7: u32 = 0xc1a8;
pub const CLEAR_TIMER_DAMAGE_DIGIT8: u32 = 0xc308;
pub const CLEAR_TIMER_DAMAGE_DIGIT9: u32 = 0xc468;
pub const CLEAR_TEXT_SHADOW: u32 = 0xd1c8;
pub const CLEAR_BONUS_TEXT: u32 = 0xd340;
pub const CLEAR_CHEAP_SHOT_TEXT: u32 = 0xd528;
pub const CLEAR_STAR_FINISH_TEXT: u32 = 0xd708;
pub const CLEAR_NO_ITEM_TEXT: u32 = 0xd8e8;
pub const CLEAR_SHIELD_BREAKER_TEXT: u32 = 0xdac8;
pub const CLEAR_JUDO_WARRIOR_TEXT: u32 = 0xdca8;
pub const CLEAR_HAWK_TEXT: u32 = 0xde88;
pub const CLEAR_SHOOTER_TEXT: u32 = 0xe068;
pub const CLEAR_HEAVY_DAMAGE_TEXT: u32 = 0xe248;
pub const CLEAR_ALL_VARIATIONS_TEXT: u32 = 0xe428;
pub const CLEAR_ITEM_STRIKE_TEXT: u32 = 0xe608;
pub const CLEAR_DOUBLE_KOTEXT: u32 = 0xe7e8;
pub const CLEAR_TRICKSTER_TEXT: u32 = 0xe9c8;
pub const CLEAR_GIANT_IMPACT_TEXT: u32 = 0xeba8;
pub const CLEAR_SPEEDSTER_TEXT: u32 = 0xed88;
pub const CLEAR_ITEM_THROW_TEXT: u32 = 0xef68;
pub const CLEAR_TRIPLE_KOTEXT: u32 = 0xf148;
pub const CLEAR_LAST_CHANCE_TEXT: u32 = 0xf328;
pub const CLEAR_PACIFIST_TEXT: u32 = 0xf508;
pub const CLEAR_PERFECT_TEXT: u32 = 0xf6e8;
pub const CLEAR_NO_MISS_TEXT: u32 = 0xf8c8;
pub const CLEAR_NO_DAMAGE_TEXT: u32 = 0xfaa8;
pub const CLEAR_FULL_POWER_TEXT: u32 = 0xfc88;
pub const CLEAR_MEW_CATCHER_TEXT: u32 = 0xfe68;
pub const CLEAR_STAR_CLEAR_TEXT: u32 = 0x10048;
pub const CLEAR_VEGETARIAN_TEXT: u32 = 0x10228;
pub const CLEAR_HEART_THROB_TEXT: u32 = 0x10408;
pub const CLEAR_THROW_DOWN_TEXT: u32 = 0x105e8;
pub const CLEAR_SMASH_MANIA_TEXT: u32 = 0x107c8;
pub const CLEAR_SMASHLESS_TEXT: u32 = 0x109a8;
pub const CLEAR_SPECIAL_MOVE_TEXT: u32 = 0x10b88;
pub const CLEAR_SINGLE_MOVE_TEXT: u32 = 0x10d68;
pub const CLEAR_POKEMON_FINISH_TEXT: u32 = 0x10f48;
pub const CLEAR_BOOBY_TRAP_TEXT: u32 = 0x11128;
pub const CLEAR_FIGHTER_STANCE_TEXT: u32 = 0x11308;
pub const CLEAR_MYSTIC_TEXT: u32 = 0x114e8;
pub const CLEAR_COMET_MYSTIC_TEXT: u32 = 0x116c8;
pub const CLEAR_ACID_CLEAR_TEXT: u32 = 0x118a8;
pub const CLEAR_BUMPER_CLEAR_TEXT: u32 = 0x11a88;
pub const CLEAR_TORNADO_CLEAR_TEXT: u32 = 0x11c68;
pub const CLEAR_ARWING_CLEAR_TEXT: u32 = 0x11e48;
pub const CLEAR_COUNTER_ATTACK_TEXT: u32 = 0x12028;
pub const CLEAR_METEOR_SMASH_TEXT: u32 = 0x12208;
pub const CLEAR_AERIAL_TEXT: u32 = 0x123e8;
pub const CLEAR_LAST_SECOND_TEXT: u32 = 0x125c8;
pub const CLEAR_LUCKY3_TEXT: u32 = 0x127a8;
pub const CLEAR_JACKPOT_TEXT: u32 = 0x12988;
pub const CLEAR_YOSHI_RAINBOW_TEXT: u32 = 0x12b68;
pub const CLEAR_KIRBY_RANKS_TEXT: u32 = 0x12d48;
pub const CLEAR_BROS_CALAMITY_TEXT: u32 = 0x12f28;
pub const CLEAR_DKDEFENDER_TEXT: u32 = 0x13108;
pub const CLEAR_DKPERFECT_TEXT: u32 = 0x132e8;
pub const CLEAR_GOOD_FRIEND_TEXT: u32 = 0x134c8;
pub const CLEAR_TRUE_FRIEND_TEXT: u32 = 0x136a8;
pub const CLEAR_NO_MISS_CLEAR_TEXT: u32 = 0x13888;
pub const CLEAR_NO_DAMAGE_CLEAR_TEXT: u32 = 0x13a68;
pub const CLEAR_SPEED_KING_TEXT: u32 = 0x13c48;
pub const CLEAR_SPEED_DEMON_TEXT: u32 = 0x13e28;
pub const CLEAR_VERY_EASY_CLEAR_TEXT: u32 = 0x14008;
pub const CLEAR_EASY_CLEAR_TEXT: u32 = 0x141e8;
pub const CLEAR_NORMAL_CLEAR_TEXT: u32 = 0x143c8;
pub const CLEAR_HARD_CLEAR_TEXT: u32 = 0x145a8;
pub const CLEAR_VERY_HARD_CLEAR_TEXT: u32 = 0x14788;
pub const SCORE: SpriteFile = SpriteFile {
    file: 81,
    offsets: &[0x408],
};
pub const SCORE_SCORE_TEXT: u32 = 0x408;
pub const OBJECTIVES: SpriteFile = SpriteFile {
    file: 151,
    offsets: &[0xc0, 0x1d0],
};
pub const OBJECTIVES_PLATFORM: u32 = 0xc0;
pub const OBJECTIVES_TARGET: u32 = 0x1d0;
// `sc1PIntroGetFighterCObjDesc`'s camera overrides after the initial
// camera play: each kind's six `COBJDesc` eye and at points, generated at
// build time from the user's ROM by `crates/ssb-tablegen`.
include!(concat!(env!("OUT_DIR"), "/campaign_ally_cameras.rs"));
pub const CAMERA_SLOT: u32 = 0xF100;
/// The intro's camera animations in pack order: `sc1PIntroMakeFighterCamera`'s
/// `camanim_joints` by `FTKind`, then
/// `sc1PIntroMakeStageCamera`'s by stage without the bonus stages' null
/// entries.
pub const CAMERA_OFFSETS: [u32; 23] = {
    let fighters = crate::d_arrays::SC_1P_INTRO_MAKE_FIGHTER_CAMERA_CAMANIM_JOINTS;
    let stages = crate::d_arrays::SC_1P_INTRO_MAKE_STAGE_CAMERA_CAMANIM_JOINTS;
    let mut out = [0; 23];
    let mut n = 0;
    while n < fighters.len() {
        out[n] = fighters[n];
        n += 1;
    }
    let mut i = 0;
    while i < stages.len() {
        if stages[i] != 0 {
            out[n] = stages[i];
            n += 1;
        }
        i += 1;
    }
    // Stub tables are all zeros: every stage entry is skipped.
    assert!(n == 23 || n == fighters.len(), "intro camera count");
    out
};
/// Camera scalar commands share StageJoint's encoding. These cameras have
/// no update process: pack the one initial play, before source eye/at overrides.
pub fn initial_camera(data: &[u8], offset: u32) -> Result<[f32; 10], crate::objanim::AnimError> {
    let mut j = crate::objanim::StageJoint::start_changed(offset, 0.0);
    j.tick(data, 1.0, &mut Default::default())?;
    let defaults = [0.0, 0.0, 1500.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 30.0];
    Ok(core::array::from_fn(|i| {
        j.track_value(i).unwrap_or(defaults[i])
    }))
}
/// Native little-endian camera payload in the reserved animation slot.
pub fn packed_camera(pack: &crate::pack::Pack<'_>, index: usize) -> Option<[f32; 10]> {
    let a = pack.effect_anim(CAMERA_SLOT)?;
    let bytes = pack.anim_script(&a)?.get(index * 40..index * 40 + 40)?;
    Some(core::array::from_fn(|i| {
        f32::from_le_bytes(bytes[i * 4..i * 4 + 4].try_into().unwrap())
    }))
}

/// Final Destination's layer file (`StageLastFile2`), which holds the boss
/// camera animations and wallpaper effects.
pub const BOSS_CAMERA_FILE: u32 = 114;
/// `sc1PGameWaitStageBossUpdate`'s camera animation:
/// `gr_desc[1].dobjdesc - llGRLastMapFileHead + D_NF_00006010`, an offset
/// into Final Destination's layer file 114 (`StageLastFile2`).
pub const BOSS_INTRO_CAMERA: u32 = 0x6010;
/// `sc1PGameBossDefeatInterfaceProcSet`'s (`D_NF_00006450`).
pub const BOSS_DEFEAT_CAMERA: u32 = 0x6450;
/// The reserved animation slots their baked frames are packed under.
pub const BOSS_INTRO_CAMERA_SLOT: u32 = 0xF101;
pub const BOSS_DEFEAT_CAMERA_SLOT: u32 = 0xF102;
/// Floats per baked camera frame: eye XYZ, look-at XYZ, field of view.
pub const CAMERA_FRAME_FLOATS: usize = 7;

/// `gcPlayCamAnim` from `gcAddCObjCamAnimJoint` to the end: one
/// `[eye.x, eye.y, eye.z, at.x, at.y, at.z, fovy]` per play, the last
/// being the play its script ends on (`gmCameraAnimFuncCamera` sets the
/// default camera after it). Tracks a script never keys keep the
/// `CObj` defaults of [`initial_camera`]. Path tracks (`EyeI`/`AtI`) are
/// not modelled; neither boss script has one (`tests/boss.rs`).
pub fn camera_frames(
    data: &[u8],
    offset: u32,
) -> Result<alloc::vec::Vec<[f32; CAMERA_FRAME_FLOATS]>, crate::objanim::AnimError> {
    let mut j = crate::objanim::StageJoint::start_changed(offset, 0.0);
    let mut pose = crate::figatree::JointPose::default();
    let defaults = [0.0, 0.0, 1500.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 30.0];
    let mut out = alloc::vec::Vec::new();
    while !j.ended() && out.len() < 4096 {
        j.tick(data, 1.0, &mut pose)?;
        let v = |i: usize| j.track_value(i).unwrap_or(defaults[i]);
        out.push([v(0), v(1), v(2), v(4), v(5), v(6), v(9)]);
    }
    Ok(out)
}

/// A baked boss camera animation's frames from the pack.
pub fn packed_camera_frames<'a>(
    pack: &'a crate::pack::Pack<'a>,
    slot: u32,
) -> Option<impl Iterator<Item = [f32; CAMERA_FRAME_FLOATS]> + 'a> {
    let a = pack.effect_anim(slot)?;
    let bytes = pack.anim_script(&a)?;
    let (frames, _) = bytes.as_chunks::<{ CAMERA_FRAME_FLOATS * 4 }>();
    Some(frames.iter().map(|c| {
        core::array::from_fn(|i| {
            f32::from_le_bytes([c[i * 4], c[i * 4 + 1], c[i * 4 + 2], c[i * 4 + 3]])
        })
    }))
}
