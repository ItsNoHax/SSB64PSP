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
use ssb_engine::math::{div_nonzero, Vec2, Vec3};

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

impl Contact {
    /// Every field written: what [`contact_or_none`] reads for a `None`.
    pub const NONE: Contact = Contact {
        line: 0,
        flags: 0,
        normal: Vec2::ZERO,
    };
}

/// A found flag and a contact whose floats are always written. Float math
/// on a `None`'s unwritten payload may run before the tag test, and stack
/// garbage that is a signalling NaN traps a PSP (RE-531): loops over the
/// [`Contacts`] that read a contact's normal go through this.
pub fn contact_or_none(c: Option<Contact>) -> (bool, Contact) {
    ssb_engine::math::defined_or(c, Contact::NONE)
}

/// Source current contacts, retained across frames for first-contact callbacks.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Contacts {
    /// `MAP_FLAG_FLOOR` in `mask_curr`: an airborne pass landed, or a
    /// grounded pass swept onto a different floor line
    /// (`mpProcessCheckTestFloorCollision`). Standing on the same floor is
    /// `mask_stat` and leaves it clear.
    pub floor: bool,
    /// `MAP_FLAG_FLOOREDGE`: [`CliffEdge`] caught the fighter at a floor's
    /// end (`mpCommonCheckSetFighterCliffEdge`); it is grounded 40 units in
    /// and teeters (`ftCommonOttottoSetStatus`).
    pub floor_edge: bool,
    pub left_wall: Option<Contact>,
    pub right_wall: Option<Contact>,
    pub ceiling: Option<Contact>,
}

/// `MAP_PROC_TYPE_CLIFFEDGE` (`mpCommonProcFighterOnCliffEdge`): a fighter
/// walking off its floor's end towards the way it faces, without pushing
/// the stick past 60, stops 40 units in instead.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CliffEdge {
    /// `lr`. An enum, not a sign float: a `None` `Option<CliffEdge>` leaves
    /// its payload unwritten, and a float compare LLVM hoists above the tag
    /// test would trap on stack garbage (a PSP traps on a signalling NaN).
    pub facing: Facing,
    /// `input.pl.stick_range.x`.
    pub stick_x: i8,
}

/// `mpCommonCheckSetFighterCliffEdge`'s inset and stick limit.
const CLIFF_EDGE_INSET: f32 = 40.0;
const CLIFF_EDGE_STICK_MAX: i32 = 60;

/// `mpProcessCheckTestLCliffCollision`/`RCliffCollision`'s inputs.
///
/// Not an `Option`: a `None` leaves its payload unwritten, and LLVM hoists
/// `reach.x * facing` out of [`move_air`]'s substep loop above the test,
/// so a signalling NaN in that stack garbage trapped a PSP (D-050). Every
/// field is written; [`Self::enabled`] gates the check instead, as
/// `MAP_PROC_TYPE_CLIFF` does in `mpCommonRunFighterSpecialCollisions`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct CliffQuery {
    /// `MAP_PROC_TYPE_CLIFF`: the status's map process catches ledges.
    pub enabled: bool,
    /// `lr`.
    pub facing: Facing,
    pub wait: u16,
    /// `cliffcatch_coll`.
    pub reach: Vec2,
    /// The other fighters' held `(line, lr)`.
    pub occupied: [Option<(u16, Facing)>; 3],
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct AirOptions {
    pub ignore_line: Option<u16>,
    pub skip_pass: bool,
    /// Disabled by default: no ledge catch.
    pub cliff: CliffQuery,
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

/// `mpProcessCheckTestFloorCollisionAdjNew` with no callback: the floor a
/// point crossed from above while moving `from` to `to` (a moving floor by
/// its own motion), and where. Nothing moves.
pub fn floor_crossing<I, F>(surfaces: &F, from: Vec2, to: Vec2) -> Option<(Contact, Vec2)>
where
    F: Fn() -> I,
    I: IntoIterator<Item = MapSurface>,
{
    sweep(surfaces, Kind::Floor, from, to)
}

/// `mpCollisionCheckLWallLineCollisionSame` / `...RWall...` (`kind` is
/// [`Kind::LeftWall`] or [`Kind::RightWall`]): where the nearest wall line
/// of that side crosses the segment `from` to `to`, groups where they are now.
pub fn wall_crossing<I, F>(surfaces: &F, kind: Kind, from: Vec2, to: Vec2) -> Option<Vec2>
where
    F: Fn() -> I,
    I: IntoIterator<Item = MapSurface>,
{
    debug_assert!(matches!(kind, Kind::LeftWall | Kind::RightWall));
    query(surfaces, kind, from, to).map(|(_, point)| point)
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
    // A flag beside written values, not an `Option<(Vec2, u16)>`: LLVM
    // computes the compare below before the `None` test, and a signalling
    // NaN in an unwritten payload traps a PSP (D-050).
    let mut found = false;
    let mut best = (Vec2::ZERO, u16::MAX);
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
            if !found
                || if high {
                    axis(p) > axis(best.0)
                } else {
                    axis(p) < axis(best.0)
                }
            {
                found = true;
                best = (p, id);
            }
        }
    }
    found.then_some(best)
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
                    // `div_nonzero`: LLVM computes a guarded division
                    // before its guard (RE-469).
                    let x = x1 + div_nonzero((y - y1) * (x2 - x1), y2 - y1);
                    consider(x - side * width);
                }
            }
            for (x, y) in [(x1, y1), (x2, y2)] {
                let y = y - pos.y;
                let width = if y >= coll.bottom && y <= coll.center && coll.center != coll.bottom {
                    Some(div_nonzero(
                        (y - coll.bottom) * coll.width,
                        coll.center - coll.bottom,
                    ))
                } else if y >= coll.center && y <= coll.top && coll.top != coll.center {
                    Some(div_nonzero(
                        (coll.top - y) * coll.width,
                        coll.top - coll.center,
                    ))
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

/// `mpCommonRunFighterCollisionDefault` (`mpCommonRunDefaultCollision`):
/// one wall, ceiling and floor pass from `from` (`pos_prev`, with the
/// `previous` diamond `mpCommonCopyCollDataStats` points at) to `to`, without
/// `mpProcessUpdateMain`'s substeps. Returns the corrected position.
pub fn run_default_collision<I, F>(
    coll: &BodyColl,
    previous: &BodyColl,
    from: Vec3,
    to: Vec3,
    surfaces: F,
) -> Vec3
where
    F: Fn() -> I,
    I: IntoIterator<Item = MapSurface>,
{
    move_air_steps(
        coll,
        previous,
        from,
        to,
        Vec3::ZERO,
        AirOptions::default(),
        surfaces,
        1,
    )
    .moved
    .pos
}

/// What [`run_default_collision_project`] left: the corrected `translate`
/// and the `floor_line_id`, `floor_flags` and `floor_angle` it wrote.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DefaultColl {
    pub pos: Vec3,
    /// The floor the bottom crossed (`mpProcessRunFloorCollisionAdjNewNULL`)
    /// or, failing that, the one projected under it
    /// (`mpProcessSetCollProjectFloorID`); `None` for line id -1.
    pub floor: Option<Standing>,
}

/// `mpCommonRunDefaultCollision` (mpcommon.c:904) as the item and weapon
/// makers reach it (`mpCommonRunItemCollisionDefault`,
/// `mpCommonRunWeaponCollisionDefault`): one left wall, right wall and
/// ceiling pass from `from` (`pos_prev`, probed with the `previous` diamond
/// `mpCommonCopyCollDataStats` points at) to `to` (probed with `coll`), no
/// substeps; a floor the bottom crossed puts the body on its line
/// (`mpProcessSetCollideFloor` and the floor edge adjust), else the floor
/// below is only projected (`mpProcessSetCollProjectFloorID`), which moves
/// nothing.
pub fn run_default_collision_project<I, F>(
    coll: &BodyColl,
    previous: &BodyColl,
    from: Vec3,
    to: Vec3,
    surfaces: F,
) -> DefaultColl
where
    F: Fn() -> I,
    I: IntoIterator<Item = MapSurface>,
{
    let prev = from;
    let mut pos = to;
    let mut current = Contacts::default();
    let lwalls = walls(&surfaces, *coll, *previous, prev, pos, Kind::LeftWall, None);
    if let Some(hit) = correct_wall(&surfaces, *coll, &mut pos, Kind::LeftWall, lwalls) {
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
        current.right_wall = Some(hit);
    }
    ceiling_step(&surfaces, *coll, *previous, prev, &mut pos, &current);
    let floor = sweep(
        &surfaces,
        Kind::Floor,
        point(prev, Vec2::new(0.0, previous.bottom)),
        point(pos, Vec2::new(0.0, coll.bottom)),
    )
    .map(|(h, _)| h)
    .or_else(|| adjacent_horizontal(&surfaces, &current, Kind::Floor, pos, coll.bottom));
    if let Some(hit) = floor {
        collide_floor(&surfaces, *coll, &mut pos, hit);
        return DefaultColl {
            pos,
            floor: Some(Standing {
                line: hit.line,
                flags: hit.flags,
                normal: hit.normal,
            }),
        };
    }
    DefaultColl {
        pos,
        floor: project_floor(&surfaces, Vec3::new(pos.x, pos.y + coll.bottom, pos.z)),
    }
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
    move_air_steps(coll, previous, from, to, push, options, surfaces, steps)
}

#[allow(clippy::too_many_arguments)]
fn move_air_steps<I, F>(
    coll: &BodyColl,
    previous: &BodyColl,
    from: Vec3,
    to: Vec3,
    push: Vec3,
    options: AirOptions,
    surfaces: F,
    steps: u32,
) -> AirMoved
where
    F: Fn() -> I,
    I: IntoIterator<Item = MapSurface>,
{
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
        let check = options.cliff;
        if check.enabled {
            if let Some((line, corner)) = cliff(
                &surfaces,
                check.facing.sign(),
                check.wait,
                check.reach,
                prev,
                pos,
            ) {
                if !check.occupied.contains(&Some((line, check.facing))) {
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

/// [`move_air`] with `mpProcessSetCollideFloor` in place of the landing
/// (`wpMapProcAll`): a floor met puts the body on its line without landing
/// it, and the sweep goes on (`is_coll_end` stays clear). Returns the last
/// floor met (`floor_line_id`, `floor_angle`) beside the contacts;
/// [`Contacts::floor`] is its `MAP_FLAG_FLOOR`.
pub fn move_air_collide_floor<I, F>(
    coll: &BodyColl,
    from: Vec3,
    to: Vec3,
    surfaces: F,
) -> (Vec3, Contacts, Option<Contact>)
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
    let mut floor_hit = None;
    for _ in 0..steps {
        let prev = pos;
        pos += step;
        let mut current = Contacts::default();
        for kind in [Kind::LeftWall, Kind::RightWall] {
            let lines = walls(&surfaces, *coll, *coll, prev, pos, kind, None);
            if let Some(hit) = correct_wall(&surfaces, *coll, &mut pos, kind, lines) {
                if kind == Kind::LeftWall {
                    contacts.left_wall = Some(hit);
                    current.left_wall = Some(hit);
                } else {
                    contacts.right_wall = Some(hit);
                    current.right_wall = Some(hit);
                }
            }
        }
        let ceil = ceiling_step(&surfaces, *coll, *coll, prev, &mut pos, &current);
        if let Some(hit) = ceil.contact {
            contacts.ceiling = Some(hit);
        }
        let bottom = Vec2::new(0.0, coll.bottom);
        let floor = sweep(
            &surfaces,
            Kind::Floor,
            point(prev, bottom),
            point(pos, bottom),
        )
        .map(|(h, _)| h)
        .or_else(|| adjacent_horizontal(&surfaces, &current, Kind::Floor, pos, coll.bottom));
        if let Some(hit) = floor {
            contacts.floor = true;
            floor_hit = Some(hit);
            collide_floor(&surfaces, *coll, &mut pos, hit);
        }
    }
    (pos, contacts, floor_hit)
}

/// What [`move_air_check_coll_end`] found.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CollEnd {
    /// `translate` after the last substep run: the full move, or the substep
    /// that met the map (`is_coll_end` stops `mpProcessUpdateMain`).
    pub pos: Vec3,
    /// The `mask_curr` bits the tests raised.
    pub mask: u16,
    /// `ceil_line_id`/`ceil_angle` of a ceiling the top crossed.
    pub ceiling: Option<Contact>,
    /// `floor_line_id`/`floor_flags`/`floor_angle` of a floor the bottom
    /// crossed.
    pub floor: Option<Contact>,
}

/// `mpProcessUpdateMain` with `wpMapProcAllCheckCollEnd`: each substep only
/// tests the walls (`mpProcessCheckTest[L/R]WallCollisionAdjNew`), the
/// ceiling (`mpProcessCheckTestCeilCollisionAdjNew`) and the floor
/// (`mpProcessRunFloorCollisionAdjNewNULL`); nothing moves the body. A hit
/// sets `is_coll_end`, which ends the substeps. No wall is run, so
/// `mask_unk` stays clear and the wall-adjacent ceiling and floor probes
/// never apply; `ignore_line_id` is -1, so a pass-through floor counts.
pub fn move_air_check_coll_end<I, F>(coll: &BodyColl, from: Vec3, to: Vec3, surfaces: F) -> CollEnd
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
    let mut out = CollEnd {
        pos: from,
        mask: 0,
        ceiling: None,
        floor: None,
    };
    for _ in 0..steps {
        let prev = out.pos;
        out.pos += step;
        let pos = out.pos;
        for (kind, mask) in [(Kind::LeftWall, MASK_LWALL), (Kind::RightWall, MASK_RWALL)] {
            if walls(&surfaces, *coll, *coll, prev, pos, kind, None)[0].is_some() {
                out.mask |= mask;
            }
        }
        let top = Vec2::new(0.0, coll.top);
        if let Some((hit, _)) = sweep(&surfaces, Kind::Ceiling, point(prev, top), point(pos, top)) {
            out.mask |= MASK_CEIL;
            out.ceiling = Some(hit);
        }
        let bottom = Vec2::new(0.0, coll.bottom);
        if let Some((hit, _)) = sweep(
            &surfaces,
            Kind::Floor,
            point(prev, bottom),
            point(pos, bottom),
        ) {
            out.mask |= MASK_FLOOR;
            out.floor = Some(hit);
        }
        if out.mask != 0 {
            break;
        }
    }
    out
}

/// `mpCollisionGetFCCommonFloor` (`kind` [`Kind::Floor`]) and
/// `mpCollisionGetLRCommon[L/R]Wall`: where `pos` projects onto `line`
/// along its own axis (x for a floor, y for a wall). Returns the signed
/// distance to it (`floor_y - pos.y`, or `wall_x - pos.x`) and its angle,
/// or `None` when `pos` lies past the line's ends (±0.001).
pub fn line_common<I, F>(surfaces: &F, kind: Kind, line: u16, pos: Vec3) -> Option<(f32, Contact)>
where
    F: Fn() -> I,
    I: IntoIterator<Item = MapSurface>,
{
    if matches!(kind, Kind::Floor | Kind::Ceiling) {
        return height(surfaces, kind, line, pos.x).map(|(y, hit)| (y - pos.y, hit));
    }
    let (lo, _) = edge(surfaces, kind, line, false)?;
    let (hi, _) = edge(surfaces, kind, line, true)?;
    if pos.y <= lo.y - 0.001 || hi.y + 0.001 <= pos.y {
        return None;
    }
    let y = pos.y.clamp(lo.y, hi.y);
    // The first segment spanning `y`, as the original's `break` picks.
    for (i, s) in surfaces().into_iter().enumerate() {
        if s.kind != kind || line_id(i, s) != line {
            continue;
        }
        let [x1, y1, x2, y2] = s.coords();
        if (y1 <= y && y <= y2) || (y2 <= y && y <= y1) {
            // `mpCollisionGetLineDistanceLR`.
            let x = if y1 == y2 {
                x1
            } else {
                x1 + div_nonzero((y - y1) * (x2 - x1), y2 - y1)
            };
            return Some((x - pos.x, contact(i, s)));
        }
    }
    None
}

/// `mpCollisionGetEdgeUnder[L/R]LineID` (a floor's left or right end),
/// `mpCollisionGetEdge[Right/Left][U/D]LineID` (a wall's upper or lower
/// end), then `mpCollisionGetLineTypeID`: the line joined at that end.
pub fn line_neighbor<I, F>(surfaces: &F, kind: Kind, line: u16, high: bool) -> Option<(Kind, u16)>
where
    F: Fn() -> I,
    I: IntoIterator<Item = MapSurface>,
{
    neighbor(surfaces, kind, line, high)
}

/// `mpCollisionCheck[Floor/Ceil/LWall/RWall]LineCollisionDiff`: the nearest
/// line of `kind` the point crosses moving `from` to `to` (a moving line by
/// its own motion), and where.
pub fn line_crossing<I, F>(
    surfaces: &F,
    kind: Kind,
    from: Vec2,
    to: Vec2,
) -> Option<(Contact, Vec2)>
where
    F: Fn() -> I,
    I: IntoIterator<Item = MapSurface>,
{
    sweep(surfaces, kind, from, to)
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
    move_ground_pushed(coll, from, to, Vec3::ZERO, line, stop_edge, None, surfaces)
}

/// `mpCommonCheckSetFighterCliffEdge` @ 0x800DD820: past the floor's left
/// end facing left (or its right end facing right) with the stick short of
/// 60, the fighter stands 40 units in from that end, unless a wall within
/// its width stands there.
fn cliff_edge<I, F>(
    surfaces: &F,
    coll: &BodyColl,
    line: u16,
    pos: Vec3,
    c: CliffEdge,
) -> Option<Moved>
where
    F: Fn() -> I,
    I: IntoIterator<Item = MapSurface>,
{
    let (left, _) = edge(surfaces, Kind::Floor, line, false)?;
    let (x, kind, far) = if pos.x <= left.x {
        if !(c.facing == Facing::Left && i32::from(c.stick_x) > -CLIFF_EDGE_STICK_MAX) {
            return None;
        }
        (left.x + CLIFF_EDGE_INSET, Kind::LeftWall, coll.width)
    } else {
        if !(c.facing == Facing::Right && i32::from(c.stick_x) < CLIFF_EDGE_STICK_MAX) {
            return None;
        }
        let (right, _) = edge(surfaces, Kind::Floor, line, true)?;
        (right.x - CLIFF_EDGE_INSET, Kind::RightWall, -coll.width)
    };
    // `mpCollisionGetFCCommonFloor`: the floor's height 40 units in.
    let moved = follow_floor(surfaces, coll, Vec3::new(x, pos.y, pos.z), line);
    moved.floor?;
    let y = moved.pos.y + coll.bottom;
    query(
        surfaces,
        kind,
        Vec2::new(x, y),
        Vec2::new(x + far, coll.center + y - coll.bottom),
    )
    .is_none()
    .then_some(moved)
}

/// [`move_ground`] with `coll_data.vel_push`, added beside the line speed on
/// the first substep. Unlike the line speed it is not part of the displacement
/// `mpProcessUpdateMain` splits.
#[allow(clippy::too_many_arguments)]
pub fn move_ground_pushed<I, F>(
    coll: &BodyColl,
    from: Vec3,
    to: Vec3,
    push: Vec3,
    line: u16,
    stop_edge: bool,
    cliff_edge_query: Option<CliffEdge>,
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
        // `MAP_PROC_TYPE_CLIFFEDGE` ends the pass either way
        // (`is_coll_end`); only a teeter keeps the fighter grounded.
        let mut teeter = None;
        if let Some(c) = cliff_edge_query.filter(|_| moved.floor.is_none()) {
            teeter = Some(cliff_edge(&surfaces, coll, line, pos, c));
            if let Some(Some(edge)) = teeter {
                moved = edge;
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
        if let Some(edge) = teeter {
            contacts.floor = false;
            contacts.floor_edge = edge.is_some();
            return (moved, contacts);
        }
        pos = moved.pos;
        if moved.floor.is_none() || contacts.left_wall.is_some() || contacts.right_wall.is_some() {
            return (moved, contacts);
        }
    }
    // Staying on the floor is `mask_stat`, not `mask_curr`: only the swept
    // new line above raises `MAP_FLAG_FLOOR` on the ground (N64 trace, RE-554).
    let moved = follow_floor(&surfaces, coll, pos, line);
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
/// …and meets the surface's normal at more than this
/// (`syVectorAngleDiff3D`).
const DAMAGE_COLLIDE_ANGLE_MIN: f32 = ssb_engine::math::f_clc_dtor32(110.0);

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

/// `syVectorAngleDiff3D(pos_diff, normal) > F_CLC_DTOR32(110.0F)`
/// (`mpcommon.c:734-799`); a zero vector gives 0, which never strikes.
pub(crate) fn faces(diff: Vec3, normal: Vec2) -> bool {
    ssb_engine::math::sy_vector_angle_diff_3d(diff, Vec3::new(normal.x, normal.y, 0.0))
        > DAMAGE_COLLIDE_ANGLE_MIN
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

/// The statuses whose `proc_map` is `mpCommonProcFighterOnCliffEdge`
/// (`ftcommonstatus.h`; `Dash` once it may run, `ftCommonDashProcMap`).
pub(crate) fn teeters_at_edge(f: &Fighter) -> bool {
    match f.status.status {
        AnyStatus::Common(Status::Dash) => f.attributes.dash_to_run <= f.status.anim_frame,
        AnyStatus::Common(s) => matches!(
            s,
            Status::Wait
                | Status::WalkSlow
                | Status::WalkMiddle
                | Status::WalkFast
                | Status::RunBrake
                | Status::LandingLight
                | Status::LandingHeavy
                | Status::LandingFallSpecial
                | Status::Appeal
        ),
        _ => false,
    }
}

/// The grounded statuses whose map pass is `MAP_PROC_TYPE_STOPEDGE`
/// (`mpCommonCheckFighterOnEdge`): `mpCommonSetFighterFallOnEdgeBreak` in
/// the status tables (`ftcommonstatus.h`, `ft*status.h`), and the
/// `proc_map`s that call `mpCommonProcFighterOnEdge` or check
/// `mpCommonCheckFighterOnEdge` themselves (catch, heavy throw, item
/// shoot, Kirby's Final Cutter and the fighters' ground specials). They
/// stop at the floor's end instead of walking off it (RE-524: Kirby's
/// getup roll).
pub(crate) fn stops_at_edge(s: AnyStatus) -> bool {
    use status::{DonkeyStatus as D, LinkStatus as L, MarioStatus as M, SamusStatus as S};
    match s {
        AnyStatus::Common(s) => matches!(
            s,
            Status::LightThrowDrop
                | Status::LightThrowDash
                | Status::LightThrowF
                | Status::LightThrowB
                | Status::LightThrowHi
                | Status::LightThrowLw
                | Status::LightThrowF4
                | Status::LightThrowB4
                | Status::LightThrowHi4
                | Status::LightThrowLw4
                | Status::HeavyThrowF
                | Status::HeavyThrowB
                | Status::HeavyThrowF4
                | Status::HeavyThrowB4
                | Status::SwordSwing1
                | Status::SwordSwing3
                | Status::SwordSwing4
                | Status::SwordSwingDash
                | Status::BatSwing1
                | Status::BatSwing3
                | Status::BatSwing4
                | Status::BatSwingDash
                | Status::HarisenSwing1
                | Status::HarisenSwing3
                | Status::HarisenSwing4
                | Status::HarisenSwingDash
                | Status::StarRodSwing1
                | Status::StarRodSwing3
                | Status::StarRodSwing4
                | Status::StarRodSwingDash
                | Status::LGunShoot
                | Status::FireFlowerShoot
                | Status::EscapeF
                | Status::EscapeB
                | Status::Catch
                | Status::CatchPull
                | Status::CatchWait
                | Status::ThrowF
                | Status::ThrowB
                | Status::Attack11
                | Status::Attack12
                | Status::AttackDash
                | Status::AttackS3Hi
                | Status::AttackS3HiS
                | Status::AttackS3
                | Status::AttackS3LwS
                | Status::AttackS3Lw
                | Status::AttackHi3F
                | Status::AttackHi3
                | Status::AttackHi3B
                | Status::AttackLw3
                | Status::AttackS4Hi
                | Status::AttackS4HiS
                | Status::AttackS4
                | Status::AttackS4LwS
                | Status::AttackS4Lw
                | Status::AttackHi4
                | Status::AttackLw4
                | Status::DokanStart
                | Status::DokanEnd
                | Status::PassiveStandF
                | Status::PassiveStandB
                | Status::DownForwardD
                | Status::DownForwardU
                | Status::DownBackD
                | Status::DownBackU
                | Status::DownAttackD
                | Status::DownAttackU
        ),
        // Luigi shares Mario's table.
        AnyStatus::Mario(s) => matches!(s, M::Attack13 | M::SpecialN),
        AnyStatus::Fox(s) => matches!(
            s,
            FoxStatus::Attack100Start
                | FoxStatus::Attack100Loop
                | FoxStatus::Attack100End
                | FoxStatus::SpecialN
                | FoxStatus::SpecialLwStart
                | FoxStatus::SpecialLwHit
                | FoxStatus::SpecialLwEnd
                | FoxStatus::SpecialLwLoop
                | FoxStatus::SpecialLwTurn
        ),
        AnyStatus::Donkey(s) => matches!(
            s,
            D::SpecialNStart
                | D::SpecialNLoop
                | D::SpecialNEnd
                | D::SpecialNFull
                | D::SpecialLwStart
                | D::SpecialLwLoop
                | D::SpecialLwEnd
        ),
        AnyStatus::Samus(s) => {
            matches!(
                s,
                S::SpecialNStart | S::SpecialNLoop | S::SpecialNEnd | S::SpecialLw
            )
        }
        AnyStatus::Link(s) => matches!(
            s,
            L::Attack13 | L::Attack100Start | L::Attack100Loop | L::Attack100End
        ),
        // `ftCaptainSpecialLwDecideMapCollide` stops only once `flag3` is
        // set: `crate::captain::special_lw_stops_at_edge`.
        AnyStatus::Captain(s) => matches!(
            s,
            CaptainStatus::Attack13
                | CaptainStatus::Attack100Start
                | CaptainStatus::Attack100Loop
                | CaptainStatus::Attack100End
                | CaptainStatus::SpecialN
                | CaptainStatus::SpecialLwLanding
        ),
        AnyStatus::Kirby(s) => matches!(
            s,
            KirbyStatus::Attack100Start
                | KirbyStatus::Attack100Loop
                | KirbyStatus::Attack100End
                | KirbyStatus::CopyMarioSpecialN
                | KirbyStatus::CopyLuigiSpecialN
                | KirbyStatus::CopyFoxSpecialN
                | KirbyStatus::CopySamusSpecialNStart
                | KirbyStatus::CopySamusSpecialNLoop
                | KirbyStatus::CopySamusSpecialNEnd
                | KirbyStatus::CopyDonkeySpecialNStart
                | KirbyStatus::CopyDonkeySpecialNLoop
                | KirbyStatus::CopyDonkeySpecialNEnd
                | KirbyStatus::CopyDonkeySpecialNFull
                | KirbyStatus::SpecialHi
                | KirbyStatus::SpecialNThrow
                | KirbyStatus::CopyCaptainSpecialN
        ),
        AnyStatus::Pikachu(s) => matches!(
            s,
            PikachuStatus::SpecialN
                | PikachuStatus::SpecialLwStart
                | PikachuStatus::SpecialLwLoop
                | PikachuStatus::SpecialLwHit
                | PikachuStatus::SpecialLwEnd
        ),
        // Jigglypuff's Attack100 statuses (220 to 222) are not ported.
        AnyStatus::Purin(s) => matches!(s, PurinStatus::SpecialN | PurinStatus::SpecialHi),
        AnyStatus::Ness(s) => matches!(
            s,
            NessStatus::Attack13
                | NessStatus::SpecialN
                | NessStatus::SpecialHiStart
                | NessStatus::SpecialHiHold
                | NessStatus::SpecialHiEnd
                | NessStatus::SpecialLwStart
                | NessStatus::SpecialLwHold
                | NessStatus::SpecialLwHit
                | NessStatus::SpecialLwEnd
        ),
        AnyStatus::Yoshi(_) | AnyStatus::Boss(_) => false,
    }
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
            PikachuStatus::SpecialAirHiStart
                | PikachuStatus::SpecialAirHi
                | PikachuStatus::SpecialAirHiEnd
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
                    && f.status.clock
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
        // `ftMarioSpecialHiProcMap`: once motion flag 1 launches the jump,
        // a fall catches ledges (`mpCommonCheckFighterPassCliff`); rising,
        // it only projects (RE-468: Mario's recovery in How to Play).
        AnyStatus::Mario(status::MarioStatus::SpecialHi | status::MarioStatus::SpecialAirHi) => {
            f.motion_script.flags[1] != 0 && f.physics.vel_air.y < 0.0
        }
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

/// `mpCollisionGetFCCommonFloor`'s `dist` for a point at `(x, y)`: the
/// line's height under `x` less `y`, both taken relative to a moving
/// line's offset as the original does (`object_pos - yakumono translate`).
pub fn floor_distance<I, F>(surfaces: &F, line: u16, x: f32, y: f32) -> Option<f32>
where
    F: Fn() -> I,
    I: IntoIterator<Item = MapSurface>,
{
    let offset = surfaces()
        .into_iter()
        .enumerate()
        .find(|(i, s)| s.kind == Kind::Floor && line_id(*i, *s) == line)?
        .1
        .motion
        .map(|m| m.offset);
    let (ox, oy) = offset.map_or((x, y), |o| (x - o.x, y - o.y));
    let f = collision::floor_height(
        surfaces().into_iter().enumerate().filter_map(|(i, s)| {
            (s.kind == Kind::Floor && line_id(i, s) == line).then_some((line, s.segment))
        }),
        ox,
    )?;
    Some(f.y - oy)
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
    false
}

#[cfg(test)]
pub(crate) fn air_callback(f: &mut Fighter) {
    air_callback_at(f, f.pos);
}

/// The air statuses' `proc_map` callbacks; `translate` is TopN's
/// translation when the map process began, before the collision moved it.
pub(crate) fn air_callback_at(f: &mut Fighter, translate: Vec3) {
    let contacts = f.map_contacts;
    if matches!(
        f.status.status,
        AnyStatus::Common(Status::ThrownKirbyStar | Status::ThrownCopyStar)
    ) {
        // Ceiling, then left wall, then right wall; the wave on the struck
        // side of the collision diamond.
        let (c, t) = (f.coll, translate);
        let hit = if let Some(hit) = contacts.ceiling {
            Some((hit, Vec3::new(t.x, t.y + c.top, t.z)))
        } else if let Some(hit) = contacts.left_wall {
            Some((hit, Vec3::new(t.x + c.width, t.y + c.center, t.z)))
        } else {
            contacts
                .right_wall
                .map(|hit| (hit, Vec3::new(t.x - c.width, t.y + c.center, t.z)))
        };
        if let Some((hit, wave)) = hit {
            crate::capture_kirby::on_landing(f, f.pos.y, hit.normal, wave);
        }
        return;
    }
    if f.status.status == AnyStatus::Pikachu(PikachuStatus::SpecialAirHi) {
        for (found, hit) in
            [contacts.ceiling, contacts.left_wall, contacts.right_wall].map(contact_or_none)
        {
            if !found {
                continue;
            }
            if crate::pikachu::halts(hit.normal, f.physics.vel_air) {
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
            let (found, hit) = contact_or_none(hit);
            if found && prev.is_none() {
                if ssb_engine::math::lb_common_check_adjust_sim_2d(
                    &mut f.physics.vel_air,
                    Vec3::new(hit.normal.x, hit.normal.y, 0.0),
                    crate::status::FIREFOX_BOUND_ANGLE,
                ) {
                    f.facing = if f.physics.vel_air.x >= 0.0 {
                        Facing::Right
                    } else {
                        Facing::Left
                    };
                    // `fp->joints[nFTPartsJointTopN]->rotate.vec.f.y = lr *
                    // F_CST_DTOR32(90)`, the bits `F_CLC_DTOR32(90)` has.
                    f.topn_lr = f.facing.sign();
                    f.fox_special_hi.angle = ssb_engine::math::sy_arc_tan2(
                        f.physics.vel_air.y,
                        f.physics.vel_air.x * f.facing.sign(),
                    );
                    // `ftFoxSpecialHiUpdateModelPitch` (`ftfoxspecialhi.c:266`).
                    crate::model_pitch::special_hi(f);
                }
                // Source gives ceiling, LWall, RWall priority, even when
                // the chosen contact is too steep to redirect.
                break;
            }
        }
    } else {
        crate::captain::special_lw_proc_map(f);
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
    // A flag beside written values, as in [`edge`].
    let mut found = false;
    let mut best = (0u16, 0.0f32);
    for (i, s) in surfaces().into_iter().enumerate() {
        if s.kind != Kind::Floor {
            continue;
        }
        let o = s.motion.map_or(Vec2::ZERO, |m| m.offset);
        let local = Vec2::new(pos.x - o.x, pos.y - o.y);
        let Some(hit) = collision::project_floor([(line_id(i, s), s.segment)], local) else {
            continue;
        };
        if !found || hit.dist.abs() < best.1.abs() {
            found = true;
            best = (hit.line, hit.dist);
        }
    }
    found.then_some(best)
}

/// `mpCollisionCheckProjectFloor`'s line id, flags and angle: the nearest
/// floor at or below `pos` (see [`project_floor_line`]).
pub fn project_floor<I, F>(surfaces: &F, pos: Vec3) -> Option<Standing>
where
    F: Fn() -> I,
    I: IntoIterator<Item = MapSurface>,
{
    // A flag beside written values, as in [`edge`].
    let mut found = false;
    let mut best = (Standing::NONE, 0.0f32);
    for (i, s) in surfaces().into_iter().enumerate() {
        if s.kind != Kind::Floor {
            continue;
        }
        let o = s.motion.map_or(Vec2::ZERO, |m| m.offset);
        let local = Vec2::new(pos.x - o.x, pos.y - o.y);
        let Some(hit) = collision::project_floor([(line_id(i, s), s.segment)], local) else {
            continue;
        };
        if !found || hit.dist.abs() < best.1.abs() {
            found = true;
            best = (
                Standing {
                    line: hit.line,
                    flags: hit.flags,
                    normal: hit.normal,
                },
                hit.dist,
            );
        }
    }
    found.then_some(best.0)
}

/// `mpCollisionCheckProjectRWall`'s `ga_dist`: the signed X distance from
/// `pos` to the nearest right wall at or left of it whose span covers
/// `pos.y` (a moving group's line where it stands now).
pub fn project_rwall_dist<I, F>(surfaces: &F, pos: Vec3) -> Option<f32>
where
    F: Fn() -> I,
    I: IntoIterator<Item = MapSurface>,
{
    // A flag beside a written value, as in [`edge`].
    let mut found = false;
    let mut best = 0.0f32;
    for s in surfaces().into_iter() {
        if s.kind != Kind::RightWall {
            continue;
        }
        let [x1, y1, x2, y2] = s.coords();
        if !((y1 <= pos.y && y2 >= pos.y) || (y2 <= pos.y && y1 >= pos.y)) {
            continue;
        }
        // `mpCollisionGetLineDistanceLR`.
        let x = if y1 == y2 {
            x1
        } else {
            x1 + div_nonzero((pos.y - y1) * (x2 - x1), y2 - y1)
        };
        let dist = x - pos.x;
        if x <= pos.x && (!found || dist.abs() < best.abs()) {
            found = true;
            best = dist;
        }
    }
    found.then_some(best)
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

/// `mpCollisionCheckExistLineID`: the runtime leaves out the lines of a
/// group that is off or hidden, so a line exists when a surface names it.
pub fn line_exists<I, F>(surfaces: &F, line: u16) -> bool
where
    F: Fn() -> I,
    I: IntoIterator<Item = MapSurface>,
{
    surfaces()
        .into_iter()
        .enumerate()
        .any(|(i, s)| line_id(i, s) == line)
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
