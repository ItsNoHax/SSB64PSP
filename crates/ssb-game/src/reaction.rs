//! Hit reactions after the hit — the damage statuses' own update and
//! interrupt (`ftcommondamage.c`), `DamageFall`, the knockdown chain
//! (`ftcommondownwaitbounce.c`, `ftcommondownstand.c`,
//! `ftcommondownforwardback.c`, `ftcommondownattack.c`), teching
//! (`ftcommonpassive.c`, `ftcommonpassivestand.c`), rolls
//! (`ftcommonescape.c`), clank recoil (`ftcommonrebound.c`) and the shield
//! break chain (`ftcommonshieldbreak*.c`, `ftcommonfurafura.c`).
//!
//! Status lengths are the statuses' figatree lengths
//! ([`crate::motion::anim_length`]); hit-status windows (a roll's or a tech's
//! intangibility, a getup attack's hitboxes) come from their motion scripts.
//!
//! Surface reactions (`ftcommonwalldamage.c`, `ftcommonstopceil.c` and
//! `ftCommonDamageAirCommonProcMap`) run on the shared map solver's damage
//! sweep ([`crate::map::move_damage`]); `DamageFall` catches ledges through
//! [`crate::map::allows_cliff`] like the other falls.

use ssb_engine::input::N64Buttons;

use crate::fighter::{Facing, Fighter, FighterKind};
use crate::status::{self, AnyStatus, Status, StatusTiming};

/// `FTCOMMON_DOWNWAIT_STAND_WAIT`.
pub const DOWNWAIT_STAND_WAIT: i32 = 180;
/// `FTCOMMON_DOWNWAIT_STAND_STICK_RANGE_MIN`.
pub const DOWNWAIT_STAND_STICK_RANGE_MIN: i32 = 20;
/// `FTCOMMON_DOWNBOUNCE_ATTACK_BUFFER`.
pub const DOWNBOUNCE_ATTACK_BUFFER: i32 = 60;
/// `FTCOMMON_DOWN_FORWARD_BACK_RANGE_MIN`.
pub const DOWN_FORWARD_BACK_RANGE_MIN: i32 = 20;
/// `FTCOMMON_PASSIVE_BUFFER_TICS_MAX`: a Z tap this recent techs.
pub const PASSIVE_BUFFER_TICS_MAX: u32 = 20;
/// `FTCOMMON_PASSIVE_F_OR_B_RANGE`.
pub const PASSIVE_F_OR_B_RANGE: i32 = 20;
/// `FTCOMMON_ESCAPE_STICK_RANGE_MIN`.
pub const ESCAPE_STICK_RANGE_MIN: i32 = 56;
/// `FTCOMMON_ESCAPE_BUFFER_TICS_MAX`.
pub const ESCAPE_BUFFER_TICS_MAX: u8 = 4;
/// `FTCOMMON_FURAFURA_BREAKOUT_WAIT_DEFAULT` and the US `..._MIN`.
pub const FURAFURA_BREAKOUT_WAIT_DEFAULT: i32 = 400;
pub const FURAFURA_BREAKOUT_WAIT_MIN: i32 = 90;
/// `ftCommonFuraFura`/`ftMainProcParams`: a broken shield comes back at 30.
pub const SHIELD_HEALTH_AFTER_BREAK: f32 = 30.0;
/// `F_CST_DTOR32(50.0F)`.
const STICK_ANGLE_50: f32 = 0.872_664_6;

/// Per-status counters (`status_vars.common.downbounce/downwait/rebound`).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ReactionState {
    pub itemthrow_buffer_tics: u8,
    /// `downbounce.attack_buffer`.
    pub attack_buffer: i32,
    /// `damage.dust_effect_int`: frames to the next dust cloud.
    pub dust_effect_int: i32,
    /// `downwait.stand_wait`.
    pub stand_wait: i32,
    /// `rebound.rebound_timer`.
    pub rebound_timer: f32,
    /// `rebound.anim_speed`.
    pub rebound_speed: f32,
    /// `damage.is_knockback_over`: the hit's knockback reached
    /// [`KNOCKBACK_OVER`]. A status variable, so a status change keeps it.
    pub is_knockback_over: bool,
    /// `proc_passive == ftCommonDamageCheckSetInvincible`: set by
    /// `ftCommonDamageInitDamageVars` for a non-electric damage status,
    /// cleared by the next status (`ftMainSetStatus` clears `proc_passive`).
    pub is_passive_invincible: bool,
    /// `damage.coll_mask_curr` / `coll_mask_prev`: the surfaces struck this
    /// frame and last ([`crate::map::MASK_LWALL`] and friends). Status
    /// variables: `WallDamage` inherits the mask of the hit that made it, so
    /// the same wall cannot bounce the fighter twice in a row.
    /// (`coll_mask_ignore` and `wall_collide_angle` are written but never
    /// read in the source, so they are not kept.)
    pub coll_mask_curr: u16,
    pub coll_mask_prev: u16,
}

/// `FTCOMMON_WALLDAMAGE_INTANGIBLE_TIMER`.
pub const WALLDAMAGE_INTANGIBLE_TIMER: u16 = 15;
/// `ftCommonWallDamageSetStatus` plays `WallDamage` at double speed.
pub const WALLDAMAGE_ANIM_SPEED: f32 = 2.0;
/// `lbCommonScale2D(&vel_air, 0.8F)`: a bounce keeps 80% of the speed.
pub const WALLDAMAGE_VEL_MUL: f32 = 0.8;

/// `ftCommonDamageInitDamageVars`: knockback at or above this sets
/// `is_knockback_over`, which grants one frame of invincibility once hitlag
/// ends.
pub const KNOCKBACK_OVER: f32 = 65000.0;

/// `ftCommonDamageCheckSetInvincible`, the damage statuses' `proc_passive`:
/// after the hitlag of a knockback overrun, one frame of hit invincibility
/// (`ftParamSetTimedHitStatusInvincible(fp, 1)`). Runs after the timers
/// count down, as `ftMainProcUpdateMain` calls `proc_passive` after them.
pub fn check_set_invincible(f: &mut Fighter) {
    if f.reaction.is_passive_invincible && f.hitlag == 0 && f.reaction.is_knockback_over {
        f.reaction.is_knockback_over = false;
        set_timed_invincible(f, 1);
    }
}

/// `ftParamSetTimedHitStatusInvincible`, with its `NoDamage` flicker.
pub fn set_timed_invincible(f: &mut Fighter, frames: u16) {
    if f.invincible_frames < frames {
        f.invincible_frames = frames;
    }
    crate::colanim::check_set(f, crate::colanim::ColAnimId::FIGHTER_NO_DAMAGE, 0);
}

/// `ftParamSetTimedHitStatusIntangible`, with its `NoDamage` flicker.
pub fn set_timed_intangible(f: &mut Fighter, frames: u16) {
    if f.intangible_frames < frames {
        f.intangible_frames = frames;
    }
    crate::colanim::check_set(f, crate::colanim::ColAnimId::FIGHTER_NO_DAMAGE, 0);
}

/// The status's figatree length as its timing.
fn timing(f: &Fighter, status: Status) -> StatusTiming {
    match crate::motion::anim_length(f.kind, status.into()) {
        Some(len) => StatusTiming::frames(len),
        None => StatusTiming::unknown(),
    }
}

fn timing_at(f: &Fighter, status: Status, speed: f32) -> StatusTiming {
    match crate::motion::anim_length(f.kind, status.into()) {
        Some(len) => StatusTiming::at_speed(len, speed),
        None => StatusTiming {
            anim_length: None,
            anim_speed: speed,
            looping: false,
        },
    }
}

fn set(f: &mut Fighter, status: Status) {
    let t = timing(f, status);
    status::set_status(f, status, 0.0, t);
}

fn set_preserve(f: &mut Fighter, status: Status, preserve: status::Preserve) {
    let t = timing(f, status);
    status::set_any_status_preserve(f, status.into(), 0.0, t, preserve);
}

/// `ftParamGetStickAngleRads`: the stick's angle above horizontal.
fn stick_angle(f: &Fighter) -> f32 {
    ssb_engine::math::atan2(f.stick.y as f32, (f.stick.x as f32).abs())
}

/// `mpCommonSetFighterGround` for a reaction that lands.
fn set_ground(f: &mut Fighter, floor_y: f32) {
    if !f.is_grounded() {
        f.land(floor_y);
    }
}

/// `ftParamVelDamageTransferGround`: landing carries the damage velocity's
/// X, capped at 250, along the floor.
fn vel_damage_transfer_ground(f: &mut Fighter) {
    if !f.is_grounded() || f.physics.vel_damage_ground != 0.0 {
        return;
    }
    let normal = f
        .floor
        .map(|s| s.normal)
        .unwrap_or(ssb_engine::math::Vec2::new(0.0, 1.0));
    let g = f.physics.vel_knockback.x.clamp(-250.0, 250.0);
    f.physics.vel_damage_ground = g;
    f.physics.vel_knockback.x = normal.y * g;
    f.physics.vel_knockback.y = -normal.x * g;
}

/// `ftCommonDownBounceCheckUpOrDown`: whether the fighter lands face down,
/// from the pitch of the model's first joint (`joints[4]->rotate.x`). The
/// runtime supplies the posed joint; without one the fighter lands face up.
pub fn is_face_down(f: &Fighter) -> bool {
    let Some(t) = f.joint_transforms[4] else {
        return false;
    };
    // The facing yaw maps model +Z to world `lr * X`; a pitch about the
    // model's X axis tips its Y axis toward model Z.
    let up = t.axes[1];
    let theta = ssb_engine::math::atan2(up.x * f.facing.sign(), up.y);
    let mut r = theta / core::f32::consts::TAU;
    r -= r as i32 as f32;
    r < -0.5 || (r > 0.0 && r < 0.5)
}

// ---------------------------------------------------------------------------
// Damage statuses
// ---------------------------------------------------------------------------

/// The damage statuses on `ftCommonDamageCommonProc*` (ground table,
/// `DamageAir1`-`3` and `DamageE1`).
pub fn is_damage_common(s: Status) -> bool {
    matches!(
        s,
        Status::DamageHi1
            | Status::DamageHi2
            | Status::DamageHi3
            | Status::DamageN1
            | Status::DamageN2
            | Status::DamageN3
            | Status::DamageLw1
            | Status::DamageLw2
            | Status::DamageLw3
            | Status::DamageAir1
            | Status::DamageAir2
            | Status::DamageAir3
            | Status::DamageE1
    )
}

/// The tumbles on `ftCommonDamageAirCommonProc*`.
pub fn is_damage_air(s: Status) -> bool {
    matches!(
        s,
        Status::DamageE2
            | Status::DamageFlyHi
            | Status::DamageFlyN
            | Status::DamageFlyLw
            | Status::DamageFlyTop
            | Status::DamageFlyRoll
            | Status::WallDamage
    )
}

/// `ftCommonFallProcInterrupt`'s checks (`DamageFall` has the same).
pub fn air_interrupt(f: &mut Fighter) -> bool {
    status::check_special_n(f)
        || status::check_special_hi(f)
        || status::check_special_lw(f)
        || status::check_attack_air(f)
        || status::check_jump_aerial(f)
}

/// `ftCommonDamageFallSetStatusFromDamage`.
pub fn set_damage_fall(f: &mut Fighter) {
    let fastfall = f.physics.is_fastfall;
    set(f, Status::DamageFall);
    f.physics.is_fastfall = fastfall;
    crate::physics::clamp_air_vel_x(&mut f.physics, f.attributes.air_speed_max_x);
}

/// `ftCommonDamageFallSetStatusFromCliffWait`: a ledge hang that times out
/// cannot tech with a Z tap made while hanging.
pub fn set_damage_fall_from_cliff_wait(f: &mut Fighter) {
    set_damage_fall(f);
    f.tics_since_last_z = status::ZTRIGLAST_TICS_MAX;
}

// ---------------------------------------------------------------------------
// Surface reactions
// ---------------------------------------------------------------------------

/// `ftCommonDamageAirCommonProcMap`, after the damage sweep: a struck wall or
/// ceiling bounces the fighter (`ftCommonWallDamageCheckGoto`), a struck
/// floor techs or knocks it down.
pub fn damage_air_proc_map(f: &mut Fighter, sweep: &crate::map::DamageMoved) {
    if !sweep.collide || wall_damage_check_goto(f, sweep) {
        return;
    }
    if sweep.mask_curr & crate::map::MASK_FLOOR != 0 {
        let y = f.pos.y;
        if !check_passive_stand(f, y) && !check_passive(f, y) {
            set_down_bounce(f, y);
        }
    }
}

/// `ftCommonWallDamageCheckGoto`: left wall, then right wall, then ceiling.
fn wall_damage_check_goto(f: &mut Fighter, sweep: &crate::map::DamageMoved) -> bool {
    let mask = sweep.mask_curr;
    let normal = if mask & crate::map::MASK_LWALL != 0 {
        sweep.lwall_normal
    } else if mask & crate::map::MASK_RWALL != 0 {
        sweep.rwall_normal
    } else if mask & crate::map::MASK_CEIL != 0 {
        sweep.ceil_normal
    } else {
        return false;
    };
    set_wall_damage(f, normal);
    true
}

/// `ftCommonWallDamageSetStatus`: the fighter's whole velocity reflects off
/// the surface at 80% as damage velocity, with fresh hitstun for that speed
/// and 15 frames of intangibility. It faces away from its new direction.
pub fn set_wall_damage(f: &mut Fighter, normal: ssb_engine::math::Vec2) {
    let mut vx = f.physics.vel_air.x + f.physics.vel_knockback.x;
    let mut vy = f.physics.vel_air.y + f.physics.vel_knockback.y;
    // `lbCommonReflect2D`.
    let d = (normal.x * vx + normal.y * vy) * -2.0;
    vx += normal.x * d;
    vy += normal.y * d;
    vx *= WALLDAMAGE_VEL_MUL;
    vy *= WALLDAMAGE_VEL_MUL;
    // `vel_damage_air = vel_air` copies the air velocity's Z as well.
    f.physics.vel_knockback = ssb_engine::math::Vec3::new(vx, vy, f.physics.vel_air.z);
    f.physics.vel_air = ssb_engine::math::Vec3::ZERO;
    f.facing = if vx < 0.0 {
        Facing::Right
    } else {
        Facing::Left
    };
    let knockback = ssb_engine::math::sqrt(vx * vx + vy * vy);
    let hitstun = crate::attack::hitstun_frames(knockback) as i32;
    let (vel, ground) = (f.physics.vel_knockback, f.physics.vel_damage_ground);
    let t = timing_at(f, Status::WallDamage, WALLDAMAGE_ANIM_SPEED);
    status::set_any_status_preserve(
        f,
        Status::WallDamage.into(),
        0.0,
        t,
        status::Preserve::DAMAGE_PLAYER,
    );
    f.physics.vel_knockback = vel;
    f.physics.vel_damage_ground = ground;
    f.hitstun = hitstun.max(0) as u16;
    f.damage_knockback_stack = knockback;
    set_timed_intangible(f, WALLDAMAGE_INTANGIBLE_TIMER);
}

/// `ftCommonStopCeilSetStatus`: a fast rise into a ceiling stops dead.
pub fn set_stop_ceil(f: &mut Fighter) {
    set(f, Status::StopCeil);
    status::play_anim_events(f);
    f.physics.vel_air.y = 0.0;
    f.physics.vel_air.z = 0.0;
}

// ---------------------------------------------------------------------------
// Status updates
// ---------------------------------------------------------------------------

/// `proc_update` + `proc_interrupt` for the statuses this module owns.
/// Returns whether `current` was one of them.
pub fn update(f: &mut Fighter, current: Status) -> bool {
    match current {
        // `ftCommonDamageCommonProcUpdate` / `ProcInterrupt`.
        s if is_damage_common(s) => {
            if f.status.animation_ended() && f.hitstun == 0 {
                status::set_wait_or_fall(f);
            } else if f.hitstun == 0 {
                if f.is_grounded() {
                    status::ground_interrupt(f);
                } else {
                    air_interrupt(f);
                }
            }
        }
        // `ftCommonWallDamageProcUpdate`: the bounce lasts its hitstun, not
        // its animation. The fall's interrupt runs as the new status's.
        Status::WallDamage => {
            update_dust_effect(f);
            if f.hitstun == 0 {
                set_damage_fall(f);
                air_interrupt(f);
            }
        }
        // `ftAnimEndSetFall`.
        Status::StopCeil => {
            if f.status.animation_ended() {
                status::set_fall(f);
            }
        }
        // `ftCommonDamageAirCommonProcUpdate` / `ProcInterrupt`.
        s if is_damage_air(s) => {
            update_dust_effect(f);
            if f.status.animation_ended() && f.hitstun == 0 {
                set_damage_fall(f);
            } else if f.hitstun == 0 {
                air_interrupt(f);
            }
        }
        // `ftCommonDamageFallProcInterrupt`.
        Status::DamageFall => {
            if crate::item_use::holds_hammer(f)
                && f.button_tap().contains(N64Buttons::A | N64Buttons::B)
            {
                crate::item_use::hammer_fall(f);
                return true;
            }
            air_interrupt(f);
        }
        Status::DownBounceD | Status::DownBounceU => update_down_bounce(f),
        Status::DownWaitD | Status::DownWaitU => update_down_wait(f),
        // `ftAnimEndSetWait` + `ftCommonDownStandProcInterrupt`.
        Status::DownStandD | Status::DownStandU => {
            if f.status.animation_ended() {
                status::set_wait(f);
            } else if f.motion_script.flags[1] != 0 && !status::check_kneebend(f) {
                status::check_pass(f);
            }
        }
        Status::PassiveStandF
        | Status::PassiveStandB
        | Status::DownForwardD
        | Status::DownForwardU
        | Status::DownBackD
        | Status::DownBackU
        | Status::DownAttackD
        | Status::DownAttackU
        | Status::Passive => {
            if f.status.animation_ended() {
                status::set_wait(f);
            }
        }
        // `ftCommonReboundWaitProcUpdate`.
        Status::ReboundWait => set_rebound(f),
        // `ftCommonReboundProcUpdate`.
        Status::Rebound => {
            f.reaction.rebound_timer -= 1.0;
            if f.reaction.rebound_timer <= 0.0 {
                status::set_wait(f);
            }
        }
        Status::EscapeF | Status::EscapeB => update_escape(f),
        // `ftCommonShieldBreakFlyProcUpdate`.
        Status::ShieldBreakFly => {
            if f.status.animation_ended() {
                set_shield_break_fall(f);
            }
        }
        Status::ShieldBreakFall => {}
        // `ftCommonShieldBreakDownProcUpdate`.
        Status::ShieldBreakDownD | Status::ShieldBreakDownU => {
            if f.status.animation_ended() {
                set_shield_break_stand(f);
            }
        }
        // `ftCommonShieldBreakStandProcUpdate`.
        Status::ShieldBreakStandD | Status::ShieldBreakStandU => {
            if f.status.animation_ended() {
                set_furafura(f);
            }
        }
        Status::FuraFura => update_furafura(f),
        _ => return false,
    }
    true
}

/// Landing (`proc_map`) for the airborne reactions. Returns whether the
/// fighter's status handled the floor contact.
pub fn on_landing(f: &mut Fighter, floor_y: f32) -> bool {
    let AnyStatus::Common(current) = f.status.status else {
        return false;
    };
    match current {
        // `ftCommonDamageFallProcMap`'s floor half (its ledge half is the
        // shared cliff sweep). The airborne damage statuses land through
        // [`damage_air_proc_map`] instead.
        Status::DamageFall => {
            if !check_passive_stand(f, floor_y) && !check_passive(f, floor_y) {
                set_down_bounce(f, floor_y);
            }
            true
        }
        // `ftCommonShieldBreakFlyProcMap` / `...FallProcMap`.
        Status::ShieldBreakFly | Status::ShieldBreakFall => {
            set_shield_break_down(f, floor_y);
            true
        }
        _ => false,
    }
}

/// The reactions whose `proc_physics` is `ftPhysicsApplyGroundVelTransN`:
/// the runtime samples their clip's TransN into [`Fighter::root_motion`].
pub fn moves_by_transn(s: AnyStatus) -> bool {
    matches!(
        s,
        AnyStatus::Common(
            Status::EscapeF
                | Status::EscapeB
                | Status::PassiveStandF
                | Status::PassiveStandB
                | Status::DownForwardD
                | Status::DownForwardU
                | Status::DownBackD
                | Status::DownBackU
        )
    )
}

/// Ground physics for the reactions that move by their animation
/// (`ftPhysicsApplyGroundVelTransN`). Returns whether it applied.
pub fn apply_ground_physics(f: &mut Fighter) -> bool {
    if !moves_by_transn(f.status.status) {
        return false;
    }
    crate::physics::apply_ground_vel_transn(&mut f.physics, f.root_motion, f.facing.sign());
    true
}

/// Air physics for the shield-break flight and fall
/// (`ftPhysicsApplyAirVelFriction`). Returns whether it applied.
pub fn apply_air_physics(f: &mut Fighter) -> bool {
    if !matches!(
        f.status.status,
        AnyStatus::Common(Status::ShieldBreakFly | Status::ShieldBreakFall)
    ) {
        return false;
    }
    if f.physics.is_fastfall {
        crate::physics::apply_fast_fall(&mut f.physics, &f.attributes);
    } else {
        crate::physics::apply_gravity_default(&mut f.physics, &f.attributes);
    }
    if !crate::physics::check_clamp_air_vel_x_dec(&mut f.physics, f.attributes.air_speed_max_x) {
        crate::physics::apply_air_friction(&mut f.physics, &f.attributes);
    }
    true
}

// ---------------------------------------------------------------------------
// Teching
// ---------------------------------------------------------------------------

/// `ftCommonPassiveStandCheckInterruptDamage`: a recent Z tap with the stick
/// held sideways techs into a roll.
fn check_passive_stand(f: &mut Fighter, floor_y: f32) -> bool {
    if f.tics_since_last_z >= PASSIVE_BUFFER_TICS_MAX
        || i32::from(f.stick.x).abs() < PASSIVE_F_OR_B_RANGE
    {
        return false;
    }
    let status = if f.stick.x as f32 * f.facing.sign() >= 0.0 {
        Status::PassiveStandF
    } else {
        Status::PassiveStandB
    };
    set_ground(f, floor_y);
    set(f, status);
    vel_damage_transfer_ground(f);
    true
}

/// `ftCommonPassiveCheckInterruptDamage`: a recent Z tap techs in place.
fn check_passive(f: &mut Fighter, floor_y: f32) -> bool {
    if f.tics_since_last_z >= PASSIVE_BUFFER_TICS_MAX {
        return false;
    }
    set_ground(f, floor_y);
    set(f, Status::Passive);
    vel_damage_transfer_ground(f);
    true
}

// ---------------------------------------------------------------------------
// Knockdown
// ---------------------------------------------------------------------------

/// `ftCommonDamageSetDustEffectInterval`: frames between the dust clouds a
/// launched fighter trails, by its knockback speed.
pub fn set_dust_effect_interval(f: &mut Fighter) {
    let vel = if f.is_grounded() {
        f.physics.vel_damage_ground.abs()
    } else {
        f.physics.vel_knockback.length()
    };
    f.reaction.dust_effect_int = if vel < 120.0 {
        0
    } else if vel < 150.0 {
        8
    } else if vel < 200.0 {
        5
    } else if vel < 300.0 {
        3
    } else if vel < 600.0 {
        2
    } else {
        1
    };
}

/// `ftCommonDamageUpdateDustEffect`: a large dust cloud at joint 4 each
/// interval.
fn update_dust_effect(f: &mut Fighter) {
    if f.reaction.dust_effect_int != 0 {
        f.reaction.dust_effect_int -= 1;
        if f.reaction.dust_effect_int == 0 {
            let lr = f.facing.sign() as i8;
            crate::fteffect::request(
                f,
                crate::fteffect::EffectRequest::at_joint(
                    crate::fteffect::kind::DUST_EXPAND_LARGE,
                    4,
                    lr,
                ),
            );
            set_dust_effect_interval(f);
        }
    }
}

/// `ftCommonDownBounceSetStatus`.
pub fn set_down_bounce(f: &mut Fighter, floor_y: f32) {
    set_ground(f, floor_y);
    let status = if is_face_down(f) {
        Status::DownBounceD
    } else {
        Status::DownBounceU
    };
    set(f, status);
    down_bounce_effects(f);
    f.reaction.attack_buffer = 0;
    f.damage_mul = 0.5;
    vel_damage_transfer_ground(f);
}

/// `ftCommonDownBounceUpdateEffects`: the impact wave (the sound and rumble
/// are not ported).
fn down_bounce_effects(f: &mut Fighter) {
    let lr = f.facing.sign() as i8;
    crate::fteffect::request(
        f,
        crate::fteffect::EffectRequest::at_joint(crate::fteffect::kind::IMPACT_WAVE, 0, lr),
    );
}

fn is_down(f: &Fighter) -> bool {
    matches!(
        f.status.status,
        AnyStatus::Common(Status::DownBounceD | Status::DownWaitD)
    )
}

/// `ftCommonDownBounceProcUpdate`: an A or B tap during the bounce is
/// buffered for 60 frames into a getup attack.
fn update_down_bounce(f: &mut Fighter) {
    if f.reaction.attack_buffer != 0 {
        f.reaction.attack_buffer -= 1;
    }
    if f.button_tap().0 & (N64Buttons::A | N64Buttons::B) != 0 {
        f.reaction.attack_buffer = DOWNBOUNCE_ATTACK_BUFFER;
    }
    if f.status.animation_ended()
        && !check_down_attack_from_bounce(f)
        && !check_down_forward_or_back(f)
    {
        set_down_wait(f);
    }
}

/// `ftCommonDownWaitSetStatus`.
fn set_down_wait(f: &mut Fighter) {
    let status = if f.status.status == Status::DownBounceD {
        Status::DownWaitD
    } else {
        Status::DownWaitU
    };
    set(f, status);
    f.reaction.stand_wait = DOWNWAIT_STAND_WAIT;
    // `ftParamSetCaptureImmuneMask`: grabs and Egg Lay miss a downed fighter.
    f.grab.capture_immune = true;
    f.damage_mul = 0.5;
}

/// `ftCommonDownWaitProcUpdate` + `ProcInterrupt`.
fn update_down_wait(f: &mut Fighter) {
    f.reaction.stand_wait -= 1;
    if f.reaction.stand_wait == 0 {
        set_down_stand(f);
        return;
    }
    if !check_down_attack_from_wait(f) && !check_down_forward_or_back(f) {
        check_down_stand(f);
    }
}

/// `ftCommonDownStandSetStatus`.
fn set_down_stand(f: &mut Fighter) {
    let status = if is_down(f) {
        Status::DownStandD
    } else {
        Status::DownStandU
    };
    set(f, status);
    f.motion_script.flags[1] = 0;
}

/// `ftCommonDownStandCheckInterruptCommon`: stick up or a Z tap.
fn check_down_stand(f: &mut Fighter) -> bool {
    if (i32::from(f.stick.y) >= DOWNWAIT_STAND_STICK_RANGE_MIN && stick_angle(f) >= STICK_ANGLE_50)
        || f.button_tap().contains(N64Buttons::Z)
    {
        set_down_stand(f);
        return true;
    }
    false
}

/// `ftCommonDownForwardOrBackCheckInterruptCommon`.
fn check_down_forward_or_back(f: &mut Fighter) -> bool {
    if i32::from(f.stick.x).abs() < DOWN_FORWARD_BACK_RANGE_MIN || stick_angle(f) >= STICK_ANGLE_50
    {
        return false;
    }
    let forward = f.stick.x as f32 * f.facing.sign() >= 0.0;
    let status = match (forward, is_down(f)) {
        (true, true) => Status::DownForwardD,
        (true, false) => Status::DownForwardU,
        (false, true) => Status::DownBackD,
        (false, false) => Status::DownBackU,
    };
    set(f, status);
    status::play_anim_events(f);
    true
}

/// `ftCommonDownAttackCheckInterruptDownBounce`.
fn check_down_attack_from_bounce(f: &mut Fighter) -> bool {
    if f.reaction.attack_buffer == 0 {
        return false;
    }
    let status = if f.status.status == Status::DownBounceD {
        Status::DownAttackD
    } else {
        Status::DownAttackU
    };
    set(f, status);
    status::play_anim_events(f);
    true
}

/// `ftCommonDownAttackCheckInterruptDownWait`.
fn check_down_attack_from_wait(f: &mut Fighter) -> bool {
    if f.button_tap().0 & (N64Buttons::A | N64Buttons::B) == 0 {
        return false;
    }
    let status = if f.status.status == Status::DownWaitD {
        Status::DownAttackD
    } else {
        Status::DownAttackU
    };
    set(f, status);
    status::play_anim_events(f);
    true
}

// ---------------------------------------------------------------------------
// Rolls
// ---------------------------------------------------------------------------

/// `ftCommonEscapeGetStatus`: a hard, fresh sideways stick.
pub fn escape_status(f: &Fighter) -> Option<Status> {
    if i32::from(f.stick.x).abs() >= ESCAPE_STICK_RANGE_MIN
        && f.stick.tap_x < ESCAPE_BUFFER_TICS_MAX
    {
        Some(if f.stick.x as f32 * f.facing.sign() >= 0.0 {
            Status::EscapeF
        } else {
            Status::EscapeB
        })
    } else {
        None
    }
}

/// `ftCommonEscapeSetStatus`.
pub fn set_escape(f: &mut Fighter, status: Status) {
    f.reaction.itemthrow_buffer_tics = 0;
    // `ftCommonEscapeProcStatus`.
    f.motion_script.flags[1] = 0;
    set(f, status);
    status::play_anim_events(f);
}

/// `ftCommonEscapeCheckInterruptGuard`.
pub fn check_escape_guard(f: &mut Fighter) -> bool {
    match escape_status(f) {
        Some(status) => {
            set_escape(f, status);
            f.reaction.itemthrow_buffer_tics = 5;
            true
        }
        None => false,
    }
}

/// `ftCommonEscapeCheckInterruptDash`: Z during a dash rolls forward.
pub fn check_escape_dash(f: &mut Fighter) -> bool {
    if f.button_tap().contains(N64Buttons::Z) {
        set_escape(f, Status::EscapeF);
        return true;
    }
    false
}

/// `ftCommonEscapeProcUpdate`: the script's flag 1 turns the fighter round;
/// at the end, Yoshi can keep his shield up.
fn update_escape(f: &mut Fighter) {
    if f.motion_script.flags[1] != 0 {
        f.motion_script.flags[1] = 0;
        f.facing = match f.facing {
            Facing::Right => Facing::Left,
            Facing::Left => Facing::Right,
        };
    }
    if f.status.animation_ended() {
        f.physics.vel_air = ssb_engine::math::Vec3::ZERO;
        f.physics.vel_ground = ssb_engine::math::Vec3::ZERO;
        if f.kind != FighterKind::Yoshi || !status::check_guard_from_escape(f) {
            status::set_wait(f);
        }
    } else {
        crate::item_throw::check_escape(f);
    }
}

// ---------------------------------------------------------------------------
// Clank recoil
// ---------------------------------------------------------------------------

/// `ftCommonReboundWaitSetStatus`: pushed back `2 * rebound` along the
/// ground, away from the side the clank came from.
pub fn set_rebound_wait(f: &mut Fighter, attack_rebound: f32, hit_lr: f32) {
    set(f, Status::ReboundWait);
    let length = crate::motion::combat_attrs(f.kind).map_or(16.0, |a| a.rebound_anim_length);
    f.reaction.rebound_speed = length / attack_rebound;
    f.reaction.rebound_timer = attack_rebound;
    let lr = f.facing.sign();
    let lr_rebound = if lr == hit_lr { -1.0 } else { 1.0 };
    // `vel_ground.x` is facing-relative in the original.
    f.physics.vel_ground.x = lr * lr_rebound * (2.0 * f.reaction.rebound_timer);
}

/// `ftCommonReboundSetStatus`.
fn set_rebound(f: &mut Fighter) {
    let t = timing_at(f, Status::Rebound, f.reaction.rebound_speed);
    let vel = f.physics.vel_ground;
    status::set_status(f, Status::Rebound, 0.0, t);
    f.physics.vel_ground = vel;
}

// ---------------------------------------------------------------------------
// Shield break
// ---------------------------------------------------------------------------

/// `ftCommonShieldBreakFlySetStatus`: the broken shield launches the fighter
/// straight up at `shield_break_vel_y`.
pub fn set_shield_break_fly(f: &mut Fighter) {
    if f.is_grounded() {
        f.become_airborne();
        f.floor = None;
    }
    set_preserve(f, Status::ShieldBreakFly, status::Preserve::DAMAGE_PLAYER);
    status::play_anim_events(f);
    f.physics.vel_air.x = 0.0;
    f.physics.vel_air.y =
        crate::motion::combat_attrs(f.kind).map_or(70.0, |a| a.shield_break_vel_y);
    crate::colanim::check_set(f, crate::colanim::ColAnimId::FIGHTER_SHIELD_BREAK_FLY, 0);
}

/// `ftCommonShieldBreakFallSetStatus`.
fn set_shield_break_fall(f: &mut Fighter) {
    set_preserve(
        f,
        Status::ShieldBreakFall,
        status::Preserve {
            hitstatus: true,
            damage_player: true,
            ..status::Preserve::NONE
        },
    );
    crate::physics::clamp_air_vel_x(&mut f.physics, f.attributes.air_speed_max_x);
}

/// `ftCommonShieldBreakDownSetStatus`.
fn set_shield_break_down(f: &mut Fighter, floor_y: f32) {
    set_ground(f, floor_y);
    let status = if is_face_down(f) {
        Status::ShieldBreakDownD
    } else {
        Status::ShieldBreakDownU
    };
    set_preserve(f, status, status::Preserve::HITSTATUS);
    down_bounce_effects(f);
}

/// `ftCommonShieldBreakStandSetStatus`.
fn set_shield_break_stand(f: &mut Fighter) {
    let status = if f.status.status == Status::ShieldBreakDownD {
        Status::ShieldBreakStandD
    } else {
        Status::ShieldBreakStandU
    };
    set_preserve(f, status, status::Preserve::HITSTATUS);
}

/// `ftCommonFuraFuraSetStatus`: dizzy for `max(400 - percent, 0) + 90`
/// frames, shortened by mashing.
fn set_furafura(f: &mut Fighter) {
    set(f, Status::FuraFura);
    f.guard.shield_health = SHIELD_HEALTH_AFTER_BREAK;
    let wait =
        (FURAFURA_BREAKOUT_WAIT_DEFAULT - i32::from(f.damage)).max(0) + FURAFURA_BREAKOUT_WAIT_MIN;
    crate::grab::init_breakout(f, wait);
    crate::colanim::check_set(f, crate::colanim::ColAnimId::FIGHTER_FURA_FURA, 0);
}

/// `ftCommonFuraFuraProcUpdate`: each mash counts four frames.
fn update_furafura(f: &mut Fighter) {
    f.guard.shield_health = SHIELD_HEALTH_AFTER_BREAK;
    f.grab.breakout_wait -= 1;
    let before = f.grab.breakout_wait;
    crate::grab::update_breakout(f);
    f.grab.breakout_wait += (f.grab.breakout_wait - before) * 3;
    if f.grab.breakout_wait <= 0 {
        status::set_wait(f);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::HitStatus;

    fn grounded(kind: FighterKind) -> Fighter {
        let mut f = Fighter::new(kind, 0, 3);
        f.situation = crate::fighter::Situation::Ground;
        f.floor = Some(crate::ground::Standing {
            line: 0,
            normal: ssb_engine::math::Vec2::new(0.0, 1.0),
            flags: 0,
        });
        f
    }

    fn step(f: &mut Fighter) {
        f.tick_timers();
        if !f.is_in_hitlag() {
            status::update(f);
        }
    }

    #[test]
    fn a_roll_is_intangible_and_ends_on_its_animation() {
        let mut f = grounded(FighterKind::Mario);
        set_escape(&mut f, Status::EscapeF);
        let len = crate::motion::anim_length(FighterKind::Mario, Status::EscapeF.into()).unwrap();
        let mut intangible = 0;
        for _ in 0..len as usize + 2 {
            if f.hitstatus == HitStatus::Intangible {
                intangible += 1;
            }
            step(&mut f);
        }
        assert_eq!(intangible, 16, "RollF: SetHitStatusAll(3) from 4 to 20");
        assert_eq!(f.status.status, Status::Wait);
    }

    #[test]
    fn a_damage_fall_landing_without_a_tech_bounces_then_waits_down() {
        let mut f = Fighter::new(FighterKind::Mario, 0, 3);
        status::set_status(&mut f, Status::DamageFall, 0.0, StatusTiming::unknown());
        f.tics_since_last_z = status::ZTRIGLAST_TICS_MAX;
        assert!(on_landing(&mut f, 0.0));
        assert_eq!(f.status.status, Status::DownBounceU);
        assert_eq!(f.damage_mul, 0.5);
        for _ in 0..40 {
            step(&mut f);
        }
        assert_eq!(f.status.status, Status::DownWaitU);
    }

    #[test]
    fn a_knockback_overrun_is_invincible_for_one_frame_after_hitlag() {
        for (kb, expect) in [(65000.0, true), (64999.0, false)] {
            let mut f = Fighter::new(FighterKind::Mario, 0, 3);
            crate::attack::init_damage_vars_full(
                &mut f,
                None,
                0,
                kb,
                45,
                1.0,
                0,
                crate::combat::Element::Normal,
                false,
            );
            assert_eq!(f.reaction.is_knockback_over, expect);
            f.hitlag = 2;
            f.tick_timers();
            check_set_invincible(&mut f);
            assert_eq!(f.invincible_frames, 0, "not during hitlag");
            f.tick_timers();
            check_set_invincible(&mut f);
            assert_eq!(f.invincible_frames, u16::from(expect));
            assert!(!f.reaction.is_knockback_over);
            f.tick_timers();
            check_set_invincible(&mut f);
            assert_eq!(f.invincible_frames, 0, "one frame only");
        }
        // Electric: `ftCommonDamageSetStatus` grants it when `DamageE`
        // hands over.
        let mut f = Fighter::new(FighterKind::Mario, 0, 3);
        crate::attack::init_damage_vars_full(
            &mut f,
            None,
            0,
            70000.0,
            45,
            1.0,
            0,
            crate::combat::Element::Electric,
            false,
        );
        assert_eq!(f.status.status, Status::DamageE2);
        assert!(!f.reaction.is_passive_invincible);
        crate::attack::update_damage_e(&mut f);
        assert_eq!(f.invincible_frames, 1);
    }

    #[test]
    fn a_z_tap_within_20_frames_techs() {
        let mut f = Fighter::new(FighterKind::Mario, 0, 3);
        status::set_status(&mut f, Status::DamageFall, 0.0, StatusTiming::unknown());
        f.tics_since_last_z = 19;
        assert!(on_landing(&mut f, 0.0));
        assert_eq!(f.status.status, Status::Passive);
        let mut f = Fighter::new(FighterKind::Mario, 0, 3);
        status::set_status(&mut f, Status::DamageFall, 0.0, StatusTiming::unknown());
        f.tics_since_last_z = 3;
        f.stick.x = 40;
        assert!(on_landing(&mut f, 0.0));
        assert_eq!(f.status.status, Status::PassiveStandF);
    }

    #[test]
    fn a_broken_shield_flies_lands_and_gets_dizzy() {
        let mut f = grounded(FighterKind::Mario);
        set_shield_break_fly(&mut f);
        assert!(!f.is_grounded());
        assert_eq!(f.physics.vel_air.y, 70.0);
        let len =
            crate::motion::anim_length(FighterKind::Mario, Status::ShieldBreakFly.into()).unwrap();
        for _ in 0..len as usize {
            step(&mut f);
        }
        assert_eq!(f.status.status, Status::ShieldBreakFall);
        assert!(on_landing(&mut f, 0.0));
        assert_eq!(f.status.status, Status::ShieldBreakDownU);
        for _ in 0..400 {
            step(&mut f);
            if f.status.status == Status::FuraFura {
                break;
            }
        }
        assert_eq!(f.status.status, Status::FuraFura);
        assert_eq!(f.grab.breakout_wait, 400 + 90);
    }

    #[test]
    fn rebound_pushes_away_from_the_clank_for_its_timer() {
        let mut f = grounded(FighterKind::Mario);
        f.facing = Facing::Right;
        set_rebound_wait(&mut f, 20.0, 1.0);
        assert_eq!(f.physics.vel_ground.x, -40.0);
        step(&mut f);
        assert_eq!(f.status.status, Status::Rebound);
        for _ in 0..20 {
            step(&mut f);
        }
        assert_eq!(f.status.status, Status::Wait);
    }
}
