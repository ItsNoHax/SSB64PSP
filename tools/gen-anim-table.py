#!/usr/bin/env python3
"""Generate ssb-rom's fighter animation lookup table from the decompilation.

The game finds a status's animation through three records, each of which
names both of its sides:

    dFTCommonActionStatusDescs[status - 6].mflags.motion_id
      -> dFT<Name>MotionDescs[motion_id].anim_file_id
        -> relocData file <id>_FT<Name>Anim<X>.c

None of that lives in the archive — the first two tables are in the game
code's data segment — so `ssb-rom` carries the resolved `(fighter, status) ->
file id` pairing as a constant, exactly as it carries `FIGHTER_FILES`. This
script is how that constant is produced, so the transcription is reproducible
rather than hand-typed.

It also emits the animation length the decompilation's own C sources give for
each file, which `romtool anims --verify` checks the ROM-derived lengths
against. The two readings are independent: one walks compressed archive bytes,
the other walks C macros somebody else wrote by hand.

Usage:
    tools/gen-anim-table.py [--refs refs/ssb-decomp-re]
"""
import argparse
import os
import re
import sys

PROJECT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))

# Statuses whose animation length the status machine actually needs: the ones
# that end when their animation runs out. Looping statuses (Wait, the walks,
# Run, Fall) and those timed by FTAttributes instead (KneeBend, the walks'
# phase matching) are deliberately absent.
# Each entry is (slot name, FTCommonStatus id, animation names the resolved
# symbol is allowed to end with). The animation names are a check, not a
# lookup: they come from the decompilation's file names while the index comes
# from the FTCommonMotion enum, so a motion table parsed even one entry out of
# step resolves to an animation whose name no longer fits its status.
SLOTS = [
    # Statuses that end when their animation runs out. These are the ones the
    # status machine needs a *length* for (RE-035).
    ("Dash",     15, ["Dash"]),
    ("Turn",     18, ["Turn"]),
    ("RunBrake", 17, ["RunBrake"]),
    ("Squat",    28, ["Crouch", "Squat"]),
    ("SquatRv",  30, ["CrouchEnd", "SquatRv"]),
    # Jigglypuff's landing animation is called JumpSquat: it serves both
    # KneeBend and Landing, exactly as everyone else's LandingAirX does.
    ("Landing",  31, ["LandingAirX", "Landing", "JumpSquat"]),
    ("Pass",     33, ["ShieldDrop", "Pass"]),
    # The rest of the movement statuses. These end by being interrupted rather
    # than by running out, so nothing needed their length -- but a fighter that
    # only animates while dashing or crouching is a fighter that spends most of
    # its time in a rest pose, so they are here for the poses.
    # Several fighters' idle file is symbol-named after a different use of the
    # same animation -- Fox's reads `EggLay`. That is a naming quirk, not a
    # table read out of step: a shift would misname every *later* slot too, and
    # every fighter's seven length-bearing slots verify against the ROM.
    ("Wait",       10, ["Wait", "EggLay", "Idle"]),
    ("WalkSlow",   11, ["Walk1", "WalkSlow"]),
    ("WalkMiddle", 12, ["Walk2", "WalkMiddle"]),
    ("WalkFast",   13, ["Walk3", "WalkFast"]),
    ("Run",        16, ["Run"]),
    # Jumpsquat and landing are the same knees-bent pose, and most of the
    # roster shares one file between them -- the mirror of the Jigglypuff case
    # noted against `Landing` below.
    ("KneeBend",   20, ["JumpSquat", "KneeBend", "LandingAirX"]),
    ("JumpF",      22, ["JumpF", "Jump"]),
    ("JumpB",      23, ["JumpB", "Jump"]),
    # Yoshi has one aerial-jump animation and uses it both ways; Kirby has
    # none at all, and his motion slots are the null placeholders RE-035 found.
    ("JumpAerialF", 24, ["JumpAerialF", "JumpAerialB", "JumpAerial", "Jump"]),
    ("JumpAerialB", 25, ["JumpAerialB", "JumpAerialF", "JumpAerial", "Jump"]),
    ("Fall",       26, ["Fall"]),
    ("FallAerial", 27, ["FallAerial", "Fall"]),
    ("SquatWait",  29, ["CrouchIdle", "SquatWait"]),
]

# Character status tables begin at `nFTCommonMotionSpecialStart`, so they
# cannot be resolved through the common-status pairing above.  Keep the
# target fighter and decompilation animation symbol explicit rather than
# treating the similarly numbered statuses of other fighters as Mario moves.
# Both Super Jump Punch statuses use one figatree; their different
# motion-script entry points are gameplay data, while their skeletal pose is
# the same.
SPECIAL_SLOTS = [
    # Luigi runs Mario's special statuses (`ftluigistatus.h`) and his motion
    # table names Mario's figatrees for them, so both fill these slots.
    ("MarioSpecialN", ("Mario", "Luigi"), "FTMarioAnimFireballGround"),
    ("MarioSpecialAirN", ("Mario", "Luigi"), "FTMarioAnimFireballAir"),
    ("MarioSpecialHi", ("Mario", "Luigi"), "FTMarioAnimSuperJumpPunchAir"),
    ("MarioSpecialAirHi", ("Mario", "Luigi"), "FTMarioAnimSuperJumpPunchAir"),
    ("MarioSpecialLw", ("Mario", "Luigi"), "FTMarioAnimMarioTornadoGround"),
    ("MarioSpecialAirLw", ("Mario", "Luigi"), "FTMarioAnimMarioTornadoAir"),
    ("FoxAttack11", "Fox", "FTFoxAnimJab1"),
    ("FoxAttack12", "Fox", "FTFoxAnimJab2"),
    ("FoxAttack100Start", "Fox", "FTFoxAnimJabLoopStart"),
    ("FoxAttack100Loop", "Fox", "FTFoxAnimJabLoop"),
    ("FoxAttack100End", "Fox", "FTFoxAnimJabLoopEnd"),
    ("FoxAttackDash", "Fox", "FTFoxAnimDashAttack"),
    ("FoxAttackS3Hi", "Fox", "FTFoxAnimFTiltHigh"),
    ("FoxAttackS3HiS", "Fox", "FTFoxAnimFTiltMidHigh"),
    ("FoxAttackS3", "Fox", "FTFoxAnimFTilt"),
    ("FoxAttackS3LwS", "Fox", "FTFoxAnimFTiltMidLow"),
    ("FoxAttackS3Lw", "Fox", "FTFoxAnimFTiltLow"),
    ("FoxAttackHi3", "Fox", "FTFoxAnimUTilt"),
    ("FoxAttackLw3", "Fox", "FTFoxAnimDTilt"),
    ("FoxAttackS4", "Fox", "FTFoxAnimFSmash"),
    ("FoxAttackHi4", "Fox", "FTFoxAnimUSmash"),
    ("FoxAttackLw4", "Fox", "FTFoxAnimDSmash"),
    ("FoxAttackAirN", "Fox", "FTFoxAnimAttackAirN"),
    ("FoxAttackAirF", "Fox", "FTFoxAnimAttackAirF"),
    ("FoxAttackAirB", "Fox", "FTFoxAnimAttackAirB"),
    ("FoxAttackAirHi", "Fox", "FTFoxAnimAttackAirU"),
    ("FoxAttackAirLw", "Fox", "FTFoxAnimAttackAirD"),
    ("FoxSpecialN", "Fox", "FTFoxAnimLaser"),
    ("FoxSpecialAirN", "Fox", "FTFoxAnimLaserAerial"),
    ("FoxSpecialHiStart", "Fox", "FTFoxAnimFireFoxStartGround"),
    ("FoxSpecialAirHiStart", "Fox", "FTFoxAnimFireFoxStartAerial"),
    ("FoxSpecialHiHold", "Fox", "FTFoxAnimReadyingFireFoxGround"),
    ("FoxSpecialAirHiHold", "Fox", "FTFoxAnimReadyingFireFoxAir"),
    ("FoxSpecialHi", "Fox", "FTFoxAnimFireFoxGround"),
    ("FoxSpecialAirHi", "Fox", "FTFoxAnimFireFoxAir"),
    ("FoxSpecialHiEnd", "Fox", "FTFoxAnimFireFoxEndGround"),
    ("FoxSpecialAirHiEnd", "Fox", "FTFoxAnimFireFoxEndAir"),
    ("FoxSpecialAirHiBound", "Fox", "FTFoxAnimLandingWhileFireFoxAir"),
    ("FoxSpecialLwStart", "Fox", "FTFoxAnimShineStart"),
    ("FoxSpecialLwTurn", "Fox", "FTFoxAnimSwitchDirectionShine"),
    ("FoxSpecialLwHit", "Fox", "FTFoxAnimReflecting"),
    ("FoxSpecialLwLoop", "Fox", "FTFoxAnimShine"),
    ("FoxSpecialAirLwStart", "Fox", "FTFoxAnimShireStartAir"),
    ("FoxSpecialAirLwTurn", "Fox", "FTFoxAnimSwitchDirectionShineAir"),
    ("FoxSpecialAirLwHit", "Fox", "FTFoxAnimUnknown"),
    ("FoxSpecialAirLwLoop", "Fox", "FTFoxAnimShineAirEnd"),
    ("FoxSpecialLwEnd", "Fox", "FTFoxAnimShine"),
    ("FoxSpecialAirLwEnd", "Fox", "FTFoxAnimShineAirEnd"),
    # Donkey Kong's common attacks and special-state poses. The source
    # character motion table resolves these names to archive files.
    ("DonkeyAttack11", "Donkey", "FTDonkeyAnimJab1"),
    ("DonkeyAttack12", "Donkey", "FTDonkeyAnimJab2"),
    ("DonkeyAttackDash", "Donkey", "FTDonkeyAnimDashAttack"),
    ("DonkeyAttackS3Hi", "Donkey", "FTDonkeyAnimFTiltHigh"),
    ("DonkeyAttackS3", "Donkey", "FTDonkeyAnimFTilt"),
    ("DonkeyAttackS3Lw", "Donkey", "FTDonkeyAnimFTiltLow"),
    ("DonkeyAttackHi3", "Donkey", "FTDonkeyAnimUTilt"),
    ("DonkeyAttackLw3", "Donkey", "FTDonkeyAnimDTilt"),
    ("DonkeyAttackS4Hi", "Donkey", "FTDonkeyAnimFSmashHigh"),
    ("DonkeyAttackS4HiS", "Donkey", "FTDonkeyAnimFSmashMidHigh"),
    ("DonkeyAttackS4", "Donkey", "FTDonkeyAnimFSmash"),
    ("DonkeyAttackS4LwS", "Donkey", "FTDonkeyAnimFSmashMidLow"),
    ("DonkeyAttackS4Lw", "Donkey", "FTDonkeyAnimFSmashLow"),
    ("DonkeyAttackHi4", "Donkey", "FTDonkeyAnimUSmash"),
    ("DonkeyAttackLw4", "Donkey", "FTDonkeyAnimDSmash"),
    ("DonkeyAttackAirN", "Donkey", "FTDonkeyAnimAttackAirN"),
    ("DonkeyAttackAirF", "Donkey", "FTDonkeyAnimAttackAirF"),
    ("DonkeyAttackAirB", "Donkey", "FTDonkeyAnimAttackAirB"),
    ("DonkeyAttackAirHi", "Donkey", "FTDonkeyAnimAttackAirU"),
    ("DonkeyAttackAirLw", "Donkey", "FTDonkeyAnimAttackAirD"),
    ("DonkeySpecialNStart", "Donkey", "FTDonkeyAnimGiantPunchGroundLoopStart"),
    ("DonkeySpecialAirNStart", "Donkey", "FTDonkeyAnimGiantPunchAirLoopStart"),
    ("DonkeySpecialNLoop", "Donkey", "FTDonkeyAnimGiantPunchGroundLoop"),
    ("DonkeySpecialAirNLoop", "Donkey", "FTDonkeyAnimGiantPunchAirLoop"),
    ("DonkeySpecialNEnd", "Donkey", "FTDonkeyAnimGiantPunchGroundFullyChargedPunch"),
    ("DonkeySpecialAirNEnd", "Donkey", "FTDonkeyAnimGiantPunchAirFullyChargedPunch"),
    ("DonkeySpecialHi", "Donkey", "FTDonkeyAnimSpinningKongGround"),
    ("DonkeySpecialAirHi", "Donkey", "FTDonkeyAnimSpinningKongAir"),
    ("DonkeySpecialLwStart", "Donkey", "FTDonkeyAnimHandSlapStart"),
    ("DonkeySpecialLwLoop", "Donkey", "FTDonkeyAnimHandSlapLoop"),
    ("DonkeySpecialLwEnd", "Donkey", "FTDonkeyAnimHandSlapEnd"),
]


# Donkey Kong's cargo carry (`ftdonkeythrowf*.c`). Its motion descriptors
# reuse one held-pose figatree for Wait, KneeBend, Fall, Landing and Damage;
# the source symbol is named after the landing, but the pairing is by
# `dFTDonkeyMotionDescs` position (`nFTDonkeyMotionThrowFWait` onward).
SPECIAL_SLOTS += [
    ("DonkeyThrowFWait", "Donkey", "FTDonkeyAnimCargoLanding"),
    ("DonkeyThrowFWalkSlow", "Donkey", "FTDonkeyAnimCargoVerySlowWalk"),
    ("DonkeyThrowFWalkMiddle", "Donkey", "FTDonkeyAnimCargoSlowWalk"),
    ("DonkeyThrowFWalkFast", "Donkey", "FTDonkeyAnimCargoWalk"),
    ("DonkeyThrowFTurn", "Donkey", "FTDonkeyAnimCargoTurn"),
    ("DonkeyThrowFKneeBend", "Donkey", "FTDonkeyAnimCargoLanding"),
    ("DonkeyThrowFFall", "Donkey", "FTDonkeyAnimCargoLanding"),
    ("DonkeyThrowFLanding", "Donkey", "FTDonkeyAnimCargoLanding"),
    ("DonkeyThrowFDamage", "Donkey", "FTDonkeyAnimCargoLanding"),
    ("DonkeyThrowFF", "Donkey", "FTDonkeyAnimCargoAirThrow"),
    ("DonkeyThrowAirFF", "Donkey", "FTDonkeyAnimCargoAirThrow"),
]

# Shared grab, capture and thrown statuses (`ftcommoncatch*.c`,
# `ftcommoncapture*.c`, `ftcommonthrow*.c`). These resolve through the common
# status -> motion pairing like `SLOTS`, but are packed only for the fighters
# whose gameplay is ported, so the pack does not carry 27 copies of moves no
# playable fighter can reach yet. The thrown symbols are auto-named and do not
# describe the motion (Fox's `ThrownFoxFStart` file is labelled `ThrownDK`),
# so no name check is applied; the index pairing is the evidence.
GRAB_FIGHTERS = {"Mario", "Fox", "Donkey", "Samus", "Luigi", "Link", "Yoshi", "Captain",
                 "Kirby", "Pikachu", "Purin", "Ness"}
GRAB_SLOTS = [
    ("Catch",             166),
    ("CatchPull",         167),
    ("ThrowF",            169),
    ("ThrowB",            170),
    ("CapturePulled",     171),
    ("ThrownDonkeyF",     181),
    ("ThrownMarioBStart", 182),
    ("ThrownFoxFStart",   183),
    ("Shouldered",        184),
    ("ThrownMarioB",      185),
    ("ThrownCommon",      186),
    ("ThrownFoxF",        187),
    ("ThrownFoxB",        188),
]

# Samus's common attacks and specials (`216_SamusMainMotion.c`,
# `dFTSamusMotionDescs`). They follow the grab slots so the earlier slot
# numbers stay stable for the fighters already ported.
LATE_SPECIAL_SLOTS = [
    ("SamusAttack11", "Samus", "FTSamusAnimJab1"),
    ("SamusAttack12", "Samus", "FTSamusAnimJab2"),
    ("SamusAttackDash", "Samus", "FTSamusAnimDashAttack"),
    ("SamusAttackS3Hi", "Samus", "FTSamusAnimFTiltHigh"),
    ("SamusAttackS3HiS", "Samus", "FTSamusAnimFTiltMidHigh"),
    ("SamusAttackS3", "Samus", "FTSamusAnimFTilt"),
    ("SamusAttackS3LwS", "Samus", "FTSamusAnimFTiltMidLow"),
    ("SamusAttackS3Lw", "Samus", "FTSamusAnimFTiltLow"),
    ("SamusAttackHi3", "Samus", "FTSamusAnimUTilt"),
    ("SamusAttackLw3", "Samus", "FTSamusAnimDTilt"),
    ("SamusAttackS4Hi", "Samus", "FTSamusAnimFSmashHigh"),
    ("SamusAttackS4HiS", "Samus", "FTSamusAnimFSmashMidHigh"),
    ("SamusAttackS4", "Samus", "FTSamusAnimFSmash"),
    ("SamusAttackS4LwS", "Samus", "FTSamusAnimFSmashMidLow"),
    ("SamusAttackS4Lw", "Samus", "FTSamusAnimFSmashLow"),
    ("SamusAttackHi4", "Samus", "FTSamusAnimUSmash"),
    ("SamusAttackLw4", "Samus", "FTSamusAnimDSmash"),
    ("SamusAttackAirN", "Samus", "FTSamusAnimAttackAirN"),
    ("SamusAttackAirF", "Samus", "FTSamusAnimAttackAirF"),
    ("SamusAttackAirB", "Samus", "FTSamusAnimAttackAirB"),
    ("SamusAttackAirHi", "Samus", "FTSamusAnimAttackAirU"),
    ("SamusAttackAirLw", "Samus", "FTSamusAnimAttackAirD"),
    ("SamusSpecialNStart", "Samus", "FTSamusAnimStartingChargeShot"),
    ("SamusSpecialNLoop", "Samus", "FTSamusAnimChargingNeutralSpecial"),
    ("SamusSpecialNEnd", "Samus", "FTSamusAnimShooting"),
    ("SamusSpecialAirNStart", "Samus", "FTSamusAnimStartingChargeShotAir"),
    ("SamusSpecialAirNEnd", "Samus", "FTSamusAnimShootingAir"),
    ("SamusSpecialHi", "Samus", "FTSamusAnimScrewAttackGround"),
    ("SamusSpecialAirHi", "Samus", "FTSamusAnimScrewAttackAir"),
    ("SamusSpecialLw", "Samus", "FTSamusAnimBomb"),
    ("SamusSpecialAirLw", "Samus", "FTSamusAnimBombAir"),
]

# Luigi's common attacks and jab finisher (`220_LuigiMainMotion.c`,
# `dFTLuigiMotionDescs`). Most name Mario's figatrees: the two share a
# skeleton, and Luigi has his own dash attack, down tilt and forward smashes.
LATE_SPECIAL_SLOTS += [
    ("LuigiAttack11", "Luigi", "FTMarioAnimJab1"),
    ("LuigiAttack12", "Luigi", "FTMarioAnimJab2"),
    ("LuigiAttack13", "Luigi", "FTMarioAnimJab3"),
    ("LuigiAttackDash", "Luigi", "FTLuigiAnimDashAttack"),
    ("LuigiAttackS3Hi", "Luigi", "FTMarioAnimFTiltHigh"),
    ("LuigiAttackS3", "Luigi", "FTMarioAnimFTilt"),
    ("LuigiAttackS3Lw", "Luigi", "FTMarioAnimFTiltLow"),
    ("LuigiAttackHi3", "Luigi", "FTMarioAnimUTilt"),
    ("LuigiAttackLw3", "Luigi", "FTLuigiAnimDTilt"),
    ("LuigiAttackS4Hi", "Luigi", "FTLuigiAnimFSmashHigh"),
    ("LuigiAttackS4HiS", "Luigi", "FTLuigiAnimFSmashMidHigh"),
    ("LuigiAttackS4", "Luigi", "FTLuigiAnimFSmash"),
    ("LuigiAttackS4LwS", "Luigi", "FTLuigiAnimFSmashMidLow"),
    ("LuigiAttackS4Lw", "Luigi", "FTLuigiAnimFSmashLow"),
    ("LuigiAttackHi4", "Luigi", "FTMarioAnimUSmash"),
    ("LuigiAttackLw4", "Luigi", "FTMarioAnimDSmash"),
    ("LuigiAttackAirN", "Luigi", "FTMarioAnimAttackAirN"),
    ("LuigiAttackAirF", "Luigi", "FTMarioAnimAttackAirF"),
    ("LuigiAttackAirB", "Luigi", "FTMarioAnimAttackAirB"),
    ("LuigiAttackAirHi", "Luigi", "FTMarioAnimAttackAirU"),
    ("LuigiAttackAirLw", "Luigi", "FTMarioAnimAttackAirD"),
]

# Link's common attacks, then his own statuses in `ftLinkStatus` order
# without the two Appear entries (`224_LinkMainMotion.c`,
# `dFTLinkMotionDescs`). He has one forward tilt and one forward smash. The
# boomerang throw and its empty-handed variant share one figatree.
LATE_SPECIAL_SLOTS += [
    ("LinkAttack11", "Link", "FTLinkAnimJab1"),
    ("LinkAttack12", "Link", "FTLinkAnimJab2"),
    ("LinkAttackDash", "Link", "FTLinkAnimDashAttack"),
    ("LinkAttackS3", "Link", "FTLinkAnimFTilt"),
    ("LinkAttackHi3", "Link", "FTLinkAnimUTilt"),
    ("LinkAttackLw3", "Link", "FTLinkAnimDTilt"),
    ("LinkAttackS4", "Link", "FTLinkAnimFSmash"),
    ("LinkAttackHi4", "Link", "FTLinkAnimUSmash"),
    ("LinkAttackLw4", "Link", "FTLinkAnimDSmash"),
    ("LinkAttackAirN", "Link", "FTLinkAnimAttackAirN"),
    ("LinkAttackAirF", "Link", "FTLinkAnimAttackAirF"),
    ("LinkAttackAirB", "Link", "FTLinkAnimAttackAirB"),
    ("LinkAttackAirHi", "Link", "FTLinkAnimAttackAirU"),
    ("LinkAttackAirLw", "Link", "FTLinkAnimAttackAirD"),
    ("LinkAttack13", "Link", "FTLinkAnimJab3"),
    ("LinkAttack100Start", "Link", "FTLinkAnimJabLoopStart"),
    ("LinkAttack100Loop", "Link", "FTLinkAnimJabLoop"),
    ("LinkAttack100End", "Link", "FTLinkAnimJabLoopEnd"),
    ("LinkSpecialHi", "Link", "FTLinkAnimUpSpecialGround"),
    ("LinkSpecialHiEnd", "Link", "FTLinkAnimUpSpecialEndGround"),
    ("LinkSpecialAirHi", "Link", "FTLinkAnimUpSpecialAir"),
    ("LinkSpecialN", "Link", "FTLinkAnimMissingBoomerang"),
    ("LinkSpecialNGet", "Link", "FTLinkAnimCatchingBoomerang"),
    ("LinkSpecialNEmpty", "Link", "FTLinkAnimMissingBoomerang"),
    ("LinkSpecialAirN", "Link", "FTLinkAnimMissingBoomerangAir"),
    ("LinkSpecialAirNReturn", "Link", "FTLinkAnimCatchingBoomerangAir"),
    ("LinkSpecialAirNEmpty", "Link", "FTLinkAnimMissingBoomerangAir"),
    ("LinkSpecialLw", "Link", "FTLinkAnimBomb"),
    ("LinkSpecialAirLw", "Link", "FTLinkAnimBombAir"),
]

# Yoshi's common attacks, then his own statuses in `ftYoshiStatus` order
# without the two Appear entries and `SpecialAirLwLoop`, whose motion id is
# -1 (`246_YoshiMainMotion.c`, `dFTYoshiMotionDescs`). He has three forward
# tilts and three forward smashes. Each Egg Lay grab status keeps the figatree
# of the status before it.
LATE_SPECIAL_SLOTS += [
    ("YoshiAttack11", "Yoshi", "FTYoshiAnimJab1"),
    ("YoshiAttack12", "Yoshi", "FTYoshiAnimJab2"),
    ("YoshiAttackDash", "Yoshi", "FTYoshiAnimDashAttack"),
    ("YoshiAttackS3Hi", "Yoshi", "FTYoshiAnimFTiltHigh"),
    ("YoshiAttackS3", "Yoshi", "FTYoshiAnimFTilt"),
    ("YoshiAttackS3Lw", "Yoshi", "FTYoshiAnimFTiltLow"),
    ("YoshiAttackHi3", "Yoshi", "FTYoshiAnimUTilt"),
    ("YoshiAttackLw3", "Yoshi", "FTYoshiAnimDTilt"),
    ("YoshiAttackS4Hi", "Yoshi", "FTYoshiAnimFSmashHigh"),
    ("YoshiAttackS4", "Yoshi", "FTYoshiAnimFSmash"),
    ("YoshiAttackS4Lw", "Yoshi", "FTYoshiAnimFSmashLow"),
    ("YoshiAttackHi4", "Yoshi", "FTYoshiAnimUSmash"),
    ("YoshiAttackLw4", "Yoshi", "FTYoshiAnimDSmash"),
    ("YoshiAttackAirN", "Yoshi", "FTYoshiAnimAttackAirN"),
    ("YoshiAttackAirF", "Yoshi", "FTYoshiAnimAttackAirF"),
    ("YoshiAttackAirB", "Yoshi", "FTYoshiAnimAttackAirB"),
    ("YoshiAttackAirHi", "Yoshi", "FTYoshiAnimAttackAirU"),
    ("YoshiAttackAirLw", "Yoshi", "FTYoshiAnimAttackAirD"),
    ("YoshiSpecialHi", "Yoshi", "FTYoshiAnimEggThrowGround"),
    ("YoshiSpecialAirHi", "Yoshi", "FTYoshiAnimEggThrowAir"),
    ("YoshiSpecialLwStart", "Yoshi", "FTYoshiAnimGroundPoundGroundStart"),
    ("YoshiSpecialLwLanding", "Yoshi", "FTYoshiAnimGroundPoundLanding"),
    ("YoshiSpecialAirLwStart", "Yoshi", "FTYoshiAnimGroundPoundAir"),
    ("YoshiSpecialN", "Yoshi", "FTYoshiAnimEggLayGrabbedSomeoneStillGoingOut"),
    ("YoshiSpecialNCatch", "Yoshi", "FTYoshiAnimEggLayGrabbedSomeoneStillGoingOut"),
    ("YoshiSpecialNRelease", "Yoshi", "FTYoshiAnimEggLayGrabbedSomeoneComingInAndSwallowing"),
    ("YoshiSpecialAirN", "Yoshi", "FTYoshiAnimEggLayAirGrabOut"),
    ("YoshiSpecialAirNCatch", "Yoshi", "FTYoshiAnimEggLayAirGrabOut"),
    ("YoshiSpecialAirNRelease", "Yoshi", "FTYoshiAnimEggLayAirGrabIn"),
]

# Captain Falcon's five forward tilts, three forward smashes and extended
# statuses in `ftCaptainStatus` order (the four entry animations are omitted).
LATE_SPECIAL_SLOTS += [
    ("CaptainAttack11", "Captain", "FTCaptainAnimJab1"),
    ("CaptainAttack12", "Captain", "FTCaptainAnimJab2"),
    ("CaptainAttackDash", "Captain", "FTCaptainAnimDashAttack"),
    ("CaptainAttackS3Hi", "Captain", "FTCaptainAnimFTiltHigh"),
    ("CaptainAttackS3HiS", "Captain", "FTCaptainAnimFTiltMidHigh"),
    ("CaptainAttackS3", "Captain", "FTCaptainAnimFTilt"),
    ("CaptainAttackS3LwS", "Captain", "FTCaptainAnimFTiltMidLow"),
    ("CaptainAttackS3Lw", "Captain", "FTCaptainAnimFTiltLow"),
    ("CaptainAttackHi3", "Captain", "FTCaptainAnimUTilt"),
    ("CaptainAttackLw3", "Captain", "FTCaptainAnimDTilt"),
    ("CaptainAttackS4Hi", "Captain", "FTCaptainAnimFSmashHigh"),
    ("CaptainAttackS4", "Captain", "FTCaptainAnimFSmash"),
    ("CaptainAttackS4Lw", "Captain", "FTCaptainAnimFSmashLow"),
    ("CaptainAttackHi4", "Captain", "FTCaptainAnimUSmash"),
    ("CaptainAttackLw4", "Captain", "FTCaptainAnimDSmash"),
    ("CaptainAttackAirN", "Captain", "FTCaptainAnimAttackAirN"),
    ("CaptainAttackAirF", "Captain", "FTCaptainAnimAttackAirF"),
    ("CaptainAttackAirB", "Captain", "FTCaptainAnimAttackAirB"),
    ("CaptainAttackAirHi", "Captain", "FTCaptainAnimAttackAirU"),
    ("CaptainAttackAirLw", "Captain", "FTCaptainAnimAttackAirD"),
    ("CaptainAttack13", "Captain", "FTCaptainAnimJab3"),
    ("CaptainAttack100Start", "Captain", "FTCaptainAnimJabLoopStart"),
    ("CaptainAttack100Loop", "Captain", "FTCaptainAnimJabLoop"),
    ("CaptainAttack100End", "Captain", "FTCaptainAnimJabLoopEnd"),
    ("CaptainSpecialN", "Captain", "FTCaptainAnimFalconPunchGround"),
    ("CaptainSpecialAirN", "Captain", "FTCaptainAnimFalconPunchAir"),
    ("CaptainSpecialLw", "Captain", "FTCaptainAnimDownSpecial"),
    ("CaptainSpecialLwAir", "Captain", "FTCaptainAnimVelocityXDownSpecialAir"),
    ("CaptainSpecialLwLanding", "Captain", "FTCaptainAnimLandingDownSpecial"),
    ("CaptainSpecialAirLw", "Captain", "FTCaptainAnimDownSpecialAir"),
    ("CaptainSpecialLwBound", "Captain", "FTCaptainAnimVelocityXDownSpecialAir"),
    ("CaptainSpecialHi", "Captain", "FTCaptainAnimFalconDive"),
    ("CaptainSpecialHiCatch", "Captain", "FTCaptainAnimCatchingEnemyWhileDiving"),
    ("CaptainSpecialHiThrow", "Captain", "FTCaptainAnimFalconDiveEnd1"),
    ("CaptainSpecialAirHi", "Captain", "FTCaptainAnimFalconDive"),
]

# Kirby's common attacks, the two dedicated aerial landings, then his own
# statuses in `ftKirbyStatus` order without Appear and the copy abilities
# (`228_KirbyMainMotion.c`, `dFTKirbyMotionDescs`). He has three forward
# tilts and one forward smash. `SpecialNCatch` has motion -1 and keeps the
# loop's figatree, and `SpecialAirLwFall`'s motion names no figatree, so
# neither has a slot.
LATE_SPECIAL_SLOTS += [
    ("KirbyAttack11", "Kirby", "FTKirbyAnimJab1"),
    ("KirbyAttack12", "Kirby", "FTKirbyAnimJab2"),
    ("KirbyAttackDash", "Kirby", "FTKirbyAnimDashAttack"),
    ("KirbyAttackS3Hi", "Kirby", "FTKirbyAnimFTiltHigh"),
    ("KirbyAttackS3", "Kirby", "FTKirbyAnimFTilt"),
    ("KirbyAttackS3Lw", "Kirby", "FTKirbyAnimFTiltLow"),
    ("KirbyAttackHi3", "Kirby", "FTKirbyAnimUTilt"),
    ("KirbyAttackLw3", "Kirby", "FTKirbyAnimDTilt"),
    ("KirbyAttackS4", "Kirby", "FTKirbyAnimFSmash"),
    ("KirbyAttackHi4", "Kirby", "FTKirbyAnimUSmash"),
    ("KirbyAttackLw4", "Kirby", "FTKirbyAnimDSmash"),
    ("KirbyAttackAirN", "Kirby", "FTKirbyAnimAttackAirN"),
    ("KirbyAttackAirF", "Kirby", "FTKirbyAnimAttackAirF"),
    ("KirbyAttackAirB", "Kirby", "FTKirbyAnimAttackAirB"),
    ("KirbyAttackAirHi", "Kirby", "FTKirbyAnimAttackAirU"),
    ("KirbyAttackAirLw", "Kirby", "FTKirbyAnimAttackAirD"),
    ("KirbyLandingAirF", "Kirby", "FTKirbyAnimLandingAirF"),
    ("KirbyLandingAirB", "Kirby", "FTKirbyAnimLandingAirB"),
    ("KirbyAttack100Start", "Kirby", "FTKirbyAnimJabLoopStart"),
    ("KirbyAttack100Loop", "Kirby", "FTKirbyAnimJabLoop"),
    ("KirbyAttack100End", "Kirby", "FTKirbyAnimJabLoopEnd"),
    ("KirbyJumpAerialF1", "Kirby", "FTKirbyAnimJump2"),
    ("KirbyJumpAerialF2", "Kirby", "FTKirbyAnimJump3"),
    ("KirbyJumpAerialF3", "Kirby", "FTKirbyAnimJump4"),
    ("KirbyJumpAerialF4", "Kirby", "FTKirbyAnimJump5"),
    ("KirbyJumpAerialF5", "Kirby", "FTKirbyAnimJump6"),
    ("KirbyThrowF", "Kirby", "FTKirbyAnimForwardThrow"),
    ("KirbyThrowFFall", "Kirby", "FTKirbyAnimForwardThrowFall"),
    ("KirbyThrowFLanding", "Kirby", "FTKirbyAnimForwardThrowRecoil"),
    ("KirbySpecialHi", "Kirby", "FTKirbyAnimFinalCutter"),
    ("KirbySpecialHiLanding", "Kirby", "FTKirbyAnimFinalCutterLand"),
    ("KirbySpecialAirHi", "Kirby", "FTKirbyAnimFinalCutter"),
    ("KirbySpecialAirHiFall", "Kirby", "FTKirbyAnimFinalCutterImpact"),
    ("KirbySpecialLwStart", "Kirby", "FTKirbyAnimStoneStartGround"),
    ("KirbySpecialLwUnk", "Kirby", "FTKirbyAnimStoneGround"),
    ("KirbySpecialLwHold", "Kirby", "FTKirbyAnimStoneGround"),
    ("KirbySpecialLwEnd", "Kirby", "FTKirbyAnimStoneCancel"),
    ("KirbySpecialAirLwStart", "Kirby", "FTKirbyAnimStoneStartAir"),
    ("KirbySpecialAirLwHold", "Kirby", "FTKirbyAnimStoneGround"),
    ("KirbySpecialAirLwLanding", "Kirby", "FTKirbyAnimStoneGround"),
    ("KirbySpecialAirLwEnd", "Kirby", "FTKirbyAnimStoneCancel"),
    ("KirbySpecialNStart", "Kirby", "FTKirbyAnimInhaleStartGround"),
    ("KirbySpecialNLoop", "Kirby", "FTKirbyAnimInhaleGround"),
    ("KirbySpecialNEnd", "Kirby", "FTKirbyAnimInhaleEnd"),
    ("KirbySpecialNEat", "Kirby", "FTKirbyAnimInhaleSwallowed"),
    ("KirbySpecialNThrow", "Kirby", "FTKirbyAnimInhaleSpit"),
    ("KirbySpecialNWait", "Kirby", "FTKirbyAnimInhaleStuffed"),
    ("KirbySpecialNTurn", "Kirby", "FTKirbyAnimInhaleTurn"),
    ("KirbySpecialNCopy", "Kirby", "FTKirbyAnimInhaleAbsorb"),
]

# Kirby's copy abilities for the ported fighters, one slot per figatree in
# `dFTKirbyMotionDescs` order (Mario and Luigi share theirs; Giant Punch's
# End and Full statuses share theirs; each Egg Lay catch reuses its lay).
# The decompilation's file names describe the motions only loosely: the
# aerial Egg Lay is `EggThrowAir` and its release `EggThrowEndAir`.
LATE_SPECIAL_SLOTS += [
    ("KirbyCopyMarioSpecialN", "Kirby", "FTKirbyAnimLuigiFireballGround"),
    ("KirbyCopyMarioSpecialAirN", "Kirby", "FTKirbyAnimLuigiFireballAir"),
    ("KirbyCopyFoxSpecialN", "Kirby", "FTKirbyAnimLaserGround"),
    ("KirbyCopyFoxSpecialAirN", "Kirby", "FTKirbyAnimLaserAir"),
    ("KirbyCopySamusSpecialNStart", "Kirby", "FTKirbyAnimChargeShotStart"),
    ("KirbyCopySamusSpecialNLoop", "Kirby", "FTKirbyAnimCharging"),
    ("KirbyCopySamusSpecialNEnd", "Kirby", "FTKirbyAnimShootingChargeShot"),
    ("KirbyCopySamusSpecialAirNStart", "Kirby", "FTKirbyAnimChargeShotAir"),
    ("KirbyCopySamusSpecialAirNEnd", "Kirby", "FTKirbyAnimShootingChargeShotAir"),
    ("KirbyCopyDonkeySpecialNStart", "Kirby", "FTKirbyAnimChargePunchStartGround"),
    ("KirbyCopyDonkeySpecialAirNStart", "Kirby", "FTKirbyAnimChargeStartAir"),
    ("KirbyCopyDonkeySpecialNLoop", "Kirby", "FTKirbyAnimChargePunchGround"),
    ("KirbyCopyDonkeySpecialAirNLoop", "Kirby", "FTKirbyAnimChargePunchAir"),
    ("KirbyCopyDonkeySpecialNEnd", "Kirby", "FTKirbyAnimChargePunchGroundFull"),
    ("KirbyCopyDonkeySpecialAirNEnd", "Kirby", "FTKirbyAnimChargePunchAirFull"),
    ("KirbyCopyLinkSpecialN", "Kirby", "FTKirbyAnimBoomerangMiss"),
    ("KirbyCopyLinkSpecialNGet", "Kirby", "FTKirbyAnimBoomerangCatch"),
    ("KirbyCopyLinkSpecialAirN", "Kirby", "FTKirbyAnimBoomerangAirMiss"),
    ("KirbyCopyLinkSpecialAirNReturn", "Kirby", "FTKirbyAnimBoomerangAirCatch"),
    ("KirbyCopyCaptainSpecialN", "Kirby", "FTKirbyAnimFalconPunchGround"),
    ("KirbyCopyCaptainSpecialAirN", "Kirby", "FTKirbyAnimFalconPunchAir"),
    ("KirbyCopyYoshiSpecialN", "Kirby", "FTKirbyAnimEggLayGround"),
    ("KirbyCopyYoshiSpecialNRelease", "Kirby", "FTKirbyAnimEggThrowGround"),
    ("KirbyCopyYoshiSpecialAirN", "Kirby", "FTKirbyAnimEggThrowAir"),
    ("KirbyCopyYoshiSpecialAirNRelease", "Kirby", "FTKirbyAnimEggThrowEndAir"),
]

LATE_SPECIAL_SLOTS += [
    ('PikachuAttack11', 'Pikachu', 'FTPikachuAnimJab1'),
    ('PikachuAttackDash', 'Pikachu', 'FTPikachuAnimDashAttack'),
    ('PikachuAttackS3Hi', 'Pikachu', 'FTPikachuAnimFTiltHigh'),
    ('PikachuAttackS3', 'Pikachu', 'FTPikachuAnimFTilt'),
    ('PikachuAttackS3Lw', 'Pikachu', 'FTPikachuAnimFTiltLow'),
    ('PikachuAttackHi3', 'Pikachu', 'FTPikachuAnimUTilt'),
    ('PikachuAttackLw3', 'Pikachu', 'FTPikachuAnimDTilt'),
    ('PikachuAttackS4', 'Pikachu', 'FTPikachuAnimFSmash'),
    ('PikachuAttackHi4', 'Pikachu', 'FTPikachuAnimUSmash'),
    ('PikachuAttackLw4', 'Pikachu', 'FTPikachuAnimDSmash'),
    ('PikachuAttackAirN', 'Pikachu', 'FTPikachuAnimAttackAirN'),
    ('PikachuAttackAirF', 'Pikachu', 'FTPikachuAnimAttackAirF'),
    ('PikachuAttackAirB', 'Pikachu', 'FTPikachuAnimAttackAirB'),
    ('PikachuAttackAirHi', 'Pikachu', 'FTPikachuAnimAttackAirU'),
    ('PikachuAttackAirLw', 'Pikachu', 'FTPikachuAnimAttackAirD'),
    ('PikachuLandingAirF', 'Pikachu', 'FTPikachuAnimLandingAirF'),
    ('PikachuLandingAirLw', 'Pikachu', 'FTPikachuAnimLandingAirD'),
    ('PikachuSpecialN', 'Pikachu', 'FTPikachuAnimNeutralSpecialGround'),
    ('PikachuSpecialAirN', 'Pikachu', 'FTPikachuAnimNeutralSpecialAir'),
    ('PikachuSpecialLwStart', 'Pikachu', 'FTPikachuAnimDownSpecialStart'),
    ('PikachuSpecialLwLoop', 'Pikachu', 'FTPikachuAnimGettingThundered'),
    ('PikachuSpecialLwHit', 'Pikachu', 'FTPikachuAnimGettingThundered'),
    ('PikachuSpecialLwEnd', 'Pikachu', 'FTPikachuAnimDownSpecialEnd'),
    ('PikachuSpecialAirLwStart', 'Pikachu', 'FTPikachuAnimDownSpecialStartAir'),
    ('PikachuSpecialAirLwLoop', 'Pikachu', 'FTPikachuAnimDownSpecialThunderedAir'),
    ('PikachuSpecialAirLwHit', 'Pikachu', 'FTPikachuAnimDownSpecialThunderedAir'),
    ('PikachuSpecialAirLwEnd', 'Pikachu', 'FTPikachuAnimDownSpecialEndAir'),
    ('PikachuSpecialHi', 'Pikachu', 'FTPikachuAnimUpSpecialEnd'),
    ('PikachuSpecialHiEnd', 'Pikachu', 'FTPikachuAnimUpSpecialEnd'),
    ('PikachuSpecialAirHi', 'Pikachu', 'FTPikachuAnimUpSpecialAirEnd'),
    ('PikachuSpecialAirHiEnd', 'Pikachu', 'FTPikachuAnimUpSpecialAirEnd'),
    ('KirbyCopyPikachuSpecialN', 'Kirby', 'FTKirbyAnimThunderJoltGround'),
    ('KirbyCopyPikachuSpecialAirN', 'Kirby', 'FTKirbyAnimThunderJoltAir'),
]

# Jigglypuff's common attacks, the two dedicated aerial landings (the forward
# one names Kirby's figatree), then her own statuses in `ftPurinStatus` order
# without the unreachable rapid jab and Appear (`232_PurinMainMotion.c`,
# `dFTPurinMotionDescs`). Rest and Sing have one figatree each, named after
# their aerial use; the grounded statuses share them.
LATE_SPECIAL_SLOTS += [
    ("PurinAttack11", "Purin", "FTPurinAnimJab1"),
    ("PurinAttack12", "Purin", "FTPurinAnimJab2"),
    ("PurinAttackDash", "Purin", "FTPurinAnimDashAttack"),
    ("PurinAttackS3Hi", "Purin", "FTPurinAnimFTiltHigh"),
    ("PurinAttackS3", "Purin", "FTPurinAnimFTilt"),
    ("PurinAttackS3Lw", "Purin", "FTPurinAnimFTiltLow"),
    ("PurinAttackHi3", "Purin", "FTPurinAnimUTilt"),
    ("PurinAttackLw3", "Purin", "FTPurinAnimDTilt"),
    ("PurinAttackS4", "Purin", "FTPurinAnimFSmash"),
    ("PurinAttackHi4", "Purin", "FTPurinAnimUSmash"),
    ("PurinAttackLw4", "Purin", "FTPurinAnimDSmash"),
    ("PurinAttackAirN", "Purin", "FTPurinAnimAttackAirN"),
    ("PurinAttackAirF", "Purin", "FTPurinAnimAttackAirF"),
    ("PurinAttackAirB", "Purin", "FTPurinAnimAttackAirB"),
    ("PurinAttackAirHi", "Purin", "FTPurinAnimAttackAirU"),
    ("PurinAttackAirLw", "Purin", "FTPurinAnimAttackAirD"),
    ("PurinLandingAirF", "Purin", "FTKirbyAnimLandingAirF"),
    ("PurinLandingAirB", "Purin", "FTPurinAnimLandingAirB"),
    ("PurinJumpAerialF1", "Purin", "FTPurinAnimJump2"),
    ("PurinJumpAerialF2", "Purin", "FTPurinAnimJump3"),
    ("PurinJumpAerialF3", "Purin", "FTPurinAnimJump4"),
    ("PurinJumpAerialF4", "Purin", "FTPurinAnimJump5"),
    ("PurinJumpAerialF5", "Purin", "FTPurinAnimJump6"),
    ("PurinSpecialN", "Purin", "FTPurinAnimPoundGround"),
    ("PurinSpecialAirN", "Purin", "FTPurinAnimPoundAir"),
    ("PurinSpecialHi", "Purin", "FTPurinAnimSingAir"),
    ("PurinSpecialLw", "Purin", "FTPurinAnimRestAir"),
    # Kirby's Pound names Jigglypuff's figatrees (`dFTKirbyMotionDescs`).
    ("KirbyCopyPurinSpecialN", "Kirby", "FTPurinAnimPoundGround"),
    ("KirbyCopyPurinSpecialAirN", "Kirby", "FTPurinAnimPoundAir"),
]

# Shared statuses added after the per-fighter slots, so earlier slot numbers
# stay stable. They resolve through the common status -> motion pairing, for
# the same fighters as `GRAB_SLOTS`.
LATE_COMMON_SLOTS = [
    ("FuraSleep", 165),
]

# Ness follows FuraSleep to preserve every existing slot ordinal.
POST_SPECIAL_SLOTS = [
    ("NessAttack11", "Ness", "FTNessAnimJab1"),
    ("NessAttack12", "Ness", "FTNessAnimJab2"),
    ("NessAttackDash", "Ness", "FTNessAnimDashAttack"),
    ("NessAttackS3Hi", "Ness", "FTNessAnimFTiltHigh"),
    ("NessAttackS3", "Ness", "FTNessAnimFTilt"),
    ("NessAttackS3Lw", "Ness", "FTNessAnimFTiltLow"),
    ("NessAttackHi3", "Ness", "FTNessAnimUTilt"),
    ("NessAttackLw3", "Ness", "FTNessAnimDTilt"),
    ("NessAttackS4", "Ness", "FTNessAnimFSmash"),
    ("NessAttackHi4", "Ness", "FTNessAnimUSmash"),
    ("NessAttackLw4", "Ness", "FTNessAnimDSmash"),
    ("NessAttackAirN", "Ness", "FTNessAnimAttackAirN"),
    ("NessAttackAirF", "Ness", "FTNessAnimAttackAirF"),
    ("NessAttackAirB", "Ness", "FTNessAnimAttackAirB"),
    ("NessAttackAirHi", "Ness", "FTNessAnimAttackAirU"),
    ("NessAttackAirLw", "Ness", "FTNessAnimAttackAirD"),
    ("NessLandingAirF", "Ness", "FTNessAnimLandingAirF"),
    ("NessLandingAirB", "Ness", "FTNessAnimLandingAirB"),
    ("NessLandingAirLw", "Ness", "FTNessAnimLandingAirX"),
    ("NessAttack13", "Ness", "FTNessAnimJab3"),
    ("NessSpecialN", "Ness", "FTNessAnimPKFireGround"),
    ("NessSpecialAirN", "Ness", "FTNessAnimPKFireAir"),
    ("NessSpecialHiStart", "Ness", "FTNessAnimPKThunderStartGround1"),
    ("NessSpecialHiHold", "Ness", "FTNessAnimPKThunderStartGround2"),
    ("NessSpecialHiEnd", "Ness", "FTNessAnimPKThunderEnd"),
    ("NessSpecialHiJibaku", "Ness", "FTNessAnimDKTAAir"),
    ("NessSpecialAirHiStart", "Ness", "FTNessAnimPKThunderStartAir"),
    ("NessSpecialAirHiHold", "Ness", "FTNessAnimPKThunderAir"),
    ("NessSpecialAirHiEnd", "Ness", "FTNessAnimPKThunderEndAir"),
    ("NessSpecialAirHiBound", "Ness", "FTNessAnimClashingDuringPKTA"),
    ("NessSpecialAirHiJibaku", "Ness", "FTNessAnimDKTAAir"),
    ("NessSpecialLwStart", "Ness", "FTNessAnimDownBStartGround"),
    ("NessSpecialLwHold", "Ness", "FTNessAnimHealingDownB"),
    ("NessSpecialLwHit", "Ness", "FTNessAnimHealingDownB"),
    ("NessSpecialLwEnd", "Ness", "FTNessAnimDownSpecialEndGround"),
    ("NessSpecialAirLwStart", "Ness", "FTNessAnimDownSpecialStartAir"),
    ("NessSpecialAirLwHold", "Ness", "FTNessAnimHealingAirDownB"),
    ("NessSpecialAirLwHit", "Ness", "FTNessAnimHealingAirDownB"),
    ("NessSpecialAirLwEnd", "Ness", "FTNessAnimDownSpecialEndAir"),
    ("KirbyCopyNessSpecialN", "Kirby", "FTKirbyAnimPKFireGround"),
    ("KirbyCopyNessSpecialAirN", "Kirby", "FTKirbyAnimPKFireAir"),
]

# Shared reaction statuses (`ftcommonwalldamage.c`, `ftcommonstopceil.c`,
# `ftcommondown*.c`, `ftcommonpassive*.c`, `ftcommonrebound.c`,
# `ftcommonescape.c`, `ftcommonshieldbreak*.c`, `ftcommonfurafura.c`), in
# `ftCommonStatus` order after every earlier slot so existing ordinals stay
# stable. They resolve through the common status -> motion pairing, for the
# same fighters as `GRAB_SLOTS`. `DownWaitD`/`U` (motion -2) keep the
# bounce's clip and `ReboundWait` (-1) the pose it clanked in, so they have
# no slot.
REACTION_SLOTS = [
    ("WallDamage",        56),
    ("StopCeil",          66),
    ("DownBounceD",       67),
    ("DownBounceU",       68),
    ("DownStandD",        71),
    ("DownStandU",        72),
    ("PassiveStandF",     73),
    ("PassiveStandB",     74),
    ("DownForwardD",      75),
    ("DownForwardU",      76),
    ("DownBackD",         77),
    ("DownBackU",         78),
    ("DownAttackD",       79),
    ("DownAttackU",       80),
    ("Passive",           81),
    ("Rebound",           83),
    ("EscapeF",          156),
    ("EscapeB",          157),
    ("ShieldBreakFly",   158),
    ("ShieldBreakFall",  159),
    ("ShieldBreakDownD", 160),
    ("ShieldBreakDownU", 161),
    ("ShieldBreakStandD", 162),
    ("ShieldBreakStandU", 163),
    ("FuraFura",         164),
]

CLIFF_SLOTS = [(name, status) for status, name in enumerate([
    "CliffCatch", "CliffWait", "CliffQuick", "CliffClimbQuick1",
    "CliffClimbQuick2", "CliffSlow", "CliffClimbSlow1", "CliffClimbSlow2",
    "CliffAttackQuick1", "CliffAttackQuick2", "CliffAttackSlow1", "CliffAttackSlow2",
    "CliffEscapeQuick1", "CliffEscapeQuick2", "CliffEscapeSlow1", "CliffEscapeSlow2",
], 84)]

# The damage statuses (`ftcommondamage.c`, `ftcommondamagefall.c`,
# `ftcommonfallspecial.c`, `ftcommonlandingfallspecial.c`) and the common
# attack statuses from `Appeal` to `LandingAirNull` (`ftcommonattack*.c`,
# `ftcommonlandingair.c`), for every playable fighter through the common
# status -> motion pairing. Several fighters already carry some of the
# attacks in per-fighter slots above; those resolve to the same files, which
# the pack stores once. A null motion (Mario has no mid-angle forward tilt)
# leaves the slot empty. `WallDamage` (56) is a reaction slot already.
COMMON_MOVE_SLOTS = [(f"Damage{name}", status) for status, name in enumerate([
    "Hi1", "Hi2", "Hi3", "N1", "N2", "N3", "Lw1", "Lw2", "Lw3",
    "Air1", "Air2", "Air3", "E1", "E2", "FlyHi", "FlyN", "FlyLw", "FlyTop", "FlyRoll",
], 37)] + [
    ("DamageFall",         57),
    ("FallSpecial",        58),
    ("LandingFallSpecial", 59),
] + [(name, status) for status, name in enumerate([
    "Appeal", "Attack11", "Attack12", "AttackDash",
    "AttackS3Hi", "AttackS3HiS", "AttackS3", "AttackS3LwS", "AttackS3Lw",
    "AttackHi3F", "AttackHi3", "AttackHi3B", "AttackLw3",
    "AttackS4Hi", "AttackS4HiS", "AttackS4", "AttackS4LwS", "AttackS4Lw",
    "AttackHi4", "AttackLw4",
    "AttackAirN", "AttackAirF", "AttackAirB", "AttackAirHi", "AttackAirLw",
    "LandingAirN", "LandingAirF", "LandingAirB", "LandingAirHi", "LandingAirLw",
    "LandingAirNull",
], 189)]

# The jab finisher Mario and Luigi share (`MarioStatus::Attack13`). Luigi's
# earlier slot names the same figatree but is packed for Luigi only; this
# one serves both, so the status needs no per-fighter slot.
FINAL_SPECIAL_SLOTS = [
    ("MarioAttack13", ("Mario", "Luigi"), "FTMarioAnimJab3"),
]

# The remaining shared statuses that name a motion of their own
# (`ftcommonrebirth.c`, `ftcommonwalk.c`, `ftcommonturnrun.c`,
# `ftcommonkneebend.c`, `ftcommonpass.c`, `ftcommonottotto.c`,
# `ftcommontwister.c`, `ftcommondokan.c`, `ftcommonget.c`,
# `ftcommonitemthrow.c`, `ftcommonitemswing.c`, `ftcommonitemshoot.c`,
# `ftcommonhammer*.c`, `ftcommonguard*.c`, `ftcommonthrown*.c`), in
# `ftCommonStatus` order, for every playable fighter through the common
# status -> motion pairing. Statuses with motion -1/-2 (`TaruCann`,
# `DokanWait`, `LiftWait`, `LiftTurn`, `Guard`, `GuardSetOff`,
# `CaptureWaitKirby`) keep the previous clip and have no slot. The five
# hammer statuses after `HammerWalk` all name `HammerWalk`, and
# `CaptureKirby`/`CaptureYoshi` name `DamageFall`/`CapturePulled`, so they
# reuse those slots.
TAIL_COMMON_SLOTS = [
    ("RebirthDown",     7),
    ("RebirthStand",    8),
    ("RebirthWait",     9),
    ("WalkEnd",        14),
    ("TurnRun",        19),
    ("GuardKneeBend",  21),
    ("GuardPass",      34),
    ("OttottoWait",    35),
    ("Ottotto",        36),
    ("Twister",        60),
    ("DokanStart",     62),
    ("DokanEnd",       64),
    ("DokanWalk",      65),
    ("LightGet",      100),
    ("HeavyGet",      101),
] + [(name, status) for status, name in enumerate([
    "LightThrowDrop", "LightThrowDash", "LightThrowF", "LightThrowB",
    "LightThrowHi", "LightThrowLw", "LightThrowF4", "LightThrowB4",
    "LightThrowHi4", "LightThrowLw4", "LightThrowAirF", "LightThrowAirB",
    "LightThrowAirHi", "LightThrowAirLw", "LightThrowAirF4", "LightThrowAirB4",
    "LightThrowAirHi4", "LightThrowAirLw4", "HeavyThrowF", "HeavyThrowB",
    "HeavyThrowF4", "HeavyThrowB4",
    "SwordSwing1", "SwordSwing3", "SwordSwing4", "SwordSwingDash",
    "BatSwing1", "BatSwing3", "BatSwing4", "BatSwingDash",
    "HarisenSwing1", "HarisenSwing3", "HarisenSwing4", "HarisenSwingDash",
    "StarRodSwing1", "StarRodSwing3", "StarRodSwing4", "StarRodSwingDash",
    "LGunShoot", "LGunShootAir", "FireFlowerShoot", "FireFlowerShootAir",
    "HammerWait", "HammerWalk",
], 104)] + [
    ("GuardOn",       152),
    ("GuardOff",      154),
    ("ThrownKirbyStar", 175),
    ("ThrownCopyStar",  176),
    ("YoshiEgg",        178),
    ("CaptureCaptain",  179),
    ("ThrownDonkeyUnk", 180),
]

# The battle-entry clips (`ftCommonAppearSetStatus`, RE-390), last so every
# earlier slot keeps its index. Each is one fighter status, resolved through
# that fighter's own motion enum (`nFT<Name>Motion<Slot>`) into its motion
# table, because the rows sit past `nFTCommonMotionSpecialStart`. They are
# 32-bit `AnimJoint` clips (`FTANIM_FLAG_ANIMJOINT`), not figatrees, so no
# length is read from their C source. Captain Falcon and Ness enter in phases.
APPEAR_SLOTS = ["AppearR", "AppearL", "AppearRStart", "AppearLStart",
                "AppearREnd", "AppearLEnd", "AppearWait"]

# The fighter whose motion enum a table uses.
MOTION_ENUM_OWNER = {"MMario": "Mario", "NMario": "Mario", "NFox": "Fox",
                     "NDonkey": "Donkey", "GDonkey": "Donkey", "NSamus": "Samus",
                     "NLuigi": "Luigi", "NLink": "Link", "NYoshi": "Yoshi",
                     "NCaptain": "Captain", "NKirby": "Kirby",
                     "NPikachu": "Pikachu", "NPurin": "Purin", "NNess": "Ness"}

ALL_SLOTS = (SLOTS + [(name, None, None) for name, _, _ in SPECIAL_SLOTS]
             + [(name, status, None) for name, status in GRAB_SLOTS]
             + [(name, None, None) for name, _, _ in LATE_SPECIAL_SLOTS]
             + [(name, status, None) for name, status in LATE_COMMON_SLOTS]
             + [(name, None, None) for name, _, _ in POST_SPECIAL_SLOTS]
             + [(name, status, None) for name, status in REACTION_SLOTS + CLIFF_SLOTS
                + COMMON_MOVE_SLOTS]
             + [(name, None, None) for name, _, _ in FINAL_SPECIAL_SLOTS]
             + [(name, status, None) for name, status in TAIL_COMMON_SLOTS]
             + [(name, None, None) for name in APPEAR_SLOTS])

# The slots whose animation ends on its own, and whose length the status
# machine therefore reads (RE-035). Everything after them loops until it is
# interrupted -- Wait, the walks, Run, Fall -- so a missing length there is the
# correct answer rather than a fault.
TIMED_SLOTS = {"Dash", "Turn", "RunBrake", "Squat", "SquatRv", "Landing", "Pass"}

# Master Hand never walks, dashes or crouches. Its whole common status table
# points at one looping idle, so it has no lengths to extract and gets zeros.
# Every other fighter must resolve to a finite animation.
NO_GROUND_STATUSES = {"Boss"}

# Fighter order must match ssb_rom::fighter::FIGHTER_FILES.
FIGHTERS = ["Mario", "Fox", "Donkey", "Samus", "Luigi", "Link", "Yoshi",
            "Captain", "Kirby", "Pikachu", "Purin", "Ness", "Boss",
            "MMario", "NMario", "NFox", "NDonkey", "NSamus", "NLuigi",
            "NLink", "NYoshi", "NCaptain", "NKirby", "NPikachu", "NPurin",
            "NNess", "GDonkey"]

ACTION_STATUS_START = 6

# ── AObjEvent16 decoding ────────────────────────────────────────────────
# Mirrors ftAnimParseDObjFigatree. Only the two facts the length depends on
# matter: how many u16s each command consumes, and which ones add to anim_wait.

TRACKS = ["ROTX", "ROTY", "ROTZ", "TRAI", "TRAX", "TRAY", "TRAZ",
          "SCAX", "SCAY", "SCAZ"]
TRACK_BITS = {f"FT_ANIM_{n}": 1 << i for i, n in enumerate(TRACKS)}

# opcode -> u16s read per set track flag
VALUES_PER_TRACK = {2: 1, 3: 1, 4: 2, 5: 2, 6: 1, 7: 1, 8: 1, 9: 1, 10: 1, 11: 0}
# opcodes whose payload is added to anim_wait, i.e. that advance the clock
BLOCK_OPS = {1, 2, 4, 7, 9, 14}
OP_END, OP_TRANSLATE_INTERP, OP_LOOP = 0, 12, 13

MACROS = {
    "ftAnimBlock": (1, 1), "ftAnimBlock0": (1, 0),
    "ftAnimSetValBlockT": (2, 1), "ftAnimSetValBlock": (2, 0),
    "ftAnimSetValT": (3, 1), "ftAnimSetVal": (3, 0),
    "ftAnimSetValRateBlockT": (4, 1), "ftAnimSetValRateBlock": (4, 0),
    "ftAnimSetValRateT": (5, 1), "ftAnimSetValRate": (5, 0),
    "ftAnimSetTargetRateBlockT": (6, 1), "ftAnimSetTargetRateBlock": (6, 0),
    "ftAnimSetTargetRateT": (6, 1), "ftAnimSetTargetRate": (6, 0),
    "ftAnimSetVal0RateBlockT": (7, 1), "ftAnimSetVal0RateBlock": (7, 0),
    "ftAnimSetVal0RateT": (8, 1), "ftAnimSetVal0Rate": (8, 0),
    "ftAnimSetValAfterBlockT": (9, 1), "ftAnimSetValAfterBlock": (9, 0),
    "ftAnimSetValAfterT": (10, 1), "ftAnimSetValAfter": (10, 0),
    "ftAnimSetFlagsT": (14, 1), "ftAnimSetFlags": (14, 0),
}

CALL_RE = re.compile(r"(_?[A-Za-z]\w*)\s*\(")
COMMENT_RE = re.compile(r"/\*.*?\*/|//[^\n]*", re.S)
ARRAY_RE = re.compile(r"^u16\s+(d\w+?_joint\d+)\s*\[(\d+)\]\s*=\s*\{", re.M)


def cmd(op, flags, toggle):
    return ((op << 11) | (flags << 1) | toggle) & 0xFFFF


def evconst(expr):
    expr = expr.strip()
    for name, bit in TRACK_BITS.items():
        expr = expr.replace(name, str(bit))
    return eval(expr, {"__builtins__": {}}, {})


def split_args(text):
    out, depth, cur = [], 0, ""
    for ch in text:
        if ch in "([":
            depth += 1
        elif ch in ")]":
            depth -= 1
        if ch == "," and depth == 0:
            out.append(cur)
            cur = ""
        else:
            cur += ch
    if cur.strip():
        out.append(cur)
    return out


def expand(body):
    """Expand an array initialiser into the u16 words it compiles to."""
    words, i = [], 0
    while i < len(body):
        if body[i] in " \t\r\n,":
            i += 1
            continue
        call = CALL_RE.match(body, i)
        if call:
            depth, j = 1, call.end()
            while depth:
                depth += 1 if body[j] in "([" else -1 if body[j] in ")]" else 0
                j += 1
            name, args = call.group(1), split_args(body[call.end():j - 1])
            if name == "ftAnimEnd":
                words.append(0)
            elif name == "_FT_ANIM_CMD":
                words.append(cmd(*(evconst(a) for a in args[:3])))
            elif name == "ftAnimLoop":
                words += [evconst(args[0]) & 0xFFFF, evconst(args[1]) & 0xFFFF]
            elif name in MACROS:
                op, toggle = MACROS[name]
                words.append(cmd(op, evconst(args[0]), toggle))
                if toggle:
                    words.append(evconst(args[1]) & 0xFFFF)
            else:
                raise ValueError(f"unknown macro {name}")
            i = j
            continue
        lit = re.match(r"[-+]?(0[xX][0-9a-fA-F]+|\d+)", body[i:])
        if not lit:
            raise ValueError(f"unparsable at {body[i:i + 40]!r}")
        words.append(int(lit.group(0), 0) & 0xFFFF)
        i += lit.end()
    return words


def script_frames(words):
    """Frames one joint script runs for, or None when it loops forever.

    Raises when the walk desynchronises rather than returning a plausible
    number: a wrong word-consumption model runs off the end of the script.
    """
    total, i = 0, 0
    while True:
        if i >= len(words):
            raise ValueError("ran off the end without an End command")
        word = words[i]
        op, flags, toggle = word >> 11, (word >> 1) & 0x3FF, word & 1
        i += 1
        if op == OP_END:
            return total
        if op == OP_LOOP:
            return None
        if op == OP_TRANSLATE_INTERP:
            i += 1
            continue
        payload = 0
        if toggle:
            payload, i = words[i], i + 1
        if op in BLOCK_OPS:
            total += payload
        i += VALUES_PER_TRACK.get(op, 0) * bin(flags).count("1")


def file_frames(path):
    """The animation's length, requiring every joint to agree on it."""
    src = COMMENT_RE.sub(" ", open(path).read())
    lengths = []
    for m in ARRAY_RE.finditer(src):
        end = src.index("};", m.end())
        lengths.append(script_frames(expand(src[m.end():end])))
    if not lengths:
        # Some files are transcribed as the joint pointer table plus
        # `_script<N>_<M>` arrays, with unreferenced bytes before the table
        # disassembled as scripts too. Only the scripts the table points at
        # are joints, so follow it.
        table = re.search(r"^u16\s*\*\s*d\w+_ptrs\d+\s*\[\d+\]\s*=\s*\{(.*?)\};", src, re.M | re.S)
        if table:
            for name in re.findall(r"\b(d\w+_script\d+_\d+)\b", table.group(1)):
                m = re.search(r"^u16\s+" + name + r"\s*\[\d+\]\s*=\s*\{", src, re.M)
                end = src.index("};", m.end())
                lengths.append(script_frames(expand(src[m.end():end])))
    if not lengths:
        raise ValueError(f"{path}: no joint scripts")
    if len(set(lengths)) != 1:
        raise ValueError(f"{path}: joints disagree: {sorted(set(lengths))}")
    return lengths[0]


# ── the three pairing records ───────────────────────────────────────────

def motion_enum(refs):
    src = open(os.path.join(refs, "src/ft/ftdef.h")).read()
    body = re.search(r"typedef enum FTCommonMotion\s*\{(.*?)\}", src, re.S).group(1)
    body = COMMENT_RE.sub(" ", body)
    # C enumerator semantics: an explicit value is either a number or an
    # earlier enumerator (`AttackAirN = AttackAirStart`, `AttackAirEnd =
    # AttackAirLw`); anything else is the previous value plus one. Reading
    # the alias's right-hand side as another enumerator would shift every
    # motion after `AttackAirStart` by one per alias.
    out, nxt = {}, 0
    for item in body.split(","):
        m = re.match(r"\s*(nFTCommonMotion\w+)\s*(?:=\s*(\S+))?\s*$", item)
        if not m:
            continue
        name, val = m.groups()
        if val is not None:
            nxt = out[val] if val in out else int(val, 0)
        out[name] = nxt
        nxt += 1
    return out


def status_motions(refs):
    """FTCommonStatus id -> motion_id."""
    path = os.path.join(refs, "src/ft/ftcommon/ftcommonstatus.h")
    src = open(path).read()
    body = src[src.index("FTStatusDesc dFTCommonActionStatusDescs"):]
    enum, out = motion_enum(refs), {}
    parts = re.split(r"//\s*Status (\d+) \(0x[0-9A-Fa-f]+\):", body)
    for i in range(1, len(parts), 2):
        m = re.search(r"(nFTCommonMotion\w+)", parts[i + 1])
        if m:
            out[int(parts[i])] = enum[m.group(1)]
    return out


def motion_descs(refs):
    """Fighter -> [(animation symbol, leading runtime joint) per motion_id].

    An `FTMotionDesc` is three words, and the decompilation spells a table
    both ways: brace groups, and bare `0x0, 0x80000000, 0x80000000,` word
    runs for the null placeholders some fighters carry (Kirby has no
    aerial-jump animation, so motions 18 and 19 are null; Fox and Donkey
    Kong have unbraced nulls among their thrown motions). Grouping the
    flattened words in threes counts both spellings. Matching only the
    entries that name a symbol, or only brace groups, would silently shift
    every later motion_id by the number of holes.
    """
    src = COMMENT_RE.sub(" ", open(os.path.join(refs, "src/ft/ftdata.c")).read())
    out = {}
    for m in re.finditer(r"^FTMotionDesc dFT(\w+)MotionDescs\[\]\s*=\s*$", src, re.M):
        start = src.index("{", m.end())
        body = src[start + 1:src.index("\n};", start)]
        words = [w.strip() for w in body.replace("{", " ").replace("}", " ").split(",")]
        words = [w for w in words if w]
        if len(words) % 3:
            raise ValueError(f"{m.group(1)} motion table: {len(words)} words, not a multiple of 3")
        entries = []
        for i in range(0, len(words), 3):
            sym = re.match(r"&ll(\w+?)FileID$", words[i])
            flags = words[i + 2]
            runtime = bool(re.search(r"FTANIM_FLAG_(?:TRANSN|XROTN|YROTN)_JOINT", flags))
            runtime |= any(int(n, 16) & 0xE0000000 != 0
                           for n in re.findall(r"0x[0-9A-Fa-f]+", flags))
            entries.append((sym.group(1) if sym else None, runtime))
        out[m.group(1)] = entries
    return out


def fighter_motions(refs, name):
    """`nFT<name>Motion*` -> motion id, from the fighter's own header."""
    path = os.path.join(refs, f"src/ft/ftchar/ft{name.lower()}/ft{name.lower()}.h")
    if not os.path.exists(path):
        return {}
    src = COMMENT_RE.sub(" ", open(path).read())
    m = re.search(r"typedef enum \w*Motion\s*\{(.*?)\}", src, re.S)
    if not m:
        return {}
    common = motion_enum(refs)
    out, nxt = {}, 0
    for item in m.group(1).split(","):
        item = item.strip()
        if not item:
            continue
        if "=" in item:
            key, val = (x.strip() for x in item.split("=", 1))
            nxt = common[val] if val in common else out.get(val, int(val, 0))
        else:
            key = item
        out[key] = nxt
        nxt += 1
    return out


def anim_files(refs):
    """Animation symbol -> (relocData file id, filename)."""
    reloc = os.path.join(refs, "src/relocData")
    out = {}
    for name in os.listdir(reloc):
        m = re.match(r"^(\d+)_(FT\w*Anim\w+)\.c$", name)
        if m:
            out[m.group(2)] = (int(m.group(1)), os.path.join(reloc, name))
    return out


def resolve(refs):
    smot, descs, files = status_motions(refs), motion_descs(refs), anim_files(refs)
    cache, rows, problems = {}, [], []
    for fighter in FIGHTERS:
        table = descs[fighter]
        entry = []
        exempt = fighter in NO_GROUND_STATUSES
        for slot, status, allowed in SLOTS:
            sym, runtime = table[smot[status]]
            if sym is None:
                # A null motion is a move the fighter does not have. Kirby and
                # Jigglypuff have no aerial jump, and RE-035 found those exact
                # placeholders. Record the absence rather than failing: the
                # slot gets no file and the runtime keeps the rest pose.
                entry.append((slot, 0, None, 0, False))
                continue
            # `FT<Name>Anim<X>` -> `<X>`
            anim = re.sub(r"^FT\w*?Anim", "", sym)
            if anim not in allowed and not exempt:
                problems.append(
                    f"{fighter} {slot}: resolved {sym}, expected one of {allowed}")
            fid, path = files[sym]
            if fid not in cache:
                cache[fid] = file_frames(path)
            entry.append((slot, fid, sym, cache[fid], runtime))
        def special(slot, target, sym):
            targets = target if isinstance(target, tuple) else (target,)
            if fighter not in targets:
                entry.append((slot, 0, None, 0, False))
                return
            runtime_options = {runtime for name, runtime in table if name == sym}
            if len(runtime_options) != 1:
                problems.append(f"{fighter} {slot}: inconsistent runtime-joint flags for {sym}")
            runtime = next(iter(runtime_options), False)
            fid, path = files[sym]
            if fid not in cache:
                cache[fid] = file_frames(path)
            entry.append((slot, fid, sym, cache[fid], runtime))
        for slot, target, sym in SPECIAL_SLOTS:
            special(slot, target, sym)
        def common(slot, status):
            sym, runtime = table[smot[status]] if fighter in GRAB_FIGHTERS else (None, False)
            if sym is None:
                entry.append((slot, 0, None, 0, False))
                return
            fid, path = files[sym]
            if fid not in cache:
                cache[fid] = file_frames(path)
            entry.append((slot, fid, sym, cache[fid], runtime))
        for slot, status in GRAB_SLOTS:
            common(slot, status)
        for slot, target, sym in LATE_SPECIAL_SLOTS:
            special(slot, target, sym)
        for slot, status in LATE_COMMON_SLOTS:
            common(slot, status)
        for slot, target, sym in POST_SPECIAL_SLOTS:
            special(slot, target, sym)
        for slot, status in REACTION_SLOTS + CLIFF_SLOTS + COMMON_MOVE_SLOTS:
            common(slot, status)
        for slot, target, sym in FINAL_SPECIAL_SLOTS:
            special(slot, target, sym)
        for slot, status in TAIL_COMMON_SLOTS:
            common(slot, status)
        owner = MOTION_ENUM_OWNER.get(fighter, fighter)
        motions = fighter_motions(refs, owner)
        for slot in APPEAR_SLOTS:
            motion = motions.get(f"nFT{owner}Motion{slot}")
            sym, runtime = table[motion] if motion is not None and motion < len(table) else (None, False)
            if sym is None:
                entry.append((slot, 0, None, 0, False))
                continue
            fid, _ = files[sym]
            entry.append((slot, fid, sym, 0, runtime))
        rows.append((fighter, entry))
    return rows, problems


# ── emission ────────────────────────────────────────────────────────────

def emit(rows, out):
    slots = ", ".join(f'"{s}"' for s, _, _ in ALL_SLOTS)
    w = out.write
    w("// Generated by tools/gen-anim-table.py from refs/ssb-decomp-re.\n")
    w("// Do not edit by hand; re-run the generator instead.\n\n")
    w(f"/// The statuses carried, in slot order.\n")
    w(f"pub const SLOT_NAMES: [&str; SLOT_COUNT] = [{slots}];\n\n")
    w("#[rustfmt::skip]\n#[allow(clippy::large_const_arrays)]\npub const FIGHTER_ANIMS: "
      f"[FighterAnims; {len(rows)}] = [\n")
    for fighter, entry in rows:
        ids = ", ".join(f"{fid:4d}" for _, fid, _, _, _ in entry)
        w(f'    FighterAnims {{ name: "{fighter}",{" " * (9 - len(fighter))}'
          f"files: [{ids}] }},\n")
    w("];\n\n")
    w("/// Whether a motion's extra figatree entry is a leading runtime joint.\n")
    w("/// Read from `FTMotionDesc.anim_desc` in `ftdata.c`.\n")
    w("#[rustfmt::skip]\n#[allow(clippy::large_const_arrays)]\npub const LEADING_RUNTIME_JOINT: "
      f"[[bool; SLOT_COUNT]; {len(rows)}] = [\n")
    for fighter, entry in rows:
        flags = ", ".join("true" if runtime else "false" for _, _, _, _, runtime in entry)
        w(f"    [{flags}],  // {fighter}\n")
    w("];\n\n")
    w("/// Lengths the decompilation's own C sources give for the same files.\n")
    w("/// `romtool anims --verify` checks the ROM against these.\n")
    w("#[rustfmt::skip]\n#[allow(clippy::large_const_arrays)]\npub const EXPECTED_FRAMES: "
      f"[[u16; SLOT_COUNT]; {len(rows)}] = [\n")
    for fighter, entry in rows:
        lens = ", ".join(f"{0 if n is None else n:3d}" for _, _, _, n, _ in entry)
        w(f"    [{lens}],  // {fighter}\n")
    w("];\n")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--refs", default=os.path.join(PROJECT, "refs/ssb-decomp-re"))
    ap.add_argument("--out", default="-")
    args = ap.parse_args()
    rows, problems = resolve(args.refs)
    for fighter, entry in rows:
        if fighter in NO_GROUND_STATUSES:
            continue
        for slot, fid, sym, frames, _ in entry:
            if frames is None and slot in TIMED_SLOTS:
                problems.append(f"{sym} (file {fid}, {slot}) loops; it has no length")
    if problems:
        sys.exit("\n".join(problems))
    out = sys.stdout if args.out == "-" else open(args.out, "w")
    emit(rows, out)


if __name__ == "__main__":
    main()
