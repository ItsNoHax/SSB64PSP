//! `ftcommonitemswing.c`, `ftcommonitemshoot.c` and `fthammer.c`.
use crate::fighter::Fighter;
use crate::item::{equipment::Kind, ItemKind, ItemRequest};
use crate::status::{self, AnyStatus, JumpInput, Preserve, Status, StatusTiming};
use ssb_engine::{input::N64Buttons, math::Vec3};

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct State {
    pub fan_reset: u8,
    pub flame_index: u8,
    pub flame_wait: u8,
    pub effect_wait: u8,
    pub fire_count: u32,
    pub released: bool,
    pub release_lag: u8,
    pub hammer_tics: u16,
    pub knee_frame: f32,
    pub landing_frame: f32,
    pub turn_left: u8,
    pub yaw: Option<f32>,
}
pub fn held_kind(f: &Fighter) -> Option<Kind> {
    match f.items.held?.kind {
        ItemKind::Equipment(k) => Some(k),
        _ => None,
    }
}
pub fn holds_hammer(f: &Fighter) -> bool {
    held_kind(f) == Some(Kind::Hammer)
}
pub fn is_hammer(s: AnyStatus) -> bool {
    matches!(s, AnyStatus::Common(s) if (146..=151).contains(&(s as u16)))
}

/// The swing callbacks' `ftPhysicsApplyGroundVelTransN` and
/// `ftPhysicsApplyGroundFrictionOrTransN`. In `ftdata.c`, Yoshi, Kirby
/// and Jigglypuff's smash clips have bit 0x40000000; every dash does.
pub fn apply_ground_physics(f: &mut Fighter) -> bool {
    let dash = matches!(
        f.status.status,
        AnyStatus::Common(
            Status::SwordSwingDash
                | Status::BatSwingDash
                | Status::HarisenSwingDash
                | Status::StarRodSwingDash
        )
    );
    let smash = matches!(
        f.status.status,
        AnyStatus::Common(
            Status::SwordSwing4 | Status::BatSwing4 | Status::HarisenSwing4 | Status::StarRodSwing4
        )
    ) && matches!(
        f.kind.polygon_base().unwrap_or(f.kind),
        crate::fighter::FighterKind::Yoshi
            | crate::fighter::FighterKind::Kirby
            | crate::fighter::FighterKind::Purin
    );
    if dash || smash {
        crate::physics::apply_ground_vel_transn(
            &mut f.physics,
            f.root_motion,
            f.topn_lr,
            f.attributes.size,
        );
        true
    } else {
        false
    }
}

/// Air shooters use `ftPhysicsApplyAirVelDrift`, which keeps an existing
/// fast fall but does not accept a new fast-fall input.
pub fn skips_fast_fall(s: AnyStatus) -> bool {
    matches!(
        s,
        AnyStatus::Common(Status::LGunShootAir | Status::FireFlowerShootAir)
    )
}
fn timing(f: &Fighter, s: Status, speed: f32) -> StatusTiming {
    let mut t = crate::motion::anim_length(f.kind, s.into())
        .map_or(StatusTiming::unknown(), StatusTiming::frames);
    t.anim_speed = speed;
    t
}
/// Returns true if a held swing/shoot item takes this attack input.
/// `swing`: neutral, forward tilt, forward smash, dash (source table order).
pub fn check(f: &mut Fighter, swing: usize, air: bool) -> bool {
    let Some(k) = held_kind(f) else {
        return false;
    };
    let s = match k {
        Kind::RayGun => {
            if air {
                Status::LGunShootAir
            } else {
                Status::LGunShoot
            }
        }
        Kind::FireFlower => {
            if air {
                Status::FireFlowerShootAir
            } else {
                Status::FireFlowerShoot
            }
        }
        Kind::Hammer => return false,
        _ if air => return false,
        _ => [
            [
                Status::SwordSwing1,
                Status::SwordSwing3,
                Status::SwordSwing4,
                Status::SwordSwingDash,
            ],
            [
                Status::BatSwing1,
                Status::BatSwing3,
                Status::BatSwing4,
                Status::BatSwingDash,
            ],
            [
                Status::HarisenSwing1,
                Status::HarisenSwing3,
                Status::HarisenSwing4,
                Status::HarisenSwingDash,
            ],
            [
                Status::StarRodSwing1,
                Status::StarRodSwing3,
                Status::StarRodSwing4,
                Status::StarRodSwingDash,
            ],
        ][k as usize][swing],
    };
    let speed = if k == Kind::Bat && swing == 2 {
        0.75
    } else if k == Kind::Fan && swing != 3 {
        2.0
    } else {
        1.0
    };
    f.motion_script.flags[0] = 0;
    status::set_status(f, s, 0.0, timing(f, s, speed));
    status::play_anim_events(f);
    f.item_use.fan_reset = 0;
    f.item_use.flame_index = 0;
    f.item_use.flame_wait = 1;
    f.item_use.effect_wait = 1;
    f.item_use.fire_count = 0;
    f.item_use.released = false;
    f.item_use.release_lag = 0;
    f.motion_script.flags[1] = 0;
    if air {
        crate::physics::clamp_air_vel_x(&mut f.physics, f.attributes.air_speed_max_x);
    }
    true
}
fn fx(
    f: &mut Fighter,
    kind: u16,
    joint: i8,
    offset: Vec3,
    scatter: Option<Vec3>,
    reverse: bool,
    scale: bool,
) {
    crate::fteffect::request(
        f,
        crate::fteffect::EffectRequest {
            kind,
            joint,
            offset: Some(offset),
            scatter,
            lr: (f.facing.sign() * if reverse { -1.0 } else { 1.0 }) as i8,
            is_scale_pos: scale,
            flag: 0,
        },
    );
}
fn shoot(
    f: &mut Fighter,
    kind: crate::monster_weapon::ShotKind,
    offset: Vec3,
    smash: bool,
    index: u8,
    cost: u16,
) {
    let joint = crate::item_throw::itemlight_joint(f.kind) as u8;
    let pos = f.joint_world(joint, offset * (1.0 / f.attributes.size));
    f.weapon_spawn = Some(crate::weapon::WeaponSpawn {
        kind: crate::weapon::WeaponKind::Equipment {
            kind,
            smash,
            angle_index: index,
        },
        owner_port: f.port,
        team: f.team,
        position: pos,
        facing: f.facing.sign(),
        stale: crate::stale::WeaponStale::of(f),
    });
    f.items.request(ItemRequest::UseAmmo(cost));
    f.items.held_multi -= cost;
}
pub fn proc_hit(f: &mut Fighter) {
    if held_kind(f) == Some(Kind::Fan)
        && matches!(f.status.status, AnyStatus::Common(s) if (126..=141).contains(&(s as u16)))
    {
        f.items.request(ItemRequest::Scale(1.5));
        f.item_use.fan_reset = 2;
    }
}
pub fn update(f: &mut Fighter) -> bool {
    let AnyStatus::Common(s) = f.status.status else {
        return false;
    };
    if is_hammer(f.status.status) {
        hammer_update(f, s);
        return true;
    }
    if !(126..=145).contains(&(s as u16)) {
        return false;
    }
    use crate::fteffect::kind as E;
    let joint = crate::item_throw::itemlight_joint(f.kind) as i8;
    match s {
        Status::HarisenSwing1
        | Status::HarisenSwing3
        | Status::HarisenSwing4
        | Status::HarisenSwingDash => {
            if f.items.held.is_some() && f.item_use.fan_reset != 0 {
                f.item_use.fan_reset -= 1;
                if f.item_use.fan_reset == 0 {
                    f.items.request(ItemRequest::Scale(1.0));
                }
            }
        }
        Status::StarRodSwing1
        | Status::StarRodSwing3
        | Status::StarRodSwing4
        | Status::StarRodSwingDash
            if f.items.held.is_some() =>
        {
            f.motion_script.flags[1] = 0;
            let flag = f.motion_script.flags[0];
            if flag != 0 {
                if f.items.held_multi != 0 {
                    shoot(
                        f,
                        crate::monster_weapon::ShotKind::StarRod,
                        Vec3::new(0.0, 200.0, 0.0),
                        flag != 1,
                        0,
                        1,
                    );
                } else {
                    fx(
                        f,
                        E::DUST_LIGHT,
                        joint,
                        Vec3::new(0.0, 200.0, 0.0),
                        None,
                        true,
                        true,
                    );
                }
                f.motion_script.flags[0] = 0;
            }
        }
        _ => {}
    }
    if f.status.animation_ended() {
        status::set_wait_or_fall(f);
    }
    true
}
/// The source's `proc_accessory`, after physics/map and forwarded effects.
pub fn accessory(f: &mut Fighter) {
    if f.is_in_hitlag() {
        return;
    }
    use crate::fteffect::kind as E;
    let joint = crate::item_throw::itemlight_joint(f.kind) as i8;
    match f.status.status {
        AnyStatus::Common(Status::LGunShoot | Status::LGunShootAir) => {
            if f.items.held.is_some() && f.motion_script.flags[0] != 0 {
                if f.items.held_multi != 0 {
                    shoot(
                        f,
                        crate::monster_weapon::ShotKind::RayGun,
                        Vec3::new(0.0, 60.0, 180.0),
                        false,
                        0,
                        1,
                    );
                    fx(
                        f,
                        E::SPARKLE_WHITE_SCALE,
                        joint,
                        Vec3::new(0.0, 60.0, 180.0),
                        None,
                        false,
                        true,
                    );
                    fx(
                        f,
                        E::DUST_DASH_SMALL,
                        0,
                        Vec3::new(0.0, 0.0, -180.0),
                        None,
                        false,
                        false,
                    );
                } else {
                    fx(
                        f,
                        E::DUST_LIGHT,
                        joint,
                        Vec3::new(0.0, 60.0, 180.0),
                        None,
                        true,
                        true,
                    );
                }
                f.motion_script.flags[0] = 0;
            }
        }
        AnyStatus::Common(Status::FireFlowerShoot | Status::FireFlowerShootAir) => flower(f),
        _ => {}
    }
}
fn flower(f: &mut Fighter) {
    use crate::fteffect::kind as E;
    if !f.input.buttons.contains(N64Buttons::A) {
        f.item_use.released = true;
    }
    f.item_use.release_lag = (f.item_use.release_lag + 1).min(20);
    if f.item_use.release_lag < 20 && f.button_tap().contains(N64Buttons::A) {
        f.item_use.release_lag = 0;
    }
    if f.items.held.is_none() {
        return;
    }
    let joint = crate::item_throw::itemlight_joint(f.kind) as i8;
    if f.motion_script.flags[0] != 0 {
        let cost = if f.item_use.fire_count == 0 { 2 } else { 1 };
        f.item_use.effect_wait -= 1;
        if f.item_use.effect_wait == 0 {
            f.item_use.effect_wait = 12;
            if f.items.held_multi < cost {
                fx(
                    f,
                    E::DUST_LIGHT,
                    joint,
                    Vec3::new(60.0, 100.0, 0.0),
                    None,
                    true,
                    true,
                );
            } else {
                fx(
                    f,
                    E::DUST_LIGHT,
                    0,
                    Vec3::new(0.0, 0.0, -180.0),
                    None,
                    false,
                    false,
                );
            }
        }
        f.item_use.flame_wait -= 1;
        if f.item_use.flame_wait == 0 {
            f.item_use.flame_wait = 8;
            if f.items.held_multi >= cost {
                let i = f.item_use.flame_index;
                shoot(
                    f,
                    crate::monster_weapon::ShotKind::FireFlower,
                    Vec3::new(60.0, 100.0, 0.0),
                    false,
                    if i >= 5 { 8 - i } else { i },
                    cost,
                );
            }
            f.item_use.fire_count = (f.item_use.fire_count + 1).min(65536);
            f.item_use.flame_index += 1;
            if f.item_use.flame_index == 8 {
                f.item_use.flame_index = 0;
                f.motion.set(crate::stale::MotionAttackId::FireFlowerShoot);
                f.stats.restart();
            }
        }
        if f.motion_script.flags[0] == 1 {
            if f.items.held_multi >= cost {
                fx(
                    f,
                    E::SPARKLE_WHITE_SCALE,
                    joint,
                    Vec3::new(0.0, 80.0, 0.0),
                    Some(Vec3::new(90.0, 90.0, 90.0)),
                    false,
                    true,
                );
                fx(
                    f,
                    E::DUST_DASH_SMALL,
                    0,
                    Vec3::new(0.0, 0.0, -180.0),
                    None,
                    false,
                    false,
                );
            }
            f.motion_script.flags[0] = 2;
            f.status.timing.anim_speed = 0.0;
        }
    }
    if f.item_use.fire_count >= 5 && f.item_use.released && f.item_use.release_lag >= 20 {
        f.motion_script.flags[0] = 0;
        f.status.timing.anim_speed = 1.0;
    }
}
fn hammer_set(f: &mut Fighter, s: Status) {
    let keep = is_hammer(f.status.status);
    let frame = if keep { f.status.anim_frame } else { 0.0 };
    status::set_any_status_preserve(
        f,
        s.into(),
        frame,
        timing(f, s, 1.0),
        if keep {
            Preserve {
                hit: true,
                colanim: true,
                ..Preserve::NONE
            }
        } else {
            Preserve::NONE
        },
    );
    if f.colanim.id != crate::colanim::ColAnimId::FIGHTER_HAMMER {
        crate::colanim::check_set(f, crate::colanim::ColAnimId::FIGHTER_HAMMER, 0);
    }
}
pub fn hammer_wait(f: &mut Fighter) {
    hammer_set(f, Status::HammerWait);
}
pub fn hammer_fall(f: &mut Fighter) {
    hammer_set(f, Status::HammerFall);
    crate::physics::clamp_air_vel_x(&mut f.physics, f.attributes.air_speed_max_x);
}
pub fn tick_hammer(f: &mut Fighter) {
    if !holds_hammer(f) || f.status.status == Status::LightGet {
        return;
    }
    f.item_use.hammer_tics = f.item_use.hammer_tics.saturating_sub(1);
    if f.item_use.hammer_tics == 120 {
        f.items.request(ItemRequest::HammerWarning);
    }
    if f.item_use.hammer_tics == 0 {
        f.items.request(ItemRequest::Destroy);
        f.items.held = None;
        if is_hammer(f.status.status) {
            status::set_wait_or_fall(f);
        }
        if f.colanim.id == crate::colanim::ColAnimId::FIGHTER_HAMMER {
            crate::colanim::reset_stat_update(f);
        }
    }
}
fn hammer_update(f: &mut Fighter, mut s: Status) {
    if s == Status::HammerLanding {
        f.item_use.landing_frame += 1.0;
        // The source really uses <= 4, so the first update ends landing.
        if f.item_use.landing_frame <= 4.0 {
            hammer_wait(f);
        }
        return;
    }
    if s == Status::HammerFall {
        return;
    }
    if s == Status::HammerKneeBend {
        f.item_use.knee_frame += 1.0;
        if f.status.jump_input == JumpInput::Button
            && f.item_use.knee_frame <= 3.0
            && f.button_release().contains(
                N64Buttons::C_UP | N64Buttons::C_DOWN | N64Buttons::C_LEFT | N64Buttons::C_RIGHT,
            )
        {
            f.status.is_shorthop = true;
        }
        if f.item_use.knee_frame >= f.attributes.kneebend_anim_length {
            let (x, y) = if f.status.jump_input == JumpInput::Button {
                status::jump_force_button(f.stick.x, f.status.is_shorthop)
            } else {
                (f.stick.x as f32, f.status.jump_force as f32)
            };
            hammer_fall(f);
            f.physics.vel_air.y = y * f.attributes.jump_height_mul + f.attributes.jump_height_base;
            f.physics.vel_air.x = x * f.attributes.jump_vel_x;
            f.stick.tap_y = status::STICKBUFFER_MAX;
        } else {
            f.status.jump_force = f.status.jump_force.max(f.stick.y);
        }
        return;
    }
    if s == Status::HammerTurn {
        turn(f);
        if f.item_use.turn_left == 0 {
            hammer_wait(f);
            s = Status::HammerWait;
        }
    }
    let input = status::jump_input_type(f, status::KNEEBEND_STICK_MIN);
    if input != JumpInput::None {
        hammer_set(f, Status::HammerKneeBend);
        f.status.jump_input = input;
        f.status.jump_force = f.stick.y;
        f.status.is_shorthop = false;
        f.item_use.knee_frame = 0.0;
        return;
    }
    if s == Status::HammerTurn {
        return;
    }
    if f.stick.y as i32 <= status::PASS_STICK_MIN
        && f.stick.tap_y < status::PASS_BUFFER_TICS_MAX
        && f.floor.is_some_and(|s| s.passable())
    {
        let frame = f.status.anim_frame;
        let line = f.floor.map(|s| s.line);
        hammer_fall(f);
        // `ftCommonPassSetStatusParam` keeps HammerWalk's animation and hits.
        f.ignore_line = line;
        f.physics.vel_air.y = 0.0;
        f.stick.tap_y = status::STICKBUFFER_MAX;
        f.status.anim_frame = frame;
        if f.colanim.id != crate::colanim::ColAnimId::FIGHTER_HAMMER {
            crate::colanim::check_set(f, crate::colanim::ColAnimId::FIGHTER_HAMMER, 0);
        }
        return;
    }
    if s == Status::HammerWait {
        if f.stick.forward(f.facing) <= status::TURN_STICK_MIN {
            f.item_use.turn_left = 12;
            f.item_use.yaw = Some(core::f32::consts::FRAC_PI_2 * f.facing.sign());
            hammer_set(f, Status::HammerTurn);
            turn(f);
        } else if f.stick.forward(f.facing) >= 8 {
            hammer_set(f, Status::HammerWalk);
        }
    } else if s == Status::HammerWalk
        && (f.stick.forward(f.facing) < 0 || f.stick.x.unsigned_abs() < 8)
    {
        hammer_wait(f);
    }
}
fn turn(f: &mut Fighter) {
    if f.item_use.turn_left != 0 {
        f.item_use.turn_left -= 1;
        if f.item_use.turn_left == 6 {
            f.facing = f.facing.flipped();
        }
        f.item_use.yaw = Some(f.item_use.yaw.unwrap_or(0.0) - core::f32::consts::PI / 12.0);
    }
}
pub fn on_ground_lost(f: &mut Fighter) -> bool {
    if is_hammer(f.status.status) {
        hammer_fall(f);
        return true;
    }
    let s = if f.status.status == Status::LGunShoot {
        Status::LGunShootAir
    } else if f.status.status == Status::FireFlowerShoot {
        Status::FireFlowerShootAir
    } else {
        return false;
    };
    switch_shoot(f, s);
    true
}
fn switch_shoot(f: &mut Fighter, s: Status) {
    let speed = if matches!(s, Status::LGunShoot | Status::LGunShootAir) {
        1.0
    } else {
        f.status.timing.anim_speed
    };
    status::set_status(f, s, f.status.anim_frame, timing(f, s, speed));
    if !s.is_grounded() {
        crate::physics::clamp_air_vel_x(&mut f.physics, f.attributes.air_speed_max_x);
    }
}
pub fn on_landing(f: &mut Fighter) -> bool {
    if f.status.status == Status::HammerFall {
        if f.physics.vel_air.y > -20.0 {
            hammer_wait(f);
        } else {
            hammer_set(f, Status::HammerLanding);
            f.item_use.landing_frame = 0.0;
        }
        return true;
    }
    let s = if f.status.status == Status::LGunShootAir {
        Status::LGunShoot
    } else if f.status.status == Status::FireFlowerShootAir {
        Status::FireFlowerShoot
    } else {
        return false;
    };
    switch_shoot(f, s);
    true
}
