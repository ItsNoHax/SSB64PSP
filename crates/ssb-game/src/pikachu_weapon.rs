//! `wppikachuthunderjolt.c` / `wppikachuthunder.c` (US).
use super::*;
use crate::map::{Contact, MASK_LWALL, MASK_RWALL};
use crate::wpmap::WeaponColl;
use ssb_engine::math::{
    defined_or, f_clc_dtor32, f_cst_dtor32, sy_arc_tan2, sy_vector_angle_diff_3d,
};

// `llPikachuSpecial1ThunderJolt{Air,Ground}WeaponAttributes` and
// `llPikachuMainThunder{Head,Trail}WeaponAttributes`, generated from the ROM
// at build time. `wpManagerMakeWeapon` copies each record's damage and map
// collision into the weapon it makes; the ground record's offset is applied
// by [`ThunderJolt::hit`].
const JOLT_COLL: BodyColl =
    crate::monster_weapon::weapon_attributes::PIKACHU_THUNDER_JOLT_AIR.map_coll;
/// The ground weapon's own `coll_data.map_coll` (its ROM record's box).
const JOLT_GROUND_COLL: BodyColl =
    crate::monster_weapon::weapon_attributes::PIKACHU_THUNDER_JOLT_GROUND.map_coll;
const HEAD_COLL: BodyColl = crate::monster_weapon::weapon_attributes::PIKACHU_THUNDER_HEAD.map_coll;
const JOLT_AIR: Hitbox =
    crate::monster_weapon::weapon_attributes::PIKACHU_THUNDER_JOLT_AIR.hitbox();
const JOLT_GROUND: Hitbox =
    crate::monster_weapon::weapon_attributes::PIKACHU_THUNDER_JOLT_GROUND.hitbox();
pub(super) const TRAIL_HIT: Hitbox =
    crate::monster_weapon::weapon_attributes::PIKACHU_THUNDER_TRAIL.hitbox();

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThunderJolt {
    pub owner_port: u8,
    pub position: Vec3,
    pub velocity: Vec3,
    pub lifetime: u16,
    pub damage: i32,
    /// `weapon_vars.thunder_jolt.line_type`: the line kind the ground weapon
    /// crawls; `None` for the aerial weapon.
    pub line_type: Option<MapSurfaceKind>,
    /// The aerial weapon's `coll_data` (`wpMapTestAllCheckFloor`).
    coll: WeaponColl,
    /// The ground weapon's `coll_data.floor_line_id`, `lwall_line_id` and
    /// `rwall_line_id` (`None` for -1).
    floor_line: Option<u16>,
    lwall_line: Option<u16>,
    rwall_line: Option<u16>,
    /// `coll_data.pos_prev`: `translate` before this frame's `vel_air`.
    pos_prev: Vec3,
    /// `lr`: ±1 on a floor; 2 ascending, 3 descending on a wall.
    pub direction: i8,
    /// `weapon_vars.thunder_jolt.rotate`: the angle of the line under the
    /// jolt at its last successful `mpCollisionGet[FC/LR]Common*` query.
    rotate: Vec2,
    /// The root DObj's `rotate.z`, which steers the ground weapon.
    rotate_z: f32,
    /// Counts `gcAddAnimAll` calls: the aerial weapon's own, then the ground
    /// weapon's at its creation and at every restart. Presentation only.
    pub anim_epoch: u16,
    /// `gcPlayAnimAll` calls since the last `gcAddAnimAll`.
    pub anim_ticks: u16,
    /// The ground weapon's `GObj::anim_frame`, at `gcSetAllAnimSpeed`'s 0.5.
    anim_frame: f32,
    /// The next play only parses the new scripts (`AOBJ_ANIM_CHANGED`).
    anim_fresh: bool,
    /// The ground weapon's root `rotate.y`: 180 or 0 degrees, set when it is
    /// made and on a reflect. Presentation only.
    pub model_rotate_y: f32,
    /// `wp->p_sfx`/`sfx_id`: the ground weapon's loop.
    pub sfx: Option<crate::sound::FgmHandle>,
}

/// `nWPPikachuThunderJoltStatus*`: `wpPikachuThunderJoltGroundGetStatus`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum JoltStatus {
    Null,
    Active,
    Destroy,
}

/// `gcSetAllAnimSpeed(new_gobj, 0.5F)` in `wpPikachuThunderJoltGroundMakeWeapon`.
pub const JOLT_GROUND_ANIM_SPEED: f32 = 0.5;
/// `WPPIKACHUJOLT_ANIM_PUSH_FRAME`.
const JOLT_ANIM_PUSH_FRAME: f32 = 7.5;
/// `WPPIKACHUJOLT_VEL`.
const JOLT_VEL: f32 = 55.0;
/// `WPPIKACHUJOLT_GRAVITY`, `WPPIKACHUJOLT_TVEL` (US).
const JOLT_GRAVITY: f32 = 0.0;
const JOLT_TVEL: f32 = 50.0;
const DEG_180: f32 = core::f32::consts::PI;
impl ThunderJolt {
    /// The air weapon's `WEAPON_FLAG_COLLPROJECT` target: its own box, its
    /// translate and its `coll_data` floor.
    pub(super) fn collproject(&mut self) -> (BodyColl, &mut Vec3, &mut FloorState) {
        (JOLT_COLL, &mut self.position, &mut self.coll.floor)
    }

    pub(super) fn new(s: WeaponSpawn) -> Self {
        let (sin, cos) = sin_cos(-core::f32::consts::FRAC_PI_4);
        Self {
            owner_port: s.owner_port,
            position: s.position,
            velocity: Vec3::new(cos * 40.0 * s.facing, sin * 40.0, 0.0),
            lifetime: 100,
            damage: JOLT_AIR.damage,
            line_type: None,
            coll: WeaponColl::default(),
            floor_line: None,
            lwall_line: None,
            rwall_line: None,
            pos_prev: s.position,
            direction: if s.facing < 0.0 { -1 } else { 1 },
            // Read only when the ground weapon's first query fails, which
            // its landing on the line rules out; up, never an unwritten
            // value (D-050).
            rotate: Vec2::new(0.0, 1.0),
            rotate_z: 0.0,
            anim_epoch: 0,
            anim_ticks: 0,
            anim_frame: 0.0,
            anim_fresh: true,
            model_rotate_y: 0.0,
            sfx: None,
        }
    }
    /// The ground weapon (`dWPPikachuThunderJoltGroundWeaponDesc`).
    pub fn grounded(&self) -> bool {
        self.line_type.is_some()
    }
    /// The root DObj's `rotate.z`: the ground weapon's
    /// `atan2(-angle.x, angle.y)` of the line it crawls.
    pub fn rotate_z(&self) -> f32 {
        self.rotate_z
    }
    /// `wpPikachuThunderJoltGroundAddAnim`: restart the animation and play it
    /// once.
    fn restart_anim(&mut self) {
        self.anim_epoch = self.anim_epoch.wrapping_add(1);
        self.anim_ticks = 1;
        self.anim_frame = 0.0;
        self.anim_fresh = false;
    }
    pub(super) fn hit(&self) -> (Hitbox, Vec3) {
        if self.grounded() {
            // Ground attribute offset (0,100,0), turned with the DObj's
            // `rotate.z`.
            let (sin, cos) = sin_cos(self.rotate_z);
            (
                JOLT_GROUND,
                self.position + Vec3::new(-sin * 100.0, cos * 100.0, 0.0),
            )
        } else {
            (JOLT_AIR, self.position)
        }
    }
    pub(super) fn reflect(&mut self, f: &Reflector) {
        self.owner_port = f.port;
        self.lifetime = 100;
        if self.velocity.x * f.lr < 0.0 {
            self.velocity.x = -self.velocity.x;
        }
        if self.grounded() {
            self.direction = if self.velocity.x >= 0.0 { 1 } else { -1 };
            // `wpPikachuThunderJoltGroundProcReflector`.
            self.model_rotate_y = if self.velocity.x >= 0.0 { DEG_180 } else { 0.0 };
        }
        self.damage = ((self.damage as f32 * 1.8 + 0.99) as i32).min(100);
    }
    /// `coll_data.<kind>_line_id`.
    fn line_of(&self, kind: MapSurfaceKind) -> Option<u16> {
        match kind {
            MapSurfaceKind::Floor => self.floor_line,
            MapSurfaceKind::LeftWall => self.lwall_line,
            MapSurfaceKind::RightWall => self.rwall_line,
            MapSurfaceKind::Ceiling => None,
        }
    }
    /// Ride `line` of `kind` with `lr`: `line_type`, the `coll_data` line id
    /// and `lr`, as each transition sets them.
    fn set_line(&mut self, kind: MapSurfaceKind, line: u16, lr: i8) {
        self.line_type = Some(kind);
        match kind {
            MapSurfaceKind::Floor => self.floor_line = Some(line),
            MapSurfaceKind::LeftWall => self.lwall_line = Some(line),
            MapSurfaceKind::RightWall => self.rwall_line = Some(line),
            MapSurfaceKind::Ceiling => {}
        }
        self.direction = lr;
    }
    /// The ground weapon on `s`'s line with `lr`, angled by it, as a
    /// transition onto it leaves the jolt.
    #[cfg(test)]
    pub(crate) fn ride(&mut self, s: MapSurface, lr: i8) {
        self.set_line(s.kind, s.topology.map_or(0, |t| t.line), lr);
        self.rotate = surface_normal(s.kind, s.segment);
        self.rotate_z = sy_arc_tan2(-self.rotate.x, self.rotate.y);
        self.damage = JOLT_GROUND.damage;
    }
    /// `wpPikachuThunderJoltAirProcUpdate`/`...GroundProcUpdate` and their
    /// `proc_map`. Every end makes a small dust cloud where the jolt is.
    pub(super) fn tick<I, F>(&mut self, surfaces: F, fx: &mut Emit) -> bool
    where
        F: Fn() -> I + Copy,
        I: IntoIterator<Item = MapSurface>,
    {
        let alive = self.tick_map(surfaces, fx);
        if !alive {
            fx.push(Fx::DustExpandSmall(self.position));
        }
        alive
    }
    /// `wpProcessProcWeaponMain` for either weapon: animation, `proc_update`,
    /// `translate += vel_air`, `proc_map`. Returns false where the original
    /// makes its dust and ends the weapon.
    fn tick_map<I, F>(&mut self, surfaces: F, fx: &mut Emit) -> bool
    where
        F: Fn() -> I + Copy,
        I: IntoIterator<Item = MapSurface>,
    {
        // `wpProcessProcWeaponMain` plays the DObj animation before
        // `proc_update`. The first play after `gcAddAnimAll` only parses.
        self.anim_ticks = self.anim_ticks.wrapping_add(1);
        if core::mem::take(&mut self.anim_fresh) {
        } else if self.grounded() {
            self.anim_frame += JOLT_GROUND_ANIM_SPEED;
        }
        let Some(kind) = self.line_type else {
            // `wpPikachuThunderJoltAirProcUpdate`.
            self.lifetime = self.lifetime.saturating_sub(1);
            if self.lifetime == 0 {
                return false;
            }
            // `wpMainApplyGravityClampTVel`.
            crate::wpmap::apply_gravity_clamp_tvel(&mut self.velocity, JOLT_GRAVITY, JOLT_TVEL);
            self.pos_prev = self.position;
            self.position += self.velocity;
            return self.air_map(surfaces, fx);
        };
        // `wpPikachuThunderJoltGroundProcUpdate`, ahead of the lifetime:
        // `efManagerPikachuThunderJoltMakeEffect` at the DObj, then the
        // animation restarts.
        if self.anim_frame == JOLT_ANIM_PUSH_FRAME {
            fx.push(Fx::ThunderJolt {
                pos: self.position,
                rotate_z: self.rotate_z,
            });
            self.restart_anim();
        }
        self.lifetime = self.lifetime.saturating_sub(1);
        if self.lifetime == 0 {
            return false;
        }
        // `cosf(rotate.z)`, `__sinf(rotate.z)`.
        let (sin, cos) = sin_cos(self.rotate_z);
        self.velocity.x = cos * JOLT_VEL;
        self.velocity.y = sin * JOLT_VEL;
        if kind == MapSurfaceKind::Floor {
            self.velocity.x *= self.direction as f32;
        } else {
            if self.direction == 2 && self.velocity.y < 0.0 {
                self.velocity.y = -self.velocity.y;
            }
            if self.direction == 3 && self.velocity.y > 0.0 {
                self.velocity.y = -self.velocity.y;
            }
        }
        self.pos_prev = self.position;
        self.position += self.velocity;
        self.ground_map(&surfaces)
    }
    /// `wpPikachuThunderJoltAirProcMap`: a floor, else a left wall, else a
    /// right wall met by `wpMapTestAllCheckFloor` makes the ground weapon;
    /// on a wall, at a height clamped to the wall's ends.
    fn air_map<I, F>(&mut self, surfaces: F, fx: &mut Emit) -> bool
    where
        F: Fn() -> I + Copy,
        I: IntoIterator<Item = MapSurface>,
    {
        self.coll.begin();
        if self
            .coll
            .test_all_check_floor(&JOLT_COLL, self.pos_prev, &mut self.position, &surfaces)
        {
            return self.make_ground(MapSurfaceKind::Floor, surfaces, fx);
        }
        for (mask, kind, line) in [
            (MASK_LWALL, MapSurfaceKind::LeftWall, self.coll.lwall_line),
            (MASK_RWALL, MapSurfaceKind::RightWall, self.coll.rwall_line),
        ] {
            if self.coll.mask_curr & mask == 0 {
                continue;
            }
            if let Some(line) = line {
                // `mpCollisionGet[L/R]WallEdgeU`, then `...EdgeD`.
                for high in [true, false] {
                    let (found, edge) = defined_or(
                        crate::map::line_edge(&surfaces, kind, line, high),
                        Vec2::ZERO,
                    );
                    if found
                        && (if high {
                            self.position.y > edge.y
                        } else {
                            self.position.y < edge.y
                        })
                    {
                        self.position.y = edge.y;
                    }
                }
            }
            return self.make_ground(kind, surfaces, fx);
        }
        true
    }
    /// `wpPikachuThunderJoltGroundMakeWeapon`: the ground weapon replaces
    /// the aerial one at its `translate`, keeping its life and line ids.
    fn make_ground<I, F>(&mut self, kind: MapSurfaceKind, surfaces: F, fx: &mut Emit) -> bool
    where
        F: Fn() -> I + Copy,
        I: IntoIterator<Item = MapSurface>,
    {
        let air_vel_x = self.velocity.x;
        self.line_type = Some(kind);
        self.floor_line = self.coll.floor.line;
        self.lwall_line = self.coll.lwall_line;
        self.rwall_line = self.coll.rwall_line;
        self.damage = JOLT_GROUND.damage;
        // `wpManagerMakeWeapon` zeroes `vel_air`, adds the ground animation
        // and (`WEAPON_FLAG_COLLPROJECT`) sets `pos_prev` to the aerial
        // weapon's `translate`; `lr` stays the aerial weapon's until the
        // switch below.
        self.velocity = Vec3::ZERO;
        self.anim_epoch = self.anim_epoch.wrapping_add(1);
        self.anim_ticks = 0;
        self.anim_frame = 0.0;
        self.anim_fresh = true;
        self.pos_prev = self.position;
        // The `WEAPON_FLAG_COLLPROJECT | WEAPON_FLAG_PARENT_WEAPON` pass: a
        // zero-length sweep from the aerial translate, probed with the
        // aerial box on the `pos_prev` side and the ground weapon's own
        // (taller, bottom-anchored) box on the translate side. Its floor and
        // wall ids are overwritten by the copies above, made after it in
        // the original, so only the translate survives.
        self.position = crate::map::run_default_collision_project(
            &JOLT_GROUND_COLL,
            &JOLT_COLL,
            self.pos_prev,
            self.position,
            surfaces,
        )
        .pos;
        // `wpPikachuThunderJoltGroundProcMap(new_gobj)`: its result is
        // dropped, so an end only leaves its dust.
        if !self.ground_map(&surfaces) {
            fx.push(Fx::DustExpandSmall(self.position));
        }
        match self.line_type {
            Some(MapSurfaceKind::Floor) => {
                self.direction = if air_vel_x >= 0.0 { 1 } else { -1 };
                self.model_rotate_y = if self.direction >= 0 { DEG_180 } else { 0.0 };
            }
            Some(MapSurfaceKind::LeftWall) => {
                if sy_arc_tan2(self.rotate.y, self.rotate.x) > f_cst_dtor32(135.0) {
                    self.model_rotate_y = 0.0;
                    self.direction = 3;
                } else {
                    self.model_rotate_y = DEG_180;
                    self.direction = 2;
                }
            }
            Some(MapSurfaceKind::RightWall) => {
                if sy_arc_tan2(self.rotate.y, self.rotate.x) > f_cst_dtor32(45.0) {
                    self.model_rotate_y = 0.0;
                    self.direction = 2;
                } else {
                    self.model_rotate_y = DEG_180;
                    self.direction = 3;
                }
            }
            _ => {}
        }
        // `efManagerPikachuThunderJoltMakeEffect` at the new weapon's DObj,
        // before its FGM.
        fx.push(Fx::ThunderJolt {
            pos: self.position,
            rotate_z: self.rotate_z,
        });
        // `wpMainPlayFGM(new_wp, nSYAudioFGMPikachuElectricLoop)`: the new
        // weapon has no sound yet.
        self.sfx = crate::sound::play_fgm(crate::sound::id::nSYAudioFGMPikachuElectricLoop);
        // `wpManagerMakeWeapon` links the ground weapon after this one, so
        // `gcRunAll` runs its process this same frame: the N64 cpufx-s
        // trace's new jolt has already spent a life and moved 55 (RE-541).
        self.tick_map(surfaces, fx)
    }
    /// `wpPikachuThunderJoltGroundProcMap`: follow the line along its own
    /// axis; past its end, step onto the joined wall or floor; a wall or
    /// floor crossed on the way (`GetStatus`) takes over. Returns false
    /// where the original makes its dust and ends the weapon.
    fn ground_map<I, F>(&mut self, surfaces: &F) -> bool
    where
        F: Fn() -> I,
        I: IntoIterator<Item = MapSurface>,
    {
        let Some(kind) = self.line_type else {
            return true;
        };
        let Some(line) = self
            .line_of(kind)
            .filter(|&l| crate::map::line_exists(surfaces, l))
        else {
            return false;
        };
        let (on_line, (dist, hit)) = defined_or(
            crate::map::line_common(surfaces, kind, line, self.position),
            (0.0, Contact::NONE),
        );
        if on_line {
            self.rotate = hit.normal;
            if kind == MapSurfaceKind::Floor {
                self.position.y += dist;
            } else {
                self.position.x += dist;
            }
            return self.get_status(surfaces) != JoltStatus::Destroy;
        }
        // `mpCollisionGetEdgeUnder[R/L]LineID` (floor, by `lr` ±1) or
        // `mpCollisionGetEdge[Right/Left][U/D]LineID` (wall, U when `lr` 2).
        let high = match (kind, self.direction) {
            (MapSurfaceKind::Floor, 1) => Some(true),
            (MapSurfaceKind::Floor, -1) => Some(false),
            (MapSurfaceKind::Floor, _) => None,
            (_, lr) => Some(lr == 2),
        };
        let next = high.and_then(|high| crate::map::line_neighbor(surfaces, kind, line, high));
        if let Some((next_kind, next_line)) = next {
            use MapSurfaceKind::{Floor, LeftWall, RightWall};
            // The new line, `lr`, and which of its ends the jolt starts at
            // (`mpCollisionGet[L/R]WallEdge[U/D]`, `mpCollisionGetFloorEdge[L/R]`).
            let forward = match kind {
                Floor => self.direction == 1,
                _ => self.direction == 2,
            };
            let to = match (kind, forward, next_kind) {
                (Floor, true, LeftWall) => Some((2, false)),
                (Floor, true, RightWall) => Some((3, true)),
                (Floor, false, LeftWall) => Some((3, true)),
                (Floor, false, RightWall) => Some((2, false)),
                (LeftWall, true, Floor) => Some((1, false)),
                (LeftWall, false, Floor) => Some((-1, true)),
                (RightWall, true, Floor) => Some((-1, true)),
                (RightWall, false, Floor) => Some((1, false)),
                _ => None,
            };
            if let Some((lr, high)) = to {
                self.set_line(next_kind, next_line, lr);
                let (found, edge) = defined_or(
                    crate::map::line_edge(surfaces, next_kind, next_line, high),
                    Vec2::ZERO,
                );
                if found {
                    self.position = Vec3::new(edge.x, edge.y, 0.0);
                }
                return self.check_destroy(surfaces);
            }
        }
        self.get_status(surfaces) == JoltStatus::Active
    }
    /// `wpPikachuThunderJoltGroundGetStatus`: from `pos_prev` to `translate`,
    /// a floor jolt crossing the wall ahead (not the one joined at its other
    /// end) climbs it; a descending wall jolt crossing a floor (not the one
    /// joined at its top) runs along it; an ascending one crossing a ceiling
    /// ends. Otherwise `rotate.z` follows the line under it.
    fn get_status<I, F>(&mut self, surfaces: &F) -> JoltStatus
    where
        F: Fn() -> I,
        I: IntoIterator<Item = MapSurface>,
    {
        use MapSurfaceKind::{Ceiling, Floor, LeftWall, RightWall};
        let from = Vec2::new(self.pos_prev.x, self.pos_prev.y);
        let to = Vec2::new(self.position.x, self.position.y);
        let lr = self.direction;
        // (line kind crossed, the line id it must not be, its new `lr`).
        let check = match (self.line_type, lr) {
            (Some(Floor), 1) => Some((LeftWall, self.joined(surfaces, Floor, false), 2)),
            (Some(Floor), _) => Some((RightWall, self.joined(surfaces, Floor, true), 2)),
            (Some(LeftWall), 3) => Some((Floor, self.joined(surfaces, LeftWall, true), -1)),
            (Some(RightWall), 3) => Some((Floor, self.joined(surfaces, RightWall, true), 1)),
            (Some(LeftWall | RightWall), _) => Some((Ceiling, None, 0)),
            _ => None,
        };
        if let Some((cross_kind, excluded, new_lr)) = check {
            let (crossed, (hit, at)) = defined_or(
                crate::map::line_crossing(surfaces, cross_kind, from, to),
                (Contact::NONE, Vec2::ZERO),
            );
            if crossed && excluded != Some(hit.line) {
                if cross_kind == Ceiling {
                    return JoltStatus::Destroy;
                }
                let at3 = Vec3::new(at.x, at.y, 0.0);
                let (on_line, (_, angle)) = defined_or(
                    crate::map::line_common(surfaces, cross_kind, hit.line, at3),
                    (0.0, Contact::NONE),
                );
                if !on_line {
                    return JoltStatus::Destroy;
                }
                self.set_line(cross_kind, hit.line, new_lr);
                self.position = at3;
                self.restart_anim();
                self.rotate_z = sy_arc_tan2(-angle.normal.x, angle.normal.y);
                return JoltStatus::Active;
            }
        }
        self.rotate_z = sy_arc_tan2(-self.rotate.x, self.rotate.y);
        JoltStatus::Null
    }
    /// The line joined at one end of the current `kind` line
    /// (`mpCollisionGetEdge*LineID`), any kind.
    fn joined<I, F>(&self, surfaces: &F, kind: MapSurfaceKind, high: bool) -> Option<u16>
    where
        F: Fn() -> I,
        I: IntoIterator<Item = MapSurface>,
    {
        let line = self.line_of(kind)?;
        crate::map::line_neighbor(surfaces, kind, line, high).map(|(_, l)| l)
    }
    /// `wpPikachuThunderJoltGroundCheckDestroy`: at a corner, the new line
    /// turning more than `WPPIKACHUJOLT_ROTATE_ANGLE_MAX` (100 degrees) from
    /// the last one ends the jolt; otherwise `rotate.z` follows it.
    fn check_destroy<I, F>(&mut self, surfaces: &F) -> bool
    where
        F: Fn() -> I,
        I: IntoIterator<Item = MapSurface>,
    {
        self.restart_anim();
        let kind = self.line_type.unwrap_or(MapSurfaceKind::Floor);
        // The jolt stands on the new line's end, so the query succeeds; a
        // failure would leave `angle` unwritten, read here as no turn.
        let (on_line, (_, hit)) = defined_or(
            self.line_of(kind)
                .and_then(|line| crate::map::line_common(surfaces, kind, line, self.position)),
            (0.0, Contact::NONE),
        );
        let angle = if on_line { hit.normal } else { self.rotate };
        if sy_vector_angle_diff_3d(
            Vec3::new(angle.x, angle.y, 0.0),
            Vec3::new(self.rotate.x, self.rotate.y, 0.0),
        ) > f_clc_dtor32(100.0)
        {
            return false;
        }
        self.rotate_z = sy_arc_tan2(-angle.x, angle.y);
        true
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThunderHead {
    pub owner_port: u8,
    pub position: Vec3,
    pub lifetime: u16,
    pub group: u16,
    pub motion_count: u16,
    pub hit_ports: u8,
    pub notify_destroy: bool,
}
impl ThunderHead {
    /// `wpPikachuThunderHeadMakeTrailEffect`'s effect branch.
    pub(super) fn trail_effect(&self, lifetime: u8, texture: u8) -> Fx {
        Fx::ThunderTrail {
            pos: Vec3::new(self.position.x, self.position.y, 0.0),
            lifetime,
            texture,
        }
    }
    pub(super) fn new(s: WeaponSpawn, group: u16) -> Self {
        Self {
            owner_port: s.owner_port,
            position: s.position,
            lifetime: 40,
            group,
            motion_count: s.stale.motion_count,
            hit_ports: 0,
            notify_destroy: true,
        }
    }
    pub(super) fn tick<I, F>(&mut self, surfaces: F, fx: &mut Emit) -> bool
    where
        F: Fn() -> I,
        I: IntoIterator<Item = MapSurface>,
    {
        self.lifetime = self.lifetime.saturating_sub(1);
        if self.lifetime == 0 {
            // `wpPikachuThunderHeadProcUpdate`: dust, then the last segment.
            fx.push(Fx::DustExpandSmall(self.position));
            fx.push(self.trail_effect(10, 3));
            return false;
        }
        let from = self.position;
        self.position.y -= 450.0;
        // `wpPikachuThunderHeadProcMap`: the 450 fall splits into two
        // substeps, and the one that meets the map is where it ends.
        if crate::wpmap::check_coll_end(&HEAD_COLL, from, &mut self.position, &surfaces) {
            fx.push(Fx::Quake(1));
            fx.push(Fx::SparkleWhite(self.position));
            return false;
        }
        true
    }
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThunderTrail {
    pub owner_port: u8,
    pub position: Vec3,
    pub lifetime: u16,
    pub group: u16,
    pub hit_ports: u8,
    /// `mobj->texture_id_curr`: 0 when made (`gcAddMObjForDObj`), then the
    /// frame each update draws (RE-417).
    pub texture: u8,
}
impl ThunderTrail {
    pub(super) fn new(h: ThunderHead) -> Self {
        Self {
            owner_port: h.owner_port,
            position: h.position,
            lifetime: 10,
            group: h.group,
            hit_ports: h.hit_ports,
            texture: 0,
        }
    }
    /// `wpPikachuThunderTrailProcUpdate`: under `WPPIKACHUTHUNDER_EXPIRE`
    /// the segment becomes a fading effect; otherwise it picks a frame.
    pub(super) fn tick(&mut self, fx: &mut Emit) -> bool {
        self.lifetime = self.lifetime.saturating_sub(1);
        if self.lifetime >= 6 {
            fx.push(Fx::TextureRand(3));
            true
        } else {
            fx.push(Fx::ThunderTrail {
                pos: Vec3::new(self.position.x, self.position.y, 0.0),
                lifetime: 6,
                texture: 0,
            });
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fighter::FighterKind;
    fn spawn(kind: WeaponKind, x: f32, y: f32) -> WeaponSpawn {
        WeaponSpawn {
            kind,
            owner_port: 0,
            team: 0,
            position: Vec3::new(x, y, 0.0),
            facing: 1.0,
            stale: crate::stale::WeaponStale::FRESH,
        }
    }
    fn surface(
        kind: MapSurfaceKind,
        line: u16,
        a: (i16, i16, u16),
        b: (i16, i16, u16),
    ) -> MapSurface {
        MapSurface {
            motion: None,
            kind,
            segment: Segment {
                x1: a.0,
                y1: a.1,
                x2: b.0,
                y2: b.1,
                flags: 0,
            },
            topology: Some(SurfaceTopology {
                line,
                point: 0,
                segments: 1,
                vertex1: a.2,
                vertex2: b.2,
            }),
        }
    }
    /// `wpPikachuThunderJoltGroundMakeWeapon`'s `WEAPON_FLAG_COLLPROJECT`
    /// pass (wppikachuthunderjolt.c:694) is a zero-length sweep at the
    /// aerial translate, but its translate side probes with the ground
    /// weapon's own box (`top 200`, `center 100`, `bottom 0`): at a floor's
    /// corner with a wall, the taller box's side moves the new weapon out
    /// of the wall before its first `proc_map`.
    #[test]
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
    fn ground_jolt_make_snaps_its_own_box_out_of_a_wall_corner() {
        let made_at = |map: [MapSurface; 2]| {
            // The aerial weapon resting on the floor: its bottom (-50) on y 0.
            let mut j = ThunderJolt::new(spawn(WeaponKind::PikachuThunderJolt, 0.0, 50.0));
            let (_, floor) = crate::map::floor_point(|| map, 0, 0.0).expect("the floor");
            j.coll.floor.set(floor);
            let mut fx = crate::wpeffect::Emit::default();
            j.make_ground(MapSurfaceKind::Floor, || map, &mut fx);
            let made = fx.iter().find_map(|e| match e {
                crate::wpeffect::WeaponEffect::ThunderJolt { pos, .. } => Some(pos),
                _ => None,
            });
            made.expect("the ground make's burst")
        };
        // A wall rising straight from the floor's end 40 to the right: both
        // boxes' right points (x + 50) are past it.
        let upright = [
            surface(MapSurfaceKind::Floor, 0, (-1000, 0, 0), (40, 0, 1)),
            surface(MapSurfaceKind::LeftWall, 1, (40, 0, 1), (40, 1000, 2)),
        ];
        assert_eq!(made_at(upright), Vec3::new(-10.0, 0.0, 0.0));
        // A wall leaning over the floor: x 53.3 at the aerial box's side
        // height (y 50), x 40 at the ground box's (y 150). Only the ground
        // box reaches it.
        let leaning = [
            surface(MapSurfaceKind::Floor, 0, (-1000, 0, 0), (60, 0, 1)),
            surface(MapSurfaceKind::LeftWall, 1, (60, 0, 1), (20, 300, 2)),
        ];
        let made = made_at(leaning);
        assert!((made.x + 10.0).abs() < 1e-3, "{made:?}");
        assert_eq!(made.y, 0.0);
    }
    #[test]
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
    fn air_jolt_converts_with_remaining_life_and_ground_damage() {
        let mut j = ThunderJolt::new(spawn(WeaponKind::PikachuThunderJolt, 0.0, 70.0));
        let floor = [surface(
            MapSurfaceKind::Floor,
            0,
            (-1000, 0, 0),
            (1000, 0, 1),
        )];
        let mut fx = crate::wpeffect::Emit::default();
        assert!(j.tick(|| floor, &mut fx));
        // The air jolt's life, then the ground weapon's first process in
        // the same frame: a second life and one 55 step (RE-541).
        assert_eq!(j.lifetime, 98);
        assert_eq!(j.position.y, 0.0);
        assert_eq!(j.hit().0.damage, 7);
        assert_eq!(j.hit().1.y, 100.0);
        // The burst stays where the ground weapon was made.
        let made = fx.iter().find_map(|e| match e {
            crate::wpeffect::WeaponEffect::ThunderJolt { pos, rotate_z } => Some((pos, rotate_z)),
            _ => None,
        });
        let (made, rotate_z) = made.expect("the ground make's burst");
        assert_eq!(j.position.x - made.x, 55.0);
        assert_eq!((made.y, rotate_z), (0.0, 0.0));
        let x = j.position.x;
        assert!(j.tick(|| floor, &mut crate::wpeffect::Emit::default()));
        assert_eq!(j.position.x - x, 55.0);
    }
    #[test]
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
    fn ground_jolt_restarts_its_animation_every_push_frame() {
        let mut j = ThunderJolt::new(spawn(WeaponKind::PikachuThunderJolt, 0.0, 70.0));
        let floor = [surface(
            MapSurfaceKind::Floor,
            0,
            (-10000, 0, 0),
            (10000, 0, 1),
        )];
        assert!(j.tick(|| floor, &mut crate::wpeffect::Emit::default()));
        // A new ground weapon: its animation is added and, its process
        // running in the frame it is made, played once, which only parses
        // (RE-541).
        assert_eq!((j.anim_epoch, j.anim_ticks), (1, 1));
        assert_eq!(j.anim_frame, 0.0);
        assert_eq!(j.model_rotate_y, core::f32::consts::PI);
        assert_eq!(j.rotate_z(), 0.0);
        // Each later play adds 0.5, and `anim_frame == 7.5` restarts it
        // with one play.
        for ticks in 2..=15 {
            assert!(j.tick(|| floor, &mut crate::wpeffect::Emit::default()));
            assert_eq!((j.anim_epoch, j.anim_ticks), (1, ticks));
        }
        assert!(j.tick(|| floor, &mut crate::wpeffect::Emit::default()));
        assert_eq!((j.anim_epoch, j.anim_ticks), (2, 1));
        for _ in 0..15 {
            assert!(j.tick(|| floor, &mut crate::wpeffect::Emit::default()));
        }
        assert_eq!((j.anim_epoch, j.anim_ticks), (3, 1));
    }
    #[test]
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
    fn air_jolt_resolves_ceiling_without_converting_and_expires_on_tick_100() {
        let mut j = ThunderJolt::new(spawn(WeaponKind::PikachuThunderJolt, 0.0, 0.0));
        j.velocity = Vec3::new(0.0, 40.0, 0.0);
        let ceiling = [surface(
            MapSurfaceKind::Ceiling,
            0,
            (-100, 50, 0),
            (100, 50, 1),
        )];
        assert!(j.tick(|| ceiling, &mut crate::wpeffect::Emit::default()));
        assert!(!j.grounded());
        assert_eq!(j.position.y, 0.0);
        for _ in 1..99 {
            assert!(j.tick(|| [], &mut crate::wpeffect::Emit::default()));
        }
        assert!(!j.tick(|| [], &mut crate::wpeffect::Emit::default()));
    }
    #[test]
    fn convex_corner_follows_original_ids_and_wall_direction() {
        let floor = surface(MapSurfaceKind::Floor, 3, (-200, 0, 5), (200, 0, 6));
        let wall = surface(MapSurfaceKind::RightWall, 5, (200, 0, 6), (200, -400, 0));
        let mut j = ThunderJolt::new(spawn(WeaponKind::PikachuThunderJolt, 180.0, 0.0));
        j.ride(floor, 1);
        assert!(j.tick(|| [floor, wall], &mut crate::wpeffect::Emit::default()));
        assert_eq!(j.line_type, Some(MapSurfaceKind::RightWall));
        assert_eq!(j.direction, 3);
        assert_eq!(j.position, Vec3::new(200.0, 0.0, 0.0));
        assert!(j.tick(|| [floor, wall], &mut crate::wpeffect::Emit::default()));
        assert_eq!(j.position.y, -55.0);
        // Reflector's wpMainVelSetLR stores ±1 even on walls. Update treats
        // every value other than 2 as downward; it does not invent direction 3.
        j.reflect(&crate::weapon::Reflector::of(&Fighter::new(
            FighterKind::Fox,
            2,
            3,
        )));
        assert_eq!(j.direction, 1);
        assert!(j.tick(|| [floor, wall], &mut crate::wpeffect::Emit::default()));
        assert_eq!(j.position.y, -110.0);
        // Same coordinates, distinct original vertex IDs: no join exists.
        let mut disconnected = wall;
        disconnected.topology.as_mut().unwrap().vertex1 = 99;
        let mut j = ThunderJolt::new(spawn(WeaponKind::PikachuThunderJolt, 180.0, 0.0));
        j.ride(floor, 1);
        assert!(!j.tick(
            || [floor, disconnected],
            &mut crate::wpeffect::Emit::default()
        ));
    }
    #[test]
    fn neighbor_choice_uses_last_original_line_id_and_not_iterator_order() {
        let floor = surface(MapSurfaceKind::Floor, 3, (-200, 0, 5), (200, 0, 6));
        let wall = surface(MapSurfaceKind::RightWall, 5, (200, 0, 6), (200, -400, 0));
        let ceiling = surface(MapSurfaceKind::Ceiling, 7, (200, 0, 6), (400, 0, 9));
        assert_eq!(
            crate::map::line_neighbor(&|| [ceiling, floor, wall], MapSurfaceKind::Floor, 3, true),
            Some((MapSurfaceKind::Ceiling, 7))
        );
        let mut j = ThunderJolt::new(spawn(WeaponKind::PikachuThunderJolt, 180.0, 0.0));
        j.ride(floor, 1);
        assert!(!j.tick(
            || [ceiling, floor, wall],
            &mut crate::wpeffect::Emit::default()
        ));
    }
    /// `wpPikachuThunderJoltGroundGetStatus`: a floor jolt crossing the
    /// wall ahead climbs it from the crossing, ascending.
    #[test]
    fn floor_jolt_climbs_the_wall_it_crosses() {
        let floor = surface(MapSurfaceKind::Floor, 1, (-400, 0, 0), (400, 0, 1));
        let wall = surface(MapSurfaceKind::LeftWall, 2, (100, -50, 2), (100, 400, 3));
        let mut j = ThunderJolt::new(spawn(WeaponKind::PikachuThunderJolt, 80.0, 0.0));
        j.ride(floor, 1);
        assert!(j.tick(|| [floor, wall], &mut crate::wpeffect::Emit::default()));
        assert_eq!(j.line_type, Some(MapSurfaceKind::LeftWall));
        assert_eq!(j.direction, 2);
        assert_eq!(j.position, Vec3::new(100.0, 0.0, 0.0));
        assert_eq!(j.anim_ticks, 1);
        assert!(j.tick(|| [floor, wall], &mut crate::wpeffect::Emit::default()));
        assert_eq!((j.position.x, j.position.y), (100.0, 55.0));
    }
    /// `wpPikachuThunderJoltGroundCheckDestroy`: a corner turning more than
    /// 100 degrees ends the jolt; 97 degrees does not.
    #[test]
    fn corner_past_one_hundred_degrees_destroys_jolt() {
        let floor = surface(MapSurfaceKind::Floor, 3, (-200, 0, 5), (200, 0, 6));
        for (foot, alive) in [(150, true), (100, false)] {
            // An undercut right side: 97 or 104 degrees from the floor's up.
            let wall = surface(MapSurfaceKind::RightWall, 5, (200, 0, 6), (foot, -400, 0));
            let mut j = ThunderJolt::new(spawn(WeaponKind::PikachuThunderJolt, 180.0, 0.0));
            j.ride(floor, 1);
            let mut fx = crate::wpeffect::Emit::default();
            assert_eq!(j.tick(|| [floor, wall], &mut fx), alive, "foot {foot}");
            assert_eq!(j.line_type, Some(MapSurfaceKind::RightWall));
            assert_eq!((j.position.x, j.position.y), (200.0, 0.0));
            let dust = fx
                .iter()
                .any(|e| matches!(e, crate::wpeffect::WeaponEffect::DustExpandSmall(_)));
            assert_eq!(dust, !alive);
        }
    }
    /// `wpPikachuThunderJoltAirProcMap`: a left wall makes the ground
    /// weapon on it; a vertical one (`atan2(rotate.y, rotate.x)` 180 > 135)
    /// sends it down with `rotate.y` 0, and it moves 55 that same frame.
    #[test]
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
    fn air_jolt_meeting_a_left_wall_crawls_down_it() {
        let wall = surface(MapSurfaceKind::LeftWall, 4, (300, -2000, 0), (300, 2000, 1));
        let mut j = ThunderJolt::new(spawn(WeaponKind::PikachuThunderJolt, 0.0, 500.0));
        j.velocity = Vec3::new(40.0, 0.0, 0.0);
        let mut ticks = 0;
        while !j.grounded() {
            let y = j.position.y;
            assert!(j.tick(|| [wall], &mut crate::wpeffect::Emit::default()));
            ticks += 1;
            assert!(ticks < 20);
            if j.grounded() {
                assert_eq!(j.position.y, y - 55.0);
            }
        }
        assert_eq!(j.line_type, Some(MapSurfaceKind::LeftWall));
        assert_eq!((j.direction, j.model_rotate_y), (3, 0.0));
        assert_eq!(j.position.x, 300.0);
    }
    #[test]
    fn descending_wall_ceiling_contact_destroys_jolt() {
        let wall = surface(MapSurfaceKind::LeftWall, 1, (0, -400, 0), (0, 400, 1));
        let ceiling = surface(MapSurfaceKind::Ceiling, 2, (-100, 50, 2), (100, 50, 3));
        let mut j = ThunderJolt::new(spawn(WeaponKind::PikachuThunderJolt, 0.0, 25.0));
        j.ride(wall, 2);
        assert!(!j.tick(|| [wall, ceiling], &mut crate::wpeffect::Emit::default()));
    }
    #[test]
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
    fn thunder_head_is_harmless_and_trails_share_one_hit_record() {
        let mut pool = WeaponPool::default();
        pool.spawn(spawn(WeaponKind::PikachuThunder, 0.0, 1500.0));
        pool.tick(|| [], None);
        assert_eq!(pool.thunder_heads().next().unwrap().position.y, 1050.0);
        assert_eq!(pool.thunder_trails().next().unwrap().position.y, 1500.0);
        let mut defender = Fighter::new(FighterKind::Mario, 1, 3);
        defender.pos = Vec3::new(0.0, 1500.0, 0.0);
        pool.apply_hits(&mut defender);
        crate::combat::resolve(&mut defender);
        assert_eq!(defender.damage, 12);
        // New trail at 1050 inherits the group's record stored in the head.
        pool.tick(|| [], None);
        defender.pos = Vec3::new(0.0, 1050.0, 0.0);
        pool.apply_hits(&mut defender);
        crate::combat::resolve(&mut defender);
        assert_eq!(defender.damage, 12);
        let mut other = Fighter::new(FighterKind::Mario, 2, 3);
        other.pos = Vec3::new(0.0, 600.0, 0.0);
        pool.apply_hits(&mut other);
        crate::combat::resolve(&mut other);
        assert_eq!(other.damage, 0, "head has no attack collision");
        pool.tick(|| [], None);
        other.pos = Vec3::new(0.0, 1050.0, 0.0);
        pool.apply_hits(&mut other);
        crate::combat::resolve(&mut other);
        assert_eq!(other.damage, 12);
    }
    /// `wpManagerMakeWeapon` gives the trail `GMHITCOLLISION_FLAG_ALL`, so
    /// `itProcessSearchHitWeapon` tests both its offsets against an item;
    /// `wpPikachuThunderTrailProcHit` returns FALSE. The record covers the
    /// group (`wpProcessUpdateHitInteractStats`), head included, and a later
    /// trail copies the head's records (`wpPikachuThunderTrailMakeWeapon`).
    #[test]
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
    fn a_thunder_trail_hits_an_item_once_for_its_group() {
        let mut pool = WeaponPool::default();
        pool.spawn(spawn(WeaponKind::PikachuThunder, 0.0, 1500.0));
        pool.tick(|| [], None);
        let mut items = crate::item::ItemPool::default();
        let slot = items
            .make_setup_common(1, None, Vec3::ZERO, Vec3::ZERO, &core::iter::empty)
            .unwrap();
        let item = items.get_mut(slot).unwrap();
        let id = crate::item::ITEM_RECORD_BASE + slot;
        let mut emit = crate::wpeffect::Emit::default();
        // Out of the upper offset's reach (1620): only the lower one (1380).
        item.pos = Vec3::new(0.0, 1180.0, 0.0);
        pool.hit_item(item, id, &mut emit);
        assert_eq!(item.damage_queue, 12, "the lower offset meets the item");
        pool.finish_item_hits();
        assert_eq!(pool.thunder_trails().count(), 1, "the trail survives");
        pool.hit_item(item, id, &mut emit);
        assert_eq!(item.damage_queue, 12);
        // The next trail, made at 1050, inherits the head's record.
        pool.tick(|| [], None);
        assert_eq!(pool.thunder_trails().count(), 2);
        item.pos = Vec3::new(0.0, 1050.0, 0.0);
        pool.hit_item(item, id, &mut emit);
        assert_eq!(item.damage_queue, 12);
        // Another item is still fair game.
        let other = items
            .make_setup_common(1, None, Vec3::ZERO, Vec3::ZERO, &core::iter::empty)
            .unwrap();
        let item = items.get_mut(other).unwrap();
        item.pos = Vec3::new(0.0, 1050.0, 0.0);
        pool.hit_item(item, crate::item::ITEM_RECORD_BASE + other, &mut emit);
        assert_eq!(item.damage_queue, 12);
    }
    #[test]
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
    fn thunder_trail_scaled_offsets_and_expiration_match_source() {
        let mut t = ThunderTrail::new(ThunderHead::new(
            spawn(WeaponKind::PikachuThunder, 0.0, 0.0),
            1,
        ));
        for _ in 0..4 {
            assert!(t.tick(&mut crate::wpeffect::Emit::default()));
        }
        assert_eq!(t.lifetime, 6);
        assert!(!t.tick(&mut crate::wpeffect::Emit::default()));
        // Defender root-sphere centre is y=160, radius160. ±120 scaled
        // boxes of radius200 reach centre<=480; unscaled ±240 would hit550.
        let mut pool = WeaponPool::default();
        pool.insert(
            Weapon::Trail(ThunderTrail::new(ThunderHead::new(
                spawn(WeaponKind::PikachuThunder, 0.0, 0.0),
                1,
            ))),
            crate::stale::WeaponStale::FRESH,
            0,
            1.0,
        );
        let mut f = Fighter::new(FighterKind::Mario, 1, 3);
        f.pos.y = 400.0;
        pool.apply_hits(&mut f);
        crate::combat::resolve(&mut f);
        assert_eq!(f.damage, 0);
        f.pos.y = 0.0;
        pool.apply_hits(&mut f);
        crate::combat::resolve(&mut f);
        assert_eq!(f.damage, 12);
    }
    #[test]
    fn self_collision_removes_head_without_stopping_existing_trails() {
        let mut f = Fighter::new(FighterKind::Pikachu, 0, 3);
        crate::pikachu::set_special_lw(&mut f);
        f.status.set_time(24.0);
        f.pikachu.map_bound_top = Some(1000.0);
        crate::pikachu::update(&mut f);
        let mut pool = WeaponPool::default();
        pool.spawn(f.take_weapon_spawn().unwrap());
        pool.observe_owner(&f);
        pool.tick(|| [], None);
        pool.sync_owner(&mut f);
        crate::pikachu::update(&mut f);
        assert!(f.pikachu.thunder_collide);
        pool.observe_owner(&f);
        pool.tick(|| [], None);
        assert_eq!(pool.thunder_heads().count(), 0);
        assert!(pool.thunder_trails().next().is_some());
    }

    /// `wpPikachuThunderHeadSetDestroy` runs only in the head's
    /// `proc_update`, `proc_map` and `proc_dead`, all inside
    /// `wpProcessProcWeaponMain`: the head has no attack (attack state
    /// off), so no hit callback sets it. The owner sees it after that
    /// frame's weapon tick, as Pikachu's next update reads it on the N64.
    #[test]
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
    fn a_head_meeting_the_floor_tells_pikachu_after_that_weapon_tick() {
        let mut f = Fighter::new(FighterKind::Pikachu, 0, 3);
        crate::pikachu::set_special_lw(&mut f);
        f.status.set_time(24.0);
        f.pikachu.map_bound_top = Some(20_000.0);
        crate::pikachu::update(&mut f);
        let s = f.take_weapon_spawn().unwrap();
        let y = (s.position.y - 700.0) as i16;
        let floor = surface(MapSurfaceKind::Floor, 1, (-4000, y, 0), (4000, y, 1));
        let mut pool = WeaponPool::default();
        pool.spawn(s);
        pool.observe_owner(&f);
        pool.tick(|| [floor], None);
        pool.sync_owner(&mut f);
        assert_eq!(pool.thunder_heads().count(), 1);
        assert!(!f.pikachu.thunder_destroyed);
        pool.tick(|| [floor], None);
        assert_eq!(pool.thunder_heads().count(), 0);
        pool.sync_owner(&mut f);
        assert!(f.pikachu.thunder_destroyed);
    }

    #[test]
    fn thunder_damage_callback_survives_damage_status_and_suppresses_notification() {
        let mut f = Fighter::new(FighterKind::Pikachu, 0, 3);
        crate::pikachu::set_special_lw(&mut f);
        f.status.set_time(24.0);
        f.pikachu.map_bound_top = Some(20_000.0);
        crate::pikachu::update(&mut f);
        let mut pool = WeaponPool::default();
        pool.spawn(f.take_weapon_spawn().unwrap());
        crate::pikachu::on_damage(&mut f);
        f.status.status = crate::status::Status::DamageFlyN.into();
        pool.observe_owner(&f);
        for _ in 0..40 {
            pool.tick(|| [], None);
        }
        assert_eq!(pool.thunder_heads().count(), 0);
        assert!(!pool.thunder_destroyed[0]);
        crate::pikachu::set_special_lw(&mut f);
        assert!(!f.pikachu.thunder_damage);
        // The source clears proc_damage for End.
        f.status.status =
            crate::status::AnyStatus::Pikachu(crate::status::PikachuStatus::SpecialLwEnd);
        crate::pikachu::on_damage(&mut f);
        assert!(!f.pikachu.thunder_damage);
    }
}
