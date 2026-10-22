//! Census of the material-animation commands the pack's scripts reach
//! (RE-010: `gcParseMObjMatAnimJoint`'s `ANIM_CMD_22`).
//!
//! Every packed [`ssb_rom::pack::MatAnimDesc`] script (the pipeline's
//! walk of every `MObjSub` in every packed model), two ways:
//! - walked statically from its start through every jump, decoding each
//!   command's word count as `gcParseMObjMatAnimJoint` advances it;
//! - run through [`MaterialJoint`] for [`TICKS`] ticks or to its end.

use std::collections::{BTreeMap, HashSet};

use ssb_rom::matanim::MaterialJoint;
use ssb_rom::pack::Pack;

const TICKS: usize = 20_000;

fn open_pack() -> Option<Vec<u8>> {
    ssb_tablegen::test_rom(env!("CARGO_MANIFEST_DIR"))?;
    std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/generated/ssb64.pak"),
    )
    .ok()
}

fn word(file: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes(file.get(at..at + 4)?.try_into().ok()?))
}

/// Counts each opcode reachable from `script`; `Err` names a command
/// `gcParseMObjMatAnimJoint` has no case for (it would spin there).
fn walk(file: &[u8], script: usize, census: &mut BTreeMap<u32, usize>) -> Result<(), String> {
    let mut seen = HashSet::new();
    let mut pc = script;
    while seen.insert(pc) {
        let w = word(file, pc).ok_or(format!("truncated at {pc:#x}"))?;
        let (opcode, flags) = (w >> 25, (w >> 15) & 0x3FF);
        *census.entry(opcode).or_default() += 1;
        pc += 4;
        // Value words per set flag bit, over the tracks the case loops.
        let per_bit =
            |per: usize, tracks: u32| per * (flags & ((1 << tracks) - 1)).count_ones() as usize;
        pc += 4 * match opcode {
            // End.
            0 => return Ok(()),
            // Jump, SetAnim: the target word.
            1 | 14 => {
                pc = word(file, pc).ok_or(format!("truncated at {pc:#x}"))? as usize;
                0
            }
            // Wait, ANIM_CMD_12 (lengths only).
            2 | 12 => 0,
            3 | 4 | 7..=11 => per_bit(1, 10),
            5 | 6 => per_bit(2, 10),
            18..=21 => per_bit(1, 5),
            // `unk4C/6C/68/74/70`, one word each for flags 0x01..0x10.
            22 => per_bit(1, 5),
            _ => return Err(format!("opcode {opcode} at {:#x}", pc - 4)),
        };
    }
    Ok(())
}

/// Every reachable command is one `gcParseMObjMatAnimJoint` handles, and
/// none is `ANIM_CMD_22`: the RE-010 fields it fills (never read) matter
/// to no packed script.
#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn no_packed_material_script_uses_command_22() {
    let Some(bytes) = open_pack() else { return };
    let p = Pack::open(&bytes).unwrap();
    assert!(p.mat_anim_count() > 0);
    let mut census = BTreeMap::new();
    let mut bad = Vec::new();
    for i in 0..p.mat_anim_count() {
        let a = p.mat_anim(i).unwrap();
        let file = p.mat_anim_file(&a).unwrap();
        if let Err(e) = walk(file, a.script as usize, &mut census) {
            bad.push(format!(
                "mat_anim {i} (file {} script {:#x}): {e}",
                a.source_file, a.script
            ));
        }
    }
    eprintln!("{} scripts, opcode census {census:?}", p.mat_anim_count());
    assert!(bad.is_empty(), "{}", bad.join("\n"));
    assert_eq!(census.get(&22), None, "{census:?}");
}

#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn every_packed_material_script_runs_without_error() {
    let Some(bytes) = open_pack() else { return };
    let p = Pack::open(&bytes).unwrap();
    let mut bad = Vec::new();
    for i in 0..p.mat_anim_count() {
        let a = p.mat_anim(i).unwrap();
        let file = p.mat_anim_file(&a).unwrap();
        let mut j = MaterialJoint::start(a.script, 0.0);
        for _ in 0..TICKS {
            if let Err(e) = j.tick(file, 1.0) {
                bad.push(format!("mat_anim {i} (file {}): {e:?}", a.source_file));
                break;
            }
            if j.ended() {
                break;
            }
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}
