use super::*;
use crate::spgame::{manager::Manager, Backup};

fn stage_setup(stage: Stage, kind: FighterKind) -> (Game, BattleState, SceneData) {
    let mut state = BattleState::default();
    let mut scene = SceneData {
        stage: stage as u8,
        fkind: kind,
        ..Default::default()
    };
    Manager::new(&mut scene, &mut state, &Backup::default());
    let game = Game::setup_stage_all(
        &mut state,
        &mut scene,
        Difficulty::Normal,
        0,
        FighterKind::Ness,
    );
    (game, state, scene)
}

#[test]
fn mario_bros_and_giant_donkey_setup_ports_traits_teams_and_handicaps() {
    let (g, s, _) = stage_setup(Stage::Mario, FighterKind::Mario);
    assert_eq!((s.pl_count, s.cp_count, s.is_team_attack), (1, 3, false));
    assert_eq!(s.players[1].tag, Tag::Heart);
    assert_eq!(
        (s.players[2].fkind, s.players[3].fkind),
        (K::Mario, K::Luigi)
    );
    assert_eq!(s.players[2].costume, costume_common_id(K::Mario, 1));
    assert_eq!(g.setups[2].cp_trait, T::MarioBros);
    let (g, s, _) = stage_setup(Stage::Donkey, K::Donkey);
    assert_eq!(
        (s.players[3].fkind, s.players[3].handicap),
        (K::GiantDonkey, 27)
    );
    assert_eq!((s.players[1].handicap, s.players[2].handicap), (7, 7));
    assert_eq!(g.setups[3].cp_trait, T::GiantDonkey);
}

#[test]
fn team_replacements_exhaust_exactly_eighteen_yoshis_and_thirty_polygons() {
    for (stage, total, unique) in [(Stage::Yoshi, 18, 6), (Stage::Zako, 30, 12)] {
        let (mut game, mut state, scene) = stage_setup(stage, K::Yoshi);
        assert_eq!(game.team_players_remaining, total - 3);
        let mut bits = 0u32;
        for &v in &game.variations[..unique] {
            bits |= 1 << v;
        }
        assert_eq!(bits.count_ones(), unique as u32);
        for i in unique..total as usize {
            assert_eq!(game.variations[i], game.variations[i % unique]);
        }
        let points = [Vec3::new(100.0, 0.0, 0.0)];
        for order in 3..total {
            let NextEnemy::Spawn(spawn) =
                game.spawn_enemy_team_next(&mut state, &scene, 1, &points, 1000.0, 2000.0)
            else {
                panic!("missing member");
            };
            assert_eq!(
                (spawn.team_order, spawn.pos.y, spawn.facing),
                (order, 1500.0, Facing::Left)
            );
            assert_eq!(state.players[1].stock_count, 0);
        }
        assert_eq!(
            game.spawn_enemy_team_next(&mut state, &scene, 1, &points, 1000.0, 2000.0),
            NextEnemy::Sleep
        );
    }
}

#[test]
fn eighth_kirby_uses_managers_final_copy_and_race_has_no_replacements() {
    let (mut g, mut s, scene) = stage_setup(Stage::Kirby, K::Kirby);
    assert_eq!((s.cp_count, g.team_players_remaining), (2, 6));
    assert_ne!(s.players[1].costume, scene.costume);
    for _ in 0..6 {
        g.spawn_enemy_team_next(&mut s, &scene, 1, &[Vec3::ZERO], 0.0, 0.0);
    }
    assert_eq!(g.setups[1].copy_kind, K::Ness);
    let (mut g, mut s, scene) = stage_setup(Stage::Bonus3, K::Mario);
    assert_eq!((s.time_limit, g.team_players_remaining), (1, 0));
    assert!(g.setups.iter().all(|p| p.is_skip_entry));
    assert_eq!(
        g.spawn_enemy_team_next(&mut s, &scene, 1, &[], 0.0, 0.0),
        NextEnemy::Sleep
    );
}

#[test]
fn final_enemy_end_preserves_player_stock_and_waits_for_victory_zoom() {
    let (mut game, state, scene) = stage_setup(Stage::Link, K::Mario);
    let mut battle = Battle::new_1p(5, state.battle_players(), true, 391);
    battle.on_fall(1, Some(0));
    game.set_player_defeat_stats(
        &mut battle,
        &state,
        &scene,
        1,
        DefeatRecord {
            damage_player: Some(0),
            ..Default::default()
        },
        K::Link,
    );
    assert_eq!(battle.players[0].stock_count, 2);
    for _ in 0..91 {
        battle.begin_frame();
    }
    assert_eq!(battle.status, GameStatus::Set);
    assert!(battle.set_zoom);
    for _ in 0..45 {
        assert_ne!(battle.begin_frame(), crate::battle::Frame::Done);
    }
    assert_eq!(battle.begin_frame(), crate::battle::Frame::Done);
}

#[test]
fn team_stock_icons_omit_defeated_members_and_wrap_at_ten() {
    let (mut g, _, _) = stage_setup(Stage::Yoshi, K::Mario);
    let icons = g.team_stock_icons();
    assert_eq!((icons[0].x, icons[0].y), (20, 20));
    assert_eq!((icons[10].x, icons[10].y), (20, 30));
    g.enemy_stock_sprite_flags = 1 << 17;
    g.enemy_stocks_remaining -= 1;
    assert_eq!(g.team_stock_icons().len(), 17);
    assert_eq!(g.team_stock_icons()[0].costume, Some(g.variations[16]));
}
