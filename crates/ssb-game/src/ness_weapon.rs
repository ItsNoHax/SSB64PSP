//! PK Fire and PK Thunder (US). PK Fire's flame is an item
//! (`crate::item::pk_fire`): the spark's hit callback queues it for the item
//! pool.
use super::{BodyColl, Hitbox, MapSurface, OwnerView, WeaponSpawn};
use crate::wpeffect::{Emit, WeaponEffect as Fx};
use ssb_engine::math::{sin_cos, sy_arc_tan2, Vec2, Vec3};

/// `llNessSpecial1PKFireWeaponAttributes`, `llNessMainPKThunderWeaponAttributes`
/// and `llNessMainPKThunderTrailWeaponAttributes` (the trail with its first
/// attack offset), generated from the ROM at build time.
pub const SPARK_HIT: Hitbox = crate::monster_weapon::weapon_attributes::NESS_PK_FIRE.hitbox();
pub const HEAD_HIT: Hitbox = crate::monster_weapon::weapon_attributes::NESS_PK_THUNDER.hitbox();
pub const TRAIL_HIT: Hitbox = Hitbox {
    offset: crate::monster_weapon::weapon_attributes::NESS_PK_THUNDER_TRAIL.attack_offset(0),
    ..crate::monster_weapon::weapon_attributes::NESS_PK_THUNDER_TRAIL.hitbox()
};
/// `dWPDisplayPKThunderPrimColors`: the PRIM `wpDisplayPKThunderProcDisplay`
/// sets for a trail by its `trail_id` (alpha 0xFF), generated from the ROM.
pub const PK_THUNDER_TRAIL_PRIM: [[u8; 3]; 4] = crate::d_arrays::WP_DISPLAY_PK_THUNDER_PRIM_COLORS;
/// `dWPDisplayPKThunderEnvColors`: the trail's ENV, as
/// [`PK_THUNDER_TRAIL_PRIM`].
pub const PK_THUNDER_TRAIL_ENV: [[u8; 3]; 4] = crate::d_arrays::WP_DISPLAY_PK_THUNDER_ENV_COLORS;
/// The spark's `WPAttributes` map collision box.
const SPARK_MAP_COLL: BodyColl = crate::monster_weapon::weapon_attributes::NESS_PK_FIRE.map_coll;
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
    pub(super) fn tick<I, F>(&mut self, surfaces: F, fx: &mut Emit) -> bool
    where
        F: Fn() -> I,
        I: IntoIterator<Item = MapSurface>,
    {
        // `wpProcessProcWeaponMain` plays the animation first.
        self.anim_ticks = self.anim_ticks.wrapping_add(1);
        self.lifetime = self.lifetime.saturating_sub(1);
        if self.lifetime == 0 {
            // `wpNessPKFireProcUpdate`.
            fx.push(Fx::DustExpandSmall(self.position));
            return false;
        }
        let from = self.position;
        self.position += self.velocity;
        // `wpNessPKFireProcMap`.
        if crate::wpmap::check_coll_end(&SPARK_MAP_COLL, from, &mut self.position, &surfaces) {
            fx.push(Fx::DustExpandSmall(self.position));
            return false;
        }
        true
    }
    /// `wpProcessProcHitCollisions`' owner swap, `wpNessPKFireProcReflector`
    /// (the life, `wpMainReflectorSetLR`, the negated roll) and, as it
    /// returns FALSE, the reflect bonus.
    pub(super) fn reflect(&mut self, r: &super::Reflector) {
        self.owner_port = r.port;
        if self.velocity.x * r.lr < 0.0 {
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
            sy_arc_tan2(self.velocity.y, self.velocity.x)
        } else {
            self.angle - core::f32::consts::FRAC_PI_2
        }
    }
    /// The trail effect's process (`efManagerNessPKThunderTrailProcUpdate`)
    /// against this head: the owner's ring sample ten frames back
    /// (`pkthunder_trail_id - 10`) and `rotate.z` from the sample before
    /// it, less 90 degrees. Reflected
    /// (`efManagerNessPKReflectTrailProcUpdate`): the head's position less
    /// ten frames of its velocity, and no `rotate.z` write.
    pub fn trail_effect_pose(&self) -> (Vec3, Option<f32>) {
        if self.reflected {
            let pos = self.position - self.velocity * (5.0 * 2.0);
            return (Vec3::new(pos.x, pos.y, 0.0), None);
        }
        let index = (self.cursor + 12 - 10) % 12;
        let pos = self.history[index];
        let prev = self.history[(index + 11) % 12];
        (
            Vec3::new(pos.x, pos.y, 0.0),
            Some(sy_arc_tan2(pos.y - prev.y, pos.x - prev.x) - core::f32::consts::FRAC_PI_2),
        )
    }
    pub(super) fn tick<I, F>(
        &mut self,
        surfaces: F,
        owner: Option<OwnerView>,
        fx: &mut Emit,
    ) -> bool
    where
        F: Fn() -> I,
        I: IntoIterator<Item = MapSurface>,
    {
        // `wpProcessProcWeaponMain` plays the DObj animation first.
        self.anim_ticks = self.anim_ticks.wrapping_add(1);
        self.trail_spawn = self.lifetime == 158;
        self.lifetime = self.lifetime.saturating_sub(1);
        // `wpNessPKThunderHeadProcUpdate` tests `nWPNessPKThunderStatusCollide`
        // (Ness caught it) before the lifetime: an impact shock. Its other
        // ends make a small dust cloud.
        let collide = !self.reflected && owner.is_some_and(|o| o.ness_collide);
        let end = if collide {
            Fx::ImpactShock {
                pos: self.position,
                size: self.damage,
            }
        } else {
            Fx::DustExpandSmall(self.position)
        };
        if self.lifetime == 0 {
            fx.push(end);
            return false;
        }
        if !self.reflected {
            let Some(owner) = owner else {
                fx.push(end);
                return false;
            };
            if !owner.ness_control || owner.ness_motion != self.motion_count || owner.ness_collide {
                fx.push(end);
                return false;
            }
            self.cursor = (self.cursor + 1) % 12;
            self.history[(self.cursor + 11) % 12] =
                Vec2::new(self.position.x as i16 as f32, self.position.y as i16 as f32);
            if i32::from(owner.stick.x).abs() + i32::from(owner.stick.y).abs() > 45 {
                // `syVectorAngleDiff3D(vel_air, stick)`: `syUtilsArcCos` of
                // the normalised dot product, unsigned.
                let stick = Vec3::new(owner.stick.x as f32, owner.stick.y as f32, 0.0);
                let angle_diff = ssb_engine::math::sy_vector_angle_diff_3d(self.velocity, stick);
                // The side is the sign of `syVectorNormCross3D`'s z, which
                // normalising does not change; at 180 degrees the zero cross
                // product selects a negative step.
                let sign = if (self.velocity.x * stick.y - self.velocity.y * stick.x) > 0.0 {
                    1.0
                } else {
                    -1.0
                };
                let step = if angle_diff >= core::f32::consts::FRAC_PI_4 {
                    6.0 * core::f32::consts::PI / 180.0
                } else {
                    angle_diff / (45.0 / 6.0)
                };
                self.angle += step * sign;
                let (sin, cos) = sin_cos(self.angle);
                self.velocity = Vec3::new(cos * 60.0, sin * 60.0, 0.0);
            }
        }
        let from = self.position;
        self.position += self.velocity;
        let coll = BodyColl {
            top: 100.0,
            center: 0.0,
            bottom: -100.0,
            width: 100.0,
        };
        // `wpNessPKThunderHeadProcMap` and `wpNessPKReflectHeadProcMap`.
        if crate::wpmap::check_coll_end(&coll, from, &mut self.position, &surfaces) {
            fx.push(Fx::DustExpandSmall(self.position));
            return false;
        }
        true
    }
    /// The owner swap, then `wpNessPKThunderHeadProcReflector` (the first
    /// reflection) or `wpNessPKReflectHeadProcReflector` (a later one).
    pub(super) fn reflect(&mut self, r: &super::Reflector, group: u16) {
        let first = !self.reflected;
        self.reflected = true;
        self.owner_port = r.port;
        self.lifetime = 160;
        self.group = group;
        if first {
            // `wpNessPKReflectHeadMakeWeapon`: a new weapon, whose
            // `wpManagerMakeWeapon` adds the animation afresh, aimed away
            // from its owner (the reflector) and dealing the attributes'
            // damage: the old head's `proc_reflector` returns TRUE, so no
            // reflect bonus.
            self.anim_ticks = 0;
            let direction = self.position - (r.pos + Vec3::new(0.0, 250.0, 0.0));
            self.velocity = direction.normalized() * 60.0;
            self.velocity.z = 0.0;
            self.damage = 6;
        } else {
            // `wpMainReflectorSetLR`; the callback returns FALSE and the
            // bonus follows.
            if self.velocity.x * r.lr < 0.0 {
                self.velocity.x = -self.velocity.x;
            }
            self.damage = ((self.damage as f32 * 1.8 + 0.99) as i32).min(100);
        }
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
    /// `mobj->texture_id_curr`: the sprite the list draws, drawn each
    /// update by `syUtilsRandIntRange(WPPKTHUNDER_TEXTURES_NUM - 1)` in
    /// `flush_effects` (RE-550).
    pub texture: u8,
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
            texture: 0,
            // Reflected children inherit their parent's rotation minus 90°.
            // Keep the source's cumulative turns, including its janky offsets.
            rotation: if head.reflected {
                sy_arc_tan2(head.velocity.y, head.velocity.x)
                    - core::f32::consts::FRAC_PI_2 * (id as f32 + 1.0)
            } else {
                0.0
            },
            spawn_next: false,
        }
    }
    pub(super) fn tick(&mut self, head: PKThunder, fx: &mut Emit) {
        self.spawn_next = self.id < 3 && self.lifetime == 158;
        let make_effect = self.id == 3 && self.lifetime == 158;
        self.lifetime = self.lifetime.saturating_sub(1);
        if head.reflected {
            self.position = head.position - head.velocity * ((self.id as f32 + 1.5) * 2.0);
        } else {
            let index = (head.cursor + 12 - (self.id as usize + 1) * 2) % 12;
            let pos = head.history[index];
            let prev = head.history[(index + 11) % 12];
            self.position = Vec3::new(pos.x, pos.y, 0.0);
            self.rotation =
                sy_arc_tan2(pos.y - prev.y, pos.x - prev.x) - core::f32::consts::FRAC_PI_2;
        }
        // The last trail's `lifetime == WPPKTHUNDER_SPAWN_TRAIL_FRAME` makes
        // the trail effect, before `wpMainDecLifeCheckExpire` and the
        // frame's texture: the unreflected maker runs its process once; the
        // reflected one takes the head's `rotate.z` less 90 degrees first.
        if make_effect {
            let (pos, rotate_z) = head.trail_effect_pose();
            fx.push(Fx::PKTrailMake {
                head: head.group,
                reflected: head.reflected,
                pos,
                rotate_z: rotate_z
                    .unwrap_or_else(|| head.rotate_z() - core::f32::consts::FRAC_PI_2),
            });
        }
        // `wpNessPKThunderTrailProcUpdate`'s (and the reflected trail's)
        // frame: `syUtilsRandIntRange(WPPKTHUNDER_TEXTURES_NUM - 1)`.
        fx.push(Fx::TextureRand(3));
    }
    /// Root `rotate.z`: the direction from the previous trail sample, less
    /// 90 degrees.
    pub fn rotation(&self) -> f32 {
        self.rotation
    }
    /// `wpDisplayPKThunderProcDisplay`'s PRIM and ENV RGB for this trail,
    /// indexed by `trail_id` (0 to 3, the tables' length), both at alpha
    /// 0xFF.
    pub fn colors(&self) -> ([u8; 3], [u8; 3]) {
        let i = usize::from(self.id).min(PK_THUNDER_TRAIL_PRIM.len() - 1);
        (PK_THUNDER_TRAIL_PRIM[i], PK_THUNDER_TRAIL_ENV[i])
    }
    pub(crate) fn hit_position(&self) -> Vec3 {
        let (sin, cos) = sin_cos(self.rotation);
        self.position + Vec3::new(100.0 * sin, -100.0 * cos, 0.0)
    }
}
