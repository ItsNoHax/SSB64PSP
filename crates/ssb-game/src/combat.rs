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
use crate::effect::{HitEffect, HitEffectKind, HitEffectSink};
use crate::fighter::{Fighter, FighterKind, JointTransform};
use crate::stale::MotionAttackId;
use crate::status::{self, AnyStatus, Status};
use crate::team::TeamRules;

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
pub(crate) const NO_GROUP: u8 = 7;

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
    pub(crate) fn is_clear(&self) -> bool {
        !self.is_interact_hurt && !self.is_interact_shield && self.group_id == NO_GROUP
    }
}

/// Attack-record victim ids at and above this name a weapon-pool slot
/// (`victim_gobj` is a weapon); fighters use their port.
pub const WEAPON_RECORD_BASE: u8 = 0x80;

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
    /// `fgm_level`: the hit sound's strength, which also gates the orbs and
    /// sparks of a normal hit.
    pub fgm_level: u8,
    /// `fgm_kind`: the hit sound's row of
    /// [`crate::fighter_sound::HIT_COLLISION_FGMS`].
    pub fgm_kind: u8,
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

    pub(crate) fn record(&self, victim: u8) -> AttackRecord {
        self.records
            .iter()
            .find(|r| r.victim == Some(victim))
            .copied()
            .unwrap_or_default()
    }

    /// Whether this collision may touch a fighter in situation `airborne`.
    pub(crate) fn reaches(&self, airborne: bool) -> bool {
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
pub(crate) enum HitType {
    Damage,
    Shield,
    Attack(u8),
}

/// `ftMainSetHitInteractStats`: records the contact on every live collision
/// of the group, then drops those collisions from `detect` for the rest of
/// this search.
pub(crate) fn set_hit_interact(
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
    pub stat: crate::spgame::live::AttackStat,
    pub object: crate::spgame::bonus::DamageObject,
    pub source: HitSource,
    pub hitbox: attack::Hitbox,
    pub attacker_pos: Vec3,
    pub attack_handicap: u8,
    /// The hurtbox's `placement` column.
    pub placement: usize,
    /// Who `ftParamUpdate1PGameDamageStats` records as `damage_player`
    /// when this hit wins the frame.
    pub attacker: DamageBy,
    /// What `ftMainProcessHitCollisionStatsMain` makes its hit effect from;
    /// `None` for a hit that makes none (a weapon without
    /// `is_hitlag_victim`, the stage).
    pub effect: Option<LogEffect>,
}

/// A logged hit's hit-effect inputs.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LogEffect {
    /// The impact point: halfway between the attack
    /// (`gmCollisionGetFighterAttackPosition`) and the damage box's centre
    /// (`gmCollisionGetCommonImpactPosition`).
    pub pos: Vec3,
    /// `hitlog->attacker_player`.
    pub player: u8,
    /// A fighter's attack, whose switch has the slash case and the orbs.
    pub from_fighter: bool,
    /// `attack_coll->fgm_level`.
    pub fgm_level: u8,
    /// `gmCollisionGetDamageSlashRotation`.
    pub slash_rotate: f32,
}

/// `gmCollisionGetFighterAttackPosition`: a new attack's point, else the
/// middle of its sweep.
pub fn attack_point(pos_curr: Vec3, pos_prev: Vec3, state: AttackState) -> Vec3 {
    if state == AttackState::Transfer {
        pos_curr
    } else {
        (pos_curr + pos_prev) * 0.5
    }
}

/// `gmCollisionGetCommonImpactPosition`.
pub fn impact_point(a: Vec3, b: Vec3) -> Vec3 {
    (a + b) * 0.5
}

/// The hit effects one frame of searches and hit processing queues for a
/// fighter, in the order the source makes them.
pub const EFFECT_QUEUE_MAX: usize = 48;

/// The `damage_player` a logged hit leaves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DamageBy {
    /// A player's fighter, weapon or item.
    Player(u8),
    /// `GMCOMMON_PLAYERS_MAX`: the stage, or a fighter's own weapon or item.
    World,
    /// Acid: `damage_player` stays as it was.
    Keep,
}

impl DamageBy {
    /// A weapon or item hit: its owner, unless the owner is the victim.
    pub fn owner(owner: Option<u8>, victim: u8) -> DamageBy {
        match owner {
            Some(p) if p != victim => DamageBy::Player(p),
            _ => DamageBy::World,
        }
    }
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
    /// `reflect_lr`: a reflector turned a weapon back (`+1` when the weapon
    /// was to the fighter's right).
    pub reflect_lr: f32,
    /// `reflect_damage`: a weapon beat the reflector's `damage_resist`.
    pub reflect_damage: i32,
    /// `absorb_lr`: PSI Magnet took a weapon.
    pub absorb_lr: f32,
    /// The hit effects this fighter's search and hit processing made
    /// (`efManagerSetOffMakeEffect`, `efManagerDamage*MakeEffect`), for
    /// [`finish_frame_with`] to hand on.
    pub effects: [Option<HitEffect>; EFFECT_QUEUE_MAX],
    pub effects_len: usize,
    /// Thrown-body hits credit the thrower's stale queue. One slot per
    /// attack collision, flushed between fighter pairs in the search pass.
    pub(crate) thrown_stale: [Option<(u8, MotionAttackId, u16)>; 4],
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
            reflect_lr: 0.0,
            reflect_damage: 0,
            absorb_lr: 0.0,
            effects: [None; EFFECT_QUEUE_MAX],
            effects_len: 0,
            thrown_stale: [None; 4],
        }
    }
}

impl FrameHits {
    /// Queues one hit effect. The queue outsizes a frame's worst case
    /// (ten logged hits of three effects each, and the clanks).
    pub fn push_effect(&mut self, e: HitEffect) {
        debug_assert!(self.effects_len < EFFECT_QUEUE_MAX, "hit effect queue full");
        if self.effects_len < EFFECT_QUEUE_MAX {
            self.effects[self.effects_len] = Some(e);
            self.effects_len += 1;
        }
    }

    /// Queues `efManagerSetOffMakeEffect(pos, damage)`.
    pub fn push_set_off(&mut self, pos: Vec3, damage: i32) {
        self.push_effect(HitEffect {
            kind: HitEffectKind::SetOff,
            pos,
            player: 0,
            damage,
        });
    }
}

/// `nFTSpecialCollKind`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpecialCollKind {
    FoxReflector,
    NessAbsorb,
    NessReflector,
}

/// `FTSpecialColl`: the reflector or absorber sphere a status attaches
/// (`fp->special_coll`), tested against weapons with
/// `gmCollisionCheckWeaponAttackSpecialCollide`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpecialColl {
    pub kind: SpecialCollKind,
    /// `FTStruct::joints` index.
    pub joint: u8,
    pub offset: Vec3,
    pub size: Vec3,
    /// A weapon doing more damage than this breaks the reflector.
    pub damage_resist: i32,
}

/// `dFoxMainMotion_LwReflectorFTSpecialColl`.
pub const FOX_REFLECTOR: SpecialColl = SpecialColl {
    kind: SpecialCollKind::FoxReflector,
    joint: 4,
    offset: Vec3::new(0.0, 60.0, 0.0),
    size: Vec3::new(350.0, 350.0, 350.0),
    damage_resist: 50,
};
/// `dNessMainMotion_AttackS4ReflectorFTSpecialColl`.
pub const NESS_BAT_REFLECTOR: SpecialColl = SpecialColl {
    kind: SpecialCollKind::NessReflector,
    joint: 0,
    offset: Vec3::new(0.0, 150.0, 0.0),
    size: Vec3::new(300.0, 300.0, 300.0),
    damage_resist: 1000,
};
/// `dNessMainMotion_LwAbsorbFTSpecialColl` (US size 430; JP 400).
pub const NESS_ABSORB: SpecialColl = SpecialColl {
    kind: SpecialCollKind::NessAbsorb,
    joint: 0,
    offset: Vec3::new(300.0, 195.0, 0.0),
    size: Vec3::new(430.0, 430.0, 430.0),
    damage_resist: 0,
};

/// `is_reflect` with its `special_coll`: Fox's reflector statuses
/// (`ftFoxSpecialLw{Loop,Turn,Hit}` set it) and Ness's forward smash while
/// its script's flag 1 is up (`ftCommonAttackS4ProcUpdate`). `ftMainSetStatus`
/// clears the flag, so any other status has none.
pub fn reflector(f: &Fighter) -> Option<SpecialColl> {
    use crate::status::FoxStatus as Fx;
    match f.status.status {
        AnyStatus::Fox(
            Fx::SpecialLwLoop
            | Fx::SpecialLwTurn
            | Fx::SpecialLwHit
            | Fx::SpecialAirLwLoop
            | Fx::SpecialAirLwTurn
            | Fx::SpecialAirLwHit,
        ) => Some(FOX_REFLECTOR),
        AnyStatus::Common(Status::AttackS4)
            if base_kind(f.kind) == FighterKind::Ness && f.motion_script.flags[1] != 0 =>
        {
            Some(NESS_BAT_REFLECTOR)
        }
        _ => None,
    }
}

/// `is_absorb` with its `special_coll`: PSI Magnet's hold and hit statuses.
pub fn absorber(f: &Fighter) -> Option<SpecialColl> {
    crate::ness::absorbing(f).then_some(NESS_ABSORB)
}

fn base_kind(kind: FighterKind) -> FighterKind {
    kind.character()
}

/// The special collision's joint, or TopN's facing transform at the root
/// when the caller supplied no pose (host tests).
fn special_transform(f: &Fighter, joint: u8) -> JointTransform {
    f.joint_transforms
        .get(joint as usize)
        .copied()
        .flatten()
        .unwrap_or_else(|| {
            let s = f.facing.sign();
            JointTransform {
                axes: [
                    Vec3::new(0.0, 0.0, -s),
                    Vec3::new(0.0, 1.0, 0.0),
                    Vec3::new(s, 0.0, 0.0),
                ],
                origin: f.pos,
            }
        })
}

/// `gmCollisionCheckWeaponAttackSpecialCollide`: the weapon's sphere (swept
/// from `pos_prev`) against the special collision's ellipsoid.
pub fn special_contact(
    f: &Fighter,
    coll: &SpecialColl,
    pos_curr: Vec3,
    pos_prev: Vec3,
    radius: f32,
) -> bool {
    let t = special_transform(f, coll.joint);
    crate::hurtbox::test_sphere(
        &t,
        pos_curr,
        pos_prev,
        radius,
        weapon_state(pos_curr, pos_prev),
        coll.offset,
        coll.size,
    )
}

/// A weapon hitbox's attack state: a still hitbox tests its position only.
pub(crate) fn weapon_state(pos_curr: Vec3, pos_prev: Vec3) -> AttackState {
    if pos_curr == pos_prev {
        AttackState::Transfer
    } else {
        AttackState::Interpolate
    }
}

/// `damage_lr`-style side of a weapon: `+1` when it is to the fighter's
/// right (`fp->reflect_lr`, `fp->absorb_lr`).
pub fn weapon_side(f: &Fighter, weapon_pos: Vec3) -> f32 {
    if f.pos.x < weapon_pos.x {
        1.0
    } else {
        -1.0
    }
}

/// The outcome of a weapon meeting a reflector
/// (`ftMainUpdateReflectorStatWeapon`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReflectOutcome {
    /// Turned back: the weapon changes owner.
    Reflected,
    /// Too strong: the reflector breaks and the weapon takes it as a
    /// normal hit (`hit_normal_damage`).
    Broke,
}

/// `ftMainUpdateReflectorStatWeapon`, the fighter's half. `damage` is the
/// weapon's staled damage.
pub fn reflect_weapon(
    f: &mut Fighter,
    coll: &SpecialColl,
    damage: i32,
    weapon_pos: Vec3,
) -> ReflectOutcome {
    f.hits.reflect_lr = weapon_side(f, weapon_pos);
    if coll.damage_resist < damage {
        f.hits.reflect_damage = damage;
        ReflectOutcome::Broke
    } else {
        ReflectOutcome::Reflected
    }
}

/// `ftMainUpdateAbsorbStatWeapon`, the fighter's half: heals twice the
/// weapon's staled damage at once (`can_not_heal` is never set).
pub fn absorb_weapon(f: &mut Fighter, damage: i32, weapon_pos: Vec3) {
    f.hits.absorb_lr = weapon_side(f, weapon_pos);
    let heal = (damage as f32 * 2.0) as i32;
    f.damage = (i32::from(f.damage) - heal).max(0) as u16;
}

/// `ftMainSearchHitWeapon`'s attack-versus-weapon branch for one weapon
/// hitbox: the fighter's live attack collisions that reach the weapon's
/// situation and have not recorded it (`weapon_id`) are tested against it.
/// On contact, `ftMainUpdateAttackStatWeapon`: a weapon doing more than the
/// attack's damage minus 10 records itself on the attack and rebounds the
/// fighter; an attack doing more than the weapon's damage minus 10 beats the
/// weapon (`hit_attack_damage`). Returns whether the weapon lost, which ends
/// its search against this fighter.
pub fn weapon_attack_clank(
    f: &mut Fighter,
    w: &WeaponAttack,
    weapon_id: u8,
    weapon_grounded: bool,
) -> bool {
    let mut detect = [false; 4];
    for (i, coll) in f.attack_colls.iter().enumerate() {
        detect[i] = coll.state != AttackState::Off
            && coll.reaches(!weapon_grounded)
            && coll.record(weapon_id).group_id == NO_GROUP;
    }
    if !detect.contains(&true) {
        return false;
    }
    let damage = w.hitbox.damage;
    let w_state = weapon_state(w.pos_curr, w.pos_prev);
    for j in 0..f.attack_colls.len() {
        if !detect[j] {
            continue;
        }
        let coll = f.attack_colls[j];
        if !crate::hurtbox::attacks_collide(
            (w.pos_curr, w.pos_prev, w.hitbox.radius, w_state),
            (coll.pos_curr, coll.pos_prev, coll.size, coll.state),
        ) {
            continue;
        }
        // `ftMainUpdateAttackStatWeapon`: a set-off for each side that
        // stops, at `gmCollisionGetWeaponAttackFighterAttackPosition`.
        let impact = impact_point(
            attack_point(w.pos_curr, w.pos_prev, w_state),
            attack_point(coll.pos_curr, coll.pos_prev, coll.state),
        );
        if coll.damage - 10 < damage {
            let mut scratch = detect;
            set_hit_interact(f, coll.group, weapon_id, HitType::Attack(0), &mut scratch);
            set_hit_rebound(f, &coll, w.pos_curr.x);
            f.hits.push_set_off(impact, coll.damage);
        }
        if damage - 10 < coll.damage {
            f.hits.push_set_off(impact, damage);
            return true;
        }
    }
    false
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
    special_hitstatus(f) == HitStatus::Normal
        && star_hitstatus(f) == HitStatus::Normal
        && body_hitstatus(f) == HitStatus::Normal
}

/// `FTStruct::star_hitstatus`: only the Star sets it, to invincible.
pub fn star_hitstatus(f: &Fighter) -> HitStatus {
    if f.star_invincible_frames > 0 {
        HitStatus::Invincible
    } else {
        HitStatus::Normal
    }
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

/// `FTStruct::throw_gobj`: common throws and Kirby's spit/copy stars.
pub(crate) fn throw_port(f: &Fighter) -> Option<u8> {
    f.thrown.owner.map(|owner| owner.port)
}

/// `FTStruct::throw_team`, while [`throw_port`] is set.
pub(crate) fn throw_team(f: &Fighter) -> Option<u8> {
    f.thrown.owner.map(|owner| owner.team)
}

/// `(fp->throw_gobj != NULL) ? fp->throw_team : fp->team`: the team a
/// thrown fighter's attacks count for.
pub(crate) fn hit_team(f: &Fighter) -> u8 {
    throw_team(f).unwrap_or(f.team)
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
pub(crate) fn in_range(pos: Vec3, target: Vec3, range: [f32; 3], size: f32) -> bool {
    let dx = pos.x - target.x;
    let dy = pos.y - target.y;
    !(dx < -range[2] - size
        || dx > range[2] + size
        || dy < -range[1] - size
        || dy > range[0] + size)
}

/// `gmCollisionCheckWeaponInFighterRange`: the weapon hitbox (either end of
/// its sweep) within the fighter's `hit_detect_range`.
pub fn weapon_in_range(f: &Fighter, w: &WeaponAttack) -> bool {
    let range = crate::motion::combat_attrs(f.kind).map_or([f32::MAX; 3], |a| a.hit_detect_range);
    in_range(w.pos_curr, f.pos, range, w.hitbox.radius)
        || in_range(w.pos_prev, f.pos, range, w.hitbox.radius)
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

/// `dEFManagerShieldColors`' damage row: `ftCommonGuardSetOffSetStatus`'s
/// `is_damage_shield` selects it.
pub const SHIELD_COLOR_DAMAGE_ROW: usize = 4;

/// The `dEFManagerShieldColors` row `efManagerShieldProcDisplay` draws the
/// bubble in: the damage row on a set-off's frame, else the player's
/// (RE-384, RE-418).
pub fn shield_color_row(f: &Fighter) -> usize {
    if f.guard.is_damage_shield {
        SHIELD_COLOR_DAMAGE_ROW
    } else {
        usize::from(f.port).min(3)
    }
}

/// Whether Yoshi is inside his egg shield: `efManagerYoshiShieldMakeEffect`
/// and `ftParamHideModelPartAll` run together, and every path that raises
/// `is_shield` for Yoshi makes the egg, while `ftCommonGuardUpdateShieldVars`
/// and a status change without `FTSTATUS_PRESERVE_MODELPART` restore his
/// parts as they drop it. So his model is hidden, and the egg drawn, exactly
/// while this holds (RE-418).
pub fn is_yoshi_egg_shield(f: &Fighter) -> bool {
    f.kind == FighterKind::Yoshi && f.guard.is_shield
}

/// `efManagerYoshiShieldProcDisplay`'s ENV (alpha 0): (0xAE, 0xD6, 0xD6)
/// scaled by `1 - shield_health / 55`, clamped at zero. The egg's combiner
/// subtracts it from the shade, so the egg darkens (green and blue
/// faster than red) as the shield wears
/// (RE-418).
pub fn yoshi_shield_env(f: &Fighter) -> [u8; 3] {
    // The display's own literal 55.0F, equal to `FTCOMMON_GUARD_SIZE_HEALTH_DIV`.
    let blend = (1.0 - f.guard.shield_health / 55.0).max(0.0);
    [
        (f32::from(0xAEu8) * blend) as u8,
        (f32::from(0xD6u8) * blend) as u8,
        (f32::from(0xD6u8) * blend) as u8,
    ]
}

/// `efManagerYoshiShieldMakeEffect`'s DObj scale (X and Y); its kind-0x50
/// root adds only `YRotN`'s world translation, so the egg keeps this size.
pub const YOSHI_SHIELD_SCALE: f32 = 1.5;

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
pub(crate) fn check_get_update_damage(f: &mut Fighter, damage: i32) -> bool {
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

pub(crate) fn push_log(f: &mut Fighter, entry: HitLogEntry) {
    if f.hits.log_len < HIT_LOG_MAX {
        f.hits.log[f.hits.log_len] = Some(entry);
        f.hits.log_len += 1;
    }
}

/// `ftMainSetHitRebound`.
pub(crate) fn set_hit_rebound(fp: &mut Fighter, coll: &AttackColl, victim_x: f32) {
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
pub fn search_fighter_hits(
    this: &mut Fighter,
    other: &mut Fighter,
    other_after_this: bool,
    rules: TeamRules,
) {
    // `ftMainProcSearchHitAll` skips a ghost victim.
    if this.dead.is_ghost {
        return;
    }
    if this.port == other.port || this.grab.capture == Some(other.port) {
        return;
    }
    // Team attack off: a teammate's attacks, or those of a fighter a
    // teammate threw, pass through.
    if rules.spares(hit_team(other), this.team) || is_catchstatus(other) {
        return;
    }
    // A thrown fighter never hits the fighter that threw it.
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
        && throw_team(this).is_none_or(|team| {
            throw_port(this) != Some(other.port) && !rules.spares(hit_team(other), team)
        })
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
    // `gmCollisionGetFighterAttacksPosition`. Both set-offs are made in
    // `this` fighter's search.
    let impact = impact_point(
        attack_point(this_hit.pos_curr, this_hit.pos_prev, this_hit.state),
        attack_point(other_hit.pos_curr, other_hit.pos_prev, other_hit.state),
    );
    if this_hit.damage - 10 < other_hit.damage {
        set_hit_interact(
            this,
            this_hit.group,
            other.port,
            HitType::Attack(other_hit.group),
            attack_detect,
        );
        set_hit_rebound(this, &this_hit, other.pos.x);
        this.hits.push_set_off(impact, this_hit.damage);
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
        this.hits.push_set_off(impact, other_hit.damage);
    }
}

/// `ftMainUpdateShieldStatFighter`.
/// `gmCollisionGet{Fighter,Weapon,Item}AttackShieldPosition`: halfway
/// from the attack's point to the shield joint's world point at the
/// fighter's depth (`gmCollisionGetShieldPosition`,
/// `gmCollisionGetCommonImpactPosition`).
pub fn shield_impact(victim: &Fighter, attack: Vec3) -> Vec3 {
    let mut shield = victim.joint_transforms[JOINT_YROTN].map_or(victim.pos, |t| t.origin);
    shield.z = victim.pos.z;
    impact_point(attack, shield)
}

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
        victim.guard.shield_player = Some(attacker.port);
        victim.hits.shield_damage = coll.damage;
        victim.hits.shield_lr = if victim.pos.x < attacker.pos.x {
            1.0
        } else {
            -1.0
        };
    }
    // `gmCollisionGetFighterAttackShieldPosition`.
    let pos = shield_impact(
        victim,
        attack_point(coll.pos_curr, coll.pos_prev, coll.state),
    );
    victim.hits.push_set_off(pos, coll.damage);
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
    // `gmCollisionGetFighterAttackDamagePosition`.
    let impact = impact_point(
        attack_point(coll.pos_curr, coll.pos_prev, coll.state),
        hit.center,
    );
    let damage_before = victim.hits.damage_queue;
    if is_body_normal(victim)
        && hit.hitstatus == HitStatus::Normal
        && check_get_update_damage(victim, damage)
    {
        // The thrower's player for a thrown fighter's attack.
        let player = throw_port(attacker).unwrap_or(attacker.port);
        // Source CheckGet updates its damage pointer when resistance breaks.
        victim.record_combo_damage(Some(player), victim.hits.damage_queue - damage_before);
        push_log(
            victim,
            HitLogEntry {
                stat: crate::spgame::live::AttackStat {
                    flags: attacker.stats.attack.flags.body(),
                    ..attacker.stats.attack
                },
                object: crate::spgame::bonus::DamageObject::Other,
                source: HitSource::Fighter {
                    port: attacker.port,
                },
                hitbox: coll.hitbox(),
                attacker_pos: attacker.pos,
                attack_handicap: attacker.handicap,
                placement: hit.placement,
                attacker: DamageBy::Player(player),
                effect: Some(LogEffect {
                    pos: impact,
                    player,
                    from_fighter: true,
                    fgm_level: coll.fgm_level,
                    slash_rotate: slash_rotation(attacker, &coll),
                }),
            },
        );
        if player != victim.port {
            if throw_port(attacker).is_some() {
                attacker.hits.thrown_stale[i] =
                    Some((player, coll.motion_attack_id, coll.motion_count));
            } else {
                attacker
                    .stale
                    .push(coll.motion_attack_id, coll.motion_count);
            }
        }
    } else {
        // An invincible body or box, or damage a resist soaked.
        victim.hits.push_set_off(impact, damage);
    }
    crate::fighter_sound::play_hit_sfx(attacker, coll.fgm_kind, coll.fgm_level);
}

/// `gmCollisionGetDamageSlashRotation`: the angle of the attacker's air
/// velocity for a new attack, else of the attack's sweep.
pub(crate) fn slash_rotation(attacker: &Fighter, coll: &AttackColl) -> f32 {
    let (x, y) = if coll.state == AttackState::Transfer {
        (attacker.physics.vel_air.x, attacker.physics.vel_air.y)
    } else {
        (
            coll.pos_curr.x - coll.pos_prev.x,
            coll.pos_curr.y - coll.pos_prev.y,
        )
    };
    crate::particle::arc_tan2(y, x)
}

/// `ftMainUpdateShieldStatWeapon`'s writes to the weapon:
/// `shield_collide_angle` and the sign of `shield_collide_dir.z`, which
/// `proc_hop` reads.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShieldCollide {
    pub angle: f32,
    pub dir_z: f32,
}

/// What a weapon hitbox did to a fighter (`ftMainSearchHitWeapon`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WeaponContact {
    Missed,
    /// Recorded by the shield.
    Shielded(ShieldCollide),
    /// Touched a hurtbox; `true` when it will deal damage.
    Hurt(bool),
}

/// A weapon's attack for [`weapon_hit`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WeaponAttack {
    pub stat: crate::spgame::live::AttackStat,
    pub object: crate::spgame::bonus::DamageObject,
    /// Damage already staled (`wpMainGetStaledDamage`).
    pub hitbox: attack::Hitbox,
    pub pos_curr: Vec3,
    pub pos_prev: Vec3,
    pub source: HitSource,
    pub handicap: u8,
    pub can_shield: bool,
    /// `wp->player`: the weapon's owner, for `damage_player`.
    pub owner: Option<u8>,
    /// `wp->is_hitlag_victim` (Link's Boomerang): the hit makes its spark,
    /// in the colour of this player (`wp->player`).
    pub is_hitlag_victim: Option<u8>,
}

/// `ftMainSearchHitWeapon`'s shield and damage halves for one weapon hitbox
/// (reflection is the weapon pool's). Weapons test their current and
/// previous positions like fighter attacks.
pub fn weapon_hit(victim: &mut Fighter, w: WeaponAttack) -> WeaponContact {
    weapon_hit_inner(victim, w, false)
}

/// Every attack box tests the shield before any tests a hurtbox
/// (`ftMainSearchHitWeapon`). Used by Ray Gun's head and growing tail.
pub(crate) fn weapon_hit_pair(
    victim: &mut Fighter,
    w: WeaponAttack,
    tail: Option<(Vec3, Vec3)>,
) -> WeaponContact {
    let attacks = [
        Some(w),
        tail.map(|(pos_curr, pos_prev)| WeaponAttack {
            pos_curr,
            pos_prev,
            ..w
        }),
    ];
    for a in attacks.into_iter().flatten() {
        let contact = weapon_hit_inner(victim, a, true);
        if matches!(contact, WeaponContact::Shielded(_)) {
            return contact;
        }
    }
    for a in attacks.into_iter().flatten() {
        let contact = weapon_hit_inner(
            victim,
            WeaponAttack {
                can_shield: false,
                ..a
            },
            false,
        );
        if contact != WeaponContact::Missed {
            return contact;
        }
    }
    WeaponContact::Missed
}

fn weapon_hit_inner(victim: &mut Fighter, w: WeaponAttack, shield_only: bool) -> WeaponContact {
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
    let shield = if is_shield(victim) && w.can_shield {
        crate::hurtbox::test_sphere_angle(
            &shield_transform(victim),
            w.pos_curr,
            w.pos_prev,
            w.hitbox.radius,
            state,
            Vec3::ZERO,
            Vec3::new(30.0, 30.0, 30.0),
        )
    } else {
        None
    };
    if let Some((angle, dir)) = shield {
        // `ftMainUpdateShieldStatWeapon`.
        victim.hits.shield_damage_total += w.hitbox.damage + w.hitbox.shield_damage;
        if victim.hits.shield_damage < w.hitbox.damage {
            victim.guard.shield_player = w.owner;
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
        // `shield_collide_dir = { 0, 0, lr == +1 ? -dir.x : dir.x }`,
        // normalised.
        let z = if victim.facing.sign() > 0.0 {
            -dir.x
        } else {
            dir.x
        };
        let dir_z = if z > 0.0 {
            1.0
        } else if z < 0.0 {
            -1.0
        } else {
            0.0
        };
        // `gmCollisionGetWeaponAttackShieldPosition`.
        let impact = shield_impact(victim, attack_point(w.pos_curr, w.pos_prev, state));
        victim
            .hits
            .push_set_off(impact, w.hitbox.shield_damage + w.hitbox.damage);
        return WeaponContact::Shielded(ShieldCollide { angle, dir_z });
    }
    if shield_only || is_body_intangible(victim) {
        return WeaponContact::Missed;
    }
    let Some(hit) =
        crate::hurtbox::search_attack(victim, w.pos_curr, w.pos_prev, w.hitbox.radius, state)
    else {
        return WeaponContact::Missed;
    };
    // `ftMainUpdateDamageStatWeapon`.
    let damage = attack::captured_damage(victim, w.hitbox.damage);
    let damage_before = victim.hits.damage_queue;
    if is_body_normal(victim)
        && hit.hitstatus == HitStatus::Normal
        && check_get_update_damage(victim, damage)
    {
        push_log(
            victim,
            HitLogEntry {
                stat: w.stat,
                object: w.object,
                source: w.source,
                hitbox: w.hitbox,
                attacker_pos: w.pos_curr,
                attack_handicap: w.handicap,
                placement: hit.placement,
                attacker: DamageBy::owner(w.owner, victim.port),
                // `is_hitlag_victim`: Link's Boomerang (and two Pokemon)
                // make the hit's spark, at
                // `gmCollisionGetWeaponAttackFighterDamagePosition`.
                effect: w.is_hitlag_victim.map(|player| LogEffect {
                    pos: impact_point(attack_point(w.pos_curr, w.pos_prev, state), hit.center),
                    player,
                    from_fighter: false,
                    fgm_level: 0,
                    slash_rotate: 0.0,
                }),
            },
        );
        victim.record_combo_damage(w.owner, victim.hits.damage_queue - damage_before);
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
    attacker: DamageBy,
) -> bool {
    direct_hit_with_stat(
        victim,
        hitbox,
        attacker_pos,
        lr,
        handicap,
        attacker,
        crate::spgame::live::AttackStat::default(),
        crate::spgame::bonus::DamageObject::Other,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn direct_hit_with_stat(
    victim: &mut Fighter,
    hitbox: attack::Hitbox,
    attacker_pos: Vec3,
    lr: f32,
    handicap: u8,
    attacker: DamageBy,
    stat: crate::spgame::live::AttackStat,
    object: crate::spgame::bonus::DamageObject,
) -> bool {
    if !is_body_normal(victim) {
        return false;
    }
    let damage = attack::captured_damage(victim, hitbox.damage);
    let damage_before = victim.hits.damage_queue;
    let accepted = check_get_update_damage(victim, damage);
    if accepted {
        victim.record_combo_damage(
            match attacker {
                DamageBy::Player(p) => Some(p),
                _ => None,
            },
            victim.hits.damage_queue - damage_before,
        );
    }
    push_log(
        victim,
        HitLogEntry {
            stat,
            object,
            source: HitSource::Direct { lr },
            hitbox,
            attacker_pos,
            attack_handicap: handicap,
            placement: attack::DAMAGE_INDEX_N,
            attacker,
            effect: None,
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
        if let Some(e) = entry.effect {
            queue_hit_effects(this, &e, h.element, h.damage, kb);
        }
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
    crate::spgame::live::hit(this, entry.attacker, entry.stat, entry.object);
    if this.hits.damage_element == Element::Electric {
        this.hits.hitlag_mul = 1.5;
    }
}

/// The hit effects of one logged hit, by its element
/// (`ftMainProcessHitCollisionStatsMain`'s switch): the fire, electric and
/// coin sparks, a fighter's slash, and otherwise the normal spark, light
/// under 180 knockback and heavy from it, with a fighter's orbs and sparks
/// (metal dust for a metal fighter) when its hit sound is above the weakest.
/// A weapon's or item's slash is a normal hit.
pub fn queue_hit_effects(
    this: &mut Fighter,
    e: &LogEffect,
    element: Element,
    damage: i32,
    knockback: f32,
) {
    let make = |kind| HitEffect {
        kind,
        pos: e.pos,
        player: e.player,
        damage,
    };
    match element {
        Element::Fire => this.hits.push_effect(make(HitEffectKind::Fire)),
        Element::Electric => this.hits.push_effect(make(HitEffectKind::Electric)),
        Element::Coin => this.hits.push_effect(make(HitEffectKind::Coin)),
        Element::Slash if e.from_fighter => this.hits.push_effect(make(HitEffectKind::Slash {
            rotate: e.slash_rotate,
        })),
        _ => {
            this.hits.push_effect(make(if knockback < 180.0 {
                HitEffectKind::NormalLight
            } else {
                HitEffectKind::NormalHeavy
            }));
            if e.from_fighter && e.fgm_level > 0 {
                this.hits.push_effect(make(HitEffectKind::SpawnOrbs));
                let lr = this.facing.sign();
                this.hits
                    .push_effect(make(if this.kind == FighterKind::MetalMario {
                        HitEffectKind::SpawnMDust { lr }
                    } else {
                        HitEffectKind::SpawnSparks { lr }
                    }));
            }
        }
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
    proc_params_with(f, None)
}

pub(crate) fn record_shield_break(f: &mut Fighter) {
    let player = f.guard.shield_player;
    crate::spgame::live::hit(
        f,
        player.map_or(DamageBy::World, DamageBy::Player),
        crate::spgame::live::AttackStat::default(),
        crate::spgame::bonus::DamageObject::Other,
    );
    if f.hits.shield_damage != 0 {
        f.stats
            .emit(crate::spgame::live::Event::ShieldBreak { player });
    }
}

/// [`proc_params`] with the fighter's grab partner (`catch_gobj` or
/// `capture_gobj`), whose same-frame hit `ftCommonDamageUpdateMain` reads
/// and writes. A partner already processed this frame reads as unhit, as in
/// the source, where `ftMainProcParams` clears `damage_knockback`.
pub fn proc_params_with(f: &mut Fighter, partner: Option<&mut Fighter>) -> bool {
    let mut damage = 0;
    let mut is_shieldbreak = false;
    let status_before = f.status.status;
    let mut is_knockback_paused = false;
    let mut proc_hit = false;

    // `efManagerShieldProcUpdate` (process priority 3) runs before this
    // `ftMainProcParams` (priority 0) every frame and clears last frame's
    // damage colour; only a set-off below raises it again.
    f.guard.is_damage_shield = false;

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
        if f.items.held.is_some()
            && !f.items.held.is_some_and(|i| {
                i.weight == crate::item::ItemWeight::Heavy && crate::grab::is_donkey(f.kind)
            })
            && f.hits.damage_knockback != 0.0
            && (f.hitlag == 0
                || !f.is_knockback_paused
                || f.hits.damage_knockback >= f.damage_knockback_stack + 30.0)
            && f.hits.damage_queue > crate::rng::rand_int_range(60)
        {
            crate::item_throw::drop_item(f);
        }
        if f.kind == FighterKind::Boss {
            // Master Hand takes no reaction: only the colour and his hit
            // points (`ftBossCommonUpdateDamageStats`).
            set_damage_colanim(f);
            crate::boss::update_damage_stats(f);
        } else {
            match f.hits.damage_kind {
                DamageKind::None => {}
                // `ftParamStopVoiceRunProcDamage` before both
                // (`goto_damage_status` stops the voice itself).
                DamageKind::Status => goto_damage_status(f),
                DamageKind::ColAnim => set_damage_colanim(f),
                DamageKind::Catch => {
                    crate::fighter_sound::stop_voice(f);
                    update_catch_resist(f)
                }
                DamageKind::Default => update_main(f, partner),
            }
        }
        damage = f.hits.damage_lag;
        is_knockback_paused = true;
    } else if f.hits.shield_damage != 0 {
        if is_shieldbreak {
            record_shield_break(f);
            crate::reaction::set_shield_break_fly(f);
        } else {
            status::set_guard_set_off(f, f.hits.shield_damage as f32, f.hits.shield_lr);
        }
        damage = f.hits.shield_damage;
    } else if f.hits.attack_shield_push != 0 {
        if f.hits.attack_rebound != 0.0 && f.grab.catch.is_none() && f.grab.capture.is_none() {
            crate::fighter_sound::stop_voice(f);
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
    } else if f.hits.reflect_damage != 0 {
        // `ftCommonShieldBreakFlyReflectorSetStatus`.
        crate::reaction::set_shield_break_fly(f);
    } else if f.hits.reflect_lr != 0.0 {
        if base_kind(f.kind) == FighterKind::Fox {
            // `ftFoxSpecialLwHitSetStatus`: `lr = reflect_lr`.
            f.facing = if f.hits.reflect_lr > 0.0 {
                crate::fighter::Facing::Right
            } else {
                crate::fighter::Facing::Left
            };
            status::set_fox_special_lw_hit(f);
        } else if base_kind(f.kind) == FighterKind::Ness {
            // `nFTSpecialCollKindNessReflector`: the bat's reflect.
            crate::sound::play_fgm(crate::sound::id::nSYAudioFGMBatHit);
        }
    } else if f.hits.absorb_lr != 0.0 {
        crate::ness::proc_absorb(f, f.hits.absorb_lr);
    }
    if damage != 0 {
        f.hitlag = hitlag_frames(damage, status_before, f.hits.hitlag_mul);
        if f.hitlag != 0 && is_knockback_paused {
            f.is_knockback_paused = true;
        }
        f.clear_taps();
    }
    if proc_hit {
        crate::item_use::proc_hit(f);
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
    if only_flashes(f) {
        set_damage_colanim(f);
    } else {
        let (kb, angle, lr) = (
            f.hits.damage_knockback,
            f.hits.damage_angle,
            f.hits.damage_lr,
        );
        crate::fighter_sound::stop_voice(f);
        crate::grab::set_donkey_throwf_damage(f, kb, angle, lr);
    }
}

/// `ftCommonDamageGotoDamageStatus`. Every caller in the source runs
/// `ftParamStopVoiceRunProcDamage` on the fighter first; that stop is here.
pub fn goto_damage_status(f: &mut Fighter) {
    crate::fighter_sound::stop_voice(f);

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
    if f.kind.character() == FighterKind::Donkey {
        f.donkey_special_n.charge_level = 0;
    }
    if f.kind.character() == FighterKind::Samus {
        crate::samus::on_damage(f);
    }
    if f.kind.character() == FighterKind::Link {
        crate::link::on_damage(f);
    }
    crate::yoshi::on_damage(f);
    crate::item_throw::on_damage(f);
    crate::pikachu::on_damage(f);
    if crate::kirby::is_kirby(f.kind) {
        crate::kirby_copy::on_damage(f);
    }
    if element == Element::Sleep {
        status::set_fura_sleep(f);
        return;
    }
    let damage = f.hits.damage_queue;
    attack::init_damage_vars_full(f, None, damage, kb, angle, lr, index, element, true);
}

/// `ftCommonDamageCheckCatchResist`.
fn catch_resist(f: &Fighter) -> bool {
    if crate::grab::is_cargo(f.status.status) {
        only_flashes_cargo(f)
    } else {
        only_flashes(f)
    }
}

/// `ftCommonDamageUpdateMain`, for a fighter that holds or is held by
/// another, then the ordinary case. `partner` is the other side of the grab
/// when the caller has it; its `hits` are its unprocessed same-frame hit.
fn update_main(f: &mut Fighter, partner: Option<&mut Fighter>) {
    if f.status.status == Status::YoshiEgg {
        crate::capture_yoshi::on_hit(f, f.hits.damage_queue);
        return;
    }
    let partner = partner.filter(|p| Some(p.port) == f.grab.catch.or(f.grab.capture));
    if f.grab.catch.is_some() {
        if let Some(held) = partner.filter(|p| p.hits.damage_knockback != 0.0) {
            // Both sides of the grab were hit this frame.
            if catch_resist(f) && attack::capture_keep_hold(held.hits.damage_queue) {
                held.hits.damage_lag = f.hits.damage_lag;
                held.hits.hitlag_mul = f.hits.hitlag_mul;
                update_catch_resist(f);
                held.hits.damage_kind = DamageKind::ColAnim;
                return;
            }
            if !catch_resist(f) && attack::capture_keep_hold(held.hits.damage_queue) {
                crate::grab::thrown_update_damage_stats(held, f);
            }
            crate::grab::lose_grip_pair(f, held);
            goto_damage_status(f);
            held.hits.damage_kind = DamageKind::Status;
            return;
        }
        if catch_resist(f) {
            update_catch_resist(f);
            return;
        }
        crate::grab::release_on_hit(f);
        goto_damage_status(f);
        return;
    }
    if f.grab.capture.is_some() {
        let keep = attack::capture_keep_hold(f.hits.damage_queue);
        if let Some(catcher) = partner {
            if catcher.hits.damage_knockback != 0.0 {
                if keep && catch_resist(catcher) {
                    f.hits.damage_lag = catcher.hits.damage_lag;
                    f.hits.hitlag_mul = catcher.hits.hitlag_mul;
                    catcher.hits.damage_kind = DamageKind::Catch;
                    set_damage_colanim(f);
                    return;
                }
                if keep {
                    crate::grab::thrown_update_damage_stats(f, catcher);
                }
                crate::grab::lose_grip_pair(catcher, f);
                goto_damage_status(f);
                catcher.hits.damage_kind = DamageKind::Status;
                return;
            }
            if keep {
                // `grab_fp->hitlag_tics = ftParamGetHitLag(...)`.
                catcher.hitlag = hitlag_frames(
                    f.hits.damage_lag,
                    catcher.status.status,
                    catcher.hits.hitlag_mul,
                );
                set_damage_colanim(f);
                return;
            }
        } else if keep {
            // The hold survives; the catcher takes the hitlag, delivered
            // through the grab link.
            crate::grab::send_catcher_hitlag(f, f.hits.damage_lag);
            set_damage_colanim(f);
            return;
        }
        crate::grab::release_on_capture_hit(f);
        goto_damage_status(f);
        return;
    }
    if only_flashes(f) {
        set_damage_colanim(f);
        return;
    }
    goto_damage_status(f);
}

/// `ftCommonDamageSetDamageColAnim`: the hit flashes the fighter without
/// changing its status.
pub(crate) fn set_damage_colanim(f: &mut Fighter) {
    let (kb, element) = (f.hits.damage_knockback, f.hits.damage_element);
    crate::colanim::update_damage_colanim(f, kb, element);
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
/// without a weapon pool, and runs a free-for-all.
pub fn resolve_frame(fighters: &mut [&mut Fighter]) -> [bool; 4] {
    search_all(fighters, TeamRules::FREE_FOR_ALL);
    finish_frame(fighters)
}

/// The search pass: attack positions, then every pair.
pub fn search_all(fighters: &mut [&mut Fighter], rules: TeamRules) {
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
            search_fighter_hits(a, b, other > this, rules);
            flush_thrown_stale(fighters);
        }
    }
}

/// Processes each fighter's hit log, then runs `ftMainProcParams` for all.
pub fn finish_frame(fighters: &mut [&mut Fighter]) -> [bool; 4] {
    finish_frame_with(fighters, &mut crate::effect::NoEffects)
}

/// [`finish_frame`], handing each fighter's hit effects to `effects` after
/// its hit processing and before any `ftMainProcParams`, in the order
/// `ftMainProcSearchHitAll` makes them: the set-offs of its search, then
/// its logged hits' effects. The fighters' queued effects
/// ([`crate::fteffect`]) are made before the hits' and after each
/// `ftMainProcParams`.
pub fn finish_frame_with(
    fighters: &mut [&mut Fighter],
    effects: &mut dyn HitEffectSink,
) -> [bool; 4] {
    finish_frame_between(fighters, effects, &mut |_| {})
}

/// [`finish_frame_with`], running `between` after every fighter's hit
/// effects and before the first `ftMainProcParams`: the later priority-1
/// processes (the weapons' clash search,
/// [`crate::weapon::WeaponPool::flush_clash_effects`]).
pub fn finish_frame_between(
    fighters: &mut [&mut Fighter],
    effects: &mut dyn HitEffectSink,
    between: &mut dyn FnMut(&mut dyn HitEffectSink),
) -> [bool; 4] {
    // Also supports callers that use individual pair searches.
    flush_thrown_stale(fighters);
    // The catch search's statuses (priority 2) made their effects first.
    for f in fighters.iter_mut() {
        effects.fighter(f);
    }
    for f in fighters.iter_mut() {
        process_hit_collision(f);
        for e in f.hits.effects[..f.hits.effects_len].iter().flatten() {
            effects.make(e);
        }
        f.hits.effects_len = 0;
    }
    between(effects);
    let n = fighters.len();
    for this in 0..n {
        for other in 0..n {
            if this != other {
                let (a, b) = pair_mut(fighters, this, other);
                apply_attacker_element(a, b);
            }
        }
    }
    // `ftMainProcParams` runs fighter by fighter, and a grab link reads and
    // writes the partner's unprocessed hit.
    let mut landed = [false; 4];
    for i in 0..n {
        let link = fighters[i].grab.catch.or(fighters[i].grab.capture);
        let partner = link.and_then(|port| (0..n).find(|&j| j != i && fighters[j].port == port));
        let hit = match partner {
            Some(j) => {
                let (a, b) = pair_mut(fighters, i, j);
                proc_params_with(a, Some(b))
            }
            None => proc_params(fighters[i]),
        };
        // The damage statuses' effects, in this fighter's `ftMainProcParams`.
        for f in fighters.iter_mut() {
            effects.fighter(f);
        }
        if i < 4 {
            landed[i] = hit;
        }
    }
    landed
}

fn flush_thrown_stale(fighters: &mut [&mut Fighter]) {
    for i in 0..fighters.len() {
        for slot in 0..4 {
            if let Some((owner, id, count)) = fighters[i].hits.thrown_stale[slot].take() {
                let origin = fighters[i].port;
                if let Some(f) = fighters.iter_mut().find(|f| f.port == owner) {
                    f.stale.push_thrown(id, count, origin);
                }
            }
        }
    }
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
