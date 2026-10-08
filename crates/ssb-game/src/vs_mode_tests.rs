use super::*;
use crate::fighter::FighterKind;

fn state() -> BattleState {
    BattleState::default()
}

/// A menu past its first nine ticks, which ignore input.
fn menu_with(st: &BattleState) -> VsMode {
    let mut m = VsMode::new(Scene::ModeSelect, st);
    let mut s = *st;
    for _ in 0..9 {
        assert_eq!(m.tick(&Pad::default(), &mut s), None);
    }
    m
}

fn menu() -> VsMode {
    menu_with(&state())
}

fn tap(b: u16) -> Pad {
    Pad {
        hold: b,
        tap: b,
        ..Pad::default()
    }
}

fn hold(b: u16) -> Pad {
    Pad {
        hold: b,
        ..Pad::default()
    }
}

fn run(m: &mut VsMode, pad: Pad) -> Option<Scene> {
    m.tick(&pad, &mut state())
}

#[test]
fn vs_start_loads_the_character_select_one_tick_later() {
    let mut m = menu();
    let mut st = state();
    m.time = 7;
    assert_eq!(m.tick(&tap(N64Buttons::A), &mut st), None);
    assert_eq!(m.button_status[0], TabStatus::Selected);
    // `mnVSModeSaveSettings` ran at once.
    assert_eq!(st.time_limit, 7);
    // `sMNVSModeExitInterrupt`: the next tick loads.
    assert_eq!(m.tick(&Pad::default(), &mut st), Some(Scene::PlayersVs));
}

#[test]
fn b_goes_back_at_once_with_the_settings_saved() {
    let mut m = menu();
    let mut st = state();
    m.rule = VsRule::StockTeam;
    m.stock = 4;
    assert_eq!(
        m.tick(&tap(N64Buttons::B), &mut st),
        Some(Scene::ModeSelect)
    );
    assert_eq!(
        (st.rule, st.is_team_battle, st.stocks),
        (Rule::Stock, true, 4)
    );
}

#[test]
fn down_moves_the_cursor_and_wraps() {
    let mut m = menu();
    run(&mut m, tap(N64Buttons::D_DOWN));
    assert_eq!(m.cursor, Button::Rule);
    assert_eq!(
        m.button_status,
        [
            TabStatus::Not,
            TabStatus::Highlight,
            TabStatus::Not,
            TabStatus::Not
        ]
    );
    // Held, the button repeats after 12 ticks.
    for _ in 0..11 {
        run(&mut m, hold(N64Buttons::D_DOWN));
    }
    assert_eq!(m.cursor, Button::Rule);
    run(&mut m, hold(N64Buttons::D_DOWN));
    assert_eq!(m.cursor, Button::TimeStock);
    // Up from VS Start wraps to VS Options.
    let mut m = menu();
    run(&mut m, tap(N64Buttons::D_UP));
    assert_eq!(m.cursor, Button::Options);
    assert_eq!(run(&mut m, tap(N64Buttons::A)), None);
    assert_eq!(run(&mut m, Pad::default()), Some(Scene::VsOptions));
}

#[test]
fn coming_back_from_vs_options_starts_on_it() {
    let m = VsMode::new(Scene::VsOptions, &state());
    assert_eq!(m.cursor, Button::Options);
    assert_eq!(m.button_status[3], TabStatus::Highlight);
}

#[test]
fn mode_select_bgm_plays_only_back_from_the_character_select() {
    use crate::sound::{id, testing::*};
    let r = Recorder::install();
    VsMode::new(Scene::VsOptions, &state());
    assert_eq!(r.take(), []);
    VsMode::new(Scene::PlayersVs, &state());
    assert_eq!(r.take(), [Call::PlayBgm(0, id::nSYAudioBGMModeSelect)]);
    crate::sound::uninstall();
}

#[test]
fn the_rule_steps_through_its_four_values() {
    let mut m = menu();
    run(&mut m, tap(N64Buttons::D_DOWN));
    run(&mut m, Pad::default());
    run(&mut m, tap(N64Buttons::D_RIGHT));
    assert_eq!(m.rule, VsRule::Stock);
    assert_eq!(m.rule.battle_rule(), Rule::Stock);
    for _ in 0..3 {
        run(&mut m, Pad::default());
        run(&mut m, tap(N64Buttons::D_RIGHT));
    }
    assert_eq!(m.rule, VsRule::StockTeam);
    run(&mut m, Pad::default());
    run(&mut m, tap(N64Buttons::D_LEFT));
    assert_eq!(m.rule, VsRule::TimeTeam);
    assert!(m.rule.is_time() && m.rule.is_team());
}

#[test]
fn time_wraps_through_infinite_and_stock_from_one_to_ninety_nine() {
    let mut m = menu();
    run(&mut m, tap(N64Buttons::D_DOWN));
    run(&mut m, Pad::default());
    run(&mut m, tap(N64Buttons::D_DOWN));
    run(&mut m, Pad::default());
    assert_eq!(m.cursor, Button::TimeStock);
    for _ in 0..3 {
        run(&mut m, tap(N64Buttons::D_LEFT));
        run(&mut m, Pad::default());
    }
    // 3, 2, 1, then infinite.
    assert_eq!(m.time, TIMELIMIT_INFINITE);
    run(&mut m, tap(N64Buttons::D_RIGHT));
    assert_eq!(m.time, 1);

    let st = BattleState {
        rule: Rule::Stock,
        stocks: 0,
        ..state()
    };
    let mut m = menu_with(&st);
    m.cursor = Button::TimeStock;
    run(&mut m, tap(N64Buttons::D_LEFT));
    assert_eq!(m.stock, 98);
    assert_eq!(m.stocks(), 98);
    assert_eq!(m.time_stock_value(), 99);
}

#[test]
fn a_held_value_doubles_its_first_wait() {
    let mut m = menu();
    m.cursor = Button::TimeStock;
    // The button's 12, doubled for the new direction: 24 ticks to the next
    // step, then 12 at a time.
    run(&mut m, hold(N64Buttons::D_RIGHT));
    assert_eq!(m.time, 4);
    for _ in 0..23 {
        run(&mut m, hold(N64Buttons::D_RIGHT));
    }
    assert_eq!(m.time, 4);
    run(&mut m, hold(N64Buttons::D_RIGHT));
    assert_eq!(m.time, 5);
    for _ in 0..12 {
        run(&mut m, hold(N64Buttons::D_RIGHT));
    }
    assert_eq!(m.time, 6);
}

#[test]
fn five_idle_minutes_return_to_the_title() {
    let mut m = menu();
    let mut st = state();
    m.time = 9;
    let mut last = None;
    for _ in 0..5 * 3600 {
        last = m.tick(&Pad::default(), &mut st);
        if last.is_some() {
            break;
        }
    }
    assert_eq!(last, Some(Scene::Title));
    assert_eq!(st.time_limit, 9);
}

#[test]
fn a_stick_inside_the_dead_zone_is_idle() {
    // `scSubsysControllerCheckNoInputAll`: a stick within +-20 does not
    // hold off the return to the title.
    let mut m = menu();
    let drift = Pad {
        stick_x: 15,
        stick_y: -20,
        ..Pad::default()
    };
    let mut last = None;
    for _ in 0..5 * 3600 {
        last = run(&mut m, drift);
        if last.is_some() {
            break;
        }
    }
    assert_eq!(last, Some(Scene::Title));
    // Past it, it does.
    let mut m = menu();
    let push = Pad {
        stick_x: 21,
        ..Pad::default()
    };
    for _ in 0..5 * 3600 {
        assert_eq!(run(&mut m, push), None);
    }
}

#[test]
fn the_rule_arrows_blink_every_thirty_ticks_and_follow_the_rule() {
    let mut m = menu();
    assert!(m.time_stock_arrows_hidden);
    run(&mut m, tap(N64Buttons::D_DOWN));
    // Time: only the right arrow.
    assert_eq!(m.rule_arrows(), [Some(Arrow::Right), None]);
    assert!(!m.rule_arrows_hidden);
    // Shown for the move's tick and 28 more, then hidden for 30.
    for _ in 0..28 {
        run(&mut m, Pad::default());
        assert!(!m.rule_arrows_hidden);
    }
    run(&mut m, Pad::default());
    assert!(m.rule_arrows_hidden);
    for _ in 0..29 {
        run(&mut m, Pad::default());
        assert!(m.rule_arrows_hidden);
    }
    run(&mut m, Pad::default());
    assert!(!m.rule_arrows_hidden);
    // Stock: the left arrow joins after the right.
    run(&mut m, tap(N64Buttons::D_RIGHT));
    assert_eq!(m.rule_arrows(), [Some(Arrow::Right), Some(Arrow::Left)]);
    run(&mut m, Pad::default());
    run(&mut m, tap(N64Buttons::D_RIGHT));
    run(&mut m, Pad::default());
    run(&mut m, tap(N64Buttons::D_RIGHT));
    assert_eq!(m.rule_arrows(), [Some(Arrow::Left), None]);
    // Leaving Rule hides them.
    run(&mut m, Pad::default());
    run(&mut m, tap(N64Buttons::D_DOWN));
    assert!(m.rule_arrows_hidden);
    assert!(!m.time_stock_arrows_hidden);
}

#[test]
fn a_team_rule_gives_team_costumes() {
    let mut st = state();
    st.rule = Rule::Stock;
    st.players[0].fkind = Some(FighterKind::Mario);
    st.players[1].fkind = Some(FighterKind::Mario);
    st.players[1].team = 0;
    let mut m = menu_with(&st);
    m.cursor = Button::Rule;
    m.tick(&tap(N64Buttons::D_RIGHT), &mut st);
    assert_eq!(m.rule, VsRule::TimeTeam);
    assert_eq!(
        st.players[0].costume,
        costume_team_id(FighterKind::Mario, 0)
    );
    // Two Marios on one team, player by player: the first sees the
    // second's old shade 0 and takes 1; the second then takes 0.
    assert_eq!((st.players[0].shade, st.players[1].shade), (1, 0));
    // Back to Stock: royal costumes, each its own, and no shades.
    m.tick(&Pad::default(), &mut st);
    m.tick(&tap(N64Buttons::D_LEFT), &mut st);
    assert_eq!(m.rule, VsRule::Stock);
    let royal: Vec<_> = (0..4)
        .map(|j| costume_common_id(FighterKind::Mario, j))
        .collect();
    assert!(royal.contains(&st.players[0].costume));
    assert!(royal.contains(&st.players[1].costume));
    assert_ne!(st.players[0].costume, st.players[1].costume);
    assert_eq!((st.players[0].shade, st.players[1].shade), (0, 0));
}

fn pieces(m: &VsMode) -> Vec<Piece> {
    let mut out = Vec::new();
    m.visit(&mut |d| match d {
        Draw::Sprite(p) => out.push(p),
        Draw::Tiled { piece, .. } => out.push(piece),
        _ => {}
    });
    out
}

#[test]
fn the_values_draw_right_aligned_and_white() {
    let mut m = menu();
    m.time = 12;
    let p = pieces(&m);
    let digits: Vec<_> = p
        .iter()
        .filter(|p| common::DIGITS.contains(&p.offset))
        .collect();
    assert_eq!(digits.len(), 2);
    assert_eq!(
        (digits[0].offset, digits[0].x, digits[0].y),
        (common::DIGITS[2], 179.0, 116.0)
    );
    assert_eq!((digits[1].offset, digits[1].x), (common::DIGITS[1], 168.0));
    assert_eq!(digits[0].prim, Some([0xFF; 3]));
    m.time = TIMELIMIT_INFINITE;
    assert!(pieces(&m)
        .iter()
        .any(|p| p.offset == common::INFINITY && p.x == 162.0));
    // A rule change moves the Time/Stock button after VS Options.
    let mut m = menu();
    let order = |m: &VsMode| {
        pieces(m)
            .iter()
            .filter(|p| p.file == FILE_VS_MODE)
            .map(|p| p.offset)
            .collect::<Vec<_>>()
    };
    let before = order(&m);
    assert!(
        before.iter().position(|&o| o == sprite::TIME_PERIOD_TEXT)
            < before.iter().position(|&o| o == sprite::VS_OPTIONS_TEXT)
    );
    m.cursor = Button::Rule;
    run(&mut m, tap(N64Buttons::D_RIGHT));
    let after = order(&m);
    assert!(
        after.iter().position(|&o| o == sprite::STOCK_PERIOD_TEXT)
            > after.iter().position(|&o| o == sprite::VS_OPTIONS_TEXT)
    );
}
