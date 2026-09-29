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
