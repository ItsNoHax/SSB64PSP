//! Spawn requests for fighter-owned weapons.
//!
//! The original keeps weapons in the match-wide `wpManager`, not inside their
//! owner fighter. A fighter therefore emits a small, portable request and the
//! match layer owns the bounded weapon pool that consumes it. Keeping that
//! boundary explicit prevents projectile lifetime/collision from becoming a
//! hidden per-fighter side effect.

use ssb_engine::math::{sin_cos, Vec2, Vec3};

use crate::attack::{self, Hitbox};
use crate::collision::Segment;
use crate::fighter::Fighter;
use crate::ground::BodyColl;

/// The one-sided role a map segment has in the original collision tables.
///
/// This is intentionally separate from the ROM pack's `line_kind`: weapons
/// are portable gameplay objects, while the PSP runtime merely adapts packed
/// lines into this input shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapSurfaceKind {
    Floor,
    Ceiling,
    /// A right-hand map boundary. Its outward normal points left.
    RightWall,
    /// A left-hand map boundary. Its outward normal points right.
    LeftWall,
}

/// One stage segment presented to a weapon map query.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MapSurface {
    pub kind: MapSurfaceKind,
    pub segment: Segment,
}

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

/// `WPAttributes.map_coll` from Mario Special1: the projectile's collision
/// diamond is deliberately larger than its rendered sprite.
pub const MARIO_FIREBALL_MAP_COLL: BodyColl = BodyColl {
    top: 50.0,
    center: 0.0,
    bottom: -50.0,
    width: 50.0,
};

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

    /// `wpMarioFireballProcUpdate` then `wpMarioFireballProcMap`.
    ///
    /// The source calls `wpMapTestAll` with its authored collision diamond,
    /// then only rebounds when entering a floor/ceiling/wall surface this
    /// frame. The portable sweep supplies that same one-sided map contract
    /// without coupling the match-owned weapon pool to a particular stage
    /// format or runtime.
    fn tick<I, F>(&mut self, surfaces: F) -> bool
    where
        F: Fn() -> I,
        I: IntoIterator<Item = MapSurface>,
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
        if let Some(hit) = map_contact(surfaces(), self.position, wanted) {
            self.position = hit.position;
            let dot = self.velocity.x * hit.normal.x + self.velocity.y * hit.normal.y;
            self.velocity.x = (self.velocity.x - 2.0 * dot * hit.normal.x) * MARIO_FIREBALL_REBOUND;
            self.velocity.y = (self.velocity.y - 2.0 * dot * hit.normal.y) * MARIO_FIREBALL_REBOUND;
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
    pub fn tick<I, F>(&mut self, surfaces: F)
    where
        F: Fn() -> I + Copy,
        I: IntoIterator<Item = MapSurface>,
    {
        for slot in &mut self.slots {
            if let Some(fireball) = slot.as_mut() {
                if !fireball.tick(surfaces) {
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

    /// Every live Fireball, for the runtime presentation layer. The pool
    /// remains the sole owner of weapon state; renderers receive snapshots.
    pub fn fireballs(&self) -> impl Iterator<Item = MarioFireball> + '_ {
        self.slots.iter().flatten().copied()
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct MapContact {
    position: Vec3,
    normal: Vec2,
    time: f32,
}

/// Finds the first one-sided contact of Mario's authored map-collision
/// diamond. `mpProcessUpdateMain` performs this before `wpMapCheckAllRebound`;
/// support-point sweeping is the allocation-free equivalent of its individual
/// bottom/top/side probes for a weapon that has one symmetric diamond.
fn map_contact<I>(surfaces: I, from: Vec3, to: Vec3) -> Option<MapContact>
where
    I: IntoIterator<Item = MapSurface>,
{
    let mut first: Option<MapContact> = None;
    let delta = Vec2::new(to.x - from.x, to.y - from.y);

    for surface in surfaces {
        let normal = surface_normal(surface.kind, surface.segment);
        // `wpMapCheckAllRebound` only reflects an incoming velocity
        // (`lbCommonSim2D(vel, angle) < 0`). This also excludes a point left
        // touching a surface after the preceding frame's rebound.
        if delta.x * normal.x + delta.y * normal.y >= 0.0 {
            continue;
        }
        let support = diamond_support_toward_surface(normal);
        let probe_from = Vec2::new(from.x + support.x, from.y + support.y);
        let probe_to = Vec2::new(to.x + support.x, to.y + support.y);
        let Some(time) = swept_segment_intersection(probe_from, probe_to, surface.segment) else {
            continue;
        };
        if first.is_some_and(|known| time >= known.time) {
            continue;
        }
        first = Some(MapContact {
            position: Vec3::new(from.x + delta.x * time, from.y + delta.y * time, from.z),
            normal,
            time,
        });
    }
    first
}

/// The diamond point farthest *into* a surface, matching the source map
/// collision extents `(top, center, bottom, width)`.
fn diamond_support_toward_surface(normal: Vec2) -> Vec2 {
    let candidates = [
        Vec2::new(0.0, MARIO_FIREBALL_MAP_COLL.top),
        Vec2::new(
            MARIO_FIREBALL_MAP_COLL.width,
            MARIO_FIREBALL_MAP_COLL.center,
        ),
        Vec2::new(0.0, MARIO_FIREBALL_MAP_COLL.bottom),
        Vec2::new(
            -MARIO_FIREBALL_MAP_COLL.width,
            MARIO_FIREBALL_MAP_COLL.center,
        ),
    ];
    candidates
        .into_iter()
        .min_by(|a, b| {
            let da = a.x * normal.x + a.y * normal.y;
            let db = b.x * normal.x + b.y * normal.y;
            da.partial_cmp(&db).unwrap_or(core::cmp::Ordering::Equal)
        })
        .unwrap_or(Vec2::ZERO)
}

/// The collision normal represented by a one-sided map line. Source map lines
/// are grouped by kind before testing, so their winding is not gameplay data;
/// choose the perpendicular whose signed axis matches that group.
fn surface_normal(kind: MapSurfaceKind, s: Segment) -> Vec2 {
    let dx = (s.x2 - s.x1) as f32;
    let dy = (s.y2 - s.y1) as f32;
    let mut n = Vec2::new(-dy, dx);
    let choose_positive = match kind {
        MapSurfaceKind::Floor => n.y < 0.0,
        MapSurfaceKind::Ceiling => n.y > 0.0,
        MapSurfaceKind::RightWall => n.x > 0.0,
        MapSurfaceKind::LeftWall => n.x < 0.0,
    };
    if choose_positive {
        n = Vec2::new(-n.x, -n.y);
    }
    let length = n.length();
    if length == 0.0 {
        return match kind {
            MapSurfaceKind::Floor => Vec2::new(0.0, 1.0),
            MapSurfaceKind::Ceiling => Vec2::new(0.0, -1.0),
            MapSurfaceKind::RightWall => Vec2::new(-1.0, 0.0),
            MapSurfaceKind::LeftWall => Vec2::new(1.0, 0.0),
        };
    }
    Vec2::new(n.x / length, n.y / length)
}

/// Returns the movement fraction where the moving point meets the static
/// segment. The `0.001` edge slack is shared with `mpcollision.c`.
fn swept_segment_intersection(from: Vec2, to: Vec2, s: Segment) -> Option<f32> {
    const EPS: f32 = 0.001;
    let r = Vec2::new(to.x - from.x, to.y - from.y);
    let q = Vec2::new(s.x1 as f32, s.y1 as f32);
    let v = Vec2::new((s.x2 - s.x1) as f32, (s.y2 - s.y1) as f32);
    let cross = |a: Vec2, b: Vec2| a.x * b.y - a.y * b.x;
    let denom = cross(r, v);
    if denom.abs() <= EPS {
        return None;
    }
    let qmp = Vec2::new(q.x - from.x, q.y - from.y);
    let t = cross(qmp, v) / denom;
    let u = cross(qmp, r) / denom;
    ((-EPS..=1.0 + EPS).contains(&t) && (-EPS..=1.0 + EPS).contains(&u))
        .then_some(t.clamp(0.0, 1.0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fighter::{FighterKind, Situation};

    fn open_air() -> [MapSurface; 0] {
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

    fn surface(kind: MapSurfaceKind, x1: i16, y1: i16, x2: i16, y2: i16) -> MapSurface {
        MapSurface {
            kind,
            segment: Segment {
                x1,
                y1,
                x2,
                y2,
                flags: 0,
            },
        }
    }

    #[test]
    fn fireball_rebounds_from_every_authored_map_direction() {
        // Each line is positioned 50 units from the origin: exactly where the
        // sourced map-collision diamond's support point reaches it. The four
        // surface kinds have different outward normals, so this catches a
        // swapped LeftWall/RightWall interpretation rather than merely testing
        // a generic segment hit.
        let cases = [
            (
                surface(MapSurfaceKind::Floor, -100, -50, 100, -50),
                Vec3::new(0.0, -10.0, 0.0),
                Vec3::new(0.0, 1.0, 0.0),
            ),
            (
                surface(MapSurfaceKind::Ceiling, -100, 50, 100, 50),
                Vec3::new(0.0, 10.0, 0.0),
                Vec3::new(0.0, -1.0, 0.0),
            ),
            (
                surface(MapSurfaceKind::RightWall, 50, -100, 50, 100),
                Vec3::new(10.0, 0.0, 0.0),
                Vec3::new(-1.0, 0.0, 0.0),
            ),
            (
                surface(MapSurfaceKind::LeftWall, -50, -100, -50, 100),
                Vec3::new(-10.0, 0.0, 0.0),
                Vec3::new(1.0, 0.0, 0.0),
            ),
        ];

        for (surface, delta, normal) in cases {
            let from = Vec3::ZERO;
            let to = from + delta;
            let hit = map_contact([surface], from, to).expect("the diamond reaches the map line");
            assert_eq!(hit.position, Vec3::ZERO);
            assert_eq!(hit.normal, Vec2::new(normal.x, normal.y));
            let dot = delta.x * hit.normal.x + delta.y * hit.normal.y;
            let reflected = Vec2::new(
                delta.x - 2.0 * dot * hit.normal.x,
                delta.y - 2.0 * dot * hit.normal.y,
            );
            assert!(reflected.x * delta.x + reflected.y * delta.y < 0.0);
        }
    }
}
