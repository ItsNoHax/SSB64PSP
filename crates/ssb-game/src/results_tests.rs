use super::*;
use crate::battle::Player;

fn battle(rule: Rule, scores: [(u16, u16, u8); 2]) -> Battle {
    let mut players = [Player::default(); 4];
    for (i, (score, falls, place)) in scores.into_iter().enumerate() {
        players[i] = Player {
            present: true,
            is_human: i == 0,
            team: i as u8,
            score,
            falls,
            place,
            ..Player::default()
        };
    }
    let mut b = Battle::new(rule, 3, 2, players);
    // `Battle::new` resets the scores; put the finished battle's back.
    for (i, (score, falls, place)) in scores.into_iter().enumerate() {
        b.players[i].score = score;
        b.players[i].falls = falls;
        b.players[i].place = place;
    }
    b
}

#[test]
fn a_time_battle_ranks_by_kos_less_falls() {
    let r = Results::new(&battle(Rule::Time, [(1, 3, 0), (3, 1, 0)]));
    assert_eq!(r.kind, Kind::TimeRoyal);
    assert_eq!(r.points[..2], [-2, 2]);
    assert_eq!(r.places[..2], [1, 0]);
    assert_eq!(r.winner, Some(1));
    assert_eq!(r.allow_exit_wait, 410);
}

#[test]
fn equal_points_share_first_place_outside_sudden_death() {
    let r = Results::new(&battle(Rule::Time, [(2, 1, 1), (2, 1, 0)]));
    assert_eq!(r.places[..2], [0, 0]);
    // The lower port wins a shared first.
    assert_eq!(r.winner, Some(0));
    // Sudden death breaks the tie by the battle's place.
    let mut b = battle(Rule::Time, [(2, 1, 1), (2, 1, 0)]);
    b.is_sudden_death = true;
    let r = Results::new(&b);
    assert_eq!(r.places[..2], [1, 0]);
    assert_eq!(r.winner, Some(1));
}

#[test]
fn a_stock_battle_takes_the_battle_places() {
    let r = Results::new(&battle(Rule::Stock, [(0, 3, 1), (2, 1, 0)]));
    assert_eq!(r.kind, Kind::StockRoyal);
    assert_eq!(r.places[..2], [1, 0]);
    assert_eq!(r.winner, Some(1));
    assert_eq!(r.allow_exit_wait, 370);
}

#[test]
fn a_reset_is_no_contest() {
    let mut b = battle(Rule::Time, [(1, 0, 0), (0, 1, 0)]);
    b.is_reset = true;
    let mut r = Results::new(&b);
    assert_eq!(r.kind, Kind::NoContest);
    assert_eq!(r.places, [0; 4]);
    assert_eq!(r.winner, None);
    // START leaves only from tick 200.
    for _ in 0..199 {
        assert!(!r.tick(true));
    }
    assert!(r.tick(true));
}

#[test]
fn the_winner_is_announced_then_the_fanfare_then_the_results_bgm() {
    use crate::sound::{id, testing::*};
    let rec = Recorder::install();
    let kinds = [Some(FighterKind::Fox), Some(FighterKind::Kirby), None, None];
    let mut r = Results::start(&battle(Rule::Time, [(1, 3, 0), (3, 1, 0)]), kinds);
    assert_eq!(rec.take(), [Call::PlayFgm(id::nSYAudioVoicePublicWin)]);
    let mut at = alloc::vec::Vec::new();
    for _ in 0..300 {
        r.tick(false);
        for c in rec.take() {
            at.push((r.total_tics, c));
        }
    }
    // Kirby, on port 2, won: "This game's winner is... Kirby!", the crowd,
    // and Kirby's fanfare at 120. The fanfare still plays.
    assert_eq!(
        at,
        [
            (81, Call::PlayFgm(id::nSYAudioVoiceAnnounceWinnerIs)),
            (120, Call::PlayBgm(0, id::nSYAudioBGMWinKirby)),
            (210, Call::PlayFgm(id::nSYAudioVoiceAnnounceKirby)),
            (270, Call::PlayFgm(id::nSYAudioVoicePublicExcited)),
        ]
    );
    // The fanfare ends: the thread starts the results BGM, once.
    crate::sound::stop_bgm(0);
    rec.take();
    r.tick(false);
    assert_eq!(rec.take(), [Call::PlayBgm(0, id::nSYAudioBGMResults)]);
    r.tick(false);
    assert_eq!(rec.take(), []);
    // START leaves with the sounds and the music stopped.
    while !r.tick(true) {}
    assert_eq!(rec.take(), [Call::StopAllFgm, Call::StopBgmAll]);
    crate::sound::uninstall();
}

#[test]
fn no_contest_is_announced_without_any_music() {
    use crate::sound::{id, testing::*};
    let rec = Recorder::install();
    let mut b = battle(Rule::Time, [(1, 0, 0), (0, 1, 0)]);
    b.is_reset = true;
    let mut r = Results::start(&b, [Some(FighterKind::Mario); 4]);
    for _ in 0..300 {
        r.tick(false);
    }
    assert_eq!(
        rec.take(),
        [
            Call::PlayFgm(id::nSYAudioVoiceAnnounceNoContest),
            Call::PlayFgm(id::nSYAudioVoicePublicNoContest),
        ]
    );
    crate::sound::uninstall();
}
