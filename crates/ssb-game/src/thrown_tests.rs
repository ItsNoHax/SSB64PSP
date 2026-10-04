use super::*;
use crate::{
    combat::{self, AttackState},
    status::{self, Status, StatusTiming},
};
use ssb_engine::math::Vec3;

fn owner(kind: FighterKind) -> ThrowOwner {
    ThrowOwner {
        port: 0,
        kind,
        team: 2,
    }
}

fn body(kind: FighterKind, script_id: u8) -> Fighter {
    let mut f = Fighter::new(FighterKind::Mario, 1, 3);
    f.thrown.pending = Some((owner(kind), script_id));
    status::set_status(&mut f, Status::DamageFlyN, 0.0, StatusTiming::unknown());
    f
}

#[test]
fn thrown_motion_calls_the_throwers_script_before_time_zero_events() {
    for &victim in FighterKind::PLAYABLE {
        for &kind in FighterKind::PLAYABLE.iter().chain(
            [
                FighterKind::MetalMario,
                FighterKind::PolyMario,
                FighterKind::GiantDonkey,
            ]
            .iter(),
        ) {
            for script_id in 0..2 {
                let mut f = Fighter::new(victim, 1, 3);
                f.thrown.pending = Some((
                    ThrowOwner {
                        port: 0,
                        kind,
                        team: 2,
                    },
                    script_id,
                ));
                status::set_status(&mut f, Status::DamageFlyN, 0.0, StatusTiming::unknown());
                let coll = f.attack_colls[0];
                assert_eq!(
                    coll.state,
                    AttackState::New,
                    "{victim:?} by {kind:?}, row {script_id}"
                );
                let mario_back = matches!(
                    kind,
                    FighterKind::Mario | FighterKind::MetalMario | FighterKind::PolyMario
                ) && script_id == 1;
                assert_eq!(coll.damage, if mario_back { 8 } else { 6 });
                assert_eq!(coll.joint, 5);
                assert_eq!(coll.size, if mario_back { 100.0 } else { 80.0 });
                assert_eq!(
                    coll.offset,
                    if mario_back {
                        Vec3::ZERO
                    } else {
                        Vec3::new(0.0, 80.0, 0.0)
                    }
                );
                assert!(!coll.can_rebound);
                assert_eq!(f.thrown.owner.unwrap().port, 0);
                assert!(f.thrown.pending.is_none());
                assert_eq!(f.motion_script.threads[0].pc, crate::motion::NO_SCRIPT);
            }
        }
    }
}

#[test]
fn ordinary_damage_and_forwarded_events_do_not_make_thrown_attacks() {
    let mut f = Fighter::new(FighterKind::Fox, 1, 3);
    status::set_status(&mut f, Status::DamageFlyN, 0.0, StatusTiming::unknown());
    assert!(f.attack_colls.iter().all(|c| c.state == AttackState::Off));
    f.thrown.pending = Some((owner(FighterKind::Mario), 1));
    status::set_status(&mut f, Status::DamageFlyN, 4.0, StatusTiming::unknown());
    assert!(f.attack_colls.iter().all(|c| c.state == AttackState::Off));
    crate::motion::forward_effect(&mut f);
    assert!(f.attack_colls.iter().all(|c| c.state == AttackState::Off));
}

#[test]
fn throw_pointer_preservation_is_explicit_and_callback_is_one_shot() {
    let mut f = body(FighterKind::Mario, 1);
    let o = f.thrown.owner;
    status::set_any_status_preserve(
        &mut f,
        Status::DamageFlyN.into(),
        0.0,
        StatusTiming::unknown(),
        status::Preserve {
            throw_pointer: true,
            ..status::Preserve::NONE
        },
    );
    assert_eq!(f.thrown.owner, o);
    assert_eq!(f.attack_colls[0].damage, 8);
    status::set_status(&mut f, Status::DamageFlyN, 0.0, StatusTiming::unknown());
    assert_eq!(f.thrown.owner, None);
    assert!(f.attack_colls.iter().all(|c| c.state == AttackState::Off));
}

#[test]
fn thrown_attack_speed_gate_uses_three_axes_before_velocity_decay() {
    let mut f = body(FighterKind::Fox, 0);
    f.pos.y = 1000.0;
    f.hitstun = 100;
    f.physics.vel_knockback = Vec3::new(0.0, 0.0, 70.0);
    damage_physics(&mut f);
    assert_ne!(f.attack_colls[0].state, AttackState::Off);
    f.physics.vel_knockback = Vec3::new(70.0, 0.0, 0.0);
    f.tick(core::iter::empty::<(u16, crate::collision::Segment)>);
    assert!(f.physics.vel_knockback.length() < 70.0);
    assert_ne!(f.attack_colls[0].state, AttackState::Off);
    f.tick(core::iter::empty::<(u16, crate::collision::Segment)>);
    assert_eq!(f.attack_colls[0].state, AttackState::Off);
    assert_eq!(f.thrown.owner, Some(owner(FighterKind::Fox)));
}

#[test]
fn thrown_body_hits_credit_thrower_team_player_and_stale_queue() {
    for (team, rules, lands) in [
        (2, crate::team::TeamRules::TEAMS, false),
        (1, crate::team::TeamRules::TEAMS, true),
        (2, crate::team::TeamRules::FREE_FOR_ALL, true),
    ] {
        let mut thrower = Fighter::new(FighterKind::Mario, 0, 3);
        thrower.team = 2;
        let mut thrown = body(FighterKind::Mario, 1);
        thrown.team = 1;
        let mut victim = Fighter::new(FighterKind::Fox, 2, 3);
        victim.team = team;
        combat::search_all(&mut [&mut thrower, &mut thrown, &mut victim], rules);
        assert_eq!(thrower.hits.damage_queue, 0);
        assert_eq!(victim.hits.damage_queue, if lands { 8 } else { 0 });
        if lands {
            assert_eq!(
                victim.hits.log[0].unwrap().attacker,
                combat::DamageBy::Player(0)
            );
            assert_eq!(thrower.stale.next, 1);
            assert_eq!(
                thrower.stale.entries[0],
                (thrown.motion.attack_id, thrown.motion.count)
            );
            assert_eq!(thrown.stale.next, 0);
            combat::finish_frame(&mut [&mut thrower, &mut thrown, &mut victim]);
            assert_eq!(victim.damage_player, Some(0));
            assert_eq!(victim.damage, 8);
            assert!(thrown.hitlag > 0);
            assert_eq!(thrower.hitlag, 0);
        }
    }
}

#[test]
fn two_thrown_bodies_do_not_alias_their_local_motion_counts() {
    let mut queue = crate::stale::StaleQueue::default();
    let id = crate::stale::MotionAttackId::None;
    queue.push_thrown(id, 5, 1);
    queue.push_thrown(id, 5, 2);
    assert_eq!(queue.next, 2);
    queue.push_thrown(id, 5, 1);
    assert_eq!(queue.next, 2);
}

#[test]
fn body_attacks_spare_throwers_and_team_weapons_and_items_in_clanks() {
    use crate::team::TeamRules;
    use crate::weapon::{WeaponKind, WeaponPool, WeaponSpawn};
    for (port, team, rules, spared) in [
        (0, 3, TeamRules::FREE_FOR_ALL, true),
        (2, 2, TeamRules::TEAMS, true),
        (2, 3, TeamRules::TEAMS, false),
        (2, 2, TeamRules::FREE_FOR_ALL, false),
    ] {
        let mut f = body(FighterKind::Fox, 0);
        f.team = 4;
        combat::update_attack_positions(&mut f);
        let pos = f.attack_colls[0].pos_curr;
        let mut weapons = WeaponPool::default();
        weapons.team_rules = rules;
        assert!(weapons.spawn(WeaponSpawn {
            kind: WeaponKind::MarioFireball,
            owner_port: port,
            team,
            stale: crate::stale::WeaponStale::FRESH,
            position: pos,
            facing: 1.0
        }));
        weapons.apply_hits(&mut f);
        assert_eq!(
            f.attack_colls[0]
                .record(combat::WEAPON_RECORD_BASE)
                .group_id
                == combat::NO_GROUP,
            spared
        );

        let mut f = body(FighterKind::Fox, 0);
        f.team = 4;
        combat::update_attack_positions(&mut f);
        let pos = f.attack_colls[0].pos_curr;
        let mut items = crate::item::ItemPool::default();
        items.team_rules = rules;
        let slot = items
            .spawn_container(crate::item::container::Kind::Capsule, pos, Vec3::ZERO)
            .unwrap();
        let item = items.get_mut(slot).unwrap();
        item.owner = Some(port);
        item.team = team;
        item.attack.state = AttackState::Transfer;
        item.attack.can_setoff = true;
        item.attack.count = 1;
        item.attack.pos[0] = crate::item::ItemAttackPos {
            pos_curr: pos,
            pos_prev: pos,
        };
        items.search_fighter(&mut f);
        assert_eq!(
            f.attack_colls[0]
                .record(crate::item::ITEM_RECORD_BASE + slot)
                .group_id
                == combat::NO_GROUP,
            spared
        );
    }
}
