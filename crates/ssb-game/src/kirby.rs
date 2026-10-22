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
use ssb_engine::math::{sin_cos, sy_arc_tan2, Vec2, Vec3};

use crate::fighter::{Facing, Fighter, FighterKind};
use crate::grab::GrabEvent;
use crate::physics;
use crate::status::{self, AnyStatus, KirbyStatus as K, StatusTiming};
use crate::weapon::{WeaponKind, WeaponSpawn};

/// `FTKIRBY_JUMPAERIAL_VEL_MUL` and `dFTKirbyJumpAerialFVelocities`.
pub const JUMPAERIAL_VEL_MUL: f32 = 0.8;
pub const JUMPAERIAL_VELOCITIES: [f32; 4] = crate::d_arrays::FT_KIRBY_JUMP_AERIAL_F_VELOCITIES;
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
/// `FTKIRBY_VACUUM_STOPGFX_DIST_MIN` (squared: 96 units).
pub const VACUUM_STOPGFX_DIST_MIN: f32 = 9216.0;
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
/// `FTKIRBY_STONE_HEALTH_MID` (US) and `..._LOW`: Stone flashes faster below
/// each.
pub const STONE_HEALTH_MID: i32 = 22;
pub const STONE_HEALTH_LOW: i32 = 10;

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
    /// Final Cutter's attached effects' played animation frames, by
    /// [`CutterEffect`] index, while they live.
    pub cutter_effects: [Option<u16>; CutterEffect::COUNT],
    /// `FTStruct::is_damage_resist` and `damage_resist`. `set_any_status`
    /// clears the flag, as `ftMainSetStatus` does.
    pub is_damage_resist: bool,
    pub damage_resist: i32,
    /// `status_vars.kirby.speciallw.duration`.
    pub stone_duration: i32,
    /// `status_vars.kirby.speciallw.colanim_id`: Stone's flash for its
    /// remaining health.
    pub stone_colanim: crate::colanim::ColAnimId,
    /// Stone's motion flags 1 and 2.
    pub stone_flag1: bool,
    pub stone_flag2: bool,
    /// `status_vars.kirby.specialn.release_lag`.
    pub release_lag: i32,
    /// Inhale's motion flag 0: the Loop scripts' `SetFlag0(1)` asks
    /// `ftKirbySpecialNLoopProcUpdate` for the wind, whose making clears it.
    pub inhale_wind_flag0: bool,
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
            cutter_effects: [None; CutterEffect::COUNT],
            is_damage_resist: false,
            damage_resist: 0,
            stone_duration: 0,
            stone_colanim: crate::colanim::ColAnimId::NONE,
            stone_flag1: false,
            stone_flag2: false,
            release_lag: 0,
            inhale_wind_flag0: false,
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

/// Stone's `ftMainSetStatus` calls that pass `FTSTATUS_PRESERVE_HITSTATUS`
/// (`ftkirbyspeciallw.c`; their `MODELPART` and `COLANIM` come from the
/// [`crate::colanim`] preserve tables).
fn set_keep_hitstatus(f: &mut Fighter, s: K, frame: f32, timing: StatusTiming) {
    status::set_any_status_preserve(
        f,
        AnyStatus::Kirby(s),
        frame,
        timing,
        status::Preserve::HITSTATUS,
    );
}

/// The motion script's clock passing `at` this play: the status clock,
/// which runs on past the figatree's end (RE-474).
fn crossed(f: &Fighter, at: f32) -> bool {
    let frame = f.status.clock;
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
    physics::apply_air_vel_transn_all(&mut f.physics, motion, f.facing.sign(), f.attributes.size);
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

/// `ftKirbySpecialNLoseCopy`: the lost star
/// (`efManagerLoseKirbyStarMakeEffect`) leaves from where Kirby stands now.
/// Source order: passive vars, star, FGM, `copy_id`, hat, colour animation.
pub fn lose_copy(f: &mut Fighter) {
    crate::kirby_copy::init_passive_vars(f);
    f.effects
        .push(crate::fteffect::FighterEffect::LoseKirbyStar {
            pos: f.pos,
            lr: f.facing.sign() as i8,
        });
    crate::sound::play_fgm(crate::sound::id::nSYAudioFGMKirbySpecialNLoseCopy);
    f.kirby.copy_id = FighterKind::Kirby;
    f.model_parts
        .set_default(crate::kirby_copy::COPY_MODELPARTS_JOINT, 0);
    crate::colanim::reset_stat_update(f);
}

/// `FTKirbyCopy[27]` (file 228 + 0x0, `llKirbyMainMotionSpecialNFTKirbyCopy`):
/// `(copy_id, star_damage)` per swallowed `FTKind`. The model-part column is
/// [`crate::kirby_copy::COPY_MODELPART_IDS`] (RE-417); the scale column is
/// [`EFFECT_SCALE`]. ROM data generated at build time by `ssb-tablegen`, never
/// committed; zeros in stub builds.
pub use generated_copy::COPY;
/// `FTKirbyCopy::effect_scale` per swallowed `FTKind`: the captured star's
/// size and its sparks' scatter (`efManagerCaptureKirbyStar*`). ROM data
/// generated at build time; zeros in stub builds.
pub use generated_copy::EFFECT_SCALE;

/// [`EFFECT_SCALE`] for `kind`'s `FTKind` (0 past the table).
pub fn effect_scale(kind: FighterKind) -> f32 {
    EFFECT_SCALE.get(kind as usize).copied().unwrap_or(0.0)
}
#[allow(clippy::all)]
pub(crate) mod generated_copy {
    include!(concat!(env!("OUT_DIR"), "/kirby_copy.rs"));
}

/// `FTKirbyCopy::star_damage` for a swallowed fighter kind.
pub fn star_damage(kind: FighterKind) -> i32 {
    COPY[kind as usize].1
}

// ---------------------------------------------------------------------------
// Rapid jab (`ftcommonattack100.c`)
// ---------------------------------------------------------------------------

/// `ftKirbyAttack100Effect`: one rapid-jab puff's TopN-local offset, its
/// angle in degrees and its speed and per-frame change.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Attack100Effect {
    pub offset: Vec3,
    pub rotate: f32,
    pub vel: f32,
    pub add: f32,
}

#[allow(clippy::all)]
mod attack100_table {
    include!(concat!(env!("OUT_DIR"), "/kirby_attack100.rs"));
}

/// Row `index` of `llKirbyMainMotionftKirbyAttack100Effect` (file 228),
/// generated from the ROM (D-048); zeros in a stub build.
pub fn attack100_effect(index: usize) -> Attack100Effect {
    let [x, y, z, rotate, vel, add] = attack100_table::ATTACK100_EFFECT[index];
    Attack100Effect {
        offset: Vec3::new(x, y, z),
        rotate,
        vel,
        add,
    }
}

/// `ftCommonAttack100StartSetStatus`'s Kirby case.
pub fn set_attack100_start(f: &mut Fighter) {
    if crate::item_throw::check_get(f) {
        return;
    }
    set_frames(f, K::Attack100Start, 0.0, ATTACK100_START_LENGTH);
    // `ftCommonAttack100StartSetStatus` plays the first frame (RE-474).
    status::play_anim_events(f);
    f.kirby.rapid_is_anim_end = false;
    f.kirby.rapid_is_goto_loop = false;
    f.motion_script.flags[1] = 0;
    f.motion_script.flags[2] = 0;
}

/// `ftCommonAttack100LoopSetStatus`: the loop's time-zero script sets flag
/// 2, so the first puff is made here, before any `proc_update` (RE-513).
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
    attack100_loop_update_effect(f);
}

/// `ftCommonAttack100LoopKirbyUpdateEffect`: Kirby only (`fp->fkind ==
/// nFTKindKirby`, so not Polygon Kirby). On flag 2 the row `flag2 - 1`
/// offset goes through TopN to the world, marking TopN queried
/// (`gmCollisionGetFighterPartsWorldPosition`, RE-511), and a puff is made
/// there; flag 2 clears whether or not the maker found a struct.
fn attack100_loop_update_effect(f: &mut Fighter) {
    if f.kind != FighterKind::Kirby {
        return;
    }
    let flag2 = f.motion_script.flags[2];
    if flag2 == 0 {
        return;
    }
    let row = attack100_effect(usize::try_from(flag2 - 1).unwrap_or(0).min(5));
    let pos = f.joint_query(0, row.offset);
    f.effects.push(crate::fteffect::FighterEffect::VulcanJab {
        pos,
        lr: f.facing.sign() as i8,
        rotate: row.rotate,
        vel: row.vel,
        add: row.add,
    });
    f.motion_script.flags[2] = 0;
}

/// `ftCommonAttack100LoopProcUpdate` and `...ProcInterrupt`.
fn update_attack100_loop(f: &mut Fighter) {
    let speed = f.status.timing.anim_speed;
    if f.status.anim_frame >= 0.0 && f.status.anim_frame < speed {
        f.kirby.rapid_is_anim_end = true;
        // `ftParamSetMotionID`: each cycle is a new motion.
        f.set_motion_id(crate::stale::MotionAttackId::Attack100);
        f.stats.restart();
    }
    if f.motion_script.flags[1] != 0 {
        f.motion_script.flags[1] = 0;
        if f.kirby.rapid_is_anim_end && !f.kirby.rapid_is_goto_loop {
            set_frames(f, K::Attack100End, 0.0, ATTACK100_END_LENGTH);
            return;
        }
        if crate::item_throw::check_get(f) {
            return;
        }
        f.kirby.rapid_is_goto_loop = false;
    }
    attack100_loop_update_effect(f);
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
        if is_jump_aerial(f.status.status) && f.status.clock < JUMPAERIAL_FLAG1_FRAME {
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
    status::set_any_status_preserve(
        f,
        AnyStatus::Kirby(status),
        0.0,
        StatusTiming::frames(JUMPAERIAL_LENGTH),
        status::Preserve {
            playertag: true,
            ..status::Preserve::NONE
        },
    );
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
    // `ftCommonThrowSetStatus` plays the first frame.
    status::play_anim_events(f);
}

// ---------------------------------------------------------------------------
// Final Cutter (`ftkirbyspecialhi.c`)
// ---------------------------------------------------------------------------

/// `ftKirbySpecialHiSetStatus` and `ftKirbySpecialAirHiSetStatus`:
/// `ftKirbySpecialHiProcStatus` clears flags 0 to 2, the first frame
/// plays, then `proc_lagstart`/`proc_lagend` hold the Final Cutter's
/// effects through hitlag.
pub fn set_special_hi(f: &mut Fighter) {
    let s = if f.is_grounded() {
        K::SpecialHi
    } else {
        K::SpecialAirHi
    };
    set_frames(f, s, 0.0, SPECIAL_HI_LENGTH);
    f.kirby.cutter_spawned = false;
    f.kirby.cutter_effects = [None; CutterEffect::COUNT];
    f.motion_script.flags[..3].fill(0);
    // The setter plays the first frame (RE-473: the N64 takes off 22
    // frames on and falls 59 frames on).
    status::play_anim_events(f);
    crate::fteffect::set_lag_pauses_effects(f);
}

/// `ftKirbySpecialAirHiFallSetStatus`: the vertical velocity survives, and
/// the effects too (`FTSTATUS_PRESERVE_EFFECT`).
fn set_special_air_hi_fall(f: &mut Fighter) {
    let vel_y = f.physics.vel_air.y;
    f.become_airborne();
    set(f, K::SpecialAirHiFall, 0.0, StatusTiming::unknown());
    // The setter plays the first frame (RE-473).
    status::play_anim_events(f);
    crate::fteffect::set_lag_pauses_effects(f);
    f.physics.jumps_used = f.attributes.jumps_max;
    f.physics.vel_air.y = vel_y;
}

/// `ftKirbySpecialHiLandingSetStatus`, from `SpecialHi`/`SpecialAirHi`'s
/// `proc_map`: `FTSTATUS_PRESERVE_EFFECT`, and the first frame plays.
fn set_special_hi_landing(f: &mut Fighter) {
    set_frames(f, K::SpecialHiLanding, 0.0, SPECIAL_HI_LANDING_LENGTH);
    f.kirby.cutter_spawned = false;
    status::play_anim_events(f);
    crate::fteffect::set_lag_pauses_effects(f);
}

/// `ftKirbySpecialAirHiFallProcMap`'s landing: its own `ftMainSetStatus`
/// with `FTSTATUS_PRESERVE_NONE` (the falling Cutter's effect stops) and
/// no first-frame play.
fn set_special_hi_landing_from_fall(f: &mut Fighter) {
    set_frames(f, K::SpecialHiLanding, 0.0, SPECIAL_HI_LANDING_LENGTH);
    f.kirby.cutter_spawned = false;
    f.kirby.cutter_effects = [None; CutterEffect::COUNT];
    crate::fteffect::set_lag_pauses_effects(f);
}

/// Final Cutter's attached effects (`efManagerKirbyCutter*MakeEffect`,
/// file 348), by motion flag 2's value less 2.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CutterEffect {
    /// `dEFManagerKirbyCutterDrawEffectDesc`: the blade, on joint 17's
    /// whole matrix (battle matrix function 79), unanimated.
    Draw,
    /// `dEFManagerKirbyCutterUpEffectDesc`: the rising swing, at TopN.
    Up,
    /// `dEFManagerKirbyCutterDownEffectDesc`: the falling swing, at TopN.
    Down,
    /// `dEFManagerKirbyCutterTrailEffectDesc`: the trail, at joint 17.
    Trail,
}

impl CutterEffect {
    pub const COUNT: usize = 4;
    pub const ALL: [CutterEffect; CutterEffect::COUNT] = [
        CutterEffect::Draw,
        CutterEffect::Up,
        CutterEffect::Down,
        CutterEffect::Trail,
    ];

    /// `dobj->user_data.p`: the fighter joint the effect follows.
    pub const fn joint(self) -> u8 {
        match self {
            CutterEffect::Up | CutterEffect::Down => 0,
            CutterEffect::Draw | CutterEffect::Trail => CUTTER_EFFECT_JOINT,
        }
    }
}

/// `fp->joints[17]`, the Draw and Trail makers' `user_data`.
pub const CUTTER_EFFECT_JOINT: u8 = 17;

/// `ftKirbySpecialHiUpdateEffect`, first in all four Final Cutter
/// `proc_physics`. Flag 1 (1) and flag 2 (1) stop every attached effect
/// (`ftParamProcStopEffect`) once one is attached; flag 2's 2 to 5 make
/// Draw, Up, Down or Trail (`efManagerMakeEffectForce`), which sets
/// `is_effect_attach`. A made effect has played its first frame.
///
/// One slot per kind: a second Draw made while one lives (a `Landing`
/// from `SpecialHi`/`AirHi` before frame 3's stop) restarts it instead of
/// drawing two coincident blades.
fn update_cutter_effect(f: &mut Fighter) {
    if f.motion_script.flags[1] == 1 && f.is_effect_attach {
        stop_cutter_effects(f);
        f.motion_script.flags[1] = 0;
    }
    let kind = match f.motion_script.flags[2] {
        1 => {
            if f.is_effect_attach {
                stop_cutter_effects(f);
                f.motion_script.flags[2] = 0;
            }
            return;
        }
        2 => CutterEffect::Draw,
        3 => CutterEffect::Up,
        4 => CutterEffect::Down,
        5 => CutterEffect::Trail,
        _ => return,
    };
    f.kirby.cutter_effects[kind as usize] = Some(1);
    f.is_effect_attach = true;
    f.motion_script.flags[2] = 0;
}

/// `ftParamProcStopEffect`.
fn stop_cutter_effects(f: &mut Fighter) {
    f.kirby.cutter_effects = [None; CutterEffect::COUNT];
    crate::fteffect::stop_attached(f);
}

/// `kind`'s played animation frames, while it lives: only the Final
/// Cutter's own switches pass `FTSTATUS_PRESERVE_EFFECT`.
pub fn cutter_effect_ticks(f: &Fighter, kind: CutterEffect) -> Option<u16> {
    f.kirby.cutter_effects[kind as usize].filter(|_| {
        matches!(
            f.status.status,
            AnyStatus::Kirby(
                K::SpecialHi | K::SpecialAirHi | K::SpecialAirHiFall | K::SpecialHiLanding
            )
        )
    })
}

/// The Up, Down and Trail makers' root `rotate.y`: `lr * 90` degrees.
pub fn cutter_effect_rotate_y(f: &Fighter) -> f32 {
    f.facing.sign() * 90.0f32.to_radians()
}

/// `efManagerNoEjectProcUpdate`: each live cutter effect plays a frame
/// unless hitlag holds it (`ftParamProcPauseEffect`). [`Fighter`]'s update
/// calls this in hitlag too.
pub fn tick_cutter_effects(f: &mut Fighter) {
    if crate::fteffect::effects_held(f) {
        return;
    }
    for t in f.kirby.cutter_effects.iter_mut().flatten() {
        *t = t.saturating_add(1);
    }
}

/// `ftKirbySpecialHiLandingProcUpdate`'s flag 0: the wave, 200 ahead of
/// TopN.
fn make_cutter(f: &mut Fighter) {
    f.kirby.cutter_spawned = true;
    let lr = f.facing.sign();
    let position = f.joint_query(0, Vec3::ZERO) + Vec3::new(FINALCUTTER_OFF_X * lr, 0.0, 0.0);
    f.queue_weapon_spawn(WeaponSpawn {
        kind: WeaponKind::KirbyCutter {
            floor_line: f.floor.map(|s| s.line),
        },
        owner_port: f.port,
        team: f.team,
        stale: crate::stale::WeaponStale::of(f),
        position,
        facing: lr,
    });
}

/// `SetAirJumpMax`: airborne without a status change.
fn air_jump_max(f: &mut Fighter) {
    f.set_ga_air();
    f.floor = None;
    f.physics.vel_air.z = 0.0;
    f.pos.z = 0.0;
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
        // The setter plays the first frame (RE-473: Stone holds 5 frames
        // on).
        status::play_anim_events(f);
        f.kirby.stone_flag1 = false;
        f.kirby.stone_flag2 = false;
        // `ftkirbyspeciallw.c:306`: `vel_ground` is kept for the hold's
        // slide ([`keeps_vel_air`]).
        f.physics.vel_air.x = 0.0;
        f.physics.vel_air.y = 0.0;
    } else {
        // `ftKirbySpecialAirLwStartSetStatus` keeps `is_damage_resist`.
        let resist = f.kirby.is_damage_resist;
        set_keep_hitstatus(
            f,
            K::SpecialAirLwStart,
            0.0,
            StatusTiming::frames(SPECIAL_AIR_LW_START_LENGTH),
        );
        status::play_anim_events(f);
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
    f.kirby.stone_colanim = crate::colanim::ColAnimId::FIGHTER_KIRBY_SPECIAL_LW_HIGH;
    crate::colanim::check_set(
        f,
        crate::colanim::ColAnimId::FIGHTER_KIRBY_SPECIAL_LW_HIGH,
        0,
    );
}

/// `ftKirbySpecialLwUpdateColAnim`: Stone's flash follows its remaining
/// health.
fn stone_update_colanim(f: &mut Fighter) {
    use crate::colanim::ColAnimId;
    let id = if f.kirby.damage_resist < STONE_HEALTH_MID {
        if f.kirby.damage_resist < STONE_HEALTH_LOW {
            ColAnimId::FIGHTER_KIRBY_SPECIAL_LW_LOW
        } else {
            ColAnimId::FIGHTER_KIRBY_SPECIAL_LW_MID
        }
    } else {
        ColAnimId::FIGHTER_KIRBY_SPECIAL_LW_HIGH
    };
    if f.kirby.stone_colanim != id {
        crate::colanim::check_set(f, id, 0);
        f.kirby.stone_colanim = id;
    }
}

fn set_stone(f: &mut Fighter, s: K, frame: f32, preserve: status::Preserve) {
    status::set_any_status_preserve(
        f,
        AnyStatus::Kirby(s),
        frame,
        StatusTiming::frames(STONE_LENGTH),
        preserve,
    );
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
            set_keep_hitstatus(f, K::SpecialLwHold, 0.0, StatusTiming::frames(STONE_LENGTH));
        }
        set_damage_resist(f);
        f.kirby.stone_flag1 = false;
        f.kirby.stone_flag2 = true;
    }
    if f.kirby.stone_flag2 {
        if stone_check_release(f, false) {
            set_stone_end(f, grounded);
            f.kirby.stone_flag2 = false;
        }
        stone_update_colanim(f);
    }
}

/// `ftKirbySpecialLwHoldDecideNextStatus`.
fn stone_hold_decide(f: &mut Fighter, grounded: bool) {
    if stone_check_release(f, true) {
        set_stone_end(f, grounded);
    }
    stone_update_colanim(f);
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
    f.kirby.inhale_wind_flag0 = false;
    f.kirby.inhale_dist = Vec2::ZERO;
    f.kirby.victim_is_kirby = false;
    set_catch_params(f);
    // The setter plays the first frame (RE-473: the N64 loops 19 frames
    // on).
    status::play_anim_events(f);
}

/// `ftKirbySpecialNSetCatchParams`.
fn set_catch_params(f: &mut Fighter) {
    f.grab.is_catchstatus = true;
    f.grab.throw_desc = Some(crate::grab::KIRBY_INHALE);
}

/// The Inhale statuses' setters. `FTSTATUS_PRESERVE_LOOPSFX` keeps the
/// inhale's loop sound into the Loop statuses
/// (`ftKirbySpecialNLoopSetStatus`, the Start and Loop ground/air switches)
/// and into Catch (`ftKirbySpecialNCatchEatSetStatusParam`), but not into a
/// Catch switch or Eat.
fn set_inhale(f: &mut Fighter, s: K, frame: f32, timing: StatusTiming) {
    let loop_sfx = matches!(
        s,
        K::SpecialNStart
            | K::SpecialAirNStart
            | K::SpecialNLoop
            | K::SpecialAirNLoop
            | K::SpecialNCatch
            | K::SpecialAirNCatch
    );
    // `ftKirbySpecialNEatSetStatusParam` and
    // `ftKirbySpecialNCatchEatSetStatusParam` keep the slope contour
    // (`FTSTATUS_PRESERVE_SLOPECONTOUR`); their ground/air switches do not.
    let slope_contour = matches!(
        s,
        K::SpecialNCatch | K::SpecialAirNCatch | K::SpecialNEat | K::SpecialAirNEat
    );
    status::set_any_status_preserve(
        f,
        AnyStatus::Kirby(s),
        frame,
        timing,
        status::Preserve {
            loop_sfx,
            slope_contour,
            ..status::Preserve::NONE
        },
    );
    after_set_inhale(f, s);
}

/// A ground/air switch (`ftKirbySpecialN*SwitchStatus*`): only Start and
/// Loop keep the loop sound.
/// `FTKIRBY_SPECIALNSTART_STATUS_FLAGS` keeps the looping sound;
/// `FTKIRBY_SPECIALNLOOP_STATUS_FLAGS` keeps it and the box
/// (`FTSTATUS_PRESERVE_HIT`).
fn switch_inhale(f: &mut Fighter, s: K, frame: f32, timing: StatusTiming) {
    let looping = matches!(s, K::SpecialNLoop | K::SpecialAirNLoop);
    let preserve = status::Preserve {
        loop_sfx: looping || matches!(s, K::SpecialNStart | K::SpecialAirNStart),
        hit: looping,
        ..status::Preserve::NONE
    };
    set_inhale_preserve(f, s, frame, timing, preserve);
}

fn set_inhale_preserve(
    f: &mut Fighter,
    s: K,
    frame: f32,
    timing: StatusTiming,
    preserve: status::Preserve,
) {
    status::set_any_status_preserve(f, AnyStatus::Kirby(s), frame, timing, preserve);
    after_set_inhale(f, s);
}

fn after_set_inhale(f: &mut Fighter, s: K) {
    // `InhaleGround` and its aerial twin open with `SetFlag0(1)`.
    if matches!(s, K::SpecialNLoop | K::SpecialAirNLoop) {
        f.kirby.inhale_wind_flag0 = true;
    }
    f.grab.is_catchstatus = matches!(
        s,
        K::SpecialNStart | K::SpecialAirNStart | K::SpecialNLoop | K::SpecialAirNLoop
    );
    if !matches!(
        s,
        K::SpecialNStart | K::SpecialAirNStart | K::SpecialNLoop | K::SpecialAirNLoop
    ) {
        f.grab.capture_immune_mask = crate::grab::catch_kind::ALL;
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
    f.grab.is_catch_or_capture = false;
    f.grab.catch_kind = Some(held.kind);
    f.grab.capture_immune_mask = crate::grab::catch_kind::ALL;
    let lr = f.facing.sign();
    let point = Vec2::new(
        f.pos.x + crate::capture_kirby::CAPTURE_OFF.x * lr,
        f.pos.y + crate::capture_kirby::CAPTURE_OFF.y,
    );
    // The caught fighter's `ftCommonCaptureKirbyProcCapture` runs
    // `ftCommonCaptureKirbyProcPhysics` in the same hit phase, so the first
    // pull step lands on the catch frame (N64: 28 across, 36 up that frame).
    f.kirby.inhale_dist =
        crate::capture_kirby::decay_dist(Vec2::new(held.pos.x - point.x, held.pos.y - point.y));
    // `ftKirbySpecialNCatchProcUpdate` reads the copy on the eat; the
    // victim cannot change it while held.
    f.kirby.victim_is_kirby = is_kirby(held.kind);
    f.kirby.copy_pending = if f.kirby.victim_is_kirby {
        held.kirby.copy_id
    } else {
        copy_kind(COPY[held.kind as usize].0)
    };
    // `ftKirbySpecialNCatchEatSetStatusParam` plays a frame; the catch has
    // no figatree of its own, so the loop's plays on (RE-474).
    status::play_anim_events(f);
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
        // `ftKirbySpecialNThrowSetStatus` plays its first frame (RE-474).
        status::play_anim_events(f);
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
        // `ftKirbySpecialNCopySetStatus` plays its first frame (RE-474).
        status::play_anim_events(f);
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
            // `ftKirbySpecialNTurnSetStatus` plays its first frame (RE-474).
            status::play_anim_events(f);
        }
    }
}

/// `ftKirbySpecialNThrowUpdateCheckThrowStar`.
fn spit(f: &mut Fighter) {
    if f.grab.catch.take().is_none() {
        return;
    }
    f.grab.catch_kind = None;
    f.grab.capture_immune_mask = crate::grab::catch_kind::NONE;
    let lr = f.facing.sign();
    // `-victim.lr * 120`, and the victim faces Kirby.
    f.grab.send(GrabEvent::KirbyStar {
        copy: false,
        vel: Vec3::new(lr * VACUUM_THROW_VEL_BASE, 0.0, 0.0),
    });
}

/// `ftManagerInitFighter`'s Kirby case: a Kirby made holding
/// `copy_kind` (the 1P Game's Kirby Team) keeps it for life and wears its
/// hat; `nFTKindKirby` makes a bare Kirby that can lose a later copy.
/// `ftParamSetModelPartDefaultID` takes effect at the creating status's
/// `ftParamResetModelPartAll`, which [`ModelParts::reset_all`] stands for.
///
/// [`ModelParts::reset_all`]: crate::modelpart::ModelParts::reset_all
pub fn init_copy(f: &mut Fighter, copy_kind: FighterKind) {
    f.kirby.copy_id = copy_kind;
    f.kirby.copy = crate::kirby_copy::CopyState::default();
    f.kirby.is_ignore_losecopy = copy_kind != FighterKind::Kirby;
    if f.kind == FighterKind::Kirby {
        let part = crate::kirby_copy::copy_modelpart_id(copy_kind);
        f.model_parts
            .set_default(crate::kirby_copy::COPY_MODELPARTS_JOINT, part);
        f.model_parts.reset_all();
    }
}

/// `ftKirbySpecialNCopyInitCopyVars` then
/// `ftKirbySpecialNCopyUpdateCheckCopyStar`.
fn copy(f: &mut Fighter) {
    if f.kirby.copy_id == f.kirby.copy_pending {
        crate::sound::play_fgm(crate::sound::id::nSYAudioFGMKirbySpecialNCopyUnk);
    } else {
        crate::sound::play_fgm(crate::sound::id::nSYAudioFGMKirbySpecialNCopyThrow);
        f.kirby.copy_id = f.kirby.copy_pending;
        // `ftParamSetModelPartDefaultID(joint 6, copy_modelpart_id)` and
        // `ftParamResetModelPartAll` (RE-425).
        let part = crate::kirby_copy::copy_modelpart_id(f.kirby.copy_id);
        f.model_parts
            .set_default(crate::kirby_copy::COPY_MODELPARTS_JOINT, part);
        f.model_parts.reset_all();
        crate::kirby_copy::init_passive_vars(f);
    }
    if f.grab.catch.take().is_none() {
        return;
    }
    f.grab.catch_kind = None;
    f.grab.capture_immune_mask = crate::grab::catch_kind::NONE;
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

/// `ftCommonCaptureWaitKirbyUpdateBreakoutVars`'s writes to Kirby: `up`
/// lifts a grounded Kirby, `down` drops it through its floor line
/// (`ignore_line_id = floor_line_id`), `push_x` pushes it; then any wiggle
/// restarts its figatree at frame 0 (`lbCommonAddFighterPartsFigatree`).
/// The motion script runs on.
pub fn on_wiggle(f: &mut Fighter, up: bool, down: bool, push_x: Option<f32>) {
    if !matches!(
        f.status.status,
        AnyStatus::Kirby(K::SpecialNWait | K::SpecialAirNWait)
    ) {
        return;
    }
    if up && f.is_grounded() {
        switch_air(f);
        f.physics.vel_air.y = crate::capture_kirby::WIGGLE_VEL;
    } else if down && f.is_grounded() {
        let line = f.floor.map(|s| s.line);
        switch_air(f);
        f.ignore_line = line;
        f.physics.vel_air.y = -crate::capture_kirby::WIGGLE_VEL;
    }
    if let Some(x) = push_x {
        if f.is_grounded() {
            f.physics.vel_ground.x = x;
        } else {
            f.physics.vel_air.x = x;
        }
    }
    if up || down || push_x.is_some() {
        // A bare `gcAddDObjAnimJoint`: the next parse stands on frame 0.
        f.status.anim_frame = 0.0;
        if let status::ClipStart::Restart(clip) =
            crate::motion::clip_on_set_status(f.kind, f.status.status)
        {
            f.status.clip = clip;
        }
        f.status.clip_changed = true;
    }
}

// ---------------------------------------------------------------------------
// Callbacks
// ---------------------------------------------------------------------------

/// `proc_update` and `proc_interrupt` for Kirby's statuses.
/// `ftKirbySpecialNLoopProcInterrupt` / `ftKirbySpecialAirNLoopProcInterrupt`
/// with `ftKirbySpecialNLoopCheckContinueLoop`.
fn inhale_loop_interrupt(f: &mut Fighter, current: K) {
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
        // Unlike their ground/air switches, `ftKirbySpecialNEndSetStatus`
        // and `...AirNEndSetStatus` leave the mask `ftMainSetStatus` cleared.
        f.grab.capture_immune_mask = crate::grab::catch_kind::NONE;
        // They play the first frame (RE-473, RE-474).
        status::play_anim_events(f);
    }
}

pub fn update(f: &mut Fighter) {
    let AnyStatus::Kirby(current) = f.status.status else {
        return;
    };
    tick_cutter_effects(f);
    match current {
        K::Attack100Start => {
            if f.status.animation_ended() {
                set_attack100_loop(f);
            }
        }
        K::Attack100Loop => update_attack100_loop(f),
        K::Attack100End => {
            if f.status.animation_ended() {
                status::anim_end_set_wait(f);
            }
        }
        K::JumpAerialF1 | K::JumpAerialF2 | K::JumpAerialF3 | K::JumpAerialF4 | K::JumpAerialF5 => {
            update_jump_aerial_turn(f);
            if f.status.animation_ended() {
                status::anim_end_set_fall(f);
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
                // `ftKirbyThrowFFallSetStatus`: the held fighter can die
                // again; Kirby's own flag went with the status change.
                set(f, K::ThrowFFall, 0.0, StatusTiming::unknown());
                f.grab.send(crate::grab::GrabEvent::IgnoreDead(false));
            }
        }
        K::ThrowFFall => {}
        K::ThrowFLanding => {
            if crossed(f, THROWF_RELEASE_FRAME) && f.grab.catch.is_some() {
                crate::grab::release_thrown(f, -f.facing.sign());
            }
            if crossed(f, THROWF_AIR_JUMP_ADD_FRAME) {
                // `SetAirJumpAdd`.
                f.set_ga_air();
                f.floor = None;
                f.physics.vel_air.z = 0.0;
                f.pos.z = 0.0;
                f.physics.jumps_used += 1;
            }
            if f.status.animation_ended() {
                status::anim_end_set_wait_or_fall(f);
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
            if !f.kirby.cutter_spawned && f.status.clock >= CUTTER_WAVE_FRAME {
                make_cutter(f);
            }
            if f.status.animation_ended() {
                status::anim_end_set_wait(f);
            }
        }
        K::SpecialLwStart => {
            if crossed(f, STONE_START_FLAG1_FRAME) {
                set_keep_hitstatus(f, K::SpecialLwHold, 0.0, StatusTiming::frames(STONE_LENGTH));
                set_damage_resist(f);
                f.kirby.stone_flag1 = false;
            } else if f.status.animation_ended() {
                set_stone(f, K::SpecialLwUnk, 0.0, status::Preserve::NONE);
            }
        }
        K::SpecialAirLwStart => {
            if crossed(f, STONE_AIR_START_FLAG1_FRAME) {
                set_damage_resist(f);
                f.kirby.stone_flag1 = false;
            }
            if f.status.animation_ended() {
                // `ftKirbySpecialAirLwHoldSetStatus` plays the first frame
                // (RE-473).
                set_stone(f, K::SpecialAirLwHold, 0.0, status::Preserve::NONE);
                status::play_anim_events(f);
                f.physics.vel_air.y = STONE_FALL_VEL;
            }
        }
        K::SpecialLwUnk => stone_unk_decide(f, true),
        K::SpecialAirLwHold => stone_unk_decide(f, false),
        K::SpecialLwHold | K::SpecialAirLwLanding => stone_hold_decide(f, true),
        K::SpecialAirLwFall => stone_hold_decide(f, false),
        K::SpecialLwEnd | K::SpecialAirLwEnd => {
            if f.status.animation_ended() {
                status::anim_end_set_fall(f);
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
                // `ftKirbySpecialNLoopSetStatus` plays the first frame
                // (RE-473).
                status::play_anim_events(f);
                // `ftMainProcUpdateInterrupt` runs `proc_interrupt` after
                // `proc_update`, so the new Loop's interrupt already counts
                // this frame (RE-511: the N64 inhale ends a frame earlier).
                inhale_loop_interrupt(f, s);
            }
        }
        K::SpecialNLoop | K::SpecialAirNLoop => {
            // `ftKirbySpecialNLoopProcUpdate`: the wind, once per inhale.
            let queued = f
                .effects
                .iter()
                .any(|e| matches!(e, crate::fteffect::FighterEffect::KirbyInhaleWind));
            if !f.is_effect_attach && f.kirby.inhale_wind_flag0 && !queued {
                f.effects
                    .push(crate::fteffect::FighterEffect::KirbyInhaleWind);
            }
            inhale_loop_interrupt(f, current);
        }
        K::SpecialNEnd => {
            if f.status.animation_ended() {
                status::anim_end_set_wait(f);
            }
        }
        K::SpecialAirNEnd => {
            if f.status.animation_ended() {
                status::anim_end_set_fall(f);
            }
        }
        K::SpecialNCatch | K::SpecialAirNCatch => {
            let d = f.kirby.inhale_dist;
            if d.x * d.x + d.y * d.y < VACUUM_SPECIALNWAIT_DIST_MIN {
                f.grab.send(GrabEvent::KirbyEat {
                    is_kirby: f.kirby.victim_is_kirby,
                });
                crate::sound::play_fgm(crate::sound::id::nSYAudioFGMKirbySpecialNCopyEat);
                let s = if current == K::SpecialNCatch {
                    K::SpecialNEat
                } else {
                    K::SpecialAirNEat
                };
                set_inhale(f, s, 0.0, StatusTiming::frames(SPECIAL_N_EAT_LENGTH));
                // `ftKirbySpecialNEatSetStatusParam` plays its first frame
                // (RE-474).
                status::play_anim_events(f);
            } else {
                if d.x * d.x + d.y * d.y < VACUUM_STOPGFX_DIST_MIN {
                    // `ftKirbySpecialNStopEffect`.
                    crate::fteffect::stop_attached(f);
                }
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
                // `ftKirbySpecialNWaitSetStatusFromEat` plays its first
                // frame; the wait from a turn does not (RE-474).
                status::play_anim_events(f);
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
                // `ftKirbySpecialNWaitSetStatusFromTurn` leaves the mask
                // `ftMainSetStatus` cleared; the wait from the eat sets it.
                f.grab.capture_immune_mask = crate::grab::catch_kind::NONE;
            }
        }
        K::SpecialNThrow | K::SpecialAirNThrow => {
            if crossed(f, INHALE_SPIT_FRAME) {
                spit(f);
            }
            if f.status.animation_ended() {
                if current == K::SpecialNThrow {
                    status::anim_end_set_wait(f);
                } else {
                    status::anim_end_set_fall(f);
                }
            }
        }
        K::SpecialNCopy | K::SpecialAirNCopy => {
            if crossed(f, INHALE_COPY_FRAME) {
                copy(f);
            }
            if f.status.animation_ended() {
                if current == K::SpecialNCopy {
                    status::anim_end_set_wait(f);
                } else {
                    status::anim_end_set_fall(f);
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
            update_cutter_effect(f);
            // Position follows `vel_air` in the source; X is the drift.
            apply_air_vel_transn_yz(f, 1.0);
            cutter_drift(f);
            f.physics.vel_ground.x = f.physics.vel_air.x;
        }
        K::SpecialHiLanding | K::SpecialNThrow => {
            if current == K::SpecialHiLanding {
                update_cutter_effect(f);
            }
            physics::apply_ground_vel_transn(
                &mut f.physics,
                f.root_motion,
                f.topn_lr,
                f.attributes.size,
            );
        }
        K::SpecialLwHold | K::SpecialAirLwLanding => {
            // `ftKirbySpecialLwHoldProcPhysics`: slide down slopes.
            let (normal, flags) = f
                .floor
                .map(|s| (s.normal, s.flags))
                .unwrap_or((Vec2::new(0.0, 1.0), 0));
            // `ftKirbySpecialLwGetGroundAxisYaw`: the floor's roll, which
            // it also gives TopN (`rotate.z`).
            let roll = -sy_arc_tan2(normal.x, normal.y);
            f.topn_rotate_z = roll;
            let angle = roll.clamp(-STONE_SLIDE_ANGLE, STONE_SLIDE_ANGLE);
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
            physics::apply_ground_friction(&mut f.physics, friction, lr);
        }
        _ => return crate::kirby_copy::apply_ground_physics(f),
    }
    true
}

/// Grounded Stone's start has no `proc_physics` (`ftkirbystatus.h`): no
/// `ftPhysicsSetGroundVelTransferAir`, so TopN steps by the `vel_air` the
/// setter zeroed (or the landing kept), and `vel_ground` waits untouched
/// for `ftKirbySpecialLwHoldProcPhysics`.
pub fn keeps_vel_air(f: &Fighter) -> bool {
    f.status.status == AnyStatus::Kirby(K::SpecialLwStart)
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
            physics::apply_air_vel_transn_all(
                &mut f.physics,
                f.root_motion,
                f.facing.sign(),
                f.attributes.size,
            );
        }
        // `proc_physics` is NULL: the velocity carries.
        K::ThrowFFall | K::SpecialAirLwStart | K::SpecialAirLwHold | K::SpecialAirLwFall => {}
        K::SpecialHi => {
            update_cutter_effect(f);
            apply_air_vel_transn_yz(f, 1.0);
            cutter_drift(f);
        }
        K::SpecialAirHi => {
            update_cutter_effect(f);
            apply_air_vel_transn_yz(f, FINALCUTTER_AIR_TRANSN_SCALE);
            cutter_drift(f);
        }
        K::SpecialAirHiFall => {
            update_cutter_effect(f);
            cutter_drift(f);
        }
        K::SpecialHiLanding => {
            update_cutter_effect(f);
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
    switch_inhale(f, air, frame, timing);
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
    switch_inhale(f, ground, frame, timing);

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
            set_keep_hitstatus(
                f,
                K::SpecialAirLwStart,
                frame,
                StatusTiming::frames(SPECIAL_AIR_LW_START_LENGTH),
            );
            f.physics.vel_air.y = STONE_FALL_VEL;
        }
        K::SpecialLwUnk => {
            let frame = f.status.anim_frame;
            f.become_airborne();
            set_stone(f, K::SpecialAirLwHold, frame, status::Preserve::HITSTATUS);
        }
        K::SpecialLwHold | K::SpecialAirLwLanding => {
            f.become_airborne();
            set_keep_hitstatus(f, K::SpecialAirLwFall, 0.0, StatusTiming::unknown());
            f.kirby.is_damage_resist = true;
            f.physics.vel_air.y = STONE_FALL_VEL;
        }
        // `ftCommonCatchProcMap`: the held fighter drops.
        K::ThrowFLanding => {
            f.become_airborne();
            crate::grab::release_on_edge(f);
        }
        // `ftKirbySpecialHiProcMap`: `SetAirJumpMax` has not run yet; only
        // `ga` turns (`ftkirbyspecialhi.c:207`), the jump count stays.
        K::SpecialHi => f.set_ga_air(),
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
        // `mpCommonProcFighterCliffWaitOrLanding` (`ftkirbystatus.h`).
        K::JumpAerialF1 | K::JumpAerialF2 | K::JumpAerialF3 | K::JumpAerialF4 | K::JumpAerialF5 => {
            wait_or_landing(f, y);
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
            set_special_hi_landing_from_fall(f);
        }
        K::SpecialHiLanding => f.land(y),
        K::SpecialAirLwStart => {
            let frame = f.status.anim_frame;
            f.land(y);
            set_frames(f, K::SpecialLwStart, frame, SPECIAL_LW_START_LENGTH);
        }
        K::SpecialAirLwHold | K::SpecialAirLwFall => {
            f.land(y);
            set_keep_hitstatus(
                f,
                K::SpecialAirLwLanding,
                0.0,
                StatusTiming::frames(STONE_LENGTH),
            );
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

    /// `ftKirbySpecialHiProcMap` (`ftkirbyspecialhi.c:207`): a grounded
    /// Final Cutter leaving the edge before `SetAirJumpMax` turns only `ga`;
    /// it does not spend the ground jump (`mpCommonSetFighterAir` would).
    #[test]
    fn a_final_cutter_leaving_the_edge_keeps_its_jump_count() {
        let mut f = Fighter::new(FighterKind::Kirby, 0, 3);
        f.situation = Situation::Ground;
        f.status.status = AnyStatus::Kirby(K::SpecialHi);
        f.physics.jumps_used = 0;
        assert!(on_ground_lost(&mut f));
        assert_eq!(f.situation, Situation::Air);
        assert_eq!(f.status.status, AnyStatus::Kirby(K::SpecialHi));
        assert_eq!(f.physics.jumps_used, 0);
    }

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

    fn stopped(f: &Fighter) -> bool {
        f.effects
            .iter()
            .any(|e| matches!(e, crate::fteffect::FighterEffect::StopAttached))
    }

    /// Every Final Cutter setter sets `proc_lagstart`
    /// (`ftkirbyspecialhi.c:300-420`). `SpecialHi`/`AirHi` to `Landing` and
    /// to `AirHiFall` keep the effect (`FTSTATUS_PRESERVE_EFFECT`);
    /// `ftKirbySpecialAirHiFallProcMap`'s landing passes
    /// `FTSTATUS_PRESERVE_NONE` and stops it.
    #[test]
    fn final_cutter_setters_hold_effects_and_only_the_fall_landing_stops_them() {
        for ground in [true, false] {
            let mut f = kirby();
            if !ground {
                f.become_airborne();
            }
            f.motion_script.flags[..3].fill(7);
            set_special_hi(&mut f);
            assert!(f.lag_pauses_effects, "ground {ground}");
            assert_eq!(f.motion_script.flags[..3], [0, 0, 0]);

            f.is_effect_attach = true;
            f.effects.clear();
            f.physics.vel_air.y = -1.0;
            if ground {
                f.become_airborne();
            }
            f.situation = Situation::Air;
            assert!(on_landing(&mut f, 0.0));
            assert_eq!(f.status.status, AnyStatus::Kirby(K::SpecialHiLanding));
            assert!(f.lag_pauses_effects && f.is_effect_attach && !stopped(&f));
        }

        let mut f = kirby();
        f.become_airborne();
        set_special_air_hi_fall(&mut f);
        assert!(f.lag_pauses_effects);
        f.is_effect_attach = true;
        f.effects.clear();
        assert!(on_landing(&mut f, 0.0));
        assert_eq!(f.status.status, AnyStatus::Kirby(K::SpecialHiLanding));
        assert!(f.lag_pauses_effects, "the landing sets it again");
        assert!(stopped(&f) && !f.is_effect_attach);

        for (from, to, kept) in [
            (K::SpecialHi, K::SpecialHiLanding, true),
            (K::SpecialAirHi, K::SpecialHiLanding, true),
            (K::SpecialHi, K::SpecialAirHiFall, true),
            (K::SpecialAirHi, K::SpecialAirHiFall, true),
            (K::SpecialAirHiFall, K::SpecialHiLanding, false),
        ] {
            assert_eq!(
                crate::colanim::preserved_in(
                    &crate::colanim::PRESERVE_EFFECT,
                    FighterKind::Kirby,
                    AnyStatus::Kirby(from),
                    AnyStatus::Kirby(to),
                ),
                kept,
                "{from:?} -> {to:?}"
            );
        }
    }

    /// Each frame's made cutter effects (played once) as
    /// `(status, anim_frame, kind)`, and the kinds live after the frame.
    fn cutter_frame(
        f: &Fighter,
        log: &mut std::vec::Vec<(K, f32, CutterEffect)>,
    ) -> std::vec::Vec<CutterEffect> {
        let AnyStatus::Kirby(s) = f.status.status else {
            unreachable!()
        };
        let mut live = std::vec::Vec::new();
        for kind in CutterEffect::ALL {
            match cutter_effect_ticks(f, kind) {
                Some(1) => {
                    log.push((s, f.status.anim_frame, kind));
                    live.push(kind);
                }
                Some(_) => live.push(kind),
                None => {}
            }
        }
        live
    }

    /// `dKirbyMainMotion_0x1978`, the Final Cutter's parallel script, sets
    /// flag 2 to Draw at frame 2, and flag 1 (stop) with Up at 21, Draw at
    /// 24, Down at 46, Draw at 50, Trail at 52 and Draw at 58;
    /// `ftKirbySpecialHiUpdateEffect` makes each on that frame's
    /// `proc_physics`. One lives at a time; `AirHiFall` keeps the last
    /// Draw; the fall's landing stops it, `FinalCutterLand`'s flag 2 makes a
    /// Draw and its flag 1 at frame 3 stops it.
    #[test]
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
    fn final_cutter_effects_follow_the_parallel_script() {
        let mut f = kirby();
        f.situation = Situation::Ground;
        f.effects.clear();
        set_special_hi(&mut f);
        let mut log = std::vec::Vec::new();
        let mut n = 0;
        while f.status.status != AnyStatus::Kirby(K::SpecialAirHiFall) {
            status::update(&mut f);
            if f.is_grounded() {
                apply_ground_physics(&mut f);
            } else {
                apply_air_physics(&mut f);
            }
            let live = cutter_frame(&f, &mut log);
            assert!(live.len() <= 1, "frame {}: {live:?}", f.status.anim_frame);
            n += 1;
            assert!(n < 80);
        }
        use CutterEffect as C;
        assert_eq!(
            log,
            [
                (K::SpecialHi, 2.0, C::Draw),
                (K::SpecialHi, 21.0, C::Up),
                (K::SpecialHi, 24.0, C::Draw),
                (K::SpecialHi, 46.0, C::Down),
                (K::SpecialHi, 50.0, C::Draw),
                (K::SpecialHi, 52.0, C::Trail),
                (K::SpecialHi, 58.0, C::Draw),
            ]
        );
        // The fall keeps the Draw and plays it on.
        let draw = cutter_effect_ticks(&f, C::Draw).expect("kept");
        apply_air_physics(&mut f);
        update(&mut f);
        assert_eq!(cutter_effect_ticks(&f, C::Draw), Some(draw + 1));

        assert!(f.is_effect_attach);
        f.effects.clear();
        assert!(on_landing(&mut f, 0.0));
        assert!(stopped(&f) && !f.is_effect_attach);
        assert_eq!(cutter_effect_ticks(&f, C::Draw), None);
        log.clear();
        let mut stop = None;
        while f.status.status == AnyStatus::Kirby(K::SpecialHiLanding) {
            f.effects.clear();
            status::update(&mut f);
            apply_ground_physics(&mut f);
            if f.status.status != AnyStatus::Kirby(K::SpecialHiLanding) {
                break;
            }
            let live = cutter_frame(&f, &mut log);
            if !log.is_empty() && live.is_empty() && stop.is_none() {
                assert!(stopped(&f));
                stop = Some(f.status.anim_frame);
            }
        }
        // No first-frame play: the Draw comes with the first update.
        assert_eq!(log, [(K::SpecialHiLanding, 1.0, C::Draw)]);
        assert_eq!(stop, Some(3.0));
    }

    /// `ftParamProcPauseEffect` holds the cutter effects through hitlag;
    /// [`Fighter`]'s hitlag update ticks them otherwise.
    #[test]
    fn cutter_effects_hold_in_hitlag_only_while_lagstart_is_set() {
        let mut f = kirby();
        f.situation = Situation::Ground;
        set_special_hi(&mut f);
        f.motion_script.flags[2] = 3;
        apply_ground_physics(&mut f);
        assert_eq!(cutter_effect_ticks(&f, CutterEffect::Up), Some(1));
        assert!(f.is_effect_attach);
        let lives = crate::effect_order::fighter_lives(&f);
        assert_eq!(
            lives[crate::effect_order::Kind::CutterUp as usize],
            (true, Some(1))
        );
        assert_eq!(
            lives.iter().filter(|(live, _)| *live).count(),
            1,
            "only the Up"
        );
        f.hitlag = 2;
        tick_cutter_effects(&mut f);
        assert_eq!(cutter_effect_ticks(&f, CutterEffect::Up), Some(1));
        f.lag_pauses_effects = false;
        tick_cutter_effects(&mut f);
        assert_eq!(cutter_effect_ticks(&f, CutterEffect::Up), Some(2));
        // Flag 2's 1 stops it.
        f.hitlag = 0;
        f.motion_script.flags[2] = 1;
        apply_ground_physics(&mut f);
        assert_eq!(cutter_effect_ticks(&f, CutterEffect::Up), None);
        assert!(!f.is_effect_attach);
        assert_eq!(f.motion_script.flags[2], 0);
    }

    /// `ftCommonCaptureWaitKirbyProcInterrupt`'s down wiggle
    /// (`ftcommoncapturekirby.c:154`): a grounded Kirby drops through its
    /// floor line at -20, and the figatree restarts.
    #[test]
    fn a_down_wiggle_drops_grounded_kirby_through_its_floor() {
        let mut f = kirby();
        set(&mut f, K::SpecialNWait, 12.0, StatusTiming::unknown());
        f.floor = Some(crate::ground::Standing {
            line: 9,
            flags: crate::collision::flags::PASS,
            normal: Vec2::new(0.0, 1.0),
        });
        on_wiggle(&mut f, false, true, None);
        assert_eq!(f.status.status, AnyStatus::Kirby(K::SpecialAirNWait));
        assert_eq!(f.ignore_line, Some(9));
        assert_eq!(f.physics.vel_air.y, -crate::capture_kirby::WIGGLE_VEL);
        assert_eq!(f.status.anim_frame, 0.0);

        // In the air the down wiggle only restarts the figatree.
        f.status.anim_frame = 5.0;
        f.physics.vel_air.y = -3.0;
        on_wiggle(&mut f, false, true, None);
        assert_eq!(f.physics.vel_air.y, -3.0);
        assert_eq!(f.status.anim_frame, 0.0);
    }

    /// `ftParamProcPauseEffect` setters (RE-549 audit): Final Cutter's
    /// setters install it; the inhale (`ftkirbyspecialn.c:163` attaches
    /// effects without it) does not, and a status set clears it.
    #[test]
    fn final_cutter_pauses_its_effects_in_hitlag_and_the_inhale_does_not() {
        let mut f = kirby();
        set_special_hi(&mut f);
        assert!(f.lag_pauses_effects, "SpecialHi");
        set_special_n(&mut f);
        assert!(!f.lag_pauses_effects, "SpecialNStart");
        f.become_airborne();
        set_special_hi(&mut f);
        assert_eq!(f.status.status, AnyStatus::Kirby(K::SpecialAirHi));
        assert!(f.lag_pauses_effects, "SpecialAirHi");
    }

    /// `AOBJ_ANIM_CHANGED` from a wiggle: the next play stands on frame 0
    /// and counts one restart for the drawn clip; the play after advances.
    #[test]
    fn a_wiggle_restart_is_counted_by_the_play_that_poses_frame_zero() {
        let mut f = kirby();
        set(&mut f, K::SpecialNWait, 12.0, StatusTiming::unknown());
        let entry = f.status.entry;
        let restarts = f.status.clip_restarts;
        on_wiggle(&mut f, false, false, Some(8.0));
        assert_eq!(
            f.status.clip_restarts, restarts,
            "the wiggle frame draws the clip run on"
        );
        status::play_anim(&mut f);
        assert_eq!(f.status.anim_frame, 0.0);
        assert_eq!(f.status.clip_restarts, restarts.wrapping_add(1));
        assert_eq!(f.status.entry, entry, "no status set");
        status::play_anim(&mut f);
        assert_eq!(f.status.anim_frame, 1.0);
        assert_eq!(f.status.clip_restarts, restarts.wrapping_add(1));
        // A frame without a wiggle counts nothing.
        on_wiggle(&mut f, false, false, None);
        status::play_anim(&mut f);
        assert_eq!(f.status.clip_restarts, restarts.wrapping_add(1));
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
        f.status.set_time(27.0);
        assert!(!check_jump_aerial(&mut f));
        f.status.set_time(28.0);
        assert!(check_jump_aerial(&mut f));
        assert_eq!(f.status.status, AnyStatus::Kirby(K::JumpAerialF3));
    }

    #[test]
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
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

    /// `FTKirbyCopy::effect_scale`: every kind has one, and each polygon
    /// and Giant Donkey Kong repeat their base fighter's (12 + 1 redundant
    /// copies agree).
    #[test]
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
    fn the_captured_stars_scale_is_positive_and_polygons_copy_their_base() {
        assert!(EFFECT_SCALE.iter().all(|&s| s > 0.0 && s <= 4.0));
        for i in 0..12 {
            assert_eq!(EFFECT_SCALE[14 + i], EFFECT_SCALE[i], "kind {i}");
        }
        assert_eq!(
            effect_scale(FighterKind::GiantDonkey),
            effect_scale(FighterKind::Donkey)
        );
        assert_eq!(effect_scale(FighterKind::Mario), EFFECT_SCALE[0]);
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
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
    fn a_kirby_made_with_a_copy_keeps_it_and_its_hat() {
        let mut f = kirby();
        f.kirby.copy.samus_charge_level = 3;
        init_copy(&mut f, FighterKind::Fox);
        assert_eq!(f.kirby.copy_id, FighterKind::Fox);
        assert!(f.kirby.is_ignore_losecopy);
        assert_eq!(f.kirby.copy.samus_charge_level, 0);
        assert_eq!(
            crate::kirby_copy::copy_hat(&f),
            Some(crate::kirby_copy::COPY_MODELPART_IDS[FighterKind::Fox as usize])
        );
        init_copy(&mut f, FighterKind::Kirby);
        assert!(!f.kirby.is_ignore_losecopy);
        assert_eq!(crate::kirby_copy::copy_hat(&f), None);
    }

    #[test]
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
    fn stone_soaks_its_health_then_passes_the_overflow() {
        let mut f = kirby();
        f.situation = Situation::Ground;
        set_special_lw(&mut f);
        f.status.set_time(6.0);
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

    /// `FTKIRBY_SPECIALNLOOP_STATUS_FLAGS` carries `FTSTATUS_PRESERVE_HIT`:
    /// the Inhale loop's map switches keep its catch box.
    #[test]
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
    fn the_inhale_loop_keeps_its_box_through_its_switches() {
        use crate::combat::AttackState;
        let active = |f: &Fighter| f.attack_colls.iter().any(|c| c.state != AttackState::Off);
        let mut f = kirby();
        f.situation = Situation::Ground;
        set_special_n(&mut f);
        let mut n = 0;
        while !(active(&f) && f.status.status == AnyStatus::Kirby(K::SpecialNLoop)) {
            press(&mut f, N64Buttons::B, 0, 0);
            crate::status::update(&mut f);
            n += 1;
            assert!(n < 60, "the loop makes a box");
        }
        assert!(on_ground_lost(&mut f));
        assert_eq!(f.status.status, AnyStatus::Kirby(K::SpecialAirNLoop));
        assert!(active(&f), "to the air");
        assert!(on_landing(&mut f, 0.0));
        assert_eq!(f.status.status, AnyStatus::Kirby(K::SpecialNLoop));
        assert!(active(&f), "to the ground");
    }

    /// Stone's aerial setter and every one of its switches pass
    /// `FTSTATUS_PRESERVE_HITSTATUS` (`ftkirbyspeciallw.c`): an invincible
    /// Kirby stays invincible into the stone, through the fall, the landing
    /// and a fall from the landing (the shared `...LwHoldProcMap`s, which
    /// also keep the stone's flash). The grounded setter drops it.
    #[test]
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
    fn stone_keeps_the_hit_status_through_its_switches() {
        use crate::combat::HitStatus;
        let mut f = kirby();
        f.situation = Situation::Ground;
        crate::hurtbox::set_hit_status_all(&mut f, HitStatus::Invincible);
        set_special_lw(&mut f);
        assert_eq!(f.hitstatus, HitStatus::Normal);
        crate::hurtbox::set_hit_status_all(&mut f, HitStatus::Invincible);
        f.status.set_time(6.0);
        update(&mut f);
        assert_eq!(f.status.status, AnyStatus::Kirby(K::SpecialLwHold));
        assert_eq!(f.hitstatus, HitStatus::Invincible);
        let colanim = f.colanim.id;
        for (lose_floor, next) in [
            (true, K::SpecialAirLwFall),
            (false, K::SpecialAirLwLanding),
            (true, K::SpecialAirLwFall),
            (false, K::SpecialAirLwLanding),
        ] {
            if lose_floor {
                assert!(on_ground_lost(&mut f));
            } else {
                assert!(on_landing(&mut f, 0.0));
            }
            assert_eq!(f.status.status, AnyStatus::Kirby(next));
            assert_eq!(f.hitstatus, HitStatus::Invincible, "{next:?}");
            assert_eq!(f.colanim.id, colanim, "{next:?}");
        }
        let mut f = kirby();
        f.situation = Situation::Air;
        crate::hurtbox::set_hit_status_all(&mut f, HitStatus::Invincible);
        set_special_lw(&mut f);
        assert_eq!(f.status.status, AnyStatus::Kirby(K::SpecialAirLwStart));
        assert_eq!(f.hitstatus, HitStatus::Invincible);
    }

    #[test]
    fn grounded_stone_releases_on_b_or_after_its_duration() {
        let mut f = kirby();
        f.situation = Situation::Ground;
        set_special_lw(&mut f);
        f.status.set_time(6.0);
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
        f.status.set_time(SPECIAL_N_START_LENGTH);
        update(&mut f);
        assert_eq!(f.status.status, AnyStatus::Kirby(K::SpecialNLoop));
        assert!(inhale_searching(&f));
        // The Loop's interrupt already counted the Start -> Loop frame
        // (RE-511).
        for _ in 0..VACUUM_RELEASE_LAG - 1 {
            update(&mut f);
        }
        assert_eq!(f.status.status, AnyStatus::Kirby(K::SpecialNLoop));
        update(&mut f);
        assert_eq!(f.status.status, AnyStatus::Kirby(K::SpecialNEnd));
        assert!(!f.grab.is_catchstatus);
    }

    #[test]
    fn inhale_asks_for_its_wind_on_the_first_loop_update_until_attached() {
        use crate::fteffect::FighterEffect;
        let wind = |f: &Fighter| {
            f.effects
                .iter()
                .filter(|e| matches!(e, FighterEffect::KirbyInhaleWind))
                .count()
        };
        let mut f = kirby();
        f.situation = Situation::Ground;
        set_special_n(&mut f);
        assert!(!f.kirby.inhale_wind_flag0);
        f.status.set_time(SPECIAL_N_START_LENGTH);
        // The Start's update sets the Loop; its own update comes next frame.
        update(&mut f);
        assert_eq!(f.status.status, AnyStatus::Kirby(K::SpecialNLoop));
        assert_eq!(wind(&f), 0);
        update(&mut f);
        assert_eq!(wind(&f), 1);
        // Not yet made: one request stands, not one per frame.
        update(&mut f);
        assert_eq!(wind(&f), 1);
        // Made (the flush): no more.
        f.effects.clear();
        f.kirby.inhale_wind_flag0 = false;
        f.is_effect_attach = true;
        update(&mut f);
        assert_eq!(wind(&f), 0);
    }

    #[test]
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
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
        // 240 across at 28 per frame: nine pulls, the first on the catch
        // frame (the victim's `ftCommonCaptureKirbyProcCapture` runs its
        // `ProcPhysics`), then the eat.
        assert_eq!(frames, 8);
        assert_eq!(f.status.status, AnyStatus::Kirby(K::SpecialNEat));
        assert!(f
            .grab
            .outbox
            .iter()
            .any(|e| matches!(e, Some(GrabEvent::KirbyEat { is_kirby: false }))));
    }

    /// `ftKirbySpecialNCatchProcUpdate` (`ftkirbyspecialn.c:202`): inside
    /// `FTKIRBY_VACUUM_STOPGFX_DIST_MIN` (96 units) but short of the eat,
    /// `ftKirbySpecialNStopEffect` ejects the attached inhale wind.
    #[test]
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
    fn the_inhale_wind_stops_once_the_victim_is_within_96_units() {
        let stops = |f: &Fighter| {
            f.effects
                .iter()
                .filter(|e| matches!(e, crate::fteffect::FighterEffect::StopAttached))
                .count()
        };
        for (dist, stopped) in [
            (Vec2::new(60.0, 80.0), false),
            (Vec2::new(60.0, 70.0), true),
        ] {
            let mut f = kirby();
            f.situation = Situation::Ground;
            f.grab.catch = Some(1);
            f.kirby.copy_pending = FighterKind::Mario;
            set_inhale(&mut f, K::SpecialNCatch, 0.0, StatusTiming::unknown());
            f.effects.clear();
            f.is_effect_attach = true;
            f.kirby.inhale_dist = dist;
            update(&mut f);
            assert_eq!(
                f.status.status,
                AnyStatus::Kirby(K::SpecialNCatch),
                "{dist:?}"
            );
            assert_eq!(stops(&f), usize::from(stopped), "{dist:?}");
            assert_eq!(f.is_effect_attach, !stopped, "{dist:?}");
        }
    }

    /// RE-474's N64 Training trace of the swallow: each of these setters
    /// plays its first frame (`anim_frame` 1), and the eat, turn, copy and
    /// spit last 19, 11, 29 and 27 frames; the wait after a turn does not
    /// play.
    #[test]
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
    fn the_swallows_setters_play_their_first_frame_as_the_n64s() {
        fn frames_in(f: &mut Fighter) -> u32 {
            let start = f.status.status;
            let mut n = 1;
            loop {
                press(f, 0, 0, 0);
                status::update(f);
                if f.status.status != start {
                    return n;
                }
                n += 1;
                assert!(n < 200, "{start:?} never ends");
            }
        }
        let held = |f: &mut Fighter| {
            f.situation = Situation::Ground;
            f.grab.catch = Some(1);
            f.kirby.copy_pending = FighterKind::Mario;
        };
        let mut f = kirby();
        held(&mut f);
        set_inhale(&mut f, K::SpecialNCatch, 0.0, StatusTiming::unknown());
        f.kirby.inhale_dist = Vec2::new(0.0, 0.0);
        press(&mut f, 0, 0, 0);
        status::update(&mut f);
        assert_eq!(f.status.status, AnyStatus::Kirby(K::SpecialNEat));
        assert_eq!(f.status.anim_frame, 1.0);
        assert_eq!(frames_in(&mut f), 19);
        assert_eq!(f.status.status, AnyStatus::Kirby(K::SpecialNWait));
        assert_eq!(f.status.anim_frame, 1.0, "the wait from the eat plays");
        // A turn: 11 frames, then the wait without a play.
        press(&mut f, 0, -90, 0);
        status::update(&mut f);
        assert_eq!(f.status.status, AnyStatus::Kirby(K::SpecialNTurn));
        assert_eq!(f.status.anim_frame, 1.0);
        assert_eq!(frames_in(&mut f), 11);
        assert_eq!(f.status.status, AnyStatus::Kirby(K::SpecialNWait));
        assert_eq!(f.status.anim_frame, 0.0, "the wait from the turn");
        for (buttons, y, status, frames) in [
            (N64Buttons::A, 0, K::SpecialNThrow, 27),
            (0, -90, K::SpecialNCopy, 29),
        ] {
            let mut f = kirby();
            held(&mut f);
            set_inhale(&mut f, K::SpecialNWait, 0.0, StatusTiming::unknown());
            press(&mut f, buttons, 0, y);
            status::update(&mut f);
            assert_eq!(f.status.status, AnyStatus::Kirby(status));
            assert_eq!(f.status.anim_frame, 1.0, "{status:?}");
            assert_eq!(frames_in(&mut f), frames, "{status:?}");
        }
    }

    /// RE-474: the aerial Inhale's end plays its first frame as the
    /// grounded one does (an N64 Training trace off the ledge).
    #[test]
    fn the_aerial_inhale_end_plays_its_first_frame() {
        let mut f = kirby();
        f.physics.jumps_used = 1;
        status::set_fall(&mut f);
        set_inhale(&mut f, K::SpecialAirNLoop, 0.0, StatusTiming::unknown());
        f.kirby.release_lag = 0;
        press(&mut f, 0, 0, 0);
        update(&mut f);
        assert_eq!(f.status.status, AnyStatus::Kirby(K::SpecialAirNEnd));
        assert_eq!(f.status.anim_frame, 1.0);
    }

    #[test]
    fn copy_takes_the_ability_and_sends_the_star_backward() {
        let mut f = kirby();
        f.situation = Situation::Ground;
        f.grab.catch = Some(1);
        f.kirby.copy_pending = FighterKind::Captain;
        set_inhale(&mut f, K::SpecialNCopy, 0.0, StatusTiming::frames(30.0));
        f.status.set_time(INHALE_COPY_FRAME);
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
        f.status.set_time(22.0);
        update(&mut f);
        assert!(f.is_grounded());
        f.status.set_time(23.0);
        update(&mut f);
        assert!(!f.is_grounded());
        assert_eq!(f.physics.jumps_used, 6);
        f.physics.vel_air.y = -10.0;
        assert!(on_landing(&mut f, 0.0));
        assert_eq!(f.status.status, AnyStatus::Kirby(K::SpecialHiLanding));
        // The map pass that landed it set `floor_line_id`.
        f.floor = Some(crate::ground::Standing {
            line: 3,
            flags: 0,
            normal: ssb_engine::math::Vec2::new(0.0, 1.0),
        });
        f.status.set_time(2.0);
        update(&mut f);
        assert!(f.weapon_spawn.is_none());
        f.status.set_time(3.0);
        update(&mut f);
        let spawn = f.take_weapon_spawn().expect("wave at frame 3");
        assert!(matches!(
            spawn.kind,
            WeaponKind::KirbyCutter {
                floor_line: Some(3)
            }
        ));
        assert_eq!(spawn.position.x, FINALCUTTER_OFF_X);
    }

    /// `ftCommonAttack100LoopKirbyUpdateEffect` tests `fkind ==
    /// nFTKindKirby`: Polygon Kirby makes no puff and keeps its flag 2;
    /// Kirby makes one through TopN, marking it, and clears the flag.
    #[test]
    fn only_kirby_makes_vulcan_puffs_and_the_flag_clears() {
        for (kind, puffs) in [(FighterKind::Kirby, 1), (FighterKind::PolyKirby, 0)] {
            let mut f = Fighter::new(kind, 0, 4);
            f.effects.clear();
            f.queried_joints.set(0);
            f.motion_script.flags[2] = 2;
            attack100_loop_update_effect(&mut f);
            assert_eq!(f.effects.len(), puffs, "{kind:?}");
            assert_eq!(f.motion_script.flags[2], if puffs == 1 { 0 } else { 2 });
            assert_eq!(f.queried_joints.get() & 1, puffs as u64, "{kind:?}");
        }
    }

    /// `ftCommonAttack100LoopProcUpdate`'s `else if
    /// (ftCommonGetCheckInterruptCommon() == FALSE)`: at a flag 1 with an
    /// item in reach the loop picks it up instead. RE-513 traces t2l/t2c:
    /// Link and Captain Falcon go to `LightGet` (status 100) on a flag-1
    /// frame of the loop; the first flag 1 is on loop frame 5 for Link, 6
    /// for Captain Falcon and 8 for Kirby.
    #[test]
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
    fn a_rapid_jab_flag_picks_up_an_item_in_reach() {
        use crate::item::{HeldItem, ItemKind, ItemType, ItemWeight, PickupCandidate};
        use crate::status::Status;
        type Case = (FighterKind, fn(&mut Fighter), AnyStatus, u32);
        let cases: [Case; 3] = [
            (
                FighterKind::Kirby,
                set_attack100_start,
                AnyStatus::Kirby(K::Attack100Loop),
                8,
            ),
            (
                FighterKind::Link,
                crate::link::set_attack100_start,
                AnyStatus::Link(crate::status::LinkStatus::Attack100Loop),
                5,
            ),
            (
                FighterKind::Captain,
                crate::captain::set_attack100_start,
                AnyStatus::Captain(crate::status::CaptainStatus::Attack100Loop),
                6,
            ),
        ];
        for (kind, start, looping, flag1) in cases {
            let mut f = Fighter::new(kind, 0, 4);
            f.situation = Situation::Ground;
            start(&mut f);
            let mut frame = 0u32;
            while f.status.status != looping {
                press(
                    &mut f,
                    if frame.is_multiple_of(2) { 0x8000 } else { 0 },
                    0,
                    0,
                );
                status::update(&mut f);
                frame += 1;
                assert!(frame < 30, "{kind:?} never reached the loop");
            }
            let reach = crate::item_throw::pickup(kind).offset_light;
            f.items.view.candidates[0] = Some(PickupCandidate {
                item: HeldItem {
                    slot: 0,
                    kind: ItemKind::MSBomb,
                    ty: ItemType::Swing,
                    weight: ItemWeight::Light,
                },
                pos: Vec3::new(f.pos.x + f.facing.sign() * reach.x, f.pos.y + reach.y, 0.0),
                coll: crate::ground::BodyColl::default(),
                floor_line: f.floor.map(|s| s.line),
            });
            let mut updates = 0;
            while f.status.status == looping {
                press(
                    &mut f,
                    if (frame + updates).is_multiple_of(2) {
                        0x8000
                    } else {
                        0
                    },
                    0,
                    0,
                );
                status::update(&mut f);
                updates += 1;
                assert!(updates < 60, "{kind:?} never picked the item up");
            }
            assert_eq!(
                f.status.status,
                AnyStatus::Common(Status::LightGet),
                "{kind:?}"
            );
            assert_eq!(updates, flag1, "{kind:?}");
        }
    }
}
