use super::*;

fn menu() -> VsMode {
    let mut m = VsMode::new(VsRule::Time, 3, 2, false);
    // The first ten ticks ignore input.
    for _ in 0..9 {
        assert_eq!(m.tick(Input::default()), Action::None);
    }
    m
}

fn tap(b: u16) -> Input {
    Input {
        hold: b,
        tap: b,
        ..Input::default()
    }
}

#[test]
fn a_starts_from_vs_start_and_b_goes_back() {
    let mut m = menu();
    assert_eq!(m.tick(tap(N64Buttons::A)), Action::Start);
    let mut m = menu();
    assert_eq!(m.tick(tap(N64Buttons::B)), Action::Back);
}

#[test]
fn down_moves_the_cursor_and_wraps() {
    let mut m = menu();
    m.tick(tap(N64Buttons::D_DOWN));
    assert_eq!(m.cursor, Button::Rule);
    // Held, the button repeats after 12 ticks.
    for _ in 0..11 {
        m.tick(Input {
            hold: N64Buttons::D_DOWN,
            ..Input::default()
        });
    }
    assert_eq!(m.cursor, Button::Rule);
    m.tick(Input {
        hold: N64Buttons::D_DOWN,
        ..Input::default()
    });
    assert_eq!(m.cursor, Button::TimeStock);
    // Up from VS Start wraps to VS Options.
    let mut m = menu();
    m.tick(tap(N64Buttons::D_UP));
    assert_eq!(m.cursor, Button::Options);
    assert_eq!(m.tick(tap(N64Buttons::A)), Action::Options);
}

#[test]
fn the_rule_steps_through_its_four_values() {
    let mut m = menu();
    m.tick(tap(N64Buttons::D_DOWN));
    m.tick(Input::default());
    m.tick(tap(N64Buttons::D_RIGHT));
    assert_eq!(m.rule, VsRule::Stock);
    assert_eq!(m.rule.battle_rule(), Rule::Stock);
    for _ in 0..3 {
        m.tick(Input::default());
        m.tick(tap(N64Buttons::D_RIGHT));
    }
    assert_eq!(m.rule, VsRule::StockTeam);
    m.tick(Input::default());
    m.tick(tap(N64Buttons::D_LEFT));
    assert_eq!(m.rule, VsRule::TimeTeam);
    assert!(m.rule.is_time() && m.rule.is_team());
}

#[test]
fn time_wraps_through_infinite_and_stock_from_one_to_ninety_nine() {
    let mut m = menu();
    m.tick(tap(N64Buttons::D_DOWN));
    m.tick(Input::default());
    m.tick(tap(N64Buttons::D_DOWN));
    m.tick(Input::default());
    assert_eq!(m.cursor, Button::TimeStock);
    for _ in 0..3 {
        m.tick(tap(N64Buttons::D_LEFT));
        m.tick(Input::default());
    }
    // 3, 2, 1, then infinite.
    assert_eq!(m.time, TIMELIMIT_INFINITE);
    m.tick(tap(N64Buttons::D_RIGHT));
    assert_eq!(m.time, 1);

    let mut m = VsMode::new(VsRule::Stock, 3, 0, false);
    for _ in 0..9 {
        m.tick(Input::default());
    }
    m.cursor = Button::TimeStock;
    m.tick(tap(N64Buttons::D_LEFT));
    assert_eq!(m.stock, 98);
    assert_eq!(m.stocks(), 98);
}

#[test]
fn five_idle_minutes_return_to_the_title() {
    let mut m = menu();
    let mut last = Action::None;
    for _ in 0..5 * 3600 {
        last = m.tick(Input::default());
        if last != Action::None {
            break;
        }
    }
    assert_eq!(last, Action::Title);
}
