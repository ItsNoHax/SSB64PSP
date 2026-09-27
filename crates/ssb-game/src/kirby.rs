//! Kirby's own statuses: the rapid jab (`ftcommonattack100.c`), the five
//! aerial jumps (`ftcommonjumpaerial.c`), the suplex (`ftkirbythrowf.c`),
//! Final Cutter (`ftkirbyspecialhi.c`), Stone (`ftkirbyspeciallw.c`) and
//! Inhale (`ftkirbyspecialn.c`), with the motion-script events from
//! `relocData/228_KirbyMainMotion.c` (US).
//!
//! Callbacks run in the source order: `proc_update` and `proc_interrupt`
//! ([`update`]), then `proc_physics` ([`apply_ground_physics`] /
//! [`apply_air_physics`]), then `proc_map` ([`on_ground_lost`] /
//! [`on_landing`]).
//!
//! Inhale uses the shared grab link (`crate::grab`); the swallowed fighter's
//! side is `crate::capture_kirby`. The copy abilities Inhale grants are
//! `crate::kirby_copy`, which these callbacks route the copy statuses to.
//!
//! ## Documented deviations
//!
//! * **Copy loss.** Losing a copy on damage needs an RNG
//!   (`syUtilsRandFloat`), which this crate does not have.
//! * **Ledges.** Final Cutter and the suplex can catch a ledge in the
//!   source; fighter map collision resolves floors only.
//! * **Grounded Final Cutter ending.** The ground status becomes airborne at
//!   `SetAirJumpMax` (frame 23), so the source's grounded end is not
//!   reachable; the port follows the same path.

use ssb_engine::input::N64Buttons;
use ssb_engine::math::{atan2, sin_cos, Vec2, Vec3};

use crate::attack::Hitbox;
use crate::fighter::{Facing, Fighter, FighterKind};
use crate::grab::GrabEvent;
use crate::physics;
use crate::status::{self, AnyStatus, KirbyStatus as K, StatusTiming};
use crate::weapon::{WeaponKind, WeaponSpawn};

/// `FTKIRBY_JUMPAERIAL_VEL_MUL` and `dFTKirbyJumpAerialFVelocities`.
pub const JUMPAERIAL_VEL_MUL: f32 = 0.8;
pub const JUMPAERIAL_VELOCITIES: [f32; 4] = [60.0, 52.0, 47.0, 40.0];
/// `FTCOMMON_JUMPAERIAL_STICK_RANGE_MIN`, `..._TURN_STICK_RANGE_MIN`,
/// `..._TURN_FRAMES` and `..._TURN_INVERT_LR_WAIT`.
pub const JUMPAERIAL_STICK_RANGE_MIN: i32 = 53;
pub const JUMPAERIAL_TURN_STICK_RANGE_MIN: i32 = -30;
pub const JUMPAERIAL_TURN_FRAMES: u8 = 12;
pub const JUMPAERIAL_TURN_INVERT_LR_WAIT: u8 = 6;
/// `Jump2`..`Jump5` set flag 1 at `WaitAsync(28)`: the next jump opens.
pub const JUMPAERIAL_FLAG1_FRAME: f32 = 28.0;

/// `FTKIRBY_VACUUM_*` (US).
pub const VACUUM_RELEASE_LAG: i32 = 40;
pub const VACUUM_COPY_STICK_RANGE_MIN: i32 = -40;
pub const VACUUM_TURN_STICK_RANGE_MIN: i32 = 28;
pub const VACUUM_THROW_DAMAGE: i32 = 10;
pub const VACUUM_COPY_DAMAGE: i32 = 6;
pub const VACUUM_COPY_ANGLE: f32 = 75.0 * core::f32::consts::PI / 180.0;
pub const VACUUM_COPY_VEL_BASE: f32 = 100.0;
pub const VACUUM_THROW_VEL_BASE: f32 = 120.0;
pub const VACUUM_SPECIALNWAIT_DIST_MIN: f32 = 1024.0;
pub const VACUUM_GRAVITY_MUL: f32 = 2.0;
pub const VACUUM_FALL_MAX_MUL: f32 = 2.0;

/// `FTKIRBY_FINALCUTTER_*`.
pub const FINALCUTTER_OFF_X: f32 = 200.0;
pub const FINALCUTTER_AIR_ACCEL_MUL: f32 = 0.5;
/// `SpecialAirHi` computes its TransN motion with TopN scaled to 0.8.
pub const FINALCUTTER_AIR_TRANSN_SCALE: f32 = 0.8;

/// `FTKIRBY_STONE_*` (US).
pub const STONE_DURATION_MAX: i32 = 160;
pub const STONE_DURATION_MIN: i32 = 18;
pub const STONE_FALL_VEL: f32 = -140.0;
pub const STONE_SLIDE_ANGLE: f32 = 25.0 * core::f32::consts::PI / 180.0;
pub const STONE_SLIDE_TRACTION_MUL: f32 = 1.15;
pub const STONE_SLIDE_VEL_MUL: f32 = 36.0;
pub const STONE_SLIDE_CLAMP_VEL_X: f32 = 30.0;
pub const STONE_HEALTH_MAX: i32 = 38;

/// `FTCOMMON_ATTACKAIR_SKIPLANDING_VEL_Y_MAX`.
const SKIPLANDING_VEL_Y_MAX: f32 = -20.0;

/// Figatree lengths (`ssb_rom::anim::EXPECTED_FRAMES`).
const ATTACK100_START_LENGTH: f32 = 8.0;
const ATTACK100_END_LENGTH: f32 = 10.0;
/// `FTKirbyAnimAttack100Loop`'s 25 frames, if the motion table has none.
const RAPID_LOOP_LENGTH: f32 = 25.0;
const JUMPAERIAL_LENGTH: f32 = 50.0;
const THROWF_LANDING_LENGTH: f32 = 35.0;
const SPECIAL_HI_LENGTH: f32 = 60.0;
const SPECIAL_HI_LANDING_LENGTH: f32 = 35.0;
const SPECIAL_LW_START_LENGTH: f32 = 12.0;
const SPECIAL_AIR_LW_START_LENGTH: f32 = 23.0;
const STONE_LENGTH: f32 = 200.0;
const SPECIAL_LW_END_LENGTH: f32 = 30.0;
const SPECIAL_N_START_LENGTH: f32 = 20.0;
const SPECIAL_N_END_LENGTH: f32 = 20.0;
const SPECIAL_N_EAT_LENGTH: f32 = 20.0;
const SPECIAL_N_THROW_LENGTH: f32 = 28.0;
const SPECIAL_N_TURN_LENGTH: f32 = 12.0;
const SPECIAL_N_COPY_LENGTH: f32 = 30.0;

/// Motion-script frames.
const THROWF_RELEASE_FRAME: f32 = 8.0;
const THROWF_AIR_JUMP_ADD_FRAME: f32 = 9.0;
const CUTTER_AIR_JUMP_MAX_FRAME: f32 = 23.0;
const CUTTER_WAVE_FRAME: f32 = 3.0;
const STONE_START_FLAG1_FRAME: f32 = 6.0;
const STONE_AIR_START_FLAG1_FRAME: f32 = 18.0;
const INHALE_SPIT_FRAME: f32 = 7.0;
const INHALE_COPY_FRAME: f32 = 8.0;

const fn catch_box(size: f32, y: f32, z: f32) -> Hitbox {
    Hitbox {
        damage: 0,
        radius: size / 2.0,
        offset: Vec3::new(0.0, y, z),
        angle: 361,
        kb_scale: 100,
        kb_weight: 0,
        kb_base: 0,
        element: crate::combat::Element::Normal,
        shield_damage: 0,
    }
}

/// `InhaleGround` (US): the two TopN boxes the Inhale loop searches with.
pub const INHALE_CATCH: [(Hitbox, u8); 2] = [
    (catch_box(290.0, 240.0, 200.0), 0),
    (catch_box(360.0, 240.0, 410.0), 0),
];

/// Kirby's passive copy, status vars and motion flags.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KirbyState {
    /// `passive_vars.kirby.copy_id`.
    pub copy_id: FighterKind,
    /// `passive_vars.kirby.is_ignore_losecopy`: `ftManagerMakeFighter` sets
    /// it for a Kirby created already holding a copy.
    pub is_ignore_losecopy: bool,
    pub rapid_is_anim_end: bool,
    pub rapid_is_goto_loop: bool,
    /// `status_vars.common.jumpaerial.turn_tics`.
    pub jumpaerial_turn_tics: u8,
    /// Final Cutter's wave was made (motion flag 0 consumed).
    pub cutter_spawned: bool,
    /// `FTStruct::is_damage_resist` and `damage_resist`. `set_any_status`
    /// clears the flag, as `ftMainSetStatus` does.
    pub is_damage_resist: bool,
    pub damage_resist: i32,
    /// `status_vars.kirby.speciallw.duration`.
    pub stone_duration: i32,
    /// Stone's motion flags 1 and 2.
    pub stone_flag1: bool,
    pub stone_flag2: bool,
    /// `status_vars.kirby.specialn.release_lag`.
    pub release_lag: i32,
    /// `status_vars.kirby.specialn.dist`: the swallowed fighter's offset
    /// from the capture point.
    pub inhale_dist: Vec2,
    /// `status_vars.kirby.specialn.copy_id`: the copy on offer.
    pub copy_pending: FighterKind,
    /// The swallowed fighter is a Kirby (`capturekirby.is_kirby`).
    pub victim_is_kirby: bool,
    /// The copy abilities' passive and status vars.
    pub copy: crate::kirby_copy::CopyState,
}

impl Default for KirbyState {
    fn default() -> Self {
        Self {
            copy_id: FighterKind::Kirby,
            is_ignore_losecopy: false,
            rapid_is_anim_end: false,
            rapid_is_goto_loop: false,
            jumpaerial_turn_tics: 0,
            cutter_spawned: false,
            is_damage_resist: false,
            damage_resist: 0,
            stone_duration: 0,
            stone_flag1: false,
            stone_flag2: false,
            release_lag: 0,
            inhale_dist: Vec2::ZERO,
            copy_pending: FighterKind::Kirby,
            victim_is_kirby: false,
            copy: crate::kirby_copy::CopyState::default(),
        }
    }
}

/// Kirby and Polygon Kirby, which run the same statuses.
pub fn is_kirby(kind: FighterKind) -> bool {
    crate::grab::base_kind(kind) == FighterKind::Kirby
}

/// Which Kirby statuses leave the fighter grounded (`ga` after the setter).
pub fn is_grounded(s: K) -> bool {
    crate::kirby_copy::is_grounded(s)
        || matches!(
            s,
            K::Attack100Start
                | K::Attack100Loop
                | K::Attack100End
                | K::ThrowFLanding
                | K::SpecialHi
                | K::SpecialHiLanding
                | K::SpecialLwStart
                | K::SpecialLwUnk
                | K::SpecialLwHold
                | K::SpecialAirLwLanding
                | K::SpecialNStart
                | K::SpecialNLoop
                | K::SpecialNEnd
                | K::SpecialNCatch
                | K::SpecialNEat
                | K::SpecialNThrow
                | K::SpecialNWait
                | K::SpecialNTurn
                | K::SpecialNCopy
        )
}

/// `ssb_rom::anim::SLOT_KIRBY_ATTACK100_START` onward. The aerial Inhale
/// statuses name the grounded figatrees; `SpecialNCatch` keeps the loop's
/// (`AnyStatus::keeps_motion`), and `SpecialAirLwFall` has none, so the
/// held stone pose stands in.
pub fn anim_slot(s: K) -> usize {
    const B: usize = 286;
    match s {
        K::Attack100Start => B,
        K::Attack100Loop => B + 1,
        K::Attack100End => B + 2,
        K::JumpAerialF1 => B + 3,
        K::JumpAerialF2 => B + 4,
        K::JumpAerialF3 => B + 5,
        K::JumpAerialF4 => B + 6,
        K::JumpAerialF5 => B + 7,
        K::ThrowF => B + 8,
        K::ThrowFFall => B + 9,
        K::ThrowFLanding => B + 10,
        K::SpecialHi => B + 11,
        K::SpecialHiLanding => B + 12,
        K::SpecialAirHi => B + 13,
        K::SpecialAirHiFall => B + 14,
        K::SpecialLwStart => B + 15,
        K::SpecialLwUnk => B + 16,
        K::SpecialLwHold => B + 17,
        K::SpecialLwEnd => B + 18,
        K::SpecialAirLwStart => B + 19,
        K::SpecialAirLwHold | K::SpecialAirLwFall => B + 20,
        K::SpecialAirLwLanding => B + 21,
        K::SpecialAirLwEnd => B + 22,
        K::SpecialNStart | K::SpecialAirNStart => B + 23,
        K::SpecialNLoop | K::SpecialAirNLoop | K::SpecialNCatch | K::SpecialAirNCatch => B + 24,
        K::SpecialNEnd | K::SpecialAirNEnd => B + 25,
        K::SpecialNEat | K::SpecialAirNEat => B + 26,
        K::SpecialNThrow | K::SpecialAirNThrow => B + 27,
        K::SpecialNWait | K::SpecialAirNWait => B + 28,
        K::SpecialNTurn | K::SpecialAirNTurn => B + 29,
        K::SpecialNCopy | K::SpecialAirNCopy => B + 30,
        _ => crate::kirby_copy::anim_slot(s).unwrap_or(B),
    }
}

fn set(f: &mut Fighter, s: K, frame: f32, timing: StatusTiming) {
    status::set_any_status(f, AnyStatus::Kirby(s), frame, timing);
}

fn set_frames(f: &mut Fighter, s: K, frame: f32, length: f32) {
    set(f, s, frame, StatusTiming::frames(length));
}

fn crossed(f: &Fighter, at: f32) -> bool {
    let frame = f.status.anim_frame;
    frame >= at && frame - f.status.timing.anim_speed < at
}

fn tapped(f: &Fighter) -> N64Buttons {
    f.button_tap()
}

/// `mpCommonSetFighterWaitOrLanding`, run before `land` clears the
/// velocity.
pub(crate) fn wait_or_landing(f: &mut Fighter, floor_y: f32) {
    if f.physics.vel_air.y > SKIPLANDING_VEL_Y_MAX {
        f.land(floor_y);
        status::set_wait(f);
    } else {
        status::set_landing(f);
        f.land(floor_y);
    }
}

/// `ftPhysicsApplyAirVelTransNYZ`: X is left to the caller.
fn apply_air_vel_transn_yz(f: &mut Fighter, scale: f32) {
    let x = f.physics.vel_air.x;
    let mut motion = f.root_motion;
    motion.delta *= scale;
    physics::apply_air_vel_transn_all(&mut f.physics, motion, f.facing.sign());
    f.physics.vel_air.x = x;
}

/// `FTKIRBY_COPYDAMAGE_LOSECOPY_RANDOM`.
const LOSECOPY_RANDOM: f32 = 1.0 / 12.0;

/// `ftKirbySpecialNDamageCheckLoseCopy`: on a tumble-level hit, a Kirby
/// holding a copy loses it one time in twelve (the shared generator).
pub fn damage_check_lose_copy(f: &mut Fighter) {
    if is_kirby(f.kind)
        && f.kirby.copy_id != FighterKind::Kirby
        && !f.kirby.is_ignore_losecopy
        && crate::rng::rand_float() < LOSECOPY_RANDOM
    {
        lose_copy(f);
    }
}

/// `ftKirbySpecialNLoseCopy`, without its star effect and sound.
pub fn lose_copy(f: &mut Fighter) {
    crate::kirby_copy::init_passive_vars(f);
    f.kirby.copy_id = FighterKind::Kirby;
}

/// `FTKirbyCopy[27]` at `KirbyMainMotion` 0x0000: `(copy_id, star_damage)`
/// per swallowed `FTKind`. The model-part and scale columns are
/// presentation.
pub const COPY: [(u8, i32); 27] = [
    (0, 17),
    (1, 17),
    (2, 30),
    (3, 17),
    (4, 17),
    (5, 17),
    (6, 25),
    (7, 17),
    (8, 17),
    (9, 17),
    (10, 17),
    (11, 17),
    (8, 17),
    (8, 17),
    (8, 17),
    (8, 17),
    (8, 30),
    (8, 17),
    (8, 17),
    (8, 17),
    (8, 17),
    (8, 17),
    (8, 17),
    (8, 17),
    (8, 17),
    (8, 17),
    (2, 50),
];

/// `FTKirbyCopy::star_damage` for a swallowed fighter kind.
pub fn star_damage(kind: FighterKind) -> i32 {
    COPY[kind as usize].1
}

// ---------------------------------------------------------------------------
// Rapid jab (`ftcommonattack100.c`)
// ---------------------------------------------------------------------------

/// `ftCommonAttack100StartSetStatus`'s Kirby case.
pub fn set_attack100_start(f: &mut Fighter) {
    set_frames(f, K::Attack100Start, 0.0, ATTACK100_START_LENGTH);
    f.kirby.rapid_is_anim_end = false;
    f.kirby.rapid_is_goto_loop = false;
}

fn set_attack100_loop(f: &mut Fighter) {
    // The figatree loops; the script pauses after its fifth pulse and
    // resumes on the wrap.
    let len = crate::motion::anim_length(f.kind, AnyStatus::Kirby(K::Attack100Loop))
        .unwrap_or(RAPID_LOOP_LENGTH);
    status::set_any_status(
        f,
        AnyStatus::Kirby(K::Attack100Loop),
        0.0,
        StatusTiming::looping(len),
    );
}

/// `ftCommonAttack100LoopProcUpdate` and `...ProcInterrupt`.
fn update_attack100_loop(f: &mut Fighter) {
    let speed = f.status.timing.anim_speed;
    if f.status.anim_frame >= 0.0 && f.status.anim_frame < speed {
        f.kirby.rapid_is_anim_end = true;
        // `ftParamSetMotionID`: each cycle is a new motion.
        f.motion.set(crate::stale::MotionAttackId::Attack100);
    }
    if f.motion_script.flags[1] != 0 {
        f.motion_script.flags[1] = 0;
        if f.kirby.rapid_is_anim_end && !f.kirby.rapid_is_goto_loop {
            set_frames(f, K::Attack100End, 0.0, ATTACK100_END_LENGTH);
            return;
        }
        f.kirby.rapid_is_goto_loop = false;
    }
    if f.button_tap().contains(N64Buttons::A) || f.button_release().contains(N64Buttons::A) {
        f.kirby.rapid_is_goto_loop = true;
    }
}

// ---------------------------------------------------------------------------
// Aerial jumps (`ftcommonjumpaerial.c`)
// ---------------------------------------------------------------------------

fn is_jump_aerial(s: AnyStatus) -> bool {
    matches!(
        s,
        AnyStatus::Kirby(
            K::JumpAerialF1 | K::JumpAerialF2 | K::JumpAerialF3 | K::JumpAerialF4 | K::JumpAerialF5
        )
    )
}

/// `ftCommonJumpAerialCheckInterruptCommon`'s Kirby branch.
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
        i32::from(f.stick.y) >= JUMPAERIAL_STICK_RANGE_MIN
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
    let status = match f.physics.jumps_used {
        1 => K::JumpAerialF1,
        2 => K::JumpAerialF2,
        3 => K::JumpAerialF3,
        4 => K::JumpAerialF4,
        _ => K::JumpAerialF5,
    };
    set_frames(f, status, 0.0, JUMPAERIAL_LENGTH);
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
    f.kirby.jumpaerial_turn_tics = if i32::from(f.input.stick_x) * (f.facing.sign() as i32)
        < JUMPAERIAL_TURN_STICK_RANGE_MIN
    {
        JUMPAERIAL_TURN_FRAMES
    } else {
        0
    };
    update_jump_aerial_turn(f);
}

/// `ftCommonJumpAerialUpdateModelYaw`; the yaw itself is presentation.
fn update_jump_aerial_turn(f: &mut Fighter) {
    if f.kirby.jumpaerial_turn_tics == 0 {
        return;
    }
    f.kirby.jumpaerial_turn_tics -= 1;
    if f.kirby.jumpaerial_turn_tics == JUMPAERIAL_TURN_INVERT_LR_WAIT {
        f.facing = f.facing.flipped();
    }
}

// ---------------------------------------------------------------------------
// Forward throw (`ftkirbythrowf.c`)
// ---------------------------------------------------------------------------

/// `ftCommonThrowSetStatus`'s Kirby case: the suplex leaves the ground.
pub fn set_throw_f(f: &mut Fighter, length: f32) {
    f.become_airborne();
    set_frames(f, K::ThrowF, 0.0, length);
}

// ---------------------------------------------------------------------------
// Final Cutter (`ftkirbyspecialhi.c`)
// ---------------------------------------------------------------------------

pub fn set_special_hi(f: &mut Fighter) {
    let s = if f.is_grounded() {
        K::SpecialHi
    } else {
        K::SpecialAirHi
    };
    set_frames(f, s, 0.0, SPECIAL_HI_LENGTH);
    f.kirby.cutter_spawned = false;
}

/// `ftKirbySpecialAirHiFallSetStatus`: the vertical velocity survives.
fn set_special_air_hi_fall(f: &mut Fighter) {
    let vel_y = f.physics.vel_air.y;
    f.become_airborne();
    set(f, K::SpecialAirHiFall, 0.0, StatusTiming::unknown());
    f.physics.jumps_used = f.attributes.jumps_max;
    f.physics.vel_air.y = vel_y;
}

fn set_special_hi_landing(f: &mut Fighter) {
    set_frames(f, K::SpecialHiLanding, 0.0, SPECIAL_HI_LANDING_LENGTH);
    f.kirby.cutter_spawned = false;
}

/// `ftKirbySpecialHiLandingProcUpdate`'s flag 0: the wave, 200 ahead of
/// TopN.
fn make_cutter(f: &mut Fighter) {
    f.kirby.cutter_spawned = true;
    let lr = f.facing.sign();
    let position = f.joint_world(0, Vec3::ZERO) + Vec3::new(FINALCUTTER_OFF_X * lr, 0.0, 0.0);
    f.weapon_spawn = Some(WeaponSpawn {
        kind: WeaponKind::KirbyCutter {
            grounded: f.is_grounded(),
        },
        owner_port: f.port,
        stale: crate::stale::WeaponStale::of(f),
        position,
        facing: lr,
    });
}

/// `SetAirJumpMax`: airborne without a status change.
fn air_jump_max(f: &mut Fighter) {
    f.become_airborne();
    f.floor = None;
    f.physics.vel_air.z = 0.0;
    f.physics.jumps_used = f.attributes.jumps_max;
}

/// The Final Cutter drift (`FTKIRBY_FINALCUTTER_AIR_ACCEL_MUL`).
fn cutter_drift(f: &mut Fighter) {
    let attr = f.attributes;
    if !physics::check_clamp_air_vel_x_dec(&mut f.physics, attr.air_speed_max_x) {
        physics::clamp_air_vel_x_stick_range(
            &mut f.physics,
            f.input.stick_x,
            physics::AIRDRIFT_STICK_MIN,
            attr.air_accel * FINALCUTTER_AIR_ACCEL_MUL,
            attr.air_speed_max_x,
        );
        physics::apply_air_friction(&mut f.physics, &attr);
    }
}

// ---------------------------------------------------------------------------
// Stone (`ftkirbyspeciallw.c`)
// ---------------------------------------------------------------------------

pub fn set_special_lw(f: &mut Fighter) {
    if f.is_grounded() {
        set_frames(f, K::SpecialLwStart, 0.0, SPECIAL_LW_START_LENGTH);
        f.kirby.stone_flag1 = false;
        f.kirby.stone_flag2 = false;
        f.physics.vel_ground = Vec3::ZERO;
    } else {
        // `ftKirbySpecialAirLwStartSetStatus` keeps `is_damage_resist`.
        let resist = f.kirby.is_damage_resist;
        set_frames(f, K::SpecialAirLwStart, 0.0, SPECIAL_AIR_LW_START_LENGTH);
        f.kirby.is_damage_resist = resist;
        f.kirby.stone_flag2 = resist;
        if !resist {
            f.kirby.stone_flag1 = false;
        }
    }
    f.physics.vel_air.x = 0.0;
    f.physics.vel_air.y = 0.0;
}

/// `ftKirbySpecialLwSetDamageResist`.
fn set_damage_resist(f: &mut Fighter) {
    f.kirby.is_damage_resist = true;
    f.kirby.damage_resist = STONE_HEALTH_MAX;
    f.kirby.stone_duration = STONE_DURATION_MAX;
}

fn set_stone(f: &mut Fighter, s: K, frame: f32) {
    set_frames(f, s, frame, STONE_LENGTH);
    // `StoneGround_0x1BB4` sets flag 1 on its first frame.
    if matches!(s, K::SpecialLwUnk | K::SpecialAirLwHold) {
        f.kirby.stone_flag1 = true;
    }
}

/// `ftKirbySpecialLwCheckRelease`.
fn stone_check_release(f: &mut Fighter, allow_release: bool) -> bool {
    let b = tapped(f).contains(N64Buttons::B);
    if allow_release {
        if b {
            return true;
        }
    } else if f.kirby.stone_duration < STONE_DURATION_MAX - STONE_DURATION_MIN && b {
        return true;
    }
    if f.kirby.stone_duration > 0 {
        f.kirby.stone_duration -= 1;
        false
    } else {
        true
    }
}

fn set_stone_end(f: &mut Fighter, grounded: bool) {
    // `ftKirbySpecialLwEndSetStatus` puts Kirby in the air first.
    f.become_airborne();
    let s = if grounded {
        K::SpecialLwEnd
    } else {
        K::SpecialAirLwEnd
    };
    set_frames(f, s, 0.0, SPECIAL_LW_END_LENGTH);
}

/// `ftKirbySpecialLwUnkDecideNextStatus`.
fn stone_unk_decide(f: &mut Fighter, grounded: bool) {
    if !f.kirby.is_damage_resist && f.kirby.stone_flag1 {
        if grounded {
            set_frames(f, K::SpecialLwHold, 0.0, STONE_LENGTH);
        }
        set_damage_resist(f);
        f.kirby.stone_flag1 = false;
        f.kirby.stone_flag2 = true;
    }
    if f.kirby.stone_flag2 && stone_check_release(f, false) {
        set_stone_end(f, grounded);
        f.kirby.stone_flag2 = false;
    }
}

/// `ftKirbySpecialLwHoldDecideNextStatus`.
fn stone_hold_decide(f: &mut Fighter, grounded: bool) {
    if stone_check_release(f, true) {
        set_stone_end(f, grounded);
    }
}

// ---------------------------------------------------------------------------
// Inhale (`ftkirbyspecialn.c`)
// ---------------------------------------------------------------------------

/// `ftKirbySpecialNSetStatusSelect`: the copy's special, or for a Kirby
/// without a ported copy `ftKirbySpecialNStartSetStatus`, whose floor
/// callback switches an airborne Kirby into the aerial start at once.
pub fn set_special_n(f: &mut Fighter) {
    if crate::kirby_copy::set_special_n(f) {
        return;
    }
    let s = if f.is_grounded() {
        K::SpecialNStart
    } else {
        K::SpecialAirNStart
    };
    set_frames(f, s, 0.0, SPECIAL_N_START_LENGTH);
    f.kirby.copy_pending = FighterKind::Kirby;
    f.kirby.release_lag = VACUUM_RELEASE_LAG;
    f.kirby.inhale_dist = Vec2::ZERO;
    f.kirby.victim_is_kirby = false;
    set_catch_params(f);
}

/// `ftKirbySpecialNSetCatchParams`.
fn set_catch_params(f: &mut Fighter) {
    f.grab.is_catchstatus = true;
    f.grab.throw_desc = Some(crate::grab::KIRBY_INHALE);
}

fn set_inhale(f: &mut Fighter, s: K, frame: f32, timing: StatusTiming) {
    set(f, s, frame, timing);
    f.grab.is_catchstatus = matches!(
        s,
        K::SpecialNStart | K::SpecialAirNStart | K::SpecialNLoop | K::SpecialAirNLoop
    );
    if !matches!(
        s,
        K::SpecialNStart | K::SpecialAirNStart | K::SpecialNLoop | K::SpecialAirNLoop
    ) {
        f.grab.capture_immune = true;
    }
}

/// Whether the Inhale loop's boxes are searching this frame.
pub fn inhale_searching(f: &Fighter) -> bool {
    f.grab.is_catchstatus
        && matches!(
            f.status.status,
            AnyStatus::Kirby(K::SpecialNLoop | K::SpecialAirNLoop)
        )
}

/// `ftKirbySpecialNCatchProcCatch` / `...AirNCatchProcCatch`.
pub fn inhale_catch(f: &mut Fighter, held: &Fighter) {
    let s = if f.is_grounded() {
        K::SpecialNCatch
    } else {
        K::SpecialAirNCatch
    };
    let frame = f.status.anim_frame;
    let timing = f.status.timing;
    set_inhale(f, s, frame, timing);
    f.grab.catch = Some(held.port);
    f.grab.catch_kind = Some(held.kind);
    f.grab.capture_immune = true;
    let lr = f.facing.sign();
    let point = Vec2::new(
        f.pos.x + crate::capture_kirby::CAPTURE_OFF.x * lr,
        f.pos.y + crate::capture_kirby::CAPTURE_OFF.y,
    );
    f.kirby.inhale_dist = Vec2::new(held.pos.x - point.x, held.pos.y - point.y);
    // `ftKirbySpecialNCatchProcUpdate` reads the copy on the eat; the
    // victim cannot change it while held.
    f.kirby.victim_is_kirby = is_kirby(held.kind);
    f.kirby.copy_pending = if f.kirby.victim_is_kirby {
        held.kirby.copy_id
    } else {
        copy_kind(COPY[held.kind as usize].0)
    };
}

fn copy_kind(id: u8) -> FighterKind {
    FighterKind::PLAYABLE
        .iter()
        .copied()
        .find(|k| *k as u8 == id)
        .unwrap_or(FighterKind::Kirby)
}

fn inhale_ground(f: &Fighter) -> bool {
    f.is_grounded()
}

/// `ftKirbySpecialNThrowCheckGotoThrow` / `...CopyCheckGotoCopy`: the
/// damage is dealt on the input.
fn capture_damage(f: &mut Fighter, damage: i32) {
    f.grab.send(GrabEvent::KirbyDamage {
        staled: crate::grab::StaledThrow::of(f, damage),
    });
}

/// `ftKirbySpecialNWaitProcInterrupt` and `...AirNWaitProcInterrupt`.
fn update_inhale_wait(f: &mut Fighter) {
    let grounded = inhale_ground(f);
    let taps = tapped(f);
    if taps.contains(N64Buttons::A) && f.grab.catch.is_some() {
        capture_damage(f, VACUUM_THROW_DAMAGE);
        let s = if grounded {
            K::SpecialNThrow
        } else {
            K::SpecialAirNThrow
        };
        set_inhale(f, s, 0.0, StatusTiming::frames(SPECIAL_N_THROW_LENGTH));
        return;
    }
    if (taps.contains(N64Buttons::B) || i32::from(f.stick.y) < VACUUM_COPY_STICK_RANGE_MIN)
        && f.grab.catch.is_some()
    {
        capture_damage(f, VACUUM_COPY_DAMAGE);
        let s = if grounded {
            K::SpecialNCopy
        } else {
            K::SpecialAirNCopy
        };
        set_inhale(f, s, 0.0, StatusTiming::frames(SPECIAL_N_COPY_LENGTH));
        return;
    }
    if grounded {
        let mut x = i32::from(f.stick.x);
        if x.abs() < VACUUM_TURN_STICK_RANGE_MIN {
            x = 0;
        }
        if (x < 0 && f.facing == Facing::Right) || (x > 0 && f.facing == Facing::Left) {
            set_inhale(
                f,
                K::SpecialNTurn,
                0.0,
                StatusTiming::frames(SPECIAL_N_TURN_LENGTH),
            );
        }
    }
}

/// `ftKirbySpecialNThrowUpdateCheckThrowStar`.
fn spit(f: &mut Fighter) {
    if f.grab.catch.take().is_none() {
        return;
    }
    f.grab.catch_kind = None;
    f.grab.capture_immune = false;
    let lr = f.facing.sign();
    // `-victim.lr * 120`, and the victim faces Kirby.
    f.grab.send(GrabEvent::KirbyStar {
        copy: false,
        vel: Vec3::new(lr * VACUUM_THROW_VEL_BASE, 0.0, 0.0),
    });
}

/// `ftKirbySpecialNCopyInitCopyVars` then
/// `ftKirbySpecialNCopyUpdateCheckCopyStar`.
fn copy(f: &mut Fighter) {
    if f.kirby.copy_id != f.kirby.copy_pending {
        f.kirby.copy_id = f.kirby.copy_pending;
        crate::kirby_copy::init_passive_vars(f);
    }
    if f.grab.catch.take().is_none() {
        return;
    }
    f.grab.catch_kind = None;
    f.grab.capture_immune = false;
    let victim_lr = -f.facing.sign();
    let (sin, cos) = sin_cos(VACUUM_COPY_ANGLE);
    f.grab.send(GrabEvent::KirbyStar {
        copy: true,
        vel: Vec3::new(
            cos * victim_lr * VACUUM_COPY_VEL_BASE,
            sin * VACUUM_COPY_VEL_BASE,
            0.0,
        ),
    });
}

/// `ftCommonCaptureWaitKirbyUpdateBreakoutVars`'s writes to Kirby.
pub fn on_wiggle(f: &mut Fighter, up: bool, push_x: Option<f32>) {
    if !matches!(
        f.status.status,
        AnyStatus::Kirby(K::SpecialNWait | K::SpecialAirNWait)
    ) {
        return;
    }
    if up && f.is_grounded() {
        switch_air(f);
        f.physics.vel_air.y = crate::capture_kirby::WIGGLE_VEL;
    }
    if let Some(x) = push_x {
        if f.is_grounded() {
            f.physics.vel_ground.x = x;
        } else {
            f.physics.vel_air.x = x;
        }
    }
}

// ---------------------------------------------------------------------------
// Callbacks
// ---------------------------------------------------------------------------

/// `proc_update` and `proc_interrupt` for Kirby's statuses.
pub fn update(f: &mut Fighter) {
    let AnyStatus::Kirby(current) = f.status.status else {
        return;
    };
    match current {
        K::Attack100Start => {
            if f.status.animation_ended() {
                set_attack100_loop(f);
            }
        }
        K::Attack100Loop => update_attack100_loop(f),
        K::Attack100End => {
            if f.status.animation_ended() {
                status::set_wait(f);
            }
        }
        K::JumpAerialF1 | K::JumpAerialF2 | K::JumpAerialF3 | K::JumpAerialF4 | K::JumpAerialF5 => {
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
        K::ThrowF => {
            if f.status.animation_ended() {
                set(f, K::ThrowFFall, 0.0, StatusTiming::unknown());
            }
        }
        K::ThrowFFall => {}
        K::ThrowFLanding => {
            if crossed(f, THROWF_RELEASE_FRAME) && f.grab.catch.is_some() {
                crate::grab::release_thrown(f, -f.facing.sign());
            }
            if crossed(f, THROWF_AIR_JUMP_ADD_FRAME) {
                // `SetAirJumpAdd`.
                f.become_airborne();
                f.floor = None;
                f.physics.vel_air.z = 0.0;
                f.physics.jumps_used += 1;
            }
            if f.status.animation_ended() {
                status::set_wait_or_fall(f);
            }
        }
        K::SpecialHi | K::SpecialAirHi => {
            if crossed(f, CUTTER_AIR_JUMP_MAX_FRAME) {
                air_jump_max(f);
            }
            if f.status.animation_ended() {
                set_special_air_hi_fall(f);
            }
        }
        K::SpecialAirHiFall => {}
        K::SpecialHiLanding => {
            if !f.kirby.cutter_spawned && f.status.anim_frame >= CUTTER_WAVE_FRAME {
                make_cutter(f);
            }
            if f.status.animation_ended() {
                status::set_wait(f);
            }
        }
        K::SpecialLwStart => {
            if crossed(f, STONE_START_FLAG1_FRAME) {
                set_frames(f, K::SpecialLwHold, 0.0, STONE_LENGTH);
                set_damage_resist(f);
                f.kirby.stone_flag1 = false;
            } else if f.status.animation_ended() {
                set_stone(f, K::SpecialLwUnk, 0.0);
            }
        }
        K::SpecialAirLwStart => {
            if crossed(f, STONE_AIR_START_FLAG1_FRAME) {
                set_damage_resist(f);
                f.kirby.stone_flag1 = false;
            }
            if f.status.animation_ended() {
                set_stone(f, K::SpecialAirLwHold, 0.0);
                f.physics.vel_air.y = STONE_FALL_VEL;
            }
        }
        K::SpecialLwUnk => stone_unk_decide(f, true),
        K::SpecialAirLwHold => stone_unk_decide(f, false),
        K::SpecialLwHold | K::SpecialAirLwLanding => stone_hold_decide(f, true),
        K::SpecialAirLwFall => stone_hold_decide(f, false),
        K::SpecialLwEnd | K::SpecialAirLwEnd => {
            if f.status.animation_ended() {
                status::set_fall(f);
            }
        }
        K::SpecialNStart | K::SpecialAirNStart => {
            if f.status.animation_ended() {
                let s = if current == K::SpecialNStart {
                    K::SpecialNLoop
                } else {
                    K::SpecialAirNLoop
                };
                set_inhale(f, s, 0.0, StatusTiming::unknown());
            }
        }
        K::SpecialNLoop | K::SpecialAirNLoop => {
            // `ftKirbySpecialNLoopCheckContinueLoop`.
            let hold = if f.kirby.release_lag != 0 {
                f.kirby.release_lag -= 1;
                true
            } else {
                f.input.buttons.contains(N64Buttons::B)
            };
            if !hold {
                let s = if current == K::SpecialNLoop {
                    K::SpecialNEnd
                } else {
                    K::SpecialAirNEnd
                };
                set_inhale(f, s, 0.0, StatusTiming::frames(SPECIAL_N_END_LENGTH));
            }
        }
        K::SpecialNEnd => {
            if f.status.animation_ended() {
                status::set_wait(f);
            }
        }
        K::SpecialAirNEnd => {
            if f.status.animation_ended() {
                status::set_fall(f);
            }
        }
        K::SpecialNCatch | K::SpecialAirNCatch => {
            let d = f.kirby.inhale_dist;
            if d.x * d.x + d.y * d.y < VACUUM_SPECIALNWAIT_DIST_MIN {
                f.grab.send(GrabEvent::KirbyEat {
                    is_kirby: f.kirby.victim_is_kirby,
                });
                let s = if current == K::SpecialNCatch {
                    K::SpecialNEat
                } else {
                    K::SpecialAirNEat
                };
                set_inhale(f, s, 0.0, StatusTiming::frames(SPECIAL_N_EAT_LENGTH));
            } else {
                // The victim's `ftCommonCaptureKirbyProcPhysics` pulls it in.
                f.kirby.inhale_dist = crate::capture_kirby::decay_dist(d);
            }
        }
        K::SpecialNEat | K::SpecialAirNEat => {
            if f.status.animation_ended() {
                let s = if inhale_ground(f) {
                    K::SpecialNWait
                } else {
                    K::SpecialAirNWait
                };
                set_inhale(f, s, 0.0, StatusTiming::unknown());
            }
        }
        K::SpecialNWait | K::SpecialAirNWait => update_inhale_wait(f),
        K::SpecialNTurn | K::SpecialAirNTurn => {
            if f.status.animation_ended() {
                // `ftKirbySpecialNWaitUpdateLR`.
                f.facing = f.facing.flipped();
                let s = if current == K::SpecialNTurn {
                    K::SpecialNWait
                } else {
                    K::SpecialAirNWait
                };
                set_inhale(f, s, 0.0, StatusTiming::unknown());
            }
        }
        K::SpecialNThrow | K::SpecialAirNThrow => {
            if crossed(f, INHALE_SPIT_FRAME) {
                spit(f);
            }
            if f.status.animation_ended() {
                if current == K::SpecialNThrow {
                    status::set_wait(f);
                } else {
                    status::set_fall(f);
                }
            }
        }
        K::SpecialNCopy | K::SpecialAirNCopy => {
            if crossed(f, INHALE_COPY_FRAME) {
                copy(f);
            }
            if f.status.animation_ended() {
                if current == K::SpecialNCopy {
                    status::set_wait(f);
                } else {
                    status::set_fall(f);
                }
            }
        }
        _ => crate::kirby_copy::update(f),
    }
}

/// Kirby's grounded `proc_physics` where it is not plain friction.
pub fn apply_ground_physics(f: &mut Fighter) -> bool {
    let AnyStatus::Kirby(current) = f.status.status else {
        return false;
    };
    match current {
        K::SpecialHi => {
            // Position follows `vel_air` in the source; X is the drift.
            apply_air_vel_transn_yz(f, 1.0);
            cutter_drift(f);
            f.physics.vel_ground.x = f.physics.vel_air.x;
        }
        K::SpecialHiLanding | K::SpecialNThrow => {
            physics::apply_ground_vel_transn(&mut f.physics, f.root_motion, f.facing.sign());
        }
        // `proc_physics` is NULL and the setter zeroed `vel_air`.
        K::SpecialLwStart => f.physics.vel_ground.x = 0.0,
        K::SpecialLwHold | K::SpecialAirLwLanding => {
            // `ftKirbySpecialLwHoldProcPhysics`: slide down slopes.
            let (normal, flags) = f
                .floor
                .map(|s| (s.normal, s.flags))
                .unwrap_or((Vec2::new(0.0, 1.0), 0));
            let angle = (-atan2(normal.x, normal.y)).clamp(-STONE_SLIDE_ANGLE, STONE_SLIDE_ANGLE);
            let (sin, _) = sin_cos(angle);
            // `vel_ground` is facing-relative in the source.
            let lr = f.facing.sign();
            let mut vel = f.physics.vel_ground.x * lr;
            vel -= sin * STONE_SLIDE_VEL_MUL * lr;
            vel = vel.clamp(-STONE_SLIDE_CLAMP_VEL_X, STONE_SLIDE_CLAMP_VEL_X);
            f.physics.vel_ground.x = vel * lr;
            let friction = crate::collision::material_friction(flags)
                * f.attributes.traction
                * STONE_SLIDE_TRACTION_MUL;
            physics::apply_ground_friction(&mut f.physics, friction);
        }
        _ => return crate::kirby_copy::apply_ground_physics(f),
    }
    true
}

/// Kirby's airborne `proc_physics`. Every Kirby air status has its own.
pub fn apply_air_physics(f: &mut Fighter) -> bool {
    let AnyStatus::Kirby(current) = f.status.status else {
        return false;
    };
    if crate::kirby_copy::apply_air_physics(f) {
        return true;
    }
    let attr = f.attributes;
    match current {
        K::JumpAerialF1 | K::JumpAerialF2 | K::JumpAerialF3 | K::JumpAerialF4 | K::JumpAerialF5 => {
            status::check_set_fast_fall(f);
            if f.physics.is_fastfall {
                physics::apply_fast_fall(&mut f.physics, &attr);
            } else {
                physics::apply_gravity_default(&mut f.physics, &attr);
            }
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
        K::ThrowF
        | K::ThrowFLanding
        | K::SpecialLwEnd
        | K::SpecialAirLwEnd
        | K::SpecialAirNThrow => {
            physics::apply_air_vel_transn_all(&mut f.physics, f.root_motion, f.facing.sign());
        }
        // `proc_physics` is NULL: the velocity carries.
        K::ThrowFFall | K::SpecialAirLwStart | K::SpecialAirLwHold | K::SpecialAirLwFall => {}
        K::SpecialHi => {
            apply_air_vel_transn_yz(f, 1.0);
            cutter_drift(f);
        }
        K::SpecialAirHi => {
            apply_air_vel_transn_yz(f, FINALCUTTER_AIR_TRANSN_SCALE);
            cutter_drift(f);
        }
        K::SpecialAirHiFall => cutter_drift(f),
        K::SpecialHiLanding => {
            apply_air_vel_transn_yz(f, 1.0);
            cutter_drift(f);
        }
        K::SpecialAirNWait | K::SpecialAirNTurn => {
            if f.physics.is_fastfall {
                physics::apply_fast_fall(&mut f.physics, &attr);
            } else {
                physics::apply_gravity_clamp_tvel(
                    &mut f.physics,
                    VACUUM_GRAVITY_MUL * attr.gravity,
                    VACUUM_FALL_MAX_MUL * attr.tvel_base,
                );
            }
            if !physics::check_clamp_air_vel_x_dec(&mut f.physics, attr.air_speed_max_x) {
                physics::apply_air_friction(&mut f.physics, &attr);
            }
        }
        // `ftPhysicsApplyAirVelFriction`: no drift input.
        _ => {
            if f.physics.is_fastfall {
                physics::apply_fast_fall(&mut f.physics, &attr);
            } else {
                physics::apply_gravity_default(&mut f.physics, &attr);
            }
            if !physics::check_clamp_air_vel_x_dec(&mut f.physics, attr.air_speed_max_x) {
                physics::apply_air_friction(&mut f.physics, &attr);
            }
        }
    }
    true
}

/// Only the aerial jumps check fast fall, from their own physics.
pub fn skips_fast_fall(status: AnyStatus) -> bool {
    matches!(status, AnyStatus::Kirby(_))
}

/// A grounded Inhale status's switch into its aerial twin, same frame.
fn switch_air(f: &mut Fighter) {
    let AnyStatus::Kirby(current) = f.status.status else {
        return;
    };
    let air = match current {
        K::SpecialNStart => K::SpecialAirNStart,
        K::SpecialNLoop => K::SpecialAirNLoop,
        K::SpecialNEnd => K::SpecialAirNEnd,
        K::SpecialNCatch => K::SpecialAirNCatch,
        K::SpecialNEat => K::SpecialAirNEat,
        K::SpecialNThrow => K::SpecialAirNThrow,
        K::SpecialNWait => K::SpecialAirNWait,
        K::SpecialNTurn => K::SpecialAirNTurn,
        K::SpecialNCopy => K::SpecialAirNCopy,
        _ => return,
    };
    let (frame, timing) = (f.status.anim_frame, f.status.timing);
    f.become_airborne();
    set_inhale(f, air, frame, timing);
}

fn switch_ground(f: &mut Fighter, y: f32) -> bool {
    let AnyStatus::Kirby(current) = f.status.status else {
        return false;
    };
    let ground = match current {
        K::SpecialAirNStart => K::SpecialNStart,
        K::SpecialAirNLoop => K::SpecialNLoop,
        K::SpecialAirNEnd => K::SpecialNEnd,
        K::SpecialAirNCatch => K::SpecialNCatch,
        K::SpecialAirNEat => K::SpecialNEat,
        K::SpecialAirNThrow => K::SpecialNThrow,
        K::SpecialAirNWait => K::SpecialNWait,
        K::SpecialAirNTurn => K::SpecialNTurn,
        K::SpecialAirNCopy => K::SpecialNCopy,
        _ => return false,
    };
    let (frame, timing) = (f.status.anim_frame, f.status.timing);
    f.land(y);
    set_inhale(f, ground, frame, timing);
    true
}

/// Kirby's grounded `proc_map` when the floor ran out.
pub fn on_ground_lost(f: &mut Fighter) -> bool {
    let AnyStatus::Kirby(current) = f.status.status else {
        return false;
    };
    match current {
        K::SpecialNStart
        | K::SpecialNLoop
        | K::SpecialNEnd
        | K::SpecialNCatch
        | K::SpecialNEat
        | K::SpecialNThrow
        | K::SpecialNWait
        | K::SpecialNTurn
        | K::SpecialNCopy => switch_air(f),
        K::SpecialLwStart => {
            let frame = f.status.anim_frame;
            f.become_airborne();
            set_frames(f, K::SpecialAirLwStart, frame, SPECIAL_AIR_LW_START_LENGTH);
            f.physics.vel_air.y = STONE_FALL_VEL;
        }
        K::SpecialLwUnk => {
            let frame = f.status.anim_frame;
            f.become_airborne();
            set_stone(f, K::SpecialAirLwHold, frame);
        }
        K::SpecialLwHold | K::SpecialAirLwLanding => {
            f.become_airborne();
            set(f, K::SpecialAirLwFall, 0.0, StatusTiming::unknown());
            f.kirby.is_damage_resist = true;
            f.physics.vel_air.y = STONE_FALL_VEL;
        }
        // `ftCommonCatchProcMap`: the held fighter drops.
        K::ThrowFLanding => {
            f.become_airborne();
            crate::grab::release_on_edge(f);
        }
        // `SetAirJumpMax` has not run yet; the status carries on in the air.
        K::SpecialHi => f.become_airborne(),
        // `mpCommonSetFighterFallOnEdgeBreak` / `...OnGroundBreak`.
        K::Attack100Start | K::Attack100Loop | K::Attack100End | K::SpecialHiLanding => {
            f.become_airborne();
            status::set_fall(f);
        }
        _ => return crate::kirby_copy::on_ground_lost(f),
    }
    true
}

/// Kirby's aerial `proc_map` on a floor contact.
pub fn on_landing(f: &mut Fighter, y: f32) -> bool {
    let AnyStatus::Kirby(current) = f.status.status else {
        return false;
    };
    match current {
        K::JumpAerialF1 | K::JumpAerialF2 | K::JumpAerialF3 | K::JumpAerialF4 | K::JumpAerialF5 => {
            status::set_landing(f);
            f.land(y);
        }
        K::ThrowF | K::ThrowFFall => {
            if f.physics.vel_air.y < 0.0 {
                f.land(y);
                set_frames(f, K::ThrowFLanding, 0.0, THROWF_LANDING_LENGTH);
            } else {
                f.pos.y = y;
                f.floor = None;
            }
        }
        K::ThrowFLanding => {
            if f.physics.vel_air.y < 0.0 {
                wait_or_landing(f, y);
            } else {
                f.pos.y = y;
                f.floor = None;
            }
        }
        K::SpecialHi | K::SpecialAirHi => {
            if f.physics.vel_air.y < 0.0 {
                f.land(y);
                set_special_hi_landing(f);
            } else {
                f.pos.y = y;
                f.floor = None;
            }
        }
        K::SpecialAirHiFall => {
            f.land(y);
            set_special_hi_landing(f);
        }
        K::SpecialHiLanding => f.land(y),
        K::SpecialAirLwStart => {
            let frame = f.status.anim_frame;
            f.land(y);
            set_frames(f, K::SpecialLwStart, frame, SPECIAL_LW_START_LENGTH);
        }
        K::SpecialAirLwHold | K::SpecialAirLwFall => {
            f.land(y);
            set_frames(f, K::SpecialAirLwLanding, 0.0, STONE_LENGTH);
            f.kirby.is_damage_resist = true;
        }
        K::SpecialLwEnd | K::SpecialAirLwEnd => wait_or_landing(f, y),
        _ => {
            if !crate::kirby_copy::on_landing(f, y) && !switch_ground(f, y) {
                return false;
            }
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fighter::Situation;
    use ssb_engine::input::ControllerState;

    fn kirby() -> Fighter {
        let mut f = Fighter::new(FighterKind::Kirby, 0, 4);
        f.attributes.jumps_max = 6;
        f
    }

    fn press(f: &mut Fighter, buttons: u16, x: i8, y: i8) {
        f.set_input(
            ControllerState {
                buttons: N64Buttons(buttons),
                stick_x: x,
                stick_y: y,
                connected: true,
            },
            false,
            false,
        );
    }

    #[test]
    fn five_aerial_jumps_use_the_velocity_table() {
        let mut f = kirby();
        f.physics.jumps_used = 1;
        set_jump_aerial(&mut f);
        assert_eq!(f.status.status, AnyStatus::Kirby(K::JumpAerialF1));
        for (i, v) in JUMPAERIAL_VELOCITIES.iter().enumerate() {
            set_jump_aerial(&mut f);
            assert_eq!(f.physics.vel_air.y, *v);
            assert_eq!(f.physics.jumps_used as usize, i + 3);
        }
        assert_eq!(f.status.status, AnyStatus::Kirby(K::JumpAerialF5));
        assert!(!check_jump_aerial(&mut f));
    }

    #[test]
    fn a_held_stick_jumps_again_only_after_flag_one() {
        let mut f = kirby();
        f.physics.jumps_used = 2;
        set_jump_aerial(&mut f);
        press(&mut f, 0, 0, 80);
        f.status.anim_frame = 27.0;
        assert!(!check_jump_aerial(&mut f));
        f.status.anim_frame = 28.0;
        assert!(check_jump_aerial(&mut f));
        assert_eq!(f.status.status, AnyStatus::Kirby(K::JumpAerialF3));
    }

    #[test]
    fn rapid_loop_resumes_its_script_on_the_wrap_and_ends_without_input() {
        let mut f = kirby();
        f.situation = Situation::Ground;
        set_attack100_start(&mut f);
        set_attack100_loop(&mut f);
        let live = |f: &Fighter| {
            f.attack_colls
                .iter()
                .any(|c| c.state != crate::combat::AttackState::Off)
        };
        // Mashing keeps the loop going across the wrap, and the resumed
        // script keeps making pulses.
        let mut pulses_after_wrap = 0;
        for frame in 0..60 {
            press(&mut f, if frame % 2 == 0 { 0x8000 } else { 0 }, 0, 0);
            status::update(&mut f);
            assert_eq!(f.status.status, AnyStatus::Kirby(K::Attack100Loop));
            if frame > 26 && live(&f) {
                pulses_after_wrap += 1;
            }
        }
        assert!(f.kirby.rapid_is_anim_end);
        assert!(pulses_after_wrap > 0);
        // Without input, the next flag 1 ends the loop.
        for _ in 0..30 {
            press(&mut f, 0, 0, 0);
            status::update(&mut f);
            if f.status.status != AnyStatus::Kirby(K::Attack100Loop) {
                break;
            }
        }
        assert_eq!(f.status.status, AnyStatus::Kirby(K::Attack100End));
    }

    #[test]
    fn a_tumble_costs_the_copy_one_time_in_twelve_on_the_shared_generator() {
        // Seed 1: count losses over 1,200 tumbles; the LCG is deterministic.
        crate::rng::set_seed(1);
        let mut lost = 0;
        for _ in 0..1200 {
            let mut f = kirby();
            f.kirby.copy_id = FighterKind::Mario;
            damage_check_lose_copy(&mut f);
            if f.kirby.copy_id == FighterKind::Kirby {
                lost += 1;
            }
        }
        assert!((70..130).contains(&lost), "{lost}");
        // A Kirby made with a copy never loses it.
        let mut f = kirby();
        f.kirby.copy_id = FighterKind::Mario;
        f.kirby.is_ignore_losecopy = true;
        for _ in 0..100 {
            damage_check_lose_copy(&mut f);
        }
        assert_eq!(f.kirby.copy_id, FighterKind::Mario);
    }

    #[test]
    fn stone_soaks_its_health_then_passes_the_overflow() {
        let mut f = kirby();
        f.situation = Situation::Ground;
        set_special_lw(&mut f);
        f.status.anim_frame = 6.0;
        update(&mut f);
        assert_eq!(f.status.status, AnyStatus::Kirby(K::SpecialLwHold));
        // `ftMainCheckGetUpdateDamage`: 30 of the 34 health is soaked, then a
        // 12-damage hit passes its 4 overflow on.
        let mut hit = crate::weapon::MARIO_FIREBALL_HITBOX;
        hit.damage = 30;
        let at = f.pos + ssb_engine::math::Vec3::new(0.0, 100.0, 0.0);
        crate::attack::apply_hitbox_at(&hit, at, crate::stale::HANDICAP_DEFAULT, &mut f);
        assert_eq!(f.damage, 0);
        assert!(f.kirby.is_damage_resist);
        f.hitlag = 0;
        hit.damage = 12;
        crate::attack::apply_hitbox_at(&hit, at, crate::stale::HANDICAP_DEFAULT, &mut f);
        assert_eq!(f.damage, 4);
        assert!(!f.kirby.is_damage_resist);
    }

    #[test]
    fn grounded_stone_releases_on_b_or_after_its_duration() {
        let mut f = kirby();
        f.situation = Situation::Ground;
        set_special_lw(&mut f);
        f.status.anim_frame = 6.0;
        update(&mut f);
        for _ in 0..STONE_DURATION_MAX {
            update(&mut f);
            assert_eq!(f.status.status, AnyStatus::Kirby(K::SpecialLwHold));
        }
        update(&mut f);
        assert_eq!(f.status.status, AnyStatus::Kirby(K::SpecialLwEnd));
        assert!(!f.is_grounded());
    }

    #[test]
    fn inhale_releases_after_the_lag_without_b() {
        let mut f = kirby();
        f.situation = Situation::Ground;
        set_special_n(&mut f);
        f.status.anim_frame = SPECIAL_N_START_LENGTH;
        update(&mut f);
        assert_eq!(f.status.status, AnyStatus::Kirby(K::SpecialNLoop));
        assert!(inhale_searching(&f));
        for _ in 0..VACUUM_RELEASE_LAG {
            update(&mut f);
        }
        assert_eq!(f.status.status, AnyStatus::Kirby(K::SpecialNLoop));
        update(&mut f);
        assert_eq!(f.status.status, AnyStatus::Kirby(K::SpecialNEnd));
        assert!(!f.grab.is_catchstatus);
    }

    #[test]
    fn inhale_eats_once_the_victim_is_pulled_in_and_offers_its_copy() {
        let mut f = kirby();
        f.situation = Situation::Ground;
        set_special_n(&mut f);
        set_inhale(&mut f, K::SpecialNLoop, 0.0, StatusTiming::unknown());
        let mut fox = Fighter::new(FighterKind::Fox, 1, 4);
        fox.pos = Vec3::new(400.0, 100.0, 0.0);
        inhale_catch(&mut f, &fox);
        assert_eq!(f.status.status, AnyStatus::Kirby(K::SpecialNCatch));
        assert_eq!(f.kirby.copy_pending, FighterKind::Fox);
        let mut frames = 0;
        while f.status.status == AnyStatus::Kirby(K::SpecialNCatch) {
            update(&mut f);
            frames += 1;
        }
        // 240 across at 28 per frame: nine pulls, then the eat.
        assert_eq!(frames, 9);
        assert_eq!(f.status.status, AnyStatus::Kirby(K::SpecialNEat));
        assert!(f
            .grab
            .outbox
            .iter()
            .any(|e| matches!(e, Some(GrabEvent::KirbyEat { is_kirby: false }))));
    }

    #[test]
    fn copy_takes_the_ability_and_sends_the_star_backward() {
        let mut f = kirby();
        f.situation = Situation::Ground;
        f.grab.catch = Some(1);
        f.kirby.copy_pending = FighterKind::Captain;
        set_inhale(&mut f, K::SpecialNCopy, 0.0, StatusTiming::frames(30.0));
        f.status.anim_frame = INHALE_COPY_FRAME;
        update(&mut f);
        assert_eq!(f.kirby.copy_id, FighterKind::Captain);
        assert!(f.grab.catch.is_none());
        let star = f.grab.outbox.iter().flatten().find_map(|e| match e {
            GrabEvent::KirbyStar { copy: true, vel } => Some(*vel),
            _ => None,
        });
        let vel = star.expect("the copy star is released");
        assert!(vel.x < 0.0 && vel.y > 0.0);
    }

    #[test]
    fn final_cutter_goes_airborne_at_air_jump_max_and_makes_its_wave_on_landing() {
        let mut f = kirby();
        f.situation = Situation::Ground;
        set_special_hi(&mut f);
        f.status.anim_frame = 22.0;
        update(&mut f);
        assert!(f.is_grounded());
        f.status.anim_frame = 23.0;
        update(&mut f);
        assert!(!f.is_grounded());
        assert_eq!(f.physics.jumps_used, 6);
        f.physics.vel_air.y = -10.0;
        assert!(on_landing(&mut f, 0.0));
        assert_eq!(f.status.status, AnyStatus::Kirby(K::SpecialHiLanding));
        f.status.anim_frame = 2.0;
        update(&mut f);
        assert!(f.weapon_spawn.is_none());
        f.status.anim_frame = 3.0;
        update(&mut f);
        let spawn = f.take_weapon_spawn().expect("wave at frame 3");
        assert!(matches!(
            spawn.kind,
            WeaponKind::KirbyCutter { grounded: true }
        ));
        assert_eq!(spawn.position.x, FINALCUTTER_OFF_X);
    }
}
