//! The VS results screen's 2D layer, from `mn/mnvsmode/mnvsresults.c`:
//! the wallpaper (`mnVSResultsMakeWallpaper`), its fades
//! (`mnVSResultsMakeWallpaperTint`, `...WallpaperTint2`), the players' tags
//! (`mnVSResultsMakePlayerTag`), the winner's name and "WINS!"
//! (`mnVSResultsMakeResultsText`), the darkening tint
//! (`mnVSResultsMakeTint`) and the table (`mnVSResultsMakeLabel` and the
//! `mnVSResultsDrawResults*` procs it runs).
//!
//! [`Layer::tick`] runs a tic's updates and then the display procs' steps
//! (the fades' alphas and the bar's width move once per drawn frame).
//! [`Layer::visit`] yields what draws, back to front in the cameras'
//! order: the wallpaper (camera priority 80), `WallpaperTint2` (70), the
//! emblem (60, not ported), `WallpaperTint` (55), the fighters (50), the
//! player tags (30), the results text (20), the tint (17) and the table
//! (15). The sprites name the files `dMNVSResultsFileIDs` loads; the PSP
//! layer looks each up in the pack.

use crate::d_arrays as d;
use crate::fighter::FighterKind;
use crate::results::{Kind, Results};
use crate::results_scene::{count_place, distance_id, present_count, present_lower_count, spot};
use crate::rng;

/// The sprite files the results use, each an `ssb_rom::sprite` list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpriteFile {
    /// `ssb_rom::sprite::VS_RESULTS`, file 34.
    VsResults,
    /// `ssb_rom::sprite::GAME_MODES`, file 18.
    GameModes,
    /// `ssb_rom::sprite::DIGITS`, file 36.
    Digits,
    /// `ssb_rom::sprite::PLAYER_DAMAGE`, file 164.
    PlayerDamage,
    /// `ssb_rom::sprite::ANNOUNCE_COMMON`, file 37.
    Announce,
    /// `ssb_rom::sprite::PLAYER_TAGS`, file 38.
    PlayerTags,
}

/// A sprite: an entry of one of the lists, or a fighter's stock icon in a
/// costume (`fp->attr->sprites->stock_sprite` through `stock_luts`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sprite {
    In(SpriteFile, u8),
    Stock { kind: FighterKind, costume: u8 },
}

/// [`SpriteFile::VsResults`] entries.
pub const TKO_TEXT: u8 = 0;
pub const PLACE_TEXT: u8 = 1;
pub const KOS_TEXT: u8 = 2;
pub const PTS_TEXT: u8 = 3;
/// `1PArrowSprite`; 2P to 4P follow.
pub const ARROW_1P: u8 = 4;
pub const WALLPAPER: u8 = 8;
pub const WINNER: u8 = 9;
/// [`SpriteFile::Digits`]: `Digits0` to `Digits9`, then the dash.
pub const DASH: u8 = 10;
/// [`SpriteFile::PlayerTags`]: `1P` to `4P`, then `CP`.
pub const TAG_CP: u8 = 4;

/// One `SObj`: its sprite, top-left corner on the 320 x 240 screen,
/// `sprite.scalex` and colours. Every one clears `SP_FASTCOPY` and sets
/// `SP_TRANSPARENT`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Piece {
    pub sprite: Sprite,
    pub x: f32,
    pub y: f32,
    pub scale_x: f32,
    /// `sprite.red`, `.green`, `.blue`; `None` keeps the ROM's.
    pub prim: Option<[u8; 3]>,
    /// `sobj->envcolor`. The source leaves it unset on the row labels, the
    /// label, the TKO row's dash and the stock icons; the port reads that
    /// as black.
    pub env: [u8; 3],
}

/// What draws, back to front.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Draw {
    /// `mnVSResultsWallpaperProcDisplay`: the I4 wallpaper at (10, 10)
    /// through `(PRIMITIVE - ENVIRONMENT) * TEXEL0 + ENVIRONMENT`, opaque.
    Wallpaper {
        prim: [u8; 3],
        env: [u8; 3],
    },
    /// A filled rectangle, `[x0, y0, x1, y1)` in screen pixels, blended by
    /// its alpha.
    Fill {
        rect: [f32; 4],
        color: [u8; 4],
    },
    Sprite(Piece),
    /// The series emblem under its camera (priority 60, RE-420).
    Emblem,
    /// The fighters under their camera, with the particles of its DL links
    /// (the confetti, RE-420).
    Fighters,
}

/// A player's entry (`gSCManagerTransferBattleState.players[]`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Player {
    pub kind: FighterKind,
    pub costume: u8,
    /// `pkind == nFTPlayerKindMan`.
    pub human: bool,
    /// `color`, the player tag's colour id.
    pub color: u8,
}

/// The tint rectangles' extent: the cameras' viewport, (10, 10) to
/// (310, 230), in `G_CYC_1CYCLE`.
pub const VIEWPORT_RECT: [f32; 4] = [10.0, 10.0, 310.0, 230.0];

/// `mnVSResultsMakeWallpaper`'s `colors`: environment, then primitive.
const WALLPAPER_COLORS: [[[u8; 3]; 2]; 4] = d::MN_VS_RESULTS_MAKE_WALLPAPER_COLORS;

/// `mnVSResultsMakeString`'s `colors`: `prim` (the environment) and `env`
/// (the primitive).
const STRING_COLORS: [[[u8; 3]; 2]; 5] = d::MN_VS_RESULTS_MAKE_STRING_COLORS;

/// `mnVSResultsMakeString`'s `widths`, `A` to `Z`, `!` and `.`.
const LETTER_WIDTHS: [f32; 28] = d::MN_VS_RESULTS_MAKE_STRING_WIDTHS;

/// `mnVSResultsSetNumberColor`'s `colors`: the environment (`prim`), then
/// the primitive (`env`).
const NUMBER_COLORS: [[[u8; 3]; 2]; 5] = d::MN_VS_RESULTS_SET_NUMBER_COLOR_COLORS;

/// Colour channel tables (`dIFCommonPlayerTagPrimColors{R,G,B}`) as RGB.
const fn rgb(r: [u8; 5], g: [u8; 5], b: [u8; 5]) -> [[u8; 3]; 5] {
    let mut out = [[0u8; 3]; 5];
    let mut i = 0;
    while i < 5 {
        out[i] = [r[i], g[i], b[i]];
        i += 1;
    }
    out
}

/// `dIFCommonPlayerTagPrimColors{R,G,B}` (`if/ifcommon.c`); the
/// environment colours are all black.
const TAG_COLORS: [[u8; 3]; 5] = rgb(
    crate::d_arrays::IF_COMMON_PLAYER_TAG_PRIM_COLORS_R,
    crate::d_arrays::IF_COMMON_PLAYER_TAG_PRIM_COLORS_G,
    crate::d_arrays::IF_COMMON_PLAYER_TAG_PRIM_COLORS_B,
);

/// `mnVSResultsMakeWinnerText`'s US string (`wins`).
const WINS: &str = "W1I1N1S1!";

/// One piece of the table, made on a tic by the label's proc.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Row {
    /// `mnVSResultsMakeTint`.
    Tint,
    /// `mnVSResultsMakeHeader`: the arrows and stock icons.
    Header,
    /// `mnVSResultsMakeKOs(y)`.
    Kos(i32),
    /// `mnVSResultsMakeTKO(y)`.
    Tko(i32),
    /// `mnVSResultsMakeBar(y)`.
    Bar(i32),
    /// `mnVSResultsMakePointsRow`, at y 104.
    Points,
    /// `mnVSResultsMakePlaceRow(y)`.
    Place(i32),
}

/// `mnVSResultsDrawResultsTimeRoyal` (and `...TimeTeam`, identical).
const TIME_ROWS: [(u32, Row); 7] = [
    (180, Row::Tint),
    (210, Row::Header),
    (210, Row::Kos(66)),
    (230, Row::Tko(81)),
    (250, Row::Bar(98)),
    (270, Row::Points),
    (290, Row::Place(124)),
];
/// `mnVSResultsDrawResultsStockRoyal` (and `...StockTeam`, identical).
const STOCK_ROWS: [(u32, Row); 5] = [
    (180, Row::Tint),
    (210, Row::Place(66)),
    (230, Row::Bar(110)),
    (250, Row::Header),
    (250, Row::Kos(124)),
];
/// `mnVSResultsDrawResultsNoContest`.
const NO_CONTEST_ROWS: [(u32, Row); 4] = [
    (30, Row::Tint),
    (60, Row::Header),
    (60, Row::Kos(66)),
    (80, Row::Tko(81)),
];

/// The label's proc for the results' kind (`mnVSResultsMakeLabel`'s
/// `procs`): each row and the tic it is made on.
pub fn rows(kind: Kind) -> &'static [(u32, Row)] {
    match kind {
        Kind::TimeRoyal | Kind::TimeTeam => &TIME_ROWS,
        Kind::StockRoyal | Kind::StockTeam => &STOCK_ROWS,
        Kind::NoContest => &NO_CONTEST_ROWS,
    }
}

/// The layer's own state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Layer {
    /// The wallpaper's colour, once `sMNVSResultsDrawWallpaperTic` made it.
    pub wallpaper: Option<usize>,
    /// `sMNVSResultsWallpaperTintAlpha`: from `mnVSResultsFuncStart`, 0xFF
    /// down by 5 a frame.
    pub wallpaper_tint_alpha: i32,
    /// `sMNVSResultsWallpaperTint2Alpha`, made with the wallpaper after a
    /// contest.
    pub wallpaper_tint2_alpha: Option<i32>,
    /// `sMNVSResultsTintAlpha`, up by 9 a frame to 0x80.
    pub tint_alpha: Option<u32>,
    /// `mnVSResultsMakeBar`'s y, once made.
    pub bar: Option<i32>,
    /// `sMNVSResultsBarWidth`, up by 10 a frame to 190.
    pub bar_width: i32,
}

impl Layer {
    /// `mnVSResultsFuncStart`'s `mnVSResultsMakeWallpaperTint` and
    /// `mnVSResultsInitVars`'s bar width.
    pub fn new() -> Layer {
        Layer {
            wallpaper: None,
            wallpaper_tint_alpha: 0xFF,
            wallpaper_tint2_alpha: None,
            tint_alpha: None,
            bar: None,
            bar_width: 0,
        }
    }

    /// One tic, after [`Results::tick`] and before the fighters are made:
    /// `mnVSResultsFuncRun`'s wallpaper, the label proc's tint and bar,
    /// then the display procs' steps for the frame drawn after them.
    pub fn tick(&mut self, r: &Results) {
        let t = r.total_tics;
        if t == r.draw_wallpaper_tic {
            if r.kind != Kind::NoContest {
                self.wallpaper_tint2_alpha = Some(0xFF);
            }
            self.wallpaper = Some(wallpaper_color(r));
        }
        if t > r.make_results_tic {
            for &(tic, row) in rows(r.kind) {
                if tic != t {
                    continue;
                }
                match row {
                    Row::Tint => self.tint_alpha = Some(0),
                    Row::Bar(y) => self.bar = Some(y),
                    _ => {}
                }
            }
        }
        // `mnVSResultsWallpaperTintProcDisplay` and `...Tint2...`: 0xFF is
        // a multiple of 5, so the alpha stops at 0 and the eject never runs.
        let fade = |a: &mut i32| {
            if *a > 0 {
                *a -= 5;
            }
        };
        fade(&mut self.wallpaper_tint_alpha);
        if let Some(a) = self.wallpaper_tint2_alpha.as_mut() {
            fade(a);
        }
        // `mnVSResultsTintProcDisplay`.
        if let Some(a) = self.tint_alpha.as_mut() {
            if *a < 0x80 {
                *a = (*a + 9).min(0x80);
            }
        }
        // `mnVSResultsBarProcDisplay`.
        if self.bar.is_some() {
            self.bar_width = (self.bar_width + 10).min(190);
        }
    }

    /// Everything that draws this frame, back to front.
    pub fn visit(&self, r: &Results, players: &[Option<Player>; 4], mut f: impl FnMut(Draw)) {
        let t = r.total_tics;
        if let Some(c) = self.wallpaper {
            let [env, prim] = WALLPAPER_COLORS[c];
            f(Draw::Wallpaper { prim, env });
        }
        let tint = |alpha: i32| Draw::Fill {
            rect: VIEWPORT_RECT,
            color: [0, 0, 0, alpha.clamp(0, 0xFF) as u8],
        };
        if let Some(a) = self.wallpaper_tint2_alpha.filter(|&a| a > 0) {
            f(tint(a));
        }
        // `mnVSResultsMakeEmblemCamera` (60) draws between the two fades'
        // cameras (70 and 55).
        f(Draw::Emblem);
        if self.wallpaper_tint_alpha > 0 {
            f(tint(self.wallpaper_tint_alpha));
        }
        f(Draw::Fighters);
        if t >= r.init_fighters_all_tic {
            for player in (0..4).filter(|&i| r.present[i]) {
                if let Some(p) = players[player] {
                    f(Draw::Sprite(player_tag(r, player, p)));
                }
            }
        }
        if t < r.make_results_tic {
            return;
        }
        results_text(r, players, &mut f);
        if let Some(a) = self.tint_alpha {
            f(Draw::Fill {
                rect: VIEWPORT_RECT,
                color: [0, 0, 0, a as u8],
            });
        }
        // `mnVSResultsLabelProcDisplay`: the white rule, then the label.
        f(Draw::Fill {
            rect: [32.0, 42.0, 283.0, 45.0],
            color: [0xFF; 4],
        });
        f(Draw::Sprite(Piece {
            sprite: Sprite::In(SpriteFile::GameModes, u8::from(r.is_team_battle)),
            x: 32.0,
            y: 29.0,
            scale_x: 1.0,
            prim: Some([0xFF; 3]),
            env: [0; 3],
        }));
        for &(tic, row) in rows(r.kind) {
            if tic > t || tic <= r.make_results_tic {
                continue;
            }
            match row {
                Row::Tint => {}
                Row::Header => header(r, players, &mut f),
                Row::Kos(y) => {
                    f(label(KOS_TEXT, 26.0, y as f32));
                    numbers(r, y as f32, &r.kos, &mut f);
                }
                Row::Tko(y) => {
                    f(label(TKO_TEXT, 26.0, y as f32));
                    if r.kind != Kind::NoContest {
                        f(Draw::Sprite(Piece {
                            sprite: Sprite::In(SpriteFile::Digits, DASH),
                            x: 90.0,
                            y: (y + 3) as f32,
                            scale_x: 1.0,
                            prim: None,
                            env: [0; 3],
                        }));
                    }
                    numbers(r, y as f32, &r.tko, &mut f);
                }
                Row::Bar(y) => {
                    // `G_CYC_FILL` covers both corners.
                    let y = y as f32;
                    f(Draw::Fill {
                        rect: [87.0, y, 88.0 + self.bar_width as f32, y + 1.0],
                        color: [0xFF; 4],
                    });
                }
                Row::Points => {
                    f(label(PTS_TEXT, 26.0, 104.0));
                    numbers(r, 104.0, &r.points, &mut f);
                }
                Row::Place(y) => place_row(r, y as f32, &mut f),
            }
        }
    }
}

impl Default for Layer {
    fn default() -> Layer {
        Layer::new()
    }
}

/// `mnVSResultsMakeWallpaper`'s colour: a random one with no contest
/// (`syUtilsRandIntRange(GMCOMMON_PLAYERS_MAX)`), the winner's port, or
/// the winning team's colour.
fn wallpaper_color(r: &Results) -> usize {
    if r.kind == Kind::NoContest {
        return rng::rand_int_range(4) as usize;
    }
    let winner = r.winner.unwrap_or(0);
    if r.is_team_battle {
        team_color(d::MN_VS_RESULTS_MAKE_WALLPAPER_TEAM_COLORS, r, winner)
    } else {
        winner.min(3)
    }
}

/// `colors[players[player].team]`, for one of the three `{ 0, 1, 3 }`
/// tables that turn a team into a colour: `mnVSResultsMakeWallpaper`'s
/// `team_colors`, `mnVSResultsGetNumberColorID`'s `color_ids` and
/// `mnVSResultsMakeEmblem`'s `colors`.
pub(crate) fn team_color(colors: [i32; 3], r: &Results, player: usize) -> usize {
    colors
        .get(usize::from(r.team[player]))
        .map_or(0, |&c| c as usize)
}

/// `mnVSResultsGetColumnX`.
pub fn column_x(r: &Results, player: usize) -> f32 {
    let lower = present_lower_count(r, player);
    match present_count(r) {
        2 => d::MN_VS_RESULTS_GET_COLUMN_X_COLUMN_X_2P[lower.min(1)],
        3 => d::MN_VS_RESULTS_GET_COLUMN_X_COLUMN_X_3P[lower.min(2)],
        _ => d::MN_VS_RESULTS_GET_COLUMN_X_COLUMN_X_4P[lower.min(3)],
    }
}

/// `mnVSResultsGetNumberColorID`: white, or the team's colour.
pub fn number_color_id(r: &Results, player: usize) -> usize {
    if r.is_team_battle {
        team_color(d::MN_VS_RESULTS_GET_NUMBER_COLOR_ID_COLOR_IDS, r, player)
    } else {
        4
    }
}

/// A piece coloured by `mnVSResultsSetNumberColor`.
fn numbered(sprite: Sprite, x: f32, y: f32, color: usize) -> Piece {
    Piece {
        sprite,
        x,
        y,
        scale_x: 1.0,
        prim: Some(NUMBER_COLORS[color][1]),
        env: NUMBER_COLORS[color][0],
    }
}

/// `mnVSResultsMakeNumber`: a dash for a negative number, then the
/// hundreds and tens digits when they show, and the ones digit, 8 pixels
/// apart from `x + 8`.
pub fn make_number(x: f32, y: f32, number: i32, color: usize, f: &mut impl FnMut(Draw)) {
    // `mnVSResultsGetHundredsDigit`, `...TensDigit`, `...OnesDigit`: C's
    // truncating division, negated for a negative number.
    let hundreds = (number / 100).abs();
    let tens = ((number % 100) / 10).abs();
    let ones = ((number % 100) % 10).abs();
    let digit = |d: i32| Sprite::In(SpriteFile::Digits, d as u8);
    if number < 0 {
        let dx = if hundreds != 0 {
            0.0
        } else if tens != 0 {
            8.0
        } else {
            16.0
        };
        f(Draw::Sprite(numbered(
            Sprite::In(SpriteFile::Digits, DASH),
            x + dx,
            y + 3.0,
            color,
        )));
    }
    if hundreds != 0 {
        f(Draw::Sprite(numbered(digit(hundreds), x + 8.0, y, color)));
    }
    if tens != 0 || hundreds != 0 {
        f(Draw::Sprite(numbered(digit(tens), x + 16.0, y, color)));
    }
    f(Draw::Sprite(numbered(digit(ones), x + 24.0, y, color)));
}

/// A row's label (`lbCommonMakeSpriteGObj`, `sprite.red` etc. white).
fn label(sprite: u8, x: f32, y: f32) -> Draw {
    Draw::Sprite(Piece {
        sprite: Sprite::In(SpriteFile::VsResults, sprite),
        x,
        y,
        scale_x: 1.0,
        prim: Some([0xFF; 3]),
        env: [0; 3],
    })
}

/// Each present player's number in its column.
fn numbers(r: &Results, y: f32, values: &[i32; 4], f: &mut impl FnMut(Draw)) {
    for (player, &n) in values.iter().enumerate() {
        if r.present[player] {
            make_number(column_x(r, player), y, n, number_color_id(r, player), f);
        }
    }
}

/// `mnVSResultsGetDisplayPlace`: 1-based, except that a four-player
/// free-for-all with two in second shows the next place as 4th.
pub fn display_place(r: &Results, player: usize) -> i32 {
    if present_count(r) == 4 && !r.is_team_battle && count_place(r, 1) == 2 && r.places[player] == 2
    {
        4
    } else {
        r.places[player] + 1
    }
}

/// `mnVSResultsMakePlaceRow`: the label at (10, y), then each present
/// player's place (`mnVSResultsMakePlaceNumber`) placed by
/// `mnVSResultsSetPlacePosition`.
fn place_row(r: &Results, y: f32, f: &mut impl FnMut(Draw)) {
    f(label(PLACE_TEXT, 10.0, y));
    for player in (0..4).filter(|&i| r.present[i]) {
        let place = display_place(r, player);
        let color = number_color_id(r, player);
        let x = column_x(r, player);
        // First place is the "winner" plate, except for a team battle's
        // players who are neither the winner nor a shared winner.
        let plate = place == 1
            && (!r.is_team_battle || r.winner == Some(player) || r.shared_winner[player]);
        let piece = if plate {
            Piece {
                sprite: Sprite::In(SpriteFile::VsResults, WINNER),
                x: x + 2.0,
                y,
                scale_x: 1.0,
                prim: None,
                env: [0; 3],
            }
        } else {
            let digit = place.clamp(0, 9) as u8;
            numbered(
                Sprite::In(SpriteFile::PlayerDamage, digit),
                x + 15.0,
                y,
                color,
            )
        };
        f(Draw::Sprite(piece));
    }
}

/// `mnVSResultsMakeHeader`: each present player's arrow at its column
/// plus 17, y 49, white, and its stock icon 10 pixels to the left.
fn header(r: &Results, players: &[Option<Player>; 4], f: &mut impl FnMut(Draw)) {
    for player in (0..4).filter(|&i| r.present[i]) {
        let x = column_x(r, player) + 17.0;
        f(Draw::Sprite(Piece {
            sprite: Sprite::In(SpriteFile::VsResults, ARROW_1P + player as u8),
            x,
            y: 49.0,
            scale_x: 1.0,
            prim: Some([0xFF; 3]),
            env: [0; 3],
        }));
        if let Some(p) = players[player] {
            f(Draw::Sprite(Piece {
                sprite: Sprite::Stock {
                    kind: p.kind,
                    costume: p.costume,
                },
                x: x - 10.0,
                y: 49.0,
                scale_x: 1.0,
                prim: None,
                env: [0; 3],
            }));
        }
    }
}

/// `mnVSResultsMakePlayerTag` and `mnVSResultsSetPlayerTagPosition`: the
/// port's tag for a human, "CP" for a computer, in the player's colour,
/// above its fighter.
pub fn player_tag(r: &Results, player: usize, p: Player) -> Piece {
    let spot = spot(r, player).min(3);
    let dist = distance_id(r, player);
    let [x, y] = match present_count(r) {
        2 => d::MN_VS_RESULTS_SET_PLAYER_TAG_POSITION_POS_XY_2P[dist.min(1)][spot],
        3 => d::MN_VS_RESULTS_SET_PLAYER_TAG_POSITION_POS_XY_3P[dist.min(2)][spot],
        _ => d::MN_VS_RESULTS_SET_PLAYER_TAG_POSITION_POS_XY_4P[dist.min(3)][spot],
    };
    let y = y + d::MN_VS_RESULTS_SET_PLAYER_TAG_POSITION_POS_Y_KINDS
        .get(p.kind as usize)
        .map_or(0.0, |row| row[dist.min(3)]);
    let tag = if p.human { player as u8 } else { TAG_CP };
    Piece {
        sprite: Sprite::In(SpriteFile::PlayerTags, tag),
        x,
        y,
        scale_x: 1.0,
        prim: Some(TAG_COLORS[usize::from(p.color).min(4)]),
        env: [0; 3],
    }
}

/// `mnVSResultsGetCharacterID`: `A` to `Z`, then `!`, `.` and space.
pub fn character_id(c: u8) -> i32 {
    match c {
        b'!' => 0x1A,
        b'.' => 0x1B,
        b' ' => 0x1C,
        _ => i32::from(c) - i32::from(b'A'),
    }
}

/// `mnVSResultsMakeString`: each letter at the running x, advanced by its
/// width times `scale`; a digit in the string advances x by that many
/// pixels, a space by 10 times `scale`, and a period sits 26 lower. The
/// scale is `sprite.scalex`: the letters narrow but keep their height.
pub fn make_string(s: &str, x: f32, y: f32, color: usize, scale: f32, f: &mut impl FnMut(Draw)) {
    let mut cx = x;
    let [env, prim] = STRING_COLORS[color];
    for c in s.bytes() {
        if c.is_ascii_digit() {
            cx += f32::from(c - b'0');
            continue;
        }
        let id = character_id(c);
        if id == 0x1C {
            cx += 10.0 * scale;
            continue;
        }
        let Some(&width) = usize::try_from(id).ok().and_then(|i| LETTER_WIDTHS.get(i)) else {
            continue;
        };
        f(Draw::Sprite(Piece {
            sprite: Sprite::In(SpriteFile::Announce, id as u8),
            x: cx,
            y: if id == 0x1B { y + 26.0 } else { y },
            scale_x: scale,
            prim: Some(prim),
            env,
        }));
        cx += width * scale;
    }
}

/// `mnVSResultsMakeResultsText`: "NO CONTEST", or the winner's name in red
/// (a team's in its colour) and the purple "WINS!" after it.
fn results_text(r: &Results, players: &[Option<Player>; 4], f: &mut impl FnMut(Draw)) {
    if r.kind == Kind::NoContest {
        make_string("NO CONTEST", 30.0, 180.0, 4, 1.0, f);
        return;
    }
    let Some(winner) = r.winner else {
        return;
    };
    if r.is_team_battle {
        let team = usize::from(r.team[winner]).min(2);
        let name = d::MN_VS_RESULT_MAKE_TEAM_NAME_NAMES[team];
        make_string(
            name,
            d::MN_VS_RESULT_MAKE_TEAM_NAME_POS_X[team],
            180.0,
            team,
            1.0,
            f,
        );
        make_string(
            WINS,
            d::MN_VS_RESULTS_MAKE_WINNER_TEXT_X_TEAMS[team],
            180.0,
            3,
            1.0,
            f,
        );
    } else if let Some(p) = players[winner] {
        let kind = (p.kind as usize).min(11);
        let name = d::MN_VS_RESULT_MAKE_FIGHTER_NAME_NAMES[kind];
        let x = d::MN_VS_RESULT_MAKE_FIGHTER_NAME_POS_X[kind];
        make_string(
            name,
            x,
            180.0,
            0,
            d::MN_VS_RESULT_MAKE_FIGHTER_NAME_SCALES[kind],
            f,
        );
        make_string(
            WINS,
            d::MN_VS_RESULTS_MAKE_WINNER_TEXT_X_FKINDS[kind],
            180.0,
            3,
            1.0,
            f,
        );
    }
}

#[cfg(test)]
#[path = "results_layer_tests.rs"]
mod tests;
