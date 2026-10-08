//! Every sprite the options and data menus draw is among its scene's own
//! sprites (`ssb_rom::menu_pack`) or the common ones (RE-461; one pack
//! since RE-475), and every scene sprite decodes from the ROM.

use std::collections::BTreeSet;

use ssb_engine::input::N64Buttons;
use ssb_game::backup::{Backup, Selections};
use ssb_game::menu::backup_clear::BackupClear;
use ssb_game::menu::characters::CharactersMenu;
use ssb_game::menu::data::DataMenu;
use ssb_game::menu::option::OptionMenu;
use ssb_game::menu::screen_adjust::ScreenAdjust;
use ssb_game::menu::vs_record::VsRecordMenu;
use ssb_game::menu::{Draw, Pad, Scene};
use ssb_rom::menu_pack::MenuScene;

type Key = (u32, u32, Option<u8>);

fn tap(b: u16) -> Pad {
    Pad {
        hold: b,
        tap: b,
        ..Pad::default()
    }
}

fn collect(out: &mut BTreeSet<Key>) -> impl FnMut(Draw) + '_ {
    move |d| match d {
        Draw::Sprite(p) | Draw::Tiled { piece: p, .. } => {
            out.insert((p.file, p.offset, p.lut));
        }
        _ => {}
    }
}

/// What a scene's pack and the resident pack hold.
fn available(scene: MenuScene) -> BTreeSet<Key> {
    let mut set = BTreeSet::new();
    for f in scene.sprites().iter().chain(ssb_rom::sprite::FILES) {
        for &at in f.offsets {
            set.insert((f.file, at, None));
        }
    }
    for &(file, at, luts) in scene.lut_sprites() {
        for i in 0..luts.len() {
            set.insert((file, at, Some(i as u8)));
        }
    }
    set
}

fn check(scene: MenuScene, drawn: &BTreeSet<Key>) {
    // The walk reaches every sprite the scene's own pack holds.
    let own: usize = scene
        .sprites()
        .iter()
        .map(|f| f.offsets.len())
        .sum::<usize>()
        + scene.lut_sprites().iter().map(|l| l.2.len()).sum::<usize>();
    let reached = drawn
        .iter()
        .filter(|k| {
            !ssb_rom::sprite::FILES
                .iter()
                .any(|f| f.file == k.0 && f.offsets.contains(&k.1))
        })
        .count();
    let all: BTreeSet<Key> = scene
        .sprites()
        .iter()
        .flat_map(|f| f.offsets.iter().map(|&o| (f.file, o, None)))
        .collect();
    let unreached: Vec<_> = all.difference(drawn).collect();
    assert_eq!(reached, own, "{scene:?} never drew {unreached:x?}");
    let have = available(scene);
    let missing: Vec<_> = drawn.difference(&have).collect();
    assert!(
        missing.is_empty(),
        "{scene:?} draws sprites no pack holds: {missing:x?}"
    );
}

fn idle(n: usize, mut f: impl FnMut()) {
    for _ in 0..n {
        f();
    }
}

#[test]
fn option_menus_draw_only_packed_sprites() {
    let mut b = Backup::default();
    let mut drawn = BTreeSet::new();
    for quality in [0, 1] {
        let mut m = OptionMenu::new(Scene::ModeSelect, quality, &b);
        idle(10, || {
            m.tick(&Pad::default(), &mut b);
        });
        for _ in 0..3 {
            m.visit(&mut collect(&mut drawn));
            m.tick(&tap(N64Buttons::D_DOWN), &mut b);
            idle(20, || {
                m.tick(&Pad::default(), &mut b);
            });
        }
    }
    check(MenuScene::Option, &drawn);

    let mut drawn = BTreeSet::new();
    ScreenAdjust::new(0, 0).visit(&mut collect(&mut drawn));
    check(MenuScene::ScreenAdjust, &drawn);

    let mut drawn = BTreeSet::new();
    let mut s = Selections::default();
    let mut m = BackupClear::new();
    let mut step = |m: &mut BackupClear, b: &mut Backup, pad: Pad, drawn: &mut BTreeSet<Key>| {
        m.tick(&pad, b, &mut s);
        m.visit(&mut collect(drawn));
    };
    for _ in 0..10 {
        step(&mut m, &mut b, Pad::default(), &mut drawn);
    }
    step(&mut m, &mut b, tap(N64Buttons::D_UP), &mut drawn);
    for pad in [
        N64Buttons::A,
        N64Buttons::D_RIGHT,
        N64Buttons::A,
        N64Buttons::D_RIGHT,
        N64Buttons::A,
    ] {
        for _ in 0..10 {
            step(&mut m, &mut b, Pad::default(), &mut drawn);
        }
        step(&mut m, &mut b, tap(pad), &mut drawn);
    }
    for _ in 0..70 {
        step(&mut m, &mut b, Pad::default(), &mut drawn);
    }
    check(MenuScene::BackupClear, &drawn);
}

#[test]
fn data_menus_draw_only_packed_sprites() {
    let mut drawn = BTreeSet::new();
    for unlock in [0, 0x7F] {
        let b = Backup {
            unlock_mask: unlock,
            ..Backup::default()
        };
        DataMenu::new(Scene::ModeSelect, &b).visit(&mut collect(&mut drawn));
    }
    check(MenuScene::Data, &drawn);

    let mut drawn = BTreeSet::new();
    let mut b = Backup {
        fighter_mask: 0x0FFF,
        ..Backup::default()
    };
    // Every digit shows somewhere.
    for (i, r) in b.vs_records.iter_mut().enumerate() {
        for (j, k) in r.ko_count.iter_mut().enumerate() {
            *k = ((i * 7 + j * 3) % 10) as u16 + 1;
        }
        r.time_used = 3600 * i as u32 + 61 * 9;
        r.damage_given = 1234567 + i as u32;
        r.damage_taken = 89 * i as u32;
        r.selfdestructs = i as u16;
        r.games_played = 3 + i as u16;
        r.player_count_tally = 7 * i as u16;
    }
    for mask in [Backup::default().fighter_mask, 0x0FFF] {
        let b = Backup {
            fighter_mask: mask,
            ..b
        };
        let mut m = VsRecordMenu::new(&b);
        m.visit(&b, &mut collect(&mut drawn));
        m.tick(&tap(N64Buttons::A), &b);
        for _ in 0..7 {
            m.visit(&b, &mut collect(&mut drawn));
            m.tick(&tap(N64Buttons::D_RIGHT), &b);
            m.tick(&Pad::default(), &b);
        }
        m.tick(&tap(N64Buttons::A), &b);
        for _ in 0..12 {
            m.visit(&b, &mut collect(&mut drawn));
            m.tick(&tap(N64Buttons::D_RIGHT), &b);
            m.tick(&Pad::default(), &b);
        }
    }
    check(MenuScene::VsRecord, &drawn);

    let mut drawn = BTreeSet::new();
    let mut b = Backup {
        fighter_mask: 0x0FFF,
        ..Backup::default()
    };
    let mut seed = 1u32;
    let mut rand = || {
        seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12345);
        (seed >> 16) as u8
    };
    let mut m = CharactersMenu::new(&b, &mut rand);
    for _ in 0..10 {
        m.tick(&Pad::default(), &mut b, &mut rand);
    }
    for _ in 0..12 {
        for i in 0..20000 {
            m.tick_fighter(i % 30 == 0, &mut rand);
            if m.shown_special().is_some() {
                m.visit(&mut collect(&mut drawn));
            }
        }
        m.visit(&mut collect(&mut drawn));
        m.tick(&tap(N64Buttons::D_RIGHT), &mut b, &mut rand);
        m.tick(&Pad::default(), &mut b, &mut rand);
    }
    check(MenuScene::Characters, &drawn);
}

#[test]
fn every_menu_pack_sprite_decodes() {
    let Some(path) = std::env::var_os("SSB64_ROM") else {
        return;
    };
    let data = std::fs::read(path).unwrap();
    let info = ssb_rom::rom::identify(&data).unwrap();
    let archive = ssb_rom::archive::Archive::open(&data, info.region).unwrap();
    for scene in MenuScene::ALL {
        for f in scene.sprites() {
            let file = archive.load(f.file).unwrap();
            for &at in f.offsets {
                ssb_rom::sprite::decode(&file, at)
                    .unwrap_or_else(|e| panic!("{scene:?} {:#x}+{at:#x}: {e:?}", f.file));
            }
        }
        for &(id, at, luts) in scene.lut_sprites() {
            let file = archive.load(id).unwrap();
            for &lut in luts {
                let bytes = &file.data[lut as usize..lut as usize + 32];
                let s = ssb_rom::sprite::decode_with_tlut(
                    &file,
                    at,
                    &ssb_rom::texture::parse_tlut(bytes),
                )
                .unwrap();
                assert_eq!((s.width, s.height), (48, 48));
            }
        }
    }
}

/// Ticks `menu` with `pad` once, then idles `n` tics, visiting each.
fn walk<M>(
    m: &mut M,
    drawn: &mut BTreeSet<Key>,
    pads: &[u16],
    idle: usize,
    mut tick: impl FnMut(&mut M, &Pad),
    visit: impl Fn(&M, &mut dyn FnMut(Draw)),
) {
    for &b in pads {
        tick(m, &tap(b));
        visit(m, &mut collect(drawn));
        for _ in 0..idle {
            tick(m, &Pad::default());
            visit(m, &mut collect(drawn));
        }
    }
}

#[test]
fn front_menus_draw_only_packed_sprites() {
    use ssb_game::menu::mode_select::ModeSelect;
    use ssb_game::menu::one_p_mode::{OnePMode, OnePOption};
    use ssb_game::menu::title::{DemoData, NoAnims, Title};

    let mut drawn = BTreeSet::new();
    let b = Backup::default();
    let mut demo = DemoData::default();
    for time in [0u8, 37, 200] {
        let mut t = Title::new(time);
        for i in 0..400u32 {
            t.tick(
                &Pad::default(),
                Scene::Startup,
                &mut demo,
                &b,
                (i * 13) as u8,
                &mut |r| r - 1,
            );
            t.visit(&NoAnims, &mut |d| collect(&mut drawn)(d));
        }
    }
    // The layout after the opening (RE-467): its logo tree places the
    // cutout and strikes, unit-scaled here.
    struct LogoAnims;
    impl ssb_game::menu::title::Anims for LogoAnims {
        fn labels(&self, _: usize, _: usize) -> Option<[f32; 4]> {
            None
        }
        fn press_start(&self, _: usize) -> Option<[f32; 4]> {
            None
        }
        fn logo(&self, _: usize, _: usize) -> Option<[f32; 4]> {
            Some([0.0, 0.0, 1.0, 1.0])
        }
    }
    let mut t = Title::new_opening(0);
    for i in 0..400u32 {
        t.tick(
            &Pad::default(),
            Scene::Opening(ssb_game::opening::Kind::Newcomers),
            &mut demo,
            &b,
            (i * 13) as u8,
            &mut |r| r - 1,
        );
        t.visit(&LogoAnims, &mut |d| collect(&mut drawn)(d));
    }
    check(MenuScene::Title, &drawn);

    let mut drawn = BTreeSet::new();
    let mut m = ModeSelect::new(Scene::Title);
    let down = N64Buttons::D_DOWN;
    walk(
        &mut m,
        &mut drawn,
        &[0, down, down, down, down],
        12,
        |m, p| {
            m.tick(p);
        },
        |m, f| m.visit(&mut |d| f(d)),
    );
    check(MenuScene::ModeSelect, &drawn);

    let mut drawn = BTreeSet::new();
    let mut m = OnePMode::new(Scene::ModeSelect, OnePOption::OnePGame);
    walk(
        &mut m,
        &mut drawn,
        &[0, down, down, down, down, N64Buttons::A],
        12,
        |m, p| {
            m.tick(p);
        },
        |m, f| m.visit(&mut |d| f(d)),
    );
    check(MenuScene::OnePMode, &drawn);
}

#[test]
fn vs_menus_draw_only_packed_sprites() {
    use ssb_game::menu::vs_item_switch::VsItemSwitchMenu;
    use ssb_game::menu::vs_options::VsOptionsMenu;
    use ssb_game::players_vs::BattleState;
    use ssb_game::vs_mode::VsMode;

    let (down, up, right, left) = (
        N64Buttons::D_DOWN,
        N64Buttons::D_UP,
        N64Buttons::D_RIGHT,
        N64Buttons::D_LEFT,
    );
    let mut drawn = BTreeSet::new();
    let mut state = BattleState::default();
    let mut m = VsMode::new(Scene::ModeSelect, &state);
    let mut pads = vec![0, down];
    // Every rule, then the time down to infinity and up past 9 and 10.
    pads.extend([right; 4]);
    pads.extend([left; 4]);
    pads.push(down);
    pads.extend([left; 4]);
    pads.extend([right; 14]);
    pads.extend([up, right, right, down]);
    pads.extend([right; 12]);
    pads.extend([down, up, up]);
    walk(
        &mut m,
        &mut drawn,
        &pads,
        31,
        |m, p| {
            m.tick(p, &mut state);
        },
        |m, f| m.visit(&mut |d| f(d)),
    );
    check(MenuScene::VsMode, &drawn);

    let mut drawn = BTreeSet::new();
    for unlock in [0u8, 0xFF] {
        let b = Backup {
            unlock_mask: unlock,
            ..Backup::default()
        };
        let mut state = BattleState::default();
        let mut m = VsOptionsMenu::new(Scene::VsMode, &state, &b);
        let mut pads = vec![0];
        for _ in 0..5 {
            pads.extend([right, right, right, left, left, left, down]);
        }
        walk(
            &mut m,
            &mut drawn,
            &pads,
            14,
            |m, p| {
                m.tick(p, &mut state);
            },
            |m, f| m.visit(&mut |d| f(d)),
        );
    }
    check(MenuScene::VsOptions, &drawn);

    let mut drawn = BTreeSet::new();
    let mut state = BattleState::default();
    let mut m = VsItemSwitchMenu::new(&state);
    // Every appearance rate, then each toggle off and on.
    let mut pads = vec![0, left, left, left];
    pads.extend([right; 5]);
    for _ in 0..20 {
        pads.extend([right, left, down]);
    }
    walk(
        &mut m,
        &mut drawn,
        &pads,
        14,
        |m, p| {
            m.tick(p, &mut state);
        },
        |m, f| m.visit(&mut |d| f(d)),
    );
    check(MenuScene::VsItemSwitch, &drawn);
}

#[test]
fn the_bonus_select_pack_holds_its_records() {
    let packed: BTreeSet<(u32, u32)> = MenuScene::Players1PBonus
        .sprites()
        .iter()
        .flat_map(|f| f.offsets.iter().map(|&o| (f.file, o)))
        .collect();
    let drawn: BTreeSet<(u32, u32)> = ssb_game::players_1p_bonus::layer::MENU_SPRITES
        .iter()
        .copied()
        .collect();
    assert_eq!(packed, drawn);
}

/// The title's baked plays: seven label children ending at their sprites'
/// places, and "Press Start" looping every 39 plays.
#[test]
fn the_title_bakes_its_label_and_press_start_plays() {
    let Some(path) = std::env::var_os("SSB64_ROM") else {
        return;
    };
    use ssb_rom::title;
    let data = std::fs::read(path).unwrap();
    let info = ssb_rom::rom::identify(&data).unwrap();
    let archive = ssb_rom::archive::Archive::open(&data, info.region).unwrap();
    let file = archive.load(title::FILE).unwrap();
    let pins: Vec<[f32; 2]> = title::LABEL_CENTRES
        .iter()
        .map(|c| title::pin(c[0], c[1]))
        .collect();
    let labels = title::bake(
        &file.data,
        title::LABELS_DOBJDESC,
        title::LABELS_ANIM_JOINT,
        &pins,
        title::LABEL_PLAYS,
    )
    .unwrap();
    assert_eq!(labels.len(), title::LABEL_PLAYS * title::LABEL_CHILDREN);
    let last = &labels[labels.len() - title::LABEL_CHILDREN..];
    for (i, v) in last.iter().take(5).enumerate() {
        assert_eq!([v[0], v[1], v[2], v[3]], [pins[i][0], pins[i][1], 1.0, 1.0]);
    }
    let c = title::PRESS_START_CENTRE;
    let n = 400;
    let press = title::bake(
        &file.data,
        title::PRESS_START_DOBJDESC,
        title::PRESS_START_ANIM_JOINT,
        &[title::pin(c[0], c[1])],
        n,
    )
    .unwrap();
    for play in 1..n {
        let looped = 1 + (play - 1) % title::PRESS_START_PERIOD;
        assert_eq!(press[play], press[looped], "play {play}");
        assert_eq!(
            ssb_game::menu::title::press_start_frame(play),
            looped,
            "the game's period is the build's"
        );
    }
    // Twenty plays shown, then nineteen at no size.
    assert!(press[1..=19].iter().all(|v| v[2] == 1.0));
    assert!(press[20..39].iter().all(|v| v[2] < 0.0001));
}

#[test]
fn sound_test_draws_only_packed_sprites() {
    use ssb_game::menu::sound_test::SoundTest;

    let mut drawn = BTreeSet::new();
    let mut m = SoundTest::new();
    // Every row, every digit at each place (up to 244), arrows shown.
    for row in 0..3 {
        for _ in 0..250 {
            m.tick(&tap(N64Buttons::D_RIGHT));
            m.change_wait = 0;
            m.visit(&mut collect(&mut drawn));
        }
        m.tick(&tap(N64Buttons::D_DOWN));
        m.change_wait = 0;
        assert_eq!(m.option, (row + 1) % 3);
    }
    check(MenuScene::SoundTest, &drawn);
}
