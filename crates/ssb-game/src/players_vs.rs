//! The VS character select — `mn/mnplayers/mnplayersvs.c`: four slots, each
//! with a player kind (HMN, CP or NA), a puck, and a hand cursor when its
//! controller is plugged in; the portrait grid, the recall, the costume
//! picks and syncs, the free-for-all / team battle toggle, the team, CPU
//! level and handicap buttons, the time or stock arrows, the back button,
//! the ready check, and the battle settings saved on the way out
//! (`mnPlayersVSSetSceneData`). The portraits, fighter models, names,
//! gates and shutters, spotlight, puck glow, flashes, blinking and sounds
//! are presentation and stay with the host.
//!
//! One [`PlayersVs::tick`] runs the scene's processes in the original
//! order: `mnPlayersVSFuncRun` (the scene GObj's `func_run`), then the
//! cursors (process priority 2, in the order their GObjs were made), then
//! the four pucks, the puck adjust and the costume sync (priority 1, by
//! link: 20, 26, 31). `mnPlayersVSPauseSlotProcesses` stops the cursors
//! and pucks for the rest of the scene, including the rest of the tick it
//! is called in.
//!
//! Controllers: `sMNPlayersVSControllerOrders[p] != -1` is
//! `connected[p]`, given to [`PlayersVs::new`] and changed with
//! [`PlayersVs::set_connected`]; `mnPlayersVSUpdateControllerOrders` reads
//! it at the top of every tick, so a change takes effect through
//! `mnPlayersVSUpdateGate` on the next tick. `tick` takes all four ports'
//! input; a port that is not connected is never read (the source filters
//! them the same way), so a one-controller host passes port 0 and
//! [`Pad::default`] for the rest. `ControllerState::connected` is not
//! used.
//!
//! Not ported:
//! - The GObj display flags (`mnPlayersVSUpdatePuckDisplay`'s hide/show,
//!   the spotlight, ready banner and arrow blinking, the portrait flash,
//!   `mnPlayersVSFighterProcUpdate`'s turn and `is_status_selected`).
//!   [`PlayersVs::puck_visible`] gives `mnPlayersVSPuckProcUpdate`'s rule.
//! - The shutter doors' `door_offset`, which only moves sprites.
//! - `mnPlayersVSGetNextPortraitX`'s slide-in of the portraits.
//! - `func_800266A0_272A0` and every sound or announcer voice.

use ssb_engine::input::{ControllerState, N64Buttons};

use crate::battle::{Rule, TIMELIMIT_INFINITE};
use crate::costume::{costume_common_id, costume_team_id};
use crate::fighter::FighterKind;
use crate::fighter_select::{
    is_locked, portrait_center, portrait_edge_velocity, puck_fighter_kind, CursorStatus,
    PUCK_HEIGHT, PUCK_WIDTH,
};
use crate::stage_select::{gkind, UNLOCK_MASK_INISHIE};

/// `GMCOMMON_PLAYERS_MAX`.
pub const PLAYERS: usize = 4;

/// `nSCBattleTeamID*`: the three team battle teams.
pub const TEAM_RED: u8 = 0;
pub const TEAM_BLUE: u8 = 1;
pub const TEAM_GREEN: u8 = 2;

/// `mnPlayersVSResetPlayer`'s `default_teams`.
const DEFAULT_TEAMS: [u8; PLAYERS] = [TEAM_RED, TEAM_RED, TEAM_BLUE, TEAM_BLUE];

/// `dIFCommonPlayerTeamColorIDs` (`if/ifcommon.c`), indexed by team.
pub const TEAM_COLOR_IDS: [u8; 5] = [0, 1, 3, 4, 0];

/// `I_MIN_TO_TICS(5)`.
const RETURN_TICS: i32 = 5 * 60 * 60;
/// `I_SEC_TO_TICS(1)`: START is read only after this many ticks.
const START_TICS: i32 = 60;
/// `sMNPlayersVSStartProceedWait`'s start: ticks between a ready START and
/// leaving.
const START_PROCEED_WAIT: i32 = 30;
/// `mnPlayersVSSelectFighter`: ticks after a placement before the cursor
/// can grab again.
const GRAB_WAIT: i32 = 30;
/// `mnPlayersVSPuckProcUpdate`: the pucks are hidden before this tick.
const PUCK_SHOW_TICS: i32 = 30;
/// `mnPlayersVSDetectBack`: B held this many ticks returns to VS mode.
const HOLD_B_TICS: i32 = 40;
/// `mnPlayersVSCheckHandicapArrowInRangeAll`'s bounds.
const VALUE_MIN: u8 = 1;
const VALUE_MAX: u8 = 9;
/// `mnPlayersVSCursorProcUpdate`: the stock arrows wrap 0 to 98.
const STOCK_MAX: u8 = 98;

/// `scSubsysControllerCheckNoInputAll`'s held-button mask: every button.
const IDLE_BUTTONS: u16 = N64Buttons::A
    | N64Buttons::B
    | N64Buttons::START
    | N64Buttons::L
    | N64Buttons::R
    | N64Buttons::Z
    | N64Buttons::C_UP
    | N64Buttons::C_LEFT
    | N64Buttons::C_RIGHT
    | N64Buttons::C_DOWN
    | N64Buttons::D_UP
    | N64Buttons::D_LEFT
    | N64Buttons::D_RIGHT
    | N64Buttons::D_DOWN;

/// `mnPlayersVSGetNextTimeValue`/`GetPrevTimeValue`'s `timer_values`.
const TIME_VALUES: [u8; 8] = [2, 3, 5, 10, 15, 30, 60, TIMELIMIT_INFINITE];

/// The select button a pick came from: `nMNPlayersSelectButton*`.
const BUTTON_A: usize = 4;

/// `mnPlayersVSMakeCursor`'s start positions.
const CURSOR_STARTS: [(f32, f32); PLAYERS] = [
    (40.0, 170.0),
    (108.0, 170.0),
    (176.0, 170.0),
    (244.0, 170.0),
];
/// `mnPlayersVSMakePuck`: a puck with no fighter waits here.
const PUCK_START: (f32, f32) = (51.0, 161.0);

/// The display links the cursors and pucks draw in.
const DL_CURSOR: u8 = 32;
const DL_PUCK: u8 = 33;
/// `mnPlayersVSMakeCursor`'s `priorities`, also the held-puck and
/// placement tables (`held_priorities`, `unheld_priorities`).
const CURSOR_PRIORITIES: [u32; PLAYERS] = [6, 4, 2, 0];
/// `mnPlayersVSMakePuck`'s `dropped_priorities`.
const PUCK_DROPPED_PRIORITIES: [u32; PLAYERS] = [3, 2, 1, 0];

/// `nFTPlayerKind`: the kinds a VS slot takes, in the source's order
/// (the HMN/CP/NA button cycles through them).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum PlayerKind {
    Man,
    Com,
    #[default]
    Not,
}

/// `nSCBattleHandicap*`: VS options' handicap setting.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Handicap {
    #[default]
    Off,
    On,
    Auto,
}

/// One `gSCManagerTransferBattleState.players[]` entry, the fields this
/// scene reads or writes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlayerState {
    /// `level`: CPU level, 1 to 9.
    pub level: u8,
    /// `handicap`, 1 to 9.
    pub handicap: u8,
    pub pkind: PlayerKind,
    /// `fkind`; `None` is `nFTKindNull`.
    pub fkind: Option<FighterKind>,
    pub team: u8,
    /// `player`: the port, or the team in a team battle.
    pub player: u8,
    pub costume: u8,
    pub shade: u8,
    pub color: u8,
    pub is_single_stockicon: bool,
    pub tag: u8,
}

/// `gSCManagerTransferBattleState`, the fields this scene reads or writes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BattleState {
    pub is_team_battle: bool,
    /// `is_team_attack`, which only VS Options (not ported) changes.
    pub is_team_attack: bool,
    /// `game_rules`: `SCBATTLE_GAMERULE_TIME` or `_STOCK`.
    pub rule: Rule,
    /// Minutes, or [`TIMELIMIT_INFINITE`].
    pub time_limit: u8,
    /// Stocks less one: 0 to 98.
    pub stocks: u8,
    pub handicap: Handicap,
    pub is_stage_select: bool,
    /// Start with the slots reset (`mnPlayersVSResetPlayer`); cleared on
    /// the first visit.
    pub is_reset_players: bool,
    pub pl_count: u8,
    pub cp_count: u8,
    pub players: [PlayerState; PLAYERS],
}

impl Default for BattleState {
    /// `dSCManagerDefaultBattleState` (`sc/scmanager.c`), which
    /// `scManagerRunLoop` copies into `gSCManagerTransferBattleState` at
    /// boot.
    fn default() -> Self {
        let player = |tag: u8, team: u8| PlayerState {
            level: 3,
            handicap: 9,
            pkind: PlayerKind::Not,
            fkind: None,
            team,
            player: 0,
            costume: 0,
            shade: 0,
            color: 0,
            is_single_stockicon: true,
            tag,
        };
        BattleState {
            is_team_battle: false,
            is_team_attack: false,
            rule: Rule::Time,
            time_limit: 3,
            stocks: 2,
            handicap: Handicap::Off,
            is_stage_select: true,
            is_reset_players: true,
            pl_count: 0,
            cp_count: 0,
            players: [player(0, 0), player(1, 0), player(2, 1), player(3, 1)],
        }
    }
}

/// The rest of what the scene reads at start: `gSCManagerBackupData` and
/// `gSCManagerSceneData`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct SceneContext {
    /// `gSCManagerBackupData.fighter_mask`.
    pub fighter_mask: u16,
    /// `gSCManagerBackupData.unlock_mask`.
    pub unlock_mask: u8,
    /// `gSCManagerSceneData.gkind`: the last stage, which a random stage
    /// pick avoids.
    pub gkind: u8,
}

/// One port's input this tick.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Pad {
    /// `button_hold` and `stick_range`.
    pub state: ControllerState,
    /// `button_tap`.
    pub taps: N64Buttons,
}

/// What a tick asks the host to do. Each carries the battle state saved on
/// the way out (`mnPlayersVSSetSceneData`). When two scene changes land in
/// one tick the later one wins, as the source's last
/// `gSCManagerSceneData.scene_curr` write does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// The ready START's wait ran out with the stage select on
    /// (`nSCKindMaps`).
    Maps(BattleState),
    /// The same with the stage select off (`nSCKindVSBattle`): `gkind` is
    /// the random stage now in `gSCManagerSceneData.gkind`.
    Battle { state: BattleState, gkind: u8 },
    /// B held, or A on the back button: back to the VS mode menu.
    VsMode(BattleState),
    /// Five idle minutes: back to the title.
    Title(BattleState),
}

/// Where a GObj sits in its display list: `gcMoveGObjDL`'s link and
/// priority, and `seq`, which orders GObjs of equal priority. The lists
/// draw by descending priority, then ascending `seq` (`gcDLLinkGObjTail`
/// puts a GObj after the others of its priority).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct DrawKey {
    pub link: u8,
    pub priority: u32,
    pub seq: u32,
}

/// One `MNPlayersSlotVS`, the fields the logic reads.
#[derive(Clone, Debug, Default)]
pub struct Slot {
    pub pkind: PlayerKind,
    /// `fkind`; `None` is `nFTKindNull`.
    pub fkind: Option<FighterKind>,
    pub costume: u8,
    pub shade: u8,
    pub team: u8,
    pub cpu_level: u8,
    pub handicap: u8,
    pub cursor_status: CursorStatus,
    pub is_selected: bool,
    pub is_fighter_selected: bool,
    pub is_recalling: bool,
    recall_end_tic: i32,
    recall_start: (f32, f32),
    recall_end_x: f32,
    recall_mid_y: f32,
    recall_end_y: f32,
    recall_tics: i32,
    /// `holder_player`; `None` is `GMCOMMON_PLAYERS_MAX`.
    pub holder: Option<usize>,
    /// `held_player`; `None` is -1.
    pub held: Option<usize>,
    /// The puck sprite's top-left corner.
    pub puck: (f32, f32),
    puck_vel: (f32, f32),
    /// The cursor sprite's top-left corner; `None` when the slot has no
    /// cursor GObj (its controller is unplugged).
    pub cursor: Option<(f32, f32)>,
    cursor_pickup: (f32, f32),
    is_cursor_adjusting: bool,
    is_hold_b: bool,
    hold_b_tics: i32,
    /// Whether `mnPlayersVSMakeFighter` has made the slot's fighter
    /// (`player != NULL`).
    has_fighter: bool,
    /// When the cursor GObj was made: its process runs in this order.
    cursor_made: u32,
    pub cursor_draw: DrawKey,
    pub puck_draw: DrawKey,
}

/// The VS character select's state.
#[derive(Clone, Debug)]
pub struct PlayersVs {
    pub slots: [Slot; PLAYERS],
    connected: [bool; PLAYERS],
    /// `sMNPlayersVSTimeValue`.
    pub time_value: u8,
    /// `sMNPlayersVSStockValue`.
    pub stock_value: u8,
    pub is_team_battle: bool,
    /// `sMNPlayersVSGameRule`.
    pub rule: Rule,
    handicap: Handicap,
    is_stage_select: bool,
    fighter_mask: u16,
    unlock_mask: u8,
    gkind: u8,
    total_tics: i32,
    return_tic: i32,
    is_start: bool,
    start_proceed_wait: i32,
    is_slot_paused: bool,
    seq: u32,
    /// `gSCManagerTransferBattleState` as this scene leaves it.
    saved: BattleState,
    pending: Option<Outcome>,
}

/// `mnPlayersVSGetNextTimeValue`.
pub fn next_time_value(current: u8) -> u8 {
    if current == TIME_VALUES[TIME_VALUES.len() - 1] {
        return TIME_VALUES[0];
    }
    TIME_VALUES
        .into_iter()
        .find(|&v| current < v)
        .unwrap_or(TIME_VALUES[TIME_VALUES.len() - 1])
}

/// `mnPlayersVSGetPrevTimeValue`.
pub fn prev_time_value(current: u8) -> u8 {
    if current == TIME_VALUES[0] {
        return TIME_VALUES[TIME_VALUES.len() - 1];
    }
    TIME_VALUES
        .into_iter()
        .rev()
        .find(|&v| current > v)
        .unwrap_or(TIME_VALUES[TIME_VALUES.len() - 1])
}

/// `syUtilsRandTimeUCharRange(range)` from one `osGetTime() & 0xFF`.
fn rand_time_range(time_byte: u8, range: i32) -> i32 {
    i32::from(time_byte) * range / 256
}

/// A cursor hit box test: the cursor's `(x + dx, y + 3)` point inside
/// `x0..=x1`, `y0..=y1`.
fn cursor_in(cursor: (f32, f32), dx: f32, x: (f32, f32), y: (f32, f32)) -> bool {
    let px = cursor.0 + dx;
    let py = cursor.1 + 3.0;
    (x.0..=x.1).contains(&px) && (y.0..=y.1).contains(&py)
}

/// `mnPlayersVSCheckTimeArrowRInRange`.
fn time_arrow_r_in_range(c: (f32, f32)) -> bool {
    cursor_in(c, 20.0, (210.0, 230.0), (12.0, 35.0))
}

/// `mnPlayersVSCheckTimeArrowLInRange`.
fn time_arrow_l_in_range(c: (f32, f32)) -> bool {
    cursor_in(c, 20.0, (140.0, 160.0), (12.0, 35.0))
}

/// `mnPlayersVSCheckGameModeInRange`.
fn game_mode_in_range(c: (f32, f32)) -> bool {
    cursor_in(c, 20.0, (27.0, 137.0), (14.0, 35.0))
}

/// `mnPlayersVSCheckBackInRange`.
fn back_in_range(c: (f32, f32)) -> bool {
    cursor_in(c, 20.0, (244.0, 292.0), (13.0, 34.0))
}

/// The left edge of `player`'s panel: `player * 69`.
fn panel_x(player: usize) -> f32 {
    (player * 69) as f32
}

/// `mnPlayersVSCheckTeamSelectInRange`.
fn team_select_in_range(c: (f32, f32), player: usize) -> bool {
    let x = panel_x(player);
    cursor_in(c, 20.0, (x + 34.0, x + 58.0), (131.0, 141.0))
}

/// `mnPlayersVSCheckHandicapArrowRInRange`.
fn handicap_arrow_r_in_range(c: (f32, f32), player: usize) -> bool {
    let x = panel_x(player);
    cursor_in(c, 20.0, (x + 68.0, x + 90.0), (197.0, 216.0))
}

/// `mnPlayersVSCheckHandicapArrowLInRange`.
fn handicap_arrow_l_in_range(c: (f32, f32), player: usize) -> bool {
    let x = panel_x(player);
    cursor_in(c, 20.0, (x + 21.0, x + 43.0), (197.0, 216.0))
}

/// `mnPlayersVSCheckPlayerKindSelectInRange`: the HMN/CP/NA button.
fn player_kind_select_in_range(c: (f32, f32), player: usize) -> bool {
    let x = panel_x(player);
    cursor_in(c, 20.0, (x + 60.0, x + 88.0), (127.0, 145.0))
}

impl PlayersVs {
    /// `mnPlayersVSFuncStart`'s logic: `mnPlayersVSUpdateControllerOrders`,
    /// `mnPlayersVSInitVars` (with `mnPlayersVSInitPlayer` or
    /// `mnPlayersVSResetPlayer`) and `mnPlayersVSInitSlotAll`.
    pub fn new(state: BattleState, scene: SceneContext, connected: [bool; PLAYERS]) -> Self {
        let mut select = PlayersVs {
            slots: Default::default(),
            connected,
            time_value: state.time_limit,
            stock_value: state.stocks,
            is_team_battle: state.is_team_battle,
            rule: state.rule,
            handicap: state.handicap,
            is_stage_select: state.is_stage_select,
            fighter_mask: scene.fighter_mask,
            unlock_mask: scene.unlock_mask,
            gkind: scene.gkind,
            total_tics: 0,
            return_tic: RETURN_TICS,
            is_start: false,
            start_proceed_wait: 0,
            is_slot_paused: false,
            seq: 0,
            saved: state,
            pending: None,
        };
        for p in 0..PLAYERS {
            if state.is_reset_players {
                select.reset_player(p);
                select.saved.is_reset_players = false;
            } else {
                select.init_player(p);
            }
            select.slots[p].recall_end_tic = 0;
        }
        for p in 0..PLAYERS {
            select.init_slot(p);
        }
        select
    }

    /// `mnPlayersVSInitPlayer`: the slots as the last visit left them.
    fn init_player(&mut self, p: usize) {
        let saved = self.saved.players[p];
        let connected = self.connected[p];
        let s = &mut self.slots[p];
        s.fkind = saved.fkind;
        if saved.pkind == PlayerKind::Man && !connected {
            s.pkind = PlayerKind::Not;
            s.fkind = None;
        } else {
            s.pkind = saved.pkind;
        }
        s.cpu_level = saved.level;
        s.handicap = saved.handicap;
        s.team = saved.team;
        if s.pkind == PlayerKind::Man && s.fkind.is_none() {
            s.holder = Some(p);
            s.held = Some(p);
        } else {
            s.holder = None;
            s.held = None;
        }
        let placed = s.fkind.is_some();
        s.is_fighter_selected = placed;
        s.is_selected = placed;
        s.is_recalling = false;
        s.costume = saved.costume;
        s.shade = saved.shade;
        if connected && s.pkind == PlayerKind::Not {
            s.holder = Some(p);
        }
    }

    /// `mnPlayersVSResetPlayer`: a plugged-in port starts as a human
    /// holding its own puck, the rest as NA.
    fn reset_player(&mut self, p: usize) {
        let saved = self.saved.players[p];
        let connected = self.connected[p];
        let s = &mut self.slots[p];
        s.is_selected = false;
        s.cpu_level = saved.level;
        s.handicap = saved.handicap;
        s.fkind = None;
        s.is_recalling = false;
        s.team = DEFAULT_TEAMS[p];
        if connected {
            s.pkind = PlayerKind::Man;
            s.holder = Some(p);
            s.held = Some(p);
        } else {
            s.pkind = PlayerKind::Not;
            s.holder = None;
            s.held = None;
        }
    }

    /// `mnPlayersVSInitSlot`: `mnPlayersVSMakeCursor`, `mnPlayersVSMakePuck`
    /// and the placed fighter's `mnPlayersVSMakeFighter`.
    fn init_slot(&mut self, p: usize) {
        if self.connected[p] {
            self.make_cursor(p);
        } else {
            self.slots[p].cursor = None;
        }
        // `mnPlayersVSMakePuck`.
        self.slots[p].puck_draw = self.draw_key(DL_PUCK, PUCK_DROPPED_PRIORITIES[p]);
        if self.slots[p].pkind == PlayerKind::Man && self.slots[p].held.is_some() {
            self.slots[p].puck_draw = self.draw_key(DL_CURSOR, CURSOR_PRIORITIES[p] + 1);
        }
        let s = &mut self.slots[p];
        s.puck = match s.fkind {
            None => PUCK_START,
            Some(kind) => portrait_center(kind),
        };
        if s.is_selected && s.fkind.is_some() {
            s.has_fighter = true;
        }
    }

    /// `mnPlayersVSMakeCursor`.
    fn make_cursor(&mut self, p: usize) {
        let draw = self.draw_key(DL_CURSOR, CURSOR_PRIORITIES[p]);
        let made = self.next_seq();
        let s = &mut self.slots[p];
        s.cursor = Some(CURSOR_STARTS[p]);
        s.cursor_draw = draw;
        s.cursor_made = made;
    }

    fn next_seq(&mut self) -> u32 {
        self.seq += 1;
        self.seq
    }

    fn draw_key(&mut self, link: u8, priority: u32) -> DrawKey {
        DrawKey {
            link,
            priority,
            seq: self.next_seq(),
        }
    }

    /// Replugs or unplugs ports; read at the top of the next tick.
    pub fn set_connected(&mut self, connected: [bool; PLAYERS]) {
        self.connected = connected;
    }

    /// Frames since the scene started (`sMNPlayersVSTotalTimeTics`).
    pub fn total_tics(&self) -> i32 {
        self.total_tics
    }

    /// `sMNPlayersVSIsStart`: START was accepted and the scene is waiting
    /// to leave.
    pub fn is_start(&self) -> bool {
        self.is_start
    }

    /// Whether `slot`'s puck draws this tick (`mnPlayersVSPuckProcUpdate`).
    pub fn puck_visible(&self, slot: usize) -> bool {
        let s = &self.slots[slot];
        self.total_tics >= PUCK_SHOW_TICS
            && (s.pkind == PlayerKind::Com
                || (s.pkind == PlayerKind::Man
                    && (s.cursor_status != CursorStatus::Pointer
                        || s.is_selected
                        || s.is_recalling)))
    }

    /// `mnPlayersVSCheckHandicapOn`.
    fn is_handicap_on(&self) -> bool {
        self.handicap == Handicap::On
    }

    /// `mnPlayersVSGetShade`: in a team battle, the first shade no other
    /// slot with this fighter on this team wears.
    fn shade(&self, p: usize) -> u8 {
        if !self.is_team_battle {
            return 0;
        }
        let mut used = [false; PLAYERS];
        for (i, other) in self.slots.iter().enumerate() {
            if i != p && other.fkind == self.slots[p].fkind && other.team == self.slots[p].team {
                // `mnPlayersVSUpdateGameMode` parks shades at 4, one past
                // the source's four-entry array, so they mark nothing.
                if let Some(u) = used.get_mut(usize::from(other.shade)) {
                    *u = true;
                }
            }
        }
        used.iter().position(|&u| !u).unwrap_or(0) as u8
    }

    /// `mnPlayersVSGetFighterKindCount`: how many other slots show `kind`.
    fn fighter_kind_count(&self, kind: FighterKind) -> usize {
        let count = self.slots.iter().filter(|s| s.fkind == Some(kind)).count();
        count.saturating_sub(1)
    }

    /// `mnPlayersVSCheckCostumeUsed`.
    fn costume_used(&self, kind: FighterKind, p: usize, costume: u8) -> bool {
        self.slots
            .iter()
            .enumerate()
            .any(|(i, s)| i != p && s.fkind == Some(kind) && s.costume == costume)
    }

    /// `mnPlayersVSGetFreeCostumeRoyal`: the first royal costume index no
    /// other slot with this fighter wears.
    fn free_costume_royal(&self, kind: FighterKind, p: usize) -> usize {
        let mut used = [false; PLAYERS];
        for (i, s) in self.slots.iter().enumerate() {
            if i != p && s.fkind == Some(kind) {
                for (j, u) in used.iter_mut().enumerate() {
                    if costume_common_id(kind, j) == s.costume {
                        *u = true;
                    }
                }
            }
        }
        // Three other slots cannot use all four; the source has no return
        // past the loop.
        used.iter().position(|&u| !u).unwrap_or(0)
    }

    /// `mnPlayersVSGetFreeCostume`: free-for-all takes the first free
    /// royal costume, a team battle the team's.
    fn free_costume(&self, kind: FighterKind, p: usize) -> u8 {
        if self.is_team_battle {
            costume_team_id(kind, self.slots[p].team)
        } else {
            costume_common_id(kind, self.free_costume_royal(kind, p))
        }
    }

    /// `mnPlayersVSGetReadyPlayerCount`.
    pub fn ready_player_count(&self) -> usize {
        self.slots
            .iter()
            .filter(|s| s.pkind != PlayerKind::Not && s.is_fighter_selected)
            .count()
    }

    /// `mnPlayersVSCheckSingleTeam`: every placed fighter is on one team.
    pub fn is_single_team(&self) -> bool {
        let mut team = None;
        for s in self.slots.iter().filter(|s| s.is_fighter_selected) {
            match team {
                None => team = Some(s.team),
                Some(t) if t != s.team => return false,
                Some(_) => {}
            }
        }
        true
    }

    /// `mnPlayersVSCheckNoPuckOnPortraitAll`: no cursor is holding a puck.
    pub fn is_no_puck_held(&self) -> bool {
        !self
            .slots
            .iter()
            .any(|s| s.cursor.is_some() && s.cursor_status == CursorStatus::Grab)
    }

    /// `mnPlayersVSCheckReady`.
    pub fn is_ready(&self) -> bool {
        self.ready_player_count() >= 2
            && !(self.is_team_battle && self.is_single_team())
            && self.is_no_puck_held()
    }

    /// `mnPlayersVSSetSceneData`.
    fn set_scene_data(&mut self) {
        let st = &mut self.saved;
        st.time_limit = self.time_value;
        st.stocks = self.stock_value;
        st.is_team_battle = self.is_team_battle;
        st.rule = self.rule;
        for (i, (out, s)) in st.players.iter_mut().zip(self.slots.iter()).enumerate() {
            if self.is_team_battle {
                out.player = s.team;
                out.team = s.team;
            } else {
                out.player = i as u8;
            }
            out.fkind = s.fkind;
            out.pkind = s.pkind;
            out.costume = s.costume;
            out.shade = s.shade;
            let team_color = TEAM_COLOR_IDS[usize::from(out.team).min(4)];
            out.color = if out.pkind == PlayerKind::Man {
                if self.is_team_battle {
                    team_color
                } else {
                    i as u8
                }
            } else if !self.is_team_battle {
                PLAYERS as u8
            } else {
                team_color
            };
            out.tag = if out.pkind == PlayerKind::Man {
                i as u8
            } else {
                PLAYERS as u8
            };
            out.is_single_stockicon = self.rule == Rule::Time;
            if out.pkind == PlayerKind::Com {
                out.level = s.cpu_level;
            } else {
                out.handicap = s.handicap;
            }
        }
        st.pl_count = st
            .players
            .iter()
            .filter(|p| p.pkind == PlayerKind::Man)
            .count() as u8;
        st.cp_count = st
            .players
            .iter()
            .filter(|p| p.pkind == PlayerKind::Com)
            .count() as u8;
    }

    /// `syTaskmanSetLoadScene` with the scene data saved now.
    fn leave(&mut self, outcome: impl FnOnce(BattleState) -> Outcome) {
        self.set_scene_data();
        self.pending = Some(outcome(self.saved));
    }

    /// `mnPlayersVSPauseSlotProcesses`.
    fn pause_slot_processes(&mut self) {
        self.is_slot_paused = true;
    }

    /// `mnPlayersVSBackToVSMode`.
    fn back_to_vs_mode(&mut self) {
        self.leave(Outcome::VsMode);
        self.pause_slot_processes();
    }

    /// One frame. `pads` are the four ports' input; `time_byte` stands for
    /// `osGetTime() & 0xFF`, read once per `syUtilsRandTimeUCharRange`
    /// draw (random fighters and the random stage).
    pub fn tick(
        &mut self,
        pads: &[Pad; PLAYERS],
        time_byte: &mut dyn FnMut() -> u8,
    ) -> Option<Outcome> {
        self.pending = None;
        self.func_run(pads, time_byte);
        let mut order = [0, 1, 2, 3];
        order.sort_unstable_by_key(|&p| self.slots[p].cursor_made);
        for p in order {
            if self.slots[p].cursor.is_some() && !self.is_slot_paused {
                self.cursor_update(p, &pads[p], time_byte);
            }
        }
        for p in 0..PLAYERS {
            if !self.is_slot_paused {
                self.puck_update(p, time_byte);
            }
        }
        self.puck_adjust();
        self.costume_sync();
        self.pending
    }

    /// `scSubsysControllerCheckNoInputAll` over the connected ports.
    fn no_input_all(&self, pads: &[Pad; PLAYERS]) -> bool {
        let stick_in = |v: i8| (-20..=20).contains(&v);
        pads.iter().zip(self.connected).all(|(pad, connected)| {
            !connected
                || (stick_in(pad.state.stick_x)
                    && stick_in(pad.state.stick_y)
                    && pad.state.buttons.0 & IDLE_BUTTONS == 0)
        })
    }

    /// `mnPlayersVSFuncRun`.
    fn func_run(&mut self, pads: &[Pad; PLAYERS], time_byte: &mut dyn FnMut() -> u8) {
        self.total_tics += 1;
        if self.total_tics == self.return_tic {
            self.leave(Outcome::Title);
            return;
        }
        if !self.no_input_all(pads) {
            self.return_tic = self.total_tics + RETURN_TICS;
        }
        if self.is_start {
            self.start_proceed_wait -= 1;
            if self.start_proceed_wait == 0 {
                if self.is_stage_select {
                    self.leave(Outcome::Maps);
                } else {
                    let count = if self.unlock_mask & UNLOCK_MASK_INISHIE != 0 {
                        i32::from(gkind::INISHIE) + 1
                    } else {
                        i32::from(gkind::YAMABUKI) + 1
                    };
                    let gkind = loop {
                        let k = rand_time_range(time_byte(), count) as u8;
                        if k != self.gkind {
                            break k;
                        }
                    };
                    self.gkind = gkind;
                    self.leave(|state| Outcome::Battle { state, gkind });
                }
            }
        } else {
            // `scSubsysControllerGetPlayerTapButtons(START_BUTTON)`.
            let start = pads
                .iter()
                .zip(self.connected)
                .any(|(pad, connected)| connected && pad.taps.contains(N64Buttons::START));
            if start && self.total_tics > START_TICS && self.is_ready() {
                self.set_idle_player_not_all();
                self.start_proceed_wait = START_PROCEED_WAIT;
                self.is_start = true;
                self.pause_slot_processes();
            }
            for p in 0..PLAYERS {
                self.update_gate(p, time_byte);
            }
        }
    }

    /// `mnPlayersVSSetPlayerNot`.
    fn set_player_not(&mut self, p: usize) {
        let s = &mut self.slots[p];
        s.pkind = PlayerKind::Not;
        s.fkind = None;
        s.holder = None;
    }

    /// `mnPlayersVSSetIdlePlayerNotAll`: slots with no fighter placed sit
    /// the battle out.
    fn set_idle_player_not_all(&mut self) {
        for p in 0..PLAYERS {
            if !self.slots[p].is_fighter_selected {
                self.set_player_not(p);
            }
        }
    }

    /// `mnPlayersVSUpdateGate`: a newly plugged port gets a cursor and
    /// (unless it is a CPU) a human; an unplugged one drops what it holds
    /// and (unless it is a CPU) goes NA.
    fn update_gate(&mut self, p: usize, time_byte: &mut dyn FnMut() -> u8) {
        if self.connected[p] {
            if self.slots[p].cursor.is_none() {
                self.make_cursor(p);
                if self.slots[p].pkind != PlayerKind::Com {
                    self.slots[p].pkind = PlayerKind::Man;
                    self.refresh_player_kind(p, time_byte);
                }
            }
        } else if self.slots[p].cursor.is_some() {
            if let Some(held) = self.slots[p].held {
                if (p != held || self.slots[p].pkind == PlayerKind::Com)
                    && !self.select_fighter(p, BUTTON_A)
                {
                    self.slots[held].fkind = Some(self.rand_fighter_kind(held, time_byte));
                }
            }
            self.slots[p].cursor = None;
            if self.slots[p].pkind != PlayerKind::Com {
                self.slots[p].pkind = PlayerKind::Not;
                self.refresh_player_kind(p, time_byte);
            }
        }
    }

    /// The calls that follow a player kind change in `UpdateGate` and
    /// `CheckPlayerKindSelect`: `mnPlayersVSUpdatePlayerKind`,
    /// `UpdatePuckDisplay`, `UpdateCursorDisplay` and `UpdateFighter`.
    fn refresh_player_kind(&mut self, p: usize, time_byte: &mut dyn FnMut() -> u8) {
        self.update_player_kind(p, time_byte);
        self.update_puck_display(p);
        self.update_cursor_display(p);
        self.update_fighter(p);
    }

    /// `mnPlayersVSRandFighterKind`: an unlocked fighter, with `p`'s puck
    /// centred on its portrait.
    fn rand_fighter_kind(&mut self, p: usize, time_byte: &mut dyn FnMut() -> u8) -> FighterKind {
        let kind = loop {
            let k = FighterKind::PLAYABLE[rand_time_range(time_byte(), 12) as usize];
            if !is_locked(k, self.fighter_mask) {
                break k;
            }
        };
        self.slots[p].puck = portrait_center(kind);
        kind
    }

    /// Releases the puck `p`'s cursor holds, as placed, for a kind change.
    fn release_held(&mut self, p: usize) {
        if let Some(held) = self.slots[p].held {
            let h = &mut self.slots[held];
            h.holder = None;
            h.is_selected = true;
            h.is_fighter_selected = true;
            self.update_cursor_placement_priorities(Some(p), held);
        }
    }

    /// `mnPlayersVSUpdatePlayerKind`.
    fn update_player_kind(&mut self, p: usize, time_byte: &mut dyn FnMut() -> u8) {
        match self.slots[p].pkind {
            PlayerKind::Man => {
                self.release_held(p);
                let s = &mut self.slots[p];
                s.is_selected = false;
                s.fkind = None;
                s.is_fighter_selected = false;
                s.holder = Some(p);
                s.held = Some(p);
                self.update_cursor_grab_priorities(p, p);
                self.slots[p].is_cursor_adjusting = false;
            }
            PlayerKind::Com => {
                self.release_held(p);
                let s = &mut self.slots[p];
                s.is_selected = true;
                s.holder = None;
                s.held = None;
                self.update_cursor_placement_priorities(None, p);
                self.slots[p].is_fighter_selected = true;
                if self.slots[p].fkind.is_none() {
                    self.slots[p].fkind = Some(self.rand_fighter_kind(p, time_byte));
                }
                self.slots[p].is_cursor_adjusting = false;
            }
            PlayerKind::Not => {
                if let Some(holder) = self.slots[p].holder {
                    // The source marks the holder's own slot selected.
                    let h = &mut self.slots[holder];
                    h.held = None;
                    h.is_selected = true;
                    h.cursor_status = CursorStatus::Hover;
                }
                self.release_held(p);
                let s = &mut self.slots[p];
                s.is_selected = false;
                s.held = None;
                s.fkind = None;
                s.is_fighter_selected = false;
                s.is_cursor_adjusting = false;
                if self.connected[p] {
                    s.holder = Some(p);
                }
            }
        }
    }

    /// `mnPlayersVSUpdatePuckDisplay`: a human's puck goes back in hand, a
    /// CPU's is placed.
    fn update_puck_display(&mut self, p: usize) {
        let s = &mut self.slots[p];
        match s.pkind {
            PlayerKind::Man => s.is_selected = false,
            PlayerKind::Com => s.is_selected = true,
            PlayerKind::Not => {}
        }
    }

    /// `mnPlayersVSUpdateCursorDisplay`.
    fn update_cursor_display(&mut self, p: usize) {
        let s = &mut self.slots[p];
        let Some((_, y)) = s.cursor else {
            return;
        };
        s.cursor_status = if y > 122.0 || y < 36.0 {
            CursorStatus::Pointer
        } else if s.is_selected || s.pkind == PlayerKind::Not {
            CursorStatus::Hover
        } else {
            CursorStatus::Grab
        };
    }

    /// `mnPlayersVSUpdateFighter`: the fighter model is remade with the
    /// free costume, unless the slot is NA or shows nobody.
    fn update_fighter(&mut self, p: usize) {
        let s = &self.slots[p];
        let skip =
            s.has_fighter && (s.pkind == PlayerKind::Not || (s.fkind.is_none() && !s.is_selected));
        if skip {
            return;
        }
        self.slots[p].shade = self.shade(p);
        if let Some(kind) = self.slots[p].fkind {
            // `mnPlayersVSMakeFighter`.
            self.slots[p].costume = self.free_costume(kind, p);
            self.slots[p].has_fighter = true;
        }
    }

    /// `mnPlayersVSCheckPlayerKindSelect`: `p`'s cursor on `sel`'s
    /// HMN/CP/NA button cycles it (a port with no controller skips HMN).
    fn check_player_kind_select(
        &mut self,
        p: usize,
        sel: usize,
        time_byte: &mut dyn FnMut() -> u8,
    ) -> bool {
        if !player_kind_select_in_range(self.cursor_pos(p), sel) {
            return false;
        }
        let s = &mut self.slots[sel];
        s.pkind = match (s.pkind, self.connected[sel]) {
            (PlayerKind::Man, _) => PlayerKind::Com,
            (PlayerKind::Com, _) => PlayerKind::Not,
            (PlayerKind::Not, true) => PlayerKind::Man,
            (PlayerKind::Not, false) => PlayerKind::Com,
        };
        self.refresh_player_kind(sel, time_byte);
        match self.slots[sel].pkind {
            PlayerKind::Man => self.slots[sel].holder = Some(sel),
            PlayerKind::Com => self.slots[sel].holder = None,
            PlayerKind::Not => {}
        }
        true
    }

    /// `mnPlayersVSCheckPlayerKindSelectAllPlayer`: every button is tried.
    fn check_player_kind_select_all(
        &mut self,
        p: usize,
        time_byte: &mut dyn FnMut() -> u8,
    ) -> bool {
        let mut pressed = false;
        for sel in 0..PLAYERS {
            pressed |= self.check_player_kind_select(p, sel, time_byte);
        }
        pressed
    }

    fn cursor_pos(&self, p: usize) -> (f32, f32) {
        self.slots[p].cursor.unwrap_or_default()
    }

    /// `mnPlayersVSCursorProcUpdate`.
    fn cursor_update(&mut self, p: usize, pad: &Pad, time_byte: &mut dyn FnMut() -> u8) {
        self.adjust_cursor(p, pad.state);
        let taps = pad.taps;
        if taps.contains(N64Buttons::A)
            && !self.check_player_kind_select_all(p, time_byte)
            && !self.select_fighter(p, BUTTON_A)
            && !self.check_cursor_puck_grab(p)
        {
            let c = self.cursor_pos(p);
            if time_arrow_r_in_range(c) {
                if self.rule == Rule::Time {
                    self.time_value = next_time_value(self.time_value);
                } else if self.stock_value + 1 > STOCK_MAX {
                    self.stock_value = 0;
                } else {
                    self.stock_value += 1;
                }
            } else if time_arrow_l_in_range(c) {
                if self.rule == Rule::Time {
                    self.time_value = prev_time_value(self.time_value);
                } else if self.stock_value == 0 {
                    self.stock_value = STOCK_MAX;
                } else {
                    self.stock_value -= 1;
                }
            } else if game_mode_in_range(c) {
                self.update_game_mode();
            } else if back_in_range(c) {
                self.back_to_vs_mode();
            } else if !self.check_team_select_all(p) {
                self.check_handicap_arrow_all(p);
            }
        }
        let c_buttons = [
            N64Buttons::C_UP,
            N64Buttons::C_RIGHT,
            N64Buttons::C_DOWN,
            N64Buttons::C_LEFT,
        ];
        if !self.is_team_battle {
            for (button, bit) in c_buttons.into_iter().enumerate() {
                if taps.contains(bit)
                    && !self.select_fighter(p, button)
                    && self.slots[p].is_fighter_selected
                {
                    self.update_costume(p, button);
                }
            }
        } else if c_buttons.into_iter().any(|bit| taps.contains(bit)) {
            self.select_fighter(p, BUTTON_A);
        }
        if taps.contains(N64Buttons::B) && self.is_man_fighter_selected(p) {
            self.recall_puck(p);
        }
        if !self.slots[p].is_recalling {
            self.detect_back(p, pad);
        }
        if !self.slots[p].is_recalling {
            self.update_cursor_no_recall(p);
        }
    }

    /// `mnPlayersVSUpdateGameMode`: free-for-all and team battle swap, and
    /// every shown fighter's costume follows.
    fn update_game_mode(&mut self) {
        self.is_team_battle = !self.is_team_battle;
        if self.is_team_battle {
            for s in self.slots.iter_mut().filter(|s| s.fkind.is_some()) {
                s.shade = 4;
            }
        }
        self.update_gate_all();
    }

    /// `mnPlayersVSUpdateGateAll`'s costume and shade updates.
    fn update_gate_all(&mut self) {
        for i in 0..PLAYERS {
            let Some(kind) = self.slots[i].fkind else {
                continue;
            };
            self.slots[i].costume = if self.is_team_battle {
                costume_team_id(kind, self.slots[i].team)
            } else {
                costume_common_id(kind, self.free_costume_royal(kind, i))
            };
            self.slots[i].shade = self.shade(i);
        }
    }

    /// `mnPlayersVSCheckTeamSelectInRangeAll`: a slot's team button cycles
    /// red, blue, green.
    fn check_team_select_all(&mut self, p: usize) -> bool {
        if !self.is_team_battle {
            return false;
        }
        let c = self.cursor_pos(p);
        for i in 0..PLAYERS {
            if self.slots[i].pkind != PlayerKind::Not && team_select_in_range(c, i) {
                let s = &mut self.slots[i];
                s.team = if s.team == TEAM_GREEN { 0 } else { s.team + 1 };
                if let Some(kind) = s.fkind {
                    s.costume = costume_team_id(kind, s.team);
                    self.slots[i].shade = self.shade(i);
                }
                return true;
            }
        }
        false
    }

    /// `mnPlayersVSCheckHandicapArrowInRangeAll`: a placed CPU's level, or
    /// with handicap on the cursor's own placed human's handicap, steps
    /// within 1 to 9.
    fn check_handicap_arrow_all(&mut self, p: usize) -> bool {
        let c = self.cursor_pos(p);
        let handicap_on = self.is_handicap_on();
        for i in 0..PLAYERS {
            let s = &mut self.slots[i];
            let eligible =
                s.pkind == PlayerKind::Com || (handicap_on && s.pkind == PlayerKind::Man && i == p);
            if !(eligible && s.is_fighter_selected) {
                continue;
            }
            let value = if s.pkind == PlayerKind::Man {
                &mut s.handicap
            } else {
                &mut s.cpu_level
            };
            if handicap_arrow_r_in_range(c, i) {
                if *value < VALUE_MAX {
                    *value += 1;
                }
                return true;
            }
            if handicap_arrow_l_in_range(c, i) {
                if *value > VALUE_MIN {
                    *value -= 1;
                }
                return true;
            }
        }
        false
    }

    /// `mnPlayersVSAdjustCursor`.
    fn adjust_cursor(&mut self, p: usize, input: ControllerState) {
        let s = &mut self.slots[p];
        let Some(cursor) = s.cursor.as_mut() else {
            return;
        };
        if s.is_cursor_adjusting {
            let step = |target: f32, at: &mut f32| {
                let delta = (target - *at) / 5.0;
                if (-1.0..=1.0).contains(&delta) {
                    *at = target;
                } else {
                    *at += delta;
                }
            };
            step(s.cursor_pickup.0, &mut cursor.0);
            step(s.cursor_pickup.1, &mut cursor.1);
            if *cursor == s.cursor_pickup {
                s.is_cursor_adjusting = false;
            }
        } else if !s.is_recalling {
            if !(-8..=8).contains(&input.stick_x) {
                let x = f32::from(input.stick_x) / 20.0 + cursor.0;
                if (0.0..=280.0).contains(&x) {
                    cursor.0 = x;
                }
            }
            if !(-8..=8).contains(&input.stick_y) {
                let y = f32::from(input.stick_y) / -20.0 + cursor.1;
                if (10.0..=205.0).contains(&y) {
                    cursor.1 = y;
                }
            }
        }
    }

    /// `mnPlayersVSCheckPuckInRange`.
    fn puck_in_range(&self, p: usize, slot: usize) -> bool {
        let (cx, cy) = self.cursor_pos(p);
        let (px, py) = self.slots[slot].puck;
        let x = cx + 25.0;
        let y = cy + 3.0;
        (px..=px + PUCK_WIDTH).contains(&x) && (py..=py + PUCK_HEIGHT).contains(&y)
    }

    /// `mnPlayersVSSelectFighter`. A held puck with no fighter is refused.
    fn select_fighter(&mut self, p: usize, button: usize) -> bool {
        if self.slots[p].cursor_status != CursorStatus::Grab {
            return false;
        }
        match self.slots[p].held {
            Some(held) if self.slots[held].fkind.is_some() => {
                self.select_fighter_puck(p, button);
                self.slots[p].recall_end_tic = self.total_tics + GRAB_WAIT;
                true
            }
            // `held_player` -1 with a grab cursor would index out of
            // bounds in the source; it reads as no fighter here.
            _ => false,
        }
    }

    /// `mnPlayersVSSelectFighterPuck`: `p`'s cursor places the puck it
    /// holds; a C button also picks the royal costume, unless another slot
    /// wears it.
    fn select_fighter_puck(&mut self, p: usize, button: usize) {
        let Some(held) = self.slots[p].held else {
            return;
        };
        if button != BUTTON_A {
            let Some(kind) = self.slots[held].fkind else {
                return;
            };
            let costume = costume_common_id(kind, button);
            if self.costume_used(kind, held, costume) {
                return;
            }
            self.slots[held].shade = self.shade(held);
            self.slots[held].costume = costume;
        }
        self.slots[held].is_selected = true;
        self.update_cursor_placement_priorities(Some(p), held);
        self.slots[held].holder = None;
        self.slots[p].cursor_status = CursorStatus::Hover;
        self.slots[p].held = None;
        self.slots[held].is_fighter_selected = true;
    }

    /// Moves a GObj in its display list (`gcMoveGObjDL`).
    fn move_dl(&mut self, link: u8, priority: u32) -> DrawKey {
        self.draw_key(link, priority)
    }

    fn move_cursor_dl(&mut self, p: usize, priority: u32) {
        if self.slots[p].cursor.is_some() {
            self.slots[p].cursor_draw = self.move_dl(DL_CURSOR, priority);
        }
    }

    /// `mnPlayersVSUpdateCursorGrabPriorities`: the grabbing cursor and
    /// its puck draw over the rest.
    fn update_cursor_grab_priorities(&mut self, p: usize, puck: usize) {
        let top = CURSOR_PRIORITIES[PLAYERS - 1];
        self.move_cursor_dl(p, top);
        self.slots[puck].puck_draw = self.move_dl(DL_CURSOR, top + 1);
        for i in 0..PLAYERS {
            let order = PLAYERS - 1 - i;
            if i != p {
                self.move_cursor_dl(i, CURSOR_PRIORITIES[order]);
                if let Some(held) = self.slots[i].held {
                    self.slots[held].puck_draw =
                        self.move_dl(DL_CURSOR, CURSOR_PRIORITIES[order] + 1);
                }
            }
        }
    }

    /// `mnPlayersVSUpdateCursorPlacementPriorities`: cursors that hold a
    /// puck first, then the placing cursor and its puck (back on the puck
    /// link), then the empty-handed cursors. `player` `None` is
    /// `GMCOMMON_PLAYERS_MAX`. Where the source's index runs below 0 it
    /// reads outside `unheld_priorities`; those moves are skipped.
    fn update_cursor_placement_priorities(&mut self, player: Option<usize>, puck: usize) {
        let is_held: [bool; PLAYERS] = core::array::from_fn(|i| self.slots[i].held.is_some());
        let mut id = PLAYERS as i32 - 1;
        let priority = |id: i32| usize::try_from(id).ok().map(|i| CURSOR_PRIORITIES[i]);
        for (i, &held) in is_held.iter().enumerate() {
            if Some(i) != player && held {
                if let Some(pr) = priority(id) {
                    self.move_cursor_dl(i, pr);
                    if let Some(h) = self.slots[i].held {
                        self.slots[h].puck_draw = self.move_dl(DL_CURSOR, pr + 1);
                    }
                }
                id -= 1;
            }
        }
        if let Some(pr) = priority(id) {
            if let Some(p) = player {
                self.move_cursor_dl(p, pr);
            }
            self.slots[puck].puck_draw = self.move_dl(DL_PUCK, pr + 1);
        }
        id -= 1;
        for (i, &held) in is_held.iter().enumerate() {
            if Some(i) != player && !held {
                if let Some(pr) = priority(id) {
                    self.move_cursor_dl(i, pr);
                }
                id -= 1;
            }
        }
    }

    /// `mnPlayersVSSetCursorGrab`.
    fn set_cursor_grab(&mut self, p: usize, held: usize) {
        self.slots[held].holder = Some(p);
        self.slots[held].is_selected = false;
        self.slots[p].cursor_status = CursorStatus::Grab;
        self.slots[p].held = Some(held);
        self.slots[held].is_fighter_selected = false;
        self.update_fighter(held);
        self.update_cursor_grab_priorities(p, held);
        // `mnPlayersVSSetCursorPuckOffset`.
        let (px, py) = self.slots[held].puck;
        self.slots[p].cursor_pickup = (px - 11.0, py - -14.0);
        self.slots[p].is_cursor_adjusting = true;
    }

    /// `mnPlayersVSCheckCursorPuckGrab`: pucks are tried from slot 4 down;
    /// a cursor takes its own puck (unless NA) or any CPU's.
    fn check_cursor_puck_grab(&mut self, p: usize) -> bool {
        let s = &self.slots[p];
        if self.total_tics < s.recall_end_tic
            || s.is_recalling
            || s.cursor_status != CursorStatus::Hover
        {
            return false;
        }
        for i in (0..PLAYERS).rev() {
            let t = &self.slots[i];
            let kind_ok = if i == p {
                t.pkind != PlayerKind::Not
            } else {
                t.pkind == PlayerKind::Com
            };
            if t.holder.is_none() && kind_ok && self.puck_in_range(p, i) {
                self.set_cursor_grab(p, i);
                return true;
            }
        }
        false
    }

    /// `mnPlayersVSUpdateCursorNoRecall`.
    fn update_cursor_no_recall(&mut self, p: usize) {
        let s = &self.slots[p];
        let (_, y) = self.cursor_pos(p);
        let status = if y > 124.0 || y < 38.0 {
            CursorStatus::Pointer
        } else if s.held.is_none() {
            CursorStatus::Hover
        } else {
            CursorStatus::Grab
        };
        let hover_placed = status == CursorStatus::Pointer
            && s.is_selected
            && (0..PLAYERS).any(|i| self.slots[i].is_selected && self.puck_in_range(p, i));
        self.slots[p].cursor_status = if hover_placed {
            CursorStatus::Hover
        } else {
            status
        };
    }

    /// `mnPlayersVSUpdateCostume`.
    fn update_costume(&mut self, p: usize, button: usize) {
        let Some(kind) = self.slots[p].fkind else {
            return;
        };
        let costume = costume_common_id(kind, button);
        if !self.costume_used(kind, p, costume) {
            self.slots[p].costume = costume;
            self.slots[p].shade = self.shade(p);
        }
    }

    /// `mnPlayersVSCheckManFighterSelected`.
    fn is_man_fighter_selected(&self, p: usize) -> bool {
        let s = &self.slots[p];
        s.is_selected && s.held.is_none() && s.pkind == PlayerKind::Man
    }

    /// `mnPlayersVSRecallPuck`.
    fn recall_puck(&mut self, p: usize) {
        let cursor = self.cursor_pos(p);
        let s = &mut self.slots[p];
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

    /// `mnPlayersVSDetectBack`: B tapped and then held until the 40th tick
    /// of the hold goes back to VS mode.
    fn detect_back(&mut self, p: usize, pad: &Pad) {
        let s = &mut self.slots[p];
        if s.is_hold_b {
            if s.hold_b_tics != 0 {
                s.hold_b_tics += 1;
                if s.hold_b_tics <= HOLD_B_TICS {
                    if pad.state.buttons.contains(N64Buttons::B) {
                        if s.hold_b_tics == HOLD_B_TICS {
                            self.back_to_vs_mode();
                        }
                    } else {
                        s.is_hold_b = false;
                        s.hold_b_tics = 0;
                    }
                }
            }
        } else {
            if pad.taps.contains(N64Buttons::B) {
                s.is_hold_b = true;
            }
            s.hold_b_tics = 1;
        }
    }

    /// `mnPlayersVSPuckProcUpdate`.
    fn puck_update(&mut self, p: usize, time_byte: &mut dyn FnMut() -> u8) {
        match self.slots[p].holder {
            Some(holder) if !self.slots[p].is_selected => {
                let h = &self.slots[holder];
                if !h.is_cursor_adjusting {
                    if let Some((cx, cy)) = h.cursor {
                        self.slots[p].puck = (cx + 11.0, cy + -14.0);
                    }
                }
            }
            // `mnPlayersVSMovePuck`.
            _ => {
                let s = &mut self.slots[p];
                s.puck.0 += s.puck_vel.0;
                s.puck.1 += s.puck_vel.1;
            }
        }
        let kind = puck_fighter_kind(self.slots[p].puck, self.fighter_mask);
        if self.slots[p].pkind == PlayerKind::Not {
            // A connected NA slot whose (hidden) puck reaches a portrait
            // becomes a human.
            if !(self.connected[p] && kind.is_some()) {
                return;
            }
            self.slots[p].pkind = PlayerKind::Man;
            self.update_player_kind(p, time_byte);
            self.update_puck_display(p);
        }
        let s = &self.slots[p];
        if s.pkind == PlayerKind::Com && kind != s.fkind && kind.is_none() {
            // A CPU puck carried off the grid is dropped at once.
            if let Some(holder) = s.holder {
                self.select_fighter_puck(holder, BUTTON_A);
            }
        }
        if !self.slots[p].is_selected && kind != self.slots[p].fkind {
            self.slots[p].fkind = kind;
            self.update_fighter(p);
        }
    }

    /// `mnPlayersVSPuckAdjustProcUpdate`.
    fn puck_adjust(&mut self) {
        for p in 0..PLAYERS {
            if self.slots[p].is_recalling {
                self.adjust_recall(p);
            }
            if self.slots[p].is_selected {
                self.adjust_placed(p);
            }
        }
    }

    /// `mnPlayersVSPuckAdjustRecall`.
    fn adjust_recall(&mut self, p: usize) {
        let s = &mut self.slots[p];
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
            self.set_cursor_grab(p, p);
            self.slots[p].puck_vel = (0.0, 0.0);
        }
        if self.slots[p].recall_tics == 30 {
            self.slots[p].is_recalling = false;
        }
    }

    /// `mnPlayersVSPuckAdjustPlaced`, `mnPlayersVSGetPuckDistance`,
    /// `mnPlayersVSPuckAdjustOverlap` and `PuckAdjustPortraitEdge`:
    /// placed pucks of the same fighter push apart and stay inside the
    /// portrait.
    fn adjust_placed(&mut self, p: usize) {
        let (px, py) = self.slots[p].puck;
        self.slots[p].puck_vel = (0.0, 0.0);
        let kind = self.slots[p].fkind;
        for i in 0..PLAYERS {
            if i == p {
                continue;
            }
            let (ox, oy) = self.slots[i].puck;
            let dist = ssb_engine::math::sqrt((ox - px) * (ox - px) + (oy - py) * (oy - py));
            if (0.0..=15.0).contains(&dist)
                && kind == self.slots[i].fkind
                && kind.is_some()
                && self.slots[i].is_selected
            {
                let push = |at: f32, from: f32| {
                    if at == from {
                        (crate::rng::rand_int_range(2) - 1) as f32
                    } else {
                        -(from - at) / 10.0
                    }
                };
                let vx = push(px, ox);
                let vy = push(py, oy);
                self.slots[p].puck_vel.0 += vx;
                self.slots[p].puck_vel.1 += vy;
            }
        }
        // A placed puck with no fighter would index `mnPlayersVSGetPortrait`
        // out of bounds in the source; it is left where it is.
        if let Some(kind) = kind {
            let s = &mut self.slots[p];
            s.puck_vel = portrait_edge_velocity(kind, s.puck, s.puck_vel);
        }
    }

    /// `mnPlayersVSCostumeSyncProcUpdate`: in a team battle the shades are
    /// kept current; in free-for-all a fighter only one slot shows goes
    /// back to its first costume until it is placed.
    fn costume_sync(&mut self) {
        for i in 0..PLAYERS {
            let Some(kind) = self.slots[i].fkind else {
                continue;
            };
            if self.is_team_battle {
                self.slots[i].shade = self.shade(i);
            } else if self.fighter_kind_count(kind) == 0 {
                let first = costume_common_id(kind, 0);
                if self.slots[i].costume != first && !self.slots[i].is_fighter_selected {
                    self.slots[i].shade = self.shade(i);
                    self.slots[i].costume = first;
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "players_vs_tests.rs"]
mod tests;
