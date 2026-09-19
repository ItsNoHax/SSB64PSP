//! Spawn requests for fighter-owned weapons.
//!
//! The original keeps weapons in the match-wide `wpManager`, not inside their
//! owner fighter. A fighter therefore emits a small, portable request and the
//! match layer owns the bounded weapon pool that consumes it. Keeping that
//! boundary explicit prevents projectile lifetime/collision from becoming a
//! hidden per-fighter side effect.

use ssb_engine::math::{sin_cos, Vec3};

use crate::attack::{self, Hitbox};
use crate::collision::Segment;
use crate::fighter::Fighter;
use crate::ground::{self, BodyColl};

/// The weapon families that a fighter status can request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeaponKind {
    /// `nWPKindFireball`, created by Mario's `SpecialN` accessory callback.
    MarioFireball,
}

/// One deferred weapon creation. The owner is identified by player port, the
/// stable match-local identity used by the portable fighter layer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WeaponSpawn {
    pub kind: WeaponKind,
    pub owner_port: u8,
    pub position: Vec3,
    /// The owner's left/right direction at the motion-script event.
    pub facing: f32,
}

/// Mario's `dMarioSpecial1_Fireball_WeaponAttributes` and the fixed wrapper
/// attributes in `wpmariofireball.c` (US version).
pub const MARIO_FIREBALL_LIFETIME: u16 = 140;
pub const MARIO_FIREBALL_GRAVITY: f32 = 1.2;
pub const MARIO_FIREBALL_TERMINAL_VELOCITY: f32 = 55.0;
pub const MARIO_FIREBALL_SPEED: f32 = 50.0;
pub const MARIO_FIREBALL_ANGLE: f32 = -0.087_266_46; // -5 degrees
pub const MARIO_FIREBALL_REBOUND: f32 = 0.85;
pub const MARIO_FIREBALL_MIN_SPEED: f32 = 30.0;

/// The source `WPAttributes` hitbox, stored in the Mario Special1 reloc file.
pub const MARIO_FIREBALL_HITBOX: Hitbox = Hitbox {
    damage: 7,
    offset: Vec3::ZERO,
    radius: 100.0,
    angle: 361,
    kb_scale: 25,
    kb_weight: 0,
    kb_base: 10,
};

/// A live Mario Fireball. Weapons are match-owned, not fighter-owned:
/// the owner port survives long enough to exclude self-hits while the object
/// has its own position, velocity, lifetime, and collision result.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MarioFireball {
    pub owner_port: u8,
    pub position: Vec3,
    pub velocity: Vec3,
    pub lifetime: u16,
}

impl MarioFireball {
    pub fn new(spawn: WeaponSpawn) -> Self {
        let (sin, cos) = sin_cos(MARIO_FIREBALL_ANGLE);
        MarioFireball {
            owner_port: spawn.owner_port,
            position: spawn.position,
            velocity: Vec3::new(
                MARIO_FIREBALL_SPEED * cos * spawn.facing,
                MARIO_FIREBALL_SPEED * sin,
                0.0,
            ),
            lifetime: MARIO_FIREBALL_LIFETIME,
        }
    }

    /// `wpMarioFireballProcUpdate` followed by the portable floor subset of
    /// `wpMarioFireballProcMap`. The shared stage layer has no wall/ceiling
    /// queries yet, so it can faithfully rebound on floor contact now and will
    /// gain the other source map directions with that shared collision work.
    fn tick<I, F>(&mut self, floors: F) -> bool
    where
        F: Fn() -> I,
        I: IntoIterator<Item = (u16, Segment)>,
    {
        if self.lifetime == 0 {
            return false;
        }
        self.lifetime -= 1;
        if self.lifetime == 0 {
            return false;
        }
        self.velocity.y =
            (self.velocity.y - MARIO_FIREBALL_GRAVITY).max(-MARIO_FIREBALL_TERMINAL_VELOCITY);
        let wanted = self.position + self.velocity;
        let moved = ground::move_air(&BodyColl::default(), self.position, wanted, None, floors);
        self.position = moved.pos;
        if let Some(floor) = moved.floor {
            let normal = floor.normal;
            let dot = self.velocity.x * normal.x + self.velocity.y * normal.y;
            self.velocity.x = (self.velocity.x - 2.0 * dot * normal.x) * MARIO_FIREBALL_REBOUND;
            self.velocity.y = (self.velocity.y - 2.0 * dot * normal.y) * MARIO_FIREBALL_REBOUND;
            if self.velocity.x * self.velocity.x + self.velocity.y * self.velocity.y
                < MARIO_FIREBALL_MIN_SPEED * MARIO_FIREBALL_MIN_SPEED
            {
                return false;
            }
        }
        true
    }
}

/// The portable stand-in for `wpManager`'s live-object list. It is fixed-size
/// so PSP gameplay stays allocation-free; a full four-player game has room
/// for sixteen simultaneous weapons while the first Fireball batch consumes
/// only one slot per use.
pub const MAX_WEAPONS: usize = 16;

#[derive(Debug, Clone, PartialEq)]
pub struct WeaponPool {
    slots: [Option<MarioFireball>; MAX_WEAPONS],
}

impl Default for WeaponPool {
    fn default() -> Self {
        WeaponPool {
            slots: [None; MAX_WEAPONS],
        }
    }
}

impl WeaponPool {
    /// Consumes a fighter's deferred request. A full pool follows the source
    /// manager's allocation-failure shape: the already-consumed script event
    /// is not retried on a later frame.
    pub fn spawn(&mut self, spawn: WeaponSpawn) -> bool {
        let Some(slot) = self.slots.iter_mut().find(|slot| slot.is_none()) else {
            return false;
        };
        *slot = Some(match spawn.kind {
            WeaponKind::MarioFireball => MarioFireball::new(spawn),
        });
        true
    }

    /// Advances lifetime, gravity, movement, and the portable stage rebound
    /// path once. Call this once per match frame, before [`Self::apply_hits`].
    pub fn tick<I, F>(&mut self, floors: F)
    where
        F: Fn() -> I + Copy,
        I: IntoIterator<Item = (u16, Segment)>,
    {
        for slot in &mut self.slots {
            if let Some(fireball) = slot.as_mut() {
                if !fireball.tick(floors) {
                    *slot = None;
                }
            }
        }
    }

    /// Resolves every eligible Fireball against one fighter. A collision
    /// deletes the projectile through `wpMarioFireballProcHit`; invincibility
    /// leaves it live, exactly like a non-registered source hitbox.
    pub fn apply_hits(&mut self, defender: &mut Fighter) {
        for slot in &mut self.slots {
            let Some(fireball) = slot else { continue };
            if fireball.owner_port == defender.port {
                continue;
            }
            if attack::apply_hitbox_at(&MARIO_FIREBALL_HITBOX, fireball.position, defender) {
                *slot = None;
            }
        }
    }

    pub fn active_count(&self) -> usize {
        self.slots.iter().filter(|slot| slot.is_some()).count()
    }

    pub fn first_fireball(&self) -> Option<MarioFireball> {
        self.slots.iter().flatten().copied().next()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fighter::{FighterKind, Situation};

    fn open_air() -> [(u16, Segment); 0] {
        []
    }

    #[test]
    fn mario_fireball_uses_the_sourced_launch_and_lifetime() {
        let mut weapons = WeaponPool::default();
        assert!(weapons.spawn(WeaponSpawn {
            kind: WeaponKind::MarioFireball,
            owner_port: 0,
            position: Vec3::ZERO,
            facing: 1.0,
        }));
        let fireball = weapons.first_fireball().unwrap();
        assert_eq!(fireball.lifetime, MARIO_FIREBALL_LIFETIME);
        assert!(fireball.velocity.x > 49.0);
        assert!(fireball.velocity.y < 0.0);

        weapons.tick(open_air);
        let fireball = weapons.first_fireball().unwrap();
        assert_eq!(fireball.lifetime, MARIO_FIREBALL_LIFETIME - 1);
        let (sin, _) = sin_cos(MARIO_FIREBALL_ANGLE);
        assert_eq!(
            fireball.velocity.y,
            MARIO_FIREBALL_SPEED * sin - MARIO_FIREBALL_GRAVITY
        );
    }

    #[test]
    fn a_fireball_hits_an_opponent_once_and_never_its_owner() {
        let mut weapons = WeaponPool::default();
        assert!(weapons.spawn(WeaponSpawn {
            kind: WeaponKind::MarioFireball,
            owner_port: 0,
            position: Vec3::ZERO,
            facing: 1.0,
        }));
        let mut owner = Fighter::new(FighterKind::Mario, 0, 3);
        let mut target = Fighter::new(FighterKind::Mario, 1, 3);
        owner.situation = Situation::Ground;
        target.situation = Situation::Ground;

        weapons.apply_hits(&mut owner);
        assert_eq!(weapons.active_count(), 1);
        weapons.apply_hits(&mut target);
        assert_eq!(target.damage, 7);
        assert_eq!(weapons.active_count(), 0);
    }
}
