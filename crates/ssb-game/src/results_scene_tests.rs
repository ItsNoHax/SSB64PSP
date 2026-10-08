use super::*;
use crate::battle::{Battle, Player, Rule};

/// The seed is one global; these tests keep to themselves on it. (The
/// workspace gate runs one test thread anyway.)
static RNG: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn rng_lock() -> std::sync::MutexGuard<'static, ()> {
    RNG.lock().unwrap_or_else(|e| e.into_inner())
}

fn results(kind: Kind, places: &[i32], winner: Option<usize>) -> Results {
    let mut present = [false; 4];
    let mut p = [0; 4];
    for (i, &place) in places.iter().enumerate() {
        present[i] = true;
        p[i] = place;
    }
    Results {
        kind,
        is_team_battle: matches!(kind, Kind::TimeTeam | Kind::StockTeam),
        present,
        kos: [0; 4],
        tko: [0; 4],
        points: [0; 4],
        places: p,
        team: [0; 4],
        winner,
        shared_winner: [false; 4],
        total_tics: 0,
        allow_exit_wait: 370,
        draw_wallpaper_tic: 80,
        make_results_tic: 120,
        init_fighters_all_tic: 120,
        character_alpha: 0,
        fighter_kinds: [None; 4],
        audio_thread: crate::results::AudioThread::None,
    }
}

fn entrants(kinds: &[FighterKind]) -> [Option<(FighterKind, u8)>; 4] {
    core::array::from_fn(|i| kinds.get(i).map(|&k| (k, i as u8)))
}

fn fighter(s: &Scene, i: usize) -> Fighter {
    s.fighters[i].expect("fighter made")
}

#[test]
fn no_contest_puts_both_fighters_one_row_back_clapping_and_draws_nothing() {
    let _rng = rng_lock();
    let r = results(Kind::NoContest, &[0, 0], None);
    rng::set_seed(1234);
    let mut s = Scene::start(&r);
    s.init_fighters_all(&r, entrants(&[FighterKind::Mario, FighterKind::Fox]));
    assert_eq!(rng::seed(), 1234, "no contest makes no random pick");
    assert_eq!(s.transition, None);
    let (a, b) = (fighter(&s, 0), fighter(&s, 1));
    // Both share first: `places[2]` pushes them to spot 1.
    assert_eq!(a.pos, Vec3::new(-350.0, -450.0, -2000.0));
    assert_eq!(b.pos, Vec3::new(250.0, -450.0, -2000.0));
    assert_eq!((a.rotate_y, b.rotate_y), (0.0, 0.0));
    assert_eq!(a.status, Some(DemoStatus::Lose));
    assert_eq!(b.status, Some(DemoStatus::Lose));
    assert_eq!((a.scale, b.scale), (1.25, 1.15));
    assert_eq!((a.costume, b.costume), (0, 1));
}

#[test]
fn a_two_player_winner_stands_in_front_and_the_loser_turns_to_it() {
    let _rng = rng_lock();
    let r = results(Kind::StockRoyal, &[1, 0], Some(1));
    rng::set_seed(77);
    let mut s = Scene::start(&r);
    s.init_fighters_all(&r, entrants(&[FighterKind::Luigi, FighterKind::Samus]));
    // The wipe is drawn first, then the winner's pick.
    rng::set_seed(77);
    let wipe = rng::rand_int_range(TRANSITION_COUNT);
    let win =
        [DemoStatus::Win1, DemoStatus::Win2, DemoStatus::Win3][rng::rand_int_range(3) as usize];
    assert_eq!(s.transition, Some(wipe));
    let (lose, winner) = (fighter(&s, 0), fighter(&s, 1));
    assert_eq!(winner.pos, Vec3::new(100.0, -350.0, 0.0));
    assert_eq!(winner.status, Some(win));
    assert_eq!(winner.rotate_y, 0.0);
    assert_eq!(lose.pos, Vec3::new(-350.0, -450.0, -2000.0));
    assert_eq!(lose.status, Some(DemoStatus::Lose));
    let expected = ssb_engine::math::atan2(450.0, 2000.0);
    assert!((lose.rotate_y - expected).abs() < 1e-6);
    assert!(lose.rotate_y > 0.0, "turned right, toward the winner");
}

#[test]
fn kirby_picks_only_win1_or_win2() {
    let _rng = rng_lock();
    let r = results(Kind::TimeRoyal, &[0, 1], Some(0));
    let mut seen = [false; 5];
    for seed in 0..200 {
        rng::set_seed(seed);
        let mut s = Scene::start(&r);
        s.init_fighters_all(&r, entrants(&[FighterKind::Kirby, FighterKind::Mario]));
        seen[fighter(&s, 0).status.unwrap().index()] = true;
    }
    assert_eq!(seen, [true, true, false, false, false]);
}

#[test]
fn every_first_place_picks_in_port_order() {
    let _rng = rng_lock();
    // A shared first: both pick, port 0 first.
    let r = results(Kind::TimeRoyal, &[0, 1, 0], Some(0));
    rng::set_seed(5);
    let mut s = Scene::start(&r);
    s.init_fighters_all(
        &r,
        entrants(&[FighterKind::Mario, FighterKind::Fox, FighterKind::Ness]),
    );
    rng::set_seed(5);
    let _ = rng::rand_int_range(TRANSITION_COUNT);
    let ids = [DemoStatus::Win1, DemoStatus::Win2, DemoStatus::Win3];
    let first = ids[rng::rand_int_range(3) as usize];
    let second = ids[rng::rand_int_range(3) as usize];
    assert_eq!(fighter(&s, 0).status, Some(first));
    assert_eq!(fighter(&s, 2).status, Some(second));
    assert_eq!(fighter(&s, 1).status, Some(DemoStatus::Lose));
}

#[test]
fn three_players_under_a_shared_first_read_below_aheads() {
    let _rng = rng_lock();
    // Places 0, 0, 1: the third has two ahead, so `aheads[-1]`, which the
    // US build reads as 1: spot 2.
    let r = results(Kind::TimeRoyal, &[0, 0, 1], Some(0));
    let mut s = Scene::start(&r);
    s.init_fighters_all(
        &r,
        entrants(&[FighterKind::Mario, FighterKind::Fox, FighterKind::Link]),
    );
    assert_eq!(fighter(&s, 0).pos, Vec3::new(-900.0, -450.0, -2000.0));
    assert_eq!(fighter(&s, 1).pos, Vec3::new(0.0, -450.0, -2000.0));
    assert_eq!(fighter(&s, 2).pos, Vec3::new(1800.0, -700.0, -5000.0));
    // A shared winner does not turn; the others turn to the lower port's.
    assert_eq!(fighter(&s, 1).rotate_y, 0.0);
    assert!(fighter(&s, 2).rotate_y < 0.0);
}

#[test]
fn a_sole_three_player_winner_takes_the_middle_column() {
    let _rng = rng_lock();
    let r = results(Kind::StockRoyal, &[0, 2, 1], Some(0));
    let mut s = Scene::start(&r);
    s.init_fighters_all(
        &r,
        entrants(&[FighterKind::Mario, FighterKind::Fox, FighterKind::Link]),
    );
    // Port 0 moves to the middle column and port 1, which had it, to
    // port 0's; port 2 keeps its own.
    assert_eq!(fighter(&s, 0).pos, Vec3::new(0.0, -350.0, 0.0));
    assert_eq!(fighter(&s, 1).pos, Vec3::new(-2000.0, -700.0, -5000.0));
    assert_eq!(fighter(&s, 2).pos, Vec3::new(800.0, -450.0, -2000.0));
}

#[test]
fn four_players_in_order_swap_the_winner_inward() {
    let _rng = rng_lock();
    let r = results(Kind::StockRoyal, &[0, 1, 2, 3], Some(0));
    let mut s = Scene::start(&r);
    let kinds = [
        FighterKind::Mario,
        FighterKind::Fox,
        FighterKind::Donkey,
        FighterKind::Kirby,
    ];
    s.init_fighters_all(&r, entrants(&kinds));
    assert_eq!(fighter(&s, 0).pos, Vec3::new(-150.0, -350.0, 0.0));
    assert_eq!(fighter(&s, 1).pos, Vec3::new(-900.0, -450.0, -2000.0));
    assert_eq!(fighter(&s, 2).pos, Vec3::new(700.0, -700.0, -5000.0));
    assert_eq!(fighter(&s, 3).pos, Vec3::new(2800.0, -900.0, -9000.0));
    assert_eq!(fighter(&s, 3).scale, 1.22);
}

#[test]
fn only_present_players_with_fighters_are_made() {
    let _rng = rng_lock();
    let r = results(Kind::StockRoyal, &[1, 0], Some(1));
    let mut s = Scene::start(&r);
    s.init_fighters_all(
        &r,
        [
            Some((FighterKind::Mario, 0)),
            None,
            Some((FighterKind::Fox, 0)),
            None,
        ],
    );
    assert!(s.fighters[0].is_some());
    assert!(s.fighters[1].is_none());
    assert!(s.fighters[2].is_none(), "port 2 is not present");
}

#[test]
fn luigi_s_own_demo_rows_skip_his_translate_scales() {
    let _rng = rng_lock();
    for st in [
        DemoStatus::Win1,
        DemoStatus::Win2,
        DemoStatus::Win3,
        DemoStatus::Win4,
    ] {
        assert!(st.skips_translate_scales(FighterKind::Luigi));
        assert!(!st.skips_translate_scales(FighterKind::Mario));
    }
    // His claps are Mario's, played through his scales.
    assert!(!DemoStatus::Lose.skips_translate_scales(FighterKind::Luigi));
}

#[test]
fn the_fighters_come_at_tic_120_and_fade_in_by_0x16() {
    let _rng = rng_lock();
    let mut players = [Player::default(); 4];
    for (i, p) in players.iter_mut().take(2).enumerate() {
        *p = Player {
            present: true,
            team: i as u8,
            ..Player::default()
        };
    }
    let b = Battle::new(Rule::Stock, 3, 0, players);
    let mut r = Results::new(&b);
    assert_eq!(
        (
            r.draw_wallpaper_tic,
            r.make_results_tic,
            r.init_fighters_all_tic
        ),
        (80, 120, 120)
    );
    let mut due = None;
    for tic in 1..=200u32 {
        r.tick(false);
        if r.fighters_due() {
            assert!(due.is_none());
            due = Some(tic);
            assert_eq!(r.character_alpha, 0);
        }
        if tic == 121 {
            assert_eq!(r.character_alpha, 0x16);
        }
    }
    assert_eq!(due, Some(120));
    assert_eq!(r.character_alpha, 0xFF);

    let mut b = b;
    b.is_reset = true;
    let mut r = Results::new(&b);
    r.tick(false);
    assert!(r.fighters_due(), "no contest makes them on its first tic");
}
