//! PK Fire and PK Thunder (US). PK Fire's unpickable items have their own
//! source-sized allocation table, tick and records, hosted by the match pool.
//! General item damage and weapon shields await shared systems.
use super::{map_contact, BodyColl, Hitbox, MapSurface, MapSurfaceKind, OwnerView, WeaponSpawn};
use crate::fighter::Fighter;
use ssb_engine::math::{atan2, sin_cos, Vec2, Vec3};

pub const SPARK_HIT: Hitbox = Hitbox {
    damage: 4,
    offset: Vec3::ZERO,
    radius: 100.0,
    angle: 80,
    kb_scale: 50,
    kb_weight: 0,
    kb_base: 40,
};
pub const HEAD_HIT: Hitbox = Hitbox {
    damage: 6,
    offset: Vec3::ZERO,
    radius: 75.0,
    angle: 100,
    kb_scale: 30,
    kb_weight: 0,
    kb_base: 50,
};
pub const TRAIL_HIT: Hitbox = Hitbox {
    damage: 3,
    offset: Vec3::new(0.0, -100.0, 0.0),
    radius: 50.0,
    angle: 100,
    kb_scale: 30,
    kb_weight: 0,
    kb_base: 50,
};
pub const PILLAR_HIT: Hitbox = Hitbox {
    damage: 3,
    offset: Vec3::ZERO,
    radius: 100.0,
    angle: 70,
    kb_scale: 10,
    kb_weight: 0,
    kb_base: 4,
};

/// `ITEM_ALLOC_MAX`, independent of the weapon manager's capacity.
const MAX_PK_FIRE_ITEMS: usize = 16;
#[derive(Debug, Clone, Copy, PartialEq)]
struct PKFireItem {
    pillar: PKFirePillar,
    stale: crate::stale::WeaponStale,
    landed: Option<(u8, crate::stale::MotionAttackId, u16)>,
}
#[derive(Debug, Clone, PartialEq)]
pub(super) struct PKFireItems {
    slots: [Option<PKFireItem>; MAX_PK_FIRE_ITEMS],
}
impl Default for PKFireItems {
    fn default() -> Self {
        Self {
            slots: [None; MAX_PK_FIRE_ITEMS],
        }
    }
}
impl PKFireItems {
    pub(super) fn insert(&mut self, pillar: PKFirePillar, stale: crate::stale::WeaponStale) {
        if let Some(slot) = self.slots.iter_mut().find(|s| s.is_none()) {
            *slot = Some(PKFireItem {
                pillar,
                stale,
                landed: None,
            });
        }
    }
    pub(super) fn tick<I, F>(&mut self, surfaces: F)
    where
        F: Fn() -> I + Copy,
        I: IntoIterator<Item = MapSurface>,
    {
        for slot in &mut self.slots {
            if let Some(item) = slot {
                if !item.pillar.tick(surfaces) {
                    *slot = None;
                }
            }
        }
    }
    pub(super) fn apply_hits(&mut self, defender: &mut Fighter) {
        let port = (defender.port & 7) as usize;
        for item in self.slots.iter_mut().flatten() {
            let p = &mut item.pillar;
            if p.owner_port == defender.port || p.rehit[port] != 0 {
                continue;
            }
            let mut hit = PILLAR_HIT;
            hit.radius *= p.scale;
            for y in [100.0, 350.0] {
                if super::stale_hit(
                    &hit,
                    p.position + Vec3::new(0.0, y * p.scale, 0.0),
                    p.velocity.x,
                    item.stale,
                    defender,
                    &mut item.landed,
                    p.owner_port,
                )
                .registered()
                {
                    p.rehit[port] = 16;
                    break;
                }
            }
        }
    }
    pub(super) fn record_landed(&mut self, owner: &mut Fighter) {
        for item in self.slots.iter_mut().flatten() {
            if let Some((port, id, count)) = item.landed {
                if port == owner.port {
                    owner.stale.push(id, count);
                    item.landed = None;
                }
            }
        }
    }
    pub(super) fn pillars(&self) -> impl Iterator<Item = PKFirePillar> + '_ {
        self.slots.iter().flatten().map(|item| item.pillar)
    }
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PKFire {
    pub owner_port: u8,
    pub position: Vec3,
    pub velocity: Vec3,
    pub lifetime: u16,
    pub damage: i32,
}
impl PKFire {
    pub fn new(spawn: WeaponSpawn, grounded: bool) -> Self {
        let (angle, speed): (f32, f32) = if grounded {
            (-3.6, 73.0)
        } else {
            (-38.0, 95.0)
        };
        let (sin, cos) = sin_cos(angle * core::f32::consts::PI / 180.0);
        Self {
            owner_port: spawn.owner_port,
            position: spawn.position,
            velocity: Vec3::new(cos * speed * spawn.facing, sin * speed, 0.0),
            lifetime: 20,
            damage: 4,
        }
    }
    pub(super) fn tick<I, F>(&mut self, surfaces: F) -> bool
    where
        F: Fn() -> I,
        I: IntoIterator<Item = MapSurface>,
    {
        self.lifetime = self.lifetime.saturating_sub(1);
        if self.lifetime == 0 {
            return false;
        }
        let wanted = self.position + self.velocity;
        if map_contact(
            surfaces(),
            self.position,
            wanted,
            BodyColl {
                top: 10.0,
                center: 0.0,
                bottom: -10.0,
                width: 10.0,
            },
        )
        .is_some()
        {
            return false;
        }
        self.position = wanted;
        true
    }
    pub(super) fn reflect(&mut self, f: &Fighter) {
        self.owner_port = f.port;
        if self.velocity.x * f.facing.sign() < 0.0 {
            self.velocity.x = -self.velocity.x;
        }
        self.lifetime = 20;
        self.damage = ((self.damage as f32 * 1.8 + 0.99) as i32).min(100);
    }
    pub(super) fn pillar(&self) -> PKFirePillar {
        PKFirePillar {
            owner_port: self.owner_port,
            position: self.position + self.velocity.normalized() * 160.0,
            velocity: Vec3::ZERO,
            lifetime: 100,
            scale: 1.0,
            floor: None,
            rehit: [0; 8],
            initialized: false,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PKFirePillar {
    pub owner_port: u8,
    pub position: Vec3,
    pub velocity: Vec3,
    pub lifetime: i32,
    pub scale: f32,
    pub(super) floor: Option<MapSurface>,
    pub(super) rehit: [u8; 8],
    initialized: bool,
}
impl PKFirePillar {
    pub(super) fn tick<I, F>(&mut self, surfaces: F) -> bool
    where
        F: Fn() -> I,
        I: IntoIterator<Item = MapSurface>,
    {
        // itProcessUpdateAttackRecords runs even on the initial status change.
        for timer in &mut self.rehit {
            *timer = timer.saturating_sub(1);
        }
        // The item descriptor's initial update just selects Fall.
        if !self.initialized {
            self.initialized = true;
            self.velocity.x = 0.0;
            self.velocity.y = 0.0;
            return true;
        }
        self.scale = (self.lifetime as f32 * 0.5 / 100.0) + 0.5;
        self.lifetime -= 1;
        if self.lifetime < 0 {
            return false;
        }
        if self
            .floor
            .is_some_and(|floor| !surfaces().into_iter().any(|s| s == floor))
        {
            self.floor = None;
            self.velocity.x = 0.0;
            self.velocity.y = 0.0;
        }
        if self.floor.is_none() {
            self.velocity.y = (self.velocity.y - 0.45).max(-55.0);
            let wanted = self.position + self.velocity;
            if let Some(hit) = map_contact(
                surfaces(),
                self.position,
                wanted,
                BodyColl {
                    top: 400.0,
                    center: 200.0,
                    bottom: 0.0,
                    width: 100.0,
                },
            ) {
                self.position = hit.position;
                if hit.kind == MapSurfaceKind::Floor {
                    self.floor = surfaces()
                        .into_iter()
                        .find(|s| s.kind == hit.kind && s.segment == hit.segment);
                    self.velocity = Vec3::ZERO;
                } else {
                    let dot = self.velocity.x * hit.normal.x + self.velocity.y * hit.normal.y;
                    self.velocity.x = (self.velocity.x - 2.0 * dot * hit.normal.x) * 0.2;
                    self.velocity.y = (self.velocity.y - 2.0 * dot * hit.normal.y) * 0.2;
                }
            } else {
                self.position = wanted;
            }
        }
        true
    }
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PKThunder {
    pub owner_port: u8,
    pub position: Vec3,
    pub velocity: Vec3,
    pub lifetime: u16,
    pub damage: i32,
    pub motion_count: u16,
    pub group: u16,
    pub reflected: bool,
    pub angle: f32,
    pub(super) history: [Vec2; 12],
    pub(super) cursor: usize,
    pub(super) trail_spawn: bool,
}
impl PKThunder {
    pub fn new(spawn: WeaponSpawn, group: u16) -> Self {
        Self {
            owner_port: spawn.owner_port,
            position: spawn.position,
            velocity: Vec3::new(0.0, 60.0, 0.0),
            lifetime: 160,
            damage: 6,
            motion_count: spawn.stale.motion_count,
            group,
            reflected: false,
            angle: core::f32::consts::FRAC_PI_2,
            history: [Vec2::ZERO; 12],
            cursor: 0,
            trail_spawn: false,
        }
    }
    pub(super) fn tick<I, F>(&mut self, surfaces: F, owner: Option<OwnerView>) -> bool
    where
        F: Fn() -> I,
        I: IntoIterator<Item = MapSurface>,
    {
        self.trail_spawn = self.lifetime == 158;
        self.lifetime = self.lifetime.saturating_sub(1);
        if self.lifetime == 0 {
            return false;
        }
        if !self.reflected {
            let Some(owner) = owner else { return false };
            if !owner.ness_control || owner.ness_motion != self.motion_count || owner.ness_collide {
                return false;
            }
            self.cursor = (self.cursor + 1) % 12;
            self.history[(self.cursor + 11) % 12] =
                Vec2::new(self.position.x as i16 as f32, self.position.y as i16 as f32);
            if i32::from(owner.stick.x).abs() + i32::from(owner.stick.y).abs() > 45 {
                let target = atan2(owner.stick.y as f32, owner.stick.x as f32);
                let mut diff = target - self.angle;
                while diff > core::f32::consts::PI {
                    diff -= core::f32::consts::TAU;
                }
                while diff < -core::f32::consts::PI {
                    diff += core::f32::consts::TAU;
                }
                // At 180 degrees the zero cross product selects a negative step.
                let sign = if (self.velocity.x * owner.stick.y as f32
                    - self.velocity.y * owner.stick.x as f32)
                    > 0.0
                {
                    1.0
                } else {
                    -1.0
                };
                let step = if diff.abs() >= core::f32::consts::FRAC_PI_4 {
                    6.0 * core::f32::consts::PI / 180.0
                } else {
                    diff.abs() / 7.5
                };
                self.angle += step * sign;
                let (sin, cos) = sin_cos(self.angle);
                self.velocity = Vec3::new(cos * 60.0, sin * 60.0, 0.0);
            }
        }
        let wanted = self.position + self.velocity;
        if map_contact(
            surfaces(),
            self.position,
            wanted,
            BodyColl {
                top: 100.0,
                center: 0.0,
                bottom: -100.0,
                width: 100.0,
            },
        )
        .is_some()
        {
            return false;
        }
        self.position = wanted;
        true
    }
    pub(super) fn reflect(&mut self, f: &Fighter, group: u16) {
        let first = !self.reflected;
        self.reflected = true;
        self.owner_port = f.port;
        self.lifetime = 160;
        self.group = group;
        if first {
            let direction = self.position - (f.pos + Vec3::new(0.0, 250.0, 0.0));
            self.velocity = direction.normalized() * 60.0;
            self.velocity.z = 0.0;
        } else if self.velocity.x * f.facing.sign() < 0.0 {
            self.velocity.x = -self.velocity.x;
        }
        self.damage = ((self.damage as f32 * 1.8 + 0.99) as i32).min(100);
    }
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PKThunderTrail {
    pub owner_port: u8,
    pub position: Vec3,
    pub group: u16,
    pub id: u8,
    pub lifetime: u16,
    pub hit_ports: u8,
    pub(super) rotation: f32,
    pub(super) spawn_next: bool,
}
impl PKThunderTrail {
    pub(super) fn new(head: PKThunder, id: u8) -> Self {
        Self {
            owner_port: head.owner_port,
            position: head.position,
            group: head.group,
            id,
            lifetime: 160,
            hit_ports: 0,
            // Reflected children inherit their parent's rotation minus 90°.
            // Keep the source's cumulative turns, including its janky offsets.
            rotation: if head.reflected {
                atan2(head.velocity.y, head.velocity.x)
                    - core::f32::consts::FRAC_PI_2 * (id as f32 + 1.0)
            } else {
                0.0
            },
            spawn_next: false,
        }
    }
    pub(super) fn tick(&mut self, head: PKThunder) {
        self.spawn_next = self.id < 3 && self.lifetime == 158;
        self.lifetime = self.lifetime.saturating_sub(1);
        if head.reflected {
            self.position = head.position - head.velocity * ((self.id as f32 + 1.5) * 2.0);
        } else {
            let index = (head.cursor + 12 - (self.id as usize + 1) * 2) % 12;
            let pos = head.history[index];
            let prev = head.history[(index + 11) % 12];
            self.position = Vec3::new(pos.x, pos.y, 0.0);
            self.rotation = atan2(pos.y - prev.y, pos.x - prev.x) - core::f32::consts::FRAC_PI_2;
        }
    }
    pub(super) fn hit_position(&self) -> Vec3 {
        let (sin, cos) = sin_cos(self.rotation);
        self.position + Vec3::new(100.0 * sin, -100.0 * cos, 0.0)
    }
}
