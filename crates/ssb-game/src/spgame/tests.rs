use super::*;
use manager::{Manager, Scene};

#[test]
fn stage_ordinals_and_ports_match_source_tables() {
    for (i, stage) in Stage::ALL.into_iter().enumerate() {
        assert_eq!(Stage::from_index(i as u8), Some(stage));
    }
    assert_eq!(Stage::from_index(18), None);
    assert_eq!([0, 1, 2, 3].map(next_port), [1, 2, 3, 0]);
    for i in 0..27 {
        assert_eq!(FighterKind::from_ordinal(i).unwrap() as u8, i);
    }
    assert_eq!(FighterKind::from_ordinal(27), None);
}

#[test]
fn manager_carries_stocks_and_runs_all_fourteen_common_stages() {
    let mut data = SceneData {
        player: 3,
        ..Default::default()
    };
    let mut state = BattleState::default();
    let mut backup = Backup {
        unlock_mask: !0,
        ..Default::default()
    };
    let mut m = Manager::new(&mut data, &mut state, &backup);
    assert_eq!(data.ally_players, [0, 1]);
    state.players[3].stock_count = 1;
    for stage in Stage::ALL.into_iter().take(14) {
        assert_eq!(data.stage(), Some(stage));
        assert_eq!(m.scene, Scene::Intro);
        let battle_scene = m.advance(&mut data, &mut state, &mut backup);
        assert_eq!(
            battle_scene,
            if matches!(stage, Stage::Bonus1 | Stage::Bonus2) {
                Scene::BonusStage
            } else {
                Scene::Battle
            }
        );
        state.time_remain = 123;
        data.bonus_get_mask = [3, 1, 0];
        assert_eq!(
            m.advance(&mut data, &mut state, &mut backup),
            Scene::StageClear
        );
        assert_eq!(state.players[3].stock_count, 1);
        m.advance(&mut data, &mut state, &mut backup);
    }
    assert_eq!(data.bonus_count, 42);
    assert_eq!(m.scene, Scene::Ending);
    assert!(backup.spgame_records[0].is_spgame_complete);
    assert_eq!(
        m.advance(&mut data, &mut state, &mut backup),
        Scene::Staffroll
    );
    assert_eq!(
        m.advance(&mut data, &mut state, &mut backup),
        Scene::Congratulations
    );
    assert_eq!(
        m.advance(&mut data, &mut state, &mut backup),
        Scene::Startup
    );
}

#[test]
fn continues_retry_same_stage_lower_levels_every_two_losses_and_reset_on_clear() {
    let mut data = SceneData::default();
    let mut state = BattleState::default();
    let mut backup = Backup::default();
    let mut m = Manager::new(&mut data, &mut state, &backup);
    for loss in 1..=22 {
        assert_eq!(m.advance(&mut data, &mut state, &mut backup), Scene::Battle);
        state.players[0].stock_count = -1;
        assert_eq!(
            m.advance(&mut data, &mut state, &mut backup),
            Scene::Continue
        );
        data.is_continue = true;
        assert_eq!(m.advance(&mut data, &mut state, &mut backup), Scene::Intro);
        assert_eq!(m.level_drop, (loss / 2).min(9));
        assert_eq!(data.stage, 0);
        assert_eq!(state.players[0].stock_count, 2);
    }
    m.advance(&mut data, &mut state, &mut backup);
    state.time_remain = 1;
    m.advance(&mut data, &mut state, &mut backup);
    assert_eq!(
        (m.level_drop, m.level_guard, data.continues_used),
        (0, 2, 22)
    );
}

#[test]
fn bonus_failures_advance_and_reset_leaves_before_continue_or_clear() {
    for stage in [Stage::Bonus1, Stage::Bonus2, Stage::Bonus3] {
        let mut data = SceneData {
            stage: stage as u8,
            ..Default::default()
        };
        let mut state = BattleState::default();
        let mut backup = Backup::default();
        let mut m = Manager::new(&mut data, &mut state, &backup);
        m.advance(&mut data, &mut state, &mut backup);
        state.time_remain = 0;
        state.players[0].stock_count = -1;
        assert_eq!(
            m.advance(&mut data, &mut state, &mut backup),
            Scene::StageClear
        );
    }
    let mut data = SceneData::default();
    let mut state = BattleState::default();
    let mut backup = Backup::default();
    let mut m = Manager::new(&mut data, &mut state, &backup);
    m.advance(&mut data, &mut state, &mut backup);
    data.is_reset = true;
    assert_eq!(
        m.advance(&mut data, &mut state, &mut backup),
        Scene::ModeMenu
    );
    assert_eq!(backup.writes, 0);
}

#[test]
fn challenger_priority_us_twelve_minute_cutoff_and_loss_drop_can_reach_ten() {
    for (difficulty, continues, ticks, unlocked, expected) in [
        (Difficulty::Normal, 0, 0, 0, Stage::Ness),
        (Difficulty::Easy, 0, 43_199, 0, Stage::Captain),
        (Difficulty::Normal, 1, 43_200, 0, Stage::Purin),
        (
            Difficulty::Normal,
            0,
            0,
            Unlock::Ness.mask(),
            Stage::Captain,
        ),
    ] {
        let mut data = SceneData {
            stage: 14,
            ..Default::default()
        };
        let mut state = BattleState::default();
        let mut backup = Backup {
            spgame_difficulty: difficulty,
            unlock_mask: unlocked,
            ..Default::default()
        };
        let mut m = Manager::new(&mut data, &mut state, &backup);
        m.scene = Scene::Congratulations;
        m.total_time_tics = ticks;
        data.continues_used = continues;
        assert_eq!(
            m.advance(&mut data, &mut state, &mut backup),
            Scene::Challenger
        );
        assert_eq!(data.stage(), Some(expected));
        m.advance(&mut data, &mut state, &mut backup);
        data.challenger_level_drop = 8;
        state.players[0].stock_count = -1;
        m.advance(&mut data, &mut state, &mut backup);
        assert_eq!(data.challenger_level_drop, 10);
    }
}

#[test]
fn high_score_save_preserves_completion_and_uses_score_attempt_difficulty() {
    let mut backup = Backup::default();
    let mut data = SceneData {
        score: 100,
        ..Default::default()
    };
    manager::try_save_backup(&mut backup, &data, true);
    assert_eq!(backup.spgame_records[0].spgame_best_difficulty, 3);
    data.score = 200;
    manager::try_save_backup(&mut backup, &data, false);
    assert_eq!(backup.spgame_records[0].spgame_best_difficulty, 0);
    assert!(backup.spgame_records[0].is_spgame_complete);
    manager::try_save_backup(&mut backup, &data, false);
    assert_eq!(backup.writes, 2);
}

#[test]
fn sound_test_needs_all_twelve_records_for_both_tasks() {
    let mut backup = Backup::default();
    for r in &mut backup.spgame_records {
        r.bonus1_task_count = 10;
        r.bonus2_task_count = 10;
    }
    assert!(manager::check_unlock_sound_test(&backup));
    backup.spgame_records[11].bonus2_task_count = 9;
    assert!(!manager::check_unlock_sound_test(&backup));
}

#[test]
fn waits_preserve_scene_clock_entry_order_and_hud_wrap() {
    use wait::Action::*;
    let w = wait::Wait::with_pattern(Stage::Link, 3, 2, 0, 0);
    assert_eq!(w.at(91).collect::<alloc::vec::Vec<_>>(), [Countdown]);
    assert_eq!(w.at(113).collect::<alloc::vec::Vec<_>>(), [Appear(3)]);
    assert_eq!(w.at(135).collect::<alloc::vec::Vec<_>>(), [Appear(0)]);
    let w = wait::Wait::with_pattern(Stage::Donkey, 2, 4, 0, 2);
    assert_eq!(
        w.at(91).collect::<alloc::vec::Vec<_>>(),
        [Countdown, Appear(2)]
    );
    assert_eq!(w.at(121).collect::<alloc::vec::Vec<_>>(), [Zoom(2)]);
    assert_eq!(w.at(361).collect::<alloc::vec::Vec<_>>(), [CameraDefault]);
    assert_eq!(wait::interface_positions(3, 3), [160, 230, 0, 90]);
    assert_eq!(wait::Wait::new(Stage::Bonus3, 0, 4, 1).go_tick, 61);
    assert_eq!(wait::Wait::new(Stage::Boss, 0, 2, 1).go_tick, 601);
}

#[test]
fn session_loss_and_race_completion_use_real_battle_end_and_accounting() {
    use crate::battle::{EndKind, Frame, GameStatus};
    let mut backup = Backup::default();
    let mut s = session::Session::new(SceneData::default(), &backup);
    s.manager.advance(&mut s.data, &mut s.state, &mut backup);
    s.start_battle(&backup);
    s.battle.as_mut().unwrap().players[0].stock_count = 0;
    s.fall(0, bonus::DefeatRecord::default(), FighterKind::Mario);
    assert_eq!(s.battle.as_ref().unwrap().players[0].stock_count, -1);
    assert_eq!(s.battle.as_ref().unwrap().status, GameStatus::End);
    while s.battle.as_mut().unwrap().begin_frame() != Frame::Done {}
    assert_eq!(s.finish_battle(&mut backup), Scene::Continue);
    assert_eq!(s.manager.total_falls, 1);
    let mut s = session::Session::new(
        SceneData {
            stage: Stage::Bonus3 as u8,
            ..Default::default()
        },
        &backup,
    );
    s.manager.advance(&mut s.data, &mut s.state, &mut backup);
    s.start_battle(&backup);
    s.complete_race();
    assert_eq!(s.battle.as_ref().unwrap().end, Some(EndKind::Complete));
    assert_eq!(s.finish_battle(&mut backup), Scene::StageClear);
}

#[test]
fn race_timer_failure_keeps_campaign_stocks_and_wins_a_finish_gate_tie() {
    use crate::battle::{EndKind, Frame, GameStatus};
    let mut backup = Backup::default();
    let mut s = session::Session::new(
        SceneData {
            stage: Stage::Bonus3 as u8,
            time_limit: crate::battle::TIMELIMIT_INFINITE,
            ..Default::default()
        },
        &backup,
    );
    s.manager.advance(&mut s.data, &mut s.state, &mut backup);
    s.start_battle(&backup);
    let stocks = s.state.players[0].stock_count;
    let b = s.battle.as_mut().unwrap();
    assert!(b.is_1p_game && !b.is_bonus && b.time_up_is_failure);
    assert_eq!(b.time_limit, 1);
    while b.status != GameStatus::Go {
        b.begin_frame();
    }
    while b.end.is_none() {
        b.begin_frame();
    }
    assert_eq!(b.end, Some(EndKind::Failure));
    assert_eq!(b.players[0].stock_count, stocks);
    s.complete_race();
    assert_eq!(s.battle.as_ref().unwrap().end, Some(EndKind::Failure));
    while s.battle.as_mut().unwrap().begin_frame() != Frame::Done {}
    assert_eq!(s.finish_battle(&mut backup), Scene::StageClear);
    assert_eq!(s.data.time_remain, 0);
    assert_eq!(s.manager.total_falls, 0);
    assert!(!bonus::Bonus::NoDamage.contains(&s.data.bonus_get_mask));
}

#[test]
fn race_fall_consumes_a_campaign_stock_and_returns_to_results() {
    use crate::battle::{Frame, GameStatus};
    let mut backup = Backup::default();
    let mut s = session::Session::new(
        SceneData {
            stage: Stage::Bonus3 as u8,
            ..Default::default()
        },
        &backup,
    );
    s.manager.advance(&mut s.data, &mut s.state, &mut backup);
    s.start_battle(&backup);
    let stocks = s.state.players[0].stock_count;
    s.fall(0, bonus::DefeatRecord::default(), FighterKind::Mario);
    assert_eq!(s.battle.as_ref().unwrap().status, GameStatus::Wait);
    assert_eq!(
        s.battle.as_ref().unwrap().players[0].stock_count,
        stocks - 1
    );
    // Race uses the campaign's rebirth rule while stocks remain. Reaching
    // the gate later still ends it, but the fall prevents No Damage.
    s.complete_race();
    while s.battle.as_mut().unwrap().begin_frame() != Frame::Done {}
    assert_eq!(s.finish_battle(&mut backup), Scene::StageClear);
    assert_eq!(s.manager.total_falls, 1);
    assert!(!bonus::Bonus::NoDamage.contains(&s.data.bonus_get_mask));
}

#[test]
fn session_continue_halves_score_and_retries_without_resetting_attempt_totals() {
    let mut backup = Backup::default();
    let mut s = session::Session::new(SceneData::default(), &backup);
    s.manager.advance(&mut s.data, &mut s.state, &mut backup);
    s.state.players[0].stock_count = -1;
    s.manager.advance(&mut s.data, &mut s.state, &mut backup);
    s.data.score = 101;
    s.manager.total_damage = 80;
    s.manager.total_falls = 3;
    assert_eq!(s.finish_continue(true, &mut backup), Scene::Intro);
    assert_eq!(
        (s.data.score, s.data.continues_used, s.data.stage),
        (50, 1, 0)
    );
    assert_eq!((s.manager.total_damage, s.manager.total_falls), (80, 3));
    assert_eq!(s.state.players[0].stock_count, backup.spgame_stock_count);
    s.manager.advance(&mut s.data, &mut s.state, &mut backup);
    s.state.players[0].stock_count = -1;
    s.manager.advance(&mut s.data, &mut s.state, &mut backup);
    assert_eq!(s.finish_continue(false, &mut backup), Scene::Startup);
    assert_eq!(backup.spgame_records[0].spgame_hiscore, 50);
}

#[test]
fn campaign_bonus_results_save_tasks_and_time_without_consuming_stocks_or_falls() {
    for (stage, tasks_remain) in [(Stage::Bonus1, 2), (Stage::Bonus2, 0)] {
        let mut backup = Backup::default();
        let mut s = session::Session::new(
            SceneData {
                stage: stage as u8,
                ..Default::default()
            },
            &backup,
        );
        s.manager.advance(&mut s.data, &mut s.state, &mut backup);
        s.manager.total_time_tics = 500;
        s.manager.total_falls = 1;
        let mut result = BattleState {
            time_passed: 700,
            time_remain: 61,
            ..Default::default()
        };
        result.players[0].stock_count = -1;
        result.players[0].falls = 1;
        result.players[0].total_damage_all = 12;
        assert_eq!(
            s.finish_bonus_stage(&result, tasks_remain, &mut backup),
            Scene::StageClear
        );
        assert_eq!(s.state.players[0].stock_count, backup.spgame_stock_count);
        assert_eq!(
            (
                s.manager.total_time_tics,
                s.manager.total_falls,
                s.manager.total_damage
            ),
            (500, 1, 12)
        );
        assert_eq!(s.data.bonus_tasks_complete, 10 - tasks_remain);
        assert_eq!(s.data.time_remain, if tasks_remain == 0 { 2 } else { 0 });
        let record = backup.spgame_records[0];
        if stage == Stage::Bonus1 {
            assert_eq!((record.bonus1_task_count, record.bonus1_time), (8, 216_000));
            assert_eq!(s.data.bonus_get_mask, [0; 3]);
        } else {
            assert_eq!((record.bonus2_task_count, record.bonus2_time), (10, 700));
            assert!(bonus::Bonus::Perfect.contains(&s.data.bonus_get_mask));
        }
        assert_eq!(backup.writes, 1);
    }
}

#[test]
fn session_team_replacement_resets_live_match_credit_before_the_next_member() {
    let mut backup = Backup::default();
    let mut s = session::Session::new(
        SceneData {
            stage: Stage::Yoshi as u8,
            ..Default::default()
        },
        &backup,
    );
    s.manager.advance(&mut s.data, &mut s.state, &mut backup);
    s.start_battle(&backup);
    s.damage(0, 1, 20, 20);
    s.fall(
        1,
        bonus::DefeatRecord {
            damage_player: Some(0),
            ..Default::default()
        },
        FighterKind::Yoshi,
    );
    assert_eq!(s.battle.as_ref().unwrap().players[1].stock_count, -1);
    assert!(matches!(
        s.next_enemy(1, &[ssb_engine::math::Vec3::ZERO], 1000.0, 2000.0),
        setup::NextEnemy::Spawn(_)
    ));
    let p = s.battle.as_ref().unwrap().players[1];
    assert_eq!((p.stock_count, p.falls, p.score), (0, 0, 0));
    assert_eq!(s.state.players[1].total_damage_all, 0);
    assert_eq!(s.state.players[1].stock_damage_all, 0);
    assert_eq!(s.state.players[1].total_damage_players, [0; 4]);
    assert_eq!(s.battle.as_ref().unwrap().players[0].score, 1);
}

#[test]
fn very_hard_samus_and_master_hand_use_the_original_extended_handicaps() {
    use crate::stale::apply_ratio_and_handicap;
    let mut backup = Backup {
        spgame_difficulty: Difficulty::VeryHard,
        ..Default::default()
    };
    for (stage, handicap, attack_mul) in [(Stage::Samus, 40, 1.08), (Stage::Boss, 39, 1.5)] {
        let mut s = session::Session::new(
            SceneData {
                stage: stage as u8,
                ..Default::default()
            },
            &backup,
        );
        s.manager.advance(&mut s.data, &mut s.state, &mut backup);
        s.start_battle(&backup);
        assert_eq!(s.state.players[1].handicap, handicap);
        assert_eq!(
            apply_ratio_and_handicap(100.0, 100, handicap, 8),
            100.0 * attack_mul
        );
        let mut f = crate::fighter::Fighter::new(FighterKind::Mario, 0, 3);
        s.state.players[0].stock_damage_all = 90;
        s.state.players[0].total_damage_all = 90;
        s.configure_fighter(
            &mut f,
            crate::status::BlastZone {
                top: 2000.0,
                bottom: -1000.0,
                left: -2000.0,
                right: 2000.0,
            },
        );
        assert_eq!(s.state.players[0].stock_damage_all, 0);
        assert_eq!(s.state.players[0].total_damage_all, 90);
    }
}
