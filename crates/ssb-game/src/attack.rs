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
//! * **No `Damage` status.** The target does not enter a hit-reaction status
//!   or animation; only the numeric state (`damage`, `physics.vel_knockback`,
//!   `hitstun`) is real and decomp-accurate. Playing a real damage animation
//!   needs a `Damage` status family (`ftcommondamage.c`'s twelve status IDs)
//!   this slice does not add.

use ssb_engine::math::{sin_cos, Vec3};

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

/// The outcome of a hit landing, ready to apply to the defending
/// [`crate::fighter::Fighter`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HitResult {
    pub damage: i32,
    pub knockback_vel: Vec3,
    pub hitstun: u16,
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
    let hitstun = (hitstun_frames(knockback) as u16).max(1);
    HitResult {
        damage: hitbox.damage,
        knockback_vel: Vec3::new(-vel_x * lr, vel_y, 0.0),
        hitstun,
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
}
