use super::*;

/// The damage sprites' sizes in file 164.
const SIZES: [(u16, u16); 12] = [
    (16, 19),
    (11, 19),
    (17, 19),
    (16, 19),
    (17, 19),
    (15, 19),
    (17, 19),
    (16, 19),
    (17, 19),
    (17, 19),
    (19, 16),
    (22, 12),
];

fn shown(player: usize, damage: i32) -> DamageDisplay {
    let mut d = DamageDisplay::new(player, damage);
    d.is_show_interface = true;
    d
}

#[test]
fn digits_then_percent() {
    let mut digits = [0; 4];
    assert_eq!(percent_digits(0, &mut digits), 2);
    assert_eq!(digits[..2], [0, PERCENT]);
    assert_eq!(percent_digits(999, &mut digits), 4);
    assert_eq!(digits, [9, 9, 9, PERCENT]);
    assert_eq!(percent_digits(40, &mut digits), 3);
    assert_eq!(digits[..3], [4, 0, PERCENT]);
}

#[test]
fn zero_percent_is_centred_on_the_player_slot() {
    let d = shown(0, 0);
    // The first update settles 1.04 to 1.0.
    assert_eq!(d.scale, 1.0);
    let g: Vec<Glyph> = d.glyphs(false, &SIZES).collect();
    assert_eq!(g.len(), 2);
    // "0%": 14 + 17 wide, centred on x 55 at the first frame's scale of
    // 1.04, which then settles to 1.0 without moving the glyphs.
    // `%` first, then the 0.
    assert_eq!(g[0].digit, PERCENT);
    assert_eq!(g[1].digit, 0);
    let right = 55.0 + 31.0 * 1.04 * 0.5;
    // `%`: centred 8.5 in from the right, its sprite 19 wide.
    assert!((g[0].x - (right - 8.5 - 9.5)).abs() < 1e-4, "{}", g[0].x);
    assert_eq!(g[0].y, 210.0 - 8.0);
    // 0: centred 7 further left, 16 wide, snapped to whole pixels.
    assert_eq!(g[1].x, (right - 17.0 - 7.0 - 8.0) as i32 as f32);
    assert_eq!(g[1].color, [0xFF, 0xF0, 0xF0]);
    assert!(!g[1].solid);
}

#[test]
fn a_hit_swells_and_flashes_then_settles() {
    let mut d = shown(1, 0);
    d.update(30, false);
    // 30 new points: 1 + 30/300, the white flash for one frame.
    assert!((d.scale - 1.1).abs() < 1e-6);
    let g: Vec<Glyph> = d.glyphs(false, &SIZES).collect();
    assert!(g.iter().all(|g| g.solid && g.color == [0xFF; 3]));
    assert_eq!(g.len(), 3);
    d.update(30, false);
    assert_eq!(d.color_id, 1);
    // Held for four frames, then 0.05 smaller each frame.
    for _ in 0..3 {
        d.update(30, false);
    }
    let held = d.scale;
    d.update(30, false);
    assert!((held - d.scale - 0.05).abs() < 1e-5, "{held} {}", d.scale);
    for _ in 0..10 {
        d.update(30, false);
    }
    assert_eq!(d.scale, 1.0);
}

#[test]
fn damage_darkens_the_digits() {
    let d = shown(2, 150);
    let c = d.glyphs(false, &SIZES).next().unwrap().color;
    // Halfway: (0xF0 - 100) / 2 + 100, (0xF0 - 20) / 2 + 20, (0xFF - 20) / 2 + 20.
    assert_eq!(c, [170, 130, 137]);
    let d = shown(0, 400);
    assert_eq!(d.glyphs(false, &SIZES).next().unwrap().color, [100, 20, 20]);
}

#[test]
fn a_fall_breaks_the_digits_apart() {
    crate::rng::set_seed(5);
    let mut d = shown(0, 45);
    d.start_break_anim();
    assert!(d.is_update_anim);
    let before: Vec<(f32, f32)> = d.chars.iter().map(|c| c.pos).collect();
    for _ in 0..40 {
        d.update(45, false);
    }
    // Every glyph of "45%" has been set falling and has dropped.
    for (i, (c, start)) in d.chars.iter().zip(&before).take(3).enumerate() {
        assert!(c.is_lock_movement, "glyph {i}");
        assert!(c.pos.1 > start.1 + 100.0);
    }
    // The rebirth restores the digits at 0%.
    d.stop_break_anim();
    d.update(0, false);
    let g: Vec<Glyph> = d.glyphs(false, &SIZES).collect();
    assert_eq!(g.len(), 2);
    assert_eq!(g[0].y, 210.0 - 8.0 * g[0].scale);
}

#[test]
fn hidden_until_shown_and_gone_after_the_last_stock() {
    let mut d = DamageDisplay::new(0, 0);
    assert_eq!(d.glyphs(false, &SIZES).count(), 0);
    d.is_show_interface = true;
    for _ in 0..180 {
        d.update(0, true);
    }
    assert_eq!(d.glyphs(true, &SIZES).count(), 0);
}

#[test]
fn the_emblem_sits_behind_the_digits() {
    // Mario's 27x25 emblem for player 1: 55 - 13.5 + 3, 210 - 12.5 - 3.
    assert_eq!(emblem_origin(POSITION_X[0], 27, 25), (44.0, 194.0));
    assert_eq!(emblem_origin(POSITION_X[1], 30, 24), (113.0, 195.0));
}

#[test]
fn the_timer_rounds_up_below_the_limit() {
    let limit = 3 * 3600;
    assert_eq!(timer_digits(limit, limit), [0, 3, 0, 0]);
    // One tick in: 2:59 plus 59 ticks rounds to 3:00.
    assert_eq!(timer_digits(limit - 1, limit), [0, 3, 0, 0]);
    assert_eq!(timer_digits(limit - 60, limit), [0, 2, 5, 9]);
    assert_eq!(timer_digits(1, limit), [0, 0, 0, 1]);
    assert_eq!(timer_digits(0, limit), [0, 0, 0, 0]);
    assert_eq!(timer_digits(10 * 3600, 10 * 3600), [1, 0, 0, 0]);
}

#[test]
fn timer_glyphs_are_centred_on_their_slots() {
    assert_eq!(timer_origin(232.0, 13, 17), (225.0, 21.0));
}

#[test]
fn stock_icons_sit_above_the_damage() {
    // A time battle: one 8x10 icon, 55 - 24 - 4 across, 210 - 5 - 20 down.
    let single: Vec<_> = stock_icons(POSITION_X[0], 2, true, 8, 10).collect();
    assert_eq!(single, [(27.0, 185.0)]);
    // Three stocks left in a stock battle: four icons, ten apart.
    let multi: Vec<_> = stock_icons(POSITION_X[1], 3, false, 8, 10).collect();
    assert_eq!(
        multi,
        [
            (97.0, 185.0),
            (107.0, 185.0),
            (117.0, 185.0),
            (127.0, 185.0)
        ]
    );
    // Sudden death's last stock shows one icon; out of stocks, none.
    assert_eq!(stock_icons(POSITION_X[0], 0, false, 8, 10).count(), 1);
    assert_eq!(stock_icons(POSITION_X[0], -1, true, 8, 10).count(), 0);
}
