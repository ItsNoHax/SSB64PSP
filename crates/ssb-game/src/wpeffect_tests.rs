use super::*;
use crate::collision::Segment;
use crate::effect::{DisplayKind, EffectRuntime, HitEffect, HitEffectSink, NoEffects};
use crate::fighter::{Fighter, FighterKind, Situation};
use crate::particle::tests::TestBank;
use crate::particle::NIL;
use crate::rng;
use crate::stale::WeaponStale;
use crate::team::TeamRules;
use crate::weapon::{
    MapSurface, MapSurfaceKind, WeaponKind, WeaponPool, WeaponSpawn, MARIO_FIREBALL_LIFETIME,
};

/// Records the weapon effects it is handed.
#[derive(Default)]
struct Record(std::vec::Vec<WeaponEffect>);

impl HitEffectSink for Record {
    fn make(&mut self, _: &HitEffect) {}
    fn weapon(&mut self, e: &WeaponEffect) {
        self.0.push(*e);
    }
}

fn spawn(kind: WeaponKind, owner: u8, x: f32, facing: f32) -> WeaponSpawn {
    WeaponSpawn {
        kind,
        owner_port: owner,
        team: owner,
        position: Vec3::new(x, 0.0, 0.0),
        facing,
        stale: WeaponStale::FRESH,
    }
}

fn open_air() -> [MapSurface; 0] {
    []
}

fn ceiling(y: i16) -> MapSurface {
    MapSurface {
        kind: MapSurfaceKind::Ceiling,
        segment: Segment {
            x1: -5000,
            y1: y,
            x2: 5000,
            y2: y,
            flags: 0,
        },
        topology: None,
        motion: None,
    }
}

/// One frame's clash search and reactions, returning the set-offs and the
/// reactions' effects.
fn clash_frame(
    pool: &mut WeaponPool,
) -> (std::vec::Vec<WeaponEffect>, std::vec::Vec<WeaponEffect>) {
    pool.search_weapons();
    pool.finish_clashes();
    let mut setoffs = Record::default();
    pool.flush_clash_effects(&mut setoffs);
    let mut reactions = Record::default();
    pool.flush_effects(&mut reactions);
    (setoffs.0, reactions.0)
}

fn sizes(fx: &[WeaponEffect]) -> std::vec::Vec<i32> {
    fx.iter()
        .filter_map(|e| match e {
            WeaponEffect::SetOff { size, .. } => Some(*size),
            _ => None,
        })
        .collect()
}

/// `wpProcessUpdateAttackStatWeapon`: two equal-priority weapons both stop,
/// the searched (earlier) weapon's set-off first, each at its own staled
/// damage; each then runs its `proc_setoff` (the Fireball's sparkle).
#[test]
fn two_fireballs_clash_and_both_set_off_the_earlier_first() {
    let mut pool = WeaponPool::default();
    assert!(pool.spawn(spawn(WeaponKind::MarioFireball, 0, 0.0, 1.0)));
    assert!(pool.spawn(spawn(WeaponKind::LuigiFireball, 1, 150.0, -1.0)));
    let (setoffs, reactions) = clash_frame(&mut pool);
    // Mario's Fireball deals 7, Luigi's 6.
    assert_eq!(sizes(&setoffs), [7, 6]);
    let mid = match setoffs[0] {
        WeaponEffect::SetOff { pos, .. } => pos,
        _ => unreachable!(),
    };
    assert!(mid.x > 0.0 && mid.x < 150.0, "{mid:?}");
    assert_eq!(pool.active_count(), 0);
    assert_eq!(
        reactions,
        [
            WeaponEffect::SparkleWhite(Vec3::new(0.0, 0.0, 0.0)),
            WeaponEffect::SparkleWhite(Vec3::new(150.0, 0.0, 0.0)),
        ]
    );
}

/// A weapon never clashes with its owner's, nor with its team's while team
/// attack is off.
#[test]
fn owners_and_teams_spare_their_weapons() {
    let mut pool = WeaponPool::default();
    pool.spawn(spawn(WeaponKind::MarioFireball, 0, 0.0, 1.0));
    pool.spawn(spawn(WeaponKind::MarioFireball, 0, 100.0, -1.0));
    assert!(clash_frame(&mut pool).0.is_empty());
    assert_eq!(pool.active_count(), 2);

    let mut team = WeaponPool::default();
    team.team_rules = TeamRules {
        is_team_battle: true,
        is_team_attack: false,
    };
    let mut a = spawn(WeaponKind::MarioFireball, 0, 0.0, 1.0);
    let mut b = spawn(WeaponKind::LuigiFireball, 1, 100.0, -1.0);
    a.team = 1;
    b.team = 1;
    team.spawn(a);
    team.spawn(b);
    assert!(clash_frame(&mut team).0.is_empty());
    team.team_rules.is_team_attack = true;
    assert_eq!(clash_frame(&mut team).0.len(), 2);
}

/// Priority: the full charge (`dWPSamusChargeShotWeaponAttributes[7]`,
/// priority 2) beats a Fireball (1), which alone stops.
#[test]
fn a_full_charge_shot_outranks_a_fireball() {
    let mut pool = WeaponPool::default();
    pool.spawn(spawn(WeaponKind::SamusChargeShot(7), 0, 0.0, 1.0));
    pool.spawn(spawn(WeaponKind::MarioFireball, 1, 100.0, -1.0));
    let (setoffs, reactions) = clash_frame(&mut pool);
    assert_eq!(sizes(&setoffs), [7]);
    assert_eq!(pool.charge_shots().count(), 1);
    assert_eq!(pool.fireballs().count(), 0);
    assert_eq!(reactions.len(), 1);

    // A lower charge has the Fireball's priority: both stop, the shot with
    // an impact shock at its unstaled damage.
    let mut pool = WeaponPool::default();
    pool.spawn(spawn(WeaponKind::SamusChargeShot(3), 0, 0.0, 1.0));
    pool.spawn(spawn(WeaponKind::MarioFireball, 1, 100.0, -1.0));
    let (setoffs, reactions) = clash_frame(&mut pool);
    assert_eq!(setoffs.len(), 2);
    assert_eq!(pool.active_count(), 0);
    assert!(matches!(
        reactions[0],
        WeaponEffect::ImpactShock { size: 12, .. }
    ));
}

/// The clash records each side on the other: two Boomerangs that turned
/// back on their first clash do not clash again.
#[test]
fn a_clashed_pair_is_recorded_and_passes_through_afterwards() {
    let mut pool = WeaponPool::default();
    let boomerang = WeaponKind::LinkBoomerang {
        is_smash: false,
        stick_x: 0,
        stick_y: 0,
    };
    pool.spawn(spawn(boomerang, 0, 0.0, 1.0));
    pool.spawn(spawn(boomerang, 1, 200.0, -1.0));
    let (setoffs, reactions) = clash_frame(&mut pool);
    assert_eq!(setoffs.len(), 2);
    // `wpLinkBoomerangProcSetOff` makes nothing and sends it back.
    assert!(reactions.is_empty());
    assert!(pool.boomerangs().all(|b| b.is_return));
    assert!(clash_frame(&mut pool).0.is_empty());
}

/// `can_setoff` is clear for the Blaster and the Bomb: they pass through.
#[test]
fn a_blaster_and_a_bomb_never_clash() {
    for kind in [WeaponKind::FoxBlaster, WeaponKind::SamusBomb] {
        let mut pool = WeaponPool::default();
        pool.spawn(spawn(kind, 0, 0.0, 1.0));
        pool.spawn(spawn(WeaponKind::MarioFireball, 1, 50.0, -1.0));
        pool.flush_effects(&mut Record::default());
        assert!(clash_frame(&mut pool).0.is_empty(), "{kind:?}");
        assert_eq!(pool.active_count(), 2);
    }
}

/// The Star's `proc_setoff` (`wpYoshiStarProcHit`) sparkles and, without
/// `hit_normal_damage`, lets it fly on; the Final Cutter's destroys it.
#[test]
fn a_star_survives_its_set_off_and_a_cutter_does_not() {
    let mut pool = WeaponPool::default();
    pool.spawn(spawn(
        WeaponKind::KirbyCutter { grounded: false },
        0,
        0.0,
        1.0,
    ));
    pool.spawn(spawn(WeaponKind::YoshiStars, 1, -300.0, 1.0));
    let (setoffs, reactions) = clash_frame(&mut pool);
    assert!(!setoffs.is_empty());
    assert_eq!(pool.cutters().count(), 0);
    assert_eq!(pool.stars().count(), 2);
    assert!(reactions
        .iter()
        .all(|e| matches!(e, WeaponEffect::SparkleWhite(_))));
    assert_eq!(reactions.len(), 2);
}

/// `wpMarioFireballProcUpdate`: the dust cloud on its last frame, where it
/// was before that frame's move, and nothing before.
#[test]
fn a_fireball_makes_its_dust_on_the_frame_it_expires() {
    let mut pool = WeaponPool::default();
    pool.spawn(spawn(WeaponKind::MarioFireball, 0, 0.0, 1.0));
    let mut last = Vec3::ZERO;
    for tick in 1..=MARIO_FIREBALL_LIFETIME {
        if let Some(f) = pool.first_fireball() {
            last = f.position;
        }
        pool.tick(open_air, None);
        let mut r = Record::default();
        pool.flush_effects(&mut r);
        if tick < MARIO_FIREBALL_LIFETIME {
            assert!(r.0.is_empty(), "{tick}");
        } else {
            assert_eq!(r.0, [WeaponEffect::DustExpandSmall(last)]);
        }
    }
    assert_eq!(pool.active_count(), 0);
}

/// The queue is made in link order, not slot order: stars made into a
/// freed first slot come after the older Fireball.
#[test]
fn effects_are_made_in_link_order() {
    let mut pool = WeaponPool::default();
    // Slot 0: Luigi's Fireball (80 frames); slot 1: Mario's (140).
    pool.spawn(spawn(WeaponKind::LuigiFireball, 0, 0.0, 1.0));
    pool.spawn(spawn(WeaponKind::MarioFireball, 1, 0.0, 1.0));
    for tick in 1..=MARIO_FIREBALL_LIFETIME {
        if tick == MARIO_FIREBALL_LIFETIME - crate::weapon::YOSHISTAR_LIFETIME + 1 {
            // Two stars: the freed slot 0, then slot 2. They expire with the
            // Mario Fireball.
            assert!(pool.spawn(spawn(WeaponKind::YoshiStars, 2, -5000.0, 1.0)));
        }
        pool.tick(open_air, None);
        let mut r = Record::default();
        pool.flush_effects(&mut r);
        if tick == MARIO_FIREBALL_LIFETIME {
            let xs: std::vec::Vec<f32> =
                r.0.iter()
                    .map(|e| match e {
                        WeaponEffect::DustExpandSmall(p) => p.x,
                        _ => panic!("{e:?}"),
                    })
                    .collect();
            assert_eq!(xs.len(), 3);
            // The Fireball first (made first), then the stars.
            assert!(xs[0] > 0.0, "{xs:?}");
            assert!(xs[1] < -4000.0 && xs[2] < -4000.0, "{xs:?}");
        }
    }
}

/// `wpYoshiEggThrowProcMap`: a quake, the (unloaded) explosion, the shell
/// and a dust cloud, in that order; sixteen eggs landing together fit the
/// queue.
#[test]
fn an_egg_landing_makes_its_four_effects_and_sixteen_fit() {
    let mut pool = WeaponPool::default();
    let egg = WeaponKind::YoshiEgg {
        throw_force: 0,
        stick_x: 0,
    };
    for i in 0..16 {
        let mut s = spawn(egg, (i % 4) as u8, i as f32 * 300.0, 1.0);
        s.position.y = 60.0;
        assert!(pool.spawn(s));
    }
    // Thrown up at 73 degrees into a ceiling above.
    let surfaces = || [ceiling(400)];
    let mut r = Record::default();
    for _ in 0..8 {
        pool.tick(surfaces, None);
        pool.flush_effects(&mut r);
        if !r.0.is_empty() {
            break;
        }
    }
    assert_eq!(r.0.len(), 64);
    assert!(matches!(r.0[0], WeaponEffect::Quake(2)));
    assert!(matches!(r.0[1], WeaponEffect::YoshiEggExplode(_)));
    assert!(matches!(r.0[2], WeaponEffect::EggBreak(_)));
    assert!(matches!(r.0[3], WeaponEffect::DustExpandSmall(_)));
    assert_eq!(pool.effects_dropped(), 0);
}

/// The Blaster glows when made and when it meets a wall.
#[test]
fn a_blaster_glows_when_made_and_at_a_wall() {
    let mut pool = WeaponPool::default();
    pool.spawn(spawn(WeaponKind::FoxBlaster, 0, 0.0, 1.0));
    let mut r = Record::default();
    pool.flush_effects(&mut r);
    assert_eq!(r.0, [WeaponEffect::FoxBlasterGlow(Vec3::ZERO)]);
    let wall = MapSurface {
        kind: MapSurfaceKind::LeftWall,
        segment: Segment {
            x1: 300,
            y1: 1000,
            x2: 300,
            y2: -1000,
            flags: 0,
        },
        topology: None,
        motion: None,
    };
    let mut r = Record::default();
    for _ in 0..4 {
        pool.tick(|| [wall], None);
        pool.flush_effects(&mut r);
    }
    assert_eq!(pool.active_count(), 0);
    assert!(
        matches!(r.0[..], [WeaponEffect::FoxBlasterGlow(_)]),
        "{:?}",
        r.0
    );
}

/// A Fireball's `proc_hit` sparkles where it hit a fighter; the effect
/// waits for the hit collisions' flush.
#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn a_fireball_sparkles_where_it_hits() {
    let mut pool = WeaponPool::default();
    pool.spawn(spawn(WeaponKind::MarioFireball, 0, 0.0, 1.0));
    let fireball = pool.first_fireball().unwrap();
    let mut target = Fighter::new(FighterKind::Mario, 1, 3);
    target.pos = fireball.position - Vec3::new(0.0, 150.0, 0.0);
    target.situation = Situation::Ground;
    pool.apply_hits(&mut target);
    assert_eq!(pool.active_count(), 0);
    let mut r = Record::default();
    pool.flush_effects(&mut r);
    assert_eq!(r.0, [WeaponEffect::SparkleWhite(fireball.position)]);
}

/// Pikachu's Thunder segment picks a frame every frame it lives
/// (`WPPIKACHUTHUNDER_TEXTURES_NUM - 1`) and becomes a fading effect below
/// `WPPIKACHUTHUNDER_EXPIRE`. The flush draws the frame itself and keeps it
/// on the trail for its draw (RE-417); the sink sees only the effect.
#[test]
fn a_thunder_segment_draws_its_frame_and_ends_as_an_effect() {
    let mut pool = WeaponPool::default();
    pool.spawn(spawn(WeaponKind::PikachuThunder, 0, 0.0, 1.0));
    pool.tick(open_air, None);
    let mut r = Record::default();
    pool.flush_effects(&mut r);
    assert!(r.0.is_empty(), "{:?}", r.0);
    // The first trail was made that frame, at `gcAddMObjForDObj`'s 0.
    let first = |pool: &WeaponPool| {
        pool.thunder_trails()
            .next()
            .map(|t| (t.position.y, t.texture))
    };
    let (made_at, texture) = first(&pool).unwrap();
    assert_eq!(texture, 0);
    // Four frames with a drawn texture, then the effect on its fifth.
    rng::set_seed(11);
    for frame in 0..5 {
        let before = rng::seed();
        pool.tick(open_air, None);
        let mut r = Record::default();
        pool.flush_effects(&mut r);
        if frame < 4 {
            assert!(
                !r.0.iter()
                    .any(|e| matches!(e, WeaponEffect::TextureRand(_))),
                "{:?}",
                r.0
            );
            // The first trail in the link draws first.
            rng::set_seed(before);
            let want = rng::rand_int_range(3) as u8;
            let (y, got) = first(&pool).unwrap();
            assert_eq!(y, made_at);
            assert_eq!(got, want, "frame {frame}");
            assert!(got < 3);
        } else {
            assert!(matches!(
                r.0.first(),
                Some(WeaponEffect::ThunderTrail {
                    lifetime: 6,
                    texture: 0,
                    ..
                })
            ));
        }
    }
}

/// A common bank whose every script waits and lives `lifetime` frames.
fn bank(lifetime: u16) -> TestBank {
    static WAIT: &[u8] = &[0x1F, 0xFF];
    let s = TestBank::particle(lifetime, 10.0, WAIT);
    TestBank::new(&[s; 0x80])
}

/// `efManagerImpactShockMakeEffect`: the speed's draw, then the angle's;
/// the damage's scale pivots at 10.
#[test]
fn an_impact_shock_draws_its_speed_then_its_angle() {
    let bank = bank(20);
    let mut p = std::boxed::Box::new(Particles::new());
    let mut e = Effects::new(0);
    rng::set_seed(7);
    let speed = rng::rand_float() * 8.0 + 2.0;
    rng::rand_float();
    let after = rng::seed();
    rng::set_seed(7);
    let pos = Vec3::new(10.0, 20.0, 0.0);
    let pc = e.impact_shock(&mut p, &bank, pos, 16);
    assert_ne!(pc, NIL);
    assert_eq!(rng::seed(), after);
    let xf = p.particle(pc).xf;
    assert!((p.transform(xf).scale.x - 1.9).abs() < 1e-5);
    assert_eq!(e.used(), 1);
    let mut rt = EffectRuntime {
        particles: &mut p,
        effects: &mut e,
        banks: &bank,
    };
    rt.frame();
    let d = rt.particles.transform(xf).translate - pos;
    assert!(((d.x * d.x + d.y * d.y).sqrt() - speed).abs() < 1e-3);
}

/// `efManagerDustCollideMakeEffect`: X then Y scatter, the angle, then the
/// scale.
#[test]
fn a_collide_dust_scatters_x_then_y_then_aims_and_scales() {
    let bank = bank(20);
    let mut p = std::boxed::Box::new(Particles::new());
    let mut e = Effects::new(0);
    rng::set_seed(11);
    let dx = rng::rand_float() * 300.0 - 150.0;
    let dy = rng::rand_float() * 300.0 - 150.0;
    rng::rand_float();
    let scale = rng::rand_float() + 1.0;
    rng::set_seed(11);
    let pc = e.dust_collide(&mut p, &bank, Vec3::ZERO);
    let t = *p.transform(p.particle(pc).xf);
    assert!((t.translate.x - dx).abs() < 1e-4);
    assert!((t.translate.y - dy).abs() < 1e-4);
    assert!((t.scale.x - scale).abs() < 1e-6);
}

/// `efManagerPikachuThunderTrailProcUpdate`: a texture-0 segment draws on
/// each of its first five processes, turns to texture 3 on the sixth and
/// ends on the seventh; a texture-3 one never draws. Each holds a struct.
#[test]
fn a_thunder_trail_effect_draws_until_its_last_frame() {
    let bank = bank(20);
    let mut p = std::boxed::Box::new(Particles::new());
    let mut e = Effects::new(0);
    for (texture, lifetime, draws, procs) in [(0u8, 6u8, 5usize, 7usize), (3, 10, 0, 11)] {
        assert!(e.thunder_trail(Vec3::ZERO, lifetime, texture));
        assert_eq!(e.used(), 1);
        let mut rt = EffectRuntime {
            particles: &mut p,
            effects: &mut e,
            banks: &bank,
        };
        rng::set_seed(3);
        let mut ended = 0;
        for n in 1..=procs {
            rt.frame();
            if rt.effects.displays().next().is_none() {
                ended = n;
                break;
            }
        }
        assert_eq!(ended, procs, "texture {texture}");
        let got = rng::seed();
        rng::set_seed(3);
        for _ in 0..draws {
            rng::rand_int_range(3);
        }
        assert_eq!(got, rng::seed(), "texture {texture}");
        assert_eq!(e.used(), 0);
    }
    assert!(DisplayKind::ThunderTrail.life().is_none());
}

/// A texture's draw is the weapon's own: a sink without the runtime still
/// takes it.
#[test]
fn a_sink_without_a_runtime_still_draws_a_texture() {
    rng::set_seed(5);
    NoEffects.weapon(&WeaponEffect::TextureRand(3));
    let got = rng::seed();
    rng::set_seed(5);
    rng::rand_int_range(3);
    assert_eq!(got, rng::seed());
}

/// The clash queue holds two set-offs for every pair of sixteen weapons
/// of four owners.
#[test]
fn every_clash_of_sixteen_weapons_fits() {
    let mut pool = WeaponPool::default();
    for i in 0..16 {
        pool.spawn(spawn(WeaponKind::MarioFireball, (i % 4) as u8, 0.0, 1.0));
    }
    pool.search_weapons();
    let mut r = Record::default();
    pool.flush_clash_effects(&mut r);
    assert_eq!(r.0.len(), CLASH_FX_MAX);
    assert_eq!(pool.effects_dropped(), 0);
}

/// `efManagerQuakeProcUpdate` runs at priority `3 - magnitude`: of two live
/// quakes the stronger writes `vel_at` last. A write is taken once, and a
/// quake past its animation ejects without writing.
#[test]
fn the_strongest_live_quake_moves_the_camera() {
    let bank = bank(20);
    let mut p = std::boxed::Box::new(Particles::new());
    let mut e = Effects::new(0);
    assert!(e.quake(2));
    assert!(e.quake(1));
    let mut rt = EffectRuntime {
        particles: &mut p,
        effects: &mut e,
        banks: &bank,
    };
    rt.frame();
    // Made with one play; its first process plays again.
    assert_eq!(rt.effects.take_quake(), Some((2, 2)));
    assert_eq!(rt.effects.take_quake(), None);
    let life = DisplayKind::Quake { magnitude: 2 }.life().unwrap();
    let mut last = None;
    for _ in 0..life {
        rt.frame();
        if let Some(q) = rt.effects.take_quake() {
            last = Some(q);
        }
    }
    assert_eq!(last, Some((2, life - 1)));
    rt.frame();
    assert_eq!(rt.effects.take_quake(), None);
}

/// `gmCameraApplyVel` adds the quake's `(z, y)` after the camera's own
/// update, scaled past `EFCOMMON_QUAKE_MAGNITUDE`, once.
#[test]
fn a_quake_shifts_the_camera_at_after_its_update() {
    use crate::camera::{Bounds, Camera, Interest, QUAKE_MAGNITUDE};
    let mut c = Camera::default();
    c.eye = c.at + Vec3::new(0.0, 0.0, QUAKE_MAGNITUDE * 2.0);
    c.quake = Some((10.0, -4.0));
    let mut still = c;
    still.quake = None;
    let bounds = Bounds {
        top: 5000.0,
        bottom: -5000.0,
        left: -5000.0,
        right: 5000.0,
    };
    let interest = [Interest {
        target_pos: Vec3::new(300.0, 0.0, 0.0),
        facing_left: false,
        zoom_frame: 1.0,
        zoom_range: 1.0,
        idle_zoomed_out: false,
        dead_up: false,
        team_bounds: None,
    }];
    c.tick_interests(&interest, bounds, 0.0, 1.0);
    still.tick_interests(&interest, bounds, 0.0, 1.0);
    let d = c.at - still.at;
    assert!(
        (d.x - -8.0).abs() < 1e-3 && (d.y - 20.0).abs() < 1e-3,
        "{d:?}"
    );
    assert_eq!(c.eye, still.eye);
    assert!(c.quake.is_none());
    // Within the magnitude it is unscaled.
    let mut near = Camera::default();
    near.eye = near.at + Vec3::new(0.0, 0.0, 1000.0);
    near.quake = Some((3.0, 5.0));
    assert_eq!(near.vel_at(), Vec3::new(5.0, 3.0, 0.0));
    assert_eq!(near.vel_at(), Vec3::ZERO);
}

/// `WeaponPool::reset` is `Default` in place.
#[test]
fn a_reset_pool_is_a_new_pool() {
    let mut pool = WeaponPool::default();
    pool.spawn(spawn(WeaponKind::FoxBlaster, 0, 0.0, 1.0));
    pool.spawn(spawn(WeaponKind::MarioFireball, 1, 50.0, -1.0));
    pool.team_rules.is_team_battle = true;
    pool.search_weapons();
    pool.reset();
    assert_eq!(pool, WeaponPool::default());
}
