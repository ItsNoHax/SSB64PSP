//! Viewer-only additions to the shared fighter scene: status-label overlay
//! text.
//!
//! The pack-to-game bridge itself ([`FighterScene`], [`FloorSegments`], ...)
//! lives in `ssb_psp_runtime::scene`, shared with `psp-game`. This module
//! only adds what belongs to the debug viewer's own diagnostic overlay --
//! keeping it here, not in `psp-runtime`, per the rule that status-label
//! overlays are viewer diagnostics, not shared runtime.

pub use ssb_psp_runtime::scene::{facing_turn, FighterScene, FloorSegments};

use ssb_game::status::Status;

/// The status a fighter is in, as a fixed-width label for the overlay.
pub fn status_name(scene: &FighterScene) -> &'static str {
    use Status::*;
    match scene.fighter.status.status {
        Wait => "wait    ",
        WalkSlow => "walk-slw",
        WalkMiddle => "walk-mid",
        WalkFast => "walk-fst",
        Dash => "dash    ",
        Run => "run     ",
        RunBrake => "brake   ",
        Turn => "turn    ",
        KneeBend => "jumpsqt ",
        JumpF => "jump-f  ",
        JumpB => "jump-b  ",
        JumpAerialF => "djump-f ",
        JumpAerialB => "djump-b ",
        Fall => "fall    ",
        FallAerial => "fall-a  ",
        Squat => "squat   ",
        SquatWait => "squat-w ",
        LandingLight => "land    ",
        LandingHeavy => "land-hvy",
        Pass => "pass    ",
        Attack11 => "jab1    ",
        DamageN1 => "dmg-n1  ",
        DamageN2 => "dmg-n2  ",
        DamageN3 => "dmg-n3  ",
        DamageAir1 => "dmg-air1",
        DamageAir2 => "dmg-air2",
        DamageAir3 => "dmg-air3",
        DamageFlyN => "dmg-fly ",
        DamageFlyTop => "dmg-flyt",
        DamageFall => "dmg-fall",
        GuardOn => "guard-on",
        Guard => "guard   ",
        GuardOff => "guard-of",
        GuardSetOff => "guard-so",
        ShieldBreakFly => "shld-brk",
        DeadDown => "dead-dn ",
        DeadLeftRight => "dead-lr ",
        DeadUpStar => "dead-up ",
        RebirthDown => "rebrth-d",
        RebirthStand => "rebrth-s",
        RebirthWait => "rebrth-w",
        Sleep => "sleep   ",
        // Every other status is not wired with any behaviour yet
        // (`Status`'s own doc comment) — this overlay just needs a fallback,
        // not real per-status behaviour, so a wildcard is fine here even
        // though the rest of this codebase avoids them on `Status` matches.
        _ => "?       ",
    }
}

/// The floor's material, or `None` while airborne.
pub fn material(scene: &FighterScene) -> Option<u16> {
    scene.fighter.floor.map(|f| f.material())
}
