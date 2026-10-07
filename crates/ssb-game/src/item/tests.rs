use super::*;
use crate::fighter::{Facing, FighterKind};
use crate::status::{self, AnyStatus, FoxStatus, Status, StatusTiming};
use crate::weapon::{WeaponKind, WeaponPool, WeaponSpawn};

fn fighter(kind: FighterKind, port: u8) -> Fighter {
    let mut f = Fighter::new(kind, port, 3);
    status::set_wait(&mut f);
    f
}

fn pull_bomb(pool: &mut ItemPool, link: &mut Fighter) -> u8 {
    crate::link::set_special_lw(link);
    for _ in 0..29 {
        status::update(link);
        pool.take_requests(link, core::iter::empty);
    }
    let held = link
        .items
        .held
        .expect("Bomb pull script must create an item");
    assert_eq!(held.kind, ItemKind::LinkBomb);
    held.slot
}

fn throw_bomb(pool: &mut ItemPool, link: &mut Fighter, vel: Vec3) {
    link.items.request(ItemRequest::Throw {
        vel,
        throw_mul: 1.0,
        is_smash: false,
    });
    pool.take_requests(link, core::iter::empty);
    assert!(link.items.held.is_none());
}

fn make_flame(pool: &mut ItemPool) -> u8 {
    let ness = fighter(FighterKind::Ness, 0);
    let mut weapons = WeaponPool::default();
    assert!(weapons.spawn(WeaponSpawn {
        kind: WeaponKind::NessPKFire { grounded: true },
        owner_port: ness.port,
        team: ness.team,
        position: Vec3::ZERO,
        facing: 1.0,
        stale: crate::stale::WeaponStale::of(&ness),
    }));
    let mut target = fighter(FighterKind::Mario, 1);
    weapons.apply_hits(&mut target);
    pool.take_weapon_spawns(&mut weapons, core::iter::empty);
    pool.tick(core::iter::empty, None, &[], &mut NoItemAnims);
    pool.order[0]
}

#[test]
fn bomb_pull_hold_throw_and_fast_hit_explode() {
    let mut link = fighter(FighterKind::Link, 0);
    let mut pool = ItemPool::default();
    let slot = pull_bomb(&mut pool, &mut link);
    assert_eq!(
        pool.get(slot).unwrap().status,
        ItemStatus::LinkBomb(link_bomb::Status::Hold)
    );
    assert!(pool.get(slot).unwrap().is_hold);
    crate::link::set_special_lw(&mut link);
    assert_eq!(link.status.status, AnyStatus::Common(Status::LightThrowF4));
    for _ in 0..60 {
        status::update(&mut link);
        pool.take_requests(&mut link, core::iter::empty);
        if link.items.held.is_none() {
            break;
        }
    }
    let bomb = pool.get(slot).unwrap();
    assert_eq!(bomb.status, ItemStatus::LinkBomb(link_bomb::Status::Thrown));
    assert!(bomb.vel_air.x.abs() > link_bomb::EXPLODE_THRESHOLD_VEL_X);
    let mut target = fighter(FighterKind::Mario, 1);
    target.pos = bomb.pos;
    pool.get_mut(slot).unwrap().update_attack_positions();
    pool.search_fighter(&mut target);
    crate::combat::resolve(&mut target);
    // `itMainGetDamageOutput`: (2 + 66 * 0.1) truncates to 8.
    assert_eq!(target.damage, 8);
    pool.resolve(&[&link, &target], &mut NoItemAnims, core::iter::empty);
    assert_eq!(
        pool.get(slot).unwrap().status,
        ItemStatus::LinkBomb(link_bomb::Status::Explode)
    );
}

#[test]
fn a_held_bomb_flashes_once_its_fuse_reaches_the_bloat() {
    let mut link = fighter(FighterKind::Link, 0);
    let mut pool = ItemPool::default();
    let slot = pull_bomb(&mut pool, &mut link);
    pool.get_mut(slot).unwrap().lifetime = link_bomb::BLOAT_BEGIN + 1;
    pool.tick(core::iter::empty, None, &[], &mut NoItemAnims);
    assert_eq!(
        pool.get(slot).unwrap().colanim.id,
        crate::colanim::ColAnimId::NONE
    );
    pool.tick(core::iter::empty, None, &[], &mut NoItemAnims);
    let bomb = pool.get(slot).unwrap();
    assert!(bomb.is_hold);
    // `itVisualsUpdateColAnim` runs for a held item too: the first frame
    // blends from yellow towards dark red over 4.
    assert_eq!(
        bomb.colanim.id,
        crate::colanim::ColAnimId::ITEM_LINK_BOMB_CRITICAL
    );
    assert_eq!(bomb.colanim.color(), Some([0xE0, 0xC0, 0x00, 0x8C]));
    assert!(bomb.attr.is_display_colanim);
}

#[test]
fn damage_drop_obeys_the_hitlag_stack_gate_and_death_destroys_the_held_item() {
    for stacking in [false, true] {
        let mut link = fighter(FighterKind::Link, 0);
        let mut pool = ItemPool::default();
        let slot = pull_bomb(&mut pool, &mut link);
        link.hits.damage_queue = 60;
        link.hits.damage_knockback = 40.0;
        if stacking {
            link.hitlag = 5;
            link.is_knockback_paused = true;
            link.damage_knockback_stack = 20.0;
        }
        crate::combat::proc_params(&mut link);
        assert_eq!(link.items.held.is_some(), stacking);
        pool.take_requests(&mut link, core::iter::empty);
        if !stacking {
            let item = pool.get(slot).unwrap();
            assert!(!item.is_hold);
            assert_eq!(item.vel_air, Vec3::ZERO);
            assert_eq!(item.attack.throw_mul, 1.0);
            assert_eq!(
                item.status,
                ItemStatus::LinkBomb(link_bomb::Status::Dropped)
            );
        } else {
            crate::dead::set_dead_down(&mut link);
            pool.take_requests(&mut link, core::iter::empty);
            assert!(link.items.held.is_none());
            assert!(pool.get(slot).is_none());
        }
    }
}

#[test]
fn aerial_throw_landing_uses_the_source_descent_threshold() {
    for (speed, expected) in [(-19.0, Status::Wait), (-20.0, Status::LandingLight)] {
        let mut link = fighter(FighterKind::Link, 0);
        crate::item_throw::set_item_throw(&mut link, Status::LightThrowAirLw4);
        link.physics.vel_air.y = speed;
        status::set_landing_or_landing_air(&mut link);
        assert_eq!(link.status.status, expected);
    }
}

#[test]
fn polygon_pickup_uses_its_own_authored_range() {
    let mut pool = ItemPool::default();
    let mut bomb = link_bomb::make(Vec3::new(550.0, 0.0, 0.0), 1);
    bomb.is_allow_pickup = true;
    pool.alloc(bomb).unwrap();
    let mut fox = fighter(FighterKind::Fox, 0);
    let mut polygon = fighter(FighterKind::PolyFox, 1);
    pool.publish(&mut fox);
    pool.publish(&mut polygon);
    assert!(crate::item_throw::find_item(&fox).is_some());
    assert!(crate::item_throw::find_item(&polygon).is_none());
}

#[test]
fn fuse_runs_in_the_hand_and_releases_before_explosion() {
    let mut link = fighter(FighterKind::Link, 0);
    let mut pool = ItemPool::default();
    let slot = pull_bomb(&mut pool, &mut link);
    pool.observe_owner(&link);
    for _ in 0..link_bomb::LIFETIME {
        pool.tick(core::iter::empty, None, &[], &mut NoItemAnims);
    }
    assert_eq!(pool.get(slot).unwrap().lifetime, 0);
    assert!(link.items.held.is_some());
    pool.tick(core::iter::empty, None, &[], &mut NoItemAnims);
    pool.sync_owner(&mut link);
    assert!(link.items.held.is_none());
    let bomb = pool.get(slot).unwrap();
    assert_eq!(
        bomb.status,
        ItemStatus::LinkBomb(link_bomb::Status::Explode)
    );
    assert!(!bomb.is_hold);
    assert_eq!(bomb.owner, None);
    assert_eq!(bomb.attack.damage, 5);
}

#[test]
fn bomb_explosion_can_hit_link_himself() {
    let mut link = fighter(FighterKind::Link, 0);
    let mut pool = ItemPool::default();
    let slot = pull_bomb(&mut pool, &mut link);
    pool.get_mut(slot).unwrap().lifetime = 0;
    pool.observe_owner(&link);
    pool.tick(core::iter::empty, None, &[], &mut NoItemAnims);
    pool.sync_owner(&mut link);
    pool.search_fighter(&mut link);
    crate::combat::resolve(&mut link);
    assert_eq!(link.damage, 5);
}

#[test]
fn seven_damage_explodes_while_six_damage_recoils() {
    for damage in [6, 7] {
        let mut link = fighter(FighterKind::Link, 0);
        let mut pool = ItemPool::default();
        let slot = pull_bomb(&mut pool, &mut link);
        throw_bomb(&mut pool, &mut link, Vec3::ZERO);
        queue_damage(
            pool.get_mut(slot).unwrap(),
            damage,
            361,
            Element::Normal,
            -1.0,
            Attacker {
                owner: Some(1),
                team: 1,
                player: Some(1),
                handicap: 9,
            },
            Knock {
                weight: 0,
                scale: 100,
                base: 0,
            },
        );
        pool.resolve(&[&link], &mut NoItemAnims, core::iter::empty);
        let bomb = pool.get(slot).unwrap();
        if damage == 7 {
            assert_eq!(
                bomb.status,
                ItemStatus::LinkBomb(link_bomb::Status::Explode)
            );
        } else {
            assert_eq!(bomb.status, ItemStatus::LinkBomb(link_bomb::Status::Thrown));
            assert_eq!(bomb.vel_air, Vec3::new(20.0, 18.0, 0.0));
        }
    }
}

#[test]
fn slow_bomb_hit_recoils_at_the_exact_speed_threshold() {
    let mut link = fighter(FighterKind::Link, 0);
    let mut pool = ItemPool::default();
    let slot = pull_bomb(&mut pool, &mut link);
    throw_bomb(&mut pool, &mut link, Vec3::ZERO);
    let bomb = pool.get_mut(slot).unwrap();
    bomb.vel_air = Vec3::new(
        link_bomb::EXPLODE_THRESHOLD_VEL_X,
        link_bomb::EXPLODE_THRESHOLD_VEL_Y,
        0.0,
    );
    bomb.hit_normal_damage = 2;
    bomb.hit_lr = 1.0;
    pool.resolve(&[&link], &mut NoItemAnims, core::iter::empty);
    let bomb = pool.get(slot).unwrap();
    assert_eq!(bomb.status, ItemStatus::LinkBomb(link_bomb::Status::Fall));
    assert_eq!(bomb.vel_air, Vec3::new(-8.0, 20.0, 0.0));
}

#[test]
fn pk_fire_loses_three_times_jab_damage_and_one_update_tick() {
    let mut pool = ItemPool::default();
    let slot = make_flame(&mut pool);
    let mut mario = fighter(FighterKind::Mario, 1);
    status::set_attack11(&mut mario);
    for _ in 0..10 {
        if mario
            .attack_colls
            .iter()
            .any(|c| c.state != AttackState::Off)
        {
            break;
        }
        status::update(&mut mario);
    }
    let coll = *mario
        .attack_colls
        .iter()
        .find(|c| c.state != AttackState::Off)
        .unwrap();
    let flame = pool.get_mut(slot).unwrap();
    flame.pos = coll.pos_curr - flame.damage_coll.offset;
    let before = flame.lifetime;
    pool.search_hurt(&mut [&mut mario], &mut WeaponPool::default());
    assert_eq!(pool.get(slot).unwrap().damage_highest, coll.damage);
    pool.resolve(&[&mario], &mut NoItemAnims, core::iter::empty);
    assert_eq!(
        pool.get(slot).unwrap().lifetime,
        before - 3 * coll.damage - 1
    );
}

#[test]
fn pickup_script_holds_the_published_bomb() {
    let mut pool = ItemPool::default();
    let mut bomb = link_bomb::make(Vec3::new(105.0, 0.0, 0.0), 1);
    bomb.status = ItemStatus::LinkBomb(link_bomb::Status::Wait);
    bomb.is_allow_pickup = true;
    let slot = pool.alloc(bomb).unwrap();
    let mut mario = fighter(FighterKind::Mario, 1);
    pool.publish(&mut mario);
    assert!(crate::item_throw::check_get(&mut mario));
    assert_eq!(mario.status.status, AnyStatus::Common(Status::LightGet));
    for _ in 0..60 {
        status::update(&mut mario);
        pool.take_requests(&mut mario, core::iter::empty);
        if mario.items.held.is_some() {
            break;
        }
    }
    assert_eq!(mario.items.held.unwrap().slot, slot);
    assert_eq!(pool.get(slot).unwrap().owner, Some(mario.port));
    assert!(pool.get(slot).unwrap().is_hold);
}

#[test]
fn fox_reflects_bomb_and_damage_growth_caps_at_one_hundred() {
    let mut link = fighter(FighterKind::Link, 0);
    let mut pool = ItemPool::default();
    let slot = pull_bomb(&mut pool, &mut link);
    throw_bomb(&mut pool, &mut link, Vec3::new(100.0, 0.0, 0.0));
    let mut fox = fighter(FighterKind::Fox, 1);
    fox.facing = Facing::Left;
    status::set_any_status(
        &mut fox,
        AnyStatus::Fox(FoxStatus::SpecialLwLoop),
        0.0,
        StatusTiming::unknown(),
    );
    let r = crate::combat::FOX_REFLECTOR;
    let bomb = pool.get_mut(slot).unwrap();
    bomb.pos = fox.joint_world(r.joint, r.offset);
    bomb.update_attack_positions();
    pool.search_fighter(&mut fox);
    assert_eq!(pool.get(slot).unwrap().reflect_by, Some(fox.port));
    pool.resolve(&[&link, &fox], &mut NoItemAnims, core::iter::empty);
    let bomb = pool.get_mut(slot).unwrap();
    assert_eq!(bomb.owner, Some(fox.port));
    assert_eq!(bomb.attack.damage, 4);
    assert!((bomb.vel_air.x + 60.0).abs() < 0.001);
    bomb.attack.damage = 99;
    bomb.reflect_by = Some(fox.port);
    pool.resolve(&[&fox], &mut NoItemAnims, core::iter::empty);
    assert_eq!(pool.get(slot).unwrap().attack.damage, 100);
}

#[test]
fn shield_hops_below_135_degrees_and_rebounds_at_the_boundary() {
    for degrees in [120.0_f32, 135.0] {
        let mut link = fighter(FighterKind::Link, 0);
        let mut pool = ItemPool::default();
        let slot = pull_bomb(&mut pool, &mut link);
        throw_bomb(&mut pool, &mut link, Vec3::new(100.0, 0.0, 0.0));
        let bomb = pool.get_mut(slot).unwrap();
        bomb.hit_shield_damage = 2;
        bomb.shield_collide_angle = degrees.to_radians();
        bomb.shield_collide_dir = Vec3::new(0.0, 0.0, 1.0);
        pool.resolve(&[&link], &mut NoItemAnims, core::iter::empty);
        let bomb = pool.get(slot).unwrap();
        if degrees < 135.0 {
            assert!((bomb.vel_air.x - 30.0).abs() < 0.001);
            assert!((bomb.vel_air.y - 51.961525).abs() < 0.001);
        } else {
            assert!((bomb.vel_air.x + 3.6).abs() < 0.001);
            assert_eq!(bomb.vel_air.y, 25.0);
        }
        assert_eq!(bomb.status, ItemStatus::LinkBomb(link_bomb::Status::Thrown));
    }
}

#[test]
fn backward_throw_script_turns_link_before_releasing_the_bomb() {
    let mut link = fighter(FighterKind::Link, 0);
    let mut pool = ItemPool::default();
    let slot = pull_bomb(&mut pool, &mut link);
    crate::item_throw::set_item_throw(&mut link, Status::LightThrowB4);
    for _ in 0..60 {
        status::update(&mut link);
        pool.take_requests(&mut link, core::iter::empty);
        if link.items.held.is_none() {
            break;
        }
    }
    assert_eq!(link.facing, Facing::Left);
    assert!(link.items.held.is_none());
    assert!(pool.get(slot).unwrap().vel_air.x < 0.0);
}

#[test]
fn light_throw_joints_turn_each_step_and_restore_facing_outside_the_status() {
    for facing in [Facing::Right, Facing::Left] {
        let mut link = fighter(FighterKind::Link, 0);
        link.facing = facing;
        crate::item_throw::set_item_throw(&mut link, Status::LightThrowB);
        link.item_throw = crate::item_throw::ThrowState::default();
        link.motion_script.flags = [0, 0, 0, 8];
        for step in 1..=8 {
            crate::item_throw::update(&mut link);
            let yaw = core::f32::consts::FRAC_PI_2 * facing.sign()
                - core::f32::consts::PI * step as f32 / 8.0;
            let axes = crate::item_throw::model_axes(&link);
            let (sin, cos) = ssb_engine::math::sin_cos(yaw);
            assert!((axes[2].x - sin).abs() < 0.00001);
            assert!((axes[2].z - cos).abs() < 0.00001);
            let point = link.joint_world(0, Vec3::new(0.0, 0.0, 100.0)) - link.pos;
            assert!((point.x - 100.0 * sin).abs() < 0.001);
            assert!((point.z - 100.0 * cos).abs() < 0.001);
            assert_eq!(
                link.facing,
                if step < 4 { facing } else { facing.flipped() }
            );
        }
        status::set_wait(&mut link);
        assert_eq!(
            crate::item_throw::model_axes(&link)[2],
            Vec3::new(-facing.sign(), 0.0, 0.0)
        );
    }
}

#[test]
fn main_pass_runs_a_chansey_egg_once_without_skipping_its_surviving_parent() {
    let mut pool = ItemPool::default();
    let slot = pool
        .spawn_mmonster(
            mmonster::Kind::MLucky,
            Vec3::new(0.0, 1000.0, 0.0),
            None,
            0,
            &core::iter::empty,
        )
        .unwrap();
    let chansey = pool.get_mut(slot).unwrap();
    chansey.status = ItemStatus::MMonster(mmonster::Status::MLuckyMakeEgg);
    chansey.multi = 3;
    chansey.vars.mmonster.egg_spawn_wait = 0;
    pool.tick(core::iter::empty, None, &[], &mut NoItemAnims);
    let chansey = pool.get(slot).unwrap();
    assert_eq!(chansey.multi, 2);
    assert_eq!(chansey.anim_ticks, 2);
    let egg = pool.items().last().unwrap();
    assert_eq!(egg.kind, ItemKind::Container(container::Kind::Egg));
    assert_eq!(egg.anim_ticks, 2);
    assert!(egg.pos.y > chansey.pos.y);
}

#[test]
fn held_and_loose_bomb_bloat_write_the_same_promoted_body_root() {
    for held in [false, true] {
        let mut pool = ItemPool::default();
        let mut bomb = link_bomb::make(Vec3::ZERO, 0);
        bomb.is_hold = held;
        if held {
            bomb.scale = Vec3::new(1.8, 1.8, 1.0);
            link_bomb::hold_set_status(&mut bomb);
            assert_eq!(bomb.scale, Vec3::new(1.0, 1.0, 1.0));
        }
        bomb.lifetime = link_bomb::BLOAT_BEGIN - 1;
        bomb.vars.bomb_scale_int = 0;
        bomb.vars.bomb_scale_id = 3;
        let slot = pool.alloc(bomb).unwrap();
        pool.tick(core::iter::empty, None, &[], &mut NoItemAnims);
        let bomb = pool.get(slot).unwrap();
        assert_eq!(bomb.scale.x, link_bomb::BLOAT_SCALES[3]);
        assert_eq!(bomb.scale.y, link_bomb::BLOAT_SCALES[3]);
        assert_eq!(bomb.scale.z, 1.0);
    }
}

#[test]
fn main_pass_runs_pokemon_made_by_a_destroyed_ball_after_existing_siblings() {
    for siblings in [0, 1, 14] {
        let mut pool = ItemPool::default();
        let mut ball = mball::make(Vec3::new(0.0, 1000.0, 0.0), Vec3::ZERO);
        ball.status = ItemStatus::MBall(mball::Status::Open);
        ball.multi = 0;
        let ball_slot = pool.alloc(ball).unwrap();
        for i in 0..siblings {
            pool.alloc(link_bomb::make(Vec3::new(i as f32, 1000.0, 0.0), i as u16))
                .unwrap();
        }
        pool.tick(core::iter::empty, None, &[], &mut NoItemAnims);
        assert!(pool.get(ball_slot).is_none());
        let pokemon = pool.items().last().unwrap();
        assert!(matches!(pokemon.kind, ItemKind::MMonster(_)));
        assert_eq!(
            pokemon.anim_ticks, 2,
            "maker play followed by same-pass main play"
        );
        for sibling in pool.items().take(siblings) {
            assert_eq!(sibling.anim_ticks, 2);
            assert_eq!(sibling.lifetime, link_bomb::LIFETIME - 1);
        }
    }
}

#[test]
fn throw_flag_overlay_decodes_damage_velocity_and_signed_angle() {
    let mut link = fighter(FighterKind::Link, 0);
    let mut pool = ItemPool::default();
    let slot = pull_bomb(&mut pool, &mut link);
    crate::item_throw::set_item_throw(&mut link, Status::LightThrowF);
    link.motion_script.flags[0] = 1;
    link.motion_script.flags[1] = 200;
    link.motion_script.flags[2] = (150 << 12) | ((-90_i32 as u32) & 0xfff);
    crate::item_throw::update(&mut link);
    assert_eq!(link.item_throw.angle, -90);
    assert_eq!(link.item_throw.damage, 2.0);
    assert_eq!(link.item_throw.velocity, 1.5);
    assert_eq!(link.motion_script.flags, [0; 4]);
    assert!(link.items.held.is_none());
    pool.take_requests(&mut link, core::iter::empty);
    let bomb = pool.get(slot).unwrap();
    assert!((bomb.vel_air.x).abs() < 0.001);
    assert!((bomb.vel_air.y + 54.0).abs() < 0.001);
    assert_eq!(bomb.attack.throw_mul, 2.0);
}

#[test]
fn aerial_bomb_throw_preserves_air_situation_and_finishes_in_fall() {
    let mut link = fighter(FighterKind::Link, 0);
    let mut pool = ItemPool::default();
    let slot = pull_bomb(&mut pool, &mut link);
    status::set_fall(&mut link);
    crate::link::set_special_air_lw(&mut link);
    assert_eq!(
        link.status.status,
        AnyStatus::Common(Status::LightThrowAirF4)
    );
    assert!(!Status::LightThrowAirF4.is_grounded());
    assert!(!link.is_grounded());
    for _ in 0..60 {
        status::update(&mut link);
        pool.take_requests(&mut link, core::iter::empty);
        if link.status.status != AnyStatus::Common(Status::LightThrowAirF4) {
            break;
        }
    }
    assert!(link.items.held.is_none());
    assert_eq!(
        pool.get(slot).unwrap().status,
        ItemStatus::LinkBomb(link_bomb::Status::Thrown)
    );
    assert!(matches!(
        link.status.status,
        AnyStatus::Common(Status::Fall | Status::FallAerial)
    ));
}

#[test]
fn guard_slide_and_escape_buffers_select_the_source_throw_statuses() {
    use ssb_engine::input::{ControllerState, N64Buttons};
    let mut link = fighter(FighterKind::Link, 0);
    let mut pool = ItemPool::default();
    pull_bomb(&mut pool, &mut link);
    status::set_dash(&mut link);
    link.status.set_time(7.0);
    status::set_guard_on(&mut link);
    assert_eq!(link.guard.slide_tics, 13);
    assert!(!crate::item_throw::check_guard(&mut link));
    assert_eq!(link.guard.slide_tics, 12);
    link.set_input(
        ControllerState {
            buttons: N64Buttons(N64Buttons::A),
            ..Default::default()
        },
        false,
        false,
    );
    assert!(crate::item_throw::check_guard(&mut link));
    assert_eq!(
        link.status.status,
        AnyStatus::Common(Status::LightThrowDash)
    );
    status::set_run(&mut link);
    status::set_guard_on(&mut link);
    assert_eq!(link.guard.slide_tics, 4);

    link.set_input(
        ControllerState {
            stick_x: 80,
            ..Default::default()
        },
        false,
        false,
    );
    assert!(crate::reaction::check_escape_guard(&mut link));
    assert_eq!(link.reaction.itemthrow_buffer_tics, 5);
    assert!(!crate::item_throw::check_escape(&mut link));
    assert_eq!(link.reaction.itemthrow_buffer_tics, 4);
    link.set_input(
        ControllerState {
            buttons: N64Buttons(N64Buttons::A),
            ..Default::default()
        },
        false,
        false,
    );
    assert!(crate::item_throw::check_escape(&mut link));
    assert_eq!(link.status.status, AnyStatus::Common(Status::LightThrowF4));
    crate::reaction::set_escape(&mut link, Status::EscapeF);
    assert_eq!(link.reaction.itemthrow_buffer_tics, 0);
    assert!(!crate::item_throw::check_escape(&mut link));
}

#[test]
fn sixteen_slot_pool_reuses_last_freed_slot_and_appends_creation_order() {
    let mut pool = ItemPool::default();
    let mut slots = [0; ITEM_ALLOC_MAX];
    for (i, slot) in slots.iter_mut().enumerate() {
        *slot = pool
            .alloc(link_bomb::make(Vec3::new(i as f32, 0.0, 0.0), i as u16))
            .unwrap();
    }
    assert_eq!(pool.active_count(), 16);
    assert!(pool.alloc(link_bomb::make(Vec3::ZERO, 99)).is_none());
    pool.destroy(slots[3]);
    pool.destroy(slots[11]);
    assert_eq!(
        pool.alloc(link_bomb::make(Vec3::new(100.0, 0.0, 0.0), 100)),
        Some(slots[11])
    );
    assert_eq!(
        pool.alloc(link_bomb::make(Vec3::new(101.0, 0.0, 0.0), 101)),
        Some(slots[3])
    );
    let positions: std::vec::Vec<_> = pool.items().map(|i| i.pos.x).collect();
    let expected: std::vec::Vec<_> = (0..16)
        .filter(|&i| i != 3 && i != 11)
        .map(|i| i as f32)
        .chain([100.0, 101.0])
        .collect();
    assert_eq!(positions, expected);
}

#[test]
fn bounds_destroy_items_strictly_outside_each_edge() {
    let bounds = BlastZone {
        top: 100.0,
        bottom: -100.0,
        left: -100.0,
        right: 100.0,
    };
    let mut pool = ItemPool::default();
    for pos in [
        Vec3::new(-101.0, 0.0, 0.0),
        Vec3::new(101.0, 0.0, 0.0),
        Vec3::new(0.0, -101.0, 0.0),
        Vec3::new(0.0, 101.0, 0.0),
        Vec3::new(100.0, 100.0, 0.0),
    ] {
        pool.alloc(link_bomb::make(pos, 1)).unwrap();
    }
    pool.tick(core::iter::empty, Some(bounds), &[], &mut NoItemAnims);
    assert_eq!(pool.active_count(), 1);
    assert_eq!(
        pool.items().next().unwrap().pos,
        Vec3::new(100.0, 100.0, 0.0)
    );
}

#[test]
fn ground_item_carries_during_hitlag_before_the_bounds_gate() {
    let mut pool = ItemPool::default();
    let slot = make_flame(&mut pool);
    let item = pool.slots[slot as usize].as_mut().unwrap();
    item.ga = Ga::Ground;
    item.floor = Some(crate::ground::Standing {
        line: 0,
        flags: 0,
        normal: Vec2::new(0.0, 1.0),
    });
    item.pos = Vec3::ZERO;
    item.vel_air = Vec3::ZERO;
    item.hitlag_tics = 4;
    let surface = crate::map::floor_surface((
        0,
        crate::collision::Segment {
            x1: -100,
            y1: 0,
            x2: 100,
            y2: 0,
            flags: 0,
        },
    ));
    let surface = MapSurface {
        motion: Some(crate::weapon::SurfaceMotion {
            offset: Vec2::new(12.5, 20.25),
            speed: Vec3::new(12.5, 20.25, 0.0),
        }),
        ..surface
    };
    pool.tick(|| [surface], None, &[], &mut NoItemAnims);
    let item = pool.slots[slot as usize].as_ref().unwrap();
    assert_eq!(item.pos, Vec3::new(12.5, 20.25, 0.0));
    pool.tick(
        || [surface],
        Some(BlastZone {
            left: -100.0,
            right: 24.0,
            bottom: -100.0,
            top: 100.0,
        }),
        &[],
        &mut NoItemAnims,
    );
    assert!(pool.slots[slot as usize].is_none());
}

/// `itManagerMakeItem` plays the added animation once; each
/// `itProcessProcItemMain` outside hitlag plays it again.
#[test]
fn item_animation_plays_once_at_creation_and_per_update_outside_hitlag() {
    let mut pool = ItemPool::default();
    let slot = make_flame(&mut pool);
    // `make_flame` already ran one update after the creation play.
    assert_eq!(pool.get(slot).unwrap().anim_ticks, 2);
    pool.get_mut(slot).unwrap().hitlag_tics = 2;
    pool.tick(core::iter::empty, None, &[], &mut NoItemAnims);
    assert_eq!(pool.get(slot).unwrap().anim_ticks, 2);
    pool.tick(core::iter::empty, None, &[], &mut NoItemAnims);
    // The frame the hitlag runs out plays again.
    assert_eq!(pool.get(slot).unwrap().anim_ticks, 3);
}

/// `ftMainSearchHitItem`: a PK Fire pillar takes Ness's team
/// (`itNessPKFireMakeItem`) and, with team attack off, spares his
/// teammates.
#[test]
fn a_teammates_item_passes_through_only_with_team_attack_off() {
    use crate::team::TeamRules;
    let team_attack = TeamRules {
        is_team_attack: true,
        ..TeamRules::TEAMS
    };
    for (rules, victim_team, lands) in [
        (TeamRules::TEAMS, 0, false),
        (TeamRules::TEAMS, 1, true),
        (team_attack, 0, true),
        (TeamRules::FREE_FOR_ALL, 0, true),
    ] {
        let mut pool = ItemPool {
            team_rules: rules,
            ..ItemPool::default()
        };
        let slot = make_flame(&mut pool);
        let pillar = *pool.get(slot).unwrap();
        assert_eq!((pillar.owner, pillar.team), (Some(0), 0));
        let mut victim = fighter(FighterKind::Mario, 2);
        victim.team = victim_team;
        victim.pos = pillar.pos + Vec3::new(0.0, 100.0, 0.0);
        pool.search_fighter(&mut victim);
        crate::combat::resolve(&mut victim);
        assert_eq!(victim.damage > 0, lands, "{rules:?} team {victim_team}");
    }
}

/// `itProcessSearchHitFighter`: with team attack off a fighter's attacks
/// pass through its team's items.
#[test]
fn a_teammates_attack_passes_through_an_item_only_with_team_attack_off() {
    use crate::team::TeamRules;
    for (rules, attacker_team, lands) in [
        (TeamRules::TEAMS, 0, false),
        (TeamRules::TEAMS, 1, true),
        (TeamRules::FREE_FOR_ALL, 0, true),
    ] {
        let mut pool = ItemPool {
            team_rules: rules,
            ..ItemPool::default()
        };
        let slot = make_flame(&mut pool);
        let pillar = *pool.get(slot).unwrap();
        let mut attacker = fighter(FighterKind::Mario, 2);
        attacker.team = attacker_team;
        attacker.attack_colls[0] = crate::combat::AttackColl {
            state: crate::combat::AttackState::Transfer,
            damage: 10,
            size: 150.0,
            is_hit_air: true,
            is_hit_ground: true,
            pos_curr: pillar.damage_coll_pos(),
            pos_prev: pillar.damage_coll_pos(),
            ..Default::default()
        };
        pool.search_hurt(&mut [&mut attacker], &mut WeaponPool::default());
        let hit = pool.get(slot).map_or(0, |p| p.damage_queue);
        assert_eq!(hit > 0, lands, "{rules:?} team {attacker_team}");
    }
}

/// `ftMainUpdateShieldStatItem` ends with `efManagerSetOffMakeEffect`
/// halfway between the attack and the shield joint, sized by the shield
/// damage plus the damage (RE-473: the N64 draws its two at frame 3992 of
/// How to Play, when the thrown Fire Flower meets Luigi's shield).
#[test]
fn an_item_on_a_shield_makes_its_set_off() {
    use crate::effect::HitEffectKind;
    let mut link = fighter(FighterKind::Link, 0);
    let mut pool = ItemPool::default();
    let slot = pull_bomb(&mut pool, &mut link);
    throw_bomb(&mut pool, &mut link, Vec3::new(100.0, 0.0, 0.0));
    let mut mario = fighter(FighterKind::Mario, 1);
    mario.pos = Vec3::new(600.0, 0.0, 0.0);
    mario.guard.is_shield = true;
    let bomb = pool.get_mut(slot).unwrap();
    bomb.pos = mario.pos + Vec3::new(-40.0, 30.0, 0.0);
    bomb.update_attack_positions();
    pool.search_fighter(&mut mario);
    let bomb = *pool.get(slot).unwrap();
    assert!(bomb.hit_shield_damage > 0, "the shield took the hit");
    let set_offs: Vec<_> = mario.hits.effects[..mario.hits.effects_len]
        .iter()
        .flatten()
        .filter(|e| e.kind == HitEffectKind::SetOff)
        .collect();
    assert_eq!(set_offs.len(), 1);
    let p = bomb.attack.pos[0];
    let at = crate::combat::attack_point(p.pos_curr, p.pos_prev, bomb.attack.state);
    assert_eq!(set_offs[0].pos, crate::combat::impact_point(at, mario.pos));
    assert_eq!(
        set_offs[0].damage,
        bomb.attack.shield_damage + bomb.damage_output()
    );
}

/// `itProcessUpdateDamageStatFighter`: a fighter's attack on an item's
/// damage box makes the element's spark halfway to the box
/// (`gmCollisionGetFighterAttackItemDamagePosition`), in the item's search
/// (RE-473).
#[test]
fn a_fighters_attack_on_an_item_makes_its_spark() {
    use crate::effect::{HitEffect, HitEffectKind, HitEffectSink};
    use crate::wpeffect::WeaponEffect;
    #[derive(Default)]
    struct Record(Vec<WeaponEffect>);
    impl HitEffectSink for Record {
        fn make(&mut self, _: &HitEffect) {}
        fn weapon(&mut self, e: &WeaponEffect) {
            self.0.push(*e);
        }
    }
    let mut pool = ItemPool::default();
    let slot = make_flame(&mut pool);
    let pillar = *pool.get(slot).unwrap();
    let mut attacker = fighter(FighterKind::Mario, 2);
    let at = pillar.damage_coll_pos() + Vec3::new(20.0, 0.0, 0.0);
    attacker.attack_colls[0] = crate::combat::AttackColl {
        state: crate::combat::AttackState::Transfer,
        damage: 10,
        size: 150.0,
        is_hit_air: true,
        is_hit_ground: true,
        pos_curr: at,
        pos_prev: at,
        ..Default::default()
    };
    pool.search_hurt(&mut [&mut attacker], &mut WeaponPool::default());
    assert!(pool.get(slot).is_some_and(|p| p.damage_queue > 0));
    let mut sink = Record::default();
    pool.flush_effects(&mut sink);
    assert_eq!(
        sink.0,
        [WeaponEffect::Hit(HitEffect {
            kind: HitEffectKind::NormalLight,
            pos: crate::combat::impact_point(at, pillar.damage_coll_pos()),
            player: attacker.port,
            damage: 10,
        })]
    );
}
