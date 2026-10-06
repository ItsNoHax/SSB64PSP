use super::*;
use crate::dead::{self, StageBounds};
use crate::fighter::{FighterKind, Situation};
use crate::status::{self, AnyStatus, BlastZone, Status};
use ssb_engine::math::Vec2;

fn fighter(port: u8) -> Fighter {
    let mut f = Fighter::new(FighterKind::Mario, port, 3);
    f.dead.bounds = Some(StageBounds {
        map: BlastZone {
            top: 3000.0,
            bottom: -2000.0,
            left: -4000.0,
            right: 4000.0,
        },
        camera: BlastZone {
            top: 2000.0,
            bottom: -1000.0,
            left: -3000.0,
            right: 3000.0,
        },
        rebirth: Vec2::new(100.0, 1200.0),
        fog_color: [0x6E, 0xD2, 0xFF],
    });
    f
}

#[test]
fn a_side_ko_sets_off_one_explosion_and_the_flash() {
    let mut ko = KoEffects::default();
    let mut f = fighter(2);
    f.pos = Vec3::new(4001.0, 2500.0, 0.0);
    assert!(dead::check(&mut f));
    ko.observe(&mut f);
    let e = ko.explosions[2].unwrap();
    assert_eq!(e.pos, Vec3::new(4001.0, 2000.0, 0.0));
    assert_eq!((e.kind, e.player, e.ticks), (ExplodeKind::Right, 2, 0));
    assert_eq!(e.kind.rotate_z_degrees(), 90.0);
    // Taken once.
    assert_eq!(f.dead.explode, None);
    assert!(!f.dead.flash);
    ko.tick();
    assert_eq!(ko.explosions[2].unwrap().ticks, 1);
    assert_eq!(ko.flash_color(), Some([0xFF, 0xFF, 0xFF, 18]));
    for _ in 1..EXPLODE_TICKS - 1 {
        ko.tick();
    }
    assert_eq!(ko.explosions[2].unwrap().ticks, EXPLODE_TICKS - 1);
    ko.tick();
    assert_eq!(ko.explosions[2], None);
    // The flash outlasts it by one frame, then clears.
    assert_eq!(ko.flash_color().map(|c| c[3]), Some(18));
    ko.tick();
    assert_eq!(ko.flash_color(), None);
}

#[test]
fn a_down_ko_points_down_and_a_left_ko_left() {
    assert_eq!(ExplodeKind::Down.rotate_z_degrees(), 0.0);
    assert_eq!(ExplodeKind::Left.rotate_z_degrees(), 270.0);
    let mut ko = KoEffects::default();
    let mut f = fighter(0);
    f.pos.y = -2001.0;
    dead::check(&mut f);
    ko.observe(&mut f);
    assert_eq!(ko.explosions[0].map(|e| e.kind), Some(ExplodeKind::Down));
}

#[test]
fn a_falling_top_out_flashes_when_it_lands_but_makes_no_explosion() {
    let mut ko = KoEffects::default();
    let mut f = fighter(1);
    f.dead.camera_eye = Vec3::new(0.0, 400.0, 6000.0);
    f.pos.y = 3001.0;
    dead::set_dead_up_fall(&mut f);
    for _ in 0..=dead::DEADUP_WAIT {
        ko.observe(&mut f);
        assert!(ko.flash_color().is_none());
        status::update(&mut f);
        dead::tick_status(&mut f);
    }
    assert!(f.is_invisible);
    ko.observe(&mut f);
    ko.tick();
    assert!(ko.flash_color().is_some());
    assert_eq!(ko.explosions, [None; 4]);
}

#[test]
fn a_star_ko_fades_towards_the_fog_to_half_strength_then_sparkles() {
    let mut f = fighter(0);
    f.pos.y = 3001.0;
    dead::set_dead_up_star(&mut f);
    assert_eq!(f.colanim.color(), None);
    status::update(&mut f);
    // Set on the first update, clear.
    assert_eq!(f.colanim.color(), Some([0x6E, 0xD2, 0xFF, 0]));
    let mut alphas = Vec::new();
    for _ in 0..dead::DEADUP_WAIT {
        status::update(&mut f);
        dead::tick_status(&mut f);
        alphas.push(f.colanim.color().map(|c| c[3]));
    }
    // `128 - wait * 128 / 180` for wait 180 down to 2; at wait 1 the
    // fighter vanishes and the colour clears.
    assert_eq!(alphas[0], Some(0));
    assert_eq!(alphas[90], Some(64));
    assert_eq!(alphas[178], Some(127));
    assert_eq!(alphas[179], None);
    assert!(f.is_invisible);
    // The sparkle is recorded at TopN and made on the particle runtime.
    assert_eq!(f.dead.sparkle, Some(f.pos));
    let (bank, mut p, mut e) = particles();
    let mut rt = crate::effect::EffectRuntime {
        particles: &mut p,
        effects: &mut e,
        banks: &bank,
    };
    KoEffects::default().observe_with(&mut f, &mut rt);
    assert_eq!(f.dead.sparkle, None);
    let live: Vec<_> = p.list(2).collect();
    assert_eq!(live.len(), 1);
    let t = p.transform(live[0].1.xf);
    assert_eq!((t.translate, t.scale), (f.pos, Vec3::splat(5.0)));
}

/// A runtime over a bank whose scripts each wait and last 20 frames.
fn particles() -> (
    crate::particle::tests::TestBank,
    Box<crate::particle::Particles>,
    crate::effect::Effects,
) {
    static WAIT: &[u8] = &[0x1F, 0xFF];
    let s = crate::particle::tests::TestBank::particle(20, 10.0, WAIT);
    (
        crate::particle::tests::TestBank::new(&[s; 0x70]),
        Box::new(crate::particle::Particles::new()),
        crate::effect::Effects::new(0),
    )
}

#[test]
fn a_blast_makes_its_players_streaks_turned_like_the_blast() {
    let mut ko = KoEffects::default();
    let mut f = fighter(1);
    f.pos = Vec3::new(-4001.0, 0.0, 0.0);
    assert!(dead::check(&mut f));
    let (bank, mut p, mut e) = particles();
    let mut rt = crate::effect::EffectRuntime {
        particles: &mut p,
        effects: &mut e,
        banks: &bank,
    };
    ko.observe_with(&mut f, &mut rt);
    assert_eq!(ko.explosions[1].unwrap().kind, ExplodeKind::Left);
    // The KO's `efManagerQuakeMakeEffect(2)` (RE-420).
    assert!(!f.dead.quake);
    let quakes: Vec<_> = rt
        .effects
        .displays()
        .filter_map(|d| match d.kind {
            crate::effect::DisplayKind::Quake { magnitude } => Some(magnitude),
            _ => None,
        })
        .collect();
    assert_eq!(quakes, [2]);
    let live: Vec<_> = p.list(2).collect();
    assert_eq!(live.len(), 1);
    let t = p.transform(live[0].1.xf);
    assert_eq!(t.translate, ko.explosions[1].unwrap().pos);
    assert!((t.rotate.z - 270f32.to_radians()).abs() < 1e-5);
}

#[test]
fn the_halo_and_the_glow_last_until_the_fighter_falls() {
    let mut f = fighter(0);
    f.pos.y = -2001.0;
    dead::check(&mut f);
    dead::rebirth_down(&mut f, 0);
    assert_eq!(dead::halo_scale(&f), Some(1.0));
    assert_eq!(f.colanim.id, crate::colanim::ColAnimId::FIGHTER_REBIRTH);
    let mut saw_stand = false;
    let mut saw_wait = false;
    loop {
        status::update(&mut f);
        if dead::halo_scale(&f).is_none() {
            break;
        }
        saw_stand |= f.status.status == AnyStatus::Common(Status::RebirthStand);
        saw_wait |= f.status.status == AnyStatus::Common(Status::RebirthWait);
        // RebirthStand and RebirthWait keep the glow.
        assert_eq!(f.colanim.id, crate::colanim::ColAnimId::FIGHTER_REBIRTH);
        assert!(f.colanim.light.is_some() && f.colanim.color().is_some());
    }
    assert!(saw_stand && saw_wait);
    assert!(matches!(
        f.status.status,
        AnyStatus::Common(Status::Fall | Status::FallAerial)
    ));
    assert_eq!(f.situation, Situation::Air);
    // `ftParamSetTimedHitStatusInvincible`'s flicker takes over.
    assert_eq!(f.colanim.id, crate::colanim::ColAnimId::FIGHTER_NO_DAMAGE);
    assert_eq!(f.invincible_frames, 120);
}

#[test]
fn the_halo_size_is_each_fighters_attribute() {
    assert_eq!(dead::halo_size(FighterKind::Mario), 1.0);
    assert_eq!(dead::halo_size(FighterKind::Donkey), 1.7);
    assert_eq!(dead::halo_size(FighterKind::GiantDonkey), 1.7);
    assert_eq!(dead::halo_size(FighterKind::Luigi), 1.02);
    assert_eq!(dead::halo_size(FighterKind::Kirby), 1.14);
    assert_eq!(dead::halo_size(FighterKind::PolyKirby), 1.14);
    assert_eq!(dead::halo_size(FighterKind::Purin), 1.2);
}

/// RE-420: `ftCommonDeadInitStatusVars` and `DeadUpFall`'s landing request
/// `efManagerQuakeMakeEffect(2)`; `DeadUpStar` requests none.
#[test]
fn a_ko_makes_a_magnitude_two_quake_except_a_star_ko() {
    for pos in [
        Vec3::new(4001.0, 0.0, 0.0),
        Vec3::new(-4001.0, 0.0, 0.0),
        Vec3::new(0.0, -2001.0, 0.0),
    ] {
        let mut f = fighter(0);
        f.pos = pos;
        assert!(dead::check(&mut f));
        assert!(f.dead.quake, "{pos:?}");
        // The host takes it with the frame's other effects.
        KoEffects::default().observe(&mut f);
        assert!(!f.dead.quake);
    }

    let mut f = fighter(1);
    f.dead.camera_eye = Vec3::new(0.0, 400.0, 6000.0);
    f.pos.y = 3001.0;
    dead::set_dead_up_fall(&mut f);
    assert!(!f.dead.quake);
    let mut made = 0;
    for _ in 0..=dead::DEADUP_WAIT + 1 {
        status::update(&mut f);
        dead::tick_status(&mut f);
        made += usize::from(core::mem::take(&mut f.dead.quake));
    }
    assert_eq!(made, 1);

    let mut f = fighter(2);
    f.pos.y = 3001.0;
    dead::set_dead_up_star(&mut f);
    for _ in 0..=dead::DEADUP_WAIT + 1 {
        status::update(&mut f);
        dead::tick_status(&mut f);
        assert!(!f.dead.quake);
    }
    assert!(f.is_invisible);
}

#[test]
fn screen_flash_off_sets_no_flash_but_still_explodes() {
    let mut ko = KoEffects {
        flash_disabled: true,
        ..KoEffects::default()
    };
    let mut f = fighter(1);
    f.pos = Vec3::new(4001.0, 2500.0, 0.0);
    assert!(dead::check(&mut f));
    ko.observe(&mut f);
    assert!(ko.explosions[1].is_some());
    assert!(!f.dead.flash);
    assert_eq!(ko.flash_color(), None);
}
