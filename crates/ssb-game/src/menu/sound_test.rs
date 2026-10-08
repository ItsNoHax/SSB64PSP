//! `mn/mndata/mnsoundtest.c`: the Sound Test. Three rows, Music, Sound and
//! Voice, each with a number that picks an entry of its table; A plays it,
//! Z stops everything, START fades the music out and B goes back to Data.
//!
//! The scene's `func_run` (`mnSoundTestFuncRun`) and the threads it adds
//! run in [`SoundTest::tick`], in their link order: the option colours,
//! the input, the play/stop/fade functions, then the arrows' blink. The
//! colour and number threads only copy state into their `SObj`s, so
//! [`SoundTest::visit`] reads that state directly.
//!
//! `mnSoundTestFuncStart` makes no sound call: the Data menu stopped the
//! music when it chose this scene, and the scene leaves the music volume
//! as Data left it until the first tick restores `0x7000`.

use super::{data_common, fill, Draw, Pad, Piece, Repeat, Scene, FILE_COMMON, FILE_DATA_COMMON};
use crate::sound::{self, id::*};
use ssb_engine::input::N64Buttons;

/// `llMNSoundTestFileID`.
pub const FILE_SOUND_TEST: u32 = 0xC4;
/// `llIFCommonPlayerDamageFileID`.
pub const FILE_PLAYER_DAMAGE: u32 = 0xA4;
/// `llIFCommonBattlePauseFileID`.
pub const FILE_BATTLE_PAUSE: u32 = 0xC5;

/// `llMNSoundTest*Sprite` (`reloc_data.us.h`).
pub mod sprite {
    pub const MUSIC_TEXT: u32 = 0x438;
    pub const SOUND_TEXT: u32 = 0x9C0;
    pub const VOICE_TEXT: u32 = 0xE48;
    pub const CAPSULE_RIGHT: u32 = 0x1138;
    pub const COLON_EXIT_TEXT: u32 = 0x1208;
    pub const COLON_FADE_OUT_TEXT: u32 = 0x1348;
    pub const COLON_PLAY_TEXT: u32 = 0x1450;
    pub const SOUND_TEST_TEXT: u32 = 0x1BB8;
    pub const START_BUTTON: u32 = 0x1D50;
    /// `llIFCommonBattlePauseDecalAButtonSprite` and `...BButtonSprite`.
    pub const A_BUTTON: u32 = 0x958;
    pub const B_BUTTON: u32 = 0xA88;
    /// `llMNCommonArrowLSprite` and `...ArrowRSprite`.
    pub const ARROW_L: u32 = super::super::common::ARROW_L;
    pub const ARROW_R: u32 = super::super::common::ARROW_R;
}

/// `dMNSoundTestDigitSpriteOffsets`: `llIFCommonPlayerDamageDigit0Sprite`
/// to `...Digit9Sprite`.
pub const DIGITS: [u32; 10] = [
    0x148, 0x2D8, 0x500, 0x698, 0x8C0, 0xA58, 0xC80, 0xE18, 0x1040, 0x1270,
];

/// `dMNSoundTestDigitSpriteWidths` (the last two are never read).
pub const DIGIT_WIDTHS: [i32; 12] = [14, 9, 15, 14, 15, 13, 15, 14, 15, 15, 17, 20];

/// `dMNSoundTestArrowSpritePositions`: per row, the left arrow's x, both
/// arrows' y, the right arrow's x.
pub const ARROW_POSITIONS: [f32; 9] = [
    162.0, 73.0, 224.0, //
    181.0, 121.0, 243.0, //
    201.0, 168.0, 263.0,
];

/// `nMNSoundTestOption*`.
pub const OPTION_MUSIC: usize = 0;
pub const OPTION_SOUND: usize = 1;
pub const OPTION_VOICE: usize = 2;
/// `nMNSoundTestOptionEnumCount`.
pub const OPTION_COUNT: usize = 3;

/// `sMNSoundTestFadeOutWait`'s "no fade" value.
const NO_FADE: i32 = -1;
/// The music volume the scene holds outside a fade.
const BGM_VOLUME: u32 = 0x7000;
/// START's fade: to silence over 120 tics, then `syAudioStopBGMAll`.
pub const FADE_TICS: i32 = 120;

/// `mnSoundTestUpdateOptionColors`: the highlighted row and the others.
pub const COLOR_ON: [u8; 3] = [0xFF, 0xA8, 0x00];
pub const COLOR_OFF: [u8; 3] = [0x7D, 0x45, 0x07];

/// `mnSoundTestArrowsThreadUpdate`'s blink: 30 tics shown, 30 hidden.
const ARROW_BLINK: i32 = 30;

/// The scene's state (`sMNSoundTest*`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SoundTest {
    /// `sMNSoundTestOption`.
    pub option: usize,
    /// `sMNSoundTestOptionColor{R,G,B}`.
    pub colors: [[u8; 3]; OPTION_COUNT],
    /// `sMNSoundTestOptionChangeWait`.
    pub change_wait: i32,
    /// `sMNSoundTestDirectionInputKind`: 0 none, 1 left, 2 right, 3 up,
    /// 4 down.
    pub direction: i32,
    /// `sMNSoundTestOptionSelectID`.
    pub select_id: [i32; OPTION_COUNT],
    /// `sMNSoundTestSelectIDPositionsX`.
    select_x: [f32; OPTION_COUNT],
    /// `sMNSoundTestFadeOutWait`.
    pub fade_out_wait: i32,
    /// `mnSoundTestArrowsThreadUpdate`'s locals and its GObj's
    /// `GOBJ_FLAG_HIDDEN`.
    arrow_wait: i32,
    arrow_option: usize,
    pub arrows_hidden: bool,
}

impl Default for SoundTest {
    fn default() -> SoundTest {
        SoundTest::new()
    }
}

impl SoundTest {
    /// `mnSoundTestFuncStart` (`mnSoundTestInitVars`).
    pub fn new() -> SoundTest {
        SoundTest {
            option: OPTION_MUSIC,
            colors: [COLOR_OFF; OPTION_COUNT],
            change_wait: 0,
            direction: 0,
            select_id: [0; OPTION_COUNT],
            select_x: [26.5; OPTION_COUNT],
            fade_out_wait: NO_FADE,
            arrow_wait: ARROW_BLINK,
            arrow_option: OPTION_MUSIC,
            arrows_hidden: false,
        }
    }

    /// The entry count of `option`'s table.
    pub fn count(option: usize) -> i32 {
        match option {
            OPTION_MUSIC => MUSIC_IDS.len() as i32,
            OPTION_SOUND => SOUND_IDS.len() as i32,
            _ => VOICE_IDS.len() as i32,
        }
    }

    /// `mnSoundTestFuncRun`, then the arrows' thread. B returns Data; the
    /// frame still finishes.
    pub fn tick(&mut self, pad: &Pad) -> Option<Scene> {
        self.update_option_colors();
        let mut next = None;
        if pad.tap(N64Buttons::B) {
            // `gSCManagerSceneData.scene_curr = nSCKindData`.
            sound::stop_bgm_all();
            sound::stop_all_fgm();
            sound::set_bgm_volume(0, BGM_VOLUME);
            next = Some(Scene::Data);
        }
        self.update_controller_inputs(pad);
        self.update_functions(pad);
        self.update_arrows();
        next
    }

    /// `mnSoundTestUpdateOptionColors`.
    fn update_option_colors(&mut self) {
        for (i, c) in self.colors.iter_mut().enumerate() {
            *c = if i == self.option {
                COLOR_ON
            } else {
                COLOR_OFF
            };
        }
    }

    /// `mnSoundTestUpdateControllerInputs`.
    fn update_controller_inputs(&mut self, pad: &Pad) {
        const UP: u16 = N64Buttons::D_UP | N64Buttons::C_UP;
        const DOWN: u16 = N64Buttons::D_DOWN | N64Buttons::C_DOWN;
        const LEFT: u16 = N64Buttons::D_LEFT | N64Buttons::L | N64Buttons::C_LEFT;
        const RIGHT: u16 = N64Buttons::D_RIGHT | N64Buttons::R | N64Buttons::C_RIGHT;
        if self.change_wait != 0 {
            self.change_wait -= 1;
        }
        if pad.stick_in_range_lr(-32, 32)
            && pad.stick_in_range_ud(-32, 32)
            && !pad.hold(UP | RIGHT)
            && !pad.hold(DOWN | LEFT)
        {
            self.change_wait = 0;
            self.direction = 0;
        }
        // `is_button` and `stick_range` are one pair of locals shared by
        // the four tests.
        let mut r = Repeat::default();
        if r.check(self.change_wait, pad, UP, true, 32, true) {
            sound::play_fgm(nSYAudioFGMMenuScroll2);
            self.change_wait = r.wait_p(8);
            if self.option == OPTION_MUSIC {
                self.option = OPTION_VOICE;
            } else {
                self.option -= 1;
            }
            if self.option == OPTION_MUSIC {
                self.change_wait += 10;
            }
            self.direction = 3;
        }
        if r.check(self.change_wait, pad, DOWN, true, -32, false) {
            sound::play_fgm(nSYAudioFGMMenuScroll2);
            self.change_wait = r.wait_n(8);
            self.option += 1;
            if self.option > OPTION_VOICE {
                self.option = OPTION_MUSIC;
            }
            if self.option == OPTION_VOICE {
                self.change_wait += 10;
            }
            self.direction = 4;
        }
        if r.check(self.change_wait, pad, LEFT, false, -32, false) {
            self.change_wait = r.wait_n(16);
            let last = Self::count(self.option) - 1;
            let id = &mut self.select_id[self.option];
            *id -= 1;
            if *id < 0 {
                *id = last;
            }
            if *id == 0 {
                self.change_wait += 20;
            }
            if self.direction != 1 {
                self.change_wait *= 2;
            }
            self.direction = 1;
        }
        if r.check(self.change_wait, pad, RIGHT, false, 32, true) {
            self.change_wait = r.wait_p(16);
            let last = Self::count(self.option) - 1;
            let id = &mut self.select_id[self.option];
            *id += 1;
            if *id > last {
                *id = 0;
            }
            if *id == last {
                self.change_wait += 20;
            }
            if self.direction != 2 {
                self.change_wait *= 2;
            }
            self.direction = 2;
        }
    }

    /// `mnSoundTestUpdateFunctions`.
    fn update_functions(&mut self, pad: &Pad) {
        if self.fade_out_wait != NO_FADE {
            if self.fade_out_wait != 0 {
                self.fade_out_wait -= 1;
            } else {
                sound::stop_bgm_all();
                self.fade_out_wait = NO_FADE;
            }
        } else {
            sound::set_bgm_volume(0, BGM_VOLUME);
        }
        if pad.tap(N64Buttons::A) {
            let id = self.select_id[self.option] as usize;
            match self.option {
                OPTION_MUSIC => {
                    if self.fade_out_wait > 0 {
                        self.fade_out_wait = NO_FADE;
                    }
                    sound::stop_bgm_all();
                    sound::play_bgm(0, MUSIC_IDS[id]);
                }
                OPTION_SOUND => {
                    sound::stop_all_fgm();
                    sound::play_fgm(SOUND_IDS[id]);
                }
                _ => {
                    sound::stop_all_fgm();
                    sound::play_fgm(VOICE_IDS[id]);
                }
            }
        } else if pad.tap(N64Buttons::Z) {
            sound::stop_bgm_all();
            sound::stop_all_fgm();
        } else if pad.tap(N64Buttons::START) {
            sound::set_bgm_volume_fade(0, 0, FADE_TICS as u32);
            self.fade_out_wait = FADE_TICS;
            sound::stop_all_fgm();
        }
    }

    /// `mnSoundTestArrowsThreadUpdate`: a new row shows the arrows for a
    /// full 30 tics.
    fn update_arrows(&mut self) {
        if self.arrow_option != self.option {
            self.arrow_option = self.option;
            self.arrow_wait = ARROW_BLINK;
            self.arrows_hidden = false;
        }
        if self.arrow_wait == 0 {
            self.arrow_wait = ARROW_BLINK;
            self.arrows_hidden = !self.arrows_hidden;
        }
        self.arrow_wait -= 1;
    }

    /// `mnSoundTestUpdateNumberSprites` and
    /// `mnSoundTestUpdateNumberPositions`: `option`'s number (its
    /// selection plus one) as `(digit, x)`, ones digit first, as the
    /// GObj's `SObj`s hold them. Returns how many digits are shown.
    pub fn number(&self, option: usize) -> ([(usize, f32); 3], usize) {
        let mut digits = [(0usize, 0.0f32); 3];
        let mut count = 0;
        let mut width = 0;
        let mut number = self.select_id[option] + 1;
        loop {
            let d = (number % 10) as usize;
            digits[count].0 = d;
            width += DIGIT_WIDTHS[d];
            count += 1;
            // `number *= 0.1F`.
            number = (number as f32 * 0.1) as i32;
            if number == 0 || count == digits.len() {
                break;
            }
        }
        // Each digit's offset: the widths of the digits left of it, the
        // most significant at 0.
        let base = self.select_x[option] - width as f32 * 0.5
            + match option {
                OPTION_MUSIC => 171.0,
                OPTION_SOUND => 190.0,
                _ => 210.0,
            };
        let mut offset = 0;
        for slot in digits[..count].iter_mut().rev() {
            slot.1 = base + offset as f32;
            offset += DIGIT_WIDTHS[slot.0];
        }
        (digits, count)
    }

    /// The cameras back to front: the default camera's black (100), the
    /// rows' rules (`func_80017EC0`, 50), then the sprites (30) in GObj
    /// order.
    pub fn visit(&self, f: &mut impl FnMut(Draw)) {
        f(Draw::Clear([0x00, 0x00, 0x00, 0xFF]));
        // `mnSoundTest{Music,Sound,Voice}ProcDisplay`.
        for (row, (x1, y0)) in [(112, 56), (132, 104), (152, 152)].into_iter().enumerate() {
            let rgb = self.colors[row];
            f(fill(10, y0, x1, y0 + 1, rgb));
            f(fill(10, y0 + 39, x1, y0 + 40, rgb));
        }
        // `mnSoundTestMakeHeaderSObjs`.
        f(Draw::Sprite(
            Piece::clear(FILE_DATA_COMMON, data_common::DATA_HEADER, 23.0, 17.0)
                .prim([0x5F, 0x58, 0x46]),
        ));
        f(Draw::Sprite(
            Piece::clear(FILE_SOUND_TEST, sprite::SOUND_TEST_TEXT, 152.0, 23.0)
                .prim([0xF2, 0xC7, 0x0D]),
        ));
        // `mnSoundTestMake{Music,Sound,Voice}SObjs`, coloured by
        // `mnSoundTestOptionThreadUpdate`.
        let rows = [
            (sprite::MUSIC_TEXT, (55.0, 61.0), (112.0, 56.0)),
            (sprite::SOUND_TEXT, (64.0, 108.0), (132.0, 104.0)),
            (sprite::VOICE_TEXT, (94.0, 156.0), (152.0, 152.0)),
        ];
        for (row, (text, (tx, ty), (cx, cy))) in rows.into_iter().enumerate() {
            let rgb = self.colors[row];
            f(Draw::Sprite(
                Piece::clear(FILE_SOUND_TEST, text, tx, ty).prim(rgb),
            ));
            f(Draw::Sprite(
                Piece::clear(FILE_SOUND_TEST, sprite::CAPSULE_RIGHT, cx, cy).prim(rgb),
            ));
        }
        // `mnSoundTestMakeSelectIDGObjs`.
        for (option, y) in [
            (OPTION_MUSIC, 67.0),
            (OPTION_SOUND, 115.0),
            (OPTION_VOICE, 163.0),
        ] {
            let (digits, count) = self.number(option);
            for &(d, x) in &digits[..count] {
                f(Draw::Sprite(
                    Piece::clear(FILE_PLAYER_DAMAGE, DIGITS[d], x, y).prim([0xFF, 0x00, 0x00]),
                ));
            }
        }
        // `mnSoundTestMakeArrowSObjs`, placed by its thread.
        if !self.arrows_hidden {
            let at = self.option * OPTION_COUNT;
            let (lx, y, rx) = (
                ARROW_POSITIONS[at],
                ARROW_POSITIONS[at + 1],
                ARROW_POSITIONS[at + 2],
            );
            for (s, x) in [(sprite::ARROW_L, lx), (sprite::ARROW_R, rx)] {
                f(Draw::Sprite(
                    Piece::clear(FILE_COMMON, s, x, y).prim([0xFF, 0xC3, 0x26]),
                ));
            }
        }
        // `mnSoundTestMakeButtonSObjs`.
        let label = [0x73, 0x6B, 0x59];
        for p in [
            Piece::clear(FILE_BATTLE_PAUSE, sprite::A_BUTTON, 55.0, 205.0)
                .prim([0x6E, 0x77, 0x75])
                .env([0x21, 0x40, 0x3A]),
            Piece::clear(FILE_BATTLE_PAUSE, sprite::B_BUTTON, 218.0, 205.0)
                .prim([0x6E, 0x77, 0x5D])
                .env([0x29, 0x37, 0x16]),
            Piece::clear(FILE_SOUND_TEST, sprite::START_BUTTON, 121.0, 205.0)
                .prim([0x81, 0x6A, 0x62])
                .env([0x3B, 0x20, 0x16]),
            Piece::clear(FILE_SOUND_TEST, sprite::COLON_PLAY_TEXT, 72.0, 208.0).prim(label),
            Piece::clear(FILE_SOUND_TEST, sprite::COLON_FADE_OUT_TEXT, 148.0, 208.0).prim(label),
            Piece::clear(FILE_SOUND_TEST, sprite::COLON_EXIT_TEXT, 235.0, 208.0).prim(label),
        ] {
            f(Draw::Sprite(p));
        }
    }
}

/// `dMNSoundTestMusicIDs`.
pub const MUSIC_IDS: [u32; 45] = [
    nSYAudioBGMOpening,
    nSYAudioBGMExplain,
    nSYAudioBGMData,
    nSYAudioBGMModeSelect,
    nSYAudioBGMCastle,
    nSYAudioBGMJungle,
    nSYAudioBGMHyrule,
    nSYAudioBGMZebes,
    nSYAudioBGMYoster,
    nSYAudioBGMPupupu,
    nSYAudioBGMSector,
    nSYAudioBGMYamabuki,
    nSYAudioBGMInishie,
    nSYAudioBGMInishieHurry,
    nSYAudioBGMWinMario,
    nSYAudioBGMWinDonkey,
    nSYAudioBGMWinZelda,
    nSYAudioBGMWinMetroid,
    nSYAudioBGMWinYoshi,
    nSYAudioBGMWinKirby,
    nSYAudioBGMWinFox,
    nSYAudioBGMWinPMonsters,
    nSYAudioBGMWinFZero,
    nSYAudioBGMWinMother,
    nSYAudioBGMResults,
    nSYAudioBGMHammer,
    nSYAudioBGMStar,
    nSYAudioBGMTrainingMode,
    nSYAudioBGM1PIntro,
    nSYAudioBGMBossStage,
    nSYAudioBGMBossEntry,
    nSYAudioBGMLast,
    nSYAudioBGM1PBonusStage,
    nSYAudioBGM1PStageClear,
    nSYAudioBGM1PGameClear,
    nSYAudioBGM1PBonusStageClear,
    nSYAudioBGM1PBonusStageFailure,
    nSYAudioBGMZako,
    nSYAudioBGMMetal,
    nSYAudioBGM1PChallenger,
    nSYAudioBGMMessage,
    nSYAudioBGMEnding,
    nSYAudioBGM1PGameEndChoice,
    nSYAudioBGM1PGameOver,
    nSYAudioBGMStaffroll,
];

/// `dMNSoundTestSoundIDs`.
pub const SOUND_IDS: [u16; 194] = [
    nSYAudioFGMOpeningSectorAmbient,
    nSYAudioFGMOpeningNewcomersClash,
    nSYAudioFGMPublicPrologue,
    nSYAudioFGMOpeningBatM,
    nSYAudioFGMAltitudeWarn,
    nSYAudioFGMDeadExplodeL,
    nSYAudioFGMDeadExplodeS,
    nSYAudioFGMKickL,
    nSYAudioFGMKickM,
    nSYAudioFGMKickS,
    nSYAudioFGMPunchL,
    nSYAudioFGMPunchM,
    nSYAudioFGMPunchS,
    nSYAudioFGMLightSwingL,
    nSYAudioFGMLightSwingM,
    nSYAudioFGMLightSwingS,
    nSYAudioFGMShockL,
    nSYAudioFGMShockS,
    nSYAudioFGMBurnL,
    nSYAudioFGMBurnS,
    nSYAudioFGMDonkeyLanding,
    nSYAudioFGMUnkGrind2,
    nSYAudioFGMKirbyPurinJump,
    nSYAudioFGMDonkeyFoot,
    nSYAudioFGMSamusFoot,
    nSYAudioFGMMMarioFoot,
    nSYAudioFGMNessDash,
    nSYAudioFGMGroundBrakeGrind,
    nSYAudioFGMGuardOn,
    nSYAudioFGMGuardOff,
    nSYAudioFGMShieldBreak,
    nSYAudioFGMDonkeyDeadSlam,
    nSYAudioFGMYoshiDownBounce,
    nSYAudioFGMCharacterUnkZip1,
    nSYAudioFGMHeavySwing1,
    nSYAudioFGMLightSwingLw1,
    nSYAudioFGMCatch,
    nSYAudioFGMDeadUpStar,
    nSYAudioFGMEscape,
    nSYAudioFGMMSBombAttach,
    nSYAudioFGMBombHeiFuse,
    nSYAudioFGMItemMapCollide,
    nSYAudioFGMBumperHit,
    nSYAudioFGMFireFlowerBurn,
    nSYAudioFGMItemGet,
    nSYAudioFGMHammerSwing,
    nSYAudioFGMHarisenHit,
    nSYAudioFGMBatHit,
    nSYAudioFGMStarMapCollide,
    nSYAudioFGMStarGet,
    nSYAudioFGMBombHeiWalkStart,
    nSYAudioFGMShellHit,
    nSYAudioFGMItemThrow,
    nSYAudioFGMItemSpawn1,
    nSYAudioFGMContainerSmash,
    nSYAudioFGMFireFlowerShoot,
    nSYAudioFGMLGunShoot,
    nSYAudioFGMLGunEmpty,
    nSYAudioFGMStarRodSwing4,
    nSYAudioFGMStarRodSwing1,
    nSYAudioFGMStarRodEmpty,
    nSYAudioFGMSwordSwing4,
    nSYAudioFGMSwordSwing1,
    nSYAudioFGMTaruBombHit,
    nSYAudioFGMTaruBombMap,
    nSYAudioFGMExplodeL,
    nSYAudioFGMFireShoot1,
    nSYAudioFGMShockML,
    nSYAudioFGMMarioAppealGrow,
    nSYAudioFGMMarioAppealShrink,
    nSYAudioFGMUnkDial1,
    nSYAudioFGMMarioSpecialN,
    nSYAudioFGMExplodeS,
    nSYAudioFGMMarioSpecialHiJump,
    nSYAudioFGMMarioSpecialHiCoin,
    nSYAudioFGMMarioUnkSwing1,
    nSYAudioFGMBossSlam,
    nSYAudioFGMBossUnk1,
    nSYAudioFGMBossUnk2,
    nSYAudioFGMDonkeyCharge,
    nSYAudioFGMLinkSpecialLwGet,
    nSYAudioFGMLinkSpecialNReturn,
    nSYAudioFGMLinkSpecialNShoot,
    nSYAudioFGMLinkSpecialNGet,
    nSYAudioFGMLinkSpecialHi,
    nSYAudioFGMLinkCatchHookshot,
    nSYAudioFGMLinkAppear,
    nSYAudioFGMBladeSwing4,
    nSYAudioFGMBladeSwing3,
    nSYAudioFGMBladeSwing1,
    nSYAudioFGMSlashL,
    nSYAudioFGMSlashM,
    nSYAudioFGMSlashS,
    nSYAudioFGMBladeDraw,
    nSYAudioFGMChargeShotAll,
    nSYAudioFGMUnkSmallPing1,
    nSYAudioFGMFoxBlaster,
    nSYAudioFGMSamusJump1,
    nSYAudioFGMSamusSpecialNShootL,
    nSYAudioFGMSamusSpecialNShootS,
    nSYAudioFGMSamusSpecialNCharge0,
    nSYAudioFGMSamusSpecialNCharge7,
    nSYAudioFGMSamusSpecialLw,
    nSYAudioFGMSamusCatchGrappleBeam,
    nSYAudioFGMSamusSpecialHi,
    nSYAudioFGMSamusUnkSwing,
    nSYAudioFGMSamusUnkCharge,
    nSYAudioFGMYoshiEggShatter1,
    nSYAudioFGMYoshiSpecialNTongue,
    nSYAudioFGMYoshiEggShatter3,
    nSYAudioFGMYoshiSpecialHiThrow,
    nSYAudioFGMYoshiEggLayShatter,
    nSYAudioFGMUnkMechanical4,
    nSYAudioFGMUnkLongWind,
    nSYAudioFGMKirbySpecialLwLanding,
    nSYAudioFGMKirbyAttackAirHi,
    nSYAudioFGMKirbySpecialNThrow,
    nSYAudioFGMKirbySpecialNCopyEat,
    nSYAudioFGMKirbySpecialNCopyThrow,
    nSYAudioFGMKirbySpecialNCopyUnk,
    nSYAudioFGMKirbyStarPing2,
    nSYAudioFGMKirbySpecialLwStart,
    nSYAudioFGMKirbySpecialNStart,
    nSYAudioFGMKirbySpecialNLoseCopy,
    nSYAudioFGMFoxSpecialN,
    nSYAudioFGMFoxSpecialHiStart,
    nSYAudioFGMFoxSpecialHiFly,
    nSYAudioFGMFoxSpecialLwHit,
    nSYAudioFGMFoxSpecialLwStart,
    nSYAudioFGMFoxAttackAirLw,
    nSYAudioFGMFoxAppearArwing,
    nSYAudioFGMUnkShoot1,
    nSYAudioFGMPikachuElectric1,
    nSYAudioFGMPikachuElectric2,
    nSYAudioFGMPikachuElectric5,
    nSYAudioFGMPikachuElectricLoop,
    nSYAudioFGMPikachuSpecialHiStart,
    nSYAudioFGMPikachuSpecialLwThunder,
    nSYAudioFGMCaptainAppearCar1,
    nSYAudioFGMCaptainAppearCar2,
    nSYAudioFGMCaptainSpecialHi,
    nSYAudioFGMCaptainSpecialNStart,
    nSYAudioFGMCaptainSpecialNPunch,
    nSYAudioFGMCharacterUnk1,
    nSYAudioFGMNessPKThunderLoop,
    nSYAudioFGMNessSpecialLwStart,
    nSYAudioFGMCharacterUnk3,
    nSYAudioFGMUnkSwoosh1,
    nSYAudioFGMUnkGate1,
    nSYAudioFGMBossBullet,
    nSYAudioFGMSectorArwingLaser,
    nSYAudioFGMSectorAmbient1,
    nSYAudioFGMOptionBackupClear,
    nSYAudioFGMMagnify,
    nSYAudioFGMBonusComplete,
    nSYAudioFGMPlayerHeal,
    nSYAudioFGMYosterCloudVapor,
    nSYAudioFGMStockSteal,
    nSYAudioFGMBonus2PlatformLanding,
    nSYAudioFGMGamePause,
    nSYAudioFGMInishiePowerBlock,
    nSYAudioFGMBonus1TargetBreak,
    nSYAudioFGMJungleTaruCannShoot,
    nSYAudioFGMHyruleTwisterAppear,
    nSYAudioFGMHyruleTwisterTrapped,
    nSYAudioFGMPupupuWhispyWind,
    nSYAudioFGMFloorDamageFire,
    nSYAudioFGMDogasSmog,
    nSYAudioFGMIwarkRockMake,
    nSYAudioFGMKabigonFall,
    nSYAudioFGMKabigonJump,
    nSYAudioFGMKamexHydro,
    nSYAudioFGMLizardonFlame,
    nSYAudioFGMMewFly,
    nSYAudioFGMNyarsCoin,
    nSYAudioFGMMBallOpen,
    nSYAudioFGMMonsterShoot,
    nSYAudioFGMTosakintoSplash,
    nSYAudioFGMUnkMechanical1,
    nSYAudioFGMTitlePressStart,
    nSYAudioFGMMenuSelect,
    nSYAudioFGMStageSelect,
    nSYAudioFGM1PGameContinue,
    nSYAudioFGMTrainingSel2,
    nSYAudioFGMMenuScroll1,
    nSYAudioFGMMenuScroll2,
    nSYAudioFGMMenuDenied,
    nSYAudioFGMPlayerSlotClose,
    nSYAudioFGMPlayerSlotWhoosh,
    nSYAudioFGMScoreDisplayBonus,
    nSYAudioFGMStageClearScoreRegister,
    nSYAudioFGMStageClearScoreDisplay,
    nSYAudioFGMDoorClose,
    nSYAudioFGMTrainingSel,
];

/// `dMNSoundTestVoiceIDs`.
pub const VOICE_IDS: [u16; 244] = [
    // Mario
    nSYAudioVoiceMarioSmash1,
    nSYAudioVoiceMarioSmash2,
    nSYAudioVoiceMarioSmash3,
    nSYAudioVoiceMarioSpecialLw,
    nSYAudioVoiceMarioDeadUp,
    nSYAudioVoiceMarioJump,
    nSYAudioVoiceMarioJumpAerial,
    nSYAudioVoiceMarioHeavyGet,
    nSYAudioVoiceMarioDead,
    nSYAudioVoiceMarioDamage,
    nSYAudioVoiceMarioHereWe,
    // Donkey Kong
    nSYAudioVoiceDonkeyAppeal,
    nSYAudioVoiceDonkeySmash1,
    nSYAudioVoiceDonkeySmash2,
    nSYAudioVoiceDonkeySmash3,
    nSYAudioVoiceDonkeyDeadUp,
    nSYAudioVoiceDonkeyDamage,
    nSYAudioVoiceDonkeyDead1,
    nSYAudioVoiceDonkeyHeavyGet,
    nSYAudioVoiceDonkeyHeavyUnk,
    nSYAudioVoiceDonkeyDead2,
    // Link
    nSYAudioVoiceLinkSmash1,
    nSYAudioVoiceLinkSmash2,
    nSYAudioVoiceLinkSmash3,
    nSYAudioVoiceLinkSpecialHi,
    nSYAudioVoiceLinkDeadUp,
    nSYAudioVoiceLinkDamage,
    nSYAudioVoiceLinkJump,
    nSYAudioVoiceLinkJumpAerial,
    nSYAudioVoiceLinkOttotto,
    nSYAudioVoiceLinkDead,
    nSYAudioVoiceLinkGrunt2,
    // Yoshi
    nSYAudioVoiceYoshiAppeal,
    nSYAudioVoiceYoshiSmash2,
    nSYAudioVoiceYoshiSmash3,
    nSYAudioVoiceYoshiCatch,
    nSYAudioVoiceYoshiDeadUp,
    nSYAudioVoiceYoshiDamage,
    nSYAudioVoiceYoshiJump,
    nSYAudioVoiceYoshiJumpAerial,
    nSYAudioVoiceYoshiFuraSleep,
    nSYAudioVoiceYoshiSpecialLwJump,
    nSYAudioVoiceYoshiSpecialLwFall,
    nSYAudioVoiceYoshiUnkGrunt2,
    nSYAudioVoiceYoshiThrow,
    nSYAudioVoiceYoshiUnkVocalize,
    // Kirby
    nSYAudioVoiceKirbyAppeal,
    nSYAudioVoiceKirbySmash1,
    nSYAudioVoiceKirbySmash2,
    nSYAudioVoiceKirbySmash3,
    nSYAudioVoiceKirbyCopyLinkSpecialN,
    nSYAudioVoiceKirbyCopyPikachuSpecialN,
    nSYAudioVoiceKirbySpecialHi,
    nSYAudioVoiceKirbyCopyCaptainSpecialNFalcon,
    nSYAudioVoiceKirbyCopyCaptainSpecialNPunch,
    nSYAudioVoiceKirbyCopyDonkeySpecialN,
    nSYAudioVoiceKirbyCopyPurinSpecialN,
    nSYAudioVoiceKirbyDeadUp,
    nSYAudioVoiceKirbyFuraFura,
    nSYAudioVoiceKirbyDamage,
    nSYAudioVoiceKirbyHeavyGet,
    nSYAudioVoiceKirbyOttotto,
    nSYAudioVoiceKirbyCopyNessSpecialN,
    nSYAudioVoiceKirbyDead,
    nSYAudioVoiceKirbyFuraSleep,
    nSYAudioVoiceKirbySpecialLw,
    // Fox
    nSYAudioVoiceFoxDeadUp,
    nSYAudioVoiceFoxSpecialHi,
    nSYAudioVoiceFoxJumpAerial,
    nSYAudioVoiceFoxEscape,
    nSYAudioVoiceFoxSelected,
    nSYAudioVoiceFoxHeavyGet,
    nSYAudioVoiceFoxOttotto,
    nSYAudioVoiceFoxDead,
    nSYAudioVoiceFoxSmash1,
    nSYAudioVoiceFoxSmash2,
    nSYAudioVoiceFoxSmash3,
    nSYAudioVoiceFoxDamage,
    nSYAudioVoiceFoxFuraFura,
    // Pikachu
    nSYAudioVoicePikachuAppeal,
    nSYAudioVoicePikachuSmash1,
    nSYAudioVoicePikachuSmash2,
    nSYAudioVoicePikachuSmash3,
    nSYAudioVoicePikachuSpecialN,
    nSYAudioVoicePikachuSpecialLw,
    nSYAudioVoicePikachuDeadUp,
    nSYAudioVoicePikachuDamage,
    nSYAudioVoicePikachuSpecialHi,
    nSYAudioVoicePikachuHeavyGet,
    nSYAudioVoicePikachuOttotto,
    nSYAudioVoicePikachuDead,
    nSYAudioVoicePikachuFuraSleep,
    // Luigi
    nSYAudioVoiceLuigiSmash1,
    nSYAudioVoiceLuigiSmash2,
    nSYAudioVoiceLuigiSmash3,
    nSYAudioVoiceLuigiSpecialLw,
    nSYAudioVoiceLuigiDeadUp,
    nSYAudioVoiceLuigiFuraFura,
    nSYAudioVoiceLuigiDamage,
    nSYAudioVoiceLuigiJump,
    nSYAudioVoiceLuigiJumpAerial,
    nSYAudioVoiceLuigiHeavyGet,
    nSYAudioVoiceLuigiDead,
    nSYAudioVoiceLuigiHereWe,
    // Captain Falcon
    nSYAudioVoiceCaptainAppeal,
    nSYAudioVoiceCaptainSpecialHi,
    nSYAudioVoiceCaptainSmash1,
    nSYAudioVoiceCaptainSmash2,
    nSYAudioVoiceCaptainSmash3,
    nSYAudioVoiceCaptainSmash5,
    nSYAudioVoiceCaptainAttackS4,
    nSYAudioVoiceCaptainSpecialLw,
    nSYAudioVoiceCaptainSpecialNFalcon,
    nSYAudioVoiceCaptainSpecialNPunch,
    nSYAudioVoiceCaptainDeadUp,
    nSYAudioVoiceCaptainFuraFura,
    nSYAudioVoiceCaptainDamage,
    nSYAudioVoiceCaptainJumpAerial,
    nSYAudioVoiceCaptainHeavyGet,
    nSYAudioVoiceCaptainDead,
    nSYAudioVoiceCaptainFuraSleep,
    nSYAudioVoiceCaptainUnkQuick,
    // Ness
    nSYAudioVoiceNessAppeal,
    nSYAudioVoiceNessSmash1,
    nSYAudioVoiceNessSmash2,
    nSYAudioVoiceNessSmash3,
    nSYAudioVoiceNessUnkGrunt,
    nSYAudioVoiceNessDeadUp,
    nSYAudioVoiceNessFuraFura,
    nSYAudioVoiceNessDamage,
    nSYAudioVoiceNessHeavyGet,
    nSYAudioVoiceNessOttotto,
    nSYAudioVoiceNessSpecialN,
    nSYAudioVoiceNessSpecialHi,
    nSYAudioVoiceNessDead,
    nSYAudioVoiceNessFuraSleep,
    // Jigglypuff
    nSYAudioVoicePurinAppeal,
    nSYAudioVoicePurinSmash1,
    nSYAudioVoicePurinSmash2,
    nSYAudioVoicePurinSmash3,
    nSYAudioVoicePurinSpecialN,
    nSYAudioVoicePurinDeadUp,
    nSYAudioVoicePurinFuraFura,
    nSYAudioVoicePurinDamage,
    nSYAudioVoicePurinUnkGrunt2,
    nSYAudioVoicePurinUnkGrunt3,
    nSYAudioVoicePurinUnkGrunt4,
    nSYAudioVoicePurinFuraSleep,
    nSYAudioVoicePurinSpecialLwSleep,
    nSYAudioVoicePurinSpecialLwWake,
    nSYAudioVoicePurinSpecialHi,
    // Master Hand
    nSYAudioVoiceBossAppear,
    nSYAudioVoiceBossDead,
    // Announcer
    nSYAudioVoiceAnnounceTitleWait,
    nSYAudioVoiceAnnounceMario,
    nSYAudioVoiceAnnounceDonkey,
    nSYAudioVoiceAnnounceSamus,
    nSYAudioVoiceAnnounceFox,
    nSYAudioVoiceAnnounceYoshi,
    nSYAudioVoiceAnnounceLink,
    nSYAudioVoiceAnnouncePikachu,
    nSYAudioVoiceAnnounceKirby,
    nSYAudioVoiceAnnounceLuigi,
    nSYAudioVoiceAnnounceCaptain,
    nSYAudioVoiceAnnounceNess,
    nSYAudioVoiceAnnouncePurin,
    nSYAudioVoiceAnnounceRedTeam,
    nSYAudioVoiceAnnounceBlueTeam,
    nSYAudioVoiceAnnounceGreenTeam,
    nSYAudioVoiceAnnounceFreeForAll,
    nSYAudioVoiceAnnounceTeamBattle,
    nSYAudioVoiceAnnounceSelectPlayer,
    nSYAudioVoiceAnnounceContinue,
    nSYAudioVoiceAnnounceGameOver,
    nSYAudioVoiceAnnounceGo,
    nSYAudioVoiceAnnounceFive,
    nSYAudioVoiceAnnounceFour,
    nSYAudioVoiceAnnounceThree,
    nSYAudioVoiceAnnounceTwo,
    nSYAudioVoiceAnnounceOne,
    nSYAudioVoiceAnnounceSuddenDeath,
    nSYAudioVoiceAnnounceTimeUp,
    nSYAudioVoiceAnnounceGameSet,
    nSYAudioVoiceAnnounceWinnerIs,
    nSYAudioVoiceAnnounceNoContest,
    nSYAudioVoiceAnnouncePlayer1,
    nSYAudioVoiceAnnouncePlayer2,
    nSYAudioVoiceAnnouncePlayer3,
    nSYAudioVoiceAnnouncePlayer4,
    nSYAudioVoiceAnnounceComputerPlayer,
    nSYAudioVoiceAnnounceVersus,
    nSYAudioVoiceAnnounceYoshiTeam,
    nSYAudioVoiceAnnounceKirbyTeam,
    nSYAudioVoiceAnnounceGDonkey,
    nSYAudioVoiceAnnounceMarioBros,
    nSYAudioVoiceAnnounceMMario,
    nSYAudioVoiceAnnounceZako,
    nSYAudioVoiceAnnounceBonusStage,
    nSYAudioVoiceAnnounceBreakTheTargets,
    nSYAudioVoiceAnnounceBoardThePlatforms,
    nSYAudioVoiceAnnounceComplete,
    nSYAudioVoiceAnnounceFailure,
    nSYAudioVoiceAnnounceNewRecord,
    nSYAudioVoiceAnnounceTrainingMode,
    nSYAudioVoiceAnnounceHowToPlay,
    // Pokéball Pokémon
    nSYAudioVoiceMBallDogasAppear,
    nSYAudioVoiceMBallIwarkAppear,
    nSYAudioVoiceMBallKabigonFall,
    nSYAudioVoiceMBallKabigonAppear,
    nSYAudioVoiceMBallKamexAppear,
    nSYAudioVoiceMBallLuckyAppear,
    nSYAudioVoiceMBallMewAppear,
    nSYAudioVoiceMBallPippiAppear,
    nSYAudioVoiceMBallLizardonAppear,
    nSYAudioVoiceMBallSawamuraAppear,
    nSYAudioVoiceMBallSawamuraKick,
    nSYAudioVoiceMBallSpearAppear,
    nSYAudioVoiceMBallSpearSwarm,
    nSYAudioVoiceMBallStarmieAppear,
    nSYAudioVoiceMBallTosakintoAppear,
    // Saffron City Pokémon
    nSYAudioVoiceYamabukiFushigibana,
    nSYAudioVoiceYamabukiHitokage,
    nSYAudioVoiceYamabukiLucky, // No Electrode?
    nSYAudioVoiceYamabukiPorygon,
    // Audience Chants
    nSYAudioVoicePublicDonkey,
    nSYAudioVoicePublicCaptain,
    nSYAudioVoicePublicFox,
    nSYAudioVoicePublicKirby,
    nSYAudioVoicePublicLink,
    nSYAudioVoicePublicLuigi,
    nSYAudioVoicePublicMario,
    nSYAudioVoicePublicNess,
    nSYAudioVoicePublicPikachu,
    nSYAudioVoicePublicPurin,
    nSYAudioVoicePublicSamus,
    nSYAudioVoicePublicYoshi,
    // Audience Reactions
    nSYAudioVoicePublicGaspL,
    nSYAudioVoicePublicGaspS,
    nSYAudioVoicePublicCheer,
    nSYAudioVoicePublicGaspClap,
    nSYAudioVoicePublicDamageL,
    nSYAudioVoicePublicDamageS,
    nSYAudioVoicePublicAbsorb,
    nSYAudioVoicePublicClapS,
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sound::testing::{Call, Recorder};

    fn tap(b: u16) -> Pad {
        Pad {
            hold: b,
            tap: b,
            ..Pad::default()
        }
    }

    fn hold(b: u16) -> Pad {
        Pad {
            hold: b,
            ..Pad::default()
        }
    }

    #[test]
    fn tables_match_the_decomp_counts() {
        assert_eq!(MUSIC_IDS.len(), 45);
        assert_eq!(SOUND_IDS.len(), 194);
        assert_eq!(VOICE_IDS.len(), 244);
        assert_eq!(MUSIC_IDS[0], nSYAudioBGMOpening);
        assert_eq!(MUSIC_IDS[44], nSYAudioBGMStaffroll);
        assert_eq!(SOUND_IDS[193], nSYAudioFGMTrainingSel);
        assert_eq!(VOICE_IDS[0], nSYAudioVoiceMarioSmash1);
        assert_eq!(VOICE_IDS[243], nSYAudioVoicePublicClapS);
    }

    #[test]
    fn idle_tick_holds_the_music_volume_and_highlights_music() {
        let r = Recorder::install();
        let mut m = SoundTest::new();
        assert_eq!(m.colors, [COLOR_OFF; 3]);
        assert_eq!(m.tick(&Pad::default()), None);
        assert_eq!(m.colors, [COLOR_ON, COLOR_OFF, COLOR_OFF]);
        assert_eq!(r.take(), [Call::SetBgmVolume(0, 0x7000)]);
    }

    #[test]
    fn a_plays_the_selected_entry_of_each_row() {
        let r = Recorder::install();
        let mut m = SoundTest::new();
        m.tick(&tap(N64Buttons::A));
        assert_eq!(
            r.take(),
            [
                Call::SetBgmVolume(0, 0x7000),
                Call::StopBgmAll,
                Call::PlayBgm(0, nSYAudioBGMOpening),
            ]
        );
        // Left wraps Music to its last entry, the staff roll.
        m.tick(&tap(N64Buttons::D_LEFT));
        assert_eq!(m.select_id[OPTION_MUSIC], 44);
        m.change_wait = 0;
        r.take();
        m.tick(&tap(N64Buttons::A));
        assert_eq!(r.take()[2], Call::PlayBgm(0, nSYAudioBGMStaffroll));
        // Down to Sound: a scroll sound, then A stops every sound first.
        m.change_wait = 0;
        m.tick(&tap(N64Buttons::D_DOWN));
        assert_eq!(m.option, OPTION_SOUND);
        assert!(r.take().contains(&Call::PlayFgm(nSYAudioFGMMenuScroll2)));
        m.tick(&tap(N64Buttons::A));
        assert_eq!(
            r.take(),
            [
                Call::SetBgmVolume(0, 0x7000),
                Call::StopAllFgm,
                Call::PlayFgm(SOUND_IDS[0]),
            ]
        );
        // Up from Music wraps to Voice.
        m.change_wait = 0;
        m.tick(&tap(N64Buttons::D_UP));
        m.change_wait = 0;
        m.tick(&tap(N64Buttons::D_UP));
        assert_eq!(m.option, OPTION_VOICE);
        m.change_wait = 0;
        m.tick(&tap(N64Buttons::D_RIGHT));
        r.take();
        m.tick(&tap(N64Buttons::A));
        assert_eq!(r.take()[2], Call::PlayFgm(VOICE_IDS[1]));
    }

    #[test]
    fn start_fades_the_music_then_stops_it() {
        let r = Recorder::install();
        let mut m = SoundTest::new();
        m.tick(&tap(N64Buttons::START));
        assert_eq!(
            r.take(),
            [
                Call::SetBgmVolume(0, 0x7000),
                Call::SetBgmVolumeFade(0, 0, 120),
                Call::StopAllFgm,
            ]
        );
        // 120 tics counting down, the stop on the 121st, then the volume
        // back on the next.
        for _ in 0..120 {
            m.tick(&Pad::default());
        }
        assert_eq!(r.take(), []);
        m.tick(&Pad::default());
        assert_eq!(r.take(), [Call::StopBgmAll]);
        m.tick(&Pad::default());
        assert_eq!(r.take(), [Call::SetBgmVolume(0, 0x7000)]);
    }

    #[test]
    fn music_during_a_fade_cancels_it() {
        let r = Recorder::install();
        let mut m = SoundTest::new();
        m.tick(&tap(N64Buttons::START));
        m.tick(&Pad::default());
        r.take();
        m.tick(&tap(N64Buttons::A));
        assert_eq!(m.fade_out_wait, -1);
        assert_eq!(
            r.take(),
            [Call::StopBgmAll, Call::PlayBgm(0, nSYAudioBGMOpening)]
        );
    }

    #[test]
    fn z_stops_everything_and_b_returns_to_data() {
        let r = Recorder::install();
        let mut m = SoundTest::new();
        m.tick(&tap(N64Buttons::Z));
        assert_eq!(
            r.take(),
            [
                Call::SetBgmVolume(0, 0x7000),
                Call::StopBgmAll,
                Call::StopAllFgm
            ]
        );
        assert_eq!(m.tick(&tap(N64Buttons::B)), Some(Scene::Data));
        assert_eq!(
            r.take(),
            [
                Call::StopBgmAll,
                Call::StopAllFgm,
                Call::SetBgmVolume(0, 0x7000),
                Call::SetBgmVolume(0, 0x7000),
            ]
        );
    }

    #[test]
    fn held_direction_repeats_with_the_decomps_waits() {
        let _r = Recorder::install();
        let mut m = SoundTest::new();
        // A held button: 12, doubled for a new direction.
        m.tick(&hold(N64Buttons::D_RIGHT));
        assert_eq!((m.select_id[0], m.change_wait), (1, 24));
        let mut ticks = 0;
        while m.select_id[0] == 1 {
            m.tick(&hold(N64Buttons::D_RIGHT));
            ticks += 1;
        }
        // 23 tics count it down; the 24th reads 0 and steps, now 12 only.
        assert_eq!(ticks, 24);
        assert_eq!(m.change_wait, 12);
        // Releasing clears the wait and the direction.
        m.tick(&Pad::default());
        assert_eq!((m.change_wait, m.direction), (0, 0));
        // The stick past 32: (160 - x) / 16, doubled.
        let stick = Pad {
            stick_x: 80,
            ..Pad::default()
        };
        m.tick(&stick);
        assert_eq!((m.select_id[0], m.change_wait), (3, 10));
        // Reaching the last entry adds 20 before the doubling.
        m.select_id[0] = 43;
        m.change_wait = 0;
        m.direction = 0;
        m.tick(&hold(N64Buttons::R));
        assert_eq!((m.select_id[0], m.change_wait), (44, 64));
        // Up onto Music adds 10.
        m.option = OPTION_SOUND;
        m.change_wait = 0;
        m.tick(&hold(N64Buttons::C_UP));
        assert_eq!((m.option, m.change_wait), (OPTION_MUSIC, 22));
    }

    #[test]
    fn numbers_centre_on_their_widths() {
        let mut m = SoundTest::new();
        // "1": width 9 at 26.5 - 4.5 + 171.
        let (d, n) = m.number(OPTION_MUSIC);
        assert_eq!((n, d[0]), (1, (1, 193.0)));
        // "123" on Voice: widths 9, 15, 14 = 38; the hundreds leftmost.
        m.select_id[OPTION_VOICE] = 122;
        let (d, n) = m.number(OPTION_VOICE);
        let base = 26.5 - 19.0 + 210.0;
        assert_eq!(n, 3);
        assert_eq!(d, [(3, base + 24.0), (2, base + 9.0), (1, base)]);
    }

    #[test]
    fn arrows_blink_every_thirty_tics_and_follow_the_row() {
        let _r = Recorder::install();
        let mut m = SoundTest::new();
        let shown = |m: &SoundTest| {
            let mut n = 0;
            m.visit(&mut |d| {
                if let Draw::Sprite(p) = d {
                    if p.offset == sprite::ARROW_L && p.file == FILE_COMMON {
                        n += 1;
                        assert_eq!(p.y, ARROW_POSITIONS[m.option * 3 + 1]);
                    }
                }
            });
            n == 1
        };
        for _ in 0..30 {
            m.tick(&Pad::default());
            assert!(shown(&m));
        }
        for _ in 0..30 {
            m.tick(&Pad::default());
            assert!(!shown(&m));
        }
        m.tick(&Pad::default());
        assert!(shown(&m));
        // A new row shows them at once.
        for _ in 0..30 {
            m.tick(&Pad::default());
        }
        assert!(!shown(&m));
        m.tick(&tap(N64Buttons::D_DOWN));
        assert!(shown(&m));
    }
}
