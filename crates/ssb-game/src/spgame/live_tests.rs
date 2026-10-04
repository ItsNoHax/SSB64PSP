use super::{
    bonus::{DamageObject, HitAttackId},
    live::*,
    session::Session,
    *,
};
use crate::{
    attack,
    combat::{self, DamageBy},
    fighter::{Fighter, FighterKind},
    status::{self, AnyStatus, Status},
};
use ssb_engine::math::Vec3;

fn session(stage: Stage) -> (Session, Backup) {
    let mut backup = Backup::default();
    let mut s = Session::new(
        SceneData {
            stage: stage as u8,
            ..Default::default()
        },
        &backup,
    );
    s.manager.advance(&mut s.data, &mut s.state, &mut backup);
    s.start_battle(&backup);
    (s, backup)
}
fn fighter(s: &mut Session, port: u8) -> Fighter {
    let p = s.state.players[port as usize];
    let mut f = Fighter::new(p.fkind, port, p.stock_count);
    s.configure_fighter(
        &mut f,
        status::BlastZone {
            top: 1000.0,
            bottom: -1000.0,
            left: -1000.0,
            right: 1000.0,
        },
    );
    f
}
fn stat(id: HitAttackId, count: u16) -> AttackStat {
    AttackStat {
        flags: Flags(id as u16),
        count,
    }
}
fn hit(f: &mut Fighter, damage: i32, player: DamageBy, stat: AttackStat) {
    assert!(combat::direct_hit_with_stat(
        f,
        attack::Hitbox {
            damage,
            ..attack::MARIO_JAB1_HITBOX
        },
        Vec3::ZERO,
        1.0,
        9,
        player,
        stat,
        DamageObject::Other
    ));
}

#[test]
fn global_stat_sequence_wraps_without_returning_zero() {
    reset_count();
    for expected in 1..=u16::MAX {
        assert_eq!(next_count(), expected);
    }
    assert_eq!(next_count(), 1);
}

#[test]
fn unchanged_stat_id_retains_flags_count_and_aerial_landing_does_not_recount() {
    let (mut s, _) = session(Stage::Link);
    let mut f = fighter(&mut s, 0);
    status::set_any_status(
        &mut f,
        Status::AttackAirN.into(),
        0.0,
        status::StatusTiming::unknown(),
    );
    let air = f.stats.attack;
    assert!(air.flags.air());
    status::set_any_status(
        &mut f,
        Status::LandingAirN.into(),
        0.0,
        status::StatusTiming::unknown(),
    );
    assert_eq!(f.stats.attack, air);
    s.collect_fighter(&mut f);
    let b = &s.game.as_ref().unwrap().bonus;
    assert_eq!(b.attack_id_count[HitAttackId::AttackAirN as usize], 1);
    assert_eq!(b.attack_ground_air, [0, 1]);
    status::set_wait(&mut f);
    s.collect_fighter(&mut f);
    assert_eq!(
        s.game.as_ref().unwrap().bonus.attack_ground_air,
        [0, 1],
        "leaving an attack does not count the old ID"
    );
}

#[test]
fn jab_proc_status_and_final_status_update_both_count_initial_entry() {
    let (mut s, _) = session(Stage::Link);
    let mut f = fighter(&mut s, 0);
    status::set_attack11(&mut f);
    let first = f.stats.attack.count;
    s.collect_fighter(&mut f);
    assert_eq!(s.game.as_ref().unwrap().bonus.attack_id_count[1], 2);
    status::set_attack11(&mut f);
    assert_ne!(f.stats.attack.count, first);
    s.collect_fighter(&mut f);
    assert_eq!(s.game.as_ref().unwrap().bonus.attack_id_count[1], 3);
}

#[test]
fn damage_amounts_survive_percent_cap_and_healing_without_fabricating_credit() {
    let (mut s, _) = session(Stage::Link);
    let mut f = fighter(&mut s, 1);
    f.damage = 995;
    hit(
        &mut f,
        12,
        DamageBy::Player(0),
        stat(HitAttackId::AttackS4, 3),
    );
    combat::resolve(&mut f);
    assert_eq!(f.damage, 999);
    s.collect_fighter(&mut f);
    assert_eq!(s.state.players[1].total_damage_all, 12);
    assert_eq!(s.state.players[0].total_damage_given, 12);
    assert_eq!(s.state.players[1].stock_damage_all, 999);
    crate::colanim::set_heal_damage(&mut f, 2);
    f.tick_timers();
    s.collect_fighter(&mut f);
    assert_eq!(s.state.players[1].stock_damage_all, 998);
    assert_eq!(s.state.players[1].total_damage_all, 12);
    // Rebirth resets fighter fields but retains the campaign attachment.
    f.dead.bounds = Some(crate::dead::StageBounds {
        map: status::BlastZone {
            top: 1000.0,
            bottom: -1000.0,
            left: -1000.0,
            right: 1000.0,
        },
        camera: status::BlastZone {
            top: 1000.0,
            bottom: -1000.0,
            left: -1000.0,
            right: 1000.0,
        },
        rebirth: ssb_engine::math::Vec2::ZERO,
        fog_color: [0; 3],
    });
    crate::dead::rebirth_down(&mut f, 0);
    s.collect_fighter(&mut f);
    assert_eq!(s.state.players[1].stock_damage_all, 0);
    assert_eq!(s.state.players[1].total_damage_all, 12);
    assert_eq!(s.state.players[1].total_damage_players[0], 12);
    assert!(f.dead.spgame_rule);
    assert!(f.dead.team_bounds.is_some());
    f.add_damage(3);
    s.collect_fighter(&mut f);
    assert_eq!(s.state.players[1].total_damage_all, 15);
}

#[test]
fn accepted_contacts_credit_each_attacker_beyond_the_ten_hit_log_limit() {
    let (mut s, _) = session(Stage::Yoshi);
    let mut f = fighter(&mut s, 1);
    for n in 0..12 {
        hit(
            &mut f,
            2,
            DamageBy::Player(if n < 7 { 0 } else { 2 }),
            stat(HitAttackId::Attack11, 10),
        );
    }
    assert_eq!(f.hits.log_len, combat::HIT_LOG_MAX);
    combat::resolve(&mut f);
    s.collect_fighter(&mut f);
    assert_eq!(s.state.players[1].total_damage_all, 24);
    assert_eq!(s.state.players[0].total_damage_given, 14);
    assert_eq!(s.state.players[2].total_damage_given, 10);
    assert_eq!(s.state.players[1].total_damage_players, [14, 0, 10, 0]);
    assert_eq!(
        s.game.as_ref().unwrap().bonus.defend_id_count[1],
        1,
        "only the winning knockback log registers defend stats"
    );
}

#[test]
fn resistance_credits_only_overflow_and_world_or_self_hits_have_no_given_damage() {
    let (mut s, _) = session(Stage::Link);
    let mut f = fighter(&mut s, 1);
    f.kirby.is_damage_resist = true;
    f.kirby.damage_resist = 2;
    hit(
        &mut f,
        5,
        DamageBy::Player(0),
        stat(HitAttackId::Attack11, 10),
    );
    f.kirby.is_damage_resist = false;
    hit(&mut f, 4, DamageBy::World, stat(HitAttackId::AttackS4, 11));
    hit(
        &mut f,
        1,
        DamageBy::Player(1),
        stat(HitAttackId::AttackS4, 12),
    );
    combat::resolve(&mut f);
    s.collect_fighter(&mut f);
    assert_eq!(s.state.players[1].total_damage_all, 8);
    assert_eq!(s.state.players[0].total_damage_given, 3);
    assert_eq!(s.state.players[1].total_damage_given, 0);
}

#[test]
fn duplicate_stat_keeps_flags_but_changes_hit_owner_and_object_every_time() {
    let (mut s, _) = session(Stage::Link);
    let mut f = fighter(&mut s, 1);
    super::live::hit(
        &mut f,
        DamageBy::Player(0),
        stat(HitAttackId::AttackS4, 7),
        DamageObject::Other,
    );
    super::live::hit(
        &mut f,
        DamageBy::Player(2),
        stat(HitAttackId::SpecialN, 7),
        DamageObject::PokemonWeapon,
    );
    assert_eq!(f.damage_player, Some(2));
    assert_eq!(f.stats.damage.flags.id(), HitAttackId::AttackS4);
    assert_eq!(f.stats.damage_object, DamageObject::PokemonWeapon);
    assert_eq!(f.stats.damage_count, 2);
    s.collect_fighter(&mut f);
    assert_eq!(
        s.game.as_ref().unwrap().bonus.defend_id_count[HitAttackId::AttackS4 as usize],
        1
    );
    assert_eq!(
        s.game.as_ref().unwrap().bonus.defend_id_count[HitAttackId::SpecialN as usize],
        0
    );
    super::live::hit(
        &mut f,
        DamageBy::Keep,
        stat(HitAttackId::None, 0),
        DamageObject::Acid,
    );
    assert_eq!(f.damage_player, Some(2));
    assert_eq!(f.stats.damage.flags.id(), HitAttackId::None);
    assert_eq!(f.stats.damage.count, 0);
    super::live::hit(
        &mut f,
        DamageBy::World,
        stat(HitAttackId::AttackS4, 8),
        DamageObject::Arwing,
    );
    assert_eq!(f.damage_player, None);
    assert_eq!(f.stats.damage, AttackStat::default());
}

#[test]
fn thrown_damage_uses_catcher_statistics_and_actual_amount_once() {
    let (mut s, _) = session(Stage::Link);
    let mut catcher = fighter(&mut s, 0);
    let mut held = fighter(&mut s, 1);
    status::set_any_status(
        &mut catcher,
        Status::ThrowF.into(),
        0.0,
        status::StatusTiming::unknown(),
    );
    crate::grab::thrown_update_damage_stats(&mut held, &mut catcher);
    let damage = u32::from(held.damage);
    assert!(damage > 0);
    s.collect_fighter(&mut held);
    s.collect_fighter(&mut catcher);
    assert_eq!(s.state.players[1].total_damage_all, damage);
    assert_eq!(s.state.players[0].total_damage_given, damage);
    assert_eq!(held.damage_player, Some(0));
    assert_eq!(held.stats.damage, catcher.stats.attack);
    assert_eq!(
        s.game.as_ref().unwrap().bonus.defend_id_count[HitAttackId::ThrowF as usize],
        1
    );
}

#[test]
fn real_ko_callback_consumes_team_record_once_with_the_winning_attack() {
    let (mut s, _) = session(Stage::Yoshi);
    let mut enemy = fighter(&mut s, 1);
    hit(
        &mut enemy,
        10,
        DamageBy::Player(0),
        stat(HitAttackId::AttackS4, 21),
    );
    combat::resolve(&mut enemy);
    crate::dead::set_dead_down(&mut enemy);
    assert!(enemy.dead.scored);
    s.collect_fall(&mut enemy);
    s.collect_fall(&mut enemy);
    let b = &s.game.as_ref().unwrap().bonus;
    assert_eq!(b.defeat_count, 1);
    assert_eq!(b.defeats[0].attack_id, HitAttackId::AttackS4);
    assert_eq!(b.defeats[0].stat_count, 21);
    assert_eq!(b.defeats[0].damage_player, Some(0));
    assert_eq!(s.state.players[0].total_damage_given, 10);
    assert_eq!(s.battle.as_ref().unwrap().players[1].falls, 1);
}

#[test]
fn item_counts_saturate_and_cpu_consumption_does_not_change_human_bonus() {
    let (mut s, _) = session(Stage::Link);
    let mut f = fighter(&mut s, 0);
    for _ in 0..300 {
        f.stats
            .emit(Event::Item(crate::item::utility::Kind::Tomato));
    }
    s.collect_fighter(&mut f);
    let mut cpu = fighter(&mut s, 1);
    cpu.stats
        .emit(Event::Item(crate::item::utility::Kind::Heart));
    cpu.stats.emit(Event::Mew);
    s.collect_fighter(&mut cpu);
    let b = &s.game.as_ref().unwrap().bonus;
    assert_eq!(b.tomato_count, 255);
    assert_eq!(b.heart_count, 0);
    assert!(!b.mew_catcher);
}

#[test]
fn noncampaign_fighter_retains_hit_provenance_without_queuing_campaign_events() {
    let mut f = Fighter::new(FighterKind::Mario, 0, 2);
    status::set_attack11(&mut f);
    f.add_damage(7);
    f.record_combo_damage(Some(1), 7);
    assert_eq!(f.stats.drain().count(), 0);
    assert_ne!(f.stats.attack.count, 0);
}

#[test]
fn fighter_hit_clears_projectile_flag_while_body_and_shot_share_attack_identity() {
    let flags = status_flags(
        FighterKind::Mario,
        AnyStatus::Mario(status::MarioStatus::SpecialN),
    );
    assert_eq!(flags.id(), HitAttackId::SpecialN);
    assert!(flags.projectile());
    assert!(!flags.body().projectile());
    assert_eq!(flags.body().id(), flags.id());
}

#[test]
fn fired_weapon_keeps_makers_stat_after_maker_changes_status() {
    let (mut s, _) = session(Stage::Link);
    let mut maker = fighter(&mut s, 0);
    status::set_mario_special_n(&mut maker);
    let snapshot = maker.stats.attack;
    let mut pool = crate::weapon::WeaponPool::default();
    pool.spawn(crate::weapon::WeaponSpawn {
        kind: crate::weapon::WeaponKind::MarioFireball,
        owner_port: 0,
        team: maker.team,
        position: Vec3::ZERO,
        facing: 1.0,
        stale: crate::stale::WeaponStale::of(&maker),
    });
    status::set_wait(&mut maker);
    pool.tick(core::iter::empty::<crate::weapon::MapSurface>, None);
    let mut victim = fighter(&mut s, 1);
    victim.pos = pool.first_fireball().unwrap().position;
    pool.apply_hits(&mut victim);
    combat::resolve(&mut victim);
    assert!(victim.damage > 0);
    assert_eq!(victim.damage_player, Some(0));
    assert_eq!(victim.stats.damage, snapshot);
    s.collect_fighter(&mut victim);
    assert_eq!(
        s.state.players[0].total_damage_given,
        u32::from(victim.damage)
    );
    assert_eq!(s.game.as_ref().unwrap().bonus.defend_is_projectile, [0, 1]);
}

#[test]
fn reflected_weapon_uses_reflector_stat_and_credits_reflector() {
    let (mut s, _) = session(Stage::Link);
    let mut fox = Fighter::new(FighterKind::Fox, 0, 2);
    s.configure_fighter(
        &mut fox,
        status::BlastZone {
            top: 1000.0,
            bottom: -1000.0,
            left: -1000.0,
            right: 1000.0,
        },
    );
    status::set_fox_special_lw_start(&mut fox);
    status::set_any_status(
        &mut fox,
        AnyStatus::Fox(status::FoxStatus::SpecialLwLoop),
        0.0,
        status::StatusTiming::unknown(),
    );
    let reflection = fox.stats.attack;
    let mut pool = crate::weapon::WeaponPool::default();
    pool.spawn(crate::weapon::WeaponSpawn {
        kind: crate::weapon::WeaponKind::MarioFireball,
        owner_port: 1,
        team: 3,
        position: Vec3::new(100.0, 60.0, 0.0),
        facing: -1.0,
        stale: crate::stale::WeaponStale::FRESH,
    });
    pool.apply_hits(&mut fox);
    assert_eq!(pool.first_fireball().unwrap().owner_port, 0);
    let mut victim = fighter(&mut s, 1);
    victim.pos = pool.first_fireball().unwrap().position;
    pool.apply_hits(&mut victim);
    combat::resolve(&mut victim);
    assert!(victim.damage > 0);
    assert_eq!(victim.damage_player, Some(0));
    assert_eq!(victim.stats.damage, reflection);
    s.collect_fighter(&mut victim);
    assert!(s.game.as_ref().unwrap().bonus.defend_id_count[HitAttackId::SpecialLw as usize] > 0);
}

#[test]
fn real_consumable_star_and_mew_callbacks_reach_campaign_once() {
    use crate::item::{self, utility::Kind};
    let (mut s, _) = session(Stage::Link);
    let mut human = fighter(&mut s, 0);
    for kind in [Kind::Tomato, Kind::Heart] {
        human.items.held = Some(item::HeldItem {
            slot: 0,
            kind: item::ItemKind::Utility(kind),
            ty: item::ItemType::Consume,
            weight: item::ItemWeight::Light,
        });
        crate::item_throw::light_get_proc_damage(&mut human);
        crate::item_throw::light_get_proc_damage(&mut human);
    }
    let mut pool = item::ItemPool::default();
    let air = core::iter::empty::<crate::weapon::MapSurface>;
    let slot = pool
        .make_setup_common(6, None, Vec3::ZERO, Vec3::ZERO, &air)
        .unwrap();
    pool.get_mut(slot).unwrap().attack.state = combat::AttackState::Transfer;
    pool.search_fighter(&mut human);
    pool.search_fighter(&mut human);
    pool.spawn_mmonster(
        item::mmonster::Kind::Mew,
        Vec3::ZERO,
        Some(0),
        human.team,
        &air,
    )
    .unwrap();
    pool.sync_owner(&mut human);
    pool.sync_owner(&mut human);
    s.collect_fighter(&mut human);
    let b = &s.game.as_ref().unwrap().bonus;
    assert_eq!((b.tomato_count, b.heart_count, b.star_count), (1, 1, 1));
    assert!(b.mew_catcher);
    assert_eq!(s.state.players[0].total_damage_all, 0);
}

#[test]
fn shield_break_records_human_hit_owner_with_no_damage_percent() {
    let (mut s, _) = session(Stage::Link);
    let mut victim = fighter(&mut s, 1);
    status::set_guard_on(&mut victim);
    victim.guard.shield_player = Some(0);
    victim.hits.shield_damage = 60;
    victim.hits.shield_damage_total = 60;
    combat::proc_params(&mut victim);
    s.collect_fighter(&mut victim);
    assert_eq!(victim.status.status, Status::ShieldBreakFly);
    assert_eq!(victim.damage_player, Some(0));
    assert_eq!(victim.stats.damage.flags.id(), HitAttackId::None);
    assert!(s.game.as_ref().unwrap().bonus.shield_breaker);
    assert_eq!(s.state.players[1].total_damage_all, 0);
}

#[test]
fn shield_drain_clears_attack_identity_without_awarding_shield_breaker() {
    let (mut s, _) = session(Stage::Link);
    let mut f = fighter(&mut s, 1);
    f.guard.shield_player = Some(0);
    f.stats.damage = stat(HitAttackId::AttackS4, 19);
    f.hits.shield_damage = 0;
    status::set_shield_break_fly(&mut f);
    s.collect_fighter(&mut f);
    assert_eq!(f.damage_player, Some(0));
    assert_eq!(f.stats.damage, AttackStat::default());
    assert_eq!(f.stats.damage_count, 1);
    assert!(!s.game.as_ref().unwrap().bonus.shield_breaker);
}

#[test]
fn capture_preserves_last_hit_until_escape_resets_world_attribution() {
    let (mut s, _) = session(Stage::Link);
    let mut catcher = fighter(&mut s, 0);
    let mut held = fighter(&mut s, 1);
    held.damage_player = Some(2);
    held.stats.damage = stat(HitAttackId::AttackS4, 41);
    catcher.grab.send(crate::grab::GrabEvent::Capture);
    crate::grab::exchange(&mut catcher, &mut held);
    assert_eq!(held.stats.damage.count, 41);
    assert_eq!(held.stats.damage_count, 0);
    let holder = held.grab.holder.unwrap();
    crate::grab::apply_capture_knockback_with(&mut held, holder, (361, 80, 0, 20));
    s.collect_fighter(&mut held);
    assert_eq!(held.damage_player, None);
    assert_eq!(held.stats.damage_count, 1);
    assert_eq!(held.stats.damage, AttackStat::default());
    assert_eq!(s.state.players[1].total_damage_all, 0);
}

#[test]
fn yoshi_attribution_and_credit_begin_at_egg_lay() {
    let (mut s, _) = session(Stage::Link);
    let mut yoshi = fighter(&mut s, 0);
    let mut held = fighter(&mut s, 1);
    yoshi.kind = FighterKind::Yoshi;
    yoshi.stats.attack = stat(HitAttackId::SpecialN, 45);
    yoshi.grab.send(crate::grab::GrabEvent::CaptureYoshi);
    crate::grab::exchange(&mut yoshi, &mut held);
    assert_eq!(held.stats.damage_count, 0);
    held.egg.stage = 3;
    crate::capture_yoshi::update_held(&mut held, Vec3::ZERO);
    s.collect_fighter(&mut held);
    assert_eq!(held.stats.damage_count, 1);
    assert_eq!(held.stats.damage.count, 45);
    assert_eq!(held.damage_player, Some(0));
    assert_eq!(s.state.players[1].total_damage_all, 5);
    assert_eq!(s.state.players[0].total_damage_given, 5);
    assert_eq!(
        s.game.as_ref().unwrap().bonus.defend_id_count[HitAttackId::SpecialN as usize],
        1
    );
}
