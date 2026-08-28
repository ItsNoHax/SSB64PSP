//! The fighter status machine, ported from `src/ft/ftcommon/`.
//!
//! A fighter is always in exactly one *status*. The status decides which
//! physics run, which inputs are listened to, and which other statuses can be
//! entered — the original splits this into four callbacks per status
//! (`proc_update`, `proc_interrupt`, `proc_physics`, `proc_map`) and this
//! module is the movement subset of those.
//!
//! ## The interrupt chain is an ordered list, not a match
//!
//! `ftCommonGroundCheckInterrupt` is a macro of nineteen `||`-chained checks,
//! each of which *sets a status as a side effect* and returns whether it did.
//! The order is the priority order, and it is load-bearing: jumpsquat is
//! tested before dash, dash before squat, squat before turn, turn before walk.
//! Hold down-and-forward on the frame you tap and you get a jumpsquat, not a
//! dash, because of where those two sit in the list. [`ground_interrupt`]
//! preserves the order for the checks that are ported.
//!
//! ## `tap_stick_x` is a counter, not an edge
//!
//! This is the single most important thing in the input model, and it is not
//! what "tap" suggests. `ftMainProcessInput` keeps a per-axis counter that
//! **resets to 1 on the frame the stick crosses ±20**, increments while it
//! stays outside, and is pinned to 254 whenever it is inside. A dash needs
//! `|stick_x| >= 56 && tap_stick_x < 3`.
//!
//! So the window is measured from the *deadzone* crossing at 20, not from
//! reaching the action's own threshold at 56. Flick from neutral and you cross
//! 20 and 56 on the same frame, so `tap_stick_x == 1` and you dash. Roll the
//! stick out slowly and you cross 20 several frames before 56, the counter is
//! past 3 by then, and you walk instead. That is the whole tap-vs-tilt
//! distinction, and it falls out of one counter rather than any explicit
//! gesture recognition.
//!
//! ## What is not here
//!
//! Attacks, specials, grabs, shields, taunts, ledges, damage and item states.
//! Each is a status in the same table with the same shape, but none of the
//! systems they need (hitboxes, movesets, items) exist yet.
//!
//! Statuses also cannot *time out* where the original ends them on the
//! animation running out, because animation data is not extracted — see
//! [`StatusTiming`].

use crate::fighter::{Facing, Fighter, Situation};
use crate::physics::{self, PhysicsAttributes, PhysicsState};

/// Maximum control-stick deflection — `I_CONTROLLER_RANGE_MAX`.
pub const STICK_MAX: i32 = 80;

/// Deflection at which the tap counters start counting, from
/// `ftMainProcessInput`. Not a per-action threshold: every action's tap window
/// is measured from this crossing.
pub const STICK_DEADZONE: i32 = 20;

/// What the tap counters are pinned to while the stick is centred —
/// `FTINPUT_STICKBUFFER_TICS_MAX`. Any `tap < n` test fails at this value.
pub const STICKBUFFER_MAX: u8 = u8::MAX - 1;

/// Stick deflection required for the fastest walk — `FTCOMMON_WALKFAST_STICK_RANGE_MIN`.
pub const WALKFAST_STICK_MIN: i32 = 62;
/// Stick deflection required for a medium walk.
pub const WALKMIDDLE_STICK_MIN: i32 = 26;
/// Deflection *against* the facing direction that starts a turn.
pub const TURN_STICK_MIN: i32 = -20;
/// Frames within which a stick crossing counts as a dash input.
pub const DASH_BUFFER_TICS_MAX: u8 = 3;
/// Deflection required for a dash, on top of being inside the tap window.
pub const DASH_STICK_MIN: i32 = 56;
/// Frames into a dash before its deceleration begins.
pub const DASH_DECELERATE_BEGIN: f32 = 7.0;
/// What a dash keeps of its ground speed when its animation ends
/// (`ftCommonDashProcUpdate`).
pub const DASH_END_VEL_MUL: f32 = 0.75;
/// Deflection needed to hold a run.
pub const RUN_STICK_MIN: i32 = 50;
/// Frames within which an upward stick crossing counts as a jump input.
pub const KNEEBEND_BUFFER_TICS_MAX: u8 = 3;
/// Upward deflection that starts a jumpsquat from a standing state.
pub const KNEEBEND_STICK_MIN: i32 = 53;
/// Upward deflection that starts a jumpsquat out of a run. Lower than the
/// standing threshold, so a running fighter jumps more readily.
pub const KNEEBEND_RUN_STICK_MIN: i32 = 44;
/// Stick X, relative to facing, below which a jump goes backward.
pub const KNEEBEND_JUMP_F_OR_B_RANGE: i32 = -10;
/// Frames of jumpsquat within which releasing the jump button short-hops.
///
/// Smash 64 buffers the whole jumpsquat, unlike Melee where only the last
/// frame's button state matters.
pub const KNEEBEND_SHORTHOP_FRAMES: f32 = 3.0;
/// Jump forces for a button jump, from `FTCOMMON_KNEEBEND_BUTTON_*`.
pub const KNEEBEND_BUTTON_SHORT_FORCE: f32 = 9.0;
pub const KNEEBEND_BUTTON_LONG_FORCE: f32 = 17.0;
pub const KNEEBEND_BUTTON_SHORT_MIN: f32 = 36.0;
pub const KNEEBEND_BUTTON_LONG_MIN: f32 = 63.0;
pub const KNEEBEND_BUTTON_HEIGHT_CLAMP: f32 = 77.0;
/// Downward deflection that squats, and the window it must arrive in.
pub const SQUAT_STICK_MIN: i32 = -53;
pub const SQUAT_BUFFER_TICS_MAX: u8 = 4;
/// Downward deflection that drops through a passable floor.
pub const PASS_STICK_MIN: i32 = -53;
pub const PASS_BUFFER_TICS_MAX: u8 = 4;

/// A fighter's status, with `FTCommonStatus` ordinals preserved exactly.
///
/// The ordinals index per-character status tables in the ROM, so renumbering
/// would silently mis-associate every fighter's data. Only the statuses this
/// module implements are listed; the gaps are the unported ones and the
/// discriminants leave room for them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u16)]
pub enum Status {
    /// Standing still. `nFTCommonStatusControlStart` — the first status a
    /// player can act out of.
    Wait = 10,
    WalkSlow = 11,
    WalkMiddle = 12,
    WalkFast = 13,
    Dash = 15,
    Run = 16,
    RunBrake = 17,
    Turn = 18,
    /// Jumpsquat. The original's name is literal: the knees bend.
    KneeBend = 20,
    JumpF = 22,
    JumpB = 23,
    JumpAerialF = 24,
    JumpAerialB = 25,
    Fall = 26,
    /// Falling with no jumps left. Physically identical to [`Status::Fall`];
    /// the distinction drives which animation and aerials are available.
    FallAerial = 27,
    Squat = 28,
    SquatWait = 29,
    LandingLight = 31,
    LandingHeavy = 32,
    /// Dropping through a passable platform.
    Pass = 33,
}

impl Status {
    /// Which packed animation this status plays, as a slot index.
    ///
    /// The numbering is `ssb_rom::anim`'s, and it is repeated here rather than
    /// imported because Layer A must not depend on the pack format — the same
    /// arrangement `play::PhysicsAttributes` uses for the constants. The
    /// [`Status::ANIM_SLOTS`] test pins the two together.
    ///
    /// `LandingHeavy` shares `LandingLight`'s animation: they are one file
    /// played at different speeds, which is what [`Status::anim_speed`] is for
    /// (RE-035).
    pub fn anim_slot(self) -> usize {
        match self {
            Status::Dash => 0,
            Status::Turn => 1,
            Status::RunBrake => 2,
            Status::Squat => 3,
            // SquatRv — rising out of a crouch — is the animation the original
            // plays when the crouch ends; the status machine reaches it
            // through SquatWait rather than having a state of its own.
            Status::LandingLight | Status::LandingHeavy => 5,
            Status::Pass => 6,
            Status::Wait => 7,
            Status::WalkSlow => 8,
            Status::WalkMiddle => 9,
            Status::WalkFast => 10,
            Status::Run => 11,
            Status::KneeBend => 12,
            Status::JumpF => 13,
            Status::JumpB => 14,
            Status::JumpAerialF => 15,
            Status::JumpAerialB => 16,
            Status::Fall => 17,
            Status::FallAerial => 18,
            Status::SquatWait => 19,
        }
    }

    /// Playback rate for this status's animation.
    ///
    /// `ftCommonLandingSetStatus` passes 1.0 for a light landing and **0.5**
    /// for a heavy one, so the same seven-frame file takes fourteen frames
    /// after a fastfall. Storing a length without the speed would make both
    /// landings identical (RE-035).
    pub fn anim_speed(self) -> f32 {
        match self {
            Status::LandingHeavy => 0.5,
            _ => 1.0,
        }
    }

    /// Whether this status is a grounded one — the `ga` field, which the
    /// original sets through `mpCommonSetFighterGround` / `...Air`.
    pub fn is_grounded(self) -> bool {
        !matches!(
            self,
            Status::JumpF
                | Status::JumpB
                | Status::JumpAerialF
                | Status::JumpAerialB
                | Status::Fall
                | Status::FallAerial
                | Status::Pass
        )
    }

    /// Whether the player can act out of this status through the standard
    /// ground interrupt chain.
    pub fn is_actionable_on_ground(self) -> bool {
        matches!(
            self,
            Status::Wait
                | Status::WalkSlow
                | Status::WalkMiddle
                | Status::WalkFast
                | Status::Squat
                | Status::SquatWait
                | Status::LandingLight
                | Status::LandingHeavy
        )
    }

    /// Whether this is one of the three walks.
    pub fn is_walk(self) -> bool {
        matches!(
            self,
            Status::WalkSlow | Status::WalkMiddle | Status::WalkFast
        )
    }
}

/// How long a status's animation runs, when that is known.
///
/// `None` means the length lives in animation data (`AnimJoint` /
/// `AObjEvent32`) that is not extracted, so the status cannot end on its own.
/// It is deliberately not a guess: a wrong duration here would be invisible
/// in a screenshot and wrong in every replay.
///
/// The original does not have this type. It reads `gobj->anim_frame`, which
/// counts *up* by `anim_speed` each frame and, confusingly, is also tested as
/// `<= 0.0` to mean "finished" — when the animation script runs out,
/// `ftAnimParseDObjFigatree` writes the leftover negative remainder into it.
/// So `anim_frame <= 0.0` is a sentinel, while `anim_frame <= 5.0` a few lines
/// away in the same file is a genuine "within the first five frames" test.
/// Both readings are correct and they coexist.
///
/// Lengths come from two different places, and which one matters per status:
///
/// * From `FTAttributes`: [`Status::KneeBend`] (`kneebend_anim_length`, Mario
///   3, Link 7, Metal Mario 8), [`Status::Dash`] → [`Status::Run`]
///   (`dash_to_run`, Mario 14), and the three walks (`walk*_anim_length`, used
///   only to keep the animation phase continuous across a speed change).
/// * From the animation files themselves — see [`AnimLengths`] — for the
///   statuses that end when their figatree script runs out.
///
/// `anim_length` is still `None` for the statuses that genuinely loop: Wait,
/// the walks, Run, Fall and SquatWait have no end, and leave by interruption.
/// How long the statuses that end on their own animation last.
///
/// These are not in `FTAttributes`. They are the total frame count of the
/// character's figatree script for that status, which `ssb-rom`'s `anim`
/// module reads out of the animation file and the pack carries per fighter.
///
/// A zero means the animation loops, which for a playable character never
/// happens; treating zero as "no length" therefore degrades to the old
/// interrupt-only behaviour rather than ending the status instantly.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AnimLengths {
    pub dash: f32,
    pub turn: f32,
    pub run_brake: f32,
    pub squat: f32,
    pub squat_rv: f32,
    pub landing: f32,
    pub pass: f32,
}

impl AnimLengths {
    /// Mario's, as `romtool anims` reads them. The default, so a fighter built
    /// without a pack still ends its statuses instead of standing in them.
    pub const MARIO: AnimLengths = AnimLengths {
        dash: 23.0,
        turn: 12.0,
        run_brake: 23.0,
        squat: 8.0,
        squat_rv: 12.0,
        landing: 7.0,
        pass: 25.0,
    };

    /// A length, or `None` when it is zero (a looping animation).
    fn len(frames: f32) -> Option<f32> {
        (frames > 0.0).then_some(frames)
    }
}

impl Default for AnimLengths {
    fn default() -> Self {
        AnimLengths::MARIO
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct StatusTiming {
    /// Frames of animation, if known.
    pub anim_length: Option<f32>,
    /// Playback rate. Landing after a fastfall plays at half speed, which
    /// doubles its real duration in frames.
    pub anim_speed: f32,
}

impl StatusTiming {
    pub fn unknown() -> Self {
        StatusTiming {
            anim_length: None,
            anim_speed: 1.0,
        }
    }

    pub fn frames(len: f32) -> Self {
        StatusTiming {
            anim_length: Some(len),
            anim_speed: 1.0,
        }
    }

    pub fn at_speed(len: f32, speed: f32) -> Self {
        StatusTiming {
            anim_length: Some(len),
            anim_speed: speed,
        }
    }

    /// Timing for a status whose length came from the animation files, where
    /// zero means the animation loops and the status can only be interrupted.
    pub fn animation(frames: f32, speed: f32) -> Self {
        StatusTiming {
            anim_length: AnimLengths::len(frames),
            anim_speed: speed,
        }
    }
}

/// Where a jumpsquat's input came from, which decides how the jump is scaled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum JumpInput {
    #[default]
    None,
    /// The control stick was flicked up.
    Stick,
    /// A jump button was tapped. Only button jumps can be short-hopped.
    Button,
}

/// The per-frame input state the status machine reads.
///
/// This is `FTStruct::input.pl` plus the four tap/hold counters, which
/// `ftMainProcessInput` derives before any status callback runs. Deriving them
/// in one place is what lets every status share one notion of "tapped".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct StickState {
    /// Stick this frame, clamped to ±[`STICK_MAX`].
    pub x: i8,
    pub y: i8,
    /// Stick last frame, for the crossing tests.
    pub prev_x: i8,
    pub prev_y: i8,
    /// Frames since the stick crossed ±[`STICK_DEADZONE`] on each axis, or
    /// [`STICKBUFFER_MAX`] while inside it. See the module docs — this is a
    /// counter, not an edge flag.
    pub tap_x: u8,
    pub tap_y: u8,
    /// Jump buttons pressed this frame, and released this frame.
    pub jump_tapped: bool,
    pub jump_released: bool,
}

impl StickState {
    pub fn new() -> Self {
        StickState {
            tap_x: STICKBUFFER_MAX,
            tap_y: STICKBUFFER_MAX,
            ..Default::default()
        }
    }

    /// Advances one frame of input — `ftMainProcessInput` @ 0x800D9F60.
    ///
    /// The counter resets to 1 on the frame the stick crosses the deadzone
    /// *in a given direction*: crossing from +30 straight to -30 restarts it,
    /// because the test is per-sign and not on magnitude.
    pub fn step(&mut self, x: i8, y: i8, jump_tapped: bool, jump_released: bool) {
        self.prev_x = self.x;
        self.prev_y = self.y;
        self.x = clamp_stick(x);
        self.y = clamp_stick(y);
        self.tap_x = step_tap(self.tap_x, self.x as i32, self.prev_x as i32);
        self.tap_y = step_tap(self.tap_y, self.y as i32, self.prev_y as i32);
        self.jump_tapped = jump_tapped;
        self.jump_released = jump_released;
    }

    /// Stick X relative to a facing direction: positive is forward.
    pub fn forward(&self, facing: Facing) -> i32 {
        self.x as i32 * facing.sign() as i32
    }
}

fn clamp_stick(v: i8) -> i8 {
    (v as i32).clamp(-STICK_MAX, STICK_MAX) as i8
}

/// One axis of the tap counter, from `ftMainProcessInput`.
fn step_tap(current: u8, now: i32, prev: i32) -> u8 {
    let outside = |v: i32| v >= STICK_DEADZONE || v <= -STICK_DEADZONE;
    if !outside(now) {
        return STICKBUFFER_MAX;
    }
    // Same side of the deadzone as last frame: keep counting. Otherwise this
    // is the crossing frame and the count restarts at 1.
    let continued = (now >= STICK_DEADZONE && prev >= STICK_DEADZONE)
        || (now <= -STICK_DEADZONE && prev <= -STICK_DEADZONE);
    if continued {
        current.saturating_add(1).min(STICKBUFFER_MAX)
    } else {
        1
    }
}

/// The status machine's own working state, held alongside the fighter.
///
/// The original keeps this in `FTStruct::status_vars`, a union reused by every
/// status. Here the few fields the ported statuses need are named directly:
/// a union would save 20 bytes and cost the ability to assert on them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StatusState {
    pub status: Status,
    /// Frames of animation elapsed, counting up by `anim_speed`.
    pub anim_frame: f32,
    pub timing: StatusTiming,
    /// Jumpsquat: where the input came from, the best upward deflection seen
    /// so far, and whether the button was released early enough to short-hop.
    pub jump_input: JumpInput,
    pub jump_force: i8,
    pub is_shorthop: bool,
    /// Turn: the facing being turned toward, which a dash out of a turn uses.
    pub turn_toward: Facing,
}

impl Default for StatusState {
    fn default() -> Self {
        StatusState {
            status: Status::Wait,
            anim_frame: 0.0,
            timing: StatusTiming::unknown(),
            jump_input: JumpInput::None,
            jump_force: 0,
            is_shorthop: false,
            turn_toward: Facing::Right,
        }
    }
}

impl StatusState {
    /// Whether the animation has run out, when its length is known.
    ///
    /// Always `false` for a status whose length lives in unextracted animation
    /// data — such a status ends only by being interrupted.
    pub fn animation_ended(&self) -> bool {
        match self.timing.anim_length {
            Some(len) => self.anim_frame >= len,
            None => false,
        }
    }
}

// ---------------------------------------------------------------------------
// Status entry
// ---------------------------------------------------------------------------

/// `ftMainSetStatus` @ 0x800D9314, reduced to what the ported statuses need.
///
/// Entering a status always resets the animation clock; the one caller that
/// does not want that is walk-to-walk, which passes a computed
/// `anim_frame_begin` to keep the legs in phase.
pub fn set_status(f: &mut Fighter, status: Status, anim_frame_begin: f32, timing: StatusTiming) {
    // `mpCommonSetFighterGround` / `...Air`: the situation follows the status,
    // and leaving the ground has to move the velocity across.
    match (f.situation, status.is_grounded()) {
        (Situation::Ground, false) => f.become_airborne(),
        (Situation::Air, true) => {
            f.situation = Situation::Ground;
            f.physics.vel_ground.x = f.physics.vel_air.x;
            f.physics.vel_air = ssb_engine::math::Vec3::ZERO;
            f.physics.is_fastfall = false;
        }
        _ => {}
    }
    f.status.status = status;
    f.status.anim_frame = anim_frame_begin;
    f.status.timing = timing;
}

/// `ftCommonWaitSetStatus` @ 0x8013E1C8.
pub fn set_wait(f: &mut Fighter) {
    set_status(f, Status::Wait, 0.0, StatusTiming::unknown());
}

/// `ftCommonWalkGetWalkStatus` @ 0x8013E340.
pub fn walk_status_for(stick_x: i8) -> Status {
    let m = (stick_x as i32).abs();
    if m >= WALKFAST_STICK_MIN {
        Status::WalkFast
    } else if m >= WALKMIDDLE_STICK_MIN {
        Status::WalkMiddle
    } else {
        Status::WalkSlow
    }
}

/// `ftCommonWalkGetWalkAnimLength` @ 0x8013E2E0.
pub fn walk_anim_length(attr: &PhysicsAttributes, status: Status) -> f32 {
    match status {
        Status::WalkSlow => attr.walkslow_anim_length,
        Status::WalkMiddle => attr.walkmiddle_anim_length,
        Status::WalkFast => attr.walkfast_anim_length,
        // The original reads uninitialised stack here for any other status;
        // it is only ever called with a walk. Returning slow is the harmless
        // reading rather than a reproduction of undefined behaviour.
        _ => attr.walkslow_anim_length,
    }
}

/// `ftCommonWalkSetStatusParam` @ 0x8013E580.
pub fn set_walk(f: &mut Fighter, anim_frame_begin: f32) {
    let status = walk_status_for(f.input.stick_x);
    let len = walk_anim_length(&f.attributes, status);
    set_status(f, status, anim_frame_begin, StatusTiming::frames(len));
}

/// `ftCommonDashSetStatus` @ 0x8013ED00.
///
/// Dash sets the whole ground velocity at once rather than accelerating into
/// it — the initial burst is `dash_speed` on frame one, and `dash_decel` eats
/// it from frame 7 (`FTCOMMON_DASH_DECELERATE_BEGIN`).
pub fn set_dash(f: &mut Fighter) {
    let len = f.anim.dash;
    set_status(f, Status::Dash, 0.0, StatusTiming::animation(len, 1.0));
    f.physics.vel_ground.x = f.attributes.dash_speed;
    f.stick.tap_x = STICKBUFFER_MAX;
}

/// `ftCommonRunSetStatus` @ 0x8013EEE8.
pub fn set_run(f: &mut Fighter) {
    set_status(f, Status::Run, 0.0, StatusTiming::unknown());
    f.physics.vel_ground.x = f.attributes.run_speed;
}

/// `ftCommonRunBrakeSetStatus` @ 0x8013F05C.
pub fn set_run_brake(f: &mut Fighter) {
    let len = f.anim.run_brake;
    set_status(f, Status::RunBrake, 0.0, StatusTiming::animation(len, 1.0));
}

/// `ftCommonTurnSetStatus` @ 0x8013E908.
///
/// The facing does not flip here. `ftCommonTurnProcUpdate` flips it — and the
/// ground velocity with it — on the frame the motion script raises `flag1`,
/// which is partway through the animation. That delay is why a turnaround has
/// a visible pivot rather than snapping.
pub fn set_turn(f: &mut Fighter) {
    let len = f.anim.turn;
    set_status(f, Status::Turn, 0.0, StatusTiming::animation(len, 1.0));
    f.status.turn_toward = f.facing.flipped();
}

/// `ftCommonSquatSetStatusNoPass` @ 0x80143024.
pub fn set_squat(f: &mut Fighter) {
    let len = f.anim.squat;
    set_status(f, Status::Squat, 0.0, StatusTiming::animation(len, 1.0));
}

/// `ftCommonKneeBendSetStatusParam` @ 0x8013F3A0.
pub fn set_kneebend(f: &mut Fighter, input: JumpInput) {
    let len = f.attributes.kneebend_anim_length;
    set_status(f, Status::KneeBend, 0.0, StatusTiming::frames(len));
    f.status.jump_input = input;
    // The jumpsquat records the *highest* upward deflection seen while it
    // runs, not the one on the frame it started.
    f.status.jump_force = f.input.stick_y;
    f.status.is_shorthop = false;
}

/// `ftCommonJumpGetJumpForceButton` @ 0x8013F6A0.
///
/// A button jump trades height for horizontal distance on a circle: the more
/// the stick is held sideways, the lower the jump, floored at a per-length
/// minimum and capped at [`KNEEBEND_BUTTON_HEIGHT_CLAMP`].
pub fn jump_force_button(stick_x: i8, is_shorthop: bool) -> (f32, f32) {
    let vel_x = (stick_x as f32).abs();
    let ratio = vel_x / STICK_MAX as f32;
    let root = ssb_engine::math::sqrt((1.0 - ratio * ratio).max(0.0));

    let (force, min) = if is_shorthop {
        (KNEEBEND_BUTTON_SHORT_FORCE, KNEEBEND_BUTTON_SHORT_MIN)
    } else {
        (KNEEBEND_BUTTON_LONG_FORCE, KNEEBEND_BUTTON_LONG_MIN)
    };
    let mut vel_y = force * root + min;

    let max = STICK_MAX as f32;
    if vel_x * vel_x + vel_y * vel_y > max * max {
        vel_y = ssb_engine::math::sqrt((max * max - vel_x * vel_x).max(0.0));
    }
    if vel_y < min {
        vel_y = min;
    }
    if vel_y > KNEEBEND_BUTTON_HEIGHT_CLAMP {
        vel_y = KNEEBEND_BUTTON_HEIGHT_CLAMP;
    }
    let out_x = if stick_x >= 0 { vel_x } else { -vel_x };
    (out_x, vel_y)
}

/// `ftCommonJumpSetStatus` @ 0x8013F880.
///
/// Takeoff. Note the jump goes *backward* when the stick is pulled behind the
/// fighter past [`KNEEBEND_JUMP_F_OR_B_RANGE`], which is a small negative
/// number and not zero — drifting slightly back still jumps forward.
pub fn set_jump(f: &mut Fighter) {
    let status = if f.stick.forward(f.facing) > KNEEBEND_JUMP_F_OR_B_RANGE {
        Status::JumpF
    } else {
        Status::JumpB
    };
    set_status(f, status, 0.0, StatusTiming::unknown());

    let (vel_x, vel_y) = match f.status.jump_input {
        JumpInput::Button => jump_force_button(f.input.stick_x, f.status.is_shorthop),
        // A stick jump's height is the deflection that triggered it, floored
        // at the threshold so a marginal flick is not a marginal jump.
        _ => {
            let force = (f.status.jump_force as i32).max(KNEEBEND_STICK_MIN) as f32;
            (f.input.stick_x as f32, force)
        }
    };
    let attr = f.attributes;
    f.physics.vel_air.y = vel_y * attr.jump_height_mul + attr.jump_height_base;
    f.physics.vel_air.x = vel_x * attr.jump_vel_x;
    f.stick.tap_y = STICKBUFFER_MAX;
}

/// `ftCommonJumpAerialSetStatus` @ 0x8013FD74.
///
/// A midair jump always uses **full** upward deflection regardless of how far
/// the stick was actually pushed — the original comments that the stick-range
/// jump mechanic "would seem to have been considered for double jumps as
/// well" and then hardcodes `I_CONTROLLER_RANGE_MAX`.
pub fn set_jump_aerial(f: &mut Fighter) {
    let status = if f.stick.forward(f.facing) >= KNEEBEND_JUMP_F_OR_B_RANGE {
        Status::JumpAerialF
    } else {
        Status::JumpAerialB
    };
    set_status(f, status, 0.0, StatusTiming::unknown());

    let attr = f.attributes;
    f.physics.vel_air.y =
        (STICK_MAX as f32 * attr.jump_height_mul + attr.jump_height_base) * attr.jumpaerial_height;
    f.physics.vel_air.x = f.input.stick_x as f32 * attr.jumpaerial_vel_x;
    f.physics.jumps_used += 1;
    f.stick.tap_y = STICKBUFFER_MAX;
}

/// `ftCommonFallSetStatus` @ 0x8013F9E0.
pub fn set_fall(f: &mut Fighter) {
    let status = if f.physics.jumps_used >= f.attributes.jumps_max {
        Status::FallAerial
    } else {
        Status::Fall
    };
    set_status(f, status, 0.0, StatusTiming::unknown());
    physics::clamp_air_vel_x(&mut f.physics, f.attributes.air_speed_max_x);
}

/// `ftCommonLandingSetStatus` @ 0x80142D9C.
///
/// A fastfall that was still at terminal velocity on contact gets the heavy
/// landing, whose animation plays at **half speed** — so it is twice as many
/// frames of lag, which is the cost of fastfalling.
pub fn set_landing(f: &mut Fighter) {
    let heavy = f.physics.is_fastfall && f.physics.vel_air.y <= -f.attributes.tvel_fast;
    let (status, speed) = if heavy {
        (Status::LandingHeavy, 0.5)
    } else {
        (Status::LandingLight, 1.0)
    };
    let len = f.anim.landing;
    set_status(f, status, 0.0, StatusTiming::animation(len, speed));
}

/// `ftCommonPassSetStatusParam` @ 0x80141DA0.
///
/// Dropping through: the fighter goes airborne with its vertical velocity
/// *zeroed*, and the floor it was on becomes the ignored line so the very
/// next collision test does not immediately put it back.
pub fn set_pass(f: &mut Fighter) {
    f.ignore_line = f.floor.map(|s| s.line);
    let len = f.anim.pass;
    set_status(f, Status::Pass, 0.0, StatusTiming::animation(len, 1.0));
    physics::clamp_air_vel_x(&mut f.physics, f.attributes.air_speed_max_x);
    f.physics.vel_air.y = 0.0;
    f.stick.tap_y = STICKBUFFER_MAX;
}

// ---------------------------------------------------------------------------
// Interrupt checks
// ---------------------------------------------------------------------------

/// `ftCommonKneeBendGetInputTypeCommon` @ 0x8013F474.
fn jump_input_type(f: &Fighter, stick_min: i32) -> JumpInput {
    if f.stick.y as i32 >= stick_min && f.stick.tap_y <= KNEEBEND_BUFFER_TICS_MAX {
        JumpInput::Stick
    } else if f.stick.jump_tapped {
        JumpInput::Button
    } else {
        JumpInput::None
    }
}

/// `ftCommonKneeBendCheckInterruptCommon` @ 0x8013F4D0.
pub fn check_kneebend(f: &mut Fighter) -> bool {
    match jump_input_type(f, KNEEBEND_STICK_MIN) {
        JumpInput::None => false,
        input => {
            set_kneebend(f, input);
            true
        }
    }
}

/// `ftCommonKneeBendCheckInterruptRun` @ 0x8013F598. A running fighter jumps
/// off a lower stick threshold — and off `>`, not `>=`.
pub fn check_kneebend_run(f: &mut Fighter) -> bool {
    let stick =
        if f.stick.y as i32 > KNEEBEND_RUN_STICK_MIN && f.stick.tap_y <= KNEEBEND_BUFFER_TICS_MAX {
            JumpInput::Stick
        } else if f.stick.jump_tapped {
            JumpInput::Button
        } else {
            JumpInput::None
        };
    match stick {
        JumpInput::None => false,
        input => {
            set_kneebend(f, input);
            true
        }
    }
}

/// `ftCommonDashCheckInterruptCommon` @ 0x8013ED64.
///
/// A dash input pointed *behind* the fighter turns instead of dashing, which
/// is what makes a dash-dance a sequence of turns rather than of dashes.
pub fn check_dash(f: &mut Fighter) -> bool {
    if (f.stick.x as i32).abs() < DASH_STICK_MIN || f.stick.tap_x >= DASH_BUFFER_TICS_MAX {
        return false;
    }
    if f.stick.forward(f.facing) < 0 {
        set_turn(f);
        return true;
    }
    // `ftParamSetStickLR`: face the way the stick points before dashing.
    f.facing = if f.stick.x >= 0 {
        Facing::Right
    } else {
        Facing::Left
    };
    set_dash(f);
    true
}

/// `ftCommonPassCheckInputSuccess` @ 0x80141E60 — needs the floor to actually
/// be passable, so holding down on solid ground squats instead.
pub fn check_pass(f: &mut Fighter) -> bool {
    let passable = f.floor.map(|s| s.passable()).unwrap_or(false);
    if f.stick.y as i32 <= PASS_STICK_MIN && f.stick.tap_y < PASS_BUFFER_TICS_MAX && passable {
        set_pass(f);
        return true;
    }
    false
}

/// `ftCommonSquatCheckInterruptCommon` @ 0x8014310C.
///
/// Two units below the pass threshold — the pass check runs first, so squat
/// takes over only for a hold that is not a fresh flick down.
pub fn check_squat(f: &mut Fighter) -> bool {
    if f.stick.y as i32 <= SQUAT_STICK_MIN - 2 {
        set_squat(f);
        return true;
    }
    false
}

/// `ftCommonTurnCheckInputSuccess` @ 0x8013ED90.
pub fn check_turn(f: &mut Fighter) -> bool {
    if f.stick.forward(f.facing) <= TURN_STICK_MIN {
        set_turn(f);
        return true;
    }
    false
}

/// `ftCommonWalkCheckInterruptCommon` @ 0x8013E648.
pub fn check_walk(f: &mut Fighter) -> bool {
    if f.stick.forward(f.facing) >= 8 {
        set_walk(f, 0.0);
        return true;
    }
    false
}

/// `ftCommonWaitCheckInterruptCommon` @ 0x8013E2A0.
pub fn check_wait(f: &mut Fighter) -> bool {
    if f.stick.forward(f.facing) < 0 || (f.stick.x as i32).abs() < 8 {
        set_wait(f);
        return true;
    }
    false
}

/// `ftCommonJumpAerialCheckInterruptCommon` @ 0x8014019C.
pub fn check_jump_aerial(f: &mut Fighter) -> bool {
    if f.physics.jumps_used >= f.attributes.jumps_max {
        return false;
    }
    match jump_input_type(f, KNEEBEND_STICK_MIN) {
        JumpInput::None => false,
        _ => {
            set_jump_aerial(f);
            true
        }
    }
}

/// `ftCommonRunBrakeCheckInterruptRun` @ 0x8013F0A0.
pub fn check_run_brake(f: &mut Fighter) -> bool {
    if f.stick.forward(f.facing) < RUN_STICK_MIN {
        set_run_brake(f);
        return true;
    }
    false
}

/// The ground interrupt chain — `ftCommonGroundCheckInterrupt` in
/// `src/ft/fighter.h`, restricted to the ported statuses.
///
/// The order is the original's, with the unported entries (specials, attacks,
/// grab, shield, taunt, pipe) removed rather than reordered around. Returns
/// whether any check took the frame.
pub fn ground_interrupt(f: &mut Fighter) -> bool {
    check_kneebend(f)
        || check_dash(f)
        || check_pass(f)
        || check_squat(f)
        || check_turn(f)
        || check_walk(f)
}

/// A walking fighter's interrupt chain — the `ftCommonWalkCheckInterrupt`
/// macro at the top of `ftcommonwalk.c`.
///
/// Two differences from [`ground_interrupt`], both deliberate in the original:
///
/// * It ends in **Wait**, not Walk. A walk does not re-enter itself; changing
///   walk speed is handled after this returns false, by phase-matching the
///   animation rather than by starting a new status. Ending in Walk here would
///   reset the leg animation to frame zero every frame the stick moved.
/// * There is **no Turn**. Pushing the stick behind you while walking passes
///   `ftCommonWaitCheckInputSuccess` (which tests `stick * lr < 0`) and so
///   goes to Wait; Wait's chain then turns on the following frame. A walking
///   turnaround therefore costs one frame of standing that a standing
///   turnaround does not.
pub fn walk_interrupt(f: &mut Fighter) -> bool {
    check_kneebend(f) || check_dash(f) || check_squat(f) || check_wait(f)
}

// ---------------------------------------------------------------------------
// Per-status update
// ---------------------------------------------------------------------------

/// Runs one frame of the current status: its update, then its interrupts.
///
/// Split from the physics deliberately, matching the original's callback
/// order — `proc_update` can end the status, `proc_interrupt` can replace it,
/// and only then does `proc_physics` run on whatever status is now current.
pub fn update(f: &mut Fighter) {
    f.status.anim_frame += f.status.timing.anim_speed;

    match f.status.status {
        Status::KneeBend => update_kneebend(f),
        Status::Dash => update_dash(f),
        Status::Run => {
            if !(check_kneebend_run(f) || check_run_brake(f)) {
                // Runs do not end on their own; they are held.
            }
        }
        Status::Turn => update_turn(f),
        Status::Squat => {
            if f.status.animation_ended() {
                set_status(f, Status::SquatWait, 0.0, StatusTiming::unknown());
            } else {
                ground_interrupt(f);
            }
        }
        // `ftAnimEndSetWait`: the whole update function is the animation-end
        // test. Landing is here too, and its heavy variant runs the same
        // animation at half speed, so it takes twice as many frames to reach
        // the same length — the real cost of a fastfall.
        Status::RunBrake => {
            if f.status.animation_ended() {
                set_wait(f);
            }
        }
        // `ftAnimEndSetFall` @ ftcommonstatus.h: a drop-through becomes a
        // plain fall once its animation is done.
        Status::Pass => {
            if f.status.animation_ended() {
                set_fall(f);
            } else {
                check_jump_aerial(f);
            }
        }
        s if s.is_actionable_on_ground() => {
            if matches!(s, Status::LandingLight | Status::LandingHeavy)
                && f.status.animation_ended()
            {
                set_wait(f);
            } else if s.is_walk() {
                update_walk(f);
            } else {
                ground_interrupt(f);
            }
        }
        s if !s.is_grounded() => {
            check_jump_aerial(f);
        }
        _ => {}
    }
}

/// `ftCommonKneeBendProcUpdate` @ 0x8013F2A0 and `...ProcInterrupt` @ 0x8013F334.
///
/// Two things accumulate during a jumpsquat: the best upward stick deflection
/// seen (which sets a stick jump's height), and whether the jump button came
/// back up inside the short-hop window. Both are why holding up through the
/// squat jumps higher than flicking.
fn update_kneebend(f: &mut Fighter) {
    if f.status.jump_input == JumpInput::Button
        && f.status.anim_frame <= KNEEBEND_SHORTHOP_FRAMES
        && f.stick.jump_released
    {
        f.status.is_shorthop = true;
    }
    if f.status.jump_force < f.stick.y {
        f.status.jump_force = f.stick.y;
    }
    if f.status.animation_ended() {
        set_jump(f);
    }
}

/// `ftCommonDashProcUpdate` @ 0x8013EA40 and `ftCommonRunCheckInterruptDash`
/// @ 0x8013EF2C.
///
/// The dash-to-run transition is a **one-frame window**: the original tests
/// `dash_to_run <= anim_frame < dash_to_run + anim_speed`, so holding forward
/// through exactly that frame runs and missing it does not. Everything else
/// about the dash's own ending needs its animation length, which is not
/// extracted, so a dash currently persists until interrupted.
fn update_dash(f: &mut Fighter) {
    // `ftCommonDashProcUpdate` runs before the interrupt chain, and a dash
    // that reaches the end of its animation does not just stop: it keeps
    // three quarters of its speed into the Wait, so the fighter coasts.
    if f.status.animation_ended() {
        f.physics.vel_ground.x *= DASH_END_VEL_MUL;
        set_wait(f);
        return;
    }
    let to_run = f.attributes.dash_to_run;
    let speed = f.status.timing.anim_speed;
    if f.status.anim_frame >= to_run
        && f.status.anim_frame < to_run + speed
        && f.stick.forward(f.facing) >= RUN_STICK_MIN
    {
        set_run(f);
        return;
    }
    if check_kneebend_run(f) {
        return;
    }
    check_dash(f);
}

/// `ftCommonTurnProcUpdate` @ 0x8013E690.
///
/// The facing flip is not on entry — it happens partway through, and takes the
/// ground velocity's sign with it. Without the animation length the flip is
/// applied once the status has run as long as the fighter's slow-walk
/// animation would take to reach the same point; see [`StatusTiming`] for why
/// no better number is available.
fn update_turn(f: &mut Fighter) {
    if f.status.anim_frame >= 1.0 && f.facing != f.status.turn_toward {
        f.facing = f.status.turn_toward;
        f.physics.vel_ground.x = -f.physics.vel_ground.x;
    }
    // `ftCommonTurnProcUpdate` flips first and tests the animation second, so
    // a turn always completes its pivot even on the frame it ends.
    if f.status.animation_ended() {
        set_wait(f);
        return;
    }
    ground_interrupt(f);
}

/// `ftCommonWalkProcInterrupt` @ 0x8013E390.
///
/// Switching between walk speeds keeps the animation *phase*, not the frame:
/// the new frame is `(frame / old_length) * new_length`. Resetting to zero
/// would make the legs stutter every time the stick moved a little.
fn update_walk(f: &mut Fighter) {
    if walk_interrupt(f) {
        return;
    }
    let want = walk_status_for(f.input.stick_x);
    if want != f.status.status {
        let old = walk_anim_length(&f.attributes, f.status.status);
        let new = walk_anim_length(&f.attributes, want);
        let phase = if old > 0.0 {
            (f.status.anim_frame / old) * new
        } else {
            0.0
        };
        // The original casts to `s32` and back, so the phase truncates.
        set_walk(f, phase as i32 as f32);
    }
}

/// The physics for the current status — the `proc_physics` callback.
///
/// Each grounded status drives `vel_ground.x` differently, and this is where
/// the difference lives: a walk is *set* from the stick, a dash decelerates,
/// a run holds, and everything else is plain friction.
pub fn apply_status_physics(
    p: &mut PhysicsState,
    attr: &PhysicsAttributes,
    status: Status,
    anim_frame: f32,
    stick_x: i8,
    material_friction: f32,
) {
    match status {
        // `ftCommonWalkProcPhysics` @ 0x8013E548.
        Status::WalkSlow | Status::WalkMiddle | Status::WalkFast => {
            set_ground_vel_abs_stick(p, stick_x, attr.walk_speed_mul, attr.traction);
        }
        // `ftCommonDashProcPhysics` @ 0x8013EC58: no friction for the first
        // seven frames, which is what makes the initial dash burst hold.
        Status::Dash => {
            if anim_frame >= DASH_DECELERATE_BEGIN {
                physics::apply_ground_friction(p, attr.dash_decel);
            }
        }
        // A run holds its speed; nothing decelerates it.
        Status::Run => {}
        // `ftCommonRunBrakeProcPhysics` @ 0x8013F014: 1.25x traction.
        Status::RunBrake => {
            physics::apply_ground_friction(p, attr.traction * 1.25);
        }
        _ => {
            physics::apply_ground_friction(p, attr.traction * material_friction);
        }
    }
}

/// `ftPhysicsSetGroundVelAbsStickRange` @ 0x800D8A40.
///
/// Walk speed is *assigned*, not accumulated: the target is
/// `|stick_x| * walk_speed_mul`, and the fighter snaps up to it instantly but
/// only decelerates down to it. Pushing the stick further speeds you up on
/// the same frame; easing off slows you down over several.
pub fn set_ground_vel_abs_stick(p: &mut PhysicsState, stick_x: i8, vel: f32, friction: f32) {
    let target = (stick_x as f32).abs() * vel;
    if p.vel_ground.x < target {
        p.vel_ground.x = target;
    } else {
        p.vel_ground.x -= friction;
        if p.vel_ground.x < target {
            p.vel_ground.x = target;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fighter::FighterKind;

    fn mario() -> Fighter {
        let mut f = Fighter::new(FighterKind::Mario, 0, 3);
        f.situation = Situation::Ground;
        f.status.status = Status::Wait;
        f
    }

    /// Holds the stick at `(x, y)` for one frame.
    fn hold(f: &mut Fighter, x: i8, y: i8) {
        f.input.stick_x = x;
        f.input.stick_y = y;
        f.stick.step(x, y, false, false);
    }

    #[test]
    fn status_ordinals_match_the_original_enum() {
        assert_eq!(Status::Wait as u16, 10);
        assert_eq!(Status::Dash as u16, 15);
        assert_eq!(Status::KneeBend as u16, 20);
        assert_eq!(Status::Fall as u16, 26);
        assert_eq!(Status::Pass as u16, 33);
    }

    #[test]
    fn the_tap_counter_restarts_on_crossing_the_deadzone_not_the_threshold() {
        // The whole tap-vs-tilt distinction. A flick from neutral straight to
        // full crosses 20 and 56 on the same frame, so the counter reads 1.
        let mut s = StickState::new();
        s.step(80, 0, false, false);
        assert_eq!(s.tap_x, 1);

        // Rolling out slowly crosses 20 first. By the time 56 is reached the
        // counter is past the dash window, so this is a walk and not a dash.
        let mut slow = StickState::new();
        for x in [10, 25, 35, 45, 60] {
            slow.step(x, 0, false, false);
        }
        assert!(slow.tap_x >= DASH_BUFFER_TICS_MAX);
        assert_eq!(slow.tap_x, 4);
    }

    #[test]
    fn a_centred_stick_pins_the_counter_out_of_every_window() {
        let mut s = StickState::new();
        s.step(80, 0, false, false);
        s.step(0, 0, false, false);
        assert_eq!(s.tap_x, STICKBUFFER_MAX);
        assert!(s.tap_x >= DASH_BUFFER_TICS_MAX);
    }

    #[test]
    fn crossing_straight_through_the_deadzone_restarts_the_count() {
        // +30 to -30 is a new crossing, not a continuation: the test is on
        // each side separately, not on magnitude.
        let mut s = StickState::new();
        s.step(30, 0, false, false);
        s.step(30, 0, false, false);
        assert_eq!(s.tap_x, 2);
        s.step(-30, 0, false, false);
        assert_eq!(s.tap_x, 1);
    }

    #[test]
    fn a_flick_dashes_and_a_roll_walks() {
        let mut f = mario();
        hold(&mut f, 80, 0);
        assert!(check_dash(&mut f));
        assert_eq!(f.status.status, Status::Dash);
        assert_eq!(f.physics.vel_ground.x, f.attributes.dash_speed);

        let mut g = mario();
        for x in [10, 25, 35, 45, 60] {
            hold(&mut g, x, 0);
        }
        assert!(!check_dash(&mut g));
        assert!(check_walk(&mut g));
        assert!(g.status.status.is_walk());
    }

    #[test]
    fn walk_speed_follows_how_far_the_stick_is_pushed() {
        assert_eq!(walk_status_for(10), Status::WalkSlow);
        assert_eq!(walk_status_for(26), Status::WalkMiddle);
        assert_eq!(walk_status_for(61), Status::WalkMiddle);
        assert_eq!(walk_status_for(62), Status::WalkFast);
        // Symmetric: the magnitude is what matters, not the direction.
        assert_eq!(walk_status_for(-62), Status::WalkFast);
    }

    #[test]
    fn switching_walk_speed_keeps_the_animation_in_phase() {
        // Mario's medium walk is 60 frames and his fast walk 40. Halfway
        // through the medium one is frame 30; the same phase in the fast one
        // is 20.
        let mut f = mario();
        hold(&mut f, 30, 0);
        set_walk(&mut f, 0.0);
        assert_eq!(f.status.status, Status::WalkMiddle);
        f.status.anim_frame = 30.0;

        // Hold the tilt long enough for the tap window to lapse, or pushing
        // to full stick would be a dash — see the test below.
        for _ in 0..4 {
            hold(&mut f, 30, 0);
        }
        hold(&mut f, 80, 0);
        update_walk(&mut f);
        assert_eq!(f.status.status, Status::WalkFast);
        assert_eq!(f.status.anim_frame, 20.0);
    }

    #[test]
    fn pushing_from_a_tilt_to_full_stick_dashes_if_it_is_quick_enough() {
        // The tap window runs from the deadzone crossing, so a tilt that
        // becomes a full push within three frames is still a dash. This is
        // real behaviour and not a quirk of the port: the counter never saw
        // the stick return to neutral.
        let mut quick = mario();
        hold(&mut quick, 30, 0);
        hold(&mut quick, 80, 0);
        assert_eq!(quick.stick.tap_x, 2);
        assert!(ground_interrupt(&mut quick));
        assert_eq!(quick.status.status, Status::Dash);

        // Dwell on the tilt and the same push walks instead.
        let mut slow = mario();
        for _ in 0..4 {
            hold(&mut slow, 30, 0);
        }
        hold(&mut slow, 80, 0);
        assert!(ground_interrupt(&mut slow));
        assert_eq!(slow.status.status, Status::WalkFast);
    }

    #[test]
    fn a_dash_behind_you_turns_instead() {
        let mut f = mario();
        f.facing = Facing::Right;
        hold(&mut f, -80, 0);
        assert!(check_dash(&mut f));
        assert_eq!(f.status.status, Status::Turn);
        // The flip is not immediate — that is what makes a pivot visible.
        assert_eq!(f.facing, Facing::Right);
        assert_eq!(f.status.turn_toward, Facing::Left);
    }

    #[test]
    fn a_turn_flips_the_facing_and_the_momentum_together() {
        // Stick released before the flip, so nothing interrupts and the flip
        // itself is what is observed.
        let mut f = mario();
        f.facing = Facing::Right;
        f.physics.vel_ground.x = 20.0;
        hold(&mut f, -80, 0);
        set_turn(&mut f);
        hold(&mut f, 0, 0);

        f.status.anim_frame = 1.0;
        update_turn(&mut f);
        assert_eq!(f.facing, Facing::Left);
        assert_eq!(f.physics.vel_ground.x, -20.0);
    }

    #[test]
    fn turning_out_of_a_walk_costs_a_frame_that_turning_from_a_stand_does_not() {
        // The walk chain has no Turn in it, so a walking fighter goes to Wait
        // first and turns on the frame after. From a stand it is immediate.
        let mut walking = mario();
        for _ in 0..4 {
            hold(&mut walking, 60, 0);
        }
        set_walk(&mut walking, 0.0);
        assert!(walking.status.status.is_walk());

        // A gentle reversal, below the dash threshold — a hard flick the other
        // way is a backward dash input and turns immediately either way.
        hold(&mut walking, -30, 0);
        update_walk(&mut walking);
        assert_eq!(walking.status.status, Status::Wait, "walk turns via Wait");

        // The same input from a stand turns on the spot.
        let mut standing = mario();
        for _ in 0..4 {
            hold(&mut standing, -30, 0);
        }
        assert!(ground_interrupt(&mut standing));
        assert_eq!(standing.status.status, Status::Turn);

        // And Wait's own chain turns on the very next frame, so the walk has
        // cost exactly one frame rather than losing the input.
        hold(&mut walking, -30, 0);
        assert!(ground_interrupt(&mut walking));
        assert_eq!(walking.status.status, Status::Turn);
    }

    #[test]
    fn holding_the_stick_through_a_turn_dashes_out_of_it() {
        // Once the facing flips, a stick still held that way is pointing
        // *forward*, and the dash check in the interrupt chain takes it. This
        // is how a dash-dance turns back into a dash.
        let mut f = mario();
        f.facing = Facing::Right;
        hold(&mut f, -80, 0);
        set_turn(&mut f);

        f.status.anim_frame = 1.0;
        update_turn(&mut f);
        assert_eq!(f.facing, Facing::Left);
        assert_eq!(f.status.status, Status::Dash);
        assert_eq!(f.physics.vel_ground.x, f.attributes.dash_speed);
    }

    #[test]
    fn a_jumpsquat_lasts_the_characters_own_frames() {
        // Mario's is 3, Link's 7 — real extracted values, not a shared number.
        let mut f = mario();
        hold(&mut f, 0, 80);
        assert!(check_kneebend(&mut f));
        assert_eq!(f.status.status, Status::KneeBend);
        assert_eq!(f.status.timing.anim_length, Some(3.0));

        for _ in 0..2 {
            update(&mut f);
            assert_eq!(f.status.status, Status::KneeBend);
        }
        update(&mut f);
        assert_eq!(f.status.status, Status::JumpF);
        assert!(f.physics.vel_air.y > 0.0);
        assert_eq!(f.situation, Situation::Air);
    }

    #[test]
    fn releasing_the_jump_button_early_short_hops() {
        let mut f = mario();
        f.stick.step(0, 0, true, false); // button tap
        assert!(check_kneebend(&mut f));
        assert_eq!(f.status.jump_input, JumpInput::Button);

        f.stick.step(0, 0, false, true); // released on frame 1
        update(&mut f);
        assert!(f.status.is_shorthop);

        let mut g = mario();
        g.stick.step(0, 0, true, false);
        check_kneebend(&mut g);
        for _ in 0..3 {
            g.stick.step(0, 0, false, false); // held throughout
            update(&mut g);
        }
        assert!(!g.status.is_shorthop);
        assert!(
            g.physics.vel_air.y > 0.0,
            "a full hop should still leave the ground"
        );
    }

    #[test]
    fn a_short_hop_goes_lower_than_a_full_hop() {
        let (_, short) = jump_force_button(0, true);
        let (_, long) = jump_force_button(0, false);
        assert!(short < long);
        assert_eq!(
            short,
            KNEEBEND_BUTTON_SHORT_FORCE + KNEEBEND_BUTTON_SHORT_MIN
        );
        // The long jump's raw sum exceeds the clamp, so it lands on it.
        assert_eq!(long, KNEEBEND_BUTTON_HEIGHT_CLAMP);
    }

    #[test]
    fn a_sideways_button_jump_trades_height_for_distance() {
        let (_, straight) = jump_force_button(0, true);
        let (x, angled) = jump_force_button(80, true);
        assert!(angled < straight);
        assert_eq!(x, 80.0);
        // Never below the per-length floor, however far sideways.
        assert!(angled >= KNEEBEND_BUTTON_SHORT_MIN);
    }

    #[test]
    fn a_dash_becomes_a_run_only_in_its_one_frame_window() {
        let mut f = mario();
        hold(&mut f, 80, 0);
        set_dash(&mut f);
        assert_eq!(f.attributes.dash_to_run, 14.0);

        // Hold forward through the window.
        for _ in 0..14 {
            hold(&mut f, 80, 0);
            update(&mut f);
        }
        assert_eq!(f.status.status, Status::Run);
        assert_eq!(f.physics.vel_ground.x, f.attributes.run_speed);
    }

    #[test]
    fn a_dash_that_misses_the_window_stays_a_dash() {
        let mut f = mario();
        hold(&mut f, 80, 0);
        set_dash(&mut f);
        // Ease off before the window so the run check fails on that frame.
        for _ in 0..14 {
            hold(&mut f, 20, 0);
            update(&mut f);
        }
        assert_ne!(f.status.status, Status::Run);
    }

    #[test]
    fn a_run_brakes_when_the_stick_comes_back() {
        let mut f = mario();
        set_run(&mut f);
        hold(&mut f, 10, 0);
        update(&mut f);
        assert_eq!(f.status.status, Status::RunBrake);
    }

    #[test]
    fn dropping_through_needs_a_passable_floor() {
        use crate::collision::flags;
        use crate::ground::Standing;
        use ssb_engine::math::Vec2;

        let solid = Standing {
            line: 1,
            flags: 0,
            normal: Vec2::new(0.0, 1.0),
        };
        let platform = Standing {
            line: 2,
            flags: flags::PASS,
            normal: Vec2::new(0.0, 1.0),
        };

        let mut f = mario();
        f.floor = Some(solid);
        hold(&mut f, 0, -80);
        assert!(
            !check_pass(&mut f),
            "solid ground cannot be dropped through"
        );

        let mut g = mario();
        g.floor = Some(platform);
        hold(&mut g, 0, -80);
        assert!(check_pass(&mut g));
        assert_eq!(g.status.status, Status::Pass);
        assert_eq!(g.ignore_line, Some(2));
        // Zeroed rather than carried over, so a drop-through starts from rest.
        assert_eq!(g.physics.vel_air.y, 0.0);
        assert_eq!(g.situation, Situation::Air);
    }

    #[test]
    fn holding_down_on_solid_ground_squats() {
        let mut f = mario();
        f.floor = None;
        hold(&mut f, 0, -80);
        assert!(ground_interrupt(&mut f));
        assert_eq!(f.status.status, Status::Squat);
    }

    #[test]
    fn jumpsquat_outranks_dash_on_the_same_frame() {
        // Both inputs satisfied at once. The chain's order decides, and
        // jumpsquat is ahead of dash in `ftCommonGroundCheckInterrupt`.
        let mut f = mario();
        hold(&mut f, 80, 80);
        assert!(ground_interrupt(&mut f));
        assert_eq!(f.status.status, Status::KneeBend);
    }

    #[test]
    fn a_midair_jump_uses_full_height_however_far_the_stick_went() {
        let mut f = mario();
        f.situation = Situation::Air;
        set_status(&mut f, Status::Fall, 0.0, StatusTiming::unknown());

        // A barely-there flick up still gives a full double jump.
        hold(&mut f, 0, 55);
        assert!(check_jump_aerial(&mut f));
        assert_eq!(f.status.status, Status::JumpAerialF);
        assert_eq!(f.physics.jumps_used, 1);

        let attr = f.attributes;
        let expected = (STICK_MAX as f32 * attr.jump_height_mul + attr.jump_height_base)
            * attr.jumpaerial_height;
        assert_eq!(f.physics.vel_air.y, expected);
    }

    #[test]
    fn a_fighter_out_of_jumps_cannot_double_jump() {
        let mut f = mario();
        f.situation = Situation::Air;
        f.physics.jumps_used = f.attributes.jumps_max;
        hold(&mut f, 0, 80);
        assert!(!check_jump_aerial(&mut f));
    }

    #[test]
    fn falling_with_no_jumps_left_is_a_different_status() {
        let mut f = mario();
        f.situation = Situation::Air;
        set_fall(&mut f);
        assert_eq!(f.status.status, Status::Fall);

        f.physics.jumps_used = f.attributes.jumps_max;
        set_fall(&mut f);
        assert_eq!(f.status.status, Status::FallAerial);
    }

    #[test]
    fn a_fastfall_at_terminal_velocity_lands_heavy_and_at_half_speed() {
        let mut f = mario();
        f.situation = Situation::Air;
        f.physics.is_fastfall = true;
        f.physics.vel_air.y = -f.attributes.tvel_fast;
        set_landing(&mut f);
        assert_eq!(f.status.status, Status::LandingHeavy);
        assert_eq!(f.status.timing.anim_speed, 0.5);

        let mut g = mario();
        g.situation = Situation::Air;
        g.physics.vel_air.y = -1.0;
        set_landing(&mut g);
        assert_eq!(g.status.status, Status::LandingLight);
        assert_eq!(g.status.timing.anim_speed, 1.0);
    }

    #[test]
    fn walking_snaps_up_to_speed_and_eases_down() {
        let attr = PhysicsAttributes::MARIO;
        let mut p = PhysicsState::default();

        // Full stick: the target is reached on the first frame.
        set_ground_vel_abs_stick(&mut p, 80, attr.walk_speed_mul, attr.traction);
        assert_eq!(p.vel_ground.x, 80.0 * attr.walk_speed_mul);

        // Easing to half stick decelerates by traction rather than snapping.
        let before = p.vel_ground.x;
        set_ground_vel_abs_stick(&mut p, 40, attr.walk_speed_mul, attr.traction);
        assert_eq!(p.vel_ground.x, before - attr.traction);
        assert!(p.vel_ground.x > 40.0 * attr.walk_speed_mul);
    }

    #[test]
    fn a_dash_does_not_decelerate_for_its_first_seven_frames() {
        let attr = PhysicsAttributes::MARIO;
        let mut p = PhysicsState::default();
        p.vel_ground.x = attr.dash_speed;

        apply_status_physics(&mut p, &attr, Status::Dash, 6.0, 80, 4.0);
        assert_eq!(p.vel_ground.x, attr.dash_speed, "still in the burst");

        apply_status_physics(&mut p, &attr, Status::Dash, 7.0, 80, 4.0);
        assert_eq!(p.vel_ground.x, attr.dash_speed - attr.dash_decel);
    }

    #[test]
    fn a_run_holds_its_speed_but_a_brake_scrubs_it_faster_than_traction() {
        let attr = PhysicsAttributes::MARIO;
        let mut run = PhysicsState::default();
        run.vel_ground.x = attr.run_speed;
        apply_status_physics(&mut run, &attr, Status::Run, 5.0, 80, 4.0);
        assert_eq!(run.vel_ground.x, attr.run_speed);

        let mut brake = PhysicsState::default();
        brake.vel_ground.x = attr.run_speed;
        apply_status_physics(&mut brake, &attr, Status::RunBrake, 5.0, 0, 4.0);
        assert_eq!(brake.vel_ground.x, attr.run_speed - attr.traction * 1.25);
        assert!(brake.vel_ground.x < run.vel_ground.x);
    }

    #[test]
    fn a_dash_that_is_not_converted_to_a_run_ends_on_its_own() {
        // Mario's dash animation is 23 frames. Without a forward hold there is
        // no run to convert into, so the dash has to end by itself.
        let mut f = mario();
        set_dash(&mut f);
        assert_eq!(f.status.timing.anim_length, Some(23.0));
        let entry_speed = f.physics.vel_ground.x;

        // Neutral stick, so neither the run conversion nor a re-dash fires.
        for _ in 0..23 {
            hold(&mut f, 0, 0);
            update(&mut f);
        }
        assert_eq!(f.status.status, Status::Wait);
        // It coasts out rather than stopping dead, but friction has been
        // eating it since frame 7, so only check the ratio at the transition.
        assert!(f.physics.vel_ground.x > 0.0);
        assert!(f.physics.vel_ground.x < entry_speed);
    }

    #[test]
    fn a_dash_ending_keeps_three_quarters_of_its_speed() {
        let mut f = mario();
        set_dash(&mut f);
        f.status.anim_frame = f.anim.dash;
        f.physics.vel_ground.x = 40.0;
        hold(&mut f, 0, 0);
        update(&mut f);
        assert_eq!(f.status.status, Status::Wait);
        assert_eq!(f.physics.vel_ground.x, 40.0 * DASH_END_VEL_MUL);
    }

    #[test]
    fn a_turn_ends_after_twelve_frames() {
        // Every character in the game turns in 12 frames — the one length the
        // whole roster shares.
        let mut f = mario();
        set_turn(&mut f);
        assert_eq!(f.status.timing.anim_length, Some(12.0));
        for _ in 0..11 {
            hold(&mut f, 0, 0);
            update(&mut f);
            assert_eq!(f.status.status, Status::Turn);
        }
        hold(&mut f, 0, 0);
        update(&mut f);
        assert_eq!(f.status.status, Status::Wait);
    }

    #[test]
    fn a_heavy_landing_lasts_twice_as_long_as_a_light_one() {
        // Same 7-frame animation; the heavy variant plays it at half speed,
        // so a fastfall costs 14 frames of lag instead of 7.
        let light = {
            let mut f = mario();
            f.situation = Situation::Air;
            set_landing(&mut f);
            let mut n = 0;
            while f.status.status != Status::Wait && n < 100 {
                hold(&mut f, 0, 0);
                update(&mut f);
                n += 1;
            }
            n
        };
        let heavy = {
            let mut f = mario();
            f.situation = Situation::Air;
            f.physics.is_fastfall = true;
            f.physics.vel_air.y = -f.attributes.tvel_fast;
            set_landing(&mut f);
            assert_eq!(f.status.status, Status::LandingHeavy);
            let mut n = 0;
            while f.status.status != Status::Wait && n < 100 {
                hold(&mut f, 0, 0);
                update(&mut f);
                n += 1;
            }
            n
        };
        assert_eq!(light, 7);
        assert_eq!(heavy, 2 * light);
    }

    #[test]
    fn a_looping_animation_length_leaves_the_status_interrupt_only() {
        // Master Hand's ground animations loop, and the pack stores that as a
        // zero. A zero must not read as "ends immediately".
        let mut f = mario();
        f.anim.dash = 0.0;
        set_dash(&mut f);
        assert_eq!(f.status.timing.anim_length, None);
        f.status.anim_frame = 10_000.0;
        assert!(!f.status.animation_ended());
    }
}
