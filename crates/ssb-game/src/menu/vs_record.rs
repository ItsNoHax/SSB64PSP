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
use crate::players_1p::layer::{character_id, character_spacing, FONT_WIDTHS};
use crate::sound::{self, id};
use ssb_engine::input::N64Buttons;

/// `llMNVSRecordMain*Sprite` (`reloc_data.us.h`).
pub mod sprite {
    pub const QUESTION: u32 = 0x70;
    pub const LABEL_TOTAL: u32 = 0x258;
    /// `mnVSRecordMakeDigits`'s `offsets`.
    pub const DIGITS: [u32; 10] = crate::d_arrays::MN_VS_RECORD_MAKE_DIGITS_OFFSETS;
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
    /// `mnVSRecordMakeColumnIcons`'s `offsets` (`*IconBWSprite`), by
    /// `FTKind`.
    pub const ICONS_BW: [u32; 12] = crate::d_arrays::MN_VS_RECORD_MAKE_COLUMN_ICONS_OFFSETS;
    /// `mnVSRecordMakeRowIcons`'s `offsets` (`*IconColorSprite`), by
    /// `FTKind`.
    pub const ICONS_COLOR: [u32; 12] = crate::d_arrays::MN_VS_RECORD_MAKE_ROW_ICONS_OFFSETS;
    pub const PORTRAIT_WALLPAPER: u32 = 0x4D30;
    pub const LABEL: u32 = 0x5428;
    pub const SYMBOL_COLON: u32 = 0x54C0;

    /// `mnVSRecordMakePortraitStats`'s `offsets`
    /// (`llMNPlayersPortraits*Sprite`), by `FTKind`.
    pub const PORTRAITS: [u32; 12] = crate::d_arrays::MN_VS_RECORD_MAKE_PORTRAIT_STATS_OFFSETS;
}

/// `nMNVSRecordKind*`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Page {
    BattleScore,
    Ranking,
    Indiv,
}

/// `dMNVSRecordRankingColumnWidths`.
const COLUMN_WIDTHS: [i32; 7] = crate::d_arrays::MN_VS_RECORD_RANKING_COLUMN_WIDTHS;
/// `nMNVSRecordRankingKindEnumCount`.
const COLUMNS: usize = 7;

/// `mnVSRecordGetFighterKindByIndex`'s `fkinds`: the starting order.
const FKINDS_BY_INDEX: [usize; 12] = {
    let fkinds = crate::d_arrays::MN_VS_RECORD_GET_FIGHTER_KIND_BY_INDEX_FKINDS;
    let mut out = [0; 12];
    let mut i = 0;
    while i < 12 {
        out[i] = fkinds[i] as usize;
        i += 1;
    }
    out
};

/// A `u32` colour table `{ env r, g, b, prim r, g, b }` as the
/// environment and primitive colours.
const fn env_prim(c: [u32; 6]) -> ([u8; 3], [u8; 3]) {
    (
        [c[0] as u8, c[1] as u8, c[2] as u8],
        [c[3] as u8, c[4] as u8, c[5] as u8],
    )
}

/// `mnVSRecordMakeBattleScoreTableValues`'s `colors`.
const BATTLE_SCORE_COLORS: ([u8; 3], [u8; 3]) =
    env_prim(crate::d_arrays::MN_VS_RECORD_MAKE_BATTLE_SCORE_TABLE_VALUES_COLORS);
/// `mnVSRecordMakeRankingTableValues`'s `colors`.
const RANKING_COLORS: ([u8; 3], [u8; 3]) =
    env_prim(crate::d_arrays::MN_VS_RECORD_MAKE_RANKING_TABLE_VALUES_COLORS);
/// `mnVSRecordMakeIndivTableValues`'s `colors`.
const INDIV_COLORS: ([u8; 3], [u8; 3]) =
    env_prim(crate::d_arrays::MN_VS_RECORD_MAKE_INDIV_TABLE_VALUES_COLORS);
/// `mnVSRecordMakePortraitStats`'s `digit_colors`.
const PORTRAIT_DIGIT_COLORS: ([u8; 3], [u8; 3]) =
    env_prim(crate::d_arrays::MN_VS_RECORD_MAKE_PORTRAIT_STATS_DIGIT_COLORS);
/// `mnVSRecordMakePortraitStats`'s `string_colors`.
const PORTRAIT_STRING_COLOR: [u8; 3] = {
    let c = crate::d_arrays::MN_VS_RECORD_MAKE_PORTRAIT_STATS_STRING_COLORS;
    [c[0] as u8, c[1] as u8, c[2] as u8]
};
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
        // `mnVSRecordMakeString`'s `offsets`.
        let Some(&offset) = crate::d_arrays::MN_VS_RECORD_MAKE_STRING_OFFSETS.get(id) else {
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
        sound::play_bgm(0, id::nSYAudioBGMData);
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
                sound::play_fgm(id::nSYAudioFGMBurnS);
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
            sound::play_fgm(id::nSYAudioFGMBurnS);
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
                sound::play_fgm(id::nSYAudioFGMFoxFoot);
                self.step_index(backup, false);
                self.change_wait = r.wait_p(7);
            }
            if r.check(self.change_wait, pad, DOWN, true, -20, false) {
                sound::play_fgm(id::nSYAudioFGMFoxFoot);
                self.step_index(backup, true);
                self.change_wait = r.wait_n(7);
            }
            if r.check(self.change_wait, pad, RIGHT, false, 20, true) {
                sound::play_fgm(id::nSYAudioFGMFoxFoot);
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
                sound::play_fgm(id::nSYAudioFGMFoxFoot);
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
                sound::play_fgm(id::nSYAudioFGMFoxFoot);
                self.step_index(backup, true);
                self.make_stats(backup);
                self.change_wait = if r.is_button {
                    12
                } else {
                    super::wait_p(20, 7)
                };
            }
            if r.check(self.change_wait, pad, LEFT, false, -20, false) {
                sound::play_fgm(id::nSYAudioFGMFoxFoot);
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
        let subtitle =
            crate::d_arrays::MN_VS_RECORD_SUBTITLE_PROC_UPDATE_OFFSETS[self.page as usize];
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
                // `mnVSRecordMakeRankingTableHeaders`.
                let labels = crate::d_arrays::MN_VS_RECORD_MAKE_RANKING_TABLE_HEADERS_OFFSETS;
                const PADDING: [i32; 7] =
                    crate::d_arrays::MN_VS_RECORD_MAKE_RANKING_TABLE_HEADERS_X_PADDING;
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
                // `mnVSRecordMakeIndivPortraitAll`.
                let labels = crate::d_arrays::MN_VS_RECORD_MAKE_INDIV_PORTRAIT_ALL_OFFSETS;
                let at = crate::d_arrays::MN_VS_RECORD_MAKE_INDIV_PORTRAIT_ALL_POSITIONS;
                for (offset, [x, y]) in labels.into_iter().zip(at) {
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
        const OFFSETS: [[f32; 2]; 12] =
            crate::d_arrays::MN_VS_RECORD_SET_ICON_POSITION_FOR_COLUMN_OFFSETS;
        for (column, &fkind) in order.iter().enumerate() {
            let left = x + width * column as f32;
            if s.have(fkind) {
                let [dx, dy] = OFFSETS[fkind];
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
        const OFFSETS: [[f32; 2]; 12] = crate::d_arrays::MN_VS_RECORD_SET_ROW_ICON_POSITION_OFFSETS;
        let (x, y) = (25.0, 62.0);
        for (row, &fkind) in order.iter().enumerate() {
            let top = y + (row * 13) as f32;
            if s.have(fkind) {
                let [dx, dy] = OFFSETS[fkind];
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
                        BATTLE_SCORE_COLORS,
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
                BATTLE_SCORE_COLORS,
                false,
                false,
                6,
                false,
            );
        }
    }

    /// `mnVSRecordMakeRankingTableValues`.
    fn visit_ranking_values(&self, s: &Stats<'_>, backup: &Backup, f: &mut impl FnMut(Draw)) {
        const WIDTHS: [i32; 7] = crate::d_arrays::MN_VS_RECORD_MAKE_RANKING_TABLE_VALUES_COL_WIDTHS;
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
                        RANKING_COLORS,
                        true,
                        false,
                        3,
                        false,
                    ),
                    1 => digits(
                        f,
                        s.kos(fkind),
                        at,
                        y,
                        RANKING_COLORS,
                        false,
                        false,
                        6,
                        false,
                    ),
                    2 => digits(
                        f,
                        s.tko(fkind),
                        at,
                        y,
                        RANKING_COLORS,
                        false,
                        false,
                        6,
                        false,
                    ),
                    3 => digits(
                        f,
                        (s.sd_percent(fkind) * 10.0) as i32,
                        at,
                        y,
                        RANKING_COLORS,
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
                            RANKING_COLORS,
                            false,
                            false,
                            2,
                            true,
                        );
                        f(Draw::Sprite(
                            Piece::clear(FILE_VS_RECORD, sprite::SYMBOL_COLON, at - 11.0, y)
                                .env(RANKING_COLORS.0)
                                .prim(RANKING_COLORS.1),
                        ));
                        digits(
                            f,
                            (time / 3600) as i32,
                            at - 13.0,
                            y,
                            RANKING_COLORS,
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
                        RANKING_COLORS,
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
                        RANKING_COLORS,
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
        const Y: [f32; 4] = crate::d_arrays::MN_VS_RECORD_MAKE_INDIV_TABLE_VALUES_Y;
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
                INDIV_COLORS,
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
                INDIV_COLORS,
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
                INDIV_COLORS,
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
                INDIV_COLORS,
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
        let colors = PORTRAIT_DIGIT_COLORS;
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
        string(f, "RANKING", 150.0, 60.0, PORTRAIT_STRING_COLOR);
        digits(f, 12, 265.0, 58.0, colors, false, true, 2, false);
        f(Draw::Sprite(
            Piece::clear(FILE_VS_RECORD, sprite::SYMBOL_SLASH, 251.0, 59.0)
                .prim(PORTRAIT_STRING_COLOR),
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
        string(f, "USED %", 150.0, 68.0, PORTRAIT_STRING_COLOR);
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
        string(f, "ATTACK 3TOTAL", 149.0, 78.0, PORTRAIT_STRING_COLOR);
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
        string(f, "DAMAGE TOTAL", 150.0, 86.0, PORTRAIT_STRING_COLOR);
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
