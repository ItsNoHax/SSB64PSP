//! The item system: `itmanager.c`, `itmain.c`, `itprocess.c` and `itmap.c`,
//! with the two fighter items, Ness's PK Fire flame (`itnesspkfire.c`,
//! [`pk_fire`]) and Link's Bomb (`itlinkbomb.c`, [`link_bomb`]), and the
//! stage items Peach's Castle's Bumper (`itgbumper.c`, [`gbumper`]),
//! Mushroom Kingdom's POW Block (`itpowerblock.c`, [`power_block`]) and its
//! Piranha Plants (`itpakkun.c`, [`pakkun`]), and Saffron's five Pokémon
//! ([`monsters`]), plus the light containers Egg and Capsule ([`container`]),
//! the consumed utilities ([`utility`]) and the throwable ones: the
//! Motion-Sensor Bomb ([`msbomb`]), the Bob-omb ([`bombhei`]), the Bumper
//! ([`nbumper`]), both Shells ([`shell`]) and the Poké Ball ([`mball`])
//! with its thirteen Pokémon ([`mmonster`]).
//!
//! [`ItemPool`] is `gITManagerStructsAllocFree` and the item GObj link: 16
//! structs (`ITEM_ALLOC_MAX`), handed out last-freed first, and a creation
//! ordered list that every search walks. Each item runs the source's three
//! processes in their priorities:
//!
//! * `itProcessProcItemMain` ([`ItemPool::tick`], priority 3): hitlag, the
//!   status's `proc_update`, the pickup clock, movement, the map bounds,
//!   `proc_map`, and the attack positions and records.
//! * `itProcessProcSearchHitAll` ([`ItemPool::search_hurt`], priority 1):
//!   fighter attacks, other items' attacks and weapons against this item's
//!   damage collision.
//! * `itProcessProcHitCollisions` ([`ItemPool::resolve`], priority 0): the
//!   frame's damage, hit, shield/hop, set-off and reflection callbacks.
//!
//! The fighter side of `ftMainProcSearchHitAll` for items is
//! `ftMainSearchHitItem` ([`ItemPool::search_fighter`]).
//!
//! Fighters and the pool never hold references to each other. A fighter
//! asks for item work with [`ItemRequest`]s, which the pool applies with
//! [`ItemPool::take_requests`] right after that fighter's callbacks, as the
//! source's direct calls land inside them; the pool publishes what a
//! fighter's pickup search reads ([`ItemPool::publish`]) and the owners'
//! positions ([`ItemPool::observe_owner`]); and [`ItemPool::sync_owner`]
//! clears a fighter's hand when its item leaves it from the item's side.
//! Attack-record victims use [`ITEM_RECORD_BASE`] plus the struct index, as
//! the source keys records by `GObj` pointer, which a new item reuses too.
//!
//! Stage controllers make their items through [`crate::stage::StageItems`],
//! which the pool implements, and the items' calls back into the stage are
//! queued as [`StageItemEvent`]s ([`ItemPool::take_stage_events`]). The POW
//! Block and the Piranha Plants read their own `DObj` animation (a root
//! clock that ends, a translation it writes), which the runtime plays
//! through the [`ItemAnims`] port.
//!
//! Normal containers have spin, spawn/break/explosion effects and switch
//! gates. Colour animations, sounds and pickup arrows remain; `hidden`
//! keeps the despawn flash, which is display state.

use ssb_engine::math::{Vec2, Vec3};

use crate::colanim::{ColAnim, ColAnimId};
use crate::combat::{AttackState, Element, HitStatus};
use crate::fighter::Fighter;
use crate::ground::{BodyColl, Standing};
use crate::stale::MotionAttackId;
use crate::status::BlastZone;
use crate::weapon::MapSurface;

pub mod bombhei;
pub mod container;
pub mod equipment;
pub mod gbumper;
mod hit;
pub mod normal;
pub(crate) use hit::{queue_damage, touches_damage_coll, Attacker, Knock};
#[cfg(test)]
mod heavy_tests;
pub mod link_bomb;
mod map;
pub mod mball;
pub mod mmonster;
#[cfg(test)]
mod mmonster_tests;
pub mod monsters;
pub mod msbomb;
pub mod nbumper;
pub mod pakkun;
pub mod pk_fire;
pub mod power_block;
pub mod shell;
#[cfg(test)]
mod stage_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod throwable_tests;
pub mod utility;
#[cfg(test)]
mod utility_tests;

/// `ITEM_ALLOC_MAX`.
pub const ITEM_ALLOC_MAX: usize = 16;

/// Attack-record victim ids from this value name an item struct; fighters
/// use their port and weapons [`crate::combat::WEAPON_RECORD_BASE`].
pub const ITEM_RECORD_BASE: u8 = 0x40;

/// `GMATTACKREC_NUM_MAX`.
const ATTACK_RECORDS: usize = 4;
/// `ITEM_ATKCOLL_NUM_MAX`.
pub const ATTACK_COLLS: usize = 2;

/// `ITEM_REHIT_TIME_DEFAULT`.
pub const REHIT_TIME: i32 = 16;
/// `ITEM_PICKUP_WAIT_DEFAULT`.
pub const PICKUP_WAIT_DEFAULT: u16 = 1400;
/// `ITEM_DESPAWN_FLASH_BEGIN_DEFAULT`.
const DESPAWN_FLASH_BEGIN: u16 = 180;
/// `ITEM_ARROW_FLASH_INT_DEFAULT`.
const ARROW_FLASH_INT: u8 = 45;
/// `ifCommonItemArrowProcDisplay` draws while `arrow_timer >= 15`.
const ARROW_SHOW_FROM: u8 = 15;
/// `ITEM_REFLECT_MUL_DEFAULT` (US), `..._ADD_...` and `..._MAX_...`.
const REFLECT_MUL: f32 = 1.8;
const REFLECT_ADD: f32 = 0.99;
const REFLECT_MAX: i32 = 100;
/// `ITEM_HOP_ANGLE_DEFAULT`: `F_CST_DTOR32(135.0F)`.
const HOP_ANGLE: f32 = 2.356_194_5;
/// `F_CST_DTOR32(90.0F)`.
const DEG_90: f32 = core::f32::consts::FRAC_PI_2;
/// `ITEM_THROW_NUM_MAX`, `ITEM_THROW_DESPAWN_RANDOM`,
/// `ITEM_LANDING_DESPAWN_CHECK` and `ITEM_LANDING_NUM_MAX`.
const THROW_NUM_MAX: u8 = 4;
const THROW_DESPAWN_RANDOM: i32 = 4;
const LANDING_DESPAWN_CHECK: u8 = 1;
const LANDING_NUM_MAX: u8 = 2;
/// `GMCOMMON_PERCENT_DAMAGE_MAX`.
const PERCENT_DAMAGE_MAX: i32 = 999;

/// `GMHITCOLLISION_FLAG_*`.
pub const INTERACT_FIGHTER: u8 = 1 << 0;
pub const INTERACT_WEAPON: u8 = 1 << 1;
pub const INTERACT_ITEM: u8 = 1 << 2;
pub const INTERACT_ALL: u8 = INTERACT_FIGHTER | INTERACT_WEAPON | INTERACT_ITEM;

/// The ported `ITKind`s.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemKind {
    Container(container::Kind),
    Utility(utility::Kind),
    Equipment(equipment::Kind),
    MSBomb,
    BombHei,
    NBumper,
    Shell(shell::Kind),
    MBall,
    NessPKFire,
    LinkBomb,
    GBumper,
    PowerBlock,
    Pakkun,
    Monster(monsters::Kind),
    MMonster(mmonster::Kind),
}

impl ItemKind {
    /// The `ITKind` of a common kind (`<= nITKindCommonEnd`): the
    /// containers, utilities, held utilities and throwables, 0 to 19.
    pub fn common_index(self) -> Option<u8> {
        Some(match self {
            Self::Container(k) => k as u8,
            Self::Utility(k) => k as u8,
            Self::Equipment(k) => 7 + k as u8,
            Self::MSBomb => 14,
            Self::BombHei => 15,
            Self::NBumper => 16,
            Self::Shell(k) => k as u8,
            Self::MBall => 19,
            _ => return None,
        })
    }

    /// Whether the kind's maker gives it a pickup arrow
    /// (`ip->arrow_gobj = ifCommonItemArrowMakeInterface(ip)`): every
    /// common kind but the Star.
    pub fn has_arrow(self) -> bool {
        self.common_index().is_some() && self != Self::Utility(utility::Kind::Star)
    }

    /// `ITAttributes::spin_speed` as a fraction, for the kinds that spin.
    pub fn spin_speed(self) -> Option<f32> {
        match self {
            Self::Container(k) => Some(k.spin_speed()),
            Self::Utility(k) => Some(k.spin_speed()),
            Self::Equipment(k) => Some(k.spin_speed()),
            // `ITAttributes::spin_speed` (file 251).
            Self::MSBomb => Some(120.0 * 0.01),
            Self::NBumper => Some(70.0 * 0.01),
            Self::MBall => Some(20.0 * 0.01),
            Self::BombHei | Self::Shell(_) => Some(0.0),
            _ => None,
        }
    }
}

/// `ITType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemType {
    Damage,
    Swing,
    Shoot,
    Throw,
    Touch,
    Consume,
    Fighter,
}

/// `ITWeight`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemWeight {
    Heavy,
    Light,
}

/// `nMPKineticsGround` / `nMPKineticsAir`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ga {
    Ground,
    Air,
}

/// `ITAttributes`, as far as gameplay reads it. The words are decoded from
/// the relocData files (bitfields packed in 32-bit units, big-endian); the
/// layout is checked against `251_ITCommonData.c`'s Bob-omb (RE-352).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ItemAttributes {
    pub is_give_hitlag: bool,
    /// `is_display_colanim`: the item draws through
    /// `itDisplayColAnim{OPA,XLU}ProcDisplay`, which sets the
    /// environment colour its second combiner cycle blends towards.
    pub is_display_colanim: bool,
    pub weight: ItemWeight,
    pub attack_offsets: [Vec3; ATTACK_COLLS],
    pub damage_coll_offset: Vec3,
    /// Full extents; the damage collision is half of this.
    pub damage_coll_size: Vec3,
    pub map_coll: BodyColl,
    /// Diameter; the attack is half of this.
    pub size: f32,
    pub angle: i32,
    pub kb_scale: i32,
    pub damage: i32,
    pub element: Element,
    pub kb_weight: i32,
    pub shield_damage: i32,
    pub attack_count: usize,
    pub can_setoff: bool,
    pub priority: i32,
    pub can_rehit_item: bool,
    pub can_rehit_fighter: bool,
    pub can_hop: bool,
    pub can_reflect: bool,
    pub can_shield: bool,
    pub kb_base: i32,
    pub ty: ItemType,
    pub hitstatus: HitStatus,
    /// Percent.
    pub vel_scale: u16,
}

/// `GMAttackRecord` for an item attack; `victim` is a record id.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ItemRecord {
    pub victim: Option<u8>,
    pub is_interact_hurt: bool,
    pub is_interact_shield: bool,
    pub is_interact_reflect: bool,
    pub timer_rehit: i32,
    pub group_id: u8,
}

impl Default for ItemRecord {
    fn default() -> Self {
        ItemRecord {
            victim: None,
            is_interact_hurt: false,
            is_interact_shield: false,
            is_interact_reflect: false,
            timer_rehit: 0,
            group_id: 7,
        }
    }
}

impl ItemRecord {
    pub(crate) fn is_clear(&self) -> bool {
        !self.is_interact_hurt
            && !self.is_interact_shield
            && !self.is_interact_reflect
            && self.group_id == 7
    }

    fn mark(&mut self, kind: HitType) {
        match kind {
            HitType::Damage => self.is_interact_hurt = true,
            HitType::Shield => self.is_interact_shield = true,
            HitType::ShieldRehit => {
                self.is_interact_shield = true;
                self.timer_rehit = REHIT_TIME;
            }
            HitType::Reflect => {
                self.is_interact_reflect = true;
                self.timer_rehit = REHIT_TIME;
            }
            HitType::Attack(group) => self.group_id = group,
            HitType::DamageRehit => {
                self.is_interact_hurt = true;
                self.timer_rehit = REHIT_TIME;
            }
        }
    }
}

/// `nGMHitType*`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HitType {
    Damage,
    Shield,
    ShieldRehit,
    Reflect,
    Attack(u8),
    DamageRehit,
}

/// `ITAttackPos`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ItemAttackPos {
    pub pos_curr: Vec3,
    pub pos_prev: Vec3,
}

/// `ITAttackColl`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ItemAttackColl {
    pub state: AttackState,
    pub damage: i32,
    pub throw_mul: f32,
    pub stale: f32,
    pub element: Element,
    pub offsets: [Vec3; ATTACK_COLLS],
    /// Radius.
    pub size: f32,
    pub angle: i32,
    pub kb_scale: i32,
    pub kb_weight: i32,
    pub kb_base: i32,
    pub shield_damage: i32,
    pub priority: i32,
    pub interact_mask: u8,
    pub can_setoff: bool,
    pub can_rehit_item: bool,
    pub can_rehit_fighter: bool,
    pub can_rehit_shield: bool,
    pub can_hop: bool,
    pub can_reflect: bool,
    pub can_shield: bool,
    pub motion_attack_id: MotionAttackId,
    pub motion_count: u16,
    pub count: usize,
    pub pos: [ItemAttackPos; ATTACK_COLLS],
    pub records: [ItemRecord; ATTACK_RECORDS],
}

impl ItemAttackColl {
    /// The record for `victim`, or a clear one.
    pub fn record(&self, victim: u8) -> ItemRecord {
        self.records
            .iter()
            .find(|r| r.victim == Some(victim))
            .copied()
            .unwrap_or_default()
    }

    /// `itProcessSetHitInteractStats`. A new victim takes the first empty
    /// slot, or slot 0 when all four are taken, keeping that slot's flags.
    pub(crate) fn set_hit_interact(&mut self, victim: u8, kind: HitType) {
        if let Some(r) = self.records.iter_mut().find(|r| r.victim == Some(victim)) {
            r.mark(kind);
            return;
        }
        let slot = self
            .records
            .iter()
            .position(|r| r.victim.is_none())
            .unwrap_or(0);
        self.records[slot].victim = Some(victim);
        self.records[slot].mark(kind);
    }

    /// `itMainClearAttackRecord`.
    fn clear_records(&mut self) {
        self.records = [ItemRecord::default(); ATTACK_RECORDS];
    }

    /// The attack as a fighter-pipeline hitbox with `damage` already output.
    pub fn hitbox(&self, damage: i32) -> crate::attack::Hitbox {
        crate::attack::Hitbox {
            damage,
            offset: Vec3::ZERO,
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

/// `ITDamageColl`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ItemDamageColl {
    pub interact_mask: u8,
    pub hitstatus: HitStatus,
    pub offset: Vec3,
    /// Half extents.
    pub size: Vec3,
}

/// Each kind's status, standing for the proc table `itMainSetStatus`
/// copies in. `Init` is the item descriptor's own procs, before the first
/// status is set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemStatus {
    Container(container::Status),
    Utility(utility::Status),
    Equipment(equipment::Status),
    MSBomb(msbomb::Status),
    BombHei(bombhei::Status),
    NBumper(nbumper::Status),
    Shell(shell::Status),
    MBall(mball::Status),
    PKFire(pk_fire::Status),
    LinkBomb(link_bomb::Status),
    GBumper(gbumper::Status),
    PowerBlock(power_block::Status),
    Pakkun(pakkun::Status),
    Monster(monsters::Status),
    MMonster(mmonster::Status),
}

/// `ITStruct::item_vars`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ItemVars {
    /// The item root's rotation: descriptor 1's `DObj`, since
    /// `itManagerMakeItem` ejects descriptor 0's placeholder (RE-432).
    /// `rotate_z` is its Z.
    pub container_root_yaw: f32,
    pub container_root_pitch: f32,
    pub equipment_child_yaw: f32,
    pub taru_roll_step: f32,
    /// `linkbomb.scale_id`, `scale_int`, `drop_update_wait`.
    pub bomb_scale_id: i32,
    pub bomb_scale_int: i32,
    pub bomb_drop_update_wait: u16,
    /// `bumper.hit_anim_length`.
    pub bumper_hit_anim_length: u16,
    /// `pakkun.pos` and `pakkun.is_wait_fighter`.
    pub pakkun_pos: Vec3,
    pub pakkun_is_wait_fighter: bool,
    /// Which `pakkun_gobj` slot made the plant: its tree in the runtime.
    pub pakkun_index: u8,
    pub monster_offset: Vec3,
    pub monster_flags: u8,
    pub monster_spawn_wait: u16,
    pub monster_eggs: u8,
    /// The Motion-Sensor Bomb's armed shape shows (its child 0) and its
    /// ball hides (child 1).
    pub msbomb_attached: bool,
    /// `bombhei.smoke_delay`, and the root's display list: the tree's own
    /// (`llITCommonDataBombHeiWalkRightDisplayList`) or the left-walking
    /// one (`itBombHeiCommonSetWalkLR`).
    pub bombhei_smoke_delay: u16,
    pub bombhei_walk_right: bool,
    /// The shells' root `rotate.y`.
    pub shell_rotate_y: f32,
    /// `ITCommonItemVarsShell`.
    pub shell_damage_all_delay: u8,
    pub shell_dust_int: u8,
    pub shell_health: u8,
    pub shell_is_damage: bool,
    pub shell_is_setup: bool,
    pub shell_interact: u8,
    pub shell_vel_x: f32,
    /// `bumper.damage_all_delay`; the attached model and material.
    pub bumper_damage_all_delay: u16,
    pub bumper_attached: bool,
    /// `mball.is_rebound`, and `mball.owner_gobj` with that fighter's team
    /// and handicap.
    pub mball_is_rebound: bool,
    pub mball_owner: Option<(u8, u8, u8)>,
    /// The open halves show and the closed ball hides.
    pub mball_open: bool,
    /// `mball.effect_gobj`: the rays the opened ball made.
    pub mball_rays: Option<u32>,
    /// A Poké Ball Pokémon's.
    pub mmonster: mmonster::Vars,
}

/// `ITStruct`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Item {
    pub kind: ItemKind,
    pub ty: ItemType,
    /// `owner_gobj`, a fighter port.
    pub owner: Option<u8>,
    /// `team`: the owner's, or [`crate::team::TEAM_DEFAULT`] with none.
    pub team: u8,
    /// `player` (`ITEM_PORT_DEFAULT` is `None`).
    pub player: Option<u8>,
    pub handicap: u8,
    pub percent_damage: i32,
    pub hitlag_tics: u32,
    pub lr: f32,
    pub vel_ground: f32,
    pub vel_air: Vec3,
    /// The root DObj's `translate`.
    pub pos: Vec3,
    /// `coll_data.pos_prev`.
    pub pos_prev: Vec3,
    /// The root DObj's `scale`.
    pub scale: Vec3,
    pub coll: BodyColl,
    pub ga: Ga,
    /// `coll_data.floor_line_id` and the floor's flags and angle.
    pub floor: Option<Standing>,
    pub mask_prev: u16,
    pub mask_curr: u16,
    pub lwall_normal: Vec2,
    pub rwall_normal: Vec2,
    pub ceil_normal: Vec2,
    /// `coll_data.{lwall,rwall,ceil}_line_id`.
    pub lwall_line: u16,
    pub rwall_line: u16,
    pub ceil_line: u16,
    /// `is_attach_surface` with `attach_line_id`: the line whose motion
    /// carries the item.
    pub attach_line: Option<u16>,
    pub attack: ItemAttackColl,
    pub damage_coll: ItemDamageColl,
    pub hit_normal_damage: i32,
    pub hit_lr: f32,
    pub hit_refresh_damage: i32,
    pub hit_attack_damage: i32,
    pub hit_shield_damage: i32,
    pub shield_collide_angle: f32,
    pub shield_collide_dir: Vec3,
    /// `reflect_gobj`, a fighter port.
    pub reflect_by: Option<u8>,
    pub damage_highest: i32,
    pub damage_knockback: f32,
    pub damage_queue: i32,
    pub damage_angle: i32,
    pub damage_element: Element,
    pub damage_lr: f32,
    /// `damage_gobj`'s port.
    pub damage_by: Option<u8>,
    pub damage_port: Option<u8>,
    pub damage_team: u8,
    pub damage_handicap: u8,
    pub damage_lag: i32,
    pub lifetime: i32,
    pub vel_scale: f32,
    pub is_allow_pickup: bool,
    pub is_hold: bool,
    pub times_landed: u8,
    pub times_thrown: u8,
    pub weight: ItemWeight,
    pub is_damage_all: bool,
    pub is_thrown: bool,
    pub pickup_wait: u16,
    pub is_allow_knockback: bool,
    pub is_static_damage: bool,
    pub multi: u16,
    pub event_id: u8,
    /// `GOBJ_FLAG_HIDDEN`: the despawn flash and the Bomb's explosion.
    pub hidden: bool,
    /// `gcPlayAnimAll` calls on the item's DObjs: one when it is made, then
    /// one per `itProcessProcItemMain` outside hitlag. Presentation only.
    pub anim_ticks: u16,
    /// The runtime has started this item's tree ([`ItemAnims::make`]). A
    /// stage controller makes its items without the runtime at hand, so the
    /// tree starts on the item's first process: its `itManagerMakeItem`
    /// play and that process's own play run back to back, as they do in
    /// the source's frame, with nothing reading the tree between.
    pub anim_made: bool,
    /// The root DObj's `rotate.z` (the knocked-out Piranha Plant's flip).
    /// Presentation only.
    pub rotate_z: f32,
    /// Common/appear/thrown spin for normal containers.
    pub spin_step: f32,
    /// `dobj->mobj->palette_id` (the Bumper's lit frames). Presentation
    /// only.
    pub palette: u8,
    /// Direct MObj texture selection by a ground Pokémon.
    pub texture: u8,
    pub arrow_timer: u8,
    /// The script a status added to the root's DObj or first MObj: the
    /// Bob-omb's walk and the Shells' spin. Presentation only (RE-442).
    pub root_script: Option<RootScript>,
    /// `colanim`: the Bob-omb's, the Link Bomb's and the Hammer's
    /// warnings (`itMainCheckSetColAnimID`). Its colour draws only for
    /// an item whose attributes set `is_display_colanim`.
    pub colanim: ColAnim,
    pub status: ItemStatus,
    pub vars: ItemVars,
    /// Where `ITStruct::attr` points.
    pub attr: &'static ItemAttributes,
}

impl Item {
    /// `itMainCheckSetColAnimID`.
    pub fn check_set_colanim(&mut self, id: ColAnimId, length: i32) -> bool {
        self.colanim.check_set(id, length)
    }

    /// `itMainClearColAnim`.
    pub fn clear_colanim(&mut self) {
        self.colanim.reset();
    }

    /// `itMainDestroyItem`'s dust: every item but one leaving its owner's
    /// hand and the stage Pokémon (`nITKindGroundMonsterStart` to
    /// `nITKindGroundMonsterEnd`) makes `efManagerDustExpandLargeMakeEffect`
    /// at its position.
    fn push_destroy_dust(&self, emit: &mut crate::wpeffect::Emit) {
        let held = self.is_hold && self.owner.is_some();
        if !held && !matches!(self.kind, ItemKind::Monster(_)) {
            emit.push(crate::wpeffect::WeaponEffect::DustExpandLarge(self.pos));
        }
    }

    /// `gcAddDObjAnimJoint` / `gcAddMObjMatAnimJoint` on the root at frame
    /// 0, then the caller's `gcPlayAnimAll`.
    pub(crate) fn add_root_script(&mut self) {
        self.root_script = Some(RootScript {
            added: self.anim_ticks,
            cleared: None,
        });
    }

    /// `event32 = NULL` on the root script: it ends at its next command
    /// fetch. A second clear changes nothing.
    pub(crate) fn clear_root_script(&mut self) {
        if let Some(script) = self.root_script.as_mut() {
            script.cleared.get_or_insert(self.anim_ticks);
        }
    }

    /// `ifCommonItemArrowProcDisplay`'s gate: the arrow draws while the
    /// item can be picked up, for 30 of every 45 frames.
    pub fn is_arrow_shown(&self) -> bool {
        self.kind.has_arrow() && self.is_allow_pickup && self.arrow_timer >= ARROW_SHOW_FROM
    }

    /// `itVisualsUpdateColAnim`.
    fn update_colanim(&mut self) {
        if self.colanim.update() {
            self.clear_colanim();
        }
    }

    /// `itManagerMakeItem`'s field setup, without the GObj and the map
    /// projection.
    fn new(
        kind: ItemKind,
        attr: &'static ItemAttributes,
        status: ItemStatus,
        attack_state: AttackState,
        pos: Vec3,
        vel: Vec3,
        motion_count: u16,
    ) -> Self {
        let mut item = Item {
            kind,
            ty: attr.ty,
            owner: None,
            team: crate::team::TEAM_DEFAULT,
            player: None,
            handicap: crate::stale::HANDICAP_DEFAULT,
            percent_damage: 0,
            hitlag_tics: 0,
            lr: 1.0,
            vel_ground: 0.0,
            vel_air: vel,
            pos,
            pos_prev: pos,
            scale: Vec3::new(1.0, 1.0, 1.0),
            coll: attr.map_coll,
            ga: Ga::Air,
            floor: None,
            mask_prev: 0,
            mask_curr: 0,
            lwall_normal: Vec2::ZERO,
            rwall_normal: Vec2::ZERO,
            ceil_normal: Vec2::ZERO,
            lwall_line: 0,
            rwall_line: 0,
            ceil_line: 0,
            attach_line: None,
            attack: ItemAttackColl {
                state: attack_state,
                damage: attr.damage,
                throw_mul: 1.0,
                stale: 1.0,
                element: attr.element,
                offsets: attr.attack_offsets,
                size: attr.size * 0.5,
                angle: attr.angle,
                kb_scale: attr.kb_scale,
                kb_weight: attr.kb_weight,
                kb_base: attr.kb_base,
                shield_damage: attr.shield_damage,
                priority: attr.priority,
                interact_mask: INTERACT_ALL,
                can_setoff: attr.can_setoff,
                can_rehit_item: attr.can_rehit_item,
                can_rehit_fighter: attr.can_rehit_fighter,
                can_rehit_shield: false,
                can_hop: attr.can_hop,
                can_reflect: attr.can_reflect,
                can_shield: attr.can_shield,
                motion_attack_id: MotionAttackId::None,
                motion_count,
                count: attr.attack_count,
                pos: [ItemAttackPos::default(); ATTACK_COLLS],
                records: [ItemRecord::default(); ATTACK_RECORDS],
            },
            damage_coll: ItemDamageColl {
                interact_mask: INTERACT_ALL,
                hitstatus: attr.hitstatus,
                offset: attr.damage_coll_offset,
                size: attr.damage_coll_size * 0.5,
            },
            hit_normal_damage: 0,
            hit_lr: 0.0,
            hit_refresh_damage: 0,
            hit_attack_damage: 0,
            hit_shield_damage: 0,
            shield_collide_angle: 0.0,
            shield_collide_dir: Vec3::ZERO,
            reflect_by: None,
            damage_highest: 0,
            damage_knockback: 0.0,
            damage_queue: 0,
            damage_angle: 0,
            damage_element: Element::Normal,
            damage_lr: 0.0,
            damage_by: None,
            damage_port: None,
            damage_team: crate::team::TEAM_DEFAULT,
            damage_handicap: crate::stale::HANDICAP_DEFAULT,
            damage_lag: 0,
            lifetime: 0,
            vel_scale: f32::from(attr.vel_scale) * 0.01,
            is_allow_pickup: false,
            is_hold: false,
            times_landed: 0,
            times_thrown: 0,
            weight: attr.weight,
            is_damage_all: false,
            is_thrown: false,
            pickup_wait: PICKUP_WAIT_DEFAULT,
            is_allow_knockback: false,
            is_static_damage: false,
            multi: 0,
            event_id: 0,
            hidden: false,
            // `itManagerMakeItem` plays the animation it adds once.
            anim_ticks: 1,
            anim_made: false,
            rotate_z: 0.0,
            spin_step: 0.0,
            palette: 0,
            texture: 0,
            arrow_timer: 0,
            root_script: None,
            colanim: ColAnim::default(),
            status,
            vars: ItemVars::default(),
            attr,
        };
        item.set_spin_vel_lr();
        item
    }

    /// `itMainSetSpinVelLR`, less the spin itself.
    pub(crate) fn set_spin_vel_lr(&mut self) {
        self.lr = if self.vel_air.x >= 0.0 { 1.0 } else { -1.0 };
        if let Some(spin) = self.kind.spin_speed() {
            self.spin_step = spin * 0.314_159_27 * self.lr;
        }
    }

    /// `itMainApplyGravityClampTVel`.
    pub(crate) fn apply_gravity_clamp_tvel(&mut self, gravity: f32, tvel: f32) {
        self.vel_air.y -= gravity;
        let v = Vec2::new(self.vel_air.x, self.vel_air.y);
        let mag = v.length();
        if mag > tvel {
            self.vel_air.x = self.vel_air.x / mag * tvel;
            self.vel_air.y = self.vel_air.y / mag * tvel;
        }
    }

    /// `itMainGetDamageOutput`.
    pub fn damage_output(&self) -> i32 {
        let damage = if self.is_thrown {
            let mag = self.vel_air.length() * 0.1;
            ((self.attack.damage as f32 + mag) * self.attack.throw_mul) as i32
        } else {
            self.attack.damage
        };
        (damage as f32 * self.attack.stale + 0.999) as i32
    }

    /// `itProcessUpdateAttackPositions`, including the source's shared
    /// state: a new second attack sweeps from its old position.
    pub fn update_attack_positions(&mut self) {
        for i in 0..self.attack.count.min(ATTACK_COLLS) {
            let world = self.attack.offsets[i] + self.pos;
            let p = &mut self.attack.pos[i];
            match self.attack.state {
                AttackState::Off => {}
                AttackState::New => {
                    p.pos_curr = world;
                    self.attack.state = AttackState::Transfer;
                }
                AttackState::Transfer | AttackState::Interpolate => {
                    self.attack.state = AttackState::Interpolate;
                    p.pos_prev = p.pos_curr;
                    p.pos_curr = world;
                }
            }
        }
    }

    /// `itProcessUpdateAttackRecords`.
    fn update_attack_records(&mut self) {
        if self.attack.state == AttackState::Off {
            return;
        }
        for r in &mut self.attack.records {
            if r.victim.is_some() && r.timer_rehit > 0 {
                r.timer_rehit -= 1;
                if r.timer_rehit <= 0 {
                    *r = ItemRecord::default();
                }
            }
        }
    }

    /// `itMainRefreshAttackColl`.
    pub(crate) fn refresh_attack_coll(&mut self) {
        self.attack.clear_records();
        self.attack.state = AttackState::New;
        self.update_attack_positions();
    }

    /// `itMainClearOwnerStats`.
    pub(crate) fn clear_owner_stats(&mut self) {
        self.is_damage_all = true;
        self.owner = None;
        self.team = crate::team::TEAM_DEFAULT;
    }

    /// `itMainSetGroundAllowPickup`.
    pub(crate) fn set_ground_allow_pickup(&mut self) {
        self.attack.state = AttackState::Off;
        self.vel_air = Vec3::ZERO;
        self.is_allow_pickup = true;
        self.times_landed = 0;
        // `itMainResetPlayerVars`.
        self.owner = None;
        self.team = crate::team::TEAM_DEFAULT;
        self.player = None;
        self.handicap = crate::stale::HANDICAP_DEFAULT;
        self.attack.throw_mul = 1.0;
        map::set_ground(self);
    }

    /// `itMainCopyDamageStats`: the attacker becomes the owner.
    pub(crate) fn copy_damage_stats(&mut self) {
        self.owner = self.damage_by;
        self.team = self.damage_team;
        self.player = self.damage_port;
        self.handicap = self.damage_handicap;
    }

    /// `itMainSetStatus`'s common half: the procs come from `status`.
    pub(crate) fn set_status(&mut self, status: ItemStatus) {
        self.status = status;
        self.is_thrown = false;
    }

    /// `itMainVelSetRebound`.
    pub(crate) fn vel_set_rebound(&mut self) {
        self.vel_air.x *= -0.06;
        self.vel_air.y = self.vel_air.y * -0.3 + 25.0;
    }

    /// `itMainCommonProcHop`.
    pub(crate) fn common_proc_hop(&mut self) {
        self.vel_air = rotate_about(
            self.vel_air,
            self.shield_collide_dir,
            self.shield_collide_angle * 2.0,
        );
        self.set_spin_vel_lr();
    }

    /// `itMainCommonProcReflector`: `reflector_lr` is the new owner's
    /// facing.
    pub(crate) fn common_proc_reflector(&mut self, reflector_lr: f32) {
        if self.vel_air.x * reflector_lr < 0.0 {
            self.vel_air.x = -self.vel_air.x;
        }
    }

    /// The item's world damage-collision centre.
    pub fn damage_coll_pos(&self) -> Vec3 {
        self.pos + self.damage_coll.offset
    }

    /// Whether its record id names this struct.
    pub fn floor_line(&self) -> Option<u16> {
        self.floor.map(|s| s.line)
    }

    /// The tree the runtime animates for this item through [`ItemAnims`].
    pub fn anim_target(&self) -> ItemAnimTarget {
        match self.kind {
            ItemKind::PowerBlock => ItemAnimTarget::PowerBlock,
            ItemKind::Pakkun => ItemAnimTarget::Pakkun(self.vars.pakkun_index),
            ItemKind::Monster(k) => ItemAnimTarget::Monster(k),
            _ => ItemAnimTarget::Untracked,
        }
    }

    /// A play's writes to the root DObj's translation, which is the item's
    /// position.
    pub(crate) fn apply_root_write(&mut self, write: RootWrite) {
        let [x, y, z] = write.translate;
        if let Some(x) = x {
            self.pos.x = x;
        }
        if let Some(y) = y {
            self.pos.y = y;
        }
        if let Some(z) = z {
            self.pos.z = z;
        }
    }
}

/// When a root script ran, on the item's [`Item::anim_ticks`] clock. The
/// status that adds it plays the tree once more (`gcPlayAnimAll`) on the
/// tick it is added, after that tick's own play; one that clears it does so
/// after that tick's play.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RootScript {
    pub added: u16,
    pub cleared: Option<u16>,
}

impl RootScript {
    /// Plays the script has had by `anim_ticks`, and how many of them came
    /// before it was cleared.
    pub fn plays(&self, anim_ticks: u16) -> (u16, Option<u16>) {
        let plays = |at: u16| at.wrapping_sub(self.added).wrapping_add(1);
        (plays(anim_ticks), self.cleared.map(plays))
    }
}

/// The item trees whose animation gameplay reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemAnimTarget {
    /// An item whose animation is presentation only.
    Untracked,
    PowerBlock,
    /// A Piranha Plant by its `pakkun_gobj` slot.
    Pakkun(u8),
    Monster(monsters::Kind),
}

/// The scripts an item starts on itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemAnim {
    /// `llGRInishieMapPowerBlockAnimJoint` on the root.
    PowerBlockDamage,
    /// `llGRInishieMapPakkunAppearAnimJoint` on the root and
    /// `llGRInishieMapPakkunAppearMatAnimJoint` on its `MObj`.
    PakkunAppear,
    /// `llGRInishieMapPakkunDamagedMatAnimJoint` on the root's `MObj`.
    PakkunDamaged,
}

/// What one `gcPlayAnimAll` wrote to the root's translation: an axis is
/// `Some` when a live track set it.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct RootWrite {
    pub translate: [Option<f32>; 3],
}

/// The runtime half of the items' `DObj` animation. A new item's tree
/// starts its `ITAttributes` scripts (`gcAddAnimAll` + `gcPlayAnimAll` in
/// `itManagerMakeItem`); [`ItemPool`] then drives it. The defaults are a
/// tree with no scripts: every root clock is idle and nothing is written.
pub trait ItemAnims {
    /// The root DObj clock after its latest play.
    fn root_frame(&self, _target: ItemAnimTarget) -> f32 {
        0.0
    }
    /// `itManagerMakeItem`'s `gcAddAnimAll` + `gcPlayAnimAll`.
    fn make(&mut self, _target: ItemAnimTarget) {}
    /// `gcPlayAnimAll`: `itProcessProcItemMain` outside hitlag.
    fn play(&mut self, _target: ItemAnimTarget) -> RootWrite {
        RootWrite::default()
    }
    /// `gcAddDObjAnimJoint` / `gcAddMObjMatAnimJoint` at frame 0, then the
    /// caller's `gcPlayAnimAll`.
    fn add_play(&mut self, _target: ItemAnimTarget, _anim: ItemAnim) -> RootWrite {
        RootWrite::default()
    }
    /// `DObjGetStruct(item_gobj)->anim_wait == AOBJ_ANIM_NULL`.
    fn root_idle(&self, _target: ItemAnimTarget) -> bool {
        true
    }
    /// `DObjGetStruct(item_gobj)->anim_wait = AOBJ_ANIM_NULL`.
    fn stop_root(&mut self, _target: ItemAnimTarget) {}
    /// `dobj->mobj->anim_wait = AOBJ_ANIM_NULL`.
    fn stop_material(&mut self, _target: ItemAnimTarget) {}
}

/// Item trees with no runtime.
pub struct NoItemAnims;
impl ItemAnims for NoItemAnims {}

/// A stage call an item makes, delivered by
/// [`crate::stage::Stage::apply_item_events`] after the item's update or
/// collision pass, before the next process phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StageItemEvent {
    /// `grInishiePowerBlockSetDamage`: the quake, sparing `hitter`
    /// (`ip->damage_gobj`).
    PowerBlockDamage { handicap: u8, hitter: Option<u8> },
    /// `grInishiePowerBlockSetWait`: the block is gone.
    PowerBlockGone,
    /// `grYamabukiGateSetClosedWait`.
    MonsterClose,
    /// `grYamabukiGateClearMonsterGObj` after a knockout.
    MonsterClear,
}

/// Stage calls one frame can queue: one POW Block makes at most one of
/// each.
const STAGE_EVENTS_MAX: usize = 4;

/// `lbCommonReflect2D`.
pub(crate) fn reflect_2d(v: &mut Vec3, n: Vec2) {
    map::reflect(v, n);
}

/// `syVectorRotateAbout3D`.
pub(crate) fn rotate_about(v: Vec3, dir: Vec3, angle: f32) -> Vec3 {
    let mag_yz = ssb_engine::math::sqrt(dir.y * dir.y + dir.z * dir.z);
    let (sin, cos) = ssb_engine::math::sin_cos(angle);
    let (ratio_z, ratio_y);
    let (rot_x, rot_y, rot_z);
    if mag_yz != 0.0 {
        ratio_z = dir.z / mag_yz;
        ratio_y = dir.y / mag_yz;
        rot_x = v.x;
        rot_y = v.y * ratio_z - v.z * ratio_y;
        rot_z = v.y * ratio_y + v.z * ratio_z;
    } else {
        ratio_z = 0.0;
        ratio_y = 0.0;
        rot_x = v.x;
        rot_y = v.y;
        rot_z = v.z;
    }
    let mut im_z = rot_x * mag_yz - rot_z * dir.x;
    let mut im_x = rot_x * dir.x + rot_z * mag_yz;
    let rot_x = im_z * cos - rot_y * sin;
    let rot_y = im_z * sin + rot_y * cos;
    im_z = rot_x * mag_yz + im_x * dir.x;
    im_x = -rot_x * dir.x + im_x * mag_yz;
    if mag_yz != 0.0 {
        Vec3::new(
            im_z,
            rot_y * ratio_z + im_x * ratio_y,
            -rot_y * ratio_y + im_x * ratio_z,
        )
    } else {
        Vec3::new(im_z, rot_y, im_x)
    }
}

/// What the pool reads of a fighter that holds or reflects items, sampled
/// by [`ItemPool::observe_owner`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OwnerView {
    pub pos: Vec3,
    pub facing: f32,
    pub coll: BodyColl,
    /// `fp->team`, which the Pokémon's opponent searches read.
    pub team: u8,
    /// World positions of `joint_itemlight_id` and `joint_itemheavy_id`.
    pub hold_light: Vec3,
    pub hold_heavy: Vec3,
}

/// A fighter's held item (`FTStruct::item_gobj`) and what its interrupts
/// read of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HeldItem {
    pub slot: u8,
    pub kind: ItemKind,
    pub ty: ItemType,
    pub weight: ItemWeight,
}

/// An item a fighter's pickup search may take (`ftCommonGetFindItem`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PickupCandidate {
    pub item: HeldItem,
    pub pos: Vec3,
    pub coll: BodyColl,
    pub floor_line: Option<u16>,
}

/// The items in link order, as [`ItemPool::publish`] last saw them.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ItemView {
    pub candidates: [Option<PickupCandidate>; ITEM_ALLOC_MAX],
}

/// Item work a fighter's callbacks ask for.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ItemRequest {
    /// `itLinkBombMakeItem`: a Bomb in the fighter's hand.
    MakeLinkBomb {
        pos: Vec3,
    },
    UseAmmo(u16),
    Scale(f32),
    HammerWarning,
    /// `itMainSetFighterHold` on an item found by the pickup search.
    Hold {
        slot: u8,
    },
    /// `itMainSetFighterThrow`.
    Throw {
        vel: Vec3,
        throw_mul: f32,
        is_smash: bool,
    },
    /// `itMainSetFighterDrop`.
    Drop {
        vel: Vec3,
        throw_mul: f32,
    },
    /// `itMainDestroyItem` from the fighter (`ftCommonDead`).
    Destroy,
}

/// Queued requests (`FTStruct` has no such field: these are direct calls in
/// the source, delivered at the end of the callback that made them).
pub const REQUESTS_MAX: usize = 2;

/// The fighter-item link, kept on [`Fighter::items`].
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct FighterItems {
    /// `FTStruct::item_gobj`.
    pub held: Option<HeldItem>,
    pub held_multi: u16,
    pub requests: [Option<ItemRequest>; REQUESTS_MAX],
    pub view: ItemView,
}

impl FighterItems {
    pub fn request(&mut self, request: ItemRequest) {
        if let Some(slot) = self.requests.iter_mut().find(|r| r.is_none()) {
            *slot = Some(request);
        }
    }
}

/// A PK Fire flame to make (`itNessPKFireMakeItem`), queued by the weapon
/// pool's spark hit callback.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PKFireSpawn {
    pub owner_port: u8,
    /// `wp->team`, which `itNessPKFireMakeItem` copies.
    pub team: u8,
    pub pos: Vec3,
    pub weapon_pos: Vec3,
    pub weapon_coll: BodyColl,
    pub stale: crate::stale::WeaponStale,
}

/// `gITManagerStructsAllocFree` and the item link.
#[derive(Debug, Clone, PartialEq)]
pub struct ItemPool {
    slots: [Option<Item>; ITEM_ALLOC_MAX],
    /// The free list: `free[free_len - 1]` is handed out next.
    free: [u8; ITEM_ALLOC_MAX],
    free_len: usize,
    /// Live structs in link (creation) order.
    order: [u8; ITEM_ALLOC_MAX],
    order_len: usize,
    owners: [Option<OwnerView>; 4],
    /// A held item left this port's hand from the item's side.
    released: [bool; 4],
    /// A registered hit to record in its thrower's stale queue:
    /// `(port, attack id, motion count)`.
    landed: [Option<(u8, MotionAttackId, u16)>; ITEM_ALLOC_MAX],
    /// The battle's team-attack rule ([`crate::team`]).
    pub team_rules: crate::team::TeamRules,
    pub normal_switches: normal::Switches,
    pub normal_drops: normal::DropWeights,
    /// `gITManagerAppearActor`, made after the ground (`grCommonSetupInitAll`).
    pub appear: Option<normal::AppearActor>,
    /// The battle camera's look-at X (`CObj::vec.at.x`), which the Star's
    /// maker reads.
    pub camera_at_x: f32,
    /// Bumped each time a struct is handed out, so a stage's handle to a
    /// struct that was freed and reused no longer resolves.
    serials: [u16; ITEM_ALLOC_MAX],
    events: [Option<StageItemEvent>; STAGE_EVENTS_MAX],
    monster_attack_prev: u8,
    /// `gITManagerMonsterData` (`itManagerInitMonsterVars`).
    monster_data: MonsterData,
    /// `gSCManagerBackupData.unlock_mask & LBBACKUP_UNLOCK_MASK_NEWCOMERS`:
    /// Mew can come out of a Poké Ball. No save data unlocks nothing.
    pub unlock_newcomers: bool,
    monster_shots: [Option<crate::monster_weapon::MonsterShot>; ITEM_ALLOC_MAX],
    /// Free weapon structs, from [`Self::observe_weapons`]: an item's weapon
    /// maker fails without one.
    weapon_free: u8,
    fx: crate::wpeffect::WeaponFx,
}

impl Default for ItemPool {
    /// `itManagerInitItems`: struct 0 is handed out first.
    fn default() -> Self {
        let mut free = [0u8; ITEM_ALLOC_MAX];
        for (i, f) in free.iter_mut().enumerate() {
            *f = (ITEM_ALLOC_MAX - 1 - i) as u8;
        }
        ItemPool {
            slots: [None; ITEM_ALLOC_MAX],
            free,
            free_len: ITEM_ALLOC_MAX,
            order: [0; ITEM_ALLOC_MAX],
            order_len: 0,
            owners: [None; 4],
            released: [false; 4],
            landed: [None; ITEM_ALLOC_MAX],
            team_rules: crate::team::TeamRules::FREE_FOR_ALL,
            normal_switches: normal::Switches::default(),
            normal_drops: normal::DropWeights::default(),
            appear: None,
            camera_at_x: 0.0,
            serials: [0; ITEM_ALLOC_MAX],
            events: [None; STAGE_EVENTS_MAX],
            monster_attack_prev: 4,
            monster_data: MonsterData::default(),
            unlock_newcomers: false,
            monster_shots: [None; ITEM_ALLOC_MAX],
            weapon_free: crate::weapon::MAX_WEAPONS as u8,
            fx: crate::wpeffect::WeaponFx::default(),
        }
    }
}

/// `nITKindMBallMonsterStart` (Onix), `nITKindMBallCommonEnd` (Clefairy)
/// and `nITKindMew`.
pub const MBALL_MONSTER_START: u8 = 32;
pub const MBALL_COMMON_END: u8 = 43;
pub const MEW: u8 = 44;

/// `ITMonsterData`: the last two Pokémon released, which the next draw
/// leaves out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MonsterData {
    pub monster_curr: u8,
    pub monster_prev: u8,
    pub monsters_num: u8,
}

impl Default for MonsterData {
    /// `itManagerInitMonsterVars`.
    fn default() -> Self {
        MonsterData {
            monster_curr: u8::MAX,
            monster_prev: u8::MAX,
            monsters_num: MEW - MBALL_MONSTER_START,
        }
    }
}

impl MonsterData {
    /// `itMainMakeMonster`'s choice of `ITKind`. Mew needs a newcomer
    /// unlocked and a 1 in 151 draw, and never follows itself.
    pub fn choose(&mut self, unlock_newcomers: bool) -> u8 {
        let index = if unlock_newcomers
            && crate::rng::rand_int_range(151) == 0
            && self.monster_curr != MEW
            && self.monster_prev != MEW
        {
            MEW
        } else {
            let mut ids = [0u8; (MBALL_COMMON_END - MBALL_MONSTER_START + 1) as usize];
            let mut j = 0;
            for i in MBALL_MONSTER_START..=MBALL_COMMON_END {
                if i != self.monster_curr && i != self.monster_prev {
                    ids[j] = i;
                    j += 1;
                }
            }
            // The count falls from 12 to 10 as the last two fill, so the
            // draw stays within the filled ids. With Mew among the last
            // two, one more id fills than the draw reaches.
            ids[crate::rng::rand_int_range(i32::from(self.monsters_num)) as usize]
        };
        if self.monsters_num != 10 {
            self.monsters_num -= 1;
        }
        self.monster_prev = self.monster_curr;
        self.monster_curr = index;
        index
    }
}

/// Owner effects of an item callback.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub(crate) struct Effects {
    /// The item left this fighter's hand (`fp->item_gobj = NULL`).
    pub release_owner: Option<u8>,
}

impl ItemPool {
    /// `itManagerMakeItemSetupCommon` for a supported container.
    pub fn spawn_container(&mut self, kind: container::Kind, pos: Vec3, vel: Vec3) -> Option<u8> {
        let slot = self.alloc(container::make(kind, pos, vel))?;
        let mut emit = crate::wpeffect::Emit::default();
        emit.push(crate::wpeffect::WeaponEffect::ItemSpawnSwirl(pos));
        self.fx.extend(self.order_len as u32 - 1, &emit);
        Some(slot)
    }

    /// `itManagerMakeItemSetupCommon(parent, index, pos, vel, flags)` for a
    /// common kind (`ITKind` 0..=19). `parent` is the item a container's
    /// contents project from (`ITEM_FLAG_COLLPROJECT | ...PARENT_ITEM`).
    /// Kinds without a ported maker make nothing (RE-433).
    pub fn make_setup_common<I, F>(
        &mut self,
        index: u8,
        parent: Option<(Vec3, BodyColl)>,
        pos: Vec3,
        vel: Vec3,
        surfaces: &F,
    ) -> Option<u8>
    where
        F: Fn() -> I,
        I: IntoIterator<Item = MapSurface>,
    {
        if self.free_len == 0 {
            return None;
        }
        let mut item = match index {
            0 => container::make(container::Kind::Crate, pos, vel),
            1 => container::make(container::Kind::Barrel, pos, vel),
            2 => container::make(container::Kind::Capsule, pos, vel),
            3 => container::make(container::Kind::Egg, pos, vel),
            7..=13 => equipment::make(equipment::Kind::from_index(index)?, pos, vel),
            14 => msbomb::make(pos, vel),
            15 => bombhei::make(pos, vel),
            16 => nbumper::make(pos, vel),
            17 => shell::make(shell::Kind::Green, pos, vel),
            18 => shell::make(shell::Kind::Red, pos, vel),
            19 => mball::make(pos, vel),
            _ => utility::make(
                utility::Kind::from_index(index)?,
                pos,
                vel,
                self.camera_at_x,
            ),
        };
        if let Some((parent_pos, parent_coll)) = parent {
            map::run_default_collision(&mut item, parent_pos, parent_coll, surfaces);
        }
        item.update_attack_positions();
        // `itMainSetAppearSpin(item_gobj, FALSE)`: slow, unsigned.
        item.spin_step = item.kind.spin_speed().unwrap_or(0.0) * core::f32::consts::PI / 18.0;
        let slot = self.alloc(item)?;
        let mut emit = crate::wpeffect::Emit::default();
        emit.push(crate::wpeffect::WeaponEffect::ItemSpawnSwirl(pos));
        self.fx.extend(self.order_len as u32 - 1, &emit);
        Some(slot)
    }

    /// `itMainMakeMonster` after its draw: `kind`'s maker from the open
    /// ball `ball`, then the ball's owner, team, player and handicap.
    pub fn make_mmonster<I, F>(
        &mut self,
        kind: mmonster::Kind,
        ball: &Item,
        surfaces: &F,
    ) -> Option<u8>
    where
        F: Fn() -> I,
        I: IntoIterator<Item = MapSurface>,
    {
        if self.free_len == 0 {
            return None;
        }
        let owners = self.owners;
        let vel = Vec3::new(0.0, 16.0, 0.0);
        let mut monster = mmonster::make(kind, ball, vel, &owners, surfaces);
        monster.owner = ball.owner;
        monster.team = ball.team;
        monster.player = ball.player;
        monster.handicap = ball.handicap;
        self.alloc(monster)
    }

    /// A Pokémon from a Poké Ball resting at `pos` that `owner` (with
    /// `team`) threw, for captures and tests.
    pub fn spawn_mmonster<I, F>(
        &mut self,
        kind: mmonster::Kind,
        pos: Vec3,
        owner: Option<u8>,
        team: u8,
        surfaces: &F,
    ) -> Option<u8>
    where
        F: Fn() -> I,
        I: IntoIterator<Item = MapSurface>,
    {
        let mut ball = mball::make(pos, Vec3::ZERO);
        ball.owner = owner;
        ball.player = owner;
        ball.team = team;
        self.make_mmonster(kind, &ball, surfaces)
    }

    /// `itManagerMakeAppearActor`, after the stage's ground is made.
    /// `points` are the stage's `nMPMapObjKindItem` positions.
    pub fn make_appear_actor(
        &mut self,
        stage_weights: Option<&[u8; 20]>,
        points: impl IntoIterator<Item = Vec3>,
    ) {
        self.appear = normal::AppearActor::new(self.normal_switches, stage_weights, points);
    }

    /// `itManagerAppearActorProcUpdate` (Item actor link, before the item
    /// link's main processes). `started` is past `nSCBattleGameStatusWait`.
    #[inline(never)]
    pub fn tick_appear<I, F>(&mut self, started: bool, surfaces: F)
    where
        F: Fn() -> I,
        I: IntoIterator<Item = MapSurface>,
    {
        let can_alloc = self.free_len != 0;
        let Some(spawn) = self
            .appear
            .as_mut()
            .and_then(|a| a.tick(started, can_alloc))
        else {
            return;
        };
        self.make_setup_common(spawn.kind, None, spawn.pos, Vec3::ZERO, &surfaces);
    }
    /// Reads the weapon pool before the item processes: its free structs
    /// (an item's weapon maker fails without one) and the last weapon pass's
    /// rock events, which Onix reads in its update: a rock that left the
    /// stage (`itIwarkWeaponRockProcDead`) or met a new floor
    /// (`rumble_frame`).
    pub fn observe_weapons(&mut self, weapons: &mut crate::weapon::WeaponPool) {
        self.weapon_free = weapons.free_count() as u8;
        for (handle, dead) in weapons.take_rock_events() {
            let Some(slot) = self.slot_of(handle) else {
                continue;
            };
            if let Some(item) = self.slots[usize::from(slot)].as_mut() {
                if matches!(item.kind, ItemKind::MMonster(_)) {
                    mmonster::rock_event(item, dead);
                }
            }
        }
    }

    /// Item-made weapons enter the weapon link before its main processes.
    pub fn flush_monster_shots(&mut self, weapons: &mut crate::weapon::WeaponPool) {
        for shot in core::mem::take(&mut self.monster_shots)
            .into_iter()
            .flatten()
        {
            weapons.spawn_monster_shot(shot);
        }
    }
    pub fn flush_effects(&mut self, sink: &mut dyn crate::effect::HitEffectSink) {
        for fx in self.fx.drain_sorted() {
            sink.weapon(&fx);
        }
    }
    /// Live items in link order.
    pub fn items(&self) -> impl Iterator<Item = &Item> + '_ {
        self.order[..self.order_len]
            .iter()
            .filter_map(|&s| self.slots[usize::from(s)].as_ref())
    }

    pub fn get(&self, slot: u8) -> Option<&Item> {
        self.slots.get(usize::from(slot)).and_then(|s| s.as_ref())
    }

    pub fn get_mut(&mut self, slot: u8) -> Option<&mut Item> {
        self.slots
            .get_mut(usize::from(slot))
            .and_then(|s| s.as_mut())
    }

    pub fn active_count(&self) -> usize {
        self.order_len
    }

    /// `itManagerGetNextStructAlloc` plus the GObj link.
    fn alloc(&mut self, item: Item) -> Option<u8> {
        if self.free_len == 0 {
            return None;
        }
        self.free_len -= 1;
        let slot = self.free[self.free_len];
        self.slots[usize::from(slot)] = Some(item);
        self.landed[usize::from(slot)] = None;
        self.serials[usize::from(slot)] = self.serials[usize::from(slot)].wrapping_add(1);
        self.order[self.order_len] = slot;
        self.order_len += 1;
        Some(slot)
    }

    /// `itMainDestroyItem`: a held item leaves its fighter's hand.
    fn destroy(&mut self, slot: u8) {
        let Some(item) = self.slots[usize::from(slot)].take() else {
            return;
        };
        if item.is_hold {
            if let Some(port) = item.owner {
                self.mark_released(port);
            }
        }
        if let Some(i) = self.order[..self.order_len].iter().position(|&s| s == slot) {
            self.order.copy_within(i + 1..self.order_len, i);
            self.order_len -= 1;
        }
        self.free[self.free_len] = slot;
        self.free_len += 1;
    }

    fn mark_released(&mut self, port: u8) {
        if let Some(r) = self.released.get_mut(usize::from(port)) {
            *r = true;
        }
    }

    fn apply_effects(&mut self, effects: Effects) {
        if let Some(port) = effects.release_owner {
            self.mark_released(port);
        }
    }

    /// Records the fighter's hold joints and position for this frame's item
    /// callbacks. Call after the fighter's tick.
    pub fn observe_owner(&mut self, f: &Fighter) {
        let Some(slot) = self.owners.get_mut(usize::from(f.port)) else {
            return;
        };
        *slot = Some(owner_view(f));
    }

    /// Writes this frame's item-side changes back to the fighter: a held item
    /// that exploded or was destroyed leaves its hand.
    pub fn sync_owner(&mut self, f: &mut Fighter) {
        let port = usize::from(f.port);
        if port < self.released.len() && self.released[port] {
            self.released[port] = false;
            if let Some(held) = f.items.held {
                let still = self
                    .get(held.slot)
                    .is_some_and(|i| i.is_hold && i.owner == Some(f.port));
                if !still {
                    f.items.held = None;
                }
            }
        }
    }

    /// Every item as the CPU reads it, in link order
    /// ([`crate::computer::behave::World::items`]).
    pub fn cpu_sights(&self) -> impl Iterator<Item = crate::computer::behave::ItemSight> + '_ {
        self.order[..self.order_len]
            .iter()
            .filter_map(|&s| self.slots[usize::from(s)].as_ref())
            .map(|item| {
                let a = &item.attack;
                let mut attack_pos = [Vec2::ZERO; ATTACK_COLLS];
                for (p, at) in attack_pos.iter_mut().zip(&a.pos) {
                    *p = Vec2::new(at.pos_curr.x, at.pos_curr.y);
                }
                crate::computer::behave::ItemSight {
                    pos: item.pos,
                    owner: item.owner,
                    team: item.team,
                    kind: item.kind,
                    weight: item.weight,
                    is_allow_pickup: item.is_allow_pickup,
                    is_damage_all: item.is_damage_all,
                    floor_line: if item.ga == Ga::Ground {
                        item.floor_line()
                    } else {
                        None
                    },
                    coll: item.coll,
                    vel_x: item.vel_air.x,
                    lr: item.lr,
                    attack_live: !matches!(a.state, AttackState::Off | AttackState::New)
                        && a.interact_mask & INTERACT_FIGHTER != 0,
                    attack_size: a.size,
                    attack_count: a.count.min(ATTACK_COLLS),
                    attack_pos,
                }
            })
    }

    /// Fills the fighter's [`ItemView`] for its pickup search. Call right
    /// before the fighter's tick.
    pub fn publish(&self, f: &mut Fighter) {
        let mut view = ItemView::default();
        for (n, item) in self.order[..self.order_len]
            .iter()
            .filter_map(|&s| self.slots[usize::from(s)].as_ref().map(|i| (s, i)))
            .enumerate()
        {
            let (slot, item) = item;
            if item.is_allow_pickup {
                view.candidates[n] = Some(PickupCandidate {
                    item: held_item(slot, item),
                    pos: item.pos,
                    coll: item.coll,
                    floor_line: item.floor_line(),
                });
            }
        }
        f.items.held_multi = self
            .held_slot(f)
            .and_then(|s| self.get(s))
            .map_or(0, |i| i.multi);
        f.items.view = view;
    }

    /// Applies the fighter's [`ItemRequest`]s in order. Call right after the
    /// fighter's callbacks (its tick, or `ftMainProcParams` for a drop).
    // Keep this process boundary under LTO: inlining the map sweeps into
    // the PSP app loop overflows a MIPS PC16 branch (RE-352).
    #[inline(never)]
    pub fn take_requests<I, F>(&mut self, f: &mut Fighter, surfaces: F)
    where
        F: Fn() -> I,
        I: IntoIterator<Item = MapSurface>,
    {
        let requests = core::mem::take(&mut f.items.requests);
        for request in requests.into_iter().flatten() {
            match request {
                ItemRequest::UseAmmo(cost) => {
                    if let Some(item) = self.held_slot(f).and_then(|s| self.get_mut(s)) {
                        item.multi = item.multi.wrapping_sub(cost);
                    }
                }
                ItemRequest::Scale(scale) => {
                    if let Some(item) = self.held_slot(f).and_then(|s| self.get_mut(s)) {
                        item.scale = Vec3::new(scale, scale, scale);
                    }
                }
                ItemRequest::HammerWarning => {
                    // `itHammerCommonSetColAnim`.
                    if let Some(item) = self.held_slot(f).and_then(|s| self.get_mut(s)) {
                        item.check_set_colanim(ColAnimId::ITEM_HAMMER_END, 0);
                    }
                }
                ItemRequest::MakeLinkBomb { pos } => {
                    let count = f.motion.take_count();
                    let item = link_bomb::make(pos, count);
                    if let Some(slot) = self.alloc(item) {
                        self.set_fighter_hold(slot, f);
                    }
                }
                ItemRequest::Hold { slot } => {
                    if self.get(slot).is_some_and(|i| i.is_allow_pickup) {
                        self.set_fighter_hold(slot, f);
                    }
                }
                ItemRequest::Throw {
                    vel,
                    throw_mul,
                    is_smash,
                } => {
                    if let Some(slot) = self.held_slot(f) {
                        self.set_fighter_throw(slot, f, vel, throw_mul, is_smash, &surfaces);
                    }
                }
                ItemRequest::Drop { vel, throw_mul } => {
                    if let Some(slot) = self.held_slot(f) {
                        self.set_fighter_drop(slot, f, vel, throw_mul, &surfaces);
                    }
                }
                ItemRequest::Destroy => {
                    if let Some(slot) = self.held_slot(f) {
                        self.destroy(slot);
                    }
                    f.items.held = None;
                }
            }
        }
        let port = usize::from(f.port);
        if port < self.released.len() {
            self.released[port] = false;
        }
    }

    /// The struct the fighter holds. A fighter that asked to throw has
    /// already let go of its [`HeldItem`], so the item's side decides.
    fn held_slot(&self, f: &Fighter) -> Option<u8> {
        self.order[..self.order_len].iter().copied().find(|&s| {
            self.slots[usize::from(s)]
                .as_ref()
                .is_some_and(|i| i.is_hold && i.owner == Some(f.port))
        })
    }

    /// `itMainSetFighterHold`.
    fn set_fighter_hold(&mut self, slot: u8, f: &mut Fighter) {
        let view = owner_view(f);
        let Some(item) = self.get_mut(slot) else {
            return;
        };
        item.owner = Some(f.port);
        item.team = f.team;
        item.is_allow_pickup = false;
        item.is_hold = true;
        item.player = Some(f.port);
        item.handicap = f.handicap;
        item.vel_air = Vec3::ZERO;
        map::set_air(item);
        item.pos = hold_pos(item, &view);
        // `dITMainProcHoldList`.
        match item.kind {
            ItemKind::Equipment(_) => equipment::hold(item),
            ItemKind::LinkBomb => link_bomb::hold_set_status(item),
            ItemKind::Container(_) => container::hold(item),
            ItemKind::MSBomb => msbomb::hold(item),
            ItemKind::BombHei => bombhei::hold(item),
            ItemKind::NBumper => nbumper::hold(item),
            ItemKind::Shell(_) => shell::hold(item),
            ItemKind::MBall => mball::hold(item, f.team, f.handicap),
            _ => {}
        }
        item.pickup_wait = PICKUP_WAIT_DEFAULT;
        f.items.held = Some(held_item(slot, item));
        f.items.held_multi = item.multi;
    }

    /// `itMainSetFighterRelease`.
    fn set_fighter_release<I, F>(
        item: &mut Item,
        view: &OwnerView,
        vel: Vec3,
        throw_mul: f32,
        surfaces: &F,
    ) where
        F: Fn() -> I,
        I: IntoIterator<Item = MapSurface>,
    {
        let hold = hold_pos(item, view);
        item.pos = Vec3::new(hold.x, hold.y, 0.0);
        map::run_default_collision(item, view.pos, view.coll, surfaces);
        item.is_hold = false;
        item.vel_air = vel * item.vel_scale;
        item.times_thrown = (item.times_thrown + 1) & 7;
        item.is_thrown = true;
        item.attack.throw_mul = throw_mul;
        item.refresh_attack_coll();
    }

    /// `itMainSetFighterThrow`.
    fn set_fighter_throw<I, F>(
        &mut self,
        slot: u8,
        f: &mut Fighter,
        vel: Vec3,
        throw_mul: f32,
        is_smash: bool,
        surfaces: &F,
    ) where
        F: Fn() -> I,
        I: IntoIterator<Item = MapSurface>,
    {
        let view = owner_view(f);
        let Some(item) = self.get_mut(slot) else {
            return;
        };
        // `dITMainProcThrownList`.
        match item.kind {
            ItemKind::Equipment(_) => equipment::release(item, false, f.facing.sign()),
            ItemKind::LinkBomb => link_bomb::thrown_set_status(item),
            ItemKind::Container(_) => container::thrown(item),
            ItemKind::MSBomb => msbomb::thrown(item),
            ItemKind::BombHei => bombhei::thrown(item),
            ItemKind::NBumper => nbumper::thrown(item),
            ItemKind::Shell(_) => shell::thrown(item),
            ItemKind::MBall => mball::thrown(item),
            _ => {}
        }
        Self::set_fighter_release(item, &view, vel, throw_mul, surfaces);
        // `itMainSetThrownSpin`.
        if let Some(spin) = item.kind.spin_speed() {
            item.spin_step =
                spin * if is_smash {
                    -0.366_519_15
                } else {
                    -core::f32::consts::PI / 18.0
                } * if vel.x < 0.0 { -1.0 } else { 1.0 };
        }
        f.items.held = None;
    }

    /// `itMainSetFighterDrop`.
    fn set_fighter_drop<I, F>(
        &mut self,
        slot: u8,
        f: &mut Fighter,
        vel: Vec3,
        throw_mul: f32,
        surfaces: &F,
    ) where
        F: Fn() -> I,
        I: IntoIterator<Item = MapSurface>,
    {
        let view = owner_view(f);
        let Some(item) = self.get_mut(slot) else {
            return;
        };
        if item.kind == ItemKind::LinkBomb {
            link_bomb::dropped_set_status(item);
        } else if let ItemKind::Container(k) = item.kind {
            container::dropped(item);
            item.spin_step = k.spin_speed() * 0.314_159_27 * if vel.x >= 0.0 { 1.0 } else { -1.0 };
        } else {
            // `dITMainProcDroppedList`.
            match item.kind {
                ItemKind::Utility(utility::Kind::Tomato | utility::Kind::Heart) => {
                    utility::dropped(item)
                }
                ItemKind::Equipment(_) => equipment::release(item, true, f.facing.sign()),
                ItemKind::MSBomb => msbomb::dropped(item),
                ItemKind::BombHei => bombhei::dropped(item),
                ItemKind::NBumper => nbumper::dropped(item),
                ItemKind::Shell(_) => shell::dropped(item),
                ItemKind::MBall => mball::dropped(item),
                _ => {}
            }
        }
        Self::set_fighter_release(item, &view, vel, throw_mul, surfaces);
        f.items.held = None;
    }

    /// `itNessPKFireMakeItem` for each flame the weapon pool queued.
    #[inline(never)]
    pub fn take_weapon_spawns<I, F>(&mut self, weapons: &mut crate::weapon::WeaponPool, surfaces: F)
    where
        F: Fn() -> I,
        I: IntoIterator<Item = MapSurface>,
    {
        for spawn in weapons.take_item_spawns() {
            let item = pk_fire::make(spawn, &surfaces);
            self.alloc(item);
        }
    }

    /// `itProcessProcItemMain` for every item, in link order. Call once per
    /// frame after the fighters and before the weapons. `fighters` holds
    /// every fighter's `TopN` translation in link order
    /// ([`pakkun::fighter_top`]).
    #[inline(never)]
    pub fn tick<I, F>(
        &mut self,
        surfaces: F,
        bounds: Option<BlastZone>,
        fighters: &[Vec3],
        anims: &mut dyn ItemAnims,
    ) where
        F: Fn() -> I,
        I: IntoIterator<Item = MapSurface>,
    {
        self.tick_with_effects(
            surfaces,
            bounds,
            fighters,
            anims,
            &mut crate::effect::NoEffects,
        );
    }

    /// Item main with direct effect-manager calls, preserving maker RNG
    /// before container contents are selected.
    #[inline(never)]
    pub fn tick_with_effects<I, F>(
        &mut self,
        surfaces: F,
        bounds: Option<BlastZone>,
        fighters: &[Vec3],
        anims: &mut dyn ItemAnims,
        effects_sink: &mut dyn crate::effect::HitEffectSink,
    ) where
        F: Fn() -> I,
        I: IntoIterator<Item = MapSurface>,
    {
        let owners = self.owners;
        let mut events = self.events;
        let mut shots = self.monster_shots;
        let mut weapon_free = self.weapon_free;
        // gcRunGObjProcess reads priority_next after the callback. Makers
        // append to the live link, so their main processes run in this pass.
        let mut cursor = 0;
        let mut link = 0;
        while cursor < self.order_len {
            let slot = self.order[cursor];
            let Some(mut item) = self.slots[usize::from(slot)] else {
                unreachable!("live item link names an empty struct");
            };
            let handle = self.handle_of(slot);
            let mut effects = Effects::default();
            let mut push = |e| push_event(&mut events, e);
            let mut emit = crate::wpeffect::Emit::default();
            let mut spawn = ShotBuf {
                shots: &mut shots,
                free: &mut weapon_free,
            };
            let mut common = CommonPort {
                pool: self,
                surfaces: &surfaces,
                effects: effects_sink,
            };
            let mut ctx = ProcCtx {
                common: &mut common,
                owners: &owners,
                fighters,
                anims: &mut *anims,
                events: &mut push,
                shots: &mut spawn,
                fx: &mut emit,
                bounds,
                handle,
            };
            let alive = process_main(&mut item, &mut ctx, &surfaces, bounds, &mut effects);
            if !alive {
                item.push_destroy_dust(&mut emit);
            }
            self.slots[usize::from(slot)] = Some(item);
            self.fx.extend(link as u32, &emit);
            self.apply_effects(effects);
            if !alive {
                self.destroy(slot);
            } else {
                cursor += 1;
            }
            link += 1;
        }
        self.events = events;
        self.monster_shots = shots;
        self.weapon_free = weapon_free;
    }

    /// `itProcessProcHitCollisions` for every item. `fighters` supplies the
    /// reflectors' current facing; call after `ftMainProcParams`.
    pub fn resolve<I, F>(&mut self, fighters: &[&Fighter], anims: &mut dyn ItemAnims, surfaces: F)
    where
        F: Fn() -> I,
        I: IntoIterator<Item = MapSurface>,
    {
        self.resolve_with_effects(fighters, anims, surfaces, &mut crate::effect::NoEffects);
    }
    #[inline(never)]
    pub fn resolve_with_effects<I, F>(
        &mut self,
        fighters: &[&Fighter],
        anims: &mut dyn ItemAnims,
        surfaces: F,
        effects_sink: &mut dyn crate::effect::HitEffectSink,
    ) where
        F: Fn() -> I,
        I: IntoIterator<Item = MapSurface>,
    {
        let mut events = self.events;
        // Collision callbacks can make container contents too. Their
        // priority-0 process runs now; priority 3 has already passed.
        let mut cursor = 0;
        let mut link = 0;
        while cursor < self.order_len {
            let slot = self.order[cursor];
            let Some(mut item) = self.slots[usize::from(slot)] else {
                unreachable!("live item link names an empty struct");
            };
            let mut push = |e| push_event(&mut events, e);
            let mut emit = crate::wpeffect::Emit::default();
            let mut common = CommonPort {
                pool: self,
                surfaces: &surfaces,
                effects: effects_sink,
            };
            let alive = hit_collisions(
                &mut item,
                fighters,
                anims,
                &mut push,
                &mut common,
                &mut emit,
            );
            if !alive {
                item.push_destroy_dust(&mut emit);
            }
            self.fx.extend(link as u32, &emit);
            self.slots[usize::from(slot)] = Some(item);
            if !alive {
                self.destroy(slot);
            } else {
                cursor += 1;
            }
            link += 1;
        }
        self.events = events;
    }

    /// The stage calls this frame's items made, in order.
    pub fn take_stage_events(&mut self) -> impl Iterator<Item = StageItemEvent> {
        core::mem::take(&mut self.events).into_iter().flatten()
    }

    /// The live struct a stage handle names.
    fn slot_of(&self, handle: u32) -> Option<u8> {
        let slot = (handle & 0xFF) as u8;
        let serial = (handle >> 16) as u16;
        (self.get(slot).is_some() && self.serials.get(usize::from(slot)) == Some(&serial))
            .then_some(slot)
    }

    fn handle_of(&self, slot: u8) -> u32 {
        u32::from(slot) | (u32::from(self.serials[usize::from(slot)]) << 16)
    }

    /// Records this frame's landed item hits in the thrower's stale queue
    /// (`ftParamUpdateStaleQueue` from `ftMainUpdateDamageStatItem`).
    pub fn record_landed(&mut self, owner: &mut Fighter) {
        for landed in &mut self.landed {
            if let Some((port, id, count)) = *landed {
                if port == owner.port {
                    owner.stale.push(id, count);
                    *landed = None;
                }
            }
        }
    }
}

fn push_event(events: &mut [Option<StageItemEvent>; STAGE_EVENTS_MAX], e: StageItemEvent) {
    if let Some(slot) = events.iter_mut().find(|s| s.is_none()) {
        *slot = Some(e);
    }
}

impl crate::stage::StageItems for ItemPool {
    /// `itManagerMakeItemSetupCommon` with `ITEM_FLAG_PARENT_GROUND`. A
    /// stage item has no owner, so its `motion_count` never reaches a stale
    /// queue. Ground Pokémon use the same allocation and handle rules.
    fn make_item(&mut self, kind: crate::stage::StageItem, pos: Vec3) -> Option<u32> {
        use crate::stage::StageItem;
        let item = match kind {
            StageItem::Bumper => gbumper::make(pos, 0),
            StageItem::PowerBlock => power_block::make(pos, 0),
            StageItem::Pakkun(i) => pakkun::make(i, pos, 0),
            StageItem::Monster(id) => {
                let kind = monsters::Kind::from_id(id)?;
                if self.free_len == 0 {
                    return None;
                }
                monsters::make(kind, pos, &mut self.monster_attack_prev)
            }
        };
        let slot = self.alloc(item)?;
        Some(self.handle_of(slot))
    }

    fn item_pos_width(&self, handle: u32) -> Option<(Vec3, f32)> {
        let item = self.get(self.slot_of(handle)?)?;
        Some((item.pos, item.coll.width))
    }

    fn set_item_x(&mut self, handle: u32, x: f32) {
        if let Some(item) = self.slot_of(handle).and_then(|s| self.get_mut(s)) {
            item.pos.x = x;
        }
    }

    fn pakkun_set_wait_fighter(&mut self, handle: u32) {
        if let Some(item) = self.slot_of(handle).and_then(|s| self.get_mut(s)) {
            pakkun::set_wait_fighter(item);
        }
    }
}

/// What an item's callbacks reach besides the item itself.
struct ProcCtx<'a> {
    common: &'a mut dyn normal::CommonItems,
    owners: &'a [Option<OwnerView>; 4],
    fighters: &'a [Vec3],
    anims: &'a mut dyn ItemAnims,
    events: &'a mut dyn FnMut(StageItemEvent),
    shots: &'a mut dyn mmonster::ShotSink,
    fx: &'a mut crate::wpeffect::Emit,
    bounds: Option<BlastZone>,
    /// The item's own handle ([`ItemPool::handle_of`]).
    handle: u32,
}

/// The frame's item-made weapons, made in the weapon link by
/// [`ItemPool::flush_monster_shots`]; `free` counts the weapon structs left.
struct ShotBuf<'a> {
    shots: &'a mut [Option<crate::monster_weapon::MonsterShot>; ITEM_ALLOC_MAX],
    free: &'a mut u8,
}

impl mmonster::ShotSink for ShotBuf<'_> {
    fn has_free(&self) -> bool {
        *self.free > 0 && self.shots.iter().any(Option::is_none)
    }
    fn push(&mut self, shot: crate::monster_weapon::MonsterShot) {
        if !self.has_free() {
            return;
        }
        if let Some(s) = self.shots.iter_mut().find(|s| s.is_none()) {
            *s = Some(shot);
            *self.free -= 1;
        }
    }
}

fn owner_view(f: &Fighter) -> OwnerView {
    let light = crate::item_throw::itemlight_joint(f.kind);
    let heavy = crate::grab::itemheavy_joint(f.kind).unwrap_or(0);
    OwnerView {
        pos: f.pos,
        facing: f.facing.sign(),
        coll: f.coll,
        team: f.team,
        hold_light: f.joint_world(light as u8, Vec3::ZERO),
        hold_heavy: f.joint_world(heavy as u8, Vec3::ZERO),
    }
}

fn held_item(slot: u8, item: &Item) -> HeldItem {
    HeldItem {
        slot,
        kind: item.kind,
        ty: item.ty,
        weight: item.weight,
    }
}

fn hold_pos(item: &Item, view: &OwnerView) -> Vec3 {
    match item.weight {
        ItemWeight::Heavy => view.hold_heavy,
        ItemWeight::Light => view.hold_light,
    }
}

/// `itProcessProcItemMain`. Returns whether the item lives on.
fn process_main<I, F>(
    item: &mut Item,
    ctx: &mut ProcCtx<'_>,
    surfaces: &F,
    bounds: Option<BlastZone>,
    effects: &mut Effects,
) -> bool
where
    F: Fn() -> I,
    I: IntoIterator<Item = MapSurface>,
{
    if item.hitlag_tics > 0 {
        item.hitlag_tics -= 1;
    }
    let target = item.anim_target();
    if !item.anim_made {
        item.anim_made = true;
        ctx.anims.make(target);
    }
    if item.hitlag_tics == 0 {
        item.anim_ticks = item.anim_ticks.wrapping_add(1);
        if target != ItemAnimTarget::Untracked {
            let write = ctx.anims.play(target);
            item.apply_root_write(write);
        }
    }
    if item.hitlag_tics == 0 && !proc_update(item, ctx, surfaces, effects) {
        return false;
    }
    let owners = ctx.owners;
    if item.is_allow_pickup {
        item.pickup_wait = item.pickup_wait.wrapping_sub(1) & 0xFFF;
        if item.pickup_wait <= DESPAWN_FLASH_BEGIN {
            if item.pickup_wait == 0 {
                return false;
            }
            if !item.pickup_wait.is_multiple_of(2) {
                item.hidden = !item.hidden;
            }
        }
        if item.arrow_timer == 0 {
            item.arrow_timer = ARROW_FLASH_INT;
        }
        item.arrow_timer -= 1;
    } else if !matches!(
        item.status,
        ItemStatus::LinkBomb(link_bomb::Status::Explode)
            | ItemStatus::MSBomb(msbomb::Status::Explode)
            | ItemStatus::BombHei(bombhei::Status::Explode)
            | ItemStatus::NBumper(nbumper::Status::GDisappear)
            | ItemStatus::Monster(monsters::Status::Explode)
            | ItemStatus::Container(container::Status::Explode)
            | ItemStatus::Container(container::Status::Roll)
    ) {
        // `item_gobj->flags = GOBJ_FLAG_NONE`, except that the explosions
        // and the Bumper's blink hide the root DObj rather than the GObj.
        item.hidden = false;
    }
    if item.is_hold {
        if let Some(view) = item
            .owner
            .and_then(|p| owners.get(usize::from(p)).copied().flatten())
        {
            item.pos = hold_pos(item, &view);
        }
        item.update_colanim();
        return true;
    }
    item.pos_prev = item.pos;
    if item.hitlag_tics == 0 {
        item.pos += item.vel_air;
    }
    // `itProcessProcItemMain` carries surface attachments before the bounds
    // gate, including hitlag frames. The map callback consumes this carried
    // target. An attached item follows its own line; a grounded one its
    // floor.
    let attached = item
        .attach_line
        .filter(|&line| crate::map::line_exists(surfaces, line));
    if let Some(line) = attached {
        item.pos += crate::map::line_speed(surfaces, line);
    } else if item.ga == Ga::Ground {
        if let Some(floor) = item.floor {
            item.pos += crate::map::line_speed(surfaces, floor.line);
        }
    }
    if let Some(b) = bounds {
        if item.pos.y < b.bottom
            || item.pos.x > b.right
            || item.pos.x < b.left
            || item.pos.y > b.top
        {
            // `ip->proc_dead`, which only a knocked-out Piranha Plant has.
            match item.status {
                ItemStatus::Pakkun(pakkun::Status::Damaged) => pakkun::proc_dead(item, ctx.anims),
                _ => return false,
            }
        }
    }
    if has_proc_map(item) {
        item.mask_prev = item.mask_curr;
        item.mask_curr = 0;
        if !proc_map(item, surfaces, ctx.common, ctx.fx) {
            return false;
        }
    }
    item.update_attack_positions();
    item.update_attack_records();
    item.update_colanim();
    true
}

/// `ip->proc_update`. Returns whether the item lives on.
fn proc_update<I, F>(
    item: &mut Item,
    ctx: &mut ProcCtx<'_>,
    surfaces: &F,
    effects: &mut Effects,
) -> bool
where
    F: Fn() -> I,
    I: IntoIterator<Item = MapSurface>,
{
    match item.status {
        ItemStatus::Container(s) => container::update(item, s, ctx.fx),
        ItemStatus::Utility(s) => utility::update(item, s),
        ItemStatus::Equipment(s) => equipment::update(item, s),
        ItemStatus::MSBomb(s) => msbomb::update(item, s, ctx.owners, ctx.fx),
        ItemStatus::BombHei(s) => bombhei::update(item, s, ctx.owners, surfaces, ctx.fx),
        ItemStatus::NBumper(s) => nbumper::update(item, s, surfaces),
        ItemStatus::Shell(s) => shell::update(item, s, ctx.owners, surfaces, ctx.fx),
        ItemStatus::MBall(s) => match mball::update(item, s, ctx.common) {
            mball::Update::Live => true,
            mball::Update::MakeMonster => {
                ctx.common.make_monster(item);
                false
            }
        },
        ItemStatus::PKFire(s) => pk_fire::proc_update(item, s),
        ItemStatus::LinkBomb(s) => link_bomb::proc_update(item, s, ctx.owners, surfaces, effects),
        ItemStatus::GBumper(_) => gbumper::proc_update(item),
        ItemStatus::PowerBlock(s) => power_block::proc_update(item, s, ctx.anims, ctx.events),
        ItemStatus::Pakkun(s) => pakkun::proc_update(item, s, ctx.fighters, ctx.anims),
        ItemStatus::Monster(s) => monsters::proc_update(
            item, s, ctx.anims, ctx.events, ctx.shots, ctx.fx, ctx.common,
        ),
        ItemStatus::MMonster(s) => mmonster::update(
            item,
            s,
            &mut mmonster::Ctx {
                owners: ctx.owners,
                bounds: ctx.bounds,
                shots: &mut *ctx.shots,
                fx: &mut *ctx.fx,
                common: &mut *ctx.common,
                handle: ctx.handle,
            },
        ),
    }
}

/// Whether the status has a `proc_map`: the stage items have none, so
/// `itProcessProcItemMain` leaves their map masks alone.
fn has_proc_map(item: &Item) -> bool {
    match item.status {
        ItemStatus::Equipment(s) => return s != equipment::Status::Hold,
        ItemStatus::MSBomb(s) => return msbomb::has_proc_map(s),
        ItemStatus::BombHei(s) => return bombhei::has_proc_map(s),
        ItemStatus::NBumper(s) => return nbumper::has_proc_map(s),
        ItemStatus::Shell(s) => return shell::has_proc_map(s),
        ItemStatus::MBall(s) => return mball::has_proc_map(s),
        ItemStatus::MMonster(s) => return mmonster::has_proc_map(item, s),
        _ => {}
    }
    matches!(
        item.status,
        ItemStatus::PKFire(_)
            | ItemStatus::LinkBomb(_)
            | ItemStatus::Utility(_)
            | ItemStatus::Container(
                container::Status::Init
                    | container::Status::Wait
                    | container::Status::Fall
                    | container::Status::Thrown
                    | container::Status::Dropped
                    | container::Status::Roll
            )
    )
}

/// `ip->proc_map`. Returns whether the item lives on.
fn proc_map<I, F>(
    item: &mut Item,
    surfaces: &F,
    common: &mut dyn normal::CommonItems,
    fx: &mut crate::wpeffect::Emit,
) -> bool
where
    F: Fn() -> I,
    I: IntoIterator<Item = MapSurface>,
{
    match item.status {
        ItemStatus::Container(s) => return container::proc_map(item, s, surfaces, common, fx),
        ItemStatus::Utility(s) => return utility::proc_map(item, s, surfaces),
        ItemStatus::Equipment(s) => return equipment::proc_map(item, s, surfaces),
        ItemStatus::MSBomb(s) => return msbomb::proc_map(item, s, surfaces),
        ItemStatus::BombHei(s) => return bombhei::proc_map(item, s, surfaces),
        ItemStatus::NBumper(s) => return nbumper::proc_map(item, s, surfaces),
        ItemStatus::Shell(s) => return shell::proc_map(item, s, surfaces),
        ItemStatus::MBall(s) => return mball::proc_map(item, s, surfaces, common),
        ItemStatus::MMonster(s) => return mmonster::proc_map(item, s, surfaces),
        ItemStatus::PKFire(s) => pk_fire::proc_map(item, s, surfaces),
        ItemStatus::LinkBomb(s) => link_bomb::proc_map(item, s, surfaces),
        ItemStatus::GBumper(_)
        | ItemStatus::PowerBlock(_)
        | ItemStatus::Pakkun(_)
        | ItemStatus::Monster(_) => {}
    }
    true
}

/// The kind's callback for one `itProcessProcHitCollisions` event, or
/// `None` without one. `Some(false)` destroys the item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HitProc {
    Damage,
    Hit,
    Hop,
    Shield,
    SetOff,
    Reflector,
}

/// One item callback's reach into the runtime and the stage.
struct HitCtx<'a> {
    common: &'a mut dyn normal::CommonItems,
    fx: &'a mut crate::wpeffect::Emit,
    anims: &'a mut dyn ItemAnims,
    events: &'a mut dyn FnMut(StageItemEvent),
}

/// `reflector` is the reflecting fighter's facing and X position.
fn run_hit_proc(
    item: &mut Item,
    proc: HitProc,
    reflector: (f32, f32),
    ctx: &mut HitCtx<'_>,
) -> Option<bool> {
    let reflector_lr = reflector.0;
    match item.status {
        ItemStatus::Container(s) => container::hit(item, s, proc, ctx.common, ctx.fx),
        ItemStatus::Utility(s) => utility::hit_proc(item, s, proc),
        ItemStatus::Equipment(s) => equipment::hit(item, s, proc, reflector_lr),
        ItemStatus::MSBomb(s) => msbomb::hit_proc(item, s, proc, reflector_lr, ctx.fx),
        ItemStatus::BombHei(s) => bombhei::hit_proc(item, s, proc, reflector_lr, ctx.fx),
        ItemStatus::NBumper(s) => nbumper::hit_proc(item, s, proc, reflector_lr),
        ItemStatus::Shell(s) => shell::hit_proc(item, s, proc, reflector),
        ItemStatus::MBall(s) => mball::hit_proc(item, s, proc),
        ItemStatus::PKFire(s) => pk_fire::hit_proc(item, s, proc),
        ItemStatus::LinkBomb(s) => link_bomb::hit_proc(item, s, proc, reflector_lr),
        ItemStatus::GBumper(_) => gbumper::hit_proc(item, proc),
        ItemStatus::PowerBlock(s) => power_block::hit_proc(item, s, proc, ctx.anims, ctx.events),
        ItemStatus::Pakkun(s) => pakkun::hit_proc(item, s, proc, ctx.anims),
        ItemStatus::Monster(s) => monsters::hit_proc(item, s, proc, ctx.anims, ctx.events),
        ItemStatus::MMonster(s) => mmonster::hit_proc(item, s, proc),
    }
}

/// `itProcessProcHitCollisions`. Returns whether the item lives on.
fn hit_collisions(
    item: &mut Item,
    fighters: &[&Fighter],
    anims: &mut dyn ItemAnims,
    events: &mut dyn FnMut(StageItemEvent),
    common: &mut dyn normal::CommonItems,
    fx: &mut crate::wpeffect::Emit,
) -> bool {
    let ctx = &mut HitCtx {
        anims,
        events,
        common,
        fx,
    };
    if item.damage_queue != 0 {
        item.percent_damage = (item.percent_damage + item.damage_queue).min(PERCENT_DAMAGE_MAX);
        item.damage_lag = item.damage_queue;
        if run_hit_proc(item, HitProc::Damage, (0.0, 0.0), ctx) == Some(false) {
            return false;
        }
    }
    if (item.hit_normal_damage != 0 || item.hit_refresh_damage != 0)
        && run_hit_proc(item, HitProc::Hit, (0.0, 0.0), ctx) == Some(false)
    {
        return false;
    }
    if item.hit_shield_damage != 0 {
        let mut hopped = false;
        if item.attack.can_hop && item.ga == Ga::Air && item.shield_collide_angle < HOP_ANGLE {
            item.shield_collide_angle = (item.shield_collide_angle - DEG_90).max(0.0);
            if run_hit_proc(item, HitProc::Hop, (0.0, 0.0), ctx) == Some(false) {
                return false;
            }
            hopped = true;
        }
        if !hopped && run_hit_proc(item, HitProc::Shield, (0.0, 0.0), ctx) == Some(false) {
            return false;
        }
    }
    if item.hit_attack_damage != 0
        && run_hit_proc(item, HitProc::SetOff, (0.0, 0.0), ctx) == Some(false)
    {
        return false;
    }
    if let Some(port) = item.reflect_by {
        let reflector = fighters.iter().find(|f| f.port == port);
        item.owner = Some(port);
        item.player = Some(port);
        if let Some(r) = reflector {
            item.team = r.team;
            item.handicap = r.handicap;
        }
        let lr = reflector.map_or(1.0, |r| r.facing.sign());
        let x = reflector.map_or(item.pos.x, |r| r.pos.x);
        if run_hit_proc(item, HitProc::Reflector, (lr, x), ctx) == Some(false) {
            return false;
        }
        if !item.is_static_damage {
            item.attack.damage =
                ((item.attack.damage as f32 * REFLECT_MUL + REFLECT_ADD) as i32).min(REFLECT_MAX);
        }
    }
    if item.damage_lag != 0 {
        item.hitlag_tics = u32::from(crate::combat::hitlag_frames(
            item.damage_lag,
            crate::status::Status::Wait.into(),
            1.0,
        ));
    }
    item.hit_normal_damage = 0;
    item.hit_refresh_damage = 0;
    item.hit_attack_damage = 0;
    item.hit_shield_damage = 0;
    item.reflect_by = None;
    item.damage_highest = 0;
    item.damage_queue = 0;
    item.damage_lag = 0;
    item.damage_knockback = 0.0;
    true
}

/// A direct manager call can allocate while the parent callback runs. The
/// child joins the pool immediately, so later searches see it this frame.
struct CommonPort<'a, F> {
    pool: &'a mut ItemPool,
    surfaces: &'a F,
    effects: &'a mut dyn crate::effect::HitEffectSink,
}
impl<I, F> normal::CommonItems for CommonPort<'_, F>
where
    F: Fn() -> I,
    I: IntoIterator<Item = MapSurface>,
{
    fn smash_container(&mut self, pos: Vec3) {
        self.effects.container_smash(pos);
    }
    fn mball_rays(&mut self, pos: Vec3) -> Option<u32> {
        self.effects.mball_rays(pos)
    }
    fn move_display(&mut self, seq: u32, pos: Vec3) {
        self.effects.move_display(seq, pos);
    }
    fn eggs_enabled(&self) -> bool {
        self.pool.normal_switches.enabled(3)
    }
    fn make_egg(&mut self, parent: &Item, pos: Vec3, vel: Vec3) -> bool {
        if self.pool.free_len == 0 {
            return false;
        }
        let mut egg = container::make(container::Kind::Egg, pos, vel);
        map::run_default_collision(&mut egg, parent.pos, parent.coll, self.surfaces);
        // Ground Chansey is GLucky, not MLucky: no direction RNG here.
        egg.update_attack_positions();
        self.pool.alloc(egg).is_some()
    }
    fn open_container(&mut self, parent: &mut Item) -> bool {
        // `itMainMakeContainerItem`.
        let Some(kind) = self.pool.normal_drops.choose().filter(|&k| k <= 19) else {
            return false;
        };
        let vel = Vec3::new(0.0, CONTAINER_VEL_Y[usize::from(kind)], 0.0);
        let parent_at = (parent.pos, parent.coll);
        if self
            .pool
            .make_setup_common(kind, Some(parent_at), parent.pos, vel, self.surfaces)
            .is_some()
        {
            // `itMainSetAppearSpin(parent_gobj, TRUE)`.
            parent.spin_step =
                parent.kind.spin_speed().unwrap_or(0.0) * 16.0 * core::f32::consts::PI / 180.0;
        }
        true
    }
    fn make_monster(&mut self, parent: &Item) {
        // `itMainMakeMonster`: the draw and its bookkeeping, then the maker,
        // then the ball's owner, team, player and handicap. The 1P game's
        // Mew catcher bonus is not ported.
        let index = self.pool.monster_data.choose(self.pool.unlock_newcomers);
        if let Some(kind) = mmonster::Kind::from_item_kind(index) {
            self.pool.make_mmonster(kind, parent, self.surfaces);
        }
    }
    fn make_common_egg(&mut self, parent: &Item, pos: Vec3, vel: Vec3) -> Option<i8> {
        let slot = self.pool.make_setup_common(
            3,
            Some((parent.pos, parent.coll)),
            pos,
            vel,
            self.surfaces,
        )?;
        Some(self.pool.get(slot).map_or(1, |egg| egg.lr as i8))
    }
    fn open_crate(&mut self, parent: &mut Item) -> bool {
        // `itBoxCommonCheckSpawnItems`, after the smash effect.
        let drops = self.pool.normal_drops;
        let Some(mut kind) = drops.choose().filter(|&k| k <= 19) else {
            return false;
        };
        let (count, first) = match crate::rng::rand_int_range(5) {
            0 | 1 => (1, 0),
            2 => (2, 1),
            _ => (3, 3),
        };
        let parent_at = (parent.pos, parent.coll);
        let identical = crate::rng::rand_int_range(32) == 0;
        for (j, &(x, y)) in BOX_SPAWN_VELOCITIES[first..first + count]
            .iter()
            .enumerate()
        {
            if !identical && j != 0 {
                kind = drops.choose_utility().unwrap_or(kind);
            }
            let vel = Vec3::new(x, y, 0.0);
            self.pool
                .make_setup_common(kind, Some(parent_at), parent.pos, vel, self.surfaces);
        }
        true
    }
}

/// `llITCommonDataContainerVelocitiesY` (file 251 + 0), by `ITKind`.
const CONTAINER_VEL_Y: [f32; 20] = [
    26.0, 26.0, 26.0, 26.0, 26.0, 26.0, 26.0, 40.0, 26.0, 26.0, 26.0, 26.0, 26.0, 26.0, 26.0, 26.0,
    26.0, 26.0, 26.0, 26.0,
];
/// `dITBoxItemSpawnVelocities`: one item, two, then three.
const BOX_SPAWN_VELOCITIES: [(f32, f32); 6] = [
    (0.0, 48.0),
    (-2.0, 48.0),
    (2.0, 48.0),
    (-5.0, 48.2),
    (0.0, 48.2),
    (5.2, 48.2),
];
