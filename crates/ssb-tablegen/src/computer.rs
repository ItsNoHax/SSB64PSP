//! The CPU's input scripts and attack tables (`ft/ftcomputer.c`), for
//! `ssb_game::computer::scripts`.

use alloc::vec::Vec;

use crate::source::OVL3;
use crate::{Result, Source};

/// `dFTComputerPlayerInputScripts`: ROM offset and count of its `u8 *`s.
pub const INPUT_SCRIPT_TABLE: (u32, usize) = (0x10_2D80, 49);

/// `dFTComputerAttackList`: ROM offset; its first twelve entries are the
/// playable fighters' tables.
pub const ATTACK_LIST: u32 = 0x10_2B04;

/// The playable fighters, in `nFTKind` order.
pub const FIGHTERS: [&str; 12] = [
    "Mario", "Fox", "Donkey", "Samus", "Luigi", "Link", "Yoshi", "Captain", "Kirby", "Pikachu",
    "Purin", "Ness",
];

/// `FTComputerInputKind`'s names, in enum order, as constants.
#[cfg(feature = "std")]
pub const INPUT_KINDS: [&str; 49] = [
    "STICK_N",
    "MOVE_AUTO",
    "STICK_TILT_AUTO_X",
    "STICK_N_MOVE_AUTO",
    "MOVE_AUTO_STICK_TILT_HI_RELEASE_Z",
    "STICK_TILT_AUTO_X_BUTTON_A",
    "STICK_TILT_AUTO_X_D5_N_BUTTON_A",
    "MOVE_AUTO_BUTTON_A",
    "STICK_SMASH_AUTO_X_BUTTON_A",
    "STICK_SMASH_AUTO_X_BUTTON_B",
    "STICK_TILT_AUTO_X_N_Y_D5_SMASH_AUTO_X_BUTTON_B",
    "STICK_TILT_AUTO_X_N_Y_D1_BUTTON_B",
    "BUTTON_Z1",
    "STICK_SMASH_HI_BUTTON_B",
    "STICK_TILT_AUTO_X_D5_SMASH_S_BUTTON_B",
    "STICK_N_BUTTON_L",
    "BUTTON_Z2",
    "STICK_TILT_AUTO_X_D5",
    "STICK_TILT_AUTO_X_D1",
    "STICK_N_BUTTON_A",
    "STICK_TILT_AUTO_X_D5_BUTTON_A",
    "STICK_SMASH_AUTO_X_N_Y_BUTTON_A",
    "STICK_TILT_AUTO_X_D1_SMASH_S_BUTTON_A",
    "STICK_TILT_HI_BUTTON_A",
    "STICK_TILT_AUTO_X_D5_TILT_AUTO_Y_BUTTON_A",
    "STICK_SMASH_HI_BUTTON_A",
    "STICK_TILT_AUTO_X_D5_SMASH_AUTO_Y_BUTTON_A",
    "STICK_SMASH_LW_BUTTON_B",
    "STICK_N_BUTTON_Z_BUTTON_A",
    "STICK_TILT_AUTO_X_D5_BUTTON_Z_BUTTON_A",
    "STICK_SMASH_L",
    "STICK_SMASH_R",
    "STICK_TILT_LW_BUTTON_A",
    "STICK_TILT_AUTO_X_D5_TILT_LW_BUTTON_A",
    "STICK_SMASH_LW_BUTTON_A",
    "STICK_N_BUTTON_Z_HOLD",
    "BUTTON_Z_RELEASE",
    "STICK_N_X_SMASH_LW_BUTTON_B_RELEASE_B_HOLD",
    "STICK_N_BUTTON_B_RELEASE",
    "STICK_N_D1_MOVE_AUTO_SMASH_LW",
    "STICK_N_BUTTON_B_Z_RELEASE_A_PRESS",
    "STICK_TILT_AUTO_X_BUTTON_B_Z_RELEASE_A_PRESS",
    "THROW_ITEM_IMMEDIATE",
    "THROW_ITEM_WAIT",
    "WIGGLE",
    "ESCAPE_L",
    "ESCAPE_R",
    "YOSHI_SPECIAL_HI_AIM",
    "NESS_SPECIAL_HI_AIM",
];

/// `FTCOMPUTER_COMMAND_END`.
const END: u8 = 0xFF;
/// `nFTComputerCommandStickX` and `StickY`: the opcodes followed by one
/// value byte.
const STICK_X: u8 = 10;
const STICK_Y: u8 = 11;

/// Bytes per `FTComputerAttack`: `input_kind`, two frames, four floats.
const ATTACK_SIZE: u32 = 28;

/// One input script: its events up to and including `END`.
fn input_script(rom: &Source, at: u32) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    let mut i = at;
    loop {
        let b = rom.u8(i)?;
        out.push(b);
        i += 1;
        if b == END {
            return Ok(out);
        }
        if matches!(b >> 4, STICK_X | STICK_Y) {
            out.push(rom.u8(i)?);
            i += 1;
        }
        assert!(out.len() < 64, "input script at 0x{at:X} has no end");
    }
}

/// Bytes in all input scripts, each up to and including its `END`.
pub const INPUT_SCRIPT_BYTES: usize = 429;
/// Most grounded and aerial rows any attack table holds.
pub const GROUND_ATTACKS_MAX: usize = 11;
pub const AIR_ATTACKS_MAX: usize = 7;
/// Words per `ComputerAttacks` row: `input_kind`, the two frames and the
/// four floats' bits, as in the ROM.
pub const ATTACK_WORDS: usize = 7;
/// Words per fighter in the `ComputerAttacks` table: the grounded and
/// aerial row counts, then [`GROUND_ATTACKS_MAX`] grounded and
/// [`AIR_ATTACKS_MAX`] aerial rows (unused ones zero).
pub const FIGHTER_ATTACK_WORDS: usize = 2 + (GROUND_ATTACKS_MAX + AIR_ATTACKS_MAX) * ATTACK_WORDS;

/// Every input script, in pointer-table order (which swaps scripts 24 and
/// 25 of the memory order).
fn input_scripts(rom: &Source) -> Result<Vec<Vec<u8>>> {
    let (table, count) = INPUT_SCRIPT_TABLE;
    let scripts = (0..count as u32)
        .map(|i| {
            let p = rom.u32(table + i * 4)?;
            input_script(rom, OVL3.rom_of(p).expect("input script outside ovl3"))
        })
        .collect::<Result<Vec<_>>>()?;
    assert_eq!(
        scripts.iter().map(Vec::len).sum::<usize>(),
        INPUT_SCRIPT_BYTES
    );
    Ok(scripts)
}

/// The `InputScripts` table: every script's bytes back to back.
pub fn input_scripts_bytes(c: &crate::tables::Ctx) -> Result<Vec<u8>> {
    Ok(input_scripts(c.rom)?.concat())
}

/// The `InputScriptOffsets` table: each script's first byte in
/// `InputScripts`, then [`INPUT_SCRIPT_BYTES`].
pub fn input_script_offsets_bytes(c: &crate::tables::Ctx) -> Result<Vec<u8>> {
    let mut offsets = Vec::with_capacity(INPUT_SCRIPT_TABLE.1 + 1);
    let mut at = 0u16;
    for script in input_scripts(c.rom)? {
        offsets.push(at);
        at += script.len() as u16;
    }
    offsets.push(at);
    Ok(crate::tables::le_bytes(&offsets, u16::to_le_bytes))
}

/// The `ComputerAttacks` table: `dFTComputerAttackList`'s first twelve
/// tables, in `nFTKind` order. Each holds the grounded rows, an
/// `input_kind == -1` row, then the aerial ones and another -1 row.
pub fn attacks_bytes(c: &crate::tables::Ctx) -> Result<Vec<u8>> {
    let mut words = Vec::with_capacity(FIGHTERS.len() * FIGHTER_ATTACK_WORDS);
    for (i, name) in FIGHTERS.iter().enumerate() {
        let ptr = c.rom.u32(ATTACK_LIST + 4 * i as u32)?;
        let mut at = OVL3.rom_of(ptr).expect("attack table outside ovl3");
        let mut halves = [Vec::new(), Vec::new()];
        for rows in &mut halves {
            loop {
                let row: Vec<u32> = (0..ATTACK_WORDS as u32)
                    .map(|w| c.rom.u32(at + 4 * w))
                    .collect::<Result<_>>()?;
                at += ATTACK_SIZE;
                if row[0] as i32 == -1 {
                    break;
                }
                assert!(rows.len() < 64, "{name}: attack table has no separator");
                rows.push(row);
            }
        }
        let [ground, air] = halves;
        assert!(ground.len() <= GROUND_ATTACKS_MAX && air.len() <= AIR_ATTACKS_MAX);
        words.extend([ground.len() as u32, air.len() as u32]);
        for (rows, max) in [(ground, GROUND_ATTACKS_MAX), (air, AIR_ATTACKS_MAX)] {
            let n = rows.len();
            words.extend(rows.into_iter().flatten());
            words.extend(core::iter::repeat_n(0, (max - n) * ATTACK_WORDS));
        }
    }
    Ok(crate::tables::le_words(&words))
}

/// `ssb_game::computer::scripts`' layout: the `nFTComputerInput*` names.
#[cfg(feature = "std")]
pub fn layout_rs() -> String {
    use std::fmt::Write;
    let mut w = crate::emit::header("the CPU's input script names (layout only)");
    w.push_str("/// `nFTComputerInput*`: input script indices by name.\npub mod input {\n");
    for (i, name) in INPUT_KINDS.iter().enumerate() {
        let _ = writeln!(w, "    pub const {name}: usize = {i};");
    }
    w.push_str("}\n");
    w
}
