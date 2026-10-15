//! Portable source-shadow geometry from `ft/ftshadow.c`.
//!
//! This is deliberately not a generic decal: SSB64 stretches a 16x16
//! intensity texture over the clipped supporting-floor polyline.  Keeping
//! this data independent of the PSP renderer makes the source behaviour
//! host-testable and leaves GU state in `psp-runtime`.

use ssb_engine::math::{Vec2, Vec3};

use crate::fighter::Fighter;
use crate::map;
use crate::status::{AnyStatus, Status};
use crate::weapon::{MapSurface, MapSurfaceKind};

/// Source vertex texture coordinate range, in texels before the RSP's
/// texture scale: `tc` runs 0 to 1984 in s10.5, `1984 / 32 == 62`.
pub const SOURCE_TEX_SPAN: f32 = 62.0;
/// `gsSPTexture(0x8000, 0x8000, ...)` in `dFTShadowNoPrevLinkDL`: the RSP
/// multiplies every vertex texture coordinate by `0x8000 / 0x10000`, so the
/// strip samples 31 texels of the 16x16 mirrored tile per axis, one blob
/// centred on the strip. Without it the strip spans two mirror periods and
/// shows a 2x2 grid of four blobs.
pub const TEX_SCALE: f32 = 0.5;
/// Source half-depth in match Z units.
pub const HALF_DEPTH: f32 = 200.0;
/// `dFTCommonDataShadowColorDefault`.
pub const COLOR_DEFAULT: [u8; 4] = [0x00, 0x00, 0x00, 0xA0];
/// `dFTCommonDataShadowColorTeams`: red, blue and green.
pub const COLOR_TEAMS: [[u8; 4]; 3] = [
    [0x80, 0x00, 0x00, 0xB4],
    [0x00, 0x00, 0x80, 0xB4],
    [0x00, 0x80, 0x00, 0xB4],
];
/// `pos_project.y = translate.y - 65536`: the wall guard's probe length.
const WALL_PROBE_DEPTH: f32 = 65536.0;

/// The render tile's texel coordinate for a source vertex coordinate given
/// in texels (`tc / 32`), after the RSP's texture scale.
pub fn tile_texel(source_texel: f32) -> f32 {
    source_texel * TEX_SCALE
}

/// The `gDPSetPrimColor` of `ftShadowProcDisplay`: the team colour in a
/// team battle unless `is_not_teamshadows` (the 1P Game) is set.
pub fn color(is_team_battle: bool, is_not_teamshadows: bool, team: u8) -> [u8; 4] {
    if is_team_battle && !is_not_teamshadows {
        COLOR_TEAMS[usize::from(team)]
    } else {
        COLOR_DEFAULT
    }
}

/// Source `dFTShadowNoPrevLinkDL` material policy, without PSP API types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderState {
    pub depth_test: bool,
    pub depth_write: bool,
    pub alpha_test_greater: u8,
    pub alpha_blend: bool,
    pub cull: bool,
}

/// `G_RM_AA_XLU_SURF`, alpha threshold 15, and no culling. The display list
/// clears `G_ZBUFFER` and the render mode has no `Z_CMP`, so the source shadow
/// is drawn without a depth test (RE-514).
pub const RENDER_STATE: RenderState = RenderState {
    depth_test: false,
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

/// `ftShadowProcDisplay` for one fighter: the visibility gate, the floor
/// line, the wall guard and the strip.
///
/// The floor is `coll_data.floor_line_id` on the ground, where the rebirth
/// halo's fake floor (`-2`, a grounded fighter with no line here) draws
/// nothing, and `mpCollisionCheckProjectFloor` from the fighter's position
/// in the air. The strip is hidden when a left or right wall line crosses
/// the drop from the fighter above that floor
/// (`mpCollisionCheck{L,R}WallLineCollisionSame` down to `y - 65536`
/// against `ga_dist`, which is 0 on the ground). `surfaces` are the live
/// map surfaces, so a moving group's floor carries its shadow.
pub fn fighter_shadow<I, F>(surfaces: &F, fighter: &Fighter, shadow_size: f32) -> ShadowGeometry
where
    F: Fn() -> I,
    I: IntoIterator<Item = MapSurface>,
{
    if !visible_for(
        fighter.status.status,
        fighter.is_invisible,
        fighter.is_shadow_hidden,
    ) {
        return ShadowGeometry::EMPTY;
    }
    let pos = fighter.pos;
    let (line, ga_dist) = if fighter.is_grounded() {
        match fighter.floor {
            Some(standing) => (standing.line, 0.0),
            None => return ShadowGeometry::EMPTY,
        }
    } else {
        match map::project_floor_line(surfaces, pos) {
            Some(below) => below,
            None => return ShadowGeometry::EMPTY,
        }
    };
    let from = Vec2::new(pos.x, pos.y);
    let to = Vec2::new(pos.x, pos.y - WALL_PROBE_DEPTH);
    for kind in [MapSurfaceKind::LeftWall, MapSurfaceKind::RightWall] {
        if map::wall_crossing(surfaces, kind, from, to).is_some_and(|hit| pos.y - hit.y < -ga_dist)
        {
            return ShadowGeometry::EMPTY;
        }
    }
    build(
        pos,
        shadow_size,
        map::lines_of(surfaces(), MapSurfaceKind::Floor)
            .filter(move |(id, _)| *id == line)
            .map(|(_, surface)| surface.coords()),
    )
}

/// Builds `ftShadowProcDisplay`'s floor-conforming strip.
///
/// `floor` is the selected source collision line, not every stage floor, as
/// `[x1, y1, x2, y2]` segments where the line is now. The source clips
/// `x ± shadow_size` to its endpoints and follows at most two bends; its
/// `FTShadow::vtx[8]` has the same four-point capacity retained here, so
/// unexpected map shapes truncate without allocating.
pub fn build(
    position: Vec3,
    shadow_size: f32,
    floor: impl IntoIterator<Item = [f32; 4]>,
) -> ShadowGeometry {
    if shadow_size <= 0.0 {
        return ShadowGeometry::EMPTY;
    }
    let left = position.x - shadow_size;
    let right = position.x + shadow_size;
    let mut out = ShadowGeometry::EMPTY;

    for [x1, y1, x2, y2] in floor {
        let lo = x1.min(x2);
        let hi = x1.max(x2);
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
            let denom = x2 - x1;
            let y = if denom == 0.0 {
                y1
            } else {
                ((x - x1) * (y2 - y1) / denom) + y1
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
    use crate::collision::Segment;

    /// [`super::build`] over integer map segments.
    fn build(
        position: Vec3,
        shadow_size: f32,
        floor: impl IntoIterator<Item = Segment>,
    ) -> ShadowGeometry {
        super::build(
            position,
            shadow_size,
            floor
                .into_iter()
                .map(|s| [s.x1 as f32, s.y1 as f32, s.x2 as f32, s.y2 as f32]),
        )
    }

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
    fn source_material_is_translucent_without_depth_test_or_write() {
        let state = core::hint::black_box(RENDER_STATE);
        assert!(!state.depth_test);
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

    /// The RDP's `G_TX_MIRROR | G_TX_WRAP`, mask 4 tile texel of the 16x16
    /// shadow image at an integer texel centre (BILERP samples centres at
    /// integers): 0..15 then 15..0, repeating.
    fn mirrored_texel(t: i32) -> i32 {
        if t & 16 != 0 {
            15 - (t & 15)
        } else {
            t & 15
        }
    }

    /// The image texels the strip samples across one axis, at each tile
    /// texel centre the scaled coordinate range covers.
    fn sampled_row(source_from: f32, source_to: f32) -> Vec<i32> {
        let (from, to) = (tile_texel(source_from), tile_texel(source_to));
        (from.ceil() as i32..=to.floor() as i32)
            .map(mirrored_texel)
            .collect()
    }

    /// Runs of the blob's brightest texels (15, the mirror seam) in a row.
    fn peaks(row: &[i32]) -> usize {
        row.iter()
            .zip(core::iter::once(&-1).chain(row.iter()))
            .filter(|(t, prev)| **t == 15 && **prev != 15)
            .count()
    }

    #[test]
    fn texture_scale_samples_one_mirrored_blob_per_axis() {
        // Both strip axes run 0..=SOURCE_TEX_SPAN source texels (u across
        // the depth, v along the floor from `x - size` to `x + size`).
        let shadow = build(Vec3::new(10.0, 50.0, 0.0), 200.0, flat());
        let points = &shadow.points[..shadow.point_count];
        let (v_from, v_to) = (points[0].v, points[points.len() - 1].v);
        assert_eq!((v_from, v_to), (0.0, SOURCE_TEX_SPAN));
        for (from, to) in [(0.0, SOURCE_TEX_SPAN), (v_from, v_to)] {
            assert_eq!(tile_texel(to) - tile_texel(from), 31.0);
            let row = sampled_row(from, to);
            // One blob: a single peak run, centred on the strip.
            assert_eq!(peaks(&row), 1, "{row:?}");
            let centre = row.iter().position(|t| *t == 15).unwrap() as f32 + 0.5;
            assert_eq!(centre, tile_texel(to - from) / 2.0);
        }
    }

    #[test]
    fn team_shadows_use_the_team_colour_except_in_the_1p_game() {
        assert_eq!(color(false, false, 1), COLOR_DEFAULT);
        assert_eq!(color(true, false, 0), [0x80, 0x00, 0x00, 0xB4]);
        assert_eq!(color(true, false, 1), [0x00, 0x00, 0x80, 0xB4]);
        assert_eq!(color(true, false, 2), [0x00, 0x80, 0x00, 0xB4]);
        assert_eq!(color(true, true, 2), COLOR_DEFAULT);
        assert!(crate::battle::Battle::new_1p(5, Default::default(), false, 0).is_not_teamshadows);
    }

    fn fighter_at(pos: Vec3, floor: Option<u16>) -> Fighter {
        let mut fighter = Fighter::new(crate::fighter::FighterKind::Mario, 0, 3);
        crate::status::set_wait(&mut fighter);
        fighter.pos = pos;
        match floor {
            Some(line) => {
                fighter.situation = crate::fighter::Situation::Ground;
                fighter.floor = Some(crate::ground::Standing {
                    line,
                    flags: 0,
                    normal: Vec2::new(0.0, 1.0),
                });
            }
            None => fighter.situation = crate::fighter::Situation::Air,
        }
        fighter
    }

    fn surface(kind: MapSurfaceKind, line: u16, segment: [i16; 4]) -> MapSurface {
        let [x1, y1, x2, y2] = segment;
        MapSurface {
            kind,
            ..map::floor_surface((
                line,
                Segment {
                    x1,
                    y1,
                    x2,
                    y2,
                    flags: 0,
                },
            ))
        }
    }

    const FLOOR: u16 = 3;

    fn stage() -> Vec<MapSurface> {
        vec![
            surface(MapSurfaceKind::Floor, 1, [-500, 900, 500, 900]),
            surface(MapSurfaceKind::Floor, FLOOR, [-500, 20, 500, 20]),
        ]
    }

    #[test]
    fn grounded_shadows_use_the_standing_line_and_the_rebirth_floor_draws_none() {
        let map = stage();
        let shadow = fighter_shadow(
            &|| map.iter().copied(),
            &fighter_at(Vec3::new(0.0, 20.0, 0.0), Some(FLOOR)),
            200.0,
        );
        assert_eq!(shadow.point_count, 2);
        assert!(shadow.points[..2].iter().all(|p| p.y == 20.0));
        // `floor_line_id == -2`, the rebirth halo: grounded on no map line.
        let mut halo = fighter_at(Vec3::new(0.0, 20.0, 0.0), Some(FLOOR));
        halo.floor = None;
        assert!(!fighter_shadow(&|| map.iter().copied(), &halo, 200.0).visible());
    }

    #[test]
    fn airborne_shadows_project_to_the_floor_below_on_its_moving_group() {
        let mut map = stage();
        let airborne = fighter_at(Vec3::new(0.0, 300.0, 0.0), None);
        let shadow = fighter_shadow(&|| map.iter().copied(), &airborne, 200.0);
        assert!(shadow.points[..shadow.point_count]
            .iter()
            .all(|p| p.y == 20.0));
        map[1].motion = Some(crate::weapon::SurfaceMotion {
            offset: Vec2::new(0.0, 40.0),
            ..Default::default()
        });
        let shadow = fighter_shadow(&|| map.iter().copied(), &airborne, 200.0);
        assert!(shadow.points[..shadow.point_count]
            .iter()
            .all(|p| p.y == 60.0));
        let below = fighter_at(Vec3::new(0.0, -100.0, 0.0), None);
        assert!(!fighter_shadow(&|| map.iter().copied(), &below, 200.0).visible());
    }

    /// A tilted wall through the drop at `wall_y` (between the fighter at
    /// y 300 and the floor at y 20, or under the floor).
    fn crossing_wall(kind: MapSurfaceKind, wall_y: i16) -> [MapSurface; 2] {
        // Either orientation; only one crosses the probe for each side.
        [
            surface(kind, 7, [-30, wall_y - 30, 30, wall_y + 30]),
            surface(kind, 8, [-30, wall_y + 30, 30, wall_y - 30]),
        ]
    }

    #[test]
    fn a_wall_between_the_fighter_and_its_floor_hides_the_shadow() {
        for kind in [MapSurfaceKind::LeftWall, MapSurfaceKind::RightWall] {
            let airborne = fighter_at(Vec3::new(0.0, 300.0, 0.0), None);
            let mut map = stage();
            map.extend(crossing_wall(kind, 150));
            assert!(
                map::wall_crossing(
                    &|| map.iter().copied(),
                    kind,
                    Vec2::new(0.0, 300.0),
                    Vec2::new(0.0, 300.0 - WALL_PROBE_DEPTH)
                )
                .is_some(),
                "{kind:?}"
            );
            assert!(
                !fighter_shadow(&|| map.iter().copied(), &airborne, 200.0).visible(),
                "{kind:?}"
            );
            // Under the floor the wall does not occlude.
            let mut map = stage();
            map.extend(crossing_wall(kind, -150));
            assert!(
                fighter_shadow(&|| map.iter().copied(), &airborne, 200.0).visible(),
                "{kind:?}"
            );
            // On the ground `ga_dist` is 0: any crossing below the feet is
            // under the floor.
            let standing = fighter_at(Vec3::new(0.0, 20.0, 0.0), Some(FLOOR));
            let mut map = stage();
            map.extend(crossing_wall(kind, -150));
            assert!(
                fighter_shadow(&|| map.iter().copied(), &standing, 200.0).visible(),
                "{kind:?}"
            );
        }
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
