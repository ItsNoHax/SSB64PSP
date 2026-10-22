//! The battle HUD's damage display — `ifCommonPlayerDamage*` in
//! `if/ifcommon.c`: each player's percent under the stage, which swells and
//! flashes white when damage lands, shades toward dark red as it climbs,
//! and breaks apart when the player falls.
//!
//! This module holds the per-player state machine and the glyphs it draws;
//! the host draws each [`Glyph`] as sprite `digit` of file 164
//! (`dIFCommonPlayerDamageDigitSpriteOffsets`), after the fighter's series
//! emblem at [`emblem_origin`].

use crate::rng;
use core::sync::atomic::{AtomicU32, AtomicU8, Ordering};

/// `GMCOMMON_PLAYERS_MAX`, also the white flash colour's index.
pub const PLAYERS_MAX: usize = 4;
/// Glyph `10`: `%`. Glyph `11` is `H.P`, the Master Hand's.
pub const PERCENT: u8 = 0xA;
/// Glyph `11`: `H.P`.
pub const HIT_POINTS: u8 = 0xB;
/// Master Hand's hit points (`ifCommonPlayerDamageUpdateDigits`).
pub const BOSS_HIT_POINTS: i32 = 300;

/// `dIFCommonPlayerDamageDigitWidths`: each glyph's advance.
pub const DIGIT_WIDTHS: [i32; 12] = crate::d_arrays::IF_COMMON_PLAYER_DAMAGE_DIGIT_WIDTHS;
/// `dIFCommonPlayerDamageDigitColors{R,G,B}`; index 4 is the flash.
const COLORS_R: [u8; 5] = crate::d_arrays::IF_COMMON_PLAYER_DAMAGE_DIGIT_COLORS_R;
const COLORS_G: [u8; 5] = crate::d_arrays::IF_COMMON_PLAYER_DAMAGE_DIGIT_COLORS_G;
const COLORS_B: [u8; 5] = crate::d_arrays::IF_COMMON_PLAYER_DAMAGE_DIGIT_COLORS_B;
/// `dIFCommonPlayerDamagePositionOffsetsX`; the Y is
/// `ifCommonPlayerDamageSetDigitPositions`'s `player_pos_y`.
pub const POSITION_X: [i32; 4] = crate::d_arrays::IF_COMMON_PLAYER_DAMAGE_POSITION_OFFSETS_X;
pub const POSITION_Y: i32 = 210;

/// `ifCommonPlayerStockMakeStockSnap`: where a lost stock's burst starts,
/// the player's interface position (`efManagerStockSnapMakeEffect`).
pub fn stock_snap_position(pos_x: i32) -> (f32, f32) {
    stock_snap_position_at(pos_x, POSITION_Y)
}

/// [`stock_snap_position`] at interface height `pos_y`.
pub fn stock_snap_position_at(pos_x: i32, pos_y: i32) -> (f32, f32) {
    (pos_x as f32, pos_y as f32)
}

/// What `ftCommonDeadUpdateScore` reads of the interface when a fighter
/// falls, inside its own process: `gIFCommonPlayerInterface`'s positions,
/// each damage display's `char_display_count` (its break's draws) and
/// `gSCManagerBattleState->is_show_score`. The host publishes it from its
/// displays ([`FallInterface::of`], [`set_fall_interface`]) before the
/// fighters' processes; a port without a display has no position (no stock
/// snap, no score).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct FallInterface {
    pub pos: [Option<(i32, i32)>; 4],
    pub char_display_count: [u8; 4],
    pub show_score: bool,
}

impl FallInterface {
    /// The interface as `displays` show it.
    pub fn of(displays: &[DamageDisplay; 4], show_score: bool) -> FallInterface {
        FallInterface {
            pos: displays.each_ref().map(|d| Some((d.pos_x, d.pos_y))),
            char_display_count: displays.each_ref().map(|d| d.char_display_count as u8),
            show_score,
        }
    }
}

// Globals, like the original's interface structs: published by the host,
// read inside a fighter's process. Kept out of `Effects` and `Fighter`,
// whose by-value temporaries sit on the game's deepest stack path.
game_static! {
    /// Each port's `(x, y)` as two `i16`s.
    static FALL_POS: [AtomicU32; 4] = [const { AtomicU32::new(0) }; 4];
}
game_static! {
    /// Bit `port`: the port has a position; bit 7: `is_show_score`.
    static FALL_FLAGS: AtomicU8 = AtomicU8::new(0);
}
game_static! {
    /// Each port's `char_display_count`, one byte each.
    static FALL_CHARS: AtomicU32 = AtomicU32::new(0);
}
game_static! {
    /// `ifCommonPlayerDamageStartBreakAnim`'s draws per port and glyph,
    /// `f32` bits, for the host's display to take.
    static BREAK_VEL_X: [AtomicU32; 16] = [const { AtomicU32::new(0) }; 16];
}
game_static! {
    /// Bit `port`: a break's draws wait in [`BREAK_VEL_X`].
    static BREAK_PENDING: AtomicU8 = AtomicU8::new(0);
}

/// Publishes the interface a fall reads ([`FallInterface`]).
pub fn set_fall_interface(ui: &FallInterface) {
    let mut flags = u8::from(ui.show_score) << 7;
    FALL_POS.with(|pos| {
        for (port, p) in ui.pos.iter().enumerate() {
            if let Some((x, y)) = *p {
                flags |= 1 << port;
                pos[port].store(
                    (u32::from(x as i16 as u16) << 16) | u32::from(y as i16 as u16),
                    Ordering::Relaxed,
                );
            }
        }
    });
    FALL_FLAGS.with(|f| f.store(flags, Ordering::Relaxed));
    FALL_CHARS.with(|c| c.store(u32::from_le_bytes(ui.char_display_count), Ordering::Relaxed));
}

/// Withdraws the published interface: a fall outside the battle frame
/// (the demos' own frames) reads no positions and no glyphs.
pub fn clear_fall_interface() {
    FALL_FLAGS.with(|f| f.store(0, Ordering::Relaxed));
    FALL_CHARS.with(|c| c.store(0, Ordering::Relaxed));
}

/// The interface [`set_fall_interface`] last published.
pub fn fall_interface() -> FallInterface {
    let flags = FALL_FLAGS.with(|f| f.load(Ordering::Relaxed));
    let pos = FALL_POS.with(|pos| {
        core::array::from_fn(|port| {
            let v = pos[port].load(Ordering::Relaxed);
            (flags & (1 << port) != 0).then_some((
                i32::from((v >> 16) as u16 as i16),
                i32::from(v as u16 as i16),
            ))
        })
    });
    FallInterface {
        pos,
        char_display_count: FALL_CHARS.with(|c| c.load(Ordering::Relaxed)).to_le_bytes(),
        show_score: flags & 0x80 != 0,
    }
}

/// Leaves a fall's break draws for `port`'s display.
pub(crate) fn put_break_vel_x(port: usize, vel_x: [f32; 4]) {
    BREAK_VEL_X.with(|v| {
        for (i, x) in vel_x.iter().enumerate() {
            v[(port & 3) * 4 + i].store(x.to_bits(), Ordering::Relaxed);
        }
    });
    BREAK_PENDING.with(|b| b.fetch_or(1 << (port & 3), Ordering::Relaxed));
}

/// Takes the break draws a fall left for `port`'s display
/// ([`DamageDisplay::start_break_anim_from`]).
pub fn take_break_vel_x(port: usize) -> Option<[f32; 4]> {
    let bit = 1 << (port & 3);
    if BREAK_PENDING.with(|b| b.fetch_and(!bit, Ordering::Relaxed)) & bit == 0 {
        return None;
    }
    Some(BREAK_VEL_X.with(|v| {
        core::array::from_fn(|i| f32::from_bits(v[(port & 3) * 4 + i].load(Ordering::Relaxed)))
    }))
}

/// `ifCommonPlayerScoreMakeEffect`: the "+1" or "-1" at the player's
/// interface position (`dIFCommonPlayerScorePositionOffsetsX` is all 0)
/// and 13 below it, in the effect camera's quarter pixels.
pub fn score_position(pos_x: i32) -> ssb_engine::math::Vec3 {
    score_position_at(pos_x, POSITION_Y)
}

/// [`score_position`] at interface height `pos_y`.
pub fn score_position_at(pos_x: i32, pos_y: i32) -> ssb_engine::math::Vec3 {
    ssb_engine::math::Vec3::new((pos_x << 2) as f32, ((pos_y + 13) << 2) as f32, 0.0)
}

/// `IFPlayerSteal` and `ifCommonPlayerStockSteal*` (RE-464): the stolen
/// player's stock icon (`SP_TEXSHUF | SP_TRANSPARENT`, its costume's LUT)
/// flying in an arc from its stocks to the thief's for 30 frames.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StockSteal {
    /// The player whose stock icon flies.
    pub stolen: u8,
    pub anim_frames: u8,
    pub steal_pos_x: u16,
    pub steal_pos_y: u16,
    pub target_pos_x: u16,
    /// The icon's `SObj` position.
    pub x: f32,
    pub y: f32,
}

/// `ifCommonPlayerStockStealMakeInterface`'s `anim_frames`.
pub const STOCK_STEAL_FRAMES: u8 = 30;

/// Where `efManagerStockStealStartMakeEffect` and `...EndMakeEffect` burst:
/// a player's first stock icon's centre.
pub fn steal_effect_position(pos_x: i32) -> (f32, f32) {
    steal_effect_position_at(pos_x, POSITION_Y)
}

/// [`steal_effect_position`] at interface height `pos_y`.
pub fn steal_effect_position_at(pos_x: i32, pos_y: i32) -> (f32, f32) {
    ((pos_x + STOCK_ICON_X) as f32, (pos_y - 20) as f32)
}

impl StockSteal {
    /// `ifCommonPlayerStockStealMakeInterface`: a `w` by `h` icon from
    /// `stolen`'s stocks (interface position `stolen_pos_x`) towards the
    /// thief's (`thief_pos_x`).
    pub fn make(stolen: u8, stolen_pos_x: i32, thief_pos_x: i32, w: u16, h: u16) -> StockSteal {
        Self::make_at(stolen, stolen_pos_x, thief_pos_x, POSITION_Y, w, h)
    }

    /// [`StockSteal::make`] at interface height `pos_y`.
    pub fn make_at(
        stolen: u8,
        stolen_pos_x: i32,
        thief_pos_x: i32,
        pos_y: i32,
        w: u16,
        h: u16,
    ) -> StockSteal {
        let half_w = (f32::from(w) * 0.5) as i32;
        let half_h = (f32::from(h) * 0.5) as i32;
        let steal_pos_x = (stolen_pos_x + STOCK_ICON_X - half_w) as u16;
        let steal_pos_y = (pos_y - half_h - 20) as u16;
        let target_pos_x = (thief_pos_x + STOCK_ICON_X - half_w) as u16;
        StockSteal {
            stolen,
            anim_frames: STOCK_STEAL_FRAMES,
            steal_pos_x,
            steal_pos_y,
            target_pos_x,
            x: f32::from(steal_pos_x),
            y: f32::from(steal_pos_y),
        }
    }

    /// `ifCommonPlayerStockStealProcUpdate`: one frame of the arc. `false`
    /// once the frames run out, when the host makes the end burst at the
    /// thief's [`steal_effect_position`] and drops the icon.
    pub fn update(&mut self) -> bool {
        self.anim_frames -= 1;
        if self.anim_frames == 0 {
            return false;
        }
        let dist_x = (i32::from(self.steal_pos_x) - i32::from(self.target_pos_x)) as f32;
        let vel_x = (f32::from(self.anim_frames) * dist_x) / 30.0;
        let vel_y = if vel_x < dist_x * 0.5 {
            -(vel_x - dist_x * 0.5)
        } else {
            vel_x - dist_x * 0.5
        };
        let half = dist_x * 0.5;
        self.x = f32::from(self.target_pos_x) + vel_x;
        self.y = (f32::from(self.steal_pos_y) + (15.0 / (half * half)) * vel_y * vel_y) - 15.0;
        true
    }
}

/// `players[].color` of a CPU outside team battles, `GMCOMMON_PLAYERS_MAX`:
/// its emblem takes the stage's fifth, grey colour.
pub const CPU_COLOR: usize = 4;

/// `dIFCommonPlayerDamageEmblemOffsets{X,Y}` (3, -3 for every player) and
/// the scale of 1: where `ifCommonPlayerDamageInitInterface` puts a `w` by
/// `h` emblem, truncated to whole pixels. It is drawn first, whether or not
/// the digits show.
pub fn emblem_origin(pos_x: i32, w: u16, h: u16) -> (f32, f32) {
    emblem_origin_at(pos_x, POSITION_Y, w, h)
}

/// [`emblem_origin`] at interface height `pos_y`.
pub fn emblem_origin_at(pos_x: i32, pos_y: i32, w: u16, h: u16) -> (f32, f32) {
    let x = (pos_x as f32 - f32::from(w) * 0.5) + EMBLEM_OFFSET_X as f32;
    let y = (pos_y as f32 - f32::from(h) * 0.5) + EMBLEM_OFFSET_Y as f32;
    (x as i32 as f32, y as i32 as f32)
}

/// `dIFCommonPlayerStocksIconOffsetsX`: the same for every player, which is
/// why the port's stock-icon functions take no player.
const STOCK_ICON_X: i32 = crate::d_arrays::IF_COMMON_PLAYER_STOCKS_ICON_OFFSETS_X[0];

/// `dIFCommonPlayerDamageEmblemOffsetsX` and `...Y`: the same for every
/// player, which is why [`emblem_origin_at`] takes no player.
const EMBLEM_OFFSET_X: i32 = crate::d_arrays::IF_COMMON_PLAYER_DAMAGE_EMBLEM_OFFSETS_X[0];
const EMBLEM_OFFSET_Y: i32 = crate::d_arrays::IF_COMMON_PLAYER_DAMAGE_EMBLEM_OFFSETS_Y[0];

/// Where a fighter's stock icons sit (`ifCommonPlayerStock*`), top-left
/// corners of `w` by `h` icons. A time battle's single icon
/// (`is_single_stockicon`, `mnPlayersVS`) shows while the player has a
/// stock count; a stock battle shows one per stock left, `stocks + 1`,
/// ten apart, up to six. More than six switches to digits, which no VS
/// menu setting reaches and is not ported.
pub fn stock_icons(
    pos_x: i32,
    stocks: i8,
    single: bool,
    w: u16,
    h: u16,
) -> impl Iterator<Item = (f32, f32)> {
    stock_icons_at(pos_x, POSITION_Y, stocks, single, w, h)
}

/// `players[player].is_single_stockicon`: the scene's per-player flags
/// where it sets them (the 1P Game, Training's
/// [`crate::training::SINGLE_STOCK`]), else a VS battle's rule (a time
/// battle shows one icon). `None` when neither is known: no stock
/// interface.
pub fn is_single_stock(
    per_player: Option<[bool; 4]>,
    rule: Option<crate::battle::Rule>,
    player: usize,
) -> Option<bool> {
    per_player
        .map(|s| s[player])
        .or_else(|| rule.map(|r| r == crate::battle::Rule::Time))
}

/// [`stock_icons`] at interface height `pos_y`.
pub fn stock_icons_at(
    pos_x: i32,
    pos_y: i32,
    stocks: i8,
    single: bool,
    w: u16,
    h: u16,
) -> impl Iterator<Item = (f32, f32)> {
    let base_x = pos_x + STOCK_ICON_X;
    let y = ((pos_y - (f32::from(h) * 0.5) as i32) - 20) as f32;
    let count = if stocks < 0 {
        0
    } else if single {
        1
    } else {
        (i32::from(stocks) + 1).min(6)
    };
    (0..count).map(move |order| {
        let x = if single {
            (base_x - (f32::from(w) * 0.5) as i32) as f32
        } else {
            (base_x + order * 10) as f32 - f32::from(w) * 0.5
        };
        (x, y)
    })
}

/// `dIFCommonTimerDigitsSpritePositionsX`: the centres of `M M : S S`'s
/// digits, and the colon's (`ifCommonTimerMakeDigits`).
pub const TIMER_X: [f32; 4] = {
    let x = crate::d_arrays::IF_COMMON_TIMER_DIGITS_SPRITE_POSITIONS_X;
    [x[0] as f32, x[1] as f32, x[2] as f32, x[3] as f32]
};
pub const TIMER_COLON_X: f32 = 260.0;
/// Every timer glyph is centred on y 30.
pub const TIMER_Y: f32 = 30.0;
/// Glyph 10 of the timer file: the colon.
pub const TIMER_COLON: u8 = 10;

/// `ifCommonTimerProcDisplay`'s digits: tens of minutes, minutes, tens of
/// seconds, seconds. Below the limit the count rounds up (`+ 59`), so the
/// last second shows `0:01`; at zero it reads `0:00`.
pub fn timer_digits(time_remain: u32, limit: u32) -> [u8; 4] {
    let units = crate::d_arrays::IF_COMMON_TIMER_DIGITS_UNIT_LENGTHS.map(u32::from);
    if time_remain == 0 {
        return [0; 4];
    }
    let mut time = if time_remain == limit {
        time_remain
    } else {
        time_remain + 59
    };
    units.map(|u| {
        let d = time / u;
        time -= d * u;
        d as u8
    })
}

/// A timer glyph's top-left corner: `(s32)(x - w / 2)`, `(s32)(30 - h / 2)`.
pub fn timer_origin(x: f32, w: u16, h: u16) -> (f32, f32) {
    (
        (x - f32::from(w) * 0.5) as i32 as f32,
        (TIMER_Y - f32::from(h) * 0.5) as i32 as f32,
    )
}

/// The announcer's letters (`dIFCommonAnnounce{TimeUp,Complete,GameSet,Failure}
/// SpriteData` in `if/ifcommon.c`) as `(x, y, sprite)`, generated at build time
/// from the ROM by `crates/ssb-tablegen` (`ifcommon.rs`): `TIME_UP` and
/// `GAME_SET` are file 82's blue letters (`ssb_rom::sprite::GAME_STATUS`
/// indices), `COMPLETE` and `FAILURE` file 37's A..Z
/// (`ssb_rom::sprite::ANNOUNCE_COMMON`).
pub use generated::{COMPLETE, FAILURE, GAME_SET, TIME_UP};
mod generated {
    include!(concat!(env!("OUT_DIR"), "/if_announce.rs"));
}

/// `IFDCharacter`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Char {
    pub pos: (f32, f32),
    pub vel: (f32, f32),
    pub image_id: u8,
    pub is_lock_movement: bool,
    /// `SP_HIDDEN` on its `SObj`.
    pub hidden: bool,
}

/// `IFPlayerDamage`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DamageDisplay {
    pub player: usize,
    pub damage: i32,
    pub pos_adjust_wait: i32,
    pub flash_reset_wait: i32,
    pub scale: f32,
    pub chars: [Char; 4],
    pub color_id: usize,
    pub is_update_anim: bool,
    pub char_display_count: usize,
    pub break_anim_frame: u8,
    pub dead_stopupdate_wait: u8,
    pub is_show_interface: bool,
    /// `gIFCommonPlayerInterface.player_pos_x[player]`:
    /// [`POSITION_X`], or the 1P Game's
    /// (`sc1PGameSetPlayerInterfacePositions`).
    pub pos_x: i32,
    /// `gIFCommonPlayerInterface.player_pos_y`: [`POSITION_Y`], or How to
    /// Play's 150.
    pub pos_y: i32,
    /// `players[player].fkind == nFTKindBoss`: the display counts Master
    /// Hand's hit points down from 300 and ends in the `H.P` glyph.
    pub is_boss: bool,
}

/// One glyph to draw: `lbCommonPrepSObjDraw` of a digit `SObj`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Glyph {
    /// Index into `dIFCommonPlayerDamageDigitSpriteOffsets`.
    pub digit: u8,
    /// The sprite's top-left corner in N64 screen pixels.
    pub x: f32,
    pub y: f32,
    pub scale: f32,
    pub color: [u8; 3],
    /// The white flash: the primitive colour through the texel's alpha.
    pub solid: bool,
}

/// `ifCommonPlayerDamageGetPercentArrayID`: the decimal digits, then `%`.
fn percent_digits(damage: i32, digits: &mut [u8; 4]) -> usize {
    let n = special_digits(damage, digits);
    digits[n] = PERCENT;
    n + 1
}

/// `ifCommonPlayerDamageGetHitPointsArrayID`: the digits, then `H.P`.
fn hit_points_digits(hit_points: i32, digits: &mut [u8; 4]) -> usize {
    let n = special_digits(hit_points, digits);
    digits[n] = HIT_POINTS;
    n + 1
}

/// `ifCommonPlayerDamageGetSpecialArrayID`: the decimal digits.
fn special_digits(damage: i32, digits: &mut [u8; 4]) -> usize {
    let mut damage = damage;
    let mut unit = 1;
    if damage >= 10 {
        loop {
            unit *= 10;
            if damage / unit < 10 {
                break;
            }
        }
    }
    let mut n = 0;
    loop {
        digits[n] = (damage / unit) as u8;
        n += 1;
        damage %= unit;
        unit /= 10;
        if unit == 0 {
            break;
        }
    }
    n
}

impl DamageDisplay {
    /// `ifCommonPlayerDamageInitInterface` for one player, then its first
    /// `ProcUpdate`.
    pub fn new(player: usize, damage: i32) -> DamageDisplay {
        Self::at(player, damage, POSITION_X[player])
    }

    /// [`DamageDisplay::new`] at interface position `pos_x`.
    pub fn at(player: usize, damage: i32, pos_x: i32) -> DamageDisplay {
        Self::at_kind(player, damage, pos_x, false)
    }

    /// [`DamageDisplay::at`] for Master Hand's hit points when `is_boss`.
    pub fn at_kind(player: usize, damage: i32, pos_x: i32, is_boss: bool) -> DamageDisplay {
        Self::at_place(player, damage, pos_x, POSITION_Y, is_boss)
    }

    /// [`DamageDisplay::at_kind`] at interface height `pos_y`
    /// (`scExplainSetPlayerInterfacePositions`).
    pub fn at_place(
        player: usize,
        damage: i32,
        pos_x: i32,
        pos_y: i32,
        is_boss: bool,
    ) -> DamageDisplay {
        let mut d = DamageDisplay {
            player,
            damage,
            pos_adjust_wait: 0,
            flash_reset_wait: 0,
            scale: 1.04,
            chars: [Char::default(); 4],
            color_id: player,
            is_update_anim: false,
            char_display_count: 0,
            break_anim_frame: 0,
            dead_stopupdate_wait: 180,
            is_show_interface: false,
            pos_x,
            pos_y,
            is_boss,
        };
        d.update(damage, false);
        d
    }

    /// `ifCommonPlayerDamageProcUpdate`. `out_of_stocks` is
    /// `stock_count == -1`.
    pub fn update(&mut self, damage: i32, out_of_stocks: bool) {
        if out_of_stocks {
            if self.dead_stopupdate_wait != 0 {
                if self.is_update_anim {
                    self.update_anim();
                }
                self.dead_stopupdate_wait -= 1;
            }
        } else if self.is_update_anim {
            self.update_anim();
        } else {
            self.update_digits(damage);
        }
    }

    /// `ifCommonPlayerDamageUpdateDigits`.
    fn update_digits(&mut self, start_damage: i32) {
        let damage = start_damage.min(999);
        let mut scale = self.scale;
        let damage_scale = start_damage - self.damage;
        if damage_scale == 0 {
            if scale == 1.0 {
                return;
            }
        } else if damage_scale < 0 {
            scale = 1.0;
        } else {
            self.pos_adjust_wait = 4;
            self.flash_reset_wait = 1;
            scale = damage_scale as f32 / 300.0 + 1.0;
        }
        let mut pos_adjust_wait = self.pos_adjust_wait;
        let mut flash_reset_wait = self.flash_reset_wait;
        let color_id = if flash_reset_wait != 0 {
            PLAYERS_MAX
        } else {
            self.player
        };
        let mut digits = [0u8; 4];
        let count = if self.is_boss {
            hit_points_digits((BOSS_HIT_POINTS - damage).max(0), &mut digits)
        } else {
            percent_digits(damage, &mut digits)
        };
        self.char_display_count = count;
        let width: i32 = digits[..count]
            .iter()
            .map(|&d| DIGIT_WIDTHS[usize::from(d)])
            .sum();
        let mut pos_x = width as f32 * scale * 0.5 + self.pos_x as f32;
        if scale > 1.0 && pos_adjust_wait == 0 {
            scale -= 0.05;
            if scale < 1.0 {
                scale = 1.0;
            }
        }
        // `SObjGetStruct(interface_gobj)->next` onward: `chars[0]` holds
        // the last glyph.
        let mut digit_id = count as i32 - 1;
        for c in self.chars.iter_mut() {
            if digit_id < 0 {
                c.hidden = true;
            } else {
                let sprite_id = digits[digit_id as usize];
                c.image_id = sprite_id;
                let offset = DIGIT_WIDTHS[usize::from(sprite_id)] as f32 * scale;
                c.pos = (pos_x - offset * 0.5, self.pos_y as f32);
                pos_x -= offset;
                c.hidden = false;
            }
            digit_id -= 1;
        }
        if pos_adjust_wait > 0 {
            pos_adjust_wait -= 1;
        }
        if flash_reset_wait > 0 {
            flash_reset_wait -= 1;
        }
        self.damage = start_damage;
        self.scale = scale;
        self.color_id = color_id;
        self.pos_adjust_wait = pos_adjust_wait;
        self.flash_reset_wait = flash_reset_wait;
    }

    /// `ifCommonPlayerDamageUpdateAnim`: every sixth frame one more glyph
    /// starts to fall, until frame 19.
    fn update_anim(&mut self) {
        if self.break_anim_frame < 19 {
            let modulo = self.break_anim_frame / 6;
            if self.break_anim_frame - modulo * 6 == 0 {
                let char_id = self.char_display_count as i32 - i32::from(modulo);
                if char_id > 0 {
                    let random = rng::rand_int_range(char_id);
                    let mut j = 0;
                    let mut i = 0;
                    while i < self.char_display_count {
                        if !self.chars[i].is_lock_movement {
                            if j == random {
                                break;
                            }
                            j += 1;
                        }
                        i += 1;
                    }
                    if let Some(c) = self.chars.get_mut(i) {
                        c.is_lock_movement = true;
                    }
                }
            }
            self.break_anim_frame += 1;
        }
        for c in self.chars.iter_mut() {
            if !c.hidden && c.is_lock_movement {
                c.vel.1 += 1.0;
                c.pos.0 += c.vel.0;
                c.pos.1 += c.vel.1;
            }
        }
    }

    /// `ifCommonPlayerDamageStartBreakAnim`, from `ftCommonDeadUpdateScore`.
    pub fn start_break_anim(&mut self) {
        for c in self.chars[..self.char_display_count].iter_mut() {
            c.vel = (rng::rand_float() * 2.0 - 1.0, -10.0);
            c.is_lock_movement = false;
        }
        self.break_anim_frame = 0;
        self.is_update_anim = true;
    }

    /// [`Self::start_break_anim`] with the glyphs' `vel.x` already drawn,
    /// in the falling fighter's process ([`crate::ko::make_dead_particles`]).
    pub fn start_break_anim_from(&mut self, vel_x: &[f32; 4]) {
        for (c, &x) in self.chars[..self.char_display_count].iter_mut().zip(vel_x) {
            c.vel = (x, -10.0);
            c.is_lock_movement = false;
        }
        self.break_anim_frame = 0;
        self.is_update_anim = true;
    }

    /// `ifCommonPlayerDamageStopBreakAnim`, from the rebirth.
    pub fn stop_break_anim(&mut self) {
        self.is_update_anim = false;
        self.scale = 1.04;
    }

    /// `ifCommonPlayerDamageProcDisplay`: the glyphs this frame draws.
    /// `sizes[digit]` is each sprite's width and height.
    pub fn glyphs(&self, out_of_stocks: bool, sizes: &[(u16, u16); 12]) -> GlyphIter<'_> {
        let show = self.is_show_interface && (!out_of_stocks || self.dead_stopupdate_wait != 0);
        let solid = self.color_id == PLAYERS_MAX;
        let color = if solid {
            [
                COLORS_R[PLAYERS_MAX],
                COLORS_G[PLAYERS_MAX],
                COLORS_B[PLAYERS_MAX],
            ]
        } else {
            let k = (1.0 - self.damage as f32 / 300.0).max(0.0);
            let c = self.color_id;
            [
                (((f32::from(COLORS_R[c]) - 100.0) * k) as i32 + 100) as u8,
                (((f32::from(COLORS_G[c]) - 20.0) * k) as i32 + 20) as u8,
                (((f32::from(COLORS_B[c]) - 20.0) * k) as i32 + 20) as u8,
            ]
        };
        GlyphIter {
            display: self,
            sizes: *sizes,
            color,
            solid,
            next: if show { 0 } else { 4 },
        }
    }
}

/// The glyphs of [`DamageDisplay::glyphs`].
pub struct GlyphIter<'a> {
    display: &'a DamageDisplay,
    sizes: [(u16, u16); 12],
    color: [u8; 3],
    solid: bool,
    next: usize,
}

impl Iterator for GlyphIter<'_> {
    type Item = Glyph;

    fn next(&mut self) -> Option<Glyph> {
        let d = self.display;
        while self.next < 4 {
            let i = self.next;
            self.next += 1;
            let c = &d.chars[i];
            // The first digit `SObj` draws without an `SP_HIDDEN` check.
            if i > 0 && c.hidden {
                continue;
            }
            let (w, h) = self.sizes[usize::from(c.image_id)];
            let mut x = c.pos.0 - f32::from(w) * 0.5 * d.scale;
            let mut y = c.pos.1 - f32::from(h) * 0.5 * d.scale;
            if i > 0 && d.scale == 1.0 && !d.is_update_anim {
                x = x as i32 as f32;
                y = y as i32 as f32;
            }
            return Some(Glyph {
                digit: c.image_id,
                x,
                y,
                scale: d.scale,
                color: self.color,
                solid: self.solid,
            });
        }
        None
    }
}

#[cfg(test)]
#[path = "hud_tests.rs"]
mod tests;
