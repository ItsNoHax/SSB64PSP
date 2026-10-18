//! 1P Game scene data arrays (`d*` symbols in the scene overlays): the stage
//! clear's bonus table (`ovl56`, `sc/sc1pmode/sc1pstageclear.c`), the
//! per-stage computer and stage setups (`ovl65`, `sc1pgame.c`) and Final
//! Destination's boss wallpapers (`ovl65`, `sc1pgameboss.c`).
//!
//! Each table's run-time address is its symbol in
//! `refs/ssb-decomp-re/symbols/symbols_us.txt`; the overlay is the one the
//! defining file's `.data` sits in (`smashbrothers.us.yaml`).

use std::fmt::Write;

use crate::emit::{self, f32_lit};
use crate::source::{OverlayData, OVL24, OVL56, OVL6, OVL65};
use crate::{Generated, Result, Source};

/// `dSC1PStageClearBonusData` (`SC1PStageClearScore[58]`: `intptr_t offset`,
/// the bonus text's sprite in `llSC1PStageClear1FileID`; `s32 points`), in
/// bonus ID order, unused IDs included.
pub const STAGE_CLEAR_BONUS: (u32, usize) = (0x8013_4EFC, 58);
const STAGE_CLEAR_BONUS_SIZE: u32 = 8;
/// `sc1PIntroGetFighterCObjDesc`'s local `cobj_descs[12][6]` (`CObjDesc`,
/// 28 bytes: `Vec3f eye, at; f32 upx`; its address is the comment above the
/// function in `sc1pintro.c`, in `ovl24`'s `.data`), by `FTKind` then camera.
pub const ALLY_CAMERAS: (u32, usize, usize) = (0x8013_51F0, 12, 6);
const COBJ_DESC_SIZE: u32 = 28;
/// `dSC1PBonusStageTargetDescs` (`GRBonusTarget[12]`: `intptr_t start,
/// dobjdesc, anim_joint`), by `FTKind` Mario to Ness (`ovl6`).
pub const BONUS_TARGETS: (u32, usize) = (0x8018_EEC4, 12);
const BONUS_TARGET_SIZE: u32 = 12;
/// `dSC1PBonusStageBumperDescs` (`intptr_t[12][2]`: `dobjdesc`, `anim_joint`;
/// both 0 for a kind with no bumpers).
pub const BONUS_BUMPERS: (u32, usize) = (0x8018_EF54, 12);
const BONUS_BUMPER_SIZE: u32 = 8;
/// `dSC1PBonusStagePlatformDescs` (`intptr_t[3][4]`: `DObjDesc`, `AnimJoint`,
/// `MObjSub`, `MatAnimJoint`), small to large.
pub const BONUS_PLATFORMS: (u32, usize) = (0x8018_EFB4, 3);
const BONUS_PLATFORM_SIZE: u32 = 16;
/// `dSC1PBonusStageBoardedPlatformDescs` (`intptr_t[3][2]`: `DObjDesc`,
/// `AnimJoint`), small to large.
pub const BONUS_BOARDED: (u32, usize) = (0x8018_EFE4, 3);
const BONUS_BOARDED_SIZE: u32 = 8;
/// `dSC1PGameComputerDesc` (`SC1PGameComputer[18]`, 22 bytes: `ub8
/// is_team_attack; u8 item_appearance_rate; u8 enemy_level[5],
/// enemy_handicap[5], ally_level[5], ally_handicap[5]`), by 1P Game stage.
pub const GAME_COMPUTER_DESC: (u32, usize) = (0x8019_2830, 18);
const GAME_COMPUTER_SIZE: u32 = 22;
/// `dSC1PGameStageDesc` (`SC1PGameStage[18]`, 16 bytes: `u8 screenflash_alpha,
/// gkind; u32 item_toggles; u8 opponent_count, fkind[2], opponent_behavior,
/// ally_count, ally_behavior`).
pub const GAME_STAGE_DESC: (u32, usize) = (0x8019_29BC, 18);
const GAME_STAGE_SIZE: u32 = 16;

/// `nSCBattleItemSwitch*` in enum order (`SC1PGameComputer::item_appearance_rate`).
const ITEM_APPEARANCE: [&str; 6] = ["None", "VeryLow", "Low", "Middle", "High", "VeryHigh"];
/// `nFTKind*` in enum order, by the port's names; `nFTKindNull` (0x1C) is `None`.
const FIGHTER_KINDS: [&str; 27] = [
    "Mario",
    "Fox",
    "Donkey",
    "Samus",
    "Luigi",
    "Link",
    "Yoshi",
    "Captain",
    "Kirby",
    "Pikachu",
    "Purin",
    "Ness",
    "Boss",
    "MetalMario",
    "PolyMario",
    "PolyFox",
    "PolyDonkey",
    "PolySamus",
    "PolyLuigi",
    "PolyLink",
    "PolyYoshi",
    "PolyCaptain",
    "PolyKirby",
    "PolyPikachu",
    "PolyPurin",
    "PolyNess",
    "GiantDonkey",
];
const FTKIND_NULL: u8 = 0x1C;
/// `nFTComputerTrait*` in enum order, by the port's `Trait` names.
const COMPUTER_TRAITS: [&str; 11] = [
    "Default",
    "Link",
    "YoshiTeam",
    "KirbyTeam",
    "PolyTeam",
    "MarioBros",
    "GiantDonkey",
    "Unk1",
    "Bonus3",
    "Ally",
    "None",
];
/// The only value every stage's `item_toggles` holds; the port does not
/// keep the field.
const ALL_ITEMS: u32 = 0xFFFF_FFFF;

/// `dSC1PGameBossCometEnvColorR`, `...G` and `...B` (`u8[7]`).
pub const BOSS_COMET_COLORS: [u32; 3] = [0x8019_2BC0, 0x8019_2BC8, 0x8019_2BD0];
pub const BOSS_COMET_COLOR_COUNT: u32 = 7;
/// `dSC1PGameBossWallpapers` (`SC1PGameBossWallpaper[4]`, 48 bytes: nine
/// words (`loop_count`, the effect, anim and plan counts, `dobj_color_id`,
/// `color_id`, `change_wait_base`, `change_damage_min`, `is_random_wallpaper`)
/// and the `SC1PGameBossEffect`, `SC1PGameBossAnim` and `SC1PGameBossPlan`
/// array pointers).
pub const BOSS_WALLPAPERS: (u32, usize) = (0x8019_2D4C, 4);
const BOSS_WALLPAPER_SIZE: u32 = 48;

/// One wallpaper's `dSC1PGameBossEffects*` (`SC1PGameBossEffect`, 16 bytes:
/// `proc_update`, `proc_display`, `o_dobjdesc`, `o_mobjsub`),
/// `dSC1PGameBossAnims*` (`SC1PGameBossAnim`, 12 bytes: `o_anim_joint`,
/// `o_matanim_joint`, `f32 anim_speed`) and `dSC1PGameBossPlans*`
/// (`SC1PGameBossPlan`, 24 bytes: `s32 count; u8 dl_link; u32 camera_tag;
/// Vec3f pos`), with the counts the C source takes by `ARRAY_COUNT`. The
/// wallpaper's header must name these arrays and counts.
pub struct BossWallpaper {
    pub effects: (u32, usize),
    pub anims: (u32, usize),
    pub plans: (u32, usize),
}
const EFFECT_SIZE: u32 = 16;
const ANIM_SIZE: u32 = 12;
const PLAN_SIZE: u32 = 24;
pub const BOSS: [BossWallpaper; 4] = [
    BossWallpaper {
        effects: (0x8019_2BD8, 1),
        anims: (0x8019_2BE8, 4),
        plans: (0x8019_2C18, 2),
    },
    BossWallpaper {
        effects: (0x8019_2C48, 1),
        anims: (0x8019_2C58, 1),
        plans: (0x8019_2C64, 1),
    },
    BossWallpaper {
        effects: (0x8019_2C7C, 2),
        anims: (0x8019_2C9C, 2),
        plans: (0x8019_2CB4, 2),
    },
    BossWallpaper {
        effects: (0x8019_2CE4, 2),
        anims: (0x8019_2D04, 2),
        plans: (0x8019_2D1C, 2),
    },
];
/// The effects' `proc_update` functions (`SC1PGameBossWallpaper*ProcUpdate*`)
/// and the port's `Update` for each.
const BOSS_UPDATES: [(u32, &str); 6] = [
    (0x8019_1D60, "Comet"),
    (0x8019_1DA4, "Fall"),
    (0x8019_1E28, "Grow"),
    (0x8019_1F28, "Late"),
    (0x8019_1AEC, "Plain"),
    (0x8019_1F90, "Closing"),
];
/// The `proc_display` functions (`SC1PGameBossWallpaper*ProcDisplay*`) and
/// the port's `Display`.
const BOSS_DISPLAYS: [(u32, &str); 4] = [
    (0x8019_1364, "Comet"),
    (0x8019_1498, "Fall"),
    (0x8019_15B8, "Translucent"),
    (0x8019_16A8, "TranslucentDirect"),
];
/// `SC1PGameBossPlan::dl_link` of every plan (`DL_LINK`).
const BOSS_DL_LINK: u8 = 5;

pub fn generate(rom: Option<&Source>) -> Result<Vec<Generated>> {
    let ovl56 = OverlayData::new(rom, OVL56);
    let ovl65 = OverlayData::new(rom, OVL65);
    Ok(vec![
        Generated {
            name: "stage_clear_bonus.rs",
            text: bonus_points(ovl56)?,
        },
        Generated {
            name: "sc1p_game_setup.rs",
            text: game_setup(ovl65)?,
        },
        Generated {
            name: "sc1p_boss.rs",
            text: boss(ovl65)?,
        },
    ])
}

/// `ssb-rom`'s table from the same array: the bonus text sprites' offsets.
pub fn generate_rom(rom: Option<&Source>) -> Result<Vec<Generated>> {
    let d = OverlayData::new(rom, OVL56);
    let (at, n) = STAGE_CLEAR_BONUS;
    let mut w = emit::header("`dSC1PStageClearBonusData`'s `offset`s");
    let offsets = (0..n as u32)
        .map(|i| d.u32(at + i * STAGE_CLEAR_BONUS_SIZE))
        .collect::<Result<Vec<_>>>()?;
    let _ = writeln!(w, "pub const BONUS_LABELS: [u32; {n}] = [");
    emit::hex_rows_u32(&mut w, &offsets);
    w.push_str("];\n");
    let mut files = vec![Generated {
        name: "campaign_bonus_labels.rs",
        text: w,
    }];
    files.push(ally_cameras(rom)?);
    files.extend(bonus_stage_layout(rom)?);
    Ok(files)
}

/// The 1P Game's intro cameras, `ssb-rom`'s `campaign::ALLY_CAMERAS`: each
/// `CObjDesc`'s eye and at points.
fn ally_cameras(rom: Option<&Source>) -> Result<Generated> {
    let d = OverlayData::new(rom, OVL24);
    let (at, kinds, cameras) = ALLY_CAMERAS;
    let mut w = emit::header("`sc1PIntroGetFighterCObjDesc`'s `cobj_descs` eye and at points");
    let _ = writeln!(
        w,
        "pub const ALLY_CAMERAS: [[[f32; 6]; {cameras}]; {kinds}] = ["
    );
    for k in 0..kinds as u32 {
        w.push_str("    [\n");
        for c in 0..cameras as u32 {
            let at = at + (k * cameras as u32 + c) * COBJ_DESC_SIZE;
            let cells = (0..6)
                .map(|i| d.f32(at + i * 4).map(f32_lit))
                .collect::<Result<Vec<_>>>()?;
            let _ = writeln!(w, "        [{}],", cells.join(", "));
        }
        w.push_str("    ],\n");
    }
    w.push_str("];\n");
    Ok(Generated {
        name: "campaign_ally_cameras.rs",
        text: w,
    })
}

/// The bonus stages' tables in `ovl6`, `ssb-rom`'s `bonus1::COURSES` and
/// `bonus2::{TREES, BUMPERS}`.
fn bonus_stage_layout(rom: Option<&Source>) -> Result<Vec<Generated>> {
    let d = OverlayData::new(rom, OVL6);
    let words =
        |at: u32, n: u32| -> Result<Vec<u32>> { (0..n).map(|i| d.u32(at + i * 4)).collect() };
    let (at, n) = BONUS_TARGETS;
    let mut w = emit::header("`dSC1PBonusStageTargetDescs`");
    let _ = writeln!(w, "pub const COURSES: [Course; {n}] = [");
    for i in 0..n as u32 {
        let c = words(at + i * BONUS_TARGET_SIZE, 3)?;
        let _ = writeln!(
            w,
            "    Course {{\n        start: 0x{:X},\n        placements: 0x{:X},\n        scripts: 0x{:X},\n    }},",
            c[0], c[1], c[2]
        );
    }
    w.push_str("];\n");
    let courses = Generated {
        name: "bonus1_courses.rs",
        text: w,
    };

    let mut w = emit::header(
        "`dSC1PBonusStagePlatformDescs` and `dSC1PBonusStageBoardedPlatformDescs` (the boarded ones have no \
         MObj or material tables)",
    );
    let (pat, pn) = BONUS_PLATFORMS;
    let (bat, bn) = BONUS_BOARDED;
    let _ = writeln!(w, "pub const TREES: [[u32; 4]; {}] = [", pn + bn);
    for i in 0..pn as u32 {
        let t = words(pat + i * BONUS_PLATFORM_SIZE, 4)?;
        let _ = writeln!(
            w,
            "    [0x{:x}, 0x{:x}, 0x{:x}, 0x{:x}],",
            t[0], t[1], t[2], t[3]
        );
    }
    for i in 0..bn as u32 {
        let t = words(bat + i * BONUS_BOARDED_SIZE, 2)?;
        let _ = writeln!(w, "    [0x{:x}, 0x{:x}, 0, 0],", t[0], t[1]);
    }
    w.push_str("];\n");
    let trees = Generated {
        name: "bonus2_trees.rs",
        text: w,
    };

    let mut w = emit::header("`dSC1PBonusStageBumperDescs` (a kind with no bumpers holds 0, 0)");
    let (at, n) = BONUS_BUMPERS;
    let _ = writeln!(w, "pub const BUMPERS: [Option<(u32, u32)>; {n}] = [");
    for i in 0..n as u32 {
        let b = words(at + i * BONUS_BUMPER_SIZE, 2)?;
        if (b[0], b[1]) == (0, 0) {
            w.push_str("    None,\n");
        } else {
            let _ = writeln!(w, "    Some((0x{:x}, 0x{:x})),", b[0], b[1]);
        }
    }
    w.push_str("];\n");
    let bumpers = Generated {
        name: "bonus2_bumpers.rs",
        text: w,
    };
    Ok(vec![courses, trees, bumpers])
}

fn bonus_points(d: OverlayData) -> Result<String> {
    let (at, n) = STAGE_CLEAR_BONUS;
    let mut w = emit::header("`dSC1PStageClearBonusData`'s `points`");
    let _ = writeln!(w, "pub const BONUS_POINTS: [i32; {n}] = [");
    let points = (0..n as u32)
        .map(|i| d.i32(at + i * STAGE_CLEAR_BONUS_SIZE + 4))
        .collect::<Result<Vec<_>>>()?;
    for row in points.chunks(16) {
        let cells: Vec<String> = row.iter().map(|p| p.to_string()).collect();
        let _ = writeln!(w, "    {},", cells.join(", "));
    }
    w.push_str("];\n");
    Ok(w)
}

fn name_of(names: &[&'static str], what: &str, id: u8) -> &'static str {
    names
        .get(usize::from(id))
        .copied()
        .unwrap_or_else(|| panic!("{what} {id} is not one the port names"))
}

fn game_setup(d: OverlayData) -> Result<String> {
    let mut w = emit::header("`dSC1PGameComputerDesc` and `dSC1PGameStageDesc`");
    let (at, n) = GAME_COMPUTER_DESC;
    let _ = writeln!(w, "pub const COMPUTER_DESC: [ComputerDesc; {n}] = [");
    for i in 0..n as u32 {
        let b = d.bytes(at + i * GAME_COMPUTER_SIZE, GAME_COMPUTER_SIZE)?;
        let _ = writeln!(
            w,
            "    ComputerDesc {{\n        is_team_attack: {},\n        item_appearance: Appearance::{},\n        \
             enemy_level: {:?},\n        enemy_handicap: {:?},\n        ally_level: {:?},\n        ally_handicap: {:?},\n    }},",
            b[0] != 0,
            name_of(&ITEM_APPEARANCE, "item_appearance_rate", b[1]),
            &b[2..7],
            &b[7..12],
            &b[12..17],
            &b[17..22],
        );
    }
    w.push_str("];\n");
    let (at, n) = GAME_STAGE_DESC;
    let _ = writeln!(w, "pub const STAGE_DESC: [StageDesc; {n}] = [");
    for i in 0..n as u32 {
        let at = at + i * GAME_STAGE_SIZE;
        let b = d.bytes(at, GAME_STAGE_SIZE)?;
        if d.rom.is_some() {
            assert_eq!(
                d.u32(at + 4)?,
                ALL_ITEMS,
                "stage {i}'s item_toggles are not all items"
            );
        }
        let fkind = |id: u8| -> String {
            if id == FTKIND_NULL {
                "None".to_string()
            } else {
                format!(
                    "Some(FighterKind::{})",
                    name_of(&FIGHTER_KINDS, "fkind", id)
                )
            }
        };
        let _ = writeln!(
            w,
            "    StageDesc {{\n        screenflash_alpha: 0x{:02X},\n        gkind: {},\n        \
             opponent_count: {},\n        fkind: [{}, {}],\n        opponent_trait: Trait::{},\n        \
             ally_count: {},\n        ally_trait: Trait::{},\n    }},",
            b[0],
            b[1],
            b[8],
            fkind(b[9]),
            fkind(b[10]),
            name_of(&COMPUTER_TRAITS, "opponent_behavior", b[11]),
            b[12],
            name_of(&COMPUTER_TRAITS, "ally_behavior", b[13]),
        );
    }
    w.push_str("];\n");
    Ok(w)
}

/// The port's name for the function at `addr`, or the first in `names` for
/// a stub (which reads zeros).
fn function(d: OverlayData, names: &[(u32, &'static str)], what: &str, addr: u32) -> &'static str {
    if d.rom.is_none() {
        return names[0].1;
    }
    names
        .iter()
        .find(|(a, _)| *a == addr)
        .map(|(_, n)| *n)
        .unwrap_or_else(|| panic!("{what} 0x{addr:08X} is not one the port names"))
}

fn boss(d: OverlayData) -> Result<String> {
    let mut w = emit::header(
        "`dSC1PGameBossWallpapers` and the effect, animation and plan arrays it points to, \
         and `dSC1PGameBossCometEnvColor{R,G,B}`",
    );
    let mut colors = String::new();
    for i in 0..BOSS_COMET_COLOR_COUNT {
        let c: Vec<String> = BOSS_COMET_COLORS
            .iter()
            .map(|&a| d.u8(a + i).map(|v| format!("0x{v:02X}")))
            .collect::<Result<_>>()?;
        let _ = writeln!(colors, "    [{}],", c.join(", "));
    }
    let _ = write!(
        w,
        "pub const COMET_ENV_COLORS: [[u8; 3]; {BOSS_COMET_COLOR_COUNT}] = [\n{colors}];\n"
    );

    let (at, n) = BOSS_WALLPAPERS;
    assert_eq!(n, BOSS.len());
    let mut assets = String::new();
    let _ = writeln!(w, "pub const WALLPAPERS: [WallpaperDesc; {n}] = [");
    for (i, layout) in BOSS.iter().enumerate() {
        let at = at + i as u32 * BOSS_WALLPAPER_SIZE;
        if d.rom.is_some() {
            let counts = [
                d.u32(at + 4)?,
                d.u32(at + 8)?,
                d.u32(at + 12)?,
                d.u32(at + 36)?,
                d.u32(at + 40)?,
                d.u32(at + 44)?,
            ];
            let want = [
                layout.effects.1 as u32,
                layout.anims.1 as u32,
                layout.plans.1 as u32,
                layout.effects.0,
                layout.anims.0,
                layout.plans.0,
            ];
            assert_eq!(counts, want, "wallpaper {i}'s header names other arrays");
        }
        let (loop_count, dobj_color_id, color_id) = (d.i32(at)?, d.i32(at + 16)?, d.i32(at + 20)?);
        let (wait_base, damage_min) = (d.i32(at + 24)?, d.i32(at + 28)?);
        let is_random = d.u32(at + 32)? != 0;

        let mut effects = Vec::new();
        let mut asset_cells = Vec::new();
        for e in 0..layout.effects.1 as u32 {
            let at = layout.effects.0 + e * EFFECT_SIZE;
            let update = function(d, &BOSS_UPDATES, "proc_update", d.u32(at)?);
            let display = function(d, &BOSS_DISPLAYS, "proc_display", d.u32(at + 4)?);
            effects.push(format!("(Update::{update}, Display::{display})"));
            // The effect's own source in file 114, with the animation
            // paired with it (`bossanim[effect]`): the controller binds the
            // material animation only to an effect with materials.
            let (graph, mobjsub) = (d.u32(at + 8)?, d.u32(at + 12)?);
            let anim = layout.anims.0 + e.min(layout.anims.1 as u32 - 1) * ANIM_SIZE;
            let (anim_joint, matanim) = (d.u32(anim)?, d.u32(anim + 4)?);
            let some = |v: u32, keep: bool| {
                if keep && v != 0 {
                    format!("Some(0x{v:X})")
                } else {
                    "None".to_string()
                }
            };
            asset_cells.push(format!(
                "Some(EffectAsset {{\n            graph: 0x{graph:X},\n            mobjsub: {},\n            \
                 anim_joint: {},\n            matanim_joint: {},\n        }})",
                some(mobjsub, true),
                some(anim_joint, true),
                some(matanim, mobjsub != 0),
            ));
        }
        asset_cells.resize(2, "None".to_string());
        let _ = writeln!(
            assets,
            "    [\n        {},\n        {},\n    ],",
            asset_cells[0], asset_cells[1]
        );

        let mut speeds = Vec::new();
        for a in 0..layout.anims.1 as u32 {
            speeds.push(f32_lit(d.f32(layout.anims.0 + a * ANIM_SIZE + 8)?));
        }
        let mut plans = String::new();
        for p in 0..layout.plans.1 as u32 {
            let at = layout.plans.0 + p * PLAN_SIZE;
            if d.rom.is_some() {
                assert_eq!(d.u8(at + 4)?, BOSS_DL_LINK, "plan {p} of wallpaper {i}");
            }
            let _ = writeln!(
                plans,
                "            Plan {{\n                count: {},\n                camera_tag: {},\n                \
                 pos: Vec3::new({}, {}, {}),\n            }},",
                d.i32(at)?,
                d.u32(at + 8)?,
                f32_lit(d.f32(at + 12)?),
                f32_lit(d.f32(at + 16)?),
                f32_lit(d.f32(at + 20)?),
            );
        }
        let _ = writeln!(
            w,
            "    WallpaperDesc {{\n        loop_count: {loop_count},\n        effects: &[{}],\n        \
             anim_speeds: &[{}],\n        plans: &[\n{plans}        ],\n        dobj_color_id: {dobj_color_id},\n        \
             color_id: {color_id},\n        change_wait_base: {wait_base},\n        \
             change_damage_min: {damage_min},\n        is_random: {is_random},\n    }},",
            effects.join(", "),
            speeds.join(", "),
        );
    }
    w.push_str("];\n");
    let _ = write!(
        w,
        "pub const EFFECT_ASSETS: [[Option<EffectAsset>; 2]; {n}] = [\n{assets}];\n"
    );
    Ok(w)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stubs_have_every_row() {
        let files = generate(None).unwrap();
        assert_eq!(files.len(), 3);
        assert!(files[0].text.contains("pub const BONUS_POINTS: [i32; 58]"));
        assert_eq!(files[1].text.matches("    ComputerDesc {").count(), 18);
        assert_eq!(files[1].text.matches("    StageDesc {").count(), 18);
        let boss = &files[2].text;
        assert_eq!(boss.matches("    WallpaperDesc {").count(), 4);
        assert_eq!(boss.matches("            Plan {").count(), 2 + 1 + 2 + 2);
        assert_eq!(boss.matches("Some(EffectAsset {").count(), 1 + 1 + 2 + 2);
        let rom = generate_rom(None).unwrap();
        assert!(rom[0].text.contains("BONUS_LABELS: [u32; 58]"));
    }

    #[test]
    fn the_layout_arrays_are_contiguous() {
        // Each wallpaper's arrays follow the previous wallpaper's, and the
        // header table follows the last.
        let mut end = BOSS[0].effects.0;
        for b in &BOSS {
            assert_eq!(b.effects.0, end);
            assert_eq!(b.anims.0, b.effects.0 + b.effects.1 as u32 * EFFECT_SIZE);
            assert_eq!(b.plans.0, b.anims.0 + b.anims.1 as u32 * ANIM_SIZE);
            end = b.plans.0 + b.plans.1 as u32 * PLAN_SIZE;
        }
        assert_eq!(BOSS_WALLPAPERS.0, end);
        assert_eq!(
            GAME_COMPUTER_DESC.0 + GAME_COMPUTER_DESC.1 as u32 * GAME_COMPUTER_SIZE,
            GAME_STAGE_DESC.0
        );
    }
}
