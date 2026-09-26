//! Costume selection — `dFTParamCostumeIDs` and `ftParamGetCostumeCommonID`
//! (`ft/ftparam.c`), and the Training character select's C-button costume
//! picks (`mnPlayers1PTrainingUpdateCostume`,
//! `mnPlayers1PTrainingCheckCostumeUsed` and
//! `mnPlayers1PTrainingGetFreeCostumeRoyal`, `mn/mnplayers/mnplayers1ptraining.c`).

use ssb_engine::input::N64Buttons;

use crate::fighter::FighterKind;

/// `FTCostume::royal` from `dFTParamCostumeIDs`, indexed by the
/// `nFTKind` order: the costume each select button (C-Up, C-Right, C-Down,
/// C-Left) picks in free-for-all. Kinds past Ness have one costume.
const ROYAL: [[u8; 4]; 12] = [
    [0, 1, 2, 3], // Mario
    [0, 1, 2, 3], // Fox
    [0, 1, 2, 3], // Donkey Kong
    [0, 1, 2, 3], // Samus
    [0, 1, 2, 3], // Luigi
    [0, 2, 3, 1], // Link
    [0, 1, 2, 3], // Yoshi
    [0, 4, 1, 3], // Captain Falcon
    [0, 1, 2, 3], // Kirby
    [0, 1, 2, 3], // Pikachu
    [0, 1, 2, 3], // Jigglypuff
    [0, 1, 2, 3], // Ness
];

/// `ftParamGetCostumeCommonID(fkind, color)`.
pub fn costume_common_id(kind: FighterKind, color: usize) -> u8 {
    ROYAL
        .get(kind as usize)
        .map(|row| row[color.min(3)])
        .unwrap_or(0)
}

/// The select-button index a C-button tap stands for on the character
/// select: C-Up 0, C-Right 1, C-Down 2, C-Left 3, tested in that order.
pub fn select_button(taps: N64Buttons) -> Option<usize> {
    [
        N64Buttons::C_UP,
        N64Buttons::C_RIGHT,
        N64Buttons::C_DOWN,
        N64Buttons::C_LEFT,
    ]
    .iter()
    .position(|&b| taps.contains(b))
}

/// One Training slot as the costume rules see it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Slot {
    pub kind: FighterKind,
    pub costume: u8,
}

/// `mnPlayers1PTrainingCheckCostumeUsed`: the other slot already wears this
/// costume of the same fighter.
pub fn costume_used(kind: FighterKind, costume: u8, other: Slot) -> bool {
    kind == other.kind && costume == other.costume
}

/// `mnPlayers1PTrainingUpdateCostume`: the costume a C-button pick gives, or
/// `None` when it is taken (the source plays `nSYAudioFGMMenuDenied`).
pub fn pick(kind: FighterKind, button: usize, other: Slot) -> Option<u8> {
    let costume = costume_common_id(kind, button);
    (!costume_used(kind, costume, other)).then_some(costume)
}

/// `mnPlayers1PTrainingGetFreeCostume`: the first royal costume the other
/// slot is not wearing.
pub fn free_costume(kind: FighterKind, other: Slot) -> u8 {
    let royal = (0..4)
        .find(|&i| !(kind == other.kind && costume_common_id(kind, i) == other.costume))
        .unwrap_or(0);
    costume_common_id(kind, royal)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn c_buttons_pick_the_royal_row() {
        assert_eq!(select_button(N64Buttons(N64Buttons::C_DOWN)), Some(2));
        assert_eq!(select_button(N64Buttons(N64Buttons::A)), None);
        assert_eq!(costume_common_id(FighterKind::Mario, 2), 2);
        assert_eq!(costume_common_id(FighterKind::Link, 1), 2);
        assert_eq!(costume_common_id(FighterKind::Captain, 1), 4);
    }

    #[test]
    fn a_costume_the_other_slot_wears_is_denied() {
        let dummy = Slot {
            kind: FighterKind::Mario,
            costume: 1,
        };
        assert_eq!(pick(FighterKind::Mario, 1, dummy), None);
        assert_eq!(pick(FighterKind::Mario, 2, dummy), Some(2));
        assert_eq!(pick(FighterKind::Fox, 1, dummy), Some(1));
        assert_eq!(free_costume(FighterKind::Mario, dummy), 0);
        let player = Slot {
            kind: FighterKind::Mario,
            costume: 0,
        };
        assert_eq!(free_costume(FighterKind::Mario, player), 1);
    }
}
