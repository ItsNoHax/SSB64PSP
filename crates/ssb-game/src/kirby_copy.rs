//! Kirby's copy abilities for the ported fighters: `ftkirbycopy*specialn.c`
//! for Mario and Luigi (Fireball), Fox (Blaster), Samus (Charge Shot),
//! Donkey Kong (Giant Punch), Link (Boomerang), Captain Falcon (Falcon
//! Punch), Yoshi (Egg Lay), Pikachu (Thunder Jolt) and Jigglypuff (Pound),
//! with the motion-script events from `relocData/228_KirbyMainMotion.c` (US).
//!
//! `ftKirbySpecialNSetStatusSelect` and `ftKirbySpecialAirNSetStatusSelect`
//! pick the status from `passive_vars.kirby.copy_id` ([`set_special_n`]).
//! Each copy mirrors its fighter's own port and reuses its weapons, catch
//! link and swallowed-fighter statuses; the Kirby versions differ in their
//! figatrees, spawn joints and a few constants, all listed below.
//!
//! Callbacks run in the source order: `proc_update` then `proc_interrupt`
//! ([`update`]), then `proc_physics` ([`apply_ground_physics`] /
//! [`apply_air_physics`]), then `proc_map` ([`on_ground_lost`] /
//! [`on_landing`]). `crate::kirby` routes the copy statuses here.
//!
//! ## Documented deviations
//!
//! * **Ness.** His copy waits on his weapon; a Kirby holding it still
//!   inhales.
//! * **Giant Punch intangibility.** The full punch's `SetHitStatusAll(3)`
//!   window waits on hit-status intangibility, which is not ported.
//! * **Escape.** Charge Shot's and Giant Punch's shield-roll interrupts wait
//!   on the unported `ftCommonEscape` statuses, as in the fighters' own ports.
//! * **Ledges.** Falcon Punch's and Egg Lay's aerial statuses catch ledges
//!   in the source; fighter map collision resolves floors only.

use ssb_engine::input::N64Buttons;
use ssb_engine::math::{sin_cos, Vec3};

use crate::attack::Hitbox;
use crate::fighter::{Fighter, FighterKind};
use crate::grab::GrabEvent;
use crate::physics;
use crate::stale::MotionAttackId;
use crate::status::{self, AnyStatus, KirbyStatus as K, StatusTiming};
use crate::weapon::{WeaponKind, WeaponSpawn};

/// `FTKIRBY_COPYMARIO_FIREBALL_SPAWN_JOINT`.
pub const FIREBALL_SPAWN_JOINT: u8 = 17;
/// `FTKIRBY_COPYFOX_BLASTER_SPAWN_JOINT` and `..._OFF_X`.
pub const BLASTER_SPAWN_JOINT: u8 = 17;
pub const BLASTER_SPAWN_OFF_X: f32 = 70.0;
/// `FTKIRBY_COPYSAMUS_CHARGE_*`.
pub const CHARGE_JOINT: u8 = 0;
pub const CHARGE_OFF_Y: f32 = 200.0;
pub const CHARGE_OFF_Z: f32 = 210.0;
pub const CHARGE_MAX: u8 = 7;
pub const CHARGE_INT: u8 = 20;
pub const CHARGE_RECOIL_BASE: f32 = 10.0;
pub const CHARGE_RECOIL_MUL: f32 = 2.0;
pub const CHARGE_RECOIL_ADD: f32 = 20.0;
/// `FTKIRBY_COPYDONKEY_GIANTPUNCH_*`.
pub const GIANTPUNCH_CHARGE_MAX: u8 = 10;
pub const GIANTPUNCH_CHARGE_DAMAGE_MUL: i32 = 2;
pub const GIANTPUNCH_CHARGE_ANIM_SPEED: f32 = 2.0;
pub const GIANTPUNCH_VEL_MUL: f32 = 8.0;
/// `FTKIRBY_COPYLINK_BOOMERANG_*`.
pub const BOOMERANG_SPAWN_JOINT: u8 = 0;
pub const BOOMERANG_SMASH_STICK_MIN: i32 = 56;
pub const BOOMERANG_SMASH_BUFFER: u8 = 8;
/// `FTKIRBY_COPYCAPTAIN_FALCONPUNCH_*`.
pub const FALCONPUNCH_VEL_BASE: f32 = 65.0;
pub const FALCONPUNCH_VEL_MUL: f32 = 0.92;
/// `FTKIRBY_COPYPIKACHU_THUNDERJOLT_SPAWN_*`. The shared weapon launches
/// at -45 degrees with speed 40, as for Pikachu's own Thunder Jolt.
pub const THUNDERJOLT_SPAWN_JOINT: u8 = 0;
pub const THUNDERJOLT_SPAWN_OFF_X: f32 = 200.0;
pub const THUNDERJOLT_SPAWN_OFF_Y: f32 = 200.0;

/// Figatree lengths (`ssb_rom::anim::EXPECTED_FRAMES`).
const FIREBALL_LENGTH: f32 = 46.0;
const BLASTER_LENGTH: f32 = 55.0;
const BLASTER_AIR_LENGTH: f32 = 45.0;
const CHARGE_START_LENGTH: f32 = 16.0;
const CHARGE_END_LENGTH: f32 = 30.0;
const CHARGE_AIR_END_LENGTH: f32 = 29.0;
const GIANTPUNCH_START_LENGTH: f32 = 8.0;
/// `ChargePunchGround` and `ChargePunchAir` loop every 12 frames.
const GIANTPUNCH_LOOP_LENGTH: f32 = 12.0;
const GIANTPUNCH_END_LENGTH: f32 = 80.0;
const BOOMERANG_LENGTH: f32 = 46.0;
const BOOMERANG_GET_LENGTH: f32 = 20.0;
const FALCONPUNCH_LENGTH: f32 = 90.0;
const EGG_LAY_LENGTH: f32 = 38.0;
const EGG_LAY_RELEASE_LENGTH: f32 = 35.0;
const THUNDERJOLT_LENGTH: f32 = 64.0;
/// Jigglypuff's `PoundGround` and `PoundAir` figatrees.
const POUND_LENGTH: f32 = 55.0;

/// Motion-script frames. `LuigiFireballGround` and its three siblings set
/// flag 0 at `WaitAsync(16)`.
const FIREBALL_FRAME: f32 = 16.0;
/// `LaserGround_0x1E18`: flag 0 at 25, flag 1 four frames later.
/// `LaserAir` sets both at 15.
const BLASTER_FRAME: f32 = 25.0;
const BLASTER_REPEAT_FRAME: f32 = 29.0;
const BLASTER_AIR_FRAME: f32 = 15.0;
/// `Boomerang_0x2124`: `WaitAsync(26)` then `SetFlag0(1)`.
const BOOMERANG_FRAME: f32 = 26.0;
/// `FalconPunchAir`: flags 1 and 2 at 40; flag 2 becomes 2 at 55.
const FALCONPUNCH_BOOST_FRAME: f32 = 40.0;
const FALCONPUNCH_DRIFT_FRAME: f32 = 55.0;
/// `EggLayGround_0x21D8`: the catch box from `WaitAsync(18)` until the
/// clear after `Wait(6)`.
pub const EGG_LAY_CATCH_FRAMES: core::ops::Range<f32> = 18.0..24.0;
/// `EggLayGround_0x2218`: `WaitAsync(25)` then `SetFlag1(1)`.
const EGG_LAY_CATCH_FLAG1_FRAME: f32 = 25.0;
/// `EggThrowGround`: flag 2 at frame 6 hides the swallowed fighter, flag 1
/// at frame 20 lays it.
const EGG_LAY_SWALLOW_FRAME: f32 = 6.0;
const EGG_LAY_LAY_FRAME: f32 = 20.0;
/// `ThunderJoltGround` and `ThunderJoltAir`: `WaitAsync(21)`, flag 0.
const THUNDERJOLT_FRAME: f32 = 21.0;

/// The Egg Lay catch box: joint 30 (Kirby's heavy-item joint), size 300,
/// 100 up. Yoshi's uses joint 31 with the same box.
pub const EGG_LAY_CATCH: (Hitbox, u8) = (crate::yoshi::EGG_LAY_CATCH.0, 30);

/// `passive_vars.kirby`'s copy fields and the copy statuses' status vars
/// and motion flags. The Boomerang's `copylink_boomerang_gobj` is
/// `crate::link::LinkState::boomerang_out`, which the weapon pool keeps for
/// every thrower.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct CopyState {
    /// Motion flag 0 was consumed: the Fireball, Blaster shot, Charge Shot,
    /// Boomerang or Thunder Jolt was made.
    pub spawned: bool,
    /// `passive_vars.kirby.copysamus_charge_level`.
    pub samus_charge_level: u8,
    /// `passive_vars.kirby.copysamus_charge_recoil`. Unlike Samus's, landing
    /// does not reset it; only a new copy does.
    pub samus_charge_recoil: i32,
    /// `status_vars.kirby.copysamus_specialn.is_release`.
    pub samus_is_release: bool,
    /// `status_vars.kirby.copysamus_specialn.charge_int`.
    pub samus_charge_int: u8,
    /// `charge_gobj != NULL`: a charging shot on Kirby.
    pub samus_charge_shot: bool,
    /// `passive_vars.kirby.copydonkey_charge_level`.
    pub donkey_charge_level: u8,
    /// `status_vars.kirby.copydonkey_specialn.charge_level`: the level the
    /// punch was released at.
    pub donkey_attack_charge: u8,
    pub donkey_is_release: bool,
    pub donkey_is_charging: bool,
    pub donkey_is_cancel: bool,
    /// `status_vars.kirby.copylink_specialn.is_smash`.
    pub link_is_smash: bool,
    /// Falcon Punch's flag 1 was consumed.
    pub captain_boosted: bool,
    /// `passive_vars.kirby.copycaptain_falcon_punch_unk`: aerial punches
    /// thrown. Nothing reads it.
    pub captain_punch_count: i32,
    /// Pound's flag 1 was consumed.
    pub purin_boosted: bool,
    /// `passive_vars.kirby.copypurin_unk`: aerial Pounds. Nothing reads it.
    pub purin_count: i32,
}

fn set(f: &mut Fighter, s: K, frame: f32, timing: StatusTiming) {
    // The aerial Fireball, Blaster and Giant Punch setters pass
    // `FTSTATUS_PRESERVE_FASTFALL`; every other status change clears it.
    let fastfall = matches!(
        s,
        K::CopyMarioSpecialAirN
            | K::CopyLuigiSpecialAirN
            | K::CopyFoxSpecialAirN
            | K::CopyDonkeySpecialAirNStart
            | K::CopyDonkeySpecialAirNLoop
            | K::CopyDonkeySpecialAirNEnd
            | K::CopyDonkeySpecialAirNFull
    );
    let preserve = status::Preserve {
        fastfall,
        ..status::Preserve::NONE
    };
    status::set_any_status_preserve(f, AnyStatus::Kirby(s), frame, timing, preserve);
}

fn taps(f: &Fighter) -> N64Buttons {
    f.button_tap()
}

/// The copy statuses this module owns.
pub fn is_copy(s: K) -> bool {
    (K::CopyMarioSpecialN..=K::CopyDonkeySpecialAirNFull).contains(&s)
        || (K::CopyLinkSpecialN..=K::CopyYoshiSpecialAirNRelease).contains(&s)
        || matches!(
            s,
            K::CopyPikachuSpecialN
                | K::CopyPikachuSpecialAirN
                | K::CopyNessSpecialN
                | K::CopyNessSpecialAirN
        )
}

/// Which copy statuses leave Kirby grounded.
pub fn is_grounded(s: K) -> bool {
    matches!(
        s,
        K::CopyMarioSpecialN
            | K::CopyLuigiSpecialN
            | K::CopyFoxSpecialN
            | K::CopySamusSpecialNStart
            | K::CopySamusSpecialNLoop
            | K::CopySamusSpecialNEnd
            | K::CopyDonkeySpecialNStart
            | K::CopyDonkeySpecialNLoop
            | K::CopyDonkeySpecialNEnd
            | K::CopyDonkeySpecialNFull
            | K::CopyLinkSpecialN
            | K::CopyLinkSpecialNGet
            | K::CopyLinkSpecialNEmpty
            | K::CopyCaptainSpecialN
            | K::CopyYoshiSpecialN
            | K::CopyYoshiSpecialNCatch
            | K::CopyYoshiSpecialNRelease
            | K::CopyPikachuSpecialN
            | K::CopyPurinSpecialN
            | K::CopyNessSpecialN
    )
}

/// `ssb_rom::anim::SLOT_KIRBY_COPY_MARIO_SPECIAL_N` onward, one slot per
/// figatree.
pub fn anim_slot(s: K) -> Option<usize> {
    const C: usize = 317;
    Some(match s {
        K::CopyMarioSpecialN | K::CopyLuigiSpecialN => C,
        K::CopyMarioSpecialAirN | K::CopyLuigiSpecialAirN => C + 1,
        K::CopyFoxSpecialN => C + 2,
        K::CopyFoxSpecialAirN => C + 3,
        K::CopySamusSpecialNStart => C + 4,
        K::CopySamusSpecialNLoop => C + 5,
        K::CopySamusSpecialNEnd => C + 6,
        K::CopySamusSpecialAirNStart => C + 7,
        K::CopySamusSpecialAirNEnd => C + 8,
        K::CopyDonkeySpecialNStart => C + 9,
        K::CopyDonkeySpecialAirNStart => C + 10,
        K::CopyDonkeySpecialNLoop => C + 11,
        K::CopyDonkeySpecialAirNLoop => C + 12,
        K::CopyDonkeySpecialNEnd | K::CopyDonkeySpecialNFull => C + 13,
        K::CopyDonkeySpecialAirNEnd | K::CopyDonkeySpecialAirNFull => C + 14,
        K::CopyLinkSpecialN | K::CopyLinkSpecialNEmpty => C + 15,
        K::CopyLinkSpecialNGet => C + 16,
        K::CopyLinkSpecialAirN | K::CopyLinkSpecialAirNEmpty => C + 17,
        K::CopyLinkSpecialAirNReturn => C + 18,
        K::CopyCaptainSpecialN => C + 19,
        K::CopyCaptainSpecialAirN => C + 20,
        K::CopyYoshiSpecialN | K::CopyYoshiSpecialNCatch => C + 21,
        K::CopyYoshiSpecialNRelease => C + 22,
        K::CopyYoshiSpecialAirN | K::CopyYoshiSpecialAirNCatch => C + 23,
        K::CopyYoshiSpecialAirNRelease => C + 24,
        K::CopyPikachuSpecialN => 373,
        K::CopyPikachuSpecialAirN => 374,
        K::CopyNessSpecialN => 444,
        K::CopyNessSpecialAirN => 445,
        K::CopyPurinSpecialN => 402,
        K::CopyPurinSpecialAirN => 403,
        _ => return None,
    })
}

/// `nFTMotionAttackIDSpecialNCopy*` of each copy status.
pub fn attack_id(s: K) -> Option<MotionAttackId> {
    use MotionAttackId as M;
    Some(match s {
        K::CopyMarioSpecialN | K::CopyMarioSpecialAirN => M::SpecialNCopyMario,
        K::CopyLuigiSpecialN | K::CopyLuigiSpecialAirN => M::SpecialNCopyLuigi,
        K::CopyFoxSpecialN | K::CopyFoxSpecialAirN => M::SpecialNCopyFox,
        K::CopySamusSpecialNStart
        | K::CopySamusSpecialNLoop
        | K::CopySamusSpecialNEnd
        | K::CopySamusSpecialAirNStart
        | K::CopySamusSpecialAirNEnd => M::SpecialNCopySamus,
        K::CopyDonkeySpecialNStart
        | K::CopyDonkeySpecialAirNStart
        | K::CopyDonkeySpecialNLoop
        | K::CopyDonkeySpecialAirNLoop
        | K::CopyDonkeySpecialNEnd
        | K::CopyDonkeySpecialAirNEnd
        | K::CopyDonkeySpecialNFull
        | K::CopyDonkeySpecialAirNFull => M::SpecialNCopyDonkey,
        K::CopyLinkSpecialN
        | K::CopyLinkSpecialNGet
        | K::CopyLinkSpecialNEmpty
        | K::CopyLinkSpecialAirN
        | K::CopyLinkSpecialAirNReturn
        | K::CopyLinkSpecialAirNEmpty => M::SpecialNCopyLink,
        K::CopyCaptainSpecialN | K::CopyCaptainSpecialAirN => M::SpecialNCopyCaptain,
        K::CopyYoshiSpecialN
        | K::CopyYoshiSpecialNCatch
        | K::CopyYoshiSpecialNRelease
        | K::CopyYoshiSpecialAirN
        | K::CopyYoshiSpecialAirNCatch
        | K::CopyYoshiSpecialAirNRelease => M::SpecialNCopyYoshi,
        K::CopyPikachuSpecialN | K::CopyPikachuSpecialAirN => M::SpecialNCopyPikachu,
        K::CopyPurinSpecialN | K::CopyPurinSpecialAirN => M::SpecialNCopyPurin,
        K::CopyNessSpecialN | K::CopyNessSpecialAirN => M::SpecialNCopyNess,
        _ => return None,
    })
}

/// `FTKIRBY_COPY_MODELPARTS_JOINT`: the joint whose model part the copy
/// swaps.
pub const COPY_MODELPARTS_JOINT: u8 = 6;

/// `dKirbyMainMotion_0x0000[kind].copy_modelpart_id`
/// (`llKirbyMainMotionSpecialNFTKirbyCopy`, `228_KirbyMainMotion.c`) for the
/// twelve playable kinds: joint 6's part while Kirby holds that copy
/// (RE-417). Kirby's own row is 0, his bare head.
pub const COPY_MODELPART_IDS: [u8; 12] = [12, 7, 4, 8, 11, 10, 5, 9, 0, 6, 3, 13];

/// `copy[copy_id].copy_modelpart_id` (0 outside the twelve).
pub fn copy_modelpart_id(copy_id: FighterKind) -> i8 {
    COPY_MODELPART_IDS
        .get(copy_id as usize)
        .map_or(0, |&part| part as i8)
}

/// The copy hat Kirby wears: `ftKirbySpecialNCopyInitCopyVars`'s
/// `ftParamSetModelPartDefaultID(joint 6, copy[copy_id].copy_modelpart_id)`,
/// which `ftKirbySpecialNLoseCopy` resets to 0. `None` for part 0 or another
/// fighter.
pub fn copy_hat(f: &Fighter) -> Option<u8> {
    if f.kind != FighterKind::Kirby {
        return None;
    }
    COPY_MODELPART_IDS
        .get(f.kirby.copy_id as usize)
        .copied()
        .filter(|&part| part != 0)
}

/// `ftKirbySpecialNInitPassiveVars`: a new or lost copy starts clean.
pub fn init_passive_vars(f: &mut Fighter) {
    match f.kirby.copy_id {
        FighterKind::Samus => {
            f.kirby.copy.samus_charge_level = 0;
            f.kirby.copy.samus_charge_recoil = 0;
        }
        FighterKind::Donkey => f.kirby.copy.donkey_charge_level = 0,
        FighterKind::Captain => f.kirby.copy.captain_punch_count = 0,
        FighterKind::Purin => f.kirby.copy.purin_count = 0,
        _ => {}
    }
}

/// `dFTKirbySpecialNStatusList` / `dFTKirbySpecialAirNStatusList`: the copy
/// special for `copy_id`. Returns `false` when Kirby has no ported copy, so
/// the caller inhales instead.
pub fn set_special_n(f: &mut Fighter) -> bool {
    let ground = f.is_grounded();
    match f.kirby.copy_id {
        FighterKind::Mario | FighterKind::Luigi => set_fireball(f, ground),
        FighterKind::Fox => set_blaster(f, ground),
        FighterKind::Samus => set_charge_start(f, ground),
        FighterKind::Donkey => set_giant_punch_start(f, ground),
        FighterKind::Link => set_boomerang(f, ground),
        FighterKind::Captain => {
            let s = if ground {
                K::CopyCaptainSpecialN
            } else {
                K::CopyCaptainSpecialAirN
            };
            set(f, s, 0.0, StatusTiming::frames(FALCONPUNCH_LENGTH));
            status::play_anim_events(f);
            f.kirby.copy.captain_boosted = false;
        }
        FighterKind::Yoshi => {
            let s = if ground {
                K::CopyYoshiSpecialN
            } else {
                K::CopyYoshiSpecialAirN
            };
            set(f, s, 0.0, StatusTiming::frames(EGG_LAY_LENGTH));
            set_egg_lay_catch_params(f);
            status::play_anim_events(f);
        }
        FighterKind::Pikachu => {
            let s = if ground {
                K::CopyPikachuSpecialN
            } else {
                K::CopyPikachuSpecialAirN
            };
            set(f, s, 0.0, StatusTiming::frames(THUNDERJOLT_LENGTH));
            status::play_anim_events(f);
            f.kirby.copy.spawned = false;
        }
        FighterKind::Ness => {
            set(
                f,
                if ground {
                    K::CopyNessSpecialN
                } else {
                    K::CopyNessSpecialAirN
                },
                0.0,
                StatusTiming::frames(if ground { 72.0 } else { 60.0 }),
            );
            status::play_anim_events(f);
            f.kirby.copy.spawned = false;
        }
        FighterKind::Purin => {
            let s = if ground {
                K::CopyPurinSpecialN
            } else {
                K::CopyPurinSpecialAirN
            };
            set(f, s, 0.0, StatusTiming::frames(POUND_LENGTH));
            f.kirby.copy.purin_boosted = false;
            status::play_anim_events(f);
        }
        _ => return false,
    }
    true
}

// ---------------------------------------------------------------------------
// Fireball (`ftkirbycopymariospecialn.c`)
// ---------------------------------------------------------------------------

/// `FTKIRBY_COPYMARIO_FIREBALL_CHECK_FTKIND`: Mario's copy or Luigi's.
fn fireball_status(f: &Fighter, ground: bool) -> K {
    match (f.kirby.copy_id == FighterKind::Luigi, ground) {
        (false, true) => K::CopyMarioSpecialN,
        (false, false) => K::CopyMarioSpecialAirN,
        (true, true) => K::CopyLuigiSpecialN,
        (true, false) => K::CopyLuigiSpecialAirN,
    }
}

fn set_fireball(f: &mut Fighter, ground: bool) {
    let s = fireball_status(f, ground);
    set(f, s, 0.0, StatusTiming::frames(FIREBALL_LENGTH));
    status::play_anim_events(f);
    f.kirby.copy.spawned = false;
}

/// `ftKirbyCopyMarioSpecialNProcAccessory`: the `copy_id` switch picks the
/// Fireball's attribute row.
fn make_fireball(f: &mut Fighter) {
    if f.kirby.copy.spawned || f.status.clock < FIREBALL_FRAME {
        return;
    }
    f.kirby.copy.spawned = true;
    let kind = if f.kirby.copy_id == FighterKind::Luigi {
        WeaponKind::LuigiFireball
    } else {
        WeaponKind::MarioFireball
    };
    f.weapon_spawn = Some(WeaponSpawn {
        kind,
        owner_port: f.port,
        team: f.team,
        stale: crate::stale::WeaponStale::of(f),
        position: f.joint_world(FIREBALL_SPAWN_JOINT, Vec3::ZERO),
        facing: f.facing.sign(),
    });
}

// ---------------------------------------------------------------------------
// Blaster (`ftkirbycopyfoxspecialn.c`)
// ---------------------------------------------------------------------------

fn set_blaster(f: &mut Fighter, ground: bool) {
    if ground {
        set(
            f,
            K::CopyFoxSpecialN,
            0.0,
            StatusTiming::frames(BLASTER_LENGTH),
        );
    } else {
        set(
            f,
            K::CopyFoxSpecialAirN,
            0.0,
            StatusTiming::frames(BLASTER_AIR_LENGTH),
        );
    }
    status::play_anim_events(f);
    f.kirby.copy.spawned = false;
}

fn blaster_frames(s: K) -> (f32, f32) {
    if s == K::CopyFoxSpecialN {
        (BLASTER_FRAME, BLASTER_REPEAT_FRAME)
    } else {
        (BLASTER_AIR_FRAME, BLASTER_AIR_FRAME)
    }
}

/// `ftKirbyCopyFoxSpecialNProcUpdate`: the shot leaves joint 17, 70 along
/// its X.
fn update_blaster(f: &mut Fighter, s: K) {
    if !f.kirby.copy.spawned && f.status.clock >= blaster_frames(s).0 {
        f.kirby.copy.spawned = true;
        f.weapon_spawn = Some(WeaponSpawn {
            kind: WeaponKind::FoxBlaster,
            owner_port: f.port,
            team: f.team,
            stale: crate::stale::WeaponStale::of(f),
            position: f.joint_world(
                BLASTER_SPAWN_JOINT,
                Vec3::new(BLASTER_SPAWN_OFF_X, 0.0, 0.0),
            ),
            facing: f.facing.sign(),
        });
    }
    if f.status.animation_ended() {
        status::anim_end_set_wait_or_fall(f);
    }
}

/// `ftKirbyCopyFoxSpecialNProcInterrupt`: B after flag 1 fires again, as a
/// new motion.
fn interrupt_blaster(f: &mut Fighter, s: K) {
    if f.status.clock >= blaster_frames(s).1 && taps(f).contains(N64Buttons::B) {
        set_blaster(f, f.is_grounded());
        f.motion.set(MotionAttackId::SpecialNCopyFox);
        f.stats.restart();
    }
}

// ---------------------------------------------------------------------------
// Charge Shot (`ftkirbycopysamusspecialn.c`)
// ---------------------------------------------------------------------------

/// `ftKirbyCopySamusSpecialNStartGetAnimSpeed`.
fn charge_start_speed(level: u8) -> f32 {
    let ret = f32::from(level) / f32::from(CHARGE_MAX);
    -0.160_000_03 * ret + 1.0
}

/// `ftKirbyCopySamusSpecialNStartSetStatus` / `...AirNStartSetStatus` and
/// `ftKirbyCopySamusSpecialNInitStatusVars`.
fn set_charge_start(f: &mut Fighter, ground: bool) {
    let speed = charge_start_speed(f.kirby.copy.samus_charge_level);
    let s = if ground {
        K::CopySamusSpecialNStart
    } else {
        K::CopySamusSpecialAirNStart
    };
    set(
        f,
        s,
        0.0,
        StatusTiming::at_speed(CHARGE_START_LENGTH, speed),
    );
    status::play_anim_events(f);
    f.kirby.copy.samus_charge_shot = false;
    f.kirby.copy.spawned = false;
    f.kirby.copy.samus_is_release = !ground || f.kirby.copy.samus_charge_level == CHARGE_MAX;
}

/// `ftKirbyCopySamusSpecialNLoopSetStatus`.
fn set_charge_loop(f: &mut Fighter) {
    set(f, K::CopySamusSpecialNLoop, 0.0, StatusTiming::unknown());
    f.kirby.copy.samus_charge_int = CHARGE_INT;
    f.kirby.copy.samus_charge_shot = true;
}

/// `ftKirbyCopySamusSpecialNEndSetStatus`.
fn set_charge_end(f: &mut Fighter) {
    set(
        f,
        K::CopySamusSpecialNEnd,
        0.0,
        StatusTiming::frames(CHARGE_END_LENGTH),
    );
    f.kirby.copy.spawned = false;
}

/// `ftKirbyCopySamusSpecialAirNEndSetStatus`.
fn set_charge_air_end(f: &mut Fighter) {
    if f.is_grounded() {
        f.become_airborne();
        physics::clamp_air_vel_x(&mut f.physics, f.attributes.air_speed_max_x);
    }
    set(
        f,
        K::CopySamusSpecialAirNEnd,
        0.0,
        StatusTiming::frames(CHARGE_AIR_END_LENGTH),
    );
    f.kirby.copy.spawned = false;
}

/// `ftKirbyCopySamusSpecialNGetChargeShotPosition`: joint 0 plus
/// (0, 200, 210).
pub fn charge_shot_position(f: &Fighter) -> Vec3 {
    f.joint_world(CHARGE_JOINT, Vec3::new(0.0, CHARGE_OFF_Y, CHARGE_OFF_Z))
}

/// The flag-0 half of `ftKirbyCopySamusSpecialNEndProcUpdate`.
fn fire_charge_shot(f: &mut Fighter) {
    let level = f.kirby.copy.samus_charge_level;
    f.weapon_spawn = Some(WeaponSpawn {
        kind: WeaponKind::SamusChargeShot(level),
        owner_port: f.port,
        team: f.team,
        stale: crate::stale::WeaponStale::of(f),
        position: charge_shot_position(f),
        facing: f.facing.sign(),
    });
    f.kirby.copy.samus_charge_shot = false;
    let recoil_x = f32::from(level) + 1.0;
    if f.is_grounded() {
        // Source ground velocity is facing-relative; the port's is world X.
        f.physics.vel_ground.x =
            -(CHARGE_RECOIL_MUL * recoil_x + CHARGE_RECOIL_BASE) * f.facing.sign();
    } else {
        f.physics.vel_air.x =
            (CHARGE_RECOIL_MUL * recoil_x + CHARGE_RECOIL_BASE) * -f.facing.sign();
        let recoil_y = recoil_x
            + CHARGE_RECOIL_ADD
            + (f.kirby.copy.samus_charge_recoil as f32 * -CHARGE_RECOIL_BASE);
        if f.physics.vel_air.y < recoil_y {
            f.physics.vel_air.y = recoil_y;
        }
        f.kirby.copy.samus_charge_recoil += 1;
    }
    f.kirby.copy.samus_charge_level = 0;
}

/// Whether a charging shot is on Kirby, for presentation.
pub fn is_charging(f: &Fighter) -> bool {
    f.kirby.copy.samus_charge_shot && f.status.status == AnyStatus::Kirby(K::CopySamusSpecialNLoop)
}

// ---------------------------------------------------------------------------
// Giant Punch (`ftkirbycopydonkeyspecialn.c`)
// ---------------------------------------------------------------------------

/// `ftKirbyCopyDonkeySpecialNStartSetStatus` / `...AirNStartSetStatus` and
/// `ftKirbyCopyDonkeySpecialNInitStatusVars`.
fn set_giant_punch_start(f: &mut Fighter, ground: bool) {
    let s = if ground {
        K::CopyDonkeySpecialNStart
    } else {
        K::CopyDonkeySpecialAirNStart
    };
    set(f, s, 0.0, StatusTiming::frames(GIANTPUNCH_START_LENGTH));
    status::play_anim_events(f);
    f.kirby.copy.donkey_is_release = f.kirby.copy.donkey_charge_level == GIANTPUNCH_CHARGE_MAX;
    f.kirby.copy.donkey_is_charging = false;
    f.kirby.copy.donkey_is_cancel = false;
}

/// `ftKirbyCopyDonkeySpecialNLoopSetProcDamageAnimSpeed`: a full charge
/// swings at double speed.
fn giant_punch_loop_timing(f: &Fighter) -> StatusTiming {
    let speed = if f.kirby.copy.donkey_charge_level == GIANTPUNCH_CHARGE_MAX {
        GIANTPUNCH_CHARGE_ANIM_SPEED
    } else {
        1.0
    };
    StatusTiming::at_speed(GIANTPUNCH_LOOP_LENGTH, speed)
}

fn set_giant_punch_loop(f: &mut Fighter, s: K) {
    let timing = giant_punch_loop_timing(f);
    set(f, s, 0.0, timing);
}

/// `ftKirbyCopyDonkeySpecialNEndSetStatus` / `...AirNEndSetStatus` and
/// `ftKirbyCopyDonkeySpecialNGetStatusChargeLevelReset`.
fn set_giant_punch_end(f: &mut Fighter) {
    let ground = f.is_grounded();
    let level = f.kirby.copy.donkey_charge_level;
    let s = match (ground, level == GIANTPUNCH_CHARGE_MAX) {
        (true, false) => K::CopyDonkeySpecialNEnd,
        (true, true) => K::CopyDonkeySpecialNFull,
        (false, false) => K::CopyDonkeySpecialAirNEnd,
        (false, true) => K::CopyDonkeySpecialAirNFull,
    };
    set(f, s, 0.0, StatusTiming::frames(GIANTPUNCH_END_LENGTH));
    if ground {
        // Facing-relative in the source.
        f.physics.vel_ground.x = f32::from(level) * GIANTPUNCH_VEL_MUL * f.facing.sign();
    }
    f.kirby.copy.donkey_attack_charge = level;
    f.kirby.copy.donkey_charge_level = 0;
}

/// `ftKirbyCopyDonkeySpecialNLoopProcUpdate`, run when the loop wraps.
fn giant_punch_loop_wrap(f: &mut Fighter, s: K) {
    let copy = &mut f.kirby.copy;
    if copy.donkey_is_charging && copy.donkey_charge_level < GIANTPUNCH_CHARGE_MAX {
        copy.donkey_charge_level += 1;
        if copy.donkey_charge_level == GIANTPUNCH_CHARGE_MAX {
            copy.donkey_is_cancel = true;
            crate::colanim::check_set(
                f,
                crate::colanim::ColAnimId::FIGHTER_COMMON_SPECIAL_N_CHARGE,
                0,
            );
        }
    }
    if f.kirby.copy.donkey_is_cancel {
        status::set_wait_or_fall(f);
    } else if f.kirby.copy.donkey_is_release {
        set_giant_punch_end(f);
    } else {
        f.kirby.copy.donkey_is_charging = true;
        set_giant_punch_loop(f, s);
    }
}

/// Damage added to the uncharged punch's boxes at their creation.
pub fn giant_punch_bonus(f: &Fighter) -> i32 {
    if matches!(
        f.status.status,
        AnyStatus::Kirby(K::CopyDonkeySpecialNEnd | K::CopyDonkeySpecialAirNEnd)
    ) {
        i32::from(f.kirby.copy.donkey_attack_charge) * GIANTPUNCH_CHARGE_DAMAGE_MUL
    } else {
        0
    }
}

// ---------------------------------------------------------------------------
// Boomerang (`ftkirbycopylinkspecialn.c`)
// ---------------------------------------------------------------------------

/// `ftKirbyCopyLinkSpecialNProcStatus`.
fn boomerang_proc_status(f: &mut Fighter) {
    f.kirby.copy.spawned = false;
    f.kirby.copy.link_is_smash = i32::from(f.input.stick_x).abs() >= BOOMERANG_SMASH_STICK_MIN
        && f.stick.hold_x < BOOMERANG_SMASH_BUFFER;
}

/// `ftKirbyCopyLinkSpecialNSetStatus` / `...AirNSetStatus`: with a
/// Boomerang out, Kirby only mimes the throw and may catch it.
fn set_boomerang(f: &mut Fighter, ground: bool) {
    boomerang_proc_status(f);
    let empty = f.link.boomerang_out;
    let s = match (ground, empty) {
        (true, false) => K::CopyLinkSpecialN,
        (true, true) => K::CopyLinkSpecialNEmpty,
        (false, false) => K::CopyLinkSpecialAirN,
        (false, true) => K::CopyLinkSpecialAirNEmpty,
    };
    set(f, s, 0.0, StatusTiming::frames(BOOMERANG_LENGTH));
    if empty {
        f.is_special_interrupt = true;
    }
    status::play_anim_events(f);
}

/// `ftKirbyCopyLinkSpecialNGetSetStatus`, called by the weapon pool when a
/// returning Boomerang reaches a Kirby whose `is_special_interrupt` is set.
pub fn set_boomerang_get(f: &mut Fighter) {
    let s = if f.is_grounded() {
        K::CopyLinkSpecialNGet
    } else {
        K::CopyLinkSpecialAirNReturn
    };
    set(f, s, 0.0, StatusTiming::frames(BOOMERANG_GET_LENGTH));
    status::play_anim_events(f);
}

/// `ftKirbyCopyLinkSpecialNMakeBoomerang`: joint 0, with the stick read at
/// flag 0.
fn make_boomerang(f: &mut Fighter) {
    if f.kirby.copy.spawned || f.status.clock < BOOMERANG_FRAME {
        return;
    }
    f.kirby.copy.spawned = true;
    f.weapon_spawn = Some(WeaponSpawn {
        kind: WeaponKind::LinkBoomerang {
            is_smash: f.kirby.copy.link_is_smash,
            stick_x: f.input.stick_x,
            stick_y: f.input.stick_y,
        },
        owner_port: f.port,
        team: f.team,
        stale: crate::stale::WeaponStale::of(f),
        position: f.joint_world(BOOMERANG_SPAWN_JOINT, Vec3::ZERO),
        facing: f.facing.sign(),
    });
    f.link.boomerang_out = true;
}

// ---------------------------------------------------------------------------
// Falcon Punch (`ftkirbycopycaptainspecialn.c`)
// ---------------------------------------------------------------------------

/// `ftKirbyCopyCaptainSpecialNGetAngle`.
fn falcon_punch_angle(stick_y: i32) -> f32 {
    let mut y = stick_y.abs().min(50) - 10;
    if y < 0 {
        y = 0;
    }
    if stick_y < 0 {
        y = -y;
    }
    (y * 30) as f32 / 40.0 * (core::f32::consts::PI / 180.0)
}

/// `ftKirbyCopyCaptainSpecialAirNProcPhysics`. Flag 2 is 0 before the
/// boost, 1 from frame 40 and 2 from frame 55.
fn falcon_punch_air_physics(f: &mut Fighter) {
    let attr = f.attributes;
    if !f.kirby.copy.captain_boosted && f.status.clock >= FALCONPUNCH_BOOST_FRAME {
        f.kirby.copy.captain_boosted = true;
        f.kirby.copy.captain_punch_count += 1;
        let (sin, cos) = sin_cos(falcon_punch_angle(i32::from(f.stick.y)));
        f.physics.vel_air.y = sin * FALCONPUNCH_VEL_BASE;
        f.physics.vel_air.x = cos * f.facing.sign() * FALCONPUNCH_VEL_BASE;
    }
    if f.status.clock < FALCONPUNCH_BOOST_FRAME {
        air_vel_friction(f);
    } else if f.status.clock < FALCONPUNCH_DRIFT_FRAME {
        // Kirby's version multiplies by a single, Falcon's by a double.
        f.physics.vel_air.y *= FALCONPUNCH_VEL_MUL;
        f.physics.vel_air.x *= FALCONPUNCH_VEL_MUL;
    } else {
        // `ftPhysicsApplyAirVelDriftFastFall`.
        status::check_set_fast_fall(f);
        gravity(f);
        physics::apply_air_drift(&mut f.physics, &attr, f.input.stick_x);
    }
}

// ---------------------------------------------------------------------------
// Egg Lay (`ftkirbycopyyoshispecialn.c`)
// ---------------------------------------------------------------------------

/// `ftKirbyCopyYoshiSpecialNSetCatchParams`, which pairs with the swallowed
/// fighter's `ftCommonCaptureYoshiProcCapture`.
fn set_egg_lay_catch_params(f: &mut Fighter) {
    f.grab.is_catchstatus = true;
    f.grab.throw_desc = Some(crate::yoshi::EGG_LAY_THROW_DESC);
}

/// Whether Egg Lay's catch box is out.
pub fn egg_lay_searching(f: &Fighter) -> bool {
    matches!(
        f.status.status,
        AnyStatus::Kirby(K::CopyYoshiSpecialN | K::CopyYoshiSpecialAirN)
    ) && EGG_LAY_CATCH_FRAMES.contains(&f.status.clock)
}

/// `ftKirbyCopyYoshiSpecialNCatchProcCatch` / `...AirNCatchProcCatch` and
/// `ftKirbyCopyYoshiSpecialNCatchInitStatusVars`. The status keeps the
/// frame.
pub fn egg_lay_catch(f: &mut Fighter, held: &Fighter) {
    let frame = f.status.anim_frame;
    let s = if f.is_grounded() {
        K::CopyYoshiSpecialNCatch
    } else {
        K::CopyYoshiSpecialAirNCatch
    };
    set(f, s, frame, StatusTiming::frames(EGG_LAY_LENGTH));
    status::play_anim_events(f);
    f.grab.capture_immune = true;
    physics::stop_all(&mut f.physics);
    f.grab.catch = Some(held.port);
    f.grab.catch_kind = Some(held.kind);
    f.grab.is_catchstatus = false;
}

/// `ftKirbyCopyYoshiSpecialNReleaseSetStatus` / `...AirNReleaseSetStatus`.
fn set_egg_lay_release(f: &mut Fighter, ground: bool) {
    let s = if ground {
        K::CopyYoshiSpecialNRelease
    } else {
        K::CopyYoshiSpecialAirNRelease
    };
    set(f, s, 0.0, StatusTiming::frames(EGG_LAY_RELEASE_LENGTH));
    status::play_anim_events(f);
    f.grab.capture_immune = true;
}

/// `ftKirbyCopyYoshiSpecialAirNCatchUpdateCaptureVars`.
fn update_egg_lay_capture_vars(f: &mut Fighter) {
    if crossed(f, EGG_LAY_SWALLOW_FRAME) && f.grab.catch.is_some() {
        f.grab.send(GrabEvent::YoshiEggStage(1));
    }
    if crossed(f, EGG_LAY_LAY_FRAME) && f.grab.catch.take().is_some() {
        f.grab.catch_kind = None;
        f.grab.send(GrabEvent::YoshiEggStage(3));
        f.grab.capture_immune = false;
    }
}

/// The motion script's clock passing `at` this play: the status clock,
/// which runs on past the figatree's end (RE-474).
fn crossed(f: &Fighter, at: f32) -> bool {
    let frame = f.status.clock;
    frame >= at && frame - f.status.timing.anim_speed < at
}

// ---------------------------------------------------------------------------
// Thunder Jolt (`ftkirbycopypikachuspecialn.c`)
// ---------------------------------------------------------------------------

/// `ftKirbyCopyPikachuSpecialNProcAccessory`: offset the root's world
/// position in world X/Y, rather than transforming the offsets by the joint.
fn make_thunder_jolt(f: &mut Fighter) {
    if f.kirby.copy.spawned || f.status.clock < THUNDERJOLT_FRAME {
        return;
    }
    f.kirby.copy.spawned = true;
    f.weapon_spawn = Some(WeaponSpawn {
        kind: WeaponKind::PikachuThunderJolt,
        owner_port: f.port,
        team: f.team,
        stale: crate::stale::WeaponStale::of(f),
        position: f.joint_world(THUNDERJOLT_SPAWN_JOINT, Vec3::ZERO)
            + Vec3::new(
                THUNDERJOLT_SPAWN_OFF_X * f.facing.sign(),
                THUNDERJOLT_SPAWN_OFF_Y,
                0.0,
            ),
        facing: f.facing.sign(),
    });
    crate::colanim::check_set(f, crate::colanim::ColAnimId::FIGHTER_PIKACHU_SPECIAL_N, 0);
}

// ---------------------------------------------------------------------------
// Callbacks
// ---------------------------------------------------------------------------

/// `proc_update` then `proc_interrupt` for the copy statuses.
pub fn update(f: &mut Fighter) {
    let AnyStatus::Kirby(current) = f.status.status else {
        return;
    };
    match current {
        K::CopyMarioSpecialN
        | K::CopyMarioSpecialAirN
        | K::CopyLuigiSpecialN
        | K::CopyLuigiSpecialAirN => {
            make_fireball(f);
            if f.status.animation_ended() {
                status::anim_end_set_wait_or_fall(f);
            }
        }
        K::CopyFoxSpecialN | K::CopyFoxSpecialAirN => update_blaster(f, current),
        K::CopyNessSpecialN | K::CopyNessSpecialAirN => {
            if !f.kirby.copy.spawned && f.status.clock >= 20.0 {
                f.kirby.copy.spawned = true;
                crate::ness::make_pk_fire(f, true);
            }
            if f.status.animation_ended() {
                status::anim_end_set_wait_or_fall(f);
            }
        }
        K::CopyPikachuSpecialN | K::CopyPikachuSpecialAirN => {
            make_thunder_jolt(f);
            if f.status.animation_ended() {
                status::anim_end_set_wait_or_fall(f);
            }
        }
        K::CopySamusSpecialNStart | K::CopySamusSpecialAirNStart => {
            if f.status.animation_ended() {
                if !f.is_grounded() {
                    set_charge_air_end(f);
                } else if f.kirby.copy.samus_is_release {
                    set_charge_end(f);
                } else {
                    set_charge_loop(f);
                }
            }
        }
        K::CopySamusSpecialNLoop => {
            let copy = &mut f.kirby.copy;
            copy.samus_charge_int -= 1;
            if copy.samus_charge_int == 0 {
                copy.samus_charge_int = CHARGE_INT;
                if copy.samus_charge_level < CHARGE_MAX {
                    copy.samus_charge_level += 1;
                    if copy.samus_charge_level == CHARGE_MAX {
                        copy.samus_charge_shot = false;
                        crate::colanim::check_set(
                            f,
                            crate::colanim::ColAnimId::FIGHTER_COMMON_SPECIAL_N_CHARGE,
                            0,
                        );
                        status::set_wait(f);
                    }
                }
            }
        }
        K::CopySamusSpecialNEnd | K::CopySamusSpecialAirNEnd => {
            // `ShootingChargeShot` sets flag 0 on its first frame.
            if !f.kirby.copy.spawned {
                f.kirby.copy.spawned = true;
                fire_charge_shot(f);
            }
            if f.status.animation_ended() {
                status::anim_end_set_wait_or_fall(f);
            }
        }
        K::CopyDonkeySpecialNStart => {
            if f.status.animation_ended() {
                set_giant_punch_loop(f, K::CopyDonkeySpecialNLoop);
            }
        }
        K::CopyDonkeySpecialAirNStart => {
            if f.status.animation_ended() {
                set_giant_punch_loop(f, K::CopyDonkeySpecialAirNLoop);
            }
        }
        K::CopyDonkeySpecialNLoop | K::CopyDonkeySpecialAirNLoop => {
            if f.status.animation_ended() {
                giant_punch_loop_wrap(f, current);
            }
        }
        K::CopyDonkeySpecialNEnd
        | K::CopyDonkeySpecialAirNEnd
        | K::CopyDonkeySpecialNFull
        | K::CopyDonkeySpecialAirNFull => {
            // `ftKirbyCopyDonkeySpecialNEndProcUpdate`: the charge adds to
            // each collision the script makes.
            if f.status.animation_ended() {
                status::anim_end_set_wait_or_fall(f);
            } else {
                let bonus = giant_punch_bonus(f);
                for coll in &mut f.attack_colls {
                    if coll.state == crate::combat::AttackState::New {
                        coll.damage += bonus;
                    }
                }
            }
        }
        K::CopyLinkSpecialN => {
            make_boomerang(f);
            if f.status.animation_ended() {
                status::anim_end_set_wait(f);
            }
        }
        K::CopyLinkSpecialAirN => {
            make_boomerang(f);
            if f.status.animation_ended() {
                status::anim_end_set_fall(f);
            }
        }
        K::CopyLinkSpecialNGet
        | K::CopyLinkSpecialNEmpty
        | K::CopyCaptainSpecialN
        | K::CopyPurinSpecialN
        | K::CopyYoshiSpecialN => {
            if f.status.animation_ended() {
                if current == K::CopyYoshiSpecialN {
                    f.grab.is_catchstatus = false;
                }
                status::anim_end_set_wait(f);
            }
        }
        K::CopyLinkSpecialAirNReturn
        | K::CopyLinkSpecialAirNEmpty
        | K::CopyCaptainSpecialAirN
        | K::CopyPurinSpecialAirN
        | K::CopyYoshiSpecialAirN => {
            if f.status.animation_ended() {
                if current == K::CopyYoshiSpecialAirN {
                    f.grab.is_catchstatus = false;
                }
                status::anim_end_set_fall(f);
            }
        }
        // `ftKirbyCopyYoshiSpecialNCatchUpdateProcStatus`. Flag 1 stays set
        // once the script reaches it.
        K::CopyYoshiSpecialNCatch | K::CopyYoshiSpecialAirNCatch => {
            let flag1 = f.status.clock >= EGG_LAY_CATCH_FLAG1_FRAME;
            if (flag1 && f.grab.catch.is_some()) || f.status.animation_ended() {
                set_egg_lay_release(f, current == K::CopyYoshiSpecialNCatch);
            }
        }
        K::CopyYoshiSpecialNRelease => {
            update_egg_lay_capture_vars(f);
            if f.status.animation_ended() {
                status::anim_end_set_wait(f);
            }
        }
        K::CopyYoshiSpecialAirNRelease => {
            update_egg_lay_capture_vars(f);
            if f.status.animation_ended() {
                status::anim_end_set_fall(f);
            }
        }
        _ => {}
    }
    interrupt(f);
}

/// The copy statuses' `proc_interrupt`, run on whatever status `update`
/// left.
fn interrupt(f: &mut Fighter) {
    let AnyStatus::Kirby(current) = f.status.status else {
        return;
    };
    let taps = taps(f);
    match current {
        K::CopyFoxSpecialN | K::CopyFoxSpecialAirN => interrupt_blaster(f, current),
        K::CopySamusSpecialNStart => {
            if taps.contains(N64Buttons::B) || taps.contains(N64Buttons::A) {
                f.kirby.copy.samus_is_release = true;
            }
        }
        K::CopySamusSpecialNLoop => {
            if taps.contains(N64Buttons::B) || taps.contains(N64Buttons::A) {
                set_charge_end(f);
            } else if taps.contains(N64Buttons::Z) {
                f.kirby.copy.samus_charge_shot = false;
                status::set_wait(f);
            }
        }
        K::CopyDonkeySpecialNStart | K::CopyDonkeySpecialAirNStart => {
            if taps.contains(N64Buttons::B) || taps.contains(N64Buttons::A) {
                f.kirby.copy.donkey_is_release = true;
            }
        }
        K::CopyDonkeySpecialNLoop | K::CopyDonkeySpecialAirNLoop => {
            if taps.contains(N64Buttons::B) || taps.contains(N64Buttons::A) {
                f.kirby.copy.donkey_is_release = true;
            }
            if taps.contains(N64Buttons::Z) {
                f.kirby.copy.donkey_is_cancel = true;
            }
        }
        _ => {}
    }
}

/// The copy statuses' `proc_damage`: Charge Shot (until the shot leaves)
/// and Giant Punch's charge (until the punch) spend the stored charge.
pub fn on_damage(f: &mut Fighter) {
    let AnyStatus::Kirby(current) = f.status.status else {
        return;
    };
    match current {
        K::CopySamusSpecialNStart | K::CopySamusSpecialNLoop | K::CopySamusSpecialAirNStart => {
            f.kirby.copy.samus_charge_level = 0;
            f.kirby.copy.samus_charge_shot = false;
        }
        K::CopySamusSpecialNEnd | K::CopySamusSpecialAirNEnd if !f.kirby.copy.spawned => {
            f.kirby.copy.samus_charge_level = 0;
            f.kirby.copy.samus_charge_shot = false;
        }
        K::CopyDonkeySpecialNStart
        | K::CopyDonkeySpecialAirNStart
        | K::CopyDonkeySpecialNLoop
        | K::CopyDonkeySpecialAirNLoop => f.kirby.copy.donkey_charge_level = 0,
        _ => {}
    }
}

fn gravity(f: &mut Fighter) {
    if f.physics.is_fastfall {
        physics::apply_fast_fall(&mut f.physics, &f.attributes);
    } else {
        physics::apply_gravity_default(&mut f.physics, &f.attributes);
    }
}

/// `ftPhysicsApplyAirVelFriction`.
fn air_vel_friction(f: &mut Fighter) {
    let attr = f.attributes;
    gravity(f);
    if !physics::check_clamp_air_vel_x_dec(&mut f.physics, attr.air_speed_max_x) {
        physics::apply_air_friction(&mut f.physics, &attr);
    }
}

/// Grounded `proc_physics` where it is not plain friction.
pub fn apply_ground_physics(f: &mut Fighter) -> bool {
    if !matches!(
        f.status.status,
        AnyStatus::Kirby(K::CopyCaptainSpecialN | K::CopyPurinSpecialN)
    ) {
        return false;
    }
    physics::apply_ground_vel_transn(&mut f.physics, f.root_motion, f.topn_lr, f.attributes.size);
    true
}

/// Aerial `proc_physics` of the copy statuses.
pub fn apply_air_physics(f: &mut Fighter) -> bool {
    let AnyStatus::Kirby(current) = f.status.status else {
        return false;
    };
    if !is_copy(current) {
        return false;
    }
    match current {
        // `ftPhysicsApplyAirVelDrift`.
        K::CopyMarioSpecialAirN
        | K::CopyLuigiSpecialAirN
        | K::CopyFoxSpecialAirN
        | K::CopySamusSpecialAirNStart => {
            let attr = f.attributes;
            gravity(f);
            physics::apply_air_drift(&mut f.physics, &attr, f.input.stick_x);
        }
        K::CopyCaptainSpecialAirN => falcon_punch_air_physics(f),
        // `ftKirbyCopyPurinSpecialAirNProcPhysics`.
        K::CopyPurinSpecialAirN => {
            let mut boosted = f.kirby.copy.purin_boosted;
            let mut count = f.kirby.copy.purin_count;
            crate::purin::pound_air_physics(f, &mut boosted, &mut count);
            f.kirby.copy.purin_boosted = boosted;
            f.kirby.copy.purin_count = count;
        }
        _ => air_vel_friction(f),
    }
    true
}

/// A switch that keeps the animation frame and clock.
fn switch(f: &mut Fighter, s: K) {
    let (frame, mut timing) = (f.status.anim_frame, f.status.timing);
    if matches!(s, K::CopyNessSpecialN | K::CopyNessSpecialAirN) {
        timing = StatusTiming::frames(if s == K::CopyNessSpecialN { 72.0 } else { 60.0 });
    }
    set(f, s, frame, timing);
}

/// Grounded `proc_map` of the copy statuses when the floor ran out.
pub fn on_ground_lost(f: &mut Fighter) -> bool {
    let AnyStatus::Kirby(current) = f.status.status else {
        return false;
    };
    if !is_copy(current) || !is_grounded(current) {
        return false;
    }
    f.become_airborne();
    let clamp = |f: &mut Fighter| {
        physics::clamp_air_vel_x(&mut f.physics, f.attributes.air_speed_max_x);
    };
    match current {
        K::CopyMarioSpecialN => {
            switch(f, K::CopyMarioSpecialAirN);
            clamp(f);
        }
        K::CopyLuigiSpecialN => {
            switch(f, K::CopyLuigiSpecialAirN);
            clamp(f);
        }
        K::CopyNessSpecialN => {
            switch(f, K::CopyNessSpecialAirN);
            clamp(f);
        }
        K::CopyPikachuSpecialN => {
            switch(f, K::CopyPikachuSpecialAirN);
            clamp(f);
        }
        // `mpCommonProcFighterOnFloor`, where Jigglypuff's is `OnEdge`.
        K::CopyPurinSpecialN => {
            switch(f, K::CopyPurinSpecialAirN);
            clamp(f);
        }
        K::CopySamusSpecialNStart => {
            switch(f, K::CopySamusSpecialAirNStart);
            clamp(f);
            f.kirby.copy.samus_is_release = true;
        }
        // `ftKirbyCopySamusSpecialNLoopProcMap` fires as it leaves the edge.
        K::CopySamusSpecialNLoop => set_charge_air_end(f),
        K::CopySamusSpecialNEnd => {
            switch(f, K::CopySamusSpecialAirNEnd);
            clamp(f);
        }
        K::CopyDonkeySpecialNStart => {
            switch(f, K::CopyDonkeySpecialAirNStart);
            clamp(f);
        }
        K::CopyDonkeySpecialNLoop => {
            switch(f, K::CopyDonkeySpecialAirNLoop);
            clamp(f);
        }
        K::CopyLinkSpecialN => switch(f, K::CopyLinkSpecialAirN),
        K::CopyLinkSpecialNEmpty => {
            switch(f, K::CopyLinkSpecialAirNEmpty);
            f.is_special_interrupt = true;
        }
        K::CopyCaptainSpecialN => {
            switch(f, K::CopyCaptainSpecialAirN);
            clamp(f);
        }
        // The search carries over.
        K::CopyYoshiSpecialN => {
            switch(f, K::CopyYoshiSpecialAirN);
            set_egg_lay_catch_params(f);
        }
        K::CopyYoshiSpecialNCatch => switch(f, K::CopyYoshiSpecialAirNCatch),
        K::CopyYoshiSpecialNRelease => switch(f, K::CopyYoshiSpecialAirNRelease),
        // `mpCommonSetFighterFallOnEdgeBreak` / `...OnGroundBreak`: Fox,
        // Giant Punch's swings and the Boomerang catch.
        _ => status::set_fall(f),
    }
    true
}

/// Aerial `proc_map` of the copy statuses on a floor contact.
pub fn on_landing(f: &mut Fighter, y: f32) -> bool {
    let AnyStatus::Kirby(current) = f.status.status else {
        return false;
    };
    if !is_copy(current) || is_grounded(current) {
        return false;
    }
    let ground = match current {
        K::CopyMarioSpecialAirN => K::CopyMarioSpecialN,
        K::CopyLuigiSpecialAirN => K::CopyLuigiSpecialN,
        K::CopyNessSpecialAirN => K::CopyNessSpecialN,
        K::CopyPikachuSpecialAirN => K::CopyPikachuSpecialN,
        K::CopyPurinSpecialAirN => K::CopyPurinSpecialN,
        K::CopySamusSpecialAirNStart => K::CopySamusSpecialNStart,
        K::CopySamusSpecialAirNEnd => K::CopySamusSpecialNEnd,
        K::CopyDonkeySpecialAirNStart => K::CopyDonkeySpecialNStart,
        K::CopyDonkeySpecialAirNLoop => K::CopyDonkeySpecialNLoop,
        K::CopyDonkeySpecialAirNEnd => K::CopyDonkeySpecialNEnd,
        K::CopyDonkeySpecialAirNFull => K::CopyDonkeySpecialNFull,
        K::CopyLinkSpecialAirN => K::CopyLinkSpecialN,
        K::CopyLinkSpecialAirNEmpty => K::CopyLinkSpecialNEmpty,
        K::CopyCaptainSpecialAirN => K::CopyCaptainSpecialN,
        K::CopyYoshiSpecialAirN => K::CopyYoshiSpecialN,
        K::CopyYoshiSpecialAirNCatch => K::CopyYoshiSpecialNCatch,
        K::CopyYoshiSpecialAirNRelease => K::CopyYoshiSpecialNRelease,
        // `mpCommonProcFighterWaitOrLanding`: the Blaster and the
        // Boomerang catch.
        _ => {
            crate::kirby::wait_or_landing(f, y);
            return true;
        }
    };
    f.land(y);
    switch(f, ground);
    match ground {
        K::CopyLinkSpecialNEmpty => f.is_special_interrupt = true,
        K::CopyYoshiSpecialN => set_egg_lay_catch_params(f),
        _ => {}
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fighter::Situation;
    use ssb_engine::input::ControllerState;

    /// The copy hat follows `copy_id` through `COPY_MODELPART_IDS`; Kirby's
    /// own head and other fighters wear none (RE-417).
    #[test]
    fn the_copy_hat_is_the_copied_fighters_model_part() {
        assert_eq!(copy_hat(&kirby(FighterKind::Mario, true)), Some(12));
        assert_eq!(copy_hat(&kirby(FighterKind::Ness, true)), Some(13));
        assert_eq!(copy_hat(&kirby(FighterKind::Kirby, true)), None);
        let mut f = kirby(FighterKind::Pikachu, true);
        assert_eq!(copy_hat(&f), Some(6));
        crate::kirby::lose_copy(&mut f);
        assert_eq!(copy_hat(&f), None);
        let mut mario = Fighter::new(FighterKind::Mario, 0, 3);
        mario.kirby.copy_id = FighterKind::Fox;
        assert_eq!(copy_hat(&mario), None);
    }

    fn kirby(copy: FighterKind, ground: bool) -> Fighter {
        let mut f = Fighter::new(FighterKind::Kirby, 0, 3);
        f.situation = if ground {
            Situation::Ground
        } else {
            Situation::Air
        };
        f.kirby.copy_id = copy;
        f
    }

    fn press(f: &mut Fighter, buttons: u16) {
        let input = ControllerState {
            buttons: N64Buttons(buttons),
            ..Default::default()
        };
        f.set_input(input, false, false);
    }

    fn step(f: &mut Fighter) {
        crate::status::play_anim(f);
        update(f);
    }

    fn status(f: &Fighter) -> K {
        match f.status.status {
            AnyStatus::Kirby(s) => s,
            other => panic!("not a Kirby status: {other:?}"),
        }
    }

    #[test]
    fn the_copy_picks_the_special_and_a_kirby_without_one_still_inhales() {
        let mut f = kirby(FighterKind::Luigi, true);
        assert!(set_special_n(&mut f));
        assert_eq!(status(&f), K::CopyLuigiSpecialN);
        let mut f = kirby(FighterKind::Captain, false);
        assert!(set_special_n(&mut f));
        assert_eq!(status(&f), K::CopyCaptainSpecialAirN);
        let mut f = kirby(FighterKind::Ness, true);
        assert!(set_special_n(&mut f));
        assert_eq!(status(&f), K::CopyNessSpecialN);
        let mut f = kirby(FighterKind::Kirby, true);
        crate::kirby::set_special_n(&mut f);
        assert_eq!(status(&f), K::SpecialNStart);
    }

    #[test]
    fn copied_fireball_leaves_at_frame_16_with_luigis_row() {
        let mut f = kirby(FighterKind::Luigi, true);
        set_special_n(&mut f);
        // `WaitAsync(16)` then `SetFlag0(1)`; the setter played frame 1
        // (RE-474).
        for _ in 0..14 {
            step(&mut f);
            assert!(f.weapon_spawn.is_none());
        }
        step(&mut f);
        let spawn = f.take_weapon_spawn().expect("the Fireball is made");
        assert_eq!(spawn.kind, WeaponKind::LuigiFireball);
        for _ in 0..40 {
            step(&mut f);
        }
        assert!(f.weapon_spawn.is_none(), "one Fireball per status");
        assert_eq!(
            f.status.status,
            AnyStatus::Common(crate::status::Status::Wait)
        );
    }

    #[test]
    fn pikachu_copy_selects_its_ground_and_air_figatrees_and_attack_id() {
        for (ground, expected, slot) in [
            (true, K::CopyPikachuSpecialN, 373),
            (false, K::CopyPikachuSpecialAirN, 374),
        ] {
            let mut f = kirby(FighterKind::Pikachu, ground);
            assert!(set_special_n(&mut f));
            assert_eq!(status(&f), expected);
            assert!(is_copy(expected));
            assert_eq!(is_grounded(expected), ground);
            assert_eq!(anim_slot(expected), Some(slot));
            assert_eq!(
                attack_id(expected),
                Some(MotionAttackId::SpecialNCopyPikachu)
            );
            assert_eq!(f.status.timing, StatusTiming::frames(64.0));
        }
    }

    #[test]
    fn copied_thunder_jolt_leaves_at_21_from_world_offsets_of_joint_zero() {
        use crate::fighter::{Facing, JointTransform};
        for ground in [true, false] {
            let mut f = kirby(FighterKind::Pikachu, ground);
            f.facing = Facing::Left;
            f.pos = Vec3::new(-500.0, -500.0, 0.0);
            // Rotated/scaled axes must not transform the source's world offsets.
            f.joint_transforms[0] = Some(JointTransform {
                axes: [
                    Vec3::new(0.0, 2.0, 0.0),
                    Vec3::new(-2.0, 0.0, 0.0),
                    Vec3::new(0.0, 0.0, 2.0),
                ],
                origin: Vec3::new(100.0, 300.0, 50.0),
            });
            set_special_n(&mut f);
            // The setter played frame 1 (RE-474).
            for _ in 0..19 {
                step(&mut f);
                assert!(f.weapon_spawn.is_none());
            }
            step(&mut f);
            let spawn = f.take_weapon_spawn().expect("Thunder Jolt at frame 21");
            assert_eq!(spawn.kind, WeaponKind::PikachuThunderJolt);
            assert_eq!(spawn.position, Vec3::new(-100.0, 500.0, 50.0));
            assert_eq!(spawn.facing, -1.0);
            for _ in 21..64 {
                step(&mut f);
                assert!(f.weapon_spawn.is_none());
            }
            assert_eq!(
                f.status.status,
                AnyStatus::Common(if ground {
                    crate::status::Status::Wait
                } else {
                    crate::status::Status::Fall
                })
            );
        }
    }

    #[test]
    fn copied_thunder_jolt_keeps_spawn_consumption_and_frame_across_map_switches() {
        let mut f = kirby(FighterKind::Pikachu, true);
        set_special_n(&mut f);
        for _ in 0..19 {
            step(&mut f);
        }
        // The last ground step, which `vel_air` holds on the ground
        // (`ftPhysicsSetGroundVelTransferAir`).
        f.physics.vel_air.x = 100.0;
        assert!(on_ground_lost(&mut f));
        assert_eq!(status(&f), K::CopyPikachuSpecialAirN);
        assert_eq!(f.status.anim_frame, 20.0);
        assert_eq!(f.physics.vel_air.x, f.attributes.air_speed_max_x);
        step(&mut f);
        assert!(f.take_weapon_spawn().is_some());
        assert!(on_landing(&mut f, 0.0));
        assert_eq!(status(&f), K::CopyPikachuSpecialN);
        assert_eq!(f.status.anim_frame, 21.0);
        step(&mut f);
        assert!(
            f.weapon_spawn.is_none(),
            "landing preserves the consumed flag"
        );
        assert!(on_ground_lost(&mut f));
        step(&mut f);
        assert!(
            f.weapon_spawn.is_none(),
            "leaving a ledge preserves the consumed flag"
        );
    }

    #[test]
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
    fn copied_blaster_refires_on_b_after_flag_1_as_a_new_motion() {
        let mut f = kirby(FighterKind::Fox, true);
        set_special_n(&mut f);
        let first = f.motion.count;
        while f.status.anim_frame < BLASTER_FRAME {
            step(&mut f);
        }
        assert!(f.take_weapon_spawn().is_some());
        press(&mut f, N64Buttons::B);
        step(&mut f);
        assert!(f.status.anim_frame > 1.0, "B before frame 29 is ignored");
        press(&mut f, 0);
        while f.status.anim_frame < BLASTER_REPEAT_FRAME {
            step(&mut f);
        }
        press(&mut f, N64Buttons::B);
        step(&mut f);
        assert_eq!(status(&f), K::CopyFoxSpecialN);
        // The setter plays the new motion's first frame (RE-474).
        assert_eq!(f.status.anim_frame, 1.0);
        assert_ne!(f.motion.count, first);
    }

    #[test]
    fn copied_charge_shot_stores_levels_and_kicks_kirby_back_on_release() {
        let mut f = kirby(FighterKind::Samus, true);
        set_special_n(&mut f);
        while status(&f) == K::CopySamusSpecialNStart {
            step(&mut f);
        }
        assert_eq!(status(&f), K::CopySamusSpecialNLoop);
        for _ in 0..(2 * CHARGE_INT) {
            step(&mut f);
        }
        assert_eq!(f.kirby.copy.samus_charge_level, 2);
        press(&mut f, N64Buttons::B);
        step(&mut f);
        assert_eq!(status(&f), K::CopySamusSpecialNEnd);
        press(&mut f, 0);
        step(&mut f);
        let spawn = f.take_weapon_spawn().expect("the shot leaves");
        assert_eq!(spawn.kind, WeaponKind::SamusChargeShot(2));
        assert_eq!(f.kirby.copy.samus_charge_level, 0);
        assert_eq!(f.physics.vel_ground.x, -16.0);
    }

    #[test]
    fn copied_charge_shot_recoil_survives_a_landing() {
        let mut f = kirby(FighterKind::Samus, false);
        set_special_n(&mut f);
        while status(&f) == K::CopySamusSpecialAirNStart {
            step(&mut f);
        }
        assert_eq!(status(&f), K::CopySamusSpecialAirNEnd);
        step(&mut f);
        assert_eq!(f.kirby.copy.samus_charge_recoil, 1);
        f.land(0.0);
        status::set_wait(&mut f);
        assert_eq!(f.kirby.copy.samus_charge_recoil, 1);
    }

    #[test]
    fn copied_giant_punch_charges_per_swing_and_adds_two_damage_a_level() {
        let mut f = kirby(FighterKind::Donkey, true);
        set_special_n(&mut f);
        while status(&f) == K::CopyDonkeySpecialNStart {
            step(&mut f);
        }
        assert_eq!(status(&f), K::CopyDonkeySpecialNLoop);
        // The first wrap only starts charging.
        for _ in 0..(3 * GIANTPUNCH_LOOP_LENGTH as usize) {
            step(&mut f);
        }
        assert_eq!(f.kirby.copy.donkey_charge_level, 2);
        press(&mut f, N64Buttons::A);
        step(&mut f);
        press(&mut f, 0);
        while status(&f) == K::CopyDonkeySpecialNLoop {
            step(&mut f);
        }
        assert_eq!(status(&f), K::CopyDonkeySpecialNEnd);
        assert_eq!(f.kirby.copy.donkey_charge_level, 0);
        assert_eq!(f.kirby.copy.donkey_attack_charge, 3);
        assert_eq!(giant_punch_bonus(&f), 6);
        assert_eq!(f.physics.vel_ground.x, 24.0);
    }

    #[test]
    fn a_full_giant_punch_cancels_the_charge_then_swings_full_at_double_speed() {
        let mut f = kirby(FighterKind::Donkey, true);
        f.kirby.copy.donkey_charge_level = GIANTPUNCH_CHARGE_MAX - 1;
        set_special_n(&mut f);
        while status(&f) == K::CopyDonkeySpecialNStart {
            step(&mut f);
        }
        while f.status.status == AnyStatus::Kirby(K::CopyDonkeySpecialNLoop) {
            step(&mut f);
        }
        assert_eq!(
            f.status.status,
            AnyStatus::Common(crate::status::Status::Wait)
        );
        assert_eq!(f.kirby.copy.donkey_charge_level, GIANTPUNCH_CHARGE_MAX);
        set_special_n(&mut f);
        assert!(f.kirby.copy.donkey_is_release);
        while status(&f) == K::CopyDonkeySpecialNStart {
            step(&mut f);
        }
        assert_eq!(f.status.timing.anim_speed, GIANTPUNCH_CHARGE_ANIM_SPEED);
        for _ in 0..6 {
            step(&mut f);
        }
        assert_eq!(status(&f), K::CopyDonkeySpecialNFull);
        assert_eq!(giant_punch_bonus(&f), 0);
    }

    #[test]
    fn a_hit_while_charging_spends_the_giant_punch_charge() {
        let mut f = kirby(FighterKind::Donkey, true);
        f.kirby.copy.donkey_charge_level = 4;
        set_special_n(&mut f);
        on_damage(&mut f);
        assert_eq!(f.kirby.copy.donkey_charge_level, 0);
    }

    #[test]
    fn copied_boomerang_leaves_at_26_and_an_outstanding_one_empties_the_throw() {
        let mut f = kirby(FighterKind::Link, true);
        set_special_n(&mut f);
        while f.status.anim_frame < BOOMERANG_FRAME {
            step(&mut f);
        }
        let spawn = f.take_weapon_spawn().expect("the Boomerang is thrown");
        assert!(matches!(spawn.kind, WeaponKind::LinkBoomerang { .. }));
        assert!(f.link.boomerang_out);
        status::set_wait(&mut f);
        set_special_n(&mut f);
        assert_eq!(status(&f), K::CopyLinkSpecialNEmpty);
        assert!(f.is_special_interrupt);
        set_boomerang_get(&mut f);
        assert_eq!(status(&f), K::CopyLinkSpecialNGet);
    }

    #[test]
    fn aerial_falcon_punch_launches_at_40_then_damps() {
        let mut f = kirby(FighterKind::Captain, false);
        set_special_n(&mut f);
        while f.status.anim_frame < FALCONPUNCH_BOOST_FRAME {
            step(&mut f);
            apply_air_physics(&mut f);
        }
        assert_eq!(f.kirby.copy.captain_punch_count, 1);
        let launched = FALCONPUNCH_VEL_BASE * FALCONPUNCH_VEL_MUL;
        assert!((f.physics.vel_air.x - launched).abs() < 1e-3);
        step(&mut f);
        apply_air_physics(&mut f);
        assert!((f.physics.vel_air.x - launched * FALCONPUNCH_VEL_MUL).abs() < 1e-3);
    }

    #[test]
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
    fn copied_egg_lay_searches_18_to_23_then_lays_the_catch() {
        let mut f = kirby(FighterKind::Yoshi, true);
        set_special_n(&mut f);
        assert!(f.grab.is_catchstatus);
        while f.status.anim_frame < 17.0 {
            step(&mut f);
            assert!(!egg_lay_searching(&f));
        }
        step(&mut f);
        assert!(egg_lay_searching(&f));
        let held = Fighter::new(FighterKind::Mario, 1, 3);
        egg_lay_catch(&mut f, &held);
        assert_eq!(status(&f), K::CopyYoshiSpecialNCatch);
        while status(&f) == K::CopyYoshiSpecialNCatch {
            step(&mut f);
        }
        // The release's setter plays its first frame (RE-474).
        assert_eq!(f.status.anim_frame, 1.0);
        assert_eq!(status(&f), K::CopyYoshiSpecialNRelease);
        for _ in 0..20 {
            step(&mut f);
        }
        assert!(f.grab.catch.is_none());
        assert!(!f.grab.capture_immune);
    }

    #[test]
    fn copy_statuses_switch_between_ground_and_air_keeping_the_frame() {
        let mut f = kirby(FighterKind::Mario, true);
        set_special_n(&mut f);
        for _ in 0..5 {
            step(&mut f);
        }
        assert!(on_ground_lost(&mut f));
        assert_eq!(f.situation, Situation::Air);
        assert_eq!(status(&f), K::CopyMarioSpecialAirN);
        // Frame 1 from the setter's play, then five (RE-474).
        assert_eq!(f.status.anim_frame, 6.0);
        assert!(on_landing(&mut f, 0.0));
        assert_eq!(status(&f), K::CopyMarioSpecialN);
        let mut f = kirby(FighterKind::Fox, true);
        set_special_n(&mut f);
        assert!(on_ground_lost(&mut f));
        assert_eq!(
            f.status.status,
            AnyStatus::Common(crate::status::Status::Fall)
        );
    }

    #[test]
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
    fn copied_pound_uses_jigglypuffs_boost_and_floor_callback() {
        let mut f = kirby(FighterKind::Purin, true);
        status::set_wait(&mut f);
        assert!(set_special_n(&mut f));
        assert_eq!(status(&f), K::CopyPurinSpecialN);
        assert_eq!(f.status.status.anim_slot(), 402);
        assert!(on_ground_lost(&mut f));
        assert_eq!(status(&f), K::CopyPurinSpecialAirN);
        while f.status.anim_frame < crate::purin::POUND_BOOST_FRAME {
            status::update(&mut f);
            apply_air_physics(&mut f);
        }
        assert_eq!(f.kirby.copy.purin_count, 1);
        let x = crate::purin::POUND_VEL_BASE * crate::purin::POUND_VEL_MUL * f.facing.sign();
        assert!((f.physics.vel_air.x - x).abs() < 1e-3);
        assert!(on_landing(&mut f, 0.0));
        assert_eq!(status(&f), K::CopyPurinSpecialN);
        // `dKirbyMainMotion_Pound` drives the grounded half too.
        assert!(crate::motion::motion_desc(FighterKind::Kirby, f.status.status).is_some());
    }
}
