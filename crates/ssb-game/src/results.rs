//! The VS results' rankings — `mnVSResultsInitVars`,
//! `mnVSResultsInitRankings`, `mnVSResultsGetWinPlayer` and
//! `mnVSResultsCheckExit` in `mn/mnvsmode/mnvsresults.c`: each player's
//! KOs, falls and points, the places they sort into, the winner, when the
//! fighters appear and when START may leave. The fighters themselves are
//! [`crate::results_scene`]; the text and confetti are not ported.

use crate::battle::{Battle, Rule};

/// `nMNVSResultsKind*`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    TimeRoyal,
    StockRoyal,
    TimeTeam,
    StockTeam,
    /// A reset from the pause menu (`is_reset`).
    NoContest,
}

/// The results (`sMNVSResults*`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Results {
    pub kind: Kind,
    pub present: [bool; 4],
    /// `score`, capped at 999.
    pub kos: [i32; 4],
    /// `falls`, capped at 999.
    pub tko: [i32; 4],
    pub points: [i32; 4],
    /// 0 is first; everyone is 0 with no contest.
    pub places: [i32; 4],
    pub winner: Option<usize>,
    pub shared_winner: [bool; 4],
    pub total_tics: u32,
    /// `sMNVSResultsAllowExitWait`.
    pub allow_exit_wait: u32,
    /// `sMNVSResultsDrawWallpaperTic`: 80, or 1 with no contest.
    pub draw_wallpaper_tic: u32,
    /// `sMNVSResultsMakeResultsTic`: 120, or 1 with no contest.
    pub make_results_tic: u32,
    /// `sMNVSResultsInitFightersAllTic`: 120, or 1 with no contest.
    pub init_fighters_all_tic: u32,
    /// `sMNVSResultsCharacterAlpha`: the fighters' fade-in, 0 to 0xFF.
    pub character_alpha: i32,
}

/// `MNVSResultsScore`.
#[derive(Clone, Copy)]
struct Score {
    score: i32,
    place: i32,
    player: usize,
}

/// `mnVSResultsOrderResults`: by score, and in sudden death by the
/// battle's place among equal scores.
fn order(results: &mut [Score], sudden_death: bool) {
    let n = results.len();
    for i in 0..n {
        for j in i + 1..n {
            if results[i].score < results[j].score
                || (sudden_death
                    && results[i].score == results[j].score
                    && results[j].place < results[i].place)
            {
                results.swap(i, j);
            }
        }
    }
}

/// The places after [`order`]: equal scores share a place, and in sudden
/// death so do equal battle places.
fn places_of(results: &[Score], sudden_death: bool) -> impl Iterator<Item = (usize, i32)> + '_ {
    let mut place = 0;
    let mut score = results.first().map_or(0, |r| r.score);
    let mut winner = results.first().map_or(0, |r| r.place);
    results.iter().map(move |r| {
        if score != r.score || (sudden_death && winner != r.place) {
            place += 1;
            score = r.score;
            winner = r.place;
        }
        (r.player, place)
    })
}

impl Results {
    /// `mnVSResultsInitVars` and `mnVSResultsInitRankings` for a finished
    /// battle.
    #[allow(clippy::needless_range_loop)]
    pub fn new(b: &Battle) -> Results {
        let team = b.is_team_battle;
        let mut kind = match (b.rule, team) {
            (Rule::Time, false) => Kind::TimeRoyal,
            (Rule::Time, true) => Kind::TimeTeam,
            (Rule::Stock, false) => Kind::StockRoyal,
            (Rule::Stock, true) => Kind::StockTeam,
        };
        let mut allow_exit_wait = if b.rule == Rule::Time { 410 } else { 370 };
        let mut tics = (80, 120, 120);
        if b.is_reset {
            kind = Kind::NoContest;
            allow_exit_wait = 200;
            tics = (1, 1, 1);
        }
        let present = b.players.map(|p| p.present);
        let kos = b.players.map(|p| i32::from(p.score).min(999));
        let tko = b.players.map(|p| i32::from(p.falls).min(999));
        let points = core::array::from_fn(|i| kos[i] - tko[i]);
        let mut places = [0i32; 4];
        let sudden = b.is_sudden_death;
        if b.rule == Rule::Stock {
            // `mnVSResultsSetPlaceStock`.
            for (i, p) in b.players.iter().enumerate() {
                if p.present {
                    places[i] = i32::from(p.place);
                }
            }
        } else if !team {
            // `mnVSResultsSetRoyalPlace`.
            let mut scores = [Score {
                score: 0,
                place: 0,
                player: 0,
            }; 4];
            let mut n = 0;
            for (i, p) in b.players.iter().enumerate().filter(|(_, p)| p.present) {
                scores[n] = Score {
                    score: kos[i] - tko[i],
                    place: i32::from(p.place),
                    player: i,
                };
                n += 1;
            }
            order(&mut scores[..n], sudden);
            for (player, place) in places_of(&scores[..n], sudden) {
                places[player] = place;
            }
        } else {
            // `mnVSResultsSetTeamPlaceAll`: teams by their total points.
            let mut scores = [Score {
                score: 0,
                place: 0,
                player: 0,
            }; 4];
            let mut n = 0;
            for t in 0..4u8 {
                let Some(first) = b.players.iter().position(|p| p.present && p.team == t) else {
                    continue;
                };
                let total = (0..4)
                    .filter(|&i| present[i] && b.players[i].team == t)
                    .map(|i| kos[i] - tko[i])
                    .sum();
                scores[n] = Score {
                    score: total,
                    place: i32::from(b.players[first].place),
                    player: usize::from(t),
                };
                n += 1;
            }
            order(&mut scores[..n], sudden);
            for (t, place) in places_of(&scores[..n], sudden) {
                for i in 0..4 {
                    if present[i] && usize::from(b.players[i].team) == t {
                        places[i] = place;
                    }
                }
            }
        }
        if kind == Kind::NoContest {
            places = [0; 4];
        }
        let mut r = Results {
            kind,
            present,
            kos,
            tko,
            points,
            places,
            winner: None,
            shared_winner: [false; 4],
            total_tics: 0,
            allow_exit_wait,
            draw_wallpaper_tic: tics.0,
            make_results_tic: tics.1,
            init_fighters_all_tic: tics.2,
            character_alpha: 0,
        };
        r.winner = r.find_winner();
        r
    }

    /// `mnVSResultsGetWinPlayer`: the first player in first place, or among
    /// a team's first-place players the best points, then the most KOs.
    // The port-indexed loops start past the running winner, as the
    // source's do; iterator adaptors would hide that.
    #[allow(clippy::needless_range_loop)]
    fn find_winner(&mut self) -> Option<usize> {
        let first = |i: usize| self.present[i] && self.places[i] == 0;
        match self.kind {
            Kind::TimeRoyal | Kind::StockRoyal => (0..4).find(|&i| first(i)),
            Kind::NoContest => None,
            Kind::TimeTeam | Kind::StockTeam => {
                let possible: [bool; 4] = core::array::from_fn(first);
                let mut win = (0..4).find(|&i| possible[i])?;
                for i in win + 1..4 {
                    if possible[i] && self.points[win] < self.points[i] {
                        win = i;
                    }
                }
                let mut multi = [false; 4];
                let mut is_multi = false;
                for i in win + 1..4 {
                    if possible[i] && self.points[win] == self.points[i] {
                        multi[win] = true;
                        multi[i] = true;
                        is_multi = true;
                    }
                }
                if is_multi {
                    for i in win + 1..4 {
                        if multi[i] && self.kos[win] < self.kos[i] {
                            win = i;
                        }
                    }
                    self.shared_winner = [false; 4];
                    for i in win + 1..4 {
                        if self.kos[win] == self.kos[i] {
                            self.shared_winner[i] = true;
                            self.shared_winner[win] = true;
                        }
                    }
                }
                Some(win)
            }
        }
    }

    /// A frame of `mnVSResultsFuncRun`: the tic count, the fighters'
    /// fade-in once they are made (0x16 a tic), and the exit check, START
    /// once `allow_exit_wait` ticks have passed. Returns whether to leave.
    /// [`Results::fighters_due`] says whether this tic makes the fighters.
    pub fn tick(&mut self, start_tapped: bool) -> bool {
        self.total_tics += 1;
        if self.init_fighters_all_tic < self.total_tics && self.character_alpha < 0xFF {
            self.character_alpha = (self.character_alpha + 0x16).min(0xFF);
        }
        self.total_tics >= self.allow_exit_wait && start_tapped
    }

    /// Whether this tic runs `mnVSResultsInitFightersAll`.
    pub fn fighters_due(&self) -> bool {
        self.total_tics == self.init_fighters_all_tic
    }
}

#[cfg(test)]
#[path = "results_tests.rs"]
mod tests;
