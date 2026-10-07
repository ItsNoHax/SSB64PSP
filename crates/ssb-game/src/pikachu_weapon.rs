//! `wppikachuthunderjolt.c` / `wppikachuthunder.c` (US).
use super::*;

const JOLT_COLL: BodyColl = BodyColl {
    top: 50.0,
    center: 0.0,
    bottom: -50.0,
    width: 50.0,
};
const HEAD_COLL: BodyColl = BodyColl {
    top: 100.0,
    center: 0.0,
    bottom: -100.0,
    width: 50.0,
};
const JOLT_AIR: Hitbox = Hitbox {
    damage: 10,
    radius: 100.0,
    offset: Vec3::ZERO,
    angle: 361,
    kb_scale: 30,
    kb_weight: 0,
    kb_base: 50,
    element: crate::combat::Element::Electric,
    shield_damage: 1,
};
const JOLT_GROUND: Hitbox = Hitbox {
    damage: 7,
    radius: 100.0,
    offset: Vec3::ZERO,
    angle: 361,
    kb_scale: 20,
    kb_weight: 0,
    kb_base: 10,
    element: crate::combat::Element::Electric,
    shield_damage: 1,
};
pub(super) const TRAIL_HIT: Hitbox = Hitbox {
    damage: 12,
    radius: 200.0,
    offset: Vec3::ZERO,
    angle: 70,
    kb_scale: 50,
    kb_weight: 0,
    kb_base: 80,
    element: crate::combat::Element::Electric,
    shield_damage: 1,
};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThunderJolt {
    pub owner_port: u8,
    pub position: Vec3,
    pub velocity: Vec3,
    pub lifetime: u16,
    pub damage: i32,
    pub surface: Option<MapSurface>,
    /// `lr`: ±1 on floor; 2 ascending, 3 descending on wall.
    pub direction: i8,
    normal: Vec2,
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
}

/// `gcSetAllAnimSpeed(new_gobj, 0.5F)` in `wpPikachuThunderJoltGroundMakeWeapon`.
pub const JOLT_GROUND_ANIM_SPEED: f32 = 0.5;
/// `WPPIKACHUJOLT_ANIM_PUSH_FRAME`.
const JOLT_ANIM_PUSH_FRAME: f32 = 7.5;
const DEG_180: f32 = core::f32::consts::PI;
impl ThunderJolt {
    pub(super) fn new(s: WeaponSpawn) -> Self {
        let (sin, cos) = sin_cos(-core::f32::consts::FRAC_PI_4);
        Self {
            owner_port: s.owner_port,
            position: s.position,
            velocity: Vec3::new(cos * 40.0 * s.facing, sin * 40.0, 0.0),
            lifetime: 100,
            damage: 10,
            surface: None,
            direction: if s.facing < 0.0 { -1 } else { 1 },
            normal: Vec2::ZERO,
            anim_epoch: 0,
            anim_ticks: 0,
            anim_frame: 0.0,
            anim_fresh: true,
            model_rotate_y: 0.0,
        }
    }
    /// The ground weapon's root `rotate.z`: `wpPikachuThunderJoltGroundProcMap`
    /// sets it to `atan2(-angle.x, angle.y)` of the line it rides.
    pub fn rotate_z(&self) -> f32 {
        ssb_engine::math::atan2(-self.normal.x, self.normal.y)
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
        if self.surface.is_some() {
            // Ground attribute offset (0,100,0), rotated with the surface.
            (
                JOLT_GROUND,
                self.position + Vec3::new(self.normal.x * 100.0, self.normal.y * 100.0, 0.0),
            )
        } else {
            (JOLT_AIR, self.position)
        }
    }
    pub(super) fn reflect(&mut self, f: &Fighter) {
        self.owner_port = f.port;
        self.lifetime = 100;
        if self.velocity.x * f.facing.sign() < 0.0 {
            self.velocity.x = -self.velocity.x;
        }
        if self.surface.is_some() {
            self.direction = if self.velocity.x >= 0.0 { 1 } else { -1 };
            // `wpPikachuThunderJoltGroundProcReflector`.
            self.model_rotate_y = if self.velocity.x >= 0.0 { DEG_180 } else { 0.0 };
        }
        self.damage = ((self.damage as f32 * 1.8 + 0.99) as i32).min(100);
    }
    fn attach(&mut self, s: MapSurface, pos: Vec3, direction: i8) {
        self.surface = Some(s);
        self.position = pos;
        self.normal = surface_normal(s.kind, s.segment);
        self.direction = direction;
    }
    /// `wpPikachuThunderJoltAirProcUpdate`/`...GroundProcUpdate` and their
    /// `proc_map`. Every end makes a small dust cloud where the jolt is.
    pub(super) fn tick<I, F>(&mut self, surfaces: F, fx: &mut Emit) -> bool
    where
        F: Fn() -> I + Copy,
        I: IntoIterator<Item = MapSurface>,
    {
        let alive = self.tick_map(surfaces);
        if !alive {
            fx.push(Fx::DustExpandSmall(self.position));
        }
        alive
    }
    fn tick_map<I, F>(&mut self, surfaces: F) -> bool
    where
        F: Fn() -> I + Copy,
        I: IntoIterator<Item = MapSurface>,
    {
        // `wpProcessProcWeaponMain` plays the DObj animation before
        // `proc_update`. The first play after `gcAddAnimAll` only parses.
        self.anim_ticks = self.anim_ticks.wrapping_add(1);
        if core::mem::take(&mut self.anim_fresh) {
        } else if self.surface.is_some() {
            self.anim_frame += JOLT_GROUND_ANIM_SPEED;
        }
        // `wpPikachuThunderJoltGroundProcUpdate`, ahead of the lifetime.
        if self.surface.is_some() && self.anim_frame == JOLT_ANIM_PUSH_FRAME {
            self.restart_anim();
        }
        self.lifetime = self.lifetime.saturating_sub(1);
        if self.lifetime == 0 {
            return false;
        }
        if self.surface.is_none() {
            self.velocity.y = self.velocity.y.max(-50.0);
            let wanted = self.position + self.velocity;
            // `wpMapTestAllCheckFloor`, then LWALL, then RWALL. Ceiling does
            // not convert the aerial jolt into a crawler.
            for kind in [
                MapSurfaceKind::Floor,
                MapSurfaceKind::LeftWall,
                MapSurfaceKind::RightWall,
            ] {
                let hit = map_contact(
                    surfaces().into_iter().filter(|s| s.kind == kind),
                    self.position,
                    wanted,
                    JOLT_COLL,
                );
                if let Some(hit) = hit {
                    let s = hit.surface;
                    let pos = project(s, hit.position).unwrap_or(hit.position);
                    let angle = ssb_engine::math::atan2(hit.normal.y, hit.normal.x);
                    let direction = match kind {
                        MapSurfaceKind::Floor => {
                            if self.velocity.x >= 0.0 {
                                1
                            } else {
                                -1
                            }
                        }
                        MapSurfaceKind::LeftWall => {
                            if angle > core::f32::consts::PI * 0.75 {
                                3
                            } else {
                                2
                            }
                        }
                        MapSurfaceKind::RightWall => {
                            if angle > core::f32::consts::FRAC_PI_4 {
                                2
                            } else {
                                3
                            }
                        }
                        _ => unreachable!(),
                    };
                    self.damage = 7;
                    self.attach(s, pos, direction);
                    // `wpPikachuThunderJoltGroundMakeWeapon`: a new weapon,
                    // whose `wpManagerMakeWeapon` adds the ground animation.
                    self.anim_epoch = self.anim_epoch.wrapping_add(1);
                    self.anim_ticks = 0;
                    self.anim_frame = 0.0;
                    self.anim_fresh = true;
                    self.model_rotate_y = match (kind, direction) {
                        (MapSurfaceKind::Floor, d) if d >= 0 => DEG_180,
                        (MapSurfaceKind::Floor, _) => 0.0,
                        (MapSurfaceKind::LeftWall, 3) | (MapSurfaceKind::RightWall, 2) => 0.0,
                        _ => DEG_180,
                    };
                    return true;
                }
            }
            self.position = map_contact(
                surfaces()
                    .into_iter()
                    .filter(|s| s.kind == MapSurfaceKind::Ceiling),
                self.position,
                wanted,
                JOLT_COLL,
            )
            .map_or(wanted, |hit| hit.position);
            return true;
        }
        let Some(current) = refresh_surface(surfaces(), self.surface.unwrap()) else {
            return false;
        };
        self.surface = Some(current);
        self.velocity.x = self.normal.y * 55.0;
        self.velocity.y = -self.normal.x * 55.0;
        if current.kind == MapSurfaceKind::Floor {
            self.velocity.x *= self.direction as f32;
        } else if self.direction == 2 {
            self.velocity.y = self.velocity.y.abs();
        } else {
            self.velocity.y = -self.velocity.y.abs();
        }
        let from = self.position;
        let wanted = from + self.velocity;
        let line = current.topology.map(|t| t.line);
        // The source projects against every segment of the current line.
        let on_line = surfaces()
            .into_iter()
            .filter(|s| same_line(*s, current))
            .find_map(|s| project(s, wanted).map(|pos| (s, pos)));
        if on_line.is_none() {
            if let Some(alive) = self.edge_transition(surfaces, current, from) {
                return alive;
            }
        }
        let projected = on_line.map_or(wanted, |(_, p)| p);
        // `GetStatus`: incoming floor/wall or ceiling crossing can supersede
        // the projected current line. Exclude the authored opposite edge.
        let collision_kind = match current.kind {
            MapSurfaceKind::Floor => {
                if self.direction == 1 {
                    MapSurfaceKind::LeftWall
                } else {
                    MapSurfaceKind::RightWall
                }
            }
            _ => {
                if self.direction == 3 {
                    MapSurfaceKind::Floor
                } else {
                    MapSurfaceKind::Ceiling
                }
            }
        };
        let excluded = if collision_kind == MapSurfaceKind::Ceiling {
            None
        } else {
            endpoint(
                surfaces,
                current,
                if current.kind == MapSurfaceKind::Floor {
                    self.direction < 0
                } else {
                    true
                },
            )
            .and_then(|(_, v)| neighbor(surfaces, line, v))
        };
        let collision = surfaces()
            .into_iter()
            .filter(|s| s.kind == collision_kind)
            .filter_map(|s| {
                let t = swept_coords_intersection(
                    Vec2::new(from.x, from.y),
                    Vec2::new(projected.x, projected.y),
                    s.coords(),
                )?;
                let n = surface_normal(s.kind, s.segment);
                if (projected.x - from.x) * n.x + (projected.y - from.y) * n.y >= 0.0 {
                    return None;
                }
                Some((
                    t,
                    s,
                    Vec3::new(
                        from.x + (projected.x - from.x) * t,
                        from.y + (projected.y - from.y) * t,
                        from.z,
                    ),
                ))
            })
            .min_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(core::cmp::Ordering::Equal))
            // Source chooses the closest crossing before excluding its line.
            .filter(|(_, s, _)| excluded.is_none() || s.topology.map(|t| t.line) != excluded);
        if let Some((_, s, pos)) = collision {
            if s.kind == MapSurfaceKind::Ceiling {
                return false;
            }
            let dir = if s.kind == MapSurfaceKind::Floor {
                if current.kind == MapSurfaceKind::LeftWall {
                    -1
                } else {
                    1
                }
            } else {
                2
            };
            self.attach(s, pos, dir);
            self.restart_anim();
            return true;
        }
        if let Some((s, pos)) = on_line {
            self.attach(s, pos, self.direction);
            return true;
        }
        false
    }
    fn edge_transition<I, F>(
        &mut self,
        surfaces: F,
        current: MapSurface,
        from: Vec3,
    ) -> Option<bool>
    where
        F: Fn() -> I,
        I: IntoIterator<Item = MapSurface>,
    {
        let line = current.topology.map(|t| t.line);
        // Exact endpoint identity, normalized by X (floor) / Y (wall).
        // Missing topology ends the crawler rather than inventing a join.
        let positive = if current.kind == MapSurfaceKind::Floor {
            self.direction == 1
        } else {
            self.direction == 2
        };
        let (_, vertex) = endpoint(&surfaces, current, positive)?;
        let next_line = neighbor(&surfaces, line, vertex)?;
        let s = surfaces().into_iter().find(|s| {
            s.topology.is_some_and(|t| {
                t.line == next_line && (t.vertex1 == vertex || t.vertex2 == vertex)
            })
        })?;
        let dir = match (current.kind, self.direction, s.kind) {
            (MapSurfaceKind::Floor, 1, MapSurfaceKind::LeftWall) => 2,
            (MapSurfaceKind::Floor, 1, MapSurfaceKind::RightWall) => 3,
            (MapSurfaceKind::Floor, -1, MapSurfaceKind::LeftWall) => 3,
            (MapSurfaceKind::Floor, -1, MapSurfaceKind::RightWall) => 2,
            (MapSurfaceKind::LeftWall, 2, MapSurfaceKind::Floor) => 1,
            (MapSurfaceKind::LeftWall, _, MapSurfaceKind::Floor) => -1,
            (MapSurfaceKind::RightWall, 2, MapSurfaceKind::Floor) => -1,
            (MapSurfaceKind::RightWall, _, MapSurfaceKind::Floor) => 1,
            _ => return None,
        };
        let n = surface_normal(s.kind, s.segment);
        if n.x * self.normal.x + n.y * self.normal.y
            < sin_cos(100.0 * core::f32::consts::PI / 180.0).1
        {
            return Some(false);
        }
        let t = s.topology.unwrap();
        let [x1, y1, x2, y2] = s.coords();
        let pos = if t.vertex1 == vertex {
            Vec3::new(x1, y1, from.z)
        } else {
            Vec3::new(x2, y2, from.z)
        };
        self.attach(s, pos, dir);
        self.restart_anim();
        Some(true)
    }
}
fn same_line(a: MapSurface, b: MapSurface) -> bool {
    match (a.topology, b.topology) {
        (Some(a), Some(b)) => a.line == b.line,
        _ => a == b,
    }
}
fn project(s: MapSurface, p: Vec3) -> Option<Vec3> {
    let [x1, y1, x2, y2] = s.coords();
    if matches!(s.kind, MapSurfaceKind::Floor | MapSurfaceKind::Ceiling) {
        if p.x < x1.min(x2) - 0.001 || p.x > x1.max(x2) + 0.001 || x1 == x2 {
            return None;
        }
        Some(Vec3::new(p.x, y1 + (p.x - x1) * (y2 - y1) / (x2 - x1), p.z))
    } else {
        if p.y < y1.min(y2) - 0.001 || p.y > y1.max(y2) + 0.001 || y1 == y2 {
            return None;
        }
        Some(Vec3::new(x1 + (p.y - y1) * (x2 - x1) / (y2 - y1), p.y, p.z))
    }
}
fn endpoint<I, F>(surfaces: F, current: MapSurface, positive: bool) -> Option<(Vec2, u16)>
where
    F: Fn() -> I,
    I: IntoIterator<Item = MapSurface>,
{
    let floor = matches!(
        current.kind,
        MapSurfaceKind::Floor | MapSurfaceKind::Ceiling
    );
    let mut ends = [None, None];
    for s in surfaces().into_iter().filter(|s| same_line(*s, current)) {
        let Some(t) = s.topology else { continue };
        let [x1, y1, x2, y2] = s.coords();
        if t.point == 0 {
            ends[0] = Some((Vec2::new(x1, y1), t.vertex1));
        }
        if t.point + 1 == t.segments {
            ends[1] = Some((Vec2::new(x2, y2), t.vertex2));
        }
    }
    let (a, b) = (ends[0]?, ends[1]?);
    let first_lower = if floor { a.0.x < b.0.x } else { a.0.y < b.0.y };
    Some(if first_lower == positive { b } else { a })
}
fn neighbor<I, F>(surfaces: F, line: Option<u16>, vertex: u16) -> Option<u16>
where
    F: Fn() -> I,
    I: IntoIterator<Item = MapSurface>,
{
    // `func_ovl2_800FB31C`: ascending original line IDs, last match wins.
    surfaces()
        .into_iter()
        .filter_map(|s| s.topology)
        .filter(|t| {
            Some(t.line) != line
                && ((t.point == 0 && t.vertex1 == vertex)
                    || (t.point + 1 == t.segments && t.vertex2 == vertex))
        })
        .map(|t| t.line)
        .max()
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
        let wanted = self.position + Vec3::new(0.0, -450.0, 0.0);
        if let Some(hit) = map_contact(surfaces(), self.position, wanted, HEAD_COLL) {
            self.position = hit.position;
            // `wpPikachuThunderHeadProcMap`.
            fx.push(Fx::Quake(1));
            fx.push(Fx::SparkleWhite(self.position));
            return false;
        }
        self.position = wanted;
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
    #[test]
    fn air_jolt_converts_with_remaining_life_and_ground_damage() {
        let mut j = ThunderJolt::new(spawn(WeaponKind::PikachuThunderJolt, 0.0, 70.0));
        let floor = [surface(
            MapSurfaceKind::Floor,
            0,
            (-1000, 0, 0),
            (1000, 0, 1),
        )];
        assert!(j.tick(|| floor, &mut crate::wpeffect::Emit::default()));
        assert_eq!(j.lifetime, 99);
        assert_eq!(j.position.y, 0.0);
        assert_eq!(j.hit().0.damage, 7);
        assert_eq!(j.hit().1.y, 100.0);
        let x = j.position.x;
        assert!(j.tick(|| floor, &mut crate::wpeffect::Emit::default()));
        assert_eq!(j.position.x - x, 55.0);
    }
    #[test]
    fn ground_jolt_restarts_its_animation_every_push_frame() {
        let mut j = ThunderJolt::new(spawn(WeaponKind::PikachuThunderJolt, 0.0, 70.0));
        let floor = [surface(
            MapSurfaceKind::Floor,
            0,
            (-10000, 0, 0),
            (10000, 0, 1),
        )];
        assert!(j.tick(|| floor, &mut crate::wpeffect::Emit::default()));
        // A new ground weapon: its animation is added, not yet played.
        assert_eq!((j.anim_epoch, j.anim_ticks), (1, 0));
        assert_eq!(j.model_rotate_y, core::f32::consts::PI);
        assert_eq!(j.rotate_z(), 0.0);
        // The first play only parses; each later one adds 0.5, and
        // `anim_frame == 7.5` restarts it with one play.
        for ticks in 1..=15 {
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
        assert!(j.surface.is_none());
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
        j.attach(floor, j.position, 1);
        assert!(j.tick(|| [floor, wall], &mut crate::wpeffect::Emit::default()));
        assert_eq!(j.surface.unwrap().kind, MapSurfaceKind::RightWall);
        assert_eq!(j.direction, 3);
        assert_eq!(j.position, Vec3::new(200.0, 0.0, 0.0));
        assert!(j.tick(|| [floor, wall], &mut crate::wpeffect::Emit::default()));
        assert_eq!(j.position.y, -55.0);
        // Reflector's wpMainVelSetLR stores ±1 even on walls. Update treats
        // every value other than 2 as downward; it does not invent direction 3.
        j.reflect(&Fighter::new(FighterKind::Fox, 2, 3));
        assert_eq!(j.direction, 1);
        assert!(j.tick(|| [floor, wall], &mut crate::wpeffect::Emit::default()));
        assert_eq!(j.position.y, -110.0);
        // Same coordinates, distinct original vertex IDs: no join exists.
        let mut disconnected = wall;
        disconnected.topology.as_mut().unwrap().vertex1 = 99;
        let mut j = ThunderJolt::new(spawn(WeaponKind::PikachuThunderJolt, 180.0, 0.0));
        j.attach(floor, j.position, 1);
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
        assert_eq!(neighbor(|| [ceiling, floor, wall], Some(3), 6), Some(7));
        let mut j = ThunderJolt::new(spawn(WeaponKind::PikachuThunderJolt, 180.0, 0.0));
        j.attach(floor, j.position, 1);
        assert!(!j.tick(
            || [ceiling, floor, wall],
            &mut crate::wpeffect::Emit::default()
        ));
    }
    #[test]
    fn descending_wall_ceiling_contact_destroys_jolt() {
        let wall = surface(MapSurfaceKind::LeftWall, 1, (0, -400, 0), (0, 400, 1));
        let ceiling = surface(MapSurfaceKind::Ceiling, 2, (-100, 50, 2), (100, 50, 3));
        let mut j = ThunderJolt::new(spawn(WeaponKind::PikachuThunderJolt, 0.0, 25.0));
        j.attach(wall, j.position, 2);
        assert!(!j.tick(|| [wall, ceiling], &mut crate::wpeffect::Emit::default()));
    }
    #[test]
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
    #[test]
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
