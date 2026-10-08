//! `mn/mndata/mndata.c`: the Data menu. Characters and VS Record, and
//! Sound Test once the backup unlocks it; the tabs move up when Sound Test
//! joins them.
//!
//! The US build's `mnDataMakeMenuGObj` adds no display, so the Japanese
//! subtitle is not drawn.

use super::{
    decals, labels, option_tab, Draw, Pad, Piece, Repeat, Scene, TabStatus, DOWN, FILE_DATA,
    IDLE_RETURN, UP,
};
use crate::backup::Backup;
use crate::sound::{self, id};
use crate::spgame::Unlock;
use ssb_engine::input::N64Buttons;

/// `llMNData*Sprite`.
pub mod sprite {
    pub const CHARACTERS_TEXT: u32 = 0x14E0;
    pub const VS_RECORD_TEXT: u32 = 0x1900;
    pub const SOUND_TEST_TEXT: u32 = 0x1D20;
    pub const DATA_TEXT: u32 = 0x23A8;
    pub const DATA_ICON_DARK: u32 = 0x4A78;
}

/// `nMNDataOption*`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DataOption {
    Characters,
    VsRecord,
    SoundTest,
}

impl DataOption {
    fn from_index(i: usize) -> DataOption {
        [
            DataOption::Characters,
            DataOption::VsRecord,
            DataOption::SoundTest,
        ][i.min(2)]
    }
}

/// The menu's state (`sMNData*`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DataMenu {
    pub option: DataOption,
    last_option: DataOption,
    /// `sMNDataIsHaveSoundTest`.
    pub is_have_sound_test: bool,
    is_proceed_scene: bool,
    change_wait: i32,
    total_tics: i32,
    return_tic: i32,
    pub tab_status: [TabStatus; 3],
    scene_curr: Scene,
}

impl DataMenu {
    /// `mnDataFuncStart` (`mnDataInitVars`).
    pub fn new(scene_prev: Scene, backup: &Backup) -> DataMenu {
        let option = match scene_prev {
            Scene::VsRecord => DataOption::VsRecord,
            Scene::SoundTest => DataOption::SoundTest,
            _ => DataOption::Characters,
        };
        // `mnDataCheckSoundTestUnlocked`.
        let is_have_sound_test = backup.unlock_mask & Unlock::SoundTest.mask() != 0;
        if matches!(
            scene_prev,
            Scene::VsRecord | Scene::Characters | Scene::SoundTest
        ) {
            sound::play_bgm(0, id::nSYAudioBGMModeSelect);
        }
        DataMenu {
            option,
            last_option: if is_have_sound_test {
                DataOption::SoundTest
            } else {
                DataOption::VsRecord
            },
            is_have_sound_test,
            is_proceed_scene: false,
            change_wait: 0,
            total_tics: 0,
            return_tic: IDLE_RETURN,
            tab_status: [0, 1, 2].map(|i| TabStatus::of(DataOption::from_index(i) == option)),
            scene_curr: Scene::Data,
        }
    }

    fn select(&mut self, option: DataOption) {
        self.tab_status[self.option as usize] = TabStatus::Not;
        self.option = option;
        self.tab_status[option as usize] = TabStatus::Highlight;
    }

    /// `mnDataFuncRun`.
    pub fn tick(&mut self, pad: &Pad) -> Option<Scene> {
        self.total_tics += 1;
        if self.total_tics < 10 {
            return None;
        }
        if self.total_tics == self.return_tic {
            return Some(Scene::Title);
        }
        if !pad.no_input_all() {
            self.return_tic = self.total_tics + IDLE_RETURN;
        }
        if self.is_proceed_scene {
            // The US build returns at once.
            return Some(self.scene_curr);
        }
        if self.change_wait != 0 {
            self.change_wait -= 1;
        }
        if pad.released(UP | DOWN) {
            self.change_wait = 0;
        }
        if pad.tap(N64Buttons::A | N64Buttons::START) {
            sound::play_fgm(id::nSYAudioFGMMenuSelect);
            sound::stop_bgm_all();
            self.tab_status[self.option as usize] = TabStatus::Selected;
            self.scene_curr = match self.option {
                DataOption::Characters => Scene::Characters,
                DataOption::VsRecord => Scene::VsRecord,
                DataOption::SoundTest => Scene::SoundTest,
            };
            self.is_proceed_scene = true;
            return None;
        }
        if pad.tap(N64Buttons::B) {
            return Some(Scene::ModeSelect);
        }
        let mut r = Repeat::default();
        if r.check(self.change_wait, pad, UP, true, 20, true) {
            sound::play_fgm(id::nSYAudioFGMMenuScroll2);
            self.change_wait = r.wait_p(7);
            let next = if self.option == DataOption::Characters {
                self.last_option
            } else {
                DataOption::from_index(self.option as usize - 1)
            };
            self.select(next);
            if self.option == DataOption::Characters {
                self.change_wait += 8;
            }
        }
        if r.check(self.change_wait, pad, DOWN, true, -20, false) {
            sound::play_fgm(id::nSYAudioFGMMenuScroll2);
            self.change_wait = r.wait_n(7);
            let next = if self.option == self.last_option {
                DataOption::Characters
            } else {
                DataOption::from_index(self.option as usize + 1)
            };
            self.select(next);
            if self.option == self.last_option {
                self.change_wait += 8;
            }
        }
        None
    }

    /// The cameras back to front: decals (80), labels (60), then the tabs
    /// (40).
    pub fn visit(&self, f: &mut impl FnMut(Draw)) {
        decals(f, FILE_DATA, sprite::DATA_ICON_DARK);
        labels(
            f,
            [0xA0, 0x78, 0x14, 0xE6],
            FILE_DATA,
            sprite::DATA_TEXT,
            (206.0, 131.0),
        );
        // `mnDataMakeCharacters` and `mnDataMakeVSRecord`.
        let ((cx, cy), (vx, vy)) = if self.is_have_sound_test {
            ((133, 42), (101, 89))
        } else {
            ((113, 57), (81, 126))
        };
        option_tab(f, cx as f32, cy as f32, 16, self.tab_status[0]);
        f(Draw::Sprite(
            Piece::clear(
                FILE_DATA,
                sprite::CHARACTERS_TEXT,
                (cx + 26) as f32,
                (cy + 4) as f32,
            )
            .prim([0; 3]),
        ));
        option_tab(f, vx as f32, vy as f32, 16, self.tab_status[1]);
        f(Draw::Sprite(
            Piece::clear(
                FILE_DATA,
                sprite::VS_RECORD_TEXT,
                (vx + 27) as f32,
                (vy + 4) as f32,
            )
            .prim([0; 3]),
        ));
        if self.is_have_sound_test {
            option_tab(f, 69.0, 136.0, 16, self.tab_status[2]);
            f(Draw::Sprite(
                Piece::clear(FILE_DATA, sprite::SOUND_TEST_TEXT, 95.0, 140.0).prim([0; 3]),
            ));
        }
    }
}
