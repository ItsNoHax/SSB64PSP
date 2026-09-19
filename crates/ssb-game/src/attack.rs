//! Hitboxes, hurtboxes and hit resolution — Training Mode's first real
//! grounded attack, `F1` acceptance criterion 5.
//!
//! Ported from three places in the decompilation:
//!
//! * `relocData/202_MarioMainMotion.c`'s `dMarioMainMotion_Jab1` — the literal
//!   `ftMotionCommandMakeAttackColl(...)` arguments for Mario's neutral jab
//!   (`Attack11`), which is where a Smash 64 hitbox's numbers actually live.
//!   There is no separate per-character "attack table"; the hitbox is a
//!   motion-event argument list baked into the animation script.
//! * `ft/ftcommon/ftcommondamage.c`'s `ftCommonDamageGetKnockbackAngle` and
//!   `ftCommonDamageInitDamageVars` — the "Sakurai angle" resolution and the
//!   knockback-vector construction.
//! * `ft/ftparam.c`'s `ftParamGetCommonKnockback` and `ftParamGetHitStun` —
//!   the knockback magnitude and hitstun-length formulas.
//!
//! ## What is simplified here, and why
//!
//! * **One hitbox, not five.** `Jab1` actually spawns two hitboxes (joints 9
//!   and 10, the fist and forearm). Only the primary one (joint 10, the arg
//!   set `MakeAttackColl(0, ...)`) is ported; the secondary is a documented
//!   gap, not a guess, until per-bone attachment exists.
//! * **A sphere hurtbox, not a per-bone capsule set.** The original tests a
//!   hitbox sphere against eleven `FTDamageColl` capsules per fighter
//!   (`gmCollisionCheckFighterInFighterRange`). No per-bone hurtbox system
//!   exists yet, so [`MARIO_HURTBOX_RADIUS`] stands in with a single sphere at
//!   the fighter's root — sized from Mario's own real collision-diamond width
//!   (`dMarioMain_attr.map_coll = { 320, 190, 0, 150 }`, so half of `150`),
//!   not an invented number.
//! * **No hitlag, no handicap, no damage-ratio scaling, no `recent_damage`
//!   accumulation.** `ftParamGetCommonKnockback`'s handicap and damage-ratio
//!   terms are fixed at their singleplayer-neutral value of `1.0`, and
//!   `recent_damage` (a short-window stale-move accumulator) is treated as
//!   always zero. None of those systems exist yet; wiring real values in
//!   later must not silently change these formulas' shape.
//! * **Knockback decay, not a friction curve.** The original decelerates
//!   `vel_damage_ground` by friction every frame the way normal ground
//!   movement does. Until that exists, [`crate::fighter::Fighter::tick_timers`]
//!   holds knockback constant for the hit's hitstun duration and then snaps it
//!   to zero the frame hitstun ends, rather than bleeding it off gradually.
//! * **`Damage` status entered, but not fully.** A landed hit now moves the
//!   defender into the real `DamageHi/N/Lw1-3`/`DamageAir1-3`/`DamageFlyN`/
//!   `FlyTop` status (`damage_status`), matching `ftCommonDamageGetDamageLevel`'s
//!   hitstun tiers and the defender's ground/air situation. Still missing:
//!   the hit-location Hi/Lw index (every hit reads as "N", the middle
//!   column — no system yet computes where on the target's body a hitbox
//!   landed), `DamageFlyRoll` (its selection is a coin flip, and there is no
//!   shared RNG source to drive it), and the "a shallow hit's upward
//!   knockback launches the target airborne anyway" angle branch (dropped —
//!   see [`crate::status::Status::is_grounded`]'s doc comment). No animation
//!   plays for any of these yet — `Status::anim_slot`'s catch-all keeps the
//!   current pose, same as `Attack11`.

use ssb_engine::math::{sin_cos, Vec3};

use crate::fighter::Fighter;
use crate::status::{self, Status, StatusTiming};

/// A hitbox descriptor, transcribed field-for-field from a
/// `ftMotionCommandMakeAttackColl(aid, gid, jid, dmg, reb, elem, sz, ox, oy,
/// oz, ang, kbs, kbw, ga, sd, fl, fk, kbb)` call, so it can be checked against
/// the decomp source by eye. Fields the ported hit-resolution path does not
/// use yet (rebound, element, shield damage, sound) are not carried.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hitbox {
    /// `dmg` — base damage, added to the target's percent as-is.
    pub damage: i32,
    /// `ox, oy, oz` — offset from the attachment joint. `(0, 0, 0)` for
    /// [`MARIO_JAB1_HITBOX`], so the missing joint attachment (see module
    /// docs) is not yet observable.
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
};

/// The frame window `Jab1`'s hitbox is active — `WaitAsync(2)` before
/// `MakeAttackColl`, then `Wait(2)` before `ClearAttackCollAll`. Half-open:
/// active for `anim_frame` in `2.0..4.0`.
pub const MARIO_JAB1_HITBOX_START: f32 = 2.0;
pub const MARIO_JAB1_HITBOX_END: f32 = 4.0;

/// `Attack11`'s total length in frames, summed from `dMarioMainMotion_Jab1`'s
/// own `WaitAsync`/`Wait` commands (`2 + 2 + 10`). Not extracted from a
/// figatree file — there is no ported animation for this status yet (module
/// docs) — but a real number read off the motion script, not a guess.
pub const MARIO_ATTACK11_LENGTH_FRAMES: f32 = 14.0;

/// Mario's hurtbox radius stand-in — half of `dMarioMain_attr.map_coll`'s
/// `150.0` width. See the module docs' "sphere hurtbox" simplification.
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

/// `ftParamGetCommonKnockback` @ `ftparam.c:1451`, restricted to
/// singleplayer-neutral handicap/damage-ratio terms and `recent_damage == 0`
/// (module docs).
pub fn common_knockback(
    defender_damage_percent: u16,
    hitbox: &Hitbox,
    defender_weight: f32,
) -> f32 {
    let scale = hitbox.kb_scale as f32 * 0.01;
    let knockback = if hitbox.kb_weight != 0 {
        (((1.0 + (10.0 * hitbox.kb_weight as f32 * 0.05)) * defender_weight * 1.4) + 18.0) * scale
            + hitbox.kb_base as f32
    } else {
        let damage_add = defender_damage_percent as f32;
        let hit_damage = hitbox.damage as f32;
        ((((damage_add * 0.1) + (damage_add * hit_damage * 0.05)) * defender_weight * 1.4) + 18.0)
            * scale
            + hitbox.kb_base as f32
    };
    knockback.min(2500.0)
}

/// `ftParamGetHitStun` @ `ftparam.c:1505`.
pub fn hitstun_frames(knockback: f32) -> f32 {
    knockback / 1.875
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

/// `ftCommonDamageInitDamageVars`'s status-table lookup
/// (`ftcommondamage.c:473`), restricted to the `damage_index == N` ("hit the
/// middle of the target") column — no hit-location-relative Hi/Lw index
/// exists yet, so every hit reads as a middle hit — and dropping the
/// ground-hit-still-launches-airborne branch (see [`Status::is_grounded`]'s
/// docs). `DamageFlyRoll`'s random branch is not ported: it needs a shared
/// RNG source this module does not have, so a tumble always reads as
/// `DamageFlyN`/`DamageFlyTop`.
pub fn damage_status(level: u8, defender_was_airborne: bool, angle: f32) -> Status {
    if level == 3 {
        return if angle > FLYTOP_ANGLE_LOW && angle < FLYTOP_ANGLE_HIGH {
            Status::DamageFlyTop
        } else {
            Status::DamageFlyN
        };
    }
    let table = if defender_was_airborne {
        [Status::DamageAir1, Status::DamageAir2, Status::DamageAir3]
    } else {
        [Status::DamageN1, Status::DamageN2, Status::DamageN3]
    };
    table[level as usize]
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
pub fn resolve_hit(
    hitbox: &Hitbox,
    attacker_pos: Vec3,
    defender_pos: Vec3,
    defender_damage_percent: u16,
    defender_weight: f32,
    defender_airborne: bool,
) -> HitResult {
    let lr = damage_lr(defender_pos, attacker_pos);
    let knockback = common_knockback(defender_damage_percent, hitbox, defender_weight);
    let angle = sakurai_angle_radians(hitbox.angle, defender_airborne, knockback);
    let (sin, cos) = sin_cos(angle);
    let vel_x = cos * knockback;
    let vel_y = sin * knockback;
    let hitstun_f = hitstun_frames(knockback);
    let level = damage_level(hitstun_f);
    HitResult {
        damage: hitbox.damage,
        knockback_vel: Vec3::new(-vel_x * lr, vel_y, 0.0),
        hitstun: (hitstun_f as u16).max(1),
        status: damage_status(level, defender_airborne, angle),
    }
}

/// Whether `anim_frame` falls within `Jab1`'s active hitbox window.
pub fn jab1_hitbox_active(anim_frame: f32) -> bool {
    (MARIO_JAB1_HITBOX_START..MARIO_JAB1_HITBOX_END).contains(&anim_frame)
}

/// A swept-free sphere-vs-sphere overlap test —
/// `gmCollisionCheckAttackInFighterRange`'s shape, without the swept
/// previous-position term (module docs).
pub fn spheres_overlap(a_pos: Vec3, a_radius: f32, b_pos: Vec3, b_radius: f32) -> bool {
    let d = a_pos - b_pos;
    let r = a_radius + b_radius;
    d.length_squared() <= r * r
}

/// `F1` criterion 5: tests `attacker`'s active hitbox against `defender` and
/// applies the hit. `hit_by_current_attack` is the caller's per-target
/// hit-suppression state — the simplified stand-in for the original's
/// per-attack `GMAttackRecord` hit list (module docs) — cleared as soon as
/// the attacker leaves `Attack11` so the next jab can hit again.
///
/// Only `Attack11`'s hitbox is ported (module docs), so anything else the
/// attacker is doing is a no-op call.
pub fn apply_hit_from(
    attacker: &Fighter,
    defender: &mut Fighter,
    hit_by_current_attack: &mut bool,
) {
    if attacker.status.status != Status::Attack11 {
        *hit_by_current_attack = false;
        return;
    }
    if *hit_by_current_attack {
        return;
    }
    if !jab1_hitbox_active(attacker.status.anim_frame) {
        return;
    }
    let hitbox = MARIO_JAB1_HITBOX;
    let hitbox_pos = attacker.pos + hitbox.offset;
    if !spheres_overlap(
        hitbox_pos,
        hitbox.radius,
        defender.pos,
        MARIO_HURTBOX_RADIUS,
    ) {
        return;
    }
    if is_shielding(defender.status.status) {
        apply_shield_hit(&hitbox, attacker, defender);
        *hit_by_current_attack = true;
        return;
    }
    let result = resolve_hit(
        &hitbox,
        attacker.pos,
        defender.pos,
        defender.damage,
        defender.attributes.weight,
        !defender.is_grounded(),
    );
    defender.damage = defender.damage.saturating_add(result.damage as u16);
    // `ftCommonDamageInitDamageVars` @ `ftcommondamage.c:557` zeroes the
    // fighter's normal ground/air velocity outright before writing the
    // knockback vector — a hit fully overrides existing movement rather than
    // adding to it. `status::set_status` runs first because it is what moves
    // `vel_ground` into `vel_air` on a ground-to-air transition, and that
    // transferred value must not survive the zeroing below.
    status::set_status(defender, result.status, 0.0, StatusTiming::unknown());
    defender.physics.vel_ground = Vec3::ZERO;
    defender.physics.vel_air = Vec3::ZERO;
    defender.physics.vel_knockback = result.knockback_vel;
    defender.hitstun = result.hitstun;
    *hit_by_current_attack = true;
}

/// Whether a hit landing on this status should be redirected into
/// [`apply_shield_hit`] rather than the normal Damage-family path — the
/// statuses `ftMainUpdateShieldStatFighter`'s caller treats as "currently
/// shielding" (`ftmain.c`'s hit-search gates a shield hit on `fp->is_shield`,
/// which these three statuses hold for the whole time they are active).
pub fn is_shielding(status: Status) -> bool {
    matches!(
        status,
        Status::GuardOn | Status::Guard | Status::GuardSetOff
    )
}

/// `ftMainUpdateShieldStatFighter` @ `ftmain.c:2059`, reduced to the
/// single-hit case (module docs: no `shield_damage_total` multi-hit
/// accumulation) — a hit landing on a shield deals no damage/knockback/
/// hitstun at all, only shield health loss and a `GuardSetOff` pushback.
pub fn apply_shield_hit(hitbox: &Hitbox, attacker: &Fighter, defender: &mut Fighter) {
    let shield_lr = damage_lr(defender.pos, attacker.pos);
    status::set_guard_set_off(defender, hitbox.damage as f32, shield_lr);
    defender.guard.shield_health -= hitbox.damage as f32;
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
        let kb = common_knockback(0, &MARIO_JAB1_HITBOX, 1.0);
        assert_eq!(kb, 17.0);
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
        let result = resolve_hit(&MARIO_JAB1_HITBOX, attacker, defender, 0, 1.0, false);
        assert_eq!(result.damage, 2);
        // Flat, grounded, low knockback: pure +X push away from the attacker.
        assert!(result.knockback_vel.x > 0.0);
        assert_eq!(result.knockback_vel.y, 0.0);
        assert_eq!(result.hitstun, 9); // floor(17.0 / 1.875) == 9
                                       // hitstun 9 < DAMAGE_LEVEL_HITSTUN_LOW (12): level 0, grounded -> N1.
        assert_eq!(result.status, Status::DamageN1);
    }

    #[test]
    fn hitbox_window_is_two_frames_starting_at_frame_two() {
        assert!(!jab1_hitbox_active(1.99));
        assert!(jab1_hitbox_active(2.0));
        assert!(jab1_hitbox_active(3.99));
        assert!(!jab1_hitbox_active(4.0));
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
    fn damage_status_picks_the_grounded_or_airborne_table_by_prior_situation() {
        assert_eq!(damage_status(0, false, 0.0), Status::DamageN1);
        assert_eq!(damage_status(1, false, 0.0), Status::DamageN2);
        assert_eq!(damage_status(2, false, 0.0), Status::DamageN3);
        assert_eq!(damage_status(0, true, 0.0), Status::DamageAir1);
        assert_eq!(damage_status(2, true, 0.0), Status::DamageAir3);
    }

    #[test]
    fn damage_status_tumble_is_flytop_only_within_the_near_vertical_window() {
        // Level 3 always tumbles airborne, regardless of prior situation.
        assert_eq!(damage_status(3, false, 0.0), Status::DamageFlyN);
        assert_eq!(
            damage_status(3, true, 90.0f32.to_radians()),
            Status::DamageFlyTop
        );
        assert_eq!(
            damage_status(3, true, 69.0f32.to_radians()),
            Status::DamageFlyN
        );
        assert_eq!(
            damage_status(3, true, 111.0f32.to_radians()),
            Status::DamageFlyN
        );
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
        status::set_status(
            &mut attacker,
            Status::Attack11,
            3.0,
            StatusTiming::unknown(),
        );

        let mut hit_by_current_attack = false;
        apply_hit_from(&attacker, &mut defender, &mut hit_by_current_attack);

        assert_eq!(defender.status.status, Status::DamageN1);
        assert!(hit_by_current_attack);
        assert!(defender.hitstun > 0);

        // Hitstun running out returns the defender to Wait.
        for _ in 0..defender.hitstun {
            defender.tick_timers();
        }
        crate::status::update(&mut defender);
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
        status::set_status(&mut defender, Status::Guard, 0.0, StatusTiming::unknown());
        let starting_health = defender.guard.shield_health;
        status::set_status(
            &mut attacker,
            Status::Attack11,
            3.0,
            StatusTiming::unknown(),
        );

        let mut hit_by_current_attack = false;
        apply_hit_from(&attacker, &mut defender, &mut hit_by_current_attack);

        assert_eq!(defender.status.status, Status::GuardSetOff);
        assert_eq!(defender.damage, 0);
        assert_eq!(defender.hitstun, 0);
        assert_eq!(
            defender.guard.shield_health,
            starting_health - MARIO_JAB1_HITBOX.damage as f32
        );
        assert_ne!(defender.physics.vel_ground.x, 0.0);
    }
}
