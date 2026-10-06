//! The title, the mode select and the 1P mode menu (RE-462).

use super::mode_select::{ModeOption, ModeSelect};
use super::one_p_mode::{OnePMode, OnePOption};
use super::title::{self, DemoData, Layout, NoAnims, Title};
use super::*;
use crate::backup::{Backup, CHARACTER_MASK_STARTER};

fn tap(buttons: u16) -> Pad {
    Pad {
        hold: buttons,
        tap: buttons,
        ..Pad::default()
    }
}

fn idle() -> Pad {
    Pad::default()
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

/// Ticks the title `n` times with no input.
fn run_title(t: &mut Title, demo: &mut DemoData, n: usize) -> Option<Scene> {
    let backup = Backup::default();
    let mut range = |r: i32| r - 1;
    let mut out = None;
    for _ in 0..n {
        if let Some(s) = t.tick(&idle(), Scene::Startup, demo, &backup, 0, &mut range) {
            out = Some(s);
        }
    }
    out
}

#[test]
fn the_title_lays_out_at_170_and_220_and_shows_press_start_at_280() {
    let mut demo = DemoData::default();
    let mut t = Title::new(0);
    // Tic 170 is the first tick: the labels and the layout's end.
    run_title(&mut t, &mut demo, 1);
    assert_eq!(t.layout, Layout::Final);
    let shown = sprites(|f| t.visit(&NoAnims, &mut |d| f(d)));
    // Two fire frames, the logo, five labels, footer and header.
    assert_eq!(shown.len(), 10);
    assert!(!shown.iter().any(|p| p.offset == title::sprite::PRESS_START));
    // The logo's 0x4C alpha through its own combiner.
    let logo = shown
        .iter()
        .find(|p| p.offset == title::sprite::LOGO_ANIM_FULL)
        .unwrap();
    assert_eq!(
        (logo.alpha, logo.solid, logo.prim),
        (Some(0x4C), true, Some([0xFF, 0, 0]))
    );
    run_title(&mut t, &mut demo, 109);
    assert!(!sprites(|f| t.visit(&NoAnims, &mut |d| f(d)))
        .iter()
        .any(|p| p.offset == title::sprite::PRESS_START));
    run_title(&mut t, &mut demo, 1);
    assert!(sprites(|f| t.visit(&NoAnims, &mut |d| f(d)))
        .iter()
        .any(|p| p.offset == title::sprite::PRESS_START));
    // `is_title_anim_viewed` at tic 280, the allow wait of the final layout.
    assert!(demo.is_title_anim_viewed);
}

#[test]
fn start_waits_for_the_animation_on_a_first_boot() {
    let backup = Backup::default();
    let mut demo = DemoData::default();
    let mut t = Title::new(0);
    let mut range = |r: i32| r - 1;
    let mut start = |t: &mut Title, demo: &mut DemoData| {
        t.tick(
            &tap(N64Buttons::START),
            Scene::Startup,
            demo,
            &backup,
            0,
            &mut range,
        )
    };
    run_title(&mut t, &mut demo, 5);
    assert_eq!(start(&mut t, &mut demo), None);
    run_title(&mut t, &mut demo, 3);
    assert!(!sprites(|f| t.visit(&NoAnims, &mut |d| f(d))).is_empty());
    // Once seen, START proceeds after the three-tic wait, B never does.
    let mut demo = DemoData {
        is_title_anim_viewed: true,
        ..DemoData::default()
    };
    let mut t = Title::new(0);
    run_title(&mut t, &mut demo, 1);
    let b = t.tick(
        &tap(N64Buttons::B),
        Scene::Startup,
        &mut demo,
        &backup,
        0,
        &mut |r| r - 1,
    );
    assert_eq!(b, None);
    assert_eq!(start(&mut t, &mut demo), None);
    // The proceeding title draws its black camera last.
    let mut last = None;
    t.visit(&NoAnims, &mut |d| last = Some(d));
    assert_eq!(last, Some(Draw::Clear([0, 0, 0, 0xFF])));
    assert_eq!(run_title(&mut t, &mut demo, 3), Some(Scene::ModeSelect));
}

#[test]
fn the_title_goes_to_its_demos_after_650_or_1190_tics() {
    let mut demo = DemoData::default();
    let mut t = Title::new(0);
    // From tic 170 to 650.
    assert_eq!(run_title(&mut t, &mut demo, 481), None);
    assert!(demo.is_extend_demo_wait);
    assert_eq!(run_title(&mut t, &mut demo, 3), Some(Scene::Explain));
    // An extended wait: tic 1190.
    let mut t = Title::new(0);
    let backup = Backup::default();
    let mut next = None;
    for _ in 0..1024 {
        next = next.or(
            t.tick(&idle(), Scene::Explain, &mut demo, &backup, 0, &mut |r| {
                r - 1
            }),
        );
    }
    assert_eq!(next, Some(Scene::Characters));
    // After the mode select or the auto demo: the N64 logo.
    let mut t = Title::new(0);
    for _ in 0..1024 {
        next = t.tick(
            &idle(),
            Scene::ModeSelect,
            &mut demo,
            &backup,
            0,
            &mut |r| r - 1,
        );
        if next.is_some() {
            break;
        }
    }
    assert_eq!(next, Some(Scene::Startup));
}

#[test]
fn demo_fighters_cycle_through_the_unlocked() {
    let backup = Backup::default();
    let mut demo = DemoData::default();
    let mut seen = 0u16;
    // The first eligible each time: every starter once, in order.
    for _ in 0..4 {
        demo.set_demo_fighter_kinds(&backup, &mut |_| 0);
        seen |= 1 << demo.demo_fkind[0] as u16 | 1 << demo.demo_fkind[1] as u16;
    }
    assert_eq!(seen, CHARACTER_MASK_STARTER);
    assert_eq!(demo.demo_mask_prev, CHARACTER_MASK_STARTER);
    // Every starter shown: the mask starts over, the first comes back.
    demo.set_demo_fighter_kinds(&backup, &mut |_| 0);
    assert_eq!(demo.demo_mask_prev.count_ones(), 2);
}

#[test]
fn a_boot_counts_until_the_animation_is_seen() {
    let mut backup = Backup::default();
    let boot = backup.boot;
    title::count_boot(&DemoData::default(), &mut backup);
    assert_eq!(backup.boot, boot.wrapping_add(1));
    let seen = DemoData {
        is_title_anim_viewed: true,
        ..DemoData::default()
    };
    title::count_boot(&seen, &mut backup);
    assert_eq!(backup.boot, boot.wrapping_add(1));
}

#[test]
fn press_start_loops_every_39_plays() {
    assert_eq!(title::press_start_frame(0), 0);
    assert_eq!(title::press_start_frame(1), 1);
    assert_eq!(title::press_start_frame(39), 39);
    assert_eq!(title::press_start_frame(40), 1);
}

#[test]
fn the_fire_brightens_toward_a_new_colour_every_260_tics() {
    let mut demo = DemoData::default();
    let mut t = Title::new(0);
    let start = t.fire_color();
    assert_eq!(start, [0xFF; 3]);
    // 80 tics of change toward a colour other than white.
    run_title(&mut t, &mut demo, 260);
    let next = t.fire_color();
    assert_ne!(next, start);
}

#[test]
fn mode_select_moves_round_its_four_options() {
    let mut m = ModeSelect::new(Scene::Data);
    assert_eq!(m.option, ModeOption::Data);
    for _ in 0..10 {
        m.tick(&idle());
    }
    m.tick(&tap(N64Buttons::D_DOWN));
    assert_eq!(m.option, ModeOption::OnePMode);
    // Held, the button's wait of 12 repeats on the twelfth tic.
    for _ in 0..11 {
        m.tick(&Pad {
            hold: N64Buttons::D_RIGHT,
            ..Pad::default()
        });
    }
    assert_eq!(m.option, ModeOption::OnePMode);
    m.tick(&Pad {
        hold: N64Buttons::D_RIGHT,
        ..Pad::default()
    });
    assert_eq!(m.option, ModeOption::Data);
    m.tick(&idle());
    assert_eq!(m.tick(&tap(N64Buttons::A)), Some(Scene::Data));
    assert_eq!(ModeSelect::new(Scene::Title).tick(&idle()), None);
    // B goes to the title; an up-left stick moves nothing.
    let mut m = ModeSelect::new(Scene::VsMode);
    for _ in 0..10 {
        m.tick(&idle());
    }
    let diagonal = Pad {
        stick_x: -60,
        stick_y: 60,
        ..Pad::default()
    };
    m.tick(&diagonal);
    assert_eq!(m.option, ModeOption::VsMode);
    assert_eq!(m.tick(&tap(N64Buttons::B)), Some(Scene::Title));
}

#[test]
fn mode_select_draws_labels_first_once_the_options_are_remade() {
    let mut m = ModeSelect::new(Scene::Title);
    let order = |m: &ModeSelect| {
        sprites(|f| m.visit(&mut |d| f(d)))
            .iter()
            .map(|p| p.offset)
            .collect::<Vec<_>>()
    };
    let first = order(&m);
    // The decals, then the bright 1P Mode icon before its label.
    assert_eq!(first[4], mode_select::sprite::CONTROLLER_ICON);
    assert_eq!(first[8], mode_select::sprite::ONE_P_MODE_TEXT);
    for _ in 0..10 {
        m.tick(&idle());
    }
    m.tick(&tap(N64Buttons::D_UP));
    assert_eq!(m.option, ModeOption::Data);
    let after = order(&m);
    assert_eq!(after[4], mode_select::sprite::ONE_P_MODE_TEXT);
    assert_eq!(after[11], mode_select::sprite::DATA_ICON);
}

#[test]
fn one_p_mode_keeps_its_option_and_proceeds_a_tic_later() {
    assert_eq!(
        OnePMode::new(Scene::ModeSelect, OnePOption::Bonus2Practice).option,
        OnePOption::Bonus2Practice
    );
    let mut m = OnePMode::new(Scene::Players1PTraining, OnePOption::OnePGame);
    assert_eq!(m.option, OnePOption::TrainingMode);
    for _ in 0..10 {
        m.tick(&idle());
    }
    m.tick(&tap(N64Buttons::D_DOWN));
    assert_eq!(m.option, OnePOption::Bonus1Practice);
    assert_eq!(m.tab_status[1], TabStatus::Not);
    assert_eq!(m.tab_status[2], TabStatus::Highlight);
    for _ in 0..20 {
        m.tick(&idle());
    }
    assert_eq!(m.tick(&tap(N64Buttons::A)), None);
    assert_eq!(m.tab_status[2], TabStatus::Selected);
    assert_eq!(m.tick(&idle()), Some(Scene::Players1PBonus1));
    // Up from the first wraps to Bonus 2 Practice.
    let mut m = OnePMode::new(Scene::Players1PGame, OnePOption::OnePGame);
    for _ in 0..10 {
        m.tick(&idle());
    }
    m.tick(&tap(N64Buttons::C_UP));
    assert_eq!(m.option, OnePOption::Bonus2Practice);
    assert_eq!(m.tick(&tap(N64Buttons::B)), Some(Scene::ModeSelect));
}

#[test]
fn the_front_menus_go_back_to_the_title_after_five_idle_minutes() {
    let mut m = ModeSelect::new(Scene::Title);
    let mut o = OnePMode::new(Scene::ModeSelect, OnePOption::OnePGame);
    let (mut a, mut b) = (None, None);
    for _ in 0..IDLE_RETURN {
        a = a.or(m.tick(&idle()));
        b = b.or(o.tick(&idle()));
    }
    assert_eq!((a, b), (Some(Scene::Title), Some(Scene::Title)));
}
