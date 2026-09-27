//! Jigglypuff's own statuses: the five aerial jumps (`ftcommonjumpaerial.c`),
//! Pound, Sing and Rest (`ftpurinspecial{n,hi,lw}.c`), with the motion-script
//! events from `relocData/232_PurinMainMotion.c` (US).
//!
//! Callbacks run in the source order: `proc_update` and `proc_interrupt`
//! ([`update`]), then `proc_physics` ([`apply_ground_physics`] /
//! [`apply_air_physics`]), then `proc_map` ([`on_ground_lost`] /
//! [`on_landing`]).
//!
//! ## Documented deviations
//!
//! * **Rapid jab.** `ftCommonAttack100CheckFighterKind` lists Jigglypuff, but
//!   her `Jab2` script never sets flag 1, so `Attack100Start` is unreachable
//!   (`ftpurin.h` calls the three statuses unused). They are not ported.
//! * **Ledges.** Pound's and Sing's grounded map callbacks
//!   (`mpCommonProcFighterOnEdge`) stop at a ledge in the source, and the
//!   aerial jumps can catch one. Fighter map collision resolves floors only,
//!   so a grounded special that runs out of floor goes airborne as for
//!   `mpCommonProcFighterOnFloor`.
//! * **Hit status.** Rest's intangibility, down smash's per-part
//!   intangibility and the throws' `SetHitStatusAll(2)` come from the motion
//!   scripts ([`crate::motion`]).
//! * **Effects.** Sing's note effect (`efManagerPurinSingMakeEffect`) is not
//!   drawn.

use ssb_engine::input::N64Buttons;
use ssb_engine::math::sin_cos;

use crate::fighter::{Fighter, FighterKind};
use crate::physics;
use crate::status::{self, AnyStatus, PurinStatus as P, StatusTiming};

/// `FTPURIN_JUMPAERIAL_VEL_MUL` and `dFTPurinJumpAerialFVelocities`.
pub const JUMPAERIAL_VEL_MUL: f32 = 0.8;
pub const JUMPAERIAL_VELOCITIES: [f32; 4] = [60.0, 40.0, 20.0, 0.0];
/// `FTPURIN_POUND_VEL_BASE` and `FTPURIN_POUND_VEL_MUL`.
pub const POUND_VEL_BASE: f32 = 65.0;
pub const POUND_VEL_MUL: f32 = 0.92;

/// `Jump2` through `Jump5`: `WaitAsync(25)`, then `SetFlag1(1)`. `Jump6`
/// sets none; it is the last jump anyway.
const JUMPAERIAL_FLAG1_FRAME: f32 = 25.0;
/// `PoundGround` (both Pound statuses): flags 1 and 2 at 12, flag 2
/// becomes 2 at 30.
pub const POUND_BOOST_FRAME: f32 = 12.0;
pub const POUND_DRIFT_FRAME: f32 = 30.0;
/// Figatree lengths (`ssb_rom::anim::EXPECTED_FRAMES`).
const JUMPAERIAL_LENGTH: f32 = 50.0;
const POUND_LENGTH: f32 = 55.0;
const SING_LENGTH: f32 = 180.0;
const REST_LENGTH: f32 = 250.0;

/// `passive_vars.purin` and the aerial jump's status vars.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct PurinState {
    /// `status_vars.common.jumpaerial.turn_tics`.
    pub jumpaerial_turn_tics: u8,
    /// `passive_vars.purin.unk_0x0`: aerial Pounds since the last landing
    /// (`mpCommonSetFighterLandingParams` clears it). Nothing reads it.
    pub pound_count: i32,
    /// Pound's motion flag 1 was consumed.
    pub pound_boosted: bool,
}

pub fn is_purin(kind: FighterKind) -> bool {
    crate::grab::base_kind(kind) == FighterKind::Purin
}

/// Which Jigglypuff statuses leave the fighter grounded.
pub fn is_grounded(s: P) -> bool {
    matches!(s, P::SpecialN | P::SpecialHi | P::SpecialLw)
}

/// `ssb_rom::anim::SLOT_PURIN_JUMP_AERIAL_F1` onward. Sing and Rest share
/// one figatree between their grounded and aerial statuses.
pub fn anim_slot(s: P) -> usize {
    const B: usize = 393;
    match s {
        P::JumpAerialF1 => B,
        P::JumpAerialF2 => B + 1,
        P::JumpAerialF3 => B + 2,
        P::JumpAerialF4 => B + 3,
        P::JumpAerialF5 => B + 4,
        P::SpecialN => B + 5,
        P::SpecialAirN => B + 6,
        P::SpecialHi | P::SpecialAirHi => B + 7,
        P::SpecialLw | P::SpecialAirLw => B + 8,
    }
}

fn set(f: &mut Fighter, s: P, frame: f32, length: f32) {
    status::set_any_status(f, AnyStatus::Purin(s), frame, StatusTiming::frames(length));
}

/// A switch that keeps the animation frame and clock.
fn switch(f: &mut Fighter, s: P) {
    let (frame, timing) = (f.status.anim_frame, f.status.timing);
    status::set_any_status(f, AnyStatus::Purin(s), frame, timing);
}

// ---------------------------------------------------------------------------
// Aerial jumps (`ftcommonjumpaerial.c`)
// ---------------------------------------------------------------------------

fn is_jump_aerial(s: AnyStatus) -> bool {
    matches!(
        s,
        AnyStatus::Purin(
            P::JumpAerialF1 | P::JumpAerialF2 | P::JumpAerialF3 | P::JumpAerialF4 | P::JumpAerialF5
        )
    )
}

/// `ftCommonJumpAerialCheckInterruptCommon`'s Jigglypuff branch.
pub fn check_jump_aerial(f: &mut Fighter) -> bool {
    if f.physics.jumps_used >= f.attributes.jumps_max {
        return false;
    }
    let jump = if f.physics.jumps_used == 1 {
        status::jump_input_type(f, status::KNEEBEND_STICK_MIN) != status::JumpInput::None
    } else {
        if is_jump_aerial(f.status.status) && f.status.anim_frame < JUMPAERIAL_FLAG1_FRAME {
            return false;
        }
        // `ftCommonJumpAerialMultiGetJumpInputType`: a held stick or a held
        // jump button, not a tap.
        i32::from(f.stick.y) >= crate::kirby::JUMPAERIAL_STICK_RANGE_MIN
            || f.input.buttons.contains(N64Buttons::C_UP)
            || f.input.buttons.contains(N64Buttons::C_DOWN)
            || f.input.buttons.contains(N64Buttons::C_LEFT)
            || f.input.buttons.contains(N64Buttons::C_RIGHT)
    };
    if jump {
        set_jump_aerial(f);
    }
    jump
}

/// `ftCommonJumpAerialMultiSetStatus`.
pub fn set_jump_aerial(f: &mut Fighter) {
    let s = match f.physics.jumps_used {
        1 => P::JumpAerialF1,
        2 => P::JumpAerialF2,
        3 => P::JumpAerialF3,
        4 => P::JumpAerialF4,
        _ => P::JumpAerialF5,
    };
    set(f, s, 0.0, JUMPAERIAL_LENGTH);
    let attr = f.attributes;
    f.physics.vel_air.x = f.input.stick_x as f32 * attr.jumpaerial_vel_x;
    if f.physics.jumps_used == 1 {
        f.physics.vel_air.y = (status::STICK_MAX as f32 * attr.jump_height_mul
            + attr.jump_height_base)
            * attr.jumpaerial_height;
        f.stick.tap_y = status::STICKBUFFER_MAX;
    } else {
        let index =
            ((f.physics.jumps_used - 2).max(0) as usize).min(JUMPAERIAL_VELOCITIES.len() - 1);
        f.physics.vel_air.y = JUMPAERIAL_VELOCITIES[index];
    }
    f.physics.jumps_used += 1;
    f.is_special_interrupt = true;
    f.purin.jumpaerial_turn_tics = if i32::from(f.input.stick_x) * (f.facing.sign() as i32)
        < crate::kirby::JUMPAERIAL_TURN_STICK_RANGE_MIN
    {
        crate::kirby::JUMPAERIAL_TURN_FRAMES
    } else {
        0
    };
    update_jump_aerial_turn(f);
}

/// `ftCommonJumpAerialUpdateModelYaw`; the yaw itself is presentation.
fn update_jump_aerial_turn(f: &mut Fighter) {
    if f.purin.jumpaerial_turn_tics == 0 {
        return;
    }
    f.purin.jumpaerial_turn_tics -= 1;
    if f.purin.jumpaerial_turn_tics == crate::kirby::JUMPAERIAL_TURN_INVERT_LR_WAIT {
        f.facing = f.facing.flipped();
    }
}

// ---------------------------------------------------------------------------
// Specials
// ---------------------------------------------------------------------------

/// `ftPurinSpecialNSetStatus` / `ftPurinSpecialAirNSetStatus`.
pub fn set_special_n(f: &mut Fighter) {
    let s = if f.is_grounded() {
        P::SpecialN
    } else {
        P::SpecialAirN
    };
    set(f, s, 0.0, POUND_LENGTH);
    f.purin.pound_boosted = false;
}

/// `ftPurinSpecialHiSetStatus` / `ftPurinSpecialAirHiSetStatus`.
pub fn set_special_hi(f: &mut Fighter) {
    let s = if f.is_grounded() {
        P::SpecialHi
    } else {
        P::SpecialAirHi
    };
    set(f, s, 0.0, SING_LENGTH);
}

/// `ftPurinSpecialLwSetStatus` / `ftPurinSpecialAirLwSetStatus`.
pub fn set_special_lw(f: &mut Fighter) {
    let s = if f.is_grounded() {
        P::SpecialLw
    } else {
        P::SpecialAirLw
    };
    set(f, s, 0.0, REST_LENGTH);
}

/// `ftPurinSpecialNGetAngle` (and `ftKirbyCopyPurinSpecialNGetAngle`).
pub fn pound_angle(stick_y: i32) -> f32 {
    let mut y = stick_y.abs().min(50) - 10;
    if y < 0 {
        y = 0;
    }
    if stick_y < 0 {
        y = -y;
    }
    (y * 20) as f32 / 40.0 * (core::f32::consts::PI / 180.0)
}

/// `ftPurinSpecialAirNProcPhysics`. Flag 2 is 0 before the boost, 1 from
/// frame 12 and 2 from frame 30. Kirby's Pound shares the body; `count` is
/// its own `copypurin_unk`.
pub(crate) fn pound_air_physics(f: &mut Fighter, boosted: &mut bool, count: &mut i32) {
    let attr = f.attributes;
    let frame = f.status.anim_frame;
    if !*boosted && frame >= POUND_BOOST_FRAME {
        *boosted = true;
        *count += 1;
        let (sin, cos) = sin_cos(pound_angle(i32::from(f.stick.y)));
        f.physics.vel_air.y = sin * POUND_VEL_BASE;
        f.physics.vel_air.x = cos * f.facing.sign() * POUND_VEL_BASE;
    }
    if frame < POUND_BOOST_FRAME {
        air_vel_friction(f);
    } else if frame < POUND_DRIFT_FRAME {
        f.physics.vel_air.y *= POUND_VEL_MUL;
        f.physics.vel_air.x *= POUND_VEL_MUL;
    } else {
        // `ftPhysicsApplyAirVelDriftFastFall`.
        status::check_set_fast_fall(f);
        gravity(f);
        physics::apply_air_drift(&mut f.physics, &attr, f.input.stick_x);
    }
}

fn gravity(f: &mut Fighter) {
    if f.physics.is_fastfall {
        physics::apply_fast_fall(&mut f.physics, &f.attributes);
    } else {
        physics::apply_gravity_default(&mut f.physics, &f.attributes);
    }
}

/// `ftPhysicsApplyAirVelFriction`.
fn air_vel_friction(f: &mut Fighter) {
    let attr = f.attributes;
    gravity(f);
    if !physics::check_clamp_air_vel_x_dec(&mut f.physics, attr.air_speed_max_x) {
        physics::apply_air_friction(&mut f.physics, &attr);
    }
}

// ---------------------------------------------------------------------------
// Callbacks
// ---------------------------------------------------------------------------

/// `proc_update` then `proc_interrupt` of Jigglypuff's statuses.
pub fn update(f: &mut Fighter) {
    let AnyStatus::Purin(current) = f.status.status else {
        return;
    };
    match current {
        P::JumpAerialF1 | P::JumpAerialF2 | P::JumpAerialF3 | P::JumpAerialF4 | P::JumpAerialF5 => {
            update_jump_aerial_turn(f);
            if f.status.animation_ended() {
                status::set_fall(f);
            } else if !status::check_special_n(f)
                && !status::check_special_hi(f)
                && !status::check_special_lw(f)
                && !status::check_attack_air(f)
            {
                check_jump_aerial(f);
            }
        }
        // `ftAnimEndSetWait`; `ftPurinSpecialHiProcUpdate` adds only the
        // note effect.
        P::SpecialN | P::SpecialHi | P::SpecialLw => {
            if f.status.animation_ended() {
                status::set_wait(f);
            }
        }
        P::SpecialAirN | P::SpecialAirHi | P::SpecialAirLw => {
            if f.status.animation_ended() {
                status::set_fall(f);
            }
        }
    }
}

/// Grounded `proc_physics`: Pound follows its figatree's root motion
/// (`ftPhysicsApplyGroundVelTransN`); Sing and Rest use the default
/// friction.
pub fn apply_ground_physics(f: &mut Fighter) -> bool {
    if f.status.status != AnyStatus::Purin(P::SpecialN) {
        return false;
    }
    physics::apply_ground_vel_transn(&mut f.physics, f.root_motion, f.facing.sign());
    true
}

/// Aerial `proc_physics`. Every Jigglypuff air status has its own.
pub fn apply_air_physics(f: &mut Fighter) -> bool {
    let AnyStatus::Purin(current) = f.status.status else {
        return false;
    };
    let attr = f.attributes;
    match current {
        // `ftCommonJumpAerialProcPhysics`'s Jigglypuff case.
        P::JumpAerialF1 | P::JumpAerialF2 | P::JumpAerialF3 | P::JumpAerialF4 | P::JumpAerialF5 => {
            status::check_set_fast_fall(f);
            gravity(f);
            if !physics::check_clamp_air_vel_x_dec(&mut f.physics, attr.air_speed_max_x) {
                physics::clamp_air_vel_x_stick_range(
                    &mut f.physics,
                    f.input.stick_x,
                    physics::AIRDRIFT_STICK_MIN,
                    attr.air_accel * JUMPAERIAL_VEL_MUL,
                    attr.air_speed_max_x * JUMPAERIAL_VEL_MUL,
                );
            }
            physics::apply_air_friction(&mut f.physics, &attr);
        }
        P::SpecialAirN => {
            let mut boosted = f.purin.pound_boosted;
            let mut count = f.purin.pound_count;
            pound_air_physics(f, &mut boosted, &mut count);
            f.purin.pound_boosted = boosted;
            f.purin.pound_count = count;
        }
        _ => air_vel_friction(f),
    }
    true
}

/// Only the aerial jumps and Pound's drift check fast fall, from their own
/// physics.
pub fn skips_fast_fall(status: AnyStatus) -> bool {
    matches!(status, AnyStatus::Purin(_))
}

/// Grounded `proc_map` when the floor ran out: `SwitchStatusAir` keeps the
/// frame and clamps the air speed.
pub fn on_ground_lost(f: &mut Fighter) -> bool {
    let AnyStatus::Purin(current) = f.status.status else {
        return false;
    };
    let air = match current {
        P::SpecialN => P::SpecialAirN,
        P::SpecialHi => P::SpecialAirHi,
        P::SpecialLw => P::SpecialAirLw,
        _ => return false,
    };
    f.become_airborne();
    switch(f, air);
    physics::clamp_air_vel_x(&mut f.physics, f.attributes.air_speed_max_x);
    true
}

/// Aerial `proc_map` on a floor contact.
pub fn on_landing(f: &mut Fighter, y: f32) -> bool {
    let AnyStatus::Purin(current) = f.status.status else {
        return false;
    };
    let ground = match current {
        P::SpecialAirN => P::SpecialN,
        P::SpecialAirHi => P::SpecialHi,
        P::SpecialAirLw => P::SpecialLw,
        // `mpCommonProcFighterCliffWaitOrLanding`.
        _ => {
            crate::kirby::wait_or_landing(f, y);
            return true;
        }
    };
    f.land(y);
    switch(f, ground);
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::attack::apply_hit_from;
    use crate::fighter::Facing;
    use crate::status::Status;
    use ssb_engine::input::ControllerState;

    fn purin(ground: bool) -> Fighter {
        let mut f = Fighter::new(FighterKind::Purin, 0, 3);
        f.attributes.jumps_max = 6;
        if ground {
            status::set_wait(&mut f);
        } else {
            f.become_airborne();
            status::set_fall(&mut f);
        }
        f
    }

    fn input(f: &mut Fighter, buttons: u16, x: i8, y: i8) {
        f.set_input(
            ControllerState {
                buttons: N64Buttons(buttons),
                stick_x: x,
                stick_y: y,
                ..Default::default()
            },
            false,
            false,
        );
    }

    fn steps(f: &mut Fighter, frames: usize) {
        for _ in 0..frames {
            status::update(f);
        }
    }

    fn close(actual: f32, expected: f32) {
        assert!((actual - expected).abs() < 0.001, "{actual} != {expected}");
    }

    #[test]
    fn b_and_stick_route_ground_and_air_specials() {
        for ground in [true, false] {
            for (stick_y, g, a) in [
                (0, P::SpecialN, P::SpecialAirN),
                (80, P::SpecialHi, P::SpecialAirHi),
                (-80, P::SpecialLw, P::SpecialAirLw),
            ] {
                let mut f = purin(ground);
                input(&mut f, N64Buttons::B, 0, stick_y);
                status::update(&mut f);
                assert_eq!(
                    f.status.status,
                    AnyStatus::Purin(if ground { g } else { a })
                );
            }
        }
    }

    #[test]
    fn pound_angle_clamps_and_offsets_the_stick() {
        close(pound_angle(0), 0.0);
        close(pound_angle(10), 0.0);
        close(pound_angle(50), 20f32.to_radians());
        close(pound_angle(80), 20f32.to_radians());
        close(pound_angle(-30), -10f32.to_radians());
    }

    #[test]
    fn aerial_pound_boosts_at_frame_12_then_decays_then_drifts() {
        let mut f = purin(false);
        f.facing = Facing::Left;
        set_special_n(&mut f);
        while f.status.anim_frame < POUND_BOOST_FRAME {
            assert!(!f.purin.pound_boosted);
            steps(&mut f, 1);
            apply_air_physics(&mut f);
        }
        assert_eq!(f.purin.pound_count, 1);
        // The boost and flag 2's first decay land on the same tick.
        close(f.physics.vel_air.x, -POUND_VEL_BASE * POUND_VEL_MUL);
        close(f.physics.vel_air.y, 0.0);
        steps(&mut f, 1);
        apply_air_physics(&mut f);
        close(
            f.physics.vel_air.x,
            -POUND_VEL_BASE * POUND_VEL_MUL * POUND_VEL_MUL,
        );
        assert_eq!(f.purin.pound_count, 1);
        f.land(0.0);
        assert_eq!(f.purin.pound_count, 0);
    }

    #[test]
    fn grounded_specials_switch_air_and_back_keeping_the_frame() {
        let mut f = purin(true);
        set_special_lw(&mut f);
        steps(&mut f, 7);
        assert!(on_ground_lost(&mut f));
        assert_eq!(f.status.status, AnyStatus::Purin(P::SpecialAirLw));
        assert_eq!(f.status.anim_frame, 7.0);
        assert!(on_landing(&mut f, 0.0));
        assert_eq!(f.status.status, AnyStatus::Purin(P::SpecialLw));
        assert_eq!(f.status.anim_frame, 7.0);
        steps(&mut f, 250);
        assert_eq!(f.status.status, AnyStatus::Common(Status::Wait));
    }

    #[test]
    fn five_aerial_jumps_use_source_velocities_and_flag1_gate() {
        let mut f = purin(false);
        f.physics.jumps_used = 1;
        // The first aerial jump reads a tapped jump button.
        f.set_input(
            ControllerState {
                buttons: N64Buttons(N64Buttons::C_UP),
                ..Default::default()
            },
            true,
            false,
        );
        assert!(check_jump_aerial(&mut f));
        assert_eq!(f.status.status, AnyStatus::Purin(P::JumpAerialF1));
        let attr = f.attributes;
        close(
            f.physics.vel_air.y,
            (80.0 * attr.jump_height_mul + attr.jump_height_base) * attr.jumpaerial_height,
        );
        for (expected, velocity) in [
            (P::JumpAerialF2, 60.0),
            (P::JumpAerialF3, 40.0),
            (P::JumpAerialF4, 20.0),
            (P::JumpAerialF5, 0.0),
        ] {
            // Held jump button, but flag 1 has not fired yet.
            input(&mut f, 0, 0, 0);
            steps(&mut f, 1);
            input(&mut f, N64Buttons::C_UP, 0, 0);
            assert!(!check_jump_aerial(&mut f));
            input(&mut f, 0, 0, 0);
            while f.status.anim_frame < JUMPAERIAL_FLAG1_FRAME {
                steps(&mut f, 1);
            }
            input(&mut f, N64Buttons::C_UP, 0, 0);
            assert!(check_jump_aerial(&mut f));
            assert_eq!(f.status.status, AnyStatus::Purin(expected));
            close(f.physics.vel_air.y, velocity);
        }
        assert_eq!(f.physics.jumps_used, 6);
        input(&mut f, 0, 0, 0);
        steps(&mut f, 30);
        input(&mut f, N64Buttons::C_UP, 0, 0);
        assert!(!check_jump_aerial(&mut f));
    }

    #[test]
    fn rest_is_intangible_for_its_first_30_frames_then_hits_hard() {
        let mut dummy = purin(true);
        set_special_lw(&mut dummy);
        let mut attacker = Fighter::new(FighterKind::Purin, 1, 3);
        attacker.pos = dummy.pos;
        status::set_wait(&mut attacker);
        set_special_lw(&mut attacker);
        steps(&mut attacker, 1);
        assert!(crate::combat::is_body_intangible(&dummy));
        assert!(!apply_hit_from(&mut attacker, &mut dummy));
        // The attacker's own Rest pulse lands on a vulnerable target.
        let mut target = Fighter::new(FighterKind::Mario, 2, 3);
        target.pos = attacker.pos;
        status::set_wait(&mut target);
        assert!(apply_hit_from(&mut attacker, &mut target));
        assert_eq!(target.damage, 20);
        steps(&mut dummy, 30);
        assert!(!crate::combat::is_body_intangible(&dummy));
    }

    #[test]
    fn sing_sleeps_grounded_targets_only_and_mashing_wakes_them() {
        let mut singer = purin(true);
        set_special_hi(&mut singer);
        steps(&mut singer, 28);
        let mut grounded = Fighter::new(FighterKind::Mario, 1, 3);
        grounded.pos = singer.pos;
        status::set_wait(&mut grounded);
        let mut airborne = Fighter::new(FighterKind::Mario, 2, 3);
        airborne.pos = singer.pos;
        airborne.become_airborne();
        status::set_fall(&mut airborne);
        assert!(!apply_hit_from(&mut singer, &mut airborne));
        assert_eq!(airborne.status.status, AnyStatus::Common(Status::Fall));
        // Sing deals no damage, so `attack_damage` stays 0 and its
        // `proc_hit` never runs; the sleep still lands.
        apply_hit_from(&mut singer, &mut grounded);
        assert_eq!(grounded.status.status, AnyStatus::Common(Status::FuraSleep));
        assert_eq!(grounded.damage, 0);
        assert_eq!(
            grounded.grab.breakout_wait,
            status::FURASLEEP_BREAKOUT_WAIT_DEFAULT + status::FURASLEEP_BREAKOUT_WAIT_MIN
        );
        // One A tap: one frame, plus one, times four.
        input(&mut grounded, N64Buttons::A, 0, 0);
        status::update(&mut grounded);
        assert_eq!(grounded.grab.breakout_wait, 375 - 1 - 4);
        input(&mut grounded, 0, 0, 0);
        steps(&mut grounded, 400);
        assert_eq!(grounded.status.status, AnyStatus::Common(Status::Wait));
    }

    #[test]
    fn jab_never_reaches_the_rapid_jab() {
        let mut f = purin(true);
        status::set_attack11(&mut f);
        for i in 0..60 {
            input(&mut f, if i % 2 == 0 { N64Buttons::A } else { 0 }, 0, 0);
            status::update(&mut f);
            assert!(matches!(f.status.status, AnyStatus::Common(_)));
        }
    }

    #[test]
    fn kirby_status_ordinals_match_the_source() {
        use crate::status::KirbyStatus as K;
        assert_eq!(K::CopyPikachuSpecialN as u16, 252);
        assert_eq!(K::CopyPikachuSpecialAirN as u16, 253);
        assert_eq!(K::CopyPurinSpecialN as u16, 293);
        assert_eq!(K::CopyPurinSpecialAirN as u16, 294);
        assert_eq!(P::SpecialAirLw as u16, 235);
    }
}
