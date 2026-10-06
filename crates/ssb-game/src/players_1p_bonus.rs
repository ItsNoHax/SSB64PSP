//! The Bonus Practice character select — `mn/mnplayers/mnplayers1pbonus.c`,
//! the scene behind both `nSCKind1PBonus1Players` ("Break the Targets")
//! and `nSCKind1PBonus2Players` ("Board the Platforms"): the hand cursor,
//! the player's puck, the portrait grid, the costume picks, the title that
//! switches between the two practices, the auto-start once a fighter is
//! placed, and the scene data it saves. [`layer`] holds its presentation,
//! with the records; the spotlight, the sounds and the announcer's voices
//! stay with the host.
//!
//! The scene is a copy of `mnPlayers1PGame` (`crate::players_1p`) with the
//! options removed. What differs:
//!
//! - Nothing is recalled: `mnPlayers1PBonusInitPlayer` and `ResetPlayer`
//!   always start the puck held with `nFTKindNull` under it. The scene
//!   only writes `gSCManagerSceneData.bonus_fkind` and `bonus_costume`;
//!   it never reads them.
//! - The cursor starts at (80, 170), and the puck shows from the first
//!   tick (no 30-tick wait).
//! - Placing a fighter arms the start (`sMNPlayers1PBonusIsSelected`,
//!   `StartWait` 140): START, from tick 1, or the wait running out goes on
//!   to `nSCKind1PBonusStage`. Picking the puck up disarms it. The cursor
//!   is never paused.
//! - A on the title (`mnPlayers1PBonusCheckGameModeInRange`) swaps the
//!   practice (`mnPlayers1PBonusUpdateGameMode`).
//! - No backup write: `mnPlayers1PBonusSetSceneData` only writes the scene
//!   data, and the bonus stage reads which practice from `scene_prev`.
//!
//! One [`Players1PBonus::tick`] runs the processes in the 1P select's
//! order: `mnPlayers1PBonusFuncRun`, the cursor (priority 2), then the puck
//! and the puck adjust (priority 1, links 20 and 24).
//!
//! The original reads the idle timer from every controller
//! (`scSubsysControllerCheckNoInputAll`); here, as in the other selects,
//! the host passes the one controller it reads. START is read from the
//! scene's player (`gSYControllerDevices[sMNPlayers1PBonusManPlayer]`), as
//! the source does.

use ssb_engine::input::{ControllerState, N64Buttons};

use crate::costume::costume_common_id;
use crate::fighter::FighterKind;
use crate::fighter_select::{portrait_edge_velocity, puck_fighter_kind};
use crate::players_1p::Slot;
use crate::spgame::{Backup, BONUSGAME_TASK_MAX, CHARACTER_MASK_ALL};

pub use crate::fighter_select::{is_locked, portrait, CursorStatus, PORTRAIT_KINDS};

/// `I_MIN_TO_TICS(5)`.
const RETURN_TICS: i32 = 5 * 60 * 60;
/// `mnPlayers1PBonusSelectFighterPuck`'s `sMNPlayers1PBonusStartWait`:
/// ticks from a placement to the auto-start.
pub const START_WAIT: i32 = 140;
/// Ticks after a placement before the cursor can grab again.
const GRAB_WAIT: i32 = 30;
/// B returns to the 1P menu from this tick.
const BACK_TICS: i32 = 10;
/// `mnPlayers1PBonusMakeCursor`'s start.
const CURSOR_START: (f32, f32) = (80.0, 170.0);
/// `TIME_SEC`, `TIME_MIN`.
const TIME_SEC: i32 = 60;
const TIME_MIN: i32 = 60 * TIME_SEC;
/// `I_HRS_TO_TICS(1) - 1`: `mnPlayers1PBonusGetBestTime`'s ceiling.
pub const BEST_TIME_MAX: u32 = 60 * 60 * 60 - 1;

/// `sMNPlayers1PBonusBonusKind`: which practice the scene is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BonusKind {
    /// 0: Break the Targets, `nSCKind1PBonus1Players`.
    Targets,
    /// 1: Board the Platforms, `nSCKind1PBonus2Players`.
    Platforms,
}

impl BonusKind {
    /// `mnPlayers1PBonusUpdateGameMode`.
    pub fn other(self) -> Self {
        match self {
            BonusKind::Targets => BonusKind::Platforms,
            BonusKind::Platforms => BonusKind::Targets,
        }
    }

    /// 1 for Bonus 1 Practice, 2 for Bonus 2.
    pub fn number(self) -> u8 {
        match self {
            BonusKind::Targets => 1,
            BonusKind::Platforms => 2,
        }
    }
}

/// What the select reads from `gSCManagerSceneData`: the man `player`, and
/// `scene_curr` as the practice (`nSCKind1PBonus1Players` is
/// [`BonusKind::Targets`], anything else [`BonusKind::Platforms`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SceneData {
    pub player: u8,
    pub bonus: BonusKind,
}

impl Default for SceneData {
    fn default() -> Self {
        SceneData {
            player: 0,
            bonus: BonusKind::Targets,
        }
    }
}

/// `mnPlayers1PBonusSetSceneData`: what the select leaves in
/// `gSCManagerSceneData` on every way out. `bonus_fkind` is the fighter
/// under the puck whether placed or not (`None`, `nFTKindNull`, when
/// there is none), and `bonus_costume` the slot's last costume.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Saved {
    pub player: u8,
    pub bonus_fkind: Option<FighterKind>,
    pub bonus_costume: u8,
}

/// What a tick asks the host to do. `bonus` is the practice shown when the
/// scene left; the host sets `scene_prev` to its `nSCKind1PBonus*Players`
/// for [`Outcome::Proceed`] and [`Outcome::Back`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// START, or the start wait running out, with a fighter placed: on to
    /// `nSCKind1PBonusStage`, which plays the practice `scene_prev` names.
    Proceed { saved: Saved, bonus: BonusKind },
    /// B, or A on the back button: back to the 1P menu (`nSCKind1PMode`).
    Back { saved: Saved, bonus: BonusKind },
    /// Five idle minutes: back to the title. `scene_prev` is the scene
    /// as entered (`scene_curr`), whatever the title now shows.
    Timeout(Saved),
}

/// The Bonus Practice character select's state.
#[derive(Clone, Debug)]
pub struct Players1PBonus {
    /// `sMNPlayers1PBonusSlot`.
    pub slot: Slot,
    /// The presentation's GObjs.
    pub view: layer::View,
    /// `sMNPlayers1PBonusManPlayer`.
    pub player: u8,
    /// `sMNPlayers1PBonusBonusKind`.
    pub bonus: BonusKind,
    /// `sMNPlayers1PBonusIsSelected`: the start is armed.
    pub is_selected: bool,
    /// `sMNPlayers1PBonusStartWait`.
    pub start_wait: i32,
    fighter_mask: u16,
    total_tics: i32,
    return_tic: i32,
    pending: Option<Outcome>,
}

impl Players1PBonus {
    /// `mnPlayers1PBonusInitVars` and `InitPlayer`, then `InitSlot`
    /// (`MakeCursor`, `MakePuck`, `MakeGate`, `ResetPlayer`) and
    /// `MakeLabels`.
    pub fn new(scene: SceneData, backup: &Backup) -> Self {
        let mut select = Players1PBonus {
            slot: Slot::held(CURSOR_START),
            view: layer::View::default(),
            player: scene.player,
            bonus: scene.bonus,
            is_selected: false,
            start_wait: 0,
            fighter_mask: backup.fighter_mask,
            total_tics: 0,
            return_tic: RETURN_TICS,
            pending: None,
        };
        select.v_init();
        select
    }

    /// `mnPlayers1PBonusCheckFighterLocked`.
    pub fn is_locked(&self, kind: FighterKind) -> bool {
        is_locked(kind, self.fighter_mask)
    }

    /// The fighter mask the portraits are drawn with.
    pub fn fighter_mask(&self) -> u16 {
        self.fighter_mask
    }

    /// Whether the puck draws this tick (`mnPlayers1PBonusPuckProcUpdate`).
    pub fn puck_visible(&self) -> bool {
        let s = &self.slot;
        s.cursor_status != CursorStatus::Pointer || s.is_selected || s.is_recalling
    }

    /// `mnPlayers1PBonusGetForcePuckFighterKind`: the portrait under the
    /// puck, locked or not, for the records.
    pub fn force_puck_fighter_kind(&self) -> Option<FighterKind> {
        puck_fighter_kind(self.slot.puck, CHARACTER_MASK_ALL)
    }

    /// The values `mnPlayers1PBonusSetSceneData` saves.
    pub fn saved(&self) -> Saved {
        Saved {
            player: self.player,
            bonus_fkind: self.slot.kind,
            bonus_costume: self.slot.costume,
        }
    }

    /// Records the first scene change of this tick with the data at that
    /// moment, as `syTaskmanSetLoadScene` takes effect at frame end.
    fn leave(&mut self, outcome: Outcome) {
        if self.pending.is_none() {
            self.pending = Some(outcome);
        }
    }

    fn leave_proceed(&mut self) {
        let (saved, bonus) = (self.saved(), self.bonus);
        self.leave(Outcome::Proceed { saved, bonus });
    }

    /// `mnPlayers1PBonusBackTo1PMode`.
    fn leave_back(&mut self) {
        let (saved, bonus) = (self.saved(), self.bonus);
        self.leave(Outcome::Back { saved, bonus });
    }

    /// One frame. `input` is the player's controller; `taps` its newly
    /// pressed buttons.
    pub fn tick(&mut self, input: ControllerState, taps: N64Buttons) -> Option<Outcome> {
        self.pending = None;
        self.func_run(input, taps);
        self.cursor_update(input, taps);
        self.puck_update();
        self.puck_adjust();
        self.view_tick();
        self.pending
    }

    /// `mnPlayers1PBonusFuncRun`.
    fn func_run(&mut self, input: ControllerState, taps: N64Buttons) {
        self.total_tics += 1;
        if self.total_tics == self.return_tic {
            let saved = self.saved();
            self.leave(Outcome::Timeout(saved));
            return;
        }
        let stick_in = |v: i8| (-20..=20).contains(&v);
        if input.buttons.0 != 0 || !stick_in(input.stick_x) || !stick_in(input.stick_y) {
            self.return_tic = self.total_tics + RETURN_TICS;
        }
        if self.is_selected && !self.slot.is_fighter_selected {
            self.is_selected = false;
        }
        if self.is_selected {
            // The wait counts down even on the tick START is pressed.
            self.start_wait -= 1;
            if self.start_wait == 0 || taps.contains(N64Buttons::START) {
                self.leave_proceed();
            }
        }
    }

    /// `mnPlayers1PBonusCursorProcUpdate`.
    fn cursor_update(&mut self, input: ControllerState, taps: N64Buttons) {
        self.slot.adjust_cursor(input);
        if taps.contains(N64Buttons::A) && !self.select_fighter(0) && !self.check_cursor_puck_grab()
        {
            let (x, y) = (self.slot.cursor.0 + 20.0, self.slot.cursor.1 + 3.0);
            if (27.0..=207.0).contains(&x) && (14.0..=35.0).contains(&y) {
                // `mnPlayers1PBonusCheckGameModeInRange`.
                self.update_game_mode();
            } else if (13.0..=34.0).contains(&y) && (244.0..=292.0).contains(&x) {
                // `mnPlayers1PBonusCheckBackInRange`.
                self.leave_back();
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
        // `mnPlayers1PBonusCheckManFighterSelected`.
        if taps.contains(N64Buttons::B) && self.slot.is_selected {
            self.slot.recall_puck();
        }
        // `mnPlayers1PBonusDetectBack`.
        if !self.slot.is_recalling && self.total_tics >= BACK_TICS && taps.contains(N64Buttons::B) {
            self.leave_back();
        }
        if !self.slot.is_recalling {
            // `mnPlayers1PBonusUpdateCursorNoRecall`.
            self.slot.cursor_status = self.slot.no_recall_status();
        }
    }

    /// `mnPlayers1PBonusUpdateGameMode`: the other practice, its title
    /// (and announcer voice, the host's) and its records.
    fn update_game_mode(&mut self) {
        self.bonus = self.bonus.other();
        self.v_make_hiscore();
    }

    /// `mnPlayers1PBonusCheckSelectFighter`. A, like C-Up, picks the first
    /// costume.
    fn select_fighter(&mut self, button: usize) -> bool {
        if self.slot.cursor_status != CursorStatus::Grab || self.slot.kind.is_none() {
            return false;
        }
        self.select_fighter_puck(button);
        self.slot.recall_end_tic = self.total_tics + GRAB_WAIT;
        true
    }

    /// `mnPlayers1PBonusSelectFighterPuck`: placed, and the start armed.
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
        self.v_make_portrait_flash();
        self.start_wait = START_WAIT;
        self.is_selected = true;
    }

    /// `mnPlayers1PBonusUpdateFighter`: the fighter is made again in
    /// `mnPlayers1PBonusGetCostume(fkind, 0)`, unless the puck names no
    /// fighter and is not placed; the records are remade either way.
    fn update_fighter(&mut self) {
        if self.slot.kind.is_none() && !self.slot.is_selected {
            self.v_hide_fighter();
            self.v_make_hiscore();
            return;
        }
        if let Some(kind) = self.slot.kind {
            self.slot.costume = costume(kind);
        }
        self.v_make_fighter();
        self.v_make_hiscore();
    }

    /// `mnPlayers1PBonusSetCursorGrab` (and `CheckCursorPuckGrab`'s copy of
    /// it).
    fn set_cursor_grab(&mut self) {
        let s = &mut self.slot;
        s.is_held = true;
        s.is_selected = false;
        s.cursor_status = CursorStatus::Grab;
        s.is_fighter_selected = false;
        self.update_fighter();
        self.slot.set_cursor_puck_offset();
        self.slot.is_cursor_adjusting = true;
        self.v_destroy_portrait_flash();
        self.v_update_name_and_emblem();
    }

    /// `mnPlayers1PBonusCheckCursorPuckGrab`.
    fn check_cursor_puck_grab(&mut self) -> bool {
        let s = &self.slot;
        if self.total_tics < s.recall_end_tic || s.cursor_status != CursorStatus::Hover {
            return false;
        }
        if !s.is_held && s.puck_in_range() {
            self.set_cursor_grab();
            return true;
        }
        false
    }

    /// `mnPlayers1PBonusUpdateCostume`.
    fn update_costume(&mut self, button: usize) {
        if let Some(kind) = self.slot.kind {
            self.slot.costume = costume_common_id(kind, button);
        }
    }

    /// `mnPlayers1PBonusPuckProcUpdate`, ending with `MakeHiScore`.
    fn puck_update(&mut self) {
        self.view.puck_shown = self.puck_visible();
        self.slot.follow_or_move_puck();
        let kind = puck_fighter_kind(self.slot.puck, self.fighter_mask);
        if !self.slot.is_selected && kind != self.slot.kind {
            self.slot.kind = kind;
            self.update_fighter();
            self.v_update_name_and_emblem();
        }
        self.v_make_hiscore();
    }

    /// `mnPlayers1PBonusPuckAdjustProcUpdate`.
    fn puck_adjust(&mut self) {
        if self.slot.is_recalling {
            // `mnPlayers1PBonusPuckAdjustRecall`.
            if self.slot.recall_step() {
                self.set_cursor_grab();
                self.slot.puck_vel = (0.0, 0.0);
            }
            if self.slot.recall_tics == 30 {
                self.slot.is_recalling = false;
            }
        }
        if self.slot.is_selected {
            // `mnPlayers1PBonusPuckAdjustPlaced`: only the portrait edge.
            if let Some(kind) = self.slot.kind {
                let s = &mut self.slot;
                s.puck_vel = portrait_edge_velocity(kind, s.puck, s.puck_vel);
            }
        }
    }

    /// The records the select shows for `kind` (the fighter under the
    /// puck, [`Players1PBonus::force_puck_fighter_kind`]) in `bonus`, and
    /// the total when every fighter has finished it.
    pub fn records(&self, backup: &Backup) -> Records {
        records(
            backup,
            self.bonus,
            self.force_puck_fighter_kind(),
            self.fighter_mask,
        )
    }
}

/// `mnPlayers1PBonusGetCostume(fkind, 0)`. The source has no `return`; the
/// value left in `v0` is `ftParamGetCostumeCommonID(fkind,
/// ftParamGetCostumeCommonID(fkind, 0))`.
fn costume(kind: FighterKind) -> u8 {
    costume_common_id(kind, usize::from(costume_common_id(kind, 0)))
}

/// A time as the records draw it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Time {
    pub mins: i32,
    pub secs: i32,
    pub csecs: i32,
}

/// `mnPlayers1PBonusGetMins`, `GetSec` and `GetCSec`. Hundredths step
/// through 0, 1, 3, 5, 7, 9 over the six ticks of a tenth.
pub fn time(tics: u32) -> Time {
    let tics = tics as i32;
    let rest = tics % TIME_MIN;
    let tenths = (rest % TIME_SEC) / 6 * 10;
    let hundredths = ((rest % 6) as f32 / 0.554) as i32;
    Time {
        mins: tics / TIME_MIN,
        secs: rest / TIME_SEC,
        csecs: tenths + hundredths,
    }
}

/// `mnPlayers1PBonusGetBestTime`: `bonus`'s time, at most an hour less a
/// tick.
pub fn best_time(backup: &Backup, bonus: BonusKind, kind: FighterKind) -> u32 {
    let r = &backup.spgame_records[kind as usize];
    let t = match bonus {
        BonusKind::Targets => r.bonus1_time,
        BonusKind::Platforms => r.bonus2_time,
    };
    t.min(BEST_TIME_MAX)
}

/// `mnPlayers1PBonusGetBestTaskCount`.
pub fn best_task_count(backup: &Backup, bonus: BonusKind, kind: FighterKind) -> u8 {
    let r = &backup.spgame_records[kind as usize];
    match bonus {
        BonusKind::Targets => r.bonus1_task_count,
        BonusKind::Platforms => r.bonus2_task_count,
    }
}

/// `mnPlayers1PBonusCheckBonusComplete`: all ten targets or platforms.
pub fn is_complete(backup: &Backup, bonus: BonusKind, kind: FighterKind) -> bool {
    best_task_count(backup, bonus, kind) == BONUSGAME_TASK_MAX
}

/// The fighter's record line (`mnPlayers1PBonusMakeHiScore`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FighterRecord {
    /// `MakeBestTime`, for a finished practice.
    Time(Time),
    /// `MakeBestTaskCount`: targets broken or platforms boarded.
    Tasks(u8),
}

/// What the records draw.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Records {
    pub bonus: BonusKind,
    /// `None` with no portrait under the puck.
    pub fighter: Option<FighterRecord>,
    /// `mnPlayers1PBonusMakeTotalTime`'s minutes, seconds and hundredths,
    /// only when every fighter has finished (`CheckBonusCompleteAll`).
    pub total: Option<Time>,
}

/// `mnPlayers1PBonusMakeHiScore` for `kind` and, if
/// `mnPlayers1PBonusCheckBonusCompleteAll`, `MakeTotalTime`'s carried sums
/// (`GetTotalMins`, `GetTotalSec`, `GetTotalCSec` skip locked fighters;
/// the completeness check does not).
pub fn records(
    backup: &Backup,
    bonus: BonusKind,
    kind: Option<FighterKind>,
    fighter_mask: u16,
) -> Records {
    let fighter = kind.map(|k| {
        if is_complete(backup, bonus, k) {
            FighterRecord::Time(time(best_time(backup, bonus, k)))
        } else {
            FighterRecord::Tasks(best_task_count(backup, bonus, k))
        }
    });
    let all = FighterKind::PLAYABLE;
    let total = all.iter().all(|&k| is_complete(backup, bonus, k)).then(|| {
        let mut sum = Time {
            mins: 0,
            secs: 0,
            csecs: 0,
        };
        for &k in all.iter().filter(|&&k| !is_locked(k, fighter_mask)) {
            let t = time(best_time(backup, bonus, k));
            sum.mins += t.mins;
            sum.secs += t.secs;
            sum.csecs += t.csecs;
        }
        let carry = sum.csecs / 100;
        let secs = sum.secs + carry;
        Time {
            mins: sum.mins + secs / TIME_SEC,
            secs: secs % TIME_SEC,
            csecs: sum.csecs % 100,
        }
    });
    Records {
        bonus,
        fighter,
        total,
    }
}

#[path = "players_1p_bonus_layer.rs"]
pub mod layer;

#[cfg(test)]
#[path = "players_1p_bonus_tests.rs"]
mod tests;
