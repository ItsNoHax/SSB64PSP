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
//! * **No per-bone joint attachment.** A hitbox's `ox`/`oy`/`oz` offset is
//!   from an attachment joint in the original; here it is just added to the
//!   attacker's root position (`oy`/`oz` as-is, `ox` mirrored by facing —
//!   see [`apply_hit_from`]). `Jab1`'s own offset is `(0, 0, 0)`, so it is
//!   unaffected; moves with a real reach offset (the tilts, the dash attack)
//!   are approximated by this, not modelled exactly.
//! * **A sphere hurtbox, not a per-bone capsule set.** The original tests a
//!   hitbox sphere against eleven `FTDamageColl` capsules per fighter
//!   (`gmCollisionCheckFighterInFighterRange`). No per-bone hurtbox system
//!   exists yet, so [`MARIO_HURTBOX_RADIUS`] stands in with a single sphere at
//!   the fighter's root — sized from Mario's own real collision-diamond width
//!   (`dMarioMain_attr.map_coll = { 320, 190, 0, 150 }`, so half of `150`),
//!   not an invented number.
//! * **No hitlag, no handicap, no damage-ratio scaling, no `recent_damage`
//!   accumulation.** `ftParamGetCommonKnockback`'s handicap and damage-ratio
//!   terms are fixed at their singleplayer-neutral value of `1.0`, and
//!   `recent_damage` (a short-window stale-move accumulator) is treated as
//!   always zero. None of those systems exist yet; wiring real values in
//!   later must not silently change these formulas' shape.
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
use crate::status::{self, Status, StatusTiming};

/// A hitbox descriptor, transcribed field-for-field from a
/// `ftMotionCommandMakeAttackColl(aid, gid, jid, dmg, reb, elem, sz, ox, oy,
/// oz, ang, kbs, kbw, ga, sd, fl, fk, kbb)` call, so it can be checked against
/// the decomp source by eye. Fields the ported hit-resolution path does not
/// use yet (rebound, element, shield damage, sound) are not carried.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hitbox {
    /// `dmg` — base damage, added to the target's percent as-is.
    pub damage: i32,
    /// `ox, oy, oz` — offset from the attachment joint. `(0, 0, 0)` for
    /// [`MARIO_JAB1_HITBOX`], so the missing joint attachment (see module
    /// docs) is not yet observable.
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
}

impl ActiveHitbox {
    pub const fn new(hitbox: Hitbox, start: f32, end: f32) -> Self {
        ActiveHitbox { hitbox, start, end }
    }

    pub fn is_active(&self, anim_frame: f32) -> bool {
        (self.start..self.end).contains(&anim_frame)
    }
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
/// two hitboxes: `hit_by_current_attack`'s suppression already means only
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

/// Mario's neutral aerial — `dMarioMainMotion_AttackAirN`. Three
/// simultaneous hitboxes (`jid` 25/20/5 — foot, shin, and a wider late
/// sweetspot), each with a weaker second phase after frame 11.
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

/// Mario's down smash — `dMarioMainMotion_DSmash`. Two hitboxes (each
/// spawned twice under different `jid`s with identical other arguments, so
/// two `ActiveHitbox`es cover all four `MakeAttackColl` calls).
/// `WaitAsync(4)` then `WaitAsync(8)` then two `MakeAttackColl`s,
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
    ],
    length_frames: 34.0,
    landing_lag_percent: None,
};

/// The attack data for `status`, under `kind` — every per-character motion
/// script's `MakeAttackColl` argument list, transcribed field-for-field (see
/// each `MoveData` constant's own doc comment for its source line). `None`
/// for any status/fighter this codebase has not extracted moveset data for
/// yet, which is everything except the Mario moves listed here (`P2`'s bulk
/// port scope).
pub fn move_data(kind: crate::fighter::FighterKind, status: Status) -> Option<&'static MoveData> {
    use crate::fighter::FighterKind;
    match (kind, status) {
        (FighterKind::Mario, Status::Attack11) => Some(&MARIO_JAB1),
        (FighterKind::Mario, Status::AttackDash) => Some(&MARIO_DASH_ATTACK),
        (FighterKind::Mario, Status::AttackS3Hi) => Some(&MARIO_FTILT_HI),
        (FighterKind::Mario, Status::AttackS3) => Some(&MARIO_FTILT),
        (FighterKind::Mario, Status::AttackS3Lw) => Some(&MARIO_FTILT_LOW),
        (FighterKind::Mario, Status::AttackHi3) => Some(&MARIO_UTILT),
        (FighterKind::Mario, Status::AttackLw3) => Some(&MARIO_DTILT),
        (FighterKind::Mario, Status::AttackAirN) => Some(&MARIO_AIR_N),
        (FighterKind::Mario, Status::AttackAirF) => Some(&MARIO_AIR_F),
        (FighterKind::Mario, Status::AttackAirB) => Some(&MARIO_AIR_B),
        (FighterKind::Mario, Status::AttackAirHi) => Some(&MARIO_AIR_HI),
        (FighterKind::Mario, Status::AttackAirLw) => Some(&MARIO_AIR_LW),
        (FighterKind::Mario, Status::AttackS4Hi) => Some(&MARIO_FSMASH_HI),
        (FighterKind::Mario, Status::AttackS4HiS) => Some(&MARIO_FSMASH_HI_S),
        (FighterKind::Mario, Status::AttackS4) => Some(&MARIO_FSMASH),
        (FighterKind::Mario, Status::AttackS4LwS) => Some(&MARIO_FSMASH_LOW_S),
        (FighterKind::Mario, Status::AttackS4Lw) => Some(&MARIO_FSMASH_LOW),
        (FighterKind::Mario, Status::AttackHi4) => Some(&MARIO_USMASH),
        (FighterKind::Mario, Status::AttackLw4) => Some(&MARIO_DSMASH),
        _ => None,
    }
}

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

/// `ftParamGetCommonKnockback` @ `ftparam.c:1451`, restricted to
/// singleplayer-neutral handicap/damage-ratio terms and `recent_damage == 0`
/// (module docs).
pub fn common_knockback(
    defender_damage_percent: u16,
    hitbox: &Hitbox,
    defender_weight: f32,
) -> f32 {
    let scale = hitbox.kb_scale as f32 * 0.01;
    let knockback = if hitbox.kb_weight != 0 {
        (((1.0 + (10.0 * hitbox.kb_weight as f32 * 0.05)) * defender_weight * 1.4) + 18.0) * scale
            + hitbox.kb_base as f32
    } else {
        let damage_add = defender_damage_percent as f32;
        let hit_damage = hitbox.damage as f32;
        ((((damage_add * 0.1) + (damage_add * hit_damage * 0.05)) * defender_weight * 1.4) + 18.0)
            * scale
            + hitbox.kb_base as f32
    };
    knockback.min(2500.0)
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
pub fn resolve_hit(
    hitbox: &Hitbox,
    attacker_pos: Vec3,
    defender_pos: Vec3,
    defender_damage_percent: u16,
    defender_weight: f32,
    defender_airborne: bool,
) -> HitResult {
    let lr = damage_lr(defender_pos, attacker_pos);
    let knockback = common_knockback(defender_damage_percent, hitbox, defender_weight);
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

/// `F1` criterion 5: tests `attacker`'s active hitbox against `defender` and
/// applies the hit. `hit_by_current_attack` is the caller's per-target
/// hit-suppression state — the simplified stand-in for the original's
/// per-attack `GMAttackRecord` hit list (module docs) — cleared as soon as
/// the attacker leaves `Attack11` so the next jab can hit again.
///
/// Only `Attack11`'s hitbox is ported (module docs), so anything else the
/// attacker is doing is a no-op call.
pub fn apply_hit_from(
    attacker: &Fighter,
    defender: &mut Fighter,
    hit_by_current_attack: &mut bool,
) {
    let Some(move_data) = move_data(attacker.kind, attacker.status.status) else {
        *hit_by_current_attack = false;
        return;
    };
    if *hit_by_current_attack {
        return;
    }
    let Some(active) = move_data
        .hitboxes
        .iter()
        .find(|h| h.is_active(attacker.status.anim_frame))
    else {
        return;
    };
    let hitbox = active.hitbox;
    // `ox` mirrors with facing (a joint-space X offset rotated by the
    // fighter's own transform in the original); `oy`/`oz` do not need that,
    // matching decomp's own attachment convention. This closes the "offset
    // not yet observable" gap `Jab1` used to have — `Jab1`'s own offset is
    // `(0, 0, 0)` so it is unaffected.
    let hitbox_pos = attacker.pos
        + Vec3::new(
            hitbox.offset.x * attacker.facing.sign(),
            hitbox.offset.y,
            hitbox.offset.z,
        );
    if !spheres_overlap(
        hitbox_pos,
        hitbox.radius,
        defender.pos,
        MARIO_HURTBOX_RADIUS,
    ) {
        return;
    }
    if defender.invincible_frames > 0 {
        // `nGMHitStatusInvincible`: the hitbox simply does not register —
        // `hit_by_current_attack` is left alone so the same active window
        // can still connect once invincibility ends.
        return;
    }
    if is_shielding(defender.status.status) {
        apply_shield_hit(&hitbox, attacker, defender);
        *hit_by_current_attack = true;
        return;
    }
    let result = resolve_hit(
        &hitbox,
        attacker.pos,
        defender.pos,
        defender.damage,
        defender.attributes.weight,
        !defender.is_grounded(),
    );
    defender.damage = defender.damage.saturating_add(result.damage as u16);
    // `ftCommonDamageInitDamageVars` @ `ftcommondamage.c:557` zeroes the
    // fighter's normal ground/air velocity outright before writing the
    // knockback vector — a hit fully overrides existing movement rather than
    // adding to it. `status::set_status` runs first because it is what moves
    // `vel_ground` into `vel_air` on a ground-to-air transition, and that
    // transferred value must not survive the zeroing below.
    status::set_status(defender, result.status, 0.0, StatusTiming::unknown());
    defender.physics.vel_ground = Vec3::ZERO;
    defender.physics.vel_air = Vec3::ZERO;
    defender.physics.vel_knockback = result.knockback_vel;
    defender.hitstun = result.hitstun;
    *hit_by_current_attack = true;
}

/// Whether a hit landing on this status should be redirected into
/// [`apply_shield_hit`] rather than the normal Damage-family path — the
/// statuses `ftMainUpdateShieldStatFighter`'s caller treats as "currently
/// shielding" (`ftmain.c`'s hit-search gates a shield hit on `fp->is_shield`,
/// which these three statuses hold for the whole time they are active).
pub fn is_shielding(status: Status) -> bool {
    matches!(
        status,
        Status::GuardOn | Status::Guard | Status::GuardSetOff
    )
}

/// `ftMainUpdateShieldStatFighter` @ `ftmain.c:2059`, reduced to the
/// single-hit case (module docs: no `shield_damage_total` multi-hit
/// accumulation) — a hit landing on a shield deals no damage/knockback/
/// hitstun at all, only shield health loss and a `GuardSetOff` pushback.
pub fn apply_shield_hit(hitbox: &Hitbox, attacker: &Fighter, defender: &mut Fighter) {
    let shield_lr = damage_lr(defender.pos, attacker.pos);
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
        let kb = common_knockback(0, &MARIO_JAB1_HITBOX, 1.0);
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
        let result = resolve_hit(&MARIO_JAB1_HITBOX, attacker, defender, 0, 1.0, false);
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

        let mut hit_by_current_attack = false;
        apply_hit_from(&attacker, &mut defender, &mut hit_by_current_attack);

        assert_eq!(defender.status.status, Status::DamageN1);
        assert!(hit_by_current_attack);
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

        let mut hit_by_current_attack = false;
        apply_hit_from(&attacker, &mut defender, &mut hit_by_current_attack);

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

        let mut hit_by_current_attack = false;
        apply_hit_from(&attacker, &mut defender, &mut hit_by_current_attack);

        assert_eq!(defender.status.status, Status::Wait);
        assert_eq!(defender.damage, 0);
        assert!(!hit_by_current_attack);
    }

    #[test]
    fn move_data_is_none_for_an_unported_fighter_or_status() {
        assert!(move_data(crate::fighter::FighterKind::Fox, Status::Attack11).is_none());
        assert!(move_data(crate::fighter::FighterKind::Mario, Status::Attack12).is_none());
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

        let mut hit_by_current_attack = false;
        apply_hit_from(&attacker, &mut defender, &mut hit_by_current_attack);

        // Only reachable if the offset flipped to -90 with facing; at +90 the
        // defender at x=-90 would be 180 units away, past even the 115-unit
        // far-hitbox radius.
        assert!(hit_by_current_attack);
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
        assert_eq!(strong.len(), 2); // the two ox=10/ox=0 hitboxes share a window
        assert_eq!(weak.len(), 2);
        assert!(strong.iter().all(|h| h.hitbox.damage == 14));
        assert!(weak.iter().all(|h| h.hitbox.damage == 11));
    }
}
