//! Light item throws and pickup (`ftcommonitemthrow.c`, `ftcommonget.c`).
//! The item callbacks are delivered through the fighter's item requests.

use ssb_engine::input::N64Buttons;
use ssb_engine::math::{Vec2, Vec3};

use crate::fighter::{Facing, Fighter, FighterKind};
use crate::item::{HeldItem, ItemRequest, ItemType, ItemWeight};
use crate::status::{self, Status, StatusTiming};

/// `FTAttributes::itemlight_joint_id`, including polygon variants.
pub fn itemlight_joint(kind: FighterKind) -> usize {
    match crate::grab::base_kind(kind) {
        FighterKind::Samus => 11,
        FighterKind::Link | FighterKind::Purin => 16,
        FighterKind::Pikachu => 12,
        FighterKind::Yoshi => 18,
        _ => 17,
    }
}

/// `FTItemPickup`, from each fighter's `relocData/*Main.c`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pickup {
    pub offset_light: Vec2,
    pub range_light: Vec2,
    pub offset_heavy: Vec2,
    pub range_heavy: Vec2,
}

pub fn pickup(kind: FighterKind) -> Pickup {
    let (offset, range, range_y, heavy) = match kind {
        FighterKind::Donkey => (163.0, 588.0, 200.0, 116.0),
        FighterKind::Samus | FighterKind::Captain => (107.0, 386.0, 200.0, 75.0),
        FighterKind::Kirby => (112.0, 403.0, 150.0, 80.0),
        FighterKind::Purin => (118.0, 420.0, 200.0, 80.0),
        FighterKind::Yoshi => (100.0, 588.0, 200.0, 0.0),
        FighterKind::PolyDonkey | FighterKind::GiantDonkey => (163.0, 490.0, 150.0, 116.0),
        FighterKind::PolySamus | FighterKind::PolyCaptain => (107.0, 322.0, 150.0, 75.0),
        FighterKind::PolyKirby => (112.0, 336.0, 150.0, 80.0),
        FighterKind::PolyPurin => (118.0, 350.0, 150.0, 80.0),
        FighterKind::PolyYoshi => (100.0, 490.0, 150.0, 0.0),
        FighterKind::PolyFox => (105.0, 314.0, 150.0, 75.0),
        FighterKind::PolyPikachu => (78.0, 230.0, 150.0, 75.0),
        FighterKind::PolyMario
        | FighterKind::PolyLuigi
        | FighterKind::PolyLink
        | FighterKind::PolyNess
        | FighterKind::MetalMario => (105.0, 315.0, 150.0, 75.0),
        FighterKind::Boss => (0.0, 150.0, 150.0, 0.0),
        _ => (105.0, 378.0, 200.0, 75.0),
    };
    Pickup {
        offset_light: Vec2::new(offset, 0.0),
        range_light: Vec2::new(range, range_y),
        offset_heavy: Vec2::new(heavy, 0.0),
        range_heavy: Vec2::new(150.0, 150.0),
    }
}

/// `ftCommonGetFindItem`: same floor, strict overlap, closest horizontal
/// distance; creation order wins ties. No heavy item is ported yet.
pub fn find_item(f: &Fighter) -> Option<HeldItem> {
    let p = pickup(f.kind);
    let floor = f.floor.map(|s| s.line);
    let mut closest = f32::MAX;
    let mut found = None;
    for c in f.items.view.candidates.iter().flatten() {
        if c.item.weight != ItemWeight::Light || c.floor_line != floor {
            continue;
        }
        let x = f.pos.x + f.facing.sign() * p.offset_light.x;
        let y = f.pos.y + p.offset_light.y;
        if x - p.range_light.x - c.coll.width < c.pos.x
            && c.pos.x < x + p.range_light.x + c.coll.width
            && y - p.range_light.y - c.coll.top < c.pos.y
            && c.pos.y < y + p.range_light.y - c.coll.bottom
        {
            let distance = (x - c.pos.x).abs();
            if distance < closest {
                closest = distance;
                found = Some(c.item);
            }
        }
    }
    found
}

/// `ftCommonGetCheckInterruptCommon` / `ftCommonGetSetStatus`.
pub fn check_get(f: &mut Fighter) -> bool {
    if f.items.held.is_some() || find_item(f).is_none() {
        return false;
    }
    f.motion_script.flags[1] = 0;
    let timing = timing(f, Status::LightGet);
    status::set_status(f, Status::LightGet, 0.0, timing);
    status::play_anim_events(f);
    true
}

/// `dFTCommonDataItemThrowDescs`, indexed from `LightThrowDrop`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThrowDesc {
    pub is_smash: bool,
    pub velocity: f32,
    pub angle: i32,
    pub damage_scale: f32,
}

const fn desc(is_smash: bool, velocity: f32, angle: i32, damage_scale: f32) -> ThrowDesc {
    ThrowDesc {
        is_smash,
        velocity,
        angle,
        damage_scale,
    }
}

pub const THROW_DESCS: [ThrowDesc; 18] = [
    desc(false, 36.0, 110, 50.0),
    desc(false, 120.0, 10, 100.0),
    desc(false, 60.0, 15, 100.0),
    desc(false, 60.0, 15, 100.0),
    desc(false, 65.0, 90, 100.0),
    desc(false, 65.0, -70, 100.0),
    desc(true, 110.0, 8, 100.0),
    desc(true, 110.0, 8, 100.0),
    desc(true, 110.0, 90, 100.0),
    desc(true, 110.0, -70, 100.0),
    desc(false, 75.0, 8, 100.0),
    desc(false, 75.0, 8, 100.0),
    desc(false, 80.0, 90, 100.0),
    desc(false, 75.0, -90, 100.0),
    desc(true, 120.0, 7, 100.0),
    desc(true, 120.0, 7, 100.0),
    desc(true, 120.0, 90, 100.0),
    desc(true, 140.0, -90, 100.0),
];

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThrowState {
    pub turn_tics: u32,
    pub turn_step: u32,
    pub turn_invert_wait: u32,
    pub angle: i32,
    pub velocity: f32,
    pub damage: f32,
}

impl Default for ThrowState {
    fn default() -> Self {
        Self {
            turn_tics: 0,
            turn_step: 0,
            turn_invert_wait: 0,
            angle: 361,
            velocity: 1.0,
            damage: 1.0,
        }
    }
}

fn timing(f: &Fighter, s: Status) -> StatusTiming {
    crate::motion::anim_length(f.kind, s.into())
        .map_or(StatusTiming::unknown(), StatusTiming::frames)
}

pub fn is_throw(s: Status) -> bool {
    (Status::LightThrowDrop as u16..=Status::LightThrowAirLw4 as u16).contains(&(s as u16))
}

/// `ftCommonItemThrowUpdateModelYaw`: the turn flips facing halfway.
/// Joint yaw is presentation; the gameplay direction follows the source.
fn update_turn(f: &mut Fighter) {
    let flag = f.motion_script.flags[3];
    if flag != 0 {
        f.item_throw.turn_tics = flag;
        f.item_throw.turn_step = flag;
        f.item_throw.turn_invert_wait = flag / 2;
        f.motion_script.flags[3] = 0;
    }
    if f.item_throw.turn_tics != 0 {
        f.item_throw.turn_tics -= 1;
        if f.item_throw.turn_invert_wait != 0 {
            f.item_throw.turn_invert_wait -= 1;
            if f.item_throw.turn_invert_wait == 0 {
                f.facing = match f.facing {
                    Facing::Right => Facing::Left,
                    Facing::Left => Facing::Right,
                };
            }
        }
    }
}

/// `ftCommonItemThrowSetStatus`.
pub fn set_item_throw(f: &mut Fighter, s: Status) {
    debug_assert!(is_throw(s));
    f.motion_script.flags = [0; 4];
    let t = timing(f, s);
    status::set_status(f, s, 0.0, t);
    status::play_anim_events(f);
    f.item_throw = ThrowState::default();
    update_turn(f);
}

/// `ftCommonItemThrowProcUpdate` / `ftCommonGetProcUpdate`.
pub fn update(f: &mut Fighter, s: Status) -> bool {
    if s == Status::LightGet {
        if f.motion_script.flags[1] != 0 {
            f.motion_script.flags[1] = 0;
            if let Some(item) = find_item(f) {
                f.items.request(ItemRequest::Hold { slot: item.slot });
            }
        }
        if f.status.animation_ended() {
            status::set_wait(f);
        }
        return true;
    }
    if !is_throw(s) {
        return false;
    }
    update_turn(f);
    // `motion_vars.item_throw` overlays flags 0..2, big-endian bitfields.
    let flag2 = f.motion_script.flags[2];
    if flag2 != 0 {
        f.item_throw.velocity = ((flag2 >> 12) & 0xfff) as f32 * 0.01;
        f.item_throw.angle = ((flag2 << 20) as i32) >> 20;
        f.motion_script.flags[2] = 0;
    }
    let flag1 = f.motion_script.flags[1];
    if flag1 != 0 {
        f.item_throw.damage = (flag1 & 0xff_ffff) as f32 * 0.01;
        f.motion_script.flags[1] = 0;
    }
    if f.items.held.is_some() && f.motion_script.flags[0] != 0 {
        let d = THROW_DESCS[(s as usize) - Status::LightThrowDrop as usize];
        // All ported fighter attributes have velocity/damage scale 100%.
        let speed = d.velocity * f.item_throw.velocity;
        let angle = if f.item_throw.angle == 361 {
            d.angle
        } else {
            f.item_throw.angle
        };
        let r = angle as f32 * core::f32::consts::PI / 180.0;
        let (sin, cos) = ssb_engine::math::sin_cos(r);
        let vel = Vec3::new(cos * speed * f.facing.sign(), sin * speed, 0.0);
        let throw_mul = d.damage_scale * 0.01 * f.item_throw.damage;
        f.items.request(if s == Status::LightThrowDrop {
            ItemRequest::Drop { vel, throw_mul }
        } else {
            ItemRequest::Throw {
                vel,
                throw_mul,
                is_smash: d.is_smash,
            }
        });
        f.items.held = None;
        f.motion_script.flags[0] = 0;
    }
    if f.status.animation_ended() {
        status::set_wait_or_fall(f);
    }
    true
}

/// `ftCommonLightThrowCheckItemTypeThrow`.
pub fn check_item_type_throw(f: &Fighter) -> bool {
    f.items
        .held
        .is_some_and(|i| i.ty == ItemType::Throw || f.input.buttons.contains(N64Buttons::Z))
        && f.button_tap().contains(N64Buttons::A)
}

/// `ftCommonLightThrowDecideSetStatus`.
pub fn decide_set_status(f: &mut Fighter) {
    let x = i32::from(f.stick.x);
    let y = i32::from(f.stick.y);
    let forward = x as f32 * f.facing.sign() >= 0.0;
    let angle = ssb_engine::math::atan2(y as f32, (x as f32).abs());
    let s = if x.abs() >= 56 && f.stick.hold_x < 8 {
        if forward {
            Status::LightThrowF4
        } else {
            Status::LightThrowB4
        }
    } else if y >= 53 && (f.stick.hold_y as f32) < 4.0 + f.attributes.kneebend_anim_length {
        Status::LightThrowHi4
    } else if y <= -53 && f.stick.hold_y < 4 {
        Status::LightThrowLw4
    } else if x.abs() >= 20 && angle.abs() <= 50.0_f32.to_radians() {
        if forward {
            Status::LightThrowF
        } else {
            Status::LightThrowB
        }
    } else if y >= 20 && angle > 50.0_f32.to_radians() {
        Status::LightThrowHi
    } else if y <= -20 && angle < -50.0_f32.to_radians() {
        Status::LightThrowLw
    } else if f.items.held.is_some_and(|i| i.ty == ItemType::Throw) {
        Status::LightThrowF
    } else {
        Status::LightThrowDrop
    };
    set_item_throw(f, s);
}

pub fn check_guard(f: &mut Fighter) -> bool {
    if f.items.held.is_some() && f.button_tap().contains(N64Buttons::A) {
        if f.guard.slide_tics != 0 {
            set_item_throw(f, Status::LightThrowDash);
        } else {
            decide_set_status(f);
        }
        return true;
    }
    if f.guard.slide_tics != 0 {
        f.guard.slide_tics -= 1;
    }
    false
}

pub fn check_escape(f: &mut Fighter) -> bool {
    if check_item_type_throw(f) && f.reaction.itemthrow_buffer_tics != 0 {
        let s = if f.status.status == Status::EscapeF {
            Status::LightThrowF4
        } else {
            Status::LightThrowB4
        };
        set_item_throw(f, s);
        return true;
    }
    if f.reaction.itemthrow_buffer_tics != 0 {
        f.reaction.itemthrow_buffer_tics -= 1;
    }
    false
}

/// `ftCommonAttackAirCheckInterruptCommon`'s item branch.
pub fn check_air(f: &mut Fighter) -> bool {
    let x = f.stick.x as f32;
    let y = f.stick.y as f32;
    let neutral = x.abs() < status::ATTACKAIR_DIRECTION_STICK_RANGE_MIN as f32
        && y.abs() < status::ATTACKAIR_DIRECTION_STICK_RANGE_MIN as f32;
    let s = if neutral {
        Status::LightThrowAirF
    } else if y > 1.191_753_6 * x.abs() {
        if f.stick.hold_y < 8 {
            Status::LightThrowAirHi4
        } else {
            Status::LightThrowAirHi
        }
    } else if y < -1.191_753_6 * x.abs() {
        if f.stick.hold_y < 8 {
            Status::LightThrowAirLw4
        } else {
            Status::LightThrowAirLw
        }
    } else if x * f.facing.sign() >= 0.0 {
        if f.stick.hold_x < 8 {
            Status::LightThrowAirF4
        } else {
            Status::LightThrowAirF
        }
    } else if f.stick.hold_x < 8 {
        Status::LightThrowAirB4
    } else {
        Status::LightThrowAirB
    };
    if neutral && f.items.held.is_some_and(|i| i.ty != ItemType::Throw) {
        drop_item(f);
        return false;
    }
    set_item_throw(f, s);
    true
}

/// `ftSetupDropItem`: release with zero velocity and ordinary damage.
pub fn drop_item(f: &mut Fighter) {
    f.items.request(ItemRequest::Drop {
        vel: Vec3::ZERO,
        throw_mul: 1.0,
    });
    f.items.held = None;
}
