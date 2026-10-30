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
//! * **Effect room.** `ftCommonYoshiEggProcUpdate` falls back to counting
//!   `breakout_wait` when `efManagerYoshiEggLayMakeEffect` finds no room.
//!   The port's egg is not drawn from the effect pool, so it always has
//!   room and that branch (`flag0 == 1`, `breakout_wait-- <= 0`, 16
//!   frames instead of the 10-frame break) cannot run; it is not ported.

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

/// `dFTCommonYoshiEggDamageCollDescs` by `FTKind`: `(effect_size, offset,
/// size)`, generated at build time from the ROM by `crates/ssb-tablegen`
/// (`battle_data.rs`). `effect_size` is the egg's X and Y scale.
pub use generated::EGG_DESCS;
mod generated {
    include!(concat!(env!("OUT_DIR"), "/yoshi_egg.rs"));
}

/// `dFTCommonYoshiEggDamageCollDescs[fkind].effect_size`.
pub fn egg_effect_size(kind: crate::fighter::FighterKind) -> Option<f32> {
    EGG_DESCS.get(kind as usize).map(|d| d.0)
}

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
    /// `is_damagefloor`: acid hit the egg ([`on_hit`]).
    pub is_damagefloor: bool,
}

/// `ftCommonCaptureYoshiProcCapture`, run on the swallowed fighter once its
/// catcher's snapshot has been delivered.
pub fn capture(f: &mut Fighter, catcher_port: u8, holder: Holder) {
    // `ftParamStopVoiceRunProcDamage`.
    crate::fighter_sound::stop_voice(f);
    crate::item_throw::drop_heavy_item(f);
    grab::drop_own_catch(f);
    f.grab.capture = Some(catcher_port);
    f.grab.holder = Some(holder);
    f.grab.is_catch_or_capture = false;
    f.grab.is_catchstatus = false;
    f.facing = holder.facing.flipped();
    // `mpCommonSetFighterAir`, unconditional (`ftcommoncaptureyoshi.c:112`).
    f.set_fighter_air();
    status::set_status(f, Status::CaptureYoshi, 0.0, StatusTiming::unknown());
    // `ftCommonCaptureYoshiProcCapture` plays the first frame (RE-474).
    status::play_anim_events(f);
    f.egg = CaptureYoshiState::default();
    f.grab.breakout_wait = 0;
    f.grab.capture_immune_mask = crate::grab::catch_kind::ALL;
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
            crate::hurtbox::set_hit_status_all(f, crate::combat::HitStatus::Intangible);
        }
        _ => {}
    }
}

/// `ftCommonYoshiEggSetStatus`, with `ftCommonYoshiEggProcStatus`.
fn set_egg(f: &mut Fighter) {
    let Some(holder) = f.grab.holder else {
        return;
    };
    // `ftcommoncaptureyoshi.c:353-359`: guarded, then again unconditionally.
    f.set_fighter_air();
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
    f.grab.capture_immune_mask = crate::grab::catch_kind::ALL;
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
    // `ftParamSetPlayerTagWait(fighter_gobj, 1)`: the tag marks the hidden
    // fighter for the egg's whole life.
    f.interface.tag_wait = 1;
    let lr = holder.facing.sign();
    f.pos = holder.pos + Vec3::new(-lr * LAY_OFF_X, LAY_OFF_Y, 0.0);
    // `mpCommonRunFighterCollisionDefault(fighter_gobj, &Yoshi's
    // translate, &Yoshi's coll_data)`: run by the caller, which has the map
    // ([`take_lay_sweep`]).
    f.grab.release_sweep = Some((holder.pos, holder.coll));
    f.physics.vel_air = Vec3::new(-lr * LAY_VEL_X, LAY_VEL_Y, 0.0);
    // `damage_mul = FTCOMMON_YOSHIEGG_DAMAGE_MUL` (ftcommoncaptureyoshi.c:381):
    // the egg's hits register at half (`ftParamGetCapturedDamage`) until
    // `ftMainSetStatus` resets it.
    f.damage_mul = DAMAGE_MUL;
    f.grab.capture = None;
    f.grab.holder = None;
    f.egg.lr = lr;
    f.egg.is_damagefloor = false;
}

/// The lay's map sweep, `(Yoshi's TopN, Yoshi's diamond)`, on the frame
/// [`update_held`] laid the egg; `None` on every other frame.
pub fn take_lay_sweep(f: &mut Fighter) -> Option<(Vec3, crate::ground::BodyColl)> {
    if f.status.status != Status::YoshiEgg {
        return None;
    }
    f.grab.release_sweep.take()
}

/// The egg effect's process in the fighter's hitlag.
/// `efManagerYoshiEggLayProcUpdate` never reads `is_pause_effect` and the
/// egg sets no `proc_lagstart`, so the egg plays through the hitlag of a
/// hit on it: [`Fighter`]'s update calls this in hitlag, where
/// [`update_egg`] does not run.
pub fn tick_egg_effect(f: &mut Fighter) {
    if egg_effect(f).is_some() {
        f.egg.effect.process();
    }
}

/// `ftCommonYoshiEggProcUpdate`: the egg ends on the frame after the break
/// animation finishes.
///
/// The egg effect's own process (`efManagerYoshiEggLayProcUpdate`, in the
/// effect link after the fighters) runs here first, for the frame before,
/// as the port's display effects do; an effect made this frame gets its
/// first process next frame.
///
/// On a damaging floor the effect stops and the egg breaks at once.
pub fn update_egg(f: &mut Fighter) {
    let is_escape = if f.egg.is_damagefloor {
        // `ftParamProcStopEffect`.
        f.egg.effect.made = false;
        true
    } else {
        if f.egg.effect.made {
            f.egg.effect.process();
        } else {
            // `ftCommonYoshiEggMakeEffect`: the port's egg always has room.
            f.egg.effect.make();
        }
        f.egg.breaking && f.egg.break_frames >= EGG_BREAK_FRAMES
    };
    if is_escape {
        escape(f);
    }
}

/// The escape half of `ftCommonYoshiEggProcUpdate` (`:179-196`).
fn escape(f: &mut Fighter) {
    // The effects' copy of TopN's translation has `z` zeroed; TopN keeps
    // its own.
    let pos = Vec3::new(f.pos.x, f.pos.y, 0.0);
    f.effects
        .push(crate::fteffect::FighterEffect::YoshiEggExplode { pos });
    f.effects.push(crate::fteffect::FighterEffect::At {
        kind: crate::fteffect::kind::EGG_BREAK,
        pos,
    });
    crate::sound::play_fgm(crate::sound::id::nSYAudioFGMYoshiEggLayShatter);
    f.physics.vel_air = Vec3::new(0.0, ESCAPE_VEL_Y, 0.0);

    f.pos.y += ESCAPE_OFF_Y;
    // `mpCommonSetFighterAir`, unconditional (`ftcommoncaptureyoshi.c:193`).
    f.set_fighter_air();
    status::set_any_status_preserve(
        f,
        Status::Fall.into(),
        0.0,
        StatusTiming::unknown(),
        status::Preserve::DAMAGE_PLAYER,
    );
    // `ftParamSetTimedHitStatusIntangible` (`intangible_tics`), after the
    // Fall's `ftMainSetStatus` cleared the tag wait the egg set.
    crate::reaction::set_timed_intangible(f, INTANGIBLE_TIMER);
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

/// `ftCommonYoshiEggProcTrap` for a registered hit: the egg took half
/// damage (`damage_mul`), and takes no knockback or damage status
/// (`damage_kind = nFTDamageKindNone`), and each point of it takes four
/// frames off the escape wait.
///
/// Acid (`damage_object_class` Ground, `damage_object_kind` Acid, the
/// last `ftParamUpdate1PGameDamageStats`) breaks the egg on its next
/// update ([`update_egg`]).
pub fn on_hit(f: &mut Fighter, damage_queue: i32) {
    // `ftMainProcParams` already added the queue, which `damage_mul`
    // ([`set_egg`]) halved as the hit registered.
    if !f.egg.breaking {
        f.egg.escape_wait -= ((2.0 * damage_queue as f32) / 0.5) as i32;
    }
    if f.stats.damage_object == crate::spgame::bonus::DamageObject::Acid {
        f.egg.escape_wait = 0;
        f.egg.is_damagefloor = true;
    }
}

/// `ftCommonYoshiEggProcMap`, grounded half: the egg keeps its status and
/// only `ga` turns, like [`on_landing`] (the jump count is untouched).
pub fn on_ground_lost(f: &mut Fighter) -> bool {
    if f.status.status != Status::YoshiEgg {
        return false;
    }
    f.set_ga_air();
    true
}

/// `ftCommonYoshiEggProcMap`, aerial half: only `fp->ga` turns to ground,
/// not `mpCommonSetFighterGround`, so the flight's `vel_air.x` never
/// becomes a ground velocity (the egg stops dead where it lands), and the
/// jump count and landing params are untouched.
pub fn on_landing(f: &mut Fighter, floor_y: f32) -> bool {
    if f.status.status != Status::YoshiEgg {
        return false;
    }
    f.situation = crate::fighter::Situation::Ground;
    f.pos.y = floor_y;
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
        assert!(f.is_invisible && f.is_shadow_hidden);
        // `ftParamSetHitStatusAll(Intangible)` (`ftcommoncaptureyoshi.c:77`).
        assert_eq!(f.hitstatus, crate::combat::HitStatus::Intangible);
        assert!(crate::combat::is_body_intangible(&f));
        f.egg.stage = 3;
        update_held(&mut f, Vec3::new(280.0, 0.0, 0.0));
        assert_eq!(f.status.status, Status::YoshiEgg);
        // The egg's `ftMainSetStatus` restores `Normal` (`ftmain.c:4428`).
        assert_eq!(f.hitstatus, crate::combat::HitStatus::Normal);
        assert_eq!(f.pos, Vec3::new(-100.0, 90.0, 0.0));
        assert_eq!(f.physics.vel_air, Vec3::new(-20.0, 60.0, 0.0));
        assert_eq!(f.damage, 5);
        assert!(f.is_invisible && f.grab.capture.is_none());
        assert_eq!(f.egg.escape_wait, ESCAPE_WAIT_MAX);
        // `ftParamSetPlayerTagWait(fighter_gobj, 1)` after the egg's
        // `ftMainSetStatus` (`ftcommoncaptureyoshi.c:368`): the tag marks
        // the hidden fighter.
        assert_eq!(f.interface.tag_wait, 1);
    }

    fn egg() -> Fighter {
        let mut f = swallowed();
        f.egg.stage = 3;
        update_held(&mut f, Vec3::ZERO);
        f
    }

    /// `ftCommonYoshiEggSetStatus` (`:353-359`): `mpCommonSetFighterAir`
    /// runs again unconditionally, so an airborne swallowed fighter lays with
    /// `jumps_used` 1 whatever it held.
    #[test]
    fn the_egg_is_laid_with_one_jump_spent() {
        let mut f = swallowed();
        assert_eq!(f.situation, Situation::Air);
        f.physics.jumps_used = 2;
        f.egg.stage = 3;
        update_held(&mut f, Vec3::ZERO);
        assert_eq!(f.status.status, Status::YoshiEgg);
        assert_eq!(f.physics.jumps_used, 1);
    }

    /// `ftCommonYoshiEggProcMap` (`:283`): an egg leaving its floor turns
    /// only `ga`; the jump count and depth stay as they were.
    #[test]
    fn an_egg_leaving_its_floor_only_turns_airborne() {
        let mut f = egg();
        f.situation = Situation::Ground;
        f.physics.jumps_used = 0;
        f.pos.z = 5.0;
        assert!(on_ground_lost(&mut f));
        assert_eq!(f.situation, Situation::Air);
        assert_eq!(f.physics.jumps_used, 0);
        assert_eq!(f.pos.z, 5.0);
    }

    /// `ftCommonYoshiEggProcTrap`: an acid hit (`damage_object_class`
    /// Ground, kind Acid) zeroes the wait and sets `is_damagefloor`;
    /// `ftCommonYoshiEggProcUpdate` then stops the egg effect and escapes
    /// at once, without the break animation. Any other hit only shortens
    /// the wait.
    #[test]
    fn an_acid_hit_breaks_the_egg_on_its_next_update() {
        use crate::spgame::bonus::DamageObject;
        let mut f = egg();
        update_egg(&mut f);
        f.stats.damage_object = DamageObject::Bumper;
        on_hit(&mut f, 10);
        update_egg(&mut f);
        assert_eq!(f.status.status, Status::YoshiEgg, "a bumper hit");
        assert!(!f.egg.is_damagefloor);
        f.stats.damage_object = DamageObject::Acid;
        on_hit(&mut f, 10);
        assert!(f.egg.is_damagefloor);
        assert_eq!(f.egg.escape_wait, 0);
        assert!(!f.egg.breaking, "no break animation");
        update_egg(&mut f);
        assert_eq!(f.status.status, Status::Fall);
        assert!(!f.egg.effect.made, "ftCommonYoshiEggProcStopEffect");
        // A new egg starts clear (`ftCommonYoshiEggSetStatus`).
        let mut f = egg();
        f.egg.is_damagefloor = true;
        f.status.status = Status::CaptureYoshi.into();
        f.grab.holder = Some(holder());
        f.egg.stage = 3;
        update_held(&mut f, Vec3::ZERO);
        assert!(!f.egg.is_damagefloor);
    }

    /// `ftCommonYoshiEggSetStatus`'s `mpCommonRunFighterCollisionDefault`:
    /// the lay offset is swept from Yoshi with Yoshi's diamond, so a wall
    /// between Yoshi and the lay point stops the egg; the lay velocity then
    /// moves it the same frame and the egg's `proc_map` tests from Yoshi.
    #[test]
    fn the_egg_is_laid_against_a_wall_behind_yoshi() {
        use crate::collision::Segment;
        use crate::weapon::{MapSurface, MapSurfaceKind, SurfaceTopology};
        let wall = MapSurface {
            motion: None,
            kind: MapSurfaceKind::RightWall,
            segment: Segment {
                x1: -40,
                y1: -500,
                x2: -40,
                y2: 500,
                flags: 0,
            },
            topology: Some(SurfaceTopology {
                line: 0,
                point: 0,
                segments: 1,
                vertex1: 0,
                vertex2: 1,
            }),
        };
        let surfaces = || core::iter::once(wall);
        let mut f = swallowed();
        let mut h = holder();
        h.coll = crate::ground::BodyColl {
            top: 400.0,
            center: 250.0,
            bottom: 0.0,
            width: 50.0,
        };
        f.grab.holder = Some(h);
        f.coll = crate::ground::BodyColl {
            top: 200.0,
            center: 100.0,
            bottom: 0.0,
            width: 30.0,
        };
        f.egg.stage = 3;
        f.tick_physics_map(&surfaces);
        assert_eq!(f.status.status, Status::YoshiEgg);
        // Unswept, the egg would stand at x -100 - 20, behind the wall.
        assert_eq!(f.pos.x, -40.0 + 30.0, "{:?}", f.pos);
        assert_eq!(f.pos.y, 90.0 + 60.0, "laid and moved the same frame");
        assert!(f.grab.release_sweep.is_none());
    }

    /// `efManagerYoshiEggLayProcUpdate` never reads `is_pause_effect` and
    /// the egg sets no `proc_lagstart`: a hit on the egg (`damage_lag`
    /// hitlag) holds the fighter, not the egg's animation.
    #[test]
    fn the_egg_effect_keeps_playing_through_hitlag() {
        let surfaces = || core::iter::empty::<crate::weapon::MapSurface>();
        let mut f = egg();
        for _ in 0..3 {
            update_egg(&mut f);
        }
        assert_eq!(
            (f.egg.effect.index, f.egg.effect.plays),
            (EGG_ANIM_THROW, 3)
        );
        let frame = f.status.anim_frame;
        f.hitlag = 5;
        for t in 1..=4 {
            f.tick_interrupt(&surfaces);
            assert!(f.is_in_hitlag());
            assert_eq!(f.egg.effect.plays, 3 + t, "hitlag tick {t}");
        }
        assert_eq!(f.status.anim_frame, frame, "the fighter is held");
        // The last hitlag tick runs the full update: one play, not two.
        f.tick_interrupt(&surfaces);
        assert!(!f.is_in_hitlag());
        assert_eq!(f.egg.effect.plays, 8);
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
        // `ftParamSetTimedHitStatusIntangible` writes `intangible_tics`
        // (`ftcommoncaptureyoshi.c:195`; ROM `0x8014CB08` calls `0x800EA948`,
        // which stores `0x5A8`), not the invincibility timer.
        assert_eq!(f.intangible_frames, INTANGIBLE_TIMER);
        assert_eq!(f.invincible_frames, 0);
        assert_eq!(
            crate::combat::special_hitstatus(&f),
            crate::combat::HitStatus::Intangible
        );
        // The Fall's `ftMainSetStatus` clears the tag wait
        // (`FTSTATUS_PRESERVE_DAMAGEPLAYER` only); the escape sets none.
        assert_eq!(f.interface.tag_wait, 0);
        assert!(!f.is_invisible && f.grab.capture_immune_mask == 0);
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

    /// `ftCommonYoshiEggProcUpdate`'s escape (`:179-196`): the explosion
    /// (Yoshi's particle script 3) and the shell (`efManagerEggBreakMakeEffect`)
    /// at TopN's translation with `z` zeroed, then the shatter FGM; TopN
    /// rises by `ESCAPE_OFF_Y` and the unconditional `mpCommonSetFighterAir`
    /// (`ftcommoncaptureyoshi.c:193`) zeroes its `z` and spends the ground
    /// jump, even for an egg already airborne.
    #[test]
    fn the_escape_explodes_and_breaks_the_egg_then_plays_the_shatter() {
        use crate::fteffect::{kind, FighterEffect};
        let r = crate::sound::testing::Recorder::install();
        let mut f = egg();
        f.pos = Vec3::new(120.0, 400.0, 7.0);
        update_egg(&mut f);
        f.egg.escape_wait = 0;
        assert_eq!(f.situation, crate::fighter::Situation::Air);
        f.physics.jumps_used = 0;
        let mut frames = 0;
        while f.status.status == Status::YoshiEgg {
            f.effects.clear();
            let _ = r.take();
            physics(&mut f);
            update_egg(&mut f);
            frames += 1;
            assert!(frames < 200, "never escapes");
        }
        assert_eq!(f.status.status, Status::Fall);
        let pos = Vec3::new(120.0, 400.0, 0.0);
        let queued: std::vec::Vec<_> = f.effects.iter().copied().collect();
        assert_eq!(
            queued[..2],
            [
                FighterEffect::YoshiEggExplode { pos },
                FighterEffect::At {
                    kind: kind::EGG_BREAK,
                    pos
                }
            ]
        );
        assert_eq!(
            r.take(),
            [crate::sound::testing::Call::PlayFgm(
                crate::sound::id::nSYAudioFGMYoshiEggLayShatter
            )]
        );
        assert_eq!(f.pos, Vec3::new(120.0, 400.0 + ESCAPE_OFF_Y, 0.0));
        assert_eq!(f.physics.jumps_used, 1);
        crate::sound::uninstall();
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

    /// `damage_mul` 0.5 halves the hit as it registers
    /// (`ftParamGetCapturedDamage`); `ftMainProcParams` adds that once and
    /// `ftCommonYoshiEggProcTrap` takes `2 * damage_queue / 0.5` frames off
    /// the wait, with no damage status.
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
    #[test]
    fn a_hit_on_the_egg_counts_its_halved_damage_once() {
        let mut f = egg();
        assert_eq!(f.damage, 5);
        let hitbox = crate::attack::Hitbox {
            damage: 9,
            ..crate::attack::mario_jab1_hitbox()
        };
        assert!(crate::combat::direct_hit(
            &mut f,
            hitbox,
            Vec3::new(-100.0, 0.0, 0.0),
            1.0,
            9,
            crate::combat::DamageBy::Player(0),
        ));
        assert_eq!(f.hits.damage_queue, 5);
        crate::combat::finish_frame(&mut [&mut f]);
        assert_eq!(f.damage, 5 + 5);
        assert_eq!(f.egg.escape_wait, ESCAPE_WAIT_MAX - 20);
        assert_eq!(f.status.status, Status::YoshiEgg);
    }
}
