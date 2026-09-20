//! Portable source-shadow geometry from `ft/ftshadow.c`.
//!
//! This is deliberately not a generic decal: SSB64 stretches a 16x16
//! intensity texture over the clipped supporting-floor polyline.  Keeping
//! this data independent of the PSP renderer makes the source behaviour
//! host-testable and leaves GU state in `psp-runtime`.

use ssb_engine::math::Vec3;

use crate::collision::Segment;
use crate::status::{AnyStatus, Status};

/// Source texture coordinate range: `1984 / 32 == 62` texels.
pub const SOURCE_TEX_SPAN: f32 = 62.0;
/// Source half-depth in match Z units.
pub const HALF_DEPTH: f32 = 200.0;

/// Source `dFTShadowNoPrevLinkDL` material policy, without PSP API types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderState {
    pub depth_test: bool,
    pub depth_write: bool,
    pub alpha_test_greater: u8,
    pub alpha_blend: bool,
    pub cull: bool,
}

/// `G_RM_AA_XLU_SURF`, alpha threshold 15, and no culling.
pub const RENDER_STATE: RenderState = RenderState {
    depth_test: true,
    depth_write: false,
    alpha_test_greater: 0x0F,
    alpha_blend: true,
    cull: false,
};

/// Common statuses whose source setup hides `FTStruct::is_shadow_hide`.
pub fn status_hides_shadow(status: AnyStatus) -> bool {
    matches!(
        status,
        AnyStatus::Common(
            Status::DeadDown
                | Status::DeadLeftRight
                | Status::DeadUpStar
                | Status::DeadUpFall
                | Status::Sleep
                | Status::Entry
                | Status::EntryNull
                | Status::RebirthDown
                | Status::RebirthStand
                | Status::RebirthWait
        )
    )
}

/// Source display gate. Invincibility is intentionally not an argument: it
/// does not hide a fighter shadow in SSB64.
pub fn visible_for(status: AnyStatus, is_invisible: bool, is_shadow_hidden: bool) -> bool {
    !is_invisible && !is_shadow_hidden && !status_hides_shadow(status)
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShadowPoint {
    pub x: f32,
    pub y: f32,
    /// V in source texels, before the renderer maps it to its texture period.
    pub v: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShadowGeometry {
    pub points: [ShadowPoint; 4],
    pub point_count: usize,
}

impl ShadowGeometry {
    pub const EMPTY: Self = Self {
        points: [ShadowPoint {
            x: 0.0,
            y: 0.0,
            v: 0.0,
        }; 4],
        point_count: 0,
    };

    pub fn visible(&self) -> bool {
        self.point_count >= 2
    }
}

/// Builds `ftShadowProcDisplay`'s floor-conforming strip.
///
/// `floor` is the selected source collision line, not every stage floor. The
/// source clips `x ± shadow_size` to its endpoints and follows at most two
/// bends; its `FTShadow::vtx[8]` has the same four-point capacity retained
/// here, so unexpected map shapes truncate without allocating.
pub fn build(
    position: Vec3,
    shadow_size: f32,
    floor: impl IntoIterator<Item = Segment>,
) -> ShadowGeometry {
    if shadow_size <= 0.0 {
        return ShadowGeometry::EMPTY;
    }
    let left = position.x - shadow_size;
    let right = position.x + shadow_size;
    let mut out = ShadowGeometry::EMPTY;

    for segment in floor {
        let lo = segment.x1.min(segment.x2) as f32;
        let hi = segment.x1.max(segment.x2) as f32;
        let begin = left.max(lo);
        let end = right.min(hi);
        if begin > end {
            continue;
        }
        // Both orientations occur in source map data. Insert in X order so
        // triangle winding and texture interpolation match the source's
        // explicit left/right endpoint branches.
        for x in [begin, end] {
            let duplicate = out.points[..out.point_count]
                .iter()
                .any(|point| (point.x - x).abs() < 0.001);
            if duplicate || out.point_count == out.points.len() {
                continue;
            }
            let denom = segment.x2 as f32 - segment.x1 as f32;
            let y = if denom == 0.0 {
                segment.y1 as f32
            } else {
                ((x - segment.x1 as f32) * (segment.y2 as f32 - segment.y1 as f32) / denom)
                    + segment.y1 as f32
            };
            out.points[out.point_count] = ShadowPoint {
                x,
                y,
                // Source: `((x - center + size) * 992) / size`.
                v: (((x - position.x + shadow_size) * (SOURCE_TEX_SPAN * 0.5)) / shadow_size)
                    .clamp(0.0, SOURCE_TEX_SPAN),
            };
            out.point_count += 1;
        }
    }

    // Source processes the collision polyline in its authored direction.
    // Sorting the fixed output reproduces its explicit reversed-line branch
    // when the portable collision iterator presents a descending segment.
    for i in 1..out.point_count {
        let point = out.points[i];
        let mut j = i;
        while j > 0 && out.points[j - 1].x > point.x {
            out.points[j] = out.points[j - 1];
            j -= 1;
        }
        out.points[j] = point;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flat() -> [Segment; 1] {
        [Segment {
            x1: -500,
            y1: 20,
            x2: 500,
            y2: 20,
            flags: 0,
        }]
    }

    #[test]
    fn shadow_size_is_the_source_half_width() {
        let shadow = build(Vec3::new(10.0, 999.0, 0.0), 200.0, flat());
        assert_eq!(shadow.point_count, 2);
        assert_eq!(shadow.points[0].x, -190.0);
        assert_eq!(shadow.points[1].x, 210.0);
        assert_eq!(shadow.points[0].v, 0.0);
        assert_eq!(shadow.points[1].v, SOURCE_TEX_SPAN);
    }

    #[test]
    fn projection_follows_a_sloped_floor_and_bends() {
        let floor = [
            Segment {
                x1: -300,
                y1: 0,
                x2: 0,
                y2: 150,
                flags: 0,
            },
            Segment {
                x1: 0,
                y1: 150,
                x2: 300,
                y2: 75,
                flags: 0,
            },
        ];
        let shadow = build(Vec3::new(0.0, 800.0, 0.0), 200.0, floor);
        assert_eq!(shadow.point_count, 3);
        assert_eq!(shadow.points[0].y, 50.0);
        assert_eq!(shadow.points[1].y, 150.0);
        assert_eq!(shadow.points[2].y, 100.0);
    }

    #[test]
    fn descending_floor_orientation_matches_the_source_left_right_branch() {
        let floor = [
            Segment {
                x1: 300,
                y1: 75,
                x2: 0,
                y2: 150,
                flags: 0,
            },
            Segment {
                x1: 0,
                y1: 150,
                x2: -300,
                y2: 0,
                flags: 0,
            },
        ];
        let shadow = build(Vec3::new(0.0, 0.0, 0.0), 200.0, floor);
        assert_eq!(shadow.point_count, 3);
        assert_eq!(shadow.points[0].y, 50.0);
        assert_eq!(shadow.points[1].y, 150.0);
        assert_eq!(shadow.points[2].y, 100.0);
    }

    #[test]
    fn floor_edges_clip_the_texture_coordinates_with_the_geometry() {
        let shadow = build(
            Vec3::new(0.0, 0.0, 0.0),
            200.0,
            [Segment {
                x1: -50,
                y1: 0,
                x2: 50,
                y2: 0,
                flags: 0,
            }],
        );
        assert_eq!(shadow.point_count, 2);
        assert_eq!(shadow.points[0].v, 23.25);
        assert_eq!(shadow.points[1].v, 38.75);
    }

    #[test]
    fn no_floor_means_no_stale_geometry() {
        assert!(!build(Vec3::ZERO, 200.0, core::iter::empty()).visible());
    }

    #[test]
    fn airborne_height_does_not_fade_or_rescale_the_source_shadow() {
        let grounded = build(Vec3::new(0.0, 0.0, 0.0), 200.0, flat());
        let airborne = build(Vec3::new(0.0, 9000.0, 0.0), 200.0, flat());
        assert_eq!(grounded, airborne);
    }

    #[test]
    fn fighters_build_independent_fixed_capacity_strips() {
        let left = build(Vec3::new(-250.0, 0.0, 0.0), 100.0, flat());
        let right = build(Vec3::new(250.0, 0.0, 0.0), 200.0, flat());
        assert_eq!((left.points[0].x, left.points[1].x), (-350.0, -150.0));
        assert_eq!((right.points[0].x, right.points[1].x), (50.0, 450.0));
    }

    #[test]
    fn source_material_requires_translucent_depth_test_without_depth_write() {
        let state = core::hint::black_box(RENDER_STATE);
        assert!(state.depth_test);
        assert!(!state.depth_write);
        assert_eq!(state.alpha_test_greater, 0x0F);
        assert!(state.alpha_blend);
        assert!(!state.cull);
    }

    #[test]
    fn lifecycle_hides_ko_and_respawn_but_not_post_rebirth_invincibility() {
        assert!(!visible_for(
            AnyStatus::Common(Status::DeadDown),
            false,
            false
        ));
        assert!(!visible_for(
            AnyStatus::Common(Status::RebirthWait),
            false,
            false
        ));
        assert!(visible_for(AnyStatus::Common(Status::Wait), false, false));
        assert!(!visible_for(AnyStatus::Common(Status::Wait), true, false));
        assert!(!visible_for(AnyStatus::Common(Status::Wait), false, true));
    }

    #[test]
    fn lifecycle_transitions_reset_the_portable_shadow_gate() {
        let mut fighter = crate::fighter::Fighter::new(crate::fighter::FighterKind::Mario, 0, 3);
        crate::status::set_status(
            &mut fighter,
            Status::DeadDown,
            0.0,
            crate::status::StatusTiming::unknown(),
        );
        assert!(fighter.is_shadow_hidden);
        crate::status::set_wait(&mut fighter);
        assert!(!fighter.is_shadow_hidden);
        assert!(visible_for(
            fighter.status.status,
            fighter.is_invisible,
            fighter.is_shadow_hidden
        ));
    }
}
