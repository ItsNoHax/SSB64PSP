//! The pack's `TABLES` section: the ROM-derived tables a runtime reads from
//! `ssb64.pak` instead of from constants compiled into it (D-048), read by
//! `ssb-tables`.
//!
//! This module owns the section's layout: the table ids ([`IDS`], in
//! section order), each table's element kind, and the byte format. The
//! `ssb-tables` build script writes the ids and format constants as Rust
//! ([`ids_rs`]); the values come only from [`section`], from the ROM.
//!
//! Format, little-endian:
//!
//! | Bytes | Field |
//! |---|---|
//! | 0 | [`MAGIC`] |
//! | 4 | [`SECTION_VERSION`] |
//! | 8 | table count |
//! | 12 | 0 |
//! | 16 | directory: per table `id u16, kind u16, offset u32, len u32`, offset from the section start |
//! | after | payload: each table's elements, little-endian, in declaration order, [`TABLE_ALIGN`]-aligned |
//!
//! A table a section lacks reads as zeros (the stub); `section(None)` is
//! the empty section.

use alloc::string::String;
use alloc::vec::Vec;
use core::fmt::Write;

use crate::{d_arrays, sin_table, Result, Source};

/// `words` as little-endian bytes.
#[cfg_attr(not(feature = "std"), allow(dead_code))]
pub(crate) fn le_words(words: &[u32]) -> Vec<u8> {
    le_bytes(words, u32::to_le_bytes)
}

/// `"TBLS"`.
pub const MAGIC: u32 = u32::from_le_bytes(*b"TBLS");
/// The section format's version (the pack's own `VERSION` covers it too).
pub const SECTION_VERSION: u32 = 1;
/// Bytes before the directory.
pub const HEADER_SIZE: usize = 16;
/// Bytes per directory entry.
pub const ENTRY_SIZE: usize = 12;
/// Each table's alignment in the section, so a runtime reads a table of
/// 4-byte elements in place.
pub const TABLE_ALIGN: usize = 4;

/// A table's element type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum Kind {
    U8 = 1,
    U16 = 2,
    U32 = 3,
    I16 = 4,
    I32 = 5,
    F32 = 6,
}

impl Kind {
    const ALL: [(Kind, &'static str); 6] = [
        (Kind::U8, "U8"),
        (Kind::U16, "U16"),
        (Kind::U32, "U32"),
        (Kind::I16, "I16"),
        (Kind::I32, "I32"),
        (Kind::F32, "F32"),
    ];

    /// Bytes per element.
    pub const fn size(self) -> usize {
        match self {
            Kind::U8 => 1,
            Kind::U16 | Kind::I16 => 2,
            Kind::U32 | Kind::I32 | Kind::F32 => 4,
        }
    }
}

/// A table's length (layout).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Len {
    /// Exactly this many elements: the table has a fixed place in a
    /// runtime's store (`TableId::offset`).
    Exact(usize),
    /// As many elements as the ROM's data gives (strings, jagged rows): read
    /// in place from the installed section (`ssb_tables::VarTable`).
    Var,
}

/// Reads one table's little-endian bytes from the ROM.
pub type Reader = fn(&Ctx) -> Result<Vec<u8>>;

/// One table: its `TableId` variant, element kind, what it is and its
/// reader (the table's little-endian bytes). `read` is `None` for a table
/// whose reader needs `std` in a build without it: the section then lacks
/// the table (it reads as zeros).
pub struct TableDesc {
    pub name: &'static str,
    pub kind: Kind,
    pub len: Len,
    pub doc: &'static str,
    pub read: Option<Reader>,
}

impl TableDesc {
    /// The table's bytes in a store: `count * kind.size()`; 0 for
    /// [`Len::Var`].
    pub const fn bytes(&self) -> usize {
        match self.len {
            Len::Exact(n) => n * self.kind.size(),
            Len::Var => 0,
        }
    }
}

/// What the readers share while one section is built: the ROM, and the
/// intermediate results more than one table is read from, made once.
pub struct Ctx<'a> {
    pub rom: &'a Source,
    #[cfg(feature = "std")]
    pub(crate) anim: core::cell::OnceCell<crate::anim_table::Rows>,
}

impl<'a> Ctx<'a> {
    pub fn new(rom: &'a Source) -> Self {
        Self {
            rom,
            #[cfg(feature = "std")]
            anim: core::cell::OnceCell::new(),
        }
    }
}

/// `Some(reader)` with `std`; `None` without (the generator needs `std`
/// today).
macro_rules! std_reader {
    ($f:path) => {{
        #[cfg(feature = "std")]
        let r: Option<Reader> = Some($f);
        #[cfg(not(feature = "std"))]
        let r: Option<Reader> = None;
        r
    }};
}

/// Every table, in id order (`TableId` is the index).
pub const IDS: &[TableDesc] = &[
    TableDesc {
        name: "SinTable",
        kind: Kind::U16,
        len: Len::Exact(2048),
        doc: "`gSYSinTable` (`sys/sintable.c`): `u16[2048]`.",
        read: Some(|c| Ok(le_bytes(&sin_table::values(Some(c.rom))?, u16::to_le_bytes))),
    },
    TableDesc {
        name: "LbCommonSinLookup",
        kind: Kind::F32,
        len: Len::Exact(1024),
        doc: "`dLBCommonSinLookup` (`lb/lbcommon.c`): `f32[1024]`.",
        read: Some(|c| {
            let words = d_arrays::engine_words("LB_COMMON_SIN_LOOKUP", Some(c.rom))?;
            Ok(le_bytes(&words, u32::to_le_bytes))
        }),
    },
    TableDesc {
        name: "DArrU8",
        kind: Kind::U8,
        len: Len::Exact(d_arrays::group_bytes(d_arrays::Group::U8)),
        doc: "The `u8` `d*` arrays of `ssb-game` and `ssb-rom` (`d_arrays`), each 4-byte aligned.",
        read: Some(|c| d_arrays::group(d_arrays::Group::U8, c.rom)),
    },
    TableDesc {
        name: "DArrU16",
        kind: Kind::U16,
        len: Len::Exact(d_arrays::group_bytes(d_arrays::Group::U16) / 2),
        doc: "The `u16` `d*` arrays (`d_arrays`), each 4-byte aligned.",
        read: Some(|c| d_arrays::group(d_arrays::Group::U16, c.rom)),
    },
    TableDesc {
        name: "DArrU32",
        kind: Kind::U32,
        len: Len::Exact(d_arrays::group_bytes(d_arrays::Group::U32) / 4),
        doc: "The `u32` `d*` arrays (`d_arrays`), and each string array's offsets in `DArrStr`.",
        read: Some(|c| d_arrays::group(d_arrays::Group::U32, c.rom)),
    },
    TableDesc {
        name: "DArrI32",
        kind: Kind::I32,
        len: Len::Exact(d_arrays::group_bytes(d_arrays::Group::I32) / 4),
        doc: "The `s32` `d*` arrays (`d_arrays`).",
        read: Some(|c| d_arrays::group(d_arrays::Group::I32, c.rom)),
    },
    TableDesc {
        name: "DArrF32",
        kind: Kind::F32,
        len: Len::Exact(d_arrays::group_bytes(d_arrays::Group::F32) / 4),
        doc: "The `f32` `d*` arrays (`d_arrays`).",
        read: Some(|c| d_arrays::group(d_arrays::Group::F32, c.rom)),
    },
    TableDesc {
        name: "DArrStr",
        kind: Kind::U8,
        len: Len::Var,
        doc: "The `char *` `d*` arrays' strings, NUL-terminated, after one NUL (a `NULL`).",
        read: Some(|c| d_arrays::strings(c.rom)),
    },
    TableDesc {
        name: "FighterFiles",
        kind: Kind::U32,
        len: Len::Exact(27 * 7),
        doc: "`dFTManagerDataFiles` by `FTKind`: `file_main_id`, `o_attributes`, \
              `file_special1_id`..`4`, and bit `k` set when special file `k` is loaded.",
        read: std_reader!(crate::rom_layout::fighter_files_bytes),
    },
    TableDesc {
        name: "TransitionDescs",
        kind: Kind::U32,
        len: Len::Exact(11 * 3),
        doc: "`dLBTransitionDescs`: `file_id`, `o_dobjdesc`, `o_anim_joint`.",
        read: std_reader!(crate::rom_layout::transitions_bytes),
    },
    TableDesc {
        name: "ManagerEffects",
        kind: Kind::U32,
        len: Len::Exact(46 * 6),
        doc: "`dEFManager*EffectDesc` by slot: file, `o_dobjdesc`, `o_anim_joint`, \
              `o_matanim_joint`, `dl_link`, `proc_display`.",
        read: std_reader!(crate::rom_layout::effects_bytes),
    },
    TableDesc {
        name: "WeaponDescs",
        kind: Kind::U32,
        len: Len::Exact(35 * 6),
        doc: "Each weapon's attributes file and offset, `WPDesc.flags`, and where \
              `WPAttributes.data` relocates to (present, file, offset).",
        read: std_reader!(crate::rom_layout::weapons_bytes),
    },
    TableDesc {
        name: "Bonus1Courses",
        kind: Kind::U32,
        len: Len::Exact(12 * 3),
        doc: "`dSC1PBonusStageTargetDescs`: `start`, `dobjdesc`, `anim_joint`.",
        read: std_reader!(crate::sc1p_data::courses_bytes),
    },
    TableDesc {
        name: "Bonus2Trees",
        kind: Kind::U32,
        len: Len::Exact(6 * 4),
        doc: "`dSC1PBonusStagePlatformDescs`, then `dSC1PBonusStageBoardedPlatformDescs` (two words and two zeros).",
        read: std_reader!(crate::sc1p_data::trees_bytes),
    },
    TableDesc {
        name: "Bonus2Bumpers",
        kind: Kind::U32,
        len: Len::Exact(12 * 2),
        doc: "`dSC1PBonusStageBumperDescs`: `dobjdesc`, `anim_joint` (0, 0: none).",
        read: std_reader!(crate::sc1p_data::bumpers_bytes),
    },
    TableDesc {
        name: "CampaignBonusLabels",
        kind: Kind::U32,
        len: Len::Exact(58),
        doc: "`dSC1PStageClearBonusData`'s `offset`s.",
        read: std_reader!(crate::sc1p_data::bonus_labels_bytes),
    },
    TableDesc {
        name: "CampaignAllyCameras",
        kind: Kind::F32,
        len: Len::Exact(12 * 6 * 6),
        doc: "`sc1PIntroGetFighterCObjDesc`'s `cobj_descs` eye and at points.",
        read: std_reader!(crate::sc1p_data::ally_cameras_bytes),
    },
    TableDesc {
        name: "EndingCongra",
        kind: Kind::U32,
        len: Len::Exact(12 * 4),
        doc: "`dMNCongraPictures`: bottom file and offset, top file and offset.",
        read: std_reader!(crate::ending_data::congra_bytes),
    },
    TableDesc {
        name: "StaffrollNameImages",
        kind: Kind::U32,
        len: Len::Exact(56),
        doc: "`dSCStaffrollNameAndJobSpriteInfo`'s `offset`s.",
        read: std_reader!(crate::ending_data::name_images_bytes),
    },
    TableDesc {
        name: "StaffrollTextSprites",
        kind: Kind::U32,
        len: Len::Exact(74),
        doc: "`dSCStaffrollTextBoxSpriteInfo`'s `offset`s.",
        read: std_reader!(crate::ending_data::text_sprites_bytes),
    },
    TableDesc {
        name: "AnimFiles",
        kind: Kind::U16,
        len: Len::Exact(27 * ANIM_SLOTS),
        doc: "Each fighter's animation file per slot (`ssb_rom::anim`).",
        read: std_reader!(crate::anim_table::files_bytes),
    },
    TableDesc {
        name: "AnimRuntimeBits",
        kind: Kind::U32,
        len: Len::Exact(27 * ANIM_SLOTS),
        doc: "Each fighter's `FTAnimDesc` word per slot: hidden-part bits 31..5, leading runtime joint 31..29.",
        read: std_reader!(crate::anim_table::runtime_bytes),
    },
    TableDesc {
        name: "AnimExpectedFrames",
        kind: Kind::U16,
        len: Len::Exact(27 * ANIM_SLOTS),
        doc: "Each fighter's figatree length per slot (0: none, a loop, an `AnimJoint` clip).",
        read: std_reader!(crate::anim_table::frames_bytes),
    },
    TableDesc {
        name: "AnimBossJoint",
        kind: Kind::U8,
        len: Len::Exact(30),
        doc: "Master Hand's slots holding a 32-bit `AnimJoint` clip (1).",
        read: std_reader!(crate::anim_table::boss_joint_bytes),
    },
];

/// `ssb_rom::anim::SLOT_COUNT` (layout; `anim_table::SLOTS`).
pub const ANIM_SLOTS: usize = 667;

// The counts above are the generators' layout.
#[cfg(feature = "std")]
const _: () = {
    assert!(crate::anim_table::SLOT_COUNT == ANIM_SLOTS);
    assert!(crate::rom_layout::FIGHTER_DATA_FILES.1 == 27);
    assert!(crate::rom_layout::TRANSITION_DESCS.1 == 11);
    assert!(crate::rom_layout::EFFECTS.len() == 46);
    assert!(crate::rom_layout::WEAPON_DESCS.len() == 35);
    assert!(crate::sc1p_data::BONUS_TARGETS.1 == 12);
    assert!(crate::sc1p_data::TREE_COUNT == 6);
    assert!(crate::sc1p_data::BONUS_BUMPERS.1 == 12);
    assert!(crate::sc1p_data::STAGE_CLEAR_BONUS.1 == 58);
    assert!(crate::sc1p_data::ALLY_CAMERAS.1 == 12 && crate::sc1p_data::ALLY_CAMERAS.2 == 6);
    assert!(crate::ending_data::CONGRA_PICTURES.1 == 12);
    assert!(crate::ending_data::NAME_AND_JOB.1 == 56);
    assert!(crate::ending_data::TEXT_BOX.1 == 74);
};

/// The `ssb-tables` build script's `table_ids.rs`: the ids, kinds and
/// format constants. Layout only.
pub fn ids_rs() -> String {
    let mut out = String::from(
        "// Generated by crates/ssb-tables/build.rs from ssb_tablegen::tables.\n\
         // Layout only (no ROM data); never edit.\n\n",
    );
    let _ = writeln!(out, "/// The section's magic, `\"TBLS\"`.");
    let _ = writeln!(out, "pub const MAGIC: u32 = 0x{MAGIC:08X};");
    let _ = writeln!(out, "/// The section format's version.");
    let _ = writeln!(out, "pub const SECTION_VERSION: u32 = {SECTION_VERSION};");
    let _ = writeln!(out, "/// Bytes before the directory.");
    let _ = writeln!(out, "pub const HEADER_SIZE: usize = {HEADER_SIZE};");
    let _ = writeln!(out, "/// Bytes per directory entry.");
    let _ = writeln!(out, "pub const ENTRY_SIZE: usize = {ENTRY_SIZE};");
    out.push_str(
        "\n/// A table's element type.\n\
         #[derive(Debug, Clone, Copy, PartialEq, Eq)]\n#[repr(u16)]\npub enum Kind {\n",
    );
    for (k, name) in Kind::ALL {
        let _ = writeln!(out, "    {name} = {},", k as u16);
    }
    out.push_str("}\n\nimpl Kind {\n    /// The kind with this code.\n    pub const fn from_u16(v: u16) -> Option<Kind> {\n        match v {\n");
    for (k, name) in Kind::ALL {
        let _ = writeln!(out, "            {} => Some(Kind::{name}),", k as u16);
    }
    out.push_str("            _ => None,\n        }\n    }\n\n    /// One element's size in bytes.\n    pub const fn size(self) -> usize {\n        match self {\n");
    for (k, name) in Kind::ALL {
        let _ = writeln!(out, "            Kind::{name} => {},", k.size());
    }
    out.push_str("        }\n    }\n}\n");
    out.push_str(
        "\n/// A table in the section.\n\
         #[derive(Debug, Clone, Copy, PartialEq, Eq)]\n#[repr(u16)]\npub enum TableId {\n",
    );
    for (i, t) in IDS.iter().enumerate() {
        let _ = writeln!(out, "    /// {}\n    {} = {i},", t.doc, t.name);
    }
    out.push_str("}\n\nimpl TableId {\n");
    let _ = writeln!(out, "    /// Every id, in section order.");
    let _ = writeln!(out, "    pub const ALL: [TableId; {}] = [", IDS.len());
    for t in IDS {
        let _ = writeln!(out, "        TableId::{},", t.name);
    }
    out.push_str("    ];\n\n    /// The table's element kind.\n    pub const fn kind(self) -> Kind {\n        match self {\n");
    for t in IDS {
        let kind = Kind::ALL.iter().find(|(k, _)| *k == t.kind).unwrap().1;
        let _ = writeln!(out, "            TableId::{} => Kind::{kind},", t.name);
    }
    out.push_str("        }\n    }\n\n    /// The table's length in bytes.\n    pub const fn size(self) -> usize {\n        match self {\n");
    for t in IDS {
        let _ = writeln!(out, "            TableId::{} => {},", t.name, t.bytes());
    }
    out.push_str("        }\n    }\n\n    /// The table's offset in a store that holds every table, each 4-byte aligned.\n    pub const fn offset(self) -> usize {\n        match self {\n");
    let mut at = 0;
    for t in IDS {
        let _ = writeln!(out, "            TableId::{} => {at},", t.name);
        at = (at + t.bytes()).next_multiple_of(4);
    }
    out.push_str("        }\n    }\n\n    /// A variable-length table: read in place, not from the store (`size` 0).\n    pub const fn is_var(self) -> bool {\n        match self {\n");
    for t in IDS {
        let _ = writeln!(
            out,
            "            TableId::{} => {},",
            t.name,
            t.len == Len::Var
        );
    }
    out.push_str("        }\n    }\n}\n");
    let _ = writeln!(
        out,
        "\n/// The bytes of a store that holds every table (`TableId::offset`)."
    );
    let _ = writeln!(out, "pub const STORE_SIZE: usize = {at};");
    let max = IDS.iter().map(TableDesc::bytes).max().unwrap_or(0);
    let _ = writeln!(out, "\n/// The largest table's length in bytes.");
    let _ = writeln!(out, "pub const MAX_SIZE: usize = {max};");
    out
}

/// `values` as little-endian bytes.
pub(crate) fn le_bytes<T: Copy, const N: usize>(values: &[T], f: fn(T) -> [u8; N]) -> Vec<u8> {
    values.iter().flat_map(|&v| f(v)).collect()
}

/// The `TABLES` section read from `rom`; the empty section (every table
/// zeros) for `None`.
pub fn section(rom: Option<&Source>) -> Result<Vec<u8>> {
    let tables: Vec<(usize, Vec<u8>)> = match rom {
        Some(rom) => {
            let ctx = Ctx::new(rom);
            IDS.iter()
                .enumerate()
                .filter_map(|(i, t)| Some((i, t, t.read?)))
                .map(|(i, t, read)| {
                    let bytes = read(&ctx)?;
                    // The layout's size: a mismatch is a reader bug, not a
                    // ROM's.
                    match t.len {
                        Len::Exact(_) => assert_eq!(bytes.len(), t.bytes(), "table {}", t.name),
                        Len::Var => assert_eq!(bytes.len() % t.kind.size(), 0, "table {}", t.name),
                    }
                    Ok((i, bytes))
                })
                .collect::<Result<_>>()?
        }
        None => Vec::new(),
    };
    let mut out = Vec::new();
    for v in [MAGIC, SECTION_VERSION, tables.len() as u32, 0] {
        out.extend_from_slice(&v.to_le_bytes());
    }
    let mut at = HEADER_SIZE + tables.len() * ENTRY_SIZE;
    let mut offsets = Vec::with_capacity(tables.len());
    for (_, t) in &tables {
        at = at.next_multiple_of(TABLE_ALIGN);
        offsets.push(at);
        at += t.len();
    }
    for ((i, t), &at) in tables.iter().zip(&offsets) {
        out.extend_from_slice(&(*i as u16).to_le_bytes());
        out.extend_from_slice(&(IDS[*i].kind as u16).to_le_bytes());
        out.extend_from_slice(&(at as u32).to_le_bytes());
        out.extend_from_slice(&(t.len() as u32).to_le_bytes());
    }
    for ((_, t), &at) in tables.iter().zip(&offsets) {
        out.resize(at, 0);
        out.extend_from_slice(t);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::format;

    #[test]
    fn the_empty_section_is_a_header() {
        let s = section(None).unwrap();
        assert_eq!(s.len(), HEADER_SIZE);
        assert_eq!(&s[0..4], b"TBLS");
        assert_eq!(u32::from_le_bytes(s[8..12].try_into().unwrap()), 0);
    }

    #[test]
    fn ids_rs_names_every_table() {
        let text = ids_rs();
        for t in IDS {
            assert!(text.contains(&format!("    {} = ", t.name)), "{}", t.name);
        }
        assert!(text.contains("pub const MAGIC: u32 = 0x534C4254;"));
    }
}
