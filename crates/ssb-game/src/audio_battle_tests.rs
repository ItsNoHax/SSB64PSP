//! The in-battle sound calls, recorded (`sound::testing::Recorder`): the
//! battle's pause, timer and end, the countdown, the item music, the
//! crowd, the hit sounds and the weapons' sound slots.

use crate::battle::{Battle, Frame, GameStatus, Player, Rule};
use crate::fighter::{Fighter, FighterKind};
use crate::sound::id::*;
use crate::sound::testing::{Call, Recorder};

fn two_players() -> [Player; 4] {
    let mut p = [Player::default(); 4];
    p[0] = Player {
        present: true,
        is_human: true,
        team: 0,
        ..Player::default()
    };
    p[1] = Player {
        present: true,
        team: 1,
        ..Player::default()
    };
    p
}

fn go_battle(minutes: u8) -> Battle {
    let mut b = Battle::new(Rule::Time, minutes, 2, two_players());
    while b.status == GameStatus::Wait {
        b.begin_frame();
    }
    // The first Go frame reads no time.
    b.begin_frame();
    b
}

fn plays(calls: &[Call]) -> Vec<u16> {
    calls
        .iter()
        .filter_map(|c| match c {
            Call::PlayFgm(id) => Some(*id),
            _ => None,
        })
        .collect()
}

#[test]
fn a_pause_holds_the_fgms_then_chimes_and_halves_the_music() {
    let r = Recorder::install();
    let mut b = go_battle(3);
    r.take();
    b.pause();
    assert_eq!(
        r.take(),
        [
            Call::PauseFgm,
            Call::PlayFgm(nSYAudioFGMGamePause),
            Call::SetBgmVolume(0, 0x3C00),
        ]
    );
    for _ in 0..10 {
        assert_eq!(b.begin_frame(), Frame::Frozen);
    }
    assert!(r.take().is_empty());
    // The held FGMs and the volume return as Go does, after the camera.
    b.unpause(true);
    for _ in 0..20 {
        b.begin_frame();
        assert!(r.take().is_empty());
    }
    assert_eq!(b.begin_frame(), Frame::Run);
    assert_eq!(r.take(), [Call::ResumeFgm, Call::SetBgmVolume(0, 0x7800)]);
}

#[test]
fn a_reset_stops_every_fgm_and_no_music() {
    let r = Recorder::install();
    let mut b = go_battle(3);
    b.pause();
    r.take();
    b.reset();
    assert_eq!(r.take(), [Call::StopAllFgm]);
}

#[test]
fn the_last_five_seconds_count_down_and_fade_the_music() {
    let r = Recorder::install();
    let mut b = go_battle(1);
    r.take();
    while b.time_remain > 301 {
        b.begin_frame();
    }
    assert!(r.take().is_empty());
    // `time_remain <= 300`: "five", and the music at full ramp.
    b.begin_frame();
    assert_eq!(b.time_remain, 300);
    assert_eq!(
        r.take(),
        [
            Call::PlayFgm(nSYAudioVoiceAnnounceFive),
            Call::SetBgmVolume(0, 30720),
        ]
    );
    // Every frame resets the volume; each second boundary says its number.
    let mut voices = vec![];
    while b.time_remain > 1 {
        b.begin_frame();
        let calls = r.take();
        let t = b.time_remain;
        let vol = ((t as f32 / 300.0) * 20480.0 + 10240.0) as u32;
        assert_eq!(calls.last(), Some(&Call::SetBgmVolume(0, vol)));
        for id in plays(&calls) {
            voices.push((t, id));
        }
    }
    assert_eq!(
        voices,
        [
            (240, nSYAudioVoiceAnnounceFour),
            (180, nSYAudioVoiceAnnounceThree),
            (120, nSYAudioVoiceAnnounceTwo),
            (60, nSYAudioVoiceAnnounceOne),
        ]
    );
    // Time up queues its voice; the next frame stops every FGM and plays
    // the queue (`ifCommonBattleInterfaceProcUpdate`).
    b.begin_frame();
    assert_eq!(b.time_remain, 0);
    assert_eq!(r.take(), [Call::SetBgmVolume(0, 10240)]);
    assert_eq!(b.begin_frame(), Frame::Frozen);
    assert_eq!(
        r.take(),
        [Call::StopAllFgm, Call::PlayFgm(nSYAudioVoiceAnnounceTimeUp)]
    );
}

#[test]
fn mushroom_kingdom_hurries_its_music_at_thirty_seconds() {
    let r = Recorder::install();
    crate::music::reset();
    crate::music::set_play_bgm(nSYAudioBGMInishie);
    let mut b = go_battle(1);
    b.gkind = Some(8);
    r.take();
    while b.time_remain > 1801 {
        b.begin_frame();
    }
    assert_eq!(crate::music::bgm_default(), nSYAudioBGMInishie);
    b.begin_frame();
    assert_eq!(crate::music::bgm_default(), nSYAudioBGMInishieHurry);
    // `ftParamTryUpdateItemMusic` with no item music plays the new default.
    crate::music::flush_update(core::iter::empty());
    assert_eq!(r.take(), [Call::PlayBgm(0, nSYAudioBGMInishieHurry)]);
    assert_eq!(crate::music::bgm_current(), nSYAudioBGMInishieHurry);
    b.begin_frame();
    crate::music::flush_update(core::iter::empty());
    assert!(r.take().is_empty());
}

#[test]
fn the_countdown_says_three_two_one_go() {
    let r = Recorder::install();
    let mut c = crate::countdown::Countdown::new();
    let sizes = [(16u16, 16u16); 64];
    let mut said = vec![];
    for tic in 1..=400u32 {
        c.tick(&sizes);
        for id in plays(&r.take()) {
            said.push((tic, id));
        }
    }
    let ids: Vec<u16> = said.iter().map(|&(_, id)| id).collect();
    assert_eq!(
        ids,
        [
            nSYAudioVoiceAnnounceThree,
            nSYAudioVoiceAnnounceTwo,
            nSYAudioVoiceAnnounceOne,
            nSYAudioVoiceAnnounceGo,
        ]
    );
    // A second apart.
    assert_eq!(said[1].0 - said[0].0, 60);
    assert_eq!(said[2].0 - said[1].0, 60);
    assert_eq!(said[3].0 - said[2].0, 60);
}

fn holding_hammer(port: u8) -> Fighter {
    let mut f = Fighter::new(FighterKind::Mario, port, 3);
    f.items.held = Some(crate::item::HeldItem {
        slot: 0,
        kind: crate::item::ItemKind::Equipment(crate::item::equipment::Kind::Hammer),
        ty: crate::item::ItemType::Swing,
        weight: crate::item::ItemWeight::Light,
    });
    f
}

#[test]
fn the_item_music_lengths_are_swapped_so_the_star_outlasts_the_hammer() {
    let r = Recorder::install();
    crate::music::reset();
    // `ftParamGetItemMusicLength`'s source bug, kept.
    assert_eq!(
        crate::music::item_music_length(nSYAudioBGMStar),
        crate::music::ITHAMMER_BGM_DURATION
    );
    assert_eq!(
        crate::music::item_music_length(nSYAudioBGMHammer),
        crate::music::ITSTAR_BGM_DURATION
    );
    crate::music::set_play_bgm(nSYAudioBGMCastle);
    crate::music::try_play_item_music(nSYAudioBGMHammer);
    crate::music::try_play_item_music(nSYAudioBGMStar);
    // The Hammer's music does not replace the Star's.
    crate::music::try_play_item_music(nSYAudioBGMHammer);
    assert_eq!(
        r.take(),
        [
            Call::PlayBgm(0, nSYAudioBGMCastle),
            Call::PlayBgm(0, nSYAudioBGMHammer),
            Call::PlayBgm(0, nSYAudioBGMStar),
        ]
    );
    // One fighter with a Hammer and one with a Star: the Star's plays on.
    let hammer = holding_hammer(0);
    let mut star = Fighter::new(FighterKind::Fox, 1, 3);
    star.star_invincible_frames = crate::music::ITSTAR_WARN_BEGIN_FRAME + 1;
    crate::music::try_update_item_music([&hammer, &star]);
    assert!(r.take().is_empty());
    // At the Star's warning frame the Hammer's music takes over, and with
    // neither the stage's returns.
    star.star_invincible_frames = crate::music::ITSTAR_WARN_BEGIN_FRAME;
    crate::music::try_update_item_music([&hammer, &star]);
    assert_eq!(r.take(), [Call::PlayBgm(0, nSYAudioBGMHammer)]);
    let plain = Fighter::new(FighterKind::Mario, 0, 3);
    crate::music::try_update_item_music([&plain, &star]);
    assert_eq!(r.take(), [Call::PlayBgm(0, nSYAudioBGMCastle)]);
}

#[test]
fn the_battle_scenes_start_and_leave_their_music_in_the_source_order() {
    let r = Recorder::install();
    crate::music::reset();
    crate::music::start_battle(nSYAudioBGMZebes);
    assert_eq!(
        r.take(),
        [
            Call::PlayBgm(0, nSYAudioBGMZebes),
            Call::PlayFgm(nSYAudioVoicePublicExcited),
        ]
    );
    crate::music::start_battle_prologue(nSYAudioBGMMetal);
    assert_eq!(r.take(), [Call::PlayFgm(nSYAudioFGMPublicPrologue)]);
    assert_eq!(crate::music::bgm_current(), nSYAudioBGMMetal);
    crate::music::start_training();
    assert_eq!(
        r.take(),
        [
            Call::PlayBgm(0, nSYAudioBGMTrainingMode),
            Call::StopAllFgm,
            Call::PlayFgm(nSYAudioVoicePublicExcited),
        ]
    );
    crate::music::leave_battle();
    assert_eq!(
        r.take(),
        [
            Call::StopBgmAll,
            Call::SetBgmVolume(0, 0x7800),
            Call::StopAllFgm
        ]
    );
    let f = Fighter::new(FighterKind::Mario, 0, 3);
    crate::music::boss_go([&f]);
    assert_eq!(r.take(), [Call::PlayBgm(0, nSYAudioBGMLast)]);
}

#[test]
fn a_hit_sound_pans_against_the_attackers_side() {
    let r = Recorder::install();
    let mut f = Fighter::new(FighterKind::Kirby, 0, 3);
    f.pos.x = 4000.0;
    // Punch, level 2: `bal = 4000 / 8000 * 60 = 30`, `64 - 30`.
    crate::fighter_sound::play_hit_sfx(&mut f, 0, 2);
    f.pos.x = -20000.0;
    // Bat, level 2, clamped to the far right.
    crate::fighter_sound::play_hit_sfx(&mut f, 7, 2);
    assert_eq!(
        r.take(),
        [
            Call::PlayFgmBalance(nSYAudioFGMPunchL, 34),
            Call::PlayFgmBalance(nSYAudioFGMBatHit, 124),
        ]
    );
}

#[test]
fn a_hit_stops_the_attackers_store_info_sound_first() {
    let r = Recorder::install();
    let mut f = Fighter::new(FighterKind::Mario, 0, 3);
    crate::fighter_sound::play_fgm_store_info(&mut f, nSYAudioFGMKickS);
    let h = f.sound.sfx.expect("played");
    crate::fighter_sound::play_hit_sfx(&mut f, 1, 0);
    assert_eq!(
        r.take(),
        [
            Call::PlayFgm(nSYAudioFGMKickS),
            Call::StopFgm(h),
            Call::PlayFgmBalance(nSYAudioFGMKickS, 64),
        ]
    );
}

#[test]
fn a_loop_slot_plays_only_when_empty_and_a_status_change_stops_it() {
    let r = Recorder::install();
    let mut f = Fighter::new(FighterKind::Samus, 0, 3);
    crate::fighter_sound::play_loop_sfx(&mut f, nSYAudioFGMSamusSpecialNCharge0);
    crate::fighter_sound::play_loop_sfx(&mut f, nSYAudioFGMSamusSpecialNCharge1);
    let h = f.sound.loop_sfx.expect("played");
    crate::status::set_status(
        &mut f,
        crate::status::Status::Wait,
        0.0,
        crate::status::StatusTiming::unknown(),
    );
    assert_eq!(
        r.take(),
        [
            Call::PlayFgm(nSYAudioFGMSamusSpecialNCharge0),
            Call::StopFgm(h),
        ]
    );
    assert!(f.sound.loop_sfx.is_none());
}

mod crowd {
    use super::*;
    use crate::public;

    fn fighters(attacker_damage: u16) -> (Fighter, Fighter) {
        let victim = Fighter::new(FighterKind::Fox, 0, 3);
        let mut attacker = Fighter::new(FighterKind::Mario, 1, 3);
        attacker.damage = attacker_damage;
        (victim, attacker)
    }

    fn step(victim: &Fighter, attacker: &Fighter) {
        public::proc_update(&[Some(victim), Some(attacker), None, None], -2000.0, true);
    }

    #[test]
    fn a_first_hard_hit_draws_a_damage_reaction_by_knockback() {
        for (kb, id) in [
            (105.0, nSYAudioVoicePublicDamageS),
            (135.0, nSYAudioVoicePublicDamageM),
            (165.0, nSYAudioVoicePublicDamageL),
        ] {
            let r = Recorder::install();
            public::make_actor();
            let (v, a) = fighters(0);
            public::common_check(Some(1), kb);
            step(&v, &a);
            assert_eq!(plays(&r.take()), [id]);
        }
        // Under 100 the crowd stays quiet.
        let r = Recorder::install();
        public::make_actor();
        let (v, a) = fighters(0);
        public::common_check(Some(1), 99.0);
        step(&v, &a);
        assert!(plays(&r.take()).is_empty());
    }

    #[test]
    fn a_combo_within_sixty_tics_gasps_claps_or_cheers() {
        for (kb, id) in [
            (105.0, nSYAudioVoicePublicGaspClap),
            (135.0, nSYAudioVoicePublicAmazed),
            (165.0, nSYAudioVoicePublicCheer),
        ] {
            let r = Recorder::install();
            public::make_actor();
            let (v, a) = fighters(0);
            public::common_check(Some(1), 100.0);
            step(&v, &a);
            r.take();
            public::common_check(Some(1), kb);
            step(&v, &a);
            let calls = r.take();
            assert_eq!(plays(&calls), [id]);
            // The previous reaction stops first (`ftPublicPlayCommon`).
            assert!(matches!(calls[0], Call::StopFgm(_)));
        }
    }

    #[test]
    fn a_fighter_at_100_percent_landing_a_combo_starts_the_chant() {
        let r = Recorder::install();
        public::make_actor();
        let (v, a) = fighters(120);
        public::common_check(Some(1), 100.0);
        step(&v, &a);
        r.take();
        public::common_check(Some(1), 140.0);
        step(&v, &a);
        // `ftPublicTryStartCall`: the call's opener, then the common
        // reaction stops.
        let calls = r.take();
        assert_eq!(plays(&calls), [nSYAudioVoicePublicAmazed]);
        assert!(matches!(calls.last(), Some(Call::StopFgm(_))));
        // While the opener plays the chant waits; once it ends, "Mario!".
        step(&v, &a);
        assert!(plays(&r.take()).is_empty());
        crate::sound::play_fgm(nSYAudioFGMGuardOn);
        r.take();
        step(&v, &a);
        assert_eq!(plays(&r.take()), [nSYAudioVoicePublicMario]);
    }

    #[test]
    fn the_defeated_announcements_wait_for_each_other() {
        let r = Recorder::install();
        public::make_actor();
        let (v, a) = fighters(0);
        public::defeated_add_id(nSYAudioVoiceAnnounceComputerPlayer);
        public::defeated_add_id(nSYAudioVoiceAnnounceDefeated);
        step(&v, &a);
        assert_eq!(plays(&r.take()), [nSYAudioVoiceAnnounceComputerPlayer]);
        step(&v, &a);
        assert!(plays(&r.take()).is_empty());
        // Another sound took the newest serial: the Recorder's "ended".
        crate::sound::play_fgm(nSYAudioFGMGuardOn);
        r.take();
        step(&v, &a);
        assert_eq!(plays(&r.take()), [nSYAudioVoiceAnnounceDefeated]);
    }

    #[test]
    fn only_the_twelve_fighters_have_a_chant() {
        assert!(public::has_call(FighterKind::Ness));
        assert!(!public::has_call(FighterKind::Boss));
    }
}
