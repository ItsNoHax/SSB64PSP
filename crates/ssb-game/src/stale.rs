//! Stale-move queue, motion IDs and handicaps — `ft/ftparam.c`'s
//! `ftParamGetStale`, `ftParamGetStaledDamage`, `ftParamSetMotionID`,
//! `ftParamUpdateStaleQueue`, plus `dFTCommonDataHandicapTable`
//! (`ft/ftcommondata.c`) as `ftParamGetCommonKnockback` reads it.
//!
//! ## Where the state lives
//!
//! The original keeps each player's queue in
//! `gSCManagerBattleState->players[player].stale_info[5]` and hands out
//! motion counts from the global `gFTManagerMotionCount`. Here the queue lives
//! on the attacking [`Fighter`] ([`Fighter::stale`]), and each fighter counts
//! its own motions. The two are equivalent: a player's queue only ever holds
//! that player's own `(attack_id, motion_count)` pairs, and the lookup only
//! compares counts for equality, so any counter that never repeats a value
//! for one fighter gives the same staling. The only difference is when the
//! 16-bit counter wraps (after 65,535 of that fighter's own motions rather
//! than everyone's).
//!
//! ## Documented deviations
//!
//! * **Damage is staled at contact, not at `MakeAttackColl`.** The source
//!   stales a hitbox's damage once, when the motion command creates it. Here
//!   it is staled when the hit lands. The queue can only change between the
//!   two if another of the attacker's motions lands a hit while the box is
//!   live (a projectile, say), which ported moves do not do.
//! * **Weapons are not staled yet.** `wpMainGetStaledDamage` multiplies by a
//!   `stale` captured when the weapon spawns; weapon hits do not feed the
//!   queue either. See `TODO.md`.

use crate::fighter::{Fighter, FighterKind};
use crate::status::{
    AnyStatus, CaptainStatus, DonkeyStatus, FoxStatus, LinkStatus, MarioStatus, SamusStatus,
    Status, YoshiStatus,
};

/// `FTMotionAttackIndex` (`ft/ftdef.h`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u16)]
pub enum MotionAttackId {
    #[default]
    None = 0,
    Attack11,
    Attack12,
    Attack13,
    Attack100,
    AttackDash,
    AttackS3,
    AttackHi3,
    AttackLw3,
    AttackS4,
    AttackHi4,
    AttackLw4,
    AttackAirN,
    AttackAirF,
    AttackAirB,
    AttackAirHi,
    AttackAirLw,
    SpecialHi,
    SpecialN,
    SpecialNCopyMario,
    SpecialNCopyLuigi,
    SpecialNCopyFox,
    SpecialNCopySamus,
    SpecialNCopyDonkey,
    SpecialNCopyPikachu,
    SpecialNCopyNess,
    SpecialNCopyLink,
    SpecialNCopyPurin,
    SpecialNCopyCaptain,
    SpecialNCopyYoshi,
    SpecialLw,
    DownAttackD,
    DownAttackU,
    CliffAttackQuick,
    CliffAttackSlow,
    ThrowF,
    ThrowB,
    SwordSwing1,
    SwordSwing3,
    SwordSwing4,
    SwordSwingDash,
    BatSwing1,
    BatSwing3,
    BatSwing4,
    BatSwingDash,
    HarisenSwing1,
    HarisenSwing3,
    HarisenSwing4,
    HarisenSwingDash,
    StarRodSwing1,
    StarRodSwing3,
    StarRodSwing4,
    StarRodSwingDash,
    LGunShoot,
    FireFlowerShoot,
    Hammer,
    ItemThrow,
}

/// `dFTParamStaleTable` (`ftparam.c`): the multiplier for a move found 1st,
/// 2nd, 3rd or 4th most recently in the queue.
pub const STALE_TABLE: [f32; 4] = [0.75, 0.82, 0.89, 0.96];

/// `ARRAY_COUNT(SCPlayerData::stale_info)`.
pub const STALE_QUEUE_LEN: usize = 5;

/// `FTCOMMON_HANDICAP_DEFAULT` (`ft/ftdef.h`). Training's battle state
/// (`dSCManagerDefaultBattleState`) gives every player this handicap.
pub const HANDICAP_DEFAULT: u8 = 9;

/// `dSCManagerDefaultBattleState.damage_ratio`, which Training copies.
pub const DAMAGE_RATIO_DEFAULT: u8 = 100;

/// `dFTCommonDataHandicapTable` (US), indexed by `handicap - 1`: `[0]`
/// scales knockback dealt, `[1]` knockback received. Written with the same
/// float expressions as the source so the constants round identically.
#[rustfmt::skip]
pub const HANDICAP_TABLE: [[f32; 2]; 34] = [
    [0.55, 1.818_181_8],
    [0.6, 15.0 / 9.0],
    [0.7, 10.0 / 7.0],
    [0.75, 12.0 / 9.0],
    [0.8, 11.25 / 9.0],
    [0.9, 10.0 / 9.0],
    [0.95, 10.0 / 9.5],
    [1.0, 1.0], // `9.0F / 9.0F`
    [1.09, 0.917_431_2],
    [0.5, 45.0 / 9.0],
    [0.55, 36.0 / 9.0],
    [0.6, 30.0 / 9.0],
    [0.65, 20.0 / 7.0],
    [0.7, 22.5 / 9.0],
    [0.75, 1.818_181_8],
    [0.8, 1.694_915_3],
    [0.85, 1.587_301_6],
    [0.9, 1.492_537_3],
    [0.95, 1.408_450_7],
    [0.65, 30.0 / 9.0],
    [0.7, 20.0 / 7.0],
    [0.74, 22.5 / 9.0],
    [0.77, 20.0 / 9.0],
    [0.8, 18.0 / 9.0],
    [1.0, 4.0 / 7.0],
    [1.05, 0.537_634_43],
    [1.1, 0.512_820_5],
    [1.15, 0.476_190_5],
    [1.23, 0.450_450_45],
    [1.05, 0.434_782_62],
    [1.1, 3.0 / 7.5],
    [1.15, (10.0 / 3.0) / 9.0],
    [1.2, 0.357_142_87],
    [1.25, 3.0 / 9.0],
];

/// `ftParamGetCommonKnockback`'s trailing factors:
/// `* (damage_ratio * 0.01F) * table[attack - 1][0]`, then
/// `* table[defend - 1][1]`, in that order.
pub fn apply_ratio_and_handicap(base: f32, damage_ratio: u8, attack: u8, defend: u8) -> f32 {
    let row = |h: u8| usize::from(h.clamp(1, HANDICAP_TABLE.len() as u8)) - 1;
    let atk = HANDICAP_TABLE[row(attack)][0];
    let def = HANDICAP_TABLE[row(defend)][1];
    (base * (f32::from(damage_ratio) * 0.01) * atk) * def
}

/// `SCPlayerData::stale_info` and `stale_id`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct StaleQueue {
    /// `stale_id`: the slot the next landed move is written to.
    pub next: usize,
    /// `stale_info[i] = { attack_id, motion_count }`.
    pub entries: [(MotionAttackId, u16); STALE_QUEUE_LEN],
}

impl StaleQueue {
    /// `ftParamGetStale` @ 0x800EA470. Walks back from the newest entry over
    /// four slots; the newest slot matching the current motion itself does
    /// not count and extends the walk by one.
    pub fn stale(&self, attack_id: MotionAttackId, motion_count: u16) -> f32 {
        if attack_id == MotionAttackId::None {
            return 1.0;
        }
        let prev = |id: usize| if id != 0 { id - 1 } else { STALE_QUEUE_LEN - 1 };
        let start = prev(self.next);
        let mut current = start;
        let mut i: i32 = 0;
        while i < STALE_TABLE.len() as i32 {
            let (id, count) = self.entries[current];
            if id == attack_id {
                if count != motion_count {
                    return STALE_TABLE[i as usize];
                } else if current == start {
                    i -= 1;
                }
            }
            current = prev(current);
            i += 1;
        }
        1.0
    }

    /// `ftParamGetStaledDamage` @ 0x800EA54C.
    pub fn staled_damage(&self, damage: i32, attack_id: MotionAttackId, motion_count: u16) -> i32 {
        let stale = self.stale(attack_id, motion_count);
        if stale != 1.0 {
            (damage as f32 * stale + 0.999) as i32
        } else {
            damage
        }
    }

    /// `ftParamUpdateStaleQueue` @ 0x800EA614, for an attacker that is not
    /// the defender (the caller checks `attack_player != defend_player`).
    pub fn push(&mut self, attack_id: MotionAttackId, motion_count: u16) {
        if self
            .entries
            .iter()
            .any(|&(id, count)| id == attack_id && count == motion_count)
        {
            return;
        }
        self.entries[self.next] = (attack_id, motion_count);
        self.next = if self.next == STALE_QUEUE_LEN - 1 {
            0
        } else {
            self.next + 1
        };
    }
}

/// A fighter's current `motion_attack_id` / `motion_count` and the counter
/// that hands out counts (`gFTManagerMotionCount`, module docs).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MotionId {
    pub attack_id: MotionAttackId,
    pub count: u16,
    next_count: u16,
}

impl Default for MotionId {
    /// `ftManagerMakeFighter` sets `motion_attack_id = None`,
    /// `motion_count = 0`; `ftManagerInitFighters` starts the counter at 1.
    fn default() -> Self {
        MotionId {
            attack_id: MotionAttackId::None,
            count: 0,
            next_count: 1,
        }
    }
}

impl MotionId {
    /// `ftParamSetMotionID` @ 0x800EA5E8 with `ftParamGetMotionCount`,
    /// which skips zero when the counter wraps.
    pub fn set(&mut self, attack_id: MotionAttackId) {
        self.attack_id = attack_id;
        self.count = self.next_count;
        self.next_count = self.next_count.wrapping_add(1);
        if self.next_count == 0 {
            self.next_count = 1;
        }
    }
}

/// `FTStatusDesc::mflags.attack_id` for the statuses the port has, from
/// `ftcommonstatus.h` and the per-fighter status tables (`ft<name>status.h`).
/// Luigi shares Mario's special statuses.
pub fn status_attack_id(kind: FighterKind, status: AnyStatus) -> MotionAttackId {
    use MotionAttackId as M;
    let _ = kind;
    match status {
        AnyStatus::Common(s) => match s {
            Status::DownAttackD => M::DownAttackD,
            Status::DownAttackU => M::DownAttackU,
            Status::CliffAttackQuick1 | Status::CliffAttackQuick2 => M::CliffAttackQuick,
            Status::CliffAttackSlow1 | Status::CliffAttackSlow2 => M::CliffAttackSlow,
            Status::SwordSwing1 => M::SwordSwing1,
            Status::SwordSwing3 => M::SwordSwing3,
            Status::SwordSwing4 => M::SwordSwing4,
            Status::SwordSwingDash => M::SwordSwingDash,
            Status::BatSwing1 => M::BatSwing1,
            Status::BatSwing3 => M::BatSwing3,
            Status::BatSwing4 => M::BatSwing4,
            Status::BatSwingDash => M::BatSwingDash,
            Status::HarisenSwing1 => M::HarisenSwing1,
            Status::HarisenSwing3 => M::HarisenSwing3,
            Status::HarisenSwing4 => M::HarisenSwing4,
            Status::HarisenSwingDash => M::HarisenSwingDash,
            Status::StarRodSwing1 => M::StarRodSwing1,
            Status::StarRodSwing3 => M::StarRodSwing3,
            Status::StarRodSwing4 => M::StarRodSwing4,
            Status::StarRodSwingDash => M::StarRodSwingDash,
            Status::LGunShoot | Status::LGunShootAir => M::LGunShoot,
            Status::FireFlowerShoot | Status::FireFlowerShootAir => M::FireFlowerShoot,
            Status::ThrowF => M::ThrowF,
            Status::ThrowB => M::ThrowB,
            Status::Attack11 => M::Attack11,
            Status::Attack12 => M::Attack12,
            Status::AttackDash => M::AttackDash,
            Status::AttackS3Hi
            | Status::AttackS3HiS
            | Status::AttackS3
            | Status::AttackS3LwS
            | Status::AttackS3Lw => M::AttackS3,
            Status::AttackHi3F | Status::AttackHi3 | Status::AttackHi3B => M::AttackHi3,
            Status::AttackLw3 => M::AttackLw3,
            Status::AttackS4Hi
            | Status::AttackS4HiS
            | Status::AttackS4
            | Status::AttackS4LwS
            | Status::AttackS4Lw => M::AttackS4,
            Status::AttackHi4 => M::AttackHi4,
            Status::AttackLw4 => M::AttackLw4,
            Status::AttackAirN | Status::LandingAirN => M::AttackAirN,
            Status::AttackAirF | Status::LandingAirF => M::AttackAirF,
            Status::AttackAirB | Status::LandingAirB => M::AttackAirB,
            Status::AttackAirHi | Status::LandingAirHi => M::AttackAirHi,
            Status::AttackAirLw | Status::LandingAirLw => M::AttackAirLw,
            _ => M::None,
        },
        AnyStatus::Mario(s) => match s {
            MarioStatus::Attack13 => M::Attack13,
            MarioStatus::SpecialN | MarioStatus::SpecialAirN => M::SpecialN,
            MarioStatus::SpecialHi | MarioStatus::SpecialAirHi => M::SpecialHi,
            MarioStatus::SpecialLw | MarioStatus::SpecialAirLw => M::SpecialLw,
        },
        AnyStatus::Fox(s) => match s {
            FoxStatus::Attack100Start | FoxStatus::Attack100Loop | FoxStatus::Attack100End => {
                M::Attack100
            }
            FoxStatus::SpecialN | FoxStatus::SpecialAirN => M::SpecialN,
            FoxStatus::SpecialHiStart
            | FoxStatus::SpecialAirHiStart
            | FoxStatus::SpecialHiHold
            | FoxStatus::SpecialAirHiHold
            | FoxStatus::SpecialHi
            | FoxStatus::SpecialAirHi
            | FoxStatus::SpecialHiEnd
            | FoxStatus::SpecialAirHiEnd
            | FoxStatus::SpecialAirHiBound => M::SpecialHi,
            FoxStatus::SpecialLwStart
            | FoxStatus::SpecialLwHit
            | FoxStatus::SpecialLwEnd
            | FoxStatus::SpecialLwLoop
            | FoxStatus::SpecialLwTurn
            | FoxStatus::SpecialAirLwStart
            | FoxStatus::SpecialAirLwHit
            | FoxStatus::SpecialAirLwEnd
            | FoxStatus::SpecialAirLwLoop
            | FoxStatus::SpecialAirLwTurn => M::SpecialLw,
        },
        AnyStatus::Donkey(s) => match s {
            DonkeyStatus::SpecialNStart
            | DonkeyStatus::SpecialAirNStart
            | DonkeyStatus::SpecialNLoop
            | DonkeyStatus::SpecialAirNLoop
            | DonkeyStatus::SpecialNEnd
            | DonkeyStatus::SpecialAirNEnd
            | DonkeyStatus::SpecialNFull
            | DonkeyStatus::SpecialAirNFull => M::SpecialN,
            DonkeyStatus::SpecialHi | DonkeyStatus::SpecialAirHi => M::SpecialHi,
            DonkeyStatus::SpecialLwStart
            | DonkeyStatus::SpecialLwLoop
            | DonkeyStatus::SpecialLwEnd => M::SpecialLw,
            DonkeyStatus::ThrowFWait
            | DonkeyStatus::ThrowFWalkSlow
            | DonkeyStatus::ThrowFWalkMiddle
            | DonkeyStatus::ThrowFWalkFast
            | DonkeyStatus::ThrowFTurn
            | DonkeyStatus::ThrowFKneeBend
            | DonkeyStatus::ThrowFFall
            | DonkeyStatus::ThrowFLanding
            | DonkeyStatus::ThrowFDamage
            | DonkeyStatus::ThrowFF
            | DonkeyStatus::ThrowAirFF => M::ThrowF,
        },
        AnyStatus::Samus(s) => match s {
            SamusStatus::SpecialNStart
            | SamusStatus::SpecialNLoop
            | SamusStatus::SpecialNEnd
            | SamusStatus::SpecialAirNStart
            | SamusStatus::SpecialAirNEnd => M::SpecialN,
            SamusStatus::SpecialHi | SamusStatus::SpecialAirHi => M::SpecialHi,
            SamusStatus::SpecialLw | SamusStatus::SpecialAirLw => M::SpecialLw,
        },
        AnyStatus::Link(s) => match s {
            LinkStatus::Attack13 => M::Attack13,
            LinkStatus::Attack100Start | LinkStatus::Attack100Loop | LinkStatus::Attack100End => {
                M::Attack100
            }
            LinkStatus::SpecialHi | LinkStatus::SpecialHiEnd | LinkStatus::SpecialAirHi => {
                M::SpecialHi
            }
            LinkStatus::SpecialN
            | LinkStatus::SpecialNGet
            | LinkStatus::SpecialNEmpty
            | LinkStatus::SpecialAirN
            | LinkStatus::SpecialAirNReturn
            | LinkStatus::SpecialAirNEmpty => M::SpecialN,
            LinkStatus::SpecialLw | LinkStatus::SpecialAirLw => M::SpecialLw,
        },
        AnyStatus::Yoshi(s) => match s {
            YoshiStatus::SpecialHi | YoshiStatus::SpecialAirHi => M::SpecialHi,
            YoshiStatus::SpecialLwStart
            | YoshiStatus::SpecialLwLanding
            | YoshiStatus::SpecialAirLwStart
            | YoshiStatus::SpecialAirLwLoop => M::SpecialLw,
            YoshiStatus::SpecialN
            | YoshiStatus::SpecialNCatch
            | YoshiStatus::SpecialNRelease
            | YoshiStatus::SpecialAirN
            | YoshiStatus::SpecialAirNCatch
            | YoshiStatus::SpecialAirNRelease => M::SpecialN,
        },
        AnyStatus::Captain(s) => match s {
            CaptainStatus::Attack13 => M::Attack13,
            CaptainStatus::Attack100Start
            | CaptainStatus::Attack100Loop
            | CaptainStatus::Attack100End => M::Attack100,
            CaptainStatus::SpecialN | CaptainStatus::SpecialAirN => M::SpecialN,
            CaptainStatus::SpecialLw
            | CaptainStatus::SpecialLwAir
            | CaptainStatus::SpecialLwLanding
            | CaptainStatus::SpecialAirLw
            | CaptainStatus::SpecialLwBound => M::SpecialLw,
            CaptainStatus::SpecialHi
            | CaptainStatus::SpecialHiCatch
            | CaptainStatus::SpecialHiThrow
            | CaptainStatus::SpecialAirHi => M::SpecialHi,
        },
    }
}

/// `ftMainSetStatus`'s motion-ID step (`ftmain.c`): a status with no attack
/// ID, or a different one, starts a new motion. A status that shares the
/// current ID (a landing lag after its aerial, a ground/air switch) keeps it.
///
/// `Attack11` and `AttackLw3` also call `ftParamSetMotionID` from their own
/// `proc_status`/`InitStatusVars`, so they always start a new motion.
pub fn on_set_status(f: &mut Fighter, status: AnyStatus) {
    let id = status_attack_id(f.kind, status);
    if id == MotionAttackId::None || id != f.motion.attack_id {
        f.motion.set(id);
    }
    if matches!(
        status,
        AnyStatus::Common(Status::Attack11 | Status::AttackLw3)
    ) {
        f.motion.set(id);
    }
}

/// The attacker's current motion, staled against its own queue.
pub fn staled_damage(attacker: &Fighter, damage: i32) -> i32 {
    attacker
        .stale
        .staled_damage(damage, attacker.motion.attack_id, attacker.motion.count)
}

/// `ftParamUpdateStaleQueue(attacker, defender, ...)` for a landed hit.
pub fn record_hit(attacker: &mut Fighter, defender_port: u8) {
    if attacker.port != defender_port {
        let (id, count) = (attacker.motion.attack_id, attacker.motion.count);
        attacker.stale.push(id, count);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_queue_is_fresh() {
        let q = StaleQueue::default();
        assert_eq!(q.stale(MotionAttackId::Attack11, 3), 1.0);
        assert_eq!(q.staled_damage(10, MotionAttackId::Attack11, 3), 10);
        assert_eq!(q.stale(MotionAttackId::None, 0), 1.0);
    }

    #[test]
    fn repeated_move_steps_through_the_stale_table() {
        let mut q = StaleQueue::default();
        q.push(MotionAttackId::AttackS4, 1);
        assert_eq!(q.stale(MotionAttackId::AttackS4, 2), 0.75);
        q.push(MotionAttackId::AttackHi4, 2);
        assert_eq!(q.stale(MotionAttackId::AttackS4, 3), 0.82);
        q.push(MotionAttackId::AttackHi4, 3);
        q.push(MotionAttackId::AttackHi4, 4);
        assert_eq!(q.stale(MotionAttackId::AttackS4, 5), 0.96);
        q.push(MotionAttackId::AttackHi4, 5);
        // Five newer entries: the smash has left the four-slot window.
        assert_eq!(q.stale(MotionAttackId::AttackS4, 6), 1.0);
    }

    #[test]
    fn the_same_motion_does_not_stale_itself() {
        let mut q = StaleQueue::default();
        q.push(MotionAttackId::AttackS4, 1);
        q.push(MotionAttackId::AttackAirN, 2);
        // The newest entry is this very motion: skipped, and the older
        // smash still reads at the first multiplier.
        assert_eq!(q.stale(MotionAttackId::AttackAirN, 2), 1.0);
        q.push(MotionAttackId::AttackS4, 3);
        q.push(MotionAttackId::AttackS4, 3);
        assert_eq!(q.next, 3, "a repeated (id, count) pair is not queued twice");
        // Its own newest entry is skipped; the aerial takes the first slot
        // of the walk, so the older smash reads at the second multiplier.
        assert_eq!(q.stale(MotionAttackId::AttackS4, 3), 0.82);
    }

    #[test]
    fn staled_damage_rounds_up() {
        let mut q = StaleQueue::default();
        q.push(MotionAttackId::AttackS4, 1);
        // 17 * 0.75 = 12.75 -> (12.75 + 0.999) as i32 = 13.
        assert_eq!(q.staled_damage(17, MotionAttackId::AttackS4, 2), 13);
    }

    #[test]
    fn motion_ids_follow_the_status_rule() {
        let mut f = Fighter::new(FighterKind::Mario, 0, 3);
        on_set_status(&mut f, AnyStatus::Common(Status::AttackAirN));
        let aerial = f.motion;
        on_set_status(&mut f, AnyStatus::Common(Status::LandingAirN));
        assert_eq!(f.motion, aerial, "landing lag keeps the aerial's motion");
        on_set_status(&mut f, AnyStatus::Common(Status::Attack11));
        let jab = f.motion.count;
        on_set_status(&mut f, AnyStatus::Common(Status::Attack11));
        assert_ne!(f.motion.count, jab, "every jab is a new motion");
        on_set_status(&mut f, AnyStatus::Common(Status::Wait));
        assert_eq!(f.motion.attack_id, MotionAttackId::None);
    }

    #[test]
    fn default_handicaps_are_neutral_to_float_precision() {
        let kb = apply_ratio_and_handicap(17.0, DAMAGE_RATIO_DEFAULT, 9, 9);
        assert!((kb - 17.0).abs() < 1e-4, "{kb}");
        assert_eq!(apply_ratio_and_handicap(17.0, 100, 8, 8), 17.0);
    }
}
