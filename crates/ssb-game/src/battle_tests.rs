use super::*;

fn two_players() -> [Player; 4] {
    let mut p = [Player::default(); 4];
    p[0] = Player {
        present: true,
        is_human: true,
        team: 0,
        ..Player::default()
    };
    p[1] = Player {
        present: true,
        team: 1,
        ..Player::default()
    };
    p
}

fn run_to_go(b: &mut Battle) -> u32 {
    let mut frames = 0;
    while b.status == GameStatus::Wait {
        assert_eq!(b.begin_frame(), Frame::Run);
        frames += 1;
        assert!(frames < 1000);
    }
    frames
}

#[test]
fn the_countdown_says_go_on_frame_391() {
    let mut b = Battle::new(Rule::Time, 3, 2, two_players());
    assert_eq!(b.status, GameStatus::Wait);
    assert_eq!(run_to_go(&mut b), Battle::GO_TICK);
    assert_eq!(Battle::GO_TICK, 391);
    assert_eq!(b.time_remain, 3 * 3600);
}

#[test]
fn time_runs_from_the_second_go_frame_and_time_up_ends_the_battle() {
    let mut b = Battle::new(Rule::Time, 1, 2, two_players());
    run_to_go(&mut b);
    // The first Go frame starts the scheduler count and reads no time.
    b.begin_frame();
    assert_eq!(b.time_remain, 3600);
    for _ in 0..3599 {
        assert_eq!(b.begin_frame(), Frame::Run);
    }
    assert_eq!(b.status, GameStatus::Go);
    b.begin_frame();
    assert_eq!(b.time_remain, 0);
    assert_eq!(b.status, GameStatus::End);
    assert_eq!(b.end, Some(EndKind::TimeUp));
    // 90 frozen frames of "Time!", then `Set`, then three more.
    let mut frozen = 0;
    loop {
        match b.begin_frame() {
            Frame::Frozen => frozen += 1,
            Frame::Done => break,
            Frame::Run => panic!("the world runs after the end"),
        }
    }
    assert_eq!(
        frozen,
        u32::from(END_RESTORE_WAIT) + 1 + u32::from(SET_RESTORE_WAIT)
    );
}

#[test]
fn an_infinite_time_battle_never_times_out() {
    let mut b = Battle::new(Rule::Time, TIMELIMIT_INFINITE, 2, two_players());
    run_to_go(&mut b);
    for _ in 0..20000 {
        b.begin_frame();
    }
    assert_eq!(b.status, GameStatus::Go);
    assert!(b.time_passed > 19000);
}

#[test]
fn kos_credit_the_last_attacker_and_self_destructs_count_apart() {
    let mut b = Battle::new(Rule::Time, 3, 2, two_players());
    b.on_fall(1, Some(0));
    b.on_fall(1, None);
    assert_eq!(b.players[0].score, 1);
    assert_eq!(b.players[0].kos[1], 1);
    assert_eq!(b.players[1].falls, 2);
    assert_eq!(b.players[1].self_destructs, 1);
    // Time rules take no stocks.
    assert_eq!(b.players[1].stock_count, 2);
    assert_eq!(b.winner(), Some(0));
}

#[test]
fn a_stock_battle_ends_when_one_player_is_left() {
    let mut b = Battle::new(Rule::Stock, 3, 1, two_players());
    b.on_fall(1, Some(0));
    assert_eq!(b.players[1].stock_count, 0);
    assert_eq!(b.status, GameStatus::Wait);
    b.on_fall(1, Some(0));
    assert_eq!(b.players[1].stock_count, -1);
    assert_eq!(b.players[1].place, 1);
    assert_eq!(b.end, Some(EndKind::GameSet));
    assert_eq!(b.status, GameStatus::End);
    assert_eq!(b.winner(), Some(0));
}

#[test]
fn a_tie_on_score_minus_falls_goes_to_sudden_death() {
    let mut b = Battle::new(Rule::Time, 3, 2, two_players());
    b.on_fall(1, Some(0));
    b.on_fall(0, Some(1));
    assert_eq!(b.sudden_death(), Some([true, true, false, false]));
    assert_eq!(b.winner(), None);
    b.on_fall(1, None);
    assert_eq!(b.sudden_death(), None);
    assert_eq!(b.winner(), Some(0));
}

#[test]
fn a_vs_fighter_faces_the_nearest_other_spawn() {
    assert_eq!(
        start_facing(0.0, [-1500.0, 3000.0].into_iter()),
        Facing::Left
    );
    assert_eq!(
        start_facing(0.0, [1500.0, -3000.0].into_iter()),
        Facing::Right
    );
    assert_eq!(start_facing(100.0, core::iter::empty()), Facing::Right);
}

#[test]
fn sudden_death_is_a_no_stock_battle_that_says_go_after_90_ticks() {
    let mut b = Battle::new(Rule::Time, 3, 2, two_players());
    b.on_fall(1, Some(0));
    b.on_fall(0, Some(1));
    let mut sd = b.sudden_death_battle().unwrap();
    assert!(sd.is_sudden_death);
    assert_eq!(sd.rule, Rule::Stock);
    assert_eq!(sd.players[0].stock_count, 0);
    assert_eq!(sd.players[1].stock_count, 0);
    assert_eq!(run_to_go(&mut sd), 91);
    sd.on_fall(0, Some(1));
    assert_eq!(sd.end, Some(EndKind::GameSet));
    assert_eq!(sd.winner(), Some(1));
    // A decided battle has no sudden death.
    let mut b = Battle::new(Rule::Time, 3, 2, two_players());
    b.on_fall(1, Some(0));
    assert!(b.sudden_death_battle().is_none());
}
