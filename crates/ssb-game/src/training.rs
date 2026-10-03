//! Training Mode's menu — the logic of `sc/sc1pmode/sc1ptrainingmode.c`:
//! `sSC1PTrainingModeMenu` as `sc1PTrainingModeInitVars` sets it up,
//! START into and B/START out of the menu, the stick's held/tapped/repeat
//! directions, the six main options and the CP, Item, Speed and View
//! settings, the Reset and Exit choices, the slowed-down speeds' skipped
//! ticks (`sc1PTrainingModeCheckLagTic`) and the CPU dummy's behaviour
//! (`dSC1PTrainingModeDummyBehaviors`).
//!
//! The host runs the effects a frame reports: it pauses, spawns the item,
//! moves the camera and reloads or leaves the scene. The menu's sprites
//! and the stat display are drawn from this state.

use crate::computer::Behavior;
use crate::item::ItemKind;
use ssb_engine::input::{ControllerState, N64Buttons};

/// `nSC1PTrainingModeMenuMain*`: the main menu's rows, top to bottom.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MainOption {
    Cp,
    Item,
    Speed,
    View,
    Reset,
    Exit,
}

impl MainOption {
    pub const ALL: [MainOption; 6] = [
        MainOption::Cp,
        MainOption::Item,
        MainOption::Speed,
        MainOption::View,
        MainOption::Reset,
        MainOption::Exit,
    ];
}

/// `nSC1PTrainingModeMenuCPEnumCount`.
pub const CP_OPTION_COUNT: u8 = 5;
/// `nSC1PTrainingModeMenuItemEnumCount`: None, then the 16 utilities.
pub const ITEM_OPTION_COUNT: u8 = 17;
/// `nSC1PTrainingModeMenuSpeedEnumCount`.
pub const SPEED_OPTION_COUNT: u8 = 4;
/// `nSC1PTrainingModeMenuViewEnumCount`.
pub const VIEW_OPTION_COUNT: u8 = 2;

/// `dSC1PTrainingModeDummyBehaviors`: Stand, Walk, Evade, Jump, Attack.
pub const DUMMY_BEHAVIORS: [Behavior; CP_OPTION_COUNT as usize] = [
    Behavior::Stand,
    Behavior::Walk,
    Behavior::Evade,
    Behavior::Jump,
    Behavior::Default,
];

/// `dSC1PTrainingModeLagIntervals`: per speed, the ticks that run before
/// a skip and the ticks skipped. Full, 2/3, 1/2, 1/4.
pub const LAG_INTERVALS: [[u8; 2]; SPEED_OPTION_COUNT as usize] = [[0, 0], [1, 1], [0, 1], [0, 3]];

/// `nSC1PTrainingModeMenuViewCloseUp`.
pub const VIEW_CLOSE_UP: u8 = 0;
/// `nSC1PTrainingModeMenuViewNormal`.
pub const VIEW_NORMAL: u8 = 1;

/// Close-Up's `gmCameraSetStatusPlayerZoom` pan scale and field of view.
pub const CLOSE_UP_PAN_SCALE: f32 = 0.1;
pub const CLOSE_UP_FOV: f32 = 28.0;

/// `sc1PTrainingModeGetItemCount() < 4`: the most items on the stage
/// before the Item option refuses.
pub const ITEM_LIMIT: usize = 4;
/// Ticks between two item spawns.
const ITEM_SPAWN_WAIT: u8 = 8;
/// The view's magnifying glass stays hidden this long after Normal.
const MAGNIFY_WAIT: u16 = 180;
/// `rapid_scroll_wait` before the first repeat, then between repeats.
const RAPID_SCROLL_FIRST: i32 = 30;
const RAPID_SCROLL_NEXT: i32 = 5;
/// `sc1PTrainingModeUpdateMenuInputs`: stick past this reads as a D-pad.
const STICK_THRESHOLD: i8 = 40;

const DIR_UP: u16 = N64Buttons::D_UP;
const DIR_DOWN: u16 = N64Buttons::D_DOWN;
const DIR_LEFT: u16 = N64Buttons::D_LEFT;
const DIR_RIGHT: u16 = N64Buttons::D_RIGHT;

/// Whether an item counts toward [`ITEM_LIMIT`]: a common kind
/// (`<= nITKindCommonEnd`) or a Poké Ball's Pokémon
/// (`>= nITKindMBallMonsterStart`), not a fighter's or the stage's.
pub fn counts_toward_limit(kind: ItemKind) -> bool {
    kind.common_index().is_some() || matches!(kind, ItemKind::MMonster(_))
}

/// The `ITKind` an Item option spawns: `option + (nITKindUtilityStart - 1)`.
/// Option 0 is None.
pub fn item_option_kind(option: u8) -> Option<u8> {
    (option != 0 && option < ITEM_OPTION_COUNT).then_some(option + 3)
}

/// `sc1PTrainingModeItemDisplayProcDisplay`'s held item as an Item option:
/// None for no item or a non-common one.
pub fn held_item_option(kind: Option<ItemKind>) -> u8 {
    kind.and_then(ItemKind::common_index)
        .filter(|&i| i >= 4)
        .map_or(0, |i| i - 3)
}

/// What a menu frame did, for the host to carry out and the sprites to
/// follow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MenuFrame {
    /// The CP option moved: the dummy now runs [`TrainingMenu::dummy_behavior`].
    pub cp_changed: bool,
    pub item_changed: bool,
    pub speed_changed: bool,
    /// The View option moved to this setting: the camera zooms on the
    /// player (`gmCameraSetStatusPlayerZoom`) or goes back to the battle
    /// camera (`gmCameraSetStatusDefault`).
    pub view_changed: Option<u8>,
    /// `itManagerMakeItemSetupCommon` for this `ITKind`, 200 above the
    /// player, rising at 30.
    pub spawn_item: Option<u8>,
    /// `nSYAudioFGMMenuDenied`: four items are already out.
    pub spawn_denied: bool,
    /// The cursor moved to another row.
    pub main_changed: bool,
    /// Left the menu: game status back to Go, both fighters' control
    /// unlocked. `true` when B left it, which the player's fighter sees
    /// as held (`fp->input.pl.button_hold |= B_BUTTON`).
    pub leave: Option<bool>,
    /// Reset (`exit_or_reset = 1`) or Exit chose to load the next scene.
    pub load_scene: bool,
}

/// `SC1PTrainingModeMenu`'s state and settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrainingMenu {
    pub stats: crate::training_layer::Stats,
    pub main_option: MainOption,
    pub item_option: u8,
    pub cp_option: u8,
    pub speed_option: u8,
    pub view_option: u8,
    /// The CPU dummy's port.
    pub dummy: u8,
    button_hold: u16,
    button_tap: u16,
    button_queue: u16,
    rapid_scroll_wait: i32,
    pub lagtic_wait: u8,
    pub frameadvance_wait: u8,
    pub item_spawn_wait: u8,
    /// `exit_or_reset`: Reset reloads Training, Exit goes to its select.
    pub exit_or_reset: bool,
    pub magnify_wait: u16,
    /// `gIFCommonPlayerInterface.is_magnify_display`.
    pub magnify_display: bool,
    /// The stick must come back to neutral after the menu opens.
    is_read_menu_inputs: bool,
}

impl TrainingMenu {
    /// `sc1PTrainingModeInitVars`'s menu: CP Stand, no item, full speed,
    /// the normal view, the cursor on CP. The dummy takes port 1 unless
    /// the player is on it.
    pub fn new(player: u8) -> TrainingMenu {
        TrainingMenu {
            stats: crate::training_layer::Stats::default(),
            main_option: MainOption::Cp,
            item_option: 0,
            cp_option: 0,
            speed_option: 0,
            view_option: VIEW_NORMAL,
            dummy: if player == 0 { 1 } else { 0 },
            button_hold: 0,
            button_tap: 0,
            button_queue: 0,
            rapid_scroll_wait: RAPID_SCROLL_FIRST,
            lagtic_wait: 0,
            frameadvance_wait: 0,
            item_spawn_wait: 0,
            exit_or_reset: false,
            magnify_wait: 0,
            // `sc1PTrainingModeInitDisplayVars`.
            magnify_display: true,
            is_read_menu_inputs: false,
        }
    }

    /// `sc1PTrainingModeUpdateDummyBehavior`: the CP option's behaviour;
    /// the trait becomes `nFTComputerTraitNone`.
    pub fn dummy_behavior(&self) -> Behavior {
        DUMMY_BEHAVIORS[usize::from(self.cp_option)]
    }

    /// `sc1PTrainingModeCheckEnterMenu`: START during Go opens the menu
    /// unless the player's fighter ignores menus (`is_menu_ignore`). The
    /// host pauses and locks both fighters' control.
    pub fn check_enter(&mut self, tapped: N64Buttons, menu_ignore: bool) -> bool {
        if tapped.contains(N64Buttons::START) && !menu_ignore {
            self.is_read_menu_inputs = false;
            true
        } else {
            false
        }
    }

    /// `sc1PTrainingModeUpdateMenuInputs`: the stick as D-pad directions,
    /// tapped on change and repeated after 30 ticks, every 5.
    fn update_inputs(&mut self, controller: &ControllerState) {
        let mut buttons = 0;
        if controller.stick_x > STICK_THRESHOLD {
            buttons |= DIR_RIGHT;
        }
        if controller.stick_x < -STICK_THRESHOLD {
            buttons |= DIR_LEFT;
        }
        if controller.stick_y > STICK_THRESHOLD {
            buttons |= DIR_UP;
        }
        if controller.stick_y < -STICK_THRESHOLD {
            buttons |= DIR_DOWN;
        }
        if !self.is_read_menu_inputs {
            if buttons == 0 {
                self.is_read_menu_inputs = true;
            }
            return;
        }
        self.button_tap = (buttons ^ self.button_hold) & buttons;
        if buttons ^ self.button_hold != 0 {
            self.button_queue = self.button_tap;
            self.rapid_scroll_wait = RAPID_SCROLL_FIRST;
        } else {
            self.rapid_scroll_wait -= 1;
            if self.rapid_scroll_wait > 0 {
                self.button_queue = 0;
            } else {
                self.button_queue = buttons;
                self.rapid_scroll_wait = RAPID_SCROLL_NEXT;
            }
        }
        self.button_hold = buttons;
    }

    /// `sc1PTrainingModeCheckUpdateOptionID`: left/right steps `option`
    /// through `0..count`, wrapping.
    fn step_option(&self, option: &mut u8, count: u8) -> bool {
        if self.button_queue & (DIR_LEFT | DIR_RIGHT) == 0 {
            return false;
        }
        if self.button_queue & DIR_LEFT != 0 {
            *option = if *option == 0 { count - 1 } else { *option - 1 };
        } else {
            *option += 1;
            if *option >= count {
                *option = 0;
            }
        }
        true
    }

    /// `sc1PTrainingModeUpdateMenu`: one paused tick. `item_count` is
    /// `sc1PTrainingModeGetItemCount` (items for which
    /// [`counts_toward_limit`] holds).
    pub fn update(
        &mut self,
        controller: &ControllerState,
        tapped: N64Buttons,
        item_count: usize,
    ) -> MenuFrame {
        let mut frame = MenuFrame::default();
        self.update_inputs(controller);
        let a = tapped.contains(N64Buttons::A);
        // `dSC1PTrainingModeMenuUpdateFuncList`.
        let chose = match self.main_option {
            MainOption::Cp => {
                let mut o = self.cp_option;
                frame.cp_changed = self.step_option(&mut o, CP_OPTION_COUNT);
                self.cp_option = o;
                false
            }
            MainOption::Item => {
                let mut o = self.item_option;
                frame.item_changed = self.step_option(&mut o, ITEM_OPTION_COUNT);
                self.item_option = o;
                if self.item_spawn_wait == 0 {
                    if a {
                        if let Some(kind) = item_option_kind(self.item_option) {
                            if item_count < ITEM_LIMIT {
                                frame.spawn_item = Some(kind);
                                self.item_spawn_wait = ITEM_SPAWN_WAIT;
                            } else {
                                frame.spawn_denied = true;
                            }
                        }
                    }
                } else {
                    self.item_spawn_wait -= 1;
                }
                false
            }
            MainOption::Speed => {
                let mut o = self.speed_option;
                frame.speed_changed = self.step_option(&mut o, SPEED_OPTION_COUNT);
                self.speed_option = o;
                if frame.speed_changed {
                    self.lagtic_wait = 0;
                    self.frameadvance_wait = 0;
                }
                false
            }
            MainOption::View => {
                let mut o = self.view_option;
                if self.step_option(&mut o, VIEW_OPTION_COUNT) {
                    self.view_option = o;
                    if o == VIEW_NORMAL {
                        self.magnify_wait = MAGNIFY_WAIT;
                    } else {
                        self.magnify_display = false;
                        self.magnify_wait = 0;
                    }
                    frame.view_changed = Some(o);
                }
                false
            }
            MainOption::Reset if a => {
                self.exit_or_reset = true;
                true
            }
            MainOption::Exit if a => true,
            MainOption::Reset | MainOption::Exit => false,
        };
        if chose {
            frame.load_scene = true;
            return frame;
        }
        // `sc1PTrainingModeUpdateMainOption`.
        if self.button_queue & (DIR_UP | DIR_DOWN) != 0 {
            let i = MainOption::ALL
                .iter()
                .position(|&o| o == self.main_option)
                .unwrap_or(0);
            let n = MainOption::ALL.len();
            let i = if self.button_queue & DIR_UP != 0 {
                (i + n - 1) % n
            } else {
                (i + 1) % n
            };
            self.main_option = MainOption::ALL[i];
            frame.main_changed = true;
        }
        // `sc1PTrainingModeCheckLeaveMenu`.
        if tapped.0 & (N64Buttons::B | N64Buttons::START) != 0 {
            frame.leave = Some(tapped.contains(N64Buttons::B));
        }
        frame
    }

    /// `sc1PTrainingModeCheckLagTic`: `true` skips this tick's
    /// `gcRunAll`; only the camera runs.
    pub fn check_lag_tic(&mut self) -> bool {
        let interval = LAG_INTERVALS[usize::from(self.speed_option)];
        if self.lagtic_wait == 0 {
            if self.frameadvance_wait == 0 {
                self.lagtic_wait = interval[0];
            } else {
                self.frameadvance_wait -= 1;
                return true;
            }
        } else {
            self.lagtic_wait -= 1;
        }
        if self.lagtic_wait == 0 {
            self.frameadvance_wait = interval[1];
        }
        false
    }

    /// `sc1PTrainingModeViewOptionProcUpdate`, run with the other GObj
    /// processes: the magnifying glass comes back 180 ticks after Normal.
    pub fn tick_processes(&mut self) {
        self.stats.tick();
        if self.magnify_wait != 0 {
            self.magnify_wait -= 1;
            if self.magnify_wait == 0 {
                self.magnify_display = true;
            }
        }
    }
}

#[cfg(test)]
#[path = "training_tests.rs"]
mod tests;
