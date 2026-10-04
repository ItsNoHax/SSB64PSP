//! Item pickup, lift and throws (`ftcommonitemthrow.c`, `ftcommonget.c`).
//! The item callbacks are delivered through the fighter's item requests.

use ssb_engine::input::N64Buttons;
use ssb_engine::math::{Vec2, Vec3};

use crate::fighter::{Facing, Fighter, FighterKind};
use crate::item::{HeldItem, ItemRequest, ItemType, ItemWeight};
use crate::status::{self, AnyStatus, DonkeyStatus, Status, StatusTiming};

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
/// distance; creation order wins ties. Each weight uses its own hand reach.
pub fn find_item(f: &Fighter) -> Option<HeldItem> {
    find_weight(f, None)
}
fn find_weight(f: &Fighter, weight: Option<ItemWeight>) -> Option<HeldItem> {
    let p = pickup(f.kind);
    let floor = f.floor.map(|s| s.line);
    let mut closest = f32::MAX;
    let mut found = None;
    for c in f.items.view.candidates.iter().flatten() {
        if weight.is_some_and(|w| c.item.weight != w) || c.floor_line != floor {
            continue;
        }
        let (offset, range) = if c.item.weight == ItemWeight::Heavy {
            (p.offset_heavy, p.range_heavy)
        } else {
            (p.offset_light, p.range_light)
        };
        let x = f.pos.x + f.facing.sign() * offset.x;
        let y = f.pos.y + offset.y;
        if x - range.x - c.coll.width < c.pos.x
            && c.pos.x < x + range.x + c.coll.width
            && y - range.y - c.coll.top < c.pos.y
            && c.pos.y < y + range.y - c.coll.bottom
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
    if f.items.held.is_some() {
        return false;
    }
    let Some(item) = find_item(f) else {
        return false;
    };
    let s = if item.weight == ItemWeight::Heavy {
        Status::HeavyGet
    } else {
        Status::LightGet
    };
    f.motion_script.flags[1] = 0;
    let timing = timing(f, s.into());
    status::set_status(f, s, 0.0, timing);
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

pub const THROW_DESCS: [ThrowDesc; 22] = [
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
    desc(false, 70.0, 60, 100.0),
    desc(false, 70.0, 60, 100.0),
    desc(true, 90.0, 20, 100.0),
    desc(true, 90.0, 20, 100.0),
];

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThrowState {
    /// TopN yaw during script-driven turns and the eight-frame lift turn.
    pub model_yaw: Option<f32>,
    pub lift_turn_tics: u8,
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
            model_yaw: None,
            lift_turn_tics: 0,
            turn_tics: 0,
            turn_step: 0,
            turn_invert_wait: 0,
            angle: 361,
            velocity: 1.0,
            damage: 1.0,
        }
    }
}

fn timing(f: &Fighter, s: AnyStatus) -> StatusTiming {
    crate::motion::anim_length(f.kind, s).map_or(StatusTiming::unknown(), StatusTiming::frames)
}

pub fn is_throw(s: Status) -> bool {
    (Status::LightThrowDrop as u16..=Status::HeavyThrowB4 as u16).contains(&(s as u16))
}
pub fn is_donkey_throw(s: AnyStatus) -> bool {
    matches!(
        s,
        AnyStatus::Donkey(
            DonkeyStatus::HeavyThrowF
                | DonkeyStatus::HeavyThrowB
                | DonkeyStatus::HeavyThrowF4
                | DonkeyStatus::HeavyThrowB4
        )
    )
}
pub fn common_throw(s: AnyStatus) -> Option<Status> {
    match s {
        AnyStatus::Common(s) if is_throw(s) => Some(s),
        AnyStatus::Donkey(DonkeyStatus::HeavyThrowF) => Some(Status::HeavyThrowF),
        AnyStatus::Donkey(DonkeyStatus::HeavyThrowB) => Some(Status::HeavyThrowB),
        AnyStatus::Donkey(DonkeyStatus::HeavyThrowF4) => Some(Status::HeavyThrowF4),
        AnyStatus::Donkey(DonkeyStatus::HeavyThrowB4) => Some(Status::HeavyThrowB4),
        _ => None,
    }
}
fn facing_yaw(f: &Fighter) -> f32 {
    core::f32::consts::FRAC_PI_2 * f.facing.sign()
}
pub fn model_yaw(f: &Fighter) -> Option<f32> {
    if f.status.status == Status::HammerTurn {
        return f.item_use.yaw;
    }
    if common_throw(f.status.status).is_some()
        || matches!(f.status.status, AnyStatus::Common(Status::LiftTurn))
    {
        f.item_throw.model_yaw
    } else {
        None
    }
}

/// TopN's world axes for collision joints and held-item attachments.
/// `ftCommonItemThrowUpdateModelYaw` refreshes these on every turn step,
/// including the steps before and after the halfway facing inversion.
pub fn model_axes(f: &Fighter) -> [Vec3; 3] {
    let Some(yaw) = model_yaw(f) else {
        let sign = f.facing.sign();
        return [
            Vec3::new(0.0, 0.0, -sign),
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::new(sign, 0.0, 0.0),
        ];
    };
    let (sin, cos) = ssb_engine::math::sin_cos(yaw);
    [
        Vec3::new(cos, 0.0, -sin),
        Vec3::new(0.0, 1.0, 0.0),
        Vec3::new(sin, 0.0, cos),
    ]
}

/// `ftCommonItemThrowUpdateModelYaw`: the turn flips facing halfway.
/// Joint transforms follow this yaw; the gameplay direction flips halfway.
fn update_turn(f: &mut Fighter) {
    let flag = f.motion_script.flags[3];
    if flag != 0 {
        f.item_throw.turn_tics = flag;
        f.item_throw.turn_step = flag;
        f.item_throw.turn_invert_wait = flag / 2;
        f.motion_script.flags[3] = 0;
    }
    if f.item_throw.turn_tics != 0 {
        let yaw = f.item_throw.model_yaw.unwrap_or_else(|| facing_yaw(f));
        f.item_throw.model_yaw = Some(yaw - core::f32::consts::PI / f.item_throw.turn_step as f32);
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
    set_throw(f, s.into());
}
fn set_throw(f: &mut Fighter, s: AnyStatus) {
    f.motion_script.flags = [0; 4];
    let t = timing(f, s);
    status::set_any_status(f, s, 0.0, t);
    status::play_anim_events(f);
    f.item_throw = ThrowState::default();
    update_turn(f);
}

/// `ftCommonItemThrowProcUpdate` / `ftCommonGetProcUpdate`.
pub fn update(f: &mut Fighter) -> bool {
    let current = f.status.status;
    if matches!(
        current,
        AnyStatus::Common(Status::LightGet | Status::HeavyGet)
    ) {
        let heavy = current == Status::HeavyGet;
        if f.motion_script.flags[1] != 0 {
            f.motion_script.flags[1] = 0;
            if let Some(item) = find_weight(
                f,
                Some(if heavy {
                    ItemWeight::Heavy
                } else {
                    ItemWeight::Light
                }),
            ) {
                f.items.request(ItemRequest::Hold { slot: item.slot });
            }
        }
        if f.status.animation_ended() {
            if heavy && f.items.held.is_some() {
                if crate::grab::is_donkey(f.kind) {
                    crate::grab::set_donkey_throwf_wait(f);
                } else {
                    set_lift_wait(f);
                }
            } else {
                // LightGet's proc_damage also starts the Hammer's timer.
                if !heavy {
                    light_get_proc_damage(f);
                }
                status::set_wait(f);
            }
        }
        return true;
    }
    if current == Status::LiftWait {
        if !check_heavy_throw(f) && f.stick.forward(f.facing) <= status::TURN_STICK_MIN {
            status::set_status(f, Status::LiftTurn, 0.0, StatusTiming::unknown());
            f.item_throw.model_yaw = Some(facing_yaw(f));
            f.item_throw.lift_turn_tics = 8;
            lift_turn(f);
        }
        return true;
    }
    if current == Status::LiftTurn {
        lift_turn(f);
        if f.item_throw.lift_turn_tics == 0 {
            set_lift_wait(f);
        }
        check_heavy_throw(f);
        return true;
    }
    let Some(s) = common_throw(current) else {
        return false;
    };
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
fn set_lift_wait(f: &mut Fighter) {
    status::set_status(f, Status::LiftWait, 0.0, StatusTiming::unknown());
}
fn lift_turn(f: &mut Fighter) {
    f.item_throw.lift_turn_tics -= 1;
    f.item_throw.model_yaw =
        Some(f.item_throw.model_yaw.unwrap_or_else(|| facing_yaw(f)) - core::f32::consts::PI / 8.0);
    if f.item_throw.lift_turn_tics == 4 {
        f.facing = f.facing.flipped();
        f.physics.vel_ground.x = -f.physics.vel_ground.x;
    }
}
/// `ftCommonHeavyThrowCheckInterruptCommon`: A or B, before cargo actions.
pub fn check_heavy_throw(f: &mut Fighter) -> bool {
    if !f.items.held.is_some_and(|i| i.weight == ItemWeight::Heavy)
        || !f.button_tap().contains(N64Buttons::A | N64Buttons::B)
    {
        return false;
    }
    let x = i32::from(f.stick.x);
    let angle = ssb_engine::math::atan2(f.stick.y as f32, x.abs() as f32);
    let forward = x as f32 * f.facing.sign() >= 0.0;
    let s = if x.abs() >= 56 && f.stick.hold_x < 8 {
        if forward {
            Status::HeavyThrowF4
        } else {
            Status::HeavyThrowB4
        }
    } else if x.abs() >= 20 && angle.abs() <= 50.0_f32.to_radians() {
        if forward {
            Status::HeavyThrowF
        } else {
            Status::HeavyThrowB
        }
    } else {
        Status::HeavyThrowF
    };
    let s = if crate::grab::is_donkey(f.kind) {
        AnyStatus::Donkey(match s {
            Status::HeavyThrowF => DonkeyStatus::HeavyThrowF,
            Status::HeavyThrowB => DonkeyStatus::HeavyThrowB,
            Status::HeavyThrowF4 => DonkeyStatus::HeavyThrowF4,
            _ => DonkeyStatus::HeavyThrowB4,
        })
    } else {
        s.into()
    };
    set_throw(f, s);
    true
}
/// Common lift statuses' `proc_damage` and `proc_map` drop immediately.
pub fn common_heavy(s: AnyStatus) -> bool {
    matches!(
        s,
        AnyStatus::Common(
            Status::HeavyGet
                | Status::LiftWait
                | Status::LiftTurn
                | Status::HeavyThrowF
                | Status::HeavyThrowB
                | Status::HeavyThrowF4
                | Status::HeavyThrowB4
        )
    )
}
pub fn on_damage(f: &mut Fighter) {
    if (common_heavy(f.status.status) || is_donkey_throw(f.status.status)) && f.items.held.is_some()
    {
        drop_item(f);
    }
    if f.status.status == Status::LightGet {
        light_get_proc_damage(f);
    }
}

/// `ftCommonLightGetProcMap`/`HeavyGet`'s and the lift statuses' ground
/// loss: a heavy item drops, a consumable is eaten.
pub fn on_floor_lost(f: &mut Fighter) {
    if common_heavy(f.status.status) && f.items.held.is_some() {
        drop_item(f);
    }
    if f.status.status == Status::LightGet {
        light_get_proc_damage(f);
    }
}

/// `ftCommonLightGetProcDamage`: a held Tomato or Heart heals and is
/// destroyed. The Hammer's timer and music are not ported.
pub fn light_get_proc_damage(f: &mut Fighter) {
    let Some(held) = f.items.held else {
        return;
    };
    if crate::item_use::holds_hammer(f) {
        f.item_use.hammer_tics = 720;
        return;
    }
    if held.ty != ItemType::Consume {
        return;
    }
    let heal = match held.kind {
        crate::item::ItemKind::Utility(crate::item::utility::Kind::Tomato) => {
            crate::item::utility::TOMATO_DAMAGE_HEAL
        }
        crate::item::ItemKind::Utility(crate::item::utility::Kind::Heart) => {
            crate::item::utility::HEART_DAMAGE_HEAL
        }
        _ => return,
    };
    crate::colanim::set_heal_damage(f, heal);
    f.items.request(ItemRequest::Destroy);
    f.items.held = None;
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
