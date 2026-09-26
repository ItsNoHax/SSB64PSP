//! Captain Falcon's extended statuses (`ftcaptainspecial{n,hi,lw}.c` and
//! `ftcommonattack100.c`). Motion-event frames are from file 235 (US).

use ssb_engine::input::{newly_pressed, newly_released, N64Buttons};
use ssb_engine::math::{sin_cos, Vec3};

use crate::attack::Hitbox;
use crate::fighter::Fighter;
use crate::grab::{GrabEvent, ThrowHitDesc};
use crate::physics;
use crate::status::{self, AnyStatus, CaptainStatus, StatusTiming};

const PUNCH_VEL_BASE: f32 = 65.0;
const PUNCH_VEL_MUL: f32 = 0.92;
const DIVE_DRIFT: f32 = 0.72;
const DIVE_LANDING_LAG: f32 = 0.65;
const DIVE_AIR_ACCEL_MUL: f32 = 1.1;
const DIVE_AIR_SPEED_MUL: f32 = 0.8;
const DIVE_TURN_STICK_MIN: i32 = 18;

/// `dCaptainMainMotion_0x1C54`: the Falcon Dive victim knockback and the
/// damage-release descriptor if Captain is hit while holding.
pub const DIVE_THROW: [ThrowHitDesc; 2] = [
    ThrowHitDesc {
        status: None,
        damage: 20,
        angle: 361,
        kb_scale: 82,
        kb_weight: 0,
        kb_base: 30,
    },
    ThrowHitDesc {
        status: None,
        damage: 8,
        angle: 361,
        kb_scale: 100,
        kb_weight: 0,
        kb_base: 0,
    },
];

/// The two root-attached catch boxes at Falcon Dive frame 13. The second is
/// cleared one frame later; the first remains until frame 45.
pub const DIVE_CATCH: [(Hitbox, u8); 2] = [
    (
        Hitbox {
            damage: 1,
            radius: 100.0,
            offset: Vec3::new(0.0, 260.0, 180.0),
            angle: 361,
            kb_scale: 100,
            kb_weight: 0,
            kb_base: 0,
        },
        0,
    ),
    (
        Hitbox {
            damage: 1,
            radius: 150.0,
            offset: Vec3::new(0.0, 260.0, 400.0),
            angle: 361,
            kb_scale: 100,
            kb_weight: 0,
            kb_base: 0,
        },
        0,
    ),
];

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CaptainState {
    pub rapid_is_anim_end: bool,
    pub rapid_is_goto_loop: bool,
    pub punch_launched: bool,
    pub kick_scale: f32,
    pub kick_hit_count: u8,
    pub dive_vel: Vec3,
    pub dive_turn_done: bool,
    pub dive_cliff_wait: u8,
    /// Grounded Falcon Dive victim's TopN, held still during the catch.
    pub dive_target_pos: Option<Vec3>,
    pub dive_target_kind: Option<crate::fighter::FighterKind>,
}

impl Default for CaptainState {
    fn default() -> Self {
        Self {
            rapid_is_anim_end: false,
            rapid_is_goto_loop: false,
            punch_launched: false,
            kick_scale: 1.0,
            kick_hit_count: 0,
            dive_vel: Vec3::ZERO,
            dive_turn_done: false,
            dive_cliff_wait: 15,
            dive_target_pos: None,
            dive_target_kind: None,
        }
    }
}

fn set(f: &mut Fighter, s: CaptainStatus, frame: f32, length: f32) {
    status::set_any_status(
        f,
        AnyStatus::Captain(s),
        frame,
        StatusTiming::frames(length),
    );
}

pub fn set_attack100_start(f: &mut Fighter) {
    set(f, CaptainStatus::Attack100Start, 0.0, 6.0);
    f.captain.rapid_is_anim_end = false;
    f.captain.rapid_is_goto_loop = false;
}

fn set_attack100_loop(f: &mut Fighter) {
    set(f, CaptainStatus::Attack100Loop, 0.0, 37.0);
}

fn crossed(f: &Fighter, at: f32) -> bool {
    f.status.anim_frame >= at && f.status.anim_frame - f.status.timing.anim_speed < at
}

pub fn set_special_n(f: &mut Fighter) {
    set(f, CaptainStatus::SpecialN, 0.0, 90.0);
    f.captain.punch_launched = false;
}

pub fn set_special_air_n(f: &mut Fighter) {
    set(f, CaptainStatus::SpecialAirN, 0.0, 90.0);
    f.captain.punch_launched = false;
}

pub fn set_special_lw(f: &mut Fighter) {
    set(f, CaptainStatus::SpecialLw, 0.0, 85.0);
    f.captain.kick_scale = 1.0;
    f.captain.kick_hit_count = 0;
}

pub fn set_special_air_lw(f: &mut Fighter) {
    set(f, CaptainStatus::SpecialAirLw, 0.0, 50.0);
    f.captain.kick_scale = 1.0;
    f.captain.kick_hit_count = 0;
}

pub fn set_special_hi(f: &mut Fighter) {
    // Grounded Falcon Dive first enters air, and spends every jump.
    f.become_airborne();
    set(f, CaptainStatus::SpecialHi, 0.0, 65.0);
    f.physics.jumps_used = f.attributes.jumps_max;
    f.captain.dive_vel = Vec3::ZERO;
    f.captain.dive_turn_done = false;
    f.captain.dive_cliff_wait = 15;
    f.captain.dive_target_pos = None;
    f.captain.dive_target_kind = None;
    f.grab.is_catchstatus = true;
    f.grab.throw_desc = Some(DIVE_THROW);
}

pub fn set_special_air_hi(f: &mut Fighter) {
    set_special_hi(f);
    set(f, CaptainStatus::SpecialAirHi, 0.0, 65.0);
    f.grab.is_catchstatus = true;
    f.grab.throw_desc = Some(DIVE_THROW);
}

pub fn dive_searching(f: &Fighter) -> bool {
    matches!(
        f.status.status,
        AnyStatus::Captain(CaptainStatus::SpecialHi | CaptainStatus::SpecialAirHi)
    ) && (13.0..45.0).contains(&f.status.anim_frame)
}

pub fn dive_catch(f: &mut Fighter, held: &Fighter) {
    set(f, CaptainStatus::SpecialHiCatch, 0.0, 16.0);
    f.grab.capture_immune = true;
    f.grab.is_catchstatus = false;
    f.grab.catch = Some(held.port);
    f.grab.catch_kind = Some(held.kind);
    f.captain.dive_target_pos = held.is_grounded().then_some(held.pos);
    f.captain.dive_target_kind = Some(held.kind);
    f.grab.send(GrabEvent::CaptureCaptain);
    physics::stop_all(&mut f.physics);
}

fn dive_throw(f: &mut Fighter) {
    set(f, CaptainStatus::SpecialHiThrow, 0.0, 60.0);
    f.grab.capture_immune = false;
    if f.grab.catch.take().is_some() {
        f.grab.send(GrabEvent::Release {
            lr: f.facing.sign(),
            desc: DIVE_THROW[0],
            shield_catch: false,
        });
    }
    f.grab.catch_kind = None;
    f.captain.dive_target_pos = None;
    f.captain.dive_target_kind = None;
}

/// `ftCaptainSpecialLwProcHit`: each of the first six contact callbacks halves
/// the air velocity multiplier. The source scales `vel_air` even on ground,
/// so grounded `vel_ground` is deliberately unaffected.
pub fn on_kick_hit(f: &mut Fighter) {
    if matches!(
        f.status.status,
        AnyStatus::Captain(CaptainStatus::SpecialLw | CaptainStatus::SpecialLwAir)
    ) && f.captain.kick_hit_count < 6
    {
        f.captain.kick_hit_count += 1;
        f.captain.kick_scale /= 2.0;
    }
}

pub fn update(f: &mut Fighter) {
    let AnyStatus::Captain(current) = f.status.status else {
        return;
    };
    match current {
        CaptainStatus::Attack13 => {
            status::rapid_input(f);
            if f.status.anim_frame >= crate::captain_attack::JAB3_FLAG1_FRAME
                && f.attack1.rapid_requested
            {
                set_attack100_start(f);
            } else if f.status.animation_ended() {
                status::set_wait(f);
            }
        }
        CaptainStatus::Attack100Start => {
            if f.status.animation_ended() {
                set_attack100_loop(f);
            }
        }
        CaptainStatus::Attack100Loop => {
            if f.status.animation_ended() {
                f.captain.rapid_is_anim_end = true;
            }
            let flag = [4.0, 12.0, 20.0, 28.0, 35.0]
                .into_iter()
                .any(|at| crossed(f, at));
            if flag {
                if f.captain.rapid_is_anim_end && !f.captain.rapid_is_goto_loop {
                    set(f, CaptainStatus::Attack100End, 0.0, 9.0);
                    return;
                }
                f.captain.rapid_is_goto_loop = false;
            }
            if f.status.animation_ended() {
                set_attack100_loop(f);
            }
            let taps = newly_pressed(f.prev_input.buttons, f.input.buttons);
            let releases = newly_released(f.prev_input.buttons, f.input.buttons);
            if taps.contains(N64Buttons::A) || releases.contains(N64Buttons::A) {
                f.captain.rapid_is_goto_loop = true;
            }
        }
        CaptainStatus::Attack100End | CaptainStatus::SpecialN | CaptainStatus::SpecialLwLanding => {
            if f.status.animation_ended() {
                status::set_wait(f);
            }
        }
        CaptainStatus::SpecialAirN
        | CaptainStatus::SpecialAirLw
        | CaptainStatus::SpecialLwBound
        | CaptainStatus::SpecialHiThrow => {
            if f.status.animation_ended() {
                status::set_fall(f);
            }
        }
        CaptainStatus::SpecialLw | CaptainStatus::SpecialLwAir => {
            if current == CaptainStatus::SpecialLw
                && f.is_grounded() == false
                && f.status.anim_frame >= 32.0
            {
                set(f, CaptainStatus::SpecialLwAir, 0.0, 30.0);
            } else if f.status.animation_ended() {
                status::set_wait_or_fall(f);
            }
        }
        CaptainStatus::SpecialHi | CaptainStatus::SpecialAirHi => {
            if !f.captain.dive_turn_done && f.status.anim_frame >= 13.0 {
                f.captain.dive_turn_done = true;
                if (f.input.stick_x as i32).abs() > DIVE_TURN_STICK_MIN {
                    f.facing = if f.input.stick_x < 0 {
                        crate::fighter::Facing::Left
                    } else {
                        crate::fighter::Facing::Right
                    };
                }
            }
            if f.status.animation_ended() {
                f.grab.is_catchstatus = false;
                status::set_fall_special(f, DIVE_DRIFT, false, true, DIVE_LANDING_LAG, false);
            }
        }
        CaptainStatus::SpecialHiCatch => {
            if f.status.animation_ended() {
                dive_throw(f);
            }
        }
    }
}

pub fn apply_ground_physics(f: &mut Fighter) -> bool {
    match f.status.status {
        AnyStatus::Captain(
            CaptainStatus::SpecialN
            | CaptainStatus::SpecialLw
            | CaptainStatus::SpecialLwAir
            | CaptainStatus::SpecialLwLanding,
        ) => {
            physics::apply_ground_vel_transn(&mut f.physics, f.root_motion, f.facing.sign());
            true
        }
        _ => false,
    }
}

pub fn apply_air_physics(f: &mut Fighter) -> bool {
    let AnyStatus::Captain(current) = f.status.status else {
        return false;
    };
    match current {
        CaptainStatus::SpecialAirN => {
            if !f.captain.punch_launched && f.status.anim_frame >= 40.0 {
                f.captain.punch_launched = true;
                let y = i32::from(f.input.stick_y);
                let magnitude = (y.abs().min(50) - 10).max(0) as f32;
                let angle = magnitude * 30.0 / 40.0
                    * (core::f32::consts::PI / 180.0)
                    * if y < 0 { -1.0 } else { 1.0 };
                let (sin, cos) = sin_cos(angle);
                f.physics.vel_air.x = cos * f.facing.sign() * PUNCH_VEL_BASE;
                f.physics.vel_air.y = sin * PUNCH_VEL_BASE;
            }
            match f.status.anim_frame {
                frame if frame < 40.0 => {
                    physics::apply_gravity_default(&mut f.physics, &f.attributes);
                    if !physics::check_clamp_air_vel_x_dec(
                        &mut f.physics,
                        f.attributes.air_speed_max_x,
                    ) {
                        physics::apply_air_friction(&mut f.physics, &f.attributes);
                    }
                }
                frame if frame >= 55.0 => {
                    status::check_set_fast_fall(f);
                    if f.physics.is_fastfall {
                        physics::apply_fast_fall(&mut f.physics, &f.attributes);
                    } else {
                        physics::apply_gravity_default(&mut f.physics, &f.attributes);
                    }
                    physics::apply_air_drift(&mut f.physics, &f.attributes, f.input.stick_x);
                }
                _ => {
                    f.physics.vel_air.x *= PUNCH_VEL_MUL;
                    f.physics.vel_air.y *= PUNCH_VEL_MUL;
                }
            }
        }
        CaptainStatus::SpecialLw
        | CaptainStatus::SpecialLwAir
        | CaptainStatus::SpecialAirLw
        | CaptainStatus::SpecialLwBound
        | CaptainStatus::SpecialHiThrow => {
            let coast = matches!(current, CaptainStatus::SpecialLw) && f.status.anim_frame >= 36.0
                || matches!(current, CaptainStatus::SpecialLwAir) && f.status.anim_frame >= 16.0;
            if coast {
                physics::apply_gravity_default(&mut f.physics, &f.attributes);
                if !physics::check_clamp_air_vel_x_dec(&mut f.physics, f.attributes.air_speed_max_x)
                {
                    physics::apply_air_friction(&mut f.physics, &f.attributes);
                }
            } else {
                physics::apply_air_vel_transn_all(&mut f.physics, f.root_motion, f.facing.sign());
            }
            if matches!(
                current,
                CaptainStatus::SpecialLw | CaptainStatus::SpecialLwAir
            ) {
                f.physics.vel_air.x *= f.captain.kick_scale;
                f.physics.vel_air.y *= f.captain.kick_scale;
            }
        }
        CaptainStatus::SpecialHi | CaptainStatus::SpecialAirHi => {
            f.physics.vel_air.x = f.captain.dive_vel.x;
            f.physics.vel_air.y = f.captain.dive_vel.y;
            let max = f.attributes.air_speed_max_x * DIVE_AIR_SPEED_MUL;
            if !physics::check_clamp_air_vel_x_dec(&mut f.physics, max) {
                physics::clamp_air_vel_x_stick_range(
                    &mut f.physics,
                    f.input.stick_x,
                    physics::AIRDRIFT_STICK_MIN,
                    f.attributes.air_accel * DIVE_AIR_ACCEL_MUL,
                    max,
                );
                physics::apply_air_friction(&mut f.physics, &f.attributes);
            }
            f.captain.dive_vel = f.physics.vel_air;
            physics::apply_air_vel_transn_all(&mut f.physics, f.root_motion, f.facing.sign());
            f.physics.vel_air.x += f.captain.dive_vel.x;
            f.physics.vel_air.y += f.captain.dive_vel.y;
            if f.captain.dive_cliff_wait > 0 {
                f.captain.dive_cliff_wait -= 1;
            }
        }
        CaptainStatus::SpecialHiCatch => {
            physics::stop_all(&mut f.physics);
            if let (Some(target), Some(kind)) =
                (f.captain.dive_target_pos, f.captain.dive_target_kind)
            {
                let offset = crate::grab::captain_offset(kind);
                let anchor = f.grab.anchor.unwrap_or(f.pos);
                let want = target + Vec3::new(offset.x * f.facing.sign(), offset.y, 0.0)
                    - (anchor - f.pos);
                let delta = want - f.pos;
                let d2 = delta.length_squared();
                f.pos += if d2 > 180.0 * 180.0 {
                    delta * (180.0 / ssb_engine::math::sqrt(d2))
                } else {
                    delta
                };
            }
        }
        _ => return false,
    }
    true
}

pub fn skips_fast_fall(status: AnyStatus) -> bool {
    matches!(status, AnyStatus::Captain(_))
}

pub fn on_ground_lost(f: &mut Fighter) -> bool {
    let AnyStatus::Captain(current) = f.status.status else {
        return false;
    };
    match current {
        CaptainStatus::SpecialN => {
            let frame = f.status.anim_frame;
            f.become_airborne();
            set(f, CaptainStatus::SpecialAirN, frame, 90.0);
            physics::clamp_air_vel_x(&mut f.physics, f.attributes.air_speed_max_x);
        }
        CaptainStatus::SpecialLw => {
            f.become_airborne();
            f.physics.jumps_used = 1;
        }
        _ => return false,
    }
    true
}

pub fn on_landing(f: &mut Fighter, y: f32) -> bool {
    let AnyStatus::Captain(current) = f.status.status else {
        return false;
    };
    match current {
        CaptainStatus::SpecialAirN => {
            let frame = f.status.anim_frame;
            f.land(y);
            set(f, CaptainStatus::SpecialN, frame, 90.0);
        }
        CaptainStatus::SpecialAirLw => {
            f.land(y);
            set(f, CaptainStatus::SpecialLwLanding, 0.0, 45.0);
        }
        CaptainStatus::SpecialLw | CaptainStatus::SpecialLwAir | CaptainStatus::SpecialLwBound => {
            f.land(y);
        }
        CaptainStatus::SpecialHi | CaptainStatus::SpecialAirHi => {
            if f.physics.vel_air.y < 0.0 && f.captain.dive_cliff_wait == 0 {
                f.land(y);
                status::set_landing(f);
            } else {
                f.pos.y = y;
                f.floor = None;
            }
        }
        CaptainStatus::SpecialHiThrow => {
            f.land(y);
            status::set_wait(f);
        }
        _ => return false,
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fighter::FighterKind;

    fn captain() -> Fighter {
        Fighter::new(FighterKind::Captain, 0, 4)
    }

    #[test]
    fn punch_launches_at_script_flag_and_then_damps() {
        let mut f = captain();
        f.become_airborne();
        set_special_air_n(&mut f);
        f.status.anim_frame = 39.0;
        apply_air_physics(&mut f);
        assert!(!f.captain.punch_launched);
        f.status.anim_frame = 40.0;
        apply_air_physics(&mut f);
        assert!(f.captain.punch_launched);
        assert!((f.physics.vel_air.x - 65.0 * 0.92).abs() < 0.001);
    }

    #[test]
    fn kick_hit_scale_stops_after_six_contacts() {
        let mut f = captain();
        set_special_lw(&mut f);
        for _ in 0..7 {
            on_kick_hit(&mut f);
        }
        assert_eq!(f.captain.kick_hit_count, 6);
        assert_eq!(f.captain.kick_scale, 1.0 / 64.0);
        f.root_motion.delta.z = 20.0;
        apply_ground_physics(&mut f);
        assert_eq!(f.physics.vel_ground.x, 20.0);
        f.become_airborne();
        apply_air_physics(&mut f);
        assert_eq!(f.physics.vel_air.x, 20.0 / 64.0);
    }

    #[test]
    fn kick_stops_using_root_motion_after_flag_zero() {
        let mut f = captain();
        set_special_lw(&mut f);
        f.become_airborne();
        f.root_motion.delta.z = 20.0;
        f.status.anim_frame = 35.0;
        apply_air_physics(&mut f);
        assert_eq!(f.physics.vel_air.x, 20.0);
        f.root_motion.delta.z = 100.0;
        f.status.anim_frame = 36.0;
        apply_air_physics(&mut f);
        assert!(f.physics.vel_air.x < 20.0);
    }

    #[test]
    fn dive_catch_moves_toward_grounded_victim_only() {
        let mut f = captain();
        let mut victim = Fighter::new(FighterKind::Mario, 1, 4);
        victim.situation = crate::fighter::Situation::Ground;
        victim.pos = Vec3::new(500.0, 0.0, 0.0);
        set_special_hi(&mut f);
        dive_catch(&mut f, &victim);
        apply_air_physics(&mut f);
        assert!(f.pos.x > 0.0 && f.pos.x <= 180.0);

        let mut airborne = captain();
        victim.become_airborne();
        set_special_hi(&mut airborne);
        dive_catch(&mut airborne, &victim);
        apply_air_physics(&mut airborne);
        assert_eq!(airborne.pos, Vec3::ZERO);
    }
}
