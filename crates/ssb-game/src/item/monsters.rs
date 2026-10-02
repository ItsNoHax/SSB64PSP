//! Saffron City's five ground Pokémon (`itglucky.c`, `itmarumine.c`,
//! `ithitokage.c`, `itfushigibana.c`, `itporygon.c`). Their animations
//! write local X/Y each play; the callbacks add the map object's offset.

use super::{
    HitProc, Item, ItemAnims, ItemAttributes, ItemKind, ItemStatus, ItemType, ItemWeight,
    StageItemEvent,
};
use crate::combat::{AttackState, Element, HitStatus};
use crate::ground::BodyColl;
use crate::monster_weapon::MonsterShot;
use crate::wpeffect::{Emit, WeaponEffect as Fx};
use ssb_engine::math::{sin_cos, Vec3};

/// `nITKindGroundMonsterStart` order, also the gate's random IDs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Kind {
    Chansey,
    Electrode,
    Charmander,
    Venusaur,
    Porygon,
}

impl Kind {
    pub fn from_id(id: u8) -> Option<Self> {
        Some(match id {
            0 => Self::Chansey,
            1 => Self::Electrode,
            2 => Self::Charmander,
            3 => Self::Venusaur,
            4 => Self::Porygon,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Common,
    Damaged,
    Explode,
}

/// `ITAttributes` in file 264: 0xBC, 0x104, 0x1FC, 0x278, 0x16C.
const BASE: ItemAttributes = ItemAttributes {
    is_give_hitlag: true,
    weight: ItemWeight::Light,
    attack_offsets: [Vec3::ZERO; 2],
    damage_coll_offset: Vec3::ZERO,
    damage_coll_size: Vec3::new(390.0, 360.0, 360.0),
    map_coll: BodyColl {
        top: 180.0,
        center: 0.0,
        bottom: -180.0,
        width: 195.0,
    },
    size: 300.0,
    angle: 361,
    kb_scale: 100,
    damage: 5,
    element: Element::Normal,
    kb_weight: 0,
    shield_damage: 0,
    attack_count: 1,
    can_setoff: false,
    priority: 1,
    can_rehit_item: false,
    can_rehit_fighter: false,
    can_hop: false,
    can_reflect: false,
    can_shield: false,
    kb_base: 20,
    ty: ItemType::Touch,
    hitstatus: HitStatus::Normal,
    vel_scale: 100,
};
pub static ATTRIBUTES: [ItemAttributes; 5] = [
    BASE,
    ItemAttributes {
        map_coll: BodyColl {
            top: 225.0,
            center: 0.0,
            bottom: -225.0,
            width: 248.0,
        },
        size: 400.0,
        damage: 10,
        element: Element::Fire,
        can_rehit_item: true,
        can_rehit_fighter: true,
        can_shield: true,
        kb_base: 40,
        ty: ItemType::Fighter,
        hitstatus: HitStatus::None,
        ..BASE
    },
    ItemAttributes {
        damage_coll_size: Vec3::new(200.0, 200.0, 200.0),
        map_coll: BodyColl {
            top: 252.0,
            center: 0.0,
            bottom: -252.0,
            width: 273.0,
        },
        size: 340.0,
        can_rehit_item: true,
        can_rehit_fighter: true,
        can_shield: true,
        ty: ItemType::Fighter,
        ..BASE
    },
    ItemAttributes {
        damage_coll_size: Vec3::new(200.0, 200.0, 200.0),
        map_coll: BodyColl {
            top: 360.0,
            center: 0.0,
            bottom: -360.0,
            width: 510.0,
        },
        size: 200.0,
        can_rehit_item: true,
        can_rehit_fighter: true,
        can_shield: true,
        ty: ItemType::Fighter,
        ..BASE
    },
    ItemAttributes {
        map_coll: BodyColl {
            top: 300.0,
            center: 0.0,
            bottom: -300.0,
            width: 360.0,
        },
        size: 200.0,
        kb_scale: 40,
        damage: 16,
        can_rehit_item: true,
        can_rehit_fighter: true,
        can_shield: true,
        kb_base: 80,
        ty: ItemType::Fighter,
        ..BASE
    },
];

/// The shared `dGRYamabukiMonsterAttackKind` anti-repeat rule. Draw only
/// after allocation succeeds, as the two source makers do.
pub(super) fn make(kind: Kind, pos: Vec3, prev: &mut u8) -> Item {
    let mut item = Item::new(
        ItemKind::Monster(kind),
        &ATTRIBUTES[kind as usize],
        ItemStatus::Monster(Status::Common),
        if kind == Kind::Electrode {
            AttackState::Off
        } else {
            AttackState::New
        },
        pos,
        Vec3::ZERO,
        0,
    );
    item.vars.monster_offset = pos;
    item.is_allow_knockback = true;
    item.vars.monster_eggs = 1;
    if matches!(kind, Kind::Charmander | Kind::Venusaur) {
        let mut flags = crate::rng::rand_int_range(4) as u8;
        if flags == *prev || flags & *prev != 0 {
            flags = (flags + 1) % 4;
        }
        *prev = flags;
        item.vars.monster_flags = flags;
        item.texture = u8::from(flags == 2);
    }
    if kind == Kind::Chansey {
        item.attack.interact_mask = super::INTERACT_FIGHTER;
    }
    item.update_attack_positions();
    item
}

fn kind(item: &Item) -> Kind {
    let ItemKind::Monster(k) = item.kind else {
        unreachable!()
    };
    k
}

/// The two `ITMonsterEvent`s: full-size radii (unlike ITAttributes).
fn monster_event(item: &mut Item, k: Kind) {
    let timer = if item.event_id == 0 { 0 } else { 8 };
    if item.multi == timer {
        let first = item.event_id == 0;
        let a = &mut item.attack;
        a.size = 300.0;
        a.element = Element::Normal;
        a.can_setoff = false;
        a.shield_damage = 0;
        (a.angle, a.damage, a.kb_scale, a.kb_weight, a.kb_base) = match (k, first) {
            (Kind::Porygon, true) => (40, 18, 40, 0, 70),
            (Kind::Porygon, false) => (40, 8, 70, 0, 40),
            (Kind::Venusaur, true) => (40, 20, 100, 90, 0),
            (Kind::Venusaur, false) => (361, 8, 70, 0, 30),
            _ => unreachable!(),
        };
        item.event_id = 1;
    }
    item.multi = item.multi.wrapping_add(1);
}

fn explode_event(item: &mut Item) {
    let (timer, damage, size) = [
        (0, 30, 700.0),
        (2, 30, 350.0),
        (4, 20, 300.0),
        (6, 10, 200.0),
    ][item.event_id as usize];
    if item.multi == timer {
        item.attack.angle = 361;
        item.attack.damage = damage;
        item.attack.size = size;
        item.attack.can_reflect = false;
        item.attack.can_shield = false;
        item.attack.element = Element::Fire;
        item.attack.can_setoff = false;
        item.event_id = (item.event_id + 1).min(3);
    }
}

pub(super) fn proc_update(
    item: &mut Item,
    status: Status,
    anims: &mut dyn ItemAnims,
    event: &mut dyn FnMut(StageItemEvent),
    shots: &mut dyn FnMut(MonsterShot),
    fx: &mut Emit,
) -> bool {
    let k = kind(item);
    if status == Status::Damaged {
        item.apply_gravity_clamp_tvel(1.2, 100.0);
        item.rotate_z -= 0.104_719_76 * item.lr;
        return true;
    }
    item.pos.x += item.vars.monster_offset.x;
    item.pos.y += item.vars.monster_offset.y;
    if status == Status::Explode {
        explode_event(item);
        item.multi += 1;
        if item.multi == 6 {
            event(StageItemEvent::MonsterClose);
            return false;
        }
        return true;
    }
    if matches!(k, Kind::Venusaur | Kind::Porygon) {
        monster_event(item, k);
        if item.multi == if k == Kind::Venusaur { 128 } else { 32 } {
            fx.push(Fx::DustLight {
                pos: Vec3::new(item.pos.x, 0.0, item.pos.z),
                lr: -1,
            });
        }
    }
    let frame = anims.root_frame(item.anim_target());
    if matches!(k, Kind::Charmander | Kind::Venusaur) {
        let flags = item.vars.monster_flags;
        let shooting = flags == 2 || (flags & 1 != 0 && (40.0..=120.0).contains(&frame));
        item.texture = u8::from(shooting);
        if shooting {
            if item.vars.monster_spawn_wait == 0 {
                let off = if k == Kind::Charmander {
                    -250.0
                } else {
                    -540.0
                };
                let pos = item.pos + Vec3::new(off, 0.0, 0.0);
                shots(MonsterShot::new(k == Kind::Venusaur, pos));
                item.vars.monster_spawn_wait = if k == Kind::Charmander { 8 } else { 16 };
                if k == Kind::Venusaur {
                    fx.push(Fx::DustCollide(pos));
                }
            } else if k == Kind::Charmander {
                item.vars.monster_spawn_wait -= 1;
            }
            // Venusaur decrements even on the spawn frame; Charmander's
            // else branch yields a nine-frame interval.
            if k == Kind::Venusaur && item.vars.monster_spawn_wait > 0 {
                item.vars.monster_spawn_wait -= 1;
            }
        }
    }
    if k == Kind::Chansey && (80.0..=85.0).contains(&frame) {
        if item.multi == 0 && item.vars.monster_eggs != 0 {
            let mut made = !anims.eggs_enabled();
            if !made {
                let pos = item.pos + Vec3::new(-200.0, 200.0, 0.0);
                let vel = Vec3::new(
                    -(crate::rng::rand_float() * 8.0 + 8.0),
                    crate::rng::rand_float() * 8.0 + 30.0,
                    0.0,
                );
                made = anims.make_egg(pos, vel);
                if made {
                    fx.push(Fx::DustLight { pos, lr: -1 });
                }
            }
            if made {
                item.multi = 10;
                item.vars.monster_eggs -= 1;
            }
        }
        if item.vars.monster_eggs != 0 && item.multi > 0 {
            item.multi -= 1;
        }
    }
    if anims.root_idle(item.anim_target()) {
        if k == Kind::Electrode {
            item.refresh_attack_coll();
            item.clear_owner_stats();
            item.vars.monster_offset = Vec3::ZERO;
            item.damage_coll.hitstatus = HitStatus::None;
            item.hidden = true;
            item.refresh_attack_coll();
            item.multi = 0;
            item.attack.throw_mul = 1.0;
            item.event_id = 0;
            explode_event(item);
            item.set_status(ItemStatus::Monster(Status::Explode));
            fx.push(Fx::ScaledExplosion {
                pos: item.pos,
                scale: 1.4,
            });
            fx.push(Fx::Quake(1));
        } else {
            event(StageItemEvent::MonsterClose);
            return false;
        }
    }
    true
}

pub(super) fn hit_proc(
    item: &mut Item,
    status: Status,
    proc: HitProc,
    anims: &mut dyn ItemAnims,
    event: &mut dyn FnMut(StageItemEvent),
) -> Option<bool> {
    let k = kind(item);
    if status != Status::Common {
        return None;
    }
    if k == Kind::Chansey && proc == HitProc::Hit {
        item.attack.state = AttackState::Off;
        return Some(true);
    }
    if matches!(k, Kind::Chansey | Kind::Charmander) && proc == HitProc::Damage {
        if item.damage_knockback >= 100.0 {
            let angle = crate::attack::sakurai_angle_radians(
                item.damage_angle,
                item.ga == super::Ga::Air,
                item.damage_knockback,
            );
            let (sin, cos) = sin_cos(angle);
            item.vel_air = Vec3::new(
                cos * item.damage_knockback * -item.damage_lr,
                sin * item.damage_knockback,
                0.0,
            );
            item.attack.state = AttackState::Off;
            item.damage_coll.hitstatus = HitStatus::None;
            anims.stop_root(item.anim_target());
            event(StageItemEvent::MonsterClear);
            item.set_status(ItemStatus::Monster(Status::Damaged));
        }
        return Some(true);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::item::{ItemAnimTarget, ItemPool, RootWrite};
    use crate::stage::{StageItem, StageItems};

    #[derive(Default)]
    struct Clock {
        frame: f32,
        idle: bool,
        enabled: bool,
        egg_success: bool,
        attempts: u8,
        stopped: bool,
    }
    impl ItemAnims for Clock {
        fn play(&mut self, _: ItemAnimTarget) -> RootWrite {
            RootWrite {
                translate: [Some(-100.0), Some(0.0), None],
            }
        }
        fn root_frame(&self, _: ItemAnimTarget) -> f32 {
            self.frame
        }
        fn root_idle(&self, _: ItemAnimTarget) -> bool {
            self.idle
        }
        fn stop_root(&mut self, _: ItemAnimTarget) {
            self.stopped = true;
        }
        fn eggs_enabled(&self) -> bool {
            self.enabled
        }
        fn make_egg(&mut self, pos: Vec3, vel: Vec3) -> bool {
            self.attempts += 1;
            assert_eq!(pos, Vec3::new(-200.0, 200.0, 0.0));
            assert!((-16.0..=-8.0).contains(&vel.x) && (30.0..38.0).contains(&vel.y));
            self.egg_success
        }
    }

    fn update(item: &mut Item, clock: &mut Clock) -> (Vec<StageItemEvent>, Vec<MonsterShot>) {
        let ItemStatus::Monster(status) = item.status else {
            panic!()
        };
        let (mut events, mut shots) = (Vec::new(), Vec::new());
        proc_update(
            item,
            status,
            clock,
            &mut |e| events.push(e),
            &mut |s| shots.push(s),
            &mut Emit::default(),
        );
        (events, shots)
    }

    #[test]
    fn gate_ids_allocate_all_five_and_failure_does_not_draw_attack_rng() {
        let mut pool = ItemPool::default();
        for id in 0..5 {
            let h = pool.make_item(StageItem::Monster(id), Vec3::ZERO).unwrap();
            assert_eq!(
                pool.get((h & 0xFF) as u8).unwrap().kind,
                ItemKind::Monster(Kind::from_id(id).unwrap())
            );
        }
        for _ in 5..16 {
            pool.make_item(StageItem::Bumper, Vec3::ZERO).unwrap();
        }
        let seed = crate::rng::seed();
        assert!(pool.make_item(StageItem::Monster(2), Vec3::ZERO).is_none());
        assert_eq!(crate::rng::seed(), seed);
    }

    #[test]
    fn flame_and_razor_preserve_their_different_spawn_frame_decrements() {
        for (k, interval) in [(Kind::Charmander, 9), (Kind::Venusaur, 16)] {
            let mut item = make(k, Vec3::ZERO, &mut 4);
            item.vars.monster_flags = 1;
            let mut clock = Clock {
                frame: 39.0,
                ..Clock::default()
            };
            assert!(update(&mut item, &mut clock).1.is_empty());
            let mut frames = Vec::new();
            for frame in 40..=121 {
                clock.frame = frame as f32;
                if !update(&mut item, &mut clock).1.is_empty() {
                    frames.push(frame);
                }
            }
            assert_eq!(frames[0], 40);
            assert!(frames.windows(2).all(|f| f[1] - f[0] == interval));
            assert!(frames.last().unwrap() <= &120);
            assert_eq!(item.texture, 0);
            item.vars.monster_flags = 2;
            item.vars.monster_spawn_wait = 0;
            assert_eq!(update(&mut item, &mut clock).1.len(), 1);
            assert_eq!(item.texture, 1); // instant is outside the window test
        }
    }

    #[test]
    fn porygon_and_venusaur_replace_the_entire_authored_hit_event() {
        for (k, first, second) in [
            (Kind::Porygon, (18, 40, 0, 70), (8, 70, 0, 40)),
            (Kind::Venusaur, (20, 100, 90, 0), (8, 70, 0, 30)),
        ] {
            let mut item = make(k, Vec3::ZERO, &mut 4);
            let mut clock = Clock::default();
            update(&mut item, &mut clock);
            let fields = |a: &Item| {
                (
                    a.attack.damage,
                    a.attack.kb_scale,
                    a.attack.kb_weight,
                    a.attack.kb_base,
                )
            };
            assert_eq!(fields(&item), first);
            assert_eq!(item.attack.size, 300.0);
            for _ in 1..=8 {
                update(&mut item, &mut clock);
            }
            assert_eq!(fields(&item), second);
            assert_eq!(item.event_id, 1);
        }
    }

    #[test]
    fn electrode_explosion_stays_hidden_and_closes_after_six_updates() {
        let mut pool = ItemPool::default();
        pool.make_item(StageItem::Monster(1), Vec3::ZERO).unwrap();
        let mut clock = Clock {
            idle: true,
            ..Clock::default()
        };
        let tick = |pool: &mut ItemPool, c: &mut Clock| {
            pool.tick(core::iter::empty::<crate::weapon::MapSurface>, None, &[], c)
        };
        tick(&mut pool, &mut clock);
        assert_eq!(pool.get(0).unwrap().attack.size, 700.0);
        for (i, size) in [700.0, 700.0, 350.0, 350.0, 300.0].into_iter().enumerate() {
            tick(&mut pool, &mut clock);
            let item = pool.get(0).unwrap();
            assert!(item.hidden && !item.attack.can_shield && !item.attack.can_reflect);
            assert_eq!((item.multi, item.attack.size), (i as u16 + 1, size));
        }
        tick(&mut pool, &mut clock);
        assert_eq!(pool.active_count(), 0);
        assert_eq!(
            pool.take_stage_events().collect::<Vec<_>>(),
            [StageItemEvent::MonsterClose]
        );
    }

    #[test]
    fn strong_hits_clear_the_gate_and_stop_only_the_root_animation() {
        for k in [Kind::Chansey, Kind::Charmander] {
            let mut item = make(k, Vec3::ZERO, &mut 4);
            let mut clock = Clock::default();
            let mut events = Vec::new();
            item.damage_knockback = 99.99;
            hit_proc(
                &mut item,
                Status::Common,
                HitProc::Damage,
                &mut clock,
                &mut |e| events.push(e),
            );
            assert_eq!(item.status, ItemStatus::Monster(Status::Common));
            item.damage_knockback = 100.0;
            item.damage_angle = 90;
            item.damage_lr = 1.0;
            hit_proc(
                &mut item,
                Status::Common,
                HitProc::Damage,
                &mut clock,
                &mut |e| events.push(e),
            );
            assert_eq!(events, [StageItemEvent::MonsterClear]);
            assert!(clock.stopped);
            assert_eq!(item.status, ItemStatus::Monster(Status::Damaged));
            assert_eq!(item.attack.state, AttackState::Off);
            assert_eq!(item.damage_coll.hitstatus, HitStatus::None);
            assert!((item.vel_air.y - 100.0).abs() < 0.001);
            update(&mut item, &mut clock);
            assert!((item.vel_air.y - 98.8).abs() < 0.001);
        }
    }

    #[test]
    fn chansey_disabled_egg_takes_no_rng_and_enabled_egg_retries_allocation() {
        let mut item = make(Kind::Chansey, Vec3::ZERO, &mut 4);
        let mut clock = Clock {
            frame: 80.0,
            ..Clock::default()
        };
        let seed = crate::rng::seed();
        update(&mut item, &mut clock);
        assert_eq!(
            (item.vars.monster_eggs, clock.attempts, crate::rng::seed()),
            (0, 0, seed)
        );
        item.vars.monster_eggs = 1;
        item.multi = 0;
        clock.enabled = true;
        update(&mut item, &mut clock);
        assert_eq!((item.vars.monster_eggs, clock.attempts), (1, 1));
        clock.egg_success = true;
        update(&mut item, &mut clock);
        assert_eq!((item.vars.monster_eggs, clock.attempts), (0, 2));
        assert_eq!(item.multi, 10);
        hit_proc(
            &mut item,
            Status::Common,
            HitProc::Hit,
            &mut clock,
            &mut |_| {},
        );
        assert_eq!(item.attack.state, AttackState::Off);
    }
}
