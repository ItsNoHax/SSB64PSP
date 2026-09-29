use super::*;

#[test]
fn the_dead_explode_flash_rises_for_6_frames_and_fades_for_30() {
    let mut c = ColAnim::default();
    assert!(c.check_set(ColAnimId::ScreenFlashDeadExplode, 0));
    let mut alphas: Vec<Option<u8>> = Vec::new();
    let mut ended_at = None;
    for frame in 0..40 {
        if c.update() {
            ended_at = Some(frame);
            c.reset();
            break;
        }
        alphas.push(c.color().map(|rgba| rgba[3]));
    }
    // `(0x6E - 0) / 6` = 18 a frame up, then `(0 - 108) / 30` = -3 down.
    let up: Vec<_> = (1..=6).map(|k| Some(18 * k)).collect();
    assert_eq!(alphas[..6], up[..]);
    assert_eq!(alphas[6], Some(105));
    assert_eq!(alphas[35], Some(18));
    assert_eq!(alphas.len(), 36);
    // `ClearColorAll`, then `End` on the 37th frame.
    assert_eq!(ended_at, Some(36));
    assert_eq!(c.color(), None);
    assert!(c.color1.rgba[..3] == [0xFF; 3]);
}

#[test]
fn the_rebirth_glow_is_lit_from_below_and_pulses_forever() {
    let mut c = ColAnim::default();
    c.check_set(ColAnimId::FighterRebirth, 0);
    c.run_update();
    assert_eq!(c.light, Some((0.0, -70.0)));
    assert_eq!(c.color(), Some([0xFF, 0xFF, 0xFF, 0xFF]));
    let mut alphas = Vec::new();
    for _ in 0..(2 + 36 + 18) * 3 {
        c.run_update();
        alphas.push(c.color().unwrap()[3]);
    }
    // Waits 2, then `(10 - 255) / 36` = -6 a frame for 36 frames.
    assert_eq!(alphas[0], 0xFF);
    assert_eq!(alphas[1], 0xFF - 6);
    assert_eq!(alphas[36], 0xFF - 6 * 36);
    // Then `(0xB4 - 39) / 18` = 7 a frame for 18, and back to 0xFF.
    assert_eq!(alphas[37], 0xFF - 6 * 36 + 7);
    assert_eq!(alphas[54], 0xFF - 6 * 36 + 7 * 18);
    // `GotoStart`: full white again, held for two frames.
    assert_eq!(alphas[55..58], [0xFF, 0xFF, 0xFF - 6]);
    assert_eq!(c.id, ColAnimId::FighterRebirth);
}

#[test]
fn a_lower_priority_animation_does_not_replace_a_higher_one() {
    let mut c = ColAnim::default();
    c.check_set(ColAnimId::ScreenFlashDeadExplode, 0);
    assert!(!c.check_set(ColAnimId::FighterRebirth, 0));
    assert_eq!(c.id, ColAnimId::ScreenFlashDeadExplode);
    assert!(c.check_set(ColAnimId::ScreenFlashDeadExplode, 0));
}

#[test]
fn a_status_change_ends_an_unlocked_animation_unless_it_preserves_it() {
    let mut c = ColAnim::default();
    c.check_set(ColAnimId::FighterRebirth, 0);
    c.run_update();
    on_set_status(&mut c, true);
    assert_eq!(c.id, ColAnimId::FighterRebirth);
    on_set_status(&mut c, false);
    assert_eq!(c.id, ColAnimId::None);
    assert_eq!((c.color(), c.light), (None, None));
}
