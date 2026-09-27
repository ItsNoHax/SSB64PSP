//! The per-frame hit pipeline — `ftMainProcSearchHitAll`,
//! `ftMainProcessHitCollisionStatsMain` and `ftMainProcParams`.
//!
//! The original resolves combat in three passes over every fighter, each run
//! for all fighters before the next starts:
//!
//! 1. **Search** ([`search_fighter_hits`], and the weapon pool's
//!    `apply_hits`): every live attack collision of every other fighter is
//!    tested against this fighter. A contact only *records* — the attacker's
//!    per-victim [`AttackRecord`], the defender's `damage_queue`,
//!    `damage_lag` and hit log, the shield's `shield_damage_total`, and the
//!    attacker's `attack_damage`/`attack_shield_push`/`attack_rebound`.
//!    Attack collisions of two grounded fighters that meet trade priority
//!    first (clank, `ftMainUpdateAttackStatFighter`).
//! 2. **Process** ([`process_hit_collision`]): the hit log entry with the
//!    strongest knockback, computed against the frame's whole
//!    `damage_queue`, decides the damage angle, element, side and hurtbox
//!    placement.
//! 3. **Params** ([`proc_params`]): shield regeneration and damage, then
//!    one outcome in priority order — a damage status, a shield push or
//!    break, a clank rebound, or the attacker's own `proc_hit` — and the
//!    hitlag that follows from it.
//!
//! Attack collisions come from the motion scripts ([`crate::motion`]); their
//! records follow the source's group rules. Positions update once per frame
//! ([`update_attack_positions`]) and contacts use the swept tests of
//! `gmCollisionTestRectangle`/`gmCollisionTestSphere`, so a fast swing hits
//! what it passed through.

use ssb_engine::math::Vec3;

use crate::attack;
use crate::fighter::{Fighter, FighterKind, JointTransform};
use crate::stale::MotionAttackId;
use crate::status::{self, AnyStatus, Status};

/// `GMHitStatus`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
#[repr(u8)]
pub enum HitStatus {
    /// Terminates the damage-collision list.
    None = 0,
    #[default]
    Normal = 1,
    /// Touched, but takes no damage: the attacker still registers the hit.
    Invincible = 2,
    /// Not touched at all.
    Intangible = 3,
}

impl HitStatus {
    pub fn from_raw(raw: u32) -> Self {
        match raw {
            0 => HitStatus::None,
            2 => HitStatus::Invincible,
            3 => HitStatus::Intangible,
            _ => HitStatus::Normal,
        }
    }
}

/// `GMHitElement`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Element {
    #[default]
    Normal,
    Fire,
    Electric,
    Slash,
    Coin,
    Freezing,
    Sleep,
}

impl Element {
    pub fn from_raw(raw: u32) -> Self {
        match raw {
            1 => Element::Fire,
            2 => Element::Electric,
            3 => Element::Slash,
            4 => Element::Coin,
            5 => Element::Freezing,
            6 => Element::Sleep,
            _ => Element::Normal,
        }
    }
}

/// `GMAttackState`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AttackState {
    #[default]
    Off,
    /// Made this frame: the position is set without a previous one.
    New,
    /// One frame old: tested at its current position only.
    Transfer,
    /// Swept from the previous frame's position.
    Interpolate,
}

/// `GMHitFlags::group_id` for "no clank recorded".
const NO_GROUP: u8 = 7;

/// `GMAttackRecord`, keyed by the victim's player port.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AttackRecord {
    pub victim: Option<u8>,
    pub is_interact_hurt: bool,
    pub is_interact_shield: bool,
    /// The victim attack's group after a clank, or `NO_GROUP`.
    pub group_id: u8,
}

impl Default for AttackRecord {
    fn default() -> Self {
        AttackRecord {
            victim: None,
            is_interact_hurt: false,
            is_interact_shield: false,
            group_id: NO_GROUP,
        }
    }
}

impl AttackRecord {
    /// Whether this record still lets the attack touch its victim.
    fn is_clear(&self) -> bool {
        !self.is_interact_hurt && !self.is_interact_shield && self.group_id == NO_GROUP
    }
}

/// `GMATTACKREC_NUM_MAX`.
pub const ATTACK_RECORDS: usize = 4;

/// `FTAttackColl`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct AttackColl {
    pub state: AttackState,
    pub group: u8,
    /// `FTStruct::joints` index.
    pub joint: u8,
    /// Already staled (`ftParamGetStaledDamage` at creation).
    pub damage: i32,
    pub can_rebound: bool,
    pub element: Element,
    /// Radius (`size * 0.5`).
    pub size: f32,
    pub offset: Vec3,
    pub angle: i32,
    pub kb_scale: i32,
    pub kb_weight: i32,
    pub kb_base: i32,
    pub shield_damage: i32,
    pub is_hit_air: bool,
    pub is_hit_ground: bool,
    /// `MakeAttackCollScaled`: the offset is divided by `FTAttributes::size`.
    pub is_scale_pos: bool,
    pub motion_attack_id: MotionAttackId,
    pub motion_count: u16,
    pub pos_curr: Vec3,
    pub pos_prev: Vec3,
    pub records: [AttackRecord; ATTACK_RECORDS],
}

impl AttackColl {
    /// `ftParamClearAttackRecordID`.
    pub fn clear_records(&mut self) {
        self.records = [AttackRecord::default(); ATTACK_RECORDS];
    }

    fn record(&self, victim: u8) -> AttackRecord {
        self.records
            .iter()
            .find(|r| r.victim == Some(victim))
            .copied()
            .unwrap_or_default()
    }

    /// Whether this collision may touch a fighter in situation `airborne`.
    fn reaches(&self, airborne: bool) -> bool {
        if airborne {
            self.is_hit_air
        } else {
            self.is_hit_ground
        }
    }

    /// The motion command's fields as a [`attack::Hitbox`].
    pub fn hitbox(&self) -> attack::Hitbox {
        attack::Hitbox {
            damage: self.damage,
            offset: self.offset,
            radius: self.size,
            angle: self.angle,
            kb_scale: self.kb_scale,
            kb_weight: self.kb_weight,
            kb_base: self.kb_base,
            element: self.element,
            shield_damage: self.shield_damage,
        }
    }
}

/// `ftParamClearAttackCollAll`.
pub fn clear_attack_colls(f: &mut Fighter) {
    for coll in &mut f.attack_colls {
        coll.state = AttackState::Off;
    }
}

/// Which way `ftMainSetHitInteractStats` marks a record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HitType {
    Damage,
    Shield,
    Attack(u8),
}

/// `ftMainSetHitInteractStats`: records the contact on every live collision
/// of the group, then drops those collisions from `detect` for the rest of
/// this search.
fn set_hit_interact(
    attacker: &mut Fighter,
    group: u8,
    victim: u8,
    kind: HitType,
    detect: &mut [bool; 4],
) {
    for (i, coll) in attacker.attack_colls.iter_mut().enumerate() {
        if coll.state == AttackState::Off || coll.group != group {
            continue;
        }
        let slot = coll
            .records
            .iter()
            .position(|r| r.victim == Some(victim))
            .or_else(|| coll.records.iter().position(|r| r.victim.is_none()))
            .unwrap_or(0);
        let record = &mut coll.records[slot];
        if record.victim != Some(victim) {
            // A reused slot keeps the previous victim's flags, as the source
            // overwrites only the victim pointer.
            record.victim = Some(victim);
        }
        match kind {
            HitType::Damage => record.is_interact_hurt = true,
            HitType::Shield => record.is_interact_shield = true,
            HitType::Attack(g) => record.group_id = g,
        }
        detect[i] = false;
    }
}

/// Where a registered hit came from, for the knockback and push direction.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HitSource {
    Fighter {
        port: u8,
    },
    /// Pushed away from the attack's position.
    Position,
    /// A weapon's side comes from its travel unless it is nearly still.
    Weapon {
        vel_x: f32,
    },
    /// A thrown or scripted hit with an explicit side.
    Direct {
        lr: f32,
    },
}

/// `FTHitLog`, with the attack's fields copied at contact time.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HitLogEntry {
    pub source: HitSource,
    pub hitbox: attack::Hitbox,
    pub attacker_pos: Vec3,
    pub attack_handicap: u8,
    /// The hurtbox's `placement` column.
    pub placement: usize,
}

/// `sFTMainHitLogs` holds ten.
pub const HIT_LOG_MAX: usize = 10;

/// `FTStruct::damage_kind`: set by the other side of a grab link before
/// this fighter's `ftMainProcParams` runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DamageKind {
    /// `ftCommonDamageUpdateMain`.
    #[default]
    Default,
    None,
    Status,
    ColAnim,
    Catch,
}

/// The per-frame hit fields of `FTStruct`, zeroed by [`proc_params`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FrameHits {
    pub log: [Option<HitLogEntry>; HIT_LOG_MAX],
    pub log_len: usize,
    pub damage_queue: i32,
    pub damage_lag: i32,
    pub shield_damage_total: i32,
    pub shield_damage: i32,
    pub shield_lr: f32,
    pub attack_damage: i32,
    pub attack_shield_push: i32,
    pub attack_rebound: f32,
    pub hit_lr: f32,
    pub hitlag_mul: f32,
    pub damage_knockback: f32,
    pub damage_angle: i32,
    pub damage_element: Element,
    pub damage_lr: f32,
    pub damage_index: usize,
    pub damage_kind: DamageKind,
}

impl Default for FrameHits {
    fn default() -> Self {
        FrameHits {
            log: [None; HIT_LOG_MAX],
            log_len: 0,
            damage_queue: 0,
            damage_lag: 0,
            shield_damage_total: 0,
            shield_damage: 0,
            shield_lr: 0.0,
            attack_damage: 0,
            attack_shield_push: 0,
            attack_rebound: 0.0,
            hit_lr: 0.0,
            hitlag_mul: 1.0,
            damage_knockback: 0.0,
            damage_angle: 0,
            damage_element: Element::Normal,
            damage_lr: 0.0,
            damage_index: 0,
            damage_kind: DamageKind::Default,
        }
    }
}

/// `FTAttributes` fields the hit pipeline reads.
pub use crate::motion::CombatAttrs;

/// The fighter's hit-status as set by rebirth/intangibility timers
/// (`special_hitstatus`).
pub fn special_hitstatus(f: &Fighter) -> HitStatus {
    if f.intangible_frames > 0 {
        HitStatus::Intangible
    } else if f.invincible_frames > 0 {
        HitStatus::Invincible
    } else {
        HitStatus::Normal
    }
}

/// `ftParamGetBestHitStatusAll`'s whole-body part: whether any body-wide
/// status makes the fighter untouchable.
pub fn is_body_intangible(f: &Fighter) -> bool {
    special_hitstatus(f) == HitStatus::Intangible || body_hitstatus(f) == HitStatus::Intangible
}

/// Whether every body-wide hit status is normal.
pub fn is_body_normal(f: &Fighter) -> bool {
    special_hitstatus(f) == HitStatus::Normal && body_hitstatus(f) == HitStatus::Normal
}

/// `FTStruct::hitstatus`, including the `ftParamSetHitStatusAll` writes of
/// Kirby's capture and star statuses (`crate::capture_kirby`).
fn body_hitstatus(f: &Fighter) -> HitStatus {
    if crate::capture_kirby::is_intangible(f) {
        HitStatus::Intangible
    } else {
        f.hitstatus
    }
}

/// `FTStruct::throw_gobj`: the Kirby that spat this fighter out as a star.
fn throw_port(f: &Fighter) -> Option<u8> {
    if crate::capture_kirby::is_star(f.status.status) {
        f.kirby_capture.thrower
    } else {
        None
    }
}

/// `FTStruct::is_catchstatus`: a catch status's collisions only grab.
pub fn is_catchstatus(f: &Fighter) -> bool {
    f.grab.is_catchstatus
}

/// `FTStruct::is_shield`.
pub fn is_shield(f: &Fighter) -> bool {
    f.guard.is_shield
}

/// The end of `ftMainProcPhysicsMap`: collisions made this frame take their
/// first position, older ones keep the previous one for the swept test.
/// Call once per frame for every fighter, after its pose is final.
pub fn update_attack_positions(f: &mut Fighter) {
    let size_mul = 1.0 / crate::motion::combat_attrs(f.kind).map_or(1.0, |a| a.size);
    for i in 0..f.attack_colls.len() {
        let coll = f.attack_colls[i];
        let mut offset = coll.offset;
        if coll.is_scale_pos {
            offset *= size_mul;
        }
        let world = f.joint_world(coll.joint, offset);
        let coll = &mut f.attack_colls[i];
        match coll.state {
            AttackState::Off => {}
            AttackState::New => {
                coll.pos_curr = world;
                coll.pos_prev = world;
                coll.state = AttackState::Transfer;
            }
            AttackState::Transfer | AttackState::Interpolate => {
                coll.state = AttackState::Interpolate;
                coll.pos_prev = coll.pos_curr;
                coll.pos_curr = world;
            }
        }
    }
}

/// `gmCollisionCheckAttackInFighterRange` against `hit_detect_range`.
fn in_range(pos: Vec3, target: Vec3, range: [f32; 3], size: f32) -> bool {
    let dx = pos.x - target.x;
    let dy = pos.y - target.y;
    !(dx < -range[2] - size
        || dx > range[2] + size
        || dy < -range[1] - size
        || dy > range[0] + size)
}

fn attack_in_fighter_range(coll: &AttackColl, f: &Fighter) -> bool {
    let range = crate::motion::combat_attrs(f.kind).map_or([f32::MAX; 3], |a| a.hit_detect_range);
    if coll.state == AttackState::Transfer {
        in_range(coll.pos_curr, f.pos, range, coll.size)
    } else {
        in_range(coll.pos_curr, f.pos, range, coll.size)
            || in_range(coll.pos_prev, f.pos, range, coll.size)
    }
}

/// `nFTPartsJointYRotN`.
const JOINT_YROTN: usize = 3;
/// `FTCOMMON_GUARD_SIZE_*`.
const GUARD_SIZE_HEALTH_DIV: f32 = 55.0;
const GUARD_SIZE_SCALE_MUL_INIT: f32 = 0.65;
const GUARD_SIZE_SCALE_MUL_ADD: f32 = 0.35;
const GUARD_SIZE_SCALE_MUL_DIV: f32 = 30.0;

/// The shield's joint (`YRotN`, scaled by `ftCommonGuardUpdateShieldCollision`).
pub fn shield_transform(f: &Fighter) -> JointTransform {
    let attrs = crate::motion::combat_attrs(f.kind);
    let shield_size = attrs.map_or(260.0, |a| a.shield_size);
    let scale_mul = if f.kind == FighterKind::Yoshi {
        1.0
    } else {
        f.guard.shield_health / GUARD_SIZE_HEALTH_DIV
    };
    let scale = ((GUARD_SIZE_SCALE_MUL_INIT * scale_mul) + GUARD_SIZE_SCALE_MUL_ADD) * shield_size
        / GUARD_SIZE_SCALE_MUL_DIV;
    let base = f.joint_transforms[JOINT_YROTN].unwrap_or(JointTransform {
        axes: [
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::new(0.0, 0.0, 1.0),
        ],
        origin: f.pos,
    });
    JointTransform {
        axes: [
            base.axes[0] * scale,
            base.axes[1] * scale,
            base.axes[2] * scale,
        ],
        origin: base.origin,
    }
}

/// `gmCollisionCheckFighterAttackShieldCollide`: a 30-unit sphere at the
/// shield joint.
fn attack_hits_shield(
    pos_curr: Vec3,
    pos_prev: Vec3,
    size: f32,
    state: AttackState,
    f: &Fighter,
) -> bool {
    let t = shield_transform(f);
    crate::hurtbox::test_sphere(
        &t,
        pos_curr,
        pos_prev,
        size,
        state,
        Vec3::ZERO,
        Vec3::new(30.0, 30.0, 30.0),
    )
}

/// `ftMainCheckGetUpdateDamage`: a `damage_resist` pool (Kirby's Stone)
/// soaks the hit, and only its overflow reaches `damage_queue`.
fn check_get_update_damage(f: &mut Fighter, damage: i32) -> bool {
    let mut damage = damage;
    if f.kirby.is_damage_resist {
        f.kirby.damage_resist -= damage;
        if f.kirby.damage_resist <= 0 {
            f.kirby.is_damage_resist = false;
            damage = -f.kirby.damage_resist;
        }
    }
    if f.kirby.is_damage_resist {
        return false;
    }
    f.hits.damage_queue += damage;
    if f.hits.damage_lag < damage {
        f.hits.damage_lag = damage;
    }
    true
}

fn push_log(f: &mut Fighter, entry: HitLogEntry) {
    if f.hits.log_len < HIT_LOG_MAX {
        f.hits.log[f.hits.log_len] = Some(entry);
        f.hits.log_len += 1;
    }
}

/// `ftMainSetHitRebound`.
fn set_hit_rebound(fp: &mut Fighter, coll: &AttackColl, victim_x: f32) {
    if fp.hits.attack_shield_push < coll.damage {
        fp.hits.attack_shield_push = coll.damage;
        if coll.can_rebound && fp.is_grounded() {
            fp.hits.attack_rebound = fp.hits.attack_shield_push as f32 * 1.62 + 4.0;
            fp.hits.hit_lr = if fp.pos.x < victim_x { 1.0 } else { -1.0 };
        }
    }
}

/// `ftMainSearchHitFighter` for one attacker: `other`'s attack collisions
/// against `this`. `other_after_this` is the fighter-list order; only then do
/// the two fighters' attacks trade priority, so each pair clanks once.
pub fn search_fighter_hits(this: &mut Fighter, other: &mut Fighter, other_after_this: bool) {
    if this.port == other.port || this.grab.capture == Some(other.port) || is_catchstatus(other) {
        return;
    }
    // A thrown star never hits the fighter that threw it.
    if throw_port(other) == Some(this.port) {
        return;
    }
    let this_air = !this.is_grounded();
    let mut damage_detect = [false; 4];
    for (i, coll) in other.attack_colls.iter().enumerate() {
        damage_detect[i] = coll.state != AttackState::Off
            && coll.reaches(this_air)
            && coll.record(this.port).is_clear();
    }
    if !damage_detect.iter().any(|&d| d) {
        return;
    }
    // Clank: both grounded, `this` not grabbing.
    if other_after_this
        && other.grab.capture != Some(this.port)
        && other.is_grounded()
        && this.is_grounded()
        && !is_catchstatus(this)
        && throw_port(this) != Some(other.port)
    {
        let mut attack_detect = [false; 4];
        let other_air = !other.is_grounded();
        for (i, coll) in this.attack_colls.iter().enumerate() {
            attack_detect[i] = coll.state != AttackState::Off
                && coll.reaches(other_air)
                && coll.record(other.port).is_clear();
        }
        if attack_detect.iter().any(|&d| d) {
            for i in 0..4 {
                if !damage_detect[i] {
                    continue;
                }
                for j in 0..4 {
                    if !attack_detect[j] {
                        continue;
                    }
                    if attacks_collide(&other.attack_colls[i], &this.attack_colls[j]) {
                        update_attack_stat(
                            other,
                            i,
                            this,
                            j,
                            &mut damage_detect,
                            &mut attack_detect,
                        );
                        if !damage_detect[i] {
                            break;
                        }
                    }
                }
            }
        }
    }
    let mut in_reach = false;
    for (detect, coll) in damage_detect.iter_mut().zip(&other.attack_colls) {
        if *detect {
            *detect = attack_in_fighter_range(coll, this);
            in_reach |= *detect;
        }
    }
    if !in_reach {
        return;
    }
    if is_shield(this) {
        for i in 0..4 {
            if !damage_detect[i] {
                continue;
            }
            let c = other.attack_colls[i];
            if attack_hits_shield(c.pos_curr, c.pos_prev, c.size, c.state, this) {
                update_shield_stat(other, i, this, &mut damage_detect);
            }
        }
    }
    if is_body_intangible(this) {
        return;
    }
    for i in 0..4 {
        if !damage_detect[i] {
            continue;
        }
        let c = other.attack_colls[i];
        if let Some(hit) =
            crate::hurtbox::search_attack(this, c.pos_curr, c.pos_prev, c.size, c.state)
        {
            update_damage_stat(other, i, this, hit, &mut damage_detect);
        }
    }
}

/// `gmCollisionCheckFighterAttacksCollide`.
fn attacks_collide(a: &AttackColl, b: &AttackColl) -> bool {
    crate::hurtbox::attacks_collide(
        (a.pos_curr, a.pos_prev, a.size, a.state),
        (b.pos_curr, b.pos_prev, b.size, b.state),
    )
}

/// `ftMainUpdateAttackStatFighter`: clank. The stronger side (by more than
/// 10 damage) cuts through; otherwise both stop, and a grounded rebound-able
/// attack recoils.
fn update_attack_stat(
    other: &mut Fighter,
    oi: usize,
    this: &mut Fighter,
    ti: usize,
    damage_detect: &mut [bool; 4],
    attack_detect: &mut [bool; 4],
) {
    let other_hit = other.attack_colls[oi];
    let this_hit = this.attack_colls[ti];
    if this_hit.damage - 10 < other_hit.damage {
        set_hit_interact(
            this,
            this_hit.group,
            other.port,
            HitType::Attack(other_hit.group),
            attack_detect,
        );
        set_hit_rebound(this, &this_hit, other.pos.x);
    }
    if other_hit.damage - 10 < this_hit.damage {
        set_hit_interact(
            other,
            other_hit.group,
            this.port,
            HitType::Attack(this_hit.group),
            damage_detect,
        );
        set_hit_rebound(other, &other_hit, this.pos.x);
    }
}

/// `ftMainUpdateShieldStatFighter`.
fn update_shield_stat(
    attacker: &mut Fighter,
    i: usize,
    victim: &mut Fighter,
    detect: &mut [bool; 4],
) {
    let coll = attacker.attack_colls[i];
    set_hit_interact(attacker, coll.group, victim.port, HitType::Shield, detect);
    if attacker.hits.attack_shield_push < coll.damage {
        attacker.hits.attack_shield_push = coll.damage;
    }
    victim.hits.shield_damage_total += coll.damage + coll.shield_damage;
    if victim.hits.shield_damage < coll.damage {
        victim.hits.shield_damage = coll.damage;
        victim.hits.shield_lr = if victim.pos.x < attacker.pos.x {
            1.0
        } else {
            -1.0
        };
    }
}

/// `ftMainUpdateDamageStatFighter`. An invincible body or hurtbox still
/// counts as touched for the attacker, which takes its hitlag and never
/// hits that victim again with this attack.
fn update_damage_stat(
    attacker: &mut Fighter,
    i: usize,
    victim: &mut Fighter,
    hit: crate::hurtbox::HurtHit,
    detect: &mut [bool; 4],
) {
    let coll = attacker.attack_colls[i];
    set_hit_interact(attacker, coll.group, victim.port, HitType::Damage, detect);
    let damage = attack::captured_damage(victim, coll.damage);
    if attacker.hits.attack_damage < damage {
        attacker.hits.attack_damage = damage;
    }
    if is_body_normal(victim)
        && hit.hitstatus == HitStatus::Normal
        && check_get_update_damage(victim, damage)
    {
        push_log(
            victim,
            HitLogEntry {
                source: HitSource::Fighter {
                    port: attacker.port,
                },
                hitbox: coll.hitbox(),
                attacker_pos: attacker.pos,
                attack_handicap: attacker.handicap,
                placement: hit.placement,
            },
        );
        if attacker.port != victim.port {
            attacker
                .stale
                .push(coll.motion_attack_id, coll.motion_count);
        }
    }
}

/// What a weapon hitbox did to a fighter (`ftMainSearchHitWeapon`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeaponContact {
    Missed,
    /// Recorded by the shield.
    Shielded,
    /// Touched a hurtbox; `true` when it will deal damage.
    Hurt(bool),
}

/// A weapon's attack for [`weapon_hit`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WeaponAttack {
    /// Damage already staled (`wpMainGetStaledDamage`).
    pub hitbox: attack::Hitbox,
    pub pos_curr: Vec3,
    pub pos_prev: Vec3,
    pub source: HitSource,
    pub handicap: u8,
    pub can_shield: bool,
}

/// `ftMainSearchHitWeapon`'s shield and damage halves for one weapon hitbox
/// (reflection is the weapon pool's). Weapons test their current and
/// previous positions like fighter attacks.
pub fn weapon_hit(victim: &mut Fighter, w: WeaponAttack) -> WeaponContact {
    let state = if w.pos_curr == w.pos_prev {
        AttackState::Transfer
    } else {
        AttackState::Interpolate
    };
    let range =
        crate::motion::combat_attrs(victim.kind).map_or([f32::MAX; 3], |a| a.hit_detect_range);
    if !(in_range(w.pos_curr, victim.pos, range, w.hitbox.radius)
        || in_range(w.pos_prev, victim.pos, range, w.hitbox.radius))
    {
        return WeaponContact::Missed;
    }
    if is_shield(victim)
        && w.can_shield
        && attack_hits_shield(w.pos_curr, w.pos_prev, w.hitbox.radius, state, victim)
    {
        // `ftMainUpdateShieldStatWeapon`.
        victim.hits.shield_damage_total += w.hitbox.damage + w.hitbox.shield_damage;
        if victim.hits.shield_damage < w.hitbox.damage {
            victim.hits.shield_damage = w.hitbox.damage;
            victim.hits.shield_lr = match w.source {
                HitSource::Weapon { vel_x } => {
                    if vel_x < 0.0 {
                        1.0
                    } else {
                        -1.0
                    }
                }
                HitSource::Direct { lr } => lr,
                HitSource::Fighter { .. } | HitSource::Position => {
                    attack::damage_lr(victim.pos, w.pos_curr)
                }
            };
        }
        return WeaponContact::Shielded;
    }
    if is_body_intangible(victim) {
        return WeaponContact::Missed;
    }
    let Some(hit) =
        crate::hurtbox::search_attack(victim, w.pos_curr, w.pos_prev, w.hitbox.radius, state)
    else {
        return WeaponContact::Missed;
    };
    // `ftMainUpdateDamageStatWeapon`.
    let damage = attack::captured_damage(victim, w.hitbox.damage);
    if is_body_normal(victim)
        && hit.hitstatus == HitStatus::Normal
        && check_get_update_damage(victim, damage)
    {
        push_log(
            victim,
            HitLogEntry {
                source: w.source,
                hitbox: w.hitbox,
                attacker_pos: w.pos_curr,
                attack_handicap: w.handicap,
                placement: hit.placement,
            },
        );
        return WeaponContact::Hurt(true);
    }
    WeaponContact::Hurt(false)
}

/// Queues a hit that bypasses collision (a scripted or held-object hit), as
/// a weapon or fighter would, and returns whether it will deal damage.
pub fn direct_hit(
    victim: &mut Fighter,
    hitbox: attack::Hitbox,
    attacker_pos: Vec3,
    lr: f32,
    handicap: u8,
) -> bool {
    if !is_body_normal(victim) {
        return false;
    }
    let damage = attack::captured_damage(victim, hitbox.damage);
    check_get_update_damage(victim, damage);
    push_log(
        victim,
        HitLogEntry {
            source: HitSource::Direct { lr },
            hitbox,
            attacker_pos,
            attack_handicap: handicap,
            placement: attack::DAMAGE_INDEX_N,
        },
    );
    true
}

/// `ftMainProcessHitCollisionStatsMain`: the logged hit with the most
/// knockback against the frame's whole `damage_queue` sets the damage
/// angle, element, side and placement. Call after every search of `this`.
pub fn process_hit_collision(this: &mut Fighter) {
    if this.hits.log_len == 0 {
        return;
    }
    let mut best = -1.0_f32;
    let mut index = 0;
    for i in 0..this.hits.log_len {
        let Some(entry) = this.hits.log[i] else {
            continue;
        };
        let h = entry.hitbox;
        let kb = attack::knockback(
            this.damage,
            this.hits.damage_queue,
            h.damage,
            h.kb_weight,
            h.kb_scale,
            h.kb_base,
            this.attributes.weight,
            entry.attack_handicap,
            this.handicap,
        );
        if best < kb {
            best = kb;
            index = i;
        }
    }
    let Some(entry) = this.hits.log[index] else {
        return;
    };
    this.hits.damage_angle = entry.hitbox.angle;
    this.hits.damage_element = entry.hitbox.element;
    this.hits.damage_lr = match entry.source {
        HitSource::Fighter { .. } | HitSource::Position => {
            attack::damage_lr(this.pos, entry.attacker_pos)
        }
        HitSource::Weapon { vel_x } => {
            attack::HitDirection::from_weapon(this.pos, entry.attacker_pos, vel_x).damage_lr
        }
        HitSource::Direct { lr } => lr,
    };
    this.hits.damage_index = entry.placement;
    this.hits.damage_knockback = best;
    if this.hits.damage_element == Element::Electric {
        this.hits.hitlag_mul = 1.5;
    }
}

/// Attacker-side half of the electric rule: an electric attack slows its
/// user's hitlag too (`attacker_fp->hitlag_mul = 1.5`).
pub fn apply_attacker_element(attacker: &mut Fighter, victim: &Fighter) {
    if let Some(entry) = victim.hits.log[..victim.hits.log_len]
        .iter()
        .flatten()
        .find(|e| matches!(e.source, HitSource::Fighter { port } if port == attacker.port))
    {
        if victim.hits.damage_element == Element::Electric
            && entry.hitbox.element == Element::Electric
        {
            attacker.hits.hitlag_mul = 1.5;
        }
    }
}

/// `FTSTATUS_*` shield heal cap and interval.
const SHIELD_HEALTH_HEAL_MAX: f32 = 55.0;
const SHIELD_BREAK_RESET: f32 = 30.0;

/// `ftMainProcParams`. Call once per frame for every fighter, after every
/// fighter's search. Returns whether this fighter's own attack landed
/// (`proc_hit`).
pub fn proc_params(f: &mut Fighter) -> bool {
    let mut damage = 0;
    let mut is_shieldbreak = false;
    let status_before = f.status.status;
    let mut is_knockback_paused = false;
    let mut proc_hit = false;

    if !is_shield(f) && f.guard.shield_health < SHIELD_HEALTH_HEAL_MAX {
        f.guard.heal_wait -= 1.0;
        if f.guard.heal_wait == 0.0 {
            f.guard.shield_health += 1.0;
            f.guard.heal_wait = status::GUARD_HEAL_INTERVAL;
        }
    }
    f.guard.shield_health -= f.hits.shield_damage_total as f32;
    if f.guard.shield_health <= 0.0 {
        f.guard.shield_health = SHIELD_BREAK_RESET;
        is_shieldbreak = true;
    }
    if f.hits.damage_knockback != 0.0 {
        if matches!(
            status_before,
            AnyStatus::Common(Status::Squat | Status::SquatWait)
        ) {
            f.hits.damage_knockback *= 2.0 / 3.0;
        }
        let resist = f.knockback_resist.max(f.knockback_resist_passive);
        f.hits.damage_knockback -= resist;
        if f.hits.damage_knockback <= 0.0 {
            f.hits.damage_knockback = 0.0;
        }
        f.add_damage(f.hits.damage_queue);
        match f.hits.damage_kind {
            DamageKind::None => {}
            DamageKind::Status => goto_damage_status(f),
            DamageKind::ColAnim => {}
            DamageKind::Catch => update_catch_resist(f),
            DamageKind::Default => update_main(f),
        }
        damage = f.hits.damage_lag;
        is_knockback_paused = true;
    } else if f.hits.shield_damage != 0 {
        if is_shieldbreak {
            crate::reaction::set_shield_break_fly(f);
        } else {
            status::set_guard_set_off(f, f.hits.shield_damage as f32, f.hits.shield_lr);
        }
        damage = f.hits.shield_damage;
    } else if f.hits.attack_shield_push != 0 {
        if f.hits.attack_rebound != 0.0 && f.grab.catch.is_none() && f.grab.capture.is_none() {
            crate::reaction::set_rebound_wait(f, f.hits.attack_rebound, f.hits.hit_lr);
        }
        damage = f.hits.attack_shield_push;
    } else if f.hits.attack_damage != 0 {
        // `proc_hit`.
        proc_hit = true;
        crate::link::on_attack_hit(f);
        crate::captain::on_kick_hit(f);
        crate::capture_kirby::on_star_hit(f);
        damage = f.hits.attack_damage;
    }
    if damage != 0 {
        f.hitlag = hitlag_frames(damage, status_before, f.hits.hitlag_mul);
        if f.hitlag != 0 && is_knockback_paused {
            f.is_knockback_paused = true;
        }
        f.clear_taps();
    }
    f.hits = FrameHits::default();
    proc_hit
}

/// `ftParamGetHitLag` (US), with the electric multiplier.
pub fn hitlag_frames(damage: i32, status: AnyStatus, hitlag_mul: f32) -> u16 {
    let mut tics = ((((damage as f32) * (1.0 / 3.0)) + 5.0) as i32 as f32 * hitlag_mul) as i32;
    if matches!(status, AnyStatus::Common(Status::Squat | Status::SquatWait)) {
        tics = (tics as f32 * (2.0 / 3.0)) as i32;
    }
    tics.max(0) as u16
}

/// `ftCommonDamageCheckCatchResist`'s shared test: a hit that only flashes.
fn only_flashes(f: &Fighter) -> bool {
    // A sleep hit always leaves the current status.
    if f.hits.damage_element == Element::Sleep {
        return false;
    }
    f.hits.damage_knockback == 0.0
        || (f.hitlag > 0
            && f.is_knockback_paused
            && f.hits.damage_knockback < f.damage_knockback_stack + 30.0)
}

/// `ftCommonDamageUpdateCatchResist`: Donkey Kong's cargo stance holds on
/// unless the hit launches.
fn update_catch_resist(f: &mut Fighter) {
    if !only_flashes(f) {
        let (kb, angle, lr) = (
            f.hits.damage_knockback,
            f.hits.damage_angle,
            f.hits.damage_lr,
        );
        crate::grab::set_donkey_throwf_damage(f, kb, angle, lr);
    }
}

/// `ftCommonDamageGotoDamageStatus`.
pub fn goto_damage_status(f: &mut Fighter) {
    // `is_cliff_hold`: set by the ledge-hang statuses.
    if matches!(
        f.status.status,
        AnyStatus::Common(Status::CliffCatch | Status::CliffWait)
    ) {
        f.cliffcatch_wait = status::CLIFF_CATCH_WAIT;
    }
    let (kb, angle, lr, index, element) = (
        f.hits.damage_knockback,
        f.hits.damage_angle,
        f.hits.damage_lr,
        f.hits.damage_index,
        f.hits.damage_element,
    );
    if f.kind == FighterKind::Donkey {
        f.donkey_special_n.charge_level = 0;
    }
    if f.kind == FighterKind::Samus {
        crate::samus::on_damage(f);
    }
    if f.kind == FighterKind::Link {
        crate::link::on_damage(f);
    }
    crate::yoshi::on_damage(f);
    crate::pikachu::on_damage(f);
    if crate::kirby::is_kirby(f.kind) {
        crate::kirby_copy::on_damage(f);
    }
    if element == Element::Sleep {
        status::set_fura_sleep(f);
        return;
    }
    attack::init_damage_vars_full(f, None, kb, angle, lr, index, element, true);
}

/// `ftCommonDamageUpdateMain`, for a fighter that holds or is held by
/// another, then the ordinary case.
fn update_main(f: &mut Fighter) {
    if f.status.status == Status::YoshiEgg {
        crate::capture_yoshi::on_hit(f, f.hits.damage_queue);
        return;
    }
    if f.grab.catch.is_some() {
        // The partner's own knockback is not visible from here; the
        // catcher's reaction follows the source's single-hit branches.
        if crate::grab::is_cargo(f.status.status) && only_flashes_cargo(f) {
            update_catch_resist(f);
            return;
        }
        if only_flashes(f) {
            return;
        }
        crate::grab::release_on_hit(f);
        goto_damage_status(f);
        return;
    }
    if f.grab.capture.is_some() {
        if attack::capture_keep_hold(f.hits.damage_queue) {
            // The hold survives; the catcher takes the hitlag
            // (`grab_fp->hitlag_tics`), delivered through the grab link.
            crate::grab::send_catcher_hitlag(f, f.hits.damage_lag);
            return;
        }
        crate::grab::release_on_capture_hit(f);
        goto_damage_status(f);
        return;
    }
    if only_flashes(f) {
        return;
    }
    goto_damage_status(f);
}

/// `ftCommonDamageCheckCatchResist` for Donkey Kong's cargo statuses.
fn only_flashes_cargo(f: &Fighter) -> bool {
    if f.hits.damage_element == Element::Sleep {
        return false;
    }
    only_flashes(f) || attack::damage_level(attack::hitstun_frames(f.hits.damage_knockback)) < 3
}

/// One fighter's end of frame: its hit log, then `ftMainProcParams`.
/// Returns whether its own attack landed.
pub fn resolve(f: &mut Fighter) -> bool {
    process_hit_collision(f);
    proc_params(f)
}

/// Runs the whole pipeline for a set of fighters and returns, per fighter,
/// whether its own attack landed (`proc_hit`). Weapons must be applied
/// between the searches and [`finish_frame`]; this helper is for callers
/// without a weapon pool.
pub fn resolve_frame(fighters: &mut [&mut Fighter]) -> [bool; 4] {
    search_all(fighters);
    finish_frame(fighters)
}

/// The search pass: attack positions, then every pair.
pub fn search_all(fighters: &mut [&mut Fighter]) {
    for f in fighters.iter_mut() {
        update_attack_positions(f);
    }
    let n = fighters.len();
    for this in 0..n {
        for other in 0..n {
            if this == other {
                continue;
            }
            let (a, b) = pair_mut(fighters, this, other);
            search_fighter_hits(a, b, other > this);
        }
    }
}

/// Processes each fighter's hit log, then runs `ftMainProcParams` for all.
pub fn finish_frame(fighters: &mut [&mut Fighter]) -> [bool; 4] {
    for f in fighters.iter_mut() {
        process_hit_collision(f);
    }
    let n = fighters.len();
    for this in 0..n {
        for other in 0..n {
            if this != other {
                let (a, b) = pair_mut(fighters, this, other);
                apply_attacker_element(a, b);
            }
        }
    }
    let mut landed = [false; 4];
    for (i, f) in fighters.iter_mut().enumerate() {
        let hit = proc_params(f);
        if i < 4 {
            landed[i] = hit;
        }
    }
    landed
}

fn pair_mut<'a>(
    fighters: &'a mut [&mut Fighter],
    a: usize,
    b: usize,
) -> (&'a mut Fighter, &'a mut Fighter) {
    assert_ne!(a, b);
    if a < b {
        let (lo, hi) = fighters.split_at_mut(b);
        (&mut *lo[a], &mut *hi[0])
    } else {
        let (lo, hi) = fighters.split_at_mut(a);
        (&mut *hi[0], &mut *lo[b])
    }
}
