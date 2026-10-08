//! `mn/mncommon/mncongra.c`: the US build's congratulations picture after
//! the staff roll.
//!
//! The 1P Game enters it with `scene_prev` set to `nSCKind1PGame`, so the
//! picture is the 1P fighter's (`gSCManagerSceneData.fkind`). The picture
//! is two 300 x 110 sprites. After eight ticks any of A, B or START starts
//! a 90-tick black fade (`lbFadeMakeActor`); when the fade ends the frame
//! is blacked out and the scene ends five frames later.
use crate::fighter::FighterKind;
use ssb_engine::input::N64Buttons;

/// `sMNCongraSkipWait`'s start.
pub const SKIP_WAIT: u32 = 8;
/// `lbFadeMakeActor(..., 90, ...)`.
pub const FADE_LENGTH: u32 = 90;
/// `mnCongraFuncDraw`'s scene change wait after the blackout.
pub const SCENE_CHANGE_WAIT: u32 = 5;
/// The bottom and top pictures' positions (`mnCongraFuncStart`).
pub const BOTTOM_POSITION: [f32; 2] = [10.0, 120.0];
pub const TOP_POSITION: [f32; 2] = [10.0, 10.0];

/// `mnCongraFuncStart`'s announcement: "Incredible!" from a million points.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Voice {
    Incredible,
    Congratulations,
}

/// `lbFadeProcUpdate`'s clock (`lbfade.c`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fade {
    /// `sLBFadeAlphaCurrent` of `sLBFadeAlphaMax`.
    pub current: u32,
    pub max: u32,
    /// `sLBFadeLength`: `fade_length + 2`, counted down.
    pub length: u32,
}

impl Fade {
    pub fn new(fade_length: u32) -> Self {
        Self {
            current: 0,
            max: fade_length,
            length: fade_length + 2,
        }
    }

    /// `lbFadeProcUpdate`. Returns `true` on the tick the fade sets its
    /// proceed flag.
    pub fn update(&mut self) -> bool {
        if self.current < self.max {
            self.current += 1;
        }
        if self.length != 0 {
            self.length -= 1;
            return self.length == 0;
        }
        false
    }

    /// `lbFadeProcDisplay`'s alpha for an opaque colour (`color.a != 0`).
    pub fn alpha(&self) -> u8 {
        ((self.current as f32 / self.max as f32) * 255.0) as i32 as u8
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Congra {
    /// `sMNCongraFighterKind`.
    pub fkind: FighterKind,
    pub voice: Voice,
    skip_wait: u32,
    is_proceed: bool,
    /// The black fade, once A, B or START starts it.
    pub fade: Option<Fade>,
    /// `syVideoSetFlags(SYVIDEO_FLAG_BLACKOUT)`: the picture is no longer
    /// shown from the frame after the fade ends.
    pub blackout: bool,
    blackout_pending: bool,
    scene_change_wait: u32,
    finished: bool,
}

impl Congra {
    /// `mnCongraFuncStart`, which plays [`Voice`].
    pub fn new(fkind: FighterKind, score: u32) -> Self {
        let voice = if score >= 1_000_000 {
            Voice::Incredible
        } else {
            Voice::Congratulations
        };
        crate::sound::play_fgm(match voice {
            Voice::Incredible => crate::sound::id::nSYAudioVoiceAnnounceIncredible,
            Voice::Congratulations => crate::sound::id::nSYAudioVoiceAnnounceCongra,
        });
        Self {
            fkind,
            voice,
            skip_wait: SKIP_WAIT,
            is_proceed: false,
            fade: None,
            blackout: false,
            blackout_pending: false,
            scene_change_wait: 0,
            finished: false,
        }
    }

    /// One frame: `mnCongraActorFuncRun`, the fade's process, then
    /// `mnCongraFuncDraw`'s scene change. Returns `true` on the frame the
    /// scene ends.
    pub fn tick(&mut self, taps: N64Buttons) -> bool {
        if self.finished {
            return false;
        }
        if self.blackout_pending {
            self.blackout_pending = false;
            self.blackout = true;
        }
        if self.skip_wait != 0 {
            self.skip_wait -= 1;
        }
        if self.skip_wait == 0
            && !self.is_proceed
            && taps.contains(N64Buttons::A | N64Buttons::B | N64Buttons::START)
        {
            self.is_proceed = true;
            self.fade = Some(Fade::new(FADE_LENGTH));
        }
        // The fade's process is queued in this tick's run and runs after
        // the actor's `func_run`.
        let mut proceed_scene = false;
        if let Some(fade) = self.fade.as_mut() {
            proceed_scene = fade.update();
        }
        // `mnCongraFuncDraw`, after `gcDrawAll`.
        if proceed_scene {
            self.blackout_pending = true;
            self.scene_change_wait = SCENE_CHANGE_WAIT;
        }
        if self.scene_change_wait != 0 {
            self.scene_change_wait -= 1;
            if self.scene_change_wait == 0 {
                self.finished = true;
            }
        }
        self.finished
    }
}
