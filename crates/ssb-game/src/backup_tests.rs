use super::*;
use crate::battle::{Battle, Player, Rule};
use crate::spgame::Unlock;

fn played() -> Backup {
    let mut b = Backup {
        unlock_mask: Unlock::Ness.mask() | Unlock::Inishie.mask(),
        fighter_mask: 1 << FighterKind::Ness as u16,
        characters_fkind: FighterKind::Ness,
        spgame_difficulty: Difficulty::VeryHard,
        spgame_stock_count: 4,
        screen_adjust_h: -3,
        screen_adjust_v: 5,
        ground_mask: 0x1FF,
        vs_itemswitch_battles: 42,
        vs_total_battles: 300,
        boot: 7,
        ..Backup::default()
    };
    b.spgame_records[11].spgame_hiscore = 1_234_567;
    b.spgame_records[11].bonus2_time = 4321;
    b.spgame_records[11].bonus1_task_count = 10;
    b.spgame_records[11].is_spgame_complete = true;
    b.vs_records[3].ko_count[7] = 99;
    b.vs_records[3].damage_taken = 777_777;
    b.vs_records[3].played_against[11] = 3;
    b
}

#[test]
fn the_default_lays_out_as_the_n64_struct() {
    let copy = Backup::default().encode();
    // `is_allow_screenflash`, stereo, the screen offsets, Mario, no
    // unlocks, no newcomers, Easy, two stocks.
    assert_eq!(&copy[0x450..0x45C], &[1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 1, 2]);
    // The first 1P record's Break the Targets time at +16 and Board the
    // Platforms time at +24 (`I_MIN_TO_TICS(60)`).
    assert_eq!(
        &copy[0x45C + 16..0x45C + 20],
        &BONUS_TIME_DEFAULT.to_be_bytes()
    );
    assert_eq!(
        &copy[0x45C + 24..0x45C + 28],
        &BONUS_TIME_DEFAULT.to_be_bytes()
    );
    // The signature at 0x5E4, the checksum after its padding.
    assert_eq!(&copy[0x5E4..0x5E6], &SIGNATURE.to_be_bytes());
    assert_eq!(&copy[0x5E8..], &checksum(&copy).to_be_bytes());
}

#[test]
fn a_copy_round_trips() {
    let b = played();
    assert_eq!(Backup::decode(&b.encode()), Some(b));
}

#[test]
fn every_byte_before_the_checksum_is_covered() {
    let copy = played().encode();
    for i in [0, 0x451, 0x457, 0x5DC, 0x5E7] {
        let mut bad = copy;
        bad[i] ^= 0x40;
        assert_eq!(Backup::decode(&bad), None, "byte {i:#x}");
    }
}

#[test]
fn the_signature_must_hold() {
    let mut b = played();
    b.signature = 0;
    assert_eq!(Backup::decode(&b.encode()), None);
}

#[test]
fn an_empty_sram_falls_back_to_the_defaults_and_writes_them() {
    for image in [None, Some(&[][..]), Some(&[0u8; IMAGE_SIZE][..])] {
        let (b, loaded) = load(image);
        assert_eq!(loaded, Loaded::Defaults);
        assert_eq!(b.writes, 1);
        assert_eq!(Backup { writes: 0, ..b }, Backup::default());
    }
}

#[test]
fn a_bad_first_copy_is_restored_from_the_second() {
    let b = played();
    let mut image = b.image();
    assert_eq!(load(Some(&image)), (b.clone(), Loaded::First));
    image[0x10] ^= 1;
    let (loaded, from) = load(Some(&image));
    assert_eq!(from, Loaded::Second);
    assert_eq!(loaded.writes, 1);
    assert_eq!(
        Backup {
            writes: 0,
            ..loaded
        },
        b
    );
    image[COPY_OFFSET + 0x10] ^= 1;
    assert_eq!(load(Some(&image)).1, Loaded::Defaults);
}

#[test]
fn the_clears_reset_their_own_fields() {
    let mut b = played();
    b.unlock_mask |= Unlock::ItemSwitch.mask();
    b.clear_newcomers();
    assert_eq!(
        b.unlock_mask,
        Unlock::Inishie.mask() | Unlock::ItemSwitch.mask()
    );
    assert_eq!(b.fighter_mask, 0);
    b.clear_prize();
    assert_eq!(
        (b.unlock_mask, b.ground_mask, b.vs_itemswitch_battles),
        (0, 0, 0)
    );
    assert_eq!(b.vs_total_battles, 300);
    b.clear_vs_record();
    assert_eq!(b.vs_records[3], VsRecord::default());
    assert_eq!(b.vs_total_battles, 0);
    b.clear_1p_high_score();
    let r = b.spgame_records[11];
    assert_eq!((r.spgame_hiscore, r.is_spgame_complete), (0, false));
    assert_eq!((r.bonus1_task_count, r.bonus2_time), (10, 4321));
    b.clear_bonus_stage_time();
    assert_eq!(b.spgame_records[11], Record::default());
    b.write();
    b.clear_all_data();
    assert_eq!(
        b,
        Backup {
            writes: 1,
            ..Backup::default()
        }
    );
}

#[test]
fn correcting_errors_drops_locked_selections() {
    let mut b = Backup {
        characters_fkind: FighterKind::Luigi,
        ..Backup::default()
    };
    let mut s = Selections {
        fkind: Some(FighterKind::Ness),
        training_man_fkind: Some(FighterKind::Fox),
        training_com_fkind: Some(FighterKind::Purin),
        maps_vsmode_gkind: crate::stage_select::gkind::INISHIE,
        maps_training_gkind: 3,
        ..Selections::default()
    };
    s.players[2] = SelectedPlayer {
        fkind: Some(FighterKind::Captain),
        is_man: false,
    };
    b.correct_errors(&mut s);
    assert_eq!(b.characters_fkind, FighterKind::Mario);
    assert_eq!(
        (s.fkind, s.training_man_fkind, s.training_com_fkind),
        (None, Some(FighterKind::Fox), None)
    );
    assert_eq!(
        s.players[2],
        SelectedPlayer {
            fkind: None,
            is_man: true
        }
    );
    assert_eq!(
        (s.maps_vsmode_gkind, s.maps_training_gkind),
        (crate::stage_select::gkind::CASTLE, 3)
    );
    assert!(s.items_reset);
}

#[test]
fn the_boot_count_wraps_and_writes() {
    let mut b = Backup {
        boot: 255,
        ..Backup::default()
    };
    b.count_boot();
    assert_eq!((b.boot, b.writes), (0, 1));
}

fn vs_battle() -> Battle {
    let present = Player {
        present: true,
        ..Player::default()
    };
    let mut b = Battle::new(
        Rule::Time,
        2,
        0,
        [present, present, present, Player::default()],
    );
    b.time_passed = 125 * 60;
    b.players[0].kos = [0, 2, 1, 0];
    b.players[0].total_damage_given = 150;
    b.players[0].total_damage_all = 80;
    b.players[1].self_destructs = 1;
    b.players[1].total_damage_all = 999_990;
    b
}

#[test]
fn vs_results_record_each_fighter_against_the_others() {
    let mut backup = Backup::default();
    backup.vs_records[FighterKind::Fox as usize].damage_taken = 20;
    let kinds = [
        Some(FighterKind::Mario),
        Some(FighterKind::Fox),
        Some(FighterKind::Mario),
        None,
    ];
    crate::results::save_backup(&mut backup, &vs_battle(), kinds, 3);
    assert_eq!(backup.writes, 1);
    assert_eq!(
        (backup.vs_total_battles, backup.vs_itemswitch_battles),
        (1, 1)
    );
    assert_eq!(backup.ground_mask, 1 << 3);
    let mario = backup.vs_records[0];
    // Two Marios: both add their battle to Mario's record.
    assert_eq!(mario.games_played, 2);
    assert_eq!(mario.time_used, 250);
    assert_eq!((mario.damage_given, mario.damage_taken), (150, 80));
    assert_eq!(mario.player_count_tally, 6);
    // Port 0's KOs on Fox (port 1) and Mario (port 2).
    assert_eq!((mario.ko_count[0], mario.ko_count[1]), (1, 2));
    assert_eq!((mario.played_against[0], mario.played_against[1]), (2, 2));
    assert_eq!(mario.player_count_tallies[1], 6);
    let fox = backup.vs_records[1];
    assert_eq!((fox.games_played, fox.selfdestructs), (1, 1));
    assert_eq!(fox.damage_taken, 999_999);
    assert_eq!(fox.played_against[0], 2);
}

#[test]
fn vs_results_unlock_the_item_switch_then_mushroom_kingdom() {
    let mut b = Backup {
        vs_itemswitch_battles: 99,
        ..Backup::default()
    };
    assert_eq!(crate::results::unlocks(&b), [None, None]);
    b.vs_itemswitch_battles = 100;
    assert_eq!(
        crate::results::unlocks(&b),
        [Some(Unlock::ItemSwitch), None]
    );
    b.ground_mask = GROUND_MASK_ALL;
    for (i, r) in b.spgame_records.iter_mut().enumerate() {
        r.is_spgame_complete = CHARACTER_MASK_STARTER & (1 << i) != 0;
    }
    assert_eq!(
        crate::results::unlocks(&b),
        [Some(Unlock::ItemSwitch), Some(Unlock::Inishie)]
    );
    b.unlock_mask = Unlock::ItemSwitch.mask();
    assert_eq!(crate::results::unlocks(&b), [Some(Unlock::Inishie), None]);
    b.spgame_records[0].is_spgame_complete = false;
    assert_eq!(crate::results::unlocks(&b), [None, None]);
}

#[test]
fn the_unlock_masks_match_lbdef() {
    assert_eq!(UNLOCK_MASK_NEWCOMERS, 0x0F);
    assert_eq!(UNLOCK_MASK_PRIZE, 0x70);
    assert_eq!(
        crate::stage_select::UNLOCK_MASK_INISHIE,
        Unlock::Inishie.mask()
    );
    assert_eq!(CHARACTER_MASK_STARTER, 0x0FFF & !0x0C90);
}

#[test]
fn a_reloaded_save_unlocks_the_newcomers_and_mushroom_kingdom_on_the_selects() {
    use crate::fighter_select::is_locked;
    use crate::stage_select::{gkind, StageSelect};
    let mut b = Backup::default();
    let newcomers = [
        FighterKind::Luigi,
        FighterKind::Captain,
        FighterKind::Purin,
        FighterKind::Ness,
    ];
    for k in newcomers {
        assert!(is_locked(k, b.fighter_mask));
    }
    assert!(StageSelect::new(gkind::CASTLE, b.unlock_mask).is_locked(gkind::INISHIE));
    for u in [
        Unlock::Luigi,
        Unlock::Ness,
        Unlock::Captain,
        Unlock::Purin,
        Unlock::Inishie,
    ] {
        crate::spgame::message::apply_unlock(&mut b, u);
    }
    let (loaded, from) = load(Some(&b.image()));
    assert_eq!(from, Loaded::First);
    for k in newcomers {
        assert!(!is_locked(k, loaded.fighter_mask), "{k:?}");
    }
    assert!(!StageSelect::new(gkind::CASTLE, loaded.unlock_mask).is_locked(gkind::INISHIE));
    // Luigi's unlock alone is not Mushroom Kingdom's.
    let luigi = Backup {
        unlock_mask: Unlock::Luigi.mask(),
        ..Backup::default()
    };
    assert!(StageSelect::new(gkind::CASTLE, luigi.unlock_mask).is_locked(gkind::INISHIE));
}

#[test]
fn a_vs_battle_totals_damage_taken_and_given_to_other_players() {
    let present = Player {
        present: true,
        ..Player::default()
    };
    let mut b = Battle::new(
        Rule::Stock,
        0,
        3,
        [present, present, Player::default(), Player::default()],
    );
    let mut f = crate::fighter::Fighter::new(FighterKind::Fox, 1, 3);
    f.stats.enable();
    f.record_combo_damage(Some(0), 12);
    f.add_damage(12);
    // A self or stage hit is damage taken, not given.
    f.record_combo_damage(Some(1), 5);
    f.record_combo_damage(None, 5);
    f.add_damage(5);
    b.collect_damage(&mut f);
    assert_eq!(
        (
            b.players[0].total_damage_given,
            b.players[0].total_damage_all
        ),
        (12, 0)
    );
    assert_eq!(
        (
            b.players[1].total_damage_given,
            b.players[1].total_damage_all
        ),
        (0, 17)
    );
    // Drained.
    b.collect_damage(&mut f);
    assert_eq!(b.players[1].total_damage_all, 17);
}
