//! The Bonus Practice character select's presentation, from
//! `mn/mnplayers/mnplayers1pbonus.c`: the wallpaper and the records
//! (`mnPlayers1PBonusMakeWallpaper`, `...MakeTotalTime`,
//! `...MakeHiScore` with `...MakeBestTime` or `...MakeBestTaskCount`), the
//! title and back button (`mnPlayers1PBonusMakeLabels`), the portraits and
//! their slide-in, the portrait flash, the panel (`mnPlayers1PBonusMakeGate`:
//! the card, the "1P" text, the name and emblem), the fighter's turn and
//! demo status, the puck, the cursor and the "ready to fight" banner with
//! "Press Start".
//!
//! The pieces are the 1P select's (`players_1p::layer`) moved right: the
//! panel at x 58 instead of 25, the fighter at x -700 instead of -1100.
//! The puck has no glow, and always draws under the cursor.
//!
//! Every camera has the viewport (10, 10) to (310, 230), and a higher
//! camera priority draws first: the wallpaper and records (80, link 26),
//! the title and back button (70, link 34), the unlocked portraits' fire
//! backgrounds (60, link 32), the flash (50, link 33), the portraits (40,
//! link 27), the card, "1P" text, name, emblem and "Press Start" (30, link
//! 28), the fighter (20), the dropped puck (15, link 31), the cursor and
//! held puck (13, link 30) and the banner (10, link 35). On link 26 the
//! total time GObj (made by `FuncStart`) draws before the fighter's record
//! (remade by every puck tick).

use crate::fighter::FighterKind;
use crate::players_1p::layer::{mode, number, FILE_PLAYERS_1P_MODE, GATE_LUTS, KIND_TEXT_X};
use crate::players_vs::layer::{
    self as vs, common, piece, Fighter, Flash, Piece, FILE_EMBLEMS, FILE_GAME_MODES,
    FILE_PLAYERS_COMMON, FLASH_LENGTH,
};
use crate::results_scene::FIGHTER_SCALES;

use super::{portrait, BonusKind, FighterRecord, Players1PBonus, Records, Time};

pub use vs::{Draw, VIEWPORT};

/// `llMNPlayersGameModes*` offsets (US) this scene adds.
pub mod game_modes {
    pub const BONUS1_BREAK_THE_TARGETS_TEXT: u32 = 0xBD8;
    pub const BONUS2_BOARD_THE_PLATFORMS_TEXT: u32 = 0x1058;
}

/// `llMNPlayers1PMode*` offsets (US) this scene adds.
pub mod records {
    pub const BEST_TIME_TEXT: u32 = 0x12E0;
    pub const TOTAL_BEST_TIME_TEXT: u32 = 0x1410;
    pub const TARGETS_TEXT: u32 = 0x1658;
    pub const PLATFORMS_TEXT: u32 = 0x1898;
    /// The seconds mark (`"`).
    pub const SEC: u32 = 0x1F48;
    /// The hundredths mark.
    pub const CSEC: u32 = 0x1FC8;
}

/// The sprites this scene draws that the resident pack's sprite files
/// (`ssb_rom::sprite::FILES`) do not hold, as `(file, offset)`: the
/// practice titles and the records' labels and marks. Every other sprite
/// the layer draws is one the VS or 1P select's lists name.
pub const MENU_SPRITES: &[(u32, u32)] = &[
    (FILE_GAME_MODES, game_modes::BONUS1_BREAK_THE_TARGETS_TEXT),
    (FILE_GAME_MODES, game_modes::BONUS2_BOARD_THE_PLATFORMS_TEXT),
    (FILE_PLAYERS_1P_MODE, records::BEST_TIME_TEXT),
    (FILE_PLAYERS_1P_MODE, records::TOTAL_BEST_TIME_TEXT),
    (FILE_PLAYERS_1P_MODE, records::TARGETS_TEXT),
    (FILE_PLAYERS_1P_MODE, records::PLATFORMS_TEXT),
    (FILE_PLAYERS_1P_MODE, records::SEC),
    (FILE_PLAYERS_1P_MODE, records::CSEC),
];

/// `mnPlayers1PBonusMakeGate`'s card and "1P" text `pos_x` base.
const GATE_X: f32 = 58.0;
/// `mnPlayers1PBonusMakeFighter`'s `translate`.
pub const FIGHTER_POSITION: [f32; 3] = [-700.0, -850.0, 0.0];
/// The records' label colour and the digits' environment and primitive
/// (`colors2`).
const RECORD_TEXT: [u8; 3] = [0x7E, 0x7C, 0x77];
const RECORD_DIGITS: ([u8; 3], [u8; 3]) = ([0; 3], RECORD_TEXT);

/// The scene's display state.
#[derive(Debug, Clone)]
pub struct View {
    /// The portraits' `pos.x`, by portrait.
    pub portrait_x: [f32; 12],
    /// `sMNPlayers1PBonusReadyBlinkWait`.
    ready_blink: i32,
    /// The banner's and the "Press Start" GObj's flags.
    pub ready_banner: bool,
    pub ready_press: bool,
    /// `name_emblem_gobj` shown.
    pub name_shown: bool,
    /// The fighter `mnPlayers1PBonusMakeNameAndEmblem` last drew.
    pub name_made: Option<FighterKind>,
    pub flash: Option<Flash>,
    /// `sMNPlayers1PBonusSlot.player`, the fighter GObj.
    pub fighter: Option<Fighter>,
    is_status_selected: bool,
    /// The puck GObj's flags as `mnPlayers1PBonusPuckProcUpdate` last set
    /// them.
    pub puck_shown: bool,
    /// Runs of `mnPlayers1PBonusMakeHiScore`; the fighter's record draws
    /// once one has run.
    pub hiscore_made: u32,
}

impl Default for View {
    fn default() -> Self {
        View {
            portrait_x: vs::PORTRAIT_START_X,
            ready_blink: 0,
            ready_banner: false,
            ready_press: false,
            name_shown: false,
            name_made: None,
            flash: None,
            fighter: None,
            is_status_selected: false,
            puck_shown: false,
            hiscore_made: 0,
        }
    }
}

impl Players1PBonus {
    /// The presentation half of `mnPlayers1PBonusFuncStart`: the gate's
    /// name and emblem start hidden (`ResetPlayer` leaves no fighter), and
    /// no fighter is made.
    pub(super) fn v_init(&mut self) {
        self.view = View::default();
        self.v_update_name_and_emblem();
    }

    /// `mnPlayers1PBonusMakeHiScore`.
    pub(super) fn v_make_hiscore(&mut self) {
        self.view.hiscore_made += 1;
    }

    /// `mnPlayers1PBonusUpdateNameAndEmblem` and `MakeNameAndEmblem`:
    /// hidden with no fighter and no placed puck; with no fighter the old
    /// sprites stay.
    pub(super) fn v_update_name_and_emblem(&mut self) {
        let s = &self.slot;
        let v = &mut self.view;
        if s.kind.is_none() && !s.is_selected {
            v.name_shown = false;
            return;
        }
        v.name_shown = true;
        if let Some(kind) = s.kind {
            v.name_made = Some(kind);
        }
    }

    /// `mnPlayers1PBonusMakeFighter` (only with a fighter) and the rest of
    /// `UpdateFighter`'s second branch: shown, its selected status
    /// cleared. The fighter is made again in `nFTDemoStatusNull`, keeping
    /// its turn.
    pub(super) fn v_make_fighter(&mut self) {
        if let Some(kind) = self.slot.kind {
            let v = &mut self.view;
            let (rotate_y, serial) = v.fighter.map_or((0.0, 0), |f| (f.rotate_y, f.serial + 1));
            v.fighter = Some(Fighter {
                kind,
                rotate_y,
                status: None,
                serial,
                hidden: false,
            });
        }
        if let Some(f) = self.view.fighter.as_mut() {
            f.hidden = false;
        }
        self.view.is_status_selected = false;
    }

    /// `mnPlayers1PBonusUpdateFighter`'s first branch: the fighter is
    /// hidden.
    pub(super) fn v_hide_fighter(&mut self) {
        if let Some(f) = self.view.fighter.as_mut() {
            f.hidden = true;
        }
    }

    /// `mnPlayers1PBonusMakePortraitFlash`.
    pub(super) fn v_make_portrait_flash(&mut self) {
        self.view.flash = self.slot.kind.map(|kind| Flash {
            portrait: portrait(kind),
            hidden: false,
            length: FLASH_LENGTH,
        });
    }

    /// `mnPlayers1PBonusDestroyPortraitFlash`.
    pub(super) fn v_destroy_portrait_flash(&mut self) {
        self.view.flash = None;
    }

    /// The display GObjs' processes, once per tick after the logic's, as
    /// the 1P select orders them: "Press Start"'s ready check (link 22),
    /// the flash (26), the banner's ready check (28), the portraits' slide
    /// and the fighter's turn. Both ready checks read the armed start
    /// (`sMNPlayers1PBonusIsSelected`) and share one blink counter.
    pub(super) fn view_tick(&mut self) {
        let ready = self.is_selected;
        self.view.ready_press = vs::step_ready(&mut self.view.ready_blink, ready);
        vs::step_flash(&mut self.view.flash);
        self.view.ready_banner = vs::step_ready(&mut self.view.ready_blink, ready);
        for (i, x) in self.view.portrait_x.iter_mut().enumerate() {
            if let Some(next) = vs::next_portrait_x(i, *x) {
                *x = next;
            }
        }
        let selected = self.slot.is_fighter_selected;
        let v = &mut self.view;
        if let Some(f) = v.fighter.as_mut() {
            vs::turn_fighter(f, &mut v.is_status_selected, selected);
        }
    }

    /// The fighter's `translate`, scale and costume, when it draws.
    pub fn fighter_draw(&self) -> Option<(Fighter, [f32; 3], f32, u8)> {
        let f = self.view.fighter.filter(|f| !f.hidden)?;
        let scale = FIGHTER_SCALES[(f.kind as usize).min(11)];
        Some((f, FIGHTER_POSITION, scale, self.slot.costume))
    }

    /// What draws, back to front. `records` are the backup's
    /// ([`Players1PBonus::records`]).
    pub fn visit(&self, records: &Records, mut f: impl FnMut(Draw)) {
        f(Draw::Scissor(VIEWPORT));
        // Camera 80, link 26: the stone, the total, the fighter's record.
        vs::visit_stone(&mut f);
        if let Some(total) = records.total {
            visit_total_time(&total, &mut f);
        }
        if self.view.hiscore_made > 0 {
            if let Some(record) = records.fighter {
                visit_hiscore(records.bonus, record, &mut f);
            }
        }
        // Camera 70, link 34: `mnPlayers1PBonusMakeLabels`.
        let title = match self.bonus {
            BonusKind::Targets => game_modes::BONUS1_BREAK_THE_TARGETS_TEXT,
            BonusKind::Platforms => game_modes::BONUS2_BOARD_THE_PLATFORMS_TEXT,
        };
        f(Draw::Sprite(Piece {
            prim: Some([0xE3, 0xAC, 0x04]),
            ..piece(FILE_GAME_MODES, title, 27.0, 24.0)
        }));
        f(Draw::Sprite(piece(
            FILE_PLAYERS_COMMON,
            common::BACK_BUTTON,
            244.0,
            23.0,
        )));
        // Cameras 60, 50 and 40.
        vs::visit_portraits(
            &self.view.portrait_x,
            self.fighter_mask(),
            &[self.view.flash],
            &mut f,
        );
        // Camera 30, link 28: the card, "1P", the name and emblem, then
        // "Press Start".
        self.visit_gate(&mut f);
        if self.view.ready_press {
            vs::visit_press_start(&mut f);
        }
        f(Draw::Fighters);
        // Cameras 15 and 13.
        let p = usize::from(self.player).min(3);
        if self.view.puck_shown {
            f(Draw::Sprite(piece(
                FILE_PLAYERS_COMMON,
                common::PUCKS[p],
                self.slot.puck.0,
                self.slot.puck.1,
            )));
        }
        vs::visit_cursor(p, self.slot.cursor, self.slot.cursor_status, &mut f);
        if self.view.ready_banner {
            vs::visit_ready_banner(&mut f);
        }
    }

    /// `mnPlayers1PBonusMakeGate` and `...MakeNameAndEmblem`.
    fn visit_gate(&self, f: &mut impl FnMut(Draw)) {
        let p = usize::from(self.player).min(3);
        f(Draw::Sprite(Piece {
            lut: Some(GATE_LUTS[p]),
            ..piece(FILE_PLAYERS_1P_MODE, mode::RED_CARD, GATE_X, 127.0)
        }));
        f(Draw::Sprite(Piece {
            prim: Some([0; 3]),
            ..piece(
                FILE_PLAYERS_COMMON,
                common::TEXT_1P[p],
                KIND_TEXT_X[p] + GATE_X,
                132.0,
            )
        }));
        let Some(kind) = self.view.name_made.filter(|_| self.view.name_shown) else {
            return;
        };
        let k = (kind as usize).min(11);
        f(Draw::Sprite(Piece {
            prim: Some([0; 3]),
            ..piece(FILE_EMBLEMS, vs::EMBLEMS[k], 68.0, 144.0)
        }));
        f(Draw::Sprite(piece(
            FILE_PLAYERS_COMMON,
            common::NAMES[k],
            66.0,
            202.0,
        )));
    }
}

/// A records label or mark: `prim` the label colour, `env` black.
fn label(offset: u32, x: f32, y: f32) -> Draw {
    Draw::Sprite(Piece {
        prim: Some(RECORD_TEXT),
        ..piece(FILE_PLAYERS_1P_MODE, offset, x, y)
    })
}

/// Two-digit (or `max`-digit) fixed numbers in the records' colours.
fn digits(n: i32, x: f32, y: f32, max: u32, f: &mut impl FnMut(Draw)) {
    // `mnPlayers1PBonusMakeNumber` draws a negative number as 0.
    number(n.max(0) as u32, x, y, RECORD_DIGITS, max, true, &mut |p| {
        f(Draw::Sprite(p))
    });
}

/// `mnPlayers1PBonusMakeHiScore`: the best time of a finished practice
/// (`MakeBestTime`), else the targets or platforms count
/// (`MakeBestTaskCount`, US positions).
fn visit_hiscore(bonus: BonusKind, record: FighterRecord, f: &mut impl FnMut(Draw)) {
    match record {
        FighterRecord::Time(t) => {
            f(label(records::BEST_TIME_TEXT, 177.0, 198.0));
            digits(t.mins, 237.0, 195.0, 2, f);
            f(label(records::SEC, 239.0, 195.0));
            digits(t.secs, 259.0, 195.0, 2, f);
            f(label(records::CSEC, 261.0, 195.0));
            digits(t.csecs, 283.0, 195.0, 2, f);
        }
        FighterRecord::Tasks(count) => {
            let text = match bonus {
                BonusKind::Targets => records::TARGETS_TEXT,
                BonusKind::Platforms => records::PLATFORMS_TEXT,
            };
            f(label(text, 235.0, 195.0));
            digits(i32::from(count), 225.0, 194.0, 2, f);
        }
    }
}

/// `mnPlayers1PBonusMakeTotalTime`, in its order: the label, the
/// hundredths, their mark, the seconds, their mark, then three digits of
/// minutes.
fn visit_total_time(t: &Time, f: &mut impl FnMut(Draw)) {
    f(label(records::TOTAL_BEST_TIME_TEXT, 142.0, 209.0));
    digits(t.csecs, 283.0, 206.0, 2, f);
    f(label(records::CSEC, 261.0, 206.0));
    digits(t.secs, 259.0, 206.0, 2, f);
    f(label(records::SEC, 239.0, 206.0));
    digits(t.mins, 237.0, 206.0, 3, f);
}

/// Whether `(file, offset)` is one of the VS or 1P select's listed
/// sprites (resident in the pack), the gate card, or this scene's
/// [`MENU_SPRITES`].
#[cfg(test)]
pub(crate) fn is_listed(file: u32, offset: u32) -> bool {
    use crate::players_1p::layer::{self as one, FILE_DIGITS};
    let in_lists = |sprites: &[(u32, u32)], lists: &[(u32, &[u32])]| {
        sprites.contains(&(file, offset))
            || lists.iter().any(|&(f, l)| f == file && l.contains(&offset))
    };
    in_lists(vs::SPRITES, vs::SPRITE_LISTS)
        || in_lists(one::SPRITES, one::SPRITE_LISTS)
        || (file, offset) == (FILE_PLAYERS_1P_MODE, mode::RED_CARD)
        || (file == FILE_DIGITS && one::DIGITS.contains(&offset))
        || MENU_SPRITES.contains(&(file, offset))
}

#[cfg(test)]
#[path = "players_1p_bonus_layer_tests.rs"]
mod tests;
