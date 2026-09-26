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
    YoshiEgg { throw_force: i16, stick_x: i8 },
    /// `wpYoshiStarMakeStars`: one `nWPKindYoshiStar` each way.
    YoshiStars,
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
};

/// Fox Special1's US `WPAttributes` and `WPBLASTER_VEL_X`.
pub const FOX_BLASTER_SPEED: f32 = 160.0;
pub const FOX_BLASTER_HITBOX: Hitbox = Hitbox {
    damage: 6,
    offset: Vec3::ZERO,
    radius: 40.0,
    angle: 10,
    kb_scale: 100,
    kb_weight: 1,
    kb_base: 0,
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

    fn tick<I, F>(&mut self, surfaces: F) -> bool
    where
        F: Fn() -> I,
        I: IntoIterator<Item = MapSurface>,
    {
        self.scale_x = (self.scale_x + 16.0 / 3.0).min(160.0 / 3.0);
        let wanted = self.position + self.velocity;
        if map_contact(surfaces(), self.position, wanted, FOX_BLASTER_MAP_COLL).is_some() {
            return false;
        }
        self.position = wanted;
        true
    }
}

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
};

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
        SAMUS_CHARGE_SHOT_LEVELS[usize::from(self.charge)].0 / SAMUS_CHARGE_SHOT_GFX_SIZE_DIV
    }

    fn tick<I, F>(&mut self, surfaces: F) -> bool
    where
        F: Fn() -> I,
        I: IntoIterator<Item = MapSurface>,
    {
        let lr = if self.velocity.x < 0.0 { -1.0 } else { 1.0 };
        self.rotate_z -= 18.0f32.to_radians() * lr;
        let half = SAMUS_CHARGE_SHOT_LEVELS[usize::from(self.charge)].4 * 0.5;
        let coll = BodyColl {
            top: half,
            center: 0.0,
            bottom: -half,
            width: half,
        };
        let wanted = self.position + self.velocity;
        if map_contact(surfaces(), self.position, wanted, coll).is_some() {
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
    pub floor: Option<(Segment, f32)>,
    /// Ports already in the attack record.
    pub hit_ports: u8,
    /// `bomb_blink_timer` and the current palette, presentation only.
    pub blink_timer: u16,
    pub blink_palette: u8,
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
    fn tick<I, F>(&mut self, surfaces: F) -> bool
    where
        F: Fn() -> I,
        I: IntoIterator<Item = MapSurface>,
    {
        self.lifetime -= 1;
        if self.exploded {
            return self.lifetime != 0;
        }
        if self.lifetime == 0 {
            self.explode();
            return true;
        }
        match self.floor {
            None => {
                self.velocity.y -= SAMUS_BOMB_WAIT_GRAVITY;
                let speed = Vec2::new(self.velocity.x, self.velocity.y).length();
                if speed > SAMUS_BOMB_WAIT_TVEL {
                    let scale = SAMUS_BOMB_WAIT_TVEL / speed;
                    self.velocity.x *= scale;
                    self.velocity.y *= scale;
                }
            }
            Some((segment, vel_ground)) => {
                // `wpMainVelGroundTransferAir` along the floor line.
                let normal = surface_normal(MapSurfaceKind::Floor, segment);
                self.velocity.x = self.lr * normal.y * vel_ground;
                self.velocity.y = self.lr * -normal.x * vel_ground;
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

        let wanted = self.position + self.velocity;
        if let Some((segment, _)) = self.floor {
            // `wpMapTestLRWallCheckFloor`: slide along the line until it ends.
            let (lo, hi) = if segment.x1 <= segment.x2 {
                (segment.x1, segment.x2)
            } else {
                (segment.x2, segment.x1)
            };
            if wanted.x < f32::from(lo) || wanted.x > f32::from(hi) {
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
                        self.floor = Some((hit.segment, self.velocity.x * self.lr));
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

fn segment_y(s: Segment, x: f32) -> f32 {
    let dx = f32::from(s.x2 - s.x1);
    if dx == 0.0 {
        return f32::from(s.y1);
    }
    f32::from(s.y1) + (x - f32::from(s.x1)) * f32::from(s.y2 - s.y1) / dx
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
        }
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
    fn tick<I, F>(&mut self, surfaces: F, parent: Option<OwnerView>) -> (bool, bool)
    where
        F: Fn() -> I,
        I: IntoIterator<Item = MapSurface>,
    {
        self.lifetime -= 1;
        if self.lifetime == 0 {
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
    fn tick<I, F>(&mut self, surfaces: F) -> bool
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
                self.explode();
                return true;
            }
            self.velocity.y -= EGGTHROW_GRAVITY;
            let speed = Vec2::new(self.velocity.x, self.velocity.y).length();
            if speed > EGGTHROW_TVEL {
                self.velocity.x *= EGGTHROW_TVEL / speed;
                self.velocity.y *= EGGTHROW_TVEL / speed;
            }
        } else {
            self.is_spin = true;
            self.velocity = Self::launch_velocity(self.throw_force, self.stick_x, self.lr);
            self.position.z = 0.0;
        }
        let wanted = self.position + self.velocity;
        match map_contact(surfaces(), self.position, wanted, YOSHI_EGG_MAP_COLL) {
            Some(hit) => {
                self.position = hit.position;
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
        }
    }

    /// `wpYoshiStarGetScale`, presentation only.
    pub fn scale(&self) -> f32 {
        (f32::from(self.lifetime) * YOSHISTAR_LIFETIME_SCALE_MUL + YOSHISTAR_LIFETIME_SCALE_ADD)
            .min(1.0)
    }

    /// `wpYoshiStarProcUpdate`, then the manager's move.
    fn tick(&mut self) -> bool {
        self.lifetime -= 1;
        if self.lifetime == 0 {
            return false;
        }
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
        self.position = self.position + self.velocity;
        true
    }

    /// `wpYoshiStarProcReflector`, then the reflect damage bonus.
    fn reflect(&mut self, reflector: &Fighter) {
        self.lifetime = YOSHISTAR_LIFETIME;
        self.owner_port = reflector.port;
        if self.velocity.x * reflector.facing.sign() < 0.0 {
            self.velocity.x = -self.velocity.x;
        }
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

#[derive(Debug, Clone, Copy, PartialEq)]
enum Weapon {
    Fireball(MarioFireball),
    Blaster(FoxBlaster),
    ChargeShot(SamusChargeShot),
    Bomb(SamusBomb),
    Boomerang(LinkBoomerang),
    Egg(YoshiEgg),
    Star(YoshiStar),
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
        let attr = self.attributes();
        self.velocity.y = (self.velocity.y - attr.gravity).max(-attr.vel_terminal);
        let wanted = self.position + self.velocity;
        if let Some(hit) = map_contact(surfaces(), self.position, wanted, MARIO_FIREBALL_MAP_COLL) {
            self.position = hit.position;
            let dot = self.velocity.x * hit.normal.x + self.velocity.y * hit.normal.y;
            self.velocity.x = (self.velocity.x - 2.0 * dot * hit.normal.x) * attr.rebound;
            self.velocity.y = (self.velocity.y - 2.0 * dot * hit.normal.y) * attr.rebound;
            if self.velocity.x * self.velocity.x + self.velocity.y * self.velocity.y
                < attr.vel_min * attr.vel_min
            {
                return false;
            }
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
    slots: [Option<Weapon>; MAX_WEAPONS],
    /// This frame's [`OwnerView`] per port, from [`Self::observe_owner`].
    owners: [Option<OwnerView>; MAX_OWNERS],
    /// A returning Boomerang reached this port's thrower while its
    /// `is_special_interrupt` was set; [`Self::sync_owner`] delivers it.
    caught: [bool; MAX_OWNERS],
}

impl Default for WeaponPool {
    fn default() -> Self {
        WeaponPool {
            slots: [None; MAX_WEAPONS],
            owners: [None; MAX_OWNERS],
            caught: [false; MAX_OWNERS],
        }
    }
}

impl WeaponPool {
    /// Consumes a fighter's deferred request. A full pool follows the source
    /// manager's allocation-failure shape: the already-consumed script event
    /// is not retried on a later frame.
    pub fn spawn(&mut self, spawn: WeaponSpawn) -> bool {
        if spawn.kind == WeaponKind::YoshiStars {
            // Two `wpManagerMakeWeapon` calls; each fails on its own.
            let lr = if spawn.facing < 0.0 { -1.0 } else { 1.0 };
            let first = self.insert(Weapon::Star(YoshiStar::new(spawn, lr)));
            let second = self.insert(Weapon::Star(YoshiStar::new(spawn, -lr)));
            return first || second;
        }
        let Some(slot) = self.slots.iter_mut().find(|slot| slot.is_none()) else {
            return false;
        };
        *slot = Some(match spawn.kind {
            WeaponKind::MarioFireball => Weapon::Fireball(MarioFireball::new(spawn, 0)),
            WeaponKind::LuigiFireball => Weapon::Fireball(MarioFireball::new(spawn, 1)),
            WeaponKind::FoxBlaster => Weapon::Blaster(FoxBlaster::new(spawn)),
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
        });
        true
    }

    fn insert(&mut self, weapon: Weapon) -> bool {
        match self.slots.iter_mut().find(|slot| slot.is_none()) {
            Some(slot) => {
                *slot = Some(weapon);
                true
            }
            None => false,
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
            });
        }
    }

    /// Delivers the pool's writes to a fighter after [`Self::tick`]:
    /// `wpLinkBoomerangCheckOwnerCatch`'s catch status, and
    /// `wpLinkBoomerangClearGObjs`, which clears the thrower's
    /// `boomerang_gobj` when the Boomerang goes away (or was never made
    /// because the pool was full).
    pub fn sync_owner(&mut self, f: &mut Fighter) {
        let port = usize::from(f.port);
        if port < MAX_OWNERS && core::mem::take(&mut self.caught[port]) {
            crate::link::set_special_n_get(f);
        }
        f.link.boomerang_out = self
            .slots
            .iter()
            .flatten()
            .any(|w| matches!(w, Weapon::Boomerang(b) if b.parent_port == Some(f.port)));
    }

    /// Advances each weapon's source physics and map callback once. Call this
    /// once per match frame, before [`Self::apply_hits`].
    pub fn tick<I, F>(&mut self, surfaces: F)
    where
        F: Fn() -> I + Copy,
        I: IntoIterator<Item = MapSurface>,
    {
        let owners = self.owners;
        for slot in &mut self.slots {
            if let Some(weapon) = slot.as_mut() {
                let alive = match weapon {
                    Weapon::Fireball(fireball) => fireball.tick(surfaces),
                    Weapon::Blaster(blaster) => blaster.tick(surfaces),
                    Weapon::ChargeShot(shot) => shot.tick(surfaces),
                    Weapon::Bomb(bomb) => bomb.tick(surfaces),
                    Weapon::Boomerang(boomerang) => {
                        let parent = boomerang
                            .parent_port
                            .and_then(|port| owners.get(usize::from(port)).copied().flatten());
                        let (alive, caught) = boomerang.tick(surfaces, parent);
                        if caught {
                            if let Some(port) = boomerang.parent_port {
                                self.caught[usize::from(port)] = true;
                            }
                        }
                        alive
                    }
                    Weapon::Egg(egg) => egg.tick(surfaces),
                    Weapon::Star(star) => star.tick(),
                };
                if !alive {
                    *slot = None;
                }
            }
        }
    }

    /// Resolves every eligible weapon against one fighter. Fireball and
    /// Blaster both delete on registered contact; invincibility leaves the
    /// shot live, exactly like a non-registered source hitbox.
    pub fn apply_hits(&mut self, defender: &mut Fighter) {
        for slot in &mut self.slots {
            let Some(weapon) = slot else { continue };
            let (owner, mut hitbox, position) = match weapon {
                Weapon::Fireball(f) => (f.owner_port, MARIO_FIREBALL_HITBOX, f.position),
                Weapon::Blaster(b) => (b.owner_port, FOX_BLASTER_HITBOX, b.position),
                Weapon::ChargeShot(c) => (c.owner_port, c.hitbox(), c.position),
                Weapon::Bomb(b) => (b.owner_port, b.hitbox(), b.position),
                Weapon::Boomerang(b) => (b.owner_port, b.hitbox(), b.position),
                Weapon::Egg(e) => (e.owner_port, e.hitbox(), e.position),
                Weapon::Star(s) => (s.owner_port, s.hitbox(), s.position),
            };
            if owner == defender.port {
                continue;
            }
            let bit = 1u8 << (defender.port & 7);
            if matches!(weapon, Weapon::Boomerang(b) if b.hit_ports & bit != 0) {
                continue;
            }
            // The Bomb's `WPAttributes::can_reflect` is clear, and its attack
            // record outlives the explosion.
            // The exploding egg can no longer be reflected, and it keeps the
            // record of what the egg hit.
            if let Weapon::Egg(egg) = weapon {
                if egg.hit_ports & bit != 0 {
                    continue;
                }
                if egg.exploded {
                    if attack::apply_hitbox_at(
                        &hitbox,
                        position,
                        crate::stale::HANDICAP_DEFAULT,
                        defender,
                    )
                    .registered()
                    {
                        egg.hit_ports |= bit;
                    }
                    continue;
                }
            }
            if let Weapon::Bomb(bomb) = weapon {
                let bit = 1u8 << (defender.port & 7);
                if bomb.hit_ports & bit != 0 {
                    continue;
                }
                if attack::apply_hitbox_at(
                    &hitbox,
                    position,
                    crate::stale::HANDICAP_DEFAULT,
                    defender,
                )
                .registered()
                {
                    bomb.hit_ports |= bit;
                    if !bomb.exploded {
                        bomb.explode();
                    }
                }
                continue;
            }
            if defender.kind == crate::fighter::FighterKind::Fox
                && matches!(
                    defender.status.status,
                    crate::status::AnyStatus::Fox(
                        crate::status::FoxStatus::SpecialLwLoop
                            | crate::status::FoxStatus::SpecialLwTurn
                            | crate::status::FoxStatus::SpecialAirLwLoop
                            | crate::status::FoxStatus::SpecialAirLwTurn
                    )
                )
            {
                let dx = position.x - defender.pos.x;
                let dy = position.y - (defender.pos.y + 60.0);
                if dx * dx + dy * dy <= 350.0 * 350.0 {
                    // `wpMainReflectorSetLR`: turn X toward Fox's facing,
                    // transfer ownership, and apply the US 1.8x + 0.99 bonus.
                    match weapon {
                        Weapon::Fireball(f) => {
                            f.owner_port = defender.port;
                            if f.velocity.x * defender.facing.sign() < 0.0 {
                                f.velocity.x = -f.velocity.x;
                            }
                            f.lifetime = f.attributes().lifetime;
                            f.damage = ((f.damage as f32 * 1.8 + 0.99) as i32).min(100);
                        }
                        Weapon::Blaster(b) => {
                            b.owner_port = defender.port;
                            if b.velocity.x * defender.facing.sign() < 0.0 {
                                b.velocity.x = -b.velocity.x;
                            }
                            b.scale_x = 1.0;
                            b.damage = ((b.damage as f32 * 1.8 + 0.99) as i32).min(100);
                        }
                        Weapon::ChargeShot(c) => {
                            c.owner_port = defender.port;
                            if c.velocity.x * defender.facing.sign() < 0.0 {
                                c.velocity.x = -c.velocity.x;
                            }
                            c.damage = ((c.damage as f32 * 1.8 + 0.99) as i32).min(100);
                        }
                        Weapon::Bomb(_) => unreachable!("bombs skip the reflector"),
                        Weapon::Boomerang(b) => b.reflect(defender),
                        Weapon::Egg(e) => e.reflect(defender),
                        Weapon::Star(s) => s.reflect(defender),
                    }
                    crate::status::set_fox_special_lw_hit(defender);
                    continue;
                }
            }
            hitbox.damage = match weapon {
                Weapon::Fireball(f) => f.damage,
                Weapon::Blaster(b) => b.damage,
                Weapon::ChargeShot(c) => c.damage,
                Weapon::Bomb(_) => hitbox.damage,
                Weapon::Boomerang(b) => b.damage,
                Weapon::Egg(e) => e.damage,
                Weapon::Star(s) => s.damage,
            };
            // `wp->handicap` is the owner's; every Training player has the
            // default handicap. Weapon damage is not staled yet (TODO.md).
            if attack::apply_hitbox_at(&hitbox, position, crate::stale::HANDICAP_DEFAULT, defender)
                .registered()
            {
                // The Boomerang survives a hit and turns back.
                if let Weapon::Boomerang(b) = weapon {
                    b.hit_ports |= bit;
                    b.on_hit();
                    continue;
                }
                // `wpYoshiEggThrowProcHit`: the egg explodes in place.
                if let Weapon::Egg(e) = weapon {
                    e.hit_ports |= bit;
                    e.explode();
                    continue;
                }
                *slot = None;
            }
        }
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

    pub fn boomerangs(&self) -> impl Iterator<Item = LinkBoomerang> + '_ {
        self.slots.iter().flatten().filter_map(|w| match w {
            Weapon::Boomerang(b) => Some(*b),
            _ => None,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct MapContact {
    position: Vec3,
    normal: Vec2,
    time: f32,
    kind: MapSurfaceKind,
    segment: Segment,
}

/// Finds the first one-sided contact of a weapon's authored map-collision
/// diamond. `mpProcessUpdateMain` performs this before weapon map callbacks;
/// support-point sweeping is the allocation-free equivalent of its individual
/// bottom/top/side probes for a weapon that has one symmetric diamond.
fn map_contact<I>(surfaces: I, from: Vec3, to: Vec3, coll: BodyColl) -> Option<MapContact>
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
        let support = diamond_support_toward_surface(normal, coll);
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
            kind: surface.kind,
            segment: surface.segment,
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
    fn yoshi_egg_explodes_on_hit_and_bomb_spawns_two_stars() {
        let mut weapons = WeaponPool::default();
        assert!(weapons.spawn(WeaponSpawn {
            kind: WeaponKind::YoshiEgg {
                throw_force: 10,
                stick_x: 0,
            },
            owner_port: 0,
            position: Vec3::ZERO,
            facing: 1.0,
        }));
        weapons.tick(open_air);
        let egg = weapons.eggs().next().unwrap();
        assert!(egg.is_spin);
        assert_eq!(egg.hitbox().damage, 14);
        let mut target = Fighter::new(FighterKind::Mario, 1, 3);
        target.pos = egg.position;
        target.situation = Situation::Ground;
        weapons.apply_hits(&mut target);
        assert_eq!(target.damage, 14);
        let egg = weapons.eggs().next().unwrap();
        assert!(egg.exploded);
        assert_eq!(egg.lifetime, EGGTHROW_EXPLODE_LIFETIME);

        let mut stars = WeaponPool::default();
        assert!(stars.spawn(WeaponSpawn {
            kind: WeaponKind::YoshiStars,
            owner_port: 0,
            position: Vec3::ZERO,
            facing: 1.0,
        }));
        let pair: Vec<_> = stars.stars().collect();
        assert_eq!(pair.len(), 2);
        assert_eq!(pair[0].hitbox().damage, 4);
        assert_eq!(pair[0].position.x, -pair[1].position.x);
        assert_eq!(pair[0].lifetime, YOSHISTAR_LIFETIME);
    }

    #[test]
    fn fox_blaster_moves_straight_and_hits_once() {
        let mut weapons = WeaponPool::default();
        assert!(weapons.spawn(WeaponSpawn {
            kind: WeaponKind::FoxBlaster,
            owner_port: 0,
            position: Vec3::ZERO,
            facing: 1.0,
        }));
        assert_eq!(weapons.blasters().next().unwrap().velocity.x, 160.0);
        weapons.tick(open_air);
        let shot = weapons.blasters().next().unwrap();
        assert_eq!(shot.position.x, 160.0);
        assert_eq!(shot.scale_x, 1.0 + 16.0 / 3.0);
        let mut owner = Fighter::new(FighterKind::Fox, 0, 3);
        owner.pos = shot.position;
        weapons.apply_hits(&mut owner);
        assert_eq!(weapons.active_count(), 1);
        let mut target = Fighter::new(FighterKind::Mario, 1, 3);
        target.pos = shot.position;
        target.situation = Situation::Ground;
        weapons.apply_hits(&mut target);
        assert_eq!(target.damage, 6);
        assert_eq!(weapons.active_count(), 0);
    }

    #[test]
    fn fox_reflector_transfers_fireball_ownership_and_reverses_it() {
        let mut weapons = WeaponPool::default();
        weapons.spawn(WeaponSpawn {
            kind: WeaponKind::MarioFireball,
            owner_port: 0,
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

    #[test]
    fn charge_shot_uses_its_level_row_and_ends_on_a_wall() {
        let mut weapons = WeaponPool::default();
        weapons.spawn(WeaponSpawn {
            kind: WeaponKind::SamusChargeShot(7),
            owner_port: 0,
            position: Vec3::ZERO,
            facing: -1.0,
        });
        let shot = weapons.charge_shots().next().unwrap();
        assert_eq!(shot.velocity.x, -74.0);
        assert_eq!(shot.hitbox().damage, 26);
        assert_eq!(shot.hitbox().radius, 130.0);
        let wall = [surface(MapSurfaceKind::LeftWall, -100, -500, -100, 500)];
        weapons.tick(|| wall);
        assert_eq!(weapons.active_count(), 1);
        weapons.tick(|| wall);
        assert_eq!(weapons.active_count(), 0);
    }

    #[test]
    fn bomb_explodes_after_its_fuse_and_hits_each_fighter_once() {
        let mut weapons = WeaponPool::default();
        weapons.spawn(WeaponSpawn {
            kind: WeaponKind::SamusBomb,
            owner_port: 0,
            position: Vec3::ZERO,
            facing: 1.0,
        });
        for _ in 0..99 {
            weapons.tick(open_air);
        }
        assert!(!weapons.bombs().next().unwrap().exploded);
        weapons.tick(open_air);
        let bomb = weapons.bombs().next().unwrap();
        assert!(bomb.exploded);
        assert_eq!(bomb.hitbox().radius, SAMUS_BOMB_EXPLODE_SIZE);
        let mut target = Fighter::new(FighterKind::Mario, 1, 3);
        target.pos = bomb.position + Vec3::new(150.0, 0.0, 0.0);
        target.situation = Situation::Ground;
        weapons.apply_hits(&mut target);
        assert_eq!(target.damage, 9);
        weapons.apply_hits(&mut target);
        assert_eq!(target.damage, 9);
        for _ in 0..5 {
            weapons.tick(open_air);
        }
        assert_eq!(weapons.active_count(), 1);
        weapons.tick(open_air);
        assert_eq!(weapons.active_count(), 0);
    }

    #[test]
    fn bomb_bounces_then_settles_on_the_floor() {
        let mut weapons = WeaponPool::default();
        weapons.spawn(WeaponSpawn {
            kind: WeaponKind::SamusBomb,
            owner_port: 0,
            position: Vec3::new(0.0, 200.0, 0.0),
            facing: 1.0,
        });
        let floor = [surface(MapSurfaceKind::Floor, -1000, 0, 1000, 0)];
        let mut settled = None;
        for tick in 0..90 {
            weapons.tick(|| floor);
            let bomb = weapons.bombs().next().unwrap();
            if bomb.floor.is_some() {
                settled = Some(tick);
                break;
            }
        }
        assert!(settled.is_some(), "the bomb comes to rest before its fuse");
        let bomb = weapons.bombs().next().unwrap();
        assert!((bomb.position.y - 75.0).abs() < 1.0);
        weapons.tick(|| floor);
        let bomb = weapons.bombs().next().unwrap();
        assert!(bomb.floor.is_some());
        assert_eq!(bomb.position.y, 75.0);
    }

    fn boomerang_spawn(is_smash: bool, stick_y: i8, facing: f32) -> WeaponSpawn {
        WeaponSpawn {
            kind: WeaponKind::LinkBoomerang {
                is_smash,
                stick_x: 0,
                stick_y,
            },
            owner_port: 0,
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
            weapons.tick(open_air);
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
            weapons.tick(open_air);
            weapons.sync_owner(&mut link);
        }
        assert!(!link.link.boomerang_out);
        assert_eq!(
            link.status.status,
            crate::status::AnyStatus::Link(crate::status::LinkStatus::SpecialNGet)
        );
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
        assert_eq!(target.damage, 9);
        let b = weapons.boomerangs().next().unwrap();
        assert!(b.is_return);
        assert_eq!(b.homing_angle, BOOMERANG_HOMING_ANGLE_MAX);
        assert_eq!(b.lr, -1.0);
        target.hitlag = 0;
        weapons.apply_hits(&mut target);
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
            weapons.tick(|| floor);
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
        let wall = [surface(MapSurfaceKind::RightWall, 400, -500, 400, 1000)];
        for _ in 0..10 {
            weapons.tick(|| wall);
        }
        let b = weapons.boomerangs().next().unwrap();
        assert!(b.is_return);
        assert_eq!(b.homing_angle, BOOMERANG_HOMING_ANGLE_MAX);
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
}
