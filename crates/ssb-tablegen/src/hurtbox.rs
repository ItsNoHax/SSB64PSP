//! Each fighter's hurtbox descriptors, `FTAttributes::damage_coll_descs`,
//! for `ssb_game::hurtbox`.
//!
//! The array is inline in the struct: eleven 36-byte `FTDamageCollDesc {
//! s32 joint_id; s32 placement; sb32 is_grabbable; Vec3f offset; Vec3f
//! size; }` (`fttypes.h`) ending where `hit_detect_range` begins.
//! `ftParamResetFighterDamageCollsAll` skips a row whose `joint_id` is -1;
//! every fighter's used rows come first, so the generated table holds the
//! rows before the first -1, and the generator fails if a used row follows
//! one. The emitted rows call `ssb_game::hurtbox::hurt`, which halves
//! `size` as `ftParamResetFighterDamageCollsAll` does.

use std::fmt::Write;

use crate::fighters::FIGHTERS;
use crate::source::be_u32;
use crate::{emit, Error, Result, Source};

/// `FTAttributes::damage_coll_descs[11]`: `hit_detect_range` (`0x290`, the
/// `Vec3f` before `setup_parts` at `0x29C`) minus eleven rows.
pub const DAMAGE_COLL_DESCS_OFFSET: u32 = 0x104;
/// `sizeof(FTDamageCollDesc)`.
pub const DAMAGE_COLL_DESC_SIZE: u32 = 36;
/// `ARRAY_COUNT(FTAttributes::damage_coll_descs)`.
pub const DAMAGE_COLL_DESCS: u32 = 11;
/// `FTAttributes::hit_detect_range`, the field after the array.
pub const HIT_DETECT_RANGE_OFFSET: u32 = 0x290;
const _: () = assert!(
    DAMAGE_COLL_DESCS_OFFSET + DAMAGE_COLL_DESCS * DAMAGE_COLL_DESC_SIZE == HIT_DETECT_RANGE_OFFSET
);
/// The `joint_id` of an unused row.
const NO_JOINT: u32 = u32::MAX;

/// One row: joint, placement, grabbable, offset, full size.
type Row = (u8, u8, bool, [f32; 3], [f32; 3]);

fn rows(data: &[u8], attributes: u32, file: u32) -> Result<Vec<Row>> {
    let mut out = Vec::new();
    let mut ended = false;
    for i in 0..DAMAGE_COLL_DESCS {
        let at = (attributes + DAMAGE_COLL_DESCS_OFFSET + i * DAMAGE_COLL_DESC_SIZE) as usize;
        let word = |n: usize| be_u32(data, at + 4 * n);
        let joint = word(0)?;
        if joint == NO_JOINT {
            ended = true;
            continue;
        }
        let (placement, grabbable) = (word(1)?, word(2)?);
        // A used row after an unused one, or a field out of its range,
        // means the offset or the row rule is wrong.
        if ended || joint >= 64 || placement > 2 || grabbable > 1 {
            return Err(Error::BadRelocation { file, offset: at });
        }
        let f = |n: usize| -> Result<f32> { Ok(f32::from_bits(word(n)?)) };
        out.push((
            joint as u8,
            placement as u8,
            grabbable != 0,
            [f(3)?, f(4)?, f(5)?],
            [f(6)?, f(7)?, f(8)?],
        ));
    }
    Ok(out)
}

fn vec(v: [f32; 3]) -> String {
    let cells: Vec<String> = v.iter().map(|&x| emit::f32_lit(x)).collect();
    format!("[{}]", cells.join(", "))
}

pub fn generate(rom: Option<&Source>) -> Result<String> {
    let mut out =
        emit::header("each fighter's hurtbox descriptors (`FTAttributes::damage_coll_descs`)");
    out.push_str(
        "/// Per `FTKind`: `FTAttributes::damage_coll_descs` up to the first unused\n\
         /// (`joint_id` -1) row, through `hurt` (`size` halved).\n",
    );
    let _ = writeln!(
        out,
        "#[rustfmt::skip]\npub static DAMAGE_COLLS: [&[DamageCollDesc]; {}] = [",
        FIGHTERS.len()
    );
    for f in FIGHTERS.iter() {
        let rows = match rom {
            Some(rom) => rows(&rom.file(f.main_file)?.data, f.attributes, f.main_file)?,
            None => Vec::new(),
        };
        let cells: Vec<String> = rows
            .iter()
            .map(|&(joint, placement, grabbable, offset, size)| {
                format!(
                    "\n        hurt({joint}, {placement}, {grabbable}, {}, {})",
                    vec(offset),
                    vec(size)
                )
            })
            .collect();
        let close = if cells.is_empty() { "" } else { ",\n    " };
        let _ = writeln!(out, "    &[{}{close}], // {}", cells.join(","), f.name);
    }
    out.push_str("];\n");
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stub_tables_are_empty() {
        let text = generate(None).unwrap();
        assert!(text.contains("pub static DAMAGE_COLLS: [&[DamageCollDesc]; 27]"));
        assert_eq!(text.matches("    &[], //").count(), 27);
    }
}
