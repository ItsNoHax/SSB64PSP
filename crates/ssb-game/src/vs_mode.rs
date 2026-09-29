//! The VS mode menu — `mnVSModeMain` in `mn/mnvsmode/mnvsmode.c`: four
//! buttons (VS Start, Rule, Time/Stock, VS Options), the rule (time, stock,
//! and their team variants), the time limit (1 to 99 minutes or infinite)
//! and the stock count (1 to 99), with the menus' held-input repeat
//! (`mnCommon*` in `mndef.h`). This is the state machine; the menu's
//! sprites are not drawn yet.

use ssb_engine::input::N64Buttons;

use crate::battle::{Rule, TIMELIMIT_INFINITE};

/// `nMNVSModeOption*`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Button {
    Start,
    Rule,
    TimeStock,
    Options,
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

/// What a frame of the menu decided.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    None,
    /// VS Start: on to `mnPlayersVS`.
    Start,
    /// VS Options.
    Options,
    /// B: back to the mode select.
    Back,
    /// Five idle minutes: back to the title.
    Title,
}

/// One controller's input this frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Input {
    pub hold: u16,
    pub tap: u16,
    pub stick_x: i8,
    pub stick_y: i8,
}

const UP: u16 = N64Buttons::D_UP | N64Buttons::C_UP;
const DOWN: u16 = N64Buttons::D_DOWN | N64Buttons::C_DOWN;
const LEFT: u16 = N64Buttons::D_LEFT | N64Buttons::L | N64Buttons::C_LEFT;
const RIGHT: u16 = N64Buttons::D_RIGHT | N64Buttons::R | N64Buttons::C_RIGHT;

/// `I_MIN_TO_TICS(5)`.
const IDLE_RETURN: u32 = 5 * 3600;

/// The menu's state (`sMNVSMode*`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VsMode {
    pub cursor: Button,
    pub rule: VsRule,
    /// Minutes, or [`TIMELIMIT_INFINITE`].
    pub time: u8,
    /// Stocks less one: 0 to 98.
    pub stock: u8,
    change_wait: i32,
    /// `sMNVSModeInputDirection`: 0 none, 1 up, 2 down, 3 left, 4 right.
    direction: u8,
    total_tics: u32,
    return_tic: u32,
}

/// `mnCommonGetOptionChangeWaitP`/`N`, or 12 for a button.
fn wait_for(button: bool, stick: i32, div: i32, positive: bool) -> i32 {
    if button {
        12
    } else if positive {
        (160 - stick) / div
    } else {
        (stick + 160) / div
    }
}

impl VsMode {
    /// `mnVSModeFuncStartVars` from the battle settings; `from_options`
    /// puts the cursor on VS Options.
    pub fn new(rule: VsRule, time: u8, stock: u8, from_options: bool) -> VsMode {
        VsMode {
            cursor: if from_options {
                Button::Options
            } else {
                Button::Start
            },
            rule,
            time,
            stock,
            change_wait: 0,
            direction: 0,
            total_tics: 0,
            return_tic: IDLE_RETURN,
        }
    }

    /// The battle's stock count (`gSCManagerTransferBattleState.stocks`).
    pub fn stocks(&self) -> i8 {
        self.stock as i8
    }

    /// `mnVSModeMain`.
    pub fn tick(&mut self, input: Input) -> Action {
        self.total_tics += 1;
        if self.total_tics < 10 {
            return Action::None;
        }
        if self.total_tics == self.return_tic {
            return Action::Title;
        }
        let idle = input.hold == 0 && input.tap == 0 && input.stick_x == 0 && input.stick_y == 0;
        if !idle {
            self.return_tic = self.total_tics + IDLE_RETURN;
        }
        if self.change_wait != 0 {
            self.change_wait -= 1;
        }
        let (sx, sy) = (i32::from(input.stick_x), i32::from(input.stick_y));
        if (-20..=20).contains(&sx)
            && (-20..=20).contains(&sy)
            && input.hold & (UP | RIGHT) == 0
            && input.hold & (DOWN | LEFT) == 0
        {
            self.change_wait = 0;
            self.direction = 0;
        }
        if input.tap & (N64Buttons::A | N64Buttons::START) != 0 {
            match self.cursor {
                Button::Start => return Action::Start,
                Button::Options => return Action::Options,
                _ => {}
            }
        }
        if input.tap & N64Buttons::B != 0 {
            return Action::Back;
        }
        let ready = self.change_wait == 0;
        let up_button = ready && input.hold & UP != 0;
        if up_button || (ready && sy > 20) {
            self.change_wait = wait_for(up_button, sy, 7, true);
            if self.cursor == Button::Rule {
                self.change_wait += 8;
            }
            self.cursor = match self.cursor {
                Button::Start => Button::Options,
                Button::Rule => Button::Start,
                Button::TimeStock => Button::Rule,
                Button::Options => Button::TimeStock,
            };
            self.direction = 1;
        }
        let ready = self.change_wait == 0;
        let down_button = ready && input.hold & DOWN != 0;
        if down_button || (ready && sy < -20) {
            self.change_wait = wait_for(down_button, sy, 7, false);
            if self.cursor == Button::TimeStock {
                self.change_wait += 8;
            }
            self.cursor = match self.cursor {
                Button::Start => Button::Rule,
                Button::Rule => Button::TimeStock,
                Button::TimeStock => Button::Options,
                Button::Options => Button::Start,
            };
            self.direction = 2;
        }
        let ready = self.change_wait == 0;
        let left_button = ready && input.hold & LEFT != 0;
        let left = left_button || (ready && sx < -20);
        match self.cursor {
            Button::Rule => {
                if left && self.rule > VsRule::Time {
                    self.rule = self.rule.prev();
                    self.change_wait = wait_for(left_button, sx, 7, false);
                    self.direction = 3;
                }
                let ready = self.change_wait == 0;
                let right_button = ready && input.hold & RIGHT != 0;
                if (right_button || (ready && sx > 20)) && self.rule < VsRule::StockTeam {
                    self.rule = self.rule.next();
                    self.change_wait = wait_for(right_button, sx, 7, true);
                    self.direction = 4;
                    return Action::None;
                }
            }
            Button::TimeStock => {
                if left {
                    self.change_wait = wait_for(left_button, sx, 14, false);
                    if self.direction != 3 {
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
                    self.direction = 3;
                }
                let ready = self.change_wait == 0;
                let right_button = ready && input.hold & RIGHT != 0;
                if right_button || (ready && sx > 20) {
                    self.change_wait = wait_for(right_button, sx, 14, true);
                    if self.direction != 4 {
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
                    self.direction = 4;
                }
            }
            _ => {}
        }
        Action::None
    }
}

#[cfg(test)]
#[path = "vs_mode_tests.rs"]
mod tests;
