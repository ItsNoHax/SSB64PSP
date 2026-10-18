//! Battle data arrays of the fighter, weapon and stage code (`d*` symbols in
//! `ovl2` and `ovl3`): the handicap table, costume IDs, item throw and
//! ground-hazard attacks, Yoshi's egg boxes, the Samus Charge Shot levels,
//! Master Hand's wait tables and the Zebes acid and Sector Z Arwing tables.
//!
//! Each table's run-time address is its symbol in
//! `refs/ssb-decomp-re/symbols/symbols_us.txt`; the overlay is the one the
//! defining file's `.data` sits in (`smashbrothers.us.yaml`).

use std::fmt::Write;

use crate::emit::{self, f32_lit};
use crate::source::{OverlayData, OVL2, OVL3};
use crate::{Generated, Result, Source};

/// `dFTCommonDataHandicapTable` (`ft/ftcommondata.c`): `f32[40][2]`.
pub const HANDICAP_TABLE: (u32, usize) = (0x8012_C830, 40);
/// `dFTParamCostumeIDs` (`ft/ftparam.c`): 8-byte `FTCostume`s; the port
/// keeps the twelve playable fighters'.
pub const COSTUME_IDS: (u32, usize) = (0x8012_B830, 12);
/// `dFTCommonDataItemThrowDescs` (`ft/ftcommondata.c`): 32-bit `FTItemThrow`
/// bitfields from `nFTCommonStatusLightThrowDrop`.
pub const ITEM_THROW_DESCS: (u32, usize) = (0x8012_C9E0, 22);
/// `dFTMainGroundHitCollisionAttributes` (`ft/ftmain.c`): seven-word
/// `GRAttackColl`s.
pub const GROUND_HIT_ATTACKS: (u32, usize) = (0x8012_8D30, 6);
/// `dGRZebesAcidAttributes` (`gr/grcommon/grzebes.c`): 12-byte `GRZebesAcid`s.
pub const ZEBES_ACID: (u32, usize) = (0x8012_EA60, 16);
/// `dGRSectorArwingMapPositionsX` (`s16[3]`), `...LaserCounts` (`u8[8]`),
/// `...PilotIDs` (`u8[56]`) and `...PilotWaitTimers` (`u8[6][2]`)
/// (`gr/grcommon/grsector.c`).
pub const SECTOR_MAP_POSITIONS_X: (u32, usize) = (0x8012_E978, 3);
pub const SECTOR_LASER_COUNTS: (u32, usize) = (0x8012_E980, 8);
pub const SECTOR_PILOT_IDS: (u32, usize) = (0x8012_E988, 56);
pub const SECTOR_PILOT_WINDOWS: (u32, usize) = (0x8012_E9C0, 6);
/// `dEFManagerShieldColors` (`ef/efmanager.c`): six-byte `SYColorRGBPair`s
/// (PRIM RGB, ENV RGB) per player, then the damaged shield's.
pub const SHIELD_COLORS: (u32, usize) = (0x8012_E114, 5);
/// `dFTCommonYoshiEggDamageCollDescs` (`ft/ftcommon/ftcommoncaptureyoshi.c`,
/// `ovl3`): 28-byte `ftCommonYoshiEggDesc`s, one per `FTKind`.
pub const YOSHI_EGG_DESCS: (u32, usize) = (0x8018_8720, 27);
/// `dWPSamusChargeShotWeaponAttributes` (`wp/wpsamus/wpsamuschargeshot.c`,
/// `ovl3`): 36-byte `wpSamusChargeShotAttributes`.
pub const CHARGE_SHOT_ATTRIBUTES: (u32, usize) = (0x8018_8F10, 8);
/// `ftbosswait.o`'s `.data` (`ovl3`, `dFTBossWaitRandomStatusIDs` on) and
/// the 16 bytes after it, which `ftBossWaitDecideStatusComputer` can read.
pub const BOSS_WAIT_DATA: (u32, u32) = (0x8018_8DC0, 0x50);

pub fn generate(rom: Option<&Source>) -> Result<Vec<Generated>> {
    let ovl2 = OverlayData::new(rom, OVL2);
    let ovl3 = OverlayData::new(rom, OVL3);
    Ok(vec![
        Generated {
            name: "handicap.rs",
            text: handicap(ovl2)?,
        },
        Generated {
            name: "costume_ids.rs",
            text: costumes(ovl2)?,
        },
        Generated {
            name: "item_throw_descs.rs",
            text: item_throws(ovl2)?,
        },
        Generated {
            name: "ground_hit_attacks.rs",
            text: ground_hits(ovl2)?,
        },
        Generated {
            name: "zebes_acid.rs",
            text: zebes_acid(ovl2)?,
        },
        Generated {
            name: "sector_arwing.rs",
            text: sector(ovl2)?,
        },
        Generated {
            name: "shield_colors.rs",
            text: shield_colors(ovl2)?,
        },
        Generated {
            name: "yoshi_egg.rs",
            text: yoshi_egg(ovl3)?,
        },
        Generated {
            name: "charge_shot.rs",
            text: charge_shot(ovl3)?,
        },
        Generated {
            name: "boss_wait.rs",
            text: boss_wait(ovl3)?,
        },
    ])
}

fn handicap(d: OverlayData) -> Result<String> {
    let (at, n) = HANDICAP_TABLE;
    let mut w = emit::header("`dFTCommonDataHandicapTable`");
    w.push_str(
        "/// Indexed by `handicap - 1`: `[0]` scales knockback dealt, `[1]`\n\
         /// knockback received.\n",
    );
    let _ = writeln!(w, "pub const HANDICAP_TABLE: [[f32; 2]; {n}] = [");
    for i in 0..n as u32 {
        let (a, b) = (d.f32(at + i * 8)?, d.f32(at + i * 8 + 4)?);
        let _ = writeln!(w, "    [{}, {}],", f32_lit(a), f32_lit(b));
    }
    w.push_str("];\n");
    Ok(w)
}

fn costumes(d: OverlayData) -> Result<String> {
    let (at, n) = COSTUME_IDS;
    let mut w = emit::header("`dFTParamCostumeIDs` (`FTCostume::royal`, `::team`)");
    let mut royal = String::new();
    let mut team = String::new();
    for i in 0..n as u32 {
        let b = d.bytes(at + i * 8, 7)?;
        let _ = writeln!(royal, "    [{}, {}, {}, {}],", b[0], b[1], b[2], b[3]);
        let _ = writeln!(team, "    [{}, {}, {}],", b[4], b[5], b[6]);
    }
    let _ = write!(
        w,
        "/// `FTCostume::royal` by `nFTKind`, C-Up, C-Right, C-Down, C-Left.\n\
         pub const ROYAL: [[u8; 4]; {n}] = [\n{royal}];\n\n\
         /// `FTCostume::team` by `nFTKind`: red, blue, green.\n\
         pub const TEAM: [[u8; 3]; {n}] = [\n{team}];\n"
    );
    Ok(w)
}

/// A signed `bits`-wide field at `shift` of `word`.
fn sbits(word: u32, shift: u32, bits: u32) -> i32 {
    ((word << (32 - shift - bits)) as i32) >> (32 - bits)
}

fn item_throws(d: OverlayData) -> Result<String> {
    let (at, n) = ITEM_THROW_DESCS;
    let mut w = emit::header("`dFTCommonDataItemThrowDescs`");
    let _ = writeln!(w, "pub const THROW_DESCS: [ThrowDesc; {n}] = [");
    for i in 0..n as u32 {
        // Big-endian bitfields fill from the top: is_smash_throw:1,
        // vel_scale:10, angle:11, damage_scale:10 (unsigned).
        let v = d.u32(at + i * 4)?;
        let _ = writeln!(
            w,
            "    ThrowDesc {{ is_smash: {}, velocity: {}, angle: {}, damage_scale: {} }},",
            v >> 31 != 0,
            f32_lit(sbits(v, 21, 10) as f32),
            sbits(v, 10, 11),
            f32_lit((v & 0x3FF) as f32),
        );
    }
    w.push_str("];\n");
    Ok(w)
}

fn ground_hits(d: OverlayData) -> Result<String> {
    let (at, n) = GROUND_HIT_ATTACKS;
    let mut w = emit::header("`dFTMainGroundHitCollisionAttributes`");
    let _ = writeln!(w, "pub const DAMAGE_FLOORS: [GroundAttack; {n}] = [");
    for i in 0..n as u32 {
        let f = |k: u32| d.i32(at + i * 28 + k * 4);
        let _ = writeln!(
            w,
            "    GroundAttack {{ kind: {}, damage: {}, angle: {}, kb_scale: {}, kb_weight: {}, kb_base: {}, element: {} }},",
            f(0)?, f(1)?, f(2)?, f(3)?, f(4)?, f(5)?, f(6)?
        );
    }
    w.push_str("];\n");
    Ok(w)
}

fn zebes_acid(d: OverlayData) -> Result<String> {
    let (at, n) = ZEBES_ACID;
    let mut w = emit::header("`dGRZebesAcidAttributes`");
    let _ = writeln!(w, "pub const ACID_STEPS: [AcidStep; {n}] = [");
    for i in 0..n as u32 {
        let r = at + i * 12;
        let _ = writeln!(
            w,
            "    AcidStep {{ wait_base: {}, random_min: {}, random_max: {}, level: {} }},",
            d.u16(r)?,
            d.u16(r + 2)?,
            d.u16(r + 4)?,
            f32_lit(d.f32(r + 8)?)
        );
    }
    w.push_str("];\n");
    Ok(w)
}

fn shield_colors(d: OverlayData) -> Result<String> {
    let (at, n) = SHIELD_COLORS;
    let mut w = emit::header("`dEFManagerShieldColors`");
    let _ = writeln!(w, "pub const SHIELD_COLORS: [([u8; 3], [u8; 3]); {n}] = [");
    for i in 0..n as u32 {
        let b = d.bytes(at + i * 6, 6)?;
        let _ = writeln!(
            w,
            "    ([0x{:02X}, 0x{:02X}, 0x{:02X}], [0x{:02X}, 0x{:02X}, 0x{:02X}]),",
            b[0], b[1], b[2], b[3], b[4], b[5]
        );
    }
    w.push_str("];\n");
    Ok(w)
}

fn sector(d: OverlayData) -> Result<String> {
    let mut w = emit::header("the Sector Z Arwing's `dGRSectorArwing*` tables");
    let (at, n) = SECTOR_MAP_POSITIONS_X;
    let xs: Vec<String> = (0..n as u32)
        .map(|i| d.i16(at + i * 2).map(|x| x.to_string()))
        .collect::<Result<_>>()?;
    let _ = writeln!(
        w,
        "/// `dGRSectorArwingMapPositionsX`.\npub const MAP_POSITIONS_X: [i16; {n}] = [{}];\n",
        xs.join(", ")
    );
    for (name, sym, (at, n)) in [
        ("LASER_COUNTS", "LaserCounts", SECTOR_LASER_COUNTS),
        ("PILOT_IDS", "PilotIDs", SECTOR_PILOT_IDS),
    ] {
        let b = d.bytes(at, n as u32)?;
        let cells: Vec<String> = b.iter().map(u8::to_string).collect();
        let _ = writeln!(
            w,
            "/// `dGRSectorArwing{sym}`.\n#[rustfmt::skip]\npub const {name}: [u8; {n}] = [{}];\n",
            cells.join(", ")
        );
    }
    let (at, n) = SECTOR_PILOT_WINDOWS;
    let b = d.bytes(at, 2 * n as u32)?;
    let cells: Vec<String> = b
        .chunks(2)
        .map(|p| format!("({}, {})", p[0], p[1]))
        .collect();
    let _ = writeln!(
        w,
        "/// `dGRSectorArwingPilotWaitTimers`: `(first, count)` windows into\n\
         /// [`PILOT_IDS`].\npub const PILOT_WINDOWS: [(u8, u8); {n}] = [{}];",
        cells.join(", ")
    );
    Ok(w)
}

fn yoshi_egg(d: OverlayData) -> Result<String> {
    let (at, n) = YOSHI_EGG_DESCS;
    let mut w = emit::header("`dFTCommonYoshiEggDamageCollDescs`");
    w.push_str("/// By `FTKind`: `(effect_size, offset, size)`.\n");
    let _ = writeln!(
        w,
        "pub const EGG_DESCS: [(f32, [f32; 3], [f32; 3]); {n}] = ["
    );
    for i in 0..n as u32 {
        let f: Vec<String> = (0..7)
            .map(|k| d.f32(at + i * 28 + k * 4).map(f32_lit))
            .collect::<Result<_>>()?;
        let _ = writeln!(
            w,
            "    ({}, [{}, {}, {}], [{}, {}, {}]),",
            f[0], f[1], f[2], f[3], f[4], f[5], f[6]
        );
    }
    w.push_str("];\n");
    Ok(w)
}

fn charge_shot(d: OverlayData) -> Result<String> {
    let (at, n) = CHARGE_SHOT_ATTRIBUTES;
    let mut w = emit::header("`dWPSamusChargeShotWeaponAttributes`");
    let mut levels = String::new();
    let mut priorities = Vec::new();
    for i in 0..n as u32 {
        let r = at + i * 36;
        let _ = writeln!(
            levels,
            "    ({}, {}, {}, {}, {}),",
            f32_lit(d.f32(r)?),
            f32_lit(d.f32(r + 4)?),
            d.i32(r + 8)?,
            f32_lit(d.i32(r + 12)? as f32),
            f32_lit(d.i32(r + 16)? as f32),
        );
        priorities.push(d.i32(r + 32)?.to_string());
    }
    let _ = write!(
        w,
        "/// `(gfx_size, vel_x, damage, attack_size, coll_size)` per charge level.\n\
         pub const SAMUS_CHARGE_SHOT_LEVELS: [(f32, f32, i32, f32, f32); {n}] = [\n{levels}];\n\n\
         /// `priority` per charge level.\n\
         pub const SAMUS_CHARGE_SHOT_PRIORITIES: [i32; {n}] = [{}];\n",
        priorities.join(", ")
    );
    Ok(w)
}

fn boss_wait(d: OverlayData) -> Result<String> {
    let (at, n) = BOSS_WAIT_DATA;
    let mut w = emit::header("`ftbosswait.o`'s `.data` and the 16 bytes after it");
    let b = d.bytes(at, n)?;
    let _ = writeln!(
        w,
        "#[rustfmt::skip]\npub const WAIT_DATA: [u8; 0x{n:X}] = ["
    );
    for row in b.chunks(16) {
        let cells: Vec<String> = row.iter().map(|x| format!("0x{x:02X}")).collect();
        let _ = writeln!(w, "    {},", cells.join(", "));
    }
    w.push_str("];\n");
    Ok(w)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn throw_bitfields_unpack_from_the_top() {
        // { FALSE, 36, 110, 50 } as IDO packs it.
        let v = (36u32 << 21) | (110 << 10) | 50;
        assert_eq!(
            (v >> 31, sbits(v, 21, 10), sbits(v, 10, 11), v & 0x3FF),
            (0, 36, 110, 50)
        );
        // { TRUE, 110, -70, 100 }.
        let v = (1 << 31) | (110 << 21) | (((-70i32 as u32) & 0x7FF) << 10) | 100;
        assert_eq!(
            (v >> 31, sbits(v, 21, 10), sbits(v, 10, 11), v & 0x3FF),
            (1, 110, -70, 100)
        );
    }

    #[test]
    fn stubs_have_every_row() {
        let files = generate(None).unwrap();
        assert_eq!(files.len(), 10);
        let text = &files[0].text;
        assert_eq!(text.matches("    [0.0, 0.0],").count(), HANDICAP_TABLE.1);
    }
}
