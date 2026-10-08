//! `mn/mnvsmode/mnvsitemswitch.c`: the Item Switch. The appearance rate
//! (none to very high) and fifteen item toggles; B writes them to
//! `gSCManagerTransferBattleState` (`mnVSItemSwitchSetItemToggles`) and
//! goes back to VS Options. The Green Shell's toggle also switches the Red
//! Shell; with any item on, the four containers are always on.
//!
//! Unlike the other menus, the scene has no return to the title: its
//! `sMNVSItemSwitchReturnTic` is set and never read. The US build's
//! `mnItemSwitchMakeSubtitle` adds no display.

use super::{
    fill_prim, Draw, Pad, Piece, Repeat, Scene, DOWN, FILE_VS_ITEM_SWITCH, LEFT, RIGHT, UP,
};
use crate::item::normal::Appearance;
use crate::players_vs::BattleState;
use crate::sound::{self, id};
use ssb_engine::input::N64Buttons;

/// `llMNVSItemSwitch*Sprite` (`reloc_data.us.h`).
pub mod sprite {
    pub const LABEL_VS_OPTIONS: u32 = 0x9A8;
    pub const LABEL_ITEM_SWITCH: u32 = 0xB20;
    /// `Appearance{None,VeryLow,Low,Middle,High,VeryHigh}Sprite`.
    pub const APPEARANCE: [u32; 6] = [0xCE8, 0xEA8, 0xF98, 0x10D0, 0x11E8, 0x13A8];
    pub const TOGGLE_ON: u32 = 0x1488;
    pub const TOGGLE_OFF: u32 = 0x1568;
    pub const TOGGLE_SLASH: u32 = 0x1608;
    pub const DECAL_BUTTON: u32 = 0x3430;
    pub const ITEM_LIST: u32 = 0x5E60;
    pub const CURSOR: u32 = 0x63A8;
}

/// `mnVSItemSwitchMakeAppearance`'s `pos_x`, by rate.
const APPEARANCE_X: [f32; 6] = [242.0, 240.0, 254.0, 244.0, 252.0, 238.0];

/// `nSCBattleItemSwitch*`, in order.
const RATES: [Appearance; 6] = [
    Appearance::None,
    Appearance::VeryLow,
    Appearance::Low,
    Appearance::Middle,
    Appearance::High,
    Appearance::VeryHigh,
];

/// `nITKind*` the toggles switch.
pub mod kind {
    pub const BOX: u8 = 0;
    pub const TARU: u8 = 1;
    pub const CAPSULE: u8 = 2;
    pub const EGG: u8 = 3;
    pub const TOMATO: u8 = 4;
    pub const HEART: u8 = 5;
    pub const STAR: u8 = 6;
    pub const SWORD: u8 = 7;
    pub const BAT: u8 = 8;
    pub const HARISEN: u8 = 9;
    pub const STAR_ROD: u8 = 10;
    pub const L_GUN: u8 = 11;
    pub const F_FLOWER: u8 = 12;
    pub const HAMMER: u8 = 13;
    pub const MS_BOMB: u8 = 14;
    pub const BOMB_HEI: u8 = 15;
    pub const N_BUMPER: u8 = 16;
    pub const G_SHELL: u8 = 17;
    pub const R_SHELL: u8 = 18;
    pub const M_BALL: u8 = 19;
}

/// `dMNVSItemSwitchTogglesItemKinds`: row 0 is the appearance rate.
pub const TOGGLE_KINDS: [u8; ROWS] = [
    0,
    kind::SWORD,
    kind::BAT,
    kind::HAMMER,
    kind::HARISEN,
    kind::MS_BOMB,
    kind::BOMB_HEI,
    kind::N_BUMPER,
    kind::G_SHELL,
    kind::M_BALL,
    kind::L_GUN,
    kind::F_FLOWER,
    kind::STAR_ROD,
    kind::TOMATO,
    kind::HEART,
    kind::STAR,
];

/// `(nITKindUtilityEnd - nITKindUtilityStart) + 1`: the rows.
pub const ROWS: usize = 16;

/// The menu's state (`sMNVSItemSwitch*`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VsItemSwitchMenu {
    /// `sMNVSItemSwitchOptionSelectID`: 0 the rate, 1 to 15 the toggles.
    pub select: usize,
    /// `sMNVSItemSwitchOptionStatuses`: `[0]` the rate
    /// (`nSCBattleItemSwitch*`), the rest on (1) or off (0).
    pub statuses: [u8; ROWS],
    change_wait: i32,
    total_tics: i32,
    scene_curr: Scene,
    /// `sMNVSItemSwitchOptionGObjs[0]` was remade: the rate's display now
    /// follows the toggles'.
    appearance_remade: bool,
}

impl VsItemSwitchMenu {
    /// `mnVSItemSwitchFuncStart` (`mnVSItemSwitchInitVars`,
    /// `mnVSItemSwitchGetItemSettings`).
    pub fn new(state: &BattleState) -> VsItemSwitchMenu {
        let mut statuses = [0; ROWS];
        statuses[0] = state.item_appearance as u8;
        for i in 1..ROWS {
            statuses[i] = u8::from((1u32 << TOGGLE_KINDS[i]) & state.item_toggles != 0);
        }
        VsItemSwitchMenu {
            select: 0,
            statuses,
            change_wait: 0,
            total_tics: 0,
            scene_curr: Scene::VsItemSwitch,
            appearance_remade: false,
        }
    }

    /// The appearance rate on screen.
    pub fn appearance(&self) -> Appearance {
        RATES[usize::from(self.statuses[0]).min(5)]
    }

    /// `mnVSItemSwitchSetItemToggles` (`mnVSItemSwitchCheckAllTogglesOff`,
    /// `mnVSItemSwitchSetItemSettings`).
    fn set_item_toggles(&self, state: &mut BattleState) {
        if self.statuses[1..].iter().all(|&s| s == 0) {
            state.item_toggles = 0;
            return;
        }
        state.item_appearance = self.appearance();
        for (&k, &status) in TOGGLE_KINDS.iter().zip(&self.statuses).skip(1) {
            let mut bits = 1u32 << k;
            if k == kind::G_SHELL {
                bits |= 1 << kind::R_SHELL;
            }
            if status != 0 {
                state.item_toggles |= bits;
            } else {
                state.item_toggles &= !bits;
            }
        }
        state.item_toggles |=
            (1 << kind::EGG) | (1 << kind::CAPSULE) | (1 << kind::TARU) | (1 << kind::BOX);
    }

    fn step_rate(&mut self, up: bool) {
        self.statuses[0] = match (up, self.statuses[0]) {
            (true, 5) => 0,
            (true, r) => r + 1,
            (false, 0) => 5,
            (false, r) => r - 1,
        };
        // `mnVSItemSwitchUpdateOption`: the rate's GObj is remade.
        self.appearance_remade = true;
    }

    /// `mnVSItemSwitchFuncRun`. Returns the scene `syTaskmanSetLoadScene`
    /// loads; the rest of the frame still runs, as the original's does.
    pub fn tick(&mut self, pad: &Pad, state: &mut BattleState) -> Option<Scene> {
        self.total_tics += 1;
        if self.total_tics < 10 {
            return None;
        }
        let mut load = false;
        if self.change_wait != 0 {
            self.change_wait -= 1;
        }
        if pad.released(UP | RIGHT | DOWN | LEFT) {
            self.change_wait = 0;
        }
        if pad.tap(N64Buttons::B) {
            self.scene_curr = Scene::VsOptions;
            self.set_item_toggles(state);
            load = true;
        }
        let mut r = Repeat::default();
        if r.check(self.change_wait, pad, UP, true, 20, true) {
            sound::play_fgm(id::nSYAudioFGMMenuScroll2);
            self.change_wait = r.wait_p(7);
            self.select = if self.select == 0 {
                ROWS - 1
            } else {
                self.select - 1
            };
            if self.select == 0 {
                self.change_wait += 8;
            }
        }
        if r.check(self.change_wait, pad, DOWN, true, -20, false) {
            sound::play_fgm(id::nSYAudioFGMMenuScroll2);
            self.change_wait = r.wait_n(7);
            self.select = if self.select == ROWS - 1 {
                0
            } else {
                self.select + 1
            };
            if self.select == ROWS - 1 {
                self.change_wait += 8;
            }
        }
        if r.check(self.change_wait, pad, LEFT, false, -20, false) {
            if self.select == 0 {
                sound::play_fgm(id::nSYAudioFGMMenuScroll1);
                self.step_rate(false);
            } else if self.statuses[self.select] == 0 {
                sound::play_fgm(id::nSYAudioFGMMenuScroll1);
                self.statuses[self.select] = 1;
            }
            self.change_wait = r.wait_n(7);
        }
        if r.check(self.change_wait, pad, RIGHT, false, 20, true) {
            if self.select == 0 {
                sound::play_fgm(id::nSYAudioFGMMenuScroll1);
                self.step_rate(true);
            } else if self.statuses[self.select] != 0 {
                sound::play_fgm(id::nSYAudioFGMMenuScroll1);
                self.statuses[self.select] = 0;
            }
            self.change_wait = r.wait_p(7);
        }
        if pad.tap(N64Buttons::A) {
            sound::play_fgm(id::nSYAudioFGMMenuScroll1);
            if self.select == 0 {
                self.step_rate(true);
            } else {
                self.statuses[self.select] = u8::from(self.statuses[self.select] == 0);
            }
        }
        load.then_some(self.scene_curr)
    }

    /// The cameras back to front: the decal (60), the labels, item list
    /// and settings (40), then the cursor (30). Every camera has the
    /// viewport `(10, 10)` to `(310, 230)`; the default camera (100) first
    /// fills the screen with opaque black (`COBJ_FLAG_FILLCOLOR`).
    pub fn visit(&self, f: &mut impl FnMut(Draw)) {
        // `mnVSItemSwitchMakeDecal`.
        f(Draw::Sprite(
            Piece::clear(FILE_VS_ITEM_SWITCH, sprite::DECAL_BUTTON, 10.0, 10.0)
                .prim([0x48, 0x2A, 0x23]),
        ));
        // `mnVSItemSwitchLabelsProcDisplay`.
        f(fill_prim(79, 34, 310, 39, [0x80, 0x80, 0x80, 0xFF]));
        f(Draw::Sprite(
            Piece::clear(FILE_VS_ITEM_SWITCH, sprite::LABEL_VS_OPTIONS, 84.0, 24.0)
                .prim([0xF2, 0xC7, 0x0D]),
        ));
        f(Draw::Sprite(
            Piece::clear(FILE_VS_ITEM_SWITCH, sprite::LABEL_ITEM_SWITCH, 222.0, 30.0)
                .prim([0xFF; 3]),
        ));
        // `mnVSItemSwitchMakeItemList`.
        f(Draw::Sprite(
            Piece::clear(FILE_VS_ITEM_SWITCH, sprite::ITEM_LIST, 125.0, 48.0).prim([0xFF; 3]),
        ));
        // `mnVSItemSwitchInitToggles`.
        if !self.appearance_remade {
            self.appearance_sprite(f);
        }
        for i in 1..ROWS {
            let y = (i * 10 + 54) as f32;
            let (on, off) = if self.statuses[i] != 0 {
                (PICKED, UNPICKED)
            } else {
                (UNPICKED, PICKED)
            };
            let word = |offset, x, rgb| {
                Draw::Sprite(Piece::clear(FILE_VS_ITEM_SWITCH, offset, x, y).prim(rgb))
            };
            f(word(sprite::TOGGLE_ON, 244.0, on));
            f(word(sprite::TOGGLE_OFF, 244.0 + 21.0 + 5.0, off));
            f(word(sprite::TOGGLE_SLASH, 244.0 + 21.0, UNPICKED));
        }
        if self.appearance_remade {
            self.appearance_sprite(f);
        }
        // `mnVSItemSwitchMakeCursor` (`mnVSItemSwitchSetCursorPosition`).
        let y = if self.select == 0 {
            47.0
        } else {
            (self.select * 10 + 51) as f32
        };
        f(Draw::Sprite(
            Piece::clear(FILE_VS_ITEM_SWITCH, sprite::CURSOR, 115.0, y).prim([0xFF, 0xDE, 0x00]),
        ));
    }

    /// `mnVSItemSwitchMakeAppearance`.
    fn appearance_sprite(&self, f: &mut impl FnMut(Draw)) {
        let rate = usize::from(self.statuses[0]).min(5);
        f(Draw::Sprite(
            Piece::clear(
                FILE_VS_ITEM_SWITCH,
                sprite::APPEARANCE[rate],
                APPEARANCE_X[rate],
                49.0,
            )
            .prim([0xFF, 0x00, 0x00]),
        ));
    }
}

/// A selected toggle word's colour, and the other's.
const PICKED: [u8; 3] = [0xFF, 0x00, 0x28];
const UNPICKED: [u8; 3] = [0x32, 0x32, 0x32];
