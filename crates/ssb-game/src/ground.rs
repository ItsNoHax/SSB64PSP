//! Moving a body against the stage, ported from `mp/mpprocess.c`.
//!
//! [`collision`](crate::collision) answers questions about surfaces. This
//! module is what asks them each tick: it takes where a body was and where its
//! velocity wants it to go, and returns where it actually ends up.
//!
//! Two behaviours here matter more than they look:
//!
//! * **Substepping.** `mpProcessUpdateMain` splits a tick's movement into
//!   250-unit pieces before testing any of them. Maximum knockback velocity is
//!   2500 units per frame, so without this a launched fighter would cross a
//!   whole stage between two collision tests and land on nothing.
//! * **Following the floor.** A grounded fighter is not left at whatever
//!   height it had last frame; `mpProcessSetCollideFloor` re-reads the
//!   surface under its new x every tick. That is what walking up Dream Land's
//!   slope is — there is no separate slope code.
//!
//! ## Limits
//!
//! Floors only. Walls and ceilings (`mpProcessCheckTestLWallCollision` and
//! friends) are not ported, so a body may pass sideways through a wall and
//! nothing stops it at a ceiling.

use crate::collision::{self, FloorBelow, Segment};
use ssb_engine::math::{Vec2, Vec3};

/// A body's collision offsets — `MPObjectColl`.
///
/// These are offsets from the body's origin to the corners of the collision
/// diamond the original tests with. Only `bottom` is used so far, because only
/// floors are ported: the floor query runs at `pos.y + bottom`, and landing
/// puts the body back at `surface - bottom`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BodyColl {
    pub top: f32,
    pub bottom: f32,
    pub left: f32,
    pub right: f32,
}

impl Default for BodyColl {
    fn default() -> Self {
        // A fighter's origin sits at its feet, so the floor probe is the
        // origin itself. Real per-character values come from `FTAttributes`.
        BodyColl {
            top: 0.0,
            bottom: 0.0,
            left: 0.0,
            right: 0.0,
        }
    }
}

/// The floor a body is standing on.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Standing {
    /// The line id, as `coll_data->floor_line_id` holds it.
    pub line: u16,
    /// The surface flags under the body — see [`collision::flags`].
    pub flags: u16,
    /// Unit surface normal, pointing up out of the floor.
    pub normal: Vec2,
}

impl Standing {
    /// Whether this surface can be dropped through.
    pub fn passable(&self) -> bool {
        self.flags & collision::flags::PASS != 0
    }

    /// Whether this surface's ends can be ledge-grabbed.
    pub fn grabbable(&self) -> bool {
        self.flags & collision::flags::CLIFF != 0
    }

    /// The `MPMaterial` index that selects ground friction.
    pub fn material(&self) -> u16 {
        self.flags & collision::flags::MATERIAL
    }
}

/// What a tick's movement did against the stage.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Moved {
    /// Where the body actually ended up.
    pub pos: Vec3,
    /// The floor it is on, if it landed on or stayed on one.
    pub floor: Option<Standing>,
}

/// The movement size above which `mpProcessUpdateMain` subdivides a tick.
///
/// A tenth of maximum knockback velocity, per the original's own comment.
pub const SUBSTEP_SPAN: f32 = 250.0;

/// How many pieces `mpProcessUpdateMain` splits this tick's movement into.
///
/// The original derives this from the larger axis and adds one, so a movement
/// at exactly the threshold still gets a single step. It never clamps: at the
/// game's maximum velocity the count reaches 10 on its own.
pub fn substep_count(from: Vec3, to: Vec3) -> u32 {
    let dx = (to.x - from.x).abs();
    let dy = (to.y - from.y).abs();
    if dx <= SUBSTEP_SPAN && dy <= SUBSTEP_SPAN {
        return 1;
    }
    let larger = if dx > dy { dx } else { dy };
    (larger / SUBSTEP_SPAN) as u32 + 1
}

/// Moves an airborne body from `from` to `to`, landing it if it crosses a floor.
///
/// Ports the floor path of `mpProcessUpdateMain` +
/// `mpProcessCheckTestFloorCollisionAdjNew` + `mpProcessSetLandingFloor`.
///
/// `floors` is called once per substep and must yield every floor segment
/// worth testing as `(line_id, segment)`. It is a closure rather than a single
/// iterator because the movement may be tested several times; taking it by
/// value would consume it on the first pass. Nothing is allocated and the call
/// is monomorphised, so on the PSP this reads straight out of the pack.
///
/// `ignore_line` is the drop-through platform a fighter is currently falling
/// past: a floor is skipped when it is passable *and* is that line, exactly as
/// the original's `ignore_line_id` test reads.
pub fn move_air<I, F>(
    coll: &BodyColl,
    from: Vec3,
    to: Vec3,
    ignore_line: Option<u16>,
    floors: F,
) -> Moved
where
    F: Fn() -> I,
    I: IntoIterator<Item = (u16, Segment)>,
{
    let steps = substep_count(from, to);
    let step = Vec3::new(
        (to.x - from.x) / steps as f32,
        (to.y - from.y) / steps as f32,
        (to.z - from.z) / steps as f32,
    );

    let mut pos = from;
    for _ in 0..steps {
        let prev = pos;
        pos = Vec3::new(pos.x + step.x, pos.y + step.y, pos.z + step.z);

        let a = Vec2::new(prev.x, prev.y + coll.bottom);
        let b = Vec2::new(pos.x, pos.y + coll.bottom);

        let Some(hit) = collision::check_floor(floors(), a, b) else {
            continue;
        };
        // A platform being dropped through is not a floor this tick.
        if hit.flags & collision::flags::PASS != 0 && Some(hit.line) == ignore_line {
            continue;
        }
        // The original stops the substep loop on the first contact rather than
        // continuing to the requested position.
        return land(coll, pos, hit.line, hit.flags, hit.normal, floors);
    }
    Moved { pos, floor: None }
}

/// Settles a body onto the line it just hit — `mpProcessSetLandingFloor`.
///
/// The body keeps the x it arrived at and takes the surface's height there.
/// When that x is past the end of the line, the original does not leave it
/// hanging: it moves the body to the corner it went off, which is what puts a
/// fighter exactly on a ledge instead of beside it.
fn land<I, F>(coll: &BodyColl, pos: Vec3, line: u16, flags: u16, normal: Vec2, floors: F) -> Moved
where
    F: Fn() -> I,
    I: IntoIterator<Item = (u16, Segment)>,
{
    let same_line = || floors().into_iter().filter(|(l, _)| *l == line);

    if let Some(f) = collision::floor_height(same_line(), pos.x) {
        return Moved {
            pos: Vec3::new(pos.x, f.y - coll.bottom, pos.z),
            floor: Some(Standing {
                line,
                flags: f.flags,
                normal: f.normal,
            }),
        };
    }
    if let Some(edge) = collision::line_edge(same_line(), pos.x) {
        return Moved {
            pos: Vec3::new(edge.x, edge.y - coll.bottom, pos.z),
            floor: Some(Standing {
                line,
                flags,
                normal,
            }),
        };
    }
    Moved {
        pos,
        floor: Some(Standing {
            line,
            flags,
            normal,
        }),
    }
}

/// Moves a grounded body along its floor — `mpProcessSetCollideFloor`.
///
/// The body's y is not carried over from last tick; it is re-read from the
/// surface under its new x. Returns `floor: None` when the body has walked off
/// the end of its line, which is the caller's cue to become airborne.
pub fn move_ground<I, F>(coll: &BodyColl, pos: Vec3, line: u16, floors: F) -> Moved
where
    F: Fn() -> I,
    I: IntoIterator<Item = (u16, Segment)>,
{
    let same_line = floors().into_iter().filter(|(l, _)| *l == line);

    match collision::floor_height(same_line, pos.x) {
        Some(f) => Moved {
            pos: Vec3::new(pos.x, f.y - coll.bottom, pos.z),
            floor: Some(Standing {
                line,
                flags: f.flags,
                normal: f.normal,
            }),
        },
        // Walked off the end. The original looks for a wall under the corner
        // to slide down; with no wall query it simply falls.
        None => Moved { pos, floor: None },
    }
}

/// Places a body on the floor beneath it — the spawn case.
///
/// Ports `mpProcessSetCollProjectFloorID`, which is how the original finds
/// what an airborne object is above. A spawn point sits a little over its
/// surface, so this is what turns one into a standing position.
pub fn settle<I>(coll: &BodyColl, pos: Vec3, floors: I) -> Option<(Vec3, Standing, f32)>
where
    I: IntoIterator<Item = (u16, Segment)>,
{
    let probe = Vec2::new(pos.x, pos.y + coll.bottom);
    let FloorBelow {
        y,
        dist,
        line,
        flags,
        normal,
    } = collision::project_floor(floors, probe)?;
    Some((
        Vec3::new(pos.x, y - coll.bottom, pos.z),
        Standing {
            line,
            flags,
            normal,
        },
        dist,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A flat platform from x -100 to 100 at y 0, as line 0.
    fn platform() -> [(u16, Segment); 1] {
        [(
            0,
            Segment {
                x1: -100,
                y1: 0,
                x2: 100,
                y2: 0,
                flags: collision::flags::CLIFF,
            },
        )]
    }

    /// A ramp climbing from (0,0) to (200,100), as line 1.
    fn ramp() -> [(u16, Segment); 1] {
        [(
            1,
            Segment {
                x1: 0,
                y1: 0,
                x2: 200,
                y2: 100,
                flags: 0,
            },
        )]
    }

    #[test]
    fn a_short_movement_is_not_subdivided() {
        assert_eq!(substep_count(Vec3::ZERO, Vec3::new(10.0, -20.0, 0.0)), 1);
        assert_eq!(
            substep_count(Vec3::ZERO, Vec3::new(SUBSTEP_SPAN, 0.0, 0.0)),
            1,
            "exactly at the threshold is still one step"
        );
    }

    #[test]
    fn maximum_knockback_is_split_ten_ways() {
        // 2500 units is the game's maximum velocity; the original's comment
        // says this is where the count of 10 comes from.
        assert_eq!(substep_count(Vec3::ZERO, Vec3::new(0.0, -2500.0, 0.0)), 11);
        assert_eq!(substep_count(Vec3::ZERO, Vec3::new(0.0, -2499.0, 0.0)), 10);
    }

    #[test]
    fn a_falling_body_lands_on_the_platform() {
        let m = move_air(
            &BodyColl::default(),
            Vec3::new(0.0, 10.0, 0.0),
            Vec3::new(0.0, -5.0, 0.0),
            None,
            platform,
        );
        assert_eq!(m.pos.y, 0.0);
        let f = m.floor.expect("landed");
        assert_eq!(f.line, 0);
        assert!(f.grabbable(), "the main platform is ledge-grabbable");
    }

    #[test]
    fn a_body_launched_across_the_stage_still_finds_the_floor() {
        // Without substepping this movement jumps from well above the platform
        // to well below it in one test, and the swept query would have to
        // catch it along a 2000-unit diagonal. This is the tunnelling case.
        let m = move_air(
            &BodyColl::default(),
            Vec3::new(-90.0, 2000.0, 0.0),
            Vec3::new(90.0, -2000.0, 0.0),
            None,
            platform,
        );
        assert!(m.floor.is_some(), "landed rather than passing through");
        assert_eq!(m.pos.y, 0.0);
    }

    #[test]
    fn dropping_through_a_platform_ignores_only_that_line() {
        let soft = || {
            [(
                7,
                Segment {
                    x1: -100,
                    y1: 0,
                    x2: 100,
                    y2: 0,
                    flags: collision::flags::PASS,
                },
            )]
        };
        let through = move_air(
            &BodyColl::default(),
            Vec3::new(0.0, 10.0, 0.0),
            Vec3::new(0.0, -5.0, 0.0),
            Some(7),
            soft,
        );
        assert!(through.floor.is_none(), "fell through the ignored platform");

        let onto = move_air(
            &BodyColl::default(),
            Vec3::new(0.0, 10.0, 0.0),
            Vec3::new(0.0, -5.0, 0.0),
            Some(99),
            soft,
        );
        assert!(onto.floor.is_some(), "a different line still catches it");
    }

    #[test]
    fn a_solid_platform_cannot_be_dropped_through() {
        // `ignore_line_id` only applies to surfaces flagged passable, so
        // naming a solid line must not let a fighter fall through it.
        let m = move_air(
            &BodyColl::default(),
            Vec3::new(0.0, 10.0, 0.0),
            Vec3::new(0.0, -5.0, 0.0),
            Some(0),
            platform,
        );
        assert!(m.floor.is_some(), "solid floors ignore ignore_line");
    }

    #[test]
    fn walking_up_a_slope_follows_it() {
        let m = move_ground(&BodyColl::default(), Vec3::new(100.0, 0.0, 0.0), 1, ramp);
        assert_eq!(m.pos.y, 50.0, "halfway along a 200x100 ramp");
        assert!(m.floor.is_some());
    }

    #[test]
    fn walking_off_the_end_leaves_the_ground() {
        let m = move_ground(&BodyColl::default(), Vec3::new(300.0, 0.0, 0.0), 1, ramp);
        assert!(m.floor.is_none(), "past the end of the line");
    }

    #[test]
    fn a_standing_body_stays_where_it_is() {
        // The stability property the whole slice rests on: re-running the
        // grounded update must not drift the body.
        let mut pos = Vec3::new(37.5, 0.0, 0.0);
        for _ in 0..60 {
            let m = move_ground(&BodyColl::default(), pos, 1, ramp);
            assert!(m.floor.is_some());
            pos = m.pos;
        }
        assert_eq!(pos, Vec3::new(37.5, 18.75, 0.0));
    }

    #[test]
    fn landing_past_the_edge_puts_the_body_on_the_corner() {
        // Arriving beyond the line's end must not leave the body hanging in
        // space at the height it happened to have.
        let m = land(
            &BodyColl::default(),
            Vec3::new(500.0, -50.0, 0.0),
            1,
            0,
            Vec2::new(0.0, 1.0),
            ramp,
        );
        assert_eq!(m.pos.x, 200.0);
        assert_eq!(m.pos.y, 100.0);
    }

    #[test]
    fn the_collision_offset_lifts_the_body_off_the_surface() {
        // A body whose probe point is 12 units below its origin must stand 12
        // units above the surface, not on it.
        let coll = BodyColl {
            bottom: -12.0,
            ..BodyColl::default()
        };
        let m = move_air(
            &coll,
            Vec3::new(0.0, 30.0, 0.0),
            Vec3::new(0.0, 5.0, 0.0),
            None,
            platform,
        );
        assert_eq!(m.pos.y, 12.0);
    }

    #[test]
    fn a_spawn_settles_onto_what_it_is_above() {
        let (pos, floor, dist) =
            settle(&BodyColl::default(), Vec3::new(50.0, 4.0, 0.0), platform()).expect("a floor");
        assert_eq!(pos.y, 0.0);
        assert_eq!(floor.line, 0);
        assert_eq!(dist, -4.0, "the spawn sat 4 units above its surface");
    }

    #[test]
    fn settling_finds_nothing_below_open_air() {
        assert!(settle(
            &BodyColl::default(),
            Vec3::new(9000.0, 0.0, 0.0),
            platform()
        )
        .is_none());
    }
}
