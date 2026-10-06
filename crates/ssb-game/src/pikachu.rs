//! Pikachu's specials (`ftpikachuspecial{n,hi,lw}.c`, US).
//! Quick Attack's wall/ceiling cancellation consumes the shared map normals.
use crate::fighter::{Facing, Fighter, FighterKind};
use crate::physics;
use crate::status::{self, AnyStatus, PikachuStatus as P, StatusTiming};
use crate::weapon::{WeaponKind, WeaponSpawn};
use ssb_engine::math::{atan2, sin_cos, Vec2, Vec3};

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct PikachuState {
    pub spawned: bool,
    pub zip_frames: i32,
    pub subsequent_zip: bool,
    pub zip_stick: Vec2,
    pub pass_timer: u8,
    pub zip_flag1: u8,
    /// Stage-owned `MPGroundData.map_bound_top`; supplied by the match.
    pub map_bound_top: Option<f32>,
    /// Weapon pool supplies the live Thunder head after its map callback.
    pub thunder_position: Option<Vec3>,
    pub thunder_destroyed: bool,
    pub thunder_collide: bool,
    /// The active Loop/Hit damage callback marks its head Destroy.
    pub thunder_damage: bool,
    pub thunder_motion: u16,
}

pub fn is_pikachu(kind: FighterKind) -> bool {
    crate::grab::base_kind(kind) == FighterKind::Pikachu
}
pub fn is_grounded(s: P) -> bool {
    matches!(
        s,
        P::SpecialN
            | P::SpecialLwStart
            | P::SpecialLwLoop
            | P::SpecialLwHit
            | P::SpecialLwEnd
            | P::SpecialHiStart
            | P::SpecialHi
            | P::SpecialHiEnd
    )
}
pub fn anim_slot(s: P) -> usize {
    359 + match s {
        // The battle entry runs in `crate::appear`.
        P::AppearR | P::AppearL => {
            return crate::appear::slot(crate::status::AnyStatus::Pikachu(s)).unwrap_or(0)
        }
        P::SpecialN => 0,
        P::SpecialAirN => 1,
        P::SpecialLwStart => 2,
        P::SpecialLwLoop => 3,
        P::SpecialLwHit => 4,
        P::SpecialLwEnd => 5,
        P::SpecialAirLwStart => 6,
        P::SpecialAirLwLoop => 7,
        P::SpecialAirLwHit => 8,
        P::SpecialAirLwEnd => 9,
        P::SpecialHiStart | P::SpecialAirHiStart => 10,
        P::SpecialHi => 10,
        P::SpecialHiEnd => 11,
        P::SpecialAirHi => 12,
        P::SpecialAirHiEnd => 13,
    }
}
fn length(s: P) -> f32 {
    match s {
        P::SpecialN | P::SpecialAirN => 64.0,
        P::SpecialLwStart | P::SpecialAirLwStart => 24.0,
        P::SpecialLwLoop | P::SpecialAirLwLoop | P::SpecialLwHit | P::SpecialAirLwHit => 60.0,
        P::SpecialLwEnd | P::SpecialAirLwEnd => 38.0,
        _ => 46.0,
    }
}
fn set(f: &mut Fighter, s: P, frame: f32) {
    let mut timing = StatusTiming::frames(length(s));
    if matches!(
        s,
        P::SpecialHiStart | P::SpecialAirHiStart | P::SpecialHi | P::SpecialAirHi
    ) {
        timing.anim_speed = 0.0;
    }
    status::set_any_status(f, AnyStatus::Pikachu(s), frame, timing);
}
/// A ground/air switch: every one of Pikachu's special switches keeps the
/// hit status (`FTSTATUS_PRESERVE_HITSTATUS`, `ftpikachuspecial*.c`) and
/// the colour animation ([`crate::colanim::preserved`]).
fn switch(f: &mut Fighter, s: P, frame: f32) {
    let mut timing = StatusTiming::frames(length(s));
    if matches!(
        s,
        P::SpecialHiStart | P::SpecialAirHiStart | P::SpecialHi | P::SpecialAirHi
    ) {
        timing.anim_speed = 0.0;
    }
    status::set_any_status_preserve(
        f,
        AnyStatus::Pikachu(s),
        frame,
        timing,
        status::Preserve::HITSTATUS,
    );
}
pub fn set_special_n(f: &mut Fighter) {
    set(
        f,
        if f.is_grounded() {
            P::SpecialN
        } else {
            P::SpecialAirN
        },
        0.0,
    );
    f.pikachu.spawned = false;
}
pub fn set_special_lw(f: &mut Fighter) {
    set(
        f,
        if f.is_grounded() {
            P::SpecialLwStart
        } else {
            P::SpecialAirLwStart
        },
        0.0,
    );
    f.pikachu.spawned = false;
    f.pikachu.thunder_position = None;
    f.pikachu.thunder_destroyed = false;
    f.pikachu.thunder_collide = false;
    f.pikachu.thunder_damage = false;
}

pub fn on_damage(f: &mut Fighter) {
    if thunder_controlling(f.status.status) {
        f.pikachu.thunder_damage = true;
    }
}
pub fn set_special_hi(f: &mut Fighter) {
    f.pikachu.zip_frames = 20;
    f.pikachu.subsequent_zip = false;
    f.pikachu.pass_timer = 0;
    f.pikachu.zip_flag1 = 0;
    f.physics.vel_air.x = 0.0;
    f.physics.vel_air.y = 0.0;
    f.physics.vel_ground.x = 0.0;
    set(
        f,
        if f.is_grounded() {
            P::SpecialHiStart
        } else {
            P::SpecialAirHiStart
        },
        0.0,
    );
    // `ftPikachuSpecialHiInitMiscVars`.
    crate::hurtbox::set_hit_status_all(f, crate::combat::HitStatus::Intangible);
    crate::colanim::check_set(
        f,
        crate::colanim::ColAnimId::FIGHTER_PIKACHU_SPECIAL_HI_START,
        0,
    );
}
fn set_stick_lr(f: &mut Fighter) {
    f.facing = if f.stick.x >= 0 {
        Facing::Right
    } else {
        Facing::Left
    };
}
fn set_zip(f: &mut Fighter) {
    let stick = Vec2::new(f.stick.x as f32, f.stick.y as f32);
    let magnitude = stick.length().min(80.0);
    let ground = f.is_grounded()
        && magnitude >= 60.0
        && f.floor
            .is_some_and(|s| !s.passable() && s.normal.x * stick.x + s.normal.y * stick.y <= 0.0);
    set_stick_lr(f);
    f.pikachu.zip_frames = 5;
    f.physics.jumps_used = f.attributes.jumps_max;
    let mul = if f.pikachu.subsequent_zip { 0.9 } else { 1.0 };
    if ground {
        f.pikachu.zip_stick = stick;
        f.physics.vel_ground.x = (3.0 * magnitude + 90.0) * mul * f.facing.sign();
        set(f, P::SpecialHi, 0.0);
    } else {
        f.become_airborne();
        let (direction, magnitude) = if magnitude > 60.0 {
            (stick, magnitude)
        } else {
            (Vec2::new(0.0, 80.0), 80.0)
        };
        f.pikachu.zip_stick = direction;
        let angle = atan2(direction.y, direction.x * f.facing.sign());
        let (sin, cos) = sin_cos(angle);
        let speed = (3.0 * magnitude + 90.0) * mul;
        f.physics.vel_air.x = cos * speed * f.facing.sign();
        f.physics.vel_air.y = sin * speed;
        set(f, P::SpecialAirHi, 0.0);
    }
}
fn set_zip_end(f: &mut Fighter) {
    f.pikachu.zip_flag1 = 0;
    let s = if f.is_grounded() {
        f.physics.vel_ground.x *= 0.2;
        f.physics.vel_air.x = 0.0;
        f.physics.vel_air.y = 0.0;
        P::SpecialHiEnd
    } else {
        f.physics.vel_air.x *= 0.2;
        f.physics.vel_air.y *= 0.2;
        f.physics.vel_ground.x = 0.0;
        P::SpecialAirHiEnd
    };
    set(f, s, 0.0);
}
pub(crate) fn map_end_zip(f: &mut Fighter) {
    set_zip_end(f);
}
fn can_sub_zip(f: &Fighter) -> bool {
    let s = Vec2::new(f.stick.x as f32, f.stick.y as f32);
    !f.pikachu.subsequent_zip && s.length() >= 60.0 && {
        let prev = f.pikachu.zip_stick;
        (prev.x * s.x + prev.y * s.y) / (prev.length() * s.length())
            < sin_cos(42.0 * core::f32::consts::PI / 180.0).1
    }
}
fn make_thunder(f: &mut Fighter) {
    f.pikachu.spawned = true;
    f.pikachu.thunder_motion = f.motion.count;
    if let Some(top) = f.pikachu.map_bound_top {
        let anchor = f.joint_world(11, Vec3::ZERO);
        f.weapon_spawn = Some(WeaponSpawn {
            kind: WeaponKind::PikachuThunder,
            owner_port: f.port,
            team: f.team,
            position: Vec3::new(anchor.x, top - 500.0, anchor.z),
            facing: f.facing.sign(),
            stale: crate::stale::WeaponStale::of(f),
        });
    } else {
        f.pikachu.thunder_destroyed = true;
    }
}
pub fn thunder_controlling(s: AnyStatus) -> bool {
    matches!(
        s,
        AnyStatus::Pikachu(
            P::SpecialLwLoop | P::SpecialAirLwLoop | P::SpecialLwHit | P::SpecialAirLwHit
        )
    )
}
pub fn update(f: &mut Fighter) {
    let AnyStatus::Pikachu(s) = f.status.status else {
        return;
    };
    let ground = is_grounded(s);
    match s {
        // The battle entry runs in `crate::appear`.
        P::AppearR | P::AppearL => {}
        P::SpecialN | P::SpecialAirN => {
            if !f.pikachu.spawned && f.status.anim_frame >= 21.0 {
                f.pikachu.spawned = true;
                f.weapon_spawn = Some(WeaponSpawn {
                    kind: WeaponKind::PikachuThunderJolt,
                    owner_port: f.port,
                    team: f.team,
                    position: f.joint_world(11, Vec3::ZERO),
                    facing: f.facing.sign(),
                    stale: crate::stale::WeaponStale::of(f),
                });
                crate::colanim::check_set(
                    f,
                    crate::colanim::ColAnimId::FIGHTER_PIKACHU_SPECIAL_N,
                    0,
                );
            }
            if f.status.animation_ended() {
                if ground {
                    status::anim_end_set_wait(f)
                } else {
                    status::anim_end_set_fall(f)
                }
            }
        }
        P::SpecialLwStart | P::SpecialAirLwStart => {
            if f.status.anim_frame >= 24.0 && !f.pikachu.spawned {
                make_thunder(f);
            }
            if f.status.animation_ended() {
                if !f.pikachu.spawned {
                    make_thunder(f);
                }
                set(
                    f,
                    if ground {
                        P::SpecialLwLoop
                    } else {
                        P::SpecialAirLwLoop
                    },
                    0.0,
                );
            }
        }
        P::SpecialLwLoop | P::SpecialAirLwLoop => {
            let collide = f.pikachu.thunder_position.is_some_and(|p| {
                (f.pos.x - p.x).abs() < 200.0 && (f.pos.y - (p.y + 225.0)).abs() < 800.0
            });
            if !f.pikachu.thunder_destroyed && collide {
                f.pikachu.thunder_collide = true;
                set(
                    f,
                    if ground {
                        P::SpecialLwHit
                    } else {
                        P::SpecialAirLwHit
                    },
                    0.0,
                );
                if !ground {
                    f.physics.vel_air.y = 20.0;
                }
            } else if f.pikachu.thunder_destroyed || f.status.anim_frame >= 60.0 {
                set(
                    f,
                    if ground {
                        P::SpecialLwEnd
                    } else {
                        P::SpecialAirLwEnd
                    },
                    0.0,
                );
            }
        }
        P::SpecialLwHit | P::SpecialAirLwHit => {
            if f.status.anim_frame >= 30.0 {
                set(
                    f,
                    if ground {
                        P::SpecialLwEnd
                    } else {
                        P::SpecialAirLwEnd
                    },
                    0.0,
                );
            }
        }
        P::SpecialLwEnd | P::SpecialAirLwEnd => {
            if f.status.animation_ended() {
                if ground {
                    status::anim_end_set_wait(f)
                } else {
                    status::anim_end_set_fall(f)
                }
            }
        }
        P::SpecialHiStart | P::SpecialAirHiStart | P::SpecialHi | P::SpecialAirHi => {
            f.pikachu.zip_frames -= 1;
            if f.pikachu.zip_frames <= 0 {
                if matches!(s, P::SpecialHiStart | P::SpecialAirHiStart) {
                    set_zip(f)
                } else {
                    set_zip_end(f)
                }
            }
        }
        P::SpecialHiEnd | P::SpecialAirHiEnd => {
            if f.pikachu.zip_flag1 == 0 && f.status.anim_frame >= 9.0 {
                f.pikachu.zip_flag1 = 1;
            }
            if f.pikachu.zip_flag1 == 1 {
                if can_sub_zip(f) {
                    f.pikachu.subsequent_zip = true;
                    f.pikachu.zip_flag1 = 0;
                    set_zip(f)
                } else {
                    f.pikachu.zip_flag1 = 2;
                }
            } else if f.status.animation_ended() {
                if ground {
                    status::set_wait(f)
                } else {
                    status::set_fall_special(f, 0.4, true, true, 0.4, false)
                }
            }
        }
    }
}
pub fn apply_ground_physics(f: &mut Fighter) -> bool {
    match f.status.status {
        AnyStatus::Pikachu(P::SpecialHi) => {
            let n = f.floor.map_or(Vec2::new(0.0, 1.0), |s| s.normal);
            f.physics.vel_air.x = f.physics.vel_ground.x * n.y;
            f.physics.vel_air.y = -f.physics.vel_ground.x * n.x;
        }
        AnyStatus::Pikachu(P::SpecialHiEnd) if f.pikachu.zip_flag1 == 0 => {}
        _ => return false,
    }
    true
}
pub fn apply_air_physics(f: &mut Fighter) -> bool {
    match f.status.status {
        AnyStatus::Pikachu(
            P::SpecialAirN | P::SpecialAirLwStart | P::SpecialAirLwLoop | P::SpecialAirLwEnd,
        ) => {
            if f.physics.is_fastfall {
                physics::apply_fast_fall(&mut f.physics, &f.attributes);
            } else {
                physics::apply_gravity_default(&mut f.physics, &f.attributes);
            }
            if !physics::check_clamp_air_vel_x_dec(&mut f.physics, f.attributes.air_speed_max_x) {
                physics::apply_air_friction(&mut f.physics, &f.attributes);
            }
        }
        AnyStatus::Pikachu(P::SpecialAirHi) => {
            f.pikachu.pass_timer = f.pikachu.pass_timer.saturating_add(1);
        }
        AnyStatus::Pikachu(P::SpecialAirHiStart | P::SpecialAirLwHit) => {
            let g = if f.status.status == AnyStatus::Pikachu(P::SpecialAirHiStart) {
                0.8
            } else {
                0.5
            };
            physics::apply_gravity_clamp_tvel(&mut f.physics, g, f.attributes.tvel_base);
            if !physics::check_clamp_air_vel_x_dec(&mut f.physics, f.attributes.air_speed_max_x) {
                physics::apply_air_friction(&mut f.physics, &f.attributes);
            }
        }
        AnyStatus::Pikachu(P::SpecialAirHiEnd) => {
            if f.pikachu.zip_flag1 != 0 {
                physics::apply_gravity_default(&mut f.physics, &f.attributes);
                physics::clamp_air_vel_x_stick_range(
                    &mut f.physics,
                    f.input.stick_x,
                    physics::AIRDRIFT_STICK_MIN,
                    f.attributes.air_accel * 0.5,
                    f.attributes.air_speed_max_x * 0.5,
                );
            } else {
                f.physics.vel_air.y -= f.physics.vel_air.y / 9.0;
            }
            physics::apply_air_friction(&mut f.physics, &f.attributes);
        }
        _ => return false,
    }
    true
}
pub fn on_ground_lost(f: &mut Fighter) -> bool {
    let AnyStatus::Pikachu(s) = f.status.status else {
        return false;
    };
    let target = match s {
        P::SpecialN => P::SpecialAirN,
        P::SpecialLwStart => P::SpecialAirLwStart,
        P::SpecialLwLoop => P::SpecialAirLwLoop,
        P::SpecialLwHit => P::SpecialAirLwHit,
        P::SpecialLwEnd => P::SpecialAirLwEnd,
        P::SpecialHiStart => P::SpecialAirHiStart,
        P::SpecialHi => P::SpecialAirHi,
        P::SpecialHiEnd => P::SpecialAirHiEnd,
        _ => return false,
    };
    let frame = f.status.anim_frame;
    let zip_velocity = f.physics.vel_air;
    f.become_airborne();
    if s == P::SpecialHi {
        f.physics.jumps_used = f.attributes.jumps_max;
        f.physics.vel_air = zip_velocity;
    } else if !matches!(s, P::SpecialHiStart | P::SpecialHiEnd) {
        physics::clamp_air_vel_x(&mut f.physics, f.attributes.air_speed_max_x);
    }
    switch(f, target, frame);
    true
}
pub fn on_landing(f: &mut Fighter, y: f32, normal: Vec2) -> bool {
    let AnyStatus::Pikachu(s) = f.status.status else {
        return false;
    };
    let target = match s {
        P::SpecialAirN => P::SpecialN,
        P::SpecialAirLwStart => P::SpecialLwStart,
        P::SpecialAirLwLoop => P::SpecialLwLoop,
        P::SpecialAirLwHit => P::SpecialLwHit,
        P::SpecialAirLwEnd => P::SpecialLwEnd,
        P::SpecialAirHiStart => P::SpecialHiStart,
        P::SpecialAirHi => P::SpecialHi,
        P::SpecialAirHiEnd => {
            f.land(y);
            // `FTPIKACHU_QUICKATTACK_LANDING_LAG`.
            status::set_landing_fall_special(f, false, 0.4);
            return true;
        }
        _ => return false,
    };
    let v = f.physics.vel_air;
    let frame = f.status.anim_frame;
    f.land(y);
    switch(f, target, frame);
    if s == P::SpecialAirHi {
        f.physics.vel_ground.x = v.x;
        f.physics.vel_air = v;
        if normal.x * v.x + normal.y * v.y
            < -core::f32::consts::FRAC_1_SQRT_2 * Vec2::new(v.x, v.y).length()
        {
            set_zip_end(f);
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fighter::{JointTransform, Situation};
    use crate::status::Status;
    use ssb_engine::input::{ControllerState, N64Buttons};

    fn pikachu(ground: bool) -> Fighter {
        let mut f = Fighter::new(FighterKind::Pikachu, 0, 3);
        if ground {
            status::set_wait(&mut f);
        } else {
            status::set_fall(&mut f);
        }
        f
    }

    fn input(f: &mut Fighter, buttons: u16, x: i8, y: i8) {
        f.set_input(
            ControllerState {
                buttons: N64Buttons(buttons),
                stick_x: x,
                stick_y: y,
                ..Default::default()
            },
            false,
            false,
        );
    }

    fn steps(f: &mut Fighter, frames: usize) {
        for _ in 0..frames {
            status::update(f);
        }
    }

    fn close(actual: f32, expected: f32) {
        assert!((actual - expected).abs() < 0.001, "{actual} != {expected}");
    }

    #[test]
    fn b_and_stick_route_ground_and_air_specials() {
        for ground in [true, false] {
            for (stick_y, expected) in [
                (0, if ground { P::SpecialN } else { P::SpecialAirN }),
                (
                    80,
                    if ground {
                        P::SpecialHiStart
                    } else {
                        P::SpecialAirHiStart
                    },
                ),
                (
                    -80,
                    if ground {
                        P::SpecialLwStart
                    } else {
                        P::SpecialAirLwStart
                    },
                ),
            ] {
                let mut f = pikachu(ground);
                input(&mut f, N64Buttons::B, 0, stick_y);
                status::update(&mut f);
                assert_eq!(f.status.status, AnyStatus::Pikachu(expected));
            }
        }
    }

    #[test]
    fn thunder_jolt_frame_21_uses_joint_11_and_survives_map_switches() {
        let mut f = pikachu(true);
        f.joint_transforms[11] = Some(JointTransform {
            axes: [
                Vec3::new(0.0, 2.0, 0.0),
                Vec3::new(-2.0, 0.0, 0.0),
                Vec3::new(0.0, 0.0, 2.0),
            ],
            origin: Vec3::new(123.0, 456.0, 78.0),
        });
        f.facing = Facing::Left;
        set_special_n(&mut f);
        steps(&mut f, 20);
        assert!(f.weapon_spawn.is_none());
        assert!(on_ground_lost(&mut f));
        assert_eq!(f.status.anim_frame, 20.0);
        status::update(&mut f);
        let spawn = f
            .take_weapon_spawn()
            .expect("flag 0 fires at WaitAsync(21)");
        assert_eq!(spawn.kind, WeaponKind::PikachuThunderJolt);
        assert_eq!(spawn.position, Vec3::new(123.0, 456.0, 78.0));
        assert_eq!(spawn.facing, -1.0);
        assert!(on_landing(&mut f, 0.0, Vec2::new(0.0, 1.0)));
        assert_eq!(f.status.anim_frame, 21.0);
        steps(&mut f, 43);
        assert!(
            f.weapon_spawn.is_none(),
            "landing must not refire consumed flag 0"
        );
        assert_eq!(f.status.status, AnyStatus::Common(Status::Wait));
    }

    #[test]
    fn jab_buffers_another_attack11_at_flag_1_as_a_new_motion() {
        let mut f = pikachu(true);
        status::set_attack11(&mut f);
        let first_motion = f.motion.count;
        input(&mut f, N64Buttons::A, 0, 0);
        status::update(&mut f);
        input(&mut f, 0, 0, 0);
        // `ftMainPlayAnimEventsAll` already played the entry frame.
        steps(&mut f, 7);
        assert_eq!(f.status.anim_frame, 9.0);
        assert_eq!(f.motion.count, first_motion);
        status::update(&mut f);
        assert_eq!(f.status.status, AnyStatus::Common(Status::Attack11));
        assert_eq!(f.status.anim_frame, 1.0);
        assert_ne!(f.motion.count, first_motion);
    }

    #[test]
    fn quick_attack_waits_20_then_zips_5_and_reads_the_second_direction_at_9() {
        let mut f = pikachu(false);
        input(&mut f, 0, 80, 0);
        set_special_hi(&mut f);
        steps(&mut f, 19);
        assert_eq!(f.status.status, AnyStatus::Pikachu(P::SpecialAirHiStart));
        assert_eq!(f.status.anim_frame, 0.0, "startup freezes the figatree");
        status::update(&mut f);
        assert_eq!(f.status.status, AnyStatus::Pikachu(P::SpecialAirHi));
        close(f.physics.vel_air.x, 330.0);
        assert_eq!(f.physics.jumps_used, f.attributes.jumps_max);
        steps(&mut f, 4);
        assert_eq!(f.status.status, AnyStatus::Pikachu(P::SpecialAirHi));
        assert_eq!(f.status.anim_frame, 0.0, "zip freezes the figatree");
        status::update(&mut f);
        assert_eq!(f.status.status, AnyStatus::Pikachu(P::SpecialAirHiEnd));
        close(f.physics.vel_air.x, 66.0);
        input(&mut f, 0, 0, 80);
        steps(&mut f, 8);
        assert!(!f.pikachu.subsequent_zip);
        status::update(&mut f);
        assert_eq!(f.status.status, AnyStatus::Pikachu(P::SpecialAirHi));
        assert!(f.pikachu.subsequent_zip);
        close(f.physics.vel_air.y, 297.0);
        steps(&mut f, 5);
        input(&mut f, 0, -80, 0);
        steps(&mut f, 9);
        assert_eq!(
            f.status.status,
            AnyStatus::Pikachu(P::SpecialAirHiEnd),
            "only two zips"
        );
    }

    #[test]
    fn quick_attack_requires_more_than_42_degrees_for_a_second_zip() {
        for (x, y, second_zip) in [(60, 50, false), (60, 55, true)] {
            // atan(50/60) < 42 degrees; atan(55/60) > 42 degrees.
            let mut f = pikachu(false);
            input(&mut f, 0, 80, 0);
            set_special_hi(&mut f);
            steps(&mut f, 25);
            input(&mut f, 0, x, y);
            steps(&mut f, 9);
            assert_eq!(f.pikachu.subsequent_zip, second_zip);
        }
        // Magnitude 59 still fails even when the direction changes 90 degrees.
        let mut f = pikachu(false);
        input(&mut f, 0, 80, 0);
        set_special_hi(&mut f);
        steps(&mut f, 25);
        input(&mut f, 0, 0, 59);
        steps(&mut f, 9);
        assert!(!f.pikachu.subsequent_zip);
    }

    #[test]
    fn aerial_zip_at_or_below_60_defaults_up_and_zero_x_faces_right() {
        for x in [0, 59, 60, 61] {
            let mut f = pikachu(false);
            f.facing = Facing::Left;
            input(&mut f, 0, x, 0);
            set_special_hi(&mut f);
            steps(&mut f, 20);
            assert_eq!(f.facing, Facing::Right);
            if x <= 60 {
                close(f.physics.vel_air.x, 0.0);
                close(f.physics.vel_air.y, 330.0);
            } else {
                close(f.physics.vel_air.x, 273.0);
                close(f.physics.vel_air.y, 0.0);
            }
        }
    }

    #[test]
    fn quick_attack_recovery_enters_source_fall_special_flags() {
        let mut f = pikachu(false);
        input(&mut f, 0, 80, 0);
        set_special_hi(&mut f);
        steps(&mut f, 25 + 46);
        assert_eq!(f.status.status, AnyStatus::Common(Status::FallSpecial));
        assert!(f.fall_special.is_fall_accelerate);
        assert!(f.fall_special.is_goto_landing);
        assert!(!f.fall_special.is_allow_interrupt);
        close(f.fall_special.drift, f.attributes.air_speed_max_x * 0.4);
        close(f.fall_special.landing_lag, 0.4);
    }

    #[test]
    fn neutral_and_thunder_air_physics_apply_friction_without_stick_drift() {
        for s in [
            P::SpecialAirN,
            P::SpecialAirLwStart,
            P::SpecialAirLwLoop,
            P::SpecialAirLwEnd,
        ] {
            let mut f = pikachu(false);
            set(&mut f, s, 0.0);
            input(&mut f, 0, 80, 0);
            assert!(apply_air_physics(&mut f));
            close(f.physics.vel_air.x, 0.0);
            close(f.physics.vel_air.y, -f.attributes.gravity);
        }
    }

    #[test]
    fn air_friction_specials_preserve_an_existing_fastfall() {
        for s in [
            P::SpecialAirN,
            P::SpecialAirLwStart,
            P::SpecialAirLwLoop,
            P::SpecialAirLwEnd,
        ] {
            let mut f = pikachu(false);
            set(&mut f, s, 0.0);
            f.physics.is_fastfall = true;
            f.physics.vel_air.y = -10.0;
            assert!(apply_air_physics(&mut f));
            close(f.physics.vel_air.y, -f.attributes.tvel_fast);
        }
    }

    #[test]
    fn thunder_uses_stage_top_at_24_then_strict_collision_and_hit_recovery() {
        let mut f = pikachu(false);
        f.pikachu.map_bound_top = Some(5000.0);
        f.joint_transforms[11] = Some(JointTransform {
            axes: [Vec3::ZERO; 3],
            origin: Vec3::new(123.0, 456.0, 78.0),
        });
        set_special_lw(&mut f);
        steps(&mut f, 23);
        assert!(f.weapon_spawn.is_none());
        status::update(&mut f);
        let spawn = f.take_weapon_spawn().expect("Thunder at frame 24");
        assert_eq!(spawn.kind, WeaponKind::PikachuThunder);
        assert_eq!(spawn.position, Vec3::new(123.0, 4500.0, 78.0));
        assert_eq!(f.status.status, AnyStatus::Pikachu(P::SpecialAirLwLoop));
        // Source tests dist_x < 200 and dist_y < 800, offsetting head Y by 225.
        f.pikachu.thunder_position = Some(Vec3::new(f.pos.x + 200.0, f.pos.y - 225.0, 0.0));
        status::update(&mut f);
        assert_eq!(f.status.status, AnyStatus::Pikachu(P::SpecialAirLwLoop));
        f.pikachu.thunder_position = Some(Vec3::new(f.pos.x + 199.0, f.pos.y + 575.0, 0.0));
        status::update(&mut f);
        assert_eq!(f.status.status, AnyStatus::Pikachu(P::SpecialAirLwLoop));
        f.pikachu.thunder_position = Some(Vec3::new(f.pos.x + 199.0, f.pos.y + 574.0, 0.0));
        status::update(&mut f);
        assert_eq!(f.status.status, AnyStatus::Pikachu(P::SpecialAirLwHit));
        assert!(f.pikachu.thunder_collide);
        close(f.physics.vel_air.y, 20.0);
        steps(&mut f, 29);
        assert_eq!(f.status.status, AnyStatus::Pikachu(P::SpecialAirLwHit));
        status::update(&mut f);
        assert_eq!(f.status.status, AnyStatus::Pikachu(P::SpecialAirLwEnd));
        steps(&mut f, 38);
        assert_eq!(f.status.status, AnyStatus::Common(Status::Fall));
        assert!(f.weapon_spawn.is_none(), "one Thunder per startup");
    }

    #[test]
    fn zip_end_lands_into_landing_fall_special() {
        let mut f = pikachu(false);
        set(&mut f, P::SpecialAirHiEnd, 10.0);
        assert!(on_landing(&mut f, 123.0, Vec2::new(0.0, 1.0)));
        assert_eq!(f.situation, Situation::Ground);
        assert_eq!(f.pos.y, 123.0);
        assert_eq!(
            f.status.status,
            AnyStatus::Common(Status::LandingFallSpecial)
        );
        // `FTPIKACHU_QUICKATTACK_LANDING_LAG` is the landing's speed.
        close(f.status.timing.anim_speed, 0.4);
        assert!(!f.fall_special.landing_allow_interrupt);
    }
}
