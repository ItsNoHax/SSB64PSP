//! Stage collision queries, ported from `mp/mpcollision.c`.
//!
//! Smash does not test a point against a surface; it tests the **swept
//! segment** from where a fighter was to where it wants to be. That is what
//! stops a fast faller from passing through a platform in one frame, and it is
//! why every query here takes `from` and `to` rather than a position.
//!
//! Collision is 2D polylines (RE-029). A caller walks the lines it cares about
//! and yields their segments; nothing here owns storage, so the same code runs
//! against a pack on the PSP and against a fixture in a test.
//!
//! ## What a vertex's flags mean
//!
//! From `mpdef.h`: the upper byte is surface bits, the lower byte the material
//! that sets friction. Two bits decide how a floor behaves —
//! [`flags::PASS`] (drop-through) and [`flags::CLIFF`] (ledge-grabbable).
//! Dream Land is the check: its three floating platforms are `PASS`, its main
//! platform is `CLIFF` and not `PASS`, and its ceiling and walls are neither.
//! That is exactly how the stage plays.
//!
//! ## Limits
//!
//! Segments are taken in world space. A line owned by a moving group
//! ("yakumono") is stored in that group's own space, and the original adds the
//! group's `DObj` translation before testing; until stage animation exists the
//! caller must not feed those lines in, or they will be tested where they
//! rest rather than where they are.

use ssb_engine::math::Vec2;

/// Surface bits in a collision vertex's flags.
pub mod flags {
    /// `MAP_VERTEX_COLL_PASS`: a fighter may drop through this floor.
    pub const PASS: u16 = 1 << 14;
    /// `MAP_VERTEX_COLL_CLIFF`: this floor's ends can be hung from.
    pub const CLIFF: u16 = 1 << 15;
    /// The `MPMaterial` in the low byte, which selects friction.
    pub const MATERIAL: u16 = 0x00FF;
}

/// The slack the original allows on every edge test, in game units.
///
/// Not a tuning knob: `0.001` appears literally throughout `mpcollision.c`,
/// and it is what lets a fighter standing exactly on a surface still register
/// as touching it.
const EPS: f32 = 0.001;

/// One segment of a collision polyline.
///
/// Positions stay `i16` because that is how the ROM stores them and how the
/// original reads them — `mpCollisionCheckFloorSurfaceTilt` takes its vertices
/// as `s32` and only the moving object's position as `f32`. Widening them
/// would change which side of a boundary a coordinate falls on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Segment {
    pub x1: i16,
    pub y1: i16,
    pub x2: i16,
    pub y2: i16,
    /// The flags of the segment's *first* vertex, which is the one the
    /// original reports through `stand_coll_flags`.
    pub flags: u16,
}

impl Segment {
    /// Whether a fighter can drop through this surface.
    pub fn passable(&self) -> bool {
        self.flags & flags::PASS != 0
    }

    /// Whether this surface's ends can be ledge-grabbed.
    pub fn grabbable(&self) -> bool {
        self.flags & flags::CLIFF != 0
    }

    /// The `MPMaterial` index that selects ground friction.
    pub fn material(&self) -> u16 {
        self.flags & flags::MATERIAL
    }
}

/// Where a swept movement met a floor.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FloorHit {
    /// The point on the surface, in world units.
    pub point: Vec2,
    /// The line the surface belongs to, as `stand_line_id` reports it.
    pub line: u16,
    /// The surface flags at that point.
    pub flags: u16,
    /// Unit surface normal, pointing up out of the floor.
    pub normal: Vec2,
}

/// Finds the floor a movement from `from` to `to` crosses.
///
/// Ports `mpCollisionCheckFloorLineCollisionSame`. `segments` yields
/// `(line_id, segment)` for every floor segment worth testing; when several
/// are crossed the one nearest the starting height wins, and ties go to the
/// first yielded, matching the original's `line_project_pos <= ...` test.
///
/// The iterator is taken by value and monomorphised, so there is no dynamic
/// dispatch and nothing is allocated: the caller decides what storage the
/// segments come from.
pub fn check_floor<I>(segments: I, from: Vec2, to: Vec2) -> Option<FloorHit>
where
    I: IntoIterator<Item = (u16, Segment)>,
{
    let mut best: Option<FloorHit> = None;
    let mut best_dist = f32::MAX;

    for (line, s) in segments {
        // The original splits on whether the segment is level, because a level
        // one makes the tilted solver divide by zero.
        let hit = if s.y1 == s.y2 {
            // And it only tests level floors while moving *downward*: standing
            // still or rising can never land on one.
            if to.y >= from.y {
                continue;
            }
            check_flat(&s, from, to)
        } else {
            check_tilt(&s, from, to)
        };
        let Some(point) = hit else { continue };

        let dist = (point.y - from.y).abs();
        if dist >= best_dist {
            continue;
        }
        best_dist = dist;
        best = Some(FloorHit {
            point,
            line,
            flags: s.flags,
            normal: normal_of(&s),
        });
    }
    best
}

/// The upward unit normal of a floor segment, from `mpCollisionGetFCAngle`
/// with `ud = +1`.
fn normal_of(s: &Segment) -> Vec2 {
    let dy = (s.y2 - s.y1) as f32;
    if dy == 0.0 {
        return Vec2::new(0.0, 1.0);
    }
    let slope = -(dy / (s.x2 - s.x1) as f32);
    let n = Vec2::new(slope, 1.0);
    let len = n.length();
    if len == 0.0 {
        return Vec2::new(0.0, 1.0);
    }
    Vec2::new(n.x / len, n.y / len)
}

/// Ports `mpCollisionCheckFCSurfaceFlat`: a level floor segment.
///
/// The caller guarantees `to.y < from.y`, so the vertical span is never zero
/// and the division below is safe. The original has no such guard and would
/// return a NaN intersection if one ever reached it with level motion — the
/// `is_finite` check keeps that from becoming a silent teleport here.
fn check_flat(s: &Segment, from: Vec2, to: Vec2) -> Option<Vec2> {
    let y = s.y1 as f32;
    let span_y = from.y - to.y;

    // The movement must cross the surface's height, from above.
    if span_y > 0.0 {
        if y - EPS > from.y || to.y >= y {
            return None;
        }
    } else if y + EPS < from.y || to.y <= y {
        return None;
    }

    let span_x = from.x - to.x;
    let (near, far) = min_max(s.x1, s.x2);

    // And it must overlap the segment horizontally.
    if span_x > 0.0 {
        if far < to.x || from.x < near {
            return None;
        }
    } else if far < from.x || to.x < near {
        return None;
    }

    // Where the movement was when it reached the surface's height.
    let x = ((y - from.y) / span_y) * span_x + from.x;
    if !x.is_finite() || x < near || x > far {
        return None;
    }
    Some(Vec2::new(x, y))
}

/// Ports `mpCollisionCheckFloorSurfaceTilt`: a sloped floor segment.
fn check_tilt(s: &Segment, from: Vec2, to: Vec2) -> Option<Vec2> {
    let (v1x, v1y) = (s.x1 as f32, s.y1 as f32);
    let vdist_x = (s.x2 - s.x1) as f32;
    let vdist_y = (s.y2 - s.y1) as f32;
    let span_x = from.x - to.x;
    let span_y = from.y - to.y;

    // Bounding-box rejection on both axes, with the movement's direction
    // deciding which end of it to compare against.
    let (near_y, far_y) = min_max(s.y1, s.y2);
    if span_y > 0.0 {
        if far_y + EPS < to.y || from.y < near_y - EPS {
            return None;
        }
    } else if far_y + EPS < from.y || to.y < near_y - EPS {
        return None;
    }

    let (near_x, far_x) = min_max(s.x1, s.x2);
    if span_x > 0.0 {
        if far_x < to.x || from.x < near_x {
            return None;
        }
    } else if far_x < from.x || to.x < near_x {
        return None;
    }

    // The destination has to end up below the surface's line; otherwise the
    // movement stayed above it and there is nothing to land on.
    if to.y - (v1y + ((to.x - v1x) / vdist_x) * vdist_y) > -EPS {
        return None;
    }

    // How far the *start* was above the surface.
    let above = from.y - (v1y + ((from.x - v1x) / vdist_x) * vdist_y);
    if above < EPS {
        // Already on the surface, within tolerance: the contact point is
        // directly below where the movement began.
        if above > -EPS && from.x <= far_x && near_x <= from.x {
            return Some(Vec2::new(
                from.x,
                v1y + ((from.x - v1x) / vdist_x) * vdist_y,
            ));
        }
        return None;
    }

    // Started clear of the surface and ended below it: solve the crossing.
    let dx = v1x - from.x;
    let dy = v1y - from.y;
    let scale = vdist_y * span_x - vdist_x * span_y;
    let mut num = span_y * dx - span_x * dy;

    // `num / scale` is how far along the segment the crossing is. The original
    // snaps a slight overshoot to the end rather than rejecting it, so a
    // fighter landing exactly on a joint between two segments lands on one of
    // them instead of falling between.
    let t = num / scale;
    if t < 0.0 {
        if t < -EPS {
            return None;
        }
        num = 0.0;
    } else if t > 1.0 {
        if t > 1.0 + EPS {
            return None;
        }
        num = scale;
    }

    // The same fraction along the *movement*, which must also be within it.
    let along = (vdist_x * dy - vdist_y * dx) / scale;
    if !(-EPS..=1.0 + EPS).contains(&along) {
        return None;
    }

    let inv = 1.0 / scale;
    Some(Vec2::new(
        v1x + num * vdist_x * inv,
        v1y + num * vdist_y * inv,
    ))
}

fn min_max(a: i16, b: i16) -> (f32, f32) {
    if b < a {
        (b as f32, a as f32)
    } else {
        (a as f32, b as f32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Dream Land's main platform: level, `(-2318, 0) .. (2318, 0)`, with
    /// ledges you can hang from and no drop-through.
    const MAIN: Segment = Segment {
        x1: -2318,
        y1: 0,
        x2: 2318,
        y2: 0,
        flags: flags::CLIFF,
    };

    /// One of its floating platforms, which you *can* drop through.
    const PLATFORM: Segment = Segment {
        x1: 951,
        y1: 907,
        x2: 1421,
        y2: 907,
        flags: flags::PASS,
    };

    fn v(x: f32, y: f32) -> Vec2 {
        Vec2::new(x, y)
    }

    #[test]
    fn falling_onto_a_level_floor_lands_on_it() {
        let hit = check_floor([(3, MAIN)], v(100.0, 50.0), v(100.0, -10.0)).expect("landed");
        assert_eq!(hit.line, 3);
        assert!((hit.point.y - 0.0).abs() < 1e-3);
        assert!((hit.point.x - 100.0).abs() < 1e-3, "fell straight down");
        assert_eq!(hit.normal, v(0.0, 1.0));
    }

    #[test]
    fn a_fast_fall_cannot_tunnel_through_the_stage() {
        // The whole reason the query is swept: at 400 units a frame both ends
        // of the movement are clear of the surface, and a point test would see
        // nothing at either one.
        let hit = check_floor([(3, MAIN)], v(0.0, 200.0), v(0.0, -200.0));
        assert!(hit.is_some(), "a point test would have missed this");
    }

    #[test]
    fn rising_through_a_level_floor_does_not_land() {
        // Jumping up through a platform from below must pass, whatever its
        // flags say -- level floors are only tested while falling.
        assert!(check_floor([(1, PLATFORM)], v(1000.0, 500.0), v(1000.0, 1000.0)).is_none());
    }

    #[test]
    fn moving_beside_a_floor_misses_it() {
        // Falling well to the right of the platform's end.
        assert!(check_floor([(1, PLATFORM)], v(3000.0, 950.0), v(3000.0, 800.0)).is_none());
    }

    #[test]
    fn a_diagonal_fall_lands_where_it_crossed_not_where_it_ended() {
        // From (0, 100) to (200, -100): crosses y = 0 halfway, at x = 100.
        let hit = check_floor([(3, MAIN)], v(0.0, 100.0), v(200.0, -100.0)).expect("landed");
        assert!(
            (hit.point.x - 100.0).abs() < 1e-2,
            "crossed at x = {}",
            hit.point.x
        );
    }

    #[test]
    fn the_nearest_surface_wins() {
        // Falling from above the platform, through it, and on toward the main
        // floor: both are crossed, and the platform is the one landed on.
        let hit = check_floor(
            [(3, MAIN), (1, PLATFORM)],
            v(1000.0, 1200.0),
            v(1000.0, -100.0),
        )
        .expect("landed");
        assert_eq!(hit.line, 1, "should stop at the platform, not fall past it");
        assert!((hit.point.y - 907.0).abs() < 1e-3);
    }

    #[test]
    fn a_slope_reports_its_tilt() {
        // Rises 100 over 100: a 45-degree ramp.
        let ramp = Segment {
            x1: 0,
            y1: 0,
            x2: 100,
            y2: 100,
            flags: 0,
        };
        let hit = check_floor([(0, ramp)], v(50.0, 200.0), v(50.0, -50.0)).expect("landed");
        assert!(
            (hit.point.y - 50.0).abs() < 1e-2,
            "halfway up the ramp: {}",
            hit.point.y
        );
        // The normal leans away from the climb and stays unit length.
        assert!(hit.normal.x < 0.0, "normal leans back down the slope");
        assert!(hit.normal.y > 0.0, "and still points up");
        assert!((hit.normal.length() - 1.0).abs() < 1e-4);
    }

    #[test]
    fn a_slope_is_missed_when_the_movement_stays_above_it() {
        let ramp = Segment {
            x1: 0,
            y1: 0,
            x2: 100,
            y2: 100,
            flags: 0,
        };
        assert!(check_floor([(0, ramp)], v(50.0, 200.0), v(50.0, 120.0)).is_none());
    }

    #[test]
    fn surface_flags_come_back_with_the_hit() {
        let hit = check_floor([(3, MAIN)], v(0.0, 50.0), v(0.0, -50.0)).unwrap();
        assert_eq!(hit.flags, flags::CLIFF);
        assert!(MAIN.grabbable(), "Dream Land's main platform has ledges");
        assert!(!MAIN.passable(), "and you cannot drop through it");

        let hit = check_floor([(1, PLATFORM)], v(1000.0, 950.0), v(1000.0, 850.0)).unwrap();
        assert_eq!(hit.flags, flags::PASS);
        assert!(PLATFORM.passable(), "its floating platforms are soft");
    }

    #[test]
    fn the_material_is_the_low_byte() {
        let icy = Segment {
            flags: flags::CLIFF | 3,
            ..MAIN
        };
        assert_eq!(icy.material(), 3);
        assert!(icy.grabbable(), "the surface bits survive the mask");
    }

    #[test]
    fn no_movement_lands_on_nothing() {
        // Standing still is not a crossing, so a resting fighter re-running the
        // query must not be re-placed every frame.
        assert!(check_floor([(3, MAIN)], v(0.0, 0.0), v(0.0, 0.0)).is_none());
    }

    #[test]
    fn an_empty_stage_is_not_an_error() {
        assert!(check_floor([], v(0.0, 100.0), v(0.0, -100.0)).is_none());
    }
}
