//! Saffron City's Charmander flame and Venusaur razor (`ithitokage.c`,
//! `itfushigibana.c`), made with `WEAPON_FLAG_PARENT_ITEM`.

use crate::attack::Hitbox;
use crate::combat::Element;
use crate::weapon::{map_contact, MapSurface};
use crate::wpeffect::{Emit, WeaponEffect as Fx};
use ssb_engine::math::Vec3;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MonsterShot {
    pub razor: bool,
    pub owner_port: u8,
    pub damage: i32,
    pub position: Vec3,
    pub velocity: Vec3,
    pub lifetime: u16,
    pub lr: f32,
    pub rotate_z: f32,
}

impl MonsterShot {
    pub fn new(razor: bool, position: Vec3) -> Self {
        let (sin, cos) = ssb_engine::math::sin_cos(-0.209_439_52);
        Self {
            razor,
            owner_port: 4,
            damage: if razor { 3 } else { 2 },
            position,
            velocity: if razor {
                Vec3::new(-100.0, 0.0, 0.0)
            } else {
                Vec3::new(cos * -45.0, sin * 45.0, 0.0)
            },
            lifetime: if razor { 24 } else { 20 },
            lr: -1.0,
            rotate_z: 0.0,
        }
    }

    /// File 264's WPAttributes, 0x244 (flame) and 0x308 (razor).
    pub fn hitbox(&self) -> Hitbox {
        Hitbox {
            damage: self.damage,
            offset: Vec3::ZERO,
            radius: if self.razor { 100.0 } else { 160.0 },
            angle: if self.razor { 90 } else { 0 },
            kb_scale: if self.razor { 60 } else { 100 },
            kb_weight: if self.razor { 0 } else { 3 },
            kb_base: if self.razor { 30 } else { 0 },
            element: if self.razor {
                Element::Normal
            } else {
                Element::Fire
            },
            shield_damage: 1,
        }
    }

    pub(crate) fn make_fx(&self, fx: &mut Emit) {
        if !self.razor {
            fx.push(Fx::MonsterFlame {
                pos: self.position,
                vel: self.velocity,
            });
        }
    }

    pub(crate) fn hit_fx(&self, fx: &mut Emit) {
        if self.razor {
            fx.push(Fx::DamageSlash {
                pos: self.position,
                size: self.damage,
                lr: self.lr,
            });
        } else {
            fx.push(Fx::SparkleWhite(self.position));
        }
    }

    pub(crate) fn tick<I, F>(&mut self, surfaces: F, fx: &mut Emit) -> bool
    where
        F: Fn() -> I,
        I: IntoIterator<Item = MapSurface>,
    {
        if self.razor {
            self.velocity.x += 5.0 * self.lr;
        }
        self.lifetime = self.lifetime.saturating_sub(1);
        if self.lifetime == 0 {
            return false;
        }
        let wanted = self.position + self.velocity;
        if !self.razor {
            let coll = crate::ground::BodyColl {
                top: 50.0,
                center: 0.0,
                bottom: -50.0,
                width: 50.0,
            };
            if let Some(hit) = map_contact(surfaces(), self.position, wanted, coll) {
                self.position = hit.position;
                fx.push(Fx::DustExpandSmall(self.position));
                return false;
            }
        }
        self.position = wanted;
        true
    }

    pub(crate) fn reface(&mut self) {
        self.rotate_z =
            ssb_engine::math::atan2(self.velocity.y, self.velocity.x) + core::f32::consts::PI;
    }
}
