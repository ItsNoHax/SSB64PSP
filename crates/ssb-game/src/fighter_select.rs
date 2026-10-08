//! The Training character select — `mn/mnplayers/mnplayers1ptraining.c`:
//! the hand cursor, the two pucks (the player's and the CPU's), the portrait
//! grid, the recall, the costume picks and syncs, the ready check and the
//! scene data it saves. [`layer`] holds its presentation; the
//! spotlight stays with the host.
//!
//! One [`FighterSelect::tick`] runs the scene's processes in the original
//! order: `mnPlayers1PTrainingFuncRun` (the scene GObj's `func_run`), then
//! the cursor (process priority 2), then the player's and the CPU's puck,
//! the puck adjust and the costume sync (priority 1, by link: 20, 26, 31).
//!
//! `gSCManagerSceneData.player` is 0 until a 1P game changes it, so the
//! player is slot 0 and the CPU slot 1. The other two slots are
//! `nFTPlayerKindNot` and take part in nothing.

use ssb_engine::input::{ControllerState, N64Buttons};

use crate::costume::costume_common_id;
use crate::fighter::FighterKind;
use crate::sound::{self, id, FgmHandle};

/// The player's slot (`sMNPlayers1PTrainingManPlayer`).
pub const MAN: usize = 0;
/// The CPU's slot (`sMNPlayers1PTrainingComPlayer`).
pub const COM: usize = 1;

/// `mnPlayers1PTrainingGetFighterKind`: the portraits, top row then bottom.
pub const PORTRAIT_KINDS: [FighterKind; 12] = [
    FighterKind::Luigi,
    FighterKind::Mario,
    FighterKind::Donkey,
    FighterKind::Link,
    FighterKind::Samus,
    FighterKind::Captain,
    FighterKind::Ness,
    FighterKind::Yoshi,
    FighterKind::Kirby,
    FighterKind::Fox,
    FighterKind::Pikachu,
    FighterKind::Purin,
];

/// `mnPlayers1PTrainingGetPortrait`, indexed by `nFTKind`.
const KIND_PORTRAITS: [usize; 12] = [1, 9, 2, 4, 0, 3, 7, 5, 8, 10, 11, 6];

/// `mnPlayers1PTrainingGetPortrait`.
pub fn portrait(kind: FighterKind) -> usize {
    KIND_PORTRAITS[(kind as usize).min(11)]
}

/// Portrait cell size and the grid's top-left corner, in N64 screen pixels.
pub const PORTRAIT_WIDTH: f32 = 45.0;
pub const PORTRAIT_HEIGHT: f32 = 43.0;
pub const PORTRAIT_LEFT: f32 = 25.0;
pub const PORTRAIT_TOP: f32 = 36.0;
/// The puck sprite's hit box (`mnPlayers1PTrainingCheckPuckInRange`).
pub const PUCK_WIDTH: f32 = 26.0;
pub const PUCK_HEIGHT: f32 = 24.0;

/// `I_MIN_TO_TICS(5)`.
const RETURN_TICS: i32 = 5 * 60 * 60;
/// START is read only after this many ticks.
const START_TICS: i32 = 60;
/// Ticks between a ready START and the stage select.
const START_PROCEED_WAIT: i32 = 30;
/// Ticks after a placement before the cursor can grab again.
const GRAB_WAIT: i32 = 30;
/// B returns to the 1P menu from this tick.
const BACK_TICS: i32 = 10;
/// The pucks are hidden before this tick.
const PUCK_SHOW_TICS: i32 = 30;

/// The select button a pick came from: `nMNPlayersSelectButton*`.
const BUTTON_A: usize = 4;

/// `nFTPlayerKind`: the two kinds a Training slot takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlayerKind {
    Man,
    Com,
}

/// `nMNPlayersCursorStatus`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum CursorStatus {
    #[default]
    Pointer,
    Grab,
    Hover,
}

/// `gSCManagerSceneData`'s Training fields. `None` is `nFTKindNull`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct SceneData {
    pub man_kind: Option<FighterKind>,
    pub man_costume: u8,
    pub com_kind: Option<FighterKind>,
    pub com_costume: u8,
}

/// What a tick asks the host to do. Each carries the scene data saved on
/// the way out (`mnPlayers1PTrainingSetSceneData`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// The ready START's wait ran out: on to the stage select.
    Proceed(SceneData),
    /// B, or A on the back button: back to the 1P menu.
    Back(SceneData),
    /// Five idle minutes: back to the title.
    Timeout(SceneData),
}

/// One `MNPlayersSlotTraining`, the fields the logic reads.
#[derive(Clone, Debug)]
pub struct Slot {
    pub player_kind: PlayerKind,
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
    /// `holder_player`; `None` is `GMCOMMON_PLAYERS_MAX`.
    pub holder: Option<usize>,
    /// `held_player`; `None` is -1.
    pub held: Option<usize>,
    /// The puck sprite's top-left corner.
    pub puck: (f32, f32),
    puck_vel: (f32, f32),
    /// The cursor sprite's top-left corner (the player's slot only).
    pub cursor: (f32, f32),
    pub cursor_status: CursorStatus,
    cursor_pickup: (f32, f32),
    is_cursor_adjusting: bool,
}

impl Slot {
    fn blank(player_kind: PlayerKind) -> Slot {
        Slot {
            player_kind,
            kind: None,
            costume: 0,
            is_selected: false,
            is_fighter_selected: false,
            is_recalling: false,
            recall_end_tic: 0,
            recall_start: (0.0, 0.0),
            recall_end_x: 0.0,
            recall_mid_y: 0.0,
            recall_end_y: 0.0,
            recall_tics: 0,
            holder: None,
            held: None,
            puck: (0.0, 0.0),
            puck_vel: (0.0, 0.0),
            cursor: (0.0, 0.0),
            cursor_status: CursorStatus::Pointer,
            cursor_pickup: (0.0, 0.0),
            is_cursor_adjusting: false,
        }
    }
}

/// `mnPlayers1PTrainingCheckFighterLocked`: the four unlockable fighters
/// need their `fighter_mask` bit.
pub fn is_locked(kind: FighterKind, fighter_mask: u16) -> bool {
    FighterKind::UNLOCKABLE.contains(&kind) && fighter_mask & (1 << kind as u16) == 0
}

/// The Training character select's state.
#[derive(Clone, Debug)]
pub struct FighterSelect {
    pub slots: [Slot; 2],
    /// The presentation's GObjs.
    pub view: layer::View,
    fighter_mask: u16,
    total_tics: i32,
    return_tic: i32,
    is_start: bool,
    start_proceed_wait: i32,
    pending: Option<Outcome>,
    /// The player's cursor's `p_sfx`: its last fighter-name voice.
    p_sfx: Option<FgmHandle>,
}

impl FighterSelect {
    /// `sMNPlayers1PTrainingFighterMask`: the backup's `fighter_mask`.
    pub fn fighter_mask(&self) -> u16 {
        self.fighter_mask
    }

    /// `mnPlayers1PTrainingFuncStart`: `mnPlayers1PTrainingInitVars`,
    /// `mnPlayers1PTrainingInitSlotAll` and the scene's audio. `from_maps`
    /// is `scene_prev == nSCKindMaps` (the select's BGM plays on).
    /// `time_byte` stands for `osGetTime() & 0xFF`, read once per draw of
    /// the CPU's random fighter.
    pub fn new(
        scene: SceneData,
        fighter_mask: u16,
        from_maps: bool,
        mut time_byte: impl FnMut() -> u8,
    ) -> Self {
        let mut man = Slot::blank(PlayerKind::Man);
        match scene.man_kind {
            // `mnPlayers1PTrainingResetPlayer`: the cursor holds its own
            // puck.
            None => {
                man.holder = Some(MAN);
                man.held = Some(MAN);
            }
            Some(kind) => {
                man.kind = Some(kind);
                man.costume = scene.man_costume;
                man.is_fighter_selected = true;
                man.is_selected = true;
            }
        }
        let mut com = Slot::blank(PlayerKind::Com);
        let (kind, costume) = match scene.com_kind {
            Some(kind) => (kind, scene.com_costume),
            None => {
                // `syUtilsRandTimeUCharRange(nFTKindPlayableEnd + 1)` until
                // the draw is unlocked.
                let kind = loop {
                    let k = FighterKind::PLAYABLE[(i32::from(time_byte()) * 12 / 256) as usize];
                    if !is_locked(k, fighter_mask) {
                        break k;
                    }
                };
                let first = costume_common_id(kind, 0);
                let taken = man.kind == Some(kind) && man.costume == first;
                (
                    kind,
                    if taken {
                        costume_common_id(kind, 1)
                    } else {
                        first
                    },
                )
            }
        };
        com.kind = Some(kind);
        com.costume = costume;
        com.is_fighter_selected = true;
        com.is_selected = true;
        let mut select = FighterSelect {
            slots: [man, com],
            view: layer::View::default(),
            fighter_mask,
            total_tics: 0,
            return_tic: RETURN_TICS,
            is_start: false,
            start_proceed_wait: 0,
            pending: None,
            p_sfx: None,
        };
        // `mnPlayers1PTrainingMakeCursor` and `MakePuck`.
        select.slots[MAN].cursor = (70.0, 170.0);
        for slot in 0..2 {
            select.slots[slot].puck = match select.slots[slot].kind {
                None => (51.0, 161.0),
                Some(kind) => portrait_center(kind),
            };
        }
        select.v_init();
        if !from_maps {
            sound::play_bgm(0, id::nSYAudioBGMBattleSelect);
        }
        sound::stop_all_fgm();
        sound::play_fgm(id::nSYAudioVoiceAnnounceTrainingMode);
        select
    }

    /// `mnPlayers1PTrainingCheckReady`.
    pub fn is_ready(&self) -> bool {
        self.slots[MAN].is_fighter_selected && self.slots[COM].is_fighter_selected
    }

    /// Whether `slot`'s puck draws this tick (`mnPlayers1PTrainingPuckProcUpdate`).
    pub fn puck_visible(&self, slot: usize) -> bool {
        let s = &self.slots[slot];
        self.total_tics >= PUCK_SHOW_TICS
            && (s.player_kind == PlayerKind::Com
                || s.cursor_status != CursorStatus::Pointer
                || s.is_selected
                || s.is_recalling)
    }

    fn scene_data(&self) -> SceneData {
        SceneData {
            man_kind: self.slots[MAN].kind,
            man_costume: self.slots[MAN].costume,
            com_kind: self.slots[COM].kind,
            com_costume: self.slots[COM].costume,
        }
    }

    /// Records the first scene change of this tick with the scene data at
    /// that moment, as `syTaskmanSetLoadScene` takes effect at frame end.
    fn leave(&mut self, outcome: fn(SceneData) -> Outcome) {
        if self.pending.is_none() {
            self.pending = Some(outcome(self.scene_data()));
        }
    }

    /// One frame. `input` is the player's controller; `taps` its newly
    /// pressed buttons.
    pub fn tick(&mut self, input: ControllerState, taps: N64Buttons) -> Option<Outcome> {
        self.pending = None;
        let paused = self.func_run(input, taps);
        if !paused {
            self.cursor_update(input, taps);
            self.puck_update(MAN);
            self.puck_update(COM);
        }
        self.puck_adjust();
        self.costume_sync();
        self.view_tick();
        self.pending
    }

    /// `mnPlayers1PTrainingFuncRun`. Returns whether the slot processes are
    /// paused.
    fn func_run(&mut self, input: ControllerState, taps: N64Buttons) -> bool {
        self.total_tics += 1;
        if self.total_tics == self.return_tic {
            self.leave(Outcome::Timeout);
            return self.is_start;
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
        } else if taps.contains(N64Buttons::START) && self.total_tics > START_TICS {
            if self.is_ready() {
                sound::play_fgm(id::nSYAudioVoicePublicCheer);
                self.start_proceed_wait = START_PROCEED_WAIT;
                self.is_start = true;
            } else {
                sound::play_fgm(id::nSYAudioFGMMenuDenied);
            }
        }
        self.is_start
    }

    /// `mnPlayers1PTrainingCursorProcUpdate`.
    fn cursor_update(&mut self, input: ControllerState, taps: N64Buttons) {
        self.adjust_cursor(input);
        if taps.contains(N64Buttons::A)
            && !self.select_fighter(BUTTON_A)
            && !self.check_cursor_puck_grab()
            && self.back_in_range()
        {
            self.back_to_1p_mode();
            sound::play_fgm(id::nSYAudioFGMMenuScroll2);
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
            if taps.contains(bit)
                && !self.select_fighter(button)
                && self.slots[MAN].is_fighter_selected
            {
                self.update_costume(MAN, button);
            }
        }
        let man = &self.slots[MAN];
        if taps.contains(N64Buttons::B)
            && man.is_selected
            && man.held.is_none()
            && man.player_kind == PlayerKind::Man
        {
            self.recall_puck(MAN);
        }
        if !self.slots[MAN].is_recalling
            && self.total_tics >= BACK_TICS
            && taps.contains(N64Buttons::B)
        {
            self.back_to_1p_mode();
        }
        if !self.slots[MAN].is_recalling {
            self.update_cursor_no_recall();
        }
    }

    /// `mnPlayers1PTrainingBackTo1PMode`.
    fn back_to_1p_mode(&mut self) {
        self.leave(Outcome::Back);
        sound::stop_bgm_all();
        sound::stop_all_fgm();
    }

    /// `mnPlayers1PTrainingAdjustCursor`.
    fn adjust_cursor(&mut self, input: ControllerState) {
        let s = &mut self.slots[MAN];
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

    /// `mnPlayers1PTrainingCheckPuckInRange`.
    fn puck_in_range(&self, slot: usize) -> bool {
        let (cx, cy) = self.slots[MAN].cursor;
        let (px, py) = self.slots[slot].puck;
        let x = cx + 25.0;
        let y = cy + 3.0;
        (px..=px + PUCK_WIDTH).contains(&x) && (py..=py + PUCK_HEIGHT).contains(&y)
    }

    /// `mnPlayers1PTrainingSelectFighter`.
    fn select_fighter(&mut self, button: usize) -> bool {
        if self.slots[MAN].cursor_status != CursorStatus::Grab {
            return false;
        }
        match self.slots[MAN].held {
            Some(held) if self.slots[held].kind.is_some() => {
                self.select_fighter_puck(MAN, button);
                self.slots[MAN].recall_end_tic = self.total_tics + GRAB_WAIT;
                true
            }
            _ => {
                sound::play_fgm(id::nSYAudioFGMMenuDenied);
                false
            }
        }
    }

    /// The other slot's fighter and costume.
    fn other(&self, slot: usize) -> (Option<FighterKind>, u8) {
        let o = &self.slots[1 - slot];
        (o.kind, o.costume)
    }

    /// `mnPlayers1PTrainingCheckCostumeUsed`.
    fn costume_used(&self, kind: FighterKind, slot: usize, costume: u8) -> bool {
        self.other(slot) == (Some(kind), costume)
    }

    /// `mnPlayers1PTrainingGetFreeCostume`.
    fn free_costume(&self, kind: FighterKind, slot: usize) -> u8 {
        let royal = (0..4)
            .find(|&i| !self.costume_used(kind, slot, costume_common_id(kind, i)))
            .unwrap_or(0);
        costume_common_id(kind, royal)
    }

    /// `mnPlayers1PTrainingSelectFighterPuck`: `slot`'s cursor places the
    /// puck it holds.
    fn select_fighter_puck(&mut self, slot: usize, button: usize) {
        let Some(held) = self.slots[slot].held else {
            return;
        };
        if button != BUTTON_A {
            let Some(kind) = self.slots[held].kind else {
                return;
            };
            let costume = costume_common_id(kind, button);
            if self.costume_used(kind, held, costume) {
                sound::play_fgm(id::nSYAudioFGMMenuDenied);
                return;
            }
            self.slots[held].costume = costume;
        }
        self.slots[held].is_selected = true;
        self.v_placement_priorities(held);
        self.slots[held].holder = None;
        self.slots[slot].cursor_status = CursorStatus::Hover;
        self.slots[slot].held = None;
        self.slots[held].is_fighter_selected = true;
        // `mnPlayers1PTrainingAnnounceFighter`: only the player's cursor
        // places pucks, so its `p_sfx` is the one.
        if let Some(kind) = self.slots[held].kind {
            self.p_sfx = announce_fighter(self.p_sfx, kind);
        }
        self.v_make_portrait_flash(held);
    }

    /// `mnPlayers1PTrainingUpdateFighter`: the fighter model is remade with
    /// the first free costume.
    fn update_fighter(&mut self, slot: usize) {
        if let Some(kind) = self.slots[slot].kind {
            self.slots[slot].costume = self.free_costume(kind, slot);
        }
        self.v_update_fighter(slot);
    }

    /// `mnPlayers1PTrainingSetCursorGrab`.
    fn set_cursor_grab(&mut self, slot: usize, held: usize) {
        self.slots[held].holder = Some(slot);
        self.slots[held].is_selected = false;
        self.slots[slot].cursor_status = CursorStatus::Grab;
        self.slots[slot].held = Some(held);
        self.slots[held].is_fighter_selected = false;
        self.update_fighter(held);
        self.v_grab_priorities(held);
        // `mnPlayers1PTrainingSetCursorPuckOffset`.
        let (px, py) = self.slots[held].puck;
        self.slots[slot].cursor_pickup = (px - 11.0, py - -14.0);
        self.slots[slot].is_cursor_adjusting = true;
        sound::play_fgm(id::nSYAudioFGMSamusDash);
        self.v_destroy_portrait_flash(held);
        self.v_update_name_and_emblem(held);
    }

    /// `mnPlayers1PTrainingCheckCursorPuckGrab`: the CPU's puck is tried
    /// before the player's own.
    fn check_cursor_puck_grab(&mut self) -> bool {
        let man = &self.slots[MAN];
        if self.total_tics < man.recall_end_tic
            || man.is_recalling
            || man.cursor_status != CursorStatus::Hover
        {
            return false;
        }
        for slot in [COM, MAN] {
            if self.slots[slot].holder.is_none() && self.puck_in_range(slot) {
                self.set_cursor_grab(MAN, slot);
                return true;
            }
        }
        false
    }

    /// `mnPlayers1PTrainingBackInRange`: the back button's box.
    fn back_in_range(&self) -> bool {
        let (cx, cy) = self.slots[MAN].cursor;
        let y = cy + 3.0;
        if !(13.0..=34.0).contains(&y) {
            return false;
        }
        (244.0..=292.0).contains(&(cx + 20.0))
    }

    /// `mnPlayers1PTrainingUpdateCostume`.
    fn update_costume(&mut self, slot: usize, button: usize) {
        let Some(kind) = self.slots[slot].kind else {
            return;
        };
        let costume = costume_common_id(kind, button);
        if self.costume_used(kind, slot, costume) {
            sound::play_fgm(id::nSYAudioFGMMenuDenied);
        } else {
            self.slots[slot].costume = costume;
            sound::play_fgm(id::nSYAudioFGMMenuScroll2);
        }
    }

    /// `mnPlayers1PTrainingRecallPuck`.
    fn recall_puck(&mut self, slot: usize) {
        let cursor = self.slots[MAN].cursor;
        let s = &mut self.slots[slot];
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

    /// `mnPlayers1PTrainingUpdateCursorNoRecall`.
    fn update_cursor_no_recall(&mut self) {
        let s = &self.slots[MAN];
        let status = if s.cursor.1 > 124.0 || s.cursor.1 < 38.0 {
            CursorStatus::Pointer
        } else if s.held.is_none() {
            CursorStatus::Hover
        } else {
            CursorStatus::Grab
        };
        let hover_placed = status == CursorStatus::Pointer
            && s.is_selected
            && (0..2).any(|i| self.slots[i].is_selected && self.puck_in_range(i));
        self.slots[MAN].cursor_status = if hover_placed {
            CursorStatus::Hover
        } else {
            status
        };
    }

    /// `mnPlayers1PTrainingGetPuckFighterKind`.
    fn puck_fighter_kind(&self, slot: usize) -> Option<FighterKind> {
        puck_fighter_kind(self.slots[slot].puck, self.fighter_mask)
    }

    /// `mnPlayers1PTrainingPuckProcUpdate`.
    fn puck_update(&mut self, slot: usize) {
        self.view.slots[slot].puck_shown = self.puck_visible(slot);
        match self.slots[slot].holder {
            Some(holder) if !self.slots[slot].is_selected => {
                if !self.slots[holder].is_cursor_adjusting {
                    let (cx, cy) = self.slots[holder].cursor;
                    self.slots[slot].puck = (cx + 11.0, cy + -14.0);
                }
            }
            // `mnPlayers1PTrainingMovePuck`.
            _ => {
                let s = &mut self.slots[slot];
                s.puck.0 += s.puck_vel.0;
                s.puck.1 += s.puck_vel.1;
            }
        }
        let kind = self.puck_fighter_kind(slot);
        let s = &self.slots[slot];
        if s.player_kind == PlayerKind::Com && kind != s.kind && kind.is_none() {
            if let Some(holder) = s.holder {
                self.select_fighter_puck(holder, BUTTON_A);
            }
        }
        if !self.slots[slot].is_selected && kind != self.slots[slot].kind {
            self.slots[slot].kind = kind;
            self.update_fighter(slot);
            self.v_update_name_and_emblem(slot);
        }
    }

    /// `mnPlayers1PTrainingPuckAdjustProcUpdate`.
    fn puck_adjust(&mut self) {
        for slot in [MAN, COM] {
            if self.slots[slot].is_recalling {
                self.adjust_recall(slot);
            }
            if self.slots[slot].is_selected {
                self.adjust_placed(slot);
            }
        }
    }

    /// `mnPlayers1PTrainingPuckAdjustRecall`.
    fn adjust_recall(&mut self, slot: usize) {
        let s = &mut self.slots[slot];
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
            self.set_cursor_grab(slot, slot);
            self.slots[slot].puck_vel = (0.0, 0.0);
        }
        if self.slots[slot].recall_tics == 30 {
            self.slots[slot].is_recalling = false;
        }
    }

    /// `mnPlayers1PTrainingPuckAdjustPlaced`, `PuckAdjustOverlap` and
    /// `PuckAdjustPortraitEdge`.
    fn adjust_placed(&mut self, slot: usize) {
        let other = 1 - slot;
        let (px, py) = self.slots[slot].puck;
        let (ox, oy) = self.slots[other].puck;
        let dist = ssb_engine::math::sqrt((ox - px) * (ox - px) + (oy - py) * (oy - py));
        self.slots[slot].puck_vel = (0.0, 0.0);
        let kind = self.slots[slot].kind;
        if (0.0..=15.0).contains(&dist)
            && kind == self.slots[other].kind
            && kind.is_some()
            && self.slots[other].is_selected
        {
            let push = |at: f32, from: f32| {
                if at == from {
                    (crate::rng::rand_int_range(2) - 1) as f32
                } else {
                    -(from - at) / 10.0
                }
            };
            self.slots[slot].puck_vel.0 += push(px, ox);
            self.slots[slot].puck_vel.1 += push(py, oy);
        }
        let Some(kind) = kind else {
            return;
        };
        let s = &mut self.slots[slot];
        s.puck_vel = portrait_edge_velocity(kind, s.puck, s.puck_vel);
    }

    /// `mnPlayers1PTrainingCostumeSyncProcUpdate`: a fighter only one slot
    /// shows goes back to its first costume until it is placed.
    fn costume_sync(&mut self) {
        for slot in 0..2 {
            let Some(kind) = self.slots[slot].kind else {
                continue;
            };
            if self.slots[1 - slot].kind == Some(kind) {
                continue;
            }
            let first = costume_common_id(kind, 0);
            let s = &mut self.slots[slot];
            if s.costume != first && !s.is_fighter_selected {
                s.costume = first;
            }
        }
    }
}

/// `mnPlayers1PTrainingGetPuckFighterKind` (and the VS screen's
/// `mnPlayersVSGetPuckFighterKind`): the unlocked fighter whose portrait
/// holds the centre of a puck whose top-left corner is at `puck`.
pub(crate) fn puck_fighter_kind(puck: (f32, f32), fighter_mask: u16) -> Option<FighterKind> {
    let (px, py) = puck;
    let x = px as i32 + 13;
    let y = py as i32 + 12;
    let row = if y > 35 && y < 79 {
        0
    } else if y > 78 && y < 122 {
        6
    } else {
        return None;
    };
    if !(x > 24 && x < 295) {
        return None;
    }
    let kind = PORTRAIT_KINDS[((x - 25) / 45) as usize + row];
    (!is_locked(kind, fighter_mask)).then_some(kind)
}

/// `mnPlayers1PTrainingPuckAdjustPortraitEdge` (and
/// `mnPlayersVSPuckAdjustPortraitEdge`): the velocity that keeps a placed
/// puck's centre 5 pixels inside `kind`'s portrait.
pub(crate) fn portrait_edge_velocity(
    kind: FighterKind,
    puck: (f32, f32),
    vel: (f32, f32),
) -> (f32, f32) {
    let p = portrait(kind);
    let edge_x = (p % 6) as f32 * PORTRAIT_WIDTH + PORTRAIT_LEFT;
    let edge_y = (p / 6) as f32 * PORTRAIT_HEIGHT + PORTRAIT_TOP;
    let mut vel = vel;
    let x = puck.0 + vel.0 + 13.0;
    let y = puck.1 + vel.1 + 12.0;
    if x < edge_x + 5.0 {
        vel.0 = ((edge_x + 5.0) - x) / 10.0;
    }
    if (edge_x + 45.0) - 5.0 < x {
        vel.0 = -(x - ((edge_x + 45.0) - 5.0)) / 10.0;
    }
    if y < edge_y + 5.0 {
        vel.1 = ((edge_y + 5.0) - y) / 10.0;
    }
    if (edge_y + 43.0) - 5.0 < y {
        vel.1 = -(y - ((edge_y + 43.0) - 5.0)) / 10.0;
    }
    vel
}

/// `mnPlayers1PTrainingCenterPuckInPortrait`.
pub fn portrait_center(kind: FighterKind) -> (f32, f32) {
    let p = portrait(kind);
    if p >= 6 {
        ((p * 45 - 6 * 45 + 36) as f32, 89.0)
    } else {
        ((p * 45 + 36) as f32, 46.0)
    }
}

/// `announce_names` in `mnPlayers*AnnounceFighter`: each playable
/// fighter's name voice, in [`FighterKind`] order.
pub const ANNOUNCE_NAMES: [u16; 12] = [
    id::nSYAudioVoiceAnnounceMario,
    id::nSYAudioVoiceAnnounceFox,
    id::nSYAudioVoiceAnnounceDonkey,
    id::nSYAudioVoiceAnnounceSamus,
    id::nSYAudioVoiceAnnounceLuigi,
    id::nSYAudioVoiceAnnounceLink,
    id::nSYAudioVoiceAnnounceYoshi,
    id::nSYAudioVoiceAnnounceCaptain,
    id::nSYAudioVoiceAnnounceKirby,
    id::nSYAudioVoiceAnnouncePikachu,
    id::nSYAudioVoiceAnnouncePurin,
    id::nSYAudioVoiceAnnounceNess,
];

/// `mnPlayers*AnnounceFighter`: stops the cursor's last name voice
/// (`func_80026738_27338(p_sfx)`), plays `nSYAudioFGMMarioDash` and
/// `kind`'s name, and returns the new `p_sfx`. A `NULL` `p_sfx` stops
/// nothing here.
pub fn announce_fighter(p_sfx: Option<FgmHandle>, kind: FighterKind) -> Option<FgmHandle> {
    if let Some(h) = p_sfx {
        sound::stop_fgm(h);
    }
    sound::play_fgm(id::nSYAudioFGMMarioDash);
    ANNOUNCE_NAMES
        .get(kind as usize)
        .and_then(|&v| sound::play_fgm(v))
}

#[path = "fighter_select_layer.rs"]
pub mod layer;

#[cfg(test)]
#[path = "fighter_select_tests.rs"]
mod tests;
