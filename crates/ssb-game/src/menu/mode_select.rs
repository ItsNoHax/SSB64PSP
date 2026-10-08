//! `mn/mncommon/mnmodeselect.c`: the main menu. 1P Mode, VS Mode, Option
//! and Data around a diagonal, the chosen one's bright icon, the others'
//! dark ones.
//!
//! A change ejects the four option GObjs and makes them again
//! (`mnModeSelectEjectOptions`, `mnModeSelectMakeOptions`), which appends
//! their displays after the labels' in display link 1: from the first
//! change on, the labels draw first. The US build draws no Japanese
//! subtitle or frame.

use super::{Draw, Pad, Piece, Repeat, Scene, IDLE_RETURN};
use crate::sound::{self, id};
use ssb_engine::input::N64Buttons;

/// `llMNMainFileID`.
pub const FILE_MAIN: u32 = 0x01;

/// `llMNMain*Sprite` (`reloc_data.us.h`).
pub mod sprite {
    pub const CONTROLLER_ICON: u32 = 0x1990;
    pub const CONSOLE_ICON: u32 = 0x2520;
    pub const DATA_ICON: u32 = 0x30B0;
    pub const MODE_SELECT_TEXT: u32 = 0x40F0;
    pub const SETTINGS_ICON: u32 = 0x4C80;
    pub const ONE_P_MODE_TEXT: u32 = 0x5570;
    pub const VS_MODE_TEXT: u32 = 0x57E0;
    pub const DATA_TEXT: u32 = 0x5980;
    pub const CONTROLLER_ICON_DARK: u32 = 0x6048;
    pub const CONSOLE_ICON_DARK: u32 = 0x6708;
    pub const DATA_ICON_DARK: u32 = 0x6DC8;
    pub const DECAL_BAR_EDGE: u32 = 0x72E8;
    pub const SMASH_LOGO: u32 = 0x7AA8;
    pub const DECAL_BAR_MIDDLE: u32 = 0x7C38;
    pub const SETTINGS_ICON_DARK: u32 = 0x82F8;
    pub const OPTION_TEXT: u32 = 0x84F8;
}

/// `nMNModeSelectOption*`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModeOption {
    OnePMode,
    VsMode,
    Option,
    Data,
}

impl ModeOption {
    const ALL: [ModeOption; 4] = [
        ModeOption::OnePMode,
        ModeOption::VsMode,
        ModeOption::Option,
        ModeOption::Data,
    ];

    /// Each option's bright icon, dark icon and corner.
    fn icon(self) -> (u32, u32, f32, f32) {
        match self {
            ModeOption::OnePMode => (
                sprite::CONTROLLER_ICON,
                sprite::CONTROLLER_ICON_DARK,
                169.0,
                27.0,
            ),
            ModeOption::VsMode => (sprite::CONSOLE_ICON, sprite::CONSOLE_ICON_DARK, 128.0, 64.0),
            ModeOption::Option => (
                sprite::SETTINGS_ICON,
                sprite::SETTINGS_ICON_DARK,
                87.0,
                101.0,
            ),
            ModeOption::Data => (sprite::DATA_ICON, sprite::DATA_ICON_DARK, 46.0, 138.0),
        }
    }

    fn scene(self) -> Scene {
        match self {
            ModeOption::OnePMode => Scene::OnePMode,
            ModeOption::VsMode => Scene::VsMode,
            ModeOption::Option => Scene::Option,
            ModeOption::Data => Scene::Data,
        }
    }
}

/// `U_JPAD | R_JPAD | U_CBUTTONS | R_CBUTTONS` and the opposite four.
const UP_RIGHT: u16 =
    N64Buttons::D_UP | N64Buttons::D_RIGHT | N64Buttons::C_UP | N64Buttons::C_RIGHT;
const DOWN_LEFT: u16 =
    N64Buttons::D_DOWN | N64Buttons::D_LEFT | N64Buttons::C_DOWN | N64Buttons::C_LEFT;

/// The menu's state (`sMNModeSelect*`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ModeSelect {
    pub option: ModeOption,
    change_wait: i32,
    total_tics: i32,
    return_tic: i32,
    /// The options were made again after the labels.
    remade: bool,
}

impl ModeSelect {
    /// `mnModeSelectFuncStart` (`mnModeSelectInitVars`).
    pub fn new(scene_prev: Scene) -> ModeSelect {
        let option = match scene_prev {
            Scene::VsMode => ModeOption::VsMode,
            Scene::Option => ModeOption::Option,
            Scene::Data => ModeOption::Data,
            _ => ModeOption::OnePMode,
        };
        if !matches!(
            scene_prev,
            Scene::OnePMode | Scene::VsMode | Scene::Option | Scene::Data
        ) {
            sound::play_bgm(0, id::nSYAudioBGMModeSelect);
        }
        ModeSelect {
            option,
            change_wait: 0,
            total_tics: 0,
            return_tic: IDLE_RETURN,
            remade: false,
        }
    }

    fn select(&mut self, option: ModeOption) {
        self.option = option;
        self.remade = true;
    }

    /// `mnModeSelectFuncRun`. Returns the scene to load.
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
        if self.change_wait != 0 {
            self.change_wait -= 1;
        }
        if pad.released(UP_RIGHT | DOWN_LEFT) {
            self.change_wait = 0;
        }
        if pad.tap(N64Buttons::A | N64Buttons::START) {
            sound::play_fgm(id::nSYAudioFGMMenuSelect);
            return Some(self.option.scene());
        }
        let mut load = None;
        if pad.tap(N64Buttons::B) {
            sound::stop_bgm_all();
            load = Some(Scene::Title);
        }
        // Not up-and-left or down-and-right at once.
        let diagonal = (pad.stick_ud(20, true) > 0 && pad.stick_lr(-20, false) < 0)
            || (pad.stick_ud(-20, false) < 0 && pad.stick_lr(20, true) > 0);
        if diagonal {
            return load;
        }
        let mut r = Repeat::default();
        if r.check(self.change_wait, pad, UP_RIGHT, true, 20, true)
            || (self.change_wait == 0 && r.check_stick(self.change_wait, pad, false, 20, true))
        {
            self.change_wait = r.wait_p(7);
            sound::play_fgm(id::nSYAudioFGMMenuScroll2);
            let i = self.option as usize;
            let next = if i == 0 { 3 } else { i - 1 };
            self.select(ModeOption::ALL[next]);
            if next == 0 {
                self.change_wait += 8;
            }
        }
        if r.check(self.change_wait, pad, DOWN_LEFT, true, -20, false)
            || (self.change_wait == 0 && r.check_stick(self.change_wait, pad, false, -20, false))
        {
            self.change_wait = r.wait_n(7);
            sound::play_fgm(id::nSYAudioFGMMenuScroll2);
            let i = self.option as usize;
            let next = if i == 3 { 0 } else { i + 1 };
            self.select(ModeOption::ALL[next]);
            if next == 3 {
                self.change_wait += 8;
            }
        }
        load
    }

    /// `mnModeSelectMakeOptions`' four icons.
    fn options(&self, f: &mut impl FnMut(Draw)) {
        for o in ModeOption::ALL {
            let (bright, dark, x, y) = o.icon();
            let p = if o == self.option {
                Piece::clear(FILE_MAIN, bright, x, y)
                    .prim([0xFF; 3])
                    .env([0; 3])
            } else {
                Piece::clear(FILE_MAIN, dark, x, y).prim([0x96; 3])
            };
            f(Draw::Sprite(p));
        }
    }

    /// `mnModeSelectMakeLabels`.
    fn labels(f: &mut impl FnMut(Draw)) {
        for (s, x, y) in [
            (sprite::ONE_P_MODE_TEXT, 224.0, 52.0),
            (sprite::VS_MODE_TEXT, 183.0, 89.0),
            (sprite::OPTION_TEXT, 142.0, 126.0),
            (sprite::DATA_TEXT, 102.0, 163.0),
        ] {
            f(Draw::Sprite(
                Piece::clear(FILE_MAIN, s, x, y).prim([0xFF, 0x00, 0x00]),
            ));
        }
    }

    /// The cameras back to front: the decals (80), then the options and
    /// labels (60).
    pub fn visit(&self, f: &mut impl FnMut(Draw)) {
        // `mnModeSelectMakeDecals`.
        f(Draw::Sprite(Piece::at(
            super::FILE_COMMON,
            super::common::SMASH_BROS_COLLAGE,
            10.0,
            10.0,
        )));
        let bar = [0x08, 0x33, 0x65];
        // `masks` 4 (16 texels) over `lrs` 96, `lrt` 38.
        f(Draw::Tiled {
            piece: Piece::clear(FILE_MAIN, sprite::DECAL_BAR_MIDDLE, 0.0, 37.0).prim(bar),
            size: [96.0, 38.0],
        });
        f(Draw::Sprite(
            Piece::clear(FILE_MAIN, sprite::DECAL_BAR_EDGE, 96.0, 37.0).prim(bar),
        ));
        f(Draw::Sprite(
            Piece::clear(FILE_MAIN, sprite::MODE_SELECT_TEXT, 28.0, 27.0)
                .env([0; 3])
                .prim([0x3C, 0x73, 0xB4]),
        ));
        f(Draw::Sprite(
            Piece::clear(FILE_MAIN, sprite::SMASH_LOGO, 226.0, 137.0).prim(bar),
        ));
        if self.remade {
            Self::labels(f);
            self.options(f);
        } else {
            self.options(f);
            Self::labels(f);
        }
    }
}
