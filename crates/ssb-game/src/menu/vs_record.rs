//! `mn/mndata/mnvsrecord.c`: VS Record. Three pages, A and START forward,
//! B back (and out to Data from the first):
//!
//! - Battle Score: who KO'd whom, sorted by KOs, with each row's total.
//! - Ranking: seven columns (Win %, KOs, TKO, SD %, Time, Use %, Avg),
//!   sorted by the first; left and right turn the columns, up and down
//!   move the highlighted row.
//! - Individual: the highlighted fighter's portrait, ranking, use, damage
//!   given and taken, and their record against each fighter; left and
//!   right change the fighter.
//!
//! The page's values are made when the page is (`mnVSRecordMakeStats`);
//! the backup cannot change while the scene runs, so [`VsRecordMenu::visit`]
//! reads them from it each frame. Locked newcomers show a question mark
//! and no values.

use super::{
    data_common, fill, fill_prim, Draw, Pad, Piece, Repeat, Scene, FILE_DATA_COMMON, FILE_FONTS,
    FILE_PORTRAITS, FILE_VS_RECORD,
};
use crate::backup::{Backup, VsRecord};
use crate::players_1p::layer::{character_id, character_spacing, FONT, FONT_WIDTHS};
use ssb_engine::input::N64Buttons;

/// `llMNVSRecordMain*Sprite` (`reloc_data.us.h`).
pub mod sprite {
    pub const QUESTION: u32 = 0x70;
    pub const LABEL_TOTAL: u32 = 0x258;
    pub const DIGITS: [u32; 10] = [
        0x2F0, 0x390, 0x430, 0x4D0, 0x570, 0x610, 0x6B0, 0x750, 0x7F0, 0x890,
    ];
    pub const SYMBOL_POINT: u32 = 0x910;
    pub const LABEL_WIN_PERCENT: u32 = 0xA08;
    pub const LABEL_KOS: u32 = 0xAF8;
    pub const LABEL_TKO: u32 = 0xBE8;
    pub const LABEL_SD_PERCENT: u32 = 0xCD8;
    pub const LABEL_TIME: u32 = 0xE10;
    pub const LABEL_USE_PERCENT: u32 = 0xF08;
    pub const LABEL_AVG: u32 = 0x1008;
    pub const LABEL_KOD: u32 = 0x1140;
    pub const SYMBOL_SLASH: u32 = 0x11D0;
    pub const UNKNOWN1: u32 = 0x1318;
    pub const UNKNOWN2: u32 = 0x1458;
    pub const BATTLE_SCORE: u32 = 0x15D0;
    pub const DOWN_ARROWS: u32 = 0x1668;
    pub const SIDE_ARROWS: u32 = 0x17A8;
    /// `*IconBWSprite`, by `FTKind`.
    pub const ICONS_BW: [u32; 12] = [
        0x1918, 0x1A98, 0x1CA8, 0x1E88, 0x2008, 0x2370, 0x2178, 0x2540, 0x2930, 0x2B30, 0x27C8,
        0x2698,
    ];
    /// `*IconColorSprite`, by `FTKind`.
    pub const ICONS_COLOR: [u32; 12] = [
        0x2D18, 0x2EF8, 0x3198, 0x3438, 0x3618, 0x3A38, 0x37F8, 0x3CD8, 0x4308, 0x45A8, 0x4098,
        0x3EB8,
    ];
    pub const PORTRAIT_WALLPAPER: u32 = 0x4D30;
    pub const LABEL: u32 = 0x5428;
    pub const SYMBOL_COLON: u32 = 0x54C0;

    /// `llMNPlayersPortraits*Sprite`, by `FTKind`.
    pub const PORTRAITS: [u32; 12] = [
        0x4728, 0xD068, 0x8BC8, 0xAE18, 0x6978, 0x11508, 0x13758, 0x19E48, 0xF2B8, 0x159A8,
        0x1C098, 0x17BF8,
    ];
}

/// `nMNVSRecordKind*`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Page {
    BattleScore,
    Ranking,
    Indiv,
}

/// `dMNVSRecordRankingColumnWidths`.
const COLUMN_WIDTHS: [i32; 7] = [33, 33, 33, 33, 46, 35, 34];
/// `nMNVSRecordRankingKindEnumCount`.
const COLUMNS: usize = 7;

/// `mnVSRecordGetFighterKindByIndex`: the starting order.
const FKINDS_BY_INDEX: [usize; 12] = [0, 2, 5, 3, 6, 8, 1, 9, 4, 7, 11, 10];

/// The values' colours: environment black, primitive `E5D199`.
const VALUE_COLORS: ([u8; 3], [u8; 3]) = ([0; 3], [0xE5, 0xD1, 0x99]);
/// The labels' grey.
const GREY: [u8; 3] = [0x8A, 0x88, 0x92];
/// The arrows' orange.
const ORANGE: [u8; 3] = [0xE3, 0x7D, 0x0C];

/// The menu's state (`sMNVSRecord*`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VsRecordMenu {
    pub page: Page,
    is_change_subtitle: bool,
    /// `sMNVSRecordBattleScoreFighterKinds`, `...RankingFighterKindOrder`
    /// and `...IndivFighterKinds`.
    pub battle_score_order: [usize; 12],
    pub ranking_order: [usize; 12],
    pub indiv_order: [usize; 12],
    pub current_index: usize,
    fighter_mask: u16,
    /// `sMNVSRecordFirstColumn`.
    pub first_column: usize,
    change_wait: i32,
}

/// The backup's records with the scene's fighter mask: the source's stat
/// helpers.
struct Stats<'a> {
    records: &'a [VsRecord; 12],
    mask: u16,
}

impl Stats<'_> {
    /// `mnVSRecordCheckHaveFighterKind`.
    fn have(&self, fkind: usize) -> bool {
        match fkind {
            4 | 7 | 10 | 11 => self.mask & (1 << fkind) != 0,
            _ => true,
        }
    }

    /// `mnVSRecordGetKOs`.
    fn kos(&self, fkind: usize) -> i32 {
        (0..12)
            .filter(|&i| self.have(i))
            .map(|i| i32::from(self.records[fkind].ko_count[i]))
            .sum()
    }

    /// `mnVSRecordGetTKO`.
    fn tko(&self, fkind: usize) -> i32 {
        let kod: i32 = (0..12)
            .filter(|&i| self.have(i))
            .map(|i| i32::from(self.records[i].ko_count[fkind]))
            .sum();
        (i32::from(self.records[fkind].selfdestructs) + kod).min(9999)
    }

    /// `mnVSRecordGetTotalTKO`.
    fn total_tko(&self) -> i32 {
        (0..12).filter(|&i| self.have(i)).map(|i| self.tko(i)).sum()
    }

    /// `mnVSRecordGetWinPercent`.
    fn win_percent(&self, fkind: usize) -> f32 {
        let kos = self.kos(fkind) as f32;
        let tko = self.total_tko() as f32;
        (if tko != 0.0 { kos / tko } else { 0.0 }) * 100.0
    }

    /// `mnVSRecordGetAvg`.
    fn avg(&self, fkind: usize) -> f32 {
        let r = &self.records[fkind];
        if r.games_played != 0 {
            f32::from(r.player_count_tally) / f32::from(r.games_played)
        } else {
            0.0
        }
    }

    /// `mnVSRecordGetGamesPlayedSum`.
    fn games_played_sum(&self) -> i32 {
        self.records.iter().map(|r| i32::from(r.games_played)).sum()
    }

    /// `mnVSRecordGetUsePercent`.
    fn use_percent(&self, fkind: usize) -> f32 {
        let sum = self.games_played_sum();
        let ratio = if sum as f32 != 0.0 {
            f32::from(self.records[fkind].games_played) / sum as f32
        } else {
            0.0
        };
        ratio * 100.0
    }

    /// `mnVSRecordGetSDPercent`.
    fn sd_percent(&self, fkind: usize) -> f32 {
        let tko = self.tko(fkind) as f32;
        let sd = f32::from(self.records[fkind].selfdestructs);
        (if tko != 0.0 { sd / tko } else { 0.0 }) * 100.0
    }

    /// `mnVSRecordGetWinPercentAgainst`.
    fn win_percent_against(&self, this: usize, against: usize) -> f32 {
        let kos_for = f32::from(self.records[this].ko_count[against]);
        let total = kos_for + f32::from(self.records[against].ko_count[this]);
        (if total != 0.0 { kos_for / total } else { 0.0 }) * 100.0
    }

    /// `mnVSRecordGetAvgAgainst`.
    fn avg_against(&self, this: usize, against: usize) -> f32 {
        let r = &self.records[this];
        if r.played_against[against] != 0 {
            f32::from(r.player_count_tallies[against]) / f32::from(r.played_against[against])
        } else {
            0.0
        }
    }

    /// The source's sort: `fkinds` from [`FKINDS_BY_INDEX`], a locked
    /// fighter sinking past every unlocked one, ties kept.
    fn sort(&self, stats: &[f64; 12]) -> [usize; 12] {
        let mut fkinds = FKINDS_BY_INDEX;
        for i in 0..12 {
            for j in i + 1..12 {
                if !self.have(fkinds[i])
                    || (stats[fkinds[i]] < stats[fkinds[j]] && self.have(fkinds[j]))
                {
                    fkinds.swap(i, j);
                }
            }
        }
        fkinds
    }

    /// `mnVSRecordGetRanking`.
    fn ranking(&self, fkind: usize) -> i32 {
        let stats: [f64; 12] = core::array::from_fn(|i| f64::from(self.win_percent(i)));
        let fkinds = self.sort(&stats);
        let mut rank = [0i32; 12];
        let mut current = 1;
        for i in 0..12 {
            rank[fkinds[i]] = current;
            if i != 11 && stats[fkinds[i]] != stats[fkinds[i + 1]] {
                current = i as i32 + 2;
            }
        }
        rank[fkind]
    }
}

/// `mnVSRecordGetDigitCount`.
fn digit_count(number: i32, max: i32) -> i32 {
    let mut count = max;
    while count > 0 {
        if number / 10i32.pow(count as u32 - 1) != 0 {
            return count;
        }
        count -= 1;
    }
    0
}

/// `mnVSRecordMakeDigits`: right-aligned on `x`, 4 pixels apart (5 when
/// wide); a tenths digit and its point come first when shown.
#[allow(clippy::too_many_arguments)]
fn digits(
    f: &mut impl FnMut(Draw),
    number: i32,
    x: f32,
    y: f32,
    (env, prim): ([u8; 3], [u8; 3]),
    mut show_tenths: bool,
    wide: bool,
    max: i32,
    fixed: bool,
) {
    let mut number = number.max(0);
    let step = if wide { 5.0 } else { 4.0 };
    let mut at = x;
    let digit = |f: &mut dyn FnMut(Draw), d: i32, at: f32, y: f32| {
        f(Draw::Sprite(
            Piece::clear(FILE_VS_RECORD, sprite::DIGITS[(d % 10) as usize], at, y)
                .env(env)
                .prim(prim),
        ));
    };
    if show_tenths && number == 1000 {
        show_tenths = false;
        number /= 10;
    }
    if show_tenths {
        let decimal = number % 10;
        number /= 10;
        at -= step;
        digit(f, decimal, at, y);
        at -= if wide { 3.0 } else { 2.0 };
        f(Draw::Sprite(
            Piece::clear(FILE_VS_RECORD, sprite::SYMBOL_POINT, at, y + 4.0)
                .env(env)
                .prim(prim),
        ));
    }
    at -= step;
    digit(f, number, at, y);
    let count = if fixed { max } else { digit_count(number, max) };
    for i in 1..count {
        at -= step;
        digit(f, number / 10i32.pow(i as u32), at, y);
    }
}

/// `mnVSRecordMakeString`: `MNCommonFonts` glyphs from `x`; a digit
/// advances by its value and a space by 4.
fn string(f: &mut impl FnMut(Draw), s: &str, x: f32, y: f32, color: [u8; 3]) {
    let s = s.as_bytes();
    let mut at = x;
    for (i, &c) in s.iter().enumerate() {
        if c.is_ascii_digit() {
            at += f32::from(c - b'0');
            continue;
        }
        if c == b' ' {
            at += 4.0;
            continue;
        }
        let id = character_id(c);
        let Some(&offset) = FONT.get(id) else {
            continue;
        };
        let dy = match c {
            b'\'' => -1.0,
            b'.' => 4.0,
            _ => 0.0,
        };
        f(Draw::Sprite(
            Piece::clear(FILE_FONTS, offset, at, y + dy).prim(color),
        ));
        at += FONT_WIDTHS[id] + character_spacing(s, i);
    }
}

/// `mnVSRecordMakeRankingTableValues`' and `...Headers`' column order from
/// `first`.
fn column_order(first: usize) -> [usize; COLUMNS] {
    core::array::from_fn(|i| (first + i) % COLUMNS)
}

impl VsRecordMenu {
    /// `mnVSRecordFuncStart`: `mnVSRecordInitVars`, then the Battle Score
    /// page (`mnVSRecordMakeStats`).
    pub fn new(backup: &Backup) -> VsRecordMenu {
        let mut m = VsRecordMenu {
            page: Page::BattleScore,
            is_change_subtitle: false,
            battle_score_order: [0; 12],
            ranking_order: [0; 12],
            indiv_order: [0; 12],
            current_index: 0,
            fighter_mask: backup.fighter_mask,
            first_column: 0,
            change_wait: 0,
        };
        m.make_stats(backup);
        m
    }

    fn stats<'a>(&self, backup: &'a Backup) -> Stats<'a> {
        Stats {
            records: &backup.vs_records,
            mask: self.fighter_mask,
        }
    }

    /// `mnVSRecordSortData` for the page (`mnVSRecordMakeStats`).
    fn make_stats(&mut self, backup: &Backup) {
        let s = self.stats(backup);
        match self.page {
            Page::BattleScore => {
                let stats = core::array::from_fn(|i| f64::from(s.kos(i)));
                self.battle_score_order = s.sort(&stats);
            }
            Page::Ranking => {
                let stats = core::array::from_fn(|i| match self.first_column {
                    0 => f64::from(s.win_percent(i)),
                    1 => f64::from(s.kos(i)),
                    2 => f64::from(s.tko(i)),
                    3 => f64::from(s.sd_percent(i)),
                    4 => f64::from(backup.vs_records[i].time_used),
                    5 => f64::from(s.use_percent(i)),
                    _ => f64::from(s.avg(i)),
                });
                self.ranking_order = s.sort(&stats);
            }
            Page::Indiv => {
                let this = self.ranking_order[self.current_index];
                let stats = core::array::from_fn(|i| f64::from(s.win_percent_against(this, i)));
                self.indiv_order = s.sort(&stats);
            }
        }
    }

    /// The highlighted fighter's next unlocked row, `step` +1 or -1.
    fn step_index(&mut self, backup: &Backup, forward: bool) {
        let s = self.stats(backup);
        loop {
            self.current_index = if forward {
                if self.current_index == 11 {
                    0
                } else {
                    self.current_index + 1
                }
            } else if self.current_index == 0 {
                11
            } else {
                self.current_index - 1
            };
            if s.have(self.ranking_order[self.current_index]) {
                break;
            }
        }
    }

    /// `mnVSRecordFuncRun`, then the subtitle's and arrows' processes.
    pub fn tick(&mut self, pad: &Pad, backup: &Backup) -> Option<Scene> {
        use super::{DOWN, LEFT, RIGHT, UP};
        let mut load = None;
        if self.change_wait != 0 {
            self.change_wait -= 1;
        }
        if self.page == Page::Indiv && pad.released(RIGHT | UP | LEFT | DOWN) {
            self.change_wait = 0;
        }
        self.is_change_subtitle = false;
        if pad.tap(N64Buttons::B) {
            if self.page == Page::BattleScore {
                load = Some(Scene::Data);
            } else {
                self.page = if self.page == Page::Indiv {
                    Page::Ranking
                } else {
                    Page::BattleScore
                };
                self.is_change_subtitle = true;
                self.make_stats(backup);
            }
        }
        if pad.tap(N64Buttons::A | N64Buttons::START) && self.page < Page::Indiv {
            self.page = if self.page == Page::BattleScore {
                Page::Ranking
            } else {
                Page::Indiv
            };
            self.is_change_subtitle = true;
            self.make_stats(backup);
        }
        let mut r = Repeat::default();
        if self.page == Page::Ranking {
            if r.check(self.change_wait, pad, UP, true, 20, true) {
                self.step_index(backup, false);
                self.change_wait = r.wait_p(7);
            }
            if r.check(self.change_wait, pad, DOWN, true, -20, false) {
                self.step_index(backup, true);
                self.change_wait = r.wait_n(7);
            }
            if r.check(self.change_wait, pad, RIGHT, false, 20, true) {
                self.first_column = if self.first_column == 0 {
                    COLUMNS - 1
                } else {
                    self.first_column - 1
                };
                self.is_change_subtitle = true;
                self.make_stats(backup);
                self.change_wait = r.wait_p(7);
            }
            if r.check(self.change_wait, pad, LEFT, false, -20, false) {
                self.first_column = if self.first_column == COLUMNS - 1 {
                    0
                } else {
                    self.first_column + 1
                };
                self.is_change_subtitle = true;
                self.make_stats(backup);
                self.change_wait = r.wait_n(7);
            }
        }
        if self.page == Page::Indiv {
            if r.check(self.change_wait, pad, RIGHT, false, 20, true) {
                self.step_index(backup, true);
                self.make_stats(backup);
                self.change_wait = if r.is_button {
                    12
                } else {
                    super::wait_p(20, 7)
                };
            }
            if r.check(self.change_wait, pad, LEFT, false, -20, false) {
                self.step_index(backup, false);
                self.make_stats(backup);
                self.change_wait = if r.is_button {
                    12
                } else {
                    super::wait_n(-20, 7)
                };
            }
        }
        load
    }

    /// The cameras back to front: labels and arrows (80), the ranking
    /// highlight (70), the grid (60), the values (40), the headers (20).
    pub fn visit(&self, backup: &Backup, f: &mut impl FnMut(Draw)) {
        let s = self.stats(backup);
        // `mnVSRecordMakeLabels`.
        f(Draw::Sprite(
            Piece::clear(FILE_DATA_COMMON, data_common::DATA_HEADER, 24.0, 17.0)
                .prim([0x5F, 0x58, 0x46]),
        ));
        f(Draw::Sprite(
            Piece::clear(FILE_VS_RECORD, sprite::LABEL, 99.0, 23.0)
                .env([0; 3])
                .prim([0xF2, 0xC7, 0x0D]),
        ));
        // `mnVSRecordSubtitleProcUpdate`'s `offsets`.
        let subtitle = match self.page {
            Page::BattleScore => sprite::BATTLE_SCORE,
            Page::Ranking => sprite::UNKNOWN2,
            Page::Indiv => sprite::UNKNOWN1,
        };
        f(Draw::Sprite(
            Piece::clear(FILE_VS_RECORD, subtitle, 222.0, 28.0).prim([0; 3]),
        ));
        if self.page == Page::Indiv {
            f(Draw::Sprite(
                Piece::clear(FILE_DATA_COMMON, data_common::ARROW_L, 40.0, 78.0).prim(ORANGE),
            ));
            f(Draw::Sprite(
                Piece::clear(FILE_DATA_COMMON, data_common::ARROW_R, 105.0, 78.0).prim(ORANGE),
            ));
        }
        if self.page != Page::Indiv {
            f(Draw::Sprite(
                Piece::clear(FILE_VS_RECORD, sprite::DOWN_ARROWS, 281.0, 39.0).prim(ORANGE),
            ));
        }
        if self.page == Page::Ranking {
            f(Draw::Sprite(
                Piece::clear(FILE_VS_RECORD, sprite::SIDE_ARROWS, 25.0, 47.0).prim(ORANGE),
            ));
            // `mnVSRecordRankingHighlightProcDisplay`.
            let y = 62 + self.current_index as i32 * 13;
            f(fill_prim(24, y, 295, y + 12, [0x27, 0x00, 0xFF, 0xFF]));
        }
        self.visit_grid(f);
        match self.page {
            Page::BattleScore => {
                self.visit_battle_score_values(&s, backup, f);
                f(Draw::Sprite(
                    Piece::clear(FILE_VS_RECORD, sprite::LABEL_TOTAL, 264.0, 50.0).prim(GREY),
                ));
                self.visit_row_icons(&s, &self.battle_score_order, f);
                self.visit_column_icons(&s, &self.battle_score_order, 18.0, (49.0, 49.0), f);
            }
            Page::Ranking => {
                self.visit_ranking_values(&s, backup, f);
                let labels = [
                    sprite::LABEL_WIN_PERCENT,
                    sprite::LABEL_KOS,
                    sprite::LABEL_TKO,
                    sprite::LABEL_SD_PERCENT,
                    sprite::LABEL_TIME,
                    sprite::LABEL_USE_PERCENT,
                    sprite::LABEL_AVG,
                ];
                const PADDING: [i32; 7] = [2, 2, 2, 4, 4, 3, 1];
                let mut x = 48;
                for c in column_order(self.first_column) {
                    f(Draw::Sprite(
                        Piece::clear(FILE_VS_RECORD, labels[c], (PADDING[c] + x) as f32, 49.0)
                            .prim(GREY),
                    ));
                    x += COLUMN_WIDTHS[c];
                }
                self.visit_row_icons(&s, &self.ranking_order, f);
            }
            Page::Indiv => {
                self.visit_indiv_values(&s, backup, f);
                let labels = [
                    (sprite::LABEL_WIN_PERCENT, 29.0, 159.0),
                    (sprite::LABEL_KOS, 28.0, 171.0),
                    (sprite::LABEL_KOD, 25.0, 183.0),
                    (sprite::LABEL_AVG, 26.0, 195.0),
                ];
                for (offset, x, y) in labels {
                    f(Draw::Sprite(
                        Piece::clear(FILE_VS_RECORD, offset, x, y).prim(GREY),
                    ));
                }
                self.visit_column_icons(&s, &self.indiv_order, 19.0, (66.0, 145.0), f);
                self.visit_portrait_stats(&s, backup, self.ranking_order[self.current_index], f);
            }
        }
    }

    /// `mnVSRecordTableGridProcDisplay`.
    fn visit_grid(&self, f: &mut impl FnMut(Draw)) {
        let c = [0x62, 0x62, 0x6A];
        match self.page {
            Page::BattleScore => {
                for i in 0..14 {
                    f(fill(24, 48 + i * 13, 295, 48 + i * 13, c));
                }
                f(fill(24, 48, 24, 217, c));
                for i in 0..14 {
                    let x = if i == 13 { i * 18 + 61 } else { i * 18 + 48 };
                    f(fill(x, 48, x, 217, c));
                }
            }
            Page::Ranking => {
                f(fill(24, 46, 295, 46, c));
                for i in 0..13 {
                    f(fill(24, 61 + i * 13, 295, 61 + i * 13, c));
                }
                f(fill(24, 46, 24, 217, c));
                let mut x = 48;
                let ids = column_order(self.first_column);
                for i in 0..=ids.len() {
                    f(fill(x, 46, x, 217, c));
                    if let Some(&id) = ids.get(i) {
                        x += COLUMN_WIDTHS[id];
                    }
                }
            }
            Page::Indiv => {
                f(fill(26, 144, 293, 144, c));
                for i in 0..5 {
                    f(fill(26, 157 + i * 12, 293, 157 + i * 12, c));
                }
                f(fill(26, 144, 26, 205, c));
                for i in 0..13 {
                    f(fill(65 + i * 19, 144, 65 + i * 19, 205, c));
                }
            }
        }
    }

    /// `mnVSRecordMakeLockedIcon`.
    fn locked_icon(x: f32, y: f32) -> Draw {
        Draw::Sprite(Piece::clear(FILE_VS_RECORD, sprite::QUESTION, x, y).prim(GREY))
    }

    /// `mnVSRecordMakeColumnIcons` (`mnVSRecordSetIconPositionForColumn`).
    fn visit_column_icons(
        &self,
        s: &Stats<'_>,
        order: &[usize; 12],
        width: f32,
        (x, y): (f32, f32),
        f: &mut impl FnMut(Draw),
    ) {
        const OFFSETS: [(f32, f32); 12] = [
            (1.0, -5.0),
            (1.0, -6.0),
            (0.0, -6.0),
            (0.0, -4.0),
            (1.0, -6.0),
            (0.0, -5.0),
            (1.0, -5.0),
            (0.0, -3.0),
            (0.0, 1.0),
            (0.0, -5.0),
            (0.0, -1.0),
            (0.0, -2.0),
        ];
        for (column, &fkind) in order.iter().enumerate() {
            let left = x + width * column as f32;
            if s.have(fkind) {
                let (dx, dy) = OFFSETS[fkind];
                f(Draw::Sprite(Piece::clear(
                    FILE_VS_RECORD,
                    sprite::ICONS_BW[fkind],
                    left + dx,
                    y + dy,
                )));
            } else {
                f(Self::locked_icon(left + 5.0, y));
            }
        }
    }

    /// `mnVSRecordMakeRowIcons` (`mnVSRecordSetRowIconPosition`).
    fn visit_row_icons(&self, s: &Stats<'_>, order: &[usize; 12], f: &mut impl FnMut(Draw)) {
        const OFFSETS: [(f32, f32); 12] = [
            (5.0, 0.0),
            (5.0, 0.0),
            (0.0, 0.0),
            (0.0, 0.0),
            (3.0, 0.0),
            (5.0, 0.0),
            (5.0, 0.0),
            (1.0, 0.0),
            (0.0, 1.0),
            (0.0, 0.0),
            (4.0, 0.0),
            (3.0, 0.0),
        ];
        let (x, y) = (25.0, 62.0);
        for (row, &fkind) in order.iter().enumerate() {
            let top = y + (row * 13) as f32;
            if s.have(fkind) {
                let (dx, dy) = OFFSETS[fkind];
                f(Draw::Sprite(Piece::clear(
                    FILE_VS_RECORD,
                    sprite::ICONS_COLOR[fkind],
                    x + dx,
                    top + dy,
                )));
            } else {
                f(Self::locked_icon(x + 8.0, top));
            }
        }
    }

    /// `mnVSRecordMakeBattleScoreTableValues`.
    fn visit_battle_score_values(&self, s: &Stats<'_>, backup: &Backup, f: &mut impl FnMut(Draw)) {
        let order = &self.battle_score_order;
        for (i, &row) in order.iter().enumerate() {
            if !s.have(row) {
                continue;
            }
            let x = 66.0;
            let y = (i * 13) as f32;
            for (j, &column) in order.iter().enumerate() {
                if s.have(column) {
                    let kos = i32::from(backup.vs_records[row].ko_count[column]);
                    digits(
                        f,
                        kos,
                        x + (j * 18) as f32,
                        y + 65.0,
                        VALUE_COLORS,
                        false,
                        false,
                        4,
                        false,
                    );
                }
            }
            digits(
                f,
                s.kos(row),
                x + 216.0 + 10.0,
                y + 65.0,
                VALUE_COLORS,
                false,
                false,
                6,
                false,
            );
        }
    }

    /// `mnVSRecordMakeRankingTableValues`.
    fn visit_ranking_values(&self, s: &Stats<'_>, backup: &Backup, f: &mut impl FnMut(Draw)) {
        const WIDTHS: [i32; 7] = [27, 30, 30, 23, 35, 27, 39];
        let columns = column_order(self.first_column);
        for (i, &fkind) in self.ranking_order.iter().enumerate() {
            if !s.have(fkind) {
                continue;
            }
            let mut x = 48;
            let y = (i * 13) as f32 + 65.0;
            for c in columns {
                let at = (WIDTHS[c] + x) as f32;
                match c {
                    0 => digits(
                        f,
                        (s.win_percent(fkind) * 10.0) as i32,
                        at,
                        y,
                        VALUE_COLORS,
                        true,
                        false,
                        3,
                        false,
                    ),
                    1 => digits(f, s.kos(fkind), at, y, VALUE_COLORS, false, false, 6, false),
                    2 => digits(f, s.tko(fkind), at, y, VALUE_COLORS, false, false, 6, false),
                    3 => digits(
                        f,
                        (s.sd_percent(fkind) * 10.0) as i32,
                        at,
                        y,
                        VALUE_COLORS,
                        true,
                        false,
                        3,
                        false,
                    ),
                    4 => {
                        let time = backup.vs_records[fkind].time_used;
                        digits(
                            f,
                            ((time % 3600) / 60) as i32,
                            at,
                            y,
                            VALUE_COLORS,
                            false,
                            false,
                            2,
                            true,
                        );
                        f(Draw::Sprite(
                            Piece::clear(FILE_VS_RECORD, sprite::SYMBOL_COLON, at - 11.0, y)
                                .env(VALUE_COLORS.0)
                                .prim(VALUE_COLORS.1),
                        ));
                        digits(
                            f,
                            (time / 3600) as i32,
                            at - 13.0,
                            y,
                            VALUE_COLORS,
                            false,
                            false,
                            3,
                            false,
                        );
                    }
                    5 => digits(
                        f,
                        (s.use_percent(fkind) * 10.0) as i32,
                        at,
                        y,
                        VALUE_COLORS,
                        true,
                        false,
                        3,
                        false,
                    ),
                    _ => digits(
                        f,
                        (s.avg(fkind) * 10.0) as i32,
                        at - 15.0,
                        y,
                        VALUE_COLORS,
                        true,
                        false,
                        1,
                        false,
                    ),
                }
                x += COLUMN_WIDTHS[c];
            }
        }
    }

    /// `mnVSRecordMakeIndivTableValues`.
    fn visit_indiv_values(&self, s: &Stats<'_>, backup: &Backup, f: &mut impl FnMut(Draw)) {
        const Y: [f32; 4] = [160.0, 172.0, 184.0, 196.0];
        let this = self.ranking_order[self.current_index];
        for i in 0..12 {
            // The source tests the ranking order here, not the column's.
            if !s.have(self.ranking_order[i]) {
                continue;
            }
            let against = self.indiv_order[i];
            let x = (i * 19) as f32 + 84.0;
            let r = &backup.vs_records;
            digits(
                f,
                (s.win_percent_against(this, against) * 10.0) as i32,
                x,
                Y[0],
                VALUE_COLORS,
                true,
                false,
                3,
                false,
            );
            digits(
                f,
                i32::from(r[this].ko_count[against]),
                x,
                Y[1],
                VALUE_COLORS,
                false,
                false,
                4,
                false,
            );
            digits(
                f,
                i32::from(r[against].ko_count[this]),
                x,
                Y[2],
                VALUE_COLORS,
                false,
                false,
                4,
                false,
            );
            digits(
                f,
                (s.avg_against(this, against) * 10.0) as i32,
                x,
                Y[3],
                VALUE_COLORS,
                true,
                false,
                3,
                false,
            );
        }
    }

    /// `mnVSRecordMakePortraitStats`.
    fn visit_portrait_stats(
        &self,
        s: &Stats<'_>,
        backup: &Backup,
        fkind: usize,
        f: &mut impl FnMut(Draw),
    ) {
        let colors = ([0; 3], GREY);
        f(Draw::Sprite(Piece::clear(
            FILE_VS_RECORD,
            sprite::PORTRAIT_WALLPAPER,
            52.0,
            55.0,
        )));
        f(Draw::Sprite(Piece::clear(
            FILE_PORTRAITS,
            sprite::PORTRAITS[fkind],
            57.0,
            60.0,
        )));
        string(f, "RANKING", 150.0, 60.0, GREY);
        digits(f, 12, 265.0, 58.0, colors, false, true, 2, false);
        f(Draw::Sprite(
            Piece::clear(FILE_VS_RECORD, sprite::SYMBOL_SLASH, 251.0, 59.0).prim(GREY),
        ));
        digits(
            f,
            s.ranking(fkind),
            250.0,
            58.0,
            colors,
            false,
            true,
            2,
            false,
        );
        string(f, "USED %", 150.0, 68.0, GREY);
        digits(
            f,
            (s.use_percent(fkind) * 10.0) as i32,
            265.0,
            66.0,
            colors,
            true,
            true,
            3,
            false,
        );
        string(f, "ATTACK 3TOTAL", 149.0, 78.0, GREY);
        let r = &backup.vs_records[fkind];
        digits(
            f,
            r.damage_given as i32,
            265.0,
            76.0,
            colors,
            false,
            true,
            6,
            false,
        );
        string(f, "DAMAGE TOTAL", 150.0, 86.0, GREY);
        digits(
            f,
            r.damage_taken as i32,
            265.0,
            84.0,
            colors,
            false,
            true,
            6,
            false,
        );
    }
}
