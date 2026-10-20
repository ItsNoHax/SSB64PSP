//! `wpmap.c`: the weapon map callbacks over the shared sweeps of
//! [`crate::map`], with the `MPCollData` masks `wpProcessProcWeaponMain`
//! rolls over before each `proc_map` (wpprocess.c:183-188). A grounded
//! weapon takes `wpMapProcLRWallCheckFloor`, the item ground pass
//! ([`crate::map::move_ground`]); an airborne one takes
//! `wpMapProcAllCheckFloor` ([`crate::map::move_air`], landing) or
//! `wpMapProcAll` ([`crate::map::move_air_collide_floor`], carried).

use ssb_engine::math::{Vec2, Vec3};

use crate::ground::{BodyColl, Standing};
use crate::map::{AirOptions, Contacts, MASK_CEIL, MASK_FLOOR, MASK_LWALL, MASK_RWALL};
use crate::weapon::MapSurface;

/// `MAP_FLAG_MAIN_MASK`.
pub const MASK_MAIN: u16 = MASK_LWALL | MASK_RWALL | MASK_CEIL | MASK_FLOOR;

/// A weapon's `coll_data`: the contact masks and the angles of the last
/// surfaces met.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct WeaponColl {
    pub mask_prev: u16,
    pub mask_curr: u16,
    /// `floor_line_id`, `floor_flags` and `floor_angle`: the floor a
    /// grounded weapon stands on, or the last one an airborne pass met.
    pub floor: Option<Standing>,
    /// `lwall_line_id`/`rwall_line_id` of the last walls run.
    pub lwall_line: Option<u16>,
    pub rwall_line: Option<u16>,
    pub lwall_angle: Vec2,
    pub rwall_angle: Vec2,
    pub ceil_angle: Vec2,
}

impl WeaponColl {
    /// `wpProcessProcWeaponMain` before `proc_map`: `mask_prev = mask_curr`,
    /// `mask_curr = 0`.
    pub fn begin(&mut self) {
        self.mask_prev = self.mask_curr;
        self.mask_curr = 0;
    }

    /// The surfaces first touched this frame
    /// (`(mask_prev ^ mask_curr) & mask_curr & MAP_FLAG_MAIN_MASK`).
    pub fn fresh(&self) -> u16 {
        (self.mask_prev ^ self.mask_curr) & self.mask_curr & MASK_MAIN
    }

    fn note_walls(&mut self, contacts: &Contacts) {
        if let Some(hit) = contacts.left_wall {
            self.mask_curr |= MASK_LWALL;
            self.lwall_line = Some(hit.line);
            self.lwall_angle = hit.normal;
        }
        if let Some(hit) = contacts.right_wall {
            self.mask_curr |= MASK_RWALL;
            self.rwall_line = Some(hit.line);
            self.rwall_angle = hit.normal;
        }
        if let Some(hit) = contacts.ceiling {
            self.mask_curr |= MASK_CEIL;
            self.ceil_angle = hit.normal;
        }
    }

    /// `floor_angle`, up when no floor was ever met.
    pub fn floor_angle(&self) -> Vec2 {
        self.floor.map_or(Vec2::new(0.0, 1.0), |f| f.normal)
    }

    /// `wpMapTestLRWallCheckFloor` from `pos_prev` to `pos` (`vel_air`
    /// added, the line's `vel_speed` not yet: the sweep adds it on its first
    /// substep). The walls stop the weapon; leaving the line's end snaps it
    /// to the corner. `MAP_FLAG_FLOOR` marks only a change of floor line
    /// (`mpProcessCheckTestFloorCollision`, mpprocess.c:1020; RE-554).
    /// Returns whether it is still on a floor.
    pub fn test_lr_wall_check_floor<I, F>(
        &mut self,
        coll: &BodyColl,
        pos_prev: Vec3,
        pos: &mut Vec3,
        surfaces: &F,
    ) -> bool
    where
        F: Fn() -> I,
        I: IntoIterator<Item = MapSurface>,
    {
        let Some(floor) = self.floor else {
            return false;
        };
        let (moved, contacts) =
            crate::map::move_ground(coll, pos_prev, *pos, floor.line, false, surfaces);
        *pos = moved.pos;
        self.note_walls(&contacts);
        if contacts.floor {
            self.mask_curr |= MASK_FLOOR;
        }
        match moved.floor {
            Some(f) => {
                self.floor = Some(f);
                true
            }
            None => false,
        }
    }

    /// `wpMapTestAllCheckFloor`: walls and ceilings stop the weapon, a floor
    /// lands it (`mpProcessSetLandingFloor`). Returns whether it landed.
    pub fn test_all_check_floor<I, F>(
        &mut self,
        coll: &BodyColl,
        pos_prev: Vec3,
        pos: &mut Vec3,
        surfaces: &F,
    ) -> bool
    where
        F: Fn() -> I,
        I: IntoIterator<Item = MapSurface>,
    {
        let result = crate::map::move_air(coll, pos_prev, *pos, AirOptions::default(), surfaces);
        *pos = result.moved.pos;
        self.note_walls(&result.contacts);
        match result.moved.floor {
            Some(f) => {
                self.mask_curr |= MASK_FLOOR;
                self.floor = Some(f);
                true
            }
            None => false,
        }
    }

    /// `wpMapTestAll`: walls and ceilings stop the weapon; a floor carries
    /// it along its line without landing (`mpProcessSetCollideFloor`).
    pub fn test_all<I, F>(&mut self, coll: &BodyColl, pos_prev: Vec3, pos: &mut Vec3, surfaces: &F)
    where
        F: Fn() -> I,
        I: IntoIterator<Item = MapSurface>,
    {
        let (moved, contacts, floor) =
            crate::map::move_air_collide_floor(coll, pos_prev, *pos, surfaces);
        *pos = moved;
        self.note_walls(&contacts);
        if let Some(hit) = floor {
            self.mask_curr |= MASK_FLOOR;
            self.floor = Some(Standing {
                line: hit.line,
                flags: hit.flags,
                normal: hit.normal,
            });
        }
    }

    /// `wpMapTestAllCheckCollEnd`: the walls, ceiling and floor are only
    /// tested ([`crate::map::move_air_check_coll_end`]); `pos` keeps the
    /// substep that met one. Returns `is_coll_end`.
    pub fn test_all_check_coll_end<I, F>(
        &mut self,
        coll: &BodyColl,
        pos_prev: Vec3,
        pos: &mut Vec3,
        surfaces: &F,
    ) -> bool
    where
        F: Fn() -> I,
        I: IntoIterator<Item = MapSurface>,
    {
        let end = crate::map::move_air_check_coll_end(coll, pos_prev, *pos, surfaces);
        *pos = end.pos;
        self.mask_curr |= end.mask;
        if let Some(hit) = end.ceiling {
            self.ceil_angle = hit.normal;
        }
        if let Some(hit) = end.floor {
            self.floor = Some(Standing {
                line: hit.line,
                flags: hit.flags,
                normal: hit.normal,
            });
        }
        end.mask != 0
    }

    /// `wpMapCheckAllRebound`: each surface of `check` first touched this
    /// frame that `vel` moves into reflects it, in wall, ceiling, floor
    /// order; any reflection then scales it by `mod_vel`. Returns the
    /// contact point beside the struck side (`pos`) on a rebound.
    pub fn check_all_rebound(
        &self,
        vel: &mut Vec3,
        check: u16,
        mod_vel: f32,
        translate: Vec3,
        coll: &BodyColl,
    ) -> Option<Vec3> {
        let fresh = self.fresh() & check;
        let mut at = None;
        for (mask, normal, offset) in [
            (
                MASK_LWALL,
                self.lwall_angle,
                Vec2::new(coll.width, coll.center),
            ),
            (
                MASK_RWALL,
                self.rwall_angle,
                Vec2::new(-coll.width, coll.center),
            ),
            (MASK_CEIL, self.ceil_angle, Vec2::new(0.0, coll.top)),
            (MASK_FLOOR, self.floor_angle(), Vec2::new(0.0, coll.bottom)),
        ] {
            if fresh & mask != 0 && sim(*vel, normal) < 0.0 {
                reflect(vel, normal);
                at = Some(Vec3::new(
                    translate.x + offset.x,
                    translate.y + offset.y,
                    translate.z,
                ));
            }
        }
        if at.is_some() {
            scale(vel, mod_vel);
        }
        at
    }
}

/// `wpMapTestAllCheckCollEnd` for a weapon that reads only the result
/// (its `proc_map` ends it, or explodes it in place, at `translate`).
pub fn check_coll_end<I, F>(coll: &BodyColl, pos_prev: Vec3, pos: &mut Vec3, surfaces: &F) -> bool
where
    F: Fn() -> I,
    I: IntoIterator<Item = MapSurface>,
{
    WeaponColl::default().test_all_check_coll_end(coll, pos_prev, pos, surfaces)
}

/// `lbCommonSim2D`: the dot product over the sum of the magnitudes.
pub fn sim(v: Vec3, n: Vec2) -> f32 {
    let a = ssb_engine::math::sqrt(v.x * v.x + v.y * v.y);
    let b = ssb_engine::math::sqrt(n.x * n.x + n.y * n.y);
    (v.x * n.x + v.y * n.y) / (b + a)
}

/// `lbCommonReflect2D`.
pub fn reflect(v: &mut Vec3, n: Vec2) {
    let d = (n.x * v.x + n.y * v.y) * -2.0;
    v.x += n.x * d;
    v.y += n.y * d;
}

/// `lbCommonScale2D`.
pub fn scale(v: &mut Vec3, s: f32) {
    v.x *= s;
    v.y *= s;
}

/// `lbCommonMag2D`.
pub fn mag(v: Vec3) -> f32 {
    ssb_engine::math::sqrt(v.x * v.x + v.y * v.y)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collision::Segment;
    use crate::weapon::MapSurfaceKind as Kind;

    const BODY: BodyColl = BodyColl {
        top: 40.0,
        center: 20.0,
        bottom: 0.0,
        width: 20.0,
    };

    fn line(kind: Kind, x1: i16, y1: i16, x2: i16, y2: i16) -> MapSurface {
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

    /// `mpProcessUpdateMain`: a 600 step runs as three 200 substeps, and
    /// `is_coll_end` stops it after the one that met a floor.
    #[test]
    fn coll_end_keeps_the_substep_that_met_the_floor() {
        let floors = [
            line(Kind::Floor, -100, -300, 100, -300),
            line(Kind::Floor, -100, -550, 100, -550),
        ];
        let mut coll = WeaponColl::default();
        let mut pos = Vec3::new(0.0, -600.0, 0.0);
        assert!(coll.test_all_check_coll_end(&BODY, Vec3::ZERO, &mut pos, &|| floors));
        assert_eq!(pos, Vec3::new(0.0, -400.0, 0.0));
        assert_eq!(coll.mask_curr, MASK_FLOOR);
        // Nothing met: the whole step.
        let mut pos = Vec3::new(0.0, -200.0, 0.0);
        assert!(!check_coll_end(&BODY, Vec3::ZERO, &mut pos, &|| floors));
        assert_eq!(pos.y, -200.0);
    }

    /// `wpMapProcAllCheckCollEnd` tests the walls and the ceiling as well,
    /// and moves nothing back out of them.
    #[test]
    fn coll_end_meets_walls_and_ceilings_where_the_step_ends() {
        for (s, step, mask) in [
            (
                line(Kind::LeftWall, 100, -200, 100, 200),
                Vec3::new(150.0, 0.0, 0.0),
                MASK_LWALL,
            ),
            (
                line(Kind::RightWall, -100, 200, -100, -200),
                Vec3::new(-150.0, 0.0, 0.0),
                MASK_RWALL,
            ),
            (
                line(Kind::Ceiling, -200, 100, 200, 100),
                Vec3::new(0.0, 150.0, 0.0),
                MASK_CEIL,
            ),
        ] {
            let mut coll = WeaponColl::default();
            let mut pos = step;
            assert!(
                coll.test_all_check_coll_end(&BODY, Vec3::ZERO, &mut pos, &|| [s]),
                "{:?}",
                s.kind
            );
            assert_eq!(coll.mask_curr, mask, "{:?}", s.kind);
            assert_eq!(pos, step, "{:?}", s.kind);
        }
    }
}
