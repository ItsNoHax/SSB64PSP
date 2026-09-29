//! Hitbox descriptors and the damage side of a hit: knockback, hitstun,
//! hitlag and `ftCommonDamageInitDamageVars`.
//!
//! Attack collisions are made by the motion scripts ([`crate::motion`]) and
//! resolved by the per-frame pipeline in [`crate::combat`]. This module
//! keeps the formulas both use:
//!
//! * `ft/ftcommon/ftcommondamage.c`'s `ftCommonDamageGetKnockbackAngle` and
//!   `ftCommonDamageInitDamageVars` — the "Sakurai angle" resolution, the
//!   launch/slide/bounce branches, `DamageFlyRoll` and the electric
//!   `DamageE1`/`DamageE2` detour.
//! * `ft/ftparam.c`'s `ftParamGetCommonKnockback`, `ftParamGetHitStun` and
//!   `ftParamGetHitLag`.
//!
//! [`register_hitbox`] feeds a positioned hitbox (a weapon) into the
//! defender's hit log; [`apply_hitbox_at`] also resolves it at once, for
//! callers outside the frame pipeline. A caller with no posed joints falls
//! back to one sphere of [`MARIO_HURTBOX_RADIUS`] at the fighter's root.

use ssb_engine::math::{sin_cos, Vec3};

use crate::fighter::Fighter;
use crate::status::{self, AnyStatus, Status, StatusTiming};

/// A hitbox descriptor, transcribed field-for-field from a
/// `ftMotionCommandMakeAttackColl(aid, gid, jid, dmg, reb, elem, sz, ox, oy,
/// oz, ang, kbs, kbw, ga, sd, fl, fk, kbb)` call, so it can be checked against
/// the decomp source by eye. Sound (`fl`, `fk`) is not carried.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hitbox {
    /// `dmg` — base damage, added to the target's percent as-is.
    pub damage: i32,
    /// `ox, oy, oz` — offset from the attachment joint.
    pub offset: Vec3,
    /// `sz` — the motion command's "size", which the original halves into a
    /// radius (`fttypes.h`); this field is already that radius.
    pub radius: f32,
    /// `ang` — knockback angle in degrees, or the sentinel `361` for the
    /// "Sakurai angle" ([`sakurai_angle_radians`]).
    pub angle: i32,
    /// `kbs` — knockback growth (KBG), applied as `kbs * 0.01`.
    pub kb_scale: i32,
    /// `kbw` — weight-dependent knockback scale (WDSC). Zero selects the
    /// damage-dependent branch of [`common_knockback`].
    pub kb_weight: i32,
    /// `kbb` — base knockback (BKB).
    pub kb_base: i32,
    /// `elem` — the hit element (`GMHitElement`).
    pub element: crate::combat::Element,
    /// `sd` — extra shield damage.
    pub shield_damage: i32,
}

/// Mario's `Attack11` (neutral jab) primary hitbox — `dMarioMainMotion_Jab1`,
/// `relocData/202_MarioMainMotion.c`:
/// `ftMotionCommandMakeAttackColl(0, 0, 10, 2, 1, 0, 160, 0, 0, 0, 361, 50, 0,
/// 3, 0, 0, 0, 8)`.
pub const MARIO_JAB1_HITBOX: Hitbox = Hitbox {
    damage: 2,
    offset: Vec3::new(0.0, 0.0, 0.0),
    radius: 160.0 / 2.0,
    angle: 361,
    kb_scale: 50,
    kb_weight: 0,
    kb_base: 8,
    element: crate::combat::Element::Normal,
    shield_damage: 0,
};

/// The root-sphere hurtbox used when a fighter has no posed joints — half
/// of `dMarioMain_attr.map_coll`'s `150.0` width ([`crate::hurtbox`]).
pub const MARIO_HURTBOX_RADIUS: f32 = 150.0 / 2.0;

/// `FTCOMMON_DAMAGE_SAKURAI_*` — `ft/ftcommon.h`.
const SAKURAI_KNOCKBACK_LOW: f32 = 32.0;
const SAKURAI_ANGLE_LOW_GR_DEG: f32 = 0.0;
const SAKURAI_ANGLE_HIGH_GD_DEG: f32 = 42.5;
const SAKURAI_ANGLE_DEFAULT_AR_DEG: f32 = 43.0;
/// The motion-event angle sentinel meaning "use the Sakurai angle" rather
/// than a literal degree value.
const SAKURAI_ANGLE_SENTINEL: i32 = 361;

/// `ftCommonDamageGetKnockbackAngle` @ `ftcommondamage.c:285`.
///
/// A literal `angle_i` is used as-is. The sentinel `361` means: 43 degrees if
/// the target is airborne, flat (0 degrees) if grounded and the knockback is
/// below 32, otherwise a near-vertical jump from 0 to the 42.5-degree cap
/// within about a tenth of a unit of knockback past 32 — this is the original
/// formula's own coarseness, not a bug introduced here.
pub fn sakurai_angle_radians(angle_i: i32, target_airborne: bool, knockback: f32) -> f32 {
    if angle_i != SAKURAI_ANGLE_SENTINEL {
        return (angle_i as f32).to_radians();
    }
    if target_airborne {
        return SAKURAI_ANGLE_DEFAULT_AR_DEG.to_radians();
    }
    if knockback < SAKURAI_KNOCKBACK_LOW {
        return SAKURAI_ANGLE_LOW_GR_DEG.to_radians();
    }
    let mut deg =
        ((knockback - SAKURAI_KNOCKBACK_LOW) / 0.099998474) * SAKURAI_ANGLE_HIGH_GD_DEG + 1.0;
    if deg > SAKURAI_ANGLE_HIGH_GD_DEG {
        deg = SAKURAI_ANGLE_HIGH_GD_DEG;
    }
    deg.to_radians()
}

/// `ftParamGetCommonKnockback` @ `ftparam.c:1451` for a hitbox, with
/// `recent_damage == 0`. A landed hit passes its own damage as
/// `recent_damage` instead ([`crate::combat::proc_params`]).
pub fn common_knockback(
    defender_damage_percent: u16,
    hitbox: &Hitbox,
    defender_weight: f32,
    attack_handicap: u8,
    defend_handicap: u8,
) -> f32 {
    knockback(
        defender_damage_percent,
        0,
        hitbox.damage,
        hitbox.kb_weight,
        hitbox.kb_scale,
        hitbox.kb_base,
        defender_weight,
        attack_handicap,
        defend_handicap,
    )
}

/// `ftParamGetCommonKnockback` @ `ftparam.c:1451`. The damage ratio is
/// Training's `100` ([`crate::stale::DAMAGE_RATIO_DEFAULT`]); the handicaps
/// index `dFTCommonDataHandicapTable` ([`crate::stale::HANDICAP_TABLE`]).
/// Throws pass the throw's own damage as `recent_damage`.
#[allow(clippy::too_many_arguments)]
pub fn knockback(
    percent_damage: u16,
    recent_damage: i32,
    hit_damage: i32,
    kb_weight: i32,
    kb_scale: i32,
    kb_base: i32,
    weight: f32,
    attack_handicap: u8,
    defend_handicap: u8,
) -> f32 {
    let scale = kb_scale as f32 * 0.01;
    let base = if kb_weight != 0 {
        (((1.0 + (10.0 * kb_weight as f32 * 0.05)) * weight * 1.4) + 18.0) * scale + kb_base as f32
    } else {
        let damage_add = percent_damage as f32 + recent_damage as f32;
        let hit_damage = hit_damage as f32;
        ((((damage_add * 0.1) + (damage_add * hit_damage * 0.05)) * weight * 1.4) + 18.0) * scale
            + kb_base as f32
    };
    let knockback = crate::stale::apply_ratio_and_handicap(
        base,
        crate::stale::DAMAGE_RATIO_DEFAULT,
        attack_handicap,
        defend_handicap,
    );
    knockback.min(2500.0)
}

/// `ftCommonDamageInitDamageVars` @ `ftcommondamage.c:473`, for callers that
/// already know the knockback and direction (throws, grab escapes, the cargo
/// stagger). It adds `damage` to the percent first and reads as a middle
/// (`N`) hit; see [`init_damage_vars_full`] for the rest.
pub fn init_damage_vars(
    f: &mut Fighter,
    status_replace: Option<AnyStatus>,
    damage: i32,
    knockback: f32,
    angle_i: i32,
    lr: f32,
    allow_losecopy: bool,
) {
    f.add_damage(damage);
    init_damage_vars_full(
        f,
        status_replace,
        damage,
        knockback,
        angle_i,
        lr,
        DAMAGE_INDEX_N,
        crate::combat::Element::Normal,
        allow_losecopy,
    );
}

/// `FTDamageColl::placement`: which column of the damage status tables a
/// hurtbox selects (`0` low, `1` middle, `2` high).
pub const DAMAGE_INDEX_LW: usize = 0;
pub const DAMAGE_INDEX_N: usize = 1;
pub const DAMAGE_INDEX_HI: usize = 2;

/// `F_CST_DTOR32(100.0F)`: a tumble whose velocity points more than 100
/// degrees away from the floor normal bounces off it.
const DAMAGE_BOUNCE_ANGLE: f32 = 1.745_329_3;
/// `F_CST_DTOR32(90.0F)`.
const DAMAGE_LAUNCH_ANGLE: f32 = core::f32::consts::FRAC_PI_2;
/// `FTCOMMON_DAMAGE_FIGHTER_FLYROLL_*`.
const FLYROLL_DAMAGE_MIN: u16 = 100;
const FLYROLL_RANDOM_CHANCE: f32 = 0.5;

/// [`init_damage_vars_full`] for a normal-element hit.
pub fn set_damage_status(
    f: &mut Fighter,
    status_replace: Option<AnyStatus>,
    knockback: f32,
    angle_i: i32,
    lr: f32,
    damage_index: usize,
) {
    init_damage_vars_full(
        f,
        status_replace,
        0,
        knockback,
        angle_i,
        lr,
        damage_index,
        crate::combat::Element::Normal,
        false,
    );
}

/// `syVectorAngleDiff3D`: the angle between two vectors.
fn angle_between(a: Vec3, b: Vec3) -> f32 {
    let la = a.length();
    let lb = b.length();
    if la == 0.0 || lb == 0.0 {
        return 0.0;
    }
    let c = ((a.x * b.x + a.y * b.y + a.z * b.z) / (la * lb)).clamp(-1.0, 1.0);
    ssb_engine::math::atan2(ssb_engine::math::sqrt(1.0 - c * c), c)
}

/// `ftCommonDamageInitDamageVars` @ `ftcommondamage.c:473`, without the
/// percent update. `allow_losecopy` is the source's last argument. The fighter turns to `lr` and enters the status from the
/// air or ground table (`damage_index` column, [`damage_status`]).
///
/// On the ground, knockback less than 90 degrees from the floor normal
/// launches the fighter with the ground-table status; a tumble always
/// launches, bouncing off the floor at 0.8 of its vertical speed when it
/// points more than 100 degrees into it; anything else slides along the
/// floor through `vel_damage_ground`. An airborne tumble between 70 and 110
/// degrees is `DamageFlyTop`; otherwise, at 100% or more, a coin flip on the
/// shared generator picks `DamageFlyRoll`. An electric hit first plays
/// `DamageE1`/`E2`, entering the real status when hitlag ends.
#[allow(clippy::too_many_arguments)]
pub fn init_damage_vars_full(
    f: &mut Fighter,
    status_replace: Option<AnyStatus>,
    damage: i32,
    knockback: f32,
    angle_i: i32,
    lr: f32,
    damage_index: usize,
    element: crate::combat::Element,
    allow_losecopy: bool,
) {
    if crate::map::is_cliff_hold(f.status.status) {
        crate::status::cliff_release_position(f);
    }
    let airborne = !f.is_grounded();
    let angle = sakurai_angle_radians(angle_i, airborne, knockback);
    let (sin, cos) = sin_cos(angle);
    let vel_x = cos * knockback;
    let vel_y = sin * knockback;
    let hitstun_f = hitstun_frames(knockback);
    let hitstun = (hitstun_f as i32).max(1) as u16;
    let level = if status_replace.is_some() {
        3
    } else {
        damage_level(hitstun_f)
    };
    let lr = if lr > 0.0 { 1.0 } else { -1.0 };
    f.facing = if lr > 0.0 {
        crate::fighter::Facing::Right
    } else {
        crate::fighter::Facing::Left
    };
    let vel = Vec3::new(-vel_x * lr, vel_y, 0.0);
    let mut launch = false;
    let (vel_damage, vel_damage_ground) = if airborne {
        (vel, 0.0)
    } else {
        let normal = f
            .floor
            .map(|floor| floor.normal)
            .unwrap_or(ssb_engine::math::Vec2::new(0.0, 1.0));
        let angle_diff = angle_between(Vec3::new(normal.x, normal.y, 0.0), vel);
        if angle_diff < DAMAGE_LAUNCH_ANGLE {
            launch = true;
            (vel, 0.0)
        } else if level == 3 {
            launch = true;
            if angle_diff > DAMAGE_BOUNCE_ANGLE {
                (Vec3::new(vel.x, -vel.y * 0.8, 0.0), 0.0)
            } else {
                (vel, 0.0)
            }
        } else {
            let ground = -vel_x * lr;
            (
                Vec3::new(normal.y * ground, -normal.x * ground, 0.0),
                ground,
            )
        }
    };
    let mut status = damage_status(level, airborne, damage_index);
    if launch {
        // `mpCommonSetFighterAir`.
        f.become_airborne();
        f.floor = None;
        f.physics.jumps_used = 1;
        f.pos.z = 0.0;
    }
    if level == 3 && !f.is_grounded() {
        if angle > FLYTOP_ANGLE_LOW && angle < FLYTOP_ANGLE_HIGH {
            status = Status::DamageFlyTop;
        } else if f.damage >= FLYROLL_DAMAGE_MIN && crate::rng::rand_float() < FLYROLL_RANDOM_CHANCE
        {
            status = Status::DamageFlyRoll;
        }
    }
    // `ftCommonDamageCheckElementSetColAnim` for a hit that did damage, then
    // `ftCommonDamageCheckMakeScreenFlash`.
    if damage != 0 {
        crate::colanim::damage_element_colanim(f, element, level);
    }
    if let Some(flash) = crate::colanim::damage_screen_flash(knockback, element) {
        f.screen_flash = Some(flash);
    }
    // `ftKirbySpecialNDamageCheckLoseCopy`: a tumble-level hit costs Kirby
    // its copy one time in twelve.
    if level == 3 && allow_losecopy {
        crate::kirby::damage_check_lose_copy(f);
    }
    let mut status_set: AnyStatus = status_replace.unwrap_or(status.into());
    let mut status_var = status_set;
    if element == crate::combat::Element::Electric {
        if let AnyStatus::Common(s) = status_set {
            if (Status::DamageHi1..=Status::WallDamage).contains(&s) {
                status_var = status_set;
                status_set = if level == 3 {
                    Status::DamageE2.into()
                } else {
                    Status::DamageE1.into()
                };
            }
        }
    }
    let timing = anim_timing(f, status_set);
    // `ftCommonDamageInitDamageVars`: `FTSTATUS_PRESERVE_DAMAGEPLAYER`.
    status::set_any_status_preserve(f, status_set, 0.0, timing, status::Preserve::DAMAGE_PLAYER);
    status::play_anim_events(f);
    f.reaction.is_knockback_over = knockback >= crate::reaction::KNOCKBACK_OVER;
    f.damage_e_status = if matches!(
        f.status.status,
        AnyStatus::Common(Status::DamageE1 | Status::DamageE2)
    ) {
        Some(status_var)
    } else {
        None
    };
    // `proc_passive`: `ftCommonDamageSetStatus` for an electric hit,
    // `ftCommonDamageCheckSetInvincible` otherwise.
    f.reaction.is_passive_invincible = f.damage_e_status.is_none();
    f.physics.vel_ground = Vec3::ZERO;
    f.physics.vel_air = Vec3::ZERO;
    f.physics.is_fastfall = false;
    f.physics.vel_knockback = vel_damage;
    f.physics.vel_damage_ground = vel_damage_ground;
    f.hitstun = hitstun;
    f.stick.tap_x = crate::status::STICKBUFFER_MAX;
    f.stick.tap_y = crate::status::STICKBUFFER_MAX;
    f.damage_knockback_stack = knockback;
    f.tics_since_last_z = crate::status::ZTRIGLAST_TICS_MAX;
    f.is_smash_di = true;
    f.reaction.coll_mask_curr = 0;
}

/// A status's figatree length as its timing.
fn anim_timing(f: &Fighter, status: AnyStatus) -> StatusTiming {
    match crate::motion::anim_length(f.kind, status) {
        Some(len) => StatusTiming::frames(len),
        None => StatusTiming::unknown(),
    }
}

/// `ftCommonDamageSetStatus`, the `proc_passive` an electric hit leaves: once
/// hitlag is over, `DamageE1`/`E2` hands over to the status the hit chose.
pub fn update_damage_e(f: &mut Fighter) {
    if f.hitlag > 0 {
        return;
    }
    let Some(status) = f.damage_e_status.take() else {
        return;
    };
    // `ftMainSetStatus` keeps the damage velocity and hitstun (status
    // variables), and clears `damage_knockback_stack`.
    let (vel, ground, hitstun) = (
        f.physics.vel_knockback,
        f.physics.vel_damage_ground,
        f.hitstun,
    );
    let timing = anim_timing(f, status);
    status::set_any_status_preserve(f, status, 0.0, timing, status::Preserve::DAMAGE_PLAYER);
    status::play_anim_events(f);
    f.physics.vel_knockback = vel;
    f.physics.vel_damage_ground = ground;
    f.hitstun = hitstun;
    if f.reaction.is_knockback_over {
        f.reaction.is_knockback_over = false;
        crate::reaction::set_timed_invincible(f, 1);
    }
}

/// `ftParamGetHitStun` @ `ftparam.c:1505`.
pub fn hitstun_frames(knockback: f32) -> f32 {
    knockback / 1.875
}

/// `ftParamGetHitLag` @ `ftparam.c:1511` (US), with `hitlag_mul == 1`: the
/// frames a hit freezes a fighter for, taken from the damage and the status
/// the fighter was in when it landed. A crouch keeps two thirds.
pub fn hitlag_frames(damage: i32, status: AnyStatus) -> u16 {
    let mut tics = ((damage as f32 * (1.0 / 3.0)) + 5.0) as i32;
    if matches!(status, AnyStatus::Common(Status::Squat | Status::SquatWait)) {
        tics = (tics as f32 * (2.0 / 3.0)) as i32;
    }
    tics.max(0) as u16
}

/// `this_fp->damage_lr = (defender.x < attacker.x) ? +1 : -1` —
/// `ftmain.c:2860`. Which way the hit pushes: away from the attacker, by
/// relative position, not by the attacker's facing.
pub fn damage_lr(defender_pos: Vec3, attacker_pos: Vec3) -> f32 {
    if defender_pos.x < attacker_pos.x {
        1.0
    } else {
        -1.0
    }
}

/// `FTCOMMON_DAMAGE_LEVEL_HITSTUN_*` — `ft/ftcommon.h`. Which of the four
/// damage tiers (`DamageX1`/`X2`/`X3`/tumble) a hit's hitstun falls into —
/// `ftCommonDamageGetDamageLevel` @ `ftcommondamage.c:314`.
const DAMAGE_LEVEL_HITSTUN_LOW: f32 = 12.0;
const DAMAGE_LEVEL_HITSTUN_MID: f32 = 24.0;
const DAMAGE_LEVEL_HITSTUN_HIGH: f32 = 32.0;

/// `ftCommonDamageGetDamageLevel` @ `ftcommondamage.c:314`. `3` is the
/// "tumble" tier: the hit always launches the defender airborne regardless of
/// where it landed.
pub fn damage_level(hitstun: f32) -> u8 {
    if hitstun < DAMAGE_LEVEL_HITSTUN_LOW {
        0
    } else if hitstun < DAMAGE_LEVEL_HITSTUN_MID {
        1
    } else if hitstun < DAMAGE_LEVEL_HITSTUN_HIGH {
        2
    } else {
        3
    }
}

/// `FTCOMMON_DAMAGE_FIGHTER_FLYTOP_ANGLE_{LOW,HIGH}` — `ft/ftcommon.h`.
const FLYTOP_ANGLE_LOW: f32 = 1.221_730_6; // 70 degrees
const FLYTOP_ANGLE_HIGH: f32 = 1.919_862_2; // 110 degrees

/// `dFTCommonDamageStatusGroundIDs` / `dFTCommonDamageStatusAirIDs`
/// (`ftcommondamage.c:13`), indexed by damage level and the hurtbox's
/// placement column. The `DamageFlyTop` override is
/// [`set_damage_status`]'s.
pub fn damage_status(level: u8, defender_was_airborne: bool, damage_index: usize) -> Status {
    const GROUND: [[Status; 3]; 4] = [
        [Status::DamageLw1, Status::DamageN1, Status::DamageHi1],
        [Status::DamageLw2, Status::DamageN2, Status::DamageHi2],
        [Status::DamageLw3, Status::DamageN3, Status::DamageHi3],
        [Status::DamageFlyLw, Status::DamageFlyN, Status::DamageFlyHi],
    ];
    const AIR: [[Status; 3]; 4] = [
        [Status::DamageAir1; 3],
        [Status::DamageAir2; 3],
        [Status::DamageAir3; 3],
        [Status::DamageFlyLw, Status::DamageFlyN, Status::DamageFlyHi],
    ];
    let table = if defender_was_airborne { &AIR } else { &GROUND };
    table[usize::from(level.min(3))][damage_index.min(2)]
}

/// The outcome of a hit landing, ready to apply to the defending
/// [`crate::fighter::Fighter`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HitResult {
    pub damage: i32,
    pub knockback_vel: Vec3,
    pub hitstun: u16,
    /// The Damage-family status the defender enters — see [`damage_status`].
    pub status: Status,
}

/// `ftCommonDamageInitDamageVars`'s knockback-vector construction
/// (`ftcommondamage.c:466`), for the grounded, non-launching case (module
/// docs: no ground/air damage-velocity split exists yet, so this always
/// writes to [`crate::physics::PhysicsState::vel_knockback`]).
#[allow(clippy::too_many_arguments)]
pub fn resolve_hit(
    hitbox: &Hitbox,
    attacker_pos: Vec3,
    defender_pos: Vec3,
    defender_damage_percent: u16,
    defender_weight: f32,
    defender_airborne: bool,
    attack_handicap: u8,
    defend_handicap: u8,
) -> HitResult {
    let knockback = common_knockback(
        defender_damage_percent,
        hitbox,
        defender_weight,
        attack_handicap,
        defend_handicap,
    );
    resolve_hit_with_knockback(
        hitbox,
        attacker_pos,
        defender_pos,
        knockback,
        defender_airborne,
    )
}

/// [`resolve_hit`] for a knockback already computed, such as one reduced by
/// `knockback_resist_status`.
pub fn resolve_hit_with_knockback(
    hitbox: &Hitbox,
    attacker_pos: Vec3,
    defender_pos: Vec3,
    knockback: f32,
    defender_airborne: bool,
) -> HitResult {
    let lr = damage_lr(defender_pos, attacker_pos);
    let angle = sakurai_angle_radians(hitbox.angle, defender_airborne, knockback);
    let (sin, cos) = sin_cos(angle);
    let vel_x = cos * knockback;
    let vel_y = sin * knockback;
    let hitstun_f = hitstun_frames(knockback);
    let level = damage_level(hitstun_f);
    let status = if level == 3 && angle > FLYTOP_ANGLE_LOW && angle < FLYTOP_ANGLE_HIGH {
        Status::DamageFlyTop
    } else {
        damage_status(level, defender_airborne, DAMAGE_INDEX_N)
    };
    HitResult {
        damage: hitbox.damage,
        knockback_vel: Vec3::new(-vel_x * lr, vel_y, 0.0),
        hitstun: (hitstun_f as u16).max(1),
        status,
    }
}

/// A swept-free sphere-vs-sphere overlap test —
/// `gmCollisionCheckAttackInFighterRange`'s shape, without the swept
/// previous-position term (module docs).
pub fn spheres_overlap(a_pos: Vec3, a_radius: f32, b_pos: Vec3, b_radius: f32) -> bool {
    let d = a_pos - b_pos;
    let r = a_radius + b_radius;
    d.length_squared() <= r * r
}

/// One attacker against one defender for one frame, through the whole hit
/// pipeline ([`crate::combat`]): attack positions, both searches, the hit
/// logs and both fighters' `ftMainProcParams`. Returns whether the
/// attacker's attack landed (`proc_hit`, which has already run). The match
/// loop drives [`crate::combat`] directly so weapons join the same frame.
pub fn apply_hit_from(attacker: &mut Fighter, defender: &mut Fighter) -> bool {
    crate::combat::resolve_frame(&mut [attacker, defender])[0]
}

/// What one hitbox did to a defender.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HitOutcome {
    /// No contact: the box may still connect later.
    Missed,
    /// The shield took it.
    Shielded,
    /// A hurtbox was touched but the defender is invincible there.
    Touched,
    /// Damage registered (`ftMainCheckGetUpdateDamage`).
    Damaged,
}

impl HitOutcome {
    pub fn registered(self) -> bool {
        self != HitOutcome::Missed
    }

    pub fn of(contact: crate::combat::WeaponContact) -> Self {
        match contact {
            crate::combat::WeaponContact::Missed => HitOutcome::Missed,
            crate::combat::WeaponContact::Shielded(_) => HitOutcome::Shielded,
            crate::combat::WeaponContact::Hurt(false) => HitOutcome::Touched,
            crate::combat::WeaponContact::Hurt(true) => HitOutcome::Damaged,
        }
    }
}

/// `FTCOMMON_DAMAGE_CATCH_RELEASE_THRESHOLD`: a held fighter whose queued
/// damage reaches this is knocked out of the hold.
pub const CATCH_RELEASE_THRESHOLD: i32 = 6;

/// `ftCommonDamageCheckCaptureKeepHold` @ 0x80140EC0.
pub fn capture_keep_hold(damage_queue: i32) -> bool {
    damage_queue < CATCH_RELEASE_THRESHOLD
}

/// `ftParamGetCapturedDamage` @ 0x800EA40C: a held fighter takes half,
/// rounded up, then `damage_mul` (0.5 while knocked down).
pub fn captured_damage(defender: &Fighter, damage: i32) -> i32 {
    let mut damage = damage;
    if defender.grab.capture.is_some() {
        damage = (damage as f32 * 0.5 + 0.999) as i32;
    }
    (damage as f32 * defender.damage_mul + 0.999) as i32
}

/// Which way a registered hit pushes the defender (`damage_lr`) and which
/// way a shield it lands on slides (`shield_lr`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HitDirection {
    pub damage_lr: f32,
    pub shield_lr: f32,
}

impl HitDirection {
    /// A fighter attack: both by relative position (`ftmain.c:2860`,
    /// `ftMainUpdateShieldStatFighter`).
    pub fn from_position(defender_pos: Vec3, attacker_pos: Vec3) -> Self {
        let lr = damage_lr(defender_pos, attacker_pos);
        HitDirection {
            damage_lr: lr,
            shield_lr: lr,
        }
    }

    /// A weapon: `ftMainProcessHitCollisionStatsMain` takes the side from
    /// the weapon's travel unless it is nearly still (`|vel.x| < 5`), and
    /// `ftMainUpdateShieldStatWeapon` always from its travel.
    pub fn from_weapon(defender_pos: Vec3, weapon_pos: Vec3, weapon_vel_x: f32) -> Self {
        let travel_lr = if weapon_vel_x < 0.0 { 1.0 } else { -1.0 };
        let damage_lr = if weapon_vel_x.abs() < 5.0 {
            damage_lr(defender_pos, weapon_pos)
        } else {
            travel_lr
        };
        HitDirection {
            damage_lr,
            shield_lr: travel_lr,
        }
    }
}

/// Registers a positioned hitbox (a weapon, or a host test's stand-in) in
/// the defender's frame without resolving it: the damage lands in the
/// defender's `ftMainProcParams` ([`crate::combat::proc_params`]).
pub fn register_hitbox(
    hitbox: &Hitbox,
    pos_curr: Vec3,
    pos_prev: Vec3,
    source: crate::combat::HitSource,
    attack_handicap: u8,
    defender: &mut Fighter,
) -> HitOutcome {
    HitOutcome::of(register_hitbox_contact(
        hitbox,
        pos_curr,
        pos_prev,
        source,
        attack_handicap,
        defender,
    ))
}

/// [`register_hitbox`], keeping the shield's hop data.
pub fn register_hitbox_contact(
    hitbox: &Hitbox,
    pos_curr: Vec3,
    pos_prev: Vec3,
    source: crate::combat::HitSource,
    attack_handicap: u8,
    defender: &mut Fighter,
) -> crate::combat::WeaponContact {
    crate::combat::weapon_hit(
        defender,
        crate::combat::WeaponAttack {
            hitbox: *hitbox,
            pos_curr,
            pos_prev,
            source,
            handicap: attack_handicap,
            can_shield: true,
            owner: None,
        },
    )
}

/// Applies one already-positioned hitbox to a defender at once: registers
/// it, then runs the defender's hit processing and `ftMainProcParams`. For
/// callers outside the frame pipeline (grab releases, host tests).
pub fn apply_hitbox_at(
    hitbox: &Hitbox,
    attacker_pos: Vec3,
    attack_handicap: u8,
    defender: &mut Fighter,
) -> HitOutcome {
    let outcome = register_hitbox(
        hitbox,
        attacker_pos,
        attacker_pos,
        crate::combat::HitSource::Position,
        attack_handicap,
        defender,
    );
    crate::combat::process_hit_collision(defender);
    crate::combat::proc_params(defender);
    outcome
}

/// Whether a hit landing on this status meets the shield — the statuses in
/// which `fp->is_shield` is set.
pub fn is_shielding(status: AnyStatus) -> bool {
    matches!(
        status,
        AnyStatus::Common(Status::GuardOn | Status::Guard | Status::GuardSetOff)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Mario's jab at 0% is the reference case: `dMarioMain_attr.weight ==
    /// 1.0`, so `common_knockback`'s weight term drops out and the whole
    /// formula reduces to arithmetic anyone can check by hand.
    #[test]
    fn jab_knockback_at_zero_percent_matches_the_formula_by_hand() {
        // ((0*1.4)+18) * (50*0.01) + 8 == 18*0.5+8 == 17.0
        let kb = common_knockback(0, &MARIO_JAB1_HITBOX, 1.0, 8, 8);
        assert_eq!(kb, 17.0);
    }

    /// Training's default handicap 9 (`{ 1.09, 0.9174312 }`) leaves this
    /// value exact, which is why the golden scenes whose only hit is this
    /// jab did not move when handicaps arrived (RE-341).
    #[test]
    fn jab_knockback_at_zero_percent_survives_training_handicaps() {
        let hc = crate::stale::HANDICAP_DEFAULT;
        assert_eq!(common_knockback(0, &MARIO_JAB1_HITBOX, 1.0, hc, hc), 17.0);
    }

    #[test]
    fn hitstun_divides_knockback_by_1_875() {
        assert_eq!(hitstun_frames(17.0), 17.0 / 1.875);
    }

    /// 17.0 knockback is below `SAKURAI_KNOCKBACK_LOW` (32.0), so a grounded
    /// target is launched dead flat — the real "jabs don't launch you" feel.
    #[test]
    fn low_knockback_grounded_sakurai_angle_is_flat() {
        let angle = sakurai_angle_radians(361, false, 17.0);
        assert_eq!(angle, 0.0);
    }

    #[test]
    fn airborne_sakurai_angle_is_43_degrees() {
        let angle = sakurai_angle_radians(361, true, 17.0);
        assert!((angle - 43.0f32.to_radians()).abs() < 1e-6);
    }

    #[test]
    fn a_literal_angle_bypasses_sakurai_entirely() {
        let angle = sakurai_angle_radians(90, false, 5.0);
        assert!((angle - 90.0f32.to_radians()).abs() < 1e-6);
    }

    #[test]
    fn high_knockback_grounded_sakurai_angle_saturates_near_instantly() {
        // The original's own coarseness (module docs): a hair past 32
        // knockback already reads as the 42.5-degree cap.
        let angle = sakurai_angle_radians(361, false, 33.0);
        assert!((angle - SAKURAI_ANGLE_HIGH_GD_DEG.to_radians()).abs() < 1e-6);
    }

    #[test]
    fn defender_left_of_attacker_is_pushed_further_left() {
        let lr = damage_lr(Vec3::new(-10.0, 0.0, 0.0), Vec3::new(0.0, 0.0, 0.0));
        assert_eq!(lr, 1.0);
    }

    #[test]
    fn defender_right_of_attacker_is_pushed_further_right() {
        let lr = damage_lr(Vec3::new(10.0, 0.0, 0.0), Vec3::new(0.0, 0.0, 0.0));
        assert_eq!(lr, -1.0);
    }

    #[test]
    fn resolve_hit_pushes_the_defender_away_from_the_attacker() {
        let attacker = Vec3::new(0.0, 0.0, 0.0);
        let defender = Vec3::new(10.0, 0.0, 0.0);
        let result = resolve_hit(&MARIO_JAB1_HITBOX, attacker, defender, 0, 1.0, false, 8, 8);
        assert_eq!(result.damage, 2);
        // Flat, grounded, low knockback: pure +X push away from the attacker.
        assert!(result.knockback_vel.x > 0.0);
        assert_eq!(result.knockback_vel.y, 0.0);
        assert_eq!(result.hitstun, 9); // floor(17.0 / 1.875) == 9
                                       // hitstun 9 < DAMAGE_LEVEL_HITSTUN_LOW (12): level 0, grounded -> N1.
        assert_eq!(result.status, Status::DamageN1);
    }

    /// Enters `status` and runs its motion script `frames` frames in, as
    /// the status machine would.
    fn at_frame(f: &mut Fighter, status: AnyStatus, frames: u32) {
        status::set_any_status(f, status, 0.0, StatusTiming::unknown());
        for _ in 0..frames {
            f.status.anim_frame += 1.0;
            crate::motion::advance(f);
        }
    }

    #[test]
    fn jab_uses_the_posed_hand_joint() {
        use crate::fighter::{FighterKind, JointTransform};
        let mut attacker = Fighter::new(FighterKind::Mario, 0, 3);
        let mut defender = Fighter::new(FighterKind::Mario, 1, 3);
        at_frame(&mut attacker, Status::Attack11.into(), 2);
        attacker.joint_transforms[10] = Some(JointTransform {
            axes: [
                Vec3::new(1.0, 0.0, 0.0),
                Vec3::new(0.0, 1.0, 0.0),
                Vec3::new(0.0, 0.0, 1.0),
            ],
            origin: Vec3::new(500.0, 0.0, 0.0),
        });
        defender.pos = Vec3::new(500.0, 0.0, 0.0);
        apply_hit_from(&mut attacker, &mut defender);
        assert_eq!(defender.damage, 2);
    }

    #[test]
    fn spheres_overlap_at_exactly_the_combined_radius() {
        let a = Vec3::new(0.0, 0.0, 0.0);
        let b = Vec3::new(10.0, 0.0, 0.0);
        assert!(spheres_overlap(a, 5.0, b, 5.0));
        assert!(!spheres_overlap(a, 4.0, b, 5.0));
    }

    #[test]
    fn damage_level_thresholds_match_ftcommon_h() {
        assert_eq!(damage_level(11.999), 0);
        assert_eq!(damage_level(12.0), 1);
        assert_eq!(damage_level(23.999), 1);
        assert_eq!(damage_level(24.0), 2);
        assert_eq!(damage_level(31.999), 2);
        assert_eq!(damage_level(32.0), 3);
    }

    #[test]
    fn damage_status_picks_the_table_and_the_placement_column() {
        assert_eq!(damage_status(0, false, DAMAGE_INDEX_N), Status::DamageN1);
        assert_eq!(damage_status(1, false, DAMAGE_INDEX_N), Status::DamageN2);
        assert_eq!(damage_status(2, false, DAMAGE_INDEX_N), Status::DamageN3);
        assert_eq!(damage_status(0, false, DAMAGE_INDEX_HI), Status::DamageHi1);
        assert_eq!(damage_status(2, false, DAMAGE_INDEX_LW), Status::DamageLw3);
        assert_eq!(damage_status(0, true, DAMAGE_INDEX_HI), Status::DamageAir1);
        assert_eq!(damage_status(2, true, DAMAGE_INDEX_LW), Status::DamageAir3);
        assert_eq!(damage_status(3, false, DAMAGE_INDEX_N), Status::DamageFlyN);
        assert_eq!(damage_status(3, true, DAMAGE_INDEX_HI), Status::DamageFlyHi);
        assert_eq!(damage_status(3, true, DAMAGE_INDEX_LW), Status::DamageFlyLw);
    }

    fn grounded_mario() -> Fighter {
        let mut f = Fighter::new(crate::fighter::FighterKind::Mario, 1, 3);
        f.situation = crate::fighter::Situation::Ground;
        f.floor = Some(crate::ground::Standing {
            line: 0,
            flags: 0,
            normal: ssb_engine::math::Vec2::new(0.0, 1.0),
        });
        f
    }

    #[test]
    fn a_tumble_is_flytop_only_within_the_near_vertical_window() {
        for (angle, expected) in [
            (90, Status::DamageFlyTop),
            (69, Status::DamageFlyN),
            (111, Status::DamageFlyN),
        ] {
            let mut f = grounded_mario();
            set_damage_status(&mut f, None, 100.0, angle, 1.0, DAMAGE_INDEX_N);
            assert_eq!(f.status.status, AnyStatus::Common(expected), "{angle}");
        }
    }

    #[test]
    fn a_flat_grounded_hit_slides_along_the_floor() {
        let mut f = grounded_mario();
        // Sakurai angle below 32 knockback is flat.
        set_damage_status(&mut f, None, 20.0, 361, 1.0, DAMAGE_INDEX_N);
        assert!(f.is_grounded());
        assert_eq!(f.status.status, AnyStatus::Common(Status::DamageN1));
        assert_eq!(f.physics.vel_damage_ground, -20.0);
        assert_eq!(f.physics.vel_knockback, Vec3::new(-20.0, 0.0, 0.0));
        assert_eq!(f.facing, crate::fighter::Facing::Right);
    }

    #[test]
    fn an_upward_grounded_hit_launches_with_its_ground_status() {
        let mut f = grounded_mario();
        f.physics.jumps_used = 0;
        set_damage_status(&mut f, None, 20.0, 45, -1.0, DAMAGE_INDEX_HI);
        assert!(!f.is_grounded(), "angle_diff < 90 launches");
        assert_eq!(f.status.status, AnyStatus::Common(Status::DamageHi1));
        assert_eq!(f.physics.jumps_used, 1);
        assert!(f.physics.vel_knockback.x > 0.0 && f.physics.vel_knockback.y > 0.0);
        assert_eq!(f.facing, crate::fighter::Facing::Left);
    }

    #[test]
    fn a_downward_grounded_tumble_bounces_off_the_floor() {
        let mut f = grounded_mario();
        set_damage_status(&mut f, None, 100.0, 270, 1.0, DAMAGE_INDEX_N);
        assert!(!f.is_grounded());
        assert!((f.physics.vel_knockback.y - 80.0).abs() < 1e-3);
    }

    /// End-to-end through `apply_hit_from`: a grounded jab at 0% enters
    /// `DamageN1`, not just a bare knockback push (module docs' formerly-open
    /// "no Damage status" gap).
    #[test]
    fn a_landed_jab_puts_the_defender_into_a_damage_status() {
        let mut attacker = Fighter::new(crate::fighter::FighterKind::Mario, 0, 3);
        let mut defender = Fighter::new(crate::fighter::FighterKind::Mario, 1, 3);
        attacker.pos = Vec3::new(0.0, 0.0, 0.0);
        defender.pos = Vec3::new(10.0, 0.0, 0.0);
        defender.situation = crate::fighter::Situation::Ground;
        at_frame(&mut attacker, Status::Attack11.into(), 2);

        assert!(apply_hit_from(&mut attacker, &mut defender));

        assert_eq!(defender.status.status, Status::DamageN1);
        assert!(defender.hitstun > 0);
        // The attack's record keeps it from hitting again.
        assert!(!apply_hit_from(&mut attacker, &mut defender));
        assert_eq!(defender.damage, 2);

        // Hitlag, then hitstun and the reaction clip, return the defender
        // to Wait.
        assert!(defender.hitlag > 0 && attacker.hitlag > 0);
        for _ in 0..60 {
            defender.tick_timers();
            if !defender.is_in_hitlag() {
                crate::status::update(&mut defender);
            }
        }
        assert_eq!(defender.status.status, Status::Wait);
    }

    /// A jab landing on a shielding defender blocks entirely: no damage, no
    /// hitstun, just shield-health loss and a `GuardSetOff` pushback —
    /// `is_shielding`/`apply_shield_hit`'s module docs.
    #[test]
    fn a_jab_landing_on_a_shield_pushes_back_instead_of_damaging() {
        let mut attacker = Fighter::new(crate::fighter::FighterKind::Mario, 0, 3);
        let mut defender = Fighter::new(crate::fighter::FighterKind::Mario, 1, 3);
        attacker.pos = Vec3::new(0.0, 0.0, 0.0);
        defender.pos = Vec3::new(10.0, 0.0, 0.0);
        defender.situation = crate::fighter::Situation::Ground;
        status::set_guard(&mut defender);
        let starting_health = defender.guard.shield_health;
        at_frame(&mut attacker, Status::Attack11.into(), 2);

        apply_hit_from(&mut attacker, &mut defender);

        assert_eq!(defender.status.status, Status::GuardSetOff);
        assert_eq!(defender.damage, 0);
        assert_eq!(defender.hitstun, 0);
        assert_eq!(
            defender.guard.shield_health,
            starting_health - MARIO_JAB1_HITBOX.damage as f32
        );
        assert_ne!(defender.physics.vel_ground.x, 0.0);
    }

    #[test]
    fn a_hit_breaking_the_shield_goes_straight_to_shield_break() {
        let mut defender = grounded_mario();
        status::set_guard(&mut defender);
        defender.guard.shield_health = 2.0;
        let outcome = apply_hitbox_at(&MARIO_JAB1_HITBOX, Vec3::ZERO, 9, &mut defender);
        assert_eq!(outcome, HitOutcome::Shielded);
        assert_eq!(defender.status.status, Status::ShieldBreakFly);
        assert_eq!(defender.guard.shield_health, 30.0);
    }

    /// `ftMainProcessHitCollisionStatsMain` passes the frame's
    /// `damage_queue`, which already holds this hit, as `recent_damage`.
    #[test]
    fn knockback_counts_the_hit_on_top_of_the_prior_percent() {
        let hitbox = Hitbox {
            damage: 10,
            radius: 50.0,
            offset: Vec3::ZERO,
            angle: 45,
            kb_scale: 100,
            kb_weight: 0,
            kb_base: 0,
            element: crate::combat::Element::Normal,
            shield_damage: 0,
        };
        let mut defender = grounded_mario();
        defender.damage = 50;
        apply_hitbox_at(&hitbox, Vec3::new(-10.0, 0.0, 0.0), 9, &mut defender);
        let expected = knockback(50, 10, 10, 0, 100, 0, 1.0, 9, 9);
        assert!(expected > knockback(50, 0, 10, 0, 100, 0, 1.0, 9, 9));
        assert_eq!(defender.damage, 60);
        let v = defender.physics.vel_knockback;
        assert!(((v.x * v.x + v.y * v.y).sqrt() - expected).abs() < 1e-2);
    }

    #[test]
    fn crouching_takes_two_thirds_of_the_knockback() {
        let hitbox = Hitbox {
            damage: 10,
            radius: 50.0,
            offset: Vec3::ZERO,
            angle: 0,
            kb_scale: 100,
            kb_weight: 0,
            kb_base: 0,
            element: crate::combat::Element::Normal,
            shield_damage: 0,
        };
        let mut defender = grounded_mario();
        status::set_status(
            &mut defender,
            Status::SquatWait,
            0.0,
            StatusTiming::unknown(),
        );
        apply_hitbox_at(&hitbox, Vec3::new(-10.0, 0.0, 0.0), 9, &mut defender);
        let expected = knockback(0, 10, 10, 0, 100, 0, 1.0, 9, 9) * 2.0 / 3.0;
        assert!((defender.physics.vel_knockback.x.abs() - expected).abs() < 1e-3);
    }

    #[test]
    fn hitlag_follows_the_damage_and_a_crouch_shortens_it() {
        // US: `(damage / 3 + 5)` frames.
        assert_eq!(hitlag_frames(2, Status::Wait.into()), 5);
        assert_eq!(hitlag_frames(12, Status::Wait.into()), 9);
        assert_eq!(hitlag_frames(12, Status::Squat.into()), 6);
        // An electric hit's 1.5x applies to the truncated count.
        assert_eq!(
            crate::combat::hitlag_frames(12, Status::Wait.into(), 1.5),
            13
        );
    }

    #[test]
    fn percent_stops_at_999() {
        let mut defender = grounded_mario();
        defender.damage = 995;
        apply_hitbox_at(&MARIO_JAB1_HITBOX, Vec3::ZERO, 9, &mut defender);
        defender.invincible_frames = 0;
        defender.add_damage(50);
        assert_eq!(defender.damage, 999);
    }

    #[test]
    fn a_fast_weapon_pushes_along_its_travel() {
        let behind = HitDirection::from_weapon(Vec3::ZERO, Vec3::new(20.0, 0.0, 0.0), 40.0);
        assert_eq!(behind.damage_lr, -1.0, "moving right pushes right");
        assert_eq!(behind.shield_lr, -1.0);
        let slow = HitDirection::from_weapon(Vec3::ZERO, Vec3::new(20.0, 0.0, 0.0), 4.0);
        assert_eq!(slow.damage_lr, 1.0, "a slow weapon pushes away from itself");
        assert_eq!(slow.shield_lr, -1.0);
    }

    #[test]
    fn an_air_hit_reaction_ends_in_fall_and_lands_without_leaving_it() {
        let mut f = Fighter::new(crate::fighter::FighterKind::Mario, 1, 3);
        set_damage_status(&mut f, None, 30.0, 0, 1.0, DAMAGE_INDEX_N);
        assert_eq!(f.status.status, Status::DamageAir2);
        assert!(!f.is_grounded());
        f.hitstun = 0;
        f.status.anim_frame = 1000.0;
        status::update(&mut f);
        assert_eq!(f.status.status, Status::Fall);

        let mut f = grounded_mario();
        set_damage_status(&mut f, None, 30.0, 0, 1.0, DAMAGE_INDEX_N);
        assert!(f.is_grounded());
        f.become_airborne();
        f.land(0.0);
        assert_eq!(f.status.status, Status::DamageN2);
    }

    /// `nGMHitStatusInvincible`: a post-respawn invincible defender takes no
    /// damage, but the attack still connects: its record is spent and the
    /// attacker takes hitlag.
    #[test]
    fn an_invincible_defender_is_touched_but_not_hurt() {
        let mut attacker = Fighter::new(crate::fighter::FighterKind::Mario, 0, 3);
        let mut defender = Fighter::new(crate::fighter::FighterKind::Mario, 1, 3);
        attacker.pos = Vec3::new(0.0, 0.0, 0.0);
        defender.pos = Vec3::new(10.0, 0.0, 0.0);
        defender.situation = crate::fighter::Situation::Ground;
        defender.invincible_frames = 10;
        at_frame(&mut attacker, Status::Attack11.into(), 2);

        assert!(apply_hit_from(&mut attacker, &mut defender));

        assert_ne!(defender.status.status, Status::DamageN1);
        assert_eq!(defender.damage, 0);
        assert!(attacker.hitlag > 0);
        assert!(attacker.attack_colls[0].records[0].is_interact_hurt);
        // An intangible defender is not touched at all.
        let mut attacker = Fighter::new(crate::fighter::FighterKind::Mario, 0, 3);
        let mut defender = Fighter::new(crate::fighter::FighterKind::Mario, 1, 3);
        defender.pos = Vec3::new(10.0, 0.0, 0.0);
        defender.intangible_frames = 10;
        at_frame(&mut attacker, Status::Attack11.into(), 2);
        assert!(!apply_hit_from(&mut attacker, &mut defender));
        assert_eq!(attacker.hitlag, 0);
    }
}
