//! Results-screen wipe lifecycle (`lb/lbtransition.c`, RE-146/147, RE-464):
//! the viewer's capture-then-play wrapper around
//! `ssb_psp_runtime::transition`.

use ssb_rom::pack::Pack;

use ssb_psp_runtime::gu::Gpu;
use ssb_psp_runtime::meshdraw::DrawState;
use ssb_psp_runtime::transition::Transition;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Idle,
    AwaitingCapture,
    CaptureQueued,
    Playing,
}

pub struct ResultsTransition {
    phase: Phase,
    wipe: Option<Transition>,
}

impl ResultsTransition {
    pub fn new() -> Self {
        Self {
            phase: Phase::Idle,
            wipe: None,
        }
    }

    /// Starts a wipe while the completed battle scene is still current.
    #[cfg_attr(not(feature = "transition_audit_capture"), allow(dead_code))]
    pub fn begin(&mut self, pack: &Pack<'_>, id: u32) -> bool {
        self.wipe = Transition::make(pack, id);
        if self.wipe.is_none() {
            return false;
        }
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
        if !self.wipe.as_mut().is_some_and(|w| w.update(pack)) {
            self.phase = Phase::Idle;
            self.wipe = None;
        }
    }

    /// Draws through `lbTransitionMakeCamera`'s camera.
    pub unsafe fn draw(&mut self, pack: &Pack<'_>, gpu: &mut Gpu, state: &mut DrawState) -> u32 {
        match (&self.wipe, self.phase) {
            (Some(w), Phase::Playing) => w.draw(pack, gpu, state),
            _ => 0,
        }
    }
}
