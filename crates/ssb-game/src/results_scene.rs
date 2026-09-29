//! The VS results' fighters: `mnVSResultsInitFightersAll` and the helpers
//! it calls in `mn/mnvsmode/mnvsresults.c` -- where each present player's
//! fighter stands, how big it is, which way it faces and which demo status
//! (`nFTDemoStatusWin1` to `nFTDemoStatusLose`) it plays -- plus
//! `mnVSResultsMakeFighterCamera`'s camera and the scene's one random pick
//! before the fighters, the wipe (`mnVSResultsFuncStart`).
//!
//! The clips and the drawing are the PSP layer's. [`Results`] carries the
//! rankings and the tics.

use crate::fighter::FighterKind;
use crate::results::{Kind, Results};
use crate::rng;
use ssb_engine::math::Vec3;

/// `ARRAY_COUNT(dLBTransitionDescs)`: the wipes `mnVSResultsFuncStart`
/// picks from.
pub const TRANSITION_COUNT: i32 = 11;

/// `dSCSubsysFighterScales` (`sc/scsubsys/scsubsysdata.c`), by fighter kind.
pub const FIGHTER_SCALES: [f32; 12] = [
    1.25, 1.15, 1.00, 1.03, 1.21, 1.33, 1.05, 1.07, 1.22, 1.20, 1.26, 1.30,
];

/// `mnVSResultsFuncLights`: the fighters' light angles
/// (`scSubsysFighterSetLightParams(10.0F, 10.0F, ...)`).
pub const LIGHT_ANGLE: [f32; 2] = [10.0, 10.0];

/// A perspective camera as a `CObj` holds it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Camera {
    pub eye: Vec3,
    pub at: Vec3,
    pub up: Vec3,
    /// Degrees.
    pub fovy: f32,
    pub aspect: f32,
    pub near: f32,
    pub far: f32,
    /// `syRdpSetViewport`'s `ulx, uly, lrx, lry` on the 320 x 240 screen.
    pub viewport: [f32; 4],
}

/// `mnVSResultsMakeFighterCamera`: eye (0, 0, 1800) on the origin, and
/// `dGCPerspDefault`'s projection (30 degrees, 4:3, 100 to 12800), which
/// `gcAddCameraMatrixSets` gives every camera it makes.
pub const CAMERA: Camera = Camera {
    eye: Vec3 {
        x: 0.0,
        y: 0.0,
        z: 1800.0,
    },
    at: Vec3 {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    },
    up: Vec3 {
        x: 0.0,
        y: 1.0,
        z: 0.0,
    },
    fovy: 30.0,
    aspect: 4.0 / 3.0,
    near: 100.0,
    far: 12800.0,
    viewport: [10.0, 10.0, 310.0, 230.0],
};

/// `nFTDemoStatusWin1` to `nFTDemoStatusLose`, the statuses the results
/// set (`scSubsysFighterSetStatus`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DemoStatus {
    Win1,
    Win2,
    Win3,
    Win4,
    Lose,
}

impl DemoStatus {
    /// The status's row from Win1: its submotion row less one, and its
    /// clip slot less `anim::SLOT_WIN1`.
    pub fn index(self) -> usize {
        self as usize
    }

    /// Whether the fighter's row plays its translations as they are:
    /// `FTANIM_FLAG_TRANSLATE_SCALES`, which clears `is_have_translate_scale`
    /// in `ftMainSetStatus`. Of the fighters with `translate_scales`, only
    /// Luigi's own demo rows set it (`dFTLuigiSubMotionDescs`); his Lose is
    /// Mario's claps, whose row does not, so it plays scaled.
    pub fn skips_translate_scales(self, kind: FighterKind) -> bool {
        kind == FighterKind::Luigi && self != DemoStatus::Lose
    }
}

/// One results fighter: `mnVSResultsMakeFighter`'s kind and costume, and
/// the TopN transform the scene then sets.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Fighter {
    pub player: usize,
    pub kind: FighterKind,
    pub costume: u8,
    /// `translate`, set by `mnVSResultsSetFighterPosition`.
    pub pos: Vec3,
    /// `rotate.y` in radians: 0 faces the camera; `mnVSResultsFaceWinner`
    /// turns the others to the winner. A demo fighter's status does not set
    /// it (`ftMainSetStatus` skips `nFTPlayerKindDemo`).
    pub rotate_y: f32,
    /// `mnVSResultsSetFighterScale`.
    pub scale: f32,
    /// `mnVSResultsSetFighterStatus`; `None` where the two-player switch
    /// has no case for the place.
    pub status: Option<DemoStatus>,
}

/// `pos_x_2p`, `pos_x_3p` and `pos_x_4p` in `mnVSResultsSetFighterPosition`,
/// by distance ID and spot.
const POS_X_2P: [[f32; 4]; 2] = [
    [-150.0, -350.0, -700.0, -1000.0],
    [100.0, 250.0, 600.0, 1000.0],
];
const POS_X_3P: [[f32; 4]; 3] = [
    [-450.0, -900.0, -2000.0, -3000.0],
    [0.0, 0.0, 0.0, 0.0],
    [400.0, 800.0, 1800.0, 2800.0],
];
const POS_X_4P: [[f32; 4]; 4] = [
    [-450.0, -900.0, -2000.0, -3000.0],
    [-150.0, -350.0, -700.0, -1000.0],
    [150.0, 300.0, 700.0, 1000.0],
    [400.0, 800.0, 1800.0, 2800.0],
];
/// `pos_yz`, by spot.
const POS_YZ: [[f32; 2]; 4] = [
    [-350.0, 0.0],
    [-450.0, -2000.0],
    [-700.0, -5000.0],
    [-900.0, -9000.0],
];

/// The results scene's own state.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Scene {
    /// `mnVSResultsFuncStart`'s wipe, picked but not drawn; none with no
    /// contest.
    pub transition: Option<i32>,
    /// `sMNVSResultsFighterGObjs`, by player.
    pub fighters: [Option<Fighter>; 4],
}

impl Scene {
    /// `mnVSResultsFuncStart`'s random pick: the wipe
    /// (`syUtilsRandIntRange(ARRAY_COUNT(dLBTransitionDescs))`), except
    /// with no contest.
    pub fn start(r: &Results) -> Scene {
        let transition = (r.kind != Kind::NoContest).then(|| rng::rand_int_range(TRANSITION_COUNT));
        Scene {
            transition,
            fighters: [None; 4],
        }
    }

    /// `mnVSResultsInitFightersAll`, when [`Results::fighters_due`]:
    /// `entrants` is each player's fighter kind and costume
    /// (`gSCManagerTransferBattleState.players`). Present players are
    /// made in port order, each drawing its Win pick as it is made, then
    /// everyone below first place turns to the winner.
    pub fn init_fighters_all(&mut self, r: &Results, entrants: [Option<(FighterKind, u8)>; 4]) {
        for (player, entrant) in entrants.into_iter().enumerate() {
            let Some((kind, costume)) = entrant.filter(|_| r.present[player]) else {
                continue;
            };
            // `mnVSResultsInitFighter`.
            let spot = spot(r, player);
            self.fighters[player] = Some(Fighter {
                player,
                kind,
                costume,
                pos: position(r, player, spot),
                rotate_y: 0.0,
                scale: FIGHTER_SCALES.get(kind as usize).copied().unwrap_or(1.0),
                status: status(r, player, kind),
            });
        }
        if r.kind == Kind::NoContest {
            return;
        }
        // `mnVSResultsFaceWinner`.
        let Some(winner) = r.winner.and_then(|w| self.fighters[w]) else {
            return;
        };
        for f in self.fighters.iter_mut().flatten() {
            if r.places[f.player] != 0 {
                f.rotate_y =
                    ssb_engine::math::atan2(winner.pos.x - f.pos.x, winner.pos.z - f.pos.z);
            }
        }
    }
}

/// `mnVSResultsGetPresentCount`.
pub(crate) fn present_count(r: &Results) -> usize {
    r.present.iter().filter(|&&p| p).count()
}

/// `mnVSResultsGetPresentLowerCount`: present players on lower ports.
pub(crate) fn present_lower_count(r: &Results, player: usize) -> usize {
    r.present[..player.min(4)].iter().filter(|&&p| p).count()
}

/// `mnVSResultsGetPlacePlayer`: the first present player in `place`.
fn place_player(r: &Results, place: i32) -> Option<usize> {
    (0..4).find(|&i| r.present[i] && r.places[i] == place)
}

/// `mnVSResultsGetPlayerCountPlace`.
pub(crate) fn count_place(r: &Results, place: i32) -> usize {
    (0..4)
        .filter(|&i| r.present[i] && r.places[i] == place)
        .count()
}

/// `mnVSResultsGetPlayerCountAhead`: other present players placed better.
fn count_ahead(r: &Results, player: usize) -> i32 {
    (0..4)
        .filter(|&i| i != player && r.present[i] && r.places[i] < r.places[player])
        .count() as i32
}

/// `mnVSResultsGetPlayerDistanceID`: the column, left to right. It is the
/// player's order among the present players, except that a sole winner
/// takes a middle column and the player there takes its place.
pub(crate) fn distance_id(r: &Results, player: usize) -> usize {
    let mut foes = present_lower_count(r, player);
    if count_place(r, 0) == 1 {
        match present_count(r) {
            2 => {}
            3 => {
                if r.places[player] == 0 {
                    if matches!(present_lower_count(r, player), 0 | 2) {
                        foes = 1;
                    }
                } else if present_lower_count(r, player) == 1 {
                    foes = place_player(r, 0).map_or(foes, |w| present_lower_count(r, w));
                }
            }
            _ => {
                let p = &r.places;
                match player {
                    0 if p[0] == 0 && p[1] != 0 => foes = 1,
                    1 if p[0] == 0 && p[1] != 0 => foes = 0,
                    2 if p[3] == 0 && p[2] != 0 => foes = 3,
                    3 if p[3] == 0 && p[2] != 0 => foes = 2,
                    _ => {}
                }
            }
        }
    }
    foes
}

/// `mnVSResultsGetSpot`: the row back from the camera, the place pushed
/// back one more by a shared place (`places`) and by
/// `aheads[place - ahead]` (`{ 0, 0, 1, 1 }`).
///
/// A tie above the player makes `place - ahead` negative. The source then
/// reads below `aheads` on its stack: in the US build (`addiu t0,sp,44`,
/// `addiu t6,sp,64`) that is the end of `places`, whose last three entries
/// are 1. So a negative index counts as 1 here.
pub(crate) fn spot(r: &Results, player: usize) -> usize {
    let place = r.places[player];
    let ahead = match place - count_ahead(r, player) {
        0 | 1 => 0,
        _ => 1,
    };
    let places = [0, 0, 1, 1, 1];
    let shared = places[count_place(r, place).min(4)];
    (place + ahead + shared).max(0) as usize
}

/// `mnVSResultsSetFighterPosition(fighter, player, spot)`.
fn position(r: &Results, player: usize, spot: usize) -> Vec3 {
    // The tables have four spots; no ranking reaches a fifth.
    let spot = spot.min(3);
    let d = distance_id(r, player);
    let x = match present_count(r) {
        2 => POS_X_2P[d.min(1)][spot],
        3 => POS_X_3P[d.min(2)][spot],
        _ => POS_X_4P[d.min(3)][spot],
    };
    Vec3::new(x, POS_YZ[spot][0], POS_YZ[spot][1])
}

/// `mnVSResultsGetStatusWin`: Win1 to Win3 at random, Win1 or Win2 for
/// Kirby (`syUtilsRandIntRange`).
fn status_win(kind: FighterKind) -> DemoStatus {
    const STATUS_IDS: [DemoStatus; 3] = [DemoStatus::Win1, DemoStatus::Win2, DemoStatus::Win3];
    let range = if kind == FighterKind::Kirby { 2 } else { 3 };
    STATUS_IDS[rng::rand_int_range(range) as usize]
}

/// `mnVSResultsSetFighterStatus`: Lose for everyone with no contest,
/// otherwise a Win pick for first place and Lose for the rest.
fn status(r: &Results, player: usize, kind: FighterKind) -> Option<DemoStatus> {
    if r.kind == Kind::NoContest {
        return Some(DemoStatus::Lose);
    }
    let place = r.places[player];
    match present_count(r) {
        2 => match place {
            0 => Some(status_win(kind)),
            1 => Some(DemoStatus::Lose),
            _ => None,
        },
        _ if place == 0 => Some(status_win(kind)),
        _ => Some(DemoStatus::Lose),
    }
}

#[cfg(test)]
#[path = "results_scene_tests.rs"]
mod tests;
