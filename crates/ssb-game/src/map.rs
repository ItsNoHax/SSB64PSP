//! Fighter map processing from `mpcommon.c` and `mpprocess.c`.
//! Queries retain the source order: LWall, RWall, ceiling, floor, cliff.
//! Moving groups retain local integer vertices and float translations.
//! Previous collision diamonds are explicit when the source copies collision state.

use crate::collision::{self, Segment};
use crate::fighter::{Facing, Fighter};
use crate::ground::{self, BodyColl, Moved, Standing};
use crate::status::{
    self, AnyStatus, CaptainStatus, FoxStatus, KirbyStatus, NessStatus, PikachuStatus, PurinStatus,
    Status,
};
use crate::weapon::{MapSurface, MapSurfaceKind as Kind, SurfaceTopology};
use ssb_engine::math::{Vec2, Vec3};

#[cfg(test)]
#[path = "map_tests.rs"]
mod tests;

/// `MPYakumonoStatus`, independent of render-node visibility.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GroupStatus {
    #[default]
    None,
    On,
    Show,
    Off,
    Hidden,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct MapGroup {
    pub status: GroupStatus,
    pub animated: bool,
    pub translate: Vec3,
    pub speed: Vec3,
}

impl MapGroup {
    pub fn exists(self) -> bool {
        !matches!(self.status, GroupStatus::Off | GroupStatus::Hidden)
    }
    pub fn motion(self) -> Option<crate::weapon::SurfaceMotion> {
        (self.animated || self.status != GroupStatus::None).then_some(
            crate::weapon::SurfaceMotion {
                offset: Vec2::new(self.translate.x, self.translate.y),
                speed: self.speed,
            },
        )
    }
    /// `mpCollisionSetYakumonoPosID`: a direct stage-script movement.
    pub fn set_position(&mut self, pos: Vec3) {
        self.speed = pos - self.translate;
        self.translate = pos;
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Contact {
    pub line: u16,
    pub flags: u16,
    pub normal: Vec2,
}

/// Source current contacts, retained across frames for first-contact callbacks.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Contacts {
    pub floor: bool,
    pub left_wall: Option<Contact>,
    pub right_wall: Option<Contact>,
    pub ceiling: Option<Contact>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CliffQuery {
    pub facing: f32,
    pub wait: u16,
    pub reach: Vec2,
    pub occupied: Option<(u16, f32)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct AirOptions {
    pub ignore_line: Option<u16>,
    pub skip_pass: bool,
    pub cliff: Option<CliffQuery>,
    /// `MAP_PROC_TYPE_CEILHEAVY` with `vel_air.y >= 30`
    /// ([`CEILHEAVY_VEL_Y_MIN`]): a ceiling contact stops the sweep and
    /// reports [`AirMoved::ceil_stop`].
    pub ceil_heavy: bool,
}

/// `mpCommonRunFighterSpecialCollisions`: the rise speed at which a ceiling
/// contact counts as a heavy bonk (`ftCommonStopCeilSetStatus`).
pub const CEILHEAVY_VEL_Y_MIN: f32 = 30.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AirMoved {
    pub moved: Moved,
    pub contacts: Contacts,
    pub cliff: Option<(u16, Vec2)>,
    /// `mask_curr & MAP_FLAG_CEILHEAVY`.
    pub ceil_stop: bool,
}

pub fn floors<I>(surfaces: I) -> impl Iterator<Item = (u16, Segment)>
where
    I: IntoIterator<Item = MapSurface>,
{
    surfaces
        .into_iter()
        .enumerate()
        .filter_map(|(i, s)| (s.kind == Kind::Floor).then_some((line_id(i, s), s.segment)))
}

/// Compatibility adapter for host fixtures that provide floors only.
pub fn floor_surface((line, segment): (u16, Segment)) -> MapSurface {
    MapSurface {
        motion: None,
        kind: Kind::Floor,
        segment,
        topology: Some(SurfaceTopology {
            line,
            point: 0,
            segments: 1,
            vertex1: u16::MAX,
            vertex2: u16::MAX,
        }),
    }
}

fn line_id(index: usize, s: MapSurface) -> u16 {
    s.topology.map_or(index as u16, |t| t.line)
}

fn contact(index: usize, s: MapSurface) -> Contact {
    Contact {
        line: line_id(index, s),
        flags: s.segment.flags,
        normal: crate::weapon::surface_normal(s.kind, s.segment),
    }
}

fn point(pos: Vec3, offset: Vec2) -> Vec2 {
    Vec2::new(pos.x + offset.x, pos.y + offset.y)
}

/// Axis transforms of the source FC/LR one-sided tests. A ceiling is a
/// reflected floor; walls exchange X/Y with the same signed-side convention.
fn transform(kind: Kind, p: Vec2) -> Vec2 {
    match kind {
        Kind::Floor => p,
        Kind::Ceiling => Vec2::new(p.x, -p.y),
        Kind::LeftWall => Vec2::new(p.y, -p.x),
        Kind::RightWall => Vec2::new(p.y, p.x),
    }
}

fn transformed(kind: Kind, s: MapSurface) -> [f32; 4] {
    let [x1, y1, x2, y2] = s.coords();
    let a = transform(kind, Vec2::new(x1, y1));
    let b = transform(kind, Vec2::new(x2, y2));
    [a.x, a.y, b.x, b.y]
}

fn query<I, F>(surfaces: &F, kind: Kind, from: Vec2, to: Vec2) -> Option<(Contact, Vec2)>
where
    F: Fn() -> I,
    I: IntoIterator<Item = MapSurface>,
{
    query_at(surfaces, kind, from, to, false)
}

fn sweep<I, F>(surfaces: &F, kind: Kind, from: Vec2, to: Vec2) -> Option<(Contact, Vec2)>
where
    F: Fn() -> I,
    I: IntoIterator<Item = MapSurface>,
{
    query_at(surfaces, kind, from, to, true)
}

fn query_at<I, F>(
    surfaces: &F,
    kind: Kind,
    from: Vec2,
    to: Vec2,
    moving: bool,
) -> Option<(Contact, Vec2)>
where
    F: Fn() -> I,
    I: IntoIterator<Item = MapSurface>,
{
    let b = transform(kind, to);
    let mut best = None;
    let mut distance = f32::MAX;
    for (i, s) in surfaces().into_iter().enumerate() {
        if s.kind != kind {
            continue;
        }
        let speed = if moving {
            s.motion.map_or(Vec3::ZERO, |m| m.speed)
        } else {
            Vec3::ZERO
        };
        let a = transform(kind, Vec2::new(from.x + speed.x, from.y + speed.y));
        let Some(p) = collision::check_floor_coords(transformed(kind, s), a, b) else {
            continue;
        };
        let d = (p.y - a.y).abs();
        if d >= distance {
            continue;
        }
        distance = d;
        let p = match kind {
            Kind::Floor => p,
            Kind::Ceiling => Vec2::new(p.x, -p.y),
            Kind::LeftWall => Vec2::new(-p.y, p.x),
            Kind::RightWall => Vec2::new(p.y, p.x),
        };
        best = Some((contact(i, s), p));
    }
    best
}

fn height<I, F>(surfaces: &F, kind: Kind, line: u16, x: f32) -> Option<(f32, Contact)>
where
    F: Fn() -> I,
    I: IntoIterator<Item = MapSurface>,
{
    let offset = surfaces()
        .into_iter()
        .enumerate()
        .find(|(i, s)| s.kind == kind && line_id(*i, *s) == line)?
        .1
        .motion
        .map_or(Vec2::ZERO, |m| m.offset);
    let f = collision::floor_height(
        surfaces().into_iter().enumerate().filter_map(|(i, s)| {
            (s.kind == kind && line_id(i, s) == line).then_some((line, s.segment))
        }),
        x - offset.x,
    )?;
    let normal = if kind == Kind::Ceiling {
        Vec2::new(-f.normal.x, -f.normal.y)
    } else {
        f.normal
    };
    Some((
        f.y + offset.y,
        Contact {
            line,
            flags: f.flags,
            normal,
        },
    ))
}

/// Endpoint selection is over the entire polyline, never its first segment.
fn edge<I, F>(surfaces: &F, kind: Kind, line: u16, high: bool) -> Option<(Vec2, u16)>
where
    F: Fn() -> I,
    I: IntoIterator<Item = MapSurface>,
{
    let mut found: Option<(Vec2, u16)> = None;
    for (i, s) in surfaces().into_iter().enumerate() {
        if s.kind != kind || line_id(i, s) != line {
            continue;
        }
        let ids = s.topology.map_or([u16::MAX; 2], |t| [t.vertex1, t.vertex2]);
        let [x1, y1, x2, y2] = s.coords();
        for (p, id) in [(Vec2::new(x1, y1), ids[0]), (Vec2::new(x2, y2), ids[1])] {
            let axis = |v: Vec2| {
                if matches!(kind, Kind::Floor | Kind::Ceiling) {
                    v.x
                } else {
                    v.y
                }
            };
            if found.is_none_or(|(old, _)| {
                if high {
                    axis(p) > axis(old)
                } else {
                    axis(p) < axis(old)
                }
            }) {
                found = Some((p, id));
            }
        }
    }
    found
}

/// Original neighbor builder: shared vertex IDs, last matching line wins.
fn neighbor<I, F>(surfaces: &F, kind: Kind, line: u16, high: bool) -> Option<(Kind, u16)>
where
    F: Fn() -> I,
    I: IntoIterator<Item = MapSurface>,
{
    let (_, vertex) = edge(surfaces, kind, line, high)?;
    if vertex == u16::MAX {
        return None;
    }
    let mut result = None;
    for (i, s) in surfaces().into_iter().enumerate() {
        let Some(t) = s.topology else { continue };
        if t.line != line
            && (t.vertex1 == vertex || t.vertex2 == vertex)
            && result.is_none_or(|(_, old)| t.line > old)
        {
            result = Some((s.kind, line_id(i, s)));
        }
    }
    result
}

/// Five distinct wall lines, in probe order (`sMPProcessMultiWallCollideLineIDs`).
fn walls<I, F>(
    surfaces: &F,
    coll: BodyColl,
    previous: BodyColl,
    from: Vec3,
    to: Vec3,
    kind: Kind,
    ground_line: Option<u16>,
) -> [Option<u16>; 5]
where
    F: Fn() -> I,
    I: IntoIterator<Item = MapSurface>,
{
    let side = if kind == Kind::LeftWall { 1.0 } else { -1.0 };
    let waist = Vec2::new(side * coll.width, coll.center);
    let bottom = Vec2::new(0.0, coll.bottom);
    let top = Vec2::new(0.0, coll.top);
    let prev_waist = Vec2::new(side * previous.width, previous.center);
    let prev_bottom = Vec2::new(0.0, previous.bottom);
    let prev_top = Vec2::new(0.0, previous.top);
    let mut lines = [None; 5];
    let exclude = ground_line
        .and_then(|line| neighbor(surfaces, Kind::Floor, line, kind == Kind::RightWall))
        .map(|(_, line)| line);
    let mut add = |line| {
        if Some(line) == exclude {
            return;
        }
        if !lines.contains(&Some(line)) {
            if let Some(slot) = lines.iter_mut().find(|s| s.is_none()) {
                *slot = Some(line);
            }
        }
    };
    for (probe, (a, b)) in [
        (point(from, prev_waist), point(to, waist)),
        (point(from, prev_bottom), point(to, bottom)),
        (point(from, prev_top), point(to, top)),
        (point(to, bottom), point(to, waist)),
        (point(to, top), point(to, waist)),
    ]
    .into_iter()
    .enumerate()
    {
        if let Some((hit, _)) = query_at(surfaces, kind, a, b, probe < 3) {
            add(hit.line);
        }
    }
    if ground_line.is_some() {
        return lines;
    }
    // A waist sweep across a neighboring ceiling/floor can touch a wall
    // even when none of the three vertical probes crosses the wall itself.
    for (other, tip) in [(Kind::Ceiling, top), (Kind::Floor, bottom)] {
        if let Some((hit, _)) = sweep(surfaces, other, point(from, waist), point(to, waist)) {
            if other == Kind::Floor && hit.flags & collision::flags::PASS != 0 {
                continue;
            }
            let high = kind == Kind::RightWall;
            if let Some((neighbor_kind, line)) = neighbor(surfaces, other, hit.line, high) {
                if neighbor_kind == kind
                    && sweep(surfaces, other, point(from, tip), point(to, tip))
                        .is_none_or(|(h, _)| h.line != hit.line)
                    && query(surfaces, other, point(to, tip), point(to, waist))
                        .is_none_or(|(h, _)| h.line != hit.line)
                {
                    add(line);
                }
            }
        }
    }
    lines
}

/// `mpProcessRun[L/R]WallCollisionAdjNew`: choose the most restrictive X
/// from bottom/waist/top surface samples and every vertex inside the diamond.
fn correct_wall<I, F>(
    surfaces: &F,
    coll: BodyColl,
    pos: &mut Vec3,
    kind: Kind,
    lines: [Option<u16>; 5],
) -> Option<Contact>
where
    F: Fn() -> I,
    I: IntoIterator<Item = MapSurface>,
{
    let side = if kind == Kind::LeftWall { 1.0 } else { -1.0 };
    let mut best = if side > 0.0 { 65536.0 } else { -65536.0 };
    let mut hit = None;
    for line in lines.into_iter().flatten() {
        let (upper, _) = edge(surfaces, kind, line, true)?;
        let (lower, _) = edge(surfaces, kind, line, false)?;
        for (i, s) in surfaces().into_iter().enumerate() {
            if s.kind != kind || line_id(i, s) != line {
                continue;
            }
            let mut consider = |x: f32| {
                if side * x < side * best {
                    best = x;
                    hit = Some(contact(i, s));
                }
            };
            if upper.y < pos.y + coll.bottom {
                consider(upper.x);
                continue;
            }
            if pos.y + coll.top < lower.y {
                consider(lower.x);
                continue;
            }
            let [x1, y1, x2, y2] = s.coords();
            for (offset, width) in [
                (coll.bottom, 0.0),
                (coll.center, coll.width),
                (coll.top, 0.0),
            ] {
                let y = pos.y + offset;
                let lo = (y1).min(y2);
                let hi = (y1).max(y2);
                if y >= lo - 0.001 && y <= hi + 0.001 && y1 != y2 {
                    let y = y.clamp(lo, hi);
                    let x = x1 + (y - y1) * (x2 - x1) / (y2 - y1);
                    consider(x - side * width);
                }
            }
            for (x, y) in [(x1, y1), (x2, y2)] {
                let y = y - pos.y;
                let width = if y >= coll.bottom && y <= coll.center && coll.center != coll.bottom {
                    Some((y - coll.bottom) * coll.width / (coll.center - coll.bottom))
                } else if y >= coll.center && y <= coll.top && coll.top != coll.center {
                    Some((coll.top - y) * coll.width / (coll.top - coll.center))
                } else {
                    None
                };
                if let Some(width) = width {
                    consider(x - side * width);
                }
            }
        }
    }
    if hit.is_some() && side * pos.x > side * best {
        pos.x = best;
    }
    hit
}

/// What one substep's ceiling test found: whether the body touched a ceiling
/// (`mpProcessCheckTestCeilCollisionAdjNew`), and the contact recorded once
/// `mpProcessRunCeilCollisionAdjNew` placed it.
struct CeilStep {
    touched: bool,
    /// The ceiling line the sweep crossed (`ceil_line_id`/`ceil_angle`).
    hit: Option<Contact>,
    contact: Option<Contact>,
}

/// The ceiling half of a substep, shared by [`move_air`] and
/// [`move_damage`].
fn ceiling_step<I, F>(
    surfaces: &F,
    coll: BodyColl,
    previous: BodyColl,
    prev: Vec3,
    pos: &mut Vec3,
    current: &Contacts,
) -> CeilStep
where
    F: Fn() -> I,
    I: IntoIterator<Item = MapSurface>,
{
    let mut out = CeilStep {
        touched: false,
        hit: None,
        contact: None,
    };
    let top = Vec2::new(0.0, coll.top);
    let ceil = sweep(
        surfaces,
        Kind::Ceiling,
        point(prev, Vec2::new(0.0, previous.top)),
        point(*pos, top),
    )
    .map(|(h, _)| h)
    .or_else(|| adjacent_horizontal(surfaces, current, Kind::Ceiling, *pos, coll.top));
    if let Some(hit) = ceil {
        out.touched = true;
        out.hit = Some(hit);
        if let Some((y, h)) = height(surfaces, Kind::Ceiling, hit.line, pos.x) {
            pos.y = y - coll.top;
            out.hit = Some(h);
            out.contact = Some(h);
            adjust_edges(surfaces, coll, pos, Kind::Ceiling, h);
        } else if let Some((corner, _)) = edge(
            surfaces,
            Kind::Ceiling,
            hit.line,
            pos.x > edge(surfaces, Kind::Ceiling, hit.line, false).map_or(pos.x, |e| e.0.x),
        ) {
            pos.y = corner.y - coll.top;
            let right = pos.x > corner.x;
            if neighbor(surfaces, Kind::Ceiling, hit.line, right).is_some_and(|(k, _)| {
                k == if right {
                    Kind::LeftWall
                } else {
                    Kind::RightWall
                }
            }) {
                pos.x = corner.x;
                out.contact = Some(hit);
            }
        }
    }
    out
}

/// Static air path of `mpCommonRunFighterSpecialCollisions`.
pub fn move_air<I, F>(
    coll: &BodyColl,
    from: Vec3,
    to: Vec3,
    options: AirOptions,
    surfaces: F,
) -> AirMoved
where
    F: Fn() -> I,
    I: IntoIterator<Item = MapSurface>,
{
    move_air_from_shape(coll, coll, from, to, options, surfaces)
}

/// [`move_air`] with `coll_data.vel_push` (Whispy's wind). The push joins
/// the first substep only and does not count toward the substep split,
/// exactly as `mpProcessUpdateMain` adds it after computing `update_count`.
pub fn move_air_pushed<I, F>(
    coll: &BodyColl,
    from: Vec3,
    to: Vec3,
    push: Vec3,
    options: AirOptions,
    surfaces: F,
) -> AirMoved
where
    F: Fn() -> I,
    I: IntoIterator<Item = MapSurface>,
{
    move_air_inner(coll, coll, from, to, push, options, surfaces)
}

/// `p_map_coll` may name another body's diamond during a copied-data sweep.
/// Ordinary fighter processing aliases it to `map_coll`, including substeps.
pub fn move_air_from_shape<I, F>(
    coll: &BodyColl,
    previous: &BodyColl,
    from: Vec3,
    to: Vec3,
    options: AirOptions,
    surfaces: F,
) -> AirMoved
where
    F: Fn() -> I,
    I: IntoIterator<Item = MapSurface>,
{
    move_air_inner(coll, previous, from, to, Vec3::ZERO, options, surfaces)
}

fn move_air_inner<I, F>(
    coll: &BodyColl,
    previous: &BodyColl,
    from: Vec3,
    to: Vec3,
    push: Vec3,
    options: AirOptions,
    surfaces: F,
) -> AirMoved
where
    F: Fn() -> I,
    I: IntoIterator<Item = MapSurface>,
{
    let steps = ground::substep_count(from, to);
    let step = Vec3::new(
        (to.x - from.x) / steps as f32,
        (to.y - from.y) / steps as f32,
        (to.z - from.z) / steps as f32,
    );
    let mut pos = from;
    let mut contacts = Contacts::default();
    let mut ceil_stop = false;
    for i in 0..steps {
        let prev = pos;
        if i == 0 {
            pos += push;
        }
        pos += step;
        let mut current = Contacts::default();
        let lwalls = walls(&surfaces, *coll, *previous, prev, pos, Kind::LeftWall, None);
        if let Some(hit) = correct_wall(&surfaces, *coll, &mut pos, Kind::LeftWall, lwalls) {
            contacts.left_wall = Some(hit);
            current.left_wall = Some(hit);
        }
        let rwalls = walls(
            &surfaces,
            *coll,
            *previous,
            prev,
            pos,
            Kind::RightWall,
            None,
        );
        if let Some(hit) = correct_wall(&surfaces, *coll, &mut pos, Kind::RightWall, rwalls) {
            contacts.right_wall = Some(hit);
            current.right_wall = Some(hit);
        }
        let ceil = ceiling_step(&surfaces, *coll, *previous, prev, &mut pos, &current);
        if let Some(hit) = ceil.contact {
            contacts.ceiling = Some(hit);
        }
        if ceil.touched && options.ceil_heavy {
            // `MAP_PROC_TYPE_CEILHEAVY`: a fast rise into a ceiling ends
            // the sweep after this substep (`is_coll_end`).
            ceil_stop = true;
        }
        let bottom = Vec2::new(0.0, coll.bottom);
        let floor = sweep(
            &surfaces,
            Kind::Floor,
            point(prev, Vec2::new(0.0, previous.bottom)),
            point(pos, bottom),
        )
        .map(|(h, _)| h)
        .or_else(|| adjacent_horizontal(&surfaces, &current, Kind::Floor, pos, coll.bottom));
        if let Some(hit) = floor.filter(|h| {
            h.flags & collision::flags::PASS == 0
                || (!options.skip_pass && Some(h.line) != options.ignore_line)
        }) {
            contacts.floor = true;
            let mut moved = land(&surfaces, coll, pos, hit);
            if let Some(floor) = moved.floor {
                adjust_edges(
                    &surfaces,
                    *coll,
                    &mut moved.pos,
                    Kind::Floor,
                    Contact {
                        line: floor.line,
                        flags: floor.flags,
                        normal: floor.normal,
                    },
                );
            }
            return AirMoved {
                moved,
                contacts,
                cliff: None,
                ceil_stop,
            };
        }
        if let Some(check) = options.cliff {
            if let Some((line, corner)) =
                cliff(&surfaces, check.facing, check.wait, check.reach, prev, pos)
            {
                if check.occupied != Some((line, check.facing)) {
                    return AirMoved {
                        moved: Moved { pos, floor: None },
                        contacts,
                        cliff: Some((line, corner)),
                        ceil_stop,
                    };
                }
            }
        }
        if ceil_stop {
            break;
        }
    }
    AirMoved {
        moved: Moved { pos, floor: None },
        contacts,
        cliff: None,
        ceil_stop,
    }
}

fn adjacent_horizontal<I, F>(
    surfaces: &F,
    contacts: &Contacts,
    kind: Kind,
    pos: Vec3,
    offset: f32,
) -> Option<Contact>
where
    F: Fn() -> I,
    I: IntoIterator<Item = MapSurface>,
{
    let (wall_kind, wall) = if let Some(wall) = contacts.left_wall {
        (Kind::LeftWall, wall)
    } else {
        (Kind::RightWall, contacts.right_wall?)
    };
    let (k, line) = neighbor(surfaces, wall_kind, wall.line, kind == Kind::Ceiling)?;
    if k != kind {
        return None;
    }
    let (y, hit) = height(surfaces, kind, line, pos.x)?;
    ((kind == Kind::Floor && y > pos.y + offset) || (kind == Kind::Ceiling && y < pos.y + offset))
        .then_some(hit)
}

/// Source floor/ceiling contour adjustment after placing the vertical tip.
fn adjust_edges<I, F>(surfaces: &F, coll: BodyColl, pos: &mut Vec3, kind: Kind, hit: Contact)
where
    F: Fn() -> I,
    I: IntoIterator<Item = MapSurface>,
{
    for wall_kind in [Kind::LeftWall, Kind::RightWall] {
        let side = if wall_kind == Kind::LeftWall {
            1.0
        } else {
            -1.0
        };
        let tip = Vec2::new(
            pos.x,
            pos.y
                + if kind == Kind::Floor {
                    coll.bottom
                } else {
                    coll.top
                },
        );
        let waist = Vec2::new(pos.x + side * coll.width, pos.y + coll.center);
        let Some((wall, _)) = query(surfaces, wall_kind, tip, waist) else {
            continue;
        };
        let neighbor_high = if kind == Kind::Floor {
            side < 0.0
        } else {
            side > 0.0
        };
        if neighbor(surfaces, kind, hit.line, neighbor_high).is_some_and(|(_, l)| l == wall.line) {
            continue;
        }
        let lower = edge(surfaces, wall_kind, wall.line, false).map(|e| e.0.y);
        let upper = edge(surfaces, wall_kind, wall.line, true);
        if kind == Kind::Floor
            && !lower
                .zip(upper)
                .is_some_and(|(lo, (hi, _))| waist.y >= lo - 0.001 && waist.y <= hi.y + 0.001)
        {
            if let Some((upper, _)) = upper {
                // Preserve the asymmetric active source: the RWall branch
                // overwrites its upper-edge probe using the waist position.
                let (a, b) = if side > 0.0 {
                    let a = Vec2::new(upper.x - 2.0, upper.y);
                    (
                        a,
                        Vec2::new(
                            a.x - 2.0 * coll.width,
                            a.y - 2.0 * (coll.center - coll.bottom),
                        ),
                    )
                } else {
                    (
                        Vec2::new(
                            waist.x + 2.0 * coll.width,
                            waist.y - 2.0 * (coll.center - coll.bottom),
                        ),
                        waist,
                    )
                };
                if let Some((_, p)) = query(surfaces, Kind::Floor, a, b) {
                    if let Some((y, _)) = height(surfaces, Kind::Floor, hit.line, p.x) {
                        pos.x = p.x;
                        pos.y = y - coll.bottom;
                    }
                }
            }
            continue;
        }
        let direction = if kind == Kind::Floor { -1.0 } else { 1.0 };
        let start = Vec2::new(
            waist.x + direction * side * 2.0 * hit.normal.y * coll.width,
            waist.y - direction * side * 2.0 * hit.normal.x * coll.width,
        );
        if let Some((_, p)) = query(surfaces, wall_kind, start, waist) {
            let x = p.x - side * coll.width;
            if let Some((y, _)) = height(surfaces, kind, hit.line, x) {
                pos.x = x;
                pos.y = y - if kind == Kind::Floor {
                    coll.bottom
                } else {
                    coll.top
                };
            }
        }
    }
}

pub fn line_speed<I, F>(surfaces: F, line: u16) -> Vec3
where
    F: Fn() -> I,
    I: IntoIterator<Item = MapSurface>,
{
    surfaces()
        .into_iter()
        .enumerate()
        .find(|(i, s)| line_id(*i, *s) == line)
        .and_then(|(_, s)| s.motion)
        .map_or(Vec3::ZERO, |m| m.speed)
}

fn follow_floor<I, F>(surfaces: &F, coll: &BodyColl, pos: Vec3, line: u16) -> Moved
where
    F: Fn() -> I,
    I: IntoIterator<Item = MapSurface>,
{
    match height(surfaces, Kind::Floor, line, pos.x) {
        Some((y, h)) => Moved {
            pos: Vec3::new(pos.x, y - coll.bottom, pos.z),
            floor: Some(Standing {
                line,
                flags: h.flags,
                normal: h.normal,
            }),
        },
        None => Moved { pos, floor: None },
    }
}

fn land<I, F>(surfaces: &F, coll: &BodyColl, pos: Vec3, hit: Contact) -> Moved
where
    F: Fn() -> I,
    I: IntoIterator<Item = MapSurface>,
{
    let moved = follow_floor(surfaces, coll, pos, hit.line);
    if moved.floor.is_some() {
        return moved;
    }
    let high = pos.x > edge(surfaces, Kind::Floor, hit.line, false).map_or(pos.x, |e| e.0.x);
    let pos = edge(surfaces, Kind::Floor, hit.line, high)
        .map_or(pos, |(p, _)| Vec3::new(p.x, p.y - coll.bottom, pos.z));
    Moved {
        pos,
        floor: Some(Standing {
            line: hit.line,
            flags: hit.flags,
            normal: hit.normal,
        }),
    }
}

pub fn cliff_corner<I, F>(surfaces: F, line: u16, facing: f32) -> Option<Vec2>
where
    F: Fn() -> I,
    I: IntoIterator<Item = MapSurface>,
{
    edge(&surfaces, Kind::Floor, line, facing < 0.0).map(|e| e.0)
}

pub fn move_ground<I, F>(
    coll: &BodyColl,
    from: Vec3,
    to: Vec3,
    line: u16,
    stop_edge: bool,
    surfaces: F,
) -> (Moved, Contacts)
where
    F: Fn() -> I,
    I: IntoIterator<Item = MapSurface>,
{
    move_ground_pushed(coll, from, to, Vec3::ZERO, line, stop_edge, surfaces)
}

/// [`move_ground`] with `coll_data.vel_push`, added beside the line speed on
/// the first substep. Unlike the line speed it is not part of the displacement
/// `mpProcessUpdateMain` splits.
pub fn move_ground_pushed<I, F>(
    coll: &BodyColl,
    from: Vec3,
    to: Vec3,
    push: Vec3,
    line: u16,
    stop_edge: bool,
    surfaces: F,
) -> (Moved, Contacts)
where
    F: Fn() -> I,
    I: IntoIterator<Item = MapSurface>,
{
    let speed = line_speed(&surfaces, line) + push;
    let counted = speed - push;
    let steps = ground::substep_count(from, to + counted);
    let step = Vec3::new(
        (to.x - from.x) / steps as f32,
        0.0,
        (to.z - from.z) / steps as f32,
    );
    let mut pos = from;
    let mut contacts = Contacts::default();
    for i in 0..steps {
        let prev = pos;
        if i == 0 {
            pos += speed;
        }
        pos += step;
        for kind in [Kind::LeftWall, Kind::RightWall] {
            let lines = walls(&surfaces, *coll, *coll, prev, pos, kind, Some(line));
            if let Some(hit) = correct_wall(&surfaces, *coll, &mut pos, kind, lines) {
                if kind == Kind::LeftWall {
                    contacts.left_wall = Some(hit);
                } else {
                    contacts.right_wall = Some(hit);
                }
            }
        }
        let mut moved = follow_floor(&surfaces, coll, pos, line);
        if moved.floor.is_none() {
            if let Some((corner, _)) = edge(
                &surfaces,
                Kind::Floor,
                line,
                pos.x > edge(&surfaces, Kind::Floor, line, false).map_or(pos.x, |e| e.0.x),
            ) {
                moved.pos.y = corner.y - coll.bottom;
                let right = pos.x > corner.x;
                if neighbor(&surfaces, Kind::Floor, line, right).is_some_and(|(k, _)| {
                    k == if right {
                        Kind::LeftWall
                    } else {
                        Kind::RightWall
                    }
                }) {
                    moved = follow_floor(
                        &surfaces,
                        coll,
                        Vec3::new(corner.x, moved.pos.y, pos.z),
                        line,
                    );
                }
            }
        }
        if moved.floor.is_none() && stop_edge {
            if let Some(corner) = edge(
                &surfaces,
                Kind::Floor,
                line,
                pos.x > edge(&surfaces, Kind::Floor, line, false).map_or(pos.x, |e| e.0.x),
            )
            .map(|e| e.0)
            {
                let side = if pos.x <= corner.x { 1.0 } else { -1.0 };
                let kind = if side > 0.0 {
                    Kind::LeftWall
                } else {
                    Kind::RightWall
                };
                if query(
                    &surfaces,
                    kind,
                    Vec2::new(corner.x + side, corner.y + 1.0),
                    Vec2::new(
                        corner.x + side * coll.width,
                        corner.y + coll.center - coll.bottom,
                    ),
                )
                .is_none()
                {
                    moved =
                        follow_floor(&surfaces, coll, Vec3::new(corner.x, corner.y, pos.z), line);
                }
            }
        }
        if let Some(floor) = moved.floor {
            adjust_edges(
                &surfaces,
                *coll,
                &mut moved.pos,
                Kind::Floor,
                Contact {
                    line: floor.line,
                    flags: floor.flags,
                    normal: floor.normal,
                },
            );
        }
        let bottom = Vec2::new(0.0, coll.bottom);
        if let Some((hit, _)) = sweep(
            &surfaces,
            Kind::Floor,
            point(prev, bottom),
            point(moved.pos, bottom),
        )
        .filter(|(h, _)| h.line != line)
        {
            moved = land(&surfaces, coll, moved.pos, hit);
            contacts.floor = moved.floor.is_some();
            return (moved, contacts);
        }
        pos = moved.pos;
        contacts.floor = moved.floor.is_some();
        if moved.floor.is_none() || contacts.left_wall.is_some() || contacts.right_wall.is_some() {
            return (moved, contacts);
        }
    }
    let moved = follow_floor(&surfaces, coll, pos, line);
    contacts.floor = moved.floor.is_some();
    (moved, contacts)
}

/// Hand-reach sweep used by both cliff checks, including the source's
/// left-side-only material-4 exclusion and full-polyline endpoints.
pub fn cliff<I, F>(
    surfaces: F,
    facing: f32,
    wait: u16,
    reach: Vec2,
    from: Vec3,
    to: Vec3,
) -> Option<(u16, Vec2)>
where
    F: Fn() -> I,
    I: IntoIterator<Item = MapSurface>,
{
    if wait != 0 {
        return None;
    }
    let offset = Vec2::new(reach.x * facing, reach.y);
    let (hit, crossing) = sweep(
        &surfaces,
        Kind::Floor,
        point(from, offset),
        point(to, offset),
    )?;
    if hit.flags & collision::flags::CLIFF == 0
        || (facing > 0.0 && hit.flags & collision::flags::MATERIAL == 4)
    {
        return None;
    }
    let (corner, _) = edge(&surfaces, Kind::Floor, hit.line, facing < 0.0)?;
    let distance = if facing > 0.0 {
        crossing.x - corner.x
    } else {
        corner.x - crossing.x
    };
    (distance < 800.0).then_some((hit.line, corner))
}

/// `MAP_FLAG_LWALL`, `MAP_FLAG_RWALL`, `MAP_FLAG_CEIL` and `MAP_FLAG_FLOOR`,
/// as the damage statuses keep them in `status_vars.common.damage.coll_mask_*`.
pub const MASK_LWALL: u16 = 1 << 0;
pub const MASK_RWALL: u16 = 1 << 5;
pub const MASK_CEIL: u16 = 1 << 10;
pub const MASK_FLOOR: u16 = 1 << 11;

/// `mpCommonProcFighterDamage`: a surface bounces a tumbling fighter only when
/// the frame's movement is longer than this…
pub const DAMAGE_COLLIDE_SPEED_MIN: f32 = 30.0;
/// …and meets the surface's normal at more than 110° (`F_CLC_DTOR32(110)`),
/// compared through its cosine.
const DAMAGE_COLLIDE_COS_MAX: f32 = -0.342_020_15;

/// Result of [`move_damage`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DamageMoved {
    /// Where the body ended up; `floor` is set only by a landing.
    pub moved: Moved,
    pub contacts: Contacts,
    /// `coll_mask_curr`: the surfaces the fighter struck head-on this frame.
    pub mask_curr: u16,
    /// `mpCommonCheckFighterDamageCollision`'s return: the last substep's
    /// `is_collide`.
    pub collide: bool,
    /// The struck surfaces' normals (`lwall_angle`, `rwall_angle`,
    /// `ceil_angle`), for `ftCommonWallDamageCheckGoto`.
    pub lwall_normal: Vec2,
    pub rwall_normal: Vec2,
    pub ceil_normal: Vec2,
}

/// `syVectorAngleDiff3D(pos_diff, normal) > 110°` with the length gate.
fn strikes(diff: Vec3, normal: Vec2) -> bool {
    let mag2 = ssb_engine::math::sqrt(diff.x * diff.x + diff.y * diff.y);
    if mag2 <= DAMAGE_COLLIDE_SPEED_MIN {
        return false;
    }
    faces(diff, normal)
}

fn faces(diff: Vec3, normal: Vec2) -> bool {
    let n = normal.length();
    let d = diff.length();
    if n == 0.0 || d == 0.0 {
        return false;
    }
    (diff.x * normal.x + diff.y * normal.y) / (n * d) < DAMAGE_COLLIDE_COS_MAX
}

/// `mpProcessSetCollideFloor` for an airborne body: put it on the floor line
/// without landing. Past either end it drops to the corner, and slides onto
/// it only where a wall hangs below that end.
fn collide_floor<I, F>(surfaces: &F, coll: BodyColl, pos: &mut Vec3, hit: Contact) -> bool
where
    F: Fn() -> I,
    I: IntoIterator<Item = MapSurface>,
{
    if let Some((y, h)) = height(surfaces, Kind::Floor, hit.line, pos.x) {
        pos.y = y - coll.bottom;
        adjust_edges(surfaces, coll, pos, Kind::Floor, h);
        return true;
    }
    let Some((low, _)) = edge(surfaces, Kind::Floor, hit.line, false) else {
        return false;
    };
    let (corner, right) = if pos.x <= low.x {
        (low, false)
    } else {
        match edge(surfaces, Kind::Floor, hit.line, true) {
            Some((high, _)) => (high, true),
            None => return false,
        }
    };
    let wall = if right {
        Kind::LeftWall
    } else {
        Kind::RightWall
    };
    pos.y = corner.y - coll.bottom;
    if neighbor(surfaces, Kind::Floor, hit.line, right).is_some_and(|(k, _)| k == wall) {
        pos.x = corner.x;
        adjust_edges(surfaces, coll, pos, Kind::Floor, hit);
        return true;
    }
    false
}

/// `mpCommonCheckFighterDamageCollision` + `mpCommonProcFighterDamage`, the
/// map sweep of the airborne damage statuses (`DamageE2`, the `DamageFly*`
/// family and `WallDamage`). Walls and the ceiling stop the body as usual,
/// but one struck head-on (see [`strikes`]) that was not already struck last
/// frame (`prev_mask`) ends the sweep and is reported in
/// [`DamageMoved::mask_curr`]. A floor is landed on only when the movement
/// meets it at more than 110° and the fighter is out of hitlag; a grazing
/// floor, or any floor during hitlag, carries the body along without landing.
pub fn move_damage<I, F>(
    coll: &BodyColl,
    from: Vec3,
    to: Vec3,
    prev_mask: u16,
    hitlag: bool,
    ignore_line: Option<u16>,
    surfaces: F,
) -> DamageMoved
where
    F: Fn() -> I,
    I: IntoIterator<Item = MapSurface>,
{
    move_damage_pushed(
        coll,
        from,
        to,
        Vec3::ZERO,
        prev_mask,
        hitlag,
        ignore_line,
        surfaces,
    )
}

/// [`move_damage`] with `coll_data.vel_push` on the first substep. The
/// strike tests keep using `pos_diff`, which excludes the push.
#[allow(clippy::too_many_arguments)]
pub fn move_damage_pushed<I, F>(
    coll: &BodyColl,
    from: Vec3,
    to: Vec3,
    push: Vec3,
    prev_mask: u16,
    hitlag: bool,
    ignore_line: Option<u16>,
    surfaces: F,
) -> DamageMoved
where
    F: Fn() -> I,
    I: IntoIterator<Item = MapSurface>,
{
    let diff = to - from;
    let steps = ground::substep_count(from, to);
    let step = Vec3::new(
        diff.x / steps as f32,
        diff.y / steps as f32,
        diff.z / steps as f32,
    );
    let mut out = DamageMoved {
        moved: Moved {
            pos: from,
            floor: None,
        },
        contacts: Contacts::default(),
        mask_curr: 0,
        collide: false,
        lwall_normal: Vec2::ZERO,
        rwall_normal: Vec2::ZERO,
        ceil_normal: Vec2::ZERO,
    };
    let mut pos = from;
    for i in 0..steps {
        let prev = pos;
        if i == 0 {
            pos += push;
        }
        pos += step;
        let mut collide = false;
        let mut current = Contacts::default();
        let lwalls = walls(&surfaces, *coll, *coll, prev, pos, Kind::LeftWall, None);
        if let Some(hit) = correct_wall(&surfaces, *coll, &mut pos, Kind::LeftWall, lwalls) {
            out.contacts.left_wall = Some(hit);
            current.left_wall = Some(hit);
            out.lwall_normal = hit.normal;
            if prev_mask & MASK_LWALL == 0 && strikes(diff, hit.normal) {
                out.mask_curr |= MASK_LWALL;
                collide = true;
            }
        }
        let rwalls = walls(&surfaces, *coll, *coll, prev, pos, Kind::RightWall, None);
        if let Some(hit) = correct_wall(&surfaces, *coll, &mut pos, Kind::RightWall, rwalls) {
            out.contacts.right_wall = Some(hit);
            current.right_wall = Some(hit);
            out.rwall_normal = hit.normal;
            if prev_mask & MASK_RWALL == 0 && strikes(diff, hit.normal) {
                out.mask_curr |= MASK_RWALL;
                collide = true;
            }
        }
        let ceil = ceiling_step(&surfaces, *coll, *coll, prev, &mut pos, &current);
        if let Some(hit) = ceil.contact {
            out.contacts.ceiling = Some(hit);
        }
        if let Some(hit) = ceil.hit {
            out.ceil_normal = hit.normal;
            if prev_mask & MASK_CEIL == 0 && strikes(diff, hit.normal) {
                out.mask_curr |= MASK_CEIL;
                collide = true;
            }
        }
        let bottom = Vec2::new(0.0, coll.bottom);
        let floor = sweep(
            &surfaces,
            Kind::Floor,
            point(prev, bottom),
            point(pos, bottom),
        )
        .map(|(h, _)| h)
        .or_else(|| adjacent_horizontal(&surfaces, &current, Kind::Floor, pos, coll.bottom))
        .filter(|h| h.flags & collision::flags::PASS == 0 || Some(h.line) != ignore_line);
        if let Some(hit) = floor {
            if !hitlag && faces(diff, hit.normal) {
                // `mpProcessSetLandingFloor`.
                let mut moved = land(&surfaces, coll, pos, hit);
                if let Some(floor) = moved.floor {
                    adjust_edges(
                        &surfaces,
                        *coll,
                        &mut moved.pos,
                        Kind::Floor,
                        Contact {
                            line: floor.line,
                            flags: floor.flags,
                            normal: floor.normal,
                        },
                    );
                    out.contacts.floor = true;
                    out.mask_curr |= MASK_FLOOR;
                    out.moved = moved;
                    out.collide = true;
                    return out;
                }
            } else if collide_floor(&surfaces, *coll, &mut pos, hit) {
                out.contacts.floor = true;
            }
        }
        out.collide = collide;
        if collide {
            break;
        }
    }
    out.moved = Moved { pos, floor: None };
    out
}

pub(crate) fn stops_at_edge(s: AnyStatus) -> bool {
    matches!(
        s,
        AnyStatus::Purin(PurinStatus::SpecialN | PurinStatus::SpecialHi)
            | AnyStatus::Ness(
                NessStatus::SpecialHiStart | NessStatus::SpecialHiHold | NessStatus::SpecialHiEnd
            )
            | AnyStatus::Kirby(KirbyStatus::SpecialHi)
            | AnyStatus::Common(Status::Catch)
    )
}

/// The statuses on `mpCommonProcFighterCliffFloorCeil`, whose ceiling test
/// is `MAP_PROC_TYPE_CEILHEAVY`.
pub fn is_ceil_heavy_status(s: AnyStatus) -> bool {
    matches!(
        s,
        AnyStatus::Common(
            Status::JumpF
                | Status::JumpB
                | Status::JumpAerialF
                | Status::JumpAerialB
                | Status::Fall
                | Status::FallAerial
                | Status::Pass
                | Status::GuardPass
                | Status::StopCeil
        )
    )
}

pub(crate) fn allows_cliff(f: &Fighter) -> bool {
    match f.status.status {
        AnyStatus::Common(s) => matches!(
            s,
            Status::JumpF
                | Status::JumpB
                | Status::JumpAerialF
                | Status::JumpAerialB
                | Status::Fall
                | Status::FallAerial
                | Status::Pass
                | Status::FallSpecial
                | Status::DamageFall
        ),
        AnyStatus::Fox(s) => matches!(s, FoxStatus::SpecialAirHiEnd | FoxStatus::SpecialAirHiBound),
        AnyStatus::Ness(s) => matches!(
            s,
            NessStatus::SpecialAirHiStart
                | NessStatus::SpecialAirHiHold
                | NessStatus::SpecialAirHiEnd
                | NessStatus::SpecialAirHiJibaku
                | NessStatus::SpecialAirHiBound
        ),
        AnyStatus::Pikachu(s) => matches!(
            s,
            PikachuStatus::SpecialAirHi | PikachuStatus::SpecialAirHiEnd
        ),
        AnyStatus::Captain(s) => {
            matches!(s, CaptainStatus::SpecialHi | CaptainStatus::SpecialAirHi)
                && (f.physics.vel_air.y >= 0.0 || f.captain.dive_cliff_wait == 0)
        }
        AnyStatus::Kirby(s) => matches!(
            s,
            KirbyStatus::SpecialHi
                | KirbyStatus::SpecialAirHi
                | KirbyStatus::SpecialAirHiFall
                | KirbyStatus::JumpAerialF1
                | KirbyStatus::JumpAerialF2
                | KirbyStatus::JumpAerialF3
                | KirbyStatus::JumpAerialF4
                | KirbyStatus::JumpAerialF5
        ),
        AnyStatus::Donkey(s) => s == status::DonkeyStatus::SpecialAirHi,
        AnyStatus::Yoshi(s) => {
            s == status::YoshiStatus::SpecialAirHi
                || s == status::YoshiStatus::SpecialAirLwLoop
                || matches!(
                    s,
                    status::YoshiStatus::SpecialLwStart | status::YoshiStatus::SpecialAirLwStart
                ) && f.physics.vel_air.y <= 0.0
                    && f.status.anim_frame
                        >= if s == status::YoshiStatus::SpecialLwStart {
                            30.0
                        } else {
                            5.0
                        }
        }
        AnyStatus::Purin(s) => matches!(
            s,
            PurinStatus::JumpAerialF1
                | PurinStatus::JumpAerialF2
                | PurinStatus::JumpAerialF3
                | PurinStatus::JumpAerialF4
                | PurinStatus::JumpAerialF5
        ),
        AnyStatus::Link(s) => s == status::LinkStatus::SpecialAirHi,
        _ => false,
    }
}

pub fn is_cliff_hold(s: AnyStatus) -> bool {
    matches!(
        s,
        AnyStatus::Common(
            Status::CliffCatch
                | Status::CliffWait
                | Status::CliffQuick
                | Status::CliffSlow
                | Status::CliffClimbQuick1
                | Status::CliffClimbSlow1
                | Status::CliffAttackQuick1
                | Status::CliffAttackSlow1
                | Status::CliffEscapeQuick1
                | Status::CliffEscapeSlow1
        )
    )
}

pub fn is_cliff_phase2(s: AnyStatus) -> bool {
    matches!(
        s,
        AnyStatus::Common(
            Status::CliffClimbQuick2
                | Status::CliffClimbSlow2
                | Status::CliffAttackQuick2
                | Status::CliffAttackSlow2
                | Status::CliffEscapeQuick2
                | Status::CliffEscapeSlow2
        )
    )
}

pub fn floor_point<I, F>(surfaces: F, line: u16, x: f32) -> Option<(f32, Standing)>
where
    F: Fn() -> I,
    I: IntoIterator<Item = MapSurface>,
{
    height(&surfaces, Kind::Floor, line, x).map(|(y, h)| {
        (
            y,
            Standing {
                line,
                flags: h.flags,
                normal: h.normal,
            },
        )
    })
}

pub(crate) fn ground_callback(f: &mut Fighter, floor: Option<Standing>) -> bool {
    if f.map_contacts.left_wall.is_none() && f.map_contacts.right_wall.is_none() {
        return false;
    }
    if f.status.status == AnyStatus::Pikachu(PikachuStatus::SpecialHi) {
        if floor.is_none() {
            f.become_airborne();
        } else {
            f.floor = floor;
        }
        crate::pikachu::map_end_zip(f);
        return true;
    }
    if f.status.status == AnyStatus::Ness(NessStatus::SpecialHiJibaku) {
        if floor.is_none() {
            f.become_airborne();
            crate::ness::map_end_blast(f);
        } else {
            f.floor = floor;
            crate::ness::map_down_bounce(f);
        }
        return true;
    }
    crate::captain::map_wall(f)
}

pub(crate) fn air_callback(f: &mut Fighter) {
    let contacts = f.map_contacts;
    if matches!(
        f.status.status,
        AnyStatus::Common(Status::ThrownKirbyStar | Status::ThrownCopyStar)
    ) {
        if let Some(hit) = contacts
            .ceiling
            .or(contacts.left_wall)
            .or(contacts.right_wall)
        {
            crate::capture_kirby::on_landing(f, f.pos.y, hit.normal);
        }
        return;
    }
    if f.status.status == AnyStatus::Pikachu(PikachuStatus::SpecialAirHi) {
        for hit in [contacts.ceiling, contacts.left_wall, contacts.right_wall]
            .into_iter()
            .flatten()
        {
            let v = f.physics.vel_air;
            if hit.normal.x * v.x + hit.normal.y * v.y
                < v.length() * -core::f32::consts::FRAC_1_SQRT_2
            {
                crate::pikachu::map_end_zip(f);
            }
        }
    } else if f.status.status == AnyStatus::Ness(NessStatus::SpecialAirHiJibaku) {
        crate::ness::map_blast_contacts(f);
    } else if f.status.status == AnyStatus::Fox(FoxStatus::SpecialAirHi) {
        for (hit, prev) in [
            (contacts.ceiling, f.map_contacts_prev.ceiling),
            (contacts.left_wall, f.map_contacts_prev.left_wall),
            (contacts.right_wall, f.map_contacts_prev.right_wall),
        ] {
            if let Some(hit) = hit.filter(|_| prev.is_none()) {
                if adjust_velocity(&mut f.physics.vel_air, hit.normal, -0.342_020_15) {
                    f.facing = if f.physics.vel_air.x >= 0.0 {
                        Facing::Right
                    } else {
                        Facing::Left
                    };
                    f.fox_special_hi.angle = ssb_engine::math::atan2(
                        f.physics.vel_air.y,
                        f.physics.vel_air.x * f.facing.sign(),
                    );
                }
                // Source gives ceiling, LWall, RWall priority, even when
                // the chosen contact is too steep to redirect.
                break;
            }
        }
    } else {
        crate::captain::map_wall(f);
    }
}

/// `lbCommonCheckAdjustSim2D`; similarity uses the sum of magnitudes.
pub(crate) fn adjust_velocity(v: &mut Vec3, normal: Vec2, minimum: f32) -> bool {
    let speed = ssb_engine::math::sqrt(v.x * v.x + v.y * v.y);
    let sim = (v.x * normal.x + v.y * normal.y) / (normal.length() + speed);
    if sim <= 0.0 && sim >= minimum {
        let signed = speed
            * if normal.x * v.y - normal.y * v.x < 0.0 {
                -1.0
            } else {
                1.0
            };
        v.x = -normal.y * signed;
        v.y = normal.x * signed;
        true
    } else {
        false
    }
}

/// `mpCollisionCheckProjectFloor`: the nearest floor at or below `pos`,
/// testing a moving group's lines in its own local frame. Returns the line
/// and the signed distance down to it.
pub fn project_floor_line<I, F>(surfaces: &F, pos: Vec3) -> Option<(u16, f32)>
where
    F: Fn() -> I,
    I: IntoIterator<Item = MapSurface>,
{
    let mut best: Option<(u16, f32)> = None;
    for (i, s) in surfaces().into_iter().enumerate() {
        if s.kind != Kind::Floor {
            continue;
        }
        let o = s.motion.map_or(Vec2::ZERO, |m| m.offset);
        let local = Vec2::new(pos.x - o.x, pos.y - o.y);
        let Some(hit) = collision::project_floor([(line_id(i, s), s.segment)], local) else {
            continue;
        };
        if best.is_none_or(|(_, d)| hit.dist.abs() < d.abs()) {
            best = Some((hit.line, hit.dist));
        }
    }
    best
}

/// `mpCollisionGet{Floor,Ceil}Edge{L,R}` and `mpCollisionGet{L,R}WallEdge{D,U}`:
/// a line's low or high end along its own axis (x for floors and ceilings,
/// y for walls).
pub fn line_edge<I, F>(surfaces: &F, kind: Kind, line: u16, high: bool) -> Option<Vec2>
where
    F: Fn() -> I,
    I: IntoIterator<Item = MapSurface>,
{
    edge(surfaces, kind, line, high).map(|(p, _)| p)
}

/// Every segment of one kind with the line id it belongs to, in the order
/// `gMPCollisionLineGroups[kind]` lists them.
pub fn lines_of<I>(surfaces: I, kind: Kind) -> impl Iterator<Item = (u16, MapSurface)>
where
    I: IntoIterator<Item = MapSurface>,
{
    surfaces
        .into_iter()
        .enumerate()
        .filter(move |(_, s)| s.kind == kind)
        .map(|(i, s)| (line_id(i, s), s))
}

/// `mpCollisionGetFloorEdgeL` / `...R`.
pub fn floor_edge<I, F>(surfaces: &F, line: u16, right: bool) -> Option<Vec2>
where
    F: Fn() -> I,
    I: IntoIterator<Item = MapSurface>,
{
    edge(surfaces, Kind::Floor, line, right).map(|(p, _)| p)
}

/// `mpCollisionGetEdgeUnderLLineID` / `...R` followed by
/// `mpCollisionGetLineTypeID`: the kind of line below a floor's end.
pub fn floor_edge_under<I, F>(surfaces: &F, line: u16, right: bool) -> Option<Kind>
where
    F: Fn() -> I,
    I: IntoIterator<Item = MapSurface>,
{
    neighbor(surfaces, Kind::Floor, line, right).map(|(k, _)| k)
}
