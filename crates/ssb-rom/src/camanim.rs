//! Camera animations (`gcAddCObjCamAnimJoint`, `gcPlayCamAnim`) baked one
//! play per frame.
//!
//! A camera's script is the joint script's 32-bit stream
//! ([`crate::objanim`]) pointed at the ten camera tracks
//! (`nGCAnimTrackEyeX` to `nGCAnimTrackFovY`): eye, the eye's path
//! (`EyeI`), at, at's path (`AtI`), `up.x` and the vertical field of view.
//! `gcPlayCObjCamAnim` writes each live track into the `CObj`; a track the
//! script never keys keeps whatever the camera held, so a bake starts from
//! the camera's own values ([`CamInit`]).
//!
//! Each baked play is `[eye.x, eye.y, eye.z, at.x, at.y, at.z, up.x,
//! fovy]`; the last is the play the script ends on, after which the camera
//! keeps its values (`anim_wait` goes `AOBJ_ANIM_NULL`).

use crate::figatree::JointPose;
use crate::objanim::{AnimError, StageJoint};

/// Floats per baked play.
pub const FRAME_FLOATS: usize = 8;

/// One baked play.
pub type Frame = [f32; FRAME_FLOATS];

/// The camera's values before its script's first play.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CamInit {
    pub eye: [f32; 3],
    pub at: [f32; 3],
    pub up_x: f32,
    pub fovy: f32,
}

impl CamInit {
    /// `dGCCObjVecDefault` and `dGCPerspDefault`'s field of view: what a
    /// scene camera holds once `gcAddXObjForCamera` has given it a view and
    /// a perspective.
    pub const DEFAULT: CamInit = CamInit {
        eye: [0.0, 0.0, 1500.0],
        at: [0.0; 3],
        up_x: 0.0,
        fovy: 30.0,
    };

    fn frame(&self) -> Frame {
        [
            self.eye[0],
            self.eye[1],
            self.eye[2],
            self.at[0],
            self.at[1],
            self.at[2],
            self.up_x,
            self.fovy,
        ]
    }
}

/// `gcAddCObjCamAnimJoint(cobj, script, 0.0F)` then `gcPlayCamAnim` until
/// the script ends: one [`Frame`] per play. `plays_cap` bounds a looping
/// script.
pub fn bake(
    data: &[u8],
    script: u32,
    init: CamInit,
    plays_cap: usize,
) -> Result<alloc::vec::Vec<Frame>, AnimError> {
    let mut j = StageJoint::start_camera(script, 0.0);
    let mut pose = JointPose::default();
    let mut cam = init.frame();
    let mut out = alloc::vec::Vec::new();
    while !j.ended() && out.len() < plays_cap {
        j.tick(data, 1.0, &mut pose)?;
        apply(&j, data, &mut cam);
        out.push(cam);
    }
    Ok(out)
}

/// `gcPlayCObjCamAnim`'s writes, track by track.
fn apply(j: &StageJoint, data: &[u8], cam: &mut Frame) {
    let path = |at: Option<u32>, t: f32| {
        at.and_then(|at| crate::interp::Spline::read(data, at))
            .and_then(|s| s.cubic(data, t.clamp(0.0, 1.0)))
    };
    for track in 0..10 {
        let Some(v) = j.track_value(track) else {
            continue;
        };
        match track {
            0..=2 => cam[track] = v,
            3 => {
                if let Some(p) = path(j.interp(), v) {
                    cam[0..3].copy_from_slice(&p);
                }
            }
            4..=6 => cam[track - 1] = v,
            7 => {
                if let Some(p) = path(j.interp_at(), v) {
                    cam[3..6].copy_from_slice(&p);
                }
            }
            8 => cam[6] = v,
            _ => cam[7] = v,
        }
    }
}

/// Little-endian bytes of `frames`, as a pack carries them.
pub fn to_bytes(frames: &[Frame]) -> alloc::vec::Vec<u8> {
    frames
        .iter()
        .flat_map(|f| f.iter().flat_map(|v| v.to_le_bytes()))
        .collect()
}

/// The `i`th baked play from a pack's bytes, or the last once `i` is past
/// the end; `None` for no plays.
pub fn frame_at(bytes: &[u8], i: usize) -> Option<Frame> {
    let (frames, _) = bytes.as_chunks::<{ FRAME_FLOATS * 4 }>();
    let c = frames.get(i).or(frames.last())?;
    Some(core::array::from_fn(|k| {
        f32::from_le_bytes([c[k * 4], c[k * 4 + 1], c[k * 4 + 2], c[k * 4 + 3]])
    }))
}

/// The number of baked plays in a pack's bytes.
pub fn frame_count(bytes: &[u8]) -> usize {
    bytes.len() / (FRAME_FLOATS * 4)
}
