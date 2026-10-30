//! `ssb-rom`'s tables of archive file ids and offsets that the game keeps in
//! its code overlays' data segments: each fighter's main file and
//! `FTAttributes` offset (`dFTManagerDataFiles`), the loading-break
//! transitions (`dLBTransitionDescs`) and the manager effects
//! (`dEFManager*EffectDesc`), and the weapons' `WPDesc` render flags. Only
//! the layout is committed: where each array is, and the labels and counts
//! the port gives its entries.

use std::fmt::Write;

use crate::emit;
use crate::item_sounds::WEAPONS;
use crate::source::{OverlayData, OVL0, OVL2, OVL3};
use crate::{Error, Generated, Result, Source};

/// `dFTManagerDataFiles` (`ft/ftdata.c`): `FTData *` per `FTKind`.
pub const FIGHTER_DATA_FILES: (u32, usize) = (0x8011_6E10, 27);
/// `FTData` (`ft/ftdef.h`): nine file ids from `file_main_id` at +0, the
/// nine file head pointers (`p_file_main`, ...) from +0x28, and
/// `o_attributes` at +0x60.
const FT_DATA_FILE_IDS: u32 = 0;
const FT_DATA_P_FILES: u32 = 0x28;
const FT_DATA_FILES: u32 = 9;
const FT_DATA_O_ATTRIBUTES: u32 = 0x60;
/// `file_special1_id` ..= `file_special4_id`: the sixth to ninth file ids.
const FT_DATA_SPECIALS: (usize, usize) = (5, 4);
/// Each `FTKind`'s symbol prefix, for diagnostics (`FighterFile::name`).
pub const FIGHTER_NAMES: [&str; 27] = [
    "Mario", "Fox", "Donkey", "Samus", "Luigi", "Link", "Yoshi", "Captain", "Kirby", "Pikachu",
    "Purin", "Ness", "Boss", "MMario", "NMario", "NFox", "NDonkey", "NSamus", "NLuigi", "NLink",
    "NYoshi", "NCaptain", "NKirby", "NPikachu", "NPurin", "NNess", "GDonkey",
];

/// `dLBTransitionDescs` (`lb/lbtransition.c`, `ovl0`): `LBTransitionDesc`
/// (`u32 file_id; intptr_t o_dobjdesc; intptr_t o_anim_joint; s32 user`).
pub const TRANSITION_DESCS: (u32, usize) = (0x800D_5D60, 11);
const TRANSITION_DESC_SIZE: u32 = 16;
/// The port's label and the animation's length in frames for each
/// transition, in `dLBTransitionDescs` order. The frame count is measured
/// from the animation (`tests/transitions.rs`), not stored in the desc.
pub const TRANSITIONS: [(&str, u32); 11] = [
    ("aeroplane", 64),
    ("check", 64),
    ("gakubuthi", 64),
    ("kannon", 64),
    ("star", 64),
    ("sudare1", 64),
    ("sudare2", 64),
    ("camera", 64),
    ("block", 64),
    ("rot_scale", 64),
    ("curtain", 72),
];

/// The file ids the effect descs' `file_head` variables hold that are not a
/// fighter's file (`sEFManagerEffectsFileHead` and friends: the code loads
/// each with a `ll*FileID`): `(variable, archive file id)`.
pub const FILE_HEADS: [(u32, u32); 5] = [
    (0x8013_13B0, 83),
    (0x8013_13B4, 84),
    (0x8013_13B8, 85),
    (0x8013_0D98, 163),
    (0x8018_D040, 86),
];
/// `EFDesc` (`ef/eftypes.h`): `dl_link` at +1, `file_head` at +4,
/// `proc_display` at +0x14, `o_dobjdesc` at +0x18, `o_mobjsub` at +0x1C,
/// `o_anim_joint` at +0x20, `o_matanim_joint` at +0x24.
const EF_DESC_DL_LINK: u32 = 1;
const EF_DESC_FILE_HEAD: u32 = 4;
const EF_DESC_PROC_DISPLAY: u32 = 0x14;
const EF_DESC_O_DOBJDESC: u32 = 0x18;
const EF_DESC_O_ANIM_JOINT: u32 = 0x20;
const EF_DESC_O_MATANIM_JOINT: u32 = 0x24;
/// The 46 distinct manager effects the port draws: the `dEFManager<Name>
/// EffectDesc` and its address, in the port's inventory order (the
/// `AnimDesc::EFFECT` slot is the index). Of descs with identical objects
/// the first by this order is listed.
pub const EFFECTS: [(&str, u32); 46] = [
    ("DamageSlash", 0x8012_DF24),
    ("DamageFlyOrbs", 0x8012_DF74),
    ("ImpactWave", 0x8012_DFC4),
    ("DamageFlySparks", 0x8012_E014),
    ("DamageFlyMDust", 0x8012_E064),
    ("ShockSmall", 0x8012_DF4C),
    ("FireSpark", 0x8012_E0B4),
    ("CatchSwirl", 0x8012_E184),
    ("ReflectBreak", 0x8012_E1AC),
    ("DeadExplode", 0x8012_E33C),
    ("NessPKFlash", 0x8012_E4BC),
    ("MBallRays", 0x8012_E55C),
    ("RebirthHalo", 0x8012_E770),
    ("ItemGetSwirl", 0x8012_E798),
    ("Shield", 0x8012_E134),
    ("FoxReflector", 0x8012_E0EC),
    ("YoshiEggEscape", 0x8012_E604),
    ("PikachuUnk", 0x8012_E1D4),
    ("PikachuThunderShock", 0x8012_E1FC),
    ("PikachuThunderTrail", 0x8012_E224),
    ("ThunderJolt", 0x8012_E24C),
    ("VulcanJab", 0x8012_E274),
    ("KirbyCutterTrail", 0x8012_E3F4),
    ("KirbyCutterUp", 0x8012_E37C),
    ("KirbyEntryStar", 0x8012_E534),
    ("KirbyCutterDown", 0x8012_E3A4),
    ("KirbyCutterDraw", 0x8012_E3CC),
    ("SamusGrappleBeam", 0x8012_E29C),
    ("SamusEntryPoint", 0x8012_E67C),
    ("CaptainFalconKick", 0x8012_E2C4),
    ("CaptainEntryCar", 0x8012_E6A4),
    ("CaptainFalconPunch", 0x8012_E2EC),
    ("PurinSing", 0x8012_E314),
    ("NessPsychicMagnet", 0x8012_E41C),
    ("NessPKReflectTrail", 0x8012_E46C),
    ("NessPKThunderWave", 0x8012_E494),
    ("LinkEntryWave", 0x8012_E4E4),
    ("LinkEntryBeam", 0x8012_E50C),
    ("LinkSpinAttack", 0x8012_E62C),
    ("MBallThrown", 0x8012_E584),
    ("CaptureKirbyStar", 0x8012_E720),
    ("YoshiEntryEgg", 0x8012_E5AC),
    ("YoshiEggLay", 0x8012_E5DC),
    ("DonkeyEntryTaru", 0x8012_E654),
    ("MarioEntryDokan", 0x8012_E6CC),
    ("FoxEntryArwing", 0x8012_E6F4),
];
/// `dEFManagerDeadExplodeMatAnimJoints[player]`: the blast's material script
/// per player. `efManagerDeadExplodeMakeEffect` writes the player's into
/// the desc before use, so the desc's own field is the last player's; the
/// pack binds player 0's, which is the entry listed here.
const DEAD_EXPLODE_DESC: u32 = 0x8012_E33C;
const DEAD_EXPLODE_MAT_ANIM_JOINTS: u32 = 0x8012_E36C;

/// `WPDesc` (`wp/wptypes.h`): `flags` at +0, `o_attributes` at +0xC.
const WP_DESC_FLAGS: u32 = 0;
const WP_DESC_O_ATTRIBUTES: u32 = 0xC;
/// The `WPDesc` each [`WEAPONS`] row is made with, in that order:
/// `(row name, desc address)`. The `ovl3` descs are `wp/*.c`'s and
/// `it/*.c`'s `d*WeaponDesc`; the Arwing's are `gr/grcommon/grsector.c`'s,
/// in `ovl2`. Two rows share another row's desc, whose `o_attributes` the
/// maker rewrites before `wpManagerMakeWeapon`:
/// - `LUIGI_FIREBALL`: `dWPMarioFireballWeaponDesc`
///   (`wpMarioFireballMakeWeapon`, `wpmariofireball.c:167-168`);
/// - `STARROD_SMASH`: `dITStarRodWeaponStarWeaponDesc`
///   (`itStarRodWeaponStarMakeWeapon`, `itstarrod.c:374`).
pub const WEAPON_DESCS: [(&str, u32); 35] = [
    ("MARIO_FIREBALL", 0x8018_8E90),
    ("LUIGI_FIREBALL", 0x8018_8E90),
    ("FOX_BLASTER", 0x8018_8ED0),
    ("SAMUS_BOMB", 0x8018_9070),
    ("SAMUS_CHARGE_SHOT", 0x8018_9030),
    ("LINK_SPIN_ATTACK", 0x8018_9360),
    ("LINK_BOOMERANG", 0x8018_93A0),
    ("KIRBY_CUTTER", 0x8018_92A0),
    ("NESS_PK_THUNDER", 0x8018_91D0),
    ("NESS_PK_THUNDER_TRAIL", 0x8018_9204),
    ("NESS_PK_FIRE", 0x8018_9190),
    ("PIKACHU_THUNDER_HEAD", 0x8018_9120),
    ("PIKACHU_THUNDER_TRAIL", 0x8018_9154),
    ("PIKACHU_THUNDER_JOLT_AIR", 0x8018_90B0),
    ("PIKACHU_THUNDER_JOLT_GROUND", 0x8018_90E4),
    ("YOSHI_EGG_THROW", 0x8018_92E0),
    ("YOSHI_STAR", 0x8018_9320),
    ("BOSS_BULLET_NORMAL", 0x8018_93E0),
    ("BOSS_BULLET_HARD", 0x8018_9414),
    ("LGUN_AMMO", 0x8018_9C24),
    ("FFLOWER_FLAME", 0x8018_9D34),
    ("STARROD", 0x8018_A1C4),
    ("STARROD_SMASH", 0x8018_A1C4),
    ("WARK_ROCK", 0x8018_AB04),
    ("NYARS_COIN", 0x8018_ACF4),
    ("LIZARDON_FLAME", 0x8018_ADC4),
    ("SPEAR_SWARM", 0x8018_AE74),
    ("KAMEX_HYDRO", 0x8018_AF74),
    ("STARMIE_SWIFT", 0x8018_B1E4),
    ("DOGAS_SMOG", 0x8018_B334),
    ("PIPPI_SWARM", 0x8018_AEA8),
    ("ARWING_LASER_2D", 0x8012_E9F0),
    ("ARWING_LASER_3D", 0x8012_EA24),
    ("HITOKAGE_FLAME", 0x8018_B4F4),
    ("FUSHIGIBANA_RAZOR", 0x8018_B564),
];
/// The rows whose desc's `o_attributes` is rewritten at run time (above).
const WEAPON_DESC_SHARED: [&str; 2] = ["LUIGI_FIREBALL", "STARROD_SMASH"];

/// `ssb-rom`'s tables.
pub fn generate_rom(rom: Option<&Source>) -> Result<Vec<Generated>> {
    let ovl2 = OverlayData::new(rom, OVL2);
    let (files, heads) = fighter_files(ovl2)?;
    Ok(vec![
        Generated {
            name: "fighter_files.rs",
            text: files,
        },
        Generated {
            name: "transition_assets.rs",
            text: transitions(OverlayData::new(rom, OVL0))?,
        },
        Generated {
            name: "effect_tables.rs",
            text: effects(ovl2, &heads)?,
        },
        Generated {
            name: "weapon_tables.rs",
            text: weapons(rom)?,
        },
    ])
}

/// `FIGHTER_FILES`, and the `(variable, file id)` of every fighter file head
/// the `FTData`s declare (`p_file_*` and the id beside it).
fn fighter_files(d: OverlayData) -> Result<(String, Vec<(u32, u32)>)> {
    let (at, n) = FIGHTER_DATA_FILES;
    let mut w = emit::header("`dFTManagerDataFiles`");
    let _ = writeln!(w, "pub const FIGHTER_FILES: [FighterFile; {n}] = [");
    let mut heads: Vec<(u32, u32)> = Vec::new();
    for (kind, name) in FIGHTER_NAMES.iter().enumerate() {
        let mut specials = [None; FT_DATA_SPECIALS.1];
        let (file, offset) = if d.rom.is_some() {
            let data = d.u32(at + kind as u32 * 4)?;
            for (k, special) in specials.iter_mut().enumerate() {
                let i = (FT_DATA_SPECIALS.0 + k) as u32;
                if d.u32(data + FT_DATA_P_FILES + 4 * i)? != 0 {
                    *special = Some(d.u32(data + FT_DATA_FILE_IDS + 4 * i)?);
                }
            }
            for i in 0..FT_DATA_FILES {
                let head = d.u32(data + FT_DATA_P_FILES + 4 * i)?;
                let id = d.u32(data + FT_DATA_FILE_IDS + 4 * i)?;
                if head == 0 {
                    continue;
                }
                match heads.iter().find(|(h, _)| *h == head) {
                    Some(&(_, known)) if known != id => {
                        return Err(Error::BadRelocation {
                            file: id,
                            offset: head as usize,
                        })
                    }
                    Some(_) => {}
                    None => heads.push((head, id)),
                }
            }
            (
                d.u32(data + FT_DATA_FILE_IDS)?,
                d.u32(data + FT_DATA_O_ATTRIBUTES)?,
            )
        } else {
            (0, 0)
        };
        let _ = writeln!(
            w,
            "    FighterFile {{ kind: {kind}, name: \"{name}\", file: {file}, offset: 0x{offset:04X}, specials: {specials:?} }},"
        );
    }
    w.push_str("];\n");
    Ok((w, heads))
}

fn transitions(d: OverlayData) -> Result<String> {
    let (at, n) = TRANSITION_DESCS;
    let mut w = emit::header("`dLBTransitionDescs`");
    let _ = writeln!(w, "pub const ASSETS: [TransitionAsset; {n}] = [");
    for (i, (name, frames)) in TRANSITIONS.iter().enumerate() {
        let at = at + i as u32 * TRANSITION_DESC_SIZE;
        let (file, graph, anim_joints) = (d.u32(at)?, d.u32(at + 4)?, d.u32(at + 8)?);
        let _ = writeln!(
            w,
            "    TransitionAsset {{ name: \"{name}\", file: {file}, graph: 0x{graph:X}, anim_joints: 0x{anim_joints:X}, frames: {frames} }},"
        );
    }
    w.push_str("];\n");
    Ok(w)
}

/// `UpperSnake` of a `CamelCase` effect name: `NessPKFlash` is
/// `NESS_PK_FLASH`.
fn snake(name: &str) -> String {
    let chars: Vec<char> = name.chars().collect();
    let mut out = String::new();
    for (i, &c) in chars.iter().enumerate() {
        let prev = i.checked_sub(1).map(|p| chars[p]);
        let next = chars.get(i + 1).copied();
        let boundary = c.is_ascii_uppercase()
            && prev.is_some_and(|p| {
                p.is_ascii_lowercase()
                    || p.is_ascii_digit()
                    || (p.is_ascii_uppercase() && next.is_some_and(|x| x.is_ascii_lowercase()))
            });
        if boundary {
            out.push('_');
        }
        out.push(c.to_ascii_uppercase());
    }
    out
}

/// `MANAGER_EFFECT_KEYS`, `MANAGER_EFFECT_ANIM_JOINTS`,
/// `MANAGER_EFFECT_MAT_ANIM_JOINTS` and each effect's index.
fn effects(d: OverlayData, fighter_heads: &[(u32, u32)]) -> Result<String> {
    let mut keys = String::new();
    let mut anims = String::new();
    let mut mats = String::new();
    let mut slots = String::new();
    let mut links = String::new();
    let mut displays = String::new();
    for (slot, (name, at)) in EFFECTS.iter().enumerate() {
        let at = *at;
        let (link, display) = if d.rom.is_some() {
            (
                d.u8(at + EF_DESC_DL_LINK)?,
                d.u32(at + EF_DESC_PROC_DISPLAY)?,
            )
        } else {
            (0, 0)
        };
        let _ = writeln!(links, "    {link},");
        let _ = writeln!(displays, "    0x{display:08X},");
        let (file, dobj, anim, mat) = if d.rom.is_some() {
            let head = d.u32(at + EF_DESC_FILE_HEAD)?;
            let file = FILE_HEADS
                .iter()
                .chain(fighter_heads)
                .find(|(h, _)| *h == head)
                .map(|&(_, id)| id)
                .ok_or(Error::BadRelocation {
                    file: 0,
                    offset: head as usize,
                })?;
            let mat = if at == DEAD_EXPLODE_DESC {
                d.u32(DEAD_EXPLODE_MAT_ANIM_JOINTS)?
            } else {
                d.u32(at + EF_DESC_O_MATANIM_JOINT)?
            };
            (
                file,
                d.u32(at + EF_DESC_O_DOBJDESC)?,
                d.u32(at + EF_DESC_O_ANIM_JOINT)?,
                mat,
            )
        } else {
            (0, 0, 0, 0)
        };
        let opt = |v: u32| match v {
            0 => "None".to_string(),
            v => format!("Some(0x{v:04X})"),
        };
        let _ = writeln!(keys, "    ({file}, 0x{dobj:04X}),");
        let _ = writeln!(anims, "    {},", opt(anim));
        let _ = writeln!(mats, "    {},", opt(mat));
        let _ = writeln!(slots, "    pub const {}: usize = {slot};", snake(name));
    }
    let mut w = emit::header("`dEFManager*EffectDesc`");
    let _ = writeln!(
        w,
        "pub const MANAGER_EFFECT_KEYS: &[(u32, u32)] = &[\n{keys}];"
    );
    let _ = writeln!(
        w,
        "pub const MANAGER_EFFECT_ANIM_JOINTS: &[Option<u32>] = &[\n{anims}];"
    );
    let _ = writeln!(
        w,
        "pub const MANAGER_EFFECT_MAT_ANIM_JOINTS: &[Option<u32>] = &[\n{mats}];"
    );
    let _ = writeln!(
        w,
        "/// `EFDesc.dl_link`.\npub const MANAGER_EFFECT_DL_LINKS: &[u8] = &[\n{links}];"
    );
    let _ = writeln!(
        w,
        "/// `EFDesc.proc_display`'s address.\npub const MANAGER_EFFECT_DISPLAYS: &[u32] = &[\n{displays}];"
    );
    let _ = writeln!(
        w,
        "/// Each effect's index in the tables above.\npub mod slot {{\n{slots}}}"
    );
    Ok(w)
}

/// `WEAPON_DESC_FLAGS`: each [`WEAPONS`] row's `(attributes file,
/// attributes offset, WPDesc.flags, data)`: `data` is where
/// `WPAttributes.data` (offset 0) relocates to, `None` when it is `NULL`
/// (the Fire Flower's flame draws particles only). A desc's own
/// `o_attributes` must name its row's record (two readings: the decomp's
/// pairing and the ROM's word), except where the maker rewrites it.
fn weapons(rom: Option<&Source>) -> Result<String> {
    let mut rows = String::new();
    for ((name, file, attributes), (desc_name, at)) in WEAPONS.iter().zip(WEAPON_DESCS) {
        assert_eq!(*name, desc_name, "WEAPON_DESCS follows WEAPONS");
        let (flags, data) = match rom {
            Some(source) => {
                let d = OverlayData::new(rom, if at >= OVL3.vram { OVL3 } else { OVL2 });
                let o_attributes = d.u32(at + WP_DESC_O_ATTRIBUTES)?;
                if !WEAPON_DESC_SHARED.contains(name) && o_attributes != *attributes {
                    return Err(Error::BadRelocation {
                        file: *file,
                        offset: o_attributes as usize,
                    });
                }
                let f = source.file(*file)?;
                let data = f
                    .intern_relocs
                    .iter()
                    .find(|r| r.at == *attributes)
                    .map(|r| (*file, r.target))
                    .or_else(|| {
                        f.extern_relocs
                            .iter()
                            .find(|r| r.at == *attributes)
                            .map(|r| (u32::from(r.target_file), r.target_offset))
                    });
                (d.u8(at + WP_DESC_FLAGS)?, data)
            }
            None => (0, None),
        };
        let data = match data {
            Some((f, o)) => format!("Some(({f}, 0x{o:04X}))"),
            None => "None".to_string(),
        };
        let _ = writeln!(
            rows,
            "    ({file}, 0x{attributes:04X}, 0x{flags:02X}, {data}), // {name}"
        );
    }
    let mut w = emit::header("the weapons' `WPDesc.flags` and `WPAttributes.data`");
    let _ = writeln!(
        w,
        "/// `(attributes file, attributes offset, WPDesc.flags, WPAttributes.data)`.\n\
         pub type WeaponDescFlags = (u32, u32, u8, Option<(u32, u32)>);\n\
         pub const WEAPON_DESC_FLAGS: &[WeaponDescFlags] = &[\n{rows}];"
    );
    Ok(w)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stubs_have_every_row() {
        let files = generate_rom(None).unwrap();
        assert_eq!(files.len(), 4);
        assert_eq!(files[0].text.matches("FighterFile {").count(), 27);
        assert_eq!(files[1].text.matches("TransitionAsset {").count(), 11);
        assert_eq!(files[2].text.matches("    (0, 0x0000),").count(), 46);
        assert_eq!(files[2].text.matches("pub const").count(), 5 + 46);
        assert_eq!(files[3].text.matches(", 0x00, None), //").count(), 35);
    }

    #[test]
    fn weapon_descs_follow_the_weapon_records_and_lie_in_their_overlay() {
        for ((name, ..), (desc_name, at)) in WEAPONS.iter().zip(WEAPON_DESCS) {
            assert_eq!(*name, desc_name);
            let overlay = if at >= OVL3.vram { OVL3 } else { OVL2 };
            overlay.at(at);
            overlay.at(at + WP_DESC_O_ATTRIBUTES + 3);
        }
    }

    #[test]
    fn effect_names_become_constants() {
        assert_eq!(snake("NessPKFlash"), "NESS_PK_FLASH");
        assert_eq!(snake("DamageFlyMDust"), "DAMAGE_FLY_M_DUST");
        assert_eq!(snake("MBallRays"), "M_BALL_RAYS");
        assert_eq!(snake("Shield"), "SHIELD");
    }

    #[test]
    fn effect_names_are_distinct_and_in_the_overlay() {
        let mut seen = std::collections::HashSet::new();
        for (name, at) in EFFECTS {
            assert!(seen.insert(snake(name)), "{name}");
            OVL2.at(at);
            OVL2.at(at + 0x27);
        }
        assert_eq!(seen.len(), 46);
    }

    /// `dEFManagerLoseKirbyStarEffectDesc` (0x8012E748) is not a row of
    /// [`EFFECTS`]: it draws the same `llITCommonDataKirbyStarDObjDesc`
    /// from the same file as `CaptureKirbyStar`, so its object key would
    /// repeat that row's.
    #[test]
    fn the_lost_star_shares_the_captured_stars_object() {
        let Some(path) = crate::test_rom(env!("CARGO_MANIFEST_DIR")) else {
            return;
        };
        let rom = Source::open(&path).unwrap();
        let d = OverlayData::new(Some(&rom), OVL2);
        let (capture, lose) = (0x8012_E720, 0x8012_E748);
        assert_eq!(EFFECTS.iter().filter(|(_, at)| *at == capture).count(), 1);
        assert!(EFFECTS.iter().all(|(_, at)| *at != lose));
        for field in [EF_DESC_FILE_HEAD, EF_DESC_O_DOBJDESC] {
            let a = d.u32(capture + field).unwrap();
            assert_ne!(a, 0);
            assert_eq!(a, d.u32(lose + field).unwrap(), "{field:#x}");
        }
    }
}
