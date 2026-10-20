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
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
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
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
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
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
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
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
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
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
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
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
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
    assert!((turns[0] - ssb_engine::math::sy_arc_tan2(0.5, 0.866)).abs() < 1e-5);
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
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
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
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
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

/// `is_effect_attach`: the Thunder Shock maker sets it, and a status change
/// outside `PRESERVE_EFFECT` runs `ftParamProcStopEffect`, which ejects the
/// fighter's attached effects and clears it.
#[test]
fn a_status_change_stops_the_attached_thunder_shock() {
    let bank = bank(20);
    let mut p = std::boxed::Box::new(Particles::new());
    let mut e = Effects::new(0);
    let mut f = Fighter::new(FighterKind::Pikachu, 0, 3);
    f.situation = Situation::Ground;
    f.effects.push(FighterEffect::ThunderShock {
        pos: Vec3::new(0.0, 100.0, 50.0),
        gfx: 2,
    });
    let mut rt = EffectRuntime {
        particles: &mut p,
        effects: &mut e,
        banks: &bank,
    };
    flush(&mut f, &mut rt);
    assert!(f.is_effect_attach);
    assert_eq!(rt.effects.displays().count(), 1);
    f.effects.push(FighterEffect::Pause(true));
    flush(&mut f, &mut rt);
    assert!(rt.effects.displays().all(|d| d.paused));
    on_set_status(&mut f, crate::status::AnyStatus::Common(Status::Wait));
    assert!(!f.is_effect_attach);
    flush(&mut f, &mut rt);
    assert_eq!(rt.effects.displays().count(), 0);
}

/// `ftKirbySpecialHiUpdateEffect` stops the attached effects and makes the
/// next one in one `proc_physics` (frame 21: stop, then Up): the queued
/// stop's flush must not clear the `is_effect_attach` the synchronous
/// maker set after it, or frame 24's stop is skipped and frame 25's stops
/// the new Draw. A maker queued before the stop is still cleared by it.
#[test]
fn a_flushed_stop_keeps_an_attach_made_after_it() {
    let bank = bank(20);
    let mut p = std::boxed::Box::new(Particles::new());
    let mut e = Effects::new(0);
    let mut rt = EffectRuntime {
        particles: &mut p,
        effects: &mut e,
        banks: &bank,
    };
    let mut f = Fighter::new(FighterKind::Kirby, 0, 3);
    f.is_effect_attach = true;
    stop_attached(&mut f);
    // The synchronous maker, after the stop in source order.
    f.is_effect_attach = true;
    flush(&mut f, &mut rt);
    assert!(f.is_effect_attach);

    let mut f = Fighter::new(FighterKind::Pikachu, 0, 3);
    f.situation = Situation::Ground;
    f.effects.push(FighterEffect::ThunderShock {
        pos: Vec3::new(0.0, 100.0, 50.0),
        gfx: 2,
    });
    stop_attached(&mut f);
    flush(&mut f, &mut rt);
    assert!(!f.is_effect_attach);
}

/// RE-512 trace s3 (Pikachu port 0 at (-1222.5, 1022), `lr` 1, TopN scale
/// 0.95): `func_ovl2_800EE018` takes the spark's joint-11 world point `A`
/// into TopN's frame `B` through the inverse of TopN's collision matrix,
/// for the six sparks of the first smash. Bit patterns from the trace.
#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn thunder_shock_points_enter_topn_frame_as_the_trace_records() {
    use crate::slope_contour::{invert, transform_matrix, world_position};
    let v = |b: [u32; 3]| {
        Vec3::new(
            f32::from_bits(b[0]),
            f32::from_bits(b[1]),
            f32::from_bits(b[2]),
        )
    };
    // (TopN translate.z, A, B)
    let cases: [(u32, [u32; 3], [u32; 3]); 6] = [
        (
            0x4170_0000,
            [0xc491_5b4a, 0x4492_206c, 0x4170_3e37],
            [0xbc82_fc00, 0x431a_c030, 0x427b_2560],
        ),
        (
            0x40c0_0000,
            [0xc47a_0f8a, 0x448d_d791, 0xc2ae_c1e8],
            [0x42c4_9638, 0x42ed_5700, 0x4369_f478],
        ),
        (
            0x0000_0000,
            [0xc469_6246, 0x448e_ae54, 0x428d_c071],
            [0xc295_365c, 0x42fb_7810, 0x4398_162a],
        ),
        (
            0x0000_0000,
            [0xc46a_b684, 0x448b_5230, 0xc25e_12cd],
            [0x4269_c2f3, 0x42c2_e170, 0x4395_49de],
        ),
        (
            0x0000_0000,
            [0xc46a_c322, 0x4488_d456, 0x4233_e7e9],
            [0xc23d_5fe8, 0x4298_eab0, 0x4395_2f4e],
        ),
        (
            0x0000_0000,
            [0xc46b_7402, 0x448a_4698, 0xc257_77d8],
            [0x4262_ceff, 0x42b1_46a0, 0x4393_baf0],
        ),
    ];
    let scale = f32::from_bits(0x3f73_3333);
    for (i, (tz, a, b)) in cases.into_iter().enumerate() {
        let top = transform_matrix(
            v([0xc498_d000, 0x447f_8000, tz]),
            [0.0, f32::from_bits(0x3fc9_0fdb), 0.0],
            [scale; 3],
        );
        let got = world_position(&invert(&top), v(a));
        let want = v(b);
        let ulps = |x: f32, y: f32| (x.to_bits() as i64 - y.to_bits() as i64).abs();
        assert!(
            ulps(got.x, want.x) <= 2 && ulps(got.y, want.y) <= 2 && ulps(got.z, want.z) <= 2,
            "spark {}: {got:?} vs {want:?}",
            i + 1
        );
    }
}

/// RE-512 trace s3: a spark lives 6 frames (animation frames 0..=5), and a
/// spark made on the frame its fighter's hitlag starts holds frame 0
/// through the hitlag: 16 frames of hitlag give 21 frames, 9 give 14. The
/// frame runs as the match does: `efManagerFuncRun`, the interrupt pass
/// (`proc_lagend` when hitlag runs out), the update pass (the maker), the
/// effect processes, then the hit pass (`proc_lagstart`).
#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn a_thunder_shock_spark_lives_six_frames_plus_the_hitlag_it_starts_in() {
    for (hitlag, life_frames) in [(0u16, 6usize), (16, 21), (9, 14)] {
        let bank = bank(20);
        let mut p = std::boxed::Box::new(Particles::new());
        let mut e = Effects::new(0);
        let mut f = Fighter::new(FighterKind::Pikachu, 0, 3);
        f.situation = Situation::Ground;
        f.lag_pauses_effects = true;
        let mut rt = EffectRuntime {
            particles: &mut p,
            effects: &mut e,
            banks: &bank,
        };
        let mut frames = std::vec::Vec::new();
        for frame in 0..40 {
            rt.run();
            f.tick_timers();
            flush(&mut f, &mut rt);
            if frame == 0 {
                f.effects.push(FighterEffect::ThunderShock {
                    pos: Vec3::new(0.0, 150.0, 60.0),
                    gfx: 2,
                });
                flush(&mut f, &mut rt);
            }
            rt.effects.process(rt.particles, rt.banks);
            if frame == 0 && hitlag > 0 {
                // `ftMainProcParams`'s hitlag start (`combat::proc_params_with`).
                f.hitlag = hitlag;
                f.effects.push(FighterEffect::Pause(true));
                flush(&mut f, &mut rt);
            }
            if let Some(d) = rt.effects.displays().next() {
                frames.push(d.ticks - 1);
            }
        }
        let mut want = std::vec![0u16; usize::from(hitlag.max(1))];
        want.extend(1..=5);
        assert_eq!(frames.len(), life_frames, "hitlag {hitlag}: {frames:?}");
        assert_eq!(frames, want, "hitlag {hitlag}");
    }
}

/// RE-512 trace s3: Pikachu's forward smash makes six sparks, the first on
/// the 20th frame after the status is set and then every third frame.
#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn pikachu_forward_smash_makes_six_sparks_every_third_frame_from_frame_twenty() {
    use crate::collision::{flags, Segment};
    let floor = || {
        [(
            0u16,
            Segment {
                x1: -2318,
                y1: 0,
                x2: 2318,
                y2: 0,
                flags: flags::CLIFF,
            },
        )]
    };
    let mut f = Fighter::new(FighterKind::Pikachu, 0, 3);
    f.pos = Vec3::new(0.0, 4.0, 0.0);
    assert!(f.place_on_stage(floor()));
    status::set_wait(&mut f);
    status::set_fsmash(&mut f);
    f.effects.clear();
    let mut made = std::vec::Vec::new();
    for frame in 1..=60 {
        f.tick(floor);
        for e in f.effects.iter() {
            if let FighterEffect::ThunderShock { .. } = e {
                made.push(frame);
            }
        }
        f.effects.clear();
    }
    assert_eq!(made, [20, 23, 26, 29, 32, 35]);
}

/// RE-512 trace s3, port 0's TopN `FTParts` words (`transform_update_mode`,
/// `unk_dobjtrans_0x5`) at the end of each frame of two forward smashes:
/// status 204 set on frames 454 and 597, the first spark on +20 with 16
/// and 9 frames of hitlag. A live spark's display (`func_ovl0_800C994C`)
/// marks TopN (mode 1, byte 5 1) on every frame a spark lives (474..=509,
/// 617..=645); on the frame the last one is gone the attack collision's
/// position query (`gmCollisionGetFighterPartsWorldPosition`) still sets
/// the mode alone (510, 646), and the frame after nothing does (511, 647).
#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn a_live_thunder_shock_spark_marks_topn_as_the_trace_records() {
    use crate::collision::{flags, Segment};
    let floor = || {
        [(
            0u16,
            Segment {
                x1: -2318,
                y1: 0,
                x2: 2318,
                y2: 0,
                flags: flags::CLIFF,
            },
        )]
    };
    // (status frame, hitlag, last frame with both marks, frames sampled)
    for (start, hitlag, last_marked) in [(454u32, 16u16, 509u32), (597, 9, 645)] {
        let bank = bank(20);
        let mut p = std::boxed::Box::new(Particles::new());
        let mut e = Effects::new(0);
        let mut rt = EffectRuntime {
            particles: &mut p,
            effects: &mut e,
            banks: &bank,
        };
        let mut f = Fighter::new(FighterKind::Pikachu, 0, 3);
        f.pos = Vec3::new(0.0, 4.0, 0.0);
        assert!(f.place_on_stage(floor()));
        status::set_wait(&mut f);
        status::set_fsmash(&mut f);
        f.effects.clear();
        let mut got = std::vec::Vec::new();
        for frame in start + 1..=last_marked + 4 {
            rt.run();
            // The match's order: each pass's effects made after it.
            let surfaces = || floor().into_iter().map(crate::map::floor_surface);
            f.tick_interrupt(&surfaces);
            flush(&mut f, &mut rt);
            f.tick_physics_map(&surfaces);
            flush(&mut f, &mut rt);
            crate::combat::update_attack_positions(&mut f);
            rt.effects.process(rt.particles, rt.banks);
            if frame == start + 20 {
                f.hitlag = hitlag;
                f.effects.push(FighterEffect::Pause(true));
                flush(&mut f, &mut rt);
            }
            // The display: one `func_ovl0_800C994C` per live spark.
            for d in rt.effects.displays() {
                if let DisplayKind::ThunderShock { .. } = d.kind {
                    let _ = f.topn_world_matrix();
                }
            }
            got.push((
                frame,
                f.queried_joints.get() & 1 != 0,
                f.world_joints.get() & 1 != 0,
            ));
        }
        let want: std::vec::Vec<_> = (start + 1..=last_marked + 4)
            .map(|frame| {
                let marked = (start + 20..=last_marked).contains(&frame);
                (frame, marked || frame == last_marked + 1, marked)
            })
            .collect();
        assert_eq!(got, want, "smash from {start}");
    }
}

/// RE-513 trace k1, Kirby's rapid jab from `ftCommonAttack100LoopSetStatus`
/// (frame 1012) with A mashed: the puffs `ftCommonAttack100LoopKirbyUpdateEffect`
/// makes (frame, `rotate` of the row) and TopN's `transform_update_mode`
/// at the end of each frame. The first puff comes from the setter's
/// frame-0 events, the later ones from flag 2 at loop frames 6, 11, 16 and
/// 21, then 2 after the wrap (the script resumes at `SetFlag1`, then
/// `Goto JabLoop`). The puff's query marks TopN on its frame, as the jab
/// hitbox's position queries do on its two frames; the next frame's
/// `ftMainPlayAnim` clears it.
#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn kirbys_vulcan_jab_puffs_and_topn_marks_follow_the_trace() {
    use crate::collision::{flags, Segment};
    use ssb_engine::input::{ControllerState, N64Buttons};
    let floor = || {
        [(
            0u16,
            Segment {
                x1: -2318,
                y1: 0,
                x2: 2318,
                y2: 0,
                flags: flags::CLIFF,
            },
        )]
    };
    let bank = bank(20);
    let mut p = std::boxed::Box::new(Particles::new());
    let mut e = Effects::new(0);
    let mut rt = EffectRuntime {
        particles: &mut p,
        effects: &mut e,
        banks: &bank,
    };
    let mut f = Fighter::new(FighterKind::Kirby, 0, 4);
    f.pos = Vec3::new(0.0, 4.0, 0.0);
    assert!(f.place_on_stage(floor()));
    status::set_wait(&mut f);
    crate::kirby::set_attack100_start(&mut f);
    f.effects.clear();
    let looping = crate::status::AnyStatus::Kirby(crate::status::KirbyStatus::Attack100Loop);
    let mut frame = None::<u32>;
    let mut puffs = std::vec::Vec::new();
    let mut marked = std::vec::Vec::new();
    for tick in 0u32..80 {
        f.set_input(
            ControllerState {
                buttons: N64Buttons(if tick.is_multiple_of(2) {
                    N64Buttons::A
                } else {
                    0
                }),
                ..Default::default()
            },
            false,
            false,
        );
        rt.run();
        let surfaces = || floor().into_iter().map(crate::map::floor_surface);
        f.tick_interrupt(&surfaces);
        flush(&mut f, &mut rt);
        f.tick_physics_map(&surfaces);
        flush(&mut f, &mut rt);
        crate::combat::update_attack_positions(&mut f);
        rt.effects.process(rt.particles, rt.banks);
        if frame.is_none() && f.status.status == looping {
            frame = Some(1012);
        }
        let Some(now) = frame else { continue };
        assert_eq!(f.status.status, looping, "frame {now}");
        for d in rt.effects.displays() {
            if d.kind == DisplayKind::VulcanJab && d.lifetime == 6 {
                assert_eq!(d.lr, 1);
                puffs.push((now, (d.rotate.z.to_degrees()).round() as i32));
            }
        }
        if f.queried_joints.get() & 1 != 0 {
            marked.push(now);
        }
        frame = Some(now + 1);
        if now == 1044 {
            break;
        }
    }
    assert_eq!(
        puffs,
        [
            (1012, 0),
            (1018, 19),
            (1023, -13),
            (1028, 10),
            (1033, -10),
            (1039, 0),
            (1043, 19)
        ]
    );
    assert_eq!(
        marked,
        [
            1012, 1013, 1018, 1019, 1023, 1024, 1028, 1029, 1033, 1034, 1037, 1038, 1039, 1040,
            1043, 1044
        ]
    );
}

/// `dDonkeyMainMotion_AppearR` bursts Donkey Kong out of his entry barrel
/// with `Effect(.., BoxSmash, ..)`: `ftParamMakeEffect` calls
/// `itBoxContainerSmashMakeEffect`, seven crate staves from one struct and
/// the source's 42 draws.
#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn donkey_kongs_box_smash_makes_the_crate_staves() {
    let bank = bank(20);
    let mut p = Box::new(Particles::new());
    let mut e = Effects::new(0);
    let mut f = Fighter::new(FighterKind::Donkey, 0, 3);
    f.situation = Situation::Ground;
    rng::set_seed(7);
    let free = e.free_num;
    make(
        &mut f,
        &mut EffectRuntime {
            particles: &mut p,
            effects: &mut e,
            banks: &bank,
        },
        EffectRequest {
            kind: kind::BOX_SMASH,
            joint: 0,
            offset: Some(Vec3::new(0.0, 250.0, 0.0)),
            scatter: None,
            lr: 1,
            is_scale_pos: false,
            flag: 0,
        },
    );
    assert_eq!(rng::seed(), after_draws(7, 42));
    assert_eq!(e.free_num, free - 1);
    let d = e
        .displays()
        .find(|d| d.kind == DisplayKind::ContainerSmash)
        .expect("the staves are made");
    let (piece, pieces) = e.container_pieces(d).unwrap();
    assert_eq!(piece, crate::effect::SmashPiece::Box);
    assert!(pieces.iter().all(|s| s.pos == pieces[0].pos));
}

/// A surface strike's queue makes, in its order, the impact wave (axis Z,
/// turned by the surface) and a magnitude-2 quake, whose camera write
/// lands after the camera's process (RE-503).
#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn a_surface_impact_flushes_a_wave_and_a_late_quake_in_queue_order() {
    use crate::effect::QuakeSlot;
    let bank = bank(20);
    for quake_first in [false, true] {
        let mut p = Box::new(Particles::new());
        let mut e = Effects::new(0);
        let mut f = mario();
        let normal = ssb_engine::math::Vec2::new(-0.6, 0.8);
        let pos = Vec3::new(12.0, 34.0, 0.0);
        surface_impact(&mut f, pos, normal, quake_first);
        let mut rt = EffectRuntime {
            particles: &mut p,
            effects: &mut e,
            banks: &bank,
        };
        flush(&mut f, &mut rt);
        let kinds: std::vec::Vec<DisplayKind> = e.displays().map(|d| d.kind).collect();
        let (wave, quake) = (DisplayKind::ImpactWave, DisplayKind::Quake { magnitude: 2 });
        let want = if quake_first {
            [quake, wave]
        } else {
            [wave, quake]
        };
        assert_eq!(kinds, want);
        let d = e.displays().find(|d| d.kind == wave).unwrap();
        assert_eq!(d.translate, pos);
        assert_eq!(d.rotate.z, ssb_engine::math::sy_arc_tan2(0.6, 0.8));
        assert_eq!(d.index, 4);
        let mut rt = EffectRuntime {
            particles: &mut p,
            effects: &mut e,
            banks: &bank,
        };
        rt.frame();
        assert_eq!(rt.effects.take_quake(QuakeSlot::BeforeCamera), None);
        assert!(rt
            .effects
            .take_quake(QuakeSlot::AfterCamera)
            .is_some_and(|(m, _)| m == 2));
    }
}

/// RE-540: `ftParamPauseEffect` sets only `ep->is_pause_effect`, which the
/// display procs read (`efManagerHaveStructProcUpdate`,
/// `efManagerNoEjectProcUpdate`), and `lbParticlePauseAllID` has no caller.
/// Kirby's inhale wind, the one particle effect attached to a fighter,
/// keeps ticking and following its owner through a hitlag pause.
#[test]
fn a_paused_fighters_attached_particle_keeps_running_and_following() {
    use crate::particle::tests::{any_bank, live_structs};
    let bank = any_bank();
    let mut p = std::boxed::Box::new(Particles::new());
    let mut e = Effects::new(0);
    let mut f = Fighter::new(FighterKind::Kirby, 0, 3);
    f.pos = Vec3::new(100.0, 0.0, 0.0);
    let mut rt = EffectRuntime {
        particles: &mut p,
        effects: &mut e,
        banks: &bank,
    };
    f.effects.push(FighterEffect::KirbyInhaleWind);
    flush(&mut f, &mut rt);
    assert!(f.is_effect_attach);
    let wind = live_structs(rt.particles);
    assert_eq!(wind.len(), 1);
    let lifetime = |p: &Particles| {
        (0..crate::particle::LINKS_NUM)
            .flat_map(|l| p.list(l).map(|(_, s)| s.lifetime))
            .next()
    };
    let before = lifetime(rt.particles).unwrap();
    f.effects.push(FighterEffect::Pause(true));
    flush(&mut f, &mut rt);
    rt.effects.fighter_anchors[0] = Some((Vec3::new(500.0, 0.0, 0.0), 1));
    rt.frame();
    assert_eq!(live_structs(rt.particles), wind, "still alive");
    assert!(
        lifetime(rt.particles).unwrap() < before,
        "and still ticking"
    );
    let t = rt.particles.transform(wind[0]).translate;
    assert_eq!((t.x, t.y), (500.0 + 800.0, 230.0), "following its owner");
}

/// `ftCommonThrownKirbyStarProcUpdate` makes the captured star once
/// (`is_effect_attach` guards `ftCommonThrownKirbyStarMakeEffect`), attached
/// to the spat fighter; the Fall's `ftMainSetStatus` stops it.
#[test]
fn a_spat_star_makes_its_effect_once_and_the_fall_stops_it() {
    use crate::effect::HitEffectSink;
    let bank = bank(200);
    let mut p = std::boxed::Box::new(Particles::new());
    let mut e = Effects::new(0);
    let mut f = Fighter::new(FighterKind::Mario, 1, 4);
    crate::capture_kirby::set_star(
        &mut f,
        false,
        Vec3::new(120.0, 0.0, 0.0),
        crate::thrown::ThrowOwner {
            port: 0,
            kind: FighterKind::Kirby,
            team: 0,
        },
    );
    let mut rt = EffectRuntime {
        particles: &mut p,
        effects: &mut e,
        banks: &bank,
    };
    let stars = |e: &Effects| {
        e.displays()
            .filter(|d| d.kind == DisplayKind::CaptureKirbyStar)
            .map(|d| d.owner)
            .collect::<std::vec::Vec<_>>()
    };
    status::update(&mut f);
    status::update(&mut f);
    let queued = f
        .effects
        .iter()
        .filter(|e| matches!(e, FighterEffect::CaptureKirbyStar))
        .count();
    assert_eq!(queued, 1);
    rt.fighter(&mut f);
    assert!(f.is_effect_attach);
    assert_eq!(stars(rt.effects), [1]);
    // The release (`flag2` 2) stops the star and makes a splash, whose
    // `is_effect_attach` keeps the star from being made again through the
    // four countdown frames (N64: gone at the release, Fall four frames
    // later).
    let mut released = None;
    for n in 0..40 {
        status::update(&mut f);
        crate::capture_kirby::apply_air_physics(&mut f);
        rt.fighter(&mut f);
        if f.status.status == Status::Fall {
            assert_eq!(released.map(|r| n - r), Some(4));
            break;
        }
        if f.kirby_capture.flag2 == 3 {
            released.get_or_insert(n);
            assert!(stars(rt.effects).is_empty());
            assert!(f.is_effect_attach, "the splash's");
            assert_eq!(rt.particles.generators().count(), 1);
        } else {
            assert_eq!(stars(rt.effects), [1]);
        }
    }
    assert_eq!(f.status.status, Status::Fall);
    assert!(stars(rt.effects).is_empty());
    assert!(!f.is_effect_attach);
}

/// `ftKirbySpecialNLoseCopy` drops the lost star from where Kirby stands,
/// facing Kirby's way; it is not attached.
#[test]
fn losing_a_copy_drops_the_lost_star() {
    let bank = bank(200);
    let mut p = std::boxed::Box::new(Particles::new());
    let mut e = Effects::new(0);
    let mut f = Fighter::new(FighterKind::Kirby, 2, 4);
    f.kirby.copy_id = FighterKind::Mario;
    f.pos = Vec3::new(-250.0, 30.0, 0.0);
    f.facing = crate::fighter::Facing::Left;
    crate::kirby::lose_copy(&mut f);
    assert_eq!(f.kirby.copy_id, FighterKind::Kirby);
    let mut rt = EffectRuntime {
        particles: &mut p,
        effects: &mut e,
        banks: &bank,
    };
    flush(&mut f, &mut rt);
    let d = *rt.effects.displays().next().unwrap();
    assert_eq!(d.kind, DisplayKind::LoseKirbyStar);
    assert_eq!(d.translate, f.pos);
    assert_eq!(d.vel, [-20.0, 80.0]);
    assert!(!f.is_effect_attach);
}
