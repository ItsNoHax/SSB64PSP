use super::super::{BattleState, Handicap, Pad, PlayerKind, PlayersVs, SceneContext, PLAYERS};
use super::*;
use crate::fighter::FighterKind;
use crate::fighter_select::portrait_center;
use ssb_engine::input::{ControllerState, N64Buttons};

const ONE_CONTROLLER: [bool; PLAYERS] = [true, false, false, false];

fn tap_pad(button: u16) -> [Pad; PLAYERS] {
    let p = Pad {
        state: ControllerState {
            buttons: N64Buttons(button),
            stick_x: 0,
            stick_y: 0,
            connected: true,
        },
        taps: N64Buttons(button),
    };
    [p, Pad::default(), Pad::default(), Pad::default()]
}

fn idle(s: &mut PlayersVs) {
    s.tick(&tap_pad(0), &mut || 0);
}

fn tap(s: &mut PlayersVs, button: u16) {
    s.tick(&tap_pad(button), &mut || 0);
}

fn first_visit(state: BattleState) -> PlayersVs {
    PlayersVs::new(state, SceneContext::default(), ONE_CONTROLLER)
}

fn cursor_at(s: &mut PlayersVs, x: f32, y: f32) {
    s.slots[0].cursor = Some((x, y));
    idle(s);
}

fn hover(s: &mut PlayersVs, kind: FighterKind) {
    let (px, py) = portrait_center(kind);
    cursor_at(s, px - 11.0, py + 14.0);
}

fn toggle_kind(s: &mut PlayersVs, sel: usize) {
    cursor_at(s, sel as f32 * 69.0 + 52.0, 130.0);
    tap(s, N64Buttons::A);
}

fn draws(s: &PlayersVs) -> Vec<Draw> {
    let mut out = Vec::new();
    s.visit(|d| out.push(d));
    out
}

fn sprites_at(s: &PlayersVs, file: u32, offset: u32) -> Vec<Piece> {
    draws(s)
        .into_iter()
        .filter_map(|d| match d {
            Draw::Sprite(p) if p.file == file && p.offset == offset => Some(p),
            _ => None,
        })
        .collect()
}

#[test]
fn the_portraits_slide_in_from_both_sides() {
    // `mnPlayersVSGetNextPortraitX` from the start positions.
    assert_eq!(next_portrait_x(0, -35.0), Some(-33.1));
    assert_eq!(next_portrait_x(0, 24.0), Some(25.0));
    assert_eq!(next_portrait_x(0, 25.0), None);
    assert_eq!(next_portrait_x(3, 310.0), Some(302.2));
    assert_eq!(next_portrait_x(3, 165.0), Some(160.0));
    let mut s = first_visit(BattleState::default());
    assert_eq!(s.view.portrait_x[0], -35.0);
    assert_eq!(s.view.portrait_x[5], 310.0);
    let mut arrived = [0; 12];
    for tick in 1..=40 {
        idle(&mut s);
        for (i, x) in s.view.portrait_x.iter().enumerate() {
            if arrived[i] == 0 && *x == [25.0, 70.0, 115.0, 160.0, 205.0, 250.0][i % 6] {
                arrived[i] = tick;
            }
        }
    }
    // All start 60 to 150 away; the fast centre ones take 20 ticks and the
    // slow outer ones the longest.
    assert_eq!(arrived[2], 20);
    assert_eq!(arrived[3], 20);
    assert_eq!(arrived[1], 27);
    assert_eq!(arrived[0], 32);
    assert_eq!(arrived[5], 34);
    assert!(arrived.iter().all(|&t| t > 0));
}

#[test]
fn locked_portraits_draw_their_shadow_and_question_mark() {
    let s = first_visit(BattleState::default());
    // No save data: Luigi, Captain Falcon, Ness and Jigglypuff are locked.
    let shadows: Vec<u32> = draws(&s)
        .into_iter()
        .filter_map(|d| match d {
            Draw::Shadow(p) => Some(p.offset),
            _ => None,
        })
        .collect();
    assert_eq!(shadows, [0x20538, 0x1E2E8, 0x22788, 0x249D8]);
    let marks = sprites_at(&s, FILE_PORTRAITS, 0xF68);
    assert_eq!(marks.len(), 4);
    assert_eq!(marks[0].prim, Some([0xC4, 0xB9, 0xA9]));
    assert_eq!(marks[0].env, [0x5B, 0x41, 0x33]);
    // Every fire background is opaque; eight unlocked portraits draw.
    let bgs = sprites_at(&s, FILE_PORTRAITS, 0x24D0);
    assert_eq!(bgs.len(), 12);
    assert!(bgs.iter().all(|p| !p.transparent));
    let unlocked = PORTRAIT_SPRITES
        .iter()
        .filter(|&&o| !sprites_at(&s, FILE_PORTRAITS, o).is_empty());
    assert_eq!(unlocked.count(), 8);
}

#[test]
fn the_shutters_open_for_a_player_and_close_for_na() {
    let mut s = first_visit(BattleState::default());
    assert!(s.view.slots.iter().all(|v| v.door_offset == DOOR_CLOSED));
    for _ in 0..20 {
        idle(&mut s);
    }
    assert_eq!(s.view.slots[0].door_offset, 1);
    idle(&mut s);
    assert_eq!(s.view.slots[0].door_offset, 0);
    assert_eq!(s.view.slots[1].door_offset, DOOR_CLOSED);
    // A CPU in port 2 opens its doors; NA closes them again.
    toggle_kind(&mut s, 1);
    assert_eq!(s.view.slots[1].door_offset, 39);
    for _ in 0..20 {
        idle(&mut s);
    }
    assert_eq!(s.view.slots[1].door_offset, 0);
    cursor_at(&mut s, 69.0 + 52.0, 130.0);
    tap(&mut s, N64Buttons::A);
    assert_eq!(s.slots[1].pkind, PlayerKind::Not);
    assert_eq!(s.view.slots[1].door_offset, 2);
    // Each door is drawn inside its panel's scissor.
    let d = draws(&s);
    let at = d
        .iter()
        .position(|d| *d == Draw::Scissor([91.0, 126.0, 157.0, 217.0]))
        .unwrap();
    match (d[at + 1], d[at + 2], d[at + 3]) {
        (Draw::Sprite(l), Draw::Sprite(r), Draw::Scissor(VIEWPORT)) => {
            assert_eq!((l.x, r.x), (69.0 - 19.0 + 2.0, 69.0 + 88.0 - 2.0));
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn the_puck_glow_rises_and_falls_by_nine() {
    let mut s = first_visit(BattleState::default());
    let mut glow = Vec::new();
    for _ in 0..60 {
        idle(&mut s);
        glow.push(s.view.glow);
    }
    assert_eq!(&glow[..3], &[9, 18, 27]);
    // 28 steps reach 252; the 29th passes 0xFF and turns down at once.
    assert_eq!(glow[27], 252);
    assert_eq!(glow[28], 0xFF - 9);
    // Down to 0x80, which then climbs again.
    let low = glow.iter().position(|&g| g == 0x80).unwrap();
    assert_eq!(low, 42);
    assert_eq!(glow[low + 1], 0x80 + 9);
}

#[test]
fn a_cpu_opened_gets_its_level_arrows_and_a_flash() {
    let mut s = first_visit(BattleState::default());
    toggle_kind(&mut s, 2);
    let v = &s.view.slots[2];
    assert_eq!(v.level, Some(false), "CP Level");
    assert_eq!(v.value, Some(3));
    let a = v.arrows.unwrap();
    assert!(a.left && a.right && !a.hidden);
    // The random draw is Mario: his portrait flashes.
    let flash = v.flash.unwrap();
    assert_eq!(flash.portrait, 1);
    assert!(flash.hidden, "the first run hides it");
    assert_eq!(v.name_made, Some((FighterKind::Mario, false)));
    assert!(v.name_shown && v.name_hidden, "the level covers the name");
    // The gate: a CPU's LUT for port 3.
    assert_eq!(v.gate_lut, 4 + 2);
    assert!(v.kind_text_com);
    let mut shown = Vec::new();
    for _ in 0..16 {
        idle(&mut s);
        shown.push(s.view.slots[2].flash.map(|f| !f.hidden));
    }
    assert_eq!(shown[0], Some(true));
    assert_eq!(shown[1], Some(false));
    // Shown on runs 2, 4, ... 14, gone on the sixteenth.
    assert_eq!(shown[12], Some(true));
    assert_eq!(shown[13], Some(false));
    assert_eq!(shown[14], None);
    // The arrows blink every ten runs.
    let blink: Vec<bool> = (0..20)
        .map(|_| {
            idle(&mut s);
            s.view.slots[2].arrows.unwrap().hidden
        })
        .collect();
    assert_eq!(blink.iter().filter(|&&h| h).count(), 10);
}

#[test]
fn the_level_arrows_leave_at_the_ends_and_the_value_follows() {
    let mut s = first_visit(BattleState::default());
    toggle_kind(&mut s, 2);
    // `mnPlayersVSCheckHandicapArrowLInRange`: x + 21 to x + 43 at the
    // cursor's x + 20.
    let x = 2.0 * 69.0 + 5.0;
    cursor_at(&mut s, x, 200.0);
    tap(&mut s, N64Buttons::A);
    tap(&mut s, N64Buttons::A);
    assert_eq!(s.slots[2].cpu_level, 1);
    assert_eq!(s.view.slots[2].value, Some(1));
    let a = s.view.slots[2].arrows.unwrap();
    assert!(!a.left && a.right);
    let digits = sprites_at(&s, FILE_MN_COMMON, 0xD3E0);
    assert_eq!((digits[0].x, digits[0].y), (2.0 * 69.0 + 67.0, 200.0));
}

#[test]
fn a_human_placed_with_handicap_off_shows_no_level() {
    let mut s = first_visit(BattleState::default());
    hover(&mut s, FighterKind::Yoshi);
    let v = &s.view.slots[0];
    assert_eq!(v.name_made, Some((FighterKind::Yoshi, true)));
    assert!(v.name_shown && !v.name_hidden);
    tap(&mut s, N64Buttons::A);
    let v = &s.view.slots[0];
    assert_eq!(v.level, None);
    assert_eq!(v.flash.map(|f| f.portrait), Some(7));
    // The emblem in the human's grey and the name under it.
    let emblem = sprites_at(&s, FILE_EMBLEMS, 0x2C58);
    assert_eq!(
        (emblem[0].x, emblem[0].y, emblem[0].prim),
        (24.0, 143.0, Some([0x1E; 3]))
    );
    assert_eq!(sprites_at(&s, FILE_PLAYERS_COMMON, 0x2ED8)[0].y, 201.0);
}

#[test]
fn with_handicap_on_a_placed_human_shows_its_handicap() {
    let state = BattleState {
        handicap: Handicap::On,
        ..BattleState::default()
    };
    let mut s = first_visit(state);
    hover(&mut s, FighterKind::Fox);
    tap(&mut s, N64Buttons::A);
    let v = &s.view.slots[0];
    assert_eq!(v.level, Some(true), "Handicap");
    assert_eq!(v.value, Some(9));
    assert!(v.name_hidden);
    assert!(!v.arrows.unwrap().right, "no right arrow at 9");
    // Picking the puck back up (30 ticks on) takes the level down and shows
    // the name.
    for _ in 0..30 {
        idle(&mut s);
    }
    tap(&mut s, N64Buttons::A);
    let v = &s.view.slots[0];
    assert!(s.slots[0].held.is_some());
    assert_eq!((v.level, v.arrows, v.value), (None, None, None));
    assert!(!v.name_hidden);
    // Auto handicap has no arrows for a human.
    let state = BattleState {
        handicap: Handicap::Auto,
        ..BattleState::default()
    };
    let mut s = first_visit(state);
    hover(&mut s, FighterKind::Fox);
    tap(&mut s, N64Buttons::A);
    assert_eq!(s.view.slots[0].level, Some(true));
    assert!(s.view.slots[0].arrows.is_none());
}

#[test]
fn the_fighter_turns_until_placed_then_plays_its_selected_status() {
    let mut s = first_visit(BattleState::default());
    hover(&mut s, FighterKind::Yoshi);
    let f = s.view.slots[0].fighter.unwrap();
    assert_eq!((f.kind, f.status), (FighterKind::Yoshi, None));
    assert!((f.rotate_y - dtor(2.0)).abs() < 1e-6);
    for _ in 0..9 {
        idle(&mut s);
    }
    let f = s.view.slots[0].fighter.unwrap();
    assert!((f.rotate_y - dtor(20.0)).abs() < 1e-4);
    let serial = f.serial;
    tap(&mut s, N64Buttons::A);
    // The tap's tick turns it to 40; then 20 a tick until past 360, which
    // 360 itself is not: 17 more.
    let mut ticks = 1;
    while s.view.slots[0].fighter.unwrap().status.is_none() {
        idle(&mut s);
        ticks += 1;
    }
    assert_eq!(ticks, 18);
    let f = s.view.slots[0].fighter.unwrap();
    assert_eq!(f.rotate_y, 0.0);
    assert_eq!(f.status, Some(DemoStatus::Win2));
    assert_eq!(f.serial, serial + 1);
    assert_eq!(Fighter::position(0), [-1250.0, -850.0, 0.0]);
    assert_eq!(Fighter::position(3), [1270.0, -850.0, 0.0]);
    // A CPU opened with the fighter already facing front plays at once.
    toggle_kind(&mut s, 1);
    let f = s.view.slots[1].fighter.unwrap();
    assert_eq!(f.status, Some(status_selected(FighterKind::Mario)));
    assert_eq!(f.status, Some(DemoStatus::Win3));
    // NA hides it.
    cursor_at(&mut s, 69.0 + 52.0, 130.0);
    tap(&mut s, N64Buttons::A);
    assert!(s.view.slots[1].fighter.unwrap().hidden);
}

#[test]
fn the_ready_banner_blinks_thirty_of_forty_counts() {
    let mut s = first_visit(BattleState::default());
    hover(&mut s, FighterKind::Yoshi);
    tap(&mut s, N64Buttons::A);
    assert!(!s.view.ready_banner && !s.view.ready_press);
    toggle_kind(&mut s, 1);
    assert!(s.is_ready());
    // Two GObjs step one counter: each tick counts twice.
    let mut banner = Vec::new();
    let mut press = Vec::new();
    for _ in 0..40 {
        idle(&mut s);
        banner.push(s.view.ready_banner);
        press.push(s.view.ready_press);
    }
    assert_eq!(banner.iter().filter(|&&b| b).count(), 30);
    assert_eq!(press.iter().filter(|&&b| b).count(), 30);
    let d = draws(&s);
    assert!(
        s.view.ready_banner
            || !d.iter().any(|d| matches!(
                d,
                Draw::Tiled {
                    size: [320.0, 17.0],
                    ..
                }
            ))
    );
}

#[test]
fn the_rule_number_is_right_aligned_on_its_digits() {
    let mut xs = Vec::new();
    rule_number(15, 212.0, 23.0, 2, &DARK_DIGIT_WIDTHS, |p| {
        xs.push((p.offset, p.x))
    });
    // The 5 (11 wide) ends at 212, the 1 (7 wide) left of it.
    assert_eq!(xs, [(0x5888, 201.0), (0x5440, 194.0)]);
    xs.clear();
    rule_number(3, 208.0, 23.0, 2, &DARK_DIGIT_WIDTHS, |p| {
        xs.push((p.offset, p.x))
    });
    assert_eq!(xs, [(0x5668, 198.0)]);
    // A stock rule shows the stocks plus one.
    let state = BattleState {
        rule: crate::battle::Rule::Stock,
        stocks: 2,
        ..BattleState::default()
    };
    let s = first_visit(state);
    assert_eq!(sprites_at(&s, FILE_PLAYERS_COMMON, 0x5668)[0].x, 200.0);
    assert_eq!(sprites_at(&s, FILE_PLAYERS_COMMON, 0x5270).len(), 1);
    // An infinite time is the infinity sign at (194, 24).
    let state = BattleState {
        time_limit: TIMELIMIT_INFINITE,
        ..BattleState::default()
    };
    let s = first_visit(state);
    let inf = sprites_at(&s, FILE_PLAYERS_COMMON, 0x3EF0);
    assert_eq!(
        (inf[0].x, inf[0].y, inf[0].env),
        (194.0, 24.0, [0x32, 0x1C, 0x0E])
    );
}

#[test]
fn team_battle_swaps_the_label_and_adds_team_buttons() {
    let mut s = first_visit(BattleState::default());
    let label = |s: &PlayersVs| {
        sprites_at(s, FILE_GAME_MODES, 0x280).len()
            + 2 * sprites_at(s, FILE_GAME_MODES, 0x4E0).len()
    };
    assert_eq!(label(&s), 1);
    toggle_kind(&mut s, 2);
    cursor_at(&mut s, 40.0, 20.0);
    tap(&mut s, N64Buttons::A);
    assert!(s.is_team_battle);
    assert_eq!(label(&s), 2);
    let mode = sprites_at(&s, FILE_GAME_MODES, 0x4E0)[0];
    assert_eq!(mode.prim, Some([0x61, 0xAD, 0x49]));
    // Every slot gets a team button; port 3 starts blue.
    let red = sprites_at(&s, FILE_PLAYERS_COMMON, 0xE3C8);
    let blue = sprites_at(&s, FILE_PLAYERS_COMMON, 0xEC08);
    assert_eq!((red.len(), blue.len()), (2, 2));
    assert_eq!(blue[0].x, 2.0 * 69.0 + 34.0);
    // The gates take the team colour: red for port 1's human, blue for the
    // CPU.
    assert_eq!(s.view.slots[0].gate_lut, 0);
    assert_eq!(s.view.slots[2].gate_lut, 4 + 1);
}

#[test]
fn pucks_draw_before_cursors_and_the_hand_follows_its_status() {
    let mut s = first_visit(BattleState::default());
    for _ in 0..30 {
        idle(&mut s);
    }
    toggle_kind(&mut s, 2);
    let d = draws(&s);
    let kinds: Vec<&str> = d
        .iter()
        .filter_map(|d| match d {
            Draw::Puck { .. } => Some("puck"),
            Draw::Sprite(p) if common::CURSORS.contains(&p.offset) => Some("hand"),
            _ => None,
        })
        .collect();
    // The CPU's placed puck on link 33, then the cursor on link 32. The
    // human's held puck hides while its cursor points.
    assert_eq!(kinds, ["puck", "hand"]);
    match d.iter().find(|d| matches!(d, Draw::Puck { .. })) {
        Some(Draw::Puck { piece, glow }) => {
            assert_eq!(piece.offset, common::PUCKS[4], "CP puck");
            assert_eq!(i32::from(*glow), s.view.glow);
        }
        _ => unreachable!(),
    }
    // Over the HMN/CP/NA button the hand points; its number sits at (7, 15).
    let hand = sprites_at(&s, FILE_PLAYERS_COMMON, common::CURSORS[0]);
    let num = sprites_at(&s, FILE_PLAYERS_COMMON, common::TEXT_GRADIENT[0]);
    assert_eq!((num[0].x - hand[0].x, num[0].y - hand[0].y), (7.0, 15.0));
    assert_eq!(
        (num[0].prim, num[0].env),
        (Some([0xE0, 0x15, 0x15]), [0x5B, 0, 0])
    );
}

#[test]
fn the_draw_order_follows_the_cameras() {
    let mut s = first_visit(BattleState::default());
    hover(&mut s, FighterKind::Yoshi);
    let d = draws(&s);
    assert_eq!(d[0], Draw::Scissor(VIEWPORT));
    assert!(matches!(d[1], Draw::Tiled { size: [300.0, 220.0], piece } if !piece.transparent));
    let index = |pred: &dyn Fn(&Draw) -> bool| d.iter().position(pred).unwrap();
    let portrait =
        index(&|d| matches!(d, Draw::Sprite(p) if p.file == FILE_PORTRAITS && p.offset == 0x13758));
    let card = index(&|d| matches!(d, Draw::Sprite(p) if p.lut.is_some()));
    let door = index(&|d| matches!(d, Draw::Sprite(p) if p.offset == common::DOOR_LEFT));
    let button = index(&|d| matches!(d, Draw::Sprite(p) if p.offset == common::KIND_LABELS[0]));
    let fighters = index(&|d| *d == Draw::Fighters);
    let hand = index(&|d| matches!(d, Draw::Sprite(p) if common::CURSORS.contains(&p.offset)));
    assert!(
        portrait < card && card < door && door < button && button < fighters && fighters < hand
    );
    // NA slots show the NA button; the human's text is "1P" in black.
    let na = sprites_at(&s, FILE_PLAYERS_COMMON, common::KIND_LABELS[2]);
    assert_eq!(na.len(), 3);
    let text = sprites_at(&s, FILE_PLAYERS_COMMON, common::TEXT_1P[0]);
    assert_eq!(
        (text[0].x, text[0].y, text[0].prim),
        (30.0, 131.0, Some([0; 3]))
    );
}
