//! Results-screen wipe lifecycle (`lb/lbtransition.c`, RE-146/147).

use ssb_rom::pack::{AnimDesc, ObjectDesc, Pack};
use ssb_rom::scene::Mat4;
use ssb_rom::skeleton::{StageAnimator, MAX_NODES};

use crate::gu::Gpu;
use crate::meshdraw::{self, DrawState};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Idle,
    AwaitingCapture,
    CaptureQueued,
    Playing,
}

pub struct ResultsTransition {
    phase: Phase,
    anim: Option<AnimDesc>,
    object: Option<ObjectDesc>,
    player: StageAnimator,
    posed: [Mat4; MAX_NODES],
    posed_len: usize,
}

impl ResultsTransition {
    pub fn new() -> Self {
        Self {
            phase: Phase::Idle,
            anim: None,
            object: None,
            player: StageAnimator::new(),
            posed: [Mat4::IDENTITY; MAX_NODES],
            posed_len: 0,
        }
    }

    /// Starts a wipe while the completed battle scene is still current.
    #[cfg_attr(not(feature = "transition_audit_capture"), allow(dead_code))]
    pub fn begin(&mut self, pack: &Pack<'_>, id: u32) -> bool {
        let Some(anim) = pack.transition_anim(id) else {
            return false;
        };
        let Some(object) = pack.transition_object(&anim) else {
            return false;
        };
        self.anim = Some(anim);
        self.object = Some(object);
        self.player.start(pack, &anim);
        self.posed_len = 0;
        self.phase = Phase::AwaitingCapture;
        true
    }

    /// Queues the one-time snapshot. The caller must still render the battle
    /// scene this frame; `Gpu::end_frame` performs the copy after GE sync.
    pub fn queue_capture(&mut self, gpu: &mut Gpu) {
        if self.phase == Phase::AwaitingCapture {
            gpu.request_transition_capture();
            self.phase = Phase::CaptureQueued;
        }
    }

    /// Called immediately after the frame containing the requested capture
    /// has completed and swapped. The next frame may enter results and draw.
    pub fn capture_completed(&mut self) {
        if self.phase == Phase::CaptureQueued {
            self.phase = Phase::Playing;
        }
    }

    pub fn tick(&mut self, pack: &Pack<'_>) {
        if self.phase != Phase::Playing {
            return;
        }
        let Some(anim) = self.anim else {
            self.phase = Phase::Idle;
            return;
        };
        let Some(script) = pack.anim_script(&anim) else {
            self.phase = Phase::Idle;
            return;
        };
        if self.player.tick(script).is_err() || self.player.ended() {
            self.phase = Phase::Idle;
        }
    }

    /// Draws through the original transition camera: 45° FOV, 15:11 aspect,
    /// eye distance `1100 / tan(22.5°)` and an origin look-at.
    pub unsafe fn draw(&mut self, pack: &Pack<'_>, gpu: &mut Gpu, state: &mut DrawState) -> u32 {
        if self.phase != Phase::Playing {
            return 0;
        }
        const EYE_Z: f32 = 2655.6348;
        let (Some(_anim), Some(object)) = (self.anim, self.object) else {
            self.phase = Phase::Idle;
            return 0;
        };
        gpu.set_perspective(45.0, 15.0 / 11.0, 100.0, 10_000.0);
        gpu.reset_modelview();
        gpu.model_transform([0.0, 0.0, -EYE_Z], [0.0; 3], meshdraw::MODEL_SCALE);
        let base = gpu.model_matrix();
        self.posed_len = self.player.compose(pack, &object, &mut self.posed);
        meshdraw::draw_object_posed(
            pack,
            &object,
            &base,
            &self.posed[..self.posed_len],
            None,
            state,
            None,
            0,
        )
    }
}
