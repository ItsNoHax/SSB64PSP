//! The CPU's input scripts and attack tables (`ft/ftcomputer.c`), for
//! `ssb_game::computer::scripts`.

use std::fmt::Write;

use crate::emit::{self, f32_lit};
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

struct Attack {
    input: i32,
    start: i32,
    end: i32,
    box_: [f32; 4],
}

fn attack(rom: &Source, at: u32) -> Result<Attack> {
    Ok(Attack {
        input: rom.u32(at)? as i32,
        start: rom.u32(at + 4)? as i32,
        end: rom.u32(at + 8)? as i32,
        box_: [
            rom.f32(at + 12)?,
            rom.f32(at + 16)?,
            rom.f32(at + 20)?,
            rom.f32(at + 24)?,
        ],
    })
}

fn emit_rows(out: &mut String, label: &str, rows: &[Attack]) {
    let _ = writeln!(
        out,
        "#[rustfmt::skip]\npub static {label}: [Attack; {}] = [",
        rows.len()
    );
    for r in rows {
        let _ = writeln!(
            out,
            "    Attack {{ input: {}, hit_start_frame: {}, hit_end_frame: {}, detect_near_x: {}, \
             detect_far_x: {}, detect_near_y: {}, detect_far_y: {} }},",
            r.input,
            r.start,
            r.end,
            f32_lit(r.box_[0]),
            f32_lit(r.box_[1]),
            f32_lit(r.box_[2]),
            f32_lit(r.box_[3]),
        );
    }
    out.push_str("];\n\n");
}

fn screaming(name: &str) -> String {
    name.to_uppercase()
}

pub fn generate(rom: Option<&Source>) -> Result<String> {
    let mut w = emit::header("the CPU's input scripts and attack tables (ft/ftcomputer.c)");
    // The scripts in pointer-table order; each is named by its place among
    // the scripts in memory, which is the decompilation's numbering.
    let (table, count) = INPUT_SCRIPT_TABLE;
    let mut scripts: Vec<(usize, Vec<u8>)> = Vec::new();
    if let Some(rom) = rom {
        let ptrs: Vec<u32> = (0..count as u32)
            .map(|i| rom.u32(table + i * 4))
            .collect::<Result<_>>()?;
        let mut sorted = ptrs.clone();
        sorted.sort_unstable();
        for p in &ptrs {
            let at = OVL3.rom_of(*p).expect("input script outside ovl3");
            let number = sorted.iter().position(|q| q == p).expect("sorted");
            scripts.push((number, input_script(rom, at)?));
        }
    } else {
        scripts = (0..count).map(|i| (i, Vec::new())).collect();
    }
    w.push_str("/// `dFTComputerPlayerInputScripts`, in pointer-table order (which swaps\n");
    w.push_str("/// scripts 24 and 25).\n#[rustfmt::skip]\n");
    let _ = writeln!(w, "pub static INPUT_SCRIPTS: [&[u8]; {count}] = [");
    for (n, bytes) in &scripts {
        let cells: Vec<String> = bytes.iter().map(|b| format!("0x{b:02X}")).collect();
        let _ = writeln!(w, "    &[{}], // Script{n}", cells.join(", "));
    }
    w.push_str("];\n\n");
    w.push_str("/// `nFTComputerInput*`: [`INPUT_SCRIPTS`] indices by name.\npub mod input {\n");
    for (i, name) in INPUT_KINDS.iter().enumerate() {
        let _ = writeln!(w, "    pub const {name}: usize = {i};");
    }
    w.push_str("}\n\n");

    w.push_str(
        "/// One `FTComputerAttack` row: the input script, the hitbox's active\n\
         /// frames and the box around the fighter where it connects.\n\
         #[derive(Debug, Clone, Copy, PartialEq)]\n\
         pub struct Attack {\n    pub input: usize,\n    pub hit_start_frame: i32,\n\
         \x20   pub hit_end_frame: i32,\n    pub detect_near_x: f32,\n    pub detect_far_x: f32,\n\
         \x20   pub detect_near_y: f32,\n    pub detect_far_y: f32,\n}\n\n",
    );
    // Each table holds the grounded attacks, an `input_kind == -1` row, then
    // the aerial ones and another -1 row. The tables are emitted in memory
    // order (the decompilation's source order).
    let mut tables: Vec<(u32, &str, Vec<Attack>, Vec<Attack>)> = Vec::new();
    for (i, name) in FIGHTERS.iter().enumerate() {
        let (mut ground, mut air, mut addr) = (Vec::new(), Vec::new(), i as u32);
        if let Some(rom) = rom {
            let ptr = rom.u32(ATTACK_LIST + 4 * i as u32)?;
            addr = ptr;
            let mut at = OVL3.rom_of(ptr).expect("attack table outside ovl3");
            for rows in [&mut ground, &mut air] {
                loop {
                    let row = attack(rom, at)?;
                    at += ATTACK_SIZE;
                    if row.input == -1 {
                        break;
                    }
                    assert!(rows.len() < 64, "{name}: attack table has no separator");
                    rows.push(row);
                }
            }
        }
        tables.push((addr, name, ground, air));
    }
    let mut by_address: Vec<&(u32, &str, Vec<Attack>, Vec<Attack>)> = tables.iter().collect();
    by_address.sort_by_key(|t| t.0);
    for (_, name, ground, air) in by_address {
        let up = screaming(name);
        let _ = writeln!(w, "/// `dFTComputerAttacks{name}`, grounded rows.");
        emit_rows(&mut w, &format!("ATTACKS_{up}_GROUND"), ground);
        let _ = writeln!(w, "/// `dFTComputerAttacks{name}`, aerial rows.");
        emit_rows(&mut w, &format!("ATTACKS_{up}_AIR"), air);
    }
    w.push_str(
        "/// `dFTComputerAttackList` for the twelve playable fighters, in `nFTKind`\n\
         /// order: the grounded and aerial rows.\n#[rustfmt::skip]\n\
         pub static ATTACKS: [(&[Attack], &[Attack]); 12] = [\n",
    );
    for name in FIGHTERS {
        let up = screaming(name);
        let _ = writeln!(w, "    (&ATTACKS_{up}_GROUND, &ATTACKS_{up}_AIR),");
    }
    w.push_str("];\n");
    Ok(w)
}
