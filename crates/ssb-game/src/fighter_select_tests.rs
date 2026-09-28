use super::*;

fn pad(buttons: u16, stick_x: i8, stick_y: i8) -> ControllerState {
    ControllerState {
        buttons: N64Buttons(buttons),
        stick_x,
        stick_y,
        connected: true,
    }
}

fn idle(s: &mut FighterSelect) -> Option<Outcome> {
    s.tick(pad(0, 0, 0), N64Buttons(0))
}

fn tap(s: &mut FighterSelect, button: u16) -> Option<Outcome> {
    s.tick(pad(button, 0, 0), N64Buttons(button))
}

/// A first visit: no saved fighters, the CPU drawn from `byte`.
fn first_visit(byte: u8) -> FighterSelect {
    FighterSelect::new(SceneData::default(), 0, || byte)
}

/// Puts the cursor where the held puck's centre lands on `kind`'s
/// portrait, then lets one tick update the cursor and the puck.
fn hover_portrait(s: &mut FighterSelect, kind: FighterKind) {
    let (px, py) = portrait_center(kind);
    // The held puck sits at the cursor + (11, -14).
    s.slots[MAN].cursor = (px - 11.0, py + 14.0);
    idle(s);
}

fn place(s: &mut FighterSelect, kind: FighterKind) {
    hover_portrait(s, kind);
    tap(s, N64Buttons::A);
}

#[test]
fn a_first_visit_holds_the_players_puck_and_draws_an_unlocked_cpu() {
    // 90 * 12 / 256 = 4, Luigi: locked with an empty fighter mask, so the
    // draw repeats. 0 is Mario.
    let mut draws = [90u8, 0].into_iter();
    let s = FighterSelect::new(SceneData::default(), 0, || draws.next().unwrap());
    assert_eq!(s.slots[COM].kind, Some(FighterKind::Mario));
    assert!(s.slots[COM].is_fighter_selected);
    assert_eq!(s.slots[MAN].held, Some(MAN));
    assert_eq!(s.slots[MAN].holder, Some(MAN));
    assert_eq!(s.slots[MAN].cursor, (70.0, 170.0));
    assert_eq!(s.slots[COM].puck, portrait_center(FighterKind::Mario));
    assert!(!s.is_ready());
}

#[test]
fn the_held_puck_follows_the_cursor_and_names_the_portrait_under_it() {
    let mut s = first_visit(0);
    idle(&mut s);
    assert_eq!(s.slots[MAN].puck, (81.0, 156.0));
    assert_eq!(s.slots[MAN].kind, None);
    assert_eq!(s.slots[MAN].cursor_status, CursorStatus::Pointer);
    hover_portrait(&mut s, FighterKind::Fox);
    assert_eq!(s.slots[MAN].kind, Some(FighterKind::Fox));
    assert_eq!(s.slots[MAN].cursor_status, CursorStatus::Grab);
    // A locked portrait names no fighter.
    hover_portrait(&mut s, FighterKind::Ness);
    assert_eq!(s.slots[MAN].kind, None);
}

#[test]
fn the_stick_moves_the_cursor_a_twentieth_of_its_deflection() {
    let mut s = first_visit(0);
    s.tick(pad(0, 80, 80), N64Buttons(0));
    assert_eq!(s.slots[MAN].cursor, (74.0, 166.0));
    // Inside the ±8 dead zone nothing moves.
    s.tick(pad(0, 8, -8), N64Buttons(0));
    assert_eq!(s.slots[MAN].cursor, (74.0, 166.0));
}

#[test]
fn a_places_the_puck_and_start_after_sixty_ticks_proceeds_thirty_later() {
    let mut s = first_visit(65); // 65 * 12 / 256 = 3, Samus.
    assert_eq!(s.slots[COM].kind, Some(FighterKind::Samus));
    place(&mut s, FighterKind::Fox);
    assert!(s.slots[MAN].is_selected);
    assert_eq!(s.slots[MAN].held, None);
    assert!(s.is_ready());
    // START before tick 61 is refused.
    assert_eq!(tap(&mut s, N64Buttons::START), None);
    while s.total_tics < 60 {
        idle(&mut s);
    }
    assert_eq!(tap(&mut s, N64Buttons::START), None);
    for _ in 0..29 {
        assert_eq!(idle(&mut s), None);
    }
    assert_eq!(
        idle(&mut s),
        Some(Outcome::Proceed(SceneData {
            man_kind: Some(FighterKind::Fox),
            man_costume: 0,
            com_kind: Some(FighterKind::Samus),
            com_costume: 0,
        }))
    );
}

#[test]
fn b_recalls_a_placed_puck_to_the_cursor_and_b_again_goes_back() {
    let mut s = first_visit(0);
    place(&mut s, FighterKind::Fox);
    for _ in 0..10 {
        idle(&mut s);
    }
    assert_eq!(tap(&mut s, N64Buttons::B), None);
    assert!(s.slots[MAN].is_recalling);
    assert!(!s.is_ready());
    // The puck flies for ten ticks; the eleventh hands it to the cursor.
    for _ in 0..11 {
        idle(&mut s);
    }
    assert_eq!(s.slots[MAN].held, Some(MAN));
    assert_eq!(s.slots[MAN].cursor_status, CursorStatus::Grab);
    // The recall ends on its thirtieth tick; B then leaves.
    for _ in 0..19 {
        idle(&mut s);
    }
    assert!(!s.slots[MAN].is_recalling);
    assert!(matches!(tap(&mut s, N64Buttons::B), Some(Outcome::Back(_))));
}

#[test]
fn b_before_tick_ten_is_ignored() {
    let mut s = first_visit(0);
    assert_eq!(tap(&mut s, N64Buttons::B), None);
    for _ in 0..8 {
        idle(&mut s);
    }
    assert!(matches!(tap(&mut s, N64Buttons::B), Some(Outcome::Back(_))));
}

#[test]
fn the_cursor_carries_the_cpu_puck_and_it_snaps_back_down_off_the_grid() {
    let mut s = first_visit(0); // CPU Mario.
    place(&mut s, FighterKind::Fox);
    // Wait out the grab delay, then pick up the CPU's puck.
    for _ in 0..30 {
        idle(&mut s);
    }
    let (px, py) = s.slots[COM].puck;
    s.slots[MAN].cursor = (px - 20.0, py + 5.0);
    idle(&mut s);
    assert_eq!(s.slots[MAN].cursor_status, CursorStatus::Hover);
    tap(&mut s, N64Buttons::A);
    assert_eq!(s.slots[MAN].held, Some(COM));
    assert!(!s.is_ready());
    // Let the cursor glide onto its pickup point.
    for _ in 0..30 {
        idle(&mut s);
    }
    hover_portrait(&mut s, FighterKind::Pikachu);
    assert_eq!(s.slots[COM].kind, Some(FighterKind::Pikachu));
    // Off the grid the CPU puck is dropped at once, keeping Pikachu.
    s.slots[MAN].cursor = (70.0, 190.0);
    idle(&mut s);
    assert!(s.slots[COM].is_selected);
    assert_eq!(s.slots[COM].kind, Some(FighterKind::Pikachu));
    assert_eq!(s.slots[MAN].held, None);
    assert!(s.is_ready());
}

#[test]
fn c_buttons_pick_a_costume_the_other_slot_is_not_wearing() {
    let mut s = first_visit(0); // CPU Mario, costume 0.
    place(&mut s, FighterKind::Mario);
    // The free costume skips the CPU's.
    assert_eq!(s.slots[MAN].costume, 1);
    tap(&mut s, N64Buttons::C_DOWN);
    assert_eq!(s.slots[MAN].costume, 2);
    // C-Up is costume 0, the CPU's: refused.
    tap(&mut s, N64Buttons::C_UP);
    assert_eq!(s.slots[MAN].costume, 2);
}

#[test]
fn a_c_button_places_the_held_puck_in_that_costume() {
    let mut s = first_visit(0);
    hover_portrait(&mut s, FighterKind::Link);
    tap(&mut s, N64Buttons::C_RIGHT);
    assert!(s.slots[MAN].is_fighter_selected);
    assert_eq!(
        s.slots[MAN].costume,
        costume_common_id(FighterKind::Link, 1)
    );
}

#[test]
fn a_placed_puck_is_pushed_back_inside_its_portrait() {
    let mut s = first_visit(0);
    // The puck's centre 2 pixels inside Kirby's left edge.
    let edge_x = (portrait(FighterKind::Kirby) % 6) as f32 * 45.0 + 25.0;
    s.slots[MAN].cursor = (edge_x + 2.0 - 13.0 - 11.0, 100.0 - 12.0 + 14.0);
    idle(&mut s);
    tap(&mut s, N64Buttons::A);
    assert_eq!(s.slots[MAN].kind, Some(FighterKind::Kirby));
    let before = s.slots[MAN].puck.0;
    for _ in 0..40 {
        idle(&mut s);
    }
    let centre = s.slots[MAN].puck.0 + 13.0;
    assert!(s.slots[MAN].puck.0 > before);
    assert!(centre >= edge_x + 4.0, "centre {centre}");
}

#[test]
fn saved_fighters_start_placed_and_ready() {
    let s = FighterSelect::new(
        SceneData {
            man_kind: Some(FighterKind::Kirby),
            man_costume: 2,
            com_kind: Some(FighterKind::Fox),
            com_costume: 1,
        },
        0,
        || unreachable!(),
    );
    assert!(s.is_ready());
    assert_eq!(s.slots[MAN].costume, 2);
    assert_eq!(s.slots[MAN].puck, portrait_center(FighterKind::Kirby));
}

#[test]
fn an_unshared_fighter_that_is_not_placed_returns_to_its_first_costume() {
    let mut s = first_visit(0);
    hover_portrait(&mut s, FighterKind::Yoshi);
    s.slots[MAN].costume = 3;
    idle(&mut s);
    assert_eq!(s.slots[MAN].costume, 0);
}

#[test]
fn five_idle_minutes_return_to_the_title() {
    let mut s = first_visit(0);
    let mut outcome = None;
    for _ in 0..RETURN_TICS {
        outcome = idle(&mut s);
    }
    assert!(matches!(outcome, Some(Outcome::Timeout(_))));
}
