//! Samus's specials: `ftsamusspecialn.c` (Charge Shot), `ftsamusspecialhi.c`
//! (Screw Attack) and `ftsamusspeciallw.c` (Bomb), with the motion-script
//! events from `relocData/216_SamusMainMotion.c` (US).
//!
//! Callbacks run in the source order: `proc_update`, then `proc_interrupt`
//! ([`update`]), then `proc_physics` ([`apply_air_physics`]), then
//! `proc_map` ([`on_ground_lost`] / [`on_landing`]). Every grounded Samus
//! status uses `ftPhysicsApplyGroundVelFriction`: Bomb's flag-3 drift needs
//! the grounded script past frame 10, but that script leaves the ground at
//! frame 3 and a landing fast-forwards past its flag events.
//!
//! The charging Charge Shot is a weapon with no attack collision in the
//! source (`nGMAttackStateOff`) that follows joint 16. Only its existence and
//! size matter to gameplay, so the port keeps it as [`SamusState::charge_shot`]
//! and asks the match-owned weapon pool for a launched shot on release. A
//! full weapon pool therefore refuses the shot at release rather than at the
//! start of the charge.

use ssb_engine::input::{newly_pressed, N64Buttons};
use ssb_engine::math::Vec3;

use crate::fighter::{Fighter, Situation};
use crate::physics;
use crate::status::{self, AnyStatus, SamusStatus, StatusTiming};
use crate::weapon::{WeaponKind, WeaponSpawn};

/// `FTSAMUS_CHARGE_JOINT`: Charge Shot's attachment joint.
pub const CHARGE_JOINT: u8 = 16;
/// `FTSAMUS_CHARGE_MAX`.
pub const CHARGE_MAX: u8 = 7;
/// `FTSAMUS_CHARGE_INT`: frames per charge level.
pub const CHARGE_INT: u8 = 20;
/// `FTSAMUS_CHARGE_OFF_X`.
pub const CHARGE_OFF_X: f32 = 180.0;
pub const CHARGE_RECOIL_BASE: f32 = 10.0;
pub const CHARGE_RECOIL_MUL: f32 = 2.0;
pub const CHARGE_RECOIL_ADD: f32 = 20.0;

pub const SCREWATTACK_DRIFT_MUL: f32 = 0.5;
pub const SCREWATTACK_DRIFT_CLAMP: f32 = 20.0;
pub const SCREWATTACK_VEL_X_BASE: f32 = 10.0;
pub const SCREWATTACK_VEL_Y_BASE: f32 = 62.0;
pub const SCREWATTACK_FALLSPECIAL_DRIFT: f32 = 0.66;
pub const SCREWATTACK_LANDING_LAG: f32 = 0.4;

pub const BOMB_OFF_Y: f32 = 60.0;
pub const BOMB_VEL_Y_BASE: f32 = 40.0;
pub const BOMB_VEL_Y_SUB: f32 = 10.0;
pub const BOMB_DRIFT: f32 = 0.66;

/// `ScrewAttackGround`: `WaitAsync(4)` then `SetAirJumpMax(0)` and
/// `SetFlag1(1)`.
const SCREW_TAKEOFF_FRAME: f32 = 4.0;
/// `Bomb`: `WaitAsync(3)` then `SetAirJumpMax(0)`.
const BOMB_HOP_FRAME: f32 = 3.0;
/// Both Bomb scripts: `WaitAsync(10)` then `SetFlag0(1)`.
const BOMB_SPAWN_FRAME: f32 = 10.0;

/// Figatree lengths (`ssb_rom::anim::EXPECTED_FRAMES`).
const SPECIAL_N_START_LENGTH: f32 = 16.0;
const SPECIAL_N_END_LENGTH: f32 = 30.0;
const SPECIAL_AIR_N_END_LENGTH: f32 = 29.0;
const SPECIAL_HI_LENGTH: f32 = 50.0;
const SPECIAL_AIR_HI_LENGTH: f32 = 48.0;
const SPECIAL_LW_LENGTH: f32 = 57.0;

/// `FTSamusPassiveVars` plus the per-status Charge Shot, Screw Attack and
/// Bomb flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SamusState {
    /// `passive_vars.samus.charge_level`, kept between uses.
    pub charge_level: u8,
    /// `passive_vars.samus.charge_recoil`: aerial shots since landing.
    pub charge_recoil: i32,
    /// `status_vars.samus.specialn.is_release`.
    pub is_release: bool,
    /// `status_vars.samus.specialn.charge_int`.
    pub charge_int: u8,
    /// `charge_gobj != NULL`: a charging shot follows joint 16.
    pub charge_shot: bool,
    /// `proc_damage == ftSamusSpecialNProcDamage`.
    pub damage_resets_charge: bool,
    /// Motion flag 0 of `Shooting`/`ShootingAir` was consumed.
    pub shot_fired: bool,
    /// `SetAirJumpMax` of the grounded script has not yet run.
    pub takeoff_pending: bool,
    /// Motion flag 0 of either Bomb script was consumed.
    pub bomb_spawned: bool,
}

fn taps(f: &Fighter) -> N64Buttons {
    newly_pressed(f.prev_input.buttons, f.input.buttons)
}

fn set(f: &mut Fighter, status: SamusStatus, frame: f32, timing: StatusTiming) {
    status::set_any_status(f, AnyStatus::Samus(status), frame, timing);
}

/// `ftSamusSpecialNStartGetAnimSpeed`.
fn start_anim_speed(charge_level: u8) -> f32 {
    let ret = f32::from(charge_level) / f32::from(CHARGE_MAX);
    -0.160_000_03 * ret + 1.0
}

/// `ftSamusSpecialNStartSetStatus` / `ftSamusSpecialAirNStartSetStatus`.
pub fn set_special_n(f: &mut Fighter) {
    let ground = f.is_grounded();
    let speed = start_anim_speed(f.samus.charge_level);
    let status = if ground {
        SamusStatus::SpecialNStart
    } else {
        SamusStatus::SpecialAirNStart
    };
    set(
        f,
        status,
        0.0,
        StatusTiming::at_speed(SPECIAL_N_START_LENGTH, speed),
    );
    f.samus.damage_resets_charge = true;
    f.samus.charge_shot = false;
    f.samus.shot_fired = false;
    f.samus.is_release = !ground || f.samus.charge_level == CHARGE_MAX;
}

/// `ftSamusSpecialNLoopSetStatus`.
fn set_special_n_loop(f: &mut Fighter) {
    set(f, SamusStatus::SpecialNLoop, 0.0, StatusTiming::unknown());
    f.samus.damage_resets_charge = true;
    f.samus.charge_int = CHARGE_INT;
    f.samus.charge_shot = true;
}

/// `ftSamusSpecialNEndSetStatus`.
fn set_special_n_end(f: &mut Fighter) {
    set(
        f,
        SamusStatus::SpecialNEnd,
        0.0,
        StatusTiming::frames(SPECIAL_N_END_LENGTH),
    );
    f.samus.damage_resets_charge = true;
    f.samus.shot_fired = false;
}

/// `ftSamusSpecialAirNEndSetStatus`.
fn set_special_air_n_end(f: &mut Fighter) {
    if f.is_grounded() {
        f.become_airborne();
        physics::clamp_air_vel_x(&mut f.physics, f.attributes.air_speed_max_x);
    }
    set(
        f,
        SamusStatus::SpecialAirNEnd,
        0.0,
        StatusTiming::frames(SPECIAL_AIR_N_END_LENGTH),
    );
    f.samus.damage_resets_charge = true;
    f.samus.shot_fired = false;
}

/// `ftSamusSpecialNDestroyChargeShot`.
fn destroy_charge_shot(f: &mut Fighter) {
    f.samus.charge_shot = false;
}

/// `ftSamusSpecialNProcDamage`: a hit during Charge Shot spends the charge.
pub fn on_damage(f: &mut Fighter) {
    if f.samus.damage_resets_charge {
        f.samus.charge_level = 0;
        destroy_charge_shot(f);
        f.samus.damage_resets_charge = false;
    }
}

/// `ftSamusSpecialNGetChargeShotPosition`: joint 16 plus 180 along its X.
pub fn charge_shot_position(f: &Fighter) -> Vec3 {
    f.joint_world(CHARGE_JOINT, Vec3::new(CHARGE_OFF_X, 0.0, 0.0))
}

/// The flag-0 half of `ftSamusSpecialNEndProcUpdate`.
fn fire_charge_shot(f: &mut Fighter) {
    let level = f.samus.charge_level;
    f.weapon_spawn = Some(WeaponSpawn {
        kind: WeaponKind::SamusChargeShot(level),
        owner_port: f.port,
        position: charge_shot_position(f),
        facing: f.facing.sign(),
    });
    f.samus.charge_shot = false;
    let recoil_x = f32::from(level) + 1.0;
    if f.situation == Situation::Air {
        f.physics.vel_air.x =
            (CHARGE_RECOIL_MUL * recoil_x + CHARGE_RECOIL_BASE) * -f.facing.sign();
        let recoil_y =
            recoil_x + CHARGE_RECOIL_ADD + (f.samus.charge_recoil as f32 * -CHARGE_RECOIL_BASE);
        if f.physics.vel_air.y < recoil_y {
            f.physics.vel_air.y = recoil_y;
        }
        f.samus.charge_recoil += 1;
    } else {
        // Source ground velocity is facing-relative; the port's is world X.
        f.physics.vel_ground.x =
            -(CHARGE_RECOIL_MUL * recoil_x + CHARGE_RECOIL_BASE) * f.facing.sign();
    }
    f.samus.charge_level = 0;
    f.samus.damage_resets_charge = false;
}

/// `ftSamusSpecialHiSetStatus`.
pub fn set_special_hi(f: &mut Fighter) {
    set(
        f,
        SamusStatus::SpecialHi,
        0.0,
        StatusTiming::frames(SPECIAL_HI_LENGTH),
    );
    f.samus.takeoff_pending = true;
}

/// `ftSamusSpecialAirHiSetStatus`.
pub fn set_special_air_hi(f: &mut Fighter) {
    set(
        f,
        SamusStatus::SpecialAirHi,
        0.0,
        StatusTiming::frames(SPECIAL_AIR_HI_LENGTH),
    );
    f.physics.jumps_used = f.attributes.jumps_max;
    f.physics.vel_air.y = SCREWATTACK_VEL_Y_BASE;
    physics::clamp_air_vel_x(&mut f.physics, SCREWATTACK_DRIFT_CLAMP);
}

/// `ftSamusSpecialLwSetStatus`.
pub fn set_special_lw(f: &mut Fighter) {
    set(
        f,
        SamusStatus::SpecialLw,
        0.0,
        StatusTiming::frames(SPECIAL_LW_LENGTH),
    );
    f.samus.takeoff_pending = true;
    f.samus.bomb_spawned = false;
}

/// `ftSamusSpecialAirLwSetStatus`.
pub fn set_special_air_lw(f: &mut Fighter) {
    set(
        f,
        SamusStatus::SpecialAirLw,
        0.0,
        StatusTiming::frames(SPECIAL_LW_LENGTH),
    );
    f.samus.takeoff_pending = false;
    f.samus.bomb_spawned = false;
    f.physics.vel_air.y = BOMB_VEL_Y_BASE - BOMB_VEL_Y_SUB;
    physics::clamp_air_vel_x(&mut f.physics, f.attributes.air_speed_max_x * BOMB_DRIFT);
    f.physics.jumps_used = f.attributes.jumps_max;
}

/// `ftSamusSpecialLwMakeBomb`: TopN plus 60 up.
fn make_bomb(f: &mut Fighter) {
    if f.samus.bomb_spawned || f.status.anim_frame < BOMB_SPAWN_FRAME {
        return;
    }
    f.samus.bomb_spawned = true;
    f.weapon_spawn = Some(WeaponSpawn {
        kind: WeaponKind::SamusBomb,
        owner_port: f.port,
        position: f.joint_world(0, Vec3::new(0.0, BOMB_OFF_Y, 0.0)),
        facing: f.facing.sign(),
    });
}

/// `SetAirJumpMax`: the fighter becomes airborne without a status change.
fn air_jump_max(f: &mut Fighter) {
    f.become_airborne();
    f.floor = None;
    f.physics.vel_air.z = 0.0;
    f.physics.jumps_used = f.attributes.jumps_max;
}

/// Samus's `proc_update` and `proc_interrupt` callbacks.
pub fn update(f: &mut Fighter) {
    let AnyStatus::Samus(current) = f.status.status else {
        return;
    };
    match current {
        SamusStatus::SpecialNStart | SamusStatus::SpecialAirNStart => {
            if f.status.animation_ended() {
                if f.situation == Situation::Air {
                    set_special_air_n_end(f);
                } else if f.samus.is_release {
                    set_special_n_end(f);
                } else {
                    set_special_n_loop(f);
                }
            }
        }
        SamusStatus::SpecialNLoop => {
            f.samus.charge_int -= 1;
            if f.samus.charge_int == 0 {
                f.samus.charge_int = CHARGE_INT;
                if f.samus.charge_level < CHARGE_MAX {
                    f.samus.charge_level += 1;
                    if f.samus.charge_level == CHARGE_MAX {
                        destroy_charge_shot(f);
                        f.samus.damage_resets_charge = false;
                        status::set_wait(f);
                    }
                }
            }
        }
        SamusStatus::SpecialNEnd | SamusStatus::SpecialAirNEnd => {
            if !f.samus.shot_fired {
                f.samus.shot_fired = true;
                fire_charge_shot(f);
            }
            if f.status.animation_ended() {
                status::set_wait_or_fall(f);
            }
        }
        SamusStatus::SpecialHi => {
            if f.samus.takeoff_pending && f.status.anim_frame >= SCREW_TAKEOFF_FRAME {
                f.samus.takeoff_pending = false;
                air_jump_max(f);
                // `SetFlag1(1)`, consumed by this frame's physics.
                f.physics.vel_air.x = f.facing.sign() * SCREWATTACK_VEL_X_BASE;
            }
            end_screw_attack(f);
        }
        SamusStatus::SpecialAirHi => end_screw_attack(f),
        SamusStatus::SpecialLw => {
            make_bomb(f);
            if f.samus.takeoff_pending && f.status.anim_frame >= BOMB_HOP_FRAME {
                f.samus.takeoff_pending = false;
                air_jump_max(f);
            }
            if f.situation == Situation::Air {
                // `ftSamusSpecialLwTransferStatusAir`.
                let frame = f.status.anim_frame;
                let timing = f.status.timing;
                set(f, SamusStatus::SpecialAirLw, frame, timing);
                f.physics.vel_air.y = BOMB_VEL_Y_BASE;
                f.physics.jumps_used = f.attributes.jumps_max;
            } else if f.status.animation_ended() {
                status::set_wait(f);
            }
        }
        SamusStatus::SpecialAirLw => {
            make_bomb(f);
            if f.status.animation_ended() {
                status::set_fall(f);
            }
        }
    }
    interrupt(f);
}

fn end_screw_attack(f: &mut Fighter) {
    if f.status.animation_ended() {
        status::set_fall_special(
            f,
            SCREWATTACK_FALLSPECIAL_DRIFT,
            true,
            true,
            SCREWATTACK_LANDING_LAG,
            false,
        );
    }
}

/// `ftSamusSpecialNStartProcInterrupt` / `ftSamusSpecialNLoopProcInterrupt`.
/// The aerial start has none. The loop's shield-roll escape waits on the
/// unported `ftCommonEscape` statuses.
fn interrupt(f: &mut Fighter) {
    let taps = taps(f);
    match f.status.status {
        AnyStatus::Samus(SamusStatus::SpecialNStart) => {
            if taps.contains(N64Buttons::B) || taps.contains(N64Buttons::A) {
                f.samus.is_release = true;
            }
        }
        AnyStatus::Samus(SamusStatus::SpecialNLoop) => {
            if taps.contains(N64Buttons::B) || taps.contains(N64Buttons::A) {
                set_special_n_end(f);
            } else if taps.contains(N64Buttons::Z) {
                destroy_charge_shot(f);
                f.samus.damage_resets_charge = false;
                status::set_wait(f);
            }
        }
        _ => {}
    }
}

/// Aerial `proc_physics` for every Samus status that can be airborne,
/// including the grounded Screw Attack after `SetAirJumpMax`. Returns
/// `false` for statuses this module does not own.
pub fn apply_air_physics(f: &mut Fighter) -> bool {
    let AnyStatus::Samus(current) = f.status.status else {
        return false;
    };
    let attr = f.attributes;
    match current {
        SamusStatus::SpecialAirNStart => {
            gravity(f);
            physics::apply_air_drift(&mut f.physics, &attr, f.input.stick_x);
        }
        SamusStatus::SpecialAirNEnd => {
            gravity(f);
            if !physics::check_clamp_air_vel_x_dec(&mut f.physics, attr.air_speed_max_x) {
                physics::apply_air_friction(&mut f.physics, &attr);
            }
        }
        SamusStatus::SpecialHi => {
            // `ftPhysicsApplyAirVelTransNYZ`: the figatree drives Y and Z.
            let mut transn = f.physics;
            physics::apply_air_vel_transn_all(&mut transn, f.root_motion, f.facing.sign());
            f.physics.vel_air.y = transn.vel_air.y;
            f.physics.vel_air.z = transn.vel_air.z;
            screw_drift(f);
        }
        SamusStatus::SpecialAirHi => {
            physics::apply_gravity_default(&mut f.physics, &attr);
            screw_drift(f);
        }
        SamusStatus::SpecialAirLw => {
            physics::apply_gravity_default(&mut f.physics, &attr);
            let clamp = attr.air_speed_max_x * BOMB_DRIFT;
            if !physics::check_clamp_air_vel_x_dec(&mut f.physics, clamp) {
                physics::clamp_air_vel_x_stick_range(
                    &mut f.physics,
                    f.input.stick_x,
                    physics::AIRDRIFT_STICK_MIN,
                    attr.air_accel * BOMB_DRIFT,
                    clamp,
                );
                physics::apply_air_friction(&mut f.physics, &attr);
            }
        }
        _ => return false,
    }
    true
}

fn gravity(f: &mut Fighter) {
    if f.physics.is_fastfall {
        physics::apply_fast_fall(&mut f.physics, &f.attributes);
    } else {
        physics::apply_gravity_default(&mut f.physics, &f.attributes);
    }
}

fn screw_drift(f: &mut Fighter) {
    physics::clamp_air_vel_x_stick_range(
        &mut f.physics,
        f.input.stick_x,
        0,
        SCREWATTACK_DRIFT_MUL,
        SCREWATTACK_DRIFT_CLAMP,
    );
}

/// Grounded `proc_map` callbacks after the floor ran out. Returns `false`
/// when the status has no Samus-specific edge handling.
pub fn on_ground_lost(f: &mut Fighter) -> bool {
    let AnyStatus::Samus(current) = f.status.status else {
        return false;
    };
    let frame = f.status.anim_frame;
    let timing = f.status.timing;
    match current {
        // `ftSamusSpecialNStartSwitchStatusAir`.
        SamusStatus::SpecialNStart => {
            f.become_airborne();
            set(f, SamusStatus::SpecialAirNStart, frame, timing);
            physics::clamp_air_vel_x(&mut f.physics, f.attributes.air_speed_max_x);
            f.samus.is_release = true;
        }
        // `ftSamusSpecialNLoopProcMap` fires as it leaves the edge.
        SamusStatus::SpecialNLoop => set_special_air_n_end(f),
        // `ftSamusSpecialNEndSwitchStatusAir`.
        SamusStatus::SpecialNEnd => {
            f.become_airborne();
            set(f, SamusStatus::SpecialAirNEnd, frame, timing);
            physics::clamp_air_vel_x(&mut f.physics, f.attributes.air_speed_max_x);
        }
        // `ftSamusSpecialLwSwitchStatusAir`.
        SamusStatus::SpecialLw => {
            f.become_airborne();
            f.samus.takeoff_pending = false;
            set(f, SamusStatus::SpecialAirLw, frame, timing);
        }
        // `mpCommonSetFighterFallOnEdgeBreak`.
        SamusStatus::SpecialHi => {
            f.become_airborne();
            status::set_fall(f);
        }
        _ => return false,
    }
    true
}

/// Aerial `proc_map` callbacks on floor contact. The caller has already
/// recorded the floor. Returns `false` when the common landing applies.
pub fn on_landing(f: &mut Fighter, floor_y: f32) -> bool {
    let AnyStatus::Samus(current) = f.status.status else {
        return false;
    };
    let frame = f.status.anim_frame;
    let timing = f.status.timing;
    match current {
        SamusStatus::SpecialAirNStart => {
            f.land(floor_y);
            set(f, SamusStatus::SpecialNStart, frame, timing);
        }
        SamusStatus::SpecialAirNEnd => {
            f.land(floor_y);
            set(f, SamusStatus::SpecialNEnd, frame, timing);
        }
        SamusStatus::SpecialAirLw => {
            f.land(floor_y);
            f.samus.takeoff_pending = false;
            set(f, SamusStatus::SpecialLw, frame, timing);
        }
        // `ftSamusSpecialHiProcMap`: rising Screw Attack projects through
        // floors; a falling one lands into `LandingFallSpecial`.
        SamusStatus::SpecialHi | SamusStatus::SpecialAirHi => {
            if f.physics.vel_air.y >= 0.0 {
                f.floor = None;
                f.pos.y = floor_y;
                return true;
            }
            f.land(floor_y);
            status::set_landing_fall_special(f);
        }
        _ => return false,
    }
    true
}

/// No Samus special calls `ftPhysicsCheckSetFastFall`.
pub fn skips_fast_fall(status: AnyStatus) -> bool {
    matches!(status, AnyStatus::Samus(_))
}

/// Whether a charging shot is on Samus's arm, for presentation.
pub fn is_charging(f: &Fighter) -> bool {
    f.samus.charge_shot && f.status.status == AnyStatus::Samus(SamusStatus::SpecialNLoop)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fighter::FighterKind;
    use crate::status::Status;
    use ssb_engine::input::ControllerState;

    fn samus(ground: bool) -> Fighter {
        let mut f = Fighter::new(FighterKind::Samus, 0, 3);
        f.attributes.jumps_max = 2;
        f.attributes.air_speed_max_x = 28.0;
        if ground {
            f.situation = Situation::Ground;
        }
        f
    }

    fn press(f: &mut Fighter, buttons: u16) {
        let input = ControllerState {
            buttons: N64Buttons(buttons),
            ..Default::default()
        };
        f.set_input(input, false, false);
    }

    #[test]
    fn charge_gains_a_level_every_twenty_frames_and_stores_the_full_charge() {
        let mut f = samus(true);
        set_special_n(&mut f);
        assert!(!f.samus.is_release);
        for _ in 0..16 {
            press(&mut f, 0);
            status::update(&mut f);
        }
        assert_eq!(f.status.status, AnyStatus::Samus(SamusStatus::SpecialNLoop));
        assert!(is_charging(&f));
        for _ in 0..20 {
            press(&mut f, 0);
            status::update(&mut f);
        }
        assert_eq!(f.samus.charge_level, 1);
        for _ in 0..(20 * 6) {
            press(&mut f, 0);
            status::update(&mut f);
        }
        assert_eq!(f.samus.charge_level, CHARGE_MAX);
        assert_eq!(f.status.status, AnyStatus::Common(Status::Wait));
        assert!(!is_charging(&f));

        // A stored full charge skips the loop and slows the start motion.
        set_special_n(&mut f);
        assert!(f.samus.is_release);
        assert_eq!(f.status.timing.anim_speed, 1.0 - 0.160_000_03);
        while f.status.status != AnyStatus::Samus(SamusStatus::SpecialNEnd) {
            press(&mut f, 0);
            status::update(&mut f);
        }
        press(&mut f, 0);
        status::update(&mut f);
        let shot = f.take_weapon_spawn().expect("Shooting fires on flag 0");
        assert_eq!(shot.kind, WeaponKind::SamusChargeShot(CHARGE_MAX));
        assert_eq!(f.samus.charge_level, 0);
        assert_eq!(f.physics.vel_ground.x, -(2.0 * 8.0 + 10.0));
        press(&mut f, 0);
        status::update(&mut f);
        assert!(f.take_weapon_spawn().is_none());
    }

    #[test]
    fn a_hit_while_charging_spends_the_stored_charge() {
        let mut f = samus(true);
        f.samus.charge_level = 4;
        set_special_n(&mut f);
        on_damage(&mut f);
        assert_eq!(f.samus.charge_level, 0);
        let mut idle = samus(true);
        idle.samus.charge_level = 4;
        on_damage(&mut idle);
        assert_eq!(idle.samus.charge_level, 4);
    }

    #[test]
    fn aerial_shots_recoil_less_each_time_until_landing() {
        let mut f = samus(false);
        f.samus.charge_level = 2;
        set_special_n(&mut f);
        assert!(f.samus.is_release);
        while f.status.status != AnyStatus::Samus(SamusStatus::SpecialAirNEnd) {
            press(&mut f, 0);
            status::update(&mut f);
        }
        f.physics.vel_air.y = -50.0;
        press(&mut f, 0);
        status::update(&mut f);
        assert_eq!(f.physics.vel_air.x, -(2.0 * 3.0 + 10.0));
        assert_eq!(f.physics.vel_air.y, 3.0 + 20.0);
        assert_eq!(f.samus.charge_recoil, 1);
        f.land(0.0);
        assert_eq!(f.samus.charge_recoil, 0);
    }

    #[test]
    fn grounded_screw_attack_takes_off_on_frame_four() {
        let mut f = samus(true);
        set_special_hi(&mut f);
        for _ in 0..3 {
            status::update(&mut f);
            assert!(f.is_grounded());
        }
        status::update(&mut f);
        assert_eq!(f.situation, Situation::Air);
        assert_eq!(f.status.status, AnyStatus::Samus(SamusStatus::SpecialHi));
        assert_eq!(f.physics.vel_air.x, SCREWATTACK_VEL_X_BASE);
        assert_eq!(f.physics.jumps_used, 2);
        let screw = crate::attack::move_data(f.kind, f.status.status).unwrap();
        assert_eq!(
            screw.hitboxes.iter().filter(|h| h.is_active(4.0)).count(),
            4
        );
        assert_eq!(
            screw.hitboxes.iter().filter(|h| h.is_active(30.0)).count(),
            4
        );
        assert_eq!(
            screw.hitboxes.iter().filter(|h| h.is_active(32.0)).count(),
            0
        );
        while f.status.status == AnyStatus::Samus(SamusStatus::SpecialHi) {
            status::update(&mut f);
        }
        assert_eq!(f.status.status, AnyStatus::Common(Status::FallSpecial));
    }

    #[test]
    fn aerial_screw_attack_launches_and_ends_on_its_finisher() {
        let mut f = samus(false);
        f.physics.vel_air.x = 30.0;
        set_special_air_hi(&mut f);
        assert_eq!(f.physics.vel_air.y, SCREWATTACK_VEL_Y_BASE);
        assert_eq!(f.physics.vel_air.x, SCREWATTACK_DRIFT_CLAMP);
        let screw = crate::attack::move_data(f.kind, f.status.status).unwrap();
        let finisher: &[_] = &[screw.hitboxes[52]];
        assert!(finisher[0].is_active(30.0));
        assert_eq!(finisher[0].hitbox.kb_weight, 80);
    }

    #[test]
    fn grounded_bomb_hops_on_frame_three_and_drops_on_frame_ten() {
        let mut f = samus(true);
        set_special_lw(&mut f);
        for _ in 0..2 {
            status::update(&mut f);
        }
        assert!(f.is_grounded());
        status::update(&mut f);
        assert_eq!(f.status.status, AnyStatus::Samus(SamusStatus::SpecialAirLw));
        assert_eq!(f.status.anim_frame, 3.0);
        assert_eq!(f.physics.vel_air.y, BOMB_VEL_Y_BASE);
        for _ in 3..9 {
            status::update(&mut f);
            assert!(f.weapon_spawn.is_none());
        }
        status::update(&mut f);
        let bomb = f.take_weapon_spawn().expect("flag 0 at frame 10");
        assert_eq!(bomb.kind, WeaponKind::SamusBomb);
        assert_eq!(bomb.position, Vec3::new(0.0, BOMB_OFF_Y, 0.0));

        // Landing keeps the frame and does not hop again.
        f.land(0.0);
        let frame = f.status.anim_frame;
        let timing = f.status.timing;
        set(&mut f, SamusStatus::SpecialLw, frame, timing);
        f.samus.takeoff_pending = false;
        status::update(&mut f);
        assert!(f.is_grounded());
    }
}
