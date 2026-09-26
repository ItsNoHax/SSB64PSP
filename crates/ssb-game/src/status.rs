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

use ssb_engine::input::{newly_pressed, newly_released, N64Buttons};
use ssb_engine::math::{Vec2, Vec3};

use crate::collision::{self, Segment};
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
/// `FTCOMMON_SPECIALHI_STICK_RANGE_MIN` — holding this much up while tapping
/// B selects a fighter's up-special from either standard interrupt chain.
pub const SPECIALHI_STICK_MIN: i32 = 53;
/// `FTCOMMON_SPECIALN_TURN_STICK_RANGE_MIN` — a neutral-B reverses Mario
/// when the stick is held this far behind his current facing.
pub const SPECIALN_TURN_STICK_MIN: i32 = -20;
/// The ROM-verified Fireball ground and air figatree duration (files 635 and
/// 636). The shared motion script emits its weapon event at frame 16.
pub const MARIO_FIREBALL_LENGTH_FRAMES: f32 = 46.0;
pub const MARIO_FIREBALL_SPAWN_FRAME: f32 = 16.0;
/// `dMarioMainMotion_SuperJumpPunchAir`: two frames to the initial hit, one
/// frame through its cleanup, then six to `SetFlag1(1)`/`SetFlag2(1)`.
pub const MARIO_SUPERJUMP_LAUNCH_FRAME: f32 = 9.0;
/// `dLuigiMainMotion_SuperJumpPunchAir`/`0x17FC`: the same events, but only
/// four frames from the sweet spot's end to the flags.
pub const LUIGI_SUPERJUMP_LAUNCH_FRAME: f32 = 7.0;

/// The frame the fighter's Super Jump Punch script raises flags 1 and 2.
pub fn superjump_launch_frame(kind: crate::fighter::FighterKind) -> f32 {
    if kind == crate::fighter::FighterKind::Luigi {
        LUIGI_SUPERJUMP_LAUNCH_FRAME
    } else {
        MARIO_SUPERJUMP_LAUNCH_FRAME
    }
}
/// The ROM-verified Super Jump Punch figatree duration. Its motion-event
/// script ends after 27 frames, but `ftMarioSpecialHiProcUpdate` waits for
/// the 40-frame skeleton animation itself to end before entering FallSpecial.
pub const MARIO_SUPERJUMP_LENGTH_FRAMES: f32 = 40.0;
/// `FTMARIO_SUPERJUMP_AIR_DRIFT` and `_LANDING_LAG` from `ftmario.h`.
pub const MARIO_SUPERJUMP_AIR_DRIFT: f32 = 0.6;
pub const MARIO_SUPERJUMP_LANDING_LAG: f32 = 0.28;
/// `FTMARIO_SUPERJUMP_STICK_RANGE_MIN` and the narrower post-launch facing
/// threshold in `ftMarioSpecialHiProcInterrupt`.
pub const MARIO_SUPERJUMP_TURN_STICK_MIN: i32 = 50;
pub const MARIO_SUPERJUMP_FACING_STICK_MIN: i32 = 20;
/// `FTCOMMON_SPECIALLW_STICK_RANGE_MIN` — holding this much down while
/// tapping B selects Mario's Tornado.
pub const SPECIALLW_STICK_MIN: i32 = -53;
pub const MARIO_TORNADO_VEL_X_GROUND: f32 = 0.025;
pub const MARIO_TORNADO_VEL_X_AIR: f32 = 0.03;
pub const MARIO_TORNADO_VEL_X_CLAMP: f32 = 17.0;
pub const MARIO_TORNADO_VEL_Y_CLAMP: f32 = 40.0;
pub const MARIO_TORNADO_VEL_Y_BASE: f32 = 15.0;
pub const MARIO_TORNADO_VEL_Y_TAP: f32 = 22.0;
/// The ground and air scripts both set flags 1/2 and clear flag 3 after the
/// thirteenth one-frame pulse, at frame 43.
pub const MARIO_TORNADO_FINISH_FRAME: f32 = 43.0;
pub const MARIO_TORNADO_GROUND_LENGTH_FRAMES: f32 = 87.0;
pub const MARIO_TORNADO_AIR_LENGTH_FRAMES: f32 = 83.0;

/// `FTCOMMON_ATTACKS3_STICK_RANGE_MIN`/`FTCOMMON_ATTACKHI3_STICK_RANGE_MIN`/
/// `FTCOMMON_ATTACKLW3_STICK_RANGE_MIN` — `ft/ftcommon.h`. Forward deflection
/// (relative to facing) `AttackS3` needs, and the up/down deflection
/// `AttackHi3`/`AttackLw3` need, before their own angle test even runs.
pub const ATTACKS3_STICK_RANGE_MIN: i32 = 20;
pub const ATTACKHI3_STICK_RANGE_MIN: i32 = 20;
pub const ATTACKLW3_STICK_RANGE_MIN: i32 = -20;
/// `tan(17°)`, from `ftCommonAttackS3SetStatus`'s "3ANGLE" branch real test
/// `ftParamGetStickAngleRads(fp) > F_CST_DTOR32(17.0F)` — reframed as a slope
/// comparison for the same reason [`CLIFF_MOTION_ANGLE_TAN_50`] is (no
/// `atan2` in `ssb_engine::math`).
const ATTACKS3_3ANGLE_TAN_17: f32 = 0.305_730_7;

/// `FTCOMMON_ATTACKAIR_DIRECTION_STICK_RANGE_MIN` — `ft/ftcommon.h`: below
/// this on both axes, an aerial attack reads as neutral.
pub const ATTACKAIR_DIRECTION_STICK_RANGE_MIN: i32 = 20;

/// `FTCOMMON_ATTACKS4_*`/`FTCOMMON_ATTACKHI4_*`/`FTCOMMON_ATTACKLW4_*` —
/// `ft/ftcommon.h`. Smashes need a hard flick (`56`/`53` units) inside a
/// short tap window (`tap_x`/`tap_y < 3`/`4`), unlike a tilt's plain
/// magnitude gate — this, checked first in the real interrupt chain, is
/// what makes a fast flick a smash and a slow push a tilt.
pub const ATTACKS4_STICK_RANGE_MIN: i32 = 56;
pub const ATTACKS4_BUFFER_TICS_MAX: u8 = 3;
pub const ATTACKHI4_STICK_RANGE_MIN: i32 = 53;
pub const ATTACKHI4_BUFFER_TICS_MAX: u8 = 4;
pub const ATTACKLW4_STICK_RANGE_MIN: i32 = -53;
pub const ATTACKLW4_BUFFER_TICS_MAX: u8 = 4;
/// `tan(21°)`/`tan(7°)`, from `ftCommonAttackS4SetStatus`'s 5-way angle
/// split (`FTCOMMON_ATTACKS4_5ANGLE_{HI,HIS,LW,LWS}_MIN`) — Mario has all
/// five forward-smash motion files, unlike his forward tilt's three, so his
/// forward smash really does use this branch rather than the 3-way one.
const ATTACKS4_5ANGLE_TAN_21: f32 = 0.383_864_04;
const ATTACKS4_5ANGLE_TAN_7: f32 = 0.122_784_56;
/// Fox has all five forward-tilt motion files; source thresholds are ±30° and ±10°.
const ATTACKS3_5ANGLE_TAN_30: f32 = 0.577_350_26;
const ATTACKS3_5ANGLE_TAN_10: f32 = 0.176_326_98;

/// A fighter's status, with `FTCommonStatus` ordinals preserved exactly —
/// the complete common table (0..=219), transcribed from
/// `ft/ftcommon/ftcommonstatus.h`'s own `// Status N (0x..): Name` comments.
///
/// The ordinals index per-character status tables in the ROM, so renumbering
/// would silently mis-associate every fighter's data. Statuses above 219
/// (`nFTCommonStatusSpecialStart`) are per-character specials/grabs and are
/// not part of this common table; they belong to each fighter's own status
/// enum (`P2`/fighter-bulk-port scope).
///
/// Behaviour (`proc_update`/`proc_interrupt`/`proc_physics`) is wired for a
/// growing subset only — movement (module docs) and the [`Status::DamageHi1`]
/// family (`crate::attack`). Every other variant exists so the ordinal space
/// is correct and future batches can attach behaviour without renumbering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u16)]
pub enum Status {
    DeadDown = 0,
    /// The original's single status for both directions of a "star KO":
    /// `nFTCommonStatusDeadLeftRight`.
    DeadLeftRight = 1,
    DeadUpStar = 2,
    DeadUpFall = 3,
    Sleep = 4,
    Entry = 5,
    EntryNull = 6,
    RebirthDown = 7,
    RebirthStand = 8,
    RebirthWait = 9,
    /// Standing still. `nFTCommonStatusControlStart` — the first status a
    /// player can act out of.
    Wait = 10,
    WalkSlow = 11,
    WalkMiddle = 12,
    WalkFast = 13,
    WalkEnd = 14,
    Dash = 15,
    Run = 16,
    RunBrake = 17,
    Turn = 18,
    TurnRun = 19,
    /// Jumpsquat. The original's name is literal: the knees bend.
    KneeBend = 20,
    GuardKneeBend = 21,
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
    SquatRv = 30,
    LandingLight = 31,
    LandingHeavy = 32,
    /// Dropping through a passable platform.
    Pass = 33,
    GuardPass = 34,
    OttottoWait = 35,
    Ottotto = 36,

    // -- Damage/hitstun family (`ft/ftcommon/ftcommondamage.c`) -------------
    DamageHi1 = 37,
    DamageHi2 = 38,
    DamageHi3 = 39,
    DamageN1 = 40,
    DamageN2 = 41,
    DamageN3 = 42,
    DamageLw1 = 43,
    DamageLw2 = 44,
    DamageLw3 = 45,
    DamageAir1 = 46,
    DamageAir2 = 47,
    DamageAir3 = 48,
    DamageE1 = 49,
    DamageE2 = 50,
    DamageFlyHi = 51,
    DamageFlyN = 52,
    DamageFlyLw = 53,
    DamageFlyTop = 54,
    DamageFlyRoll = 55,
    WallDamage = 56,
    DamageFall = 57,
    FallSpecial = 58,
    LandingFallSpecial = 59,
    Twister = 60,
    TaruCann = 61,

    // -- Barrel/warp pipe, ceiling stick ------------------------------------
    DokanStart = 62,
    DokanWait = 63,
    DokanEnd = 64,
    DokanWalk = 65,
    StopCeil = 66,

    // -- Knockdown / tech / meteor-passive ----------------------------------
    DownBounceD = 67,
    DownBounceU = 68,
    DownWaitD = 69,
    DownWaitU = 70,
    DownStandD = 71,
    DownStandU = 72,
    PassiveStandF = 73,
    PassiveStandB = 74,
    DownForwardD = 75,
    DownForwardU = 76,
    DownBackD = 77,
    DownBackU = 78,
    DownAttackD = 79,
    DownAttackU = 80,
    Passive = 81,
    ReboundWait = 82,
    Rebound = 83,

    // -- Ledge (`mpCommonProcFighterOnCliffEdge`) ---------------------------
    CliffCatch = 84,
    CliffWait = 85,
    CliffQuick = 86,
    CliffClimbQuick1 = 87,
    CliffClimbQuick2 = 88,
    CliffSlow = 89,
    CliffClimbSlow1 = 90,
    CliffClimbSlow2 = 91,
    CliffAttackQuick1 = 92,
    CliffAttackQuick2 = 93,
    CliffAttackSlow1 = 94,
    CliffAttackSlow2 = 95,
    CliffEscapeQuick1 = 96,
    CliffEscapeQuick2 = 97,
    CliffEscapeSlow1 = 98,
    CliffEscapeSlow2 = 99,

    // -- Item pickup/carry/throw ---------------------------------------------
    LightGet = 100,
    HeavyGet = 101,
    LiftWait = 102,
    LiftTurn = 103,
    LightThrowDrop = 104,
    LightThrowDash = 105,
    LightThrowF = 106,
    LightThrowB = 107,
    LightThrowHi = 108,
    LightThrowLw = 109,
    LightThrowF4 = 110,
    LightThrowB4 = 111,
    LightThrowHi4 = 112,
    LightThrowLw4 = 113,
    LightThrowAirF = 114,
    LightThrowAirB = 115,
    LightThrowAirHi = 116,
    LightThrowAirLw = 117,
    LightThrowAirF4 = 118,
    LightThrowAirB4 = 119,
    LightThrowAirHi4 = 120,
    LightThrowAirLw4 = 121,
    HeavyThrowF = 122,
    HeavyThrowB = 123,
    HeavyThrowF4 = 124,
    HeavyThrowB4 = 125,

    // -- Item-weapon swings (sword/bat/harisen/star rod, all shared forms) --
    SwordSwing1 = 126,
    SwordSwing3 = 127,
    SwordSwing4 = 128,
    SwordSwingDash = 129,
    BatSwing1 = 130,
    BatSwing3 = 131,
    BatSwing4 = 132,
    BatSwingDash = 133,
    HarisenSwing1 = 134,
    HarisenSwing3 = 135,
    HarisenSwing4 = 136,
    HarisenSwingDash = 137,
    StarRodSwing1 = 138,
    StarRodSwing3 = 139,
    StarRodSwing4 = 140,
    StarRodSwingDash = 141,
    LGunShoot = 142,
    LGunShootAir = 143,
    FireFlowerShoot = 144,
    FireFlowerShootAir = 145,

    // -- Hammer item ----------------------------------------------------------
    HammerWait = 146,
    HammerWalk = 147,
    HammerTurn = 148,
    HammerKneeBend = 149,
    HammerFall = 150,
    HammerLanding = 151,

    // -- Shield ---------------------------------------------------------------
    GuardOn = 152,
    Guard = 153,
    GuardOff = 154,
    GuardSetOff = 155,
    EscapeF = 156,
    EscapeB = 157,
    ShieldBreakFly = 158,
    ShieldBreakFall = 159,
    ShieldBreakDownD = 160,
    ShieldBreakDownU = 161,
    ShieldBreakStandD = 162,
    ShieldBreakStandU = 163,
    FuraFura = 164,
    FuraSleep = 165,

    // -- Grabs/throws (`ft/ftcommon/ftcommoncatch*.c`, `ftcommonthrow*.c`) --
    Catch = 166,
    CatchPull = 167,
    CatchWait = 168,
    ThrowF = 169,
    ThrowB = 170,
    CapturePulled = 171,
    CaptureWait = 172,
    CaptureKirby = 173,
    CaptureWaitKirby = 174,
    ThrownKirbyStar = 175,
    ThrownCopyStar = 176,
    CaptureYoshi = 177,
    YoshiEgg = 178,
    CaptureCaptain = 179,
    ThrownDonkeyUnk = 180,
    // `nFTCommonStatusThrownStart`..`ThrownEnd` in `ftdef.h`. The status
    // table's comments call 182/183/185/187/188 `ThrownMarioB1`, `ThrownUnk1`,
    // `ThrownMarioB2`, `ThrownUnk2` and `ThrownUnk3`; the enum names are the
    // ones the thrown-status tables (`dMarioMain_thrown_status` and friends)
    // are written in.
    ThrownDonkeyF = 181,
    ThrownMarioBStart = 182,
    ThrownFoxFStart = 183,
    Shouldered = 184,
    ThrownMarioB = 185,
    ThrownCommon = 186,
    ThrownFoxF = 187,
    ThrownFoxB = 188,

    Appeal = 189,

    // -- Base moveset (`ft/ftcommon/ftcommonattack*.c`) ------------------------
    /// Neutral jab — `nFTCommonStatusAttack11`. `F1` criterion 5's grounded
    /// attack; see `crate::attack` for the hitbox/knockback/hitstun it drives.
    Attack11 = 190,
    Attack12 = 191,
    AttackDash = 192,
    AttackS3Hi = 193,
    AttackS3HiS = 194,
    AttackS3 = 195,
    AttackS3LwS = 196,
    AttackS3Lw = 197,
    AttackHi3F = 198,
    AttackHi3 = 199,
    AttackHi3B = 200,
    AttackLw3 = 201,
    AttackS4Hi = 202,
    AttackS4HiS = 203,
    AttackS4 = 204,
    AttackS4LwS = 205,
    AttackS4Lw = 206,
    AttackHi4 = 207,
    AttackLw4 = 208,
    AttackAirN = 209,
    AttackAirF = 210,
    AttackAirB = 211,
    AttackAirHi = 212,
    AttackAirLw = 213,
    LandingAirN = 214,
    LandingAirF = 215,
    LandingAirB = 216,
    LandingAirHi = 217,
    LandingAirLw = 218,
    LandingAirNull = 219,
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
            // No `Attack11` (jab) animation is extracted yet — `crate::attack`'s
            // module docs cover why. Standing in with Wait's slot keeps the
            // pose rather than guessing a clip; `tick_skeleton_animation`
            // already treats "the pack lacks this slot's data" as "keep the
            // current pose", so this is the same fallback other unextracted
            // animations get, not a special case.
            Status::Attack11 => Status::Wait.anim_slot(),
            // Shared grab slots (`ssb_rom::anim::SLOT_CATCH` on). `CatchWait`
            // and `CaptureWait` have no motion of their own (script id -2):
            // they hold the pose the pull ended on, so they map to the pull.
            Status::Catch => 110,
            Status::CatchPull | Status::CatchWait => 111,
            Status::ThrowF => 112,
            Status::ThrowB => 113,
            Status::CapturePulled | Status::CaptureWait => 114,
            Status::ThrownDonkeyF => 115,
            Status::ThrownMarioBStart => 116,
            Status::ThrownFoxFStart => 117,
            Status::Shouldered => 118,
            Status::ThrownMarioB => 119,
            Status::ThrownCommon => 120,
            Status::ThrownFoxF => 121,
            Status::ThrownFoxB => 122,
            // Every other status (the bulk of the just-added common table,
            // `Status` doc comment): no animation is extracted for it yet.
            // Same fallback as `Attack11` — keep the current pose rather than
            // guess a clip.
            _ => Status::Wait.anim_slot(),
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
    ///
    /// The Damage/Fly family is a documented simplification
    /// (`crate::attack`'s module docs): the original can put a
    /// [`Status::DamageHi1`]-table status in the air too, when a shallow hit's
    /// knockback still has an upward component (`ftCommonDamageInitDamageVars`'s
    /// `angle_diff < 90deg` branch). That branch is not ported, so here the
    /// Hi/N/Lw statuses are always grounded and only the dedicated
    /// Air/Fly/Fall statuses are airborne.
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
                | Status::DamageAir1
                | Status::DamageAir2
                | Status::DamageAir3
                | Status::DamageFlyHi
                | Status::DamageFlyN
                | Status::DamageFlyLw
                | Status::DamageFlyTop
                | Status::DamageFlyRoll
                | Status::DamageFall
                | Status::ShieldBreakFly
                | Status::ShieldBreakFall
                | Status::CliffCatch
                | Status::CliffWait
                | Status::CliffClimbQuick1
                | Status::CliffClimbSlow1
                | Status::CliffAttackQuick1
                | Status::CliffAttackSlow1
                | Status::CliffEscapeQuick1
                | Status::CliffEscapeSlow1
                | Status::AttackAirN
                | Status::AttackAirF
                | Status::AttackAirB
                | Status::AttackAirHi
                | Status::AttackAirLw
                | Status::FallSpecial
                // `ftCommonThrownSetStatusQueue`/`...Immediate` put the held
                // fighter in the air (`ga = nMPKineticsAir`).
                | Status::ThrownDonkeyF
                | Status::ThrownMarioBStart
                | Status::ThrownFoxFStart
                | Status::Shouldered
                | Status::ThrownMarioB
                | Status::ThrownCommon
                | Status::ThrownFoxF
                | Status::ThrownFoxB
                // `ftCommonCaptureYoshiProcCapture` and
                // `ftCommonYoshiEggSetStatus` both set the fighter airborne;
                // the egg's own map callback lands it without a new status.
                | Status::CaptureYoshi
                | Status::CaptureCaptain
                | Status::YoshiEgg
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

/// A per-character status, starting at `nFTCommonStatusSpecialStart` (220) —
/// outside the shared common table (`Status`, 0..=219). The original numbers
/// every fighter's own extended statuses independently from 220, so Mario's
/// 220 and, say, Fox's 220 are different moves entirely; a flat shared enum
/// would either collide them or need artificial renumbering that breaks
/// ordinal fidelity with the ROM. Each fighter that needs one gets its own
/// enum like this; [`AnyStatus`] is what actually gets stored on a
/// [`crate::fighter::Fighter`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u16)]
pub enum MarioStatus {
    /// `nFTMarioStatusAttack13` — the jab combo's third-hit finisher.
    Attack13 = 220,
    /// `nFTMarioStatusSpecialN` — Fireball from the ground.
    SpecialN = 223,
    /// `nFTMarioStatusSpecialAirN` — Fireball in the air.
    SpecialAirN = 224,
    /// `nFTMarioStatusSpecialHi` — Super Jump Punch from the ground.
    SpecialHi = 225,
    /// `nFTMarioStatusSpecialAirHi` — Super Jump Punch in the air.
    SpecialAirHi = 226,
    /// `nFTMarioStatusSpecialLw` — grounded Tornado continuation.
    SpecialLw = 227,
    /// `nFTMarioStatusSpecialAirLw` — Tornado's airborne phase and entry.
    SpecialAirLw = 228,
}

/// Fox's extended statuses (`ftfox.h`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u16)]
pub enum FoxStatus {
    Attack100Start = 220,
    Attack100Loop = 221,
    Attack100End = 222,
    SpecialN = 225,
    SpecialAirN = 226,
    SpecialHiStart = 227,
    SpecialAirHiStart = 228,
    SpecialHiHold = 229,
    SpecialAirHiHold = 230,
    SpecialHi = 231,
    SpecialAirHi = 232,
    SpecialHiEnd = 233,
    SpecialAirHiEnd = 234,
    SpecialAirHiBound = 235,
    SpecialLwStart = 236,
    SpecialLwHit = 237,
    SpecialLwEnd = 238,
    SpecialLwLoop = 239,
    SpecialLwTurn = 240,
    SpecialAirLwStart = 241,
    SpecialAirLwHit = 242,
    SpecialAirLwEnd = 243,
    SpecialAirLwLoop = 244,
    SpecialAirLwTurn = 245,
}

/// Donkey Kong's character status table, `ftdonkey.h`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u16)]
pub enum DonkeyStatus {
    SpecialNStart = 222,
    SpecialAirNStart = 223,
    SpecialNLoop = 224,
    SpecialAirNLoop = 225,
    SpecialNEnd = 226,
    SpecialAirNEnd = 227,
    SpecialNFull = 228,
    SpecialAirNFull = 229,
    SpecialHi = 230,
    SpecialAirHi = 231,
    SpecialLwStart = 232,
    SpecialLwLoop = 233,
    SpecialLwEnd = 234,
    // Cargo carry (`ftdonkeythrowf*.c`), `nFTDonkeyStatusThrowFStart` on.
    ThrowFWait = 235,
    ThrowFWalkSlow = 236,
    ThrowFWalkMiddle = 237,
    ThrowFWalkFast = 238,
    ThrowFTurn = 239,
    ThrowFKneeBend = 240,
    ThrowFFall = 241,
    ThrowFLanding = 242,
    ThrowFDamage = 243,
    ThrowFF = 244,
    ThrowAirFF = 245,
}

/// Samus's character status table, `ftsamus.h`. Appear (220, 221) belongs
/// to the unported match-entry sequence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u16)]
pub enum SamusStatus {
    SpecialNStart = 222,
    SpecialNLoop = 223,
    SpecialNEnd = 224,
    SpecialAirNStart = 225,
    SpecialAirNEnd = 226,
    SpecialHi = 227,
    SpecialAirHi = 228,
    SpecialLw = 229,
    SpecialAirLw = 230,
}

/// Link's character status table, `ftlink.h`. Appear (224, 225) belongs to
/// the unported match-entry sequence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u16)]
pub enum LinkStatus {
    Attack13 = 220,
    Attack100Start = 221,
    Attack100Loop = 222,
    Attack100End = 223,
    SpecialHi = 226,
    SpecialHiEnd = 227,
    SpecialAirHi = 228,
    SpecialN = 229,
    SpecialNGet = 230,
    SpecialNEmpty = 231,
    SpecialAirN = 232,
    SpecialAirNReturn = 233,
    SpecialAirNEmpty = 234,
    SpecialLw = 235,
    SpecialAirLw = 236,
}

/// Yoshi's character status table, `ftyoshi.h`. Appear (220, 221) belongs to
/// the unported match-entry sequence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u16)]
pub enum YoshiStatus {
    SpecialHi = 222,
    SpecialAirHi = 223,
    SpecialLwStart = 224,
    SpecialLwLanding = 225,
    SpecialAirLwStart = 226,
    SpecialAirLwLoop = 227,
    SpecialN = 228,
    SpecialNCatch = 229,
    SpecialNRelease = 230,
    SpecialAirN = 231,
    SpecialAirNCatch = 232,
    SpecialAirNRelease = 233,
}

/// Captain Falcon's `ftCaptainStatus` table. Entry-car statuses 224–227 are
/// reserved for the match-entry sequence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u16)]
pub enum CaptainStatus {
    Attack13 = 220,
    Attack100Start = 221,
    Attack100Loop = 222,
    Attack100End = 223,
    SpecialN = 228,
    SpecialAirN = 229,
    SpecialLw = 230,
    SpecialLwAir = 231,
    SpecialLwLanding = 232,
    SpecialAirLw = 233,
    SpecialLwBound = 234,
    SpecialHi = 235,
    SpecialHiCatch = 236,
    SpecialHiThrow = 237,
    SpecialAirHi = 238,
}

/// A fighter's current status: the shared common one, or one of a specific
/// fighter's own extended ones. Nothing here ties a variant to a particular
/// [`crate::fighter::FighterKind`] — same as the original, where a status ID
/// is just a number and its meaning depends on which fighter's status table
/// it is read against.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AnyStatus {
    Common(Status),
    Mario(MarioStatus),
    Fox(FoxStatus),
    Donkey(DonkeyStatus),
    Samus(SamusStatus),
    Link(LinkStatus),
    Yoshi(YoshiStatus),
    Captain(CaptainStatus),
}

impl AnyStatus {
    pub fn is_grounded(self) -> bool {
        match self {
            AnyStatus::Common(s) => s.is_grounded(),
            // `Attack13` and Mario's ground specials are grounded variants.
            AnyStatus::Mario(
                MarioStatus::Attack13
                | MarioStatus::SpecialN
                | MarioStatus::SpecialHi
                | MarioStatus::SpecialLw,
            ) => true,
            AnyStatus::Mario(
                MarioStatus::SpecialAirN | MarioStatus::SpecialAirHi | MarioStatus::SpecialAirLw,
            ) => false,
            AnyStatus::Fox(
                FoxStatus::SpecialAirN
                | FoxStatus::SpecialAirHiStart
                | FoxStatus::SpecialAirHiHold
                | FoxStatus::SpecialAirHi
                | FoxStatus::SpecialAirHiEnd
                | FoxStatus::SpecialAirHiBound
                | FoxStatus::SpecialAirLwStart
                | FoxStatus::SpecialAirLwHit
                | FoxStatus::SpecialAirLwEnd
                | FoxStatus::SpecialAirLwLoop
                | FoxStatus::SpecialAirLwTurn,
            ) => false,
            AnyStatus::Fox(_) => true,
            AnyStatus::Donkey(
                DonkeyStatus::SpecialAirNStart
                | DonkeyStatus::SpecialAirNLoop
                | DonkeyStatus::SpecialAirNEnd
                | DonkeyStatus::SpecialAirNFull
                | DonkeyStatus::SpecialAirHi
                | DonkeyStatus::ThrowFFall
                | DonkeyStatus::ThrowAirFF
                // Forced to damage level 3, so always launched airborne;
                // landing keeps the status (`mpCommonUpdateFighterKinetics`).
                | DonkeyStatus::ThrowFDamage,
            ) => false,
            AnyStatus::Donkey(_) => true,
            AnyStatus::Samus(
                SamusStatus::SpecialAirNStart
                | SamusStatus::SpecialAirNEnd
                | SamusStatus::SpecialAirHi
                | SamusStatus::SpecialAirLw,
            ) => false,
            AnyStatus::Samus(_) => true,
            AnyStatus::Link(
                LinkStatus::SpecialAirHi
                | LinkStatus::SpecialAirN
                | LinkStatus::SpecialAirNReturn
                | LinkStatus::SpecialAirNEmpty
                | LinkStatus::SpecialAirLw,
            ) => false,
            AnyStatus::Link(_) => true,
            // `ftYoshiSpecialLwStartSetStatus` puts the fighter in the air
            // before it sets the status, so the whole Yoshi Bomb is airborne
            // until `SpecialLwLanding`.
            AnyStatus::Yoshi(
                YoshiStatus::SpecialAirHi
                | YoshiStatus::SpecialLwStart
                | YoshiStatus::SpecialAirLwStart
                | YoshiStatus::SpecialAirLwLoop
                | YoshiStatus::SpecialAirN
                | YoshiStatus::SpecialAirNCatch
                | YoshiStatus::SpecialAirNRelease,
            ) => false,
            AnyStatus::Yoshi(_) => true,
            AnyStatus::Captain(CaptainStatus::SpecialAirN | CaptainStatus::SpecialLwAir | CaptainStatus::SpecialAirLw | CaptainStatus::SpecialLwBound | CaptainStatus::SpecialHi | CaptainStatus::SpecialAirHi | CaptainStatus::SpecialHiThrow | CaptainStatus::SpecialHiCatch) => false,
            AnyStatus::Captain(_) => true,
        }
    }

    pub fn is_actionable_on_ground(self) -> bool {
        match self {
            AnyStatus::Common(s) => s.is_actionable_on_ground(),
            AnyStatus::Mario(_) => false,
            AnyStatus::Fox(_) => false,
            AnyStatus::Donkey(_) => false,
            AnyStatus::Samus(_) => false,
            AnyStatus::Link(_) => false,
            AnyStatus::Yoshi(_) => false,
            AnyStatus::Captain(_) => false,
        }
    }

    pub fn is_walk(self) -> bool {
        match self {
            AnyStatus::Common(s) => s.is_walk(),
            AnyStatus::Mario(_) => false,
            AnyStatus::Fox(_) => false,
            AnyStatus::Donkey(s) => matches!(
                s,
                DonkeyStatus::ThrowFWalkSlow
                    | DonkeyStatus::ThrowFWalkMiddle
                    | DonkeyStatus::ThrowFWalkFast
            ),
            AnyStatus::Samus(_) => false,
            AnyStatus::Link(_) => false,
            AnyStatus::Yoshi(_) => false,
            AnyStatus::Captain(_) => false,
        }
    }

    /// Statuses whose motion id is -1/-2: `ftMainSetStatus` loads no new
    /// figatree, so the previous animation keeps playing.
    pub fn keeps_motion(self) -> bool {
        matches!(
            self,
            AnyStatus::Common(Status::CatchWait | Status::CaptureWait)
                | AnyStatus::Yoshi(YoshiStatus::SpecialAirLwLoop)
        )
    }

    /// See `Status::anim_slot`'s docs. Mario's Super Jump Punch pair has
    /// dedicated extracted slots; other extended statuses keep the current
    /// pose until their clip is added to the pack.
    pub fn anim_slot(self) -> usize {
        match self {
            AnyStatus::Common(s) => s.anim_slot(),
            AnyStatus::Mario(MarioStatus::SpecialN) => 20,
            AnyStatus::Mario(MarioStatus::SpecialAirN) => 21,
            AnyStatus::Mario(MarioStatus::SpecialHi) => 22,
            AnyStatus::Mario(MarioStatus::SpecialAirHi) => 23,
            AnyStatus::Mario(MarioStatus::SpecialLw) => 24,
            AnyStatus::Mario(MarioStatus::SpecialAirLw) => 25,
            AnyStatus::Mario(MarioStatus::Attack13) => Status::Wait.anim_slot(),
            AnyStatus::Fox(s) => match s {
                FoxStatus::Attack100Start => 28,
                FoxStatus::Attack100Loop => 29,
                FoxStatus::Attack100End => 30,
                FoxStatus::SpecialN => 47,
                FoxStatus::SpecialAirN => 48,
                FoxStatus::SpecialHiStart => 49,
                FoxStatus::SpecialAirHiStart => 50,
                FoxStatus::SpecialHiHold => 51,
                FoxStatus::SpecialAirHiHold => 52,
                FoxStatus::SpecialHi => 53,
                FoxStatus::SpecialAirHi => 54,
                FoxStatus::SpecialHiEnd => 55,
                FoxStatus::SpecialAirHiEnd => 56,
                FoxStatus::SpecialAirHiBound => 57,
                FoxStatus::SpecialLwStart => 58,
                FoxStatus::SpecialLwTurn => 59,
                FoxStatus::SpecialLwHit => 60,
                FoxStatus::SpecialLwLoop => 61,
                FoxStatus::SpecialAirLwStart => 62,
                FoxStatus::SpecialAirLwTurn => 63,
                FoxStatus::SpecialAirLwHit => 64,
                FoxStatus::SpecialAirLwLoop => 65,
                FoxStatus::SpecialLwEnd => 66,
                FoxStatus::SpecialAirLwEnd => 67,
            },
            AnyStatus::Donkey(s) => match s {
                DonkeyStatus::SpecialNStart => 88,
                DonkeyStatus::SpecialAirNStart => 89,
                DonkeyStatus::SpecialNLoop => 90,
                DonkeyStatus::SpecialAirNLoop => 91,
                DonkeyStatus::SpecialNEnd | DonkeyStatus::SpecialNFull => 92,
                DonkeyStatus::SpecialAirNEnd | DonkeyStatus::SpecialAirNFull => 93,
                DonkeyStatus::SpecialHi => 94,
                DonkeyStatus::SpecialAirHi => 95,
                DonkeyStatus::SpecialLwStart => 96,
                DonkeyStatus::SpecialLwLoop => 97,
                DonkeyStatus::SpecialLwEnd => 98,
                DonkeyStatus::ThrowFWait => 99,
                DonkeyStatus::ThrowFWalkSlow => 100,
                DonkeyStatus::ThrowFWalkMiddle => 101,
                DonkeyStatus::ThrowFWalkFast => 102,
                DonkeyStatus::ThrowFTurn => 103,
                DonkeyStatus::ThrowFKneeBend => 104,
                DonkeyStatus::ThrowFFall => 105,
                DonkeyStatus::ThrowFLanding => 106,
                DonkeyStatus::ThrowFDamage => 107,
                DonkeyStatus::ThrowFF => 108,
                DonkeyStatus::ThrowAirFF => 109,
            },
            AnyStatus::Samus(s) => match s {
                SamusStatus::SpecialNStart => 145,
                SamusStatus::SpecialNLoop => 146,
                SamusStatus::SpecialNEnd => 147,
                SamusStatus::SpecialAirNStart => 148,
                SamusStatus::SpecialAirNEnd => 149,
                SamusStatus::SpecialHi => 150,
                SamusStatus::SpecialAirHi => 151,
                SamusStatus::SpecialLw => 152,
                SamusStatus::SpecialAirLw => 153,
            },
            // `ssb_rom::anim::SLOT_LINK_ATTACK13` onward.
            AnyStatus::Link(s) => match s {
                LinkStatus::Attack13 => 189,
                LinkStatus::Attack100Start => 190,
                LinkStatus::Attack100Loop => 191,
                LinkStatus::Attack100End => 192,
                LinkStatus::SpecialHi => 193,
                LinkStatus::SpecialHiEnd => 194,
                LinkStatus::SpecialAirHi => 195,
                LinkStatus::SpecialN => 196,
                LinkStatus::SpecialNGet => 197,
                LinkStatus::SpecialNEmpty => 198,
                LinkStatus::SpecialAirN => 199,
                LinkStatus::SpecialAirNReturn => 200,
                LinkStatus::SpecialAirNEmpty => 201,
                LinkStatus::SpecialLw => 202,
                LinkStatus::SpecialAirLw => 203,
            },
            // `ssb_rom::anim::SLOT_YOSHI_SPECIAL_HI` onward. The loop keeps
            // the start's figatree (`keeps_motion`); this slot is not read.
            AnyStatus::Yoshi(s) => match s {
                YoshiStatus::SpecialHi => 222,
                YoshiStatus::SpecialAirHi => 223,
                YoshiStatus::SpecialLwStart => 224,
                YoshiStatus::SpecialLwLanding => 225,
                YoshiStatus::SpecialAirLwStart | YoshiStatus::SpecialAirLwLoop => 226,
                YoshiStatus::SpecialN => 227,
                YoshiStatus::SpecialNCatch => 228,
                YoshiStatus::SpecialNRelease => 229,
                YoshiStatus::SpecialAirN => 230,
                YoshiStatus::SpecialAirNCatch => 231,
                YoshiStatus::SpecialAirNRelease => 232,
            },
            AnyStatus::Captain(s) => match s {
                CaptainStatus::Attack13 => 253,
                CaptainStatus::Attack100Start => 254,
                CaptainStatus::Attack100Loop => 255,
                CaptainStatus::Attack100End => 256,
                CaptainStatus::SpecialN => 257,
                CaptainStatus::SpecialAirN => 258,
                CaptainStatus::SpecialLw => 259,
                CaptainStatus::SpecialLwAir => 260,
                CaptainStatus::SpecialLwLanding => 261,
                CaptainStatus::SpecialAirLw => 262,
                CaptainStatus::SpecialLwBound => 263,
                CaptainStatus::SpecialHi => 264,
                CaptainStatus::SpecialHiCatch => 265,
                CaptainStatus::SpecialHiThrow => 266,
                CaptainStatus::SpecialAirHi => 267,
            },
        }
    }

    /// See `Status::anim_speed`'s docs.
    pub fn anim_speed(self) -> f32 {
        match self {
            AnyStatus::Common(s) => s.anim_speed(),
            AnyStatus::Mario(_) => 1.0,
            AnyStatus::Fox(_) => 1.0,
            // `ftDonkeyThrowFKneeBendSetStatus`, `...FallSetStatus` and
            // `...LandingSetStatus` pass an animation speed of 0: the held
            // pose does not play. Their frame counters still run.
            AnyStatus::Donkey(
                DonkeyStatus::ThrowFKneeBend
                | DonkeyStatus::ThrowFFall
                | DonkeyStatus::ThrowFLanding,
            ) => 0.0,
            AnyStatus::Donkey(_) => 1.0,
            // `ftSamusSpecialNStartGetAnimSpeed` is applied by the setter.
            AnyStatus::Samus(_) => 1.0,
            AnyStatus::Link(_) => 1.0,
            // `ftYoshiSpecialAirLwLoopSetStatus` passes an animation speed
            // of 0: the pose and the motion script hold.
            AnyStatus::Yoshi(YoshiStatus::SpecialAirLwLoop) => 0.0,
            AnyStatus::Yoshi(_) => 1.0,
            AnyStatus::Captain(_) => 1.0,
        }
    }
}

impl From<Status> for AnyStatus {
    fn from(status: Status) -> Self {
        AnyStatus::Common(status)
    }
}

/// Lets existing code compare a fighter's current status against a bare
/// common `Status` (`f.status.status == Status::Wait`) without wrapping it
/// on every call site — the overwhelming majority of this module's status
/// comparisons are against common statuses, and only the handful that deal
/// with an extended one need to match on [`AnyStatus`] directly.
impl PartialEq<Status> for AnyStatus {
    fn eq(&self, other: &Status) -> bool {
        matches!(self, AnyStatus::Common(s) if s == other)
    }
}
impl PartialEq<AnyStatus> for Status {
    fn eq(&self, other: &AnyStatus) -> bool {
        other == self
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
    /// `hold_stick_x`: the same count as `tap_x`, which no status consumes.
    pub hold_x: u8,
    /// Jump buttons pressed this frame, and released this frame.
    pub jump_tapped: bool,
    pub jump_released: bool,
}

impl StickState {
    pub fn new() -> Self {
        StickState {
            tap_x: STICKBUFFER_MAX,
            tap_y: STICKBUFFER_MAX,
            hold_x: STICKBUFFER_MAX,
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
        self.hold_x = step_tap(self.hold_x, self.x as i32, self.prev_x as i32);
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
    pub status: AnyStatus,
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
            status: AnyStatus::Common(Status::Wait),
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
// Shield/guard
// ---------------------------------------------------------------------------

/// `FTCOMMON_GUARD_*` — `ft/ftcommon.h`. `SETOFF_MUL` is the `REGION_US`
/// value; the JP/EU build uses 1.75 instead.
pub const GUARD_HEALTH_MAX: f32 = 55.0;
/// The value shield health is reset to after a break —
/// `ftMainProcParams` @ `ftmain.c:3852` (also `ftCommonFuraFura`'s own
/// reset once the break's fly/fall/down chain lands there, which this batch
/// does not port — see [`set_shield_break_fly`]).
pub const GUARD_HEALTH_BREAK_RESPAWN: f32 = 30.0;
pub const GUARD_RELEASE_LAG: i32 = 8;
pub const GUARD_DECAY_INT: i32 = 16;
pub const GUARD_SETOFF_MUL: f32 = 1.62;
pub const GUARD_SETOFF_ADD: f32 = 4.0;
pub const GUARD_VEL_MUL: f32 = 2.0;
/// `ftMainProcParams` @ `ftmain.c:3838`: frames between passive shield-health
/// regen ticks while not shielding and below max.
pub const GUARD_HEAL_INTERVAL: f32 = 10.0;

/// `FTStruct`'s shield fields: `shield_health`/`shield_damage` live directly
/// on `FTStruct` in the original (they persist across Guard's own statuses,
/// unlike `status_vars.common.guard`'s `release_lag`/`decay_wait`/
/// `is_release`, which really is part of the status union) — grouped here
/// instead because nothing else needs them split.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GuardState {
    pub shield_health: f32,
    /// The damage of the hit that last triggered `GuardSetOff` — drives its
    /// pushback and stun length. `shield_damage_total`'s per-frame
    /// multi-hit accumulation (`ftmain.c:2069`) is not ported: this codebase
    /// resolves one hitbox at a time (`crate::attack`'s module docs), so the
    /// single triggering hit's damage stands in for the accumulated total.
    pub shield_damage: f32,
    pub release_lag: i32,
    pub decay_wait: i32,
    /// Set the frame the shield button comes up; the shield keeps ticking
    /// until `release_lag` also reaches zero.
    pub is_release: bool,
    pub setoff_frames: f32,
    /// Frames until the next point of passive regen while not shielding.
    pub heal_wait: f32,
    /// `status_vars.common.guard.is_setoff`: the shield was hit since it
    /// went up. A grab out of it is a shield grab (half throw damage).
    pub is_setoff: bool,
}

impl Default for GuardState {
    fn default() -> Self {
        GuardState {
            shield_health: GUARD_HEALTH_MAX,
            shield_damage: 0.0,
            release_lag: 0,
            decay_wait: 0,
            is_release: false,
            setoff_frames: 0.0,
            heal_wait: GUARD_HEAL_INTERVAL,
            is_setoff: false,
        }
    }
}

/// `ftCommonGuardCheckScheduleRelease` @ `ftcommonguard1.c:25`: the shield
/// button coming up schedules the release; it does not drop the shield
/// itself, which is [`guard_update_shield_vars`]'s job.
pub fn guard_check_schedule_release(f: &mut Fighter) {
    if !f.input.buttons.contains(ssb_engine::input::N64Buttons::Z) {
        f.guard.is_release = true;
    }
}

/// `ftCommonGuardUpdateShieldVars` @ `ftcommonguard1.c:72`, minus the Yoshi
/// hurtbox-collision special case and the visual/effect side (model
/// hide/show, particle effects) — module docs' usual "no rendering fidelity
/// this batch" scope cut. Returns whether the shield has now fully lowered
/// (`release_lag` spent and the button already up), which is what the
/// caller uses in place of the original's `is_shield` flag to know when to
/// leave `GuardOff`.
pub fn guard_update_shield_vars(f: &mut Fighter) -> bool {
    if f.guard.decay_wait != 0 {
        f.guard.decay_wait -= 1;
        if f.guard.decay_wait == 0 {
            f.guard.shield_health -= 1.0;
            if f.guard.shield_health > 0.0 {
                f.guard.decay_wait = GUARD_DECAY_INT;
            }
        }
    }
    if f.guard.release_lag != 0 {
        f.guard.release_lag -= 1;
    }
    f.guard.release_lag == 0 && f.guard.is_release
}

/// `ftCommonGuardOnSetStatus` @ `ftcommonguard1.c:415`, restricted to
/// `slide_tics == 0` (the plain, no-dash-into-shield entry — `check_dash`'s
/// `ftCommonGuardOnCheckInterruptDashRun` case is not ported).
pub fn set_guard_on(f: &mut Fighter) {
    set_status(f, Status::GuardOn, 0.0, StatusTiming::unknown());
    f.guard.release_lag = GUARD_RELEASE_LAG;
    f.guard.decay_wait = GUARD_DECAY_INT;
    f.guard.is_release = false;
    f.guard.is_setoff = false;
}

/// `ftCommonGuardOnCheckInterruptCommon` @ `ftcommonguard1.c:460`. Sits
/// exactly where `ftCommonGroundCheckInterrupt` (`fighter.h`) puts it: right
/// after `Attack1`, before every other ground check.
pub fn check_guard_on(f: &mut Fighter) -> bool {
    if f.input.buttons.contains(ssb_engine::input::N64Buttons::Z) && f.guard.shield_health > 0.0 {
        set_guard_on(f);
        return true;
    }
    false
}

/// `ftCommonGuardSetStatus` @ `ftcommonguard1.c:491`.
pub fn set_guard(f: &mut Fighter) {
    set_status(f, Status::Guard, 0.0, StatusTiming::unknown());
}

/// `ftCommonGuardOffSetStatus` @ `ftcommonguard2.c:78`.
pub fn set_guard_off(f: &mut Fighter) {
    set_status(f, Status::GuardOff, 0.0, StatusTiming::unknown());
}

/// `ftCommonShieldBreakFlyCommonSetStatus`, restricted to the status change
/// and the shield-health respawn value it eventually settles on
/// (`ftmain.c:3852`). The fly → fall → down/stand → `FuraFura` mash-out
/// chain itself is a documented gap: `Status::ShieldBreakFly` has no
/// `update` arm yet, so a broken shield currently just stops there rather
/// than playing out the real vulnerable-flail sequence.
pub fn set_shield_break_fly(f: &mut Fighter) {
    set_status(f, Status::ShieldBreakFly, 0.0, StatusTiming::unknown());
    f.guard.shield_health = GUARD_HEALTH_BREAK_RESPAWN;
}

/// `ftCommonGuardSetOffSetStatus` @ `ftcommonguard2.c:113`: a hit landing on
/// a shielding fighter pushes them back instead of dealing damage/hitstun —
/// [`crate::attack::apply_shield_hit`] is what decides to call this instead
/// of the normal Damage-family entry.
///
/// `shield_lr`/`fp->lr` decide the pushback's direction: away from the
/// fighter's own facing when the hit came from the side already faced (the
/// ordinary case), toward it otherwise. Ground-only: shields are a grounded
/// status, so this only ever writes `vel_ground`, matching the original's
/// `fp->physics.vel_ground.x` write.
pub fn set_guard_set_off(f: &mut Fighter, hit_damage: f32, shield_lr: f32) {
    set_status(f, Status::GuardSetOff, 0.0, StatusTiming::unknown());
    f.guard.shield_damage = hit_damage;
    f.guard.is_setoff = true;
    let setoff_frames = hit_damage * GUARD_SETOFF_MUL + GUARD_SETOFF_ADD;
    f.guard.setoff_frames = setoff_frames;
    let dir = if f.facing.sign() == shield_lr {
        -1.0
    } else {
        1.0
    };
    f.physics.vel_ground.x = dir * setoff_frames * GUARD_VEL_MUL;
}

// ---------------------------------------------------------------------------
// KO / death / respawn
// ---------------------------------------------------------------------------

/// `FTCOMMON_DEAD_WAIT`/`FTCOMMON_DEADUP_WAIT` — `ft/ftcommon.h`.
pub const DEAD_WAIT: f32 = 45.0;
/// `ftCommonDeadUpStarProcUpdate`'s two `FTCOMMON_DEADUP_WAIT` phases (fly
/// off-screen, star flash) collapsed into one wait — see [`set_dead_up_star`].
pub const DEADUP_TOTAL_WAIT: f32 = 180.0 * 2.0 + DEAD_WAIT;
pub const REBIRTH_INVINCIBLE_FRAMES: u16 = 120;
/// `FTCOMMON_REBIRTH_HALO_DESPAWN_WAIT` minus `..._STAND_WAIT`: how long
/// `RebirthDown`'s halo-lowering phase actually runs before the original
/// moves on to `RebirthStand` — `ftCommonRebirthDownProcUpdate`'s
/// `halo_despawn_wait == DESPAWN_WAIT - STAND_WAIT` test.
pub const REBIRTH_DOWN_WAIT: f32 = 390.0 - 75.0;
/// The remaining `FTCOMMON_REBIRTH_HALO_STAND_WAIT`, absorbed into
/// `RebirthWait` here since `RebirthStand`'s own exit is gated on an
/// unextracted animation length (see [`set_rebirth_stand`]).
pub const REBIRTH_WAIT_WAIT: f32 = 75.0;

/// A stage's blast-zone extents — `MPGroundData.map_bound_*`
/// (`ssb_rom::pack::StageDesc::bounds`). Passed in by the caller (mirroring
/// how [`Fighter::tick`] takes its floors) so Layer A stays free of the pack
/// format.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BlastZone {
    pub top: f32,
    pub bottom: f32,
    pub left: f32,
    pub right: f32,
}

/// `ftCommonDeadCheckInterruptCommon` @ `ftcommondead.c:533`, restricted to
/// the ordinary (non-1P-team, non-`is_limit_map_bounds`, non-`is_ghost`)
/// case — the other branches are 1P-mode/camera-bound/already-dead special
/// cases this codebase has no equivalent state for yet. Order matches the
/// original: bottom, then right, then left, then top. Always picks
/// `DeadUpStar` for a top-out — the original's 1-in-6 `DeadUpFall` branch
/// needs an RNG source this crate does not have yet (same gap as
/// `crate::attack`'s `DamageFlyRoll`).
///
/// Stocks are decremented immediately on death (`ftCommonDeadUpdateScore`,
/// called from every `DeadXxxSetStatus`), not at respawn time — matching
/// [`try_rebirth`]'s later read of `f.stocks`.
pub fn check_dead(f: &mut Fighter, bounds: BlastZone) -> bool {
    if f.pos.y < bounds.bottom {
        set_dead_down(f);
    } else if f.pos.x > bounds.right || f.pos.x < bounds.left {
        set_dead_left_right(f);
    } else if f.pos.y > bounds.top {
        set_dead_up_star(f);
    } else {
        return false;
    }
    f.stocks -= 1;
    true
}

fn enter_dead(f: &mut Fighter, status: Status, wait: f32) {
    // `ftCommonDeadResetCommonVars` → `ftCommonThrownDecideDeadResult`.
    crate::grab::release_on_dead(f);
    set_status(f, status, 0.0, StatusTiming::frames(wait));
    f.physics = crate::physics::PhysicsState::default();
    f.situation = Situation::Air;
}

/// `ftCommonDeadDownSetStatus` @ `ftcommondead.c:194`, minus the
/// score/rumble/screen-flash/sound/camera-bound-clamp side effects (module
/// docs' usual rendering/scene-manager scope cut).
pub fn set_dead_down(f: &mut Fighter) {
    enter_dead(f, Status::DeadDown, DEAD_WAIT);
}

/// `ftCommonDeadRightSetStatus`/`...LeftSetStatus` @ `ftcommondead.c:235,277`
/// — both enter the same `DeadLeftRight` ordinal in the original.
pub fn set_dead_left_right(f: &mut Fighter) {
    enter_dead(f, Status::DeadLeftRight, DEAD_WAIT);
}

/// `ftCommonDeadUpStarSetStatus` @ `ftcommondead.c:740`, collapsed to a
/// single wait of [`DEADUP_TOTAL_WAIT`] frames instead of the original's two
/// `motion_vars.flags.flag1`-driven phases (off-screen flight with a
/// position/colour tween, then a star-flash pause) — both phases are purely
/// visual on top of the same real frame counts, which this sum preserves.
pub fn set_dead_up_star(f: &mut Fighter) {
    enter_dead(f, Status::DeadUpStar, DEADUP_TOTAL_WAIT);
}

/// `ftCommonDeadCheckRebirth` @ `ftcommondead.c:95`, restricted to the
/// stock-match case (no 1P-mode enemy-team respawn). Call once a Dead-family
/// status's wait has elapsed ([`StatusState::animation_ended`]) — `update`
/// cannot do this itself because, unlike every other status transition in
/// this module, it needs the stage's respawn point from outside.
///
/// Returns `false` and leaves the status alone if `f` is not in a
/// finished Dead status, so callers can poll it unconditionally each tick.
pub fn try_rebirth(f: &mut Fighter, respawn_pos: Vec3) -> bool {
    let in_dead_family = matches!(
        f.status.status,
        AnyStatus::Common(Status::DeadDown | Status::DeadLeftRight | Status::DeadUpStar)
    );
    if !in_dead_family || !f.status.animation_ended() {
        return false;
    }
    if f.stocks < 0 {
        // `ftCommonSleepSetStatus`: out of stocks, out of the match. No
        // callback exists for `Sleep` yet (module docs) — parking here is
        // the same "ordinal-only, no behaviour" gap every other unwired
        // status has.
        set_status(f, Status::Sleep, 0.0, StatusTiming::unknown());
    } else {
        set_rebirth_down(f, respawn_pos);
    }
    true
}

/// `ftCommonRebirthDownSetStatus` @ `ftcommonrebirth.c:20`, minus the halo
/// map-object lookup/spacing (multiple simultaneous respawns are offset
/// sideways in the original so their halos do not overlap — cosmetic) and
/// the halo-drop animation itself: the fighter is placed at `respawn_pos`
/// immediately rather than tweened down from above. `damage = 0` on respawn
/// is real (`dFTManagerDefaultFighterDesc`'s reset), not a simplification.
pub fn set_rebirth_down(f: &mut Fighter, respawn_pos: Vec3) {
    f.pos = respawn_pos;
    f.damage = 0;
    f.physics = crate::physics::PhysicsState::default();
    f.situation = Situation::Ground;
    set_status(
        f,
        Status::RebirthDown,
        0.0,
        StatusTiming::frames(REBIRTH_DOWN_WAIT),
    );
}

/// `ftCommonRebirthStandSetStatus` @ `ftcommonrebirth.c:157`. No animation
/// length is extracted for the landing pose, so — like [`set_guard_on`] —
/// this resolves into `RebirthWait` on its very next update tick rather than
/// waiting for the original's `ftAnimEndCheckSetStatus`.
pub fn set_rebirth_stand(f: &mut Fighter) {
    set_status(f, Status::RebirthStand, 0.0, StatusTiming::unknown());
}

/// `ftCommonRebirthWaitSetStatus` @ `ftcommonrebirth.c:198`.
pub fn set_rebirth_wait(f: &mut Fighter) {
    set_status(
        f,
        Status::RebirthWait,
        0.0,
        StatusTiming::frames(REBIRTH_WAIT_WAIT),
    );
}

/// `ftCommonRebirthWaitProcUpdate`'s exit @ `ftcommonrebirth.c:173`: back
/// under normal control, with a window of hit-invincibility.
fn end_rebirth(f: &mut Fighter) {
    f.invincible_frames = REBIRTH_INVINCIBLE_FRAMES;
    set_fall(f);
}

// ---------------------------------------------------------------------------
// Fastfall / FallSpecial
// ---------------------------------------------------------------------------

/// `FTCOMMON_FASTFALL_STICK_RANGE_MIN`/`_BUFFER_TICS_MAX` — `ft/ftcommon.h`.
pub const FASTFALL_STICK_RANGE_MIN: i32 = -53;
pub const FASTFALL_BUFFER_TICS_MAX: u8 = 4;

/// `ftPhysicsCheckSetFastFall` @ `ftphysics.c:231`, minus the
/// collision-animation side effect (rendering, out of scope). A one-shot
/// per-airtime latch — `is_fastfall` only clears again on landing
/// (`Fighter::land`) — so this only ever fires once per fall. Was
/// previously never called from anywhere in this codebase: `Fighter::tick_air`
/// read `physics.is_fastfall` but nothing ever set it from a real input,
/// so a downward flick while falling did nothing. Now called from
/// `tick_air` every airborne tick, same as the original calls it from every
/// airborne status's own `proc_physics`.
pub fn check_set_fast_fall(f: &mut Fighter) {
    if !f.physics.is_fastfall
        && f.physics.vel_air.y < 0.0
        && (f.stick.y as i32) <= FASTFALL_STICK_RANGE_MIN
        && f.stick.tap_y < FASTFALL_BUFFER_TICS_MAX
    {
        f.physics.is_fastfall = true;
        f.stick.tap_y = STICKBUFFER_MAX;
    }
}

/// `ftCommonFallSpecialSetStatus`/status_vars, minus `is_allow_pass`'s
/// drop-through-platform nuance during the fall (module docs on
/// [`set_fall_special`]).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct FallSpecialState {
    /// Air-drift clamp for this particular use, already multiplied by
    /// `attr.air_speed_max_x` — a recovery move's own drift multiplier,
    /// not the fighter's normal one.
    pub drift: f32,
    pub is_goto_landing: bool,
    pub landing_lag: f32,
    pub is_allow_interrupt: bool,
    /// `false` selects the terminal-velocity-clamped fall
    /// `ftCommonFallSpecialProcPhysics` uses by default; `true` selects
    /// plain default gravity instead (used by moves that keep accelerating
    /// rather than settling immediately).
    pub is_fall_accelerate: bool,
}

/// Motion-script state used by `ftMarioSpecialHiProcInterrupt`. The script's
/// flag 2 is a one-shot: it selects Mario's launch-facing direction once, at
/// the same frame flag 1 enables TransN motion.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct MarioSpecialHiState {
    pub launch_started: bool,
}

/// `ftMarioSpecialNInitStatusVars`: the accessory callback consumes motion
/// script flag 0 once, so a ground/air map transition must preserve this flag
/// rather than creating another Fireball at the same animation frame.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct MarioSpecialNState {
    pub spawned: bool,
}

/// Persistent charge and per-status flags from `FTDonkeyStatusVars::specialn`
/// and `FTPassiveVars::donkey`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DonkeySpecialNState {
    pub charge_level: u8,
    pub attack_charge: u8,
    pub release: bool,
    pub charging: bool,
    pub cancel: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DonkeySpecialLwState {
    pub loop_requested: bool,
}

/// Fox's Blaster event flags: each status entry fires once, then B can
/// restart the move after the source script's flag-1 gate.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct FoxSpecialNState {
    pub spawned: bool,
}

/// `ftFoxSpecialHiStatusVars`, retained across ground/air switches.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct FoxSpecialHiState {
    pub gravity_delay: u8,
    pub launch_delay: u8,
    pub travel_frames: u8,
    pub decelerate_wait: u8,
    pub angle: f32,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct FoxSpecialLwState {
    pub release_lag: u8,
    pub released: bool,
    pub gravity_delay: u8,
    pub turn_frames: u8,
    pub turned: bool,
}

/// `ftMarioSpecialLwStatusVars` plus the persistent tornado-rise expenditure.
/// Flag 3 gates B-tap rises; flag 1 starts reducing the horizontal clamp at
/// the finisher; flag 2 permanently spends the aerial rise until the fighter
/// is reset, just as `passive_vars.mario.is_expend_tornado` does.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MarioSpecialLwState {
    pub friction: f32,
    pub rise_enabled: bool,
    pub rise_exhausted: bool,
}

impl Default for MarioSpecialLwState {
    fn default() -> Self {
        MarioSpecialLwState {
            friction: 0.0,
            rise_enabled: false,
            rise_exhausted: false,
        }
    }
}

/// `FTCOMMON_FALLSPECIAL_SKIPLANDING_VEL_Y_MAX` — `ft/ftcommon.h`.
pub const FALLSPECIAL_SKIPLANDING_VEL_Y_MAX: f32 = -20.0;

/// `ftCommonFallSpecialSetStatus` @ `ftcommonfallspecial.c:72`, minus the
/// collision-animation/rumble side effects and `is_allow_pass` (module docs).
/// This is the shared "helpless fall" a recovery move lands in after its own
/// launch phase — every fighter's up-special that has one calls into this
/// same status rather than defining its own.
pub fn set_fall_special(
    f: &mut Fighter,
    drift_mul: f32,
    is_fall_accelerate: bool,
    is_goto_landing: bool,
    landing_lag: f32,
    is_allow_interrupt: bool,
) {
    let drift = f.attributes.air_speed_max_x * drift_mul;
    set_status(f, Status::FallSpecial, 0.0, StatusTiming::unknown());
    physics::clamp_air_vel_x(&mut f.physics, drift);
    f.physics.jumps_used = f.attributes.jumps_max;
    f.fall_special = FallSpecialState {
        drift,
        is_goto_landing,
        landing_lag,
        is_allow_interrupt,
        is_fall_accelerate,
    };
    f.is_special_interrupt = true;
}

/// `ftMarioSpecialNSetStatus` @ 0x80156014.
pub fn set_mario_special_n(f: &mut Fighter) {
    set_any_status(
        f,
        AnyStatus::Mario(MarioStatus::SpecialN),
        0.0,
        StatusTiming::frames(MARIO_FIREBALL_LENGTH_FRAMES),
    );
    f.mario_special_n = MarioSpecialNState::default();
}

/// `ftMarioSpecialAirNSetStatus` @ 0x80156054.
pub fn set_mario_special_air_n(f: &mut Fighter) {
    set_any_status(
        f,
        AnyStatus::Mario(MarioStatus::SpecialAirN),
        0.0,
        StatusTiming::frames(MARIO_FIREBALL_LENGTH_FRAMES),
    );
    f.mario_special_n = MarioSpecialNState::default();
}

/// `ftFoxSpecialNSetStatus` / `ftFoxSpecialAirNSetStatus`; durations from
/// US ROM figatree files 779 and 780.
pub fn set_fox_special_n(f: &mut Fighter) {
    let (status, length) = if f.situation == Situation::Ground {
        (FoxStatus::SpecialN, 55.0)
    } else {
        (FoxStatus::SpecialAirN, 45.0)
    };
    set_any_status(f, AnyStatus::Fox(status), 0.0, StatusTiming::frames(length));
    f.fox_special_n = FoxSpecialNState::default();
}

/// `ftFoxSpecialLwStartSetStatus` and aerial counterpart.
pub fn set_fox_special_lw_start(f: &mut Fighter) {
    let ground = f.situation == Situation::Ground;
    let status = if ground {
        FoxStatus::SpecialLwStart
    } else {
        FoxStatus::SpecialAirLwStart
    };
    set_any_status(f, AnyStatus::Fox(status), 0.0, StatusTiming::frames(4.0));
    f.fox_special_lw = FoxSpecialLwState {
        release_lag: 18,
        gravity_delay: 4,
        ..Default::default()
    };
    if !ground {
        f.physics.vel_air.y = 0.0;
        f.physics.vel_air.x /= 2.0;
    }
}

fn set_fox_special_lw_phase(f: &mut Fighter, phase: u8) {
    let ground = f.situation == Situation::Ground;
    let (status, timing) = match (phase, ground) {
        (0, true) => (FoxStatus::SpecialLwLoop, StatusTiming::unknown()),
        (0, false) => (FoxStatus::SpecialAirLwLoop, StatusTiming::unknown()),
        (1, true) => (FoxStatus::SpecialLwTurn, StatusTiming::unknown()),
        (1, false) => (FoxStatus::SpecialAirLwTurn, StatusTiming::unknown()),
        (2, true) => (FoxStatus::SpecialLwEnd, StatusTiming::frames(18.0)),
        (_, false) => (FoxStatus::SpecialAirLwEnd, StatusTiming::frames(18.0)),
        _ => unreachable!(),
    };
    set_any_status(f, AnyStatus::Fox(status), 0.0, timing);
    if phase == 1 {
        f.fox_special_lw.turn_frames = 4;
        f.fox_special_lw.turned = false;
    }
}

/// Reflector's hit response, entered by the match weapon collision pass.
pub fn set_fox_special_lw_hit(f: &mut Fighter) {
    let status = if f.situation == Situation::Ground {
        FoxStatus::SpecialLwHit
    } else {
        FoxStatus::SpecialAirLwHit
    };
    set_any_status(f, AnyStatus::Fox(status), 0.0, StatusTiming::frames(6.0));
}

/// `ftFoxSpecialAirLwCommonProcPhysics`.
pub fn apply_fox_special_lw_air_physics(f: &mut Fighter) {
    if f.fox_special_lw.gravity_delay > 0 {
        f.fox_special_lw.gravity_delay -= 1;
    } else {
        physics::apply_gravity_clamp_tvel(&mut f.physics, 0.8, f.attributes.tvel_base);
    }
    if !physics::check_clamp_air_vel_x_dec(&mut f.physics, f.attributes.air_speed_max_x) {
        physics::apply_air_friction(&mut f.physics, &f.attributes);
    }
}

/// `ftFoxSpecialHiStartSetStatus` and aerial counterpart.
pub fn set_fox_special_hi_start(f: &mut Fighter) {
    let ground = f.situation == Situation::Ground;
    let status = if ground {
        FoxStatus::SpecialHiStart
    } else {
        FoxStatus::SpecialAirHiStart
    };
    set_any_status(f, AnyStatus::Fox(status), 0.0, StatusTiming::frames(8.0));
    f.fox_special_hi = FoxSpecialHiState {
        gravity_delay: 15,
        ..Default::default()
    };
    if ground {
        f.physics.vel_ground.x /= 2.0;
    } else {
        f.physics.vel_air.y = 0.0;
        f.physics.vel_air.x /= 2.0;
    }
}

fn set_fox_special_hi_hold(f: &mut Fighter) {
    let status = if f.situation == Situation::Ground {
        FoxStatus::SpecialHiHold
    } else {
        FoxStatus::SpecialAirHiHold
    };
    set_any_status(f, AnyStatus::Fox(status), 0.0, StatusTiming::unknown());
    f.fox_special_hi.launch_delay = 35;
}

fn set_fox_special_hi_travel(f: &mut Fighter) {
    let x = f.stick.x as f32;
    let y = f.stick.y as f32;
    let angled = x.abs() + y.abs() >= 45.0;
    let ground_normal = f
        .floor
        .map(|floor| floor.normal)
        .unwrap_or(Vec2::new(0.0, 1.0));
    // The ground branch is selected when the stick points into the floor.
    let on_ground = f.situation == Situation::Ground
        && angled
        && ground_normal.x * x + ground_normal.y * y <= 0.0;
    if angled && x.abs() >= 11.0 {
        f.facing = if x < 0.0 { Facing::Left } else { Facing::Right };
    }
    let angle = if on_ground {
        ssb_engine::math::atan2(-ground_normal.x * f.facing.sign(), ground_normal.y)
    } else if angled {
        ssb_engine::math::atan2(y, x * f.facing.sign())
    } else {
        core::f32::consts::FRAC_PI_2
    };
    let status = if on_ground {
        FoxStatus::SpecialHi
    } else {
        FoxStatus::SpecialAirHi
    };
    set_any_status(f, AnyStatus::Fox(status), 0.0, StatusTiming::unknown());
    f.fox_special_hi.angle = angle;
    f.fox_special_hi.travel_frames = 30;
    f.fox_special_hi.decelerate_wait = 0;
    if on_ground {
        f.physics.vel_ground.x = 115.0 * f.facing.sign();
    } else {
        let (sin, cos) = ssb_engine::math::sin_cos(angle);
        f.physics.vel_air.x = cos * 115.0 * f.facing.sign();
        f.physics.vel_air.y = sin * 115.0;
        f.physics.jumps_used = f.attributes.jumps_max;
    }
}

fn set_fox_special_hi_end(f: &mut Fighter) {
    let status = if f.situation == Situation::Ground {
        FoxStatus::SpecialHiEnd
    } else {
        FoxStatus::SpecialAirHiEnd
    };
    let length = if f.situation == Situation::Ground {
        29.0
    } else {
        20.0
    };
    set_any_status(f, AnyStatus::Fox(status), 0.0, StatusTiming::frames(length));
}

/// Floor-only portion of `ftFoxSpecialAirHiProcMap`. The source redirects a
/// shallow collision along the surface and keeps the travel status airborne.
/// Steeper contact enters bound if its approach exceeds 110 degrees from the
/// surface normal; other contacts finish on the ground.
pub fn fox_fire_fox_floor_contact(f: &mut Fighter, normal: Vec2, floor_y: f32) -> bool {
    if f.status.status != AnyStatus::Fox(FoxStatus::SpecialAirHi) {
        return false;
    }
    let velocity = f.physics.vel_air;
    let speed = ssb_engine::math::sqrt(velocity.x * velocity.x + velocity.y * velocity.y);
    if speed <= 0.0 {
        f.land(floor_y);
        set_fox_special_hi_end(f);
        return true;
    }
    let dot = normal.x * velocity.x + normal.y * velocity.y;
    let similarity = dot / (1.0 + speed);
    if (-0.342_020_15..=0.0).contains(&similarity) {
        let orientation = if normal.x * velocity.y - normal.y * velocity.x < 0.0 {
            -1.0
        } else {
            1.0
        };
        f.physics.vel_air.x = -normal.y * speed * orientation;
        f.physics.vel_air.y = normal.x * speed * orientation;
        f.facing = if f.physics.vel_air.x >= 0.0 {
            Facing::Right
        } else {
            Facing::Left
        };
        f.fox_special_hi.angle =
            ssb_engine::math::atan2(f.physics.vel_air.y, f.physics.vel_air.x * f.facing.sign());
        f.pos.y = floor_y;
        return true;
    }
    if dot / speed < -0.342_020_15 {
        f.land(floor_y);
        set_any_status(
            f,
            AnyStatus::Fox(FoxStatus::SpecialAirHiBound),
            0.0,
            StatusTiming::frames(20.0),
        );
    } else {
        f.land(floor_y);
        set_fox_special_hi_end(f);
    }
    true
}

/// Fire Fox's own physics callbacks; the launched phase has no normal gravity.
pub fn apply_fox_special_hi_air_physics(f: &mut Fighter) {
    match f.status.status {
        AnyStatus::Fox(FoxStatus::SpecialAirHiStart | FoxStatus::SpecialAirHiHold) => {
            if f.fox_special_hi.gravity_delay > 0 {
                f.fox_special_hi.gravity_delay -= 1;
            } else {
                physics::apply_gravity_clamp_tvel(&mut f.physics, 0.5, f.attributes.tvel_base);
            }
            if !physics::check_clamp_air_vel_x_dec(&mut f.physics, f.attributes.air_speed_max_x) {
                physics::apply_air_friction(&mut f.physics, &f.attributes);
            }
        }
        AnyStatus::Fox(FoxStatus::SpecialAirHi) => {
            f.fox_special_hi.decelerate_wait = f.fox_special_hi.decelerate_wait.saturating_add(1);
            if f.fox_special_hi.decelerate_wait >= 2 {
                let (sin, cos) = ssb_engine::math::sin_cos(f.fox_special_hi.angle);
                f.physics.vel_air.x -= 3.035_714_4 * cos * f.facing.sign();
                f.physics.vel_air.y -= 3.035_714_4 * sin;
            }
        }
        AnyStatus::Fox(FoxStatus::SpecialAirHiEnd | FoxStatus::SpecialAirHiBound) => {
            physics::apply_gravity_default(&mut f.physics, &f.attributes);
            physics::apply_air_friction(&mut f.physics, &f.attributes);
        }
        _ => {}
    }
}

pub fn apply_fox_special_hi_ground_physics(f: &mut Fighter) {
    if f.status.status == AnyStatus::Fox(FoxStatus::SpecialHi) {
        f.fox_special_hi.decelerate_wait = f.fox_special_hi.decelerate_wait.saturating_add(1);
        if f.fox_special_hi.decelerate_wait >= 2 {
            physics::apply_ground_friction(&mut f.physics, 3.035_714_4);
        }
    } else {
        physics::apply_ground_friction(&mut f.physics, 1.5);
    }
}

/// Ground-to-air map callback for Fire Fox's corresponding phases.
pub fn switch_fox_special_hi_air(f: &mut Fighter) {
    let (status, timing) = match f.status.status {
        AnyStatus::Fox(FoxStatus::SpecialHiStart) => {
            (FoxStatus::SpecialAirHiStart, StatusTiming::frames(8.0))
        }
        AnyStatus::Fox(FoxStatus::SpecialHiHold) => {
            (FoxStatus::SpecialAirHiHold, StatusTiming::unknown())
        }
        AnyStatus::Fox(FoxStatus::SpecialHi) => (FoxStatus::SpecialAirHi, StatusTiming::unknown()),
        AnyStatus::Fox(FoxStatus::SpecialHiEnd) => {
            (FoxStatus::SpecialAirHiEnd, StatusTiming::frames(20.0))
        }
        _ => return,
    };
    set_any_status(f, AnyStatus::Fox(status), f.status.anim_frame, timing);
    if matches!(
        status,
        FoxStatus::SpecialAirHiStart | FoxStatus::SpecialAirHiHold
    ) {
        physics::clamp_air_vel_x(&mut f.physics, f.attributes.air_speed_max_x);
    }
}

pub fn switch_fox_special_lw_air(f: &mut Fighter) {
    let (status, timing) = match f.status.status {
        AnyStatus::Fox(FoxStatus::SpecialLwStart) => {
            (FoxStatus::SpecialAirLwStart, StatusTiming::frames(4.0))
        }
        AnyStatus::Fox(FoxStatus::SpecialLwLoop) => {
            (FoxStatus::SpecialAirLwLoop, StatusTiming::unknown())
        }
        AnyStatus::Fox(FoxStatus::SpecialLwHit) => {
            (FoxStatus::SpecialAirLwHit, StatusTiming::frames(6.0))
        }
        AnyStatus::Fox(FoxStatus::SpecialLwEnd) => {
            (FoxStatus::SpecialAirLwEnd, StatusTiming::frames(18.0))
        }
        AnyStatus::Fox(FoxStatus::SpecialLwTurn) => {
            (FoxStatus::SpecialAirLwTurn, StatusTiming::unknown())
        }
        _ => return,
    };
    set_any_status(f, AnyStatus::Fox(status), f.status.anim_frame, timing);
}

/// `ftMarioSpecialAirNSwitchStatusGround` @ 0x80155F4C.
pub fn switch_mario_fireball_ground(f: &mut Fighter) {
    set_any_status(
        f,
        AnyStatus::Mario(MarioStatus::SpecialN),
        f.status.anim_frame,
        StatusTiming::frames(MARIO_FIREBALL_LENGTH_FRAMES),
    );
}

/// `ftMarioSpecialNSwitchStatusAir` @ 0x80155FA0.
pub fn switch_mario_fireball_air(f: &mut Fighter) {
    f.become_airborne();
    set_any_status(
        f,
        AnyStatus::Mario(MarioStatus::SpecialAirN),
        f.status.anim_frame,
        StatusTiming::frames(MARIO_FIREBALL_LENGTH_FRAMES),
    );
    physics::clamp_air_vel_x(&mut f.physics, f.attributes.air_speed_max_x);
}

/// `ftDonkeySpecialNStartSetStatus` and its aerial counterpart.
pub fn set_donkey_special_n(f: &mut Fighter) {
    let status = if f.is_grounded() {
        DonkeyStatus::SpecialNStart
    } else {
        DonkeyStatus::SpecialAirNStart
    };
    set_any_status(f, AnyStatus::Donkey(status), 0.0, StatusTiming::frames(8.0));
    let charge = f.donkey_special_n.charge_level;
    f.donkey_special_n.release = charge == 10;
    f.donkey_special_n.charging = false;
    f.donkey_special_n.cancel = false;
}

fn donkey_charge_loop(f: &mut Fighter) {
    let status = if f.is_grounded() {
        DonkeyStatus::SpecialNLoop
    } else {
        DonkeyStatus::SpecialAirNLoop
    };
    // The figatree loops every 12 frames; its zero decoded length denotes
    // that loop rather than a one-frame animation.
    set_any_status(
        f,
        AnyStatus::Donkey(status),
        0.0,
        StatusTiming::frames(12.0),
    );
}

fn donkey_charge_release(f: &mut Fighter) {
    let full = f.donkey_special_n.charge_level == 10;
    let status = match (f.is_grounded(), full) {
        (true, false) => DonkeyStatus::SpecialNEnd,
        (false, false) => DonkeyStatus::SpecialAirNEnd,
        (true, true) => DonkeyStatus::SpecialNFull,
        (false, true) => DonkeyStatus::SpecialAirNFull,
    };
    let charge = f.donkey_special_n.charge_level;
    f.donkey_special_n.attack_charge = charge;
    f.donkey_special_n.charge_level = 0;
    set_any_status(
        f,
        AnyStatus::Donkey(status),
        0.0,
        StatusTiming::frames(80.0),
    );
    if f.is_grounded() {
        f.physics.vel_ground.x = f32::from(charge) * 8.0 * f.facing.sign();
    }
}

/// `ftDonkeySpecialHiSetStatus` / `ftDonkeySpecialAirHiSetStatus`.
pub fn set_donkey_special_hi(f: &mut Fighter) {
    let ground = f.is_grounded();
    let status = if ground {
        DonkeyStatus::SpecialHi
    } else {
        DonkeyStatus::SpecialAirHi
    };
    set_any_status(
        f,
        AnyStatus::Donkey(status),
        0.0,
        StatusTiming::frames(100.0),
    );
    f.physics.jumps_used = f.attributes.jumps_max;
    if ground {
        f.physics.vel_air.y = 0.0;
    } else {
        f.physics.vel_air.y = 20.3;
    }
    physics::clamp_air_vel_x(&mut f.physics, 38.0);
}

pub fn set_donkey_special_lw(f: &mut Fighter) {
    set_any_status(
        f,
        AnyStatus::Donkey(DonkeyStatus::SpecialLwStart),
        0.0,
        StatusTiming::frames(3.0),
    );
    f.donkey_special_lw.loop_requested = false;
}

pub fn apply_donkey_special_hi_ground_physics(f: &mut Fighter) {
    physics::apply_clamp_ground_vel_stick_range(
        &mut f.physics,
        f.input.stick_x,
        0,
        0.025,
        f.facing.sign(),
        26.0,
    );
}

pub fn apply_donkey_special_hi_air_physics(f: &mut Fighter) {
    // The aerial tail's first eight-frame loop sets motion flag1 at frame 57.
    let gravity_mul = if f.status.anim_frame >= 57.0 {
        1.0
    } else {
        0.07
    };
    physics::apply_gravity_clamp_tvel(
        &mut f.physics,
        f.attributes.gravity * gravity_mul,
        f.attributes.tvel_base,
    );
    physics::clamp_air_vel_x_stick_range(&mut f.physics, f.input.stick_x, 0, 0.05, 38.0);
}

pub fn switch_donkey_special_air(f: &mut Fighter) {
    let status = match f.status.status {
        AnyStatus::Donkey(DonkeyStatus::SpecialNStart) => DonkeyStatus::SpecialAirNStart,
        AnyStatus::Donkey(DonkeyStatus::SpecialNLoop) => DonkeyStatus::SpecialAirNLoop,
        AnyStatus::Donkey(DonkeyStatus::SpecialNEnd) => DonkeyStatus::SpecialAirNEnd,
        AnyStatus::Donkey(DonkeyStatus::SpecialNFull) => DonkeyStatus::SpecialAirNFull,
        AnyStatus::Donkey(DonkeyStatus::SpecialHi) => DonkeyStatus::SpecialAirHi,
        _ => {
            f.become_airborne();
            set_fall(f);
            return;
        }
    };
    f.become_airborne();
    set_any_status(
        f,
        AnyStatus::Donkey(status),
        f.status.anim_frame,
        f.status.timing,
    );
    physics::clamp_air_vel_x(
        &mut f.physics,
        if status == DonkeyStatus::SpecialAirHi {
            38.0
        } else {
            f.attributes.air_speed_max_x
        },
    );
}

pub fn switch_donkey_special_ground(f: &mut Fighter) {
    let status = match f.status.status {
        AnyStatus::Donkey(DonkeyStatus::SpecialAirNStart) => DonkeyStatus::SpecialNStart,
        AnyStatus::Donkey(DonkeyStatus::SpecialAirNLoop) => DonkeyStatus::SpecialNLoop,
        AnyStatus::Donkey(DonkeyStatus::SpecialAirNEnd) => DonkeyStatus::SpecialNEnd,
        AnyStatus::Donkey(DonkeyStatus::SpecialAirNFull) => DonkeyStatus::SpecialNFull,
        AnyStatus::Donkey(DonkeyStatus::SpecialAirHi) => DonkeyStatus::SpecialHi,
        _ => return,
    };
    set_any_status(
        f,
        AnyStatus::Donkey(status),
        f.status.anim_frame,
        f.status.timing,
    );
    if status == DonkeyStatus::SpecialHi {
        physics::clamp_ground_vel(&mut f.physics, 26.0);
    }
}

/// `ftCommonSpecialNCheckInterruptCommon` @ 0x80151098 for Mario and Fox.
/// Neutral B is strictly between the up/down-special thresholds.
pub fn check_special_n(f: &mut Fighter) -> bool {
    if !matches!(
        f.kind,
        crate::fighter::FighterKind::Mario
            | crate::fighter::FighterKind::Fox
            | crate::fighter::FighterKind::Donkey
            | crate::fighter::FighterKind::Samus
            | crate::fighter::FighterKind::Luigi
            | crate::fighter::FighterKind::Link
            | crate::fighter::FighterKind::Yoshi
            | crate::fighter::FighterKind::Captain
    ) || !newly_pressed(f.prev_input.buttons, f.input.buttons).contains(N64Buttons::B)
        || !(SPECIALLW_STICK_MIN < f.stick.y as i32 && (f.stick.y as i32) < SPECIALHI_STICK_MIN)
    {
        return false;
    }
    if f.stick.forward(f.facing) < SPECIALN_TURN_STICK_MIN {
        f.facing = f.facing.flipped();
    }
    match f.kind {
        // Luigi runs Mario's special statuses (`dFTLuigiSpecialStatusDescs`).
        crate::fighter::FighterKind::Mario | crate::fighter::FighterKind::Luigi => {
            if f.situation == Situation::Ground {
                set_mario_special_n(f);
            } else {
                set_mario_special_air_n(f);
            }
        }
        crate::fighter::FighterKind::Fox => set_fox_special_n(f),
        crate::fighter::FighterKind::Donkey => set_donkey_special_n(f),
        crate::fighter::FighterKind::Samus => crate::samus::set_special_n(f),
        crate::fighter::FighterKind::Link => {
            if f.situation == Situation::Ground {
                crate::link::set_special_n(f);
            } else {
                crate::link::set_special_air_n(f);
            }
        }
        crate::fighter::FighterKind::Yoshi => {
            if f.situation == Situation::Ground {
                crate::yoshi::set_special_n(f);
            } else {
                crate::yoshi::set_special_air_n(f);
            }
        }
        crate::fighter::FighterKind::Captain => {
            if f.is_grounded() {
                crate::captain::set_special_n(f);
            } else {
                crate::captain::set_special_air_n(f);
            }
        }
        _ => unreachable!(),
    }
    true
}

/// `ftMarioSpecialHiSetStatus` @ 0x80156428.
pub fn set_mario_special_hi(f: &mut Fighter) {
    set_any_status(
        f,
        AnyStatus::Mario(MarioStatus::SpecialHi),
        0.0,
        StatusTiming::frames(MARIO_SUPERJUMP_LENGTH_FRAMES),
    );
    f.mario_special_hi = MarioSpecialHiState::default();
}

/// `ftMarioSpecialAirHiSetStatus` @ 0x80156478.
pub fn set_mario_special_air_hi(f: &mut Fighter) {
    set_any_status(
        f,
        AnyStatus::Mario(MarioStatus::SpecialAirHi),
        0.0,
        StatusTiming::frames(MARIO_SUPERJUMP_LENGTH_FRAMES),
    );
    f.mario_special_hi = MarioSpecialHiState::default();
    f.physics.vel_air.y = 0.0;
    f.physics.vel_air.x /= 1.5;
}

/// `ftCommonSpecialHiCheckInterruptCommon` @ 0x80151160, limited to Mario:
/// B edge plus an upward stick. Every other fighter remains unported here.
pub fn check_special_hi(f: &mut Fighter) -> bool {
    if !matches!(
        f.kind,
        crate::fighter::FighterKind::Mario
            | crate::fighter::FighterKind::Fox
            | crate::fighter::FighterKind::Donkey
            | crate::fighter::FighterKind::Samus
            | crate::fighter::FighterKind::Luigi
            | crate::fighter::FighterKind::Link
            | crate::fighter::FighterKind::Yoshi
            | crate::fighter::FighterKind::Captain
    ) || !newly_pressed(f.prev_input.buttons, f.input.buttons).contains(N64Buttons::B)
        || (f.stick.y as i32) < SPECIALHI_STICK_MIN
    {
        return false;
    }
    if f.kind == crate::fighter::FighterKind::Captain {
        crate::captain::set_special_hi(f);
    } else if f.kind == crate::fighter::FighterKind::Yoshi {
        if f.is_grounded() {
            crate::yoshi::set_special_hi(f);
        } else {
            crate::yoshi::set_special_air_hi(f);
        }
    } else if f.kind == crate::fighter::FighterKind::Link {
        if f.is_grounded() {
            crate::link::set_special_hi(f);
        } else {
            crate::link::set_special_air_hi(f);
        }
    } else if f.kind == crate::fighter::FighterKind::Samus {
        if f.is_grounded() {
            crate::samus::set_special_hi(f);
        } else {
            crate::samus::set_special_air_hi(f);
        }
    } else if f.kind == crate::fighter::FighterKind::Donkey {
        set_donkey_special_hi(f);
    } else if f.kind == crate::fighter::FighterKind::Fox {
        set_fox_special_hi_start(f);
    } else if f.situation == Situation::Ground {
        set_mario_special_hi(f);
    } else {
        set_mario_special_air_hi(f);
    }
    true
}

/// `ftMarioSpecialHiProcPhysics` @ 0x80156240, aerial branch. Before the
/// source motion script raises flag1 the move uses capped gravity and normal
/// air friction; from frame 9 onward it consumes TransN and damps all axes.
pub fn apply_mario_special_air_hi_physics(f: &mut Fighter) {
    if f.status.anim_frame >= superjump_launch_frame(f.kind) {
        physics::apply_air_vel_transn_all(&mut f.physics, f.root_motion, f.facing.sign());
        f.physics.vel_air *= 0.95;
    } else {
        physics::apply_gravity_clamp_tvel(&mut f.physics, 0.5, f.attributes.tvel_base);
        if !physics::check_clamp_air_vel_x_dec(&mut f.physics, f.attributes.air_speed_max_x) {
            physics::apply_air_friction(&mut f.physics, &f.attributes);
        }
    }
}

/// `ftMarioSpecialHiProcInterrupt` @ `ftmariospecialhi.c:64`. Before launch,
/// a strong horizontal stick may increase the TransN root's Z rotation; at
/// launch, flag 2 selects the facing direction exactly once. The runtime
/// supplies the sampled root motion, while this portable callback preserves
/// the original's player-input semantics.
pub fn apply_mario_special_hi_interrupt(f: &mut Fighter) {
    let stick_x = f.stick.x as i32;
    if f.status.anim_frame < superjump_launch_frame(f.kind) {
        if stick_x.abs() >= MARIO_SUPERJUMP_TURN_STICK_MIN {
            let clamped = stick_x.signum() * MARIO_SUPERJUMP_TURN_STICK_MIN;
            let desired_rotation =
                -((stick_x - clamped) as f32 * MARIO_SUPERJUMP_AIR_DRIFT * core::f32::consts::PI
                    / 180.0);
            if f.root_motion.rotate_z.abs() < desired_rotation.abs() {
                f.root_motion.rotate_z = desired_rotation;
            }
        }
    } else if !f.mario_special_hi.launch_started {
        f.mario_special_hi.launch_started = true;
        if stick_x.abs() >= MARIO_SUPERJUMP_FACING_STICK_MIN {
            f.facing = if stick_x < 0 {
                Facing::Left
            } else {
                Facing::Right
            };
        }
    }
}

fn init_mario_tornado_status(f: &mut Fighter) {
    f.mario_special_lw.friction = 0.0;
    // Both source motion scripts run SetFlag3(1) at frame zero. A ground ↔
    // air map transition deliberately does not call this helper, preserving
    // `ftMarioSpecialAirLwSetDisableRise`'s one-way gate.
    f.mario_special_lw.rise_enabled = true;
}

/// `ftMarioSpecialLwSetStatus` @ 0x8015688C. Counterintuitively, a grounded
/// down-B begins in the aerial Tornado status with its sourced -7 Y velocity;
/// the ground status is selected only when the move subsequently lands.
pub fn set_mario_special_lw(f: &mut Fighter) {
    f.become_airborne();
    set_any_status(
        f,
        AnyStatus::Mario(MarioStatus::SpecialAirLw),
        0.0,
        StatusTiming::frames(MARIO_TORNADO_AIR_LENGTH_FRAMES),
    );
    f.physics.vel_air.y = -7.0;
    physics::clamp_air_vel_x(&mut f.physics, MARIO_TORNADO_VEL_X_CLAMP);
    init_mario_tornado_status(f);
}

/// `ftMarioSpecialAirLwSetStatus` @ 0x80156910.
pub fn set_mario_special_air_lw(f: &mut Fighter) {
    set_any_status(
        f,
        AnyStatus::Mario(MarioStatus::SpecialAirLw),
        0.0,
        StatusTiming::frames(MARIO_TORNADO_AIR_LENGTH_FRAMES),
    );
    f.physics.vel_air.y = MARIO_TORNADO_VEL_Y_BASE
        - if f.mario_special_lw.rise_exhausted {
            0.0
        } else {
            MARIO_TORNADO_VEL_Y_TAP
        };
    physics::clamp_air_vel_x(&mut f.physics, MARIO_TORNADO_VEL_X_CLAMP);
    init_mario_tornado_status(f);
}

/// `ftMarioSpecialAirLwSwitchStatusGround` @ 0x801567B0. Keep the current
/// animation frame while replacing the aerial figatree with the ground one.
pub fn switch_mario_tornado_ground(f: &mut Fighter) {
    f.mario_special_lw.rise_enabled = false;
    set_any_status(
        f,
        AnyStatus::Mario(MarioStatus::SpecialLw),
        f.status.anim_frame,
        StatusTiming::frames(MARIO_TORNADO_GROUND_LENGTH_FRAMES),
    );
    physics::clamp_ground_vel(&mut f.physics, MARIO_TORNADO_VEL_X_CLAMP);
}

/// `ftMarioSpecialLwSwitchStatusAir` @ 0x80156808.
pub fn switch_mario_tornado_air(f: &mut Fighter) {
    f.mario_special_lw.rise_enabled = false;
    f.become_airborne();
    set_any_status(
        f,
        AnyStatus::Mario(MarioStatus::SpecialAirLw),
        f.status.anim_frame,
        StatusTiming::frames(MARIO_TORNADO_AIR_LENGTH_FRAMES),
    );
    f.physics.vel_air.y = f.physics.vel_air.y.min(MARIO_TORNADO_VEL_Y_CLAMP);
    physics::clamp_air_vel_x(&mut f.physics, MARIO_TORNADO_VEL_X_CLAMP);
}

/// `ftCommonSpecialLwCheckInterruptCommon`, restricted to Mario.
pub fn check_special_lw(f: &mut Fighter) -> bool {
    if !matches!(
        f.kind,
        crate::fighter::FighterKind::Mario
            | crate::fighter::FighterKind::Fox
            | crate::fighter::FighterKind::Donkey
            | crate::fighter::FighterKind::Samus
            | crate::fighter::FighterKind::Luigi
            | crate::fighter::FighterKind::Link
            | crate::fighter::FighterKind::Yoshi
            | crate::fighter::FighterKind::Captain
    ) || !newly_pressed(f.prev_input.buttons, f.input.buttons).contains(N64Buttons::B)
        || (f.stick.y as i32) > SPECIALLW_STICK_MIN
    {
        return false;
    }
    if f.kind == crate::fighter::FighterKind::Captain {
        if f.is_grounded() {
            crate::captain::set_special_lw(f);
        } else {
            crate::captain::set_special_air_lw(f);
        }
    } else if f.kind == crate::fighter::FighterKind::Yoshi {
        if f.is_grounded() {
            crate::yoshi::set_special_lw_start(f);
        } else {
            crate::yoshi::set_special_air_lw_start(f);
        }
    } else if f.kind == crate::fighter::FighterKind::Link {
        if f.is_grounded() {
            crate::link::set_special_lw(f);
        } else {
            crate::link::set_special_air_lw(f);
        }
    } else if f.kind == crate::fighter::FighterKind::Samus {
        if f.is_grounded() {
            crate::samus::set_special_lw(f);
        } else {
            crate::samus::set_special_air_lw(f);
        }
    } else if f.kind == crate::fighter::FighterKind::Donkey {
        if f.is_grounded() {
            set_donkey_special_lw(f);
        } else {
            return false;
        }
    } else if f.kind == crate::fighter::FighterKind::Fox {
        set_fox_special_lw_start(f);
    } else if f.situation == Situation::Ground {
        set_mario_special_lw(f);
    } else {
        set_mario_special_air_lw(f);
    }
    true
}

fn mario_tornado_clamp(f: &mut Fighter) -> f32 {
    let mut clamp = MARIO_TORNADO_VEL_X_CLAMP;
    if f.status.anim_frame >= MARIO_TORNADO_FINISH_FRAME {
        f.mario_special_lw.friction -= 2.0;
        clamp += f.mario_special_lw.friction;
    }
    clamp.max(0.0)
}

/// `ftMarioSpecialLwProcPhysics` @ 0x80156630. Returns true when a sourced
/// B-tap transitions from the ground phase to the aerial phase this tick.
pub fn apply_mario_special_lw_ground_physics(f: &mut Fighter) -> bool {
    let clamp = mario_tornado_clamp(f);
    physics::apply_clamp_ground_vel_stick_range(
        &mut f.physics,
        f.input.stick_x,
        0,
        MARIO_TORNADO_VEL_X_GROUND,
        f.facing.sign(),
        clamp,
    );
    if f.mario_special_lw.rise_enabled
        && newly_pressed(f.prev_input.buttons, f.input.buttons).contains(N64Buttons::B)
    {
        f.physics.vel_air.y += MARIO_TORNADO_VEL_Y_TAP;
        switch_mario_tornado_air(f);
        true
    } else {
        false
    }
}

/// `ftMarioSpecialAirLwProcPhysics` @ 0x801566C4.
pub fn apply_mario_special_lw_air_physics(f: &mut Fighter) {
    if !f.mario_special_lw.rise_exhausted
        && f.mario_special_lw.rise_enabled
        && newly_pressed(f.prev_input.buttons, f.input.buttons).contains(N64Buttons::B)
    {
        f.physics.vel_air.y =
            (f.physics.vel_air.y + MARIO_TORNADO_VEL_Y_TAP).min(MARIO_TORNADO_VEL_Y_CLAMP);
    }
    physics::apply_gravity_default(&mut f.physics, &f.attributes);
    let clamp = mario_tornado_clamp(f);
    physics::clamp_air_vel_x_stick_range(
        &mut f.physics,
        f.input.stick_x,
        0,
        MARIO_TORNADO_VEL_X_AIR,
        clamp,
    );
}

/// `ftCommonLandingFallSpecialSetStatus` @ `ftcommonlanding.c:89`. No
/// extracted animation length (module docs elsewhere on collapsed landing
/// statuses — `set_guard_on`'s doc comment covers the pattern), so this
/// resolves into `Wait` on its very next update tick.
pub fn set_landing_fall_special(f: &mut Fighter) {
    set_status(f, Status::LandingFallSpecial, 0.0, StatusTiming::unknown());
}

// ---------------------------------------------------------------------------
// Jab combo
// ---------------------------------------------------------------------------

/// `FTCOMMON_ATTACK1_FOLLOWUP_FRAMES_DEFAULT` — `ft/ftcommon.h`. `Attack12`
/// uses it for every fighter; `Attack11` reads the fighter's own attribute
/// (`attack11_followup_frames`).
pub const ATTACK1_FOLLOWUP_FRAMES_DEFAULT: f32 = 24.0;

/// `FTStruct::attack1_followup_frames`/`status_vars.common.attack1.is_goto_followup`.
///
/// The original tests these across two separate callbacks per frame
/// (`ftCommonAttack11ProcUpdate` then `ftCommonAttack11ProcInterrupt` →
/// `ftCommonAttack12CheckGoto`): a tap while the window is open either
/// chains immediately (if the current hit's animation has already ended —
/// the motion script's own `SetFlag1(1)`) or sets `is_goto_followup` so the
/// very next frame that ends the animation chains instead of going to
/// `Wait`. Since `Jab1`/`Jab2`'s `SetFlag1(1)` always lands exactly at the
/// script's own total length (`crate::attack::MARIO_JAB2`'s doc comment),
/// `StatusState::animation_ended` already *is* that flag — no separate one
/// is needed, and `update`'s `Attack11` arm collapses both original
/// callbacks into a single per-frame check without changing the outcome in
/// any case that isn't a same-frame status-transition race neither engine's
/// callback order is possible to fully pin down without the original binary
/// running.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Attack1State {
    pub followup_frames: f32,
    pub is_goto_followup: bool,
    /// Fox rapid-jab input count. Source counts both A taps and releases.
    pub rapid_input_count: u8,
    pub rapid_requested: bool,
    pub rapid_keep_loop: bool,
}

// ---------------------------------------------------------------------------
// Ledges
// ---------------------------------------------------------------------------

/// `FTCOMMON_CLIFF_*` — `ft/ftcommon.h`.
pub const CLIFF_CATCH_WAIT: u16 = 30;
pub const CLIFF_DAMAGE_HIGH: u16 = 100;
pub const CLIFF_FALL_WAIT_DAMAGE_LOW: i32 = 1080;
pub const CLIFF_FALL_WAIT_DAMAGE_HIGH: i32 = 480;
pub const CLIFF_MOTION_STICK_RANGE_MIN: i32 = 20;
/// The real `800.0F` corner-proximity tolerance from
/// `mpProcessCheckTestLCliffCollision`/`RCliffCollision` @ `mpprocess.c:1031,1082`.
const CLIFF_CATCH_CORNER_RANGE: f32 = 800.0;
/// `tan(50°)`. `ftCommonCliffClimbOrFallCheckInterruptCommon`'s real test is
/// `ftParamGetStickAngleRads(fp) > F_CST_DTOR32(50.0F)`, where the angle is
/// `atan2(stick_y, |stick_x|)` — reframed here as the equivalent slope
/// comparison (`stick_y > tan(50°) * |stick_x|`) so it needs no `atan2`
/// (`ssb_engine::math` has none).
const CLIFF_MOTION_ANGLE_TAN_50: f32 = 1.191_753_6;

/// `FTStruct::status_vars.common.cliffwait`/`cliffmotion`, plus which floor
/// line's edge is held (`coll_data.cliff_id`).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct CliffState {
    pub line: u16,
    /// Frames left before letting go automatically —
    /// `ftCommonCliffWaitSetStatus`'s damage-dependent `fall_wait`.
    pub fall_wait: i32,
    /// A climb/drop input only counts once the stick has passed back through
    /// neutral since the catch — `ftCommonCliffClimbOrFallCheckInterruptCommon`'s
    /// own `is_allow_interrupt` latch.
    pub is_allow_interrupt: bool,
}

/// `mpProcessCheckTestLCliffCollision`/`RCliffCollision` @
/// `mpprocess.c:1031,1082`, wrapped by `mpCommonProcFighterCliff` @
/// `mpcommon.c:584`, restricted to `cliffcatch_coll == (0, 0)` — the
/// per-character hand-reach offset (`FTAttributes.cliffcatch_coll`) is not in
/// the extracted attribute range yet, the same class of gap as
/// `crate::attack`'s hitbox-offset simplifications — and dropping the
/// material-4 exemption the original's left-side test has.
///
/// `from`/`to` are the fighter's position before/after the movement being
/// tested; the caller supplies both because, like [`try_rebirth`]'s respawn
/// point, this needs data external to a single `Fighter`. Returns the caught
/// line and the exact corner point to hang from, or `None`. The caller still
/// owns ledge-hog exclusivity (the original's loop over every other
/// fighter checking `is_cliff_hold && same cliff_id && same lr`) — that
/// needs match-wide fighter awareness no function taking one `Fighter` can
/// have, so it is a filter the caller applies to this function's result
/// before calling [`set_cliff_catch`], not part of this function.
pub fn cliff_catch_candidate<I, F>(
    facing: Facing,
    cliffcatch_wait: u16,
    from: Vec3,
    to: Vec3,
    floors: F,
) -> Option<(u16, Vec2)>
where
    F: Fn() -> I,
    I: IntoIterator<Item = (u16, Segment)>,
{
    if cliffcatch_wait != 0 {
        return None;
    }
    let hit = collision::check_floor(floors(), Vec2::new(from.x, from.y), Vec2::new(to.x, to.y))?;
    if hit.flags & collision::flags::CLIFF == 0 {
        return None;
    }
    let seg = floors().into_iter().find(|(id, _)| *id == hit.line)?.1;
    let (left, right) = if seg.x1 <= seg.x2 {
        (
            Vec2::new(seg.x1 as f32, seg.y1 as f32),
            Vec2::new(seg.x2 as f32, seg.y2 as f32),
        )
    } else {
        (
            Vec2::new(seg.x2 as f32, seg.y2 as f32),
            Vec2::new(seg.x1 as f32, seg.y1 as f32),
        )
    };
    // `lr == +1` tests the left corner (`mpCollisionGetFloorEdgeL`), `lr ==
    // -1` the right — see the module docs' offset simplification for why
    // this collapses the original's two separate L/R sweeps into one.
    let corner = if facing == Facing::Right { left } else { right };
    if (hit.point.x - corner.x).abs() >= CLIFF_CATCH_CORNER_RANGE {
        return None;
    }
    Some((hit.line, corner))
}

/// `ftCommonCliffCatchSetStatus` @ `ftcommoncliffcatchwait.c:39`, minus the
/// capture-immunity mask and Samus-effect side cases. The fighter hangs
/// exactly at `corner` — no per-character reach offset exists yet (module
/// docs), so there is no arm's-length gap the way a real hang has one.
pub fn set_cliff_catch(f: &mut Fighter, line: u16, corner: Vec2) {
    f.cliff.line = line;
    f.pos = Vec3::new(corner.x, corner.y, f.pos.z);
    f.situation = Situation::Air;
    f.physics = crate::physics::PhysicsState::default();
    set_status(f, Status::CliffCatch, 0.0, StatusTiming::unknown());
}

/// `ftCommonCliffWaitSetStatus` @ `ftcommoncliffcatchwait.c:98`.
pub fn set_cliff_wait(f: &mut Fighter) {
    set_status(f, Status::CliffWait, 0.0, StatusTiming::unknown());
    f.cliff.is_allow_interrupt = false;
    f.cliff.fall_wait = if f.damage < CLIFF_DAMAGE_HIGH {
        CLIFF_FALL_WAIT_DAMAGE_LOW
    } else {
        CLIFF_FALL_WAIT_DAMAGE_HIGH
    };
}

/// `ftCommonCliffQuickOrSlowSetStatus` @ `ftcommoncliffclimb.c:58`, jumping
/// straight to the requested action's `Quick1`/`Slow1` status rather than
/// passing through the intermediate `CliffQuick`/`CliffSlow` dispatch
/// status — that status has no extracted animation length either, so it
/// would resolve on the very next tick regardless (same collapse as
/// [`set_guard_on`]). The real, damage-dependent Quick/Slow choice this
/// makes is preserved exactly.
fn set_cliff_action(f: &mut Fighter, quick1: Status, slow1: Status) {
    let status = if f.damage < CLIFF_DAMAGE_HIGH {
        quick1
    } else {
        slow1
    };
    set_status(f, status, 0.0, StatusTiming::unknown());
}

/// `ftCommonCliffClimbOrFallCheckInterruptCommon` @ `ftcommoncliffclimb.c:84`.
fn check_cliff_climb_or_fall(f: &mut Fighter) -> bool {
    let x = f.stick.x as f32;
    let y = f.stick.y as f32;
    if x.abs() < CLIFF_MOTION_STICK_RANGE_MIN as f32
        && y.abs() < CLIFF_MOTION_STICK_RANGE_MIN as f32
    {
        f.cliff.is_allow_interrupt = true;
        return false;
    }
    if !f.cliff.is_allow_interrupt {
        return false;
    }
    let steep_up = y > CLIFF_MOTION_ANGLE_TAN_50 * x.abs();
    let forward = x * f.facing.sign() >= 0.0;
    let not_steep_down = y > -CLIFF_MOTION_ANGLE_TAN_50 * x.abs();
    if steep_up || (not_steep_down && forward) {
        set_cliff_action(f, Status::CliffClimbQuick1, Status::CliffClimbSlow1);
    } else {
        // Holding away and down lets go outright — `ftCommonFallSetStatus`,
        // not `DamageFall`; that one is only the wait-timeout's exit.
        f.cliffcatch_wait = CLIFF_CATCH_WAIT;
        set_fall(f);
    }
    true
}

/// `ftCommonCliffWaitProcInterrupt` @ `ftcommoncliffcatchwait.c:89`, in the
/// original's own priority order: attack, then escape, then climb-or-fall,
/// then the auto-release timeout ([`ftCommonCliffWaitCheckFall`]).
fn update_cliff_wait(f: &mut Fighter) {
    let tapped = newly_pressed(f.prev_input.buttons, f.input.buttons);
    if tapped.contains(N64Buttons::A | N64Buttons::B) {
        set_cliff_action(f, Status::CliffAttackQuick1, Status::CliffAttackSlow1);
    } else if tapped.contains(N64Buttons::Z) {
        set_cliff_action(f, Status::CliffEscapeQuick1, Status::CliffEscapeSlow1);
    } else if !check_cliff_climb_or_fall(f) {
        f.cliff.fall_wait -= 1;
        if f.cliff.fall_wait <= 0 {
            f.cliffcatch_wait = CLIFF_CATCH_WAIT;
            set_damage_fall(f);
        }
    }
}

/// `ftCommonCliffClimbQuick1ProcUpdate`/`...Slow1ProcUpdate`/the matching
/// `Attack`/`Escape` pairs @ `ftcommoncliffclimb.c:117,123`, `ftcommoncliffattack.c:24,30`,
/// `ftcommoncliffescape.c:24,30` — no extracted animation length for any of
/// them, so each resolves into its own `...2` status on the next tick,
/// collapsing what is a real multi-frame climb/attack/dodge animation into
/// one tick (same class of gap as [`set_guard_on`]).
fn cliff_phase1_to_2(status: Status) -> Status {
    match status {
        Status::CliffClimbQuick1 => Status::CliffClimbQuick2,
        Status::CliffClimbSlow1 => Status::CliffClimbSlow2,
        Status::CliffAttackQuick1 => Status::CliffAttackQuick2,
        Status::CliffAttackSlow1 => Status::CliffAttackSlow2,
        Status::CliffEscapeQuick1 => Status::CliffEscapeQuick2,
        Status::CliffEscapeSlow1 => Status::CliffEscapeSlow2,
        _ => unreachable!("only called with a cliff phase-1 status"),
    }
}

/// `ftCommonCliffCommon2UpdateCollData` @ `ftcommoncliffclimb.c:231`, reduced
/// to the position it always leaves the fighter at (the ledge corner plus
/// five units back onto the stage) — the per-character
/// `attr->cliff_status_ga` table that lets some recovery options end
/// airborne is not extracted, so every option here ends grounded, matching
/// the common case (climbing/attacking/rolling up all land on stage).
fn end_cliff_recovery(f: &mut Fighter) {
    f.pos.x += 5.0 * f.facing.sign();
    f.situation = Situation::Ground;
    f.physics = crate::physics::PhysicsState::default();
    set_wait(f);
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
    set_any_status(f, AnyStatus::Common(status), anim_frame_begin, timing);
}

/// [`set_status`], generalized to also accept an extended
/// ([`AnyStatus::Mario`]-style) status — see [`AnyStatus`]'s docs.
pub fn set_any_status(
    f: &mut Fighter,
    status: AnyStatus,
    anim_frame_begin: f32,
    timing: StatusTiming,
) {
    // `mpCommonSetFighterGround` / `...Air`: the situation follows the status,
    // and leaving the ground has to move the velocity across.
    match (f.situation, status.is_grounded()) {
        (Situation::Ground, false) => f.become_airborne(),
        (Situation::Air, true) => {
            f.situation = Situation::Ground;
            f.physics.vel_ground.x = f.physics.vel_air.x;
            f.physics.vel_air = ssb_engine::math::Vec3::ZERO;
            f.physics.is_fastfall = false;
            f.samus.charge_recoil = 0;
        }
        _ => {}
    }
    f.status.status = status;
    // `ftMainSetStatus` clears it; the setters that allow a boomerang catch
    // set it again afterwards.
    f.is_special_interrupt = false;
    // `ftMainSetStatus` resets `knockback_resist_status`; Yoshi's aerial
    // jump sets it again.
    f.knockback_resist = 0.0;
    // `ftCommonEntry`, `ftCommonDead`, `ftCommonRebirth`, and sleep each
    // toggle `FTStruct::is_shadow_hide`. Keep the source-owned flag portable
    // so rendering has one display gate and capture systems can use it too.
    f.is_shadow_hidden = crate::shadow::status_hides_shadow(status);
    f.is_invisible = false;
    f.status.anim_frame = anim_frame_begin;
    f.status.timing = timing;
}

/// `ftCommonWaitSetStatus` @ 0x8013E1C8.
pub fn set_wait(f: &mut Fighter) {
    set_status(f, Status::Wait, 0.0, StatusTiming::unknown());
    f.is_special_interrupt = true;
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
    f.is_special_interrupt = status != Status::WalkFast;
}

/// `ftCommonDashSetStatus` @ 0x8013ED00.
///
/// Dash sets the whole ground velocity at once rather than accelerating into
/// it — the initial burst is `dash_speed` on frame one, and `dash_decel` eats
/// it from frame 7 (`FTCOMMON_DASH_DECELERATE_BEGIN`).
pub fn set_dash(f: &mut Fighter) {
    let len = f.anim.dash;
    set_status(f, Status::Dash, 0.0, StatusTiming::animation(len, 1.0));
    f.physics.vel_ground.x = f.attributes.dash_speed * f.facing.sign();
    f.stick.tap_x = STICKBUFFER_MAX;
}

/// `ftCommonRunSetStatus` @ 0x8013EEE8.
pub fn set_run(f: &mut Fighter) {
    set_status(f, Status::Run, 0.0, StatusTiming::unknown());
    f.physics.vel_ground.x = f.attributes.run_speed * f.facing.sign();
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
    f.is_special_interrupt = true;
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
    f.is_special_interrupt = true;
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
    // `jumps_used` counts the ground jump too. The original sets it to 1 at
    // takeoff, leaving only `jumps_max - 1` aerial jumps available.
    f.physics.jumps_used = 1;
    f.stick.tap_y = STICKBUFFER_MAX;
    f.is_special_interrupt = true;
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
    f.is_special_interrupt = true;
    if crate::yoshi::is_yoshi(f.kind) {
        crate::yoshi::set_jump_aerial(f);
    }
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
    f.is_special_interrupt = true;
}

/// `mpCommonSetFighterWaitOrFall`: Wait on the ground, Fall in the air.
pub fn set_wait_or_fall(f: &mut Fighter) {
    if f.is_grounded() {
        set_wait(f);
    } else {
        set_fall(f);
    }
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

/// `ftCommonLandingAirSetStatus`, for a fighter with a dedicated
/// `LandingAirX` motion file for the aerial they landed out of. No
/// animation length is extracted for it (module docs), so — like
/// [`set_guard_on`] — it resolves into `Wait` on its very next update tick
/// rather than holding for its real multi-frame recovery.
fn set_landing_air(f: &mut Fighter, status: Status) {
    set_status(f, status, 0.0, StatusTiming::unknown());
}

/// `ftCommonLandingAirNullSetStatus`, for a fighter with no dedicated
/// `LandingAirX` motion file for the aerial they landed out of —
/// `F_PCT_TO_DEC(flag1)` scales the fighter's own normal landing-lag length
/// (`f.anim.landing`), which is real, extracted data, unlike
/// [`set_landing_air`]'s case.
fn set_landing_air_null(f: &mut Fighter, percent: u8) {
    let len = f.anim.landing * (percent as f32 / 100.0);
    set_status(f, Status::LandingAirNull, 0.0, StatusTiming::frames(len));
}

/// `ftCommonAttackAirProcMap` @ `ftcommonattackair.c:50`, reduced to its
/// "still mid-move" branch: the original also has a `vel_air.y >
/// FTCOMMON_ATTACKAIR_SKIPLANDING_VEL_Y_MAX` branch that skips landing lag
/// entirely (falling too slowly to have "committed" to the swing) and a
/// third branch for landing after the move's own landing-lag window has
/// already closed (plain [`set_landing`]). Both need the motion script's
/// `SetFlag1`/`SetFlag1(0)` timing, which — for every Mario aerial ported so
/// far — brackets almost the entire move (on a few frames in, off right at
/// the very end), so landing while still in the attack status is the
/// overwhelmingly common real case this collapses to.
///
/// Falls back to the plain [`set_landing`] for any status/fighter this
/// isn't ported for yet, or that has no aerial `MoveData` (its
/// `landing_lag_percent` is only meaningful there).
pub fn set_landing_or_landing_air(f: &mut Fighter) {
    let fox_reflector_ground = match f.status.status {
        AnyStatus::Fox(FoxStatus::SpecialAirLwStart) => {
            Some((FoxStatus::SpecialLwStart, StatusTiming::frames(4.0)))
        }
        AnyStatus::Fox(FoxStatus::SpecialAirLwLoop) => {
            Some((FoxStatus::SpecialLwLoop, StatusTiming::unknown()))
        }
        AnyStatus::Fox(FoxStatus::SpecialAirLwHit) => {
            Some((FoxStatus::SpecialLwHit, StatusTiming::frames(6.0)))
        }
        AnyStatus::Fox(FoxStatus::SpecialAirLwEnd) => {
            Some((FoxStatus::SpecialLwEnd, StatusTiming::frames(18.0)))
        }
        AnyStatus::Fox(FoxStatus::SpecialAirLwTurn) => {
            Some((FoxStatus::SpecialLwTurn, StatusTiming::unknown()))
        }
        _ => None,
    };
    if let Some((status, timing)) = fox_reflector_ground {
        set_any_status(f, AnyStatus::Fox(status), f.status.anim_frame, timing);
        return;
    }
    let fox_ground = match f.status.status {
        AnyStatus::Fox(FoxStatus::SpecialAirHiStart) => Some((FoxStatus::SpecialHiStart, 8.0)),
        AnyStatus::Fox(FoxStatus::SpecialAirHiHold) => Some((FoxStatus::SpecialHiHold, 0.0)),
        AnyStatus::Fox(FoxStatus::SpecialAirHiEnd) => Some((FoxStatus::SpecialHiEnd, 29.0)),
        _ => None,
    };
    if let Some((status, length)) = fox_ground {
        let timing = if length == 0.0 {
            StatusTiming::unknown()
        } else {
            StatusTiming::frames(length)
        };
        set_any_status(f, AnyStatus::Fox(status), f.status.anim_frame, timing);
        return;
    }
    if f.status.status == AnyStatus::Fox(FoxStatus::SpecialAirHi) {
        set_any_status(
            f,
            AnyStatus::Fox(FoxStatus::SpecialHiEnd),
            0.0,
            StatusTiming::frames(29.0),
        );
        return;
    }
    if f.status.status == AnyStatus::Mario(MarioStatus::SpecialAirN) {
        return switch_mario_fireball_ground(f);
    }
    if f.status.status == AnyStatus::Mario(MarioStatus::SpecialAirLw) {
        return switch_mario_tornado_ground(f);
    }
    let AnyStatus::Common(current) = f.status.status else {
        return set_landing(f);
    };
    match current {
        // Samus has no `LandingAirF`/`LandingAirLw` motion (`dFTSamusMotionDescs`),
        // and Luigi none for `LandingAirLw` (`dFTLuigiMotionDescs`).
        Status::AttackAirF | Status::AttackAirLw
            if f.kind == crate::fighter::FighterKind::Samus =>
        {
            let percent = crate::attack::move_data(f.kind, current.into())
                .and_then(|m| m.landing_lag_percent)
                .unwrap_or(100);
            set_landing_air_null(f, percent);
        }
        // Link has `LandingAirF` and `LandingAirLw` (`LandingAirD`) motions
        // only (`dFTLinkMotionDescs`).
        Status::AttackAirB | Status::AttackAirHi if f.kind == crate::fighter::FighterKind::Link => {
            let percent = crate::attack::move_data(f.kind, current.into())
                .and_then(|m| m.landing_lag_percent)
                .unwrap_or(100);
            set_landing_air_null(f, percent);
        }
        Status::AttackAirLw if f.kind == crate::fighter::FighterKind::Luigi => {
            let percent = crate::attack::move_data(f.kind, current.into())
                .and_then(|m| m.landing_lag_percent)
                .unwrap_or(100);
            set_landing_air_null(f, percent);
        }
        Status::AttackAirF => set_landing_air(f, Status::LandingAirF),
        Status::AttackAirB => set_landing_air(f, Status::LandingAirB),
        // Fox and Yoshi have `LandingAirF` and `LandingAirB` only.
        Status::AttackAirHi | Status::AttackAirLw
            if matches!(
                f.kind,
                crate::fighter::FighterKind::Fox | crate::fighter::FighterKind::Yoshi
            ) =>
        {
            let percent = crate::attack::move_data(f.kind, current.into())
                .and_then(|m| m.landing_lag_percent)
                .unwrap_or(100);
            set_landing_air_null(f, percent);
        }
        Status::AttackAirHi => set_landing_air(f, Status::LandingAirHi),
        Status::AttackAirLw => set_landing_air(f, Status::LandingAirLw),
        Status::AttackAirN => {
            let percent = crate::attack::move_data(f.kind, Status::AttackAirN.into())
                .and_then(|m| m.landing_lag_percent)
                .unwrap_or(100);
            set_landing_air_null(f, percent);
        }
        // `ftCommonFallSpecialProcMap` @ `ftcommonfallspecial.c:51`, minus
        // the cliff-catch branch — no status in this codebase auto-catches
        // a ledge yet, ledge detection is caller-invoked
        // (`cliff_catch_candidate`'s own docs), so this is the same
        // pre-existing gap, not a new one — and `is_allow_pass`'s
        // drop-through nuance (`FallSpecialState`'s docs).
        Status::FallSpecial => {
            if f.fall_special.is_goto_landing
                || f.physics.vel_air.y < FALLSPECIAL_SKIPLANDING_VEL_Y_MAX
            {
                set_landing_fall_special(f);
            } else {
                set_wait(f);
            }
        }
        _ => set_landing(f),
    }
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

/// `ftCommonAttack11SetStatus` @ `ftcommonattack1.c:127`, reduced to the
/// no-item case. Chains into `Attack12` on a well-timed repeated tap (see
/// [`Attack1State`] and `update`'s `Attack11` arm); `Attack12`'s own
/// further chain into `Attack13` is still not ported, since `Attack13` is a
/// per-character status beyond the common 0..=219 table this codebase does
/// not have an extension point for yet.
pub fn set_attack11(f: &mut Fighter) {
    set_status(
        f,
        Status::Attack11,
        0.0,
        StatusTiming::frames(attack_length(f, Status::Attack11)),
    );
    f.attack1 = Attack1State {
        followup_frames: attack11_followup_frames(f.kind),
        is_goto_followup: false,
        ..Attack1State::default()
    };
}

/// `FTAttributes::attack1_followup_frames`, which `ftCommonAttack11SetStatus`
/// loads (`<Fighter>Main.c`). `Attack12` always uses
/// [`ATTACK1_FOLLOWUP_FRAMES_DEFAULT`].
fn attack11_followup_frames(kind: crate::fighter::FighterKind) -> f32 {
    use crate::fighter::FighterKind;
    let base = match kind {
        FighterKind::MetalMario => FighterKind::Mario,
        FighterKind::GiantDonkey => FighterKind::Donkey,
        k => k.polygon_base().unwrap_or(k),
    };
    match base {
        FighterKind::Fox | FighterKind::Samus => 30.0,
        FighterKind::Donkey => 28.0,
        _ => ATTACK1_FOLLOWUP_FRAMES_DEFAULT,
    }
}

/// `ftCommonAttack12SetStatus` @ `ftcommonattack1.c:152`, reduced to the
/// no-item case (`attack1_followup_frames` is the same default for every
/// fighter this batch covers, so the original's per-`fkind` `switch` that
/// all resolves to the same constant is not reproduced as one).
pub fn set_attack12(f: &mut Fighter) {
    let len = attack_length(f, Status::Attack12);
    set_status(f, Status::Attack12, 0.0, StatusTiming::frames(len));
    let rapid_input_count = f.attack1.rapid_input_count;
    let rapid_requested = f.attack1.rapid_requested;
    f.attack1 = Attack1State {
        followup_frames: ATTACK1_FOLLOWUP_FRAMES_DEFAULT,
        is_goto_followup: false,
        rapid_input_count,
        rapid_requested,
        rapid_keep_loop: false,
    };
}

/// `ftCommonAttack100StartCheckInterruptCommon`'s `inputs_min` for the
/// fighters this codebase ports: `None` when the fighter has no `Attack100`.
fn rapid_inputs_min(kind: crate::fighter::FighterKind) -> Option<u8> {
    match kind {
        crate::fighter::FighterKind::Fox => Some(4),
        crate::fighter::FighterKind::Link => Some(5),
        crate::fighter::FighterKind::Captain => Some(6),
        _ => None,
    }
}

/// The counting half of `ftCommonAttack100StartCheckInterruptCommon`: both
/// A taps and A releases count. Returns whether the count has reached the
/// fighter's minimum.
pub(crate) fn rapid_input(f: &mut Fighter) -> bool {
    let Some(min) = rapid_inputs_min(f.kind) else {
        return false;
    };
    if newly_pressed(f.prev_input.buttons, f.input.buttons).contains(N64Buttons::A)
        || newly_released(f.prev_input.buttons, f.input.buttons).contains(N64Buttons::A)
    {
        f.attack1.rapid_input_count = f.attack1.rapid_input_count.saturating_add(1);
        if f.attack1.rapid_input_count >= min {
            f.attack1.rapid_requested = true;
            return true;
        }
    }
    false
}

/// The motion-script frame at which a jab's `SetFlag1(1)` lands, for a
/// fighter whose flag lands before the figatree ends. `None` keeps the
/// collapsed "flag 1 is the animation end" reading (`Attack1State`).
fn attack1_flag1_frame(kind: crate::fighter::FighterKind, status: Status) -> Option<f32> {
    match (kind, status) {
        (crate::fighter::FighterKind::Link, Status::Attack11) => {
            Some(crate::link_attack::JAB1_FLAG1_FRAME)
        }
        (crate::fighter::FighterKind::Link, Status::Attack12) => {
            Some(crate::link_attack::JAB2_FLAG1_FRAME)
        }
        // `Jab1` sets flag 1 at frame 10 of 24; `Jab2` sets none, and Yoshi
        // has no `Attack13` to chain into.
        (crate::fighter::FighterKind::Yoshi, Status::Attack11) => {
            Some(crate::yoshi_attack::JAB1_FLAG1_FRAME)
        }
        (crate::fighter::FighterKind::Captain, Status::Attack11) => {
            Some(crate::captain_attack::JAB1_FLAG1_FRAME)
        }
        (crate::fighter::FighterKind::Captain, Status::Attack12) => {
            Some(crate::captain_attack::JAB2_FLAG1_FRAME)
        }
        _ => None,
    }
}

/// `ftCommonAttack11ProcUpdate`, then `ftCommonAttack11ProcInterrupt`, in
/// source order, for a jab whose flag 1 lands mid-animation.
fn update_attack11_flagged(f: &mut Fighter) {
    let flag1 = attack1_flag1_frame(f.kind, Status::Attack11)
        .is_some_and(|frame| f.status.anim_frame >= frame);
    if flag1 && f.attack1.is_goto_followup {
        return set_attack12(f);
    }
    if f.status.animation_ended() {
        return set_wait(f);
    }
    if f.status.anim_frame <= 2.0 && crate::grab::check_catch_attack11(f) {
        return;
    }
    // `ftCommonAttack100StartCheckInterruptCommon` cannot start the loop
    // from `Attack11`; it only records the request.
    rapid_input(f);
    // `ftCommonAttack12CheckGoto`.
    if f.attack1.followup_frames > 0.0 {
        f.attack1.followup_frames -= f.status.timing.anim_speed;
        if newly_pressed(f.prev_input.buttons, f.input.buttons).contains(N64Buttons::A) {
            if flag1 {
                return set_attack12(f);
            }
            f.attack1.is_goto_followup = true;
        }
    }
}

/// `ftCommonAttack12ProcUpdate`, then `ftCommonAttack12ProcInterrupt`
/// (`ftCommonAttack13CheckGoto`, then
/// `ftCommonAttack100StartCheckInterruptCommon`), in source order.
fn update_attack12_flagged(f: &mut Fighter) {
    let flag1 = attack1_flag1_frame(f.kind, Status::Attack12)
        .is_some_and(|frame| f.status.anim_frame >= frame);
    if flag1 && f.attack1.rapid_requested && f.kind != crate::fighter::FighterKind::Captain {
        return set_rapid_start(f);
    }
    if flag1 && f.attack1.is_goto_followup {
        if let Some(status) = attack13_status(f.kind) {
            return set_attack13(f, status);
        }
    }
    if f.status.animation_ended() {
        return set_wait(f);
    }
    if f.attack1.followup_frames > 0.0 && attack13_status(f.kind).is_some() {
        f.attack1.followup_frames -= f.status.timing.anim_speed;
        if newly_pressed(f.prev_input.buttons, f.input.buttons).contains(N64Buttons::A) {
            if flag1 {
                let status = attack13_status(f.kind).expect("checked above");
                return set_attack13(f, status);
            }
            f.attack1.is_goto_followup = true;
        }
    }
    if rapid_input(f) && flag1 && f.kind != crate::fighter::FighterKind::Captain {
        set_rapid_start(f);
    }
}

/// `ftCommonAttack100StartSetStatus` for the fighters with their own
/// `Attack100` statuses.
fn set_rapid_start(f: &mut Fighter) {
    match f.kind {
        crate::fighter::FighterKind::Fox => set_fox_rapid_start(f),
        crate::fighter::FighterKind::Link => crate::link::set_attack100_start(f),
        crate::fighter::FighterKind::Captain => crate::captain::set_attack100_start(f),
        _ => unreachable!("no Attack100 for this fighter"),
    }
}

fn set_fox_rapid_start(f: &mut Fighter) {
    // ROM figatree file 753, `FTFoxAnimJabLoopStart`: 8 frames.
    set_any_status(
        f,
        AnyStatus::Fox(FoxStatus::Attack100Start),
        0.0,
        StatusTiming::frames(8.0),
    );
    f.attack1.rapid_keep_loop = false;
}

fn set_fox_rapid_loop(f: &mut Fighter) {
    // File 754 loops; its motion script has five pulse windows over 32 frames.
    set_any_status(
        f,
        AnyStatus::Fox(FoxStatus::Attack100Loop),
        0.0,
        StatusTiming::frames(32.0),
    );
    f.attack1.rapid_keep_loop = false;
}

fn set_fox_rapid_end(f: &mut Fighter) {
    // ROM figatree file 755, `FTFoxAnimJabLoopEnd`: 8 frames.
    set_any_status(
        f,
        AnyStatus::Fox(FoxStatus::Attack100End),
        0.0,
        StatusTiming::frames(8.0),
    );
}

/// Frame length for a status from `crate::attack::move_data`, or `0.0` if
/// this fighter/status has no ported moveset data — the same "no length
/// known" fallback [`StatusTiming::unknown`] already covers for statuses
/// with no known length.
fn attack_length(f: &Fighter, status: Status) -> f32 {
    crate::attack::move_data(f.kind, status.into())
        .map(|m| m.length_frames)
        .unwrap_or(0.0)
}

/// `ftCommonAttackDashSetStatus` @ `ftcommonattackdash.c:10`.
pub fn set_dash_attack(f: &mut Fighter) {
    let len = attack_length(f, Status::AttackDash);
    set_status(f, Status::AttackDash, 0.0, StatusTiming::frames(len));
}

/// How many angled variants of a forward tilt or forward smash a fighter's
/// motion table carries. `ftCommonAttackS3SetStatus` and
/// `ftCommonAttackS4SetStatus` test `motion_desc[...HiS].anim_file_id` for
/// the five-angle branch, then `motion_desc[...Hi]` for the three-angle one;
/// a fighter with neither always gets the straight move.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AngleVariants {
    One,
    Three,
    Five,
}

/// Read from each fighter's `dFT<Name>MotionDescs`.
fn ftilt_variants(kind: crate::fighter::FighterKind) -> AngleVariants {
    use crate::fighter::FighterKind;
    match crate::grab::base_kind(kind) {
        FighterKind::Fox | FighterKind::Samus | FighterKind::Captain => AngleVariants::Five,
        FighterKind::Link => AngleVariants::One,
        _ => AngleVariants::Three,
    }
}

fn fsmash_variants(kind: crate::fighter::FighterKind) -> AngleVariants {
    use crate::fighter::FighterKind;
    match crate::grab::base_kind(kind) {
        FighterKind::Fox | FighterKind::Link => AngleVariants::One,
        FighterKind::Yoshi | FighterKind::Captain => AngleVariants::Three,
        _ => AngleVariants::Five,
    }
}

/// `ftCommonAttackS3SetStatus` @ `ftcommonattacks3.c:10`.
pub fn set_ftilt(f: &mut Fighter) {
    let x = f.stick.x as f32;
    let y = f.stick.y as f32;
    let variants = ftilt_variants(f.kind);
    let five = variants == AngleVariants::Five;
    let three = variants == AngleVariants::Three;
    let status = if five && y > ATTACKS3_5ANGLE_TAN_30 * x.abs() {
        Status::AttackS3Hi
    } else if five && y > ATTACKS3_5ANGLE_TAN_10 * x.abs() {
        Status::AttackS3HiS
    } else if five && y < -ATTACKS3_5ANGLE_TAN_30 * x.abs() {
        Status::AttackS3Lw
    } else if five && y < -ATTACKS3_5ANGLE_TAN_10 * x.abs() {
        Status::AttackS3LwS
    } else if three && y > ATTACKS3_3ANGLE_TAN_17 * x.abs() {
        Status::AttackS3Hi
    } else if three && y < -ATTACKS3_3ANGLE_TAN_17 * x.abs() {
        Status::AttackS3Lw
    } else {
        Status::AttackS3
    };
    let len = attack_length(f, status);
    set_status(f, status, 0.0, StatusTiming::frames(len));
}

/// `ftCommonAttackHi3SetStatus` @ `ftcommonattackhi3.c:10`, reduced to the
/// no-`Hi3F`/`Hi3B` case — Mario has no forward/backward-leaning up-tilt
/// motion files, so (as with [`set_ftilt`]) this is exactly Mario's real
/// behaviour, not a cut-down version of it.
pub fn set_utilt(f: &mut Fighter) {
    let len = attack_length(f, Status::AttackHi3);
    set_status(f, Status::AttackHi3, 0.0, StatusTiming::frames(len));
}

/// `ftCommonAttackLw3SetStatus` @ `ftcommonattacklw3.c:59`.
pub fn set_dtilt(f: &mut Fighter) {
    let len = attack_length(f, Status::AttackLw3);
    set_status(f, Status::AttackLw3, 0.0, StatusTiming::frames(len));
}

/// The status-setting half of `ftCommonAttackAirCheckInterruptCommon` @
/// `ftcommonattackair.c:71` (the direction dispatch lives in
/// [`check_attack_air`]) — `ftMainSetStatus(..., FTSTATUS_PRESERVE_FASTFALL)`
/// needs no explicit handling here: an already-airborne fighter entering
/// another airborne status is a no-op for [`set_status`]'s own ground/air
/// transfer, which is the only thing that ever touches `is_fastfall`.
pub fn set_air_attack(f: &mut Fighter, status: Status) {
    let len = attack_length(f, status);
    set_status(f, status, 0.0, StatusTiming::frames(len));
    f.link.dair_rehit_timer = 0;
    f.link.dair_cleared = false;
}

/// `ftCommonAttackS4SetStatus` @ `ftcommonattacks4.c:70`. The three-angle
/// branch uses the forward tilt's 17-degree split
/// (`FTCOMMON_ATTACKS4_3ANGLE_{HI,LW}_MIN`).
pub fn set_fsmash(f: &mut Fighter) {
    let x = f.stick.x as f32;
    let y = f.stick.y as f32;
    let status = match fsmash_variants(f.kind) {
        AngleVariants::One => Status::AttackS4,
        AngleVariants::Three if y > ATTACKS3_3ANGLE_TAN_17 * x.abs() => Status::AttackS4Hi,
        AngleVariants::Three if y < -ATTACKS3_3ANGLE_TAN_17 * x.abs() => Status::AttackS4Lw,
        AngleVariants::Three => Status::AttackS4,
        AngleVariants::Five => fsmash_five(x, y),
    };
    let len = attack_length(f, status);
    set_status(f, status, 0.0, StatusTiming::frames(len));
}

fn fsmash_five(x: f32, y: f32) -> Status {
    if y > ATTACKS4_5ANGLE_TAN_21 * x.abs() {
        Status::AttackS4Hi
    } else if y > ATTACKS4_5ANGLE_TAN_7 * x.abs() {
        Status::AttackS4HiS
    } else if y < -ATTACKS4_5ANGLE_TAN_21 * x.abs() {
        Status::AttackS4Lw
    } else if y < -ATTACKS4_5ANGLE_TAN_7 * x.abs() {
        Status::AttackS4LwS
    } else {
        Status::AttackS4
    }
}

/// `ftCommonAttackHi4SetStatus` @ `ftcommonattackhi4.c:10`.
pub fn set_usmash(f: &mut Fighter) {
    let len = attack_length(f, Status::AttackHi4);
    set_status(f, Status::AttackHi4, 0.0, StatusTiming::frames(len));
}

/// `ftCommonAttackLw4SetStatus` @ `ftcommonattacklw4.c:10`.
pub fn set_dsmash(f: &mut Fighter) {
    let len = attack_length(f, Status::AttackLw4);
    set_status(f, Status::AttackLw4, 0.0, StatusTiming::frames(len));
}

/// `ftCommonDamageFallSetStatusFromDamage` @ `ftcommondamagefall.c:53`,
/// reduced to the status change: hitstun over an airborne Damage/Fly status
/// ends into `DamageFall`, a plain fall the fighter is not yet fighting out
/// of — this is what [`update`]'s airborne-Damage arm calls.
pub fn set_damage_fall(f: &mut Fighter) {
    set_status(f, Status::DamageFall, 0.0, StatusTiming::unknown());
}

// ---------------------------------------------------------------------------
// Interrupt checks
// ---------------------------------------------------------------------------

/// `ftCommonKneeBendGetInputTypeCommon` @ 0x8013F474.
pub(crate) fn jump_input_type(f: &Fighter, stick_min: i32) -> JumpInput {
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

/// `ftCommonAttackDashCheckInterruptCommon` @ `ftcommonattackdash.c:24`,
/// minus the item-swing/light-throw branches (no items yet).
pub fn check_attack_dash(f: &mut Fighter) -> bool {
    if newly_pressed(f.prev_input.buttons, f.input.buttons).contains(N64Buttons::A) {
        set_dash_attack(f);
        return true;
    }
    false
}

/// `ftCommonAttackS4CheckInterruptCommon` @ `ftcommonattacks4.c:216`, minus
/// the item branches.
pub fn check_fsmash(f: &mut Fighter) -> bool {
    if !newly_pressed(f.prev_input.buttons, f.input.buttons).contains(N64Buttons::A) {
        return false;
    }
    if (f.stick.x as i32).abs() < ATTACKS4_STICK_RANGE_MIN
        || f.stick.tap_x >= ATTACKS4_BUFFER_TICS_MAX
    {
        return false;
    }
    set_fsmash(f);
    true
}

/// `ftCommonAttackHi4CheckInterruptCommon` @ `ftcommonattackhi4.c:60`, minus
/// the light-throw branch.
pub fn check_usmash(f: &mut Fighter) -> bool {
    if !newly_pressed(f.prev_input.buttons, f.input.buttons).contains(N64Buttons::A) {
        return false;
    }
    if (f.stick.y as i32) < ATTACKHI4_STICK_RANGE_MIN || f.stick.tap_y >= ATTACKHI4_BUFFER_TICS_MAX
    {
        return false;
    }
    set_usmash(f);
    true
}

/// `ftCommonAttackLw4CheckInterruptCommon` @ `ftcommonattacklw4.c:60`, minus
/// the light-throw branch.
pub fn check_dsmash(f: &mut Fighter) -> bool {
    if !newly_pressed(f.prev_input.buttons, f.input.buttons).contains(N64Buttons::A) {
        return false;
    }
    if (f.stick.y as i32) > ATTACKLW4_STICK_RANGE_MIN || f.stick.tap_y >= ATTACKLW4_BUFFER_TICS_MAX
    {
        return false;
    }
    set_dsmash(f);
    true
}

/// `ftCommonAttackS3CheckInterruptCommon` @ `ftcommonattacks3.c:39`, minus
/// the item branches. `ftParamGetStickAngleRads`'s `atan2(y, |x|)` gate
/// (`|angle| <= 50°`) is reframed as `|y| <= tan(50°) * |x|`, the same
/// slope-comparison trick [`CLIFF_MOTION_ANGLE_TAN_50`] uses.
pub fn check_ftilt(f: &mut Fighter) -> bool {
    if !newly_pressed(f.prev_input.buttons, f.input.buttons).contains(N64Buttons::A) {
        return false;
    }
    let x = f.stick.x as f32;
    let y = f.stick.y as f32;
    if x * f.facing.sign() < ATTACKS3_STICK_RANGE_MIN as f32 {
        return false;
    }
    if y.abs() > CLIFF_MOTION_ANGLE_TAN_50 * x.abs() {
        return false;
    }
    set_ftilt(f);
    true
}

/// `ftCommonAttackHi3CheckInterruptCommon` @ `ftcommonattackhi3.c:29`, minus
/// the light-throw branch.
pub fn check_utilt(f: &mut Fighter) -> bool {
    if !newly_pressed(f.prev_input.buttons, f.input.buttons).contains(N64Buttons::A) {
        return false;
    }
    if (f.stick.y as i32) < ATTACKHI3_STICK_RANGE_MIN {
        return false;
    }
    let x = f.stick.x as f32;
    let y = f.stick.y as f32;
    if y <= CLIFF_MOTION_ANGLE_TAN_50 * x.abs() {
        return false;
    }
    set_utilt(f);
    true
}

/// `ftCommonAttackLw3CheckInterruptCommon` @ `ftcommonattacklw3.c:70`, minus
/// the light-throw branch.
pub fn check_dtilt(f: &mut Fighter) -> bool {
    if !newly_pressed(f.prev_input.buttons, f.input.buttons).contains(N64Buttons::A) {
        return false;
    }
    if (f.stick.y as i32) > ATTACKLW3_STICK_RANGE_MIN {
        return false;
    }
    let x = f.stick.x as f32;
    let y = f.stick.y as f32;
    if y >= -CLIFF_MOTION_ANGLE_TAN_50 * x.abs() {
        return false;
    }
    set_dtilt(f);
    true
}

/// `ftCommonAttackAirCheckInterruptCommon` @ `ftcommonattackair.c:71`, minus
/// the item/hammer branches. Neutral (both axes under the range minimum)
/// goes to `AttackAirN`; otherwise the same `tan(50°)` angle split as the
/// ground tilts picks up/down, and forward-relative-to-facing picks
/// forward/back.
pub fn check_attack_air(f: &mut Fighter) -> bool {
    if !newly_pressed(f.prev_input.buttons, f.input.buttons).contains(N64Buttons::A) {
        return false;
    }
    let x = f.stick.x as f32;
    let y = f.stick.y as f32;
    let status = if x.abs() < ATTACKAIR_DIRECTION_STICK_RANGE_MIN as f32
        && y.abs() < ATTACKAIR_DIRECTION_STICK_RANGE_MIN as f32
    {
        Status::AttackAirN
    } else if y > CLIFF_MOTION_ANGLE_TAN_50 * x.abs() {
        Status::AttackAirHi
    } else if y < -CLIFF_MOTION_ANGLE_TAN_50 * x.abs() {
        Status::AttackAirLw
    } else if x * f.facing.sign() >= 0.0 {
        Status::AttackAirF
    } else {
        Status::AttackAirB
    };
    if crate::attack::move_data(f.kind, status.into()).is_none() {
        return false;
    }
    set_air_attack(f, status);
    true
}

/// `ftCommonAttack1CheckInterruptCommon` @ `ftcommonattack1.c:244`, restricted
/// to the no-item case (`fp->item_gobj == NULL`), which is every fighter in
/// this slice — Training has no items.
pub fn check_attack1(f: &mut Fighter) -> bool {
    if newly_pressed(f.prev_input.buttons, f.input.buttons).contains(N64Buttons::A) {
        set_attack11(f);
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
/// The order is the original's, with the unported entries (specials, grab,
/// shield, taunt, pipe) removed rather than reordered around. `Attack1`
/// (`check_attack1`) is the one attack this slice ports, and it sits exactly
/// where the original's macro puts it: after every unported attack/special
/// check, before `GuardOn`/`Appeal` (also unported) and `KneeBend`. Returns
/// whether any check took the frame.
pub fn ground_interrupt(f: &mut Fighter) -> bool {
    check_special_n(f)
        || check_special_hi(f)
        || check_special_lw(f)
        || crate::grab::check_catch_common(f)
        || check_fsmash(f)
        || check_usmash(f)
        || check_dsmash(f)
        || check_ftilt(f)
        || check_utilt(f)
        || check_dtilt(f)
        || check_attack1(f)
        || check_guard_on(f)
        || check_kneebend(f)
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
///
/// `check_attack1`/the tilt checks were missing here before the base-moveset
/// batch even though `ftCommonWalkCheckInterrupt`'s real macro has the same
/// attack prefix `ftCommonGroundCheckInterrupt` does — a real gap (attacking
/// out of a walk did nothing), not a deliberate original difference, now
/// closed alongside adding the tilts themselves.
pub fn walk_interrupt(f: &mut Fighter) -> bool {
    check_special_n(f)
        || check_special_hi(f)
        || check_special_lw(f)
        || crate::grab::check_catch_common(f)
        || check_fsmash(f)
        || check_usmash(f)
        || check_dsmash(f)
        || check_ftilt(f)
        || check_utilt(f)
        || check_dtilt(f)
        || check_attack1(f)
        || check_guard_on(f)
        || check_kneebend(f)
        || check_dash(f)
        || check_squat(f)
        || check_wait(f)
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

    // Every extended (`AnyStatus::Mario`-style) status is handled
    // separately, in `update_extended` — unwrapping to a bare `Status` here
    // means the common-table match below needs no changes at all to stay
    // exactly what it was before `AnyStatus` existed.
    if crate::grab::update(f) {
        return;
    }
    let AnyStatus::Common(current) = f.status.status else {
        update_extended(f);
        return;
    };

    match current {
        Status::KneeBend => update_kneebend(f),
        Status::Dash => update_dash(f),
        Status::Run => {
            if !(crate::grab::check_catch_dash_run(f)
                || check_attack_dash(f)
                || check_kneebend_run(f)
                || check_run_brake(f))
            {
                // Runs do not end on their own; they are held.
            }
        }
        Status::Turn => update_turn(f),
        Status::Squat => {
            if f.status.animation_ended() {
                set_status(f, Status::SquatWait, 0.0, StatusTiming::unknown());
                f.is_special_interrupt = true;
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
        // `ftCommonAttack11ProcUpdate` @ `ftcommonattack1.c:29`, reduced to
        // its `else` branch: no combo followup is ported (`crate::attack`'s
        // module docs), so the jab always ends in `ftAnimEndSetWait` rather
        // than checking for a buffered second tap.
        // `ftCommonAttack11ProcUpdate` @ `ftcommonattack1.c:31` and
        // `ftCommonAttack11ProcInterrupt` → `ftCommonAttack12CheckGoto` @
        // `ftcommonattack1.c:75,349`, collapsed into one check — see
        // `Attack1State`'s doc comment for why that is safe.
        Status::Attack11 if attack1_flag1_frame(f.kind, Status::Attack11).is_some() => {
            update_attack11_flagged(f)
        }
        Status::Attack12 if attack1_flag1_frame(f.kind, Status::Attack12).is_some() => {
            update_attack12_flagged(f)
        }
        Status::Attack11 => {
            // `ftCommonAttack11ProcInterrupt`'s `interrupt_catch_timer < 2`:
            // a Z tap in the jab's first two frames becomes a grab.
            if f.status.anim_frame <= 2.0 && crate::grab::check_catch_attack11(f) {
                return;
            }
            rapid_input(f);
            if f.attack1.followup_frames > 0.0 {
                f.attack1.followup_frames -= f.status.timing.anim_speed;
                if newly_pressed(f.prev_input.buttons, f.input.buttons).contains(N64Buttons::A) {
                    f.attack1.is_goto_followup = true;
                }
            }
            if f.status.animation_ended() {
                if f.attack1.is_goto_followup {
                    set_attack12(f);
                } else {
                    set_wait(f);
                }
            }
        }
        // `ftCommonAttack12ProcUpdate` @ `ftcommonattack1.c:47`, minus the
        // `Attack100` branch: it doesn't apply to Mario at all
        // (`ftCommonAttack100CheckFighterKind` — he isn't in it). The
        // `Attack13` chain itself now works, via [`attack13_status`] —
        // gating on which fighters have one the same way
        // `ftCommonAttack13CheckFighterKind` does, rather than assuming
        // every fighter does.
        Status::Attack12 => {
            rapid_input(f);
            if f.attack1.followup_frames > 0.0 {
                f.attack1.followup_frames -= f.status.timing.anim_speed;
                if newly_pressed(f.prev_input.buttons, f.input.buttons).contains(N64Buttons::A) {
                    f.attack1.is_goto_followup = true;
                }
            }
            if f.status.animation_ended() {
                if f.kind == crate::fighter::FighterKind::Fox && f.attack1.rapid_requested {
                    set_fox_rapid_start(f);
                } else {
                    match (f.attack1.is_goto_followup, attack13_status(f.kind)) {
                        (true, Some(status)) => set_attack13(f, status),
                        _ => set_wait(f),
                    }
                }
            }
        }
        // `ftCommonAttackDashProcUpdate`/`AttackS3ProcUpdate`/`AttackHi3ProcUpdate`
        // all reduce to `ftAnimEndSetWait` once combo-followup handling is
        // out of scope (`Attack11`'s own precedent) — none of these three
        // have a followup.
        // `ftCommonAttackS4ProcUpdate` @ `ftcommonattacks4.c:11` also reduces
        // to this for Mario — its Pikachu/Ness-specific reflector handling
        // doesn't apply. `AttackHi4`/`AttackLw4` have no `ProcUpdate`
        // override at all, so the default is the same.
        Status::AttackDash
        | Status::AttackS3Hi
        | Status::AttackS3HiS
        | Status::AttackS3
        | Status::AttackS3LwS
        | Status::AttackS3Lw
        | Status::AttackHi3
        | Status::AttackS4Hi
        | Status::AttackS4HiS
        | Status::AttackS4
        | Status::AttackS4LwS
        | Status::AttackS4Lw
        | Status::AttackHi4
        | Status::AttackLw4 => {
            if f.status.animation_ended() {
                set_wait(f);
            }
        }
        // `ftCommonAttackLw3ProcUpdate` @ `ftcommonattacklw3.c:10`, minus the
        // repeated-tap extension (`is_goto_attacklw3`) — down tilt is
        // performed from a crouch and ends back in it, not in `Wait`.
        Status::AttackLw3 => {
            if f.status.animation_ended() {
                set_status(f, Status::SquatWait, 0.0, StatusTiming::unknown());
                f.is_special_interrupt = true;
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
        // `ftCommonGuardOnProcUpdate` @ `ftcommonguard1.c:362`, minus the
        // Yoshi/effects side (module docs). No animation length is extracted
        // for the shield-raise, so — unlike a real multi-frame startup —
        // `GuardOn` resolves into `Guard` (or straight back out, or into a
        // break) on its very first update tick; see the `GuardState` docs.
        Status::GuardOn => {
            guard_check_schedule_release(f);
            guard_update_shield_vars(f);
            if f.guard.shield_health <= 0.0 {
                set_shield_break_fly(f);
            } else if f.guard.is_release {
                set_guard_off(f);
            } else {
                set_guard(f);
                // `ftCommonGuardCommonProcInterrupt`'s catch check.
                crate::grab::check_catch_guard(f);
            }
        }
        // `ftCommonGuardProcUpdate` @ `ftcommonguard1.c:472`.
        Status::Guard => {
            guard_check_schedule_release(f);
            guard_update_shield_vars(f);
            if f.guard.shield_health <= 0.0 {
                set_shield_break_fly(f);
            } else if f.guard.is_release {
                set_guard_off(f);
            } else {
                crate::grab::check_catch_guard(f);
            }
        }
        // `ftCommonGuardOffProcUpdate` @ `ftcommonguard2.c:60`, using
        // `guard_update_shield_vars`'s "fully lowered" return in place of the
        // original's unextracted animation length (`GuardState` docs).
        Status::GuardOff => {
            let fully_released = guard_update_shield_vars(f);
            if f.guard.shield_health <= 0.0 {
                set_shield_break_fly(f);
            } else if fully_released {
                set_wait(f);
            }
        }
        // `ftCommonGuardSetOffProcUpdate` @ `ftcommonguard2.c:93`.
        Status::GuardSetOff => {
            guard_check_schedule_release(f);
            f.guard.setoff_frames -= 1.0;
            if f.guard.setoff_frames <= 0.0 {
                if f.guard.is_release {
                    set_guard_off(f);
                } else {
                    set_guard(f);
                }
            }
        }
        // `ftCommonRebirthDownProcUpdate` @ `ftcommonrebirth.c:124`, minus
        // the camera-mode switch it also does partway through (cosmetic).
        Status::RebirthDown => {
            if f.status.animation_ended() {
                set_rebirth_stand(f);
            }
        }
        // `ftCommonRebirthStandProcUpdate` @ `ftcommonrebirth.c:150`: see
        // `set_rebirth_stand`'s docs for why this collapses to one tick.
        Status::RebirthStand => {
            set_rebirth_wait(f);
        }
        // `ftCommonRebirthWaitProcUpdate`/`...ProcInterrupt` @
        // `ftcommonrebirth.c:173,187`: the player can cancel the respawn
        // wait early by acting, which grants the same invincibility window
        // immediately rather than waiting out the rest of the timer.
        Status::RebirthWait => {
            if ground_interrupt(f) {
                f.invincible_frames = REBIRTH_INVINCIBLE_FRAMES;
            } else if f.status.animation_ended() {
                end_rebirth(f);
            }
        }
        // `ftCommonCliffCatchProcUpdate` @ `ftcommoncliffcatchwait.c:10`: see
        // `set_cliff_catch`'s docs for the collapse.
        Status::CliffCatch => set_cliff_wait(f),
        Status::CliffWait => update_cliff_wait(f),
        s @ (Status::CliffClimbQuick1
        | Status::CliffClimbSlow1
        | Status::CliffAttackQuick1
        | Status::CliffAttackSlow1
        | Status::CliffEscapeQuick1
        | Status::CliffEscapeSlow1) => {
            set_status(f, cliff_phase1_to_2(s), 0.0, StatusTiming::unknown());
        }
        Status::CliffClimbQuick2
        | Status::CliffClimbSlow2
        | Status::CliffAttackQuick2
        | Status::CliffAttackSlow2
        | Status::CliffEscapeQuick2
        | Status::CliffEscapeSlow2 => {
            end_cliff_recovery(f);
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
        // `ftCommonDamageCommonProcInterrupt` @ `ftcommondamage.c:166`,
        // restricted to the no-hammer case: hitstun ends the status into
        // `Wait` (module docs: the grounded Damage statuses are always
        // grounded here, so this always takes the `ga != Air` branch).
        Status::DamageHi1
        | Status::DamageHi2
        | Status::DamageHi3
        | Status::DamageN1
        | Status::DamageN2
        | Status::DamageN3
        | Status::DamageLw1
        | Status::DamageLw2
        | Status::DamageLw3 => {
            if f.hitstun == 0 {
                set_wait(f);
            }
        }
        // `ftCommonDamageAirCommonProcInterrupt` @ `ftcommondamage.c:191`:
        // an airborne hit reaction ends into `DamageFall`, not directly back
        // under player control — `crate::attack::set_damage_fall`.
        Status::DamageAir1
        | Status::DamageAir2
        | Status::DamageAir3
        | Status::DamageFlyHi
        | Status::DamageFlyN
        | Status::DamageFlyLw
        | Status::DamageFlyTop
        | Status::DamageFlyRoll => {
            if f.hitstun == 0 {
                set_damage_fall(f);
            }
        }
        // `ftAnimEndSetFall`: an aerial attack that runs out without landing
        // first drops the fighter into a plain fall — landing mid-move is
        // handled separately, in `Fighter::tick_air` via
        // `set_landing_or_landing_air`.
        Status::AttackAirLw if f.kind == crate::fighter::FighterKind::Link => {
            crate::link::update_attack_air_lw(f)
        }
        Status::AttackAirN
        | Status::AttackAirF
        | Status::AttackAirB
        | Status::AttackAirHi
        | Status::AttackAirLw => {
            if f.status.animation_ended() {
                set_fall(f);
            }
        }
        // `LandingAirF`/`Hi`/`B`/`Lw` have no extracted animation length
        // (`set_landing_air`'s docs), so they collapse to `Wait` on the tick
        // after they are entered.
        Status::LandingAirF | Status::LandingAirB | Status::LandingAirHi | Status::LandingAirLw => {
            set_wait(f);
        }
        // `LandingAirNull`'s length is real (`set_landing_air_null`'s docs).
        Status::LandingAirNull => {
            if f.status.animation_ended() {
                set_wait(f);
            }
        }
        // `ftCommonFallSpecialProcInterrupt` @ `ftcommonfallspecial.c:10`:
        // unlike a plain `Fall`, only a jump cancel is checked here — no
        // aerial attack. In practice `check_jump_aerial` never succeeds
        // either, since `set_fall_special` already spends every jump
        // (`jumps_used = jumps_max`), matching the real "helpless" recovery
        // fall. The physics (gravity/drift/fastfall) run in `Fighter::tick_air`,
        // not here — same split as every other status.
        Status::FallSpecial => {
            check_jump_aerial(f);
        }
        // `ftCommonLandingFallSpecialSetStatus`'s status has no extracted
        // animation length either (`set_landing_fall_special`'s docs).
        Status::LandingFallSpecial => {
            set_wait(f);
        }
        // `ftCommonFallProcInterrupt` @ `ftcommonfall.c:10`: attack outranks
        // a second jump.
        // `check_attack_air` needs `&mut Fighter`, which a match guard on
        // `f.status.status` can't borrow alongside — clippy's
        // `collapsible_match` suggestion for this doesn't compile.
        #[allow(clippy::collapsible_match)]
        // `ftCommonYoshiEggProcUpdate`; its interrupt only wiggles the
        // effect.
        Status::YoshiEgg => crate::capture_yoshi::update_egg(f),
        s if !s.is_grounded() => {
            // `ftCommonJumpAerialUpdateModelYaw`: only Yoshi sets a turn.
            if matches!(s, Status::JumpAerialF | Status::JumpAerialB) {
                crate::yoshi::update_jump_aerial_turn(f);
            }
            if !check_special_n(f)
                && !check_special_hi(f)
                && !check_special_lw(f)
                && !check_attack_air(f)
            {
                check_jump_aerial(f);
            }
        }
        _ => {}
    }
}

/// The extended-status counterpart of `update`'s common-table match, for
/// whichever fighter-specific status a [`Fighter`] is currently in — see
/// [`AnyStatus`].
fn update_extended(f: &mut Fighter) {
    match f.status.status {
        AnyStatus::Samus(_) => crate::samus::update(f),
        AnyStatus::Link(_) => crate::link::update(f),
        AnyStatus::Yoshi(_) => crate::yoshi::update(f),
        AnyStatus::Captain(_) => crate::captain::update(f),
        AnyStatus::Donkey(DonkeyStatus::SpecialNStart | DonkeyStatus::SpecialAirNStart) => {
            let taps = newly_pressed(f.prev_input.buttons, f.input.buttons);
            if taps.contains(N64Buttons::A) || taps.contains(N64Buttons::B) {
                f.donkey_special_n.release = true;
            }
            if f.status.animation_ended() {
                donkey_charge_loop(f);
            }
        }
        AnyStatus::Donkey(DonkeyStatus::SpecialNLoop | DonkeyStatus::SpecialAirNLoop) => {
            let taps = newly_pressed(f.prev_input.buttons, f.input.buttons);
            if taps.contains(N64Buttons::A) || taps.contains(N64Buttons::B) {
                f.donkey_special_n.release = true;
            }
            if taps.contains(N64Buttons::Z) {
                f.donkey_special_n.cancel = true;
            }
            if f.status.animation_ended() {
                if f.donkey_special_n.charging && f.donkey_special_n.charge_level < 10 {
                    f.donkey_special_n.charge_level += 1;
                    if f.donkey_special_n.charge_level == 10 {
                        f.donkey_special_n.cancel = true;
                    }
                }
                if f.donkey_special_n.cancel {
                    if f.is_grounded() {
                        set_wait(f);
                    } else {
                        set_fall(f);
                    }
                } else if f.donkey_special_n.release {
                    donkey_charge_release(f);
                } else {
                    f.donkey_special_n.charging = true;
                    donkey_charge_loop(f);
                }
            }
        }
        AnyStatus::Donkey(
            DonkeyStatus::SpecialNEnd
            | DonkeyStatus::SpecialAirNEnd
            | DonkeyStatus::SpecialNFull
            | DonkeyStatus::SpecialAirNFull,
        ) => {
            if f.status.animation_ended() {
                if f.is_grounded() {
                    set_wait(f);
                } else {
                    set_fall(f);
                }
            }
        }
        AnyStatus::Donkey(DonkeyStatus::SpecialHi | DonkeyStatus::SpecialAirHi) => {
            if f.status.animation_ended() {
                if f.is_grounded() {
                    set_wait(f);
                } else {
                    set_fall_special(f, 1.0, false, true, 0.3, true);
                }
            }
        }
        AnyStatus::Donkey(DonkeyStatus::SpecialLwStart) => {
            if f.status.animation_ended() {
                set_any_status(
                    f,
                    AnyStatus::Donkey(DonkeyStatus::SpecialLwLoop),
                    0.0,
                    StatusTiming::frames(28.0),
                );
            }
        }
        AnyStatus::Donkey(DonkeyStatus::SpecialLwLoop) => {
            if newly_pressed(f.prev_input.buttons, f.input.buttons).contains(N64Buttons::B) {
                f.donkey_special_lw.loop_requested = true;
            }
            if f.status.animation_ended() {
                if f.donkey_special_lw.loop_requested {
                    f.donkey_special_lw.loop_requested = false;
                    set_any_status(
                        f,
                        AnyStatus::Donkey(DonkeyStatus::SpecialLwLoop),
                        0.0,
                        StatusTiming::frames(28.0),
                    );
                } else {
                    set_any_status(
                        f,
                        AnyStatus::Donkey(DonkeyStatus::SpecialLwEnd),
                        0.0,
                        StatusTiming::frames(5.0),
                    );
                }
            }
        }
        AnyStatus::Donkey(DonkeyStatus::SpecialLwEnd) => {
            if f.status.animation_ended() {
                set_wait(f);
            }
        }
        // `ftCommonAttack13ProcUpdate` @ `ftcommonattack1.c:63`, minus the
        // Captain-only `Attack100` branch (doesn't apply to Mario).
        AnyStatus::Mario(MarioStatus::Attack13) => {
            if f.status.animation_ended() {
                set_wait(f);
            }
        }
        AnyStatus::Mario(MarioStatus::SpecialN | MarioStatus::SpecialAirN) => {
            // `dMarioMainMotion_0x1688`: WaitAsync(16), then SetFlag0(1).
            // The source accessory callback consumes that flag exactly once
            // and hands weapon creation to `wpManager` after fighter update.
            if f.status.anim_frame >= MARIO_FIREBALL_SPAWN_FRAME && !f.mario_special_n.spawned {
                f.mario_special_n.spawned = true;
                // `ftMarioSpecialNProcAccessory`'s `fkind` switch picks
                // the Fireball attribute row.
                let kind = if f.kind == crate::fighter::FighterKind::Luigi {
                    crate::weapon::WeaponKind::LuigiFireball
                } else {
                    crate::weapon::WeaponKind::MarioFireball
                };
                f.weapon_spawn = Some(crate::weapon::WeaponSpawn {
                    kind,
                    owner_port: f.port,
                    // `ftMarioSpecialNProcAccessory` asks the runtime for
                    // Mario joint 16's world position. Host-only gameplay
                    // stays usable without a skeleton by honestly falling
                    // back to the fighter root.
                    position: f.weapon_spawn_anchor.unwrap_or(f.pos),
                    facing: f.facing.sign(),
                });
            }
            if f.status.animation_ended() {
                if f.situation == Situation::Ground {
                    set_wait(f);
                } else {
                    set_fall(f);
                }
            }
        }
        AnyStatus::Mario(MarioStatus::SpecialHi | MarioStatus::SpecialAirHi) => {
            if f.status.animation_ended() {
                set_fall_special(
                    f,
                    MARIO_SUPERJUMP_AIR_DRIFT,
                    true,
                    true,
                    MARIO_SUPERJUMP_LANDING_LAG,
                    false,
                );
            } else {
                apply_mario_special_hi_interrupt(f);
            }
        }
        AnyStatus::Mario(MarioStatus::SpecialLw) => {
            if f.status.animation_ended() {
                set_wait(f);
            }
        }
        AnyStatus::Mario(MarioStatus::SpecialAirLw) => {
            if f.status.anim_frame >= MARIO_TORNADO_FINISH_FRAME {
                f.mario_special_lw.rise_enabled = false;
                f.mario_special_lw.rise_exhausted = true;
            }
            if f.status.animation_ended() {
                set_fall(f);
            }
        }
        AnyStatus::Fox(FoxStatus::Attack100Start) => {
            if f.status.animation_ended() {
                set_fox_rapid_loop(f);
            }
        }
        AnyStatus::Fox(FoxStatus::Attack100Loop) => {
            // `ftCommonAttack100LoopProcInterrupt` records either A edge.
            if newly_pressed(f.prev_input.buttons, f.input.buttons).contains(N64Buttons::A)
                || newly_released(f.prev_input.buttons, f.input.buttons).contains(N64Buttons::A)
            {
                f.attack1.rapid_keep_loop = true;
            }
            if f.status.animation_ended() {
                if f.attack1.rapid_keep_loop {
                    set_fox_rapid_loop(f);
                } else {
                    set_fox_rapid_end(f);
                }
            }
        }
        AnyStatus::Fox(FoxStatus::Attack100End) => {
            if f.status.animation_ended() {
                set_wait(f);
            }
        }
        AnyStatus::Fox(FoxStatus::SpecialN | FoxStatus::SpecialAirN) => {
            let spawn_frame = if f.status.status == AnyStatus::Fox(FoxStatus::SpecialN) {
                25.0
            } else {
                15.0
            };
            if f.status.anim_frame >= spawn_frame && !f.fox_special_n.spawned {
                f.fox_special_n.spawned = true;
                f.weapon_spawn = Some(crate::weapon::WeaponSpawn {
                    kind: crate::weapon::WeaponKind::FoxBlaster,
                    owner_port: f.port,
                    position: f.weapon_spawn_anchor.unwrap_or(ssb_engine::math::Vec3::new(
                        f.pos.x + 60.0 * f.facing.sign(),
                        f.pos.y,
                        f.pos.z,
                    )),
                    facing: f.facing.sign(),
                });
            }
            let repeat_frame = if f.status.status == AnyStatus::Fox(FoxStatus::SpecialN) {
                29.0
            } else {
                15.0
            };
            if f.status.anim_frame >= repeat_frame
                && newly_pressed(f.prev_input.buttons, f.input.buttons).contains(N64Buttons::B)
            {
                set_fox_special_n(f);
            } else if f.status.animation_ended() {
                if f.situation == Situation::Ground {
                    set_wait(f);
                } else {
                    set_fall(f);
                }
            }
        }
        AnyStatus::Fox(FoxStatus::SpecialHiStart | FoxStatus::SpecialAirHiStart) => {
            if f.status.animation_ended() {
                set_fox_special_hi_hold(f);
            }
        }
        AnyStatus::Fox(FoxStatus::SpecialHiHold | FoxStatus::SpecialAirHiHold) => {
            if f.fox_special_hi.launch_delay > 0 {
                f.fox_special_hi.launch_delay -= 1;
            }
            if f.fox_special_hi.launch_delay == 0 {
                set_fox_special_hi_travel(f);
            }
        }
        AnyStatus::Fox(FoxStatus::SpecialHi | FoxStatus::SpecialAirHi) => {
            if f.fox_special_hi.travel_frames > 0 {
                f.fox_special_hi.travel_frames -= 1;
            }
            if f.fox_special_hi.travel_frames == 0 {
                set_fox_special_hi_end(f);
            }
        }
        AnyStatus::Fox(FoxStatus::SpecialHiEnd) => {
            if f.status.animation_ended() {
                set_wait(f);
            }
        }
        AnyStatus::Fox(FoxStatus::SpecialAirHiEnd | FoxStatus::SpecialAirHiBound) => {
            if f.status.animation_ended() {
                set_fall_special(f, 1.0, false, true, 0.34, true);
            }
        }
        AnyStatus::Fox(FoxStatus::SpecialLwStart | FoxStatus::SpecialAirLwStart) => {
            if !f.input.buttons.contains(N64Buttons::B) {
                f.fox_special_lw.released = true;
            }
            if f.status.animation_ended() {
                set_fox_special_lw_phase(f, 0);
            }
        }
        AnyStatus::Fox(FoxStatus::SpecialLwLoop | FoxStatus::SpecialAirLwLoop) => {
            if !f.input.buttons.contains(N64Buttons::B) {
                f.fox_special_lw.released = true;
            }
            f.fox_special_lw.release_lag = f.fox_special_lw.release_lag.saturating_sub(1);
            if f.fox_special_lw.released && f.fox_special_lw.release_lag == 0 {
                set_fox_special_lw_phase(f, 2);
            } else if f.stick.forward(f.facing) <= TURN_STICK_MIN {
                set_fox_special_lw_phase(f, 1);
            }
        }
        AnyStatus::Fox(FoxStatus::SpecialLwTurn | FoxStatus::SpecialAirLwTurn) => {
            if !f.input.buttons.contains(N64Buttons::B) {
                f.fox_special_lw.released = true;
            }
            f.fox_special_lw.release_lag = f.fox_special_lw.release_lag.saturating_sub(1);
            f.fox_special_lw.turn_frames = f.fox_special_lw.turn_frames.saturating_sub(1);
            if !f.fox_special_lw.turned {
                f.facing = f.facing.flipped();
                f.fox_special_lw.turned = true;
            }
            if f.fox_special_lw.turn_frames == 0 {
                if f.fox_special_lw.released && f.fox_special_lw.release_lag == 0 {
                    set_fox_special_lw_phase(f, 2);
                } else {
                    set_fox_special_lw_phase(f, 0);
                }
            }
        }
        AnyStatus::Fox(FoxStatus::SpecialLwHit | FoxStatus::SpecialAirLwHit) => {
            if !f.input.buttons.contains(N64Buttons::B) {
                f.fox_special_lw.released = true;
            }
            f.fox_special_lw.release_lag = f.fox_special_lw.release_lag.saturating_sub(1);
            if f.status.animation_ended() {
                set_fox_special_lw_phase(
                    f,
                    if f.fox_special_lw.released && f.fox_special_lw.release_lag == 0 {
                        2
                    } else {
                        0
                    },
                );
            }
        }
        AnyStatus::Fox(FoxStatus::SpecialLwEnd | FoxStatus::SpecialAirLwEnd) => {
            if f.status.animation_ended() {
                if f.situation == Situation::Ground {
                    set_wait(f);
                } else {
                    set_fall(f);
                }
            }
        }
        AnyStatus::Common(_) => unreachable!("update dispatches Common statuses itself"),
        // Cargo statuses are run by `crate::grab::update` before this.
        AnyStatus::Donkey(_) => {}
    }
}

/// `ftCommonAttack13CheckFighterKind` @ `ftcommonattack1.c:9` — which
/// fighters have a jab-combo finisher at all, and which extended status it
/// is. `None` for a fighter with no `Attack13` (most of them; the original
/// falls back to `Attack100`'s rapid-jab loop instead for a different,
/// smaller set).
pub fn attack13_status(kind: crate::fighter::FighterKind) -> Option<AnyStatus> {
    match kind {
        crate::fighter::FighterKind::Mario | crate::fighter::FighterKind::Luigi => {
            Some(AnyStatus::Mario(MarioStatus::Attack13))
        }
        crate::fighter::FighterKind::Link => Some(AnyStatus::Link(LinkStatus::Attack13)),
        crate::fighter::FighterKind::Captain => Some(AnyStatus::Captain(CaptainStatus::Attack13)),
        _ => None,
    }
}

/// `ftCommonAttack13SetStatus` @ `ftcommonattack1.c:199`, reduced to Mario's
/// own branch (every fighter with an `Attack13` maps to a different
/// per-character status in the original; [`attack13_status`] is this
/// codebase's equivalent of that `switch`).
pub fn set_attack13(f: &mut Fighter, status: AnyStatus) {
    let len = crate::attack::move_data(f.kind, status)
        .map(|m| m.length_frames)
        .unwrap_or(0.0);
    set_any_status(f, status, 0.0, StatusTiming::frames(len));
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
    // Every `ftCommonDashProcInterrupt` branch checks the catch input
    // (`...Common` or `...DashRun`, which are the same without items).
    if crate::grab::check_catch_dash_run(f) {
        return;
    }
    // `ftCommonDashProcInterrupt`'s `anim_frame <= 20.0` window.
    if f.status.anim_frame <= 20.0 && check_attack_dash(f) {
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
    // Only ever called while already in one of the (common-table) walk
    // statuses, so this always matches.
    let AnyStatus::Common(current) = f.status.status else {
        return;
    };
    let want = walk_status_for(f.input.stick_x);
    if want != current {
        let old = walk_anim_length(&f.attributes, current);
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
        f.status.status = Status::Wait.into();
        f
    }

    fn airborne_mario() -> Fighter {
        let mut f = Fighter::new(FighterKind::Mario, 0, 3);
        f.situation = Situation::Air;
        f.status.status = Status::Fall.into();
        f
    }

    #[test]
    fn holding_down_while_falling_triggers_fast_fall_once() {
        let mut f = airborne_mario();
        f.physics.vel_air.y = -1.0; // already falling
        hold(&mut f, 0, -80);
        check_set_fast_fall(&mut f);
        assert!(f.physics.is_fastfall);

        // A one-shot latch: it does not re-trigger or reset itself just by
        // holding the stick again.
        f.physics.is_fastfall = false;
        check_set_fast_fall(&mut f);
        // tap_y was pinned to STICKBUFFER_MAX on the first trigger, so the
        // buffer-window gate now fails even though the stick is still down.
        assert!(!f.physics.is_fastfall);
    }

    #[test]
    fn fast_fall_does_not_trigger_while_rising() {
        let mut f = airborne_mario();
        f.physics.vel_air.y = 5.0; // still going up
        hold(&mut f, 0, -80);
        check_set_fast_fall(&mut f);
        assert!(!f.physics.is_fastfall);
    }

    #[test]
    fn fast_fall_does_not_trigger_on_a_shallow_push() {
        let mut f = airborne_mario();
        f.physics.vel_air.y = -1.0;
        hold(&mut f, 0, -30); // short of FASTFALL_STICK_RANGE_MIN (-53)
        check_set_fast_fall(&mut f);
        assert!(!f.physics.is_fastfall);
    }

    /// Holds the stick at `(x, y)` for one frame.
    fn hold(f: &mut Fighter, x: i8, y: i8) {
        f.input.stick_x = x;
        f.input.stick_y = y;
        f.stick.step(x, y, false, false);
    }

    /// Holds the stick at `(x, y)` long enough for `tap_x`/`tap_y` to age
    /// past a smash's tap window — a slow push rather than a fresh flick, so
    /// a tilt check fires instead of a smash one for the same deflection.
    fn hold_stale(f: &mut Fighter, x: i8, y: i8) {
        for _ in 0..5 {
            hold(f, x, y);
        }
    }

    #[test]
    fn status_ordinals_match_the_original_enum() {
        assert_eq!(Status::Wait as u16, 10);
        assert_eq!(Status::Dash as u16, 15);
        assert_eq!(Status::KneeBend as u16, 20);
        assert_eq!(Status::Fall as u16, 26);
        assert_eq!(Status::Pass as u16, 33);
        assert_eq!(Status::Attack11 as u16, 190);

        // Spot-check the full common table added for the Damage/Guard/Cliff/
        // Catch/base-moveset batch, one per section, against
        // `ftcommonstatus.h`'s own numbering comments.
        assert_eq!(Status::DeadDown as u16, 0);
        assert_eq!(Status::RebirthWait as u16, 9);
        assert_eq!(Status::DamageHi1 as u16, 37);
        assert_eq!(Status::DamageFlyRoll as u16, 55);
        assert_eq!(Status::DamageFall as u16, 57);
        assert_eq!(Status::CliffCatch as u16, 84);
        assert_eq!(Status::HeavyThrowB4 as u16, 125);
        assert_eq!(Status::GuardOn as u16, 152);
        assert_eq!(Status::Catch as u16, 166);
        assert_eq!(Status::Appeal as u16, 189);
        assert_eq!(Status::LandingAirNull as u16, 219);
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
        assert_eq!(f.physics.vel_ground.x, -f.attributes.dash_speed);
    }

    #[test]
    fn run_velocity_follows_facing() {
        let mut f = mario();
        f.facing = Facing::Left;
        set_run(&mut f);
        assert_eq!(f.physics.vel_ground.x, -f.attributes.run_speed);
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
    fn ground_jump_consumes_one_of_marios_two_jumps() {
        let mut f = mario();
        hold(&mut f, 0, 80);
        assert!(check_kneebend(&mut f));
        update(&mut f);
        update(&mut f);
        update(&mut f);

        assert_eq!(f.status.status, Status::JumpF);
        assert_eq!(f.physics.jumps_used, 1);

        hold(&mut f, 0, 0);
        hold(&mut f, 0, 80);
        assert!(check_jump_aerial(&mut f));
        assert_eq!(f.physics.jumps_used, 2);
        assert!(!check_jump_aerial(&mut f));
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

    /// Taps the N64 A button for one frame.
    fn tap_a(f: &mut Fighter) {
        f.prev_input = f.input;
        f.input.buttons = N64Buttons(N64Buttons::A);
    }

    #[test]
    fn tapping_a_from_wait_enters_the_jab() {
        let mut f = mario();
        assert!(!check_attack1(&mut f));
        assert_eq!(f.status.status, Status::Wait); // no tap fed in yet
        tap_a(&mut f);
        assert!(check_attack1(&mut f));
        assert_eq!(f.status.status, Status::Attack11);
    }

    #[test]
    fn fox_jab_uses_fox_script_length_and_five_angle_tilt() {
        let mut f = Fighter::new(FighterKind::Fox, 0, 3);
        set_attack11(&mut f);
        assert_eq!(f.status.timing.anim_length, Some(10.0));
        f.stick.x = 80;
        f.stick.y = 20;
        set_ftilt(&mut f);
        assert_eq!(f.status.status, Status::AttackS3HiS);
        assert_eq!(f.status.timing.anim_length, Some(14.0));
        set_fsmash(&mut f);
        assert_eq!(f.status.status, Status::AttackS4);
    }

    #[test]
    fn fire_fox_floor_contact_redirects_shallow_and_bounds_steep() {
        let normal = Vec2::new(0.0, 1.0);
        let mut fox = Fighter::new(FighterKind::Fox, 0, 3);
        fox.situation = Situation::Air;
        fox.status.status = AnyStatus::Fox(FoxStatus::SpecialAirHi);
        fox.physics.vel_air.x = 100.0;
        fox.physics.vel_air.y = -10.0;
        assert!(fox_fire_fox_floor_contact(&mut fox, normal, 0.0));
        assert_eq!(fox.status.status, AnyStatus::Fox(FoxStatus::SpecialAirHi));
        assert!(fox.physics.vel_air.x > 100.0);
        assert_eq!(fox.physics.vel_air.y, 0.0);
        assert_eq!(fox.situation, Situation::Air);

        fox.physics.vel_air.x = 10.0;
        fox.physics.vel_air.y = -100.0;
        assert!(fox_fire_fox_floor_contact(&mut fox, normal, 0.0));
        assert_eq!(
            fox.status.status,
            AnyStatus::Fox(FoxStatus::SpecialAirHiBound)
        );
        assert_eq!(fox.situation, Situation::Air);
    }

    #[test]
    fn fox_rapid_jab_counts_press_and_release_then_exits_after_idle_cycle() {
        let mut f = Fighter::new(FighterKind::Fox, 0, 3);
        set_attack11(&mut f);
        for held in [true, false, true, false] {
            f.prev_input = f.input;
            f.input.buttons.set(N64Buttons::A, held);
            rapid_input(&mut f);
        }
        assert_eq!(f.attack1.rapid_input_count, 4);
        assert!(f.attack1.rapid_requested);
        set_attack12(&mut f);
        assert_eq!(f.attack1.rapid_input_count, 4);
        f.prev_input = f.input;
        f.status.anim_frame = 10.0;
        update(&mut f);
        assert_eq!(f.status.status, AnyStatus::Fox(FoxStatus::Attack100Start));
        f.status.anim_frame = 8.0;
        update(&mut f);
        assert_eq!(f.status.status, AnyStatus::Fox(FoxStatus::Attack100Loop));
        f.status.anim_frame = 32.0;
        update(&mut f);
        assert_eq!(f.status.status, AnyStatus::Fox(FoxStatus::Attack100End));
        f.status.anim_frame = 8.0;
        update(&mut f);
        assert_eq!(f.status.status, Status::Wait);
    }

    fn link_on_ground() -> Fighter {
        let mut f = Fighter::new(FighterKind::Link, 0, 3);
        f.situation = Situation::Ground;
        f
    }

    fn release_a(f: &mut Fighter) {
        f.prev_input = f.input;
        f.input.buttons = N64Buttons(0);
    }

    #[test]
    fn link_jab_chains_on_its_mid_animation_flag1() {
        let mut f = link_on_ground();
        set_attack11(&mut f);
        release_a(&mut f);
        update(&mut f);
        tap_a(&mut f);
        update(&mut f);
        assert!(f.attack1.is_goto_followup);
        // Jab1's figatree runs 24 frames, but flag 1 lands at frame 10.
        for frame in 3..10 {
            release_a(&mut f);
            update(&mut f);
            assert_eq!(f.status.status, Status::Attack11, "frame {frame}");
        }
        release_a(&mut f);
        update(&mut f);
        assert_eq!(f.status.status, Status::Attack12);

        // A tap after flag 1 chains on that frame.
        for _ in 0..14 {
            release_a(&mut f);
            update(&mut f);
        }
        assert_eq!(f.status.status, Status::Attack12);
        tap_a(&mut f);
        update(&mut f);
        assert_eq!(f.status.status, AnyStatus::Link(LinkStatus::Attack13));
        while f.status.status != Status::Wait {
            release_a(&mut f);
            update(&mut f);
        }
    }

    #[test]
    fn link_needs_five_a_edges_for_the_rapid_jab() {
        let mut f = link_on_ground();
        set_attack11(&mut f);
        let mut edges = 0;
        while f.status.status == Status::Attack11 {
            if edges % 2 == 0 {
                tap_a(&mut f);
            } else {
                release_a(&mut f);
            }
            edges += 1;
            update(&mut f);
        }
        assert_eq!(f.status.status, Status::Attack12);
        assert!(f.attack1.rapid_input_count >= 5);
        while f.status.status == Status::Attack12 {
            release_a(&mut f);
            update(&mut f);
        }
        assert_eq!(f.status.status, AnyStatus::Link(LinkStatus::Attack100Start));
        assert_eq!(f.status.anim_frame, 0.0);

        // Four edges are not enough.
        let mut f = link_on_ground();
        set_attack11(&mut f);
        for held in [true, false, true, false] {
            f.prev_input = f.input;
            f.input.buttons.set(N64Buttons::A, held);
            rapid_input(&mut f);
        }
        assert!(!f.attack1.rapid_requested);
        assert_eq!(attack11_followup_frames(FighterKind::Link), 24.0);
        assert_eq!(attack11_followup_frames(FighterKind::Fox), 30.0);
        assert_eq!(attack11_followup_frames(FighterKind::Donkey), 28.0);
    }

    #[test]
    fn link_lands_out_of_aerials_by_his_motion_table() {
        for (air, landing) in [
            (Status::AttackAirF, Status::LandingAirF),
            (Status::AttackAirLw, Status::LandingAirLw),
            (Status::AttackAirB, Status::LandingAirNull),
            (Status::AttackAirHi, Status::LandingAirNull),
            (Status::AttackAirN, Status::LandingAirNull),
        ] {
            let mut f = Fighter::new(FighterKind::Link, 0, 3);
            f.anim.landing = 10.0;
            set_air_attack(&mut f, air);
            set_landing_or_landing_air(&mut f);
            assert_eq!(f.status.status, landing, "{air:?}");
        }
    }

    #[test]
    fn fox_fire_fox_charges_then_launches_and_enters_freefall() {
        let mut f = Fighter::new(FighterKind::Fox, 0, 3);
        f.situation = Situation::Air;
        f.stick.x = 0;
        f.stick.y = 80;
        set_fox_special_hi_start(&mut f);
        assert_eq!(
            f.status.status,
            AnyStatus::Fox(FoxStatus::SpecialAirHiStart)
        );
        assert_eq!(f.fox_special_hi.gravity_delay, 15);
        for _ in 0..8 {
            update(&mut f);
        }
        assert_eq!(f.status.status, AnyStatus::Fox(FoxStatus::SpecialAirHiHold));
        for _ in 0..34 {
            update(&mut f);
        }
        assert_eq!(f.status.status, AnyStatus::Fox(FoxStatus::SpecialAirHiHold));
        update(&mut f);
        assert_eq!(f.status.status, AnyStatus::Fox(FoxStatus::SpecialAirHi));
        assert!((f.physics.vel_air.y - 115.0).abs() < 0.01);
        for _ in 0..30 {
            update(&mut f);
        }
        assert_eq!(f.status.status, AnyStatus::Fox(FoxStatus::SpecialAirHiEnd));
        for _ in 0..20 {
            update(&mut f);
        }
        assert_eq!(f.status.status, AnyStatus::Common(Status::FallSpecial));
        assert_eq!(f.fall_special.landing_lag, 0.34);
    }

    #[test]
    fn fox_reflector_holds_until_release_lag_then_ends() {
        let mut f = Fighter::new(FighterKind::Fox, 0, 3);
        f.situation = Situation::Ground;
        f.input.buttons = N64Buttons(N64Buttons::B);
        set_fox_special_lw_start(&mut f);
        for _ in 0..4 {
            update(&mut f);
        }
        assert_eq!(f.status.status, AnyStatus::Fox(FoxStatus::SpecialLwLoop));
        f.input.buttons = N64Buttons::default();
        for _ in 0..17 {
            update(&mut f);
        }
        assert_eq!(f.status.status, AnyStatus::Fox(FoxStatus::SpecialLwLoop));
        update(&mut f);
        assert_eq!(f.status.status, AnyStatus::Fox(FoxStatus::SpecialLwEnd));
    }

    #[test]
    fn fox_neutral_b_spawns_once_and_tap_restarts_after_gate() {
        let mut f = Fighter::new(FighterKind::Fox, 0, 3);
        f.situation = Situation::Ground;
        set_fox_special_n(&mut f);
        f.status.anim_frame = 23.0;
        update(&mut f);
        assert!(f.weapon_spawn.is_none());
        f.status.anim_frame = 24.0;
        update(&mut f);
        let shot = f.weapon_spawn.take().unwrap();
        assert_eq!(shot.kind, crate::weapon::WeaponKind::FoxBlaster);
        assert_eq!(shot.position.x, 60.0);
        f.status.anim_frame = 27.0;
        f.input.buttons = N64Buttons(N64Buttons::B);
        update(&mut f);
        assert_eq!(f.status.anim_frame, 28.0);
        f.prev_input = f.input;
        f.input.buttons = N64Buttons::default();
        f.status.anim_frame = 28.0;
        update(&mut f);
        f.prev_input = f.input;
        f.input.buttons = N64Buttons(N64Buttons::B);
        update(&mut f);
        assert_eq!(f.status.status, AnyStatus::Fox(FoxStatus::SpecialN));
        assert_eq!(f.status.anim_frame, 0.0);
        assert!(!f.fox_special_n.spawned);
    }

    #[test]
    fn the_jab_is_not_interruptible_through_the_ground_chain() {
        // Attack1's own interrupt handling (item-throw branches, the Attack100
        // rapid-jab check) is not ported (`crate::attack`'s module docs); what
        // matters here is that the ordinary per-frame dispatch — which is what
        // actually gates `ground_interrupt` — cannot cut a jab short with a
        // dash input, because `Attack11` is not `is_actionable_on_ground`.
        let mut f = mario();
        tap_a(&mut f);
        update(&mut f); // Wait's dispatch runs the ground chain, entering the jab
        assert_eq!(f.status.status, Status::Attack11);
        hold(&mut f, 80, 0); // a full-stick dash input, mid-jab
        update(&mut f);
        assert_eq!(f.status.status, Status::Attack11);
    }

    #[test]
    fn the_jab_returns_to_wait_when_its_animation_ends() {
        let mut f = mario();
        set_attack11(&mut f);
        for _ in 0..(crate::attack::MARIO_ATTACK11_LENGTH_FRAMES as i32 - 1) {
            update(&mut f);
            assert_eq!(f.status.status, Status::Attack11);
        }
        update(&mut f);
        assert_eq!(f.status.status, Status::Wait);
    }

    #[test]
    fn a_repeated_tap_mid_jab_chains_into_attack12() {
        let mut f = mario();
        set_attack11(&mut f);
        update(&mut f); // frame 1, well before Jab1's own animation end
        tap_a(&mut f);
        update(&mut f); // buffers is_goto_followup; Jab1 hasn't ended yet
        assert_eq!(f.status.status, Status::Attack11);
        assert!(f.attack1.is_goto_followup);

        for _ in 0..(crate::attack::MARIO_ATTACK11_LENGTH_FRAMES as i32) {
            if f.status.status != Status::Attack11 {
                break;
            }
            update(&mut f);
        }
        assert_eq!(f.status.status, Status::Attack12);
    }

    #[test]
    fn no_tap_during_jab1_still_ends_in_wait() {
        let mut f = mario();
        set_attack11(&mut f);
        for _ in 0..30 {
            if f.status.status == Status::Wait {
                break;
            }
            update(&mut f);
        }
        assert_eq!(f.status.status, Status::Wait);
    }

    #[test]
    fn no_tap_during_jab2_ends_in_wait() {
        let mut f = mario();
        set_attack12(&mut f);
        for _ in 0..30 {
            if f.status.status == Status::Wait {
                break;
            }
            update(&mut f);
        }
        assert_eq!(f.status.status, Status::Wait);
    }

    #[test]
    fn a_repeated_tap_mid_jab2_chains_into_the_attack13_finisher() {
        let mut f = mario();
        set_attack12(&mut f);
        update(&mut f); // frame 1, well before Jab2's own animation end
        tap_a(&mut f);
        update(&mut f); // buffers is_goto_followup; Jab2 hasn't ended yet
        assert_eq!(f.status.status, Status::Attack12);
        assert!(f.attack1.is_goto_followup);

        for _ in 0..20 {
            if f.status.status != Status::Attack12 {
                break;
            }
            update(&mut f);
        }
        assert_eq!(f.status.status, AnyStatus::Mario(MarioStatus::Attack13));

        // And it plays out and ends back in Wait on its own, same as any
        // other attack.
        for _ in 0..20 {
            if f.status.status == Status::Wait {
                break;
            }
            update(&mut f);
        }
        assert_eq!(f.status.status, Status::Wait);
    }

    #[test]
    fn neutral_forward_tap_does_a_mid_forward_tilt() {
        let mut f = mario();
        hold_stale(&mut f, 80, 0);
        tap_a(&mut f);
        update(&mut f);
        assert_eq!(f.status.status, Status::AttackS3);
    }

    #[test]
    fn forward_and_up_tap_does_a_high_forward_tilt() {
        let mut f = mario();
        hold_stale(&mut f, 80, 40);
        tap_a(&mut f);
        update(&mut f);
        assert_eq!(f.status.status, Status::AttackS3Hi);
    }

    #[test]
    fn forward_and_down_tap_does_a_low_forward_tilt() {
        let mut f = mario();
        hold_stale(&mut f, 80, -40);
        tap_a(&mut f);
        update(&mut f);
        assert_eq!(f.status.status, Status::AttackS3Lw);
    }

    #[test]
    fn a_facing_left_fighter_needs_the_stick_pointed_left_for_a_forward_tilt() {
        let mut f = mario();
        f.facing = Facing::Left;
        hold_stale(&mut f, 80, 0); // pointed right = away from facing
        tap_a(&mut f);
        update(&mut f);
        assert_ne!(f.status.status, Status::AttackS3);

        let mut f = mario();
        f.facing = Facing::Left;
        hold_stale(&mut f, -80, 0);
        tap_a(&mut f);
        update(&mut f);
        assert_eq!(f.status.status, Status::AttackS3);
    }

    #[test]
    fn straight_up_tap_does_an_up_tilt() {
        let mut f = mario();
        hold_stale(&mut f, 0, 80);
        tap_a(&mut f);
        update(&mut f);
        assert_eq!(f.status.status, Status::AttackHi3);
    }

    #[test]
    fn straight_down_tap_does_a_down_tilt_and_ends_in_squat_wait() {
        let mut f = mario();
        hold_stale(&mut f, 0, -80);
        tap_a(&mut f);
        update(&mut f);
        assert_eq!(f.status.status, Status::AttackLw3);

        let len = crate::attack::move_data(f.kind, Status::AttackLw3.into())
            .unwrap()
            .length_frames;
        for _ in 0..(len as i32 - 1) {
            update(&mut f);
            assert_eq!(f.status.status, Status::AttackLw3);
        }
        update(&mut f);
        assert_eq!(f.status.status, Status::SquatWait);
    }

    #[test]
    fn a_fresh_flick_forward_smashes_rather_than_tilts() {
        // Same magnitude as the tilt tests, but a fresh crossing (tap_x ==
        // 1) instead of a held one — this is the real distinction between a
        // tilt and a smash, `check_fsmash`'s own doc comment.
        let mut f = mario();
        hold(&mut f, 80, 0);
        tap_a(&mut f);
        update(&mut f);
        assert_eq!(f.status.status, Status::AttackS4);
    }

    #[test]
    fn a_forward_smash_picks_one_of_five_angles() {
        let mut f = mario();
        hold(&mut f, 80, 0);
        tap_a(&mut f);
        update(&mut f);
        assert_eq!(f.status.status, Status::AttackS4);

        let mut f = mario();
        hold(&mut f, 80, 40); // steep enough for the full Hi, not just HiS
        tap_a(&mut f);
        update(&mut f);
        assert_eq!(f.status.status, Status::AttackS4Hi);

        let mut f = mario();
        hold(&mut f, 80, 12); // shallow: the mid-high "S" variant
        tap_a(&mut f);
        update(&mut f);
        assert_eq!(f.status.status, Status::AttackS4HiS);

        let mut f = mario();
        hold(&mut f, 80, -40);
        tap_a(&mut f);
        update(&mut f);
        assert_eq!(f.status.status, Status::AttackS4Lw);

        let mut f = mario();
        hold(&mut f, 80, -12);
        tap_a(&mut f);
        update(&mut f);
        assert_eq!(f.status.status, Status::AttackS4LwS);
    }

    #[test]
    fn straight_up_flick_does_an_up_smash() {
        let mut f = mario();
        hold(&mut f, 0, 80);
        tap_a(&mut f);
        update(&mut f);
        assert_eq!(f.status.status, Status::AttackHi4);
        assert!(!f.status.animation_ended());
        for _ in 0..30 {
            if f.status.status == Status::Wait {
                break;
            }
            update(&mut f);
        }
        assert_eq!(f.status.status, Status::Wait);
    }

    #[test]
    fn straight_down_flick_does_a_down_smash() {
        let mut f = mario();
        hold(&mut f, 0, -80);
        tap_a(&mut f);
        update(&mut f);
        assert_eq!(f.status.status, Status::AttackLw4);
    }

    #[test]
    fn a_neutral_a_tap_still_jabs_instead_of_tilting() {
        let mut f = mario();
        tap_a(&mut f);
        update(&mut f);
        assert_eq!(f.status.status, Status::Attack11);
    }

    #[test]
    fn tapping_attack_while_dashing_does_a_dash_attack() {
        let mut f = mario();
        set_status(&mut f, Status::Dash, 0.0, StatusTiming::unknown());
        tap_a(&mut f);
        update(&mut f);
        assert_eq!(f.status.status, Status::AttackDash);
    }

    #[test]
    fn tapping_attack_while_running_does_a_dash_attack() {
        let mut f = mario();
        set_status(&mut f, Status::Run, 0.0, StatusTiming::unknown());
        tap_a(&mut f);
        update(&mut f);
        assert_eq!(f.status.status, Status::AttackDash);
    }

    #[test]
    fn a_neutral_tap_in_the_air_does_a_neutral_aerial() {
        let mut f = airborne_mario();
        tap_a(&mut f);
        update(&mut f);
        assert_eq!(f.status.status, Status::AttackAirN);
    }

    #[test]
    fn a_forward_tap_in_the_air_does_a_forward_aerial() {
        let mut f = airborne_mario();
        hold(&mut f, 80, 0);
        tap_a(&mut f);
        update(&mut f);
        assert_eq!(f.status.status, Status::AttackAirF);
    }

    #[test]
    fn a_back_tap_in_the_air_does_a_back_aerial() {
        let mut f = airborne_mario();
        f.facing = Facing::Right;
        hold(&mut f, -80, 0);
        tap_a(&mut f);
        update(&mut f);
        assert_eq!(f.status.status, Status::AttackAirB);
    }

    #[test]
    fn an_up_tap_in_the_air_does_an_up_aerial() {
        let mut f = airborne_mario();
        hold(&mut f, 0, 80);
        tap_a(&mut f);
        update(&mut f);
        assert_eq!(f.status.status, Status::AttackAirHi);
    }

    #[test]
    fn a_down_tap_in_the_air_does_a_down_aerial() {
        let mut f = airborne_mario();
        hold(&mut f, 0, -80);
        tap_a(&mut f);
        update(&mut f);
        assert_eq!(f.status.status, Status::AttackAirLw);
    }

    #[test]
    fn attacking_outranks_a_second_jump_in_the_air() {
        let mut f = airborne_mario();
        f.stick.jump_tapped = true; // would otherwise double-jump
        tap_a(&mut f);
        update(&mut f);
        assert_eq!(f.status.status, Status::AttackAirN);
    }

    #[test]
    fn an_aerial_that_runs_out_without_landing_falls() {
        let mut f = airborne_mario();
        set_air_attack(&mut f, Status::AttackAirF);
        let len = crate::attack::move_data(f.kind, Status::AttackAirF.into())
            .unwrap()
            .length_frames;
        for _ in 0..(len as i32 - 1) {
            update(&mut f);
            assert_eq!(f.status.status, Status::AttackAirF);
        }
        update(&mut f);
        assert_eq!(f.status.status, Status::Fall);
    }

    #[test]
    fn set_fall_special_spends_every_jump_and_clamps_drift() {
        let mut f = airborne_mario();
        f.physics.jumps_used = 0;
        f.physics.vel_air.x = 999.0;
        set_fall_special(&mut f, 0.5, false, false, 0.0, true);

        assert_eq!(f.status.status, Status::FallSpecial);
        assert_eq!(f.physics.jumps_used, f.attributes.jumps_max);
        assert_eq!(f.fall_special.drift, f.attributes.air_speed_max_x * 0.5);
        assert_eq!(f.physics.vel_air.x, f.fall_special.drift);
    }

    #[test]
    fn fall_special_cannot_jump_cancel_since_its_jumps_are_already_spent() {
        let mut f = airborne_mario();
        set_fall_special(&mut f, 1.0, false, false, 0.0, true);
        hold(&mut f, 0, 80); // a stick-jump input
        update(&mut f);
        assert_eq!(f.status.status, Status::FallSpecial);
    }

    #[test]
    fn fall_special_landing_goes_to_wait_when_not_falling_fast() {
        let mut f = airborne_mario();
        set_fall_special(&mut f, 1.0, false, false, 0.0, true);
        f.physics.vel_air.y = -5.0; // above FALLSPECIAL_SKIPLANDING_VEL_Y_MAX
        set_landing_or_landing_air(&mut f);
        assert_eq!(f.status.status, Status::Wait);
    }

    #[test]
    fn fall_special_landing_takes_the_real_status_when_falling_fast() {
        let mut f = airborne_mario();
        set_fall_special(&mut f, 1.0, false, false, 0.0, true);
        f.physics.vel_air.y = -50.0; // past FALLSPECIAL_SKIPLANDING_VEL_Y_MAX
        set_landing_or_landing_air(&mut f);
        assert_eq!(f.status.status, Status::LandingFallSpecial);
        update(&mut f);
        assert_eq!(f.status.status, Status::Wait);
    }

    #[test]
    fn fall_special_landing_is_forced_when_is_goto_landing_is_set() {
        let mut f = airborne_mario();
        set_fall_special(&mut f, 1.0, false, true, 0.0, true);
        f.physics.vel_air.y = 0.0; // would otherwise skip landing
        set_landing_or_landing_air(&mut f);
        assert_eq!(f.status.status, Status::LandingFallSpecial);
    }

    #[test]
    fn landing_mid_aerial_with_a_dedicated_clip_takes_landing_air_then_wait() {
        let mut f = airborne_mario();
        set_air_attack(&mut f, Status::AttackAirF);
        set_landing_or_landing_air(&mut f);
        assert_eq!(f.status.status, Status::LandingAirF);
        update(&mut f);
        assert_eq!(f.status.status, Status::Wait);
    }

    #[test]
    fn landing_mid_neutral_aerial_scales_the_real_landing_lag_by_percent() {
        let mut f = airborne_mario();
        set_air_attack(&mut f, Status::AttackAirN);
        set_landing_or_landing_air(&mut f);
        assert_eq!(f.status.status, Status::LandingAirNull);
        let expected = f.anim.landing * 0.5; // AttackAirN's landing_lag_percent
        assert_eq!(f.status.timing.anim_length, Some(expected));
        for _ in 0..10 {
            if f.status.status == Status::Wait {
                break;
            }
            update(&mut f);
        }
        assert_eq!(f.status.status, Status::Wait);
    }

    fn hold_z(f: &mut Fighter, held: bool) {
        f.input.buttons.set(ssb_engine::input::N64Buttons::Z, held);
    }

    #[test]
    fn holding_z_from_wait_shields_then_settles_into_guard() {
        let mut f = mario();
        hold_z(&mut f, true);
        update(&mut f); // Wait's ground chain sees Z held -> GuardOn
        assert_eq!(f.status.status, Status::GuardOn);
        update(&mut f); // GuardOn resolves same tick it is entered (module docs)
        assert_eq!(f.status.status, Status::Guard);
    }

    #[test]
    fn releasing_z_leaves_guard_through_guard_off_and_back_to_wait() {
        let mut f = mario();
        hold_z(&mut f, true);
        update(&mut f);
        update(&mut f);
        assert_eq!(f.status.status, Status::Guard);

        hold_z(&mut f, false);
        update(&mut f); // schedules the release, straight into GuardOff
        assert_eq!(f.status.status, Status::GuardOff);

        // `release_lag` was already ticking down during GuardOn/Guard, so
        // `GuardOff` clears it in at most `GUARD_RELEASE_LAG` more updates.
        for _ in 0..GUARD_RELEASE_LAG {
            if f.status.status == Status::Wait {
                break;
            }
            update(&mut f);
        }
        assert_eq!(f.status.status, Status::Wait);
    }

    #[test]
    fn a_shield_held_long_enough_decays_and_eventually_breaks() {
        let mut f = mario();
        hold_z(&mut f, true);
        update(&mut f);
        update(&mut f);
        assert_eq!(f.status.status, Status::Guard);
        let starting_health = f.guard.shield_health;

        for _ in 0..GUARD_DECAY_INT {
            update(&mut f);
        }
        assert_eq!(f.guard.shield_health, starting_health - 1.0);

        // Enough decay ticks to exhaust the whole bar breaks the shield.
        for _ in 0..(GUARD_DECAY_INT * (starting_health as i32 - 1)) {
            update(&mut f);
        }
        assert_eq!(f.status.status, Status::ShieldBreakFly);
        assert_eq!(f.guard.shield_health, GUARD_HEALTH_BREAK_RESPAWN);
    }

    #[test]
    fn guard_on_requires_shield_health() {
        let mut f = mario();
        f.guard.shield_health = 0.0;
        hold_z(&mut f, true);
        assert!(!check_guard_on(&mut f));
        assert_eq!(f.status.status, Status::Wait);
    }

    fn bounds() -> BlastZone {
        BlastZone {
            top: 200.0,
            bottom: -200.0,
            left: -300.0,
            right: 300.0,
        }
    }

    #[test]
    fn falling_below_the_blast_zone_kills_and_costs_a_stock() {
        let mut f = mario();
        f.pos.y = -201.0;
        let stocks_before = f.stocks;
        assert!(check_dead(&mut f, bounds()));
        assert_eq!(f.status.status, Status::DeadDown);
        assert_eq!(f.stocks, stocks_before - 1);
    }

    #[test]
    fn crossing_either_side_enters_the_shared_left_right_status() {
        let mut right = mario();
        right.pos.x = 301.0;
        assert!(check_dead(&mut right, bounds()));
        assert_eq!(right.status.status, Status::DeadLeftRight);

        let mut left = mario();
        left.pos.x = -301.0;
        assert!(check_dead(&mut left, bounds()));
        assert_eq!(left.status.status, Status::DeadLeftRight);
    }

    #[test]
    fn staying_inside_the_blast_zone_does_not_kill() {
        let mut f = mario();
        assert!(!check_dead(&mut f, bounds()));
        assert_eq!(f.status.status, Status::Wait);
        assert_eq!(f.stocks, 3);
    }

    #[test]
    fn a_dead_fighter_respawns_after_its_wait_with_damage_reset() {
        let mut f = mario();
        f.damage = 80;
        f.pos.y = -201.0;
        check_dead(&mut f, bounds());
        let stocks_after_death = f.stocks;

        for _ in 0..(DEAD_WAIT as i32 - 1) {
            update(&mut f);
            assert!(!try_rebirth(&mut f, Vec3::ZERO));
        }
        update(&mut f);
        assert!(try_rebirth(&mut f, Vec3::new(5.0, 5.0, 0.0)));
        assert_eq!(f.status.status, Status::RebirthDown);
        assert_eq!(f.damage, 0);
        assert_eq!(f.pos, Vec3::new(5.0, 5.0, 0.0));
        assert_eq!(f.stocks, stocks_after_death);
    }

    #[test]
    fn running_out_of_stocks_sleeps_instead_of_respawning() {
        let mut f = mario();
        f.stocks = 0;
        f.pos.y = -201.0;
        check_dead(&mut f, bounds()); // stocks: 0 -> -1
        for _ in 0..(DEAD_WAIT as i32) {
            update(&mut f);
        }
        assert!(try_rebirth(&mut f, Vec3::ZERO));
        assert_eq!(f.status.status, Status::Sleep);
    }

    #[test]
    fn the_full_rebirth_sequence_ends_grounded_and_invincible() {
        let mut f = mario();
        set_rebirth_down(&mut f, Vec3::new(1.0, 2.0, 0.0));
        assert_eq!(f.situation, Situation::Ground);

        for _ in 0..(REBIRTH_DOWN_WAIT as i32 - 1) {
            update(&mut f);
            assert_eq!(f.status.status, Status::RebirthDown);
        }
        update(&mut f); // RebirthDown's wait ends -> RebirthStand
        assert_eq!(f.status.status, Status::RebirthStand);
        update(&mut f); // RebirthStand collapses straight to RebirthWait
        assert_eq!(f.status.status, Status::RebirthWait);

        for _ in 0..(REBIRTH_WAIT_WAIT as i32 - 1) {
            update(&mut f);
            assert_eq!(f.status.status, Status::RebirthWait);
        }
        update(&mut f);
        assert_eq!(f.status.status, Status::Fall);
        assert_eq!(f.invincible_frames, REBIRTH_INVINCIBLE_FRAMES);
    }

    #[test]
    fn acting_during_rebirth_wait_cancels_it_early_with_invincibility() {
        let mut f = mario();
        set_rebirth_down(&mut f, Vec3::ZERO);
        for _ in 0..(REBIRTH_DOWN_WAIT as i32) {
            update(&mut f);
        }
        update(&mut f); // -> RebirthWait
        assert_eq!(f.status.status, Status::RebirthWait);

        tap_a(&mut f); // jabs, which runs through ground_interrupt
        update(&mut f);
        assert_eq!(f.status.status, Status::Attack11);
        assert_eq!(f.invincible_frames, REBIRTH_INVINCIBLE_FRAMES);
    }

    /// A level, ledge-grabbable line from -2318 to 2318 — the same shape as
    /// Dream Land's main platform (`crate::collision`'s own test fixture).
    const LEDGE: Segment = Segment {
        x1: -2318,
        y1: 0,
        x2: 2318,
        y2: 0,
        flags: collision::flags::CLIFF,
    };

    fn floors() -> [(u16, Segment); 1] {
        [(3, LEDGE)]
    }

    #[test]
    fn falling_near_the_right_edge_while_facing_left_is_caught() {
        let from = Vec3::new(2300.0, 50.0, 0.0);
        let to = Vec3::new(2300.0, -50.0, 0.0);
        let caught = cliff_catch_candidate(Facing::Left, 0, from, to, floors);
        assert_eq!(caught, Some((3, Vec2::new(2318.0, 0.0))));
    }

    #[test]
    fn a_line_without_the_cliff_flag_cannot_be_caught() {
        let plain = Segment { flags: 0, ..LEDGE };
        let from = Vec3::new(2300.0, 50.0, 0.0);
        let to = Vec3::new(2300.0, -50.0, 0.0);
        let caught = cliff_catch_candidate(Facing::Left, 0, from, to, || [(3u16, plain)]);
        assert_eq!(caught, None);
    }

    #[test]
    fn still_on_cliffcatch_cooldown_cannot_be_caught() {
        let from = Vec3::new(2300.0, 50.0, 0.0);
        let to = Vec3::new(2300.0, -50.0, 0.0);
        let caught = cliff_catch_candidate(Facing::Left, 1, from, to, floors);
        assert_eq!(caught, None);
    }

    #[test]
    fn too_far_from_the_corner_is_not_caught() {
        // The right corner is at x=2318; 800 units short of it is x=1518,
        // so 1500 is just outside the real corner-proximity tolerance.
        let from = Vec3::new(1500.0, 50.0, 0.0);
        let to = Vec3::new(1500.0, -50.0, 0.0);
        let caught = cliff_catch_candidate(Facing::Left, 0, from, to, floors);
        assert_eq!(caught, None);
    }

    #[test]
    fn catching_a_ledge_hangs_exactly_at_its_corner() {
        let mut f = mario();
        f.situation = Situation::Air;
        set_cliff_catch(&mut f, 3, Vec2::new(2318.0, 0.0));
        assert_eq!(f.status.status, Status::CliffCatch);
        assert_eq!(f.pos.x, 2318.0);
        assert_eq!(f.pos.y, 0.0);
        assert_eq!(f.situation, Situation::Air);

        update(&mut f); // CliffCatch collapses straight to CliffWait
        assert_eq!(f.status.status, Status::CliffWait);
    }

    #[test]
    fn low_damage_gets_the_long_fall_wait_high_damage_the_short_one() {
        let mut f = mario();
        set_cliff_catch(&mut f, 3, Vec2::ZERO);
        update(&mut f);
        assert_eq!(f.cliff.fall_wait, CLIFF_FALL_WAIT_DAMAGE_LOW);

        let mut hurt = mario();
        hurt.damage = CLIFF_DAMAGE_HIGH;
        set_cliff_catch(&mut hurt, 3, Vec2::ZERO);
        update(&mut hurt);
        assert_eq!(hurt.cliff.fall_wait, CLIFF_FALL_WAIT_DAMAGE_HIGH);
    }

    #[test]
    fn holding_the_ledge_past_the_fall_wait_drops_into_damage_fall() {
        let mut f = mario();
        set_cliff_catch(&mut f, 3, Vec2::ZERO);
        update(&mut f); // -> CliffWait
        for _ in 0..(CLIFF_FALL_WAIT_DAMAGE_LOW - 1) {
            update(&mut f);
            assert_eq!(f.status.status, Status::CliffWait);
        }
        update(&mut f);
        assert_eq!(f.status.status, Status::DamageFall);
        assert_eq!(f.cliffcatch_wait, CLIFF_CATCH_WAIT);
    }

    #[test]
    fn tapping_attack_from_the_ledge_climbs_up_attacking() {
        let mut f = mario();
        f.facing = Facing::Right;
        set_cliff_catch(&mut f, 3, Vec2::new(100.0, 0.0));
        update(&mut f); // -> CliffWait
        let x_before = f.pos.x;

        tap_a(&mut f);
        update(&mut f); // -> CliffAttackQuick1 (0% damage)
        assert_eq!(f.status.status, Status::CliffAttackQuick1);
        update(&mut f); // -> CliffAttackQuick2
        assert_eq!(f.status.status, Status::CliffAttackQuick2);
        update(&mut f); // -> Wait, standing on stage
        assert_eq!(f.status.status, Status::Wait);
        assert_eq!(f.situation, Situation::Ground);
        assert_eq!(f.pos.x, x_before + 5.0);
    }

    #[test]
    fn holding_toward_the_stage_climbs_back_up() {
        let mut f = mario();
        f.facing = Facing::Right;
        set_cliff_catch(&mut f, 3, Vec2::ZERO);
        update(&mut f); // -> CliffWait
        update(&mut f); // arms is_allow_interrupt (neutral stick this frame)

        hold(&mut f, 80, 0); // forward, same side as facing
        update(&mut f);
        assert_eq!(f.status.status, Status::CliffClimbQuick1);
    }

    #[test]
    fn holding_away_and_down_lets_go_of_the_ledge() {
        let mut f = mario();
        f.facing = Facing::Right;
        set_cliff_catch(&mut f, 3, Vec2::ZERO);
        update(&mut f); // -> CliffWait
        update(&mut f); // arms is_allow_interrupt

        hold(&mut f, -80, -80); // away from facing and down
        update(&mut f);
        assert_eq!(f.status.status, Status::Fall);
        assert_eq!(f.cliffcatch_wait, CLIFF_CATCH_WAIT);
    }

    #[test]
    fn holding_straight_up_always_climbs_regardless_of_facing() {
        let mut f = mario();
        f.facing = Facing::Left;
        set_cliff_catch(&mut f, 3, Vec2::ZERO);
        update(&mut f);
        update(&mut f); // arm the latch

        hold(&mut f, 0, 80); // straight up
        update(&mut f);
        assert_eq!(f.status.status, Status::CliffClimbQuick1);
    }

    #[test]
    fn tapping_b_up_starts_ground_super_jump_punch() {
        let mut f = mario();
        hold(&mut f, 0, 80);
        f.prev_input.buttons = N64Buttons::default();
        f.input.buttons = N64Buttons(N64Buttons::B);

        assert!(check_special_hi(&mut f));
        assert_eq!(f.status.status, AnyStatus::Mario(MarioStatus::SpecialHi));
        assert_eq!(
            f.status.timing.anim_length,
            Some(MARIO_SUPERJUMP_LENGTH_FRAMES)
        );
    }

    #[test]
    fn aerial_super_jump_preserves_the_sourced_entry_velocity_rule() {
        let mut f = airborne_mario();
        f.physics.vel_air = Vec3::new(30.0, -12.0, 0.0);
        hold(&mut f, 0, 80);
        f.prev_input.buttons = N64Buttons::default();
        f.input.buttons = N64Buttons(N64Buttons::B);

        assert!(check_special_hi(&mut f));
        assert_eq!(f.status.status, AnyStatus::Mario(MarioStatus::SpecialAirHi));
        assert_eq!(f.physics.vel_air, Vec3::new(20.0, 0.0, 0.0));
    }

    #[test]
    fn neutral_b_reverses_then_queues_one_fireball_on_source_frame_sixteen() {
        let mut f = mario();
        hold(&mut f, -30, 0);
        f.prev_input.buttons = N64Buttons::default();
        f.input.buttons = N64Buttons(N64Buttons::B);

        assert!(check_special_n(&mut f));
        assert_eq!(f.status.status, AnyStatus::Mario(MarioStatus::SpecialN));
        assert_eq!(f.facing, Facing::Left);
        assert_eq!(
            f.status.timing.anim_length,
            Some(MARIO_FIREBALL_LENGTH_FRAMES)
        );

        for _ in 0..15 {
            update(&mut f);
            assert_eq!(f.take_weapon_spawn(), None);
        }
        update(&mut f);
        assert_eq!(
            f.take_weapon_spawn(),
            Some(crate::weapon::WeaponSpawn {
                kind: crate::weapon::WeaponKind::MarioFireball,
                owner_port: 0,
                position: Vec3::ZERO,
                facing: -1.0,
            })
        );
        update(&mut f);
        assert_eq!(f.take_weapon_spawn(), None);
    }

    #[test]
    fn fireball_map_transition_keeps_the_pending_spawn_and_animation_frame() {
        let mut f = airborne_mario();
        set_mario_special_air_n(&mut f);
        f.status.anim_frame = 15.0;
        switch_mario_fireball_ground(&mut f);
        assert_eq!(f.status.status, AnyStatus::Mario(MarioStatus::SpecialN));
        assert_eq!(f.status.anim_frame, 15.0);
        assert!(!f.mario_special_n.spawned);
        update(&mut f);
        assert!(f.mario_special_n.spawned);
    }

    #[test]
    fn down_b_starts_the_sourced_aerial_tornado_phase_from_ground_or_air() {
        let mut grounded = mario();
        hold(&mut grounded, 0, -80);
        grounded.prev_input.buttons = N64Buttons::default();
        grounded.input.buttons = N64Buttons(N64Buttons::B);
        assert!(check_special_lw(&mut grounded));
        assert_eq!(
            grounded.status.status,
            AnyStatus::Mario(MarioStatus::SpecialAirLw)
        );
        assert_eq!(grounded.situation, Situation::Air);
        assert_eq!(grounded.physics.vel_air.y, -7.0);

        let mut airborne = airborne_mario();
        hold(&mut airborne, 0, -80);
        airborne.prev_input.buttons = N64Buttons::default();
        airborne.input.buttons = N64Buttons(N64Buttons::B);
        assert!(check_special_lw(&mut airborne));
        assert_eq!(
            airborne.status.status,
            AnyStatus::Mario(MarioStatus::SpecialAirLw)
        );
        assert_eq!(airborne.physics.vel_air.y, -7.0);
    }

    #[test]
    fn tornado_finisher_spends_its_rise_and_reduces_horizontal_clamp() {
        let mut f = airborne_mario();
        set_mario_special_air_lw(&mut f);
        f.status.anim_frame = MARIO_TORNADO_FINISH_FRAME;
        update(&mut f);
        assert!(f.mario_special_lw.rise_exhausted);
        assert!(!f.mario_special_lw.rise_enabled);
        apply_mario_special_lw_air_physics(&mut f);
        assert_eq!(f.mario_special_lw.friction, -2.0);
    }

    #[test]
    fn super_jump_steers_before_launch_then_selects_facing_once() {
        let mut f = mario();
        set_mario_special_hi(&mut f);
        hold(&mut f, 80, 0);

        update(&mut f);
        assert!(f.root_motion.rotate_z < 0.0);
        assert!(!f.mario_special_hi.launch_started);

        f.status.anim_frame = MARIO_SUPERJUMP_LAUNCH_FRAME;
        hold(&mut f, -80, 0);
        update(&mut f);
        assert_eq!(f.facing, Facing::Left);
        assert!(f.mario_special_hi.launch_started);

        hold(&mut f, 80, 0);
        update(&mut f);
        assert_eq!(f.facing, Facing::Left);
    }

    #[test]
    fn super_jump_enters_fall_special_only_after_the_40_frame_figatree() {
        let mut f = airborne_mario();
        set_mario_special_air_hi(&mut f);
        for _ in 0..(MARIO_SUPERJUMP_LENGTH_FRAMES as usize - 1) {
            update(&mut f);
            assert_eq!(f.status.status, AnyStatus::Mario(MarioStatus::SpecialAirHi));
        }
        update(&mut f);
        assert_eq!(f.status.status, AnyStatus::Common(Status::FallSpecial));
        assert_eq!(f.fall_special.drift, f.attributes.air_speed_max_x * 0.6);
        assert!(f.fall_special.is_goto_landing);
        assert!(f.fall_special.is_fall_accelerate);
        assert_eq!(f.fall_special.landing_lag, 0.28);
    }

    #[test]
    fn donkey_charge_cycles_store_ten_levels_then_cancel_without_spending_them() {
        let mut f = Fighter::new(crate::fighter::FighterKind::Donkey, 0, 3);
        f.situation = Situation::Ground;
        set_donkey_special_n(&mut f);
        for _ in 0..8 {
            update(&mut f);
        }
        assert_eq!(
            f.status.status,
            AnyStatus::Donkey(DonkeyStatus::SpecialNLoop)
        );
        for _ in 0..11 {
            update(&mut f);
        }
        assert_eq!(f.donkey_special_n.charge_level, 0);
        for _ in 0..(12 * 11) {
            update(&mut f);
        }
        assert_eq!(f.donkey_special_n.charge_level, 10);
        assert_eq!(f.status.status, AnyStatus::Common(Status::Wait));
        set_donkey_special_n(&mut f);
        assert!(f.donkey_special_n.release);
        for _ in 0..20 {
            update(&mut f);
        }
        assert_eq!(
            f.status.status,
            AnyStatus::Donkey(DonkeyStatus::SpecialNFull)
        );
        assert_eq!(f.donkey_special_n.attack_charge, 10);
        assert_eq!(f.donkey_special_n.charge_level, 0);
    }

    #[test]
    fn donkey_hand_slap_enters_two_pulse_loop_then_finishes() {
        let mut f = Fighter::new(crate::fighter::FighterKind::Donkey, 0, 3);
        f.situation = Situation::Ground;
        set_donkey_special_lw(&mut f);
        for _ in 0..3 {
            update(&mut f);
        }
        assert_eq!(
            f.status.status,
            AnyStatus::Donkey(DonkeyStatus::SpecialLwLoop)
        );
        for _ in 0..28 {
            update(&mut f);
        }
        assert_eq!(
            f.status.status,
            AnyStatus::Donkey(DonkeyStatus::SpecialLwEnd)
        );
        for _ in 0..5 {
            update(&mut f);
        }
        assert_eq!(f.status.status, AnyStatus::Common(Status::Wait));
    }
}
