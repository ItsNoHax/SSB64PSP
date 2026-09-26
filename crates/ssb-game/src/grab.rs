//! Grabs, captures and throws — the shared `ftcommoncatch*.c`,
//! `ftcommoncapture*.c`, `ftcommonthrow*.c` and `ftcommonthrown*.c` statuses,
//! plus Donkey Kong's cargo carry (`ftdonkeythrowf*.c`).
//!
//! ## Two fighters, one link
//!
//! The original links a grabbing fighter (`catch_gobj`) and the fighter it
//! holds (`capture_gobj`) by pointer, and each side's callbacks write the
//! other's status directly. Here a [`Fighter`] holds no reference to another
//! fighter, so a callback that must act on the partner queues a
//! [`GrabEvent`] in [`GrabState::outbox`] instead. The match loop calls
//! [`exchange`] right after each fighter's tick. That delivers the events
//! before the partner runs its own tick, which is the order the original's
//! direct writes give: a fighter processed later in the frame sees its new
//! status in the same frame. [`exchange`] also refreshes each side's
//! [`Holder`] snapshot, which is what a held fighter reads where the original
//! reads `capture_fp`.
//!
//! [`search_catch`] is `ftMainProcSearchCatch`. The match loop calls it in the
//! hit-collision phase, after every fighter has ticked, as the original does.
//!
//! ## Documented deviations
//!
//! * **Attachment joint.** The runtime samples the catcher's heavy-item
//!   joint matrix and the held fighter's first child translation. Gameplay
//!   applies the negative child translation through that matrix; rendering
//!   uses its rotation. Host tests without a skeleton use the catcher root.
//! * **Catch collision.** Catch boxes use their posed joint transform,
//!   including hand rotations and offsets. Hurtboxes remain one root sphere.
//! * **No 1P stats.** Throw damage is staled by the catcher's queue
//!   ([`crate::stale`]) when the catcher queues the release, and the
//!   catcher's queue records the throw when the damage lands.
//! * **Throw attack colls.** Mario's and Fox's back throws make attack
//!   collisions ([`crate::attack::move_data`]). `apply_hit_from` skips the
//!   held fighter, as `ftMainSearchFighterAttack` skips `capture_gobj`, so
//!   only bystanders can be hit.
//! * **Held fighters hit by a third party.** `ftCommonDamageUpdateMain`
//!   keeps the hold below [`crate::attack::CATCH_RELEASE_THRESHOLD`] and
//!   otherwise drops it, sending the catcher
//!   [`GrabEvent::CaptureHitRelease`]. The branches for both fighters being
//!   hit in the same frame need the source's deferred damage queue, which
//!   the immediate hit path does not have; hits resolve one at a time.
//! * **Losing grip.** `ftCommonThrownReleaseFighterLoseGrip` snaps a thrown
//!   fighter to its joint 4 minus 300 and runs the catcher-relative floor
//!   collision. Here the fighter keeps its held position and the normal
//!   airborne collision runs from there on its next tick.

use ssb_engine::input::N64Buttons;
use ssb_engine::math::Vec3;

use crate::attack::{self, Hitbox};
use crate::collision::{self, Segment};
use crate::fighter::{Facing, Fighter, FighterKind, JointTransform, Situation};
use crate::physics;
use crate::stale::MotionAttackId;
use crate::status::{self, AnyStatus, DonkeyStatus, JumpInput, Status, StatusTiming};

/// `FTCOMMON_CATCH_THROW_WAIT` — frames before a held fighter is thrown
/// forward automatically.
pub const CATCH_THROW_WAIT: i32 = 60;
/// `FTCOMMON_CATCH_THROW_STICK_RANGE_MIN`.
pub const CATCH_THROW_STICK_RANGE_MIN: i32 = 20;
/// `FTCOMMON_CAPTURE_MASH_STICK_RANGE_MIN`.
pub const CAPTURE_MASH_STICK_RANGE_MIN: i32 = 40;
/// `FTCOMMON_THROWFF_TURN_STICK_RANGE_MIN`.
pub const THROWFF_TURN_STICK_RANGE_MIN: i32 = 20;
/// `FTCOMMON_THROWFF_TURN_FRAMES`.
pub const THROWFF_TURN_FRAMES: i32 = 6;
/// `FTCOMMON_THROWFFALL_SKIPLANDING_VEL_Y_MAX`.
pub const THROWFFALL_SKIPLANDING_VEL_Y_MAX: f32 = -20.0;

/// `FTThrowHitDesc`: the knockback a throw deals. `[0]` is the throw itself;
/// `[1]` is used when the catcher is hit and lets go.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThrowHitDesc {
    /// `status_id`: a forced damage status, or `None` for the table's own.
    pub status: Option<Status>,
    pub damage: i32,
    pub angle: i32,
    pub kb_scale: i32,
    pub kb_weight: i32,
    pub kb_base: i32,
}

const fn desc(
    status: Option<Status>,
    damage: i32,
    angle: i32,
    kbs: i32,
    kbw: i32,
    kbb: i32,
) -> ThrowHitDesc {
    ThrowHitDesc {
        status,
        damage,
        angle,
        kb_scale: kbs,
        kb_weight: kbw,
        kb_base: kbb,
    }
}

const FLY_N: Option<Status> = Some(Status::DamageFlyN);

// `ftMotionCommandSetThrow` descriptors from `202_MarioMainMotion.c`,
// `208_FoxMainMotion.c` and `212_DonkeyMainMotion.c`. Mario's back throw is
// the US descriptor (`#else` branch); status 55 is `DamageFlyRoll`.
const MARIO_CATCH: [ThrowHitDesc; 2] = [desc(None, 6, 361, 100, 0, 0); 2];
const MARIO_THROW_F: [ThrowHitDesc; 2] = [
    desc(FLY_N, 12, 45, 70, 0, 80),
    desc(None, 6, 361, 100, 0, 0),
];
const MARIO_THROW_B: [ThrowHitDesc; 2] = [
    desc(Some(Status::DamageFlyRoll), 16, 45, 70, 0, 70),
    desc(None, 8, 361, 100, 0, 0),
];
const FOX_CATCH: [ThrowHitDesc; 2] = [desc(None, 2, 361, 100, 0, 0); 2];
const FOX_THROW_F: [ThrowHitDesc; 2] = [
    desc(FLY_N, 12, 45, 60, 0, 80),
    desc(None, 2, 361, 100, 0, 0),
];
const FOX_THROW_B: [ThrowHitDesc; 2] = [
    desc(FLY_N, 15, 45, 60, 0, 80),
    desc(None, 9, 361, 100, 0, 0),
];
const DONKEY_CATCH: [ThrowHitDesc; 2] = [desc(None, 2, 361, 100, 0, 0); 2];
const DONKEY_THROW_FF: [ThrowHitDesc; 2] =
    [desc(FLY_N, 8, 45, 80, 0, 70), desc(None, 2, 361, 100, 0, 0)];
const DONKEY_THROW_B: [ThrowHitDesc; 2] = [
    desc(FLY_N, 18, 45, 70, 0, 80),
    desc(None, 9, 361, 100, 0, 0),
];
// `220_LuigiMainMotion.c` (US). The catch descriptor equals Mario's. Luigi's
// forward throw rolls the target and his back throw does not, the reverse of
// Mario's pair.
const LUIGI_THROW_F: [ThrowHitDesc; 2] = [
    desc(Some(Status::DamageFlyRoll), 16, 45, 70, 0, 70),
    desc(None, 6, 361, 100, 0, 0),
];
const LUIGI_THROW_B: [ThrowHitDesc; 2] = [
    desc(FLY_N, 12, 45, 70, 0, 80),
    desc(None, 8, 361, 100, 0, 0),
];
// `224_LinkMainMotion.c`. The catch descriptor equals Mario's.
const LINK_THROW_F: [ThrowHitDesc; 2] = [
    desc(FLY_N, 14, 45, 70, 0, 80),
    desc(None, 6, 361, 100, 0, 0),
];
const LINK_THROW_B: [ThrowHitDesc; 2] = [
    desc(FLY_N, 16, 45, 70, 0, 70),
    desc(None, 8, 361, 100, 0, 0),
];
// `246_YoshiMainMotion.c`. The catch descriptor equals Mario's.
const YOSHI_THROW_F: [ThrowHitDesc; 2] = [
    desc(FLY_N, 12, 45, 70, 0, 80),
    desc(None, 6, 361, 100, 0, 0),
];
const YOSHI_THROW_B: [ThrowHitDesc; 2] = [
    desc(FLY_N, 16, 45, 70, 0, 70),
    desc(None, 8, 361, 100, 0, 0),
];
// `216_SamusMainMotion.c`. Both throws carry element 2 (electric), which
// only selects hit effects.
const SAMUS_CATCH: [ThrowHitDesc; 2] = [desc(None, 8, 361, 100, 0, 0); 2];
const SAMUS_THROW_F: [ThrowHitDesc; 2] = [
    desc(FLY_N, 16, 40, 60, 0, 90),
    desc(None, 8, 361, 100, 0, 0),
];
const SAMUS_THROW_B: [ThrowHitDesc; 2] = [
    desc(FLY_N, 18, 40, 60, 0, 90),
    desc(None, 8, 361, 100, 0, 0),
];
// `235_CaptainMainMotion.c`: Falcon's standard grab and throw descriptors.
const CAPTAIN_THROW_F: [ThrowHitDesc; 2] = [
    desc(FLY_N, 12, 70, 50, 0, 100),
    desc(None, 6, 361, 100, 0, 0),
];
const CAPTAIN_THROW_B: [ThrowHitDesc; 2] = [
    desc(FLY_N, 16, 361, 50, 0, 100),
    desc(None, 8, 361, 100, 0, 0),
];

/// `FTThrowReleaseDesc dFTCommonCaptureKnockbackCatch`: what the catcher
/// suffers when the held fighter breaks free. `{ angle, kbs, kbw, kbb }`.
const CAPTURE_KNOCKBACK_CATCH: (i32, i32, i32, i32) = (361, 100, 30, 0);
/// `dFTCommonCaptureKnockbackCapture`: what the escaping fighter suffers.
const CAPTURE_KNOCKBACK_CAPTURE: (i32, i32, i32, i32) = (361, 80, 0, 20);

/// The motion flag and frame at which a script's `SetFlag` fires, or `None`.
/// Frame numbers are the scripts' `Wait`/`WaitAsync` cursors.
#[derive(Debug, Clone, Copy, PartialEq)]
struct ThrowScript {
    /// `SetThrow` descriptor, if the script sets one.
    desc: Option<[ThrowHitDesc; 2]>,
    /// `SetFlag1(1)`: turn around (back throws).
    flag1: Option<f32>,
    /// `SetFlag2(n)`: release. `1` throws toward the new back, `2` forward.
    flag2: Option<(f32, u8)>,
    /// Figatree length (`ssb_rom::anim::EXPECTED_FRAMES`).
    length: f32,
}

fn throw_script(kind: FighterKind, back: bool) -> ThrowScript {
    match (base_kind(kind), back) {
        (FighterKind::Mario, false) => ThrowScript {
            desc: Some(MARIO_THROW_F),
            flag1: None,
            flag2: Some((14.0, 1)),
            length: 28.0,
        },
        // Wait(4) WaitAsync(10) Wait(8), two Wait(14) loops, then SetFlag1 at
        // 46; the following WaitAsync(45) has already passed, so SetFlag2
        // fires on the same frame.
        (FighterKind::Mario, true) => ThrowScript {
            desc: Some(MARIO_THROW_B),
            flag1: Some(46.0),
            flag2: Some((46.0, 1)),
            length: 67.0,
        },
        (FighterKind::Fox, false) => ThrowScript {
            desc: Some(FOX_THROW_F),
            flag1: None,
            flag2: Some((10.0, 1)),
            length: 35.0,
        },
        (FighterKind::Fox, true) => ThrowScript {
            desc: Some(FOX_THROW_B),
            flag1: None,
            flag2: Some((19.0, 2)),
            length: 40.0,
        },
        // Donkey Kong's forward throw only lifts; the cargo statuses throw.
        (FighterKind::Donkey, false) => ThrowScript {
            desc: None,
            flag1: None,
            flag2: None,
            length: 20.0,
        },
        (FighterKind::Donkey, true) => ThrowScript {
            desc: Some(DONKEY_THROW_B),
            flag1: Some(4.0),
            flag2: Some((9.0, 1)),
            length: 22.0,
        },
        // Luigi's throws play Mario's figatrees and keep Mario's event frames.
        (FighterKind::Luigi, false) => ThrowScript {
            desc: Some(LUIGI_THROW_F),
            flag1: None,
            flag2: Some((14.0, 1)),
            length: 28.0,
        },
        (FighterKind::Luigi, true) => ThrowScript {
            desc: Some(LUIGI_THROW_B),
            flag1: Some(46.0),
            flag2: Some((46.0, 1)),
            length: 67.0,
        },
        // WaitAsync(12), Wait(8): both throws release at frame 20.
        (FighterKind::Link, false) => ThrowScript {
            desc: Some(LINK_THROW_F),
            flag1: None,
            flag2: Some((20.0, 1)),
            length: 40.0,
        },
        (FighterKind::Link, true) => ThrowScript {
            desc: Some(LINK_THROW_B),
            flag1: None,
            flag2: Some((20.0, 2)),
            length: 40.0,
        },
        // WaitAsync(4), WaitAsync(19): both throws release at frame 19.
        (FighterKind::Yoshi, false) => ThrowScript {
            desc: Some(YOSHI_THROW_F),
            flag1: None,
            flag2: Some((19.0, 1)),
            length: 39.0,
        },
        (FighterKind::Yoshi, true) => ThrowScript {
            desc: Some(YOSHI_THROW_B),
            flag1: None,
            flag2: Some((19.0, 2)),
            length: 49.0,
        },
        (FighterKind::Captain, false) => ThrowScript {
            desc: Some(CAPTAIN_THROW_F),
            flag1: None,
            flag2: Some((20.0, 1)),
            length: 44.0,
        },
        (FighterKind::Captain, true) => ThrowScript {
            desc: Some(CAPTAIN_THROW_B),
            flag1: None,
            flag2: Some((20.0, 2)),
            length: 44.0,
        },
        // Wait(4) then WaitAsync(9): both throws release at frame 9.
        (FighterKind::Samus, false) => ThrowScript {
            desc: Some(SAMUS_THROW_F),
            flag1: None,
            flag2: Some((9.0, 1)),
            length: 42.0,
        },
        (FighterKind::Samus, true) => ThrowScript {
            desc: Some(SAMUS_THROW_B),
            flag1: None,
            flag2: Some((9.0, 2)),
            length: 42.0,
        },
        _ => ThrowScript {
            desc: None,
            flag1: None,
            flag2: None,
            length: 0.0,
        },
    }
}

/// Polygon, Metal and Giant fighters share their base fighter's motion data.
pub(crate) fn base_kind(kind: FighterKind) -> FighterKind {
    match kind {
        FighterKind::MetalMario => FighterKind::Mario,
        FighterKind::GiantDonkey => FighterKind::Donkey,
        k => k.polygon_base().unwrap_or(k),
    }
}

/// `FTAttributes::joint_itemheavy_id`: the joint a held fighter hangs from
/// (`203_MarioMain.c`, `209_FoxMain.c`, `213_DonkeyMain.c`,
/// `217_SamusMain.c`, `221_LuigiMain.c`, `225_LinkMain.c`, `247_YoshiMain.c`).
/// The runtime samples its world position into [`GrabState::anchor`].
pub fn itemheavy_joint(kind: FighterKind) -> Option<usize> {
    match base_kind(kind) {
        FighterKind::Mario | FighterKind::Luigi => Some(28),
        FighterKind::Fox => Some(30),
        FighterKind::Donkey => Some(29),
        FighterKind::Samus => Some(36),
        FighterKind::Link => Some(35),
        // The tongue, which also carries a swallowed fighter.
        FighterKind::Yoshi => Some(31),
        FighterKind::Captain => Some(29),
        _ => None,
    }
}

/// `FTAttributes::is_have_catch`: every fighter except the Polygon team.
pub fn has_catch(kind: FighterKind) -> bool {
    !kind.is_polygon()
}

/// The `Catch` script's attack collisions: `(hitbox, joint)`, active over
/// [`catch_coll_frames`].
fn catch_colls(kind: FighterKind) -> &'static [(Hitbox, u8)] {
    const fn catch(size: f32, x: f32, y: f32, z: f32) -> Hitbox {
        Hitbox {
            damage: 0,
            radius: size / 2.0,
            offset: Vec3::new(x, y, z),
            angle: 361,
            kb_scale: 100,
            kb_weight: 0,
            kb_base: 0,
        }
    }
    const MARIO: [(Hitbox, u8); 1] = [(catch(290.0, 0.0, 0.0, 0.0), 28)];
    const FOX: [(Hitbox, u8); 1] = [(catch(260.0, 50.0, 0.0, 0.0), 30)];
    const DONKEY: [(Hitbox, u8); 2] = [
        (catch(330.0, 0.0, 0.0, 0.0), 29),
        (catch(180.0, 0.0, 200.0, 250.0), 0),
    ];
    // The Grapple Beam: both boxes ride the beam joint.
    const SAMUS: [(Hitbox, u8); 2] = [
        (catch(210.0, 0.0, 0.0, 0.0), 36),
        (catch(160.0, 0.0, 0.0, 200.0), 36),
    ];
    // The Hookshot: one box on the hook joint.
    const LINK: [(Hitbox, u8); 1] = [(catch(180.0, 0.0, 0.0, 0.0), 35)];
    // The tongue.
    const YOSHI: [(Hitbox, u8); 1] = [(catch(220.0, 0.0, 0.0, 0.0), 31)];
    const CAPTAIN: [(Hitbox, u8); 1] = [(catch(300.0, 0.0, 0.0, 0.0), 29)];
    match base_kind(kind) {
        FighterKind::Yoshi => &YOSHI,
        FighterKind::Captain => &CAPTAIN,
        FighterKind::Link => &LINK,
        // `dLuigiMainMotion_Catch` has Mario's box and window.
        FighterKind::Mario | FighterKind::Luigi => &MARIO,
        FighterKind::Fox => &FOX,
        FighterKind::Donkey => &DONKEY,
        FighterKind::Samus => &SAMUS,
        _ => &[],
    }
}

/// Frames the `Catch` script's collisions exist: `WaitAsync(6)` and a
/// one-frame clear, or Samus's five `Wait(4)` loops before the beam and its
/// 19-frame reach.
fn catch_coll_frames(kind: FighterKind) -> core::ops::Range<f32> {
    match base_kind(kind) {
        FighterKind::Samus => 20.0..39.0,
        // `WaitAsync(17)`, then four `Wait(3)` loops before the clear.
        FighterKind::Link => 17.0..29.0,
        // `WaitAsync(15)`, then `Wait(6)` before the clear.
        FighterKind::Yoshi => 15.0..21.0,
        _ => 6.0..7.0,
    }
}
/// Figatree lengths of `Catch` and `CatchPull`.
fn catch_length(kind: FighterKind) -> f32 {
    match base_kind(kind) {
        FighterKind::Samus => 100.0,
        FighterKind::Link => 85.0,
        FighterKind::Yoshi => 70.0,
        _ => 16.0,
    }
}
fn catch_pull_length(kind: FighterKind) -> f32 {
    match base_kind(kind) {
        FighterKind::Samus => 10.0,
        FighterKind::Link => 6.0,
        FighterKind::Yoshi => 3.0,
        _ => 2.0,
    }
}
/// `(frame, flag1, flag2)` of a `Catch` script that sets the `CatchPull`
/// start frame, which `ftCommonCatchProcUpdate` then winds down to zero over
/// `flag1` frames. Samus sets `SetFlag1(17)`/`SetFlag2(9)` at frame 20 and
/// Link `SetFlag1(12)`/`SetFlag2(5)` at frame 17, Yoshi `SetFlag1(6)`/
/// `SetFlag2(2)` at frame 15.
fn catch_pull_flags(kind: FighterKind) -> Option<(f32, f32, f32)> {
    match base_kind(kind) {
        FighterKind::Samus => Some((20.0, 17.0, 9.0)),
        FighterKind::Link => Some((17.0, 12.0, 5.0)),
        FighterKind::Yoshi => Some((15.0, 6.0, 2.0)),
        _ => None,
    }
}
/// `CapturePulled`'s figatree (3 frames) holds its last pose.
const CAPTURE_PULLED_LENGTH: f32 = 3.0;
/// `ThrowFTurn`'s figatree, and the frame its script sets flag1.
const DONKEY_THROWF_TURN_LENGTH: f32 = 12.0;
/// Cargo throw figatree; `SetFlag2(1)` fires at `WaitAsync(18)`.
const DONKEY_THROWFF_LENGTH: f32 = 40.0;
const DONKEY_THROWFF_RELEASE_FRAME: f32 = 18.0;
/// `dDonkeyMain_attr.throw_walk*_anim_length`.
const DONKEY_THROW_WALK_LENGTHS: [f32; 3] = [50.0, 35.0, 20.0];

/// `thrown_status[catch_fp->fkind].ft_thrown[is_back]` from each thrower's
/// `FTAttributes`: the status to play first (if any) and the one to settle
/// into. Indexed by the held fighter's `FTKind`.
pub fn thrown_status(
    thrower: FighterKind,
    held: FighterKind,
    back: bool,
) -> (Option<Status>, Status) {
    use Status::{ThrownCommon as C, ThrownFoxB as FB, ThrownFoxF as FF};
    match base_kind(thrower) {
        // `dLuigiMain_thrown_status` equals `dMarioMain_thrown_status`.
        FighterKind::Mario | FighterKind::Luigi => {
            if back {
                (Some(Status::ThrownMarioBStart), Status::ThrownMarioB)
            } else {
                (None, C)
            }
        }
        FighterKind::Donkey => {
            if back {
                (None, C)
            } else {
                (Some(Status::ThrownDonkeyF), Status::Shouldered)
            }
        }
        // `dFoxMain_thrown_status`: the Arwing-sized fighters (Fox, Kirby,
        // Jigglypuff and Polygons of them) take Fox's own thrown motions;
        // Donkey Kong and Samus start in `ThrownFoxFStart`.
        FighterKind::Fox => {
            let index = held as u8;
            match (index, back) {
                (1 | 8 | 10 | 15 | 22 | 24, false) => (None, FF),
                (1 | 8 | 10 | 15 | 22 | 24, true) => (None, FB),
                (2 | 16 | 26, false) => (Some(Status::ThrownFoxFStart), C),
                (3 | 17, false) => (Some(Status::ThrownFoxFStart), FF),
                (3 | 17, true) => (None, FB),
                _ => (None, C),
            }
        }
        _ => (None, C),
    }
}

/// Figatree length of a thrown status for the fighter playing it, or `None`
/// when it loops or the fighter has no such motion.
pub fn thrown_length(held: FighterKind, status: Status) -> Option<f32> {
    let lengths: [u16; 8] = match base_kind(held) {
        // Luigi's thrown motions name Mario's figatrees.
        FighterKind::Mario | FighterKind::Luigi => [20, 10, 0, 0, 40, 0, 0, 0],
        FighterKind::Fox => [18, 10, 0, 0, 0, 10, 10, 18],
        FighterKind::Donkey => [20, 10, 5, 0, 0, 6, 0, 0],
        FighterKind::Samus => [20, 10, 5, 0, 0, 0, 5, 10],
        FighterKind::Link => [32, 10, 5, 0, 0, 0, 0, 0],
        FighterKind::Yoshi => [20, 10, 0, 0, 0, 0, 0, 0],
        FighterKind::Captain => [20, 10, 0, 0, 0, 0, 0, 0],
        _ => [0; 8],
    };
    let index = (status as u16).checked_sub(Status::ThrownDonkeyF as u16)? as usize;
    lengths
        .get(index)
        .copied()
        .filter(|&n| n > 0)
        .map(f32::from)
}

/// What a held fighter reads from its catcher — the `capture_fp` fields the
/// original dereferences. Refreshed by [`exchange`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Holder {
    pub kind: FighterKind,
    pub pos: Vec3,
    pub facing: Facing,
    pub status: AnyStatus,
    /// World position of the catcher's `joint_itemheavy_id` joint.
    pub anchor: Vec3,
    /// Full current heavy-item joint transform, when a skeleton is present.
    pub anchor_transform: Option<JointTransform>,
    /// The catcher's floor line, or `None` while it is airborne.
    pub floor_line: Option<u16>,
    pub percent: u16,
    /// `capture_fp->handicap`.
    pub handicap: u8,
}

/// A throw's damage staled by the catcher at the moment it queued the
/// release, and the motion to record in the catcher's queue if it lands.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StaledThrow {
    pub damage: i32,
    pub attack_id: MotionAttackId,
    pub motion_count: u16,
}

impl StaledThrow {
    pub(crate) fn of(f: &Fighter, damage: i32) -> Self {
        StaledThrow {
            damage: crate::stale::staled_damage(f, damage),
            attack_id: f.motion.attack_id,
            motion_count: f.motion.count,
        }
    }
}

/// A write the original makes to the other fighter of a grab.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GrabEvent {
    /// `proc_capture` → `ftCommonCapturePulledProcCapture`.
    Capture,
    /// `ftCommonCatchPullProcUpdate`: `is_goto_pulled_wait = TRUE`.
    GotoPulledWait,
    /// `ftCommonThrownSetStatusQueue` / `...Immediate`.
    Thrown { first: Option<Status>, then: Status },
    /// `ftCommonCaptureShoulderedSetStatus`: the lift's fixed 8 damage.
    Shouldered { staled: StaledThrow },
    /// `ftCommonThrownReleaseThrownUpdateStats`.
    Release {
        lr: f32,
        desc: ThrowHitDesc,
        shield_catch: bool,
        staled: StaledThrow,
    },
    /// `ftCommonThrownSetStatusDamageRelease`: the catcher was hit.
    DamageRelease {
        desc: ThrowHitDesc,
        shield_catch: bool,
        staled: StaledThrow,
    },
    /// Sent by the held fighter: a third party's hit reached
    /// `FTCOMMON_DAMAGE_CATCH_RELEASE_THRESHOLD`, so the catcher loses its
    /// grip and takes `ftCommonThrownSetStatusNoDamageRelease`.
    CaptureHitRelease,
    /// `ftCommonCatchCaptureSetStatusRelease`: the catcher fell off its floor.
    LoseGrip,
    /// `ftCommonCaptureApplyCatchKnockback`: the held fighter broke free.
    BreakoutKnockback,
    /// `ftCommonThrownDecideDeadResult`: the partner was KO'd.
    Dead,
    /// `proc_capture` → `ftCommonCaptureYoshiProcCapture` (Egg Lay).
    CaptureYoshi,
    /// Falcon Dive's `ftCommonCaptureCaptainProcCapture`.
    CaptureCaptain,
    /// Yoshi's release script writes `status_vars.common.captureyoshi.stage`.
    YoshiEggStage(u8),
}

const OUTBOX: usize = 4;

/// One fighter's side of a grab.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct GrabState {
    /// Port of the fighter this one holds — `catch_gobj`.
    pub catch: Option<u8>,
    /// Port of the fighter holding this one — `capture_gobj`.
    pub capture: Option<u8>,
    /// Snapshot of the catcher, while [`GrabState::capture`] is set.
    pub holder: Option<Holder>,
    /// `is_catchstatus`: this status searches for a fighter to grab.
    pub is_catchstatus: bool,
    /// `capture_immune_mask != 0`: cannot be grabbed.
    pub capture_immune: bool,
    pub is_shield_catch: bool,
    /// `throw_desc`, set by the current motion script's `SetThrow`.
    pub throw_desc: Option<[ThrowHitDesc; 2]>,
    /// Kind of the held fighter, for the thrown-status table.
    pub catch_kind: Option<FighterKind>,
    /// `status_vars.common.catchwait.throw_wait`.
    pub throw_wait: i32,
    /// `status_vars.common.catchmain.catch_pull_frame_begin` and
    /// `catch_pull_anim_frames`.
    pub catch_pull_frame_begin: f32,
    pub catch_pull_anim_frames: f32,
    /// `status_vars.common.capture.is_goto_pulled_wait`.
    pub is_goto_pulled_wait: bool,
    /// `status_vars.common.thrown.status_id`: the queued thrown status.
    pub thrown_queue: Option<Status>,
    pub breakout_wait: i32,
    pub breakout_lr: i8,
    pub breakout_ud: i8,
    /// `throwff.is_turn` / `turn_tics`.
    pub throwff_turn_tics: i32,
    /// Runtime-sampled world position of `joint_itemheavy_id`.
    pub anchor: Option<Vec3>,
    pub anchor_transform: Option<JointTransform>,
    /// Translation of this fighter's first child of TopN, sampled from its
    /// current runtime joint pose for capture placement.
    pub held_child_offset: Option<Vec3>,
    /// `FTCOMMON_CAPTURECAPTAIN_MASK_NOUPDATE`: a grounded victim stays put.
    pub captain_no_update: bool,
    /// Events for the partner, drained by [`exchange`].
    pub outbox: [Option<GrabEvent>; OUTBOX],
}

impl GrabState {
    pub(crate) fn send(&mut self, event: GrabEvent) {
        if let Some(slot) = self.outbox.iter_mut().find(|e| e.is_none()) {
            *slot = Some(event);
        }
    }
}

fn is_donkey(kind: FighterKind) -> bool {
    base_kind(kind) == FighterKind::Donkey
}

/// Whether `f` is in one of Donkey Kong's cargo statuses
/// (`nFTDonkeyStatusThrowFStart..=ThrowFEnd`).
pub fn is_cargo(status: AnyStatus) -> bool {
    matches!(
        status,
        AnyStatus::Donkey(
            DonkeyStatus::ThrowFWait
                | DonkeyStatus::ThrowFWalkSlow
                | DonkeyStatus::ThrowFWalkMiddle
                | DonkeyStatus::ThrowFWalkFast
                | DonkeyStatus::ThrowFTurn
                | DonkeyStatus::ThrowFKneeBend
                | DonkeyStatus::ThrowFFall
                | DonkeyStatus::ThrowFLanding
                | DonkeyStatus::ThrowFDamage
        )
    )
}

/// Whether the status places this fighter at its catcher's hand.
pub fn is_held(status: AnyStatus) -> bool {
    matches!(
        status,
        AnyStatus::Common(
            Status::CapturePulled
                | Status::CaptureWait
                | Status::CaptureYoshi
                | Status::CaptureCaptain
                | Status::ThrownDonkeyF
                | Status::ThrownMarioBStart
                | Status::ThrownFoxFStart
                | Status::Shouldered
                | Status::ThrownMarioB
                | Status::ThrownCommon
                | Status::ThrownFoxF
                | Status::ThrownFoxB
        )
    )
}

fn is_thrown(status: AnyStatus) -> bool {
    is_held(status)
        && !matches!(
            status,
            AnyStatus::Common(
                Status::CapturePulled
                    | Status::CaptureWait
                    | Status::CaptureYoshi
                    | Status::CaptureCaptain
            )
        )
}

// ---------------------------------------------------------------------------
// Catch (`ftcommoncatch1.c`, `ftcommoncatch2.c`)
// ---------------------------------------------------------------------------

fn tapped(f: &Fighter) -> N64Buttons {
    ssb_engine::input::newly_pressed(f.prev_input.buttons, f.input.buttons)
}

/// `ftCommonCatchSetStatus` @ 0x80149BA8.
pub fn set_catch(f: &mut Fighter) {
    let length = catch_length(f.kind);
    status::set_status(f, Status::Catch, 0.0, StatusTiming::frames(length));
    f.grab.throw_desc = Some(match base_kind(f.kind) {
        FighterKind::Fox => FOX_CATCH,
        FighterKind::Donkey => DONKEY_CATCH,
        FighterKind::Samus => SAMUS_CATCH,
        // `dLinkMainMotion_0x0E5C` equals Mario's catch descriptor.
        _ => MARIO_CATCH,
    });
    f.grab.catch_pull_frame_begin = 0.0;
    f.grab.catch_pull_anim_frames = 0.0;
    // `ftParamSetCatchParams(fp, FTCATCHKIND_MASK_COMMON, ...)`.
    f.grab.is_catchstatus = true;
    f.grab.is_shield_catch = false;
}

/// `ftCommonCatchCheckInterruptGuard` @ 0x80149C60: A while shielding.
pub fn check_catch_guard(f: &mut Fighter) -> bool {
    let is_shield_catch = f.guard.is_setoff;
    if tapped(f).contains(N64Buttons::A) && has_catch(f.kind) {
        set_catch(f);
        f.grab.is_shield_catch = is_shield_catch;
        return true;
    }
    false
}

/// `ftCommonCatchCheckInterruptCommon` @ 0x80149CE0: Z held, A tapped. The
/// item-throw branch ahead of it needs items, which are not ported.
pub fn check_catch_common(f: &mut Fighter) -> bool {
    if f.input.buttons.contains(N64Buttons::Z)
        && tapped(f).contains(N64Buttons::A)
        && has_catch(f.kind)
    {
        set_catch(f);
        return true;
    }
    false
}

/// `ftCommonCatchCheckInterruptDashRun` @ 0x80149D80. Same input as the
/// common check once the item branch is gone.
pub fn check_catch_dash_run(f: &mut Fighter) -> bool {
    check_catch_common(f)
}

/// `ftCommonCatchCheckInterruptAttack11` @ 0x80149E24: Z tapped in a jab's
/// first two frames.
pub fn check_catch_attack11(f: &mut Fighter) -> bool {
    if tapped(f).contains(N64Buttons::Z) && has_catch(f.kind) {
        set_catch(f);
        return true;
    }
    false
}

/// `ftCommonCatchPullProcCatch` @ 0x80149F04: this fighter's `proc_catch`.
fn catch_pull(f: &mut Fighter, held: &Fighter) {
    let length = catch_pull_length(f.kind);
    let begin = f.grab.catch_pull_frame_begin;
    status::set_status(f, Status::CatchPull, begin, StatusTiming::frames(length));
    f.grab.catch = Some(held.port);
    f.grab.catch_kind = Some(held.kind);
    f.grab.is_catchstatus = false;
    f.grab.capture_immune = true;
}

/// `ftCommonCatchWaitSetStatus` @ 0x8014A000.
fn set_catch_wait(f: &mut Fighter) {
    status::set_status(f, Status::CatchWait, 0.0, StatusTiming::unknown());
    f.grab.throw_wait = CATCH_THROW_WAIT;
    f.grab.capture_immune = true;
}

/// `ftCommonThrowCheckInterruptCatchWait` @ 0x8014A394.
fn check_throw(f: &mut Fighter) -> bool {
    let taps = tapped(f);
    let x = f.stick.x as i32;
    let prev = f.stick.prev_x as i32;
    let is_throwf =
        f.grab.throw_wait == 0 || taps.contains(N64Buttons::A) || taps.contains(N64Buttons::B);
    if !is_throwf {
        let right = prev < CATCH_THROW_STICK_RANGE_MIN && x >= CATCH_THROW_STICK_RANGE_MIN;
        let left = prev > -CATCH_THROW_STICK_RANGE_MIN && x <= -CATCH_THROW_STICK_RANGE_MIN;
        if !right && !left {
            return false;
        }
    }
    set_throw(f, is_throwf);
    true
}

/// `ftCommonThrowSetStatus` @ 0x8014A1E8, minus Kirby's branch. Samus's
/// branch only attaches the Grapple Beam glow effect.
fn set_throw(f: &mut Fighter, is_throwf: bool) {
    let back = !(is_throwf || f.stick.forward(f.facing) >= 0);
    let script = throw_script(f.kind, back);
    let status = if back { Status::ThrowB } else { Status::ThrowF };
    status::set_status(f, status, 0.0, StatusTiming::frames(script.length));
    if let Some(desc) = script.desc {
        f.grab.throw_desc = Some(desc);
    }
    f.grab.capture_immune = true;
    let held = f.grab.catch_kind.unwrap_or(FighterKind::Mario);
    let (first, then) = thrown_status(f.kind, held, back);
    f.grab.send(GrabEvent::Thrown { first, then });
}

/// `ftCommonThrowProcUpdate` @ 0x8014A0C0.
fn update_throw(f: &mut Fighter) {
    let back = f.status.status == Status::ThrowB;
    let script = throw_script(f.kind, back);
    let frame = f.status.anim_frame;
    let crossed = |at: f32| frame >= at && frame - f.status.timing.anim_speed < at;
    if script.flag1.is_some_and(crossed) {
        f.facing = f.facing.flipped();
        f.physics.vel_ground.x = -f.physics.vel_ground.x;
    }
    if let Some((at, which)) = script.flag2 {
        if crossed(at) && f.grab.catch.is_some() {
            let lr = if which == 1 {
                -f.facing.sign()
            } else {
                f.facing.sign()
            };
            release_thrown(f, lr);
        }
    }
    if f.status.animation_ended() {
        if is_donkey(f.kind) && !back && f.grab.catch.is_some() {
            f.grab.send(GrabEvent::Shouldered {
                staled: StaledThrow::of(f, SHOULDERED_DAMAGE),
            });
            set_donkey_throwf_wait(f);
            return;
        }
        status::set_wait_or_fall(f);
    }
}

/// The release half of a throw's `SetFlag2`: the held fighter takes the
/// throw's knockback and the link is dropped.
fn release_thrown(f: &mut Fighter, lr: f32) {
    let desc = f.grab.throw_desc.map(|d| d[0]).unwrap_or(MARIO_CATCH[0]);
    f.grab.send(GrabEvent::Release {
        lr,
        desc,
        shield_catch: f.grab.is_shield_catch,
        staled: StaledThrow::of(f, desc.damage),
    });
    f.grab.catch = None;
    f.grab.catch_kind = None;
    f.grab.capture_immune = false;
}

/// `ftCommonCatchCaptureSetStatusRelease` @ 0x80149AC8: the catcher left its
/// floor, so it falls and the held fighter drops.
pub fn release_on_edge(f: &mut Fighter) {
    status::set_fall(f);
    if f.grab.catch.take().is_some() {
        f.grab.catch_kind = None;
        f.grab.send(GrabEvent::LoseGrip);
    }
}

// ---------------------------------------------------------------------------
// Capture (`ftcommoncapturepulled.c`, `ftcommoncapturewait.c`,
// `ftcommoncapture.c`) and thrown (`ftcommonthrown1.c`, `ftcommonthrown2.c`)
// ---------------------------------------------------------------------------

/// Grabbed while grabbing: the fighter this one held is released with
/// `ftCommonThrownSetStatusDamageRelease`.
pub(crate) fn drop_own_catch(f: &mut Fighter) {
    if f.grab.catch.take().is_some() {
        f.grab.catch_kind = None;
        let desc = f.grab.throw_desc.map(|d| d[1]).unwrap_or(MARIO_CATCH[1]);
        f.grab.send(GrabEvent::DamageRelease {
            desc,
            shield_catch: f.grab.is_shield_catch,
            staled: StaledThrow::of(f, desc.damage),
        });
    }
}

/// `ftCommonCapturePulledProcCapture` @ 0x8014A860, run on the grabbed
/// fighter once its catcher's snapshot has been delivered.
fn capture_pulled(f: &mut Fighter, catcher_port: u8, holder: Holder) {
    drop_own_catch(f);
    f.grab.capture = Some(catcher_port);
    f.grab.holder = Some(holder);
    f.grab.is_catchstatus = false;
    f.facing = holder.facing.flipped();
    status::set_status(
        f,
        Status::CapturePulled,
        0.0,
        StatusTiming::frames(CAPTURE_PULLED_LENGTH),
    );
    f.grab.is_goto_pulled_wait = false;
    f.grab.capture_immune = true;
    f.physics.vel_air = Vec3::ZERO;
    f.physics.vel_ground = Vec3::ZERO;
    f.physics.vel_knockback = Vec3::ZERO;
    f.hitstun = 0;
}

// `dCaptainMainMotion_0x0000`: offset from the victim's TopN to Falcon's
// heavy-item joint, indexed by FTKind. These are signed short pairs in file 235.
const CAPTAIN_OFFSETS: [(f32, f32); 27] = [
    (30.0, 70.0),
    (40.0, 40.0),
    (100.0, 250.0),
    (80.0, 210.0),
    (30.0, 70.0),
    (100.0, 160.0),
    (0.0, 110.0),
    (80.0, 210.0),
    (20.0, 100.0),
    (-10.0, 80.0),
    (20.0, 100.0),
    (30.0, 70.0),
    (0.0, 0.0),
    (30.0, 70.0),
    (30.0, 70.0),
    (40.0, 40.0),
    (120.0, 260.0),
    (80.0, 210.0),
    (30.0, 70.0),
    (100.0, 160.0),
    (0.0, 0.0),
    (80.0, 210.0),
    (20.0, 100.0),
    (-10.0, 80.0),
    (20.0, 100.0),
    (50.0, 150.0),
    (100.0, 250.0),
];

pub(crate) fn captain_offset(kind: FighterKind) -> Vec3 {
    let (x, y) = CAPTAIN_OFFSETS[kind as usize];
    Vec3::new(x, y, 0.0)
}

fn capture_captain(f: &mut Fighter, catcher_port: u8, holder: Holder) {
    drop_own_catch(f);
    f.grab.capture = Some(catcher_port);
    f.grab.holder = Some(holder);
    f.grab.captain_no_update = f.is_grounded();
    f.facing = holder.facing.flipped();
    f.become_airborne();
    status::set_status(f, Status::CaptureCaptain, 0.0, StatusTiming::unknown());
    f.grab.capture_immune = true;
    physics::stop_all(&mut f.physics);
}

fn update_capture_captain(f: &mut Fighter, holder: Holder) {
    if f.grab.captain_no_update {
        return;
    }
    let (x, y) = CAPTAIN_OFFSETS[f.kind as usize];
    let want = holder.anchor - Vec3::new(x * holder.facing.sign(), y, 0.0);
    let delta = want - f.pos;
    let d2 = delta.length_squared();
    if d2 > 180.0 * 180.0 {
        let scale = 180.0 / ssb_engine::math::sqrt(d2);
        f.pos += delta * scale;
    } else {
        f.pos = want;
    }
}

/// `ftCommonCaptureWaitSetStatus` @ 0x8014AA58.
fn set_capture_wait(f: &mut Fighter) {
    status::set_status(f, Status::CaptureWait, 0.0, StatusTiming::unknown());
    f.grab.capture_immune = true;
}

/// `ftCommonThrownSetStatusQueue` / `ftCommonThrownSetStatusImmediate`.
fn set_thrown(f: &mut Fighter, status: Status, queue: Option<Status>) {
    f.situation = Situation::Air;
    f.floor = None;
    f.physics.jumps_used = 1;
    let timing = match thrown_length(f.kind, status) {
        Some(len) => StatusTiming::frames(len),
        None => StatusTiming::unknown(),
    };
    status::set_status(f, status, 0.0, timing);
    f.grab.capture_immune = true;
    f.grab.thrown_queue = queue;
}

/// `ftCommonThrownProcUpdate` @ 0x8014AAF0: the start statuses advance to the
/// queued one, except while Donkey Kong is still lifting.
fn update_thrown(f: &mut Fighter) {
    if !matches!(
        f.status.status,
        AnyStatus::Common(
            Status::ThrownDonkeyF | Status::ThrownMarioBStart | Status::ThrownFoxFStart
        )
    ) || !f.status.animation_ended()
    {
        return;
    }
    let lifting = f
        .grab
        .holder
        .is_some_and(|h| is_donkey(h.kind) && h.status == Status::ThrowF);
    if !lifting {
        if let Some(next) = f.grab.thrown_queue {
            set_thrown(f, next, None);
        }
    }
}

/// `ftCommonCaptureTrappedInitBreakoutVars` @ 0x8014E3EC.
pub(crate) fn init_breakout(f: &mut Fighter, wait: i32) {
    f.grab.breakout_wait = wait;
    f.grab.breakout_lr = 0;
    f.grab.breakout_ud = 0;
}

/// `ftCommonCaptureTrappedUpdateBreakoutVars` @ 0x8014E400.
pub(crate) fn update_breakout(f: &mut Fighter) -> bool {
    let taps = tapped(f);
    let mut is_mash = false;
    if taps.contains(N64Buttons::A) || taps.contains(N64Buttons::B) || taps.contains(N64Buttons::Z)
    {
        is_mash = true;
        f.grab.breakout_wait -= 1;
    }
    let (lr, ud) = (f.grab.breakout_lr, f.grab.breakout_ud);
    let (x, y) = (f.stick.x as i32, f.stick.y as i32);
    if x < -CAPTURE_MASH_STICK_RANGE_MIN {
        f.grab.breakout_lr = -1;
    }
    if x > CAPTURE_MASH_STICK_RANGE_MIN {
        f.grab.breakout_lr = 1;
    }
    if y < -CAPTURE_MASH_STICK_RANGE_MIN {
        f.grab.breakout_ud = -1;
    }
    if y > CAPTURE_MASH_STICK_RANGE_MIN {
        f.grab.breakout_ud = 1;
    }
    if f.grab.breakout_lr != lr || f.grab.breakout_ud != ud {
        is_mash = true;
        f.grab.breakout_wait -= 1;
    }
    is_mash
}

/// The literal damage `ftCommonCaptureShoulderedSetStatus` stales.
const SHOULDERED_DAMAGE: i32 = 8;

/// `ftCommonCaptureShoulderedSetStatus` @ 0x8014E558 (US breakout base 14).
/// Donkey Kong's forward throw deals 8, staled, on the lift. Returns the
/// damage dealt.
fn set_shouldered(f: &mut Fighter, staled: StaledThrow) -> i32 {
    set_thrown(f, Status::Shouldered, None);
    init_breakout(f, (f32::from(f.damage) * 0.08 + 14.0) as i32);
    let damage = if f.invincible_frames == 0 {
        staled.damage
    } else {
        0
    };
    f.damage = f.damage.saturating_add(damage.max(0) as u16);
    damage
}

/// `ftCommonCaptureShoulderedProcInterrupt` @ 0x8014E4D4.
fn update_shouldered(f: &mut Fighter) {
    update_breakout(f);
    if f.grab.breakout_wait <= 0 {
        f.grab.send(GrabEvent::BreakoutKnockback);
        apply_capture_knockback(f);
    }
}

/// `ftCommonCaptureApplyCaptureKnockback` @ 0x8014E2A8: the escaping
/// fighter's own knockback, pushed away from its catcher.
fn apply_capture_knockback(f: &mut Fighter) {
    let Some(holder) = f.grab.holder else {
        return;
    };
    lose_grip(f);
    if !f.is_grounded() {
        f.physics.jumps_used = 1;
        f.pos.z = 0.0;
        f.physics.vel_air.z = 0.0;
    }
    let (angle, kbs, kbw, kbb) = CAPTURE_KNOCKBACK_CAPTURE;
    let knockback = attack::knockback(
        f.damage,
        0,
        0,
        kbw,
        kbs,
        kbb,
        f.attributes.weight,
        holder.handicap,
        f.handicap,
    );
    let lr = if f.pos.x < holder.pos.x { 1.0 } else { -1.0 };
    attack::init_damage_vars(f, None, 0, knockback, angle, lr);
}

/// `ftCommonCaptureApplyCatchKnockback` @ 0x8014E1D0: the catcher's recoil
/// when the held fighter escapes.
fn apply_catch_knockback(f: &mut Fighter, capture_handicap: u8) {
    let (angle, kbs, kbw, kbb) = CAPTURE_KNOCKBACK_CATCH;
    let knockback = attack::knockback(
        f.damage,
        0,
        0,
        kbw,
        kbs,
        kbb,
        f.attributes.weight,
        capture_handicap,
        f.handicap,
    );
    let lr = f.facing.sign();
    attack::init_damage_vars(f, None, 0, knockback, angle, lr);
}

/// Drops the link on the held side (`ftCommonThrownReleaseFighterLoseGrip`
/// plus `capture_gobj = NULL`), leaving the fighter airborne where it is.
fn lose_grip(f: &mut Fighter) {
    f.grab.capture = None;
    f.grab.holder = None;
    f.grab.thrown_queue = None;
    f.grab.capture_immune = false;
    f.grab.captain_no_update = false;
    f.is_invisible = false;
    if f.is_grounded() && f.floor.is_none() {
        f.become_airborne();
    }
}

/// `ftCommonThrownReleaseThrownUpdateStats` @ 0x8014AFD0 and
/// `ftCommonThrownSetStatusDamageRelease` @ 0x8014B330. Returns the damage
/// dealt, for the catcher's stale queue.
fn release_with(
    f: &mut Fighter,
    desc: ThrowHitDesc,
    lr: Option<f32>,
    shield_catch: bool,
    staled: StaledThrow,
) -> i32 {
    let Some(holder) = f.grab.holder else {
        return 0;
    };
    if lr.is_some() {
        // `ftCommonThrownProcPhysics(catch_gobj)` runs just before release.
        if f.status.status != Status::CaptureCaptain {
            f.pos = held_attachment(f, holder);
        }
    }
    lose_grip(f);
    if lr.is_some() || !f.is_grounded() {
        f.become_airborne();
    }
    let knockback = attack::knockback(
        f.damage,
        desc.damage,
        desc.damage,
        desc.kb_weight,
        desc.kb_scale,
        desc.kb_base,
        f.attributes.weight,
        holder.handicap,
        f.handicap,
    );
    let lr = lr.unwrap_or(if f.pos.x < holder.pos.x { 1.0 } else { -1.0 });
    let mut damage = staled.damage;
    if shield_catch {
        damage = (damage as f32 * 0.5 + 0.999) as i32;
    }
    if f.invincible_frames > 0 {
        damage = 0;
    }
    attack::init_damage_vars(
        f,
        desc.status.map(AnyStatus::Common),
        damage,
        knockback,
        desc.angle,
        lr,
    );
    damage
}

// ---------------------------------------------------------------------------
// Donkey Kong's cargo (`ftdonkeythrowf*.c`)
// ---------------------------------------------------------------------------

fn set_donkey(f: &mut Fighter, status: DonkeyStatus, frame: f32, timing: StatusTiming) {
    status::set_any_status(f, AnyStatus::Donkey(status), frame, timing);
}

/// `ftDonkeyThrowFWaitSetStatus` @ 0x8014D49C.
pub fn set_donkey_throwf_wait(f: &mut Fighter) {
    set_donkey(f, DonkeyStatus::ThrowFWait, 0.0, StatusTiming::unknown());
}

fn walk_status(stick_x: i8) -> DonkeyStatus {
    match status::walk_status_for(stick_x) {
        Status::WalkSlow => DonkeyStatus::ThrowFWalkSlow,
        Status::WalkMiddle => DonkeyStatus::ThrowFWalkMiddle,
        _ => DonkeyStatus::ThrowFWalkFast,
    }
}

fn walk_length(status: AnyStatus) -> f32 {
    match status {
        AnyStatus::Donkey(DonkeyStatus::ThrowFWalkSlow) => DONKEY_THROW_WALK_LENGTHS[0],
        AnyStatus::Donkey(DonkeyStatus::ThrowFWalkMiddle) => DONKEY_THROW_WALK_LENGTHS[1],
        _ => DONKEY_THROW_WALK_LENGTHS[2],
    }
}

/// `ftDonkeyThrowFWalkSetStatusParam` @ 0x8014D68C.
fn set_donkey_throwf_walk(f: &mut Fighter, frame: f32) {
    set_donkey(f, walk_status(f.stick.x), frame, StatusTiming::unknown());
}

/// `ftDonkeyThrowFFSetStatus` @ 0x8014DF14.
fn set_donkey_throwff(f: &mut Fighter, is_turn: bool) {
    let status = if f.is_grounded() {
        DonkeyStatus::ThrowFF
    } else {
        DonkeyStatus::ThrowAirFF
    };
    set_donkey(f, status, 0.0, StatusTiming::frames(DONKEY_THROWFF_LENGTH));
    f.grab.throw_desc = Some(DONKEY_THROW_FF);
    f.grab.capture_immune = true;
    f.grab.throwff_turn_tics = 0;
    if is_turn {
        f.facing = f.facing.flipped();
        f.grab.throwff_turn_tics = THROWFF_TURN_FRAMES;
    }
}

/// `ftDonkeyThrowFFCheckInterruptThrowFCommon` @ 0x8014DFA8.
fn check_donkey_throwff(f: &mut Fighter) -> bool {
    let taps = tapped(f);
    if !(taps.contains(N64Buttons::A) || taps.contains(N64Buttons::B)) {
        return false;
    }
    let x = f.stick.x as i32;
    let is_turn = x.abs() >= THROWFF_TURN_STICK_RANGE_MIN
        && f.stick.forward(f.facing) < 0
        && !f.is_grounded();
    set_donkey_throwff(f, is_turn);
    true
}

/// `ftDonkeyThrowFKneeBendCheckInterruptThrowFCommon` @ 0x8014D9B8.
fn check_donkey_kneebend(f: &mut Fighter) -> bool {
    let input = status::jump_input_type(f, status::KNEEBEND_STICK_MIN);
    if input == JumpInput::None {
        return false;
    }
    set_donkey(
        f,
        DonkeyStatus::ThrowFKneeBend,
        0.0,
        StatusTiming::unknown(),
    );
    f.status.jump_force = f.stick.y;
    f.status.jump_input = input;
    f.status.is_shorthop = false;
    true
}

/// `ftDonkeyThrowFFallCheckInterruptPass` @ 0x8014DC08.
fn check_donkey_pass(f: &mut Fighter) -> bool {
    let passable = f.floor.is_some_and(|s| s.passable());
    if f.stick.y as i32 <= status::PASS_STICK_MIN
        && f.stick.tap_y < status::PASS_BUFFER_TICS_MAX
        && passable
    {
        f.ignore_line = f.floor.map(|s| s.line);
        set_donkey_throwf_fall(f);
        f.physics.vel_air.y = 0.0;
        f.stick.tap_y = status::STICKBUFFER_MAX;
        return true;
    }
    false
}

/// `ftDonkeyThrowFTurnCheckInterruptThrowFCommon` @ 0x8014D810.
fn check_donkey_turn(f: &mut Fighter) -> bool {
    if f.stick.forward(f.facing) <= status::TURN_STICK_MIN {
        set_donkey(
            f,
            DonkeyStatus::ThrowFTurn,
            0.0,
            StatusTiming::frames(DONKEY_THROWF_TURN_LENGTH),
        );
        return true;
    }
    false
}

/// `ftDonkeyThrowFFallSetStatus` @ 0x8014DA98.
fn set_donkey_throwf_fall(f: &mut Fighter) {
    let fastfall = f.physics.is_fastfall;
    f.become_airborne();
    set_donkey(f, DonkeyStatus::ThrowFFall, 0.0, StatusTiming::unknown());
    f.physics.is_fastfall = fastfall;
    physics::clamp_air_vel_x(&mut f.physics, f.attributes.air_speed_max_x);
}

/// `ftDonkeyThrowFJumpSetStatus` @ 0x8014DAF8.
fn set_donkey_throwf_jump(f: &mut Fighter) {
    f.become_airborne();
    set_donkey(f, DonkeyStatus::ThrowFFall, 0.0, StatusTiming::unknown());
    let (vel_x, vel_y) = match f.status.jump_input {
        JumpInput::Button => status::jump_force_button(f.input.stick_x, f.status.is_shorthop),
        _ => (f.input.stick_x as f32, f.status.jump_force as f32),
    };
    // `jump_force` is read after `ftMainSetStatus`, which does not clear it.
    let attr = f.attributes;
    f.physics.vel_air.y = vel_y * attr.jump_height_mul + attr.jump_height_base;
    f.physics.vel_air.x = vel_x * attr.jump_vel_x;
    f.stick.tap_y = status::STICKBUFFER_MAX;
}

/// `ftDonkeyThrowFLandingSetStatus` @ 0x8014DCA4.
fn set_donkey_throwf_landing(f: &mut Fighter) {
    set_donkey(f, DonkeyStatus::ThrowFLanding, 0.0, StatusTiming::unknown());
    f.status.jump_force = 0;
}

/// The cargo Wait/Walk interrupt head: heavy-item throw (no items), cargo
/// throw, jump, platform drop.
fn cargo_common_interrupt(f: &mut Fighter) -> bool {
    check_donkey_throwff(f) || check_donkey_kneebend(f) || check_donkey_pass(f)
}

/// `ftDonkeyThrowFDamageSetStatus` @ 0x8014E0E0: hit while carrying but
/// resisted, so Donkey Kong staggers and keeps his hold. The forced status
/// makes `ftCommonDamageInitDamageVars` treat it as a tumble-level hit, so he
/// is always launched airborne.
pub fn set_donkey_throwf_damage(f: &mut Fighter, knockback: f32, angle: i32, lr: f32) {
    attack::init_damage_vars(
        f,
        Some(AnyStatus::Donkey(DonkeyStatus::ThrowFDamage)),
        0,
        knockback,
        angle,
        lr,
    );
}

/// Whether a hit on a fighter holding someone is absorbed by Donkey Kong's
/// cargo stance (`ftCommonDamageCheckCatchResist`'s cargo branch): only
/// hits below tumble strength.
pub fn cargo_resists(f: &Fighter, knockback: f32) -> bool {
    is_donkey(f.kind)
        && is_cargo(f.status.status)
        && attack::damage_level(attack::hitstun_frames(knockback)) < 3
}

/// A catcher hit hard enough to drop its hold: the held fighter takes the
/// descriptor's `[1]` knockback (`ftCommonDamageSetDamageStatus`'s last
/// branch).
pub fn release_on_hit(f: &mut Fighter) {
    if f.grab.catch.take().is_some() {
        f.grab.catch_kind = None;
        f.grab.capture_immune = false;
        let desc = f.grab.throw_desc.map(|d| d[1]).unwrap_or(MARIO_CATCH[1]);
        f.grab.send(GrabEvent::DamageRelease {
            desc,
            shield_catch: f.grab.is_shield_catch,
            staled: StaledThrow::of(f, desc.damage),
        });
    }
}

/// The held side of `ftCommonDamageUpdateMain`'s capture branch when the hit
/// breaks the hold: `ftCommonThrownDecideFighterLoseGrip(catcher, held)`
/// drops both links, and the catcher is told to take
/// `ftCommonThrownSetStatusNoDamageRelease`.
pub fn release_on_capture_hit(f: &mut Fighter) {
    if f.grab.capture.is_some() {
        f.grab.send(GrabEvent::CaptureHitRelease);
        lose_grip(f);
    }
}

/// `dFTCommonThrownNoDamageKnockback`: `{ -1, 0, 361, 0, 0, 20, 0 }`.
const NO_DAMAGE_KNOCKBACK: ThrowHitDesc = desc(None, 0, 361, 0, 0, 20);

/// `ftCommonThrownSetStatusNoDamageRelease` @ 0x8014B5B4, on the catcher.
/// The attack handicap is the source's literal `9`, and the fighter keeps
/// its facing (`lr = fp->lr`).
fn set_no_damage_release(f: &mut Fighter) {
    f.pos.z = 0.0;
    let d = NO_DAMAGE_KNOCKBACK;
    let knockback = attack::knockback(
        f.damage,
        d.damage,
        d.damage,
        d.kb_weight,
        d.kb_scale,
        d.kb_base,
        f.attributes.weight,
        crate::stale::HANDICAP_DEFAULT,
        f.handicap,
    );
    let lr = f.facing.sign();
    attack::init_damage_vars(f, None, 0, knockback, d.angle, lr);
}

/// `ftCommonThrownDecideDeadResult` @ 0x8014AF2C, the KO'd side.
pub fn release_on_dead(f: &mut Fighter) {
    if f.grab.catch.take().is_some() || f.grab.capture.take().is_some() {
        f.grab.send(GrabEvent::Dead);
    }
    f.grab.catch_kind = None;
    f.grab.holder = None;
    f.grab.thrown_queue = None;
    f.grab.capture_immune = false;
    f.grab.is_catchstatus = false;
}

// ---------------------------------------------------------------------------
// Status machine hooks
// ---------------------------------------------------------------------------

/// Runs `proc_update`/`proc_interrupt` for the grab statuses. Returns `false`
/// when the current status is not one of them.
pub fn update(f: &mut Fighter) -> bool {
    match f.status.status {
        // `ftCommonCatchProcUpdate`. Only Samus's and Link's scripts set
        // flag2.
        AnyStatus::Common(Status::Catch) => {
            if f.grab.catch_pull_frame_begin > 0.0 {
                f.grab.catch_pull_frame_begin -= f.grab.catch_pull_anim_frames;
                if f.grab.catch_pull_frame_begin <= 0.0 {
                    f.grab.catch_pull_frame_begin = 0.0;
                }
            }
            let frame = f.status.anim_frame;
            if let Some((at, flag1, flag2)) = catch_pull_flags(f.kind) {
                if frame >= at && frame - f.status.timing.anim_speed < at {
                    f.grab.catch_pull_frame_begin = flag2;
                    f.grab.catch_pull_anim_frames = flag2 / flag1;
                }
            }
            if f.status.animation_ended() {
                f.grab.is_catchstatus = false;
                status::set_wait(f);
            }
        }
        // `ftCommonCatchPullProcUpdate` @ 0x80149EC0.
        AnyStatus::Common(Status::CatchPull) => {
            if f.status.animation_ended() {
                set_catch_wait(f);
                f.grab.send(GrabEvent::GotoPulledWait);
            }
        }
        // `ftCommonCatchWaitProcInterrupt` @ 0x80149FCC.
        AnyStatus::Common(Status::CatchWait) => {
            if f.grab.throw_wait != 0 {
                f.grab.throw_wait -= 1;
            }
            check_throw(f);
        }
        AnyStatus::Common(Status::ThrowF | Status::ThrowB) => update_throw(f),
        // `ftCommonCapturePulledProcPhysics`'s status switch.
        AnyStatus::Common(Status::CapturePulled) => {
            if f.grab.is_goto_pulled_wait {
                set_capture_wait(f);
            }
        }
        AnyStatus::Common(Status::CaptureWait | Status::CaptureYoshi | Status::CaptureCaptain) => {}
        AnyStatus::Common(Status::Shouldered) => update_shouldered(f),
        s if is_thrown(s) => update_thrown(f),
        AnyStatus::Donkey(DonkeyStatus::ThrowFWait) => {
            if !(cargo_common_interrupt(f) || check_donkey_turn(f))
                && f.stick.forward(f.facing) >= 8
            {
                set_donkey_throwf_walk(f, 0.0);
            }
        }
        // `ftDonkeyThrowFWalkProcInterrupt` @ 0x8014D590.
        AnyStatus::Donkey(
            DonkeyStatus::ThrowFWalkSlow
            | DonkeyStatus::ThrowFWalkMiddle
            | DonkeyStatus::ThrowFWalkFast,
        ) => {
            if cargo_common_interrupt(f) {
                return true;
            }
            if f.stick.forward(f.facing) < 0 || (f.stick.x as i32).abs() < 8 {
                set_donkey_throwf_wait(f);
                return true;
            }
            let next = AnyStatus::Donkey(walk_status(f.stick.x.saturating_abs()));
            if next != f.status.status {
                let frame = (walk_length(next)
                    * (f.status.anim_frame / walk_length(f.status.status)))
                    as i32;
                set_donkey_throwf_walk(f, frame as f32);
            }
        }
        // `ftDonkeyThrowFTurnProcUpdate` / `...ProcInterrupt`.
        AnyStatus::Donkey(DonkeyStatus::ThrowFTurn) => {
            let frame = f.status.anim_frame;
            if frame >= 1.0 && frame - f.status.timing.anim_speed < 1.0 {
                f.facing = f.facing.flipped();
                f.physics.vel_ground.x = -f.physics.vel_ground.x;
            }
            if f.status.animation_ended() {
                set_donkey_throwf_wait(f);
            } else if !check_donkey_throwff(f) {
                check_donkey_kneebend(f);
            }
        }
        // `ftDonkeyThrowFKneeBendProcUpdate` / `...ProcInterrupt`. The
        // status plays at speed 0; `anim_frame` is the source's
        // `kneebend_anim_frame` counter.
        AnyStatus::Donkey(DonkeyStatus::ThrowFKneeBend) => {
            let frame = f.status.anim_frame;
            if f.status.jump_input == JumpInput::Button
                && frame <= status::KNEEBEND_SHORTHOP_FRAMES
                && f.stick.jump_released
            {
                f.status.is_shorthop = true;
            }
            if f.attributes.kneebend_anim_length <= frame {
                set_donkey_throwf_jump(f);
            } else if !check_donkey_throwff(f) && f.status.jump_force < f.stick.y {
                f.status.jump_force = f.stick.y;
            }
        }
        AnyStatus::Donkey(DonkeyStatus::ThrowFFall) => {
            check_donkey_throwff(f);
        }
        // `ftDonkeyThrowFLandingProcUpdate` @ 0x8014DC50 tests
        // `landing_anim_frame <= 4.0F` after incrementing it, which is true on
        // the first update: the cargo landing lasts one frame.
        AnyStatus::Donkey(DonkeyStatus::ThrowFLanding) => {
            if f.status.anim_frame <= 4.0 {
                set_donkey_throwf_wait(f);
            }
        }
        // `ftDonkeyThrowFDamageProcUpdate` @ 0x8014E050.
        AnyStatus::Donkey(DonkeyStatus::ThrowFDamage) => {
            if f.hitstun == 0 {
                if f.is_grounded() {
                    set_donkey_throwf_wait(f);
                } else {
                    set_donkey_throwf_fall(f);
                }
            }
        }
        // `ftDonkeyThrowFFProcUpdate` @ 0x8014DD00.
        AnyStatus::Donkey(DonkeyStatus::ThrowFF | DonkeyStatus::ThrowAirFF) => {
            if f.grab.throwff_turn_tics > 0 {
                f.grab.throwff_turn_tics -= 1;
            }
            let frame = f.status.anim_frame;
            if frame >= DONKEY_THROWFF_RELEASE_FRAME
                && frame - f.status.timing.anim_speed < DONKEY_THROWFF_RELEASE_FRAME
                && f.grab.catch.is_some()
            {
                release_thrown(f, -f.facing.sign());
            }
            if f.status.animation_ended() {
                status::set_wait_or_fall(f);
            }
        }
        _ => return false,
    }
    true
}

/// `ftCommonCapturePulledRotateScale`: place the held fighter's TopN so its
/// first child lands exactly on the catcher's heavy-item joint. The source
/// scales the negative child translation by the held fighter's own TopN
/// scale (`attr->size`) and transforms it through that joint's matrix.
fn held_attachment(f: &Fighter, holder: Holder) -> Vec3 {
    match (holder.anchor_transform, f.grab.held_child_offset) {
        (Some(joint), Some(child)) => joint.point(held_child_point(child, f.attributes.size)),
        _ => holder.anchor,
    }
}

/// `this_pos = -child->translate * TopN->scale`, component by component.
/// TopN's scale is uniform (`ftManagerMakeFighter` writes `attr->size` to
/// all three axes).
fn held_child_point(child: Vec3, size: f32) -> Vec3 {
    Vec3::new(-child.x * size, -child.y * size, -child.z * size)
}

/// Refreshes the held X/Z location after the animation runtime advances its
/// child pose. `CapturePulled` and `CaptureWait` keep Y under floor collision;
/// thrown and shouldered statuses take all three attachment coordinates.
pub fn refresh_held_attachment(f: &mut Fighter) {
    if !is_held(f.status.status) {
        return;
    }
    let Some(holder) = f.grab.holder else { return };
    if f.status.status == Status::CaptureCaptain {
        update_capture_captain(f, holder);
        return;
    }
    let point = held_attachment(f, holder);
    f.pos.x = point.x;
    f.pos.z = point.z;
    if !matches!(
        f.status.status,
        AnyStatus::Common(Status::CapturePulled | Status::CaptureWait | Status::CaptureYoshi)
    ) {
        f.pos.y = point.y;
    }
}

/// Physics for a held fighter: `ftCommonCapturePulledProcPhysics` or
/// `ftCommonThrownProcPhysics`, then the matching map callback against the
/// catcher's floor line. Returns `false` when the fighter is not held.
pub fn tick_held<I, F>(f: &mut Fighter, floors: F) -> bool
where
    F: Fn() -> I,
    I: IntoIterator<Item = (u16, Segment)>,
{
    if !is_held(f.status.status) {
        return false;
    }
    let Some(holder) = f.grab.holder else {
        // The link is gone without a release event; do not leave the
        // fighter pinned.
        lose_grip(f);
        status::set_fall(f);
        return true;
    };
    f.physics.vel_air = Vec3::ZERO;
    f.physics.vel_ground = Vec3::ZERO;
    if f.status.status == Status::CaptureYoshi {
        let attachment = held_attachment(f, holder);
        crate::capture_yoshi::update_held(f, attachment);
        return true;
    }
    if f.status.status == Status::CaptureCaptain {
        update_capture_captain(f, holder);
        return true;
    }
    let pulled = matches!(
        f.status.status,
        AnyStatus::Common(Status::CapturePulled | Status::CaptureWait)
    );
    let attachment = held_attachment(f, holder);
    f.pos.x = attachment.x;
    f.pos.z = attachment.z;
    if !pulled {
        f.pos.y = attachment.y;
    }
    if pulled {
        // `ftCommonCapturePulledProcMap` / `ftCommonCaptureWaitProcMap`.
        let Some(line) = holder.floor_line else {
            f.situation = Situation::Air;
            f.floor = None;
            return true;
        };
        let on_line = || floors().into_iter().filter(move |(l, _)| *l == line);
        match collision::floor_height(on_line(), f.pos.x) {
            Some(below) => {
                let dist = below.y - f.pos.y;
                if f.status.status == Status::CaptureWait || dist >= 0.0 {
                    f.pos.y += dist;
                    f.situation = Situation::Ground;
                    f.floor = Some(crate::ground::Standing {
                        line: below.line,
                        flags: below.flags,
                        normal: below.normal,
                    });
                    f.physics.jumps_used = 0;
                } else {
                    f.pos.y += dist * 0.5;
                    f.situation = Situation::Air;
                    f.floor = None;
                    f.physics.jumps_used = 1;
                }
            }
            None => {
                let edge_x = if holder.facing == Facing::Right {
                    f32::MAX
                } else {
                    f32::MIN
                };
                if let Some(edge) = collision::line_edge(on_line(), edge_x) {
                    f.pos.y = if f.status.status == Status::CaptureWait {
                        edge.y
                    } else {
                        f.pos.y + (edge.y - f.pos.y) * 0.5
                    };
                }
                f.situation = Situation::Air;
                f.floor = None;
                f.physics.jumps_used = 1;
            }
        }
    } else {
        // `ftCommonThrownProcMap`: the fighter stays airborne; the floor
        // lookup only feeds the shadow.
        f.situation = Situation::Air;
        f.floor = None;
    }
    true
}

/// Whether a catcher in a holding status just lost its floor. Called from the
/// grounded tick when the walk off an edge left no floor.
pub fn on_ground_lost(f: &mut Fighter) -> bool {
    match f.status.status {
        // `ftCommonCatchProcMap`: `mpCommonCheckFighterOnEdge == FALSE`.
        AnyStatus::Common(
            Status::Catch | Status::CatchPull | Status::CatchWait | Status::ThrowF | Status::ThrowB,
        ) => {
            f.become_airborne();
            release_on_edge(f);
            true
        }
        // `ftDonkeyThrowFCommonProcMap` → `ftDonkeyThrowFFallSetStatus`.
        s if is_cargo(s) => {
            set_donkey_throwf_fall(f);
            true
        }
        // `ftDonkeyThrowFFProcMap` → `ftDonkeyThrowFFSwitchStatusAir`.
        AnyStatus::Donkey(DonkeyStatus::ThrowFF) => {
            f.become_airborne();
            let (frame, timing) = (f.status.anim_frame, f.status.timing);
            set_donkey(f, DonkeyStatus::ThrowAirFF, frame, timing);
            f.grab.capture_immune = true;
            true
        }
        _ => false,
    }
}

/// Landing for the cargo air statuses. Returns whether it handled it.
pub fn on_landing(f: &mut Fighter, floor_y: f32) -> bool {
    match f.status.status {
        // `ftDonkeyThrowFFallProcMap` @ 0x8014DA30.
        AnyStatus::Donkey(DonkeyStatus::ThrowFFall) => {
            let skip = f.physics.vel_air.y > THROWFFALL_SKIPLANDING_VEL_Y_MAX;
            f.land(floor_y);
            if skip {
                set_donkey_throwf_wait(f);
            } else {
                set_donkey_throwf_landing(f);
            }
            true
        }
        // `ftDonkeyThrowAirFFProcMap` → `ftDonkeyThrowAirFFSwitchStatusGround`.
        AnyStatus::Donkey(DonkeyStatus::ThrowAirFF) => {
            f.land(floor_y);
            let (frame, timing) = (f.status.anim_frame, f.status.timing);
            set_donkey(f, DonkeyStatus::ThrowFF, frame, timing);
            f.grab.throwff_turn_tics = 0;
            f.grab.capture_immune = true;
            true
        }
        AnyStatus::Donkey(DonkeyStatus::ThrowFDamage) => {
            f.land(floor_y);
            true
        }
        _ => false,
    }
}

/// The grounded physics status a cargo status borrows: the walks use the
/// common walk physics, everything else plain friction.
pub fn ground_physics_status(status: AnyStatus) -> Option<Status> {
    match status {
        AnyStatus::Donkey(DonkeyStatus::ThrowFWalkSlow) => Some(Status::WalkSlow),
        AnyStatus::Donkey(DonkeyStatus::ThrowFWalkMiddle) => Some(Status::WalkMiddle),
        AnyStatus::Donkey(DonkeyStatus::ThrowFWalkFast) => Some(Status::WalkFast),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Two-fighter phases
// ---------------------------------------------------------------------------

fn holder_of(f: &Fighter) -> Holder {
    Holder {
        kind: f.kind,
        pos: f.pos,
        facing: f.facing,
        status: f.status.status,
        anchor: f.grab.anchor.unwrap_or(f.pos),
        anchor_transform: f.grab.anchor_transform,
        floor_line: f.floor.map(|s| s.line),
        percent: f.damage,
        handicap: f.handicap,
    }
}

/// Delivers `from`'s queued events to `to` and refreshes the held side's
/// snapshot of its catcher. Call after each fighter's tick and after the
/// hit phase.
pub fn exchange(from: &mut Fighter, to: &mut Fighter) {
    if to.grab.capture == Some(from.port) {
        to.grab.holder = Some(holder_of(from));
    }
    let events = core::mem::take(&mut from.grab.outbox);
    for event in events.into_iter().flatten() {
        deliver(event, from, to);
    }
    if to.grab.capture == Some(from.port) {
        to.grab.holder = Some(holder_of(from));
    }
}

/// `ftParamUpdateStaleQueue(capture_fp->player, this_fp->player, ...)`,
/// which the throw paths call only when damage was dealt.
fn record_throw(catcher: &mut Fighter, held: &Fighter, staled: StaledThrow, damage: i32) {
    if damage != 0 && catcher.port != held.port {
        catcher.stale.push(staled.attack_id, staled.motion_count);
    }
}

fn deliver(event: GrabEvent, from: &mut Fighter, to: &mut Fighter) {
    match event {
        GrabEvent::Capture => capture_pulled(to, from.port, holder_of(from)),
        GrabEvent::GotoPulledWait => to.grab.is_goto_pulled_wait = true,
        GrabEvent::Thrown { first, then } => match first {
            Some(first) => set_thrown(to, first, Some(then)),
            None => set_thrown(to, then, None),
        },
        GrabEvent::Shouldered { staled } => {
            let damage = set_shouldered(to, staled);
            record_throw(from, to, staled, damage);
        }
        GrabEvent::Release {
            lr,
            desc,
            shield_catch,
            staled,
        } => {
            to.grab.holder = Some(holder_of(from));
            let damage = release_with(to, desc, Some(lr), shield_catch, staled);
            record_throw(from, to, staled, damage);
        }
        GrabEvent::DamageRelease {
            desc,
            shield_catch,
            staled,
        } => {
            let damage = release_with(to, desc, None, shield_catch, staled);
            record_throw(from, to, staled, damage);
        }
        GrabEvent::CaptureHitRelease => {
            if to.grab.catch.take().is_some() {
                to.grab.catch_kind = None;
                to.grab.capture_immune = false;
                set_no_damage_release(to);
            }
        }
        GrabEvent::LoseGrip => {
            lose_grip(to);
            if to.is_grounded() {
                status::set_wait(to);
            } else {
                status::set_fall(to);
            }
        }
        GrabEvent::BreakoutKnockback => {
            if to.grab.catch.take().is_some() {
                to.grab.catch_kind = None;
                to.grab.capture_immune = false;
                apply_catch_knockback(to, from.handicap);
            }
        }
        GrabEvent::Dead => {
            to.grab.catch = None;
            to.grab.catch_kind = None;
            lose_grip(to);
            status::set_wait_or_fall(to);
        }
        GrabEvent::CaptureYoshi => crate::capture_yoshi::capture(to, from.port, holder_of(from)),
        GrabEvent::CaptureCaptain => capture_captain(to, from.port, holder_of(from)),
        GrabEvent::YoshiEggStage(stage) => to.egg.stage = stage,
    }
}

/// `ftMainSearchFighterCatch` + `ftMainProcSearchCatch` for one catcher
/// against one other fighter. On a catch, the catcher enters `CatchPull`
/// here and the grabbed fighter's `CapturePulled` is queued for
/// [`exchange`]. Yoshi's Egg Lay searches with its own box and
/// `proc_catch`/`proc_capture` pair.
pub fn search_catch(catcher: &mut Fighter, other: &Fighter) -> bool {
    if !catcher.grab.is_catchstatus {
        return false;
    }
    let egg_lay = crate::yoshi::egg_lay_searching(catcher);
    let dive = crate::captain::dive_searching(catcher);
    if !egg_lay
        && !dive
        && (catcher.status.status != Status::Catch
            || !catch_coll_frames(catcher.kind).contains(&catcher.status.anim_frame))
    {
        return false;
    }
    if other.grab.capture_immune || other.invincible_frames > 0 || other.stocks <= 0 {
        return false;
    }
    let colls: &[(Hitbox, u8)] = if egg_lay {
        core::slice::from_ref(&crate::yoshi::EGG_LAY_CATCH)
    } else if dive {
        if catcher.status.anim_frame < 14.0 {
            &crate::captain::DIVE_CATCH
        } else {
            &crate::captain::DIVE_CATCH[..1]
        }
    } else {
        catch_colls(catcher.kind)
    };
    let hit = colls.iter().any(|(hitbox, joint)| {
        attack::spheres_overlap(
            catcher.joint_world(*joint, hitbox.offset),
            hitbox.radius,
            other.pos,
            attack::MARIO_HURTBOX_RADIUS,
        )
    });
    if !hit {
        return false;
    }
    if dive {
        crate::captain::dive_catch(catcher, other);
    } else if egg_lay {
        crate::yoshi::catch(catcher, other);
        catcher.grab.send(GrabEvent::CaptureYoshi);
    } else {
        catch_pull(catcher, other);
        catcher.grab.send(GrabEvent::Capture);
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fighter::FighterKind;

    #[test]
    fn captain_capture_freezes_grounded_victim_and_pulls_airborne_victim() {
        let catcher = Fighter::new(FighterKind::Captain, 0, 4);
        let holder = holder_of(&catcher);
        let mut victim = Fighter::new(FighterKind::Mario, 1, 4);
        victim.pos = Vec3::new(500.0, 0.0, 0.0);
        victim.situation = Situation::Ground;
        capture_captain(&mut victim, catcher.port, holder);
        assert_eq!(victim.status.status, Status::CaptureCaptain);
        assert!(!victim.is_grounded());
        update_capture_captain(&mut victim, holder);
        assert_eq!(victim.pos.x, 500.0);

        let mut airborne = Fighter::new(FighterKind::Mario, 2, 4);
        airborne.pos = Vec3::new(500.0, 0.0, 0.0);
        capture_captain(&mut airborne, catcher.port, holder);
        update_capture_captain(&mut airborne, holder);
        assert!(airborne.pos.x < 500.0 && airborne.pos.x >= 320.0);
    }

    fn grounded(kind: FighterKind, port: u8, x: f32) -> Fighter {
        let mut f = Fighter::new(kind, port, 4);
        f.pos = Vec3::new(x, 0.0, 0.0);
        f.situation = Situation::Ground;
        f.floor = Some(crate::ground::Standing {
            line: 0,
            flags: 0,
            normal: ssb_engine::math::Vec2::new(0.0, 1.0),
        });
        f
    }

    fn floor() -> impl Fn() -> core::iter::Once<(u16, Segment)> {
        || {
            core::iter::once((
                0,
                Segment {
                    x1: -3000,
                    y1: 0,
                    x2: 3000,
                    y2: 0,
                    flags: 0,
                },
            ))
        }
    }

    fn tick(f: &mut Fighter) {
        f.tick(floor());
    }

    fn press(f: &mut Fighter, buttons: u16, stick_x: i8) {
        let input = ssb_engine::input::ControllerState {
            buttons: N64Buttons(buttons),
            stick_x,
            ..Default::default()
        };
        f.set_input(input, false, false);
    }

    /// Ticks `a` then `b`, exchanging after each, then runs the catch search.
    fn frame(a: &mut Fighter, b: &mut Fighter) {
        tick(a);
        exchange(a, b);
        tick(b);
        exchange(b, a);
        search_catch(a, b);
        exchange(a, b);
    }

    /// Runs the two-frame `CatchPull` out.
    fn to_catch_wait(a: &mut Fighter, b: &mut Fighter) {
        while a.status.status != Status::CatchWait {
            press(a, 0, 0);
            press(b, 0, 0);
            frame(a, b);
        }
        assert_eq!(b.status.status, Status::CaptureWait);
    }

    fn grab(a: &mut Fighter, b: &mut Fighter) {
        press(a, N64Buttons::Z, 0);
        tick(a);
        press(a, N64Buttons::Z | N64Buttons::A, 0);
        press(b, 0, 0);
        frame(a, b);
        assert_eq!(a.status.status, Status::Catch);
        for _ in 0..8 {
            press(a, 0, 0);
            press(b, 0, 0);
            frame(a, b);
            if a.grab.catch.is_some() {
                break;
            }
        }
    }

    #[test]
    fn z_and_a_grab_on_the_sixth_frame_and_pull() {
        let mut mario = grounded(FighterKind::Mario, 0, 0.0);
        let mut dummy = grounded(FighterKind::Mario, 1, 150.0);
        grab(&mut mario, &mut dummy);
        assert_eq!(mario.grab.catch, Some(1));
        assert_eq!(mario.status.status, Status::CatchPull);
        assert_eq!(dummy.grab.capture, Some(0));
        assert_eq!(dummy.status.status, Status::CapturePulled);
        assert_eq!(dummy.facing, Facing::Left);
    }

    #[test]
    fn catch_uses_animated_hand_position() {
        let mut mario = grounded(FighterKind::Mario, 0, 0.0);
        let dummy = grounded(FighterKind::Mario, 1, 400.0);
        set_catch(&mut mario);
        mario.status.anim_frame = 6.0;
        assert!(!search_catch(&mut mario, &dummy));
        mario.joint_transforms[28] = Some(JointTransform {
            axes: [
                Vec3::new(0.0, 0.0, -1.0),
                Vec3::new(0.0, 1.0, 0.0),
                Vec3::new(1.0, 0.0, 0.0),
            ],
            origin: Vec3::new(400.0, 0.0, 0.0),
        });
        assert!(search_catch(&mut mario, &dummy));
    }

    #[test]
    fn samus_grapple_beam_reaches_late_and_starts_the_pull_part_way() {
        let mut samus = grounded(FighterKind::Samus, 0, 0.0);
        let dummy = grounded(FighterKind::Mario, 1, 0.0);
        set_catch(&mut samus);
        assert_eq!(samus.status.timing.anim_length, Some(100.0));
        for _ in 0..19 {
            press(&mut samus, 0, 0);
            tick(&mut samus);
            assert!(!search_catch(&mut samus, &dummy), "no beam before frame 20");
        }
        press(&mut samus, 0, 0);
        tick(&mut samus);
        tick(&mut samus);
        assert_eq!(samus.status.anim_frame, 21.0);
        // Flag2 = 9 over flag1 = 17 frames, one step already taken.
        assert_eq!(samus.grab.catch_pull_frame_begin, 9.0 - 9.0 / 17.0);
        assert!(search_catch(&mut samus, &dummy));
        assert_eq!(samus.status.status, Status::CatchPull);
        assert_eq!(samus.status.anim_frame, 9.0 - 9.0 / 17.0);
        assert_eq!(samus.status.timing.anim_length, Some(10.0));
        assert_eq!(itemheavy_joint(FighterKind::Samus), Some(36));
        assert_eq!(
            thrown_status(FighterKind::Samus, FighterKind::Fox, true),
            (None, Status::ThrownCommon)
        );
    }

    #[test]
    fn samus_throws_release_on_frame_nine() {
        assert_eq!(
            throw_script(FighterKind::Samus, false).flag2,
            Some((9.0, 1))
        );
        assert_eq!(throw_script(FighterKind::Samus, true).flag2, Some((9.0, 2)));
        assert_eq!(SAMUS_THROW_F[0].damage, 16);
        assert_eq!(SAMUS_THROW_B[0].damage, 18);
    }

    #[test]
    fn link_hookshot_reaches_late_and_starts_the_pull_part_way() {
        let mut link = grounded(FighterKind::Link, 0, 0.0);
        let dummy = grounded(FighterKind::Mario, 1, 0.0);
        set_catch(&mut link);
        assert_eq!(link.status.timing.anim_length, Some(85.0));
        for _ in 0..16 {
            press(&mut link, 0, 0);
            tick(&mut link);
            assert!(!search_catch(&mut link, &dummy), "no hook before frame 17");
        }
        press(&mut link, 0, 0);
        tick(&mut link);
        tick(&mut link);
        assert_eq!(link.status.anim_frame, 18.0);
        // Flag2 = 5 over flag1 = 12 frames, one step already taken.
        assert_eq!(link.grab.catch_pull_frame_begin, 5.0 - 5.0 / 12.0);
        assert!(search_catch(&mut link, &dummy));
        assert_eq!(link.status.status, Status::CatchPull);
        assert_eq!(link.status.timing.anim_length, Some(6.0));
        assert_eq!(catch_coll_frames(FighterKind::Link), 17.0..29.0);
        assert_eq!(itemheavy_joint(FighterKind::Link), Some(35));
        assert_eq!(
            thrown_status(FighterKind::Link, FighterKind::Fox, false),
            (None, Status::ThrownCommon)
        );
        let forward = throw_script(FighterKind::Link, false);
        assert_eq!(forward.flag2, Some((20.0, 1)));
        assert_eq!(forward.desc.unwrap()[0].damage, 14);
        let back = throw_script(FighterKind::Link, true);
        assert_eq!(back.flag2, Some((20.0, 2)));
        assert_eq!(back.desc.unwrap()[0].damage, 16);
        assert_eq!(
            thrown_length(FighterKind::Link, Status::ThrownDonkeyF),
            Some(32.0)
        );
    }

    #[test]
    fn yoshi_tongue_and_throws_use_their_motion_frames() {
        assert_eq!(itemheavy_joint(FighterKind::Yoshi), Some(31));
        assert_eq!(catch_colls(FighterKind::Yoshi)[0].1, 31);
        assert_eq!(catch_coll_frames(FighterKind::Yoshi), 15.0..21.0);
        assert_eq!(catch_pull_flags(FighterKind::Yoshi), Some((15.0, 6.0, 2.0)));
        let forward = throw_script(FighterKind::Yoshi, false);
        assert_eq!(forward.flag2, Some((19.0, 1)));
        assert_eq!(forward.desc.unwrap()[0].damage, 12);
        let back = throw_script(FighterKind::Yoshi, true);
        assert_eq!(back.flag2, Some((19.0, 2)));
        assert_eq!(back.desc.unwrap()[0].damage, 16);
    }

    #[test]
    fn luigi_throws_swap_marios_roll_and_keep_his_frames() {
        let forward = throw_script(FighterKind::Luigi, false);
        assert_eq!(forward.flag2, Some((14.0, 1)));
        assert_eq!(forward.desc.unwrap()[0].status, Some(Status::DamageFlyRoll));
        let back = throw_script(FighterKind::Luigi, true);
        assert_eq!(back.flag1, Some(46.0));
        assert_eq!(back.desc.unwrap()[0].status, Some(Status::DamageFlyN));
        assert_eq!(back.desc.unwrap()[0].damage, 12);
        assert_eq!(itemheavy_joint(FighterKind::Luigi), Some(28));
        assert_eq!(catch_colls(FighterKind::Luigi).len(), 1);
        assert_eq!(
            thrown_status(FighterKind::Luigi, FighterKind::Fox, true),
            (Some(Status::ThrownMarioBStart), Status::ThrownMarioB)
        );
        assert_eq!(
            thrown_length(FighterKind::Luigi, Status::ThrownMarioB),
            Some(40.0)
        );
    }

    #[test]
    fn held_topn_cancels_child_translation_through_holder_rotation() {
        let mut catcher = grounded(FighterKind::Mario, 0, 0.0);
        let joint = JointTransform {
            axes: [
                Vec3::new(0.0, 0.0, -1.0),
                Vec3::new(0.0, 1.0, 0.0),
                Vec3::new(1.0, 0.0, 0.0),
            ],
            origin: Vec3::new(100.0, 200.0, 0.0),
        };
        catcher.grab.anchor = Some(joint.origin);
        catcher.grab.anchor_transform = Some(joint);
        let mut held = grounded(FighterKind::Mario, 1, 0.0);
        held.grab.capture = Some(0);
        held.grab.holder = Some(holder_of(&catcher));
        held.grab.held_child_offset = Some(Vec3::new(0.0, 50.0, 60.0));
        held.status.status = Status::CapturePulled.into();
        refresh_held_attachment(&mut held);
        assert_eq!(held.pos, Vec3::new(40.0, 0.0, 0.0));
        held.status.status = Status::ThrownCommon.into();
        refresh_held_attachment(&mut held);
        assert_eq!(held.pos, Vec3::new(40.0, 150.0, 0.0));
    }

    /// `ftCommonCapturePulledRotateScale` multiplies the child translation
    /// by the held fighter's TopN scale: Donkey Kong's `size` is 1.25.
    #[test]
    fn held_child_offset_is_scaled_by_the_held_fighters_size() {
        let mut catcher = grounded(FighterKind::Mario, 0, 0.0);
        let joint = JointTransform {
            axes: [
                Vec3::new(0.0, 0.0, -1.0),
                Vec3::new(0.0, 1.0, 0.0),
                Vec3::new(1.0, 0.0, 0.0),
            ],
            origin: Vec3::new(100.0, 200.0, 0.0),
        };
        catcher.grab.anchor = Some(joint.origin);
        catcher.grab.anchor_transform = Some(joint);
        let mut held = grounded(FighterKind::Donkey, 1, 0.0);
        held.attributes.size = 1.25;
        held.grab.capture = Some(0);
        held.grab.holder = Some(holder_of(&catcher));
        held.grab.held_child_offset = Some(Vec3::new(0.0, 50.0, 60.0));
        held.status.status = Status::ThrownCommon.into();
        refresh_held_attachment(&mut held);
        // -(0, 50, 60) * 1.25 = (0, -62.5, -75) through the joint's axes.
        assert_eq!(held.pos, Vec3::new(25.0, 137.5, 0.0));
    }

    #[test]
    fn a_fighter_out_of_reach_is_not_grabbed() {
        let mut mario = grounded(FighterKind::Mario, 0, 0.0);
        let mut dummy = grounded(FighterKind::Mario, 1, 400.0);
        grab(&mut mario, &mut dummy);
        assert_eq!(mario.grab.catch, None);
        assert_eq!(dummy.status.status, Status::Wait);
    }

    #[test]
    fn grab_after_shield_hit_keeps_shield_catch_damage_reduction() {
        let mut mario = grounded(FighterKind::Mario, 0, 0.0);
        let mut dummy = grounded(FighterKind::Mario, 1, 150.0);
        status::set_guard_on(&mut mario);
        status::set_guard_set_off(&mut mario, 1.0, 1.0);
        status::set_guard(&mut mario);
        press(&mut mario, N64Buttons::A | N64Buttons::Z, 0);
        assert!(check_catch_guard(&mut mario));
        assert!(mario.grab.is_shield_catch);
        for _ in 0..8 {
            press(&mut mario, 0, 0);
            frame(&mut mario, &mut dummy);
            if mario.grab.catch.is_some() {
                break;
            }
        }
        to_catch_wait(&mut mario, &mut dummy);
        press(&mut mario, N64Buttons::A, 0);
        frame(&mut mario, &mut dummy);
        for _ in 0..20 {
            press(&mut mario, 0, 0);
            frame(&mut mario, &mut dummy);
            if dummy.grab.capture.is_none() {
                break;
            }
        }
        assert_eq!(dummy.damage, 6);
    }

    #[test]
    fn catch_wait_throws_forward_after_sixty_frames() {
        let mut mario = grounded(FighterKind::Mario, 0, 0.0);
        let mut dummy = grounded(FighterKind::Mario, 1, 150.0);
        grab(&mut mario, &mut dummy);
        let mut waited = 0;
        while mario.status.status != Status::ThrowF {
            press(&mut mario, 0, 0);
            frame(&mut mario, &mut dummy);
            if mario.status.status == Status::CatchWait {
                waited += 1;
            }
            assert!(waited <= CATCH_THROW_WAIT);
        }
        assert_eq!(dummy.status.status, Status::ThrownCommon);
        // Entered with `throw_wait = 60`; the 60th decrement throws.
        assert_eq!(waited, CATCH_THROW_WAIT);
    }

    #[test]
    fn mario_forward_throw_releases_on_frame_fourteen() {
        let mut mario = grounded(FighterKind::Mario, 0, 0.0);
        let mut dummy = grounded(FighterKind::Mario, 1, 150.0);
        grab(&mut mario, &mut dummy);
        to_catch_wait(&mut mario, &mut dummy);
        press(&mut mario, N64Buttons::A, 0);
        frame(&mut mario, &mut dummy);
        assert_eq!(mario.status.status, Status::ThrowF);
        for _ in 0..40 {
            press(&mut mario, 0, 0);
            frame(&mut mario, &mut dummy);
            if dummy.grab.capture.is_none() {
                break;
            }
        }
        assert_eq!(mario.status.anim_frame, 14.0);
        assert_eq!(dummy.status.status, Status::DamageFlyN);
        assert_eq!(dummy.damage, 12);
        assert!(
            dummy.physics.vel_knockback.x > 0.0,
            "thrown forward, away from Mario"
        );
        assert!(dummy.physics.vel_knockback.y > 0.0);
        assert_eq!(mario.grab.catch, None);
    }

    #[test]
    fn back_throw_turns_mario_and_launches_behind_him() {
        let mut mario = grounded(FighterKind::Mario, 0, 0.0);
        let mut dummy = grounded(FighterKind::Mario, 1, 150.0);
        grab(&mut mario, &mut dummy);
        to_catch_wait(&mut mario, &mut dummy);
        press(&mut mario, 0, -60);
        frame(&mut mario, &mut dummy);
        assert_eq!(mario.status.status, Status::ThrowB);
        assert_eq!(dummy.status.status, Status::ThrownMarioBStart);
        for _ in 0..60 {
            press(&mut mario, 0, 0);
            frame(&mut mario, &mut dummy);
            if dummy.grab.capture.is_none() {
                break;
            }
        }
        assert_eq!(dummy.status.status, Status::DamageFlyRoll);
        assert_eq!(dummy.damage, 16);
        assert!(
            dummy.physics.vel_knockback.x < 0.0,
            "launched behind Mario's start facing"
        );
    }

    #[test]
    fn donkey_forward_throw_lifts_into_cargo_and_throws() {
        let mut dk = grounded(FighterKind::Donkey, 0, 0.0);
        let mut dummy = grounded(FighterKind::Mario, 1, 150.0);
        grab(&mut dk, &mut dummy);
        assert_eq!(dk.grab.catch, Some(1));
        to_catch_wait(&mut dk, &mut dummy);
        press(&mut dk, N64Buttons::A, 0);
        frame(&mut dk, &mut dummy);
        assert_eq!(dk.status.status, Status::ThrowF);
        assert_eq!(dummy.status.status, Status::ThrownDonkeyF);
        for _ in 0..30 {
            press(&mut dk, 0, 0);
            frame(&mut dk, &mut dummy);
            if dk.status.status == AnyStatus::Donkey(DonkeyStatus::ThrowFWait) {
                break;
            }
        }
        assert_eq!(
            dk.status.status,
            AnyStatus::Donkey(DonkeyStatus::ThrowFWait)
        );
        assert_eq!(dummy.status.status, Status::Shouldered);
        assert_eq!(dummy.damage, 8, "the lift deals 8");
        assert_eq!(dummy.grab.breakout_wait, 14);

        // Walk with the cargo, then throw it.
        press(&mut dk, 0, 70);
        frame(&mut dk, &mut dummy);
        assert_eq!(
            dk.status.status,
            AnyStatus::Donkey(DonkeyStatus::ThrowFWalkFast)
        );
        press(&mut dk, N64Buttons::A, 70);
        frame(&mut dk, &mut dummy);
        assert_eq!(dk.status.status, AnyStatus::Donkey(DonkeyStatus::ThrowFF));
        for _ in 0..30 {
            press(&mut dk, 0, 0);
            frame(&mut dk, &mut dummy);
            if dummy.grab.capture.is_none() {
                break;
            }
        }
        assert_eq!(dk.status.anim_frame, 18.0, "WaitAsync(18) SetFlag2(1)");
        assert!(
            dummy.physics.vel_knockback.x > 0.0,
            "thrown the way Donkey Kong faces"
        );
        assert_eq!(dummy.status.status, Status::DamageFlyN);
        assert_eq!(dummy.damage, 16);
    }

    #[test]
    fn mashing_out_of_the_shoulder_knocks_both_back() {
        let mut dk = grounded(FighterKind::Donkey, 0, 0.0);
        let mut dummy = grounded(FighterKind::Mario, 1, 150.0);
        grab(&mut dk, &mut dummy);
        to_catch_wait(&mut dk, &mut dummy);
        press(&mut dk, N64Buttons::A, 0);
        frame(&mut dk, &mut dummy);
        for _ in 0..30 {
            press(&mut dk, 0, 0);
            frame(&mut dk, &mut dummy);
        }
        assert_eq!(dummy.status.status, Status::Shouldered);
        for i in 0..40 {
            press(&mut dk, 0, 0);
            let button = if i % 2 == 0 { N64Buttons::A } else { 0 };
            press(&mut dummy, button, 0);
            frame(&mut dk, &mut dummy);
            if dummy.grab.capture.is_none() {
                break;
            }
        }
        assert_eq!(dummy.grab.capture, None);
        assert_eq!(dk.grab.catch, None);
        assert!(dummy.hitstun > 0);
        assert!(dk.hitstun > 0);
    }

    #[test]
    fn cargo_keeps_a_light_hit_and_drops_on_a_strong_hit() {
        let mut dk = grounded(FighterKind::Donkey, 0, 0.0);
        let mut dummy = grounded(FighterKind::Mario, 1, 150.0);
        grab(&mut dk, &mut dummy);
        to_catch_wait(&mut dk, &mut dummy);
        press(&mut dk, N64Buttons::A, 0);
        frame(&mut dk, &mut dummy);
        for _ in 0..30 {
            press(&mut dk, 0, 0);
            frame(&mut dk, &mut dummy);
            if dk.status.status == AnyStatus::Donkey(DonkeyStatus::ThrowFWait) {
                break;
            }
        }
        let mut hit = Hitbox {
            damage: 2,
            radius: 100.0,
            offset: Vec3::ZERO,
            angle: 45,
            kb_scale: 5,
            kb_weight: 0,
            kb_base: 0,
        };
        assert!(attack::apply_hitbox_at(&hit, dk.pos, 9, &mut dk).registered());
        assert_eq!(
            dk.status.status,
            AnyStatus::Donkey(DonkeyStatus::ThrowFDamage)
        );
        assert_eq!(dk.grab.catch, Some(dummy.port));

        hit.damage = 20;
        hit.kb_scale = 200;
        hit.kb_base = 100;
        assert!(attack::apply_hitbox_at(&hit, dk.pos, 9, &mut dk).registered());
        exchange(&mut dk, &mut dummy);
        assert_eq!(dk.grab.catch, None);
        assert_eq!(dummy.grab.capture, None);
        assert!(dummy.hitstun > 0);
    }

    #[test]
    fn walking_off_an_edge_while_holding_drops_the_hold() {
        let mut mario = grounded(FighterKind::Mario, 0, 0.0);
        let mut dummy = grounded(FighterKind::Mario, 1, 150.0);
        grab(&mut mario, &mut dummy);
        release_on_edge(&mut mario);
        exchange(&mut mario, &mut dummy);
        assert_eq!(mario.status.status, Status::Fall);
        assert_eq!(dummy.grab.capture, None);
        assert_eq!(dummy.status.status, Status::Wait);
    }

    #[test]
    fn thrown_tables_follow_the_attribute_arrays() {
        use FighterKind::*;
        assert_eq!(
            thrown_status(Mario, Fox, true),
            (Some(Status::ThrownMarioBStart), Status::ThrownMarioB)
        );
        assert_eq!(
            thrown_status(Donkey, Mario, false),
            (Some(Status::ThrownDonkeyF), Status::Shouldered)
        );
        assert_eq!(thrown_status(Fox, Fox, false), (None, Status::ThrownFoxF));
        assert_eq!(
            thrown_status(Fox, Donkey, false),
            (Some(Status::ThrownFoxFStart), Status::ThrownCommon)
        );
        assert_eq!(
            thrown_status(Fox, Samus, false),
            (Some(Status::ThrownFoxFStart), Status::ThrownFoxF)
        );
        assert_eq!(
            thrown_status(Fox, Mario, true),
            (None, Status::ThrownCommon)
        );
        assert_eq!(thrown_status(Fox, Purin, true), (None, Status::ThrownFoxB));
    }

    /// A third party's hit on a held fighter: `ftParamGetCapturedDamage`
    /// halves it, and `ftCommonDamageCheckCaptureKeepHold` keeps the hold
    /// while the queued damage stays under 6.
    #[test]
    fn a_light_hit_on_a_held_fighter_keeps_the_hold() {
        let mut mario = grounded(FighterKind::Mario, 0, 0.0);
        let mut dummy = grounded(FighterKind::Mario, 1, 150.0);
        grab(&mut mario, &mut dummy);
        to_catch_wait(&mut mario, &mut dummy);
        let hit = Hitbox {
            damage: 9,
            offset: Vec3::ZERO,
            radius: 80.0,
            angle: 361,
            kb_scale: 100,
            kb_weight: 0,
            kb_base: 10,
        };
        let outcome = attack::apply_hitbox_at(&hit, dummy.pos, 9, &mut dummy);
        assert_eq!(outcome, attack::HitOutcome::Damaged);
        // (9 * 0.5 + 0.999) as i32 == 5, below the release threshold.
        assert_eq!(dummy.damage, 5);
        assert_eq!(dummy.status.status, Status::CaptureWait);
        assert_eq!(dummy.grab.capture, Some(0));
        exchange(&mut dummy, &mut mario);
        assert_eq!(mario.grab.catch, Some(1));
        assert_eq!(mario.status.status, Status::CatchWait);
    }

    /// At 6 or more queued damage the held fighter takes its normal damage
    /// status and the catcher loses its grip with
    /// `dFTCommonThrownNoDamageKnockback` (20 base knockback, no damage).
    #[test]
    fn a_strong_hit_on_a_held_fighter_breaks_the_hold() {
        let mut mario = grounded(FighterKind::Mario, 0, 0.0);
        let mut dummy = grounded(FighterKind::Mario, 1, 150.0);
        grab(&mut mario, &mut dummy);
        to_catch_wait(&mut mario, &mut dummy);
        let facing = mario.facing;
        let hit = Hitbox {
            damage: 12,
            offset: Vec3::ZERO,
            radius: 80.0,
            angle: 361,
            kb_scale: 100,
            kb_weight: 0,
            kb_base: 10,
        };
        let outcome = attack::apply_hitbox_at(&hit, dummy.pos, 9, &mut dummy);
        assert_eq!(outcome, attack::HitOutcome::Damaged);
        assert_eq!(dummy.damage, 6);
        assert_eq!(dummy.grab.capture, None);
        assert!(!is_held(dummy.status.status));
        exchange(&mut dummy, &mut mario);
        assert_eq!(mario.grab.catch, None);
        assert_eq!(mario.damage, 0, "the catcher's release deals no damage");
        // 20 knockback -> 10.7 hitstun frames -> level 0, grounded.
        assert_eq!(mario.status.status, Status::DamageN1);
        assert_eq!(mario.facing, facing, "lr = fp->lr keeps the facing");
        assert!(
            mario.physics.vel_knockback.x * facing.sign() < 0.0,
            "pushed backwards"
        );
    }

    /// `ftMainSearchFighterAttack` skips `capture_gobj`: Mario's back-throw
    /// swing hits a bystander but never the fighter he is holding.
    #[test]
    fn mario_back_throw_swing_hits_a_bystander_but_not_the_held_fighter() {
        let mut mario = grounded(FighterKind::Mario, 0, 0.0);
        let mut dummy = grounded(FighterKind::Mario, 1, 150.0);
        // Without a skeleton, joint 10 falls back to the root and the swing
        // sits 120 units in front of Mario; the held fighter hangs at the
        // root, well inside the same box.
        let mut bystander = grounded(FighterKind::Fox, 2, 260.0);
        grab(&mut mario, &mut dummy);
        to_catch_wait(&mut mario, &mut dummy);
        press(&mut mario, 0, -60);
        frame(&mut mario, &mut dummy);
        assert_eq!(mario.status.status, Status::ThrowB);
        let mut on_held = attack::HitRecord::default();
        let mut on_bystander = attack::HitRecord::default();
        let mut bystander_hit_at = None;
        for _ in 0..60 {
            press(&mut mario, 0, 0);
            press(&mut bystander, 0, 0);
            frame(&mut mario, &mut dummy);
            if dummy.grab.capture.is_none() {
                break;
            }
            attack::apply_hit_from(&mut mario, &mut dummy, &mut on_held);
            let before = bystander.damage;
            attack::apply_hit_from(&mut mario, &mut bystander, &mut on_bystander);
            if bystander.damage != before && bystander_hit_at.is_none() {
                bystander_hit_at = Some(mario.status.anim_frame);
            }
            assert_eq!(dummy.damage, 0, "the held fighter is never swung into");
        }
        let at = bystander_hit_at.expect("the swing reaches the bystander");
        assert!((18.0..46.0).contains(&at), "hit on frame {at}");
        assert_eq!(bystander.damage, 10);
        // The throw itself still lands on the held fighter.
        assert_eq!(dummy.damage, 16);
        // The bystander hit and the throw share Mario's `ThrowB` motion, so
        // the queue holds that one motion once.
        assert_eq!(mario.stale.next, 1);
        assert_eq!(mario.stale.entries[0].0, MotionAttackId::ThrowB);
    }

    /// Fox's back throw makes two boxes on joint 20 for frames 11..19.
    #[test]
    fn fox_back_throw_swing_hits_a_bystander() {
        let mut fox = grounded(FighterKind::Fox, 0, 0.0);
        let mut dummy = grounded(FighterKind::Mario, 1, 150.0);
        let mut bystander = grounded(FighterKind::Mario, 2, -100.0);
        grab(&mut fox, &mut dummy);
        to_catch_wait(&mut fox, &mut dummy);
        press(&mut fox, 0, -60);
        frame(&mut fox, &mut dummy);
        assert_eq!(fox.status.status, Status::ThrowB);
        let mut record = attack::HitRecord::default();
        for _ in 0..30 {
            press(&mut fox, 0, 0);
            frame(&mut fox, &mut dummy);
            attack::apply_hit_from(&mut fox, &mut bystander, &mut record);
        }
        assert_eq!(bystander.damage, 10);
    }

    /// A second back throw of the same fighter is staled by the first:
    /// `ftParamGetStaledDamage(16) == (16 * 0.75 + 0.999) as i32 == 12`.
    #[test]
    fn a_repeated_throw_is_staled() {
        let mut mario = grounded(FighterKind::Mario, 0, 0.0);
        let mut dummy = grounded(FighterKind::Mario, 1, 150.0);
        let throw_back = |mario: &mut Fighter, dummy: &mut Fighter| {
            grab(mario, dummy);
            to_catch_wait(mario, dummy);
            press(mario, 0, -60);
            frame(mario, dummy);
            for _ in 0..60 {
                press(mario, 0, 0);
                frame(mario, dummy);
                if dummy.grab.capture.is_none() {
                    break;
                }
            }
        };
        throw_back(&mut mario, &mut dummy);
        assert_eq!(dummy.damage, 16);
        let (stale, motion) = (mario.stale, mario.motion);
        mario = grounded(FighterKind::Mario, 0, 0.0);
        mario.stale = stale;
        mario.motion = motion;
        dummy = grounded(FighterKind::Mario, 1, 150.0);
        throw_back(&mut mario, &mut dummy);
        assert_eq!(dummy.damage, 12);
    }
}
