//! Spawn requests for fighter-owned weapons.
//!
//! The original keeps weapons in the match-wide `wpManager`, not inside their
//! owner fighter. A fighter therefore emits a small, portable request and the
//! match layer owns the bounded weapon pool that consumes it. Keeping that
//! boundary explicit prevents projectile lifetime/collision from becoming a
//! hidden per-fighter side effect.

use ssb_engine::math::{sin_cos, Vec2, Vec3};
use ssb_engine::memory::{FreeOrder, ObjectPool};

use crate::attack::{self, Hitbox};
use crate::collision::Segment;
use crate::fighter::Fighter;
use crate::ground::{BodyColl, FloorState};
use crate::map::{MASK_CEIL, MASK_FLOOR, MASK_LWALL, MASK_RWALL};
use crate::monster_weapon::{ShotKind, ShotProc};
use crate::status::BlastZone;
use crate::wpeffect::{Emit, WeaponEffect as Fx};
#[path = "ness_weapon.rs"]
mod ness;
#[path = "pikachu_weapon.rs"]
mod pikachu;
pub use ness::{PKFire, PKThunder, PKThunderTrail, PK_THUNDER_TRAIL_ENV, PK_THUNDER_TRAIL_PRIM};
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
#[derive(Debug, Clone, Copy, PartialEq)]
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
        /// The held egg's `DObj` scale at the throw, presentation only
        /// ([`crate::yoshi::egg_scale`]).
        scale: Vec3,
    },
    /// `wpYoshiStarMakeStars`: one `nWPKindYoshiStar` each way.
    YoshiStars,
    /// `nWPKindCutter`, Final Cutter's wave. `floor_line`: Kirby's
    /// `floor_line_id`, which the wave follows if it lies under the spawn.
    KirbyCutter {
        floor_line: Option<u16>,
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
    /// `nWPKindBulletNormal` or `nWPKindBulletHard`, Master Hand's finger
    /// gun (`wpBossBullet{Normal,Hard}MakeWeapon`).
    BossBullet {
        hard: bool,
    },
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

/// The parent fighter's `coll_data.p_translate` and `map_coll` as
/// `wpManagerMakeWeapon`'s `WEAPON_FLAG_COLLPROJECT` pass reads them
/// (`mpCommonRunWeaponCollisionDefault`, wpmanager.c:329): the sweep runs
/// from `pos` with `coll`'s diamond to the weapon's spawn point with its own.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct SpawnParent {
    pub pos: Vec3,
    pub coll: BodyColl,
}

impl WeaponKind {
    /// Kinds whose make runs from the fighter's `proc_accessory`, after
    /// `ftMainProcPhysicsMap` moved it (`ftMarioSpecialNProcAccessory`,
    /// `ftNessSpecialNProcAccessory`, `ftPikachuSpecialNProcAccessory`,
    /// `ftCommonFireFlowerShootProcAccessory`, `ftCommonLGunShootProcAccessory`):
    /// their parent translate is the fighter's post-physics one. Every other
    /// fighter maker runs in `proc_update` or `proc_physics`, before the move.
    pub fn made_in_accessory(&self) -> bool {
        matches!(
            self,
            WeaponKind::MarioFireball
                | WeaponKind::LuigiFireball
                | WeaponKind::NessPKFire { .. }
                | WeaponKind::PikachuThunderJolt
                | WeaponKind::Equipment {
                    kind: ShotKind::FireFlower | ShotKind::RayGun,
                    ..
                }
        )
    }
}

/// Mario's row of `dWPMarioFireballWeaponAttributes` (`wpmariofireball.c`,
/// US version), read from the ROM at build time ([`FIREBALL_ATTRIBUTES`]).
pub const MARIO_FIREBALL_LIFETIME: u16 = FIREBALL_ATTRIBUTES[0].lifetime;
pub const MARIO_FIREBALL_GRAVITY: f32 = FIREBALL_ATTRIBUTES[0].gravity;
pub const MARIO_FIREBALL_TERMINAL_VELOCITY: f32 = FIREBALL_ATTRIBUTES[0].vel_terminal;
pub const MARIO_FIREBALL_SPEED: f32 = FIREBALL_ATTRIBUTES[0].vel_base;
pub const MARIO_FIREBALL_ANGLE: f32 = FIREBALL_ATTRIBUTES[0].angle;
pub const MARIO_FIREBALL_REBOUND: f32 = FIREBALL_ATTRIBUTES[0].rebound;
pub const MARIO_FIREBALL_MIN_SPEED: f32 = FIREBALL_ATTRIBUTES[0].vel_min;

/// Luigi's row of `dWPMarioFireballWeaponAttributes` (US): no gravity, a
/// slower level shot and a shorter life. Its `WPAttributes` in
/// `222_LuigiSpecial1.c` differ from Mario's only in damage.
pub const LUIGI_FIREBALL_LIFETIME: u16 = FIREBALL_ATTRIBUTES[1].lifetime;
pub const LUIGI_FIREBALL_SPEED: f32 = FIREBALL_ATTRIBUTES[1].vel_base;
pub const LUIGI_FIREBALL_DAMAGE: i32 =
    crate::monster_weapon::weapon_attributes::LUIGI_FIREBALL.damage;

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

/// `dWPMarioFireballWeaponAttributes`: index 0 is Mario, 1 is Luigi. The
/// values are the ROM's ([`crate::d_arrays`]); the damage is the hitbox's
/// (`WPAttributes`, not part of this table).
pub const FIREBALL_ATTRIBUTES: [FireballAttributes; 2] = [
    fireball_attributes(0, MARIO_FIREBALL_HITBOX.damage),
    fireball_attributes(1, LUIGI_FIREBALL_DAMAGE),
];

/// [`FireballAttributes::angle`] is one value because both rows have the
/// same ground and air angle (`angle_ground`, `angle_air`), so the spawn does
/// not need the owner's situation.
const _: () = {
    use crate::d_arrays as d;
    let mut row = 0;
    while row < 2 {
        assert!(
            d::WP_MARIO_FIREBALL_WEAPON_ATTRIBUTES_ANGLE_GROUND[row].to_bits()
                == d::WP_MARIO_FIREBALL_WEAPON_ATTRIBUTES_ANGLE_AIR[row].to_bits()
        );
        row += 1;
    }
};

/// One row of `dWPMarioFireballWeaponAttributes`.
const fn fireball_attributes(row: usize, damage: i32) -> FireballAttributes {
    use crate::d_arrays as d;
    let lifetime = d::WP_MARIO_FIREBALL_WEAPON_ATTRIBUTES_LIFETIME[row];
    assert!(lifetime >= 0 && lifetime <= u16::MAX as i32);
    FireballAttributes {
        lifetime: lifetime as u16,
        vel_terminal: d::WP_MARIO_FIREBALL_WEAPON_ATTRIBUTES_VEL_TERMINAL[row],
        vel_min: d::WP_MARIO_FIREBALL_WEAPON_ATTRIBUTES_VEL_MIN[row],
        gravity: d::WP_MARIO_FIREBALL_WEAPON_ATTRIBUTES_GRAVITY[row],
        rebound: d::WP_MARIO_FIREBALL_WEAPON_ATTRIBUTES_COLLIDE_REBOUND[row],
        rotate_speed: d::WP_MARIO_FIREBALL_WEAPON_ATTRIBUTES_ROTATE_SPEED[row],
        angle: d::WP_MARIO_FIREBALL_WEAPON_ATTRIBUTES_ANGLE_GROUND[row],
        vel_base: d::WP_MARIO_FIREBALL_WEAPON_ATTRIBUTES_VEL_BASE[row],
        damage,
    }
}

/// `WPAttributes.map_coll` from Mario Special1: the projectile's collision
/// diamond is deliberately larger than its rendered sprite.
pub const MARIO_FIREBALL_MAP_COLL: BodyColl =
    crate::monster_weapon::weapon_attributes::MARIO_FIREBALL.map_coll;

/// The source `WPAttributes` hitbox, stored in the Mario Special1 reloc file
/// (`llMarioSpecial1FireballWeaponAttributes`).
pub const MARIO_FIREBALL_HITBOX: Hitbox =
    crate::monster_weapon::weapon_attributes::MARIO_FIREBALL.hitbox();

/// Fox Special1's US `WPAttributes` and `WPBLASTER_VEL_X`.
pub const FOX_BLASTER_SPEED: f32 = 160.0;
pub const FOX_BLASTER_HITBOX: Hitbox =
    crate::monster_weapon::weapon_attributes::FOX_BLASTER.hitbox();
pub const FOX_BLASTER_MAP_COLL: BodyColl =
    crate::monster_weapon::weapon_attributes::FOX_BLASTER.map_coll;

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
        let from = self.position;
        self.position += self.velocity;
        // `wpFoxBlasterProcMap`: the glow where the shot stopped.
        if crate::wpmap::check_coll_end(&FOX_BLASTER_MAP_COLL, from, &mut self.position, &surfaces)
        {
            fx.push(Fx::FoxBlasterGlow(self.position));
            return false;
        }
        true
    }
}

/// `llBossMainMotionBulletNormalWeaponAttributes` and `...Hard...` in
/// `249_BossMainMotion.c`, which are identical.
pub const BOSS_BULLET_HITBOX: Hitbox =
    crate::monster_weapon::weapon_attributes::BOSS_BULLET_NORMAL.hitbox();
pub const BOSS_BULLET_MAP_COLL: BodyColl =
    crate::monster_weapon::weapon_attributes::BOSS_BULLET_NORMAL.map_coll;
/// Source `wpBossBullet`: a straight shot (`vel = { 160 * lr, -25 }`, no
/// `proc_update`). Its `proc_map` and `proc_hit` both end it: the blast
/// `wpBossBulletExplodeInitVars` sets up never runs, since
/// `wpBossBulletProcMap` returns TRUE after it (ROM 0x0E86F0).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BossBullet {
    pub owner_port: u8,
    pub damage: i32,
    pub position: Vec3,
    pub velocity: Vec3,
    /// `nWPKindBulletHard`; the two share their attributes.
    pub hard: bool,
}

/// `wpMainReflectorRotateWeaponModel` (`wp/wpmain.c:170`): the model
/// rotation `[x, y, z]` that turns a weapon along `vel`. The velocity is
/// `syVectorNorm3D`'d (a multiply by `1 / |vel|`) before its cross product
/// with `(0, 0, -1)` (moving right) or `(0, 0, +1)`, whose `x` and `y` give
/// the pitch: the normalisation moves the ratio `syUtilsArcTan2` takes by an
/// ulp for some velocities (RE-503).
pub fn reflector_rotate(vel: Vec3) -> [f32; 3] {
    use ssb_engine::math::{f_cst_dtor32, sy_arc_tan2};
    let dir_z = if vel.x > 0.0 { -1.0 } else { 1.0 };
    let mut v = vel;
    crate::slope_contour::norm3(&mut v);
    let angle = crate::slope_contour::cross(v, Vec3::new(0.0, 0.0, dir_z));
    if dir_z == -1.0 {
        [sy_arc_tan2(angle.x, angle.y), f_cst_dtor32(90.0), 0.0]
    } else {
        [sy_arc_tan2(-angle.x, angle.y), f_cst_dtor32(-90.0), 0.0]
    }
}

impl BossBullet {
    fn new(spawn: WeaponSpawn, hard: bool) -> Self {
        let lr = if spawn.facing < 0.0 { -1.0 } else { 1.0 };
        Self {
            owner_port: spawn.owner_port,
            damage: BOSS_BULLET_HITBOX.damage,
            position: spawn.position,
            velocity: Vec3::new(
                crate::boss::BULLET_VEL_X * lr,
                crate::boss::BULLET_VEL_Y,
                0.0,
            ),
            hard,
        }
    }

    pub fn hitbox(&self) -> Hitbox {
        Hitbox {
            damage: self.damage,
            ..BOSS_BULLET_HITBOX
        }
    }

    /// The shot's move and `wpBossBulletProcMap`.
    fn tick<I, F>(&mut self, surfaces: F, fx: &mut Emit) -> bool
    where
        F: Fn() -> I,
        I: IntoIterator<Item = MapSurface>,
    {
        let from = self.position;
        self.position += self.velocity;
        // `wpBossBulletProcMap`: `wpBossBulletExplodeInitVars`, the sound and
        // the sparkle, then TRUE, so `wpProcessProcWeaponMain` destroys the
        // bullet that frame (ROM 0x0E86F0 returns 1): the blast never lives.
        if crate::wpmap::check_coll_end(&BOSS_BULLET_MAP_COLL, from, &mut self.position, &surfaces)
        {
            crate::sound::play_fgm(crate::sound::id::nSYAudioFGMExplodeS);
            fx.push(Fx::SparkleWhiteMultiExplode(self.position));
            return false;
        }
        true
    }
}

/// `WPAttributes::priority` of every ported weapon (`wpManagerMakeWeapon`),
/// read from each `ll*WeaponAttributes` record
/// (`crates/ssb-rom/tests/weapon_attributes.rs`).
pub const WEAPON_PRIORITY: i32 = 1;

/// `dWPSamusChargeShotWeaponAttributes` (US), generated at build time from
/// the ROM by `crates/ssb-tablegen` (`battle_data.rs`):
/// `SAMUS_CHARGE_SHOT_PRIORITIES`, the `priority` `wpSamusChargeShotLaunch`
/// writes (only the full charge outranks), and `SAMUS_CHARGE_SHOT_LEVELS`,
/// `(gfx size, X velocity, damage, attack size, map-collision size)` per
/// charge level.
pub use charge_shot::{SAMUS_CHARGE_SHOT_LEVELS, SAMUS_CHARGE_SHOT_PRIORITIES};
mod charge_shot {
    include!(concat!(env!("OUT_DIR"), "/charge_shot.rs"));
}

/// `WPCHARGESHOT_GFX_SIZE_DIV`.
pub const SAMUS_CHARGE_SHOT_GFX_SIZE_DIV: f32 = 30.0;

/// `dSamusSpecial1_ChargeShot_WeaponAttributes`: the knockback that
/// `wpSamusChargeShotLaunch` keeps while it replaces damage and size.
pub const SAMUS_CHARGE_SHOT_HITBOX: Hitbox =
    crate::monster_weapon::weapon_attributes::SAMUS_CHARGE_SHOT.hitbox();

/// `dWPSamusChargeShotWeaponAttributes`' sound IDs, a charge level each.
const fn samus_charge_shot_fgms() -> [(u16, u16, u16); 8] {
    use crate::d_arrays as d;
    let mut out = [(0, 0, 0); 8];
    let mut i = 0;
    while i < 8 {
        let shoot = d::WP_SAMUS_CHARGE_SHOT_WEAPON_ATTRIBUTES_SHOOT_FGM_ID[i];
        let charge = d::WP_SAMUS_CHARGE_SHOT_WEAPON_ATTRIBUTES_CHARGE_FGM_ID[i];
        let attack = d::WP_SAMUS_CHARGE_SHOT_WEAPON_ATTRIBUTES_ATTACK_FGM_ID[i];
        assert!(shoot <= u16::MAX as u32 && charge <= u16::MAX as u32 && attack <= u16::MAX as u32);
        out[i] = (shoot as u16, charge as u16, attack as u16);
        i += 1;
    }
    out
}

/// `dWPSamusChargeShotWeaponAttributes`' `(shoot_fgm_id, charge_fgm_id,
/// attack_fgm_id)` per charge level: the launch sound, the owner's charging
/// loop, and the hit sound `wpSamusChargeShotLaunch` gives the shot.
pub const SAMUS_CHARGE_SHOT_FGMS: [(u16, u16, u16); 8] = samus_charge_shot_fgms();

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
        let from = self.position;
        self.position += self.velocity;
        // `wpSamusChargeShotProcMap` (the shot is released).
        if crate::wpmap::check_coll_end(&coll, from, &mut self.position, &surfaces) {
            fx.push(Fx::DustExpandSmall(self.position));
            return false;
        }
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
/// offset 0x0C onward).
pub const SAMUS_BOMB_HITBOX: Hitbox = crate::monster_weapon::weapon_attributes::SAMUS_BOMB.hitbox();
pub const SAMUS_BOMB_MAP_COLL: BodyColl =
    crate::monster_weapon::weapon_attributes::SAMUS_BOMB.map_coll;

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
    /// `ga == nMPKineticsGround` (`wpMapSetGround`), with `vel_ground`.
    pub grounded: bool,
    pub vel_ground: f32,
    /// `coll_data`: the masks and the floor it stands on.
    pub coll: crate::wpmap::WeaponColl,
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
            grounded: false,
            vel_ground: 0.0,
            coll: crate::wpmap::WeaponColl::default(),
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

    /// `wpSamusBombExplodeInitVars`: `ga` is kept, so a grounded explosion
    /// still rides its line.
    fn explode(&mut self) {
        self.exploded = true;
        self.lifetime = SAMUS_BOMB_EXPLODE_LIFETIME;
        self.velocity = Vec3::ZERO;
    }

    /// `wpMainVelSetLR`.
    fn set_lr(&mut self) {
        self.lr = if self.velocity.x < 0.0 { -1.0 } else { 1.0 };
    }

    /// `wpSamusBombProcUpdate` (or the explosion's), the move of
    /// `wpProcessProcWeaponMain`, then `wpSamusBombProcMap`.
    fn tick<I, F>(&mut self, surfaces: F, fx: &mut Emit) -> bool
    where
        F: Fn() -> I,
        I: IntoIterator<Item = MapSurface>,
    {
        self.lifetime -= 1;
        if self.exploded {
            if self.lifetime == 0 {
                return false;
            }
            self.ride_line(&surfaces);
            return true;
        }
        if self.lifetime == 0 {
            // `wpSamusBombProcUpdate`.
            fx.push(Fx::SparkleWhiteMultiExplode(self.position));
            self.explode();
            crate::sound::play_fgm(crate::sound::id::nSYAudioFGMExplodeS);
            // `proc_map` is now NULL; the main pass still moves it.
            self.ride_line(&surfaces);
            return true;
        }
        if self.grounded {
            // `wpMainVelGroundTransferAir`.
            let angle = self.coll.floor_angle();
            self.velocity.x = self.lr * angle.y * self.vel_ground;
            self.velocity.y = self.lr * -angle.x * self.vel_ground;
            self.rotate_z -= SAMUS_BOMB_WAIT_ROTATE_SPEED_GROUND * self.lr;
        } else {
            crate::wpmap::apply_gravity_clamp_tvel(
                &mut self.velocity,
                SAMUS_BOMB_WAIT_GRAVITY,
                SAMUS_BOMB_WAIT_TVEL,
            );
            self.rotate_z -= SAMUS_BOMB_WAIT_ROTATE_SPEED_AIR * self.lr;
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
        let pos_prev = self.position;
        self.position += self.velocity;
        self.coll.begin();
        if !self.grounded {
            let landed = self.coll.test_all_check_floor(
                &SAMUS_BOMB_MAP_COLL,
                pos_prev,
                &mut self.position,
                &surfaces,
            );
            if self
                .coll
                .check_all_rebound(
                    &mut self.velocity,
                    MASK_CEIL | MASK_RWALL | MASK_LWALL,
                    SAMUS_BOMB_WAIT_COLLIDE_MOD_VEL,
                    self.position,
                    &SAMUS_BOMB_MAP_COLL,
                )
                .is_some()
            {
                self.set_lr();
            }
            if landed {
                crate::wpmap::reflect(&mut self.velocity, self.coll.floor_angle());
                crate::wpmap::scale(&mut self.velocity, SAMUS_BOMB_FLOOR_MOD_VEL);
                self.set_lr();
                if crate::wpmap::mag(self.velocity) < SAMUS_BOMB_GROUND_MIN_SPEED {
                    // `wpMapSetGround`.
                    self.grounded = true;
                    self.vel_ground = self.velocity.x * self.lr;
                }
            }
        } else if !self.coll.test_lr_wall_check_floor(
            &SAMUS_BOMB_MAP_COLL,
            pos_prev,
            &mut self.position,
            &surfaces,
        ) {
            // `wpMapSetAir`.
            self.grounded = false;
        }
        true
    }

    /// `wpProcessProcWeaponMain` without a `proc_map`: a grounded weapon
    /// rides its line's `vel_speed` while the line exists.
    fn ride_line<I, F>(&mut self, surfaces: &F)
    where
        F: Fn() -> I,
        I: IntoIterator<Item = MapSurface>,
    {
        self.position += self.velocity;
        if let Some(floor) = self.coll.floor.standing().filter(|_| self.grounded) {
            if crate::map::line_exists(surfaces, floor.line) {
                self.position += crate::map::line_speed(surfaces, floor.line);
            }
        }
    }
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

/// `llLinkSpecial1BoomerangWeaponAttributes` (`226_LinkSpecial1.c`, US).
pub const LINK_BOOMERANG_HITBOX: Hitbox =
    crate::monster_weapon::weapon_attributes::LINK_BOOMERANG.hitbox();
pub const LINK_BOOMERANG_MAP_COLL: BodyColl =
    crate::monster_weapon::weapon_attributes::LINK_BOOMERANG.map_coll;

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
    /// `wp->p_sfx`/`sfx_id`: the flight sound (`wpMainPlayFGM`).
    pub sfx: Option<crate::sound::FgmHandle>,
    /// `coll_data`: the masks whose fresh bits bound it.
    pub coll: crate::wpmap::WeaponColl,
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
            sfx: None,
            coll: crate::wpmap::WeaponColl::default(),
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
            ssb_engine::math::sy_arc_tan2(y as f32, i32::from(stick_x).abs() as f32)
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
        // `wpMainPlayFGM(wp, nSYAudioFGMLinkSpecialNReturn)`, the source's
        // last line; nothing before it plays or reads a sound.
        self.sfx = play_weapon_fgm(self.sfx, crate::sound::id::nSYAudioFGMLinkSpecialNReturn);
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
        let mut angle = ssb_engine::math::sy_arc_tan2(dist_y, dist_x);
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
                self.default_angle = clamp_angle_360(ssb_engine::math::sy_arc_tan2(
                    self.velocity.y,
                    self.velocity.x,
                ));
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
        let pos_prev = self.position;
        self.position += self.velocity;
        // `wpLinkBoomerangProcMap`.
        self.coll.begin();
        if self.is_reflect || self.is_return {
            return (true, false);
        }
        self.coll.test_all(
            &LINK_BOOMERANG_MAP_COLL,
            pos_prev,
            &mut self.position,
            &surfaces,
        );
        let fresh = self.coll.fresh();
        if fresh != 0 {
            fx.push(Fx::DustCollide(self.position));
            let mut bound = false;
            for (mask, normal) in [
                (MASK_LWALL, self.coll.lwall_angle),
                (MASK_RWALL, self.coll.rwall_angle),
                (MASK_CEIL, self.coll.ceil_angle),
                (MASK_FLOOR, self.coll.floor_angle()),
            ] {
                if fresh & mask != 0 {
                    bound |= self.bound(normal);
                }
            }
            if bound {
                self.set_return(true);
            }
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
    fn reflect(&mut self, reflector: &Reflector) {
        if !self.is_reflect {
            self.is_reflect = true;
            self.is_return = false;
            self.is_forward = false;
            self.lifetime = BOOMERANG_LIFETIME_REFLECT;
        }
        let dist_x = self.position.x - reflector.pos.x;
        let dist_y = self.position.y - (reflector.pos.y + 250.0);
        self.default_angle = clamp_angle_360(ssb_engine::math::sy_arc_tan2(dist_y, dist_x));
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
/// `247_YoshiMain.c`, US).
pub const YOSHI_EGG_HITBOX: Hitbox =
    crate::monster_weapon::weapon_attributes::YOSHI_EGG_THROW.hitbox();
pub const YOSHI_EGG_MAP_COLL: BodyColl =
    crate::monster_weapon::weapon_attributes::YOSHI_EGG_THROW.map_coll;

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
    /// The `DObj` scale the held egg had at the throw, presentation only:
    /// kind 46 draws X by `scale.x` and Y by `scale.y`.
    pub scale: Vec3,
}

impl YoshiEgg {
    fn new(spawn: WeaponSpawn, throw_force: i16, stick_x: i8, scale: Vec3) -> Self {
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
            scale,
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
            // `wpYoshiEgg{Hit,Expire}InitVars` write the size raw; only
            // `wpManagerMakeWeapon` halves the attribute's.
            radius: if self.exploded {
                EGGTHROW_EXPLODE_SIZE
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
                crate::sound::play_fgm(crate::sound::id::nSYAudioFGMYoshiEggShatter1);
                fx.push(Fx::YoshiEggExplode(self.position));
                fx.push(Fx::EggBreak(self.position));
                self.explode();
                return true;
            }
            self.rotate_z += self.spin_step;
            crate::wpmap::apply_gravity_clamp_tvel(
                &mut self.velocity,
                EGGTHROW_GRAVITY,
                EGGTHROW_TVEL,
            );
        } else {
            self.is_spin = true;
            self.velocity = Self::launch_velocity(self.throw_force, self.stick_x, self.lr);
            self.spin_step = (f32::from(self.throw_force) * EGGTHROW_ANGLE_FORCE_MUL
                + EGGTHROW_ANGLE_ADD)
                .to_radians();
            self.position.z = 0.0;
        }
        let from = self.position;
        self.position += self.velocity;
        // `wpYoshiEggThrowProcMap`: the egg bursts where it stopped.
        if crate::wpmap::check_coll_end(&YOSHI_EGG_MAP_COLL, from, &mut self.position, &surfaces) {
            fx.push(Fx::Quake(2));
            crate::sound::play_fgm(crate::sound::id::nSYAudioFGMYoshiEggShatter1);
            fx.push(Fx::YoshiEggExplode(self.position));
            fx.push(Fx::EggBreak(self.position));
            fx.push(Fx::DustExpandSmall(self.position));
            self.explode();
        }
        true
    }

    /// `wpYoshiEggThrowProcReflector`, then the reflect damage bonus.
    fn reflect(&mut self, reflector: &Reflector) {
        self.lifetime = EGGTHROW_LIFETIME;
        self.owner_port = reflector.port;
        if self.velocity.x * reflector.lr < 0.0 {
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
/// US).
pub const YOSHI_STAR_HITBOX: Hitbox = crate::monster_weapon::weapon_attributes::YOSHI_STAR.hitbox();

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
    fn reflect(&mut self, reflector: &Reflector) {
        self.lifetime = YOSHISTAR_LIFETIME;
        self.owner_port = reflector.port;
        if self.velocity.x * reflector.lr < 0.0 {
            self.velocity.x = -self.velocity.x;
        }
        self.rotate_z = ssb_engine::math::sy_arc_tan2(self.velocity.y, self.velocity.x);
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
/// `llKirbyMainCutterWeaponAttributes` in `229_KirbyMain.c` (US).
pub const KIRBY_CUTTER_HITBOX: Hitbox =
    crate::monster_weapon::weapon_attributes::KIRBY_CUTTER.hitbox();
pub const KIRBY_CUTTER_MAP_COLL: BodyColl =
    crate::monster_weapon::weapon_attributes::KIRBY_CUTTER.map_coll;

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
    /// `ga == nMPKineticsGround` (`wpMapSetGround`), with `vel_ground`.
    pub grounded: bool,
    pub vel_ground: f32,
    /// `coll_data`: the masks and the floor it rides.
    pub coll: crate::wpmap::WeaponColl,
    pub hit_ports: u8,
    /// `wpMainVelSetModelPitch`'s root `rotate.y`: +90 degrees for a
    /// rightward `vel_air.x`, else -90. Presentation only.
    pub model_rotate_y: f32,
    /// Root `rotate.z`: `wpKirbyCutterProcUpdate` sets it to the floor's
    /// slope while grounded; leaving the floor aims the flight by it.
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
    fn new(spawn: WeaponSpawn) -> Self {
        let lr = if spawn.facing < 0.0 { -1.0 } else { 1.0 };
        Self {
            owner_port: spawn.owner_port,
            position: spawn.position,
            velocity: Vec3::new(lr * KIRBY_CUTTER_VEL, 0.0, 0.0),
            lifetime: KIRBY_CUTTER_LIFETIME,
            lr,
            damage: KIRBY_CUTTER_HITBOX.damage,
            grounded: false,
            vel_ground: 0.0,
            coll: crate::wpmap::WeaponColl::default(),
            hit_ports: 0,
            model_rotate_y: model_pitch(lr),
            rotate_z: 0.0,
            anim_ticks: 0,
        }
    }

    /// `wpKirbyCutterMakeWeapon` after `wpManagerMakeWeapon`: Kirby's
    /// `floor_line_id` grounds the wave if it lies under the maker's
    /// unsnapped `pos` (`mpCollisionGetFCCommonFloor`), which writes the
    /// line and `floor_angle`; `floor_flags` keeps the spawn pass's value.
    fn make_floor<I, F>(&mut self, line: Option<u16>, pos: Vec3, surfaces: &F)
    where
        F: Fn() -> I,
        I: IntoIterator<Item = MapSurface>,
    {
        let Some(line) = line else { return };
        if let Some((_, floor)) = crate::map::floor_point(surfaces, line, pos.x) {
            self.coll.floor.line = Some(floor.line);
            self.coll.floor.angle = floor.normal;
            self.set_ground();
        }
    }

    /// `wpMapSetGround`.
    fn set_ground(&mut self) {
        self.grounded = true;
        self.vel_ground = self.velocity.x * self.lr;
    }

    /// `wpKirbyCutterProcUpdate`, the move of `wpProcessProcWeaponMain`,
    /// then `wpKirbyCutterProcMap`.
    fn tick<I, F>(&mut self, surfaces: F, fx: &mut Emit) -> bool
    where
        F: Fn() -> I,
        I: IntoIterator<Item = MapSurface>,
    {
        // `wpProcessProcWeaponMain` plays the DObj animation before
        // `proc_update`.
        self.anim_ticks = self.anim_ticks.wrapping_add(1);
        self.lifetime -= 1;
        if self.lifetime == 0 {
            fx.push(Fx::DustExpandSmall(self.position));
            return false;
        }
        if self.grounded {
            // `-syUtilsArcTan2(floor_angle.x, floor_angle.y)`.
            let angle = self.coll.floor_angle();
            self.rotate_z = -ssb_engine::math::sy_arc_tan2(angle.x, angle.y);
        }
        // `vel_air` stays level on the floor: the ground pass puts the wave
        // back on its line.
        let pos_prev = self.position;
        self.position += self.velocity;
        self.coll.begin();
        if !self.grounded {
            if self.coll.test_all_check_floor(
                &KIRBY_CUTTER_MAP_COLL,
                pos_prev,
                &mut self.position,
                &surfaces,
            ) {
                self.set_ground();
            }
        } else if !self.coll.test_lr_wall_check_floor(
            &KIRBY_CUTTER_MAP_COLL,
            pos_prev,
            &mut self.position,
            &surfaces,
        ) {
            // `wpMapSetAir`: the flight follows the last slope.
            self.grounded = false;
            let (sin, cos) = sin_cos(self.rotate_z);
            self.velocity.x = cos * self.vel_ground;
            self.velocity.y = sin * self.vel_ground;
            if self.model_rotate_y < 0.0 {
                self.velocity.x = -self.velocity.x;
                self.velocity.y = -self.velocity.y;
            }
        }
        if self.coll.mask_curr & (MASK_CEIL | MASK_RWALL | MASK_LWALL) != 0 {
            fx.push(Fx::DustExpandSmall(self.position));
            return false;
        }
        true
    }

    /// `wpKirbyCutterProcReflector`, then the reflect damage bonus.
    fn reflect(&mut self, reflector: &Reflector) {
        self.lifetime = KIRBY_CUTTER_LIFETIME;
        self.owner_port = reflector.port;
        if self.velocity.x * reflector.lr < 0.0 {
            self.velocity.x = -self.velocity.x;
            self.velocity.y = -self.velocity.y;
        }
        self.lr = -self.lr;
        self.model_rotate_y = model_pitch(self.velocity.x);
        self.hit_ports = 0;
        self.damage = ((self.damage as f32 * 1.8 + 0.99) as i32).min(100);
    }
}

/// One of Ness's weapons, as [`WeaponPool::ness_shots`] lists them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NessShot {
    Fire(PKFire),
    Head(PKThunder),
    Trail(PKThunderTrail),
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
    BossBullet(BossBullet),
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
    /// `coll_data`: the masks `wpMapCheckAllRebound` gates on.
    pub coll: crate::wpmap::WeaponColl,
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
            coll: crate::wpmap::WeaponColl::default(),
        }
    }

    pub fn attributes(&self) -> &'static FireballAttributes {
        &FIREBALL_ATTRIBUTES[self.index as usize]
    }

    /// `wpMarioFireballProcUpdate`, the move of `wpProcessProcWeaponMain`,
    /// then `wpMarioFireballProcMap`: `wpMapTestAll`, then a rebound off
    /// any surface first touched this frame.
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
        crate::wpmap::apply_gravity_clamp_tvel(&mut self.velocity, attr.gravity, attr.vel_terminal);
        self.rotate_x += attr.rotate_speed;
        let pos_prev = self.position;
        self.position += self.velocity;
        self.coll.begin();
        self.coll.test_all(
            &MARIO_FIREBALL_MAP_COLL,
            pos_prev,
            &mut self.position,
            &surfaces,
        );
        if self
            .coll
            .check_all_rebound(
                &mut self.velocity,
                crate::wpmap::MASK_MAIN,
                attr.rebound,
                self.position,
                &MARIO_FIREBALL_MAP_COLL,
            )
            .is_some()
        {
            if crate::wpmap::mag(self.velocity) < attr.vel_min {
                fx.push(Fx::DustExpandSmall(self.position));
                return false;
            }
            fx.push(Fx::FireGrind(self.position));
        }
        true
    }
}

/// The portable stand-in for `wpManager`'s live-object list: fixed-size so
/// PSP gameplay stays allocation-free, with `WEAPON_ALLOC_MAX`'s 32 structs
/// (`wp/wpdef.h`), so a four-fighter match spawns every weapon the N64
/// would. `wpManagerMakeWeapon` fails, as [`WeaponPool`]'s spawn does, only
/// once all 32 are live.
pub const MAX_WEAPONS: usize = 32;

/// `wpManagerAllocWeapons` links struct `i` to `i + 1`;
/// `wpManagerGetNextStructAlloc` pops the head and
/// `wpManagerSetPrevStructAlloc` pushes a freed struct back on it.
const WEAPON_FREE_ORDER: FreeOrder = FreeOrder::Lifo;

type WeaponSlots = ObjectPool<Weapon, MAX_WEAPONS>;

/// Player ports the pool keeps owner views and catch events for.
const MAX_OWNERS: usize = 4;

#[derive(Debug, Clone, PartialEq)]
pub struct WeaponPool {
    star_rod_smash_desc: bool,
    /// `sWPManagerStructsAllocFree` (`wpManagerAllocWeapons`): the
    /// structs, last freed first handed out ([`WEAPON_FREE_ORDER`]).
    slots: WeaponSlots,
    /// PK Fire flames the sparks' hit callbacks made this frame, for
    /// [`crate::item::ItemPool::take_weapon_spawns`]: the first
    /// `item_spawn_len` slots, all `Some`. Slots past it are never read, so
    /// no unwritten `None` payload reaches a float operation (RE-531).
    item_spawns: [Option<crate::item::PKFireSpawn>; MAX_WEAPONS],
    item_spawn_len: usize,
    /// Four `WPAttackColl` victim records, shared by fighters and items.
    /// Fighters use ports; items use `ITEM_RECORD_BASE + slot`.
    hit_records: [[Option<u8>; 4]; MAX_WEAPONS],
    /// Item searches queue `hit_normal_damage`; callbacks run after search.
    pending_item_hits: [bool; MAX_WEAPONS],
    /// `wp->hit_normal_damage` from a fighter's search this frame: the
    /// `proc_hit` that [`Self::react_to_fighter_hits`] runs once every
    /// fighter has searched.
    proc_hit: [bool; MAX_WEAPONS],
    /// The rest of a fighter search's `wpProcessProcHitCollisions` inputs:
    /// [`Self::react_to_fighter_hits`] runs them after every fighter's
    /// search, as it runs [`Self::proc_hit`].
    pending: [PendingProcs; MAX_WEAPONS],
    /// This frame's [`OwnerView`] per port, from [`Self::observe_owner`].
    owners: [Option<OwnerView>; MAX_OWNERS],
    /// A returning Boomerang reached this port's thrower while its
    /// `is_special_interrupt` was set; [`Self::sync_owner`] delivers it.
    caught: [bool; MAX_OWNERS],
    thunder_destroyed: [bool; MAX_OWNERS],
    /// Ness's `is_thunder_destroy` writes from the hit collisions, for
    /// [`Self::sync_hit_owner`].
    ness_thunder_destroyed: [bool; MAX_OWNERS],
    next_group: u16,
    /// Each slot's [`WeaponSpawn::stale`].
    stale: [crate::stale::WeaponStale; MAX_WEAPONS],
    /// Each slot's `wp->team`: its [`WeaponSpawn::team`], a parent weapon's
    /// for a trail, and its reflector's once reflected.
    teams: [u8; MAX_WEAPONS],
    /// Each slot's `wp->lr` as `wpManagerMakeWeapon` sets it: the owner's
    /// facing, a parent weapon's for a trail, +1 for a ground weapon. The
    /// kinds that keep their own `lr` (or rederive it from their velocity)
    /// answer from that instead ([`Self::cpu_lr`]).
    lrs: [f32; MAX_WEAPONS],
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
    /// runs in [`Self::react_to_fighter_hits`]), so a clash runs no second
    /// one.
    set_off: [bool; MAX_WEAPONS],
    /// Each record's `timer_rehit` (`WEAPON_REHIT_TIME_DEFAULT` on a
    /// `can_rehit_fighter` item weapon's hit; zero otherwise).
    rehit: [[u8; 4]; MAX_WEAPONS],
    /// Onix's rocks' events for [`crate::item::ItemPool::observe_weapons`].
    rock_events: [Option<(u32, bool)>; 2 * MAX_WEAPONS],
    /// The battle's motion and stat counts ([`crate::manager_counts`]).
    pub counts: crate::manager_counts::ManagerCounts,
    /// The weapons as `wpProcessProcSearchHitWeapon` reads them, taken
    /// before any hit reacts ([`Self::begin_clashes`]).
    clash_view: ClashView,
}

/// The weapons at priority 1, before the port's at-once reactions
/// ([`WeaponPool::begin_clashes`]): link order, each slot's clash attack
/// and group, its attack for the item searches, and whether a slot was
/// refilled since.
#[derive(Debug, Clone, Copy, PartialEq)]
struct ClashView {
    taken: bool,
    order: [usize; MAX_WEAPONS],
    count: usize,
    attacks: [Option<ClashAttack>; MAX_WEAPONS],
    /// [`WeaponPool::item_attack`] before any fighter's search reacts.
    item_attacks: [Option<ItemAttack>; MAX_WEAPONS],
    groups: [Option<u16>; MAX_WEAPONS],
    refilled: [bool; MAX_WEAPONS],
}

impl Default for ClashView {
    fn default() -> Self {
        ClashView {
            taken: false,
            order: [0; MAX_WEAPONS],
            count: 0,
            attacks: [None; MAX_WEAPONS],
            item_attacks: [None; MAX_WEAPONS],
            groups: [None; MAX_WEAPONS],
            refilled: [false; MAX_WEAPONS],
        }
    }
}

/// A weapon's attack as the item searches read it: owner, staled hitbox,
/// position, velocity and previous position.
type ItemAttack = (u8, Hitbox, Vec3, Vec3, Vec3);

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

/// `wpMainStopFGM`.
pub(crate) fn stop_weapon_fgm(sfx: Option<crate::sound::FgmHandle>) {
    if let Some(h) = sfx {
        crate::sound::stop_fgm(h);
    }
}

/// `wpMainPlayFGM`: stops the weapon's sound, then plays `sfx_id` in its
/// place.
pub(crate) fn play_weapon_fgm(
    sfx: Option<crate::sound::FgmHandle>,
    sfx_id: u16,
) -> Option<crate::sound::FgmHandle> {
    stop_weapon_fgm(sfx);
    crate::sound::play_fgm(sfx_id)
}

/// `wpMainDestroyWeapon`: the slot empties and the weapon's sound stops.
fn destroy(slots: &mut WeaponSlots, i: usize) {
    if let Some(w) = slots.remove_at(i) {
        stop_weapon_fgm(w.sfx());
    }
}

impl Weapon {
    /// `wp->p_sfx`: only the Boomerang and the ground Thunder Jolt keep one.
    fn sfx(&self) -> Option<crate::sound::FgmHandle> {
        match self {
            Weapon::Boomerang(b) => b.sfx,
            Weapon::Jolt(j) => j.sfx,
            _ => None,
        }
    }

    /// `wp->attack_coll.fgm_id`: the weapon's `WPAttributes::sfx`, which
    /// `ftMainUpdateDamageStatWeapon` and `itProcessUpdateDamageStatWeapon`
    /// play on a hit.
    fn fgm_id(&self) -> u16 {
        use crate::item_sounds::weapon as wp;
        use crate::monster_weapon::ShotKind as K;
        match self {
            Weapon::Fireball(f) if f.index == 0 => wp::MARIO_FIREBALL,
            Weapon::Fireball(_) => wp::LUIGI_FIREBALL,
            Weapon::Blaster(_) => wp::FOX_BLASTER,
            // `wpSamusChargeShotLaunch` replaces the attributes' sound.
            Weapon::ChargeShot(c) => SAMUS_CHARGE_SHOT_FGMS[usize::from(c.charge)].2,
            Weapon::Bomb(_) => wp::SAMUS_BOMB,
            Weapon::Boomerang(_) => wp::LINK_BOOMERANG,
            Weapon::Egg(_) => wp::YOSHI_EGG_THROW,
            Weapon::Star(_) => wp::YOSHI_STAR,
            Weapon::Cutter(_) => wp::KIRBY_CUTTER,
            Weapon::Jolt(j) if j.grounded() => wp::PIKACHU_THUNDER_JOLT_GROUND,
            Weapon::Jolt(_) => wp::PIKACHU_THUNDER_JOLT_AIR,
            Weapon::Thunder(_) => wp::PIKACHU_THUNDER_HEAD,
            Weapon::Trail(_) => wp::PIKACHU_THUNDER_TRAIL,
            Weapon::PKFire(_) => wp::NESS_PK_FIRE,
            Weapon::PKThunder(_) => wp::NESS_PK_THUNDER,
            Weapon::PKTrail(_) => wp::NESS_PK_THUNDER_TRAIL,
            Weapon::Laser(l) if l.three_d => wp::ARWING_LASER_3D,
            Weapon::Laser(_) => wp::ARWING_LASER_2D,
            Weapon::BossBullet(b) if b.hard => wp::BOSS_BULLET_HARD,
            Weapon::BossBullet(_) => wp::BOSS_BULLET_NORMAL,
            Weapon::Monster(m) => match m.kind {
                K::HitokageFlame => wp::HITOKAGE_FLAME,
                K::FushigibanaRazor => wp::FUSHIGIBANA_RAZOR,
                K::IwarkRock => wp::WARK_ROCK,
                K::NyarsCoin => wp::NYARS_COIN,
                K::LizardonFlame => wp::LIZARDON_FLAME,
                K::SpearSwarm => wp::SPEAR_SWARM,
                K::PippiSwarm => wp::PIPPI_SWARM,
                K::KamexHydro => wp::KAMEX_HYDRO,
                K::StarmieSwift => wp::STARMIE_SWIFT,
                K::DogasSmog => wp::DOGAS_SMOG,
                K::RayGun => wp::LGUN_AMMO,
                K::StarRod if m.star_smash => wp::STARROD_SMASH,
                K::StarRod => wp::STARROD,
                K::FireFlower => wp::FFLOWER_FLAME,
            },
        }
    }

    /// `wp->attack_coll.can_*`.
    fn flags(&self) -> WeaponFlags {
        match self {
            // Mario and Luigi Fireball, Charge Shot, Yoshi Star, PK Fire,
            // aerial Thunder Jolt.
            Weapon::Fireball(_) | Weapon::ChargeShot(_) | Weapon::Star(_) | Weapon::PKFire(_) => {
                wflags(true, true, true, true)
            }
            Weapon::Jolt(j) => wflags(true, !j.grounded(), true, true),
            Weapon::Blaster(_) => wflags(false, true, true, true),
            // The 2D shot hops, reflects and is absorbed; the 3D shot is
            // only absorbed, before and after it bursts.
            Weapon::Laser(l) => wflags(false, !l.three_d, !l.three_d, true),
            Weapon::Monster(m) => {
                let a = m.attributes();
                wflags(a.can_setoff, a.can_hop, a.can_reflect, a.can_absorb)
            }
            Weapon::Bomb(_) => wflags(false, true, false, false),
            // The burst clears `can_hop` and `can_reflect` only.
            Weapon::BossBullet(_) => wflags(false, true, true, true),
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
            Weapon::Jolt(j) => j.grounded(),
            Weapon::Cutter(c) => c.grounded,
            Weapon::Bomb(b) => b.grounded,
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
            Weapon::BossBullet(w) => w.position,
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

    /// `WEAPON_FLAG_COLLPROJECT` makers' own `coll_data.map_coll`, their
    /// translate and, for the kinds that keep one, their `coll_data` floor.
    /// `None` for kinds made without the flag (Boomerang, Pikachu Thunder,
    /// PK Thunder trail, Sector lasers, item-made shots) and for the Star
    /// Rod star, whose maker overwrites the translate with the unsnapped
    /// `pos` after `wpManagerMakeWeapon` (itstarrod.c:391).
    fn collproject(&mut self) -> Option<(BodyColl, &mut Vec3, Option<&mut FloorState>)> {
        use crate::monster_weapon::weapon_attributes as wa;
        Some(match self {
            Weapon::Fireball(w) => {
                let coll = if w.index == 0 {
                    MARIO_FIREBALL_MAP_COLL
                } else {
                    wa::LUIGI_FIREBALL.map_coll
                };
                (coll, &mut w.position, Some(&mut w.coll.floor))
            }
            Weapon::Blaster(w) => (FOX_BLASTER_MAP_COLL, &mut w.position, None),
            Weapon::ChargeShot(w) => (wa::SAMUS_CHARGE_SHOT.map_coll, &mut w.position, None),
            Weapon::Bomb(w) => (
                SAMUS_BOMB_MAP_COLL,
                &mut w.position,
                Some(&mut w.coll.floor),
            ),
            Weapon::Egg(w) => (YOSHI_EGG_MAP_COLL, &mut w.position, None),
            Weapon::Star(w) => (wa::YOSHI_STAR.map_coll, &mut w.position, None),
            Weapon::Cutter(w) => (
                KIRBY_CUTTER_MAP_COLL,
                &mut w.position,
                Some(&mut w.coll.floor),
            ),
            Weapon::Jolt(w) => {
                let (coll, pos, floor) = w.collproject();
                (coll, pos, Some(floor))
            }
            Weapon::PKFire(w) => (wa::NESS_PK_FIRE.map_coll, &mut w.position, None),
            Weapon::PKThunder(w) => (wa::NESS_PK_THUNDER.map_coll, &mut w.position, None),
            Weapon::BossBullet(w) => {
                let coll = if w.hard {
                    wa::BOSS_BULLET_HARD.map_coll
                } else {
                    wa::BOSS_BULLET_NORMAL.map_coll
                };
                (coll, &mut w.position, None)
            }
            Weapon::Monster(m) if matches!(m.kind, ShotKind::FireFlower | ShotKind::RayGun) => {
                (m.attributes().map_coll, &mut m.position, None)
            }
            Weapon::Monster(_)
            | Weapon::Boomerang(_)
            | Weapon::Thunder(_)
            | Weapon::Trail(_)
            | Weapon::PKTrail(_)
            | Weapon::Laser(_) => return None,
        })
    }

    /// `wpManagerMakeWeapon`'s `WEAPON_FLAG_COLLPROJECT` pass
    /// (`mpCommonRunWeaponCollisionDefault`): one default-collision pass from
    /// the parent's translate to the spawn point, which moves the weapon out
    /// of walls, ceilings and floors its parent's body did not cross, and
    /// projects the floor below it. `mask_curr` is reset after the pass
    /// (`mpCommonResetCollDataStats`), so no mask survives.
    fn snap<I, F>(&mut self, parent: &SpawnParent, surfaces: &F)
    where
        F: Fn() -> I,
        I: IntoIterator<Item = MapSurface>,
    {
        let Some((coll, pos, floor)) = self.collproject() else {
            return;
        };
        let r = crate::map::run_default_collision_project(
            &coll,
            &parent.coll,
            parent.pos,
            *pos,
            surfaces,
        );
        *pos = r.pos;
        if let Some(floor) = floor {
            floor.project(r.floor);
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
            Weapon::Fireball(_) => {
                crate::sound::play_fgm(crate::sound::id::nSYAudioFGMExplodeS);
                fx.push(Fx::SparkleWhite(pos));
            }
            // `wpFoxBlasterProcHit`.
            Weapon::Blaster(_) => fx.push(Fx::FoxBlasterGlow(pos)),
            // `grSectorArwingWeaponLaser2DProcHit` for all four;
            // `...3DProcHit` and `...3DProcAbsorb` burst, and the burst
            // clears them.
            Weapon::Laser(l) => {
                if !l.three_d {
                    fx.push(shock(l.damage));
                } else if !l.exploded {
                    crate::sound::play_fgm(crate::sound::id::nSYAudioFGMExplodeS);
                    fx.push(Fx::SparkleWhiteMultiExplode(pos));
                }
            }
            // `wpSamusChargeShotProcHit`.
            Weapon::ChargeShot(c) => fx.push(shock(c.damage)),
            // `wpSamusBombProcHit` and `...ProcAbsorb`; the explosion
            // clears them.
            Weapon::Bomb(b) => {
                if !b.exploded {
                    crate::sound::play_fgm(crate::sound::id::nSYAudioFGMExplodeS);
                    fx.push(Fx::SparkleWhiteMultiExplode(pos));
                }
            }
            // `wpLinkBoomerangProcHit`, `...ProcShield`, `...ProcSetOff`.
            Weapon::Boomerang(_) => {}
            // `wpYoshiEggThrowProcHit` (no `proc_absorb`); the explosion
            // clears it.
            Weapon::Egg(e) => {
                if !e.exploded && proc != Proc::Absorb {
                    crate::sound::play_fgm(crate::sound::id::nSYAudioFGMYoshiEggShatter1);
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
                    crate::sound::play_fgm(crate::sound::id::nSYAudioFGMExplodeS);
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
            // `wpBossBulletProcHit` for all four.
            Weapon::BossBullet(_) => fx.push(Fx::SparkleWhiteMultiExplode(pos)),
        }
    }

    /// `wpProcessProcHitCollisions`'s shield branch after
    /// `ftMainUpdateShieldStatWeapon` recorded the fighter: an airborne
    /// `can_hop` weapon that met the shield under 135 degrees hops, any
    /// other runs its `proc_shield`. Returns whether the weapon lives on.
    fn on_shield(&mut self, shield: crate::combat::ShieldCollide, fx: &mut Emit) -> bool {
        // The Bomb's burst clears `proc_hop` and `proc_shield`.
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
            // `wpKirbyCutterProcShield` and `wpNessPKThunderTrailProcHit`
            // return FALSE.
            Weapon::Cutter(_) | Weapon::PKTrail(_) => {}
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
                s.rotate_z = ssb_engine::math::sy_arc_tan2(s.velocity.y, s.velocity.x);
                s.lr = if s.velocity.x > 0.0 { 1.0 } else { -1.0 };
            }
            // `wpPikachuThunderJoltAirProcHop`.
            Weapon::Jolt(j) => j.velocity = turn(j.velocity),
            // `wpBossBulletProcHop`.
            Weapon::BossBullet(b) => b.velocity = turn(b.velocity),
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
/// A Thunder trail's second attack offset: the authored -240 at DObj scale
/// 0.5 (`wpProcessUpdateHitOffsets`).
fn trail_low_attack(t: ThunderTrail) -> Vec3 {
    t.position - Vec3::new(0.0, 120.0, 0.0)
}

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

/// `wpProcessProcHitCollisions`' reads of `reflect_gobj`: the reflector as
/// its search left it (nothing moves a fighter between its search, at
/// priority 1, and the weapons' pass at priority 0).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Reflector {
    pub(crate) port: u8,
    pub(crate) team: u8,
    /// `fp->lr`.
    pub(crate) lr: f32,
    /// `DObjGetStruct(fighter_gobj)->translate`.
    pub(crate) pos: Vec3,
    /// `fp->stat_flags` and `fp->stat_count` (US: `reflect_stat_flags`).
    pub(crate) stat: crate::spgame::live::AttackStat,
    /// `fp->handicap`.
    pub(crate) handicap: u8,
}

impl Reflector {
    pub(crate) fn of(f: &Fighter) -> Self {
        Reflector {
            port: f.port,
            team: f.team,
            lr: f.facing.sign(),
            pos: f.pos,
            stat: f.stats.attack,
            handicap: f.handicap,
        }
    }
}

/// What the fighters' searches left for one weapon's
/// `wpProcessProcHitCollisions` besides `hit_normal_damage`
/// ([`WeaponPool::react_to_fighter_hits`]).
#[derive(Debug, Clone, Copy, PartialEq)]
struct PendingProcs {
    /// `hit_shield_damage` and the first shield's `shield_collide_angle`
    /// and `shield_collide_dir` (`ftMainUpdateShieldStatWeapon` keeps the
    /// first: every fighter reads the same staled damage).
    shield: Option<crate::combat::ShieldCollide>,
    /// `hit_attack_damage` from a fighter's attack: `proc_setoff`.
    set_off: bool,
    /// `reflect_gobj`: the last reflector searched.
    reflect: Option<Reflector>,
    /// `absorb_gobj`: `proc_absorb`.
    absorb: bool,
}

impl PendingProcs {
    const NONE: PendingProcs = PendingProcs {
        shield: None,
        set_off: false,
        reflect: None,
        absorb: false,
    };
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
        stat: crate::spgame::live::AttackStat::default(),
        object: crate::spgame::bonus::DamageObject::Other,
        hitbox,
        pos_curr: position,
        pos_prev: prev,
        source: crate::combat::HitSource::Weapon { vel_x: velocity.x },
        // Read by no clank, reflect or absorb check.
        handicap: crate::stale::HANDICAP_DEFAULT,
        can_shield: true,
        owner: Some(owner),
        is_hitlag_victim: None,
        fgm_id: None,
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
    team: u8,
    landed: &'a mut Option<(u8, crate::stale::MotionAttackId, u16)>,
    rules: crate::team::TeamRules,
    slot: usize,
    /// `hit_normal_damage`.
    proc_hit: &'a mut bool,
    pending: &'a mut PendingProcs,
}

/// `ftMainSearchHitWeapon` for one of Sector Z's Arwing lasers against one
/// fighter; its callbacks wait in `h` ([`laser_procs`]). Its hits credit no
/// fighter until a reflector takes it, and the 3D shot and its burst pass
/// shields (`can_shield` clear).
fn laser_hit(laser: &ArwingLaser, h: LaserHit<'_>, defender: &mut Fighter) {
    let owner = laser.owner_port;
    if owner == defender.port || h.rules.spares(defender.team, h.team) {
        return;
    }
    let bit = 1u8 << (defender.port & 7);
    if h.ports & bit != 0 {
        return;
    }
    let mut staled = laser.hitbox();
    staled.damage = h.stale.damage(staled.damage);
    let owner_player = (owner != sector::GROUND_PORT).then_some(owner);
    let laser_fgm = Weapon::Laser(*laser).fgm_id();
    let attack = |pos: Vec3, vel: Vec3, can_shield: bool| crate::combat::WeaponAttack {
        stat: h.stale.stat,
        object: crate::spgame::bonus::DamageObject::Arwing,
        hitbox: staled,
        pos_curr: pos,
        pos_prev: pos - vel,
        source: crate::combat::HitSource::Weapon { vel_x: vel.x },
        handicap: h.stale.handicap,
        can_shield,
        owner: owner_player,
        is_hitlag_victim: None,
        fgm_id: Some(laser_fgm),
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
        return;
    }
    let flags = wflags(false, !laser.three_d, !laser.three_d, true);
    let pre = pre_hit(
        defender,
        flags,
        false,
        owner,
        h.team,
        h.rules,
        h.slot,
        staled,
        laser.position,
        laser.velocity,
    );
    match pre {
        PreHit::None | PreHit::SetOff => {}
        // `ftMainUpdateReflectorStatWeapon` records the reflector.
        PreHit::Reflected => {
            record_weapon_victim(h.records, defender.port);
            h.pending.reflect = Some(Reflector::of(defender));
            return;
        }
        // The 2D shot's `proc_hit` (the 3D shot cannot be reflected).
        PreHit::ReflectorBroke => {
            record_weapon_victim(h.records, defender.port);
            *h.proc_hit = true;
            return;
        }
        PreHit::Absorbed => {
            record_weapon_victim(h.records, defender.port);
            h.pending.absorb = true;
            return;
        }
    }
    let contact = crate::combat::weapon_hit(
        defender,
        attack(laser.position, laser.velocity, laser.can_shield()),
    );
    if let crate::combat::WeaponContact::Shielded(shield) = contact {
        record_weapon_victim(h.records, defender.port);
        h.pending.shield.get_or_insert(shield);
        return;
    }
    if attack::HitOutcome::of(contact).registered() {
        record_weapon_victim(h.records, defender.port);
        if contact == crate::combat::WeaponContact::Hurt(true) {
            landed(h.landed);
        }
        *h.proc_hit = true;
    }
}

/// `wpProcessProcHitCollisions` for one of Sector Z's Arwing lasers, in
/// its order: `proc_hit`, the shield, `proc_reflector`, `proc_absorb`.
/// Returns whether the laser lives on.
fn laser_procs(
    laser: &mut ArwingLaser,
    proc_hit: bool,
    p: PendingProcs,
    records: &mut [Option<u8>; 4],
    stale: &mut crate::stale::WeaponStale,
    team: &mut u8,
    emit: &mut Emit,
) -> bool {
    let hit_fx = |l: &ArwingLaser, emit: &mut Emit| {
        if !l.three_d {
            emit.push(Fx::ImpactShock {
                pos: l.position,
                size: l.damage,
            });
        } else if !l.exploded {
            // `grSectorArwingWeaponLaser3DProcHit` / `...ProcAbsorb`.
            crate::sound::play_fgm(crate::sound::id::nSYAudioFGMExplodeS);
            emit.push(Fx::SparkleWhiteMultiExplode(l.position));
        }
    };
    if proc_hit {
        hit_fx(laser, emit);
        if !laser.three_d {
            return false;
        }
        // `grSectorArwingWeaponLaserExplodeInitVars`, which clears every
        // later callback.
        laser.explode();
        *records = [None; 4];
        return true;
    }
    if let Some(shield) = p.shield {
        // `grSectorArwingWeaponLaser2DProcHop` under 135 degrees, else its
        // `proc_shield` (`...ProcHit`). The 3D shot passes shields.
        if shield.angle < WEAPON_HOP_ANGLE_DEFAULT {
            let angle = (shield.angle - DEG_90).max(0.0);
            laser.velocity = hop_velocity(laser.velocity, angle, shield.dir_z);
            laser.reface();
        } else {
            hit_fx(laser, emit);
            return false;
        }
    }
    // `grSectorArwingWeaponLaser2DProcReflector`.
    if let Some(r) = p.reflect {
        reflect_shot(
            &mut laser.velocity,
            &mut laser.owner_port,
            &mut laser.damage,
            &r,
        );
        laser.reface();
        stale.stat = r.stat;
        stale.handicap = r.handicap;
        *team = r.team;
    }
    // `proc_absorb` destroys it.
    if p.absorb {
        hit_fx(laser, emit);
        return false;
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
    rules: crate::team::TeamRules,
    slot: usize,
    set_off: &'a mut bool,
    pending: &'a mut PendingProcs,
    landed: &'a mut Option<(u8, crate::stale::MotionAttackId, u16)>,
    /// `hit_normal_damage`: the `proc_hit` waits for every fighter's search.
    proc_hit: &'a mut bool,
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

/// `ftMainSearchHitWeapon` for one item-made weapon against one fighter
/// ([`crate::monster_weapon`]); its callbacks wait in `h`. Its attack
/// sweeps from its last position (the Hydro Pump's offset moves while it
/// stands), its hits credit its player, and the Smog passes shields
/// (`can_shield` clear).
fn monster_hit(
    m: &mut crate::monster_weapon::MonsterShot,
    mut h: MonsterHit<'_>,
    defender: &mut Fighter,
) {
    let owner = m.owner.unwrap_or(sector::GROUND_PORT);
    if m.owner == Some(defender.port) || h.rules.spares(defender.team, *h.team) {
        return;
    }
    let bit = 1u8 << (defender.port & 7);
    if h.ports & bit != 0 {
        return;
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
    // The callbacks wait for `wpProcessProcHitCollisions`
    // ([`WeaponPool::react_to_fighter_hits`]).
    match pre {
        PreHit::None => {}
        PreHit::SetOff => {
            *h.set_off = true;
            h.pending.set_off = true;
            record_shot_victim(&mut h, defender.port, rehit);
            return;
        }
        // `ftMainUpdateReflectorStatWeapon` records the reflector.
        PreHit::Reflected => {
            record_shot_victim(&mut h, defender.port, rehit);
            h.pending.reflect = Some(Reflector::of(defender));
            return;
        }
        PreHit::ReflectorBroke => {
            record_shot_victim(&mut h, defender.port, rehit);
            *h.proc_hit = true;
            return;
        }
        PreHit::Absorbed => {
            record_shot_victim(&mut h, defender.port, rehit);
            h.pending.absorb = true;
            return;
        }
    }
    let contact = crate::combat::weapon_hit_pair(
        defender,
        crate::combat::WeaponAttack {
            stat: m.stat,
            object: if matches!(
                m.kind,
                ShotKind::RayGun | ShotKind::StarRod | ShotKind::FireFlower
            ) {
                crate::spgame::bonus::DamageObject::Other
            } else {
                crate::spgame::bonus::DamageObject::PokemonWeapon
            },
            hitbox: staled,
            pos_curr: curr,
            pos_prev: prev,
            source: crate::combat::HitSource::Weapon {
                vel_x: m.velocity.x,
            },
            handicap: h.stale.handicap,
            can_shield: attr.can_shield,
            owner: m.player,
            is_hitlag_victim: m
                .is_hitlag_victim()
                .then_some(m.player.unwrap_or(sector::GROUND_PORT)),
            fgm_id: Some(Weapon::Monster(*m).fgm_id()),
        },
        if m.kind == ShotKind::RayGun {
            m.attack_tail
        } else {
            None
        },
    );
    if let crate::combat::WeaponContact::Shielded(shield) = contact {
        record_shot_victim(&mut h, defender.port, false);
        h.pending.shield.get_or_insert(shield);
        return;
    }
    if attack::HitOutcome::of(contact).registered() {
        if contact == crate::combat::WeaponContact::Hurt(true) {
            *h.landed = m
                .player
                .map(|p| (p, h.stale.attack_id, h.stale.motion_count));
        }
        record_shot_victim(&mut h, defender.port, rehit);
        *h.proc_hit = true;
    }
}

/// `wpProcessProcHitCollisions` for a fighter-made weapon. Returns whether
/// it lives on.
fn weapon_procs(
    weapon: &mut Weapon,
    hit: bool,
    p: PendingProcs,
    stale: &mut crate::stale::WeaponStale,
    team: &mut u8,
    emit: &mut Emit,
) -> bool {
    if hit {
        match weapon {
            // `wpSamusBombProcHit`.
            Weapon::Bomb(bomb) => {
                if !bomb.exploded {
                    crate::sound::play_fgm(crate::sound::id::nSYAudioFGMExplodeS);
                    emit.push(Fx::SparkleWhiteMultiExplode(bomb.position));
                    bomb.explode();
                }
            }
            _ => {
                // `proc_hit`, which makes its effects first.
                weapon.proc_effects(Proc::Hit, emit);
                match weapon {
                    // The Boomerang survives a hit and turns back.
                    Weapon::Boomerang(b) => b.on_hit(),
                    // `wpKirbyCutterProcHit` returns FALSE: the wave carries
                    // on.
                    Weapon::Cutter(_) => {}
                    // `wpYoshiEggThrowProcHit`: the egg explodes in place.
                    Weapon::Egg(e) => e.explode(),
                    _ => return false,
                }
            }
        }
    }
    // The Egg's and the Bomb's bursts clear every later callback.
    if matches!(weapon, Weapon::Egg(e) if e.exploded)
        || matches!(weapon, Weapon::Bomb(b) if b.exploded)
    {
        return true;
    }
    if let Some(shield) = p.shield {
        if !weapon.on_shield(shield, emit) {
            return false;
        }
    }
    if p.set_off {
        weapon.proc_effects(Proc::SetOff, emit);
        match weapon {
            // `wpLinkBoomerangProcSetOff`.
            Weapon::Boomerang(b) => b.set_off(),
            // `wpYoshiEggThrowProcHit`: it explodes in place.
            Weapon::Egg(e) => {
                e.explode();
                return true;
            }
            // `wpYoshiStarProcHit` without `hit_normal_damage`.
            Weapon::Star(_) => {}
            // `wpKirbyCutterProcSetOff` and every `ProcHit` that returns
            // TRUE.
            _ => return false,
        }
    }
    if let Some(r) = p.reflect {
        match weapon {
            Weapon::Fireball(f) => {
                reflect_shot(&mut f.velocity, &mut f.owner_port, &mut f.damage, &r);
                f.lifetime = f.attributes().lifetime;
            }
            Weapon::Blaster(b) => {
                reflect_shot(&mut b.velocity, &mut b.owner_port, &mut b.damage, &r);
                b.scale_x = 1.0;
            }
            Weapon::ChargeShot(c) => {
                reflect_shot(&mut c.velocity, &mut c.owner_port, &mut c.damage, &r);
            }
            Weapon::Boomerang(b) => b.reflect(&r),
            Weapon::Egg(e) => e.reflect(&r),
            Weapon::Star(s) => s.reflect(&r),
            Weapon::Cutter(c) => c.reflect(&r),
            Weapon::Jolt(j) => j.reflect(&r),
            // `wpBossBulletProcReflector`.
            Weapon::BossBullet(b) => {
                reflect_shot(&mut b.velocity, &mut b.owner_port, &mut b.damage, &r);
            }
            _ => unreachable!("not reflectable"),
        }
        // `wpProcessProcHitCollisions`: the reflector's team.
        *team = r.team;
        stale.stat = r.stat;
        stale.handicap = r.handicap;
    }
    if p.absorb {
        weapon.proc_effects(Proc::Absorb, emit);
        // `proc_absorb`: the Cutter survives.
        if !matches!(weapon, Weapon::Cutter(_)) {
            return false;
        }
    }
    true
}

/// `wpProcessProcHitCollisions` for an item-made weapon. Returns whether it
/// lives on.
fn monster_procs(
    m: &mut crate::monster_weapon::MonsterShot,
    hit: bool,
    p: PendingProcs,
    team: &mut u8,
    emit: &mut Emit,
) -> bool {
    if hit {
        m.proc_fx(ShotProc::Hit, emit);
        if !m.survives(ShotProc::Hit) {
            return false;
        }
    }
    if let Some(shield) = p.shield {
        if m.attributes().can_hop && shield.angle < WEAPON_HOP_ANGLE_DEFAULT {
            let angle = (shield.angle - DEG_90).max(0.0);
            m.hop(hop_velocity(m.velocity, angle, shield.dir_z));
        } else {
            m.proc_fx(ShotProc::Shield, emit);
            if !m.survives(ShotProc::Shield) {
                return false;
            }
        }
    }
    if p.set_off {
        m.proc_fx(ShotProc::SetOff, emit);
        if !m.survives(ShotProc::SetOff) {
            return false;
        }
    }
    if let Some(r) = p.reflect {
        m.reflect(r.port, r.team, r.lr, emit);
        m.stat = r.stat;
        *team = r.team;
    }
    if p.absorb {
        m.proc_fx(ShotProc::Absorb, emit);
        if !m.survives(ShotProc::Absorb) {
            return false;
        }
    }
    true
}

/// The kinds that keep a public mask of the fighters in their attack
/// record: `bit` joins it.
fn weapon_saw(weapon: &mut Weapon, bit: u8) {
    match weapon {
        Weapon::Boomerang(b) => b.hit_ports |= bit,
        Weapon::Cutter(c) => c.hit_ports |= bit,
        Weapon::Egg(e) => e.hit_ports |= bit,
        _ => {}
    }
}

/// `wpMainReflectorSetLR` and the reflect bonus for the straight shots:
/// turn X toward the reflector's facing, take its ownership and deal
/// `damage * 1.8 + 0.99` (US), at most 100.
fn reflect_shot(velocity: &mut Vec3, owner: &mut u8, damage: &mut i32, reflector: &Reflector) {
    *owner = reflector.port;
    if velocity.x * reflector.lr < 0.0 {
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
#[allow(clippy::too_many_arguments)]
fn stale_hit(
    hitbox: &Hitbox,
    position: Vec3,
    velocity: Vec3,
    stale: crate::stale::WeaponStale,
    defender: &mut Fighter,
    landed: &mut Option<(u8, crate::stale::MotionAttackId, u16)>,
    owner: u8,
    fgm_id: u16,
) -> attack::HitOutcome {
    let mut hitbox = *hitbox;
    hitbox.damage = stale.damage(hitbox.damage);
    let outcome = attack::HitOutcome::of(crate::combat::weapon_hit(
        defender,
        crate::combat::WeaponAttack {
            stat: stale.stat,
            object: crate::spgame::bonus::DamageObject::Other,
            hitbox,
            pos_curr: position,
            pos_prev: position - velocity,
            source: crate::combat::HitSource::Weapon { vel_x: velocity.x },
            handicap: stale.handicap,
            can_shield: true,
            owner: Some(owner),
            is_hitlag_victim: None,
            fgm_id: Some(fgm_id),
        },
    ));
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
    fgm_id: u16,
) -> crate::combat::WeaponContact {
    let mut hitbox = *hitbox;
    hitbox.damage = stale.damage(hitbox.damage);
    let contact = crate::combat::weapon_hit(
        defender,
        crate::combat::WeaponAttack {
            stat: stale.stat,
            object: crate::spgame::bonus::DamageObject::Other,
            hitbox,
            pos_curr: position,
            pos_prev: position - velocity,
            source: crate::combat::HitSource::Weapon { vel_x: velocity.x },
            handicap: stale.handicap,
            can_shield: true,
            owner: Some(owner),
            is_hitlag_victim,
            fgm_id: Some(fgm_id),
        },
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
            slots: ObjectPool::with_order(WEAPON_FREE_ORDER),
            item_spawns: [None; MAX_WEAPONS],
            item_spawn_len: 0,
            hit_records: [[None; 4]; MAX_WEAPONS],
            pending_item_hits: [false; MAX_WEAPONS],
            proc_hit: [false; MAX_WEAPONS],
            pending: [PendingProcs::NONE; MAX_WEAPONS],
            owners: [None; MAX_OWNERS],
            caught: [false; MAX_OWNERS],
            thunder_destroyed: [false; MAX_OWNERS],
            ness_thunder_destroyed: [false; MAX_OWNERS],
            next_group: 1,
            stale: [crate::stale::WeaponStale::FRESH; MAX_WEAPONS],
            teams: [crate::team::TEAM_DEFAULT; MAX_WEAPONS],
            lrs: [1.0; MAX_WEAPONS],
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
            counts: crate::manager_counts::ManagerCounts,
            clash_view: ClashView::default(),
        }
    }
}

impl WeaponPool {
    /// [`Self::default`] in place: the pool holds its effect queues, which a
    /// temporary would put on the (256 KB) PSP main-thread stack.
    pub fn reset(&mut self) {
        self.slots.reset();
        self.item_spawns = [None; MAX_WEAPONS];
        self.item_spawn_len = 0;
        self.hit_records = [[None; 4]; MAX_WEAPONS];
        self.pending_item_hits = [false; MAX_WEAPONS];
        self.proc_hit = [false; MAX_WEAPONS];
        self.pending = [PendingProcs::NONE; MAX_WEAPONS];
        self.owners = [None; MAX_OWNERS];
        self.caught = [false; MAX_OWNERS];
        self.thunder_destroyed = [false; MAX_OWNERS];
        self.ness_thunder_destroyed = [false; MAX_OWNERS];
        self.next_group = 1;
        self.stale = [crate::stale::WeaponStale::FRESH; MAX_WEAPONS];
        self.teams = [crate::team::TEAM_DEFAULT; MAX_WEAPONS];
        self.lrs = [1.0; MAX_WEAPONS];
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
        self.clash_view = ClashView::default();
    }

    fn tick_pk_thunder<I, F>(&mut self, surfaces: F, bounds: Option<BlastZone>)
    where
        F: Fn() -> I + Copy,
        I: IntoIterator<Item = MapSurface>,
    {
        let mut pending = [None; MAX_WEAPONS];
        for (i, queued) in pending.iter_mut().enumerate() {
            if let Some(Weapon::PKThunder(h)) = self.slots.slot_value_mut(i) {
                let owner = self.owners.get(h.owner_port as usize).copied().flatten();
                let mut emit = Emit::default();
                let alive = h.tick(surfaces, owner, &mut emit);
                self.fx.extend(self.seq[i], &emit);
                // `wpNessPKThunderHeadProcDead` destroys the trails with the
                // head; the owner learns of it from the missing head.
                if !alive || bounds.is_some_and(|b| out_of_bounds(b, h.position)) {
                    destroy(&mut self.slots, i);
                    continue;
                }
                // The head lives on: its trail effect's process (after the
                // weapons') reads it (`crate::effect::Effects::pk_trail_pose`).
                let (pos, rotate_z) = h.trail_effect_pose();
                let mut pose = Emit::default();
                pose.push(crate::wpeffect::WeaponEffect::PKTrailPose {
                    head: h.group,
                    pos,
                    rotate_z,
                });
                self.fx.extend(self.seq[i], &pose);
                if h.trail_spawn {
                    *queued = Some((
                        PKThunderTrail::new(*h, 0),
                        self.stale[i],
                        self.teams[i],
                        self.lrs[i],
                    ));
                }
            }
        }
        let mut heads = [None; MAX_WEAPONS];
        for (i, slot) in self.slots.values().iter().enumerate() {
            if let Some(Weapon::PKThunder(h)) = slot {
                heads[i] = Some(*h);
            }
        }
        for (i, queued) in pending.iter_mut().enumerate() {
            if let Some(Weapon::PKTrail(t)) = self.slots.slot_value_mut(i) {
                if let Some(head) = heads.iter().flatten().find(|h| h.group == t.group) {
                    let mut emit = Emit::default();
                    t.tick(*head, &mut emit);
                    self.fx.extend(self.seq[i], &emit);
                    if t.spawn_next {
                        let mut child = PKThunderTrail::new(*head, t.id + 1);
                        child.position = t.position;
                        *queued = Some((child, self.stale[i], self.teams[i], self.lrs[i]));
                    }
                } else {
                    destroy(&mut self.slots, i);
                }
            }
        }
        // `wpManagerMakeWeapon` queues the trail's process behind every
        // running weapon's at its priority, and `gcRunAll` walks on into it:
        // a trail runs its first update on the frame it is made (N64: the
        // head and each trail make the next one two frames apart).
        for (trail, stale, team, lr) in pending.into_iter().flatten() {
            let Some(j) = self.insert_at(Weapon::PKTrail(trail), stale, team, lr) else {
                continue;
            };
            let Some(Weapon::PKTrail(t)) = self.slots.slot_value_mut(j) else {
                continue;
            };
            if let Some(head) = heads.iter().flatten().find(|h| h.group == t.group) {
                let mut emit = Emit::default();
                // Lifetime 160: it makes no trail or effect yet.
                t.tick(*head, &mut emit);
                self.fx.extend(self.seq[j], &emit);
            }
        }
    }
    /// The `DObj` translations of the weapons that set `is_camera_follow`
    /// (`wpLinkBoomerangMakeWeapon`, `wpYoshiEggThrowMakeWeapon`,
    /// `wpNessPKThunderHeadMakeWeapon`), which `gmCameraUpdateInterests`
    /// frames. An egg still in Yoshi's hand is not in the pool: add
    /// [`crate::yoshi::held_egg_position`] for it. A reflected PK Thunder
    /// is `wpNessPKReflectHeadMakeWeapon`'s new weapon, which leaves the
    /// flag clear.
    pub fn camera_follow_positions(&self) -> impl Iterator<Item = Vec3> + '_ {
        self.slots
            .values()
            .iter()
            .flatten()
            .filter_map(|w| match w {
                Weapon::Boomerang(_) | Weapon::Egg(_) => Some(w.position()),
                Weapon::PKThunder(h) if !h.reflected => Some(h.position),
                _ => None,
            })
    }
    pub fn pk_fires(&self) -> impl Iterator<Item = PKFire> + '_ {
        self.slots.values().iter().flatten().filter_map(|w| {
            if let Weapon::PKFire(p) = w {
                Some(*p)
            } else {
                None
            }
        })
    }
    pub fn pk_thunders(&self) -> impl Iterator<Item = PKThunder> + '_ {
        self.slots.values().iter().flatten().filter_map(|w| {
            if let Weapon::PKThunder(p) = w {
                Some(*p)
            } else {
                None
            }
        })
    }
    pub fn pk_trails(&self) -> impl Iterator<Item = PKThunderTrail> + '_ {
        self.slots.values().iter().flatten().filter_map(|w| {
            if let Weapon::PKTrail(p) = w {
                Some(*p)
            } else {
                None
            }
        })
    }
    /// Ness's live weapons in link order: PK Fire sparks, PK Thunder heads
    /// and their trails. A first reflection's head
    /// (`wpNessPKReflectHeadMakeWeapon`) is a new weapon, last in the link.
    pub fn ness_shots(&self) -> impl Iterator<Item = NessShot> + '_ {
        let (order, n) = self.link_order();
        (0..n).filter_map(move |k| match self.slots.values()[order[k]] {
            Some(Weapon::PKFire(w)) => Some(NessShot::Fire(w)),
            Some(Weapon::PKThunder(w)) => Some(NessShot::Head(w)),
            Some(Weapon::PKTrail(w)) => Some(NessShot::Trail(w)),
            _ => None,
        })
    }
    /// Live item-made weapons, in link order (the trace's and the draw's),
    /// for the runtime's draw passes.
    pub fn monster_shots(&self) -> impl Iterator<Item = crate::monster_weapon::MonsterShot> + '_ {
        let (order, n) = self.link_order();
        (0..n).filter_map(move |k| match self.slots.values()[order[k]] {
            Some(Weapon::Monster(m)) => Some(m),
            _ => None,
        })
    }

    /// Free weapon structs.
    pub fn free_count(&self) -> usize {
        MAX_WEAPONS - self.slots.len()
    }

    /// `wpManagerMakeWeapon` for an item-made weapon
    /// (`WEAPON_FLAG_PARENT_ITEM`): the item's team and staling. Saffron's
    /// flame makes its particles here; the Poké Ball Pokémon's makers made
    /// theirs in the item's update.
    pub fn spawn_monster_shot(&mut self, shot: crate::monster_weapon::MonsterShot) -> bool {
        let seq = self.next_seq;
        let made = self.insert(
            Weapon::Monster(shot),
            crate::stale::WeaponStale {
                handicap: shot.handicap,
                ..crate::stale::WeaponStale::FRESH
            },
            shot.team,
            shot.lr,
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
        // `WEAPON_FLAG_PARENT_GROUND`: `wp->lr` is +1.
        self.insert(
            Weapon::Laser(laser),
            crate::stale::WeaponStale::FRESH,
            sector::GROUND_TEAM,
            1.0,
        )
    }

    /// Records Sector Z's Arwing's roll for its lasers' hops and
    /// reflections. Call it after the stage ticks.
    pub fn observe_arwing_roll(&mut self, roll: f32) {
        self.ground_roll = roll;
    }

    /// The live Arwing lasers, in slot order.
    pub fn arwing_lasers(&self) -> impl Iterator<Item = ArwingLaser> + '_ {
        self.slots
            .values()
            .iter()
            .flatten()
            .filter_map(|w| match w {
                Weapon::Laser(l) => Some(*l),
                _ => None,
            })
    }

    /// Consumes a fighter's deferred request. A full pool follows the source
    /// manager's allocation-failure shape: the already-consumed script event
    /// is not retried on a later frame.
    pub fn spawn(&mut self, spawn: WeaponSpawn) -> bool {
        self.spawn_projected(spawn, None, &core::iter::empty::<MapSurface>)
    }

    /// [`Self::spawn`] with `wpManagerMakeWeapon`'s
    /// `WEAPON_FLAG_COLLPROJECT` pass against the map when the request names
    /// its parent ([`Fighter::take_weapon_spawn_with_parent`]). Effects the
    /// makers place at their `pos` argument (Fox's glow, the Fire Flower's
    /// flame) and the Final Cutter's floor lookup keep the unsnapped point.
    pub fn spawn_projected<I, F>(
        &mut self,
        spawn: WeaponSpawn,
        parent: Option<SpawnParent>,
        surfaces: &F,
    ) -> bool
    where
        F: Fn() -> I,
        I: IntoIterator<Item = MapSurface>,
    {
        let snap = |w: &mut Weapon| {
            if let Some(parent) = &parent {
                w.snap(parent, surfaces);
            }
        };
        if spawn.kind == WeaponKind::YoshiStars {
            // Two `wpManagerMakeWeapon` calls; each fails on its own, and
            // each snaps from Yoshi to its own offset point.
            let lr = if spawn.facing < 0.0 { -1.0 } else { 1.0 };
            let mut star = Weapon::Star(YoshiStar::new(spawn, lr));
            snap(&mut star);
            let first = self.insert(star, spawn.stale, spawn.team, lr);
            let mut star = Weapon::Star(YoshiStar::new(spawn, -lr));
            snap(&mut star);
            let second = self.insert(star, spawn.stale, spawn.team, -lr);
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
                    stat: spawn.stale.stat,
                    owner: Some(spawn.owner_port),
                    player: Some(spawn.owner_port),
                    team: spawn.team,
                    lr: spawn.facing,
                    handle: u32::MAX,
                    handicap: spawn.stale.handicap,
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
                // `itFFlowerShootFlame`'s particles go to its `pos`, not the
                // snapped translate.
                let mut emit = Emit::default();
                m.make_fx(&mut emit);
                let lr = m.lr;
                let mut w = Weapon::Monster(m);
                snap(&mut w);
                let made = self.insert_at(w, spawn.stale, spawn.team, lr);
                if let Some(i) = made {
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
                let mut w = Weapon::Blaster(FoxBlaster::new(spawn));
                snap(&mut w);
                let made = self.insert_at(
                    w,
                    spawn.stale,
                    spawn.team,
                    if spawn.facing < 0.0 { -1.0 } else { 1.0 },
                );
                // `wpFoxBlasterMakeWeapon`: the glow at the unsnapped `pos`.
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
            WeaponKind::BossBullet { hard } => Weapon::BossBullet(BossBullet::new(spawn, hard)),
            WeaponKind::LinkBoomerang {
                is_smash,
                stick_x,
                stick_y,
            } => Weapon::Boomerang(LinkBoomerang::new(spawn, is_smash, stick_x, stick_y)),
            WeaponKind::YoshiEgg {
                throw_force,
                stick_x,
                scale,
            } => Weapon::Egg(YoshiEgg::new(spawn, throw_force, stick_x, scale)),
            WeaponKind::YoshiStars => unreachable!("handled above"),
            WeaponKind::KirbyCutter { floor_line } => {
                let mut w = Weapon::Cutter(KirbyCutter::new(spawn));
                snap(&mut w);
                if let Weapon::Cutter(c) = &mut w {
                    c.make_floor(floor_line, spawn.position, surfaces);
                }
                w
            }
        };
        let mut weapon = weapon;
        if !matches!(spawn.kind, WeaponKind::KirbyCutter { .. }) {
            snap(&mut weapon);
        }
        let lr = if spawn.facing < 0.0 { -1.0 } else { 1.0 };
        let made = self.insert_at(weapon, spawn.stale, spawn.team, lr);
        if let Some(i) = made {
            match self.slots.slot_value_mut(i) {
                // `wpLinkBoomerangMakeWeapon`'s `wpMainPlayFGM`.
                Some(Weapon::Boomerang(b)) => {
                    b.sfx = play_weapon_fgm(b.sfx, crate::sound::id::nSYAudioFGMLinkSpecialNShoot);
                }
                // `wpSamusChargeShotMakeWeapon` with `is_release`, or the
                // charging shot's release: `wpSamusChargeShotLaunch`.
                Some(Weapon::ChargeShot(c)) => {
                    crate::sound::play_fgm(SAMUS_CHARGE_SHOT_FGMS[usize::from(c.charge)].0);
                }
                _ => {}
            }
        }
        made.is_some()
    }

    fn insert(
        &mut self,
        weapon: Weapon,
        stale: crate::stale::WeaponStale,
        team: u8,
        lr: f32,
    ) -> bool {
        self.insert_at(weapon, stale, team, lr).is_some()
    }

    /// `wpManagerMakeWeapon`: the free list's head, placed last in the link.
    fn insert_at(
        &mut self,
        mut weapon: Weapon,
        mut stale: crate::stale::WeaponStale,
        team: u8,
        lr: f32,
    ) -> Option<usize> {
        if self.slots.is_full() {
            return None;
        }
        // `wpManagerMakeWeapon`'s ground-parent case (wpmanager.c:185,
        // :188): a fresh motion count, then a stat count. A fighter or item
        // parent's counts ([`crate::stale::WeaponStale::of`]) are never 0.
        if stale.motion_count == 0 {
            stale.motion_count = self.counts.motion();
        }
        if stale.stat.count == 0 {
            stale.stat.count = self.counts.stat();
        }
        if let Weapon::Monster(m) = &mut weapon {
            if m.stat.count == 0 {
                m.stat.count = stale.stat.count;
            }
        }
        let i = self.slots.insert(weapon).ok()?.index();
        self.stale[i] = stale;
        self.teams[i] = team;
        self.lrs[i] = lr;
        self.landed[i] = None;
        self.hit_records[i] = [None; 4];
        self.pending_item_hits[i] = false;
        self.proc_hit[i] = false;
        self.pending[i] = PendingProcs::NONE;
        self.clash_damage[i] = 0;
        self.set_off[i] = false;
        self.clash_view.refilled[i] = true;
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
        if let Some(slot) = self.item_spawns.get_mut(self.item_spawn_len) {
            *slot = Some(spawn);
            self.item_spawn_len += 1;
        }
    }

    /// The PK Fire flames made since the last call, in order.
    pub fn take_item_spawns(&mut self) -> impl Iterator<Item = crate::item::PKFireSpawn> + '_ {
        let len = core::mem::take(&mut self.item_spawn_len);
        self.item_spawns[..len].iter_mut().filter_map(Option::take)
    }

    /// `itProcessSearchHitWeapon`'s view of slot `i`: the item searches
    /// (priority 1) precede every weapon's `wpProcessProcHitCollisions`
    /// (priority 0), so a weapon reads as [`Self::begin_clashes`] took it
    /// even after a fighter's hit has already run its `proc_hit` (the
    /// Boomerang's return damage and velocity) or destroyed it. A slot
    /// refilled since, or a frame without the view, reads as it stands.
    fn searched_item_attack(&self, i: usize) -> Option<ItemAttack> {
        let view = &self.clash_view;
        if view.taken && !view.refilled[i] {
            view.item_attacks[i]
        } else {
            self.item_attack(i)
        }
    }

    /// A slot's attack for the item and clash searches: owner, staled
    /// hitbox, position, velocity and previous position.
    fn item_attack(&self, i: usize) -> Option<ItemAttack> {
        let stale = self.stale[i];
        let (owner, mut hitbox, pos, vel) = match self.slots.values()[i]? {
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
            Weapon::BossBullet(b) => (b.owner_port, b.hitbox(), b.position, b.velocity),
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
            // `wpNessPKThunderTrailProcUpdate` sets the trail's position,
            // so its attack box transfers (no sweep); `vel_air` (zero
            // before a reflect) picks the item's knockback side.
            Weapon::PKTrail(t) => {
                let pos = t.hit_position();
                let hitbox = Hitbox {
                    damage: stale.damage(ness::TRAIL_HIT.damage),
                    ..ness::TRAIL_HIT
                };
                return Some((t.owner_port, hitbox, pos, t.velocity, pos));
            }
            // `wpPikachuThunderHeadMakeWeapon` turns the head's attack off.
            Weapon::Thunder(_) => return None,
            // A trail segment stands still: its box transfers. This is its
            // first attack offset (the authored +240 at DObj scale 0.5);
            // [`trail_low_attack`] is the second. `vel_air` stays zero.
            Weapon::Trail(t) => {
                let pos = t.position + Vec3::new(0.0, 120.0, 0.0);
                let hitbox = Hitbox {
                    damage: stale.damage(pikachu::TRAIL_HIT.damage),
                    ..pikachu::TRAIL_HIT
                };
                return Some((t.owner_port, hitbox, pos, Vec3::ZERO, pos));
            }
        };
        hitbox.damage = stale.damage(hitbox.damage);
        Some((owner, hitbox, pos, vel, pos - vel))
    }

    /// `itProcessSearchHitWeapon` for one item (`id` is its record id):
    /// every weapon that has not recorded it tests its damage box. A contact
    /// queues the weapon's staled damage on the item and defers the weapon's
    /// `proc_hit` until all item searches finish. Attack clashes precede
    /// the hurtbox search and queue `proc_setoff` for `finish_clashes`.
    pub fn hit_item(
        &mut self,
        item: &mut crate::item::Item,
        id: u8,
        emit: &mut crate::wpeffect::Emit,
    ) {
        use crate::item::INTERACT_WEAPON;
        if item.damage_coll.interact_mask & INTERACT_WEAPON == 0 {
            return;
        }
        let (order, count) = self.link_order();
        for &i in &order[..count] {
            let Some((owner, hitbox, pos, vel, prev)) = self.searched_item_attack(i) else {
                continue;
            };
            // `ip->owner_gobj == wp->owner_gobj`: a ground weapon's NULL
            // owner is an ownerless item's too (Saffron's flame and its
            // Charmander).
            let item_owner = item.owner.unwrap_or(sector::GROUND_PORT);
            if item_owner == owner && !item.is_damage_all {
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
                && item_owner != owner
                && !self.team_rules.spares(item.team, self.teams[i])
                && item.attack.record(weapon_victim_id(i)).is_clear()
            {
                if let Some(w) = self.clash_attack(i) {
                    let mut skip = false;
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
                            self.record_item_victim(i, id);
                            self.clash_damage[i] = self.clash_damage[i].max(w.damage);
                            self.clash_fx.push(
                                0,
                                Fx::SetOff {
                                    pos: impact,
                                    size: w.damage,
                                },
                            );
                        }
                        // `wp->hit_attack_damage` (a fighter's set-off, an
                        // earlier item's clash): the next weapon; the item's
                        // own: its hurtbox check.
                        if self.clash_damage[i] != 0 || self.set_off[i] {
                            skip = true;
                            break;
                        }
                        if item.hit_attack_damage != 0 {
                            break;
                        }
                    }
                    if skip {
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
            let tail_hit = matches!(self.slots.values()[i], Some(Weapon::Monster(m)) if m.kind == ShotKind::RayGun && m.attack_tail.is_some_and(|(tail, old)| crate::item::touches_damage_coll(item, tail, old, hitbox.radius, state)));
            // `itProcessSearchHitWeapon` walks every attack offset; a
            // Thunder trail has two.
            let trail_low_hit = matches!(self.slots.values()[i], Some(Weapon::Trail(t)) if {
                let low = trail_low_attack(t);
                crate::item::touches_damage_coll(item, low, low, hitbox.radius, state)
            });
            if !crate::item::touches_damage_coll(item, pos, prev, hitbox.radius, state)
                && !tail_hit
                && !trail_low_hit
            {
                continue;
            }
            // `itProcessUpdateDamageStatWeapon`.
            self.record_item_victim(i, id);
            // `is_rehit`: only a `nITTypeDamage` item is rehit.
            if item.ty == crate::item::ItemType::Damage
                && matches!(self.slots.values()[i], Some(Weapon::Monster(m)) if m.attributes().can_rehit_item)
            {
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
            // `wp->is_hitlag_victim` (the Boomerang, two Pokemon):
            // `gmCollisionGetWeaponAttackItemDamagePosition` and the spark.
            let is_hitlag_victim = match self.slots.values()[i] {
                Some(Weapon::Boomerang(_)) => true,
                Some(Weapon::Monster(m)) => m.is_hitlag_victim(),
                _ => false,
            };
            if is_hitlag_victim && item.damage_coll.hitstatus == crate::combat::HitStatus::Normal {
                let impact = crate::combat::impact_point(
                    crate::combat::attack_point(pos, prev, state),
                    item.damage_coll_pos(),
                );
                emit.push(crate::item::damage_spark(
                    hitbox.element,
                    impact,
                    owner,
                    hitbox.damage,
                    None,
                ));
            }
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
                    handicap: self.stale[i].handicap,
                },
                crate::item::Knock {
                    weight: hitbox.kb_weight,
                    scale: hitbox.kb_scale,
                    base: hitbox.kb_base,
                },
            );
            // `itProcessUpdateDamageStatWeapon`'s last line.
            if let Some(w) = self.slots.slot_value(i) {
                crate::sound::play_fgm(w.fgm_id());
            }
            self.pending_item_hits[i] = true;
        }
    }

    /// `wpProcessUpdateHitInteractStats` against item `id`: a weapon with a
    /// `group_id` records it for every weapon of its group. PK Thunder's
    /// trails share one group; a reflected head shares its trails' group
    /// (`wpNessPKReflectHeadMakeWeapon`), the first head's is 0.
    fn record_item_victim(&mut self, i: usize, id: u8) {
        let group = |w: &Option<Weapon>| match w {
            Some(Weapon::PKTrail(t)) => Some(t.group),
            Some(Weapon::PKThunder(h)) if h.reflected => Some(h.group),
            // Pikachu's head and trails always share a `group_id`
            // (wppikachuthunder.c:171, :222).
            Some(Weapon::Thunder(h)) => Some(h.group),
            Some(Weapon::Trail(t)) => Some(t.group),
            _ => None,
        };
        let Some(g) = group(&self.slots.values()[i]) else {
            record_weapon_victim(&mut self.hit_records[i], id);
            return;
        };
        for j in 0..MAX_WEAPONS {
            if group(&self.slots.values()[j]) == Some(g) {
                record_weapon_victim(&mut self.hit_records[j], id);
            }
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
        let Some(weapon) = self.slots.slot_value_mut(i) else {
            return;
        };
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
                destroy(&mut self.slots, i);
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
            // `wpNessPKThunderTrailProcHit` and
            // `wpPikachuThunderTrailProcHit` return FALSE.
            Weapon::PKTrail(_) | Weapon::Trail(_) => {}
            _ => destroy(&mut self.slots, i),
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
        for i in 0..MAX_WEAPONS {
            if matches!(self.slots.slot_value(i), Some(Weapon::Boomerang(b)) if b.parent_port == Some(port))
            {
                destroy(&mut self.slots, i);
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
            f.ness.thunder_position = self.slots.values().iter().flatten().find_map(|w| match w {
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
            f.pikachu.thunder_position =
                self.slots.values().iter().flatten().find_map(|w| match w {
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
            .values()
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
        for i in 0..MAX_WEAPONS {
            if let Some(weapon) = self.slots.slot_value_mut(i) {
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
                                trails[i] = Some((
                                    ThunderTrail::new(*head),
                                    self.stale[i],
                                    self.teams[i],
                                    self.lrs[i],
                                    self.hit_records[i],
                                ));
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
                    Weapon::BossBullet(bullet) => bullet.tick(surfaces, fx),
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
                    destroy(&mut self.slots, i);
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
        // `wpPikachuThunderTrailMakeWeapon` copies the head's attack
        // records: an item the group hit stays recorded.
        for (trail, stale, team, lr, records) in trails.into_iter().flatten() {
            if let Some(j) = self.insert_at(Weapon::Trail(trail), stale, team, lr) {
                self.hit_records[j] = records;
            }
        }
    }

    /// Resolves every eligible weapon against one fighter, then runs the
    /// `proc_hit`s it caused ([`Self::search_hits`] and
    /// [`Self::react_to_fighter_hits`]). Fireball and Blaster both delete
    /// on registered contact; invincibility leaves the shot live, exactly
    /// like a non-registered source hitbox.
    pub fn apply_hits(&mut self, defender: &mut Fighter) {
        self.search_hits(defender);
        self.react_to_fighter_hits();
        let (port, lr) = (defender.port, defender.facing.sign());
        self.react_to_reflectors(&|p| (p == port).then_some(lr));
    }

    /// `ftMainSearchHitWeapon` for one fighter. A registered hit (and a
    /// broken reflector) only records the fighter and sets the weapon's
    /// `hit_normal_damage`: its `proc_hit` runs in
    /// `wpProcessProcHitCollisions` (weapon priority 0), after every
    /// fighter's search (priority 1), so a second fighter searched the same
    /// frame still meets the live weapon. A set-off (`hit_attack_damage`),
    /// shield (`hit_shield_damage`), reflector (`reflect_gobj`) and absorber
    /// (`absorb_gobj`) wait the same way ([`PendingProcs`]).
    pub fn search_hits(&mut self, defender: &mut Fighter) {
        // `ftMainProcSearchHitAll` skips a ghost.
        if defender.dead.is_ghost {
            return;
        }
        let rules = self.team_rules;
        let bit = 1u8 << (defender.port & 7);
        let mut thunder_groups = [None; MAX_WEAPONS];
        // `gGCCommonLinks[nGCCommonLinkIDWeapon]`: the weapons in link
        // order, not slot order (a freed slot refills with a newer weapon).
        let (order, n) = self.link_order();
        for &i in &order[..n] {
            // Ness's trails follow their head in the link.
            if matches!(self.slots.values()[i], Some(Weapon::PKTrail(_))) {
                self.search_pk_trail(i, defender);
                continue;
            }
            let Some(weapon) = self.slots.slot_value_mut(i) else {
                continue;
            };
            let fgm_id = weapon.fgm_id();
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
            if let Weapon::Thunder(_) = weapon {
                continue;
            }
            if let Weapon::Trail(t) = weapon {
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
                        crate::item_sounds::weapon::PIKACHU_THUNDER_TRAIL,
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
                monster_hit(
                    m,
                    MonsterHit {
                        records,
                        rehit: &mut self.rehit[i],
                        ports,
                        stale: self.stale[i],
                        team: &mut self.teams[i],
                        rules,
                        slot: i,
                        set_off: &mut self.set_off[i],
                        pending: &mut self.pending[i],
                        landed: &mut self.landed[i],
                        proc_hit: &mut self.proc_hit[i],
                    },
                    defender,
                );
                continue;
            }
            if let Weapon::Laser(laser) = weapon {
                laser_hit(
                    laser,
                    LaserHit {
                        records,
                        ports,
                        stale: self.stale[i],
                        team: self.teams[i],
                        landed: &mut self.landed[i],
                        rules,
                        slot: i,
                        proc_hit: &mut self.proc_hit[i],
                        pending: &mut self.pending[i],
                    },
                    defender,
                );
                continue;
            }
            let (owner, mut hitbox, position, velocity) = match weapon {
                Weapon::Jolt(j) => {
                    let (hit, pos) = j.hit();
                    (j.owner_port, hit, pos, j.velocity)
                }
                Weapon::PKFire(s) => (s.owner_port, ness::SPARK_HIT, s.position, s.velocity),
                Weapon::PKThunder(h) => (h.owner_port, ness::HEAD_HIT, h.position, h.velocity),
                Weapon::PKTrail(_) => unreachable!("searched by search_pk_trail"),
                Weapon::Thunder(_) | Weapon::Trail(_) => unreachable!("handled above"),
                Weapon::Fireball(f) => {
                    (f.owner_port, MARIO_FIREBALL_HITBOX, f.position, f.velocity)
                }
                Weapon::Blaster(b) => (b.owner_port, FOX_BLASTER_HITBOX, b.position, b.velocity),
                Weapon::ChargeShot(c) => (c.owner_port, c.hitbox(), c.position, c.velocity),
                Weapon::Bomb(b) => (b.owner_port, b.hitbox(), b.position, b.velocity),
                Weapon::BossBullet(b) => (b.owner_port, b.hitbox(), b.position, b.velocity),
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
            // The weapon's attack record: once it has met this fighter in any
            // way, it passes through. A hopped shot keeps its shield record.
            if ports & bit != 0 {
                continue;
            }
            hitbox.damage = match weapon {
                Weapon::Jolt(j) => j.damage,
                Weapon::PKFire(s) => s.damage,
                Weapon::PKThunder(h) => h.damage,
                Weapon::PKTrail(_) => unreachable!("searched after the heads"),
                Weapon::Thunder(_) | Weapon::Trail(_) => unreachable!("handled above"),
                Weapon::Fireball(f) => f.damage,
                Weapon::Blaster(b) => b.damage,
                Weapon::ChargeShot(c) => c.damage,
                Weapon::Bomb(_) => hitbox.damage,
                Weapon::BossBullet(b) => b.damage,
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
                        fgm_id,
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
                    fgm_id,
                );
                if let crate::combat::WeaponContact::Shielded(shield) = contact {
                    record_weapon_victim(records, defender.port);
                    self.pending[i].shield.get_or_insert(shield);
                    continue;
                }
                if attack::HitOutcome::of(contact).registered() {
                    record_weapon_victim(records, defender.port);
                    bomb.hit_ports |= bit;
                    self.proc_hit[i] = true;
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
            // `ftMainUpdateAttackStatWeapon`, `...ReflectorStatWeapon` and
            // `...AbsorbStatWeapon` record the fighter and leave the
            // callbacks to `wpProcessProcHitCollisions`: a fighter searched
            // later this frame still meets the weapon as it was.
            match pre {
                PreHit::None => {}
                // `hit_attack_damage`: `proc_setoff`.
                PreHit::SetOff => {
                    self.set_off[i] = true;
                    self.pending[i].set_off = true;
                    weapon_saw(weapon, bit);
                    record_weapon_victim(records, defender.port);
                    continue;
                }
                // `ftMainUpdateReflectorStatWeapon` records the reflector
                // (`nGMHitTypeReflect`), kept through the reflection: the
                // shot passes it from then on. `reflect_gobj`: the last
                // reflector wins.
                PreHit::Reflected => {
                    weapon_saw(weapon, bit);
                    record_weapon_victim(records, defender.port);
                    self.pending[i].reflect = Some(Reflector::of(defender));
                    continue;
                }
                // `hit_normal_damage`: the weapon's `proc_hit`.
                PreHit::ReflectorBroke => {
                    weapon_saw(weapon, bit);
                    record_weapon_victim(records, defender.port);
                    self.proc_hit[i] = true;
                    continue;
                }
                // `absorb_gobj`: `proc_absorb`.
                PreHit::Absorbed => {
                    weapon_saw(weapon, bit);
                    record_weapon_victim(records, defender.port);
                    self.pending[i].absorb = true;
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
                fgm_id,
            );
            // `ftMainUpdateShieldStatWeapon` records the fighter; then
            // `wpProcessProcHitCollisions` hops the weapon or runs its
            // `proc_shield`.
            if let crate::combat::WeaponContact::Shielded(shield) = contact {
                record_weapon_victim(records, defender.port);
                self.pending[i].shield.get_or_insert(shield);
                continue;
            }
            if attack::HitOutcome::of(contact).registered() {
                // `wpProcessUpdateHitInteractStats`; the `proc_hit` waits.
                weapon_saw(weapon, bit);
                record_weapon_victim(records, defender.port);
                self.proc_hit[i] = true;
            }
        }
        for (_, w) in self.slots.iter_mut() {
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

    /// `ftMainSearchHitWeapon` for Ness's PK Thunder trail in slot `i`, in its
    /// place in the link. A trail records the fighter for its whole
    /// group (`wpProcessUpdateHitInteractStats`): the trails of one head
    /// and, once reflected, the head too (`wpNessPKReflectHeadMakeWeapon`
    /// gives it a `group_id` its trails copy; the first head's is 0). Its
    /// `proc_hit` and `proc_shield` (`wpNessPKThunderTrailProcHit`) wait
    /// for [`Self::react_to_fighter_hits`].
    fn search_pk_trail(&mut self, i: usize, defender: &mut Fighter) {
        let Some(Weapon::PKTrail(t)) = self.slots.values()[i] else {
            return;
        };
        let bit = 1u8 << (defender.port & 7);
        if t.owner_port == defender.port
            || self.team_rules.spares(defender.team, self.teams[i])
            || t.hit_ports & bit != 0
        {
            return;
        }
        // A reflected head that met this fighter recorded it for its
        // trails.
        let head_saw = self
            .slots
            .values()
            .iter()
            .zip(&self.hit_records)
            .any(|(w, r)| {
                matches!(w, Some(Weapon::PKThunder(h)) if h.reflected && h.group == t.group)
                    && recorded_fighter_ports(r) & bit != 0
            });
        if head_saw {
            return;
        }
        let contact = stale_contact(
            &ness::TRAIL_HIT,
            t.hit_position(),
            Vec3::ZERO,
            self.stale[i],
            defender,
            &mut self.landed[i],
            t.owner_port,
            None,
            crate::item_sounds::weapon::NESS_PK_THUNDER_TRAIL,
        );
        match contact {
            crate::combat::WeaponContact::Missed => return,
            // `ftMainUpdateShieldStatWeapon`: `proc_shield`.
            crate::combat::WeaponContact::Shielded(shield) => {
                self.pending[i].shield.get_or_insert(shield);
            }
            // `hit_normal_damage`: `proc_hit`.
            crate::combat::WeaponContact::Hurt(_) => self.proc_hit[i] = true,
        }
        for (j, slot) in self.slots.values_mut().enumerate() {
            match slot {
                Some(Weapon::PKTrail(o)) if o.group == t.group => o.hit_ports |= bit,
                Some(Weapon::PKThunder(h)) if h.reflected && h.group == t.group => {
                    record_weapon_victim(&mut self.hit_records[j], defender.port);
                }
                _ => {}
            }
        }
    }

    /// `wpProcessProcHitCollisions`, in link order, for every weapon a
    /// fighter's [`Self::search_hits`] met this frame: `proc_hit`, then the
    /// shield's hop or `proc_shield`, `proc_setoff`, the reflector and
    /// `proc_absorb`, each only while the weapon lives.
    ///
    /// The reflector and `proc_absorb` wait for
    /// [`Self::react_to_reflectors`]: `wpMainReflectorSetLR` reads the
    /// reflector's `lr` after its `ftMainProcParams` (fighters link 3,
    /// weapons link 5, both priority 0), where Fox's reflector turns to the
    /// shot (`ftFoxSpecialLwHitSetStatus`).
    pub fn react_to_fighter_hits(&mut self) {
        let (order, n) = self.link_order();
        let mut late = [PendingProcs::NONE; MAX_WEAPONS];
        for &i in &order[..n] {
            let hit = core::mem::take(&mut self.proc_hit[i]);
            let mut pending = core::mem::replace(&mut self.pending[i], PendingProcs::NONE);
            late[i] = PendingProcs {
                reflect: pending.reflect.take(),
                absorb: core::mem::take(&mut pending.absorb),
                ..PendingProcs::NONE
            };
            self.run_procs(i, hit, pending);
            if self.slots.values()[i].is_none() {
                late[i] = PendingProcs::NONE;
            }
        }
        self.proc_hit = [false; MAX_WEAPONS];
        self.pending = late;
        // `wpNessPKThunderHeadSetDestroyTrails` only marks the trails: they
        // run this pass's callbacks, stay listed and drawn, and their next
        // `proc_update` ends them ([`Self::tick_pk_thunder`] drops a trail
        // whose head group is gone).
    }

    /// The rest of `wpProcessProcHitCollisions` for the fighters' searches:
    /// the reflector (`wpMainReflectorSetLR` with the reflector's `lr` as
    /// `lr_of` gives it now, after `ftMainProcParams`) and `proc_absorb`.
    pub fn react_to_reflectors(&mut self, lr_of: &dyn Fn(u8) -> Option<f32>) {
        let (order, n) = self.link_order();
        for &i in &order[..n] {
            let mut pending = core::mem::replace(&mut self.pending[i], PendingProcs::NONE);
            if let Some(r) = pending.reflect.as_mut() {
                if let Some(lr) = lr_of(r.port) {
                    r.lr = lr;
                }
            }
            self.run_procs(i, false, pending);
        }
        self.pending = [PendingProcs::NONE; MAX_WEAPONS];
    }

    /// One weapon's `wpProcessProcHitCollisions` callbacks for `hit` and
    /// `pending`, each only while it lives.
    fn run_procs(&mut self, i: usize, hit: bool, pending: PendingProcs) {
        {
            if !hit && pending == PendingProcs::NONE {
                return;
            }
            if matches!(
                self.slots.values()[i],
                Some(Weapon::PKFire(_) | Weapon::PKThunder(_) | Weapon::PKTrail(_))
            ) {
                self.pk_procs(i, hit, pending);
                return;
            }
            let Some(weapon) = self.slots.slot_value_mut(i) else {
                return;
            };
            let mut emit = Emit::default();
            let alive = match weapon {
                Weapon::Laser(laser) => laser_procs(
                    laser,
                    hit,
                    pending,
                    &mut self.hit_records[i],
                    &mut self.stale[i],
                    &mut self.teams[i],
                    &mut emit,
                ),
                Weapon::Monster(m) => monster_procs(m, hit, pending, &mut self.teams[i], &mut emit),
                _ => weapon_procs(
                    weapon,
                    hit,
                    pending,
                    &mut self.stale[i],
                    &mut self.teams[i],
                    &mut emit,
                ),
            };
            self.fx.extend(self.seq[i], &emit);
            if !alive {
                destroy(&mut self.slots, i);
            }
        }
    }

    /// `wpProcessProcHitCollisions` for Ness's weapons: PK Fire's spark
    /// (`wpNessPKFireProcHit` for `proc_hit` and `proc_setoff`, which makes
    /// the pillar; `...ProcAbsorb` for `proc_shield` and `proc_absorb`;
    /// `...ProcHop`; `...ProcReflector`), the PK Thunder head
    /// (`wpNessPKThunderHeadProcHit` for every callback but
    /// `...ProcReflector`) and its trails (`wpNessPKThunderTrailProcHit`,
    /// which returns FALSE). The reflector's owner swap (`owner_gobj`,
    /// `team`, US `stat_flags`) runs here, after every fighter's search.
    fn pk_procs(&mut self, i: usize, hit: bool, p: PendingProcs) {
        let free_slots = MAX_WEAPONS - self.slots.len();
        let Some(weapon) = self.slots.slot_value_mut(i) else {
            return;
        };
        let mut emit = Emit::default();
        let mut pillar = false;
        let trail = matches!(weapon, Weapon::PKTrail(_));
        let thunder_owner = match weapon {
            Weapon::PKThunder(h) if !h.reflected => Some(h.owner_port),
            _ => None,
        };
        let alive = 'procs: {
            if hit {
                weapon.proc_effects(Proc::Hit, &mut emit);
                if !trail {
                    pillar = matches!(weapon, Weapon::PKFire(_));
                    break 'procs false;
                }
            }
            if let Some(shield) = p.shield {
                if !weapon.on_shield(shield, &mut emit) {
                    break 'procs false;
                }
            }
            if p.set_off {
                weapon.proc_effects(Proc::SetOff, &mut emit);
                if !trail {
                    pillar = matches!(weapon, Weapon::PKFire(_));
                    break 'procs false;
                }
            }
            if let Some(r) = p.reflect {
                self.teams[i] = r.team;
                self.stale[i].stat = r.stat;
                self.stale[i].handicap = r.handicap;
                match weapon {
                    Weapon::PKFire(spark) => spark.reflect(&r),
                    Weapon::PKThunder(h) => {
                        let first = !h.reflected;
                        if first {
                            // `wpNessPKReflectHeadMakeWeapon` fails with the
                            // pool full; the old head goes regardless.
                            if free_slots == 0 {
                                break 'procs false;
                            }
                            // The new weapon is last in the link, with fresh
                            // attack records and its parent's `wp->lr`
                            // (`wpManagerMakeWeapon`'s
                            // `WEAPON_FLAG_PARENT_WEAPON`).
                            self.next_seq = self.next_seq.wrapping_add(1);
                            self.seq[i] = self.next_seq;
                            self.hit_records[i] = [None; 4];
                        }
                        // A later reflection keeps the `group_id` on the
                        // N64, whose trails then outlive the head's
                        // renewed life (the double-reflect crash). Here a
                        // new group ends them as a first reflection does.
                        let group = self.next_group;
                        self.next_group = self.next_group.wrapping_add(1);
                        h.reflect(&r, group);
                        // `wpNessPKThunderHeadProcReflector` returns TRUE
                        // for the old head: the new one has no callbacks
                        // pending. `wpNessPKReflectHeadProcReflector`
                        // returns FALSE.
                        if first {
                            break 'procs true;
                        }
                    }
                    _ => unreachable!("trails cannot be reflected"),
                }
            }
            if p.absorb {
                weapon.proc_effects(Proc::Absorb, &mut emit);
                if !trail {
                    break 'procs false;
                }
            }
            true
        };
        self.fx.extend(self.seq[i], &emit);
        // `wpNessPKThunderHeadSetDestroyTrails(head, TRUE)` from the head's
        // `proc_hit`, `proc_shield`, `proc_setoff`, `proc_reflector` (after
        // giving the head back to Ness) and `proc_absorb` sets Ness's
        // `is_thunder_destroy` in this pass: his next update counts
        // `pkthunder_end_delay` down.
        if let Some(port) = thunder_owner {
            if !alive || matches!(self.slots.values()[i], Some(Weapon::PKThunder(h)) if h.reflected)
            {
                self.mark_ness_thunder_destroyed(port);
            }
        }
        if pillar {
            if let Some(Weapon::PKFire(spark)) = self.slots.values()[i] {
                self.queue_item_spawn(spark.item_spawn(self.stale[i], self.teams[i]));
            }
        }
        if !alive {
            destroy(&mut self.slots, i);
        }
    }

    /// Slot `i`'s `wp->lr`. Fireballs, Blaster shots, both Thunders and
    /// their trails, and the Arwing's lasers keep the make's; the Charge
    /// Shot and PK Fire rederive theirs from the velocity on every make,
    /// hop and reflect (`wpMainVelSetLR`), which leaves its sign.
    fn cpu_lr(&self, i: usize) -> f32 {
        let sign = |x: f32| if x >= 0.0 { 1.0 } else { -1.0 };
        match self.slots.values()[i] {
            Some(Weapon::Bomb(b)) => b.lr,
            Some(Weapon::Boomerang(b)) => b.lr,
            Some(Weapon::Egg(e)) => e.lr,
            Some(Weapon::Star(s)) => s.lr,
            Some(Weapon::Cutter(c)) => c.lr,
            Some(Weapon::Monster(m)) => m.lr,
            // On a wall the Jolt's `lr` is 2 or 3, and the source reads it
            // as is.
            Some(Weapon::Jolt(j)) => f32::from(j.direction),
            Some(Weapon::ChargeShot(c)) => sign(c.velocity.x),
            Some(Weapon::PKFire(p)) => sign(p.velocity.x),
            _ => self.lrs[i],
        }
    }

    /// Every weapon's live hits as the CPU reads them
    /// (`func_ovl3_80135B78`), in link order: one entry per attack
    /// position. Pikachu's Thunder head has no fighter attack.
    pub fn cpu_threats(&self) -> impl Iterator<Item = crate::computer::behave::WeaponThreat> + '_ {
        let (order, n) = self.link_order();
        (0..n).flat_map(move |k| {
            let i = order[k];
            let threat = |owner: u8, pos: Vec3, vel_x: f32, size: f32| {
                crate::computer::behave::WeaponThreat {
                    owner,
                    team: self.teams[i],
                    pos: Vec2::new(pos.x, pos.y),
                    vel_x,
                    lr: self.cpu_lr(i),
                    size,
                }
            };
            let mut out = [None, None];
            match self.slots.values()[i] {
                Some(Weapon::Thunder(_)) | None => {}
                Some(Weapon::Trail(t)) => {
                    let r = pikachu::TRAIL_HIT.radius;
                    out[0] = Some(threat(
                        t.owner_port,
                        t.position + Vec3::new(0.0, 120.0, 0.0),
                        0.0,
                        r,
                    ));
                    out[1] = Some(threat(
                        t.owner_port,
                        t.position - Vec3::new(0.0, 120.0, 0.0),
                        0.0,
                        r,
                    ));
                }
                Some(Weapon::PKTrail(t)) => {
                    out[0] = Some(threat(
                        t.owner_port,
                        t.hit_position(),
                        0.0,
                        ness::TRAIL_HIT.radius,
                    ));
                }
                Some(Weapon::PKThunder(h)) => {
                    out[0] = Some(threat(
                        h.owner_port,
                        h.position,
                        h.velocity.x,
                        ness::HEAD_HIT.radius,
                    ));
                }
                Some(_) => {
                    if let Some((owner, hitbox, pos, vel, _)) = self.item_attack(i) {
                        out[0] = Some(threat(owner, pos, vel.x, hitbox.radius));
                    }
                }
            }
            out.into_iter().flatten()
        })
    }

    /// `ftComputerGetOwnWeaponPositionKind(fp, nWPKindPKThunderTrail)`:
    /// the first of the port's PK Thunder trails in the link.
    pub fn own_pk_trail(&self, port: u8) -> Option<Vec3> {
        let (order, n) = self.link_order();
        order[..n]
            .iter()
            .find_map(|&i| match self.slots.values()[i] {
                Some(Weapon::PKTrail(t)) if t.owner_port == port => Some(t.position),
                _ => None,
            })
    }

    /// The live slots in link order.
    fn link_order(&self) -> ([usize; MAX_WEAPONS], usize) {
        let mut order = [0usize; MAX_WEAPONS];
        let mut n = 0;
        for (i, slot) in self.slots.values().iter().enumerate() {
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
        let w = self.slots.values()[i]?;
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
    /// (and every weapon of its group, as the clash view saw them) records
    /// weapon `victim`. A slot refilled since the view holds another weapon.
    fn record_clash(&mut self, i: usize, victim: usize) {
        let id = weapon_victim_id(victim);
        let view = &self.clash_view;
        match view.groups[i] {
            Some(group) => {
                for j in 0..MAX_WEAPONS {
                    if view.groups[j] == Some(group) && !view.refilled[j] {
                        record_weapon_victim(&mut self.hit_records[j], id);
                    }
                }
            }
            None if !view.refilled[i] => record_weapon_victim(&mut self.hit_records[i], id),
            None => {}
        }
    }

    /// The priority-1 view of the weapons for [`Self::search_weapons`]: call
    /// it once a frame after [`Self::tick`] and before [`Self::apply_hits`].
    /// The source's weapons are still whole when their clash search runs
    /// (their hits react at priority 0), while the port's hit searches react
    /// at once, so the search reads the attacks, owners, teams and groups
    /// taken here. Also clears the frame's set-offs and clash damage.
    pub fn begin_clashes(&mut self) {
        self.set_off = [false; MAX_WEAPONS];
        self.clash_damage = [0; MAX_WEAPONS];
        let (order, count) = self.link_order();
        // Built in place, empty slots without the attack calls: a whole view
        // built on the stack and moved, and each slot's attacks copied, cost
        // about 7 KB of copies a frame (RE-580). Neither attack reads the
        // view.
        let view = &mut self.clash_view;
        view.taken = true;
        view.order = order;
        view.count = count;
        view.refilled = [false; MAX_WEAPONS];
        for i in 0..MAX_WEAPONS {
            let Some(w) = &self.slots.values()[i] else {
                let view = &mut self.clash_view;
                view.attacks[i] = None;
                view.item_attacks[i] = None;
                view.groups[i] = None;
                continue;
            };
            let group = w.group();
            let attack = self.clash_attack(i);
            let item_attack = self.item_attack(i);
            let view = &mut self.clash_view;
            view.attacks[i] = attack;
            view.item_attacks[i] = item_attack;
            view.groups[i] = group;
        }
    }

    /// `wpProcessProcSearchHitWeapon` for every weapon, in link order (the
    /// priority-1 process after the fighters' and items' searches, whose
    /// records it reads). Each weapon tests only the weapons after it in the
    /// link that are not its owner's or, with team attack off, its team's,
    /// and that neither side has recorded; the first pair of boxes that
    /// meet clashes (`wpProcessUpdateAttackStatWeapon`): a side whose
    /// priority is not above the other's records it, takes its staled
    /// damage as `hit_attack_damage` and makes a set-off at the pair's
    /// midpoint, the searched weapon's side first.
    ///
    /// Call it after the fighters' and items' hit searches and before
    /// [`Self::finish_clashes`]. It reads the weapons as
    /// [`Self::begin_clashes`] took them (or, without one this frame, as
    /// they stand). The set-offs wait in a queue
    /// ([`Self::flush_clash_effects`]) and the reactions for
    /// [`Self::finish_clashes`].
    pub fn search_weapons(&mut self) {
        if !self.clash_view.taken {
            self.begin_clashes();
        }
        self.clash_view.taken = false;
        let (order, n) = (self.clash_view.order, self.clash_view.count);
        for a in 0..n {
            let this = order[a];
            let Some(this_attack) = self.clash_view.attacks[this] else {
                continue;
            };
            for &other in &order[a + 1..n] {
                let Some(other_attack) = self.clash_view.attacks[other] else {
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
        let refilled = self.clash_view.refilled;
        if v.priority <= o.priority {
            self.record_clash(victim, other);
            if !refilled[victim] {
                self.clash_damage[victim] = self.clash_damage[victim].max(v.damage);
            }
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
            if !refilled[other] {
                self.clash_damage[other] = self.clash_damage[other].max(o.damage);
            }
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
        let mut ness_thunder = None;
        for &i in &order[..n] {
            let damage = core::mem::take(&mut self.clash_damage[i]);
            if damage == 0 || self.set_off[i] {
                continue;
            }
            let seq = self.seq[i];
            let stale = self.stale[i];
            let team = self.teams[i];
            let Some(weapon) = self.slots.slot_value_mut(i) else {
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
                // `wpNessPKThunderHeadProcHit`: its trails are marked and
                // end at their next update; Ness learns of it this pass.
                Weapon::PKThunder(h) => {
                    if !h.reflected {
                        ness_thunder = Some(h.owner_port);
                    }
                    false
                }
                // The Fireball's, Charge Shot's, Final Cutter's and Thunder
                // Jolt's return TRUE.
                _ => false,
            };
            if !alive {
                destroy(&mut self.slots, i);
            }
            if let Some(port) = ness_thunder.take() {
                self.mark_ness_thunder_destroyed(port);
            }
        }
    }

    fn mark_ness_thunder_destroyed(&mut self, port: u8) {
        if let Some(flag) = self.ness_thunder_destroyed.get_mut(usize::from(port)) {
            *flag = true;
        }
    }

    /// Delivers `is_thunder_destroy` writes from the weapons' hit
    /// collisions (`wpNessPKThunderHeadSetDestroyTrails`) to Ness: call it
    /// after [`Self::finish_clashes`], so his next update reads it.
    pub fn sync_hit_owner(&mut self, f: &mut Fighter) {
        let port = usize::from(f.port);
        if port < MAX_OWNERS && core::mem::take(&mut self.ness_thunder_destroyed[port]) {
            f.ness.thunder_destroyed = true;
        }
    }

    /// Makes the weapons' queued effects ([`crate::wpeffect`]) in link
    /// order. Call it at the end of each weapon pass: after a fighter's
    /// weapon is made, after [`Self::tick`] (and the items' pass, which
    /// comes first in the source), and after the hit collisions
    /// (`ftMainProcParams` of every fighter first).
    ///
    /// A trail's frame (`Fx::TextureRand`) is drawn here, in the same link
    /// order, and kept on a Thunder or PK Thunder trail for its draw
    /// (RE-417, RE-550).
    pub fn flush_effects(&mut self, sink: &mut dyn crate::effect::HitEffectSink) {
        let (fx, slots, seqs) = (&mut self.fx, &mut self.slots, &self.seq);
        for (seq, e) in fx.drain_sorted_tagged() {
            let Fx::TextureRand(n) = e else {
                sink.weapon(&e);
                continue;
            };
            let frame = crate::rng::rand_int_range(i32::from(n)) as u8;
            let owner = slots
                .values_mut()
                .zip(seqs)
                .find(|(w, &s)| s == seq && w.is_some());
            match owner {
                Some((Some(Weapon::Trail(t)), _)) => t.texture = frame,
                Some((Some(Weapon::PKTrail(t)), _)) => t.texture = frame,
                _ => {}
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
        self.slots.values().iter().flatten().filter_map(|w| {
            if let Weapon::Jolt(j) = w {
                Some(*j)
            } else {
                None
            }
        })
    }
    pub fn thunder_heads(&self) -> impl Iterator<Item = ThunderHead> + '_ {
        self.slots.values().iter().flatten().filter_map(|w| {
            if let Weapon::Thunder(h) = w {
                Some(*h)
            } else {
                None
            }
        })
    }
    pub fn thunder_trails(&self) -> impl Iterator<Item = ThunderTrail> + '_ {
        self.slots.values().iter().flatten().filter_map(|w| {
            if let Weapon::Trail(t) = w {
                Some(*t)
            } else {
                None
            }
        })
    }

    pub fn active_count(&self) -> usize {
        self.slots.len()
    }

    pub fn first_fireball(&self) -> Option<MarioFireball> {
        self.fireballs().next()
    }

    /// Every live Fireball, for the runtime presentation layer. The pool
    /// remains the sole owner of weapon state; renderers receive snapshots.
    pub fn fireballs(&self) -> impl Iterator<Item = MarioFireball> + '_ {
        self.slots
            .values()
            .iter()
            .flatten()
            .filter_map(|w| match w {
                Weapon::Fireball(f) => Some(*f),
                _ => None,
            })
    }

    pub fn blasters(&self) -> impl Iterator<Item = FoxBlaster> + '_ {
        self.slots
            .values()
            .iter()
            .flatten()
            .filter_map(|w| match w {
                Weapon::Blaster(b) => Some(*b),
                _ => None,
            })
    }

    /// Master Hand's live finger-gun shots.
    pub fn boss_bullets(&self) -> impl Iterator<Item = BossBullet> + '_ {
        self.slots
            .values()
            .iter()
            .flatten()
            .filter_map(|w| match w {
                Weapon::BossBullet(b) => Some(*b),
                _ => None,
            })
    }

    pub fn charge_shots(&self) -> impl Iterator<Item = SamusChargeShot> + '_ {
        self.slots
            .values()
            .iter()
            .flatten()
            .filter_map(|w| match w {
                Weapon::ChargeShot(c) => Some(*c),
                _ => None,
            })
    }

    pub fn bombs(&self) -> impl Iterator<Item = SamusBomb> + '_ {
        self.slots
            .values()
            .iter()
            .flatten()
            .filter_map(|w| match w {
                Weapon::Bomb(b) => Some(*b),
                _ => None,
            })
    }

    pub fn eggs(&self) -> impl Iterator<Item = YoshiEgg> + '_ {
        self.slots
            .values()
            .iter()
            .flatten()
            .filter_map(|w| match w {
                Weapon::Egg(e) => Some(*e),
                _ => None,
            })
    }

    pub fn stars(&self) -> impl Iterator<Item = YoshiStar> + '_ {
        self.slots
            .values()
            .iter()
            .flatten()
            .filter_map(|w| match w {
                Weapon::Star(s) => Some(*s),
                _ => None,
            })
    }

    pub fn cutters(&self) -> impl Iterator<Item = KirbyCutter> + '_ {
        self.slots
            .values()
            .iter()
            .flatten()
            .filter_map(|w| match w {
                Weapon::Cutter(c) => Some(*c),
                _ => None,
            })
    }

    pub fn boomerangs(&self) -> impl Iterator<Item = LinkBoomerang> + '_ {
        self.slots
            .values()
            .iter()
            .flatten()
            .filter_map(|w| match w {
                Weapon::Boomerang(b) => Some(*b),
                _ => None,
            })
    }

    /// The live weapons that play an animation of their own, each with its
    /// pool slot: the slot is the weapon's `GObj` for as long as it lives,
    /// so a presentation layer keys one set of players on it.
    pub fn animated(&self) -> impl Iterator<Item = (usize, AnimatedWeapon)> + '_ {
        self.slots.values().iter().enumerate().filter_map(|(i, w)| {
            let w = match w.as_ref()? {
                Weapon::Boomerang(b) => AnimatedWeapon::Boomerang(*b),
                Weapon::Cutter(c) => AnimatedWeapon::Cutter(*c),
                Weapon::Jolt(j) => AnimatedWeapon::Jolt(*j),
                Weapon::PKFire(p) => AnimatedWeapon::PKFire(*p),
                Weapon::PKThunder(p) => AnimatedWeapon::PKThunder(*p),
                _ => return None,
            };
            Some((i, w))
        })
    }
}

/// A weapon [`WeaponPool::animated`] yields.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AnimatedWeapon {
    Boomerang(LinkBoomerang),
    Cutter(KirbyCutter),
    Jolt(ThunderJolt),
    PKFire(PKFire),
    PKThunder(PKThunder),
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

#[cfg(test)]
mod tests {
    use super::*;

    /// `wpManagerAllocWeapons`' free list as the decomp builds it, and a
    /// scripted make/destroy sequence: the pool hands out the same structs,
    /// and the link (make order) is unchanged by which struct a weapon got.
    #[test]
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
    fn weapon_structs_come_off_the_original_free_list() {
        struct FreeList {
            next: [Option<usize>; MAX_WEAPONS],
            head: Option<usize>,
        }
        let mut list = FreeList {
            next: core::array::from_fn(|i| (i + 1 < MAX_WEAPONS).then_some(i + 1)),
            head: Some(0),
        };
        let mut pool = WeaponPool::default();
        let spawn = WeaponSpawn {
            kind: WeaponKind::MarioFireball,
            owner_port: 0,
            team: 0,
            stale: crate::stale::WeaponStale::FRESH,
            position: Vec3::new(0.0, 100.0, 0.0),
            facing: 1.0,
        };
        // `Some(n)`: destroy the n-th weapon made; `None`: make one.
        let mut script = std::vec![None; 8];
        script.extend([Some(2), Some(6), Some(0), None, None, Some(5), None]);
        script.extend([Some(3), Some(9), None, None]);
        script.extend(std::iter::repeat_n(None, 30));
        let mut made = std::vec::Vec::new();
        let mut destroyed = std::vec::Vec::new();
        let mut not_lowest = 0;
        for step in script {
            match step {
                None => {
                    let empty: std::vec::Vec<bool> =
                        pool.slots.values().iter().map(Option::is_none).collect();
                    // `wpManagerGetNextStructAlloc`.
                    let expected = list.head.inspect(|&w| list.head = list.next[w]);
                    assert_eq!(pool.spawn(spawn), expected.is_some());
                    let got =
                        (0..MAX_WEAPONS).find(|&i| empty[i] && pool.slots.values()[i].is_some());
                    assert_eq!(got, expected, "make {}", made.len());
                    if got.is_some() && got != empty.iter().position(|&e| e) {
                        not_lowest += 1;
                    }
                    made.extend(got);
                }
                Some(n) => {
                    destroy(&mut pool.slots, made[n]);
                    destroyed.push(n);
                    // `wpManagerSetPrevStructAlloc`.
                    list.next[made[n]] = list.head;
                    list.head = Some(made[n]);
                }
            }
        }
        assert_eq!(pool.active_count(), MAX_WEAPONS);
        assert!(not_lowest > 0, "the script tells LIFO from lowest-first");
        let (order, n) = pool.link_order();
        let live: std::vec::Vec<usize> = (0..made.len())
            .filter(|k| !destroyed.contains(k))
            .map(|k| made[k])
            .collect();
        assert_eq!(&order[..n], &live[..], "link = make order");
    }
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
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
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

    /// `ftMainSearchHitWeapon` (priority 1) for every fighter before any
    /// `wpProcessProcHitCollisions` (priority 0): two fighters on one spot
    /// both take a shot that its `proc_hit` then destroys.
    #[test]
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
    fn two_fighters_searched_in_one_frame_both_take_the_shot() {
        use crate::monster_weapon::MonsterShot;
        for shot in 0..2 {
            let mut pool = WeaponPool::default();
            if shot == 0 {
                pool.spawn_monster_shot(MonsterShot::saffron(true, Vec3::new(0.0, 100.0, 0.0)));
            } else {
                assert!(pool.spawn(WeaponSpawn {
                    kind: WeaponKind::MarioFireball,
                    owner_port: 3,
                    team: 3,
                    stale: crate::stale::WeaponStale::FRESH,
                    position: Vec3::new(0.0, 100.0, 0.0),
                    facing: 1.0,
                }));
            }
            let mut a = Fighter::new(FighterKind::Mario, 0, 3);
            let mut b = Fighter::new(FighterKind::Mario, 1, 3);
            pool.search_hits(&mut a);
            pool.search_hits(&mut b);
            pool.react_to_fighter_hits();
            crate::combat::resolve(&mut a);
            crate::combat::resolve(&mut b);
            let damage = if shot == 0 {
                3
            } else {
                MARIO_FIREBALL_HITBOX.damage
            };
            assert_eq!((i32::from(a.damage), i32::from(b.damage)), (damage, damage));
            assert!(pool.slots.values().iter().all(Option::is_none));
        }
    }

    /// `ftMainUpdateAttackStatWeapon`, `...ShieldStatWeapon` and
    /// `...ReflectorStatWeapon` only set the weapon's `hit_attack_damage`,
    /// `hit_shield_damage` and `reflect_gobj`; `wpProcessProcHitCollisions`
    /// (priority 0) runs after every fighter's search (priority 1). A second
    /// fighter searched the same frame still meets the Fireball as it was:
    /// unreflected, with its owner and damage, and its `proc_hit` runs
    /// before the set-off, shield and reflector callbacks.
    #[test]
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
    fn a_set_off_shield_or_reflector_waits_for_every_fighters_search() {
        use crate::monster_weapon::MonsterShot;
        let at = Vec3::new(0.0, 100.0, 0.0);
        let spawn = |shot: &str| {
            let mut pool = WeaponPool::default();
            match shot {
                "fireball" => assert!(pool.spawn(WeaponSpawn {
                    kind: WeaponKind::MarioFireball,
                    owner_port: 3,
                    team: 3,
                    stale: crate::stale::WeaponStale::FRESH,
                    position: at,
                    facing: 1.0,
                })),
                "laser" => assert!(pool.spawn_arwing_laser(ArwingLaser::new(
                    false,
                    at,
                    Vec3::new(-60.0, 0.0, 0.0),
                    Vec3::ZERO,
                    0.0
                ))),
                "pk fire" | "pk thunder" => assert!(pool.spawn(WeaponSpawn {
                    kind: if shot == "pk fire" {
                        WeaponKind::NessPKFire { grounded: false }
                    } else {
                        WeaponKind::NessPKThunder
                    },
                    owner_port: 3,
                    team: 3,
                    stale: crate::stale::WeaponStale::FRESH,
                    position: at,
                    facing: 1.0,
                })),
                _ => {
                    pool.spawn_monster_shot(MonsterShot::saffron(true, at));
                }
            }
            pool
        };
        let cases = [
            ("fireball", "set-off"),
            ("fireball", "shield"),
            ("fireball", "reflector"),
            ("laser", "shield"),
            ("laser", "reflector"),
            ("razor", "shield"),
            ("razor", "reflector"),
            ("pk fire", "set-off"),
            ("pk fire", "shield"),
            ("pk fire", "reflector"),
            ("pk thunder", "set-off"),
            ("pk thunder", "shield"),
            ("pk thunder", "reflector"),
        ];
        for (shot, first) in cases {
            // The second fighter alone.
            let mut pool = spawn(shot);
            let mut alone = Fighter::new(FighterKind::Mario, 2, 3);
            pool.apply_hits(&mut alone);
            crate::combat::resolve(&mut alone);
            assert!(alone.damage > 0, "{shot}");
            let mut pool = spawn(shot);
            let mut a = match first {
                "set-off" => swinging(FighterKind::Mario, 15, at),
                "shield" => {
                    let mut f = Fighter::new(FighterKind::Mario, 1, 3);
                    f.situation = Situation::Ground;
                    f.guard.is_shield = true;
                    f.guard.shield_health = 55.0;
                    f
                }
                _ => {
                    let mut fox = Fighter::new(FighterKind::Fox, 1, 3);
                    fox.situation = Situation::Ground;
                    crate::status::set_fox_special_lw_hit(&mut fox);
                    fox
                }
            };
            let mut b = Fighter::new(FighterKind::Mario, 2, 3);
            pool.search_hits(&mut a);
            assert_eq!(pool.active_count(), 1, "{shot} {first}: the weapon waits");
            pool.search_hits(&mut b);
            pool.react_to_fighter_hits();
            crate::combat::resolve(&mut a);
            crate::combat::resolve(&mut b);
            assert_eq!(a.damage, 0, "{shot} {first}");
            assert_eq!(
                b.damage, alone.damage,
                "{shot} {first}: the second fighter takes the unreflected shot"
            );
            if shot != "razor" {
                assert_eq!(pool.active_count(), 0, "{shot} {first}: `proc_hit` ends it");
            }
        }
    }

    /// `wpNessPKThunderHeadProcReflector` runs at priority 0, after
    /// `wpProcessProcHitCollisions` has given the head its reflector's
    /// owner, team and (US) `stat_flags`. `wpNessPKReflectHeadMakeWeapon`'s
    /// new weapon copies them, the old head's `wp->lr` and its staling,
    /// takes the attributes' damage (the callback returns TRUE: no reflect
    /// bonus) and starts with empty attack records. A second reflection
    /// (`wpNessPKReflectHeadProcReflector`, FALSE) earns the bonus.
    #[test]
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
    fn a_reflected_pk_thunder_is_a_new_weapon_at_the_attributes_damage() {
        let at = Vec3::new(0.0, 100.0, 0.0);
        let mut pool = WeaponPool::default();
        assert!(pool.spawn(WeaponSpawn {
            kind: WeaponKind::NessPKThunder,
            owner_port: 3,
            team: 3,
            stale: crate::stale::WeaponStale::FRESH,
            position: at,
            facing: 1.0,
        }));
        pool.hit_records[0] = [Some(2), None, None, None];
        let reflector = |port: u8| {
            let mut fox = Fighter::new(FighterKind::Fox, port, 3);
            fox.situation = Situation::Ground;
            fox.facing = crate::fighter::Facing::Left;
            crate::status::set_fox_special_lw_hit(&mut fox);
            fox
        };
        let mut fox = reflector(1);
        pool.search_hits(&mut fox);
        // The search only names the reflector.
        let h = pool.pk_thunders().next().unwrap();
        assert!(!h.reflected);
        assert_eq!(h.owner_port, 3);
        pool.react_to_fighter_hits();
        crate::combat::resolve(&mut fox);
        pool.react_to_reflectors(&|_| Some(fox.facing.sign()));
        let h = pool.pk_thunders().next().unwrap();
        assert!(h.reflected);
        assert_eq!(h.owner_port, 1);
        assert_eq!(h.damage, 6);
        assert_eq!(h.lifetime, 160);
        // Aimed away from the reflector's TopN plus 250.
        assert_eq!(h.velocity, Vec3::new(0.0, -60.0, 0.0));
        assert_eq!(pool.teams[0], fox.team);
        assert_eq!(pool.stale[0].stat, fox.stats.attack);
        assert_eq!(pool.cpu_lr(0), 1.0, "the old head's lr");
        assert_eq!(pool.hit_records[0], [None; 4]);
        let mut again = reflector(2);
        pool.search_hits(&mut again);
        pool.react_to_fighter_hits();
        crate::combat::resolve(&mut again);
        pool.react_to_reflectors(&|_| Some(again.facing.sign()));
        let h = pool.pk_thunders().next().unwrap();
        assert_eq!(h.owner_port, 2);
        assert_eq!(h.damage, 11, "6 * 1.8 + 0.99");
        assert_eq!(h.velocity.x, 0.0);
    }

    /// `ftMainUpdateDamageStatWeapon` sets `hit_normal_damage` before it
    /// tests the hit statuses: PK Fire's spark makes its pillar on an
    /// invincible fighter too (`wpNessPKFireProcHit`).
    #[test]
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
    fn pk_fire_makes_its_pillar_on_an_invincible_fighter() {
        let mut pool = WeaponPool::default();
        assert!(pool.spawn(WeaponSpawn {
            kind: WeaponKind::NessPKFire { grounded: false },
            owner_port: 3,
            team: 3,
            stale: crate::stale::WeaponStale::FRESH,
            position: Vec3::new(0.0, 100.0, 0.0),
            facing: 1.0,
        }));
        let mut f = Fighter::new(FighterKind::Mario, 1, 3);
        crate::hurtbox::set_hit_status_all(&mut f, crate::combat::HitStatus::Invincible);
        pool.apply_hits(&mut f);
        crate::combat::resolve(&mut f);
        assert_eq!(f.damage, 0);
        assert_eq!(pool.active_count(), 0);
        assert_eq!(pool.take_item_spawns().count(), 1);
    }

    /// `ftMainUpdateAttackStatWeapon` records the fighter in the weapon it
    /// sets off (`wpProcessUpdateHitInteractStats`, a group other than 7):
    /// the Star, which lives on, passes through it afterwards.
    #[test]
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
    fn a_star_passes_the_fighter_that_set_it_off() {
        let at = Vec3::new(0.0, 100.0, 0.0);
        let pool_spawn = |at| WeaponSpawn {
            kind: WeaponKind::YoshiStars,
            owner_port: 3,
            team: 3,
            stale: crate::stale::WeaponStale::FRESH,
            position: at,
            facing: 1.0,
        };
        let mut pool = WeaponPool::default();
        assert!(pool.spawn(pool_spawn(at)));
        let stars = pool.stars().count();
        let star = pool.stars().next().unwrap().position;
        let mut f = swinging(FighterKind::Mario, 15, star);
        f.pos = star - at;
        // Without the attack, the Star hurts a fresh fighter there.
        let mut fresh = f.clone();
        fresh.attack_colls[0].state = crate::combat::AttackState::Off;
        let mut control = WeaponPool::default();
        assert!(control.spawn(pool_spawn(at)));
        control.apply_hits(&mut fresh);
        crate::combat::resolve(&mut fresh);
        assert!(fresh.damage > 0);
        pool.apply_hits(&mut f);
        crate::combat::resolve(&mut f);
        assert_eq!(f.damage, 0);
        assert_eq!(pool.stars().count(), stars, "the Star survives");
        f.attack_colls[0].state = crate::combat::AttackState::Off;
        pool.apply_hits(&mut f);
        crate::combat::resolve(&mut f);
        assert_eq!(f.damage, 0);
    }

    #[test]
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
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
        items.take_weapon_spawns(&mut spawner, open_air, &mut crate::effect::NoEffects);
        items
    }

    #[test]
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
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
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
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
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
    fn surviving_weapon_evicts_first_item_record_after_four_victims() {
        let mut items = flames_at_damage_centres(&[Vec3::ZERO; 5]);
        let mut weapons = WeaponPool::default();
        weapons.spawn(WeaponSpawn {
            kind: WeaponKind::KirbyCutter { floor_line: None },
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
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
    fn item_victims_share_the_four_records_with_fighter_victims() {
        let mut items = flames_at_damage_centres(&[Vec3::ZERO; 4]);
        let mut weapons = WeaponPool::default();
        weapons.spawn(WeaponSpawn {
            kind: WeaponKind::KirbyCutter { floor_line: None },
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
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
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
        assert_eq!(
            fireball.lifetime,
            std::hint::black_box(MARIO_FIREBALL_LIFETIME) - 1
        );
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
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
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
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
    fn yoshi_egg_explodes_on_hit_and_bomb_spawns_two_stars() {
        let mut weapons = WeaponPool::default();
        assert!(weapons.spawn(WeaponSpawn {
            kind: WeaponKind::YoshiEgg {
                throw_force: 10,
                stick_x: 0,
                scale: Vec3::new(1.0, 1.0, 1.0),
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
                    scale: Vec3::new(1.0, 1.0, 1.0),
                },
                owner_port: 0,
                team: 0,
                stale: crate::stale::WeaponStale::FRESH,
                position: Vec3::ZERO,
                facing: 1.0,
            },
            10,
            0,
            Vec3::new(1.0, 1.0, 1.0),
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

    /// `wpManagerMakeWeapon` copies the owner's handicap (`wp->handicap =
    /// fp->handicap`, wpmanager.c:123): Master Hand's bullet (handicap 36
    /// in the 1P Game) knocks back about 8% less than a default-handicap
    /// shot, as the N64 run measured (RE-457).
    #[test]
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
    fn a_weapon_hits_with_its_owners_handicap() {
        let knockback = |handicap: u8| {
            let mut owner = Fighter::new(FighterKind::Boss, 1, 3);
            owner.handicap = handicap;
            let stale = crate::stale::WeaponStale::of(&owner);
            assert_eq!(stale.handicap, handicap);
            let mut weapons = WeaponPool::default();
            assert!(weapons.spawn(WeaponSpawn {
                kind: WeaponKind::BossBullet { hard: false },
                owner_port: 1,
                team: 1,
                stale,
                position: Vec3::ZERO,
                facing: -1.0,
            }));
            weapons.tick(open_air, None);
            let Some(Weapon::BossBullet(b)) =
                weapons.slots.values().iter().flatten().next().copied()
            else {
                panic!("no bullet");
            };
            let mut target = Fighter::new(FighterKind::Kirby, 0, 3);
            target.pos = b.position;
            target.situation = Situation::Ground;
            target.damage = 50;
            weapons.apply_hits(&mut target);
            crate::combat::resolve(&mut target);
            assert!(target.damage > 50, "the bullet missed");
            target.damage_knockback_stack
        };
        let default = knockback(crate::stale::HANDICAP_DEFAULT);
        let boss = knockback(36);
        assert!(default > 0.0);
        assert!(boss < default * 0.95, "handicap 36 {boss} vs 9 {default}");
    }

    /// `wpManagerMakeWeapon` captures the owner's stale factor; the hit
    /// deals `damage * stale + 0.999` and records the motion in the owner's
    /// queue (`ftMainUpdateDamageStatWeapon`).
    #[test]
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
    fn a_repeated_egg_throw_is_staled_and_recorded() {
        use crate::stale::MotionAttackId;
        let mut owner = Fighter::new(FighterKind::Yoshi, 0, 3);
        owner.set_motion_id(MotionAttackId::SpecialHi);
        owner
            .stale
            .push(MotionAttackId::SpecialHi, owner.motion.count);
        owner.set_motion_id(MotionAttackId::SpecialHi);
        let stale = crate::stale::WeaponStale::of(&owner);
        assert_eq!(stale.stale, 0.75);
        let mut weapons = WeaponPool::default();
        assert!(weapons.spawn(WeaponSpawn {
            kind: WeaponKind::YoshiEgg {
                throw_force: 10,
                stick_x: 0,
                scale: Vec3::new(1.0, 1.0, 1.0),
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
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
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
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
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

    /// `wpMainReflectorSetLR` reads Fox's `lr` in the weapon's priority-0
    /// pass, after his `ftMainProcParams` turned him to the shot
    /// (`ftFoxSpecialLwHitSetStatus`: `lr = reflect_lr`). A Fox facing away
    /// from a Fireball coming from behind still sends it back (N64 1P
    /// stage 4, tick 1421: the shot then hits its Mario).
    #[test]
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
    fn a_reflector_facing_away_turns_the_shot_with_its_new_facing() {
        let mut weapons = WeaponPool::default();
        weapons.spawn(WeaponSpawn {
            kind: WeaponKind::MarioFireball,
            owner_port: 0,
            team: 0,
            stale: crate::stale::WeaponStale::FRESH,
            position: Vec3::new(-100.0, 60.0, 0.0),
            facing: 1.0,
        });
        let mut fox = Fighter::new(FighterKind::Fox, 1, 3);
        fox.pos = Vec3::ZERO;
        fox.facing = crate::fighter::Facing::Right;
        fox.situation = Situation::Ground;
        crate::status::set_fox_special_lw_start(&mut fox);
        crate::status::set_any_status(
            &mut fox,
            crate::status::AnyStatus::Fox(crate::status::FoxStatus::SpecialLwLoop),
            0.0,
            crate::status::StatusTiming::unknown(),
        );
        weapons.search_hits(&mut fox);
        weapons.react_to_fighter_hits();
        crate::combat::resolve(&mut fox);
        assert_eq!(fox.facing.sign(), -1.0, "turned to the shot");
        weapons.react_to_reflectors(&|_| Some(fox.facing.sign()));
        let fireball = weapons.first_fireball().unwrap();
        assert_eq!(fireball.owner_port, fox.port);
        assert!(fireball.velocity.x < 0.0, "{:?}", fireball.velocity);
    }

    /// `ftMainUpdateReflectorStatWeapon` records the reflector
    /// (`wpProcessUpdateHitInteractStats`, `nGMHitTypeReflect`) and
    /// `wpProcessProcHitCollisions` keeps the records: a Fireball one Fox
    /// turned back and another turned again passes through the first.
    #[test]
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
    fn a_shot_passes_through_the_fox_that_reflected_it() {
        let mut weapons = fireball_at(Vec3::new(100.0, 60.0, 0.0), -1.0);
        let fox = |port: u8, reflecting: bool| {
            let mut fox = Fighter::new(FighterKind::Fox, port, 3);
            fox.pos = Vec3::ZERO;
            fox.situation = Situation::Ground;
            if reflecting {
                crate::status::set_fox_special_lw_start(&mut fox);
                crate::status::set_any_status(
                    &mut fox,
                    crate::status::AnyStatus::Fox(crate::status::FoxStatus::SpecialLwLoop),
                    0.0,
                    crate::status::StatusTiming::unknown(),
                );
            }
            fox
        };
        for port in [1, 2] {
            let mut reflector = fox(port, true);
            weapons.apply_hits(&mut reflector);
            crate::combat::resolve(&mut reflector);
            assert_eq!(weapons.first_fireball().unwrap().owner_port, port);
        }
        let mut first = fox(1, false);
        weapons.apply_hits(&mut first);
        crate::combat::resolve(&mut first);
        assert_eq!(first.damage, 0);
        assert!(weapons.first_fireball().is_some());
    }

    /// `ftMainSearchHitWeapon` walks `gGCCommonLinks[nGCCommonLinkIDWeapon]`:
    /// of two equal hits the first in the link sets the fighter's
    /// `damage_lr` (the damage status keeps the first of equal
    /// knockbacks). A freed slot refills with a newer weapon, last in the link.
    #[test]
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
    fn the_weapon_search_walks_the_link_not_the_slots() {
        let spawn = |weapons: &mut WeaponPool, x: f32, facing: f32| {
            assert!(weapons.spawn(WeaponSpawn {
                kind: WeaponKind::MarioFireball,
                owner_port: 0,
                team: 0,
                stale: crate::stale::WeaponStale::FRESH,
                position: Vec3::new(x, 60.0, 0.0),
                facing,
            }));
        };
        let lr = |weapons: &mut WeaponPool| {
            let mut f = Fighter::new(FighterKind::Mario, 1, 3);
            f.situation = Situation::Ground;
            weapons.apply_hits(&mut f);
            crate::combat::resolve(&mut f);
            assert!(f.damage > 0);
            // The damage status faces the fighter against `damage_lr`.
            f.facing
        };
        let mut left = WeaponPool::default();
        spawn(&mut left, -60.0, 1.0);
        let mut right = WeaponPool::default();
        spawn(&mut right, 60.0, -1.0);
        let (left_lr, right_lr) = (lr(&mut left), lr(&mut right));
        assert_ne!(left_lr, right_lr);
        // Slot 0 frees, then the right shot refills it: slot order would
        // search it first.
        let mut both = WeaponPool::default();
        spawn(&mut both, 5000.0, 1.0);
        spawn(&mut both, -60.0, 1.0);
        both.slots.remove_at(0);
        spawn(&mut both, 60.0, -1.0);
        assert!(matches!(both.slots.values()[0], Some(Weapon::Fireball(f)) if f.position.x > 0.0));
        assert_eq!(lr(&mut both), left_lr);
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
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
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
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
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
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
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
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
    fn a_strong_weapon_breaks_foxs_reflector() {
        let mut fox = Fighter::new(FighterKind::Fox, 1, 3);
        fox.situation = Situation::Ground;
        crate::status::set_fox_special_lw_hit(&mut fox);
        assert!(crate::combat::reflector(&fox).is_some());
        let mut weapons = fireball_at(Vec3::new(100.0, 60.0, 0.0), -1.0);
        for w in weapons.slots.values_mut().flatten() {
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
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
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
            kind: WeaponKind::KirbyCutter { floor_line: None },
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
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
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
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
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
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
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
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
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
            if bomb.grounded {
                settled = Some(tick);
                break;
            }
        }
        assert!(settled.is_some(), "the bomb comes to rest before its fuse");
        let bomb = weapons.bombs().next().unwrap();
        assert!((bomb.position.y - 75.0).abs() < 1.0);
        weapons.tick(|| floor, None);
        let bomb = weapons.bombs().next().unwrap();
        assert!(bomb.grounded);
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
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
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
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
    fn fireball_rebounds_from_every_authored_map_direction() {
        // A line of each kind 50 units out, crossed by a 100-unit step:
        // `wpMapTestAll` stops the diamond on it and `wpMapCheckAllRebound`
        // reflects the velocity off that kind's normal. The four kinds face
        // different ways, so this catches a swapped LeftWall/RightWall.
        let cases = [
            (
                surface(MapSurfaceKind::Floor, -100, -50, 100, -50),
                Vec3::new(0.0, -100.0, 0.0),
                Vec2::new(0.0, 1.0),
            ),
            (
                surface(MapSurfaceKind::Ceiling, -100, 50, 100, 50),
                Vec3::new(0.0, 100.0, 0.0),
                Vec2::new(0.0, -1.0),
            ),
            (
                surface(MapSurfaceKind::LeftWall, 50, -100, 50, 100),
                Vec3::new(100.0, 0.0, 0.0),
                Vec2::new(-1.0, 0.0),
            ),
            (
                surface(MapSurfaceKind::RightWall, -50, -100, -50, 100),
                Vec3::new(-100.0, 0.0, 0.0),
                Vec2::new(1.0, 0.0),
            ),
        ];
        for (surface, delta, normal) in cases {
            let mut coll = crate::wpmap::WeaponColl::default();
            let mut pos = delta;
            coll.begin();
            coll.test_all(&MARIO_FIREBALL_MAP_COLL, Vec3::ZERO, &mut pos, &|| {
                [surface]
            });
            assert_ne!(pos, delta, "{:?} stops the step", surface.kind);
            let mut vel = delta;
            let at = coll.check_all_rebound(
                &mut vel,
                crate::wpmap::MASK_MAIN,
                1.0,
                pos,
                &MARIO_FIREBALL_MAP_COLL,
            );
            assert!(at.is_some(), "{:?} rebounds", surface.kind);
            let dot = delta.x * normal.x + delta.y * normal.y;
            assert_eq!(
                (vel.x, vel.y),
                (
                    delta.x - 2.0 * dot * normal.x,
                    delta.y - 2.0 * dot * normal.y
                ),
                "{:?}",
                surface.kind
            );
        }
    }

    /// `wpBossBulletProcMap`: the frame the shot meets the map it sparkles
    /// at its `translate`, its whole step on, and ends.
    #[test]
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
    fn a_boss_bullet_ends_at_its_step_on_meeting_the_map() {
        let floor = surface(MapSurfaceKind::Floor, -8000, -100, 8000, -100);
        let mut b = BossBullet::new(
            WeaponSpawn {
                kind: WeaponKind::BossBullet { hard: false },
                owner_port: 0,
                team: 0,
                position: Vec3::ZERO,
                facing: 1.0,
                stale: crate::stale::WeaponStale::FRESH,
            },
            false,
        );
        let mut fx = crate::wpeffect::Emit::default();
        let mut from = b.position;
        let mut ticks = 0;
        while b.tick(|| [floor], &mut fx) {
            from = b.position;
            ticks += 1;
            assert!(ticks < 40);
        }
        assert_eq!(b.position, from + b.velocity);
        let sparkles: std::vec::Vec<_> = fx
            .iter()
            .filter_map(|e| match e {
                Fx::SparkleWhiteMultiExplode(p) => Some(p),
                _ => None,
            })
            .collect();
        assert_eq!(sparkles, [b.position]);
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
        let mut w = weapons.slots.values()[0].unwrap();
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
        let mut w = weapons.slots.values()[0].unwrap();
        assert!(w.on_shield(
            shield_at(120.0, -1.0),
            &mut crate::wpeffect::Emit::default()
        ));
        let Weapon::Fireball(after) = w else {
            unreachable!()
        };
        assert!((after.velocity.y - (-v.x * sin + v.y * cos)).abs() < 1e-3);
        // Under 90 degrees the angle clamps to zero.
        let mut w = weapons.slots.values()[0].unwrap();
        assert!(w.on_shield(shield_at(60.0, 1.0), &mut crate::wpeffect::Emit::default()));
        let Weapon::Fireball(after) = w else {
            unreachable!()
        };
        assert!((after.velocity - v).length() < 1e-4);
        let mut w = weapons.slots.values()[0].unwrap();
        assert!(!w.on_shield(shield_at(135.0, 1.0), &mut crate::wpeffect::Emit::default()));
    }

    /// `wpMainVelSetModelPitch`'s yaw, `wpKirbyCutterProcUpdate`'s slope
    /// roll and one animation tick per update.
    #[test]
    fn cutter_wave_yaw_follows_its_velocity_and_roll_its_floor() {
        let spawn = |facing| WeaponSpawn {
            kind: WeaponKind::KirbyCutter {
                floor_line: Some(0),
            },
            owner_port: 0,
            team: 0,
            stale: crate::stale::WeaponStale::FRESH,
            position: Vec3::new(0.0, 100.0, 0.0),
            facing,
        };
        let left = KirbyCutter::new(spawn(-1.0));
        assert_eq!(left.model_rotate_y, -core::f32::consts::FRAC_PI_2);
        // A floor rising 100 over 1000 to the right.
        let slope = [surface(MapSurfaceKind::Floor, -1000, 0, 1000, 200)];
        let mut c = KirbyCutter::new(spawn(1.0));
        c.make_floor(Some(0), spawn(1.0).position, &|| slope);
        assert_eq!(c.model_rotate_y, core::f32::consts::FRAC_PI_2);
        assert!(c.tick(|| slope, &mut crate::wpeffect::Emit::default()));
        assert_eq!(c.anim_ticks, 1);
        assert!(c.grounded);
        let want = ssb_engine::math::sy_arc_tan2(100.0, 1000.0);
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
        bomb.grounded = true;
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
        jolt.ride(surface(MapSurfaceKind::Floor, -500, 0, 500, 0), 1);
        assert!(!Weapon::Jolt(jolt)
            .on_shield(shield_at(100.0, 1.0), &mut crate::wpeffect::Emit::default()));
        // The Final Cutter wave cannot hop and flies on.
        let cutter = KirbyCutter::new(spawn(WeaponKind::KirbyCutter { floor_line: None }));
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
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
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
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
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

    #[test]
    fn the_cpu_reads_a_weapons_make_lr_not_its_velocity() {
        let mut pool = WeaponPool::default();
        assert!(pool.spawn(WeaponSpawn {
            kind: WeaponKind::FoxBlaster,
            owner_port: 2,
            team: 2,
            position: Vec3::new(500.0, 100.0, 0.0),
            facing: -1.0,
            stale: crate::stale::WeaponStale::FRESH,
        }));
        let t = pool.cpu_threats().next().unwrap();
        assert_eq!((t.owner, t.team, t.lr), (2, 2, -1.0));
        assert!(t.vel_x < 0.0);
        assert_eq!(t.size, FOX_BLASTER_HITBOX.radius);
        // A reflector turns the shot's velocity, not its `wp->lr`.
        if let Some(Weapon::Blaster(b)) = pool.slots.values_mut().flatten().next() {
            b.velocity.x = -b.velocity.x;
        }
        let t = pool.cpu_threats().next().unwrap();
        assert!(t.vel_x > 0.0);
        assert_eq!(t.lr, -1.0);
        // PK Fire rederives it from the velocity.
        let mut pool = WeaponPool::default();
        assert!(pool.spawn(WeaponSpawn {
            kind: WeaponKind::NessPKFire { grounded: true },
            owner_port: 1,
            team: 1,
            position: Vec3::ZERO,
            facing: 1.0,
            stale: crate::stale::WeaponStale::FRESH,
        }));
        if let Some(Weapon::PKFire(p)) = pool.slots.values_mut().flatten().next() {
            p.velocity.x = -p.velocity.x.abs();
        }
        assert_eq!(pool.cpu_threats().next().unwrap().lr, -1.0);
    }
}

#[cfg(test)]
mod reflector_rotate_tests {
    use super::*;
    use ssb_engine::math::sy_arc_tan2;

    /// The renderer's former formula: the cross product of the raw
    /// velocity, which divides `vy / vx` unscaled.
    fn raw_pitch(v: Vec3) -> f32 {
        if v.x > 0.0 {
            sy_arc_tan2(-v.y, v.x)
        } else {
            sy_arc_tan2(-v.y, -v.x)
        }
    }

    /// `wpMainReflectorRotateWeaponModel` spelled out step by step:
    /// `syVectorNorm3D` then `lbCommonCross3D` with `(0, 0, -+1)`.
    fn decomp(vel: Vec3) -> [f32; 3] {
        let dz: f32 = if vel.x > 0.0 { -1.0 } else { 1.0 };
        let mag = ssb_engine::math::sqrt_exact(vel.x * vel.x + vel.y * vel.y + vel.z * vel.z);
        // A zero vector returns as it is.
        let (x, y, z) = if mag == 0.0 {
            (vel.x, vel.y, vel.z)
        } else {
            (
                vel.x * (1.0 / mag),
                vel.y * (1.0 / mag),
                vel.z * (1.0 / mag),
            )
        };
        let ax = y * dz - z * 0.0;
        let ay = z * 0.0 - x * dz;
        let deg = |d: f32| ssb_engine::math::f_cst_dtor32(d);
        if dz == -1.0 {
            [sy_arc_tan2(ax, ay), deg(90.0), 0.0]
        } else {
            [sy_arc_tan2(-ax, ay), deg(-90.0), 0.0]
        }
    }

    /// The normalised velocity's ratio rounds differently from the raw
    /// one: (-160, -24) pitches an ulp lower than the unscaled formula gave.
    #[test]
    fn the_velocity_is_normalised_before_the_cross_product() {
        let v = Vec3::new(-160.0, -24.0, 0.0);
        let r = reflector_rotate(v);
        assert_eq!(r[0].to_bits(), 0x3e18_769b);
        assert_eq!(raw_pitch(v).to_bits(), 0x3e18_769c);
        assert_eq!(r.map(f32::to_bits), decomp(v).map(f32::to_bits));
        assert_eq!(r[1], -core::f32::consts::FRAC_PI_2);
    }

    /// Over a grid of velocities in both directions the port matches the
    /// decomp's steps bit for bit; the raw formula differs on many.
    #[test]
    fn the_rotation_matches_the_decomp_over_a_velocity_grid() {
        let mut raw_differs = 0;
        for x in -200..=200 {
            for y in -60..=60 {
                let v = Vec3::new(x as f32 * 0.8, y as f32 * 0.5, (y % 3) as f32);
                let r = reflector_rotate(v);
                assert_eq!(r.map(f32::to_bits), decomp(v).map(f32::to_bits), "{v:?}");
                if v.z == 0.0 && r[0].to_bits() != raw_pitch(v).to_bits() {
                    raw_differs += 1;
                }
            }
        }
        assert!(raw_differs > 1000);
        // Master Hand's bullets (`vel = { 160 * lr, -25 }`): the same either
        // way.
        for lr in [-1.0, 1.0] {
            let v = Vec3::new(
                crate::boss::BULLET_VEL_X * lr,
                crate::boss::BULLET_VEL_Y,
                0.0,
            );
            assert_eq!(reflector_rotate(v)[0].to_bits(), raw_pitch(v).to_bits());
        }
    }
}

#[cfg(test)]
mod map_pass_tests {
    //! Batch 16: the `wpmap.c` passes (W1-W4).
    use super::*;
    use crate::map::{MASK_CEIL, MASK_FLOOR, MASK_LWALL};

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

    fn cutter_on<const N: usize>(map: [MapSurface; N], line: u16, x: f32, y: f32) -> KirbyCutter {
        let spawn = WeaponSpawn {
            kind: WeaponKind::KirbyCutter {
                floor_line: Some(line),
            },
            owner_port: 0,
            team: 0,
            stale: crate::stale::WeaponStale::FRESH,
            position: Vec3::new(x, y - KIRBY_CUTTER_MAP_COLL.bottom, 0.0),
            facing: 1.0,
        };
        let mut c = KirbyCutter::new(spawn);
        c.make_floor(Some(line), spawn.position, &|| map);
        c
    }

    /// W1, `wpKirbyCutterProcMap` (wpkirbycutter.c:71,85): the ground pass
    /// tests the walls, and a wall in `mask_curr` ends the wave with dust.
    #[test]
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
    fn a_grounded_cutter_dies_on_a_wall() {
        let map = [
            surface(MapSurfaceKind::Floor, -2000, 0, 2000, 0),
            surface(MapSurfaceKind::LeftWall, 600, 0, 600, 1000),
        ];
        let mut c = cutter_on(map, 0, 0.0, 0.0);
        for _ in 0..KIRBY_CUTTER_LIFETIME - 2 {
            let mut fx = crate::wpeffect::Emit::default();
            if !c.tick(|| map, &mut fx) {
                assert!(c.coll.mask_curr & MASK_LWALL != 0);
                assert!(c.position.x <= 600.0 - KIRBY_CUTTER_MAP_COLL.width);
                assert!(fx.iter().any(|e| matches!(e, Fx::DustExpandSmall(_))));
                return;
            }
            assert!(c.grounded);
            assert!(c.position.x < 600.0);
        }
        panic!("the wave passed the wall");
    }

    /// W2, `wpSamusBombProcMap` (wpsamusbomb.c:152): a grounded Bomb takes
    /// `wpMapTestLRWallCheckFloor`, so a wall stops it.
    #[test]
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
    fn a_grounded_bomb_stops_at_a_wall() {
        let map = [
            surface(MapSurfaceKind::Floor, -2000, 0, 2000, 0),
            surface(MapSurfaceKind::LeftWall, 600, 0, 600, 1000),
        ];
        let mut bomb = SamusBomb::new(WeaponSpawn {
            kind: WeaponKind::SamusBomb,
            owner_port: 0,
            team: 0,
            stale: crate::stale::WeaponStale::FRESH,
            position: Vec3::new(500.0, -SAMUS_BOMB_MAP_COLL.bottom, 0.0),
            facing: 1.0,
        });
        bomb.grounded = true;
        bomb.vel_ground = 7.0;
        bomb.coll.floor.set(crate::ground::Standing {
            line: 0,
            flags: 0,
            normal: Vec2::new(0.0, 1.0),
        });
        let mut walled = false;
        for _ in 0..60 {
            assert!(bomb.tick(|| map, &mut crate::wpeffect::Emit::default()));
            assert!(bomb.grounded);
            assert!(bomb.position.x <= 600.0 - SAMUS_BOMB_MAP_COLL.width);
            assert_eq!(bomb.position.y, -SAMUS_BOMB_MAP_COLL.bottom);
            walled |= bomb.coll.mask_curr & MASK_LWALL != 0;
        }
        assert!(walled);
    }

    /// W3: leaving the line's end snaps the wave to the corner
    /// (`mpProcessCheckTestFloorCollisionNew`, mpprocess.c:968) before
    /// `wpMapSetAir` aims it down the last slope.
    #[test]
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
    fn a_grounded_cutter_leaves_a_ledge_from_its_corner() {
        // Falling 200 over 1500 to the right, ending at (500, 0).
        let map = [surface(MapSurfaceKind::Floor, -1000, 200, 500, 0)];
        let mut c = cutter_on(map, 0, 0.0, 200.0 / 3.0);
        for _ in 0..10 {
            assert!(c.tick(|| map, &mut crate::wpeffect::Emit::default()));
            if !c.grounded {
                assert!(c.position.x > 500.0);
                assert_eq!(c.position.y, -KIRBY_CUTTER_MAP_COLL.bottom);
                // `cos/sin(rotate.z) * vel_ground`, down the slope.
                assert!(c.velocity.x > 0.0 && c.velocity.y < 0.0);
                let speed = Vec2::new(c.velocity.x, c.velocity.y).length();
                assert!((speed - KIRBY_CUTTER_VEL).abs() < 1e-3);
                return;
            }
            // `vel_air` stays level: full speed in x, the floor sets y.
            assert_eq!(c.velocity, Vec3::new(KIRBY_CUTTER_VEL, 0.0, 0.0));
        }
        panic!("the wave never left the ledge");
    }

    /// W3: a grounded wave swept onto another floor line lands on it and
    /// follows it; `MAP_FLAG_FLOOR` marks only that line change (RE-554).
    #[test]
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
    fn a_grounded_cutter_follows_a_floor_line_change() {
        let map = [
            surface(MapSurfaceKind::Floor, -1000, 0, 0, 0),
            // Rising 1 in 5, crossing y = 0 at x = 0.
            surface(MapSurfaceKind::Floor, -100, -20, 1000, 200),
        ];
        let mut c = cutter_on(map, 0, -50.0, 0.0);
        assert!(c.tick(|| map, &mut crate::wpeffect::Emit::default()));
        assert!(c.grounded);
        assert_eq!(c.coll.floor.line.unwrap(), 1);
        assert!(c.coll.mask_curr & MASK_FLOOR != 0);
        let y = 50.0 * 0.2 - KIRBY_CUTTER_MAP_COLL.bottom;
        assert!((c.position.y - y).abs() < 1e-3, "{}", c.position.y);
        assert!(c.tick(|| map, &mut crate::wpeffect::Emit::default()));
        assert!(c.grounded);
        assert_eq!(c.coll.floor.line.unwrap(), 1);
        assert_eq!(c.coll.mask_curr & MASK_FLOOR, 0);
        let y = 150.0 * 0.2 - KIRBY_CUTTER_MAP_COLL.bottom;
        assert!((c.position.y - y).abs() < 1e-3, "{}", c.position.y);
    }

    /// W4, `wpMapCheckAllRebound` (wpmap.c:174): only a surface first
    /// touched this frame (`mask_prev ^ mask_curr`) that the velocity moves
    /// into reflects it, and the scale follows any reflection.
    #[test]
    fn rebound_needs_a_fresh_contact_moving_into_it() {
        let body = BodyColl {
            top: 20.0,
            center: 10.0,
            bottom: 0.0,
            width: 10.0,
        };
        let mut coll = crate::wpmap::WeaponColl {
            mask_curr: MASK_LWALL,
            lwall_angle: Vec2::new(-1.0, 0.0),
            ..Default::default()
        };
        let at = Vec3::new(100.0, 50.0, 3.0);
        let mut vel = Vec3::new(10.0, -4.0, 0.0);
        let pos = coll.check_all_rebound(&mut vel, crate::wpmap::MASK_MAIN, 0.5, at, &body);
        assert_eq!(pos, Some(Vec3::new(110.0, 60.0, 3.0)));
        assert_eq!(vel, Vec3::new(-5.0, -2.0, 0.0));
        // Touched last frame too: not fresh.
        coll.mask_prev = MASK_LWALL;
        let mut vel = Vec3::new(10.0, -4.0, 0.0);
        assert!(coll
            .check_all_rebound(&mut vel, crate::wpmap::MASK_MAIN, 0.5, at, &body)
            .is_none());
        assert_eq!(vel, Vec3::new(10.0, -4.0, 0.0));
        // Fresh but moving away, or outside `check`.
        coll.mask_prev = 0;
        let mut vel = Vec3::new(-10.0, 0.0, 0.0);
        assert!(coll
            .check_all_rebound(&mut vel, crate::wpmap::MASK_MAIN, 0.5, at, &body)
            .is_none());
        let mut vel = Vec3::new(10.0, 0.0, 0.0);
        assert!(coll
            .check_all_rebound(&mut vel, MASK_CEIL, 0.5, at, &body)
            .is_none());
    }

    /// W4 in `wpMarioFireballProcMap`: a floor still touched from the last
    /// frame does not rebound the Fireball again.
    #[test]
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
    fn a_fireball_rebounds_only_off_a_fresh_floor() {
        let map = [surface(MapSurfaceKind::Floor, -2000, 0, 2000, 0)];
        for (prev, rebounds) in [(0, true), (MASK_FLOOR, false)] {
            let spawn = WeaponSpawn {
                kind: WeaponKind::MarioFireball,
                owner_port: 0,
                team: 0,
                stale: crate::stale::WeaponStale::FRESH,
                position: Vec3::new(0.0, 5.0 - MARIO_FIREBALL_MAP_COLL.bottom, 0.0),
                facing: 1.0,
            };
            let mut ball = MarioFireball::new(spawn, 0);
            ball.velocity = Vec3::new(30.0, -20.0, 0.0);
            ball.coll.mask_curr = prev;
            let mut fx = crate::wpeffect::Emit::default();
            assert!(ball.tick(|| map, &mut fx));
            assert!(ball.coll.mask_curr & MASK_FLOOR != 0);
            assert_eq!(ball.velocity.y > 0.0, rebounds);
            assert_eq!(fx.iter().any(|e| matches!(e, Fx::FireGrind(_))), rebounds);
        }
    }
}

#[cfg(test)]
mod spawn_snap_tests {
    //! Batch 17: `wpManagerMakeWeapon`'s `WEAPON_FLAG_COLLPROJECT` pass.
    use super::*;
    use crate::fighter::FighterKind;

    /// A fighter-sized parent standing at the origin.
    const PARENT: SpawnParent = SpawnParent {
        pos: Vec3::ZERO,
        coll: BodyColl {
            top: 200.0,
            center: 100.0,
            bottom: 0.0,
            width: 60.0,
        },
    };

    fn line(kind: MapSurfaceKind, line: u16, x1: i16, y1: i16, x2: i16, y2: i16) -> MapSurface {
        MapSurface {
            motion: None,
            kind,
            segment: Segment {
                x1,
                y1,
                x2,
                y2,
                flags: 0,
            },
            topology: Some(SurfaceTopology {
                line,
                point: 0,
                segments: 1,
                vertex1: 2 * line,
                vertex2: 2 * line + 1,
            }),
        }
    }

    fn request(kind: WeaponKind, x: f32, y: f32) -> WeaponSpawn {
        WeaponSpawn {
            kind,
            owner_port: 0,
            team: 0,
            position: Vec3::new(x, y, 0.0),
            facing: 1.0,
            stale: crate::stale::WeaponStale::FRESH,
        }
    }

    /// A wall 120 to the right of the parent, which its side (x 60) has not
    /// crossed.
    fn wall() -> [MapSurface; 1] {
        [line(MapSurfaceKind::LeftWall, 0, 120, 0, 120, 1000)]
    }

    /// The pass sweeps from the parent's side point to the weapon's: a
    /// fireball made past a wall the parent stands short of comes back to
    /// touch it with its own box (width 50). The no-map path leaves it.
    #[test]
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
    fn a_fireball_made_past_a_wall_snaps_back_to_it() {
        let map = wall();
        let mut pool = WeaponPool::default();
        assert!(pool.spawn_projected(
            request(WeaponKind::MarioFireball, 150.0, 100.0),
            Some(PARENT),
            &|| map,
        ));
        assert_eq!(pool.fireballs().next().unwrap().position.x, 120.0 - 50.0);
        let mut pool = WeaponPool::default();
        pool.spawn(request(WeaponKind::MarioFireball, 150.0, 100.0));
        assert_eq!(pool.fireballs().next().unwrap().position.x, 150.0);
    }

    /// The ceiling pass moves a bomb made above a ceiling the parent's
    /// top (200) is under down to hang its own top (75) from it.
    #[test]
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
    fn a_bomb_made_above_a_ceiling_snaps_under_it() {
        let map = [line(MapSurfaceKind::Ceiling, 0, -1000, 250, 1000, 250)];
        let mut pool = WeaponPool::default();
        pool.spawn_projected(
            request(WeaponKind::SamusBomb, 0.0, 300.0),
            Some(PARENT),
            &|| map,
        );
        assert_eq!(pool.bombs().next().unwrap().position.y, 250.0 - 75.0);
    }

    /// A floor the weapon's bottom crossed from the parent's puts it on the
    /// line (`mpProcessSetCollideFloor`) and records it; with no crossing
    /// the floor below is only projected (`mpProcessSetCollProjectFloorID`),
    /// and a miss clears the line id but keeps the angle.
    #[test]
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
    fn the_floor_pass_lands_a_crossing_and_projects_otherwise() {
        let map = [line(MapSurfaceKind::Floor, 3, -1000, 0, 1000, 0)];
        let above = SpawnParent {
            pos: Vec3::new(0.0, 50.0, 0.0),
            ..PARENT
        };
        let mut pool = WeaponPool::default();
        pool.spawn_projected(
            request(WeaponKind::MarioFireball, 0.0, -30.0),
            Some(above),
            &|| map,
        );
        let f = pool.fireballs().next().unwrap();
        assert_eq!(f.position.y, 50.0, "its bottom (-50) on the line");
        assert_eq!(f.coll.floor.line, Some(3));
        // Clear above the floor: only the projection.
        let mut pool = WeaponPool::default();
        pool.spawn_projected(
            request(WeaponKind::MarioFireball, 0.0, 400.0),
            Some(above),
            &|| map,
        );
        let f = pool.fireballs().next().unwrap();
        assert_eq!(f.position.y, 400.0);
        assert_eq!(f.coll.floor.line, Some(3));
        // Past the floor's end: no line, the angle as it was.
        let mut pool = WeaponPool::default();
        pool.spawn_projected(
            request(WeaponKind::MarioFireball, 1500.0, 400.0),
            Some(above),
            &|| map,
        );
        let f = pool.fireballs().next().unwrap();
        assert_eq!(f.coll.floor.line, None);
        assert_eq!(f.coll.floor_angle(), Vec2::new(0.0, 1.0));
    }

    /// `wpFoxBlasterMakeWeapon` makes its glow at its `pos` argument, not
    /// at the snapped translate.
    #[test]
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
    fn the_blaster_glow_stays_at_the_unsnapped_point() {
        let map = wall();
        let mut pool = WeaponPool::default();
        pool.spawn_projected(
            request(WeaponKind::FoxBlaster, 150.0, 100.0),
            Some(PARENT),
            &|| map,
        );
        assert_eq!(pool.blasters().next().unwrap().position.x, 120.0 - 10.0);
        let glows: Vec<_> = pool
            .fx
            .drain()
            .filter_map(|e| match e {
                Fx::FoxBlasterGlow(pos) => Some(pos),
                _ => None,
            })
            .collect();
        assert_eq!(glows, [Vec3::new(150.0, 100.0, 0.0)]);
    }

    /// `wpYoshiStarMakeStars` makes each star at its own offset point and
    /// each `wpManagerMakeWeapon` snaps on its own: only the star made past
    /// the wall moves.
    #[test]
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
    fn each_yoshi_star_snaps_from_its_own_point() {
        let map = [line(MapSurfaceKind::LeftWall, 0, 200, -1000, 200, 1000)];
        let mut pool = WeaponPool::default();
        pool.spawn_projected(
            request(WeaponKind::YoshiStars, 0.0, 0.0),
            Some(PARENT),
            &|| map,
        );
        let width = crate::monster_weapon::weapon_attributes::YOSHI_STAR
            .map_coll
            .width;
        let mut xs: Vec<f32> = pool.stars().map(|s| s.position.x).collect();
        xs.sort_by(f32::total_cmp);
        assert_eq!(xs, [-YOSHISTAR_OFF_X, 200.0 - width]);
    }

    /// The Charge Shot's ROM box is a point: the pass stops its translate
    /// on the wall itself.
    #[test]
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
    fn the_charge_shot_snaps_as_a_point() {
        let map = wall();
        let mut pool = WeaponPool::default();
        pool.spawn_projected(
            request(WeaponKind::SamusChargeShot(7), 150.0, 100.0),
            Some(PARENT),
            &|| map,
        );
        assert_eq!(pool.charge_shots().next().unwrap().position.x, 120.0);
    }

    /// `wpKirbyCutterMakeWeapon` grounds the wave on Kirby's floor only if
    /// the line lies under its unsnapped `pos`: made past a wall at the
    /// floor's end, the wave snaps back over the floor but stays airborne.
    #[test]
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
    fn the_cutter_finds_its_floor_at_the_unsnapped_point() {
        let map = [
            line(MapSurfaceKind::Floor, 0, -1000, 0, 100, 0),
            line(MapSurfaceKind::LeftWall, 1, 100, 0, 100, 1000),
        ];
        let kind = WeaponKind::KirbyCutter {
            floor_line: Some(0),
        };
        let mut pool = WeaponPool::default();
        pool.spawn_projected(request(kind, 130.0, 0.0), Some(PARENT), &|| map);
        let c = pool.cutters().next().unwrap();
        assert_eq!(c.position.x, 100.0 - KIRBY_CUTTER_MAP_COLL.width);
        assert!(!c.grounded);
        let mut pool = WeaponPool::default();
        pool.spawn_projected(request(kind, 0.0, 0.0), Some(PARENT), &|| map);
        let c = pool.cutters().next().unwrap();
        assert!(c.grounded);
        assert_eq!(c.coll.floor.line, Some(0));
    }

    /// Makers in `proc_update` or `proc_physics` see the fighter before
    /// this frame's move; the `proc_accessory` makers see it after.
    #[test]
    fn the_parent_is_read_when_the_maker_runs() {
        let mut f = crate::fighter::Fighter::new(FighterKind::Fox, 0, 3);
        f.pos = Vec3::new(10.0, 20.0, 0.0);
        f.queue_weapon_spawn(request(WeaponKind::FoxBlaster, 0.0, 0.0));
        f.pos = Vec3::new(30.0, 40.0, 0.0);
        let (_, parent) = f.take_weapon_spawn_with_parent().unwrap();
        assert_eq!(parent.pos, Vec3::new(10.0, 20.0, 0.0));
        assert_eq!(parent.coll, f.coll);
        f.pos = Vec3::new(10.0, 20.0, 0.0);
        f.queue_weapon_spawn(request(WeaponKind::MarioFireball, 0.0, 0.0));
        f.pos = Vec3::new(30.0, 40.0, 0.0);
        let (_, parent) = f.take_weapon_spawn_with_parent().unwrap();
        assert_eq!(parent.pos, Vec3::new(30.0, 40.0, 0.0));
    }
}
