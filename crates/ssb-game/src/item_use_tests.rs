use crate::{
    fighter::{Fighter, FighterKind},
    item::{
        equipment::{self, Kind},
        ItemPool, ItemRequest, ItemStatus,
    },
    item_use,
    status::{self, Status},
    weapon::WeaponPool,
};
use ssb_engine::math::Vec3;

#[test]
fn swing_physics_uses_authored_dash_and_character_smash_motion() {
    for kind in [
        FighterKind::Mario,
        FighterKind::Yoshi,
        FighterKind::Kirby,
        FighterKind::Purin,
    ] {
        let mut f = Fighter::new(kind, 0, 3);
        f.root_motion.delta.z = 23.0;
        status::set_status(
            &mut f,
            Status::SwordSwingDash,
            0.0,
            status::StatusTiming::unknown(),
        );
        assert!(item_use::apply_ground_physics(&mut f));
        assert_eq!(f.physics.vel_ground.x, 23.0 * f.facing.sign());
        status::set_status(
            &mut f,
            Status::SwordSwing4,
            0.0,
            status::StatusTiming::unknown(),
        );
        assert_eq!(
            item_use::apply_ground_physics(&mut f),
            kind != FighterKind::Mario
        );
    }
}

#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn ray_gun_tail_shield_precedes_head_hurtbox() {
    use crate::combat::{HitSource, WeaponAttack, WeaponContact};
    use crate::fighter::JointTransform;
    let mut target = Fighter::new(FighterKind::Mario, 1, 3);
    target.guard.is_shield = true;
    target.guard.shield_health = 55.0;
    target.joint_transforms[3] = Some(JointTransform {
        axes: [
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::new(0.0, 0.0, 1.0),
        ],
        origin: Vec3::new(800.0, 100.0, 0.0),
    });
    let shot = crate::monster_weapon::MonsterShot::equipment(
        crate::monster_weapon::ShotKind::RayGun,
        crate::monster_weapon::ShotParent::GROUND,
        Vec3::ZERO,
        false,
        0,
    );
    let head = Vec3::new(0.0, 100.0, 0.0);
    let tail = Vec3::new(800.0, 100.0, 0.0);
    let attack = WeaponAttack {
        stat: crate::spgame::live::AttackStat::default(),
        object: crate::spgame::bonus::DamageObject::Other,
        hitbox: shot.hitbox(),
        pos_curr: head,
        pos_prev: head,
        source: HitSource::Weapon { vel_x: 300.0 },
        handicap: 9,
        can_shield: true,
        owner: Some(0),
        is_hitlag_victim: None,
        fgm_id: None,
    };
    assert!(matches!(
        crate::combat::weapon_hit_pair(&mut target, attack, Some((tail, tail))),
        WeaponContact::Shielded(_)
    ));
    assert_eq!(target.hits.shield_damage, 10);
    crate::combat::resolve(&mut target);
    assert_eq!(target.damage, 0);
}

#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn ray_gun_second_box_hits_a_fighter_when_the_head_misses() {
    use crate::monster_weapon::{MonsterShot, ShotKind, ShotParent};
    let mut shot = MonsterShot::equipment(
        ShotKind::RayGun,
        ShotParent {
            stat: crate::spgame::live::AttackStat::default(),
            owner: Some(0),
            player: Some(0),
            team: 0,
            lr: 1.0,
            handle: 0,
        },
        Vec3::new(1000.0, 100.0, 0.0),
        false,
        0,
    );
    let head = shot.position;
    let tail = Vec3::new(0.0, 100.0, 0.0);
    shot.attack = Some((head, head));
    shot.attack_tail = Some((tail, tail));
    let mut pool = WeaponPool::default();
    pool.spawn_monster_shot(shot);
    let mut target = Fighter::new(FighterKind::Mario, 1, 3);
    pool.apply_hits(&mut target);
    crate::combat::resolve(&mut target);
    assert_eq!(target.damage, 10);
    assert_eq!(pool.monster_shots().count(), 0);
}

#[test]
fn fire_flower_rehits_an_item_after_sixteen_weapon_updates() {
    use crate::monster_weapon::{MonsterShot, ShotKind, ShotParent};
    let mut items = ItemPool::default();
    let slot = items
        .make_setup_common(1, None, Vec3::ZERO, Vec3::ZERO, &core::iter::empty)
        .unwrap();
    let item = items.get_mut(slot).unwrap();
    let mut shot = MonsterShot::equipment(
        ShotKind::FireFlower,
        ShotParent {
            stat: crate::spgame::live::AttackStat::default(),
            owner: Some(0),
            player: Some(0),
            team: 0,
            lr: 1.0,
            handle: 0,
        },
        Vec3::ZERO,
        false,
        0,
    );
    shot.velocity = Vec3::ZERO;
    let mut pool = WeaponPool::default();
    pool.spawn_monster_shot(shot);
    let id = crate::item::ITEM_RECORD_BASE + slot;
    pool.hit_item(item, id, &mut crate::wpeffect::Emit::default());
    assert_eq!(item.damage_queue, 3);
    item.damage_queue = 0;
    for _ in 0..15 {
        pool.tick(core::iter::empty, None);
        pool.hit_item(item, id, &mut crate::wpeffect::Emit::default());
    }
    assert_eq!(item.damage_queue, 0);
    pool.tick(core::iter::empty, None);
    pool.hit_item(item, id, &mut crate::wpeffect::Emit::default());
    assert_eq!(item.damage_queue, 3);
}

#[test]
fn hammer_walk_keeps_animation_hits_and_colour_clock() {
    let (mut f, _, _) = held(Kind::Hammer);
    item_use::hammer_wait(&mut f);
    crate::colanim::run_update(&mut f);
    let colour = f.colanim;
    f.status.set_time(13.0);
    f.stick.x = 20;
    f.attack_colls[0].damage = 17;
    item_use::update(&mut f);
    assert_eq!(f.status.status, Status::HammerWalk);
    assert_eq!(f.status.anim_frame, 13.0);
    assert_eq!(f.attack_colls[0].damage, 17);
    assert_eq!(f.colanim, colour);
}

#[test]
fn ray_gun_tail_grows_and_hop_and_reflection_reset_it() {
    use crate::monster_weapon::{MonsterShot, ShotKind, ShotParent};
    let parent = ShotParent {
        stat: crate::spgame::live::AttackStat::default(),
        handle: 0,
        owner: Some(0),
        player: Some(0),
        team: 0,
        lr: -1.0,
    };
    let mut m = MonsterShot::equipment(ShotKind::RayGun, parent, Vec3::ZERO, false, 0);
    assert_eq!(m.attack_tail.unwrap().0.x, -5.0);
    let mut fx = crate::wpeffect::Emit::default();
    assert!(m.tick(core::iter::empty, None, &mut fx));
    assert_eq!(m.scale_x, 11.0);
    assert!((m.attack_tail.unwrap().0.x - (m.position.x + 55.0)).abs() < 0.001);
    for _ in 0..10 {
        m.tick(core::iter::empty, None, &mut fx);
    }
    assert_eq!(m.scale_x, 160.0 / 3.0);
    m.hop(Vec3::new(300.0, 0.0, 0.0));
    assert_eq!(m.scale_x, 1.0);
    assert_eq!(m.lr, -1.0);
    m.scale_x = 30.0;
    m.reflect(1, 2, -1.0, &mut fx);
    assert_eq!(
        (m.owner, m.player, m.team, m.damage, m.scale_x),
        (Some(1), Some(1), 2, 18, 1.0)
    );
    assert_eq!(m.velocity.x, -300.0);
}

#[test]
fn star_expires_on_update_31_but_flame_on_30_and_reflection_renews_flame() {
    use crate::monster_weapon::{MonsterShot, ShotKind, ShotParent};
    let parent = ShotParent {
        stat: crate::spgame::live::AttackStat::default(),
        handle: 0,
        owner: Some(0),
        player: Some(0),
        team: 0,
        lr: 1.0,
    };
    let mut fx = crate::wpeffect::Emit::default();
    let mut star = MonsterShot::equipment(
        ShotKind::StarRod,
        parent,
        Vec3::new(0.0, 0.0, 40.0),
        false,
        0,
    );
    assert_eq!(star.position.z, 0.0);
    for _ in 0..30 {
        assert!(star.tick(core::iter::empty, None, &mut fx));
    }
    assert!(!star.tick(core::iter::empty, None, &mut fx));
    let mut flame = MonsterShot::equipment(ShotKind::FireFlower, parent, Vec3::ZERO, false, 0);
    for _ in 0..29 {
        assert!(flame.tick(core::iter::empty, None, &mut fx));
    }
    flame.reflect(1, 1, -1.0, &mut fx);
    assert_eq!(flame.lifetime, 30);
    for _ in 0..29 {
        assert!(flame.tick(core::iter::empty, None, &mut fx));
    }
    assert!(!flame.tick(core::iter::empty, None, &mut fx));
}

fn held(k: Kind) -> (Fighter, ItemPool, u8) {
    let mut f = Fighter::new(FighterKind::Mario, 0, 3);
    let mut p = ItemPool::default();
    let index = match k {
        Kind::Sword => 7,
        Kind::Bat => 8,
        Kind::Fan => 9,
        Kind::StarRod => 10,
        Kind::RayGun => 11,
        Kind::FireFlower => 12,
        Kind::Hammer => 13,
    };
    let slot = p
        .make_setup_common(index, None, Vec3::ZERO, Vec3::ZERO, &core::iter::empty)
        .unwrap();
    p.get_mut(slot).unwrap().is_allow_pickup = true;
    f.items.request(ItemRequest::Hold { slot });
    p.take_requests(&mut f, core::iter::empty);
    (f, p, slot)
}
#[test]
fn seven_makers_hold_drop_and_keep_ammo() {
    for (k, ammo) in [
        (Kind::Sword, 0),
        (Kind::Bat, 0),
        (Kind::Fan, 0),
        (Kind::StarRod, 20),
        (Kind::RayGun, 16),
        (Kind::FireFlower, 60),
        (Kind::Hammer, 0),
    ] {
        let (mut f, mut p, slot) = held(k);
        assert_eq!(
            p.get(slot).unwrap().status,
            ItemStatus::Equipment(equipment::Status::Hold)
        );
        assert_eq!(f.items.held_multi, ammo);
        crate::item_throw::drop_item(&mut f);
        p.take_requests(&mut f, core::iter::empty);
        let i = p.get(slot).unwrap();
        assert_eq!(i.status, ItemStatus::Equipment(equipment::Status::Dropped));
        assert_eq!(i.multi, ammo);
        assert!(!i.is_hold);
    }
}
#[test]
fn swing_types_use_original_statuses_and_rates() {
    for (k, statuses, speeds) in [
        (
            Kind::Sword,
            [
                Status::SwordSwing1,
                Status::SwordSwing3,
                Status::SwordSwing4,
                Status::SwordSwingDash,
            ],
            [1.0; 4],
        ),
        (
            Kind::Bat,
            [
                Status::BatSwing1,
                Status::BatSwing3,
                Status::BatSwing4,
                Status::BatSwingDash,
            ],
            [1.0, 1.0, 0.75, 1.0],
        ),
        (
            Kind::Fan,
            [
                Status::HarisenSwing1,
                Status::HarisenSwing3,
                Status::HarisenSwing4,
                Status::HarisenSwingDash,
            ],
            [2.0, 2.0, 2.0, 1.0],
        ),
        (
            Kind::StarRod,
            [
                Status::StarRodSwing1,
                Status::StarRodSwing3,
                Status::StarRodSwing4,
                Status::StarRodSwingDash,
            ],
            [1.0; 4],
        ),
    ] {
        let (mut f, _, _) = held(k);
        for n in 0..4 {
            assert!(item_use::check(&mut f, n, false));
            assert_eq!(f.status.status, statuses[n]);
            assert_eq!(f.status.timing.anim_speed, speeds[n]);
        }
    }
}
#[test]
fn fan_hit_scale_resets_after_two_updates() {
    let (mut f, mut p, slot) = held(Kind::Fan);
    item_use::check(&mut f, 0, false);
    item_use::proc_hit(&mut f);
    p.take_requests(&mut f, core::iter::empty);
    assert_eq!(p.get(slot).unwrap().scale.x, 1.5);
    item_use::update(&mut f);
    p.take_requests(&mut f, core::iter::empty);
    assert_eq!(p.get(slot).unwrap().scale.x, 1.5);
    item_use::update(&mut f);
    p.take_requests(&mut f, core::iter::empty);
    assert_eq!(p.get(slot).unwrap().scale.x, 1.0);
}
#[test]
fn gun_consumes_ammo_even_when_weapon_pool_is_full() {
    let (mut f, mut p, slot) = held(Kind::RayGun);
    item_use::check(&mut f, 0, false);
    let mut w = WeaponPool::default();
    for _ in 0..crate::weapon::MAX_WEAPONS {
        w.spawn_monster_shot(crate::monster_weapon::MonsterShot::saffron(
            false,
            Vec3::ZERO,
        ));
    }
    f.motion_script.flags[0] = 1;
    item_use::update(&mut f);
    item_use::accessory(&mut f);
    let spawn = f.take_weapon_spawn().unwrap();
    assert!(!w.spawn(spawn));
    p.take_requests(&mut f, core::iter::empty);
    assert_eq!(p.get(slot).unwrap().multi, 15);
    assert_eq!(f.motion_script.flags[0], 0);
}
#[test]
fn flower_first_costs_two_then_one_and_waits_for_five_shots() {
    let (mut f, mut p, slot) = held(Kind::FireFlower);
    item_use::check(&mut f, 0, false);
    f.motion_script.flags[0] = 1;
    // A released immediately: still at least five shots, eight frames apart.
    for tick in 0..33 {
        item_use::accessory(&mut f);
        p.take_requests(&mut f, core::iter::empty);
        f.take_weapon_spawn();
        if tick == 0 {
            assert_eq!(p.get(slot).unwrap().multi, 58);
            assert_eq!(f.status.timing.anim_speed, 0.0);
        }
        if tick == 31 {
            assert_eq!(f.item_use.fire_count, 4);
            assert_eq!(f.status.timing.anim_speed, 0.0);
        }
    }
    assert_eq!(f.item_use.fire_count, 5);
    assert_eq!(p.get(slot).unwrap().multi, 54);
    assert_eq!(f.status.timing.anim_speed, 1.0);
    assert_eq!(f.motion_script.flags[0], 0);
}
#[test]
fn shooting_switches_keep_clock_and_flower_pause() {
    for k in [Kind::RayGun, Kind::FireFlower] {
        let (mut f, _, _) = held(k);
        item_use::check(&mut f, 0, false);
        f.status.set_time(8.0);
        if k == Kind::FireFlower {
            f.status.timing.anim_speed = 0.0;
        }
        let speed = f.status.timing.anim_speed;
        f.item_use.fire_count = 3;
        assert!(item_use::on_ground_lost(&mut f));
        assert!(!f.is_grounded());
        assert_eq!(f.status.anim_frame, 8.0);
        assert_eq!(f.status.timing.anim_speed, speed);
        assert!(item_use::on_landing(&mut f));
        assert!(f.is_grounded());
        assert_eq!(f.item_use.fire_count, 3);
        assert_eq!(f.status.timing.anim_speed, speed);
    }
}
#[test]
fn hammer_timer_waits_for_pickup_then_warns_and_destroys() {
    let (mut f, mut p, slot) = held(Kind::Hammer);
    status::set_status(
        &mut f,
        Status::LightGet,
        0.0,
        status::StatusTiming::unknown(),
    );
    f.item_use.hammer_tics = 720;
    item_use::tick_hammer(&mut f);
    assert_eq!(f.item_use.hammer_tics, 720);
    crate::item_throw::light_get_proc_damage(&mut f);
    status::set_wait(&mut f);
    assert_eq!(f.status.status, Status::HammerWait);
    f.item_use.hammer_tics = 121;
    f.hitlag = 5;
    item_use::tick_hammer(&mut f);
    p.take_requests(&mut f, core::iter::empty);
    assert_eq!(
        p.get(slot).unwrap().colanim.id,
        crate::colanim::ColAnimId::ITEM_HAMMER_END
    );
    f.item_use.hammer_tics = 1;
    item_use::tick_hammer(&mut f);
    p.take_requests(&mut f, core::iter::empty);
    assert!(p.get(slot).is_none());
    assert!(f.items.held.is_none());
    assert_eq!(f.status.status, Status::Wait);
}
#[test]
fn hammer_landing_exits_on_first_update_and_turn_flips_halfway() {
    let (mut f, _, _) = held(Kind::Hammer);
    item_use::hammer_fall(&mut f);
    f.physics.vel_air.y = -30.0;
    item_use::on_landing(&mut f);
    assert_eq!(f.status.status, Status::HammerLanding);
    item_use::update(&mut f);
    assert_eq!(f.status.status, Status::HammerWait);
    let facing = f.facing;
    f.stick.x = -80;
    item_use::update(&mut f);
    assert_eq!(f.status.status, Status::HammerTurn);
    for _ in 0..4 {
        item_use::update(&mut f);
    }
    assert_eq!(f.facing, facing);
    item_use::update(&mut f);
    assert_eq!(f.facing, facing.flipped());
}
#[test]
fn smash_changes_star_rod_descriptor_for_later_tilts() {
    use crate::monster_weapon::ShotKind;
    let (mut f, _, _) = held(Kind::StarRod);
    let mut w = WeaponPool::default();
    for (smash, damage, speed) in [(false, 8, 80.0), (true, 12, 120.0), (false, 12, 80.0)] {
        let spawn = crate::weapon::WeaponSpawn {
            kind: crate::weapon::WeaponKind::Equipment {
                kind: ShotKind::StarRod,
                smash,
                angle_index: 0,
            },
            owner_port: 0,
            team: f.team,
            position: Vec3::ZERO,
            facing: 1.0,
            stale: crate::stale::WeaponStale::of(&f),
        };
        assert!(w.spawn(spawn));
        let last = w.monster_shots().last().unwrap();
        assert_eq!(last.damage, damage);
        assert_eq!(last.velocity.x, speed);
        f.motion.set(crate::stale::MotionAttackId::StarRodSwing3);
    }
}
/// RE-472, How to Play frame 3701: Mario's `FireFlowerShootAir` lands, and
/// `ftMainSetStatus` (inside the frame's passes, `is_events_forward`) runs
/// the old status's `proc_accessory` before the switch, so the flame clock
/// counts twice that frame (7 to 5 on the N64). The held pose stays still:
/// `gcSetAnimSpeed(0)` is the clip's rate, not only the status clock's.
#[test]
fn a_flower_switching_in_the_frames_passes_counts_its_flame_clock_twice() {
    let (mut f, _, _) = held(Kind::FireFlower);
    item_use::check(&mut f, 0, false);
    f.motion_script.flags[0] = 1;
    item_use::accessory(&mut f);
    f.take_weapon_spawn();
    assert_eq!(status::clip_speed(&f), 0.0);
    item_use::accessory(&mut f);
    let wait = f.item_use.flame_wait;
    f.motion_script.is_events_forward = true;
    assert!(item_use::on_ground_lost(&mut f));
    assert_eq!(f.item_use.flame_wait, wait - 1);
    assert_eq!(status::clip_speed(&f), 0.0);
    // Outside the passes (a setter's own call) the old accessory does not run.
    f.motion_script.is_events_forward = false;
    assert!(item_use::on_landing(&mut f));
    assert_eq!(f.item_use.flame_wait, wait - 1);
}
