use super::*;
use crate::colanim::ColAnimId;
use crate::effect::{life, DisplayKind, Effects, EFFECT_ALLOC_NUM};
use crate::fighter::{JointTransform, Situation};
use crate::particle::tests::TestBank;
use crate::particle::{Particles, NIL};
use crate::rng;
use crate::status::{self, Status, StatusTiming};

/// A common bank whose every script waits and lives `lifetime` frames.
fn bank(lifetime: u16) -> TestBank {
    static WAIT: &[u8] = &[0x1F, 0xFF];
    let s = TestBank::particle(lifetime, 10.0, WAIT);
    TestBank::new(&[s; 0x80])
}

fn mario() -> Fighter {
    let mut f = Fighter::new(FighterKind::Mario, 0, 3);
    f.situation = Situation::Ground;
    f
}

/// The seed after `n` draws from `seed`.
fn after_draws(seed: i32, n: usize) -> i32 {
    rng::set_seed(seed);
    for _ in 0..n {
        rng::rand_float();
    }
    rng::seed()
}

fn params(f: &Fighter) -> std::vec::Vec<EffectRequest> {
    f.effects
        .iter()
        .filter_map(|e| match e {
            FighterEffect::Param(r) => Some(*r),
            _ => None,
        })
        .collect()
}

/// `dMarioMainMotion_Dash` opens with
/// `Effect(0, DustLight, 0, (0, 0, -120), (0, 60, 0))`. Set during the
/// frame's passes, its status script skips it and the effect copy makes it
/// at the end of the physics pass (`ftMainUpdateMotionEventsForwardEffect`).
#[test]
fn a_status_set_in_the_frame_makes_its_effects_after_the_physics_pass() {
    let mut f = mario();
    f.motion_script.is_events_forward = true;
    status::set_status(&mut f, Status::Dash, 0.0, StatusTiming::unknown());
    assert!(f.effects.is_empty());
    crate::motion::end_physics(&mut f);
    let r = params(&f);
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].kind, kind::DUST_LIGHT);
    assert_eq!(r[0].joint, 0);
    assert_eq!(r[0].offset, Some(Vec3::new(0.0, 0.0, -120.0)));
    assert_eq!(r[0].scatter, Some(Vec3::new(0.0, 60.0, 0.0)));
    assert_eq!(r[0].lr, f.facing.sign() as i8);
    assert!(!f.motion_script.is_events_forward);
}

/// Set outside the frame's passes (a hit's damage status), the script makes
/// the effect at once and its copy starts past it: nothing is made twice.
#[test]
fn a_status_set_outside_the_frame_makes_them_at_once_and_once() {
    let mut f = mario();
    status::set_status(&mut f, Status::Dash, 0.0, StatusTiming::unknown());
    assert_eq!(params(&f).len(), 1);
    crate::motion::end_physics(&mut f);
    assert_eq!(params(&f).len(), 1);
}

/// The dash's loop: a cloud on its first frame and four frames later, and
/// no more.
#[test]
fn the_dash_makes_two_clouds_four_frames_apart() {
    let mut f = mario();
    f.motion_script.is_events_forward = true;
    status::set_status(&mut f, Status::Dash, 0.0, StatusTiming::unknown());
    crate::motion::end_physics(&mut f);
    let mut made = std::vec::Vec::new();
    for frame in 0..12 {
        made.push(f.effects.len());
        f.effects.clear();
        f.motion_script.is_events_forward = true;
        f.status.anim_frame += 1.0;
        crate::motion::advance(&mut f);
        crate::motion::end_physics(&mut f);
        let _ = frame;
    }
    assert_eq!(made, [1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0]);
}

/// `ftParamMakeEffect` draws the scatter in x, y, z order (a zero axis
/// draws nothing), then the maker draws: `efManagerDustLightMakeEffect`'s
/// spin and angle.
#[test]
fn the_scatter_draws_come_before_the_makers_in_axis_order() {
    let bank = bank(20);
    let mut p = std::boxed::Box::new(Particles::new());
    let mut e = Effects::new(0);
    let mut f = mario();
    f.pos = Vec3::new(1000.0, 0.0, 0.0);
    rng::set_seed(7);
    let rx = rng::rand_float();
    let rz = rng::rand_float();
    let spin = rng::rand_float();
    let r = EffectRequest {
        kind: kind::DUST_LIGHT,
        joint: 0,
        offset: Some(Vec3::new(0.0, 0.0, 0.0)),
        scatter: Some(Vec3::new(10.0, 0.0, 30.0)),
        lr: 1,
        is_scale_pos: false,
        flag: 0,
    };
    rng::set_seed(7);
    let mut rt = EffectRuntime {
        particles: &mut p,
        effects: &mut e,
        banks: &bank,
    };
    make(&mut f, &mut rt, r);
    assert_eq!(rng::seed(), after_draws(7, 4));
    let (_, pc) = rt.particles.list(1).next().expect("the dust");
    let t = *rt.particles.transform(pc.xf);
    // TopN's frame: offset z is world x, offset x is world -z.
    let local = Vec3::new((rx - 0.5) * 20.0, 0.0, (rz - 0.5) * 60.0);
    let want = f.joint_world(0, local);
    assert!((t.translate.x - want.x).abs() < 1e-3, "{t:?} {want:?}");
    assert!((t.translate.z - want.z).abs() < 1e-3);
    assert!((t.translate.y - 39.375).abs() < 1e-3);
    assert!((t.rotate.z - spin * core::f32::consts::TAU).abs() < 1e-4);
}

/// Flames, sparks and shocks take the next of the five
/// `effect_joint_ids` each time, starting from the second.
#[test]
fn flames_cycle_the_effect_joints() {
    let bank = bank(20);
    let mut p = std::boxed::Box::new(Particles::new());
    let mut e = Effects::new(0);
    let mut f = mario();
    for j in 0..40 {
        f.joint_transforms[j] = Some(JointTransform {
            axes: [Vec3::X, Vec3::Y, Vec3::Z],
            origin: Vec3::new(j as f32 * 10.0, 0.0, 0.0),
        });
    }
    let mut rt = EffectRuntime {
        particles: &mut p,
        effects: &mut e,
        banks: &bank,
    };
    let mut xs = std::vec::Vec::new();
    for _ in 0..6 {
        let pc = {
            make(
                &mut f,
                &mut rt,
                EffectRequest::at_joint(kind::FLAME_STATIC, 0, 1),
            );
            rt.particles.list(0).next().unwrap().0
        };
        let xf = rt.particles.particle(pc).xf;
        xs.push(rt.particles.transform(xf).translate.x as i32 / 10);
    }
    // `dMarioMain_attr.effect_joint_ids`: 12, 15, 20, 25, 9.
    assert_eq!(xs, [15, 20, 25, 9, 12, 15]);
}

/// `efManagerShockSmallMakeEffect` draws five numbers (one of them
/// wasted), holds a struct for its 20-play life, and starts its process the
/// frame after it is made.
#[test]
fn a_small_shock_draws_five_times_and_holds_its_struct_for_its_life() {
    let bank = bank(20);
    let mut p = std::boxed::Box::new(Particles::new());
    let mut e = Effects::new(0);
    rng::set_seed(3);
    assert!(e.shock_small(Vec3::ZERO));
    assert_eq!(rng::seed(), after_draws(3, 5));
    assert_eq!(e.used(), 1);
    // Made during this frame's processes: not yet run.
    e.process(&mut p, &bank);
    assert_eq!(e.displays().next().unwrap().ticks, 1);
    let mut frames = 0;
    while e.displays().next().is_some() {
        e.func_run();
        e.process(&mut p, &bank);
        frames += 1;
        assert!(frames < 100);
    }
    assert_eq!(frames, usize::from(life::SHOCK_SMALL) - 1);
    assert_eq!(e.used(), 0);
}

/// The slash takes no struct (`dEFManagerDamageSlashEffectDesc` has no
/// `EFFECT_FLAG_USERDATA`); a refused shock draws nothing.
#[test]
fn the_slash_takes_no_struct_and_a_refused_effect_draws_nothing() {
    let bank = bank(200);
    let mut p = std::boxed::Box::new(Particles::new());
    let mut e = Effects::new(0);
    for _ in 0..EFFECT_ALLOC_NUM - 4 {
        assert!(e.quake(0));
    }
    let seed = rng::seed();
    assert!(!e.shock_small(Vec3::ZERO));
    assert_eq!(rng::seed(), seed);
    assert!(e.damage_slash(Vec3::ZERO, 12, 0.5));
    assert_eq!(e.free_num, 4);
    let slash = e.displays().find(|d| d.kind == DisplayKind::Slash).unwrap();
    // `(size - 5) * 0.18 + 1`.
    assert!((slash.scale.x - 2.26).abs() < 1e-5);
    let _ = (&mut p, &bank);
}

/// `efManagerDamageSpawnSparksProcUpdate`: a spark at lifetimes 8, 4 and
/// 0, flying at 18, 0 and -18 degrees on the fighter's side, then the
/// spawner ends.
#[test]
fn a_spark_spawner_throws_three_sparks_at_its_angles() {
    let bank = bank(20);
    let mut p = std::boxed::Box::new(Particles::new());
    let mut e = Effects::new(0);
    assert!(e.damage_spawn_sparks(Vec3::ZERO, -1, false));
    let mut sparks = std::vec::Vec::new();
    for _ in 0..12 {
        e.func_run();
        e.process(&mut p, &bank);
        for d in e
            .displays()
            .filter(|d| d.kind == DisplayKind::FlySparks && d.ticks == 1)
        {
            sparks.push(d.vel);
        }
    }
    let spawned = e
        .displays()
        .filter(|d| d.kind == DisplayKind::FlySparks)
        .count();
    assert_eq!(spawned, 3);
    assert_eq!(sparks.len(), 3);
    // Left-facing (`lr` -1): all fly left; up 18 degrees, level, down 18.
    let (s18, c18) = ssb_engine::math::sin_cos(18f32.to_radians());
    for (v, (sin, cos)) in sparks.iter().zip([(s18, c18), (0.0, 1.0), (-s18, c18)]) {
        assert!((v[0] + cos * 50.0).abs() < 1e-3, "{v:?}");
        assert!((v[1] - sin * 50.0).abs() < 1e-3, "{v:?}");
    }
    assert!(e.displays().all(|d| d.kind != DisplayKind::SpawnSparks));
}

/// `efManagerDamageSpawnOrbsMakeEffect`: `lifetime` 4, 8 or 12, an orb at
/// each multiple of four down to zero, and each orb lives 12 to 15
/// frames.
#[test]
fn an_orb_spawner_throws_an_orb_every_four_frames() {
    let bank = bank(20);
    let mut p = std::boxed::Box::new(Particles::new());
    for seed in 1..20 {
        let mut e = Effects::new(0);
        rng::set_seed(seed);
        let lifetime = rng::rand_int_range(3) * 4 + 4;
        rng::set_seed(seed);
        assert!(e.damage_spawn_orbs(Vec3::ZERO));
        let mut made = std::vec::Vec::new();
        for frame in 0..40 {
            e.func_run();
            e.process(&mut p, &bank);
            // A new orb has had only its maker's play.
            if e.displays()
                .any(|d| d.kind == DisplayKind::FlyOrbs && d.ticks == 1)
            {
                made.push(frame);
            }
        }
        let want: std::vec::Vec<i32> = (0..=lifetime / 4).map(|i| i * 4).collect();
        assert_eq!(made, want, "seed {seed}");
        assert_eq!(e.displays().count(), 0);
        assert_eq!(e.used(), 0);
    }
}

/// A hit's orbs, sparks and slash come through the hit pipeline's sink.
#[test]
fn a_hits_display_effects_draw_their_gate_first() {
    let mut e = Effects::new(0);
    rng::set_seed(11);
    let gate = rng::rand_int_range(4);
    rng::set_seed(11);
    let made = e.damage_spawn_orbs_random(Vec3::ZERO);
    assert_eq!(made, gate == 0);
    // A refused gate draws only itself.
    if gate != 0 {
        assert_eq!(rng::seed(), after_draws(11, 1));
    }
}

/// `dGMColScriptsFighterDamageFire*`'s flames and the fast fall's sparkle
/// are queued as the animation runs.
#[test]
fn colour_animation_effect_events_are_queued() {
    let mut f = mario();
    assert!(crate::colanim::check_set(
        &mut f,
        ColAnimId(ColAnimId::DAMAGE_FIRE_START + 1),
        0
    ));
    for _ in 0..8 {
        crate::colanim::run_update(&mut f);
    }
    let kinds: std::vec::Vec<u16> = params(&f).iter().map(|r| r.kind).collect();
    assert!(
        kinds
            .iter()
            .any(|&k| k == kind::FLAME_LR || k == kind::FLAME_RANDOM),
        "{kinds:?}"
    );
}

/// `ftParamKirbyTryMakeMapStarEffect`: only Kirby, only on a new contact.
#[test]
fn kirby_lands_with_a_star_and_mario_does_not() {
    for (k, want) in [(FighterKind::Kirby, 1), (FighterKind::Mario, 0)] {
        let mut f = Fighter::new(k, 0, 3);
        f.map_contacts.floor = true;
        kirby_map_star(&mut f);
        assert_eq!(f.effects.len(), want, "{k:?}");
        f.effects.clear();
        f.map_contacts_prev = f.map_contacts;
        kirby_map_star(&mut f);
        assert_eq!(f.effects.len(), 0);
    }
}

/// The impact wave turns with a grounded fighter's floor
/// (`syUtilsArcTan2(-floor_angle.x, floor_angle.y)`) and is level in the
/// air.
#[test]
fn the_impact_wave_follows_the_floor() {
    let bank = bank(20);
    let mut p = std::boxed::Box::new(Particles::new());
    let mut e = Effects::new(0);
    let mut f = mario();
    let n = ssb_engine::math::Vec2::new(-0.5, 0.866);
    f.floor = Some(crate::ground::Standing {
        line: 1,
        flags: 0,
        normal: n,
    });
    let mut rt = EffectRuntime {
        particles: &mut p,
        effects: &mut e,
        banks: &bank,
    };
    make(
        &mut f,
        &mut rt,
        EffectRequest::at_joint(kind::IMPACT_WAVE, 0, 1),
    );
    f.situation = Situation::Air;
    make(
        &mut f,
        &mut rt,
        EffectRequest::at_joint(kind::IMPACT_WAVE, 0, 1),
    );
    let turns: std::vec::Vec<f32> = rt.effects.displays().map(|d| d.rotate.z).collect();
    assert!((turns[0] - crate::particle::arc_tan2(0.5, 0.866)).abs() < 1e-5);
    assert!(turns[0] > 0.4);
    assert_eq!(turns[1], 0.0);
}

/// Every motion script of every fighter, and every colour animation, run a
/// frame at a time with the match's flushes, stays inside the queue.
#[test]
fn every_script_fits_the_fighter_queue() {
    let mut max = 0;
    for k in [
        FighterKind::Mario,
        FighterKind::Fox,
        FighterKind::Donkey,
        FighterKind::Samus,
        FighterKind::Luigi,
        FighterKind::Link,
        FighterKind::Yoshi,
        FighterKind::Captain,
        FighterKind::Kirby,
        FighterKind::Pikachu,
        FighterKind::Purin,
        FighterKind::Ness,
    ] {
        let table = crate::motion::fighter_scripts(k).unwrap();
        for desc in table.motions {
            let mut f = Fighter::new(k, 0, 3);
            f.motion_script.threads[0].pc = desc.script;
            f.motion_script.threads[0].wait = 0.0;
            f.motion_script.effect_threads = f.motion_script.threads;
            for _ in 0..300 {
                f.motion_script.is_events_forward = true;
                f.status.anim_frame += 1.0;
                crate::motion::advance(&mut f);
                crate::motion::end_physics(&mut f);
                max = max.max(f.effects.len());
                assert_eq!(f.effects.dropped, 0);
                f.effects.clear();
            }
        }
        for id in 0..crate::colanim::DESCS.len() as u8 {
            let mut f = Fighter::new(k, 0, 3);
            f.colanim.check_set(ColAnimId(id), 0);
            for _ in 0..300 {
                crate::colanim::run_update(&mut f);
                max = max.max(f.effects.len());
                assert_eq!(f.effects.dropped, 0);
                f.effects.clear();
            }
        }
    }
    assert!(max < EFFECT_QUEUE_MAX, "{max}");
}

/// `ftCommonDamageSetDustEffectInterval`'s steps by knockback speed, and a
/// launched fighter's first cloud on its first frame.
#[test]
fn a_launched_fighter_trails_dust_by_its_speed() {
    let mut f = mario();
    f.situation = Situation::Air;
    for (speed, want) in [
        (100.0, 0),
        (130.0, 8),
        (180.0, 5),
        (250.0, 3),
        (400.0, 2),
        (700.0, 1),
    ] {
        f.physics.vel_knockback = Vec3::new(speed, 0.0, 0.0);
        crate::reaction::set_dust_effect_interval(&mut f);
        assert_eq!(f.reaction.dust_effect_int, want, "{speed}");
    }
}

#[test]
fn a_refused_display_is_counted_only_for_a_full_pool() {
    let mut e = Effects::new(0);
    for _ in 0..crate::effect::DISPLAY_MAX {
        assert!(e.damage_slash(Vec3::ZERO, 5, 0.0));
    }
    assert!(!e.damage_slash(Vec3::ZERO, 5, 0.0));
    assert_eq!(e.displays_refused, 1);
    let _ = NIL;
}

/// `ftMainSetStatus` during the frame's passes first makes the old
/// status's pending effects: the dash's second cloud still comes when the
/// dash ends on its frame.
#[test]
fn a_status_change_in_the_frame_makes_the_old_scripts_pending_effects() {
    let mut f = mario();
    f.motion_script.is_events_forward = true;
    status::set_status(&mut f, Status::Dash, 0.0, StatusTiming::unknown());
    crate::motion::end_physics(&mut f);
    f.effects.clear();
    for frame in 1..=4 {
        f.motion_script.is_events_forward = true;
        f.status.anim_frame += 1.0;
        crate::motion::advance(&mut f);
        if frame == 4 {
            status::set_status(&mut f, Status::Wait, 0.0, StatusTiming::unknown());
            assert_eq!(params(&f).len(), 1);
            assert_eq!(params(&f)[0].kind, kind::DUST_LIGHT);
        }
        crate::motion::end_physics(&mut f);
    }
    assert_eq!(params(&f).len(), 1);
}

#[test]
fn yoshis_roll_egg_hides_on_success_and_stops_on_a_status_change() {
    let bank = bank(20);
    let mut p = Box::new(Particles::new());
    let mut e = Effects::new(0);
    let mut f = Fighter::new(FighterKind::Yoshi, 0, 3);
    f.situation = Situation::Ground;
    crate::reaction::set_escape(&mut f, Status::EscapeF);
    flush(
        &mut f,
        &mut EffectRuntime {
            particles: &mut p,
            effects: &mut e,
            banks: &bank,
        },
    );
    assert!(f.yoshi.egg_escape_active);
    assert!(f.model_parts.is_modify);
    assert!(e.displays().any(|d| d.kind == DisplayKind::YoshiEggEscape));
    let free = e.free_num;
    for _ in 0..40 {
        e.func_run();
        e.process(&mut p, &bank);
    }
    assert_eq!(e.free_num, free, "no update or animation expires the egg");
    status::set_status(&mut f, Status::Wait, 0.0, StatusTiming::unknown());
    assert!(!f.yoshi.egg_escape_active);
    assert!(!f.model_parts.is_modify);
    flush(
        &mut f,
        &mut EffectRuntime {
            particles: &mut p,
            effects: &mut e,
            banks: &bank,
        },
    );
    assert!(!e.displays().any(|d| d.kind == DisplayKind::YoshiEggEscape));
    assert_eq!(e.free_num, free + 1);
}

#[test]
fn a_roll_ending_before_flush_does_not_hide_the_next_status() {
    let bank = bank(20);
    let mut p = Box::new(Particles::new());
    let mut e = Effects::new(0);
    let mut f = Fighter::new(FighterKind::Yoshi, 0, 3);
    crate::reaction::set_escape(&mut f, Status::EscapeB);
    status::set_status(&mut f, Status::Wait, 0.0, StatusTiming::unknown());
    flush(
        &mut f,
        &mut EffectRuntime {
            particles: &mut p,
            effects: &mut e,
            banks: &bank,
        },
    );
    assert!(!f.yoshi.egg_escape_active);
    assert!(!f.model_parts.is_modify);
    assert!(!e.displays().any(|d| d.kind == DisplayKind::YoshiEggEscape));
    assert_eq!(e.free_num, EFFECT_ALLOC_NUM as u8);
}

#[test]
fn a_forced_roll_egg_uses_the_reserved_structs_and_refusal_keeps_yoshi_visible() {
    let mut e = Effects::new(0);
    for _ in 0..EFFECT_ALLOC_NUM - 4 {
        assert!(e.fire_spark(0));
    }
    assert!(!e.fire_spark(0));
    for port in 0..4 {
        assert!(e.yoshi_egg_escape(port));
    }
    assert_eq!(e.free_num, 0);
    let bank = bank(20);
    let mut p = Box::new(Particles::new());
    let mut f = Fighter::new(FighterKind::Yoshi, 0, 3);
    crate::reaction::set_escape(&mut f, Status::EscapeF);
    flush(
        &mut f,
        &mut EffectRuntime {
            particles: &mut p,
            effects: &mut e,
            banks: &bank,
        },
    );
    assert!(!f.yoshi.egg_escape_active);
    assert!(!f.model_parts.is_modify);
    e.stop_yoshi_egg_escape(2);
    assert_eq!(e.free_num, 1);
    assert_eq!(
        e.displays()
            .filter(|d| d.kind == DisplayKind::YoshiEggEscape)
            .count(),
        3
    );
}

#[test]
fn yoshi_shield_release_makes_shell_fragments_once_at_yrotn() {
    let mut f = Fighter::new(FighterKind::Yoshi, 0, 3);
    f.guard.is_shield = true;
    f.guard.is_release = true;
    f.guard.release_lag = 1;
    let origin = Vec3::new(500.0, 300.0, 20.0);
    f.joint_transforms[3] = Some(JointTransform {
        axes: [Vec3::X, Vec3::Y, Vec3::Z],
        origin,
    });
    status::guard_update_shield_vars(&mut f);
    assert!(!f.guard.is_shield);
    let effects: std::vec::Vec<_> = f.effects.iter().copied().collect();
    assert_eq!(
        effects,
        [FighterEffect::At {
            kind: kind::EGG_BREAK,
            pos: origin
        }]
    );
    status::guard_update_shield_vars(&mut f);
    assert_eq!(f.effects.len(), 1);
    let mut mario = mario();
    mario.guard.is_shield = true;
    mario.guard.is_release = true;
    status::guard_update_shield_vars(&mut mario);
    assert!(mario.effects.is_empty());
}
