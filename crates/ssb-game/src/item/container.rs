//! Normal containers: `itbox.c`, `ittaru.c`, `itegg.c`, `itcapsule.c`.
use super::{
    map, normal::CommonItems, HitProc, Item, ItemAttributes, ItemKind, ItemStatus, ItemType,
    ItemWeight,
};
use crate::{
    combat::{AttackState, Element, HitStatus},
    ground::BodyColl,
    weapon::MapSurface,
    wpeffect::{Emit, WeaponEffect as Fx},
};
use ssb_engine::math::{Vec2, Vec3};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Kind {
    Crate = 0,
    Barrel = 1,
    Capsule = 2,
    Egg = 3,
}
impl Kind {
    pub fn spin_speed(self) -> f32 {
        match self {
            Self::Crate => 40.0 * 0.01,
            Self::Barrel => 0.0,
            Self::Capsule => 120.0 * 0.01,
            Self::Egg => 1.0,
        }
    }
    pub fn heavy(self) -> bool {
        matches!(self, Self::Crate | Self::Barrel)
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Init,
    Wait,
    Fall,
    Hold,
    Thrown,
    Dropped,
    Explode,
    Roll,
}

/// File 251's `ITAttributes` at 0x50 and 0xACC (US).
pub static CAPSULE_ATTRIBUTES: ItemAttributes = ItemAttributes {
    is_give_hitlag: true,
    is_display_colanim: false,
    weight: ItemWeight::Light,
    attack_offsets: [Vec3::ZERO; 2],
    damage_coll_offset: Vec3::ZERO,
    damage_coll_size: Vec3::new(150.0, 150.0, 150.0),
    map_coll: BodyColl {
        top: 120.0,
        center: 0.0,
        bottom: -100.0,
        width: 60.0,
    },
    size: 200.0,
    angle: 361,
    kb_scale: 80,
    damage: 5,
    element: Element::Normal,
    kb_weight: 0,
    shield_damage: 0,
    attack_count: 1,
    can_setoff: true,
    priority: 1,
    can_rehit_item: false,
    can_rehit_fighter: false,
    can_hop: true,
    can_reflect: true,
    can_shield: true,
    kb_base: 20,
    ty: ItemType::Throw,
    hitstatus: HitStatus::Normal,
    vel_scale: 80,
};
pub static EGG_ATTRIBUTES: ItemAttributes = ItemAttributes {
    map_coll: BodyColl {
        top: 200.0,
        center: 0.0,
        bottom: -140.0,
        width: 280.0,
    },
    damage: 3,
    kb_base: 60,
    vel_scale: 100,
    ..CAPSULE_ATTRIBUTES
};
/// File 251, `llITCommonDataBoxItemAttributes` (0x5CC).
pub static CRATE_ATTRIBUTES: ItemAttributes = ItemAttributes {
    weight: ItemWeight::Heavy,
    damage_coll_size: Vec3::new(450.0, 450.0, 450.0),
    map_coll: BodyColl {
        top: 225.0,
        center: 0.0,
        bottom: -225.0,
        width: 225.0,
    },
    damage: 15,
    kb_scale: 100,
    kb_base: 40,
    can_hop: false,
    ty: ItemType::Damage,
    vel_scale: 100,
    ..CAPSULE_ATTRIBUTES
};
/// File 251, `llITCommonDataTaruItemAttributes` (0x634).
pub static BARREL_ATTRIBUTES: ItemAttributes = ItemAttributes {
    damage_coll_size: Vec3::new(294.0, 316.0, 294.0),
    map_coll: BodyColl {
        top: 236.0,
        center: 0.0,
        bottom: -236.0,
        width: 221.0,
    },
    size: 290.0,
    damage: 12,
    ..CRATE_ATTRIBUTES
};
pub const HEAVY_EVENTS: [(u16, i32, i32, f32); 4] = [
    (0, 361, 20, 350.0),
    (4, 361, 15, 250.0),
    (6, 361, 10, 150.0),
    (8, 361, 1, 0.0),
];
/// The Egg intentionally initializes from the Capsule table, then uses its
/// own table on updates, as the source does. These are full radii.
pub const CAPSULE_EVENTS: [(u16, i32, i32, f32); 4] = [
    (0, 361, 30, 350.0),
    (2, 361, 30, 250.0),
    (4, 361, 20, 150.0),
    (6, 361, 1, 0.0),
];
pub const EGG_EVENTS: [(u16, i32, i32, f32); 4] = [
    (0, 361, 30, 350.0),
    (4, 361, 30, 250.0),
    (6, 361, 20, 150.0),
    (8, 361, 1, 0.0),
];

pub(super) fn make(kind: Kind, pos: Vec3, vel: Vec3) -> Item {
    let attr = match kind {
        Kind::Crate => &CRATE_ATTRIBUTES,
        Kind::Barrel => &BARREL_ATTRIBUTES,
        Kind::Capsule => &CAPSULE_ATTRIBUTES,
        Kind::Egg => &EGG_ATTRIBUTES,
    };
    let mut item = Item::new(
        ItemKind::Container(kind),
        attr,
        ItemStatus::Container(Status::Init),
        AttackState::Off,
        pos,
        vel,
        0,
    );
    // `itManagerMakeItemSetupCommon`: slow appear spin is unsigned.
    item.spin_step = kind.spin_speed() * core::f32::consts::PI / 18.0;
    item.is_damage_all = kind.heavy();
    if kind == Kind::Crate {
        item.vars.container_root_yaw = core::f32::consts::FRAC_PI_2;
    }
    item.update_attack_positions();
    item
}
fn kind(item: &Item) -> Kind {
    let ItemKind::Container(k) = item.kind else {
        unreachable!()
    };
    k
}
fn set(item: &mut Item, s: Status) {
    item.set_status(ItemStatus::Container(s));
}
pub(super) fn hold(item: &mut Item) {
    // Under the new attach joint, `gcSetDObjTransformsForGObj` gives the
    // root descriptor 1's transform: zero rotation, unit scale.
    item.rotate_z = 0.0;
    item.scale = Vec3::new(1.0, 1.0, 1.0);
    item.vars.container_root_yaw = 0.0;
    item.vars.container_root_pitch = 0.0;
    set(item, Status::Hold);
}
pub(super) fn thrown(item: &mut Item) {
    if !kind(item).heavy() {
        item.is_damage_all = true;
        item.damage_coll.hitstatus = HitStatus::Normal;
    }
    release_pose(item);
    set(item, Status::Thrown);
}
pub(super) fn dropped(item: &mut Item) {
    if kind(item) == Kind::Egg {
        item.is_damage_all = true;
        item.damage_coll.hitstatus = HitStatus::Normal;
    }
    release_pose(item);
    set(item, Status::Dropped);
}
fn wait(item: &mut Item) {
    if kind(item) == Kind::Crate {
        let n = item.floor.map_or(Vec2::new(0.0, 1.0), |f| f.normal);
        item.rotate_z = ssb_engine::math::atan2(n.y, n.x) - core::f32::consts::FRAC_PI_2;
    }
    item.attack.state = AttackState::Off;
    item.vel_air = Vec3::ZERO;
    item.is_allow_pickup = true;
    item.times_landed = 0;
    item.owner = None;
    item.player = None;
    item.team = crate::team::TEAM_DEFAULT;
    item.handicap = crate::stale::HANDICAP_DEFAULT;
    item.attack.throw_mul = 1.0;
    map::set_ground(item);
    if kind(item) == Kind::Egg {
        item.scale = Vec3::new(1.0, 1.0, 1.0);
    }
    set(item, Status::Wait);
}
fn fall(item: &mut Item) {
    item.is_allow_pickup = false;
    if !kind(item).heavy() {
        item.is_damage_all = true;
        item.damage_coll.hitstatus = HitStatus::Normal;
    }
    if kind(item) == Kind::Egg {
        item.attack.state = AttackState::Off;
    }
    map::set_air(item);
    set(item, Status::Fall);
}
/// `it{Box,Taru}{Thrown,Dropped}SetStatus` run before the release, so
/// their `DObjGetStruct(item_gobj)->child` is the item root under the
/// attach joint.
fn release_pose(item: &mut Item) {
    match kind(item) {
        Kind::Crate => item.vars.container_root_yaw = core::f32::consts::FRAC_PI_2,
        Kind::Barrel => {
            item.vars.container_root_pitch = core::f32::consts::FRAC_PI_2;
            item.coll.top = item.coll.width;
            item.coll.bottom = -item.coll.width;
        }
        _ => {}
    }
}
fn attack_event(item: &mut Item, events: &[(u16, i32, i32, f32); 4]) {
    let (timer, angle, damage, size) = events[item.event_id as usize];
    if item.multi == timer {
        item.attack.angle = angle;
        item.attack.damage = damage;
        item.attack.size = size;
        item.event_id = (item.event_id + 1).min(3);
    }
}
fn explode(item: &mut Item, fx: &mut Emit) {
    item.attack.state = AttackState::Off;
    item.vel_air = Vec3::ZERO;
    fx.push(Fx::ScaledExplosion {
        pos: item.pos,
        scale: 1.4,
    });
    fx.push(Fx::Quake(1));
    item.hidden = true;
    item.multi = 0;
    item.event_id = 0;
    item.attack.throw_mul = 1.0;
    item.attack.can_rehit_item = true;
    if kind(item) != Kind::Barrel {
        item.attack.can_hop = false;
    }
    item.attack.can_reflect = false;
    item.attack.can_setoff = false;
    item.attack.element = Element::Fire;
    item.damage_coll.hitstatus = HitStatus::None;
    item.clear_owner_stats();
    item.refresh_attack_coll();
    attack_event(
        item,
        if kind(item).heavy() {
            &HEAVY_EVENTS
        } else {
            &CAPSULE_EVENTS
        },
    );
    set(item, Status::Explode);
}
fn open(item: &mut Item, common: &mut dyn CommonItems, fx: &mut Emit) -> bool {
    if kind(item).heavy() {
        common.smash_container(item.pos);
    }
    let opened = if kind(item) == Kind::Crate {
        common.open_crate(item)
    } else {
        common.open_container(item)
    };
    if opened {
        if kind(item) == Kind::Egg {
            fx.push(Fx::EggBreak(item.pos));
        }
        false
    } else {
        explode(item, fx);
        true
    }
}
pub(super) fn update(item: &mut Item, status: Status, fx: &mut Emit) -> bool {
    match status {
        Status::Init | Status::Fall | Status::Thrown | Status::Dropped => {
            match kind(item) {
                Kind::Crate => item.apply_gravity_clamp_tvel(4.0, 120.0),
                Kind::Barrel => {
                    item.apply_gravity_clamp_tvel(4.0, 90.0);
                    item.rotate_z += item.vars.taru_roll_step;
                }
                _ => item.apply_gravity_clamp_tvel(1.2, 100.0),
            }
            item.rotate_z += item.spin_step;
        }
        Status::Explode => {
            item.multi += 1;
            let egg = kind(item) == Kind::Egg;
            if item.multi == if kind(item) == Kind::Capsule { 6 } else { 8 } {
                if egg {
                    fx.push(Fx::EggBreak(item.pos));
                }
                return false;
            }
            attack_event(
                item,
                if kind(item).heavy() {
                    &HEAVY_EVENTS
                } else if egg {
                    &EGG_EVENTS
                } else {
                    &CAPSULE_EVENTS
                },
            );
        }
        Status::Roll => {
            let n = item.floor.map_or(Vec2::new(0.0, 1.0), |f| f.normal);
            item.vel_air.x +=
                -(ssb_engine::math::atan2(n.y, n.x) - core::f32::consts::FRAC_PI_2) * 1.4;
            item.lr = if item.vel_air.x >= 0.0 { 1.0 } else { -1.0 };
            let speed = Vec2::new(item.vel_air.x, item.vel_air.y).length();
            if speed < 0.1 {
                item.lifetime -= 1;
                if item.lifetime < 60 {
                    if item.lifetime == 0 {
                        return false;
                    }
                    if item.lifetime % 2 != 0 {
                        item.hidden = !item.hidden;
                    }
                }
            }
            item.vars.taru_roll_step = if item.lr == -1.0 { 0.0045 } else { -0.0045 } * speed;
            item.rotate_z += item.vars.taru_roll_step;
        }
        Status::Wait | Status::Hold => {}
    }
    true
}
pub(super) fn proc_map<I, F>(
    item: &mut Item,
    status: Status,
    surfaces: &F,
    common: &mut dyn CommonItems,
    fx: &mut Emit,
) -> bool
where
    F: Fn() -> I,
    I: IntoIterator<Item = MapSurface>,
{
    match status {
        Status::Wait => {
            if !map::check_lr_wall_proc_no_floor(item, surfaces) {
                fall(item);
            }
        }
        Status::Init | Status::Fall | Status::Dropped
            if status != Status::Dropped || kind(item) != Kind::Barrel =>
        {
            let (common_rebound, ground) = match kind(item) {
                Kind::Crate => (0.2, 0.5),
                Kind::Barrel => (0.5, 0.2),
                Kind::Egg => (0.2, 0.5),
                Kind::Capsule => (0.2, 0.4),
            };
            let result = map::check_destroy_dropped(item, common_rebound, ground, surfaces);
            if result.destroy {
                return false;
            }
            if result.goto_wait {
                wait(item);
            }
        }
        Status::Thrown | Status::Dropped if kind(item) == Kind::Barrel => {
            let floor = map::test_all_collision_flag(item, crate::map::MASK_FLOOR, surfaces);
            if map::check_collide_all_rebound(
                item,
                crate::map::MASK_CEIL | crate::map::MASK_LWALL | crate::map::MASK_RWALL,
                0.5,
            ) {
                item.set_spin_vel_lr();
            }
            if floor {
                // Signed comparisons in `itTaruThrownProcMap`, including
                // unconditional destruction after the >=90 branch.
                if item.vel_air.y >= 90.0 {
                    open(item, common, fx);
                    return false;
                } else if item.vel_air.y < 30.0 {
                    item.lifetime = 360;
                    item.vel_air.y = 0.0;
                    set(item, Status::Roll);
                } else {
                    let n = item.floor.unwrap().normal;
                    map::reflect(&mut item.vel_air, n);
                    item.vel_air.y *= 0.2;
                    item.set_spin_vel_lr();
                }
                item.clear_owner_stats();
            }
        }
        Status::Roll => {
            if !map::test_lr_wall_check_floor(item, surfaces) {
                // This callback changes only the proc table, not kinetics.
                set(item, Status::Dropped);
            } else if item.mask_curr & (crate::map::MASK_LWALL | crate::map::MASK_RWALL) != 0 {
                return open(item, common, fx);
            }
        }
        Status::Thrown => {
            if map::test_all_collision_flag(item, map::MASK_MAIN, surfaces) {
                return open(item, common, fx);
            }
        }
        Status::Hold | Status::Explode => {}
        Status::Init | Status::Fall | Status::Dropped => unreachable!(),
    }
    true
}
pub(super) fn hit(
    item: &mut Item,
    status: Status,
    proc: HitProc,
    common: &mut dyn CommonItems,
    fx: &mut Emit,
) -> Option<bool> {
    if kind(item).heavy() {
        return match (status, proc) {
            (Status::Wait | Status::Thrown | Status::Dropped | Status::Roll, HitProc::Damage)
                if item.percent_damage >= if kind(item) == Kind::Crate { 15 } else { 10 } =>
            {
                Some(open(item, common, fx))
            }
            (
                Status::Thrown | Status::Dropped | Status::Roll,
                HitProc::Hit | HitProc::Shield | HitProc::SetOff | HitProc::Reflector,
            ) => Some(open(item, common, fx)),
            _ => None,
        };
    }
    match (status, proc) {
        (Status::Init | Status::Wait | Status::Fall, HitProc::Damage)
        | (
            Status::Thrown | Status::Dropped,
            HitProc::Damage | HitProc::Hit | HitProc::Shield | HitProc::SetOff | HitProc::Reflector,
        ) => Some(open(item, common, fx)),
        (Status::Thrown | Status::Dropped, HitProc::Hop) => {
            item.common_proc_hop();
            Some(true)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fighter::{Fighter, FighterKind};
    use crate::item::{ItemAnimTarget, ItemAnims, ItemPool, ItemRequest, NoItemAnims};
    use crate::stage::{StageItem, StageItems};
    use crate::weapon::{WeaponKind, WeaponPool, WeaponSpawn};

    struct Drops(bool);
    impl CommonItems for Drops {
        fn eggs_enabled(&self) -> bool {
            false
        }
        fn make_egg(&mut self, _: &Item, _: Vec3, _: Vec3) -> bool {
            false
        }
        fn open_container(&mut self, _: &mut Item) -> bool {
            self.0
        }
    }
    struct Clock;
    impl ItemAnims for Clock {
        fn root_frame(&self, _: ItemAnimTarget) -> f32 {
            80.0
        }
        fn root_idle(&self, _: ItemAnimTarget) -> bool {
            false
        }
    }
    fn floor() -> MapSurface {
        crate::map::floor_surface((
            0,
            crate::collision::Segment {
                x1: -3000,
                y1: 0,
                x2: 3000,
                y2: 0,
                flags: 0,
            },
        ))
    }

    #[test]
    fn egg_and_capsule_explosions_preserve_events_owner_and_end_frame() {
        for (kind, end) in [(Kind::Egg, 8), (Kind::Capsule, 6)] {
            let mut item = make(kind, Vec3::ZERO, Vec3::new(12.0, 34.0, 0.0));
            item.owner = Some(0);
            item.attack.throw_mul = 1.5;
            let mut fx = Emit::default();
            assert_eq!(
                hit(
                    &mut item,
                    Status::Init,
                    HitProc::Damage,
                    &mut Drops(false),
                    &mut fx
                ),
                Some(true)
            );
            assert_eq!(
                (item.status, item.owner, item.vel_air),
                (ItemStatus::Container(Status::Explode), None, Vec3::ZERO)
            );
            assert_eq!(
                (item.attack.damage, item.attack.size, item.event_id),
                (30, 350.0, 1)
            );
            assert_eq!(
                (
                    item.attack.element,
                    item.attack.throw_mul,
                    item.damage_coll.hitstatus
                ),
                (Element::Fire, 1.0, HitStatus::None)
            );
            assert!(!item.attack.can_setoff && !item.attack.can_reflect && !item.attack.can_hop);
            for tick in 1..end {
                assert!(update(&mut item, Status::Explode, &mut Emit::default()));
                let radius = if kind == Kind::Egg {
                    if tick < 4 {
                        350.0
                    } else if tick < 6 {
                        250.0
                    } else {
                        150.0
                    }
                } else if tick < 2 {
                    350.0
                } else if tick < 4 {
                    250.0
                } else {
                    150.0
                };
                assert_eq!(item.attack.size, radius, "{kind:?} tick {tick}");
            }
            let mut final_fx = Emit::default();
            assert!(!update(&mut item, Status::Explode, &mut final_fx));
            assert_eq!(
                final_fx.iter().any(|f| matches!(f, Fx::EggBreak(_))),
                kind == Kind::Egg
            );
        }
    }
    #[test]
    fn successful_drop_breaks_even_when_the_selected_child_cannot_allocate() {
        for kind in [Kind::Egg, Kind::Capsule] {
            for proc in [
                HitProc::Hit,
                HitProc::Shield,
                HitProc::SetOff,
                HitProc::Reflector,
                HitProc::Damage,
            ] {
                let mut item = make(kind, Vec3::ZERO, Vec3::ZERO);
                let mut fx = Emit::default();
                assert_eq!(
                    hit(&mut item, Status::Thrown, proc, &mut Drops(true), &mut fx),
                    Some(false)
                );
                assert_eq!(
                    fx.iter().any(|f| matches!(f, Fx::EggBreak(_))),
                    kind == Kind::Egg
                );
            }
        }
    }
    #[test]
    fn dropped_container_honours_forced_despawn_and_thrown_container_explodes_on_floor() {
        for kind in [Kind::Egg, Kind::Capsule] {
            let mut item = make(
                kind,
                Vec3::new(0.0, 300.0, 0.0),
                Vec3::new(0.0, -200.0, 0.0),
            );
            item.pos_prev = item.pos;
            item.pos.y = 50.0;
            item.times_thrown = 4;
            assert!(!proc_map(
                &mut item,
                Status::Dropped,
                &|| [floor()],
                &mut Drops(false),
                &mut Emit::default()
            ));
            let mut item = make(
                kind,
                Vec3::new(0.0, 300.0, 0.0),
                Vec3::new(0.0, -200.0, 0.0),
            );
            item.pos_prev = item.pos;
            item.pos.y = 50.0;
            assert!(proc_map(
                &mut item,
                Status::Thrown,
                &|| [floor()],
                &mut Drops(false),
                &mut Emit::default()
            ));
            assert_eq!(item.status, ItemStatus::Container(Status::Explode));
        }
    }
    #[test]
    fn normal_landing_resets_the_thrower_and_can_be_picked_up_again() {
        let mut pool = ItemPool::default();
        let slot = pool
            .spawn_container(Kind::Egg, Vec3::new(0.0, 140.0, 0.0), Vec3::ZERO)
            .unwrap();
        let item = pool.get_mut(slot).unwrap();
        item.times_landed = 1;
        item.owner = Some(0);
        item.attack.throw_mul = 1.5;
        pool.tick(|| [floor()], None, &[], &mut NoItemAnims);
        let item = pool.get(slot).unwrap();
        assert_eq!(item.status, ItemStatus::Container(Status::Wait));
        assert!(item.is_allow_pickup);
        assert_eq!(
            (item.owner, item.attack.throw_mul, item.ga),
            (None, 1.0, super::super::Ga::Ground)
        );
        let mut f = Fighter::new(FighterKind::Mario, 0, 3);
        f.items.request(ItemRequest::Hold { slot });
        pool.take_requests(&mut f, || [floor()]);
        assert_eq!(
            pool.get(slot).unwrap().status,
            ItemStatus::Container(Status::Hold)
        );
        f.items.request(ItemRequest::Throw {
            vel: Vec3::new(100.0, 0.0, 0.0),
            throw_mul: 1.0,
            is_smash: true,
        });
        pool.take_requests(&mut f, || [floor()]);
        let item = pool.get(slot).unwrap();
        assert_eq!(item.status, ItemStatus::Container(Status::Thrown));
        assert!(item.is_thrown && item.is_damage_all);
        assert_eq!(item.vel_air.x, 100.0);
        assert!(item.spin_step < 0.0);
        assert!(f.items.held.is_none());
    }
    #[test]
    fn capsule_throw_scales_velocity_and_drop_keeps_its_source_flags() {
        let mut pool = ItemPool::default();
        let slot = pool
            .spawn_container(Kind::Capsule, Vec3::ZERO, Vec3::ZERO)
            .unwrap();
        wait(pool.get_mut(slot).unwrap());
        let mut f = Fighter::new(FighterKind::Mario, 0, 3);
        f.items.request(ItemRequest::Hold { slot });
        pool.take_requests(&mut f, core::iter::empty);
        f.items.request(ItemRequest::Drop {
            vel: Vec3::new(100.0, 0.0, 0.0),
            throw_mul: 0.5,
        });
        pool.take_requests(&mut f, core::iter::empty);
        let item = pool.get(slot).unwrap();
        assert_eq!(item.status, ItemStatus::Container(Status::Dropped));
        assert!(!item.is_damage_all);
        assert!((item.vel_air.x - 80.0).abs() < 0.0001);
        assert!(item.is_thrown);
    }
    #[test]
    fn chansey_allocates_in_link_order_and_retries_a_full_pool_without_extra_rng() {
        let mut pool = ItemPool::default();
        let h = pool.make_item(StageItem::Monster(0), Vec3::ZERO).unwrap();
        let slot = (h & 255) as u8;
        for _ in 1..super::super::ITEM_ALLOC_MAX {
            pool.spawn_container(Kind::Capsule, Vec3::ZERO, Vec3::ZERO)
                .unwrap();
        }
        pool.tick(core::iter::empty, None, &[], &mut Clock);
        assert_eq!(pool.get(slot).unwrap().vars.monster_eggs, 1);
        pool.destroy(1);
        pool.tick(core::iter::empty, None, &[], &mut Clock);
        assert_eq!(pool.get(slot).unwrap().vars.monster_eggs, 0);
        let egg = pool.items().last().unwrap();
        assert_eq!(egg.kind, ItemKind::Container(Kind::Egg));
        assert_eq!(egg.pos, Vec3::new(-200.0, 200.0, 0.0));
        assert_eq!(egg.anim_ticks, 1);
        assert!(egg.vel_air.x < 0.0 && egg.lr < 0.0);
        assert!(egg.spin_step > 0.0);
        assert_eq!(pool.active_count(), 16);
    }
    #[test]
    fn disabled_eggs_do_not_allocate_or_advance_the_random_seed() {
        for switches in [
            super::super::normal::Switches {
                toggles: 0,
                ..Default::default()
            },
            super::super::normal::Switches {
                appearance: super::super::normal::Appearance::None,
                ..Default::default()
            },
        ] {
            let mut pool = ItemPool {
                normal_switches: switches,
                ..Default::default()
            };
            pool.make_item(StageItem::Monster(0), Vec3::ZERO).unwrap();
            let seed = crate::rng::seed();
            pool.tick(core::iter::empty, None, &[], &mut Clock);
            assert_eq!(crate::rng::seed(), seed);
            assert_eq!(pool.active_count(), 1);
            assert_eq!(pool.items().next().unwrap().vars.monster_eggs, 0);
        }
    }
    #[test]
    fn a_weapon_container_clash_precedes_the_hurtbox_and_defers_setoff() {
        let mut pool = ItemPool::default();
        let slot = pool
            .spawn_container(Kind::Egg, Vec3::ZERO, Vec3::ZERO)
            .unwrap();
        let item = pool.get_mut(slot).unwrap();
        thrown(item);
        item.attack.state = AttackState::New;
        item.update_attack_positions();
        item.owner = Some(1);
        let mut weapons = WeaponPool::default();
        assert!(weapons.spawn(WeaponSpawn {
            kind: WeaponKind::MarioFireball,
            owner_port: 0,
            team: 0,
            position: Vec3::ZERO,
            facing: 1.0,
            stale: crate::stale::WeaponStale::FRESH
        }));
        weapons.search_weapons();
        weapons.hit_item(item, super::super::ITEM_RECORD_BASE + slot);
        assert_eq!(item.damage_queue, 0);
        assert_eq!(item.hit_attack_damage, 3);
        assert_eq!(weapons.active_count(), 1);
        weapons.finish_clashes();
        assert_eq!(weapons.active_count(), 0);
        pool.resolve(&[], &mut NoItemAnims, core::iter::empty);
        assert_eq!(
            pool.get(slot).unwrap().status,
            ItemStatus::Container(Status::Explode)
        );
        assert!(pool.get(slot).unwrap().hidden);
        pool.tick(core::iter::empty, None, &[], &mut NoItemAnims);
        assert!(pool.get(slot).unwrap().hidden);
    }
}
