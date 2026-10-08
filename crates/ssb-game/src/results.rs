//! The VS results' rankings — `mnVSResultsInitVars`,
//! `mnVSResultsInitRankings`, `mnVSResultsGetWinPlayer` and
//! `mnVSResultsCheckExit` in `mn/mnvsmode/mnvsresults.c`: each player's
//! KOs, falls and points, the places they sort into, the winner, when the
//! fighters appear and when START may leave. The fighters themselves are
//! [`crate::results_scene`]; the text, table and wallpaper are
//! [`crate::results_layer`]; the confetti is not ported. The announcer,
//! the winner's fanfare and the results BGM after it
//! (`mnVSResultsAnnounceWinner`, `mnVSResultsPlayWinBGM`,
//! `mnVSResultsAudioThreadUpdate`) run in [`Results::tick`].

use crate::battle::{Battle, Rule};
use crate::fighter::FighterKind;
use crate::fighter_select::ANNOUNCE_NAMES;
use crate::sound::{self, id};

/// `mnVSResultsAudioThreadUpdate`, the GObj thread `mnVSResultsMakeAudioThread`
/// starts with the win BGM: it waits for player 0 to leave `AL_STOPPED`,
/// then for it to stop, then plays `nSYAudioBGMResults` and ends.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioThread {
    /// Not made (before tic 120, or no contest), or ended.
    None,
    /// The first loop: the win BGM has not started yet.
    WaitStart,
    /// The second loop: the win BGM plays.
    WaitEnd,
}

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
    /// `sMNVSResultsIsTeamBattle`, kept with no contest.
    pub is_team_battle: bool,
    pub present: [bool; 4],
    /// `score`, capped at 999.
    pub kos: [i32; 4],
    /// `falls`, capped at 999.
    pub tko: [i32; 4],
    pub points: [i32; 4],
    /// 0 is first; everyone is 0 with no contest.
    pub places: [i32; 4],
    /// `gSCManagerTransferBattleState.players[].team`: the team in a team
    /// battle.
    pub team: [u8; 4],
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
    /// `sMNVSResultsFighterKinds`: the battle's fighters, set by
    /// [`Results::start`].
    pub fighter_kinds: [Option<FighterKind>; 4],
    /// The results BGM's thread.
    pub audio_thread: AudioThread,
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
            is_team_battle: team,
            present,
            kos,
            tko,
            points,
            places,
            team: b.players.map(|p| p.team),
            winner: None,
            shared_winner: [false; 4],
            total_tics: 0,
            allow_exit_wait,
            draw_wallpaper_tic: tics.0,
            make_results_tic: tics.1,
            init_fighters_all_tic: tics.2,
            character_alpha: 0,
            fighter_kinds: [None; 4],
            audio_thread: AudioThread::None,
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

    /// `mnVSResultsFuncStart`: [`Results::new`] with the battle's
    /// fighters, and the crowd's cheer unless there was no contest.
    pub fn start(b: &Battle, fighter_kinds: [Option<FighterKind>; 4]) -> Results {
        let r = Results {
            fighter_kinds,
            ..Results::new(b)
        };
        if r.kind != Kind::NoContest {
            sound::play_fgm(id::nSYAudioVoicePublicWin);
        }
        r
    }

    /// A frame of `mnVSResultsFuncRun`: the tic count, the fighters'
    /// fade-in once they are made (0x16 a tic), the announcer, the win BGM
    /// at tic 120, and the exit check, START once `allow_exit_wait` ticks
    /// have passed, which stops the sounds and the music. Then the results
    /// BGM's thread. Returns whether to leave. [`Results::fighters_due`]
    /// says whether this tic makes the fighters.
    pub fn tick(&mut self, start_tapped: bool) -> bool {
        self.total_tics += 1;
        if self.init_fighters_all_tic < self.total_tics && self.character_alpha < 0xFF {
            self.character_alpha = (self.character_alpha + 0x16).min(0xFF);
        }
        self.announce_winner();
        let made_thread = self.kind != Kind::NoContest && self.total_tics == 120;
        if made_thread {
            self.play_win_bgm();
        }
        let leave = self.total_tics >= self.allow_exit_wait && start_tapped;
        if leave {
            sound::stop_all_fgm();
            sound::stop_bgm_all();
        }
        // The thread's GObj (link 17) runs after the scene's; it starts on
        // the next frame.
        self.audio_thread_update();
        if made_thread {
            self.audio_thread = AudioThread::WaitStart;
        }
        leave
    }

    /// `mnVSResultsGetFighterKind(mnVSResultsGetWinPlayer())`.
    fn winner_kind(&self) -> Option<FighterKind> {
        self.winner.and_then(|w| self.fighter_kinds[w])
    }

    /// `mnVSResultsAnnounceWinner`, keyed on the tic count.
    fn announce_winner(&self) {
        let t = self.total_tics;
        let voice = if self.kind == Kind::NoContest {
            match t {
                2 => Some(id::nSYAudioVoiceAnnounceNoContest),
                71 => Some(id::nSYAudioVoicePublicNoContest),
                _ => None,
            }
        } else if !self.is_team_battle {
            match t {
                81 => Some(id::nSYAudioVoiceAnnounceWinnerIs),
                210 => self
                    .winner_kind()
                    .and_then(|k| ANNOUNCE_NAMES.get(k as usize).copied()),
                270 => Some(id::nSYAudioVoicePublicExcited),
                _ => None,
            }
        } else {
            match t {
                // `announcer_teams[mnVSResultsGetWinTeam()]`.
                81 => self.winner.and_then(|w| {
                    [
                        id::nSYAudioVoiceAnnounceRedTeam,
                        id::nSYAudioVoiceAnnounceBlueTeam,
                        id::nSYAudioVoiceAnnounceGreenTeam,
                    ]
                    .get(usize::from(self.team[w]))
                    .copied()
                }),
                130 => Some(id::nSYAudioVoiceAnnounceWins),
                150 => Some(id::nSYAudioVoicePublicExcited),
                _ => None,
            }
        };
        if let Some(v) = voice {
            sound::play_fgm(v);
        }
    }

    /// `mnVSResultsPlayWinBGM`: the winner's series fanfare.
    fn play_win_bgm(&self) {
        use FighterKind::*;
        let bgm = match self.winner_kind() {
            Some(Mario | Luigi) => id::nSYAudioBGMWinMario,
            Some(Fox) => id::nSYAudioBGMWinFox,
            Some(Donkey) => id::nSYAudioBGMWinDonkey,
            Some(Samus) => id::nSYAudioBGMWinMetroid,
            Some(Link) => id::nSYAudioBGMWinZelda,
            Some(Yoshi) => id::nSYAudioBGMWinYoshi,
            Some(Captain) => id::nSYAudioBGMWinFZero,
            Some(Pikachu | Purin) => id::nSYAudioBGMWinPMonsters,
            Some(Kirby) => id::nSYAudioBGMWinKirby,
            Some(Ness) => id::nSYAudioBGMWinMother,
            _ => id::nSYAudioBGMWinDefault,
        };
        sound::play_bgm(0, bgm);
    }

    /// One wake of `mnVSResultsAudioThreadUpdate`: `syAudioCheckBGMPlaying`
    /// stands for `gSYAudioCSPlayers[0]->state != AL_STOPPED`. The poll only
    /// gates the results BGM.
    fn audio_thread_update(&mut self) {
        match self.audio_thread {
            AudioThread::None => {}
            AudioThread::WaitStart => {
                if sound::bgm_playing(0) {
                    self.audio_thread = AudioThread::WaitEnd;
                }
            }
            AudioThread::WaitEnd => {
                if !sound::bgm_playing(0) {
                    sound::play_bgm(0, id::nSYAudioBGMResults);
                    self.audio_thread = AudioThread::None;
                }
            }
        }
    }

    /// Whether this tic runs `mnVSResultsInitFightersAll`.
    pub fn fighters_due(&self) -> bool {
        self.total_tics == self.init_fighters_all_tic
    }
}

/// `mnVSResultsSaveBackup`, at the results' start: the VS counters and the
/// stage played, then each present fighter's record against the others in
/// `battle` (`gSCManagerTransferBattleState`, the battle before any sudden
/// death). `kinds` are the ports' fighters.
pub fn save_backup(
    backup: &mut crate::backup::Backup,
    battle: &Battle,
    kinds: [Option<crate::fighter::FighterKind>; 4],
    gkind: u8,
) {
    backup.vs_total_battles = backup.vs_total_battles.wrapping_add(1);
    backup.ground_mask |= 1u16.wrapping_shl(u32::from(gkind));
    // Stops at `U8_MAX`.
    backup.vs_itemswitch_battles = backup.vs_itemswitch_battles.saturating_add(1);
    // `mnVSResultsGetPlayerCount`: `pl_count + cp_count`.
    let count = battle.players.iter().filter(|p| p.present).count() as u16;
    for (i, p) in battle.players.iter().enumerate() {
        let Some(this) = kinds[i].filter(|_| p.present) else {
            continue;
        };
        let r = &mut backup.vs_records[this as usize % 12];
        r.time_used = r.time_used.wrapping_add(battle.time_passed / 60);
        r.time_used = r.time_used.min(60 * 60 * 1000 - 1);
        r.damage_given = r
            .damage_given
            .wrapping_add(p.total_damage_given)
            .min(999_999);
        r.damage_taken = r.damage_taken.wrapping_add(p.total_damage_all).min(999_999);
        r.selfdestructs = r.selfdestructs.wrapping_add(p.self_destructs).min(9999);
        r.games_played = r.games_played.wrapping_add(1);
        r.player_count_tally = r.player_count_tally.wrapping_add(count);
        for (j, q) in battle.players.iter().enumerate() {
            let Some(vs) = kinds[j].filter(|_| i != j && q.present) else {
                continue;
            };
            let vs = vs as usize % 12;
            r.ko_count[vs] = r.ko_count[vs].wrapping_add(p.kos[j]).min(9999);
            r.player_count_tallies[vs] = r.player_count_tallies[vs].wrapping_add(count);
            r.played_against[vs] = r.played_against[vs].wrapping_add(1);
        }
    }
    backup.write();
}

/// `mnVSResultsFuncRun`'s exit: the Item Switch after 100 VS battles, then
/// Mushroom Kingdom once every starter stage has been played and every
/// starter has cleared the 1P Game. Any queued unlock goes to the message
/// scene, which returns to the VS character select.
pub fn unlocks(backup: &crate::backup::Backup) -> [Option<crate::spgame::Unlock>; 2] {
    use crate::backup::{CHARACTER_MASK_STARTER, GROUND_MASK_ALL};
    use crate::spgame::Unlock;
    let mut out = [None; 2];
    let mut n = 0;
    if backup.unlock_mask & Unlock::ItemSwitch.mask() == 0 && backup.vs_itemswitch_battles >= 100 {
        out[n] = Some(Unlock::ItemSwitch);
        n += 1;
    }
    if backup.unlock_mask & Unlock::Inishie.mask() == 0
        && backup.ground_mask & GROUND_MASK_ALL == GROUND_MASK_ALL
    {
        let complete = backup
            .spgame_records
            .iter()
            .enumerate()
            .filter(|(_, r)| r.is_spgame_complete)
            .fold(0u16, |m, (i, _)| m | 1 << i);
        if complete & CHARACTER_MASK_STARTER == CHARACTER_MASK_STARTER {
            out[n] = Some(Unlock::Inishie);
        }
    }
    out
}

#[cfg(test)]
#[path = "results_tests.rs"]
mod tests;
