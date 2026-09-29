//! Colour animations — `GMColAnim`, `ftMainUpdateColAnim` (`ft/ftmain.c`)
//! and the `ftParam*ColAnim*` setters (`ft/ftparam.c`).
//!
//! A colour animation runs a small script (`gm/gmcolscripts.c`) that sets
//! and blends `color1`, the colour a fighter is fogged towards
//! (`ftDisplayMainCalcFogColor`) and the colour the screen flash fills with
//! (`ifScreenFlashProcDisplay`), and can turn the fighter's light. Only the
//! scripts the KO presentation runs are transcribed ([`ColAnimId`]); the
//! interpreter follows the source event for event.
//!
//! One script thread is modelled: none of these scripts starts a parallel
//! one (`nGMColEventSetParallelScript`).

/// One `gmColCommand*` event.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ColEvent {
    /// `nGMColEventSetColor1`: `color1` at once, its steps cleared.
    SetColor1([u8; 4]),
    /// `nGMColEventBlendColor1`: `color1` steps towards the colour over the
    /// frames.
    BlendColor1(u16, [u8; 4]),
    /// `nGMColEventWait`.
    Wait(u16),
    /// `nGMColEventGoto` back to the script's first event.
    GotoStart,
    /// `nGMColEventClearColorAll`.
    ClearColorAll,
    /// `nGMColEventEnd`.
    End,
    /// `nGMColEventSetLight`: the fighter's light angles, in degrees.
    SetLight(i16, i16),
}

/// `dGMColScriptsFighterRebirth`: the respawn's white pulse, lit from
/// below.
pub const FIGHTER_REBIRTH: &[ColEvent] = &[
    ColEvent::SetLight(0, -70),
    ColEvent::SetColor1([0xFF, 0xFF, 0xFF, 0xFF]),
    ColEvent::Wait(2),
    ColEvent::BlendColor1(36, [0xFF, 0xFF, 0xFF, 0x0A]),
    ColEvent::Wait(36),
    ColEvent::BlendColor1(18, [0xFF, 0xFF, 0xFF, 0xB4]),
    ColEvent::Wait(18),
    ColEvent::GotoStart,
];

/// `dGMColScriptsScreenFlashDeadExplode`: the white flash of a blast KO.
pub const SCREEN_FLASH_DEAD_EXPLODE: &[ColEvent] = &[
    ColEvent::SetColor1([0xFF, 0xFF, 0xFF, 0x00]),
    ColEvent::BlendColor1(6, [0xFF, 0xFF, 0xFF, 0x6E]),
    ColEvent::Wait(6),
    ColEvent::BlendColor1(30, [0xFF, 0xFF, 0xFF, 0x00]),
    ColEvent::Wait(30),
    ColEvent::ClearColorAll,
    ColEvent::End,
];

/// `nGMColAnim*`, for the scripts this port runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ColAnimId {
    /// Id 0: no script.
    #[default]
    None,
    FighterRebirth,
    ScreenFlashDeadExplode,
}

impl ColAnimId {
    /// `dGMColScriptsDescs[id].p_script`.
    pub fn script(self) -> Option<&'static [ColEvent]> {
        match self {
            ColAnimId::None => None,
            ColAnimId::FighterRebirth => Some(FIGHTER_REBIRTH),
            ColAnimId::ScreenFlashDeadExplode => Some(SCREEN_FLASH_DEAD_EXPLODE),
        }
    }

    /// `dGMColScriptsDescs[id].priority`.
    pub fn priority(self) -> u8 {
        match self {
            ColAnimId::None => 0,
            ColAnimId::FighterRebirth => 10,
            ColAnimId::ScreenFlashDeadExplode => 60,
        }
    }

    /// `dGMColScriptsDescs[id].is_unlocked`: a status change ends it.
    pub fn is_unlocked(self) -> bool {
        !matches!(self, ColAnimId::None)
    }
}

/// `GMColKeys`: a colour and its per-frame step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ColKeys {
    pub rgba: [u8; 4],
    pub step: [i16; 4],
}

/// `GMColAnim`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ColAnim {
    pub id: ColAnimId,
    /// `cs[0].p_script` as an index into the id's script.
    pc: Option<usize>,
    /// `cs[0].color_event_timer`.
    timer: u16,
    /// `length`: frames until the animation ends by itself; 0 for none.
    pub length: i32,
    pub color1: ColKeys,
    pub is_use_color1: bool,
    /// `light_angle_x`, `light_angle_y`, when `is_use_light`.
    pub light: Option<(f32, f32)>,
}

impl ColAnim {
    /// `ftParamCheckSetColAnimID`: starts `id` unless a higher-priority
    /// animation is running.
    pub fn check_set(&mut self, id: ColAnimId, length: i32) -> bool {
        if id.priority() < self.id.priority() {
            return false;
        }
        self.id = id;
        self.length = length;
        self.pc = id.script().map(|_| 0);
        self.timer = 0;
        self.is_use_color1 = false;
        self.light = None;
        true
    }

    /// `ftParamResetColAnim`. The colour and its steps are kept.
    pub fn reset(&mut self) {
        self.pc = None;
        self.length = 0;
        self.id = ColAnimId::None;
        self.is_use_color1 = false;
        self.light = None;
    }

    /// `ftMainUpdateColAnim`: one frame. Returns `true` when the animation
    /// ends (its script's `End`, or its `length` running out).
    pub fn update(&mut self) -> bool {
        let script = self.id.script().unwrap_or(&[]);
        if self.pc.is_some() && self.timer != 0 {
            self.timer -= 1;
        }
        while let Some(pc) = self.pc.filter(|_| self.timer == 0) {
            let Some(&event) = script.get(pc) else {
                self.pc = None;
                break;
            };
            self.pc = Some(pc + 1);
            match event {
                // With no parallel script, the end is the animation's.
                ColEvent::End => return true,
                ColEvent::Wait(frames) => self.timer = frames,
                ColEvent::GotoStart => self.pc = Some(0),
                ColEvent::ClearColorAll => self.is_use_color1 = false,
                ColEvent::SetColor1(rgba) => {
                    self.is_use_color1 = true;
                    self.color1 = ColKeys { rgba, step: [0; 4] };
                }
                ColEvent::BlendColor1(frames, target) => {
                    let frames = i32::from(frames.max(1));
                    for (step, (&to, &from)) in self
                        .color1
                        .step
                        .iter_mut()
                        .zip(target.iter().zip(&self.color1.rgba))
                    {
                        *step = ((i32::from(to) - i32::from(from)) / frames) as i16;
                    }
                }
                ColEvent::SetLight(x, y) => self.light = Some((f32::from(x), f32::from(y))),
            }
        }
        if self.is_use_color1 {
            for (c, &step) in self.color1.rgba.iter_mut().zip(&self.color1.step) {
                *c = c.wrapping_add(step as u8);
            }
        }
        if self.length != 0 {
            self.length -= 1;
            if self.length == 0 {
                return true;
            }
        }
        false
    }

    /// `ftMainRunUpdateColAnim`: an ended animation gives way to
    /// `ftParamResetStatUpdateColAnim`. The port has no hit-status colour
    /// animations yet, so that is a plain reset.
    pub fn run_update(&mut self) {
        while self.update() {
            self.reset();
        }
    }

    /// `color1` when it is in use.
    pub fn color(&self) -> Option<[u8; 4]> {
        self.is_use_color1.then_some(self.color1.rgba)
    }
}

/// `ftMainSetStatus`: an unlocked animation ends with the status unless the
/// new one preserves it (`FTSTATUS_PRESERVE_COLANIM`).
pub(crate) fn on_set_status(c: &mut ColAnim, preserve: bool) {
    if !preserve && c.id.is_unlocked() {
        c.reset();
    }
}

#[cfg(test)]
#[path = "colanim_tests.rs"]
mod tests;
