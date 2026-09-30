//! The Training character select's presentation, from
//! `mn/mnplayers/mnplayers1ptraining.c`: the wallpaper and labels
//! (`mnPlayers1PTrainingMakeWallpaper`, `mnPlayers1PTrainingMakeLabels`),
//! the portraits and their slide-in, the portrait flash, the two panels
//! (`mnPlayers1PTrainingMakeGate`: the card, the "1P"/"CP" text, the name
//! and emblem), the fighters' turn and demo status, the pucks with their
//! glow, the cursor and the "ready to fight" banner with "Press Start".
//!
//! The source shares `mnplayersvs.c`'s sprites, cameras, portrait code,
//! cursor, pucks and banner; those pieces are RE-411's
//! (`players_vs::layer`). What differs is the layout: two panels on
//! `MNPlayers1PMode`'s wider card at x 53 and 185, no doors, buttons or
//! levels (`mnPlayers1PTrainingMakeHandicapLevel` is only reached from its
//! own process, which is never made), the "Training Mode" label, and the
//! fighters at x -830 and 830.
//!
//! [`View`] is the state the source keeps in GObjs and SObjs. The logic in
//! [`FighterSelect`] updates it at the source's call sites (the `v_*` hooks
//! below); [`FighterSelect::tick`] then runs the display GObjs' own
//! processes (`view_tick`). [`FighterSelect::visit`] yields what draws,
//! back to front in the cameras' order (the same priorities as the VS
//! select's: 80 wallpaper and labels, 75 fire backgrounds, 73 flashes, 70
//! portraits, 50 cards, texts, names and "Press Start", 30 fighters, 25
//! pucks, 20 cursor and held puck, 10 banner).

use crate::fighter::FighterKind;
use crate::players_vs::layer::{
    self as vs, common, piece, Draw, Fighter, Flash, Piece, FILE_EMBLEMS, FILE_GAME_MODES,
    FILE_PLAYERS_COMMON, FLASH_LENGTH,
};
use crate::players_vs::DrawKey;

use super::{portrait, FighterSelect, PlayerKind, COM, MAN};

pub use vs::VIEWPORT;

/// `llMNPlayers1PModeFileID`: the file of the Training card.
pub const FILE_PLAYERS_1P_MODE: u32 = 23;
/// `llMNPlayers1PModeRedCardSprite`.
pub const RED_CARD: u32 = 0x32A8;
/// `llMNPlayersGameModesTrainingModeTextSprite`.
pub const TRAINING_MODE_TEXT: u32 = 0x758;

/// The card and its LUT count: 0 the player's `GateMan1PLUT` (the man is
/// port 0), 1 the CPU's `GateCPLUT` (`mnPlayers1PTrainingSetGateLUT`).
pub const GATE_CARD: (u32, u32, u8) = (FILE_PLAYERS_1P_MODE, RED_CARD, 2);

/// Every sprite the layer names beyond RE-411's, as `(file, offset)`, for
/// the pack's check. The card is packed through its LUTs.
pub const SPRITES: &[(u32, u32)] = &[(FILE_GAME_MODES, TRAINING_MODE_TEXT)];

/// The cursor's and pucks' display lists (`gcMoveGObjDL`'s links).
const DL_CURSOR: u8 = 32;
const DL_PUCK: u8 = 33;
/// `mnPlayers1PTrainingMakeCursor`'s `priorities`, by port.
const CURSOR_PRIORITIES: [u32; 4] = [6, 4, 2, 0];
/// `mnPlayers1PTrainingMakePuck`'s `dropped_priorities`, by port.
const PUCK_DROPPED_PRIORITIES: [u32; 4] = [3, 2, 1, 0];

/// `mnPlayers1PTrainingMakeGate`'s card corner, by slot.
const CARD_POS: [(f32, f32); 2] = [(53.0, 127.0), (185.0, 127.0)];
/// `mnPlayers1PTrainingMakeNameAndEmblem`: the emblem's corner and grey,
/// and the name's corner, by slot. Its `pos` table is never read.
const EMBLEM_POS: [(f32, f32); 2] = [(63.0, 144.0), (195.0, 144.0)];
const EMBLEM_GREY: [u8; 2] = [0x1E, 0x44];
const NAME_POS: [(f32, f32); 2] = [(61.0, 202.0), (193.0, 202.0)];
/// `mnPlayers1PTrainingMakeFighter`'s `translate.x`, by slot (`y` -870).
const FIGHTER_X: [f32; 2] = [-830.0, 830.0];

/// One slot's display GObjs.
#[derive(Debug, Clone, Default)]
pub struct SlotView {
    /// `name_emblem_gobj` shown.
    pub name_shown: bool,
    /// The fighter `mnPlayers1PTrainingMakeNameAndEmblem` last drew.
    pub name_made: Option<FighterKind>,
    pub flash: Option<Flash>,
    /// `sMNPlayers1PTrainingSlots[].player`, the fighter GObj.
    pub fighter: Option<Fighter>,
    is_status_selected: bool,
    /// The puck GObj's flags as `mnPlayers1PTrainingPuckProcUpdate` last
    /// set them.
    pub puck_shown: bool,
    pub puck_draw: DrawKey,
}

/// The scene's display state.
#[derive(Debug, Clone)]
pub struct View {
    /// The portraits' `pos.x`, by portrait.
    pub portrait_x: [f32; 12],
    /// `sMNPlayers1PTrainingPuckGlowColor` and `...IsPuckGlowIncreasing`.
    pub glow: i32,
    glow_up: bool,
    /// `sMNPlayers1PTrainingReadyBlinkWait`.
    ready_blink: i32,
    /// The banner's and the "Press Start" GObj's flags.
    pub ready_banner: bool,
    pub ready_press: bool,
    pub cursor_draw: DrawKey,
    seq: u32,
    pub slots: [SlotView; 2],
}

impl Default for View {
    fn default() -> Self {
        View {
            portrait_x: vs::PORTRAIT_START_X,
            glow: 0,
            glow_up: false,
            ready_blink: 0,
            ready_banner: false,
            ready_press: false,
            cursor_draw: DrawKey::default(),
            seq: 0,
            slots: Default::default(),
        }
    }
}

/// A fighter's `translate`: `FIGHTER_X[slot]`, -870, 0.
pub fn fighter_position(slot: usize) -> [f32; 3] {
    [FIGHTER_X[slot], -870.0, 0.0]
}

impl FighterSelect {
    fn draw_key(&mut self, link: u8, priority: u32) -> DrawKey {
        self.view.seq += 1;
        DrawKey {
            link,
            priority,
            seq: self.view.seq,
        }
    }

    /// The presentation half of `mnPlayers1PTrainingFuncStart`:
    /// `mnPlayers1PTrainingMakePortraitAll` (every portrait at its
    /// `mnPlayers1PTrainingSetPortraitWallpaperPosition` start) and, per
    /// slot, `mnPlayers1PTrainingInitSlot`'s cursor, puck, gate (with its
    /// name and emblem) and the placed fighter's
    /// `mnPlayers1PTrainingMakeFighter`.
    pub(super) fn v_init(&mut self) {
        self.view = View::default();
        // `mnPlayers1PTrainingMakeCursor`.
        self.view.cursor_draw = self.draw_key(DL_CURSOR, CURSOR_PRIORITIES[MAN]);
        for slot in [MAN, COM] {
            // `mnPlayers1PTrainingMakePuck`.
            let mut key = self.draw_key(DL_PUCK, PUCK_DROPPED_PRIORITIES[slot]);
            let s = &self.slots[slot];
            if s.player_kind == PlayerKind::Man && s.held.is_some() {
                key = self.draw_key(DL_CURSOR, CURSOR_PRIORITIES[slot] + 1);
            }
            self.view.slots[slot].puck_draw = key;
            self.v_update_name_and_emblem(slot);
            if self.slots[slot].is_selected && self.slots[slot].kind.is_some() {
                self.v_make_fighter(slot);
            }
        }
    }

    /// `mnPlayers1PTrainingUpdateNameAndEmblem` and
    /// `mnPlayers1PTrainingMakeNameAndEmblem`: hidden with no fighter and
    /// no placed puck; with no fighter the old sprites stay.
    pub(super) fn v_update_name_and_emblem(&mut self, slot: usize) {
        let s = &self.slots[slot];
        let v = &mut self.view.slots[slot];
        if s.kind.is_none() && !s.is_selected {
            v.name_shown = false;
            return;
        }
        v.name_shown = true;
        if let Some(kind) = s.kind {
            v.name_made = Some(kind);
        }
    }

    /// `mnPlayers1PTrainingMakeFighter`: the fighter is made again in its
    /// status `nFTDemoStatusNull`, keeping its turn.
    fn v_make_fighter(&mut self, slot: usize) {
        let Some(kind) = self.slots[slot].kind else {
            return;
        };
        let v = &mut self.view.slots[slot];
        let (rotate_y, serial) = v.fighter.map_or((0.0, 0), |f| (f.rotate_y, f.serial + 1));
        v.fighter = Some(Fighter {
            kind,
            rotate_y,
            status: None,
            serial,
            hidden: false,
        });
    }

    /// `mnPlayers1PTrainingUpdateFighter`'s model: an existing fighter with
    /// no fighter under an unplaced puck is hidden; otherwise it is made
    /// again and its selected status cleared.
    pub(super) fn v_update_fighter(&mut self, slot: usize) {
        let s = &self.slots[slot];
        let v = &mut self.view.slots[slot];
        if v.fighter.is_some() && s.kind.is_none() && !s.is_selected {
            if let Some(f) = v.fighter.as_mut() {
                f.hidden = true;
            }
            return;
        }
        self.v_make_fighter(slot);
        self.view.slots[slot].is_status_selected = false;
    }

    /// `mnPlayers1PTrainingMakePortraitFlash` over the slot's fighter's
    /// portrait.
    pub(super) fn v_make_portrait_flash(&mut self, slot: usize) {
        self.view.slots[slot].flash = self.slots[slot].kind.map(|kind| Flash {
            portrait: portrait(kind),
            hidden: false,
            length: FLASH_LENGTH,
        });
    }

    /// `mnPlayers1PTrainingDestroyPortraitFlash`.
    pub(super) fn v_destroy_portrait_flash(&mut self, slot: usize) {
        self.view.slots[slot].flash = None;
    }

    /// `mnPlayers1PTrainingUpdateCursorGrabPriorities` for the player's
    /// cursor, the only one: it and the puck it grabbed go to the top of
    /// the cursor link. No other slot has a cursor or holds a puck.
    pub(super) fn v_grab_priorities(&mut self, puck: usize) {
        let top = CURSOR_PRIORITIES[3];
        self.view.cursor_draw = self.draw_key(DL_CURSOR, top);
        self.view.slots[puck].puck_draw = self.draw_key(DL_CURSOR, top + 1);
    }

    /// `mnPlayers1PTrainingUpdateCursorPlacementPriorities` for the
    /// player's cursor: no other cursor holds a puck, so it takes
    /// `unheld_priorities[3]` and the placed puck goes back to the puck
    /// link just above it.
    pub(super) fn v_placement_priorities(&mut self, puck: usize) {
        let pr = CURSOR_PRIORITIES[3];
        self.view.cursor_draw = self.draw_key(DL_CURSOR, pr);
        self.view.slots[puck].puck_draw = self.draw_key(DL_PUCK, pr + 1);
    }

    /// The display GObjs' processes, once per tick after the logic's: the
    /// portraits' slide (`mnPlayers1PTrainingPortraitProcUpdate`), the
    /// flashes' threads, the fighters (`mnPlayers1PTrainingFighterProcUpdate`),
    /// the puck glow (`mnPlayers1PTrainingPuckGlowProcUpdate`) and the two
    /// ready GObjs' `mnPlayers1PTrainingReadyProcUpdate`.
    pub(super) fn view_tick(&mut self) {
        for (i, x) in self.view.portrait_x.iter_mut().enumerate() {
            if let Some(next) = vs::next_portrait_x(i, *x) {
                *x = next;
            }
        }
        for slot in [MAN, COM] {
            vs::step_flash(&mut self.view.slots[slot].flash);
            let selected = self.slots[slot].is_fighter_selected;
            let v = &mut self.view.slots[slot];
            if let Some(f) = v.fighter.as_mut() {
                vs::turn_fighter(f, &mut v.is_status_selected, selected);
            }
        }
        vs::step_glow(&mut self.view.glow, &mut self.view.glow_up);
        let ready = self.is_ready();
        self.view.ready_banner = vs::step_ready(&mut self.view.ready_blink, ready);
        self.view.ready_press = vs::step_ready(&mut self.view.ready_blink, ready);
    }

    /// What draws, back to front.
    pub fn visit(&self, mut f: impl FnMut(Draw)) {
        f(Draw::Scissor(VIEWPORT));
        // Camera 80, link 26.
        vs::visit_stone(&mut f);
        f(Draw::Sprite(Piece {
            prim: Some([0xE3, 0xAC, 0x04]),
            ..piece(FILE_GAME_MODES, TRAINING_MODE_TEXT, 27.0, 24.0)
        }));
        f(Draw::Sprite(piece(
            FILE_PLAYERS_COMMON,
            common::BACK_BUTTON,
            244.0,
            23.0,
        )));
        // Cameras 75, 73 and 70.
        let flashes = [self.view.slots[MAN].flash, self.view.slots[COM].flash];
        vs::visit_portraits(&self.view.portrait_x, self.fighter_mask, &flashes, &mut f);
        // Camera 50, link 28, in the order the GObjs were made: each slot's
        // card, "1P"/"CP" text and name, then "Press Start".
        for slot in [MAN, COM] {
            self.visit_gate(slot, &mut f);
        }
        if self.view.ready_press {
            vs::visit_press_start(&mut f);
        }
        f(Draw::Fighters);
        self.visit_pucks_and_cursor(&mut f);
        if self.view.ready_banner {
            vs::visit_ready_banner(&mut f);
        }
    }

    /// `mnPlayers1PTrainingMakeGate`, `mnPlayers1PTrainingMakePlayerKind`
    /// and the name and emblem.
    fn visit_gate(&self, slot: usize, f: &mut impl FnMut(Draw)) {
        let (x, y) = CARD_POS[slot];
        f(Draw::Sprite(Piece {
            lut: Some(slot as u8),
            ..piece(FILE_PLAYERS_1P_MODE, RED_CARD, x, y)
        }));
        let text = if slot == MAN {
            // `pos_x[player] + 53`: the man is port 0.
            piece(FILE_PLAYERS_COMMON, common::TEXT_1P[0], 8.0 + 53.0, 131.0)
        } else {
            piece(FILE_PLAYERS_COMMON, common::TEXT_CP, 192.0, 132.0)
        };
        f(Draw::Sprite(Piece {
            prim: Some([0; 3]),
            ..text
        }));
        let v = &self.view.slots[slot];
        let Some(kind) = v.name_made.filter(|_| v.name_shown) else {
            return;
        };
        let k = (kind as usize).min(11);
        let (ex, ey) = EMBLEM_POS[slot];
        f(Draw::Sprite(Piece {
            prim: Some([EMBLEM_GREY[slot]; 3]),
            ..piece(FILE_EMBLEMS, vs::EMBLEMS[k], ex, ey)
        }));
        let (nx, ny) = NAME_POS[slot];
        f(Draw::Sprite(piece(
            FILE_PLAYERS_COMMON,
            common::NAMES[k],
            nx,
            ny,
        )));
    }

    /// Cameras 25 and 20: the pucks on link 33, then link 32's cursor and
    /// held puck, each by descending priority, then as made.
    fn visit_pucks_and_cursor(&self, f: &mut impl FnMut(Draw)) {
        // `None` is the cursor.
        let mut items: [(DrawKey, Option<usize>); 3] = [(self.view.cursor_draw, None); 3];
        let mut n = 1;
        for slot in [MAN, COM] {
            if self.view.slots[slot].puck_shown {
                items[n] = (self.view.slots[slot].puck_draw, Some(slot));
                n += 1;
            }
        }
        let items = &mut items[..n];
        items.sort_unstable_by_key(|(k, _)| {
            (
                if k.link == DL_PUCK { 0 } else { 1 },
                core::cmp::Reverse(k.priority),
                k.seq,
            )
        });
        for &(_, item) in items.iter() {
            match item {
                // `mnPlayers1PTrainingUpdatePuck`: the CPU's is "CP".
                Some(slot) => {
                    let id = if slot == COM { 4 } else { MAN };
                    f(vs::puck(id, self.slots[slot].puck, self.view.glow));
                }
                None => {
                    let s = &self.slots[MAN];
                    vs::visit_cursor(MAN, s.cursor, s.cursor_status, f);
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "fighter_select_layer_tests.rs"]
mod tests;
