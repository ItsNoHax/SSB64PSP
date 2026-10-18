use super::*;
use crate::particle::tests::TestBank;
use crate::particle::{self as lb, Particles};

/// A common bank whose every script waits `wait` frames and lives
/// `lifetime`.
fn bank(lifetime: u16) -> TestBank {
    static WAIT: &[u8] = &[0x1F, 0xFF];
    let s = TestBank::particle(lifetime, 10.0, WAIT);
    TestBank::new(&[s; 0x70])
}

fn runtime() -> (std::boxed::Box<Particles>, Effects) {
    (std::boxed::Box::new(Particles::new()), Effects::new(0))
}

#[test]
fn poke_ball_rays_have_no_effect_struct_and_follow_only_their_own_sequence() {
    let bank = bank(200);
    let (mut p, mut e) = runtime();
    let seq = e.mball_rays(Vec3::new(10.0, 20.0, 0.0)).unwrap();
    assert_eq!(e.used(), 0, "EFDesc lacks EFFECT_FLAG_USERDATA");
    let pos = Vec3::new(40.0, 50.0, 0.0);
    e.move_display(seq, pos);
    assert_eq!(e.displays().next().unwrap().translate, pos);
    let mut rt = EffectRuntime {
        particles: &mut p,
        effects: &mut e,
        banks: &bank,
    };
    for _ in 1..life::MBALL_RAYS - 1 {
        rt.frame();
        assert!(rt.effects.displays().next().is_some());
    }
    rt.frame();
    assert!(rt.effects.displays().next().is_none());
    let next = rt.effects.mball_rays(Vec3::ZERO).unwrap();
    assert_ne!(next, seq);
    rt.effects.move_display(seq, pos);
    assert_eq!(rt.effects.displays().next().unwrap().translate, Vec3::ZERO);
}

/// The one live particle's script, on list `link`.
fn scripts_on(p: &Particles, link: usize) -> std::vec::Vec<u8> {
    p.list(link).map(|(i, _)| i).collect()
}

#[test]
fn a_light_spark_is_placed_scaled_by_damage_and_drifts() {
    let bank = bank(20);
    let (mut p, mut e) = runtime();
    rng::set_seed(1);
    let pos = Vec3::new(100.0, 200.0, 0.0);
    let pc = e.damage_normal_light(&mut p, &bank, pos, 1, 20, false);
    assert_ne!(pc, NIL);
    let xf = p.particle(pc).xf;
    let t = *p.transform(xf);
    assert_eq!(t.translate, pos);
    // `((size - 10) * 0.13) + 1`.
    assert!((t.scale.x - 2.3).abs() < 1e-6);
    assert_eq!(e.used(), 1);
    // One `efManagerDefaultProcUpdate`: 12 to 50 units in some direction.
    let mut rt = EffectRuntime {
        particles: &mut p,
        effects: &mut e,
        banks: &bank,
    };
    rt.frame();
    let moved = rt.particles.transform(xf).translate - pos;
    let d = (moved.x * moved.x + moved.y * moved.y).sqrt();
    assert!((12.0..50.0).contains(&d), "{d}");
    assert_eq!(moved.z, 0.0);
}

#[test]
fn damage_scales_follow_each_makers_pivot() {
    assert_eq!(damage_scale(10, 10, -0.05, 0.13), 1.0);
    assert!((damage_scale(4, 10, -0.05, 0.15) - 0.7).abs() < 1e-6);
    // The electric maker pivots at 5 with -0.08.
    assert!((damage_scale(2, 5, -0.08, 0.15) - 0.76).abs() < 1e-6);
    assert!((damage_scale(15, 5, -0.08, 0.15) - 2.5).abs() < 1e-6);
}

#[test]
fn a_maker_needs_five_structs_free() {
    let bank = bank(20);
    let (mut p, mut e) = runtime();
    // `sEFManagerStructsFreeNum < 5` refuses, so four stay free.
    for i in 0..EFFECT_ALLOC_NUM - 4 {
        assert_ne!(e.damage_fire(&mut p, &bank, Vec3::ZERO, 10), NIL, "{i}");
    }
    assert_eq!(e.damage_fire(&mut p, &bank, Vec3::ZERO, 10), NIL);
    assert_eq!(e.free_num, 4);
}

#[test]
fn a_struct_is_freed_when_its_particles_end() {
    let bank = bank(3);
    let (mut p, mut e) = runtime();
    e.damage_electric(&mut p, &bank, Vec3::ZERO, 5);
    assert_eq!(e.used(), 1);
    let mut rt = EffectRuntime {
        particles: &mut p,
        effects: &mut e,
        banks: &bank,
    };
    for _ in 0..3 {
        rt.frame();
    }
    assert_eq!(rt.particles.used_num, 0);
    assert_eq!(rt.effects.used(), 0);
    assert_eq!(rt.particles.xf_used_num, 0);
}

#[test]
fn a_heavy_spark_ends_in_a_light_one_with_the_players_colours() {
    let bank = bank(3);
    let (mut p, mut e) = runtime();
    let pos = Vec3::new(-50.0, 30.0, 0.0);
    let pc = e.damage_normal_heavy(&mut p, &bank, pos, 2, 12);
    assert_eq!(p.particle(pc).primcolor, [0xFF; 4]);
    assert_eq!(&p.particle(pc).envcolor[..3], &NORMAL_HEAVY_ENV[2]);
    // No drift: the heavy spark has no process.
    let mut rt = EffectRuntime {
        particles: &mut p,
        effects: &mut e,
        banks: &bank,
    };
    rt.frame();
    let xf = rt.particles.particle(pc).xf;
    assert_eq!(rt.particles.transform(xf).translate, pos);
    rt.frame();
    rt.frame();
    // `efManagerDamageNormalHeavyProcDead`: a light spark at the heavy
    // one's point, from its struct's player and size.
    let live = scripts_on(rt.particles, 0);
    assert_eq!(live.len(), 1);
    let light = rt.particles.particle(live[0]);
    let t = rt.particles.transform(light.xf);
    assert!((t.scale.x - damage_scale(12, 10, -0.05, 0.13)).abs() < 1e-6);
    assert_eq!(rt.effects.used(), 1);
}

#[test]
fn a_coins_end_makes_a_dust_cloud_200_above() {
    let bank = bank(2);
    let (mut p, mut e) = runtime();
    e.damage_coin(&mut p, &bank, Vec3::new(0.0, 100.0, 0.0));
    let mut rt = EffectRuntime {
        particles: &mut p,
        effects: &mut e,
        banks: &bank,
    };
    rt.frame();
    rt.frame();
    // The dust is on list 1 (`LBPARTICLE_MASK_GENLINK(0)`). Made in the
    // particles' `func_run`, its priority-3 move runs the same frame.
    let dust = scripts_on(rt.particles, 1);
    assert_eq!(dust.len(), 1);
    let xf = rt.particles.particle(dust[0]).xf;
    assert_eq!(
        rt.particles.transform(xf).translate,
        Vec3::new(0.0, 305.0, 0.0)
    );
}

#[test]
fn the_blasts_streaks_take_the_players_script_and_the_blasts_turn() {
    let bank = bank(20);
    let (mut p, mut e) = runtime();
    // Type 1 (the right blast), player 2: `dEFManagerDeadExplodeGenID[6]`.
    let pc = e.dead_explode(&mut p, &bank, Vec3::new(5.0, 6.0, 0.0), 2, 1);
    assert_ne!(pc, NIL);
    assert_eq!(scripts_on(&p, 2), [pc]);
    let t = p.transform(p.particle(pc).xf);
    assert_eq!(t.status, lb::TransformStatus::Ready);
    assert!((t.rotate.z - core::f32::consts::FRAC_PI_2).abs() < 1e-6);
    // No struct is taken.
    assert_eq!(e.used(), 0);
    assert_eq!(DEAD_EXPLODE_IDS[6], 0x3D);
    // The star sparkle, scaled on list 2.
    let s = e.sparkle_white_dead(&mut p, &bank, Vec3::ZERO, 5.0);
    assert_eq!(p.transform(p.particle(s).xf).scale, Vec3::splat(5.0));
}

// ---------------------------------------------------------------------------
// The hit pipeline's choice of effect (`ftMainProcessHitCollisionStatsMain`)
// ---------------------------------------------------------------------------

use crate::combat::{self, AttackColl, AttackState, Element, HitStatus};
use crate::fighter::{Fighter, FighterKind, Situation};
use crate::team::TeamRules;
use ssb_engine::math::Vec2;

#[derive(Default)]
struct Record {
    made: std::vec::Vec<HitEffect>,
}

impl HitEffectSink for Record {
    fn make(&mut self, e: &HitEffect) {
        self.made.push(*e);
    }
}

fn standing(kind: FighterKind, port: u8, x: f32) -> Fighter {
    let mut f = Fighter::new(kind, port, 3);
    crate::status::set_wait(&mut f);
    f.situation = Situation::Ground;
    f.floor = Some(crate::ground::Standing {
        line: 0,
        flags: 0,
        normal: Vec2::new(0.0, 1.0),
    });
    f.pos.x = x;
    f
}

fn swing(f: &mut Fighter, damage: i32, element: Element, kb_base: i32, fgm_level: u8) {
    f.attack_colls[0] = AttackColl {
        state: AttackState::New,
        damage,
        element,
        size: 150.0,
        angle: 361,
        kb_scale: 100,
        kb_base,
        fgm_level,
        is_hit_air: true,
        is_hit_ground: true,
        offset: Vec3::new(0.0, 150.0, 0.0),
        ..Default::default()
    };
}

/// One frame: Mario on port 2 swings at Fox on port 1.
fn hit(damage: i32, element: Element, kb_base: i32, fgm_level: u8) -> (Record, Fighter) {
    let mut attacker = standing(FighterKind::Mario, 2, 0.0);
    let mut victim = standing(FighterKind::Fox, 1, 50.0);
    swing(&mut attacker, damage, element, kb_base, fgm_level);
    let mut rec = Record::default();
    combat::search_all(&mut [&mut attacker, &mut victim], TeamRules::FREE_FOR_ALL);
    combat::finish_frame_with(&mut [&mut attacker, &mut victim], &mut rec);
    (rec, victim)
}

#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn a_weak_normal_hit_makes_a_light_spark_at_the_impact_point() {
    let (rec, victim) = hit(8, Element::Normal, 0, 0);
    assert!(victim.damage > 0);
    assert_eq!(rec.made.len(), 1);
    let e = rec.made[0];
    assert_eq!(e.kind, HitEffectKind::NormalLight);
    assert_eq!((e.player, e.damage), (2, 8));
    // Halfway between the new attack's point and the (unposed) victim's
    // stand-in box at its origin.
    let attack = Vec3::new(0.0, 150.0, 0.0);
    assert_eq!(e.pos, (attack + Vec3::new(50.0, 0.0, 0.0)) * 0.5);
}

#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn knockback_of_180_or_more_makes_a_heavy_spark() {
    let (rec, _) = hit(8, Element::Normal, 200, 0);
    assert_eq!(rec.made[0].kind, HitEffectKind::NormalHeavy);
}

#[test]
fn the_element_picks_the_spark() {
    assert_eq!(
        hit(8, Element::Fire, 0, 0).0.made[0].kind,
        HitEffectKind::Fire
    );
    assert_eq!(
        hit(8, Element::Electric, 0, 0).0.made[0].kind,
        HitEffectKind::Electric
    );
    assert_eq!(
        hit(8, Element::Coin, 0, 0).0.made[0].kind,
        HitEffectKind::Coin
    );
    assert!(matches!(
        hit(8, Element::Slash, 0, 0).0.made[0].kind,
        HitEffectKind::Slash { .. }
    ));
    // Freezing and sleep fall to the normal spark.
    assert_eq!(
        hit(8, Element::Freezing, 0, 0).0.made[0].kind,
        HitEffectKind::NormalLight
    );
}

#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn a_louder_normal_hit_adds_orbs_and_sparks_on_the_victims_side() {
    let (rec, victim) = hit(8, Element::Normal, 0, 1);
    let kinds: std::vec::Vec<_> = rec.made.iter().map(|e| e.kind).collect();
    // `this_fp->lr` as the hit is processed: the victim still faces right.
    // Its damage status, in `ftMainProcParams` after the effects, turns it
    // to the attacker.
    assert_eq!(
        kinds,
        [
            HitEffectKind::NormalLight,
            HitEffectKind::SpawnOrbs,
            HitEffectKind::SpawnSparks { lr: 1.0 }
        ]
    );
    assert_eq!(victim.facing.sign(), -1.0);
}

#[test]
fn an_invincible_victim_takes_a_set_off_and_no_damage() {
    let mut attacker = standing(FighterKind::Mario, 0, 0.0);
    let mut victim = standing(FighterKind::Fox, 1, 50.0);
    victim.damage_colls.hitstatus[0] = HitStatus::Invincible;
    swing(&mut attacker, 12, Element::Normal, 0, 0);
    let mut rec = Record::default();
    combat::search_all(&mut [&mut attacker, &mut victim], TeamRules::FREE_FOR_ALL);
    combat::finish_frame_with(&mut [&mut attacker, &mut victim], &mut rec);
    assert_eq!(victim.damage, 0);
    assert_eq!(rec.made.len(), 1);
    assert_eq!(
        (rec.made[0].kind, rec.made[0].damage),
        (HitEffectKind::SetOff, 12)
    );
}

#[test]
fn a_clank_sets_off_both_attacks() {
    let mut a = standing(FighterKind::Mario, 0, 0.0);
    let mut b = standing(FighterKind::Fox, 1, 50.0);
    swing(&mut a, 10, Element::Normal, 0, 0);
    swing(&mut b, 12, Element::Normal, 0, 0);
    let mut rec = Record::default();
    combat::search_all(&mut [&mut a, &mut b], TeamRules::FREE_FOR_ALL);
    combat::finish_frame_with(&mut [&mut a, &mut b], &mut rec);
    let set_offs: std::vec::Vec<i32> = rec
        .made
        .iter()
        .filter(|e| e.kind == HitEffectKind::SetOff)
        .map(|e| e.damage)
        .collect();
    // Within 10 of each other: both stop, `this` (port 0) first.
    assert_eq!(set_offs, [10, 12]);
    assert_eq!(rec.made.len(), 2);
}

#[test]
fn a_hit_spark_draws_the_common_banks_script_for_its_player() {
    let bank = bank(20);
    let (mut p, mut e) = runtime();
    let (rec, _) = hit(8, Element::Normal, 0, 0);
    let mut rt = EffectRuntime {
        particles: &mut p,
        effects: &mut e,
        banks: &bank,
    };
    rt.make(&rec.made[0]);
    assert_eq!(rt.particles.used_num, 1);
    assert_eq!(rt.effects.used(), 1);
    // The display effects make nothing yet.
    rt.make(&HitEffect {
        kind: HitEffectKind::SpawnOrbs,
        ..rec.made[0]
    });
    assert_eq!(rt.particles.used_num, 1);
}

/// RE-422: each drawn display effect's `EFDesc::dl_link`, which places it
/// before (15, 10) or after (18) stage layer 3.
#[test]
fn display_effects_draw_on_their_desc_links() {
    use DisplayKind as K;
    assert_eq!(K::Slash.dl_link(), 18);
    assert_eq!(K::ShockSmall.dl_link(), 18);
    assert_eq!(K::ImpactWave.dl_link(), 10);
    assert_eq!(K::MBallRays.dl_link(), 10);
    for k in [
        K::FlyOrbs,
        K::FlySparks,
        K::FlyMDust,
        K::StarRodSpark,
        K::FireSpark,
        K::ThunderTrail,
    ] {
        assert_eq!(k.dl_link(), 15, "{k:?}");
    }
    for k in [
        K::SpawnOrbs,
        K::SpawnSparks,
        K::SpawnMDust,
        K::Quake { magnitude: 0 },
    ] {
        assert_eq!(k.dl_link(), 0, "{k:?}");
    }
}

/// `efManagerPikachuThunderShockMakeEffect`'s spark: attached to its
/// fighter, held while `ftParamPauseEffect` pauses it, ended after its
/// seven plays or at once by `ftParamStopEffect`, which leaves other
/// fighters' sparks alone.
#[test]
fn a_thunder_shock_spark_pauses_with_its_fighter_and_stops_on_its_status_change() {
    let bank = bank(200);
    let (mut p, mut e) = runtime();
    let pos = Vec3::new(30.0, 120.0, 0.0);
    assert!(e.thunder_shock(0, pos, 1));
    assert!(e.thunder_shock(1, pos, 2));
    let mut rt = EffectRuntime {
        particles: &mut p,
        effects: &mut e,
        banks: &bank,
    };
    let ticks = |e: &Effects, owner: u8| e.displays().find(|d| d.owner == owner).map(|d| d.ticks);
    rt.frame();
    rt.frame();
    let before = ticks(rt.effects, 0).unwrap();
    rt.effects.pause_attached(0, true);
    for _ in 0..10 {
        rt.frame();
    }
    assert_eq!(ticks(rt.effects, 0), Some(before), "a paused spark holds");
    assert!(
        ticks(rt.effects, 1).is_none(),
        "the other fighter's spark ran its 7 plays"
    );
    rt.effects.pause_attached(0, false);
    rt.frame();
    assert_eq!(ticks(rt.effects, 0), Some(before + 1));
    assert!(rt.effects.thunder_shock(1, pos, 0));
    rt.effects.stop_attached(0);
    assert!(ticks(rt.effects, 0).is_none());
    assert!(ticks(rt.effects, 1).is_some());
    assert_eq!(life::THUNDER_SHOCK, [7, 7, 7]);
}

/// RE-513 trace k1, puffs 7 (`lr` 1) and 12 (`lr` -1), both from row 2
/// (`rotate` 19, `vel` 180, `add` -30): the root DObj's translate on the
/// make frame and after each `efManagerKirbyVulcanJabProcUpdate`, bit for
/// bit. The sixth update adds a zero velocity; the seventh ejects. The puff
/// is not attached: a paused or stopped fighter leaves it alone.
#[test]
fn a_vulcan_jab_puff_moves_as_the_trace_records() {
    let bits = |v: f32| v.to_bits();
    let cases = [
        (
            1i8,
            [0x44fbe846u32, 0x435a6667, 0x41da6667],
            [
                (0x44fbe846u32, 0x435a6667u32),
                (0x4506d065, 0x43859927),
                (0x450de700, 0x43991dea),
                (0x451337f4, 0x43a7c17c),
                (0x4516c342, 0x43b183dd),
                (0x451888e9, 0x43b6650e),
                (0x451888e9, 0x43b6650e),
            ],
            (0u32, 0x3ea9c91fu32),
        ),
        (
            -1,
            [0x44be7b78, 0x435a6667, 0xc1da6667],
            [
                (0x44be7b78, 0x435a6667),
                (0x44acc2f4, 0x43857d4b),
                (0x449e95be, 0x4398ebc4),
                (0x4493f3d6, 0x43a77e9f),
                (0x448cdd3b, 0x43b135dc),
                (0x448951ee, 0x43b6117a),
                (0x448951ee, 0x43b6117a),
            ],
            (0x40490fdb, 0xbea9c91f),
        ),
    ];
    for (lr, pos, want, (rot_y, rot_z)) in cases {
        let bank = bank(200);
        let (mut p, mut e) = runtime();
        let pos = Vec3::new(
            f32::from_bits(pos[0]),
            f32::from_bits(pos[1]),
            f32::from_bits(pos[2]),
        );
        assert!(e.vulcan_jab(pos, lr, 19.0, 180.0, -30.0));
        let mut rt = EffectRuntime {
            particles: &mut p,
            effects: &mut e,
            banks: &bank,
        };
        let puff = |e: &Effects| {
            e.displays()
                .find(|d| d.kind == DisplayKind::VulcanJab)
                .map(|d| {
                    (
                        bits(d.translate.x),
                        bits(d.translate.y),
                        d.translate.z,
                        d.rotate,
                    )
                })
        };
        let (_, _, z, rotate) = puff(rt.effects).unwrap();
        assert_eq!(z, pos.z);
        assert_eq!((bits(rotate.y), bits(rotate.z)), (rot_y, rot_z), "lr {lr}");
        let mut got = std::vec::Vec::new();
        for frame in 0..7 {
            let (x, y, _, _) = puff(rt.effects).unwrap();
            got.push((x, y));
            if frame == 2 {
                // Neither the fighter's hitlag nor its status change
                // touches an unattached puff.
                rt.effects.pause_attached(0, true);
                rt.effects.stop_attached(0);
            }
            rt.frame();
        }
        assert_eq!(got, want, "lr {lr}");
        assert!(puff(rt.effects).is_none(), "the seventh update ejects");
    }
}

/// A DL link draws its effects in `GObj` (make) order, which a reused
/// pool slot does not follow: the third puff takes the first one's slot
/// but still draws after the second (RE-513).
#[test]
fn display_effects_draw_in_make_order_not_pool_order() {
    let bank = bank(200);
    let (mut p, mut e) = runtime();
    let at = |x: f32| Vec3::new(x, 0.0, 0.0);
    assert!(e.vulcan_jab(at(1.0), 1, 0.0, 0.0, 0.0));
    let mut rt = EffectRuntime {
        particles: &mut p,
        effects: &mut e,
        banks: &bank,
    };
    for _ in 0..3 {
        rt.frame();
    }
    assert!(rt.effects.vulcan_jab(at(2.0), 1, 0.0, 0.0, 0.0));
    // The first puff's seventh update ejects it.
    for _ in 0..5 {
        rt.frame();
    }
    assert!(rt.effects.vulcan_jab(at(3.0), 1, 0.0, 0.0, 0.0));
    let xs = |it: &mut dyn Iterator<Item = &Display>| {
        it.map(|d| d.translate.x).collect::<std::vec::Vec<_>>()
    };
    assert_eq!(xs(&mut rt.effects.displays()), [3.0, 2.0], "pool order");
    assert_eq!(xs(&mut rt.effects.displays_in_make_order()), [2.0, 3.0]);
}
