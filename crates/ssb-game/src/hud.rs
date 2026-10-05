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

/// `GMCOMMON_PLAYERS_MAX`, also the white flash colour's index.
pub const PLAYERS_MAX: usize = 4;
/// Glyph `10`: `%`. Glyph `11` is `H.P`, the Master Hand's.
pub const PERCENT: u8 = 0xA;
/// Glyph `11`: `H.P`.
pub const HIT_POINTS: u8 = 0xB;
/// Master Hand's hit points (`ifCommonPlayerDamageUpdateDigits`).
pub const BOSS_HIT_POINTS: i32 = 300;

/// `dIFCommonPlayerDamageDigitWidths`: each glyph's advance.
pub const DIGIT_WIDTHS: [i32; 12] = [14, 9, 15, 14, 15, 13, 15, 14, 15, 15, 17, 20];
/// `dIFCommonPlayerDamageDigitColors{R,G,B}`; index 4 is the flash.
const COLORS_R: [u8; 5] = [0xFF, 0xF0, 0xF0, 0xFF, 0xFF];
const COLORS_G: [u8; 5] = [0xF0, 0xFF, 0xF0, 0xFF, 0xFF];
const COLORS_B: [u8; 5] = [0xF0, 0xF0, 0xFF, 0xFF, 0xFF];
/// `dIFCommonPlayerDamagePositionOffsetsX` and
/// `ifCommonPlayerDamageSetDigitPositions`'s `player_pos_y`.
pub const POSITION_X: [i32; 4] = [55, 125, 195, 265];
pub const POSITION_Y: i32 = 210;

/// `players[].color` of a CPU outside team battles, `GMCOMMON_PLAYERS_MAX`:
/// its emblem takes the stage's fifth, grey colour.
pub const CPU_COLOR: usize = 4;

/// `dIFCommonPlayerDamageEmblemOffsets{X,Y}` (3, -3 for every player) and
/// the scale of 1: where `ifCommonPlayerDamageInitInterface` puts a `w` by
/// `h` emblem, truncated to whole pixels. It is drawn first, whether or not
/// the digits show.
pub fn emblem_origin(pos_x: i32, w: u16, h: u16) -> (f32, f32) {
    let x = (pos_x as f32 - f32::from(w) * 0.5) + 3.0;
    let y = (POSITION_Y as f32 - f32::from(h) * 0.5) - 3.0;
    (x as i32 as f32, y as i32 as f32)
}

/// `dIFCommonPlayerStocksIconOffsetsX`: -24 for every player.
const STOCK_ICON_X: i32 = -24;

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
    let base_x = pos_x + STOCK_ICON_X;
    let y = ((POSITION_Y - (f32::from(h) * 0.5) as i32) - 20) as f32;
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
pub const TIMER_X: [f32; 4] = [232.0, 247.0, 273.0, 288.0];
pub const TIMER_COLON_X: f32 = 260.0;
/// Every timer glyph is centred on y 30.
pub const TIMER_Y: f32 = 30.0;
/// Glyph 10 of the timer file: the colon.
pub const TIMER_COLON: u8 = 10;

/// `ifCommonTimerProcDisplay`'s digits: tens of minutes, minutes, tens of
/// seconds, seconds. Below the limit the count rounds up (`+ 59`), so the
/// last second shows `0:01`; at zero it reads `0:00`.
pub fn timer_digits(time_remain: u32, limit: u32) -> [u8; 4] {
    const UNITS: [u32; 4] = [36000, 3600, 600, 60];
    if time_remain == 0 {
        return [0; 4];
    }
    let mut time = if time_remain == limit {
        time_remain
    } else {
        time_remain + 59
    };
    UNITS.map(|u| {
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

/// `dIFCommonAnnounceTimeUpSpriteData` and `...GameSetSpriteData`: file
/// 82's blue letters (`ssb_rom::sprite::GAME_STATUS` indices) at their
/// top-left corners.
pub const TIME_UP: [(f32, f32, u8); 6] = [
    (45.0, 95.0, 3),
    (82.0, 95.0, 4),
    (100.0, 95.0, 5),
    (151.0, 95.0, 6),
    (195.0, 95.0, 7),
    (238.0, 95.0, 8),
];

/// `dIFCommonAnnounceCompleteSpriteData`, ANNOUNCE_COMMON's A..Z indices.
pub const COMPLETE: [(f32, f32, u8); 9] = [
    (46.0, 101.0, 2),
    (71.0, 101.0, 14),
    (104.0, 100.0, 12),
    (143.0, 101.0, 15),
    (168.0, 101.0, 11),
    (189.0, 101.0, 4),
    (212.0, 101.0, 19),
    (237.0, 101.0, 4),
    (267.0, 101.0, 26),
];
pub const GAME_SET: [(f32, f32, u8); 7] = [
    (22.0, 95.0, 11),
    (62.0, 95.0, 10),
    (104.0, 95.0, 5),
    (154.0, 95.0, 6),
    (191.0, 95.0, 9),
    (230.0, 95.0, 6),
    (262.0, 95.0, 3),
];

/// `dIFCommonAnnounceFailureSpriteData`, A..Z in ANNOUNCE_COMMON.
pub const FAILURE: [(f32, f32, u8); 7] = [
    (77.0, 101.0, 5),
    (97.0, 101.0, 0),
    (130.0, 101.0, 8),
    (145.0, 101.0, 11),
    (167.0, 101.0, 20),
    (197.0, 101.0, 17),
    (225.0, 101.0, 4),
];

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
                c.pos = (pos_x - offset * 0.5, POSITION_Y as f32);
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
