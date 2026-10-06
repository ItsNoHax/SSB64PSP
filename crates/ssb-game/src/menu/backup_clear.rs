//! `mn/mnoption/mnbackupclear.c`: Backup Clear. Six options, each asking
//! "Is it okay?" (All Data Clear asks "Are you sure?" after it); Yes clears
//! that part of the backup (`lbBackupClear*`), corrects the selections
//! against it (`lbBackupCorrectErrors`), writes it and blinks Yes for 60
//! ticks before the options come back.
//!
//! The US build's `mnBackupClearMakeUnused` adds no display, so the
//! Japanese subtitle under the options is not drawn.

use super::{
    fill, Draw, Pad, Piece, Repeat, Scene, DOWN, FILE_BACKUP_CLEAR, FILE_BACKUP_CLEAR_HEADER, UP,
};
use crate::backup::{Backup, Selections};
use ssb_engine::input::N64Buttons;

/// `llMNBackupClear*Sprite` and `*Palette`.
pub mod sprite {
    pub const HEADER_BACKUP_CLEAR: u32 = 0xB60;
    pub const OPTION_NEWCOMERS: u32 = 0x3A00;
    pub const OPTION_1P_HIGH_SCORE: u32 = 0x4050;
    pub const OPTION_VS_RECORD: u32 = 0x46A0;
    pub const OPTION_PRIZE: u32 = 0x5340;
    pub const OPTION_ALL_DATA_CLEAR: u32 = 0x5990;
    pub const OPTION_CIRCLE: u32 = 0x5DB8;
    pub const IS_OKAY_TEXT: u32 = 0x63C8;
    pub const ARE_YOU_SURE_TEXT: u32 = 0x69D8;
    pub const OPTION_BONUS_STAGE_TIME: u32 = 0x7020;
    pub const OPTION_YES: u32 = 0x7580;
    pub const OPTION_NO: u32 = 0x7AB8;
    /// `llMNBackupClearHeaderOptionSprite`, in file 0x4E.
    pub const HEADER_OPTION: u32 = 0xB40;

    /// `OptionYesHighlightPalette`, `OptionYesNotPalette` and
    /// `OptionConfirmPalette`: the Yes sprite's LUTs, by pack LUT index.
    pub const YES_LUTS: [u32; 3] = [0x7500, 0x7528, 0x7550];
    /// `OptionNoHighlightPalette` and `OptionNoNotPalette`.
    pub const NO_LUTS: [u32; 2] = [0x7A60, 0x7A88];
}

/// Pack LUT indices of [`sprite::YES_LUTS`] and [`sprite::NO_LUTS`].
pub const LUT_HIGHLIGHT: u8 = 0;
pub const LUT_NOT: u8 = 1;
pub const LUT_CONFIRM: u8 = 2;

/// `nMNBackupClearOption*`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClearOption {
    Newcomers,
    HighScore1P,
    BonusStageTime,
    VsRecord,
    Prize,
    AllDataClear,
}

impl ClearOption {
    pub const ALL: [ClearOption; 6] = [
        ClearOption::Newcomers,
        ClearOption::HighScore1P,
        ClearOption::BonusStageTime,
        ClearOption::VsRecord,
        ClearOption::Prize,
        ClearOption::AllDataClear,
    ];

    /// `mnBackupClearSetOptionSpriteColors`' `offsets`.
    fn sprite(self) -> u32 {
        match self {
            ClearOption::Newcomers => sprite::OPTION_NEWCOMERS,
            ClearOption::HighScore1P => sprite::OPTION_1P_HIGH_SCORE,
            ClearOption::BonusStageTime => sprite::OPTION_BONUS_STAGE_TIME,
            ClearOption::VsRecord => sprite::OPTION_VS_RECORD,
            ClearOption::Prize => sprite::OPTION_PRIZE,
            ClearOption::AllDataClear => sprite::OPTION_ALL_DATA_CLEAR,
        }
    }
}

/// `mnBackupClearMakeOptionConfirm`'s GObj.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Confirm {
    /// 1 "Is it okay?", 2 "Are you sure?".
    pub kind: u8,
    /// 0 Yes, 1 No.
    pub yes_or_no: u8,
    /// The Yes sprite's LUT, which the confirm blink swaps.
    pub yes_lut: u8,
}

impl Confirm {
    fn new(kind: u8, yes_or_no: u8) -> Confirm {
        Confirm {
            kind,
            yes_or_no,
            yes_lut: if yes_or_no == 0 {
                LUT_HIGHLIGHT
            } else {
                LUT_NOT
            },
        }
    }
}

/// The menu's state (`sMNBackupClear*`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BackupClear {
    pub option: ClearOption,
    /// `sMNBackupClearOptionMenuKind`: 0 the options, 1 and 2 a confirm.
    menu_kind: u8,
    yes_or_no: u8,
    /// `sMNBackupClearOptionConfirmLUTOrigin`.
    lut_origin: u8,
    change_wait: i32,
    total_tics: i32,
    update_wait: i32,
    confirm_anim_length: i32,
    /// The option GObjs, made by `mnBackupClearSetOptionSpriteColors`.
    pub options_shown: bool,
    /// Whether each option was made highlighted.
    highlighted: [bool; 6],
    pub confirm: Option<Confirm>,
    /// Set on the tick All Data Clear ran `lbBackupApplyOptions`.
    pub apply_options: bool,
}

impl Default for BackupClear {
    fn default() -> Self {
        Self::new()
    }
}

impl BackupClear {
    /// `mnBackupClearFuncStart` (`mnBackupClearInitVars`).
    pub fn new() -> BackupClear {
        let mut m = BackupClear {
            option: ClearOption::Newcomers,
            menu_kind: 0,
            yes_or_no: 1,
            lut_origin: LUT_HIGHLIGHT,
            change_wait: 0,
            total_tics: 0,
            update_wait: 10,
            confirm_anim_length: 0,
            options_shown: false,
            highlighted: [false; 6],
            confirm: None,
            apply_options: false,
        };
        m.make_options();
        m
    }

    /// `mnBackupClearSetOptionSpriteColors`.
    fn make_options(&mut self) {
        self.options_shown = true;
        self.highlighted = ClearOption::ALL.map(|o| o == self.option);
    }

    fn make_confirm(&mut self, kind: u8) {
        self.confirm = Some(Confirm::new(kind, self.yes_or_no));
    }

    /// `mnBackupClearApplyOptionID`.
    fn apply(&mut self, backup: &mut Backup, selections: &mut Selections) {
        match self.option {
            ClearOption::Newcomers => backup.clear_newcomers(),
            ClearOption::HighScore1P => backup.clear_1p_high_score(),
            ClearOption::VsRecord => backup.clear_vs_record(),
            ClearOption::BonusStageTime => backup.clear_bonus_stage_time(),
            ClearOption::Prize => backup.clear_prize(),
            ClearOption::AllDataClear => {
                backup.clear_all_data();
                self.apply_options = true;
            }
        }
        backup.correct_errors(selections);
        backup.write();
    }

    /// `mnBackupClearFuncRun`.
    pub fn tick(
        &mut self,
        pad: &Pad,
        backup: &mut Backup,
        selections: &mut Selections,
    ) -> Option<Scene> {
        self.apply_options = false;
        self.total_tics += 1;
        if self.update_wait != 0 {
            self.update_wait -= 1;
            return None;
        }
        if self.confirm_anim_length != 0 {
            self.confirm_anim_length -= 1;
            if self.confirm_anim_length % 10 == 0 {
                if let Some(c) = self.confirm.as_mut() {
                    c.yes_lut = if c.yes_lut == LUT_CONFIRM {
                        self.lut_origin
                    } else {
                        LUT_CONFIRM
                    };
                }
            }
            if self.confirm_anim_length == 0 {
                self.confirm = None;
                self.make_options();
            }
            return None;
        }
        if self.change_wait != 0 {
            self.change_wait -= 1;
        }
        if pad.released(UP | DOWN) {
            self.change_wait = 0;
        }
        match self.menu_kind {
            0 => self.update_main(pad),
            kind => {
                self.update_confirm(pad, kind, backup, selections);
                None
            }
        }
    }

    /// `mnBackupClearUpdateOptionMainMenu`.
    fn update_main(&mut self, pad: &Pad) -> Option<Scene> {
        if pad.tap(N64Buttons::A | N64Buttons::START) {
            self.options_shown = false;
            self.menu_kind = 1;
            self.yes_or_no = 1;
            self.make_confirm(1);
            self.update_wait = 10;
            return None;
        }
        if pad.tap(N64Buttons::B) {
            return Some(Scene::Option);
        }
        let all = ClearOption::ALL;
        let index = all.iter().position(|&o| o == self.option).unwrap_or(0);
        let mut r = Repeat::default();
        if r.check(self.change_wait, pad, UP, true, 20, true) {
            self.change_wait = r.wait_p(7);
            self.highlighted[index] = false;
            self.option = all[if index == 0 { all.len() - 1 } else { index - 1 }];
            if self.option == ClearOption::Newcomers {
                self.change_wait += 8;
            }
            self.highlighted[all.iter().position(|&o| o == self.option).unwrap_or(0)] = true;
        } else if r.check(self.change_wait, pad, DOWN, true, -20, false) {
            self.change_wait = r.wait_n(7);
            self.highlighted[index] = false;
            self.option = all[if index == all.len() - 1 { 0 } else { index + 1 }];
            if self.option == ClearOption::AllDataClear {
                self.change_wait += 8;
            }
            self.highlighted[all.iter().position(|&o| o == self.option).unwrap_or(0)] = true;
        }
        None
    }

    /// `mnBackupClearUpdateOptionConfirmMenu`.
    fn update_confirm(
        &mut self,
        pad: &Pad,
        kind: u8,
        backup: &mut Backup,
        selections: &mut Selections,
    ) {
        if pad.tap(N64Buttons::A | N64Buttons::START) {
            if self.yes_or_no == 0 {
                if kind == 1 && self.option == ClearOption::AllDataClear {
                    self.menu_kind = 2;
                    self.yes_or_no = 1;
                    self.make_confirm(2);
                    self.update_wait = 10;
                } else {
                    if let Some(c) = self.confirm.as_mut() {
                        self.lut_origin = c.yes_lut;
                        c.yes_lut = LUT_CONFIRM;
                    }
                    self.menu_kind = 0;
                    self.confirm_anim_length = 60;
                    self.apply(backup, selections);
                }
            } else {
                self.menu_kind = 0;
                self.confirm = None;
                self.make_options();
                self.update_wait = 10;
            }
            return;
        }
        let mut r = Repeat::default();
        if pad.tap(N64Buttons::B) {
            self.menu_kind = 0;
            self.confirm = None;
            self.make_options();
            self.update_wait = 10;
        } else if pad.tap(super::RIGHT_NO_TRIGGER)
            || r.check_stick(self.change_wait, pad, false, 20, true)
        {
            self.change_wait = super::wait_n(r.stick, 7);
            if self.yes_or_no == 1 {
                self.yes_or_no = 0;
                self.make_confirm(kind);
            }
        } else if (pad.tap(super::LEFT_NO_TRIGGER)
            || r.check_stick(self.change_wait, pad, false, -20, false))
            && self.yes_or_no == 0
        {
            self.yes_or_no = 1;
            self.change_wait = super::wait_n(r.stick, 7);
            self.make_confirm(kind);
        }
    }

    /// The main camera (80): its links 0 (the header), 1 (the US build's
    /// empty subtitle) and 2 (the options or the confirm).
    pub fn visit(&self, f: &mut impl FnMut(Draw)) {
        // `mnBackupClearMakeHeaderSObjs`.
        f(Draw::Sprite(
            Piece::clear(FILE_BACKUP_CLEAR_HEADER, sprite::HEADER_OPTION, 24.0, 17.0)
                .prim([0x5F, 0x58, 0x46]),
        ));
        f(Draw::Sprite(
            Piece::clear(FILE_BACKUP_CLEAR, sprite::HEADER_BACKUP_CLEAR, 133.0, 22.0)
                .env([0; 3])
                .prim([0xF2, 0xC7, 0x0D]),
        ));
        if self.options_shown {
            for (i, o) in ClearOption::ALL.into_iter().enumerate() {
                // `mnBackupClearUpdateOptionTabColors`.
                let prim = if self.highlighted[i] {
                    [0xFF, 0xA8, 0x00]
                } else {
                    [0x7D, 0x45, 0x07]
                };
                f(Draw::Sprite(
                    Piece::clear(FILE_BACKUP_CLEAR, o.sprite(), 95.0, 54.0 + 27.0 * i as f32)
                        .prim(prim),
                ));
            }
        }
        if let Some(c) = self.confirm {
            // `mnBackupClearOptionConfirmProcDisplay`.
            let blue = [0x00, 0x00, 0xFF];
            f(fill(58, 64, 262, 64, blue));
            f(fill(58, 172, 262, 172, blue));
            f(fill(58, 64, 58, 172, blue));
            f(fill(262, 64, 262, 172, blue));
            let mut yes = Piece::clear(FILE_BACKUP_CLEAR, sprite::OPTION_YES, 189.0, 106.0);
            yes.lut = Some(c.yes_lut);
            f(Draw::Sprite(yes));
            let mut no = Piece::clear(FILE_BACKUP_CLEAR, sprite::OPTION_NO, 83.0, 106.0);
            no.lut = Some(if c.yes_or_no == 0 {
                LUT_NOT
            } else {
                LUT_HIGHLIGHT
            });
            f(Draw::Sprite(no));
            let x = if c.yes_or_no == 0 { 193.0 } else { 87.0 };
            let orange = [0xEF, 0x9D, 0x00];
            f(Draw::Sprite(
                Piece::clear(FILE_BACKUP_CLEAR, sprite::OPTION_CIRCLE, x, 110.0).prim(orange),
            ));
            let text = if c.kind == 1 {
                sprite::IS_OKAY_TEXT
            } else {
                sprite::ARE_YOU_SURE_TEXT
            };
            f(Draw::Sprite(
                Piece::clear(FILE_BACKUP_CLEAR, text, 59.0, 83.0).prim(orange),
            ));
        }
    }
}
