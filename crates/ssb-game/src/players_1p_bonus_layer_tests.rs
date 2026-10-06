use super::*;
use crate::fighter_select::portrait_center;
use crate::players_1p::layer::{self as one, FILE_DIGITS};
use crate::players_1p_bonus::{records as make_records, SceneData};
use crate::spgame::{Backup, CHARACTER_MASK_ALL};
use ssb_engine::input::{ControllerState, N64Buttons};

fn pieces(s: &Players1PBonus, records: &Records) -> Vec<Draw> {
    let mut out = Vec::new();
    s.visit(records, |d| out.push(d));
    out
}

fn sprites(draws: &[Draw]) -> Vec<Piece> {
    draws
        .iter()
        .filter_map(|d| match d {
            Draw::Sprite(p) | Draw::Shadow(p) => Some(*p),
            Draw::Tiled { piece, .. } => Some(*piece),
            Draw::Puck { piece, .. } => Some(*piece),
            _ => None,
        })
        .collect()
}

fn at(draws: &[Draw], file: u32, offset: u32) -> Vec<(f32, f32)> {
    sprites(draws)
        .into_iter()
        .filter(|p| p.file == file && p.offset == offset)
        .map(|p| (p.x, p.y))
        .collect()
}

fn idle(s: &mut Players1PBonus) {
    s.tick(ControllerState::default(), N64Buttons(0));
}

fn new(bonus: BonusKind, player: u8) -> Players1PBonus {
    Players1PBonus::new(SceneData { player, bonus }, &Backup::default())
}

#[test]
fn the_title_names_the_practice() {
    let mut s = new(BonusKind::Targets, 0);
    idle(&mut s);
    let r = s.records(&Backup::default());
    let d = pieces(&s, &r);
    assert_eq!(
        at(
            &d,
            FILE_GAME_MODES,
            game_modes::BONUS1_BREAK_THE_TARGETS_TEXT
        ),
        [(27.0, 24.0)]
    );
    assert!(at(
        &d,
        FILE_GAME_MODES,
        game_modes::BONUS2_BOARD_THE_PLATFORMS_TEXT
    )
    .is_empty());
    s.bonus = BonusKind::Platforms;
    let d = pieces(&s, &r);
    assert_eq!(
        at(
            &d,
            FILE_GAME_MODES,
            game_modes::BONUS2_BOARD_THE_PLATFORMS_TEXT
        ),
        [(27.0, 24.0)]
    );
    assert_eq!(
        at(&d, FILE_PLAYERS_COMMON, common::BACK_BUTTON),
        [(244.0, 23.0)]
    );
}

#[test]
fn the_panel_sits_at_58_with_the_ports_lut() {
    let mut s = new(BonusKind::Targets, 2);
    let (px, py) = portrait_center(FighterKind::Fox);
    s.slot.cursor = (px - 11.0, py + 14.0);
    idle(&mut s);
    let d = pieces(&s, &s.records(&Backup::default()));
    let card = sprites(&d)
        .into_iter()
        .find(|p| p.offset == mode::RED_CARD && p.file == FILE_PLAYERS_1P_MODE);
    assert_eq!(
        card.map(|p| (p.lut, p.x, p.y)),
        Some((Some(3), 58.0, 127.0))
    );
    assert_eq!(
        at(&d, FILE_PLAYERS_COMMON, common::TEXT_1P[2]),
        [(63.0, 132.0)]
    );
    assert_eq!(
        at(&d, FILE_EMBLEMS, vs::EMBLEMS[FighterKind::Fox as usize]),
        [(68.0, 144.0)]
    );
    assert_eq!(
        at(
            &d,
            FILE_PLAYERS_COMMON,
            common::NAMES[FighterKind::Fox as usize]
        ),
        [(66.0, 202.0)]
    );
    assert_eq!(
        s.fighter_draw().map(|(f, pos, _, _)| (f.kind, pos)),
        Some((FighterKind::Fox, [-700.0, -850.0, 0.0]))
    );
}

#[test]
fn a_fighter_short_of_ten_shows_its_count() {
    let mut backup = Backup::default();
    backup.spgame_records[FighterKind::Mario as usize].bonus2_task_count = 7;
    let mut s = new(BonusKind::Platforms, 0);
    let (px, py) = portrait_center(FighterKind::Mario);
    s.slot.cursor = (px - 11.0, py + 14.0);
    idle(&mut s);
    let d = pieces(&s, &s.records(&backup));
    assert_eq!(
        at(&d, FILE_PLAYERS_1P_MODE, records::PLATFORMS_TEXT),
        [(235.0, 195.0)]
    );
    // "07", right-aligned on 225.
    let digits: Vec<_> = sprites(&d)
        .into_iter()
        .filter(|p| p.file == FILE_DIGITS)
        .map(|p| (p.offset, p.x, p.y, p.prim, p.env))
        .collect();
    assert_eq!(
        digits,
        [
            (
                one::DIGITS[7],
                217.0,
                194.0,
                Some([0x7E, 0x7C, 0x77]),
                [0; 3]
            ),
            (
                one::DIGITS[0],
                209.0,
                194.0,
                Some([0x7E, 0x7C, 0x77]),
                [0; 3]
            ),
        ]
    );
    assert!(at(&d, FILE_PLAYERS_1P_MODE, records::BEST_TIME_TEXT).is_empty());
}

#[test]
fn a_finished_fighter_shows_its_best_time() {
    let mut backup = Backup::default();
    let fox = FighterKind::Fox as usize;
    backup.spgame_records[fox].bonus1_task_count = 10;
    // 1:15 and 13 ticks: "01'15"21".
    backup.spgame_records[fox].bonus1_time = 75 * 60 + 13;
    let mut s = new(BonusKind::Targets, 0);
    let (px, py) = portrait_center(FighterKind::Fox);
    s.slot.cursor = (px - 11.0, py + 14.0);
    idle(&mut s);
    let d = pieces(&s, &s.records(&backup));
    let pieces: Vec<_> = sprites(&d)
        .into_iter()
        .filter(|p| p.y == 195.0 || p.y == 198.0)
        .map(|p| (p.file, p.offset, p.x))
        .collect();
    let digit = |n: usize, x: f32| (FILE_DIGITS, one::DIGITS[n], x);
    assert_eq!(
        pieces,
        [
            (FILE_PLAYERS_1P_MODE, records::BEST_TIME_TEXT, 177.0),
            digit(1, 229.0),
            digit(0, 221.0),
            (FILE_PLAYERS_1P_MODE, records::SEC, 239.0),
            digit(5, 251.0),
            digit(1, 243.0),
            (FILE_PLAYERS_1P_MODE, records::CSEC, 261.0),
            digit(1, 275.0),
            digit(2, 267.0),
        ]
    );
    assert!(at(&d, FILE_PLAYERS_1P_MODE, records::TARGETS_TEXT).is_empty());
}

#[test]
fn the_total_draws_only_when_all_twelve_are_finished() {
    let mut backup = Backup::default();
    for r in backup.spgame_records.iter_mut() {
        r.bonus1_task_count = 10;
        r.bonus1_time = 59 * 60 + 59;
    }
    let mut s = new(BonusKind::Targets, 0);
    idle(&mut s);
    let r = make_records(&backup, BonusKind::Targets, None, CHARACTER_MASK_ALL);
    let d = pieces(&s, &r);
    assert_eq!(
        at(&d, FILE_PLAYERS_1P_MODE, records::TOTAL_BEST_TIME_TEXT),
        [(142.0, 209.0)]
    );
    // 11:59"88: three minute digits ending on 237.
    let row: Vec<_> = sprites(&d)
        .into_iter()
        .filter(|p| p.y == 206.0)
        .map(|p| (p.offset, p.x))
        .collect();
    assert_eq!(
        row,
        [
            (one::DIGITS[8], 275.0),
            (one::DIGITS[8], 267.0),
            (records::CSEC, 261.0),
            (one::DIGITS[9], 251.0),
            (one::DIGITS[5], 243.0),
            (records::SEC, 239.0),
            (one::DIGITS[1], 229.0),
            (one::DIGITS[1], 221.0),
            (one::DIGITS[0], 213.0),
        ]
    );
    // The Platforms aren't finished: no total.
    s.bonus = BonusKind::Platforms;
    let d = pieces(&s, &s.records(&backup));
    assert!(at(&d, FILE_PLAYERS_1P_MODE, records::TOTAL_BEST_TIME_TEXT).is_empty());
}

/// Every sprite the layer draws, over the states that change what it
/// draws, is one the VS or 1P select's lists hold (resident in the pack)
/// or in [`MENU_SPRITES`] (the scene's menu pack) — and every one of
/// those is drawn.
#[test]
fn every_drawn_sprite_is_listed() {
    // Nothing finished (the counts), then everything (the times, the
    // total).
    let mut finished = Backup::default();
    for r in finished.spgame_records.iter_mut() {
        r.bonus1_task_count = 10;
        r.bonus2_task_count = 10;
    }
    let mut drawn = Vec::new();
    for backup in [Backup::default(), finished] {
        for bonus in [BonusKind::Targets, BonusKind::Platforms] {
            for player in 0..4 {
                let mut s = new(bonus, player);
                for kind in [FighterKind::Ness, FighterKind::Fox] {
                    let (px, py) = portrait_center(kind);
                    s.slot.cursor = (px - 11.0, py + 14.0);
                    idle(&mut s);
                    drawn.extend(sprites(&pieces(&s, &s.records(&backup))));
                }
                // Placed: the flash, "Press Start" and the banner.
                s.tick(ControllerState::default(), N64Buttons(N64Buttons::A));
                drawn.extend(sprites(&pieces(&s, &s.records(&backup))));
            }
        }
    }
    for p in &drawn {
        assert!(is_listed(p.file, p.offset), "{}+{:#x}", p.file, p.offset);
    }
    for &(file, offset) in MENU_SPRITES {
        assert!(
            drawn.iter().any(|p| (p.file, p.offset) == (file, offset)),
            "{file}+{offset:#x} never drawn"
        );
    }
}
