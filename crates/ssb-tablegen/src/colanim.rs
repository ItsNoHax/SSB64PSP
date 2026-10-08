//! The colour-animation scripts and `dGMColScriptsDescs`
//! (`gm/gmcolscripts.c`), for `ssb_game::colanim`.
//!
//! Each script is a run of `gmColCommand*` words (`gm/gmdef.h`): a 6-bit
//! event in the top bits of the first word, its arguments in the rest of
//! that word and in the words after it. They are decoded into
//! `ssb_game::colanim::ColEvent`s, and their pointers into the scripts they
//! name.

use std::fmt::Write;

use crate::emit;
use crate::source::OVL2;
use crate::{Result, Source};

/// ROM offset of the first script, `dGMColScriptsFighterComPlayer`. The
/// scripts follow each other without gaps, in this order.
pub const SCRIPTS_START: u32 = 0x0A_8290;

/// Every `dGMColScripts*` array, in memory order: its name without the
/// `dGMColScripts` prefix, and its size in bytes.
pub const SCRIPTS: [(&str, u32); 93] = [
    ("FighterComPlayer", 20),
    ("FighterHitStatusNormal", 8),
    ("FighterHitStatusIntangible", 36),
    ("FighterHitStatusInvincible", 36),
    ("FighterDamageCommon", 44),
    ("FighterCommonSpecialNCharge", 64),
    ("FighterFallSpecial", 28),
    ("FighterFastFall", 32),
    ("FighterHeal", 64),
    ("FighterNoDamage", 52),
    ("FighterRebirth", 48),
    ("FighterDamageFireSub1", 28),
    ("FighterDamageFireSub2", 36),
    ("FighterDamageFireWeak", 68),
    ("FighterDamageFireMid", 68),
    ("FighterDamageFireStrong", 68),
    ("FighterDamageFireFly", 68),
    ("FighterDamageElectricCommonSub", 36),
    ("FighterDamageElectricCommonWeak", 36),
    ("FighterDamageElectricCommonMid", 36),
    ("FighterDamageElectricCommonStrong", 36),
    ("FighterDamageElectricCommonFly", 36),
    ("FighterDamageElectricSkeletonSub", 44),
    ("FighterDamageElectricSkeletonWeak", 68),
    ("FighterDamageElectricSkeletonMid", 68),
    ("FighterDamageElectricSkeletonStrong", 68),
    ("FighterDamageElectricSkeletonFly", 68),
    ("FighterDamageElectricBalloonSub", 44),
    ("FighterDamageElectricBalloonWeak", 68),
    ("FighterDamageElectricBalloonMid", 68),
    ("FighterDamageElectricBalloonStrong", 68),
    ("FighterDamageElectricBalloonFly", 68),
    ("FighterDamageElectricSamusSub", 48),
    ("FighterDamageElectricSamusWeak", 68),
    ("FighterDamageElectricSamusMid", 68),
    ("FighterDamageElectricSamusStrong", 68),
    ("FighterDamageElectricSamusFly", 68),
    ("FighterDamageIceWeak", 28),
    ("FighterDamageIceMid", 4),
    ("FighterDamageIceStrong", 4),
    ("FighterDamageIceFly", 4),
    ("FighterShieldBreakFly", 40),
    ("FighterFuraFura", 44),
    ("FighterFuraSleep", 48),
    ("FighterMarioSpecialN", 44),
    ("FighterMarioAppeal", 60),
    ("FighterDonkeySpecialNLoop", 40),
    ("FighterDonkeySpecialNEnd", 36),
    ("FighterUnknown1", 4),
    ("FighterSamusSpecialNEnd", 32),
    ("FighterSamusSpecialHi", 56),
    ("FighterFoxSpecialLw", 32),
    ("FighterFoxSpecialHiStart", 84),
    ("FighterFoxSpecialHiHold", 40),
    ("FighterFoxSpecialHi", 44),
    ("FighterLinkSpecialHi", 52),
    ("FighterCaptainSpecialN", 80),
    ("FighterKirbySpecialLwHigh", 40),
    ("FighterKirbySpecialLwMid", 40),
    ("FighterKirbySpecialLwLow", 40),
    ("FighterKirbySpecialLwStart", 124),
    ("FighterKirbySpecialLwEnd", 124),
    ("FighterPikachuAttackS4", 56),
    ("FighterPikachuSpecialHiStart", 4),
    ("FighterPikachuSpecialHiStartLoop", 32),
    ("FighterPikachuSpecialHi", 28),
    ("FighterPikachuSpecialN", 72),
    ("FighterPikachuSpecialLwHit", 88),
    ("FighterPikachuSpecialLwEnd", 40),
    ("FighterCaptainSpecialHi", 44),
    ("FighterNessSpecialLwHold", 32),
    ("FighterNessSpecialLwHit", 52),
    ("FighterNessSpecialHiHold", 56),
    ("FighterNessSpecialHiJibaku", 96),
    ("FighterNessAppear", 32),
    ("FighterBossDeskArrange", 24),
    ("FighterBossOkuhikouki", 28),
    ("FighterBossOkupunch", 32),
    ("FighterBossYubideppou2", 40),
    ("FighterBossYubideppou3", 56),
    ("FighterChallenger", 24),
    ("FighterHammer", 40),
    ("FighterStar", 144),
    ("FighterStarRod", 28),
    ("FighterBat", 60),
    ("ItemBombHeiCritical", 48),
    ("ItemHammerEnd", 32),
    ("ItemLinkBombCritical", 48),
    ("ScreenFlashDeadExplode", 40),
    ("ScreenFlashDamageNormal", 24),
    ("ScreenFlashDamageFire", 24),
    ("ScreenFlashDamageElectric", 24),
    ("ScreenFlashDamageIce", 24),
];

/// The colour-animation ids the port names, after the script each one's
/// `GMColDesc` selects (`ssb_game::colanim::ColAnimId::*`). The ROM's table
/// must agree; id 0 selects no script. (`GMColAnimKind`'s own names for ids
/// 41 to 49 do not match their scripts, so the script names are used.)
pub const ID_NAMES: [(u8, &str); 85] = [
    (1, "FighterComPlayer"),
    (2, "FighterHitStatusNormal"),
    (3, "FighterHitStatusIntangible"),
    (4, "FighterHitStatusInvincible"),
    (5, "FighterDamageCommon"),
    (6, "FighterCommonSpecialNCharge"),
    (7, "FighterFallSpecial"),
    (8, "FighterFastFall"),
    (9, "FighterHeal"),
    (10, "FighterNoDamage"),
    (11, "FighterRebirth"),
    (12, "FighterDamageFireWeak"),
    (13, "FighterDamageFireMid"),
    (14, "FighterDamageFireStrong"),
    (15, "FighterDamageFireFly"),
    (16, "FighterDamageElectricCommonWeak"),
    (17, "FighterDamageElectricCommonMid"),
    (18, "FighterDamageElectricCommonStrong"),
    (19, "FighterDamageElectricCommonFly"),
    (20, "FighterDamageElectricSkeletonWeak"),
    (21, "FighterDamageElectricSkeletonMid"),
    (22, "FighterDamageElectricSkeletonStrong"),
    (23, "FighterDamageElectricSkeletonFly"),
    (24, "FighterDamageElectricSamusWeak"),
    (25, "FighterDamageElectricSamusMid"),
    (26, "FighterDamageElectricSamusStrong"),
    (27, "FighterDamageElectricSamusFly"),
    (28, "FighterDamageElectricBalloonWeak"),
    (29, "FighterDamageElectricBalloonMid"),
    (30, "FighterDamageElectricBalloonStrong"),
    (31, "FighterDamageElectricBalloonFly"),
    (32, "FighterDamageIceWeak"),
    (33, "FighterDamageIceMid"),
    (34, "FighterDamageIceStrong"),
    (35, "FighterDamageIceFly"),
    (36, "FighterShieldBreakFly"),
    (37, "FighterFuraFura"),
    (38, "FighterFuraSleep"),
    (39, "FighterMarioSpecialN"),
    (40, "FighterMarioAppeal"),
    (41, "FighterFoxSpecialLw"),
    (42, "FighterFoxSpecialHiStart"),
    (43, "FighterFoxSpecialHi"),
    (44, "FighterLinkSpecialHi"),
    (45, "FighterSamusSpecialHi"),
    (46, "FighterUnknown1"),
    (47, "FighterSamusSpecialNEnd"),
    (48, "FighterDonkeySpecialNLoop"),
    (49, "FighterDonkeySpecialNEnd"),
    (50, "FighterCaptainSpecialN"),
    (51, "FighterKirbySpecialLwHigh"),
    (52, "FighterKirbySpecialLwMid"),
    (53, "FighterKirbySpecialLwLow"),
    (54, "FighterKirbySpecialLwStart"),
    (55, "FighterKirbySpecialLwEnd"),
    (56, "FighterPikachuAttackS4"),
    (57, "FighterPikachuSpecialHiStart"),
    (58, "FighterPikachuSpecialHi"),
    (59, "FighterPikachuSpecialN"),
    (60, "FighterPikachuSpecialLwHit"),
    (61, "FighterPikachuSpecialLwEnd"),
    (62, "FighterCaptainSpecialHi"),
    (63, "FighterNessSpecialLwHold"),
    (64, "FighterNessSpecialLwHit"),
    (65, "FighterNessSpecialHiHold"),
    (66, "FighterNessSpecialHiJibaku"),
    (67, "FighterNessAppear"),
    (68, "FighterBossDeskArrange"),
    (69, "FighterBossOkuhikouki"),
    (70, "FighterBossOkupunch"),
    (71, "FighterBossYubideppou2"),
    (72, "FighterBossYubideppou3"),
    (73, "FighterHammer"),
    (74, "FighterStar"),
    (75, "FighterStarRod"),
    (76, "FighterBat"),
    (77, "ItemBombHeiCritical"),
    (78, "ItemHammerEnd"),
    (79, "ItemLinkBombCritical"),
    (80, "FighterChallenger"),
    (81, "ScreenFlashDeadExplode"),
    (82, "ScreenFlashDamageNormal"),
    (83, "ScreenFlashDamageFire"),
    (84, "ScreenFlashDamageElectric"),
    (85, "ScreenFlashDamageIce"),
];

/// `dGMColScriptsDescs`: ROM offset and entry count. A `GMColDesc` is a
/// script pointer, `u8 priority` and `ub8 is_unlocked`, padded to 8 bytes.
pub const DESCS: (u32, usize) = (0x0A_93D0, 86);
const DESC_SIZE: u32 = 8;

fn sign(v: u32, bits: u32) -> i32 {
    ((v << (32 - bits)) as i32) >> (32 - bits)
}

/// `name` in `SCREAMING_SNAKE_CASE`: an underscore before an upper-case
/// letter that follows a lower-case letter or digit, or that starts a word
/// after an acronym.
pub fn screaming(name: &str) -> String {
    let c: Vec<char> = name.chars().collect();
    let mut out = String::new();
    for i in 0..c.len() {
        if i > 0 {
            let (p, x) = (c[i - 1], c[i]);
            let next_lower = c.get(i + 1).is_some_and(|n| n.is_ascii_lowercase());
            if ((p.is_ascii_lowercase() || p.is_ascii_digit()) && x.is_ascii_uppercase())
                || (p.is_ascii_uppercase() && x.is_ascii_uppercase() && next_lower)
            {
                out.push('_');
            }
        }
        out.push(c[i].to_ascii_uppercase());
    }
    out
}

struct Scripts {
    /// ROM offset of each script's first word.
    starts: Vec<u32>,
}

impl Scripts {
    fn new() -> Self {
        let mut at = SCRIPTS_START;
        let starts = SCRIPTS
            .iter()
            .map(|(_, size)| {
                let start = at;
                at += size;
                start
            })
            .collect();
        Self { starts }
    }

    /// The script a run-time pointer names.
    fn named(&self, vram: u32) -> &'static str {
        let at = OVL2
            .rom_of(vram)
            .expect("colour script pointer outside ovl2");
        let i = self
            .starts
            .iter()
            .position(|s| *s == at)
            .unwrap_or_else(|| panic!("0x{vram:08X} is not a colour script's first event"));
        SCRIPTS[i].0
    }
}

fn rgba(w: u32) -> String {
    let b = w.to_be_bytes();
    format!(
        "[0x{:02X}, 0x{:02X}, 0x{:02X}, 0x{:02X}]",
        b[0], b[1], b[2], b[3]
    )
}

/// Script `i`'s events, as Rust expressions.
fn events(rom: &Source, scripts: &Scripts, i: usize) -> Result<Vec<String>> {
    let (name, size) = SCRIPTS[i];
    let start = scripts.starts[i];
    let end = start + size;
    let mut out = Vec::new();
    let mut at = start;
    let next = |at: &mut u32| -> Result<u32> {
        assert!(*at < end, "{name}: an event runs past the script");
        let w = rom.u32(*at)?;
        *at += 4;
        Ok(w)
    };
    while at < end {
        let w = next(&mut at)?;
        let arg = w & 0x03FF_FFFF;
        let ev = match w >> 26 {
            0 => "End".to_string(),
            1 => format!("Wait({arg})"),
            2 => format!("Goto(Script::{})", scripts.named(next(&mut at)?)),
            3 => format!("LoopBegin({arg})"),
            4 => "LoopEnd".to_string(),
            5 => format!("Subroutine(Script::{})", scripts.named(next(&mut at)?)),
            6 => "Return".to_string(),
            8 => "ClearColorAll".to_string(),
            9 => format!("SetColor1({})", rgba(next(&mut at)?)),
            // `gmColCommandBlendColor2` writes `nGMColEventBlendColor1` too.
            10 => format!("BlendColor1({arg}, {})", rgba(next(&mut at)?)),
            11 => format!("SetColor2({})", rgba(next(&mut at)?)),
            op @ (13 | 14) => {
                let (a, b, c) = (next(&mut at)?, next(&mut at)?, next(&mut at)?);
                let hi = |x: u32| sign(x >> 16, 16);
                let lo = |x: u32| sign(x & 0xFFFF, 16);
                format!(
                    "Effect(ColEffect {{ joint: {}, kind: {}, flag: {}, offset: [{}, {}, {}], \
                     scatter: [{}, {}, {}], item_hold: {} }})",
                    sign((w >> 19) & 0x7F, 7),
                    (w >> 10) & 0x1FF,
                    w & 0x3FF,
                    hi(a),
                    lo(a),
                    hi(b),
                    lo(b),
                    hi(c),
                    lo(c),
                    op == 14
                )
            }
            15 => format!(
                "SetLight({}, {})",
                sign((w >> 13) & 0x1FFF, 13),
                sign(w & 0x1FFF, 13)
            ),
            16 => "ClearLight".to_string(),
            17 => "PlayFgm".to_string(),
            18 => format!("SetSkeletonId({arg})"),
            op => panic!("{name}: colour event {op} at 0x{:X}", at - 4),
        };
        out.push(ev);
    }
    Ok(out)
}

pub fn generate(rom: Option<&Source>) -> Result<String> {
    let scripts = Scripts::new();
    let mut w = emit::header("the colour-animation scripts and dGMColScriptsDescs");
    // A stub has no `Effect` events to name `ColEffect`.
    w.push_str(if rom.is_some() {
        "use super::{ColDesc, ColEffect, ColEvent, ColEvent::*};\n\n"
    } else {
        "use super::{ColDesc, ColEvent, ColEvent::*};\n\n"
    });
    w.push_str(
        "/// One `dGMColScripts*` array.\n\
         #[derive(Debug, Clone, Copy, PartialEq, Eq)]\n\
         pub enum Script {\n",
    );
    for (name, _) in SCRIPTS {
        let _ = writeln!(w, "    /// `dGMColScripts{name}`.\n    {name},");
    }
    w.push_str("}\n\nimpl Script {\n    /// The script's events.\n");
    w.push_str("    pub fn events(self) -> &'static [ColEvent] {\n        match self {\n");
    for (name, _) in SCRIPTS {
        let _ = writeln!(w, "            Script::{name} => &{},", screaming(name));
    }
    w.push_str("        }\n    }\n}\n\n");
    for (i, (name, _)) in SCRIPTS.iter().enumerate() {
        let mut evs = match rom {
            Some(rom) => events(rom, &scripts, i)?,
            None => vec!["End".to_string()],
        };
        let last = evs.last().map(String::as_str).unwrap_or("");
        if !(last == "End" || last.starts_with("Goto(") || last == "Return") {
            // Runs off its end into the next array, which is what a `Goto`
            // to that array does.
            evs.push(format!("Goto(Script::{})", SCRIPTS[i + 1].0));
        }
        let _ = writeln!(
            w,
            "/// `dGMColScripts{name}`.\nstatic {}: [ColEvent; {}] = [",
            screaming(name),
            evs.len()
        );
        for e in evs {
            let _ = writeln!(w, "    {e},");
        }
        w.push_str("];\n\n");
    }

    let (table, count) = DESCS;
    let mut descs: Vec<(Option<&str>, u8, bool)> = Vec::new();
    for i in 0..count as u32 {
        descs.push(match rom {
            Some(rom) => {
                let at = table + i * DESC_SIZE;
                let ptr = rom.u32(at)?;
                let script = (ptr != 0).then(|| scripts.named(ptr));
                (script, rom.u8(at + 4)?, rom.u8(at + 5)? != 0)
            }
            None => (None, 0, false),
        });
    }
    w.push_str("/// `dGMColScriptsDescs`, indexed by colour-animation id.\n");
    let _ = writeln!(w, "pub static DESCS: [ColDesc; {count}] = [");
    for (script, priority, unlocked) in &descs {
        let script = script.map_or("None".to_string(), |s| format!("Some(Script::{s})"));
        let _ = writeln!(
            w,
            "    ColDesc {{ script: {script}, priority: {priority}, is_unlocked: {unlocked} }},"
        );
    }
    w.push_str("];\n\n");
    w.push_str("impl super::ColAnimId {\n");
    if rom.is_some() {
        let named: Vec<(u8, &str)> = descs
            .iter()
            .enumerate()
            .filter_map(|(i, (script, _, _))| script.map(|s| (i as u8, s)))
            .collect();
        assert_eq!(named, ID_NAMES, "dGMColScriptsDescs names other scripts");
    }
    for (i, name) in ID_NAMES {
        let _ = writeln!(
            w,
            "    /// Id {i}: `dGMColScripts{name}`.\n    pub const {}: Self = Self({i});",
            screaming(name)
        );
    }
    w.push_str("}\n");
    Ok(w)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_become_screaming_snake_case() {
        assert_eq!(screaming("FighterComPlayer"), "FIGHTER_COM_PLAYER");
        assert_eq!(
            screaming("FighterDamageFireSub1"),
            "FIGHTER_DAMAGE_FIRE_SUB1"
        );
        assert_eq!(screaming("ScreenFlashDamageIce"), "SCREEN_FLASH_DAMAGE_ICE");
    }

    #[test]
    fn the_scripts_end_where_the_descs_begin() {
        let total: u32 = SCRIPTS.iter().map(|(_, size)| size).sum();
        assert_eq!(SCRIPTS_START + total, DESCS.0);
    }
}
