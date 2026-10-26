//! Ness's US callbacks from `ftnessspecial{n,hi,lw}.c` and
//! `ftNessJumpAerialProcPhysics`. Update precedes physics and map.
//! Map responses use the shared fighter solver; hit status (including part
//! intangibility) comes from the motion scripts. Sound
//! and effects await their shared systems.
use crate::fighter::{Facing, Fighter, FighterKind};
use crate::physics;
use crate::spawn_pose::SpawnPose;
use crate::status::{self, AnyStatus, NessStatus as N, Status, StatusTiming};
use crate::weapon::{WeaponKind, WeaponSpawn};
use ssb_engine::input::N64Buttons;
use ssb_engine::math::{sin_cos, sy_arc_tan2, Vec2, Vec3};

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
    /// The `efManagerNessPKThunderWaveMakeEffect` field: how many
    /// `gcPlayAnimAll` calls it has run. Presentation only; read it through
    /// [`pk_wave_effect_ticks`].
    pub pk_wave_effect: Option<u16>,
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
/// The `ftMainSetStatus` flags the port reads, for a setter (`is_switch`
/// false) or a ground/air switch (`ftnessspecial*.c`).
///
/// - `FTSTATUS_PRESERVE_LOOPSFX`: `ftNessSpecial(Air)HiHoldSetStatus` and
///   `ftNessSpecial(Air)LwHoldSetStatus`; of the switches, Hi Hold's and Lw
///   Hit's (`FTNESS_SPECIALHIHOLD_STATUS_FLAGS`,
///   `FTNESS_SPECIALLWHIT_STATUS_FLAGS`).
/// - `FTSTATUS_PRESERVE_HITSTATUS`: every switch; no setter.
/// - `FTSTATUS_PRESERVE_HIT`: the Hi Hold, Jibaku, Lw Hold and Lw Hit
///   switches (`..._SPECIALHIHOLD_`, `..._SPECIALHI_`, `..._SPECIALLWHOLD_`,
///   `..._SPECIALLWHIT_STATUS_FLAGS`). The switch's script replay skips the
///   attack events (`ftMainUpdateMotionEventsForward`), so without it a
///   landing PK Thunder blast loses its hitbox.
///
/// `TEXTUREPART`, `EFFECT` and `COLANIM` come from the
/// [`crate::colanim::PRESERVE_COLANIM`] tables, `RUMBLE` from
/// [`crate::rumble::preserved`].
fn preserve(s: N, is_switch: bool) -> status::Preserve {
    if !is_switch {
        return status::Preserve {
            loop_sfx: matches!(
                s,
                N::SpecialHiHold | N::SpecialAirHiHold | N::SpecialLwHold | N::SpecialAirLwHold
            ),
            ..status::Preserve::NONE
        };
    }
    status::Preserve {
        loop_sfx: matches!(
            s,
            N::SpecialHiHold | N::SpecialAirHiHold | N::SpecialLwHit | N::SpecialAirLwHit
        ),
        hitstatus: true,
        hit: matches!(
            s,
            N::SpecialHiHold
                | N::SpecialAirHiHold
                | N::SpecialHiJibaku
                | N::SpecialAirHiJibaku
                | N::SpecialLwHold
                | N::SpecialAirLwHold
                | N::SpecialLwHit
                | N::SpecialAirLwHit
        ),
        ..status::Preserve::NONE
    }
}
fn set(f: &mut Fighter, s: N) {
    f.physics.is_fastfall = false;
    status::set_any_status_preserve(f, AnyStatus::Ness(s), 0.0, timing(s), preserve(s, false));
    // Every one of Ness's special setters plays the first frame
    // (`ftnessspecial*.c`; RE-474's N64 Training traces).
    status::play_anim_events(f);
}
fn switch(f: &mut Fighter, s: N) {
    f.physics.is_fastfall = false;
    let frame = if matches!(s, N::SpecialHiJibaku | N::SpecialAirHiJibaku) {
        // The source skips frame-zero script initialization on blast switches.
        f.status.anim_frame.max(1.0)
    } else {
        f.status.anim_frame
    };
    status::set_any_status_preserve(f, AnyStatus::Ness(s), frame, timing(s), preserve(s, true));
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
    // `ftNessSpecialNProcAccessory` / `ftKirbyCopyNessSpecialNProcAccessory`:
    // TopN after the map step, then the world offset.
    let pose = if copied {
        SpawnPose::accessory(0, Vec3::ZERO).with_world(Vec3::new(240.0, 190.0, 0.0))
    } else {
        SpawnPose::accessory(0, Vec3::ZERO)
            .with_world(Vec3::new(100.0, 180.0, 0.0))
            .zero_z()
    };
    f.queue_weapon_spawn_posed(
        WeaponSpawn {
            kind: WeaponKind::NessPKFire { grounded: ground },
            owner_port: f.port,
            team: f.team,
            position: f.pos,
            facing: f.facing.sign(),
            stale: crate::stale::WeaponStale::of(f),
        },
        pose,
    );
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
    // `FTSTATUS_PRESERVE_NONE` stops any earlier wave.
    f.ness.pk_wave_effect = None;
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
    f.ness.thunder_motion = f.motion.count;
    // `ftNessSpecialHiHoldInitStatusVars`, after the Hold's set and play:
    // joint 12, `pos.z = 0`.
    f.queue_weapon_spawn_posed(
        WeaponSpawn {
            kind: WeaponKind::NessPKThunder,
            owner_port: f.port,
            team: f.team,
            position: f.pos,
            facing: f.facing.sign(),
            stale: crate::stale::WeaponStale::of(f),
        },
        SpawnPose::update(12, Vec3::ZERO).zero_z(),
    );
    // `ftNessSpecialHiHoldInitStatusVars`, after `ftNessSpecialHiMakePKThunder`:
    // the setter's `FTSTATUS_PRESERVE_LOOPSFX` stopped any attached effect,
    // so the wave is made unless the first frame's events attached one.
    if !f.is_effect_attach {
        make_pk_wave_effect(f);
        f.is_effect_attach = true;
    }
    f.physics.jumps_used = f.attributes.jumps_max;
}

/// The PK Thunder wave's played animation frames, while it exists. Only
/// the Hold ground/air switches pass `FTSTATUS_PRESERVE_EFFECT`
/// (`FTNESS_SPECIALHIHOLD_STATUS_FLAGS`); `SpecialHiEnd`, the Jibaku
/// statuses and every other status stop it.
pub fn pk_wave_effect_ticks(f: &Fighter) -> Option<u16> {
    f.ness
        .pk_wave_effect
        .filter(|_| thunder_controlling(f.status.status))
}

/// `efManagerNessPKThunderWaveMakeEffect`: `efManagerMakeEffectNoForce` of
/// `dEFManagerNessPKThunderWaveEffectDesc` plays the animation once when it
/// is made; `user_data` is joint [`PK_WAVE_JOINT`], `rotate.y` is
/// `lr * 90` degrees and `translate.z` is 0.
fn make_pk_wave_effect(f: &mut Fighter) {
    f.ness.pk_wave_effect = Some(1);
}

/// `fp->joints[5]`, the wave's `user_data`.
pub const PK_WAVE_JOINT: u8 = 5;

/// The wave's own process is bare `gcPlayAnimAll`, which never reads
/// `is_pause_effect`: the wave plays through Ness's hitlag.
/// [`Fighter`]'s update calls this in hitlag too.
pub fn tick_pk_wave_effect(f: &mut Fighter) {
    if pk_wave_effect_ticks(f).is_some() {
        f.ness.pk_wave_effect = f.ness.pk_wave_effect.map(|t| t.saturating_add(1));
    }
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
    // The bound's impact wave: TopN's `x, y` (z 0), moved to each struck
    // side of the collision diamond in turn, as the source moves one `pos`.
    let mut pos = Vec3::new(f.pos.x, f.pos.y, 0.0);
    for (hit, side) in [
        (contacts.ceiling, 0.0),
        (contacts.left_wall, 1.0),
        (contacts.right_wall, -1.0),
    ] {
        let wall = side != 0.0;
        // A flag and a defined contact: the hardware trapped on this loop
        // speculating the normal's compare onto a `None` (RE-531).
        let (found, hit) = crate::map::contact_or_none(hit);
        if !found {
            continue;
        }
        let v = f.physics.vel_air;
        if jibaku_halts(hit.normal, v) {
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
            if wall {
                pos.x += side * f.coll.width;
                pos.y += f.coll.center;
            } else {
                pos.y += f.coll.top;
            }
            crate::fteffect::surface_impact(f, pos, hit.normal, false);
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
                let (found, contact) = crate::map::contact_or_none(contact);
                if found {
                    angle = sy_arc_tan2(contact.normal.y, contact.normal.x);
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
            f.ness.blast_angle = sy_arc_tan2(f.physics.vel_air.y, f.physics.vel_air.x * lr);
        }
    }
}
/// `FTNESS_PKJIBAKU_HALT_ANGLE` (`ftness.h`): `F_CLC_DTOR32(155.0F)`.
const PKJIBAKU_HALT_ANGLE: f32 = ssb_engine::math::f_clc_dtor32(155.0);

/// `ftNessSpecialAirHiJibakuProcMap`'s halt test (`ftnessspecialhi.c:626-668`):
/// `syVectorAngleDiff3D(surface_angle, vel_air) > FTNESS_PKJIBAKU_HALT_ANGLE`.
pub(crate) fn jibaku_halts(normal: Vec2, vel_air: Vec3) -> bool {
    ssb_engine::math::sy_vector_angle_diff_3d(Vec3::new(normal.x, normal.y, 0.0), vel_air)
        > PKJIBAKU_HALT_ANGLE
}

fn set_blast(f: &mut Fighter, thunder: Vec3) {
    f.ness.thunder_collide = true;
    let dx = f.pos.x - thunder.x;
    let dy = f.pos.y + 150.0 - thunder.y;
    let facing = if dx >= 0.0 {
        Facing::Right
    } else {
        Facing::Left
    };
    // `ftNessSpecialHiJibakuSetStatus` (`ftnessspecialhi.c:731`): a solid
    // floor, and the blast at least `F_CST_DTOR32(90)` from its normal, keeps
    // Ness grounded; past `F_CST_DTOR32(155)` he bounces down. A flag and a
    // defined floor: the gate must never read a `None`'s payload (RE-531).
    let (on_floor, floor) = ssb_engine::math::defined_or(f.floor, crate::ground::Standing::NONE);
    let angle_diff = ssb_engine::math::sy_vector_angle_diff_3d(
        Vec3::new(floor.normal.x, floor.normal.y, 0.0),
        Vec3::new(dx, dy, 0.0),
    );
    let ground = f.is_grounded()
        && on_floor
        && !floor.passable()
        && angle_diff >= ssb_engine::math::f_cst_dtor32(90.0);
    if ground && angle_diff > ssb_engine::math::f_cst_dtor32(155.0) {
        // `goto setdown` comes before the `lr` write, and a grounded Ness
        // keeps his ground velocity: `ftCommonDownBounceSetStatus` alone.
        crate::reaction::set_down_bounce(f, f.pos.y);
        return;
    }
    // Both setters write `lr` from the blast's side before the angle.
    f.facing = facing;
    f.ness.blast_angle = sy_arc_tan2(dy, dx * f.facing.sign());
    if ground {
        // `ftNessSpecialHiJibakuInitStatusVars` pitches the model before
        // `ftMainSetStatus`, whose figatree reset drops it again.
        crate::model_pitch::special_hi(f);
        set(f, N::SpecialHiJibaku);
        f.physics.vel_ground.x = 200.0 * f.facing.sign();
    } else {
        f.become_airborne();
        let (sin, cos) = sin_cos(f.ness.blast_angle);
        f.physics.vel_air = Vec3::new(cos * 200.0 * f.facing.sign(), sin * 200.0, 0.0);
        // Dropped the same way (`ftNessSpecialAirHiJibakuSetStatus`); the
        // pitch drawn is `ftNessSpecialAirHiJibakuProcPhysics`'s.
        crate::model_pitch::special_hi(f);
        set(f, N::SpecialAirHiJibaku);
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

/// The field's own process: `dEFManagerNessPsychicMagnetEffectDesc` runs
/// bare `gcPlayAnimAll`, which never reads `is_pause_effect`, and
/// `ftnessspeciallw.c` sets no `proc_lagstart`. The field plays through
/// Ness's hitlag: [`Fighter`]'s update calls this in hitlag too.
pub fn tick_magnet_effect(f: &mut Fighter) {
    if magnet_effect_ticks(f).is_some() {
        f.ness.magnet_effect = f.ness.magnet_effect.map(|t| t.saturating_add(1));
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
    // `lr = absorb_lr` alone: a grounded magnet's world ground velocity
    // (the hold's `ftPhysicsApplyGroundVelFriction`) turns with Ness.
    let facing = if absorb_lr > 0.0 {
        Facing::Right
    } else {
        Facing::Left
    };
    status::set_lr(f, facing);
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
    tick_magnet_effect(f);
    tick_pk_wave_effect(f);
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
            if f.status.clock >= 14.0 {
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
                status::set_fall_special(f, 0.6, true, true, 0.17, false);
            }
        }
        N::SpecialN | N::SpecialAirN => {
            if !f.ness.spawned && f.status.clock >= 20.0 {
                f.ness.spawned = true;
                make_pk_fire(f, false);
            }
            if f.status.animation_ended() {
                status::anim_end_set_wait_or_fall(f);
            }
        }
        _ => {
            if f.status.animation_ended() {
                status::anim_end_set_wait_or_fall(f);
            }
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
        // `ftNessSpecialLwProcPhysics`: the delay alone; with no ground
        // transfer TopN steps by the `vel_air` left ([`keeps_vel_air`]).
        N::SpecialLwStart => {
            f.ness.gravity_delay = f.ness.gravity_delay.saturating_sub(1);
            true
        }
        // `ftNessSpecialHiProcPhysics` (the delay, then friction) and the
        // generic `ftPhysicsApplyGroundVelFriction` of the hold, hit and
        // end statuses (`ftnessstatus.h`), which leave the delay alone.
        N::SpecialHiStart
        | N::SpecialHiHold
        | N::SpecialHiEnd
        | N::SpecialLwHold
        | N::SpecialLwHit
        | N::SpecialLwEnd => {
            if s == N::SpecialHiStart {
                f.ness.gravity_delay = f.ness.gravity_delay.saturating_sub(1);
            }
            let material = f.floor.map_or(1.0, |floor| {
                crate::collision::material_friction(floor.flags)
            });
            physics::apply_ground_friction_with_material(
                &mut f.physics,
                &f.attributes,
                material,
                f.facing.sign(),
            );
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
        physics::apply_air_vel_transn_all(
            &mut transn,
            f.root_motion,
            f.facing.sign(),
            f.attributes.size,
        );
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
        // `SpecialAirN` and `SpecialAirHiBound`: `ftPhysicsApplyAirVelFriction`,
        // which keeps a fast fall already under way.
        _ => physics::apply_air_vel_friction(&mut f.physics, &f.attributes),
    }
    true
}
/// Grounded PSI Magnet's start: `ftNessSpecialLwProcPhysics` does no
/// `ftPhysicsSetGroundVelTransferAir`, so TopN steps by the `vel_air` the
/// last transfer (or the landing) left.
pub fn keeps_vel_air(f: &Fighter) -> bool {
    f.status.status == AnyStatus::Ness(N::SpecialLwStart)
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
        f.ness.blast_angle =
            sy_arc_tan2(f.physics.vel_air.y, f.physics.vel_air.x * f.facing.sign());
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
    if s == N::SpecialAirHiJibaku && jibaku_halts(normal, velocity) {
        down_bounce(f);
    } else {
        switch(f, ground);
    }
    true
}
