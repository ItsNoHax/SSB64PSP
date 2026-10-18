//! The seven held utilities (`it{Sword,Bat,Harisen,StarRod,LGun,FFlower,Hammer}.c`).
use super::{map, HitProc, Item, ItemAttributes, ItemKind, ItemStatus};
use crate::combat::AttackState;
use crate::weapon::MapSurface;
use ssb_engine::math::Vec3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Sword,
    Bat,
    Fan,
    StarRod,
    RayGun,
    FireFlower,
    Hammer,
}
impl Kind {
    pub fn from_index(i: u8) -> Option<Self> {
        Some(match i {
            7 => Self::Sword,
            8 => Self::Bat,
            9 => Self::Fan,
            10 => Self::StarRod,
            11 => Self::RayGun,
            12 => Self::FireFlower,
            13 => Self::Hammer,
            _ => return None,
        })
    }
    /// `ITAttributes::spin_speed` as a fraction.
    pub fn spin_speed(self) -> f32 {
        use super::spin;
        f32::from(match self {
            Self::Sword => spin::SWORD,
            Self::Bat => spin::BAT,
            Self::Fan => spin::HARISEN,
            Self::StarRod => spin::STARROD,
            Self::RayGun => spin::LGUN,
            Self::FireFlower => spin::FFLOWER,
            Self::Hammer => spin::HAMMER,
        }) * 0.01
    }
    fn physics(self) -> (f32, f32, f32, f32) {
        match self {
            Self::Sword | Self::StarRod => (1.2, 100.0, 0.2, 0.5),
            Self::Bat => (1.5, 120.0, 0.2, 0.5),
            Self::Fan => (1.0, 80.0, 0.0, 0.3),
            Self::RayGun => (1.5, 130.0, 0.2, 0.1),
            Self::FireFlower => (1.2, 100.0, 0.0, 0.5),
            Self::Hammer => (1.5, 120.0, 0.5, 0.2),
        }
    }
}
/// Per [`Kind`]: file 251's `ll*ItemAttributes` at 0x190, 0x1D8, 0x220,
/// 0x48C, 0x268, 0x2E4 and 0x374.
pub static ATTRIBUTES: [ItemAttributes; 7] = [
    super::attributes::SWORD,
    super::attributes::BAT,
    super::attributes::HARISEN,
    super::attributes::STARROD,
    super::attributes::LGUN,
    super::attributes::FFLOWER,
    super::attributes::HAMMER,
];
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Init,
    Wait,
    Fall,
    Hold,
    Thrown,
    Dropped,
}
pub(super) fn make(k: Kind, pos: Vec3, vel: Vec3) -> Item {
    let mut i = Item::new(
        ItemKind::Equipment(k),
        &ATTRIBUTES[k as usize],
        ItemStatus::Equipment(Status::Init),
        AttackState::Off,
        pos,
        vel,
        0,
    );
    i.multi = match k {
        Kind::StarRod => 20,
        Kind::RayGun => 16,
        Kind::FireFlower => 60,
        _ => 0,
    };
    i.vars.container_root_yaw = match k {
        Kind::Fan => -core::f32::consts::FRAC_PI_2,
        Kind::RayGun => {
            if !crate::rng::rand_ushort().is_multiple_of(2) {
                core::f32::consts::FRAC_PI_2
            } else {
                -core::f32::consts::FRAC_PI_2
            }
        }
        Kind::StarRod | Kind::FireFlower => 0.0,
        _ => core::f32::consts::FRAC_PI_2,
    };
    i
}
fn kind(i: &Item) -> Kind {
    let ItemKind::Equipment(k) = i.kind else {
        unreachable!()
    };
    k
}
pub(super) fn hold(i: &mut Item) {
    if kind(i) != Kind::FireFlower {
        i.vars.container_root_yaw = 0.0;
    }
    i.set_status(ItemStatus::Equipment(Status::Hold));
}
pub(super) fn release(i: &mut Item, dropped: bool, lr: f32) {
    i.set_status(ItemStatus::Equipment(if dropped {
        Status::Dropped
    } else {
        Status::Thrown
    }));
    i.vars.equipment_child_yaw = match kind(i) {
        Kind::Fan => -core::f32::consts::FRAC_PI_2,
        Kind::RayGun => core::f32::consts::FRAC_PI_2 * lr,
        Kind::FireFlower => 0.0,
        _ => core::f32::consts::FRAC_PI_2,
    };
    if dropped && kind(i) == Kind::Hammer {
        // `itHammerDroppedSetStatus`.
        i.clear_colanim();
    }
}
fn wait(i: &mut Item) {
    i.set_ground_allow_pickup();
    i.set_status(ItemStatus::Equipment(Status::Wait));
}
pub(super) fn update(i: &mut Item, s: Status) -> bool {
    if !matches!(s, Status::Wait | Status::Hold) {
        let (g, t, _, _) = kind(i).physics();
        i.apply_gravity_clamp_tvel(g, t);
        i.rotate_z += i.spin_step;
    }
    true
}
pub(super) fn proc_map<I, F>(i: &mut Item, s: Status, surfaces: &F) -> bool
where
    F: Fn() -> I,
    I: IntoIterator<Item = MapSurface>,
{
    if s == Status::Hold {
        return true;
    }
    if s == Status::Wait {
        if !map::check_lr_wall_proc_no_floor(i, surfaces) {
            i.is_allow_pickup = false;
            map::set_air(i);
            i.set_status(ItemStatus::Equipment(Status::Fall));
        }
        return true;
    }
    let k = kind(i);
    let (_, _, common, ground) = k.physics();
    let depleted = i.multi == 0
        && (matches!(k, Kind::RayGun | Kind::FireFlower)
            && matches!(s, Status::Thrown | Status::Dropped)
            || k == Kind::StarRod && s == Status::Dropped);
    if depleted {
        return !map::check_destroy_landing(i, common, surfaces);
    }
    let result = map::check_destroy_dropped(i, common, ground, surfaces);
    // Bat, Fan and Star Rod's fall callback discards the destroy return.
    if result.destroy
        && !(matches!(k, Kind::Bat | Kind::Fan | Kind::StarRod)
            && matches!(s, Status::Init | Status::Fall))
    {
        return false;
    }
    if result.goto_wait {
        wait(i);
    }
    true
}
pub(super) fn hit(i: &mut Item, s: Status, proc: HitProc, lr: f32) -> Option<bool> {
    if !matches!(s, Status::Thrown | Status::Dropped) {
        return None;
    }
    match proc {
        HitProc::Hit | HitProc::Shield | HitProc::SetOff => {
            i.attack.state = AttackState::Off;
            i.vel_set_rebound();
        }
        HitProc::Hop => i.common_proc_hop(),
        HitProc::Reflector => i.common_proc_reflector(lr),
        _ => return None,
    }
    Some(true)
}
