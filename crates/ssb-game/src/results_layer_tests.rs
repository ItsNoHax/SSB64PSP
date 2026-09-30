use super::*;

/// The seed is one global; these tests keep to themselves on it.
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
    let no_contest = kind == Kind::NoContest;
    let tic = |t: u32| if no_contest { 1 } else { t };
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
        draw_wallpaper_tic: tic(80),
        make_results_tic: tic(120),
        init_fighters_all_tic: tic(120),
        character_alpha: 0,
    }
}

fn player(kind: FighterKind, human: bool, color: u8) -> Option<Player> {
    Some(Player {
        kind,
        costume: 0,
        human,
        color,
    })
}

/// Ticks the results and the layer to `tic`.
fn run_to(r: &mut Results, layer: &mut Layer, tic: u32) {
    while r.total_tics < tic {
        r.tick(false);
        layer.tick(r);
    }
}

fn draws(layer: &Layer, r: &Results, players: &[Option<Player>; 4]) -> Vec<Draw> {
    let mut out = Vec::new();
    layer.visit(r, players, |d| out.push(d));
    out
}

fn pieces(d: &[Draw]) -> Vec<Piece> {
    d.iter()
        .filter_map(|d| match d {
            Draw::Sprite(p) => Some(*p),
            _ => None,
        })
        .collect()
}

fn numbers_at(x: f32, y: f32, n: i32) -> Vec<(Sprite, f32, f32)> {
    let mut out = Vec::new();
    make_number(x, y, n, 4, &mut |d| {
        if let Draw::Sprite(p) = d {
            out.push((p.sprite, p.x, p.y));
        }
    });
    out
}

fn digit(d: u8) -> Sprite {
    Sprite::In(SpriteFile::Digits, d)
}

#[test]
fn a_number_right_aligns_its_digits_and_leads_with_a_dash_when_negative() {
    assert_eq!(numbers_at(100.0, 66.0, 5), [(digit(5), 124.0, 66.0)]);
    assert_eq!(
        numbers_at(100.0, 66.0, 12),
        [(digit(1), 116.0, 66.0), (digit(2), 124.0, 66.0)]
    );
    // A zero tens digit still shows under a hundreds digit.
    assert_eq!(
        numbers_at(100.0, 66.0, 105),
        [
            (digit(1), 108.0, 66.0),
            (digit(0), 116.0, 66.0),
            (digit(5), 124.0, 66.0)
        ]
    );
    // The dash sits 3 lower, just left of the first digit.
    assert_eq!(
        numbers_at(100.0, 66.0, -3),
        [(digit(DASH), 116.0, 69.0), (digit(3), 124.0, 66.0)]
    );
    assert_eq!(
        numbers_at(100.0, 66.0, -12),
        [
            (digit(DASH), 108.0, 69.0),
            (digit(1), 116.0, 66.0),
            (digit(2), 124.0, 66.0)
        ]
    );
    assert_eq!(numbers_at(100.0, 66.0, -120)[0], (digit(DASH), 100.0, 69.0));
}

#[test]
fn a_string_advances_by_letter_widths_and_digits_by_pixels() {
    let mut xs = Vec::new();
    make_string(WINS, 175.0, 180.0, 3, 1.0, &mut |d| {
        if let Draw::Sprite(p) = d {
            xs.push((p.sprite, p.x));
        }
    });
    let a = |c: u8| Sprite::In(SpriteFile::Announce, c);
    // W 39, I 9, N 29, S 24, each plus the 1 after it.
    assert_eq!(
        xs,
        [
            (a(22), 175.0),
            (a(8), 215.0),
            (a(13), 225.0),
            (a(18), 255.0),
            (a(26), 280.0)
        ]
    );
    // The scale is x only, and a period sits 26 lower.
    let mut ps = Vec::new();
    make_string("C2.2F", 27.0, 180.0, 0, 0.7, &mut |d| {
        if let Draw::Sprite(p) = d {
            ps.push(p);
        }
    });
    assert_eq!(ps.len(), 3);
    assert!((ps[1].x - (27.0 + 24.0 * 0.7 + 2.0)).abs() < 1e-4);
    assert_eq!((ps[1].y, ps[1].scale_x), (206.0, 0.7));
    // "NO CONTEST": the space is 10 wide.
    let mut n = Vec::new();
    make_string("NO CONTEST", 30.0, 180.0, 4, 1.0, &mut |d| {
        if let Draw::Sprite(p) = d {
            n.push(p);
        }
    });
    assert_eq!(n.len(), 9);
    assert_eq!(n[2].x, 30.0 + 29.0 + 34.0 + 10.0);
    // White letters edged in black.
    assert_eq!((n[0].prim, n[0].env), (Some([0xFF; 3]), [0; 3]));
}

#[test]
fn columns_spread_by_present_count() {
    let r = results(Kind::StockRoyal, &[0, 1], Some(0));
    assert_eq!((column_x(&r, 0), column_x(&r, 1)), (135.0, 215.0));
    let r = results(Kind::StockRoyal, &[0, 1, 2], Some(0));
    assert_eq!(column_x(&r, 2), 225.0);
    let mut r = results(Kind::StockRoyal, &[0, 1, 2, 3], Some(0));
    assert_eq!(column_x(&r, 3), 235.0);
    // Ports 0 and 2: the second present player takes the second column.
    r.present = [true, false, true, false];
    assert_eq!(column_x(&r, 2), 215.0);
}

#[test]
fn the_wallpaper_fades_in_after_tic_80_in_the_winners_colour() {
    let _rng = rng_lock();
    rng::set_seed(77);
    let mut r = results(Kind::StockRoyal, &[1, 0], Some(1));
    let mut l = Layer::new();
    run_to(&mut r, &mut l, 1);
    assert_eq!(l.wallpaper_tint_alpha, 250);
    run_to(&mut r, &mut l, 51);
    assert_eq!(l.wallpaper_tint_alpha, 0, "stops at 0");
    run_to(&mut r, &mut l, 79);
    assert_eq!(l.wallpaper, None);
    run_to(&mut r, &mut l, 80);
    assert_eq!(l.wallpaper, Some(1), "port 2's blue");
    assert_eq!(l.wallpaper_tint2_alpha, Some(250));
    assert_eq!(rng::seed(), 77, "a contest's wallpaper draws nothing");
    let d = draws(&l, &r, &[None; 4]);
    assert_eq!(
        d[0],
        Draw::Wallpaper {
            prim: [0x86, 0x86, 0xD1],
            env: [0x39, 0x39, 0x99]
        }
    );
    assert_eq!(
        d[1],
        Draw::Fill {
            rect: VIEWPORT_RECT,
            color: [0, 0, 0, 250]
        }
    );
    // The emblem's camera draws between the two fades' (RE-420).
    assert_eq!(d[2], Draw::Emblem);
    assert_eq!(d[3], Draw::Fighters);
    run_to(&mut r, &mut l, 130);
    assert_eq!(l.wallpaper_tint2_alpha, Some(0));
}

#[test]
fn a_team_wallpaper_takes_the_teams_colour() {
    let mut r = results(Kind::TimeTeam, &[1, 0], Some(1));
    r.team = [0, 2, 0, 0];
    let mut l = Layer::new();
    run_to(&mut r, &mut l, 80);
    assert_eq!(l.wallpaper, Some(3), "green");
}

#[test]
fn no_contest_draws_its_wallpaper_colour_on_the_first_tic() {
    let _rng = rng_lock();
    rng::set_seed(77);
    let mut r = results(Kind::NoContest, &[0, 0], None);
    let mut l = Layer::new();
    run_to(&mut r, &mut l, 1);
    assert_ne!(rng::seed(), 77);
    rng::set_seed(77);
    let pick = rng::rand_int_range(4) as usize;
    assert_eq!(l.wallpaper, Some(pick));
    assert_eq!(l.wallpaper_tint2_alpha, None);
}

#[test]
fn the_tint_rises_by_9_to_0x80_and_the_bar_grows_by_10_to_190() {
    let mut r = results(Kind::StockRoyal, &[0, 1], Some(0));
    let mut l = Layer::new();
    run_to(&mut r, &mut l, 179);
    assert_eq!(l.tint_alpha, None);
    run_to(&mut r, &mut l, 180);
    assert_eq!(l.tint_alpha, Some(9));
    run_to(&mut r, &mut l, 194);
    assert_eq!(l.tint_alpha, Some(0x80));
    run_to(&mut r, &mut l, 229);
    assert_eq!((l.bar, l.bar_width), (None, 0));
    run_to(&mut r, &mut l, 230);
    assert_eq!((l.bar, l.bar_width), (Some(110), 10));
    run_to(&mut r, &mut l, 300);
    assert_eq!(l.bar_width, 190);
    let d = draws(&l, &r, &[None; 4]);
    assert!(d.contains(&Draw::Fill {
        rect: [87.0, 110.0, 278.0, 111.0],
        color: [0xFF; 4]
    }));
}

#[test]
fn a_stock_battles_table_is_places_bar_then_header_and_kos() {
    let mut r = results(Kind::StockRoyal, &[1, 0], Some(1));
    r.kos = [0, 1, 0, 0];
    let players = [
        player(FighterKind::Luigi, true, 0),
        player(FighterKind::Kirby, false, 1),
        None,
        None,
    ];
    let mut l = Layer::new();
    run_to(&mut r, &mut l, 209);
    let before = pieces(&draws(&l, &r, &players));
    run_to(&mut r, &mut l, 210);
    let at = pieces(&draws(&l, &r, &players));
    let new: Vec<_> = at[before.len()..].to_vec();
    // The place label, Luigi's 2 and Kirby's plate.
    assert_eq!(new.len(), 3);
    assert_eq!(
        (new[0].sprite, new[0].x, new[0].y),
        (Sprite::In(SpriteFile::VsResults, PLACE_TEXT), 10.0, 66.0)
    );
    assert_eq!(
        (new[1].sprite, new[1].x, new[1].prim),
        (
            Sprite::In(SpriteFile::PlayerDamage, 2),
            150.0,
            Some([0xFF; 3])
        )
    );
    assert_eq!(
        (new[2].sprite, new[2].x, new[2].prim),
        (Sprite::In(SpriteFile::VsResults, WINNER), 217.0, None)
    );
    run_to(&mut r, &mut l, 250);
    let all = pieces(&draws(&l, &r, &players));
    let tail = &all[all.len() - 7..];
    // The arrows at column + 17 and the stock icons 10 left of them, then
    // the KOs label and one digit each at column + 24.
    assert_eq!(
        (tail[0].sprite, tail[0].x, tail[0].y),
        (Sprite::In(SpriteFile::VsResults, ARROW_1P), 152.0, 49.0)
    );
    assert_eq!(
        (tail[1].sprite, tail[1].x),
        (
            Sprite::Stock {
                kind: FighterKind::Luigi,
                costume: 0
            },
            142.0
        )
    );
    assert_eq!(
        tail[2].sprite,
        Sprite::In(SpriteFile::VsResults, ARROW_1P + 1)
    );
    assert_eq!(
        (tail[4].sprite, tail[4].y),
        (Sprite::In(SpriteFile::VsResults, KOS_TEXT), 124.0)
    );
    assert_eq!((tail[5].sprite, tail[5].x), (digit(0), 159.0));
    assert_eq!((tail[6].sprite, tail[6].x), (digit(1), 239.0));
}

#[test]
fn the_results_text_names_the_winner_then_wins() {
    let mut r = results(Kind::StockRoyal, &[1, 0], Some(1));
    let players = [
        player(FighterKind::Luigi, true, 0),
        player(FighterKind::Kirby, false, 1),
        None,
        None,
    ];
    let mut l = Layer::new();
    run_to(&mut r, &mut l, 120);
    let p = pieces(&draws(&l, &r, &players));
    // Two tags, "KIRBY" and "WINS!", then the label.
    assert_eq!(p.len(), 2 + 5 + 5 + 1);
    // Luigi's tag: port 1, one row back in the left column; Kirby's "CP" in
    // front on the right.
    assert_eq!(
        (p[0].sprite, p[0].x, p[0].y),
        (Sprite::In(SpriteFile::PlayerTags, 0), 112.0, 75.0)
    );
    assert_eq!(p[0].prim, Some([0xED, 0x36, 0x36]));
    assert_eq!(
        (p[1].sprite, p[1].x, p[1].y),
        (Sprite::In(SpriteFile::PlayerTags, TAG_CP), 173.0, 50.0)
    );
    assert_eq!(
        (p[2].sprite, p[2].x),
        (Sprite::In(SpriteFile::Announce, 10), 40.0)
    );
    assert_eq!((p[2].prim, p[2].env), (Some([0xFF; 3]), [0xFF, 0, 0]));
    assert_eq!((p[7].x, p[7].env), (165.0, [0x60, 0x03, 0xD4]));
    assert_eq!(p[12].sprite, Sprite::In(SpriteFile::GameModes, 0));
    // Nothing of it before tic 120.
    let mut r = results(Kind::StockRoyal, &[1, 0], Some(1));
    let mut l = Layer::new();
    run_to(&mut r, &mut l, 119);
    assert!(pieces(&draws(&l, &r, &players)).is_empty());
}

#[test]
fn a_team_battle_colours_its_numbers_and_plates_only_the_winner() {
    let mut r = results(Kind::TimeTeam, &[0, 0, 1], Some(1));
    r.team = [1, 1, 0, 0];
    assert_eq!(number_color_id(&r, 0), 1);
    assert_eq!(number_color_id(&r, 2), 0);
    let mut l = Layer::new();
    run_to(&mut r, &mut l, 290);
    let p = pieces(&draws(&l, &r, &[None; 4]));
    let place: Vec<_> = p[p.len() - 3..].to_vec();
    // Port 0 shares first but is not the winner: a blue 1.
    assert_eq!(place[0].sprite, Sprite::In(SpriteFile::PlayerDamage, 1));
    assert_eq!(place[0].prim, Some([0x91, 0xC0, 0xFF]));
    assert_eq!(place[1].sprite, Sprite::In(SpriteFile::VsResults, WINNER));
    assert_eq!(place[2].sprite, Sprite::In(SpriteFile::PlayerDamage, 2));
    // "BLUE" in blue, then "WINS!" at 170.
    let text: Vec<_> = p
        .iter()
        .filter(|p| matches!(p.sprite, Sprite::In(SpriteFile::Announce, _)))
        .collect();
    assert_eq!((text[0].x, text[0].env), (60.0, [0x12, 0x00, 0xD9]));
    assert_eq!(text[4].x, 170.0);
    // The team label.
    assert!(p
        .iter()
        .any(|p| p.sprite == Sprite::In(SpriteFile::GameModes, 1)));
}

#[test]
fn four_players_with_two_in_second_show_the_next_as_fourth() {
    let r = results(Kind::TimeRoyal, &[0, 1, 1, 2], Some(0));
    assert_eq!(
        (0..4).map(|i| display_place(&r, i)).collect::<Vec<_>>(),
        [1, 2, 2, 4]
    );
    let r = results(Kind::TimeTeam, &[0, 1, 1, 2], Some(0));
    assert_eq!(display_place(&r, 3), 3);
}

#[test]
fn a_time_battles_rows_come_every_20_tics_from_210() {
    let kinds: Vec<_> = rows(Kind::TimeRoyal).iter().map(|&(t, _)| t).collect();
    assert_eq!(kinds, [180, 210, 210, 230, 250, 270, 290]);
    let mut r = results(Kind::TimeRoyal, &[0, 1], Some(0));
    r.kos = [3, 1, 0, 0];
    r.tko = [1, 2, 0, 0];
    r.points = [2, -1, 0, 0];
    let mut l = Layer::new();
    run_to(&mut r, &mut l, 270);
    let p = pieces(&draws(&l, &r, &[None; 4]));
    let pts = p
        .iter()
        .position(|p| p.sprite == Sprite::In(SpriteFile::VsResults, PTS_TEXT))
        .unwrap();
    assert_eq!((p[pts].x, p[pts].y), (26.0, 104.0));
    // 2, then -1: the dash at column + 16.
    assert_eq!((p[pts + 1].sprite, p[pts + 1].x), (digit(2), 159.0));
    assert_eq!(
        (p[pts + 2].sprite, p[pts + 2].x, p[pts + 2].y),
        (digit(DASH), 231.0, 107.0)
    );
    // The TKO row's own dash at (90, 84), in the ROM's colour.
    let dash = p
        .iter()
        .find(|p| p.sprite == digit(DASH) && p.x == 90.0)
        .unwrap();
    assert_eq!((dash.y, dash.prim), (84.0, None));
}

#[test]
fn no_contest_shows_kos_at_60_and_falls_at_80_without_the_dash() {
    let mut r = results(Kind::NoContest, &[0, 0], None);
    let mut l = Layer::new();
    run_to(&mut r, &mut l, 30);
    assert_eq!(l.tint_alpha, Some(9));
    run_to(&mut r, &mut l, 59);
    let p = pieces(&draws(&l, &r, &[None; 4]));
    assert_eq!(p.len(), 9 + 1, "NO CONTEST and the label");
    run_to(&mut r, &mut l, 80);
    let p = pieces(&draws(&l, &r, &[None; 4]));
    assert!(p
        .iter()
        .any(|p| p.sprite == Sprite::In(SpriteFile::VsResults, TKO_TEXT)));
    assert!(!p.iter().any(|p| p.sprite == digit(DASH)));
    assert!(!p
        .iter()
        .any(|p| p.sprite == Sprite::In(SpriteFile::VsResults, PLACE_TEXT)));
}
