//! The swallowed side of Yoshi's Egg Lay: `ftcommoncaptureyoshi.c`'s
//! `CaptureYoshi` (177) and `YoshiEgg` (178) common statuses.
//!
//! `CaptureYoshi` is a held status of the grab link (`crate::grab`): its
//! position follows Yoshi's tongue, and Yoshi's release script writes the
//! swallow stages through [`crate::grab::GrabEvent::YoshiEggStage`]. Stage 1
//! hides the fighter; stage 3 lays the egg.
//!
//! In the egg the fighter mashes out. The source ends the egg on the frame
//! after its `efManagerYoshiEggLayMakeEffect` effect finishes its break
//! animation (`llYoshiSpecial3EggLayBreakAnimJoint`), which starts on the
//! frame the mash counter runs out. Both joints of that animation run 10
//! frames (`339_YoshiSpecial3.c`; opcode 12 adds length and does not wait),
//! so the port counts [`EGG_BREAK_FRAMES`] instead of running the effect.
//!
//! ## Documented deviations
//!
//! * **Egg hurtbox.** `ftCommonYoshiEggSetDamageCollCollisions` resizes the
//!   first hurtbox per fighter (`dFTCommonYoshiEggDamageCollDescs`) and makes
//!   the rest intangible; [`crate::hurtbox::yoshi_egg_coll`] is that box
//!   (RE-332, RE-349).
//! * **Map collision on laying.** `mpCommonRunFighterCollisionDefault` sweeps
//!   from Yoshi to the egg. The shared map solver is not yet wired to that
//!   placement step, so the egg is placed without the sweep.
//! * **Acid floors.** `ftCommonYoshiEggProcTrap`'s damaging-floor escape
//!   needs ground hazards, which are not ported.

use ssb_engine::math::Vec3;

use crate::fighter::Fighter;
use crate::grab::{self, Holder};
use crate::physics;
use crate::status::{self, Status, StatusTiming};

/// `FTCOMMON_YOSHIEGG_*` (US).
pub const INTANGIBLE_TIMER: u16 = 12;
pub const BREAKOUT_INPUTS_MIN: i32 = 750;
pub const ESCAPE_WAIT_MAX: i32 = 250;
pub const ESCAPE_WAIT_DEFAULT: i32 = 15;
pub const LAY_VEL_X: f32 = 20.0;
pub const LAY_VEL_Y: f32 = 60.0;
pub const LAY_OFF_X: f32 = 200.0;
pub const LAY_OFF_Y: f32 = 90.0;
pub const DAMAGE_MUL: f32 = 0.5;
pub const ESCAPE_OFF_Y: f32 = 10.0;
pub const ESCAPE_VEL_Y: f32 = 70.0;
/// Each mash input takes 12 frames off the escape wait.
const MASH_FRAMES: i32 = 12;
/// `ftKirbySpecialNApplyCaptureDamage(capture_gobj, fighter_gobj, 5)`.
pub const LAY_DAMAGE: u16 = 5;
/// Length of `llYoshiSpecial3EggLayBreakAnimJoint`.
pub const EGG_BREAK_FRAMES: u8 = 10;

/// `FTCOMMON_YOSHIEGG_WIGGLE_*`.
pub const WIGGLE_STICK_RANGE_MIN: i32 = 26;
pub const WIGGLE_GFX_RANGE_XY: f32 = 22.0;
pub const WIGGLE_ANIM_SPEED: f32 = 5.0;

/// `efManagerYoshiEggLaySetAnim`'s indices into
/// `dEFManagerYoshiEggLayAnimJoints`, plus the `EFDesc`'s own Throw table.
pub const EGG_ANIM_WAIT: u8 = 0;
pub const EGG_ANIM_BREAK: u8 = 1;
pub const EGG_ANIM_THROW: u8 = 2;
/// The `gcPlayAnimAll` call, counting the make's, on which the Throw
/// animation (`llYoshiSpecial3EggLayThrowAnimJoint`) reaches its end
/// (`crates/ssb-rom/tests/display_effects.rs`).
pub const EGG_THROW_PLAYS: u16 = 17;

/// `dFTCommonYoshiEggDamageCollDescs[fkind].effect_size`: the egg's X and Y
/// scale, by `FTKind`.
pub const EGG_EFFECT_SIZES: [f32; 27] = [
    2.0, 1.9, 3.5, 2.2, 2.2, 2.0, 2.5, 2.2, 1.8, 1.8, 2.0, 1.8, 2.0, 2.0, 2.0, 1.9, 3.5, 2.2, 2.2,
    2.0, 2.5, 2.2, 1.8, 1.8, 2.0, 1.8, 5.7,
];

/// `efManagerYoshiEggLayMakeEffect`'s egg, as its process and the egg
/// status's callbacks drive it (RE-417). Drawing only: the escape keeps
/// counting [`EGG_BREAK_FRAMES`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EggLayEffect {
    /// Whether `ftCommonYoshiEggMakeEffect` has made it.
    pub made: bool,
    /// `effect_vars.yoshi_egg_lay.index` and `force_index`.
    pub index: u8,
    pub force_index: u8,
    /// Bumped by every `efManagerYoshiEggLaySetAnim` (and the make).
    pub epoch: u8,
    /// `gcPlayAnimAll` calls since the animation was set.
    pub plays: u16,
    /// `gcSetAnimSpeed`: [`WIGGLE_ANIM_SPEED`] on a mash, 1 otherwise.
    pub speed: f32,
    /// `ftCommonYoshiEggProcInterrupt`'s translate of the tree's root.
    pub wiggle: [f32; 2],
}

impl Default for EggLayEffect {
    fn default() -> Self {
        EggLayEffect {
            made: false,
            index: EGG_ANIM_THROW,
            force_index: EGG_ANIM_THROW,
            epoch: 0,
            plays: 0,
            speed: 1.0,
            wiggle: [0.0; 2],
        }
    }
}

impl EggLayEffect {
    /// `efManagerYoshiEggLayMakeEffect`: the Throw table, played once.
    fn make(&mut self) {
        *self = EggLayEffect {
            made: true,
            epoch: self.epoch.wrapping_add(1),
            plays: 1,
            ..EggLayEffect::default()
        };
    }

    /// `efManagerYoshiEggLayProcUpdate`.
    fn process(&mut self) {
        if self.force_index != self.index {
            self.index = self.force_index;
            self.epoch = self.epoch.wrapping_add(1);
            self.plays = 0;
        }
        self.plays = self.plays.saturating_add(1);
        if self.index == EGG_ANIM_THROW && self.plays >= EGG_THROW_PLAYS {
            self.force_index = EGG_ANIM_WAIT;
        }
    }
}

/// The egg a fighter is trapped in: the effect while the fighter is in
/// `YoshiEgg` and `ftCommonYoshiEggMakeEffect` has made it.
pub fn egg_effect(f: &Fighter) -> Option<&EggLayEffect> {
    (f.status.status == Status::YoshiEgg && f.egg.effect.made).then_some(&f.egg.effect)
}

/// `status_vars.common.captureyoshi`, and the break animation's clock.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct CaptureYoshiState {
    /// `stage`: 0 caught, 1 swallow requested, 2 swallowed, 3 lay.
    pub stage: u8,
    /// `breakout_wait`: frames left before the egg starts to break.
    pub escape_wait: i32,
    /// Motion flag 0: the wait ran out and the egg is breaking.
    pub breaking: bool,
    /// Frames the break animation has played.
    pub break_frames: u8,
    /// `lr` of the Yoshi that laid the egg.
    pub lr: f32,
    /// `effect_gobj`: the egg drawn around the fighter (RE-417).
    pub effect: EggLayEffect,
}

/// `ftCommonCaptureYoshiProcCapture`, run on the swallowed fighter once its
/// catcher's snapshot has been delivered.
pub fn capture(f: &mut Fighter, catcher_port: u8, holder: Holder) {
    grab::drop_own_catch(f);
    f.grab.capture = Some(catcher_port);
    f.grab.holder = Some(holder);
    f.grab.is_catchstatus = false;
    f.facing = holder.facing.flipped();
    f.become_airborne();
    status::set_status(f, Status::CaptureYoshi, 0.0, StatusTiming::unknown());
    // `ftCommonCaptureYoshiProcCapture` plays the first frame (RE-474).
    status::play_anim_events(f);
    f.egg = CaptureYoshiState::default();
    f.grab.breakout_wait = 0;
    f.grab.capture_immune = true;
    physics::stop_all(&mut f.physics);
    f.hitstun = 0;
}

/// `ftCommonCaptureYoshiProcPhysics` after the held placement: only X and Z
/// follow the tongue, then the stage Yoshi's script last wrote applies.
pub fn update_held(f: &mut Fighter, attachment: Vec3) {
    f.pos.x = attachment.x;
    f.pos.z = attachment.z;
    // `mpCommonUpdateFighterProjectFloor`: the fighter stays airborne.
    f.situation = crate::fighter::Situation::Air;
    f.floor = None;
    match f.egg.stage {
        3 => set_egg(f),
        1 => {
            f.egg.stage = 2;
            f.is_invisible = true;
            f.is_shadow_hidden = true;
        }
        _ => {}
    }
}

/// `ftCommonYoshiEggSetStatus`, with `ftCommonYoshiEggProcStatus`.
fn set_egg(f: &mut Fighter) {
    let Some(holder) = f.grab.holder else {
        return;
    };
    f.become_airborne();
    status::set_status(
        f,
        Status::YoshiEgg,
        0.0,
        StatusTiming {
            anim_length: None,
            anim_speed: 0.0,
            looping: false,
        },
    );
    f.egg.escape_wait = ESCAPE_WAIT_MAX;
    f.egg.breaking = false;
    f.egg.break_frames = 0;
    f.egg.effect = EggLayEffect {
        epoch: f.egg.effect.epoch,
        ..EggLayEffect::default()
    };
    f.grab.capture_immune = true;
    f.is_invisible = true;
    grab::init_breakout(f, BREAKOUT_INPUTS_MIN);
    f.add_damage(i32::from(LAY_DAMAGE));
    let player = f.grab.capture;
    f.record_combo_damage(player, i32::from(LAY_DAMAGE));
    crate::spgame::live::hit(
        f,
        player.map_or(
            crate::combat::DamageBy::World,
            crate::combat::DamageBy::Player,
        ),
        holder.stat,
        crate::spgame::bonus::DamageObject::Other,
    );
    let lr = holder.facing.sign();
    f.pos = holder.pos + Vec3::new(-lr * LAY_OFF_X, LAY_OFF_Y, 0.0);
    f.physics.vel_air = Vec3::new(-lr * LAY_VEL_X, LAY_VEL_Y, 0.0);
    f.grab.capture = None;
    f.grab.holder = None;
    f.egg.lr = lr;
}

/// `ftCommonYoshiEggProcUpdate`: the egg ends on the frame after the break
/// animation finishes.
///
/// The egg effect's own process (`efManagerYoshiEggLayProcUpdate`, in the
/// effect link after the fighters) runs here first, for the frame before,
/// as the port's display effects do; an effect made this frame gets its
/// first process next frame.
pub fn update_egg(f: &mut Fighter) {
    if f.egg.effect.made {
        f.egg.effect.process();
    } else {
        // `ftCommonYoshiEggMakeEffect`.
        f.egg.effect.make();
    }
    if f.egg.breaking && f.egg.break_frames >= EGG_BREAK_FRAMES {
        escape(f);
    }
}

fn escape(f: &mut Fighter) {
    f.physics.vel_air = Vec3::new(0.0, ESCAPE_VEL_Y, 0.0);
    f.pos.z = 0.0;
    f.pos.y += ESCAPE_OFF_Y;
    f.become_airborne();
    status::set_any_status_preserve(
        f,
        Status::Fall.into(),
        0.0,
        StatusTiming::unknown(),
        status::Preserve::DAMAGE_PLAYER,
    );
    f.grab.capture_immune = false;
    // `ftParamSetTimedHitStatusIntangible`: the port's timed invincibility
    // is the one that lets attacks pass through.
    f.invincible_frames = f.invincible_frames.max(INTANGIBLE_TIMER);
    f.interface.tag_wait = 1;
    crate::colanim::check_set(f, crate::colanim::ColAnimId::FIGHTER_NO_DAMAGE, 0);
}

/// The bookkeeping half of `ftCommonYoshiEggProcPhysics`, then the break
/// effect's own update for this frame. The ground friction or the air drift
/// with fast fall that follows is the common physics.
pub fn physics(f: &mut Fighter) {
    // `ftCommonYoshiEggProcInterrupt`: the egg leans with the stick on the
    // ground.
    let lean = |v: i8| {
        let v = i32::from(v);
        if v.abs() >= WIGGLE_STICK_RANGE_MIN {
            WIGGLE_GFX_RANGE_XY * if v < 0 { -1.0 } else { 1.0 }
        } else {
            0.0
        }
    };
    f.egg.effect.wiggle = if f.is_grounded() {
        [lean(f.stick.x), lean(f.stick.y)]
    } else {
        [0.0; 2]
    };
    if !f.egg.breaking {
        let before = f.grab.breakout_wait;
        let mashed = grab::update_breakout(f);
        // `gcSetAnimSpeed` on the effect.
        f.egg.effect.speed = if mashed { WIGGLE_ANIM_SPEED } else { 1.0 };
        f.egg.escape_wait -= (before - f.grab.breakout_wait) * MASH_FRAMES;
        let wait = f.egg.escape_wait;
        f.egg.escape_wait -= 1;
        if wait <= 0 {
            f.egg.breaking = true;
            f.egg.escape_wait = ESCAPE_WAIT_DEFAULT;
        }
    }
    if f.egg.breaking {
        f.egg.effect.force_index = EGG_ANIM_BREAK;
        f.egg.break_frames = f.egg.break_frames.saturating_add(1);
    }
}

/// `ftCommonYoshiEggProcTrap` for a registered hit: the egg takes half
/// damage (`damage_mul`), no knockback or damage status
/// (`damage_kind = nFTDamageKindNone`), and each point of it takes four
/// frames off the escape wait.
pub fn on_hit(f: &mut Fighter, damage: i32) {
    let queued = (damage as f32 * DAMAGE_MUL + 0.999) as i32;
    f.add_damage(queued);
    if !f.egg.breaking {
        f.egg.escape_wait -= ((2.0 * queued as f32) / 0.5) as i32;
    }
}

/// `ftCommonYoshiEggProcMap`, grounded half: the egg keeps its status.
pub fn on_ground_lost(f: &mut Fighter) -> bool {
    if f.status.status != Status::YoshiEgg {
        return false;
    }
    f.become_airborne();
    true
}

/// `ftCommonYoshiEggProcMap`, aerial half.
pub fn on_landing(f: &mut Fighter, floor_y: f32) -> bool {
    if f.status.status != Status::YoshiEgg {
        return false;
    }
    f.land(floor_y);
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fighter::{Facing, FighterKind, Situation};
    use ssb_engine::input::{ControllerState, N64Buttons};

    fn holder() -> Holder {
        Holder {
            stat: crate::spgame::live::AttackStat::default(),
            kind: FighterKind::Yoshi,
            pos: Vec3::new(100.0, 0.0, 0.0),
            facing: Facing::Right,
            status: crate::status::AnyStatus::Yoshi(crate::status::YoshiStatus::SpecialNRelease),
            anchor: Vec3::new(300.0, 120.0, 0.0),
            anchor_transform: None,
            floor_line: Some(0),
            coll: crate::ground::BodyColl::default(),
            percent: 0,
            handicap: crate::stale::HANDICAP_DEFAULT,
            kirby_dist: ssb_engine::math::Vec2::ZERO,
        }
    }

    fn swallowed() -> Fighter {
        let mut f = Fighter::new(FighterKind::Mario, 1, 3);
        f.situation = Situation::Ground;
        f.pos = Vec3::new(300.0, 0.0, 0.0);
        capture(&mut f, 0, holder());
        f
    }

    fn mash(f: &mut Fighter, down: bool) {
        f.set_input(
            ControllerState {
                buttons: N64Buttons(if down { N64Buttons::A } else { 0 }),
                ..Default::default()
            },
            false,
            false,
        );
    }

    #[test]
    fn swallow_hides_then_lays_an_egg_behind_yoshi() {
        let mut f = swallowed();
        assert_eq!(f.status.status, Status::CaptureYoshi);
        assert_eq!(f.facing, Facing::Left);
        update_held(&mut f, Vec3::new(280.0, 500.0, 0.0));
        assert_eq!(f.pos, Vec3::new(280.0, 0.0, 0.0), "Y does not follow");
        f.egg.stage = 1;
        update_held(&mut f, Vec3::new(280.0, 0.0, 0.0));
        assert_eq!(f.egg.stage, 2);
        assert!(f.is_invisible);
        f.egg.stage = 3;
        update_held(&mut f, Vec3::new(280.0, 0.0, 0.0));
        assert_eq!(f.status.status, Status::YoshiEgg);
        assert_eq!(f.pos, Vec3::new(-100.0, 90.0, 0.0));
        assert_eq!(f.physics.vel_air, Vec3::new(-20.0, 60.0, 0.0));
        assert_eq!(f.damage, 5);
        assert!(f.is_invisible && f.grab.capture.is_none());
        assert_eq!(f.egg.escape_wait, ESCAPE_WAIT_MAX);
    }

    fn egg() -> Fighter {
        let mut f = swallowed();
        f.egg.stage = 3;
        update_held(&mut f, Vec3::ZERO);
        f
    }

    /// Frames until the egg breaks open, driving the source callback order.
    fn frames_to_escape(f: &mut Fighter, mash_every: Option<u32>) -> u32 {
        for frame in 1..=400 {
            if let Some(n) = mash_every {
                mash(f, frame % n == 0);
            }
            update_egg(f);
            if f.status.status != Status::YoshiEgg {
                return frame;
            }
            physics(f);
        }
        panic!("the egg never broke");
    }

    #[test]
    fn an_unmashed_egg_breaks_after_the_wait_and_the_break_animation() {
        let mut f = egg();
        // The wait counts 250 down to 0 on physics frames 1..=251; the
        // break animation plays on 251..=260; frame 261's update escapes.
        assert_eq!(frames_to_escape(&mut f, None), 261);
        assert_eq!(f.status.status, Status::Fall);
        assert_eq!(f.physics.vel_air, Vec3::new(0.0, ESCAPE_VEL_Y, 0.0));
        assert_eq!(f.invincible_frames, INTANGIBLE_TIMER);
        assert!(!f.is_invisible && !f.grab.capture_immune);
    }

    #[test]
    fn mashing_takes_twelve_frames_per_input_off_the_wait() {
        let mut slow = egg();
        let mut fast = egg();
        let unmashed = frames_to_escape(&mut slow, None);
        let mashed = frames_to_escape(&mut fast, Some(2));
        assert!(mashed < unmashed / 5, "{mashed} vs {unmashed}");
    }

    /// `efManagerYoshiEggLayProcUpdate` (RE-417): made on the egg's first
    /// update with the Throw table played once, Wait from the play after
    /// Throw ends, Break once the wait runs out.
    #[test]
    fn the_egg_effect_throws_then_waits_then_breaks() {
        let mut f = egg();
        assert!(egg_effect(&f).is_none());
        update_egg(&mut f);
        let e = *egg_effect(&f).unwrap();
        assert_eq!((e.index, e.plays), (EGG_ANIM_THROW, 1));
        let epoch = e.epoch;
        for n in 2..=EGG_THROW_PLAYS {
            physics(&mut f);
            update_egg(&mut f);
            assert_eq!(
                (f.egg.effect.index, f.egg.effect.plays),
                (EGG_ANIM_THROW, n)
            );
        }
        physics(&mut f);
        update_egg(&mut f);
        assert_eq!((f.egg.effect.index, f.egg.effect.plays), (EGG_ANIM_WAIT, 1));
        assert_eq!(f.egg.effect.epoch, epoch.wrapping_add(1));
        f.egg.escape_wait = 0;
        physics(&mut f);
        assert!(f.egg.breaking);
        update_egg(&mut f);
        assert_eq!(
            (f.egg.effect.index, f.egg.effect.plays),
            (EGG_ANIM_BREAK, 1)
        );
        // The break's ten frames, then the escape ends the effect.
        for _ in 0..EGG_BREAK_FRAMES {
            physics(&mut f);
            update_egg(&mut f);
        }
        assert!(egg_effect(&f).is_none());
    }

    /// `ftCommonYoshiEggProcPhysics`'s `gcSetAnimSpeed` and
    /// `ftCommonYoshiEggProcInterrupt`'s lean.
    #[test]
    fn a_mash_speeds_the_egg_and_the_stick_leans_it_on_the_ground() {
        let mut f = egg();
        update_egg(&mut f);
        mash(&mut f, true);
        physics(&mut f);
        assert_eq!(f.egg.effect.speed, WIGGLE_ANIM_SPEED);
        mash(&mut f, false);
        physics(&mut f);
        assert_eq!(f.egg.effect.speed, 1.0);
        assert_eq!(f.egg.effect.wiggle, [0.0; 2], "airborne");
        f.situation = Situation::Ground;
        f.set_input(
            ControllerState {
                stick_x: -40,
                stick_y: 20,
                ..Default::default()
            },
            false,
            false,
        );
        physics(&mut f);
        assert_eq!(f.egg.effect.wiggle, [-WIGGLE_GFX_RANGE_XY, 0.0]);
    }

    #[test]
    fn a_hit_on_the_egg_halves_the_damage_and_shortens_the_wait() {
        let mut f = egg();
        on_hit(&mut f, 9);
        assert_eq!(f.damage, 5 + 5);
        assert_eq!(f.egg.escape_wait, ESCAPE_WAIT_MAX - 20);
        assert_eq!(f.status.status, Status::YoshiEgg);
    }
}
