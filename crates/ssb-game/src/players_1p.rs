//! The 1P Game character select — `mn/mnplayers/mnplayers1pgame.c`: the
//! hand cursor, the player's puck, the portrait grid, the recall, the
//! costume picks, the level, stock and (hidden) time options, the ready
//! check and the scene and backup data it saves. [`layer`] holds its
//! presentation; the spotlight and sounds stay with the host.
//!
//! The scene has one slot (`sMNPlayers1PGameSlot`), for the port in
//! `gSCManagerSceneData.player` (`sMNPlayers1PGameManPlayer`). Unlike the
//! Training select it has no CPU puck, no costume conflicts and no puck
//! overlap: a placed puck keeps the velocity
//! `mnPlayers1PGamePuckAdjustPortraitEdge` last gave it, as the original
//! never clears it.
//!
//! One [`Players1P::tick`] runs the scene's processes in the original
//! order: `mnPlayers1PGameFuncRun` (the scene GObj's `func_run`), then the
//! cursor (process priority 2, paused once START is accepted), then the
//! puck and the puck adjust (priority 1, links 20 and 24).
//!
//! The original reads START and the idle timer from every controller
//! (`scSubsysControllerGetPlayerTapButtons`,
//! `scSubsysControllerCheckNoInputAll`); here, as in the Training select,
//! the host passes the one controller it reads.

use ssb_engine::input::{ControllerState, N64Buttons};

use crate::battle::TIMELIMIT_INFINITE;
use crate::costume::costume_common_id;
use crate::fighter::FighterKind;
use crate::fighter_select::{portrait_center, portrait_edge_velocity, puck_fighter_kind};
use crate::spgame::{self, Backup, Difficulty, CHARACTER_MASK_ALL};

pub use crate::fighter_select::{is_locked, portrait, CursorStatus, PORTRAIT_KINDS};

/// `I_MIN_TO_TICS(5)`.
const RETURN_TICS: i32 = 5 * 60 * 60;
/// START is read only after this many ticks.
const START_TICS: i32 = 60;
/// Ticks between a ready START and the 1P Game.
const START_PROCEED_WAIT: i32 = 30;
/// Ticks after a placement before the cursor can grab again.
const GRAB_WAIT: i32 = 30;
/// B returns to the 1P menu from this tick.
const BACK_TICS: i32 = 10;
/// The puck is hidden before this tick.
const PUCK_SHOW_TICS: i32 = 30;
/// The stock option's range: one to five stocks.
pub const STOCK_MAX: i8 = 4;

/// What the select reads from `gSCManagerSceneData`. `kind` `None` is
/// `nFTKindNull`, a first visit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SceneData {
    pub player: u8,
    pub kind: Option<FighterKind>,
    pub costume: u8,
    /// `spgame_time_limit`: minutes, or [`TIMELIMIT_INFINITE`].
    pub time_limit: u8,
}

impl Default for SceneData {
    fn default() -> Self {
        SceneData {
            player: 0,
            kind: None,
            costume: 0,
            time_limit: 5,
        }
    }
}

/// `mnPlayers1PGameSetSceneData`: what the select leaves in the scene and
/// backup data on its way out. `kind` is `None` unless the puck is placed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Saved {
    pub scene: SceneData,
    pub difficulty: Difficulty,
    pub stock_count: i8,
}

impl Saved {
    /// The same fields as the portable frontend's select returns them,
    /// for [`spgame::frontend::Frontend::start`].
    pub fn selection(&self) -> spgame::select::Selection {
        spgame::select::Selection {
            player: self.scene.player,
            kind: self.scene.kind,
            costume: self.scene.costume,
            time_limit: self.scene.time_limit,
            difficulty: self.difficulty,
            stocks: self.stock_count,
        }
    }

    /// Writes the saved values into the 1P Game's scene data and backup,
    /// with `spgame_stage` reset to 0, then counts the `lbBackupWrite`.
    /// The portable [`spgame::SceneData`] always names a fighter, so a
    /// `None` kind leaves its `fkind` alone; the host keeps the select's
    /// own [`SceneData`] for the next visit.
    pub fn apply(&self, scene: &mut spgame::SceneData, backup: &mut Backup) {
        scene.time_limit = self.scene.time_limit;
        scene.player = self.scene.player;
        backup.spgame_difficulty = self.difficulty;
        scene.stage = 0;
        backup.spgame_stock_count = self.stock_count;
        if let Some(kind) = self.scene.kind {
            scene.fkind = kind;
        }
        scene.costume = self.scene.costume;
        backup.writes += 1;
    }
}

/// What a tick asks the host to do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// The ready START's wait ran out: on to the 1P Game (`nSCKind1PGame`).
    Proceed(Saved),
    /// B, or A on the back button: back to the 1P menu (`nSCKind1PMode`).
    Back(Saved),
    /// Five idle minutes: back to the title.
    Timeout(Saved),
}

/// `MNPlayersSlot1PGame`, the fields the logic reads.
#[derive(Clone, Debug)]
pub struct Slot {
    pub kind: Option<FighterKind>,
    pub costume: u8,
    pub is_selected: bool,
    pub is_fighter_selected: bool,
    pub is_recalling: bool,
    recall_end_tic: i32,
    recall_start: (f32, f32),
    recall_end_x: f32,
    recall_mid_y: f32,
    recall_end_y: f32,
    recall_tics: i32,
    /// `holder_player` is the player (else `GMCOMMON_PLAYERS_MAX`).
    pub is_held: bool,
    /// The puck sprite's top-left corner.
    pub puck: (f32, f32),
    pub puck_vel: (f32, f32),
    /// The cursor sprite's top-left corner.
    pub cursor: (f32, f32),
    pub cursor_status: CursorStatus,
    cursor_pickup: (f32, f32),
    is_cursor_adjusting: bool,
}

/// `mnPlayers1PGameGetNextTimeValue` and `...GetPrevTimeValue`: both swap
/// five minutes and infinite.
pub fn next_time_value(value: u8) -> u8 {
    if value == 5 {
        TIMELIMIT_INFINITE
    } else {
        5
    }
}

/// `nSC1PGameDifficulty*` from the select's level value.
pub fn difficulty(level: i32) -> Difficulty {
    match level {
        0 => Difficulty::VeryEasy,
        1 => Difficulty::Easy,
        2 => Difficulty::Normal,
        3 => Difficulty::Hard,
        _ => Difficulty::VeryHard,
    }
}

/// The 1P Game character select's state.
#[derive(Clone, Debug)]
pub struct Players1P {
    pub slot: Slot,
    /// The presentation's GObjs.
    pub view: layer::View,
    /// `sMNPlayers1PGameManPlayer`.
    pub player: u8,
    /// `sMNPlayers1PGameLevelValue`, a `nSC1PGameDifficulty`.
    pub level: i32,
    /// `sMNPlayers1PGameStockValue`: stocks less one.
    pub stock: i8,
    /// `sMNPlayers1PGameTimeSetting`.
    pub time_setting: u8,
    fighter_mask: u16,
    total_tics: i32,
    return_tic: i32,
    is_start: bool,
    start_proceed_wait: i32,
    pending: Option<Outcome>,
}

impl Players1P {
    /// `mnPlayers1PGameInitVars` and `mnPlayers1PGameInitPlayer`, then
    /// `mnPlayers1PGameMakeCursor` and `MakePuck`.
    pub fn new(scene: SceneData, backup: &Backup) -> Self {
        let placed = scene.kind.is_some();
        let slot = Slot {
            kind: scene.kind,
            costume: scene.costume,
            is_selected: placed,
            is_fighter_selected: placed,
            is_recalling: false,
            recall_end_tic: 0,
            recall_start: (0.0, 0.0),
            recall_end_x: 0.0,
            recall_mid_y: 0.0,
            recall_end_y: 0.0,
            recall_tics: 0,
            is_held: !placed,
            puck: scene.kind.map_or((51.0, 161.0), portrait_center),
            puck_vel: (0.0, 0.0),
            cursor: (60.0, 170.0),
            cursor_status: CursorStatus::Pointer,
            cursor_pickup: (0.0, 0.0),
            is_cursor_adjusting: false,
        };
        let mut select = Players1P {
            slot,
            view: layer::View::default(),
            player: scene.player,
            level: backup.spgame_difficulty as i32,
            stock: backup.spgame_stock_count,
            time_setting: scene.time_limit,
            fighter_mask: backup.fighter_mask,
            total_tics: 0,
            return_tic: RETURN_TICS,
            is_start: false,
            start_proceed_wait: 0,
            pending: None,
        };
        select.v_init();
        select
    }

    /// `mnPlayers1PGameCheckReady`.
    pub fn is_ready(&self) -> bool {
        self.slot.is_fighter_selected
    }

    /// Whether the puck draws this tick (`mnPlayers1PGamePuckProcUpdate`).
    pub fn puck_visible(&self) -> bool {
        let s = &self.slot;
        self.total_tics >= PUCK_SHOW_TICS
            && (s.cursor_status != CursorStatus::Pointer || s.is_selected || s.is_recalling)
    }

    /// `mnPlayers1PGameGetForcePuckFighterKind`: the portrait under the
    /// puck, locked or not, for the records.
    pub fn force_puck_fighter_kind(&self) -> Option<FighterKind> {
        puck_fighter_kind(self.slot.puck, CHARACTER_MASK_ALL)
    }

    /// The values `mnPlayers1PGameSetSceneData` saves.
    pub fn saved(&self) -> Saved {
        Saved {
            scene: SceneData {
                player: self.player,
                kind: self.slot.kind.filter(|_| self.slot.is_fighter_selected),
                costume: self.slot.costume,
                time_limit: self.time_setting,
            },
            difficulty: difficulty(self.level),
            stock_count: self.stock,
        }
    }

    /// Records the first scene change of this tick with the data at that
    /// moment, as `syTaskmanSetLoadScene` takes effect at frame end.
    fn leave(&mut self, outcome: fn(Saved) -> Outcome) {
        if self.pending.is_none() {
            self.pending = Some(outcome(self.saved()));
        }
    }

    /// One frame. `input` is the player's controller; `taps` its newly
    /// pressed buttons.
    pub fn tick(&mut self, input: ControllerState, taps: N64Buttons) -> Option<Outcome> {
        self.pending = None;
        self.func_run(input, taps);
        // `mnPlayers1PGamePauseSlotProcesses` pauses only the cursor.
        if !self.is_start {
            self.cursor_update(input, taps);
        }
        self.puck_update();
        self.puck_adjust();
        self.view_tick();
        self.pending
    }

    /// `mnPlayers1PGameFuncRun`.
    fn func_run(&mut self, input: ControllerState, taps: N64Buttons) {
        self.total_tics += 1;
        if self.total_tics == self.return_tic {
            self.leave(Outcome::Timeout);
            return;
        }
        let stick_in = |v: i8| (-20..=20).contains(&v);
        if input.buttons.0 != 0 || !stick_in(input.stick_x) || !stick_in(input.stick_y) {
            self.return_tic = self.total_tics + RETURN_TICS;
        }
        if self.is_start {
            self.start_proceed_wait -= 1;
            if self.start_proceed_wait == 0 {
                self.leave(Outcome::Proceed);
            }
        } else if taps.contains(N64Buttons::START)
            && self.total_tics > START_TICS
            && self.is_ready()
        {
            self.start_proceed_wait = START_PROCEED_WAIT;
            self.is_start = true;
        }
    }

    /// `mnPlayers1PGameCursorProcUpdate`.
    fn cursor_update(&mut self, input: ControllerState, taps: N64Buttons) {
        self.adjust_cursor(input);
        if taps.contains(N64Buttons::A) && !self.select_fighter(0) && !self.check_cursor_puck_grab()
        {
            let (x, y) = (self.slot.cursor.0 + 20.0, self.slot.cursor.1 + 3.0);
            let time_row = (12.0..=35.0).contains(&y);
            if time_row && (210.0..=230.0).contains(&x) {
                // `mnPlayers1PGameCheckTimeArrowRInRange`.
                self.time_setting = next_time_value(self.time_setting);
            } else if time_row && (140.0..=160.0).contains(&x) {
                // `mnPlayers1PGameCheckTimeArrowLInRange`.
                self.time_setting = next_time_value(self.time_setting);
            } else if (13.0..=34.0).contains(&y) && (244.0..=292.0).contains(&x) {
                // `mnPlayers1PGameCheckBackInRange`.
                self.leave(Outcome::Back);
            } else if !self.check_level_arrow_press(x, y) {
                self.check_stock_arrow_press(x, y);
            }
        }
        for (button, bit) in [
            N64Buttons::C_UP,
            N64Buttons::C_RIGHT,
            N64Buttons::C_DOWN,
            N64Buttons::C_LEFT,
        ]
        .into_iter()
        .enumerate()
        {
            if taps.contains(bit) && !self.select_fighter(button) && self.slot.is_fighter_selected {
                self.update_costume(button);
            }
        }
        // `mnPlayers1PGameCheckManFighterSelected`.
        if taps.contains(N64Buttons::B) && self.slot.is_selected {
            self.recall_puck();
        }
        // `mnPlayers1PGameDetectBack`.
        if !self.slot.is_recalling && self.total_tics >= BACK_TICS && taps.contains(N64Buttons::B) {
            self.leave(Outcome::Back);
        }
        if !self.slot.is_recalling {
            self.update_cursor_no_recall();
        }
    }

    /// `mnPlayers1PGameCheckLevelArrowPress`. `(x, y)` is the cursor's
    /// point: its corner plus (20, 3).
    fn check_level_arrow_press(&mut self, x: f32, y: f32) -> bool {
        if !(155.0..=174.0).contains(&y) {
            return false;
        }
        if (258.0..=280.0).contains(&x) {
            if self.level < Difficulty::VeryHard as i32 {
                self.level += 1;
                self.v_make_level();
            }
            true
        } else if (190.0..=212.0).contains(&x) {
            if self.level > Difficulty::VeryEasy as i32 {
                self.level -= 1;
                self.v_make_level();
            }
            true
        } else {
            false
        }
    }

    /// `mnPlayers1PGameCheckStockArrowPress`.
    fn check_stock_arrow_press(&mut self, x: f32, y: f32) -> bool {
        if !(175.0..=194.0).contains(&y) {
            return false;
        }
        if (258.0..=280.0).contains(&x) {
            if self.stock < STOCK_MAX {
                self.stock += 1;
            }
            true
        } else if (190.0..=212.0).contains(&x) {
            if self.stock > 0 {
                self.stock -= 1;
            }
            true
        } else {
            false
        }
    }

    /// `mnPlayers1PGameAdjustCursor`.
    fn adjust_cursor(&mut self, input: ControllerState) {
        let s = &mut self.slot;
        if s.is_cursor_adjusting {
            let step = |target: f32, at: &mut f32| {
                let delta = (target - *at) / 5.0;
                if (-1.0..=1.0).contains(&delta) {
                    *at = target;
                } else {
                    *at += delta;
                }
            };
            step(s.cursor_pickup.0, &mut s.cursor.0);
            step(s.cursor_pickup.1, &mut s.cursor.1);
            if s.cursor == s.cursor_pickup {
                s.is_cursor_adjusting = false;
            }
        } else if !s.is_recalling {
            if !(-8..=8).contains(&input.stick_x) {
                let x = f32::from(input.stick_x) / 20.0 + s.cursor.0;
                if (0.0..=280.0).contains(&x) {
                    s.cursor.0 = x;
                }
            }
            if !(-8..=8).contains(&input.stick_y) {
                let y = f32::from(input.stick_y) / -20.0 + s.cursor.1;
                if (10.0..=205.0).contains(&y) {
                    s.cursor.1 = y;
                }
            }
        }
    }

    /// `mnPlayers1PGameCheckPuckInRange`.
    fn puck_in_range(&self) -> bool {
        let (cx, cy) = self.slot.cursor;
        let (px, py) = self.slot.puck;
        let x = cx + 25.0;
        let y = cy + 3.0;
        (px..=px + 26.0).contains(&x) && (py..=py + 24.0).contains(&y)
    }

    /// `mnPlayers1PGameCheckSelectFighter`. A, like C-Up, picks the first
    /// costume.
    fn select_fighter(&mut self, button: usize) -> bool {
        if self.slot.cursor_status != CursorStatus::Grab || self.slot.kind.is_none() {
            return false;
        }
        self.select_fighter_puck(button);
        self.slot.recall_end_tic = self.total_tics + GRAB_WAIT;
        true
    }

    /// `mnPlayers1PGameSelectFighterPuck`.
    fn select_fighter_puck(&mut self, button: usize) {
        let Some(kind) = self.slot.kind else {
            return;
        };
        let s = &mut self.slot;
        s.costume = costume_common_id(kind, button);
        s.is_selected = true;
        s.is_held = false;
        s.cursor_status = CursorStatus::Hover;
        s.is_fighter_selected = true;
        self.v_placement_priorities();
        self.v_make_portrait_flash();
    }

    /// `mnPlayers1PGameUpdateFighter`: the fighter model is made again in
    /// its first costume, unless the puck names no fighter and is not
    /// placed.
    fn update_fighter(&mut self) {
        if self.slot.kind.is_none() && !self.slot.is_selected {
            self.v_hide_fighter();
            return;
        }
        if let Some(kind) = self.slot.kind {
            self.slot.costume = costume_common_id(kind, 0);
        }
        self.v_make_fighter();
    }

    /// `mnPlayers1PGameSetCursorGrab` (and `CheckCursorPuckGrab`'s copy of
    /// it).
    fn set_cursor_grab(&mut self) {
        let s = &mut self.slot;
        s.is_held = true;
        s.is_selected = false;
        s.cursor_status = CursorStatus::Grab;
        s.is_fighter_selected = false;
        self.update_fighter();
        self.v_grab_priorities();
        // `mnPlayers1PGameSetCursorPuckOffset`.
        let s = &mut self.slot;
        s.cursor_pickup = (s.puck.0 - 11.0, s.puck.1 - -14.0);
        s.is_cursor_adjusting = true;
        self.v_destroy_portrait_flash();
        self.v_update_name_and_emblem();
    }

    /// `mnPlayers1PGameCheckCursorPuckGrab`.
    fn check_cursor_puck_grab(&mut self) -> bool {
        let s = &self.slot;
        if self.total_tics < s.recall_end_tic
            || s.is_recalling
            || s.cursor_status != CursorStatus::Hover
        {
            return false;
        }
        if !s.is_held && self.puck_in_range() {
            self.set_cursor_grab();
            return true;
        }
        false
    }

    /// `mnPlayers1PGameUpdateCostume`: no other slot can hold the costume.
    fn update_costume(&mut self, button: usize) {
        if let Some(kind) = self.slot.kind {
            self.slot.costume = costume_common_id(kind, button);
        }
    }

    /// `mnPlayers1PGameRecallPuck`.
    fn recall_puck(&mut self) {
        let cursor = self.slot.cursor;
        let s = &mut self.slot;
        s.is_fighter_selected = false;
        s.is_selected = false;
        s.is_recalling = true;
        s.recall_tics = 0;
        s.recall_start = s.puck;
        s.recall_end_x = (cursor.0 + 20.0).min(280.0);
        s.recall_end_y = (cursor.1 + -15.0).max(10.0);
        s.recall_mid_y = if s.recall_end_y < s.recall_start.1 {
            s.recall_end_y
        } else {
            s.recall_start.1
        } - 20.0;
    }

    /// `mnPlayers1PGameUpdateCursorNoRecall`.
    fn update_cursor_no_recall(&mut self) {
        let s = &self.slot;
        let status = if s.cursor.1 > 124.0 || s.cursor.1 < 38.0 {
            CursorStatus::Pointer
        } else if !s.is_held {
            CursorStatus::Hover
        } else {
            CursorStatus::Grab
        };
        let hover_placed = status == CursorStatus::Pointer && s.is_selected && self.puck_in_range();
        self.slot.cursor_status = if hover_placed {
            CursorStatus::Hover
        } else {
            status
        };
    }

    /// `mnPlayers1PGamePuckProcUpdate`.
    fn puck_update(&mut self) {
        self.view.puck_shown = self.puck_visible();
        let s = &mut self.slot;
        if !s.is_selected && s.is_held {
            if !s.is_cursor_adjusting {
                s.puck = (s.cursor.0 + 11.0, s.cursor.1 + -14.0);
            }
        } else {
            // `mnPlayers1PGameMovePuck`.
            s.puck.0 += s.puck_vel.0;
            s.puck.1 += s.puck_vel.1;
        }
        let kind = puck_fighter_kind(self.slot.puck, self.fighter_mask);
        if !self.slot.is_selected && kind != self.slot.kind {
            self.slot.kind = kind;
            self.update_fighter();
            self.v_update_name_and_emblem();
        }
    }

    /// `mnPlayers1PGamePuckAdjustProcUpdate`.
    fn puck_adjust(&mut self) {
        if self.slot.is_recalling {
            self.adjust_recall();
        }
        if self.slot.is_selected {
            // `mnPlayers1PGamePuckAdjustPlaced`: only the portrait edge.
            if let Some(kind) = self.slot.kind {
                let s = &mut self.slot;
                s.puck_vel = portrait_edge_velocity(kind, s.puck, s.puck_vel);
            }
        }
    }

    /// `mnPlayers1PGamePuckAdjustRecall`.
    fn adjust_recall(&mut self) {
        let s = &mut self.slot;
        s.recall_tics += 1;
        if s.recall_tics < 11 {
            let vx = (s.recall_end_x - s.recall_start.0) / 10.0;
            let vy = if s.recall_tics < 6 {
                (s.recall_mid_y - s.recall_start.1) / 5.0
            } else {
                (s.recall_end_y - s.recall_mid_y) / 5.0
            };
            s.puck_vel = (vx, vy);
        } else if s.recall_tics == 11 {
            self.set_cursor_grab();
            self.slot.puck_vel = (0.0, 0.0);
        }
        if self.slot.recall_tics == 30 {
            self.slot.is_recalling = false;
        }
    }

    /// The 1P records the select shows for the fighter under the puck
    /// (`mnPlayers1PGameGetHiScore`, `...GetBonusCount` and the record's
    /// `spgame_best_difficulty`), and the totals
    /// (`mnPlayers1PGameGetTotalHiScore`, `...GetTotalBonusCount`).
    pub fn records(backup: &Backup, kind: Option<FighterKind>) -> Records {
        let total = |f: fn(&spgame::Record) -> u32| {
            backup
                .spgame_records
                .iter()
                .fold(0u32, |sum, r| sum.wrapping_add(f(r)))
        };
        Records {
            fighter: kind.map(|k| {
                let r = &backup.spgame_records[k as usize];
                (
                    r.spgame_hiscore,
                    r.spgame_total_bonuses,
                    r.spgame_best_difficulty,
                )
            }),
            total_hiscore: total(|r| r.spgame_hiscore),
            total_bonuses: total(|r| r.spgame_total_bonuses),
        }
    }
}

/// The numbers `mnPlayers1PGameMakeFighterRecord` and `MakeTotalRecord`
/// draw.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Records {
    /// High score, bonus count and best difficulty (0 for none, else the
    /// difficulty plus one).
    pub fighter: Option<(u32, u32, u8)>,
    pub total_hiscore: u32,
    pub total_bonuses: u32,
}

#[path = "players_1p_layer.rs"]
pub mod layer;

#[cfg(test)]
#[path = "players_1p_tests.rs"]
mod tests;
