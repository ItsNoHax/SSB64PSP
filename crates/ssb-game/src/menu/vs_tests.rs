use super::vs_item_switch::{self, kind, VsItemSwitchMenu, ROWS};
use super::vs_options::{self, VsOption, VsOptionsMenu};
use super::*;
use crate::backup::Backup;
use crate::item::normal::Appearance;
use crate::players_vs::{BattleState, Handicap};
use crate::spgame::Unlock;

fn tap(buttons: u16) -> Pad {
    Pad {
        hold: buttons,
        tap: buttons,
        ..Pad::default()
    }
}

fn hold(buttons: u16) -> Pad {
    Pad {
        hold: buttons,
        ..Pad::default()
    }
}

fn idle() -> Pad {
    Pad::default()
}

fn backup(item_switch: bool) -> Backup {
    let mut b = Backup::default();
    if item_switch {
        b.unlock_mask |= Unlock::ItemSwitch.mask();
    }
    b
}

fn options(item_switch: bool, st: &BattleState) -> VsOptionsMenu {
    let mut m = VsOptionsMenu::new(Scene::VsMode, st, &backup(item_switch));
    let mut s = *st;
    for _ in 0..9 {
        assert_eq!(m.tick(&idle(), &mut s), None);
    }
    m
}

fn pieces(visit: impl FnOnce(&mut dyn FnMut(Draw))) -> Vec<Piece> {
    let mut out = Vec::new();
    visit(&mut |d| match d {
        Draw::Sprite(p) => out.push(p),
        Draw::Tiled { piece, .. } => out.push(piece),
        _ => {}
    });
    out
}

fn fills(visit: impl FnOnce(&mut dyn FnMut(Draw))) -> Vec<([f32; 4], [u8; 4])> {
    let mut out = Vec::new();
    visit(&mut |d| {
        if let Draw::Fill { rect, rgba } = d {
            out.push((rect, rgba));
        }
    });
    out
}

#[test]
fn item_switch_needs_its_unlock() {
    let st = BattleState::default();
    let m = options(false, &st);
    assert!(!m.is_have_item_switch);
    let mut m2 = m;
    let mut s = st;
    // Up from Handicap wraps to Damage.
    m2.tick(&tap(UP), &mut s);
    assert_eq!(m2.option, VsOption::Damage);
    let m = options(true, &st);
    assert!(m.is_have_item_switch);
    let mut m2 = m;
    m2.tick(&tap(UP), &mut s);
    assert_eq!(m2.option, VsOption::ItemSwitch);
    // The rows sit lower without it.
    let ys = |m: &VsOptionsMenu| {
        pieces(|f| m.visit(&mut |d| f(d)))
            .iter()
            .filter(|p| p.offset == vs_options::sprite::BUBBLE)
            .map(|p| p.y)
            .collect::<Vec<_>>()
    };
    assert_eq!(ys(&options(false, &st)), [65.0, 97.0, 129.0, 161.0]);
    assert_eq!(ys(&options(true, &st)), [61.0, 90.0, 119.0, 148.0, 177.0]);
}

#[test]
fn coming_back_from_item_switch_highlights_it() {
    let st = BattleState::default();
    let m = VsOptionsMenu::new(Scene::VsItemSwitch, &st, &backup(true));
    assert_eq!(m.option, VsOption::ItemSwitch);
}

#[test]
fn down_wraps_and_waits_eight_more_at_the_end() {
    let st = BattleState::default();
    let mut m = options(false, &st);
    let mut s = st;
    for want in [VsOption::TeamAttack, VsOption::StageSelect] {
        m.tick(&tap(DOWN), &mut s);
        assert_eq!(m.option, want);
        m.tick(&idle(), &mut s);
    }
    // Held down onto the last option: 12 + 8 ticks before the wrap.
    m.tick(&hold(DOWN), &mut s);
    assert_eq!(m.option, VsOption::Damage);
    for _ in 0..19 {
        m.tick(&hold(DOWN), &mut s);
    }
    assert_eq!(m.option, VsOption::Damage);
    m.tick(&hold(DOWN), &mut s);
    assert_eq!(m.option, VsOption::Handicap);
}

#[test]
fn the_handicap_steps_left_and_right_and_sets_every_player() {
    let st = BattleState::default();
    let mut m = options(false, &st);
    let mut s = st;
    // Left from Off: Auto, then On; right walks back.
    m.tick(&tap(LEFT), &mut s);
    assert_eq!(m.handicap, Handicap::Auto);
    assert!(s.players.iter().all(|p| p.handicap == 5));
    m.tick(&idle(), &mut s);
    m.tick(&tap(LEFT), &mut s);
    assert_eq!(m.handicap, Handicap::On);
    assert!(s.players.iter().all(|p| p.handicap == 9));
    m.tick(&idle(), &mut s);
    m.tick(&tap(LEFT), &mut s);
    assert_eq!(m.handicap, Handicap::On, "no wrap");
    m.tick(&idle(), &mut s);
    m.tick(&tap(RIGHT), &mut s);
    assert_eq!(m.handicap, Handicap::Auto);
    // A cycles Off -> On -> Auto -> Off.
    let mut m = options(false, &st);
    for want in [Handicap::On, Handicap::Auto, Handicap::Off] {
        m.tick(&tap(N64Buttons::A), &mut s);
        assert_eq!(m.handicap, want);
    }
    // The underline sits under the setting.
    m.handicap = Handicap::On;
    let f = fills(|f| m.visit(&mut |d| f(d)));
    assert_eq!(
        f.last().unwrap(),
        &([190.0, 81.0, 217.0, 82.0], [0xFF, 0x00, 0x28, 0xFF])
    );
}

#[test]
fn team_attack_and_stage_select_toggle() {
    let st = BattleState::default();
    let mut m = options(true, &st);
    let mut s = st;
    m.tick(&tap(DOWN), &mut s);
    m.tick(&idle(), &mut s);
    assert_eq!(m.option, VsOption::TeamAttack);
    m.tick(&tap(LEFT), &mut s);
    assert!(m.team_attack);
    m.tick(&tap(N64Buttons::A), &mut s);
    assert!(!m.team_attack);
    m.tick(&tap(DOWN), &mut s);
    m.tick(&idle(), &mut s);
    m.tick(&tap(RIGHT), &mut s);
    assert!(!m.stage_select);
    // Not written until the scene is left.
    assert!(s.is_stage_select);
    assert_eq!(m.tick(&tap(N64Buttons::B), &mut s), Some(Scene::VsMode));
    assert!(!s.is_stage_select && !s.is_team_attack);
}

#[test]
fn the_damage_ratio_wraps_from_fifty_to_two_hundred() {
    let st = BattleState::default();
    let mut m = options(false, &st);
    let mut s = st;
    m.option = VsOption::Damage;
    m.damage = 51;
    m.tick(&tap(LEFT), &mut s);
    assert_eq!(m.damage, 50);
    m.tick(&idle(), &mut s);
    m.tick(&tap(LEFT), &mut s);
    assert_eq!(m.damage, 200);
    m.tick(&idle(), &mut s);
    m.tick(&tap(RIGHT), &mut s);
    assert_eq!(m.damage, 50);
    // A held stick steps by `(160 - x) / 14` ticks.
    let stick = Pad {
        stick_x: 90,
        ..Pad::default()
    };
    m.tick(&idle(), &mut s);
    m.tick(&stick, &mut s);
    assert_eq!(m.damage, 51);
    for _ in 0..4 {
        m.tick(&stick, &mut s);
    }
    assert_eq!(m.damage, 51);
    m.tick(&stick, &mut s);
    assert_eq!(m.damage, 52);
    assert_eq!(m.tick(&tap(N64Buttons::B), &mut s), Some(Scene::VsMode));
    assert_eq!(s.damage_ratio, 52);
    // Three red digits, right-aligned at 220.
    let p = pieces(|f| m.visit(&mut |d| f(d)));
    let d: Vec<_> = p
        .iter()
        .filter(|p| common::DIGITS.contains(&p.offset))
        .map(|p| (p.offset, p.x))
        .collect();
    assert_eq!(d, [(common::DIGITS[2], 209.0), (common::DIGITS[5], 198.0)]);
}

#[test]
fn a_on_item_switch_loads_it_with_the_settings_written() {
    let st = BattleState::default();
    let mut m = options(true, &st);
    let mut s = st;
    m.option = VsOption::ItemSwitch;
    m.handicap = Handicap::Off;
    s.players[0].handicap = 3;
    assert_eq!(
        m.tick(&tap(N64Buttons::A), &mut s),
        Some(Scene::VsItemSwitch)
    );
    // Off resets every player's handicap.
    assert_eq!(s.players[0].handicap, 9);
    // A elsewhere does not leave.
    let mut m = options(true, &st);
    assert_eq!(m.tick(&tap(N64Buttons::START), &mut s), None);
}

#[test]
fn five_idle_minutes_return_to_the_title() {
    let st = BattleState::default();
    let mut m = options(false, &st);
    let mut s = st;
    m.damage = 120;
    let mut last = None;
    for _ in 0..IDLE_RETURN {
        last = m.tick(&idle(), &mut s);
        if last.is_some() {
            break;
        }
    }
    assert_eq!(last, Some(Scene::Title));
    assert_eq!(s.damage_ratio, 120);
}

fn item_switch(st: &BattleState) -> VsItemSwitchMenu {
    let mut m = VsItemSwitchMenu::new(st);
    let mut s = *st;
    for _ in 0..9 {
        assert_eq!(m.tick(&idle(), &mut s), None);
    }
    m
}

#[test]
fn the_item_switch_reads_and_writes_the_toggles() {
    let st = BattleState {
        item_toggles: !(1 << kind::BAT),
        ..BattleState::default()
    };
    let m = item_switch(&st);
    assert_eq!(m.statuses[0], Appearance::Middle as u8);
    assert_eq!(m.statuses[2], 0, "Home-Run Bat");
    assert!(m.statuses.iter().skip(1).filter(|&&s| s == 0).count() == 1);
    // Turn the Green Shell off: the Red Shell goes with it.
    let mut m = m;
    let mut s = st;
    m.select = 8;
    m.tick(&tap(RIGHT), &mut s);
    assert_eq!(m.statuses[8], 0);
    assert_eq!(m.tick(&tap(N64Buttons::B), &mut s), Some(Scene::VsOptions));
    assert_eq!(s.item_toggles & (1 << kind::G_SHELL), 0);
    assert_eq!(s.item_toggles & (1 << kind::R_SHELL), 0);
    assert_eq!(s.item_toggles & (1 << kind::BAT), 0);
    assert_ne!(s.item_toggles & (1 << kind::SWORD), 0);
    assert_eq!(s.item_toggles & 0xF, 0xF, "containers");
}

#[test]
fn every_toggle_off_clears_the_mask_and_keeps_the_old_rate() {
    let st = BattleState::default();
    let mut m = item_switch(&st);
    let mut s = st;
    for i in 1..ROWS {
        m.statuses[i] = 0;
    }
    m.tick(&tap(N64Buttons::A), &mut s);
    assert_eq!(m.appearance(), Appearance::High);
    m.tick(&tap(N64Buttons::B), &mut s);
    assert_eq!(s.item_toggles, 0);
    // `mnVSItemSwitchSetItemSettings` (which writes the rate) is skipped.
    assert_eq!(s.item_appearance, Appearance::Middle);
}

#[test]
fn the_item_switch_cursor_wraps_and_the_rate_cycles() {
    let st = BattleState::default();
    let mut m = item_switch(&st);
    let mut s = st;
    m.tick(&tap(UP), &mut s);
    assert_eq!(m.select, ROWS - 1);
    m.tick(&idle(), &mut s);
    m.tick(&tap(DOWN), &mut s);
    assert_eq!(m.select, 0);
    m.tick(&idle(), &mut s);
    // Left from None wraps to Very High; right from Very High to None.
    m.statuses[0] = 0;
    m.tick(&tap(LEFT), &mut s);
    assert_eq!(m.appearance(), Appearance::VeryHigh);
    m.tick(&idle(), &mut s);
    m.tick(&tap(RIGHT), &mut s);
    assert_eq!(m.appearance(), Appearance::None);
    // Left on an off toggle turns it on; on an on toggle, nothing.
    m.select = 3;
    m.statuses[3] = 0;
    m.tick(&idle(), &mut s);
    m.tick(&tap(LEFT), &mut s);
    assert_eq!(m.statuses[3], 1);
    m.tick(&idle(), &mut s);
    m.tick(&tap(LEFT), &mut s);
    assert_eq!(m.statuses[3], 1);
    // The cursor follows the row.
    let p = pieces(|f| m.visit(&mut |d| f(d)));
    let cursor = p
        .iter()
        .find(|p| p.offset == vs_item_switch::sprite::CURSOR)
        .unwrap();
    assert_eq!((cursor.x, cursor.y), (115.0, 81.0));
    // The rate redrawn after a change follows the toggles.
    let last_switch = p
        .iter()
        .rposition(|p| p.offset == vs_item_switch::sprite::TOGGLE_SLASH)
        .unwrap();
    let rate = p
        .iter()
        .position(|p| vs_item_switch::sprite::APPEARANCE.contains(&p.offset))
        .unwrap();
    assert!(rate > last_switch);
}

#[test]
fn the_item_switch_never_returns_to_the_title() {
    let st = BattleState::default();
    let mut m = item_switch(&st);
    let mut s = st;
    for _ in 0..IDLE_RETURN + 10 {
        assert_eq!(m.tick(&idle(), &mut s), None);
    }
}
