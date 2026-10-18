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
    // `dSCManagerDefaultBackupData`'s Easy, plus one.
    assert_eq!(backup.spgame_records[0].spgame_best_difficulty, 2);
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

/// How the KO order of a team stage's members overlaps: each death is
/// scored at once and its replacement made at the end of its `Dead*` wait
/// (`ftCommonDeadCheckRebirth`), so a second member can fall before the
/// first is replaced.
#[derive(Clone, Copy, Debug)]
enum KoPattern {
    OneAtATime,
    /// Both of two live members fall, then both are replaced, first first.
    Pairs,
    /// Both fall, then the second is replaced before the first.
    PairsReversed,
}

/// Drive a started stage's enemies to the last KO through `Session`'s
/// `fall` and `next_enemy` alone: the stage's team counts, the order its
/// members are made in, and the end at the last stock.
fn clear_enemies(s: &mut session::Session, pattern: KoPattern) -> alloc::vec::Vec<u8> {
    let stage = s.data.stage().unwrap();
    let desc = setup::STAGE_DESC[stage as usize];
    let total = usize::from(desc.opponent_count);
    let at_once = match stage {
        Stage::Kirby => KIRBY_SIM_COUNT,
        Stage::Yoshi | Stage::Zako => MAX_OPPONENT_COUNT,
        _ => total,
    }
    .min(total);
    let mut live: alloc::vec::Vec<u8> = s
        .state
        .present()
        .filter(|(_, p)| p.is_spgame_enemy)
        .map(|(i, _)| i as u8)
        .collect();
    assert_eq!(live.len(), at_once, "{stage:?} initial enemies");
    let game = s.game.as_ref().unwrap();
    assert_eq!(
        usize::from(game.team_players_remaining),
        total - at_once,
        "{stage:?} queued members"
    );
    let mut orders: alloc::vec::Vec<u8> = live
        .iter()
        .map(|&p| game.setups[usize::from(p)].team_order)
        .collect();
    let point = [ssb_engine::math::Vec3::new(1000.0, 0.0, 0.0)];
    let mut kos = 0;
    let mut spawns = 0;
    while !live.is_empty() {
        let n = match pattern {
            KoPattern::OneAtATime => 1,
            _ => live.len().min(2),
        };
        let fallen: alloc::vec::Vec<u8> = live.drain(..n).collect();
        for &port in &fallen {
            let order = s.game.as_ref().unwrap().setups[usize::from(port)].team_order;
            let kind = s.state.players[usize::from(port)].fkind;
            assert_ne!(
                s.battle.as_ref().unwrap().status,
                crate::battle::GameStatus::End,
                "{stage:?}: a KO after the stage ended"
            );
            s.fall(
                port,
                bonus::DefeatRecord {
                    team_order: order,
                    damage_player: Some(s.data.player),
                    ..Default::default()
                },
                kind,
            );
            kos += 1;
            let g = s.game.as_ref().unwrap();
            assert_eq!(
                usize::from(g.enemy_stocks_remaining),
                total - kos,
                "{stage:?} stocks after KO {kos}"
            );
            assert_eq!(g.bonus.defeat_count, kos);
            assert_eq!(
                s.battle.as_ref().unwrap().status == crate::battle::GameStatus::End,
                kos == total,
                "{stage:?}: the end at KO {kos} of {total}"
            );
        }
        let order: alloc::vec::Vec<u8> = match pattern {
            KoPattern::PairsReversed => fallen.iter().rev().copied().collect(),
            _ => fallen,
        };
        for port in order {
            match s.next_enemy(port, &point, 3000.0, 8000.0) {
                setup::NextEnemy::Spawn(e) => {
                    spawns += 1;
                    assert_eq!(e.player, port);
                    assert_eq!(usize::from(e.team_order), at_once + spawns - 1);
                    assert_eq!(e.pos, ssb_engine::math::Vec3::new(1000.0, 5500.0, 0.0));
                    assert_eq!(e.facing, crate::fighter::Facing::Left);
                    assert_eq!(s.state.players[usize::from(port)].stock_count, 0);
                    let b = s.battle.as_ref().unwrap().players[usize::from(port)];
                    assert_eq!((b.present, b.stock_count, b.falls), (true, 0, 0));
                    orders.push(e.team_order);
                    live.push(port);
                }
                setup::NextEnemy::Sleep => {}
            }
        }
    }
    let g = s.game.as_ref().unwrap();
    assert_eq!(kos, total, "{stage:?} KOs");
    assert_eq!(spawns, total - at_once, "{stage:?} replacements");
    assert_eq!((g.team_players_remaining, g.enemy_stocks_remaining), (0, 0));
    if at_once < total {
        assert_eq!(g.current_variation as usize, total, "{stage:?} variations");
    }
    assert_eq!(g.bonus.defeat_count, total);
    if matches!(stage, Stage::Yoshi | Stage::Kirby | Stage::Zako) {
        let mut sorted = orders.clone();
        sorted.sort_unstable();
        assert_eq!(
            sorted,
            (0..total as u8).collect::<alloc::vec::Vec<_>>(),
            "{stage:?} orders"
        );
        let all = if total >= 32 {
            u32::MAX
        } else {
            (1u32 << total) - 1
        };
        assert_eq!(g.enemy_stock_sprite_flags, all);
        assert!(g.team_stock_icons().is_empty());
    }
    assert_eq!(
        s.battle.as_ref().unwrap().status,
        crate::battle::GameStatus::End
    );
    orders
}

/// The user's report: in the stages with many opponents the members keep
/// coming or the stage ends early, and the stage before Master Hand
/// panics. Every multi-fighter stage runs to its last KO, the stage clear
/// and the next stage, for every overlap of falls.
#[test]
fn every_multi_fighter_stage_clears_at_its_last_ko_and_hands_over() {
    for pattern in [
        KoPattern::OneAtATime,
        KoPattern::Pairs,
        KoPattern::PairsReversed,
    ] {
        for (stage, next) in [
            (Stage::Mario, Stage::Pikachu),
            (Stage::Donkey, Stage::Bonus2),
            (Stage::Yoshi, Stage::Fox),
            (Stage::Kirby, Stage::Samus),
            (Stage::Zako, Stage::Boss),
            (Stage::Link, Stage::Yoshi),
            (Stage::Boss, Stage::Boss),
        ] {
            let mut backup = Backup::default();
            let mut s = session::Session::new(
                SceneData {
                    stage: stage as u8,
                    ..Default::default()
                },
                &backup,
            );
            assert_eq!(
                s.manager.advance(&mut s.data, &mut s.state, &mut backup),
                Scene::Battle
            );
            s.start_battle(&backup);
            let allies = s
                .state
                .present()
                .filter(|&(i, p)| i != usize::from(s.data.player) && !p.is_spgame_enemy)
                .count();
            assert_eq!(
                allies,
                usize::from(setup::STAGE_DESC[stage as usize].ally_count),
                "{stage:?} allies"
            );
            clear_enemies(&mut s, pattern);
            if stage == Stage::Boss {
                continue;
            }
            // Through `Set` to the end of the battle.
            let mut frames = 0;
            while s.battle.as_mut().unwrap().begin_frame() != crate::battle::Frame::Done {
                frames += 1;
                assert!(frames < 1000);
            }
            assert_eq!(
                s.finish_battle(&mut backup),
                Scene::StageClear,
                "{stage:?} {pattern:?}"
            );
            assert_eq!(s.finish_stage_clear(&mut backup), Scene::Intro);
            assert_eq!(s.data.stage(), Some(next));
            assert_eq!(
                s.manager.advance(&mut s.data, &mut s.state, &mut backup),
                if next == Stage::Bonus2 {
                    Scene::BonusStage
                } else {
                    Scene::Battle
                }
            );
            if next != Stage::Bonus2 {
                s.start_battle(&backup);
                let enemies = s.state.present().filter(|(_, p)| p.is_spgame_enemy).count();
                assert!(enemies >= 1, "{next:?} enemies");
            }
        }
    }
}

/// The campaign host's KO loop over real fighters: each frame every
/// fighter ticks, its fall is collected, then an enemy whose `Dead*` wait
/// ended is replaced or put to sleep (`psp-game`'s `collect_fall` and
/// `replace_enemies`). With no floor every member falls out of the
/// Polygon stage's team bounds by itself; `drift[i]` sends the i-th member
/// made sideways, and `hold[i]` keeps it still for that many frames, so
/// the KOs come one at a time, in pairs and in threes, through each side.
/// Returns the frame of each KO.
fn play_team_stage(stage: Stage, drift: &[f32], hold: &[u32]) -> alloc::vec::Vec<u32> {
    use crate::dead::StageBounds;
    use crate::fighter::Fighter;
    use crate::status::BlastZone;
    use ssb_engine::math::{Vec2, Vec3};
    let mut backup = Backup::default();
    let mut s = session::Session::new(
        SceneData {
            stage: stage as u8,
            ..Default::default()
        },
        &backup,
    );
    s.manager.advance(&mut s.data, &mut s.state, &mut backup);
    s.start_battle(&backup);
    // `0x10C`'s (the Polygon stage's) bounds.
    let bounds = StageBounds {
        map: BlastZone {
            top: 9000.0,
            bottom: -4200.0,
            left: -9000.0,
            right: 9000.0,
        },
        camera: BlastZone {
            top: 3000.0,
            bottom: -2000.0,
            left: -4000.0,
            right: 4000.0,
        },
        rebirth: Vec2::new(0.0, 1800.0),
        fog_color: [0; 3],
    };
    let team = BlastZone {
        top: 8000.0,
        bottom: -3800.0,
        left: -7500.0,
        right: 7500.0,
    };
    let points = [
        Vec3::new(-1170.0, 2.0, 0.0),
        Vec3::new(1200.0, 2.0, 0.0),
        Vec3::new(0.0, 1803.0, 0.0),
    ];
    let total = usize::from(setup::STAGE_DESC[stage as usize].opponent_count);
    let mut made = 0usize;
    let mut make = |s: &mut session::Session, port: u8, kind: FighterKind, pos: Vec3| {
        let mut f = Fighter::new(kind, port, 3);
        f.dead.bounds = Some(bounds);
        f.pos = pos;
        f.situation = crate::fighter::Situation::Air;
        crate::status::set_fall(&mut f);
        s.configure_fighter(&mut f, team);
        f.physics.vel_air.x = drift.get(made).copied().unwrap_or(0.0);
        let wait = hold.get(made).copied().unwrap_or(0);
        made += 1;
        (f, wait)
    };
    let mut fighters: alloc::vec::Vec<(Fighter, u32)> = alloc::vec::Vec::new();
    let enemies: alloc::vec::Vec<(u8, FighterKind)> = s
        .state
        .present()
        .filter(|(_, p)| p.is_spgame_enemy)
        .map(|(i, p)| (i as u8, p.fkind))
        .collect();
    for (i, (port, kind)) in enemies.into_iter().enumerate() {
        let f = make(
            &mut s,
            port,
            kind,
            Vec3::new(i as f32 * 1000.0 - 1000.0, 1000.0, 0.0),
        );
        fighters.push(f);
    }
    let none = || core::iter::empty::<crate::weapon::MapSurface>();
    let mut kos = alloc::vec::Vec::new();
    for frame in 0..200_000u32 {
        if s.battle.as_ref().unwrap().status == crate::battle::GameStatus::End {
            break;
        }
        for (f, wait) in fighters.iter_mut() {
            if *wait != 0 {
                *wait -= 1;
                continue;
            }
            f.tick_map(none);
        }
        for (f, _) in fighters.iter_mut() {
            let fell = f.dead.scored;
            s.collect_fall(f);
            if fell {
                kos.push(frame);
                assert!(kos.len() <= total, "{stage:?}: KO {} of {total}", kos.len());
            }
        }
        for slot in fighters.iter_mut() {
            if !core::mem::take(&mut slot.0.dead.enemy_next_pending) {
                continue;
            }
            let port = slot.0.port;
            match s.next_enemy(port, &points, bounds.camera.top, bounds.map.top) {
                setup::NextEnemy::Sleep => crate::dead::set_sleep(&mut slot.0),
                setup::NextEnemy::Spawn(e) => *slot = make(&mut s, port, e.fkind, e.pos),
            }
        }
    }
    let g = s.game.as_ref().unwrap();
    assert_eq!(kos.len(), total, "{stage:?} KOs");
    assert_eq!((g.enemy_stocks_remaining, g.team_players_remaining), (0, 0));
    assert_eq!(g.bonus.defeat_count, total);
    assert_eq!(made, total);
    assert_eq!(
        s.battle.as_ref().unwrap().status,
        crate::battle::GameStatus::End
    );
    // Every member was made and KO'd: each fighter is asleep or dead.
    for (f, _) in &fighters {
        use crate::status::{AnyStatus, Status};
        assert!(
            matches!(
                f.status.status,
                AnyStatus::Common(
                    Status::Sleep
                        | Status::DeadDown
                        | Status::DeadLeftRight
                        | Status::DeadUpStar
                        | Status::DeadUpFall
                )
            ),
            "{stage:?}: {:?}",
            f.status.status
        );
    }
    kos
}

/// The user's report on a PSP: the team stages ended with members still
/// on the stage, and the Polygon Team (the stage before Master Hand)
/// panicked. A member put to sleep when the team had no one left was KO'd
/// again every wait where it died, taking the team's stocks and, with two
/// sleepers on one frame at the last stock, a 31st Polygon KO record.
#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn team_stages_end_on_the_last_members_ko_with_sleepers_kept_asleep() {
    for stage in [Stage::Yoshi, Stage::Kirby, Stage::Zako, Stage::Mario] {
        // All at once, every drop in step: the KOs come in threes.
        let kos = play_team_stage(stage, &[], &[]);
        assert!(kos.windows(2).any(|w| w[0] == w[1]));
        // One at a time, through the sides and the bottom.
        let drift: alloc::vec::Vec<f32> = (0..30).map(|i| [0.0, 90.0, -90.0][i % 3]).collect();
        let hold: alloc::vec::Vec<u32> = (0..30).map(|i| (i as u32 % 3) * 40).collect();
        play_team_stage(stage, &drift, &hold);
        // In pairs, through the bottom, while a third member waits: the
        // Polygon Team's last two members fall on one frame at three
        // stocks left, the panic's order.
        let hold: alloc::vec::Vec<u32> =
            (0..30).map(|i| if i % 3 == 2 { 120 } else { 0 }).collect();
        let kos = play_team_stage(stage, &[], &hold);
        assert!(kos.windows(2).any(|w| w[0] == w[1]));
        play_team_stage(stage, &drift, &hold);
    }
}
