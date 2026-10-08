//! `mn/mn1pmode/mn1pmode.c`: the 1P mode menu. 1P Game and Training Mode
//! on full option tabs, Bonus 1 and Bonus 2 Practice on the file's short
//! tab.
//!
//! `mn1PModeInitVars` has no default case: entered from the mode select,
//! the menu keeps the option it last had (the overlay's BSS), which the
//! host keeps for it. `mn1PModeFuncStart`'s anti-piracy check
//! (`gSYMainDmemOK`, `LBBACKUP_ERROR_HALFSTICKRANGE`) is not ported, as a
//! genuine console never fails it; the US build's subtitle GObj draws
//! nothing.

use super::{
    fill_prim, option_tab, Draw, Pad, Piece, Repeat, Scene, TabStatus, FILE_COMMON, IDLE_RETURN,
};
use crate::sound::{self, id};
use ssb_engine::input::N64Buttons;

/// `llMN1PFileID`.
pub const FILE_1P: u32 = 0x02;

/// `llMN1P*Sprite` and the `MNCommon` ones it draws.
pub mod sprite {
    pub const OPTION_TAB: u32 = 0x1108;
    pub const ONE_P_GAME_TEXT: u32 = 0x2A28;
    pub const CONTROLLER_ICON_DARK: u32 = 0x50F8;
    pub const ONE_P_TEXT: u32 = 0x5338;
    pub const TRAINING_MODE_TEXT: u32 = 0x5AC8;
    pub const BONUS_1_PRACTICE_TEXT: u32 = 0x5F28;
    pub const BONUS_2_PRACTICE_TEXT: u32 = 0x6388;
    /// `llMNCommonGameModeTextSprite`.
    pub const GAME_MODE_TEXT: u32 = 0xD240;
}

/// `nMN1PModeOption*`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OnePOption {
    #[default]
    OnePGame,
    TrainingMode,
    Bonus1Practice,
    Bonus2Practice,
}

impl OnePOption {
    const ALL: [OnePOption; 4] = [
        OnePOption::OnePGame,
        OnePOption::TrainingMode,
        OnePOption::Bonus1Practice,
        OnePOption::Bonus2Practice,
    ];

    fn scene(self) -> Scene {
        match self {
            OnePOption::OnePGame => Scene::Players1PGame,
            OnePOption::TrainingMode => Scene::Players1PTraining,
            OnePOption::Bonus1Practice => Scene::Players1PBonus1,
            OnePOption::Bonus2Practice => Scene::Players1PBonus2,
        }
    }
}

/// The menu's state (`sMN1PMode*`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OnePMode {
    pub option: OnePOption,
    is_proceed_scene: bool,
    change_wait: i32,
    total_tics: i32,
    return_tic: i32,
    /// Each option's colours as `mn1PModeSetOptionSpriteColors` last set
    /// them.
    pub tab_status: [TabStatus; 4],
    scene_curr: Scene,
}

impl OnePMode {
    /// `mn1PModeFuncStart` (`mn1PModeInitVars`): the select the menu came
    /// back from, else the option it last had.
    pub fn new(scene_prev: Scene, last: OnePOption) -> OnePMode {
        let option = match scene_prev {
            Scene::Players1PGame => OnePOption::OnePGame,
            Scene::Players1PTraining => OnePOption::TrainingMode,
            Scene::Players1PBonus1 => OnePOption::Bonus1Practice,
            Scene::Players1PBonus2 => OnePOption::Bonus2Practice,
            _ => last,
        };
        if scene_prev != Scene::ModeSelect {
            sound::play_bgm(0, id::nSYAudioBGMModeSelect);
        }
        OnePMode {
            option,
            is_proceed_scene: false,
            change_wait: 0,
            total_tics: 0,
            return_tic: IDLE_RETURN,
            tab_status: OnePOption::ALL.map(|o| TabStatus::of(o == option)),
            scene_curr: Scene::OnePMode,
        }
    }

    fn select(&mut self, option: OnePOption) {
        self.option = option;
        self.tab_status[option as usize] = TabStatus::Highlight;
    }

    /// `mn1PModeFuncRun`. Returns the scene to load.
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
        let mut load = self.is_proceed_scene;
        if self.change_wait != 0 {
            self.change_wait -= 1;
        }
        if pad
            .released(N64Buttons::D_UP | N64Buttons::C_UP | N64Buttons::D_DOWN | N64Buttons::C_DOWN)
        {
            self.change_wait = 0;
        }
        if pad.tap(N64Buttons::A | N64Buttons::START) {
            sound::play_fgm(id::nSYAudioFGMMenuSelect);
            self.tab_status[self.option as usize] = TabStatus::Selected;
            self.scene_curr = self.option.scene();
            self.is_proceed_scene = true;
            return load.then_some(self.scene_curr);
        }
        if pad.tap(N64Buttons::B) {
            self.scene_curr = Scene::ModeSelect;
            load = true;
        }
        let mut r = Repeat::default();
        if r.check(
            self.change_wait,
            pad,
            N64Buttons::D_UP | N64Buttons::C_UP,
            true,
            20,
            true,
        ) {
            sound::play_fgm(id::nSYAudioFGMMenuScroll2);
            self.change_wait = r.wait_p(7);
            self.tab_status[self.option as usize] = TabStatus::Not;
            let i = self.option as usize;
            let next = if i == 0 { 3 } else { i - 1 };
            if next == 0 {
                self.change_wait += 8;
            }
            self.select(OnePOption::ALL[next]);
        }
        if r.check(
            self.change_wait,
            pad,
            N64Buttons::D_DOWN | N64Buttons::C_DOWN,
            true,
            -20,
            false,
        ) {
            sound::play_fgm(id::nSYAudioFGMMenuScroll2);
            self.change_wait = r.wait_n(7);
            self.tab_status[self.option as usize] = TabStatus::Not;
            let i = self.option as usize;
            let next = if i == 3 { 0 } else { i + 1 };
            if next == 3 {
                self.change_wait += 8;
            }
            self.select(OnePOption::ALL[next]);
        }
        load.then_some(self.scene_curr)
    }

    /// The cameras back to front: the decals (80), the labels (60), then
    /// the options (40).
    pub fn visit(&self, f: &mut impl FnMut(Draw)) {
        // `mn1PModeMakeDecals`.
        super::decals(f, FILE_1P, sprite::CONTROLLER_ICON_DARK);
        // `mn1PModeMakeLabels` (`mn1PModeLabelsProcDisplay`).
        f(fill_prim(225, 143, 310, 230, [0xA0, 0x78, 0x14, 0xE6]));
        for (file, s, x, y) in [
            (FILE_COMMON, super::common::SMASH_LOGO, 235.0, 158.0),
            (FILE_1P, sprite::ONE_P_TEXT, 161.0, 194.0),
            (FILE_COMMON, sprite::GAME_MODE_TEXT, 188.0, 88.0),
        ] {
            f(Draw::Sprite(Piece::clear(file, s, x, y).prim([0; 3])));
        }
        // `mn1PModeMake1PGame` and `...TrainingMode`: full tabs.
        option_tab(f, 124.0, 42.0, 16, self.tab_status[0]);
        f(Draw::Sprite(
            Piece::clear(FILE_1P, sprite::ONE_P_GAME_TEXT, 161.0, 46.0).prim([0; 3]),
        ));
        option_tab(f, 99.0, 84.0, 16, self.tab_status[1]);
        f(Draw::Sprite(
            Piece::clear(FILE_1P, sprite::TRAINING_MODE_TEXT, 107.0, 87.0).prim([0; 3]),
        ));
        // `mn1PModeMakeBonus1Practice` and `...Bonus2Practice`: the one
        // short tab, coloured alone.
        for (i, (tx, ty, text, lx, ly)) in [
            (78.0, 126.0, sprite::BONUS_1_PRACTICE_TEXT, 97.0, 127.0),
            (67.0, 148.0, sprite::BONUS_2_PRACTICE_TEXT, 86.0, 149.0),
        ]
        .into_iter()
        .enumerate()
        {
            let (env, prim) = self.tab_status[2 + i].colors();
            f(Draw::Sprite(
                Piece::clear(FILE_1P, sprite::OPTION_TAB, tx, ty)
                    .env(env)
                    .prim(prim),
            ));
            f(Draw::Sprite(
                Piece::clear(FILE_1P, text, lx, ly).prim([0; 3]),
            ));
        }
    }
}
