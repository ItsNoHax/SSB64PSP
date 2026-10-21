//! The battle camera's process (`gGMCameraGObj`'s `func_camera` by
//! `gGMCameraStruct.status`, `gm/gmcamera.c`): which camera function runs
//! this tick, over [`Camera`]'s motions.
//!
//! [`CameraRig`] is the camera plus the statuses set over it
//! (`gmCameraSetStatusPlayerZoom`, `gmCameraSetStatusMapZoom`,
//! `gmCameraSetStatusAnim`); [`interest_of`] is one fighter's entry in
//! `gmCameraUpdateInterests`; [`CameraStage`] is what the camera reads of
//! the stage, built by the host from its stage data.

use ssb_engine::math::Vec3;

use super::{idle_zoomed_out, Bounds, Camera, Interest, StageCamera};
use crate::boss::CameraRequest;
use crate::dead::CameraMode;
use crate::fighter::{Facing, Fighter};

/// `gmCameraSetStatusPlayerZoom(fighter, 0, 0, closeup_camera_zoom, 0.1,
/// 28)` as `ifCommonEntryFocusThread` sets it: the fighter and the
/// distance only. The camera reads the fighter's position live each tick
/// (`gmCameraUpdatePlayerZoom`), so none is held here (RE-543).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EntryZoom {
    /// The fighter's slot in the battle's seats (the player's, then the
    /// dummies').
    pub slot: u8,
    /// `pzoom_dist`: the fighter's `closeup_camera_zoom`.
    pub dist: f32,
}

/// [`EntryZoom`] resolved by the battle's camera pass: the zoomed
/// fighter's translate this tick, its `cam_offset_y` and the distance.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EntryFocusTarget {
    pub pos: Vec3,
    pub cam_offset_y: f32,
    pub dist: f32,
}

/// `gmCameraSetStatusPlayerZoom(fighter, eye_x, eye_y, dist, pan, fov)`
/// as the auto demo sets it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DemoZoom {
    /// The fighter's port.
    pub port: u8,
    /// `pzoom_eye_x`, `pzoom_eye_y` in radians.
    pub eye: (f32, f32),
    /// `pzoom_dist`: the fighter's `closeup_camera_zoom`.
    pub dist: f32,
    pub pan_scale: f32,
    pub fov: f32,
    /// The fighter's translation and `cam_offset_y` at this tick.
    pub pos: Vec3,
    pub cam_offset_y: f32,
}

/// A camera status set over the battle camera.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CameraStatus {
    /// `gmCameraSetStatusMapZoom(&origin, &target)`.
    MapZoom { origin: Vec3, target: Vec3 },
    /// `gmCameraSetStatusAnim`: the host plays the baked frames into the
    /// camera; `ended` hands back to the battle camera on the next tick.
    Anim {
        frame: usize,
        vel: Vec3,
        ended: bool,
    },
}

/// What the battle camera reads of a fighter besides the [`Fighter`]:
/// `FTAttributes::cam_offset_y` and `FTStruct::camera_zoom_frame`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Framing {
    pub cam_offset_y: f32,
    pub zoom_frame: f32,
}

impl Default for Framing {
    /// No offset, `camera_zoom_frame` 1.
    fn default() -> Self {
        Framing {
            cam_offset_y: 0.0,
            zoom_frame: 1.0,
        }
    }
}

/// The 1P bonus stages' following camera by stage: Race to the Finish
/// (`BONUS3`), a Board the Platforms stage, or any other.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BonusFollow {
    Race,
    Platforms,
    #[default]
    Other,
}

/// What the battle camera reads of the stage (`gMPCollisionGroundData`,
/// `gGMCameraStruct.status_default`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CameraStage {
    /// `camera_bound_*`.
    pub camera: Bounds,
    /// `camera_bound_team_*`: a 1P Game enemy's camera bounds.
    pub team_camera: Bounds,
    /// `map_bound_top`, which `gmCameraSetDeadUpStarPosition` scales by.
    pub map_top: f32,
    /// `MPGroundData.light_angle.z` in radians.
    pub light_angle_z: f32,
    /// `gGMCameraStruct.status_default` by `gkind`.
    pub stage_camera: StageCamera,
    pub follow: BonusFollow,
}

/// The fighter's entry in `gmCameraUpdateInterests`, by its camera mode:
/// the fighter's position, the entry position while entering, or
/// `gmCameraSetDeadUpStarPosition`'s top-out point; `None` for a ghost.
pub fn interest_of(f: &Fighter, framing: Framing, stage: &CameraStage) -> Option<Interest> {
    let (mut target, facing) = match f.dead.camera_mode {
        CameraMode::Ghost => return None,
        CameraMode::Entry | CameraMode::Explain => (f.entry.pos, f.entry.lr.unwrap_or(f.facing)),
        CameraMode::DeadUp => (f.dead.up_pos, f.facing),
        CameraMode::Default => (f.pos, f.facing),
    };
    target.y += framing.cam_offset_y;
    // `gmCameraUpdateInterests`: a 1P Game enemy (the only fighter with
    // team blast bounds) is clamped to the team camera bounds instead,
    // whatever its camera mode.
    let team_bounds = f.dead.team_bounds.map(|_| stage.team_camera);
    let dead_up = f.dead.camera_mode == CameraMode::DeadUp && team_bounds.is_none();
    if dead_up {
        let top = stage.camera.top;
        target.x = top * target.x / stage.map_top;
        target.y = top;
    }
    Some(Interest {
        target_pos: target,
        facing_left: matches!(facing, Facing::Left),
        zoom_frame: framing.zoom_frame,
        zoom_range: f.dead.camera_zoom_range.0,
        idle_zoomed_out: idle_zoomed_out(f),
        dead_up,
        team_bounds,
    })
}

/// The battle camera (`gGMCameraGObj`) with the statuses set over it.
/// Derefs to its [`Camera`].
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct CameraRig {
    pub camera: Camera,
    /// The entry focus's zoom (`ifCommonEntryFocusThread`'s
    /// `gmCameraSetStatusPlayerZoom`, RE-402): which fighter and how close.
    /// The battle's camera pass resolves the fighter's live position for
    /// [`CameraRig::tick`] (RE-543). `None` runs the battle camera.
    pub entry_zoom: Option<EntryZoom>,
    /// The 1P bonus stages' following camera ([`CameraStage::follow`]).
    pub bonus_follow: bool,
    /// A camera status other than the battle camera's
    /// (`gmCameraSetStatusMapZoom`, `gmCameraSetStatusAnim`): Master Hand's
    /// background attacks and the boss stage's camera animations.
    pub status: Option<CameraStatus>,
    /// Training's Close-Up view (`sc1PTrainingModeUpdateViewOption`'s
    /// `gmCameraSetStatusPlayerZoom`): the fighter's `closeup_camera_zoom`.
    /// `None` runs the battle camera.
    pub player_zoom: Option<f32>,
    /// The auto demo's focus (`scAutoDemoSetCameraPlayerZoom`), on any
    /// player's fighter: the camera pass refreshes its target before each
    /// tick. `None` runs the battle camera.
    pub demo_zoom: Option<DemoZoom>,
}

impl core::ops::Deref for CameraRig {
    type Target = Camera;
    fn deref(&self) -> &Camera {
        &self.camera
    }
}

impl core::ops::DerefMut for CameraRig {
    fn deref_mut(&mut self) -> &mut Camera {
        &mut self.camera
    }
}

impl CameraRig {
    /// Master Hand's camera request: `gmCameraSetStatusMapZoom` or
    /// `gmCameraSetStatusDefault`.
    pub fn apply_boss(&mut self, request: CameraRequest) {
        match request {
            CameraRequest::MapZoom { at, eye } => self.set_status_map_zoom(at, eye),
            CameraRequest::Default => self.status = None,
        }
    }

    /// `gmCameraSetStatusMapZoom(&origin, &target)`: [`Camera::begin_map_zoom`]
    /// under the status that ticks it.
    pub fn set_status_map_zoom(&mut self, origin: Vec3, target: Vec3) {
        self.camera.begin_map_zoom(origin, target);
        self.status = Some(CameraStatus::MapZoom { origin, target });
    }

    /// Advances the battle camera one tick. `pos` and `cam_offset_y` are
    /// the camera's own fighter's (the lead's); `interests` are
    /// [`interest_of`] each fighter, the lead's first, then the others in
    /// link order; any past the fourth are ignored. `entry` is
    /// [`CameraRig::entry_zoom`] resolved on the zoomed fighter as its
    /// process left it this tick.
    pub fn tick(
        &mut self,
        pos: Vec3,
        cam_offset_y: f32,
        interests: &[Interest],
        entry: Option<EntryFocusTarget>,
        stage: &CameraStage,
    ) {
        // `gcEndProcessAll` on the movie camera: its scene moves it.
        if self.camera.movie.is_some() {
            return;
        }
        match self.status {
            // `gmCameraMapZoomFuncCamera`.
            Some(CameraStatus::MapZoom { origin, target }) => {
                self.camera.tick_map_zoom(origin, target);
                return;
            }
            // `gmCameraAnimFuncCamera`: the host plays the animation; once
            // its script has ended the battle camera takes over
            // (`gmCameraSetStatusDefault`).
            Some(CameraStatus::Anim { ended, .. }) => {
                if ended {
                    self.status = None;
                } else {
                    return;
                }
            }
            None => {}
        }
        let bounds = stage.camera;
        if self.bonus_follow {
            let target = bounds.clamp(Vec3::new(pos.x, pos.y + cam_offset_y, 0.0));
            let race = stage.follow == BonusFollow::Race;
            let pitch = if stage.follow == BonusFollow::Other {
                -9.0
            } else {
                -15.0
            };
            self.camera.tick_player_zoom(
                target,
                (0.0, pitch * core::f32::consts::PI / 180.0),
                if race { 7000.0 } else { 9000.0 },
                0.3,
                31.5,
            );
            return;
        }
        // `gmCameraPlayerZoomFuncCamera`: the battle camera while the
        // zoomed fighter is out of the camera bounds or more than 1000
        // units off the stage plane (Kirby's star, Fox's Arwing).
        if let Some(e) = entry {
            if self.camera.tick_player_zoom_in_bounds(
                e.pos,
                e.cam_offset_y,
                bounds,
                (0.0, 0.0),
                e.dist,
                crate::pause::ZOOM_PAN_SCALE,
                crate::appear::FOCUS_ZOOM_FOV,
            ) {
                return;
            }
        }
        // `gmCameraPlayerZoomFuncCamera`: the battle camera while the
        // fighter is out of the camera bounds.
        if let Some(z) = self.demo_zoom {
            if self.camera.tick_player_zoom_in_bounds(
                z.pos,
                z.cam_offset_y,
                bounds,
                z.eye,
                z.dist,
                z.pan_scale,
                z.fov,
            ) {
                return;
            }
        }
        if let Some(dist) = self.player_zoom {
            if self.camera.tick_player_zoom_in_bounds(
                pos,
                cam_offset_y,
                bounds,
                (0.0, 0.0),
                dist,
                crate::training::CLOSE_UP_PAN_SCALE,
                crate::training::CLOSE_UP_FOV,
            ) {
                return;
            }
        }
        // `gGMCameraStruct.status_default` by `gkind`
        // (`gmCameraMakeDefaultCamera`): the stage's own battle camera.
        self.camera.stage_camera = stage.stage_camera;
        // `gGMCameraStruct.viewport_width / .viewport_height`: 300 / 220 for
        // the battle, 300 / 150 for How to Play's. The PSP shows the
        // battle viewport at that aspect (D-047).
        let (w, h) = self.camera.viewport_size();
        let aspect = w / h;
        let interests = &interests[..interests.len().min(4)];
        self.camera
            .tick_interests(interests, bounds, stage.light_angle_z, aspect);
    }
}

#[cfg(test)]
#[path = "rig_tests.rs"]
mod tests;
