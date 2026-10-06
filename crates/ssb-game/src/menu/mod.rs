//! The options and data menus that show and edit the backup: `mnOption`,
//! `mnScreenAdjust` and `mnBackupClear` (`mn/mnoption/`), and `mnData`,
//! `mnVSRecord` and `mnCharacters` (`mn/mndata/`).
//!
//! Each menu is a state machine whose `tick` is the scene's `func_run`
//! (with the processes it adds, in their run order) and whose `visit`
//! yields what its cameras draw, back to front: a camera with a larger
//! priority draws first, then its display links in GObj order. Every one of
//! these cameras has the viewport `(10, 10)` to `(310, 230)`.
//!
//! The menus read the controllers through `scSubsysController*` ([`Pad`])
//! and the change-wait macros of `mndef.h` ([`wait_p`], [`wait_n`]). A tick
//! that calls `syTaskmanSetLoadScene` returns the scene it set
//! ([`Scene`]); the frame still finishes, as the original's does.

pub mod backup_clear;
pub mod characters;
pub mod data;
pub mod option;
pub mod screen_adjust;
pub mod vs_record;

#[cfg(test)]
mod tests;

use ssb_engine::input::N64Buttons;

/// `I_MIN_TO_TICS(5)`: five idle minutes back to the title.
pub const IDLE_RETURN: i32 = 5 * 60 * 60;

/// Every camera's `syRdpSetViewport`.
pub const VIEWPORT: [f32; 4] = [10.0, 10.0, 310.0, 230.0];

/// `llMNCommonFileID`.
pub const FILE_COMMON: u32 = 0x00;
/// `llMNOptionFileID`.
pub const FILE_OPTION: u32 = 0x04;
/// `llMNDataFileID`.
pub const FILE_DATA: u32 = 0x05;
/// `llMNScreenAdjustFileID`.
pub const FILE_SCREEN_ADJUST: u32 = 0x0F;
/// `llMNCharactersFileID`.
pub const FILE_CHARACTERS: u32 = 0x10;
/// `llMNPlayersPortraitsFileID`.
pub const FILE_PORTRAITS: u32 = 0x13;
/// `llMNVSRecordMainFileID`.
pub const FILE_VS_RECORD: u32 = 0x1F;
/// `llMNDataCommonFileID`.
pub const FILE_DATA_COMMON: u32 = 0x20;
/// `llMNCommonFontsFileID`.
pub const FILE_FONTS: u32 = 0x21;
/// `llMNBackupClearFileID`.
pub const FILE_BACKUP_CLEAR: u32 = 0x4D;
/// `llMNBackupClearHeaderOptionFileID`.
pub const FILE_BACKUP_CLEAR_HEADER: u32 = 0x4E;

/// `llMNCommon*Sprite` (`reloc_data.us.h`).
pub mod common {
    pub const OPTION_TAB_LEFT: u32 = 0x1E8;
    pub const OPTION_TAB_MIDDLE: u32 = 0x330;
    pub const OPTION_TAB_RIGHT: u32 = 0x568;
    pub const DECAL_PAPER: u32 = 0x2A30;
    pub const SMASH_LOGO: u32 = 0x31F8;
    pub const SLASH: u32 = 0xBA28;
    pub const SMASH_BROS_COLLAGE: u32 = 0x18000;
}

/// `llMNDataCommon*Sprite`.
pub mod data_common {
    pub const DATA_HEADER: u32 = 0xB40;
    pub const ARROW_L: u32 = 0xBE0;
    pub const ARROW_R: u32 = 0xC80;
}

/// The scenes these menus go to (`nSCKind*`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scene {
    Title,
    ModeSelect,
    Option,
    ScreenAdjust,
    BackupClear,
    Data,
    VsRecord,
    Characters,
    SoundTest,
    AutoDemo,
}

/// One frame of the controllers, as `scSubsysController*` read them: the
/// port's menus take the first connected controller only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Pad {
    pub hold: u16,
    pub tap: u16,
    pub stick_x: i8,
    pub stick_y: i8,
}

pub const UP: u16 = N64Buttons::D_UP | N64Buttons::C_UP;
pub const DOWN: u16 = N64Buttons::D_DOWN | N64Buttons::C_DOWN;
pub const LEFT: u16 = N64Buttons::D_LEFT | N64Buttons::L | N64Buttons::C_LEFT;
pub const RIGHT: u16 = N64Buttons::D_RIGHT | N64Buttons::R | N64Buttons::C_RIGHT;
/// `L_JPAD | L_CBUTTONS` and `R_JPAD | R_CBUTTONS`, without the triggers.
pub const LEFT_NO_TRIGGER: u16 = N64Buttons::D_LEFT | N64Buttons::C_LEFT;
pub const RIGHT_NO_TRIGGER: u16 = N64Buttons::D_RIGHT | N64Buttons::C_RIGHT;
/// Every button `scSubsysControllerCheckNoInputAll` checks.
const ALL_BUTTONS: u16 = N64Buttons::A
    | N64Buttons::B
    | N64Buttons::START
    | N64Buttons::L
    | N64Buttons::R
    | N64Buttons::Z
    | N64Buttons::C_UP
    | N64Buttons::C_LEFT
    | N64Buttons::C_RIGHT
    | N64Buttons::C_DOWN
    | N64Buttons::D_UP
    | N64Buttons::D_LEFT
    | N64Buttons::D_RIGHT
    | N64Buttons::D_DOWN;

impl Pad {
    /// `scSubsysControllerGetPlayerHoldButtons`.
    pub fn hold(&self, mask: u16) -> bool {
        self.hold & mask != 0
    }

    /// `scSubsysControllerGetPlayerTapButtons`.
    pub fn tap(&self, mask: u16) -> bool {
        self.tap & mask != 0
    }

    /// `scSubsysControllerGetPlayerStickUD(range, up_or_down)`: the stick's
    /// y past `range` upwards (or downwards), else 0.
    pub fn stick_ud(&self, range: i32, up: bool) -> i32 {
        let y = i32::from(self.stick_y);
        if (up && range < y) || (!up && range > y) {
            y
        } else {
            0
        }
    }

    /// `scSubsysControllerGetPlayerStickLR(range, right_or_left)`.
    pub fn stick_lr(&self, range: i32, right: bool) -> i32 {
        let x = i32::from(self.stick_x);
        if (right && range < x) || (!right && range > x) {
            x
        } else {
            0
        }
    }

    /// `scSubsysControllerGetPlayerStickInRangeLR(min, max)`.
    pub fn stick_in_range_lr(&self, min: i32, max: i32) -> bool {
        (min..=max).contains(&i32::from(self.stick_x))
    }

    /// `scSubsysControllerGetPlayerStickInRangeUD(min, max)`.
    pub fn stick_in_range_ud(&self, min: i32, max: i32) -> bool {
        (min..=max).contains(&i32::from(self.stick_y))
    }

    /// `scSubsysControllerCheckNoInputAll`.
    pub fn no_input_all(&self) -> bool {
        self.stick_in_range_lr(-20, 20)
            && self.stick_in_range_ud(-20, 20)
            && !self.hold(ALL_BUTTONS)
    }

    /// The menus' "nothing held" test before they clear the change wait:
    /// the stick inside +-20 and none of `masks` held.
    pub fn released(&self, masks: u16) -> bool {
        self.stick_in_range_lr(-20, 20) && self.stick_in_range_ud(-20, 20) && !self.hold(masks)
    }
}

/// `mnCommonGetOptionChangeWaitP`.
pub fn wait_p(stick: i32, div: i32) -> i32 {
    (160 - stick) / div
}

/// `mnCommonGetOptionChangeWaitN`.
pub fn wait_n(stick: i32, div: i32) -> i32 {
    (stick + 160) / div
}

/// The `mnCommonCheckGetOption*` pair a menu tests for one direction:
/// a held button (`is_button`) or else the stick past `range`. `stick` keeps
/// the last stick read, as the original's `stick_range` local does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Repeat {
    pub is_button: bool,
    pub stick: i32,
}

impl Repeat {
    /// `mnCommonCheckGetOptionButtonInput(wait, is_button, mask) ||
    /// mnCommonCheckGetOption{UD,LR}(wait, stick_range, range, b)`.
    pub fn check(
        &mut self,
        wait: i32,
        pad: &Pad,
        mask: u16,
        ud: bool,
        range: i32,
        positive: bool,
    ) -> bool {
        if wait != 0 {
            return false;
        }
        self.is_button = pad.hold(mask);
        if self.is_button {
            return true;
        }
        self.stick = if ud {
            pad.stick_ud(range, positive)
        } else {
            pad.stick_lr(range, positive)
        };
        self.stick != 0
    }

    /// `mnCommonCheckGetOptionStick{UD,LR}` alone.
    pub fn check_stick(
        &mut self,
        wait: i32,
        pad: &Pad,
        ud: bool,
        range: i32,
        positive: bool,
    ) -> bool {
        if wait != 0 {
            return false;
        }
        self.stick = if ud {
            pad.stick_ud(range, positive)
        } else {
            pad.stick_lr(range, positive)
        };
        self.stick != 0
    }

    /// `mnCommonSetOptionChangeWaitP(wait, is_button, stick_range, div)`.
    pub fn wait_p(&self, div: i32) -> i32 {
        if self.is_button {
            12
        } else {
            wait_p(self.stick, div)
        }
    }

    /// `mnCommonSetOptionChangeWaitN`.
    pub fn wait_n(&self, div: i32) -> i32 {
        if self.is_button {
            12
        } else {
            wait_n(self.stick, div)
        }
    }
}

/// One `SObj`: a sprite of a file, its top-left corner on the 320 x 240
/// screen and its colours.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Piece {
    pub file: u32,
    pub offset: u32,
    /// A swapped `sprite.LUT`: the pack's `ROLE_LUT` variant of this index.
    pub lut: Option<u8>,
    pub x: f32,
    pub y: f32,
    /// `sprite.red`, `.green`, `.blue`; `None` keeps the ROM's.
    pub prim: Option<[u8; 3]>,
    /// `sobj->envcolor`; black where the source leaves it unset.
    pub env: [u8; 3],
    /// `SP_TRANSPARENT` set and `SP_FASTCOPY` cleared.
    pub transparent: bool,
}

impl Piece {
    /// A sprite as `lbCommonMakeSObjForGObj` makes it.
    pub fn at(file: u32, offset: u32, x: f32, y: f32) -> Piece {
        Piece {
            file,
            offset,
            lut: None,
            x,
            y,
            prim: None,
            env: [0; 3],
            transparent: false,
        }
    }

    /// The `attr &= ~SP_FASTCOPY; attr |= SP_TRANSPARENT` every menu sets.
    pub fn clear(file: u32, offset: u32, x: f32, y: f32) -> Piece {
        Piece {
            transparent: true,
            ..Piece::at(file, offset, x, y)
        }
    }

    pub fn prim(mut self, rgb: [u8; 3]) -> Piece {
        self.prim = Some(rgb);
        self
    }

    pub fn env(mut self, rgb: [u8; 3]) -> Piece {
        self.env = rgb;
        self
    }
}

/// What a menu draws, back to front.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Draw {
    Sprite(Piece),
    /// A wrapping `SObj` (`cms`/`cmt` wrap with `masks`, `lrs`, `lrt`): the
    /// sprite repeated over `size` from its corner.
    Tiled {
        piece: Piece,
        size: [f32; 2],
    },
    /// `gDPFillRectangle`, `[x0, y0, x1, y1)` in screen pixels: a `G_CYC_FILL`
    /// rectangle's inclusive corner is made exclusive here. Below `0xFF`
    /// alpha it blends (`G_RM_AA_XLU_SURF`).
    Fill {
        rect: [f32; 4],
        rgba: [u8; 4],
    },
    /// `mnCharacters`' fighter under its camera.
    Fighter,
    /// `mnCharacters`' series emblem under its camera.
    Emblem,
}

/// A `G_CYC_FILL` rectangle: inclusive lower-right corner.
pub(crate) fn fill(x0: i32, y0: i32, x1: i32, y1: i32, rgb: [u8; 3]) -> Draw {
    Draw::Fill {
        rect: [x0 as f32, y0 as f32, (x1 + 1) as f32, (y1 + 1) as f32],
        rgba: [rgb[0], rgb[1], rgb[2], 0xFF],
    }
}

/// A one-cycle `G_CC_PRIMITIVE` rectangle: exclusive lower-right corner.
pub(crate) fn fill_prim(x0: i32, y0: i32, x1: i32, y1: i32, rgba: [u8; 4]) -> Draw {
    Draw::Fill {
        rect: [x0 as f32, y0 as f32, x1 as f32, y1 as f32],
        rgba,
    }
}

/// `mnOptionMakeOptionTabs` and `mnDataMakeOptionTab`: a tab's left end,
/// its middle repeated `lrs * 8` texels (`masks` 4, `lrt` 29) and its
/// right end, coloured by `status` (`mn*SetOptionSpriteColors`).
pub(crate) fn option_tab(f: &mut impl FnMut(Draw), x: f32, y: f32, lrs: i32, status: TabStatus) {
    let (env, prim) = status.colors();
    let tint = |p: Piece| p.env(env).prim(prim);
    f(Draw::Sprite(tint(Piece::clear(
        FILE_COMMON,
        common::OPTION_TAB_LEFT,
        x,
        y,
    ))));
    f(Draw::Tiled {
        piece: tint(Piece::clear(
            FILE_COMMON,
            common::OPTION_TAB_MIDDLE,
            x + 16.0,
            y,
        )),
        size: [(lrs * 8) as f32, 29.0],
    });
    f(Draw::Sprite(tint(Piece::clear(
        FILE_COMMON,
        common::OPTION_TAB_RIGHT,
        x + 16.0 + (lrs * 8) as f32,
        y,
    ))));
}

/// `nMNOptionTabStatus*`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabStatus {
    Not,
    Highlight,
    Selected,
}

impl TabStatus {
    pub(crate) fn of(highlighted: bool) -> TabStatus {
        if highlighted {
            TabStatus::Highlight
        } else {
            TabStatus::Not
        }
    }

    /// `mnOptionSetOptionSpriteColors`' pairs: `envcolor` from `.prim`,
    /// `sprite.red`.. from `.env`.
    pub(crate) fn colors(self) -> ([u8; 3], [u8; 3]) {
        match self {
            TabStatus::Highlight => ([0x82, 0x00, 0x28], [0xFF, 0x00, 0x28]),
            TabStatus::Not => ([0x00, 0x00, 0x00], [0x82, 0x82, 0xAA]),
            TabStatus::Selected => ([0x00, 0x00, 0x00], [0xFF, 0xFF, 0xFF]),
        }
    }
}

/// `mnOptionMakeDecals` and `mnDataMakeDecals`: the collage, two paper
/// decals and the dark icon (`icon` of `icon_file`).
pub(crate) fn decals(f: &mut impl FnMut(Draw), icon_file: u32, icon: u32) {
    f(Draw::Sprite(Piece::at(
        FILE_COMMON,
        common::SMASH_BROS_COLLAGE,
        10.0,
        10.0,
    )));
    for (x, y) in [(140.0, 143.0), (225.0, 56.0)] {
        f(Draw::Sprite(
            Piece::clear(FILE_COMMON, common::DECAL_PAPER, x, y).prim([0xA0, 0x78, 0x14]),
        ));
    }
    f(Draw::Sprite(
        Piece::clear(icon_file, icon, 10.0, 10.0).prim([0x99, 0x99, 0x99]),
    ));
}

/// `mnOptionLabelsProcDisplay` and `mnDataLabelsProcDisplay`: the corner's
/// translucent box, then the Smash logo and the menu's title (`title` of
/// `title_file` at `title_at`).
pub(crate) fn labels(
    f: &mut impl FnMut(Draw),
    box_rgba: [u8; 4],
    title_file: u32,
    title: u32,
    title_at: (f32, f32),
) {
    f(fill_prim(225, 143, 310, 230, box_rgba));
    f(Draw::Sprite(
        Piece::clear(FILE_COMMON, common::SMASH_LOGO, 235.0, 158.0).prim([0; 3]),
    ));
    f(Draw::Sprite(
        Piece::clear(title_file, title, title_at.0, title_at.1).prim([0; 3]),
    ));
}
