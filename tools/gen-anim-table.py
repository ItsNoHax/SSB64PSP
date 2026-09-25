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
GRAB_FIGHTERS = {"Mario", "Fox", "Donkey", "Samus", "Luigi", "Link"}
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

ALL_SLOTS = (SLOTS + [(name, None, None) for name, _, _ in SPECIAL_SLOTS]
             + [(name, status, None) for name, status in GRAB_SLOTS]
             + [(name, None, None) for name, _, _ in LATE_SPECIAL_SLOTS])

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
        raise ValueError(f"{path}: no joint scripts")
    if len(set(lengths)) != 1:
        raise ValueError(f"{path}: joints disagree: {sorted(set(lengths))}")
    return lengths[0]


# ── the three pairing records ───────────────────────────────────────────

def motion_enum(refs):
    src = open(os.path.join(refs, "src/ft/ftdef.h")).read()
    body = re.search(r"typedef enum FTCommonMotion\s*\{(.*?)\}", src, re.S).group(1)
    out, nxt = {}, 0
    for name, val in re.findall(r"(nFTCommonMotion\w+)\s*(?:=\s*(-?\d+))?", body):
        nxt = int(val) if val else nxt
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
        for slot, status in GRAB_SLOTS:
            sym, runtime = table[smot[status]] if fighter in GRAB_FIGHTERS else (None, False)
            if sym is None:
                entry.append((slot, 0, None, 0, False))
                continue
            fid, path = files[sym]
            if fid not in cache:
                cache[fid] = file_frames(path)
            entry.append((slot, fid, sym, cache[fid], runtime))
        for slot, target, sym in LATE_SPECIAL_SLOTS:
            special(slot, target, sym)
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
    w("#[rustfmt::skip]\npub const FIGHTER_ANIMS: "
      f"[FighterAnims; {len(rows)}] = [\n")
    for fighter, entry in rows:
        ids = ", ".join(f"{fid:4d}" for _, fid, _, _, _ in entry)
        w(f'    FighterAnims {{ name: "{fighter}",{" " * (9 - len(fighter))}'
          f"files: [{ids}] }},\n")
    w("];\n\n")
    w("/// Whether a motion's extra figatree entry is a leading runtime joint.\n")
    w("/// Read from `FTMotionDesc.anim_desc` in `ftdata.c`.\n")
    w("#[rustfmt::skip]\npub const LEADING_RUNTIME_JOINT: "
      f"[[bool; SLOT_COUNT]; {len(rows)}] = [\n")
    for fighter, entry in rows:
        flags = ", ".join("true" if runtime else "false" for _, _, _, _, runtime in entry)
        w(f"    [{flags}],  // {fighter}\n")
    w("];\n\n")
    w("/// Lengths the decompilation's own C sources give for the same files.\n")
    w("/// `romtool anims --verify` checks the ROM against these.\n")
    w("#[rustfmt::skip]\npub const EXPECTED_FRAMES: "
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
