//! `sc1pstageclear.c`: timed score registration, nine-row bonus pages,
//! and the input gate. Display state is portable; the host draws it.
use super::{results, BattleState, Difficulty, SceneData, Stage};
use ssb_engine::input::N64Buttons;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Stage,
    Game,
    Result,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Row {
    pub bonus_id: usize,
    pub points: i32,
    pub reveal_tic: u32,
}

#[derive(Debug, Clone)]
pub struct StageClear {
    pub kind: Kind,
    pub stage: Stage,
    pub total_tics: u32,
    /// Score registered internally and score last rebuilt on screen differ.
    pub total_score: i64,
    pub shown_score: u32,
    pub timer_text: bool,
    pub timer_digits: bool,
    pub timer_multiplied: bool,
    pub damage_text: bool,
    pub damage_digits: bool,
    pub damage_multiplied: bool,
    pub target_text: bool,
    pub objectives_shown: u8,
    pub bonus_table: bool,
    pub rows: [Option<Row>; 9],
    pub page_arrow_tic: Option<u32>,
    pub allow_proceed: bool,
    pub seconds: u32,
    pub damage: u32,
    objectives: u8,
    pub difficulty: Difficulty,
    bonus_flags: [u32; 3],
    next_bonus: usize,
    page_count: u32,
    has_bonus: bool,
    last_page: bool,
    can_change_page: bool,
    pub no_timer: bool,
    base_tic: u32,
    common_tic: u32,
    show_next_tic: u32,
    bonus_advance_tic: u32,
    finished: bool,
}

impl StageClear {
    pub fn new(data: &SceneData, state: &BattleState, difficulty: Difficulty) -> Self {
        let stage = data.stage().expect("valid campaign stage");
        assert!(!stage.is_challenger() && data.bonus_tasks_complete <= 10);
        let kind = match stage {
            Stage::Boss => Kind::Game,
            Stage::Bonus1 | Stage::Bonus2 | Stage::Bonus3 => Kind::Result,
            _ => Kind::Stage,
        };
        Self {
            kind,
            stage,
            total_tics: 0,
            total_score: i64::from(data.score),
            shown_score: data.score,
            timer_text: false,
            timer_digits: false,
            timer_multiplied: false,
            damage_text: false,
            damage_digits: false,
            damage_multiplied: false,
            target_text: false,
            objectives_shown: 0,
            bonus_table: false,
            rows: [None; 9],
            page_arrow_tic: None,
            allow_proceed: false,
            seconds: data.time_remain,
            damage: state.players[data.player as usize].total_damage_given,
            objectives: data.bonus_tasks_complete,
            difficulty,
            bonus_flags: data.bonus_get_mask,
            next_bonus: 0,
            page_count: 0,
            has_bonus: data.bonus_get_mask.iter().any(|m| *m != 0),
            last_page: false,
            can_change_page: false,
            no_timer: data.time_limit == crate::battle::TIMELIMIT_INFINITE,
            base_tic: 0,
            common_tic: 0,
            show_next_tic: 0,
            bonus_advance_tic: 0,
            finished: false,
        }
    }

    fn update_score(&mut self) {
        self.shown_score = self.total_score.max(0) as u32;
    }
    fn timer_score(&mut self) {
        self.timer_multiplied = true;
        let multiplier = match self.stage {
            Stage::Bonus3 => 500,
            Stage::Bonus1 | Stage::Bonus2 => 200,
            _ => 50,
        };
        self.total_score += i64::from(self.seconds) * multiplier;
    }
    fn finish_base(&mut self) {
        if self.has_bonus {
            self.common_tic = self.total_tics + 10;
            self.show_next_tic = self.common_tic;
            self.bonus_advance_tic = self.common_tic;
        } else {
            self.allow_proceed = true;
        }
    }

    fn bonus_page(&mut self) {
        self.bonus_table = true;
        self.timer_text = false;
        self.damage_text = false;
        self.timer_digits = false;
        self.damage_digits = false;
        self.target_text = false;
        self.objectives_shown = 0;
        self.rows = [None; 9];
        self.page_arrow_tic = None;
        self.page_count = 0;
        self.can_change_page = false;
        let mut points = 0;
        while self.next_bonus < 96 {
            if self.page_count == 9 {
                self.last_page =
                    !(self.next_bonus..96).any(|i| self.bonus_flags[i / 32] & (1 << (i % 32)) != 0);
                if !self.last_page {
                    self.page_arrow_tic = Some(self.total_tics + 90);
                }
                break;
            }
            let id = self.next_bonus;
            self.next_bonus += 1;
            if self.bonus_flags[id / 32] & (1 << (id % 32)) != 0 {
                // The source's bonus table has 58 entries. Invalid bits are
                // a contract violation, not an invented bonus row.
                let value = results::bonus_points(id, self.stage, self.difficulty);
                self.rows[self.page_count as usize] = Some(Row {
                    bonus_id: id,
                    points: value,
                    reveal_tic: self.total_tics + self.page_count * 10,
                });
                points += i64::from(value);
                self.page_count += 1;
            }
        }
        if self.next_bonus == 96 {
            self.last_page = true;
        }
        // Negative score is floored after each page, not after each row.
        self.total_score = (self.total_score + points).max(0);
        self.show_next_tic = self.total_tics + self.page_count * 10 + 20;
    }

    /// Returns the committed score once. Taps on the same tick that the
    /// proceed gate opens are ignored (FuncRun reads input before updates).
    pub fn tick(&mut self, taps: N64Buttons) -> Option<u32> {
        if self.finished {
            return None;
        }
        self.total_tics += 1;
        let t = self.total_tics;
        if t < 10 {
            return None;
        }
        if taps.0 & (N64Buttons::A | N64Buttons::B | N64Buttons::START) != 0 {
            if self.allow_proceed {
                self.finished = true;
                return Some(self.total_score.max(0) as u32);
            } else if self.can_change_page && !self.last_page {
                self.common_tic = t;
            }
        }
        match self.kind {
            Kind::Stage | Kind::Game => {
                if !self.no_timer {
                    if t == 10 {
                        self.timer_text = true;
                    }
                    if t == 20 {
                        self.timer_digits = true;
                    }
                    if t == 60 {
                        self.timer_score();
                    }
                    if t == 80 {
                        self.update_score();
                    }
                }
                let (text, digits, mult, eject) = if self.no_timer {
                    (10, 20, 40, 60)
                } else {
                    (30, 40, 100, 120)
                };
                if t == text {
                    self.damage_text = true;
                }
                if t == digits {
                    self.damage_digits = true;
                }
                if t == mult {
                    self.damage_multiplied = true;
                    self.total_score += i64::from(self.damage) * 10;
                }
                if t == eject {
                    self.update_score();
                    self.finish_base();
                }
            }
            Kind::Result => {
                if self.stage != Stage::Bonus3 {
                    if t == 10 {
                        self.target_text = true;
                    }
                    if t == 20 {
                        self.base_tic = 20 + u32::from(self.objectives) * 10;
                    }
                    // The target GObj's own process registers the score at
                    // its deadline, but does not become visible until later.
                    if t >= 20
                        && (t - 20).is_multiple_of(10)
                        && (t - 20) / 10 < u32::from(self.objectives)
                    {
                        self.total_score += 1000;
                        self.update_score();
                    }
                    if self.target_text {
                        self.objectives_shown = if t > 20 {
                            ((t - 21) / 10 + 1).min(u32::from(self.objectives)) as u8
                        } else {
                            0
                        };
                    }
                } else if t == 10 {
                    self.base_tic = 10;
                }
                if self.base_tic != 0 {
                    if self.no_timer && self.stage != Stage::Bonus3 {
                        if t == self.base_tic {
                            self.finish_base();
                        }
                    } else {
                        let timer = self.stage == Stage::Bonus3 || self.objectives == 10;
                        if t == self.base_tic + 10 && timer {
                            self.timer_text = true;
                        }
                        if t == self.base_tic + 30 && timer {
                            self.timer_digits = true;
                        }
                        if t == self.base_tic + 50 && timer {
                            self.timer_score();
                        }
                        if t == self.base_tic + 70 {
                            if timer {
                                self.update_score();
                            }
                            self.finish_base();
                        }
                    }
                }
            }
        }
        if self.has_bonus {
            if t == self.common_tic {
                self.bonus_page();
            } else if t == self.show_next_tic {
                self.update_score();
                self.can_change_page = true;
                if self.last_page {
                    self.bonus_advance_tic = t + 20;
                }
            } else if t == self.bonus_advance_tic {
                self.allow_proceed = true;
            }
        }
        None
    }

    pub fn row_shown(&self, index: usize) -> bool {
        self.rows[index].is_some_and(|row| row.reveal_tic < self.total_tics)
    }

    pub fn page_arrow_shown(&self) -> bool {
        self.page_arrow_tic.is_some_and(|tic| tic < self.total_tics)
    }
}
