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
//! A downward wiggle tests the victim's `coll_data.floor_flags & 0x4000`
//! (the pass-through flag), which only `CaptureKirby`'s floor projections
//! write ([`CaptureKirbyState::floor_flags`]), and makes a grounded Kirby
//! ignore its own floor line.
//!
//! Star floor/ceiling/wall reflection consumes the shared map contacts.
//!
//! Both star statuses' `proc_update` makes the captured star once
//! (`ftCommonThrownKirbyStarMakeEffect`, queued as
//! [`crate::fteffect::FighterEffect::CaptureKirbyStar`]); each release stops
//! it and makes a star splash, whose `is_effect_attach` keeps it from being
//! made again.
//!
//! ## Documented deviations
//!
//! * **Stale floor flags.** A victim with no floor below it through the
//!   whole pull keeps the flags its last projection or landing left; the
//!   port keeps a grounded victim's floor and otherwise none.
//!
//! * **Visual scale.** The swallow and star shrink the victim's first joint
//!   (`FTCOMMON_CAPTUREKIRBY_MAGNITUDE_*`); that is presentation.

use ssb_engine::math::{sqrt, Vec2, Vec3};

use crate::combat::HitStatus;
use crate::fighter::{Facing, Fighter, FighterKind};
use crate::grab::{self, GrabEvent, Holder};
use crate::hurtbox;
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
pub const KNOCKBACK_CATCH: (i32, i32, i32, i32) =
    crate::grab::release_desc(crate::d_arrays::FT_COMMON_CAPTURE_KIRBY_KNOCKBACK_CATCH);
pub const KNOCKBACK_CAPTURE: (i32, i32, i32, i32) =
    crate::grab::release_desc(crate::d_arrays::FT_COMMON_CAPTURE_KIRBY_KNOCKBACK_CAPTURE);

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
    /// The victim's `coll_data.floor_flags`: its floor's at the capture,
    /// then each `CaptureKirby` floor projection that finds a line
    /// (`mpCommonUpdateFighterProjectFloor`). `CaptureWaitKirby` leaves it.
    pub floor_flags: u16,
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

/// `ftCommonCaptureKirbyProcCapture`, run on the victim once Kirby's
/// snapshot has been delivered.
pub fn capture(f: &mut Fighter, catcher_port: u8, holder: Holder) {
    // `ftParamStopVoiceRunProcDamage`.
    crate::fighter_sound::stop_voice(f);
    grab::drop_own_catch(f);
    f.grab.capture = Some(catcher_port);
    f.grab.holder = Some(holder);
    f.grab.is_catch_or_capture = false;
    f.grab.is_catchstatus = false;
    f.facing = holder.facing.flipped();
    let floor_flags = f.floor.map_or(0, |s| s.flags);
    // `mpCommonSetFighterAir`, unconditional: an airborne victim gets its
    // aerial jump back.
    f.set_fighter_air();
    status::set_status(f, Status::CaptureKirby, 0.0, StatusTiming::unknown());
    // `ftCommonCaptureKirbyProcCapture` plays the first frame (RE-474).
    status::play_anim_events(f);
    crate::rumble::make(f, 7, 0);
    f.kirby_capture = CaptureKirbyState {
        floor_flags,
        ..CaptureKirbyState::default()
    };
    f.grab.capture_immune_mask = crate::grab::catch_kind::ALL;
    physics::stop_all(&mut f.physics);
    f.hitstun = 0;
    place(f, holder);
}

/// `mpCommonUpdateFighterProjectFloor` (`mpProcessSetCollProjectFloorID`):
/// the floor below the victim's map-box bottom. Only a found line writes
/// `floor_flags`.
pub fn project_floor<I>(f: &mut Fighter, floors: I)
where
    I: IntoIterator<Item = (u16, crate::collision::Segment)>,
{
    let probe = Vec2::new(f.pos.x, f.pos.y + f.coll.bottom);
    if let Some(below) = crate::collision::project_floor(floors, probe) {
        f.kirby_capture.floor_flags = below.flags;
    }
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

/// Held physics for the two capture statuses: `CaptureKirby`'s
/// `ftCommonCaptureKirbyProcPhysics` (`CaptureWaitKirby` has none). Both
/// stay airborne. The eat frame's `ProcPhysics` sets `CaptureWaitKirby`,
/// whose own `proc_map` ([`place_wait`]) then runs in that frame's map
/// phase, so the victim already stands at Kirby's side on it (N64 Castle
/// trace: x 508 on the eat row).
pub fn update_held(f: &mut Fighter, holder: Holder) {
    f.situation = crate::fighter::Situation::Air;
    f.floor = None;
    if f.status.status == Status::CaptureKirby {
        place(f, holder);
        if f.kirby_capture.is_goto_wait {
            set_capture_wait(f);
        }
    }
}

/// `ftCommonCaptureWaitKirbyProcMap`: TopN at Kirby's, 10 units ahead.
/// A `proc_map`, so it runs after the dead check, and in hitlag too.
pub fn place_wait(f: &mut Fighter, holder: Holder) {
    f.pos = holder.pos;
    f.pos.x += WAIT_OFF_X * holder.facing.sign();
}

/// `ftCommonCaptureWaitKirbySetStatus`.
fn set_capture_wait(f: &mut Fighter) {
    status::set_status(f, Status::CaptureWaitKirby, 0.0, StatusTiming::unknown());
    f.grab.capture_immune_mask = crate::grab::catch_kind::ALL;
    f.is_invisible = true;
    hurtbox::set_hit_status_all(f, HitStatus::Intangible);
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
        let (mut up, mut down) = (false, false);
        if y >= WIGGLE_STICK_RANGE_MIN && f.stick.tap_y < WIGGLE_BUFFER_TICS_MAX {
            f.stick.tap_y = crate::status::STICKBUFFER_MAX;
            up = true;
        } else if y <= -WIGGLE_STICK_RANGE_MIN
            && f.stick.tap_y < WIGGLE_BUFFER_TICS_MAX
            && f.kirby_capture.floor_flags & crate::collision::flags::PASS != 0
        {
            f.stick.tap_y = crate::status::STICKBUFFER_MAX;
            down = true;
        }
        let mut push = None;
        if x.abs() >= WIGGLE_STICK_RANGE_MIN + 3 && f.stick.tap_x < WIGGLE_BUFFER_TICS_MAX * 2 {
            f.stick.tap_x = crate::status::STICKBUFFER_MAX;
            push = Some(if x < 0 { -WIGGLE_VEL } else { WIGGLE_VEL });
        }
        if up || down || push.is_some() {
            f.grab.send(GrabEvent::KirbyWiggle {
                up,
                down,
                push_x: push,
            });
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
    grab::apply_capture_knockback_with(f, holder, KNOCKBACK_CAPTURE);
}

/// `ftCommonThrownKirbyStarSetStatus` / `ftCommonThrownCopyStarSetStatus`
/// and their `proc_status`: the victim leaves the link at `vel`.
pub fn set_star(f: &mut Fighter, copy: bool, vel: Vec3, owner: crate::thrown::ThrowOwner) {
    let is_kirby = f.kirby_capture.is_kirby;
    grab::lose_grip(f);
    f.become_airborne();
    let status = if copy {
        Status::ThrownCopyStar
    } else {
        Status::ThrownKirbyStar
    };
    f.thrown.owner = Some(owner);
    status::set_any_status_preserve(
        f,
        status.into(),
        0.0,
        StatusTiming::unknown(),
        status::Preserve {
            throw_pointer: true,
            ..status::Preserve::NONE
        },
    );
    if !copy {
        // `ftCommonThrownKirbyStarSetStatus` rewrites the new collisions'
        // damage from `FTKirbyCopy::star_damage`.
        let damage = crate::kirby::star_damage(f.kind);
        for coll in &mut f.attack_colls {
            if coll.state == crate::combat::AttackState::New {
                coll.damage = damage;
            }
        }
    }
    f.physics.vel_air = vel;
    f.physics.vel_ground = Vec3::ZERO;
    f.physics.vel_knockback = Vec3::ZERO;
    f.grab.capture_immune_mask = crate::grab::catch_kind::ALL;
    f.is_invisible = true;
    f.is_shadow_hidden = true;
    hurtbox::set_hit_status_all(f, HitStatus::Intangible);
    f.interface.tag_wait = 1;
    f.kirby_capture = CaptureKirbyState {
        is_goto_wait: false,
        is_kirby,
        lr: if vel.x < 0.0 { -1.0 } else { 1.0 },
        flag1: if copy { 10 } else { 30 },
        flag2: if copy { 1 } else { 2 },
        // The star's own map pass writes the floor from here on.
        floor_flags: f.kirby_capture.floor_flags,
    };
    if !copy {
        grab::init_breakout(f, STAR_BREAKOUT_INPUTS_MIN);
    }
}

/// `ftCommonThrownKirbyEscape`.
fn escape(f: &mut Fighter) {
    status::set_any_status_preserve(
        f,
        Status::Fall.into(),
        0.0,
        StatusTiming::unknown(),
        status::Preserve::DAMAGE_PLAYER,
    );
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
                crate::kirby_copy::init_passive_vars(f);
                f.kirby.copy_id = FighterKind::Kirby;
                f.model_parts
                    .set_default(crate::kirby_copy::COPY_MODELPARTS_JOINT, 0);
                f.model_parts.reset_all();
            }
            f.is_invisible = false;
            hurtbox::set_hit_status_all(f, HitStatus::Normal);
            crate::fteffect::stop_attached(f);
            escape(f);
            star_splash(f);
        }
        2 => {
            s.flag1 = 4;
            s.flag2 = 3;
            let lr = if s.lr != 0.0 {
                s.lr
            } else if f.physics.vel_air.x < 0.0 {
                -1.0
            } else {
                1.0
            };
            f.physics.vel_air.y = STAR_RELEASE_VEL_Y;
            f.physics.vel_air.x = lr * STAR_RELEASE_VEL_X;
            // The captured star ends in a splash.
            crate::fteffect::stop_attached(f);
            star_splash(f);
            // Only `is_invisible`: the shadow stays hidden until the
            // escape's `ftMainSetStatus`.
            f.is_invisible = false;
            hurtbox::set_hit_status_all(f, HitStatus::Normal);
            release_countdown(f);
        }
        3 => release_countdown(f),
        _ => {}
    }
    true
}

/// `efManagerStarSplashMakeEffect(&TopN translate, (-vel_air.x < 0) ? -1 :
/// +1)`: success sets `is_effect_attach`, which keeps the star statuses'
/// `proc_update` from making the captured star again.
fn star_splash(f: &mut Fighter) {
    let lr = if -f.physics.vel_air.x < 0.0 { -1 } else { 1 };
    f.effects
        .push(crate::fteffect::FighterEffect::StarSplash { pos: f.pos, lr });
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

/// `ftCommonThrownCommonStarProcMap`: the star reflects off the floor (or
/// the ceiling or a wall) instead of landing, and a reflection that turns
/// it around ends its travel. Then a magnitude-2 quake and an impact wave at
/// `wave`: TopN's translation as the map process found it, before the
/// collision moved it, put on the struck side of the collision diamond.
/// Returns `true` for a star status.
pub fn on_landing(f: &mut Fighter, floor_y: f32, normal: Vec2, wave: Vec3) -> bool {
    if !is_star(f.status.status) {
        return false;
    }
    f.pos.y = floor_y;
    crate::fteffect::surface_impact(f, wave, normal, true);
    // `lbCommonReflect2D`, whatever the velocity's side of the surface.
    let v = f.physics.vel_air;
    let dot = (normal.x * v.x + normal.y * v.y) * -2.0;
    f.physics.vel_air.x = v.x + normal.x * dot;
    f.physics.vel_air.y = v.y + normal.y * dot;
    let after = f.physics.vel_air;
    if after.x * v.x + v.y * after.y < 0.0 {
        f.kirby_capture.lr = if normal.x < 0.0 { -1.0 } else { 1.0 };
        f.kirby_capture.flag1 = 0;
    }
    true
}

/// `ftCommonThrownCommonStarProcHit`: a star that hits something falls.
pub fn on_star_hit(f: &mut Fighter) {
    // `ftCommonThrownCommonStarProcHit`, both stars' `proc_hit`.
    if !is_star(f.status.status) {
        return;
    }
    escape(f);
    physics::clamp_air_vel_x(&mut f.physics, f.attributes.air_speed_max_x);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
    fn a_spat_star_hits_with_its_copy_table_damage_through_the_common_moveset() {
        // `dFTCommonMoveset_DamageBumpHit` makes the collision;
        // `ftCommonThrownKirbyStarSetStatus` rewrites its damage.
        for (kind, damage) in [
            (crate::fighter::FighterKind::Mario, 17),
            (crate::fighter::FighterKind::Donkey, 30),
            (crate::fighter::FighterKind::Yoshi, 25),
        ] {
            let mut f = Fighter::new(kind, 1, 3);
            set_star(
                &mut f,
                false,
                Vec3::new(40.0, 0.0, 0.0),
                crate::thrown::ThrowOwner {
                    port: 0,
                    kind: FighterKind::Kirby,
                    team: 0,
                },
            );
            let live: std::vec::Vec<_> = f
                .attack_colls
                .iter()
                .filter(|c| c.state != crate::combat::AttackState::Off)
                .collect();
            assert!(!live.is_empty(), "{kind:?}");
            assert!(live.iter().all(|c| c.damage == damage), "{kind:?}");
        }
    }

    /// `ftCommonCaptureWaitKirbySetStatus` (`:244`): invisible and
    /// intangible in Kirby's mouth; the breakout's damage status restores
    /// `Normal` (`ftMainSetStatus`, `ftmain.c:4428`).
    #[test]
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
    fn the_swallowed_fighter_is_intangible_until_it_breaks_out() {
        let mut f = Fighter::new(FighterKind::Mario, 1, 3);
        set_capture_wait(&mut f);
        assert!(f.is_invisible);
        assert_eq!(f.hitstatus, HitStatus::Intangible);
        assert!(crate::combat::is_body_intangible(&f));
        let kirby = Fighter::new(FighterKind::Kirby, 0, 3);
        f.grab.capture = Some(0);
        escape_breakout(&mut f, crate::grab::holder_of(&kirby));
        assert_ne!(f.status.status, Status::CaptureWaitKirby);
        assert_eq!(f.hitstatus, HitStatus::Normal);
        assert!(!crate::combat::is_body_intangible(&f));
        // The breakout calls no `ftParamSetPlayerTagWait` (ROM `0x8014BD04`);
        // the damage status's `ftMainSetStatus` clears the wait, and only
        // level-3 knockback of 130 or more sets 10.
        assert_eq!(f.interface.tag_wait, 0);
    }

    /// The down wiggle needs the victim's projected floor to be passable
    /// (`coll_data.floor_flags & MAP_VERTEX_COLL_PASS`), which only
    /// `mpCommonUpdateFighterProjectFloor` writes.
    #[test]
    fn a_down_wiggle_needs_a_passable_floor_below_the_victim() {
        let kirby = Fighter::new(FighterKind::Kirby, 0, 3);
        let mut holder = crate::grab::holder_of(&kirby);
        holder.status = crate::status::AnyStatus::Kirby(KirbyStatus::SpecialNWait);
        for (flags, sent) in [(crate::collision::flags::PASS, true), (0, false)] {
            let mut f = Fighter::new(FighterKind::Mario, 1, 3);
            f.pos = Vec3::new(0.0, 300.0, 0.0);
            project_floor(
                &mut f,
                [(
                    4,
                    crate::collision::Segment {
                        x1: -500,
                        y1: 0,
                        x2: 500,
                        y2: 0,
                        flags,
                    },
                )],
            );
            assert_eq!(f.kirby_capture.floor_flags, flags);
            set_capture_wait(&mut f);
            f.grab.holder = Some(holder);
            f.grab.breakout_wait = 1000;
            f.stick.y = -80;
            f.stick.tap_y = 0;
            f.grab.outbox = Default::default();
            update_captured(&mut f);
            let wiggle = f.grab.outbox.iter().flatten().find_map(|e| match e {
                GrabEvent::KirbyWiggle { up, down, push_x } => Some((*up, *down, *push_x)),
                _ => None,
            });
            assert_eq!(wiggle.is_some(), sent, "flags {flags:#x}");
            if sent {
                assert_eq!(wiggle, Some((false, true, None)));
            }
        }
    }

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
        set_star(
            &mut f,
            false,
            Vec3::new(120.0, 0.0, 0.0),
            crate::thrown::ThrowOwner {
                port: 0,
                kind: FighterKind::Kirby,
                team: 0,
            },
        );
        assert_eq!(f.hitstatus, HitStatus::Intangible);
        assert!(crate::combat::is_body_intangible(&f));
        assert!(f.is_invisible && f.is_shadow_hidden);
        assert_eq!(f.interface.tag_wait, 1);
        apply_air_physics(&mut f);
        assert_eq!(f.physics.vel_air.x, 116.0);
        for _ in 0..29 {
            apply_air_physics(&mut f);
        }
        assert_eq!(f.kirby_capture.flag1, 0);
        apply_air_physics(&mut f);
        assert_eq!(f.kirby_capture.flag2, 3);
        // `ftParamSetHitStatusAll(Normal)` with the release (`:361`).
        assert_eq!(f.hitstatus, HitStatus::Normal);
        assert!(!crate::combat::is_body_intangible(&f));
        assert_eq!(f.physics.vel_air.y, STAR_RELEASE_VEL_Y);
        // The release shows the fighter but keeps its shadow hidden
        // (`:359` writes only `is_invisible`; ROM `0x8014BF04` clears bit 0
        // of byte 397 and leaves byte 398's `is_shadow_hide`) until the
        // Fall's `ftMainSetStatus` (`ftmain.c:4463`).
        assert!(!f.is_invisible && f.is_shadow_hidden);
        assert_eq!(f.interface.tag_wait, 1);
        for _ in 0..3 {
            apply_air_physics(&mut f);
            assert_eq!(f.status.status, Status::ThrownKirbyStar);
            assert!(f.is_shadow_hidden);
        }
        apply_air_physics(&mut f);
        assert_eq!(f.status.status, Status::Fall);
        // `ftCommonThrownKirbyEscape` is the bare Fall setter
        // (`FTSTATUS_PRESERVE_DAMAGEPLAYER`): shadow shown, tag wait cleared.
        assert!(!f.is_invisible && !f.is_shadow_hidden);
        assert_eq!(f.interface.tag_wait, 0);
    }

    #[test]
    fn copy_star_strips_a_swallowed_kirbys_copy() {
        let mut f = Fighter::new(FighterKind::Kirby, 1, 4);
        f.kirby.copy_id = FighterKind::Fox;
        f.kirby_capture.is_kirby = true;
        // The copy star keeps the capture's breakout count.
        f.grab.breakout_wait = 400;
        set_star(
            &mut f,
            true,
            Vec3::new(-26.0, 96.0, 0.0),
            crate::thrown::ThrowOwner {
                port: 0,
                kind: FighterKind::Kirby,
                team: 0,
            },
        );
        assert_eq!(f.hitstatus, HitStatus::Intangible);
        for _ in 0..10 {
            apply_air_physics(&mut f);
        }
        assert_eq!(f.status.status, Status::ThrownCopyStar);
        apply_air_physics(&mut f);
        assert_eq!(f.status.status, Status::Fall);
        // `ftParamSetHitStatusAll(Normal)` before the escape (`:329`).
        assert_eq!(f.hitstatus, HitStatus::Normal);
        assert_eq!(f.kirby.copy_id, FighterKind::Kirby);
    }

    /// `ftCommonThrownCommonStarProcMap` reflects with `lbCommonReflect2D`
    /// whichever side of the struck surface the velocity points: a star
    /// leaving a wall it touches is turned back into it.
    #[test]
    fn a_star_reflects_off_any_contact_even_moving_away() {
        let mut f = Fighter::new(FighterKind::Mario, 1, 4);
        set_star(
            &mut f,
            false,
            Vec3::new(30.0, 0.0, 0.0),
            crate::thrown::ThrowOwner {
                port: 0,
                kind: FighterKind::Kirby,
                team: 0,
            },
        );
        f.kirby_capture.lr = -1.0;
        let pos = f.pos;
        assert!(on_landing(&mut f, pos.y, Vec2::new(1.0, 0.0), pos));
        assert_eq!(f.physics.vel_air.x, -30.0);
        assert_eq!(f.kirby_capture.lr, 1.0);
        assert_eq!(f.kirby_capture.flag1, 0);
    }

    /// `ftCommonThrownCommonStarUpdatePhysics` (`flag2 == 1`, `:320`):
    /// `ftKirbySpecialNInitPassiveVars` runs before `copy_id` resets, so a
    /// spat Kirby's stored charge does not survive into its next copy.
    #[test]
    fn a_spat_kirby_loses_its_copys_stored_charge() {
        for (copy, set, read) in [
            (
                FighterKind::Samus,
                (|f: &mut Fighter| {
                    f.kirby.copy.samus_charge_level = 5;
                    f.kirby.copy.samus_charge_recoil = 3;
                }) as fn(&mut Fighter),
                (|f: &Fighter| {
                    i32::from(f.kirby.copy.samus_charge_level) + f.kirby.copy.samus_charge_recoil
                }) as fn(&Fighter) -> i32,
            ),
            (
                FighterKind::Donkey,
                |f| f.kirby.copy.donkey_charge_level = 7,
                |f| i32::from(f.kirby.copy.donkey_charge_level),
            ),
            (
                FighterKind::Captain,
                |f| f.kirby.copy.captain_punch_count = 2,
                |f| f.kirby.copy.captain_punch_count,
            ),
            (
                FighterKind::Purin,
                |f| f.kirby.copy.purin_count = 1,
                |f| f.kirby.copy.purin_count,
            ),
        ] {
            let mut f = Fighter::new(FighterKind::Kirby, 1, 4);
            f.kirby.copy_id = copy;
            set(&mut f);
            f.kirby_capture.is_kirby = true;
            f.grab.breakout_wait = 400;
            set_star(
                &mut f,
                true,
                Vec3::new(-26.0, 96.0, 0.0),
                crate::thrown::ThrowOwner {
                    port: 0,
                    kind: FighterKind::Kirby,
                    team: 0,
                },
            );
            for _ in 0..11 {
                apply_air_physics(&mut f);
            }
            assert_eq!(f.status.status, Status::Fall, "{copy:?}");
            assert_eq!(f.kirby.copy_id, FighterKind::Kirby, "{copy:?}");
            assert_eq!(read(&f), 0, "{copy:?}");
        }
    }
}
