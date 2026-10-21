//! Every character status whose `proc_physics` is one of the generic
//! `ftPhysicsApply*` callbacks (the `dFT*StatusDescs` tables in
//! `ft<char>status.h`), checked by behaviour: ground friction vs TransN vs
//! none, and aerial friction vs drift vs drift-with-fast-fall vs TransN.

use super::*;
use crate::status::{
    AnyStatus, CaptainStatus, DonkeyStatus, FoxStatus, KirbyStatus, LinkStatus, MarioStatus,
    NessStatus, PikachuStatus, PurinStatus, SamusStatus, YoshiStatus,
};

#[derive(Clone, Copy, Debug, PartialEq)]
enum C {
    /// `ftPhysicsApplyGroundVelFriction`
    GroundFriction,
    /// `ftPhysicsApplyGroundVelTransN`
    GroundTransN,
    /// `ftPhysicsApplyAirVelFriction`: gravity (fast if already falling
    /// fast), no stick drift, no fast-fall input.
    AirFriction,
    /// `ftPhysicsApplyAirVelDrift`: stick drift, no fast-fall input.
    AirDrift,
    /// `ftPhysicsApplyAirVelDriftFastFall`
    AirDriftFastFall,
    /// `ftPhysicsApplyAirVelTransNAll`
    AirTransNAll,
    /// `ftPhysicsApplyAirVelTransNYZ`
    #[allow(dead_code)]
    AirTransNYZ,
    /// No `proc_physics`.
    Null,
    /// Anything else the probe saw.
    Other,
}

#[rustfmt::skip]
const ROWS: &[(&str, FighterKind, AnyStatus, C)] = &[
    ("captain.Attack100End", FighterKind::Captain, AnyStatus::Captain(CaptainStatus::Attack100End), C::GroundTransN),
    ("captain.Attack100Loop", FighterKind::Captain, AnyStatus::Captain(CaptainStatus::Attack100Loop), C::GroundTransN),
    ("captain.Attack100Start", FighterKind::Captain, AnyStatus::Captain(CaptainStatus::Attack100Start), C::GroundTransN),
    ("captain.Attack13", FighterKind::Captain, AnyStatus::Captain(CaptainStatus::Attack13), C::GroundTransN),
    ("captain.SpecialHiThrow", FighterKind::Captain, AnyStatus::Captain(CaptainStatus::SpecialHiThrow), C::AirTransNAll),
    ("donkey.SpecialAirNEnd", FighterKind::Donkey, AnyStatus::Donkey(DonkeyStatus::SpecialAirNEnd), C::AirFriction),
    ("donkey.SpecialAirNFull", FighterKind::Donkey, AnyStatus::Donkey(DonkeyStatus::SpecialAirNFull), C::AirFriction),
    ("donkey.SpecialAirNLoop", FighterKind::Donkey, AnyStatus::Donkey(DonkeyStatus::SpecialAirNLoop), C::AirFriction),
    ("donkey.SpecialAirNStart", FighterKind::Donkey, AnyStatus::Donkey(DonkeyStatus::SpecialAirNStart), C::AirFriction),
    ("donkey.SpecialLwEnd", FighterKind::Donkey, AnyStatus::Donkey(DonkeyStatus::SpecialLwEnd), C::GroundFriction),
    ("donkey.SpecialLwLoop", FighterKind::Donkey, AnyStatus::Donkey(DonkeyStatus::SpecialLwLoop), C::GroundFriction),
    ("donkey.SpecialLwStart", FighterKind::Donkey, AnyStatus::Donkey(DonkeyStatus::SpecialLwStart), C::GroundFriction),
    ("donkey.SpecialNEnd", FighterKind::Donkey, AnyStatus::Donkey(DonkeyStatus::SpecialNEnd), C::GroundFriction),
    ("donkey.SpecialNFull", FighterKind::Donkey, AnyStatus::Donkey(DonkeyStatus::SpecialNFull), C::GroundFriction),
    ("donkey.SpecialNLoop", FighterKind::Donkey, AnyStatus::Donkey(DonkeyStatus::SpecialNLoop), C::GroundFriction),
    ("donkey.SpecialNStart", FighterKind::Donkey, AnyStatus::Donkey(DonkeyStatus::SpecialNStart), C::GroundFriction),
    ("donkey.ThrowAirFF", FighterKind::Donkey, AnyStatus::Donkey(DonkeyStatus::ThrowAirFF), C::AirDrift),
    ("donkey.ThrowFF", FighterKind::Donkey, AnyStatus::Donkey(DonkeyStatus::ThrowFF), C::GroundFriction),
    ("donkey.ThrowFFall", FighterKind::Donkey, AnyStatus::Donkey(DonkeyStatus::ThrowFFall), C::AirDriftFastFall),
    ("donkey.ThrowFKneeBend", FighterKind::Donkey, AnyStatus::Donkey(DonkeyStatus::ThrowFKneeBend), C::GroundFriction),
    ("donkey.ThrowFLanding", FighterKind::Donkey, AnyStatus::Donkey(DonkeyStatus::ThrowFLanding), C::GroundFriction),
    ("donkey.ThrowFTurn", FighterKind::Donkey, AnyStatus::Donkey(DonkeyStatus::ThrowFTurn), C::GroundFriction),
    ("donkey.ThrowFWait", FighterKind::Donkey, AnyStatus::Donkey(DonkeyStatus::ThrowFWait), C::GroundFriction),
    ("fox.Attack100End", FighterKind::Fox, AnyStatus::Fox(FoxStatus::Attack100End), C::GroundFriction),
    ("fox.Attack100Loop", FighterKind::Fox, AnyStatus::Fox(FoxStatus::Attack100Loop), C::GroundFriction),
    ("fox.Attack100Start", FighterKind::Fox, AnyStatus::Fox(FoxStatus::Attack100Start), C::GroundFriction),
    ("fox.SpecialAirHiEnd", FighterKind::Fox, AnyStatus::Fox(FoxStatus::SpecialAirHiEnd), C::AirDrift),
    ("fox.SpecialAirN", FighterKind::Fox, AnyStatus::Fox(FoxStatus::SpecialAirN), C::AirDrift),
    ("fox.SpecialHiHold", FighterKind::Fox, AnyStatus::Fox(FoxStatus::SpecialHiHold), C::GroundFriction),
    ("fox.SpecialHiStart", FighterKind::Fox, AnyStatus::Fox(FoxStatus::SpecialHiStart), C::GroundFriction),
    ("fox.SpecialLwEnd", FighterKind::Fox, AnyStatus::Fox(FoxStatus::SpecialLwEnd), C::GroundFriction),
    ("fox.SpecialLwHit", FighterKind::Fox, AnyStatus::Fox(FoxStatus::SpecialLwHit), C::GroundFriction),
    ("fox.SpecialLwLoop", FighterKind::Fox, AnyStatus::Fox(FoxStatus::SpecialLwLoop), C::GroundFriction),
    ("fox.SpecialLwStart", FighterKind::Fox, AnyStatus::Fox(FoxStatus::SpecialLwStart), C::GroundFriction),
    ("fox.SpecialLwTurn", FighterKind::Fox, AnyStatus::Fox(FoxStatus::SpecialLwTurn), C::GroundFriction),
    ("fox.SpecialN", FighterKind::Fox, AnyStatus::Fox(FoxStatus::SpecialN), C::GroundFriction),
    ("kirby.Attack100End", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::Attack100End), C::GroundFriction),
    ("kirby.Attack100Loop", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::Attack100Loop), C::GroundFriction),
    ("kirby.Attack100Start", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::Attack100Start), C::GroundFriction),
    ("kirby.CopyDonkeySpecialAirNEnd", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::CopyDonkeySpecialAirNEnd), C::AirFriction),
    ("kirby.CopyDonkeySpecialAirNFull", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::CopyDonkeySpecialAirNFull), C::AirFriction),
    ("kirby.CopyDonkeySpecialAirNLoop", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::CopyDonkeySpecialAirNLoop), C::AirFriction),
    ("kirby.CopyDonkeySpecialAirNStart", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::CopyDonkeySpecialAirNStart), C::AirFriction),
    ("kirby.CopyDonkeySpecialNEnd", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::CopyDonkeySpecialNEnd), C::GroundFriction),
    ("kirby.CopyDonkeySpecialNFull", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::CopyDonkeySpecialNFull), C::GroundFriction),
    ("kirby.CopyDonkeySpecialNLoop", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::CopyDonkeySpecialNLoop), C::GroundFriction),
    ("kirby.CopyDonkeySpecialNStart", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::CopyDonkeySpecialNStart), C::GroundFriction),
    ("kirby.CopyFoxSpecialAirN", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::CopyFoxSpecialAirN), C::AirDrift),
    ("kirby.CopyFoxSpecialN", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::CopyFoxSpecialN), C::GroundFriction),
    ("kirby.CopyLinkSpecialAirN", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::CopyLinkSpecialAirN), C::AirFriction),
    ("kirby.CopyLinkSpecialAirNEmpty", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::CopyLinkSpecialAirNEmpty), C::AirFriction),
    ("kirby.CopyLinkSpecialAirNReturn", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::CopyLinkSpecialAirNReturn), C::AirFriction),
    ("kirby.CopyLinkSpecialN", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::CopyLinkSpecialN), C::GroundFriction),
    ("kirby.CopyLinkSpecialNEmpty", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::CopyLinkSpecialNEmpty), C::GroundFriction),
    ("kirby.CopyLinkSpecialNGet", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::CopyLinkSpecialNGet), C::GroundFriction),
    ("kirby.CopyLuigiSpecialAirN", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::CopyLuigiSpecialAirN), C::AirDrift),
    ("kirby.CopyLuigiSpecialN", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::CopyLuigiSpecialN), C::GroundFriction),
    ("kirby.CopyMarioSpecialAirN", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::CopyMarioSpecialAirN), C::AirDrift),
    ("kirby.CopyMarioSpecialN", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::CopyMarioSpecialN), C::GroundFriction),
    ("kirby.CopyNessSpecialAirN", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::CopyNessSpecialAirN), C::AirFriction),
    ("kirby.CopyNessSpecialN", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::CopyNessSpecialN), C::GroundFriction),
    ("kirby.CopyPikachuSpecialAirN", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::CopyPikachuSpecialAirN), C::AirFriction),
    ("kirby.CopyPikachuSpecialN", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::CopyPikachuSpecialN), C::GroundFriction),
    ("kirby.CopyPurinSpecialN", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::CopyPurinSpecialN), C::GroundTransN),
    ("kirby.CopySamusSpecialAirNEnd", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::CopySamusSpecialAirNEnd), C::AirFriction),
    ("kirby.CopySamusSpecialAirNStart", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::CopySamusSpecialAirNStart), C::AirDrift),
    ("kirby.CopySamusSpecialNEnd", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::CopySamusSpecialNEnd), C::GroundFriction),
    ("kirby.CopySamusSpecialNLoop", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::CopySamusSpecialNLoop), C::GroundFriction),
    ("kirby.CopySamusSpecialNStart", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::CopySamusSpecialNStart), C::GroundFriction),
    ("kirby.CopyYoshiSpecialAirN", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::CopyYoshiSpecialAirN), C::AirFriction),
    ("kirby.CopyYoshiSpecialAirNCatch", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::CopyYoshiSpecialAirNCatch), C::AirFriction),
    ("kirby.CopyYoshiSpecialAirNRelease", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::CopyYoshiSpecialAirNRelease), C::AirFriction),
    ("kirby.CopyYoshiSpecialN", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::CopyYoshiSpecialN), C::GroundFriction),
    ("kirby.CopyYoshiSpecialNCatch", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::CopyYoshiSpecialNCatch), C::GroundFriction),
    ("kirby.CopyYoshiSpecialNRelease", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::CopyYoshiSpecialNRelease), C::GroundFriction),
    ("kirby.SpecialAirLwEnd", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::SpecialAirLwEnd), C::AirTransNAll),
    ("kirby.SpecialAirLwFall", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::SpecialAirLwFall), C::Null),
    ("kirby.SpecialAirLwHold", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::SpecialAirLwHold), C::Null),
    ("kirby.SpecialAirLwStart", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::SpecialAirLwStart), C::Null),
    ("kirby.SpecialAirNCatch", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::SpecialAirNCatch), C::AirFriction),
    ("kirby.SpecialAirNCopy", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::SpecialAirNCopy), C::AirFriction),
    ("kirby.SpecialAirNEat", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::SpecialAirNEat), C::AirFriction),
    ("kirby.SpecialAirNEnd", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::SpecialAirNEnd), C::AirFriction),
    ("kirby.SpecialAirNLoop", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::SpecialAirNLoop), C::AirFriction),
    ("kirby.SpecialAirNStart", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::SpecialAirNStart), C::AirFriction),
    ("kirby.SpecialAirNThrow", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::SpecialAirNThrow), C::AirTransNAll),
    ("kirby.SpecialLwEnd", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::SpecialLwEnd), C::AirTransNAll),
    ("kirby.SpecialLwStart", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::SpecialLwStart), C::Null),
    ("kirby.SpecialLwUnk", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::SpecialLwUnk), C::GroundFriction),
    ("kirby.SpecialNCatch", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::SpecialNCatch), C::GroundFriction),
    ("kirby.SpecialNCopy", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::SpecialNCopy), C::GroundFriction),
    ("kirby.SpecialNEat", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::SpecialNEat), C::GroundFriction),
    ("kirby.SpecialNEnd", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::SpecialNEnd), C::GroundFriction),
    ("kirby.SpecialNLoop", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::SpecialNLoop), C::GroundFriction),
    ("kirby.SpecialNStart", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::SpecialNStart), C::GroundFriction),
    ("kirby.SpecialNThrow", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::SpecialNThrow), C::GroundTransN),
    ("kirby.SpecialNTurn", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::SpecialNTurn), C::GroundFriction),
    ("kirby.SpecialNWait", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::SpecialNWait), C::GroundFriction),
    ("kirby.ThrowF", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::ThrowF), C::AirTransNAll),
    ("kirby.ThrowFFall", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::ThrowFFall), C::Null),
    ("link.Attack100End", FighterKind::Link, AnyStatus::Link(LinkStatus::Attack100End), C::GroundFriction),
    ("link.Attack100Loop", FighterKind::Link, AnyStatus::Link(LinkStatus::Attack100Loop), C::GroundFriction),
    ("link.Attack100Start", FighterKind::Link, AnyStatus::Link(LinkStatus::Attack100Start), C::GroundFriction),
    ("link.Attack13", FighterKind::Link, AnyStatus::Link(LinkStatus::Attack13), C::GroundFriction),
    ("link.SpecialAirLw", FighterKind::Link, AnyStatus::Link(LinkStatus::SpecialAirLw), C::AirFriction),
    ("link.SpecialAirN", FighterKind::Link, AnyStatus::Link(LinkStatus::SpecialAirN), C::AirFriction),
    ("link.SpecialAirNEmpty", FighterKind::Link, AnyStatus::Link(LinkStatus::SpecialAirNEmpty), C::AirFriction),
    ("link.SpecialAirNReturn", FighterKind::Link, AnyStatus::Link(LinkStatus::SpecialAirNReturn), C::AirFriction),
    ("link.SpecialHiEnd", FighterKind::Link, AnyStatus::Link(LinkStatus::SpecialHiEnd), C::GroundFriction),
    ("link.SpecialLw", FighterKind::Link, AnyStatus::Link(LinkStatus::SpecialLw), C::GroundFriction),
    ("link.SpecialN", FighterKind::Link, AnyStatus::Link(LinkStatus::SpecialN), C::GroundFriction),
    ("link.SpecialNEmpty", FighterKind::Link, AnyStatus::Link(LinkStatus::SpecialNEmpty), C::GroundFriction),
    ("link.SpecialNGet", FighterKind::Link, AnyStatus::Link(LinkStatus::SpecialNGet), C::GroundFriction),
    ("luigi.Attack13", FighterKind::Luigi, AnyStatus::Mario(MarioStatus::Attack13), C::GroundFriction),
    ("luigi.SpecialAirN", FighterKind::Luigi, AnyStatus::Mario(MarioStatus::SpecialAirN), C::AirDrift),
    ("luigi.SpecialN", FighterKind::Luigi, AnyStatus::Mario(MarioStatus::SpecialN), C::GroundFriction),
    ("mario.Attack13", FighterKind::Mario, AnyStatus::Mario(MarioStatus::Attack13), C::GroundFriction),
    ("mario.SpecialAirN", FighterKind::Mario, AnyStatus::Mario(MarioStatus::SpecialAirN), C::AirDrift),
    ("mario.SpecialN", FighterKind::Mario, AnyStatus::Mario(MarioStatus::SpecialN), C::GroundFriction),
    ("ness.Attack13", FighterKind::Ness, AnyStatus::Ness(NessStatus::Attack13), C::GroundFriction),
    ("ness.SpecialAirN", FighterKind::Ness, AnyStatus::Ness(NessStatus::SpecialAirN), C::AirFriction),
    ("ness.SpecialHiEnd", FighterKind::Ness, AnyStatus::Ness(NessStatus::SpecialHiEnd), C::GroundFriction),
    ("ness.SpecialHiHold", FighterKind::Ness, AnyStatus::Ness(NessStatus::SpecialHiHold), C::GroundFriction),
    ("ness.SpecialLwEnd", FighterKind::Ness, AnyStatus::Ness(NessStatus::SpecialLwEnd), C::GroundFriction),
    ("ness.SpecialLwHit", FighterKind::Ness, AnyStatus::Ness(NessStatus::SpecialLwHit), C::GroundFriction),
    ("ness.SpecialLwHold", FighterKind::Ness, AnyStatus::Ness(NessStatus::SpecialLwHold), C::GroundFriction),
    ("ness.SpecialN", FighterKind::Ness, AnyStatus::Ness(NessStatus::SpecialN), C::GroundFriction),
    ("pikachu.SpecialAirN", FighterKind::Pikachu, AnyStatus::Pikachu(PikachuStatus::SpecialAirN), C::AirFriction),
    ("pikachu.SpecialN", FighterKind::Pikachu, AnyStatus::Pikachu(PikachuStatus::SpecialN), C::GroundFriction),
    ("purin.SpecialAirHi", FighterKind::Purin, AnyStatus::Purin(PurinStatus::SpecialAirHi), C::AirFriction),
    ("purin.SpecialAirLw", FighterKind::Purin, AnyStatus::Purin(PurinStatus::SpecialAirLw), C::AirFriction),
    ("purin.SpecialHi", FighterKind::Purin, AnyStatus::Purin(PurinStatus::SpecialHi), C::GroundFriction),
    ("purin.SpecialLw", FighterKind::Purin, AnyStatus::Purin(PurinStatus::SpecialLw), C::GroundFriction),
    ("purin.SpecialN", FighterKind::Purin, AnyStatus::Purin(PurinStatus::SpecialN), C::GroundTransN),
    ("samus.SpecialAirNEnd", FighterKind::Samus, AnyStatus::Samus(SamusStatus::SpecialAirNEnd), C::AirFriction),
    ("samus.SpecialAirNStart", FighterKind::Samus, AnyStatus::Samus(SamusStatus::SpecialAirNStart), C::AirDrift),
    ("samus.SpecialNEnd", FighterKind::Samus, AnyStatus::Samus(SamusStatus::SpecialNEnd), C::GroundFriction),
    ("samus.SpecialNLoop", FighterKind::Samus, AnyStatus::Samus(SamusStatus::SpecialNLoop), C::GroundFriction),
    ("samus.SpecialNStart", FighterKind::Samus, AnyStatus::Samus(SamusStatus::SpecialNStart), C::GroundFriction),
    ("yoshi.SpecialAirLwStart", FighterKind::Yoshi, AnyStatus::Yoshi(YoshiStatus::SpecialAirLwStart), C::AirTransNAll),
    ("yoshi.SpecialAirN", FighterKind::Yoshi, AnyStatus::Yoshi(YoshiStatus::SpecialAirN), C::AirFriction),
    ("yoshi.SpecialAirNCatch", FighterKind::Yoshi, AnyStatus::Yoshi(YoshiStatus::SpecialAirNCatch), C::AirFriction),
    ("yoshi.SpecialAirNRelease", FighterKind::Yoshi, AnyStatus::Yoshi(YoshiStatus::SpecialAirNRelease), C::AirFriction),
    ("yoshi.SpecialLwLanding", FighterKind::Yoshi, AnyStatus::Yoshi(YoshiStatus::SpecialLwLanding), C::GroundFriction),
    ("yoshi.SpecialLwStart", FighterKind::Yoshi, AnyStatus::Yoshi(YoshiStatus::SpecialLwStart), C::AirTransNAll),
    ("yoshi.SpecialN", FighterKind::Yoshi, AnyStatus::Yoshi(YoshiStatus::SpecialN), C::GroundFriction),
    ("yoshi.SpecialNCatch", FighterKind::Yoshi, AnyStatus::Yoshi(YoshiStatus::SpecialNCatch), C::GroundFriction),
    ("yoshi.SpecialNRelease", FighterKind::Yoshi, AnyStatus::Yoshi(YoshiStatus::SpecialNRelease), C::GroundFriction),
    // Entries whose table comment carries a note after the name.
    ("ness.SpecialAirHiBound", FighterKind::Ness, AnyStatus::Ness(NessStatus::SpecialAirHiBound), C::AirFriction),
    ("pikachu.SpecialLwStart", FighterKind::Pikachu, AnyStatus::Pikachu(PikachuStatus::SpecialLwStart), C::GroundFriction),
    ("pikachu.SpecialLwLoop", FighterKind::Pikachu, AnyStatus::Pikachu(PikachuStatus::SpecialLwLoop), C::GroundFriction),
    ("pikachu.SpecialLwHit", FighterKind::Pikachu, AnyStatus::Pikachu(PikachuStatus::SpecialLwHit), C::GroundFriction),
    // Status 227, labelled a second `SpecialLwHit` in the table.
    ("pikachu.SpecialLwEnd", FighterKind::Pikachu, AnyStatus::Pikachu(PikachuStatus::SpecialLwEnd), C::GroundFriction),
    ("pikachu.SpecialAirLwStart", FighterKind::Pikachu, AnyStatus::Pikachu(PikachuStatus::SpecialAirLwStart), C::AirFriction),
    ("pikachu.SpecialAirLwLoop", FighterKind::Pikachu, AnyStatus::Pikachu(PikachuStatus::SpecialAirLwLoop), C::AirFriction),
    ("pikachu.SpecialAirLwEnd", FighterKind::Pikachu, AnyStatus::Pikachu(PikachuStatus::SpecialAirLwEnd), C::AirFriction),
    ("pikachu.SpecialHiStart", FighterKind::Pikachu, AnyStatus::Pikachu(PikachuStatus::SpecialHiStart), C::GroundFriction),
];

fn probe(
    kind: FighterKind,
    status: AnyStatus,
    grounded: bool,
    stick_x: i8,
    delta: Vec3,
) -> Fighter {
    probe_with(kind, status, grounded, stick_x, delta, false)
}

fn probe_with(
    kind: FighterKind,
    status: AnyStatus,
    grounded: bool,
    stick_x: i8,
    delta: Vec3,
    fastfall: bool,
) -> Fighter {
    let mut f = Fighter::new(kind, 0, 3);
    f.physics.is_fastfall = fastfall;
    f.status.status = status;
    if grounded {
        f.situation = Situation::Ground;
        f.floor = Some(Standing {
            line: 0,
            flags: 0,
            normal: ssb_engine::math::Vec2::new(0.0, 1.0),
        });
        f.physics.vel_ground = Vec3::new(10.0, 0.0, 0.0);
    } else {
        f.situation = Situation::Air;
        f.floor = None;
        f.physics.vel_air = Vec3::new(20.0, -5.0, 0.0);
    }
    f.input.stick_x = stick_x;
    f.stick.x = stick_x;
    f.stick.y = -80;
    f.input.stick_y = -80;
    f.stick.tap_y = 0;
    f.set_root_motion(RootMotion {
        delta,
        ..RootMotion::default()
    });
    if grounded {
        f.ground_physics();
    } else {
        f.air_physics();
    }
    f
}

fn classify(kind: FighterKind, status: AnyStatus, grounded: bool) -> C {
    let a = probe(kind, status, grounded, 80, Vec3::new(3.0, 4.0, 5.0));
    let b = probe(kind, status, grounded, -80, Vec3::new(3.0, 4.0, 5.0));
    let c = probe(kind, status, grounded, 80, Vec3::new(7.0, 9.0, 11.0));
    if grounded {
        let x = a.physics.vel_ground.x;
        if x != c.physics.vel_ground.x {
            C::GroundTransN
        } else if x == 10.0 {
            C::Null
        } else if (0.0..10.0).contains(&x) && x == b.physics.vel_ground.x {
            C::GroundFriction
        } else {
            C::Other
        }
    } else {
        let v = a.physics.vel_air;
        if v != c.physics.vel_air {
            C::AirTransNAll
        } else if v == Vec3::new(20.0, -5.0, 0.0) {
            C::Null
        } else {
            // A fast fall already under way keeps falling fast
            // (`fp->is_fastfall ? ftPhysicsApplyFastFall : ...`).
            let fast = probe_with(kind, status, false, 0, Vec3::new(3.0, 4.0, 5.0), true);
            let mut want = crate::physics::PhysicsState {
                vel_air: Vec3::new(20.0, -5.0, 0.0),
                ..crate::physics::PhysicsState::default()
            };
            crate::physics::apply_fast_fall(&mut want, &fast.attributes);
            if fast.physics.vel_air.y != want.vel_air.y {
                return C::Other;
            }
            match (v.x != b.physics.vel_air.x, a.physics.is_fastfall) {
                (false, false) => C::AirFriction,
                (true, false) => C::AirDrift,
                (true, true) => C::AirDriftFastFall,
                (false, true) => C::Other,
            }
        }
    }
}

#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn every_generic_character_proc_physics_matches_its_status_table() {
    let mut bad = Vec::new();
    for &(name, kind, status, want) in ROWS {
        let grounded = status.is_grounded();
        let got = std::panic::catch_unwind(|| classify(kind, status, grounded));
        match got {
            Ok(got) if got == want => {}
            Ok(got) => bad.push(format!(
                "{name} ({}): want {want:?}, got {got:?}",
                if grounded { "ground" } else { "air" }
            )),
            Err(_) => bad.push(format!("{name}: panicked")),
        }
    }
    assert!(
        bad.is_empty(),
        "{} mismatches:\n{}",
        bad.len(),
        bad.join("\n")
    );
}

/// Every character status's `proc_physics`, as its table names it.
#[rustfmt::skip]
const ALL: &[(&str, FighterKind, AnyStatus, &str)] = &[
    ("captain.Attack13", FighterKind::Captain, AnyStatus::Captain(CaptainStatus::Attack13), "ftPhysicsApplyGroundVelTransN"),
    ("captain.Attack100Start", FighterKind::Captain, AnyStatus::Captain(CaptainStatus::Attack100Start), "ftPhysicsApplyGroundVelTransN"),
    ("captain.Attack100Loop", FighterKind::Captain, AnyStatus::Captain(CaptainStatus::Attack100Loop), "ftPhysicsApplyGroundVelTransN"),
    ("captain.Attack100End", FighterKind::Captain, AnyStatus::Captain(CaptainStatus::Attack100End), "ftPhysicsApplyGroundVelTransN"),
    ("captain.AppearRStart", FighterKind::Captain, AnyStatus::Captain(CaptainStatus::AppearRStart), "ftCommonAppearProcPhysics"),
    ("captain.AppearLStart", FighterKind::Captain, AnyStatus::Captain(CaptainStatus::AppearLStart), "ftCommonAppearProcPhysics"),
    ("captain.AppearREnd", FighterKind::Captain, AnyStatus::Captain(CaptainStatus::AppearREnd), "ftCommonAppearProcPhysics"),
    ("captain.AppearLEnd", FighterKind::Captain, AnyStatus::Captain(CaptainStatus::AppearLEnd), "ftCommonAppearProcPhysics"),
    ("captain.SpecialN", FighterKind::Captain, AnyStatus::Captain(CaptainStatus::SpecialN), "ftCaptainSpecialNProcPhysics"),
    ("captain.SpecialAirN", FighterKind::Captain, AnyStatus::Captain(CaptainStatus::SpecialAirN), "ftCaptainSpecialAirNProcPhysics"),
    ("captain.SpecialLw", FighterKind::Captain, AnyStatus::Captain(CaptainStatus::SpecialLw), "ftCaptainSpecialLwProcPhysics"),
    ("captain.SpecialLwAir", FighterKind::Captain, AnyStatus::Captain(CaptainStatus::SpecialLwAir), "ftCaptainSpecialLwProcPhysics"),
    ("captain.SpecialLwLanding", FighterKind::Captain, AnyStatus::Captain(CaptainStatus::SpecialLwLanding), "ftCaptainSpecialLwLandingProcPhysics"),
    ("captain.SpecialAirLw", FighterKind::Captain, AnyStatus::Captain(CaptainStatus::SpecialAirLw), "ftCaptainSpecialAirLwProcPhysics"),
    ("captain.SpecialLwBound", FighterKind::Captain, AnyStatus::Captain(CaptainStatus::SpecialLwBound), "ftCaptainSpecialLwBoundProcPhysics"),
    ("captain.SpecialHi", FighterKind::Captain, AnyStatus::Captain(CaptainStatus::SpecialHi), "ftCaptainSpecialHiProcPhysics"),
    ("captain.SpecialHiCatch", FighterKind::Captain, AnyStatus::Captain(CaptainStatus::SpecialHiCatch), "ftCaptainSpecialHiCatchProcPhysics"),
    ("captain.SpecialHiThrow", FighterKind::Captain, AnyStatus::Captain(CaptainStatus::SpecialHiThrow), "ftPhysicsApplyAirVelTransNAll"),
    ("captain.SpecialAirHi", FighterKind::Captain, AnyStatus::Captain(CaptainStatus::SpecialAirHi), "ftCaptainSpecialHiProcPhysics"),
    ("donkey.AppearR", FighterKind::Donkey, AnyStatus::Donkey(DonkeyStatus::AppearR), "ftCommonAppearProcPhysics"),
    ("donkey.AppearL", FighterKind::Donkey, AnyStatus::Donkey(DonkeyStatus::AppearL), "ftCommonAppearProcPhysics"),
    ("donkey.SpecialNStart", FighterKind::Donkey, AnyStatus::Donkey(DonkeyStatus::SpecialNStart), "ftPhysicsApplyGroundVelFriction"),
    ("donkey.SpecialAirNStart", FighterKind::Donkey, AnyStatus::Donkey(DonkeyStatus::SpecialAirNStart), "ftPhysicsApplyAirVelFriction"),
    ("donkey.SpecialNLoop", FighterKind::Donkey, AnyStatus::Donkey(DonkeyStatus::SpecialNLoop), "ftPhysicsApplyGroundVelFriction"),
    ("donkey.SpecialAirNLoop", FighterKind::Donkey, AnyStatus::Donkey(DonkeyStatus::SpecialAirNLoop), "ftPhysicsApplyAirVelFriction"),
    ("donkey.SpecialNEnd", FighterKind::Donkey, AnyStatus::Donkey(DonkeyStatus::SpecialNEnd), "ftPhysicsApplyGroundVelFriction"),
    ("donkey.SpecialAirNEnd", FighterKind::Donkey, AnyStatus::Donkey(DonkeyStatus::SpecialAirNEnd), "ftPhysicsApplyAirVelFriction"),
    ("donkey.SpecialNFull", FighterKind::Donkey, AnyStatus::Donkey(DonkeyStatus::SpecialNFull), "ftPhysicsApplyGroundVelFriction"),
    ("donkey.SpecialAirNFull", FighterKind::Donkey, AnyStatus::Donkey(DonkeyStatus::SpecialAirNFull), "ftPhysicsApplyAirVelFriction"),
    ("donkey.SpecialHi", FighterKind::Donkey, AnyStatus::Donkey(DonkeyStatus::SpecialHi), "ftDonkeySpecialHiProcPhysics"),
    ("donkey.SpecialAirHi", FighterKind::Donkey, AnyStatus::Donkey(DonkeyStatus::SpecialAirHi), "ftDonkeySpecialAirHiProcPhysics"),
    ("donkey.SpecialLwStart", FighterKind::Donkey, AnyStatus::Donkey(DonkeyStatus::SpecialLwStart), "ftPhysicsApplyGroundVelFriction"),
    ("donkey.SpecialLwLoop", FighterKind::Donkey, AnyStatus::Donkey(DonkeyStatus::SpecialLwLoop), "ftPhysicsApplyGroundVelFriction"),
    ("donkey.SpecialLwEnd", FighterKind::Donkey, AnyStatus::Donkey(DonkeyStatus::SpecialLwEnd), "ftPhysicsApplyGroundVelFriction"),
    ("donkey.ThrowFWait", FighterKind::Donkey, AnyStatus::Donkey(DonkeyStatus::ThrowFWait), "ftPhysicsApplyGroundVelFriction"),
    ("donkey.ThrowFWalkSlow", FighterKind::Donkey, AnyStatus::Donkey(DonkeyStatus::ThrowFWalkSlow), "ftCommonWalkProcPhysics"),
    ("donkey.ThrowFWalkMiddle", FighterKind::Donkey, AnyStatus::Donkey(DonkeyStatus::ThrowFWalkMiddle), "ftCommonWalkProcPhysics"),
    ("donkey.ThrowFWalkFast", FighterKind::Donkey, AnyStatus::Donkey(DonkeyStatus::ThrowFWalkFast), "ftCommonWalkProcPhysics"),
    ("donkey.ThrowFTurn", FighterKind::Donkey, AnyStatus::Donkey(DonkeyStatus::ThrowFTurn), "ftPhysicsApplyGroundVelFriction"),
    ("donkey.ThrowFKneeBend", FighterKind::Donkey, AnyStatus::Donkey(DonkeyStatus::ThrowFKneeBend), "ftPhysicsApplyGroundVelFriction"),
    ("donkey.ThrowFFall", FighterKind::Donkey, AnyStatus::Donkey(DonkeyStatus::ThrowFFall), "ftPhysicsApplyAirVelDriftFastFall"),
    ("donkey.ThrowFLanding", FighterKind::Donkey, AnyStatus::Donkey(DonkeyStatus::ThrowFLanding), "ftPhysicsApplyGroundVelFriction"),
    ("donkey.ThrowFDamage", FighterKind::Donkey, AnyStatus::Donkey(DonkeyStatus::ThrowFDamage), "ftCommonDamageCommonProcPhysics"),
    ("donkey.ThrowFF", FighterKind::Donkey, AnyStatus::Donkey(DonkeyStatus::ThrowFF), "ftPhysicsApplyGroundVelFriction"),
    ("donkey.ThrowAirFF", FighterKind::Donkey, AnyStatus::Donkey(DonkeyStatus::ThrowAirFF), "ftPhysicsApplyAirVelDrift"),
    ("donkey.HeavyThrowF", FighterKind::Donkey, AnyStatus::Donkey(DonkeyStatus::HeavyThrowF), "ftCommonItemThrowProcPhysics"),
    ("donkey.HeavyThrowB", FighterKind::Donkey, AnyStatus::Donkey(DonkeyStatus::HeavyThrowB), "ftCommonItemThrowProcPhysics"),
    ("donkey.HeavyThrowF4", FighterKind::Donkey, AnyStatus::Donkey(DonkeyStatus::HeavyThrowF4), "ftCommonItemThrowProcPhysics"),
    ("donkey.HeavyThrowB4", FighterKind::Donkey, AnyStatus::Donkey(DonkeyStatus::HeavyThrowB4), "ftCommonItemThrowProcPhysics"),
    ("fox.Attack100Start", FighterKind::Fox, AnyStatus::Fox(FoxStatus::Attack100Start), "ftPhysicsApplyGroundVelFriction"),
    ("fox.Attack100Loop", FighterKind::Fox, AnyStatus::Fox(FoxStatus::Attack100Loop), "ftPhysicsApplyGroundVelFriction"),
    ("fox.Attack100End", FighterKind::Fox, AnyStatus::Fox(FoxStatus::Attack100End), "ftPhysicsApplyGroundVelFriction"),
    ("fox.AppearR", FighterKind::Fox, AnyStatus::Fox(FoxStatus::AppearR), "ftCommonAppearProcPhysics"),
    ("fox.AppearL", FighterKind::Fox, AnyStatus::Fox(FoxStatus::AppearL), "ftCommonAppearProcPhysics"),
    ("fox.SpecialN", FighterKind::Fox, AnyStatus::Fox(FoxStatus::SpecialN), "ftPhysicsApplyGroundVelFriction"),
    ("fox.SpecialAirN", FighterKind::Fox, AnyStatus::Fox(FoxStatus::SpecialAirN), "ftPhysicsApplyAirVelDrift"),
    ("fox.SpecialHiStart", FighterKind::Fox, AnyStatus::Fox(FoxStatus::SpecialHiStart), "ftPhysicsApplyGroundVelFriction"),
    ("fox.SpecialAirHiStart", FighterKind::Fox, AnyStatus::Fox(FoxStatus::SpecialAirHiStart), "ftFoxSpecialAirHiStartProcPhysics"),
    ("fox.SpecialHiHold", FighterKind::Fox, AnyStatus::Fox(FoxStatus::SpecialHiHold), "ftPhysicsApplyGroundVelFriction"),
    ("fox.SpecialAirHiHold", FighterKind::Fox, AnyStatus::Fox(FoxStatus::SpecialAirHiHold), "ftFoxSpecialAirHiStartProcPhysics"),
    ("fox.SpecialHi", FighterKind::Fox, AnyStatus::Fox(FoxStatus::SpecialHi), "ftFoxSpecialHiProcPhysics"),
    ("fox.SpecialAirHi", FighterKind::Fox, AnyStatus::Fox(FoxStatus::SpecialAirHi), "ftFoxSpecialAirHiProcPhysics"),
    ("fox.SpecialHiEnd", FighterKind::Fox, AnyStatus::Fox(FoxStatus::SpecialHiEnd), "ftFoxSpecialHiEndProcPhysics"),
    ("fox.SpecialAirHiEnd", FighterKind::Fox, AnyStatus::Fox(FoxStatus::SpecialAirHiEnd), "ftPhysicsApplyAirVelDrift"),
    ("fox.SpecialAirHiBound", FighterKind::Fox, AnyStatus::Fox(FoxStatus::SpecialAirHiBound), "ftFoxSpecialAirHiBoundProcPhysics"),
    ("fox.SpecialLwStart", FighterKind::Fox, AnyStatus::Fox(FoxStatus::SpecialLwStart), "ftPhysicsApplyGroundVelFriction"),
    ("fox.SpecialLwHit", FighterKind::Fox, AnyStatus::Fox(FoxStatus::SpecialLwHit), "ftPhysicsApplyGroundVelFriction"),
    ("fox.SpecialLwEnd", FighterKind::Fox, AnyStatus::Fox(FoxStatus::SpecialLwEnd), "ftPhysicsApplyGroundVelFriction"),
    ("fox.SpecialLwLoop", FighterKind::Fox, AnyStatus::Fox(FoxStatus::SpecialLwLoop), "ftPhysicsApplyGroundVelFriction"),
    ("fox.SpecialLwTurn", FighterKind::Fox, AnyStatus::Fox(FoxStatus::SpecialLwTurn), "ftPhysicsApplyGroundVelFriction"),
    ("fox.SpecialAirLwStart", FighterKind::Fox, AnyStatus::Fox(FoxStatus::SpecialAirLwStart), "ftFoxSpecialAirLwCommonProcPhysics"),
    ("fox.SpecialAirLwHit", FighterKind::Fox, AnyStatus::Fox(FoxStatus::SpecialAirLwHit), "ftFoxSpecialAirLwCommonProcPhysics"),
    ("fox.SpecialAirLwEnd", FighterKind::Fox, AnyStatus::Fox(FoxStatus::SpecialAirLwEnd), "ftFoxSpecialAirLwCommonProcPhysics"),
    ("fox.SpecialAirLwLoop", FighterKind::Fox, AnyStatus::Fox(FoxStatus::SpecialAirLwLoop), "ftFoxSpecialAirLwCommonProcPhysics"),
    ("fox.SpecialAirLwTurn", FighterKind::Fox, AnyStatus::Fox(FoxStatus::SpecialAirLwTurn), "ftFoxSpecialAirLwCommonProcPhysics"),
    ("kirby.Attack100Start", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::Attack100Start), "ftPhysicsApplyGroundVelFriction"),
    ("kirby.Attack100Loop", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::Attack100Loop), "ftPhysicsApplyGroundVelFriction"),
    ("kirby.Attack100End", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::Attack100End), "ftPhysicsApplyGroundVelFriction"),
    ("kirby.JumpAerialF1", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::JumpAerialF1), "ftCommonJumpAerialProcPhysics"),
    ("kirby.JumpAerialF2", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::JumpAerialF2), "ftCommonJumpAerialProcPhysics"),
    ("kirby.JumpAerialF3", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::JumpAerialF3), "ftCommonJumpAerialProcPhysics"),
    ("kirby.JumpAerialF4", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::JumpAerialF4), "ftCommonJumpAerialProcPhysics"),
    ("kirby.JumpAerialF5", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::JumpAerialF5), "ftCommonJumpAerialProcPhysics"),
    ("kirby.ThrowF", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::ThrowF), "ftPhysicsApplyAirVelTransNAll"),
    ("kirby.ThrowFFall", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::ThrowFFall), "NULL"),
    ("kirby.ThrowFLanding", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::ThrowFLanding), "ftKirbyThrowFLandingProcPhysics"),
    ("kirby.CopyMarioSpecialN", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::CopyMarioSpecialN), "ftPhysicsApplyGroundVelFriction"),
    ("kirby.CopyMarioSpecialAirN", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::CopyMarioSpecialAirN), "ftPhysicsApplyAirVelDrift"),
    ("kirby.CopyLuigiSpecialN", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::CopyLuigiSpecialN), "ftPhysicsApplyGroundVelFriction"),
    ("kirby.CopyLuigiSpecialAirN", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::CopyLuigiSpecialAirN), "ftPhysicsApplyAirVelDrift"),
    ("kirby.CopyFoxSpecialN", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::CopyFoxSpecialN), "ftPhysicsApplyGroundVelFriction"),
    ("kirby.CopyFoxSpecialAirN", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::CopyFoxSpecialAirN), "ftPhysicsApplyAirVelDrift"),
    ("kirby.CopySamusSpecialNStart", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::CopySamusSpecialNStart), "ftPhysicsApplyGroundVelFriction"),
    ("kirby.CopySamusSpecialNLoop", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::CopySamusSpecialNLoop), "ftPhysicsApplyGroundVelFriction"),
    ("kirby.CopySamusSpecialNEnd", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::CopySamusSpecialNEnd), "ftPhysicsApplyGroundVelFriction"),
    ("kirby.CopySamusSpecialAirNStart", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::CopySamusSpecialAirNStart), "ftPhysicsApplyAirVelDrift"),
    ("kirby.CopySamusSpecialAirNEnd", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::CopySamusSpecialAirNEnd), "ftPhysicsApplyAirVelFriction"),
    ("kirby.CopyDonkeySpecialNStart", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::CopyDonkeySpecialNStart), "ftPhysicsApplyGroundVelFriction"),
    ("kirby.CopyDonkeySpecialAirNStart", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::CopyDonkeySpecialAirNStart), "ftPhysicsApplyAirVelFriction"),
    ("kirby.CopyDonkeySpecialNLoop", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::CopyDonkeySpecialNLoop), "ftPhysicsApplyGroundVelFriction"),
    ("kirby.CopyDonkeySpecialAirNLoop", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::CopyDonkeySpecialAirNLoop), "ftPhysicsApplyAirVelFriction"),
    ("kirby.CopyDonkeySpecialNEnd", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::CopyDonkeySpecialNEnd), "ftPhysicsApplyGroundVelFriction"),
    ("kirby.CopyDonkeySpecialAirNEnd", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::CopyDonkeySpecialAirNEnd), "ftPhysicsApplyAirVelFriction"),
    ("kirby.CopyDonkeySpecialNFull", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::CopyDonkeySpecialNFull), "ftPhysicsApplyGroundVelFriction"),
    ("kirby.CopyDonkeySpecialAirNFull", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::CopyDonkeySpecialAirNFull), "ftPhysicsApplyAirVelFriction"),
    ("kirby.AppearR", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::AppearR), "ftCommonAppearProcPhysics"),
    ("kirby.AppearL", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::AppearL), "ftCommonAppearProcPhysics"),
    ("kirby.CopyPikachuSpecialN", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::CopyPikachuSpecialN), "ftPhysicsApplyGroundVelFriction"),
    ("kirby.CopyPikachuSpecialAirN", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::CopyPikachuSpecialAirN), "ftPhysicsApplyAirVelFriction"),
    ("kirby.CopyNessSpecialN", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::CopyNessSpecialN), "ftPhysicsApplyGroundVelFriction"),
    ("kirby.CopyNessSpecialAirN", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::CopyNessSpecialAirN), "ftPhysicsApplyAirVelFriction"),
    ("kirby.SpecialHi", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::SpecialHi), "ftKirbySpecialHiProcPhysics"),
    ("kirby.SpecialHiLanding", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::SpecialHiLanding), "ftKirbySpecialHiLandingProcPhysics"),
    ("kirby.SpecialAirHi", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::SpecialAirHi), "ftKirbySpecialAirHiProcPhysics"),
    ("kirby.SpecialAirHiFall", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::SpecialAirHiFall), "ftKirbySpecialAirHiFallProcPhysics"),
    ("kirby.SpecialLwStart", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::SpecialLwStart), "NULL"),
    ("kirby.SpecialLwUnk", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::SpecialLwUnk), "ftPhysicsApplyGroundVelFriction"),
    ("kirby.SpecialLwHold", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::SpecialLwHold), "ftKirbySpecialLwHoldProcPhysics"),
    ("kirby.SpecialLwEnd", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::SpecialLwEnd), "ftPhysicsApplyAirVelTransNAll"),
    ("kirby.SpecialAirLwStart", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::SpecialAirLwStart), "NULL"),
    ("kirby.SpecialAirLwHold", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::SpecialAirLwHold), "NULL"),
    ("kirby.SpecialAirLwLanding", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::SpecialAirLwLanding), "ftKirbySpecialLwHoldProcPhysics"),
    ("kirby.SpecialAirLwFall", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::SpecialAirLwFall), "NULL"),
    ("kirby.SpecialAirLwEnd", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::SpecialAirLwEnd), "ftPhysicsApplyAirVelTransNAll"),
    ("kirby.SpecialNStart", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::SpecialNStart), "ftPhysicsApplyGroundVelFriction"),
    ("kirby.SpecialNLoop", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::SpecialNLoop), "ftPhysicsApplyGroundVelFriction"),
    ("kirby.SpecialNEnd", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::SpecialNEnd), "ftPhysicsApplyGroundVelFriction"),
    ("kirby.SpecialNCatch", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::SpecialNCatch), "ftPhysicsApplyGroundVelFriction"),
    ("kirby.SpecialNEat", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::SpecialNEat), "ftPhysicsApplyGroundVelFriction"),
    ("kirby.SpecialNThrow", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::SpecialNThrow), "ftPhysicsApplyGroundVelTransN"),
    ("kirby.SpecialNWait", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::SpecialNWait), "ftPhysicsApplyGroundVelFriction"),
    ("kirby.SpecialNTurn", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::SpecialNTurn), "ftPhysicsApplyGroundVelFriction"),
    ("kirby.SpecialNCopy", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::SpecialNCopy), "ftPhysicsApplyGroundVelFriction"),
    ("kirby.SpecialAirNStart", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::SpecialAirNStart), "ftPhysicsApplyAirVelFriction"),
    ("kirby.SpecialAirNLoop", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::SpecialAirNLoop), "ftPhysicsApplyAirVelFriction"),
    ("kirby.SpecialAirNEnd", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::SpecialAirNEnd), "ftPhysicsApplyAirVelFriction"),
    ("kirby.SpecialAirNCatch", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::SpecialAirNCatch), "ftPhysicsApplyAirVelFriction"),
    ("kirby.SpecialAirNEat", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::SpecialAirNEat), "ftPhysicsApplyAirVelFriction"),
    ("kirby.SpecialAirNThrow", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::SpecialAirNThrow), "ftPhysicsApplyAirVelTransNAll"),
    ("kirby.SpecialAirNWait", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::SpecialAirNWait), "ftKirbySpecialAirNWaitProcPhysics"),
    ("kirby.SpecialAirNTurn", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::SpecialAirNTurn), "ftKirbySpecialAirNWaitProcPhysics"),
    ("kirby.SpecialAirNCopy", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::SpecialAirNCopy), "ftPhysicsApplyAirVelFriction"),
    ("kirby.CopyLinkSpecialN", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::CopyLinkSpecialN), "ftPhysicsApplyGroundVelFriction"),
    ("kirby.CopyLinkSpecialNGet", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::CopyLinkSpecialNGet), "ftPhysicsApplyGroundVelFriction"),
    ("kirby.CopyLinkSpecialNEmpty", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::CopyLinkSpecialNEmpty), "ftPhysicsApplyGroundVelFriction"),
    ("kirby.CopyLinkSpecialAirN", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::CopyLinkSpecialAirN), "ftPhysicsApplyAirVelFriction"),
    ("kirby.CopyLinkSpecialAirNReturn", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::CopyLinkSpecialAirNReturn), "ftPhysicsApplyAirVelFriction"),
    ("kirby.CopyLinkSpecialAirNEmpty", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::CopyLinkSpecialAirNEmpty), "ftPhysicsApplyAirVelFriction"),
    ("kirby.CopyPurinSpecialN", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::CopyPurinSpecialN), "ftPhysicsApplyGroundVelTransN"),
    ("kirby.CopyPurinSpecialAirN", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::CopyPurinSpecialAirN), "ftKirbyCopyPurinSpecialAirNProcPhysics"),
    ("kirby.CopyCaptainSpecialN", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::CopyCaptainSpecialN), "ftKirbyCopyCaptainSpecialNProcPhysics"),
    ("kirby.CopyCaptainSpecialAirN", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::CopyCaptainSpecialAirN), "ftKirbyCopyCaptainSpecialAirNProcPhysics"),
    ("kirby.CopyYoshiSpecialN", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::CopyYoshiSpecialN), "ftPhysicsApplyGroundVelFriction"),
    ("kirby.CopyYoshiSpecialNCatch", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::CopyYoshiSpecialNCatch), "ftPhysicsApplyGroundVelFriction"),
    ("kirby.CopyYoshiSpecialNRelease", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::CopyYoshiSpecialNRelease), "ftPhysicsApplyGroundVelFriction"),
    ("kirby.CopyYoshiSpecialAirN", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::CopyYoshiSpecialAirN), "ftPhysicsApplyAirVelFriction"),
    ("kirby.CopyYoshiSpecialAirNCatch", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::CopyYoshiSpecialAirNCatch), "ftPhysicsApplyAirVelFriction"),
    ("kirby.CopyYoshiSpecialAirNRelease", FighterKind::Kirby, AnyStatus::Kirby(KirbyStatus::CopyYoshiSpecialAirNRelease), "ftPhysicsApplyAirVelFriction"),
    ("link.Attack13", FighterKind::Link, AnyStatus::Link(LinkStatus::Attack13), "ftPhysicsApplyGroundVelFriction"),
    ("link.Attack100Start", FighterKind::Link, AnyStatus::Link(LinkStatus::Attack100Start), "ftPhysicsApplyGroundVelFriction"),
    ("link.Attack100Loop", FighterKind::Link, AnyStatus::Link(LinkStatus::Attack100Loop), "ftPhysicsApplyGroundVelFriction"),
    ("link.Attack100End", FighterKind::Link, AnyStatus::Link(LinkStatus::Attack100End), "ftPhysicsApplyGroundVelFriction"),
    ("link.AppearR", FighterKind::Link, AnyStatus::Link(LinkStatus::AppearR), "ftCommonAppearProcPhysics"),
    ("link.AppearL", FighterKind::Link, AnyStatus::Link(LinkStatus::AppearL), "ftCommonAppearProcPhysics"),
    ("link.SpecialHi", FighterKind::Link, AnyStatus::Link(LinkStatus::SpecialHi), "ftLinkSpecialHiProcPhysics"),
    ("link.SpecialHiEnd", FighterKind::Link, AnyStatus::Link(LinkStatus::SpecialHiEnd), "ftPhysicsApplyGroundVelFriction"),
    ("link.SpecialAirHi", FighterKind::Link, AnyStatus::Link(LinkStatus::SpecialAirHi), "ftLinkSpecialAirHiProcPhysics"),
    ("link.SpecialN", FighterKind::Link, AnyStatus::Link(LinkStatus::SpecialN), "ftPhysicsApplyGroundVelFriction"),
    ("link.SpecialNGet", FighterKind::Link, AnyStatus::Link(LinkStatus::SpecialNGet), "ftPhysicsApplyGroundVelFriction"),
    ("link.SpecialNEmpty", FighterKind::Link, AnyStatus::Link(LinkStatus::SpecialNEmpty), "ftPhysicsApplyGroundVelFriction"),
    ("link.SpecialAirN", FighterKind::Link, AnyStatus::Link(LinkStatus::SpecialAirN), "ftPhysicsApplyAirVelFriction"),
    ("link.SpecialAirNReturn", FighterKind::Link, AnyStatus::Link(LinkStatus::SpecialAirNReturn), "ftPhysicsApplyAirVelFriction"),
    ("link.SpecialAirNEmpty", FighterKind::Link, AnyStatus::Link(LinkStatus::SpecialAirNEmpty), "ftPhysicsApplyAirVelFriction"),
    ("link.SpecialLw", FighterKind::Link, AnyStatus::Link(LinkStatus::SpecialLw), "ftPhysicsApplyGroundVelFriction"),
    ("link.SpecialAirLw", FighterKind::Link, AnyStatus::Link(LinkStatus::SpecialAirLw), "ftPhysicsApplyAirVelFriction"),
    ("luigi.Attack13", FighterKind::Luigi, AnyStatus::Mario(MarioStatus::Attack13), "ftPhysicsApplyGroundVelFriction"),
    ("luigi.AppearR", FighterKind::Luigi, AnyStatus::Mario(MarioStatus::AppearR), "ftCommonAppearProcPhysics"),
    ("luigi.AppearL", FighterKind::Luigi, AnyStatus::Mario(MarioStatus::AppearL), "ftCommonAppearProcPhysics"),
    ("luigi.SpecialN", FighterKind::Luigi, AnyStatus::Mario(MarioStatus::SpecialN), "ftPhysicsApplyGroundVelFriction"),
    ("luigi.SpecialAirN", FighterKind::Luigi, AnyStatus::Mario(MarioStatus::SpecialAirN), "ftPhysicsApplyAirVelDrift"),
    ("luigi.SpecialHi", FighterKind::Luigi, AnyStatus::Mario(MarioStatus::SpecialHi), "ftMarioSpecialHiProcPhysics"),
    ("luigi.SpecialAirHi", FighterKind::Luigi, AnyStatus::Mario(MarioStatus::SpecialAirHi), "ftMarioSpecialHiProcPhysics"),
    ("luigi.SpecialLw", FighterKind::Luigi, AnyStatus::Mario(MarioStatus::SpecialLw), "ftMarioSpecialLwProcPhysics"),
    ("luigi.SpecialAirLw", FighterKind::Luigi, AnyStatus::Mario(MarioStatus::SpecialAirLw), "ftMarioSpecialAirLwProcPhysics"),
    ("mario.Attack13", FighterKind::Mario, AnyStatus::Mario(MarioStatus::Attack13), "ftPhysicsApplyGroundVelFriction"),
    ("mario.AppearR", FighterKind::Mario, AnyStatus::Mario(MarioStatus::AppearR), "ftCommonAppearProcPhysics"),
    ("mario.AppearL", FighterKind::Mario, AnyStatus::Mario(MarioStatus::AppearL), "ftCommonAppearProcPhysics"),
    ("mario.SpecialN", FighterKind::Mario, AnyStatus::Mario(MarioStatus::SpecialN), "ftPhysicsApplyGroundVelFriction"),
    ("mario.SpecialAirN", FighterKind::Mario, AnyStatus::Mario(MarioStatus::SpecialAirN), "ftPhysicsApplyAirVelDrift"),
    ("mario.SpecialHi", FighterKind::Mario, AnyStatus::Mario(MarioStatus::SpecialHi), "ftMarioSpecialHiProcPhysics"),
    ("mario.SpecialAirHi", FighterKind::Mario, AnyStatus::Mario(MarioStatus::SpecialAirHi), "ftMarioSpecialHiProcPhysics"),
    ("mario.SpecialLw", FighterKind::Mario, AnyStatus::Mario(MarioStatus::SpecialLw), "ftMarioSpecialLwProcPhysics"),
    ("mario.SpecialAirLw", FighterKind::Mario, AnyStatus::Mario(MarioStatus::SpecialAirLw), "ftMarioSpecialAirLwProcPhysics"),
    ("ness.Attack13", FighterKind::Ness, AnyStatus::Ness(NessStatus::Attack13), "ftPhysicsApplyGroundVelFriction"),
    ("ness.AppearRStart", FighterKind::Ness, AnyStatus::Ness(NessStatus::AppearRStart), "ftCommonAppearProcPhysics"),
    ("ness.AppearLStart", FighterKind::Ness, AnyStatus::Ness(NessStatus::AppearLStart), "ftCommonAppearProcPhysics"),
    ("ness.AppearWait", FighterKind::Ness, AnyStatus::Ness(NessStatus::AppearWait), "ftCommonAppearProcPhysics"),
    ("ness.AppearREnd", FighterKind::Ness, AnyStatus::Ness(NessStatus::AppearREnd), "ftCommonAppearProcPhysics"),
    ("ness.AppearLEnd", FighterKind::Ness, AnyStatus::Ness(NessStatus::AppearLEnd), "ftCommonAppearProcPhysics"),
    ("ness.SpecialN", FighterKind::Ness, AnyStatus::Ness(NessStatus::SpecialN), "ftPhysicsApplyGroundVelFriction"),
    ("ness.SpecialAirN", FighterKind::Ness, AnyStatus::Ness(NessStatus::SpecialAirN), "ftPhysicsApplyAirVelFriction"),
    ("ness.SpecialHiStart", FighterKind::Ness, AnyStatus::Ness(NessStatus::SpecialHiStart), "ftNessSpecialHiProcPhysics"),
    ("ness.SpecialHiHold", FighterKind::Ness, AnyStatus::Ness(NessStatus::SpecialHiHold), "ftPhysicsApplyGroundVelFriction"),
    ("ness.SpecialHiEnd", FighterKind::Ness, AnyStatus::Ness(NessStatus::SpecialHiEnd), "ftPhysicsApplyGroundVelFriction"),
    ("ness.SpecialHi", FighterKind::Ness, AnyStatus::Ness(NessStatus::SpecialHiJibaku), "ftNessSpecialHiJibakuProcPhysics"),
    ("ness.SpecialAirHiStart", FighterKind::Ness, AnyStatus::Ness(NessStatus::SpecialAirHiStart), "ftNessSpecialAirHiProcPhysics"),
    ("ness.SpecialAirHiHold", FighterKind::Ness, AnyStatus::Ness(NessStatus::SpecialAirHiHold), "ftNessSpecialAirHiProcPhysics"),
    ("ness.SpecialAirHiEnd", FighterKind::Ness, AnyStatus::Ness(NessStatus::SpecialAirHiEnd), "ftNessSpecialAirHiProcPhysics"),
    ("ness.SpecialAirHiBound", FighterKind::Ness, AnyStatus::Ness(NessStatus::SpecialAirHiBound), "ftPhysicsApplyAirVelFriction"),
    ("ness.SpecialAirHi", FighterKind::Ness, AnyStatus::Ness(NessStatus::SpecialAirHiJibaku), "ftNessSpecialAirHiJibakuProcPhysics"),
    ("ness.SpecialLwStart", FighterKind::Ness, AnyStatus::Ness(NessStatus::SpecialLwStart), "ftNessSpecialLwProcPhysics"),
    ("ness.SpecialLwHold", FighterKind::Ness, AnyStatus::Ness(NessStatus::SpecialLwHold), "ftPhysicsApplyGroundVelFriction"),
    ("ness.SpecialLwHit", FighterKind::Ness, AnyStatus::Ness(NessStatus::SpecialLwHit), "ftPhysicsApplyGroundVelFriction"),
    ("ness.SpecialLwEnd", FighterKind::Ness, AnyStatus::Ness(NessStatus::SpecialLwEnd), "ftPhysicsApplyGroundVelFriction"),
    ("ness.SpecialAirLwStart", FighterKind::Ness, AnyStatus::Ness(NessStatus::SpecialAirLwStart), "ftNessSpecialAirLwProcPhysics"),
    ("ness.SpecialAirLwHold", FighterKind::Ness, AnyStatus::Ness(NessStatus::SpecialAirLwHold), "ftNessSpecialAirLwProcPhysics"),
    ("ness.SpecialAirLwHit", FighterKind::Ness, AnyStatus::Ness(NessStatus::SpecialAirLwHit), "ftNessSpecialAirLwProcPhysics"),
    ("ness.SpecialAirLwEnd", FighterKind::Ness, AnyStatus::Ness(NessStatus::SpecialAirLwEnd), "ftNessSpecialAirLwProcPhysics"),
    ("pikachu.AppearR", FighterKind::Pikachu, AnyStatus::Pikachu(PikachuStatus::AppearR), "ftCommonAppearProcPhysics"),
    ("pikachu.AppearL", FighterKind::Pikachu, AnyStatus::Pikachu(PikachuStatus::AppearL), "ftCommonAppearProcPhysics"),
    ("pikachu.SpecialN", FighterKind::Pikachu, AnyStatus::Pikachu(PikachuStatus::SpecialN), "ftPhysicsApplyGroundVelFriction"),
    ("pikachu.SpecialAirN", FighterKind::Pikachu, AnyStatus::Pikachu(PikachuStatus::SpecialAirN), "ftPhysicsApplyAirVelFriction"),
    ("pikachu.SpecialLwStart", FighterKind::Pikachu, AnyStatus::Pikachu(PikachuStatus::SpecialLwStart), "ftPhysicsApplyGroundVelFriction"),
    ("pikachu.SpecialLwLoop", FighterKind::Pikachu, AnyStatus::Pikachu(PikachuStatus::SpecialLwLoop), "ftPhysicsApplyGroundVelFriction"),
    ("pikachu.SpecialLwHit", FighterKind::Pikachu, AnyStatus::Pikachu(PikachuStatus::SpecialLwHit), "ftPhysicsApplyGroundVelFriction"),
    ("pikachu.SpecialAirLwStart", FighterKind::Pikachu, AnyStatus::Pikachu(PikachuStatus::SpecialAirLwStart), "ftPhysicsApplyAirVelFriction"),
    ("pikachu.SpecialAirLwLoop", FighterKind::Pikachu, AnyStatus::Pikachu(PikachuStatus::SpecialAirLwLoop), "ftPhysicsApplyAirVelFriction"),
    ("pikachu.SpecialAirLwHit", FighterKind::Pikachu, AnyStatus::Pikachu(PikachuStatus::SpecialAirLwHit), "ftPikachuSpecialAirLwHitProcPhysics"),
    ("pikachu.SpecialAirLwEnd", FighterKind::Pikachu, AnyStatus::Pikachu(PikachuStatus::SpecialAirLwEnd), "ftPhysicsApplyAirVelFriction"),
    ("pikachu.SpecialHiStart", FighterKind::Pikachu, AnyStatus::Pikachu(PikachuStatus::SpecialHiStart), "ftPhysicsApplyGroundVelFriction"),
    ("pikachu.SpecialHi", FighterKind::Pikachu, AnyStatus::Pikachu(PikachuStatus::SpecialHi), "ftPikachuSpecialHiProcPhysics"),
    ("pikachu.SpecialHiEnd", FighterKind::Pikachu, AnyStatus::Pikachu(PikachuStatus::SpecialHiEnd), "ftPikachuSpecialHiEndProcPhysics"),
    ("pikachu.SpecialAirHiStart", FighterKind::Pikachu, AnyStatus::Pikachu(PikachuStatus::SpecialAirHiStart), "ftPikachuSpecialAirHiStartProcPhysics"),
    ("pikachu.SpecialAirHi", FighterKind::Pikachu, AnyStatus::Pikachu(PikachuStatus::SpecialAirHi), "ftPikachuSpecialAirHiProcPhysics"),
    ("pikachu.SpecialAirHiEnd", FighterKind::Pikachu, AnyStatus::Pikachu(PikachuStatus::SpecialAirHiEnd), "ftPikachuSpecialAirHiEndProcPhysics"),
    ("purin.JumpAerialF1", FighterKind::Purin, AnyStatus::Purin(PurinStatus::JumpAerialF1), "ftCommonJumpAerialProcPhysics"),
    ("purin.JumpAerialF2", FighterKind::Purin, AnyStatus::Purin(PurinStatus::JumpAerialF2), "ftCommonJumpAerialProcPhysics"),
    ("purin.JumpAerialF3", FighterKind::Purin, AnyStatus::Purin(PurinStatus::JumpAerialF3), "ftCommonJumpAerialProcPhysics"),
    ("purin.JumpAerialF4", FighterKind::Purin, AnyStatus::Purin(PurinStatus::JumpAerialF4), "ftCommonJumpAerialProcPhysics"),
    ("purin.JumpAerialF5", FighterKind::Purin, AnyStatus::Purin(PurinStatus::JumpAerialF5), "ftCommonJumpAerialProcPhysics"),
    ("purin.AppearR", FighterKind::Purin, AnyStatus::Purin(PurinStatus::AppearR), "ftCommonAppearProcPhysics"),
    ("purin.AppearL", FighterKind::Purin, AnyStatus::Purin(PurinStatus::AppearL), "ftCommonAppearProcPhysics"),
    ("purin.SpecialN", FighterKind::Purin, AnyStatus::Purin(PurinStatus::SpecialN), "ftPhysicsApplyGroundVelTransN"),
    ("purin.SpecialAirN", FighterKind::Purin, AnyStatus::Purin(PurinStatus::SpecialAirN), "ftPurinSpecialAirNProcPhysics"),
    ("purin.SpecialHi", FighterKind::Purin, AnyStatus::Purin(PurinStatus::SpecialHi), "ftPhysicsApplyGroundVelFriction"),
    ("purin.SpecialAirHi", FighterKind::Purin, AnyStatus::Purin(PurinStatus::SpecialAirHi), "ftPhysicsApplyAirVelFriction"),
    ("purin.SpecialLw", FighterKind::Purin, AnyStatus::Purin(PurinStatus::SpecialLw), "ftPhysicsApplyGroundVelFriction"),
    ("purin.SpecialAirLw", FighterKind::Purin, AnyStatus::Purin(PurinStatus::SpecialAirLw), "ftPhysicsApplyAirVelFriction"),
    ("samus.AppearR", FighterKind::Samus, AnyStatus::Samus(SamusStatus::AppearR), "ftCommonAppearProcPhysics"),
    ("samus.AppearL", FighterKind::Samus, AnyStatus::Samus(SamusStatus::AppearL), "ftCommonAppearProcPhysics"),
    ("samus.SpecialNStart", FighterKind::Samus, AnyStatus::Samus(SamusStatus::SpecialNStart), "ftPhysicsApplyGroundVelFriction"),
    ("samus.SpecialNLoop", FighterKind::Samus, AnyStatus::Samus(SamusStatus::SpecialNLoop), "ftPhysicsApplyGroundVelFriction"),
    ("samus.SpecialNEnd", FighterKind::Samus, AnyStatus::Samus(SamusStatus::SpecialNEnd), "ftPhysicsApplyGroundVelFriction"),
    ("samus.SpecialAirNStart", FighterKind::Samus, AnyStatus::Samus(SamusStatus::SpecialAirNStart), "ftPhysicsApplyAirVelDrift"),
    ("samus.SpecialAirNEnd", FighterKind::Samus, AnyStatus::Samus(SamusStatus::SpecialAirNEnd), "ftPhysicsApplyAirVelFriction"),
    ("samus.SpecialHi", FighterKind::Samus, AnyStatus::Samus(SamusStatus::SpecialHi), "ftSamusSpecialHiProcPhysics"),
    ("samus.SpecialAirHi", FighterKind::Samus, AnyStatus::Samus(SamusStatus::SpecialAirHi), "ftSamusSpecialAirHiProcPhysics"),
    ("samus.SpecialLw", FighterKind::Samus, AnyStatus::Samus(SamusStatus::SpecialLw), "ftSamusSpecialLwProcPhysics"),
    ("samus.SpecialAirLw", FighterKind::Samus, AnyStatus::Samus(SamusStatus::SpecialAirLw), "ftSamusSpecialAirLwProcPhysics"),
    ("yoshi.AppearR", FighterKind::Yoshi, AnyStatus::Yoshi(YoshiStatus::AppearR), "ftCommonAppearProcPhysics"),
    ("yoshi.AppearL", FighterKind::Yoshi, AnyStatus::Yoshi(YoshiStatus::AppearL), "ftCommonAppearProcPhysics"),
    ("yoshi.SpecialHi", FighterKind::Yoshi, AnyStatus::Yoshi(YoshiStatus::SpecialHi), "ftYoshiSpecialHiProcPhysics"),
    ("yoshi.SpecialAirHi", FighterKind::Yoshi, AnyStatus::Yoshi(YoshiStatus::SpecialAirHi), "ftYoshiSpecialAirHiProcPhysics"),
    ("yoshi.SpecialLwStart", FighterKind::Yoshi, AnyStatus::Yoshi(YoshiStatus::SpecialLwStart), "ftPhysicsApplyAirVelTransNAll"),
    ("yoshi.SpecialLwLanding", FighterKind::Yoshi, AnyStatus::Yoshi(YoshiStatus::SpecialLwLanding), "ftPhysicsApplyGroundVelFriction"),
    ("yoshi.SpecialAirLwStart", FighterKind::Yoshi, AnyStatus::Yoshi(YoshiStatus::SpecialAirLwStart), "ftPhysicsApplyAirVelTransNAll"),
    ("yoshi.SpecialAirLwLoop", FighterKind::Yoshi, AnyStatus::Yoshi(YoshiStatus::SpecialAirLwLoop), "ftYoshiSpecialAirLwLoopProcPhysics"),
    ("yoshi.SpecialN", FighterKind::Yoshi, AnyStatus::Yoshi(YoshiStatus::SpecialN), "ftPhysicsApplyGroundVelFriction"),
    ("yoshi.SpecialNCatch", FighterKind::Yoshi, AnyStatus::Yoshi(YoshiStatus::SpecialNCatch), "ftPhysicsApplyGroundVelFriction"),
    ("yoshi.SpecialNRelease", FighterKind::Yoshi, AnyStatus::Yoshi(YoshiStatus::SpecialNRelease), "ftPhysicsApplyGroundVelFriction"),
    ("yoshi.SpecialAirN", FighterKind::Yoshi, AnyStatus::Yoshi(YoshiStatus::SpecialAirN), "ftPhysicsApplyAirVelFriction"),
    ("yoshi.SpecialAirNCatch", FighterKind::Yoshi, AnyStatus::Yoshi(YoshiStatus::SpecialAirNCatch), "ftPhysicsApplyAirVelFriction"),
    ("yoshi.SpecialAirNRelease", FighterKind::Yoshi, AnyStatus::Yoshi(YoshiStatus::SpecialAirNRelease), "ftPhysicsApplyAirVelFriction"),
];

/// `ftPhysicsCheckSetFastFall` runs from `ftPhysicsApplyAirVelDriftFastFall`
/// and `ftCommonJumpAerialProcPhysics` alone among the character tables'
/// callbacks; the ones listed as conditional reach it on one motion flag
/// (Falcon Punch's and Pound's `flag2 == 2`) or on hitstun.
#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn only_the_original_fast_fall_callbacks_take_a_down_flick() {
    const CONDITIONAL: &[&str] = &[
        "ftCaptainSpecialNProcPhysics",
        "ftKirbyCopyCaptainSpecialNProcPhysics",
        "ftKirbyCopyPurinSpecialNProcPhysics",
        "ftPurinSpecialNProcPhysics",
        "ftCommonDamageCommonProcPhysics",
    ];
    let mut bad = Vec::new();
    for &(name, kind, status, callback) in ALL {
        // The battle entry's physics is `crate::appear::physics`, ahead of
        // the airborne step.
        if status.is_grounded()
            || CONDITIONAL.contains(&callback)
            || callback == "ftCommonAppearProcPhysics"
        {
            continue;
        }
        let want = matches!(
            callback,
            "ftPhysicsApplyAirVelDriftFastFall" | "ftCommonJumpAerialProcPhysics"
        );
        let got = std::panic::catch_unwind(|| {
            probe(kind, status, false, 0, Vec3::new(3.0, 4.0, 5.0))
                .physics
                .is_fastfall
        });
        match got {
            Ok(got) if got == want => {}
            Ok(got) => bad.push(format!("{name} ({callback}): fast fall {got}, want {want}")),
            Err(_) => bad.push(format!("{name}: panicked")),
        }
    }
    assert!(
        bad.is_empty(),
        "{} mismatches:\n{}",
        bad.len(),
        bad.join("\n")
    );
}

fn grounded(kind: FighterKind) -> Fighter {
    let mut f = Fighter::new(kind, 0, 3);
    f.situation = Situation::Ground;
    f.floor = Some(Standing {
        line: 0,
        flags: 0,
        normal: ssb_engine::math::Vec2::new(0.0, 1.0),
    });
    f
}

/// `ftCaptainSpecialLwProcPhysics` scales `vel_air` after the grounded
/// TransN transfer (`ftcaptainspeciallw.c:97`): a hit slows the grounded
/// kick's step.
#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn a_grounded_falcon_kick_slows_by_its_hit_scale() {
    let step = |scale: f32| {
        let mut f = grounded(FighterKind::Captain);
        crate::captain::set_special_lw(&mut f);
        f.captain.kick_scale = scale;
        f.set_root_motion(RootMotion {
            delta: Vec3::new(0.0, 0.0, 12.0),
            ..RootMotion::default()
        });
        let x = f.pos.x;
        let (_, want) = f.ground_physics();
        (want.x - x, f.physics.vel_air.x)
    };
    let (full, full_air) = step(1.0);
    let (half, half_air) = step(0.5);
    assert!(full != 0.0);
    assert_eq!(half, full * 0.5);
    assert_eq!(half_air, full_air * 0.5);
}

/// Grounded Stone's start has no `proc_physics`: it stands still on the
/// `vel_air` its setter zeroed and keeps `vel_ground` for the hold's slide
/// (`ftkirbyspeciallw.c:296-307`, `ftKirbySpecialLwHoldProcPhysics`).
#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn stone_on_the_ground_holds_still_and_keeps_its_run_for_the_slide() {
    let mut f = grounded(FighterKind::Kirby);
    f.physics.vel_ground.x = 10.0;
    f.physics.vel_air.x = 10.0;
    crate::kirby::set_special_lw(&mut f);
    let x = f.pos.x;
    let (_, want) = f.ground_physics();
    assert_eq!(want.x, x);
    assert_eq!(f.physics.vel_ground.x, 10.0);
}

/// Grounded PSI Magnet: the start only counts the gravity delay down and
/// steps by the `vel_air` left; the hold, hit and end are
/// `ftPhysicsApplyGroundVelFriction` and leave the delay alone
/// (`ftnessstatus.h`, `ftnessspeciallw.c:71`).
#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn grounded_psi_magnet_start_coasts_and_its_hold_takes_friction() {
    let mut f = grounded(FighterKind::Ness);
    crate::ness::set_special_lw(&mut f);
    f.physics.vel_air = Vec3::new(3.0, 0.0, 0.0);
    f.physics.vel_ground.x = 10.0;
    let x = f.pos.x;
    let (_, want) = f.ground_physics();
    assert_eq!(want.x, x + 3.0);
    assert_eq!(f.physics.vel_ground.x, 10.0);
    assert_eq!(f.ness.gravity_delay, 3);

    for s in [
        NessStatus::SpecialLwHold,
        NessStatus::SpecialLwHit,
        NessStatus::SpecialLwEnd,
        NessStatus::SpecialHiHold,
        NessStatus::SpecialHiEnd,
    ] {
        f.status.status = AnyStatus::Ness(s);
        f.physics.vel_ground.x = 10.0;
        f.ness.gravity_delay = 3;
        f.ground_physics();
        assert!(f.physics.vel_ground.x < 10.0, "{s:?}");
        assert_eq!(f.ness.gravity_delay, 3, "{s:?}");
    }
}

/// `ftSamusSpecialLwProcPhysics`: with the script's flag 3 up (the landed
/// ball) the stick rolls the Bomb's ball; before it, plain friction.
#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn the_landed_bomb_ball_rolls_with_the_stick() {
    for (flag3, facing) in [(0u32, 1.0f32), (1, 1.0), (1, -1.0)] {
        let mut f = grounded(FighterKind::Samus);
        f.status.status = AnyStatus::Samus(SamusStatus::SpecialLw);
        if facing < 0.0 {
            f.facing = f.facing.flipped();
        }
        assert_eq!(f.facing.sign(), facing);
        f.motion_script.flags[3] = flag3;
        f.input.stick_x = 80;
        f.ground_physics();
        let roll = 80.0 * (f.attributes.walk_speed_mul * crate::samus::BOMB_DRIFT);
        let want = match (flag3, facing > 0.0) {
            (0, _) => 0.0,
            // Facing the stick: up to speed at once.
            (_, true) => roll,
            // Facing away: speeds up by traction a frame.
            (_, false) => f.attributes.traction,
        };
        assert_eq!(
            f.physics.vel_ground.x, want,
            "flag3 {flag3} facing {facing}"
        );
    }
}
