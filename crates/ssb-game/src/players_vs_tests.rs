use super::*;

const ONE_CONTROLLER: [bool; PLAYERS] = [true, false, false, false];

fn pad(hold: u16, taps: u16) -> Pad {
    Pad {
        state: ControllerState {
            buttons: N64Buttons(hold),
            stick_x: 0,
            stick_y: 0,
            connected: true,
        },
        taps: N64Buttons(taps),
    }
}

/// Port 0's input and neutral, unread input on the other three.
fn port0(p: Pad) -> [Pad; PLAYERS] {
    [p, Pad::default(), Pad::default(), Pad::default()]
}

/// Every random draw reads 0: Mario, and stage 0.
fn idle(s: &mut PlayersVs) -> Option<Outcome> {
    s.tick(&port0(Pad::default()), &mut || 0)
}

fn tap(s: &mut PlayersVs, button: u16) -> Option<Outcome> {
    s.tick(&port0(pad(button, button)), &mut || 0)
}

fn first_visit(state: BattleState) -> PlayersVs {
    PlayersVs::new(state, SceneContext::default(), ONE_CONTROLLER)
}

/// Puts port 0's cursor at `(x, y)` and lets one tick update its status.
fn cursor_at(s: &mut PlayersVs, x: f32, y: f32) {
    s.slots[0].cursor = Some((x, y));
    idle(s);
}

/// Puts the cursor where its held puck's centre lands on `kind`'s portrait.
fn hover_portrait(s: &mut PlayersVs, kind: FighterKind) {
    let (px, py) = portrait_center(kind);
    // The held puck sits at the cursor + (11, -14).
    cursor_at(s, px - 11.0, py + 14.0);
}

fn place(s: &mut PlayersVs, kind: FighterKind) {
    hover_portrait(s, kind);
    tap(s, N64Buttons::A);
}

/// Port 0's cursor presses slot `sel`'s HMN/CP/NA button.
fn toggle_kind(s: &mut PlayersVs, sel: usize) {
    cursor_at(s, sel as f32 * 69.0 + 52.0, 130.0);
    tap(s, N64Buttons::A);
}

/// Slot 0 on Fox and slot 2 a CPU (Mario, from the zero draw).
fn fox_vs_cpu_mario(state: BattleState) -> PlayersVs {
    let mut s = first_visit(state);
    place(&mut s, FighterKind::Fox);
    toggle_kind(&mut s, 2);
    s
}

#[test]
fn a_first_visit_with_one_controller_gives_port_one_a_human_holding_its_puck() {
    let s = first_visit(BattleState::default());
    let man = &s.slots[0];
    assert_eq!(man.pkind, PlayerKind::Man);
    assert_eq!((man.holder, man.held), (Some(0), Some(0)));
    assert_eq!(man.cursor, Some((40.0, 170.0)));
    assert_eq!(man.puck, (51.0, 161.0));
    for p in 1..PLAYERS {
        let slot = &s.slots[p];
        assert_eq!(slot.pkind, PlayerKind::Not);
        assert_eq!((slot.holder, slot.held, slot.cursor), (None, None, None));
        assert_eq!(slot.fkind, None);
    }
    // `mnPlayersVSResetPlayer`'s teams; level and handicap from the state.
    assert_eq!(s.slots.clone().map(|slot| slot.team), [0, 0, 1, 1]);
    assert_eq!((s.slots[2].cpu_level, s.slots[2].handicap), (3, 9));
    assert_eq!((s.time_value, s.stock_value, s.rule), (3, 2, Rule::Time));
    assert!(!s.is_team_battle);
    assert!(!s.is_ready());
}

#[test]
fn a_saved_human_on_an_unplugged_port_comes_back_as_na() {
    let mut state = BattleState {
        is_reset_players: false,
        ..BattleState::default()
    };
    state.players[0].pkind = PlayerKind::Man;
    state.players[1].pkind = PlayerKind::Man;
    state.players[1].fkind = Some(FighterKind::Kirby);
    state.players[2].pkind = PlayerKind::Com;
    state.players[2].fkind = Some(FighterKind::Samus);
    state.players[2].costume = 2;
    let s = first_visit(state);
    assert_eq!(s.slots[0].pkind, PlayerKind::Man);
    assert_eq!(s.slots[0].held, Some(0));
    assert_eq!(
        (s.slots[1].pkind, s.slots[1].fkind),
        (PlayerKind::Not, None)
    );
    let com = &s.slots[2];
    assert_eq!(com.pkind, PlayerKind::Com);
    assert!(com.is_selected && com.is_fighter_selected);
    assert_eq!(com.costume, 2);
    assert_eq!(com.puck, portrait_center(FighterKind::Samus));
}

#[test]
fn the_kind_button_cycles_an_unplugged_slot_between_cpu_and_na() {
    let mut s = first_visit(BattleState::default());
    toggle_kind(&mut s, 2);
    let com = &s.slots[2];
    assert_eq!(com.pkind, PlayerKind::Com);
    // The zero draw is Mario, placed on his portrait.
    assert_eq!(com.fkind, Some(FighterKind::Mario));
    assert_eq!(com.puck, portrait_center(FighterKind::Mario));
    assert!(com.is_selected && com.is_fighter_selected);
    assert_eq!(com.holder, None);
    // Port 0 still holds its own puck.
    assert_eq!(s.slots[0].held, Some(0));
    tap(&mut s, N64Buttons::A);
    assert_eq!(
        (s.slots[2].pkind, s.slots[2].fkind),
        (PlayerKind::Not, None)
    );
    // No controller: NA goes back to CPU, not HMN.
    tap(&mut s, N64Buttons::A);
    assert_eq!(s.slots[2].pkind, PlayerKind::Com);
}

#[test]
fn a_places_the_players_puck_on_the_portrait_under_it() {
    let mut s = first_visit(BattleState::default());
    hover_portrait(&mut s, FighterKind::Fox);
    assert_eq!(s.slots[0].fkind, Some(FighterKind::Fox));
    assert_eq!(s.slots[0].cursor_status, CursorStatus::Grab);
    assert!(!s.slots[0].is_fighter_selected);
    tap(&mut s, N64Buttons::A);
    let man = &s.slots[0];
    assert!(man.is_selected && man.is_fighter_selected);
    assert_eq!((man.holder, man.held), (None, None));
    assert_eq!(man.cursor_status, CursorStatus::Hover);
    assert_eq!(man.costume, 0);
    // One placed player is not enough.
    assert_eq!(s.ready_player_count(), 1);
    assert!(!s.is_ready());
}

#[test]
fn the_cursor_grabs_the_cpu_puck_and_places_it_on_another_fighter() {
    let mut s = fox_vs_cpu_mario(BattleState::default());
    // Wait out the grab delay, then hover the CPU's puck.
    for _ in 0..30 {
        idle(&mut s);
    }
    let (px, py) = s.slots[2].puck;
    cursor_at(&mut s, px - 20.0, py + 5.0);
    assert_eq!(s.slots[0].cursor_status, CursorStatus::Hover);
    tap(&mut s, N64Buttons::A);
    assert_eq!(s.slots[0].held, Some(2));
    assert_eq!(s.slots[2].holder, Some(0));
    assert!(!s.slots[2].is_fighter_selected);
    // A cursor holding a puck blocks the ready check.
    assert!(!s.is_ready());
    for _ in 0..30 {
        idle(&mut s);
    }
    hover_portrait(&mut s, FighterKind::Pikachu);
    assert_eq!(s.slots[2].fkind, Some(FighterKind::Pikachu));
    tap(&mut s, N64Buttons::A);
    let com = &s.slots[2];
    assert!(com.is_selected && com.is_fighter_selected);
    assert_eq!(com.holder, None);
    assert_eq!(s.slots[0].held, None);
    assert!(s.is_ready());
}

#[test]
fn the_arrows_step_a_cpu_level_and_with_handicap_on_the_humans_handicap() {
    let mut s = fox_vs_cpu_mario(BattleState {
        handicap: Handicap::On,
        ..BattleState::default()
    });
    // Slot 2's arrows sit at 138 + 68..90 (right) and 138 + 21..43 (left).
    cursor_at(&mut s, 190.0, 200.0);
    tap(&mut s, N64Buttons::A);
    assert_eq!(s.slots[2].cpu_level, 4);
    cursor_at(&mut s, 145.0, 200.0);
    for _ in 0..5 {
        tap(&mut s, N64Buttons::A);
    }
    assert_eq!(s.slots[2].cpu_level, 1);
    // The handicap is already at its top, 9.
    cursor_at(&mut s, 50.0, 200.0);
    tap(&mut s, N64Buttons::A);
    assert_eq!(s.slots[0].handicap, 9);
    cursor_at(&mut s, 5.0, 200.0);
    tap(&mut s, N64Buttons::A);
    assert_eq!(s.slots[0].handicap, 8);
}

#[test]
fn with_handicap_off_a_humans_arrows_do_nothing() {
    let mut s = fox_vs_cpu_mario(BattleState::default());
    cursor_at(&mut s, 5.0, 200.0);
    tap(&mut s, N64Buttons::A);
    assert_eq!(s.slots[0].handicap, 9);
}

#[test]
fn free_for_all_gives_a_repeated_fighter_the_next_free_costume() {
    let mut s = first_visit(BattleState::default());
    toggle_kind(&mut s, 2); // CPU Mario, costume 0.
    place(&mut s, FighterKind::Mario);
    assert_eq!(s.slots[0].costume, 1);
    // C-Down is costume 2; C-Up is 0, the CPU's: refused.
    tap(&mut s, N64Buttons::C_DOWN);
    assert_eq!(s.slots[0].costume, 2);
    tap(&mut s, N64Buttons::C_UP);
    assert_eq!(s.slots[0].costume, 2);
}

#[test]
fn team_battle_dresses_fighters_in_team_costumes_and_the_team_button_cycles() {
    let mut s = fox_vs_cpu_mario(BattleState::default());
    cursor_at(&mut s, 30.0, 15.0);
    tap(&mut s, N64Buttons::A);
    assert!(s.is_team_battle);
    // Slot 0 is red, slot 2 blue (`mnPlayersVSResetPlayer`).
    assert_eq!(
        s.slots[0].costume,
        costume_team_id(FighterKind::Fox, TEAM_RED)
    );
    assert_eq!(
        s.slots[2].costume,
        costume_team_id(FighterKind::Mario, TEAM_BLUE)
    );
    assert_eq!((s.slots[0].costume, s.slots[2].costume), (1, 3));
    assert!(s.is_ready());
    // Slot 0's team button: red to blue leaves one team, which is not ready.
    cursor_at(&mut s, 20.0, 130.0);
    tap(&mut s, N64Buttons::A);
    assert_eq!(s.slots[0].team, TEAM_BLUE);
    assert_eq!(
        s.slots[0].costume,
        costume_team_id(FighterKind::Fox, TEAM_BLUE)
    );
    assert!(s.is_single_team());
    assert!(!s.is_ready());
    tap(&mut s, N64Buttons::A);
    assert_eq!(s.slots[0].team, TEAM_GREEN);
    assert!(s.is_ready());
    tap(&mut s, N64Buttons::A);
    assert_eq!(s.slots[0].team, TEAM_RED);
    // Back to free-for-all: royal costumes again.
    cursor_at(&mut s, 30.0, 15.0);
    tap(&mut s, N64Buttons::A);
    assert!(!s.is_team_battle);
    assert_eq!((s.slots[0].costume, s.slots[2].costume), (0, 0));
}

#[test]
fn ready_needs_two_placed_players() {
    let mut s = first_visit(BattleState::default());
    toggle_kind(&mut s, 2);
    // The CPU is placed, port 0 still holds its puck.
    assert_eq!(s.ready_player_count(), 1);
    assert!(!s.is_ready());
    place(&mut s, FighterKind::Fox);
    assert_eq!(s.ready_player_count(), 2);
    assert!(s.is_ready());
}

#[test]
fn start_after_a_second_proceeds_thirty_ticks_later_with_the_battle_settings() {
    let mut s = fox_vs_cpu_mario(BattleState::default());
    while s.total_tics() < 60 {
        idle(&mut s);
    }
    // Tick 61 is the first to read START.
    assert_eq!(tap(&mut s, N64Buttons::START), None);
    assert!(s.is_start());
    for _ in 0..29 {
        assert_eq!(idle(&mut s), None);
    }
    let Some(Outcome::Maps(state)) = idle(&mut s) else {
        panic!("expected the stage select");
    };
    assert!(!state.is_reset_players);
    assert_eq!(
        (state.time_limit, state.stocks, state.rule),
        (3, 2, Rule::Time)
    );
    assert!(!state.is_team_battle);
    assert_eq!((state.pl_count, state.cp_count), (1, 1));
    let man = state.players[0];
    assert_eq!(man.pkind, PlayerKind::Man);
    assert_eq!(man.fkind, Some(FighterKind::Fox));
    assert_eq!((man.costume, man.shade), (0, 0));
    assert_eq!((man.player, man.color, man.tag), (0, 0, 0));
    assert_eq!(man.handicap, 9);
    assert!(man.is_single_stockicon);
    let com = state.players[2];
    assert_eq!(com.pkind, PlayerKind::Com);
    assert_eq!(com.fkind, Some(FighterKind::Mario));
    assert_eq!((com.player, com.color, com.tag, com.level), (2, 4, 4, 3));
    for p in [1, 3] {
        assert_eq!(state.players[p].pkind, PlayerKind::Not);
        assert_eq!(state.players[p].fkind, None);
    }
}

#[test]
fn start_is_refused_before_tick_sixty_one_and_when_not_ready() {
    let mut s = fox_vs_cpu_mario(BattleState::default());
    tap(&mut s, N64Buttons::START);
    assert!(!s.is_start());
    let mut s = first_visit(BattleState::default());
    while s.total_tics() < 60 {
        idle(&mut s);
    }
    tap(&mut s, N64Buttons::START);
    assert!(!s.is_start());
}

#[test]
fn with_the_stage_select_off_start_draws_a_stage_other_than_the_last() {
    let state = BattleState {
        is_stage_select: false,
        ..BattleState::default()
    };
    let mut s = PlayersVs::new(state, SceneContext::default(), ONE_CONTROLLER);
    place(&mut s, FighterKind::Fox);
    toggle_kind(&mut s, 2);
    while s.total_tics() < 60 {
        idle(&mut s);
    }
    tap(&mut s, N64Buttons::START);
    for _ in 0..29 {
        idle(&mut s);
    }
    // 0 is the last stage (Castle) and is drawn again; 200 * 8 / 256 = 6.
    let mut draws = [0u8, 200].into_iter();
    let outcome = s.tick(&port0(Pad::default()), &mut || draws.next().unwrap());
    assert!(matches!(
        outcome,
        Some(Outcome::Battle { gkind: 6, state }) if state.pl_count == 1
    ));
}

#[test]
fn b_held_forty_ticks_goes_back_to_vs_mode() {
    let mut s = first_visit(BattleState::default());
    let hold_b = pad(N64Buttons::B, 0);
    assert_eq!(tap(&mut s, N64Buttons::B), None);
    for _ in 0..38 {
        assert_eq!(s.tick(&port0(hold_b), &mut || 0), None);
    }
    assert!(matches!(
        s.tick(&port0(hold_b), &mut || 0),
        Some(Outcome::VsMode(_))
    ));
}

#[test]
fn letting_go_of_b_early_cancels_the_back() {
    let mut s = first_visit(BattleState::default());
    let hold_b = pad(N64Buttons::B, 0);
    tap(&mut s, N64Buttons::B);
    for _ in 0..20 {
        s.tick(&port0(hold_b), &mut || 0);
    }
    idle(&mut s);
    for _ in 0..40 {
        assert_eq!(s.tick(&port0(hold_b), &mut || 0), None);
    }
}

#[test]
fn a_on_the_back_button_goes_back_at_once() {
    let mut s = first_visit(BattleState::default());
    cursor_at(&mut s, 230.0, 15.0);
    assert!(matches!(
        tap(&mut s, N64Buttons::A),
        Some(Outcome::VsMode(_))
    ));
}

#[test]
fn b_on_a_placed_puck_recalls_it_instead_of_going_back() {
    let mut s = first_visit(BattleState::default());
    place(&mut s, FighterKind::Fox);
    assert_eq!(tap(&mut s, N64Buttons::B), None);
    assert!(s.slots[0].is_recalling);
    assert!(!s.slots[0].is_fighter_selected);
    for _ in 0..11 {
        idle(&mut s);
    }
    assert_eq!(s.slots[0].held, Some(0));
    assert_eq!(s.slots[0].cursor_status, CursorStatus::Grab);
}

#[test]
fn the_time_and_stock_arrows_step_and_wrap() {
    assert_eq!(next_time_value(3), 5);
    assert_eq!(next_time_value(60), TIMELIMIT_INFINITE);
    assert_eq!(next_time_value(TIMELIMIT_INFINITE), 2);
    assert_eq!(prev_time_value(2), TIMELIMIT_INFINITE);
    assert_eq!(prev_time_value(TIMELIMIT_INFINITE), 60);
    let mut s = first_visit(BattleState::default());
    cursor_at(&mut s, 200.0, 15.0);
    tap(&mut s, N64Buttons::A);
    assert_eq!(s.time_value, 5);
    let mut s = first_visit(BattleState {
        rule: Rule::Stock,
        stocks: 0,
        ..BattleState::default()
    });
    cursor_at(&mut s, 130.0, 15.0);
    tap(&mut s, N64Buttons::A);
    assert_eq!(s.stock_value, 98);
}

#[test]
fn unplugging_a_port_makes_its_slot_na_and_replugging_brings_the_cursor_back() {
    let mut s = first_visit(BattleState::default());
    s.set_connected([false; PLAYERS]);
    idle(&mut s);
    assert_eq!(s.slots[0].pkind, PlayerKind::Not);
    assert_eq!(s.slots[0].cursor, None);
    assert_eq!(s.slots[0].held, None);
    s.set_connected(ONE_CONTROLLER);
    idle(&mut s);
    let man = &s.slots[0];
    assert_eq!(man.pkind, PlayerKind::Man);
    assert_eq!(man.cursor, Some((40.0, 170.0)));
    assert_eq!((man.holder, man.held), (Some(0), Some(0)));
}

#[test]
fn five_idle_minutes_return_to_the_title() {
    let mut s = first_visit(BattleState::default());
    let mut outcome = None;
    for _ in 0..RETURN_TICS {
        outcome = idle(&mut s);
    }
    assert!(matches!(outcome, Some(Outcome::Title(_))));
}
