use super::*;

fn sprites(slot: u32) -> Option<SpriteInfo> {
    let y = if (MENU_LABEL + 4..MENU_LABEL + 6 * 8).contains(&slot) {
        65 + ((slot - MENU_LABEL - 4) / 8) as i16 * 20
    } else {
        20
    };
    Some(SpriteInfo {
        size: [9, 11],
        color: [255; 4],
        pos: [86, y],
    })
}

fn draws(m: &TrainingMenu, open: bool) -> alloc::vec::Vec<Draw> {
    let mut out = alloc::vec::Vec::new();
    visit(m, &m.stats, open, 0, sprites, |d| out.push(d));
    out
}

#[test]
fn counters_hold_90_process_ticks_then_clear_and_new_hits_replace_them() {
    let mut s = Stats::default();
    s.observe(27, 3);
    s.observe(0, 0);
    assert_eq!((s.damage.shown, s.combo.shown), (27, 3));
    // Drawing the same frame twice must not restart the wait.
    s.observe(0, 0);
    for _ in 0..89 {
        s.tick();
        s.observe(0, 0);
    }
    assert_eq!((s.damage.shown, s.combo.shown), (27, 3));
    s.tick();
    s.observe(0, 0);
    assert_eq!((s.damage.shown, s.combo.shown), (0, 0));
    s.observe(1200, 101);
    assert_eq!((s.damage.shown, s.combo.shown), (999, 99));
    s.observe(0, 0);
    s.tick();
    s.observe(4, 1);
    assert_eq!((s.damage.shown, s.combo.shown), (4, 1));
}

#[test]
fn opening_replaces_stats_with_the_menu_and_closing_restores_them() {
    let m = TrainingMenu::new(0);
    let closed = draws(&m, false);
    let open = draws(&m, true);
    assert!(closed
        .iter()
        .all(|d| matches!(d, Draw::Sprite { slot, .. } if *slot < MENU_LABEL)));
    assert!(open.iter().any(|d| matches!(
        d,
        Draw::Fill {
            color: [0, 100, 255, 100],
            ..
        }
    )));
    assert!(open
        .iter()
        .filter_map(|d| if let Draw::Sprite { slot, .. } = d {
            Some(*slot)
        } else {
            None
        })
        .all(|slot| slot >= MENU_LABEL));
    assert_eq!(draws(&m, false), closed);
}

#[test]
fn all_rows_move_cursor_and_underline_and_reset_exit_hide_arrows() {
    let mut m = TrainingMenu::new(0);
    for (i, option) in MainOption::ALL.into_iter().enumerate() {
        m.main_option = option;
        let d = draws(&m, true);
        assert!(d.iter().any(|d| matches!(d, Draw::Sprite { slot, pos, .. } if *slot == MENU_OPTION + 30 * 4 && *pos == [71.0, 64.0 + i as f32 * 20.0])));
        let arrows = d.iter().filter(|d| matches!(d, Draw::Sprite { slot, .. } if *slot == MENU_OPTION + 28 * 4 || *slot == MENU_OPTION + 29 * 4)).count();
        assert_eq!(arrows, if i < 4 { 2 } else { 0 });
        assert!(d.iter().any(|d| matches!(d, Draw::Fill { rect, color: [255, 0, 0, 255] } if rect[0] == 73.0 && rect[3] - rect[1] == 2.0)));
    }
}

#[test]
fn view_uses_callback_order_and_motion_bomb_keeps_arrows_level() {
    let mut m = TrainingMenu::new(0);
    m.main_option = MainOption::Item;
    m.item_option = 11;
    let d = draws(&m, true);
    assert!(d.iter().any(|d| matches!(d, Draw::Sprite { slot, pos, .. } if *slot == MENU_OPTION + 11 * 4 && pos[1] == 83.0)));
    assert!(d.iter().any(|d| matches!(d, Draw::Sprite { slot, pos, .. } if *slot == MENU_OPTION + 28 * 4 && pos[1] == 88.0)));
    assert!(d
        .iter()
        .any(|d| matches!(d, Draw::Sprite { slot, .. } if *slot == MENU_OPTION + 27 * 4)));
    m.view_option = 0;
    assert!(draws(&m, true)
        .iter()
        .any(|d| matches!(d, Draw::Sprite { slot, .. } if *slot == MENU_OPTION + 26 * 4)));
}

#[test]
fn digits_truncate_half_width_and_item_brackets_follow_the_icon() {
    let mut m = TrainingMenu::new(0);
    m.stats.observe(123, 45);
    let d = draws(&m, false);
    for (digit, x, y) in [
        (1, 70.0, 20.0),
        (2, 80.0, 20.0),
        (3, 90.0, 20.0),
        (4, 64.0, 36.0),
        (5, 74.0, 36.0),
    ] {
        assert!(d.iter().any(|d| matches!(d, Draw::Sprite { slot, pos, .. } if *slot == DISPLAY_OPTION + digit * 4 && *pos == [x,y])));
    }
    let mut d = alloc::vec::Vec::new();
    visit(&m, &m.stats, false, 8, sprites, |x| d.push(x));
    for (index, x) in [(18, 283.0), (36, 274.0), (37, 292.0)] {
        assert!(d.iter().any(|d| matches!(d, Draw::Sprite { slot, pos, .. } if *slot == DISPLAY_OPTION + index * 4 && *pos == [x,36.0])));
    }
}
