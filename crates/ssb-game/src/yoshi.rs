//! Yoshi's own statuses: `ftyoshispecialn.c` (Egg Lay), `ftyoshispecialhi.c`
//! (Egg Throw), `ftyoshispeciallw.c` (Yoshi Bomb) and his aerial jump
//! (`ftcommonjumpaerial.c`), with the motion script events from
//! `relocData/246_YoshiMainMotion.c` (US).
//!
//! Callbacks run in the source order: `proc_update` ([`update`]), then
//! `proc_physics` ([`apply_air_physics`], or ground friction), then
//! `proc_map` ([`on_ground_lost`] / [`on_landing`]).
//!
//! The Egg Lay grab uses the shared grab link (`crate::grab`): the search
//! runs in [`crate::grab::search_catch`], the swallowed fighter's side is
//! `crate::capture_yoshi`, and the stage writes Yoshi's scripts make on
//! that fighter travel as [`crate::grab::GrabEvent::YoshiEggStage`].
//!
//! The egg of Egg Throw is a weapon parented to Yoshi. Until the throw its
//! attack is off and its update does nothing, so the port keeps it as
//! [`YoshiState::egg_held`] and hands the pool a thrown egg at the throw
//! (`crate::weapon::YoshiEgg`). The Yoshi Bomb stars are pool weapons.

use ssb_engine::input::N64Buttons;
use ssb_engine::math::Vec3;

use crate::attack::Hitbox;
use crate::fighter::{Fighter, FighterKind, Situation};
use crate::grab::{GrabEvent, ThrowHitDesc};
use crate::physics;
use crate::status::{self, AnyStatus, StatusTiming, YoshiStatus};
use crate::weapon::{WeaponKind, WeaponSpawn};

/// `FTYOSHI_JUMPAERIAL_KNOCKBACK_RESIST` (US).
pub const JUMPAERIAL_KNOCKBACK_RESIST: f32 = 140.0;
/// `FTCOMMON_JUMPAERIAL_TURN_STICK_RANGE_MIN`, `..._TURN_FRAMES` and
/// `..._TURN_INVERT_LR_WAIT`.
pub const JUMPAERIAL_TURN_STICK_RANGE_MIN: i32 = -30;
pub const JUMPAERIAL_TURN_FRAMES: u8 = 12;
pub const JUMPAERIAL_TURN_INVERT_LR_WAIT: u8 = 6;

/// `FTYOSHI_YOSHIBOMB_VEL_X_CLAMP` and `..._VEL_Y_CLAMP`.
pub const YOSHIBOMB_VEL_X_CLAMP: f32 = 30.0;
pub const YOSHIBOMB_VEL_Y_CLAMP: f32 = -150.0;

/// `FTYOSHI_EGGTHROW_JOINT` (`nFTPartsJointYRotN`).
pub const EGGTHROW_JOINT: u8 = 3;

/// Figatree lengths (`ssb_rom::anim::EXPECTED_FRAMES`).
const SPECIAL_HI_LENGTH: f32 = 72.0;
const SPECIAL_LW_START_LENGTH: f32 = 30.0;
const SPECIAL_AIR_LW_START_LENGTH: f32 = 28.0;
const SPECIAL_LW_LANDING_LENGTH: f32 = 44.0;
const SPECIAL_N_LENGTH: f32 = 38.0;
const SPECIAL_N_RELEASE_LENGTH: f32 = 35.0;

/// `EggThrowGround` (both Egg Throw statuses): `SetFlag2(1)` at frame 4
/// makes the egg, `SetFlag2(2)` at frame 23 throws it.
const EGG_MAKE_FRAME: f32 = 4.0;
const EGG_THROW_FRAME: f32 = 23.0;
/// `GroundPound` sets flag 1 at frame 30, `GroundPoundAir` at frame 5.
const BOMB_GROUND_FLAG1_FRAME: f32 = 30.0;
const BOMB_AIR_FLAG1_FRAME: f32 = 5.0;
/// `GroundPoundLanding`: `Wait(3)` then `SetFlag0(1)` makes the stars.
const BOMB_STARS_FRAME: f32 = 3.0;
/// `EggLay_0x1730` (both Egg Lay statuses): the catch box exists from
/// `WaitAsync(18)` until the clear after `Wait(6)`.
pub const EGG_LAY_CATCH_FRAMES: core::ops::Range<f32> = 18.0..24.0;
/// `EggLay_0x1760` (both catch statuses): `WaitAsync(25)`, `SetFlag1(1)`.
const EGG_LAY_CATCH_FLAG1_FRAME: f32 = 25.0;
/// `EggLayGrabbedSomeoneComingInAndSwallowing`: flag 2 at frame 6 hides the
/// swallowed fighter, flag 1 at frame 20 lays it.
const EGG_LAY_SWALLOW_FRAME: f32 = 6.0;
const EGG_LAY_LAY_FRAME: f32 = 20.0;

/// The Egg Lay catch box: joint 31 (the tongue), size 300, 100 up.
pub const EGG_LAY_CATCH: (Hitbox, u8) = (
    Hitbox {
        damage: 0,
        radius: 150.0,
        offset: Vec3::new(0.0, 100.0, 0.0),
        angle: 361,
        kb_scale: 100,
        kb_weight: 0,
        kb_base: 0,
    },
    31,
);

/// `dYoshiMainMotion_0x16F8`, the Egg Lay script's `SetThrow`. Only `[1]`
/// is read: the release when Yoshi is hit while holding.
pub const EGG_LAY_THROW_DESC: [ThrowHitDesc; 2] = [
    ThrowHitDesc {
        status: None,
        damage: 20,
        angle: 361,
        kb_scale: 100,
        kb_weight: 0,
        kb_base: 0,
    },
    ThrowHitDesc {
        status: None,
        damage: 5,
        angle: 361,
        kb_scale: 100,
        kb_weight: 0,
        kb_base: 0,
    },
];

/// Yoshi's status vars and the aerial jump's turn.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct YoshiState {
    /// `status_vars.yoshi.specialhi.egg_gobj != NULL`: the egg is in hand.
    pub egg_held: bool,
    /// `status_vars.yoshi.specialhi.throw_force`: frames B was held.
    pub throw_force: i16,
    /// Motion flag 1 of Egg Throw, set by the throw. The aerial status then
    /// checks ledges as well as floors.
    pub egg_thrown: bool,
    /// `status_vars.common.jumpaerial.turn_tics`.
    pub jumpaerial_turn_tics: u8,
}

/// Yoshi and Polygon Yoshi, which run the same statuses.
pub fn is_yoshi(kind: FighterKind) -> bool {
    crate::grab::base_kind(kind) == FighterKind::Yoshi
}

fn set(f: &mut Fighter, status: YoshiStatus, frame: f32, length: f32) {
    status::set_any_status(
        f,
        AnyStatus::Yoshi(status),
        frame,
        StatusTiming::frames(length),
    );
}

fn crossed(f: &Fighter, at: f32) -> bool {
    let frame = f.status.anim_frame;
    frame >= at && frame - f.status.timing.anim_speed < at
}

// ---------------------------------------------------------------------------
// Aerial jump (`ftcommonjumpaerial.c`)
// ---------------------------------------------------------------------------

/// The Yoshi half of `ftCommonJumpAerialSetStatus`: knockback armour, and a
/// turn when the stick points back. `ftCommonJumpAerialUpdateModelYaw` runs
/// once at the end of the setter.
pub fn set_jump_aerial(f: &mut Fighter) {
    f.knockback_resist = JUMPAERIAL_KNOCKBACK_RESIST;
    f.yoshi.jumpaerial_turn_tics = if i32::from(f.input.stick_x) * (f.facing.sign() as i32)
        < JUMPAERIAL_TURN_STICK_RANGE_MIN
    {
        JUMPAERIAL_TURN_FRAMES
    } else {
        0
    };
    update_jump_aerial_turn(f);
}

/// `ftCommonJumpAerialUpdateModelYaw`: the model turns over twelve frames
/// and the facing flips halfway. The yaw itself is presentation.
pub fn update_jump_aerial_turn(f: &mut Fighter) {
    if !is_yoshi(f.kind) || f.yoshi.jumpaerial_turn_tics == 0 {
        return;
    }
    f.yoshi.jumpaerial_turn_tics -= 1;
    if f.yoshi.jumpaerial_turn_tics == JUMPAERIAL_TURN_INVERT_LR_WAIT {
        f.facing = f.facing.flipped();
    }
}

// ---------------------------------------------------------------------------
// Egg Lay (`ftyoshispecialn.c`)
// ---------------------------------------------------------------------------

/// `ftYoshiSpecialNSetCatchParams`: this status searches for a fighter.
fn set_catch_params(f: &mut Fighter) {
    f.grab.is_catchstatus = true;
    f.grab.throw_desc = Some(EGG_LAY_THROW_DESC);
}

/// `ftYoshiSpecialNSetStatus`.
pub fn set_special_n(f: &mut Fighter) {
    set(f, YoshiStatus::SpecialN, 0.0, SPECIAL_N_LENGTH);
    set_catch_params(f);
}

/// `ftYoshiSpecialAirNSetStatus`.
pub fn set_special_air_n(f: &mut Fighter) {
    set(f, YoshiStatus::SpecialAirN, 0.0, SPECIAL_N_LENGTH);
    set_catch_params(f);
}

/// Whether this status's catch box is out: the Egg Lay search window.
pub fn egg_lay_searching(f: &Fighter) -> bool {
    matches!(
        f.status.status,
        AnyStatus::Yoshi(YoshiStatus::SpecialN | YoshiStatus::SpecialAirN)
    ) && EGG_LAY_CATCH_FRAMES.contains(&f.status.anim_frame)
}

/// `ftYoshiSpecialNCatchProcCatch` / `...AirNCatchProcCatch` and
/// `ftYoshiSpecialNCatchInitStatusVars`. The status keeps the frame.
pub fn catch(f: &mut Fighter, held: &Fighter) {
    let frame = f.status.anim_frame;
    let next = if f.situation == Situation::Air {
        YoshiStatus::SpecialAirNCatch
    } else {
        YoshiStatus::SpecialNCatch
    };
    set(f, next, frame, SPECIAL_N_LENGTH);
    f.grab.capture_immune = true;
    physics::stop_all(&mut f.physics);
    f.grab.catch = Some(held.port);
    f.grab.catch_kind = Some(held.kind);
    f.grab.is_catchstatus = false;
}

/// `ftYoshiSpecialNReleaseSetStatus` / `...AirNReleaseSetStatus`.
fn set_release(f: &mut Fighter) {
    let next = if f.situation == Situation::Air {
        YoshiStatus::SpecialAirNRelease
    } else {
        YoshiStatus::SpecialNRelease
    };
    set(f, next, 0.0, SPECIAL_N_RELEASE_LENGTH);
    f.grab.capture_immune = true;
}

/// `ftYoshiSpecialNCatchUpdateCaptureVars`.
fn update_capture_vars(f: &mut Fighter) {
    if crossed(f, EGG_LAY_SWALLOW_FRAME) && f.grab.catch.is_some() {
        f.grab.send(GrabEvent::YoshiEggStage(1));
    }
    if crossed(f, EGG_LAY_LAY_FRAME) && f.grab.catch.take().is_some() {
        f.grab.catch_kind = None;
        f.grab.send(GrabEvent::YoshiEggStage(3));
        f.grab.capture_immune = false;
    }
}

// ---------------------------------------------------------------------------
// Egg Throw (`ftyoshispecialhi.c`)
// ---------------------------------------------------------------------------

/// `ftYoshiSpecialHiInitStatusVars`.
fn init_special_hi(f: &mut Fighter) {
    f.yoshi.egg_held = false;
    f.yoshi.throw_force = 0;
    f.yoshi.egg_thrown = false;
}

/// `ftYoshiSpecialHiSetStatus`.
pub fn set_special_hi(f: &mut Fighter) {
    set(f, YoshiStatus::SpecialHi, 0.0, SPECIAL_HI_LENGTH);
    init_special_hi(f);
}

/// `ftYoshiSpecialAirHiSetStatus`.
pub fn set_special_air_hi(f: &mut Fighter) {
    set(f, YoshiStatus::SpecialAirHi, 0.0, SPECIAL_HI_LENGTH);
    init_special_hi(f);
}

/// `ftYoshiSpecialHiProcDamage`: a held egg is destroyed.
pub fn on_damage(f: &mut Fighter) {
    if is_yoshi(f.kind) {
        f.yoshi.egg_held = false;
    }
}

/// `ftYoshiSpecialHiUpdateEggThrowForce` and `ftYoshiSpecialHiUpdateEggVars`.
fn update_egg(f: &mut Fighter) {
    if f.input.buttons.contains(N64Buttons::B) {
        f.yoshi.throw_force = f.yoshi.throw_force.saturating_add(1);
    }
    if crossed(f, EGG_THROW_FRAME) {
        if core::mem::take(&mut f.yoshi.egg_held) {
            f.weapon_spawn = Some(WeaponSpawn {
                kind: WeaponKind::YoshiEgg {
                    throw_force: f.yoshi.throw_force,
                    stick_x: f.input.stick_x,
                },
                owner_port: f.port,
                stale: crate::stale::WeaponStale::of(f),
                position: f.joint_world(EGGTHROW_JOINT, Vec3::ZERO),
                facing: f.facing.sign(),
            });
        }
        f.yoshi.egg_thrown = true;
    } else if crossed(f, EGG_MAKE_FRAME) {
        f.yoshi.egg_held = true;
    }
}

// ---------------------------------------------------------------------------
// Yoshi Bomb (`ftyoshispeciallw.c`)
// ---------------------------------------------------------------------------

/// `ftYoshiSpecialLwStartSetStatus`: Yoshi leaves the ground first.
pub fn set_special_lw_start(f: &mut Fighter) {
    f.become_airborne();
    set(f, YoshiStatus::SpecialLwStart, 0.0, SPECIAL_LW_START_LENGTH);
    f.physics.jumps_used = f.attributes.jumps_max;
}

/// `ftYoshiSpecialAirLwStartSetStatus`.
pub fn set_special_air_lw_start(f: &mut Fighter) {
    set(
        f,
        YoshiStatus::SpecialAirLwStart,
        0.0,
        SPECIAL_AIR_LW_START_LENGTH,
    );
    f.physics.jumps_used = f.attributes.jumps_max;
}

/// `ftYoshiSpecialAirLwLoopSetStatus`: the pose and the box hold, the
/// horizontal speed is capped and the fall is at least 150 a frame.
fn set_special_air_lw_loop(f: &mut Fighter) {
    if f.physics.vel_air.x.abs() > YOSHIBOMB_VEL_X_CLAMP {
        f.physics.vel_air.x = f.physics.vel_air.x.signum() * YOSHIBOMB_VEL_X_CLAMP;
    }
    let vel_y = f.physics.vel_air.y;
    let frame = f.status.anim_frame;
    status::set_any_status(
        f,
        AnyStatus::Yoshi(YoshiStatus::SpecialAirLwLoop),
        frame,
        StatusTiming {
            anim_length: None,
            anim_speed: 0.0,
        },
    );
    f.physics.vel_air.y = vel_y.min(YOSHIBOMB_VEL_Y_CLAMP);
}

fn set_special_lw_landing(f: &mut Fighter, floor_y: f32) {
    f.land(floor_y);
    set(
        f,
        YoshiStatus::SpecialLwLanding,
        0.0,
        SPECIAL_LW_LANDING_LENGTH,
    );
}

/// `ftYoshiSpecialLwLandingProcUpdate`'s flag 0: `wpYoshiStarMakeStars`
/// from TopN, one star each way.
fn make_stars(f: &mut Fighter) {
    f.weapon_spawn = Some(WeaponSpawn {
        kind: WeaponKind::YoshiStars,
        owner_port: f.port,
        stale: crate::stale::WeaponStale::of(f),
        position: f.joint_world(0, Vec3::ZERO),
        facing: f.facing.sign(),
    });
}

// ---------------------------------------------------------------------------
// Status machine hooks
// ---------------------------------------------------------------------------

/// Yoshi's `proc_update` callbacks.
pub fn update(f: &mut Fighter) {
    let AnyStatus::Yoshi(current) = f.status.status else {
        return;
    };
    match current {
        YoshiStatus::SpecialHi => {
            update_egg(f);
            if f.status.animation_ended() {
                status::set_wait(f);
            }
        }
        YoshiStatus::SpecialAirHi => {
            update_egg(f);
            if f.status.animation_ended() {
                status::set_fall(f);
            }
        }
        YoshiStatus::SpecialLwStart | YoshiStatus::SpecialAirLwStart => {
            if f.status.animation_ended() {
                set_special_air_lw_loop(f);
            }
        }
        YoshiStatus::SpecialAirLwLoop => {}
        YoshiStatus::SpecialLwLanding => {
            if crossed(f, BOMB_STARS_FRAME) {
                make_stars(f);
            }
            if f.status.animation_ended() {
                status::set_wait(f);
            }
        }
        YoshiStatus::SpecialN => {
            if f.status.animation_ended() {
                f.grab.is_catchstatus = false;
                status::set_wait(f);
            }
        }
        YoshiStatus::SpecialAirN => {
            if f.status.animation_ended() {
                f.grab.is_catchstatus = false;
                status::set_fall(f);
            }
        }
        // `ftYoshiSpecialNCatchUpdateProcStatus`. Flag 1 stays set once the
        // script reaches it.
        YoshiStatus::SpecialNCatch | YoshiStatus::SpecialAirNCatch => {
            let flag1 = f.status.anim_frame >= EGG_LAY_CATCH_FLAG1_FRAME;
            if (flag1 && f.grab.catch.is_some()) || f.status.animation_ended() {
                set_release(f);
            }
        }
        YoshiStatus::SpecialNRelease => {
            update_capture_vars(f);
            if f.status.animation_ended() {
                status::set_wait(f);
            }
        }
        YoshiStatus::SpecialAirNRelease => {
            update_capture_vars(f);
            if f.status.animation_ended() {
                status::set_fall(f);
            }
        }
    }
    if !matches!(
        f.status.status,
        AnyStatus::Yoshi(YoshiStatus::SpecialHi | YoshiStatus::SpecialAirHi)
    ) {
        // Nothing updates or throws an egg outside Egg Throw.
        f.yoshi.egg_held = false;
    }
}

/// Aerial `proc_physics` for Yoshi's airborne statuses and his aerial jump.
/// Returns `false` for statuses this module does not own.
pub fn apply_air_physics(f: &mut Fighter) -> bool {
    if !is_yoshi(f.kind) {
        return false;
    }
    let attr = f.attributes;
    match f.status.status {
        // `ftYoshiJumpAerialProcPhysics`: the figatree drives Y and Z, and
        // no gravity applies.
        AnyStatus::Common(status::Status::JumpAerialF | status::Status::JumpAerialB) => {
            let mut transn = f.physics;
            physics::apply_air_vel_transn_all(&mut transn, f.root_motion, f.facing.sign());
            f.physics.vel_air.y = transn.vel_air.y;
            f.physics.vel_air.z = transn.vel_air.z;
            if !physics::check_clamp_air_vel_x_dec(&mut f.physics, attr.air_speed_max_x) {
                physics::clamp_air_vel_x_stick_range(
                    &mut f.physics,
                    f.input.stick_x,
                    physics::AIRDRIFT_STICK_MIN,
                    attr.air_accel,
                    attr.air_speed_max_x,
                );
                physics::apply_air_friction(&mut f.physics, &attr);
            }
        }
        // `ftPhysicsApplyAirVelFriction`: no drift input.
        AnyStatus::Yoshi(
            YoshiStatus::SpecialAirHi
            | YoshiStatus::SpecialAirN
            | YoshiStatus::SpecialAirNCatch
            | YoshiStatus::SpecialAirNRelease,
        ) => {
            if f.physics.is_fastfall {
                physics::apply_fast_fall(&mut f.physics, &attr);
            } else {
                physics::apply_gravity_default(&mut f.physics, &attr);
            }
            if !physics::check_clamp_air_vel_x_dec(&mut f.physics, attr.air_speed_max_x) {
                physics::apply_air_friction(&mut f.physics, &attr);
            }
        }
        // `ftPhysicsApplyAirVelTransNAll`.
        AnyStatus::Yoshi(YoshiStatus::SpecialLwStart | YoshiStatus::SpecialAirLwStart) => {
            physics::apply_air_vel_transn_all(&mut f.physics, f.root_motion, f.facing.sign());
        }
        // `ftYoshiSpecialAirLwLoopProcPhysics`: the fall speed holds.
        AnyStatus::Yoshi(YoshiStatus::SpecialAirLwLoop) => {
            if !physics::check_clamp_air_vel_x_dec(&mut f.physics, attr.air_speed_max_x) {
                physics::apply_air_friction(&mut f.physics, &attr);
            }
        }
        _ => return false,
    }
    true
}

/// No Yoshi status, and not his aerial jump, calls
/// `ftPhysicsCheckSetFastFall`.
pub fn skips_fast_fall(f: &Fighter) -> bool {
    matches!(f.status.status, AnyStatus::Yoshi(_))
        || (is_yoshi(f.kind)
            && matches!(
                f.status.status,
                AnyStatus::Common(status::Status::JumpAerialF | status::Status::JumpAerialB)
            ))
}

/// Grounded `proc_map` callbacks after the floor ran out. Returns `false`
/// when the common fall applies.
pub fn on_ground_lost(f: &mut Fighter) -> bool {
    let AnyStatus::Yoshi(current) = f.status.status else {
        return false;
    };
    let frame = f.status.anim_frame;
    match current {
        // `ftYoshiSpecialNSwitchStatusAir`: the search carries over.
        YoshiStatus::SpecialN => {
            f.become_airborne();
            set(f, YoshiStatus::SpecialAirN, frame, SPECIAL_N_LENGTH);
            set_catch_params(f);
        }
        YoshiStatus::SpecialNCatch => {
            f.become_airborne();
            set(f, YoshiStatus::SpecialAirNCatch, frame, SPECIAL_N_LENGTH);
        }
        YoshiStatus::SpecialNRelease => {
            f.become_airborne();
            set(
                f,
                YoshiStatus::SpecialAirNRelease,
                frame,
                SPECIAL_N_RELEASE_LENGTH,
            );
        }
        // `ftYoshiSpecialHiSwitchStatusAir`: the egg stays in hand.
        YoshiStatus::SpecialHi => {
            f.become_airborne();
            set(f, YoshiStatus::SpecialAirHi, frame, SPECIAL_HI_LENGTH);
            physics::clamp_air_vel_x(&mut f.physics, f.attributes.air_speed_max_x);
        }
        _ => return false,
    }
    true
}

/// Aerial `proc_map` callbacks on floor contact. The caller has already
/// recorded the floor. Returns `false` when the common landing applies.
pub fn on_landing(f: &mut Fighter, floor_y: f32) -> bool {
    let AnyStatus::Yoshi(current) = f.status.status else {
        return false;
    };
    let frame = f.status.anim_frame;
    match current {
        // `ftYoshiSpecialAirNSwitchStatusGround`.
        YoshiStatus::SpecialAirN => {
            f.land(floor_y);
            set(f, YoshiStatus::SpecialN, frame, SPECIAL_N_LENGTH);
            set_catch_params(f);
        }
        YoshiStatus::SpecialAirNCatch => {
            f.land(floor_y);
            set(f, YoshiStatus::SpecialNCatch, frame, SPECIAL_N_LENGTH);
        }
        YoshiStatus::SpecialAirNRelease => {
            f.land(floor_y);
            set(
                f,
                YoshiStatus::SpecialNRelease,
                frame,
                SPECIAL_N_RELEASE_LENGTH,
            );
        }
        // `ftYoshiSpecialAirHiSwitchStatusGround`. After the throw the
        // source also catches ledges; ledge detection is not ported.
        YoshiStatus::SpecialAirHi => {
            f.land(floor_y);
            set(f, YoshiStatus::SpecialHi, frame, SPECIAL_HI_LENGTH);
        }
        // `ftYoshiSpecialLwStartProcMap`: the start lands only once flag 1
        // is up and Yoshi is falling. Before that the floor holds him and
        // the status continues.
        YoshiStatus::SpecialLwStart | YoshiStatus::SpecialAirLwStart => {
            let flag1_frame = if current == YoshiStatus::SpecialLwStart {
                BOMB_GROUND_FLAG1_FRAME
            } else {
                BOMB_AIR_FLAG1_FRAME
            };
            if frame >= flag1_frame && f.physics.vel_air.y <= 0.0 {
                set_special_lw_landing(f, floor_y);
            } else {
                f.pos.y = floor_y;
                f.floor = None;
            }
        }
        // `ftYoshiSpecialAirLwLoopProcMap`.
        YoshiStatus::SpecialAirLwLoop => set_special_lw_landing(f, floor_y),
        _ => return false,
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::status::Status;
    use ssb_engine::input::ControllerState;

    fn yoshi(ground: bool) -> Fighter {
        let mut f = Fighter::new(FighterKind::Yoshi, 0, 3);
        f.attributes.jumps_max = 2;
        f.attributes.gravity = 2.8;
        f.attributes.tvel_base = 58.0;
        f.attributes.air_speed_max_x = 44.0;
        f.attributes.air_friction = 0.4;
        if ground {
            f.situation = Situation::Ground;
        }
        f
    }

    fn hold(f: &mut Fighter, buttons: u16, stick_x: i8) {
        f.set_input(
            ControllerState {
                buttons: N64Buttons(buttons),
                stick_x,
                ..Default::default()
            },
            false,
            false,
        );
    }

    #[test]
    fn egg_throw_makes_the_egg_at_4_and_throws_it_at_23_with_the_held_force() {
        let mut f = yoshi(true);
        set_special_hi(&mut f);
        for frame in 1..=22 {
            hold(&mut f, N64Buttons::B, 0);
            status::update(&mut f);
            assert_eq!(f.yoshi.egg_held, frame >= 4, "frame {frame}");
            assert!(f.weapon_spawn.is_none());
        }
        hold(&mut f, 0, 70);
        status::update(&mut f);
        let spawn = f.take_weapon_spawn().expect("thrown at frame 23");
        assert_eq!(
            spawn.kind,
            WeaponKind::YoshiEgg {
                throw_force: 22,
                stick_x: 70
            }
        );
        assert!(!f.yoshi.egg_held && f.yoshi.egg_thrown);
        while f.status.status != Status::Wait {
            status::update(&mut f);
        }
        assert_eq!(f.status.anim_frame, 0.0);
    }

    #[test]
    fn a_hit_before_the_throw_destroys_the_egg() {
        let mut f = yoshi(true);
        set_special_hi(&mut f);
        for _ in 0..10 {
            status::update(&mut f);
        }
        assert!(f.yoshi.egg_held);
        on_damage(&mut f);
        for _ in 0..13 {
            status::update(&mut f);
        }
        assert!(f.weapon_spawn.is_none());
    }

    #[test]
    fn yoshi_bomb_hops_holds_its_box_and_lands_into_stars() {
        let mut f = yoshi(true);
        set_special_lw_start(&mut f);
        assert_eq!(f.situation, Situation::Air);
        assert_eq!(f.physics.jumps_used, 2);
        // Floor contact before flag 1 does not land.
        assert!(on_landing(&mut f, 0.0));
        assert_eq!(
            f.status.status,
            AnyStatus::Yoshi(YoshiStatus::SpecialLwStart)
        );
        for _ in 0..30 {
            status::update(&mut f);
        }
        assert_eq!(
            f.status.status,
            AnyStatus::Yoshi(YoshiStatus::SpecialAirLwLoop)
        );
        assert_eq!(f.physics.vel_air.y, YOSHIBOMB_VEL_Y_CLAMP);
        assert_eq!(f.status.anim_frame, 30.0);
        status::update(&mut f);
        assert_eq!(f.status.anim_frame, 30.0, "the loop holds its frame");
        apply_air_physics(&mut f);
        assert_eq!(f.physics.vel_air.y, YOSHIBOMB_VEL_Y_CLAMP);
        assert!(on_landing(&mut f, 0.0));
        assert_eq!(
            f.status.status,
            AnyStatus::Yoshi(YoshiStatus::SpecialLwLanding)
        );
        assert!(f.is_grounded());
        for frame in 1..=3 {
            status::update(&mut f);
            assert_eq!(f.weapon_spawn.is_some(), frame == 3);
        }
        assert_eq!(f.take_weapon_spawn().unwrap().kind, WeaponKind::YoshiStars);
    }

    #[test]
    fn loop_caps_horizontal_speed_and_keeps_a_faster_fall() {
        let mut f = yoshi(false);
        set_special_air_lw_start(&mut f);
        f.physics.vel_air = Vec3::new(-50.0, -200.0, 0.0);
        f.status.anim_frame = 27.0;
        status::update(&mut f);
        assert_eq!(f.physics.vel_air.x, -YOSHIBOMB_VEL_X_CLAMP);
        assert_eq!(f.physics.vel_air.y, -200.0);
    }

    #[test]
    fn egg_lay_searches_on_frames_18_to_23_and_releases_after_the_catch() {
        let mut f = yoshi(true);
        set_special_n(&mut f);
        assert!(f.grab.is_catchstatus);
        for frame in 1..=23 {
            status::update(&mut f);
            assert_eq!(egg_lay_searching(&f), frame >= 18, "frame {frame}");
        }
        let held = Fighter::new(FighterKind::Mario, 1, 3);
        catch(&mut f, &held);
        assert_eq!(
            f.status.status,
            AnyStatus::Yoshi(YoshiStatus::SpecialNCatch)
        );
        assert_eq!(f.status.anim_frame, 23.0);
        status::update(&mut f);
        assert_eq!(
            f.status.status,
            AnyStatus::Yoshi(YoshiStatus::SpecialNCatch)
        );
        status::update(&mut f);
        assert_eq!(
            f.status.status,
            AnyStatus::Yoshi(YoshiStatus::SpecialNRelease)
        );
        for frame in 1..=20 {
            status::update(&mut f);
            let events: Vec<_> = f.grab.outbox.iter().flatten().copied().collect();
            match frame {
                6 => assert_eq!(events, [GrabEvent::YoshiEggStage(1)]),
                20 => assert_eq!(
                    events,
                    [GrabEvent::YoshiEggStage(1), GrabEvent::YoshiEggStage(3)]
                ),
                _ => {}
            }
        }
        assert!(f.grab.catch.is_none());
        while f.status.status != Status::Wait {
            status::update(&mut f);
        }
    }

    #[test]
    fn a_miss_ends_egg_lay_at_its_animation_end() {
        let mut f = yoshi(false);
        set_special_air_n(&mut f);
        for _ in 0..38 {
            status::update(&mut f);
        }
        assert!(matches!(
            f.status.status,
            AnyStatus::Common(Status::Fall | Status::FallAerial)
        ));
        assert!(!f.grab.is_catchstatus);
    }

    #[test]
    fn a_backward_aerial_jump_turns_yoshi_halfway_and_resists_knockback() {
        let mut f = yoshi(false);
        f.physics.jumps_used = 1;
        hold(&mut f, 0, -60);
        status::set_jump_aerial(&mut f);
        assert_eq!(f.knockback_resist, JUMPAERIAL_KNOCKBACK_RESIST);
        assert_eq!(f.yoshi.jumpaerial_turn_tics, 11);
        let facing = f.facing;
        for tick in 1..=5 {
            update_jump_aerial_turn(&mut f);
            assert_eq!(f.facing != facing, tick == 5, "tick {tick}");
        }
        status::set_fall(&mut f);
        assert_eq!(f.knockback_resist, 0.0);
    }
}
