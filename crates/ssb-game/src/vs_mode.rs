//! The VS mode menu — `mn/mnvsmode/mnvsmode.c`: four buttons (VS Start,
//! Rule, Time/Stock, VS Options), the rule (time, stock, and their team
//! variants), the time limit (1 to 99 minutes or infinite) and the stock
//! count (1 to 99), with the menus' held-input repeat (`mnCommon*` in
//! `mndef.h`), the blinking arrows beside the highlighted value, and the
//! battle settings written back on the way out (`mnVSModeSaveSettings`).
//!
//! [`VsMode::tick`] is one `gcRunAll`: `mnVSModeMain` (the scene GObj's
//! `func_run`), then the two arrow processes in the order they were made
//! (`mnVSModeAnimateRuleArrows`, `mnVSModeAnimateTimeStockArrows`).
//! [`VsMode::visit`] is the `gcDrawAll` after it.
//!
//! The US build's `mnVSModeMakeSubtitle` adds no display (the Japanese
//! subtitle under the buttons is not drawn).

use ssb_engine::input::N64Buttons;

use crate::battle::{Rule, TIMELIMIT_INFINITE};
use crate::costume::{costume_common_id, costume_team_id};
use crate::menu::{
    common, decals, labels, option_tab, right_digits, Draw, Pad, Piece, Repeat, Scene, TabStatus,
    DOWN, FILE_COMMON, FILE_VS_MODE, IDLE_RETURN, LEFT, RIGHT, UP,
};
use crate::players_vs::{BattleState, PLAYERS};
use crate::sound::{self, id};

/// `llMNVSMode*Sprite` (`reloc_data.us.h`).
pub mod sprite {
    pub const VS_START_TEXT: u32 = 0x24C8;
    pub const RULE_PERIOD_TEXT: u32 = 0x2748;
    pub const TIME_TEXT: u32 = 0x28E0;
    pub const STOCK_TEXT: u32 = 0x2A80;
    pub const TEAM_TEXT: u32 = 0x2C20;
    pub const TIME_PERIOD_TEXT: u32 = 0x2EC8;
    pub const MIN_TEXT: u32 = 0x2FC8;
    pub const STOCK_PERIOD_TEXT: u32 = 0x3248;
    pub const VS_OPTIONS_TEXT: u32 = 0x3828;
    pub const CONSOLE_ICON_DARK: u32 = 0x5EB0;
    pub const VS_TEXT: u32 = 0x6118;
}

/// `nMNVSModeOption*`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Button {
    Start,
    Rule,
    TimeStock,
    Options,
}

impl Button {
    const ALL: [Button; 4] = [
        Button::Start,
        Button::Rule,
        Button::TimeStock,
        Button::Options,
    ];
}

/// `nMNVSModeRule*`, in order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum VsRule {
    Time,
    Stock,
    TimeTeam,
    StockTeam,
}

impl VsRule {
    /// `mnVSModeFuncStartVars`' rule from the battle settings.
    pub fn of(state: &BattleState) -> VsRule {
        match (state.rule, state.is_team_battle) {
            (Rule::Time, false) => VsRule::Time,
            (Rule::Stock, false) => VsRule::Stock,
            (Rule::Time, true) => VsRule::TimeTeam,
            (Rule::Stock, true) => VsRule::StockTeam,
        }
    }

    /// `mnVSModeIsTime`.
    pub fn is_time(self) -> bool {
        matches!(self, VsRule::Time | VsRule::TimeTeam)
    }

    pub fn is_team(self) -> bool {
        matches!(self, VsRule::TimeTeam | VsRule::StockTeam)
    }

    /// The battle rule `mnVSModeSaveSettings` writes.
    pub fn battle_rule(self) -> Rule {
        if self.is_time() {
            Rule::Time
        } else {
            Rule::Stock
        }
    }

    fn prev(self) -> VsRule {
        match self {
            VsRule::Stock => VsRule::Time,
            VsRule::TimeTeam => VsRule::Stock,
            VsRule::StockTeam => VsRule::TimeTeam,
            VsRule::Time => VsRule::Time,
        }
    }

    fn next(self) -> VsRule {
        match self {
            VsRule::Time => VsRule::Stock,
            VsRule::Stock => VsRule::TimeTeam,
            VsRule::TimeTeam => VsRule::StockTeam,
            VsRule::StockTeam => VsRule::StockTeam,
        }
    }
}

/// An arrow `SObj` of an arrows GObj (`user_data.s`: 0 left, 1 right).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arrow {
    Left,
    Right,
}

/// `sMNVSModeInputDirection`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Direction {
    None,
    Up,
    Down,
    Left,
    Right,
}

/// `mnVSModeAnimate*Arrows`' blink period, in ticks.
const BLINK: i32 = 30;

/// The menu's state (`sMNVSMode*`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VsMode {
    pub cursor: Button,
    pub rule: VsRule,
    /// Minutes, or [`TIMELIMIT_INFINITE`].
    pub time: u8,
    /// Stocks less one: 0 to 98.
    pub stock: u8,
    /// Each button's colours as `mnVSModeUpdateButton` last set them.
    pub button_status: [TabStatus; 4],
    change_wait: i32,
    direction: Direction,
    total_tics: i32,
    return_tic: i32,
    /// `sMNVSModeExitInterrupt`: VS Start or VS Options was chosen; the
    /// next tick loads the scene.
    exit_interrupt: bool,
    scene_curr: Scene,
    /// `sMNVSModeRuleArrowsGObj`: its `SObj`s in list order, and
    /// `GOBJ_FLAG_HIDDEN`.
    rule_arrows: [Option<Arrow>; 2],
    pub rule_arrows_hidden: bool,
    rule_blink: i32,
    /// `sMNVSModeTimeStockArrowsGObj`'s `GOBJ_FLAG_HIDDEN`.
    pub time_stock_arrows_hidden: bool,
    time_stock_blink: i32,
    /// The Time/Stock button and its arrows were remade after a rule
    /// change: their GObjs (and display links) now follow VS Options'.
    time_stock_remade: bool,
}

impl VsMode {
    /// `mnVSModeFuncStart` (`mnVSModeFuncStartVars`): the cursor on VS
    /// Options when coming back from it, the rule, time and stocks from
    /// `gSCManagerTransferBattleState`; `nSYAudioBGMModeSelect` when the
    /// scene comes back from the VS character select.
    pub fn new(scene_prev: Scene, state: &BattleState) -> VsMode {
        if scene_prev == Scene::PlayersVs {
            sound::play_bgm(0, id::nSYAudioBGMModeSelect);
        }
        let cursor = if scene_prev == Scene::VsOptions {
            Button::Options
        } else {
            Button::Start
        };
        VsMode {
            cursor,
            rule: VsRule::of(state),
            time: state.time_limit,
            stock: state.stocks,
            button_status: Button::ALL.map(|b| TabStatus::of(b == cursor)),
            change_wait: 0,
            direction: Direction::None,
            total_tics: 0,
            return_tic: IDLE_RETURN,
            exit_interrupt: false,
            scene_curr: Scene::VsMode,
            rule_arrows: [None; 2],
            rule_arrows_hidden: false,
            rule_blink: 0,
            time_stock_arrows_hidden: false,
            time_stock_blink: 0,
            time_stock_remade: false,
        }
    }

    /// The battle's stock count (`gSCManagerTransferBattleState.stocks`).
    pub fn stocks(&self) -> i8 {
        self.stock as i8
    }

    /// `mnVSModeGetTimeStockValue`.
    pub fn time_stock_value(&self) -> i32 {
        if self.rule.is_time() {
            i32::from(self.time)
        } else {
            i32::from(self.stock) + 1
        }
    }

    /// `mnVSModeSaveSettings`.
    fn save_settings(&self, state: &mut BattleState) {
        state.time_limit = self.time;
        state.stocks = self.stock;
        state.is_team_battle = self.rule.is_team();
        state.rule = self.rule.battle_rule();
    }

    fn set_button(&mut self, b: Button, status: TabStatus) {
        self.button_status[b as usize] = status;
    }

    /// `mnVSModeMakeTimeStockButton` (with `mnVSModeMakeTimeStockArrows`)
    /// after `gcEjectGObj`: a fresh, visible arrows GObj at the end of its
    /// link.
    fn remake_time_stock(&mut self) {
        self.time_stock_remade = true;
        self.time_stock_arrows_hidden = false;
    }

    /// One frame: `mnVSModeMain`, then the arrow processes. Returns the
    /// scene `syTaskmanSetLoadScene` loads; the settings are already in
    /// `state` (`mnVSModeSaveSettings`).
    pub fn tick(&mut self, pad: &Pad, state: &mut BattleState) -> Option<Scene> {
        let load = self.main(pad, state);
        self.animate_rule_arrows();
        self.animate_time_stock_arrows();
        load.then_some(self.scene_curr)
    }

    /// `mnVSModeMain`: whether it called `syTaskmanSetLoadScene`.
    fn main(&mut self, pad: &Pad, state: &mut BattleState) -> bool {
        self.total_tics += 1;
        if self.total_tics < 10 {
            return false;
        }
        if self.total_tics == self.return_tic {
            self.scene_curr = Scene::Title;
            self.save_settings(state);
            return true;
        }
        if !pad.no_input_all() {
            self.return_tic = self.total_tics + IDLE_RETURN;
        }
        let mut load = self.exit_interrupt;
        if self.change_wait != 0 {
            self.change_wait -= 1;
        }
        if pad.released(UP | RIGHT | DOWN | LEFT) {
            self.change_wait = 0;
            self.direction = Direction::None;
        }
        if pad.tap(N64Buttons::A | N64Buttons::START) {
            let next = match self.cursor {
                Button::Start => Some(Scene::PlayersVs),
                Button::Options => Some(Scene::VsOptions),
                _ => None,
            };
            if let Some(next) = next {
                sound::play_fgm(id::nSYAudioFGMMenuSelect);
                self.set_button(self.cursor, TabStatus::Selected);
                self.save_settings(state);
                self.exit_interrupt = true;
                self.scene_curr = next;
                return load;
            }
        }
        if pad.tap(N64Buttons::B) {
            self.save_settings(state);
            self.scene_curr = Scene::ModeSelect;
            load = true;
        }
        let mut r = Repeat::default();
        if r.check(self.change_wait, pad, UP, true, 20, true) {
            sound::play_fgm(id::nSYAudioFGMMenuScroll2);
            self.change_wait = r.wait_p(7);
            if self.cursor == Button::Rule {
                self.change_wait += 8;
            }
            self.set_button(self.cursor, TabStatus::Not);
            self.cursor = match self.cursor {
                Button::Start => Button::Options,
                Button::Rule => Button::Start,
                Button::TimeStock => Button::Rule,
                Button::Options => Button::TimeStock,
            };
            self.set_button(self.cursor, TabStatus::Highlight);
            self.direction = Direction::Up;
            self.show_arrows();
        }
        if r.check(self.change_wait, pad, DOWN, true, -20, false) {
            sound::play_fgm(id::nSYAudioFGMMenuScroll2);
            self.change_wait = r.wait_n(7);
            if self.cursor == Button::TimeStock {
                self.change_wait += 8;
            }
            self.set_button(self.cursor, TabStatus::Not);
            self.cursor = match self.cursor {
                Button::Start => Button::Rule,
                Button::Rule => Button::TimeStock,
                Button::TimeStock => Button::Options,
                Button::Options => Button::Start,
            };
            self.set_button(self.cursor, TabStatus::Highlight);
            self.direction = Direction::Down;
            self.show_arrows();
        }
        match self.cursor {
            Button::Rule => {
                if r.check(self.change_wait, pad, LEFT, false, -20, false)
                    && self.rule > VsRule::Time
                {
                    sound::play_fgm(id::nSYAudioFGMMenuScroll1);
                    self.rule = self.rule.prev();
                    if self.rule == VsRule::Stock {
                        set_costumes_and_shades(self.rule, state);
                    }
                    self.remake_time_stock();
                    self.change_wait = r.wait_n(7);
                    self.direction = Direction::Left;
                }
                if r.check(self.change_wait, pad, RIGHT, false, 20, true)
                    && self.rule < VsRule::StockTeam
                {
                    sound::play_fgm(id::nSYAudioFGMMenuScroll1);
                    self.rule = self.rule.next();
                    if self.rule == VsRule::TimeTeam {
                        set_costumes_and_shades(self.rule, state);
                    }
                    self.remake_time_stock();
                    self.change_wait = r.wait_p(7);
                    self.direction = Direction::Right;
                    return load;
                }
            }
            Button::TimeStock => {
                if r.check(self.change_wait, pad, LEFT, false, -20, false) {
                    sound::play_fgm(id::nSYAudioFGMMenuScroll1);
                    self.change_wait = r.wait_n(14);
                    if self.direction != Direction::Left {
                        self.change_wait *= 2;
                    }
                    if self.rule.is_time() {
                        self.time = if self.time == 1 {
                            TIMELIMIT_INFINITE
                        } else {
                            self.time - 1
                        };
                        if self.time == 1 {
                            self.change_wait += 8;
                        }
                    } else {
                        self.stock = if self.stock == 0 { 98 } else { self.stock - 1 };
                        if self.stock == 0 {
                            self.change_wait += 8;
                        }
                    }
                    self.direction = Direction::Left;
                }
                if r.check(self.change_wait, pad, RIGHT, false, 20, true) {
                    sound::play_fgm(id::nSYAudioFGMMenuScroll1);
                    self.change_wait = r.wait_p(14);
                    if self.direction != Direction::Right {
                        self.change_wait *= 2;
                    }
                    if self.rule.is_time() {
                        self.time = if self.time == 100 { 1 } else { self.time + 1 };
                        if self.time == TIMELIMIT_INFINITE {
                            self.change_wait += 8;
                        }
                    } else {
                        self.stock = if self.stock == 98 { 0 } else { self.stock + 1 };
                        if self.stock == 98 {
                            self.change_wait += 8;
                        }
                    }
                    self.direction = Direction::Right;
                }
            }
            _ => {}
        }
        load
    }

    /// The cursor landing on Rule or Time/Stock shows that row's arrows and
    /// restarts their blink.
    fn show_arrows(&mut self) {
        match self.cursor {
            Button::Rule => {
                self.rule_arrows_hidden = false;
                self.rule_blink = BLINK;
            }
            Button::TimeStock => {
                self.time_stock_arrows_hidden = false;
                self.time_stock_blink = BLINK;
            }
            _ => {}
        }
    }

    fn has_rule_arrow(&self, a: Arrow) -> bool {
        self.rule_arrows.contains(&Some(a))
    }

    fn eject_rule_arrow(&mut self, a: Arrow) {
        if let Some(i) = self.rule_arrows.iter().position(|&s| s == Some(a)) {
            self.rule_arrows[i] = None;
            if i == 0 {
                self.rule_arrows = [self.rule_arrows[1], None];
            }
        }
    }

    fn make_rule_arrow(&mut self, a: Arrow) {
        let i = usize::from(self.rule_arrows[0].is_some());
        self.rule_arrows[i] = Some(a);
    }

    /// `mnVSModeAnimateRuleArrows`: blinks while the cursor is on Rule and
    /// keeps an arrow only on a side the rule can still move to.
    fn animate_rule_arrows(&mut self) {
        if self.cursor != Button::Rule {
            self.rule_arrows_hidden = true;
            return;
        }
        self.rule_blink -= 1;
        if self.rule_blink == 0 {
            self.rule_arrows_hidden = !self.rule_arrows_hidden;
            self.rule_blink = BLINK;
        }
        if self.rule == VsRule::Time {
            self.eject_rule_arrow(Arrow::Left);
        } else if !self.has_rule_arrow(Arrow::Left) {
            self.make_rule_arrow(Arrow::Left);
        }
        if self.rule == VsRule::StockTeam {
            self.eject_rule_arrow(Arrow::Right);
        } else if !self.has_rule_arrow(Arrow::Right) {
            self.make_rule_arrow(Arrow::Right);
        }
    }

    /// `mnVSModeAnimateTimeStockArrows`.
    fn animate_time_stock_arrows(&mut self) {
        if self.cursor != Button::TimeStock {
            self.time_stock_arrows_hidden = true;
            return;
        }
        self.time_stock_blink -= 1;
        if self.time_stock_blink == 0 {
            self.time_stock_arrows_hidden = !self.time_stock_arrows_hidden;
            self.time_stock_blink = BLINK;
        }
    }

    /// The rule arrows' `SObj`s in list order.
    pub fn rule_arrows(&self) -> [Option<Arrow>; 2] {
        self.rule_arrows
    }

    /// The cameras back to front: the background (80), the menu name (60),
    /// the buttons and arrows (40), then the rule and time/stock values
    /// (20). Every camera has the viewport `(10, 10)` to `(310, 230)`; the
    /// default camera (100) clears nothing.
    pub fn visit(&self, f: &mut impl FnMut(Draw)) {
        // `mnVSModeMakeBackground`.
        decals(f, FILE_VS_MODE, sprite::CONSOLE_ICON_DARK);
        // `mnVSModeMakeMenuName` (`mnVSModeRenderMenuName`).
        labels(
            f,
            [0xA0, 0x78, 0x14, 0xE6],
            FILE_VS_MODE,
            sprite::VS_TEXT,
            (158.0, 192.0),
        );
        f(Draw::Sprite(
            Piece::clear(FILE_COMMON, common::GAME_MODE_TEXT, 189.0, 87.0).prim([0; 3]),
        ));
        // Display link 2, in GObj order.
        self.vs_start_button(f);
        self.rule_button(f);
        if !self.time_stock_remade {
            self.time_stock_button(f);
        }
        self.vs_options_button(f);
        if self.time_stock_remade {
            self.time_stock_button(f);
        }
        // Display link 3.
        self.rule_value(f);
        self.time_stock_value_sprites(f);
    }

    fn text(f: &mut impl FnMut(Draw), offset: u32, x: f32, y: f32) {
        f(Draw::Sprite(
            Piece::clear(FILE_VS_MODE, offset, x, y).prim([0; 3]),
        ));
    }

    /// `mnVSModeMakeLeftArrow` and `mnVSModeMakeRightArrow`.
    fn arrow(f: &mut impl FnMut(Draw), a: Arrow, x: f32, y: f32) {
        let offset = match a {
            Arrow::Left => common::ARROW_L,
            Arrow::Right => common::ARROW_R,
        };
        f(Draw::Sprite(
            Piece::clear(FILE_COMMON, offset, x, y).prim([0xFF, 0xAE, 0x00]),
        ));
    }

    /// `mnVSModeMakeVSStartButton`.
    fn vs_start_button(&self, f: &mut impl FnMut(Draw)) {
        option_tab(f, 120.0, 31.0, 17, self.button_status[0]);
        Self::text(f, sprite::VS_START_TEXT, 153.0, 36.0);
    }

    /// `mnVSModeMakeRuleButton` and its arrows GObj.
    fn rule_button(&self, f: &mut impl FnMut(Draw)) {
        option_tab(f, 97.0, 70.0, 17, self.button_status[1]);
        Self::text(f, sprite::RULE_PERIOD_TEXT, 108.0, 75.0);
        if !self.rule_arrows_hidden {
            for a in self.rule_arrows.into_iter().flatten() {
                let x = if a == Arrow::Left { 165.0 } else { 250.0 };
                Self::arrow(f, a, x, 70.0);
            }
        }
    }

    /// `mnVSModeMakeTimeStockButton` and `mnVSModeMakeTimeStockArrows`.
    fn time_stock_button(&self, f: &mut impl FnMut(Draw)) {
        option_tab(f, 74.0, 109.0, 17, self.button_status[2]);
        if self.rule.is_time() {
            Self::text(f, sprite::TIME_PERIOD_TEXT, 97.0, 113.0);
            Self::text(f, sprite::MIN_TEXT, 197.0, 120.0);
        } else {
            Self::text(f, sprite::STOCK_PERIOD_TEXT, 106.0, 114.0);
        }
        if !self.time_stock_arrows_hidden {
            let left = if self.rule.is_time() { 155.0 } else { 165.0 };
            Self::arrow(f, Arrow::Left, left, 109.0);
            Self::arrow(f, Arrow::Right, 230.0, 109.0);
        }
    }

    /// `mnVSModeMakeVSOptionsButton`.
    fn vs_options_button(&self, f: &mut impl FnMut(Draw)) {
        option_tab(f, 51.0, 148.0, 17, self.button_status[3]);
        Self::text(f, sprite::VS_OPTIONS_TEXT, 71.0, 151.0);
    }

    /// `mnVSModeMakeRuleValue`: white.
    fn rule_value(&self, f: &mut impl FnMut(Draw)) {
        let white =
            |offset, x| Draw::Sprite(Piece::clear(FILE_VS_MODE, offset, x, 78.0).prim([0xFF; 3]));
        match self.rule {
            VsRule::Stock => f(white(sprite::STOCK_TEXT, 183.0)),
            VsRule::Time => f(white(sprite::TIME_TEXT, 187.0)),
            VsRule::StockTeam => {
                f(white(sprite::STOCK_TEXT, 165.0));
                f(white(sprite::TEAM_TEXT, 212.0));
            }
            VsRule::TimeTeam => {
                f(white(sprite::TIME_TEXT, 168.0));
                f(white(sprite::TEAM_TEXT, 212.0));
            }
        }
    }

    /// `mnVSModeMakeTimeStockValue`. Its `colors` are `0x000000FF` words
    /// stored into the `u8` sprite colours: white.
    fn time_stock_value_sprites(&self, f: &mut impl FnMut(Draw)) {
        let value = self.time_stock_value();
        if value == i32::from(TIMELIMIT_INFINITE) {
            f(Draw::Sprite(
                Piece::clear(FILE_COMMON, common::INFINITY, 162.0, 118.0).prim([0xFF; 3]),
            ));
            return;
        }
        let x = match (self.rule.is_time(), value < 10) {
            (true, true) => 185.0,
            (true, false) => 190.0,
            (false, true) => 210.0,
            (false, false) => 215.0,
        };
        right_digits(f, value, x, 116.0, [0xFF; 3], 2);
    }
}

/// `mnVSModeGetShade`: 0 outside team battles, else the first shade no
/// other player with the same fighter on the same team wears.
fn shade(rule: VsRule, state: &BattleState, p: usize) -> u8 {
    if !rule.is_team() {
        return 0;
    }
    let mut used = [false; PLAYERS];
    for (i, other) in state.players.iter().enumerate() {
        if i != p && other.fkind == state.players[p].fkind && other.team == state.players[p].team {
            // The source indexes its four-entry array with the shade.
            if let Some(u) = used.get_mut(usize::from(other.shade)) {
                *u = true;
            }
        }
    }
    // All four taken is out of the source's reach (it has no return).
    used.iter().position(|&u| !u).unwrap_or(0) as u8
}

/// `mnVSModeGetCostume`: the first royal costume index no other player
/// with this fighter wears.
fn free_costume(state: &BattleState, kind: crate::fighter::FighterKind, p: usize) -> usize {
    let mut used = [false; PLAYERS];
    for (i, other) in state.players.iter().enumerate() {
        if i != p && other.fkind == Some(kind) {
            for (j, u) in used.iter_mut().enumerate() {
                if costume_common_id(kind, j) == other.costume {
                    *u = true;
                }
            }
        }
    }
    used.iter().position(|&u| !u).unwrap_or(0)
}

/// `mnVSModeSetCostumesAndShades`: each placed fighter's costume and shade
/// for `rule`, player by player.
fn set_costumes_and_shades(rule: VsRule, state: &mut BattleState) {
    for i in 0..PLAYERS {
        let Some(kind) = state.players[i].fkind else {
            continue;
        };
        state.players[i].costume = if rule.is_team() {
            costume_team_id(kind, state.players[i].team)
        } else {
            costume_common_id(kind, free_costume(state, kind, i))
        };
        state.players[i].shade = shade(rule, state, i);
    }
}

#[cfg(test)]
#[path = "vs_mode_tests.rs"]
mod tests;
