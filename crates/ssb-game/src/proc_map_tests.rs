//! Every status's `proc_map` (the last callback of each `FTStatusDesc` in
//! `ftcommonstatus.h` and `ft<char>status.h`) against the port's map-pass
//! classification: [`stops_at_edge`] (`MAP_PROC_TYPE_STOPEDGE`),
//! [`teeters_at_edge`] (`MAP_PROC_TYPE_CLIFFEDGE`), [`allows_cliff`]
//! (`MAP_PROC_TYPE_CLIFF`) and [`is_ceil_heavy_status`]
//! (`MAP_PROC_TYPE_CEILHEAVY`).
//!
//! The table is the decomp's reading: each `proc_map` classified by the
//! `mpCommon*` primitives it can reach (`mpcommon.c:242-711`), following
//! nested `ft*ProcMap` calls but not `*SetStatus` callbacks. The port's
//! lists are the second, hand-written reading (D-034). A class reached only
//! on a branch the status never takes (a ground branch of an air-only
//! status) is listed in [`UNREACHED`] with its source line.

use super::*;
use crate::fighter::FighterKind;
use crate::status::{BossStatus, DonkeyStatus, LinkStatus, MarioStatus, SamusStatus, YoshiStatus};

mod m {
    pub const NONE: u8 = 0;
    pub const STOPEDGE: u8 = 1;
    pub const CLIFFEDGE: u8 = 2;
    pub const CLIFF: u8 = 4;
    pub const CEILHEAVY: u8 = 8;
}
use m as M;

/// `(table, status, proc_map, classes)`. Luigi's table repeats Mario's
/// callbacks status for status and is not listed; Jigglypuff's Attack100
/// (220 to 222, `mpCommonSetFighterFallOnEdgeBreak`) has no port status
/// (`ftpurin.h`: no move enters it).
#[rustfmt::skip]
const ROWS: &[(&str, AnyStatus, &str, u8)] = &[
    ("common", AnyStatus::Common(Status::DeadDown), "NULL", M::NONE),
    ("common", AnyStatus::Common(Status::DeadLeftRight), "NULL", M::NONE),
    ("common", AnyStatus::Common(Status::DeadUpStar), "NULL", M::NONE),
    ("common", AnyStatus::Common(Status::DeadUpFall), "NULL", M::NONE),
    ("common", AnyStatus::Common(Status::Sleep), "NULL", M::NONE),
    ("common", AnyStatus::Common(Status::Entry), "NULL", M::NONE),
    ("common", AnyStatus::Common(Status::EntryNull), "NULL", M::NONE),
    ("common", AnyStatus::Common(Status::RebirthDown), "ftCommonRebirthCommonProcMap", M::NONE),
    ("common", AnyStatus::Common(Status::RebirthStand), "ftCommonRebirthCommonProcMap", M::NONE),
    ("common", AnyStatus::Common(Status::RebirthWait), "ftCommonRebirthCommonProcMap", M::NONE),
    ("common", AnyStatus::Common(Status::Wait), "mpCommonProcFighterOnCliffEdge", M::CLIFFEDGE),
    ("common", AnyStatus::Common(Status::WalkSlow), "mpCommonProcFighterOnCliffEdge", M::CLIFFEDGE),
    ("common", AnyStatus::Common(Status::WalkMiddle), "mpCommonProcFighterOnCliffEdge", M::CLIFFEDGE),
    ("common", AnyStatus::Common(Status::WalkFast), "mpCommonProcFighterOnCliffEdge", M::CLIFFEDGE),
    ("common", AnyStatus::Common(Status::WalkEnd), "NULL", M::NONE),
    ("common", AnyStatus::Common(Status::Dash), "ftCommonDashProcMap", M::CLIFFEDGE),
    ("common", AnyStatus::Common(Status::Run), "mpCommonSetFighterFallOnGroundBreak", M::NONE),
    ("common", AnyStatus::Common(Status::RunBrake), "mpCommonProcFighterOnCliffEdge", M::CLIFFEDGE),
    ("common", AnyStatus::Common(Status::Turn), "mpCommonSetFighterFallOnGroundBreak", M::NONE),
    ("common", AnyStatus::Common(Status::TurnRun), "mpCommonSetFighterFallOnGroundBreak", M::NONE),
    ("common", AnyStatus::Common(Status::KneeBend), "mpCommonSetFighterFallOnGroundBreak", M::NONE),
    ("common", AnyStatus::Common(Status::GuardKneeBend), "mpCommonSetFighterFallOnGroundBreak", M::NONE),
    ("common", AnyStatus::Common(Status::JumpF), "mpCommonProcFighterCliffFloorCeil", M::CLIFF | M::CEILHEAVY),
    ("common", AnyStatus::Common(Status::JumpB), "mpCommonProcFighterCliffFloorCeil", M::CLIFF | M::CEILHEAVY),
    ("common", AnyStatus::Common(Status::JumpAerialF), "mpCommonProcFighterCliffFloorCeil", M::CLIFF | M::CEILHEAVY),
    ("common", AnyStatus::Common(Status::JumpAerialB), "mpCommonProcFighterCliffFloorCeil", M::CLIFF | M::CEILHEAVY),
    ("common", AnyStatus::Common(Status::Fall), "mpCommonProcFighterCliffFloorCeil", M::CLIFF | M::CEILHEAVY),
    ("common", AnyStatus::Common(Status::FallAerial), "mpCommonProcFighterCliffFloorCeil", M::CLIFF | M::CEILHEAVY),
    ("common", AnyStatus::Common(Status::Squat), "mpCommonSetFighterFallOnGroundBreak", M::NONE),
    ("common", AnyStatus::Common(Status::SquatWait), "mpCommonSetFighterFallOnGroundBreak", M::NONE),
    ("common", AnyStatus::Common(Status::SquatRv), "mpCommonSetFighterFallOnGroundBreak", M::NONE),
    ("common", AnyStatus::Common(Status::LandingLight), "mpCommonProcFighterOnCliffEdge", M::CLIFFEDGE),
    ("common", AnyStatus::Common(Status::LandingHeavy), "mpCommonProcFighterOnCliffEdge", M::CLIFFEDGE),
    ("common", AnyStatus::Common(Status::Pass), "mpCommonProcFighterCliffFloorCeil", M::CLIFF | M::CEILHEAVY),
    ("common", AnyStatus::Common(Status::GuardPass), "mpCommonProcFighterCliffFloorCeil", M::CLIFF | M::CEILHEAVY),
    ("common", AnyStatus::Common(Status::OttottoWait), "ftCommonOttottoProcMap", M::NONE),
    ("common", AnyStatus::Common(Status::Ottotto), "ftCommonOttottoProcMap", M::NONE),
    ("common", AnyStatus::Common(Status::DamageHi1), "mpCommonUpdateFighterKinetics", M::NONE),
    ("common", AnyStatus::Common(Status::DamageHi2), "mpCommonUpdateFighterKinetics", M::NONE),
    ("common", AnyStatus::Common(Status::DamageHi3), "mpCommonUpdateFighterKinetics", M::NONE),
    ("common", AnyStatus::Common(Status::DamageN1), "mpCommonUpdateFighterKinetics", M::NONE),
    ("common", AnyStatus::Common(Status::DamageN2), "mpCommonUpdateFighterKinetics", M::NONE),
    ("common", AnyStatus::Common(Status::DamageN3), "mpCommonUpdateFighterKinetics", M::NONE),
    ("common", AnyStatus::Common(Status::DamageLw1), "mpCommonUpdateFighterKinetics", M::NONE),
    ("common", AnyStatus::Common(Status::DamageLw2), "mpCommonUpdateFighterKinetics", M::NONE),
    ("common", AnyStatus::Common(Status::DamageLw3), "mpCommonUpdateFighterKinetics", M::NONE),
    ("common", AnyStatus::Common(Status::DamageAir1), "mpCommonUpdateFighterKinetics", M::NONE),
    ("common", AnyStatus::Common(Status::DamageAir2), "mpCommonUpdateFighterKinetics", M::NONE),
    ("common", AnyStatus::Common(Status::DamageAir3), "mpCommonUpdateFighterKinetics", M::NONE),
    ("common", AnyStatus::Common(Status::DamageE1), "mpCommonUpdateFighterKinetics", M::NONE),
    ("common", AnyStatus::Common(Status::DamageE2), "ftCommonDamageAirCommonProcMap", M::NONE),
    ("common", AnyStatus::Common(Status::DamageFlyHi), "ftCommonDamageAirCommonProcMap", M::NONE),
    ("common", AnyStatus::Common(Status::DamageFlyN), "ftCommonDamageAirCommonProcMap", M::NONE),
    ("common", AnyStatus::Common(Status::DamageFlyLw), "ftCommonDamageAirCommonProcMap", M::NONE),
    ("common", AnyStatus::Common(Status::DamageFlyTop), "ftCommonDamageAirCommonProcMap", M::NONE),
    ("common", AnyStatus::Common(Status::DamageFlyRoll), "ftCommonDamageAirCommonProcMap", M::NONE),
    ("common", AnyStatus::Common(Status::WallDamage), "ftCommonDamageAirCommonProcMap", M::NONE),
    ("common", AnyStatus::Common(Status::DamageFall), "ftCommonDamageFallProcMap", M::CLIFF),
    ("common", AnyStatus::Common(Status::FallSpecial), "ftCommonFallSpecialProcMap", M::CLIFF),
    ("common", AnyStatus::Common(Status::LandingFallSpecial), "mpCommonProcFighterOnCliffEdge", M::CLIFFEDGE),
    ("common", AnyStatus::Common(Status::Twister), "mpCommonProcFighterProject", M::NONE),
    ("common", AnyStatus::Common(Status::TaruCann), "mpCommonProcFighterProject", M::NONE),
    ("common", AnyStatus::Common(Status::DokanStart), "mpCommonSetFighterFallOnEdgeBreak", M::STOPEDGE),
    ("common", AnyStatus::Common(Status::DokanWait), "ftCommonDokanWaitProcMap", M::NONE),
    ("common", AnyStatus::Common(Status::DokanEnd), "mpCommonSetFighterFallOnEdgeBreak", M::STOPEDGE),
    ("common", AnyStatus::Common(Status::DokanWalk), "mpCommonUpdateFighterProjectFloor", M::NONE),
    ("common", AnyStatus::Common(Status::StopCeil), "mpCommonProcFighterCliffFloorCeil", M::CLIFF | M::CEILHEAVY),
    ("common", AnyStatus::Common(Status::DownBounceD), "mpCommonSetFighterFallOnGroundBreak", M::NONE),
    ("common", AnyStatus::Common(Status::DownBounceU), "mpCommonSetFighterFallOnGroundBreak", M::NONE),
    ("common", AnyStatus::Common(Status::DownWaitD), "mpCommonSetFighterFallOnGroundBreak", M::NONE),
    ("common", AnyStatus::Common(Status::DownWaitU), "mpCommonSetFighterFallOnGroundBreak", M::NONE),
    ("common", AnyStatus::Common(Status::DownStandD), "mpCommonSetFighterFallOnGroundBreak", M::NONE),
    ("common", AnyStatus::Common(Status::DownStandU), "mpCommonSetFighterFallOnGroundBreak", M::NONE),
    ("common", AnyStatus::Common(Status::PassiveStandF), "mpCommonSetFighterFallOnEdgeBreak", M::STOPEDGE),
    ("common", AnyStatus::Common(Status::PassiveStandB), "mpCommonSetFighterFallOnEdgeBreak", M::STOPEDGE),
    ("common", AnyStatus::Common(Status::DownForwardD), "mpCommonSetFighterFallOnEdgeBreak", M::STOPEDGE),
    ("common", AnyStatus::Common(Status::DownForwardU), "mpCommonSetFighterFallOnEdgeBreak", M::STOPEDGE),
    ("common", AnyStatus::Common(Status::DownBackD), "mpCommonSetFighterFallOnEdgeBreak", M::STOPEDGE),
    ("common", AnyStatus::Common(Status::DownBackU), "mpCommonSetFighterFallOnEdgeBreak", M::STOPEDGE),
    ("common", AnyStatus::Common(Status::DownAttackD), "mpCommonSetFighterFallOnEdgeBreak", M::STOPEDGE),
    ("common", AnyStatus::Common(Status::DownAttackU), "mpCommonSetFighterFallOnEdgeBreak", M::STOPEDGE),
    ("common", AnyStatus::Common(Status::Passive), "mpCommonSetFighterFallOnGroundBreak", M::NONE),
    ("common", AnyStatus::Common(Status::ReboundWait), "mpCommonSetFighterFallOnGroundBreak", M::NONE),
    ("common", AnyStatus::Common(Status::Rebound), "mpCommonSetFighterFallOnGroundBreak", M::NONE),
    ("common", AnyStatus::Common(Status::CliffCatch), "ftCommonCliffCommonProcMap", M::NONE),
    ("common", AnyStatus::Common(Status::CliffWait), "ftCommonCliffCommonProcMap", M::NONE),
    ("common", AnyStatus::Common(Status::CliffQuick), "ftCommonCliffCommonProcMap", M::NONE),
    ("common", AnyStatus::Common(Status::CliffClimbQuick1), "ftCommonCliffCommonProcMap", M::NONE),
    ("common", AnyStatus::Common(Status::CliffClimbQuick2), "ftCommonCliffClimbCommon2ProcMap", M::NONE),
    ("common", AnyStatus::Common(Status::CliffSlow), "ftCommonCliffCommonProcMap", M::NONE),
    ("common", AnyStatus::Common(Status::CliffClimbSlow1), "ftCommonCliffCommonProcMap", M::NONE),
    ("common", AnyStatus::Common(Status::CliffClimbSlow2), "ftCommonCliffClimbCommon2ProcMap", M::NONE),
    ("common", AnyStatus::Common(Status::CliffAttackQuick1), "ftCommonCliffCommonProcMap", M::NONE),
    ("common", AnyStatus::Common(Status::CliffAttackQuick2), "ftCommonCliffAttackEscape2ProcMap", M::STOPEDGE),
    ("common", AnyStatus::Common(Status::CliffAttackSlow1), "ftCommonCliffCommonProcMap", M::NONE),
    ("common", AnyStatus::Common(Status::CliffAttackSlow2), "ftCommonCliffAttackEscape2ProcMap", M::STOPEDGE),
    ("common", AnyStatus::Common(Status::CliffEscapeQuick1), "ftCommonCliffCommonProcMap", M::NONE),
    ("common", AnyStatus::Common(Status::CliffEscapeQuick2), "ftCommonCliffAttackEscape2ProcMap", M::STOPEDGE),
    ("common", AnyStatus::Common(Status::CliffEscapeSlow1), "ftCommonCliffCommonProcMap", M::NONE),
    ("common", AnyStatus::Common(Status::CliffEscapeSlow2), "ftCommonCliffAttackEscape2ProcMap", M::STOPEDGE),
    ("common", AnyStatus::Common(Status::LightGet), "ftCommonLightGetProcMap", M::NONE),
    ("common", AnyStatus::Common(Status::HeavyGet), "ftCommonHeavyGetProcMap", M::NONE),
    ("common", AnyStatus::Common(Status::LiftWait), "ftCommonHeavyGetProcMap", M::NONE),
    ("common", AnyStatus::Common(Status::LiftTurn), "ftCommonHeavyGetProcMap", M::NONE),
    ("common", AnyStatus::Common(Status::LightThrowDrop), "mpCommonSetFighterFallOnEdgeBreak", M::STOPEDGE),
    ("common", AnyStatus::Common(Status::LightThrowDash), "mpCommonSetFighterFallOnEdgeBreak", M::STOPEDGE),
    ("common", AnyStatus::Common(Status::LightThrowF), "mpCommonSetFighterFallOnEdgeBreak", M::STOPEDGE),
    ("common", AnyStatus::Common(Status::LightThrowB), "mpCommonSetFighterFallOnEdgeBreak", M::STOPEDGE),
    ("common", AnyStatus::Common(Status::LightThrowHi), "mpCommonSetFighterFallOnEdgeBreak", M::STOPEDGE),
    ("common", AnyStatus::Common(Status::LightThrowLw), "mpCommonSetFighterFallOnEdgeBreak", M::STOPEDGE),
    ("common", AnyStatus::Common(Status::LightThrowF4), "mpCommonSetFighterFallOnEdgeBreak", M::STOPEDGE),
    ("common", AnyStatus::Common(Status::LightThrowB4), "mpCommonSetFighterFallOnEdgeBreak", M::STOPEDGE),
    ("common", AnyStatus::Common(Status::LightThrowHi4), "mpCommonSetFighterFallOnEdgeBreak", M::STOPEDGE),
    ("common", AnyStatus::Common(Status::LightThrowLw4), "mpCommonSetFighterFallOnEdgeBreak", M::STOPEDGE),
    ("common", AnyStatus::Common(Status::LightThrowAirF), "mpCommonProcFighterWaitOrLanding", M::NONE),
    ("common", AnyStatus::Common(Status::LightThrowAirB), "mpCommonProcFighterWaitOrLanding", M::NONE),
    ("common", AnyStatus::Common(Status::LightThrowAirHi), "mpCommonProcFighterWaitOrLanding", M::NONE),
    ("common", AnyStatus::Common(Status::LightThrowAirLw), "mpCommonProcFighterWaitOrLanding", M::NONE),
    ("common", AnyStatus::Common(Status::LightThrowAirF4), "mpCommonProcFighterWaitOrLanding", M::NONE),
    ("common", AnyStatus::Common(Status::LightThrowAirB4), "mpCommonProcFighterWaitOrLanding", M::NONE),
    ("common", AnyStatus::Common(Status::LightThrowAirHi4), "mpCommonProcFighterWaitOrLanding", M::NONE),
    ("common", AnyStatus::Common(Status::LightThrowAirLw4), "mpCommonProcFighterWaitOrLanding", M::NONE),
    ("common", AnyStatus::Common(Status::HeavyThrowF), "ftCommonHeavyThrowProcMap", M::STOPEDGE),
    ("common", AnyStatus::Common(Status::HeavyThrowB), "ftCommonHeavyThrowProcMap", M::STOPEDGE),
    ("common", AnyStatus::Common(Status::HeavyThrowF4), "ftCommonHeavyThrowProcMap", M::STOPEDGE),
    ("common", AnyStatus::Common(Status::HeavyThrowB4), "ftCommonHeavyThrowProcMap", M::STOPEDGE),
    ("common", AnyStatus::Common(Status::SwordSwing1), "mpCommonSetFighterFallOnEdgeBreak", M::STOPEDGE),
    ("common", AnyStatus::Common(Status::SwordSwing3), "mpCommonSetFighterFallOnEdgeBreak", M::STOPEDGE),
    ("common", AnyStatus::Common(Status::SwordSwing4), "mpCommonSetFighterFallOnEdgeBreak", M::STOPEDGE),
    ("common", AnyStatus::Common(Status::SwordSwingDash), "mpCommonSetFighterFallOnEdgeBreak", M::STOPEDGE),
    ("common", AnyStatus::Common(Status::BatSwing1), "mpCommonSetFighterFallOnEdgeBreak", M::STOPEDGE),
    ("common", AnyStatus::Common(Status::BatSwing3), "mpCommonSetFighterFallOnEdgeBreak", M::STOPEDGE),
    ("common", AnyStatus::Common(Status::BatSwing4), "mpCommonSetFighterFallOnEdgeBreak", M::STOPEDGE),
    ("common", AnyStatus::Common(Status::BatSwingDash), "mpCommonSetFighterFallOnEdgeBreak", M::STOPEDGE),
    ("common", AnyStatus::Common(Status::HarisenSwing1), "mpCommonSetFighterFallOnEdgeBreak", M::STOPEDGE),
    ("common", AnyStatus::Common(Status::HarisenSwing3), "mpCommonSetFighterFallOnEdgeBreak", M::STOPEDGE),
    ("common", AnyStatus::Common(Status::HarisenSwing4), "mpCommonSetFighterFallOnEdgeBreak", M::STOPEDGE),
    ("common", AnyStatus::Common(Status::HarisenSwingDash), "mpCommonSetFighterFallOnEdgeBreak", M::STOPEDGE),
    ("common", AnyStatus::Common(Status::StarRodSwing1), "mpCommonSetFighterFallOnEdgeBreak", M::STOPEDGE),
    ("common", AnyStatus::Common(Status::StarRodSwing3), "mpCommonSetFighterFallOnEdgeBreak", M::STOPEDGE),
    ("common", AnyStatus::Common(Status::StarRodSwing4), "mpCommonSetFighterFallOnEdgeBreak", M::STOPEDGE),
    ("common", AnyStatus::Common(Status::StarRodSwingDash), "mpCommonSetFighterFallOnEdgeBreak", M::STOPEDGE),
    ("common", AnyStatus::Common(Status::LGunShoot), "ftCommonLGunShootProcMap", M::STOPEDGE),
    ("common", AnyStatus::Common(Status::LGunShootAir), "ftCommonLGunShootAirProcMap", M::NONE),
    ("common", AnyStatus::Common(Status::FireFlowerShoot), "ftCommonFireFlowerShootProcMap", M::STOPEDGE),
    ("common", AnyStatus::Common(Status::FireFlowerShootAir), "ftCommonFireFlowerShootAirProcMap", M::NONE),
    ("common", AnyStatus::Common(Status::HammerWait), "ftHammerProcMap", M::NONE),
    ("common", AnyStatus::Common(Status::HammerWalk), "ftHammerProcMap", M::NONE),
    ("common", AnyStatus::Common(Status::HammerTurn), "ftHammerProcMap", M::NONE),
    ("common", AnyStatus::Common(Status::HammerKneeBend), "ftHammerProcMap", M::NONE),
    ("common", AnyStatus::Common(Status::HammerFall), "ftCommonHammerFallProcMap", M::NONE),
    ("common", AnyStatus::Common(Status::HammerLanding), "ftHammerProcMap", M::NONE),
    ("common", AnyStatus::Common(Status::GuardOn), "mpCommonSetFighterFallOnGroundBreak", M::NONE),
    ("common", AnyStatus::Common(Status::Guard), "mpCommonSetFighterFallOnGroundBreak", M::NONE),
    ("common", AnyStatus::Common(Status::GuardOff), "mpCommonSetFighterFallOnGroundBreak", M::NONE),
    ("common", AnyStatus::Common(Status::GuardSetOff), "mpCommonSetFighterFallOnGroundBreak", M::NONE),
    ("common", AnyStatus::Common(Status::EscapeF), "mpCommonSetFighterFallOnEdgeBreak", M::STOPEDGE),
    ("common", AnyStatus::Common(Status::EscapeB), "mpCommonSetFighterFallOnEdgeBreak", M::STOPEDGE),
    ("common", AnyStatus::Common(Status::ShieldBreakFly), "ftCommonShieldBreakFlyProcMap", M::NONE),
    ("common", AnyStatus::Common(Status::ShieldBreakFall), "ftCommonShieldBreakFallProcMap", M::NONE),
    ("common", AnyStatus::Common(Status::ShieldBreakDownD), "mpCommonSetFighterFallOnGroundBreak", M::NONE),
    ("common", AnyStatus::Common(Status::ShieldBreakDownU), "mpCommonSetFighterFallOnGroundBreak", M::NONE),
    ("common", AnyStatus::Common(Status::ShieldBreakStandD), "mpCommonSetFighterFallOnGroundBreak", M::NONE),
    ("common", AnyStatus::Common(Status::ShieldBreakStandU), "mpCommonSetFighterFallOnGroundBreak", M::NONE),
    ("common", AnyStatus::Common(Status::FuraFura), "mpCommonSetFighterFallOnGroundBreak", M::NONE),
    ("common", AnyStatus::Common(Status::FuraSleep), "mpCommonSetFighterFallOnGroundBreak", M::NONE),
    ("common", AnyStatus::Common(Status::Catch), "ftCommonCatchProcMap", M::STOPEDGE),
    ("common", AnyStatus::Common(Status::CatchPull), "ftCommonCatchProcMap", M::STOPEDGE),
    ("common", AnyStatus::Common(Status::CatchWait), "ftCommonCatchProcMap", M::STOPEDGE),
    ("common", AnyStatus::Common(Status::ThrowF), "ftCommonCatchProcMap", M::STOPEDGE),
    ("common", AnyStatus::Common(Status::ThrowB), "ftCommonCatchProcMap", M::STOPEDGE),
    ("common", AnyStatus::Common(Status::CapturePulled), "ftCommonCapturePulledProcMap", M::NONE),
    ("common", AnyStatus::Common(Status::CaptureWait), "ftCommonCaptureWaitProcMap", M::NONE),
    ("common", AnyStatus::Common(Status::CaptureKirby), "mpCommonUpdateFighterProjectFloor", M::NONE),
    ("common", AnyStatus::Common(Status::CaptureWaitKirby), "ftCommonCaptureWaitKirbyProcMap", M::NONE),
    ("common", AnyStatus::Common(Status::ThrownKirbyStar), "ftCommonThrownCommonStarProcMap", M::NONE),
    ("common", AnyStatus::Common(Status::ThrownCopyStar), "ftCommonThrownCommonStarProcMap", M::NONE),
    ("common", AnyStatus::Common(Status::CaptureYoshi), "mpCommonUpdateFighterProjectFloor", M::NONE),
    ("common", AnyStatus::Common(Status::YoshiEgg), "ftCommonYoshiEggProcMap", M::NONE),
    ("common", AnyStatus::Common(Status::CaptureCaptain), "mpCommonUpdateFighterProjectFloor", M::NONE),
    ("common", AnyStatus::Common(Status::ThrownDonkeyUnk), "NULL", M::NONE),
    ("common", AnyStatus::Common(Status::ThrownDonkeyF), "ftCommonThrownProcMap", M::NONE),
    ("common", AnyStatus::Common(Status::ThrownMarioBStart), "ftCommonThrownProcMap", M::NONE),
    ("common", AnyStatus::Common(Status::ThrownFoxFStart), "ftCommonThrownProcMap", M::NONE),
    ("common", AnyStatus::Common(Status::Shouldered), "ftCommonThrownProcMap", M::NONE),
    ("common", AnyStatus::Common(Status::ThrownMarioB), "ftCommonThrownProcMap", M::NONE),
    ("common", AnyStatus::Common(Status::ThrownCommon), "ftCommonThrownProcMap", M::NONE),
    ("common", AnyStatus::Common(Status::ThrownFoxF), "ftCommonThrownProcMap", M::NONE),
    ("common", AnyStatus::Common(Status::ThrownFoxB), "ftCommonThrownProcMap", M::NONE),
    ("common", AnyStatus::Common(Status::Appeal), "mpCommonProcFighterOnCliffEdge", M::CLIFFEDGE),
    ("common", AnyStatus::Common(Status::Attack11), "mpCommonSetFighterFallOnEdgeBreak", M::STOPEDGE),
    ("common", AnyStatus::Common(Status::Attack12), "mpCommonSetFighterFallOnEdgeBreak", M::STOPEDGE),
    ("common", AnyStatus::Common(Status::AttackDash), "mpCommonSetFighterFallOnEdgeBreak", M::STOPEDGE),
    ("common", AnyStatus::Common(Status::AttackS3Hi), "mpCommonSetFighterFallOnEdgeBreak", M::STOPEDGE),
    ("common", AnyStatus::Common(Status::AttackS3HiS), "mpCommonSetFighterFallOnEdgeBreak", M::STOPEDGE),
    ("common", AnyStatus::Common(Status::AttackS3), "mpCommonSetFighterFallOnEdgeBreak", M::STOPEDGE),
    ("common", AnyStatus::Common(Status::AttackS3LwS), "mpCommonSetFighterFallOnEdgeBreak", M::STOPEDGE),
    ("common", AnyStatus::Common(Status::AttackS3Lw), "mpCommonSetFighterFallOnEdgeBreak", M::STOPEDGE),
    ("common", AnyStatus::Common(Status::AttackHi3F), "mpCommonSetFighterFallOnEdgeBreak", M::STOPEDGE),
    ("common", AnyStatus::Common(Status::AttackHi3), "mpCommonSetFighterFallOnEdgeBreak", M::STOPEDGE),
    ("common", AnyStatus::Common(Status::AttackHi3B), "mpCommonSetFighterFallOnEdgeBreak", M::STOPEDGE),
    ("common", AnyStatus::Common(Status::AttackLw3), "mpCommonSetFighterFallOnEdgeBreak", M::STOPEDGE),
    ("common", AnyStatus::Common(Status::AttackS4Hi), "mpCommonSetFighterFallOnEdgeBreak", M::STOPEDGE),
    ("common", AnyStatus::Common(Status::AttackS4HiS), "mpCommonSetFighterFallOnEdgeBreak", M::STOPEDGE),
    ("common", AnyStatus::Common(Status::AttackS4), "mpCommonSetFighterFallOnEdgeBreak", M::STOPEDGE),
    ("common", AnyStatus::Common(Status::AttackS4LwS), "mpCommonSetFighterFallOnEdgeBreak", M::STOPEDGE),
    ("common", AnyStatus::Common(Status::AttackS4Lw), "mpCommonSetFighterFallOnEdgeBreak", M::STOPEDGE),
    ("common", AnyStatus::Common(Status::AttackHi4), "mpCommonSetFighterFallOnEdgeBreak", M::STOPEDGE),
    ("common", AnyStatus::Common(Status::AttackLw4), "mpCommonSetFighterFallOnEdgeBreak", M::STOPEDGE),
    ("common", AnyStatus::Common(Status::AttackAirN), "ftCommonAttackAirProcMap", M::NONE),
    ("common", AnyStatus::Common(Status::AttackAirF), "ftCommonAttackAirProcMap", M::NONE),
    ("common", AnyStatus::Common(Status::AttackAirB), "ftCommonAttackAirProcMap", M::NONE),
    ("common", AnyStatus::Common(Status::AttackAirHi), "ftCommonAttackAirProcMap", M::NONE),
    ("common", AnyStatus::Common(Status::AttackAirLw), "ftCommonAttackAirProcMap", M::NONE),
    ("common", AnyStatus::Common(Status::LandingAirN), "mpCommonProcFighterOnCliffEdge", M::CLIFFEDGE),
    ("common", AnyStatus::Common(Status::LandingAirF), "mpCommonProcFighterOnCliffEdge", M::CLIFFEDGE),
    ("common", AnyStatus::Common(Status::LandingAirB), "mpCommonProcFighterOnCliffEdge", M::CLIFFEDGE),
    ("common", AnyStatus::Common(Status::LandingAirHi), "mpCommonProcFighterOnCliffEdge", M::CLIFFEDGE),
    ("common", AnyStatus::Common(Status::LandingAirLw), "mpCommonProcFighterOnCliffEdge", M::CLIFFEDGE),
    ("common", AnyStatus::Common(Status::LandingAirNull), "mpCommonProcFighterOnCliffEdge", M::CLIFFEDGE),
    ("boss", AnyStatus::Boss(BossStatus::Default), "mpCommonUpdateFighterProjectFloor", M::NONE),
    ("boss", AnyStatus::Boss(BossStatus::Wait), "mpCommonUpdateFighterProjectFloor", M::NONE),
    ("boss", AnyStatus::Boss(BossStatus::Move), "ftBossMoveProcMap", M::NONE),
    ("boss", AnyStatus::Boss(BossStatus::Hippataku), "mpCommonUpdateFighterProjectFloor", M::NONE),
    ("boss", AnyStatus::Boss(BossStatus::Harau), "mpCommonUpdateFighterProjectFloor", M::NONE),
    ("boss", AnyStatus::Boss(BossStatus::Okuhikouki1), "mpCommonUpdateFighterProjectFloor", M::NONE),
    ("boss", AnyStatus::Boss(BossStatus::Okuhikouki2), "mpCommonUpdateFighterProjectFloor", M::NONE),
    ("boss", AnyStatus::Boss(BossStatus::Okuhikouki3), "mpCommonUpdateFighterProjectFloor", M::NONE),
    ("boss", AnyStatus::Boss(BossStatus::Walk), "NULL", M::NONE),
    ("boss", AnyStatus::Boss(BossStatus::WalkLoop), "ftBossWalkLoopProcMap", M::NONE),
    ("boss", AnyStatus::Boss(BossStatus::WalkWait), "mpCommonUpdateFighterProjectFloor", M::NONE),
    ("boss", AnyStatus::Boss(BossStatus::WalkShoot), "mpCommonUpdateFighterProjectFloor", M::NONE),
    ("boss", AnyStatus::Boss(BossStatus::GootsubusuUp), "ftBossGootsubusuUpProcMap", M::NONE),
    ("boss", AnyStatus::Boss(BossStatus::GootsubusuWait), "ftBossGootsubusuWaitProcMap", M::NONE),
    ("boss", AnyStatus::Boss(BossStatus::GootsubusuEnd), "mpCommonUpdateFighterProjectFloor", M::NONE),
    ("boss", AnyStatus::Boss(BossStatus::GootsubusuDown), "ftBossGootsubusuDownProcMap", M::NONE),
    ("boss", AnyStatus::Boss(BossStatus::Tsutsuku1), "mpCommonUpdateFighterProjectFloor", M::NONE),
    ("boss", AnyStatus::Boss(BossStatus::Tsutsuku3), "mpCommonUpdateFighterProjectFloor", M::NONE),
    ("boss", AnyStatus::Boss(BossStatus::Tsutsuku2), "mpCommonUpdateFighterProjectFloor", M::NONE),
    ("boss", AnyStatus::Boss(BossStatus::Drill), "ftBossDrillProcMap", M::NONE),
    ("boss", AnyStatus::Boss(BossStatus::Okukouki), "mpCommonUpdateFighterProjectFloor", M::NONE),
    ("boss", AnyStatus::Boss(BossStatus::Yubideppou1), "mpCommonUpdateFighterProjectFloor", M::NONE),
    ("boss", AnyStatus::Boss(BossStatus::Yubideppou3), "mpCommonUpdateFighterProjectFloor", M::NONE),
    ("boss", AnyStatus::Boss(BossStatus::Yubideppou2), "mpCommonUpdateFighterProjectFloor", M::NONE),
    ("boss", AnyStatus::Boss(BossStatus::Okupunch1), "mpCommonUpdateFighterProjectFloor", M::NONE),
    ("boss", AnyStatus::Boss(BossStatus::Okupunch2), "mpCommonUpdateFighterProjectFloor", M::NONE),
    ("boss", AnyStatus::Boss(BossStatus::Okupunch3), "mpCommonUpdateFighterProjectFloor", M::NONE),
    ("boss", AnyStatus::Boss(BossStatus::Okutsubushi), "mpCommonUpdateFighterProjectFloor", M::NONE),
    ("boss", AnyStatus::Boss(BossStatus::OkutsubushiStart), "mpCommonUpdateFighterProjectFloor", M::NONE),
    ("boss", AnyStatus::Boss(BossStatus::DeadLeft), "NULL", M::NONE),
    ("boss", AnyStatus::Boss(BossStatus::DeadCenter), "mpCommonUpdateFighterProjectFloor", M::NONE),
    ("boss", AnyStatus::Boss(BossStatus::DeadRight), "NULL", M::NONE),
    ("boss", AnyStatus::Boss(BossStatus::Appear), "mpCommonUpdateFighterProjectFloor", M::NONE),
    ("captain", AnyStatus::Captain(CaptainStatus::Attack13), "mpCommonSetFighterFallOnEdgeBreak", M::STOPEDGE),
    ("captain", AnyStatus::Captain(CaptainStatus::Attack100Start), "mpCommonSetFighterFallOnEdgeBreak", M::STOPEDGE),
    ("captain", AnyStatus::Captain(CaptainStatus::Attack100Loop), "mpCommonSetFighterFallOnEdgeBreak", M::STOPEDGE),
    ("captain", AnyStatus::Captain(CaptainStatus::Attack100End), "mpCommonSetFighterFallOnEdgeBreak", M::STOPEDGE),
    ("captain", AnyStatus::Captain(CaptainStatus::AppearRStart), "mpCommonUpdateFighterProjectFloor", M::NONE),
    ("captain", AnyStatus::Captain(CaptainStatus::AppearLStart), "mpCommonUpdateFighterProjectFloor", M::NONE),
    ("captain", AnyStatus::Captain(CaptainStatus::AppearREnd), "mpCommonUpdateFighterProjectFloor", M::NONE),
    ("captain", AnyStatus::Captain(CaptainStatus::AppearLEnd), "mpCommonUpdateFighterProjectFloor", M::NONE),
    ("captain", AnyStatus::Captain(CaptainStatus::SpecialN), "ftCaptainSpecialNProcMap", M::STOPEDGE),
    ("captain", AnyStatus::Captain(CaptainStatus::SpecialAirN), "ftCaptainSpecialAirNProcMap", M::NONE),
    ("captain", AnyStatus::Captain(CaptainStatus::SpecialLw), "ftCaptainSpecialLwProcMap", M::STOPEDGE),
    ("captain", AnyStatus::Captain(CaptainStatus::SpecialLwAir), "ftCaptainSpecialLwAirProcMap", M::STOPEDGE),
    ("captain", AnyStatus::Captain(CaptainStatus::SpecialLwLanding), "mpCommonSetFighterFallOnEdgeBreak", M::STOPEDGE),
    ("captain", AnyStatus::Captain(CaptainStatus::SpecialAirLw), "ftCaptainSpecialAirLwProcMap", M::NONE),
    ("captain", AnyStatus::Captain(CaptainStatus::SpecialLwBound), "mpCommonProcFighterWaitOrLanding", M::NONE),
    ("captain", AnyStatus::Captain(CaptainStatus::SpecialHi), "ftCaptainSpecialHiProcMap", M::CLIFF | M::CEILHEAVY),
    ("captain", AnyStatus::Captain(CaptainStatus::SpecialHiCatch), "mpCommonUpdateFighterProjectFloor", M::NONE),
    ("captain", AnyStatus::Captain(CaptainStatus::SpecialHiThrow), "mpCommonProcFighterWaitOrLanding", M::NONE),
    ("captain", AnyStatus::Captain(CaptainStatus::SpecialAirHi), "ftCaptainSpecialHiProcMap", M::CLIFF | M::CEILHEAVY),
    ("donkey", AnyStatus::Donkey(DonkeyStatus::AppearR), "mpCommonUpdateFighterProjectFloor", M::NONE),
    ("donkey", AnyStatus::Donkey(DonkeyStatus::AppearL), "mpCommonUpdateFighterProjectFloor", M::NONE),
    ("donkey", AnyStatus::Donkey(DonkeyStatus::SpecialNStart), "ftDonkeySpecialNStartProcMap", M::STOPEDGE),
    ("donkey", AnyStatus::Donkey(DonkeyStatus::SpecialAirNStart), "ftDonkeySpecialAirNStartProcMap", M::NONE),
    ("donkey", AnyStatus::Donkey(DonkeyStatus::SpecialNLoop), "ftDonkeySpecialNLoopProcMap", M::STOPEDGE),
    ("donkey", AnyStatus::Donkey(DonkeyStatus::SpecialAirNLoop), "ftDonkeySpecialAirNLoopProcMap", M::NONE),
    ("donkey", AnyStatus::Donkey(DonkeyStatus::SpecialNEnd), "mpCommonSetFighterFallOnEdgeBreak", M::STOPEDGE),
    ("donkey", AnyStatus::Donkey(DonkeyStatus::SpecialAirNEnd), "ftDonkeySpecialAirNEndProcMap", M::NONE),
    ("donkey", AnyStatus::Donkey(DonkeyStatus::SpecialNFull), "mpCommonSetFighterFallOnEdgeBreak", M::STOPEDGE),
    ("donkey", AnyStatus::Donkey(DonkeyStatus::SpecialAirNFull), "ftDonkeySpecialAirNEndProcMap", M::NONE),
    ("donkey", AnyStatus::Donkey(DonkeyStatus::SpecialHi), "ftDonkeySpecialHiProcMap", M::NONE),
    ("donkey", AnyStatus::Donkey(DonkeyStatus::SpecialAirHi), "ftDonkeySpecialAirHiProcMap", M::CLIFF),
    ("donkey", AnyStatus::Donkey(DonkeyStatus::SpecialLwStart), "mpCommonSetFighterFallOnEdgeBreak", M::STOPEDGE),
    ("donkey", AnyStatus::Donkey(DonkeyStatus::SpecialLwLoop), "mpCommonSetFighterFallOnEdgeBreak", M::STOPEDGE),
    ("donkey", AnyStatus::Donkey(DonkeyStatus::SpecialLwEnd), "mpCommonSetFighterFallOnEdgeBreak", M::STOPEDGE),
    ("donkey", AnyStatus::Donkey(DonkeyStatus::ThrowFWait), "ftDonkeyThrowFCommonProcMap", M::NONE),
    ("donkey", AnyStatus::Donkey(DonkeyStatus::ThrowFWalkSlow), "ftDonkeyThrowFCommonProcMap", M::NONE),
    ("donkey", AnyStatus::Donkey(DonkeyStatus::ThrowFWalkMiddle), "ftDonkeyThrowFCommonProcMap", M::NONE),
    ("donkey", AnyStatus::Donkey(DonkeyStatus::ThrowFWalkFast), "ftDonkeyThrowFCommonProcMap", M::NONE),
    ("donkey", AnyStatus::Donkey(DonkeyStatus::ThrowFTurn), "ftDonkeyThrowFCommonProcMap", M::NONE),
    ("donkey", AnyStatus::Donkey(DonkeyStatus::ThrowFKneeBend), "ftDonkeyThrowFCommonProcMap", M::NONE),
    ("donkey", AnyStatus::Donkey(DonkeyStatus::ThrowFFall), "ftDonkeyThrowFFallProcMap", M::NONE),
    ("donkey", AnyStatus::Donkey(DonkeyStatus::ThrowFLanding), "ftDonkeyThrowFCommonProcMap", M::NONE),
    ("donkey", AnyStatus::Donkey(DonkeyStatus::ThrowFDamage), "mpCommonUpdateFighterKinetics", M::NONE),
    ("donkey", AnyStatus::Donkey(DonkeyStatus::ThrowFF), "ftDonkeyThrowFFProcMap", M::NONE),
    ("donkey", AnyStatus::Donkey(DonkeyStatus::ThrowAirFF), "ftDonkeyThrowAirFFProcMap", M::NONE),
    ("donkey", AnyStatus::Donkey(DonkeyStatus::HeavyThrowF), "mpCommonUpdateFighterKinetics", M::NONE),
    ("donkey", AnyStatus::Donkey(DonkeyStatus::HeavyThrowB), "mpCommonUpdateFighterKinetics", M::NONE),
    ("donkey", AnyStatus::Donkey(DonkeyStatus::HeavyThrowF4), "mpCommonUpdateFighterKinetics", M::NONE),
    ("donkey", AnyStatus::Donkey(DonkeyStatus::HeavyThrowB4), "mpCommonUpdateFighterKinetics", M::NONE),
    ("fox", AnyStatus::Fox(FoxStatus::Attack100Start), "mpCommonSetFighterFallOnEdgeBreak", M::STOPEDGE),
    ("fox", AnyStatus::Fox(FoxStatus::Attack100Loop), "mpCommonSetFighterFallOnEdgeBreak", M::STOPEDGE),
    ("fox", AnyStatus::Fox(FoxStatus::Attack100End), "mpCommonSetFighterFallOnEdgeBreak", M::STOPEDGE),
    ("fox", AnyStatus::Fox(FoxStatus::AppearR), "mpCommonUpdateFighterProjectFloor", M::NONE),
    ("fox", AnyStatus::Fox(FoxStatus::AppearL), "mpCommonUpdateFighterProjectFloor", M::NONE),
    ("fox", AnyStatus::Fox(FoxStatus::SpecialN), "mpCommonSetFighterFallOnEdgeBreak", M::STOPEDGE),
    ("fox", AnyStatus::Fox(FoxStatus::SpecialAirN), "mpCommonProcFighterWaitOrLanding", M::NONE),
    ("fox", AnyStatus::Fox(FoxStatus::SpecialHiStart), "ftFoxSpecialHiStartProcMap", M::NONE),
    ("fox", AnyStatus::Fox(FoxStatus::SpecialAirHiStart), "ftFoxSpecialAirHiStartProcMap", M::NONE),
    ("fox", AnyStatus::Fox(FoxStatus::SpecialHiHold), "ftFoxSpecialHiHoldProcMap", M::NONE),
    ("fox", AnyStatus::Fox(FoxStatus::SpecialAirHiHold), "ftFoxSpecialAirHiHoldProcMap", M::NONE),
    ("fox", AnyStatus::Fox(FoxStatus::SpecialHi), "ftFoxSpecialHiProcMap", M::NONE),
    ("fox", AnyStatus::Fox(FoxStatus::SpecialAirHi), "ftFoxSpecialAirHiProcMap", M::NONE),
    ("fox", AnyStatus::Fox(FoxStatus::SpecialHiEnd), "mpCommonSetFighterFallOnGroundBreak", M::NONE),
    ("fox", AnyStatus::Fox(FoxStatus::SpecialAirHiEnd), "ftFoxSpecialAirHiEndProcMap", M::CLIFF),
    ("fox", AnyStatus::Fox(FoxStatus::SpecialAirHiBound), "ftFoxSpecialAirHiBoundProcMap", M::CLIFF),
    ("fox", AnyStatus::Fox(FoxStatus::SpecialLwStart), "mpCommonSetFighterFallOnEdgeBreak", M::STOPEDGE),
    ("fox", AnyStatus::Fox(FoxStatus::SpecialLwHit), "mpCommonSetFighterFallOnEdgeBreak", M::STOPEDGE),
    ("fox", AnyStatus::Fox(FoxStatus::SpecialLwEnd), "mpCommonSetFighterFallOnEdgeBreak", M::STOPEDGE),
    ("fox", AnyStatus::Fox(FoxStatus::SpecialLwLoop), "mpCommonSetFighterFallOnEdgeBreak", M::STOPEDGE),
    ("fox", AnyStatus::Fox(FoxStatus::SpecialLwTurn), "mpCommonSetFighterFallOnEdgeBreak", M::STOPEDGE),
    ("fox", AnyStatus::Fox(FoxStatus::SpecialAirLwStart), "mpCommonProcFighterWaitOrLanding", M::NONE),
    ("fox", AnyStatus::Fox(FoxStatus::SpecialAirLwHit), "mpCommonProcFighterWaitOrLanding", M::NONE),
    ("fox", AnyStatus::Fox(FoxStatus::SpecialAirLwEnd), "mpCommonProcFighterWaitOrLanding", M::NONE),
    ("fox", AnyStatus::Fox(FoxStatus::SpecialAirLwLoop), "mpCommonProcFighterWaitOrLanding", M::NONE),
    ("fox", AnyStatus::Fox(FoxStatus::SpecialAirLwTurn), "mpCommonProcFighterWaitOrLanding", M::NONE),
    ("kirby", AnyStatus::Kirby(KirbyStatus::Attack100Start), "mpCommonSetFighterFallOnEdgeBreak", M::STOPEDGE),
    ("kirby", AnyStatus::Kirby(KirbyStatus::Attack100Loop), "mpCommonSetFighterFallOnEdgeBreak", M::STOPEDGE),
    ("kirby", AnyStatus::Kirby(KirbyStatus::Attack100End), "mpCommonSetFighterFallOnEdgeBreak", M::STOPEDGE),
    ("kirby", AnyStatus::Kirby(KirbyStatus::JumpAerialF1), "mpCommonProcFighterCliffWaitOrLanding", M::CLIFF),
    ("kirby", AnyStatus::Kirby(KirbyStatus::JumpAerialF2), "mpCommonProcFighterCliffWaitOrLanding", M::CLIFF),
    ("kirby", AnyStatus::Kirby(KirbyStatus::JumpAerialF3), "mpCommonProcFighterCliffWaitOrLanding", M::CLIFF),
    ("kirby", AnyStatus::Kirby(KirbyStatus::JumpAerialF4), "mpCommonProcFighterCliffWaitOrLanding", M::CLIFF),
    ("kirby", AnyStatus::Kirby(KirbyStatus::JumpAerialF5), "mpCommonProcFighterCliffWaitOrLanding", M::CLIFF),
    ("kirby", AnyStatus::Kirby(KirbyStatus::ThrowF), "ftKirbyThrowFProcMap", M::NONE),
    ("kirby", AnyStatus::Kirby(KirbyStatus::ThrowFFall), "ftKirbyThrowFProcMap", M::NONE),
    ("kirby", AnyStatus::Kirby(KirbyStatus::ThrowFLanding), "ftKirbyThrowFLandingProcMap", M::STOPEDGE),
    ("kirby", AnyStatus::Kirby(KirbyStatus::CopyMarioSpecialN), "ftKirbyCopyMarioSpecialNProcMap", M::STOPEDGE),
    ("kirby", AnyStatus::Kirby(KirbyStatus::CopyMarioSpecialAirN), "ftKirbyCopyMarioSpecialAirNProcMap", M::NONE),
    ("kirby", AnyStatus::Kirby(KirbyStatus::CopyLuigiSpecialN), "ftKirbyCopyMarioSpecialNProcMap", M::STOPEDGE),
    ("kirby", AnyStatus::Kirby(KirbyStatus::CopyLuigiSpecialAirN), "ftKirbyCopyMarioSpecialAirNProcMap", M::NONE),
    ("kirby", AnyStatus::Kirby(KirbyStatus::CopyFoxSpecialN), "mpCommonSetFighterFallOnEdgeBreak", M::STOPEDGE),
    ("kirby", AnyStatus::Kirby(KirbyStatus::CopyFoxSpecialAirN), "mpCommonProcFighterWaitOrLanding", M::NONE),
    ("kirby", AnyStatus::Kirby(KirbyStatus::CopySamusSpecialNStart), "ftKirbyCopySamusSpecialNStartProcMap", M::STOPEDGE),
    ("kirby", AnyStatus::Kirby(KirbyStatus::CopySamusSpecialNLoop), "ftKirbyCopySamusSpecialNLoopProcMap", M::STOPEDGE),
    ("kirby", AnyStatus::Kirby(KirbyStatus::CopySamusSpecialNEnd), "ftKirbyCopySamusSpecialNEndProcMap", M::STOPEDGE),
    ("kirby", AnyStatus::Kirby(KirbyStatus::CopySamusSpecialAirNStart), "ftKirbyCopySamusSpecialAirNStartProcMap", M::NONE),
    ("kirby", AnyStatus::Kirby(KirbyStatus::CopySamusSpecialAirNEnd), "ftKirbyCopySamusSpecialAirNEndProcMap", M::NONE),
    ("kirby", AnyStatus::Kirby(KirbyStatus::CopyDonkeySpecialNStart), "ftKirbyCopyDonkeySpecialNStartProcMap", M::STOPEDGE),
    ("kirby", AnyStatus::Kirby(KirbyStatus::CopyDonkeySpecialAirNStart), "ftKirbyCopyDonkeySpecialAirNStartProcMap", M::NONE),
    ("kirby", AnyStatus::Kirby(KirbyStatus::CopyDonkeySpecialNLoop), "ftKirbyCopyDonkeySpecialNLoopProcMap", M::STOPEDGE),
    ("kirby", AnyStatus::Kirby(KirbyStatus::CopyDonkeySpecialAirNLoop), "ftKirbyCopyDonkeySpecialAirNLoopProcMap", M::NONE),
    ("kirby", AnyStatus::Kirby(KirbyStatus::CopyDonkeySpecialNEnd), "mpCommonSetFighterFallOnEdgeBreak", M::STOPEDGE),
    ("kirby", AnyStatus::Kirby(KirbyStatus::CopyDonkeySpecialAirNEnd), "ftKirbyCopyDonkeySpecialAirNEndProcMap", M::NONE),
    ("kirby", AnyStatus::Kirby(KirbyStatus::CopyDonkeySpecialNFull), "mpCommonSetFighterFallOnEdgeBreak", M::STOPEDGE),
    ("kirby", AnyStatus::Kirby(KirbyStatus::CopyDonkeySpecialAirNFull), "ftKirbyCopyDonkeySpecialAirNEndProcMap", M::NONE),
    ("kirby", AnyStatus::Kirby(KirbyStatus::AppearR), "mpCommonUpdateFighterProjectFloor", M::NONE),
    ("kirby", AnyStatus::Kirby(KirbyStatus::AppearL), "mpCommonUpdateFighterProjectFloor", M::NONE),
    ("kirby", AnyStatus::Kirby(KirbyStatus::CopyPikachuSpecialN), "ftKirbyCopyPikachuSpecialNProcMap", M::NONE),
    ("kirby", AnyStatus::Kirby(KirbyStatus::CopyPikachuSpecialAirN), "ftKirbyCopyPikachuSpecialAirNProcMap", M::NONE),
    ("kirby", AnyStatus::Kirby(KirbyStatus::CopyNessSpecialN), "ftKirbyCopyNessSpecialNProcMap", M::NONE),
    ("kirby", AnyStatus::Kirby(KirbyStatus::CopyNessSpecialAirN), "ftKirbyCopyNessSpecialAirNProcMap", M::NONE),
    ("kirby", AnyStatus::Kirby(KirbyStatus::SpecialHi), "ftKirbySpecialHiProcMap", M::STOPEDGE | M::CLIFF | M::CEILHEAVY),
    ("kirby", AnyStatus::Kirby(KirbyStatus::SpecialHiLanding), "mpCommonSetFighterFallOnGroundBreak", M::NONE),
    ("kirby", AnyStatus::Kirby(KirbyStatus::SpecialAirHi), "ftKirbySpecialHiProcMap", M::STOPEDGE | M::CLIFF | M::CEILHEAVY),
    ("kirby", AnyStatus::Kirby(KirbyStatus::SpecialAirHiFall), "ftKirbySpecialAirHiFallProcMap", M::CLIFF | M::CEILHEAVY),
    ("kirby", AnyStatus::Kirby(KirbyStatus::SpecialLwStart), "ftKirbySpecialLwStartProcMap", M::NONE),
    ("kirby", AnyStatus::Kirby(KirbyStatus::SpecialLwUnk), "ftKirbySpecialLwUnkProcMap", M::NONE),
    ("kirby", AnyStatus::Kirby(KirbyStatus::SpecialLwHold), "ftKirbySpecialLwHoldProcMap", M::NONE),
    ("kirby", AnyStatus::Kirby(KirbyStatus::SpecialLwEnd), "mpCommonProcFighterWaitOrLanding", M::NONE),
    ("kirby", AnyStatus::Kirby(KirbyStatus::SpecialAirLwStart), "ftKirbySpecialAirLwStartProcMap", M::NONE),
    ("kirby", AnyStatus::Kirby(KirbyStatus::SpecialAirLwHold), "ftKirbySpecialAirLwHoldProcMap", M::NONE),
    ("kirby", AnyStatus::Kirby(KirbyStatus::SpecialAirLwLanding), "ftKirbySpecialLwHoldProcMap", M::NONE),
    ("kirby", AnyStatus::Kirby(KirbyStatus::SpecialAirLwFall), "ftKirbySpecialAirLwHoldProcMap", M::NONE),
    ("kirby", AnyStatus::Kirby(KirbyStatus::SpecialAirLwEnd), "mpCommonProcFighterWaitOrLanding", M::NONE),
    ("kirby", AnyStatus::Kirby(KirbyStatus::SpecialNStart), "ftKirbySpecialNStartProcMap", M::NONE),
    ("kirby", AnyStatus::Kirby(KirbyStatus::SpecialNLoop), "ftKirbySpecialNLoopProcMap", M::NONE),
    ("kirby", AnyStatus::Kirby(KirbyStatus::SpecialNEnd), "ftKirbySpecialNEndProcMap", M::NONE),
    ("kirby", AnyStatus::Kirby(KirbyStatus::SpecialNCatch), "ftKirbySpecialNCatchProcMap", M::NONE),
    ("kirby", AnyStatus::Kirby(KirbyStatus::SpecialNEat), "ftKirbySpecialNEatProcMap", M::NONE),
    ("kirby", AnyStatus::Kirby(KirbyStatus::SpecialNThrow), "ftKirbySpecialNThrowProcMap", M::STOPEDGE),
    ("kirby", AnyStatus::Kirby(KirbyStatus::SpecialNWait), "ftKirbySpecialNWaitProcMap", M::NONE),
    ("kirby", AnyStatus::Kirby(KirbyStatus::SpecialNTurn), "ftKirbySpecialNTurnProcMap", M::NONE),
    ("kirby", AnyStatus::Kirby(KirbyStatus::SpecialNCopy), "ftKirbySpecialNCopyProcMap", M::NONE),
    ("kirby", AnyStatus::Kirby(KirbyStatus::SpecialAirNStart), "ftKirbySpecialAirNStartProcMap", M::NONE),
    ("kirby", AnyStatus::Kirby(KirbyStatus::SpecialAirNLoop), "ftKirbySpecialAirNLoopProcMap", M::NONE),
    ("kirby", AnyStatus::Kirby(KirbyStatus::SpecialAirNEnd), "ftKirbySpecialAirNEndProcMap", M::NONE),
    ("kirby", AnyStatus::Kirby(KirbyStatus::SpecialAirNCatch), "ftKirbySpecialAirNCatchProcMap", M::NONE),
    ("kirby", AnyStatus::Kirby(KirbyStatus::SpecialAirNEat), "ftKirbySpecialAirNEatProcMap", M::NONE),
    ("kirby", AnyStatus::Kirby(KirbyStatus::SpecialAirNThrow), "ftKirbySpecialAirNThrowProcMap", M::NONE),
    ("kirby", AnyStatus::Kirby(KirbyStatus::SpecialAirNWait), "ftKirbySpecialAirNWaitProcMap", M::NONE),
    ("kirby", AnyStatus::Kirby(KirbyStatus::SpecialAirNTurn), "ftKirbySpecialAirNTurnProcMap", M::NONE),
    ("kirby", AnyStatus::Kirby(KirbyStatus::SpecialAirNCopy), "ftKirbySpecialAirNCopyProcMap", M::NONE),
    ("kirby", AnyStatus::Kirby(KirbyStatus::CopyLinkSpecialN), "ftKirbyCopyLinkSpecialNProcMap", M::NONE),
    ("kirby", AnyStatus::Kirby(KirbyStatus::CopyLinkSpecialNGet), "mpCommonSetFighterFallOnGroundBreak", M::NONE),
    ("kirby", AnyStatus::Kirby(KirbyStatus::CopyLinkSpecialNEmpty), "ftKirbyCopyLinkSpecialNEmptyProcMap", M::NONE),
    ("kirby", AnyStatus::Kirby(KirbyStatus::CopyLinkSpecialAirN), "ftKirbyCopyLinkSpecialAirNProcMap", M::NONE),
    ("kirby", AnyStatus::Kirby(KirbyStatus::CopyLinkSpecialAirNReturn), "mpCommonProcFighterWaitOrLanding", M::NONE),
    ("kirby", AnyStatus::Kirby(KirbyStatus::CopyLinkSpecialAirNEmpty), "ftKirbyCopyLinkSpecialAirNEmptyProcMap", M::NONE),
    ("kirby", AnyStatus::Kirby(KirbyStatus::CopyPurinSpecialN), "ftKirbyCopyPurinSpecialNProcMap", M::NONE),
    ("kirby", AnyStatus::Kirby(KirbyStatus::CopyPurinSpecialAirN), "ftKirbyCopyPurinSpecialAirNProcMap", M::NONE),
    ("kirby", AnyStatus::Kirby(KirbyStatus::CopyCaptainSpecialN), "ftKirbyCopyCaptainSpecialNProcMap", M::STOPEDGE),
    ("kirby", AnyStatus::Kirby(KirbyStatus::CopyCaptainSpecialAirN), "ftKirbyCopyCaptainSpecialAirNProcMap", M::NONE),
    ("kirby", AnyStatus::Kirby(KirbyStatus::CopyYoshiSpecialN), "ftKirbyCopyYoshiSpecialNProcMap", M::NONE),
    ("kirby", AnyStatus::Kirby(KirbyStatus::CopyYoshiSpecialNCatch), "ftKirbyCopyYoshiSpecialNCatchProcMap", M::NONE),
    ("kirby", AnyStatus::Kirby(KirbyStatus::CopyYoshiSpecialNRelease), "ftKirbyCopyYoshiSpecialNReleaseProcMap", M::NONE),
    ("kirby", AnyStatus::Kirby(KirbyStatus::CopyYoshiSpecialAirN), "ftKirbyCopyYoshiSpecialAirNProcMap", M::NONE),
    ("kirby", AnyStatus::Kirby(KirbyStatus::CopyYoshiSpecialAirNCatch), "ftKirbyCopyYoshiSpecialAirNCatchProcMap", M::NONE),
    ("kirby", AnyStatus::Kirby(KirbyStatus::CopyYoshiSpecialAirNRelease), "ftKirbyCopyYoshiSpecialAirNReleaseProcMap", M::NONE),
    ("link", AnyStatus::Link(LinkStatus::Attack13), "mpCommonSetFighterFallOnEdgeBreak", M::STOPEDGE),
    ("link", AnyStatus::Link(LinkStatus::Attack100Start), "mpCommonSetFighterFallOnEdgeBreak", M::STOPEDGE),
    ("link", AnyStatus::Link(LinkStatus::Attack100Loop), "mpCommonSetFighterFallOnEdgeBreak", M::STOPEDGE),
    ("link", AnyStatus::Link(LinkStatus::Attack100End), "mpCommonSetFighterFallOnEdgeBreak", M::STOPEDGE),
    ("link", AnyStatus::Link(LinkStatus::AppearR), "mpCommonUpdateFighterProjectFloor", M::NONE),
    ("link", AnyStatus::Link(LinkStatus::AppearL), "mpCommonUpdateFighterProjectFloor", M::NONE),
    ("link", AnyStatus::Link(LinkStatus::SpecialHi), "ftLinkSpecialHiProcMap", M::NONE),
    ("link", AnyStatus::Link(LinkStatus::SpecialHiEnd), "ftLinkSpecialHiEndProcMap", M::NONE),
    ("link", AnyStatus::Link(LinkStatus::SpecialAirHi), "ftLinkSpecialAirHiProcMap", M::CLIFF | M::CEILHEAVY),
    ("link", AnyStatus::Link(LinkStatus::SpecialN), "ftLinkSpecialNProcMap", M::NONE),
    ("link", AnyStatus::Link(LinkStatus::SpecialNGet), "mpCommonSetFighterFallOnGroundBreak", M::NONE),
    ("link", AnyStatus::Link(LinkStatus::SpecialNEmpty), "ftLinkSpecialNEmptyProcMap", M::NONE),
    ("link", AnyStatus::Link(LinkStatus::SpecialAirN), "ftLinkSpecialAirNProcMap", M::NONE),
    ("link", AnyStatus::Link(LinkStatus::SpecialAirNReturn), "mpCommonProcFighterWaitOrLanding", M::NONE),
    ("link", AnyStatus::Link(LinkStatus::SpecialAirNEmpty), "ftLinkSpecialAirNEmptyProcMap", M::NONE),
    ("link", AnyStatus::Link(LinkStatus::SpecialLw), "ftLinkSpecialLwProcMap", M::NONE),
    ("link", AnyStatus::Link(LinkStatus::SpecialAirLw), "ftLinkSpecialAirLwProcMap", M::NONE),
    ("mario", AnyStatus::Mario(MarioStatus::Attack13), "mpCommonSetFighterFallOnEdgeBreak", M::STOPEDGE), // shared
    ("mario", AnyStatus::Mario(MarioStatus::AppearR), "mpCommonUpdateFighterProjectFloor", M::NONE), // shared
    ("mario", AnyStatus::Mario(MarioStatus::AppearL), "mpCommonUpdateFighterProjectFloor", M::NONE), // shared
    ("mario", AnyStatus::Mario(MarioStatus::SpecialN), "ftMarioSpecialNProcMap", M::STOPEDGE), // shared
    ("mario", AnyStatus::Mario(MarioStatus::SpecialAirN), "ftMarioSpecialAirNProcMap", M::NONE), // shared
    ("mario", AnyStatus::Mario(MarioStatus::SpecialHi), "ftMarioSpecialHiProcMap", M::STOPEDGE | M::CLIFF), // shared
    ("mario", AnyStatus::Mario(MarioStatus::SpecialAirHi), "ftMarioSpecialHiProcMap", M::STOPEDGE | M::CLIFF), // shared
    ("mario", AnyStatus::Mario(MarioStatus::SpecialLw), "ftMarioSpecialLwProcMap", M::NONE), // shared
    ("mario", AnyStatus::Mario(MarioStatus::SpecialAirLw), "ftMarioSpecialAirLwProcMap", M::NONE), // shared
    ("ness", AnyStatus::Ness(NessStatus::Attack13), "mpCommonSetFighterFallOnEdgeBreak", M::STOPEDGE),
    ("ness", AnyStatus::Ness(NessStatus::AppearRStart), "mpCommonUpdateFighterProjectFloor", M::NONE),
    ("ness", AnyStatus::Ness(NessStatus::AppearLStart), "mpCommonUpdateFighterProjectFloor", M::NONE),
    ("ness", AnyStatus::Ness(NessStatus::AppearWait), "mpCommonUpdateFighterProjectFloor", M::NONE),
    ("ness", AnyStatus::Ness(NessStatus::AppearREnd), "mpCommonUpdateFighterProjectFloor", M::NONE),
    ("ness", AnyStatus::Ness(NessStatus::AppearLEnd), "mpCommonUpdateFighterProjectFloor", M::NONE),
    ("ness", AnyStatus::Ness(NessStatus::SpecialN), "ftNessSpecialNProcMap", M::STOPEDGE),
    ("ness", AnyStatus::Ness(NessStatus::SpecialAirN), "ftNessSpecialAirNProcMap", M::NONE),
    ("ness", AnyStatus::Ness(NessStatus::SpecialHiStart), "ftNessSpecialHiStartProcMap", M::STOPEDGE),
    ("ness", AnyStatus::Ness(NessStatus::SpecialHiHold), "ftNessSpecialHiHoldProcMap", M::STOPEDGE),
    ("ness", AnyStatus::Ness(NessStatus::SpecialHiEnd), "ftNessSpecialHiEndProcMap", M::STOPEDGE),
    ("ness", AnyStatus::Ness(NessStatus::SpecialHiJibaku), "ftNessSpecialHiJibakuProcMap", M::NONE),
    ("ness", AnyStatus::Ness(NessStatus::SpecialAirHiStart), "ftNessSpecialAirHiStartProcMap", M::CLIFF),
    ("ness", AnyStatus::Ness(NessStatus::SpecialAirHiHold), "ftNessSpecialAirHiHoldProcMap", M::CLIFF),
    ("ness", AnyStatus::Ness(NessStatus::SpecialAirHiEnd), "ftNessSpecialAirHiEndProcMap", M::CLIFF),
    ("ness", AnyStatus::Ness(NessStatus::SpecialAirHiBound), "ftNessSpecialAirHiJibakuBoundProcMap", M::CLIFF),
    ("ness", AnyStatus::Ness(NessStatus::SpecialAirHiJibaku), "ftNessSpecialAirHiJibakuProcMap", M::CLIFF),
    ("ness", AnyStatus::Ness(NessStatus::SpecialLwStart), "ftNessSpecialLwStartProcMap", M::STOPEDGE),
    ("ness", AnyStatus::Ness(NessStatus::SpecialLwHold), "ftNessSpecialLwHoldProcMap", M::STOPEDGE),
    ("ness", AnyStatus::Ness(NessStatus::SpecialLwHit), "ftNessSpecialLwHitProcMap", M::STOPEDGE),
    ("ness", AnyStatus::Ness(NessStatus::SpecialLwEnd), "ftNessSpecialLwEndProcMap", M::STOPEDGE),
    ("ness", AnyStatus::Ness(NessStatus::SpecialAirLwStart), "ftNessSpecialAirLwStartProcMap", M::CLIFF),
    ("ness", AnyStatus::Ness(NessStatus::SpecialAirLwHold), "ftNessSpecialAirLwHoldProcMap", M::CLIFF),
    ("ness", AnyStatus::Ness(NessStatus::SpecialAirLwHit), "ftNessSpecialAirLwHitProcMap", M::CLIFF),
    ("ness", AnyStatus::Ness(NessStatus::SpecialAirLwEnd), "ftNessSpecialAirLwEndProcMap", M::CLIFF),
    ("pikachu", AnyStatus::Pikachu(PikachuStatus::AppearR), "mpCommonUpdateFighterProjectFloor", M::NONE),
    ("pikachu", AnyStatus::Pikachu(PikachuStatus::AppearL), "mpCommonUpdateFighterProjectFloor", M::NONE),
    ("pikachu", AnyStatus::Pikachu(PikachuStatus::SpecialN), "ftPikachuSpecialNProcMap", M::STOPEDGE),
    ("pikachu", AnyStatus::Pikachu(PikachuStatus::SpecialAirN), "ftPikachuSpecialAirNProcMap", M::NONE),
    ("pikachu", AnyStatus::Pikachu(PikachuStatus::SpecialLwStart), "ftPikachuSpecialLwStartProcMap", M::STOPEDGE),
    ("pikachu", AnyStatus::Pikachu(PikachuStatus::SpecialLwLoop), "ftPikachuSpecialLwLoopProcMap", M::STOPEDGE),
    ("pikachu", AnyStatus::Pikachu(PikachuStatus::SpecialLwHit), "ftPikachuSpecialLwHitProcMap", M::STOPEDGE),
    ("pikachu", AnyStatus::Pikachu(PikachuStatus::SpecialLwEnd), "ftPikachuSpecialLwEndProcMap", M::STOPEDGE),
    ("pikachu", AnyStatus::Pikachu(PikachuStatus::SpecialAirLwStart), "ftPikachuSpecialAirLwStartProcMap", M::NONE),
    ("pikachu", AnyStatus::Pikachu(PikachuStatus::SpecialAirLwLoop), "ftPikachuSpecialAirLwLoopProcMap", M::NONE),
    ("pikachu", AnyStatus::Pikachu(PikachuStatus::SpecialAirLwHit), "ftPikachuSpecialAirLwHitProcMap", M::NONE),
    ("pikachu", AnyStatus::Pikachu(PikachuStatus::SpecialAirLwEnd), "ftPikachuSpecialAirLwEndProcMap", M::NONE),
    ("pikachu", AnyStatus::Pikachu(PikachuStatus::SpecialHiStart), "ftPikachuSpecialHiStartProcMap", M::NONE),
    ("pikachu", AnyStatus::Pikachu(PikachuStatus::SpecialHi), "ftPikachuSpecialHiProcMap", M::NONE),
    ("pikachu", AnyStatus::Pikachu(PikachuStatus::SpecialHiEnd), "ftPikachuSpecialHiEndProcMap", M::NONE),
    ("pikachu", AnyStatus::Pikachu(PikachuStatus::SpecialAirHiStart), "ftPikachuSpecialAirHiStartProcMap", M::CLIFF),
    ("pikachu", AnyStatus::Pikachu(PikachuStatus::SpecialAirHi), "ftPikachuSpecialAirHiProcMap", M::CLIFF),
    ("pikachu", AnyStatus::Pikachu(PikachuStatus::SpecialAirHiEnd), "ftPikachuSpecialAirHiEndProcMap", M::CLIFF),
    ("purin", AnyStatus::Purin(PurinStatus::JumpAerialF1), "mpCommonProcFighterCliffWaitOrLanding", M::CLIFF),
    ("purin", AnyStatus::Purin(PurinStatus::JumpAerialF2), "mpCommonProcFighterCliffWaitOrLanding", M::CLIFF),
    ("purin", AnyStatus::Purin(PurinStatus::JumpAerialF3), "mpCommonProcFighterCliffWaitOrLanding", M::CLIFF),
    ("purin", AnyStatus::Purin(PurinStatus::JumpAerialF4), "mpCommonProcFighterCliffWaitOrLanding", M::CLIFF),
    ("purin", AnyStatus::Purin(PurinStatus::JumpAerialF5), "mpCommonProcFighterCliffWaitOrLanding", M::CLIFF),
    ("purin", AnyStatus::Purin(PurinStatus::AppearR), "mpCommonUpdateFighterProjectFloor", M::NONE),
    ("purin", AnyStatus::Purin(PurinStatus::AppearL), "mpCommonUpdateFighterProjectFloor", M::NONE),
    ("purin", AnyStatus::Purin(PurinStatus::SpecialN), "ftPurinSpecialNProcMap", M::STOPEDGE),
    ("purin", AnyStatus::Purin(PurinStatus::SpecialAirN), "ftPurinSpecialAirNProcMap", M::NONE),
    ("purin", AnyStatus::Purin(PurinStatus::SpecialHi), "ftPurinSpecialHiProcMap", M::STOPEDGE),
    ("purin", AnyStatus::Purin(PurinStatus::SpecialAirHi), "ftPurinSpecialAirHiProcMap", M::NONE),
    ("purin", AnyStatus::Purin(PurinStatus::SpecialLw), "ftPurinSpecialLwProcMap", M::NONE),
    ("purin", AnyStatus::Purin(PurinStatus::SpecialAirLw), "ftPurinSpecialAirLwProcMap", M::NONE),
    ("samus", AnyStatus::Samus(SamusStatus::AppearR), "mpCommonUpdateFighterProjectFloor", M::NONE),
    ("samus", AnyStatus::Samus(SamusStatus::AppearL), "mpCommonUpdateFighterProjectFloor", M::NONE),
    ("samus", AnyStatus::Samus(SamusStatus::SpecialNStart), "ftSamusSpecialNStartProcMap", M::STOPEDGE),
    ("samus", AnyStatus::Samus(SamusStatus::SpecialNLoop), "ftSamusSpecialNLoopProcMap", M::STOPEDGE),
    ("samus", AnyStatus::Samus(SamusStatus::SpecialNEnd), "ftSamusSpecialNEndProcMap", M::STOPEDGE),
    ("samus", AnyStatus::Samus(SamusStatus::SpecialAirNStart), "ftSamusSpecialAirNStartProcMap", M::NONE),
    ("samus", AnyStatus::Samus(SamusStatus::SpecialAirNEnd), "ftSamusSpecialAirNEndProcMap", M::NONE),
    ("samus", AnyStatus::Samus(SamusStatus::SpecialHi), "ftSamusSpecialHiProcMap", M::STOPEDGE | M::CLIFF),
    ("samus", AnyStatus::Samus(SamusStatus::SpecialAirHi), "ftSamusSpecialHiProcMap", M::STOPEDGE | M::CLIFF),
    ("samus", AnyStatus::Samus(SamusStatus::SpecialLw), "ftSamusSpecialLwProcMap", M::STOPEDGE),
    ("samus", AnyStatus::Samus(SamusStatus::SpecialAirLw), "ftSamusSpecialAirLwProcMap", M::NONE),
    ("yoshi", AnyStatus::Yoshi(YoshiStatus::AppearR), "mpCommonUpdateFighterProjectFloor", M::NONE),
    ("yoshi", AnyStatus::Yoshi(YoshiStatus::AppearL), "mpCommonUpdateFighterProjectFloor", M::NONE),
    ("yoshi", AnyStatus::Yoshi(YoshiStatus::SpecialHi), "ftYoshiSpecialHiProcMap", M::NONE),
    ("yoshi", AnyStatus::Yoshi(YoshiStatus::SpecialAirHi), "ftYoshiSpecialAirHiProcMap", M::CLIFF),
    ("yoshi", AnyStatus::Yoshi(YoshiStatus::SpecialLwStart), "ftYoshiSpecialLwStartProcMap", M::CLIFF | M::CEILHEAVY),
    ("yoshi", AnyStatus::Yoshi(YoshiStatus::SpecialLwLanding), "mpCommonSetFighterFallOnGroundBreak", M::NONE),
    ("yoshi", AnyStatus::Yoshi(YoshiStatus::SpecialAirLwStart), "ftYoshiSpecialLwStartProcMap", M::CLIFF | M::CEILHEAVY),
    ("yoshi", AnyStatus::Yoshi(YoshiStatus::SpecialAirLwLoop), "ftYoshiSpecialAirLwLoopProcMap", M::CLIFF | M::CEILHEAVY),
    ("yoshi", AnyStatus::Yoshi(YoshiStatus::SpecialN), "ftYoshiSpecialNProcMap", M::NONE),
    ("yoshi", AnyStatus::Yoshi(YoshiStatus::SpecialNCatch), "ftYoshiSpecialNCatchProcMap", M::NONE),
    ("yoshi", AnyStatus::Yoshi(YoshiStatus::SpecialNRelease), "ftYoshiSpecialNReleaseProcMap", M::NONE),
    ("yoshi", AnyStatus::Yoshi(YoshiStatus::SpecialAirN), "ftYoshiSpecialAirNProcMap", M::NONE),
    ("yoshi", AnyStatus::Yoshi(YoshiStatus::SpecialAirNCatch), "ftYoshiSpecialAirNCatchProcMap", M::NONE),
    ("yoshi", AnyStatus::Yoshi(YoshiStatus::SpecialAirNRelease), "ftYoshiSpecialAirNReleaseProcMap", M::NONE),
];

/// Classes the decomp table reaches only on a branch the status never runs.
#[rustfmt::skip]
const UNREACHED: &[(AnyStatus, u8, &str)] = &[
    (AnyStatus::Mario(MarioStatus::SpecialAirHi), M::STOPEDGE, "ftmariospecialhi.c:130: ground branch; a landing ends the status"),
    (AnyStatus::Samus(SamusStatus::SpecialAirHi), M::STOPEDGE, "ftsamusspecialhi.c:68: ground branch; a landing ends the status"),
    (AnyStatus::Kirby(KirbyStatus::SpecialAirHi), M::STOPEDGE, "ftkirbyspecialhi.c:205: ground branch; a landing ends the status"),
];

fn port_classes(s: AnyStatus) -> u8 {
    let mut out = M::NONE;
    let mut f = Fighter::new(FighterKind::Mario, 0, 3);
    f.status.status = s;
    if stops_at_edge(s) {
        out |= M::STOPEDGE;
    }
    if is_ceil_heavy_status(s) {
        out |= M::CEILHEAVY;
    }
    for vel_y in [-1.0, 1.0, 40.0] {
        for flag in [0, 1] {
            for t in [0.0, 100.0] {
                f.physics.vel_air.y = vel_y;
                f.motion_script.flags = [flag; 4];
                f.yoshi.egg_thrown = flag != 0;
                f.status.set_time(t);
                f.status.anim_frame = t;
                f.captain.dive_cliff_wait = 0;
                if crate::captain::special_lw_stops_at_edge(&f) {
                    out |= M::STOPEDGE;
                }
                if teeters_at_edge(&f) {
                    out |= M::CLIFFEDGE;
                }
                if allows_cliff(&f) {
                    out |= M::CLIFF;
                }
            }
        }
    }
    out
}

fn names(c: u8) -> String {
    let all = [
        (M::STOPEDGE, "STOPEDGE"),
        (M::CLIFFEDGE, "CLIFFEDGE"),
        (M::CLIFF, "CLIFF"),
        (M::CEILHEAVY, "CEILHEAVY"),
    ];
    let v: Vec<_> = all
        .iter()
        .filter(|(b, _)| c & b != 0)
        .map(|(_, n)| *n)
        .collect();
    if v.is_empty() {
        "-".into()
    } else {
        v.join("+")
    }
}

#[test]
fn every_status_proc_map_class_matches_the_port_map_pass() {
    let mut bad = Vec::new();
    for &(table, s, proc_map, decomp) in ROWS {
        let unreached = UNREACHED
            .iter()
            .filter(|(u, _, _)| *u == s)
            .fold(M::NONE, |a, (_, c, _)| a | c);
        let want = decomp & !unreached;
        let got = port_classes(s);
        if got != want {
            bad.push(format!(
                "{table}.{s:?} ({proc_map}): decomp {}, port {}",
                names(want),
                names(got)
            ));
        }
    }
    assert!(
        bad.is_empty(),
        "{} mismatches:\n{}",
        bad.len(),
        bad.join("\n")
    );
}

/// The conditional ledge gates: the probe above only asks whether a status
/// can catch at all.
#[test]
fn conditional_ledge_gates_follow_their_proc_maps() {
    let mut f = Fighter::new(FighterKind::Samus, 0, 3);
    // `ftSamusSpecialHiProcMap`: rising projects, falling catches.
    for s in [SamusStatus::SpecialHi, SamusStatus::SpecialAirHi] {
        f.status.status = AnyStatus::Samus(s);
        f.physics.vel_air.y = 0.0;
        assert!(!allows_cliff(&f), "{s:?} rising");
        f.physics.vel_air.y = -1.0;
        assert!(allows_cliff(&f), "{s:?} falling");
    }
    // `ftYoshiSpecialAirHiProcMap`: flag 1 (the throw) first.
    f.status.status = AnyStatus::Yoshi(YoshiStatus::SpecialAirHi);
    f.yoshi.egg_thrown = false;
    assert!(!allows_cliff(&f));
    f.yoshi.egg_thrown = true;
    assert!(allows_cliff(&f));
    // `ftCaptainSpecialLwAirProcMap`: the grounded kick's edge stop on flag 3.
    f.status.status = AnyStatus::Captain(CaptainStatus::SpecialLwAir);
    f.motion_script.flags = [0; 4];
    assert!(!crate::captain::special_lw_stops_at_edge(&f));
    f.motion_script.flags[3] = 1;
    assert!(crate::captain::special_lw_stops_at_edge(&f));
}
