use super::{
    bonus::Bonus, continue_scene, frontend, intro, manager::Scene, results, stage_clear, *,
};
use crate::costume::costume_common_id;
use ssb_engine::input::{ControllerState, N64Buttons};

#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn intro_announces_once_at_source_scheduler_boundaries() {
    let mut i = intro::Intro::new(Stage::Link, FighterKind::Donkey);
    assert_eq!(i.tick(1, N64Buttons(0)).announce, [None; 3]);
    assert_eq!(
        i.tick(2, N64Buttons(0)).announce[0],
        Some(intro::Announce::Player(FighterKind::Donkey))
    );
    assert_eq!(i.tick(71, N64Buttons(0)).announce[1], None);
    assert_eq!(
        i.tick(72, N64Buttons(0)).announce[1],
        Some(intro::Announce::Versus)
    );
    assert_eq!(i.tick(131, N64Buttons(0)).announce[2], None);
    assert_eq!(
        i.tick(132, N64Buttons(0)).announce[2],
        Some(intro::Announce::Opponent(Stage::Link))
    );
    assert_eq!(i.tick(133, N64Buttons(0)).announce, [None; 3]);
}

#[test]
fn intro_skip_and_auto_return_do_not_mark_bonus_as_vs() {
    for stage in [Stage::Bonus1, Stage::Bonus2, Stage::Bonus3, Stage::Boss] {
        let mut i = intro::Intro::new(stage, FighterKind::Mario);
        let f = i.tick(1, N64Buttons(N64Buttons::A));
        assert_eq!(
            f.announce[0],
            (stage != Stage::Boss).then_some(intro::Announce::Bonus(stage))
        );
        assert!(!i.tick(59, N64Buttons(N64Buttons::A)).proceed);
        assert!(i.tick(60, N64Buttons(N64Buttons::B)).proceed);
        assert!(!i.tick(361, N64Buttons(N64Buttons::A)).proceed);
    }
    let mut i = intro::Intro::new(Stage::Link, FighterKind::Mario);
    assert!(!i.tick(360, N64Buttons(0)).proceed);
    assert!(i.tick(361, N64Buttons(0)).proceed);
}

#[test]
fn intro_fighters_slide_clamp_and_teams_reveal_on_strict_boundaries() {
    let mut ally = intro::Entrance::ally(2);
    for t in 1..=20 {
        ally.tick(t);
    }
    assert_eq!(ally.z, 0.0);
    let mut team = intro::Entrance::opponent(Stage::Yoshi, FighterKind::Yoshi, 17);
    team.tick(51);
    assert!(!team.shown);
    team.tick(52);
    assert!(team.shown);
    assert_eq!(team.z, 0.0);
    let mut team = intro::Entrance::opponent(Stage::Kirby, FighterKind::Kirby, 7);
    team.tick(70);
    assert!(!team.shown);
    team.tick(71);
    assert!(team.shown);
    assert_eq!(intro::polygon_frames(FighterKind::PolyMario, 0), [false; 3]);
    assert_eq!(
        intro::polygon_frames(FighterKind::PolyMario, 1),
        [true, false, false]
    );
}

fn tick_continue(s: &mut continue_scene::Continue, tap: u16) -> continue_scene::Frame {
    s.tick(Default::default(), N64Buttons(tap))
}

#[test]
fn continue_yes_halves_on_accept_then_waits_four_seconds() {
    let mut c = continue_scene::Continue::new(10001);
    for _ in 0..150 {
        assert_eq!(tick_continue(&mut c, N64Buttons::A).result, None);
    }
    assert_eq!(c.score, 10001);
    assert_eq!(
        tick_continue(&mut c, N64Buttons::A).event,
        Some(continue_scene::Event::Accepted)
    );
    assert_eq!(c.score, 5000);
    for _ in 0..239 {
        assert_eq!(tick_continue(&mut c, N64Buttons::A).result, None);
    }
    assert_eq!(tick_continue(&mut c, 0).result, Some(true));
    assert_eq!(tick_continue(&mut c, 0).result, None);
}

#[test]
fn continue_no_has_strict_ninety_tick_gate_and_thirty_second_auto_exit() {
    let mut c = continue_scene::Continue::new(100);
    for _ in 0..120 {
        tick_continue(&mut c, N64Buttons::D_RIGHT);
    }
    assert!(c.yes);
    tick_continue(&mut c, N64Buttons::D_RIGHT);
    assert!(!c.yes);
    for _ in 0..29 {
        tick_continue(&mut c, 0);
    }
    assert_eq!(
        tick_continue(&mut c, N64Buttons::START).event,
        Some(continue_scene::Event::GameOver)
    );
    for _ in 0..90 {
        assert_eq!(tick_continue(&mut c, N64Buttons::A).result, None);
    }
    assert_eq!(tick_continue(&mut c, N64Buttons::B).result, None); // B does not dismiss
    assert_eq!(tick_continue(&mut c, N64Buttons::A).result, Some(false));
    assert_eq!(c.score, 100);
    let mut c = continue_scene::Continue::new(100);
    for _ in 0..2399 {
        tick_continue(&mut c, 0);
    }
    assert_eq!(
        tick_continue(&mut c, N64Buttons::A).event,
        Some(continue_scene::Event::GameOver)
    );
    for _ in 0..1799 {
        assert_eq!(tick_continue(&mut c, 0).result, None);
    }
    assert_eq!(tick_continue(&mut c, 0).result, Some(false));
}

#[test]
fn continue_render_fades_use_draw_clock_and_prompt_uses_simulation_clock() {
    let mut c = continue_scene::Continue::new(0);
    for _ in 0..40 {
        tick_continue(&mut c, 0);
    }
    assert!(c.spotlight_shown);
    assert_eq!(c.spotlight_fade, Some(255));
    for _ in 0..20 {
        tick_continue(&mut c, 0);
    }
    assert_eq!(c.spotlight_fade, Some(255));
    assert!(c.room_shown && c.prompt_shown);
    assert!(!c.options_shown);
    c.draw();
    assert_eq!(c.room_fade_in, Some(250));
    for _ in 0..51 {
        c.draw();
    }
    assert_eq!(c.room_fade_in, Some(0));
    assert_eq!(c.total_tics, 60);
}

#[test]
fn stage_clear_registers_timer_then_damage_and_opens_exit_after_input() {
    let data = SceneData {
        time_remain: 10,
        score: 1000,
        ..Default::default()
    };
    let mut state = BattleState::default();
    state.players[0].total_damage_given = 50;
    let mut c = stage_clear::StageClear::new(&data, &state, Difficulty::Normal);
    for _ in 0..59 {
        c.tick(N64Buttons(0));
    }
    c.tick(N64Buttons(0));
    assert_eq!(c.total_score, 1500);
    assert_eq!(c.shown_score, 1000);
    for _ in 0..20 {
        c.tick(N64Buttons(0));
    }
    assert_eq!(c.shown_score, 1500);
    for _ in 0..39 {
        c.tick(N64Buttons(0));
    }
    assert_eq!(c.tick(N64Buttons(N64Buttons::A)), None); // 120: gate opens after input
    assert_eq!(c.shown_score, 2000);
    assert_eq!(c.tick(N64Buttons(N64Buttons::A)), Some(2000));
    assert_eq!(c.tick(N64Buttons(N64Buttons::A)), None);
}

#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn stage_clear_bonus_page_reveal_registration_and_proceed_are_separate() {
    let mut data = SceneData::default();
    Bonus::NoItem.insert(&mut data.bonus_get_mask);
    let mut c = stage_clear::StageClear::new(&data, &Default::default(), Difficulty::Normal);
    for _ in 0..130 {
        c.tick(N64Buttons(0));
    }
    assert_eq!(c.total_score, 1000);
    assert_eq!(c.shown_score, 0);
    assert!(!c.row_shown(0));
    c.tick(N64Buttons(0));
    assert!(c.row_shown(0));
    for _ in 0..29 {
        c.tick(N64Buttons(N64Buttons::A));
    }
    assert_eq!(c.shown_score, 1000);
    for _ in 0..20 {
        assert_eq!(c.tick(N64Buttons(N64Buttons::A)), None);
    }
    assert_eq!(c.tick(N64Buttons(N64Buttons::A)), Some(1000));
}

#[test]
fn nine_rows_need_next_page_and_do_not_duplicate_score_under_repeated_taps() {
    let mut data = SceneData::default();
    data.bonus_get_mask[0] = (1 << 10) - 1;
    let mut c = stage_clear::StageClear::new(&data, &Default::default(), Difficulty::Normal);
    for _ in 0..240 {
        c.tick(N64Buttons(0));
    }
    assert!(c.page_arrow_shown());
    assert!(!c.allow_proceed);
    assert_eq!(c.rows.iter().flatten().count(), 9);
    c.tick(N64Buttons(N64Buttons::B));
    assert_eq!(c.rows.iter().flatten().count(), 1);
    assert_eq!(c.rows[0].unwrap().bonus_id, 9);
    let expected = results::score(&data, &Default::default(), Difficulty::Normal);
    for _ in 0..50 {
        assert_eq!(c.tick(N64Buttons(N64Buttons::A)), None);
    }
    assert_eq!(c.tick(N64Buttons(N64Buttons::A)), Some(expected));
}

#[test]
fn bonus_tasks_register_every_ten_ticks_then_time_only_for_complete_tasks() {
    let mut data = SceneData {
        stage: Stage::Bonus1 as u8,
        time_remain: 20,
        bonus_tasks_complete: 2,
        ..Default::default()
    };
    let mut c = stage_clear::StageClear::new(&data, &Default::default(), Difficulty::Normal);
    for _ in 0..20 {
        c.tick(N64Buttons(0));
    }
    assert_eq!(c.total_score, 1000);
    assert_eq!(c.objectives_shown, 0);
    c.tick(N64Buttons(0));
    assert_eq!(c.objectives_shown, 1);
    for _ in 0..89 {
        c.tick(N64Buttons(0));
    }
    assert!(!c.timer_text);
    assert!(c.allow_proceed);
    assert_eq!(c.tick(N64Buttons(N64Buttons::A)), Some(2000));
    data.bonus_tasks_complete = 10;
    let mut c = stage_clear::StageClear::new(&data, &Default::default(), Difficulty::Normal);
    for _ in 0..190 {
        c.tick(N64Buttons(0));
    }
    assert_eq!(c.tick(N64Buttons(N64Buttons::START)), Some(14000));
}

#[test]
fn infinite_time_suppresses_time_score_but_race_still_uses_its_fixed_timer() {
    for (stage, expected, deadline) in [
        (Stage::Link, 0, 60),
        (Stage::Bonus1, 10000, 120),
        (Stage::Bonus3, 10000, 80),
    ] {
        let data = SceneData {
            stage: stage as u8,
            time_limit: 100,
            time_remain: 20,
            bonus_tasks_complete: 10,
            ..Default::default()
        };
        let mut c = stage_clear::StageClear::new(&data, &Default::default(), Difficulty::Normal);
        for _ in 0..deadline {
            c.tick(N64Buttons(0));
        }
        assert_eq!(c.tick(N64Buttons(N64Buttons::A)), Some(expected));
        assert_eq!(
            results::score(&data, &Default::default(), Difficulty::Normal),
            expected
        );
    }
}

#[test]
fn timed_ledger_matches_final_ledger_for_every_stage_and_difficulty() {
    for stage in Stage::ALL.into_iter().take(14) {
        for difficulty in [
            Difficulty::VeryEasy,
            Difficulty::Easy,
            Difficulty::Normal,
            Difficulty::Hard,
            Difficulty::VeryHard,
        ] {
            for time_limit in [5, 100] {
                let mut data = SceneData {
                    stage: stage as u8,
                    time_limit,
                    time_remain: 63,
                    bonus_tasks_complete: 10,
                    score: 200,
                    ..Default::default()
                };
                for bonus in [
                    Bonus::CheapShot,
                    Bonus::NoItem,
                    Bonus::Hawk,
                    Bonus::Shooter,
                    Bonus::HeavyDamage,
                    Bonus::AllVariations,
                    Bonus::ItemStrike,
                    Bonus::DoubleKO,
                    Bonus::Trickster,
                    Bonus::NoDamage,
                    Bonus::Perfect,
                ] {
                    bonus.insert(&mut data.bonus_get_mask);
                }
                let mut state = BattleState::default();
                state.players[0].total_damage_given = 91;
                let expected = results::score(&data, &state, difficulty);
                let mut c = stage_clear::StageClear::new(&data, &state, difficulty);
                let total = (0..1000)
                    .find_map(|_| c.tick(N64Buttons(N64Buttons::A)))
                    .expect("scene completes");
                assert_eq!(
                    total, expected,
                    "{stage:?} {difficulty:?} time={time_limit}"
                );
            }
        }
    }
}

#[test]
fn frontend_saves_select_once_and_requests_real_bonus_scene() {
    use crate::players_1p::{self, Outcome, Players1P};
    let mut backup = Backup::default();
    let mut select = Players1P::new(
        players_1p::SceneData {
            kind: Some(FighterKind::Mario),
            ..Default::default()
        },
        &backup,
    );
    let mut saved = None;
    for t in 1..=91 {
        let tap = N64Buttons(if t == 61 { N64Buttons::START } else { 0 });
        if let Some(Outcome::Proceed(s)) = select.tick(Default::default(), tap) {
            assert_eq!(t, 91);
            saved = Some(s);
        }
    }
    let mut scene = SceneData::default();
    saved.expect("Proceed").apply(&mut scene, &mut backup);
    let mut f = frontend::Frontend::campaign(scene, &backup);
    let mut events = alloc::vec::Vec::new();
    assert_eq!(backup.writes, 1);
    assert!(matches!(f.screen, frontend::Screen::Intro(_)));
    f.session.as_mut().unwrap().data.stage = Stage::Bonus1 as u8;
    f.sync(&backup);
    f.tick(
        60,
        Default::default(),
        N64Buttons(N64Buttons::A),
        &mut backup,
        |e| events.push(e),
    );
    assert_eq!(
        events.last(),
        Some(&frontend::Event::Host(Scene::BonusStage))
    );
    assert!(f.session.as_ref().unwrap().battle.is_none());
    let n = events.len();
    f.tick(
        61,
        Default::default(),
        N64Buttons(N64Buttons::A),
        &mut backup,
        |e| events.push(e),
    );
    assert_eq!(events.len(), n);
}

#[test]
fn frontend_continue_halves_once_and_retry_keeps_campaign_stage() {
    let mut backup = Backup::default();
    let mut f = frontend::Frontend::campaign(Default::default(), &backup);
    let mut s = session::Session::new(
        SceneData {
            stage: Stage::Fox as u8,
            ..Default::default()
        },
        &backup,
    );
    s.manager.scene = Scene::Continue;
    s.data.score = 10001;
    f.session = Some(alloc::boxed::Box::new(s));
    f.sync(&backup);
    for t in 1..=151 {
        f.tick(
            t,
            Default::default(),
            N64Buttons(if t == 151 { N64Buttons::A } else { 0 }),
            &mut backup,
            |_| {},
        );
    }
    assert_eq!(f.session.as_ref().unwrap().data.score, 5000);
    assert_eq!(f.session.as_ref().unwrap().data.continues_used, 0);
    for t in 152..=391 {
        f.tick(t, Default::default(), N64Buttons(0), &mut backup, |_| {});
    }
    let s = f.session.as_ref().unwrap();
    assert_eq!(s.data.score, 5000);
    assert_eq!(s.data.continues_used, 1);
    assert_eq!(s.data.stage(), Some(Stage::Fox));
    assert!(matches!(f.screen, frontend::Screen::Intro(_)));
}

#[test]
fn frontend_stage_clear_commits_ledger_once_before_next_intro() {
    let mut backup = Backup::default();
    let mut f = frontend::Frontend::campaign(Default::default(), &backup);
    let mut s = session::Session::new(Default::default(), &backup);
    s.data.score = 1000;
    s.data.time_remain = 10;
    s.manager.scene = Scene::StageClear;
    f.session = Some(alloc::boxed::Box::new(s));
    f.sync(&backup);
    for t in 1..=121 {
        f.tick(
            t,
            Default::default(),
            N64Buttons(if t == 121 { N64Buttons::A } else { 0 }),
            &mut backup,
            |_| {},
        );
    }
    let s = f.session.as_ref().unwrap();
    assert_eq!(s.data.score, 1500);
    assert_eq!(s.data.stage(), Some(Stage::Yoshi));
    assert_eq!(s.manager.scene, Scene::Intro);
}

#[test]
fn host_select_campaign_starts_at_the_first_intro() {
    let backup = Backup::default();
    let data = SceneData {
        fkind: FighterKind::Fox,
        costume: costume_common_id(FighterKind::Fox, 1),
        ..Default::default()
    };
    let f = frontend::Frontend::campaign(data, &backup);
    assert_eq!(backup.writes, 0);
    let frontend::Screen::Intro(intro) = &f.screen else {
        panic!("intro");
    };
    assert_eq!((intro.stage, intro.player), (Stage::Link, FighterKind::Fox));
    let session = f.session.as_ref().unwrap();
    assert_eq!(session.manager.scene, Scene::Intro);
}

/// A whole campaign through the frontend, A every 30 ticks: each stage's
/// intro, its battle (won; the Polygon Team lost once and continued), its
/// results, to the ending. At each intro the fighters the VS name reads
/// must have a name sprite. Returns how many intros had Polygons on the
/// ally ports.
fn campaign_walk() -> u32 {
    let mut backup = Backup {
        unlock_mask: !0,
        ..Default::default()
    };
    let mut f = frontend::Frontend::campaign(Default::default(), &backup);
    f.session = Some(alloc::boxed::Box::new(session::Session::new(
        SceneData {
            fkind: FighterKind::Kirby,
            ..Default::default()
        },
        &backup,
    )));
    f.sync(&backup);
    let mut polygons_on_ally_ports = 0;
    let mut lost_zako = false;
    let mut tic = 0;
    for _ in 0..64 {
        let s = f.session.as_ref().unwrap();
        let Some(stage) = s.data.stage() else { break };
        if s.manager.scene == Scene::Ending {
            break;
        }
        assert_eq!(s.manager.scene, Scene::Intro, "{stage:?}");
        // RE-478: `sc1PIntroMakeVSName` names only the stage's allies.
        let named: alloc::vec::Vec<_> = intro::named_allies(stage, &s.data, &s.state).collect();
        assert!(
            named.iter().all(|k| k.is_playable()),
            "{stage:?}: {named:?}"
        );
        assert_eq!(named.len(), intro::allies_num(stage));
        if s.data
            .ally_players
            .iter()
            .any(|&p| s.state.players[usize::from(p)].fkind.is_polygon())
        {
            polygons_on_ally_ports += 1;
        }
        let mut host = None;
        for _ in 0..2000 {
            tic += 1;
            f.tick(
                tic,
                Default::default(),
                N64Buttons(if tic % 30 == 0 { N64Buttons::A } else { 0 }),
                &mut backup,
                |e| {
                    if let frontend::Event::Host(scene) = e {
                        host = Some(scene);
                    }
                },
            );
            if host.is_some() {
                break;
            }
        }
        let sp = f.session.as_mut().unwrap();
        match host.expect("intro ends") {
            Scene::BonusStage => {
                sp.finish_bonus_stage(&BattleState::default(), 0, &mut backup);
            }
            Scene::Battle => {
                sp.start_battle(&backup);
                if stage == Stage::Bonus3 {
                    sp.complete_race();
                }
                if stage == Stage::Zako && !lost_zako {
                    lost_zako = true;
                    sp.battle.as_mut().unwrap().players[usize::from(sp.data.player)].stock_count =
                        -1;
                }
                sp.finish_battle(&mut backup);
            }
            other => panic!("{other:?}"),
        }
        f.sync(&backup);
        // The results (or the continue), A until the next intro.
        for _ in 0..2000 {
            let scene = f.session.as_ref().unwrap().manager.scene;
            if matches!(scene, Scene::Intro | Scene::Ending) {
                break;
            }
            tic += 1;
            f.tick(
                tic,
                Default::default(),
                N64Buttons(if tic % 30 == 0 { N64Buttons::A } else { 0 }),
                &mut backup,
                |_| {},
            );
        }
    }
    assert_eq!(f.session.as_ref().unwrap().manager.scene, Scene::Ending);
    polygons_on_ally_ports
}

/// RE-478: the Polygon Team's intro after Race to the Finish (and again
/// after its continue) and Master Hand's after the Polygon Team find
/// Polygons on the ally ports; the intro must not name them.
#[test]
fn every_campaign_intro_names_only_its_own_allies() {
    assert_eq!(campaign_walk(), 3);
}

/// RE-469's traps over the same campaign: a PSP traps the FPU's
/// divide-by-zero, invalid and overflow, which a host does not by default
/// (glibc `feenableexcept`); a `0 / 0` in the results' ledgers, the bonus
/// checks or the intros' entrances kills this test with SIGFPE.
#[cfg(all(
    target_os = "linux",
    target_env = "gnu",
    any(target_arch = "x86_64", target_arch = "x86")
))]
#[test]
fn a_campaign_through_every_stage_raises_no_fpu_exception() {
    extern "C" {
        fn feenableexcept(excepts: i32) -> i32;
        fn fedisableexcept(excepts: i32) -> i32;
    }
    const FE_INVALID: i32 = 0x01;
    const FE_DIVBYZERO: i32 = 0x04;
    const FE_OVERFLOW: i32 = 0x08;
    let traps = FE_INVALID | FE_DIVBYZERO | FE_OVERFLOW;
    unsafe { feenableexcept(traps) };
    let polygons = campaign_walk();
    unsafe { fedisableexcept(traps) };
    assert_eq!(polygons, 3);
}

#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn intro_plays_its_bgm_and_the_announcers_voices() {
    use crate::sound::{id, testing::*};
    let r = Recorder::install();
    let mut i = intro::Intro::new(Stage::Yoshi, FighterKind::Donkey);
    assert_eq!(r.take(), [Call::PlayBgm(0, id::nSYAudioBGM1PIntro)]);
    for tic in 1..=200 {
        i.tick(tic, N64Buttons(0));
    }
    assert_eq!(
        r.take(),
        [
            Call::PlayFgm(id::nSYAudioVoiceAnnounceDonkey),
            Call::PlayFgm(id::nSYAudioVoiceAnnounceVersus),
            Call::PlayFgm(id::nSYAudioVoiceAnnounceYoshiTeam),
        ]
    );
    // A tap after 60 tics leaves and stops the voices.
    assert!(i.tick(201, N64Buttons(N64Buttons::A)).proceed);
    assert_eq!(r.take(), [Call::StopAllFgm]);
    let _ = intro::Intro::new(Stage::Boss, FighterKind::Mario);
    assert_eq!(r.take(), [Call::PlayBgm(0, id::nSYAudioBGMBossStage)]);
    crate::sound::uninstall();
}

#[test]
fn continue_plays_the_choice_then_game_over() {
    use crate::sound::{id, testing::*};
    let r = Recorder::install();
    let mut c = continue_scene::Continue::new(1000);
    assert_eq!(
        r.take(),
        [
            Call::StopBgmAll,
            Call::PlayBgm(0, id::nSYAudioBGM1PGameEndChoice)
        ]
    );
    let idle = ControllerState::default();
    for _ in 0..151 {
        c.tick(idle, N64Buttons(0));
    }
    assert_eq!(r.take(), [Call::PlayFgm(id::nSYAudioVoiceAnnounceContinue)]);
    c.tick(idle, N64Buttons(N64Buttons::D_RIGHT));
    assert_eq!(r.take(), [Call::PlayFgm(id::nSYAudioFGMMenuScroll1)]);
    c.tick(idle, N64Buttons(N64Buttons::A));
    assert_eq!(
        r.take(),
        [
            Call::PlayBgm(0, id::nSYAudioBGM1PGameOver),
            Call::PlayFgm(id::nSYAudioVoiceAnnounceGameOver),
        ]
    );
    crate::sound::uninstall();
}

/// N64 loss trace (Continue tic t on frame 965 + t): A tapped on tics
/// 2, 10, 18, ... reaches the Intro on frame 1359 (tic 394); no input
/// reaches the Startup scene on frame 5165 (tic 4200).
#[test]
fn continue_matches_the_n64_loss_trace() {
    let mut c = continue_scene::Continue::new(0);
    let mut accepted = None;
    let result = (1..=500u32).find_map(|t| {
        let f = tick_continue(&mut c, if t % 8 == 2 { N64Buttons::A } else { 0 });
        if f.event == Some(continue_scene::Event::Accepted) {
            accepted = Some(t);
        }
        f.result.map(|r| (t, r))
    });
    assert_eq!(accepted, Some(154));
    assert_eq!(result, Some((394, true)));
    let mut c = continue_scene::Continue::new(0);
    let result = (1..=5000u32).find_map(|t| tick_continue(&mut c, 0).result.map(|r| (t, r)));
    assert_eq!(result, Some((4200, false)));
}
