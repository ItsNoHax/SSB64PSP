//! Where each fighter's motion data sits: its `MainMotion` archive file,
//! its `FTMotionDesc` and special-status tables, its `FTAttributes`, and its
//! demo (`scsubsys`) scripts.

use crate::source::{be_u32, OVL1};
use crate::{Result, Source};

/// A stretch of a `MainMotion` file that is not motion-script bytecode.
#[derive(Debug, Clone, Copy)]
pub struct Region {
    /// First word.
    pub start: u32,
    /// Words.
    pub len: u32,
    pub kind: RegionKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegionKind {
    /// Embedded descriptors (`FTThrowHitDesc`, `FTSpecialColl`,
    /// `FTKirbyCopy`, `Vec2h`, `WPAttributes`, `f32`): never interpreted as
    /// script, carried as zero words.
    Data,
    /// A pointer array (`void *[]`, `FTMotionDamageScript`): a null entry is
    /// carried as `NONE_PTR`.
    Pointers,
}

/// A fighter's `<id>_<Name>MainMotion` archive file.
#[derive(Debug, Clone, Copy)]
pub struct MainMotion {
    pub file: u32,
    /// Words of script and data; the archive pads the file past them to 16
    /// bytes.
    pub words: u32,
    pub regions: &'static [Region],
}

/// A fighter's demo scripts: the `s32` arrays of
/// `sc/scsubsys/scsubsysdata<name>.c`, which end where its
/// `dFT<Name>SubMotionDescs` table begins.
#[derive(Debug, Clone, Copy)]
pub struct DemoBlob {
    /// Run-time address of the first array.
    pub vram: u32,
    /// Words that hold a pointer into the blob (`Goto`, `Subroutine`,
    /// `SetParallelScript` targets).
    pub pointers: &'static [u32],
}

#[derive(Debug, Clone, Copy)]
pub struct Fighter {
    pub name: &'static str,
    /// The fighter whose special statuses and (without a `MainMotion` of its
    /// own) whose scripts a variant uses: Metal Mario, the Polygons, Giant
    /// Donkey Kong.
    pub base: Option<&'static str>,
    pub main_motion: Option<MainMotion>,
    /// `dFT<Name>MotionDescs`: ROM offset and count.
    pub motion_descs: (u32, usize),
    /// `dFT<Name>SpecialStatusDescs`: ROM offset and count.
    pub special_status_descs: Option<(u32, usize)>,
    /// `<id>_<Name>Main` and the offset of its `FTAttributes`
    /// (`ssb_rom::fighter::FIGHTER_FILES`).
    pub main_file: u32,
    pub attributes: u32,
    /// `dFT<Name>SubMotionDescs`: ROM offset and count.
    pub sub_motion_descs: (u32, usize),
    pub demo: Option<DemoBlob>,
}

/// `FTCommonMoveset` (relocData 201): the item-swing, damage and star
/// scripts every fighter's scripts call; a pointer array of damage scripts
/// from word 470.
pub const COMMON_MOVESET: MainMotion = MainMotion {
    file: 201,
    words: 524,
    regions: &[Region {
        start: 470,
        len: 54,
        kind: RegionKind::Pointers,
    }],
};

/// The thirteen fighters with motion files of their own, then the fourteen
/// variants, in the order the generated tables list them.
pub const FIGHTERS: [Fighter; 27] = [
    Fighter {
        name: "Mario",
        base: None,
        main_motion: Some(MainMotion {
            file: 202,
            words: 1640,
            regions: &[
                Region {
                    start: 749,
                    len: 14,
                    kind: RegionKind::Data,
                },
                Region {
                    start: 780,
                    len: 14,
                    kind: RegionKind::Data,
                },
                Region {
                    start: 817,
                    len: 14,
                    kind: RegionKind::Data,
                },
            ],
        }),
        motion_descs: (0x092680, 204),
        special_status_descs: Some((0x0A5708, 9)),
        main_file: 203,
        attributes: 0x0428,
        sub_motion_descs: (0x108480, 19),
        demo: Some(DemoBlob {
            vram: 0x80390DC0,
            pointers: &[11, 14, 17, 20, 23, 27, 31, 34, 38, 42, 49, 52, 55],
        }),
    },
    Fighter {
        name: "Fox",
        base: None,
        main_motion: Some(MainMotion {
            file: 208,
            words: 1703,
            regions: &[
                Region {
                    start: 776,
                    len: 14,
                    kind: RegionKind::Data,
                },
                Region {
                    start: 803,
                    len: 14,
                    kind: RegionKind::Data,
                },
                Region {
                    start: 840,
                    len: 14,
                    kind: RegionKind::Data,
                },
                Region {
                    start: 1644,
                    len: 9,
                    kind: RegionKind::Data,
                },
            ],
        }),
        motion_descs: (0x0944B0, 219),
        special_status_descs: Some((0x0A5A14, 26)),
        main_file: 209,
        attributes: 0x046C,
        sub_motion_descs: (0x108734, 24),
        demo: Some(DemoBlob {
            vram: 0x80391110,
            pointers: &[],
        }),
    },
    Fighter {
        name: "Donkey",
        base: None,
        main_motion: Some(MainMotion {
            file: 212,
            words: 1844,
            regions: &[
                Region {
                    start: 773,
                    len: 14,
                    kind: RegionKind::Data,
                },
                Region {
                    start: 828,
                    len: 14,
                    kind: RegionKind::Data,
                },
                Region {
                    start: 866,
                    len: 14,
                    kind: RegionKind::Data,
                },
            ],
        }),
        motion_descs: (0x095A30, 221),
        special_status_descs: Some((0x0A57BC, 30)),
        main_file: 213,
        attributes: 0x04A4,
        sub_motion_descs: (0x10893C, 15),
        demo: Some(DemoBlob {
            vram: 0x80391340,
            pointers: &[],
        }),
    },
    Fighter {
        name: "Samus",
        base: None,
        main_motion: Some(MainMotion {
            file: 216,
            words: 1968,
            regions: &[
                Region {
                    start: 845,
                    len: 14,
                    kind: RegionKind::Data,
                },
                Region {
                    start: 904,
                    len: 14,
                    kind: RegionKind::Data,
                },
                Region {
                    start: 942,
                    len: 14,
                    kind: RegionKind::Data,
                },
            ],
        }),
        motion_descs: (0x097AD0, 206),
        special_status_descs: Some((0x0A5C1C, 11)),
        main_file: 217,
        attributes: 0x0610,
        sub_motion_descs: (0x108B60, 15),
        demo: Some(DemoBlob {
            vram: 0x803914F0,
            pointers: &[],
        }),
    },
    Fighter {
        name: "Luigi",
        base: None,
        main_motion: Some(MainMotion {
            file: 220,
            words: 1676,
            regions: &[
                Region {
                    start: 764,
                    len: 14,
                    kind: RegionKind::Data,
                },
                Region {
                    start: 795,
                    len: 14,
                    kind: RegionKind::Data,
                },
                Region {
                    start: 832,
                    len: 14,
                    kind: RegionKind::Data,
                },
            ],
        }),
        motion_descs: (0x098F10, 204),
        special_status_descs: Some((0x0A5CF8, 9)),
        main_file: 221,
        attributes: 0x0580,
        sub_motion_descs: (0x108D9C, 15),
        demo: Some(DemoBlob {
            vram: 0x80391700,
            pointers: &[11, 14, 17, 20],
        }),
    },
    Fighter {
        name: "Link",
        base: None,
        main_motion: Some(MainMotion {
            file: 224,
            words: 1980,
            regions: &[
                Region {
                    start: 919,
                    len: 14,
                    kind: RegionKind::Data,
                },
                Region {
                    start: 973,
                    len: 14,
                    kind: RegionKind::Data,
                },
                Region {
                    start: 1026,
                    len: 14,
                    kind: RegionKind::Data,
                },
            ],
        }),
        motion_descs: (0x09A330, 212),
        special_status_descs: Some((0x0A5DAC, 17)),
        main_file: 225,
        attributes: 0x0708,
        sub_motion_descs: (0x108FE0, 16),
        demo: Some(DemoBlob {
            vram: 0x80391890,
            pointers: &[6, 9, 12, 15],
        }),
    },
    Fighter {
        name: "Yoshi",
        base: None,
        main_motion: Some(MainMotion {
            file: 246,
            words: 1520,
            regions: &[
                Region {
                    start: 831,
                    len: 14,
                    kind: RegionKind::Data,
                },
                Region {
                    start: 868,
                    len: 14,
                    kind: RegionKind::Data,
                },
                Region {
                    start: 907,
                    len: 14,
                    kind: RegionKind::Data,
                },
                Region {
                    start: 1470,
                    len: 14,
                    kind: RegionKind::Data,
                },
            ],
        }),
        motion_descs: (0x09B810, 208),
        special_status_descs: Some((0x0A66F8, 14)),
        main_file: 247,
        attributes: 0x047C,
        sub_motion_descs: (0x109280, 19),
        demo: Some(DemoBlob {
            vram: 0x80391B90,
            pointers: &[14, 17, 20, 23],
        }),
    },
    Fighter {
        name: "Captain",
        base: None,
        main_motion: Some(MainMotion {
            file: 235,
            words: 1900,
            regions: &[
                Region {
                    start: 0,
                    len: 27,
                    kind: RegionKind::Data,
                },
                Region {
                    start: 842,
                    len: 14,
                    kind: RegionKind::Data,
                },
                Region {
                    start: 873,
                    len: 14,
                    kind: RegionKind::Data,
                },
                Region {
                    start: 916,
                    len: 14,
                    kind: RegionKind::Data,
                },
                Region {
                    start: 1813,
                    len: 14,
                    kind: RegionKind::Data,
                },
            ],
        }),
        motion_descs: (0x09CC90, 214),
        special_status_descs: Some((0x0A657C, 19)),
        main_file: 236,
        attributes: 0x0488,
        sub_motion_descs: (0x109470, 15),
        demo: Some(DemoBlob {
            vram: 0x80391E50,
            pointers: &[],
        }),
    },
    Fighter {
        name: "Kirby",
        base: None,
        main_motion: Some(MainMotion {
            file: 228,
            words: 2412,
            regions: &[
                Region {
                    start: 0,
                    len: 81,
                    kind: RegionKind::Data,
                },
                Region {
                    start: 915,
                    len: 14,
                    kind: RegionKind::Data,
                },
                Region {
                    start: 950,
                    len: 14,
                    kind: RegionKind::Data,
                },
                Region {
                    start: 1010,
                    len: 14,
                    kind: RegionKind::Data,
                },
                Region {
                    start: 1160,
                    len: 36,
                    kind: RegionKind::Data,
                },
                Region {
                    start: 1829,
                    len: 14,
                    kind: RegionKind::Data,
                },
                Region {
                    start: 2152,
                    len: 14,
                    kind: RegionKind::Data,
                },
            ],
        }),
        motion_descs: (0x09E190, 276),
        special_status_descs: Some((0x0A5F00, 83)),
        main_file: 229,
        attributes: 0x0808,
        sub_motion_descs: (0x109694, 16),
        demo: Some(DemoBlob {
            vram: 0x80392010,
            pointers: &[5, 8, 11, 18, 31, 34, 37, 40],
        }),
    },
    Fighter {
        name: "Pikachu",
        base: None,
        main_motion: Some(MainMotion {
            file: 242,
            words: 1496,
            regions: &[
                Region {
                    start: 774,
                    len: 14,
                    kind: RegionKind::Data,
                },
                Region {
                    start: 802,
                    len: 14,
                    kind: RegionKind::Data,
                },
                Region {
                    start: 839,
                    len: 14,
                    kind: RegionKind::Data,
                },
            ],
        }),
        motion_descs: (0x09FB00, 211),
        special_status_descs: Some((0x0A6810, 18)),
        main_file: 243,
        attributes: 0x041C,
        sub_motion_descs: (0x1098D0, 16),
        demo: Some(DemoBlob {
            vram: 0x80392240,
            pointers: &[5, 8, 11, 14, 17, 20, 30, 35, 40],
        }),
    },
    Fighter {
        name: "Purin",
        base: None,
        main_motion: Some(MainMotion {
            file: 232,
            words: 1504,
            regions: &[
                Region {
                    start: 820,
                    len: 14,
                    kind: RegionKind::Data,
                },
                Region {
                    start: 857,
                    len: 14,
                    kind: RegionKind::Data,
                },
                Region {
                    start: 917,
                    len: 14,
                    kind: RegionKind::Data,
                },
            ],
        }),
        motion_descs: (0x0A0FB0, 209),
        special_status_descs: Some((0x0A6978, 16)),
        main_file: 233,
        attributes: 0x0474,
        sub_motion_descs: (0x109B80, 15),
        demo: Some(DemoBlob {
            vram: 0x80392480,
            pointers: &[
                14, 17, 20, 23, 26, 29, 32, 36, 39, 42, 45, 48, 60, 63, 66, 69,
            ],
        }),
    },
    Fighter {
        name: "Ness",
        base: None,
        main_motion: Some(MainMotion {
            file: 238,
            words: 1514,
            regions: &[
                Region {
                    start: 755,
                    len: 14,
                    kind: RegionKind::Data,
                },
                Region {
                    start: 787,
                    len: 14,
                    kind: RegionKind::Data,
                },
                Region {
                    start: 846,
                    len: 14,
                    kind: RegionKind::Data,
                },
                Region {
                    start: 1093,
                    len: 9,
                    kind: RegionKind::Data,
                },
                Region {
                    start: 1461,
                    len: 9,
                    kind: RegionKind::Data,
                },
            ],
        }),
        motion_descs: (0x0A2450, 220),
        special_status_descs: Some((0x0A6AB8, 25)),
        main_file: 239,
        attributes: 0x05BC,
        sub_motion_descs: (0x109D40, 15),
        demo: Some(DemoBlob {
            vram: 0x80392670,
            pointers: &[11, 15, 18, 21],
        }),
    },
    Fighter {
        name: "Boss",
        base: None,
        main_motion: Some(MainMotion {
            file: 249,
            words: 620,
            regions: &[
                Region {
                    start: 477,
                    len: 13,
                    kind: RegionKind::Data,
                },
                Region {
                    start: 490,
                    len: 13,
                    kind: RegionKind::Data,
                },
            ],
        }),
        motion_descs: (0x0A39F0, 225),
        special_status_descs: Some((0x0A6CAC, 33)),
        main_file: 250,
        attributes: 0x00E8,
        sub_motion_descs: (0x109EC8, 18),
        demo: Some(DemoBlob {
            vram: 0x803928E0,
            pointers: &[],
        }),
    },
    Fighter {
        name: "MMario",
        base: Some("Mario"),
        main_motion: Some(MainMotion {
            file: 205,
            words: 80,
            regions: &[],
        }),
        motion_descs: (0x093090, 204),
        special_status_descs: None,
        main_file: 206,
        attributes: 0x02A8,
        sub_motion_descs: (0x108570, 15),
        demo: None,
    },
    Fighter {
        name: "NMario",
        base: Some("Mario"),
        main_motion: None,
        motion_descs: (0x093AA0, 204),
        special_status_descs: None,
        main_file: 207,
        attributes: 0x0298,
        sub_motion_descs: (0x108630, 15),
        demo: None,
    },
    Fighter {
        name: "NFox",
        base: Some("Fox"),
        main_motion: None,
        motion_descs: (0x094F70, 219),
        special_status_descs: None,
        main_file: 211,
        attributes: 0x02A4,
        sub_motion_descs: (0x108860, 15),
        demo: None,
    },
    Fighter {
        name: "NDonkey",
        base: Some("Donkey"),
        main_motion: None,
        motion_descs: (0x096510, 221),
        special_status_descs: None,
        main_file: 214,
        attributes: 0x0298,
        sub_motion_descs: (0x108A00, 15),
        demo: None,
    },
    Fighter {
        name: "NSamus",
        base: Some("Samus"),
        main_motion: None,
        motion_descs: (0x0984F0, 206),
        special_status_descs: None,
        main_file: 219,
        attributes: 0x03BC,
        sub_motion_descs: (0x108C20, 15),
        demo: None,
    },
    Fighter {
        name: "NLuigi",
        base: Some("Luigi"),
        main_motion: None,
        motion_descs: (0x099920, 204),
        special_status_descs: None,
        main_file: 223,
        attributes: 0x0298,
        sub_motion_descs: (0x108E60, 1),
        demo: None,
    },
    Fighter {
        name: "NLink",
        base: Some("Link"),
        main_motion: None,
        motion_descs: (0x09ADA0, 212),
        special_status_descs: None,
        main_file: 227,
        attributes: 0x02D8,
        sub_motion_descs: (0x1090B0, 15),
        demo: None,
    },
    Fighter {
        name: "NYoshi",
        base: Some("Yoshi"),
        main_motion: None,
        motion_descs: (0x09C250, 208),
        special_status_descs: None,
        main_file: 248,
        attributes: 0x02B8,
        sub_motion_descs: (0x109370, 15),
        demo: None,
    },
    Fighter {
        name: "NCaptain",
        base: Some("Captain"),
        main_motion: None,
        motion_descs: (0x09D710, 214),
        special_status_descs: None,
        main_file: 237,
        attributes: 0x029C,
        sub_motion_descs: (0x109530, 15),
        demo: None,
    },
    Fighter {
        name: "NKirby",
        base: Some("Kirby"),
        main_motion: None,
        motion_descs: (0x09EF00, 245),
        special_status_descs: None,
        main_file: 231,
        attributes: 0x02C0,
        sub_motion_descs: (0x109760, 15),
        demo: None,
    },
    Fighter {
        name: "NPikachu",
        base: Some("Pikachu"),
        main_motion: None,
        motion_descs: (0x0A0560, 209),
        special_status_descs: None,
        main_file: 245,
        attributes: 0x02A8,
        sub_motion_descs: (0x1099A0, 15),
        demo: None,
    },
    Fighter {
        name: "NPurin",
        base: Some("Purin"),
        main_motion: None,
        motion_descs: (0x0A1A00, 209),
        special_status_descs: None,
        main_file: 234,
        attributes: 0x02A0,
        sub_motion_descs: (0x109C40, 1),
        demo: None,
    },
    Fighter {
        name: "NNess",
        base: Some("Ness"),
        main_motion: None,
        motion_descs: (0x0A2F20, 220),
        special_status_descs: None,
        main_file: 241,
        attributes: 0x02F0,
        sub_motion_descs: (0x109E00, 15),
        demo: None,
    },
    Fighter {
        name: "GDonkey",
        base: Some("Donkey"),
        main_motion: None,
        motion_descs: (0x096FF0, 221),
        special_status_descs: None,
        main_file: 215,
        attributes: 0x03C8,
        sub_motion_descs: (0x108AC0, 1),
        demo: None,
    },
];

pub fn fighter(name: &str) -> &'static Fighter {
    FIGHTERS
        .iter()
        .find(|f| f.name == name)
        .unwrap_or_else(|| panic!("no fighter {name}"))
}

/// `FTANIM_FLAG_SUBMOTION_SCRIPT`: the motion's script is in
/// `file_submotion` (the base fighter's `MainMotion`).
pub const ANIM_FLAG_SUBMOTION_SCRIPT: u32 = 0x10;
/// `FTANIM_FLAG_ANIMJOINT`: the animation is a 32-bit `AnimJoint`, not a
/// figatree.
pub const ANIM_FLAG_ANIMJOINT: u32 = 0x08;
/// `FTMotionDesc.offset` of a motion with no script.
pub const NO_OFFSET: u32 = 0x8000_0000;

/// One `FTMotionDesc`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MotionDesc {
    /// Archive file of the animation; 0 for none.
    pub anim_file: u32,
    /// Byte offset of the script in its motion file, or [`NO_OFFSET`].
    /// Demo rows hold a run-time address instead.
    pub offset: u32,
    /// `FTAnimDesc` bits.
    pub anim_desc: u32,
}

const MOTION_DESC_SIZE: u32 = 12;

/// The `FTMotionDesc`s at `(rom, count)`.
pub fn motion_descs(rom: &Source, (at, count): (u32, usize)) -> Result<Vec<MotionDesc>> {
    (0..count as u32)
        .map(|i| {
            let at = at + i * MOTION_DESC_SIZE;
            Ok(MotionDesc {
                anim_file: rom.u32(at)?,
                offset: rom.u32(at + 4)?,
                anim_desc: rom.u32(at + 8)?,
            })
        })
        .collect()
}

/// `FTStatusDesc.mflags.motion_id` (`s16 : 10`) of the `count` statuses at
/// `at`: -1 and -2 mean no motion.
pub fn status_motions(rom: &Source, at: u32, count: usize) -> Result<Vec<i16>> {
    (0..count as u32)
        .map(|i| Ok((rom.u16(at + i * crate::stat_flags::STATUS_DESC_SIZE)? as i16) >> 6))
        .collect()
}

/// `dFTCommonNullStatusDescs` and `dFTCommonActionStatusDescs`' motion ids.
pub fn common_status_motions(rom: &Source) -> Result<Vec<i16>> {
    let (at, count) = crate::stat_flags::COMMON_STATUS_DESCS;
    status_motions(rom, at, count)
}

/// The demo blob's words and its first word's run-time address.
pub fn demo_words(rom: &Source, f: &Fighter) -> Result<(u32, Vec<u32>)> {
    let Some(blob) = f.demo else {
        return Ok((0, Vec::new()));
    };
    let start = OVL1.rom_of(blob.vram).expect("demo blob outside ovl1");
    let end = f.sub_motion_descs.0;
    assert!(
        start <= end && (end - start).is_multiple_of(4),
        "{}: demo blob",
        f.name
    );
    let data = rom.bytes(start, (end - start) as usize)?;
    let words = (0..data.len() / 4)
        .map(|i| be_u32(data, i * 4))
        .collect::<Result<_>>()?;
    Ok((blob.vram, words))
}
