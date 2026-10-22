use super::*;
use crate::computer::attack::Trait as T;
use crate::fighter::FighterKind as K;
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

/// `ifCommonPlayerTagMakeInterface` reads `players[].tag` and `.color`:
/// the player's own port (`sc1pmanager.c:282-283`), an enemy's CP tag and
/// grey (`sc1pgame.c:964-965`), an ally's heart in the player's colour
/// (`sc1pgame.c:1074-1075`).
#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn campaign_tags_and_colours_are_the_players_cp_and_heart() {
    let (_, s, scene) = stage_setup(Stage::Link, K::Mario);
    let me = usize::from(scene.player);
    assert_eq!((s.players[me].tag, s.players[me].color), (Tag::P1, 0));
    let enemy = (0..PLAYERS_MAX)
        .find(|&p| p != me && s.players[p].pkind != PlayerKind::Not)
        .unwrap();
    assert_eq!((s.players[enemy].tag, s.players[enemy].color), (Tag::Cp, 4));
    let (_, s, scene) = stage_setup(Stage::Mario, K::Mario);
    let ally = usize::from(scene.ally_players[0]);
    assert_eq!(
        (s.players[ally].tag, s.players[ally].color),
        (Tag::Heart, scene.player)
    );
    for p in [2, 3] {
        assert_eq!((s.players[p].tag, s.players[p].color), (Tag::Cp, 4));
    }
}

/// `sc1pgame.c:1347`: on the Yoshi Team stage, the enemies wearing the
/// player Yoshi's costume take shade 1; every other enemy shade 0.
#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn yoshi_team_enemies_in_the_players_costume_take_shade_one() {
    let mut shaded = 0;
    for costume in 0..6 {
        let mut state = BattleState::default();
        let mut scene = SceneData {
            stage: Stage::Yoshi as u8,
            fkind: K::Yoshi,
            costume,
            ..Default::default()
        };
        Manager::new(&mut scene, &mut state, &Backup::default());
        scene.costume = costume;
        Game::setup_stage_all(&mut state, &mut scene, Difficulty::Normal, 0, K::Ness);
        for (p, b) in state.players.iter().enumerate() {
            if p == usize::from(scene.player) || b.pkind == PlayerKind::Not {
                continue;
            }
            assert_eq!(
                b.shade,
                u8::from(b.costume == costume),
                "costume {costume} port {p}"
            );
            shaded += usize::from(b.shade);
        }
    }
    // Some player costume meets its twin in the first wave.
    assert!(shaded > 0, "some costume meets its twin");
}

#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
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
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
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
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
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
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
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
    // `sc1PGameSetCameraZoom` runs once, on the proc-set's tick.
    assert!(battle.set_zoom_began);
    for _ in 0..45 {
        assert_ne!(battle.begin_frame(), crate::battle::Frame::Done);
        assert!(battle.set_zoom && !battle.set_zoom_began);
    }
    assert_eq!(battle.begin_frame(), crate::battle::Frame::Done);
}

#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
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

#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn stage_clear_zoom_draws_the_eye_x_index_then_the_eye_y_index() {
    use crate::fighter::Facing;
    use ssb_engine::math::f_clc_dtor32;
    crate::rng::set_seed(77);
    let x_index = crate::rng::rand_int_range(ZOOM_EYE_X.len() as i32) as usize;
    let y_index = crate::rng::rand_int_range(ZOOM_EYE_Y.len() as i32) as usize;
    let after = crate::rng::seed();
    crate::rng::set_seed(77);
    let (x, y) = set_camera_zoom_angles(Facing::Left);
    assert_eq!(crate::rng::seed(), after);
    assert_eq!(x.to_bits(), (-f_clc_dtor32(ZOOM_EYE_X[x_index])).to_bits());
    assert_eq!(y.to_bits(), f_clc_dtor32(ZOOM_EYE_Y[y_index]).to_bits());
    crate::rng::set_seed(77);
    assert_eq!(set_camera_zoom_angles(Facing::Right).0, -x);
}

/// N64 stage-start reads (batch 34): 1P warp builds of the decomp, Mario as
/// player 0, the default backup (Easy, 2 stocks); per CPU fighter `fp->team`,
/// `level`, `handicap` and `computer.trait` on its first frame.
/// (stage, port, team, level, handicap, trait)
const N64_STAGE_START: [(Stage, usize, u8, u8, u8, T); 21] = [
    (Stage::Link, 1, 3, 2, 9, T::Link),
    (Stage::Yoshi, 1, 3, 2, 11, T::YoshiTeam),
    (Stage::Yoshi, 2, 3, 2, 11, T::YoshiTeam),
    (Stage::Yoshi, 3, 3, 2, 11, T::YoshiTeam),
    (Stage::Fox, 1, 3, 3, 9, T::Default),
    (Stage::Mario, 1, 0, 5, 9, T::Ally),
    (Stage::Mario, 2, 3, 3, 9, T::MarioBros),
    (Stage::Mario, 3, 3, 3, 9, T::MarioBros),
    (Stage::Pikachu, 1, 3, 4, 9, T::Default),
    (Stage::Donkey, 1, 0, 4, 7, T::Ally),
    (Stage::Donkey, 2, 0, 4, 7, T::Ally),
    (Stage::Donkey, 3, 3, 4, 26, T::GiantDonkey),
    (Stage::Kirby, 1, 3, 4, 16, T::KirbyTeam),
    (Stage::Kirby, 2, 3, 4, 16, T::KirbyTeam),
    (Stage::Samus, 1, 3, 6, 9, T::Default),
    (Stage::MMario, 1, 3, 3, 31, T::Default),
    (Stage::Zako, 1, 3, 3, 21, T::PolyTeam),
    (Stage::Zako, 2, 3, 3, 21, T::PolyTeam),
    (Stage::Zako, 3, 3, 3, 21, T::PolyTeam),
    (Stage::Boss, 1, 3, 2, 36, T::Default),
    (Stage::Luigi, 1, 3, 7, 6, T::Default),
];

#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn stage_start_levels_traits_and_handicaps_match_the_n64() {
    for (stage, port, team, level, handicap, cp_trait) in N64_STAGE_START {
        let mut state = BattleState::default();
        let mut scene = SceneData {
            stage: stage as u8,
            fkind: K::Mario,
            ..Default::default()
        };
        Manager::new(&mut scene, &mut state, &Backup::default());
        let g = Game::setup_stage_all(&mut state, &mut scene, Difficulty::Easy, 0, K::Ness);
        let p = &state.players[port];
        assert_eq!(
            (p.team, p.level, p.handicap, g.setups[port].cp_trait),
            (team, level, handicap, cp_trait),
            "{stage:?} port {port}"
        );
        // The human keeps `dSCManagerDefaultBattleState`'s level 3 (N64 `fp->level`).
        assert_eq!(state.players[0].level, 3, "{stage:?} player level");
    }
}
