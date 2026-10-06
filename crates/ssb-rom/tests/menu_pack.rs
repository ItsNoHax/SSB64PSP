//! Every sprite the options and data menus draw is in its scene's menu
//! pack (`ssb_rom::menu_pack`) or the resident pack (RE-461), and every
//! menu pack sprite decodes from the ROM.

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
