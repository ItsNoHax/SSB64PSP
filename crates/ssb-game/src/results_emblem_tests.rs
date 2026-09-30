use super::*;

fn results(kind: Kind, winner: usize) -> Results {
    let no_contest = kind == Kind::NoContest;
    Results {
        kind,
        is_team_battle: matches!(kind, Kind::TimeTeam | Kind::StockTeam),
        present: [true, true, false, false],
        kos: [0; 4],
        tko: [0; 4],
        points: [0; 4],
        places: [1, 0, 0, 0],
        team: [0, 2, 0, 0],
        winner: Some(winner),
        shared_winner: [false; 4],
        total_tics: 0,
        allow_exit_wait: 370,
        draw_wallpaper_tic: if no_contest { 1 } else { 80 },
        make_results_tic: if no_contest { 1 } else { 120 },
        init_fighters_all_tic: if no_contest { 1 } else { 120 },
        character_alpha: 0,
    }
}

#[test]
fn no_contest_makes_no_emblem_and_no_confetti() {
    let mut r = results(Kind::NoContest, 0);
    assert_eq!(Emblem::make(&r, FighterKind::Mario), None);
    r.total_tics = r.make_results_tic;
    assert!(!confetti_due(&r));
}

#[test]
fn the_emblem_is_the_winners_port_colour_or_its_teams() {
    let r = results(Kind::StockRoyal, 1);
    let e = Emblem::make(&r, FighterKind::Kirby).unwrap();
    assert_eq!((e.fighter, e.color), (FighterKind::Kirby, 1));
    assert_eq!(
        (e.translate, e.scale),
        (Vec3::new(0.0, 100.0, -11000.0), 25.0)
    );
    // Team 2 (green) is colour 3.
    let r = results(Kind::StockTeam, 1);
    assert_eq!(Emblem::make(&r, FighterKind::Kirby).unwrap().color, 3);
}

#[test]
fn from_tic_40_the_emblem_shrinks_and_rises_then_holds() {
    let mut r = results(Kind::StockRoyal, 1);
    let mut e = Emblem::make(&r, FighterKind::Kirby).unwrap();
    for t in 1..40 {
        r.total_tics = t;
        e.update(&r);
    }
    assert_eq!((e.translate.y, e.scale), (100.0, 25.0));
    r.total_tics = 40;
    e.update(&r);
    assert_eq!(e.translate.y, 111.0);
    assert!((e.scale - 24.85).abs() < 1e-4);
    // 900 / 11 = 81.8: y reaches 1000 on the 82nd update (tic 121); 15 /
    // 0.15 = 100 updates bring the scale to 10 (tic 139, within rounding).
    let mut y_at = None;
    let mut scale_at = None;
    for t in 41..400 {
        r.total_tics = t;
        e.update(&r);
        if e.translate.y == MAX_Y && y_at.is_none() {
            y_at = Some(t);
        }
        if e.scale == MIN_SCALE && scale_at.is_none() {
            scale_at = Some(t);
        }
    }
    assert_eq!(y_at, Some(121));
    assert!(matches!(scale_at, Some(138..=140)), "{scale_at:?}");
    assert_eq!(
        (e.translate, e.scale),
        (Vec3::new(0.0, 1000.0, -11000.0), 10.0)
    );
}

#[test]
fn the_confetti_falls_on_the_results_text_tic() {
    let mut r = results(Kind::StockRoyal, 1);
    r.total_tics = 119;
    assert!(!confetti_due(&r));
    r.total_tics = 120;
    assert!(confetti_due(&r));
    assert_eq!(CONFETTI[0], (Vec3::new(0.0, 1000.0, -1000.0), false));
    assert_eq!(CONFETTI[1], (Vec3::new(0.0, 1000.0, -400.0), true));
}
