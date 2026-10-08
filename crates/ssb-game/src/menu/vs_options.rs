//! `mn/mnvsmode/mnvsoptions.c`: VS Options. Handicap (on, auto, off),
//! Team Attack and Stage Select (on, off), the damage ratio (50% to 200%),
//! and Item Switch once the backup unlocks it
//! (`mnVSOptionsCheckHaveItemSwitch`); the rows move down when Item Switch
//! is missing. The settings go to `gSCManagerTransferBattleState` on the
//! way out (`mnVSOptionsSetAllSettings`); a handicap change sets every
//! player's handicap at once (`mnVSOptionsSetHandicapSettings`).
//!
//! The US build's `mnVSOptionsMakeSubtitle` adds no display, so the
//! Japanese subtitle is not drawn.

use super::{
    common, fill, fill_prim, right_digits, Draw, Pad, Piece, Repeat, Scene, DOWN, FILE_COMMON,
    FILE_VS_OPTIONS, IDLE_RETURN, LEFT, RIGHT, UP,
};
use crate::backup::Backup;
use crate::players_vs::{BattleState, Handicap};
use crate::sound::{self, id};
use crate::spgame::Unlock;
use ssb_engine::input::N64Buttons;

/// `llMNVSOptions*Sprite` (`reloc_data.us.h`).
pub mod sprite {
    pub const VS_OPTIONS_TEXT: u32 = 0x2668;
    pub const BUBBLE: u32 = 0x33D8;
    pub const HANDICAP_TEXT: u32 = 0x3690;
    pub const TEAM_ATTACK_TEXT: u32 = 0x3968;
    pub const STAGE_SELECT_TEXT: u32 = 0x3CF8;
    pub const ITEM_SWITCH_TEXT: u32 = 0x3FC8;
    pub const DAMAGE_TEXT: u32 = 0x4228;
    pub const CONSOLE_ICON_DARK: u32 = 0x5F60;
}

/// `FTCOMMON_HANDICAP_DEFAULT`.
const HANDICAP_DEFAULT: u8 = 9;
/// `mnVSOptionsSetHandicapSettings`' handicap under Auto.
const HANDICAP_AUTO: u8 = 5;
/// The damage ratio's range.
const DAMAGE_MIN: i32 = 50;
const DAMAGE_MAX: i32 = 200;

/// `nMNVSOptionsOption*`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum VsOption {
    Handicap,
    TeamAttack,
    StageSelect,
    Damage,
    ItemSwitch,
}

impl VsOption {
    const ALL: [VsOption; 5] = [
        VsOption::Handicap,
        VsOption::TeamAttack,
        VsOption::StageSelect,
        VsOption::Damage,
        VsOption::ItemSwitch,
    ];
}

/// The menu's state (`sMNVSOptions*`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VsOptionsMenu {
    pub option: VsOption,
    pub handicap: Handicap,
    pub team_attack: bool,
    pub stage_select: bool,
    /// `sMNVSOptionsDamage`: 50 to 200.
    pub damage: i32,
    /// `sMNVSOptionsIsHaveItemSwitch`.
    pub is_have_item_switch: bool,
    last_option: VsOption,
    change_wait: i32,
    total_tics: i32,
    return_tic: i32,
    scene_curr: Scene,
    /// `sMNVSOptionsDamageGObj` was remade: its display now follows Item
    /// Switch's.
    damage_remade: bool,
}

impl VsOptionsMenu {
    /// `mnVSOptionsFuncStart` (`mnVSOptionsInitVars`): Item Switch
    /// highlighted when coming back from it, the settings from
    /// `gSCManagerTransferBattleState`, and Item Switch only once the
    /// backup has `LBBACKUP_UNLOCK_MASK_ITEMSWITCH`.
    pub fn new(scene_prev: Scene, state: &BattleState, backup: &Backup) -> VsOptionsMenu {
        let is_have_item_switch = backup.unlock_mask & Unlock::ItemSwitch.mask() != 0;
        VsOptionsMenu {
            option: if scene_prev == Scene::VsItemSwitch {
                VsOption::ItemSwitch
            } else {
                VsOption::Handicap
            },
            handicap: state.handicap,
            team_attack: state.is_team_attack,
            stage_select: state.is_stage_select,
            damage: i32::from(state.damage_ratio),
            is_have_item_switch,
            last_option: if is_have_item_switch {
                VsOption::ItemSwitch
            } else {
                VsOption::Damage
            },
            change_wait: 0,
            total_tics: 0,
            return_tic: IDLE_RETURN,
            scene_curr: Scene::VsOptions,
            damage_remade: false,
        }
    }

    /// `mnVSOptionsSetAllSettings`.
    fn set_all_settings(&self, state: &mut BattleState) {
        state.handicap = self.handicap;
        state.is_team_attack = self.team_attack;
        state.is_stage_select = self.stage_select;
        state.damage_ratio = self.damage as u8;
        if state.handicap == Handicap::Off {
            for p in state.players.iter_mut() {
                p.handicap = HANDICAP_DEFAULT;
            }
        }
    }

    /// `mnVSOptionsSetHandicapSettings`.
    fn set_handicap_settings(&self, state: &mut BattleState) {
        let h = if self.handicap == Handicap::Auto {
            HANDICAP_AUTO
        } else {
            HANDICAP_DEFAULT
        };
        for p in state.players.iter_mut() {
            p.handicap = h;
        }
    }

    fn step(&self, down: bool) -> VsOption {
        let i = self.option as usize;
        if down {
            if self.option == self.last_option {
                VsOption::Handicap
            } else {
                VsOption::ALL[i + 1]
            }
        } else if self.option == VsOption::Handicap {
            self.last_option
        } else {
            VsOption::ALL[i - 1]
        }
    }

    /// `mnVSOptionsFuncRun`. Returns the scene `syTaskmanSetLoadScene`
    /// loads; the rest of the frame still runs, as the original's does.
    pub fn tick(&mut self, pad: &Pad, state: &mut BattleState) -> Option<Scene> {
        self.total_tics += 1;
        if self.total_tics < 10 {
            return None;
        }
        if self.total_tics == self.return_tic {
            self.scene_curr = Scene::Title;
            self.set_all_settings(state);
            return Some(Scene::Title);
        }
        if !pad.no_input_all() {
            self.return_tic = self.total_tics + IDLE_RETURN;
        }
        let mut load = false;
        if self.change_wait != 0 {
            self.change_wait -= 1;
        }
        if pad.released(UP | RIGHT | DOWN | LEFT) {
            self.change_wait = 0;
        }
        if pad.tap(N64Buttons::A | N64Buttons::START) && self.option == VsOption::ItemSwitch {
            sound::play_fgm(id::nSYAudioFGMMenuSelect);
            self.scene_curr = Scene::VsItemSwitch;
            self.set_all_settings(state);
            load = true;
        }
        if pad.tap(N64Buttons::B) {
            self.scene_curr = Scene::VsMode;
            self.set_all_settings(state);
            load = true;
        }
        let mut r = Repeat::default();
        if r.check(self.change_wait, pad, UP, true, 20, true) {
            sound::play_fgm(id::nSYAudioFGMMenuScroll2);
            self.change_wait = r.wait_p(7);
            self.option = self.step(false);
            if self.option == VsOption::Handicap {
                self.change_wait += 8;
            }
        }
        if r.check(self.change_wait, pad, DOWN, true, -20, false) {
            sound::play_fgm(id::nSYAudioFGMMenuScroll2);
            self.change_wait = r.wait_n(7);
            self.option = self.step(true);
            if self.option == self.last_option {
                self.change_wait += 8;
            }
        }
        if r.check(self.change_wait, pad, LEFT, false, -20, false) {
            match self.option {
                VsOption::Handicap => {
                    if self.handicap != Handicap::On {
                        self.handicap = if self.handicap == Handicap::Off {
                            Handicap::Auto
                        } else {
                            Handicap::On
                        };
                        self.set_handicap_settings(state);
                        sound::play_fgm(id::nSYAudioFGMMenuScroll1);
                    }
                    self.change_wait = r.wait_n(7);
                }
                VsOption::TeamAttack => {
                    if !self.team_attack {
                        self.team_attack = true;
                        sound::play_fgm(id::nSYAudioFGMMenuScroll1);
                    }
                    self.change_wait = r.wait_n(7);
                }
                VsOption::StageSelect => {
                    if !self.stage_select {
                        self.stage_select = true;
                        sound::play_fgm(id::nSYAudioFGMMenuScroll1);
                    }
                    self.change_wait = r.wait_n(7);
                }
                VsOption::Damage => {
                    sound::play_fgm(id::nSYAudioFGMMenuScroll1);
                    self.damage = if self.damage == DAMAGE_MIN {
                        DAMAGE_MAX
                    } else {
                        self.damage - 1
                    };
                    self.change_wait = r.wait_n(14);
                    if self.damage == DAMAGE_MIN {
                        self.change_wait += 8;
                    }
                    self.damage_remade = true;
                }
                VsOption::ItemSwitch => {}
            }
        }
        if r.check(self.change_wait, pad, RIGHT, false, 20, true) {
            match self.option {
                VsOption::Handicap => {
                    if self.handicap != Handicap::Off {
                        self.handicap = if self.handicap == Handicap::On {
                            Handicap::Auto
                        } else {
                            Handicap::Off
                        };
                        self.set_handicap_settings(state);
                        sound::play_fgm(id::nSYAudioFGMMenuScroll1);
                    }
                    self.change_wait = r.wait_p(7);
                }
                VsOption::TeamAttack => {
                    if self.team_attack {
                        self.team_attack = false;
                        sound::play_fgm(id::nSYAudioFGMMenuScroll1);
                    }
                    self.change_wait = r.wait_p(7);
                }
                VsOption::StageSelect => {
                    if self.stage_select {
                        self.stage_select = false;
                        sound::play_fgm(id::nSYAudioFGMMenuScroll1);
                    }
                    self.change_wait = r.wait_p(7);
                }
                VsOption::Damage => {
                    self.damage = if self.damage == DAMAGE_MAX {
                        DAMAGE_MIN
                    } else {
                        self.damage + 1
                    };
                    self.change_wait = r.wait_p(14);
                    if self.damage == DAMAGE_MAX {
                        self.change_wait += 8;
                    }
                    sound::play_fgm(id::nSYAudioFGMMenuScroll1);
                    self.damage_remade = true;
                }
                VsOption::ItemSwitch => {}
            }
        }
        if pad.tap(N64Buttons::A) {
            match self.option {
                VsOption::Handicap => {
                    sound::play_fgm(id::nSYAudioFGMMenuScroll1);
                    self.handicap = match self.handicap {
                        Handicap::Off => Handicap::On,
                        Handicap::Auto => Handicap::Off,
                        Handicap::On => Handicap::Auto,
                    };
                    self.set_handicap_settings(state);
                }
                VsOption::TeamAttack => {
                    sound::play_fgm(id::nSYAudioFGMMenuScroll1);
                    self.team_attack = !self.team_attack;
                }
                VsOption::StageSelect => {
                    sound::play_fgm(id::nSYAudioFGMMenuScroll1);
                    self.stage_select = !self.stage_select;
                }
                _ => {}
            }
        }
        load.then_some(self.scene_curr)
    }

    /// The rows' y: `(handicap, team attack, stage select, damage)`, and
    /// the damage digits' y.
    fn rows(&self) -> ([f32; 4], f32) {
        if self.is_have_item_switch {
            ([61.0, 90.0, 119.0, 148.0], 151.0)
        } else {
            ([65.0, 97.0, 129.0, 161.0], 164.0)
        }
    }

    /// The cameras back to front: the wallpaper (80), the tint (70), the
    /// decal (60), the options (50), then the label and underline (40).
    /// Every camera has the viewport `(10, 10)` to `(310, 230)`; the
    /// default camera (100) clears nothing.
    pub fn visit(&self, f: &mut impl FnMut(Draw)) {
        // `mnVSOptionsMakeWallpaper`.
        f(Draw::Sprite(Piece::at(
            FILE_COMMON,
            common::SMASH_BROS_COLLAGE,
            10.0,
            10.0,
        )));
        // `mnVSOptionsTintProcDisplay`.
        f(fill_prim(10, 10, 310, 230, [0x0D, 0x00, 0x00, 0x99]));
        // `mnVSOptionsMakeDecal`.
        f(Draw::Sprite(
            Piece::clear(FILE_VS_OPTIONS, sprite::CONSOLE_ICON_DARK, 10.0, 10.0)
                .prim([0x4A, 0x2A, 0x23]),
        ));
        // Display link 2, in GObj order.
        let (y, digits_y) = self.rows();
        self.handicap_option(f, y[0]);
        self.team_attack_option(f, y[1]);
        self.stage_select_option(f, y[2]);
        self.damage_option(f, y[3]);
        if !self.damage_remade {
            right_digits(f, self.damage, 220.0, digits_y, [0xFF, 0x00, 0x28], 3);
        }
        if self.is_have_item_switch {
            self.bubble(f, VsOption::ItemSwitch, 82.0, 177.0);
            text(f, sprite::ITEM_SWITCH_TEXT, 128.0, 179.0);
        }
        if self.damage_remade {
            right_digits(f, self.damage, 220.0, digits_y, [0xFF, 0x00, 0x28], 3);
        }
        // `mnVSOptionsLabelProcDisplay`.
        f(fill_prim(79, 34, 310, 39, [0x80, 0x80, 0x80, 0xFF]));
        f(Draw::Sprite(
            Piece::clear(FILE_VS_OPTIONS, sprite::VS_OPTIONS_TEXT, 84.0, 24.0)
                .prim([0xF2, 0xC7, 0x0D]),
        ));
        self.underline(f);
    }

    /// `mnVSOptionsSetOptionSpriteColors` on the option's bubble.
    fn bubble(&self, f: &mut impl FnMut(Draw), option: VsOption, x: f32, y: f32) {
        let (env, prim) = if self.option == option {
            ([0xFA, 0x8C, 0x00], [0xF4, 0xC8, 0x0A])
        } else {
            ([0x00; 3], [0x82, 0x82, 0xAA])
        };
        f(Draw::Sprite(
            Piece::clear(FILE_VS_OPTIONS, sprite::BUBBLE, x, y)
                .env(env)
                .prim(prim),
        ));
    }

    /// `mnVSOptionsMakeHandicapOption` (`mnVSOptionsMakeHandicapToggle`,
    /// `mnVSOptionsSetHandicapSpriteColors`).
    fn handicap_option(&self, f: &mut impl FnMut(Draw), y: f32) {
        self.bubble(f, VsOption::Handicap, 114.0, y);
        text(f, sprite::HANDICAP_TEXT, 121.0, y + 2.0);
        let pick = |h| {
            if self.handicap == h {
                PICKED
            } else {
                UNPICKED
            }
        };
        let x = 191.0;
        let y = y + 1.0;
        toggle_word(f, common::ON_TEXT, x, y, pick(Handicap::On));
        toggle_word(
            f,
            common::AUTO_TEXT,
            x + 30.0,
            y + 1.0,
            pick(Handicap::Auto),
        );
        toggle_word(f, common::OFF_TEXT, x + 66.0, y, pick(Handicap::Off));
        toggle_word(f, common::SLASH, x + 25.0, y, UNPICKED);
        toggle_word(f, common::SLASH, x + 60.0, y, UNPICKED);
    }

    /// `mnVSOptionsMakeTeamAttackOption`.
    fn team_attack_option(&self, f: &mut impl FnMut(Draw), y: f32) {
        self.bubble(f, VsOption::TeamAttack, 106.0, y);
        text(f, sprite::TEAM_ATTACK_TEXT, 116.0, y + 2.0);
        on_off(f, 212.0, y + 1.0, self.team_attack);
    }

    /// `mnVSOptionsMakeStageSelectOption`.
    fn stage_select_option(&self, f: &mut impl FnMut(Draw), y: f32) {
        self.bubble(f, VsOption::StageSelect, 98.0, y);
        text(f, sprite::STAGE_SELECT_TEXT, 104.0, y + 1.0);
        on_off(f, 208.0, y + 1.0, self.stage_select);
    }

    /// `mnVSOptionsMakeDamageOption`.
    fn damage_option(&self, f: &mut impl FnMut(Draw), y: f32) {
        self.bubble(f, VsOption::Damage, 90.0, y);
        text(f, sprite::DAMAGE_TEXT, 116.0, y + 1.0);
        f(Draw::Sprite(
            Piece::clear(FILE_COMMON, common::PERCENTAGE, 226.0, y + 3.0).prim([0; 3]),
        ));
    }

    /// `mnVSOptionsUnderlineProcDisplay`: a `G_CYC_FILL` line under the
    /// highlighted toggle's setting.
    fn underline(&self, f: &mut impl FnMut(Draw)) {
        let have = self.is_have_item_switch;
        let (r, off_y) = match self.option {
            VsOption::Handicap => (
                [[255, 77, 283, 77], [190, 77, 216, 77], [219, 77, 251, 77]]
                    [self.handicap as usize],
                if have { 0 } else { 4 },
            ),
            VsOption::TeamAttack => (
                [[245, 106, 272, 106], [213, 106, 239, 106]][usize::from(self.team_attack)],
                if have { 0 } else { 7 },
            ),
            VsOption::StageSelect => (
                [[241, 135, 269, 135], [208, 135, 234, 135]][usize::from(self.stage_select)],
                if have { 0 } else { 10 },
            ),
            _ => return,
        };
        f(fill(
            r[0],
            r[1] + off_y,
            r[2],
            r[3] + off_y,
            [0xFF, 0x00, 0x28],
        ));
    }
}

/// A selected toggle word's colour, and the others'.
const PICKED: [u8; 3] = [0xFF, 0x00, 0x28];
const UNPICKED: [u8; 3] = [0x32, 0x32, 0x32];

fn text(f: &mut impl FnMut(Draw), offset: u32, x: f32, y: f32) {
    f(Draw::Sprite(
        Piece::clear(FILE_VS_OPTIONS, offset, x, y).prim([0; 3]),
    ));
}

fn toggle_word(f: &mut impl FnMut(Draw), offset: u32, x: f32, y: f32, rgb: [u8; 3]) {
    f(Draw::Sprite(
        Piece::clear(FILE_COMMON, offset, x, y).prim(rgb),
    ));
}

/// `mnVSOptionsMakeOnOffToggle` with `mnVSOptionsSetToggleSpriteColors`.
fn on_off(f: &mut impl FnMut(Draw), x: f32, y: f32, on: bool) {
    let (on_rgb, off_rgb) = if on {
        (PICKED, UNPICKED)
    } else {
        (UNPICKED, PICKED)
    };
    toggle_word(f, common::ON_TEXT, x, y, on_rgb);
    toggle_word(f, common::OFF_TEXT, x + 32.0, y, off_rgb);
    toggle_word(f, common::SLASH, x + 25.0, y, UNPICKED);
}
