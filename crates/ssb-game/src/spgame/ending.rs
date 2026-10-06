//! `mv/mvending/mvending.c`: the 1P Game's ending movie.
//!
//! The 1P fighter drops into the opening movie's room as a figure
//! (`nFTDemoStatusFigureDropped`) while the operator camera
//! (`llMVEndingOperatorCamAnimJoint`) plays. A black rectangle fades the
//! room in from tick 70; from tick 340 a white rectangle brightens over
//! everything; at tick 540 the room's animated props, the fighter and the
//! light are removed and the screen is black; at tick 660 the staff roll
//! follows. The movie takes no input (its stick checks only reset an
//! unused counter).
//!
//! The rectangles' alphas change in their display callbacks, so
//! [`Ending::draw`] advances them once per drawn frame, as `gcDrawAll`
//! does.
use crate::fighter::FighterKind;

/// `mvEndingFuncRun`'s clock marks.
pub const LIGHT_TIC: u32 = 340;
pub const EJECT_TIC: u32 = 540;
pub const END_TIC: u32 = 660;
/// The fade-in rectangle starts fading at this tick.
pub const FADE_IN_START: u32 = 70;
/// `sMVEndingRoomFadeInAlpha`'s step per drawn frame.
pub const FADE_IN_STEP: i32 = 7;
/// `sMVEndingRoomLightAlpha`'s step per drawn frame, and its ceiling.
pub const LIGHT_STEP: f32 = 1.1;
pub const LIGHT_MAX: f32 = 220.0;

/// `mvEndingMakeFighter`'s figure position.
pub const FIGHTER_POSITION: [f32; 3] = [-1077.804, 4038.864, -3688.5298];
/// `mvEndingSetupOperatorCamera`'s clip planes.
pub const CAMERA_NEAR: f32 = 128.0;
pub const CAMERA_FAR: f32 = 16384.0;
/// `scSubsysFighterSetLightParams(45.0F, 45.0F, ...)`.
pub const LIGHT_ANGLE: [f32; 2] = [45.0, 45.0];
/// `mvEndingMakeRoomBooks` and its siblings start their joint animations
/// 300 frames in.
pub const PROP_ANIM_START: f32 = 300.0;

/// What the movie shows; the PSP draws it.
#[derive(Debug, Clone, PartialEq)]
pub struct Ending {
    /// `sMVEndingFighterDemoDesc`: the 1P player's battle slot.
    pub fkind: FighterKind,
    pub costume: u8,
    pub shade: u8,
    /// `sMVEndingTotalTimeTics`.
    pub total_tics: u32,
    /// `sMVEndingRoomFadeInAlpha`: black over the room (camera priority 60).
    pub fade_in_alpha: i32,
    /// `sMVEndingRoomLightAlpha`, while the light object lives: white over
    /// everything (camera priority 30).
    pub light_alpha: Option<f32>,
    /// The background, books, pencils, lamp and tissues, until tick 540.
    /// The desk is never ejected.
    pub props_shown: bool,
    /// The figure, until `ftManagerDestroyFighter` at tick 540.
    pub fighter_shown: bool,
    finished: bool,
}

impl Ending {
    /// `mvEndingInitVars` and `mvEndingFuncStart`'s objects.
    pub fn new(fkind: FighterKind, costume: u8, shade: u8) -> Self {
        Self {
            fkind,
            costume,
            shade,
            total_tics: 0,
            fade_in_alpha: 0xFF,
            light_alpha: None,
            props_shown: true,
            fighter_shown: true,
            finished: false,
        }
    }

    /// `mvEndingFuncRun`. Returns `true` on the tick the staff roll is
    /// requested.
    pub fn tick(&mut self) -> bool {
        if self.finished {
            return false;
        }
        self.total_tics += 1;
        if self.total_tics >= 10 {
            match self.total_tics {
                LIGHT_TIC => self.light_alpha = Some(0.0),
                EJECT_TIC => {
                    self.props_shown = false;
                    self.fighter_shown = false;
                    self.light_alpha = None;
                }
                END_TIC => self.finished = true,
                _ => {}
            }
        }
        self.finished
    }

    /// `mvEndingRoomFadeInProcDisplay` and `mvEndingRoomLightProcDisplay`'s
    /// alpha updates, once per drawn frame.
    pub fn draw(&mut self) {
        if self.total_tics >= EJECT_TIC {
            self.fade_in_alpha = 0xFF;
        }
        if (FADE_IN_START..EJECT_TIC).contains(&self.total_tics) && self.fade_in_alpha > 0 {
            self.fade_in_alpha = (self.fade_in_alpha - FADE_IN_STEP).max(0);
        }
        if let Some(alpha) = self.light_alpha.as_mut() {
            if self.total_tics >= LIGHT_TIC && *alpha < LIGHT_MAX {
                *alpha = (*alpha + LIGHT_STEP).min(LIGHT_MAX);
            }
        }
    }

    /// The light rectangle's alpha as `gDPSetPrimColor` takes it.
    pub fn light_prim_alpha(&self) -> Option<u8> {
        self.light_alpha.map(|a| a as u8)
    }
}
