//! The real single-player battle camera, ported from `gm/gmcamera.c`'s
//! `gmCameraDefaultFuncCamera` and the functions it calls directly (RE-131).
//!
//! Simplified for this project's current scope, each simplification
//! deliberate and documented rather than silent:
//!
//! * **One fighter, no weapons.** `gmCameraUpdateInterests` unions every
//!   fighter's and camera-following weapon's own interest box; with one
//!   fighter and no weapons yet, that union is just the fighter's own box.
//! * **No per-move camera zoom.** `FTStruct::camera_zoom_frame`/
//!   `camera_zoom_range` (a move's own temporary camera pull-in/out) are not
//!   ported, so [`Camera::tick`] always uses the single-player zoom range
//!   alone (`adjust == 1.0` in the real formula's own terms).
//! * **No idle zoom-out.** The real camera zooms out 25% after a fighter has
//!   stood in `Wait` for 120 ticks; this project's own idle/wait timing is
//!   not threaded through here yet.
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

/// `dGMCameraCObjVecDefault` -- the camera's rest state before any fighter
/// has moved it. Real hardware also stores a default `target_dist`
/// elsewhere in scene setup, not shown in this struct's own initialiser; a
/// reasonable stand-in is `eye`'s own distance from `at`; using this later
/// gives the same first-frame direction the real default vector implies.
pub const DEFAULT_EYE: Vec3 = Vec3::new(1500.0, 0.0, 0.0);
pub const DEFAULT_AT: Vec3 = Vec3::new(0.0, 0.0, 0.0);
pub const DEFAULT_FOVY_DEGREES: f32 = 38.0;

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
            target_dist: (DEFAULT_EYE - DEFAULT_AT).length(),
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
        // gmCameraUpdateInterests, one fighter, no weapons: the "union of
        // every player's box" degenerates to this one fighter's own box.
        let target_pos = bounds.clamp(fighter_pos);
        let adjust = ONE_PLAYER_ZOOM;
        let (left_off, right_off) = if fighter_facing_left {
            (1000.0 * adjust, 700.0 * adjust)
        } else {
            (700.0 * adjust, 1000.0 * adjust)
        };
        let gm_left = target_pos.x - left_off;
        let gm_right = target_pos.x + right_off;
        let gm_bottom = target_pos.y - 700.0 * adjust;
        let gm_top = target_pos.y + 700.0 * adjust;
        let hz = (gm_right - gm_left) * 0.5;
        let vt = (gm_top - gm_bottom) * 0.5;
        let interest = Vec3::new(
            (gm_left + gm_right) * 0.5,
            (0.5 - target_at_y(if vt < hz { hz } else { vt })) * (gm_bottom + gm_top),
            0.0,
        );

        // gmCameraAdjustFOV(38.0): a 10%-per-frame lerp toward the real
        // default, not an instant snap -- matters once a mode other than
        // this one (not ported) has pulled `fovy` away from 38.
        self.fovy_degrees += (DEFAULT_FOVY_DEGREES - self.fovy_degrees) * 0.1;

        // gmCameraGetClampDimensionsMax: the distance at which the
        // interest box exactly fits the viewport, clamped to the real
        // hard range.
        let half_fovy_tan = ssb_engine::math::tan(self.fovy_degrees.to_radians() * 0.5);
        let vt_dist = vt / half_fovy_tan;
        let hz_dist = hz / (half_fovy_tan * viewport_aspect);
        let dist = vt_dist.max(hz_dist).clamp(MIN_DIST, MAX_DIST);

        // func_ovl2_8010C670: damp `target_dist` 7.5% of the way toward
        // `dist` each frame, snapping once within one step of it.
        let delta = self.target_dist - dist;
        let step = delta * 0.075;
        self.target_dist = if delta.abs() <= step.abs() {
            dist
        } else {
            self.target_dist - step
        };

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

/// `func_ovl2_8010C3C0` + `gmCameraGetAdjustAtAngle` combined
/// (`gm/gmcamera.c:507`/`320`): a unit eye-direction vector derived from
/// the look-at point `at`, nudged by the stage's own `light_angle.z`.
fn eye_direction(at: Vec3, light_angle_z_radians: f32) -> Vec3 {
    let y = (-((at.y - 900.0) / 133.0).to_radians())
        .clamp((-7.0f32).to_radians(), 5.0f32.to_radians());
    let x =
        (-(at.x / 133.0).to_radians()).clamp((-17.5f32).to_radians(), 17.5f32.to_radians());

    // `gGMCameraPauseCameraEyeY`/`X` are always `0.0` outside the pause
    // menu (not ported), so only `y`/`x` and the stage's own nudge remain.
    let angle_x = y + light_angle_z_radians;
    let (sin_x, cos_x) = ssb_engine::math::sin_cos(angle_x);
    let vy = -sin_x;
    let mut vz = cos_x;

    let angle_y = x;
    let (sin_y, cos_y) = ssb_engine::math::sin_cos(angle_y);
    let vx = sin_y * vz;
    vz *= cos_y;

    Vec3::new(vx, vy, vz)
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert!((cam.at.x - settled.at.x).abs() < 1.0, "at.x drifted from {} to {}", settled.at.x, cam.at.x);
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
}
