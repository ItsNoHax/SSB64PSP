//! The fighters' `FTThrowHitDesc[2]` tables (`ftdef.h`), for
//! `ssb_game::grab`.
//!
//! Each table sits in a `*MainMotion` file's data, named `d*MainMotion_*`
//! or `d*MainMotion_0xNNNN` (the suffix is the file offset), and a motion
//! script's `ftMotionCommandSetThrow` points at it. A descriptor is seven
//! big-endian `s32`s: `status_id`, `damage`, `angle`, `knockback_scale`,
//! `knockback_weight`, `knockback_base`, `element`.
//!
//! [`special_colls_bytes`] reads the `FTSpecialColl` records the
//! reflector and absorber statuses attach, from the same files: `kind`,
//! `joint_id` (`s32`), `offset`, `size` (`Vec3f`), `damage_resist` (`s32`).

use alloc::vec::Vec;

use crate::item_attributes::{name, ELEMENTS};
use crate::source::be_u32;
use crate::tables::{le_bytes, le_words, Ctx};
use crate::{Error, Result};

/// `(name, file, offset)`: the `ssb_game::grab::throw_descs` name, the
/// `*MainMotion` file and the table's offset in it.
pub const DESCS: [(&str, u32, usize); 40] = [
    ("MARIO_CATCH", 202, 0xBB4),
    ("MARIO_THROW_F", 202, 0xC30),
    // The US descriptor (`#else` branch).
    ("MARIO_THROW_B", 202, 0xCC4),
    ("FOX_CATCH", 208, 0xC20),
    ("FOX_THROW_F", 208, 0xC8C),
    ("FOX_THROW_B", 208, 0xD20),
    ("DONKEY_CATCH", 212, 0xC14),
    ("DONKEY_THROW_FF", 212, 0xCF0),
    ("DONKEY_THROW_B", 212, 0xD88),
    ("SAMUS_CATCH", 216, 0xD34),
    ("SAMUS_THROW_F", 216, 0xE20),
    ("SAMUS_THROW_B", 216, 0xEB8),
    ("LUIGI_CATCH", 220, 0xBF0),
    ("LUIGI_THROW_F", 220, 0xC6C),
    ("LUIGI_THROW_B", 220, 0xD00),
    ("LINK_CATCH", 224, 0xE5C),
    ("LINK_THROW_F", 224, 0xF34),
    ("LINK_THROW_B", 224, 0x1008),
    ("KIRBY_CATCH", 228, 0xE4C),
    ("KIRBY_THROW_F", 228, 0xED8),
    ("KIRBY_THROW_B", 228, 0xFC8),
    // `dKirbyMainMotion_0x1C94`: the Inhale script's `SetThrow`.
    ("KIRBY_INHALE", 228, 0x1C94),
    // `dKirbyMainMotion_0x21A0`: the copied Egg Lay's `SetThrow`.
    ("KIRBY_EGG_LAY", 228, 0x21A0),
    ("PURIN_CATCH", 232, 0xCD0),
    ("PURIN_THROW_F", 232, 0xD64),
    ("PURIN_THROW_B", 232, 0xE54),
    ("CAPTAIN_CATCH", 235, 0xD28),
    ("CAPTAIN_THROW_F", 235, 0xDA4),
    ("CAPTAIN_THROW_B", 235, 0xE50),
    // `dCaptainMainMotion_0x1C54`: Falcon Dive.
    ("CAPTAIN_DIVE", 235, 0x1C54),
    ("NESS_CATCH", 238, 0xBCC),
    ("NESS_THROW_F", 238, 0xC4C),
    ("NESS_THROW_B", 238, 0xD38),
    ("PIKACHU_CATCH", 242, 0xC18),
    ("PIKACHU_THROW_F", 242, 0xC88),
    ("PIKACHU_THROW_B", 242, 0xD1C),
    ("YOSHI_CATCH", 246, 0xCFC),
    ("YOSHI_THROW_F", 246, 0xD90),
    ("YOSHI_THROW_B", 246, 0xE2C),
    // `dYoshiMainMotion_0x16F8`: the Egg Lay script's `SetThrow`.
    ("YOSHI_EGG_LAY", 246, 0x16F8),
];

const DESC_SIZE: usize = 7 * 4;

/// `status_id` as an index: `None`, then `nFTCommonStatus{DamageFlyN,
/// DamageFlyTop, DamageFlyRoll}`, the only statuses the tables force.
fn status(id: i32, file: u32, at: usize) -> Result<i32> {
    Ok(match id {
        -1 => 0,
        52 => 1,
        54 => 2,
        55 => 3,
        _ => return Err(Error::BadRelocation { file, offset: at }),
    })
}

/// One descriptor: `status` ([`status`] index), `damage, angle, kb_scale,
/// kb_weight, kb_base`, `element` ([`ELEMENTS`] index).
fn desc(data: &[u8], file: u32, at: usize) -> Result<[i32; 7]> {
    let word = |i: usize| be_u32(data, at + 4 * i).map(|v| v as i32);
    let element = name(&ELEMENTS, word(6)? as u32, file, at + 24)? as i32;
    Ok([
        status(word(0)?, file, at)?,
        word(1)?,
        word(2)?,
        word(3)?,
        word(4)?,
        word(5)?,
        element,
    ])
}

/// `TableId::ThrowDescs`: both descriptors of each [`DESCS`] table.
pub fn descs_bytes(c: &Ctx) -> Result<Vec<u8>> {
    let mut out = Vec::with_capacity(DESCS.len() * 14);
    for (_, file, at) in DESCS {
        let data = c.rom.file(file)?.data;
        out.extend(desc(&data, file, at)?);
        out.extend(desc(&data, file, at + DESC_SIZE)?);
    }
    Ok(le_bytes(&out, i32::to_le_bytes))
}

/// `(name, file, offset)` of each `ll*FTSpecialColl` (`reloc_data.us.h`).
pub const SPECIAL_COLLS: [(&str, u32, usize); 3] = [
    // `llFoxMainMotionLwReflectorFTSpecialColl`.
    ("FOX_REFLECTOR", 208, 0x19B0),
    // `llNessMainMotionAttackS4ReflectorFTSpecialColl`.
    ("NESS_BAT_REFLECTOR", 238, 0x1114),
    // `llNessMainMotionLwAbsorbFTSpecialColl`.
    ("NESS_ABSORB", 238, 0x16D4),
];

/// `nFTSpecialCollKind*`, as `ssb_game::combat::SpecialCollKind` names them.
pub const SPECIAL_COLL_KINDS: [&str; 3] = ["FoxReflector", "NessAbsorb", "NessReflector"];

/// One `FTSpecialColl`: `kind` ([`SPECIAL_COLL_KINDS`] index), `joint`,
/// `offset`, `size` (f32 bits), `damage_resist`.
fn special_coll(data: &[u8], file: u32, at: usize) -> Result<[u32; 9]> {
    let word = |i: usize| be_u32(data, at + 4 * i);
    let kind = name(&SPECIAL_COLL_KINDS, word(0)?, file, at)?;
    let joint = u8::try_from(word(1)?).map_err(|_| Error::BadRelocation {
        file,
        offset: at + 4,
    })?;
    let mut w = [0; 9];
    w[0] = kind;
    w[1] = joint.into();
    for (i, v) in w.iter_mut().enumerate().skip(2) {
        *v = word(i)?;
    }
    Ok(w)
}

/// `TableId::SpecialColls`: each [`SPECIAL_COLLS`] record.
pub fn special_colls_bytes(c: &Ctx) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    for (_, file, at) in SPECIAL_COLLS {
        out.extend(special_coll(&c.rom.file(file)?.data, file, at)?);
    }
    Ok(le_words(&out))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn descriptor_decodes_signed_words() {
        let mut data = vec![0u8; DESC_SIZE];
        for (i, v) in [52i32, 18, 45, 80, 0, 60, 2].into_iter().enumerate() {
            data[4 * i..4 * i + 4].copy_from_slice(&v.to_be_bytes());
        }
        assert_eq!(desc(&data, 242, 0).unwrap(), [1, 18, 45, 80, 0, 60, 2]);
        data[..4].copy_from_slice(&53i32.to_be_bytes());
        assert!(desc(&data, 242, 0).is_err());
    }

    #[test]
    fn special_coll_decodes_kind_joint_and_floats() {
        let mut data = vec![0u8; 36];
        data[3] = 2;
        data[12..16].copy_from_slice(&150.0f32.to_bits().to_be_bytes());
        data[32..36].copy_from_slice(&1000i32.to_be_bytes());
        let c = special_coll(&data, 238, 0).unwrap();
        assert_eq!(c[..2], [2, 0]);
        assert_eq!(f32::from_bits(c[3]), 150.0);
        assert_eq!(c[8] as i32, 1000);
        data[7] = 1;
        data[6] = 1;
        assert!(special_coll(&data, 238, 0).is_err());
    }
}
