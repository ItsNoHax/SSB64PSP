//! The CPU player — `ft/ftcomputer.c`. This part holds the machinery every
//! behaviour drives: the per-fighter `FTComputer` state,
//! `ftComputerSetupAll`'s movement predictions, the byte-script command
//! interpreter `ftComputerUpdateInputs` and its three command setters.
//! The behaviours, traits and objectives that choose the commands come on
//! top of it.
//!
//! A CPU fighter reads `fp->input.cp` instead of a controller: the
//! interpreter writes [`Computer::buttons`] and [`Computer::stick`], and
//! the host hands them to the fighter as its controller state.

pub mod attack;
pub mod behave;
mod scripts;

use ssb_engine::input::{ControllerState, N64Buttons};
use ssb_engine::math::Vec2;

use crate::fighter::{Fighter, FighterKind, Situation};
use crate::status::{AnyStatus, FoxStatus, Status};

pub use scripts::{input, INPUT_SCRIPTS};

/// `FTCOMPUTER_LEVEL_MAX`.
pub const LEVEL_MAX: u8 = 9;
/// `I_CONTROLLER_RANGE_MAX`.
pub const STICK_MAX: i8 = 80;

/// `FTCOMPUTER_COMMAND_*`: the opcode nibble.
const OP_A_PRESS: u8 = 0x00;
const OP_A_RELEASE: u8 = 0x10;
const OP_B_PRESS: u8 = 0x20;
const OP_B_RELEASE: u8 = 0x30;
const OP_Z_PRESS: u8 = 0x40;
const OP_Z_RELEASE: u8 = 0x50;
const OP_L_PRESS: u8 = 0x60;
const OP_L_RELEASE: u8 = 0x70;
const OP_START_PRESS: u8 = 0x80;
const OP_START_RELEASE: u8 = 0x90;
const OP_STICK_X: u8 = 0xA0;
const OP_STICK_Y: u8 = 0xB0;
const OP_MOVE_AUTO: u8 = 0xC0;
const OP_STICK_X_VAR: u8 = 0xD0;
const OP_STICK_Y_VAR: u8 = 0xE0;
/// `FTCOMPUTER_COMMAND_DEFAULT_MAX`: bytes from here are whole commands.
const DEFAULT_MAX: u8 = 0xF0;
const CMD_WAIT: u8 = 0xF0;
const CMD_VAR_ONE: u8 = 0xF1;
const CMD_WAIT_VAR: u8 = 0xF2;
const CMD_PK_THUNDER: u8 = 0xF3;
const CMD_END: u8 = 0xFF;
/// `FTCOMPUTER_STICK_AUTOFULL`/`_AUTOHALF`: a stick value toward the target.
const STICK_AUTO_FULL: u8 = 0x7F;
const STICK_AUTO_HALF: u8 = 0x80;

/// `nFTComputerBehavior*`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Behavior {
    #[default]
    Default,
    Unk1,
    Unk2,
    Ally,
    Captain,
    Unk3,
    Unk4,
    YoshiTeam,
    KirbyTeam,
    PolyTeam,
    Bonus3,
    Stand,
    Walk,
    Evade,
    Jump,
    Unk5,
}

/// What `ftComputerUpdateInputs`'s `MoveAuto` reads about the target.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Senses {
    /// The target fighter's status, for `ftcom_flags_0x4A_b1`'s ledge
    /// check.
    pub target_status: Option<AnyStatus>,
    /// `ftComputerGetOwnWeaponPositionKind(fp, nWPKindPKThunderTrail)`.
    pub pk_thunder_trail: Option<Vec2>,
}

/// `FTComputer`, the fields the interpreter and setup use, plus
/// `fp->input.cp` and `fp->level`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Computer {
    /// `fp->level`: 1 to 9.
    pub level: u8,
    pub behavior: Behavior,
    /// `input_wait`: ticks until the next command runs; 0 when idle.
    pub input_wait: u8,
    /// `p_command`: the running script and the byte it reads next.
    pub command: Option<(usize, usize)>,
    pub target_pos: Vec2,
    /// `target_line_id`; `None` is -1.
    pub target_line: Option<u16>,
    pub origin_pos: Vec2,
    /// `floor_line_id` at setup.
    pub floor_line: Option<u16>,
    /// How far a dash carries before it becomes a run.
    pub dash_predict: f32,
    /// How high a full jump rises, less 200.
    pub jump_predict: f32,
    /// `ftcom_flags_0x4A_b1`: stop short of a target hanging on a ledge.
    pub stop_at_ledged_target: bool,
    /// `fp->input.cp.button_inputs`.
    pub buttons: N64Buttons,
    /// `fp->input.cp.stick_range`.
    pub stick: (i8, i8),
    pub objective: behave::Objective,
    pub behavior_change_wait: u16,
    /// `target_user`: an index into [`behave::World::opponents`].
    pub target_user: Option<usize>,
    /// `target_user` when it names an item: an index into
    /// [`behave::World::items`], set by `ftComputerCheckFindItem`.
    pub target_item: Option<usize>,
    pub target_dist: f32,
    pub stand_pos: Vec2,
    pub stand_stop_wait: u16,
    pub is_stop_stand: bool,
    pub target_find_wait: u16,
    pub jump_wait: u16,
    pub is_attempt_specialhi_recovery: bool,
    pub is_within_vertical_bounds: bool,
    /// `ftcom_flags_0x49_b3`: this damage fall's landing was considered.
    pub landing_checked: bool,
    /// `ftcom_flags_0x4A_b0`: this descent's fast fall was considered.
    pub fastfall_checked: bool,
    pub is_counterattack: bool,
    pub is_shield_item_weapon: bool,
    /// `unk_ftcom_0x38`: frames until a predicted hit.
    pub hit_predict: f32,
    /// `cliff_left_pos`/`cliff_right_pos`: the lowest outer edges.
    pub cliff_left: Vec2,
    pub cliff_right: Vec2,
    /// `trait`.
    pub trait_kind: attack::Trait,
    /// `objective_base`: what `ftComputerProcDefault` falls back to.
    pub objective_base: behave::Objective,
    pub item_track_wait: u16,
    pub item_throw_wait: u16,
    /// `is_opponent_ra`: a reflector answers an incoming weapon.
    pub is_opponent_ra: bool,
    /// `fighter_follow_since`/`_wait`/`_end`: the follow pacing.
    pub follow_since: u16,
    pub follow_wait: u16,
    pub follow_end: u16,
    /// `target_gobj` and `target_damage_percent` of
    /// `ftComputerWaitGetTarget`.
    pub wait_target: Option<usize>,
    pub wait_target_damage: u16,
    pub wiggle_wait: u16,
    /// `unk_ftcom_0x35`: projectile specials fired in a row.
    pub projectile_count: u8,
    /// `unk_ftcom_0x20`: ticks left in a walking burst.
    pub walk_burst: u16,
    pub walk_stop_wait: u16,
    pub edge_pos: Vec2,
    /// `input_kind`: the last attack script chosen.
    pub input_kind: Option<usize>,
    pub input_repeat_count: u8,
    /// The per-attack weight counters.
    pub attack_counts: attack::AttackCounts,
}

impl Computer {
    /// `fp->input.cp` as the controller the fighter reads.
    pub fn controller(&self) -> ControllerState {
        ControllerState {
            buttons: self.buttons,
            stick_x: self.stick.0,
            stick_y: self.stick.1,
            connected: true,
        }
    }

    /// `ftComputerSetupAll`'s predictions from the fighter's attributes:
    /// the dash's reach before `dash_to_run`, and a full jump's height.
    /// Called when the fighter spawns. The cliff positions it also takes
    /// come with the recovery objective.
    pub fn setup(f: &Fighter, level: u8) -> Computer {
        // The US build's first behaviour change; `trait` and `behavior`
        // start at their defaults.
        let mut com = Computer {
            level,
            behavior_change_wait: 1440u16.saturating_sub(u16::from(level) * 240),
            ..Computer::default()
        };
        com.origin_pos = Vec2::new(f.pos.x, f.pos.y);
        com.target_pos = com.origin_pos;
        com.floor_line = f.floor.map(|s| s.line);
        let a = &f.attributes;
        let mut dash_speed = a.dash_speed;
        for i in 0..a.dash_to_run as i32 {
            if i >= 7 {
                dash_speed -= a.dash_decel;
            }
            com.dash_predict += dash_speed;
        }
        com.jump_predict = -200.0;
        let mut jump = a.jump_height_mul * f32::from(STICK_MAX) + a.jump_height_base;
        while jump > 0.0 {
            jump -= a.gravity;
            com.jump_predict += jump;
        }
        com
    }

    fn start(&mut self, script: usize, wait: u8) {
        self.input_wait = wait;
        self.command = Some((script, 0));
    }

    /// `ftComputerSetCommandWaitShort`: the script starts after a wait that
    /// shrinks with the level, longer on the ground.
    pub fn set_command_wait_short(&mut self, grounded: bool, script: usize) {
        let slack = f32::from(LEVEL_MAX - self.level.min(LEVEL_MAX));
        let wait = if grounded {
            2.0 * (crate::rng::rand_float() * slack) + slack * 2.0 + 1.0
        } else {
            crate::rng::rand_float() * slack
                + (u32::from(LEVEL_MAX - self.level.min(LEVEL_MAX)) / 2) as f32
                + 1.0
        };
        self.start(script, wait as u8);
    }

    /// `ftComputerSetCommandImmediate`.
    pub fn set_command_immediate(&mut self, script: usize) {
        self.start(script, 1);
    }

    /// `ftComputerSetCommandWaitLong`.
    pub fn set_command_wait_long(&mut self, grounded: bool, script: usize) {
        let slack = f32::from(LEVEL_MAX - self.level.min(LEVEL_MAX));
        let wait = if grounded {
            4.0 * (crate::rng::rand_float() * slack) + slack * 4.0 + 1.0
        } else {
            crate::rng::rand_float() * slack + slack + 1.0
        };
        self.start(script, wait as u8);
    }

    /// `ftComputerUpdateInputs`: when the wait runs out, run commands until
    /// one sets a new wait or the script ends.
    pub fn update_inputs(&mut self, f: &Fighter, senses: &Senses) {
        if self.input_wait == 0 {
            return;
        }
        self.input_wait -= 1;
        if self.input_wait != 0 {
            return;
        }
        let Some((script, mut pc)) = self.command else {
            return;
        };
        let bytes = INPUT_SCRIPTS[script];
        let byte = |pc: usize| bytes.get(pc).copied().unwrap_or(CMD_END);
        // `var_t1` is only read after `0xF1` set it; 0 stands in for the
        // uninitialised register.
        let mut var: i8 = 0;
        while self.input_wait == 0 {
            let command = byte(pc);
            pc += 1;
            if command < DEFAULT_MAX {
                self.input_wait = command & 0x0F;
                match command & 0xF0 {
                    OP_A_PRESS => self.buttons.0 |= N64Buttons::A,
                    OP_A_RELEASE => self.buttons.0 &= !N64Buttons::A,
                    OP_B_PRESS => self.buttons.0 |= N64Buttons::B,
                    OP_B_RELEASE => self.buttons.0 &= !N64Buttons::B,
                    OP_Z_PRESS => self.buttons.0 |= N64Buttons::Z,
                    OP_Z_RELEASE => self.buttons.0 &= !N64Buttons::Z,
                    OP_L_PRESS => self.buttons.0 |= N64Buttons::L,
                    OP_L_RELEASE => self.buttons.0 &= !N64Buttons::L,
                    OP_START_PRESS => self.buttons.0 |= N64Buttons::START,
                    OP_START_RELEASE => self.buttons.0 &= !N64Buttons::START,
                    OP_STICK_X => {
                        self.stick.0 = self.stick_value(byte(pc), f.pos.x, self.target_pos.x);
                        pc += 1;
                    }
                    OP_STICK_Y => {
                        self.stick.1 = self.stick_value(byte(pc), f.pos.y, self.target_pos.y);
                        pc += 1;
                    }
                    OP_MOVE_AUTO => self.move_auto(f, senses),
                    OP_STICK_X_VAR => self.stick.0 = var,
                    OP_STICK_Y_VAR => self.stick.1 = var,
                    _ => {}
                }
            } else {
                match command {
                    CMD_WAIT => {
                        self.input_wait = byte(pc);
                        pc += 1;
                    }
                    CMD_VAR_ONE => var = 1,
                    CMD_WAIT_VAR => self.input_wait = var as u8,
                    CMD_PK_THUNDER => self.control_pk_thunder(senses),
                    CMD_END => {
                        self.input_wait = 0;
                        self.command = None;
                        return;
                    }
                    _ => {}
                }
            }
        }
        self.command = Some((script, pc));
    }

    /// A stick event's value byte: a raw value, or full or half range toward
    /// the target on that axis.
    fn stick_value(&self, value: u8, at: f32, target: f32) -> i8 {
        let sign = if at < target { 1 } else { -1 };
        match value {
            STICK_AUTO_FULL => sign * STICK_MAX,
            STICK_AUTO_HALF => sign * (STICK_MAX / 2),
            v => v as i8,
        }
    }

    /// `FTCOMPUTER_COMMAND_MOVEAUTO`: point the stick at the target, scaled
    /// by how far a dash or a drift carries.
    fn move_auto(&mut self, f: &Fighter, senses: &Senses) {
        let dist_x = self.target_pos.x - f.pos.x;
        let mut dist_y = self.target_pos.y - f.pos.y;
        let full = f32::from(STICK_MAX);
        let grounded = f.situation == Situation::Ground;
        let stick_x: f32 = if grounded && self.level < 5 {
            if dist_x.abs() > 100.0 {
                full / 2.0
            } else {
                0.0
            }
        } else if grounded {
            if self.dash_predict * 1.5 < dist_x.abs() {
                full
            } else if self.dash_predict < dist_x.abs() {
                // `(s16)` of the float expression.
                ((2.0 * ((dist_x.abs() - self.dash_predict) / self.dash_predict) * (full / 2.0))
                    + (full / 2.0)) as i16 as f32
            } else if dist_x.abs() > 100.0 {
                full / 2.0
            } else {
                0.0
            }
        } else if dist_x.abs() > 100.0 || f.facing.sign() * dist_x < 0.0 {
            full
        } else {
            full / 4.0
        };
        let mut stick_y = full;
        if grounded {
            if f.status.status != AnyStatus::Common(Status::KneeBend) {
                if self.target_line.is_some() && self.target_line == f.floor.map(|s| s.line) {
                    stick_y = 0.0;
                    dist_y = 0.0;
                }
                if self.stop_at_ledged_target
                    && matches!(
                        senses.target_status,
                        Some(AnyStatus::Common(Status::CliffCatch | Status::CliffWait))
                    )
                {
                    stick_y = 0.0;
                    dist_y = 0.0;
                }
            }
        } else {
            let fox_special_hi = f.kind == FighterKind::Fox
                && matches!(
                    f.status.status,
                    AnyStatus::Fox(
                        FoxStatus::SpecialHiStart
                            | FoxStatus::SpecialAirHiStart
                            | FoxStatus::SpecialHiHold
                            | FoxStatus::SpecialAirHiHold
                    )
                );
            if !fox_special_hi && dist_y < 0.0 {
                stick_y = 0.0;
                dist_y = 0.0;
            }
            match self.behavior {
                Behavior::YoshiTeam if f.pos.y < 0.0 => {
                    stick_y = 0.0;
                    dist_y = 0.0;
                }
                Behavior::KirbyTeam | Behavior::PolyTeam if f.pos.y < -300.0 => {
                    stick_y = 0.0;
                    dist_y = 0.0;
                }
                _ => {}
            }
        }
        let signed = |v: f32, d: f32| if d > 0.0 { v } else { -v };
        let (x, y) = if dist_x != 0.0 && dist_y != 0.0 {
            if dist_y.abs() < dist_x.abs() {
                (
                    signed(stick_x, dist_x),
                    (dist_y / dist_x).abs() * signed(stick_y, dist_y),
                )
            } else {
                (
                    (dist_x / dist_y).abs() * signed(stick_x, dist_x),
                    signed(stick_y, dist_y),
                )
            }
        } else if dist_x != 0.0 {
            (
                signed(stick_x, dist_x),
                (dist_y / dist_x).abs() * signed(stick_y, dist_y),
            )
        } else if dist_y != 0.0 {
            (
                (dist_x / dist_y).abs() * signed(stick_x, dist_x),
                signed(stick_y, dist_y),
            )
        } else {
            (0.0, 0.0)
        };
        self.stick = (x as i8, y as i8);
    }

    /// `ftComputerSetControlPKThunder`: steer the trail toward the target.
    fn control_pk_thunder(&mut self, senses: &Senses) {
        self.stick = match senses.pk_thunder_trail {
            Some(pos) => {
                let dx = self.target_pos.x - pos.x;
                let dy = self.target_pos.y - pos.y;
                let scale = inverse_length_or_zero(dx * dx + dy * dy);
                let full = f32::from(STICK_MAX);
                ((full * dx * scale) as i8, (full * dy * scale) as i8)
            }
            None => (0, 0),
        };
    }
}

/// `1.0F / sqrtf(len_sq)` for `ftComputerSetControlPKThunder`, with 0 for
/// a trail already on the target. The N64 masks FPU exceptions, so there
/// the stick is `0 * inf`, a NaN its `(s8)` conversion takes. A PSP traps
/// the division and the `0 * inf` (RE-469), and LLVM computes this scale
/// even on the frames the command does not run, so the denominator stays
/// nonzero out of line, where it cannot be folded into an infinity. A
/// scale of 0 gives the stick Rust's conversion of that NaN: 0.
#[inline(never)]
fn inverse_length_or_zero(len_sq: f32) -> f32 {
    if len_sq > 0.0 {
        1.0 / ssb_engine::math::sqrt(len_sq)
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests;
