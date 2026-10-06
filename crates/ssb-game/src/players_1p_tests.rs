use super::*;

fn pad(buttons: u16, stick_x: i8, stick_y: i8) -> ControllerState {
    ControllerState {
        buttons: N64Buttons(buttons),
        stick_x,
        stick_y,
        connected: true,
    }
}

fn idle(s: &mut Players1P) -> Option<Outcome> {
    s.tick(pad(0, 0, 0), N64Buttons(0))
}

fn tap(s: &mut Players1P, button: u16) -> Option<Outcome> {
    s.tick(pad(button, 0, 0), N64Buttons(button))
}

fn first_visit() -> Players1P {
    Players1P::new(SceneData::default(), &Backup::default())
}

/// Puts the cursor where the held puck's centre lands on `kind`'s
/// portrait, then lets one tick update the cursor and the puck.
fn hover_portrait(s: &mut Players1P, kind: FighterKind) {
    let (px, py) = portrait_center(kind);
    s.slot.cursor = (px - 11.0, py + 14.0);
    idle(s);
}

fn place(s: &mut Players1P, kind: FighterKind) {
    hover_portrait(s, kind);
    tap(s, N64Buttons::A);
}

/// Taps A with the cursor's point (its corner plus (20, 3)) at `(x, y)`.
fn press_at(s: &mut Players1P, x: f32, y: f32) -> Option<Outcome> {
    s.slot.cursor = (x - 20.0, y - 3.0);
    tap(s, N64Buttons::A)
}

#[test]
fn a_first_visit_holds_the_puck_with_the_backups_level_and_stocks() {
    let backup = Backup {
        spgame_difficulty: Difficulty::Hard,
        spgame_stock_count: 4,
        ..Backup::default()
    };
    let s = Players1P::new(SceneData::default(), &backup);
    assert!(s.slot.is_held);
    assert_eq!(s.slot.kind, None);
    assert_eq!(s.slot.cursor, (60.0, 170.0));
    assert_eq!(s.slot.puck, (51.0, 161.0));
    assert_eq!(s.level, Difficulty::Hard as i32);
    assert_eq!(s.stock, 4);
    assert_eq!(s.time_setting, 5);
    assert!(!s.is_ready());
}

#[test]
fn a_return_visit_starts_placed_and_ready() {
    let scene = SceneData {
        kind: Some(FighterKind::Fox),
        costume: 2,
        ..SceneData::default()
    };
    let s = Players1P::new(scene, &Backup::default());
    assert!(s.is_ready());
    assert!(!s.slot.is_held);
    assert_eq!(s.slot.puck, portrait_center(FighterKind::Fox));
    assert_eq!(s.slot.costume, 2);
    assert!(s.view.fighter.is_some());
}

#[test]
fn the_held_puck_follows_the_cursor_and_skips_locked_fighters() {
    let mut s = first_visit();
    idle(&mut s);
    assert_eq!(s.slot.puck, (71.0, 156.0));
    hover_portrait(&mut s, FighterKind::Kirby);
    assert_eq!(s.slot.kind, Some(FighterKind::Kirby));
    assert_eq!(s.slot.cursor_status, CursorStatus::Grab);
    assert_eq!(s.slot.costume, costume_common_id(FighterKind::Kirby, 0));
    // Ness is locked in the starter mask: no fighter, but the records
    // still read his portrait.
    hover_portrait(&mut s, FighterKind::Ness);
    assert_eq!(s.slot.kind, None);
    assert_eq!(s.force_puck_fighter_kind(), Some(FighterKind::Ness));
}

#[test]
fn a_places_in_the_first_costume_and_c_buttons_pick_others() {
    let mut s = first_visit();
    place(&mut s, FighterKind::Mario);
    assert!(s.is_ready());
    assert!(s.slot.is_selected);
    assert_eq!(s.slot.costume, costume_common_id(FighterKind::Mario, 0));
    assert!(s.view.flash.is_some());
    tap(&mut s, N64Buttons::C_DOWN);
    assert_eq!(s.slot.costume, costume_common_id(FighterKind::Mario, 2));

    let mut s = first_visit();
    hover_portrait(&mut s, FighterKind::Link);
    tap(&mut s, N64Buttons::C_LEFT);
    assert!(s.slot.is_selected);
    assert_eq!(s.slot.costume, costume_common_id(FighterKind::Link, 3));
}

#[test]
fn start_after_sixty_ticks_proceeds_thirty_later_with_the_saved_data() {
    let mut s = first_visit();
    place(&mut s, FighterKind::Samus);
    // Tick 3: too early.
    assert_eq!(tap(&mut s, N64Buttons::START), None);
    while s.total_tics < 60 {
        idle(&mut s);
    }
    assert_eq!(tap(&mut s, N64Buttons::START), None);
    let cursor = s.slot.cursor;
    for _ in 0..29 {
        // The cursor is paused.
        assert_eq!(s.tick(pad(0, 80, 0), N64Buttons(0)), None);
    }
    assert_eq!(s.slot.cursor, cursor);
    let Some(Outcome::Proceed(saved)) = idle(&mut s) else {
        panic!("expected Proceed");
    };
    assert_eq!(saved.scene.kind, Some(FighterKind::Samus));
    assert_eq!(saved.scene.player, 0);
    // `dSCManagerDefaultBackupData`'s Easy and two stocks.
    assert_eq!(saved.difficulty, Difficulty::Easy);
    assert_eq!(saved.stock_count, 2);
}

#[test]
fn b_recalls_the_puck_into_the_hand_and_backs_out_only_when_idle() {
    let mut s = first_visit();
    place(&mut s, FighterKind::Yoshi);
    s.slot.cursor = (150.0, 180.0);
    tap(&mut s, N64Buttons::B);
    assert!(s.slot.is_recalling);
    assert!(!s.is_ready());
    for _ in 0..10 {
        assert_eq!(idle(&mut s), None);
    }
    assert!(s.slot.is_held);
    assert_eq!(s.slot.cursor_status, CursorStatus::Grab);
    while s.slot.is_recalling {
        idle(&mut s);
    }
    // Holding the puck, B goes back to the 1P menu.
    assert!(matches!(tap(&mut s, N64Buttons::B), Some(Outcome::Back(_))));
}

#[test]
fn the_level_and_stock_arrows_step_within_their_ranges() {
    let mut s = first_visit();
    for _ in 0..4 {
        press_at(&mut s, 270.0, 160.0);
    }
    assert_eq!(s.level, Difficulty::VeryHard as i32);
    let made = s.view.level_made;
    press_at(&mut s, 270.0, 160.0);
    assert_eq!(s.view.level_made, made);
    for _ in 0..6 {
        press_at(&mut s, 200.0, 160.0);
    }
    assert_eq!(s.level, Difficulty::VeryEasy as i32);
    for _ in 0..5 {
        press_at(&mut s, 270.0, 180.0);
    }
    assert_eq!(s.stock, STOCK_MAX);
    for _ in 0..5 {
        press_at(&mut s, 200.0, 180.0);
    }
    assert_eq!(s.stock, 0);
    assert!(!s.view.stock_arrows.left);
    assert!(s.view.stock_arrows.right);
    assert_eq!(s.saved().stock_count, 0);
    assert_eq!(s.saved().difficulty, Difficulty::VeryEasy);
}

#[test]
fn the_time_arrows_swap_five_minutes_and_infinite() {
    let mut s = first_visit();
    press_at(&mut s, 220.0, 20.0);
    assert_eq!(s.time_setting, TIMELIMIT_INFINITE);
    press_at(&mut s, 150.0, 20.0);
    assert_eq!(s.time_setting, 5);
}

#[test]
fn the_back_button_and_the_idle_timer_leave() {
    let mut s = first_visit();
    assert!(matches!(
        press_at(&mut s, 260.0, 20.0),
        Some(Outcome::Back(_))
    ));
    let mut s = first_visit();
    let mut out = None;
    for _ in 0..RETURN_TICS {
        out = idle(&mut s);
    }
    assert!(matches!(out, Some(Outcome::Timeout(_))));
}

#[test]
fn a_placed_puck_keeps_its_edge_velocity() {
    let mut s = first_visit();
    let (px, py) = portrait_center(FighterKind::Donkey);
    // The puck's centre 2 pixels inside the portrait's left edge, 3 short
    // of where `PuckAdjustPortraitEdge` keeps it.
    s.slot.cursor = (px - 22.0 - 11.0, py + 14.0);
    idle(&mut s);
    assert_eq!(s.slot.kind, Some(FighterKind::Donkey));
    tap(&mut s, N64Buttons::A);
    assert!(s.slot.puck_vel.0 > 0.0);
    for _ in 0..20 {
        idle(&mut s);
    }
    // `mnPlayers1PGamePuckAdjustPlaced` never clears it.
    assert!(s.slot.puck_vel.0 != 0.0);
}

#[test]
fn saved_data_reaches_the_campaign() {
    let mut s = first_visit();
    place(&mut s, FighterKind::Pikachu);
    let mut scene = spgame::SceneData {
        stage: 7,
        ..spgame::SceneData::default()
    };
    let mut backup = Backup::default();
    s.saved().apply(&mut scene, &mut backup);
    assert_eq!(scene.fkind, FighterKind::Pikachu);
    assert_eq!(scene.stage, 0);
    assert_eq!(backup.writes, 1);
    let selection = s.saved().selection();
    assert_eq!(selection.kind, Some(FighterKind::Pikachu));
    assert_eq!(selection.campaign().unwrap().fkind, FighterKind::Pikachu);
    assert_eq!(selection.stocks, backup.spgame_stock_count);
    // A puck left unplaced saves no fighter.
    let mut s = first_visit();
    hover_portrait(&mut s, FighterKind::Pikachu);
    assert_eq!(s.saved().scene.kind, None);
}

#[test]
fn records_sum_every_fighter() {
    let mut backup = Backup::default();
    backup.spgame_records[FighterKind::Fox as usize].spgame_hiscore = 1200;
    backup.spgame_records[FighterKind::Fox as usize].spgame_total_bonuses = 7;
    backup.spgame_records[FighterKind::Fox as usize].spgame_best_difficulty = 3;
    backup.spgame_records[FighterKind::Mario as usize].spgame_hiscore = 300;
    let r = Players1P::records(&backup, Some(FighterKind::Fox));
    assert_eq!(r.fighter, Some((1200, 7, 3)));
    assert_eq!(r.total_hiscore, 1500);
    assert_eq!(r.total_bonuses, 7);
    assert_eq!(Players1P::records(&backup, None).fighter, None);
}
