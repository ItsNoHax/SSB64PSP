//! The stage select — `mnMapsFuncRun`, `mnMapsSaveSceneData`,
//! `mnMapsInitVars`, `mnMapsCheckLocked`, `mnMapsGetGroundKind` and
//! `mnMapsGetSlot` (`mn/mnmaps/mnmaps.c`). The cursor, the scroll wait,
//! the random pick and the idle return to the title. The previews, names
//! and sound effects are presentation and stay with the host.

use ssb_engine::input::{ControllerState, N64Buttons};

/// `nGRKind` values of the nine VS stages (`gr/grdef.h`).
pub mod gkind {
    pub const CASTLE: u8 = 0;
    pub const SECTOR: u8 = 1;
    pub const JUNGLE: u8 = 2;
    pub const ZEBES: u8 = 3;
    pub const HYRULE: u8 = 4;
    pub const YOSTER: u8 = 5;
    pub const PUPUPU: u8 = 6;
    pub const YAMABUKI: u8 = 7;
    pub const INISHIE: u8 = 8;
    /// The random slot's stand-in kind.
    pub const RANDOM: u8 = 0xDE;
}

/// `LBBACKUP_UNLOCK_MASK_INISHIE`: bit `nLBBackupUnlockInishie` (4).
pub const UNLOCK_MASK_INISHIE: u8 = 1 << 4;

/// `dSCManagerDefaultSceneData.maps_training_gkind` and
/// `maps_vsmode_gkind`.
pub const DEFAULT_GKIND: u8 = gkind::CASTLE;

/// The random slot.
pub const RANDOM_SLOT: usize = 9;

/// `mnMapsGetGroundKind`'s table: two rows of five slots, the random slot
/// last.
const SLOT_GKINDS: [u8; 10] = [
    gkind::CASTLE,
    gkind::JUNGLE,
    gkind::HYRULE,
    gkind::ZEBES,
    gkind::INISHIE,
    gkind::YOSTER,
    gkind::PUPUPU,
    gkind::SECTOR,
    gkind::YAMABUKI,
    gkind::RANDOM,
];

/// `I_MIN_TO_TICS(5)`: idle ticks before the screen returns to the title.
const RETURN_TICS: u32 = 5 * 60 * 60;
/// Ticks before the screen reads any input.
const INPUT_START_TICS: u32 = 10;
/// Scroll wait after a button move.
const BUTTON_SCROLL_WAIT: i32 = 12;
/// Stick deflection that moves the cursor, and the neutral box half-width.
const STICK_RANGE: i8 = 20;

const UP: u16 = N64Buttons::D_UP | N64Buttons::C_UP;
const DOWN: u16 = N64Buttons::D_DOWN | N64Buttons::C_DOWN;
const LEFT: u16 = N64Buttons::D_LEFT | N64Buttons::L | N64Buttons::C_LEFT;
const RIGHT: u16 = N64Buttons::D_RIGHT | N64Buttons::R | N64Buttons::C_RIGHT;

/// `mnMapsGetGroundKind`.
pub fn slot_gkind(slot: usize) -> u8 {
    SLOT_GKINDS[slot.min(RANDOM_SLOT)]
}

/// `mnMapsGetSlot`. Kinds outside the table fall to slot 0, where the
/// original's switch has no default and returns an undefined value.
pub fn gkind_slot(gkind: u8) -> usize {
    SLOT_GKINDS.iter().position(|&k| k == gkind).unwrap_or(0)
}

/// What a tick asks the host to do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// A or START: load the battle (or Training) on `gkind`.
    Confirm { gkind: u8 },
    /// B: back to the character select.
    Back,
    /// Five idle minutes: back to the title.
    Timeout,
}

/// The scene data a stage select writes back (`mnMapsSaveSceneData`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Saved {
    /// `gSCManagerSceneData.gkind`: the stage to load.
    pub gkind: u8,
    /// `maps_training_gkind` or `maps_vsmode_gkind`: the slot's kind,
    /// `RANDOM` for the random slot, so the cursor returns there.
    pub remembered: u8,
}

/// `mnMaps`'s state.
#[derive(Clone, Debug)]
pub struct StageSelect {
    pub cursor_slot: usize,
    unlocked_mask: u8,
    total_tics: u32,
    return_tic: u32,
    scroll_wait: i32,
}

impl StageSelect {
    /// `mnMapsInitVars`: the cursor starts on the remembered kind.
    pub fn new(remembered_gkind: u8, unlocked_mask: u8) -> Self {
        StageSelect {
            cursor_slot: gkind_slot(remembered_gkind),
            unlocked_mask,
            total_tics: 0,
            return_tic: RETURN_TICS,
            scroll_wait: 0,
        }
    }

    /// `mnMapsCheckLocked`.
    pub fn is_locked(&self, gkind: u8) -> bool {
        gkind == gkind::INISHIE && self.unlocked_mask & UNLOCK_MASK_INISHIE == 0
    }

    /// `mnMapsSaveSceneData`. The random slot draws until it lands on an
    /// unlocked kind other than `previous_gkind`; `rand` stands for
    /// `syUtilsRandTimeUCharRange(9)`, called once per draw.
    pub fn save(&self, previous_gkind: u8, mut rand: impl FnMut() -> u8) -> Saved {
        let remembered = slot_gkind(self.cursor_slot);
        let gkind = if self.cursor_slot == RANDOM_SLOT {
            loop {
                let k = rand();
                if !self.is_locked(k) && k != previous_gkind {
                    break k;
                }
            }
        } else {
            remembered
        };
        Saved { gkind, remembered }
    }

    /// One `mnMapsFuncRun` tick for one controller: `taps` are this
    /// frame's newly pressed buttons.
    pub fn tick(&mut self, input: ControllerState, taps: N64Buttons) -> Option<Outcome> {
        self.total_tics += 1;
        if self.total_tics < INPUT_START_TICS {
            return None;
        }
        if self.total_tics == self.return_tic {
            return Some(Outcome::Timeout);
        }
        let hold = input.buttons.0;
        let (x, y) = (input.stick_x, input.stick_y);
        // `scSubsysControllerCheckNoInputAll`: any held button or a stick
        // outside the dead zone restarts the idle timer.
        if hold != 0
            || !(-STICK_RANGE..=STICK_RANGE).contains(&x)
            || !(-STICK_RANGE..=STICK_RANGE).contains(&y)
        {
            self.return_tic = self.total_tics + RETURN_TICS;
        }
        if self.scroll_wait != 0 {
            self.scroll_wait -= 1;
        }
        if (-STICK_RANGE..=STICK_RANGE).contains(&x)
            && (-STICK_RANGE..=STICK_RANGE).contains(&y)
            && hold & (UP | RIGHT) == 0
            && hold & (DOWN | LEFT) == 0
        {
            self.scroll_wait = 0;
        }
        if taps.0 & (N64Buttons::A | N64Buttons::START) != 0 {
            return Some(Outcome::Confirm {
                gkind: slot_gkind(self.cursor_slot),
            });
        }
        if taps.0 & N64Buttons::B != 0 {
            return Some(Outcome::Back);
        }
        if self.scroll_wait != 0 {
            return None;
        }
        let stick = i32::from;
        if hold & UP != 0 || y > STICK_RANGE {
            if self.cursor_slot >= 5 && !self.is_locked(slot_gkind(self.cursor_slot - 5)) {
                self.cursor_slot -= 5;
            }
            self.scroll_wait = if hold & UP != 0 {
                BUTTON_SCROLL_WAIT
            } else {
                (160 - stick(y)) / 7
            };
            return None;
        }
        if hold & DOWN != 0 || y < -STICK_RANGE {
            if self.cursor_slot < 5 && !self.is_locked(slot_gkind(self.cursor_slot + 5)) {
                self.cursor_slot += 5;
            }
            self.scroll_wait = if hold & DOWN != 0 {
                BUTTON_SCROLL_WAIT
            } else {
                (stick(y) + 160) / 7
            };
            return None;
        }
        if hold & LEFT != 0 || x < -STICK_RANGE {
            self.cursor_slot = match self.cursor_slot {
                0 if self.is_locked(slot_gkind(4)) => 3,
                0 => 4,
                5 => 9,
                s => s - 1,
            };
            self.scroll_wait = if hold & LEFT != 0 {
                BUTTON_SCROLL_WAIT
            } else {
                (stick(x) + 160) / 7
            };
            return None;
        }
        if hold & RIGHT != 0 || x > STICK_RANGE {
            self.cursor_slot = match self.cursor_slot {
                3 if self.is_locked(slot_gkind(4)) => 0,
                3 => 4,
                4 => 0,
                9 => 5,
                s => s + 1,
            };
            self.scroll_wait = if hold & RIGHT != 0 {
                BUTTON_SCROLL_WAIT
            } else {
                (160 - stick(x)) / 7
            };
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pad(buttons: u16, stick_x: i8, stick_y: i8) -> ControllerState {
        ControllerState {
            buttons: N64Buttons(buttons),
            stick_x,
            stick_y,
            connected: true,
        }
    }

    fn settle(s: &mut StageSelect) {
        for _ in 0..INPUT_START_TICS {
            s.tick(pad(0, 0, 0), N64Buttons(0));
        }
    }

    #[test]
    fn input_waits_ten_ticks_and_the_cursor_starts_on_the_remembered_stage() {
        let mut s = StageSelect::new(gkind::PUPUPU, 0);
        assert_eq!(s.cursor_slot, 6);
        for _ in 0..INPUT_START_TICS - 1 {
            assert_eq!(s.tick(pad(0, 0, 0), N64Buttons(N64Buttons::A)), None);
        }
        assert_eq!(
            s.tick(pad(0, 0, 0), N64Buttons(N64Buttons::A)),
            Some(Outcome::Confirm {
                gkind: gkind::PUPUPU
            })
        );
    }

    #[test]
    fn locked_mushroom_kingdom_is_skipped_by_every_move() {
        let mut s = StageSelect::new(gkind::ZEBES, 0);
        settle(&mut s);
        // Right from Zebes (slot 3) wraps to Castle past the locked slot 4.
        s.tick(pad(N64Buttons::D_RIGHT, 0, 0), N64Buttons(0));
        assert_eq!(s.cursor_slot, 0);
        // Left from Castle lands on Zebes.
        s.tick(pad(0, 0, 0), N64Buttons(0));
        s.tick(pad(N64Buttons::D_LEFT, 0, 0), N64Buttons(0));
        assert_eq!(s.cursor_slot, 3);
        // Up from Random (slot 9) would reach slot 4: refused.
        let mut s = StageSelect::new(gkind::RANDOM, 0);
        settle(&mut s);
        s.tick(pad(N64Buttons::D_UP, 0, 0), N64Buttons(0));
        assert_eq!(s.cursor_slot, 9);
        // Unlocked, the same move goes through.
        let mut s = StageSelect::new(gkind::RANDOM, UNLOCK_MASK_INISHIE);
        settle(&mut s);
        s.tick(pad(N64Buttons::D_UP, 0, 0), N64Buttons(0));
        assert_eq!(s.cursor_slot, 4);
    }

    #[test]
    fn a_held_button_repeats_every_twelve_ticks_and_the_stick_by_its_deflection() {
        let mut s = StageSelect::new(gkind::CASTLE, 0);
        settle(&mut s);
        let mut moves = 0;
        let mut last = s.cursor_slot;
        for _ in 0..25 {
            s.tick(pad(N64Buttons::C_RIGHT, 0, 0), N64Buttons(0));
            if s.cursor_slot != last {
                moves += 1;
                last = s.cursor_slot;
            }
        }
        // Moves on ticks 0, 12 and 24 of the hold.
        assert_eq!(moves, 3);
        // A full stick (80) waits (160 - 80) / 7 = 11 ticks.
        let mut s = StageSelect::new(gkind::CASTLE, 0);
        settle(&mut s);
        s.tick(pad(0, 80, 0), N64Buttons(0));
        assert_eq!(s.cursor_slot, 1);
        assert_eq!(s.scroll_wait, 11);
        // Releasing to neutral clears the wait at once.
        s.tick(pad(0, 0, 0), N64Buttons(0));
        assert_eq!(s.scroll_wait, 0);
    }

    #[test]
    fn row_ends_wrap_as_the_source_switch_does() {
        let mut s = StageSelect::new(gkind::YOSTER, 0);
        settle(&mut s);
        s.tick(pad(N64Buttons::D_LEFT, 0, 0), N64Buttons(0));
        assert_eq!(s.cursor_slot, RANDOM_SLOT);
        s.tick(pad(0, 0, 0), N64Buttons(0));
        s.tick(pad(N64Buttons::D_RIGHT, 0, 0), N64Buttons(0));
        assert_eq!(s.cursor_slot, 5);
        s.tick(pad(0, 0, 0), N64Buttons(0));
        s.tick(pad(N64Buttons::D_UP, 0, 0), N64Buttons(0));
        assert_eq!(slot_gkind(s.cursor_slot), gkind::CASTLE);
    }

    #[test]
    fn random_redraws_locked_and_repeat_stages_and_remembers_the_random_slot() {
        let s = StageSelect::new(gkind::RANDOM, 0);
        let mut draws = [gkind::INISHIE, gkind::PUPUPU, gkind::HYRULE].into_iter();
        let saved = s.save(gkind::PUPUPU, || draws.next().unwrap());
        assert_eq!(
            saved,
            Saved {
                gkind: gkind::HYRULE,
                remembered: gkind::RANDOM
            }
        );
        let s = StageSelect::new(gkind::SECTOR, 0);
        assert_eq!(
            s.save(gkind::SECTOR, || unreachable!()).gkind,
            gkind::SECTOR
        );
    }

    #[test]
    fn five_idle_minutes_return_to_the_title_and_input_restarts_the_clock() {
        let mut s = StageSelect::new(gkind::CASTLE, 0);
        let mut outcome = None;
        for _ in 0..RETURN_TICS {
            outcome = s.tick(pad(0, 0, 0), N64Buttons(0));
        }
        assert_eq!(outcome, Some(Outcome::Timeout));
        let mut s = StageSelect::new(gkind::CASTLE, 0);
        for i in 0..RETURN_TICS {
            let hold = if i == 100 { N64Buttons::Z } else { 0 };
            assert_eq!(s.tick(pad(hold, 0, 0), N64Buttons(0)), None);
        }
    }
}
