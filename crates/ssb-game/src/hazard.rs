//! Stage hazards on the fighter side: `ftmain.c`'s ground-hit search and
//! obstacle search, plus the Twister (`ftcommontwister.c`) and Barrel Cannon
//! (`ftcommontarucann.c`) statuses.
//!
//! The stage half lives in [`crate::stage`]. It owns the registries
//! (`sFTMainGroundObstacles`, `sFTMainGroundHazards`) and publishes the
//! obstacle positions a captured fighter follows. Sound, rumble and effect
//! calls are not ported, as elsewhere in the gameplay layer.

use crate::combat::DamageBy;
use crate::combat::{self, Element, HitLogEntry, HitSource, HitStatus};
use crate::fighter::Fighter;
use crate::stage::{Stage, StageObjects};
use crate::status::{self, AnyStatus, Status, StatusTiming};
use ssb_engine::input::N64Buttons;
use ssb_engine::math::Vec3;

/// `GMHitEnvironment`, plus the damage-floor kinds 4–9 that
/// `dFTMainGroundHitCollisionAttributes` stores in the same field.
pub const ENV_ACID: i32 = 0;
pub const ENV_POWER_BLOCK: i32 = 1;
pub const ENV_TWISTER: i32 = 2;
pub const ENV_TARUCANN: i32 = 3;

/// The literal attack handicap of every ground hit except the POW Block.
pub const GROUND_HANDICAP: u8 = 9;

/// `FTCOMMON_TORNADO_RELEASE_WAIT` / `FTCOMMON_TORNADO_PICKUP_WAIT`.
pub const TWISTER_RELEASE_WAIT: i32 = 60;
pub const TWISTER_PICKUP_WAIT: u16 = 60;
/// `FTCOMMON_TARUCANN_*`.
pub const TARUCANN_RELEASE_WAIT: i32 = 180;
pub const TARUCANN_SHOOT_WAIT: i32 = 10;
pub const TARUCANN_PICKUP_WAIT: u16 = 16;
/// `fp->acid_wait = 30` and `fp->damagefloor_wait = 16`.
pub const ACID_WAIT: u16 = 30;
pub const DAMAGEFLOOR_WAIT: u16 = 16;

/// `GRAttackColl`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GroundAttack {
    pub kind: i32,
    pub damage: i32,
    pub angle: i32,
    pub kb_scale: i32,
    pub kb_weight: i32,
    pub kb_base: i32,
    pub element: i32,
}

impl GroundAttack {
    /// Seven big-endian words, in source field order.
    pub fn from_words(w: [i32; 7]) -> Self {
        GroundAttack {
            kind: w[0],
            damage: w[1],
            angle: w[2],
            kb_scale: w[3],
            kb_weight: w[4],
            kb_base: w[5],
            element: w[6],
        }
    }

    fn hitbox(self, damage: i32) -> crate::attack::Hitbox {
        crate::attack::Hitbox {
            damage,
            offset: Vec3::ZERO,
            radius: 0.0,
            angle: self.angle,
            kb_scale: self.kb_scale,
            kb_weight: self.kb_weight,
            kb_base: self.kb_base,
            element: Element::from_raw(self.element as u32),
            shield_damage: 0,
        }
    }
}

/// `FTThrowHitDesc` as the Twister and Barrel Cannon read it from their
/// stage files. `status_id` is ignored by both callers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HazardThrow {
    pub damage: i32,
    pub angle: i32,
    pub kb_scale: i32,
    pub kb_weight: i32,
    pub kb_base: i32,
    pub element: i32,
}

impl HazardThrow {
    /// Seven big-endian words, in source field order (`status_id` first).
    pub fn from_words(w: [i32; 7]) -> Self {
        HazardThrow {
            damage: w[1],
            angle: w[2],
            kb_scale: w[3],
            kb_weight: w[4],
            kb_base: w[5],
            element: w[6],
        }
    }
}

/// `dFTMainGroundHitCollisionAttributes` @ 0x80128D30.
const fn floor(
    kind: i32,
    damage: i32,
    angle: i32,
    kbs: i32,
    kbw: i32,
    element: i32,
) -> GroundAttack {
    GroundAttack {
        kind,
        damage,
        angle,
        kb_scale: kbs,
        kb_weight: kbw,
        kb_base: 0,
        element,
    }
}
pub const DAMAGE_FLOORS: [GroundAttack; 6] = [
    floor(4, 1, 361, 100, 100, 1),
    floor(5, 10, 90, 100, 200, 1),
    floor(6, 10, 90, 100, 100, 1),
    floor(7, 10, 361, 100, 80, 3),
    floor(8, 1, 90, 100, 100, 1),
    floor(9, 1, 90, 100, 100, 1),
];

/// `nMPMaterial*` values that `ftMainGetGroundHitObstacle` tests.
pub mod material {
    pub const FIRE_WEAK_S1: u16 = 7;
    pub const FIRE_STRONG_S1: u16 = 8;
    pub const FIRE_WEAK_HI1: u16 = 9;
    pub const SPIKES: u16 = 10;
    pub const FIRE_WEAK_HI2: u16 = 11;
    pub const DETECT: u16 = 14;
    pub const FIRE_WEAK_HI3: u16 = 15;
}

/// The hazard fields of `FTStruct` and the two statuses' `status_vars`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct HazardState {
    pub twister_wait: u16,
    pub tarucann_wait: u16,
    pub acid_wait: u16,
    pub damagefloor_wait: u16,
    /// `coll_data.vel_push`: Whispy's wind, added on the first map substep
    /// and cleared at the end of `ftMainProcUpdateInterrupt`.
    pub vel_push: Vec3,
    /// `status_vars.common.{twister,tarucann}.release_wait`.
    pub release_wait: i32,
    /// `status_vars.common.tarucann.shoot_wait`.
    pub shoot_wait: i32,
    /// The captor's `DObj` translation, published by the stage before
    /// fighter physics (the stage process runs first at priority 4).
    pub anchor: Vec3,
    /// The barrel's `rotate.z`, published with [`Self::anchor`].
    pub barrel_rotate: f32,
    /// `grJungleTaruCannAddAnimShoot`, raised by the status and consumed by
    /// the stage before anything reads the barrel's child pose.
    pub shoot_request: bool,
    /// `DObj::rotate.y` while spinning in the Twister. Presentation only.
    pub twister_rotate_y: f32,
    /// The stage file's `FTThrowHitDesc` for its captor, published by the
    /// stage (`llGRHyruleMapTwisterThrowHitDesc`, `llGRJungleMapTaruCannThrowHitDesc`).
    pub throw: Option<HazardThrow>,
}

/// Whether the fighter is inside a stage captor.
pub fn is_captured(s: AnyStatus) -> bool {
    matches!(s, AnyStatus::Common(Status::Twister | Status::TaruCann))
}

/// `ftParamGetBestHitStatusAll`.
pub fn best_hit_status_all(f: &Fighter) -> HitStatus {
    let n = crate::hurtbox::damage_colls(f.kind).map_or(0, |d| d.len());
    let parts = &f.damage_colls.hitstatus[..n.min(f.damage_colls.hitstatus.len())];
    let mut best = parts.first().copied().unwrap_or(HitStatus::Normal);
    if best != HitStatus::Normal {
        for &h in &parts[1..] {
            if h == HitStatus::None {
                break;
            }
            if best > h {
                best = h;
            }
        }
    }
    let body = if crate::capture_kirby::is_intangible(f) {
        HitStatus::Intangible
    } else {
        f.hitstatus
    };
    best.max(body)
        .max(combat::star_hitstatus(f))
        .max(combat::special_hitstatus(f))
}

/// `ftParamGetGroundHazardKnockback` (US only): no damage ratio.
#[allow(clippy::too_many_arguments)]
pub fn ground_hazard_knockback(
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
        let add = percent_damage as f32 + recent_damage as f32;
        ((((add * 0.1) + (add * hit_damage as f32 * 0.05)) * weight * 1.4) + 18.0) * scale
            + kb_base as f32
    };
    crate::stale::apply_ratio_and_handicap(base, 100, attack_handicap, defend_handicap).min(2500.0)
}

// ---------------------------------------------------------------------------
// Search
// ---------------------------------------------------------------------------

/// `ftMainSearchHitHazard`, run first in `ftMainProcSearchCatch`.
/// `others` lists every other fighter's status and barrel link.
pub fn search_hit_hazard(
    f: &mut Fighter,
    stage: &mut Stage,
    objects: &mut dyn StageObjects,
    others: &[AnyStatus],
) {
    if f.hitlag == 0 {
        f.hazard.twister_wait = f.hazard.twister_wait.saturating_sub(1);
        f.hazard.tarucann_wait = f.hazard.tarucann_wait.saturating_sub(1);
    }
    let barrel_taken = others.contains(&AnyStatus::Common(Status::TaruCann));
    if let Some((kind, anchor)) = stage.check_obstacles(f, objects, barrel_taken) {
        f.hazard.anchor = anchor;
        match kind {
            ENV_TWISTER => set_twister(f),
            ENV_TARUCANN => {
                f.hazard.barrel_rotate = stage.barrel_rotate();
                set_tarucann(f);
            }
            _ => {}
        }
    }
}

/// `ftMainGetGroundHitObstacle`: the damaging floor under a grounded fighter.
pub fn damage_floor(f: &Fighter) -> Option<GroundAttack> {
    if f.hazard.damagefloor_wait != 0 || !f.is_grounded() {
        return None;
    }
    let floor = f.floor?;
    let index = match floor.flags & crate::collision::flags::MATERIAL {
        material::FIRE_WEAK_S1 => 0,
        material::FIRE_STRONG_S1 => 1,
        material::FIRE_WEAK_HI1 => 2,
        material::SPIKES => 3,
        material::FIRE_WEAK_HI2 => 4,
        material::FIRE_WEAK_HI3 => 5,
        _ => return None,
    };
    Some(DAMAGE_FLOORS[index])
}

/// `ftMainSearchGroundHit`, last in `ftMainProcSearchHitAll`.
pub fn search_ground_hit(f: &mut Fighter, stage: &Stage) {
    // `ftMainSearchHitHazard` and `ftMainProcSearchHitAll` skip a ghost.
    if f.dead.is_ghost {
        return;
    }
    if f.hitlag == 0 {
        f.hazard.acid_wait = f.hazard.acid_wait.saturating_sub(1);
        f.hazard.damagefloor_wait = f.hazard.damagefloor_wait.saturating_sub(1);
    }
    if best_hit_status_all(f) != HitStatus::Normal {
        return;
    }
    if let Some((attack, handicap)) = stage.check_hazards(f) {
        update_damage_stat_ground(f, attack, attack.kind, handicap);
    }
    if let Some(attack) = damage_floor(f) {
        update_damage_stat_ground(f, attack, attack.kind, GROUND_HANDICAP);
    }
}

/// `ftMainUpdateDamageStatGround`.
fn update_damage_stat_ground(f: &mut Fighter, attack: GroundAttack, kind: i32, handicap: u8) {
    let damage = crate::attack::captured_damage(f, attack.damage);
    if combat::check_get_update_damage(f, damage) {
        let entry = HitLogEntry {
            stat: crate::spgame::live::AttackStat::default(),
            object: if kind == ENV_ACID {
                crate::spgame::bonus::DamageObject::Acid
            } else {
                crate::spgame::bonus::DamageObject::Other
            },
            source: HitSource::Direct {
                lr: f.facing.sign(),
            },
            hitbox: attack.hitbox(attack.damage),
            attacker_pos: f.pos,
            attack_handicap: handicap,
            placement: 0,
            // Acid keeps `damage_player`; the other stage attacks set
            // `GMCOMMON_PLAYERS_MAX`. The POW Block's `damage_port` is not
            // ported.
            attacker: if kind == ENV_ACID {
                combat::DamageBy::Keep
            } else {
                combat::DamageBy::World
            },
            // `ftMainProcessHitCollisionStatsMain` makes no effect for the
            // stage.
            effect: None,
        };
        combat::push_log(f, entry);
    }
    match kind {
        ENV_ACID => {
            f.hazard.acid_wait = ACID_WAIT;
            crate::sound::play_fgm(crate::sound::id::nSYAudioFGMFloorDamageFire);
        }
        4..=9 => {
            f.hazard.damagefloor_wait = DAMAGEFLOOR_WAIT;
            crate::sound::play_fgm(if kind == 7 {
                crate::sound::id::nSYAudioFGMShockML
            } else {
                crate::sound::id::nSYAudioFGMFloorDamageFire
            });
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// Statuses
// ---------------------------------------------------------------------------

/// The common entry of both setters: drop the grab and stop.
fn enter(f: &mut Fighter, status: Status) {
    // Heavy items are not ported, so there is nothing to drop.
    if f.grab.catch.is_some() {
        crate::grab::release_on_hit(f);
    } else {
        crate::grab::release_on_capture_hit(f);
    }
    status::set_status(f, status, 0.0, StatusTiming::unknown());
    status::play_anim_events(f);
    crate::physics::stop_all(&mut f.physics);
    f.hazard.release_wait = 0;
    f.hazard.shoot_wait = 0;
    f.hazard.shoot_request = false;
    f.grab.capture_immune = true;
}

/// `ftCommonTwisterSetStatus` @ 0x80143BC4.
pub fn set_twister(f: &mut Fighter) {
    // `ftParamStopVoiceRunProcDamage`.
    crate::fighter_sound::stop_voice(f);
    if f.is_grounded() {
        f.become_airborne();
    }
    enter(f, Status::Twister);
    crate::sound::play_fgm(crate::sound::id::nSYAudioFGMHyruleTwisterTrapped);
}

/// `ftCommonTaruCannSetStatus` @ 0x80143F30.
pub fn set_tarucann(f: &mut Fighter) {
    // `ftParamStopVoiceRunProcDamage`.
    crate::fighter_sound::stop_voice(f);
    enter(f, Status::TaruCann);
    crate::hurtbox::set_hit_status_all(f, HitStatus::Intangible);
    f.is_invisible = true;
    crate::sound::play_fgm(crate::sound::id::nSYAudioFGMJungleTaruCannEnter);
}

/// `proc_update` and `proc_interrupt` of both statuses. Returns `false` when
/// neither is current.
pub fn update(f: &mut Fighter, current: Status) -> bool {
    let throw = f.hazard.throw;
    match current {
        Status::Twister => {
            f.hazard.release_wait += 1;
            if f.hazard.release_wait >= TWISTER_RELEASE_WAIT {
                if let Some(t) = throw {
                    shoot_twister(f, t);
                }
            }
            true
        }
        Status::TaruCann => {
            if f.hazard.shoot_wait != 0 {
                f.hazard.shoot_wait -= 1;
                if f.hazard.shoot_wait == TARUCANN_SHOOT_WAIT / 2 {
                    crate::sound::play_fgm(crate::sound::id::nSYAudioFGMJungleTaruCannShoot);
                }
                if f.hazard.shoot_wait == 0 {
                    if let Some(t) = throw {
                        shoot_tarucann(f, t);
                    }
                    return true;
                }
            }
            f.hazard.release_wait += 1;
            if f.hazard.release_wait >= TARUCANN_RELEASE_WAIT && f.hazard.shoot_wait == 0 {
                f.hazard.shoot_wait = TARUCANN_SHOOT_WAIT;
                f.hazard.shoot_request = true;
            }
            // `ftCommonTaruCannProcInterrupt`.
            if f.hazard.shoot_wait == 0 && f.button_tap().contains(N64Buttons::A | N64Buttons::B) {
                f.hazard.shoot_wait = TARUCANN_SHOOT_WAIT;
                f.hazard.shoot_request = true;
            }
            true
        }
        _ => false,
    }
}

/// `ftCommonTwisterShootFighter` @ 0x80143CC4.
fn shoot_twister(f: &mut Fighter, t: HazardThrow) {
    f.pos.z = 0.0;
    let knockback = crate::attack::knockback(
        f.damage,
        t.damage,
        t.damage,
        t.kb_weight,
        t.kb_scale,
        t.kb_base,
        f.attributes.weight,
        GROUND_HANDICAP,
        f.handicap,
    );
    let damage = if best_hit_status_all(f) != HitStatus::Normal {
        0
    } else {
        t.damage
    };
    crate::attack::init_damage_vars_sfx(
        f,
        None,
        damage,
        knockback,
        t.angle,
        f.facing.sign(),
        0,
        Element::from_raw(t.element as u32),
        true,
        true,
    );
    crate::spgame::live::hit(
        f,
        DamageBy::World,
        crate::spgame::live::AttackStat::default(),
        crate::spgame::bonus::DamageObject::Twister,
    );
    if damage != 0 {
        f.add_damage(damage);
    }
    f.hazard.twister_wait = TWISTER_PICKUP_WAIT;
}

/// `ftCommonTaruCannShootFighter` @ 0x80144038. The barrel damage feeds
/// only the knockback; the source never adds it to the percentage.
fn shoot_tarucann(f: &mut Fighter, t: HazardThrow) {
    f.pos.z = 0.0;
    let knockback = ground_hazard_knockback(
        f.damage,
        t.damage,
        t.damage,
        t.kb_weight,
        t.kb_scale,
        t.kb_base,
        f.attributes.weight,
        GROUND_HANDICAP,
        GROUND_HANDICAP,
    );
    let lr = f.facing.sign();
    // `I_CLC_RTOD32` truncates toward zero, as does the C division.
    let degrees = ((f.hazard.barrel_rotate / core::f32::consts::PI) * 180.0) as i32;
    let mut angle = degrees * -(lr as i32) + 90;
    angle -= (angle / 360) * 360;
    crate::attack::init_damage_vars_sfx(
        f,
        Some(AnyStatus::Common(Status::DamageFlyRoll)),
        t.damage,
        knockback,
        angle,
        lr,
        0,
        Element::from_raw(t.element as u32),
        false,
        true,
    );
    crate::spgame::live::hit(
        f,
        DamageBy::World,
        crate::spgame::live::AttackStat::default(),
        crate::spgame::bonus::DamageObject::Other,
    );
    f.hazard.tarucann_wait = TARUCANN_PICKUP_WAIT;
}

/// `proc_physics` + `proc_map` (`mpCommonProcFighterProject`) of both
/// statuses. Returns `false` when neither is current.
pub fn tick_status<I, F>(f: &mut Fighter, surfaces: &F) -> bool
where
    F: Fn() -> I,
    I: IntoIterator<Item = crate::weapon::MapSurface>,
{
    let target = match f.status.status {
        AnyStatus::Common(Status::Twister) => {
            // `ftCommonTwisterProcPhysics` @ 0x80143A20.
            let t = f.hazard.release_wait as f32 * 0.016_666_668;
            let mul = ((400.0 * t) + 100.0) * 0.5;
            let spin = (1800.0 * t) * core::f32::consts::PI / 180.0;
            let (sin, cos) = ssb_engine::math::sin_cos(spin);
            let goal = f.hazard.anchor + Vec3::new(mul * cos, 500.0 * t, mul * sin);
            let mut vel = goal - f.pos;
            let mag = ssb_engine::math::sqrt(vel.x * vel.x + vel.y * vel.y + vel.z * vel.z);
            if mag > 50.0 {
                vel *= 50.0 / mag;
            }
            f.physics.vel_air = vel;
            f.hazard.twister_rotate_y = f.facing.sign() * core::f32::consts::FRAC_PI_2 + spin;
            f.pos + vel
        }
        // `ftCommonTaruCannProcPhysics`: the fighter sits at the barrel.
        AnyStatus::Common(Status::TaruCann) => f.hazard.anchor,
        _ => return false,
    };
    // `MAP_PROC_TYPE_PROJECT`: floors stop the body but it stays airborne.
    let push = core::mem::take(&mut f.hazard.vel_push);
    let from = f.pos;
    let moved = crate::map::move_air_pushed(
        &f.coll,
        from,
        target,
        push,
        crate::map::AirOptions::default(),
        surfaces,
    );
    f.pos = moved.moved.pos;
    f.floor = None;
    f.map_contacts = moved.contacts;
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fighter::FighterKind;

    #[test]
    fn damage_floors_follow_the_source_table() {
        let mut f = Fighter::new(FighterKind::Mario, 0, 3);
        f.situation = crate::fighter::Situation::Ground;
        f.floor = Some(crate::ground::Standing {
            line: 0,
            flags: material::SPIKES,
            normal: ssb_engine::math::Vec2::new(0.0, 1.0),
        });
        assert_eq!(damage_floor(&f).unwrap().kind, 7);
        f.hazard.damagefloor_wait = 1;
        assert!(damage_floor(&f).is_none());
    }

    #[test]
    fn ground_knockback_skips_the_damage_ratio_and_clamps() {
        let kb = ground_hazard_knockback(0, 0, 0, 0, 100, 3000, 1.0, 9, 9);
        assert_eq!(kb, 2500.0);
    }

    #[test]
    fn barrel_angle_truncates_like_the_source() {
        // 0.07 rad * 3 = 12.03°; facing right: -12 + 90 = 78.
        let mut f = Fighter::new(FighterKind::Mario, 0, 3);
        f.hazard.barrel_rotate = 0.21;
        let t = HazardThrow::from_words([0, 10, 0, 100, 0, 50, 0]);
        shoot_tarucann(&mut f, t);
        assert_eq!(f.hazard.tarucann_wait, TARUCANN_PICKUP_WAIT);
        assert_eq!(f.status.status, AnyStatus::Common(Status::DamageFlyRoll));
        assert_eq!(f.damage, 0, "the barrel adds no percentage");
        assert_eq!(f.damage_player, None);
        assert_eq!(f.stats.damage_count, 1);
        assert_eq!(
            f.stats.damage_object,
            crate::spgame::bonus::DamageObject::Other
        );
    }

    #[test]
    fn twister_launch_replaces_the_last_player_hit_even_when_intangible() {
        let mut f = Fighter::new(FighterKind::Mario, 0, 3);
        f.damage_player = Some(1);
        f.stats.enable();
        crate::hurtbox::set_hit_status_all(&mut f, HitStatus::Intangible);
        shoot_twister(&mut f, HazardThrow::from_words([0, 10, 0, 100, 0, 50, 0]));
        assert_eq!(f.damage, 0);
        assert_eq!(f.damage_player, None);
        assert_eq!(
            f.stats.damage_object,
            crate::spgame::bonus::DamageObject::Twister
        );
        assert_eq!(
            f.stats.drain().collect::<alloc::vec::Vec<_>>(),
            [crate::spgame::live::Event::Defend {
                player: None,
                flags: crate::spgame::live::Flags(0)
            }]
        );
    }
}
