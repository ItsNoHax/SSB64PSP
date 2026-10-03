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
use crate::monster_weapon::{ShotKind, ShotProc};
use crate::status::BlastZone;
use crate::wpeffect::{Emit, WeaponEffect as Fx};
#[path = "ness_weapon.rs"]
mod ness;
#[path = "pikachu_weapon.rs"]
mod pikachu;
pub use ness::{PKFire, PKThunder, PKThunderTrail};
pub use pikachu::{ThunderHead, ThunderJolt, ThunderTrail, JOLT_GROUND_ANIM_SPEED};
#[path = "sector_weapon.rs"]
pub mod sector;
pub use sector::ArwingLaser;

/// The one-sided role a map segment has in the original collision tables.
///
/// This is intentionally separate from the ROM pack's `line_kind`: weapons
/// are portable gameplay objects, while the PSP runtime merely adapts packed
/// lines into this input shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapSurfaceKind {
    Floor,
    Ceiling,
    /// `nMPLineKindRWall`: its normal points right (+X).
    RightWall,
    /// `nMPLineKindLWall`: its normal points left (-X).
    LeftWall,
}

/// One stage segment presented to a weapon map query.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MapSurface {
    pub kind: MapSurfaceKind,
    pub segment: Segment,
    /// Original vertex identity and polyline position for map-bound weapons.
    pub topology: Option<SurfaceTopology>,
    /// Current group translation and this tick's displacement. None is static.
    pub motion: Option<SurfaceMotion>,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct SurfaceMotion {
    pub offset: Vec2,
    pub speed: Vec3,
}

impl MapSurface {
    pub fn coords(self) -> [f32; 4] {
        let o = self.motion.map_or(Vec2::ZERO, |m| m.offset);
        [
            self.segment.x1 as f32 + o.x,
            self.segment.y1 as f32 + o.y,
            self.segment.x2 as f32 + o.x,
            self.segment.y2 as f32 + o.y,
        ]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SurfaceTopology {
    pub line: u16,
    pub point: u16,
    pub segments: u16,
    pub vertex1: u16,
    pub vertex2: u16,
}

/// The weapon families that a fighter status can request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeaponKind {
    /// `nWPKindFireball`, created by Mario's `SpecialN` accessory callback.
    MarioFireball,
    /// The same weapon with Luigi's attribute row (`fireball_item_id = 1`).
    LuigiFireball,
    /// `nWPKindBlaster`, created by Fox's neutral special.
    FoxBlaster,
    /// `nWPKindChargeShot` at the given charge level, already released.
    SamusChargeShot(u8),
    /// `nWPKindSamusBomb`, created by Samus's down special.
    SamusBomb,
    /// `nWPKindBoomerang`. The stick is read when the motion script's flag 0
    /// fires, not when the status starts.
    LinkBoomerang {
        is_smash: bool,
        stick_x: i8,
        stick_y: i8,
    },
    /// `nWPKindEggThrow` at its throw: `throw_force` and the stick are read
    /// at `SetFlag2(2)`. The spawn's `facing` is the egg's `lr`.
    YoshiEgg {
        throw_force: i16,
        stick_x: i8,
    },
    /// `wpYoshiStarMakeStars`: one `nWPKindYoshiStar` each way.
    YoshiStars,
    /// `nWPKindCutter`, Final Cutter's wave. `grounded`: Kirby stood on a
    /// floor line, which the wave then follows.
    KirbyCutter {
        grounded: bool,
    },
    NessPKFire {
        grounded: bool,
    },
    NessPKThunder,
    Equipment {
        kind: ShotKind,
        smash: bool,
        angle_index: u8,
    },
    PikachuThunderJolt,
    PikachuThunder,
}

/// One deferred weapon creation. The owner is identified by player port, the
/// stable match-local identity used by the portable fighter layer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WeaponSpawn {
    pub kind: WeaponKind,
    pub owner_port: u8,
    /// `wp->team`: the owner's team (`wpManagerMakeWeapon`).
    pub team: u8,
    pub position: Vec3,
    /// The owner's left/right direction at the motion-script event.
    pub facing: f32,
    /// The owner's staling when the weapon is made.
    pub stale: crate::stale::WeaponStale,
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

/// Luigi's row of `dWPMarioFireballWeaponAttributes` (US): no gravity, a
/// slower level shot and a shorter life. Its `WPAttributes` in
/// `222_LuigiSpecial1.c` differ from Mario's only in damage.
pub const LUIGI_FIREBALL_LIFETIME: u16 = 80;
pub const LUIGI_FIREBALL_SPEED: f32 = 36.0;
pub const LUIGI_FIREBALL_DAMAGE: i32 = 6;

/// The per-kind values `wpMarioFireball*` read through
/// `weapon_vars.fireball.index`. Both rows use the same angle on the ground
/// and in the air, so the spawn does not need the owner's situation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FireballAttributes {
    pub lifetime: u16,
    pub vel_terminal: f32,
    pub vel_min: f32,
    pub gravity: f32,
    pub rebound: f32,
    /// `rotate_speed`: added to the DObj's `rotate.x` on every update
    /// (`wpMarioFireballProcUpdate`), the spin its kind-0x47 matrix draws in
    /// the screen plane (RE-418).
    pub rotate_speed: f32,
    pub angle: f32,
    pub vel_base: f32,
    pub damage: i32,
}

/// `dWPMarioFireballWeaponAttributes`: index 0 is Mario, 1 is Luigi.
pub const FIREBALL_ATTRIBUTES: [FireballAttributes; 2] = [
    FireballAttributes {
        lifetime: MARIO_FIREBALL_LIFETIME,
        vel_terminal: MARIO_FIREBALL_TERMINAL_VELOCITY,
        vel_min: MARIO_FIREBALL_MIN_SPEED,
        gravity: MARIO_FIREBALL_GRAVITY,
        rebound: MARIO_FIREBALL_REBOUND,
        rotate_speed: 0.349_065_87, // F_CLC_DTOR32(20.0F)
        angle: MARIO_FIREBALL_ANGLE,
        vel_base: MARIO_FIREBALL_SPEED,
        damage: MARIO_FIREBALL_HITBOX.damage,
    },
    FireballAttributes {
        lifetime: LUIGI_FIREBALL_LIFETIME,
        vel_terminal: 55.0,
        vel_min: 30.0,
        gravity: 0.0,
        rebound: 0.85,
        rotate_speed: 0.436_332_3, // F_CLC_DTOR32(25.0F)
        angle: 0.0,
        vel_base: LUIGI_FIREBALL_SPEED,
        damage: LUIGI_FIREBALL_DAMAGE,
    },
];

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
    element: crate::combat::Element::Fire,
    shield_damage: 1,
};

/// Fox Special1's US `WPAttributes` and `WPBLASTER_VEL_X`.
pub const FOX_BLASTER_SPEED: f32 = 160.0;
pub const FOX_BLASTER_HITBOX: Hitbox = Hitbox {
    damage: 6,
    offset: Vec3::ZERO,
    // `wpManagerMakeWeapon` halves the attributes' size of 40.
    radius: 20.0,
    angle: 10,
    kb_scale: 100,
    kb_weight: 1,
    kb_base: 0,
    element: crate::combat::Element::Normal,
    shield_damage: 1,
};
pub const FOX_BLASTER_MAP_COLL: BodyColl = BodyColl {
    top: 10.0,
    center: 0.0,
    bottom: -10.0,
    width: 10.0,
};

/// Source `wpFoxBlaster`: horizontal velocity and a display scale that grows
/// by `16/3` per tick to `160/3` (the scale is presentation data only).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FoxBlaster {
    pub owner_port: u8,
    pub damage: i32,
    pub position: Vec3,
    pub velocity: Vec3,
    pub scale_x: f32,
}

impl FoxBlaster {
    fn new(spawn: WeaponSpawn) -> Self {
        Self {
            owner_port: spawn.owner_port,
            damage: FOX_BLASTER_HITBOX.damage,
            position: spawn.position,
            velocity: Vec3::new(spawn.facing * FOX_BLASTER_SPEED, 0.0, 0.0),
            scale_x: 1.0,
        }
    }

    fn tick<I, F>(&mut self, surfaces: F, fx: &mut Emit) -> bool
    where
        F: Fn() -> I,
        I: IntoIterator<Item = MapSurface>,
    {
        self.scale_x = (self.scale_x + 16.0 / 3.0).min(160.0 / 3.0);
        let wanted = self.position + self.velocity;
        if let Some(hit) = map_contact(surfaces(), self.position, wanted, FOX_BLASTER_MAP_COLL) {
            // `wpFoxBlasterProcMap`.
            fx.push(Fx::FoxBlasterGlow(hit.position));
            return false;
        }
        self.position = wanted;
        true
    }
}

/// `WPAttributes::priority` of every ported weapon (`wpManagerMakeWeapon`),
/// read from each `ll*WeaponAttributes` record
/// (`crates/ssb-rom/tests/weapon_attributes.rs`).
pub const WEAPON_PRIORITY: i32 = 1;

/// `dWPSamusChargeShotWeaponAttributes[].priority` (US), which
/// `wpSamusChargeShotLaunch` writes: only the full charge outranks.
pub const SAMUS_CHARGE_SHOT_PRIORITIES: [i32; 8] = [1, 1, 1, 1, 1, 1, 1, 2];

/// `dWPSamusChargeShotWeaponAttributes` (US): `(gfx size, X velocity,
/// damage, attack size, map-collision size)` per charge level.
pub const SAMUS_CHARGE_SHOT_LEVELS: [(f32, f32, i32, f32, f32); 8] = [
    (150.0, 60.0, 3, 100.0, 10.0),
    (230.0, 62.0, 6, 120.0, 10.0),
    (280.0, 64.0, 9, 140.0, 10.0),
    (340.0, 66.0, 12, 160.0, 10.0),
    (410.0, 68.0, 15, 180.0, 10.0),
    (490.0, 70.0, 18, 200.0, 10.0),
    (600.0, 72.0, 21, 240.0, 10.0),
    (700.0, 74.0, 26, 260.0, 10.0),
];
/// `WPCHARGESHOT_GFX_SIZE_DIV`.
pub const SAMUS_CHARGE_SHOT_GFX_SIZE_DIV: f32 = 30.0;

/// `dSamusSpecial1_ChargeShot_WeaponAttributes`: the knockback that
/// `wpSamusChargeShotLaunch` keeps while it replaces damage and size.
pub const SAMUS_CHARGE_SHOT_HITBOX: Hitbox = Hitbox {
    damage: 0,
    offset: Vec3::ZERO,
    radius: 0.0,
    angle: 361,
    kb_scale: 100,
    kb_weight: 0,
    kb_base: 0,
    element: crate::combat::Element::Electric,
    shield_damage: 1,
};

/// A charge level's sprite scale, `gfx_size / WPCHARGESHOT_GFX_SIZE_DIV`.
/// The charging shot takes it from the current level every update.
pub fn samus_charge_shot_scale(charge: u8) -> f32 {
    SAMUS_CHARGE_SHOT_LEVELS[usize::from(charge.min(7))].0 / SAMUS_CHARGE_SHOT_GFX_SIZE_DIV
}

/// `WPCHARGESHOT_ROTATE_SPEED`: the shot's in-plane spin per update.
pub const SAMUS_CHARGE_SHOT_ROTATE_SPEED: f32 = 18.0 * core::f32::consts::PI / 180.0;

/// Source `wpSamusChargeShot` after release: a straight shot that ends on
/// any map contact (`wpMapTestAllCheckCollEnd`) or registered hit.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SamusChargeShot {
    pub owner_port: u8,
    pub charge: u8,
    pub damage: i32,
    pub position: Vec3,
    pub velocity: Vec3,
    /// `rotate.z`, presentation only.
    pub rotate_z: f32,
}

impl SamusChargeShot {
    fn new(spawn: WeaponSpawn, charge: u8) -> Self {
        let level = SAMUS_CHARGE_SHOT_LEVELS[usize::from(charge.min(7))];
        Self {
            owner_port: spawn.owner_port,
            charge: charge.min(7),
            damage: level.2,
            position: spawn.position,
            velocity: Vec3::new(level.1 * spawn.facing, 0.0, 0.0),
            rotate_z: 0.0,
        }
    }

    pub fn hitbox(&self) -> Hitbox {
        let level = SAMUS_CHARGE_SHOT_LEVELS[usize::from(self.charge)];
        Hitbox {
            damage: self.damage,
            radius: level.3 * 0.5,
            ..SAMUS_CHARGE_SHOT_HITBOX
        }
    }

    /// Display scale of the sprite, `gfx_size / 30`.
    pub fn scale(&self) -> f32 {
        samus_charge_shot_scale(self.charge)
    }

    fn tick<I, F>(&mut self, surfaces: F, fx: &mut Emit) -> bool
    where
        F: Fn() -> I,
        I: IntoIterator<Item = MapSurface>,
    {
        let lr = if self.velocity.x < 0.0 { -1.0 } else { 1.0 };
        self.rotate_z -= SAMUS_CHARGE_SHOT_ROTATE_SPEED * lr;
        let half = SAMUS_CHARGE_SHOT_LEVELS[usize::from(self.charge)].4 * 0.5;
        let coll = BodyColl {
            top: half,
            center: 0.0,
            bottom: -half,
            width: half,
        };
        let wanted = self.position + self.velocity;
        if let Some(hit) = map_contact(surfaces(), self.position, wanted, coll) {
            // `wpSamusChargeShotProcMap`.
            fx.push(Fx::DustExpandSmall(hit.position));
            return false;
        }
        self.position = wanted;
        true
    }
}

/// `wpvars.h` Bomb constants.
pub const SAMUS_BOMB_WAIT_LIFETIME: u16 = 100;
pub const SAMUS_BOMB_EXPLODE_LIFETIME: u16 = 6;
pub const SAMUS_BOMB_EXPLODE_SIZE: f32 = 180.0;
pub const SAMUS_BOMB_WAIT_VEL_Y: f32 = 10.0;
pub const SAMUS_BOMB_WAIT_GRAVITY: f32 = 1.0;
pub const SAMUS_BOMB_WAIT_TVEL: f32 = 50.0;
pub const SAMUS_BOMB_WAIT_COLLIDE_MOD_VEL: f32 = 0.9;
pub const SAMUS_BOMB_FLOOR_MOD_VEL: f32 = 0.6;
pub const SAMUS_BOMB_GROUND_MIN_SPEED: f32 = 8.0;
pub const SAMUS_BOMB_WAIT_ROTATE_SPEED_AIR: f32 = 20.0 * core::f32::consts::PI / 180.0;
pub const SAMUS_BOMB_WAIT_ROTATE_SPEED_GROUND: f32 = 10.0 * core::f32::consts::PI / 180.0;

/// `llSamusMainBombWeaponAttributes` in `217_SamusMain.c` (the words at file
/// offset 0x0C onward): size 160, angle 361, knockback 65/0/10, 9 damage.
pub const SAMUS_BOMB_HITBOX: Hitbox = Hitbox {
    damage: 9,
    offset: Vec3::ZERO,
    radius: 80.0,
    angle: 361,
    kb_scale: 65,
    kb_weight: 0,
    kb_base: 10,
    element: crate::combat::Element::Fire,
    shield_damage: 1,
};
pub const SAMUS_BOMB_MAP_COLL: BodyColl = BodyColl {
    top: 75.0,
    center: 0.0,
    bottom: -75.0,
    width: 75.0,
};

/// Source `wpSamusBomb`. It bounces, settles and explodes after 100 frames
/// or on its first registered hit; the explosion lives six more frames and
/// keeps the hit record, so a fighter it already hit is not hit again.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SamusBomb {
    pub owner_port: u8,
    pub position: Vec3,
    pub velocity: Vec3,
    pub lifetime: u16,
    pub exploded: bool,
    /// `lr`, set from the launch and after every rebound.
    pub lr: f32,
    /// The floor line while grounded, with `vel_ground`.
    pub floor: Option<(MapSurface, f32)>,
    /// Ports already in the attack record.
    pub hit_ports: u8,
    /// `bomb_blink_timer` and the current palette, presentation only.
    pub blink_timer: u16,
    pub blink_palette: u8,
    /// `rotate.z`, presentation only.
    pub rotate_z: f32,
}

impl SamusBomb {
    fn new(spawn: WeaponSpawn) -> Self {
        Self {
            owner_port: spawn.owner_port,
            position: spawn.position,
            velocity: Vec3::new(0.0, SAMUS_BOMB_WAIT_VEL_Y, 0.0),
            lifetime: SAMUS_BOMB_WAIT_LIFETIME,
            exploded: false,
            lr: spawn.facing,
            floor: None,
            hit_ports: 0,
            blink_timer: 8,
            blink_palette: 0,
            rotate_z: 0.0,
        }
    }

    pub fn hitbox(&self) -> Hitbox {
        Hitbox {
            radius: if self.exploded {
                SAMUS_BOMB_EXPLODE_SIZE
            } else {
                SAMUS_BOMB_HITBOX.radius
            },
            ..SAMUS_BOMB_HITBOX
        }
    }

    /// `wpSamusBombExplodeInitVars`.
    fn explode(&mut self) {
        self.exploded = true;
        self.lifetime = SAMUS_BOMB_EXPLODE_LIFETIME;
        self.velocity = Vec3::ZERO;
        self.floor = None;
    }

    fn set_lr(&mut self) {
        self.lr = if self.velocity.x < 0.0 { -1.0 } else { 1.0 };
    }

    /// `wpSamusBombProcUpdate` (or the explosion's) then `wpSamusBombProcMap`.
    fn tick<I, F>(&mut self, surfaces: F, fx: &mut Emit) -> bool
    where
        F: Fn() -> I,
        I: IntoIterator<Item = MapSurface>,
    {
        self.lifetime -= 1;
        if self.exploded {
            return self.lifetime != 0;
        }
        if self.lifetime == 0 {
            fx.push(Fx::SparkleWhiteMultiExplode(self.position));
            self.explode();
            return true;
        }
        self.floor = self
            .floor
            .and_then(|(old, v)| refresh_surface(surfaces(), old).map(|s| (s, v)));
        match self.floor {
            None => {
                self.velocity.y -= SAMUS_BOMB_WAIT_GRAVITY;
                let speed = Vec2::new(self.velocity.x, self.velocity.y).length();
                if speed > SAMUS_BOMB_WAIT_TVEL {
                    let scale = SAMUS_BOMB_WAIT_TVEL / speed;
                    self.velocity.x *= scale;
                    self.velocity.y *= scale;
                }
                self.rotate_z -= SAMUS_BOMB_WAIT_ROTATE_SPEED_AIR * self.lr;
            }
            Some((segment, vel_ground)) => {
                // `wpMainVelGroundTransferAir` along the floor line.
                let normal = surface_normal(MapSurfaceKind::Floor, segment.segment);
                self.velocity.x = self.lr * normal.y * vel_ground;
                self.velocity.y = self.lr * -normal.x * vel_ground;
                self.rotate_z -= SAMUS_BOMB_WAIT_ROTATE_SPEED_GROUND * self.lr;
            }
        }
        self.blink_timer -= 1;
        if self.blink_timer == 0 {
            self.blink_palette ^= 1;
            self.blink_timer = if self.lifetime > 40 {
                8
            } else if self.lifetime > 20 {
                5
            } else {
                3
            };
        }

        let mut wanted = self.position + self.velocity;
        if let Some((segment, _)) = self.floor {
            // `wpMapTestLRWallCheckFloor`: slide along the line until it ends.
            wanted += segment.motion.map_or(Vec3::ZERO, |m| m.speed);
            let [x1, _, x2, _] = segment.coords();
            let (lo, hi) = (x1.min(x2), x1.max(x2));
            if wanted.x < lo || wanted.x > hi {
                self.floor = None;
                self.position = wanted;
            } else {
                let y = segment_y(segment, wanted.x) - SAMUS_BOMB_MAP_COLL.bottom;
                self.position = Vec3::new(wanted.x, y, wanted.z);
            }
            return true;
        }
        match map_contact(surfaces(), self.position, wanted, SAMUS_BOMB_MAP_COLL) {
            Some(hit) => {
                self.position = hit.position;
                let dot = self.velocity.x * hit.normal.x + self.velocity.y * hit.normal.y;
                self.velocity.x -= 2.0 * dot * hit.normal.x;
                self.velocity.y -= 2.0 * dot * hit.normal.y;
                if hit.kind == MapSurfaceKind::Floor {
                    self.velocity.x *= SAMUS_BOMB_FLOOR_MOD_VEL;
                    self.velocity.y *= SAMUS_BOMB_FLOOR_MOD_VEL;
                    self.set_lr();
                    let speed = Vec2::new(self.velocity.x, self.velocity.y).length();
                    if speed < SAMUS_BOMB_GROUND_MIN_SPEED {
                        // `wpMapSetGround`.
                        self.floor = Some((hit.surface, self.velocity.x * self.lr));
                    }
                } else {
                    self.velocity.x *= SAMUS_BOMB_WAIT_COLLIDE_MOD_VEL;
                    self.velocity.y *= SAMUS_BOMB_WAIT_COLLIDE_MOD_VEL;
                    self.set_lr();
                }
            }
            None => self.position = wanted,
        }
        true
    }
}

fn refresh_surface(
    surfaces: impl IntoIterator<Item = MapSurface>,
    old: MapSurface,
) -> Option<MapSurface> {
    surfaces
        .into_iter()
        .find(|s| match (s.topology, old.topology) {
            (Some(a), Some(b)) => a.line == b.line && a.point == b.point,
            _ => s.kind == old.kind && s.segment == old.segment,
        })
}

fn segment_y(s: MapSurface, x: f32) -> f32 {
    let [x1, y1, x2, y2] = s.coords();
    if x1 == x2 {
        return y1;
    }
    y1 + (x - x1) * (y2 - y1) / (x2 - x1)
}

/// `wpvars.h` Boomerang constants.
pub const BOOMERANG_OFF_X: f32 = 150.0;
pub const BOOMERANG_OFF_Y: f32 = 290.0;
pub const BOOMERANG_HOMING_ANGLE_MAX: f32 = 1.5 * core::f32::consts::PI / 180.0;
pub const BOOMERANG_HOMING_ANGLE_MIN: f32 = 0.75 * core::f32::consts::PI / 180.0;
pub const BOOMERANG_VEL_SMASH: f32 = 114.0;
pub const BOOMERANG_VEL_TILT: f32 = 85.0;
pub const BOOMERANG_RETURN_DAMAGE: i32 = 8;
pub const BOOMERANG_ANGLE_STICK_THRESHOLD: i32 = 10;
pub const BOOMERANG_LIFETIME_SMASH: u16 = 190;
pub const BOOMERANG_LIFETIME_TILT: u16 = 160;
pub const BOOMERANG_LIFETIME_REFLECT: u16 = 100;
/// `wpLinkBoomerangMakeWeapon`'s `homing_delay`.
pub const BOOMERANG_HOMING_DELAY: u8 = 130;
/// `wpLinkBoomerangCheckOwnerCatch`: catch distance from the owner's TopN
/// plus 290 up.
pub const BOOMERANG_CATCH_DIST: f32 = 180.0;

/// `dLinkSpecial1_Boomerang_WeaponAttributes` (`226_LinkSpecial1.c`, US):
/// size 200, angle 70, knockback 30/0/55, 9 damage.
pub const LINK_BOOMERANG_HITBOX: Hitbox = Hitbox {
    damage: 9,
    offset: Vec3::ZERO,
    radius: 100.0,
    angle: 70,
    kb_scale: 30,
    kb_weight: 0,
    kb_base: 55,
    element: crate::combat::Element::Normal,
    shield_damage: 1,
};
pub const LINK_BOOMERANG_MAP_COLL: BodyColl = BodyColl {
    top: 150.0,
    center: 0.0,
    bottom: -150.0,
    width: 150.0,
};

const DEG_30: f32 = core::f32::consts::PI / 6.0;
const DEG_90: f32 = core::f32::consts::FRAC_PI_2;
const DEG_180: f32 = core::f32::consts::PI;
const DEG_270: f32 = 3.0 * core::f32::consts::FRAC_PI_2;
const DEG_360: f32 = 2.0 * core::f32::consts::PI;

/// `wpLinkBoomerangClampAngle360`: one wrap only, as the source does.
fn clamp_angle_360(angle: f32) -> f32 {
    if angle > DEG_360 {
        angle - DEG_360
    } else if angle < -DEG_360 {
        angle + DEG_360
    } else {
        angle
    }
}

/// What the pool needs to know about a Boomerang's owner this frame: its
/// TopN and `FTStruct::is_special_interrupt`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OwnerView {
    pub position: Vec3,
    pub is_special_interrupt: bool,
    pub thunder_collide: bool,
    pub thunder_damage: bool,
    pub thunder_motion: u16,
    pub ness_control: bool,
    pub ness_collide: bool,
    pub ness_motion: u16,
    pub stick: crate::status::StickState,
}

/// Source `wpLinkBoomerang`. It flies out and slows down, then turns back
/// and homes on its owner, who catches it within 180 units.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LinkBoomerang {
    /// `owner_gobj`: the attacker, which a reflector changes.
    pub owner_port: u8,
    /// `weapon_vars.boomerang.parent_gobj`: the thrower it homes on.
    pub parent_port: Option<u8>,
    pub position: Vec3,
    pub velocity: Vec3,
    pub lifetime: u16,
    pub is_return: bool,
    pub is_reflect: bool,
    /// `WPLINK_BOOMERANG_FLAG_FORWARD`: bounds the homing by
    /// `flyforward_timer` once it returns.
    pub is_forward: bool,
    pub flyforward_timer: u8,
    pub default_angle: f32,
    pub homing_angle: f32,
    pub damage: i32,
    pub lr: f32,
    /// Ports in the attack record. `can_rehit_fighter` is clear.
    pub hit_ports: u8,
    /// `weapon_vars.boomerang.homing_delay`: frames before
    /// [`Self::check_off_camera`] starts sampling the camera.
    pub homing_delay: u8,
    /// `weapon_vars.boomerang.adjust_angle_delay`: the camera is sampled
    /// every ninth frame.
    pub adjust_angle_delay: u8,
    /// The root DObj's `rotate.y`, which `wpMainVelSetModelPitch` sets once
    /// from the launch velocity: +90 degrees toward +X, -90 otherwise.
    /// Presentation only.
    pub model_rotate_y: f32,
    /// `wpLinkBoomerangSetReturnVars` sets `DOBJ_FLAG_NOTEXTURE` on the
    /// root's grandchild, which stays hidden for the rest of the flight.
    /// Presentation only.
    pub grandchild_hidden: bool,
    /// `gcPlayAnimAll` calls so far: one per update. Presentation only.
    pub anim_ticks: u16,
}

impl LinkBoomerang {
    /// `wpLinkBoomerangMakeWeapon`.
    fn new(spawn: WeaponSpawn, is_smash: bool, stick_x: i8, stick_y: i8) -> Self {
        let lr = if spawn.facing < 0.0 { -1.0 } else { 1.0 };
        let (lifetime, speed) = if is_smash {
            (BOOMERANG_LIFETIME_SMASH, BOOMERANG_VEL_SMASH)
        } else {
            (BOOMERANG_LIFETIME_TILT, BOOMERANG_VEL_TILT)
        };
        let default_angle = Self::launch_angle(stick_x, stick_y, lr);
        // `wpLinkBoomerangGetAngleSetVel` scales both components by `lr`;
        // only the speed survives the first update, which rebuilds the
        // velocity from `default_angle`.
        let (sin, cos) = sin_cos(default_angle);
        LinkBoomerang {
            owner_port: spawn.owner_port,
            parent_port: Some(spawn.owner_port),
            position: spawn.position + Vec3::new(BOOMERANG_OFF_X * lr, BOOMERANG_OFF_Y, 0.0),
            velocity: Vec3::new(cos * speed, sin * speed, 0.0),
            lifetime,
            is_return: false,
            is_reflect: false,
            is_forward: true,
            flyforward_timer: 0,
            default_angle,
            homing_angle: 0.0,
            damage: LINK_BOOMERANG_HITBOX.damage,
            lr,
            hit_ports: 0,
            homing_delay: BOOMERANG_HOMING_DELAY,
            adjust_angle_delay: 0,
            // The launch velocity's X is `cos(angle) * speed * lr`, and the
            // angle is within 30 degrees of level, so its sign is `lr`.
            model_rotate_y: DEG_90 * lr,
            grandchild_hidden: false,
            anim_ticks: 0,
        }
    }

    /// `wpLinkBoomerangCheckOffCamera`: after its homing delay, every ninth
    /// frame, whether the camera sees it more than 40 pixels outside the
    /// viewport. Without a camera the delays still run.
    fn check_off_camera(&mut self, camera: Option<&crate::camera::Camera>) -> bool {
        if self.homing_delay > 0 {
            self.homing_delay -= 1;
            return false;
        }
        self.adjust_angle_delay += 1;
        if self.adjust_angle_delay <= 8 {
            return false;
        }
        self.adjust_angle_delay = 0;
        let Some(camera) = camera else { return false };
        let (x, y) = camera.project(self.position);
        let bound_x = crate::camera::BATTLE_VIEWPORT_WIDTH / 2.0 + 40.0;
        let bound_y = crate::camera::BATTLE_VIEWPORT_HEIGHT / 2.0 + 40.0;
        x < -bound_x || x > bound_x || y < -bound_y || y > bound_y
    }

    /// The angle half of `wpLinkBoomerangGetAngleSetVel`: up to 30 degrees
    /// up or down once the stick passes 10, mirrored for a left throw.
    pub fn launch_angle(stick_x: i8, stick_y: i8, lr: f32) -> f32 {
        let y = i32::from(stick_y);
        let mut angle = if y.abs() > BOOMERANG_ANGLE_STICK_THRESHOLD {
            ssb_engine::math::atan2(y as f32, i32::from(stick_x).abs() as f32)
                .clamp(-DEG_30, DEG_30)
        } else {
            0.0
        };
        if lr < 0.0 {
            angle = if angle < 0.0 {
                -DEG_180 - angle
            } else {
                DEG_180 - angle
            };
        }
        if angle < 0.0 {
            angle += DEG_360;
        }
        angle
    }

    fn speed(&self) -> f32 {
        Vec2::new(self.velocity.x, self.velocity.y).length()
    }

    /// `wpLinkBoomerangUpdateVelLR`.
    fn set_speed(&mut self, speed: f32) {
        let (sin, cos) = sin_cos(self.default_angle);
        self.velocity.x = cos * speed;
        self.velocity.y = sin * speed;
        self.lr = if self.default_angle > DEG_90 && self.default_angle < DEG_270 {
            -1.0
        } else {
            1.0
        };
    }

    /// `wpLinkBoomerangSetReturnVars`.
    fn set_return(&mut self, homing_max: bool) {
        self.is_return = true;
        self.grandchild_hidden = true;
        self.damage = BOOMERANG_RETURN_DAMAGE;
        self.default_angle -= DEG_180;
        if self.default_angle < 0.0 {
            self.default_angle += DEG_360;
        }
        self.lr = -self.lr;
        self.flyforward_timer = 140;
        self.homing_angle = if homing_max {
            BOOMERANG_HOMING_ANGLE_MAX
        } else {
            BOOMERANG_HOMING_ANGLE_MIN
        };
    }

    /// `wpLinkBoomerangGetDistUpdateAngle`: turns toward the parent's TopN
    /// plus 290 by at most `homing_angle` a frame, and returns the distance.
    fn home(&mut self, parent: Vec3) -> f32 {
        let dist_x = parent.x - self.position.x;
        let dist_y = parent.y - self.position.y + BOOMERANG_OFF_Y;
        let dist = Vec2::new(dist_x, dist_y).length();
        if self.is_forward {
            if self.flyforward_timer > 0 {
                self.flyforward_timer -= 1;
            } else {
                return dist;
            }
        }
        let mut angle = ssb_engine::math::atan2(dist_y, dist_x);
        if angle < -DEG_180 {
            angle += DEG_360;
        } else if angle > DEG_180 {
            angle -= DEG_360;
        }
        angle -= self.default_angle;
        if angle < -DEG_180 {
            angle += DEG_360;
        } else if angle > DEG_180 {
            angle -= DEG_360;
        }
        angle = angle.clamp(-self.homing_angle, self.homing_angle);
        self.default_angle = clamp_angle_360(self.default_angle + angle);
        dist
    }

    /// `wpLinkBoomerangCheckBound`. The source's similarity divides the dot
    /// product by the *sum* of the two magnitudes, so the threshold is a
    /// normal speed of about half the boomerang's speed, not 30 degrees.
    fn bound(&mut self, normal: Vec2) -> bool {
        let dot = self.velocity.x * normal.x + self.velocity.y * normal.y;
        let sim = dot / (normal.length() + self.speed());
        if sim < 0.0 {
            if sim > -ssb_engine::math::sin_cos(DEG_30).0 {
                self.velocity.x -= 2.0 * dot * normal.x;
                self.velocity.y -= 2.0 * dot * normal.y;
                self.default_angle =
                    clamp_angle_360(ssb_engine::math::atan2(self.velocity.y, self.velocity.x));
            } else {
                return true;
            }
        }
        false
    }

    /// `wpLinkBoomerangProcUpdate`, the manager's move, then
    /// `wpLinkBoomerangProcMap`. Returns `(alive, caught)`, where `caught`
    /// reports a catch by a parent whose `is_special_interrupt` is set.
    fn tick<I, F>(
        &mut self,
        surfaces: F,
        parent: Option<OwnerView>,
        camera: Option<&crate::camera::Camera>,
        fx: &mut Emit,
    ) -> (bool, bool)
    where
        F: Fn() -> I,
        I: IntoIterator<Item = MapSurface>,
    {
        // `wpProcessProcWeaponMain` plays the DObj animation before
        // `proc_update`.
        self.anim_ticks = self.anim_ticks.wrapping_add(1);
        self.lifetime -= 1;
        if self.lifetime == 0 || self.check_off_camera(camera) {
            return (false, false);
        }
        if self.is_reflect {
            // Flies straight.
        } else if self.is_return {
            self.set_speed((self.speed() + 1.0).min(90.0));
            if let Some(parent) = parent {
                let dist = self.home(parent.position);
                if dist < BOOMERANG_CATCH_DIST {
                    return (false, parent.is_special_interrupt);
                }
            }
        } else {
            let speed = (self.speed() - 1.4).max(10.0);
            self.set_speed(speed);
            if speed == 10.0 {
                self.set_return(false);
            }
        }
        let wanted = self.position + self.velocity;
        if self.is_reflect || self.is_return {
            self.position = wanted;
            return (true, false);
        }
        match map_contact(surfaces(), self.position, wanted, LINK_BOOMERANG_MAP_COLL) {
            Some(hit) => {
                self.position = hit.position;
                // `wpLinkBoomerangProcMap`: a newly touched surface.
                fx.push(Fx::DustCollide(hit.position));
                if self.bound(hit.normal) {
                    self.set_return(true);
                }
            }
            None => self.position = wanted,
        }
        (true, false)
    }

    /// `wpLinkBoomerangProcHit` after a registered hit.
    fn on_hit(&mut self) {
        if !self.is_reflect && !self.is_return {
            let speed = (self.speed() - 5.0).max(10.0);
            self.set_speed(speed);
            self.set_return(true);
        }
    }

    /// `wpLinkBoomerangProcSetOff`: an attack that beat it sends it back.
    fn set_off(&mut self) {
        if !self.is_reflect && !self.is_return {
            self.set_return(true);
        }
    }

    /// `wpLinkBoomerangProcHop`: it turns its flight angle rather than its
    /// velocity.
    fn hop(&mut self, angle: f32, dir_z: f32) {
        if dir_z > 0.0 {
            self.default_angle += angle * 2.0;
        } else {
            self.default_angle -= angle * 2.0;
        }
        self.default_angle = clamp_angle_360(self.default_angle);
    }

    /// `wpLinkBoomerangProcReflector`, then the reflect damage bonus.
    fn reflect(&mut self, reflector: &Fighter) {
        if !self.is_reflect {
            self.is_reflect = true;
            self.is_return = false;
            self.is_forward = false;
            self.lifetime = BOOMERANG_LIFETIME_REFLECT;
        }
        let dist_x = self.position.x - reflector.pos.x;
        let dist_y = self.position.y - (reflector.pos.y + 250.0);
        self.default_angle = clamp_angle_360(ssb_engine::math::atan2(dist_y, dist_x));
        self.set_speed(self.speed());
        self.owner_port = reflector.port;
        self.damage = ((self.damage as f32 * 1.8 + 0.99) as i32).min(100);
    }

    pub fn hitbox(&self) -> Hitbox {
        Hitbox {
            damage: self.damage,
            ..LINK_BOOMERANG_HITBOX
        }
    }
}

/// `wpvars.h` Egg Throw constants.
pub const EGGTHROW_LIFETIME: u16 = 50;
pub const EGGTHROW_EXPLODE_LIFETIME: u16 = 10;
pub const EGGTHROW_EXPLODE_SIZE: f32 = 340.0;
pub const EGGTHROW_TRAJECTORY_DIV: f32 = 65.0;
pub const EGGTHROW_TRAJECTORY_SUB_FORWARD: f32 = 73.0 * core::f32::consts::PI / 180.0;
pub const EGGTHROW_TRAJECTORY_SUB_BEHIND: f32 = 107.0 * core::f32::consts::PI / 180.0;
pub const EGGTHROW_ANGLE_MUL: f32 = 20.0 * core::f32::consts::PI / 180.0;
pub const EGGTHROW_ANGLE_CLAMP: f32 = 6.0 * core::f32::consts::PI / 180.0;
pub const EGGTHROW_VEL_ADD: f32 = 50.0;
pub const EGGTHROW_VEL_FORCE_MUL: f32 = 2.3;
pub const EGGTHROW_GRAVITY: f32 = 2.7;
pub const EGGTHROW_TVEL: f32 = 120.0;
/// `WPEGGTHROW_ANGLE_FORCE_MUL` and `WPEGGTHROW_ANGLE_ADD`, in degrees: the
/// thrown egg's per-frame `rotate.z` step.
pub const EGGTHROW_ANGLE_FORCE_MUL: f32 = -2.1;
pub const EGGTHROW_ANGLE_ADD: f32 = -1.5;

/// `llYoshiMainEggThrowWeaponAttributes` (the words at offset 0x0C of
/// `247_YoshiMain.c`, US): size 200, angle 361, knockback 50/0/50, 14
/// damage.
pub const YOSHI_EGG_HITBOX: Hitbox = Hitbox {
    damage: 14,
    offset: Vec3::ZERO,
    radius: 100.0,
    angle: 361,
    kb_scale: 50,
    kb_weight: 0,
    kb_base: 50,
    element: crate::combat::Element::Normal,
    shield_damage: 6,
};
pub const YOSHI_EGG_MAP_COLL: BodyColl = BodyColl {
    top: 150.0,
    center: 0.0,
    bottom: -150.0,
    width: 150.0,
};

/// Source `wpYoshiEggThrow` from its throw. It arcs, and explodes on any map
/// contact, on its first registered hit or after 50 frames. The explosion
/// keeps the attack record (`can_rehit_fighter` is clear).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct YoshiEgg {
    pub owner_port: u8,
    pub position: Vec3,
    pub velocity: Vec3,
    pub lifetime: u16,
    pub damage: i32,
    /// `weapon_vars.egg_throw.is_spin`: the throw's velocity is set.
    pub is_spin: bool,
    pub exploded: bool,
    pub throw_force: i16,
    pub stick_x: i8,
    pub lr: f32,
    pub hit_ports: u8,
    /// The DObj's `rotate.z`, presentation only. `wpYoshiEggThrowProcUpdate`
    /// adds [`Self::spin_step`] on every flight frame after the throw's.
    pub rotate_z: f32,
    /// `weapon_vars.egg_throw.angle`, radians, set at the throw.
    pub spin_step: f32,
}

impl YoshiEgg {
    fn new(spawn: WeaponSpawn, throw_force: i16, stick_x: i8) -> Self {
        YoshiEgg {
            owner_port: spawn.owner_port,
            position: spawn.position,
            velocity: Vec3::ZERO,
            lifetime: EGGTHROW_LIFETIME,
            damage: YOSHI_EGG_HITBOX.damage,
            is_spin: false,
            exploded: false,
            throw_force,
            stick_x,
            lr: if spawn.facing < 0.0 { -1.0 } else { 1.0 },
            hit_ports: 0,
            rotate_z: 0.0,
            spin_step: 0.0,
        }
    }

    /// The trajectory half of `wpYoshiEggThrowInitVars`.
    pub fn launch_velocity(throw_force: i16, stick_x: i8, lr: f32) -> Vec3 {
        let stick = i32::from(stick_x);
        let mut angle =
            (stick.abs() as f32 / EGGTHROW_TRAJECTORY_DIV).min(1.0) * EGGTHROW_ANGLE_MUL;
        if angle < EGGTHROW_ANGLE_CLAMP {
            angle = 0.0;
        }
        if stick < 0 {
            angle = -angle;
        }
        let angle = if lr > 0.0 {
            EGGTHROW_TRAJECTORY_SUB_FORWARD - angle
        } else {
            EGGTHROW_TRAJECTORY_SUB_BEHIND - angle
        };
        let speed = f32::from(throw_force) * EGGTHROW_VEL_FORCE_MUL + EGGTHROW_VEL_ADD;
        let (sin, cos) = sin_cos(angle);
        Vec3::new(cos * speed, sin * speed, 0.0)
    }

    pub fn hitbox(&self) -> Hitbox {
        Hitbox {
            damage: self.damage,
            radius: if self.exploded {
                EGGTHROW_EXPLODE_SIZE * 0.5
            } else {
                YOSHI_EGG_HITBOX.radius
            },
            ..YOSHI_EGG_HITBOX
        }
    }

    /// `wpYoshiEggHitInitVars` / `wpYoshiEggExpireInitVars`.
    fn explode(&mut self) {
        self.exploded = true;
        self.lifetime = EGGTHROW_EXPLODE_LIFETIME;
        self.velocity = Vec3::ZERO;
    }

    /// `wpYoshiEggThrowProcUpdate` (or the explosion's), the manager's move,
    /// then `wpYoshiEggThrowProcMap`.
    fn tick<I, F>(&mut self, surfaces: F, fx: &mut Emit) -> bool
    where
        F: Fn() -> I,
        I: IntoIterator<Item = MapSurface>,
    {
        if self.exploded {
            self.lifetime -= 1;
            return self.lifetime != 0;
        }
        if self.is_spin {
            self.lifetime -= 1;
            if self.lifetime == 0 {
                // `wpYoshiEggThrowProcUpdate`, then `wpYoshiEggExpireInitVars`.
                fx.push(Fx::YoshiEggExplode(self.position));
                fx.push(Fx::EggBreak(self.position));
                self.explode();
                return true;
            }
            self.rotate_z += self.spin_step;
            self.velocity.y -= EGGTHROW_GRAVITY;
            let speed = Vec2::new(self.velocity.x, self.velocity.y).length();
            if speed > EGGTHROW_TVEL {
                self.velocity.x *= EGGTHROW_TVEL / speed;
                self.velocity.y *= EGGTHROW_TVEL / speed;
            }
        } else {
            self.is_spin = true;
            self.velocity = Self::launch_velocity(self.throw_force, self.stick_x, self.lr);
            self.spin_step = (f32::from(self.throw_force) * EGGTHROW_ANGLE_FORCE_MUL
                + EGGTHROW_ANGLE_ADD)
                .to_radians();
            self.position.z = 0.0;
        }
        let wanted = self.position + self.velocity;
        match map_contact(surfaces(), self.position, wanted, YOSHI_EGG_MAP_COLL) {
            Some(hit) => {
                self.position = hit.position;
                // `wpYoshiEggThrowProcMap`.
                fx.push(Fx::Quake(2));
                fx.push(Fx::YoshiEggExplode(hit.position));
                fx.push(Fx::EggBreak(hit.position));
                fx.push(Fx::DustExpandSmall(hit.position));
                self.explode();
            }
            None => self.position = wanted,
        }
        true
    }

    /// `wpYoshiEggThrowProcReflector`, then the reflect damage bonus.
    fn reflect(&mut self, reflector: &Fighter) {
        self.lifetime = EGGTHROW_LIFETIME;
        self.owner_port = reflector.port;
        if self.velocity.x * reflector.facing.sign() < 0.0 {
            self.velocity.x = -self.velocity.x;
        }
        self.damage = ((self.damage as f32 * 1.8 + 0.99) as i32).min(100);
    }
}

/// `wpvars.h` star constants.
pub const YOSHISTAR_LIFETIME: u16 = 16;
pub const YOSHISTAR_LIFETIME_SCALE_MUL: f32 = 0.175;
pub const YOSHISTAR_LIFETIME_SCALE_ADD: f32 = 0.3;
pub const YOSHISTAR_VEL_CLAMP: f32 = 1.8;
pub const YOSHISTAR_ANGLE: f32 = 30.0 * core::f32::consts::PI / 180.0;
pub const YOSHISTAR_VEL: f32 = 30.0;
pub const YOSHISTAR_OFF_X: f32 = 300.0;
pub const YOSHISTAR_OFF_Y: f32 = 20.0;
pub const YOSHISTAR_ROTATE_SPEED: f32 = 0.24;

/// `llYoshiMainStarWeaponAttributes` (offset 0x40 of `247_YoshiMain.c`,
/// US): size 160, angle 361, knockback 100/30/0, 4 damage.
pub const YOSHI_STAR_HITBOX: Hitbox = Hitbox {
    damage: 4,
    offset: Vec3::ZERO,
    radius: 80.0,
    angle: 361,
    kb_scale: 100,
    kb_weight: 30,
    kb_base: 0,
    element: crate::combat::Element::Normal,
    shield_damage: -3,
};

/// Source `wpYoshiStar`: it slows down and is gone after 16 frames or on a
/// hit. Its map callback does nothing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct YoshiStar {
    pub owner_port: u8,
    pub position: Vec3,
    pub velocity: Vec3,
    pub lifetime: u16,
    pub damage: i32,
    pub lr: f32,
    /// The DObj's `rotate.z`, presentation only.
    pub rotate_z: f32,
}

impl YoshiStar {
    /// `wpYoshiStarMakeWeapon`.
    fn new(spawn: WeaponSpawn, lr: f32) -> Self {
        let (sin, cos) = sin_cos(YOSHISTAR_ANGLE);
        YoshiStar {
            owner_port: spawn.owner_port,
            position: spawn.position + Vec3::new(YOSHISTAR_OFF_X * lr, YOSHISTAR_OFF_Y, 0.0),
            velocity: Vec3::new(cos * YOSHISTAR_VEL * lr, sin * YOSHISTAR_VEL, 0.0),
            lifetime: YOSHISTAR_LIFETIME,
            damage: YOSHI_STAR_HITBOX.damage,
            lr,
            rotate_z: 0.0,
        }
    }

    /// `wpYoshiStarGetScale`, presentation only.
    pub fn scale(&self) -> f32 {
        (f32::from(self.lifetime) * YOSHISTAR_LIFETIME_SCALE_MUL + YOSHISTAR_LIFETIME_SCALE_ADD)
            .min(1.0)
    }

    /// `wpYoshiStarProcUpdate`, then the manager's move.
    fn tick(&mut self, fx: &mut Emit) -> bool {
        self.lifetime -= 1;
        if self.lifetime == 0 {
            fx.push(Fx::DustExpandSmall(self.position));
            return false;
        }
        self.rotate_z += YOSHISTAR_ROTATE_SPEED * self.lr;
        let speed = Vec2::new(self.velocity.x, self.velocity.y).length();
        if speed > 0.0 {
            let slowed = if speed < YOSHISTAR_VEL_CLAMP {
                0.0
            } else {
                speed - YOSHISTAR_VEL_CLAMP
            };
            self.velocity.x = self.velocity.x * slowed / speed;
            self.velocity.y = self.velocity.y * slowed / speed;
        }
        self.position += self.velocity;
        true
    }

    /// `wpYoshiStarProcReflector`, then the reflect damage bonus.
    fn reflect(&mut self, reflector: &Fighter) {
        self.lifetime = YOSHISTAR_LIFETIME;
        self.owner_port = reflector.port;
        if self.velocity.x * reflector.facing.sign() < 0.0 {
            self.velocity.x = -self.velocity.x;
        }
        self.rotate_z = ssb_engine::math::atan2(self.velocity.y, self.velocity.x);
        self.lr = -self.lr;
        self.damage = ((self.damage as f32 * 1.8 + 0.99) as i32).min(100);
    }

    pub fn hitbox(&self) -> Hitbox {
        Hitbox {
            damage: self.damage,
            ..YOSHI_STAR_HITBOX
        }
    }
}

/// `WPFINALCUTTER_LIFETIME` and `WPFINALCUTTER_VEL` (`wpvars.h`).
pub const KIRBY_CUTTER_LIFETIME: u16 = 20;
pub const KIRBY_CUTTER_VEL: f32 = 100.0;
/// `llKirbyMainCutterWeaponAttributes` in `229_KirbyMain.c` (US): size 250,
/// angle 361, knockback 50/0/70, 6 damage.
pub const KIRBY_CUTTER_HITBOX: Hitbox = Hitbox {
    damage: 6,
    offset: Vec3::ZERO,
    radius: 125.0,
    angle: 361,
    kb_scale: 50,
    kb_weight: 0,
    kb_base: 70,
    element: crate::combat::Element::Slash,
    shield_damage: 1,
};
pub const KIRBY_CUTTER_MAP_COLL: BodyColl = BodyColl {
    top: 220.0,
    center: 0.0,
    bottom: -220.0,
    width: 50.0,
};

/// Source `wpKirbyCutter`: the Final Cutter wave. It rides the floor line it
/// was made on, flies straight once off an edge, dies against a wall or
/// ceiling, and survives its hits (`wpKirbyCutterProcHit` returns FALSE)
/// without hitting a fighter twice.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KirbyCutter {
    pub owner_port: u8,
    pub position: Vec3,
    pub velocity: Vec3,
    pub lifetime: u16,
    pub lr: f32,
    pub damage: i32,
    /// Resolve the floor line under the spawn on the first tick.
    pub seek_floor: bool,
    pub floor: Option<MapSurface>,
    pub hit_ports: u8,
    /// `wpMainVelSetModelPitch`'s root `rotate.y`: +90 degrees for a
    /// rightward `vel_air.x`, else -90. Presentation only.
    pub model_rotate_y: f32,
    /// Root `rotate.z`: `wpKirbyCutterProcUpdate` sets it to the floor's
    /// slope while grounded. Presentation only.
    pub rotate_z: f32,
    /// `gcPlayAnimAll` calls on the tree, one per update.
    pub anim_ticks: u16,
}

/// `wpMainVelSetModelPitch`.
fn model_pitch(vel_x: f32) -> f32 {
    if vel_x >= 0.0 {
        core::f32::consts::FRAC_PI_2
    } else {
        -core::f32::consts::FRAC_PI_2
    }
}

impl KirbyCutter {
    fn new(spawn: WeaponSpawn, grounded: bool) -> Self {
        let lr = if spawn.facing < 0.0 { -1.0 } else { 1.0 };
        Self {
            owner_port: spawn.owner_port,
            position: spawn.position,
            velocity: Vec3::new(lr * KIRBY_CUTTER_VEL, 0.0, 0.0),
            lifetime: KIRBY_CUTTER_LIFETIME,
            lr,
            damage: KIRBY_CUTTER_HITBOX.damage,
            seek_floor: grounded,
            floor: None,
            hit_ports: 0,
            model_rotate_y: model_pitch(lr),
            rotate_z: 0.0,
            anim_ticks: 0,
        }
    }

    /// `wpKirbyCutterProcUpdate` then `wpKirbyCutterProcMap`.
    fn tick<I, F>(&mut self, surfaces: F, fx: &mut Emit) -> bool
    where
        F: Fn() -> I,
        I: IntoIterator<Item = MapSurface>,
    {
        if self.seek_floor {
            // `mpCollisionGetFCCommonFloor(fp->coll_data.floor_line_id, pos)`:
            // the owner's floor, which the spawn stands over.
            self.seek_floor = false;
            self.floor = surfaces()
                .into_iter()
                .filter(|s| s.kind == MapSurfaceKind::Floor)
                .filter(|seg| {
                    let [x1, _, x2, _] = seg.coords();
                    let (lo, hi) = (x1.min(x2), x1.max(x2));
                    self.position.x >= lo && self.position.x <= hi
                })
                .map(|seg| {
                    (
                        seg,
                        (segment_y(seg, self.position.x) - self.position.y).abs(),
                    )
                })
                .filter(|(_, d)| *d <= KIRBY_CUTTER_MAP_COLL.top)
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .map(|(seg, _)| seg);
        }
        // `wpProcessProcWeaponMain` plays the DObj animation before
        // `proc_update`.
        self.anim_ticks = self.anim_ticks.wrapping_add(1);
        self.lifetime -= 1;
        if self.lifetime == 0 {
            fx.push(Fx::DustExpandSmall(self.position));
            return false;
        }
        if let Some(segment) = self.floor {
            // `-syUtilsArcTan2(floor_angle.x, floor_angle.y)`.
            let normal = surface_normal(MapSurfaceKind::Floor, segment.segment);
            self.rotate_z = -ssb_engine::math::atan2(normal.x, normal.y);
        }
        self.floor = self.floor.and_then(|s| refresh_surface(surfaces(), s));
        if let Some(segment) = self.floor {
            // `wpMainVelGroundTransferAir` along the floor line.
            let normal = surface_normal(MapSurfaceKind::Floor, segment.segment);
            self.velocity.x = self.lr * normal.y * KIRBY_CUTTER_VEL;
            self.velocity.y = self.lr * -normal.x * KIRBY_CUTTER_VEL;
            let wanted =
                self.position + self.velocity + segment.motion.map_or(Vec3::ZERO, |m| m.speed);
            let [x1, _, x2, _] = segment.coords();
            let (lo, hi) = (x1.min(x2), x1.max(x2));
            if wanted.x < lo || wanted.x > hi {
                // `wpMapSetAir`: the wave keeps flying along the line.
                self.floor = None;
                self.position = wanted;
            } else {
                let y = segment_y(segment, wanted.x) - KIRBY_CUTTER_MAP_COLL.bottom;
                self.position = Vec3::new(wanted.x, y, wanted.z);
            }
            return true;
        }
        let wanted = self.position + self.velocity;
        match map_contact(surfaces(), self.position, wanted, KIRBY_CUTTER_MAP_COLL) {
            Some(hit) if hit.kind == MapSurfaceKind::Floor => {
                // `wpMapTestAllCheckFloor` then `wpMapSetGround`.
                self.floor = Some(hit.surface);
                self.position = hit.position;
                true
            }
            // A wall or the ceiling.
            Some(hit) => {
                fx.push(Fx::DustExpandSmall(hit.position));
                false
            }
            None => {
                self.position = wanted;
                true
            }
        }
    }

    /// `wpKirbyCutterProcReflector`, then the reflect damage bonus.
    fn reflect(&mut self, reflector: &Fighter) {
        self.lifetime = KIRBY_CUTTER_LIFETIME;
        self.owner_port = reflector.port;
        if self.velocity.x * reflector.facing.sign() < 0.0 {
            self.velocity.x = -self.velocity.x;
            self.velocity.y = -self.velocity.y;
        }
        self.lr = -self.lr;
        self.model_rotate_y = model_pitch(self.velocity.x);
        self.hit_ports = 0;
        self.damage = ((self.damage as f32 * 1.8 + 0.99) as i32).min(100);
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Weapon {
    Fireball(MarioFireball),
    Blaster(FoxBlaster),
    ChargeShot(SamusChargeShot),
    Bomb(SamusBomb),
    Boomerang(LinkBoomerang),
    Egg(YoshiEgg),
    Star(YoshiStar),
    Cutter(KirbyCutter),
    Jolt(ThunderJolt),
    Thunder(ThunderHead),
    Trail(ThunderTrail),
    PKFire(PKFire),
    PKThunder(PKThunder),
    PKTrail(PKThunderTrail),
    Laser(ArwingLaser),
    Monster(crate::monster_weapon::MonsterShot),
}

/// A live Mario Fireball. Weapons are match-owned, not fighter-owned:
/// the owner port survives long enough to exclude self-hits while the object
/// has its own position, velocity, lifetime, and collision result.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MarioFireball {
    /// `weapon_vars.fireball.index` into [`FIREBALL_ATTRIBUTES`].
    pub index: u8,
    pub owner_port: u8,
    pub damage: i32,
    pub position: Vec3,
    pub velocity: Vec3,
    pub lifetime: u16,
    /// The DObj's `rotate.x`: 0 at the make, advanced by
    /// [`FireballAttributes::rotate_speed`] on every update that does not
    /// expire.
    pub rotate_x: f32,
}

impl MarioFireball {
    /// `wpMarioFireballMakeWeapon` with the given attribute row.
    pub fn new(spawn: WeaponSpawn, index: u8) -> Self {
        let attr = &FIREBALL_ATTRIBUTES[index as usize];
        let (sin, cos) = sin_cos(attr.angle);
        MarioFireball {
            index,
            owner_port: spawn.owner_port,
            damage: attr.damage,
            position: spawn.position,
            velocity: Vec3::new(attr.vel_base * cos * spawn.facing, attr.vel_base * sin, 0.0),
            lifetime: attr.lifetime,
            rotate_x: 0.0,
        }
    }

    pub fn attributes(&self) -> &'static FireballAttributes {
        &FIREBALL_ATTRIBUTES[self.index as usize]
    }

    /// `wpMarioFireballProcUpdate` then `wpMarioFireballProcMap`.
    ///
    /// The source calls `wpMapTestAll` with its authored collision diamond,
    /// then only rebounds when entering a floor/ceiling/wall surface this
    /// frame. The portable sweep supplies that same one-sided map contract
    /// without coupling the match-owned weapon pool to a particular stage
    /// format or runtime.
    fn tick<I, F>(&mut self, surfaces: F, fx: &mut Emit) -> bool
    where
        F: Fn() -> I,
        I: IntoIterator<Item = MapSurface>,
    {
        if self.lifetime == 0 {
            return false;
        }
        self.lifetime -= 1;
        if self.lifetime == 0 {
            // `wpMarioFireballProcUpdate`.
            fx.push(Fx::DustExpandSmall(self.position));
            return false;
        }
        let attr = self.attributes();
        self.velocity.y = (self.velocity.y - attr.gravity).max(-attr.vel_terminal);
        self.rotate_x += attr.rotate_speed;
        let wanted = self.position + self.velocity;
        if let Some(hit) = map_contact(surfaces(), self.position, wanted, MARIO_FIREBALL_MAP_COLL) {
            self.position = hit.position;
            let dot = self.velocity.x * hit.normal.x + self.velocity.y * hit.normal.y;
            self.velocity.x = (self.velocity.x - 2.0 * dot * hit.normal.x) * attr.rebound;
            self.velocity.y = (self.velocity.y - 2.0 * dot * hit.normal.y) * attr.rebound;
            // `wpMarioFireballProcMap`.
            if self.velocity.x * self.velocity.x + self.velocity.y * self.velocity.y
                < attr.vel_min * attr.vel_min
            {
                fx.push(Fx::DustExpandSmall(self.position));
                return false;
            }
            fx.push(Fx::FireGrind(self.position));
        } else {
            self.position = wanted;
        }
        true
    }
}

/// The portable stand-in for `wpManager`'s live-object list. It is fixed-size
/// so PSP gameplay stays allocation-free; a full four-player game has room
/// for sixteen simultaneous weapons while Fireball and Blaster each consume
/// one slot per shot.
pub const MAX_WEAPONS: usize = 16;

/// Player ports the pool keeps owner views and catch events for.
const MAX_OWNERS: usize = 4;

#[derive(Debug, Clone, PartialEq)]
pub struct WeaponPool {
    star_rod_smash_desc: bool,
    slots: [Option<Weapon>; MAX_WEAPONS],
    /// PK Fire flames the sparks' hit callbacks made this frame, for
    /// [`crate::item::ItemPool::take_weapon_spawns`].
    item_spawns: [Option<crate::item::PKFireSpawn>; MAX_WEAPONS],
    /// Four `WPAttackColl` victim records, shared by fighters and items.
    /// Fighters use ports; items use `ITEM_RECORD_BASE + slot`.
    hit_records: [[Option<u8>; 4]; MAX_WEAPONS],
    /// Item searches queue `hit_normal_damage`; callbacks run after search.
    pending_item_hits: [bool; MAX_WEAPONS],
    /// This frame's [`OwnerView`] per port, from [`Self::observe_owner`].
    owners: [Option<OwnerView>; MAX_OWNERS],
    /// A returning Boomerang reached this port's thrower while its
    /// `is_special_interrupt` was set; [`Self::sync_owner`] delivers it.
    caught: [bool; MAX_OWNERS],
    thunder_destroyed: [bool; MAX_OWNERS],
    next_group: u16,
    /// Each slot's [`WeaponSpawn::stale`].
    stale: [crate::stale::WeaponStale; MAX_WEAPONS],
    /// Each slot's `wp->team`: its [`WeaponSpawn::team`], a parent weapon's
    /// for a trail, and its reflector's once reflected.
    teams: [u8; MAX_WEAPONS],
    /// The battle's team-attack rule ([`crate::team`]).
    pub team_rules: crate::team::TeamRules,
    /// Sector Z's Arwing's roll (`map_dobjs[1]->rotate.z`), from
    /// [`Self::observe_arwing_roll`]: its lasers reface by it.
    ground_roll: f32,
    /// A slot's hit that registered damage this frame, as `(owner port,
    /// motion)`, for [`Self::record_landed`].
    landed: [Option<(u8, crate::stale::MotionAttackId, u16)>; MAX_WEAPONS],
    /// The battle camera as last drawn (`gGMCameraMatrix`), from
    /// [`Self::observe_camera`].
    camera: Option<crate::camera::Camera>,
    /// Each slot's place in the weapon link (`GObj` make order), and the
    /// next one to give.
    seq: [u32; MAX_WEAPONS],
    next_seq: u32,
    /// The weapons' own effects, made at the end of the process that made
    /// them ([`Self::flush_effects`]).
    fx: crate::wpeffect::WeaponFx,
    /// The clash search's set-offs ([`Self::flush_clash_effects`]).
    clash_fx: crate::wpeffect::WeaponFx<{ crate::wpeffect::CLASH_FX_MAX }>,
    /// `hit_attack_damage` from the clash search, for
    /// [`Self::finish_clashes`].
    clash_damage: [i32; MAX_WEAPONS],
    /// A fighter's attack set the weapon off this frame (its `proc_setoff`
    /// already ran), so a clash runs no second one.
    set_off: [bool; MAX_WEAPONS],
    /// Each record's `timer_rehit` (`WEAPON_REHIT_TIME_DEFAULT` on a
    /// `can_rehit_fighter` item weapon's hit; zero otherwise).
    rehit: [[u8; 4]; MAX_WEAPONS],
    /// Onix's rocks' events for [`crate::item::ItemPool::observe_weapons`].
    rock_events: [Option<(u32, bool)>; 2 * MAX_WEAPONS],
}

/// `WEAPON_HOP_ANGLE_DEFAULT`: `F_CLC_DTOR32(135.0F)`.
const WEAPON_HOP_ANGLE_DEFAULT: f32 = 2.356_194_5;

/// `wpProcessSetHitInteractStats`: first empty record, otherwise slot zero.
fn record_weapon_victim(records: &mut [Option<u8>; 4], victim: u8) {
    if records.contains(&Some(victim)) {
        return;
    }
    let slot = records.iter().position(Option::is_none).unwrap_or(0);
    records[slot] = Some(victim);
}

fn recorded_fighter_ports(records: &[Option<u8>; 4]) -> u8 {
    records
        .iter()
        .flatten()
        .filter(|&&port| port < 4)
        .fold(0, |mask, &port| mask | (1 << port))
}

/// The `WPAttributes` interaction bits `ftMainSearchHitWeapon` and
/// `wpProcessProcHitCollisions` read. Transcribed from the relocData
/// attribute words (bitfields packed in 32-bit units, big-endian).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct WeaponFlags {
    can_setoff: bool,
    can_hop: bool,
    can_reflect: bool,
    can_absorb: bool,
}

const fn wflags(
    can_setoff: bool,
    can_hop: bool,
    can_reflect: bool,
    can_absorb: bool,
) -> WeaponFlags {
    WeaponFlags {
        can_setoff,
        can_hop,
        can_reflect,
        can_absorb,
    }
}

impl Weapon {
    /// `wp->attack_coll.can_*`.
    fn flags(&self) -> WeaponFlags {
        match self {
            // Mario and Luigi Fireball, Charge Shot, Yoshi Star, PK Fire,
            // aerial Thunder Jolt.
            Weapon::Fireball(_) | Weapon::ChargeShot(_) | Weapon::Star(_) | Weapon::PKFire(_) => {
                wflags(true, true, true, true)
            }
            Weapon::Jolt(j) => wflags(true, j.surface.is_none(), true, true),
            Weapon::Blaster(_) => wflags(false, true, true, true),
            // The 2D shot hops, reflects and is absorbed; the 3D shot is
            // only absorbed, before and after it bursts.
            Weapon::Laser(l) => wflags(false, !l.three_d, !l.three_d, true),
            Weapon::Monster(m) => {
                let a = m.attributes();
                wflags(a.can_setoff, a.can_hop, a.can_reflect, a.can_absorb)
            }
            Weapon::Bomb(_) => wflags(false, true, false, false),
            Weapon::Boomerang(_) | Weapon::Egg(_) => wflags(true, true, true, false),
            Weapon::Cutter(_) | Weapon::PKThunder(_) => wflags(true, false, true, true),
            Weapon::Thunder(_) | Weapon::Trail(_) | Weapon::PKTrail(_) => {
                wflags(false, false, false, false)
            }
        }
    }

    /// `wp->attack_coll.priority`.
    fn priority(&self) -> i32 {
        match self {
            Weapon::ChargeShot(c) => SAMUS_CHARGE_SHOT_PRIORITIES[usize::from(c.charge.min(7))],
            _ => WEAPON_PRIORITY,
        }
    }

    /// `wp->group_id`, non-zero for PK Thunder's and Thunder's heads and
    /// trails (`wpManagerGetGroupID`).
    fn group(&self) -> Option<u16> {
        match self {
            Weapon::PKThunder(h) => Some(h.group),
            Weapon::PKTrail(t) => Some(t.group),
            Weapon::Thunder(h) => Some(h.group),
            Weapon::Trail(t) => Some(t.group),
            _ => None,
        }
    }

    /// `wp->ga == nMPKineticsGround`: a Thunder Jolt crawling on a surface
    /// and a Final Cutter wave on the floor (`wpMapSetGround`).
    fn is_grounded(&self) -> bool {
        match self {
            Weapon::Jolt(j) => j.surface.is_some(),
            Weapon::Cutter(c) => c.floor.is_some(),
            Weapon::Bomb(b) => b.floor.is_some(),
            _ => false,
        }
    }

    /// `DObjGetStruct(weapon_gobj)->translate`.
    fn position(&self) -> Vec3 {
        match self {
            Weapon::Fireball(w) => w.position,
            Weapon::Blaster(w) => w.position,
            Weapon::Laser(w) => w.position,
            Weapon::Monster(w) => w.position,
            Weapon::ChargeShot(w) => w.position,
            Weapon::Bomb(w) => w.position,
            Weapon::Boomerang(w) => w.position,
            Weapon::Egg(w) => w.position,
            Weapon::Star(w) => w.position,
            Weapon::Cutter(w) => w.position,
            Weapon::Jolt(w) => w.position,
            Weapon::Thunder(w) => w.position,
            Weapon::Trail(w) => w.position,
            Weapon::PKFire(w) => w.position,
            Weapon::PKThunder(w) => w.position,
            Weapon::PKTrail(w) => w.position,
        }
    }

    /// The effects the kind's `proc_hit`, `proc_shield`, `proc_setoff` or
    /// `proc_absorb` makes before it changes or destroys the weapon, at its
    /// `DObj`'s translation. The shocks take `attack_coll.damage`.
    fn proc_effects(&self, proc: Proc, fx: &mut Emit) {
        let pos = self.position();
        let shock = |size| Fx::ImpactShock { pos, size };
        match self {
            // `wpMarioFireballProcHit` for all four.
            Weapon::Fireball(_) => fx.push(Fx::SparkleWhite(pos)),
            // `wpFoxBlasterProcHit`.
            Weapon::Blaster(_) => fx.push(Fx::FoxBlasterGlow(pos)),
            // `grSectorArwingWeaponLaser2DProcHit` for all four;
            // `...3DProcHit` and `...3DProcAbsorb` burst, and the burst
            // clears them.
            Weapon::Laser(l) => {
                if !l.three_d {
                    fx.push(shock(l.damage));
                } else if !l.exploded {
                    fx.push(Fx::SparkleWhiteMultiExplode(pos));
                }
            }
            // `wpSamusChargeShotProcHit`.
            Weapon::ChargeShot(c) => fx.push(shock(c.damage)),
            // `wpSamusBombProcHit` and `...ProcAbsorb`; the explosion
            // clears them.
            Weapon::Bomb(b) => {
                if !b.exploded {
                    fx.push(Fx::SparkleWhiteMultiExplode(pos));
                }
            }
            // `wpLinkBoomerangProcHit`, `...ProcShield`, `...ProcSetOff`.
            Weapon::Boomerang(_) => {}
            // `wpYoshiEggThrowProcHit` (no `proc_absorb`); the explosion
            // clears it.
            Weapon::Egg(e) => {
                if !e.exploded && proc != Proc::Absorb {
                    fx.push(Fx::YoshiEggExplode(pos));
                    fx.push(Fx::EggBreak(pos));
                }
            }
            // `wpYoshiStarProcHit` and `...ProcShield`.
            Weapon::Star(_) => fx.push(Fx::SparkleWhite(pos)),
            // `wpKirbyCutterProcHit` and `...ProcSetOff`; its
            // `...ProcShield` (also its `proc_absorb`) makes nothing.
            Weapon::Cutter(_) => {
                if matches!(proc, Proc::Hit | Proc::SetOff) {
                    fx.push(Fx::SparkleWhite(pos));
                }
            }
            // `wpPikachuThunderJoltAirProcHit` and `...GroundProcHit`.
            Weapon::Jolt(j) => fx.push(shock(j.damage)),
            // `wpNessPKFireProcHit` makes the pillar; `...ProcAbsorb`
            // (also its `proc_shield`) a dust cloud.
            Weapon::PKFire(_) => {
                if matches!(proc, Proc::Shield | Proc::Absorb) {
                    fx.push(Fx::DustExpandSmall(pos));
                }
            }
            // `wpNessPKThunderHeadProcHit` and `wpNessPKReflectHeadProcHit`.
            Weapon::PKThunder(h) => fx.push(shock(h.damage)),
            // `wpNessPKThunderTrailProcHit` and `...ReflectTrailProcHit`.
            Weapon::PKTrail(_) => fx.push(shock(ness::TRAIL_HIT.damage)),
            // `wpPikachuThunderTrailProcHit`.
            Weapon::Trail(_) => fx.push(shock(pikachu::TRAIL_HIT.damage)),
            // The head has no callbacks.
            Weapon::Thunder(_) => {}
            Weapon::Monster(m) => m.proc_fx(shot_proc(proc), fx),
        }
    }

    /// `wpProcessProcHitCollisions`'s shield branch after
    /// `ftMainUpdateShieldStatWeapon` recorded the fighter: an airborne
    /// `can_hop` weapon that met the shield under 135 degrees hops, any
    /// other runs its `proc_shield`. Returns whether the weapon lives on.
    fn on_shield(&mut self, shield: crate::combat::ShieldCollide, fx: &mut Emit) -> bool {
        // The Bomb's explosion clears `proc_hop` and `proc_shield`.
        if matches!(self, Weapon::Bomb(b) if b.exploded) {
            return true;
        }
        let hops = self.flags().can_hop && !self.is_grounded();
        if hops && shield.angle < WEAPON_HOP_ANGLE_DEFAULT {
            self.hop((shield.angle - DEG_90).max(0.0), shield.dir_z);
            // `wpFoxBlasterProcHop`.
            if let Weapon::Blaster(b) = self {
                fx.push(Fx::FoxBlasterGlow(b.position));
            }
            return true;
        }
        self.proc_effects(Proc::Shield, fx);
        match self {
            // `wpLinkBoomerangProcShield`.
            Weapon::Boomerang(b) => b.set_off(),
            Weapon::Monster(m) if m.survives(ShotProc::Shield) => {}
            // `wpKirbyCutterProcShield` returns FALSE.
            Weapon::Cutter(_) => {}
            // `wpYoshiEggThrowProcHit` and `wpSamusBombProcHit`.
            Weapon::Egg(e) => e.explode(),
            Weapon::Bomb(b) => b.explode(),
            // Every other `proc_shield` returns TRUE, PK Fire's
            // `wpNessPKFireProcAbsorb` included.
            _ => return false,
        }
        true
    }

    /// `wpProcessProcHitCollisions`'s hop branch: the kind's `proc_hop`,
    /// with `shield_collide_angle` already less 90 degrees. Every hop turns
    /// `vel_air` by twice that angle about `shield_collide_dir`; the rest of
    /// each callback is the facing it rederives. Model pitch and roll
    /// (`wpMainVelSetModelPitch`, the Blaster's and Star's `rotate.z`, PK
    /// Fire's negated roll) are presentation that nothing draws yet.
    fn hop(&mut self, angle: f32, dir_z: f32) {
        let turn = |v: Vec3| hop_velocity(v, angle, dir_z);
        match self {
            Weapon::Boomerang(b) => b.hop(angle, dir_z),
            Weapon::Monster(m) => m.hop(turn(m.velocity)),
            // `wpMarioFireballProcHop`.
            Weapon::Fireball(f) => f.velocity = turn(f.velocity),
            // `wpFoxBlasterProcHop`: the shot is drawn unstretched again.
            Weapon::Blaster(b) => {
                b.velocity = turn(b.velocity);
                b.scale_x = 1.0;
            }
            // `wpSamusChargeShotProcHop`: its spin reads the new velocity.
            Weapon::ChargeShot(c) => c.velocity = turn(c.velocity),
            // `wpSamusBombProcHop`: `wpMainVelSetLR`.
            Weapon::Bomb(b) => {
                b.velocity = turn(b.velocity);
                b.set_lr();
            }
            // `wpYoshiEggThrowProcHop`.
            Weapon::Egg(e) => e.velocity = turn(e.velocity),
            // `wpYoshiStarProcHop`: facing from a strictly positive X.
            Weapon::Star(s) => {
                s.velocity = turn(s.velocity);
                s.rotate_z = ssb_engine::math::atan2(s.velocity.y, s.velocity.x);
                s.lr = if s.velocity.x > 0.0 { 1.0 } else { -1.0 };
            }
            // `wpPikachuThunderJoltAirProcHop`.
            Weapon::Jolt(j) => j.velocity = turn(j.velocity),
            // `wpNessPKFireProcHop`.
            Weapon::PKFire(p) => {
                p.velocity = turn(p.velocity);
                p.rotate_z = -p.rotate_z;
            }
            _ => unreachable!("no proc_hop"),
        }
    }
}

/// One weapon's attack for the clash search ([`WeaponPool::search_weapons`]).
#[derive(Debug, Clone, Copy, PartialEq)]
struct ClashAttack {
    owner: u8,
    team: u8,
    /// `wpMainGetStaledDamage`.
    damage: i32,
    priority: i32,
    pos_curr: Vec3,
    pos_prev: Vec3,
    size: f32,
    state: crate::combat::AttackState,
}

/// The item weapons' name for a collision callback.
fn shot_proc(proc: Proc) -> ShotProc {
    match proc {
        Proc::Hit => ShotProc::Hit,
        Proc::Shield => ShotProc::Shield,
        Proc::SetOff => ShotProc::SetOff,
        Proc::Absorb => ShotProc::Absorb,
    }
}

/// Queues the effects of `w`'s callback `proc` under its link place `seq`.
fn push_proc(fx: &mut crate::wpeffect::WeaponFx, seq: u32, w: &Weapon, proc: Proc) {
    let mut emit = Emit::default();
    w.proc_effects(proc, &mut emit);
    fx.extend(seq, &emit);
}

/// Which of a weapon's collision callbacks runs ([`Weapon::proc_effects`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Proc {
    Hit,
    Shield,
    SetOff,
    Absorb,
}

/// The attack-record id naming weapon slot `i` as a victim
/// (`victim_gobj` is a weapon; a clash records it).
fn weapon_victim_id(i: usize) -> u8 {
    crate::combat::WEAPON_RECORD_BASE + i as u8
}

/// `syVectorRotateAbout3D(&vel_air, &shield_collide_dir, angle * 2)`, where
/// the direction is `{ 0, 0, dir_z }`.
fn hop_velocity(v: Vec3, angle: f32, dir_z: f32) -> Vec3 {
    crate::item::rotate_about(v, Vec3::new(0.0, 0.0, dir_z), angle * 2.0)
}

/// `wpProcessProcWeaponMain`'s blast-zone test, strict on every edge.
pub(crate) fn out_of_bounds(b: BlastZone, p: Vec3) -> bool {
    p.y < b.bottom
        || p.x > b.right
        || p.x < b.left
        || p.y > b.top
        || p.z < -20_000.0
        || p.z > 20_000.0
}

/// What `ftMainSearchHitWeapon` did with a weapon before its shield and
/// hurtbox tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PreHit {
    /// Nothing; test the shield and hurtboxes.
    None,
    /// A fighter attack beat it (`hit_attack_damage`, `proc_setoff`).
    SetOff,
    /// Turned back by a reflector (`reflect_gobj`, `proc_reflector`).
    Reflected,
    /// Broke the reflector; the weapon takes a normal hit (`proc_hit`).
    ReflectorBroke,
    /// Taken by PSI Magnet (`absorb_gobj`, `proc_absorb`).
    Absorbed,
}

/// `ftMainSearchHitWeapon` up to the shield: the attack-versus-weapon clank,
/// then the reflector and absorber special collisions. `damage` is the
/// weapon's staled damage and `slot` identifies it in attack records;
/// `team` is the weapon's.
#[allow(clippy::too_many_arguments)]
fn pre_hit(
    defender: &mut Fighter,
    flags: WeaponFlags,
    grounded: bool,
    owner: u8,
    team: u8,
    rules: crate::team::TeamRules,
    slot: usize,
    hitbox: Hitbox,
    position: Vec3,
    velocity: Vec3,
) -> PreHit {
    pre_hit_at(
        defender,
        flags,
        grounded,
        owner,
        team,
        rules,
        slot,
        hitbox,
        position,
        position - velocity,
        velocity,
    )
}

/// [`pre_hit`] with the attack's previous position given.
#[allow(clippy::too_many_arguments)]
fn pre_hit_at(
    defender: &mut Fighter,
    flags: WeaponFlags,
    grounded: bool,
    owner: u8,
    team: u8,
    rules: crate::team::TeamRules,
    slot: usize,
    hitbox: Hitbox,
    position: Vec3,
    prev: Vec3,
    velocity: Vec3,
) -> PreHit {
    let w = crate::combat::WeaponAttack {
        hitbox,
        pos_curr: position,
        pos_prev: prev,
        source: crate::combat::HitSource::Weapon { vel_x: velocity.x },
        handicap: crate::stale::HANDICAP_DEFAULT,
        can_shield: true,
        owner: Some(owner),
        is_hitlag_victim: None,
    };
    let reflector = crate::combat::reflector(defender).filter(|_| flags.can_reflect);
    // A thrown fighter's attacks do not meet its thrower's weapons, nor,
    // with team attack off, its thrower's teammates'.
    let thrown_spares = crate::combat::throw_team(defender).is_some_and(|throw_team| {
        crate::combat::throw_port(defender) == Some(owner) || rules.spares(throw_team, team)
    });
    if flags.can_setoff
        && !defender.grab.is_catchstatus
        && !thrown_spares
        && reflector.is_none()
        && crate::combat::weapon_attack_clank(
            defender,
            &w,
            crate::combat::WEAPON_RECORD_BASE + slot as u8,
            grounded,
        )
    {
        return PreHit::SetOff;
    }
    if !crate::combat::weapon_in_range(defender, &w) {
        return PreHit::None;
    }
    if let Some(r) = reflector {
        if crate::combat::special_contact(defender, &r, w.pos_curr, w.pos_prev, hitbox.radius) {
            return match crate::combat::reflect_weapon(defender, &r, hitbox.damage, position) {
                crate::combat::ReflectOutcome::Reflected => PreHit::Reflected,
                crate::combat::ReflectOutcome::Broke => PreHit::ReflectorBroke,
            };
        }
    }
    if let Some(a) = crate::combat::absorber(defender).filter(|_| flags.can_absorb) {
        if crate::combat::special_contact(defender, &a, w.pos_curr, w.pos_prev, hitbox.radius) {
            crate::combat::absorb_weapon(defender, hitbox.damage, position);
            return PreHit::Absorbed;
        }
    }
    PreHit::None
}

/// The pool state one Arwing laser's fighter search writes.
struct LaserHit<'a> {
    records: &'a mut [Option<u8>; 4],
    /// The fighter ports in `records`.
    ports: u8,
    stale: crate::stale::WeaponStale,
    team: &'a mut u8,
    landed: &'a mut Option<(u8, crate::stale::MotionAttackId, u16)>,
    fx: &'a mut crate::wpeffect::WeaponFx,
    seq: u32,
    rules: crate::team::TeamRules,
    slot: usize,
}

/// `ftMainSearchHitWeapon` and `wpProcessProcHitCollisions` for one of
/// Sector Z's Arwing lasers against one fighter. Returns whether the laser
/// lives on. Its hits credit no fighter until a reflector takes it, and the
/// 3D shot and its burst pass shields (`can_shield` clear).
fn laser_hit(laser: &mut ArwingLaser, h: LaserHit<'_>, defender: &mut Fighter) -> bool {
    let owner = laser.owner_port;
    if owner == defender.port || h.rules.spares(defender.team, *h.team) {
        return true;
    }
    let bit = 1u8 << (defender.port & 7);
    if h.ports & bit != 0 {
        return true;
    }
    let mut staled = laser.hitbox();
    staled.damage = h.stale.damage(staled.damage);
    let owner_player = (owner != sector::GROUND_PORT).then_some(owner);
    let attack = |pos: Vec3, vel: Vec3, can_shield: bool| crate::combat::WeaponAttack {
        hitbox: staled,
        pos_curr: pos,
        pos_prev: pos - vel,
        source: crate::combat::HitSource::Weapon { vel_x: vel.x },
        handicap: crate::stale::HANDICAP_DEFAULT,
        can_shield,
        owner: owner_player,
        is_hitlag_victim: None,
    };
    let landed = |landed: &mut Option<_>| {
        if owner_player.is_some() {
            *landed = Some((owner, h.stale.attack_id, h.stale.motion_count));
        }
    };
    // The burst only damages: every callback but `proc_update` is cleared.
    if laser.exploded {
        let contact =
            crate::combat::weapon_hit(defender, attack(laser.position, laser.velocity, false));
        if attack::HitOutcome::of(contact).registered() {
            record_weapon_victim(h.records, defender.port);
            if contact == crate::combat::WeaponContact::Hurt(true) {
                landed(h.landed);
            }
        }
        return true;
    }
    let flags = wflags(false, !laser.three_d, !laser.three_d, true);
    let pre = pre_hit(
        defender,
        flags,
        false,
        owner,
        *h.team,
        h.rules,
        h.slot,
        staled,
        laser.position,
        laser.velocity,
    );
    let mut emit = Emit::default();
    let hit_fx = |l: &ArwingLaser, emit: &mut Emit| {
        if !l.three_d {
            emit.push(Fx::ImpactShock {
                pos: l.position,
                size: l.damage,
            });
        } else if !l.exploded {
            emit.push(Fx::SparkleWhiteMultiExplode(l.position));
        }
    };
    match pre {
        PreHit::None | PreHit::SetOff => {}
        // `grSectorArwingWeaponLaser2DProcReflector`.
        PreHit::Reflected => {
            reflect_shot(
                &mut laser.velocity,
                &mut laser.owner_port,
                &mut laser.damage,
                defender,
            );
            laser.reface();
            *h.team = defender.team;
            return true;
        }
        // `proc_hit` (the 2D shot's; the 3D shot cannot be reflected) and
        // `proc_absorb`: both destroy it.
        PreHit::ReflectorBroke | PreHit::Absorbed => {
            hit_fx(laser, &mut emit);
            h.fx.extend(h.seq, &emit);
            return false;
        }
    }
    let contact = crate::combat::weapon_hit(
        defender,
        attack(laser.position, laser.velocity, laser.can_shield()),
    );
    if let crate::combat::WeaponContact::Shielded(shield) = contact {
        record_weapon_victim(h.records, defender.port);
        // `grSectorArwingWeaponLaser2DProcHop` under 135 degrees, else its
        // `proc_shield` (`...ProcHit`).
        if shield.angle < WEAPON_HOP_ANGLE_DEFAULT {
            let angle = (shield.angle - DEG_90).max(0.0);
            laser.velocity = hop_velocity(laser.velocity, angle, shield.dir_z);
            laser.reface();
            return true;
        }
        hit_fx(laser, &mut emit);
        h.fx.extend(h.seq, &emit);
        return false;
    }
    if attack::HitOutcome::of(contact).registered() {
        record_weapon_victim(h.records, defender.port);
        if contact == crate::combat::WeaponContact::Hurt(true) {
            landed(h.landed);
        }
        hit_fx(laser, &mut emit);
        h.fx.extend(h.seq, &emit);
        if !laser.three_d {
            return false;
        }
        // `grSectorArwingWeaponLaserExplodeInitVars`.
        laser.explode();
        *h.records = [None; 4];
    }
    true
}

/// The pool state one item weapon's fighter search writes.
struct MonsterHit<'a> {
    records: &'a mut [Option<u8>; 4],
    rehit: &'a mut [u8; 4],
    /// The fighter ports in `records`.
    ports: u8,
    stale: crate::stale::WeaponStale,
    team: &'a mut u8,
    fx: &'a mut crate::wpeffect::WeaponFx,
    seq: u32,
    rules: crate::team::TeamRules,
    slot: usize,
    set_off: &'a mut bool,
    landed: &'a mut Option<(u8, crate::stale::MotionAttackId, u16)>,
}

/// `wpProcessSetHitInteractStats` for a fighter: a `can_rehit_fighter`
/// weapon's record clears after `WEAPON_REHIT_TIME_DEFAULT` updates.
fn record_shot_victim(h: &mut MonsterHit<'_>, port: u8, rehit: bool) {
    record_weapon_victim(h.records, port);
    if rehit {
        if let Some(i) = h.records.iter().position(|r| *r == Some(port)) {
            h.rehit[i] = 16;
        }
    }
}

/// `ftMainSearchHitWeapon` and `wpProcessProcHitCollisions` for one
/// item-made weapon against one fighter ([`crate::monster_weapon`]).
/// Returns whether it lives on. Its attack sweeps from its last position
/// (the Hydro Pump's offset moves while it stands), its hits credit its
/// player, and the Smog passes shields (`can_shield` clear).
fn monster_hit(
    m: &mut crate::monster_weapon::MonsterShot,
    mut h: MonsterHit<'_>,
    defender: &mut Fighter,
) -> bool {
    let owner = m.owner.unwrap_or(sector::GROUND_PORT);
    if m.owner == Some(defender.port) || h.rules.spares(defender.team, *h.team) {
        return true;
    }
    let bit = 1u8 << (defender.port & 7);
    if h.ports & bit != 0 {
        return true;
    }
    let attr = m.attributes();
    let rehit = attr.can_rehit_fighter;
    let mut staled = m.hitbox();
    staled.damage = h.stale.damage(staled.damage);
    let (curr, prev) = m.attack_positions();
    let flags = wflags(
        attr.can_setoff,
        attr.can_hop,
        attr.can_reflect,
        attr.can_absorb,
    );
    let mut pre = pre_hit_at(
        defender, flags, false, owner, *h.team, h.rules, h.slot, staled, curr, prev, m.velocity,
    );
    if matches!(pre, PreHit::None) && m.kind == ShotKind::RayGun {
        if let Some((tail, old)) = m.attack_tail {
            pre = pre_hit_at(
                defender, flags, false, owner, *h.team, h.rules, h.slot, staled, tail, old,
                m.velocity,
            );
        }
    }
    let mut emit = Emit::default();
    let alive = match pre {
        PreHit::None => None,
        PreHit::SetOff => {
            *h.set_off = true;
            m.proc_fx(ShotProc::SetOff, &mut emit);
            Some(m.survives(ShotProc::SetOff))
        }
        PreHit::Reflected => {
            m.reflect(
                defender.port,
                defender.team,
                defender.facing.sign(),
                &mut emit,
            );
            *h.team = defender.team;
            Some(true)
        }
        PreHit::ReflectorBroke => {
            m.proc_fx(ShotProc::Hit, &mut emit);
            Some(m.survives(ShotProc::Hit))
        }
        PreHit::Absorbed => {
            m.proc_fx(ShotProc::Absorb, &mut emit);
            Some(m.survives(ShotProc::Absorb))
        }
    };
    if let Some(alive) = alive {
        if alive && !matches!(pre, PreHit::Reflected) {
            record_shot_victim(&mut h, defender.port, rehit);
        }
        h.fx.extend(h.seq, &emit);
        return alive;
    }
    let contact = crate::combat::weapon_hit_pair(
        defender,
        crate::combat::WeaponAttack {
            hitbox: staled,
            pos_curr: curr,
            pos_prev: prev,
            source: crate::combat::HitSource::Weapon {
                vel_x: m.velocity.x,
            },
            handicap: crate::stale::HANDICAP_DEFAULT,
            can_shield: attr.can_shield,
            owner: m.player,
            is_hitlag_victim: m
                .is_hitlag_victim()
                .then_some(m.player.unwrap_or(sector::GROUND_PORT)),
        },
        if m.kind == ShotKind::RayGun {
            m.attack_tail
        } else {
            None
        },
    );
    if let crate::combat::WeaponContact::Shielded(shield) = contact {
        record_shot_victim(&mut h, defender.port, false);
        if attr.can_hop && shield.angle < WEAPON_HOP_ANGLE_DEFAULT {
            let angle = (shield.angle - DEG_90).max(0.0);
            m.hop(hop_velocity(m.velocity, angle, shield.dir_z));
            return true;
        }
        m.proc_fx(ShotProc::Shield, &mut emit);
        h.fx.extend(h.seq, &emit);
        return m.survives(ShotProc::Shield);
    }
    if attack::HitOutcome::of(contact).registered() {
        if contact == crate::combat::WeaponContact::Hurt(true) {
            *h.landed = m
                .player
                .map(|p| (p, h.stale.attack_id, h.stale.motion_count));
        }
        record_shot_victim(&mut h, defender.port, rehit);
        m.proc_fx(ShotProc::Hit, &mut emit);
        h.fx.extend(h.seq, &emit);
        return m.survives(ShotProc::Hit);
    }
    true
}

/// `wpMainReflectorSetLR` and the reflect bonus for the straight shots:
/// turn X toward the reflector's facing, take its ownership and deal
/// `damage * 1.8 + 0.99` (US), at most 100.
fn reflect_shot(velocity: &mut Vec3, owner: &mut u8, damage: &mut i32, reflector: &Fighter) {
    *owner = reflector.port;
    if velocity.x * reflector.facing.sign() < 0.0 {
        velocity.x = -velocity.x;
    }
    *damage = ((*damage as f32 * 1.8 + 0.99) as i32).min(100);
}

/// One weapon hitbox against one fighter: `wpMainGetStaledDamage`, then
/// `ftMainSearchHitWeapon`'s shield and hurtbox tests. The hit is recorded in
/// the fighter's frame ([`crate::combat`]) and lands in its
/// `ftMainProcParams`; the weapon reacts now. `velocity` also sweeps the
/// hitbox back to where it was last frame and picks the push direction
/// ([`attack::HitDirection::from_weapon`]).
fn stale_hit(
    hitbox: &Hitbox,
    position: Vec3,
    velocity: Vec3,
    stale: crate::stale::WeaponStale,
    defender: &mut Fighter,
    landed: &mut Option<(u8, crate::stale::MotionAttackId, u16)>,
    owner: u8,
) -> attack::HitOutcome {
    let mut hitbox = *hitbox;
    hitbox.damage = stale.damage(hitbox.damage);
    // `wp->handicap` is the owner's; every Training player has the default.
    let outcome = attack::register_hitbox(
        &hitbox,
        position,
        position - velocity,
        crate::combat::HitSource::Weapon { vel_x: velocity.x },
        crate::stale::HANDICAP_DEFAULT,
        defender,
    );
    if outcome == attack::HitOutcome::Damaged {
        *landed = Some((owner, stale.attack_id, stale.motion_count));
    }
    outcome
}

/// [`stale_hit`], keeping the shield's hop data.
#[allow(clippy::too_many_arguments)]
fn stale_contact(
    hitbox: &Hitbox,
    position: Vec3,
    velocity: Vec3,
    stale: crate::stale::WeaponStale,
    defender: &mut Fighter,
    landed: &mut Option<(u8, crate::stale::MotionAttackId, u16)>,
    owner: u8,
    is_hitlag_victim: Option<u8>,
) -> crate::combat::WeaponContact {
    let mut hitbox = *hitbox;
    hitbox.damage = stale.damage(hitbox.damage);
    let contact = attack::register_hitbox_contact_with(
        &hitbox,
        position,
        position - velocity,
        crate::combat::HitSource::Weapon { vel_x: velocity.x },
        crate::stale::HANDICAP_DEFAULT,
        defender,
        is_hitlag_victim,
    );
    if contact == crate::combat::WeaponContact::Hurt(true) {
        *landed = Some((owner, stale.attack_id, stale.motion_count));
    }
    contact
}

impl Default for WeaponPool {
    fn default() -> Self {
        WeaponPool {
            star_rod_smash_desc: false,
            slots: [None; MAX_WEAPONS],
            item_spawns: [None; MAX_WEAPONS],
            hit_records: [[None; 4]; MAX_WEAPONS],
            pending_item_hits: [false; MAX_WEAPONS],
            owners: [None; MAX_OWNERS],
            caught: [false; MAX_OWNERS],
            thunder_destroyed: [false; MAX_OWNERS],
            next_group: 1,
            stale: [crate::stale::WeaponStale::FRESH; MAX_WEAPONS],
            teams: [crate::team::TEAM_DEFAULT; MAX_WEAPONS],
            team_rules: crate::team::TeamRules::FREE_FOR_ALL,
            ground_roll: 0.0,
            landed: [None; MAX_WEAPONS],
            camera: None,
            seq: [0; MAX_WEAPONS],
            next_seq: 0,
            fx: Default::default(),
            clash_fx: Default::default(),
            clash_damage: [0; MAX_WEAPONS],
            set_off: [false; MAX_WEAPONS],
            rehit: [[0; 4]; MAX_WEAPONS],
            rock_events: [None; 2 * MAX_WEAPONS],
        }
    }
}

impl WeaponPool {
    /// [`Self::default`] in place: the pool holds its effect queues, which a
    /// temporary would put on the (256 KB) PSP main-thread stack.
    pub fn reset(&mut self) {
        self.slots = [None; MAX_WEAPONS];
        self.item_spawns = [None; MAX_WEAPONS];
        self.hit_records = [[None; 4]; MAX_WEAPONS];
        self.pending_item_hits = [false; MAX_WEAPONS];
        self.owners = [None; MAX_OWNERS];
        self.caught = [false; MAX_OWNERS];
        self.thunder_destroyed = [false; MAX_OWNERS];
        self.next_group = 1;
        self.stale = [crate::stale::WeaponStale::FRESH; MAX_WEAPONS];
        self.teams = [crate::team::TEAM_DEFAULT; MAX_WEAPONS];
        self.team_rules = crate::team::TeamRules::FREE_FOR_ALL;
        self.ground_roll = 0.0;
        self.landed = [None; MAX_WEAPONS];
        self.camera = None;
        self.seq = [0; MAX_WEAPONS];
        self.next_seq = 0;
        self.fx.clear();
        self.clash_fx.clear();
        self.clash_damage = [0; MAX_WEAPONS];
        self.set_off = [false; MAX_WEAPONS];
    }

    fn tick_pk_thunder<I, F>(&mut self, surfaces: F, bounds: Option<BlastZone>)
    where
        F: Fn() -> I + Copy,
        I: IntoIterator<Item = MapSurface>,
    {
        let mut pending = [None; MAX_WEAPONS];
        for (i, slot) in self.slots.iter_mut().enumerate() {
            if let Some(Weapon::PKThunder(h)) = slot {
                let owner = self.owners.get(h.owner_port as usize).copied().flatten();
                let mut emit = Emit::default();
                let alive = h.tick(surfaces, owner, &mut emit);
                self.fx.extend(self.seq[i], &emit);
                // `wpNessPKThunderHeadProcDead` destroys the trails with the
                // head; the owner learns of it from the missing head.
                if !alive || bounds.is_some_and(|b| out_of_bounds(b, h.position)) {
                    *slot = None;
                } else if h.trail_spawn {
                    pending[i] = Some((PKThunderTrail::new(*h, 0), self.stale[i], self.teams[i]));
                }
            }
        }
        let mut heads = [None; MAX_WEAPONS];
        for (i, slot) in self.slots.iter().enumerate() {
            if let Some(Weapon::PKThunder(h)) = slot {
                heads[i] = Some(*h);
            }
        }
        for (i, slot) in self.slots.iter_mut().enumerate() {
            if let Some(Weapon::PKTrail(t)) = slot {
                if let Some(head) = heads.iter().flatten().find(|h| h.group == t.group) {
                    let mut emit = Emit::default();
                    t.tick(*head, &mut emit);
                    self.fx.extend(self.seq[i], &emit);
                    if t.spawn_next {
                        let mut child = PKThunderTrail::new(*head, t.id + 1);
                        child.position = t.position;
                        pending[i] = Some((child, self.stale[i], self.teams[i]));
                    }
                } else {
                    *slot = None;
                }
            }
        }
        for (trail, stale, team) in pending.into_iter().flatten() {
            self.insert(Weapon::PKTrail(trail), stale, team);
        }
    }
    fn clear_pk_trails(&mut self) {
        let mut groups = [None; MAX_WEAPONS];
        for (i, w) in self.slots.iter().enumerate() {
            if let Some(Weapon::PKThunder(h)) = w {
                groups[i] = Some(h.group);
            }
        }
        for slot in &mut self.slots {
            if matches!(slot, Some(Weapon::PKTrail(t)) if !groups.contains(&Some(t.group))) {
                *slot = None;
            }
        }
    }
    fn apply_pk_hits(&mut self, defender: &mut Fighter) {
        let rules = self.team_rules;
        let bit = 1u8 << (defender.port & 7);
        let mut groups = [None; MAX_WEAPONS];
        let mut pillars = [None; MAX_WEAPONS];
        let mut free_slots = self.slots.iter().filter(|s| s.is_none()).count();
        for (i, slot) in self.slots.iter_mut().enumerate() {
            let Some(w) = slot else { continue };
            match w {
                Weapon::PKTrail(t) => {
                    if t.owner_port == defender.port
                        || rules.spares(defender.team, self.teams[i])
                        || t.hit_ports & bit != 0
                        || groups.contains(&Some(t.group))
                    {
                        continue;
                    }
                    if stale_hit(
                        &ness::TRAIL_HIT,
                        t.hit_position(),
                        Vec3::ZERO,
                        self.stale[i],
                        defender,
                        &mut self.landed[i],
                        t.owner_port,
                    )
                    .registered()
                    {
                        t.hit_ports |= bit;
                        groups[i] = Some(t.group);
                        // `wpNessPKThunderTrailProcHit`: the trail flies on.
                        self.fx.push(
                            self.seq[i],
                            Fx::ImpactShock {
                                pos: t.position,
                                size: ness::TRAIL_HIT.damage,
                            },
                        );
                    }
                }
                Weapon::PKFire(spark) => {
                    if spark.owner_port == defender.port
                        || rules.spares(defender.team, self.teams[i])
                        || recorded_fighter_ports(&self.hit_records[i]) & bit != 0
                    {
                        continue;
                    }
                    let mut hit = ness::SPARK_HIT;
                    hit.damage = self.stale[i].damage(spark.damage);
                    let flags = wflags(true, true, true, true);
                    match pre_hit(
                        defender,
                        flags,
                        false,
                        spark.owner_port,
                        self.teams[i],
                        rules,
                        i,
                        hit,
                        spark.position,
                        spark.velocity,
                    ) {
                        PreHit::None => {}
                        // `wpNessPKFireProcReflector`.
                        PreHit::Reflected => {
                            spark.reflect(defender);
                            self.teams[i] = defender.team;
                            continue;
                        }
                        // `proc_hit` (`wpNessPKFireProcHit`): the pillar.
                        PreHit::SetOff | PreHit::ReflectorBroke => {
                            self.set_off[i] = true;
                            pillars[i] = Some(spark.item_spawn(self.stale[i], self.teams[i]));
                            *slot = None;
                            free_slots += 1;
                            continue;
                        }
                        // `wpNessPKFireProcAbsorb`.
                        PreHit::Absorbed => {
                            self.fx
                                .push(self.seq[i], Fx::DustExpandSmall(spark.position));
                            *slot = None;
                            free_slots += 1;
                            continue;
                        }
                    }
                    let mut hit = ness::SPARK_HIT;
                    hit.damage = spark.damage;
                    let contact = stale_contact(
                        &hit,
                        spark.position,
                        spark.velocity,
                        self.stale[i],
                        defender,
                        &mut self.landed[i],
                        spark.owner_port,
                        None,
                    );
                    if let crate::combat::WeaponContact::Shielded(shield) = contact {
                        record_weapon_victim(&mut self.hit_records[i], defender.port);
                        let mut emit = Emit::default();
                        let alive = w.on_shield(shield, &mut emit);
                        self.fx.extend(self.seq[i], &emit);
                        if !alive {
                            *slot = None;
                            free_slots += 1;
                        }
                        continue;
                    }
                    let outcome = attack::HitOutcome::of(contact);
                    if outcome.registered() {
                        if outcome == attack::HitOutcome::Damaged {
                            pillars[i] = Some(spark.item_spawn(self.stale[i], self.teams[i]));
                        }
                        *slot = None;
                        free_slots += 1;
                    }
                }
                Weapon::PKThunder(h) => {
                    if h.owner_port == defender.port || rules.spares(defender.team, self.teams[i]) {
                        continue;
                    }
                    let mut hit = ness::HEAD_HIT;
                    hit.damage = self.stale[i].damage(h.damage);
                    match pre_hit(
                        defender,
                        wflags(true, false, true, true),
                        false,
                        h.owner_port,
                        self.teams[i],
                        rules,
                        i,
                        hit,
                        h.position,
                        h.velocity,
                    ) {
                        PreHit::None => {}
                        PreHit::Reflected => {
                            // First reflection allocates a new descriptor before ejecting
                            // the old head. Allocation failure still consumes the old one.
                            if !h.reflected && free_slots == 0 {
                                *slot = None;
                                free_slots += 1;
                                continue;
                            }
                            let group = self.next_group;
                            self.next_group = self.next_group.wrapping_add(1);
                            if !h.reflected {
                                // `wpNessPKReflectHeadMakeWeapon`: a new
                                // weapon, last in the link.
                                self.next_seq = self.next_seq.wrapping_add(1);
                                self.seq[i] = self.next_seq;
                            }
                            h.reflect(defender, group);
                            self.teams[i] = defender.team;
                            continue;
                        }
                        // `wpNessPKThunderHeadProcHit` for set-off, a broken
                        // reflector and absorption alike.
                        pre @ (PreHit::SetOff | PreHit::ReflectorBroke | PreHit::Absorbed) => {
                            self.set_off[i] |= pre == PreHit::SetOff;
                            self.fx.push(
                                self.seq[i],
                                Fx::ImpactShock {
                                    pos: h.position,
                                    size: h.damage,
                                },
                            );
                            *slot = None;
                            free_slots += 1;
                            continue;
                        }
                    }
                    let mut hit = ness::HEAD_HIT;
                    hit.damage = h.damage;
                    if stale_hit(
                        &hit,
                        h.position,
                        h.velocity,
                        self.stale[i],
                        defender,
                        &mut self.landed[i],
                        h.owner_port,
                    )
                    .registered()
                    {
                        self.fx.push(
                            self.seq[i],
                            Fx::ImpactShock {
                                pos: h.position,
                                size: h.damage,
                            },
                        );
                        *slot = None;
                        free_slots += 1;
                    }
                }
                _ => {}
            }
        }
        for w in self.slots.iter_mut().flatten() {
            if let Weapon::PKTrail(t) = w {
                if groups.contains(&Some(t.group)) {
                    t.hit_ports |= bit;
                }
            }
        }
        self.clear_pk_trails();
        for spawn in pillars.into_iter().flatten() {
            self.queue_item_spawn(spawn);
        }
    }
    pub fn pk_fires(&self) -> impl Iterator<Item = PKFire> + '_ {
        self.slots.iter().flatten().filter_map(|w| {
            if let Weapon::PKFire(p) = w {
                Some(*p)
            } else {
                None
            }
        })
    }
    pub fn pk_thunders(&self) -> impl Iterator<Item = PKThunder> + '_ {
        self.slots.iter().flatten().filter_map(|w| {
            if let Weapon::PKThunder(p) = w {
                Some(*p)
            } else {
                None
            }
        })
    }
    pub fn pk_trails(&self) -> impl Iterator<Item = PKThunderTrail> + '_ {
        self.slots.iter().flatten().filter_map(|w| {
            if let Weapon::PKTrail(p) = w {
                Some(*p)
            } else {
                None
            }
        })
    }
    /// Live item-made weapons, for the runtime's draw passes.
    pub fn monster_shots(&self) -> impl Iterator<Item = crate::monster_weapon::MonsterShot> + '_ {
        self.slots.iter().flatten().filter_map(|w| match w {
            Weapon::Monster(m) => Some(*m),
            _ => None,
        })
    }

    /// Free weapon structs.
    pub fn free_count(&self) -> usize {
        self.slots.iter().filter(|s| s.is_none()).count()
    }

    /// `wpManagerMakeWeapon` for an item-made weapon
    /// (`WEAPON_FLAG_PARENT_ITEM`): the item's team and staling. Saffron's
    /// flame makes its particles here; the Poké Ball Pokémon's makers made
    /// theirs in the item's update.
    pub fn spawn_monster_shot(&mut self, shot: crate::monster_weapon::MonsterShot) -> bool {
        let seq = self.next_seq;
        let made = self.insert(
            Weapon::Monster(shot),
            crate::stale::WeaponStale::FRESH,
            shot.team,
        );
        if made && shot.kind == ShotKind::HitokageFlame {
            let mut emit = Emit::default();
            shot.make_fx(&mut emit);
            self.fx.extend(seq, &emit);
        }
        made
    }

    /// Onix's rock events since the last call, in order: `(parent handle,
    /// dead)`, where a live rock met a new floor.
    pub fn take_rock_events(&mut self) -> impl Iterator<Item = (u32, bool)> {
        core::mem::replace(&mut self.rock_events, [None; 2 * MAX_WEAPONS])
            .into_iter()
            .flatten()
    }

    fn push_rock_event(&mut self, event: (u32, bool)) {
        if let Some(slot) = self.rock_events.iter_mut().find(|s| s.is_none()) {
            *slot = Some(event);
        }
    }

    /// `wpManagerMakeWeapon` for one of Sector Z's Arwing lasers
    /// (`WEAPON_FLAG_PARENT_GROUND`): default team and staling. Returns
    /// whether a slot was free; the 2D pair's second shot is made only
    /// after the first.
    pub fn spawn_arwing_laser(&mut self, laser: ArwingLaser) -> bool {
        self.insert(
            Weapon::Laser(laser),
            crate::stale::WeaponStale::FRESH,
            sector::GROUND_TEAM,
        )
    }

    /// Records Sector Z's Arwing's roll for its lasers' hops and
    /// reflections. Call it after the stage ticks.
    pub fn observe_arwing_roll(&mut self, roll: f32) {
        self.ground_roll = roll;
    }

    /// The live Arwing lasers, in slot order.
    pub fn arwing_lasers(&self) -> impl Iterator<Item = ArwingLaser> + '_ {
        self.slots.iter().flatten().filter_map(|w| match w {
            Weapon::Laser(l) => Some(*l),
            _ => None,
        })
    }

    /// Consumes a fighter's deferred request. A full pool follows the source
    /// manager's allocation-failure shape: the already-consumed script event
    /// is not retried on a later frame.
    pub fn spawn(&mut self, spawn: WeaponSpawn) -> bool {
        if spawn.kind == WeaponKind::YoshiStars {
            // Two `wpManagerMakeWeapon` calls; each fails on its own.
            let lr = if spawn.facing < 0.0 { -1.0 } else { 1.0 };
            let first = self.insert(
                Weapon::Star(YoshiStar::new(spawn, lr)),
                spawn.stale,
                spawn.team,
            );
            let second = self.insert(
                Weapon::Star(YoshiStar::new(spawn, -lr)),
                spawn.stale,
                spawn.team,
            );
            return first || second;
        }
        let weapon = match spawn.kind {
            WeaponKind::Equipment {
                kind,
                smash,
                angle_index,
            } => {
                if kind == ShotKind::StarRod && smash {
                    self.star_rod_smash_desc = true;
                }
                let parent = crate::monster_weapon::ShotParent {
                    owner: Some(spawn.owner_port),
                    player: Some(spawn.owner_port),
                    team: spawn.team,
                    lr: spawn.facing,
                    handle: u32::MAX,
                };
                let mut m = crate::monster_weapon::MonsterShot::equipment(
                    kind,
                    parent,
                    spawn.position,
                    smash,
                    angle_index,
                );
                if kind == ShotKind::StarRod && self.star_rod_smash_desc {
                    m.damage = 12;
                    m.star_smash = true;
                }
                let made = self.insert_at(Weapon::Monster(m), spawn.stale, spawn.team);
                if let Some(i) = made {
                    let mut emit = Emit::default();
                    m.make_fx(&mut emit);
                    self.fx.extend(self.seq[i], &emit);
                }
                return made.is_some();
            }
            WeaponKind::NessPKFire { grounded } => Weapon::PKFire(PKFire::new(spawn, grounded)),
            WeaponKind::NessPKThunder => {
                let group = self.next_group;
                self.next_group = self.next_group.wrapping_add(1);
                Weapon::PKThunder(PKThunder::new(spawn, group))
            }
            WeaponKind::PikachuThunderJolt => Weapon::Jolt(ThunderJolt::new(spawn)),
            WeaponKind::PikachuThunder => {
                if let Some(flag) = self.thunder_destroyed.get_mut(spawn.owner_port as usize) {
                    *flag = false;
                }
                let group = self.next_group;
                self.next_group = self.next_group.wrapping_add(1);
                Weapon::Thunder(ThunderHead::new(spawn, group))
            }
            WeaponKind::MarioFireball => Weapon::Fireball(MarioFireball::new(spawn, 0)),
            WeaponKind::LuigiFireball => Weapon::Fireball(MarioFireball::new(spawn, 1)),
            WeaponKind::FoxBlaster => {
                let made = self.insert_at(
                    Weapon::Blaster(FoxBlaster::new(spawn)),
                    spawn.stale,
                    spawn.team,
                );
                // `wpFoxBlasterMakeWeapon`.
                if let Some(i) = made {
                    self.fx
                        .push(self.seq[i], Fx::FoxBlasterGlow(spawn.position));
                }
                return made.is_some();
            }
            WeaponKind::SamusChargeShot(charge) => {
                Weapon::ChargeShot(SamusChargeShot::new(spawn, charge))
            }
            WeaponKind::SamusBomb => Weapon::Bomb(SamusBomb::new(spawn)),
            WeaponKind::LinkBoomerang {
                is_smash,
                stick_x,
                stick_y,
            } => Weapon::Boomerang(LinkBoomerang::new(spawn, is_smash, stick_x, stick_y)),
            WeaponKind::YoshiEgg {
                throw_force,
                stick_x,
            } => Weapon::Egg(YoshiEgg::new(spawn, throw_force, stick_x)),
            WeaponKind::YoshiStars => unreachable!("handled above"),
            WeaponKind::KirbyCutter { grounded } => {
                Weapon::Cutter(KirbyCutter::new(spawn, grounded))
            }
        };
        self.insert(weapon, spawn.stale, spawn.team)
    }

    fn insert(&mut self, weapon: Weapon, stale: crate::stale::WeaponStale, team: u8) -> bool {
        self.insert_at(weapon, stale, team).is_some()
    }

    /// `wpManagerMakeWeapon`: the first free slot, placed last in the link.
    fn insert_at(
        &mut self,
        weapon: Weapon,
        stale: crate::stale::WeaponStale,
        team: u8,
    ) -> Option<usize> {
        let i = self.slots.iter().position(|slot| slot.is_none())?;
        self.slots[i] = Some(weapon);
        self.stale[i] = stale;
        self.teams[i] = team;
        self.landed[i] = None;
        self.hit_records[i] = [None; 4];
        self.pending_item_hits[i] = false;
        self.clash_damage[i] = 0;
        self.set_off[i] = false;
        self.rehit[i] = [0; 4];
        self.next_seq = self.next_seq.wrapping_add(1);
        self.seq[i] = self.next_seq;
        // A record naming the slot's last weapon does not name this one.
        let id = weapon_victim_id(i);
        for records in &mut self.hit_records {
            for r in records.iter_mut() {
                if *r == Some(id) {
                    *r = None;
                }
            }
        }
        Some(i)
    }

    /// `ftMainUpdateDamageStatWeapon`'s `ftParamUpdateStaleQueue(wp->player,
    /// ...)`: records this frame's damaging weapon hits in their owner's
    /// queue. Call it for every fighter after [`Self::apply_hits`].
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

    fn queue_item_spawn(&mut self, spawn: crate::item::PKFireSpawn) {
        if let Some(slot) = self.item_spawns.iter_mut().find(|s| s.is_none()) {
            *slot = Some(spawn);
        }
    }

    /// The PK Fire flames made since the last call, in order.
    pub fn take_item_spawns(&mut self) -> impl Iterator<Item = crate::item::PKFireSpawn> {
        core::mem::replace(&mut self.item_spawns, [None; MAX_WEAPONS])
            .into_iter()
            .flatten()
    }

    /// A slot's attack as `itProcessSearchHitWeapon` reads it: owner,
    /// staled hitbox, position and velocity. Pikachu's Thunder and both
    /// thunder trail kinds keep group records that do not name items; they
    /// pass items by.
    /// A slot's attack for the item and clash searches: owner, staled
    /// hitbox, position, velocity and previous position.
    fn item_attack(&self, i: usize) -> Option<(u8, Hitbox, Vec3, Vec3, Vec3)> {
        let stale = self.stale[i];
        let (owner, mut hitbox, pos, vel) = match self.slots[i]? {
            Weapon::Jolt(j) => {
                let (mut hit, pos) = j.hit();
                hit.damage = j.damage;
                (j.owner_port, hit, pos, j.velocity)
            }
            Weapon::Fireball(f) => (
                f.owner_port,
                Hitbox {
                    damage: f.damage,
                    ..MARIO_FIREBALL_HITBOX
                },
                f.position,
                f.velocity,
            ),
            Weapon::Blaster(b) => (
                b.owner_port,
                Hitbox {
                    damage: b.damage,
                    ..FOX_BLASTER_HITBOX
                },
                b.position,
                b.velocity,
            ),
            Weapon::ChargeShot(c) => (c.owner_port, c.hitbox(), c.position, c.velocity),
            Weapon::Bomb(b) => (b.owner_port, b.hitbox(), b.position, b.velocity),
            Weapon::Boomerang(b) => (b.owner_port, b.hitbox(), b.position, b.velocity),
            Weapon::Egg(e) => (e.owner_port, e.hitbox(), e.position, e.velocity),
            Weapon::Star(s) => (s.owner_port, s.hitbox(), s.position, s.velocity),
            Weapon::Cutter(c) => (
                c.owner_port,
                Hitbox {
                    damage: c.damage,
                    ..KIRBY_CUTTER_HITBOX
                },
                c.position,
                c.velocity,
            ),
            Weapon::PKFire(p) => (
                p.owner_port,
                Hitbox {
                    damage: p.damage,
                    ..ness::SPARK_HIT
                },
                p.position,
                p.velocity,
            ),
            Weapon::PKThunder(h) => (
                h.owner_port,
                Hitbox {
                    damage: h.damage,
                    ..ness::HEAD_HIT
                },
                h.position,
                h.velocity,
            ),
            Weapon::Laser(l) => (l.owner_port, l.hitbox(), l.position, l.velocity),
            Weapon::Monster(m) => {
                let (curr, prev) = m.attack_positions();
                let mut hitbox = m.hitbox();
                hitbox.damage = stale.damage(hitbox.damage);
                return Some((
                    m.owner.unwrap_or(sector::GROUND_PORT),
                    hitbox,
                    curr,
                    m.velocity,
                    prev,
                ));
            }
            Weapon::Thunder(_) | Weapon::Trail(_) | Weapon::PKTrail(_) => return None,
        };
        hitbox.damage = stale.damage(hitbox.damage);
        Some((owner, hitbox, pos, vel, pos - vel))
    }

    /// `itProcessSearchHitWeapon` for one item (`id` is its record id):
    /// every weapon that has not recorded it tests its damage box. A contact
    /// queues the weapon's staled damage on the item and defers the weapon's
    /// `proc_hit` until all item searches finish. Attack clashes precede
    /// the hurtbox search and queue `proc_setoff` for `finish_clashes`.
    pub fn hit_item(&mut self, item: &mut crate::item::Item, id: u8) {
        use crate::item::INTERACT_WEAPON;
        if item.damage_coll.interact_mask & INTERACT_WEAPON == 0 {
            return;
        }
        let (order, count) = self.link_order();
        for &i in &order[..count] {
            let Some((owner, hitbox, pos, vel, prev)) = self.item_attack(i) else {
                continue;
            };
            if item.owner == Some(owner) && !item.is_damage_all {
                continue;
            }
            if self.team_rules.spares(item.team, self.teams[i]) && !item.is_damage_all {
                continue;
            }
            if self.hit_records[i].contains(&Some(id)) {
                continue;
            }
            if item.attack.can_setoff
                && item.attack.state != crate::combat::AttackState::Off
                && item.attack.interact_mask & INTERACT_WEAPON != 0
                && item.owner != Some(owner)
                && !self.team_rules.spares(item.team, self.teams[i])
                && item.attack.record(weapon_victim_id(i)).is_clear()
            {
                if let Some(w) = self.clash_attack(i) {
                    for j in 0..item.attack.count {
                        let p = item.attack.pos[j];
                        if !crate::hurtbox::attacks_collide(
                            (w.pos_curr, w.pos_prev, w.size, w.state),
                            (p.pos_curr, p.pos_prev, item.attack.size, item.attack.state),
                        ) {
                            continue;
                        }
                        let impact = crate::combat::impact_point(
                            crate::combat::attack_point(w.pos_curr, w.pos_prev, w.state),
                            crate::combat::attack_point(p.pos_curr, p.pos_prev, item.attack.state),
                        );
                        if item.attack.priority <= w.priority {
                            let damage = item.damage_output();
                            item.attack.set_hit_interact(
                                weapon_victim_id(i),
                                crate::item::HitType::Attack(0),
                            );
                            item.hit_attack_damage = item.hit_attack_damage.max(damage);
                            self.clash_fx.push(
                                0,
                                Fx::SetOff {
                                    pos: impact,
                                    size: damage,
                                },
                            );
                        }
                        if w.priority <= item.attack.priority {
                            record_weapon_victim(&mut self.hit_records[i], id);
                            self.clash_damage[i] = self.clash_damage[i].max(w.damage);
                            self.clash_fx.push(
                                0,
                                Fx::SetOff {
                                    pos: impact,
                                    size: w.damage,
                                },
                            );
                        }
                        if self.clash_damage[i] != 0 || item.hit_attack_damage != 0 {
                            break;
                        }
                    }
                    if self.clash_damage[i] != 0 {
                        continue;
                    }
                }
            }
            match item.damage_coll.hitstatus {
                crate::combat::HitStatus::None | crate::combat::HitStatus::Intangible => continue,
                _ => {}
            }
            let state = if prev == pos {
                crate::combat::AttackState::Transfer
            } else {
                crate::combat::AttackState::Interpolate
            };
            let tail_hit = matches!(self.slots[i], Some(Weapon::Monster(m)) if m.kind == ShotKind::RayGun && m.attack_tail.is_some_and(|(tail, old)| crate::item::touches_damage_coll(item, tail, old, hitbox.radius, state)));
            if !crate::item::touches_damage_coll(item, pos, prev, hitbox.radius, state) && !tail_hit
            {
                continue;
            }
            // `itProcessUpdateDamageStatWeapon`.
            record_weapon_victim(&mut self.hit_records[i], id);
            if matches!(self.slots[i], Some(Weapon::Monster(m)) if m.attributes().can_rehit_item) {
                if let Some(r) = self.hit_records[i].iter().position(|r| *r == Some(id)) {
                    self.rehit[i][r] = 16;
                }
            }
            let lr = if vel.x.abs() < 5.0 {
                if item.pos.x < pos.x {
                    1.0
                } else {
                    -1.0
                }
            } else if vel.x < 0.0 {
                1.0
            } else {
                -1.0
            };
            crate::item::queue_damage(
                item,
                hitbox.damage,
                hitbox.angle,
                hitbox.element,
                lr,
                crate::item::Attacker {
                    owner: Some(owner),
                    team: self.teams[i],
                    // A ground weapon's `wp->player` names no fighter.
                    player: (owner != sector::GROUND_PORT).then_some(owner),
                    handicap: crate::stale::HANDICAP_DEFAULT,
                },
                crate::item::Knock {
                    weight: hitbox.kb_weight,
                    scale: hitbox.kb_scale,
                    base: hitbox.kb_base,
                },
            );
            self.pending_item_hits[i] = true;
        }
    }

    /// `wpProcessProcHitCollisions` for normal hits queued by item searches.
    /// Call once after every item's damage collision has been searched.
    pub fn finish_item_hits(&mut self) {
        let pending = core::mem::replace(&mut self.pending_item_hits, [false; MAX_WEAPONS]);
        for (i, hit) in pending.into_iter().enumerate() {
            if hit {
                self.weapon_proc_hit(i);
            }
        }
    }

    /// A weapon's `proc_hit` after its `hit_normal_damage` against an item,
    /// matching its reaction to a fighter's hurtbox.
    fn weapon_proc_hit(&mut self, i: usize) {
        let stale = self.stale[i];
        let slot = &mut self.slots[i];
        let Some(weapon) = slot else { return };
        push_proc(&mut self.fx, self.seq[i], weapon, Proc::Hit);
        match weapon {
            Weapon::Boomerang(b) => b.on_hit(),
            Weapon::Cutter(_) => {}
            Weapon::Egg(e) => {
                if !e.exploded {
                    e.explode();
                }
            }
            Weapon::Bomb(b) => {
                if !b.exploded {
                    b.explode();
                }
            }
            Weapon::PKFire(p) => {
                let spawn = p.item_spawn(stale, self.teams[i]);
                *slot = None;
                self.queue_item_spawn(spawn);
            }
            // `grSectorArwingWeaponLaser3DProcHit`; the burst has none.
            Weapon::Laser(l) if l.three_d => {
                if !l.exploded {
                    l.explode();
                    self.hit_records[i] = [None; 4];
                }
            }
            Weapon::Monster(m) if m.survives(ShotProc::Hit) => {}
            _ => *slot = None,
        }
    }

    /// Records what this frame's weapons may read about a fighter: the
    /// Boomerang homes on its thrower's TopN. Call it for every fighter after
    /// the fighters tick and before [`Self::tick`].
    pub fn observe_owner(&mut self, f: &Fighter) {
        if let Some(slot) = self.owners.get_mut(usize::from(f.port)) {
            *slot = Some(OwnerView {
                position: f.pos,
                is_special_interrupt: f.is_special_interrupt,
                thunder_collide: f.pikachu.thunder_collide,
                thunder_damage: f.pikachu.thunder_damage,
                thunder_motion: f.pikachu.thunder_motion,
                ness_control: crate::ness::thunder_controlling(f.status.status),
                ness_collide: f.ness.thunder_collide,
                ness_motion: f.ness.thunder_motion,
                stick: f.stick,
            });
        }
    }

    /// `ftManagerDestroyFighterWeapons`, as a fighter dies: only Link's
    /// Boomerang (`boomerang_gobj`, the one it homes back to) goes.
    pub fn destroy_boomerang(&mut self, port: u8) {
        for slot in self.slots.iter_mut() {
            if matches!(slot, Some(Weapon::Boomerang(b)) if b.parent_port == Some(port)) {
                *slot = None;
            }
        }
    }

    /// Records the battle camera the Boomerang's off-camera check projects
    /// through. `gGMCameraMatrix` is built when the camera draws, so call it
    /// before the camera advances this frame.
    pub fn observe_camera(&mut self, camera: &crate::camera::Camera) {
        self.camera = Some(*camera);
    }

    /// Delivers the pool's writes to a fighter after [`Self::tick`]:
    /// `wpLinkBoomerangCheckOwnerCatch`'s catch status, and
    /// `wpLinkBoomerangClearGObjs`, which clears the thrower's
    /// `boomerang_gobj` when the Boomerang goes away (or was never made
    /// because the pool was full).
    pub fn sync_owner(&mut self, f: &mut Fighter) {
        if crate::ness::thunder_controlling(f.status.status) {
            f.ness.thunder_position = self.slots.iter().flatten().find_map(|w| match w {
                Weapon::PKThunder(h)
                    if !h.reflected
                        && h.owner_port == f.port
                        && h.motion_count == f.ness.thunder_motion =>
                {
                    Some(h.position)
                }
                _ => None,
            });
            if f.ness.thunder_position.is_none() {
                f.ness.thunder_destroyed = true;
            }
        }
        let port = usize::from(f.port);
        if crate::pikachu::thunder_controlling(f.status.status) {
            f.pikachu.thunder_position = self.slots.iter().flatten().find_map(|w| match w {
                Weapon::Thunder(h)
                    if h.owner_port == f.port && h.motion_count == f.pikachu.thunder_motion =>
                {
                    Some(h.position)
                }
                _ => None,
            });
            if port < MAX_OWNERS && core::mem::take(&mut self.thunder_destroyed[port])
                || (f.pikachu.thunder_position.is_none() && !f.pikachu.thunder_collide)
            {
                f.pikachu.thunder_destroyed = true;
            }
        }
        if port < MAX_OWNERS && core::mem::take(&mut self.caught[port]) {
            // `wpLinkBoomerangCheckOwnerCatch`'s `fkind` check.
            if crate::kirby::is_kirby(f.kind) {
                crate::kirby_copy::set_boomerang_get(f);
            } else {
                crate::link::set_special_n_get(f);
            }
        }
        f.link.boomerang_out = self
            .slots
            .iter()
            .flatten()
            .any(|w| matches!(w, Weapon::Boomerang(b) if b.parent_port == Some(f.port)));
    }

    /// Advances each weapon's source physics and map callback once, then
    /// deletes a weapon outside `bounds` (`MPGroundData.map_bound_*`). Call
    /// this once per match frame, before [`Self::apply_hits`].
    ///
    /// `wpProcessProcWeaponMain` tests the bounds after the move and before
    /// `proc_map`; this tests the position the map callback left. A map
    /// contact only moves a weapon onto a stage surface, and every surface
    /// lies inside the blast zone, so the two orders delete the same
    /// weapons. Every `proc_dead` here returns TRUE: the Egg is a weapon
    /// only once thrown, the Charge Shot's owner link is always NULL, and
    /// the Boomerang's and PK Thunder's owner links are rederived from the
    /// pool.
    pub fn tick<I, F>(&mut self, surfaces: F, bounds: Option<BlastZone>)
    where
        F: Fn() -> I + Copy,
        I: IntoIterator<Item = MapSurface>,
    {
        let owners = self.owners;
        let camera = self.camera;
        self.tick_pk_thunder(surfaces, bounds);
        let mut trails = [None; MAX_WEAPONS];
        let mut rocks: [Option<(u32, bool)>; MAX_WEAPONS] = [None; MAX_WEAPONS];
        for (i, slot) in self.slots.iter_mut().enumerate() {
            if let Some(weapon) = slot.as_mut() {
                let mut emit = Emit::default();
                let fx = &mut emit;
                let alive = match weapon {
                    Weapon::PKFire(spark) => spark.tick(surfaces, fx),
                    Weapon::PKThunder(_) | Weapon::PKTrail(_) => true, // ticked together above
                    Weapon::Jolt(jolt) => jolt.tick(surfaces, fx),
                    Weapon::Trail(trail) => trail.tick(fx),
                    Weapon::Thunder(head) => {
                        let owner = owners.get(head.owner_port as usize).copied().flatten();
                        if owner.is_some_and(|o| {
                            o.thunder_damage && o.thunder_motion == head.motion_count
                        }) {
                            head.notify_destroy = false;
                        }
                        if owner.is_some_and(|o| {
                            o.thunder_collide && o.thunder_motion == head.motion_count
                        }) {
                            // `wpPikachuThunderHeadProcUpdate`'s collide
                            // branch: the last segment.
                            fx.push(head.trail_effect(10, 3));
                            false
                        } else {
                            // ProcUpdate makes a stationary trail before head physics/map.
                            if head.lifetime > 1 {
                                trails[i] =
                                    Some((ThunderTrail::new(*head), self.stale[i], self.teams[i]));
                            }
                            // `wpPikachuThunderHeadProcDead` notifies
                            // the owner like an expired head.
                            let alive = head.tick(surfaces, fx)
                                && !bounds.is_some_and(|b| out_of_bounds(b, head.position));
                            if !alive && head.notify_destroy {
                                if let Some(flag) =
                                    self.thunder_destroyed.get_mut(head.owner_port as usize)
                                {
                                    *flag = true;
                                }
                            }
                            alive
                        }
                    }
                    Weapon::Fireball(fireball) => fireball.tick(surfaces, fx),
                    Weapon::Blaster(blaster) => blaster.tick(surfaces, fx),
                    Weapon::ChargeShot(shot) => shot.tick(surfaces, fx),
                    Weapon::Bomb(bomb) => bomb.tick(surfaces, fx),
                    Weapon::Boomerang(boomerang) => {
                        let parent = boomerang
                            .parent_port
                            .and_then(|port| owners.get(usize::from(port)).copied().flatten());
                        let (alive, caught) = boomerang.tick(surfaces, parent, camera.as_ref(), fx);
                        if caught {
                            if let Some(port) = boomerang.parent_port {
                                self.caught[usize::from(port)] = true;
                            }
                        }
                        alive
                    }
                    Weapon::Egg(egg) => egg.tick(surfaces, fx),
                    Weapon::Star(star) => star.tick(fx),
                    Weapon::Monster(m) => {
                        let alive = m.tick(surfaces, bounds, fx);
                        if m.kind == ShotKind::IwarkRock {
                            if core::mem::take(&mut m.rumbled) {
                                rocks[i] = Some((m.parent, false));
                            }
                            if !alive {
                                rocks[i] = Some((m.parent, true));
                            }
                        }
                        alive
                    }
                    Weapon::Cutter(cutter) => cutter.tick(surfaces, fx),
                    Weapon::Laser(laser) => {
                        laser.roll = self.ground_roll;
                        let (alive, burst) = laser.tick(surfaces, fx);
                        // `wpMainClearAttackRecord`.
                        if burst {
                            self.hit_records[i] = [None; 4];
                        }
                        alive
                    }
                };
                self.fx.extend(self.seq[i], &emit);
                let alive = alive
                    && (matches!(weapon, Weapon::Thunder(_))
                        || !bounds.is_some_and(|b| out_of_bounds(b, weapon.position())));
                if !alive {
                    *slot = None;
                } else if matches!(weapon, Weapon::Monster(_)) {
                    // `wpProcessUpdateAttackRecords`: a rehit record clears
                    // when its timer runs out.
                    for (r, t) in self.hit_records[i].iter_mut().zip(&mut self.rehit[i]) {
                        if r.is_some() && *t > 0 {
                            *t -= 1;
                            if *t == 0 {
                                *r = None;
                            }
                        }
                    }
                }
            }
        }
        for event in rocks.into_iter().flatten() {
            self.push_rock_event(event);
        }
        for (trail, stale, team) in trails.into_iter().flatten() {
            self.insert(Weapon::Trail(trail), stale, team);
        }
    }

    /// Resolves every eligible weapon against one fighter. Fireball and
    /// Blaster both delete on registered contact; invincibility leaves the
    /// shot live, exactly like a non-registered source hitbox.
    pub fn apply_hits(&mut self, defender: &mut Fighter) {
        // `ftMainProcSearchHitAll` skips a ghost.
        if defender.dead.is_ghost {
            return;
        }
        self.apply_pk_hits(defender);
        let rules = self.team_rules;
        let mut thunder_groups = [None; MAX_WEAPONS];
        for (i, slot) in self.slots.iter_mut().enumerate() {
            let Some(weapon) = slot else { continue };
            let records = &mut self.hit_records[i];
            // An item may have evicted a fighter's record since the last
            // search. Keep the kind's public port mask in sync with the
            // same four records the item search reads.
            let ports = recorded_fighter_ports(records);
            match weapon {
                Weapon::Boomerang(b) => b.hit_ports = ports,
                Weapon::Cutter(c) => c.hit_ports = ports,
                Weapon::Egg(e) => e.hit_ports = ports,
                Weapon::Bomb(b) => b.hit_ports = ports,
                _ => {}
            }
            if matches!(
                weapon,
                Weapon::PKFire(_) | Weapon::PKThunder(_) | Weapon::PKTrail(_)
            ) {
                continue;
            }
            if let Weapon::Thunder(_) = weapon {
                continue;
            }
            if let Weapon::Trail(t) = weapon {
                let bit = 1u8 << (defender.port & 7);
                if t.owner_port == defender.port
                    || rules.spares(defender.team, self.teams[i])
                    || t.hit_ports & bit != 0
                    || thunder_groups.contains(&Some(t.group))
                {
                    continue;
                }
                // Trail DObj scale is 0.5; wpProcessUpdateHitOffsets scales
                // the two authored ±240 offsets, while radius stays 200.
                for y in [120.0, -120.0] {
                    if stale_hit(
                        &pikachu::TRAIL_HIT,
                        t.position + Vec3::new(0.0, y, 0.0),
                        Vec3::ZERO,
                        self.stale[i],
                        defender,
                        &mut self.landed[i],
                        t.owner_port,
                    )
                    .registered()
                    {
                        t.hit_ports |= bit;
                        thunder_groups[i] = Some(t.group);
                        // `wpPikachuThunderTrailProcHit`.
                        self.fx.push(
                            self.seq[i],
                            Fx::ImpactShock {
                                pos: t.position,
                                size: pikachu::TRAIL_HIT.damage,
                            },
                        );
                        break;
                    }
                }
                continue;
            }
            if let Weapon::Monster(m) = weapon {
                let alive = monster_hit(
                    m,
                    MonsterHit {
                        records,
                        rehit: &mut self.rehit[i],
                        ports,
                        stale: self.stale[i],
                        team: &mut self.teams[i],
                        fx: &mut self.fx,
                        seq: self.seq[i],
                        rules,
                        slot: i,
                        set_off: &mut self.set_off[i],
                        landed: &mut self.landed[i],
                    },
                    defender,
                );
                if !alive {
                    *slot = None;
                }
                continue;
            }
            if let Weapon::Laser(laser) = weapon {
                let alive = laser_hit(
                    laser,
                    LaserHit {
                        records,
                        ports,
                        stale: self.stale[i],
                        team: &mut self.teams[i],
                        landed: &mut self.landed[i],
                        fx: &mut self.fx,
                        seq: self.seq[i],
                        rules,
                        slot: i,
                    },
                    defender,
                );
                if !alive {
                    *slot = None;
                }
                continue;
            }
            let (owner, mut hitbox, position, velocity) = match weapon {
                Weapon::Jolt(j) => {
                    let (hit, pos) = j.hit();
                    (j.owner_port, hit, pos, j.velocity)
                }
                Weapon::PKFire(_) | Weapon::PKThunder(_) | Weapon::PKTrail(_) => {
                    unreachable!("handled separately")
                }
                Weapon::Thunder(_) | Weapon::Trail(_) => unreachable!("handled above"),
                Weapon::Fireball(f) => {
                    (f.owner_port, MARIO_FIREBALL_HITBOX, f.position, f.velocity)
                }
                Weapon::Blaster(b) => (b.owner_port, FOX_BLASTER_HITBOX, b.position, b.velocity),
                Weapon::ChargeShot(c) => (c.owner_port, c.hitbox(), c.position, c.velocity),
                Weapon::Bomb(b) => (b.owner_port, b.hitbox(), b.position, b.velocity),
                Weapon::Boomerang(b) => (b.owner_port, b.hitbox(), b.position, b.velocity),
                Weapon::Egg(e) => (e.owner_port, e.hitbox(), e.position, e.velocity),
                Weapon::Star(s) => (s.owner_port, s.hitbox(), s.position, s.velocity),
                Weapon::Cutter(c) => (c.owner_port, KIRBY_CUTTER_HITBOX, c.position, c.velocity),
                Weapon::Monster(_) | Weapon::Laser(_) => unreachable!("handled above"),
            };
            // `ftMainSearchHitWeapon`: not its owner, nor, with team attack
            // off, the owner's teammates.
            if owner == defender.port || rules.spares(defender.team, self.teams[i]) {
                continue;
            }
            let bit = 1u8 << (defender.port & 7);
            // The weapon's attack record: once it has met this fighter in any
            // way, it passes through. A hopped shot keeps its shield record.
            if ports & bit != 0 {
                continue;
            }
            hitbox.damage = match weapon {
                Weapon::Jolt(j) => j.damage,
                Weapon::PKFire(_) | Weapon::PKThunder(_) | Weapon::PKTrail(_) => {
                    unreachable!("handled separately")
                }
                Weapon::Thunder(_) | Weapon::Trail(_) => unreachable!("handled above"),
                Weapon::Fireball(f) => f.damage,
                Weapon::Blaster(b) => b.damage,
                Weapon::ChargeShot(c) => c.damage,
                Weapon::Bomb(_) => hitbox.damage,
                Weapon::Boomerang(b) => b.damage,
                Weapon::Egg(e) => e.damage,
                Weapon::Star(s) => s.damage,
                Weapon::Cutter(c) => c.damage,
                Weapon::Monster(_) | Weapon::Laser(_) => unreachable!("handled above"),
            };
            // The exploding egg keeps the record of what the egg hit and is
            // only a hurtbox test now.
            if let Weapon::Egg(egg) = weapon {
                if egg.exploded {
                    if stale_hit(
                        &hitbox,
                        position,
                        velocity,
                        self.stale[i],
                        defender,
                        &mut self.landed[i],
                        owner,
                    )
                    .registered()
                    {
                        record_weapon_victim(records, defender.port);
                        egg.hit_ports |= bit;
                    }
                    continue;
                }
            }
            // The Bomb can neither clank, be reflected nor be absorbed, and
            // its attack record outlives the explosion.
            if let Weapon::Bomb(bomb) = weapon {
                let contact = stale_contact(
                    &hitbox,
                    position,
                    velocity,
                    self.stale[i],
                    defender,
                    &mut self.landed[i],
                    owner,
                    None,
                );
                if let crate::combat::WeaponContact::Shielded(shield) = contact {
                    record_weapon_victim(records, defender.port);
                    let mut emit = Emit::default();
                    weapon.on_shield(shield, &mut emit);
                    self.fx.extend(self.seq[i], &emit);
                    continue;
                }
                if attack::HitOutcome::of(contact).registered() {
                    record_weapon_victim(records, defender.port);
                    bomb.hit_ports |= bit;
                    if !bomb.exploded {
                        // `wpSamusBombProcHit`.
                        self.fx
                            .push(self.seq[i], Fx::SparkleWhiteMultiExplode(bomb.position));
                        bomb.explode();
                    }
                }
                continue;
            }
            let mut staled = hitbox;
            staled.damage = self.stale[i].damage(hitbox.damage);
            let pre = pre_hit(
                defender,
                weapon.flags(),
                weapon.is_grounded(),
                owner,
                self.teams[i],
                rules,
                i,
                staled,
                position,
                velocity,
            );
            match pre {
                PreHit::None => {}
                PreHit::SetOff => push_proc(&mut self.fx, self.seq[i], weapon, Proc::SetOff),
                PreHit::ReflectorBroke => push_proc(&mut self.fx, self.seq[i], weapon, Proc::Hit),
                PreHit::Absorbed => push_proc(&mut self.fx, self.seq[i], weapon, Proc::Absorb),
                PreHit::Reflected => {}
            }
            match pre {
                PreHit::None => {}
                // `proc_setoff`.
                PreHit::SetOff => {
                    self.set_off[i] = true;
                    match weapon {
                        // `wpLinkBoomerangProcSetOff`.
                        Weapon::Boomerang(b) => {
                            record_weapon_victim(records, defender.port);
                            b.hit_ports |= bit;
                            b.set_off();
                        }
                        // `wpYoshiEggThrowProcHit`: it explodes in place.
                        Weapon::Egg(e) => {
                            record_weapon_victim(records, defender.port);
                            e.hit_ports |= bit;
                            e.explode();
                        }
                        // `wpYoshiStarProcHit` without `hit_normal_damage`.
                        Weapon::Star(_) => {}
                        // `wpKirbyCutterProcSetOff` and every `ProcHit` that
                        // returns TRUE.
                        _ => *slot = None,
                    }
                    continue;
                }
                PreHit::Reflected => {
                    match weapon {
                        Weapon::Fireball(f) => {
                            reflect_shot(
                                &mut f.velocity,
                                &mut f.owner_port,
                                &mut f.damage,
                                defender,
                            );
                            f.lifetime = f.attributes().lifetime;
                        }
                        Weapon::Blaster(b) => {
                            reflect_shot(
                                &mut b.velocity,
                                &mut b.owner_port,
                                &mut b.damage,
                                defender,
                            );
                            b.scale_x = 1.0;
                        }
                        Weapon::ChargeShot(c) => {
                            reflect_shot(
                                &mut c.velocity,
                                &mut c.owner_port,
                                &mut c.damage,
                                defender,
                            );
                        }
                        Weapon::Boomerang(b) => b.reflect(defender),
                        Weapon::Egg(e) => e.reflect(defender),
                        Weapon::Star(s) => s.reflect(defender),
                        Weapon::Cutter(c) => c.reflect(defender),
                        Weapon::Jolt(j) => j.reflect(defender),
                        _ => unreachable!("not reflectable"),
                    }
                    // `wpProcessProcHitCollisions`: the reflector's team.
                    self.teams[i] = defender.team;
                    continue;
                }
                // `hit_normal_damage`: the weapon's `proc_hit`.
                PreHit::ReflectorBroke => {
                    match weapon {
                        Weapon::Boomerang(b) => {
                            record_weapon_victim(records, defender.port);
                            b.hit_ports |= bit;
                            b.on_hit();
                        }
                        Weapon::Cutter(c) => {
                            record_weapon_victim(records, defender.port);
                            c.hit_ports |= bit;
                        }
                        Weapon::Egg(e) => {
                            record_weapon_victim(records, defender.port);
                            e.hit_ports |= bit;
                            e.explode();
                        }
                        _ => *slot = None,
                    }
                    continue;
                }
                // `proc_absorb`: Cutter survives.
                PreHit::Absorbed => {
                    match weapon {
                        Weapon::Cutter(c) => {
                            record_weapon_victim(records, defender.port);
                            c.hit_ports |= bit;
                        }
                        _ => *slot = None,
                    }
                    continue;
                }
            }
            let contact = stale_contact(
                &hitbox,
                position,
                velocity,
                self.stale[i],
                defender,
                &mut self.landed[i],
                owner,
                // `wpLinkBoomerangMakeWeapon` sets `is_hitlag_victim`.
                matches!(weapon, Weapon::Boomerang(_)).then_some(owner),
            );
            // `ftMainUpdateShieldStatWeapon` records the fighter; then
            // `wpProcessProcHitCollisions` hops the weapon or runs its
            // `proc_shield`.
            if let crate::combat::WeaponContact::Shielded(shield) = contact {
                record_weapon_victim(records, defender.port);
                let mut emit = Emit::default();
                let alive = weapon.on_shield(shield, &mut emit);
                self.fx.extend(self.seq[i], &emit);
                if !alive {
                    *slot = None;
                }
                continue;
            }
            if attack::HitOutcome::of(contact).registered() {
                // `proc_hit`, which makes its effects first.
                push_proc(&mut self.fx, self.seq[i], weapon, Proc::Hit);
                // The Boomerang survives a hit and turns back.
                if let Weapon::Boomerang(b) = weapon {
                    record_weapon_victim(records, defender.port);
                    b.hit_ports |= bit;
                    b.on_hit();
                    continue;
                }
                // `wpKirbyCutterProcHit` returns FALSE: the wave carries on.
                if let Weapon::Cutter(c) = weapon {
                    record_weapon_victim(records, defender.port);
                    c.hit_ports |= bit;
                    continue;
                }
                // `wpYoshiEggThrowProcHit`: the egg explodes in place.
                if let Weapon::Egg(e) = weapon {
                    record_weapon_victim(records, defender.port);
                    e.hit_ports |= bit;
                    e.explode();
                    continue;
                }
                *slot = None;
            }
        }
        for w in self.slots.iter_mut().flatten() {
            if let Weapon::Trail(t) = w {
                if thunder_groups.contains(&Some(t.group)) {
                    t.hit_ports |= 1u8 << (defender.port & 7);
                }
            }
            if let Weapon::Thunder(h) = w {
                if thunder_groups.contains(&Some(h.group)) {
                    h.hit_ports |= 1u8 << (defender.port & 7);
                }
            }
        }
    }

    /// The live slots in link order.
    fn link_order(&self) -> ([usize; MAX_WEAPONS], usize) {
        let mut order = [0usize; MAX_WEAPONS];
        let mut n = 0;
        for (i, slot) in self.slots.iter().enumerate() {
            if slot.is_some() {
                order[n] = i;
                n += 1;
            }
        }
        order[..n].sort_unstable_by_key(|&i| self.seq[i]);
        (order, n)
    }

    /// A slot's attack as `wpProcessProcSearchHitWeapon` reads it, or
    /// `None` when it cannot clash (`can_setoff` clear, or no attack):
    /// owner, staled damage, priority and its box
    /// `(pos_curr, pos_prev, size, state)`.
    fn clash_attack(&self, i: usize) -> Option<ClashAttack> {
        let w = self.slots[i]?;
        if !w.flags().can_setoff {
            return None;
        }
        let (owner, hitbox, pos, _vel, prev) = self.item_attack(i)?;
        Some(ClashAttack {
            owner,
            team: self.teams[i],
            damage: hitbox.damage,
            priority: w.priority(),
            pos_curr: pos,
            pos_prev: prev,
            size: hitbox.radius,
            state: crate::combat::weapon_state(pos, prev),
        })
    }

    /// `wpProcessUpdateHitInteractStats(.., nGMHitTypeAttack, 0)`: slot `i`
    /// (and every weapon of its group) records weapon `victim`.
    fn record_clash(&mut self, i: usize, victim: usize) {
        let id = weapon_victim_id(victim);
        match self.slots[i].and_then(|w| w.group()) {
            Some(group) => {
                for j in 0..MAX_WEAPONS {
                    if self.slots[j].and_then(|w| w.group()) == Some(group) {
                        record_weapon_victim(&mut self.hit_records[j], id);
                    }
                }
            }
            None => record_weapon_victim(&mut self.hit_records[i], id),
        }
    }

    /// `wpProcessProcSearchHitWeapon` for every weapon, in link order (the
    /// priority-1 process after the fighters' and items' searches). Each
    /// weapon tests only the weapons after it in the link that are not its
    /// owner's or, with team attack off, its team's, and that neither side
    /// has recorded; the first pair of boxes that meet clashes
    /// (`wpProcessUpdateAttackStatWeapon`): a side whose priority is not
    /// above the other's records it, takes its staled damage as
    /// `hit_attack_damage` and makes a set-off at the pair's midpoint, the
    /// searched weapon's side first.
    ///
    /// Call it once a frame after [`Self::tick`] and before
    /// [`Self::apply_hits`]: the source's weapons are still whole here (their
    /// fighter hits land at priority 0), while the port's hit searches react
    /// at once. The set-offs wait in a queue
    /// ([`Self::flush_clash_effects`]) and the reactions for
    /// [`Self::finish_clashes`].
    pub fn search_weapons(&mut self) {
        self.set_off = [false; MAX_WEAPONS];
        self.clash_damage = [0; MAX_WEAPONS];
        let (order, n) = self.link_order();
        for a in 0..n {
            let this = order[a];
            let Some(this_attack) = self.clash_attack(this) else {
                continue;
            };
            for &other in &order[a + 1..n] {
                let Some(other_attack) = self.clash_attack(other) else {
                    continue;
                };
                if this_attack.owner == other_attack.owner
                    || self.team_rules.spares(this_attack.team, other_attack.team)
                {
                    continue;
                }
                if self.hit_records[other].contains(&Some(weapon_victim_id(this)))
                    || self.hit_records[this].contains(&Some(weapon_victim_id(other)))
                {
                    continue;
                }
                // `gmCollisionCheckWeaponAttacksCollide(other_hit, i,
                // this_attack_coll, j)`.
                if !crate::hurtbox::attacks_collide(
                    (
                        other_attack.pos_curr,
                        other_attack.pos_prev,
                        other_attack.size,
                        other_attack.state,
                    ),
                    (
                        this_attack.pos_curr,
                        this_attack.pos_prev,
                        this_attack.size,
                        this_attack.state,
                    ),
                ) {
                    continue;
                }
                self.clash(other, &other_attack, this, &this_attack);
            }
        }
    }

    /// `wpProcessUpdateAttackStatWeapon(other, .., victim, ..)`.
    fn clash(&mut self, other: usize, o: &ClashAttack, victim: usize, v: &ClashAttack) {
        let pos = crate::combat::impact_point(
            crate::combat::attack_point(v.pos_curr, v.pos_prev, v.state),
            crate::combat::attack_point(o.pos_curr, o.pos_prev, o.state),
        );
        if v.priority <= o.priority {
            self.record_clash(victim, other);
            self.clash_damage[victim] = self.clash_damage[victim].max(v.damage);
            self.clash_fx.push(
                0,
                Fx::SetOff {
                    pos,
                    size: v.damage,
                },
            );
        }
        if o.priority <= v.priority {
            self.record_clash(other, victim);
            self.clash_damage[other] = self.clash_damage[other].max(o.damage);
            self.clash_fx.push(
                0,
                Fx::SetOff {
                    pos,
                    size: o.damage,
                },
            );
        }
    }

    /// `wpProcessProcHitCollisions`'s `hit_attack_damage` branch for the
    /// clashes: each weapon's `proc_setoff`, in link order, unless a
    /// fighter's attack already set it off this frame. Call it after
    /// [`Self::apply_hits`] and the item searches (their `proc_hit`s come
    /// first in the source).
    pub fn finish_clashes(&mut self) {
        let (order, n) = self.link_order();
        let mut thunder = false;
        for &i in &order[..n] {
            let damage = core::mem::take(&mut self.clash_damage[i]);
            if damage == 0 || self.set_off[i] {
                continue;
            }
            let seq = self.seq[i];
            let stale = self.stale[i];
            let team = self.teams[i];
            let Some(weapon) = self.slots[i].as_mut() else {
                continue;
            };
            push_proc(&mut self.fx, seq, weapon, Proc::SetOff);
            let alive = match weapon {
                // `wpLinkBoomerangProcSetOff`.
                Weapon::Boomerang(b) => {
                    b.set_off();
                    true
                }
                // `wpYoshiEggThrowProcHit`; the explosion has none.
                Weapon::Egg(e) => {
                    if !e.exploded {
                        e.explode();
                    }
                    true
                }
                // `wpYoshiStarProcHit` without `hit_normal_damage`.
                Weapon::Star(_) => true,
                Weapon::Monster(m) => m.survives(ShotProc::SetOff),
                // `wpNessPKFireProcHit`: the pillar.
                Weapon::PKFire(p) => {
                    let spawn = p.item_spawn(stale, team);
                    self.queue_item_spawn(spawn);
                    false
                }
                Weapon::PKThunder(_) => {
                    thunder = true;
                    false
                }
                // The Fireball's, Charge Shot's, Final Cutter's and Thunder
                // Jolt's return TRUE.
                _ => false,
            };
            if !alive {
                self.slots[i] = None;
            }
        }
        if thunder {
            self.clear_pk_trails();
        }
    }

    /// Makes the weapons' queued effects ([`crate::wpeffect`]) in link
    /// order. Call it at the end of each weapon pass: after a fighter's
    /// weapon is made, after [`Self::tick`] (and the items' pass, which
    /// comes first in the source), and after the hit collisions
    /// (`ftMainProcParams` of every fighter first).
    ///
    /// A trail's frame (`Fx::TextureRand`) is drawn here, in the same link
    /// order, and kept on a Thunder trail for its draw (RE-417).
    pub fn flush_effects(&mut self, sink: &mut dyn crate::effect::HitEffectSink) {
        let (fx, slots, seqs) = (&mut self.fx, &mut self.slots, &self.seq);
        for (seq, e) in fx.drain_sorted_tagged() {
            let Fx::TextureRand(n) = e else {
                sink.weapon(&e);
                continue;
            };
            let frame = crate::rng::rand_int_range(i32::from(n)) as u8;
            let owner = slots
                .iter_mut()
                .zip(seqs)
                .find(|(w, &s)| s == seq && w.is_some());
            if let Some((Some(Weapon::Trail(t)), _)) = owner {
                t.texture = frame;
            }
        }
    }

    /// Makes the clash search's set-offs, in the order it found them. Call
    /// it after the fighters' hit effects and before any
    /// `ftMainProcParams` ([`crate::combat::finish_frame_between`]).
    pub fn flush_clash_effects(&mut self, sink: &mut dyn crate::effect::HitEffectSink) {
        for e in self.clash_fx.drain() {
            sink.weapon(&e);
        }
    }

    /// Effects the queues had no room for.
    pub fn effects_dropped(&self) -> u16 {
        self.fx.dropped.saturating_add(self.clash_fx.dropped)
    }

    pub fn jolts(&self) -> impl Iterator<Item = ThunderJolt> + '_ {
        self.slots.iter().flatten().filter_map(|w| {
            if let Weapon::Jolt(j) = w {
                Some(*j)
            } else {
                None
            }
        })
    }
    pub fn thunder_heads(&self) -> impl Iterator<Item = ThunderHead> + '_ {
        self.slots.iter().flatten().filter_map(|w| {
            if let Weapon::Thunder(h) = w {
                Some(*h)
            } else {
                None
            }
        })
    }
    pub fn thunder_trails(&self) -> impl Iterator<Item = ThunderTrail> + '_ {
        self.slots.iter().flatten().filter_map(|w| {
            if let Weapon::Trail(t) = w {
                Some(*t)
            } else {
                None
            }
        })
    }

    pub fn active_count(&self) -> usize {
        self.slots.iter().filter(|slot| slot.is_some()).count()
    }

    pub fn first_fireball(&self) -> Option<MarioFireball> {
        self.fireballs().next()
    }

    /// Every live Fireball, for the runtime presentation layer. The pool
    /// remains the sole owner of weapon state; renderers receive snapshots.
    pub fn fireballs(&self) -> impl Iterator<Item = MarioFireball> + '_ {
        self.slots.iter().flatten().filter_map(|w| match w {
            Weapon::Fireball(f) => Some(*f),
            _ => None,
        })
    }

    pub fn blasters(&self) -> impl Iterator<Item = FoxBlaster> + '_ {
        self.slots.iter().flatten().filter_map(|w| match w {
            Weapon::Blaster(b) => Some(*b),
            _ => None,
        })
    }

    pub fn charge_shots(&self) -> impl Iterator<Item = SamusChargeShot> + '_ {
        self.slots.iter().flatten().filter_map(|w| match w {
            Weapon::ChargeShot(c) => Some(*c),
            _ => None,
        })
    }

    pub fn bombs(&self) -> impl Iterator<Item = SamusBomb> + '_ {
        self.slots.iter().flatten().filter_map(|w| match w {
            Weapon::Bomb(b) => Some(*b),
            _ => None,
        })
    }

    pub fn eggs(&self) -> impl Iterator<Item = YoshiEgg> + '_ {
        self.slots.iter().flatten().filter_map(|w| match w {
            Weapon::Egg(e) => Some(*e),
            _ => None,
        })
    }

    pub fn stars(&self) -> impl Iterator<Item = YoshiStar> + '_ {
        self.slots.iter().flatten().filter_map(|w| match w {
            Weapon::Star(s) => Some(*s),
            _ => None,
        })
    }

    pub fn cutters(&self) -> impl Iterator<Item = KirbyCutter> + '_ {
        self.slots.iter().flatten().filter_map(|w| match w {
            Weapon::Cutter(c) => Some(*c),
            _ => None,
        })
    }

    pub fn boomerangs(&self) -> impl Iterator<Item = LinkBoomerang> + '_ {
        self.slots.iter().flatten().filter_map(|w| match w {
            Weapon::Boomerang(b) => Some(*b),
            _ => None,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct MapContact {
    pub(crate) position: Vec3,
    normal: Vec2,
    time: f32,
    kind: MapSurfaceKind,
    surface: MapSurface,
}

/// Finds the first one-sided contact of a weapon's authored map-collision
/// diamond. `mpProcessUpdateMain` performs this before weapon map callbacks;
/// support-point sweeping is the allocation-free equivalent of its individual
/// bottom/top/side probes for a weapon that has one symmetric diamond.
pub(crate) fn map_contact<I>(
    surfaces: I,
    from: Vec3,
    to: Vec3,
    coll: BodyColl,
) -> Option<MapContact>
where
    I: IntoIterator<Item = MapSurface>,
{
    let mut first: Option<MapContact> = None;
    let delta = Vec2::new(to.x - from.x, to.y - from.y);

    for surface in surfaces {
        let normal = surface_normal(surface.kind, surface.segment);
        let speed = surface.motion.map_or(Vec3::ZERO, |m| m.speed);
        let relative = Vec2::new(delta.x - speed.x, delta.y - speed.y);
        // `wpMapCheckAllRebound` only reflects an incoming velocity
        // (`lbCommonSim2D(vel, angle) < 0`). This also excludes a point left
        // touching a surface after the preceding frame's rebound.
        if relative.x * normal.x + relative.y * normal.y >= 0.0 {
            continue;
        }
        let support = diamond_support_toward_surface(normal, coll);
        let probe_from = Vec2::new(from.x + support.x + speed.x, from.y + support.y + speed.y);
        let probe_to = Vec2::new(to.x + support.x, to.y + support.y);
        let Some(time) = swept_coords_intersection(probe_from, probe_to, surface.coords()) else {
            continue;
        };
        if first.is_some_and(|known| time >= known.time) {
            continue;
        }
        first = Some(MapContact {
            position: Vec3::new(from.x + delta.x * time, from.y + delta.y * time, from.z),
            normal,
            time,
            kind: surface.kind,
            surface,
        });
    }
    first
}

/// The diamond point farthest *into* a surface, matching the source map
/// collision extents `(top, center, bottom, width)`.
fn diamond_support_toward_surface(normal: Vec2, coll: BodyColl) -> Vec2 {
    let candidates = [
        Vec2::new(0.0, coll.top),
        Vec2::new(coll.width, coll.center),
        Vec2::new(0.0, coll.bottom),
        Vec2::new(-coll.width, coll.center),
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
pub(crate) fn surface_normal(kind: MapSurfaceKind, s: Segment) -> Vec2 {
    let dx = s.x2 as f32 - s.x1 as f32;
    let dy = s.y2 as f32 - s.y1 as f32;
    let mut n = Vec2::new(-dy, dx);
    let choose_positive = match kind {
        MapSurfaceKind::Floor => n.y < 0.0,
        MapSurfaceKind::Ceiling => n.y > 0.0,
        MapSurfaceKind::RightWall => n.x < 0.0,
        MapSurfaceKind::LeftWall => n.x > 0.0,
    };
    if choose_positive {
        n = Vec2::new(-n.x, -n.y);
    }
    let length = n.length();
    if length == 0.0 {
        return match kind {
            MapSurfaceKind::Floor => Vec2::new(0.0, 1.0),
            MapSurfaceKind::Ceiling => Vec2::new(0.0, -1.0),
            MapSurfaceKind::RightWall => Vec2::new(1.0, 0.0),
            MapSurfaceKind::LeftWall => Vec2::new(-1.0, 0.0),
        };
    }
    Vec2::new(n.x / length, n.y / length)
}

/// Returns the movement fraction where the moving point meets the static
/// segment. The `0.001` edge slack is shared with `mpcollision.c`.
pub(crate) fn swept_coords_intersection(from: Vec2, to: Vec2, coords: [f32; 4]) -> Option<f32> {
    const EPS: f32 = 0.001;
    let r = Vec2::new(to.x - from.x, to.y - from.y);
    let [x1, y1, x2, y2] = coords;
    let q = Vec2::new(x1, y1);
    let v = Vec2::new(x2 - x1, y2 - y1);
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

    #[test]
    fn saffron_shots_expire_and_only_razor_accelerates() {
        use crate::monster_weapon::MonsterShot;
        for razor in [false, true] {
            let mut pool = WeaponPool::default();
            pool.spawn_monster_shot(MonsterShot::saffron(razor, Vec3::ZERO));
            let life = if razor { 24 } else { 20 };
            for tick in 1..life {
                pool.tick(open_air, None);
                let shot = pool.monster_shots().next().unwrap();
                assert_eq!(shot.lifetime, life - tick);
                if razor {
                    assert_eq!(shot.velocity.x, -100.0 - tick as f32 * 5.0);
                }
            }
            pool.tick(open_air, None);
            assert!(pool.monster_shots().next().is_none());
        }
    }

    #[test]
    fn saffron_flame_survives_a_recorded_hit_while_razor_is_destroyed() {
        use crate::monster_weapon::MonsterShot;
        for razor in [false, true] {
            let mut pool = WeaponPool::default();
            pool.spawn_monster_shot(MonsterShot::saffron(razor, Vec3::new(0.0, 100.0, 0.0)));
            let mut mario = Fighter::new(FighterKind::Mario, 0, 3);
            pool.apply_hits(&mut mario);
            crate::combat::resolve(&mut mario);
            assert_eq!(mario.damage, if razor { 3 } else { 2 });
            assert_eq!(pool.monster_shots().count(), usize::from(!razor));
            pool.apply_hits(&mut mario);
            crate::combat::resolve(&mut mario);
            assert_eq!(mario.damage, if razor { 3 } else { 2 });
        }
    }

    #[test]
    fn saffron_reflection_keeps_source_lr_and_resets_only_the_flame_life() {
        use crate::monster_weapon::MonsterShot;
        for razor in [false, true] {
            let mut pool = WeaponPool::default();
            let mut shot = MonsterShot::saffron(razor, Vec3::new(100.0, 60.0, 0.0));
            shot.lifetime = 5;
            pool.spawn_monster_shot(shot);
            let mut fox = Fighter::new(FighterKind::Fox, 1, 3);
            fox.situation = Situation::Ground;
            crate::status::set_fox_special_lw_start(&mut fox);
            crate::status::set_any_status(
                &mut fox,
                crate::status::AnyStatus::Fox(crate::status::FoxStatus::SpecialLwLoop),
                0.0,
                crate::status::StatusTiming::unknown(),
            );
            pool.apply_hits(&mut fox);
            let shot = pool.monster_shots().next().unwrap();
            assert_eq!(shot.owner, Some(1));
            assert!(shot.velocity.x > 0.0);
            assert_eq!(shot.lr, if razor { 1.0 } else { -1.0 });
            assert_eq!(shot.lifetime, if razor { 5 } else { 20 });
            assert_eq!(shot.damage, if razor { 6 } else { 4 });
        }
    }

    fn open_air() -> [MapSurface; 0] {
        []
    }

    fn flames_at_damage_centres(centres: &[Vec3]) -> crate::item::ItemPool {
        let mut items = crate::item::ItemPool::default();
        let mut spawner = WeaponPool::default();
        for &centre in centres {
            let pos = centre - Vec3::new(0.0, 200.0, 0.0);
            spawner.queue_item_spawn(crate::item::PKFireSpawn {
                owner_port: 0,
                team: 0,
                pos,
                weapon_pos: pos,
                weapon_coll: BodyColl {
                    top: 10.0,
                    center: 0.0,
                    bottom: -10.0,
                    width: 10.0,
                },
                stale: crate::stale::WeaponStale::FRESH,
            });
        }
        items.take_weapon_spawns(&mut spawner, open_air);
        items
    }

    #[test]
    fn fireball_damages_every_overlapping_item_before_its_hit_callback() {
        let mut items = flames_at_damage_centres(&[Vec3::ZERO; 2]);
        let mut weapons = WeaponPool::default();
        weapons.spawn(WeaponSpawn {
            kind: WeaponKind::MarioFireball,
            owner_port: 1,
            team: 1,
            stale: crate::stale::WeaponStale::FRESH,
            position: Vec3::ZERO,
            facing: 1.0,
        });
        items.search_hurt(&mut [], &mut weapons);
        assert!(items
            .items()
            .all(|item| item.damage_queue == MARIO_FIREBALL_HITBOX.damage));
        assert!(weapons.first_fireball().is_none());
    }

    #[test]
    fn item_contact_expands_bomb_only_after_all_item_searches() {
        let mut items = flames_at_damage_centres(&[Vec3::ZERO, Vec3::new(250.0, 0.0, 0.0)]);
        let mut weapons = WeaponPool::default();
        weapons.spawn(WeaponSpawn {
            kind: WeaponKind::SamusBomb,
            owner_port: 1,
            team: 1,
            stale: crate::stale::WeaponStale::FRESH,
            position: Vec3::ZERO,
            facing: 1.0,
        });
        items.search_hurt(&mut [], &mut weapons);
        assert_eq!(items.get(0).unwrap().damage_queue, SAMUS_BOMB_HITBOX.damage);
        assert_eq!(items.get(1).unwrap().damage_queue, 0);
        assert!(weapons.bombs().next().unwrap().exploded);
    }

    #[test]
    fn surviving_weapon_evicts_first_item_record_after_four_victims() {
        let mut items = flames_at_damage_centres(&[Vec3::ZERO; 5]);
        let mut weapons = WeaponPool::default();
        weapons.spawn(WeaponSpawn {
            kind: WeaponKind::KirbyCutter { grounded: false },
            owner_port: 1,
            team: 1,
            stale: crate::stale::WeaponStale::FRESH,
            position: Vec3::ZERO,
            facing: 1.0,
        });
        items.search_hurt(&mut [], &mut weapons);
        for slot in 0..5 {
            assert_eq!(
                items.get(slot).unwrap().damage_queue,
                KIRBY_CUTTER_HITBOX.damage
            );
            items.get_mut(slot).unwrap().damage_queue = 0;
        }
        items.search_hurt(&mut [], &mut weapons);
        assert_eq!(
            items.get(0).unwrap().damage_queue,
            KIRBY_CUTTER_HITBOX.damage
        );
        for slot in 1..4 {
            assert_eq!(items.get(slot).unwrap().damage_queue, 0);
        }
        assert_eq!(
            items.get(4).unwrap().damage_queue,
            KIRBY_CUTTER_HITBOX.damage
        );
    }

    #[test]
    fn item_victims_share_the_four_records_with_fighter_victims() {
        let mut items = flames_at_damage_centres(&[Vec3::ZERO; 4]);
        let mut weapons = WeaponPool::default();
        weapons.spawn(WeaponSpawn {
            kind: WeaponKind::KirbyCutter { grounded: false },
            owner_port: 1,
            team: 1,
            stale: crate::stale::WeaponStale::FRESH,
            position: Vec3::ZERO,
            facing: 1.0,
        });
        let mut target = Fighter::new(FighterKind::Mario, 2, 3);
        weapons.apply_hits(&mut target);
        crate::combat::resolve(&mut target);
        assert_eq!(i32::from(target.damage), KIRBY_CUTTER_HITBOX.damage);
        items.search_hurt(&mut [], &mut weapons);
        // Four later item victims evict the first fighter record. A fresh
        // fighter state at the same port isolates the attack-record check
        // from damage-status invulnerability and displacement.
        let mut target = Fighter::new(FighterKind::Mario, 2, 3);
        weapons.apply_hits(&mut target);
        crate::combat::resolve(&mut target);
        assert_eq!(i32::from(target.damage), KIRBY_CUTTER_HITBOX.damage);
    }

    #[test]
    fn mario_fireball_uses_the_sourced_launch_and_lifetime() {
        let mut weapons = WeaponPool::default();
        assert!(weapons.spawn(WeaponSpawn {
            kind: WeaponKind::MarioFireball,
            owner_port: 0,
            team: 0,
            stale: crate::stale::WeaponStale::FRESH,
            position: Vec3::ZERO,
            facing: 1.0,
        }));
        let fireball = weapons.first_fireball().unwrap();
        assert_eq!(fireball.lifetime, MARIO_FIREBALL_LIFETIME);
        assert!(fireball.velocity.x > 49.0);
        assert!(fireball.velocity.y < 0.0);

        weapons.tick(open_air, None);
        let fireball = weapons.first_fireball().unwrap();
        assert_eq!(fireball.lifetime, MARIO_FIREBALL_LIFETIME - 1);
        let (sin, _) = sin_cos(MARIO_FIREBALL_ANGLE);
        assert_eq!(
            fireball.velocity.y,
            MARIO_FIREBALL_SPEED * sin - MARIO_FIREBALL_GRAVITY
        );
        // `wpProcessProcWeaponMain` moves it by `vel_air` in open air.
        assert_eq!(fireball.position, fireball.velocity);
    }

    /// `wpMarioFireballProcUpdate` adds the row's `rotate_speed` to
    /// `rotate.x` on every update: 20 degrees for Mario, 25 for Luigi.
    #[test]
    fn fireballs_spin_by_their_rows_rotate_speed_each_update() {
        for (kind, degrees) in [
            (WeaponKind::MarioFireball, 20.0f32),
            (WeaponKind::LuigiFireball, 25.0),
        ] {
            let mut weapons = WeaponPool::default();
            assert!(weapons.spawn(WeaponSpawn {
                kind,
                owner_port: 0,
                team: 0,
                stale: crate::stale::WeaponStale::FRESH,
                position: Vec3::ZERO,
                facing: -1.0,
            }));
            assert_eq!(weapons.first_fireball().unwrap().rotate_x, 0.0);
            for n in 1..=3 {
                weapons.tick(open_air, None);
                let spin = weapons.first_fireball().unwrap().rotate_x;
                assert!(
                    (spin - (n as f32) * degrees.to_radians()).abs() < 1e-5,
                    "{kind:?} {n} {spin}"
                );
            }
        }
    }

    #[test]
    fn yoshi_egg_explodes_on_hit_and_bomb_spawns_two_stars() {
        let mut weapons = WeaponPool::default();
        assert!(weapons.spawn(WeaponSpawn {
            kind: WeaponKind::YoshiEgg {
                throw_force: 10,
                stick_x: 0,
            },
            owner_port: 0,
            team: 0,
            stale: crate::stale::WeaponStale::FRESH,
            position: Vec3::ZERO,
            facing: 1.0,
        }));
        weapons.tick(open_air, None);
        let egg = weapons.eggs().next().unwrap();
        assert!(egg.is_spin);
        assert_eq!(egg.hitbox().damage, 14);
        let mut target = Fighter::new(FighterKind::Mario, 1, 3);
        target.pos = egg.position;
        target.situation = Situation::Ground;
        weapons.apply_hits(&mut target);
        crate::combat::resolve(&mut target);
        assert_eq!(target.damage, 14);
        let egg = weapons.eggs().next().unwrap();
        assert!(egg.exploded);
        assert_eq!(egg.lifetime, EGGTHROW_EXPLODE_LIFETIME);

        let mut stars = WeaponPool::default();
        assert!(stars.spawn(WeaponSpawn {
            kind: WeaponKind::YoshiStars,
            owner_port: 0,
            team: 0,
            stale: crate::stale::WeaponStale::FRESH,
            position: Vec3::ZERO,
            facing: 1.0,
        }));
        let pair: Vec<_> = stars.stars().collect();
        assert_eq!(pair.len(), 2);
        assert_eq!(pair[0].hitbox().damage, 4);
        assert_eq!(pair[0].position.x, -pair[1].position.x);
        assert_eq!(pair[0].lifetime, YOSHISTAR_LIFETIME);
    }

    /// `wpYoshiEggThrowProcUpdate` adds `DTOR(force * -2.1 - 1.5)` to
    /// `rotate.z` on every flight frame after the throw's, and
    /// `wpYoshiStarProcUpdate` spins each star by `0.24 * lr`.
    #[test]
    fn the_egg_and_stars_spin_as_the_source_updates_them() {
        let mut egg = YoshiEgg::new(
            WeaponSpawn {
                kind: WeaponKind::YoshiEgg {
                    throw_force: 10,
                    stick_x: 0,
                },
                owner_port: 0,
                team: 0,
                stale: crate::stale::WeaponStale::FRESH,
                position: Vec3::ZERO,
                facing: 1.0,
            },
            10,
            0,
        );
        assert!(egg.tick(Vec::new, &mut crate::wpeffect::Emit::default()));
        assert_eq!(egg.rotate_z, 0.0);
        let step = (10.0f32 * EGGTHROW_ANGLE_FORCE_MUL + EGGTHROW_ANGLE_ADD).to_radians();
        assert!(egg.tick(Vec::new, &mut crate::wpeffect::Emit::default()));
        assert!(egg.tick(Vec::new, &mut crate::wpeffect::Emit::default()));
        assert!((egg.rotate_z - 2.0 * step).abs() < 1e-6);

        let spawn = WeaponSpawn {
            kind: WeaponKind::YoshiStars,
            owner_port: 0,
            team: 0,
            stale: crate::stale::WeaponStale::FRESH,
            position: Vec3::ZERO,
            facing: 1.0,
        };
        let mut left = YoshiStar::new(spawn, -1.0);
        assert!(left.tick(&mut crate::wpeffect::Emit::default()));
        assert!(left.tick(&mut crate::wpeffect::Emit::default()));
        assert!((left.rotate_z + 2.0 * YOSHISTAR_ROTATE_SPEED).abs() < 1e-6);
    }

    /// `wpManagerMakeWeapon` captures the owner's stale factor; the hit
    /// deals `damage * stale + 0.999` and records the motion in the owner's
    /// queue (`ftMainUpdateDamageStatWeapon`).
    #[test]
    fn a_repeated_egg_throw_is_staled_and_recorded() {
        use crate::stale::MotionAttackId;
        let mut owner = Fighter::new(FighterKind::Yoshi, 0, 3);
        owner.motion.set(MotionAttackId::SpecialHi);
        owner
            .stale
            .push(MotionAttackId::SpecialHi, owner.motion.count);
        owner.motion.set(MotionAttackId::SpecialHi);
        let stale = crate::stale::WeaponStale::of(&owner);
        assert_eq!(stale.stale, 0.75);
        let mut weapons = WeaponPool::default();
        assert!(weapons.spawn(WeaponSpawn {
            kind: WeaponKind::YoshiEgg {
                throw_force: 10,
                stick_x: 0,
            },
            owner_port: 0,
            team: 0,
            stale,
            position: Vec3::ZERO,
            facing: 1.0,
        }));
        weapons.tick(open_air, None);
        let egg = weapons.eggs().next().unwrap();
        let mut target = Fighter::new(FighterKind::Mario, 1, 3);
        target.pos = egg.position;
        target.situation = Situation::Ground;
        weapons.apply_hits(&mut target);
        crate::combat::resolve(&mut target);
        // 14 * 0.75 + 0.999 = 11.499.
        assert_eq!(target.damage, 11);
        weapons.record_landed(&mut target);
        assert_eq!(owner.stale.entries[1], (MotionAttackId::None, 0));
        weapons.record_landed(&mut owner);
        assert_eq!(
            owner.stale.entries[1],
            (MotionAttackId::SpecialHi, stale.motion_count)
        );
    }

    #[test]
    fn fox_blaster_moves_straight_and_hits_once() {
        let mut weapons = WeaponPool::default();
        assert!(weapons.spawn(WeaponSpawn {
            kind: WeaponKind::FoxBlaster,
            owner_port: 0,
            team: 0,
            stale: crate::stale::WeaponStale::FRESH,
            position: Vec3::ZERO,
            facing: 1.0,
        }));
        assert_eq!(weapons.blasters().next().unwrap().velocity.x, 160.0);
        weapons.tick(open_air, None);
        let shot = weapons.blasters().next().unwrap();
        assert_eq!(shot.position.x, 160.0);
        assert_eq!(shot.scale_x, 1.0 + 16.0 / 3.0);
        let mut owner = Fighter::new(FighterKind::Fox, 0, 3);
        owner.pos = shot.position;
        weapons.apply_hits(&mut owner);
        crate::combat::resolve(&mut owner);
        assert_eq!(weapons.active_count(), 1);
        let mut target = Fighter::new(FighterKind::Mario, 1, 3);
        target.pos = shot.position;
        target.situation = Situation::Ground;
        weapons.apply_hits(&mut target);
        crate::combat::resolve(&mut target);
        assert_eq!(target.damage, 6);
        assert_eq!(weapons.active_count(), 0);
    }

    #[test]
    fn fox_reflector_transfers_fireball_ownership_and_reverses_it() {
        let mut weapons = WeaponPool::default();
        weapons.spawn(WeaponSpawn {
            kind: WeaponKind::MarioFireball,
            owner_port: 0,
            team: 0,
            stale: crate::stale::WeaponStale::FRESH,
            position: Vec3::new(100.0, 60.0, 0.0),
            facing: -1.0,
        });
        let mut fox = Fighter::new(FighterKind::Fox, 1, 3);
        fox.pos = Vec3::ZERO;
        fox.situation = Situation::Ground;
        crate::status::set_fox_special_lw_start(&mut fox);
        crate::status::set_any_status(
            &mut fox,
            crate::status::AnyStatus::Fox(crate::status::FoxStatus::SpecialLwLoop),
            0.0,
            crate::status::StatusTiming::unknown(),
        );
        weapons.apply_hits(&mut fox);
        crate::combat::resolve(&mut fox);
        let fireball = weapons.first_fireball().unwrap();
        assert_eq!(fireball.owner_port, fox.port);
        assert!(fireball.velocity.x > 0.0);
        assert_eq!(fireball.damage, 13);
        assert_eq!(fox.damage, 0);
        assert_eq!(
            fox.status.status,
            crate::status::AnyStatus::Fox(crate::status::FoxStatus::SpecialLwHit)
        );
    }

    fn fireball_at(pos: Vec3, facing: f32) -> WeaponPool {
        let mut weapons = WeaponPool::default();
        assert!(weapons.spawn(WeaponSpawn {
            kind: WeaponKind::MarioFireball,
            owner_port: 0,
            team: 0,
            stale: crate::stale::WeaponStale::FRESH,
            position: pos,
            facing,
        }));
        weapons
    }

    /// A grounded fighter with one live attack collision at `pos`.
    fn swinging(kind: FighterKind, damage: i32, pos: Vec3) -> Fighter {
        let mut f = Fighter::new(kind, 1, 3);
        f.situation = Situation::Ground;
        f.floor = Some(crate::ground::Standing {
            line: 0,
            flags: 0,
            normal: Vec2::new(0.0, 1.0),
        });
        f.attack_colls[0] = crate::combat::AttackColl {
            state: crate::combat::AttackState::Transfer,
            damage,
            can_rebound: true,
            size: 60.0,
            is_hit_air: true,
            is_hit_ground: true,
            pos_curr: pos,
            pos_prev: pos,
            ..Default::default()
        };
        f
    }

    /// `ftMainUpdateAttackStatWeapon`: a 15-damage attack against the
    /// 7-damage Fireball. `15 - 10 < 7`, so the attack records the weapon
    /// and rebounds; `7 - 10 < 15`, so the Fireball loses (`proc_setoff`
    /// destroys it) and never reaches the hurtboxes.
    #[test]
    fn an_attack_clanks_with_a_fireball_and_both_recoil_by_the_ten_rule() {
        let at = Vec3::new(120.0, 100.0, 0.0);
        let mut weapons = fireball_at(at, -1.0);
        let mut f = swinging(FighterKind::Mario, 15, at);
        weapons.apply_hits(&mut f);
        assert_eq!(weapons.active_count(), 0);
        assert_eq!(f.hits.attack_shield_push, 15);
        assert_ne!(f.attack_colls[0].records[0].group_id, 7);
        crate::combat::resolve(&mut f);
        assert_eq!(f.damage, 0);
        assert_eq!(f.status.status, crate::status::Status::ReboundWait);
        // 20 damage: `20 - 10 < 7` fails, so no rebound; the Fireball
        // still loses.
        let mut weapons = fireball_at(at, -1.0);
        let mut f = swinging(FighterKind::Mario, 20, at);
        weapons.apply_hits(&mut f);
        assert_eq!(weapons.active_count(), 0);
        assert_eq!(f.hits.attack_shield_push, 0);
        // 2 damage: `7 - 10 < 2` holds too; a weaker attack still beats a
        // Fireball within ten.
        let mut weapons = fireball_at(at, -1.0);
        let mut f = swinging(FighterKind::Mario, 2, at);
        weapons.apply_hits(&mut f);
        assert_eq!(weapons.active_count(), 0);
        assert_eq!(f.hits.attack_shield_push, 2);
    }

    /// The Blaster's `can_setoff` is clear: it passes through attacks.
    #[test]
    fn a_blaster_ignores_attacks_and_hits_the_body() {
        let mut f = swinging(FighterKind::Mario, 15, Vec3::ZERO);
        let mut weapons = WeaponPool::default();
        weapons.spawn(WeaponSpawn {
            kind: WeaponKind::FoxBlaster,
            owner_port: 0,
            team: 0,
            stale: crate::stale::WeaponStale::FRESH,
            position: Vec3::ZERO,
            facing: -1.0,
        });
        weapons.apply_hits(&mut f);
        assert_eq!(f.hits.attack_shield_push, 0);
        crate::combat::resolve(&mut f);
        assert!(f.damage > 0);
    }

    /// `wpLinkBoomerangProcSetOff`: a clank turns the Boomerang back instead
    /// of destroying it, and its record keeps it off that fighter.
    #[test]
    fn a_clanked_boomerang_returns() {
        let mut weapons = WeaponPool::default();
        weapons.spawn(WeaponSpawn {
            kind: WeaponKind::LinkBoomerang {
                is_smash: false,
                stick_x: 0,
                stick_y: 0,
            },
            owner_port: 0,
            team: 0,
            stale: crate::stale::WeaponStale::FRESH,
            position: Vec3::ZERO,
            facing: 1.0,
        });
        let b = weapons.boomerangs().next().unwrap();
        let mut f = swinging(FighterKind::Mario, 30, b.position);
        f.pos = b.position - Vec3::new(0.0, 100.0, 0.0);
        weapons.apply_hits(&mut f);
        let b = weapons.boomerangs().next().expect("survives");
        assert!(b.is_return);
        assert_ne!(b.hit_ports & 2, 0);
    }

    /// `wpLinkBoomerangProcHop` / `ProcShield`.
    #[test]
    fn a_shielded_boomerang_hops_on_a_glancing_contact_and_returns_head_on() {
        let spawn = WeaponSpawn {
            kind: WeaponKind::LinkBoomerang {
                is_smash: false,
                stick_x: 0,
                stick_y: 0,
            },
            owner_port: 0,
            team: 0,
            stale: crate::stale::WeaponStale::FRESH,
            position: Vec3::ZERO,
            facing: 1.0,
        };
        let b = LinkBoomerang::new(spawn, false, 0, 0);
        let angle = b.default_angle;
        let mut w = Weapon::Boomerang(b);
        assert!(w.on_shield(
            crate::combat::ShieldCollide {
                angle: 100.0 * core::f32::consts::PI / 180.0,
                dir_z: 1.0,
            },
            &mut crate::wpeffect::Emit::default()
        ));
        let Weapon::Boomerang(b) = w else {
            unreachable!()
        };
        assert!(!b.is_return);
        let expected = clamp_angle_360(angle + 2.0 * 10.0 * core::f32::consts::PI / 180.0);
        assert!((b.default_angle - expected).abs() < 1e-5);
        assert!(w.on_shield(
            crate::combat::ShieldCollide {
                angle: core::f32::consts::PI,
                dir_z: 0.0,
            },
            &mut crate::wpeffect::Emit::default()
        ));
        let Weapon::Boomerang(b) = w else {
            unreachable!()
        };
        assert!(b.is_return);
        // Live: a still Boomerang inside a raised shield reports 180 degrees
        // and turns back.
        let mut weapons = WeaponPool::default();
        weapons.spawn(spawn);
        let pos = weapons.boomerangs().next().unwrap().position;
        let mut f = Fighter::new(FighterKind::Mario, 1, 3);
        f.situation = Situation::Ground;
        f.pos = pos;
        f.guard.is_shield = true;
        f.guard.shield_health = 55.0;
        weapons.apply_hits(&mut f);
        assert!(f.hits.shield_damage > 0);
        let b = weapons
            .boomerangs()
            .next()
            .expect("the shield does not destroy it");
        assert!(b.is_return);
    }

    /// Fox's reflector is on in `SpecialLwHit` too, and a weapon doing more
    /// than its `damage_resist` of 50 breaks it (`reflect_damage`,
    /// `ftCommonShieldBreakFlyReflectorSetStatus`).
    #[test]
    fn a_strong_weapon_breaks_foxs_reflector() {
        let mut fox = Fighter::new(FighterKind::Fox, 1, 3);
        fox.situation = Situation::Ground;
        crate::status::set_fox_special_lw_hit(&mut fox);
        assert!(crate::combat::reflector(&fox).is_some());
        let mut weapons = fireball_at(Vec3::new(100.0, 60.0, 0.0), -1.0);
        for w in weapons.slots.iter_mut().flatten() {
            if let Weapon::Fireball(f) = w {
                f.damage = 51;
            }
        }
        weapons.apply_hits(&mut fox);
        assert_eq!(fox.hits.reflect_damage, 51);
        assert_eq!(weapons.active_count(), 0);
        crate::combat::resolve(&mut fox);
        assert_eq!(fox.status.status, crate::status::Status::ShieldBreakFly);
        assert_eq!(fox.damage, 0);
    }

    /// `ftMainUpdateAbsorbStatWeapon`: a Yoshi Star is absorbed and gone; a
    /// Final Cutter wave's `proc_absorb` is its `ProcShield`, so it flies on.
    #[test]
    fn psi_magnet_takes_stars_and_lets_cutter_waves_through() {
        let mut ness = Fighter::new(FighterKind::Ness, 1, 3);
        ness.situation = Situation::Ground;
        ness.damage = 50;
        crate::status::set_any_status(
            &mut ness,
            crate::status::AnyStatus::Ness(crate::status::NessStatus::SpecialLwHold),
            0.0,
            crate::status::StatusTiming::unknown(),
        );
        let center = ness.joint_world(0, Vec3::new(300.0, 195.0, 0.0));
        let mut weapons = WeaponPool::default();
        weapons.spawn(WeaponSpawn {
            kind: WeaponKind::KirbyCutter { grounded: false },
            owner_port: 0,
            team: 0,
            stale: crate::stale::WeaponStale::FRESH,
            position: center,
            facing: -1.0,
        });
        weapons.apply_hits(&mut ness);
        assert_eq!(weapons.active_count(), 1, "the wave survives");
        assert!(ness.hits.absorb_lr != 0.0);
        assert!(ness.damage < 50);
        crate::combat::resolve(&mut ness);
        assert_eq!(
            ness.status.status,
            crate::status::AnyStatus::Ness(crate::status::NessStatus::SpecialLwHit)
        );
    }

    #[test]
    fn a_fireball_hits_an_opponent_once_and_never_its_owner() {
        let mut weapons = WeaponPool::default();
        assert!(weapons.spawn(WeaponSpawn {
            kind: WeaponKind::MarioFireball,
            owner_port: 0,
            team: 0,
            stale: crate::stale::WeaponStale::FRESH,
            position: Vec3::ZERO,
            facing: 1.0,
        }));
        let mut owner = Fighter::new(FighterKind::Mario, 0, 3);
        let mut target = Fighter::new(FighterKind::Mario, 1, 3);
        owner.situation = Situation::Ground;
        target.situation = Situation::Ground;

        weapons.apply_hits(&mut owner);

        crate::combat::resolve(&mut owner);
        assert_eq!(weapons.active_count(), 1);
        weapons.apply_hits(&mut target);
        crate::combat::resolve(&mut target);
        assert_eq!(target.damage, 7);
        assert_eq!(weapons.active_count(), 0);
    }

    #[test]
    fn charge_shot_uses_its_level_row_and_ends_on_a_wall() {
        let mut weapons = WeaponPool::default();
        weapons.spawn(WeaponSpawn {
            kind: WeaponKind::SamusChargeShot(7),
            owner_port: 0,
            team: 0,
            stale: crate::stale::WeaponStale::FRESH,
            position: Vec3::ZERO,
            facing: -1.0,
        });
        let shot = weapons.charge_shots().next().unwrap();
        assert_eq!(shot.velocity.x, -74.0);
        assert_eq!(shot.hitbox().damage, 26);
        assert_eq!(shot.hitbox().radius, 130.0);
        let wall = [surface(MapSurfaceKind::RightWall, -100, -500, -100, 500)];
        weapons.tick(|| wall, None);
        assert_eq!(weapons.active_count(), 1);
        weapons.tick(|| wall, None);
        assert_eq!(weapons.active_count(), 0);
    }

    #[test]
    fn bomb_explodes_after_its_fuse_and_hits_each_fighter_once() {
        let mut weapons = WeaponPool::default();
        weapons.spawn(WeaponSpawn {
            kind: WeaponKind::SamusBomb,
            owner_port: 0,
            team: 0,
            stale: crate::stale::WeaponStale::FRESH,
            position: Vec3::ZERO,
            facing: 1.0,
        });
        for _ in 0..99 {
            weapons.tick(open_air, None);
        }
        assert!(!weapons.bombs().next().unwrap().exploded);
        weapons.tick(open_air, None);
        let bomb = weapons.bombs().next().unwrap();
        assert!(bomb.exploded);
        assert_eq!(bomb.hitbox().radius, SAMUS_BOMB_EXPLODE_SIZE);
        let mut target = Fighter::new(FighterKind::Mario, 1, 3);
        target.pos = bomb.position + Vec3::new(150.0, 0.0, 0.0);
        target.situation = Situation::Ground;
        weapons.apply_hits(&mut target);
        crate::combat::resolve(&mut target);
        assert_eq!(target.damage, 9);
        weapons.apply_hits(&mut target);
        crate::combat::resolve(&mut target);
        assert_eq!(target.damage, 9);
        for _ in 0..5 {
            weapons.tick(open_air, None);
        }
        assert_eq!(weapons.active_count(), 1);
        weapons.tick(open_air, None);
        assert_eq!(weapons.active_count(), 0);
    }

    #[test]
    fn bomb_bounces_then_settles_on_the_floor() {
        let mut weapons = WeaponPool::default();
        weapons.spawn(WeaponSpawn {
            kind: WeaponKind::SamusBomb,
            owner_port: 0,
            team: 0,
            stale: crate::stale::WeaponStale::FRESH,
            position: Vec3::new(0.0, 200.0, 0.0),
            facing: 1.0,
        });
        let floor = [surface(MapSurfaceKind::Floor, -1000, 0, 1000, 0)];
        let mut settled = None;
        for tick in 0..90 {
            weapons.tick(|| floor, None);
            let bomb = weapons.bombs().next().unwrap();
            if bomb.floor.is_some() {
                settled = Some(tick);
                break;
            }
        }
        assert!(settled.is_some(), "the bomb comes to rest before its fuse");
        let bomb = weapons.bombs().next().unwrap();
        assert!((bomb.position.y - 75.0).abs() < 1.0);
        weapons.tick(|| floor, None);
        let bomb = weapons.bombs().next().unwrap();
        assert!(bomb.floor.is_some());
        assert_eq!(bomb.position.y, 75.0);
        // `wpSamusBombProcUpdate` spins 10 degrees per grounded update.
        let before = bomb.rotate_z;
        weapons.tick(|| floor, None);
        let bomb = weapons.bombs().next().unwrap();
        let step = (before - bomb.rotate_z) * bomb.lr;
        assert!((step - SAMUS_BOMB_WAIT_ROTATE_SPEED_GROUND).abs() < 1e-6);
    }

    #[test]
    fn airborne_bomb_spins_twenty_degrees_per_update_toward_its_facing() {
        let mut weapons = WeaponPool::default();
        weapons.spawn(WeaponSpawn {
            kind: WeaponKind::SamusBomb,
            owner_port: 0,
            team: 0,
            stale: crate::stale::WeaponStale::FRESH,
            position: Vec3::new(0.0, 2000.0, 0.0),
            facing: -1.0,
        });
        for _ in 0..3 {
            weapons.tick(core::iter::empty, None);
        }
        let bomb = weapons.bombs().next().unwrap();
        assert!((bomb.rotate_z - 3.0 * SAMUS_BOMB_WAIT_ROTATE_SPEED_AIR).abs() < 1e-5);
    }

    fn boomerang_spawn(is_smash: bool, stick_y: i8, facing: f32) -> WeaponSpawn {
        WeaponSpawn {
            kind: WeaponKind::LinkBoomerang {
                is_smash,
                stick_x: 0,
                stick_y,
            },
            owner_port: 0,
            team: 0,
            stale: crate::stale::WeaponStale::FRESH,
            position: Vec3::ZERO,
            facing,
        }
    }

    #[test]
    fn boomerang_slows_turns_back_and_is_caught_by_an_idle_thrower() {
        let mut weapons = WeaponPool::default();
        let mut link = Fighter::new(FighterKind::Link, 0, 3);
        link.situation = Situation::Ground;
        crate::status::set_wait(&mut link);
        weapons.spawn(boomerang_spawn(false, 0, 1.0));
        let b = weapons.boomerangs().next().unwrap();
        assert_eq!(b.position, Vec3::new(BOOMERANG_OFF_X, BOOMERANG_OFF_Y, 0.0));
        assert_eq!(b.lifetime, BOOMERANG_LIFETIME_TILT);
        // 85 slows by 1.4 a frame to the 10 floor, then turns back.
        let mut frames = 0;
        while !weapons.boomerangs().next().unwrap().is_return {
            weapons.observe_owner(&link);
            weapons.tick(open_air, None);
            weapons.sync_owner(&mut link);
            frames += 1;
        }
        assert_eq!(frames, 54);
        let b = weapons.boomerangs().next().unwrap();
        assert_eq!(b.damage, BOOMERANG_RETURN_DAMAGE);
        assert_eq!(b.homing_angle, BOOMERANG_HOMING_ANGLE_MIN);
        assert!(link.link.boomerang_out);
        while weapons.active_count() == 1 {
            weapons.observe_owner(&link);
            weapons.tick(open_air, None);
            weapons.sync_owner(&mut link);
        }
        assert!(!link.link.boomerang_out);
        assert_eq!(
            link.status.status,
            crate::status::AnyStatus::Link(crate::status::LinkStatus::SpecialNGet)
        );
    }

    #[test]
    fn boomerang_keeps_its_launch_yaw_and_hides_its_grandchild_on_return() {
        let mut weapons = WeaponPool::default();
        weapons.spawn(boomerang_spawn(false, 80, -1.0));
        let b = weapons.boomerangs().next().unwrap();
        // `wpMainVelSetModelPitch` from the leftward launch velocity.
        assert_eq!(b.model_rotate_y, -DEG_90);
        assert!(!b.grandchild_hidden);
        assert_eq!(b.anim_ticks, 0);
        let mut ticks = 0;
        while !weapons.boomerangs().next().unwrap().is_return {
            weapons.tick(open_air, None);
            ticks += 1;
        }
        let b = weapons.boomerangs().next().unwrap();
        assert!(b.grandchild_hidden);
        assert_eq!(b.anim_ticks, ticks);
        assert_eq!(b.model_rotate_y, -DEG_90, "set once, at launch");
    }

    #[test]
    fn boomerang_hit_sends_it_back_without_removing_it() {
        let mut weapons = WeaponPool::default();
        weapons.spawn(boomerang_spawn(true, 0, 1.0));
        assert_eq!(
            weapons.boomerangs().next().unwrap().lifetime,
            BOOMERANG_LIFETIME_SMASH
        );
        let mut target = Fighter::new(FighterKind::Mario, 1, 3);
        target.situation = Situation::Ground;
        target.pos = weapons.boomerangs().next().unwrap().position;
        weapons.apply_hits(&mut target);
        crate::combat::resolve(&mut target);
        assert_eq!(target.damage, 9);
        let b = weapons.boomerangs().next().unwrap();
        assert!(b.is_return);
        assert_eq!(b.homing_angle, BOOMERANG_HOMING_ANGLE_MAX);
        assert_eq!(b.lr, -1.0);
        target.hitlag = 0;
        weapons.apply_hits(&mut target);
        crate::combat::resolve(&mut target);
        assert_eq!(target.damage, 9, "the record keeps the target");
    }

    #[test]
    fn boomerang_launch_angle_follows_the_stick_and_mirrors_left() {
        let up = LinkBoomerang::launch_angle(0, 80, 1.0);
        assert!((up - core::f32::consts::PI / 6.0).abs() < 1e-6);
        assert_eq!(LinkBoomerang::launch_angle(40, 10, 1.0), 0.0);
        let left_up = LinkBoomerang::launch_angle(0, 80, -1.0);
        assert!((left_up - 5.0 * core::f32::consts::PI / 6.0).abs() < 1e-5);
        let left_down = LinkBoomerang::launch_angle(0, -80, -1.0);
        assert!((left_down - 7.0 * core::f32::consts::PI / 6.0).abs() < 1e-5);
    }

    #[test]
    fn boomerang_glances_off_a_shallow_surface_and_turns_on_a_steep_one() {
        // Aimed 30 degrees down at a floor: the normal speed stays under
        // half the total, so it reflects and keeps flying out.
        let mut weapons = WeaponPool::default();
        weapons.spawn(boomerang_spawn(false, -80, 1.0));
        let floor = [surface(MapSurfaceKind::Floor, -5000, 0, 5000, 0)];
        let mut reflected = false;
        for _ in 0..20 {
            weapons.tick(|| floor, None);
            let b = weapons.boomerangs().next().unwrap();
            assert!(!b.is_return);
            if b.velocity.y > 0.0 {
                reflected = true;
                break;
            }
        }
        assert!(reflected);

        // Straight into a wall: returns with the fast homing turn.
        let mut weapons = WeaponPool::default();
        weapons.spawn(boomerang_spawn(false, 0, 1.0));
        let wall = [surface(MapSurfaceKind::LeftWall, 400, -500, 400, 1000)];
        for _ in 0..10 {
            weapons.tick(|| wall, None);
        }
        let b = weapons.boomerangs().next().unwrap();
        assert!(b.is_return);
        assert_eq!(b.homing_angle, BOOMERANG_HOMING_ANGLE_MAX);
    }

    fn surface(kind: MapSurfaceKind, x1: i16, y1: i16, x2: i16, y2: i16) -> MapSurface {
        MapSurface {
            motion: None,
            topology: None,
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
                surface(MapSurfaceKind::LeftWall, 50, -100, 50, 100),
                Vec3::new(10.0, 0.0, 0.0),
                Vec3::new(-1.0, 0.0, 0.0),
            ),
            (
                surface(MapSurfaceKind::RightWall, -50, -100, -50, 100),
                Vec3::new(-10.0, 0.0, 0.0),
                Vec3::new(1.0, 0.0, 0.0),
            ),
        ];

        for (surface, delta, normal) in cases {
            let from = Vec3::ZERO;
            let to = from + delta;
            let hit = map_contact([surface], from, to, MARIO_FIREBALL_MAP_COLL)
                .expect("the diamond reaches the map line");
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

    fn shield_at(degrees: f32, dir_z: f32) -> crate::combat::ShieldCollide {
        crate::combat::ShieldCollide {
            angle: degrees * core::f32::consts::PI / 180.0,
            dir_z,
        }
    }

    /// `wpMarioFireballProcHop` below 135 degrees; its `proc_shield`
    /// (`wpMarioFireballProcHit`) from 135 degrees up.
    #[test]
    fn a_glancing_shield_turns_a_fireball_and_a_head_on_one_destroys_it() {
        let weapons = fireball_at(Vec3::ZERO, 1.0);
        let mut w = weapons.slots[0].unwrap();
        let Weapon::Fireball(before) = w else {
            unreachable!()
        };
        // 120 degrees hops by twice (120 - 90).
        assert!(w.on_shield(shield_at(120.0, 1.0), &mut crate::wpeffect::Emit::default()));
        let Weapon::Fireball(after) = w else {
            unreachable!()
        };
        let (sin, cos) = sin_cos(60.0f32.to_radians());
        let v = before.velocity;
        assert!((after.velocity.x - (v.x * cos - v.y * sin)).abs() < 1e-3);
        assert!((after.velocity.y - (v.x * sin + v.y * cos)).abs() < 1e-3);
        // The negative direction turns the other way.
        let mut w = weapons.slots[0].unwrap();
        assert!(w.on_shield(
            shield_at(120.0, -1.0),
            &mut crate::wpeffect::Emit::default()
        ));
        let Weapon::Fireball(after) = w else {
            unreachable!()
        };
        assert!((after.velocity.y - (-v.x * sin + v.y * cos)).abs() < 1e-3);
        // Under 90 degrees the angle clamps to zero.
        let mut w = weapons.slots[0].unwrap();
        assert!(w.on_shield(shield_at(60.0, 1.0), &mut crate::wpeffect::Emit::default()));
        let Weapon::Fireball(after) = w else {
            unreachable!()
        };
        assert!((after.velocity - v).length() < 1e-4);
        let mut w = weapons.slots[0].unwrap();
        assert!(!w.on_shield(shield_at(135.0, 1.0), &mut crate::wpeffect::Emit::default()));
    }

    /// `wpMainVelSetModelPitch`'s yaw, `wpKirbyCutterProcUpdate`'s slope
    /// roll and one animation tick per update.
    #[test]
    fn cutter_wave_yaw_follows_its_velocity_and_roll_its_floor() {
        let spawn = |facing| WeaponSpawn {
            kind: WeaponKind::KirbyCutter { grounded: true },
            owner_port: 0,
            team: 0,
            stale: crate::stale::WeaponStale::FRESH,
            position: Vec3::new(0.0, 100.0, 0.0),
            facing,
        };
        let left = KirbyCutter::new(spawn(-1.0), true);
        assert_eq!(left.model_rotate_y, -core::f32::consts::FRAC_PI_2);
        // A floor rising 100 over 1000 to the right.
        let slope = [surface(MapSurfaceKind::Floor, -1000, 0, 1000, 200)];
        let mut c = KirbyCutter::new(spawn(1.0), true);
        assert_eq!(c.model_rotate_y, core::f32::consts::FRAC_PI_2);
        assert!(c.tick(|| slope, &mut crate::wpeffect::Emit::default()));
        assert_eq!(c.anim_ticks, 1);
        assert!(c.floor.is_some());
        let want = ssb_engine::math::atan2(100.0, 1000.0);
        assert!((c.rotate_z - want).abs() < 1e-4, "{} vs {want}", c.rotate_z);
    }

    /// The per-kind `proc_hop` facing writes and `proc_shield` results.
    #[test]
    fn each_hop_callback_rederives_its_facing() {
        let spawn = |kind| WeaponSpawn {
            kind,
            owner_port: 0,
            team: 0,
            stale: crate::stale::WeaponStale::FRESH,
            position: Vec3::ZERO,
            facing: 1.0,
        };
        // `wpYoshiStarProcHop`: 134 degrees less 90, doubled, turns the
        // rising star back over the top.
        let mut w = Weapon::Star(YoshiStar::new(spawn(WeaponKind::YoshiStars), 1.0));
        assert!(w.on_shield(shield_at(134.0, 1.0), &mut crate::wpeffect::Emit::default()));
        let Weapon::Star(s) = w else { unreachable!() };
        assert!(s.velocity.x < 0.0);
        assert_eq!(s.lr, -1.0);
        assert!(!Weapon::Star(s)
            .on_shield(shield_at(135.0, 1.0), &mut crate::wpeffect::Emit::default()));
        // `wpFoxBlasterProcHop` draws the shot unstretched again.
        let mut blaster = FoxBlaster::new(spawn(WeaponKind::FoxBlaster));
        blaster.scale_x = 20.0;
        let mut w = Weapon::Blaster(blaster);
        assert!(w.on_shield(shield_at(100.0, 1.0), &mut crate::wpeffect::Emit::default()));
        let Weapon::Blaster(b) = w else {
            unreachable!()
        };
        assert_eq!(b.scale_x, 1.0);
        // A Bomb on the floor is not airborne: `wpSamusBombProcHit`
        // explodes it. The explosion ignores the shield.
        let mut bomb = SamusBomb::new(spawn(WeaponKind::SamusBomb));
        bomb.floor = Some((surface(MapSurfaceKind::Floor, -500, 0, 500, 0), 0.0));
        let mut w = Weapon::Bomb(bomb);
        assert!(w.on_shield(shield_at(100.0, 1.0), &mut crate::wpeffect::Emit::default()));
        let Weapon::Bomb(b) = w else { unreachable!() };
        assert!(b.exploded);
        let lifetime = b.lifetime;
        assert!(w.on_shield(shield_at(170.0, 1.0), &mut crate::wpeffect::Emit::default()));
        let Weapon::Bomb(b) = w else { unreachable!() };
        assert_eq!(b.lifetime, lifetime);
        // An airborne Bomb hops and faces its new velocity.
        let mut bomb = SamusBomb::new(spawn(WeaponKind::SamusBomb));
        bomb.velocity = Vec3::new(10.0, 5.0, 0.0);
        let mut w = Weapon::Bomb(bomb);
        assert!(w.on_shield(shield_at(134.0, 1.0), &mut crate::wpeffect::Emit::default()));
        let Weapon::Bomb(b) = w else { unreachable!() };
        assert!(!b.exploded);
        assert!(b.velocity.x < 0.0);
        assert_eq!(b.lr, -1.0);
        // A grounded Thunder Jolt cannot hop: `proc_shield` destroys it.
        let mut jolt = ThunderJolt::new(spawn(WeaponKind::PikachuThunderJolt));
        jolt.surface = Some(surface(MapSurfaceKind::Floor, -500, 0, 500, 0));
        assert!(!Weapon::Jolt(jolt)
            .on_shield(shield_at(100.0, 1.0), &mut crate::wpeffect::Emit::default()));
        // The Final Cutter wave cannot hop and flies on.
        let cutter = KirbyCutter::new(spawn(WeaponKind::KirbyCutter { grounded: false }), false);
        assert!(Weapon::Cutter(cutter)
            .on_shield(shield_at(100.0, 1.0), &mut crate::wpeffect::Emit::default()));
    }

    /// `ftMainUpdateShieldStatWeapon`'s record keeps a hopped shot from
    /// meeting the same fighter again.
    #[test]
    fn a_recorded_fighter_lets_any_weapon_pass() {
        let mut weapons = fireball_at(Vec3::ZERO, 1.0);
        weapons.hit_records[0][0] = Some(1);
        let mut target = Fighter::new(FighterKind::Mario, 1, 3);
        target.situation = Situation::Ground;
        weapons.apply_hits(&mut target);
        crate::combat::resolve(&mut target);
        assert_eq!(target.damage, 0);
        assert_eq!(weapons.active_count(), 1);
    }

    /// `wpProcessProcWeaponMain`: strictly outside any edge deletes it.
    #[test]
    fn a_weapon_past_the_blast_zone_is_deleted() {
        let bounds = BlastZone {
            top: 5000.0,
            bottom: -5000.0,
            left: -5000.0,
            right: 5000.0,
        };
        let mut weapons = fireball_at(Vec3::new(4900.0, 0.0, 0.0), 1.0);
        let step = weapons.first_fireball().unwrap().velocity.x;
        assert!(step > 0.0);
        let mut frames = 0;
        while weapons.active_count() == 1 {
            let x = weapons.first_fireball().unwrap().position.x;
            assert!(x <= bounds.right);
            weapons.tick(open_air, Some(bounds));
            frames += 1;
            assert!(frames < 10);
        }
        // Without bounds the same shot flies on.
        let mut weapons = fireball_at(Vec3::new(4900.0, 0.0, 0.0), 1.0);
        for _ in 0..frames {
            weapons.tick(open_air, None);
        }
        assert_eq!(weapons.active_count(), 1);
    }

    /// `wpLinkBoomerangCheckOffCamera`: after the 130-frame homing delay the
    /// camera is sampled every ninth frame, 40 pixels past the viewport.
    #[test]
    fn an_off_camera_boomerang_is_removed_on_a_sampled_frame() {
        let camera = crate::camera::Camera::default();
        let mut b = LinkBoomerang::new(boomerang_spawn(true, 0, 1.0), true, 0, 0);
        b.position = Vec3::new(1.0e6, 0.0, 0.0);
        for _ in 0..BOOMERANG_HOMING_DELAY + 8 {
            assert!(!b.check_off_camera(Some(&camera)));
        }
        assert!(b.check_off_camera(Some(&camera)));
        assert_eq!(b.adjust_angle_delay, 0);
        // A sampled frame with the Boomerang in view keeps it.
        b.position = camera.at;
        for _ in 0..8 {
            assert!(!b.check_off_camera(Some(&camera)));
        }
        assert!(!b.check_off_camera(Some(&camera)));
        // Without a camera the delays still run.
        let mut b = LinkBoomerang::new(boomerang_spawn(true, 0, 1.0), true, 0, 0);
        for _ in 0..BOOMERANG_HOMING_DELAY + 9 {
            assert!(!b.check_off_camera(None));
        }
        assert_eq!(b.adjust_angle_delay, 0);
    }
}
