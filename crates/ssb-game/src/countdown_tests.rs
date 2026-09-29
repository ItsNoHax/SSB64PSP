use super::*;

/// File 82's sprite sizes, from its ROM decode.
const SIZES: [(u16, u16); 24] = [
    (62, 73),
    (70, 74),
    (24, 73),
    (36, 56),
    (17, 57),
    (50, 56),
    (32, 56),
    (41, 58),
    (36, 56),
    (39, 58),
    (43, 56),
    (41, 57),
    (8, 53),
    (97, 33),
    (15, 11),
    (15, 15),
    (11, 11),
    (19, 19),
    (32, 41),
    (30, 32),
    (46, 49),
    (48, 57),
    (53, 53),
    (55, 55),
];

fn run(c: &mut Countdown, ticks: u32) {
    for _ in 0..ticks {
        c.tick(&SIZES);
    }
}

#[test]
fn it_starts_above_the_screen_with_the_red_lamp_lit() {
    let c = Countdown::new();
    let s: Vec<SObj> = c.sobjs().collect();
    assert_eq!(s.len(), 10);
    assert_eq!(s[0].sprite, ROD);
    assert_eq!((s[0].pos.0, s[0].pos.1), (103.0, -57.0));
    // Lamp 10: the red light, colour id 6.
    assert_eq!(s[8].sprite, 18);
    assert_eq!(s[9].sprite, ROD_SHADOW);
    assert_eq!(s[9].prim, [0, 0, 0]);
}

#[test]
fn it_slides_down_53_units_in_60_ticks() {
    let mut c = Countdown::new();
    run(&mut c, 60);
    let rod = c.sobjs().next().unwrap();
    assert!(
        (rod.pos.1 - (-57.0 + 60.0 * 0.883_333_3)).abs() < 1e-3,
        "{}",
        rod.pos.1
    );
}

#[test]
fn three_pops_a_yellow_lamp_that_shrinks_back() {
    let mut c = Countdown::new();
    run(&mut c, 121);
    // Tick 120 replaced the red lamp pair with lamps 6 and 11.
    let s: Vec<SObj> = c.sobjs().collect();
    assert_eq!(s.len(), 10);
    let pop = s[8];
    assert_eq!(pop.sprite, 22, "yellow contour");
    // 3.0, then 0.2 smaller the same tick, centred on its data position.
    assert!((pop.scale - 2.8).abs() < 1e-6);
    let r = (2.8 - 1.0) * 0.5;
    assert!((pop.pos.0 - (119.0 - r * 53.0)).abs() < 1e-4);
    assert_eq!(s[9].sprite, 19, "yellow light");
    run(&mut c, 20);
    assert_eq!(c.sobjs().nth(8).unwrap().scale, 1.0);
}

#[test]
fn go_turns_blue_moves_the_shadow_and_shows_the_letters() {
    let mut c = Countdown::new();
    run(&mut c, 301);
    let s: Vec<SObj> = c.sobjs().collect();
    // Rod, frame, five dim lamps, the blue pair, the lit shadow, G O !.
    assert_eq!(s.len(), 13);
    assert_eq!(s[7].sprite, 23, "blue contour");
    assert_eq!(s[8].sprite, 20, "blue light");
    assert_eq!(s[9].sprite, ROD_SHADOW);
    assert_eq!(s[9].prim, [0x6A, 0x6A, 0x95]);
    assert_eq!(s[9].env, [0x12, 0x12, 0x2E]);
    assert_eq!(s[10].sprite, LETTER_G);
    // The letters last 60 ticks.
    run(&mut c, 59);
    assert_eq!(c.sobjs().filter(|s| s.sprite == LETTER_G).count(), 1);
    run(&mut c, 1);
    assert_eq!(c.sobjs().filter(|s| s.sprite == LETTER_G).count(), 0);
}

#[test]
fn it_slides_away_and_ends_at_seven_seconds() {
    let mut c = Countdown::new();
    run(&mut c, 420);
    assert!(!c.done);
    let rod = c.sobjs().next().unwrap();
    assert!((rod.pos.1 - -57.0).abs() < 1e-2, "{}", rod.pos.1);
    run(&mut c, 1);
    assert!(c.done);
    assert_eq!(c.sobjs().count(), 0);
}

#[test]
fn sudden_death_shows_only_the_letters() {
    let mut c = Countdown::sudden_death();
    assert_eq!(c.sobjs().count(), 0);
    c.start_go();
    c.tick(&SIZES);
    assert_eq!(
        c.sobjs().map(|s| s.sprite).collect::<Vec<_>>(),
        [LETTER_G, LETTER_O, EXCLAMATION]
    );
}
