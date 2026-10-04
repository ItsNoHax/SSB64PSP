//! Race to the Finish's bomb barrel (`ittarubomb.c`) and the ground
//! Bumper outside Peach's Castle.

use super::*;
use crate::collision::Segment;
use crate::effect::{HitEffect, HitEffectSink, SmashPiece};
use crate::stage::{StageItem, StageItems};
use crate::weapon::MapSurfaceKind as Kind;
use crate::weapon::SurfaceTopology;
use tarubomb::Status as T;

fn surface(kind: Kind, line: u16, a: (i16, i16), b: (i16, i16)) -> MapSurface {
    MapSurface {
        motion: None,
        kind,
        segment: Segment {
            x1: a.0,
            y1: a.1,
            x2: b.0,
            y2: b.1,
            flags: 0,
        },
        topology: Some(SurfaceTopology {
            line,
            point: 0,
            segments: 1,
            vertex1: line * 2,
            vertex2: line * 2 + 1,
        }),
    }
}

/// A floor at 0 and, with `wall`, a wall at X 2000 that stops rightward
/// motion.
fn course(wall: bool) -> Vec<MapSurface> {
    let mut s = vec![surface(Kind::Floor, 0, (-4000, 0), (4000, 0))];
    if wall {
        s.push(surface(Kind::LeftWall, 1, (2000, -1000), (2000, 3000)));
    }
    s
}

#[derive(Default)]
struct Smashes(Vec<(Vec3, SmashPiece)>);
impl HitEffectSink for Smashes {
    fn container_smash(&mut self, pos: Vec3, piece: SmashPiece) {
        self.0.push((pos, piece));
    }
    fn make(&mut self, _: &HitEffect) {}
}

fn tick(pool: &mut ItemPool, s: &[MapSurface], fx: &mut Smashes) {
    pool.tick_with_effects(|| s.iter().copied(), None, &[], &mut NoItemAnims, fx);
}

fn make_barrel(pool: &mut ItemPool, pos: Vec3) -> u8 {
    let handle = pool.make_item(StageItem::TaruBomb, pos).unwrap();
    pool.slot_of(handle).unwrap()
}

/// Made on its side: the map box is the width either way, no owner, and
/// the attributes' 10-damage, angle-70 attack is live from the start.
#[test]
fn the_barrel_is_made_on_its_side_and_live() {
    let mut pool = ItemPool::default();
    let slot = make_barrel(&mut pool, Vec3::new(0.0, 900.0, 0.0));
    let b = pool.get(slot).unwrap();
    assert_eq!(b.status, ItemStatus::TaruBomb(T::Fall));
    assert_eq!(
        (b.coll.top, b.coll.bottom, b.coll.width),
        (221.0, -221.0, 221.0)
    );
    assert_eq!(b.vars.container_root_pitch, core::f32::consts::FRAC_PI_2);
    // `itManagerMakeItem` places the new attack once.
    assert_eq!(b.attack.state, AttackState::Transfer);
    assert_eq!((b.attack.damage, b.attack.angle), (10, 70));
    assert_eq!(b.attack.size, 145.0);
    assert_eq!(b.damage_coll.hitstatus, HitStatus::Normal);
    assert_eq!(b.weight, ItemWeight::Heavy);
    assert!(!b.is_allow_pickup);
    assert_eq!(b.owner, None);
}

/// It falls at 4 per frame to 90, lands rolling (a landing is always
/// slower than 30 upward), keeps its speed on the flat and smashes into
/// the wall: seven barrel pieces, then a six-frame explosion.
#[test]
fn the_barrel_falls_rolls_and_smashes_into_a_wall() {
    let s = course(true);
    let mut fx = Smashes::default();
    let mut pool = ItemPool::default();
    let slot = make_barrel(&mut pool, Vec3::new(0.0, 900.0, 0.0));
    tick(&mut pool, &s, &mut fx);
    assert_eq!(pool.get(slot).unwrap().vel_air.y, -4.0);
    let mut landed = None;
    for n in 0..60 {
        tick(&mut pool, &s, &mut fx);
        let b = pool.get(slot).unwrap();
        assert!(b.vel_air.y >= -tarubomb::TVEL);
        if b.status == ItemStatus::TaruBomb(T::Roll) {
            landed = Some(n);
            break;
        }
    }
    assert!(landed.is_some());
    let b = pool.get_mut(slot).unwrap();
    assert_eq!(b.vel_air, Vec3::ZERO);
    assert_eq!(b.pos.y, 221.0);
    b.vel_air.x = 40.0;
    tick(&mut pool, &s, &mut fx);
    let b = pool.get(slot).unwrap();
    assert_eq!(b.lr, 1.0);
    // A rightward roll turns the barrel clockwise, 0.0045 per unit of speed.
    assert_eq!(b.vars.taru_roll_step, -tarubomb::ROLL_ROTATE_MUL * 40.0);
    assert!(fx.0.is_empty());
    for _ in 0..60 {
        tick(&mut pool, &s, &mut fx);
        if pool.get(slot).unwrap().status == ItemStatus::TaruBomb(T::Explode) {
            break;
        }
    }
    let b = pool.get(slot).unwrap();
    assert_eq!(b.status, ItemStatus::TaruBomb(T::Explode));
    assert!(b.hidden);
    assert_eq!(b.vel_air, Vec3::ZERO);
    assert_eq!(b.damage_coll.hitstatus, HitStatus::None);
    assert_eq!(b.attack.element, Element::Fire);
    assert_eq!((b.attack.damage, b.attack.size), (16, 350.0));
    assert!(b.attack.can_rehit_item && !b.attack.can_reflect && !b.attack.can_setoff);
    assert_eq!(fx.0, [(b.pos, SmashPiece::TaruBomb)]);
    let mut damage = Vec::new();
    for _ in 0..5 {
        tick(&mut pool, &s, &mut fx);
        damage.push(pool.get(slot).unwrap().attack.damage);
    }
    assert_eq!(damage, [16, 11, 11, 8, 8]);
    tick(&mut pool, &s, &mut fx);
    assert!(pool.get(slot).is_none());
}

/// Rolling down a slope speeds it up by 1.4 times the slope angle a frame.
#[test]
fn the_roll_gathers_speed_downhill() {
    let mut item = tarubomb::make(Vec3::ZERO);
    // A floor falling to the right at 45°.
    let n = Vec2::new(
        core::f32::consts::FRAC_1_SQRT_2,
        core::f32::consts::FRAC_1_SQRT_2,
    );
    item.floor = Some(crate::ground::Standing {
        line: 0,
        flags: 0,
        normal: n,
    });
    item.status = ItemStatus::TaruBomb(T::Roll);
    assert!(tarubomb::update(&mut item, T::Roll));
    let gain = core::f32::consts::FRAC_PI_4 * tarubomb::MUL_VEL_X;
    assert!((item.vel_air.x - gain).abs() < 1e-5);
    assert!(tarubomb::update(&mut item, T::Roll));
    assert!((item.vel_air.x - 2.0 * gain).abs() < 1e-5);
    assert_eq!(item.lr, 1.0);
}

struct Pieces(Vec<SmashPiece>);
impl normal::CommonItems for Pieces {
    fn smash_container(&mut self, _: Vec3, piece: SmashPiece) {
        self.0.push(piece);
    }
    fn eggs_enabled(&self) -> bool {
        false
    }
    fn make_egg(&mut self, _: &Item, _: Vec3, _: Vec3) -> bool {
        false
    }
    fn open_container(&mut self, _: &mut Item) -> bool {
        false
    }
}

/// `itTaruBombCommonProcDamage` smashes it from 10%; hit, shield, set-off
/// and reflection always do, and nothing hops it. The explosion has no
/// callbacks.
#[test]
fn ten_percent_or_any_contact_smashes_the_barrel() {
    let mut fx = crate::wpeffect::Emit::default();
    for (proc, percent, smashed) in [
        (HitProc::Damage, 9, false),
        (HitProc::Damage, 10, true),
        (HitProc::Hit, 0, true),
        (HitProc::Shield, 0, true),
        (HitProc::SetOff, 0, true),
        (HitProc::Reflector, 0, true),
    ] {
        let mut item = tarubomb::make(Vec3::ZERO);
        item.percent_damage = percent;
        let mut common = Pieces(Vec::new());
        assert_eq!(
            tarubomb::hit_proc(&mut item, T::Fall, proc, &mut common, &mut fx),
            Some(true)
        );
        assert_eq!(
            item.status == ItemStatus::TaruBomb(T::Explode),
            smashed,
            "{proc:?} {percent}"
        );
        assert_eq!(common.0.len(), usize::from(smashed));
    }
    let mut item = tarubomb::make(Vec3::ZERO);
    let mut common = Pieces(Vec::new());
    assert_eq!(
        tarubomb::hit_proc(&mut item, T::Roll, HitProc::Hop, &mut common, &mut fx),
        None
    );
    assert_eq!(
        tarubomb::hit_proc(&mut item, T::Explode, HitProc::Hit, &mut common, &mut fx),
        None
    );
}

/// Off an edge the roll falls again, velocity and all.
#[test]
fn a_barrel_rolling_off_an_edge_falls() {
    let s = [surface(Kind::Floor, 0, (-400, 0), (400, 0))];
    let mut fx = Smashes::default();
    let mut pool = ItemPool::default();
    let slot = make_barrel(&mut pool, Vec3::new(0.0, 230.0, 0.0));
    for _ in 0..5 {
        tick(&mut pool, &s, &mut fx);
    }
    assert_eq!(
        pool.get(slot).unwrap().status,
        ItemStatus::TaruBomb(T::Roll)
    );
    pool.get_mut(slot).unwrap().vel_air.x = 60.0;
    for _ in 0..20 {
        tick(&mut pool, &s, &mut fx);
        if pool.get(slot).unwrap().status == ItemStatus::TaruBomb(T::Fall) {
            break;
        }
    }
    let b = pool.get(slot).unwrap();
    assert_eq!(b.status, ItemStatus::TaruBomb(T::Fall));
    assert_eq!(b.vel_air.x, 60.0);
    assert!(fx.0.is_empty());
}

/// Outside Peach's Castle the ground Bumper keeps its attributes'
/// knockback, and a Race to the Finish Bumper animates its own tree.
#[test]
fn a_bonus_bumper_keeps_its_attribute_knockback_and_tree() {
    let mut pool = ItemPool::default();
    let handle = pool
        .make_item(
            StageItem::Bumper {
                castle: false,
                joint: Some(2),
            },
            Vec3::ZERO,
        )
        .unwrap();
    let b = pool.get(pool.slot_of(handle).unwrap()).unwrap();
    assert_eq!(b.attack.kb_weight, gbumper::ATTRIBUTES.kb_weight);
    assert_eq!(b.attack.angle, gbumper::ATTRIBUTES.angle);
    assert_eq!(b.anim_target(), ItemAnimTarget::Bonus3Bumper(2));
}
