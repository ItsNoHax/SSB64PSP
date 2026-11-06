//! Battle data arrays of the fighter, weapon and stage code (`d*` symbols in
//! `ovl2` and `ovl3`): the handicap table, costume IDs, item throw and
//! ground-hazard attacks, Yoshi's egg boxes, the Samus Charge Shot levels,
//! Master Hand's wait tables and the Zebes acid and Sector Z Arwing tables.
//!
//! Each table's run-time address is its symbol in
//! `refs/ssb-decomp-re/symbols/symbols_us.txt`; the overlay is the one the
//! defining file's `.data` sits in (`smashbrothers.us.yaml`).

use alloc::vec::Vec;

use crate::source::{OverlayData, OVL2, OVL3};
use crate::tables::{le_bytes, le_words, Ctx};
use crate::Result;

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

/// `TableId::Handicap`: `[knockback dealt, received]` by `handicap - 1`.
pub fn handicap_bytes(c: &Ctx) -> Result<Vec<u8>> {
    let d = OverlayData::new(Some(c.rom), OVL2);
    let (at, n) = HANDICAP_TABLE;
    let mut out = Vec::with_capacity(n * 2);
    for i in 0..2 * n as u32 {
        out.push(d.f32(at + i * 4)?);
    }
    Ok(le_bytes(&out, f32::to_le_bytes))
}

/// `TableId::CostumeRoyal` (`[u8; 4]` rows) or `CostumeTeam` (`[u8; 3]`).
fn costume_bytes(c: &Ctx, team: bool) -> Result<Vec<u8>> {
    let d = OverlayData::new(Some(c.rom), OVL2);
    let (at, n) = COSTUME_IDS;
    let mut out = Vec::new();
    for i in 0..n as u32 {
        let b = d.bytes(at + i * 8, 7)?;
        out.extend_from_slice(if team { &b[4..7] } else { &b[0..4] });
    }
    Ok(out)
}

/// `TableId::CostumeRoyal`.
pub fn costume_royal_bytes(c: &Ctx) -> Result<Vec<u8>> {
    costume_bytes(c, false)
}

/// `TableId::CostumeTeam`.
pub fn costume_team_bytes(c: &Ctx) -> Result<Vec<u8>> {
    costume_bytes(c, true)
}

/// A signed `bits`-wide field at `shift` of `word`.
fn sbits(word: u32, shift: u32, bits: u32) -> i32 {
    ((word << (32 - shift - bits)) as i32) >> (32 - bits)
}

/// `TableId::ItemThrowDescs`: `is_smash`, `velocity` (f32 bits), `angle`
/// (i32), `damage_scale` (f32 bits) per desc.
pub fn item_throws_bytes(c: &Ctx) -> Result<Vec<u8>> {
    let d = OverlayData::new(Some(c.rom), OVL2);
    let (at, n) = ITEM_THROW_DESCS;
    let mut out = Vec::with_capacity(n * 4);
    for i in 0..n as u32 {
        // Big-endian bitfields fill from the top: is_smash_throw:1,
        // vel_scale:10, angle:11, damage_scale:10 (unsigned).
        let v = d.u32(at + i * 4)?;
        out.extend([
            v >> 31,
            (sbits(v, 21, 10) as f32).to_bits(),
            sbits(v, 10, 11) as u32,
            ((v & 0x3FF) as f32).to_bits(),
        ]);
    }
    Ok(le_words(&out))
}

/// `TableId::GroundHitAttacks`: `kind, damage, angle, kb_scale, kb_weight,
/// kb_base, element` per floor.
pub fn ground_hits_bytes(c: &Ctx) -> Result<Vec<u8>> {
    let d = OverlayData::new(Some(c.rom), OVL2);
    let (at, n) = GROUND_HIT_ATTACKS;
    let mut out = Vec::with_capacity(n * 7);
    for i in 0..7 * n as u32 {
        out.push(d.i32(at + i * 4)?);
    }
    Ok(le_bytes(&out, i32::to_le_bytes))
}

/// `TableId::ZebesAcid`: `wait_base, random_min, random_max, level` (f32
/// bits) per step.
pub fn zebes_acid_bytes(c: &Ctx) -> Result<Vec<u8>> {
    let d = OverlayData::new(Some(c.rom), OVL2);
    let (at, n) = ZEBES_ACID;
    let mut out = Vec::with_capacity(n * 4);
    for i in 0..n as u32 {
        let r = at + i * 12;
        out.extend([
            d.u16(r)?.into(),
            d.u16(r + 2)?.into(),
            d.u16(r + 4)?.into(),
            d.f32(r + 8)?.to_bits(),
        ]);
    }
    Ok(le_words(&out))
}

/// `TableId::ShieldColors`: `prim[3], env[3]` per colour.
pub fn shield_colors_bytes(c: &Ctx) -> Result<Vec<u8>> {
    let (at, n) = SHIELD_COLORS;
    OverlayData::new(Some(c.rom), OVL2).bytes(at, 6 * n as u32)
}

/// `TableId::SectorMapX`.
pub fn sector_map_x_bytes(c: &Ctx) -> Result<Vec<u8>> {
    let d = OverlayData::new(Some(c.rom), OVL2);
    let (at, n) = SECTOR_MAP_POSITIONS_X;
    let xs: Vec<i16> = (0..n as u32)
        .map(|i| d.i16(at + i * 2))
        .collect::<Result<_>>()?;
    Ok(le_bytes(&xs, i16::to_le_bytes))
}

/// `TableId::SectorLasers`.
pub fn sector_lasers_bytes(c: &Ctx) -> Result<Vec<u8>> {
    let (at, n) = SECTOR_LASER_COUNTS;
    OverlayData::new(Some(c.rom), OVL2).bytes(at, n as u32)
}

/// `TableId::SectorPilots`.
pub fn sector_pilots_bytes(c: &Ctx) -> Result<Vec<u8>> {
    let (at, n) = SECTOR_PILOT_IDS;
    OverlayData::new(Some(c.rom), OVL2).bytes(at, n as u32)
}

/// `TableId::SectorPilotWindows`: `(first, count)` pairs.
pub fn sector_windows_bytes(c: &Ctx) -> Result<Vec<u8>> {
    let (at, n) = SECTOR_PILOT_WINDOWS;
    OverlayData::new(Some(c.rom), OVL2).bytes(at, 2 * n as u32)
}

/// `TableId::YoshiEgg`: `effect_size, offset[3], size[3]` per `FTKind`.
pub fn yoshi_egg_bytes(c: &Ctx) -> Result<Vec<u8>> {
    let d = OverlayData::new(Some(c.rom), OVL3);
    let (at, n) = YOSHI_EGG_DESCS;
    let f: Vec<f32> = (0..7 * n as u32)
        .map(|k| d.f32(at + k * 4))
        .collect::<Result<_>>()?;
    Ok(le_bytes(&f, f32::to_le_bytes))
}

/// `TableId::ChargeShot`: `gfx_size, vel_x` (f32 bits), `damage`,
/// `attack_size, coll_size` (f32 bits of the `s32`s), `priority` per level.
pub fn charge_shot_bytes(c: &Ctx) -> Result<Vec<u8>> {
    let d = OverlayData::new(Some(c.rom), OVL3);
    let (at, n) = CHARGE_SHOT_ATTRIBUTES;
    let mut out = Vec::with_capacity(n * 6);
    for i in 0..n as u32 {
        let r = at + i * 36;
        out.extend([
            d.f32(r)?.to_bits(),
            d.f32(r + 4)?.to_bits(),
            d.i32(r + 8)? as u32,
            (d.i32(r + 12)? as f32).to_bits(),
            (d.i32(r + 16)? as f32).to_bits(),
            d.i32(r + 32)? as u32,
        ]);
    }
    Ok(le_words(&out))
}

/// `TableId::BossWait`: `ftbosswait.o`'s `.data` and the 16 bytes after it.
pub fn boss_wait_bytes(c: &Ctx) -> Result<Vec<u8>> {
    let (at, n) = BOSS_WAIT_DATA;
    OverlayData::new(Some(c.rom), OVL3).bytes(at, n)
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
}
