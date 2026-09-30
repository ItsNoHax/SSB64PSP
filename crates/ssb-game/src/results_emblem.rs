//! The VS results' series emblem and confetti (RE-420):
//! `mnVSResultsMakeEmblem`, `mnVSResultsEmblemProcUpdate`,
//! `mnVSResultsMakeEmblemCamera` and `mnVSResultsMakeConfetti`
//! (`mn/mnvsmode/mnvsresults.c`).
//!
//! The emblem is the winner's `FTEmblemModels` tree (file 35), lit by the
//! scene's fighter light, its colour the frame of its material animation
//! that `gcAddMatAnimJointAll(gobj, joints, color)` starts at. Only one
//! `gcPlayAnimAll` ever runs, so the colour holds. The confetti is two
//! `efcommon` particle scripts the host's particle runtime plays.

use crate::fighter::FighterKind;
use crate::results::{Kind, Results};
use ssb_engine::math::Vec3;

/// `mnVSResultsMakeEmblem`'s DObj: `translate` and the x and y of `scale`
/// (z stays 1).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Emblem {
    /// The winner's kind: `dobjdescs[win_fkind]`.
    pub fighter: FighterKind,
    /// The material animation's start frame: the winner's port, or
    /// `colors[team]` (`{0, 1, 3}`) in a team battle.
    pub color: u8,
    pub translate: Vec3,
    pub scale: f32,
}

/// `DObjGetStruct(gobj)->translate`: (0, 100, -11000).
pub const START_TRANSLATE: Vec3 = Vec3::new(0.0, 100.0, -11000.0);
/// `scale.vec.f.x` and `.y`.
pub const START_SCALE: f32 = 25.0;
/// `mnVSResultsEmblemProcUpdate` starts on this tic.
pub const MOVE_TIC: u32 = 40;
/// `min_scale`, and the shrink per tic.
pub const MIN_SCALE: f32 = 10.0;
pub const SHRINK: f32 = 0.15;
/// `max_y`, and the rise per tic.
pub const MAX_Y: f32 = 1000.0;
pub const RISE: f32 = 11.0;

impl Emblem {
    /// `mnVSResultsFuncStart`'s `mnVSResultsMakeEmblem`, made unless no
    /// contest. `winner_kind` is `mnVSResultsGetFighterKind(win_player)`.
    pub fn make(r: &Results, winner_kind: FighterKind) -> Option<Emblem> {
        if r.kind == Kind::NoContest {
            return None;
        }
        let winner = r.winner.unwrap_or(0);
        let color = if r.is_team_battle {
            crate::results_layer::team_color(r, winner)
        } else {
            winner
        };
        Some(Emblem {
            fighter: winner_kind,
            color: color as u8,
            translate: START_TRANSLATE,
            scale: START_SCALE,
        })
    }

    /// `mnVSResultsEmblemProcUpdate`, after `mnVSResultsFuncRun` has
    /// counted the tic: from tic 40 the emblem shrinks to scale 10 and
    /// rises to y 1000.
    pub fn update(&mut self, r: &Results) {
        if r.total_tics < MOVE_TIC {
            return;
        }
        if MIN_SCALE < self.scale {
            self.scale = (self.scale - SHRINK).max(MIN_SCALE);
        }
        if self.translate.y < MAX_Y {
            self.translate.y = (self.translate.y + RISE).min(MAX_Y);
        }
    }
}

/// `mnVSResultsMakeEmblemCamera`: priority 60 (between `WallpaperTint2`
/// and `WallpaperTint`), DL link 33, and the same eye, target and
/// `dGCPerspDefault` projection as the fighters' camera.
pub const CAMERA: crate::results_scene::Camera = crate::results_scene::CAMERA;

/// `mnVSResultsMakeConfetti`, on `sMNVSResultsMakeResultsTic` after a
/// contest: `pos0` with `is_genlink_mask` false (the bank ORed with
/// `LBPARTICLE_MASK_GENLINK(3)`, list 4), then `pos1` with it true (list 0).
pub const CONFETTI: [(Vec3, bool); 2] = [
    (Vec3::new(0.0, 1000.0, -1000.0), false),
    (Vec3::new(0.0, 1000.0, -400.0), true),
];

/// Whether `mnVSResultsFuncRun` makes the confetti on this tic.
pub fn confetti_due(r: &Results) -> bool {
    r.kind != Kind::NoContest && r.total_tics == r.make_results_tic
}

#[cfg(test)]
#[path = "results_emblem_tests.rs"]
mod tests;
