//! The item system: `itmanager.c`, `itmain.c`, `itprocess.c` and `itmap.c`,
//! with the two fighter items, Ness's PK Fire flame (`itnesspkfire.c`,
//! [`pk_fire`]) and Link's Bomb (`itlinkbomb.c`, [`link_bomb`]).
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
//! Teams, colour animations, effects, sounds, spin and the pickup arrow are
//! not ported; `hidden` keeps the despawn flash, which is display state.

use ssb_engine::math::{Vec2, Vec3};

use crate::combat::{AttackState, Element, HitStatus};
use crate::fighter::Fighter;
use crate::ground::{BodyColl, Standing};
use crate::stale::MotionAttackId;
use crate::status::BlastZone;
use crate::weapon::MapSurface;

mod hit;
pub(crate) use hit::{queue_damage, touches_damage_coll, Attacker};
pub mod link_bomb;
mod map;
pub mod pk_fire;
#[cfg(test)]
mod tests;

/// `ITEM_ALLOC_MAX`.
pub const ITEM_ALLOC_MAX: usize = 16;

/// Attack-record victim ids from this value name an item struct; fighters
/// use their port and weapons [`crate::combat::WEAPON_RECORD_BASE`].
pub const ITEM_RECORD_BASE: u8 = 0x40;

/// `GMATTACKREC_NUM_MAX`.
const ATTACK_RECORDS: usize = 4;
/// `ITEM_ATKCOLL_NUM_MAX`.
const ATTACK_COLLS: usize = 2;

/// `ITEM_REHIT_TIME_DEFAULT`.
pub const REHIT_TIME: i32 = 16;
/// `ITEM_PICKUP_WAIT_DEFAULT`.
pub const PICKUP_WAIT_DEFAULT: u16 = 1400;
/// `ITEM_DESPAWN_FLASH_BEGIN_DEFAULT`.
const DESPAWN_FLASH_BEGIN: u16 = 180;
/// `ITEM_ARROW_FLASH_INT_DEFAULT`.
const ARROW_FLASH_INT: u8 = 45;
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
    NessPKFire,
    LinkBomb,
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
    fn is_clear(&self) -> bool {
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
    PKFire(pk_fire::Status),
    LinkBomb(link_bomb::Status),
}

/// `ITStruct::item_vars`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ItemVars {
    /// `linkbomb.scale_id`, `scale_int`, `drop_update_wait`.
    pub bomb_scale_id: i32,
    pub bomb_scale_int: i32,
    pub bomb_drop_update_wait: u16,
}

/// `ITStruct`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Item {
    pub kind: ItemKind,
    pub ty: ItemType,
    /// `owner_gobj`, a fighter port.
    pub owner: Option<u8>,
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
    pub arrow_timer: u8,
    pub status: ItemStatus,
    pub vars: ItemVars,
    /// Where `ITStruct::attr` points.
    pub attr: &'static ItemAttributes,
}

impl Item {
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
            arrow_timer: 0,
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
    MakeLinkBomb { pos: Vec3 },
    /// `itMainSetFighterHold` on an item found by the pickup search.
    Hold { slot: u8 },
    /// `itMainSetFighterThrow`.
    Throw {
        vel: Vec3,
        throw_mul: f32,
        is_smash: bool,
    },
    /// `itMainSetFighterDrop`.
    Drop { vel: Vec3, throw_mul: f32 },
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
        }
    }
}

/// Owner effects of an item callback.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub(crate) struct Effects {
    /// The item left this fighter's hand (`fp->item_gobj = NULL`).
    pub release_owner: Option<u8>,
}

impl ItemPool {
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
        item.is_allow_pickup = false;
        item.is_hold = true;
        item.player = Some(f.port);
        item.handicap = f.handicap;
        item.vel_air = Vec3::ZERO;
        map::set_air(item);
        item.pos = hold_pos(item, &view);
        match item.kind {
            ItemKind::LinkBomb => link_bomb::hold_set_status(item),
            ItemKind::NessPKFire => {}
        }
        item.pickup_wait = PICKUP_WAIT_DEFAULT;
        f.items.held = Some(held_item(slot, item));
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
        _is_smash: bool,
        surfaces: &F,
    ) where
        F: Fn() -> I,
        I: IntoIterator<Item = MapSurface>,
    {
        let view = owner_view(f);
        let Some(item) = self.get_mut(slot) else {
            return;
        };
        match item.kind {
            ItemKind::LinkBomb => link_bomb::thrown_set_status(item),
            ItemKind::NessPKFire => {}
        }
        Self::set_fighter_release(item, &view, vel, throw_mul, surfaces);
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
        match item.kind {
            ItemKind::LinkBomb => link_bomb::dropped_set_status(item),
            ItemKind::NessPKFire => {}
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
    /// frame after the fighters' and weapons' own updates.
    #[inline(never)]
    pub fn tick<I, F>(&mut self, surfaces: F, bounds: Option<BlastZone>)
    where
        F: Fn() -> I,
        I: IntoIterator<Item = MapSurface>,
    {
        let order = self.order;
        let owners = self.owners;
        for &slot in &order[..self.order_len] {
            let Some(mut item) = self.slots[usize::from(slot)] else {
                continue;
            };
            let mut effects = Effects::default();
            let alive = process_main(&mut item, &owners, &surfaces, bounds, &mut effects);
            self.slots[usize::from(slot)] = Some(item);
            self.apply_effects(effects);
            if !alive {
                self.destroy(slot);
            }
        }
    }

    /// `itProcessProcHitCollisions` for every item. `fighters` supplies the
    /// reflectors' current facing; call after `ftMainProcParams`.
    pub fn resolve(&mut self, fighters: &[&Fighter]) {
        let order = self.order;
        for &slot in &order[..self.order_len] {
            let Some(mut item) = self.slots[usize::from(slot)] else {
                continue;
            };
            let alive = hit_collisions(&mut item, fighters);
            self.slots[usize::from(slot)] = Some(item);
            if !alive {
                self.destroy(slot);
            }
        }
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

fn owner_view(f: &Fighter) -> OwnerView {
    let light = crate::item_throw::itemlight_joint(f.kind);
    let heavy = crate::grab::itemheavy_joint(f.kind).unwrap_or(0);
    OwnerView {
        pos: f.pos,
        facing: f.facing.sign(),
        coll: f.coll,
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
    owners: &[Option<OwnerView>; 4],
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
    if item.hitlag_tics == 0 && !proc_update(item, owners, surfaces, effects) {
        return false;
    }
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
    } else if item.kind != ItemKind::LinkBomb
        || item.status != ItemStatus::LinkBomb(link_bomb::Status::Explode)
    {
        // `item_gobj->flags = GOBJ_FLAG_NONE`, except that the explosion
        // hides the Bomb's DObj rather than the GObj.
        item.hidden = false;
    }
    if item.is_hold {
        if let Some(view) = item
            .owner
            .and_then(|p| owners.get(usize::from(p)).copied().flatten())
        {
            item.pos = hold_pos(item, &view);
        }
        return true;
    }
    item.pos_prev = item.pos;
    if item.hitlag_tics == 0 {
        item.pos += item.vel_air;
    }
    // `itProcessProcItemMain` carries ground attachments before the bounds gate,
    // including hitlag frames. The map callback consumes this carried target.
    if item.ga == Ga::Ground {
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
            return false;
        }
    }
    item.mask_prev = item.mask_curr;
    item.mask_curr = 0;
    if !proc_map(item, surfaces) {
        return false;
    }
    item.update_attack_positions();
    item.update_attack_records();
    true
}

/// `ip->proc_update`. Returns whether the item lives on.
fn proc_update<I, F>(
    item: &mut Item,
    owners: &[Option<OwnerView>; 4],
    surfaces: &F,
    effects: &mut Effects,
) -> bool
where
    F: Fn() -> I,
    I: IntoIterator<Item = MapSurface>,
{
    match item.status {
        ItemStatus::PKFire(s) => pk_fire::proc_update(item, s),
        ItemStatus::LinkBomb(s) => link_bomb::proc_update(item, s, owners, surfaces, effects),
    }
}

/// `ip->proc_map`. Returns whether the item lives on.
fn proc_map<I, F>(item: &mut Item, surfaces: &F) -> bool
where
    F: Fn() -> I,
    I: IntoIterator<Item = MapSurface>,
{
    match item.status {
        ItemStatus::PKFire(s) => pk_fire::proc_map(item, s, surfaces),
        ItemStatus::LinkBomb(s) => link_bomb::proc_map(item, s, surfaces),
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

fn run_hit_proc(item: &mut Item, proc: HitProc, reflector_lr: f32) -> Option<bool> {
    match item.status {
        ItemStatus::PKFire(s) => pk_fire::hit_proc(item, s, proc),
        ItemStatus::LinkBomb(s) => link_bomb::hit_proc(item, s, proc, reflector_lr),
    }
}

/// `itProcessProcHitCollisions`. Returns whether the item lives on.
fn hit_collisions(item: &mut Item, fighters: &[&Fighter]) -> bool {
    if item.damage_queue != 0 {
        item.percent_damage = (item.percent_damage + item.damage_queue).min(PERCENT_DAMAGE_MAX);
        item.damage_lag = item.damage_queue;
        if run_hit_proc(item, HitProc::Damage, 0.0) == Some(false) {
            return false;
        }
    }
    if (item.hit_normal_damage != 0 || item.hit_refresh_damage != 0)
        && run_hit_proc(item, HitProc::Hit, 0.0) == Some(false)
    {
        return false;
    }
    if item.hit_shield_damage != 0 {
        let mut hopped = false;
        if item.attack.can_hop && item.ga == Ga::Air && item.shield_collide_angle < HOP_ANGLE {
            item.shield_collide_angle = (item.shield_collide_angle - DEG_90).max(0.0);
            if run_hit_proc(item, HitProc::Hop, 0.0) == Some(false) {
                return false;
            }
            hopped = true;
        }
        if !hopped && run_hit_proc(item, HitProc::Shield, 0.0) == Some(false) {
            return false;
        }
    }
    if item.hit_attack_damage != 0 && run_hit_proc(item, HitProc::SetOff, 0.0) == Some(false) {
        return false;
    }
    if let Some(port) = item.reflect_by {
        let reflector = fighters.iter().find(|f| f.port == port);
        item.owner = Some(port);
        item.player = Some(port);
        if let Some(r) = reflector {
            item.handicap = r.handicap;
        }
        let lr = reflector.map_or(1.0, |r| r.facing.sign());
        if run_hit_proc(item, HitProc::Reflector, lr) == Some(false) {
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
