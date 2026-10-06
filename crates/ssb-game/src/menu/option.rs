//! `mn/mnoption/mnoption.c`: the Option menu. Three tabs (Sound, Screen
//! Adjust, Backup Clear); Sound toggles stereo and mono in place, the other
//! two open their scenes. Every way out writes the sound setting and the
//! screen flash back to the backup (`mnOptionWriteBackup`).
//!
//! The US build's `mnOptionMakeMenuGObj` makes an empty GObj: the Japanese
//! subtitle under the tabs is not drawn.

use super::{
    decals, labels, option_tab, Draw, Pad, Piece, Repeat, Scene, TabStatus, DOWN, FILE_COMMON,
    FILE_OPTION, IDLE_RETURN, LEFT, RIGHT, UP,
};
use crate::backup::Backup;
use ssb_engine::input::N64Buttons;

/// `llMNOption*Sprite` (`reloc_data.us.h`).
pub mod sprite {
    pub const STEREO_TEXT: u32 = 0x71F8;
    pub const MONO_TEXT: u32 = 0x73A8;
    pub const SOUND_TEXT: u32 = 0x7628;
    pub const SCREEN_ADJUST_TEXT: u32 = 0x8138;
    pub const BACKUP_CLEAR_TEXT: u32 = 0x8780;
    pub const OPTION_TEXT: u32 = 0x9288;
    pub const SETTINGS_ICON_DARK: u32 = 0xB958;
}

/// `nMNOptionOption*`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Sound,
    ScreenAdjust,
    BackupClear,
}

impl Tab {
    const ALL: [Tab; 3] = [Tab::Sound, Tab::ScreenAdjust, Tab::BackupClear];

    fn index(self) -> usize {
        self as usize
    }
}

/// The menu's state (`sMNOption*`).
#[derive(Debug, Clone, PartialEq)]
pub struct OptionMenu {
    pub tab: Tab,
    /// `sMNOptionSoundMonoOrStereo`: 1 stereo, 0 mono.
    pub mono_or_stereo: u8,
    is_screen_flash: bool,
    is_proceed_scene: bool,
    change_wait: i32,
    total_tics: i32,
    return_tic: i32,
    /// Each tab's colours as `mnOptionSetOptionSpriteColors` last set them.
    pub tab_status: [TabStatus; 3],
    /// `gSCManagerSceneData.scene_curr` as this scene leaves it.
    scene_curr: Scene,
}

impl OptionMenu {
    /// `mnOptionFuncStart` (`mnOptionInitVars`): the tab of the scene it
    /// came back from, and `dSYAudioSoundQuality` as the sound setting.
    pub fn new(scene_prev: Scene, sound_quality: u8, backup: &Backup) -> OptionMenu {
        let tab = match scene_prev {
            Scene::ScreenAdjust => Tab::ScreenAdjust,
            Scene::BackupClear => Tab::BackupClear,
            _ => Tab::Sound,
        };
        OptionMenu {
            tab,
            mono_or_stereo: u8::from(sound_quality == 1),
            is_screen_flash: backup.is_allow_screenflash,
            is_proceed_scene: false,
            change_wait: 0,
            total_tics: 0,
            return_tic: IDLE_RETURN,
            tab_status: Tab::ALL.map(|t| TabStatus::of(t == tab)),
            scene_curr: Scene::Option,
        }
    }

    /// `dSYAudioSoundQuality` after `syAudioSetQuality`.
    pub fn sound_quality(&self) -> u8 {
        self.mono_or_stereo
    }

    /// `mnOptionWriteBackup`.
    fn write_backup(&self, backup: &mut Backup) {
        backup.is_allow_screenflash = self.is_screen_flash;
        backup.sound_mono_or_stereo = self.mono_or_stereo;
        backup.write();
    }

    fn select(&mut self, tab: Tab) {
        self.tab_status[self.tab.index()] = TabStatus::Not;
        self.tab = tab;
        self.tab_status[tab.index()] = TabStatus::Highlight;
    }

    /// `mnOptionFuncRun`. Returns the scene `syTaskmanSetLoadScene` loads.
    pub fn tick(&mut self, pad: &Pad, backup: &mut Backup) -> Option<Scene> {
        self.total_tics += 1;
        if self.total_tics < 10 {
            return None;
        }
        if self.total_tics == self.return_tic {
            self.scene_curr = Scene::Title;
            self.write_backup(backup);
            return Some(Scene::Title);
        }
        if !pad.no_input_all() {
            self.return_tic = self.total_tics + IDLE_RETURN;
        }
        let mut load = self.is_proceed_scene;
        if self.change_wait != 0 {
            self.change_wait -= 1;
        }
        if pad.released(UP | DOWN) {
            self.change_wait = 0;
        }
        if pad.tap(N64Buttons::A | N64Buttons::START) {
            let next = match self.tab {
                Tab::ScreenAdjust => Some(Scene::ScreenAdjust),
                Tab::BackupClear => Some(Scene::BackupClear),
                Tab::Sound => None,
            };
            if let Some(next) = next {
                self.write_backup(backup);
                self.tab_status[self.tab.index()] = TabStatus::Selected;
                self.scene_curr = next;
                self.is_proceed_scene = true;
                return load.then_some(self.scene_curr);
            }
        }
        if pad.tap(N64Buttons::B) {
            self.write_backup(backup);
            self.scene_curr = Scene::ModeSelect;
            load = true;
        }
        let mut r = Repeat::default();
        if r.check(self.change_wait, pad, UP, true, 20, true) {
            self.change_wait = r.wait_p(7);
            let next = match self.tab {
                Tab::Sound => Tab::BackupClear,
                Tab::ScreenAdjust => Tab::Sound,
                Tab::BackupClear => Tab::ScreenAdjust,
            };
            self.select(next);
        }
        if r.check(self.change_wait, pad, DOWN, true, -20, false) {
            self.change_wait = r.wait_n(7);
            if self.tab == Tab::ScreenAdjust {
                self.change_wait += 8;
            }
            let next = match self.tab {
                Tab::Sound => Tab::ScreenAdjust,
                Tab::ScreenAdjust => Tab::BackupClear,
                Tab::BackupClear => Tab::Sound,
            };
            self.select(next);
        }
        if self.tab == Tab::Sound {
            if (pad.tap(LEFT) || r.check_stick(self.change_wait, pad, false, -20, false))
                && self.mono_or_stereo == 0
            {
                self.mono_or_stereo = 1;
                self.change_wait = super::wait_n(r.stick, 7);
            }
            if (pad.tap(RIGHT) || r.check_stick(self.change_wait, pad, false, 20, true))
                && self.mono_or_stereo == 1
            {
                self.mono_or_stereo = 0;
                self.change_wait = super::wait_p(r.stick, 7);
            }
            if pad.tap(N64Buttons::A) {
                self.mono_or_stereo = if self.mono_or_stereo == 1 { 0 } else { 1 };
            }
        }
        load.then_some(self.scene_curr)
    }

    /// The cameras back to front: decals (80), labels (60), the tabs and
    /// the sound toggle (40), then the sound underline (20).
    pub fn visit(&self, f: &mut impl FnMut(Draw)) {
        decals(f, FILE_OPTION, sprite::SETTINGS_ICON_DARK);
        labels(
            f,
            [160, 120, 20, 230],
            FILE_OPTION,
            sprite::OPTION_TEXT,
            (201.0, 120.0),
        );
        // `mnOptionMakeSound`.
        option_tab(f, 113.0, 42.0, 17, self.tab_status[0]);
        f(Draw::Sprite(
            Piece::clear(FILE_OPTION, sprite::SOUND_TEXT, 116.0, 46.0).prim([0; 3]),
        ));
        // `mnOptionMakeSoundToggle` (`mnOptionSetSoundToggleSpriteColors`).
        let (stereo, mono) = if self.mono_or_stereo != 0 {
            (0xFF, 0x32)
        } else {
            (0x32, 0xFF)
        };
        f(Draw::Sprite(
            Piece::clear(FILE_OPTION, sprite::STEREO_TEXT, 179.0, 48.0).prim([stereo; 3]),
        ));
        f(Draw::Sprite(
            Piece::clear(FILE_OPTION, sprite::MONO_TEXT, 236.0, 48.0).prim([mono; 3]),
        ));
        f(Draw::Sprite(
            Piece::clear(FILE_COMMON, super::common::SLASH, 229.0, 48.0).prim([0x32; 3]),
        ));
        // `mnOptionMakeScreenAdjust` and `mnOptionMakeBackupClear`.
        option_tab(f, 91.0, 89.0, 17, self.tab_status[1]);
        f(Draw::Sprite(
            Piece::clear(FILE_OPTION, sprite::SCREEN_ADJUST_TEXT, 103.0, 92.0).prim([0; 3]),
        ));
        option_tab(f, 69.0, 136.0, 17, self.tab_status[2]);
        f(Draw::Sprite(
            Piece::clear(FILE_OPTION, sprite::BACKUP_CLEAR_TEXT, 86.0, 140.0).prim([0; 3]),
        ));
        // `mnOptionSoundUnderlineProcDisplay`.
        if self.tab == Tab::Sound {
            let r =
                [[233, 64, 273, 64], [179, 64, 225, 64]][usize::from(self.mono_or_stereo.min(1))];
            f(super::fill(r[0], r[1], r[2], r[3], [0xFF; 3]));
        }
    }
}
