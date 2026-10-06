use super::*;
use crate::fighter_select::portrait_center;

fn pad(buttons: u16, stick_x: i8, stick_y: i8) -> ControllerState {
    ControllerState {
        buttons: N64Buttons(buttons),
        stick_x,
        stick_y,
        connected: true,
    }
}

fn idle(s: &mut Players1PBonus) -> Option<Outcome> {
    s.tick(pad(0, 0, 0), N64Buttons(0))
}

fn tap(s: &mut Players1PBonus, button: u16) -> Option<Outcome> {
    s.tick(pad(button, 0, 0), N64Buttons(button))
}

fn select(bonus: BonusKind) -> Players1PBonus {
    Players1PBonus::new(SceneData { player: 0, bonus }, &Backup::default())
}

/// Puts the cursor where the held puck's centre lands on `kind`'s
/// portrait, then lets one tick update the cursor and the puck.
fn hover_portrait(s: &mut Players1PBonus, kind: FighterKind) {
    let (px, py) = portrait_center(kind);
    s.slot.cursor = (px - 11.0, py + 14.0);
    idle(s);
}

fn place(s: &mut Players1PBonus, kind: FighterKind) -> Option<Outcome> {
    hover_portrait(s, kind);
    tap(s, N64Buttons::A)
}

/// Taps A with the cursor's point (its corner plus (20, 3)) at `(x, y)`.
fn press_at(s: &mut Players1PBonus, x: f32, y: f32) -> Option<Outcome> {
    s.slot.cursor = (x - 20.0, y - 3.0);
    tap(s, N64Buttons::A)
}

#[test]
fn every_visit_starts_holding_an_empty_puck() {
    let s = select(BonusKind::Platforms);
    assert!(s.slot.is_held);
    assert_eq!(s.slot.kind, None);
    assert_eq!(s.slot.cursor, (80.0, 170.0));
    assert_eq!(s.slot.puck, (51.0, 161.0));
    assert_eq!(s.bonus, BonusKind::Platforms);
    assert!(!s.is_selected);
    assert!(s.view.fighter.is_none());
    assert!(!s.view.name_shown);
    // The puck follows the cursor from the first tick; it shows only once
    // the cursor stops pointing.
    let mut s = s;
    idle(&mut s);
    assert_eq!(s.slot.puck, (91.0, 156.0));
    assert!(!s.view.puck_shown);
    hover_portrait(&mut s, FighterKind::Kirby);
    assert!(s.view.puck_shown);
    assert_eq!(s.slot.kind, Some(FighterKind::Kirby));
    assert_eq!(s.slot.cursor_status, CursorStatus::Grab);
    assert_eq!(s.view.fighter.map(|f| f.kind), Some(FighterKind::Kirby));
    // Locked Ness: no fighter, but the records still read his portrait.
    hover_portrait(&mut s, FighterKind::Ness);
    assert_eq!(s.slot.kind, None);
    assert_eq!(s.force_puck_fighter_kind(), Some(FighterKind::Ness));
    assert!(s.view.fighter.is_some_and(|f| f.hidden));
}

#[test]
fn placing_arms_the_start_which_runs_out_after_140_ticks() {
    let mut s = select(BonusKind::Targets);
    assert_eq!(place(&mut s, FighterKind::Fox), None);
    assert!(s.is_selected);
    assert_eq!(s.start_wait, START_WAIT);
    assert!(s.view.flash.is_some());
    for _ in 0..START_WAIT - 1 {
        assert_eq!(idle(&mut s), None);
    }
    let Some(Outcome::Proceed { saved, bonus }) = idle(&mut s) else {
        panic!("expected Proceed");
    };
    assert_eq!(bonus, BonusKind::Targets);
    assert_eq!(
        saved,
        Saved {
            player: 0,
            bonus_fkind: Some(FighterKind::Fox),
            bonus_costume: 0,
        }
    );
}

#[test]
fn start_proceeds_at_once_but_only_when_armed() {
    let mut s = select(BonusKind::Platforms);
    // Tick 1, nothing placed: START does nothing.
    assert_eq!(tap(&mut s, N64Buttons::START), None);
    place(&mut s, FighterKind::Link);
    tap(&mut s, N64Buttons::C_DOWN);
    let out = tap(&mut s, N64Buttons::START);
    assert_eq!(
        out,
        Some(Outcome::Proceed {
            saved: Saved {
                player: 0,
                bonus_fkind: Some(FighterKind::Link),
                bonus_costume: costume_common_id(FighterKind::Link, 2),
            },
            bonus: BonusKind::Platforms,
        })
    );
}

#[test]
fn a_c_button_places_and_arms_in_its_costume() {
    let mut s = select(BonusKind::Targets);
    hover_portrait(&mut s, FighterKind::Link);
    tap(&mut s, N64Buttons::C_RIGHT);
    assert!(s.slot.is_fighter_selected);
    assert!(s.is_selected);
    assert_eq!(s.slot.costume, costume_common_id(FighterKind::Link, 1));
    assert_eq!(s.slot.costume, 2);
}

#[test]
fn picking_the_puck_up_disarms_the_start() {
    let mut s = select(BonusKind::Targets);
    place(&mut s, FighterKind::Mario);
    // Let the grab wait pass, then point at the puck and grab it.
    for _ in 0..30 {
        idle(&mut s);
    }
    let puck = s.slot.puck;
    s.slot.cursor = (puck.0 - 25.0 + 5.0, puck.1 - 3.0 + 5.0);
    idle(&mut s);
    assert_eq!(s.slot.cursor_status, CursorStatus::Hover);
    tap(&mut s, N64Buttons::A);
    assert!(s.slot.is_held);
    assert!(!s.slot.is_fighter_selected);
    // `FuncRun` clears the armed start on the next tick.
    assert!(s.is_selected);
    idle(&mut s);
    assert!(!s.is_selected);
    assert_eq!(tap(&mut s, N64Buttons::START), None);
}

#[test]
fn a_on_the_title_swaps_the_practice() {
    let mut s = select(BonusKind::Targets);
    assert_eq!(press_at(&mut s, 100.0, 20.0), None);
    assert_eq!(s.bonus, BonusKind::Platforms);
    press_at(&mut s, 27.0, 35.0);
    assert_eq!(s.bonus, BonusKind::Targets);
    // Just past the title's right edge: nothing.
    press_at(&mut s, 208.0, 20.0);
    assert_eq!(s.bonus, BonusKind::Targets);
    // The swapped practice goes with the way out.
    press_at(&mut s, 100.0, 20.0);
    let out = press_at(&mut s, 260.0, 20.0);
    assert!(matches!(
        out,
        Some(Outcome::Back {
            bonus: BonusKind::Platforms,
            ..
        })
    ));
}

#[test]
fn b_recalls_a_placed_puck_and_backs_out_when_idle() {
    let mut s = select(BonusKind::Targets);
    // B before tick 10 does nothing.
    assert_eq!(tap(&mut s, N64Buttons::B), None);
    place(&mut s, FighterKind::Yoshi);
    s.slot.cursor = (150.0, 180.0);
    assert_eq!(tap(&mut s, N64Buttons::B), None);
    assert!(s.slot.is_recalling);
    for _ in 0..10 {
        assert_eq!(idle(&mut s), None);
    }
    assert!(s.slot.is_held);
    while s.slot.is_recalling {
        idle(&mut s);
    }
    // The puck is back in the hand over the portrait it left.
    let Some(Outcome::Back { saved, bonus }) = tap(&mut s, N64Buttons::B) else {
        panic!("expected Back");
    };
    assert_eq!(bonus, BonusKind::Targets);
    assert_eq!(saved.player, 0);
}

#[test]
fn an_unplaced_puck_still_saves_its_fighter() {
    let mut s = select(BonusKind::Targets);
    hover_portrait(&mut s, FighterKind::Pikachu);
    assert!(!s.slot.is_fighter_selected);
    for _ in 0..10 {
        idle(&mut s);
    }
    // B with the puck held over Pikachu backs out with him.
    let Some(Outcome::Back { saved, .. }) = tap(&mut s, N64Buttons::B) else {
        panic!("expected Back");
    };
    assert_eq!(saved.bonus_fkind, Some(FighterKind::Pikachu));
    assert_eq!(saved.bonus_costume, 0);
    // Off the portraits it saves `nFTKindNull`.
    s.slot.cursor = (240.0, 17.0);
    idle(&mut s);
    assert_eq!(s.saved().bonus_fkind, None);
    // A held puck over a portrait is placed by A wherever the cursor
    // points: it never reaches the back button.
    let mut s = select(BonusKind::Targets);
    hover_portrait(&mut s, FighterKind::Pikachu);
    assert_eq!(press_at(&mut s, 260.0, 20.0), None);
    assert!(s.slot.is_fighter_selected);
}

#[test]
fn five_idle_minutes_time_out() {
    let mut s = select(BonusKind::Platforms);
    let mut out = None;
    for _ in 0..RETURN_TICS {
        out = idle(&mut s);
    }
    assert!(matches!(out, Some(Outcome::Timeout(_))));
    // Input moves the deadline.
    let mut s = select(BonusKind::Platforms);
    for _ in 0..RETURN_TICS - 2 {
        idle(&mut s);
    }
    assert_eq!(s.tick(pad(0, 30, 0), N64Buttons(0)), None);
    assert_eq!(idle(&mut s), None);
}

#[test]
fn hundredths_step_as_the_source_rounds() {
    let cs: Vec<i32> = (0..7).map(|t| time(t).csecs).collect();
    assert_eq!(cs, [0, 1, 3, 5, 7, 9, 10]);
    assert_eq!(
        time(3 * 3600 + 25 * 60 + 59),
        Time {
            mins: 3,
            secs: 25,
            csecs: 99
        }
    );
    assert_eq!(time(BEST_TIME_MAX).mins, 59);
}

#[test]
fn records_show_tasks_until_complete_then_the_time() {
    let mut backup = Backup::default();
    let fox = FighterKind::Fox as usize;
    backup.spgame_records[fox].bonus1_task_count = 7;
    backup.spgame_records[fox].bonus2_task_count = 10;
    backup.spgame_records[fox].bonus2_time = 75 * 60 + 13;
    let r = records(&backup, BonusKind::Targets, Some(FighterKind::Fox), 0);
    assert_eq!(r.fighter, Some(FighterRecord::Tasks(7)));
    assert_eq!(r.total, None);
    let r = records(&backup, BonusKind::Platforms, Some(FighterKind::Fox), 0);
    assert_eq!(
        r.fighter,
        Some(FighterRecord::Time(Time {
            mins: 1,
            secs: 15,
            csecs: 21
        }))
    );
    // An unset time (an hour) is clamped to 59:59:99.
    backup.spgame_records[fox].bonus2_time = 60 * 60 * 60;
    let r = records(&backup, BonusKind::Platforms, Some(FighterKind::Fox), 0);
    assert_eq!(
        r.fighter,
        Some(FighterRecord::Time(Time {
            mins: 59,
            secs: 59,
            csecs: 99
        }))
    );
    assert_eq!(
        records(&backup, BonusKind::Platforms, None, 0).fighter,
        None
    );
}

#[test]
fn the_total_carries_unlocked_fighters_times() {
    let mut backup = Backup::default();
    for r in backup.spgame_records.iter_mut() {
        r.bonus1_task_count = 10;
        // 0:59 and 59 ticks: 99 hundredths.
        r.bonus1_time = 59 * 60 + 59;
    }
    // All twelve unlocked: 12 x 99 = 1188 hundredths, 11 s carried;
    // 12 x 59 + 11 = 719 s = 11:59.
    let r = records(&backup, BonusKind::Targets, None, CHARACTER_MASK_ALL);
    assert_eq!(
        r.total,
        Some(Time {
            mins: 11,
            secs: 59,
            csecs: 88
        })
    );
    // The starter mask: eight summed, 792 hundredths, 7 s carried;
    // 8 x 59 + 7 = 479 s = 7:59.
    let r = records(&backup, BonusKind::Targets, None, 0);
    assert_eq!(
        r.total,
        Some(Time {
            mins: 7,
            secs: 59,
            csecs: 92
        })
    );
    // One fighter short of ten platforms: no total.
    backup.spgame_records[11].bonus1_task_count = 9;
    assert_eq!(records(&backup, BonusKind::Targets, None, 0).total, None);
}

#[test]
fn the_ready_banner_follows_the_armed_start() {
    let mut s = select(BonusKind::Targets);
    place(&mut s, FighterKind::Samus);
    assert!(s.view.ready_banner && s.view.ready_press);
    // Both GObjs step the one counter: hidden from 30 within each 40.
    let mut shown = 0;
    for _ in 0..40 {
        idle(&mut s);
        shown += usize::from(s.view.ready_banner);
    }
    assert!(shown > 0 && shown < 40);
}
