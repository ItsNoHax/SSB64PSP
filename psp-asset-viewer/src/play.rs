//! Viewer-only additions to the shared fighter scene: status-label overlay
//! text.
//!
//! The pack-to-game bridge itself ([`Play`], [`FloorSegments`], ...) lives in
//! `ssb_psp_runtime::scene`, shared with `psp-game`. This module only adds
//! what belongs to the debug viewer's own diagnostic overlay -- keeping it
//! here, not in `psp-runtime`, per the rule that status-label overlays are
//! viewer diagnostics, not shared runtime.

pub use ssb_psp_runtime::scene::{FloorSegments, Play};

use ssb_game::status::Status;

/// The status a fighter is in, as a fixed-width label for the overlay.
pub fn status_name(play: &Play) -> &'static str {
    use Status::*;
    match play.fighter.status.status {
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
    }
}

/// The floor's material, or `None` while airborne.
pub fn material(play: &Play) -> Option<u16> {
    play.fighter.floor.map(|f| f.material())
}
