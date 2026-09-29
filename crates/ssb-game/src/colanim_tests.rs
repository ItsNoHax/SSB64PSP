use super::*;
use crate::status::{self, Status, StatusTiming};

fn fighter(kind: FighterKind) -> Fighter {
    let mut f = Fighter::new(kind, 0, 3);
    f.situation = crate::fighter::Situation::Ground;
    f
}

/// `color1` (when in use) and `skeleton_id` after each of `n` updates.
fn run(c: &mut ColAnim, n: usize) -> Vec<(Option<[u8; 4]>, u8, bool)> {
    (0..n)
        .map(|_| {
            let ended = c.update();
            (c.color(), c.skeleton_id, ended)
        })
        .collect()
}

#[test]
fn the_dead_explode_flash_rises_for_6_frames_and_fades_for_30() {
    let mut c = ColAnim::default();
    assert!(c.check_set(ColAnimId::SCREEN_FLASH_DEAD_EXPLODE, 0));
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
    c.check_set(ColAnimId::FIGHTER_REBIRTH, 0);
    assert!(!c.update());
    assert_eq!(c.light, Some((0.0, -70.0)));
    assert_eq!(c.color(), Some([0xFF, 0xFF, 0xFF, 0xFF]));
    let mut alphas = Vec::new();
    for _ in 0..(2 + 36 + 18) * 3 {
        assert!(!c.update());
        alphas.push(c.color().unwrap()[3]);
    }
    // Waits 2, then `(10 - 255) / 36` = -6 a frame for 36 frames.
    assert_eq!(alphas[0], 0xFF);
    assert_eq!(alphas[1], 0xFF - 6);
    assert_eq!(alphas[36], 0xFF - 6 * 36);
    // Then `(0xB4 - 39) / 18` = 7 a frame for 18, and back to 0xFF.
    assert_eq!(alphas[37], 0xFF - 6 * 36 + 7);
    assert_eq!(alphas[54], 0xFF - 6 * 36 + 7 * 18);
    // `Goto`: full white again, held for two frames.
    assert_eq!(alphas[55..58], [0xFF, 0xFF, 0xFF - 6]);
    assert_eq!(c.id, ColAnimId::FIGHTER_REBIRTH);
}

#[test]
fn a_lower_priority_animation_does_not_replace_a_higher_one() {
    let mut c = ColAnim::default();
    c.check_set(ColAnimId::SCREEN_FLASH_DEAD_EXPLODE, 0);
    assert!(!c.check_set(ColAnimId::FIGHTER_REBIRTH, 0));
    assert_eq!(c.id, ColAnimId::SCREEN_FLASH_DEAD_EXPLODE);
    assert!(c.check_set(ColAnimId::SCREEN_FLASH_DEAD_EXPLODE, 0));
}

#[test]
fn a_status_change_ends_an_unlocked_animation_unless_it_preserves_it() {
    let mut f = fighter(FighterKind::Mario);
    check_set(&mut f, ColAnimId::FIGHTER_REBIRTH, 0);
    run_update(&mut f);
    on_set_status(&mut f, true);
    assert_eq!(f.colanim.id, ColAnimId::FIGHTER_REBIRTH);
    on_set_status(&mut f, false);
    assert_eq!(f.colanim.id, ColAnimId::NONE);
    assert_eq!((f.colanim.color(), f.colanim.light), (None, None));
    // A locked one (`DamageCommon`) survives.
    check_set(&mut f, ColAnimId::FIGHTER_DAMAGE_COMMON, 0);
    on_set_status(&mut f, false);
    assert_eq!(f.colanim.id, ColAnimId::FIGHTER_DAMAGE_COMMON);
}

/// `dGMColScriptsDescs` is indexed by id, whatever `GMColAnimKind` calls
/// the entry.
#[test]
fn the_table_is_the_descs_order_with_their_priorities() {
    assert_eq!(DESCS.len(), 86);
    assert_eq!(ColAnimId(41).script(), Some(Script::FighterFoxSpecialLw));
    assert_eq!(
        ColAnimId(48).script(),
        Some(Script::FighterDonkeySpecialNLoop)
    );
    assert_eq!(
        ColAnimId(58).script(),
        Some(Script::FighterPikachuSpecialHi)
    );
    assert_eq!(ColAnimId::FIGHTER_NO_DAMAGE, ColAnimId(0xA));
    assert_eq!(ColAnimId::FIGHTER_STAR, ColAnimId(0x4A));
    assert_eq!(ColAnimId::SCREEN_FLASH_DEAD_EXPLODE, ColAnimId(81));
    let p = |id: ColAnimId| (id.priority(), id.is_unlocked());
    assert_eq!(p(ColAnimId::FIGHTER_HIT_STATUS_INTANGIBLE), (30, false));
    assert_eq!(p(ColAnimId::FIGHTER_DAMAGE_COMMON), (100, false));
    assert_eq!(p(ColAnimId::FIGHTER_COMMON_SPECIAL_N_CHARGE), (10, false));
    assert_eq!(p(ColAnimId::FIGHTER_NO_DAMAGE), (11, false));
    assert_eq!(p(ColAnimId::FIGHTER_SHIELD_BREAK_FLY), (60, true));
    assert_eq!(p(ColAnimId(0)), (0, false));
}

#[test]
fn every_script_runs_without_hanging() {
    for id in 0..DESCS.len() as u8 {
        let mut c = ColAnim::default();
        c.check_set(ColAnimId(id), 0);
        for _ in 0..2000 {
            if c.update() {
                break;
            }
            assert!(usize::from(c.script_id) <= STACK, "id {id}");
        }
    }
}

/// `dGMColScriptsFighterDamageFireWeak`: four loops of `Sub1`'s two
/// colours, then four of `Sub2`'s nested pair, then `End`.
#[test]
fn a_weak_fire_hit_runs_its_loops_and_subroutines() {
    let mut c = ColAnim::default();
    c.check_set(ColAnimId(ColAnimId::DAMAGE_FIRE_START), 0);
    let frames = run(&mut c, 25);
    let (a, b) = ([0xFF, 0xF0, 0x78, 0xAA], [0xDC, 0x6E, 0x1E, 0x96]);
    let (d, e) = ([0xB4, 0x64, 0x00, 0x64], [0x3C, 0x00, 0x00, 0xD2]);
    for (i, &(color, _, ended)) in frames.iter().take(24).enumerate() {
        let want = match (i < 8, i % 2) {
            (true, 0) => a,
            (true, _) => b,
            (false, 0) => d,
            (false, _) => e,
        };
        assert_eq!(color, Some(want), "frame {i}");
        assert!(!ended);
    }
    assert!(frames[24].2, "ends on the 25th frame");
}

/// The shared skeleton script: dark for 2 frames, the skeleton for 2, a
/// clear frame, twice; then blue and white frames.
#[test]
fn an_electric_hit_flashes_the_skeleton_then_blue_and_white() {
    let mut f = fighter(FighterKind::Mario);
    assert!(damage_element_colanim(&mut f, Element::Electric, 0));
    assert_eq!(
        f.colanim.id,
        ColAnimId::FIGHTER_DAMAGE_ELECTRIC_SKELETON_WEAK
    );
    let frames = run(&mut f.colanim, 19);
    let dark = Some([0x14, 0x14, 0x14, 0xFF]);
    let skel: Vec<_> = frames.iter().map(|&(c, s, _)| (c, s)).collect();
    for cycle in 0..2 {
        let at = cycle * 5;
        assert_eq!(
            skel[at..at + 5],
            [(dark, 0), (dark, 0), (None, 1), (None, 1), (None, 0)]
        );
    }
    let blue = Some([0x00, 0x00, 0x94, 0x5A]);
    let white = Some([0xFF, 0xFF, 0xFF, 0x96]);
    assert_eq!(skel[10..14], [(blue, 0), (white, 0), (blue, 0), (white, 0)]);
    assert!(frames[18].2);
    // Kirby's (id 24) shows skeleton 2 with the dark colour.
    let mut k = fighter(FighterKind::Kirby);
    damage_element_colanim(&mut k, Element::Electric, 2);
    assert_eq!(k.colanim.id, ColAnimId(0x18 + 2));
    let s: Vec<_> = run(&mut k.colanim, 5)
        .iter()
        .map(|&(c, s, _)| (c, s))
        .collect();
    assert_eq!(s, [(dark, 2), (dark, 2), (None, 1), (None, 1), (None, 0)]);
    // Samus takes 28; Master Hand's kinds the plain flash at 16.
    assert_eq!(skeleton_colanim_base(FighterKind::Samus), 0x1C);
    assert_eq!(skeleton_colanim_base(FighterKind::MetalMario), 0x10);
}

#[test]
fn a_damaging_hit_flashes_by_element_and_a_strong_one_flashes_the_screen() {
    for (element, want) in [
        (Element::Normal, ColAnimId::FIGHTER_DAMAGE_COMMON),
        (Element::Slash, ColAnimId::FIGHTER_DAMAGE_COMMON),
        (Element::Fire, ColAnimId::FIGHTER_DAMAGE_FIRE_FLY),
        // `DamageIceFly` is a bare `End`: the status's first frame ends it.
        (Element::Freezing, ColAnimId::NONE),
        (
            Element::Electric,
            ColAnimId::FIGHTER_DAMAGE_ELECTRIC_SKELETON_FLY,
        ),
    ] {
        let mut f = fighter(FighterKind::Mario);
        // 100 knockback: 53 frames of hitstun, level 3.
        crate::attack::init_damage_vars_full(&mut f, None, 10, 100.0, 45, 1.0, 1, element, false);
        assert_eq!(f.colanim.id, want, "{element:?}");
        assert_eq!(f.screen_flash, None);
    }
    // No damage, no flash.
    let mut f = fighter(FighterKind::Mario);
    crate::attack::init_damage_vars_full(&mut f, None, 0, 100.0, 45, 1.0, 1, Element::Fire, false);
    assert_eq!(f.colanim.id, ColAnimId::NONE);
    // Over 160 knockback, the screen flashes.
    let mut f = fighter(FighterKind::Mario);
    crate::attack::init_damage_vars_full(&mut f, None, 30, 161.0, 45, 1.0, 1, Element::Fire, false);
    assert_eq!(f.screen_flash, Some(ColAnimId::SCREEN_FLASH_DAMAGE_FIRE));
    assert_eq!(damage_screen_flash(160.0, Element::Normal), None);
    let mut ko = crate::ko::KoEffects::default();
    ko.observe(&mut f);
    assert_eq!(f.screen_flash, None);
    ko.tick();
    assert_eq!(ko.flash_color(), Some([0xFF, 0x8C, 0x78, 0x50 - 10]));
}

/// `DamageCommon` turns the light and flashes white, then ends into
/// `ftParamResetStatUpdateColAnim`.
#[test]
fn damage_common_ends_into_the_standing_states() {
    let mut f = fighter(FighterKind::Mario);
    check_set(&mut f, ColAnimId::FIGHTER_DAMAGE_COMMON, 0);
    run_update(&mut f);
    assert_eq!(
        (f.colanim.light, f.colanim.color()),
        (Some((90.0, 0.0)), None)
    );
    run_update(&mut f);
    assert_eq!(f.colanim.color(), Some([0xFF, 0xFF, 0xFF, 0xE6]));
    // An invincible fighter picks the flicker back up when it ends.
    f.invincible_frames = 5;
    for _ in 0..6 {
        run_update(&mut f);
    }
    assert_eq!(f.colanim.id, ColAnimId::FIGHTER_DAMAGE_COMMON);
    run_update(&mut f);
    assert_eq!(f.colanim.id, ColAnimId::FIGHTER_NO_DAMAGE);
    assert_eq!(f.colanim.color(), Some([0xFF, 0xFF, 0xFF, 0x80]));
}

/// A roll's `SetHitStatusAll(Intangible)` starts the white flicker, and
/// the status change back to normal ends it.
#[test]
fn a_roll_flickers_while_intangible() {
    let mut f = fighter(FighterKind::Mario);
    status::set_status(&mut f, Status::EscapeF, 0.0, StatusTiming::unknown());
    for _ in 0..5 {
        f.status.anim_frame += 1.0;
        crate::motion::advance(&mut f);
        run_update(&mut f);
    }
    assert_eq!(f.colanim.id, ColAnimId::FIGHTER_HIT_STATUS_INTANGIBLE);
    assert_eq!(f.colanim.color().map(|c| c[..3] == [0xFF; 3]), Some(true));
    // `HitStatusNormal` replaces the (locked) flicker, and the new status's
    // first frame runs it to its end.
    status::set_wait(&mut f);
    assert_eq!(f.colanim.id, ColAnimId::NONE);
    assert_eq!(f.colanim.color(), None);
}

/// The respawn's 120 frames of invincibility replace the glow with the
/// `NoDamage` flicker, which lasts through status changes and ends with
/// the timer.
#[test]
fn the_rebirth_invincibility_flickers_until_it_runs_out() {
    let mut f = fighter(FighterKind::Mario);
    check_set(&mut f, ColAnimId::FIGHTER_REBIRTH, 0);
    crate::reaction::set_timed_invincible(&mut f, 120);
    assert_eq!(f.colanim.id, ColAnimId::FIGHTER_NO_DAMAGE);
    status::set_fall(&mut f);
    assert_eq!(f.colanim.id, ColAnimId::FIGHTER_NO_DAMAGE);
    let mut seen = Vec::new();
    for _ in 0..120 {
        f.tick_timers();
        run_update_interrupt(&mut f);
        seen.push(f.colanim.id);
    }
    assert!(seen[..119]
        .iter()
        .all(|&id| id == ColAnimId::FIGHTER_NO_DAMAGE));
    assert_eq!(f.invincible_frames, 0);
    assert_eq!(seen[119], ColAnimId::NONE);
    assert_eq!(f.colanim.color(), None);
}

/// `SetColAnim` in a motion script: the Fireball's red flash, lit from the
/// front.
#[test]
fn mario_s_fireball_flashes_red_from_its_motion_script() {
    let mut f = fighter(FighterKind::Mario);
    status::set_mario_special_n(&mut f);
    let mut lit = None;
    for _ in 0..30 {
        f.status.anim_frame += 1.0;
        crate::motion::advance(&mut f);
        if f.colanim.id == ColAnimId::FIGHTER_MARIO_SPECIAL_N && lit.is_none() {
            run_update(&mut f);
            lit = Some((f.colanim.light, f.colanim.color()));
        }
    }
    assert_eq!(
        lit,
        Some((Some((75.0, -10.0)), Some([0xFF, 0x00, 0x00, 0xFF])))
    );
}

#[test]
fn map_switches_and_a_move_s_later_statuses_keep_the_animation() {
    use crate::status::{AnyStatus, FoxStatus, MarioStatus};
    let m = |s| AnyStatus::Mario(s);
    assert!(preserved(
        FighterKind::Mario,
        m(MarioStatus::SpecialN),
        m(MarioStatus::SpecialAirN)
    ));
    assert!(preserved(
        FighterKind::Luigi,
        m(MarioStatus::SpecialAirN),
        m(MarioStatus::SpecialN)
    ));
    assert!(!preserved(
        FighterKind::Mario,
        Status::Wait.into(),
        m(MarioStatus::SpecialN)
    ));
    assert!(preserved(
        FighterKind::Fox,
        AnyStatus::Fox(FoxStatus::SpecialLwStart),
        AnyStatus::Fox(FoxStatus::SpecialLwLoop)
    ));
    assert!(preserved(
        FighterKind::Ness,
        Status::ShieldBreakFly.into(),
        Status::ShieldBreakFall.into()
    ));
    assert!(!preserved(
        FighterKind::Ness,
        Status::Wait.into(),
        Status::Fall.into()
    ));
}

/// A full Giant Punch flashes, and the flash comes back after another
/// animation ends.
#[test]
fn a_full_charge_flashes_until_it_is_spent() {
    let mut f = fighter(FighterKind::Donkey);
    f.donkey_special_n.charge_level = 10;
    check_set(&mut f, ColAnimId::FIGHTER_DAMAGE_COMMON, 0);
    for _ in 0..12 {
        run_update(&mut f);
    }
    assert_eq!(f.colanim.id, ColAnimId::FIGHTER_COMMON_SPECIAL_N_CHARGE);
    let mut s = fighter(FighterKind::Samus);
    s.samus.charge_level = crate::samus::CHARGE_MAX - 1;
    reset_stat_update(&mut s);
    assert_eq!(s.colanim.id, ColAnimId::NONE);
    s.samus.charge_level = crate::samus::CHARGE_MAX;
    reset_stat_update(&mut s);
    assert_eq!(s.colanim.id, ColAnimId::FIGHTER_COMMON_SPECIAL_N_CHARGE);
}

#[test]
fn a_fast_fall_flashes_at_once() {
    let mut f = fighter(FighterKind::Mario);
    f.situation = crate::fighter::Situation::Air;
    f.physics.vel_air.y = -10.0;
    f.stick.y = -80;
    f.stick.tap_y = 1;
    status::check_set_fast_fall(&mut f);
    assert_eq!(f.colanim.id, ColAnimId::FIGHTER_FAST_FALL);
    assert_eq!(f.colanim.color(), Some([0x00, 0x00, 0x00, 0x80]));
}
