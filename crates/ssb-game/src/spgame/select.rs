//! `mnPlayers1PGame*`: one human puck, settings and the campaign handoff.
//! Sprite/model drawing and backup persistence belong to the host.
use super::{Backup, Difficulty, SceneData};
use crate::{costume::costume_common_id, fighter::FighterKind, fighter_select as common};
use ssb_engine::input::{ControllerState, N64Buttons};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Selection {
    pub player: u8,
    pub kind: Option<FighterKind>,
    pub costume: u8,
    pub time_limit: u8,
    pub difficulty: Difficulty,
    /// Stock count minus one, as in the backup.
    pub stocks: i8,
}

impl Default for Selection {
    fn default() -> Self {
        Self {
            player: 0,
            kind: None,
            costume: 0,
            time_limit: 5,
            difficulty: Difficulty::Normal,
            stocks: 2,
        }
    }
}

impl Selection {
    /// `mnPlayers1PGameSetSceneData`, including the write on Back/timeout.
    /// An unplaced preview is saved as `nFTKindNull`.
    pub fn save(self, backup: &mut Backup) {
        backup.spgame_difficulty = self.difficulty;
        backup.spgame_stock_count = self.stocks;
        backup.writes += 1;
    }

    pub fn campaign(self) -> Option<SceneData> {
        Some(SceneData {
            player: self.player,
            fkind: self.kind?,
            costume: self.costume,
            time_limit: self.time_limit,
            ..Default::default()
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Proceed(Selection),
    Back(Selection),
    Timeout(Selection),
}

/// The presentation processes' state; positions use original screen units.
#[derive(Debug, Clone)]
pub struct View {
    pub portrait_x: [f32; 12],
    pub puck_shown: bool,
    pub ready_banner: bool,
    pub ready_press: bool,
    pub flash_shown: bool,
    pub spotlight_shown: bool,
    /// Source writes x and y twice, leaving the spotlight's z scale alone.
    pub spotlight_scale_xy: f32,
    pub fighter_rotate_y: f32,
    pub fighter_status: Option<crate::results_scene::DemoStatus>,
    pub fighter_hidden: bool,
    /// The existing model keeps running while its preview is hidden.
    pub fighter_kind: Option<FighterKind>,
    pub fighter_serial: u32,
    ready_blink: u8,
    flash_left: u8,
    status_selected: bool,
}

impl Default for View {
    fn default() -> Self {
        Self {
            portrait_x: crate::players_vs::layer::PORTRAIT_START_X,
            puck_shown: false,
            ready_banner: false,
            ready_press: false,
            flash_shown: false,
            spotlight_shown: true,
            spotlight_scale_xy: 1.0,
            fighter_rotate_y: 0.0,
            fighter_status: None,
            fighter_hidden: true,
            fighter_serial: 0,
            fighter_kind: None,
            ready_blink: 0,
            flash_left: 0,
            status_selected: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Select {
    pub selection: Selection,
    pub cursor: (f32, f32),
    pub cursor_status: common::CursorStatus,
    pub puck: (f32, f32),
    pub selected: bool,
    pub recalling: bool,
    pub view: View,
    pub total_tics: u32,
    fighter_mask: u16,
    return_tic: u32,
    held: bool,
    start_wait: Option<u8>,
    finished: bool,
    grab_tic: u32,
    pickup: Option<(f32, f32)>,
    velocity: (f32, f32),
    recall_tics: u8,
    recall_start: (f32, f32),
    recall_end: (f32, f32),
    recall_mid_y: f32,
}

impl Select {
    pub fn new(selection: Selection, fighter_mask: u16) -> Self {
        assert!(selection.player < 4 && (0..=4).contains(&selection.stocks));
        let selected = selection.kind.is_some();
        Self {
            selection,
            cursor: (60.0, 170.0),
            cursor_status: common::CursorStatus::Pointer,
            puck: selection
                .kind
                .map_or((51.0, 161.0), common::portrait_center),
            selected,
            recalling: false,
            view: View {
                fighter_hidden: !selected,
                fighter_kind: selection.kind,
                ..Default::default()
            },
            total_tics: 0,
            fighter_mask,
            return_tic: 18000,
            held: !selected,
            start_wait: None,
            finished: false,
            grab_tic: 0,
            pickup: None,
            velocity: (0.0, 0.0),
            recall_tics: 0,
            recall_start: (0.0, 0.0),
            recall_end: (0.0, 0.0),
            recall_mid_y: 0.0,
        }
    }

    pub fn is_ready(&self) -> bool {
        self.selected
    }

    fn saved(&self) -> Selection {
        Selection {
            kind: self.selection.kind.filter(|_| self.selected),
            ..self.selection
        }
    }

    fn in_box(&self, x: (f32, f32), y: (f32, f32)) -> bool {
        (x.0..=x.1).contains(&(self.cursor.0 + 20.0))
            && (y.0..=y.1).contains(&(self.cursor.1 + 3.0))
    }

    fn on_puck(&self) -> bool {
        (self.puck.0..=self.puck.0 + 26.0).contains(&(self.cursor.0 + 25.0))
            && (self.puck.1..=self.puck.1 + 24.0).contains(&(self.cursor.1 + 3.0))
    }

    fn make_fighter(&mut self) {
        self.view.fighter_hidden = self.selection.kind.is_none();
        if let Some(kind) = self.selection.kind {
            self.view.fighter_kind = self.selection.kind;
            self.view.fighter_serial += 1;
            self.view.fighter_status = None;
            self.view.status_selected = false;
            self.selection.costume = costume_common_id(kind, 0);
        }
    }

    fn place(&mut self, costume: usize) -> bool {
        if self.cursor_status != common::CursorStatus::Grab {
            return false;
        }
        let Some(kind) = self.selection.kind else {
            return false;
        };
        self.selection.costume = costume_common_id(kind, costume);
        self.selected = true;
        self.held = false;
        self.cursor_status = common::CursorStatus::Hover;
        self.grab_tic = self.total_tics + 30;
        self.view.flash_left = 16;
        self.view.flash_shown = true;
        true
    }

    fn grab(&mut self) {
        self.selected = false;
        self.held = true;
        self.cursor_status = common::CursorStatus::Grab;
        self.make_fighter();
        self.pickup = Some((self.puck.0 - 11.0, self.puck.1 + 14.0));
        self.view.flash_left = 0;
        self.view.flash_shown = false;
    }

    /// Scene run precedes cursor/puck/adjust processes. A ready Start
    /// pauses the cursor only; the puck and display clocks keep running.
    pub fn tick(&mut self, input: ControllerState, taps: N64Buttons) -> Option<Outcome> {
        if self.finished {
            return None;
        }
        self.total_tics += 1;
        let mut outcome = None;
        if self.total_tics == self.return_tic {
            outcome = Some(Outcome::Timeout(self.saved()));
        } else {
            if input.buttons.0 != 0
                || input.stick_x.abs_diff(0) > 20
                || input.stick_y.abs_diff(0) > 20
            {
                self.return_tic = self.total_tics + 18000;
            }
            if let Some(wait) = &mut self.start_wait {
                *wait -= 1;
                if *wait == 0 {
                    outcome = Some(Outcome::Proceed(self.saved()));
                }
            } else if self.total_tics > 60 && taps.contains(N64Buttons::START) && self.is_ready() {
                self.start_wait = Some(30);
            }
        }
        if self.start_wait.is_none() {
            self.move_cursor(input);
            if taps.contains(N64Buttons::A) && !self.place(0) {
                if self.total_tics >= self.grab_tic
                    && !self.recalling
                    && self.cursor_status == common::CursorStatus::Hover
                    && !self.held
                    && self.on_puck()
                {
                    self.grab();
                } else if self.in_box((210.0, 230.0), (12.0, 35.0))
                    || self.in_box((140.0, 160.0), (12.0, 35.0))
                {
                    self.selection.time_limit = if self.selection.time_limit == 5 {
                        100
                    } else {
                        5
                    };
                } else if self.in_box((244.0, 292.0), (13.0, 34.0)) {
                    outcome.get_or_insert(Outcome::Back(self.saved()));
                } else if self.in_box((258.0, 280.0), (155.0, 174.0)) {
                    self.selection.difficulty =
                        difficulty((self.selection.difficulty as u8 + 1).min(4));
                } else if self.in_box((190.0, 212.0), (155.0, 174.0)) {
                    self.selection.difficulty =
                        difficulty((self.selection.difficulty as u8).saturating_sub(1));
                } else if self.in_box((258.0, 280.0), (175.0, 194.0)) {
                    self.selection.stocks = (self.selection.stocks + 1).min(4);
                } else if self.in_box((190.0, 212.0), (175.0, 194.0)) {
                    self.selection.stocks = (self.selection.stocks - 1).max(0);
                }
            }
            for (costume, bit) in [
                N64Buttons::C_UP,
                N64Buttons::C_RIGHT,
                N64Buttons::C_DOWN,
                N64Buttons::C_LEFT,
            ]
            .into_iter()
            .enumerate()
            {
                if taps.contains(bit) && !self.place(costume) && self.selected {
                    self.selection.costume =
                        costume_common_id(self.selection.kind.unwrap(), costume);
                }
            }
            if taps.contains(N64Buttons::B) && self.selected && !self.held {
                self.selected = false;
                self.recalling = true;
                self.recall_tics = 0;
                self.recall_start = self.puck;
                self.recall_end = (
                    (self.cursor.0 + 20.0).min(280.0),
                    (self.cursor.1 - 15.0).max(10.0),
                );
                self.recall_mid_y = self.recall_end.1.min(self.puck.1) - 20.0;
            }
            if !self.recalling {
                if self.total_tics >= 10 && taps.contains(N64Buttons::B) {
                    outcome.get_or_insert(Outcome::Back(self.saved()));
                }
                self.cursor_status = if self.cursor.1 > 124.0 || self.cursor.1 < 38.0 {
                    if self.selected && self.on_puck() {
                        common::CursorStatus::Hover
                    } else {
                        common::CursorStatus::Pointer
                    }
                } else if self.held {
                    common::CursorStatus::Grab
                } else {
                    common::CursorStatus::Hover
                };
            }
        }
        // Priority 1 runs by common-link order after the priority-2
        // cursor: fighter 3, portraits 17, puck 20, spotlight 21,
        // Press Start 22, adjust 24, flash 26, banner 28.
        self.fighter_tick();
        for (i, x) in self.view.portrait_x.iter_mut().enumerate() {
            if let Some(next) = crate::players_vs::layer::next_portrait_x(i, *x) {
                *x = next;
            }
        }
        self.puck_update();
        if let Some(kind) = self.selection.kind {
            self.view.spotlight_shown = !self.selected && !self.view.spotlight_shown;
            if !self.selected {
                self.view.spotlight_scale_xy = if kind == FighterKind::Donkey {
                    2.0
                } else {
                    1.5
                };
            }
        } else {
            self.view.spotlight_shown = false;
        }
        self.view.ready_press = self.ready_tick();
        self.adjust_puck();
        if self.view.flash_left != 0 {
            self.view.flash_left -= 1;
            self.view.flash_shown = self.view.flash_left != 0 && !self.view.flash_shown;
        }
        self.view.ready_banner = self.ready_tick();
        self.finished = outcome.is_some();
        outcome
    }

    fn move_cursor(&mut self, input: ControllerState) {
        if let Some(target) = self.pickup {
            let step = |at: &mut f32, target: f32| {
                let d = (target - *at) / 5.0;
                if (-1.0..=1.0).contains(&d) {
                    *at = target;
                } else {
                    *at += d;
                }
            };
            step(&mut self.cursor.0, target.0);
            step(&mut self.cursor.1, target.1);
            if self.cursor == target {
                self.pickup = None;
            }
        } else if !self.recalling {
            for (at, stick, divisor, bounds) in [
                (&mut self.cursor.0, input.stick_x, 20.0, (0.0, 280.0)),
                (&mut self.cursor.1, input.stick_y, -20.0, (10.0, 205.0)),
            ] {
                if !(-8..=8).contains(&stick) {
                    let next = *at + f32::from(stick) / divisor;
                    if (bounds.0..=bounds.1).contains(&next) {
                        *at = next;
                    }
                }
            }
        }
    }

    fn puck_update(&mut self) {
        self.view.puck_shown = self.total_tics >= 30
            && (self.cursor_status != common::CursorStatus::Pointer
                || self.selected
                || self.recalling);
        if !self.selected && self.held {
            if self.pickup.is_none() {
                self.puck = (self.cursor.0 + 11.0, self.cursor.1 - 14.0);
            }
        } else {
            self.puck.0 += self.velocity.0;
            self.puck.1 += self.velocity.1;
        }
        let kind = common::puck_fighter_kind(self.puck, self.fighter_mask);
        if !self.selected && kind != self.selection.kind {
            self.selection.kind = kind;
            self.make_fighter();
        }
    }

    fn adjust_puck(&mut self) {
        if self.recalling {
            self.recall_tics += 1;
            if self.recall_tics < 11 {
                self.velocity = (
                    (self.recall_end.0 - self.recall_start.0) / 10.0,
                    if self.recall_tics < 6 {
                        (self.recall_mid_y - self.recall_start.1) / 5.0
                    } else {
                        (self.recall_end.1 - self.recall_mid_y) / 5.0
                    },
                );
            } else if self.recall_tics == 11 {
                self.grab();
                self.velocity = (0.0, 0.0);
            }
            if self.recall_tics == 30 {
                self.recalling = false;
            }
        }
        if self.selected {
            self.velocity =
                common::portrait_edge_velocity(self.selection.kind.unwrap(), self.puck, (0.0, 0.0));
        }
    }

    fn fighter_tick(&mut self) {
        if let Some(kind) = self.view.fighter_kind {
            let full = core::f32::consts::TAU;
            if self.selected {
                if self.view.fighter_rotate_y < 0.1_f32.to_radians() {
                    if !self.view.status_selected {
                        self.view.fighter_status = Some(selected_status(kind));
                        self.view.status_selected = true;
                    }
                } else {
                    self.view.fighter_rotate_y += 20.0_f32.to_radians();
                    if self.view.fighter_rotate_y > full {
                        self.view.fighter_rotate_y = 0.0;
                        self.view.fighter_status = Some(selected_status(kind));
                        self.view.status_selected = true;
                    }
                }
            } else {
                self.view.fighter_rotate_y += 2.0_f32.to_radians();
                if self.view.fighter_rotate_y > full {
                    self.view.fighter_rotate_y -= full;
                }
            }
        }
    }

    fn ready_tick(&mut self) -> bool {
        if self.selected {
            self.view.ready_blink = (self.view.ready_blink + 1) % 40;
            self.view.ready_blink < 30
        } else {
            self.view.ready_blink = 0;
            false
        }
    }

    pub fn record<'a>(&self, backup: &'a Backup) -> Option<&'a super::Record> {
        self.selection
            .kind
            .map(|k| &backup.spgame_records[k as usize])
    }
}

fn difficulty(i: u8) -> Difficulty {
    [
        Difficulty::VeryEasy,
        Difficulty::Easy,
        Difficulty::Normal,
        Difficulty::Hard,
        Difficulty::VeryHard,
    ][i as usize]
}

pub fn selected_status(kind: FighterKind) -> crate::results_scene::DemoStatus {
    use crate::results_scene::DemoStatus as Status;
    match kind {
        FighterKind::Fox | FighterKind::Samus => Status::Win4,
        FighterKind::Donkey | FighterKind::Luigi | FighterKind::Link | FighterKind::Captain => {
            Status::Win1
        }
        FighterKind::Yoshi | FighterKind::Purin | FighterKind::Ness => Status::Win2,
        FighterKind::Mario | FighterKind::Kirby => Status::Win3,
        _ => Status::Win1,
    }
}
