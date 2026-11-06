//! The opening's fighter scenes' data arrays (`d*` symbols in the scenes'
//! overlays, `mv/mvopening/mvopening*.c`): each scene's start and end
//! `CObjDesc` and its fighter's `FTKeyEvent` script, and the jungle's two
//! scripts.
//!
//! Each table's run-time address is its symbol in
//! `refs/ssb-decomp-re/symbols/symbols_us.txt`; the overlay is the one the
//! defining file's `.data` sits in (`smashbrothers.us.yaml`).

use alloc::vec::Vec;

use crate::source::{
    Overlay, OverlayData, OVL36, OVL37, OVL38, OVL39, OVL40, OVL41, OVL42, OVL43, OVL51,
};
use crate::tables::{le_bytes, le_words, Ctx};
use crate::Result;

/// One scene's arrays: `dMVOpening<Name>CObjDescStart` (`CObjDesc`, 28 bytes:
/// eye, at, `up.x`), `...End` and `...KeyEvents` (`FTKeyEvent[]` halfwords,
/// the script's length counting its `FTKEY_EVENT_END`).
pub struct Scene {
    pub name: &'static str,
    pub overlay: Overlay,
    pub start: u32,
    pub end: u32,
    pub keys: u32,
    pub key_halfwords: usize,
}

/// The eight fighter scenes.
pub const SCENES: [Scene; 8] = [
    Scene {
        name: "MARIO",
        overlay: OVL36,
        start: 0x8018_E090,
        end: 0x8018_E0AC,
        keys: 0x8018_E0C8,
        key_halfwords: 15,
    },
    Scene {
        name: "DONKEY",
        overlay: OVL37,
        start: 0x8018_E070,
        end: 0x8018_E08C,
        keys: 0x8018_E0A8,
        key_halfwords: 9,
    },
    Scene {
        name: "SAMUS",
        overlay: OVL38,
        start: 0x8018_E120,
        end: 0x8018_E13C,
        keys: 0x8018_E158,
        key_halfwords: 5,
    },
    Scene {
        name: "FOX",
        overlay: OVL39,
        start: 0x8018_E090,
        end: 0x8018_E0AC,
        keys: 0x8018_E0C8,
        key_halfwords: 11,
    },
    Scene {
        name: "LINK",
        overlay: OVL40,
        start: 0x8018_E070,
        end: 0x8018_E08C,
        keys: 0x8018_E0A8,
        key_halfwords: 3,
    },
    Scene {
        name: "YOSHI",
        overlay: OVL41,
        start: 0x8018_E0C0,
        end: 0x8018_E0DC,
        keys: 0x8018_E0F8,
        key_halfwords: 7,
    },
    Scene {
        name: "PIKACHU",
        overlay: OVL42,
        start: 0x8018_E0C0,
        end: 0x8018_E0DC,
        keys: 0x8018_E0F8,
        key_halfwords: 1,
    },
    Scene {
        name: "KIRBY",
        overlay: OVL43,
        start: 0x8018_E0B0,
        end: 0x8018_E0CC,
        keys: 0x8018_E0E8,
        key_halfwords: 5,
    },
];
/// `CObjDesc`'s size, checked against the layout.
#[cfg(test)]
const COBJ_DESC_SIZE: u32 = 28;

/// `dMVOpeningJungleDonkeyKeyEvents` and `...SamusKeyEvents` (`ovl51`,
/// `mvopeningjungle.c`), in halfwords.
pub const JUNGLE_KEYS: [(&str, u32, usize); 2] = [
    ("JUNGLE_DONKEY", 0x8018_D8A8, 35),
    ("JUNGLE_SAMUS", 0x8018_D8F0, 33),
];

/// `FTKEY_EVENT_END` (`nFTKeyEventEnd`); the button and stick events take a
/// second halfword.
const KEY_END: u16 = 0;

/// Halfwords in the `OpeningKeys` table: the eight scenes' scripts in
/// [`SCENES`] order, then [`JUNGLE_KEYS`].
pub const KEY_HALFWORDS: usize = {
    let mut n = 0;
    let mut i = 0;
    while i < SCENES.len() {
        n += SCENES[i].key_halfwords;
        i += 1;
    }
    n + JUNGLE_KEYS[0].2 + JUNGLE_KEYS[1].2
};

/// `ssb-game`'s `OpeningCameras` table: each scene's
/// `dMVOpening*CObjDescStart` then `...End` (eye, at, `up.x`), in [`SCENES`]
/// order.
pub fn cameras_bytes(c: &Ctx) -> Result<Vec<u8>> {
    let mut words = Vec::new();
    for s in &SCENES {
        let d = OverlayData::new(Some(c.rom), s.overlay);
        for at in [s.start, s.end] {
            for k in 0..7 {
                words.push(d.u32(at + k * 4)?);
            }
        }
    }
    Ok(le_words(&words))
}

/// The `OpeningKeys` table: the eight scenes' `dMVOpening*KeyEvents`, then
/// the jungle's two scripts, as halfwords ([`KEY_HALFWORDS`]).
pub fn keys_bytes(c: &Ctx) -> Result<Vec<u8>> {
    let mut halves = Vec::with_capacity(KEY_HALFWORDS);
    for s in &SCENES {
        let d = OverlayData::new(Some(c.rom), s.overlay);
        keys(&mut halves, d, s.name, s.keys, s.key_halfwords)?;
    }
    let d = OverlayData::new(Some(c.rom), OVL51);
    for (name, at, n) in JUNGLE_KEYS {
        keys(&mut halves, d, name, at, n)?;
    }
    Ok(le_bytes(&halves, u16::to_le_bytes))
}

/// An `FTKeyEvent` script as halfwords, checked to end exactly at `n`
/// halfwords.
fn keys(out: &mut Vec<u16>, d: OverlayData, name: &str, at: u32, n: usize) -> Result<()> {
    let halves = (0..n as u32)
        .map(|i| d.u16(at + i * 2))
        .collect::<Result<Vec<_>>>()?;
    let mut i = 0;
    while halves[i] != KEY_END {
        i += 2;
        assert!(i < n, "{name} keys run past its {n} halfwords");
    }
    assert_eq!(i + 1, n, "{name} keys end before its {n} halfwords");
    out.extend_from_slice(&halves);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_keys_are_124_halfwords() {
        assert_eq!(KEY_HALFWORDS, 15 + 9 + 5 + 11 + 3 + 7 + 1 + 5 + 35 + 33);
    }

    #[test]
    fn each_scene_lays_its_arrays_end_to_end() {
        for s in &SCENES {
            assert_eq!(s.start + COBJ_DESC_SIZE, s.end, "{}", s.name);
            assert_eq!(s.end + COBJ_DESC_SIZE, s.keys, "{}", s.name);
        }
    }
}
