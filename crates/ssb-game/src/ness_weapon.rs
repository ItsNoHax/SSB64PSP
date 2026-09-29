//! PK Fire and PK Thunder (US). PK Fire's flame is an item
//! (`crate::item::pk_fire`): the spark's hit callback queues it for the item
//! pool.
use super::{map_contact, BodyColl, Hitbox, MapSurface, OwnerView, WeaponSpawn};
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
    element: crate::combat::Element::Fire,
    shield_damage: 1,
};
pub const HEAD_HIT: Hitbox = Hitbox {
    damage: 6,
    offset: Vec3::ZERO,
    radius: 75.0,
    angle: 100,
    kb_scale: 30,
    kb_weight: 0,
    kb_base: 50,
    element: crate::combat::Element::Electric,
    shield_damage: 1,
};
pub const TRAIL_HIT: Hitbox = Hitbox {
    damage: 3,
    offset: Vec3::new(0.0, -100.0, 0.0),
    radius: 50.0,
    angle: 100,
    kb_scale: 30,
    kb_weight: 0,
    kb_base: 50,
    element: crate::combat::Element::Electric,
    shield_damage: 1,
};
/// The spark's `WPAttributes` map collision box.
const SPARK_MAP_COLL: BodyColl = BodyColl {
    top: 10.0,
    center: 0.0,
    bottom: -10.0,
    width: 10.0,
};
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PKFire {
    pub owner_port: u8,
    pub position: Vec3,
    pub velocity: Vec3,
    pub lifetime: u16,
    pub damage: i32,
    /// Kind 46's spin: `wpNessPKFireMakeWeapon` sets `(angle + 90°) * lr`,
    /// and the hop and reflector negate it. Presentation only.
    pub rotate_z: f32,
    /// `gcPlayAnimAll` calls on its material animation, one per update.
    pub anim_ticks: u16,
}
impl PKFire {
    pub fn new(spawn: WeaponSpawn, grounded: bool) -> Self {
        let (angle, speed): (f32, f32) = if grounded {
            (-3.6, 73.0)
        } else {
            (-38.0, 95.0)
        };
        let radians = angle * core::f32::consts::PI / 180.0;
        let (sin, cos) = sin_cos(radians);
        let velocity = Vec3::new(cos * speed * spawn.facing, sin * speed, 0.0);
        // `wpMainVelSetLR`.
        let lr = if velocity.x >= 0.0 { 1.0 } else { -1.0 };
        Self {
            owner_port: spawn.owner_port,
            position: spawn.position,
            velocity,
            lifetime: 20,
            damage: 4,
            rotate_z: (radians + core::f32::consts::FRAC_PI_2) * lr,
            anim_ticks: 0,
        }
    }
    pub(super) fn tick<I, F>(&mut self, surfaces: F) -> bool
    where
        F: Fn() -> I,
        I: IntoIterator<Item = MapSurface>,
    {
        // `wpProcessProcWeaponMain` plays the animation first.
        self.anim_ticks = self.anim_ticks.wrapping_add(1);
        self.lifetime = self.lifetime.saturating_sub(1);
        if self.lifetime == 0 {
            return false;
        }
        let wanted = self.position + self.velocity;
        if map_contact(surfaces(), self.position, wanted, SPARK_MAP_COLL).is_some() {
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
        self.rotate_z = -self.rotate_z;
    }
    /// `wpNessPKFireProcHit`: the flame goes 160 units along the spark's
    /// travel (`WPPKFIRE_POS_MUL`) and is projected from the spark.
    pub(super) fn item_spawn(
        &self,
        stale: crate::stale::WeaponStale,
        team: u8,
    ) -> crate::item::PKFireSpawn {
        crate::item::PKFireSpawn {
            owner_port: self.owner_port,
            team,
            pos: self.position + self.velocity.normalized() * 160.0,
            weapon_pos: self.position,
            weapon_coll: SPARK_MAP_COLL,
            stale,
        }
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
    /// `gcPlayAnimAll` calls on the head since its weapon was made (a
    /// reflect makes a new one). Presentation only.
    pub anim_ticks: u16,
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
            anim_ticks: 0,
        }
    }
    /// The head's kind-46 spin: `angle - 90°` while steered;
    /// `wpNessPKThunderReflectHeadMakeWeapon` sets `atan2(vel_air)`.
    pub fn rotate_z(&self) -> f32 {
        if self.reflected {
            atan2(self.velocity.y, self.velocity.x)
        } else {
            self.angle - core::f32::consts::FRAC_PI_2
        }
    }
    pub(super) fn tick<I, F>(&mut self, surfaces: F, owner: Option<OwnerView>) -> bool
    where
        F: Fn() -> I,
        I: IntoIterator<Item = MapSurface>,
    {
        // `wpProcessProcWeaponMain` plays the DObj animation first.
        self.anim_ticks = self.anim_ticks.wrapping_add(1);
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
            // `wpNessPKThunderReflectHeadMakeWeapon`: a new weapon, whose
            // `wpManagerMakeWeapon` adds the animation afresh.
            self.anim_ticks = 0;
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
    /// Root `rotate.z`: the direction from the previous trail sample, less
    /// 90 degrees.
    pub fn rotation(&self) -> f32 {
        self.rotation
    }
    pub(super) fn hit_position(&self) -> Vec3 {
        let (sin, cos) = sin_cos(self.rotation);
        self.position + Vec3::new(100.0 * sin, -100.0 * cos, 0.0)
    }
}
