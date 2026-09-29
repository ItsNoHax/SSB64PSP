//! Ness's US callbacks from `ftnessspecial{n,hi,lw}.c` and
//! `ftNessJumpAerialProcPhysics`. Update precedes physics and map.
//! Map responses use the shared fighter solver; hit status (including part
//! intangibility) comes from the motion scripts. Sound
//! and effects await their shared systems.
use crate::fighter::{Facing, Fighter, FighterKind};
use crate::physics;
use crate::status::{self, AnyStatus, NessStatus as N, Status, StatusTiming};
use crate::weapon::{WeaponKind, WeaponSpawn};
use ssb_engine::input::N64Buttons;
use ssb_engine::math::{atan2, sin_cos, Vec2, Vec3};

#[cfg(test)]
#[path = "ness_tests.rs"]
mod tests;

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct NessState {
    pub spawned: bool,
    pub launch_delay: u8,
    pub end_delay: u8,
    pub gravity_delay: u8,
    pub thunder_motion: u16,
    pub thunder_position: Option<Vec3>,
    pub thunder_destroyed: bool,
    pub thunder_collide: bool,
    pub blast_frames: u8,
    pub blast_angle: f32,
    pub release_lag: u8,
    pub released: bool,
    pub jump_velocity_x: f32,
    pub dtilt_requested: bool,
    /// The `efManagerNessPsychicMagnetMakeEffect` field: how many
    /// `gcPlayAnimAll` calls it has run. Presentation only; read it through
    /// [`magnet_effect_ticks`].
    pub magnet_effect: Option<u16>,
}
pub fn is_ness(k: FighterKind) -> bool {
    crate::grab::base_kind(k) == FighterKind::Ness
}
pub fn is_grounded(s: N) -> bool {
    matches!(
        s,
        N::Attack13
            | N::SpecialN
            | N::SpecialHiStart
            | N::SpecialHiHold
            | N::SpecialHiEnd
            | N::SpecialHiJibaku
            | N::SpecialLwStart
            | N::SpecialLwHold
            | N::SpecialLwHit
            | N::SpecialLwEnd
    )
}
pub fn anim_slot(s: N) -> usize {
    424 + match s {
        // The battle entry runs in `crate::appear`.
        N::AppearRStart | N::AppearLStart | N::AppearWait | N::AppearREnd | N::AppearLEnd => {
            return crate::appear::slot(crate::status::AnyStatus::Ness(s)).unwrap_or(0)
        }
        N::Attack13 => 0,
        N::SpecialN => 1,
        N::SpecialAirN => 2,
        N::SpecialHiStart => 3,
        N::SpecialHiHold => 4,
        N::SpecialHiEnd => 5,
        N::SpecialHiJibaku => 6,
        N::SpecialAirHiStart => 7,
        N::SpecialAirHiHold => 8,
        N::SpecialAirHiEnd => 9,
        N::SpecialAirHiBound => 10,
        N::SpecialAirHiJibaku => 11,
        N::SpecialLwStart => 12,
        N::SpecialLwHold => 13,
        N::SpecialLwHit => 14,
        N::SpecialLwEnd => 15,
        N::SpecialAirLwStart => 16,
        N::SpecialAirLwHold => 17,
        N::SpecialAirLwHit => 18,
        N::SpecialAirLwEnd => 19,
    }
}
fn timing(s: N) -> StatusTiming {
    match s {
        N::Attack13 => StatusTiming::frames(25.0),
        N::SpecialN => StatusTiming::frames(72.0),
        N::SpecialAirN => StatusTiming::frames(60.0),
        N::SpecialHiStart | N::SpecialAirHiStart => StatusTiming::frames(24.0),
        N::SpecialHiEnd => StatusTiming::frames(16.0),
        N::SpecialAirHiEnd => StatusTiming::frames(31.0),
        N::SpecialLwStart | N::SpecialAirLwStart => StatusTiming::frames(15.0),
        N::SpecialLwEnd | N::SpecialAirLwEnd => StatusTiming::frames(11.0),
        _ => StatusTiming::unknown(),
    }
}
fn set(f: &mut Fighter, s: N) {
    f.physics.is_fastfall = false;
    status::set_any_status(f, AnyStatus::Ness(s), 0.0, timing(s));
}
fn switch(f: &mut Fighter, s: N) {
    f.physics.is_fastfall = false;
    let frame = if matches!(s, N::SpecialHiJibaku | N::SpecialAirHiJibaku) {
        // The source skips frame-zero script initialization on blast switches.
        f.status.anim_frame.max(1.0)
    } else {
        f.status.anim_frame
    };
    status::set_any_status(f, AnyStatus::Ness(s), frame, timing(s));
}
pub fn set_special_n(f: &mut Fighter) {
    set(
        f,
        if f.is_grounded() {
            N::SpecialN
        } else {
            N::SpecialAirN
        },
    );
    f.ness.spawned = false;
}
/// Accessory flag 0 at frame 20. Kirby uses its own TopN offset.
pub(crate) fn make_pk_fire(f: &mut Fighter, copied: bool) {
    let ground = f.is_grounded();
    let mut pos = f.joint_world(0, Vec3::ZERO);
    pos.x += if copied { 240.0 } else { 100.0 } * f.facing.sign();
    pos.y += if copied { 190.0 } else { 180.0 };
    if !copied {
        pos.z = 0.0;
    }
    f.weapon_spawn = Some(WeaponSpawn {
        kind: WeaponKind::NessPKFire { grounded: ground },
        owner_port: f.port,
        team: f.team,
        position: pos,
        facing: f.facing.sign(),
        stale: crate::stale::WeaponStale::of(f),
    });
}
pub fn set_special_hi(f: &mut Fighter) {
    let ground = f.is_grounded();
    set(
        f,
        if ground {
            N::SpecialHiStart
        } else {
            N::SpecialAirHiStart
        },
    );
    f.ness.launch_delay = 30;
    f.ness.end_delay = 30;
    f.ness.gravity_delay = 25;
    f.ness.thunder_position = None;
    f.ness.thunder_destroyed = false;
    f.ness.thunder_collide = false;
    if !ground {
        f.physics.vel_air.x /= 2.0;
        f.physics.vel_air.y = 0.0;
    }
}
pub fn thunder_controlling(s: AnyStatus) -> bool {
    matches!(s, AnyStatus::Ness(N::SpecialHiHold | N::SpecialAirHiHold))
}
fn set_thunder_hold(f: &mut Fighter) {
    set(
        f,
        if f.is_grounded() {
            N::SpecialHiHold
        } else {
            N::SpecialAirHiHold
        },
    );
    f.physics.jumps_used = f.attributes.jumps_max;
    f.ness.thunder_motion = f.motion.count;
    let mut pos = f.joint_world(12, Vec3::ZERO);
    pos.z = 0.0;
    f.weapon_spawn = Some(WeaponSpawn {
        kind: WeaponKind::NessPKThunder,
        owner_port: f.port,
        team: f.team,
        position: pos,
        facing: f.facing.sign(),
        stale: crate::stale::WeaponStale::of(f),
    });
}
fn set_thunder_end(f: &mut Fighter) {
    set(
        f,
        if f.is_grounded() {
            N::SpecialHiEnd
        } else {
            N::SpecialAirHiEnd
        },
    );
}
fn down_bounce(f: &mut Fighter) {
    f.physics.vel_air = Vec3::ZERO;
    f.physics.vel_ground.x = 0.0;
    // `ftCommonDownBounceSetStatus`: face up or down from the posed pitch.
    let y = f.pos.y;
    crate::reaction::set_down_bounce(f, y);
}
pub(crate) fn map_down_bounce(f: &mut Fighter) {
    down_bounce(f);
}
pub(crate) fn map_end_blast(f: &mut Fighter) {
    set_thunder_end(f);
}

pub(crate) fn map_blast_contacts(f: &mut Fighter) {
    let contacts = f.map_contacts;
    for (hit, wall) in [
        (contacts.ceiling, false),
        (contacts.left_wall, true),
        (contacts.right_wall, true),
    ] {
        let Some(hit) = hit else {
            continue;
        };
        let v = f.physics.vel_air;
        if hit.normal.x * v.x + hit.normal.y * v.y < v.length() * -0.906_307_8 {
            let factor = -2.0 * (hit.normal.x * v.x + hit.normal.y * v.y);
            f.physics.vel_air.x = (v.x + hit.normal.x * factor) * 0.5;
            f.physics.vel_air.y = (v.y + hit.normal.y * factor) * 0.5;
            physics::clamp_air_vel_x(&mut f.physics, f.attributes.air_speed_max_x);
            f.facing = if f.physics.vel_air.x < 0.0 {
                Facing::Right
            } else {
                Facing::Left
            };
            set(f, N::SpecialAirHiBound);
        } else if wall {
            // `ftNessSpecialHiCollideWallPhysics` (active US source).
            let lr = f.facing.sign();
            let old = if lr > 0.0 {
                f.ness.blast_angle
            } else {
                core::f32::consts::PI - f.ness.blast_angle
            };
            let mut angle = 0.0;
            for (contact, left) in [(contacts.left_wall, true), (contacts.right_wall, false)] {
                if let Some(contact) = contact {
                    angle = atan2(contact.normal.y, contact.normal.x);
                    let plus = if left {
                        old + core::f32::consts::PI < angle
                    } else {
                        angle + core::f32::consts::PI < old
                    };
                    angle += if plus {
                        core::f32::consts::FRAC_PI_2
                    } else {
                        -core::f32::consts::FRAC_PI_2
                    };
                }
            }
            let (sin, cos) = sin_cos(angle - f.ness.blast_angle * lr);
            f.physics.vel_air.x = v.x * cos - v.y * sin;
            f.physics.vel_air.y = v.x * sin + v.y * cos;
            f.ness.blast_angle = atan2(f.physics.vel_air.y, f.physics.vel_air.x * lr);
        }
    }
}
fn set_blast(f: &mut Fighter, thunder: Vec3) {
    f.ness.thunder_collide = true;
    let dx = f.pos.x - thunder.x;
    let dy = f.pos.y + 150.0 - thunder.y;
    f.facing = if dx >= 0.0 {
        Facing::Right
    } else {
        Facing::Left
    };
    f.ness.blast_angle = atan2(dy, dx * f.facing.sign());
    let ground = f.is_grounded()
        && f.floor.is_some_and(|floor| {
            if floor.passable() {
                return false;
            }
            let dot = (floor.normal.x * dx + floor.normal.y * dy)
                / ssb_engine::math::sqrt(dx * dx + dy * dy);
            if dot < sin_cos(155.0 * core::f32::consts::PI / 180.0).1 {
                return true;
            }
            dot <= 0.0
        });
    if ground {
        let n = f.floor.unwrap().normal;
        if (n.x * dx + n.y * dy)
            < ssb_engine::math::sqrt(dx * dx + dy * dy)
                * sin_cos(155.0 * core::f32::consts::PI / 180.0).1
        {
            down_bounce(f);
            return;
        }
        set(f, N::SpecialHiJibaku);
        f.physics.vel_ground.x = 200.0 * f.facing.sign();
    } else {
        f.become_airborne();
        set(f, N::SpecialAirHiJibaku);
        let (sin, cos) = sin_cos(f.ness.blast_angle);
        f.physics.vel_air = Vec3::new(cos * 200.0 * f.facing.sign(), sin * 200.0, 0.0);
    }
    f.ness.blast_frames = 28;
    f.physics.jumps_used = f.attributes.jumps_max;
}
pub fn set_special_lw(f: &mut Fighter) {
    let ground = f.is_grounded();
    set(
        f,
        if ground {
            N::SpecialLwStart
        } else {
            N::SpecialAirLwStart
        },
    );
    f.ness.release_lag = 30;
    f.ness.released = false;
    f.ness.gravity_delay = 4;
    // `FTSTATUS_PRESERVE_NONE` stops any earlier field.
    f.ness.magnet_effect = None;
    if !ground {
        f.physics.vel_air.x /= 2.0;
        f.physics.vel_air.y = 0.0;
    }
}
/// The PSI Magnet field's played animation frames, while it exists. The
/// Hold and Hit statuses and their ground/air switches pass
/// `FTSTATUS_PRESERVE_EFFECT`; `SpecialLwEnd` does not.
pub fn magnet_effect_ticks(f: &Fighter) -> Option<u16> {
    f.ness.magnet_effect.filter(|_| absorbing(f))
}

/// `ftNessSpecialLwInitVars`, from both Hold setters: the field is made
/// once, and plays its animation once when it is made.
fn make_magnet_effect(f: &mut Fighter) {
    if f.ness.magnet_effect.is_none() {
        f.ness.magnet_effect = Some(1);
    }
}

pub fn absorbing(f: &Fighter) -> bool {
    matches!(
        f.status.status,
        AnyStatus::Ness(
            N::SpecialLwHold | N::SpecialAirLwHold | N::SpecialLwHit | N::SpecialAirLwHit
        )
    )
}
/// `ftNessSpecialLwProcAbsorb`, from `ftMainProcParams` after PSI Magnet
/// took a weapon: face the weapon's side and play the hit.
pub fn proc_absorb(f: &mut Fighter, absorb_lr: f32) {
    f.facing = if absorb_lr > 0.0 {
        Facing::Right
    } else {
        Facing::Left
    };
    set(
        f,
        if f.is_grounded() {
            N::SpecialLwHit
        } else {
            N::SpecialAirLwHit
        },
    );
}
pub fn update(f: &mut Fighter) {
    let AnyStatus::Ness(s) = f.status.status else {
        return;
    };
    // The field's own `gcPlayAnimAll`; nothing pauses it.
    if magnet_effect_ticks(f).is_some() {
        f.ness.magnet_effect = f.ness.magnet_effect.map(|t| t.saturating_add(1));
    }
    match s {
        N::SpecialHiStart | N::SpecialAirHiStart => {
            if f.status.animation_ended() {
                set_thunder_hold(f);
            }
        }
        N::SpecialHiHold | N::SpecialAirHiHold => {
            f.ness.launch_delay = f.ness.launch_delay.saturating_sub(1);
            if f.ness.thunder_destroyed {
                f.ness.end_delay = f.ness.end_delay.saturating_sub(1);
            }
            if f.ness.launch_delay == 0 {
                if f.ness.thunder_destroyed && f.ness.end_delay == 0 {
                    set_thunder_end(f);
                } else if let Some(p) = f.ness.thunder_position {
                    if !f.ness.thunder_destroyed
                        && (f.pos.x - p.x).abs() < 250.0
                        && (f.pos.y + 150.0 - p.y).abs() < 370.0
                    {
                        set_blast(f, p);
                    }
                }
            }
        }
        N::SpecialHiJibaku | N::SpecialAirHiJibaku => {
            f.ness.blast_frames = f.ness.blast_frames.saturating_sub(1);
            if f.ness.blast_frames == 0 {
                set_thunder_end(f);
            }
        }
        N::SpecialLwStart | N::SpecialAirLwStart => {
            if f.status.animation_ended() {
                set(
                    f,
                    if is_grounded(s) {
                        N::SpecialLwHold
                    } else {
                        N::SpecialAirLwHold
                    },
                );
                make_magnet_effect(f);
            }
        }
        N::SpecialLwHold | N::SpecialAirLwHold => {
            if !f.input.buttons.contains(N64Buttons::B) {
                f.ness.released = true;
            }
            f.ness.release_lag = f.ness.release_lag.saturating_sub(1);
            if f.ness.release_lag == 0 && f.ness.released {
                set(
                    f,
                    if is_grounded(s) {
                        N::SpecialLwEnd
                    } else {
                        N::SpecialAirLwEnd
                    },
                );
            }
        }
        N::SpecialLwHit | N::SpecialAirLwHit => {
            if f.status.anim_frame >= 14.0 {
                set(
                    f,
                    if is_grounded(s) {
                        N::SpecialLwHold
                    } else {
                        N::SpecialAirLwHold
                    },
                );
                make_magnet_effect(f);
            }
        }
        N::SpecialAirHiEnd | N::SpecialAirHiBound => {
            if f.status.animation_ended() {
                status::set_fall_special(f, 0.6, false, true, 0.17, false);
            }
        }
        N::SpecialN | N::SpecialAirN => {
            if !f.ness.spawned && f.status.anim_frame >= 20.0 {
                f.ness.spawned = true;
                make_pk_fire(f, false);
            }
            if f.status.animation_ended() {
                status::set_wait_or_fall(f);
            }
        }
        _ => {
            if f.status.animation_ended() {
                status::set_wait_or_fall(f);
            }
        }
    }
}
/// Ness's `DTilt` sets flag 1 at frame 11. Update uses the prior request;
/// interrupt then tests this frame's A tap, as in `ftcommonattacklw3.c`.
pub(crate) fn update_dtilt(f: &mut Fighter) {
    let flag = f.status.anim_frame >= 11.0;
    if flag && f.ness.dtilt_requested {
        status::set_dtilt(f);
        return;
    }
    if f.status.animation_ended() {
        status::set_status(f, Status::SquatWait, 0.0, StatusTiming::unknown());
        f.is_special_interrupt = true;
    } else if f.button_tap().contains(N64Buttons::A) {
        if flag {
            status::set_dtilt(f);
        } else {
            f.ness.dtilt_requested = true;
        }
    }
}
fn special_slow_air(f: &mut Fighter, gravity: f32) {
    if f.ness.gravity_delay != 0 {
        f.ness.gravity_delay -= 1;
    } else {
        physics::apply_gravity_clamp_tvel(&mut f.physics, gravity, f.attributes.tvel_base);
    }
    if !physics::check_clamp_air_vel_x_dec(&mut f.physics, f.attributes.air_speed_max_x) {
        physics::apply_air_friction(&mut f.physics, &f.attributes);
    }
}
pub fn apply_ground_physics(f: &mut Fighter) -> bool {
    let AnyStatus::Ness(s) = f.status.status else {
        return false;
    };
    match s {
        N::SpecialHiJibaku => {
            if f.physics.vel_ground.x * f.facing.sign() > 0.0 {
                f.physics.vel_ground.x -= (43.0 / 7.0) * f.facing.sign();
            }
            let normal = f.floor.map_or(Vec2::new(0.0, 1.0), |x| x.normal);
            f.physics.vel_air = Vec3::new(
                f.physics.vel_ground.x * normal.y,
                -f.physics.vel_ground.x * normal.x,
                0.0,
            );
            true
        }
        N::SpecialHiStart
        | N::SpecialHiHold
        | N::SpecialHiEnd
        | N::SpecialLwStart
        | N::SpecialLwHold
        | N::SpecialLwHit
        | N::SpecialLwEnd => {
            f.ness.gravity_delay = f.ness.gravity_delay.saturating_sub(1);
            if matches!(s, N::SpecialHiStart | N::SpecialHiHold | N::SpecialHiEnd) {
                let material = f.floor.map_or(1.0, |floor| {
                    crate::collision::material_friction(floor.flags)
                });
                physics::apply_ground_friction_with_material(
                    &mut f.physics,
                    &f.attributes,
                    material,
                );
            }
            true
        }
        _ => false,
    }
}
pub fn apply_air_physics(f: &mut Fighter) -> bool {
    if is_ness(f.kind)
        && matches!(
            f.status.status,
            AnyStatus::Common(Status::JumpAerialF | Status::JumpAerialB)
        )
    {
        let mut transn = f.physics;
        physics::apply_air_vel_transn_all(&mut transn, f.root_motion, f.facing.sign());
        f.physics.vel_air.x = f.ness.jump_velocity_x;
        f.physics.vel_air.y = transn.vel_air.y;
        f.physics.vel_air.z = transn.vel_air.z;
        if !physics::check_clamp_air_vel_x_dec(&mut f.physics, f.attributes.air_speed_max_x) {
            physics::clamp_air_vel_x_stick_range(
                &mut f.physics,
                f.input.stick_x,
                physics::AIRDRIFT_STICK_MIN,
                f.attributes.air_accel,
                f.attributes.air_speed_max_x,
            );
            physics::apply_air_friction(&mut f.physics, &f.attributes);
        }
        f.ness.jump_velocity_x = f.physics.vel_air.x;
        f.physics.vel_air.x += transn.vel_air.x;
        return true;
    }
    let AnyStatus::Ness(s) = f.status.status else {
        return false;
    };
    match s {
        N::SpecialAirHiStart | N::SpecialAirHiHold | N::SpecialAirHiEnd => special_slow_air(f, 0.5),
        N::SpecialAirLwStart | N::SpecialAirLwHold | N::SpecialAirLwHit | N::SpecialAirLwEnd => {
            special_slow_air(f, 0.8)
        }
        N::SpecialAirHiJibaku => {
            let old = f.physics.vel_air;
            let (sin, cos) = sin_cos(f.ness.blast_angle);
            f.physics.vel_air.x -= (43.0 / 7.0) * cos * f.facing.sign();
            f.physics.vel_air.y -= (43.0 / 7.0) * sin;
            if f.physics.vel_air.x.abs() > old.x.abs() {
                f.physics.vel_air.x = old.x;
            }
            if f.physics.vel_air.y.abs() > old.y.abs() {
                f.physics.vel_air.y = old.y;
            }
        }
        _ => {
            physics::apply_gravity_default(&mut f.physics, &f.attributes);
            if !physics::check_clamp_air_vel_x_dec(&mut f.physics, f.attributes.air_speed_max_x) {
                physics::apply_air_friction(&mut f.physics, &f.attributes);
            }
        }
    }
    true
}
pub fn skips_fast_fall(f: &Fighter) -> bool {
    matches!(f.status.status, AnyStatus::Ness(_))
        || is_ness(f.kind)
            && matches!(
                f.status.status,
                AnyStatus::Common(Status::JumpAerialF | Status::JumpAerialB)
            )
}
pub fn skip_pass(f: &Fighter) -> bool {
    f.status.status == AnyStatus::Ness(N::SpecialAirHiJibaku) && f.ness.blast_frames > 25
}
pub fn on_ground_lost(f: &mut Fighter) -> bool {
    let AnyStatus::Ness(s) = f.status.status else {
        return false;
    };
    let air = match s {
        N::SpecialN => N::SpecialAirN,
        N::SpecialHiStart => N::SpecialAirHiStart,
        N::SpecialHiHold => N::SpecialAirHiHold,
        N::SpecialHiEnd => N::SpecialAirHiEnd,
        N::SpecialHiJibaku => N::SpecialAirHiJibaku,
        N::SpecialLwStart => N::SpecialAirLwStart,
        N::SpecialLwHold => N::SpecialAirLwHold,
        N::SpecialLwHit => N::SpecialAirLwHit,
        N::SpecialLwEnd => N::SpecialAirLwEnd,
        _ => return false,
    };
    f.become_airborne();
    switch(f, air);
    if air == N::SpecialAirHiJibaku {
        f.ness.blast_angle = atan2(f.physics.vel_air.y, f.physics.vel_air.x * f.facing.sign());
        f.physics.jumps_used = f.attributes.jumps_max;
    } else {
        physics::clamp_air_vel_x(&mut f.physics, f.attributes.air_speed_max_x);
    }
    true
}
pub fn on_landing(f: &mut Fighter, y: f32, normal: Vec2) -> bool {
    let AnyStatus::Ness(s) = f.status.status else {
        return false;
    };
    let ground = match s {
        N::SpecialAirN => N::SpecialN,
        N::SpecialAirHiStart => N::SpecialHiStart,
        N::SpecialAirHiHold => N::SpecialHiHold,
        N::SpecialAirHiEnd => N::SpecialHiEnd,
        N::SpecialAirHiJibaku => N::SpecialHiJibaku,
        N::SpecialAirLwStart => N::SpecialLwStart,
        N::SpecialAirLwHold => N::SpecialLwHold,
        N::SpecialAirLwHit => N::SpecialLwHit,
        N::SpecialAirLwEnd => N::SpecialLwEnd,
        N::SpecialAirHiBound => {
            f.land(y);
            down_bounce(f);
            return true;
        }
        _ => return false,
    };
    let velocity = f.physics.vel_air;
    f.land(y);
    if s == N::SpecialAirHiJibaku
        && normal.x * velocity.x + normal.y * velocity.y
            < velocity.length() * sin_cos(155.0 * core::f32::consts::PI / 180.0).1
    {
        down_bounce(f);
    } else {
        switch(f, ground);
    }
    true
}
