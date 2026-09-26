//! Hitboxes, hurtboxes and hit resolution — Training Mode's first real
//! grounded attack, `F1` acceptance criterion 5.
//!
//! Ported from three places in the decompilation:
//!
//! * `relocData/202_MarioMainMotion.c`'s `dMarioMainMotion_Jab1` — the literal
//!   `ftMotionCommandMakeAttackColl(...)` arguments for Mario's neutral jab
//!   (`Attack11`), which is where a Smash 64 hitbox's numbers actually live.
//!   There is no separate per-character "attack table"; the hitbox is a
//!   motion-event argument list baked into the animation script.
//! * `ft/ftcommon/ftcommondamage.c`'s `ftCommonDamageGetKnockbackAngle` and
//!   `ftCommonDamageInitDamageVars` — the "Sakurai angle" resolution and the
//!   knockback-vector construction.
//! * `ft/ftparam.c`'s `ftParamGetCommonKnockback` and `ftParamGetHitStun` —
//!   the knockback magnitude and hitstun-length formulas.
//!
//! ## What is simplified here, and why
//!
//! * **Joint attachment comes from the current pose.** The PSP runtime
//!   samples `FTStruct::joints` into portable transforms. A host caller that
//!   has no skeleton retains the old fallback for non-root joints.
//! * **A sphere hurtbox, not a per-bone capsule set.** The original tests a
//!   hitbox sphere against eleven `FTDamageColl` capsules per fighter
//!   (`gmCollisionCheckFighterInFighterRange`). No per-bone hurtbox system
//!   exists yet, so [`MARIO_HURTBOX_RADIUS`] stands in with a single sphere at
//!   the fighter's root — sized from Mario's own real collision-diamond width
//!   (`dMarioMain_attr.map_coll = { 320, 190, 0, 150 }`, so half of `150`),
//!   not an invented number.
//! * **No hitlag and no `recent_damage` for fighter hits.**
//!   `ftParamGetCommonKnockback` runs with Training's damage ratio and each
//!   side's handicap ([`crate::stale`]), and fighter hitboxes deal
//!   stale-move-scaled damage. The hit path still passes `recent_damage` as
//!   zero where the source passes the frame's `damage_queue` (see
//!   `TODO.md`).
//! * **Knockback decay, not a friction curve.** The original decelerates
//!   `vel_damage_ground` by friction every frame the way normal ground
//!   movement does. Until that exists, [`crate::fighter::Fighter::tick_timers`]
//!   holds knockback constant for the hit's hitstun duration and then snaps it
//!   to zero the frame hitstun ends, rather than bleeding it off gradually.
//! * **`Damage` status entered, but not fully.** A landed hit now moves the
//!   defender into the real `DamageHi/N/Lw1-3`/`DamageAir1-3`/`DamageFlyN`/
//!   `FlyTop` status (`damage_status`), matching `ftCommonDamageGetDamageLevel`'s
//!   hitstun tiers and the defender's ground/air situation. Still missing:
//!   the hit-location Hi/Lw index (every hit reads as "N", the middle
//!   column — no system yet computes where on the target's body a hitbox
//!   landed), `DamageFlyRoll` (its selection is a coin flip, and there is no
//!   shared RNG source to drive it), and the "a shallow hit's upward
//!   knockback launches the target airborne anyway" angle branch (dropped —
//!   see [`crate::status::Status::is_grounded`]'s doc comment). No animation
//!   plays for any of these yet — `Status::anim_slot`'s catch-all keeps the
//!   current pose, same as `Attack11`.

use ssb_engine::math::{sin_cos, Vec3};

use crate::fighter::Fighter;
use crate::status::{self, AnyStatus, FoxStatus, MarioStatus, Status, StatusTiming};

/// A hitbox descriptor, transcribed field-for-field from a
/// `ftMotionCommandMakeAttackColl(aid, gid, jid, dmg, reb, elem, sz, ox, oy,
/// oz, ang, kbs, kbw, ga, sd, fl, fk, kbb)` call, so it can be checked against
/// the decomp source by eye. Fields the ported hit-resolution path does not
/// use yet (rebound, element, shield damage, sound) are not carried.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hitbox {
    /// `dmg` — base damage, added to the target's percent as-is.
    pub damage: i32,
    /// `ox, oy, oz` — offset from the attachment joint.
    pub offset: Vec3,
    /// `sz` — the motion command's "size", which the original halves into a
    /// radius (`fttypes.h`); this field is already that radius.
    pub radius: f32,
    /// `ang` — knockback angle in degrees, or the sentinel `361` for the
    /// "Sakurai angle" ([`sakurai_angle_radians`]).
    pub angle: i32,
    /// `kbs` — knockback growth (KBG), applied as `kbs * 0.01`.
    pub kb_scale: i32,
    /// `kbw` — weight-dependent knockback scale (WDSC). Zero selects the
    /// damage-dependent branch of [`common_knockback`].
    pub kb_weight: i32,
    /// `kbb` — base knockback (BKB).
    pub kb_base: i32,
}

/// Mario's `Attack11` (neutral jab) primary hitbox — `dMarioMainMotion_Jab1`,
/// `relocData/202_MarioMainMotion.c`:
/// `ftMotionCommandMakeAttackColl(0, 0, 10, 2, 1, 0, 160, 0, 0, 0, 361, 50, 0,
/// 3, 0, 0, 0, 8)`.
pub const MARIO_JAB1_HITBOX: Hitbox = Hitbox {
    damage: 2,
    offset: Vec3::new(0.0, 0.0, 0.0),
    radius: 160.0 / 2.0,
    angle: 361,
    kb_scale: 50,
    kb_weight: 0,
    kb_base: 8,
};

/// The frame window `Jab1`'s hitbox is active — `WaitAsync(2)` before
/// `MakeAttackColl`, then `Wait(2)` before `ClearAttackCollAll`. Half-open:
/// active for `anim_frame` in `2.0..4.0`.
pub const MARIO_JAB1_HITBOX_START: f32 = 2.0;
pub const MARIO_JAB1_HITBOX_END: f32 = 4.0;

/// `Attack11`'s total length in frames, summed from `dMarioMainMotion_Jab1`'s
/// own `WaitAsync`/`Wait` commands (`2 + 2 + 10`). Not extracted from a
/// figatree file — there is no ported animation for this status yet (module
/// docs) — but a real number read off the motion script, not a guess.
pub const MARIO_ATTACK11_LENGTH_FRAMES: f32 = 14.0;

/// Mario's second `Jab1` hitbox (joint 9, the forearm) — closing the module
/// docs' former "one hitbox, not five" gap for `Jab1` specifically: with
/// both hitboxes' `ox, oy, oz` already `(0, 0, 0)`, adding the second one
/// costs nothing numerically (it lands exactly on top of the first) but
/// completes the pair the motion script actually spawns.
/// `ftMotionCommandMakeAttackColl(1, 0, 9, 2, 1, 0, 160, 0, 0, 0, 361, 50, 0,
/// 3, 0, 0, 0, 8)`.
pub const MARIO_JAB1_HITBOX_2: Hitbox = MARIO_JAB1_HITBOX;

/// Mario's hurtbox radius stand-in — half of `dMarioMain_attr.map_coll`'s
/// `150.0` width. See the module docs' "sphere hurtbox" simplification.
pub const MARIO_HURTBOX_RADIUS: f32 = 150.0 / 2.0;

/// One hitbox and the half-open frame window it is active —
/// `MakeAttackColl`/`WaitAsync`/`Wait`/`ClearAttackCollAll` baked into a
/// single value instead of the timing living apart from the data the way
/// [`MARIO_JAB1_HITBOX_START`]/`_END` used to for `Jab1` alone.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ActiveHitbox {
    pub hitbox: Hitbox,
    pub start: f32,
    pub end: f32,
    /// The `MakeAttackColl` generation within one motion. A source
    /// `ClearAttackCollAll` followed by a new `MakeAttackColl` starts a new
    /// generation even when the two frame windows touch; the original's
    /// per-attack hit record is cleared with the old collision object.
    pub hit_generation: u8,
}

impl ActiveHitbox {
    pub const fn new(hitbox: Hitbox, start: f32, end: f32) -> Self {
        ActiveHitbox {
            hitbox,
            start,
            end,
            hit_generation: 0,
        }
    }

    /// Marks an independently-created source collision generation. Most
    /// moves use generation zero throughout: replacing a hitbox without a
    /// preceding `ClearAttackCollAll` must not re-hit a target.
    pub const fn with_hit_generation(mut self, hit_generation: u8) -> Self {
        self.hit_generation = hit_generation;
        self
    }

    pub fn is_active(&self, anim_frame: f32) -> bool {
        (self.start..self.end).contains(&anim_frame)
    }
}

/// A single target's record for an attacker's currently live collision
/// generation. This is the fixed-size Training equivalent of the original
/// `GMAttackRecord` target list: it suppresses repeated contacts inside one
/// `MakeAttackColl` lifetime, but a later `ClearAttackCollAll` generation can
/// hit the target again.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct HitRecord {
    hit_generation: Option<u8>,
}

/// A full attack: every hitbox its motion script throws out, and the
/// attack's total length in frames (every `WaitAsync`/`Wait` in the script
/// summed, including any after the last `ClearAttackCollAll` — the same
/// methodology [`MARIO_ATTACK11_LENGTH_FRAMES`]'s doc comment used).
#[derive(Debug, Clone, Copy)]
pub struct MoveData {
    pub hitboxes: &'static [ActiveHitbox],
    pub length_frames: f32,
    /// For an aerial attack only: if this fighter has no dedicated
    /// `LandingAirX` motion file for it, landing mid-move instead takes
    /// `LandingAirNull` for this percentage of the fighter's normal landing
    /// lag (`ftCommonAttackAirProcMap`'s `F_PCT_TO_DEC(flag1)` branch) —
    /// `None` when a dedicated landing clip exists (or for a non-aerial
    /// move, where this is meaningless).
    pub landing_lag_percent: Option<u8>,
}

/// Mario's `DashAttack` — `dMarioMainMotion_DashAttack`,
/// `relocData/202_MarioMainMotion.c`. One hitbox slot reused with weaker
/// numbers after frame 11 (`aid` 0 both times) — a real "sourspot" the
/// original expresses as the same slot getting overwritten mid-swing, not
/// two hitboxes: `HitRecord` suppression already means only
/// one of the two windows can ever connect. `WaitAsync(7)` +
/// `MakeAttackColl(...16)`, `Wait(4)` + `MakeAttackColl(...10)`, `Wait(17)` +
/// `ClearAttackCollAll` — total `7 + 4 + 17 = 28`.
pub static MARIO_DASH_ATTACK: MoveData = MoveData {
    hitboxes: &[
        ActiveHitbox::new(
            Hitbox {
                damage: 12,
                offset: Vec3::new(40.0, 0.0, 0.0),
                radius: 250.0 / 2.0,
                angle: 361,
                kb_scale: 100,
                kb_weight: 0,
                kb_base: 16,
            },
            7.0,
            11.0,
        ),
        ActiveHitbox::new(
            Hitbox {
                damage: 10,
                offset: Vec3::new(40.0, 0.0, 0.0),
                radius: 250.0 / 2.0,
                angle: 361,
                kb_scale: 100,
                kb_weight: 0,
                kb_base: 10,
            },
            11.0,
            28.0,
        ),
    ],
    length_frames: 28.0,
    landing_lag_percent: None,
};

/// Mario's forward tilt, one of three angle variants
/// (`dMarioMainMotion_FTiltHigh`/`FTilt`/`FTiltLow`) chosen by stick angle —
/// see `crate::status::set_ftilt`. All three share this shape (only `jid`
/// differs, which this codebase does not use — module docs), so one
/// constructor builds all three `MoveData`s from their real, distinct
/// damage/knockback numbers. `WaitAsync(8)` + two `MakeAttackColl`s,
/// `Wait(10)` + `ClearAttackCollAll` — total `8 + 10 = 18`.
/// Builds one of the three `FTilt*` `MoveData`s. A macro rather than a
/// `const fn` because a `const fn` returning a `&'static [ActiveHitbox]`
/// built from a local array literal does not get the `'static` promotion a
/// literal `static` item's own initializer gets — the array would be freed
/// at the end of the function body. Expanding at each `static`'s own
/// definition site keeps the promotion working.
macro_rules! mario_ftilt {
    ($damage:expr) => {
        MoveData {
            hitboxes: &[
                ActiveHitbox::new(
                    Hitbox {
                        damage: $damage,
                        offset: Vec3::new(20.0, 0.0, 0.0),
                        radius: 180.0 / 2.0,
                        angle: 361,
                        kb_scale: 100,
                        kb_weight: 0,
                        kb_base: 10,
                    },
                    8.0,
                    18.0,
                ),
                ActiveHitbox::new(
                    Hitbox {
                        damage: $damage,
                        offset: Vec3::new(90.0, 0.0, 0.0),
                        radius: 230.0 / 2.0,
                        angle: 361,
                        kb_scale: 100,
                        kb_weight: 0,
                        kb_base: 10,
                    },
                    8.0,
                    18.0,
                ),
            ],
            length_frames: 18.0,
            landing_lag_percent: None,
        }
    };
}
/// `dMarioMainMotion_FTiltHigh`: `MakeAttackColl(0,0,24,14,...)`/`(1,0,25,14,...)`.
pub static MARIO_FTILT_HI: MoveData = mario_ftilt!(14);
/// `dMarioMainMotion_FTilt`: `MakeAttackColl(0,0,24,13,...)`/`(1,0,25,13,...)`.
pub static MARIO_FTILT: MoveData = mario_ftilt!(13);
/// `dMarioMainMotion_FTiltLow`: `MakeAttackColl(0,0,24,12,...)`/`(1,0,25,12,...)`.
pub static MARIO_FTILT_LOW: MoveData = mario_ftilt!(12);

/// Mario's up tilt — `dMarioMainMotion_UTilt`. A literal (non-Sakurai)
/// `86`-degree launch angle, unlike every other move ported so far.
/// `WaitAsync(5)` + two `MakeAttackColl`s, `Wait(12)` + `ClearAttackCollAll`
/// — total `5 + 12 = 17`.
pub static MARIO_UTILT: MoveData = MoveData {
    hitboxes: &[
        ActiveHitbox::new(
            Hitbox {
                damage: 10,
                offset: Vec3::new(0.0, 0.0, 0.0),
                radius: 180.0 / 2.0,
                angle: 86,
                kb_scale: 150,
                kb_weight: 0,
                kb_base: 0,
            },
            5.0,
            17.0,
        ),
        ActiveHitbox::new(
            Hitbox {
                damage: 10,
                offset: Vec3::new(60.0, 0.0, 0.0),
                radius: 290.0 / 2.0,
                angle: 86,
                kb_scale: 150,
                kb_weight: 0,
                kb_base: 0,
            },
            5.0,
            17.0,
        ),
    ],
    length_frames: 17.0,
    landing_lag_percent: None,
};

/// Mario's down tilt — `dMarioMainMotion_DTilt`. `WaitAsync(5)` + two
/// `MakeAttackColl`s, `Wait(7)` + `ClearAttackCollAll` — total `5 + 7 = 12`.
pub static MARIO_DTILT: MoveData = MoveData {
    hitboxes: &[
        ActiveHitbox::new(
            Hitbox {
                damage: 12,
                offset: Vec3::new(20.0, 0.0, 0.0),
                radius: 180.0 / 2.0,
                angle: 361,
                kb_scale: 100,
                kb_weight: 0,
                kb_base: 0,
            },
            5.0,
            12.0,
        ),
        ActiveHitbox::new(
            Hitbox {
                damage: 12,
                offset: Vec3::new(140.0, 0.0, 0.0),
                radius: 260.0 / 2.0,
                angle: 361,
                kb_scale: 100,
                kb_weight: 0,
                kb_base: 0,
            },
            5.0,
            12.0,
        ),
    ],
    length_frames: 12.0,
    landing_lag_percent: None,
};

/// Mario's `Attack11` (neutral jab), now both real hitboxes
/// ([`MARIO_JAB1_HITBOX`], [`MARIO_JAB1_HITBOX_2`]) instead of the one the
/// module docs used to note as a gap.
pub static MARIO_JAB1: MoveData = MoveData {
    hitboxes: &[
        ActiveHitbox::new(
            MARIO_JAB1_HITBOX,
            MARIO_JAB1_HITBOX_START,
            MARIO_JAB1_HITBOX_END,
        ),
        ActiveHitbox::new(
            MARIO_JAB1_HITBOX_2,
            MARIO_JAB1_HITBOX_START,
            MARIO_JAB1_HITBOX_END,
        ),
    ],
    length_frames: MARIO_ATTACK11_LENGTH_FRAMES,
    landing_lag_percent: None,
};

/// Mario's `Attack12` (jab2) — `dMarioMainMotion_Jab2`. A literal (non-Sakurai)
/// `70°` launch angle, like `Jab1`'s follow-up hit usually is across the
/// cast. `WaitAsync(3)` then two `MakeAttackColl`s, `Wait(3)` then
/// `ClearAttackCollAll`, `WaitAsync(8)` then `SetFlag1(1)` — total
/// `3 + 3 + 8 = 14`. The `SetFlag1(1)` at the very end is what
/// `crate::status`'s jab-combo window reads as `animation_ended()`: for
/// both `Jab1` and `Jab2`, the flag flips at exactly the script's own
/// total length, so no separate flag state is needed.
pub static MARIO_JAB2: MoveData = MoveData {
    hitboxes: &[
        ActiveHitbox::new(
            Hitbox {
                damage: 2,
                offset: Vec3::new(16.0, 0.0, 0.0),
                radius: 180.0 / 2.0,
                angle: 70,
                kb_scale: 50,
                kb_weight: 0,
                kb_base: 8,
            },
            3.0,
            6.0,
        ),
        ActiveHitbox::new(
            Hitbox {
                damage: 2,
                offset: Vec3::new(0.0, 0.0, 0.0),
                radius: 180.0 / 2.0,
                angle: 70,
                kb_scale: 50,
                kb_weight: 0,
                kb_base: 8,
            },
            3.0,
            6.0,
        ),
    ],
    length_frames: 14.0,
    landing_lag_percent: None,
};

/// Mario's jab-combo finisher — `dMarioMainMotion_Jab3`
/// (`nFTMarioStatusAttack13`, [`crate::status::MarioStatus::Attack13`]). No
/// further `SetFlag1` combo window — this is the end of the chain, matching
/// `ftCommonAttack13ProcUpdate` never checking `is_goto_followup` for
/// non-Captain fighters. Both hitboxes' `SetAttackCollSize` calls at frame 5
/// only change hitbox 0's radius (`150` → `180`); hitbox 1's call sets it to
/// the same `280` it already had, so it is one unchanging window here.
/// `WaitAsync(3)` then two `MakeAttackColl`s, `Wait(2)` then the
/// `SetAttackCollSize` calls, `Wait(3)` then `ClearAttackCollAll` — total
/// `3 + 2 + 3 = 8`.
pub static MARIO_JAB3: MoveData = MoveData {
    hitboxes: &[
        ActiveHitbox::new(
            Hitbox {
                damage: 4,
                offset: Vec3::new(0.0, 0.0, 0.0),
                radius: 150.0 / 2.0,
                angle: 361,
                kb_scale: 100,
                kb_weight: 0,
                kb_base: 10,
            },
            3.0,
            5.0,
        ),
        ActiveHitbox::new(
            Hitbox {
                damage: 4,
                offset: Vec3::new(0.0, 0.0, 0.0),
                radius: 180.0 / 2.0,
                angle: 361,
                kb_scale: 100,
                kb_weight: 0,
                kb_base: 10,
            },
            5.0,
            8.0,
        ),
        ActiveHitbox::new(
            Hitbox {
                damage: 4,
                offset: Vec3::new(0.0, 0.0, 0.0),
                radius: 280.0 / 2.0,
                angle: 361,
                kb_scale: 100,
                kb_weight: 0,
                kb_base: 10,
            },
            3.0,
            8.0,
        ),
    ],
    length_frames: 8.0,
    landing_lag_percent: None,
};

/// Mario's Super Jump Punch — `dMarioMainMotion_SuperJumpPunchAir_0x16CC`.
/// The event script opens an initial strong two-hit window at frame 2 for one
/// frame, then, after its frame-9 `SetFlag1/2`, eight adjacent two-frame
/// coin-hit windows. Each loop clears and recreates the collision objects
/// between its windows, followed by a two-frame finishing pair.
/// The status remains live through the ROM's 40-frame figatree, so its motion
/// and its hitbox script intentionally have different end times.
pub static MARIO_SUPERJUMP: MoveData = MoveData {
    hitboxes: &[
        ActiveHitbox::new(
            Hitbox {
                damage: 5,
                offset: Vec3::new(0.0, 0.0, 0.0),
                radius: 190.0,
                angle: 70,
                kb_scale: 100,
                kb_weight: 110,
                kb_base: 0,
            },
            2.0,
            3.0,
        ),
        ActiveHitbox::new(
            Hitbox {
                damage: 5,
                offset: Vec3::new(160.0, 0.0, 0.0),
                radius: 130.0,
                angle: 90,
                kb_scale: 100,
                kb_weight: 110,
                kb_base: 0,
            },
            2.0,
            3.0,
        ),
        ActiveHitbox::new(
            Hitbox {
                damage: 1,
                offset: Vec3::new(0.0, 0.0, 60.0),
                radius: 155.0,
                angle: 75,
                kb_scale: 100,
                kb_weight: 100,
                kb_base: 0,
            },
            9.0,
            11.0,
        )
        .with_hit_generation(1),
        ActiveHitbox::new(
            Hitbox {
                damage: 1,
                offset: Vec3::new(150.0, 0.0, 60.0),
                radius: 130.0,
                angle: 80,
                kb_scale: 100,
                kb_weight: 100,
                kb_base: 0,
            },
            9.0,
            11.0,
        )
        .with_hit_generation(1),
        ActiveHitbox::new(
            Hitbox {
                damage: 1,
                offset: Vec3::new(0.0, 0.0, 60.0),
                radius: 155.0,
                angle: 75,
                kb_scale: 100,
                kb_weight: 100,
                kb_base: 0,
            },
            11.0,
            13.0,
        )
        .with_hit_generation(2),
        ActiveHitbox::new(
            Hitbox {
                damage: 1,
                offset: Vec3::new(150.0, 0.0, 60.0),
                radius: 130.0,
                angle: 80,
                kb_scale: 100,
                kb_weight: 100,
                kb_base: 0,
            },
            11.0,
            13.0,
        )
        .with_hit_generation(2),
        ActiveHitbox::new(
            Hitbox {
                damage: 1,
                offset: Vec3::new(0.0, 0.0, 60.0),
                radius: 155.0,
                angle: 75,
                kb_scale: 100,
                kb_weight: 100,
                kb_base: 0,
            },
            13.0,
            15.0,
        )
        .with_hit_generation(3),
        ActiveHitbox::new(
            Hitbox {
                damage: 1,
                offset: Vec3::new(150.0, 0.0, 60.0),
                radius: 130.0,
                angle: 80,
                kb_scale: 100,
                kb_weight: 100,
                kb_base: 0,
            },
            13.0,
            15.0,
        )
        .with_hit_generation(3),
        ActiveHitbox::new(
            Hitbox {
                damage: 1,
                offset: Vec3::new(0.0, 0.0, 60.0),
                radius: 155.0,
                angle: 75,
                kb_scale: 100,
                kb_weight: 100,
                kb_base: 0,
            },
            15.0,
            17.0,
        )
        .with_hit_generation(4),
        ActiveHitbox::new(
            Hitbox {
                damage: 1,
                offset: Vec3::new(150.0, 0.0, 60.0),
                radius: 130.0,
                angle: 80,
                kb_scale: 100,
                kb_weight: 100,
                kb_base: 0,
            },
            15.0,
            17.0,
        )
        .with_hit_generation(4),
        ActiveHitbox::new(
            Hitbox {
                damage: 1,
                offset: Vec3::new(0.0, 0.0, 60.0),
                radius: 155.0,
                angle: 75,
                kb_scale: 100,
                kb_weight: 100,
                kb_base: 0,
            },
            17.0,
            19.0,
        )
        .with_hit_generation(5),
        ActiveHitbox::new(
            Hitbox {
                damage: 1,
                offset: Vec3::new(150.0, 0.0, 60.0),
                radius: 130.0,
                angle: 80,
                kb_scale: 100,
                kb_weight: 100,
                kb_base: 0,
            },
            17.0,
            19.0,
        )
        .with_hit_generation(5),
        ActiveHitbox::new(
            Hitbox {
                damage: 1,
                offset: Vec3::new(0.0, 0.0, 60.0),
                radius: 155.0,
                angle: 75,
                kb_scale: 100,
                kb_weight: 100,
                kb_base: 0,
            },
            19.0,
            21.0,
        )
        .with_hit_generation(6),
        ActiveHitbox::new(
            Hitbox {
                damage: 1,
                offset: Vec3::new(150.0, 0.0, 60.0),
                radius: 130.0,
                angle: 80,
                kb_scale: 100,
                kb_weight: 100,
                kb_base: 0,
            },
            19.0,
            21.0,
        )
        .with_hit_generation(6),
        ActiveHitbox::new(
            Hitbox {
                damage: 1,
                offset: Vec3::new(0.0, 0.0, 60.0),
                radius: 155.0,
                angle: 75,
                kb_scale: 100,
                kb_weight: 100,
                kb_base: 0,
            },
            21.0,
            23.0,
        )
        .with_hit_generation(7),
        ActiveHitbox::new(
            Hitbox {
                damage: 1,
                offset: Vec3::new(150.0, 0.0, 60.0),
                radius: 130.0,
                angle: 80,
                kb_scale: 100,
                kb_weight: 100,
                kb_base: 0,
            },
            21.0,
            23.0,
        )
        .with_hit_generation(7),
        ActiveHitbox::new(
            Hitbox {
                damage: 1,
                offset: Vec3::new(0.0, 0.0, 60.0),
                radius: 155.0,
                angle: 75,
                kb_scale: 100,
                kb_weight: 100,
                kb_base: 0,
            },
            23.0,
            25.0,
        )
        .with_hit_generation(8),
        ActiveHitbox::new(
            Hitbox {
                damage: 1,
                offset: Vec3::new(150.0, 0.0, 60.0),
                radius: 130.0,
                angle: 80,
                kb_scale: 100,
                kb_weight: 100,
                kb_base: 0,
            },
            23.0,
            25.0,
        )
        .with_hit_generation(8),
        ActiveHitbox::new(
            Hitbox {
                damage: 3,
                offset: Vec3::new(0.0, 0.0, 0.0),
                radius: 225.0,
                angle: 50,
                kb_scale: 170,
                kb_weight: 0,
                kb_base: 0,
            },
            25.0,
            27.0,
        )
        .with_hit_generation(9),
        ActiveHitbox::new(
            Hitbox {
                damage: 3,
                offset: Vec3::new(150.0, 0.0, 0.0),
                radius: 100.0,
                angle: 50,
                kb_scale: 170,
                kb_weight: 0,
                kb_base: 0,
            },
            25.0,
            27.0,
        )
        .with_hit_generation(9),
    ],
    length_frames: crate::status::MARIO_SUPERJUMP_LENGTH_FRAMES,
    landing_lag_percent: None,
};

// `dMarioMainMotion_MarioTornadoGround` and `dMarioMainMotion_0x1884` use
// thirteen one-frame multihit pulses, three frames apart. Constructing the
// repeated windows here keeps their event-script cadence explicit without
// maintaining a hand-copied list of 78 near-identical entries.
const TORNADO_GROUND_SIDE_START: Hitbox = Hitbox {
    damage: 1,
    offset: Vec3::new(0.0, 300.0, 150.0),
    radius: 70.0,
    angle: 180,
    kb_scale: 0,
    kb_weight: 1,
    kb_base: 70,
};
const TORNADO_GROUND_CENTER_START: Hitbox = Hitbox {
    damage: 1,
    offset: Vec3::ZERO,
    radius: 110.0,
    angle: 90,
    kb_scale: 0,
    kb_weight: 1,
    kb_base: 10,
};
const TORNADO_GROUND_SIDE_LOOP: Hitbox = Hitbox {
    damage: 1,
    offset: Vec3::new(0.0, 280.0, 150.0),
    radius: 80.0,
    angle: 180,
    kb_scale: 0,
    kb_weight: 1,
    kb_base: 70,
};
const TORNADO_GROUND_SIDE_FINISH: Hitbox = Hitbox {
    damage: 1,
    offset: Vec3::new(0.0, 300.0, 150.0),
    radius: 100.0,
    angle: 90,
    kb_scale: 0,
    kb_weight: 1,
    kb_base: 120,
};
const TORNADO_AIR_SIDE_START: Hitbox = TORNADO_GROUND_SIDE_START;
const TORNADO_AIR_CENTER_START: Hitbox = Hitbox {
    damage: 1,
    offset: Vec3::ZERO,
    radius: 90.0,
    angle: 90,
    kb_scale: 0,
    kb_weight: 1,
    kb_base: 10,
};
const TORNADO_AIR_SIDE_LOOP: Hitbox = Hitbox {
    damage: 1,
    offset: Vec3::new(0.0, 260.0, 150.0),
    radius: 70.0,
    angle: 180,
    kb_scale: 0,
    kb_weight: 1,
    kb_base: 70,
};
const TORNADO_AIR_LOW_LOOP: Hitbox = Hitbox {
    damage: 1,
    offset: Vec3::new(0.0, 80.0, 0.0),
    radius: 80.0,
    angle: -90,
    kb_scale: 0,
    kb_weight: 1,
    kb_base: 10,
};
const TORNADO_AIR_HIGH_LOOP: Hitbox = Hitbox {
    damage: 1,
    offset: Vec3::new(0.0, 420.0, 0.0),
    radius: 50.0,
    angle: -90,
    kb_scale: 0,
    kb_weight: 1,
    kb_base: 70,
};
const TORNADO_AIR_SIDE_FINISH: Hitbox = TORNADO_GROUND_SIDE_FINISH;
const TORNADO_AIR_LOW_FINISH: Hitbox = Hitbox {
    damage: 1,
    offset: Vec3::new(0.0, 80.0, 0.0),
    radius: 110.0,
    angle: -90,
    kb_scale: 0,
    kb_weight: 1,
    kb_base: 120,
};
const TORNADO_AIR_HIGH_FINISH: Hitbox = Hitbox {
    damage: 1,
    offset: Vec3::new(0.0, 420.0, 0.0),
    radius: 45.0,
    angle: -90,
    kb_scale: 0,
    kb_weight: 1,
    kb_base: 120,
};
const TORNADO_PLACEHOLDER: ActiveHitbox = ActiveHitbox::new(TORNADO_GROUND_SIDE_START, 0.0, 0.0);

const fn tornado_ground_hitboxes() -> [ActiveHitbox; 31] {
    let mut out = [TORNADO_PLACEHOLDER; 31];
    out[0] = ActiveHitbox::new(TORNADO_GROUND_SIDE_START, 0.0, 4.0);
    out[1] = ActiveHitbox::new(
        Hitbox {
            offset: Vec3::new(0.0, 300.0, -150.0),
            ..TORNADO_GROUND_SIDE_START
        },
        0.0,
        4.0,
    );
    out[2] = ActiveHitbox::new(TORNADO_GROUND_CENTER_START, 0.0, 4.0);
    let mut i = 0;
    while i < 13 {
        let start = 4.0 + i as f32 * 3.0;
        let first = 3 + i * 2;
        out[first] = ActiveHitbox::new(TORNADO_GROUND_SIDE_LOOP, start, start + 1.0);
        out[first + 1] = ActiveHitbox::new(
            Hitbox {
                offset: Vec3::new(0.0, 280.0, -150.0),
                ..TORNADO_GROUND_SIDE_LOOP
            },
            start,
            start + 1.0,
        );
        i += 1;
    }
    out[29] = ActiveHitbox::new(TORNADO_GROUND_SIDE_FINISH, 43.0, 45.0);
    out[30] = ActiveHitbox::new(
        Hitbox {
            offset: Vec3::new(0.0, 300.0, -150.0),
            ..TORNADO_GROUND_SIDE_FINISH
        },
        43.0,
        45.0,
    );
    out
}

const fn tornado_air_hitboxes() -> [ActiveHitbox; 60] {
    let mut out = [TORNADO_PLACEHOLDER; 60];
    out[0] = ActiveHitbox::new(TORNADO_AIR_SIDE_START, 0.0, 4.0);
    out[1] = ActiveHitbox::new(
        Hitbox {
            offset: Vec3::new(0.0, 300.0, -150.0),
            ..TORNADO_AIR_SIDE_START
        },
        0.0,
        4.0,
    );
    out[2] = ActiveHitbox::new(TORNADO_AIR_CENTER_START, 0.0, 4.0);
    out[3] = ActiveHitbox::new(TORNADO_AIR_HIGH_LOOP, 0.0, 4.0);
    let mut i = 0;
    while i < 13 {
        let start = 4.0 + i as f32 * 3.0;
        let first = 4 + i * 4;
        out[first] = ActiveHitbox::new(TORNADO_AIR_SIDE_LOOP, start, start + 1.0);
        out[first + 1] = ActiveHitbox::new(
            Hitbox {
                offset: Vec3::new(0.0, 260.0, -150.0),
                ..TORNADO_AIR_SIDE_LOOP
            },
            start,
            start + 1.0,
        );
        out[first + 2] = ActiveHitbox::new(TORNADO_AIR_LOW_LOOP, start, start + 1.0);
        out[first + 3] = ActiveHitbox::new(TORNADO_AIR_HIGH_LOOP, start, start + 1.0);
        i += 1;
    }
    out[56] = ActiveHitbox::new(TORNADO_AIR_SIDE_FINISH, 43.0, 47.0);
    out[57] = ActiveHitbox::new(
        Hitbox {
            offset: Vec3::new(0.0, 300.0, -150.0),
            ..TORNADO_AIR_SIDE_FINISH
        },
        43.0,
        47.0,
    );
    out[58] = ActiveHitbox::new(TORNADO_AIR_LOW_FINISH, 43.0, 47.0);
    out[59] = ActiveHitbox::new(TORNADO_AIR_HIGH_FINISH, 43.0, 47.0);
    out
}

pub static MARIO_TORNADO_GROUND_HITBOXES: [ActiveHitbox; 31] = tornado_ground_hitboxes();
pub static MARIO_TORNADO_AIR_HITBOXES: [ActiveHitbox; 60] = tornado_air_hitboxes();

/// Mario's grounded and aerial Tornado scripts. Their hitbox sequences
/// differ after the opening, while the grounded one is selected only after a
/// landing transition preserves the action's current animation frame.
pub static MARIO_TORNADO_GROUND: MoveData = MoveData {
    hitboxes: &MARIO_TORNADO_GROUND_HITBOXES,
    length_frames: crate::status::MARIO_TORNADO_GROUND_LENGTH_FRAMES,
    landing_lag_percent: None,
};
pub static MARIO_TORNADO_AIR: MoveData = MoveData {
    hitboxes: &MARIO_TORNADO_AIR_HITBOXES,
    length_frames: crate::status::MARIO_TORNADO_AIR_LENGTH_FRAMES,
    landing_lag_percent: None,
};

/// Mario's neutral aerial — `dMarioMainMotion_AttackAirN`. Three
/// simultaneous hitboxes (`jid` 25/20/5 — both feet share one descriptor,
/// plus a wider body box), each replaced by a weaker phase after frame 11.
/// `WaitAsync(3)` + 3×`MakeAttackColl`, `Wait(8)` + 3× weaker
/// `MakeAttackColl`, `Wait(26)` + `ClearAttackCollAll` — total
/// `3 + 8 + 26 = 37`.
pub static MARIO_AIR_N: MoveData = MoveData {
    hitboxes: &[
        ActiveHitbox::new(
            Hitbox {
                damage: 14,
                offset: Vec3::new(10.0, 0.0, 0.0),
                radius: 240.0 / 2.0,
                angle: 361,
                kb_scale: 100,
                kb_weight: 0,
                kb_base: 15,
            },
            3.0,
            11.0,
        ),
        ActiveHitbox::new(
            Hitbox {
                damage: 14,
                offset: Vec3::new(10.0, 0.0, 0.0),
                radius: 240.0 / 2.0,
                angle: 361,
                kb_scale: 100,
                kb_weight: 0,
                kb_base: 15,
            },
            3.0,
            11.0,
        ),
        ActiveHitbox::new(
            Hitbox {
                damage: 14,
                offset: Vec3::new(0.0, 0.0, 0.0),
                radius: 260.0 / 2.0,
                angle: 361,
                kb_scale: 100,
                kb_weight: 0,
                kb_base: 15,
            },
            3.0,
            11.0,
        ),
        ActiveHitbox::new(
            Hitbox {
                damage: 11,
                offset: Vec3::new(10.0, 0.0, 0.0),
                radius: 240.0 / 2.0,
                angle: 361,
                kb_scale: 100,
                kb_weight: 0,
                kb_base: 0,
            },
            11.0,
            37.0,
        ),
        ActiveHitbox::new(
            Hitbox {
                damage: 11,
                offset: Vec3::new(10.0, 0.0, 0.0),
                radius: 240.0 / 2.0,
                angle: 361,
                kb_scale: 100,
                kb_weight: 0,
                kb_base: 0,
            },
            11.0,
            37.0,
        ),
        ActiveHitbox::new(
            Hitbox {
                damage: 11,
                offset: Vec3::new(0.0, 0.0, 0.0),
                radius: 260.0 / 2.0,
                angle: 361,
                kb_scale: 100,
                kb_weight: 0,
                kb_base: 0,
            },
            11.0,
            37.0,
        ),
    ],
    length_frames: 37.0,
    landing_lag_percent: Some(50),
};

/// Mario's forward aerial — `dMarioMainMotion_AttackAirF`. Two hitboxes
/// (`jid` 25 twice, different `oy`), weaker after frame 15.
/// `WaitAsync(11)` then two `MakeAttackColl`s, `Wait(4)` then two weaker
/// ones, `Wait(12)` then `ClearAttackCollAll` — total `11 + 4 + 12 = 27`.
pub static MARIO_AIR_F: MoveData = MoveData {
    hitboxes: &[
        ActiveHitbox::new(
            Hitbox {
                damage: 16,
                offset: Vec3::new(-30.0, 45.0, 0.0),
                radius: 220.0 / 2.0,
                angle: 361,
                kb_scale: 100,
                kb_weight: 0,
                kb_base: 10,
            },
            11.0,
            15.0,
        ),
        ActiveHitbox::new(
            Hitbox {
                damage: 16,
                offset: Vec3::new(80.0, 30.0, 0.0),
                radius: 270.0 / 2.0,
                angle: 361,
                kb_scale: 100,
                kb_weight: 0,
                kb_base: 10,
            },
            11.0,
            15.0,
        ),
        ActiveHitbox::new(
            Hitbox {
                damage: 10,
                offset: Vec3::new(-30.0, 45.0, 0.0),
                radius: 220.0 / 2.0,
                angle: 361,
                kb_scale: 100,
                kb_weight: 0,
                kb_base: 0,
            },
            15.0,
            27.0,
        ),
        ActiveHitbox::new(
            Hitbox {
                damage: 10,
                offset: Vec3::new(80.0, 30.0, 0.0),
                radius: 270.0 / 2.0,
                angle: 361,
                kb_scale: 100,
                kb_weight: 0,
                kb_base: 0,
            },
            15.0,
            27.0,
        ),
    ],
    length_frames: 27.0,
    landing_lag_percent: None,
};

/// Mario's back aerial — `dMarioMainMotion_AttackAirB`. Weaker phase also
/// shrinks the hitboxes (`220`/`270` vs `240`/`290`), not just damage/KBB.
/// `WaitAsync(10)` + 2×`MakeAttackColl`, `Wait(4)` + 2× weaker, `Wait(6)` +
/// `ClearAttackCollAll` — total `10 + 4 + 6 = 20`.
pub static MARIO_AIR_B: MoveData = MoveData {
    hitboxes: &[
        ActiveHitbox::new(
            Hitbox {
                damage: 16,
                offset: Vec3::new(-30.0, 45.0, 0.0),
                radius: 240.0 / 2.0,
                angle: 361,
                kb_scale: 100,
                kb_weight: 0,
                kb_base: 10,
            },
            10.0,
            14.0,
        ),
        ActiveHitbox::new(
            Hitbox {
                damage: 16,
                offset: Vec3::new(80.0, 30.0, 0.0),
                radius: 290.0 / 2.0,
                angle: 361,
                kb_scale: 100,
                kb_weight: 0,
                kb_base: 10,
            },
            10.0,
            14.0,
        ),
        ActiveHitbox::new(
            Hitbox {
                damage: 10,
                offset: Vec3::new(-30.0, 45.0, 0.0),
                radius: 220.0 / 2.0,
                angle: 361,
                kb_scale: 100,
                kb_weight: 0,
                kb_base: 0,
            },
            14.0,
            20.0,
        ),
        ActiveHitbox::new(
            Hitbox {
                damage: 10,
                offset: Vec3::new(80.0, 30.0, 0.0),
                radius: 270.0 / 2.0,
                angle: 361,
                kb_scale: 100,
                kb_weight: 0,
                kb_base: 0,
            },
            14.0,
            20.0,
        ),
    ],
    length_frames: 20.0,
    landing_lag_percent: None,
};

/// Mario's up aerial — `dMarioMainMotion_AttackAirU`. A literal (non-Sakurai)
/// launch angle, `80°` then `70°` in the weaker phase. `WaitAsync(2)` +
/// 2×`MakeAttackColl`, `Wait(3)` + 2× weaker, `Wait(7)` +
/// `ClearAttackCollAll` — total `2 + 3 + 7 = 12`.
pub static MARIO_AIR_HI: MoveData = MoveData {
    hitboxes: &[
        ActiveHitbox::new(
            Hitbox {
                damage: 12,
                offset: Vec3::new(0.0, 0.0, 0.0),
                radius: 220.0 / 2.0,
                angle: 80,
                kb_scale: 120,
                kb_weight: 0,
                kb_base: 0,
            },
            2.0,
            5.0,
        ),
        ActiveHitbox::new(
            Hitbox {
                damage: 12,
                offset: Vec3::new(0.0, 0.0, 0.0),
                radius: 250.0 / 2.0,
                angle: 80,
                kb_scale: 120,
                kb_weight: 0,
                kb_base: 0,
            },
            2.0,
            5.0,
        ),
        ActiveHitbox::new(
            Hitbox {
                damage: 9,
                offset: Vec3::new(0.0, 0.0, 0.0),
                radius: 220.0 / 2.0,
                angle: 70,
                kb_scale: 120,
                kb_weight: 0,
                kb_base: 0,
            },
            5.0,
            12.0,
        ),
        ActiveHitbox::new(
            Hitbox {
                damage: 9,
                offset: Vec3::new(0.0, 0.0, 0.0),
                radius: 250.0 / 2.0,
                angle: 70,
                kb_scale: 120,
                kb_weight: 0,
                kb_base: 0,
            },
            5.0,
            12.0,
        ),
    ],
    length_frames: 12.0,
    landing_lag_percent: None,
};

/// Mario's down aerial — `dMarioMainMotion_AttackAirD`. A literal `-70°`
/// downward launch, and a real *pulsing* hitbox: `MakeAttackColl` at frame
/// 10, then `LoopBegin(7) { Wait(2); ClearAttackCollAll(); Wait(1);
/// RefreshAttackCollID }`, so it is on for 2 frames and off for 1, eight
/// times over (the initial hit plus seven refreshes), before a final
/// `Wait(2)` and `ClearAttackCollAll`. Windows: `[10,12)`, `[13,15)`,
/// `[16,18)`, `[19,21)`, `[22,24)`, `[25,27)`, `[28,30)`, `[31,33)` — total
/// `10 + 8×3 + 2 = 33`. Not simplified to one wide window: landing during a
/// 1-frame gap between pulses is a real way to avoid this hitbox in the
/// original, and collapsing the gaps would take that away.
pub static MARIO_AIR_LW: MoveData = MoveData {
    hitboxes: &[
        ActiveHitbox::new(mario_air_lw_hitbox(-30.0, 45.0), 10.0, 12.0),
        ActiveHitbox::new(mario_air_lw_hitbox(50.0, 30.0), 10.0, 12.0),
        ActiveHitbox::new(mario_air_lw_hitbox(-30.0, 45.0), 13.0, 15.0),
        ActiveHitbox::new(mario_air_lw_hitbox(50.0, 30.0), 13.0, 15.0),
        ActiveHitbox::new(mario_air_lw_hitbox(-30.0, 45.0), 16.0, 18.0),
        ActiveHitbox::new(mario_air_lw_hitbox(50.0, 30.0), 16.0, 18.0),
        ActiveHitbox::new(mario_air_lw_hitbox(-30.0, 45.0), 19.0, 21.0),
        ActiveHitbox::new(mario_air_lw_hitbox(50.0, 30.0), 19.0, 21.0),
        ActiveHitbox::new(mario_air_lw_hitbox(-30.0, 45.0), 22.0, 24.0),
        ActiveHitbox::new(mario_air_lw_hitbox(50.0, 30.0), 22.0, 24.0),
        ActiveHitbox::new(mario_air_lw_hitbox(-30.0, 45.0), 25.0, 27.0),
        ActiveHitbox::new(mario_air_lw_hitbox(50.0, 30.0), 25.0, 27.0),
        ActiveHitbox::new(mario_air_lw_hitbox(-30.0, 45.0), 28.0, 30.0),
        ActiveHitbox::new(mario_air_lw_hitbox(50.0, 30.0), 28.0, 30.0),
        ActiveHitbox::new(mario_air_lw_hitbox(-30.0, 45.0), 31.0, 33.0),
        ActiveHitbox::new(mario_air_lw_hitbox(50.0, 30.0), 31.0, 33.0),
    ],
    length_frames: 33.0,
    landing_lag_percent: None,
};

const fn mario_air_lw_hitbox(ox: f32, oy: f32) -> Hitbox {
    Hitbox {
        damage: 3,
        offset: Vec3::new(ox, oy, 0.0),
        radius: 350.0 / 2.0,
        angle: -70,
        kb_scale: 100,
        kb_weight: 30,
        kb_base: 0,
    }
}

/// Builds one of Mario's five forward-smash `MoveData`s
/// (`dMarioMainMotion_FSmashHigh`/`MidHigh`/`FSmash`/`MidLow`/`Low`). Same
/// macro-not-`const-fn` reason as [`mario_ftilt`]. `MidLow` and `Low` are
/// numerically identical in the original (`FSmashMidLow`'s and `FSmashLow`'s
/// `MakeAttackColl` calls have the same arguments) — not a transcription
/// mistake here, the decomp source really does repeat them.
/// `WaitAsync(4)` then `WaitAsync(16)` then two `MakeAttackColl`s,
/// `Wait(5)` then `ClearAttackCollAll` — total `4 + 16 + 5 = 25`.
macro_rules! mario_fsmash {
    ($damage:expr, $ox2:expr) => {
        MoveData {
            hitboxes: &[
                ActiveHitbox::new(
                    Hitbox {
                        damage: $damage,
                        offset: Vec3::new(0.0, 0.0, 0.0),
                        radius: 180.0 / 2.0,
                        angle: 361,
                        kb_scale: 100,
                        kb_weight: 0,
                        kb_base: 30,
                    },
                    20.0,
                    25.0,
                ),
                ActiveHitbox::new(
                    Hitbox {
                        damage: $damage,
                        offset: Vec3::new($ox2, 0.0, 0.0),
                        radius: 240.0 / 2.0,
                        angle: 361,
                        kb_scale: 100,
                        kb_weight: 0,
                        kb_base: 30,
                    },
                    20.0,
                    25.0,
                ),
            ],
            length_frames: 25.0,
            landing_lag_percent: None,
        }
    };
}
/// `dMarioMainMotion_FSmashHigh`.
pub static MARIO_FSMASH_HI: MoveData = mario_fsmash!(18, 60.0);
/// `dMarioMainMotion_FSmashMidHigh`.
pub static MARIO_FSMASH_HI_S: MoveData = mario_fsmash!(18, 50.0);
/// `dMarioMainMotion_FSmash`.
pub static MARIO_FSMASH: MoveData = mario_fsmash!(17, 50.0);
/// `dMarioMainMotion_FSmashMidLow`.
pub static MARIO_FSMASH_LOW_S: MoveData = mario_fsmash!(16, 50.0);
/// `dMarioMainMotion_FSmashLow`.
pub static MARIO_FSMASH_LOW: MoveData = mario_fsmash!(16, 50.0);

/// Mario's up smash — `dMarioMainMotion_USmash`. A literal `85°` launch
/// angle. `WaitAsync(7)` + `MakeAttackColl`, `Wait(4)` + `Wait(5)` then
/// `ClearAttackCollAll` — total `7 + 4 + 5 = 16`.
pub static MARIO_USMASH: MoveData = MoveData {
    hitboxes: &[ActiveHitbox::new(
        Hitbox {
            damage: 19,
            offset: Vec3::new(0.0, 100.0, 0.0),
            radius: 380.0 / 2.0,
            angle: 85,
            kb_scale: 120,
            kb_weight: 0,
            kb_base: 26,
        },
        7.0,
        16.0,
    )],
    length_frames: 16.0,
    landing_lag_percent: None,
};

/// Mario's down smash — `dMarioMainMotion_DSmash`. Four `MakeAttackColl`s:
/// the same two boxes on `jid` 25 and again on `jid` 20 (front and back
/// foot). `WaitAsync(4)` then `WaitAsync(8)` then the four boxes,
/// `Wait(15)` then `Wait(7)` then `ClearAttackCollAll` — total
/// `4 + 8 + 15 + 7 = 34`.
pub static MARIO_DSMASH: MoveData = MoveData {
    hitboxes: &[
        ActiveHitbox::new(
            Hitbox {
                damage: 17,
                offset: Vec3::new(0.0, 0.0, 20.0),
                radius: 170.0 / 2.0,
                angle: 361,
                kb_scale: 100,
                kb_weight: 0,
                kb_base: 20,
            },
            12.0,
            34.0,
        ),
        ActiveHitbox::new(
            Hitbox {
                damage: 17,
                offset: Vec3::new(120.0, 0.0, 50.0),
                radius: 210.0 / 2.0,
                angle: 361,
                kb_scale: 100,
                kb_weight: 0,
                kb_base: 20,
            },
            12.0,
            34.0,
        ),
        ActiveHitbox::new(
            Hitbox {
                damage: 17,
                offset: Vec3::new(0.0, 0.0, 20.0),
                radius: 170.0 / 2.0,
                angle: 361,
                kb_scale: 100,
                kb_weight: 0,
                kb_base: 20,
            },
            12.0,
            34.0,
        ),
        ActiveHitbox::new(
            Hitbox {
                damage: 17,
                offset: Vec3::new(120.0, 0.0, 50.0),
                radius: 210.0 / 2.0,
                angle: 361,
                kb_scale: 100,
                kb_weight: 0,
                kb_base: 20,
            },
            12.0,
            34.0,
        ),
    ],
    length_frames: 34.0,
    landing_lag_percent: None,
};

/// The attack data for `status`, under `kind` — every per-character motion
/// script's `MakeAttackColl` argument list, transcribed field-for-field (see
/// each `MoveData` constant's own doc comment for its source line). `None`
/// for a status/fighter whose motion data has not been ported. Fox's normal
/// attacks are in `fox_attack`; the fighter's extended statuses are still
/// being translated as part of the current `P2` batch.
pub fn move_data(
    kind: crate::fighter::FighterKind,
    status: AnyStatus,
) -> Option<&'static MoveData> {
    use crate::fighter::FighterKind;
    match (kind, status) {
        (FighterKind::Mario, AnyStatus::Common(Status::ThrowB)) => Some(&MARIO_THROW_B),
        (FighterKind::Fox, AnyStatus::Common(Status::ThrowB)) => Some(&FOX_THROW_B),
        (
            FighterKind::Donkey,
            AnyStatus::Donkey(
                crate::status::DonkeyStatus::SpecialNEnd
                | crate::status::DonkeyStatus::SpecialAirNEnd,
            ),
        ) => Some(&crate::donkey_attack::PUNCH),
        (
            FighterKind::Donkey,
            AnyStatus::Donkey(
                crate::status::DonkeyStatus::SpecialNFull
                | crate::status::DonkeyStatus::SpecialAirNFull,
            ),
        ) => Some(&crate::donkey_attack::PUNCH_FULL),
        (FighterKind::Donkey, AnyStatus::Donkey(crate::status::DonkeyStatus::SpecialHi)) => {
            Some(&crate::donkey_attack::SPIN_GROUND)
        }
        (FighterKind::Donkey, AnyStatus::Donkey(crate::status::DonkeyStatus::SpecialAirHi)) => {
            Some(&crate::donkey_attack::SPIN_AIR)
        }
        (FighterKind::Donkey, AnyStatus::Donkey(crate::status::DonkeyStatus::SpecialLwLoop)) => {
            Some(&crate::donkey_attack::HAND_SLAP)
        }
        (FighterKind::Donkey, AnyStatus::Common(status)) => match status {
            Status::Attack11 => Some(&crate::donkey_attack::JAB1),
            Status::Attack12 => Some(&crate::donkey_attack::JAB2),
            Status::AttackDash => Some(&crate::donkey_attack::DASH),
            Status::AttackS3Hi => Some(&crate::donkey_attack::FTILT_HI),
            Status::AttackS3 => Some(&crate::donkey_attack::FTILT),
            Status::AttackS3Lw => Some(&crate::donkey_attack::FTILT_LW),
            Status::AttackHi3 => Some(&crate::donkey_attack::UTILT),
            Status::AttackLw3 => Some(&crate::donkey_attack::DTILT),
            Status::AttackS4Hi => Some(&crate::donkey_attack::FSMASH_HI),
            Status::AttackS4HiS => Some(&crate::donkey_attack::FSMASH_HI_S),
            Status::AttackS4 => Some(&crate::donkey_attack::FSMASH),
            Status::AttackS4LwS => Some(&crate::donkey_attack::FSMASH_LW_S),
            Status::AttackS4Lw => Some(&crate::donkey_attack::FSMASH_LW),
            Status::AttackHi4 => Some(&crate::donkey_attack::USMASH),
            Status::AttackLw4 => Some(&crate::donkey_attack::DSMASH),
            Status::AttackAirN => Some(&crate::donkey_attack::AIR_N),
            Status::AttackAirF => Some(&crate::donkey_attack::AIR_F),
            Status::AttackAirB => Some(&crate::donkey_attack::AIR_B),
            Status::AttackAirHi => Some(&crate::donkey_attack::AIR_HI),
            Status::AttackAirLw => Some(&crate::donkey_attack::AIR_LW),
            _ => None,
        },
        (
            FighterKind::Fox,
            AnyStatus::Fox(FoxStatus::SpecialLwStart | FoxStatus::SpecialAirLwStart),
        ) => Some(&crate::fox_attack::FOX_REFLECTOR_START),
        (FighterKind::Fox, AnyStatus::Fox(FoxStatus::Attack100Loop)) => {
            Some(&crate::fox_attack::FOX_JABLOOP)
        }
        (FighterKind::Fox, AnyStatus::Common(Status::Attack11)) => {
            Some(&crate::fox_attack::FOX_JAB1)
        }
        (FighterKind::Fox, AnyStatus::Common(Status::Attack12)) => {
            Some(&crate::fox_attack::FOX_JAB2)
        }
        (FighterKind::Fox, AnyStatus::Common(Status::AttackDash)) => {
            Some(&crate::fox_attack::FOX_DASHATTACK)
        }
        (FighterKind::Fox, AnyStatus::Common(Status::AttackS3Hi)) => {
            Some(&crate::fox_attack::FOX_FTILTHIGH)
        }
        (FighterKind::Fox, AnyStatus::Common(Status::AttackS3HiS)) => {
            Some(&crate::fox_attack::FOX_FTILTMIDHIGH)
        }
        (FighterKind::Fox, AnyStatus::Common(Status::AttackS3)) => {
            Some(&crate::fox_attack::FOX_FTILT)
        }
        (FighterKind::Fox, AnyStatus::Common(Status::AttackS3LwS)) => {
            Some(&crate::fox_attack::FOX_FTILTMIDLOW)
        }
        (FighterKind::Fox, AnyStatus::Common(Status::AttackS3Lw)) => {
            Some(&crate::fox_attack::FOX_FTILTLOW)
        }
        (FighterKind::Fox, AnyStatus::Common(Status::AttackHi3)) => {
            Some(&crate::fox_attack::FOX_UTILT)
        }
        (FighterKind::Fox, AnyStatus::Common(Status::AttackLw3)) => {
            Some(&crate::fox_attack::FOX_DTILT)
        }
        (FighterKind::Fox, AnyStatus::Common(Status::AttackS4)) => {
            Some(&crate::fox_attack::FOX_FSMASH)
        }
        (FighterKind::Fox, AnyStatus::Common(Status::AttackHi4)) => {
            Some(&crate::fox_attack::FOX_USMASH)
        }
        (FighterKind::Fox, AnyStatus::Common(Status::AttackLw4)) => {
            Some(&crate::fox_attack::FOX_DSMASH)
        }
        (FighterKind::Fox, AnyStatus::Common(Status::AttackAirN)) => {
            Some(&crate::fox_attack::FOX_ATTACKAIRN)
        }
        (FighterKind::Fox, AnyStatus::Common(Status::AttackAirF)) => {
            Some(&crate::fox_attack::FOX_ATTACKAIRF)
        }
        (FighterKind::Fox, AnyStatus::Common(Status::AttackAirB)) => {
            Some(&crate::fox_attack::FOX_ATTACKAIRB)
        }
        (FighterKind::Fox, AnyStatus::Common(Status::AttackAirHi)) => {
            Some(&crate::fox_attack::FOX_ATTACKAIRU)
        }
        (FighterKind::Fox, AnyStatus::Common(Status::AttackAirLw)) => {
            Some(&crate::fox_attack::FOX_ATTACKAIRD)
        }
        (FighterKind::Mario, AnyStatus::Common(Status::Attack11)) => Some(&MARIO_JAB1),
        (FighterKind::Mario, AnyStatus::Common(Status::Attack12)) => Some(&MARIO_JAB2),
        (FighterKind::Mario, AnyStatus::Common(Status::AttackDash)) => Some(&MARIO_DASH_ATTACK),
        (FighterKind::Mario, AnyStatus::Common(Status::AttackS3Hi)) => Some(&MARIO_FTILT_HI),
        (FighterKind::Mario, AnyStatus::Common(Status::AttackS3)) => Some(&MARIO_FTILT),
        (FighterKind::Mario, AnyStatus::Common(Status::AttackS3Lw)) => Some(&MARIO_FTILT_LOW),
        (FighterKind::Mario, AnyStatus::Common(Status::AttackHi3)) => Some(&MARIO_UTILT),
        (FighterKind::Mario, AnyStatus::Common(Status::AttackLw3)) => Some(&MARIO_DTILT),
        (FighterKind::Mario, AnyStatus::Common(Status::AttackAirN)) => Some(&MARIO_AIR_N),
        (FighterKind::Mario, AnyStatus::Common(Status::AttackAirF)) => Some(&MARIO_AIR_F),
        (FighterKind::Mario, AnyStatus::Common(Status::AttackAirB)) => Some(&MARIO_AIR_B),
        (FighterKind::Mario, AnyStatus::Common(Status::AttackAirHi)) => Some(&MARIO_AIR_HI),
        (FighterKind::Mario, AnyStatus::Common(Status::AttackAirLw)) => Some(&MARIO_AIR_LW),
        (FighterKind::Mario, AnyStatus::Common(Status::AttackS4Hi)) => Some(&MARIO_FSMASH_HI),
        (FighterKind::Mario, AnyStatus::Common(Status::AttackS4HiS)) => Some(&MARIO_FSMASH_HI_S),
        (FighterKind::Mario, AnyStatus::Common(Status::AttackS4)) => Some(&MARIO_FSMASH),
        (FighterKind::Mario, AnyStatus::Common(Status::AttackS4LwS)) => Some(&MARIO_FSMASH_LOW_S),
        (FighterKind::Mario, AnyStatus::Common(Status::AttackS4Lw)) => Some(&MARIO_FSMASH_LOW),
        (FighterKind::Mario, AnyStatus::Common(Status::AttackHi4)) => Some(&MARIO_USMASH),
        (FighterKind::Mario, AnyStatus::Common(Status::AttackLw4)) => Some(&MARIO_DSMASH),
        (FighterKind::Mario, AnyStatus::Mario(MarioStatus::Attack13)) => Some(&MARIO_JAB3),
        (
            FighterKind::Mario,
            AnyStatus::Mario(MarioStatus::SpecialHi | MarioStatus::SpecialAirHi),
        ) => Some(&MARIO_SUPERJUMP),
        (FighterKind::Mario, AnyStatus::Mario(MarioStatus::SpecialLw)) => {
            Some(&MARIO_TORNADO_GROUND)
        }
        (FighterKind::Mario, AnyStatus::Mario(MarioStatus::SpecialAirLw)) => {
            Some(&MARIO_TORNADO_AIR)
        }
        _ => None,
    }
}

/// Mario's back throw — `dMarioMainMotion_ThrowB`,
/// `relocData/202_MarioMainMotion.c`. `Wait(4)`, `WaitAsync(10)`, `Wait(8)`,
/// then `MakeAttackColl(0, 0, 10, 10, 0, 0, 300, 120, 0, 0, 361, 80, 0, 3, 1,
/// 2, 0, 30)`; two `Wait(14)` loops reach `ClearAttackCollAll` at frame 46.
/// The swing only reaches bystanders: the held fighter is skipped
/// ([`apply_hit_from`]).
pub static MARIO_THROW_B: MoveData = MoveData {
    hitboxes: &[ActiveHitbox::new(
        Hitbox {
            damage: 10,
            offset: Vec3::new(120.0, 0.0, 0.0),
            radius: 300.0 / 2.0,
            angle: 361,
            kb_scale: 80,
            kb_weight: 0,
            kb_base: 30,
        },
        18.0,
        46.0,
    )],
    length_frames: 67.0,
    landing_lag_percent: None,
};

/// Fox's back throw — `dFoxMainMotion_ThrowB`,
/// `relocData/208_FoxMainMotion.c`. At `WaitAsync(11)`:
/// `MakeAttackColl(0, 0, 20, 10, 0, 0, 230, 140, 0, 0, 361, 90, 0, 3, 1, 2,
/// 1, 10)` and the same box at offset zero (`aid` 1); `WaitAsync(13)` and
/// `Wait(6)` reach `ClearAttackCollAll` at frame 19.
pub static FOX_THROW_B: MoveData = MoveData {
    hitboxes: &[
        ActiveHitbox::new(
            Hitbox {
                damage: 10,
                offset: Vec3::new(140.0, 0.0, 0.0),
                radius: 230.0 / 2.0,
                angle: 361,
                kb_scale: 90,
                kb_weight: 0,
                kb_base: 10,
            },
            11.0,
            19.0,
        ),
        ActiveHitbox::new(
            Hitbox {
                damage: 10,
                offset: Vec3::ZERO,
                radius: 230.0 / 2.0,
                angle: 361,
                kb_scale: 90,
                kb_weight: 0,
                kb_base: 10,
            },
            11.0,
            19.0,
        ),
    ],
    length_frames: 40.0,
    landing_lag_percent: None,
};

/// `FTCOMMON_DAMAGE_SAKURAI_*` — `ft/ftcommon.h`.
const SAKURAI_KNOCKBACK_LOW: f32 = 32.0;
const SAKURAI_ANGLE_LOW_GR_DEG: f32 = 0.0;
const SAKURAI_ANGLE_HIGH_GD_DEG: f32 = 42.5;
const SAKURAI_ANGLE_DEFAULT_AR_DEG: f32 = 43.0;
/// The motion-event angle sentinel meaning "use the Sakurai angle" rather
/// than a literal degree value.
const SAKURAI_ANGLE_SENTINEL: i32 = 361;

/// `ftCommonDamageGetKnockbackAngle` @ `ftcommondamage.c:285`.
///
/// A literal `angle_i` is used as-is. The sentinel `361` means: 43 degrees if
/// the target is airborne, flat (0 degrees) if grounded and the knockback is
/// below 32, otherwise a near-vertical jump from 0 to the 42.5-degree cap
/// within about a tenth of a unit of knockback past 32 — this is the original
/// formula's own coarseness, not a bug introduced here.
pub fn sakurai_angle_radians(angle_i: i32, target_airborne: bool, knockback: f32) -> f32 {
    if angle_i != SAKURAI_ANGLE_SENTINEL {
        return (angle_i as f32).to_radians();
    }
    if target_airborne {
        return SAKURAI_ANGLE_DEFAULT_AR_DEG.to_radians();
    }
    if knockback < SAKURAI_KNOCKBACK_LOW {
        return SAKURAI_ANGLE_LOW_GR_DEG.to_radians();
    }
    let mut deg =
        ((knockback - SAKURAI_KNOCKBACK_LOW) / 0.099998474) * SAKURAI_ANGLE_HIGH_GD_DEG + 1.0;
    if deg > SAKURAI_ANGLE_HIGH_GD_DEG {
        deg = SAKURAI_ANGLE_HIGH_GD_DEG;
    }
    deg.to_radians()
}

/// `ftParamGetCommonKnockback` @ `ftparam.c:1451` for a hitbox, with
/// `recent_damage == 0` (module docs).
pub fn common_knockback(
    defender_damage_percent: u16,
    hitbox: &Hitbox,
    defender_weight: f32,
    attack_handicap: u8,
    defend_handicap: u8,
) -> f32 {
    knockback(
        defender_damage_percent,
        0,
        hitbox.damage,
        hitbox.kb_weight,
        hitbox.kb_scale,
        hitbox.kb_base,
        defender_weight,
        attack_handicap,
        defend_handicap,
    )
}

/// `ftParamGetCommonKnockback` @ `ftparam.c:1451`. The damage ratio is
/// Training's `100` ([`crate::stale::DAMAGE_RATIO_DEFAULT`]); the handicaps
/// index `dFTCommonDataHandicapTable` ([`crate::stale::HANDICAP_TABLE`]).
/// Throws pass the throw's own damage as `recent_damage`.
#[allow(clippy::too_many_arguments)]
pub fn knockback(
    percent_damage: u16,
    recent_damage: i32,
    hit_damage: i32,
    kb_weight: i32,
    kb_scale: i32,
    kb_base: i32,
    weight: f32,
    attack_handicap: u8,
    defend_handicap: u8,
) -> f32 {
    let scale = kb_scale as f32 * 0.01;
    let base = if kb_weight != 0 {
        (((1.0 + (10.0 * kb_weight as f32 * 0.05)) * weight * 1.4) + 18.0) * scale + kb_base as f32
    } else {
        let damage_add = percent_damage as f32 + recent_damage as f32;
        let hit_damage = hit_damage as f32;
        ((((damage_add * 0.1) + (damage_add * hit_damage * 0.05)) * weight * 1.4) + 18.0) * scale
            + kb_base as f32
    };
    let knockback = crate::stale::apply_ratio_and_handicap(
        base,
        crate::stale::DAMAGE_RATIO_DEFAULT,
        attack_handicap,
        defend_handicap,
    );
    knockback.min(2500.0)
}

/// `ftCommonDamageInitDamageVars` @ `ftcommondamage.c:473`, for callers that
/// already know the knockback and direction (throws, grab escapes, the cargo
/// stagger). A forced `status_replace` counts as a tumble-level hit, as in
/// the original, so it always launches. The fighter turns to face `lr`.
/// The same simplifications as [`resolve_hit`] apply (middle damage column,
/// no ground-launch angle branch, no random `DamageFlyRoll`).
pub fn init_damage_vars(
    f: &mut Fighter,
    status_replace: Option<AnyStatus>,
    damage: i32,
    knockback: f32,
    angle_i: i32,
    lr: f32,
) {
    let airborne = !f.is_grounded();
    let angle = sakurai_angle_radians(angle_i, airborne, knockback);
    let (sin, cos) = sin_cos(angle);
    let hitstun_f = hitstun_frames(knockback);
    let level = if status_replace.is_some() {
        3
    } else {
        damage_level(hitstun_f)
    };
    f.facing = if lr > 0.0 {
        crate::fighter::Facing::Right
    } else {
        crate::fighter::Facing::Left
    };
    let status = status_replace.unwrap_or(damage_status(level, airborne, angle).into());
    if level == 3 {
        f.become_airborne();
    }
    f.damage = f.damage.saturating_add(damage.max(0) as u16);
    status::set_any_status(f, status, 0.0, StatusTiming::unknown());
    f.physics.vel_ground = Vec3::ZERO;
    f.physics.vel_air = Vec3::ZERO;
    f.physics.vel_knockback = Vec3::new(-cos * knockback * lr, sin * knockback, 0.0);
    f.hitstun = (hitstun_f as u16).max(1);
}

/// `ftParamGetHitStun` @ `ftparam.c:1505`.
pub fn hitstun_frames(knockback: f32) -> f32 {
    knockback / 1.875
}

/// `this_fp->damage_lr = (defender.x < attacker.x) ? +1 : -1` —
/// `ftmain.c:2860`. Which way the hit pushes: away from the attacker, by
/// relative position, not by the attacker's facing.
pub fn damage_lr(defender_pos: Vec3, attacker_pos: Vec3) -> f32 {
    if defender_pos.x < attacker_pos.x {
        1.0
    } else {
        -1.0
    }
}

/// `FTCOMMON_DAMAGE_LEVEL_HITSTUN_*` — `ft/ftcommon.h`. Which of the four
/// damage tiers (`DamageX1`/`X2`/`X3`/tumble) a hit's hitstun falls into —
/// `ftCommonDamageGetDamageLevel` @ `ftcommondamage.c:314`.
const DAMAGE_LEVEL_HITSTUN_LOW: f32 = 12.0;
const DAMAGE_LEVEL_HITSTUN_MID: f32 = 24.0;
const DAMAGE_LEVEL_HITSTUN_HIGH: f32 = 32.0;

/// `ftCommonDamageGetDamageLevel` @ `ftcommondamage.c:314`. `3` is the
/// "tumble" tier: the hit always launches the defender airborne regardless of
/// where it landed.
pub fn damage_level(hitstun: f32) -> u8 {
    if hitstun < DAMAGE_LEVEL_HITSTUN_LOW {
        0
    } else if hitstun < DAMAGE_LEVEL_HITSTUN_MID {
        1
    } else if hitstun < DAMAGE_LEVEL_HITSTUN_HIGH {
        2
    } else {
        3
    }
}

/// `FTCOMMON_DAMAGE_FIGHTER_FLYTOP_ANGLE_{LOW,HIGH}` — `ft/ftcommon.h`.
const FLYTOP_ANGLE_LOW: f32 = 1.221_730_6; // 70 degrees
const FLYTOP_ANGLE_HIGH: f32 = 1.919_862_2; // 110 degrees

/// `ftCommonDamageInitDamageVars`'s status-table lookup
/// (`ftcommondamage.c:473`), restricted to the `damage_index == N` ("hit the
/// middle of the target") column — no hit-location-relative Hi/Lw index
/// exists yet, so every hit reads as a middle hit — and dropping the
/// ground-hit-still-launches-airborne branch (see [`Status::is_grounded`]'s
/// docs). `DamageFlyRoll`'s random branch is not ported: it needs a shared
/// RNG source this module does not have, so a tumble always reads as
/// `DamageFlyN`/`DamageFlyTop`.
pub fn damage_status(level: u8, defender_was_airborne: bool, angle: f32) -> Status {
    if level == 3 {
        return if angle > FLYTOP_ANGLE_LOW && angle < FLYTOP_ANGLE_HIGH {
            Status::DamageFlyTop
        } else {
            Status::DamageFlyN
        };
    }
    let table = if defender_was_airborne {
        [Status::DamageAir1, Status::DamageAir2, Status::DamageAir3]
    } else {
        [Status::DamageN1, Status::DamageN2, Status::DamageN3]
    };
    table[level as usize]
}

/// The outcome of a hit landing, ready to apply to the defending
/// [`crate::fighter::Fighter`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HitResult {
    pub damage: i32,
    pub knockback_vel: Vec3,
    pub hitstun: u16,
    /// The Damage-family status the defender enters — see [`damage_status`].
    pub status: Status,
}

/// `ftCommonDamageInitDamageVars`'s knockback-vector construction
/// (`ftcommondamage.c:466`), for the grounded, non-launching case (module
/// docs: no ground/air damage-velocity split exists yet, so this always
/// writes to [`crate::physics::PhysicsState::vel_knockback`]).
#[allow(clippy::too_many_arguments)]
pub fn resolve_hit(
    hitbox: &Hitbox,
    attacker_pos: Vec3,
    defender_pos: Vec3,
    defender_damage_percent: u16,
    defender_weight: f32,
    defender_airborne: bool,
    attack_handicap: u8,
    defend_handicap: u8,
) -> HitResult {
    let lr = damage_lr(defender_pos, attacker_pos);
    let knockback = common_knockback(
        defender_damage_percent,
        hitbox,
        defender_weight,
        attack_handicap,
        defend_handicap,
    );
    let angle = sakurai_angle_radians(hitbox.angle, defender_airborne, knockback);
    let (sin, cos) = sin_cos(angle);
    let vel_x = cos * knockback;
    let vel_y = sin * knockback;
    let hitstun_f = hitstun_frames(knockback);
    let level = damage_level(hitstun_f);
    HitResult {
        damage: hitbox.damage,
        knockback_vel: Vec3::new(-vel_x * lr, vel_y, 0.0),
        hitstun: (hitstun_f as u16).max(1),
        status: damage_status(level, defender_airborne, angle),
    }
}

/// Whether `anim_frame` falls within `Jab1`'s active hitbox window.
pub fn jab1_hitbox_active(anim_frame: f32) -> bool {
    (MARIO_JAB1_HITBOX_START..MARIO_JAB1_HITBOX_END).contains(&anim_frame)
}

/// A swept-free sphere-vs-sphere overlap test —
/// `gmCollisionCheckAttackInFighterRange`'s shape, without the swept
/// previous-position term (module docs).
pub fn spheres_overlap(a_pos: Vec3, a_radius: f32, b_pos: Vec3, b_radius: f32) -> bool {
    let d = a_pos - b_pos;
    let r = a_radius + b_radius;
    d.length_squared() <= r * r
}

/// `jid` arguments of the US `MakeAttackColl` motion commands. The arrays are
/// in the order of each ported `MoveData`'s boxes; repeated pulse scripts use
/// the same joint pattern each cycle. The data comes from Mario/Fox/Donkey
/// `MainMotion.c`, not from the visual model's node order.
fn attack_joint(kind: crate::fighter::FighterKind, status: AnyStatus, index: usize) -> u8 {
    use crate::fighter::FighterKind::{Donkey, Fox, Mario};
    if kind == Donkey
        && matches!(
            status,
            AnyStatus::Donkey(
                crate::status::DonkeyStatus::SpecialHi | crate::status::DonkeyStatus::SpecialAirHi
            )
        )
    {
        return if index < 2 {
            [8, 14][index]
        } else {
            [8, 14, 21][(index - 2) % 3]
        };
    }
    let ids: &[u8] = match (kind, status) {
        (Mario, AnyStatus::Common(Status::ThrowB)) => &[10],
        (Fox, AnyStatus::Common(Status::ThrowB)) => &[20, 20],
        (Mario, AnyStatus::Common(Status::Attack11)) => &[10, 9],
        (Mario, AnyStatus::Common(Status::Attack12)) => &[16, 15],
        (Mario, AnyStatus::Mario(MarioStatus::Attack13)) => &[25, 25, 27],
        (Mario, AnyStatus::Common(Status::AttackDash)) => &[5],
        (Mario, AnyStatus::Common(Status::AttackS3Hi | Status::AttackS3 | Status::AttackS3Lw)) => {
            &[24, 25]
        }
        (Mario, AnyStatus::Common(Status::AttackHi3)) => &[14, 15],
        (Mario, AnyStatus::Common(Status::AttackLw3)) => &[24, 25],
        (
            Mario,
            AnyStatus::Common(
                Status::AttackS4Hi
                | Status::AttackS4HiS
                | Status::AttackS4
                | Status::AttackS4LwS
                | Status::AttackS4Lw,
            ),
        ) => &[14, 15],
        (Mario, AnyStatus::Common(Status::AttackHi4)) => &[12],
        (Mario, AnyStatus::Common(Status::AttackLw4)) => &[25, 25, 20, 20],
        (Mario, AnyStatus::Common(Status::AttackAirN)) => &[25, 20, 5],
        (
            Mario,
            AnyStatus::Common(Status::AttackAirF | Status::AttackAirB | Status::AttackAirLw),
        ) => &[25],
        (Mario, AnyStatus::Common(Status::AttackAirHi)) => &[25, 27],
        (Mario, AnyStatus::Mario(MarioStatus::SpecialHi | MarioStatus::SpecialAirHi)) => &[12, 15],
        (Mario, AnyStatus::Mario(MarioStatus::SpecialLw | MarioStatus::SpecialAirLw)) => &[0],

        (Fox, AnyStatus::Fox(FoxStatus::SpecialLwStart | FoxStatus::SpecialAirLwStart)) => &[0],
        (Fox, AnyStatus::Fox(FoxStatus::Attack100Loop)) => &[19, 20],
        (Fox, AnyStatus::Common(Status::Attack11)) => &[8],
        (Fox, AnyStatus::Common(Status::Attack12)) => &[14],
        (Fox, AnyStatus::Common(Status::AttackDash)) => &[20],
        (
            Fox,
            AnyStatus::Common(
                Status::AttackS3Hi
                | Status::AttackS3HiS
                | Status::AttackS3
                | Status::AttackS3LwS
                | Status::AttackS3Lw
                | Status::AttackHi3,
            ),
        ) => &[24, 25],
        (Fox, AnyStatus::Common(Status::AttackLw3)) => &[29],
        (Fox, AnyStatus::Common(Status::AttackS4)) => &[20],
        (Fox, AnyStatus::Common(Status::AttackHi4)) => &[25],
        (Fox, AnyStatus::Common(Status::AttackLw4)) => &[25, 20],
        (Fox, AnyStatus::Common(Status::AttackAirN)) => &[5, 20, 25],
        (Fox, AnyStatus::Common(Status::AttackAirF)) => &[25],
        (Fox, AnyStatus::Common(Status::AttackAirB)) => &[5, 25, 20],
        (Fox, AnyStatus::Common(Status::AttackAirHi)) => &[5, 25],
        (Fox, AnyStatus::Common(Status::AttackAirLw)) => &[20],

        (Donkey, AnyStatus::Common(Status::Attack11)) => &[9],
        (Donkey, AnyStatus::Common(Status::Attack12)) => &[15],
        (Donkey, AnyStatus::Common(Status::AttackDash)) => &[21],
        (Donkey, AnyStatus::Common(Status::AttackS3Hi | Status::AttackS3 | Status::AttackS3Lw)) => {
            &[14, 15, 14]
        }
        (Donkey, AnyStatus::Common(Status::AttackHi3)) => &[8, 9],
        (Donkey, AnyStatus::Common(Status::AttackLw3)) => &[14, 15],
        (
            Donkey,
            AnyStatus::Common(
                Status::AttackS4Hi
                | Status::AttackS4HiS
                | Status::AttackS4
                | Status::AttackS4LwS
                | Status::AttackS4Lw,
            ),
        ) => &[14, 15, 14],
        (Donkey, AnyStatus::Common(Status::AttackHi4)) => &[15, 9],
        (Donkey, AnyStatus::Common(Status::AttackLw4)) => &[26, 21],
        (Donkey, AnyStatus::Common(Status::AttackAirN)) => &[15, 9, 5],
        (Donkey, AnyStatus::Common(Status::AttackAirF)) => &[15, 14, 8],
        (Donkey, AnyStatus::Common(Status::AttackAirB)) => &[0],
        (Donkey, AnyStatus::Common(Status::AttackAirHi)) => &[8, 9],
        (Donkey, AnyStatus::Common(Status::AttackAirLw)) => &[26, 21],
        (
            Donkey,
            AnyStatus::Donkey(
                crate::status::DonkeyStatus::SpecialNEnd
                | crate::status::DonkeyStatus::SpecialAirNEnd,
            ),
        ) => &[14],
        (
            Donkey,
            AnyStatus::Donkey(
                crate::status::DonkeyStatus::SpecialNFull
                | crate::status::DonkeyStatus::SpecialAirNFull,
            ),
        ) => &[14, 14, 5],
        (Donkey, AnyStatus::Donkey(crate::status::DonkeyStatus::SpecialLwLoop)) => &[0],
        _ => unreachable!("ported move lacks source joint IDs"),
    };
    ids[index % ids.len()]
}

/// `F1` criterion 5: tests `attacker`'s active hitboxes against `defender` and
/// applies the hit. `hit_record` is the caller's per-target hit-suppression
/// state — the fixed-size Training stand-in for the original's per-attack
/// `GMAttackRecord` hit list. It is re-armed at a sourced
/// `ClearAttackCollAll`, including a clear/recreate boundary with no idle
/// animation frame between them.
///
/// `ftMainSearchFighterAttack` skips the defender's own catcher
/// (`other_gobj == this_fp->capture_gobj`), so a throw's attack boxes only
/// reach bystanders. A registered hit stales the box's damage by the
/// attacker's queue and records the move in it (`ftParamUpdateStaleQueue`).
pub fn apply_hit_from(attacker: &mut Fighter, defender: &mut Fighter, hit_record: &mut HitRecord) {
    let Some(move_data) = move_data(attacker.kind, attacker.status.status) else {
        *hit_record = HitRecord::default();
        return;
    };
    if defender.grab.capture == Some(attacker.port) {
        return;
    }
    let mut has_active_hitbox = false;
    for (index, active) in move_data
        .hitboxes
        .iter()
        .enumerate()
        .filter(|(_, h)| h.is_active(attacker.status.anim_frame))
    {
        has_active_hitbox = true;
        if hit_record.hit_generation == Some(active.hit_generation) {
            continue;
        }
        let mut hitbox = active.hitbox;
        if matches!(
            attacker.status.status,
            AnyStatus::Donkey(
                crate::status::DonkeyStatus::SpecialNEnd
                    | crate::status::DonkeyStatus::SpecialAirNEnd
            )
        ) {
            hitbox.damage += i32::from(attacker.donkey_special_n.attack_charge) * 2;
        }
        hitbox.damage = crate::stale::staled_damage(attacker, hitbox.damage);
        let joint = attack_joint(attacker.kind, attacker.status.status, index);
        let hitbox_pos = attacker.joint_world(joint, hitbox.offset);
        match apply_hitbox_at(&hitbox, hitbox_pos, attacker.handicap, defender) {
            HitOutcome::Missed => continue,
            outcome => {
                if outcome == HitOutcome::Damaged {
                    crate::stale::record_hit(attacker, defender.port);
                }
                hit_record.hit_generation = Some(active.hit_generation);
                return;
            }
        }
    }
    if !has_active_hitbox {
        // `ClearAttackCollAll` ends the current attack record in the source.
        // A later pulse in a multi-hit script is a new collision opportunity.
        *hit_record = HitRecord::default();
    }
}

/// What one hitbox did to a defender.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HitOutcome {
    /// No contact, or an invincible target: the box may still connect later.
    Missed,
    /// The shield took it.
    Shielded,
    /// Damage registered (`ftMainCheckGetUpdateDamage`).
    Damaged,
}

impl HitOutcome {
    pub fn registered(self) -> bool {
        self != HitOutcome::Missed
    }
}

/// `FTCOMMON_DAMAGE_CATCH_RELEASE_THRESHOLD`: a held fighter whose queued
/// damage reaches this is knocked out of the hold.
pub const CATCH_RELEASE_THRESHOLD: i32 = 6;

/// `ftCommonDamageCheckCaptureKeepHold` @ 0x80140EC0.
pub fn capture_keep_hold(damage_queue: i32) -> bool {
    damage_queue < CATCH_RELEASE_THRESHOLD
}

/// `ftParamGetCapturedDamage` @ 0x800EA40C: a held fighter takes half,
/// rounded up, then `damage_mul` (always `1.0` for the ported fighters).
pub fn captured_damage(defender: &Fighter, damage: i32) -> i32 {
    let mut damage = damage;
    if defender.grab.capture.is_some() {
        damage = (damage as f32 * 0.5 + 0.999) as i32;
    }
    (damage as f32 * 1.0 + 0.999) as i32
}

/// Applies an already-positioned hitbox to a defender. Fighter moves obtain
/// their position from a joint-relative offset; a weapon owns its world
/// position directly. Keeping collision resolution here preserves one damage,
/// shield, knockback, and invincibility path for both. `attack_handicap` is
/// the attacker's (a weapon's is its owner's).
///
/// An invincible target returns [`HitOutcome::Missed`], so a live weapon may
/// contact it again when its invincibility ends.
pub fn apply_hitbox_at(
    hitbox: &Hitbox,
    attacker_pos: Vec3,
    attack_handicap: u8,
    defender: &mut Fighter,
) -> HitOutcome {
    if !spheres_overlap(
        attacker_pos,
        hitbox.radius,
        defender.pos,
        MARIO_HURTBOX_RADIUS,
    ) {
        return HitOutcome::Missed;
    }
    if defender.invincible_frames > 0 {
        // `nGMHitStatusInvincible`: the hitbox simply does not register —
        // the hit record is left alone so the same active window
        // can still connect once invincibility ends.
        return HitOutcome::Missed;
    }
    if is_shielding(defender.status.status) {
        apply_shield_hit_at(hitbox, attacker_pos, defender);
        return HitOutcome::Shielded;
    }
    let damage = captured_damage(defender, hitbox.damage);
    let result = resolve_hit(
        hitbox,
        attacker_pos,
        defender.pos,
        defender.damage,
        defender.attributes.weight,
        !defender.is_grounded(),
        attack_handicap,
        defender.handicap,
    );
    if defender.kind == crate::fighter::FighterKind::Donkey {
        defender.donkey_special_n.charge_level = 0;
    }
    if defender.grab.capture.is_some() {
        // `ftCommonDamageUpdateMain`'s `capture_gobj` branch. The percent is
        // always added (`ftParamUpdateDamage(fp, fp->damage_queue)`).
        defender.damage = defender.damage.saturating_add(damage as u16);
        if capture_keep_hold(damage) {
            // The hold survives; only the catcher's hitlag (not ported)
            // and the colour animation react.
            return HitOutcome::Damaged;
        }
        // `ftCommonThrownDecideFighterLoseGrip(catcher, held)`, then the
        // catcher's `ftCommonThrownSetStatusNoDamageRelease` (delivered by
        // `grab::exchange`) and this fighter's normal damage status below.
        crate::grab::release_on_capture_hit(defender);
        enter_damage(defender, &result);
        return HitOutcome::Damaged;
    }
    if defender.grab.catch.is_some() {
        // `ftCommonDamageSetDamageStatus`'s `catch_gobj` branch: the cargo
        // stance absorbs anything below a tumble; otherwise the held fighter
        // is dropped with the throw descriptor's `[1]` knockback.
        let knockback = common_knockback(
            defender.damage,
            hitbox,
            defender.attributes.weight,
            attack_handicap,
            defender.handicap,
        );
        if crate::grab::cargo_resists(defender, knockback) {
            defender.damage = defender.damage.saturating_add(damage as u16);
            let lr = damage_lr(defender.pos, attacker_pos);
            crate::grab::set_donkey_throwf_damage(defender, knockback, hitbox.angle, lr);
            return HitOutcome::Damaged;
        }
        crate::grab::release_on_hit(defender);
    }
    defender.damage = defender.damage.saturating_add(damage as u16);
    enter_damage(defender, &result);
    HitOutcome::Damaged
}

/// `ftCommonDamageInitDamageVars` @ `ftcommondamage.c:557` zeroes the
/// fighter's normal ground/air velocity outright before writing the
/// knockback vector — a hit fully overrides existing movement rather than
/// adding to it. `status::set_status` runs first because it is what moves
/// `vel_ground` into `vel_air` on a ground-to-air transition, and that
/// transferred value must not survive the zeroing below.
fn enter_damage(defender: &mut Fighter, result: &HitResult) {
    status::set_status(defender, result.status, 0.0, StatusTiming::unknown());
    defender.physics.vel_ground = Vec3::ZERO;
    defender.physics.vel_air = Vec3::ZERO;
    defender.physics.vel_knockback = result.knockback_vel;
    defender.hitstun = result.hitstun;
}

/// Whether a hit landing on this status should be redirected into
/// [`apply_shield_hit`] rather than the normal Damage-family path — the
/// statuses `ftMainUpdateShieldStatFighter`'s caller treats as "currently
/// shielding" (`ftmain.c`'s hit-search gates a shield hit on `fp->is_shield`,
/// which these three statuses hold for the whole time they are active).
pub fn is_shielding(status: AnyStatus) -> bool {
    matches!(
        status,
        AnyStatus::Common(Status::GuardOn | Status::Guard | Status::GuardSetOff)
    )
}

/// `ftMainUpdateShieldStatFighter` @ `ftmain.c:2059`, reduced to the
/// single-hit case (module docs: no `shield_damage_total` multi-hit
/// accumulation) — a hit landing on a shield deals no damage/knockback/
/// hitstun at all, only shield health loss and a `GuardSetOff` pushback.
pub fn apply_shield_hit(hitbox: &Hitbox, attacker: &Fighter, defender: &mut Fighter) {
    apply_shield_hit_at(hitbox, attacker.pos, defender);
}

/// [`apply_shield_hit`] for a world-space attack source such as a weapon.
pub fn apply_shield_hit_at(hitbox: &Hitbox, attacker_pos: Vec3, defender: &mut Fighter) {
    let shield_lr = damage_lr(defender.pos, attacker_pos);
    status::set_guard_set_off(defender, hitbox.damage as f32, shield_lr);
    defender.guard.shield_health -= hitbox.damage as f32;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Mario's jab at 0% is the reference case: `dMarioMain_attr.weight ==
    /// 1.0`, so `common_knockback`'s weight term drops out and the whole
    /// formula reduces to arithmetic anyone can check by hand.
    #[test]
    fn jab_knockback_at_zero_percent_matches_the_formula_by_hand() {
        // ((0*1.4)+18) * (50*0.01) + 8 == 18*0.5+8 == 17.0
        let kb = common_knockback(0, &MARIO_JAB1_HITBOX, 1.0, 8, 8);
        assert_eq!(kb, 17.0);
    }

    #[test]
    fn hitstun_divides_knockback_by_1_875() {
        assert_eq!(hitstun_frames(17.0), 17.0 / 1.875);
    }

    /// 17.0 knockback is below `SAKURAI_KNOCKBACK_LOW` (32.0), so a grounded
    /// target is launched dead flat — the real "jabs don't launch you" feel.
    #[test]
    fn low_knockback_grounded_sakurai_angle_is_flat() {
        let angle = sakurai_angle_radians(361, false, 17.0);
        assert_eq!(angle, 0.0);
    }

    #[test]
    fn airborne_sakurai_angle_is_43_degrees() {
        let angle = sakurai_angle_radians(361, true, 17.0);
        assert!((angle - 43.0f32.to_radians()).abs() < 1e-6);
    }

    #[test]
    fn a_literal_angle_bypasses_sakurai_entirely() {
        let angle = sakurai_angle_radians(90, false, 5.0);
        assert!((angle - 90.0f32.to_radians()).abs() < 1e-6);
    }

    #[test]
    fn high_knockback_grounded_sakurai_angle_saturates_near_instantly() {
        // The original's own coarseness (module docs): a hair past 32
        // knockback already reads as the 42.5-degree cap.
        let angle = sakurai_angle_radians(361, false, 33.0);
        assert!((angle - SAKURAI_ANGLE_HIGH_GD_DEG.to_radians()).abs() < 1e-6);
    }

    #[test]
    fn defender_left_of_attacker_is_pushed_further_left() {
        let lr = damage_lr(Vec3::new(-10.0, 0.0, 0.0), Vec3::new(0.0, 0.0, 0.0));
        assert_eq!(lr, 1.0);
    }

    #[test]
    fn defender_right_of_attacker_is_pushed_further_right() {
        let lr = damage_lr(Vec3::new(10.0, 0.0, 0.0), Vec3::new(0.0, 0.0, 0.0));
        assert_eq!(lr, -1.0);
    }

    #[test]
    fn resolve_hit_pushes_the_defender_away_from_the_attacker() {
        let attacker = Vec3::new(0.0, 0.0, 0.0);
        let defender = Vec3::new(10.0, 0.0, 0.0);
        let result = resolve_hit(&MARIO_JAB1_HITBOX, attacker, defender, 0, 1.0, false, 8, 8);
        assert_eq!(result.damage, 2);
        // Flat, grounded, low knockback: pure +X push away from the attacker.
        assert!(result.knockback_vel.x > 0.0);
        assert_eq!(result.knockback_vel.y, 0.0);
        assert_eq!(result.hitstun, 9); // floor(17.0 / 1.875) == 9
                                       // hitstun 9 < DAMAGE_LEVEL_HITSTUN_LOW (12): level 0, grounded -> N1.
        assert_eq!(result.status, Status::DamageN1);
    }

    #[test]
    fn hitbox_window_is_two_frames_starting_at_frame_two() {
        assert!(!jab1_hitbox_active(1.99));
        assert!(jab1_hitbox_active(2.0));
        assert!(jab1_hitbox_active(3.99));
        assert!(!jab1_hitbox_active(4.0));
    }

    #[test]
    fn jab_uses_the_posed_hand_joint() {
        use crate::fighter::{FighterKind, JointTransform};
        let mut attacker = Fighter::new(FighterKind::Mario, 0, 3);
        let mut defender = Fighter::new(FighterKind::Mario, 1, 3);
        attacker.status.status = Status::Attack11.into();
        attacker.status.anim_frame = 2.0;
        attacker.joint_transforms[10] = Some(JointTransform {
            axes: [
                Vec3::new(1.0, 0.0, 0.0),
                Vec3::new(0.0, 1.0, 0.0),
                Vec3::new(0.0, 0.0, 1.0),
            ],
            origin: Vec3::new(500.0, 0.0, 0.0),
        });
        defender.pos = Vec3::new(500.0, 0.0, 0.0);
        let mut record = HitRecord::default();
        apply_hit_from(&mut attacker, &mut defender, &mut record);
        assert_eq!(defender.damage, 2);
    }

    #[test]
    fn spheres_overlap_at_exactly_the_combined_radius() {
        let a = Vec3::new(0.0, 0.0, 0.0);
        let b = Vec3::new(10.0, 0.0, 0.0);
        assert!(spheres_overlap(a, 5.0, b, 5.0));
        assert!(!spheres_overlap(a, 4.0, b, 5.0));
    }

    #[test]
    fn damage_level_thresholds_match_ftcommon_h() {
        assert_eq!(damage_level(11.999), 0);
        assert_eq!(damage_level(12.0), 1);
        assert_eq!(damage_level(23.999), 1);
        assert_eq!(damage_level(24.0), 2);
        assert_eq!(damage_level(31.999), 2);
        assert_eq!(damage_level(32.0), 3);
    }

    #[test]
    fn damage_status_picks_the_grounded_or_airborne_table_by_prior_situation() {
        assert_eq!(damage_status(0, false, 0.0), Status::DamageN1);
        assert_eq!(damage_status(1, false, 0.0), Status::DamageN2);
        assert_eq!(damage_status(2, false, 0.0), Status::DamageN3);
        assert_eq!(damage_status(0, true, 0.0), Status::DamageAir1);
        assert_eq!(damage_status(2, true, 0.0), Status::DamageAir3);
    }

    #[test]
    fn damage_status_tumble_is_flytop_only_within_the_near_vertical_window() {
        // Level 3 always tumbles airborne, regardless of prior situation.
        assert_eq!(damage_status(3, false, 0.0), Status::DamageFlyN);
        assert_eq!(
            damage_status(3, true, 90.0f32.to_radians()),
            Status::DamageFlyTop
        );
        assert_eq!(
            damage_status(3, true, 69.0f32.to_radians()),
            Status::DamageFlyN
        );
        assert_eq!(
            damage_status(3, true, 111.0f32.to_radians()),
            Status::DamageFlyN
        );
    }

    /// End-to-end through `apply_hit_from`: a grounded jab at 0% enters
    /// `DamageN1`, not just a bare knockback push (module docs' formerly-open
    /// "no Damage status" gap).
    #[test]
    fn a_landed_jab_puts_the_defender_into_a_damage_status() {
        let mut attacker = Fighter::new(crate::fighter::FighterKind::Mario, 0, 3);
        let mut defender = Fighter::new(crate::fighter::FighterKind::Mario, 1, 3);
        attacker.pos = Vec3::new(0.0, 0.0, 0.0);
        defender.pos = Vec3::new(10.0, 0.0, 0.0);
        defender.situation = crate::fighter::Situation::Ground;
        status::set_status(
            &mut attacker,
            Status::Attack11,
            3.0,
            StatusTiming::unknown(),
        );

        let mut hit_record = HitRecord::default();
        apply_hit_from(&mut attacker, &mut defender, &mut hit_record);

        assert_eq!(defender.status.status, Status::DamageN1);
        assert!(hit_record.hit_generation.is_some());
        assert!(defender.hitstun > 0);

        // Hitstun running out returns the defender to Wait.
        for _ in 0..defender.hitstun {
            defender.tick_timers();
        }
        crate::status::update(&mut defender);
        assert_eq!(defender.status.status, Status::Wait);
    }

    /// A jab landing on a shielding defender blocks entirely: no damage, no
    /// hitstun, just shield-health loss and a `GuardSetOff` pushback —
    /// `is_shielding`/`apply_shield_hit`'s module docs.
    #[test]
    fn a_jab_landing_on_a_shield_pushes_back_instead_of_damaging() {
        let mut attacker = Fighter::new(crate::fighter::FighterKind::Mario, 0, 3);
        let mut defender = Fighter::new(crate::fighter::FighterKind::Mario, 1, 3);
        attacker.pos = Vec3::new(0.0, 0.0, 0.0);
        defender.pos = Vec3::new(10.0, 0.0, 0.0);
        defender.situation = crate::fighter::Situation::Ground;
        status::set_status(&mut defender, Status::Guard, 0.0, StatusTiming::unknown());
        let starting_health = defender.guard.shield_health;
        status::set_status(
            &mut attacker,
            Status::Attack11,
            3.0,
            StatusTiming::unknown(),
        );

        let mut hit_record = HitRecord::default();
        apply_hit_from(&mut attacker, &mut defender, &mut hit_record);

        assert_eq!(defender.status.status, Status::GuardSetOff);
        assert_eq!(defender.damage, 0);
        assert_eq!(defender.hitstun, 0);
        assert_eq!(
            defender.guard.shield_health,
            starting_health - MARIO_JAB1_HITBOX.damage as f32
        );
        assert_ne!(defender.physics.vel_ground.x, 0.0);
    }

    /// `nGMHitStatusInvincible`: a post-respawn invincible defender takes no
    /// hit at all, not even the shield-block path.
    #[test]
    fn an_invincible_defender_is_not_hit() {
        let mut attacker = Fighter::new(crate::fighter::FighterKind::Mario, 0, 3);
        let mut defender = Fighter::new(crate::fighter::FighterKind::Mario, 1, 3);
        attacker.pos = Vec3::new(0.0, 0.0, 0.0);
        defender.pos = Vec3::new(10.0, 0.0, 0.0);
        defender.situation = crate::fighter::Situation::Ground;
        defender.invincible_frames = 10;
        status::set_status(
            &mut attacker,
            Status::Attack11,
            3.0,
            StatusTiming::unknown(),
        );

        let mut hit_record = HitRecord::default();
        apply_hit_from(&mut attacker, &mut defender, &mut hit_record);

        assert_eq!(defender.status.status, Status::Wait);
        assert_eq!(defender.damage, 0);
        assert!(hit_record.hit_generation.is_none());
    }

    #[test]
    fn move_data_is_none_for_an_unported_fighter_or_status() {
        assert!(move_data(crate::fighter::FighterKind::Fox, Status::HammerWait.into()).is_none());
        assert!(move_data(
            crate::fighter::FighterKind::Mario,
            Status::HammerWait.into()
        )
        .is_none());
    }

    #[test]
    fn fox_reflector_startup_hitbox_expires_after_two_frames() {
        for status in [FoxStatus::SpecialLwStart, FoxStatus::SpecialAirLwStart] {
            let move_data = move_data(crate::fighter::FighterKind::Fox, AnyStatus::Fox(status))
                .expect("Reflector startup has a sourced hitbox");
            assert_eq!(move_data.hitboxes.len(), 1);
            assert_eq!(move_data.hitboxes[0].hitbox.damage, 5);
            assert_eq!(move_data.hitboxes[0].hitbox.radius, 180.0);
            assert!(move_data.hitboxes[0].is_active(1.0));
            assert!(!move_data.hitboxes[0].is_active(2.0));
        }
    }

    #[test]
    fn fox_down_air_rearms_each_source_pulse() {
        let data = move_data(crate::fighter::FighterKind::Fox, Status::AttackAirLw.into())
            .expect("Fox down air is ported");
        assert_eq!(data.hitboxes.len(), 14);
        assert_eq!(data.length_frames, 24.0);
        for (pulse, pair) in data.hitboxes.as_chunks::<2>().0.iter().enumerate() {
            assert_eq!(pair[0].hit_generation, pulse as u8);
            assert_eq!(pair[1].hit_generation, pulse as u8);
            assert_eq!(pair[0].start, 4.0 + 3.0 * pulse as f32);
            assert_eq!(pair[0].end, 6.0 + 3.0 * pulse as f32);
        }
    }

    #[test]
    fn super_jump_motion_script_has_its_real_open_close_and_finish_windows() {
        let data = move_data(
            crate::fighter::FighterKind::Mario,
            AnyStatus::Mario(MarioStatus::SpecialAirHi),
        )
        .expect("Mario Super Jump Punch has sourced motion data");
        assert!(data
            .hitboxes
            .iter()
            .any(|h| h.is_active(2.0) && h.hitbox.damage == 5));
        assert!(!data.hitboxes.iter().any(|h| h.is_active(3.0)));
        assert!(data
            .hitboxes
            .iter()
            .any(|h| h.is_active(9.0) && h.hitbox.damage == 1));
        assert!(!data
            .hitboxes
            .iter()
            .any(|h| h.is_active(25.0) && h.hitbox.damage == 1));
        assert!(data
            .hitboxes
            .iter()
            .any(|h| h.is_active(25.0) && h.hitbox.damage == 3));
        assert_eq!(
            data.length_frames,
            crate::status::MARIO_SUPERJUMP_LENGTH_FRAMES
        );
    }

    /// Every `SuperJumpPunch` loop body clears its old attack collisions
    /// before recreating them. The windows meet at their frame boundary, so a
    /// plain "currently hit" bool never observes an inactive frame to reset.
    #[test]
    fn super_jump_clear_rearms_the_next_coin_hit_generation() {
        let mut attacker = Fighter::new(crate::fighter::FighterKind::Mario, 0, 3);
        let mut defender = Fighter::new(crate::fighter::FighterKind::Mario, 1, 3);
        attacker.pos = Vec3::ZERO;
        attacker.status.status = AnyStatus::Mario(MarioStatus::SpecialAirHi);
        defender.pos = Vec3::new(0.0, 0.0, 60.0);

        let mut hit_record = HitRecord::default();
        attacker.status.anim_frame = 9.0;
        apply_hit_from(&mut attacker, &mut defender, &mut hit_record);
        assert_eq!(defender.damage, 1);
        assert_eq!(hit_record.hit_generation, Some(1));

        // Still inside the same `MakeAttackColl` lifetime: one target once.
        attacker.status.anim_frame = 10.0;
        apply_hit_from(&mut attacker, &mut defender, &mut hit_record);
        assert_eq!(defender.damage, 1);

        // Frame 11 is the immediately recreated next loop collision, not a
        // gap, and is therefore a distinct legal hit.
        attacker.status.anim_frame = 11.0;
        apply_hit_from(&mut attacker, &mut defender, &mut hit_record);
        assert_eq!(defender.damage, 2);
        assert_eq!(hit_record.hit_generation, Some(2));
    }

    #[test]
    fn tornado_motion_scripts_keep_their_thirteen_one_frame_pulses() {
        let ground = move_data(
            crate::fighter::FighterKind::Mario,
            AnyStatus::Mario(MarioStatus::SpecialLw),
        )
        .unwrap();
        let air = move_data(
            crate::fighter::FighterKind::Mario,
            AnyStatus::Mario(MarioStatus::SpecialAirLw),
        )
        .unwrap();
        assert_eq!(ground.hitboxes.len(), 31);
        assert_eq!(air.hitboxes.len(), 60);
        assert!(ground.hitboxes.iter().any(|h| h.is_active(4.0)));
        assert!(!ground.hitboxes.iter().any(|h| h.is_active(5.0)));
        assert!(ground.hitboxes.iter().any(|h| h.is_active(40.0)));
        assert!(ground.hitboxes.iter().any(|h| h.is_active(43.0)));
        assert!(air.hitboxes.iter().any(|h| h.is_active(46.0)));
        assert!(!air.hitboxes.iter().any(|h| h.is_active(47.0)));
    }

    #[test]
    fn tornado_clear_gap_rearms_its_next_pulse() {
        let mut attacker = Fighter::new(crate::fighter::FighterKind::Mario, 0, 3);
        let mut defender = Fighter::new(crate::fighter::FighterKind::Mario, 1, 3);
        attacker.status.status = AnyStatus::Mario(MarioStatus::SpecialLw);
        // The loop's side hitbox at frame 4 is rooted at this exact
        // authored offset; the root hurtbox approximation then exercises the
        // same Training bridge as the live dummy.
        defender.pos = Vec3::new(150.0, 280.0, 0.0);

        let mut hit_record = HitRecord::default();
        attacker.status.anim_frame = 4.0;
        apply_hit_from(&mut attacker, &mut defender, &mut hit_record);
        assert_eq!(defender.damage, 1);

        attacker.status.anim_frame = 5.0;
        apply_hit_from(&mut attacker, &mut defender, &mut hit_record);
        assert_eq!(hit_record, HitRecord::default());

        attacker.status.anim_frame = 7.0;
        apply_hit_from(&mut attacker, &mut defender, &mut hit_record);
        assert_eq!(defender.damage, 2);
    }

    /// `DashAttack`'s single hitbox slot gets weaker after frame 11 —
    /// `MARIO_DASH_ATTACK`'s own doc comment.
    #[test]
    fn dash_attack_is_stronger_in_its_first_window_than_its_second() {
        let sweet = &MARIO_DASH_ATTACK.hitboxes[0];
        let sour = &MARIO_DASH_ATTACK.hitboxes[1];
        assert_eq!(sweet.hitbox.damage, 12);
        assert_eq!(sweet.hitbox.kb_base, 16);
        assert_eq!(sour.hitbox.damage, 10);
        assert_eq!(sour.hitbox.kb_base, 10);
        assert!(sweet.is_active(10.0));
        assert!(!sour.is_active(10.0));
        assert!(!sweet.is_active(15.0));
        assert!(sour.is_active(15.0));
    }

    #[test]
    fn a_sourspot_replacement_without_clear_does_not_rearm_a_target() {
        let mut attacker = Fighter::new(crate::fighter::FighterKind::Mario, 0, 3);
        let mut defender = Fighter::new(crate::fighter::FighterKind::Mario, 1, 3);
        attacker.pos = Vec3::ZERO;
        attacker.status.status = Status::AttackDash.into();
        defender.pos = Vec3::new(40.0, 0.0, 0.0);

        let mut hit_record = HitRecord::default();
        attacker.status.anim_frame = 10.0;
        apply_hit_from(&mut attacker, &mut defender, &mut hit_record);
        assert_eq!(defender.damage, 12);

        attacker.status.anim_frame = 15.0;
        apply_hit_from(&mut attacker, &mut defender, &mut hit_record);
        assert_eq!(defender.damage, 12);
    }

    /// A forward tilt's second hitbox reaches further out along the swing
    /// (`ox = 90`, vs. the first's `20`) — mirrored by facing, since it is a
    /// joint-space offset in the original, not a world-space one.
    #[test]
    fn a_forward_tilts_far_hitbox_mirrors_with_facing() {
        let mut attacker = Fighter::new(crate::fighter::FighterKind::Mario, 0, 3);
        let mut defender = Fighter::new(crate::fighter::FighterKind::Mario, 1, 3);
        attacker.pos = Vec3::ZERO;
        attacker.facing = crate::fighter::Facing::Left;
        defender.pos = Vec3::new(-90.0, 0.0, 0.0);
        defender.situation = crate::fighter::Situation::Ground;
        status::set_status(
            &mut attacker,
            Status::AttackS3,
            10.0,
            StatusTiming::unknown(),
        );

        let mut hit_record = HitRecord::default();
        apply_hit_from(&mut attacker, &mut defender, &mut hit_record);

        // Only reachable if the offset flipped to -90 with facing; at +90 the
        // defender at x=-90 would be 180 units away, past even the 115-unit
        // far-hitbox radius.
        assert!(hit_record.hit_generation.is_some());
        assert_eq!(defender.damage, 13);
    }

    /// `AttackAirD`'s pulsing hitbox is on 2 frames, off 1, eight times —
    /// `MARIO_AIR_LW`'s own doc comment. Landing in a gap is a real way to
    /// dodge it, so the windows must not merge into one wide range.
    #[test]
    fn down_aerial_pulses_on_and_off_rather_than_staying_active() {
        // Boundaries of the first two windows: [10,12) on, [12,13) off, [13,15) on.
        assert!(MARIO_AIR_LW.hitboxes.iter().any(|h| h.is_active(10.0)));
        assert!(MARIO_AIR_LW.hitboxes.iter().any(|h| h.is_active(11.9)));
        assert!(!MARIO_AIR_LW.hitboxes.iter().any(|h| h.is_active(12.0)));
        assert!(!MARIO_AIR_LW.hitboxes.iter().any(|h| h.is_active(12.9)));
        assert!(MARIO_AIR_LW.hitboxes.iter().any(|h| h.is_active(13.0)));
        // Last window ends exactly at the move's own length.
        assert!(MARIO_AIR_LW.hitboxes.iter().any(|h| h.is_active(32.9)));
        assert!(!MARIO_AIR_LW.hitboxes.iter().any(|h| h.is_active(33.0)));
    }

    /// Neutral aerial's three simultaneous hitboxes all weaken together
    /// after frame 11 — `MARIO_AIR_N`'s own doc comment.
    #[test]
    fn neutral_aerial_has_three_hitboxes_that_weaken_together() {
        let strong: Vec<_> = MARIO_AIR_N
            .hitboxes
            .iter()
            .filter(|h| h.is_active(5.0))
            .collect();
        let weak: Vec<_> = MARIO_AIR_N
            .hitboxes
            .iter()
            .filter(|h| h.is_active(20.0))
            .collect();
        assert_eq!(strong.len(), 3); // `jid` 25, 20 and 5 share a window
        assert_eq!(weak.len(), 3);
        assert!(strong.iter().all(|h| h.hitbox.damage == 14));
        assert!(weak.iter().all(|h| h.hitbox.damage == 11));
    }

    /// RE-332's condensed boxes: every source `MakeAttackColl` is present
    /// and reads its own `jid`.
    #[test]
    fn same_valued_boxes_keep_their_own_joints() {
        use crate::fighter::FighterKind::{Fox, Mario};
        let joints = |kind, status: Status| {
            let data = move_data(kind, status.into()).unwrap();
            (0..data.hitboxes.len())
                .map(|i| attack_joint(kind, status.into(), i))
                .collect::<Vec<_>>()
        };
        assert_eq!(joints(Mario, Status::AttackLw4), [25, 25, 20, 20]);
        assert_eq!(joints(Mario, Status::AttackAirN), [25, 20, 5, 25, 20, 5]);
        assert_eq!(joints(Fox, Status::AttackLw4), [25, 20]);
    }

    /// `move_data` finds `Attack13`'s data through the extended-status path
    /// too, not just common ones — `MARIO_JAB3`'s doc comment.
    #[test]
    fn attack13_hitbox_grows_mid_swing_without_a_damage_change() {
        let data = move_data(
            crate::fighter::FighterKind::Mario,
            AnyStatus::Mario(MarioStatus::Attack13),
        )
        .expect("Mario's Attack13 has real MoveData");
        assert!(data.hitboxes.iter().all(|h| h.hitbox.damage == 4));
        let small: Vec<_> = data.hitboxes.iter().filter(|h| h.is_active(4.0)).collect();
        let grown: Vec<_> = data.hitboxes.iter().filter(|h| h.is_active(6.0)).collect();
        assert!(small.iter().any(|h| h.hitbox.radius == 75.0));
        assert!(grown.iter().all(|h| h.hitbox.radius != 75.0));
    }

    /// A jab that connects, chains into `Attack12`, and would chain into
    /// `Attack13` still resolves each hit through the same generic
    /// `apply_hit_from` — the extended status is just another key into
    /// `move_data`.
    #[test]
    fn attack13_lands_a_real_hit_through_apply_hit_from() {
        let mut attacker = Fighter::new(crate::fighter::FighterKind::Mario, 0, 3);
        let mut defender = Fighter::new(crate::fighter::FighterKind::Mario, 1, 3);
        attacker.pos = Vec3::new(0.0, 0.0, 0.0);
        defender.pos = Vec3::new(0.0, 0.0, 0.0);
        defender.situation = crate::fighter::Situation::Ground;
        status::set_any_status(
            &mut attacker,
            AnyStatus::Mario(MarioStatus::Attack13),
            4.0,
            StatusTiming::unknown(),
        );

        let mut hit_record = HitRecord::default();
        apply_hit_from(&mut attacker, &mut defender, &mut hit_record);

        assert!(hit_record.hit_generation.is_some());
        assert_eq!(defender.damage, 4);
    }

    #[test]
    fn donkey_motion_windows_and_charge_damage_feed_hit_resolution() {
        let normal = move_data(
            crate::fighter::FighterKind::Donkey,
            Status::AttackAirLw.into(),
        )
        .unwrap();
        assert_eq!(normal.length_frames, 60.0);
        assert_eq!(
            normal
                .hitboxes
                .iter()
                .find(|h| h.is_active(6.0))
                .unwrap()
                .hitbox
                .damage,
            13
        );
        assert_eq!(
            normal
                .hitboxes
                .iter()
                .find(|h| h.is_active(12.0))
                .unwrap()
                .hitbox
                .damage,
            10
        );

        let mut attacker = Fighter::new(crate::fighter::FighterKind::Donkey, 0, 3);
        let mut defender = Fighter::new(crate::fighter::FighterKind::Mario, 1, 3);
        attacker.pos = Vec3::ZERO;
        defender.pos = Vec3::ZERO;
        attacker.donkey_special_n.attack_charge = 4;
        status::set_any_status(
            &mut attacker,
            AnyStatus::Donkey(crate::status::DonkeyStatus::SpecialNEnd),
            9.0,
            StatusTiming::frames(80.0),
        );
        apply_hit_from(&mut attacker, &mut defender, &mut HitRecord::default());
        assert_eq!(defender.damage, 22); // script base 14 + 4 * 2
        let spin = move_data(
            crate::fighter::FighterKind::Donkey,
            AnyStatus::Donkey(crate::status::DonkeyStatus::SpecialAirHi),
        )
        .unwrap();
        assert_eq!(spin.hitboxes.iter().filter(|h| h.is_active(3.0)).count(), 2);
        assert_eq!(
            spin.hitboxes.iter().filter(|h| h.is_active(17.0)).count(),
            3
        );
        assert_eq!(
            spin.hitboxes.iter().filter(|h| h.is_active(49.0)).count(),
            3
        );
        assert!(spin
            .hitboxes
            .iter()
            .filter(|h| h.is_active(49.0))
            .all(|h| h.hitbox.damage == 3));
    }

    #[test]
    fn donkey_punch_reaches_with_its_second_hitbox_when_the_first_misses() {
        let mut attacker = Fighter::new(crate::fighter::FighterKind::Donkey, 0, 3);
        let mut defender = Fighter::new(crate::fighter::FighterKind::Mario, 1, 3);
        attacker.pos = Vec3::ZERO;
        defender.pos = Vec3::new(400.0, 0.0, 0.0);
        status::set_any_status(
            &mut attacker,
            AnyStatus::Donkey(crate::status::DonkeyStatus::SpecialNEnd),
            9.0,
            StatusTiming::frames(80.0),
        );
        let mut hit_record = HitRecord::default();
        apply_hit_from(&mut attacker, &mut defender, &mut hit_record);
        assert_eq!(defender.damage, 14);
        assert_eq!(hit_record.hit_generation, Some(0));

        apply_hit_from(&mut attacker, &mut defender, &mut hit_record);
        assert_eq!(defender.damage, 14);
    }
}
