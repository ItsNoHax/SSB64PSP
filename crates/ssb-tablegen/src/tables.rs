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
        read: Some(crate::sc1p_data::courses_bytes),
    },
    TableDesc {
        name: "Bonus2Trees",
        kind: Kind::U32,
        len: Len::Exact(6 * 4),
        doc: "`dSC1PBonusStagePlatformDescs`, then `dSC1PBonusStageBoardedPlatformDescs` (two words and two zeros).",
        read: Some(crate::sc1p_data::trees_bytes),
    },
    TableDesc {
        name: "Bonus2Bumpers",
        kind: Kind::U32,
        len: Len::Exact(12 * 2),
        doc: "`dSC1PBonusStageBumperDescs`: `dobjdesc`, `anim_joint` (0, 0: none).",
        read: Some(crate::sc1p_data::bumpers_bytes),
    },
    TableDesc {
        name: "CampaignBonusLabels",
        kind: Kind::U32,
        len: Len::Exact(58),
        doc: "`dSC1PStageClearBonusData`'s `offset`s.",
        read: Some(crate::sc1p_data::bonus_labels_bytes),
    },
    TableDesc {
        name: "CampaignAllyCameras",
        kind: Kind::F32,
        len: Len::Exact(12 * 6 * 6),
        doc: "`sc1PIntroGetFighterCObjDesc`'s `cobj_descs` eye and at points.",
        read: Some(crate::sc1p_data::ally_cameras_bytes),
    },
    TableDesc {
        name: "EndingCongra",
        kind: Kind::U32,
        len: Len::Exact(12 * 4),
        doc: "`dMNCongraPictures`: bottom file and offset, top file and offset.",
        read: Some(crate::ending_data::congra_bytes),
    },
    TableDesc {
        name: "StaffrollNameImages",
        kind: Kind::U32,
        len: Len::Exact(56),
        doc: "`dSCStaffrollNameAndJobSpriteInfo`'s `offset`s.",
        read: Some(crate::ending_data::name_images_bytes),
    },
    TableDesc {
        name: "StaffrollTextSprites",
        kind: Kind::U32,
        len: Len::Exact(74),
        doc: "`dSCStaffrollTextBoxSpriteInfo`'s `offset`s.",
        read: Some(crate::ending_data::text_sprites_bytes),
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
    TableDesc {
        name: "IfTrafficSprites",
        kind: Kind::U8,
        len: Len::Exact(9),
        doc: "`dIFCommonTrafficSpriteOffsets` as file 82 manifest indices, by colour id.",
        read: std_reader!(crate::ifcommon::traffic_sprites_bytes),
    },
    TableDesc {
        name: "IfTrafficColors",
        kind: Kind::U8,
        len: Len::Exact(3 * 9),
        doc: "`dIFCommonTrafficSpriteColorsR`, `G`, `B`.",
        read: std_reader!(crate::ifcommon::traffic_colors_bytes),
    },
    TableDesc {
        name: "IfGoColors",
        kind: Kind::U8,
        len: Len::Exact(2 * 2 * 3),
        doc: "`dIFCommonTrafficGoBacklight{R,G,B}`, `...GoShadow{R,G,B}`: by state, channel.",
        read: std_reader!(crate::ifcommon::go_colors_bytes),
    },
    TableDesc {
        name: "IfTraffic",
        kind: Kind::I16,
        len: Len::Exact(15 * 3),
        doc: "`dIFCommonTrafficSpriteData`: `x`, `y`, `color_id`.",
        read: std_reader!(crate::ifcommon::traffic_bytes),
    },
    TableDesc {
        name: "IfAnnounce",
        kind: Kind::I16,
        len: Len::Exact(IF_ANNOUNCE_ROWS * 3),
        doc: "`dIFCommonAnnounce{Go,SuddenDeath,TimeUp,Complete,GameSet,Failure}SpriteData`: \
              `x`, `y`, sprite (a manifest index).",
        read: std_reader!(crate::ifcommon::announce_bytes),
    },
    TableDesc {
        name: "IfPauseDecals",
        kind: Kind::I16,
        len: Len::Exact(14 * 9),
        doc: "`dIFCommonBattlePauseDecalsSpriteData`: sprite (file 197 manifest index), `x`, `y`, \
              `prim[3]`, `env[3]`.",
        read: std_reader!(crate::ifcommon::pause_decals_bytes),
    },
    TableDesc {
        name: "IfPauseBorder",
        kind: Kind::I16,
        len: Len::Exact(5 * 4),
        doc: "`dIFCommonBattlePauseBorderRectangle`: `ulx`, `uly`, `lrx`, `lry`.",
        read: std_reader!(crate::ifcommon::pause_border_bytes),
    },
    TableDesc {
        name: "TitleFire",
        kind: Kind::U32,
        len: Len::Exact(30),
        doc: "`dMNTitleFireSpriteOffsets`.",
        read: Some(crate::menu_data::title_fire_bytes),
    },
    TableDesc {
        name: "TitleFireColors",
        kind: Kind::U8,
        len: Len::Exact(7 * 3),
        doc: "`dMNTitleFireColors{R,G,B}`: by colour, channel.",
        read: Some(crate::menu_data::title_fire_colors_bytes),
    },
    TableDesc {
        name: "TitleSprites",
        kind: Kind::I32,
        len: Len::Exact(10 * 3),
        doc: "`dMNTitleCommonSpriteDescs` (US): `x`, `y`, `offset`.",
        read: Some(crate::menu_data::title_sprites_bytes),
    },
    TableDesc {
        name: "CharactersSpecial",
        kind: Kind::U32,
        len: Len::Exact(12 * 3 * 8 * 3),
        doc: "`dMNCharactersSpecialMotion*` by `FTKind`, motion, track: `status_id`, \
              `anim_length`, `flags`.",
        read: Some(crate::menu_data::characters_special_bytes),
    },
    TableDesc {
        name: "CharactersCommon",
        kind: Kind::U32,
        len: Len::Exact(39 * 8 * 3),
        doc: "`dMNCharactersCommonMotionDescs` by motion kind, track.",
        read: Some(crate::menu_data::characters_common_bytes),
    },
    TableDesc {
        name: "CharactersAttack1",
        kind: Kind::U32,
        len: Len::Exact(12 * 8 * 3),
        doc: "`dMNCharactersAttack1MotionDescs` by `FTKind`, track.",
        read: Some(crate::menu_data::characters_attack1_bytes),
    },
    TableDesc {
        name: "CharactersAerial",
        kind: Kind::U32,
        len: Len::Exact(2 * 9 * 3),
        doc: "`dMNCharactersKirbyJumpAerialMotionDescs` and `...KirbyFallMotionDesc`, then Purin's.",
        read: Some(crate::menu_data::characters_aerial_bytes),
    },
    TableDesc {
        name: "PlayersDarkDigitWidths",
        kind: Kind::F32,
        len: Len::Exact(10),
        doc: "`llMNPlayersCommon0DarkSprite` to `...9DarkSprite`'s `width`.",
        read: Some(crate::menu_data::dark_digit_widths_bytes),
    },
    TableDesc {
        name: "StageClearPoints",
        kind: Kind::I32,
        len: Len::Exact(58),
        doc: "`dSC1PStageClearBonusData`'s `points`.",
        read: Some(crate::sc1p_data::bonus_points_bytes),
    },
    TableDesc {
        name: "SC1PComputers",
        kind: Kind::U8,
        len: Len::Exact(18 * 22),
        doc: "`dSC1PGameComputerDesc`: each `SC1PGameComputer`'s 22 bytes.",
        read: Some(crate::sc1p_data::computers_bytes),
    },
    TableDesc {
        name: "SC1PStages",
        kind: Kind::U8,
        len: Len::Exact(18 * 8),
        doc: "`dSC1PGameStageDesc` without `item_toggles` (`sc1p_data::STAGE_ROW`).",
        read: Some(crate::sc1p_data::stages_bytes),
    },
    TableDesc {
        name: "BossCometColors",
        kind: Kind::U8,
        len: Len::Exact(7 * 3),
        doc: "`dSC1PGameBossCometEnvColor{R,G,B}`: by colour, channel.",
        read: Some(crate::sc1p_data::boss_comet_colors_bytes),
    },
    TableDesc {
        name: "BossWallpapers",
        kind: Kind::U32,
        len: Len::Exact(4 * 35),
        doc: "`dSC1PGameBossWallpapers` and their effects, animations and plans \
              (`sc1p_data::BOSS_ROW`).",
        read: Some(crate::sc1p_data::boss_wallpapers_bytes),
    },
    TableDesc {
        name: "StaffrollNameGlyphs",
        kind: Kind::U8,
        len: Len::Exact(56 * 2),
        doc: "`dSCStaffrollNameAndJobSpriteInfo`'s `width`, `height`.",
        read: Some(crate::ending_data::name_glyphs_bytes),
    },
    TableDesc {
        name: "StaffrollTextGlyphs",
        kind: Kind::U8,
        len: Len::Exact(74 * 2),
        doc: "`dSCStaffrollTextBoxSpriteInfo`'s `width`, `height`.",
        read: Some(crate::ending_data::text_glyphs_bytes),
    },
    TableDesc {
        name: "OpeningCameras",
        kind: Kind::F32,
        len: Len::Exact(8 * 2 * 7),
        doc: "`dMVOpening*CObjDescStart` and `...End` of the eight fighter scenes: eye, at, `up.x`.",
        read: Some(crate::opening_data::cameras_bytes),
    },
    TableDesc {
        name: "OpeningKeys",
        kind: Kind::U16,
        len: Len::Exact(crate::opening_data::KEY_HALFWORDS),
        doc: "`dMVOpening*KeyEvents` of the eight fighter scenes, then \
              `dMVOpeningJungle{Donkey,Samus}KeyEvents`.",
        read: Some(crate::opening_data::keys_bytes),
    },
    TableDesc {
        name: "ItemAttributes",
        kind: Kind::I32,
        len: Len::Exact(crate::item_sounds::ITEMS.len() * crate::item_attributes::WORDS),
        doc: "Each `ll*ItemAttributes` record (`item_sounds::ITEMS`) as `item_attributes::WORDS` field words, sounds and `spin_speed` included.",
        read: Some(crate::item_attributes::attributes_bytes),
    },
    TableDesc {
        name: "ItemAttackEvents",
        kind: Kind::I32,
        len: Len::Exact(8 * 4 * 4),
        doc: "`item_attributes::ATTACK_EVENTS`' four `ITAttackEvent`s each: `timer, angle, damage, size`.",
        read: Some(crate::item_attributes::attack_events_bytes),
    },
    TableDesc {
        name: "ContainerVelocitiesY",
        kind: Kind::F32,
        len: Len::Exact(20),
        doc: "`llITCommonDataContainerVelocitiesY`.",
        read: Some(crate::item_attributes::container_velocities_bytes),
    },
    TableDesc {
        name: "LinkBombBloatScales",
        kind: Kind::F32,
        len: Len::Exact(6),
        doc: "`llLinkMainBombBloatScales`.",
        read: Some(crate::item_attributes::bloat_scales_bytes),
    },
    TableDesc {
        name: "WeaponSounds",
        kind: Kind::U16,
        len: Len::Exact(crate::item_sounds::WEAPONS.len()),
        doc: "`WPAttributes::sfx` per `item_sounds::WEAPONS` record.",
        read: Some(crate::item_sounds::weapon_sounds_bytes),
    },
    TableDesc {
        name: "MonsterEventSounds",
        kind: Kind::U16,
        len: Len::Exact(2 * 2),
        doc: "`ITMonsterEvent::fgm_id` of both events per `item_sounds::MONSTER_EVENTS` table.",
        read: Some(crate::item_sounds::monster_event_sounds_bytes),
    },
    TableDesc {
        name: "WeaponAttributes",
        kind: Kind::I32,
        len: Len::Exact(crate::item_sounds::WEAPONS.len() * crate::monster_weapons::WORDS),
        doc: "Each `ll*WeaponAttributes` record (`item_sounds::WEAPONS`) as `monster_weapons::WORDS` field words.",
        read: Some(crate::monster_weapons::attributes_bytes),
    },
    TableDesc {
        name: "SmogScale",
        kind: Kind::F32,
        len: Len::Exact(crate::monster_weapons::SMOG_PLAYS),
        doc: "The Smog's child `DObj` X scale after each play.",
        read: Some(crate::monster_weapons::smog_scale_bytes),
    },
    TableDesc {
        name: "ThrowDescs",
        kind: Kind::I32,
        len: Len::Exact(40 * 2 * 7),
        doc: "`throw_descs::DESCS`' two `FTThrowHitDesc`s each: `status` index, `damage, angle, kb_scale, kb_weight, kb_base, element`.",
        read: Some(crate::throw_descs::descs_bytes),
    },
    TableDesc {
        name: "SpecialColls",
        kind: Kind::U32,
        len: Len::Exact(3 * 9),
        doc: "`throw_descs::SPECIAL_COLLS`: `kind, joint`, `offset`, `size` (f32 bits), `damage_resist`.",
        read: Some(crate::throw_descs::special_colls_bytes),
    },
    TableDesc {
        name: "ItemThrowDescs",
        kind: Kind::U32,
        len: Len::Exact(22 * 4),
        doc: "`dFTCommonDataItemThrowDescs`: `is_smash`, `velocity` (f32 bits), `angle`, `damage_scale` (f32 bits).",
        read: Some(crate::battle_data::item_throws_bytes),
    },
    TableDesc {
        name: "GroundHitAttacks",
        kind: Kind::I32,
        len: Len::Exact(6 * 7),
        doc: "`dFTMainGroundHitCollisionAttributes`: `kind, damage, angle, kb_scale, kb_weight, kb_base, element`.",
        read: Some(crate::battle_data::ground_hits_bytes),
    },
    TableDesc {
        name: "ChargeShot",
        kind: Kind::U32,
        len: Len::Exact(8 * 6),
        doc: "`dWPSamusChargeShotWeaponAttributes`: `gfx_size, vel_x, damage, attack_size, coll_size` (floats as bits), `priority`.",
        read: Some(crate::battle_data::charge_shot_bytes),
    },
    TableDesc {
        name: "KirbyCopy",
        kind: Kind::U32,
        len: Len::Exact(27 * 4),
        doc: "`FTKirbyCopy` per swallowed `FTKind`: `copy_id, copy_modelpart_id, star_damage`, `effect_scale` (f32 bits).",
        read: Some(crate::kirby_copy::copy_bytes),
    },
    TableDesc {
        name: "KirbyAttack100",
        kind: Kind::F32,
        len: Len::Exact(6 * 6),
        doc: "`ftKirbyAttack100Effect[6]`: `offset[3], rotate, vel, add`.",
        read: Some(crate::kirby_attack100::effect_bytes),
    },
    TableDesc {
        name: "YoshiEgg",
        kind: Kind::F32,
        len: Len::Exact(27 * 7),
        doc: "`dFTCommonYoshiEggDamageCollDescs`: `effect_size, offset[3], size[3]` per `FTKind`.",
        read: Some(crate::battle_data::yoshi_egg_bytes),
    },
    TableDesc {
        name: "CaptureCaptain",
        kind: Kind::I16,
        len: Len::Exact(27 * 2),
        doc: "`dCaptainMainMotion_0x0000`: `(x, y)` per victim `FTKind`.",
        read: Some(crate::capture_captain::offsets_bytes),
    },
    TableDesc {
        name: "CostumeRoyal",
        kind: Kind::U8,
        len: Len::Exact(12 * 4),
        doc: "`FTCostume::royal` of the playable `FTKind`s.",
        read: Some(crate::battle_data::costume_royal_bytes),
    },
    TableDesc {
        name: "CostumeTeam",
        kind: Kind::U8,
        len: Len::Exact(12 * 3),
        doc: "`FTCostume::team` of the playable `FTKind`s.",
        read: Some(crate::battle_data::costume_team_bytes),
    },
    TableDesc {
        name: "Handicap",
        kind: Kind::F32,
        len: Len::Exact(40 * 2),
        doc: "`dFTCommonDataHandicapTable`.",
        read: Some(crate::battle_data::handicap_bytes),
    },
    TableDesc {
        name: "ShieldColors",
        kind: Kind::U8,
        len: Len::Exact(5 * 6),
        doc: "`dEFManagerShieldColors`: `prim[3], env[3]`.",
        read: Some(crate::battle_data::shield_colors_bytes),
    },
    TableDesc {
        name: "BossWait",
        kind: Kind::U8,
        len: Len::Exact(0x50),
        doc: "`ftbosswait.o`'s `.data` and the 16 bytes after it.",
        read: Some(crate::battle_data::boss_wait_bytes),
    },
    TableDesc {
        name: "EffectLife",
        kind: Kind::U16,
        len: Len::Exact(4 + 3),
        doc: "The quakes' and the Thunder Shock spark's animation lives.",
        read: Some(crate::effect_life::lives_bytes),
    },
    TableDesc {
        name: "ZebesAcid",
        kind: Kind::U32,
        len: Len::Exact(16 * 4),
        doc: "`dGRZebesAcidAttributes`: `wait_base, random_min, random_max, level` (f32 bits).",
        read: Some(crate::battle_data::zebes_acid_bytes),
    },
    TableDesc {
        name: "SectorMapX",
        kind: Kind::I16,
        len: Len::Exact(3),
        doc: "`dGRSectorArwingMapPositionsX`.",
        read: Some(crate::battle_data::sector_map_x_bytes),
    },
    TableDesc {
        name: "SectorLasers",
        kind: Kind::U8,
        len: Len::Exact(8),
        doc: "`dGRSectorArwingLaserCounts`.",
        read: Some(crate::battle_data::sector_lasers_bytes),
    },
    TableDesc {
        name: "SectorPilots",
        kind: Kind::U8,
        len: Len::Exact(56),
        doc: "`dGRSectorArwingPilotIDs`.",
        read: Some(crate::battle_data::sector_pilots_bytes),
    },
    TableDesc {
        name: "SectorPilotWindows",
        kind: Kind::U8,
        len: Len::Exact(6 * 2),
        doc: "`dGRSectorArwingPilotWaitTimers`: `(first, count)` pairs.",
        read: Some(crate::battle_data::sector_windows_bytes),
    },
    TableDesc {
        name: "AltWarning",
        kind: Kind::I16,
        len: Len::Exact(41),
        doc: "`MPGroundData::alt_warning` by `GRKind`.",
        read: Some(crate::ground::alt_warning_bytes),
    },
    TableDesc {
        name: "GroundEffectLife",
        kind: Kind::U16,
        len: Len::Exact(24),
        doc: "Each `EFGroundDesc`'s child `DObj` script's end play.",
        read: Some(crate::ground_effect::life_bytes),
    },
    TableDesc {
        name: "GroundEffectEnd",
        kind: Kind::F32,
        len: Len::Exact(24 * 2 * 2),
        doc: "Each `EFGroundDesc`'s child `translate.x, .y` at that play: first, later cycles.",
        read: Some(crate::ground_effect::end_bytes),
    },
    TableDesc {
        name: "StatFlags",
        kind: Kind::U16,
        len: Len::Exact(crate::stat_flags::FLAG_COUNT),
        doc: "`FTStatusDesc::sflags`: the common statuses, then each playable fighter's specials.",
        read: Some(crate::stat_flags::flags_bytes),
    },
    TableDesc {
        name: "DamageColls",
        kind: Kind::U32,
        len: Len::Exact(27 * crate::hurtbox::FIGHTER_WORDS),
        doc: "Per `FTKind`: `FTAttributes::damage_coll_descs`' used-row count, then 11 rows of `joint, placement, is_grabbable, offset[3], size[3]` (`f32` bits, full size).",
        read: Some(crate::hurtbox::bytes),
    },
    TableDesc {
        name: "JointMasks",
        kind: Kind::U32,
        len: Len::Exact(27 * crate::model_parts::JOINT_MASK_WORDS),
        doc: "Per `FTKind`: `setup_parts`, `setup_parts & has a display list`, `animlock`, each as (low, high) words of a 64-bit joint mask.",
        read: Some(crate::model_parts::joint_masks_bytes),
    },
    TableDesc {
        name: "HiddenParts",
        kind: Kind::U8,
        len: Len::Exact(27 * crate::model_parts::HIDDEN_PART_BYTES),
        doc: "Per `FTKind`: the count, then `(index, root joint, has a display list)` per `hiddenparts` entry rooted at joint 4 or later.",
        read: Some(crate::model_parts::hidden_parts_bytes),
    },
    TableDesc {
        name: "TextureParts",
        kind: Kind::U8,
        len: Len::Exact(27 * crate::model_parts::TEXTURE_PART_BYTES),
        doc: "Per `FTKind`: present, then `(joint, high MObj, low MObj)` per `textureparts` entry, pad.",
        read: Some(crate::model_parts::texture_parts_bytes),
    },
    TableDesc {
        name: "TexturePartMade",
        kind: Kind::U8,
        len: Len::Exact(27 * crate::model_parts::TEXTURE_PART_MADE_BYTES),
        doc: "Per `FTKind` and texture part: the made `TextureIDCurrent` at high and low detail (`i8`), has an `FTModelPartDesc`; pad.",
        read: Some(crate::model_parts::texture_part_made_bytes),
    },
    TableDesc {
        name: "MarioPhysics",
        kind: Kind::U32,
        len: Len::Exact(crate::physics::WORDS),
        doc: "Mario's `FTAttributes` physics head in `physics::FIELDS` order (`f32` bits; `jumps_max` an `s32`), then `map_coll`.",
        read: Some(crate::physics::bytes),
    },
    TableDesc {
        name: "ColAnimEvents",
        kind: Kind::U32,
        len: Len::Exact(crate::colanim::EVENTS * crate::colanim::EVENT_WORDS),
        doc: "Every colour-animation event: its ROM words (a script pointer as the script's index), padded to 4.",
        read: Some(crate::colanim::events_bytes),
    },
    TableDesc {
        name: "ColAnimScripts",
        kind: Kind::U16,
        len: Len::Exact(crate::colanim::SCRIPTS.len() + 1),
        doc: "Each colour-animation script's first event in `ColAnimEvents`, then the event count.",
        read: Some(crate::colanim::scripts_bytes),
    },
    TableDesc {
        name: "ColAnimDescs",
        kind: Kind::U8,
        len: Len::Exact(crate::colanim::DESCS.1 * crate::colanim::DESC_BYTES),
        doc: "`dGMColScriptsDescs`: script index + 1 (0 for none), `priority`, `is_unlocked`, pad.",
        read: Some(crate::colanim::descs_bytes),
    },
    TableDesc {
        name: "InputScripts",
        kind: Kind::U8,
        len: Len::Exact(crate::computer::INPUT_SCRIPT_BYTES),
        doc: "`dFTComputerPlayerInputScripts`' scripts back to back, in pointer-table order.",
        read: Some(crate::computer::input_scripts_bytes),
    },
    TableDesc {
        name: "InputScriptOffsets",
        kind: Kind::U16,
        len: Len::Exact(crate::computer::INPUT_SCRIPT_TABLE.1 + 1),
        doc: "Each input script's first byte in `InputScripts`, then the byte count.",
        read: Some(crate::computer::input_script_offsets_bytes),
    },
    TableDesc {
        name: "ComputerAttacks",
        kind: Kind::U32,
        len: Len::Exact(12 * crate::computer::FIGHTER_ATTACK_WORDS),
        doc: "Per playable `FTKind`: the grounded and aerial row counts, then 11 grounded and 7 aerial `FTComputerAttack` rows (7 ROM words each).",
        read: Some(crate::computer::attacks_bytes),
    },
];

/// `ifcommon::ANNOUNCE_ROWS` (layout).
pub const IF_ANNOUNCE_ROWS: usize = 44;

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
    assert!(crate::sc1p_data::BONUS_BUMPERS.1 == 12);
    assert!(crate::sc1p_data::STAGE_CLEAR_BONUS.1 == 58);
    assert!(crate::sc1p_data::ALLY_CAMERAS.1 == 12 && crate::sc1p_data::ALLY_CAMERAS.2 == 6);
    assert!(crate::ending_data::CONGRA_PICTURES.1 == 12);
    assert!(crate::ending_data::NAME_AND_JOB.1 == 56);
    assert!(crate::ending_data::TEXT_BOX.1 == 74);
    assert!(crate::ifcommon::ANNOUNCE_ROWS == IF_ANNOUNCE_ROWS);
    assert!(crate::ifcommon::TRAFFIC_OFFSETS.1 == 9);
    assert!(crate::ifcommon::TRAFFIC_COLOR_COUNT == 9);
    assert!(crate::ifcommon::TRAFFIC_DATA.1 == 15);
    assert!(crate::ifcommon::PAUSE_DECALS.1 == 14 && crate::ifcommon::DECAL_ROW == 9);
    assert!(crate::ifcommon::PAUSE_BORDER.1 == 5);
};

// The no_std generators' counts.
const _: () = {
    use crate::{ending_data as e, menu_data as m, opening_data as o, sc1p_data as s};
    assert!(m::TITLE_FIRE_OFFSETS.1 == 30 && m::TITLE_FIRE_COLOR_COUNT == 7);
    assert!(m::TITLE_SPRITE_DESCS.1 == 10);
    assert!(m::SPECIAL_MOTIONS.1 == 12 && m::TRACKS == 8);
    assert!(m::COMMON_MOTIONS.1 == 39 && m::ATTACK1_MOTIONS.1 == 12);
    assert!(m::DARK_DIGIT_OFFSETS.1 == 10);
    assert!(s::STAGE_CLEAR_BONUS.1 == 58 && s::TREE_COUNT == 6);
    assert!(s::GAME_COMPUTER_DESC.1 == 18 && s::GAME_STAGE_DESC.1 == 18);
    assert!(s::STAGE_ROW == 8 && s::BOSS_COMET_COLOR_COUNT == 7);
    assert!(s::BOSS_WALLPAPERS.1 == 4 && s::BOSS_ROW == 35);
    assert!(e::NAME_AND_JOB.1 == 56 && e::TEXT_BOX.1 == 74);
    assert!(o::SCENES.len() == 8);
    use crate::{battle_data as b, item_attributes as ia, item_sounds as is};
    assert!(ia::ATTACK_EVENTS.len() == 8 && ia::ATTACK_EVENT_COUNT == 4);
    assert!(ia::CONTAINER_VELOCITIES_Y.2 == 20 && ia::LINK_BOMB_BLOAT_SCALES.2 == 6);
    assert!(is::MONSTER_EVENTS.len() == 2);
    assert!(crate::throw_descs::DESCS.len() == 40 && crate::throw_descs::SPECIAL_COLLS.len() == 3);
    assert!(b::ITEM_THROW_DESCS.1 == 22 && b::GROUND_HIT_ATTACKS.1 == 6);
    assert!(b::CHARGE_SHOT_ATTRIBUTES.1 == 8 && b::YOSHI_EGG_DESCS.1 == 27);
    assert!(crate::kirby_copy::ROWS == 27 && crate::kirby_copy::PLAYABLE == 12);
    assert!(crate::kirby_attack100::ROWS == 6 && crate::kirby_attack100::COLS == 6);
    assert!(crate::capture_captain::ROWS == 27 && b::COSTUME_IDS.1 == 12);
    assert!(b::HANDICAP_TABLE.1 == 40 && b::SHIELD_COLORS.1 == 5 && b::BOSS_WAIT_DATA.1 == 0x50);
    assert!(
        crate::effect_life::QUAKE_TABLES.len() == 4 && crate::effect_life::SHOCK_TABLES.len() == 3
    );
    assert!(b::ZEBES_ACID.1 == 16 && b::SECTOR_MAP_POSITIONS_X.1 == 3);
    assert!(b::SECTOR_LASER_COUNTS.1 == 8 && b::SECTOR_PILOT_IDS.1 == 56);
    assert!(b::SECTOR_PILOT_WINDOWS.1 == 6 && crate::ground::GROUND_FILE_INFOS.len() == 41);
    assert!(crate::ground_effect::DESC_COUNT == 24);
    assert!(crate::fighters::FIGHTERS.len() == 27 && crate::computer::FIGHTERS.len() == 12);
    assert!(crate::colanim::SCRIPTS.len() == 93 && crate::colanim::DESCS.1 == 86);
    assert!(crate::colanim::EVENTS == 691 && crate::computer::INPUT_SCRIPT_TABLE.1 == 49);
    assert!(crate::computer::INPUT_SCRIPT_BYTES == 429 && crate::physics::WORDS == 28);
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
