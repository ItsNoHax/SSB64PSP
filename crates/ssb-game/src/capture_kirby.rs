//! The swallowed side of Kirby's Inhale: `ftcommoncapturekirby.c`'s
//! `CaptureKirby` (173), `CaptureWaitKirby` (174), `ThrownKirbyStar` (175)
//! and `ThrownCopyStar` (176) common statuses.
//!
//! `CaptureKirby` and `CaptureWaitKirby` are held statuses of the grab link
//! (`crate::grab`). The source's victim physics writes Kirby's
//! `status_vars.kirby.specialn.dist` while pulling itself in; the port keeps
//! that vector on Kirby ([`crate::kirby::KirbyState::inhale_dist`]), decays
//! it in Kirby's own catch update, and the victim reads it through
//! [`crate::grab::Holder::kirby_dist`]. The victim's wiggle writes Kirby's
//! velocity and status; it travels as [`GrabEvent::KirbyWiggle`].
//!
//! ## Documented deviations
//!
//! * **Drop-through wiggle.** A downward wiggle tests the victim's stale
//!   `floor_flags & 0x4000` and makes Kirby ignore its floor line. Map
//!   collision has no pass-through line flag here, so it is not ported.
//!
//! Star floor/ceiling/wall reflection consumes the shared map contacts.
//!
//! * **Visual scale.** The swallow and star shrink the victim's first joint
//!   (`FTCOMMON_CAPTUREKIRBY_MAGNITUDE_*`); that is presentation.

use ssb_engine::math::{sqrt, Vec2, Vec3};

use crate::fighter::{Facing, Fighter, FighterKind};
use crate::grab::{self, GrabEvent, Holder};
use crate::physics;
use crate::status::{self, KirbyStatus, Status, StatusTiming};

/// `FTCOMMON_CAPTUREKIRBY_*` (US).
pub const WIGGLE_STICK_RANGE_MIN: i32 = 53;
pub const WIGGLE_BUFFER_TICS_MAX: u8 = 4;
pub const DIST_X_MIN: f32 = 28.0;
pub const DIST_Y_MIN: f32 = 36.0;
pub const WIGGLE_VEL: f32 = 20.0;
/// `ftCommonCaptureWaitKirbySetStatus`'s breakout inputs.
pub const WAIT_BREAKOUT: i32 = 500;
/// `FTCOMMON_THROWNKIRBYSTAR_*` and `FTCOMMON_THROWNCOPYSTAR_*` (US).
pub const STAR_BREAKOUT_INPUTS_MIN: i32 = 3;
pub const KIRBYSTAR_DECELERATE: f32 = 4.0;
pub const COPYSTAR_DECELERATE: f32 = 5.2;
pub const STAR_RELEASE_VEL_X: f32 = 22.0;
pub const STAR_RELEASE_VEL_Y: f32 = 70.0;
/// `ftCommonCaptureWaitKirbyProcMap`: 10 in front of Kirby's TopN.
pub const WAIT_OFF_X: f32 = 10.0;
/// `ftKirbySpecialNAddCaptureDistance`.
pub const CAPTURE_OFF: Vec2 = Vec2::new(160.0, 100.0);
/// `dFTCommonCaptureKirbyKnockbackCatch` and `...KnockbackCapture`:
/// `{ angle, kbs, kbw, kbb }`.
pub const KNOCKBACK_CATCH: (i32, i32, i32, i32) = (361, 100, 90, 0);
pub const KNOCKBACK_CAPTURE: (i32, i32, i32, i32) = (80, 100, 60, 0);

/// The victim's `status_vars.common.capturekirby` and the star's motion
/// flags.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct CaptureKirbyState {
    /// `is_goto_capturewait`, written by Kirby's catch update.
    pub is_goto_wait: bool,
    /// `is_kirby`: the victim is a Kirby whose copy Kirby took.
    pub is_kirby: bool,
    /// `capturekirby.lr`: the rebound direction, or 0.
    pub lr: f32,
    /// Motion flag 1: travel frames left, then the release countdown.
    pub flag1: i32,
    /// Motion flag 2: 1 copy star, 2 spit star, 3 releasing.
    pub flag2: u8,
    /// `throw_gobj`: the Kirby that spat this star, which it never hits.
    pub thrower: Option<u8>,
    /// `ftParamSetHitStatusAll(nGMHitStatusIntangible)`.
    pub intangible: bool,
}

pub fn is_captured(status: crate::status::AnyStatus) -> bool {
    matches!(
        status,
        crate::status::AnyStatus::Common(Status::CaptureKirby | Status::CaptureWaitKirby)
    )
}

pub fn is_star(status: crate::status::AnyStatus) -> bool {
    matches!(
        status,
        crate::status::AnyStatus::Common(Status::ThrownKirbyStar | Status::ThrownCopyStar)
    )
}

/// Whether a hit passes through this fighter.
pub fn is_intangible(f: &Fighter) -> bool {
    f.kirby_capture.intangible && (is_star(f.status.status) || is_captured(f.status.status))
}

/// `ftCommonCaptureKirbyProcCapture`, run on the victim once Kirby's
/// snapshot has been delivered.
pub fn capture(f: &mut Fighter, catcher_port: u8, holder: Holder) {
    grab::drop_own_catch(f);
    f.grab.capture = Some(catcher_port);
    f.grab.holder = Some(holder);
    f.grab.is_catchstatus = false;
    f.facing = holder.facing.flipped();
    f.become_airborne();
    status::set_status(f, Status::CaptureKirby, 0.0, StatusTiming::unknown());
    f.kirby_capture = CaptureKirbyState::default();
    f.grab.capture_immune = true;
    physics::stop_all(&mut f.physics);
    f.hitstun = 0;
    place(f, holder);
}

/// `ftCommonCaptureKirbyUpdatePositionsAll`'s placement half: Kirby's
/// capture point plus the shrinking offset.
fn place(f: &mut Fighter, holder: Holder) {
    let lr = holder.facing.sign();
    f.pos.x = holder.pos.x + CAPTURE_OFF.x * lr + holder.kirby_dist.x;
    f.pos.y = holder.pos.y + CAPTURE_OFF.y + holder.kirby_dist.y;
    f.pos.z = holder.pos.z;
}

/// The decay half of `ftCommonCaptureKirbyUpdatePositionsAll`: at most
/// 28 across and 36 up per frame. Kirby owns the vector.
pub fn decay_dist(dist: Vec2) -> Vec2 {
    let step_x = dist.x.clamp(-DIST_X_MIN, DIST_X_MIN);
    let step_y = dist.y.clamp(-DIST_Y_MIN, DIST_Y_MIN);
    Vec2::new(dist.x - step_x, dist.y - step_y)
}

/// Held physics for the two capture statuses (`ftCommonCaptureKirbyProcPhysics`
/// then `ftCommonCaptureWaitKirbyProcMap`). Both stay airborne.
pub fn update_held(f: &mut Fighter, holder: Holder) {
    f.situation = crate::fighter::Situation::Air;
    f.floor = None;
    if f.status.status == Status::CaptureKirby {
        place(f, holder);
        if f.kirby_capture.is_goto_wait {
            set_capture_wait(f);
        }
    } else {
        f.pos = holder.pos;
        f.pos.x += WAIT_OFF_X * holder.facing.sign();
    }
}

/// `ftCommonCaptureWaitKirbySetStatus`.
fn set_capture_wait(f: &mut Fighter) {
    status::set_status(f, Status::CaptureWaitKirby, 0.0, StatusTiming::unknown());
    f.grab.capture_immune = true;
    f.is_invisible = true;
    f.kirby_capture.intangible = true;
    grab::init_breakout(f, WAIT_BREAKOUT);
}

/// Kirby's eat: `is_goto_capturewait`, and `is_kirby` for a Kirby victim.
pub fn on_eaten(f: &mut Fighter, is_kirby: bool) {
    f.kirby_capture.is_goto_wait = true;
    f.kirby_capture.is_kirby = is_kirby;
}

/// `ftCommonCaptureWaitKirbyProcInterrupt`, with
/// `ftCommonCaptureWaitKirbyUpdateBreakoutVars`.
pub fn update_captured(f: &mut Fighter) {
    if f.status.status != Status::CaptureWaitKirby {
        return;
    }
    let Some(holder) = f.grab.holder else {
        return;
    };
    if matches!(
        holder.status,
        crate::status::AnyStatus::Kirby(KirbyStatus::SpecialNWait | KirbyStatus::SpecialAirNWait)
    ) {
        let (x, y) = (i32::from(f.stick.x), i32::from(f.stick.y));
        let mut up = false;
        if y >= WIGGLE_STICK_RANGE_MIN && f.stick.tap_y < WIGGLE_BUFFER_TICS_MAX {
            f.stick.tap_y = crate::status::STICKBUFFER_MAX;
            up = true;
        }
        let mut push = None;
        if x.abs() >= WIGGLE_STICK_RANGE_MIN + 3 && f.stick.tap_x < WIGGLE_BUFFER_TICS_MAX * 2 {
            f.stick.tap_x = crate::status::STICKBUFFER_MAX;
            push = Some(if x < 0 { -WIGGLE_VEL } else { WIGGLE_VEL });
        }
        if up || push.is_some() {
            f.grab.send(GrabEvent::KirbyWiggle { up, push_x: push });
        }
    }
    let before = f.grab.breakout_wait;
    grab::update_breakout(f);
    f.grab.breakout_wait -= (before - f.grab.breakout_wait) * 5;
    f.grab.breakout_wait -= 1;
    if f.grab.breakout_wait <= 0 {
        f.grab.send(GrabEvent::KirbyBreakout);
        escape_breakout(f, holder);
    }
}

/// The victim's half of a finished breakout: `ftCommonCaptureApplyCaptureKnockback`
/// with the Kirby descriptor.
fn escape_breakout(f: &mut Fighter, holder: Holder) {
    f.kirby_capture.intangible = false;
    grab::apply_capture_knockback_with(f, holder, KNOCKBACK_CAPTURE);
}

/// `ftCommonThrownKirbyStarSetStatus` / `ftCommonThrownCopyStarSetStatus`
/// and their `proc_status`: the victim leaves the link at `vel`.
pub fn set_star(f: &mut Fighter, copy: bool, vel: Vec3, thrower: u8) {
    let is_kirby = f.kirby_capture.is_kirby;
    grab::lose_grip(f);
    f.become_airborne();
    let status = if copy {
        Status::ThrownCopyStar
    } else {
        Status::ThrownKirbyStar
    };
    status::set_status(f, status, 0.0, StatusTiming::unknown());
    f.physics.vel_air = vel;
    f.physics.vel_ground = Vec3::ZERO;
    f.physics.vel_knockback = Vec3::ZERO;
    f.grab.capture_immune = true;
    f.is_invisible = true;
    f.is_shadow_hidden = true;
    f.kirby_capture = CaptureKirbyState {
        is_goto_wait: false,
        is_kirby,
        lr: if vel.x < 0.0 { -1.0 } else { 1.0 },
        flag1: if copy { 10 } else { 30 },
        flag2: if copy { 1 } else { 2 },
        thrower: Some(thrower),
        intangible: true,
    };
    if !copy {
        grab::init_breakout(f, STAR_BREAKOUT_INPUTS_MIN);
    }
}

/// `ftCommonThrownKirbyEscape`.
fn escape(f: &mut Fighter) {
    status::set_status(f, Status::Fall, 0.0, StatusTiming::unknown());
    f.kirby_capture.thrower = None;
    f.kirby_capture.intangible = false;
    f.grab.capture_immune = false;
    f.is_invisible = false;
    f.is_shadow_hidden = false;
}

/// `ftCommonThrownCommonStarUpdatePhysics`. Returns `true` for a star status.
pub fn apply_air_physics(f: &mut Fighter) -> bool {
    let decelerate = match f.status.status {
        crate::status::AnyStatus::Common(Status::ThrownKirbyStar) => KIRBYSTAR_DECELERATE,
        crate::status::AnyStatus::Common(Status::ThrownCopyStar) => COPYSTAR_DECELERATE,
        _ => return false,
    };
    let s = &mut f.kirby_capture;
    if s.flag1 != 0 && s.flag2 != 3 {
        let v = f.physics.vel_air;
        let speed = sqrt(v.x * v.x + v.y * v.y);
        if decelerate < speed {
            f.physics.vel_air.x = v.x * (speed - decelerate) / speed;
            f.physics.vel_air.y = v.y * (speed - decelerate) / speed;
            // The source writes `lr` from the vertical sign.
            f.facing = if f.physics.vel_air.y < 0.0 {
                Facing::Left
            } else {
                Facing::Right
            };
        } else {
            f.physics.vel_air.x = 0.0;
            f.physics.vel_air.y = 0.0;
        }
        f.kirby_capture.flag1 -= 1;
        grab::update_breakout(f);
        if f.grab.breakout_wait <= 0 {
            f.kirby_capture.flag1 = 0;
        }
        return true;
    }
    match s.flag2 {
        1 => {
            if is_kirby(f.kind) && s.is_kirby {
                // The copy the swallowing Kirby took is lost.
                f.kirby.copy_id = FighterKind::Kirby;
            }
            escape(f);
        }
        2 => {
            s.flag1 = 4;
            s.flag2 = 3;
            s.intangible = false;
            let lr = if s.lr != 0.0 {
                s.lr
            } else if f.physics.vel_air.x < 0.0 {
                -1.0
            } else {
                1.0
            };
            f.physics.vel_air.y = STAR_RELEASE_VEL_Y;
            f.physics.vel_air.x = lr * STAR_RELEASE_VEL_X;
            f.is_invisible = false;
            f.is_shadow_hidden = false;
            release_countdown(f);
        }
        3 => release_countdown(f),
        _ => {}
    }
    true
}

fn release_countdown(f: &mut Fighter) {
    let before = f.kirby_capture.flag1;
    f.kirby_capture.flag1 -= 1;
    if before <= 0 {
        escape(f);
    }
}

fn is_kirby(kind: FighterKind) -> bool {
    crate::kirby::is_kirby(kind)
}

/// `ftCommonThrownCommonStarProcMap`'s floor case: the star reflects off
/// the floor instead of landing, and a reflection that turns it around
/// ends its travel. Returns `true` for a star status.
pub fn on_landing(f: &mut Fighter, floor_y: f32, normal: Vec2) -> bool {
    if !is_star(f.status.status) {
        return false;
    }
    f.pos.y = floor_y;
    let v = f.physics.vel_air;
    let dot = v.x * normal.x + v.y * normal.y;
    if dot < 0.0 {
        f.physics.vel_air.x = v.x - 2.0 * dot * normal.x;
        f.physics.vel_air.y = v.y - 2.0 * dot * normal.y;
        let after = f.physics.vel_air;
        if after.x * v.x + after.y * v.y < 0.0 {
            f.kirby_capture.lr = if normal.x < 0.0 { -1.0 } else { 1.0 };
            f.kirby_capture.flag1 = 0;
        }
    }
    true
}

/// `ftCommonThrownCommonStarProcHit`: a star that hits something falls.
pub fn on_star_hit(f: &mut Fighter) {
    if f.status.status != Status::ThrownKirbyStar {
        return;
    }
    escape(f);
    physics::clamp_air_vel_x(&mut f.physics, f.attributes.air_speed_max_x);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dist_decays_by_the_per_axis_step() {
        let d = decay_dist(Vec2::new(100.0, -50.0));
        assert_eq!(d, Vec2::new(72.0, -14.0));
        let d = decay_dist(Vec2::new(10.0, 5.0));
        assert_eq!(d, Vec2::ZERO);
    }

    #[test]
    fn spit_star_travels_decelerates_then_pops_out() {
        let mut f = Fighter::new(FighterKind::Mario, 1, 4);
        set_star(&mut f, false, Vec3::new(120.0, 0.0, 0.0), 0);
        assert!(is_intangible(&f));
        apply_air_physics(&mut f);
        assert_eq!(f.physics.vel_air.x, 116.0);
        for _ in 0..29 {
            apply_air_physics(&mut f);
        }
        assert_eq!(f.kirby_capture.flag1, 0);
        apply_air_physics(&mut f);
        assert_eq!(f.kirby_capture.flag2, 3);
        assert!(!is_intangible(&f));
        assert_eq!(f.physics.vel_air.y, STAR_RELEASE_VEL_Y);
        for _ in 0..3 {
            apply_air_physics(&mut f);
            assert_eq!(f.status.status, Status::ThrownKirbyStar);
        }
        apply_air_physics(&mut f);
        assert_eq!(f.status.status, Status::Fall);
    }

    #[test]
    fn copy_star_strips_a_swallowed_kirbys_copy() {
        let mut f = Fighter::new(FighterKind::Kirby, 1, 4);
        f.kirby.copy_id = FighterKind::Fox;
        f.kirby_capture.is_kirby = true;
        // The copy star keeps the capture's breakout count.
        f.grab.breakout_wait = 400;
        set_star(&mut f, true, Vec3::new(-26.0, 96.0, 0.0), 0);
        for _ in 0..10 {
            apply_air_physics(&mut f);
        }
        assert_eq!(f.status.status, Status::ThrownCopyStar);
        apply_air_physics(&mut f);
        assert_eq!(f.status.status, Status::Fall);
        assert_eq!(f.kirby.copy_id, FighterKind::Kirby);
    }
}
