//! The opening's fighter scenes' data arrays (`d*` symbols in the scenes'
//! overlays, `mv/mvopening/mvopening*.c`): each scene's start and end
//! `CObjDesc` and its fighter's `FTKeyEvent` script, and the jungle's two
//! scripts.
//!
//! Each table's run-time address is its symbol in
//! `refs/ssb-decomp-re/symbols/symbols_us.txt`; the overlay is the one the
//! defining file's `.data` sits in (`smashbrothers.us.yaml`).

use std::fmt::Write;

use crate::emit::{self, f32_lit};
use crate::source::{
    Overlay, OverlayData, OVL36, OVL37, OVL38, OVL39, OVL40, OVL41, OVL42, OVL43, OVL51,
};
use crate::{Generated, Result, Source};

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

pub fn generate(rom: Option<&Source>) -> Result<Vec<Generated>> {
    let mut w = emit::header(
        "`dMVOpening*CObjDescStart`, `...End` and `...KeyEvents` of the eight fighter scenes and \
         the jungle's key scripts",
    );
    for s in &SCENES {
        let d = OverlayData::new(rom, s.overlay);
        for (suffix, at) in [("START", s.start), ("END", s.end)] {
            let _ = writeln!(
                w,
                "pub const {}_{suffix}: CObjDesc = {};",
                s.name,
                cobj_desc(d, at)?
            );
        }
        keys(
            &mut w,
            d,
            &format!("{}_KEYS", s.name),
            s.keys,
            s.key_halfwords,
        )?;
    }
    let d = OverlayData::new(rom, OVL51);
    for (name, at, n) in JUNGLE_KEYS {
        keys(&mut w, d, &format!("{name}_KEYS"), at, n)?;
    }
    Ok(vec![Generated {
        name: "opening_scenes.rs",
        text: w,
    }])
}

/// A `CObjDesc` as the port's `desc(v(eye), v(at), up_x)`.
fn cobj_desc(d: OverlayData, at: u32) -> Result<String> {
    let f = |o: u32| d.f32(at + o).map(f32_lit);
    Ok(format!(
        "desc(v({}, {}, {}), v({}, {}, {}), {})",
        f(0)?,
        f(4)?,
        f(8)?,
        f(12)?,
        f(16)?,
        f(20)?,
        f(24)?
    ))
}

/// An `FTKeyEvent` script as halfwords. A ROM read checks that the script
/// ends exactly at `n` halfwords.
fn keys(w: &mut String, d: OverlayData, name: &str, at: u32, n: usize) -> Result<()> {
    let halves = (0..n as u32)
        .map(|i| d.u16(at + i * 2))
        .collect::<Result<Vec<_>>>()?;
    if d.rom.is_some() {
        let mut i = 0;
        while halves[i] != KEY_END {
            i += 2;
            assert!(i < n, "{name} runs past its {n} halfwords");
        }
        assert_eq!(i + 1, n, "{name} ends before its {n} halfwords");
    }
    let _ = writeln!(w, "pub const {name}: [u16; {n}] = [");
    emit::hex_rows_u16(w, &halves, 11);
    w.push_str("];\n");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stubs_have_every_scene() {
        let files = generate(None).unwrap();
        assert_eq!(files.len(), 1);
        let text = &files[0].text;
        assert_eq!(text.matches("CObjDesc = desc(").count(), 16);
        assert_eq!(text.matches("_KEYS: [u16; ").count(), 10);
        assert!(text.contains("pub const JUNGLE_SAMUS_KEYS: [u16; 33]"));
    }

    #[test]
    fn each_scene_lays_its_arrays_end_to_end() {
        for s in &SCENES {
            assert_eq!(s.start + COBJ_DESC_SIZE, s.end, "{}", s.name);
            assert_eq!(s.end + COBJ_DESC_SIZE, s.keys, "{}", s.name);
        }
    }
}
