//! The 1P Game character select's presentation, from
//! `mn/mnplayers/mnplayers1pgame.c`: the wallpaper, the time selector and
//! the records (`mnPlayers1PGameMakeWallpaper`, `...MakeTimeSelect`,
//! `...MakeFighterRecord`, `...MakeTotalRecord`), the labels with the
//! level and stock options (`mnPlayers1PGameMakeLabels`,
//! `...MakeLevelOption`, `...MakeStockOption`), the portraits and their
//! slide-in, the portrait flash, the panel (`mnPlayers1PGameMakeGate`: the
//! card, the "1P" text, the name and emblem), the fighter's turn and demo
//! status, the puck, the cursor and the "ready to fight" banner with
//! "Press Start".
//!
//! The portraits, cursor, banner and fighter turn are RE-411's
//! (`players_vs::layer`); the sprites are the same files. The puck has no
//! glow here (`lbCommonDrawSObjAttr`), and the one slot's puck always
//! draws under its cursor: dropped on link 31 (camera 15) or held on link
//! 30 one priority behind the cursor (camera 13).
//!
//! Every camera has the viewport (10, 10) to (310, 230), and a higher
//! camera priority draws first: the wallpaper, time selector and records
//! (80, link 26), the labels, level and stock (70, link 34), the unlocked
//! portraits' fire backgrounds (60, link 32), the flash (50, link 33), the
//! portraits (40, link 27), the card, "1P" text, name, emblem and "Press
//! Start" (30, link 28), the fighter (20), the dropped puck (15, link 31),
//! the cursor and held puck (13, link 30) and the banner (10, link 35).

use crate::fighter::FighterKind;
use crate::players_vs::layer::{
    self as vs, common, piece, Fighter, Flash, Piece, FILE_EMBLEMS, FILE_PLAYERS_COMMON,
    FLASH_LENGTH,
};
use crate::results_scene::FIGHTER_SCALES;

use super::{portrait, Players1P, Records};

pub use vs::VIEWPORT;

/// `llMNPlayers1PModeFileID`.
pub const FILE_PLAYERS_1P_MODE: u32 = 23;
/// `llMNPlayersDifficultyFileID`.
pub const FILE_DIFFICULTY: u32 = 24;
/// `llFTStocksZakoFileID`.
pub const FILE_STOCKS_ZAKO: u32 = 25;
/// `llMNCommonFontsFileID`.
pub const FILE_FONTS: u32 = 33;
/// `llIFCommonDigitsFileID`.
pub const FILE_DIGITS: u32 = 36;

/// `llMNPlayers1PMode*` offsets (US).
pub mod mode {
    pub const ONE_PLAYER_GAME_TEXT: u32 = 0x228;
    pub const CLOSING_PARENTHESIS: u32 = 0x2C8;
    pub const OPENING_PARENTHESIS: u32 = 0x368;
    pub const LEVEL_COLON_TEXT: u32 = 0x488;
    pub const STOCK_COLON_TEXT: u32 = 0x5A8;
    pub const OPTION_OUTLINE: u32 = 0x1208;
    pub const SMASH_LOGO: u32 = 0x1950;
    pub const OPTION_TEXT: u32 = 0x1EC8;
    pub const RED_CARD: u32 = 0x32A8;
}

/// `mnPlayers1PGameMakeLevel`'s `offsets`, by level: `VeryEasyText`,
/// `EasyText`, `NormalText`, `HardText`, `VeryHardText`.
pub const LEVEL_TEXT: [u32; 5] = [0x438, 0x98, 0x2D8, 0x178, 0x598];
/// `mnPlayers1PGameMakeLevel`'s `pos`, by level.
const LEVEL_POS: [(f32, f32); 5] = [
    (204.0, 159.0),
    (219.0, 159.0),
    (209.0, 159.0),
    (219.0, 159.0),
    (205.0, 159.0),
];
/// `mnPlayers1PGameMakeLevel`'s and `MakeHiScore`'s `colors`, by level.
pub const LEVEL_COLORS: [[u8; 3]; 5] = [
    [0x41, 0x6F, 0xE4],
    [0x8D, 0xBB, 0x5A],
    [0xE4, 0xBE, 0x41],
    [0xE4, 0x78, 0x41],
    [0xE4, 0x41, 0x41],
];
/// `llFTStocksZakoSprite`.
pub const ZAKO_STOCK: u32 = 0x80;
/// `llMNCommonFontsLetterASprite` to `...LetterZSprite`, then
/// `SymbolApostrophe`, `SymbolPercent` and `SymbolPeriod`.
pub const FONT: [u32; 29] = [
    0x40, 0xD0, 0x160, 0x1F0, 0x280, 0x310, 0x3A0, 0x430, 0x4C0, 0x550, 0x5E0, 0x670, 0x700, 0x790,
    0x820, 0x8B0, 0x940, 0x9D0, 0xA60, 0xAF0, 0xB80, 0xC10, 0xCA0, 0xD30, 0xDC0, 0xE50, 0xED0,
    0xF60, 0xFD0,
];
/// `mnPlayers1PGameMakeString`'s `widths`, by character ID.
pub(crate) const FONT_WIDTHS: [f32; 29] = [
    5.0, 4.0, 4.0, 4.0, 4.0, 4.0, 4.0, 4.0, 3.0, 4.0, 4.0, 4.0, 5.0, 5.0, 4.0, 4.0, 5.0, 4.0, 4.0,
    5.0, 4.0, 5.0, 5.0, 5.0, 5.0, 4.0, 2.0, 7.0, 3.0,
];
/// `llIFCommonDigits0Sprite` to `...9Sprite`.
pub(crate) const DIGITS: [u32; 10] = [
    0x68, 0x118, 0x1C8, 0x278, 0x328, 0x3D8, 0x488, 0x538, 0x5E8, 0x698,
];
/// `mnPlayers1PGameMakeTimeNumber`'s `widths`: a table, not the sprites'.
const TIME_DIGIT_WIDTHS: [f32; 10] = [8.0, 6.0, 9.0, 8.0, 8.0, 9.0, 8.0, 8.0, 8.0, 9.0];

/// Every sprite the layer names beyond RE-411's and the Training
/// select's, as `(file, offset)`, for the pack's check.
pub const SPRITES: &[(u32, u32)] = &[
    (FILE_PLAYERS_1P_MODE, mode::ONE_PLAYER_GAME_TEXT),
    (FILE_PLAYERS_1P_MODE, mode::CLOSING_PARENTHESIS),
    (FILE_PLAYERS_1P_MODE, mode::OPENING_PARENTHESIS),
    (FILE_PLAYERS_1P_MODE, mode::LEVEL_COLON_TEXT),
    (FILE_PLAYERS_1P_MODE, mode::STOCK_COLON_TEXT),
    (FILE_PLAYERS_1P_MODE, mode::OPTION_OUTLINE),
    (FILE_PLAYERS_1P_MODE, mode::SMASH_LOGO),
    (FILE_PLAYERS_1P_MODE, mode::OPTION_TEXT),
    (FILE_STOCKS_ZAKO, ZAKO_STOCK),
];

/// Every sprite list the layer indexes, for the pack's check.
pub const SPRITE_LISTS: &[(u32, &[u32])] = &[
    (FILE_DIFFICULTY, &LEVEL_TEXT),
    (FILE_FONTS, &FONT),
    (FILE_DIGITS, &DIGITS),
];

/// The card: file 23's `RedCardSprite` through `GateMan1PLUT` to
/// `...4PLUT` (`mnPlayers1PGameSetGateLUT`). In the pack's Training card
/// LUTs these are 0, then 2 to 4 (1 is the Training CPU's grey).
pub const GATE_LUTS: [u8; 4] = [0, 2, 3, 4];

/// `mnPlayers1PGameMakeGate`'s "1P" text `pos_x`, by port.
pub(crate) const KIND_TEXT_X: [f32; 4] = [8.0, 5.0, 5.0, 5.0];
/// `mnPlayers1PGameMakeFighter`'s `translate`.
pub const FIGHTER_POSITION: [f32; 3] = [-1100.0, -850.0, 0.0];
/// The level and stock threads' `blink_wait`.
const ARROW_BLINK: i32 = 10;

/// What the layer draws: the select's own pieces, plus what this scene
/// adds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Draw {
    /// A piece the VS and Training selects also draw.
    Select(vs::Draw),
    /// `mnPlayers1PGameLabelsProcDisplay`'s `gDPFillRectangle`,
    /// `[x0, y0, x1, y1]`, blended (`G_RM_AA_XLU_SURF`).
    Fill { rect: [f32; 4], color: [u8; 4] },
    /// `OptionOutline`: `cms` `G_TX_WRAP` with `masks` 0 (clamped in S),
    /// `cmt` `G_TX_MIRROR` with `maskt` 5 (its 32 rows, then mirrored),
    /// over `size` from the piece's corner.
    MirroredT { piece: Piece, size: [f32; 2] },
    /// A fighter's stock icon (`fp->attr->sprites->stock_sprite` through
    /// `stock_luts[costume]`), transparent.
    Stock {
        kind: FighterKind,
        costume: u8,
        x: f32,
        y: f32,
    },
}

/// A level or stock option's arrow thread (`mnPlayers1PGameLevelThreadUpdate`,
/// `...StockThreadUpdate`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Arrows {
    pub hidden: bool,
    blink_wait: i32,
    /// The left and right arrow `SObj`s, as the thread last left them.
    pub left: bool,
    pub right: bool,
}

impl Default for Arrows {
    fn default() -> Self {
        Arrows {
            hidden: false,
            blink_wait: ARROW_BLINK,
            left: false,
            right: false,
        }
    }
}

impl Arrows {
    /// One run of the thread: the GObj blinks every 10 runs, and the arrow
    /// at an end of the range is removed.
    fn step(&mut self, at_min: bool, at_max: bool) {
        self.blink_wait -= 1;
        if self.blink_wait == 0 {
            self.blink_wait = ARROW_BLINK;
            self.hidden = !self.hidden;
        }
        self.left = !at_min;
        self.right = !at_max;
    }
}

/// The scene's display state.
#[derive(Debug, Clone)]
pub struct View {
    /// The portraits' `pos.x`, by portrait.
    pub portrait_x: [f32; 12],
    /// `sMNPlayers1PGameReadyBlinkWait`.
    ready_blink: i32,
    /// The banner's and the "Press Start" GObj's flags.
    pub ready_banner: bool,
    pub ready_press: bool,
    /// `name_emblem_gobj` shown.
    pub name_shown: bool,
    /// The fighter `mnPlayers1PGameMakeNameAndEmblem` last drew.
    pub name_made: Option<FighterKind>,
    pub flash: Option<Flash>,
    /// `sMNPlayers1PGameSlot.player`, the fighter GObj.
    pub fighter: Option<Fighter>,
    is_status_selected: bool,
    /// The puck GObj's flags as `mnPlayers1PGamePuckProcUpdate` last set
    /// them.
    pub puck_shown: bool,
    pub level_arrows: Arrows,
    pub stock_arrows: Arrows,
    /// Runs of the level text's remake (`mnPlayers1PGameMakeLevel`), for
    /// tests.
    pub level_made: u32,
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
            level_arrows: Arrows::default(),
            stock_arrows: Arrows::default(),
            level_made: 0,
        }
    }
}

impl Players1P {
    /// The presentation half of `mnPlayers1PGameFuncStart`:
    /// `mnPlayers1PGameInitSlot`'s gate (with its name and emblem) and the
    /// placed fighter, and the level option's first text.
    pub(super) fn v_init(&mut self) {
        self.view = View::default();
        self.v_update_name_and_emblem();
        if self.slot.is_selected && self.slot.kind.is_some() {
            self.v_make_fighter();
        }
        self.v_make_level();
    }

    /// `mnPlayers1PGameMakeLevel`.
    pub(super) fn v_make_level(&mut self) {
        self.view.level_made += 1;
    }

    /// `mnPlayers1PGameUpdateNameAndEmblem` and `MakeNameAndEmblem`:
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

    /// `mnPlayers1PGameMakeFighter` (only with a fighter) and the rest of
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

    /// `mnPlayers1PGameUpdateFighter`'s first branch: the fighter is
    /// hidden.
    pub(super) fn v_hide_fighter(&mut self) {
        if let Some(f) = self.view.fighter.as_mut() {
            f.hidden = true;
        }
    }

    /// `mnPlayers1PGameMakePortraitFlash`.
    pub(super) fn v_make_portrait_flash(&mut self) {
        self.view.flash = self.slot.kind.map(|kind| Flash {
            portrait: portrait(kind),
            hidden: false,
            length: FLASH_LENGTH,
        });
    }

    /// `mnPlayers1PGameDestroyPortraitFlash`.
    pub(super) fn v_destroy_portrait_flash(&mut self) {
        self.view.flash = None;
    }

    /// `mnPlayers1PGameUpdateCursorGrabPriorities`: the puck joins the
    /// cursor's link behind it. It draws under the cursor either way.
    pub(super) fn v_grab_priorities(&mut self) {}

    /// `mnPlayers1PGameUpdateCursorPlacementPriorities`: the puck goes
    /// back to its own link.
    pub(super) fn v_placement_priorities(&mut self) {}

    /// The display GObjs' processes, once per tick after the logic's, by
    /// GObj link: "Press Start"'s ready check (22), the level and stock
    /// threads (23), the flash (26), the banner's ready check (28), the
    /// portraits' slide and the fighter's turn.
    pub(super) fn view_tick(&mut self) {
        let ready = self.is_ready();
        self.view.ready_press = vs::step_ready(&mut self.view.ready_blink, ready);
        self.view
            .level_arrows
            .step(self.level <= 0, self.level >= 4);
        self.view
            .stock_arrows
            .step(self.stock <= 0, self.stock >= super::STOCK_MAX);
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
    /// ([`Players1P::records`] for [`Players1P::force_puck_fighter_kind`]).
    pub fn visit(&self, records: &Records, mut f: impl FnMut(Draw)) {
        let mut put = |d: vs::Draw| f(Draw::Select(d));
        put(vs::Draw::Scissor(VIEWPORT));
        // Camera 80, link 26: the stone, the time selector, the records.
        vs::visit_stone(&mut put);
        self.visit_time(&mut put);
        visit_records(records, &mut put);
        // Camera 70, link 34.
        self.visit_labels(&mut f);
        let mut put = |d: vs::Draw| f(Draw::Select(d));
        // Cameras 60, 50 and 40.
        vs::visit_portraits(
            &self.view.portrait_x,
            self.fighter_mask,
            &[self.view.flash],
            &mut put,
        );
        // Camera 30, link 28: the card, "1P", the name and emblem, then
        // "Press Start".
        self.visit_gate(&mut put);
        if self.view.ready_press {
            vs::visit_press_start(&mut put);
        }
        put(vs::Draw::Fighters);
        // Cameras 15 and 13.
        let p = usize::from(self.player).min(3);
        if self.view.puck_shown {
            put(vs::Draw::Sprite(piece(
                FILE_PLAYERS_COMMON,
                common::PUCKS[p],
                self.slot.puck.0,
                self.slot.puck.1,
            )));
        }
        vs::visit_cursor(p, self.slot.cursor, self.slot.cursor_status, &mut put);
        if self.view.ready_banner {
            vs::visit_ready_banner(&mut put);
        }
    }

    /// `mnPlayers1PGameMakeTimeSelect` and `...MakeTimeSetting`.
    fn visit_time(&self, put: &mut impl FnMut(vs::Draw)) {
        put(vs::Draw::Sprite(piece(
            FILE_PLAYERS_COMMON,
            common::TIME_SELECTOR,
            140.0,
            22.0,
        )));
        if self.time_setting == crate::battle::TIMELIMIT_INFINITE {
            put(vs::Draw::Sprite(Piece {
                prim: Some([0xFF; 3]),
                env: [0x32, 0x1C, 0x0E],
                ..piece(FILE_PLAYERS_COMMON, common::INFINITY_DARK, 194.0, 24.0)
            }));
        } else {
            let x = if self.time_setting < 10 { 205.0 } else { 209.0 };
            vs::rule_number(
                i32::from(self.time_setting),
                x,
                23.0,
                2,
                &TIME_DIGIT_WIDTHS,
                |p| put(vs::Draw::Sprite(p)),
            );
        }
    }

    /// `mnPlayers1PGameMakeLabels` with its display, then the level and
    /// stock options: each arrow GObj, then its text or icons.
    fn visit_labels(&self, f: &mut impl FnMut(Draw)) {
        f(Draw::Fill {
            rect: [157.0, 136.0, 320.0, 141.0],
            color: [0x57, 0x60, 0x88, 0xFF],
        });
        let sprite = |f: &mut dyn FnMut(Draw), p: Piece| f(Draw::Select(vs::Draw::Sprite(p)));
        sprite(
            f,
            Piece {
                prim: Some([0xE3, 0xAC, 0x04]),
                ..piece(FILE_PLAYERS_1P_MODE, mode::ONE_PLAYER_GAME_TEXT, 27.0, 24.0)
            },
        );
        sprite(
            f,
            piece(FILE_PLAYERS_COMMON, common::BACK_BUTTON, 244.0, 23.0),
        );
        sprite(
            f,
            Piece {
                prim: Some([0xAF, 0xB1, 0xCC]),
                ..piece(FILE_PLAYERS_1P_MODE, mode::OPTION_TEXT, 180.0, 129.0)
            },
        );
        f(Draw::MirroredT {
            piece: Piece {
                prim: Some([0x57, 0x60, 0x88]),
                ..piece(FILE_PLAYERS_1P_MODE, mode::OPTION_OUTLINE, 128.0, 141.0)
            },
            size: [184.0, 64.0],
        });
        let label = [0xC5, 0xB6, 0xA7];
        sprite(
            f,
            Piece {
                prim: Some(label),
                ..piece(FILE_PLAYERS_1P_MODE, mode::LEVEL_COLON_TEXT, 145.0, 159.0)
            },
        );
        sprite(
            f,
            Piece {
                prim: Some(label),
                ..piece(FILE_PLAYERS_1P_MODE, mode::STOCK_COLON_TEXT, 144.0, 179.0)
            },
        );
        visit_arrows(&self.view.level_arrows, 158.0, f);
        let level = self.level.clamp(0, 4) as usize;
        let (x, y) = LEVEL_POS[level];
        sprite(
            f,
            Piece {
                prim: Some(LEVEL_COLORS[level]),
                ..piece(FILE_DIFFICULTY, LEVEL_TEXT[level], x, y)
            },
        );
        visit_arrows(&self.view.stock_arrows, 178.0, f);
        // `mnPlayers1PGameMakeStock`: the stocks right to left.
        for i in (1..=i32::from(self.stock) + 1).rev() {
            let x = ((i - 1) * 12) as f32 + 207.0;
            match self.slot.kind {
                None => sprite(f, piece(FILE_STOCKS_ZAKO, ZAKO_STOCK, x, 179.0)),
                Some(kind) => f(Draw::Stock {
                    kind,
                    costume: self.slot.costume,
                    x,
                    y: 178.0,
                }),
            }
        }
    }

    /// `mnPlayers1PGameMakeGate` and `...MakeNameAndEmblem`.
    fn visit_gate(&self, put: &mut impl FnMut(vs::Draw)) {
        let p = usize::from(self.player).min(3);
        put(vs::Draw::Sprite(Piece {
            lut: Some(GATE_LUTS[p]),
            ..piece(FILE_PLAYERS_1P_MODE, mode::RED_CARD, 25.0, 127.0)
        }));
        put(vs::Draw::Sprite(Piece {
            prim: Some([0; 3]),
            ..piece(
                FILE_PLAYERS_COMMON,
                common::TEXT_1P[p],
                KIND_TEXT_X[p] + 25.0,
                132.0,
            )
        }));
        let Some(kind) = self.view.name_made.filter(|_| self.view.name_shown) else {
            return;
        };
        let k = (kind as usize).min(11);
        put(vs::Draw::Sprite(Piece {
            prim: Some([0; 3]),
            ..piece(FILE_EMBLEMS, vs::EMBLEMS[k], 35.0, 144.0)
        }));
        put(vs::Draw::Sprite(piece(
            FILE_PLAYERS_COMMON,
            common::NAMES[k],
            33.0,
            202.0,
        )));
    }
}

/// An option's arrows at row `y`: left at x 194, right at 269.
fn visit_arrows(a: &Arrows, y: f32, f: &mut impl FnMut(Draw)) {
    if a.hidden {
        return;
    }
    if a.left {
        f(Draw::Select(vs::Draw::Sprite(piece(
            FILE_PLAYERS_COMMON,
            common::ARROW_L,
            194.0,
            y,
        ))));
    }
    if a.right {
        f(Draw::Select(vs::Draw::Sprite(piece(
            FILE_PLAYERS_COMMON,
            common::ARROW_R,
            269.0,
            y,
        ))));
    }
}

/// `mnPlayers1PGameMakeHiScore`, `...MakeBonusCount`, `...MakeTotalHiScore`
/// and `...MakeTotalBonusCount`.
fn visit_records(r: &Records, put: &mut impl FnMut(vs::Draw)) {
    const TEXT: [u8; 3] = [0x7E, 0x7C, 0x77];
    const NUMBER: ([u8; 3], [u8; 3]) = ([0; 3], [0x7E, 0x7C, 0x77]);
    const BONUS: ([u8; 3], [u8; 3]) = ([0; 3], [0x40, 0x6F, 0xCD]);
    let mut sprite = |p: Piece| put(vs::Draw::Sprite(p));
    let parens = |y: f32, sprite: &mut dyn FnMut(Piece)| {
        for (offset, x) in [
            (mode::OPENING_PARENTHESIS, 258.0),
            (mode::CLOSING_PARENTHESIS, 286.0),
        ] {
            sprite(Piece {
                prim: Some(BONUS.1),
                ..piece(FILE_PLAYERS_1P_MODE, offset, x, y)
            });
        }
    };
    // `mnPlayers1PGameMakeTotalRecord`.
    string("TOTAL HIGH SCORE", 109.0, 211.0, TEXT, &mut sprite);
    number(r.total_hiscore, 256.0, 208.0, NUMBER, 9, true, &mut sprite);
    parens(209.0, &mut sprite);
    number(r.total_bonuses, 285.0, 208.0, BONUS, 3, true, &mut sprite);
    // `mnPlayers1PGameMakeFighterRecord`.
    let Some((hiscore, bonuses, best)) = r.fighter else {
        return;
    };
    string("HIGH SCORE", 142.0, 201.0, TEXT, &mut sprite);
    number(hiscore, 256.0, 198.0, NUMBER, 8, true, &mut sprite);
    if best != 0 {
        sprite(Piece {
            prim: Some(LEVEL_COLORS[usize::from(best - 1).min(4)]),
            ..piece(FILE_PLAYERS_1P_MODE, mode::SMASH_LOGO, 126.0, 198.0)
        });
    }
    parens(199.0, &mut sprite);
    number(bonuses, 285.0, 198.0, BONUS, 3, true, &mut sprite);
}

/// `mnPlayers1PGameGetNumberDigitCount`.
fn digit_count(number: u32, max: u32) -> u32 {
    (1..=max)
        .rev()
        .find(|&n| number / 10u32.pow(n - 1) != 0)
        .unwrap_or(0)
}

/// `mnPlayers1PGameMakeNumber`: `IFCommonDigits` right-aligned on `x`,
/// 8 pixels apart, `colors` the environment then the primitive. A fixed
/// count draws leading zeroes.
pub(crate) fn number(
    number: u32,
    x: f32,
    y: f32,
    (env, prim): ([u8; 3], [u8; 3]),
    max: u32,
    fixed: bool,
    sprite: &mut impl FnMut(Piece),
) {
    // `number` is an `s32` in the source; the records fit.
    let number = number.min(i32::MAX as u32);
    let count = if fixed { max } else { digit_count(number, max) };
    let mut left = x;
    for i in 0..count.max(1) {
        let digit = if i == 0 {
            number
        } else {
            number.checked_div(10u32.saturating_pow(i)).unwrap_or(0)
        };
        left -= 8.0;
        sprite(Piece {
            prim: Some(prim),
            env,
            ..piece(FILE_DIGITS, DIGITS[(digit % 10) as usize], left, y)
        });
    }
}

/// `mnPlayers1PGameGetCharacterID`.
pub(crate) fn character_id(c: u8) -> usize {
    match c {
        b'\'' => 0x1A,
        b'%' => 0x1B,
        b'.' => 0x1C,
        b'A'..=b'Z' => usize::from(c - b'A'),
        _ => 0x1D,
    }
}

/// `mnPlayers1PGameGetCharacterSpacing`: the kerning after `s[i]`.
pub(crate) fn character_spacing(s: &[u8], i: usize) -> f32 {
    let next = s.get(i + 1).copied().unwrap_or(0);
    let tight = match s[i] {
        b'A' => matches!(next, b'F' | b'P' | b'T' | b'V' | b'Y'),
        b'F' | b'P' | b'V' | b'Y' => matches!(next, b'A' | b'T'),
        b'Q' | b'T' => !matches!(next, b'\'' | b'.'),
        b'\'' | b'.' => false,
        _ => next == b'T',
    };
    if tight {
        0.0
    } else {
        1.0
    }
}

/// `mnPlayers1PGameMakeString`: `MNCommonFonts` glyphs from `x`; a digit
/// advances by its value and a space by 3.
fn string(s: &str, x: f32, y: f32, color: [u8; 3], sprite: &mut impl FnMut(Piece)) {
    let s = s.as_bytes();
    let mut at = x;
    for (i, &c) in s.iter().enumerate() {
        if c.is_ascii_digit() {
            at += f32::from(c - b'0');
            continue;
        }
        if c == b' ' {
            at += 3.0;
            continue;
        }
        let id = character_id(c);
        let Some(&offset) = FONT.get(id) else {
            // ID 0x1D has no sprite; the source never draws one.
            continue;
        };
        let dy = match c {
            b'\'' => -1.0,
            b'.' => 4.0,
            _ => 0.0,
        };
        sprite(Piece {
            prim: Some(color),
            ..piece(FILE_FONTS, offset, at, y + dy)
        });
        at += FONT_WIDTHS[id] + character_spacing(s, i);
    }
}

#[cfg(test)]
#[path = "players_1p_layer_tests.rs"]
mod tests;
