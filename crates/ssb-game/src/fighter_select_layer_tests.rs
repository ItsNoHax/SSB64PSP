use super::*;
use crate::fighter_select::{portrait_center, CursorStatus, SceneData};
use crate::players_vs::layer::{common, FILE_PORTRAITS, FILE_SELECT_COMMON};
use crate::results_scene::DemoStatus;
use ssb_engine::input::{ControllerState, N64Buttons};

fn pad(buttons: u16) -> ControllerState {
    ControllerState {
        buttons: N64Buttons(buttons),
        stick_x: 0,
        stick_y: 0,
        connected: true,
    }
}

fn idle(s: &mut FighterSelect) {
    s.tick(pad(0), N64Buttons(0));
}

fn tap(s: &mut FighterSelect, button: u16) {
    s.tick(pad(button), N64Buttons(button));
}

/// A first visit with the CPU on Mario (a time byte of 0).
fn first_visit() -> FighterSelect {
    FighterSelect::new(SceneData::default(), 0, false, || 0)
}

fn hover(s: &mut FighterSelect, kind: FighterKind) {
    let (px, py) = portrait_center(kind);
    s.slots[MAN].cursor = (px - 11.0, py + 14.0);
    idle(s);
}

fn draws(s: &FighterSelect) -> Vec<Draw> {
    let mut v = Vec::new();
    s.visit(|d| v.push(d));
    v
}

fn sprites_at(s: &FighterSelect, file: u32, offset: u32) -> Vec<Piece> {
    draws(s)
        .into_iter()
        .filter_map(|d| match d {
            Draw::Sprite(p) | Draw::Shadow(p) if p.file == file && p.offset == offset => Some(p),
            _ => None,
        })
        .collect()
}

#[test]
fn the_top_bar_is_the_stone_training_mode_and_back() {
    let s = first_visit();
    let d = draws(&s);
    assert_eq!(d[0], Draw::Scissor(VIEWPORT));
    match d[1] {
        Draw::Tiled { piece, size } => {
            assert_eq!((piece.file, piece.offset), (FILE_SELECT_COMMON, 0x440));
            assert_eq!((piece.x, piece.y, size), (10.0, 10.0, [300.0, 220.0]));
            assert!(!piece.transparent);
        }
        other => panic!("{other:?}"),
    }
    let mode = sprites_at(&s, FILE_GAME_MODES, TRAINING_MODE_TEXT)[0];
    assert_eq!(
        (mode.x, mode.y, mode.prim),
        (27.0, 24.0, Some([0xE3, 0xAC, 0x04]))
    );
    let back = sprites_at(&s, FILE_PLAYERS_COMMON, common::BACK_BUTTON)[0];
    assert_eq!((back.x, back.y), (244.0, 23.0));
}

#[test]
fn the_portraits_slide_in_as_the_vs_selects_do() {
    let mut s = first_visit();
    assert_eq!(s.view.portrait_x[0], -35.0);
    assert_eq!(s.view.portrait_x[5], 310.0);
    for _ in 0..34 {
        idle(&mut s);
    }
    let x: Vec<f32> = (0..6).map(|i| s.view.portrait_x[i]).collect();
    assert_eq!(x, [25.0, 70.0, 115.0, 160.0, 205.0, 250.0]);
    // With an empty fighter mask the four unlockables are shadows.
    let shadows = draws(&s)
        .into_iter()
        .filter(|d| matches!(d, Draw::Shadow(p) if p.file == FILE_PORTRAITS))
        .count();
    assert_eq!(shadows, 4);
}

#[test]
fn the_two_cards_their_texts_and_the_cpus_name() {
    let s = first_visit();
    let cards = sprites_at(&s, FILE_PLAYERS_1P_MODE, RED_CARD);
    assert_eq!(cards.len(), 2);
    assert_eq!(
        (cards[0].x, cards[0].y, cards[0].lut),
        (53.0, 127.0, Some(0))
    );
    assert_eq!(
        (cards[1].x, cards[1].y, cards[1].lut),
        (185.0, 127.0, Some(1))
    );
    let p1 = sprites_at(&s, FILE_PLAYERS_COMMON, common::TEXT_1P[0])[0];
    assert_eq!((p1.x, p1.y, p1.prim), (61.0, 131.0, Some([0; 3])));
    let cp = sprites_at(&s, FILE_PLAYERS_COMMON, common::TEXT_CP)[0];
    assert_eq!((cp.x, cp.y), (192.0, 132.0));
    // The CPU is Mario from the start; the player has nothing yet.
    let mario = FighterKind::Mario as usize;
    let name = sprites_at(&s, FILE_PLAYERS_COMMON, common::NAMES[mario]);
    assert_eq!(name.len(), 1);
    assert_eq!((name[0].x, name[0].y), (193.0, 202.0));
    let emblem = sprites_at(&s, FILE_EMBLEMS, vs::EMBLEMS[mario])[0];
    assert_eq!(
        (emblem.x, emblem.y, emblem.prim),
        (195.0, 144.0, Some([0x44; 3]))
    );
    assert!(!s.view.slots[MAN].name_shown);
    assert!(s.view.slots[MAN].fighter.is_none());
    let f = s.view.slots[COM].fighter.unwrap();
    assert_eq!((f.kind, f.status), (FighterKind::Mario, None));
    assert_eq!(fighter_position(MAN), [-830.0, -870.0, 0.0]);
    assert_eq!(fighter_position(COM), [830.0, -870.0, 0.0]);
}

#[test]
fn the_pucks_show_from_tick_thirty_the_players_only_off_the_pointer() {
    let mut s = first_visit();
    for _ in 0..29 {
        idle(&mut s);
    }
    assert!(!s.view.slots[COM].puck_shown);
    idle(&mut s);
    assert!(s.view.slots[COM].puck_shown);
    // The cursor starts on the pointer over the card: its puck is hidden.
    assert_eq!(s.slots[MAN].cursor_status, CursorStatus::Pointer);
    assert!(!s.view.slots[MAN].puck_shown);
    hover(&mut s, FighterKind::Fox);
    assert!(s.view.slots[MAN].puck_shown);
}

#[test]
fn a_hovered_fighter_turns_and_plays_its_status_once_placed() {
    let mut s = first_visit();
    for _ in 0..30 {
        idle(&mut s);
    }
    hover(&mut s, FighterKind::Kirby);
    let f = s.view.slots[MAN].fighter.unwrap();
    assert_eq!(
        (f.kind, f.status, f.hidden),
        (FighterKind::Kirby, None, false)
    );
    assert!(s.view.slots[MAN].name_shown);
    let kirby = FighterKind::Kirby as usize;
    let name = sprites_at(&s, FILE_PLAYERS_COMMON, common::NAMES[kirby])[0];
    assert_eq!((name.x, name.y), (61.0, 202.0));
    let emblem = sprites_at(&s, FILE_EMBLEMS, vs::EMBLEMS[kirby])[0];
    assert_eq!(
        (emblem.x, emblem.y, emblem.prim),
        (63.0, 144.0, Some([0x1E; 3]))
    );
    // Two degrees a tick while held.
    let before = s.view.slots[MAN].fighter.unwrap().rotate_y;
    idle(&mut s);
    let after = s.view.slots[MAN].fighter.unwrap().rotate_y;
    assert!((after - before - vs::dtor(2.0)).abs() < 1e-5);
    // Placed: a flash on Kirby's portrait, then 20 degrees a tick round to
    // 0 and Win3.
    tap(&mut s, N64Buttons::A);
    assert_eq!(
        s.view.slots[MAN].flash.map(|f| f.portrait),
        Some(portrait(FighterKind::Kirby))
    );
    for _ in 0..18 {
        idle(&mut s);
    }
    assert!(s.view.slots[MAN].flash.is_none());
    let f = s.view.slots[MAN].fighter.unwrap();
    assert_eq!((f.rotate_y, f.status), (0.0, Some(DemoStatus::Win3)));
    // Moving the held puck off the portraits hides the fighter.
    let mut s = first_visit();
    hover(&mut s, FighterKind::Kirby);
    s.slots[MAN].cursor = (70.0, 170.0);
    idle(&mut s);
    assert!(s.view.slots[MAN].fighter.unwrap().hidden);
    assert!(!s.view.slots[MAN].name_shown);
}

#[test]
fn grabbing_the_cpus_puck_lifts_it_over_the_cursor_link() {
    let mut s = first_visit();
    for _ in 0..30 {
        idle(&mut s);
    }
    hover(&mut s, FighterKind::Kirby);
    tap(&mut s, N64Buttons::A);
    // The placed puck is back on the puck link, just above the cursor.
    assert_eq!(s.view.slots[MAN].puck_draw.link, DL_PUCK);
    assert_eq!(s.view.slots[MAN].puck_draw.priority, 1);
    for _ in 0..30 {
        idle(&mut s);
    }
    // The cursor over the CPU's puck on Mario, then A: it grabs it.
    let (px, py) = s.slots[COM].puck;
    s.slots[MAN].cursor = (px - 25.0 + 13.0, py - 3.0 + 12.0);
    idle(&mut s);
    assert_eq!(s.slots[MAN].cursor_status, CursorStatus::Hover);
    tap(&mut s, N64Buttons::A);
    assert_eq!(s.slots[MAN].held, Some(COM));
    assert_eq!(s.view.slots[COM].puck_draw.link, DL_CURSOR);
    assert!(s.view.slots[COM].puck_draw.priority > s.view.cursor_draw.priority);
    assert!(!s.view.slots[COM].fighter.unwrap().hidden);
    // The pucks draw first, the held puck before the hand over it.
    let d = draws(&s);
    let tail: Vec<_> = d
        .iter()
        .skip_while(|d| **d != Draw::Fighters)
        .filter_map(|d| match d {
            Draw::Puck { piece, .. } => Some(piece.offset),
            Draw::Sprite(p) if p.offset == common::CURSORS[1] => Some(p.offset),
            _ => None,
        })
        .collect();
    assert_eq!(
        tail,
        [common::PUCKS[0], common::PUCKS[4], common::CURSORS[1]]
    );
}

#[test]
fn the_ready_banner_and_press_start_blink_on_one_counter() {
    let mut s = FighterSelect::new(
        SceneData {
            man_kind: Some(FighterKind::Kirby),
            man_costume: 2,
            com_kind: Some(FighterKind::Mario),
            com_costume: 1,
        },
        0,
        false,
        || 0,
    );
    // Both placed from the start: both fighters made, both names shown.
    assert!(s.view.slots[MAN].fighter.is_some() && s.view.slots[COM].fighter.is_some());
    assert!(s.view.slots[MAN].name_shown && s.view.slots[COM].name_shown);
    let mut banner = Vec::new();
    let mut press = Vec::new();
    for _ in 0..20 {
        idle(&mut s);
        banner.push(s.view.ready_banner);
        press.push(s.view.ready_press);
    }
    // Two steps a tick: shown while the count is below 30 of 40.
    // The banner steps to 1, 3, .. 39; "Press Start" to 2, 4, .. 38, then
    // 40, which wraps to 0.
    assert!(banner[..15].iter().all(|&b| b) && banner[15..].iter().all(|&b| !b));
    assert!(press[..14].iter().all(|&b| b) && press[14..19].iter().all(|&b| !b));
    assert!(press[19]);
    // Once placed and facing front, each plays its selected status at once.
    assert_eq!(
        s.view.slots[MAN].fighter.unwrap().status,
        Some(DemoStatus::Win3)
    );
    assert_eq!(
        s.view.slots[COM].fighter.unwrap().status,
        Some(DemoStatus::Win3)
    );
    let d = draws(&s);
    assert!(
        d.iter().any(
            |d| matches!(d, Draw::Tiled { piece, .. } if piece.offset == common::READY_BANNER)
        ) == s.view.ready_banner
    );
}

#[test]
fn the_draw_order_follows_the_cameras() {
    let mut s = first_visit();
    for _ in 0..40 {
        idle(&mut s);
    }
    let d = draws(&s);
    let at = |pred: &dyn Fn(&Draw) -> bool| d.iter().position(pred).unwrap();
    let stone = at(&|d| matches!(d, Draw::Tiled { .. }));
    let fire = at(&|d| matches!(d, Draw::Sprite(p) if p.file == FILE_PORTRAITS));
    let card = at(&|d| matches!(d, Draw::Sprite(p) if p.offset == RED_CARD));
    let fighters = at(&|d| *d == Draw::Fighters);
    let puck = at(&|d| matches!(d, Draw::Puck { .. }));
    let cursor = at(&|d| matches!(d, Draw::Sprite(p) if p.offset == common::CURSORS[0]));
    assert!(stone < fire && fire < card && card < fighters && fighters < puck && puck < cursor);
}
