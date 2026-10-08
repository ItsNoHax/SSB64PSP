//! Sector Z's Arwing lasers (`grsector.c`): `nWPKindArwingLaser2D`, the
//! pair shot along the stage plane, and `nWPKindArwingLaser3D`, the single
//! shot from the background that bursts where it lands.
//!
//! Both are ground weapons (`WEAPON_FLAG_PARENT_GROUND`): no owner, team
//! `WEAPON_TEAM_DEFAULT`, player `WEAPON_PORT_DEFAULT`, the default handicap
//! and staling. Their `WPAttributes` are `GRSectorMap`'s (file 262 + 0xBC
//! and + 0xF0). The Arwing's controller makes them
//! ([`crate::stage::sector`]); the pool moves and hits them.

use ssb_engine::math::{sin_cos, Vec3};

use super::{map_contact, MapSurface};
use crate::attack::Hitbox;
use crate::ground::BodyColl;
use crate::wpeffect::{Emit, WeaponEffect as Fx};

/// `WEAPON_PORT_DEFAULT` (`GMCOMMON_PLAYERS_MAX`): the player of a weapon
/// no fighter made.
pub const GROUND_PORT: u8 = 4;
/// `WEAPON_TEAM_DEFAULT`.
pub const GROUND_TEAM: u8 = 4;
/// Both makers' speed.
pub const LASER_SPEED: f32 = 230.0;
/// `grSectorArwingWeaponLaserExplodeInitVars`.
pub const EXPLODE_LIFETIME: u16 = 16;
pub const EXPLODE_RADIUS: f32 = 200.0;

/// `llGRSectorMapArwingLaser2DWeaponAttributes`; `wpManagerMakeWeapon`
/// halves the size of 300.
pub const LASER_2D_HITBOX: Hitbox = Hitbox {
    damage: 16,
    offset: Vec3::ZERO,
    radius: 150.0,
    angle: 361,
    kb_scale: 75,
    kb_weight: 0,
    kb_base: 70,
    element: crate::combat::Element::Normal,
    shield_damage: 5,
};
pub const LASER_2D_MAP_COLL: BodyColl = BodyColl {
    top: 28.0,
    center: 0.0,
    bottom: -28.0,
    width: 564.0,
};

/// `llGRSectorMapArwingLaser3DWeaponAttributes`.
pub const LASER_3D_HITBOX: Hitbox = Hitbox {
    damage: 18,
    offset: Vec3::ZERO,
    radius: 150.0,
    angle: 361,
    kb_scale: 100,
    kb_weight: 0,
    kb_base: 10,
    element: crate::combat::Element::Fire,
    shield_damage: 2,
};
pub const LASER_3D_MAP_COLL: BodyColl = BodyColl {
    top: 28.0,
    center: 0.0,
    bottom: -28.0,
    width: 28.0,
};

/// `grSectorArwingWeaponLaser3DProcMap` tests the map only this close to
/// the stage plane.
const LASER_3D_MAP_DEPTH: f32 = 1000.0;

/// One Arwing laser.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ArwingLaser {
    /// `nWPKindArwingLaser3D` rather than `...2D`.
    pub three_d: bool,
    /// `wp->player`: [`GROUND_PORT`] until a reflector takes it.
    pub owner_port: u8,
    pub damage: i32,
    pub position: Vec3,
    pub velocity: Vec3,
    /// `DObjGetStruct(weapon_gobj)->rotate`.
    pub rotate: Vec3,
    /// `grSectorArwingWeaponLaserExplodeInitVars` ran: the 3D shot is a
    /// burst with no display list.
    pub exploded: bool,
    pub lifetime: u16,
    /// The Arwing's roll (`map_dobjs[1]->rotate.z`) as the pool last saw
    /// it: a hop or a reflection refaces the shot by it.
    pub roll: f32,
}

impl ArwingLaser {
    /// A shot the controller made at `position` with `velocity`, faced by
    /// `rotate` (`func_ovl2_8010719C` at the Arwing's roll `roll`).
    pub fn new(three_d: bool, position: Vec3, velocity: Vec3, rotate: Vec3, roll: f32) -> Self {
        ArwingLaser {
            three_d,
            owner_port: GROUND_PORT,
            damage: if three_d {
                LASER_3D_HITBOX.damage
            } else {
                LASER_2D_HITBOX.damage
            },
            position,
            velocity,
            rotate,
            exploded: false,
            lifetime: 0,
            roll,
        }
    }

    pub fn hitbox(&self) -> Hitbox {
        let base = if self.three_d {
            LASER_3D_HITBOX
        } else {
            LASER_2D_HITBOX
        };
        Hitbox {
            damage: self.damage,
            radius: if self.exploded {
                EXPLODE_RADIUS
            } else {
                base.radius
            },
            ..base
        }
    }

    /// `attack_coll.can_shield`: the 3D shot and its burst pass shields.
    pub fn can_shield(&self) -> bool {
        !self.three_d
    }

    /// `grSectorArwingWeaponLaserExplodeInitVars`. The caller clears the
    /// attack record (`wpMainClearAttackRecord`).
    pub fn explode(&mut self) {
        self.lifetime = EXPLODE_LIFETIME;
        self.velocity = Vec3::ZERO;
        self.exploded = true;
    }

    /// One `wpProcessProcWeaponMain`: the burst's `proc_update`, the move,
    /// then `proc_map`. Returns whether the shot lives on and whether it
    /// burst this frame.
    pub(super) fn tick<I, F>(&mut self, surfaces: F, fx: &mut Emit) -> (bool, bool)
    where
        F: Fn() -> I,
        I: IntoIterator<Item = MapSurface>,
    {
        if self.exploded {
            // `grSectorArwingWeaponLaserExplodeProcUpdate`:
            // `wpMainDecLifeCheckExpire`.
            self.lifetime = self.lifetime.saturating_sub(1);
            return (self.lifetime != 0, false);
        }
        let wanted = self.position + self.velocity;
        if !self.three_d {
            // `grSectorArwingWeaponLaser2DProcMap`.
            if let Some(hit) = map_contact(surfaces(), self.position, wanted, LASER_2D_MAP_COLL) {
                fx.push(Fx::DustExpandSmall(hit.position));
                return (false, false);
            }
            self.position = wanted;
            return (true, false);
        }
        // `grSectorArwingWeaponLaser3DProcMap`.
        if wanted.z.abs() < LASER_3D_MAP_DEPTH {
            if let Some(hit) = map_contact(surfaces(), self.position, wanted, LASER_3D_MAP_COLL) {
                self.position = hit.position;
                crate::sound::play_fgm(crate::sound::id::nSYAudioFGMExplodeS);
                fx.push(Fx::SparkleWhiteMultiExplode(self.position));
                self.explode();
                return (true, true);
            }
        }
        self.position = wanted;
        (true, false)
    }

    /// The 2D shot's `proc_hop` (`syVectorRotateAbout3D` already turned
    /// the velocity) and `proc_reflector`: refaced along its new path.
    pub(super) fn reface(&mut self) {
        let dir = self.velocity.normalized();
        self.rotate = laser_rotate(dir, self.roll);
    }
}

/// `syUtilsArcSin`.
fn arc_sin(x: f32) -> f32 {
    if x > 0.99999 {
        core::f32::consts::FRAC_PI_2
    } else if x < -0.99999 {
        -core::f32::consts::FRAC_PI_2
    } else {
        ssb_engine::math::atan2(x, ssb_engine::math::sqrt(1.0 - x * x))
    }
}

/// `func_ovl2_801070A4`: the Euler angles of the frame whose X axis is
/// `side` and whose Y axis is `up`, `dir` being the direction of travel.
fn frame_rotate(dir: Vec3, side: Vec3, up: Vec3) -> Vec3 {
    use ssb_engine::math::atan2;
    if side.z == -1.0 || side.z == 1.0 {
        if side.z == -1.0 {
            Vec3::new(atan2(up.x, up.y), core::f32::consts::FRAC_PI_2, 0.0)
        } else {
            Vec3::new(atan2(-up.x, up.y), -core::f32::consts::FRAC_PI_2, 0.0)
        }
    } else {
        Vec3::new(atan2(up.z, dir.z), arc_sin(-side.z), atan2(side.y, side.x))
    }
}

/// `func_ovl2_8010719C`: the rotation of a shot travelling along unit
/// `dir`, rolled with the Arwing (`roll` is `map_dobjs[1]->rotate.z`).
pub fn laser_rotate(dir: Vec3, roll: f32) -> Vec3 {
    let (s, c) = sin_cos(roll + core::f32::consts::FRAC_PI_2);
    let up = Vec3::new(0.0, s, c);
    let side = up.cross(dir);
    let up = dir.cross(side);
    frame_rotate(dir, side.normalized(), up.normalized())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_level_shot_leftward_faces_along_minus_x() {
        // The 2D pair: velocity (-230, 0, 0), the Arwing unrolled.
        let r = laser_rotate(Vec3::new(-1.0, 0.0, 0.0), 0.0);
        // up = (0, 1, 0) at roll 0 (sin 90, cos 90 ~ 0); side = up x dir =
        // (0, 0, 1), so the degenerate branch's `side.z == 1` case.
        assert!((r.y + core::f32::consts::FRAC_PI_2).abs() < 1e-4, "{r:?}");
        assert!(r.z.abs() < 1e-6);
    }

    #[test]
    fn the_burst_stops_and_counts_down() {
        let mut l = ArwingLaser::new(
            true,
            Vec3::ZERO,
            Vec3::new(0.0, -100.0, 230.0),
            Vec3::ZERO,
            0.0,
        );
        l.explode();
        assert_eq!(l.velocity, Vec3::ZERO);
        assert_eq!(l.hitbox().radius, EXPLODE_RADIUS);
        let mut fx = Emit::default();
        let mut lived = 0;
        while l.tick(core::iter::empty, &mut fx).0 {
            lived += 1;
        }
        assert_eq!(lived, usize::from(EXPLODE_LIFETIME) - 1);
    }
}
