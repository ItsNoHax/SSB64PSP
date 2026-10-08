//! Fighter motion scripts (`ftMotionCommand` bytecode), for
//! `ssb_game::motion`.
//!
//! A status's hitboxes, hit-status windows and flag timings are bytecode in
//! each fighter's `<id>_<Name>MainMotion` archive file, which
//! `ftMainSetStatus` starts and `ftMainUpdateMotionEventsAll` runs. The
//! words are carried as the ROM holds them, with each pointer turned into a
//! word index: into the fighter's own blob, into `FTCommonMoveset` (with
//! [`COMMON_BIT`]), or `NONE_PTR` for a pointer into any other file (throw
//! and damage descriptors). Alongside: each motion's script start,
//! animation length and clip from `dFT<Name>MotionDescs`, the special
//! statuses' motion ids, the combat fields of `FTAttributes`, and the demo
//! scripts of `dFT<Name>SubMotionDescs`.

use std::fmt::Write;

use crate::emit::{self, f32_lit};
use crate::figatree::Lengths;
use crate::fighters::{
    self, demo_words, motion_descs, Fighter, MainMotion, MotionDesc, RegionKind,
    ANIM_FLAG_SUBMOTION_SCRIPT, COMMON_MOVESET, FIGHTERS, NO_OFFSET,
};
use crate::{Result, Source};

/// A pointer outside the blob.
const NONE_PTR: u32 = 0xFFFF_FFFF;
/// Word indexes into the shared `FTCommonMoveset` carry this bit.
const COMMON_BIT: u32 = 0x4000_0000;
/// `nFTCommonStatusSpecialStart`.
const SPECIAL_STATUS_START: u16 = 220;
/// `nFTDemoStatusNull` .. `nFTDemoStatusIntroR` and the opening's nine
/// fighter-specific rows.
const DEMO_ROWS: usize = 24;

/// A `MainMotion` file as words, its pointers resolved, the first word at
/// index `base` of the blob it is part of.
fn motion_words(rom: &Source, file: &MainMotion, base: u32) -> Result<Vec<u32>> {
    let f = rom.file(file.file)?;
    let mut words: Vec<u32> = f
        .data
        .as_chunks::<4>()
        .0
        .iter()
        .map(|c| u32::from_be_bytes(*c))
        .collect();
    // The archive pads each file to 16 bytes past its last array.
    assert!(words.len() >= file.words as usize && words.len() - (file.words as usize) < 4);
    words.truncate(file.words as usize);
    for r in &f.intern_relocs {
        words[r.at as usize / 4] = base + r.target / 4;
    }
    for r in &f.extern_relocs {
        words[r.at as usize / 4] = if u32::from(r.target_file) == COMMON_MOVESET.file {
            COMMON_BIT | (r.target_offset / 4)
        } else {
            NONE_PTR
        };
    }
    for region in file.regions {
        let span = region.start as usize..(region.start + region.len) as usize;
        for w in &mut words[span] {
            match region.kind {
                RegionKind::Data => *w = 0,
                RegionKind::Pointers if *w == 0 => *w = NONE_PTR,
                RegionKind::Pointers => {}
            }
        }
    }
    Ok(words)
}

struct Motion {
    script: u32,
    length: u32,
    clip: u32,
    flags: u32,
}

fn length_of(rom: &Source, lengths: &Lengths, desc: &MotionDesc) -> (u32, u32) {
    if desc.anim_file == 0 {
        return (0, 0xFFFE);
    }
    let frames = match lengths.frames(rom, desc.anim_file) {
        Some(Some(n)) => n,
        _ => 0xFFFF,
    };
    let clip = match lengths.clip(rom, desc.anim_file) {
        Some((n, _)) if n >= 0x8000 => 0xFFFF,
        Some((n, loops)) => n | if loops { 0x8000 } else { 0 },
        None => 0xFFFF,
    };
    (frames, clip)
}

fn emit_words(w: &mut String, label: &str, words: &[u32]) {
    let _ = writeln!(
        w,
        "#[rustfmt::skip]\nstatic {label}: [u32; {}] = [",
        words.len()
    );
    emit::hex_rows_u32(w, words);
    w.push_str("];\n\n");
}

fn emit_motions(w: &mut String, label: &str, motions: &[Motion]) {
    let _ = writeln!(
        w,
        "#[rustfmt::skip]\nstatic {label}: [MotionDesc; {}] = [",
        motions.len()
    );
    for m in motions {
        let script = if m.script == NONE_PTR {
            "NO_SCRIPT".to_string()
        } else {
            m.script.to_string()
        };
        let _ = writeln!(
            w,
            "    MotionDesc {{ script: {script}, anim_length: {}, clip: 0x{:04X}, anim_flags: 0x{:08X} }},",
            m.length, m.clip, m.flags
        );
    }
    w.push_str("];\n\n");
}

fn emit_attrs(w: &mut String, rom: Option<&Source>, f: &Fighter) -> Result<()> {
    let up = f.name.to_uppercase();
    let (mut floats, mut joints, mut light) = ([0f32; 9], [0i32; 5], 0i32);
    // `dead_fgm_ids[2]`, `deadup_sfx`, `damage_sfx`, `smash_sfx[3]`,
    // `heavyget_sfx`, and the `is_have_*` bitfield word.
    let (mut sounds, mut have) = ([0u16; 8], 0u32);
    if let Some(rom) = rom {
        let main = rom.file(f.main_file)?;
        let at = f.attributes as usize;
        let word = |off: usize| crate::source::be_u32(&main.data, at + off);
        // `FTAttributes`: size, rebound_anim_length, shield_size,
        // shield_break_vel_y, jostle_width, jostle_x, hit_detect_range.
        for (i, off) in [0x00, 0x1C, 0x74, 0x78, 0x80, 0x84, 0x290, 0x294, 0x298]
            .into_iter()
            .enumerate()
        {
            floats[i] = f32::from_bits(word(off)?);
        }
        for (i, j) in joints.iter_mut().enumerate() {
            *j = word(0x2A4 + 4 * i)? as i32;
        }
        light = word(0x33C)? as i32;
        // `FTAttributes` from `dead_fgm_ids` (0xB4, after `cliffcatch_coll`)
        // to `heavyget_sfx` (0xE8, after `item_pickup` and the two throw
        // scales); `is_have_attack11..is_have_voice` is the word at 0x100.
        let half = |off: usize| -> Result<u16> {
            let w = word(off & !3)?;
            Ok(if off & 2 == 0 {
                (w >> 16) as u16
            } else {
                w as u16
            })
        };
        for (i, off) in [0xB4, 0xB6, 0xB8, 0xBA, 0xBC, 0xBE, 0xC0, 0xE8]
            .into_iter()
            .enumerate()
        {
            sounds[i] = half(off)?;
        }
        have = word(0x100)?;
    }
    // `is_have_voice`: the 22nd one-bit field, MSB first.
    let is_have_voice = have & (1 << (31 - 21)) != 0;
    // Master Hand's -1: no item-light joint.
    let light = if light < 0 {
        "u8::MAX".to_string()
    } else {
        light.to_string()
    };
    let joints: Vec<String> = joints.iter().map(i32::to_string).collect();
    let _ = write!(
        w,
        "pub static {up}_ATTRS: CombatAttrs = CombatAttrs {{\n\
         \x20   size: {},\n    rebound_anim_length: {},\n\
         \x20   shield_size: {},\n    shield_break_vel_y: {},\n\
         \x20   jostle_width: {},\n    jostle_x: {},\n\
         \x20   hit_detect_range: [{}, {}, {}],\n\
         \x20   effect_joint_ids: [{}],\n\
         \x20   joint_itemlight_id: {light},\n\
         \x20   dead_fgm_ids: [{}, {}],\n    deadup_sfx: {},\n\
         \x20   damage_sfx: {},\n    smash_sfx: [{}, {}, {}],\n\
         \x20   heavyget_sfx: {},\n    is_have_voice: {is_have_voice},\n}};\n\n",
        f32_lit(floats[0]),
        f32_lit(floats[1]),
        f32_lit(floats[2]),
        f32_lit(floats[3]),
        f32_lit(floats[4]),
        f32_lit(floats[5]),
        f32_lit(floats[6]),
        f32_lit(floats[7]),
        f32_lit(floats[8]),
        joints.join(", "),
        sounds[0],
        sounds[1],
        sounds[2],
        sounds[3],
        sounds[4],
        sounds[5],
        sounds[6],
        sounds[7],
    );
    Ok(())
}

fn emit_demo(w: &mut String, rom: Option<&Source>, f: &Fighter, doc: &str) -> Result<()> {
    let up = f.name.to_uppercase();
    let (mut words, mut rows) = (Vec::new(), vec![NONE_PTR; DEMO_ROWS]);
    if let Some(rom) = rom {
        let (vram, mut blob) = demo_words(rom, f)?;
        let index = |ptr: u32| -> u32 {
            assert!(
                ptr >= vram && (ptr - vram).is_multiple_of(4),
                "{}: demo pointer 0x{ptr:08X}",
                f.name
            );
            let i = (ptr - vram) / 4;
            assert!(
                (i as usize) < blob_len(f, rom),
                "{}: demo pointer past the blob",
                f.name
            );
            i
        };
        if let Some(demo) = f.demo {
            for &p in demo.pointers {
                blob[p as usize] = index(blob[p as usize]);
            }
        }
        let subs = motion_descs(rom, f.sub_motion_descs)?;
        for (row, desc) in rows.iter_mut().zip(&subs) {
            if desc.offset != NO_OFFSET && desc.offset != 0 {
                *row = index(desc.offset);
            }
        }
        words = blob;
    }
    w.push_str(doc);
    let _ = writeln!(
        w,
        "#[rustfmt::skip]\nstatic {up}_DEMO_WORDS: [u32; {}] = [",
        words.len()
    );
    emit::hex_rows_u32(w, &words);
    w.push_str("];\n\n");
    let rows: Vec<String> = rows
        .iter()
        .map(|r| {
            if *r == NONE_PTR {
                "NO_SCRIPT".to_string()
            } else {
                r.to_string()
            }
        })
        .collect();
    let _ = write!(
        w,
        "#[rustfmt::skip]\npub static {up}_DEMO: DemoScripts = DemoScripts {{\n\
         \x20   words: &{up}_DEMO_WORDS,\n    rows: [{}],\n}};\n\n",
        rows.join(", ")
    );
    Ok(())
}

fn blob_len(f: &Fighter, rom: &Source) -> usize {
    demo_words(rom, f).map(|(_, w)| w.len()).unwrap_or(0)
}

pub fn generate(rom: Option<&Source>) -> Result<String> {
    let lengths = Lengths::default();
    let mut w = emit::header("fighter motion scripts, motion tables and combat attributes");
    w.push_str("use super::{CombatAttrs, DemoScripts, FighterScripts, MotionDesc, NO_SCRIPT};\n\n");

    let common = match rom {
        Some(rom) => fighters::common_status_motions(rom)?,
        None => vec![-1; crate::stat_flags::COMMON_STATUS_DESCS.1],
    };
    w.push_str("/// `dFTCommonActionStatusDescs[..].mflags.motion_id`, indexed by status id\n");
    w.push_str("/// (`-1`/`-2`: no motion).\n");
    let _ = writeln!(
        w,
        "#[rustfmt::skip]\npub static COMMON_STATUS_MOTION: [i16; {}] = [",
        common.len()
    );
    for row in common.chunks(16) {
        let cells: Vec<String> = row.iter().map(i16::to_string).collect();
        let _ = writeln!(w, "    {},", cells.join(", "));
    }
    w.push_str("];\n\n");
    let _ = write!(
        w,
        "/// `nFTCommonStatusSpecialStart`.\npub const SPECIAL_STATUS_START: u16 = {SPECIAL_STATUS_START};\n\n"
    );

    let common_words = match rom {
        Some(rom) => motion_words(rom, &COMMON_MOVESET, COMMON_BIT)?,
        None => Vec::new(),
    };
    w.push_str(
        "/// `FTCommonMoveset` (relocData 201): the item-swing, damage and star\n\
         /// scripts fighter scripts call as subroutines. A script index with\n\
         /// [`super::COMMON_BIT`] set points here.\n",
    );
    let _ = writeln!(
        w,
        "#[rustfmt::skip]\npub static COMMON_MOVESET_WORDS: [u32; {}] = [",
        common_words.len()
    );
    emit::hex_rows_u32(&mut w, &common_words);
    w.push_str("];\n\n");

    for f in FIGHTERS.iter().filter(|f| f.base.is_none()) {
        let up = f.name.to_uppercase();
        let mm = f.main_motion.expect("a base fighter has a MainMotion file");
        let (mut words, mut motions, mut special) = (Vec::new(), Vec::new(), Vec::new());
        if let Some(rom) = rom {
            words = motion_words(rom, &mm, 0)?;
            for d in motion_descs(rom, f.motion_descs)? {
                let script = if d.offset == NO_OFFSET {
                    NONE_PTR
                } else {
                    d.offset / 4
                };
                let (length, clip) = length_of(rom, &lengths, &d);
                motions.push(Motion {
                    script,
                    length,
                    clip,
                    flags: d.anim_desc,
                });
            }
            if let Some((at, count)) = f.special_status_descs {
                special = fighters::status_motions(rom, at, count)?;
            }
        } else if let Some((_, count)) = f.special_status_descs {
            special = vec![-1; count];
        }
        emit_words(&mut w, &format!("{up}_WORDS"), &words);
        emit_motions(&mut w, &format!("{up}_MOTIONS"), &motions);
        let cells: Vec<String> = special.iter().map(i16::to_string).collect();
        let _ = write!(
            w,
            "#[rustfmt::skip]\nstatic {up}_SPECIAL_MOTION: [i16; {}] = [{}];\n\n",
            special.len(),
            cells.join(", ")
        );
        emit_attrs(&mut w, rom, f)?;
        let _ = write!(
            w,
            "pub static {up}: FighterScripts = FighterScripts {{\n\
             \x20   words: &{up}_WORDS,\n    motions: &{up}_MOTIONS,\n\
             \x20   special_status_motion: &{up}_SPECIAL_MOTION,\n}};\n\n"
        );
        let lower = f.name.to_lowercase();
        emit_demo(
            &mut w,
            rom,
            f,
            &format!(
                "/// `dFT{0}SubMotionDescs` (`sc/scsubsys/scsubsysdata{lower}.c`): the\n\
                 /// demo statuses' scripts, `nFTDemoStatusNull` to `nFTDemoStatusIntroR`.\n",
                f.name
            ),
        )?;
    }

    for f in FIGHTERS.iter().filter(|f| f.base.is_some()) {
        let base = fighters::fighter(f.base.expect("variant"));
        let (up, bup) = (f.name.to_uppercase(), base.name.to_uppercase());
        let base_mm = base.main_motion.expect("base MainMotion");
        // Metal Mario's own MainMotion comes first in its blob; the base's
        // (`file_submotion`) follows it, its pointers moved by its length.
        let (mut sub_base, words_ref) = (0, format!("&{bup}_WORDS"));
        let words_ref = if let Some(own) = f.main_motion {
            let (mut own_words, mut sub_words) = (Vec::new(), Vec::new());
            if let Some(rom) = rom {
                own_words = motion_words(rom, &own, 0)?;
                sub_base = own_words.len() as u32;
                sub_words = motion_words(rom, &base_mm, sub_base)?;
            }
            let _ = write!(
                w,
                "/// `{}_{}MainMotion`, then `{}_{}MainMotion` (`file_submotion`) from\n/// word {sub_base}.\n",
                own.file, f.name, base_mm.file, base.name
            );
            let mut all = own_words;
            all.extend(sub_words);
            emit_words(&mut w, &format!("{up}_WORDS"), &all);
            format!("&{up}_WORDS")
        } else {
            words_ref
        };
        let mut motions = Vec::new();
        if let Some(rom) = rom {
            for d in motion_descs(rom, f.motion_descs)? {
                let script = if d.offset == NO_OFFSET {
                    NONE_PTR
                } else {
                    assert_eq!(d.offset % 4, 0, "{}: unaligned script", f.name);
                    if d.anim_desc & ANIM_FLAG_SUBMOTION_SCRIPT != 0 {
                        sub_base + d.offset / 4
                    } else {
                        assert!(
                            f.main_motion.is_some(),
                            "{}: script names no MainMotion file",
                            f.name
                        );
                        d.offset / 4
                    }
                };
                let (length, clip) = length_of(rom, &lengths, &d);
                motions.push(Motion {
                    script,
                    length,
                    clip,
                    flags: d.anim_desc,
                });
            }
        }
        emit_motions(&mut w, &format!("{up}_MOTIONS"), &motions);
        emit_attrs(&mut w, rom, f)?;
        let _ = write!(
            w,
            "/// `dFT{}MotionDescs` over {}{}'s scripts; {}'s special statuses.\n\
             pub static {up}: FighterScripts = FighterScripts {{\n\
             \x20   words: {words_ref},\n    motions: &{up}_MOTIONS,\n\
             \x20   special_status_motion: &{bup}_SPECIAL_MOTION,\n}};\n\n",
            f.name,
            if f.main_motion.is_some() {
                "its own and "
            } else {
                ""
            },
            base.name,
            base.name
        );
        let lower = f.name.to_lowercase();
        emit_demo(
            &mut w,
            rom,
            f,
            &format!(
                "/// `dFT{0}SubMotionDescs` (`sc/scsubsys/scsubsysdata{lower}.c`).\n",
                f.name
            ),
        )?;
    }
    w.truncate(w.trim_end().len());
    w.push('\n');
    Ok(w)
}
