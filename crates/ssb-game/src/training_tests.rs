use super::*;
use crate::item::{container, equipment, shell, utility};

fn stick(x: i8, y: i8) -> ControllerState {
    ControllerState {
        buttons: N64Buttons(0),
        stick_x: x,
        stick_y: y,
        connected: true,
    }
}

const NONE: N64Buttons = N64Buttons(0);

/// Opens the menu and lets the stick settle at neutral.
fn open_menu() -> TrainingMenu {
    let mut m = TrainingMenu::new(0);
    assert!(m.check_enter(N64Buttons(N64Buttons::START), false));
    m.update(&stick(0, 0), NONE, 0);
    m
}

#[test]
fn the_menu_starts_as_init_vars_sets_it() {
    let m = TrainingMenu::new(0);
    assert_eq!(m.main_option, MainOption::Cp);
    assert_eq!(m.dummy, 1);
    assert_eq!(m.dummy_behavior(), Behavior::Stand);
    assert_eq!(m.view_option, VIEW_NORMAL);
    assert_eq!(TrainingMenu::new(1).dummy, 0);
    assert_eq!(TrainingMenu::new(3).dummy, 0);
}

#[test]
fn start_opens_the_menu_unless_the_fighter_ignores_menus() {
    let mut m = TrainingMenu::new(0);
    assert!(!m.check_enter(N64Buttons(N64Buttons::A), false));
    assert!(!m.check_enter(N64Buttons(N64Buttons::START), true));
    assert!(m.check_enter(N64Buttons(N64Buttons::START), false));
}

#[test]
fn a_held_stick_waits_for_neutral_then_repeats_after_30_ticks_every_5() {
    let mut m = TrainingMenu::new(0);
    m.check_enter(N64Buttons(N64Buttons::START), false);
    // Still held from before the menu: ignored until neutral.
    assert!(!m.update(&stick(80, 0), NONE, 0).cp_changed);
    m.update(&stick(0, 0), NONE, 0);
    let mut steps = vec![];
    for tick in 0..45 {
        if m.update(&stick(80, 0), NONE, 0).cp_changed {
            steps.push(tick);
        }
    }
    assert_eq!(steps, [0, 30, 35, 40]);
    assert_eq!(m.cp_option, 4);
    assert_eq!(m.dummy_behavior(), Behavior::Default);
}

#[test]
fn options_wrap_both_ways() {
    let mut m = open_menu();
    assert!(m.update(&stick(-80, 0), NONE, 0).cp_changed);
    assert_eq!(m.cp_option, CP_OPTION_COUNT - 1);
    m.update(&stick(0, 0), NONE, 0);
    m.update(&stick(80, 0), NONE, 0);
    assert_eq!(m.cp_option, 0);
}

#[test]
fn the_cursor_wraps_through_the_six_rows() {
    let mut m = open_menu();
    let f = m.update(&stick(0, 80), NONE, 0);
    assert!(f.main_changed);
    assert_eq!(m.main_option, MainOption::Exit);
    m.update(&stick(0, 0), NONE, 0);
    m.update(&stick(0, -80), NONE, 0);
    assert_eq!(m.main_option, MainOption::Cp);
}

#[test]
fn b_or_start_leaves_and_b_is_held_by_the_fighter() {
    let mut m = open_menu();
    assert_eq!(
        m.update(&stick(0, 0), N64Buttons(N64Buttons::B), 0).leave,
        Some(true)
    );
    assert_eq!(
        m.update(&stick(0, 0), N64Buttons(N64Buttons::START), 0)
            .leave,
        Some(false)
    );
    assert_eq!(
        m.update(&stick(0, 0), N64Buttons(N64Buttons::A), 0).leave,
        None
    );
}

#[test]
fn reset_and_exit_load_the_next_scene_on_a() {
    let mut m = open_menu();
    m.main_option = MainOption::Reset;
    let f = m.update(&stick(0, 0), N64Buttons(N64Buttons::A | N64Buttons::B), 0);
    assert!(f.load_scene);
    // A loads before B could leave.
    assert_eq!(f.leave, None);
    assert!(m.exit_or_reset);
    let mut m = open_menu();
    m.main_option = MainOption::Exit;
    assert!(
        m.update(&stick(0, 0), N64Buttons(N64Buttons::A), 0)
            .load_scene
    );
    assert!(!m.exit_or_reset);
}

#[test]
fn the_item_option_spawns_up_to_four_items_eight_ticks_apart() {
    let mut m = open_menu();
    m.main_option = MainOption::Item;
    let a = N64Buttons(N64Buttons::A);
    // None spawns nothing.
    assert_eq!(m.update(&stick(0, 0), a, 0).spawn_item, None);
    m.update(&stick(80, 0), NONE, 0);
    assert_eq!(m.item_option, 1);
    m.update(&stick(0, 0), NONE, 0);
    // Maxim Tomato is ITKind 4.
    assert_eq!(m.update(&stick(0, 0), a, 0).spawn_item, Some(4));
    for _ in 0..8 {
        assert_eq!(m.update(&stick(0, 0), a, 0).spawn_item, None);
    }
    assert_eq!(m.update(&stick(0, 0), a, 1).spawn_item, Some(4));
    m.item_spawn_wait = 0;
    let f = m.update(&stick(0, 0), a, ITEM_LIMIT);
    assert_eq!(f.spawn_item, None);
    assert!(f.spawn_denied);
    // The Poké Ball is the last option, ITKind 19.
    assert_eq!(item_option_kind(ITEM_OPTION_COUNT - 1), Some(19));
}

#[test]
fn the_item_limit_counts_common_items_and_ball_pokemon() {
    assert!(counts_toward_limit(ItemKind::Container(
        container::Kind::Crate
    )));
    assert!(counts_toward_limit(ItemKind::Shell(shell::Kind::Red)));
    assert!(counts_toward_limit(ItemKind::MBall));
    assert!(!counts_toward_limit(ItemKind::LinkBomb));
    assert!(!counts_toward_limit(ItemKind::PowerBlock));
    assert!(!counts_toward_limit(ItemKind::Monster(
        crate::item::monsters::Kind::Porygon
    )));
    assert_eq!(held_item_option(None), 0);
    assert_eq!(
        held_item_option(Some(ItemKind::Utility(utility::Kind::Tomato))),
        1
    );
    assert_eq!(
        held_item_option(Some(ItemKind::Equipment(equipment::Kind::Hammer))),
        10
    );
    assert_eq!(held_item_option(Some(ItemKind::MBall)), 16);
}

#[test]
fn every_common_index_round_trips_through_the_makers() {
    for i in 7..=13 {
        let k = equipment::Kind::from_index(i).unwrap();
        assert_eq!(ItemKind::Equipment(k).common_index(), Some(i));
    }
    for i in 4..=6 {
        let k = utility::Kind::from_index(i).unwrap();
        assert_eq!(ItemKind::Utility(k).common_index(), Some(i));
    }
}

/// Which of `n` ticks run at a speed.
fn runs(speed: u8, n: usize) -> Vec<bool> {
    let mut m = TrainingMenu::new(0);
    m.speed_option = speed;
    (0..n).map(|_| !m.check_lag_tic()).collect()
}

#[test]
fn slow_speeds_skip_ticks() {
    assert!(runs(0, 12).iter().all(|&r| r));
    // 2/3: two run, one skips.
    assert_eq!(runs(1, 6), [true, true, false, true, true, false]);
    // 1/2 and 1/4.
    assert_eq!(runs(2, 4), [true, false, true, false]);
    assert_eq!(
        runs(3, 8),
        [true, false, false, false, true, false, false, false]
    );
}

#[test]
fn a_speed_change_restarts_the_skip_pattern() {
    let mut m = open_menu();
    m.main_option = MainOption::Speed;
    m.speed_option = 3;
    m.check_lag_tic();
    assert_ne!(m.frameadvance_wait, 0);
    assert!(m.update(&stick(80, 0), NONE, 0).speed_changed);
    assert_eq!(m.speed_option, 0);
    assert_eq!((m.lagtic_wait, m.frameadvance_wait), (0, 0));
}

#[test]
fn the_view_hides_the_magnifier_until_180_ticks_after_normal() {
    let mut m = open_menu();
    m.main_option = MainOption::View;
    assert_eq!(
        m.update(&stick(80, 0), NONE, 0).view_changed,
        Some(VIEW_CLOSE_UP)
    );
    assert!(!m.magnify_display);
    m.update(&stick(0, 0), NONE, 0);
    assert_eq!(
        m.update(&stick(80, 0), NONE, 0).view_changed,
        Some(VIEW_NORMAL)
    );
    for _ in 0..179 {
        m.tick_processes();
    }
    assert!(!m.magnify_display);
    m.tick_processes();
    assert!(m.magnify_display);
}
