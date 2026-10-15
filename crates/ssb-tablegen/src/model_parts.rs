//! Each fighter's joint masks, for `ssb_game::modelpart`: `FTAttributes`'
//! `setup_parts` and `animlock` flag pairs, and which of its high-detail
//! `DObjDesc`s carry a display list (RE-425, RE-509).
//!
//! `ssb-rom`'s `fighter::setup_parts`, `fighter::animlock` and
//! `fighter::common_parts` decode the same fields;
//! `crates/ssb-rom/tests/model_parts.rs` checks the generated tables
//! against them.

use std::fmt::Write;

use crate::archive::File;
use crate::fighters::FIGHTERS;
use crate::source::be_u32;
use crate::{emit, Error, Result, Source};

/// `FTAttributes::setup_parts` (`u32 *`, two flag words).
pub const SETUP_PARTS_OFFSET: u32 = 0x29C;
/// `FTAttributes::animlock` (`u32 *`, two flag words).
pub const ANIMLOCK_OFFSET: u32 = 0x2A0;
/// `FTAttributes::commonparts_container` (`FTCommonPartContainer *`);
/// `commonparts[0].dobjdesc` is its first word, an extern pointer into the
/// fighter's model file.
pub const COMMONPARTS_OFFSET: u32 = 0x2D4;
/// `sizeof(DObjDesc)`: `id`, `dl`, then translate, rotate and scale.
pub const DOBJDESC_SIZE: u32 = 44;
/// `DOBJ_ARRAY_MAX`: the `id` that ends a `DObjDesc` array.
pub const DOBJ_ARRAY_MAX: u32 = 18;

fn missing(file: &File, at: u32) -> Error {
    Error::BadRelocation {
        file: file.id,
        offset: at as usize,
    }
}

/// The intern pointer at `at`, as a byte offset into `file`.
fn intern(file: &File, at: u32) -> Result<u32> {
    file.intern_relocs
        .iter()
        .find(|r| r.at == at)
        .map(|r| r.target)
        .ok_or_else(|| missing(file, at))
}

/// A `u32 *flags` pair as one mask, bit `n` for descriptor `n`: the first
/// word covers 0..31 and the second 32..63, each most-significant bit first
/// (`lbCommonSetupFighterPartsDObjs`, `ftParamSetAnimLocks`).
fn flag_pair(file: &File, at: u32) -> Result<u64> {
    let target = intern(file, at)? as usize;
    let (lo, hi) = (be_u32(&file.data, target)?, be_u32(&file.data, target + 4)?);
    let bit = |w: u32, i: u32| u64::from(w >> (31 - i) & 1);
    Ok((0..32).fold(0u64, |m, i| m | bit(lo, i) << i | bit(hi, i) << (i + 32)))
}

/// Which descriptors of `commonparts[0].dobjdesc` have a display list (a
/// non-null `dl` the archive relocated), up to the `DOBJ_ARRAY_MAX` entry.
fn dl_mask(rom: &Source, main: &File, attributes: u32) -> Result<u64> {
    let container = intern(main, attributes + COMMONPARTS_OFFSET)?;
    let link = main
        .extern_relocs
        .iter()
        .find(|r| r.at == container)
        .ok_or_else(|| missing(main, container))?;
    let model = rom.file(u32::from(link.target_file))?;
    let mut mask = 0u64;
    for n in 0..64u32 {
        let at = link.target_offset + n * DOBJDESC_SIZE;
        if be_u32(&model.data, at as usize)? == DOBJ_ARRAY_MAX {
            return Ok(mask);
        }
        let dl = be_u32(&model.data, at as usize + 4)?;
        if dl != 0 && model.intern_relocs.iter().any(|r| r.at == at + 4) {
            mask |= 1 << n;
        }
    }
    Err(missing(&model, link.target_offset))
}

pub fn generate(rom: Option<&Source>) -> Result<String> {
    let mut joints = Vec::with_capacity(FIGHTERS.len());
    let mut locks = Vec::with_capacity(FIGHTERS.len());
    for f in FIGHTERS.iter() {
        let (setup, dl, lock) = match rom {
            Some(rom) => {
                let main = rom.file(f.main_file)?;
                let setup = flag_pair(&main, f.attributes + SETUP_PARTS_OFFSET)?;
                let dl = dl_mask(rom, &main, f.attributes)?;
                let lock = flag_pair(&main, f.attributes + ANIMLOCK_OFFSET)?;
                (setup, dl & setup, lock)
            }
            None => (0, 0, 0),
        };
        joints.push(format!("    (0x{setup:x}, 0x{dl:x}), // {}", f.name));
        locks.push(format!("    0x{lock:x}, // {}", f.name));
    }
    let mut out = emit::header("each fighter's setup_parts, display-list and animlock joint masks");
    out.push_str(
        "/// Per `FTKind`: `(setup_parts, setup_parts & has a display list)`, bit\n\
         /// `n` for descriptor `n`.\n",
    );
    let _ = writeln!(
        out,
        "#[rustfmt::skip]\npub const JOINT_MASKS: [(u64, u64); {}] = [\n{}\n];\n",
        joints.len(),
        joints.join("\n")
    );
    out.push_str("/// Per `FTKind`: `FTAttributes::animlock`, bit `n` for descriptor `n`.\n");
    let _ = writeln!(
        out,
        "#[rustfmt::skip]\npub const ANIMLOCK_MASKS: [u64; {}] = [\n{}\n];",
        locks.len(),
        locks.join("\n")
    );
    Ok(out)
}
