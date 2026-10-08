//! Sector Z: the Arwing (`grsector.c`).
//!
//! An Arwing sleeps until the battle starts, waits ten seconds, then flies
//! one of eight patterns: five pass along the stage plane (`laser_count`
//! 2) with pilot manoeuvres and the 2D laser pair, three sweep in from the
//! background (`laser_count` 0) and may fire the 3D shot once. While it is
//! near the stage plane its wing is collision group 1, which fighters can
//! stand on. Between patterns it hides for 16 to 35 seconds; every fourth
//! pattern is a background one.
//!
//! The Arwing's twelve `DObj`s are the runtime's ([`ArwingObject`]); its
//! lasers are the weapon pool's ([`crate::weapon::ArwingLaser`]), queued
//! here and made by the match after the stage ticks
//! ([`Sector::take_lasers`]). The Arwing plays its sounds where the source
//! does.

use super::MapQuery;
use crate::fighter::Fighter;
use crate::map::{GroupStatus, MapGroup};
use crate::rng;
use crate::weapon::sector::{laser_rotate, LASER_SPEED};
use crate::weapon::{ArwingLaser, MapSurface};
use ssb_engine::math::{sin_cos, Vec3};

/// The wing's collision group (`mpCollisionSetYakumono*ID(1)`).
pub const WING_GROUP: u8 = 1;

/// `dGRSectorArwingMapPositionsX`: where a fifth of the plane patterns
/// pass instead of the centre.
pub const MAP_POSITIONS_X: [i16; 3] = [-3000, 0, 9000];
/// `dGRSectorArwingLaserCounts`: 2 for the plane patterns, 0 for the
/// background ones.
pub const LASER_COUNTS: [u8; 8] = [2, 2, 2, 2, 2, 0, 0, 0];
/// `dGRSectorArwingPilotIDs`.
pub const PILOT_IDS: [u8; 56] = [
    1, 1, 2, 3, 4, 4, 5, 0, 0, 0, 0, 2, 3, 4, 4, 5, 0, 0, 0, 0, 1, 1, 3, 4, 4, 5, 0, 0, 0, 0, 1, 1,
    2, 4, 4, 5, 0, 0, 0, 0, 1, 1, 2, 3, 5, 0, 0, 0, 0, 1, 1, 2, 3, 4, 4, 0,
];
/// `dGRSectorArwingPilotWaitTimers`: each previous pilot's
/// `(first, count)` window into [`PILOT_IDS`].
pub const PILOT_WINDOWS: [(u8, u8); 6] = [(0, 7), (7, 9), (16, 10), (26, 10), (36, 9), (45, 10)];

/// `DOBJ_FLAG_HIDDEN`.
pub const DOBJ_HIDDEN: u16 = 1 << 1;

/// The `DObj`s the controller names (`map_dobjs[i]`).
pub mod node {
    /// The flight path; drawn by the custom matrix.
    pub const PATH: u8 = 0;
    /// The pilot's roll and dip.
    pub const PILOT: u8 = 1;
    /// The left and right gun muzzles (firing).
    pub const GUN_L: u8 = 2;
    pub const GUN_R: u8 = 3;
    /// The muzzles charging.
    pub const CHARGE_L: u8 = 4;
    pub const CHARGE_R: u8 = 5;
    /// The two hulls the patterns and pilot 5 swap, and the engine flare.
    pub const HULL: u8 = 7;
    pub const FLARE: u8 = 8;
    pub const HULL_ALT: u8 = 9;
    /// The engine glow.
    pub const GLOW: u8 = 10;
    /// The background patterns' up-vector path.
    pub const UP_PATH: u8 = 11;
}

/// A script the controller starts on a node.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArwingAnim {
    /// `GRSectorDesc` field `field` (0x0, 0x1C, 0x24, 0x2C) of pattern
    /// `pattern`.
    Flight { pattern: u8, field: u8 },
    /// `dGRSectorArwingAnimJoints[id]`.
    Pilot(u8),
    /// `llFoxSpecial3_1B84_AnimJoint`.
    LaserCharge,
    /// `llFoxSpecial3_1B34_AnimJoint`.
    LaserFire,
    /// `llFoxSpecial3_2EB4_AnimJoint`.
    Flare,
    /// `llFoxSpecial3_2E74_AnimJoint`.
    Glow,
}

/// The runtime half of the Arwing: its `DObj`s' clocks, poses and flags.
pub trait ArwingObject {
    /// `grSectorArwingAddAnim`: the script (or, for `None`, `anim_wait =
    /// AOBJ_ANIM_NULL`), parsed and played on the node at once.
    fn add_anim(&mut self, node: u8, anim: Option<ArwingAnim>);
    /// `gcAddDObjAnimJoint` alone: the next [`Self::play_all`] parses it.
    fn add_anim_joint(&mut self, node: u8, anim: ArwingAnim);
    /// `gcPlayAnimAll`.
    fn play_all(&mut self);
    /// `anim_wait == AOBJ_ANIM_NULL`.
    fn anim_null(&self, node: u8) -> bool;
    /// `anim_wait = AOBJ_ANIM_NULL`.
    fn stop(&mut self, node: u8);
    fn flags(&self, node: u8) -> u16;
    fn set_flags(&mut self, node: u8, flags: u16);
    /// `map_gobj->flags`: `GOBJ_FLAG_HIDDEN` or none.
    fn set_hidden(&mut self, hidden: bool);
    fn translate(&self, node: u8) -> Vec3;
    fn set_translate(&mut self, node: u8, t: Vec3);
    fn rotate(&self, node: u8) -> Vec3;
    fn set_rotate(&mut self, node: u8, r: Vec3);
    /// The node's `TraI` value (`gcGetAObjValue`), or `None` while it has
    /// no key.
    fn path_fraction(&self, node: u8) -> Option<f32>;
    /// `syInterpQuad` on the node's path.
    fn path_tangent(&self, node: u8, t: f32) -> Option<Vec3>;
    /// `syInterpCubic` on the node's path.
    fn path_point(&self, node: u8, t: f32) -> Option<Vec3>;
    /// Node 0's matrix for the next draw ([`Sector::root_matrix`]).
    fn set_root(&mut self, m: [[f32; 4]; 4]);
}

/// `grSectorArwingStatus`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArwingStatus {
    Sleep,
    Wait,
    Patrol,
}

/// The lasers one frame can make: the 2D pair or one 3D shot.
const LASER_QUEUE: usize = 2;

/// `GRCommonGroundVarsSector`.
#[derive(Debug, Clone, PartialEq)]
pub struct Sector {
    pub target_x: f32,
    pub appear_timer: u16,
    pub state_timer: u16,
    pub status: ArwingStatus,
    pub flight_pattern: i8,
    pub type_cycle: u8,
    pub laser_ammo: u8,
    /// `unk_sector_0x4C`: 0 or 1 lets background pattern 5 or 6 fire.
    pub fire_pattern: i8,
    /// `unk_sector_0x4E`: frames until the next volley check.
    pub volley_wait: u16,
    pub laser_timer: u16,
    /// `unk_sector_0x52`: the muzzles have charged for this volley.
    pub charged: u8,
    pub pilot_curr: i8,
    pub pilot_prev: u8,
    pub laser_count: u8,
    pub is_z_near: bool,
    pub is_z_collision: bool,
    pub is_line_active: bool,
    pub is_line_collision: bool,
    lasers: [Option<ArwingLaser>; LASER_QUEUE],
}

/// The basis `func_ovl2_80106730` derives: forward along the path, side
/// and up.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Basis {
    pub forward: Vec3,
    pub side: Vec3,
    pub up: Vec3,
}

fn rotate_z(v: Vec3, angle: f32) -> Vec3 {
    let (s, c) = sin_cos(angle);
    Vec3::new(v.x * c - v.y * s, v.x * s + v.y * c, v.z)
}

impl Sector {
    /// `grSectorInitAll` @ 0x80107E7C, after `grModelSetupGroundDObjs`.
    pub fn new(groups: &mut [MapGroup], arwing: &mut dyn ArwingObject) -> Self {
        arwing.set_hidden(true);
        arwing.add_anim_joint(node::GLOW, ArwingAnim::Glow);
        arwing.play_all();
        set_group(groups, GroupStatus::Off);
        let s = Sector {
            target_x: 0.0,
            appear_timer: 600,
            state_timer: 0,
            status: ArwingStatus::Sleep,
            flight_pattern: -1,
            type_cycle: 3,
            laser_ammo: 0,
            fire_pattern: 0,
            volley_wait: 0,
            laser_timer: 0,
            charged: 0,
            pilot_curr: -1,
            pilot_prev: 0,
            laser_count: 0,
            is_z_near: false,
            is_z_collision: false,
            is_line_active: false,
            is_line_collision: false,
            lasers: [None; LASER_QUEUE],
        };
        arwing.set_root(s.root_matrix(arwing));
        s
    }

    /// The lasers this frame's tick made, in make order.
    pub fn take_lasers(&mut self) -> impl Iterator<Item = ArwingLaser> {
        core::mem::replace(&mut self.lasers, [None; LASER_QUEUE])
            .into_iter()
            .flatten()
    }

    /// `grSectorProcUpdate` @ 0x80107E08.
    pub fn tick<I, F>(
        &mut self,
        fighters: &[&mut Fighter],
        groups: &mut [MapGroup],
        arwing: &mut dyn ArwingObject,
        map: &MapQuery<'_, F>,
        started: bool,
    ) where
        F: Fn() -> I,
        I: IntoIterator<Item = MapSurface>,
    {
        match self.status {
            // `grSectorArwingUpdateSleep`.
            ArwingStatus::Sleep => {
                if started {
                    self.status = ArwingStatus::Wait;
                }
            }
            ArwingStatus::Wait => self.update_wait(arwing),
            ArwingStatus::Patrol => self.update_patrol(fighters, groups, arwing, map),
        }
        self.start_pattern(arwing);
        // `grSectorArwingLaser3DFuncMatrix` runs as the ship draws, after
        // every process of the frame; nothing later moves node 0.
        arwing.set_root(self.root_matrix(arwing));
    }

    /// `grSectorArwingUpdateWait` @ 0x80106AC0.
    fn update_wait(&mut self, arwing: &mut dyn ArwingObject) {
        if self.appear_timer != 0 {
            self.appear_timer -= 1;
            return;
        }
        self.target_x = 0.0;
        let pattern;
        if self.type_cycle != 0 {
            pattern = rng::rand_int_range(5);
            if pattern == 4 {
                let i = rng::rand_int_range(MAP_POSITIONS_X.len() as i32) as usize;
                self.target_x = f32::from(MAP_POSITIONS_X[i]);
            }
            self.type_cycle -= 1;
            self.state_timer = (rng::rand_int_range(540) + 180) as u16;
            self.pilot_curr = -1;
        } else {
            pattern = rng::rand_int_range(3) + 5;
            self.fire_pattern = if !rng::rand_ushort().is_multiple_of(2) {
                (pattern - 5) as i8
            } else {
                -1
            };
            self.type_cycle = 3;
            self.pilot_curr = -2;
        }
        self.flight_pattern = pattern as i8;
        self.status = ArwingStatus::Patrol;
        self.volley_wait = 60;
        arwing.set_translate(node::PILOT, Vec3::ZERO);
        arwing.set_rotate(node::PILOT, Vec3::ZERO);
        self.is_line_collision = false;
        self.is_line_active = true;
        self.is_z_collision = false;
        self.laser_ammo = 0;
        crate::sound::play_fgm(crate::sound::id::nSYAudioFGMSectorAmbient1);
    }

    /// `func_ovl2_80106C88` and `func_ovl2_80106CC4`: pilots 1 and 4 lift
    /// the wing off the plane while they roll.
    fn toggle_line(&mut self, back_on: u16) {
        if self.state_timer == 0 {
            self.is_line_active = false;
        } else if self.state_timer == back_on {
            self.is_line_active = true;
        }
    }

    /// `func_ovl2_80106D00`: pilot 5 swaps the hulls behind the flare and
    /// ends the pattern with its manoeuvre.
    fn pilot_swap(&mut self, arwing: &mut dyn ArwingObject) {
        if self.state_timer == 0 {
            arwing.stop(node::HULL);
            arwing.set_flags(node::HULL, 0);
            arwing.stop(node::HULL_ALT);
            arwing.set_flags(node::HULL_ALT, DOBJ_HIDDEN);
            arwing.add_anim(node::FLARE, Some(ArwingAnim::Flare));
        } else if arwing.anim_null(node::FLARE) {
            arwing.set_flags(node::HULL, DOBJ_HIDDEN);
            arwing.set_flags(node::HULL_ALT, 0);
        }
        if arwing.anim_null(node::PILOT) {
            arwing.stop(node::PATH);
        }
    }

    /// `func_ovl2_80106DD8`: the pilot's manoeuvres on the plane patterns.
    fn update_pilot(&mut self, arwing: &mut dyn ArwingObject) {
        if self.pilot_curr == -2 {
            return;
        }
        if self.pilot_curr >= 0 {
            match self.pilot_curr {
                1 => self.toggle_line(88),
                4 => self.toggle_line(178),
                5 => self.pilot_swap(arwing),
                _ => {}
            }
            if arwing.anim_null(node::PILOT) {
                self.pilot_curr = -1;
                self.state_timer = 120;
            } else {
                self.state_timer = self.state_timer.wrapping_add(1);
            }
        } else {
            self.state_timer = self.state_timer.wrapping_sub(1);
            if self.state_timer == 0 {
                let (first, count) = PILOT_WINDOWS[usize::from(self.pilot_prev)];
                let pick = usize::from(first) + rng::rand_int_range(i32::from(count)) as usize;
                let pilot = PILOT_IDS[pick];
                if pilot != 0 {
                    arwing.add_anim(node::PILOT, Some(ArwingAnim::Pilot(pilot)));
                }
                self.pilot_prev = pilot;
                self.pilot_curr = pilot as i8;
            }
        }
    }

    /// `grSectorArwingPrepareLaserCount`: `syUtilsRandIntRange(3)` is never
    /// 3, so every volley is four shots. Kept as is.
    fn prepare_laser_count() -> u8 {
        if rng::rand_int_range(3) >= 3 {
            2
        } else {
            4
        }
    }

    /// The Arwing's position on the stage plane: the path plus
    /// `arwing_target_x`, raised by the pilot's dip.
    fn plane_pos(&self, arwing: &dyn ArwingObject) -> (f32, f32) {
        let path = arwing.translate(node::PATH);
        (
            path.x + self.target_x,
            path.y + arwing.translate(node::PILOT).y,
        )
    }

    /// `grSectorArwingGetLaserAmmoCount` @ 0x80106F5C: a volley when a
    /// fighter is ahead of a plane pattern and level with it.
    fn ammo_for(&self, fighters: &[&mut Fighter], arwing: &dyn ArwingObject) -> u8 {
        let (x, y) = self.plane_pos(arwing);
        for f in fighters {
            if self.laser_count == 2 {
                let top = super::top_n(f);
                if top.x < x && top.y < y + 300.0 && top.y > y + -500.0 {
                    return Self::prepare_laser_count();
                }
            }
        }
        0
    }

    /// `func_ovl2_80107958`: when to charge and fire.
    fn update_lasers<I, F>(
        &mut self,
        fighters: &[&mut Fighter],
        arwing: &mut dyn ArwingObject,
        map: &MapQuery<'_, F>,
    ) where
        F: Fn() -> I,
        I: IntoIterator<Item = MapSurface>,
    {
        if self.laser_ammo == 0 {
            let mut ammo = 0;
            if self.pilot_curr == -2 {
                if (self.fire_pattern == 0 || self.fire_pattern == 1) && self.appear_timer == 5 {
                    ammo = Self::prepare_laser_count();
                }
            } else if !self.is_z_near {
                self.volley_wait = 60;
                self.laser_ammo = 0;
            } else {
                self.volley_wait = self.volley_wait.wrapping_sub(1);
                if self.volley_wait == 0 {
                    ammo = self.ammo_for(fighters, arwing);
                    self.volley_wait = 60;
                }
            }
            if ammo != 0 {
                self.laser_ammo = ammo;
                self.laser_timer = 0;
                self.charged = 0;
            }
            return;
        }
        if self.laser_timer == 0 {
            if self.charged == 0 {
                arwing.add_anim(node::CHARGE_L, Some(ArwingAnim::LaserCharge));
                arwing.add_anim(node::CHARGE_R, Some(ArwingAnim::LaserCharge));
                self.charged += 1;
            } else if arwing.anim_null(node::CHARGE_L) {
                // `func_ovl2_80107910`.
                if self.laser_count == 2 {
                    self.make_laser_2d(arwing);
                } else {
                    self.make_laser_3d(fighters, arwing, map);
                }
                crate::sound::play_fgm(crate::sound::id::nSYAudioFGMSectorArwingLaser);
                arwing.add_anim(node::GUN_L, Some(ArwingAnim::LaserFire));
                arwing.add_anim(node::GUN_R, Some(ArwingAnim::LaserFire));
                self.laser_timer = 30;
                self.laser_ammo -= 1;
            }
        }
        if self.laser_timer != 0 {
            self.laser_timer -= 1;
        }
        if self.laser_ammo == 0 {
            self.volley_wait = 240;
        }
    }

    fn queue(&mut self, laser: ArwingLaser) {
        if let Some(slot) = self.lasers.iter_mut().find(|s| s.is_none()) {
            *slot = Some(laser);
        }
    }

    /// `grSectorArwingWeaponLaser2DMakeWeapon` @ 0x80107330: one shot from
    /// each gun, level and leftward. The guns' model X and Z swap into the
    /// stage's Z and X: the ship flies toward -X.
    fn make_laser_2d(&mut self, arwing: &dyn ArwingObject) {
        let (x, y) = self.plane_pos(arwing);
        let roll = arwing.rotate(node::PILOT).z;
        let rotate = laser_rotate(Vec3::new(-1.0, 0.0, 0.0), roll);
        for gun in [node::GUN_L, node::GUN_R] {
            let g = rotate_z(arwing.translate(gun), roll);
            let pos = Vec3::new((x - g.z) - 566.0, y + g.y, 0.0 + g.x);
            let vel = Vec3::new(-LASER_SPEED, 0.0, 0.0);
            self.queue(ArwingLaser::new(false, pos, vel, rotate, roll));
        }
    }

    /// `func_ovl2_80106730`: the frame along node 0's path. `up` is the
    /// caller's until the background patterns' up path takes over; the
    /// source's laser maker passes an uninitialised one, which only the
    /// up path ever writes, so the maker passes +Y as the matrix does.
    pub fn basis(&self, arwing: &dyn ArwingObject, forward: Vec3, up: Vec3) -> Basis {
        let mut forward = forward;
        let mut up = up;
        let t = arwing.path_fraction(node::PATH).map(|v| v.clamp(0.0, 1.0));
        if let Some(t) = t {
            if let Some(d) = arwing.path_tangent(node::PATH, t) {
                forward = d;
            }
        }
        if !arwing.anim_null(node::UP_PATH) && self.laser_count == 0 {
            // The up path is read at node 0's fraction, not its own.
            if let (Some(t), Some(_)) = (t, arwing.path_fraction(node::UP_PATH)) {
                if let Some(p) = arwing.path_point(node::UP_PATH, t) {
                    up = p.normalized();
                }
            }
        }
        let side = up.cross(forward);
        let up = forward.cross(side);
        Basis {
            forward: forward.normalized(),
            side: side.normalized(),
            up: up.normalized(),
        }
    }

    /// `grSectorArwingLaser3DFuncMatrix` @ 0x80106904: node 0's matrix in
    /// the N64 `Mtx44f` layout (row `i` is the image of axis `i`, row 3
    /// the translation). The plane patterns fly level toward -X.
    pub fn root_matrix(&self, arwing: &dyn ArwingObject) -> [[f32; 4]; 4] {
        let b = if self.laser_count == 2 {
            Basis {
                forward: Vec3::new(-1.0, 0.0, 0.0),
                side: Vec3::new(0.0, 0.0, 1.0),
                up: Vec3::new(0.0, 1.0, 0.0),
            }
        } else {
            self.basis(arwing, Vec3::new(-1.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0))
        };
        let t = arwing.translate(node::PATH);
        [
            [b.side.x, b.side.y, b.side.z, 0.0],
            [b.up.x, b.up.y, b.up.z, 0.0],
            [b.forward.x, b.forward.y, b.forward.z, 0.0],
            [t.x + self.target_x, t.y, t.z, 1.0],
        ]
    }

    /// `grSectorArwingWeaponLaser3DMakeWeapon` @ 0x801076E8: one shot from
    /// 666 units ahead of the ship at a random fighter's feet.
    fn make_laser_3d<I, F>(
        &mut self,
        fighters: &[&mut Fighter],
        arwing: &dyn ArwingObject,
        map: &MapQuery<'_, F>,
    ) where
        F: Fn() -> I,
        I: IntoIterator<Item = MapSurface>,
    {
        let m = self.root_matrix_3d(arwing);
        let origin = Vec3::new(
            666.0 * m.forward.x + m.origin.x,
            666.0 * m.forward.y + m.origin.y,
            666.0 * m.forward.z + m.origin.z,
        );
        let count = fighters.len() as i32;
        let pick = rng::rand_int_range(count) as usize;
        let Some(f) = fighters.get(pick) else {
            return;
        };
        let target = feet(f, map).unwrap_or(Vec3::ZERO);
        let dir = (target - origin).normalized();
        let roll = arwing.rotate(node::PILOT).z;
        let rotate = laser_rotate(dir, roll);
        self.queue(ArwingLaser::new(
            true,
            origin,
            dir * LASER_SPEED,
            rotate,
            roll,
        ));
    }

    /// The 3D maker's matrix: always the path frame, whatever
    /// `laser_count` says.
    fn root_matrix_3d(&self, arwing: &dyn ArwingObject) -> Frame {
        let b = self.basis(arwing, Vec3::new(-1.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0));
        let t = arwing.translate(node::PATH);
        Frame {
            forward: b.forward,
            origin: Vec3::new(t.x + self.target_x, t.y, t.z),
        }
    }

    /// `func_ovl2_80107B30`: the flare replays while the first hull shows.
    fn update_flare(&mut self, arwing: &mut dyn ArwingObject) {
        if arwing.anim_null(node::FLARE) && arwing.flags(node::HULL) == 0 {
            arwing.add_anim(node::FLARE, Some(ArwingAnim::Flare));
            crate::sound::play_fgm(crate::sound::id::nSYAudioFGMSectorAmbient2);
        }
    }

    /// `grSectorArwingUpdateCollisions` @ 0x80107BA0.
    fn update_collisions(&mut self, groups: &mut [MapGroup], arwing: &dyn ArwingObject) {
        if self.pilot_curr == -2 {
            return;
        }
        if self.is_line_active && self.is_z_near {
            let (x, y) = self.plane_pos(arwing);
            let pos = Vec3::new(x, y, 0.0);
            if let Some(g) = groups.get_mut(usize::from(WING_GROUP)) {
                // Turning on sets the position twice, so the wing's first
                // frame has no speed.
                if !self.is_z_collision || !self.is_line_collision {
                    g.status = GroupStatus::On;
                    g.set_position(pos);
                }
                g.set_position(pos);
            }
        }
        if (!self.is_line_active || !self.is_z_near)
            && self.is_z_collision
            && self.is_line_collision
        {
            set_group(groups, GroupStatus::Off);
        }
        self.is_line_collision = self.is_line_active;
        self.is_z_collision = self.is_z_near;
    }

    /// `grSectorArwingUpdatePatrol` @ 0x80107CA0.
    fn update_patrol<I, F>(
        &mut self,
        fighters: &[&mut Fighter],
        groups: &mut [MapGroup],
        arwing: &mut dyn ArwingObject,
        map: &MapQuery<'_, F>,
    ) where
        F: Fn() -> I,
        I: IntoIterator<Item = MapSurface>,
    {
        // `grSectorArwingDecideZNear`.
        self.is_z_near = arwing.translate(node::PATH).z.abs() < 200.0;
        self.update_pilot(arwing);
        self.update_lasers(fighters, arwing, map);
        self.update_flare(arwing);
        self.update_collisions(groups, arwing);
        if arwing.anim_null(node::PATH) {
            arwing.set_hidden(true);
            self.appear_timer = (rng::rand_int_range(1140) + 960) as u16;
            self.status = ArwingStatus::Wait;
            set_group(groups, GroupStatus::Off);
        } else {
            self.appear_timer = self.appear_timer.wrapping_add(1);
        }
    }

    /// `func_ovl2_80107D50`: the chosen pattern's four scripts.
    fn start_pattern(&mut self, arwing: &mut dyn ArwingObject) {
        if self.flight_pattern == -1 {
            return;
        }
        let pattern = self.flight_pattern as u8;
        self.laser_count = LASER_COUNTS[usize::from(pattern)];
        for (field, n) in [
            (0, node::PATH),
            (1, node::HULL),
            (2, node::HULL_ALT),
            (3, node::UP_PATH),
        ] {
            arwing.add_anim(n, Some(ArwingAnim::Flight { pattern, field }));
        }
        self.flight_pattern = -1;
        arwing.set_hidden(false);
    }
}

/// The 3D maker's frame: the path's forward axis and the ship's origin.
struct Frame {
    forward: Vec3,
    origin: Vec3,
}

/// `fp->joints[TopN]->translate` raised by `coll_data.floor_dist`: a
/// standing fighter's feet, or the floor under an airborne one; `None` for
/// `floor_line_id` -1 (over no floor). The source reads the projection the
/// fighter's last map process left; the port projects from its current
/// position, which that process ended at.
fn feet<I, F>(f: &Fighter, map: &MapQuery<'_, F>) -> Option<Vec3>
where
    F: Fn() -> I,
    I: IntoIterator<Item = MapSurface>,
{
    let top = super::top_n(f);
    if f.is_grounded() {
        return f.floor.map(|_| top);
    }
    let probe = Vec3::new(top.x, top.y + f.coll.bottom, top.z);
    let (_, dist) = crate::map::project_floor_line(&map.surfaces, probe)?;
    Some(Vec3::new(top.x, top.y + dist, top.z))
}

fn set_group(groups: &mut [MapGroup], status: GroupStatus) {
    if let Some(g) = groups.get_mut(usize::from(WING_GROUP)) {
        g.status = status;
    }
}
