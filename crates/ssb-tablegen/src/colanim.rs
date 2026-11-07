//! The colour-animation scripts and `dGMColScriptsDescs`
//! (`gm/gmcolscripts.c`), for `ssb_game::colanim`.
//!
//! Each script is a run of `gmColCommand*` words (`gm/gmdef.h`): a 6-bit
//! event in the top bits of the first word, its arguments in the rest of
//! that word and in the words after it. The `ColAnimEvents` table keeps
//! those words, a script pointer replaced by the script's index;
//! `ssb_game::colanim` decodes them into `ColEvent`s once.

use alloc::vec::Vec;

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

/// `name` in `SCREAMING_SNAKE_CASE`: an underscore before an upper-case
/// letter that follows a lower-case letter or digit, or that starts a word
/// after an acronym.
#[cfg(feature = "std")]
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

    /// The index of the script a run-time pointer names.
    fn index(&self, vram: u32) -> usize {
        let at = OVL2
            .rom_of(vram)
            .expect("colour script pointer outside ovl2");
        self.starts
            .iter()
            .position(|s| *s == at)
            .unwrap_or_else(|| panic!("0x{vram:08X} is not a colour script's first event"))
    }
}

/// Events in all scripts, a run-off `Goto` included ([`events`]).
pub const EVENTS: usize = 691;
/// Words per event in the `ColAnimEvents` table: the event's words as in
/// the ROM, a script pointer replaced by the script's index in
/// [`SCRIPTS`], padded with zeros.
pub const EVENT_WORDS: usize = 4;
/// Bytes per entry in the `ColAnimDescs` table: script index + 1 (0 for
/// none), `priority`, `is_unlocked`, pad.
pub const DESC_BYTES: usize = 4;

/// Script `i`'s events, as [`EVENT_WORDS`] words each; a script that does
/// not end in `End`, `Goto` or `Return` gets a `Goto` to the next one (it
/// runs off its end into it, which is what that `Goto` does).
fn events(rom: &Source, scripts: &Scripts, i: usize) -> Result<Vec<[u32; EVENT_WORDS]>> {
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
        let op = w >> 26;
        let more = match op {
            0 | 1 | 3 | 4 | 6 | 8 | 15 | 16 | 17 | 18 => 0,
            // `gmColCommandBlendColor2` writes `nGMColEventBlendColor1` too.
            9..=11 => 1,
            13 | 14 => 3,
            2 | 5 => {
                let target = scripts.index(next(&mut at)?) as u32;
                out.push([w, target, 0, 0]);
                continue;
            }
            op => panic!("{name}: colour event {op} at 0x{:X}", at - 4),
        };
        let mut ev = [w, 0, 0, 0];
        for word in ev.iter_mut().skip(1).take(more) {
            *word = next(&mut at)?;
        }
        out.push(ev);
    }
    let ends = out
        .last()
        .is_some_and(|ev| matches!(ev[0] >> 26, 0 | 2 | 6));
    if !ends {
        out.push([2 << 26, i as u32 + 1, 0, 0]);
    }
    Ok(out)
}

/// Every script's events, in [`SCRIPTS`] order.
fn all_events(rom: &Source) -> Result<Vec<Vec<[u32; EVENT_WORDS]>>> {
    let scripts = Scripts::new();
    let all = (0..SCRIPTS.len())
        .map(|i| events(rom, &scripts, i))
        .collect::<Result<Vec<_>>>()?;
    assert_eq!(all.iter().map(Vec::len).sum::<usize>(), EVENTS);
    Ok(all)
}

/// The `ColAnimEvents` table: every script's events back to back.
pub fn events_bytes(c: &crate::tables::Ctx) -> Result<Vec<u8>> {
    let words: Vec<u32> = all_events(c.rom)?.into_iter().flatten().flatten().collect();
    Ok(crate::tables::le_words(&words))
}

/// The `ColAnimScripts` table: each script's first event in
/// `ColAnimEvents`, then [`EVENTS`].
pub fn scripts_bytes(c: &crate::tables::Ctx) -> Result<Vec<u8>> {
    let mut starts = Vec::with_capacity(SCRIPTS.len() + 1);
    let mut at = 0u16;
    for evs in all_events(c.rom)? {
        starts.push(at);
        at += evs.len() as u16;
    }
    starts.push(at);
    Ok(crate::tables::le_bytes(&starts, u16::to_le_bytes))
}

/// The `ColAnimDescs` table (`dGMColScriptsDescs`); its named scripts must
/// be [`ID_NAMES`].
pub fn descs_bytes(c: &crate::tables::Ctx) -> Result<Vec<u8>> {
    let scripts = Scripts::new();
    let (table, count) = DESCS;
    let mut out = Vec::with_capacity(count * DESC_BYTES);
    let mut named = Vec::new();
    for i in 0..count as u32 {
        let at = table + i * DESC_SIZE;
        let ptr = c.rom.u32(at)?;
        let script = (ptr != 0).then(|| scripts.index(ptr));
        if let Some(s) = script {
            named.push((i as u8, SCRIPTS[s].0));
        }
        out.extend([
            script.map_or(0, |s| s as u8 + 1),
            c.rom.u8(at + 4)?,
            u8::from(c.rom.u8(at + 5)? != 0),
            0,
        ]);
    }
    assert_eq!(named, ID_NAMES, "dGMColScriptsDescs names other scripts");
    Ok(out)
}

/// `ssb_game::colanim`'s layout: the `Script` enum, `Script::ALL` and the
/// `ColAnimId` names.
#[cfg(feature = "std")]
pub fn layout_rs() -> String {
    use std::fmt::Write;
    let mut w = String::from(
        "// Generated by crates/ssb-tablegen (colanim): the colour-animation\n\
         // script names and ids. Layout only; never edit.\n\n",
    );
    w.push_str(
        "/// One `dGMColScripts*` array.\n\
         #[derive(Debug, Clone, Copy, PartialEq, Eq)]\n\
         pub enum Script {\n",
    );
    for (name, _) in SCRIPTS {
        let _ = writeln!(w, "    /// `dGMColScripts{name}`.\n    {name},");
    }
    w.push_str("}\n\nimpl Script {\n    /// Every script, in table order.\n");
    let _ = writeln!(w, "    pub const ALL: [Script; {}] = [", SCRIPTS.len());
    for (name, _) in SCRIPTS {
        let _ = writeln!(w, "        Script::{name},");
    }
    w.push_str("    ];\n}\n\n");
    let _ = writeln!(
        w,
        "/// Events in all scripts.\npub const EVENT_COUNT: usize = {EVENTS};"
    );
    let _ = writeln!(
        w,
        "/// `dGMColScriptsDescs`' entries.\npub const DESC_COUNT: usize = {};\n",
        DESCS.1
    );
    w.push_str("impl super::ColAnimId {\n");
    for (i, name) in ID_NAMES {
        let _ = writeln!(
            w,
            "    /// Id {i}: `dGMColScripts{name}`.\n    pub const {}: Self = Self({i});",
            screaming(name)
        );
    }
    w.push_str("}\n");
    w
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(feature = "std")]
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
