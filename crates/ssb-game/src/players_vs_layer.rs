//! The VS character select's presentation, from `mn/mnplayers/mnplayersvs.c`
//! (RE-411): the wallpaper and top bar (`mnPlayersVSMakeWallpaper`,
//! `mnPlayersVSMakeLabels`, the time or stock setting), the portraits and
//! their slide-in (`mnPlayersVSMakePortraitAll`,
//! `mnPlayersVSGetNextPortraitX`), the portrait flash, the four panels
//! (`mnPlayersVSMakeGate`: the card, its shutter doors, the "1P"/"CP" text,
//! the HMN/CP/NA button, the name and emblem, the team button, the CPU level
//! or handicap with its blinking arrows), the pucks with their glow, the
//! cursors, the "ready to fight" banner, and the fighters' turn and demo
//! status (`mnPlayersVSFighterProcUpdate`).
//!
//! [`View`] is the state the source keeps in GObjs and SObjs. The logic in
//! [`PlayersVs`] updates it at the source's call sites (the `v_*` hooks
//! below); [`PlayersVs::tick`] then runs the display GObjs' own processes
//! (`view_tick`). [`PlayersVs::visit`] yields what draws, back
//! to front in the cameras' order.
//!
//! Every camera has the viewport (10, 10) to (310, 230), and a higher
//! camera priority draws first: the default camera's black clear (100),
//! the wallpaper and top bar (80, link 26), the portraits' fire backgrounds
//! (75, link 36), the flashes (73, link 37), the portraits (70, link 27),
//! the cards, "1P" texts, names, emblems and "Press Start" (50, link 28),
//! the team buttons (45, link 34), the levels (43, link 35), the doors
//! (40, link 29), the HMN/CP/NA buttons (35, link 30), the fighters (30),
//! the pucks (25, link 33), the cursors and held pucks (20, link 32) and
//! the banner (10, link 38).

use crate::battle::{Rule, TIMELIMIT_INFINITE};
use crate::fighter::FighterKind;
use crate::fighter_select::{is_locked, portrait, CursorStatus, PORTRAIT_KINDS};
use crate::results_scene::{DemoStatus, FIGHTER_SCALES};

use super::{DrawKey, Handicap, PlayerKind, PlayersVs, DL_CURSOR, DL_PUCK, PLAYERS, TEAM_GREEN};

/// `dMNPlayersVSFileIDs`' files, by `llMN*FileID` (`reloc_data.us.h`).
pub const FILE_PLAYERS_COMMON: u32 = 17;
pub const FILE_MN_COMMON: u32 = 0;
pub const FILE_EMBLEMS: u32 = 20;
pub const FILE_SELECT_COMMON: u32 = 21;
pub const FILE_GAME_MODES: u32 = 18;
pub const FILE_PORTRAITS: u32 = 19;

/// `llMNPlayersCommon*` offsets.
pub(crate) mod common {
    pub const TEXT_1P: [u32; 4] = [0x878, 0xA58, 0xC38, 0xE18];
    pub const TEXT_CP: u32 = 0xFF8;
    pub const HANDICAP_TEXT: u32 = 0x1108;
    pub const CP_LEVEL_TEXT: u32 = 0x1218;
    pub const START_TEXT: u32 = 0x1378;
    pub const PRESS_TEXT: u32 = 0x14D8;
    /// `mnPlayersVSMakeNameAndEmblem`'s `name_offsets`, by fighter kind.
    pub const NAMES: [u32; 12] = [
        0x1838, 0x25B8, 0x1FF8, 0x2358, 0x1B18, 0x2BA0, 0x2ED8, 0x3998, 0x28E8, 0x32F8, 0x3DB8,
        0x35B0,
    ];
    pub const INFINITY_DARK: u32 = 0x3EF0;
    pub const TIME_SELECTOR: u32 = 0x48B0;
    pub const STOCK_SELECTOR: u32 = 0x5270;
    /// `0DarkSprite` to `9DarkSprite`.
    pub const DARK_DIGITS: [u32; 10] = [
        0x5388, 0x5440, 0x5558, 0x5668, 0x5778, 0x5888, 0x5998, 0x5AA8, 0x5BB8, 0x5CC8,
    ];
    /// `HmnLabel`, `CPLabel`, `NALabel`, by player kind.
    pub const KIND_LABELS: [u32; 3] = [0x6048, 0x63C8, 0x6748];
    /// `CursorHandPoint`, `...Grab`, `...Hover`, by cursor status.
    pub const CURSORS: [u32; 3] = [0x6F88, 0x76E8, 0x8168];
    pub const TEXT_GRADIENT: [u32; 4] = [0x8268, 0x8368, 0x8468, 0x8568];
    /// `1PPuck` to `4PPuck`, then `CPPuck`.
    pub const PUCKS: [u32; 5] = [0x9048, 0x9B28, 0xA608, 0xB0E8, 0xBBC8];
    pub const DOOR_LEFT: u32 = 0xCDB0;
    pub const DOOR_RIGHT: u32 = 0xDFA0;
    /// `RedLabel`, `BlueLabel`, `GreenLabel`, by team.
    pub const TEAM_LABELS: [u32; 3] = [0xE3C8, 0xEC08, 0xE7E8];
    pub const ARROW_L: u32 = 0xECE8;
    pub const ARROW_R: u32 = 0xEDC8;
    pub const READY_TEXT: u32 = 0xF448;
    pub const READY_BANNER: u32 = 0xF530;
    pub const RED_CARD: u32 = 0x104B0;
    pub const BACK_BUTTON: u32 = 0x115C8;
}

/// `llMNCommonDigit0Sprite` to `...Digit9Sprite`.
const MN_DIGITS: [u32; 10] = [
    0xD310, 0xD3E0, 0xD4B0, 0xD580, 0xD650, 0xD720, 0xD7F0, 0xD8C0, 0xD990, 0xDA60,
];
const MN_COLON: u32 = 0xDCF0;
/// `mnPlayersVSMakeNameAndEmblem`'s `emblem_offsets`, by fighter kind.
pub(crate) const EMBLEMS: [u32; 12] = [
    0x618, 0x1938, 0xC78, 0x12D8, 0x618, 0x25F8, 0x2C58, 0x32B8, 0x1F98, 0x3918, 0x3918, 0x3F78,
];
pub(crate) const STONE_BACKGROUND: u32 = 0x440;
/// `FreeForAllTextSprite`, `TeamBattleTextSprite`.
const GAME_MODE_TEXT: [u32; 2] = [0x280, 0x4E0];
/// `llMNPlayersPortraits*` offsets.
const WHITE_SQUARE: u32 = 0x6F0;
const QUESTION_MARK: u32 = 0xF68;
const FIRE_BG: u32 = 0x24D0;
/// `mnPlayersVSMakePortrait`'s `offsets`, by fighter kind.
const PORTRAIT_SPRITES: [u32; 12] = [
    0x4728, 0xD068, 0x8BC8, 0xAE18, 0x6978, 0x11508, 0x13758, 0x19E48, 0xF2B8, 0x159A8, 0x1C098,
    0x17BF8,
];
/// `mnPlayersVSMakePortraitShadow`'s `offsets`, by fighter kind (the
/// unlocked fighters' entries are 0 and never read).
const SHADOW_SPRITES: [u32; 12] = [0, 0, 0, 0, 0x20538, 0, 0, 0x1E2E8, 0, 0, 0x249D8, 0x22788];

/// Every sprite the layer names, as `(file, offset)`, for the pack's
/// check. The gate card is packed through its LUTs.
pub const SPRITES: &[(u32, u32)] = &[
    (FILE_PLAYERS_COMMON, common::TEXT_CP),
    (FILE_PLAYERS_COMMON, common::HANDICAP_TEXT),
    (FILE_PLAYERS_COMMON, common::CP_LEVEL_TEXT),
    (FILE_PLAYERS_COMMON, common::START_TEXT),
    (FILE_PLAYERS_COMMON, common::PRESS_TEXT),
    (FILE_PLAYERS_COMMON, common::INFINITY_DARK),
    (FILE_PLAYERS_COMMON, common::TIME_SELECTOR),
    (FILE_PLAYERS_COMMON, common::STOCK_SELECTOR),
    (FILE_PLAYERS_COMMON, common::DOOR_LEFT),
    (FILE_PLAYERS_COMMON, common::DOOR_RIGHT),
    (FILE_PLAYERS_COMMON, common::ARROW_L),
    (FILE_PLAYERS_COMMON, common::ARROW_R),
    (FILE_PLAYERS_COMMON, common::READY_TEXT),
    (FILE_PLAYERS_COMMON, common::READY_BANNER),
    (FILE_PLAYERS_COMMON, common::BACK_BUTTON),
    (FILE_MN_COMMON, MN_COLON),
    (FILE_SELECT_COMMON, STONE_BACKGROUND),
    (FILE_PORTRAITS, WHITE_SQUARE),
    (FILE_PORTRAITS, QUESTION_MARK),
    (FILE_PORTRAITS, FIRE_BG),
];

/// Every sprite list the layer indexes, for the pack's check (0 entries are
/// never read).
pub const SPRITE_LISTS: &[(u32, &[u32])] = &[
    (FILE_PLAYERS_COMMON, &common::TEXT_1P),
    (FILE_PLAYERS_COMMON, &common::NAMES),
    (FILE_PLAYERS_COMMON, &common::DARK_DIGITS),
    (FILE_PLAYERS_COMMON, &common::KIND_LABELS),
    (FILE_PLAYERS_COMMON, &common::CURSORS),
    (FILE_PLAYERS_COMMON, &common::TEXT_GRADIENT),
    (FILE_PLAYERS_COMMON, &common::PUCKS),
    (FILE_PLAYERS_COMMON, &common::TEAM_LABELS),
    (FILE_MN_COMMON, &MN_DIGITS),
    (FILE_EMBLEMS, &EMBLEMS),
    (FILE_GAME_MODES, &GAME_MODE_TEXT),
    (FILE_PORTRAITS, &PORTRAIT_SPRITES),
    (FILE_PORTRAITS, &SHADOW_SPRITES),
];

/// The gate card: its file, offset and `mnPlayersVSSetGateLUT`'s LUT count.
pub const GATE_CARD: (u32, u32, u8) = (FILE_PLAYERS_COMMON, common::RED_CARD, 8);

/// The cameras' viewport, `[x0, y0, x1, y1]` on the 320 x 240 screen.
pub const VIEWPORT: [f32; 4] = [10.0, 10.0, 310.0, 230.0];

/// `mnPlayersVSGetNextPortraitX`'s `portrait_pos_x`, by portrait.
const PORTRAIT_X: [f32; 12] = [
    25.0, 70.0, 115.0, 160.0, 205.0, 250.0, 25.0, 70.0, 115.0, 160.0, 205.0, 250.0,
];
/// `mnPlayersVSGetNextPortraitX`'s `portrait_vel`, by portrait.
const PORTRAIT_VEL: [f32; 12] = [
    1.9, 3.9, 7.8, -7.8, -3.8, -1.8, 1.8, 3.8, 7.8, -7.8, -3.8, -1.8,
];
/// `mnPlayersVSSetPortraitWallpaperPosition`'s start `x`, by portrait.
pub(crate) const PORTRAIT_START_X: [f32; 12] = [
    -35.0, -35.0, -35.0, 310.0, 310.0, 310.0, -35.0, -35.0, -35.0, 310.0, 310.0, 310.0,
];

/// `mnPlayersVSMakeLabels`' game-mode label colours.
const GAME_MODE_COLORS: [[u8; 3]; 2] = [[0xE3, 0xAC, 0x04], [0x61, 0xAD, 0x49]];
/// `mnPlayersVSMakeTimeSetting`'s `colors`: environment, then primitive.
const RULE_DIGIT_COLORS: ([u8; 3], [u8; 3]) = ([0x32, 0x1C, 0x0E], [0xFF, 0xFF, 0xFF]);
/// `mnPlayersVSUpdateCursor`'s `colors`: the number's primitive, then
/// environment, by player.
pub(crate) const CURSOR_COLORS: [([u8; 3], [u8; 3]); 4] = [
    ([0xE0, 0x15, 0x15], [0x5B, 0x00, 0x00]),
    ([0x00, 0x00, 0xFB], [0x00, 0x00, 0x52]),
    ([0xCA, 0x94, 0x08], [0x62, 0x3C, 0x00]),
    ([0x00, 0x91, 0x00], [0x00, 0x4F, 0x00]),
];
/// `mnPlayersVSUpdateCursor`'s `pos`: the number's offset from the hand,
/// by cursor status.
pub(crate) const CURSOR_NUMBER_OFFSETS: [(f32, f32); 3] = [(7.0, 15.0), (9.0, 10.0), (9.0, 15.0)];
/// `mnPlayersVSMakePlayerKind`'s `pos_x`.
const KIND_TEXT_X: [f32; 4] = [8.0, 5.0, 5.0, 5.0];
/// `mnPlayersVSShutter1PProcDisplay` to `...4P`'s scissors, `[x0, y0, x1,
/// y1]` (`lbCommonSetSpriteScissor(ulx, lrx, uly, lry)`).
const SHUTTER_SCISSORS: [[f32; 4]; 4] = [
    [22.0, 126.0, 88.0, 217.0],
    [91.0, 126.0, 157.0, 217.0],
    [160.0, 126.0, 226.0, 217.0],
    [229.0, 126.0, 295.0, 217.0],
];
/// `mnPlayersVSShutterProcUpdate`'s `delta` and `max`.
const DOOR_STEP: i32 = 2;
pub const DOOR_CLOSED: i32 = 41;
/// `mnPlayersVSPortraitFlashThreadUpdate`'s `length`.
pub(crate) const FLASH_LENGTH: i32 = 16;
/// `mnPlayersVSArrowThreadUpdate`'s `blink_wait`.
const ARROW_BLINK: i32 = 10;

/// One sprite draw: an `SObj`'s sprite, top-left corner on the 320 x 240
/// screen and colours.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Piece {
    /// `llMN*FileID` and the sprite's offset in it.
    pub file: u32,
    pub offset: u32,
    /// The gate card's TLUT (`mnPlayersVSSetGateLUT`): 0 to 3 a human's by
    /// colour, 4 to 7 a CPU's.
    pub lut: Option<u8>,
    pub x: f32,
    pub y: f32,
    /// `sprite.red`, `.green`, `.blue`; `None` keeps the ROM's.
    pub prim: Option<[u8; 3]>,
    /// `sobj->envcolor`; black where the source leaves it unset.
    pub env: [u8; 3],
    /// `SP_TRANSPARENT` set (and `SP_FASTCOPY` cleared).
    pub transparent: bool,
}

/// What draws, back to front.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Draw {
    Sprite(Piece),
    /// A wrapping `SObj` (`cms`/`cmt` `G_TX_WRAP`, `masks`, `lrs`, `lrt`):
    /// the sprite repeated over `size` from the piece's corner.
    Tiled {
        piece: Piece,
        size: [f32; 2],
    },
    /// `mnPlayersVSPortraitProcDisplay`: `(NOISE - TEXEL0) * 0x30 + TEXEL0`
    /// over a locked fighter's shadow, blended.
    Shadow(Piece),
    /// `mnPlayersVSPuckProcDisplay`: `(TEXEL0 - 1) * glow + 1` in colour,
    /// `TEXEL0` in alpha, blended.
    Puck {
        piece: Piece,
        glow: u8,
    },
    /// `lbCommonSetSpriteScissor`, `[x0, y0, x1, y1]`.
    Scissor([f32; 4]),
    /// The fighters under their camera.
    Fighters,
}

/// `mnPlayersVSMakeHandicapLevel`'s arrows (`mnPlayersVSArrowThreadUpdate`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Arrows {
    pub hidden: bool,
    blink_wait: i32,
    /// The left and right arrow `SObj`s, as the thread last left them.
    pub left: bool,
    pub right: bool,
}

/// `mnPlayersVSMakePortraitFlash`'s white square.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Flash {
    pub portrait: usize,
    pub hidden: bool,
    pub(crate) length: i32,
}

/// One slot's fighter model (`mnPlayersVSMakeFighter`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Fighter {
    pub kind: FighterKind,
    /// `rotate.y`, radians.
    pub rotate_y: f32,
    /// The demo status; `None` is `nFTDemoStatusNull`, the `Wait` clip.
    pub status: Option<DemoStatus>,
    /// Bumped whenever a clip starts over (`ftManagerMakeFighter` or
    /// `scSubsysFighterSetStatus`).
    pub serial: u32,
    pub hidden: bool,
}

impl Fighter {
    /// `translate`: `(player * 840) - 1250`, -850, 0.
    pub fn position(player: usize) -> [f32; 3] {
        [(player * 840) as f32 - 1250.0, -850.0, 0.0]
    }

    /// `dSCSubsysFighterScales[fkind]`.
    pub fn scale(&self) -> f32 {
        FIGHTER_SCALES[(self.kind as usize).min(11)]
    }
}

/// A slot's display GObjs.
#[derive(Debug, Clone, Default)]
pub struct SlotView {
    /// `door_offset`: 41 closed, 0 open.
    pub door_offset: i32,
    /// `mnPlayersVSSetGateLUT`'s LUT.
    pub gate_lut: u8,
    /// `mnPlayersVSMakePlayerKind` made "CP" (else "1P" to "4P").
    pub kind_text_com: bool,
    /// When the "1P"/"CP" text GObj was last made, for its place in link 28.
    kind_text_seq: u32,
    /// `name_emblem_gobj` shown.
    pub name_shown: bool,
    /// The fighter `mnPlayersVSMakeNameAndEmblem` last drew, and whether
    /// the slot was a human then (the emblem's colour).
    pub name_made: Option<(FighterKind, bool)>,
    /// `mnPlayersVSHideFighterName` set `SP_HIDDEN` on the name.
    pub name_hidden: bool,
    /// `handicap_cpu_level`: `Some(true)` "Handicap", `Some(false)` "CP
    /// Level".
    pub level: Option<bool>,
    pub arrows: Option<Arrows>,
    /// `handicap_cpu_level_value`'s digit.
    pub value: Option<u8>,
    pub flash: Option<Flash>,
    pub fighter: Option<Fighter>,
    is_status_selected: bool,
    /// The puck GObj's flags as `mnPlayersVSPuckProcUpdate` last set them.
    pub puck_shown: bool,
}

/// The scene's display state.
#[derive(Debug, Clone)]
pub struct View {
    /// The portraits' `pos.x`, by portrait.
    pub portrait_x: [f32; 12],
    /// `sMNPlayersVSPuckGlowColor` and `...IsPuckGlowIncreasing`.
    pub glow: i32,
    glow_up: bool,
    /// `sMNPlayersVSReadyBlinkWait`.
    ready_blink: i32,
    /// The banner's and the "Press Start" GObj's flags.
    pub ready_banner: bool,
    pub ready_press: bool,
    seq: u32,
    pub slots: [SlotView; PLAYERS],
}

impl Default for View {
    fn default() -> Self {
        View {
            portrait_x: PORTRAIT_START_X,
            glow: 0,
            glow_up: false,
            ready_blink: 0,
            ready_banner: false,
            ready_press: false,
            seq: 0,
            slots: Default::default(),
        }
    }
}

/// `mnPlayersVSGetNextPortraitX`: `None` is its -1, the portrait in place.
pub fn next_portrait_x(portrait: usize, x: f32) -> Option<f32> {
    let target = PORTRAIT_X[portrait];
    let next = x + PORTRAIT_VEL[portrait];
    if x == target {
        None
    } else if target < x {
        Some(if next <= target { target } else { next })
    } else {
        Some(if next >= target { target } else { next })
    }
}

/// `mnPlayersVSGetNumberDigitCount`.
fn digit_count(number: i32, max: i32) -> i32 {
    let mut n = max;
    while n > 0 {
        if number / 10i32.pow((n - 1) as u32) != 0 {
            return n;
        }
        n -= 1;
    }
    0
}

/// `mnPlayersVSMakeGameRuleNumber`, not fixed-width: digits right-aligned
/// on `x`, from the ones leftwards, each its sprite's `width` apart.
/// `widths` gives the dark digits' widths.
pub fn rule_number(
    number: i32,
    x: f32,
    y: f32,
    max: i32,
    widths: &[f32; 10],
    mut f: impl FnMut(Piece),
) {
    let number = number.max(0);
    let (env, prim) = RULE_DIGIT_COLORS;
    let mut left = x;
    let mut put = |digit: i32, left: &mut f32| {
        let d = (digit % 10) as usize;
        *left -= widths[d];
        f(Piece {
            file: FILE_PLAYERS_COMMON,
            offset: common::DARK_DIGITS[d],
            lut: None,
            x: *left,
            y,
            prim: Some(prim),
            env,
            transparent: true,
        });
    };
    put(number, &mut left);
    for i in 1..digit_count(number, max) {
        put(number / 10i32.pow(i as u32), &mut left);
    }
}

/// The widths of `0DarkSprite` to `9DarkSprite` (the ROM's `Sprite.width`),
/// which `mnPlayersVSMakeGameRuleNumber` steps by.
pub const DARK_DIGIT_WIDTHS: [f32; 10] = [10.0, 7.0, 11.0, 10.0, 10.0, 11.0, 9.0, 10.0, 10.0, 9.0];

pub(crate) fn piece(file: u32, offset: u32, x: f32, y: f32) -> Piece {
    Piece {
        file,
        offset,
        lut: None,
        x,
        y,
        prim: None,
        env: [0; 3],
        transparent: true,
    }
}

fn panel_x(player: usize) -> f32 {
    (player * 69) as f32
}

/// `F_CST_DTOR32` / `F_CLC_DTOR32`.
pub(crate) fn dtor(deg: f32) -> f32 {
    deg * (core::f32::consts::PI / 180.0)
}

/// `mnPlayersVSGetStatusSelected`.
pub fn status_selected(kind: FighterKind) -> DemoStatus {
    use FighterKind as K;
    match kind {
        K::Fox | K::Samus => DemoStatus::Win4,
        K::Donkey | K::Luigi | K::Link | K::Captain => DemoStatus::Win1,
        K::Yoshi | K::Purin | K::Ness => DemoStatus::Win2,
        K::Mario | K::Kirby => DemoStatus::Win3,
        _ => DemoStatus::Win1,
    }
}

impl PlayersVs {
    fn next_view_seq(&mut self) -> u32 {
        self.view.seq += 1;
        self.view.seq
    }

    /// The presentation half of `mnPlayersVSFuncStart`:
    /// `mnPlayersVSMakePortraitAll` (every portrait at its
    /// `mnPlayersVSSetPortraitWallpaperPosition` start) and, per slot,
    /// `mnPlayersVSMakeGate` and the placed fighter's
    /// `mnPlayersVSMakeFighter`.
    pub(super) fn v_init(&mut self) {
        self.view = View::default();
        for p in 0..PLAYERS {
            // `mnPlayersVSMakeGate`.
            self.view.slots[p].door_offset = DOOR_CLOSED;
            self.v_set_gate_lut(p);
            self.v_make_player_kind(p);
            self.v_update_name_and_emblem(p);
            if self.is_handicap() || self.slots[p].pkind == PlayerKind::Com {
                self.v_update_handicap_level(p);
            }
            let s = &self.slots[p];
            if s.is_selected && s.fkind.is_some() {
                self.v_make_fighter(p);
            }
        }
    }

    /// `mnPlayersVSCheckHandicap`: handicap on or auto.
    fn is_handicap(&self) -> bool {
        self.handicap != Handicap::Off
    }

    /// `mnPlayersVSSetGateLUT` with the colour the caller passes: the port
    /// in free-for-all, the team's (`{0, 1, 3}`) in a team battle.
    pub(super) fn v_set_gate_lut(&mut self, p: usize) {
        let s = &self.slots[p];
        let color = if self.is_team_battle {
            if s.team == TEAM_GREEN {
                3
            } else {
                s.team
            }
        } else {
            p as u8
        };
        let com = if s.pkind == PlayerKind::Man { 0 } else { 4 };
        self.view.slots[p].gate_lut = color + com;
    }

    /// `mnPlayersVSMakePlayerKind`.
    pub(super) fn v_make_player_kind(&mut self, p: usize) {
        let seq = self.next_view_seq();
        let v = &mut self.view.slots[p];
        v.kind_text_com = self.slots[p].pkind == PlayerKind::Com;
        v.kind_text_seq = seq;
    }

    /// `mnPlayersVSUpdateNameAndEmblem` and `mnPlayersVSMakeNameAndEmblem`.
    pub(super) fn v_update_name_and_emblem(&mut self, p: usize) {
        let s = &self.slots[p];
        let v = &mut self.view.slots[p];
        if s.pkind == PlayerKind::Not || (s.fkind.is_none() && !s.is_selected) {
            v.name_shown = false;
            return;
        }
        v.name_shown = true;
        if let Some(kind) = s.fkind {
            v.name_made = Some((kind, s.pkind == PlayerKind::Man));
            v.name_hidden = false;
        }
    }

    /// `mnPlayersVSUpdateHandicapLevel`: the name is hidden and the level,
    /// its arrows (unless a human's handicap is auto) and its value are
    /// made again.
    pub(super) fn v_update_handicap_level(&mut self, p: usize) {
        // `mnPlayersVSHideFighterName`.
        if self.view.slots[p].name_made.is_some() {
            self.view.slots[p].name_hidden = true;
        }
        self.v_destroy_handicap_level(p);
        self.view.slots[p].level = Some(self.slots[p].pkind == PlayerKind::Man);
        if self.handicap != Handicap::Auto || self.slots[p].pkind == PlayerKind::Com {
            self.view.slots[p].arrows = Some(Arrows {
                hidden: false,
                blink_wait: ARROW_BLINK,
                left: false,
                right: false,
            });
        }
        self.v_make_handicap_value(p);
    }

    /// `mnPlayersVSMakeHandicapLevelValue`.
    pub(super) fn v_make_handicap_value(&mut self, p: usize) {
        let s = &self.slots[p];
        let value = if s.pkind == PlayerKind::Man {
            s.handicap
        } else {
            s.cpu_level
        };
        self.view.slots[p].value = Some(value);
    }

    /// `mnPlayersVSDestroyHandicapLevel`.
    pub(super) fn v_destroy_handicap_level(&mut self, p: usize) {
        let v = &mut self.view.slots[p];
        v.level = None;
        v.arrows = None;
        v.value = None;
    }

    /// `mnPlayersVSMakePortraitFlash` over the slot's fighter's portrait.
    pub(super) fn v_make_portrait_flash(&mut self, p: usize) {
        // `mnPlayersVSGetPortrait(nFTKindNull)` would read outside
        // `portraits`; a slot with no fighter gets no flash.
        self.view.slots[p].flash = self.slots[p].fkind.map(|kind| Flash {
            portrait: portrait(kind),
            hidden: false,
            length: FLASH_LENGTH,
        });
    }

    /// `mnPlayersVSDestroyPortraitFlash`.
    pub(super) fn v_destroy_portrait_flash(&mut self, p: usize) {
        self.view.slots[p].flash = None;
    }

    /// `mnPlayersVSMakeFighter`: the fighter is made again in its status
    /// `nFTDemoStatusNull`, keeping its turn.
    pub(super) fn v_make_fighter(&mut self, p: usize) {
        let Some(kind) = self.slots[p].fkind else {
            return;
        };
        let v = &mut self.view.slots[p];
        let (rotate_y, serial) = v.fighter.map_or((0.0, 0), |f| (f.rotate_y, f.serial + 1));
        v.fighter = Some(Fighter {
            kind,
            rotate_y,
            status: None,
            serial,
            hidden: false,
        });
    }

    /// `mnPlayersVSUpdateFighter`'s skip: the fighter GObj is hidden.
    pub(super) fn v_hide_fighter(&mut self, p: usize) {
        if let Some(f) = self.view.slots[p].fighter.as_mut() {
            f.hidden = true;
        }
    }

    /// `mnPlayersVSUpdateFighter`'s `is_status_selected = FALSE`.
    pub(super) fn v_clear_status_selected(&mut self, p: usize) {
        self.view.slots[p].is_status_selected = false;
    }

    /// The display GObjs' processes, once per tick after the logic's: the
    /// portraits' slide (`mnPlayersVSPortraitProcUpdate`), the shutters
    /// (`mnPlayersVSShutterProcUpdate`), the levels
    /// (`mnPlayersVSHandicapLevelProcUpdate`), the arrows and flashes'
    /// threads, the fighters (`mnPlayersVSFighterProcUpdate`), the puck
    /// glow (`mnPlayersVSUpdatePuckGlowColor`) and the two ready GObjs'
    /// `mnPlayersVSReadyProcUpdate`.
    pub(super) fn view_tick(&mut self) {
        for (i, x) in self.view.portrait_x.iter_mut().enumerate() {
            if let Some(next) = next_portrait_x(i, *x) {
                *x = next;
            }
        }
        for p in 0..PLAYERS {
            self.shutter_update(p);
            self.handicap_level_update(p);
            self.arrow_update(p);
            self.flash_update(p);
            self.fighter_update(p);
        }
        self.glow_update();
        let ready = self.is_ready();
        self.view.ready_banner = self.ready_update(ready);
        self.view.ready_press = self.ready_update(ready);
    }

    /// `mnPlayersVSShutterProcUpdate`.
    fn shutter_update(&mut self, p: usize) {
        let v = &mut self.view.slots[p];
        if self.slots[p].pkind == PlayerKind::Not {
            if v.door_offset < DOOR_CLOSED {
                v.door_offset = (v.door_offset + DOOR_STEP).min(DOOR_CLOSED);
            }
        } else if v.door_offset > 0 {
            v.door_offset = (v.door_offset - DOOR_STEP).max(0);
        }
    }

    /// `mnPlayersVSHandicapLevelProcUpdate`.
    fn handicap_level_update(&mut self, p: usize) {
        let Some(man) = self.view.slots[p].level else {
            return;
        };
        if !self.slots[p].is_fighter_selected {
            self.v_destroy_handicap_level(p);
        } else if man != (self.slots[p].pkind == PlayerKind::Man) {
            // `mnPlayersVSMakeHandicapLevel`: the label only. An NA slot's
            // is "CP Level", which never matches, so it is made each tick.
            self.view.slots[p].level = Some(self.slots[p].pkind == PlayerKind::Man);
        }
    }

    /// `mnPlayersVSArrowThreadUpdate`: blinks every 10 runs; the left
    /// arrow is gone at 1 and the right at 9.
    fn arrow_update(&mut self, p: usize) {
        let s = &self.slots[p];
        let value = if s.pkind == PlayerKind::Man {
            s.handicap
        } else {
            s.cpu_level
        };
        let Some(a) = self.view.slots[p].arrows.as_mut() else {
            return;
        };
        a.blink_wait -= 1;
        if a.blink_wait == 0 {
            a.blink_wait = ARROW_BLINK;
            a.hidden = !a.hidden;
        }
        a.left = value != 1;
        a.right = value != 9;
    }

    /// `mnPlayersVSPortraitFlashThreadUpdate`: shown and hidden on
    /// alternate runs, gone on the sixteenth.
    fn flash_update(&mut self, p: usize) {
        step_flash(&mut self.view.slots[p].flash);
    }

    /// `mnPlayersVSFighterProcUpdate`: a fighter not placed turns 2 degrees
    /// a tick; once placed it turns 20 a tick back to 0 and plays its
    /// selected status (`mnPlayersVSGetStatusSelected`).
    fn fighter_update(&mut self, p: usize) {
        let selected = self.slots[p].is_fighter_selected;
        let v = &mut self.view.slots[p];
        if let Some(f) = v.fighter.as_mut() {
            turn_fighter(f, &mut v.is_status_selected, selected);
        }
    }

    /// `mnPlayersVSUpdatePuckGlowColor`: up by 9 to 0xFF, down by 9 to 0x80.
    /// The turn at the top steps down in the same call.
    fn glow_update(&mut self) {
        step_glow(&mut self.view.glow, &mut self.view.glow_up);
    }

    /// `mnPlayersVSReadyProcUpdate`, whether its GObj shows. Both ready
    /// GObjs run it, and both step the one blink counter.
    fn ready_update(&mut self, ready: bool) -> bool {
        step_ready(&mut self.view.ready_blink, ready)
    }

    /// What draws, back to front.
    pub fn visit(&self, mut f: impl FnMut(Draw)) {
        f(Draw::Scissor(VIEWPORT));
        self.visit_top_bar(&mut f);
        self.visit_portraits(&mut f);
        self.visit_panels(&mut f);
        f(Draw::Fighters);
        self.visit_pucks_and_cursors(&mut f);
        if self.view.ready_banner {
            visit_ready_banner(&mut f);
        }
    }

    /// Camera 80, link 26: `mnPlayersVSMakeWallpaper`'s stone tile over the
    /// viewport (opaque, `SP_FASTCOPY` kept), the game-mode label, the
    /// time or stock selector with its number, and the back button.
    fn visit_top_bar(&self, f: &mut impl FnMut(Draw)) {
        visit_stone(f);
        let team = usize::from(self.is_team_battle);
        let mut mode = piece(FILE_GAME_MODES, GAME_MODE_TEXT[team], 27.0, 24.0);
        mode.prim = Some(GAME_MODE_COLORS[team]);
        f(Draw::Sprite(mode));
        let mut put = |p: Piece| f(Draw::Sprite(p));
        if self.rule == Rule::Time {
            put(piece(
                FILE_PLAYERS_COMMON,
                common::TIME_SELECTOR,
                140.0,
                22.0,
            ));
            // `mnPlayersVSMakeTimeSetting`.
            if self.time_value == TIMELIMIT_INFINITE {
                let (env, prim) = RULE_DIGIT_COLORS;
                let mut inf = piece(FILE_PLAYERS_COMMON, common::INFINITY_DARK, 194.0, 24.0);
                inf.prim = Some(prim);
                inf.env = env;
                put(inf);
            } else {
                let x = if self.time_value < 10 { 208.0 } else { 212.0 };
                rule_number(
                    i32::from(self.time_value),
                    x,
                    23.0,
                    2,
                    &DARK_DIGIT_WIDTHS,
                    &mut put,
                );
            }
        } else {
            put(piece(
                FILE_PLAYERS_COMMON,
                common::STOCK_SELECTOR,
                140.0,
                22.0,
            ));
            // `mnPlayersVSMakeStockSelect` shows `sMNPlayersVSStockValue + 1`.
            let n = i32::from(self.stock_value) + 1;
            let x = if n < 10 { 210.0 } else { 214.0 };
            rule_number(n, x, 23.0, 2, &DARK_DIGIT_WIDTHS, &mut put);
        }
        put(piece(FILE_PLAYERS_COMMON, common::BACK_BUTTON, 244.0, 23.0));
    }

    /// Cameras 75, 73 and 70: the unlocked portraits' fire backgrounds
    /// (opaque), the flashes, then the portraits (a locked one's fire
    /// background, shadow and question mark).
    fn visit_portraits(&self, f: &mut impl FnMut(Draw)) {
        let flashes: [Option<Flash>; PLAYERS] = core::array::from_fn(|p| self.view.slots[p].flash);
        visit_portraits(&self.view.portrait_x, self.fighter_mask, &flashes, f);
    }

    /// Cameras 50 to 35: the cards, "1P" texts, names and emblems and "Press
    /// Start" (link 28, in the order their GObjs were made), the team
    /// buttons, the levels, the doors under their scissors, and the
    /// HMN/CP/NA buttons.
    fn visit_panels(&self, f: &mut impl FnMut(Draw)) {
        // Link 28. The cards, names and "Press Start" keep their GObjs; a
        // "1P" text made again moves to the end of the link.
        let mut texts: [(u32, usize); PLAYERS] =
            core::array::from_fn(|p| (self.view.slots[p].kind_text_seq, p));
        texts.sort_unstable();
        let init_seq = PLAYERS as u32;
        for p in 0..PLAYERS {
            let v = &self.view.slots[p];
            let x = panel_x(p);
            f(Draw::Sprite(Piece {
                lut: Some(v.gate_lut),
                ..piece(FILE_PLAYERS_COMMON, common::RED_CARD, x + 22.0, 126.0)
            }));
            if v.kind_text_seq <= init_seq {
                f(Draw::Sprite(self.kind_text(p)));
            }
            self.visit_name(p, f);
        }
        if self.view.ready_press {
            visit_press_start(f);
        }
        for (seq, p) in texts {
            if seq > init_seq {
                f(Draw::Sprite(self.kind_text(p)));
            }
        }
        // Link 34: `mnPlayersVSMakeTeamSelectAll` gives every slot one.
        if self.is_team_battle {
            for (p, s) in self.slots.iter().enumerate() {
                let team = usize::from(s.team).min(2);
                f(Draw::Sprite(piece(
                    FILE_PLAYERS_COMMON,
                    common::TEAM_LABELS[team],
                    panel_x(p) + 34.0,
                    131.0,
                )));
            }
        }
        // Link 35.
        for p in 0..PLAYERS {
            self.visit_level(p, f);
        }
        // Link 29.
        for (p, &scissor) in SHUTTER_SCISSORS.iter().enumerate() {
            let off = self.view.slots[p].door_offset as f32;
            let x = panel_x(p);
            f(Draw::Scissor(scissor));
            f(Draw::Sprite(piece(
                FILE_PLAYERS_COMMON,
                common::DOOR_LEFT,
                x - 19.0 + off,
                126.0,
            )));
            f(Draw::Sprite(piece(
                FILE_PLAYERS_COMMON,
                common::DOOR_RIGHT,
                x + 88.0 - off,
                126.0,
            )));
            f(Draw::Scissor(VIEWPORT));
        }
        // Link 30: `mnPlayersVSUpdatePlayerKindSelect`.
        for (p, s) in self.slots.iter().enumerate() {
            let kind = s.pkind as usize;
            f(Draw::Sprite(piece(
                FILE_PLAYERS_COMMON,
                common::KIND_LABELS[kind],
                panel_x(p) + 64.0,
                131.0,
            )));
        }
    }

    /// `mnPlayersVSMakePlayerKind`: "CP" at x + 26, else the port's number.
    fn kind_text(&self, p: usize) -> Piece {
        let x = panel_x(p);
        let (offset, x) = if self.view.slots[p].kind_text_com {
            (common::TEXT_CP, x + 26.0)
        } else {
            (common::TEXT_1P[p], KIND_TEXT_X[p] + x + 22.0)
        };
        Piece {
            prim: Some([0; 3]),
            ..piece(FILE_PLAYERS_COMMON, offset, x, 131.0)
        }
    }

    /// `mnPlayersVSMakeNameAndEmblem`'s emblem (grey 0x1E for a human,
    /// 0x44 for a CPU) and name.
    fn visit_name(&self, p: usize, f: &mut impl FnMut(Draw)) {
        let v = &self.view.slots[p];
        let Some((kind, man)) = v.name_made.filter(|_| v.name_shown) else {
            return;
        };
        let x = panel_x(p);
        let k = (kind as usize).min(11);
        let grey = if man { 0x1E } else { 0x44 };
        f(Draw::Sprite(Piece {
            prim: Some([grey; 3]),
            ..piece(FILE_EMBLEMS, EMBLEMS[k], x + 24.0, 143.0)
        }));
        if !v.name_hidden {
            f(Draw::Sprite(piece(
                FILE_PLAYERS_COMMON,
                common::NAMES[k],
                x + 22.0,
                201.0,
            )));
        }
    }

    /// `mnPlayersVSMakeHandicapLevel`, its arrows and
    /// `mnPlayersVSMakeHandicapLevelValue`.
    fn visit_level(&self, p: usize, f: &mut impl FnMut(Draw)) {
        let v = &self.view.slots[p];
        let x = panel_x(p);
        if let Some(man) = v.level {
            let (offset, dx) = if man {
                (common::HANDICAP_TEXT, 35.0)
            } else {
                (common::CP_LEVEL_TEXT, 34.0)
            };
            f(Draw::Sprite(Piece {
                prim: Some([0xC2, 0xBD, 0xAD]),
                ..piece(FILE_PLAYERS_COMMON, offset, x + dx, 201.0)
            }));
            f(Draw::Sprite(Piece {
                prim: Some([0xFF; 3]),
                ..piece(FILE_MN_COMMON, MN_COLON, x + 61.0, 202.0)
            }));
        }
        if let Some(a) = v.arrows.filter(|a| !a.hidden) {
            if a.left {
                f(Draw::Sprite(piece(
                    FILE_PLAYERS_COMMON,
                    common::ARROW_L,
                    x + 25.0,
                    201.0,
                )));
            }
            if a.right {
                f(Draw::Sprite(piece(
                    FILE_PLAYERS_COMMON,
                    common::ARROW_R,
                    x + 79.0,
                    201.0,
                )));
            }
        }
        if let Some(value) = v.value {
            f(Draw::Sprite(Piece {
                prim: Some([0xFF; 3]),
                ..piece(
                    FILE_MN_COMMON,
                    MN_DIGITS[usize::from(value).min(9)],
                    x + 67.0,
                    200.0,
                )
            }));
        }
    }

    /// Cameras 25 and 20: the pucks on link 33, then link 32's cursors and
    /// held pucks, each by descending priority, then as made
    /// ([`DrawKey`]).
    fn visit_pucks_and_cursors(&self, f: &mut impl FnMut(Draw)) {
        // Up to four pucks and four cursors.
        let mut items: [(DrawKey, usize, bool); 2 * PLAYERS] =
            [(DrawKey::default(), 0, false); 2 * PLAYERS];
        let mut n = 0;
        for (p, s) in self.slots.iter().enumerate() {
            if self.view.slots[p].puck_shown {
                items[n] = (s.puck_draw, p, false);
                n += 1;
            }
            if s.cursor.is_some() {
                items[n] = (s.cursor_draw, p, true);
                n += 1;
            }
        }
        let items = &mut items[..n];
        items.sort_unstable_by(|a, b| {
            let rank = |k: &DrawKey| {
                (
                    if k.link == DL_PUCK { 0 } else { 1 },
                    core::cmp::Reverse(k.priority),
                    k.seq,
                )
            };
            rank(&a.0).cmp(&rank(&b.0))
        });
        for &(key, p, is_cursor) in items.iter() {
            debug_assert!(key.link == DL_PUCK || key.link == DL_CURSOR);
            if is_cursor {
                self.visit_cursor(p, f);
            } else {
                self.visit_puck(p, f);
            }
        }
    }

    /// `mnPlayersVSUpdatePuck`: "CP" for a CPU (`mnPlayersVSUpdatePuckDisplay`
    /// and `mnPlayersVSMakePuck`), else the port's.
    fn visit_puck(&self, p: usize, f: &mut impl FnMut(Draw)) {
        let s = &self.slots[p];
        let id = if s.pkind == PlayerKind::Com { 4 } else { p };
        f(puck(id, s.puck, self.view.glow));
    }

    /// `mnPlayersVSUpdateCursor`: the hand for the cursor's status, and the
    /// port's number beside it.
    fn visit_cursor(&self, p: usize, f: &mut impl FnMut(Draw)) {
        let s = &self.slots[p];
        if let Some(at) = s.cursor {
            visit_cursor(p, at, s.cursor_status, f);
        }
    }
}

/// `mnPlayersVSMakeWallpaper` (and `mnPlayers1PTrainingMakeWallpaper`): the
/// stone tile wrapped over the viewport, opaque with `SP_FASTCOPY` kept.
pub(crate) fn visit_stone(f: &mut impl FnMut(Draw)) {
    let mut stone = piece(FILE_SELECT_COMMON, STONE_BACKGROUND, 10.0, 10.0);
    stone.transparent = false;
    f(Draw::Tiled {
        piece: stone,
        size: [300.0, 220.0],
    });
}

/// Cameras 75, 73 and 70 (`mnPlayersVSMakePortrait`, shared by
/// `mnPlayers1PTrainingMakePortrait`): the unlocked portraits' fire
/// backgrounds (opaque), the shown flashes, then the portraits (a locked
/// one's fire background, shadow and question mark).
pub(crate) fn visit_portraits(
    portrait_x: &[f32; 12],
    fighter_mask: u16,
    flashes: &[Option<Flash>],
    f: &mut impl FnMut(Draw),
) {
    let y_of = |i: usize| if i >= 6 { 79.0 } else { 36.0 };
    let locked = |i: usize| is_locked(PORTRAIT_KINDS[i], fighter_mask);
    for i in (0..12).filter(|&i| !locked(i)) {
        let mut bg = piece(FILE_PORTRAITS, FIRE_BG, portrait_x[i], y_of(i));
        bg.transparent = false;
        f(Draw::Sprite(bg));
    }
    for fl in flashes.iter().flatten().filter(|fl| !fl.hidden) {
        let p = fl.portrait;
        let x = (if p >= 6 { p - 6 } else { p } * 45 + 26) as f32;
        let y = if p >= 6 { 80.0 } else { 37.0 };
        f(Draw::Sprite(piece(FILE_PORTRAITS, WHITE_SQUARE, x, y)));
    }
    for (i, &kind) in PORTRAIT_KINDS.iter().enumerate() {
        let (x, y) = (portrait_x[i], y_of(i));
        let kind = kind as usize;
        if locked(i) {
            // `mnPlayersVSMakePortraitShadow`.
            let mut bg = piece(FILE_PORTRAITS, FIRE_BG, x, y);
            bg.transparent = false;
            f(Draw::Sprite(bg));
            f(Draw::Shadow(piece(
                FILE_PORTRAITS,
                SHADOW_SPRITES[kind],
                x,
                y,
            )));
            let mut q = piece(FILE_PORTRAITS, QUESTION_MARK, x, y);
            q.env = [0x5B, 0x41, 0x33];
            q.prim = Some([0xC4, 0xB9, 0xA9]);
            f(Draw::Sprite(q));
        } else {
            f(Draw::Sprite(piece(
                FILE_PORTRAITS,
                PORTRAIT_SPRITES[kind],
                x,
                y,
            )));
        }
    }
}

/// `mnPlayersVSMakeReady`'s banner GObj (link 38): the strip wraps its 8
/// texels over 320 (`masks` 3, `lrs` 320, `lrt` 17), then "Ready to fight".
pub(crate) fn visit_ready_banner(f: &mut impl FnMut(Draw)) {
    let mut banner = piece(FILE_PLAYERS_COMMON, common::READY_BANNER, 0.0, 71.0);
    banner.prim = Some([0xF4, 0x56, 0x7F]);
    f(Draw::Tiled {
        piece: banner,
        size: [320.0, 17.0],
    });
    let mut text = piece(FILE_PLAYERS_COMMON, common::READY_TEXT, 50.0, 76.0);
    text.prim = Some([0xFF, 0xFF, 0x9D]);
    text.env = [0xFF, 0xCA, 0x13];
    f(Draw::Sprite(text));
}

/// `mnPlayersVSMakeReady`'s "Press Start" GObj (link 28, US).
pub(crate) fn visit_press_start(f: &mut impl FnMut(Draw)) {
    let mut press = piece(FILE_PLAYERS_COMMON, common::PRESS_TEXT, 133.0, 219.0);
    press.prim = Some([0xD6, 0xDD, 0xC6]);
    f(Draw::Sprite(press));
    let mut start = piece(FILE_PLAYERS_COMMON, common::START_TEXT, 162.0, 219.0);
    start.prim = Some([0xFF, 0x56, 0x92]);
    f(Draw::Sprite(start));
}

/// A puck (`1PPuck` to `4PPuck`, `CPPuck` for 4) under the glow.
pub(crate) fn puck(id: usize, at: (f32, f32), glow: i32) -> Draw {
    Draw::Puck {
        piece: piece(FILE_PLAYERS_COMMON, common::PUCKS[id], at.0, at.1),
        glow: glow.clamp(0, 0xFF) as u8,
    }
}

/// `mnPlayersVSUpdateCursor` (and `mnPlayers1PTrainingUpdateCursor`): the
/// hand for the cursor's status, and port `p`'s number beside it.
pub(crate) fn visit_cursor(
    p: usize,
    (x, y): (f32, f32),
    status: CursorStatus,
    f: &mut impl FnMut(Draw),
) {
    let status = match status {
        CursorStatus::Pointer => 0,
        CursorStatus::Grab => 1,
        CursorStatus::Hover => 2,
    };
    f(Draw::Sprite(piece(
        FILE_PLAYERS_COMMON,
        common::CURSORS[status],
        x,
        y,
    )));
    let (dx, dy) = CURSOR_NUMBER_OFFSETS[status];
    let (prim, env) = CURSOR_COLORS[p];
    f(Draw::Sprite(Piece {
        prim: Some(prim),
        env,
        ..piece(
            FILE_PLAYERS_COMMON,
            common::TEXT_GRADIENT[p],
            x + dx,
            y + dy,
        )
    }));
}

/// `mnPlayersVSFighterProcUpdate` (and `mnPlayers1PTrainingFighterProcUpdate`):
/// a fighter not placed turns 2 degrees a tick; once placed it turns 20 a
/// tick back to 0 and plays its selected status
/// (`mnPlayersVSGetStatusSelected`).
pub(crate) fn turn_fighter(f: &mut Fighter, is_status_selected: &mut bool, selected: bool) {
    let set_status = |f: &mut Fighter| {
        f.status = Some(status_selected(f.kind));
        f.serial += 1;
    };
    if selected {
        if f.rotate_y < dtor(0.1) {
            if !*is_status_selected {
                set_status(f);
                *is_status_selected = true;
            }
        } else {
            f.rotate_y += dtor(20.0);
            if f.rotate_y > dtor(360.0) {
                f.rotate_y = 0.0;
                set_status(f);
                *is_status_selected = true;
            }
        }
    } else {
        f.rotate_y += dtor(2.0);
        if f.rotate_y > dtor(360.0) {
            f.rotate_y -= dtor(360.0);
        }
    }
}

/// `mnPlayersVSPortraitFlashThreadUpdate`: shown and hidden on alternate
/// runs, gone on the sixteenth.
pub(crate) fn step_flash(flash: &mut Option<Flash>) {
    let Some(f) = flash.as_mut() else {
        return;
    };
    f.length -= 1;
    if f.length == 0 {
        *flash = None;
        return;
    }
    f.hidden = !f.hidden;
}

/// `mnPlayersVSUpdatePuckGlowColor`: up by 9 to 0xFF, down by 9 to 0x80.
/// The turn at the top steps down in the same call.
pub(crate) fn step_glow(glow: &mut i32, up: &mut bool) {
    if !*up {
        *glow += 9;
        if *glow > 0xFF {
            *glow = 0xFF;
            *up = true;
        }
    }
    if *up {
        *glow -= 9;
        if *glow < 0x80 {
            *glow = 0x80;
            *up = false;
        }
    }
}

/// `mnPlayersVSReadyProcUpdate`, whether its GObj shows: while ready, the
/// blink counter runs to 40 and the GObj shows below 30.
pub(crate) fn step_ready(blink: &mut i32, ready: bool) -> bool {
    if ready {
        *blink += 1;
        if *blink == 40 {
            *blink = 0;
        }
        *blink < 30
    } else {
        *blink = 0;
        false
    }
}

#[cfg(test)]
#[path = "players_vs_layer_tests.rs"]
mod tests;
