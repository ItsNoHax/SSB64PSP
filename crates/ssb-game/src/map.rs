//! Static fighter map processing from `mpcommon.c` and `mpprocess.c`.
//! Queries retain the source order: LWall, RWall, ceiling, floor, cliff.
//! Moving-group speed and changing collision diamonds require match inputs
//! which the current runtime does not supply.

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
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AirMoved {
    pub moved: Moved,
    pub contacts: Contacts,
    pub cliff: Option<(u16, Vec2)>,
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

fn transformed(kind: Kind, s: Segment) -> [f32; 4] {
    let a = transform(kind, Vec2::new(s.x1 as f32, s.y1 as f32));
    let b = transform(kind, Vec2::new(s.x2 as f32, s.y2 as f32));
    [a.x, a.y, b.x, b.y]
}

fn query<I, F>(surfaces: &F, kind: Kind, from: Vec2, to: Vec2) -> Option<(Contact, Vec2)>
where
    F: Fn() -> I,
    I: IntoIterator<Item = MapSurface>,
{
    let a = transform(kind, from);
    let b = transform(kind, to);
    let mut best = None;
    let mut distance = f32::MAX;
    for (i, s) in surfaces().into_iter().enumerate() {
        if s.kind != kind {
            continue;
        }
        let Some(p) = collision::check_floor_coords(transformed(kind, s.segment), a, b) else {
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
    let f = collision::floor_height(
        surfaces().into_iter().enumerate().filter_map(|(i, s)| {
            (s.kind == kind && line_id(i, s) == line).then_some((line, s.segment))
        }),
        x,
    )?;
    let normal = if kind == Kind::Ceiling {
        Vec2::new(-f.normal.x, -f.normal.y)
    } else {
        f.normal
    };
    Some((
        f.y,
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
        for (p, id) in [
            (Vec2::new(s.segment.x1 as f32, s.segment.y1 as f32), ids[0]),
            (Vec2::new(s.segment.x2 as f32, s.segment.y2 as f32), ids[1]),
        ] {
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
    for (a, b) in [
        (point(from, waist), point(to, waist)),
        (point(from, bottom), point(to, bottom)),
        (point(from, top), point(to, top)),
        (point(to, bottom), point(to, waist)),
        (point(to, top), point(to, waist)),
    ] {
        if let Some((hit, _)) = query(surfaces, kind, a, b) {
            add(hit.line);
        }
    }
    if ground_line.is_some() {
        return lines;
    }
    // A waist sweep across a neighboring ceiling/floor can touch a wall
    // even when none of the three vertical probes crosses the wall itself.
    for (other, tip) in [(Kind::Ceiling, top), (Kind::Floor, bottom)] {
        if let Some((hit, _)) = query(surfaces, other, point(from, waist), point(to, waist)) {
            if other == Kind::Floor && hit.flags & collision::flags::PASS != 0 {
                continue;
            }
            let high = kind == Kind::RightWall;
            if let Some((neighbor_kind, line)) = neighbor(surfaces, other, hit.line, high) {
                if neighbor_kind == kind
                    && query(surfaces, other, point(from, tip), point(to, tip))
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
            let seg = s.segment;
            for (offset, width) in [
                (coll.bottom, 0.0),
                (coll.center, coll.width),
                (coll.top, 0.0),
            ] {
                let y = pos.y + offset;
                let lo = (seg.y1 as f32).min(seg.y2 as f32);
                let hi = (seg.y1 as f32).max(seg.y2 as f32);
                if y >= lo - 0.001 && y <= hi + 0.001 && seg.y1 != seg.y2 {
                    let y = y.clamp(lo, hi);
                    let x = seg.x1 as f32
                        + (y - seg.y1 as f32) * (seg.x2 as f32 - seg.x1 as f32)
                            / (seg.y2 as f32 - seg.y1 as f32);
                    consider(x - side * width);
                }
            }
            for (x, y) in [
                (seg.x1 as f32, seg.y1 as f32),
                (seg.x2 as f32, seg.y2 as f32),
            ] {
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
    let steps = ground::substep_count(from, to);
    let step = Vec3::new(
        (to.x - from.x) / steps as f32,
        (to.y - from.y) / steps as f32,
        (to.z - from.z) / steps as f32,
    );
    let mut pos = from;
    let mut contacts = Contacts::default();
    for _ in 0..steps {
        let prev = pos;
        pos += step;
        let mut current = Contacts::default();
        let lwalls = walls(&surfaces, *coll, prev, pos, Kind::LeftWall, None);
        if let Some(hit) = correct_wall(&surfaces, *coll, &mut pos, Kind::LeftWall, lwalls) {
            contacts.left_wall = Some(hit);
            current.left_wall = Some(hit);
        }
        let rwalls = walls(&surfaces, *coll, prev, pos, Kind::RightWall, None);
        if let Some(hit) = correct_wall(&surfaces, *coll, &mut pos, Kind::RightWall, rwalls) {
            contacts.right_wall = Some(hit);
            current.right_wall = Some(hit);
        }
        let top = Vec2::new(0.0, coll.top);
        let ceil = query(&surfaces, Kind::Ceiling, point(prev, top), point(pos, top))
            .map(|(h, _)| h)
            .or_else(|| adjacent_horizontal(&surfaces, &current, Kind::Ceiling, pos, coll.top));
        if let Some(hit) = ceil {
            if let Some((y, h)) = height(&surfaces, Kind::Ceiling, hit.line, pos.x) {
                pos.y = y - coll.top;
                contacts.ceiling = Some(h);
                adjust_edges(&surfaces, *coll, &mut pos, Kind::Ceiling, h);
            } else if let Some((corner, _)) = edge(
                &surfaces,
                Kind::Ceiling,
                hit.line,
                pos.x > edge(&surfaces, Kind::Ceiling, hit.line, false).map_or(pos.x, |e| e.0.x),
            ) {
                pos.y = corner.y - coll.top;
                let right = pos.x > corner.x;
                if neighbor(&surfaces, Kind::Ceiling, hit.line, right).is_some_and(|(k, _)| {
                    k == if right {
                        Kind::LeftWall
                    } else {
                        Kind::RightWall
                    }
                }) {
                    pos.x = corner.x;
                    contacts.ceiling = Some(hit);
                }
            }
        }
        let bottom = Vec2::new(0.0, coll.bottom);
        let floor = query(
            &surfaces,
            Kind::Floor,
            point(prev, bottom),
            point(pos, bottom),
        )
        .map(|(h, _)| h)
        .or_else(|| adjacent_horizontal(&surfaces, &current, Kind::Floor, pos, coll.bottom));
        if let Some(hit) = floor.filter(|h| {
            h.flags & collision::flags::PASS == 0
                || (!options.skip_pass && Some(h.line) != options.ignore_line)
        }) {
            contacts.floor = true;
            let mut moved = ground::land(coll, pos, hit.line, hit.flags, hit.normal, || {
                floors(surfaces())
            });
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
                    };
                }
            }
        }
    }
    AirMoved {
        moved: Moved { pos, floor: None },
        contacts,
        cliff: None,
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
    let steps = ground::substep_count(from, to);
    let step = Vec3::new(
        (to.x - from.x) / steps as f32,
        0.0,
        (to.z - from.z) / steps as f32,
    );
    let mut pos = from;
    let mut contacts = Contacts::default();
    for _ in 0..steps {
        let prev = pos;
        pos += step;
        for kind in [Kind::LeftWall, Kind::RightWall] {
            let lines = walls(&surfaces, *coll, prev, pos, kind, Some(line));
            if let Some(hit) = correct_wall(&surfaces, *coll, &mut pos, kind, lines) {
                if kind == Kind::LeftWall {
                    contacts.left_wall = Some(hit);
                } else {
                    contacts.right_wall = Some(hit);
                }
            }
        }
        let mut moved = ground::move_ground(coll, pos, line, || floors(surfaces()));
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
                    moved = ground::move_ground(
                        coll,
                        Vec3::new(corner.x, moved.pos.y, pos.z),
                        line,
                        || floors(surfaces()),
                    );
                }
            }
        }
        if moved.floor.is_none() && stop_edge {
            if let Some(corner) =
                collision::line_edge(floors(surfaces()).filter(|(l, _)| *l == line), pos.x)
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
                    moved = ground::move_ground(
                        coll,
                        Vec3::new(corner.x, corner.y, pos.z),
                        line,
                        || floors(surfaces()),
                    );
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
        if let Some((hit, _)) = query(
            &surfaces,
            Kind::Floor,
            point(prev, bottom),
            point(moved.pos, bottom),
        )
        .filter(|(h, _)| h.line != line)
        {
            moved = ground::land(coll, moved.pos, hit.line, hit.flags, hit.normal, || {
                floors(surfaces())
            });
            contacts.floor = moved.floor.is_some();
            return (moved, contacts);
        }
        pos = moved.pos;
        contacts.floor = moved.floor.is_some();
        if moved.floor.is_none() || contacts.left_wall.is_some() || contacts.right_wall.is_some() {
            return (moved, contacts);
        }
    }
    let moved = ground::move_ground(coll, pos, line, || floors(surfaces()));
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
    let (hit, crossing) = query(
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
                | Status::CliffClimbQuick1
                | Status::CliffClimbSlow1
                | Status::CliffAttackQuick1
                | Status::CliffAttackSlow1
                | Status::CliffEscapeQuick1
                | Status::CliffEscapeSlow1
        )
    )
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
