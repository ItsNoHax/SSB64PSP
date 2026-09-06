//! The real battle camera, ported from `gm/gmcamera.c`'s
//! `gmCameraDefaultFuncCamera` and the functions it calls directly (RE-131).
//!
//! Simplified for this project's current scope, each simplification
//! deliberate and documented rather than silent:
//!
//! * **No weapons.** `gmCameraUpdateInterests` also unions camera-following
//!   weapons. Combat and weapons remain gated, so only its one-to-four fighter
//!   path is represented here.
//! * **Callers currently supply no per-move camera zoom.** [`Interest`]
//!   represents the original multiplier, but the current gameplay slice has
//!   not yet ported the status writes that change it. The 120-tick Wait
//!   zoom-out is represented.
//! * **No entry/explain/dead-up modes.** Always the plain "watch the
//!   fighter" case (`FTCamera`'s `default` arm), since this project has no
//!   match-start/KO camera states yet.
//! * **No pause-camera offset.** `gGMCameraPauseCameraEyeX`/`Y` are always
//!   `0.0` outside the pause menu, which does not exist in this project yet.

use ssb_engine::math::Vec3;

/// The rectangle the camera's look-at target may not leave --
/// `MPGroundData.camera_bounds` (`crates/ssb-rom/src/stage.rs`'s
/// `GroundData::camera_bounds`, packed as `pack::StageDesc::camera`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bounds {
    pub top: f32,
    pub bottom: f32,
    pub left: f32,
    pub right: f32,
}

impl Bounds {
    /// `gmCameraSetBoundsPosition` (`gm/gmcamera.c:101`): checks and fixes
    /// one axis at a time, in this exact priority order (left, right,
    /// bottom, top), looping until nothing is left to fix. A valid
    /// (non-degenerate) rectangle converges in at most two iterations; this
    /// mirrors the real function's own unbounded `while (TRUE)` rather than
    /// asserting a bound on it.
    pub fn clamp(&self, mut p: Vec3) -> Vec3 {
        loop {
            if p.x < self.left {
                p.x = self.left;
            } else if p.x > self.right {
                p.x = self.right;
            } else if p.y < self.bottom {
                p.y = self.bottom;
            } else if p.y > self.top {
                p.y = self.top;
            } else {
                break;
            }
        }
        p
    }
}

/// `gmCameraGetClampDimensionsMax`'s own distance clamp.
const MIN_DIST: f32 = 2500.0;
const MAX_DIST: f32 = 30000.0;

/// `dGMCameraPlayerZoomRanges[1]` -- the single-player zoom multiplier.
const ONE_PLAYER_ZOOM: f32 = 1.50;

/// The final battle-camera state produced by `gmCameraMakeDefaultCamera`.
///
/// The constructor briefly copies `dGMCameraCObjVecDefault`, then explicitly
/// replaces its eye/at vectors and sets `target_dist` to 10000 before the
/// first camera tick (`gm/gmcamera.c:1157-1171`). These are therefore the
/// observable initial values; the intermediate `{1500, 0, 0}` eye is not.
pub const DEFAULT_EYE: Vec3 = Vec3::new(0.0, 300.0, 10000.0);
pub const DEFAULT_AT: Vec3 = Vec3::new(0.0, 300.0, 0.0);
pub const DEFAULT_FOVY_DEGREES: f32 = 38.0;
const DEFAULT_TARGET_DIST: f32 = 10000.0;

/// One fighter entry consumed by `gmCameraUpdateInterests`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Interest {
    /// Fighter position with `FTAttributes.cam_offset_y` already applied.
    pub target_pos: Vec3,
    /// `FTStruct.lr == -1`; the wider side of the interest box faces forward.
    pub facing_left: bool,
    /// `FTStruct.camera_zoom_frame`, initialized from `FTAttributes.camera_zoom`.
    pub zoom_frame: f32,
    /// Per-status `FTStruct.camera_zoom_range` multiplier.
    pub zoom_range: f32,
    /// The original applies a further 0.75 multiplier after 120 Wait ticks.
    pub idle_zoomed_out: bool,
}

/// The real camera's own smoothly-updated state -- one `CObj` plus
/// `GMCamera`'s `target_dist`/`fovy` fields, reduced to what
/// [`Camera::tick`] actually needs.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Camera {
    pub eye: Vec3,
    pub at: Vec3,
    pub fovy_degrees: f32,
    target_dist: f32,
}

impl Default for Camera {
    fn default() -> Self {
        Camera {
            eye: DEFAULT_EYE,
            at: DEFAULT_AT,
            fovy_degrees: DEFAULT_FOVY_DEGREES,
            target_dist: DEFAULT_TARGET_DIST,
        }
    }
}

impl Camera {
    /// One frame of `gmCameraDefaultFuncCamera` (`gm/gmcamera.c:624`),
    /// ported call for call in the same order:
    /// `gmCameraUpdateInterests`, `gmCameraAdjustFOV`,
    /// `gmCameraGetClampDimensionsMax`, `func_ovl2_8010C670`,
    /// `gmCameraPan`, `func_ovl2_8010C3C0`, `func_ovl2_8010C5C0`.
    /// (`gmCameraApplyVel`/`gmCameraApplyFOV` are trivial field
    /// assignments the caller does not need a separate step for here.)
    ///
    /// `fighter_pos`: the tracked fighter's position (`DObj` translate,
    /// `FTStruct::attr.cam_offset_y` already added by the caller, matching
    /// `gmCameraUpdateInterests`' own `target_pos.y += fp->attr->cam_offset_y`).
    /// `fighter_facing_left`: `fp->lr == -1`.
    /// `bounds`: the stage's own camera bounds.
    /// `light_angle_z_radians`: `MPGroundData.light_angle.z`, already
    /// radians (RE-131 -- unlike `.x`/`.y`, which are degrees).
    /// `viewport_aspect`: width / height, `gGMCameraStruct.viewport_width /
    /// .viewport_height`.
    pub fn tick(
        &mut self,
        fighter_pos: Vec3,
        fighter_facing_left: bool,
        bounds: Bounds,
        light_angle_z_radians: f32,
        viewport_aspect: f32,
    ) {
        self.tick_interests(
            &[Interest {
                target_pos: fighter_pos,
                facing_left: fighter_facing_left,
                zoom_frame: 1.0,
                zoom_range: 1.0,
                idle_zoomed_out: false,
            }],
            bounds,
            light_angle_z_radians,
            viewport_aspect,
        );
    }

    /// The one-to-four fighter path through `gmCameraUpdateInterests`.
    pub fn tick_interests(
        &mut self,
        interests: &[Interest],
        bounds: Bounds,
        light_angle_z_radians: f32,
        viewport_aspect: f32,
    ) {
        debug_assert!(
            interests.len() <= 4,
            "the original battle supports at most four fighters"
        );
        let count = interests.len().min(4);
        let (interest, hz, vt) = calculate_interest(&interests[..count], bounds);

        self.advance(interest, hz, vt, light_angle_z_radians, viewport_aspect);
    }

    fn advance(
        &mut self,
        interest: Vec3,
        hz: f32,
        vt: f32,
        light_angle_z_radians: f32,
        viewport_aspect: f32,
    ) {
        // gmCameraAdjustFOV(38.0): a 10%-per-frame lerp toward the real
        // default, not an instant snap -- matters once a mode other than
        // this one (not ported) has pulled `fovy` away from 38.
        self.fovy_degrees += (DEFAULT_FOVY_DEGREES - self.fovy_degrees) * 0.1;

        // gmCameraGetClampDimensionsMax: the distance at which the
        // interest box exactly fits the viewport, clamped to the real
        // hard range.
        let half_fovy_tan = original_tan(self.fovy_degrees.to_radians() * 0.5);
        let vt_dist = vt / half_fovy_tan;
        let hz_dist = hz / (half_fovy_tan * viewport_aspect);
        let dist = vt_dist.max(hz_dist).clamp(MIN_DIST, MAX_DIST);

        // func_ovl2_8010C670: snap outward immediately, or damp an inward
        // move 7.5% of the way toward `dist` each frame.
        self.target_dist = approach_target_distance(self.target_dist, dist);

        // gmCameraPan: `syVectorDiff`/`Mag`/`Norm`/`Scale`/`Add` compose
        // into exactly `at.lerp(interest, scale)` when `interest != at`
        // (and are a no-op, matching `lerp`, when they are equal).
        self.at = self.at.lerp(interest, pan_scale(self.target_dist));

        // func_ovl2_8010C3C0 + gmCameraGetAdjustAtAngle: a unit
        // eye-direction vector derived from `at` and the stage's own
        // light-angle nudge.
        let direction = eye_direction(self.at, light_angle_z_radians);

        // func_ovl2_8010C5C0: move the eye 10% of the way toward the ideal
        // position for the current `target_dist`/direction -- the same
        // `lerp` shape as `gmCameraPan` above.
        let ideal_eye = self.at + direction * self.target_dist;
        self.eye = self.eye.lerp(ideal_eye, 0.1);

        // gmCameraApplyVel: no external velocity source is ported yet
        // (nothing currently writes to it), so there is nothing to add.
        // gmCameraApplyFOV: `self.fovy_degrees` above already *is* the
        // value a caller reads, unlike the real `CObj`/`GMCamera` split.
    }
}

/// `gmCameraUpdateInterests`: clamp and union every fighter's asymmetric box.
fn calculate_interest(interests: &[Interest], bounds: Bounds) -> (Vec3, f32, f32) {
    let zoom = [0.0, ONE_PLAYER_ZOOM, 1.32, 1.16, 1.0][interests.len()];
    let (mut gm_left, mut gm_right, mut gm_bottom, mut gm_top) =
        (65536.0f32, -65536.0f32, 65536.0f32, -65536.0f32);
    for interest in interests {
        let target_pos = bounds.clamp(interest.target_pos);
        let idle = if interest.idle_zoomed_out { 0.75 } else { 1.0 };
        let adjust = zoom * interest.zoom_frame * interest.zoom_range * idle;
        let (left_off, right_off) = if interest.facing_left {
            (1000.0 * adjust, 700.0 * adjust)
        } else {
            (700.0 * adjust, 1000.0 * adjust)
        };
        gm_left = gm_left.min(target_pos.x - left_off);
        gm_right = gm_right.max(target_pos.x + right_off);
        gm_bottom = gm_bottom.min(target_pos.y - 700.0 * adjust);
        gm_top = gm_top.max(target_pos.y + 700.0 * adjust);
    }
    if interests.is_empty() {
        gm_left = -2000.0;
        gm_right = 2000.0;
        gm_bottom = -2000.0;
        gm_top = 2000.0;
    }
    let hz = (gm_right - gm_left) * 0.5;
    let vt = (gm_top - gm_bottom) * 0.5;
    let interest = Vec3::new(
        (gm_left + gm_right) * 0.5,
        (0.5 - target_at_y(if vt < hz { hz } else { vt })) * (gm_bottom + gm_top),
        0.0,
    );
    (interest, hz, vt)
}

/// `gmCameraGetTargetAtY` (`gm/gmcamera.c:217`): how far to bias the look-at
/// point's `y` toward the interest box's own vertical centre, tapering in
/// as `dist` shrinks from 2000 to 1000 game units.
fn target_at_y(dist: f32) -> f32 {
    if dist > 2000.0 {
        0.0682
    } else if dist < 1000.0 {
        0.0
    } else {
        (dist - 1000.0) / 1000.0 * 0.0682
    }
}

/// `func_ovl2_8010C4D0` (`gm/gmcamera.c:539`): how much of the distance
/// between `at` and the interest centre to close each frame.
///
/// Deliberately keeps the real formula's own two discontinuities at
/// `target_dist == 2000`/`15000` rather than smoothing them away -- the
/// decompilation's own comment on this function ("Needs to be two
/// different 0.05s lol") already flags this as an original-game oddity,
/// not a decompiler artifact, so reproducing it exactly is more faithful
/// than "fixing" it.
fn pan_scale(target_dist: f32) -> f32 {
    if target_dist > 15000.0 {
        0.1
    } else if target_dist < 2000.0 {
        0.05
    } else {
        (1.0 - (target_dist - 2000.0) / 13000.0) * 0.05 + 0.05
    }
}

/// `func_ovl2_8010C670` (`gm/gmcamera.c:587`) exactly as written. When the
/// requested distance is farther away, the original snaps outward in one
/// tick (`delta < 0`, therefore `delta <= delta * 0.075`). It only damps a
/// move inward. Using absolute values here changes that asymmetric behavior.
fn approach_target_distance(current: f32, requested: f32) -> f32 {
    let delta = current - requested;
    let step = delta * 0.075;
    if delta <= step {
        requested
    } else {
        current - step
    }
}

/// `func_ovl2_8010C3C0` + `gmCameraGetAdjustAtAngle` combined
/// (`gm/gmcamera.c:507`/`320`): a unit eye-direction vector derived from
/// the look-at point `at`, nudged by the stage's own `light_angle.z`.
fn eye_direction(at: Vec3, light_angle_z_radians: f32) -> Vec3 {
    let y =
        (-((at.y - 900.0) / 133.0).to_radians()).clamp((-7.0f32).to_radians(), 5.0f32.to_radians());
    let x = (-(at.x / 133.0).to_radians()).clamp((-17.5f32).to_radians(), 17.5f32.to_radians());

    // `gGMCameraPauseCameraEyeY`/`X` are always `0.0` outside the pause
    // menu (not ported), so only `y`/`x` and the stage's own nudge remain.
    let angle_x = y + light_angle_z_radians;
    let sin_x = original_sin(angle_x);
    let cos_x = original_cos(angle_x);
    let vy = -sin_x;
    let mut vz = cos_x;

    let angle_y = x;
    let sin_y = original_sin(angle_y);
    let cos_y = original_cos(angle_y);
    let vx = sin_y * vz;
    vz *= cos_y;

    Vec3::new(vx, vy, vz)
}

/// The original does not call the platform's trigonometry routines here. Its
/// `lbCommonSin`/`Cos`/`Tan` quantize a radian angle into 4096 turns and read a
/// 1024-entry, six-decimal sine table. 1011 of its 1024 words are exactly the
/// rounded mathematical sine; retain the 13 authored exceptions explicitly
/// so this reproduces the complete table without carrying 4 KiB of constants.
fn original_sine_sample(index: u16) -> f32 {
    let exception = match index {
        372 => Some(0x3f0a_48b6),
        420 => Some(0x3f19_c1f8),
        453 => Some(0x3f23_eae6),
        500 => Some(0x3f31_a826),
        503 => Some(0x3f32_80bf),
        598 => Some(0x3f4b_41f2),
        628 => Some(0x3f52_33be),
        685 => Some(0x3f5e_28bb),
        722 => Some(0x3f65_0471),
        795 => Some(0x3f70_5dd9),
        804 => Some(0x3f71_8f60),
        842 => Some(0x3f76_1672),
        1023 => Some(0x3f80_0000),
        _ => None,
    };
    if let Some(bits) = exception {
        return f32::from_bits(bits);
    }
    let angle = index as f32 / 651.898_6;
    let value = ssb_engine::math::sin_cos(angle).0;
    ((value * 1_000_000.0 + 0.5) as u32) as f32 / 1_000_000.0
}

fn original_sin_index(index: u16) -> f32 {
    let index = index & 0x0fff;
    let low = index & 0x03ff;
    let sample = if index & 0x0400 != 0 {
        original_sine_sample(0x03ff - low)
    } else {
        original_sine_sample(low)
    };
    if index & 0x0800 != 0 {
        -sample
    } else {
        sample
    }
}

fn original_angle_index(angle: f32) -> u16 {
    ((angle * 651.898_6) as i32 as u16) & 0x0fff
}

fn original_sin(angle: f32) -> f32 {
    original_sin_index(original_angle_index(angle))
}

fn original_cos(angle: f32) -> f32 {
    original_sin_index(original_angle_index(angle).wrapping_add(0x0400))
}

fn original_tan(angle: f32) -> f32 {
    let index = original_angle_index(angle);
    original_sin_index(index) / original_sin_index(index.wrapping_add(0x0400))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_matches_the_post_constructor_battle_camera_state() {
        let camera = Camera::default();
        assert_eq!(camera.at, Vec3::new(0.0, 300.0, 0.0));
        assert_eq!(camera.eye, Vec3::new(0.0, 300.0, 10000.0));
        assert_eq!(camera.target_dist, 10000.0);
        assert_eq!(camera.fovy_degrees, 38.0);
    }

    #[test]
    fn original_camera_trig_uses_the_roms_quantized_table_words() {
        // tan(19 degrees) reads entries 216 and 807. Entry 372 is one of the
        // 13 authored values that differs from rounded mathematical sine.
        assert_eq!(original_sine_sample(216).to_bits(), 0x3ea6_8f08);
        assert_eq!(original_sine_sample(807).to_bits(), 0x3f71_f288);
        assert_eq!(original_sine_sample(372).to_bits(), 0x3f0a_48b6);
        assert!((original_tan(19.0f32.to_radians()) - 0.3442044).abs() < 1e-7);
    }

    #[test]
    fn target_distance_snaps_outward_and_damps_inward_like_the_original() {
        assert_eq!(approach_target_distance(2500.0, 4000.0), 4000.0);
        assert!((approach_target_distance(10000.0, 4000.0) - 9550.0).abs() < f32::EPSILON);
        assert_eq!(approach_target_distance(4000.0, 4000.0), 4000.0);
    }

    #[test]
    fn bounds_clamp_leaves_an_interior_point_alone() {
        let b = Bounds {
            top: 4000.0,
            bottom: -2000.0,
            left: -3900.0,
            right: 3900.0,
        };
        let p = Vec3::new(100.0, 200.0, 0.0);
        assert_eq!(b.clamp(p), p);
    }

    #[test]
    fn bounds_clamp_pulls_a_point_back_on_every_axis() {
        let b = Bounds {
            top: 4000.0,
            bottom: -2000.0,
            left: -3900.0,
            right: 3900.0,
        };
        assert_eq!(
            b.clamp(Vec3::new(9000.0, 9000.0, 0.0)),
            Vec3::new(3900.0, 4000.0, 0.0)
        );
        assert_eq!(
            b.clamp(Vec3::new(-9000.0, -9000.0, 0.0)),
            Vec3::new(-3900.0, -2000.0, 0.0)
        );
    }

    #[test]
    fn pan_scale_matches_the_real_formula_at_its_own_named_points() {
        // The two boundary constants the real function returns outright.
        assert_eq!(pan_scale(15001.0), 0.1);
        assert_eq!(pan_scale(1999.0), 0.05);
        // The midpoint of the interpolated range: (1 - 0.5) * 0.05 + 0.05.
        assert!((pan_scale(8500.0) - 0.075).abs() < 1e-6);
        // The two real discontinuities this function deliberately keeps
        // (see its own doc comment): approaching 2000 and 15000 from
        // inside the interpolated range does *not* approach the boundary
        // constants smoothly.
        assert!((pan_scale(2001.0) - 0.099996).abs() < 1e-4);
        assert!((pan_scale(14999.0) - 0.050004).abs() < 1e-4);
    }

    #[test]
    fn a_stationary_fighter_lets_the_camera_settle_and_stay_settled() {
        // No hand-derived target position here (that both duplicates the
        // formula under test and is easy to get subtly wrong -- an earlier
        // version of this test did exactly that, forgetting the single-
        // player zoom multiplier). Instead: run long enough to converge,
        // then confirm a further stretch of identical input barely moves
        // it at all.
        let mut cam = Camera::default();
        let bounds = Bounds {
            top: 4000.0,
            bottom: -2000.0,
            left: -3900.0,
            right: 3900.0,
        };
        for _ in 0..300 {
            cam.tick(Vec3::ZERO, false, bounds, 0.0, 15.0 / 11.0);
        }
        let settled = cam;
        for _ in 0..30 {
            cam.tick(Vec3::ZERO, false, bounds, 0.0, 15.0 / 11.0);
        }
        assert!(
            (cam.at.x - settled.at.x).abs() < 1.0,
            "at.x drifted from {} to {}",
            settled.at.x,
            cam.at.x
        );
        assert!((cam.eye - settled.eye).length() < 1.0);
        assert!(cam.eye.x.is_finite() && cam.eye.y.is_finite() && cam.eye.z.is_finite());
    }

    #[test]
    fn the_camera_follows_a_fighter_that_walks_away() {
        let mut cam = Camera::default();
        let bounds = Bounds {
            top: 4000.0,
            bottom: -2000.0,
            left: -3900.0,
            right: 3900.0,
        };
        for _ in 0..120 {
            cam.tick(Vec3::new(2000.0, 0.0, 0.0), false, bounds, 0.0, 15.0 / 11.0);
        }
        // 120 frames (2 seconds) is long enough for the 10%/7.5% lerps to
        // have converged close to the interest box's own centre -- the
        // fighter's position plus the facing-dependent asymmetric offset
        // (RE-131: right-facing is `-700`/`+1000` around the fighter, times
        // the 1-player zoom of `1.5`), not just taken one small step
        // toward it.
        let expected_x = 2000.0 + (1000.0 - 700.0) * 0.5 * 1.5;
        assert!(
            (cam.at.x - expected_x).abs() < 5.0,
            "at.x = {}, expected close to {expected_x}",
            cam.at.x
        );
        assert!(cam.eye.x.is_finite() && cam.eye.y.is_finite() && cam.eye.z.is_finite());
    }

    #[test]
    fn one_fighter_wrapper_matches_the_general_interest_path() {
        let bounds = Bounds {
            top: 4000.0,
            bottom: -2000.0,
            left: -3900.0,
            right: 3900.0,
        };
        let target = Vec3::new(1200.0, 250.0, 0.0);
        let mut wrapped = Camera::default();
        let mut general = Camera::default();
        wrapped.tick(target, true, bounds, 0.03, 15.0 / 11.0);
        general.tick_interests(
            &[Interest {
                target_pos: target,
                facing_left: true,
                zoom_frame: 1.0,
                zoom_range: 1.0,
                idle_zoomed_out: false,
            }],
            bounds,
            0.03,
            15.0 / 11.0,
        );
        assert_eq!(wrapped, general);
    }

    #[test]
    fn two_fighters_union_their_original_asymmetric_interest_boxes() {
        let bounds = Bounds {
            top: 4000.0,
            bottom: -2000.0,
            left: -3900.0,
            right: 3900.0,
        };
        let interests = [
            Interest {
                target_pos: Vec3::new(-1000.0, 150.0, 0.0),
                facing_left: false,
                zoom_frame: 1.0,
                zoom_range: 1.0,
                idle_zoomed_out: false,
            },
            Interest {
                target_pos: Vec3::new(1000.0, 250.0, 0.0),
                facing_left: true,
                zoom_frame: 1.0,
                zoom_range: 1.0,
                idle_zoomed_out: false,
            },
        ];
        let (interest, hz, vt) = calculate_interest(&interests, bounds);

        // With dGMCameraPlayerZoomRanges[2] == 1.32, the outer edges are
        // -1000 - 700*1.32 and 1000 + 700*1.32. The vertical union is
        // 150 - 700*1.32 through 250 + 700*1.32.
        assert!((hz - 1924.0).abs() < f32::EPSILON);
        assert!((vt - 974.0).abs() < f32::EPSILON);
        assert!(interest.x.abs() < f32::EPSILON);
        let expected_y = (0.5 - target_at_y(hz)) * 400.0;
        assert!((interest.y - expected_y).abs() < 1e-4);
    }

    #[test]
    fn settled_dream_land_camera_matches_the_original_rom_trace() {
        let bounds = Bounds {
            top: 4000.0,
            bottom: -2000.0,
            left: -3900.0,
            right: 3900.0,
        };
        // Player 1 Mario on the centre floor and player 2 Pikachu on the
        // left platform, including their packed camera offsets and the Wait
        // zoom applied after 120 ticks. These are the exact stable inputs in
        // the scripted original-ROM trace recorded by RE-151.
        let interests = [
            Interest {
                target_pos: Vec3::new(0.0, 250.0, 0.0),
                facing_left: true,
                zoom_frame: 1.0,
                zoom_range: 1.0,
                idle_zoomed_out: true,
            },
            Interest {
                target_pos: Vec3::new(-1397.0, 1054.0, 0.0),
                facing_left: false,
                zoom_frame: 1.0,
                zoom_range: 1.0,
                idle_zoomed_out: true,
            },
        ];
        let mut camera = Camera::default();
        for _ in 0..1000 {
            camera.tick_interests(&interests, bounds, (-10.0f32).to_radians(), 15.0 / 11.0);
        }

        let original_at = Vec3::new(-698.5003, 617.1825, 0.0);
        let original_eye = Vec3::new(-413.7114, 1045.3188, 3137.6443);
        assert!(
            (camera.target_dist - 3181.2507).abs() < 0.1,
            "target_dist = {}",
            camera.target_dist
        );
        assert!(
            (camera.at - original_at).length() < 0.1,
            "at = {:?}",
            camera.at
        );
        // The original's `syVectorMag3D` + `syVectorNorm3D` implementation
        // leaves a sub-unit steady-state residual where the algebraically
        // equivalent lerp converges fully. It is 0.67 game units here, under
        // one tenth of a pixel in the captured viewport.
        assert!(
            (camera.eye - original_eye).length() < 1.0,
            "eye = {:?}",
            camera.eye
        );
        assert!((camera.fovy_degrees - 38.0).abs() < 0.001);
    }
}
