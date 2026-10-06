//! `mn/mnoption/mnscreenadjust.c`: Screen Adjust. The pad or stick moves
//! the picture's centre up to 14 pixels each way (`syVideoSetCenterOffsets`)
//! over a guide picture and a frame; Z centres it; A, B or START saves the
//! offsets to the backup and goes back to Option.
//!
//! The offsets move the N64's video signal on a television. The PSP's LCD
//! shows the whole frame, so the port keeps and saves them without moving
//! its picture (RE-461).

use super::{fill_prim, Draw, Pad, Piece, Scene, FILE_SCREEN_ADJUST};
use crate::backup::Backup;
use ssb_engine::input::N64Buttons;

/// `llMNScreenAdjust*Sprite`.
pub mod sprite {
    pub const INSTRUCTION: u32 = 0x918;
    pub const GUIDE: u32 = 0x98A0;
}

/// The offsets' range either way.
pub const OFFSET_MAX: f32 = 14.0;

/// The menu's state (`sMNScreenAdjust*`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScreenAdjust {
    /// `sMNScreenAdjustOffsetH` and `...V`.
    pub offset_h: f32,
    pub offset_v: f32,
    button_hold_wait: i32,
    total_tics: i32,
}

impl ScreenAdjust {
    /// `mnScreenAdjustInitVars`, from `gSYVideoOffsetLeft` and `...Top`.
    pub fn new(video_offset_h: i16, video_offset_v: i16) -> ScreenAdjust {
        ScreenAdjust {
            offset_h: f32::from(video_offset_h),
            offset_v: f32::from(video_offset_v),
            button_hold_wait: 0,
            total_tics: 0,
        }
    }

    /// The offsets `syVideoSetCenterOffsets` was last given, as `s16`.
    pub fn video_offsets(&self) -> (i16, i16) {
        (self.offset_h as i16, self.offset_v as i16)
    }

    /// `mnScreenAdjustFuncRun`.
    pub fn tick(&mut self, pad: &Pad, backup: &mut Backup) -> Option<Scene> {
        self.total_tics += 1;
        if self.total_tics < 10 {
            return None;
        }
        let mut load = None;
        if self.button_hold_wait != 0 {
            self.button_hold_wait -= 1;
        }
        if pad.stick_in_range_lr(-30, 30) && pad.stick_in_range_ud(-30, 30) {
            self.button_hold_wait = 0;
        }
        if pad.tap(N64Buttons::A | N64Buttons::B | N64Buttons::START) {
            // `mnScreenAdjustBackupOffsets`.
            backup.screen_adjust_h = self.offset_h as i16;
            backup.screen_adjust_v = self.offset_v as i16;
            backup.write();
            load = Some(Scene::Option);
        }
        let max = OFFSET_MAX;
        if pad.tap(N64Buttons::D_UP | N64Buttons::C_UP) && self.offset_v > -max {
            self.offset_v = (self.offset_v - 1.0).max(-max);
        }
        if self.button_hold_wait == 0 {
            let s = pad.stick_ud(30, true);
            if s != 0 && self.offset_v > -max {
                self.offset_v = (self.offset_v - s as f32 / 50.0).max(-max);
            }
        }
        if pad.tap(N64Buttons::D_DOWN | N64Buttons::C_DOWN) && self.offset_v < max {
            self.offset_v = (self.offset_v + 1.0).min(max);
        }
        if self.button_hold_wait == 0 {
            let s = pad.stick_ud(-30, false);
            if s != 0 && self.offset_v < max {
                self.offset_v = (self.offset_v - s as f32 / 50.0).min(max);
            }
        }
        if pad.tap(super::LEFT) && self.offset_h > -max {
            self.offset_h = (self.offset_h - 1.0).max(-max);
        }
        if self.button_hold_wait == 0 {
            let s = pad.stick_lr(-30, false);
            if s != 0 && self.offset_h > -max {
                self.offset_h = (self.offset_h + s as f32 / 50.0).max(-max);
            }
        }
        if pad.tap(super::RIGHT) && self.offset_h < max {
            self.offset_h = (self.offset_h + 1.0).min(max);
        }
        if self.button_hold_wait == 0 {
            let s = pad.stick_lr(30, true);
            if s != 0 && self.offset_h < max {
                self.offset_h = (self.offset_h + s as f32 / 50.0).min(max);
            }
        }
        self.button_hold_wait = 0;
        if pad.tap(N64Buttons::Z) {
            self.offset_h = 0.0;
            self.offset_v = 0.0;
        }
        load
    }

    /// The sprite camera (70): the guide and the instruction; then the
    /// frame camera (40): `mnScreenAdjustFrameProcDisplay`'s lines.
    pub fn visit(&self, f: &mut impl FnMut(Draw)) {
        f(Draw::Sprite(Piece::at(
            FILE_SCREEN_ADJUST,
            sprite::GUIDE,
            10.0,
            10.0,
        )));
        f(Draw::Sprite(
            Piece::clear(FILE_SCREEN_ADJUST, sprite::INSTRUCTION, 16.0, 198.0).prim([0xFF; 3]),
        ));
        let gold = [0xBF, 0xA4, 0x47, 0xFF];
        f(fill_prim(159, 0, 161, 254, gold));
        f(fill_prim(0, 119, 334, 121, gold));
        let grey = [0x8B, 0x8B, 0x8B, 0xFF];
        f(fill_prim(44, 44, 276, 45, grey));
        f(fill_prim(44, 196, 276, 197, grey));
        f(fill_prim(44, 44, 45, 196, grey));
        f(fill_prim(276, 44, 277, 196, grey));
    }
}
