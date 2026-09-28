//! The item hit searches of `itprocess.c` and `ftmain.c`.
//!
//! * [`ItemPool::search_fighter`]: `ftMainSearchHitItem`, item attacks
//!   against one fighter — a clank with its attacks, its reflector, its
//!   shield, then its hurtboxes.
//! * [`ItemPool::search_hurt`]: `itProcessProcSearchHitAll`, fighter
//!   attacks (`itProcessSearchHitFighter`), other items
//!   (`itProcessSearchHitItem`) and weapons (`itProcessSearchHitWeapon`)
//!   against each item's damage collision.
//!
//! Contacts only record, as for fighters: the item reacts in
//! [`ItemPool::resolve`], the fighter in `ftMainProcParams`.

use ssb_engine::math::Vec3;

use super::{
    Ga, HitType, Item, ItemPool, ItemType, INTERACT_FIGHTER, INTERACT_ITEM, ITEM_RECORD_BASE,
};
use crate::combat::{self, AttackState, HitLogEntry, HitSource, HitStatus};
use crate::fighter::Fighter;

/// An item attack's swept segment: a new or one-frame-old attack tests its
/// position only.
fn sweep(item: &Item, i: usize) -> (Vec3, Vec3, f32, AttackState) {
    let p = item.attack.pos[i];
    let prev = if item.attack.state == AttackState::Interpolate {
        p.pos_prev
    } else {
        p.pos_curr
    };
    (p.pos_curr, prev, item.attack.size, item.attack.state)
}

/// `gmCollisionCheckItemInFighterRange`.
fn in_fighter_range(item: &Item, i: usize, f: &Fighter) -> bool {
    let range = crate::motion::combat_attrs(f.kind).map_or([f32::MAX; 3], |a| a.hit_detect_range);
    let (curr, prev, size, state) = sweep(item, i);
    if state == AttackState::Transfer {
        combat::in_range(curr, f.pos, range, size)
    } else {
        combat::in_range(curr, f.pos, range, size) || combat::in_range(prev, f.pos, range, size)
    }
}

/// `hit_lr` of an attacking item: its travel, or the side it is on when
/// nearly still.
fn attacker_lr(attacker_vel_x: f32, attacker_x: f32, victim_x: f32) -> f32 {
    if attacker_vel_x.abs() < 5.0 {
        if victim_x < attacker_x {
            -1.0
        } else {
            1.0
        }
    } else if attacker_vel_x < 0.0 {
        -1.0
    } else {
        1.0
    }
}

/// `damage_lr` of an item hit by a moving attacker: the opposite sense.
fn victim_lr(attacker_vel_x: f32, attacker_x: f32, victim_x: f32) -> f32 {
    -attacker_lr(attacker_vel_x, attacker_x, victim_x)
}

/// The fields of an attacker an item's damage bookkeeping copies.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Attacker {
    pub owner: Option<u8>,
    pub player: Option<u8>,
    pub handicap: u8,
}

/// The shared half of `itProcessUpdateDamageStat*`: a normal damage
/// collision queues the damage and the strongest hit's angle, element and
/// side.
pub(crate) fn queue_damage(
    victim: &mut Item,
    damage: i32,
    angle: i32,
    element: crate::combat::Element,
    lr: f32,
    by: Attacker,
) {
    if victim.damage_coll.hitstatus != HitStatus::Normal {
        return;
    }
    victim.damage_queue += damage;
    if victim.damage_highest < damage {
        victim.damage_highest = damage;
        victim.damage_angle = angle;
        victim.damage_element = element;
        victim.damage_lr = lr;
        victim.damage_by = by.owner;
        victim.damage_port = by.player;
        victim.damage_handicap = by.handicap;
    }
    // `is_allow_knockback` is never set for the ported kinds.
}

/// `gmCollisionCheck*AttackItemDamageCollide`: a sphere swept against the
/// item's damage box, grown by the radius over the item's scale.
pub(crate) fn touches_damage_coll(
    victim: &Item,
    pos_curr: Vec3,
    pos_prev: Vec3,
    radius: f32,
    state: AttackState,
) -> bool {
    crate::hurtbox::test_rectangle_world(
        pos_curr,
        pos_prev,
        radius,
        state,
        victim.damage_coll_pos(),
        victim.damage_coll.size,
        victim.scale,
    )
}

impl ItemPool {
    /// `ftMainSearchHitItem`: every item's attack against `f`. Call from the
    /// search pass, after the fighter-versus-fighter search.
    pub fn search_fighter(&mut self, f: &mut Fighter) {
        // `ftMainProcSearchHitAll` skips a ghost.
        if f.dead.is_ghost {
            return;
        }
        let order = self.order;
        for &slot in &order[..self.order_len] {
            let Some(mut item) = self.slots[usize::from(slot)] else {
                continue;
            };
            let landed = search_item_on_fighter(&mut item, slot, f);
            if landed {
                if let Some(player) = item.player {
                    if player != f.port {
                        self.landed[usize::from(slot)] = Some((
                            player,
                            item.attack.motion_attack_id,
                            item.attack.motion_count,
                        ));
                    }
                }
            }
            self.slots[usize::from(slot)] = Some(item);
        }
    }

    /// `itProcessProcSearchHitAll` for every item not in a hand. `fighters`
    /// are in link order.
    pub fn search_hurt(
        &mut self,
        fighters: &mut [&mut Fighter],
        weapons: &mut crate::weapon::WeaponPool,
    ) {
        let order = self.order;
        let len = self.order_len;
        for (n, &slot) in order.iter().enumerate().take(len) {
            let Some(mut item) = self.slots[usize::from(slot)] else {
                continue;
            };
            if item.is_hold {
                continue;
            }
            let id = ITEM_RECORD_BASE + slot;
            for f in fighters.iter_mut() {
                fighter_attacks_item(f, &mut item, id);
            }
            self.slots[usize::from(slot)] = Some(item);
            self.items_attack_item(n);
            if let Some(item) = self.slots[usize::from(slot)].as_mut() {
                weapons.hit_item(item, id);
            }
        }
        weapons.finish_item_hits();
    }

    /// `itProcessSearchHitItem` for the item at link position `n`.
    fn items_attack_item(&mut self, n: usize) {
        let this_slot = self.order[n];
        let Some(mut this) = self.slots[usize::from(this_slot)] else {
            return;
        };
        if this.damage_coll.interact_mask & INTERACT_ITEM == 0 {
            return;
        }
        let this_id = ITEM_RECORD_BASE + this_slot;
        let order = self.order;
        for (m, &other_slot) in order[..self.order_len].iter().enumerate() {
            if m == n {
                continue;
            }
            // Only items after this one in the link trade clanks with it.
            let is_check_self = m > n;
            let Some(mut other) = self.slots[usize::from(other_slot)] else {
                continue;
            };
            let other_id = ITEM_RECORD_BASE + other_slot;
            if this.owner == other.owner && !this.is_damage_all {
                continue;
            }
            if other.attack.state == AttackState::Off
                || other.attack.interact_mask & INTERACT_ITEM == 0
                || !other.attack.record(this_id).is_clear()
            {
                continue;
            }
            let mut to_hurtbox = true;
            if is_check_self
                && this.attack.can_setoff
                && other.attack.can_setoff
                && this.owner != other.owner
                && this.attack.state != AttackState::Off
                && this.attack.interact_mask & INTERACT_ITEM != 0
                && this.attack.record(other_id).is_clear()
            {
                'clank: for i in 0..other.attack.count {
                    for j in 0..this.attack.count {
                        if crate::hurtbox::attacks_collide(sweep(&other, i), sweep(&this, j)) {
                            update_attack_stat_item(&mut other, other_id, &mut this, this_id);
                            if other.hit_attack_damage != 0 {
                                to_hurtbox = false;
                                break 'clank;
                            } else if this.hit_attack_damage != 0 {
                                break 'clank;
                            }
                        }
                    }
                }
            }
            if to_hurtbox {
                for i in 0..other.attack.count {
                    if this.damage_coll.hitstatus == HitStatus::None {
                        break;
                    }
                    if this.damage_coll.hitstatus == HitStatus::Intangible {
                        continue;
                    }
                    let (curr, prev, size, state) = sweep(&other, i);
                    if touches_damage_coll(&this, curr, prev, size, state) {
                        update_damage_stat_item(&mut other, &mut this, this_id);
                        break;
                    }
                }
            }
            self.slots[usize::from(other_slot)] = Some(other);
        }
        self.slots[usize::from(this_slot)] = Some(this);
    }
}

/// `itProcessUpdateAttackStatItem`: two item attacks meet; the lower (or
/// equal) priority side records the other and is set off.
fn update_attack_stat_item(this: &mut Item, this_id: u8, victim: &mut Item, victim_id: u8) {
    let victim_damage = victim.damage_output();
    let this_damage = this.damage_output();
    if victim.attack.priority <= this.attack.priority {
        victim.attack.set_hit_interact(this_id, HitType::Attack(0));
        victim.hit_attack_damage = victim.hit_attack_damage.max(victim_damage);
    }
    if this.attack.priority <= victim.attack.priority {
        this.attack.set_hit_interact(victim_id, HitType::Attack(0));
        this.hit_attack_damage = this.hit_attack_damage.max(this_damage);
    }
}

/// `itProcessUpdateDamageStatItem`: `attack`'s hitbox touched `defend`'s
/// damage collision.
fn update_damage_stat_item(attack: &mut Item, defend: &mut Item, defend_id: u8) {
    let damage = attack.damage_output();
    let is_rehit = defend.ty == ItemType::Damage && attack.attack.can_rehit_item;
    attack.attack.set_hit_interact(
        defend_id,
        if is_rehit {
            HitType::DamageRehit
        } else {
            HitType::Damage
        },
    );
    if is_rehit {
        attack.hit_refresh_damage = attack.hit_refresh_damage.max(damage);
    } else {
        attack.hit_normal_damage = attack.hit_normal_damage.max(damage);
    }
    attack.hit_lr = attacker_lr(attack.vel_air.x, attack.pos.x, defend.pos.x);
    let lr = victim_lr(attack.vel_air.x, attack.pos.x, defend.pos.x);
    queue_damage(
        defend,
        damage,
        attack.attack.angle,
        attack.attack.element,
        lr,
        Attacker {
            owner: attack.owner,
            player: attack.player,
            handicap: attack.handicap,
        },
    );
}

/// `itProcessSearchHitFighter` for one fighter: its live attacks that reach
/// the item's situation and have not recorded it test the damage box.
fn fighter_attacks_item(f: &mut Fighter, item: &mut Item, id: u8) {
    if item.damage_coll.interact_mask & INTERACT_FIGHTER == 0 {
        return;
    }
    if item.owner == Some(f.port) && !item.is_damage_all {
        return;
    }
    if f.grab.is_catchstatus {
        return;
    }
    if item.owner.is_some() && combat::throw_port(f) == item.owner {
        return;
    }
    let air = item.ga == Ga::Air;
    let mut detect = [false; 4];
    for (i, coll) in f.attack_colls.iter().enumerate() {
        detect[i] =
            coll.state != AttackState::Off && coll.reaches(air) && coll.record(id).is_clear();
    }
    for i in 0..4 {
        if !detect[i] {
            continue;
        }
        if item.damage_coll.hitstatus == HitStatus::None {
            break;
        }
        if item.damage_coll.hitstatus == HitStatus::Intangible {
            continue;
        }
        let c = f.attack_colls[i];
        if touches_damage_coll(item, c.pos_curr, c.pos_prev, c.size, c.state) {
            // `itProcessUpdateDamageStatFighter`.
            combat::set_hit_interact(f, c.group, id, combat::HitType::Damage, &mut detect);
            if f.hits.attack_damage < c.damage {
                f.hits.attack_damage = c.damage;
            }
            let lr = if item.pos.x < f.pos.x { 1.0 } else { -1.0 };
            queue_damage(
                item,
                c.damage,
                c.angle,
                c.element,
                lr,
                Attacker {
                    owner: Some(f.port),
                    player: Some(f.port),
                    handicap: f.handicap,
                },
            );
        }
    }
}

/// `ftMainUpdateAttackStatItem`: an item attack met one of the fighter's.
/// Each side that does not beat the other by more than 10 damage records
/// the clank; the fighter's side also recoils.
fn update_attack_stat_fighter(
    item: &mut Item,
    id: u8,
    f: &mut Fighter,
    j: usize,
    attack_detect: &mut [bool; 4],
) {
    let damage = item.damage_output();
    let coll = f.attack_colls[j];
    if coll.damage - 10 < damage {
        combat::set_hit_interact(f, coll.group, id, combat::HitType::Attack(0), attack_detect);
        combat::set_hit_rebound(f, &coll, item.pos.x);
    }
    if damage - 10 < coll.damage {
        item.attack
            .set_hit_interact(f.port, HitType::Attack(coll.group));
        item.hit_attack_damage = item.hit_attack_damage.max(damage);
    }
}

/// `ftMainSearchHitItem` for one item. Returns whether its hit registered
/// damage (the stale-queue update).
fn search_item_on_fighter(item: &mut Item, slot: u8, f: &mut Fighter) -> bool {
    let id = ITEM_RECORD_BASE + slot;
    if item.owner == Some(f.port)
        || item.attack.state == AttackState::Off
        || item.attack.interact_mask & INTERACT_FIGHTER == 0
        || !item.attack.record(f.port).is_clear()
    {
        return false;
    }
    let reflector = combat::reflector(f);
    let is_reflect = reflector.is_some();
    let throw_port = combat::throw_port(f);
    if item.attack.can_setoff
        && !f.grab.is_catchstatus
        && (throw_port.is_none() || throw_port != item.owner)
        && (!is_reflect || !item.attack.can_reflect)
    {
        let item_air = item.ga == Ga::Air;
        let mut attack_detect = [false; 4];
        for (i, coll) in f.attack_colls.iter().enumerate() {
            attack_detect[i] = coll.state != AttackState::Off
                && coll.reaches(item_air)
                && coll.record(id).group_id == combat::NO_GROUP;
        }
        if attack_detect.contains(&true) {
            for i in 0..item.attack.count {
                for j in 0..4 {
                    if !attack_detect[j] {
                        continue;
                    }
                    let c = f.attack_colls[j];
                    if crate::hurtbox::attacks_collide(
                        sweep(item, i),
                        (c.pos_curr, c.pos_prev, c.size, c.state),
                    ) {
                        update_attack_stat_fighter(item, id, f, j, &mut attack_detect);
                        if item.hit_attack_damage != 0 {
                            return false;
                        }
                    }
                }
            }
        }
    }
    let mut detect = [false; 2];
    for (i, d) in detect.iter_mut().enumerate().take(item.attack.count) {
        *d = in_fighter_range(item, i, f);
    }
    if !detect.contains(&true) {
        return false;
    }
    if let Some(r) = reflector.filter(|_| item.attack.can_reflect) {
        for (i, &detected) in detect.iter().enumerate().take(item.attack.count) {
            if !detected {
                continue;
            }
            let (curr, prev, size, _) = sweep(item, i);
            if combat::special_contact(f, &r, curr, prev, size) {
                update_reflector_stat(item, f, &r);
                return false;
            }
        }
    }
    if combat::is_shield(f) && item.attack.can_shield {
        for (i, &detected) in detect.iter().enumerate().take(item.attack.count) {
            if !detected {
                continue;
            }
            let (curr, prev, size, state) = sweep(item, i);
            if let Some((angle, dir)) = crate::hurtbox::test_sphere_angle(
                &combat::shield_transform(f),
                curr,
                prev,
                size,
                state,
                Vec3::ZERO,
                Vec3::new(30.0, 30.0, 30.0),
            ) {
                update_shield_stat(item, f, angle, dir);
                return false;
            }
        }
    }
    if combat::is_body_intangible(f) {
        return false;
    }
    for (i, &detected) in detect.iter().enumerate().take(item.attack.count) {
        if !detected {
            continue;
        }
        let (curr, prev, size, state) = sweep(item, i);
        if let Some(hit) = crate::hurtbox::search_attack(f, curr, prev, size, state) {
            return update_damage_stat(item, f, hit);
        }
    }
    false
}

/// `ftMainUpdateReflectorStatItem`.
fn update_reflector_stat(item: &mut Item, f: &mut Fighter, r: &combat::SpecialColl) {
    let damage = item.damage_output();
    item.attack.set_hit_interact(f.port, HitType::Reflect);
    match combat::reflect_weapon(f, r, damage, item.pos) {
        combat::ReflectOutcome::Broke => {
            if item.attack.can_rehit_fighter {
                item.hit_refresh_damage = item.hit_refresh_damage.max(damage);
            } else {
                item.hit_normal_damage = item.hit_normal_damage.max(damage);
            }
        }
        combat::ReflectOutcome::Reflected => item.reflect_by = Some(f.port),
    }
}

/// `ftMainUpdateShieldStatItem`.
fn update_shield_stat(item: &mut Item, f: &mut Fighter, angle: f32, dir: Vec3) {
    let damage = item.damage_output();
    item.attack.set_hit_interact(
        f.port,
        if item.attack.can_rehit_shield {
            HitType::ShieldRehit
        } else {
            HitType::Shield
        },
    );
    if item.hit_shield_damage < damage {
        item.hit_shield_damage = damage;
        item.shield_collide_angle = angle;
        let z = if f.facing.sign() > 0.0 { -dir.x } else { dir.x };
        item.shield_collide_dir = if z == 0.0 {
            Vec3::ZERO
        } else {
            Vec3::new(0.0, 0.0, z.signum())
        };
    }
    f.hits.shield_damage_total += damage + item.attack.shield_damage;
    if f.hits.shield_damage < damage {
        f.hits.shield_damage = damage;
        f.hits.shield_lr = if item.vel_air.x < 0.0 { 1.0 } else { -1.0 };
    }
}

/// `ftMainUpdateDamageStatItem` for the non-touch items. Returns whether
/// the damage was taken.
fn update_damage_stat(item: &mut Item, f: &mut Fighter, hit: crate::hurtbox::HurtHit) -> bool {
    let output = item.damage_output();
    item.attack.set_hit_interact(
        f.port,
        if item.attack.can_rehit_fighter {
            HitType::DamageRehit
        } else {
            HitType::Damage
        },
    );
    let damage = crate::attack::captured_damage(f, output);
    if item.attack.can_rehit_fighter {
        item.hit_refresh_damage = item.hit_refresh_damage.max(damage);
    } else {
        item.hit_normal_damage = item.hit_normal_damage.max(damage);
    }
    item.hit_lr = attacker_lr(item.vel_air.x, item.pos.x, f.pos.x);
    if combat::is_body_normal(f)
        && hit.hitstatus == HitStatus::Normal
        && combat::check_get_update_damage(f, damage)
    {
        combat::push_log(
            f,
            HitLogEntry {
                source: HitSource::Weapon {
                    vel_x: item.vel_air.x,
                },
                hitbox: item.attack.hitbox(output),
                attacker_pos: item.pos,
                attack_handicap: item.handicap,
                placement: hit.placement,
            },
        );
        return true;
    }
    false
}
