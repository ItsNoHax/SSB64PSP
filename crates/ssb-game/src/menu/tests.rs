use super::backup_clear::{BackupClear, ClearOption, LUT_CONFIRM, LUT_HIGHLIGHT};
use super::characters::{self, CharactersMenu, Motion};
use super::data::{DataMenu, DataOption};
use super::option::{OptionMenu, Tab};
use super::screen_adjust::ScreenAdjust;
use super::vs_record::{Page, VsRecordMenu};
use super::*;
use crate::backup::{Backup, Selections};
use crate::fighter::FighterKind;
use crate::status::{AnyStatus, Status};

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

/// Runs `n` idle ticks.
fn settle<T>(m: &mut T, n: usize, mut tick: impl FnMut(&mut T, &Pad)) {
    for _ in 0..n {
        tick(m, &idle());
    }
}

fn sprites(visit: impl FnOnce(&mut dyn FnMut(Draw))) -> Vec<Piece> {
    let mut out = Vec::new();
    visit(&mut |d| {
        if let Draw::Sprite(p) = d {
            out.push(p);
        }
    });
    out
}

#[test]
fn change_waits_follow_mndef() {
    assert_eq!(wait_p(80, 7), 11);
    assert_eq!(wait_n(-80, 7), 11);
    let mut r = Repeat::default();
    assert!(r.check(0, &hold(UP), UP, true, 20, true));
    assert_eq!(r.wait_p(7), 12);
    let stick = Pad {
        stick_y: 50,
        ..Pad::default()
    };
    assert!(r.check(0, &stick, UP, true, 20, true));
    assert_eq!((r.is_button, r.stick), (false, 50));
    assert_eq!(r.wait_p(7), (160 - 50) / 7);
    assert!(!r.check(3, &stick, UP, true, 20, true));
}

#[test]
fn option_waits_ten_tics_then_moves_and_writes_on_exit() {
    let mut b = Backup::default();
    let mut m = OptionMenu::new(Scene::ModeSelect, 1, &b);
    assert_eq!((m.tab, m.mono_or_stereo), (Tab::Sound, 1));
    // Nothing happens before the tenth tic.
    for _ in 0..9 {
        assert_eq!(m.tick(&tap(N64Buttons::B), &mut b), None);
    }
    assert_eq!(m.tick(&tap(N64Buttons::D_DOWN), &mut b), None);
    assert_eq!(m.tab, Tab::ScreenAdjust);
    assert_eq!(
        m.tab_status,
        [TabStatus::Not, TabStatus::Highlight, TabStatus::Not]
    );
    // A on Screen Adjust writes the backup and loads the scene a tic later.
    let writes = b.writes;
    assert_eq!(m.tick(&tap(N64Buttons::A), &mut b), None);
    assert_eq!(b.writes, writes + 1);
    assert_eq!(m.tab_status[1], TabStatus::Selected);
    assert_eq!(m.tick(&idle(), &mut b), Some(Scene::ScreenAdjust));
}

#[test]
fn option_sound_toggles_and_is_saved() {
    let mut b = Backup::default();
    let mut m = OptionMenu::new(Scene::ModeSelect, 1, &b);
    settle(&mut m, 9, |m, p| {
        m.tick(p, &mut b);
    });
    m.tick(&tap(N64Buttons::D_RIGHT), &mut b);
    assert_eq!(m.mono_or_stereo, 0);
    m.tick(&tap(N64Buttons::A), &mut b);
    assert_eq!(m.mono_or_stereo, 1);
    m.tick(&tap(N64Buttons::A), &mut b);
    assert_eq!(m.tick(&tap(N64Buttons::B), &mut b), Some(Scene::ModeSelect));
    assert_eq!(b.sound_mono_or_stereo, 0);
    // The underline sits under Mono.
    let mut fills = Vec::new();
    m.visit(&mut |d| {
        if let Draw::Fill { rect, .. } = d {
            fills.push(rect);
        }
    });
    assert_eq!(fills.last(), Some(&[233.0, 64.0, 274.0, 65.0]));
}

#[test]
fn option_returns_to_the_title_after_five_idle_minutes() {
    let mut b = Backup::default();
    let mut m = OptionMenu::new(Scene::BackupClear, 0, &b);
    assert_eq!(m.tab, Tab::BackupClear);
    let mut out = None;
    for _ in 0..IDLE_RETURN {
        out = m.tick(&idle(), &mut b);
        if out.is_some() {
            break;
        }
    }
    assert_eq!(out, Some(Scene::Title));
}

#[test]
fn screen_adjust_clamps_resets_and_saves() {
    let mut b = Backup::default();
    let mut m = ScreenAdjust::new(3, -2);
    settle(&mut m, 9, |m, p| {
        m.tick(p, &mut b);
    });
    for _ in 0..30 {
        m.tick(&tap(N64Buttons::D_LEFT), &mut b);
    }
    assert_eq!(m.offset_h, -14.0);
    let stick = Pad {
        stick_y: 80,
        ..Pad::default()
    };
    m.tick(&stick, &mut b);
    assert_eq!(m.offset_v, -2.0 - 80.0 / 50.0);
    assert_eq!(m.tick(&tap(N64Buttons::START), &mut b), Some(Scene::Option));
    assert_eq!((b.screen_adjust_h, b.screen_adjust_v), (-14, -3));
    m.tick(&tap(N64Buttons::Z), &mut b);
    assert_eq!((m.offset_h, m.offset_v), (0.0, 0.0));
}

#[test]
fn backup_clear_confirms_clears_and_blinks() {
    let mut b = Backup::default();
    b.vs_records[0].ko_count[1] = 7;
    b.vs_total_battles = 9;
    let mut s = Selections::default();
    let mut m = BackupClear::new();
    let run = |m: &mut BackupClear, b: &mut Backup, s: &mut Selections, p: &Pad| m.tick(p, b, s);
    for _ in 0..10 {
        run(&mut m, &mut b, &mut s, &idle());
    }
    // Down three times to VS Record (each move waits out its repeat).
    for _ in 0..3 {
        run(&mut m, &mut b, &mut s, &tap(N64Buttons::D_DOWN));
        for _ in 0..12 {
            run(&mut m, &mut b, &mut s, &idle());
        }
    }
    assert_eq!(m.option, ClearOption::VsRecord);
    run(&mut m, &mut b, &mut s, &tap(N64Buttons::A));
    assert!(!m.options_shown);
    let c = m.confirm.unwrap();
    assert_eq!((c.kind, c.yes_or_no), (1, 1));
    for _ in 0..10 {
        run(&mut m, &mut b, &mut s, &idle());
    }
    run(&mut m, &mut b, &mut s, &tap(N64Buttons::D_RIGHT));
    assert_eq!(m.confirm.unwrap().yes_or_no, 0);
    let writes = b.writes;
    run(&mut m, &mut b, &mut s, &tap(N64Buttons::A));
    assert_eq!(b.vs_records[0].ko_count[1], 0);
    assert_eq!(b.writes, writes + 1);
    assert_eq!(m.confirm.unwrap().yes_lut, LUT_CONFIRM);
    // Sixty tics of blinking, every tenth swapping the LUT back.
    for i in 1..=60 {
        run(&mut m, &mut b, &mut s, &idle());
        if i < 60 && i % 10 == 0 {
            let expect = if (i / 10) % 2 == 1 {
                LUT_HIGHLIGHT
            } else {
                LUT_CONFIRM
            };
            assert_eq!(m.confirm.unwrap().yes_lut, expect, "tic {i}");
        }
    }
    assert!(m.confirm.is_none() && m.options_shown);
}

#[test]
fn all_data_clear_asks_twice_and_applies_options() {
    let mut b = Backup {
        is_allow_screenflash: false,
        ..Backup::default()
    };
    let mut s = Selections::default();
    let mut m = BackupClear::new();
    for _ in 0..10 {
        m.tick(&idle(), &mut b, &mut s);
    }
    m.tick(&tap(N64Buttons::D_UP), &mut b, &mut s);
    assert_eq!(m.option, ClearOption::AllDataClear);
    for step in [N64Buttons::A, N64Buttons::D_RIGHT, N64Buttons::A] {
        for _ in 0..10 {
            m.tick(&idle(), &mut b, &mut s);
        }
        m.tick(&tap(step), &mut b, &mut s);
    }
    assert_eq!(m.confirm.unwrap().kind, 2);
    assert!(!b.is_allow_screenflash);
    for step in [N64Buttons::D_RIGHT, N64Buttons::A] {
        for _ in 0..10 {
            m.tick(&idle(), &mut b, &mut s);
        }
        m.tick(&tap(step), &mut b, &mut s);
    }
    assert!(m.apply_options);
    assert!(b.is_allow_screenflash);
}

#[test]
fn data_shows_sound_test_only_once_unlocked() {
    let b = Backup::default();
    let mut m = DataMenu::new(Scene::ModeSelect, &b);
    assert!(!m.is_have_sound_test);
    settle(&mut m, 9, |m, p| {
        m.tick(p);
    });
    m.tick(&tap(N64Buttons::D_UP));
    assert_eq!(m.option, DataOption::VsRecord);
    assert_eq!(m.tick(&tap(N64Buttons::A)), None);
    assert_eq!(m.tick(&idle()), Some(Scene::VsRecord));

    let b = Backup {
        unlock_mask: Backup::default().unlock_mask | crate::spgame::Unlock::SoundTest.mask(),
        ..Backup::default()
    };
    let m = DataMenu::new(Scene::SoundTest, &b);
    assert!(m.is_have_sound_test);
    assert_eq!(m.option, DataOption::SoundTest);
    // The tabs move up to make room.
    let p = sprites(|f| m.visit(&mut |d| f(d)));
    assert!(p
        .iter()
        .any(|p| p.offset == super::data::sprite::SOUND_TEST_TEXT && p.x == 95.0));
    assert!(p
        .iter()
        .any(|p| p.offset == super::data::sprite::CHARACTERS_TEXT && (p.x, p.y) == (159.0, 46.0)));
}

#[test]
fn vs_record_sorts_by_kos_and_ranks_by_win_percent() {
    let mut b = Backup::default();
    // Fox KOs Mario three times; Mario KOs Fox once.
    b.vs_records[1].ko_count[0] = 3;
    b.vs_records[0].ko_count[1] = 1;
    let m = VsRecordMenu::new(&b);
    assert_eq!(m.page, Page::BattleScore);
    assert_eq!(m.battle_score_order[0], 1);
    assert_eq!(m.battle_score_order[1], 0);
    // The locked newcomers sink to the end.
    assert!(m.battle_score_order[8..]
        .iter()
        .all(|&k| matches!(k, 4 | 7 | 10 | 11)));
}

#[test]
fn vs_record_pages_turn_with_a_and_b() {
    let b = Backup::default();
    let mut m = VsRecordMenu::new(&b);
    m.tick(&tap(N64Buttons::A), &b);
    assert_eq!(m.page, Page::Ranking);
    m.tick(&idle(), &b);
    m.tick(&tap(N64Buttons::D_RIGHT), &b);
    assert_eq!(m.first_column, 6);
    m.tick(&tap(N64Buttons::START), &b);
    assert_eq!(m.page, Page::Indiv);
    m.tick(&tap(N64Buttons::B), &b);
    m.tick(&tap(N64Buttons::B), &b);
    assert_eq!(m.page, Page::BattleScore);
    assert_eq!(m.tick(&tap(N64Buttons::B), &b), Some(Scene::Data));
}

#[test]
fn vs_record_digits_show_tenths_and_full_percent() {
    let mut b = Backup::default();
    for k in [0usize, 1] {
        b.vs_records[k].games_played = 1;
    }
    let mut m = VsRecordMenu::new(&b);
    m.tick(&tap(N64Buttons::A), &b);
    m.tick(&tap(N64Buttons::A), &b);
    assert_eq!(m.page, Page::Indiv);
    // Use % of the first ranked fighter: 50.0, drawn as "50.0" right of 265.
    let p = sprites(|f| m.visit(&b, &mut |d| f(d)));
    let digits: Vec<_> = p
        .iter()
        .filter(|p| {
            p.file == FILE_VS_RECORD && p.y == 66.0
                || p.offset == super::vs_record::sprite::SYMBOL_POINT && p.y == 70.0
        })
        .map(|p| (p.offset, p.x))
        .collect();
    use super::vs_record::sprite::{DIGITS, SYMBOL_POINT};
    assert_eq!(
        digits,
        vec![
            (DIGITS[0], 260.0),
            (SYMBOL_POINT, 257.0),
            (DIGITS[0], 252.0),
            (DIGITS[5], 247.0)
        ]
    );
}

#[test]
fn characters_motions_follow_the_tables() {
    use crate::status::FoxStatus;
    let t = characters::motion(FighterKind::Fox, characters::SPECIAL_HI, 1);
    assert_eq!(
        t.motion,
        Motion::Status(AnyStatus::Fox(FoxStatus::SpecialHiHold))
    );
    assert_eq!(t.length, 35);
    // Kirby's aerials fall for 30 tics, not FallAerial.
    let t = characters::motion(FighterKind::Kirby, 27, 1);
    assert_eq!(
        (t.motion, t.length),
        (Motion::Status(AnyStatus::Common(Status::Fall)), 30)
    );
    let t = characters::motion(FighterKind::Mario, 27, 1);
    assert_eq!(
        t.motion,
        Motion::Status(AnyStatus::Common(Status::FallAerial))
    );
    // Wait plays until its clip ends, then the motion is over.
    assert_eq!(
        characters::motion(FighterKind::Ness, 14, 1).motion,
        Motion::Null
    );
}

#[test]
fn characters_skip_locked_pages_and_save_the_fighter() {
    let mut b = Backup::default();
    let mut clock = 0u8;
    let mut rand = || {
        clock = clock.wrapping_add(37);
        clock
    };
    let mut m = CharactersMenu::new(&b, &mut rand);
    assert_eq!(m.kind(), FighterKind::Mario);
    for _ in 0..9 {
        m.tick(&idle(), &mut b, &mut rand);
    }
    // Right from Mario skips Luigi, who is locked.
    m.tick(&tap(N64Buttons::D_RIGHT), &mut b, &mut rand);
    assert_eq!(m.kind(), FighterKind::Donkey);
    assert_eq!(
        m.tick(&tap(N64Buttons::B), &mut b, &mut rand),
        Some(Scene::Data)
    );
    assert_eq!(b.characters_fkind, FighterKind::Donkey);
}

#[test]
fn characters_fighter_runs_through_its_tracks() {
    let b = Backup::default();
    let mut clock = 0u8;
    let mut rand = || {
        clock = clock.wrapping_add(53);
        clock
    };
    let mut m = CharactersMenu::new(&b, &mut rand);
    let first = m.fighter.serial;
    let mut changes = 0;
    let mut last = first;
    for i in 0..2000 {
        // A clip reads frame 0 every 40 tics.
        m.tick_fighter(i % 40 == 39, &mut rand);
        if m.fighter.serial != last {
            changes += 1;
            last = m.fighter.serial;
            assert_ne!(m.fighter.motion, Motion::Null);
        }
    }
    assert!(changes > 10, "{changes}");
    assert!(m.fighter.rotate_y > 0.0 && m.fighter.rotate_y <= core::f32::consts::TAU);
}

#[test]
fn option_mono_and_stereo_set_the_audio_quality() {
    use crate::sound::{id, testing::*};
    let r = Recorder::install();
    let mut b = Backup::default();
    let _ = OptionMenu::new(Scene::ScreenAdjust, 1, &b);
    assert_eq!(r.take(), [Call::PlayBgm(0, id::nSYAudioBGMModeSelect)]);
    let mut m = OptionMenu::new(Scene::ModeSelect, 1, &b);
    assert_eq!(r.take(), []);
    settle(&mut m, 9, |m, p| {
        m.tick(p, &mut b);
    });
    m.tick(&tap(N64Buttons::D_RIGHT), &mut b);
    assert_eq!(
        r.take(),
        [
            Call::PlayFgm(id::nSYAudioFGMMenuScroll1),
            Call::SetQuality(0)
        ]
    );
    // Already mono: right again does nothing.
    m.tick(&tap(N64Buttons::D_RIGHT), &mut b);
    assert_eq!(r.take(), []);
    m.tick(&tap(N64Buttons::A), &mut b);
    assert_eq!(
        r.take(),
        [
            Call::PlayFgm(id::nSYAudioFGMMenuScroll1),
            Call::SetQuality(1)
        ]
    );
    crate::sound::uninstall();
}

#[test]
fn all_data_clear_applies_the_backups_quality() {
    use crate::sound::testing::*;
    let r = Recorder::install();
    let b = Backup {
        sound_mono_or_stereo: 0,
        ..Backup::default()
    };
    b.apply_options();
    assert_eq!(r.take(), [Call::SetQuality(0)]);
    crate::sound::uninstall();
}
