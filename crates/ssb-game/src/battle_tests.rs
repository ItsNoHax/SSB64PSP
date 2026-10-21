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
    // The countdown thread's "Go" lands after that frame's interrupts
    // (RE-556 G2b), and on that frame only.
    assert!(b.go_lands());
    b.begin_frame();
    assert!(!b.go_lands());
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

fn go_battle() -> Battle {
    let mut b = Battle::new(Rule::Time, 3, 2, two_players());
    run_to_go(&mut b);
    b
}

#[test]
fn a_pause_freezes_the_clock_and_eases_back() {
    let mut b = go_battle();
    b.begin_frame();
    b.begin_frame();
    let remain = b.time_remain;
    b.pause();
    assert_eq!(b.status, GameStatus::Pause);
    for _ in 0..50 {
        assert_eq!(b.begin_frame(), Frame::Frozen);
    }
    assert_eq!(b.time_remain, remain);
    b.unpause(true);
    // Twenty frozen frames while the camera eases back, then Go on the
    // 21st, which runs the world but reads no time.
    for _ in 0..20 {
        assert_eq!(b.begin_frame(), Frame::Frozen);
    }
    assert_eq!(b.begin_frame(), Frame::Run);
    assert_eq!(b.status, GameStatus::Go);
    assert_eq!(b.time_remain, remain);
    // The first Go frame after it reads no time either (the timer
    // restarts); the next ticks.
    b.begin_frame();
    b.begin_frame();
    assert_eq!(b.time_remain, remain - 1);
}

#[test]
fn a_reset_from_the_pause_menu_ends_the_battle() {
    let mut b = go_battle();
    b.pause();
    b.reset();
    assert!(b.is_reset);
    for _ in 0..3 {
        assert_eq!(b.begin_frame(), Frame::Frozen);
    }
    assert_eq!(b.begin_frame(), Frame::Done);
}

/// `scVSBattleSetScoreCheckSuddenDeath`'s team case: each team's
/// `score - falls` is summed, and every member of a tied team plays on.
#[test]
fn a_team_tie_sends_both_whole_teams_to_sudden_death() {
    let mut players = [Player::default(); 4];
    for (i, p) in players.iter_mut().enumerate() {
        *p = Player {
            present: true,
            is_human: i == 0,
            team: [0, 0, 1, 1][i],
            ..Player::default()
        };
    }
    let mut b = Battle::new(Rule::Time, 3, 2, players).with_teams(true, false);
    assert!(b.team_rules().spares(0, 0));
    b.on_fall(2, Some(0));
    b.on_fall(1, Some(3));
    // Per player this is 1, -1, -1, 1; per team 0 and 0.
    assert_eq!(b.sudden_death(), Some([true; 4]));
    let sd = b.sudden_death_battle().unwrap();
    assert!(sd.is_team_battle);
    assert!(sd.players.iter().all(|p| p.present));
    b.on_fall(3, Some(0));
    assert_eq!(b.sudden_death(), None);
}

#[test]
fn master_hand_falls_in_slow_motion_until_the_wallpaper_fades() {
    let mut b = Battle::new_1p(5, two_players(), false, 601);
    run_to_go(&mut b);
    b.begin_frame();
    b.boss_defeat();
    assert_eq!(b.status, GameStatus::End);
    assert_eq!(b.end, Some(EndKind::BossDefeat));
    // A second end changes nothing.
    b.announce_end_1p(true);
    assert_eq!(b.end, Some(EndKind::BossDefeat));
    // 90 paused ticks with the camera zooming on him.
    for _ in 0..BOSS_DEFEAT_ZOOM_WAIT {
        assert_eq!(b.begin_frame(), Frame::Frozen);
        assert_eq!(b.boss_defeat, Some(BossDefeat::Zoom));
        assert!(!b.boss_set);
    }
    // The proc-set, then the world runs one tick in three.
    assert_eq!(b.begin_frame(), Frame::Run);
    assert!(b.boss_set);
    assert_eq!(b.boss_defeat, Some(BossDefeat::Slow));
    let frames: Vec<Frame> = (0..6).map(|_| b.begin_frame()).collect();
    assert_eq!(
        frames,
        [
            Frame::Frozen,
            Frame::Frozen,
            Frame::Run,
            Frame::Frozen,
            Frame::Frozen,
            Frame::Run
        ]
    );
    assert!(!b.boss_set);
    assert_eq!(b.status, GameStatus::BossDefeat);
    // The fade's end: the next tick sets the scene, three more end it.
    b.boss_wallpaper_done();
    assert_eq!(b.begin_frame(), Frame::Frozen);
    assert_eq!(b.status, GameStatus::Set);
    for _ in 0..SET_RESTORE_WAIT {
        assert_eq!(b.begin_frame(), Frame::Frozen);
    }
    assert_eq!(b.begin_frame(), Frame::Done);
}

/// Four players, two teams: 0 and 2 on team 0, 1 and 3 on team 1.
fn teams(stocks: [i8; 4]) -> Battle {
    let mut p = [Player::default(); 4];
    for (i, p) in p.iter_mut().enumerate() {
        *p = Player {
            present: true,
            team: (i % 2) as u8,
            ..Player::default()
        };
    }
    with_stocks(
        Battle::new(Rule::Stock, 0, 2, p).with_teams(true, false),
        stocks,
    )
}

fn with_stocks(mut b: Battle, stocks: [i8; 4]) -> Battle {
    for (p, s) in b.players.iter_mut().zip(stocks) {
        p.stock_count = s;
    }
    b
}

#[test]
fn a_steal_takes_a_teammates_stock_and_marks_the_thief() {
    let mut b = teams([-1, 0, 2, 3]);
    assert_eq!(b.steal_stock(0), Some(2));
    assert_eq!(b.players[2].stock_count, 1);
    assert_eq!(b.players[0].stock_count, -2);
    // The opposing team is untouched.
    assert_eq!(b.players[3].stock_count, 3);
    b.land_stolen_stock(0);
    assert_eq!(b.players[0].stock_count, 0);
}

#[test]
fn no_teammate_with_a_stock_to_spare_means_no_steal() {
    let mut b = teams([-1, 2, 0, 2]);
    assert_eq!(b.steal_stock(0), None);
    assert_eq!(b.players[0].stock_count, -1);
}

#[test]
fn the_steal_list_restarts_at_each_larger_count_in_port_order() {
    // Thief 0 on team 0 with 1 (one stock) and 3 (two): the list holds
    // only 3 once its larger count is met. Team membership by `team`.
    let p = [Player {
        present: true,
        team: 0,
        ..Player::default()
    }; 4];
    let battle = |stocks| {
        with_stocks(
            Battle::new(Rule::Stock, 0, 2, p).with_teams(true, false),
            stocks,
        )
    };
    crate::rng::set_seed(12345);
    for _ in 0..32 {
        assert_eq!(battle([-1, 1, 0, 2]).steal_stock(0), Some(3));
    }
    // A smaller count after the largest stays in the list.
    crate::rng::set_seed(12345);
    let picks: std::collections::BTreeSet<u8> = (0..64)
        .map(|_| battle([-1, 2, 0, 1]).steal_stock(0).unwrap())
        .collect();
    assert_eq!(picks, [1, 3].into_iter().collect());
}

#[test]
fn only_vs_battles_outside_sudden_death_show_scores() {
    let b = teams([1; 4]);
    assert!(b.is_show_score());
    let mut sd = teams([1; 4]);
    sd.is_sudden_death = true;
    assert!(!sd.is_show_score());
}

#[test]
fn arrows_and_magnifiers_stay_hidden_until_go() {
    // `ifCommonPlayerMagnifyMakeInterface` clears `is_magnify_display`;
    // `ifCommonAnnounceGoSetStatus` sets it (ifcommon.c:2020).
    let mut b = Battle::new(Rule::Time, 3, 2, two_players());
    let mut frames = 0;
    while b.status == GameStatus::Wait {
        assert!(!b.is_magnify_display(), "shown on wait frame {frames}");
        b.begin_frame();
        frames += 1;
    }
    assert_eq!(frames, Battle::GO_TICK);
    assert!(b.is_magnify_display());
    // The pause clears it; the restore sets it with Go.
    b.pause();
    assert!(!b.is_magnify_display());
    b.unpause(false);
    assert!(!b.is_magnify_display());
    b.begin_frame();
    assert_eq!(b.status, GameStatus::Go);
    assert!(b.is_magnify_display());
    // The demos' battles start at Go.
    assert!(Battle::new_demo(two_players()).is_magnify_display());
    // Sudden death and the 1P Game wait for their Go too.
    let mut sd = Battle::new(Rule::Stock, 3, 0, two_players());
    sd.is_sudden_death = true;
    assert!(!sd.is_magnify_display());
    assert!(!Battle::new_1p(3, two_players(), false, 100).is_magnify_display());
}

#[test]
fn boss_magnifiers_hidden_until_go_and_through_defeat() {
    // `sc1PGameWaitStageBossUpdate` sets `is_magnify_display` after its
    // 600-tick sleep (sc1pgame.c:1593); `sc1PGameBossDefeatInitInterface`
    // clears it (sc1pgame.c:2018).
    use crate::spgame::wait::Wait;
    let go = Wait::with_pattern(crate::spgame::Stage::Boss, 0, 1, 1, 0).go_tick;
    let mut b = Battle::new_1p(5, two_players(), false, go);
    let mut frames = 0;
    while b.status == GameStatus::Wait {
        assert!(!b.is_magnify_display(), "shown on boss wait frame {frames}");
        b.begin_frame();
        frames += 1;
    }
    assert_eq!(frames, 601);
    assert!(b.is_magnify_display());
    b.boss_defeat();
    for i in 0..200 {
        assert!(!b.is_magnify_display(), "shown in {:?}", b.status);
        b.begin_frame();
        // `sc1PGameBossDefeatInterfaceProcUpdate` (the stock snap) runs on
        // the first interface update only (`ifCommonBattleEndUpdateInterface`).
        assert_eq!(b.boss_update, i == 0, "tick {i}");
    }
}

/// The pause menu's A+B+R+Z reset (`ifcommon.c:3021-3044`): only with all
/// four held, and the menu hides at the reset, so nothing of it is left
/// for the next battle (the 1P report: the menu showed again).
#[test]
fn a_pause_quit_needs_the_whole_combo_and_hides_the_menu() {
    use crate::pause::{self, PauseKind, PauseState};
    use ssb_engine::input::N64Buttons;
    let menu_at_pause = PauseState {
        kind: PauseKind::Default,
        retry: false,
        origin: (0.0, 0.0),
        detail: crate::modelpart::Detail::High,
        map_origin: ssb_engine::math::Vec3::ZERO,
        map_target: ssb_engine::math::Vec3::ZERO,
    };
    let mut b = go_battle();
    b.pause();
    let mut menu = Some(menu_at_pause);
    let b_ = |m: u16| N64Buttons(m);
    // One button, or three of the four: still paused, menu up.
    assert!(!pause::quit(
        &mut b,
        &mut menu,
        b_(N64Buttons::A),
        b_(N64Buttons::A)
    ));
    let three = N64Buttons::A | N64Buttons::B | N64Buttons::R;
    assert!(!pause::quit(
        &mut b,
        &mut menu,
        b_(N64Buttons::R),
        b_(three)
    ));
    assert_eq!((b.status, menu.is_some()), (GameStatus::Pause, true));
    // All four: the reset, with the menu gone through the Set frames.
    assert!(pause::quit(
        &mut b,
        &mut menu,
        b_(N64Buttons::Z),
        b_(pause::QUIT_COMBO)
    ));
    assert!(b.is_reset && menu.is_none());
    for _ in 0..3 {
        assert_eq!(b.begin_frame(), Frame::Frozen);
        assert!(!pause::quit(
            &mut b,
            &mut menu,
            b_(N64Buttons::Z),
            b_(pause::QUIT_COMBO)
        ));
    }
    assert_eq!(b.begin_frame(), Frame::Done);
    assert!(menu.is_none());
    // The next battle starts unpaused, and the combo does nothing outside
    // the pause.
    let mut next = Battle::new(Rule::Time, 3, 2, two_players());
    assert_ne!(next.status, GameStatus::Pause);
    assert!(!pause::quit(
        &mut next,
        &mut menu,
        b_(N64Buttons::Z),
        b_(pause::QUIT_COMBO)
    ));
    run_to_go(&mut next);
    assert_eq!((next.status, next.is_reset), (GameStatus::Go, false));
}
