//! `ssb_rom::anim`'s fighter animation table: for each fighter and each
//! status the port carries (a *slot*), the archive file of its animation,
//! its `FTAnimDesc` hidden-part bits, and the length its figatree runs.
//!
//! The game finds a status's animation through records that name both
//! sides: `FTStatusDesc.mflags.motion_id` (the status tables in `ovl2`),
//! then `dFT<Name>MotionDescs[motion_id].anim_file_id`. The demo statuses go
//! through `D_ovl1_80390BE8` into `dFT<Name>SubMotionDescs`. What is
//! committed here is only each slot's *recipe*: which status, motion id or
//! demo row it reads. The file ids, bits and lengths come from the ROM.

use std::collections::BTreeSet;
use std::fmt::Write;

use crate::emit;
use crate::figatree::Lengths;
use crate::fighters::{self, motion_descs, MotionDesc, ANIM_FLAG_ANIMJOINT};
use crate::{Result, Source};

/// How a slot finds its row.
#[derive(Debug, Clone, Copy)]
enum Recipe {
    /// A common status, for every fighter: its motion id indexes the
    /// fighter's own motion table.
    Status(u16),
    /// A common status, for every fighter but Master Hand.
    Common(u16),
    /// A fighter-specific animation: the file `fighter`'s motion `motion`
    /// names, for each fighter in `targets`; a variant whose base is a
    /// target takes its own row at every motion id where the base names it.
    Special {
        targets: &'static [&'static str],
        fighter: &'static str,
        motion: u16,
    },
    /// Index into [`APPEAR_MOTIONS`]: the battle-entry clips, through the
    /// fighter's own motion enum.
    Appear(usize),
    /// `nFTDemoStatus* - 0x10000`: a results/select clip, through
    /// `D_ovl1_80390BE8` into the fighter's submotion table.
    Demo(u32),
    /// Index into [`BOSS_MOTIONS`].
    Boss(usize),
    /// An opening-movie demo status, as [`Recipe::Demo`].
    OpeningDemo(u32),
    /// An opening-movie row of the submotion table.
    OpeningRow(usize),
}
use Recipe::*;

/// The fighters, in `FTKind` order (`ssb_rom::fighter::FIGHTER_FILES`).
const FIGHTERS: [&str; 27] = [
    "Mario", "Fox", "Donkey", "Samus", "Luigi", "Link", "Yoshi", "Captain", "Kirby", "Pikachu",
    "Purin", "Ness", "Boss", "MMario", "NMario", "NFox", "NDonkey", "NSamus", "NLuigi", "NLink",
    "NYoshi", "NCaptain", "NKirby", "NPikachu", "NPurin", "NNess", "GDonkey",
];

/// Slots whose animation ends on its own: the status machine reads their
/// length, so every fighter but Master Hand must have one.
const TIMED_SLOTS: [&str; 7] = [
    "Dash", "Turn", "RunBrake", "Squat", "SquatRv", "Landing", "Pass",
];

/// `nFT<Owner>Motion{AppearR, AppearL, AppearRStart, AppearLStart,
/// AppearREnd, AppearLEnd, AppearWait}`, by the fighter whose motion enum a
/// table uses.
const APPEAR_MOTIONS: [(&str, [Option<u16>; 7]); 13] = [
    ("Boss", [None, None, None, None, None, None, None]),
    (
        "Captain",
        [None, None, Some(199), Some(200), Some(201), Some(202), None],
    ),
    (
        "Donkey",
        [Some(195), Some(196), None, None, None, None, None],
    ),
    ("Fox", [Some(198), Some(199), None, None, None, None, None]),
    (
        "Kirby",
        [Some(225), Some(226), None, None, None, None, None],
    ),
    ("Link", [Some(199), Some(200), None, None, None, None, None]),
    (
        "Luigi",
        [Some(196), Some(197), None, None, None, None, None],
    ),
    (
        "Mario",
        [Some(196), Some(197), None, None, None, None, None],
    ),
    (
        "Ness",
        [
            None,
            None,
            Some(196),
            Some(197),
            Some(199),
            Some(200),
            Some(198),
        ],
    ),
    (
        "Pikachu",
        [Some(195), Some(196), None, None, None, None, None],
    ),
    (
        "Purin",
        [Some(203), Some(204), None, None, None, None, None],
    ),
    (
        "Samus",
        [Some(195), Some(196), None, None, None, None, None],
    ),
    (
        "Yoshi",
        [Some(195), Some(196), None, None, None, None, None],
    ),
];

/// `nFTBossMotionDefault` .. `nFTBossMotionAppear`.
const BOSS_MOTIONS: [u16; 30] = [
    195, 196, 197, 198, 199, 200, 201, 202, 203, 204, 205, 206, 207, 208, 209, 210, 211, 212, 213,
    214, 215, 216, 217, 218, 219, 220, 221, 222, 223, 224,
];

/// `D_ovl1_80390BE8`: ROM offset of the shared demo status table
/// (`FTOpeningDesc`, 8 bytes each; the first word is `0x10000` plus the
/// submotion row).
const DEMO_STATUS_TABLE: (u32, u32) = (0x10_81C8, 15);

/// `FTAnimDesc` bits 31..5: bit `31 - i` inserts `hiddenparts[i]`.
const HIDDEN_PART_BITS: u32 = 0xFFFF_FFE0;
/// Parts 0-2: the runtime joints `TransN`, `XRotN`, `YRotN`.
const LEADING_BITS: u32 = 0xE000_0000;

/// Every slot, in slot order, with its recipe.
static SLOTS: [(&str, Recipe); 667] = [
    ("Dash", Status(15)),
    ("Turn", Status(18)),
    ("RunBrake", Status(17)),
    ("Squat", Status(28)),
    ("SquatRv", Status(30)),
    ("Landing", Status(31)),
    ("Pass", Status(33)),
    ("Wait", Status(10)),
    ("WalkSlow", Status(11)),
    ("WalkMiddle", Status(12)),
    ("WalkFast", Status(13)),
    ("Run", Status(16)),
    ("KneeBend", Status(20)),
    ("JumpF", Status(22)),
    ("JumpB", Status(23)),
    ("JumpAerialF", Status(24)),
    ("JumpAerialB", Status(25)),
    ("Fall", Status(26)),
    ("FallAerial", Status(27)),
    ("SquatWait", Status(29)),
    (
        "MarioSpecialN",
        Special {
            targets: &["Mario", "Luigi"],
            fighter: "Mario",
            motion: 198,
        },
    ),
    (
        "MarioSpecialAirN",
        Special {
            targets: &["Mario", "Luigi"],
            fighter: "Mario",
            motion: 199,
        },
    ),
    (
        "MarioSpecialHi",
        Special {
            targets: &["Mario", "Luigi"],
            fighter: "Mario",
            motion: 200,
        },
    ),
    (
        "MarioSpecialAirHi",
        Special {
            targets: &["Mario", "Luigi"],
            fighter: "Mario",
            motion: 200,
        },
    ),
    (
        "MarioSpecialLw",
        Special {
            targets: &["Mario", "Luigi"],
            fighter: "Mario",
            motion: 202,
        },
    ),
    (
        "MarioSpecialAirLw",
        Special {
            targets: &["Mario", "Luigi"],
            fighter: "Mario",
            motion: 203,
        },
    ),
    (
        "FoxAttack11",
        Special {
            targets: &["Fox"],
            fighter: "Fox",
            motion: 165,
        },
    ),
    (
        "FoxAttack12",
        Special {
            targets: &["Fox"],
            fighter: "Fox",
            motion: 166,
        },
    ),
    (
        "FoxAttack100Start",
        Special {
            targets: &["Fox"],
            fighter: "Fox",
            motion: 195,
        },
    ),
    (
        "FoxAttack100Loop",
        Special {
            targets: &["Fox"],
            fighter: "Fox",
            motion: 196,
        },
    ),
    (
        "FoxAttack100End",
        Special {
            targets: &["Fox"],
            fighter: "Fox",
            motion: 197,
        },
    ),
    (
        "FoxAttackDash",
        Special {
            targets: &["Fox"],
            fighter: "Fox",
            motion: 167,
        },
    ),
    (
        "FoxAttackS3Hi",
        Special {
            targets: &["Fox"],
            fighter: "Fox",
            motion: 168,
        },
    ),
    (
        "FoxAttackS3HiS",
        Special {
            targets: &["Fox"],
            fighter: "Fox",
            motion: 169,
        },
    ),
    (
        "FoxAttackS3",
        Special {
            targets: &["Fox"],
            fighter: "Fox",
            motion: 170,
        },
    ),
    (
        "FoxAttackS3LwS",
        Special {
            targets: &["Fox"],
            fighter: "Fox",
            motion: 171,
        },
    ),
    (
        "FoxAttackS3Lw",
        Special {
            targets: &["Fox"],
            fighter: "Fox",
            motion: 172,
        },
    ),
    (
        "FoxAttackHi3",
        Special {
            targets: &["Fox"],
            fighter: "Fox",
            motion: 174,
        },
    ),
    (
        "FoxAttackLw3",
        Special {
            targets: &["Fox"],
            fighter: "Fox",
            motion: 176,
        },
    ),
    (
        "FoxAttackS4",
        Special {
            targets: &["Fox"],
            fighter: "Fox",
            motion: 179,
        },
    ),
    (
        "FoxAttackHi4",
        Special {
            targets: &["Fox"],
            fighter: "Fox",
            motion: 182,
        },
    ),
    (
        "FoxAttackLw4",
        Special {
            targets: &["Fox"],
            fighter: "Fox",
            motion: 183,
        },
    ),
    (
        "FoxAttackAirN",
        Special {
            targets: &["Fox"],
            fighter: "Fox",
            motion: 184,
        },
    ),
    (
        "FoxAttackAirF",
        Special {
            targets: &["Fox"],
            fighter: "Fox",
            motion: 185,
        },
    ),
    (
        "FoxAttackAirB",
        Special {
            targets: &["Fox"],
            fighter: "Fox",
            motion: 186,
        },
    ),
    (
        "FoxAttackAirHi",
        Special {
            targets: &["Fox"],
            fighter: "Fox",
            motion: 187,
        },
    ),
    (
        "FoxAttackAirLw",
        Special {
            targets: &["Fox"],
            fighter: "Fox",
            motion: 188,
        },
    ),
    (
        "FoxSpecialN",
        Special {
            targets: &["Fox"],
            fighter: "Fox",
            motion: 200,
        },
    ),
    (
        "FoxSpecialAirN",
        Special {
            targets: &["Fox"],
            fighter: "Fox",
            motion: 201,
        },
    ),
    (
        "FoxSpecialHiStart",
        Special {
            targets: &["Fox"],
            fighter: "Fox",
            motion: 202,
        },
    ),
    (
        "FoxSpecialAirHiStart",
        Special {
            targets: &["Fox"],
            fighter: "Fox",
            motion: 203,
        },
    ),
    (
        "FoxSpecialHiHold",
        Special {
            targets: &["Fox"],
            fighter: "Fox",
            motion: 204,
        },
    ),
    (
        "FoxSpecialAirHiHold",
        Special {
            targets: &["Fox"],
            fighter: "Fox",
            motion: 205,
        },
    ),
    (
        "FoxSpecialHi",
        Special {
            targets: &["Fox"],
            fighter: "Fox",
            motion: 206,
        },
    ),
    (
        "FoxSpecialAirHi",
        Special {
            targets: &["Fox"],
            fighter: "Fox",
            motion: 207,
        },
    ),
    (
        "FoxSpecialHiEnd",
        Special {
            targets: &["Fox"],
            fighter: "Fox",
            motion: 208,
        },
    ),
    (
        "FoxSpecialAirHiEnd",
        Special {
            targets: &["Fox"],
            fighter: "Fox",
            motion: 209,
        },
    ),
    (
        "FoxSpecialAirHiBound",
        Special {
            targets: &["Fox"],
            fighter: "Fox",
            motion: 210,
        },
    ),
    (
        "FoxSpecialLwStart",
        Special {
            targets: &["Fox"],
            fighter: "Fox",
            motion: 211,
        },
    ),
    (
        "FoxSpecialLwTurn",
        Special {
            targets: &["Fox"],
            fighter: "Fox",
            motion: 212,
        },
    ),
    (
        "FoxSpecialLwHit",
        Special {
            targets: &["Fox"],
            fighter: "Fox",
            motion: 213,
        },
    ),
    (
        "FoxSpecialLwLoop",
        Special {
            targets: &["Fox"],
            fighter: "Fox",
            motion: 214,
        },
    ),
    (
        "FoxSpecialAirLwStart",
        Special {
            targets: &["Fox"],
            fighter: "Fox",
            motion: 215,
        },
    ),
    (
        "FoxSpecialAirLwTurn",
        Special {
            targets: &["Fox"],
            fighter: "Fox",
            motion: 216,
        },
    ),
    (
        "FoxSpecialAirLwHit",
        Special {
            targets: &["Fox"],
            fighter: "Fox",
            motion: 217,
        },
    ),
    (
        "FoxSpecialAirLwLoop",
        Special {
            targets: &["Fox"],
            fighter: "Fox",
            motion: 218,
        },
    ),
    (
        "FoxSpecialLwEnd",
        Special {
            targets: &["Fox"],
            fighter: "Fox",
            motion: 214,
        },
    ),
    (
        "FoxSpecialAirLwEnd",
        Special {
            targets: &["Fox"],
            fighter: "Fox",
            motion: 218,
        },
    ),
    (
        "DonkeyAttack11",
        Special {
            targets: &["Donkey"],
            fighter: "Donkey",
            motion: 165,
        },
    ),
    (
        "DonkeyAttack12",
        Special {
            targets: &["Donkey"],
            fighter: "Donkey",
            motion: 166,
        },
    ),
    (
        "DonkeyAttackDash",
        Special {
            targets: &["Donkey"],
            fighter: "Donkey",
            motion: 167,
        },
    ),
    (
        "DonkeyAttackS3Hi",
        Special {
            targets: &["Donkey"],
            fighter: "Donkey",
            motion: 168,
        },
    ),
    (
        "DonkeyAttackS3",
        Special {
            targets: &["Donkey"],
            fighter: "Donkey",
            motion: 170,
        },
    ),
    (
        "DonkeyAttackS3Lw",
        Special {
            targets: &["Donkey"],
            fighter: "Donkey",
            motion: 172,
        },
    ),
    (
        "DonkeyAttackHi3",
        Special {
            targets: &["Donkey"],
            fighter: "Donkey",
            motion: 174,
        },
    ),
    (
        "DonkeyAttackLw3",
        Special {
            targets: &["Donkey"],
            fighter: "Donkey",
            motion: 176,
        },
    ),
    (
        "DonkeyAttackS4Hi",
        Special {
            targets: &["Donkey"],
            fighter: "Donkey",
            motion: 177,
        },
    ),
    (
        "DonkeyAttackS4HiS",
        Special {
            targets: &["Donkey"],
            fighter: "Donkey",
            motion: 178,
        },
    ),
    (
        "DonkeyAttackS4",
        Special {
            targets: &["Donkey"],
            fighter: "Donkey",
            motion: 179,
        },
    ),
    (
        "DonkeyAttackS4LwS",
        Special {
            targets: &["Donkey"],
            fighter: "Donkey",
            motion: 180,
        },
    ),
    (
        "DonkeyAttackS4Lw",
        Special {
            targets: &["Donkey"],
            fighter: "Donkey",
            motion: 181,
        },
    ),
    (
        "DonkeyAttackHi4",
        Special {
            targets: &["Donkey"],
            fighter: "Donkey",
            motion: 182,
        },
    ),
    (
        "DonkeyAttackLw4",
        Special {
            targets: &["Donkey"],
            fighter: "Donkey",
            motion: 183,
        },
    ),
    (
        "DonkeyAttackAirN",
        Special {
            targets: &["Donkey"],
            fighter: "Donkey",
            motion: 184,
        },
    ),
    (
        "DonkeyAttackAirF",
        Special {
            targets: &["Donkey"],
            fighter: "Donkey",
            motion: 185,
        },
    ),
    (
        "DonkeyAttackAirB",
        Special {
            targets: &["Donkey"],
            fighter: "Donkey",
            motion: 186,
        },
    ),
    (
        "DonkeyAttackAirHi",
        Special {
            targets: &["Donkey"],
            fighter: "Donkey",
            motion: 187,
        },
    ),
    (
        "DonkeyAttackAirLw",
        Special {
            targets: &["Donkey"],
            fighter: "Donkey",
            motion: 188,
        },
    ),
    (
        "DonkeySpecialNStart",
        Special {
            targets: &["Donkey"],
            fighter: "Donkey",
            motion: 197,
        },
    ),
    (
        "DonkeySpecialAirNStart",
        Special {
            targets: &["Donkey"],
            fighter: "Donkey",
            motion: 198,
        },
    ),
    (
        "DonkeySpecialNLoop",
        Special {
            targets: &["Donkey"],
            fighter: "Donkey",
            motion: 199,
        },
    ),
    (
        "DonkeySpecialAirNLoop",
        Special {
            targets: &["Donkey"],
            fighter: "Donkey",
            motion: 200,
        },
    ),
    (
        "DonkeySpecialNEnd",
        Special {
            targets: &["Donkey"],
            fighter: "Donkey",
            motion: 201,
        },
    ),
    (
        "DonkeySpecialAirNEnd",
        Special {
            targets: &["Donkey"],
            fighter: "Donkey",
            motion: 202,
        },
    ),
    (
        "DonkeySpecialHi",
        Special {
            targets: &["Donkey"],
            fighter: "Donkey",
            motion: 205,
        },
    ),
    (
        "DonkeySpecialAirHi",
        Special {
            targets: &["Donkey"],
            fighter: "Donkey",
            motion: 206,
        },
    ),
    (
        "DonkeySpecialLwStart",
        Special {
            targets: &["Donkey"],
            fighter: "Donkey",
            motion: 207,
        },
    ),
    (
        "DonkeySpecialLwLoop",
        Special {
            targets: &["Donkey"],
            fighter: "Donkey",
            motion: 208,
        },
    ),
    (
        "DonkeySpecialLwEnd",
        Special {
            targets: &["Donkey"],
            fighter: "Donkey",
            motion: 209,
        },
    ),
    (
        "DonkeyThrowFWait",
        Special {
            targets: &["Donkey"],
            fighter: "Donkey",
            motion: 210,
        },
    ),
    (
        "DonkeyThrowFWalkSlow",
        Special {
            targets: &["Donkey"],
            fighter: "Donkey",
            motion: 211,
        },
    ),
    (
        "DonkeyThrowFWalkMiddle",
        Special {
            targets: &["Donkey"],
            fighter: "Donkey",
            motion: 212,
        },
    ),
    (
        "DonkeyThrowFWalkFast",
        Special {
            targets: &["Donkey"],
            fighter: "Donkey",
            motion: 213,
        },
    ),
    (
        "DonkeyThrowFTurn",
        Special {
            targets: &["Donkey"],
            fighter: "Donkey",
            motion: 214,
        },
    ),
    (
        "DonkeyThrowFKneeBend",
        Special {
            targets: &["Donkey"],
            fighter: "Donkey",
            motion: 210,
        },
    ),
    (
        "DonkeyThrowFFall",
        Special {
            targets: &["Donkey"],
            fighter: "Donkey",
            motion: 210,
        },
    ),
    (
        "DonkeyThrowFLanding",
        Special {
            targets: &["Donkey"],
            fighter: "Donkey",
            motion: 210,
        },
    ),
    (
        "DonkeyThrowFDamage",
        Special {
            targets: &["Donkey"],
            fighter: "Donkey",
            motion: 210,
        },
    ),
    (
        "DonkeyThrowFF",
        Special {
            targets: &["Donkey"],
            fighter: "Donkey",
            motion: 108,
        },
    ),
    (
        "DonkeyThrowAirFF",
        Special {
            targets: &["Donkey"],
            fighter: "Donkey",
            motion: 108,
        },
    ),
    ("Catch", Common(166)),
    ("CatchPull", Common(167)),
    ("ThrowF", Common(169)),
    ("ThrowB", Common(170)),
    ("CapturePulled", Common(171)),
    ("ThrownDonkeyF", Common(181)),
    ("ThrownMarioBStart", Common(182)),
    ("ThrownFoxFStart", Common(183)),
    ("Shouldered", Common(184)),
    ("ThrownMarioB", Common(185)),
    ("ThrownCommon", Common(186)),
    ("ThrownFoxF", Common(187)),
    ("ThrownFoxB", Common(188)),
    (
        "SamusAttack11",
        Special {
            targets: &["Samus"],
            fighter: "Samus",
            motion: 165,
        },
    ),
    (
        "SamusAttack12",
        Special {
            targets: &["Samus"],
            fighter: "Samus",
            motion: 166,
        },
    ),
    (
        "SamusAttackDash",
        Special {
            targets: &["Samus"],
            fighter: "Samus",
            motion: 167,
        },
    ),
    (
        "SamusAttackS3Hi",
        Special {
            targets: &["Samus"],
            fighter: "Samus",
            motion: 168,
        },
    ),
    (
        "SamusAttackS3HiS",
        Special {
            targets: &["Samus"],
            fighter: "Samus",
            motion: 169,
        },
    ),
    (
        "SamusAttackS3",
        Special {
            targets: &["Samus"],
            fighter: "Samus",
            motion: 170,
        },
    ),
    (
        "SamusAttackS3LwS",
        Special {
            targets: &["Samus"],
            fighter: "Samus",
            motion: 171,
        },
    ),
    (
        "SamusAttackS3Lw",
        Special {
            targets: &["Samus"],
            fighter: "Samus",
            motion: 172,
        },
    ),
    (
        "SamusAttackHi3",
        Special {
            targets: &["Samus"],
            fighter: "Samus",
            motion: 174,
        },
    ),
    (
        "SamusAttackLw3",
        Special {
            targets: &["Samus"],
            fighter: "Samus",
            motion: 176,
        },
    ),
    (
        "SamusAttackS4Hi",
        Special {
            targets: &["Samus"],
            fighter: "Samus",
            motion: 177,
        },
    ),
    (
        "SamusAttackS4HiS",
        Special {
            targets: &["Samus"],
            fighter: "Samus",
            motion: 178,
        },
    ),
    (
        "SamusAttackS4",
        Special {
            targets: &["Samus"],
            fighter: "Samus",
            motion: 179,
        },
    ),
    (
        "SamusAttackS4LwS",
        Special {
            targets: &["Samus"],
            fighter: "Samus",
            motion: 180,
        },
    ),
    (
        "SamusAttackS4Lw",
        Special {
            targets: &["Samus"],
            fighter: "Samus",
            motion: 181,
        },
    ),
    (
        "SamusAttackHi4",
        Special {
            targets: &["Samus"],
            fighter: "Samus",
            motion: 182,
        },
    ),
    (
        "SamusAttackLw4",
        Special {
            targets: &["Samus"],
            fighter: "Samus",
            motion: 183,
        },
    ),
    (
        "SamusAttackAirN",
        Special {
            targets: &["Samus"],
            fighter: "Samus",
            motion: 184,
        },
    ),
    (
        "SamusAttackAirF",
        Special {
            targets: &["Samus"],
            fighter: "Samus",
            motion: 185,
        },
    ),
    (
        "SamusAttackAirB",
        Special {
            targets: &["Samus"],
            fighter: "Samus",
            motion: 186,
        },
    ),
    (
        "SamusAttackAirHi",
        Special {
            targets: &["Samus"],
            fighter: "Samus",
            motion: 187,
        },
    ),
    (
        "SamusAttackAirLw",
        Special {
            targets: &["Samus"],
            fighter: "Samus",
            motion: 188,
        },
    ),
    (
        "SamusSpecialNStart",
        Special {
            targets: &["Samus"],
            fighter: "Samus",
            motion: 197,
        },
    ),
    (
        "SamusSpecialNLoop",
        Special {
            targets: &["Samus"],
            fighter: "Samus",
            motion: 198,
        },
    ),
    (
        "SamusSpecialNEnd",
        Special {
            targets: &["Samus"],
            fighter: "Samus",
            motion: 199,
        },
    ),
    (
        "SamusSpecialAirNStart",
        Special {
            targets: &["Samus"],
            fighter: "Samus",
            motion: 200,
        },
    ),
    (
        "SamusSpecialAirNEnd",
        Special {
            targets: &["Samus"],
            fighter: "Samus",
            motion: 201,
        },
    ),
    (
        "SamusSpecialHi",
        Special {
            targets: &["Samus"],
            fighter: "Samus",
            motion: 202,
        },
    ),
    (
        "SamusSpecialAirHi",
        Special {
            targets: &["Samus"],
            fighter: "Samus",
            motion: 203,
        },
    ),
    (
        "SamusSpecialLw",
        Special {
            targets: &["Samus"],
            fighter: "Samus",
            motion: 204,
        },
    ),
    (
        "SamusSpecialAirLw",
        Special {
            targets: &["Samus"],
            fighter: "Samus",
            motion: 205,
        },
    ),
    (
        "LuigiAttack11",
        Special {
            targets: &["Luigi"],
            fighter: "Luigi",
            motion: 165,
        },
    ),
    (
        "LuigiAttack12",
        Special {
            targets: &["Luigi"],
            fighter: "Luigi",
            motion: 166,
        },
    ),
    (
        "LuigiAttack13",
        Special {
            targets: &["Luigi"],
            fighter: "Luigi",
            motion: 195,
        },
    ),
    (
        "LuigiAttackDash",
        Special {
            targets: &["Luigi"],
            fighter: "Luigi",
            motion: 167,
        },
    ),
    (
        "LuigiAttackS3Hi",
        Special {
            targets: &["Luigi"],
            fighter: "Luigi",
            motion: 168,
        },
    ),
    (
        "LuigiAttackS3",
        Special {
            targets: &["Luigi"],
            fighter: "Luigi",
            motion: 170,
        },
    ),
    (
        "LuigiAttackS3Lw",
        Special {
            targets: &["Luigi"],
            fighter: "Luigi",
            motion: 172,
        },
    ),
    (
        "LuigiAttackHi3",
        Special {
            targets: &["Luigi"],
            fighter: "Luigi",
            motion: 174,
        },
    ),
    (
        "LuigiAttackLw3",
        Special {
            targets: &["Luigi"],
            fighter: "Luigi",
            motion: 176,
        },
    ),
    (
        "LuigiAttackS4Hi",
        Special {
            targets: &["Luigi"],
            fighter: "Luigi",
            motion: 177,
        },
    ),
    (
        "LuigiAttackS4HiS",
        Special {
            targets: &["Luigi"],
            fighter: "Luigi",
            motion: 178,
        },
    ),
    (
        "LuigiAttackS4",
        Special {
            targets: &["Luigi"],
            fighter: "Luigi",
            motion: 179,
        },
    ),
    (
        "LuigiAttackS4LwS",
        Special {
            targets: &["Luigi"],
            fighter: "Luigi",
            motion: 180,
        },
    ),
    (
        "LuigiAttackS4Lw",
        Special {
            targets: &["Luigi"],
            fighter: "Luigi",
            motion: 181,
        },
    ),
    (
        "LuigiAttackHi4",
        Special {
            targets: &["Luigi"],
            fighter: "Luigi",
            motion: 182,
        },
    ),
    (
        "LuigiAttackLw4",
        Special {
            targets: &["Luigi"],
            fighter: "Luigi",
            motion: 183,
        },
    ),
    (
        "LuigiAttackAirN",
        Special {
            targets: &["Luigi"],
            fighter: "Luigi",
            motion: 184,
        },
    ),
    (
        "LuigiAttackAirF",
        Special {
            targets: &["Luigi"],
            fighter: "Luigi",
            motion: 185,
        },
    ),
    (
        "LuigiAttackAirB",
        Special {
            targets: &["Luigi"],
            fighter: "Luigi",
            motion: 186,
        },
    ),
    (
        "LuigiAttackAirHi",
        Special {
            targets: &["Luigi"],
            fighter: "Luigi",
            motion: 187,
        },
    ),
    (
        "LuigiAttackAirLw",
        Special {
            targets: &["Luigi"],
            fighter: "Luigi",
            motion: 188,
        },
    ),
    (
        "LinkAttack11",
        Special {
            targets: &["Link"],
            fighter: "Link",
            motion: 165,
        },
    ),
    (
        "LinkAttack12",
        Special {
            targets: &["Link"],
            fighter: "Link",
            motion: 166,
        },
    ),
    (
        "LinkAttackDash",
        Special {
            targets: &["Link"],
            fighter: "Link",
            motion: 167,
        },
    ),
    (
        "LinkAttackS3",
        Special {
            targets: &["Link"],
            fighter: "Link",
            motion: 170,
        },
    ),
    (
        "LinkAttackHi3",
        Special {
            targets: &["Link"],
            fighter: "Link",
            motion: 174,
        },
    ),
    (
        "LinkAttackLw3",
        Special {
            targets: &["Link"],
            fighter: "Link",
            motion: 176,
        },
    ),
    (
        "LinkAttackS4",
        Special {
            targets: &["Link"],
            fighter: "Link",
            motion: 179,
        },
    ),
    (
        "LinkAttackHi4",
        Special {
            targets: &["Link"],
            fighter: "Link",
            motion: 182,
        },
    ),
    (
        "LinkAttackLw4",
        Special {
            targets: &["Link"],
            fighter: "Link",
            motion: 183,
        },
    ),
    (
        "LinkAttackAirN",
        Special {
            targets: &["Link"],
            fighter: "Link",
            motion: 184,
        },
    ),
    (
        "LinkAttackAirF",
        Special {
            targets: &["Link"],
            fighter: "Link",
            motion: 185,
        },
    ),
    (
        "LinkAttackAirB",
        Special {
            targets: &["Link"],
            fighter: "Link",
            motion: 186,
        },
    ),
    (
        "LinkAttackAirHi",
        Special {
            targets: &["Link"],
            fighter: "Link",
            motion: 187,
        },
    ),
    (
        "LinkAttackAirLw",
        Special {
            targets: &["Link"],
            fighter: "Link",
            motion: 188,
        },
    ),
    (
        "LinkAttack13",
        Special {
            targets: &["Link"],
            fighter: "Link",
            motion: 195,
        },
    ),
    (
        "LinkAttack100Start",
        Special {
            targets: &["Link"],
            fighter: "Link",
            motion: 196,
        },
    ),
    (
        "LinkAttack100Loop",
        Special {
            targets: &["Link"],
            fighter: "Link",
            motion: 197,
        },
    ),
    (
        "LinkAttack100End",
        Special {
            targets: &["Link"],
            fighter: "Link",
            motion: 198,
        },
    ),
    (
        "LinkSpecialHi",
        Special {
            targets: &["Link"],
            fighter: "Link",
            motion: 201,
        },
    ),
    (
        "LinkSpecialHiEnd",
        Special {
            targets: &["Link"],
            fighter: "Link",
            motion: 202,
        },
    ),
    (
        "LinkSpecialAirHi",
        Special {
            targets: &["Link"],
            fighter: "Link",
            motion: 203,
        },
    ),
    (
        "LinkSpecialN",
        Special {
            targets: &["Link"],
            fighter: "Link",
            motion: 204,
        },
    ),
    (
        "LinkSpecialNGet",
        Special {
            targets: &["Link"],
            fighter: "Link",
            motion: 205,
        },
    ),
    (
        "LinkSpecialNEmpty",
        Special {
            targets: &["Link"],
            fighter: "Link",
            motion: 204,
        },
    ),
    (
        "LinkSpecialAirN",
        Special {
            targets: &["Link"],
            fighter: "Link",
            motion: 207,
        },
    ),
    (
        "LinkSpecialAirNReturn",
        Special {
            targets: &["Link"],
            fighter: "Link",
            motion: 208,
        },
    ),
    (
        "LinkSpecialAirNEmpty",
        Special {
            targets: &["Link"],
            fighter: "Link",
            motion: 207,
        },
    ),
    (
        "LinkSpecialLw",
        Special {
            targets: &["Link"],
            fighter: "Link",
            motion: 210,
        },
    ),
    (
        "LinkSpecialAirLw",
        Special {
            targets: &["Link"],
            fighter: "Link",
            motion: 211,
        },
    ),
    (
        "YoshiAttack11",
        Special {
            targets: &["Yoshi"],
            fighter: "Yoshi",
            motion: 165,
        },
    ),
    (
        "YoshiAttack12",
        Special {
            targets: &["Yoshi"],
            fighter: "Yoshi",
            motion: 166,
        },
    ),
    (
        "YoshiAttackDash",
        Special {
            targets: &["Yoshi"],
            fighter: "Yoshi",
            motion: 167,
        },
    ),
    (
        "YoshiAttackS3Hi",
        Special {
            targets: &["Yoshi"],
            fighter: "Yoshi",
            motion: 168,
        },
    ),
    (
        "YoshiAttackS3",
        Special {
            targets: &["Yoshi"],
            fighter: "Yoshi",
            motion: 170,
        },
    ),
    (
        "YoshiAttackS3Lw",
        Special {
            targets: &["Yoshi"],
            fighter: "Yoshi",
            motion: 172,
        },
    ),
    (
        "YoshiAttackHi3",
        Special {
            targets: &["Yoshi"],
            fighter: "Yoshi",
            motion: 174,
        },
    ),
    (
        "YoshiAttackLw3",
        Special {
            targets: &["Yoshi"],
            fighter: "Yoshi",
            motion: 176,
        },
    ),
    (
        "YoshiAttackS4Hi",
        Special {
            targets: &["Yoshi"],
            fighter: "Yoshi",
            motion: 177,
        },
    ),
    (
        "YoshiAttackS4",
        Special {
            targets: &["Yoshi"],
            fighter: "Yoshi",
            motion: 179,
        },
    ),
    (
        "YoshiAttackS4Lw",
        Special {
            targets: &["Yoshi"],
            fighter: "Yoshi",
            motion: 181,
        },
    ),
    (
        "YoshiAttackHi4",
        Special {
            targets: &["Yoshi"],
            fighter: "Yoshi",
            motion: 182,
        },
    ),
    (
        "YoshiAttackLw4",
        Special {
            targets: &["Yoshi"],
            fighter: "Yoshi",
            motion: 183,
        },
    ),
    (
        "YoshiAttackAirN",
        Special {
            targets: &["Yoshi"],
            fighter: "Yoshi",
            motion: 184,
        },
    ),
    (
        "YoshiAttackAirF",
        Special {
            targets: &["Yoshi"],
            fighter: "Yoshi",
            motion: 185,
        },
    ),
    (
        "YoshiAttackAirB",
        Special {
            targets: &["Yoshi"],
            fighter: "Yoshi",
            motion: 186,
        },
    ),
    (
        "YoshiAttackAirHi",
        Special {
            targets: &["Yoshi"],
            fighter: "Yoshi",
            motion: 187,
        },
    ),
    (
        "YoshiAttackAirLw",
        Special {
            targets: &["Yoshi"],
            fighter: "Yoshi",
            motion: 188,
        },
    ),
    (
        "YoshiSpecialHi",
        Special {
            targets: &["Yoshi"],
            fighter: "Yoshi",
            motion: 197,
        },
    ),
    (
        "YoshiSpecialAirHi",
        Special {
            targets: &["Yoshi"],
            fighter: "Yoshi",
            motion: 198,
        },
    ),
    (
        "YoshiSpecialLwStart",
        Special {
            targets: &["Yoshi"],
            fighter: "Yoshi",
            motion: 199,
        },
    ),
    (
        "YoshiSpecialLwLanding",
        Special {
            targets: &["Yoshi"],
            fighter: "Yoshi",
            motion: 200,
        },
    ),
    (
        "YoshiSpecialAirLwStart",
        Special {
            targets: &["Yoshi"],
            fighter: "Yoshi",
            motion: 201,
        },
    ),
    (
        "YoshiSpecialN",
        Special {
            targets: &["Yoshi"],
            fighter: "Yoshi",
            motion: 202,
        },
    ),
    (
        "YoshiSpecialNCatch",
        Special {
            targets: &["Yoshi"],
            fighter: "Yoshi",
            motion: 202,
        },
    ),
    (
        "YoshiSpecialNRelease",
        Special {
            targets: &["Yoshi"],
            fighter: "Yoshi",
            motion: 204,
        },
    ),
    (
        "YoshiSpecialAirN",
        Special {
            targets: &["Yoshi"],
            fighter: "Yoshi",
            motion: 205,
        },
    ),
    (
        "YoshiSpecialAirNCatch",
        Special {
            targets: &["Yoshi"],
            fighter: "Yoshi",
            motion: 205,
        },
    ),
    (
        "YoshiSpecialAirNRelease",
        Special {
            targets: &["Yoshi"],
            fighter: "Yoshi",
            motion: 207,
        },
    ),
    (
        "CaptainAttack11",
        Special {
            targets: &["Captain"],
            fighter: "Captain",
            motion: 165,
        },
    ),
    (
        "CaptainAttack12",
        Special {
            targets: &["Captain"],
            fighter: "Captain",
            motion: 166,
        },
    ),
    (
        "CaptainAttackDash",
        Special {
            targets: &["Captain"],
            fighter: "Captain",
            motion: 167,
        },
    ),
    (
        "CaptainAttackS3Hi",
        Special {
            targets: &["Captain"],
            fighter: "Captain",
            motion: 168,
        },
    ),
    (
        "CaptainAttackS3HiS",
        Special {
            targets: &["Captain"],
            fighter: "Captain",
            motion: 169,
        },
    ),
    (
        "CaptainAttackS3",
        Special {
            targets: &["Captain"],
            fighter: "Captain",
            motion: 170,
        },
    ),
    (
        "CaptainAttackS3LwS",
        Special {
            targets: &["Captain"],
            fighter: "Captain",
            motion: 171,
        },
    ),
    (
        "CaptainAttackS3Lw",
        Special {
            targets: &["Captain"],
            fighter: "Captain",
            motion: 172,
        },
    ),
    (
        "CaptainAttackHi3",
        Special {
            targets: &["Captain"],
            fighter: "Captain",
            motion: 174,
        },
    ),
    (
        "CaptainAttackLw3",
        Special {
            targets: &["Captain"],
            fighter: "Captain",
            motion: 176,
        },
    ),
    (
        "CaptainAttackS4Hi",
        Special {
            targets: &["Captain"],
            fighter: "Captain",
            motion: 177,
        },
    ),
    (
        "CaptainAttackS4",
        Special {
            targets: &["Captain"],
            fighter: "Captain",
            motion: 179,
        },
    ),
    (
        "CaptainAttackS4Lw",
        Special {
            targets: &["Captain"],
            fighter: "Captain",
            motion: 181,
        },
    ),
    (
        "CaptainAttackHi4",
        Special {
            targets: &["Captain"],
            fighter: "Captain",
            motion: 182,
        },
    ),
    (
        "CaptainAttackLw4",
        Special {
            targets: &["Captain"],
            fighter: "Captain",
            motion: 183,
        },
    ),
    (
        "CaptainAttackAirN",
        Special {
            targets: &["Captain"],
            fighter: "Captain",
            motion: 184,
        },
    ),
    (
        "CaptainAttackAirF",
        Special {
            targets: &["Captain"],
            fighter: "Captain",
            motion: 185,
        },
    ),
    (
        "CaptainAttackAirB",
        Special {
            targets: &["Captain"],
            fighter: "Captain",
            motion: 186,
        },
    ),
    (
        "CaptainAttackAirHi",
        Special {
            targets: &["Captain"],
            fighter: "Captain",
            motion: 187,
        },
    ),
    (
        "CaptainAttackAirLw",
        Special {
            targets: &["Captain"],
            fighter: "Captain",
            motion: 188,
        },
    ),
    (
        "CaptainAttack13",
        Special {
            targets: &["Captain"],
            fighter: "Captain",
            motion: 195,
        },
    ),
    (
        "CaptainAttack100Start",
        Special {
            targets: &["Captain"],
            fighter: "Captain",
            motion: 196,
        },
    ),
    (
        "CaptainAttack100Loop",
        Special {
            targets: &["Captain"],
            fighter: "Captain",
            motion: 197,
        },
    ),
    (
        "CaptainAttack100End",
        Special {
            targets: &["Captain"],
            fighter: "Captain",
            motion: 198,
        },
    ),
    (
        "CaptainSpecialN",
        Special {
            targets: &["Captain"],
            fighter: "Captain",
            motion: 203,
        },
    ),
    (
        "CaptainSpecialAirN",
        Special {
            targets: &["Captain"],
            fighter: "Captain",
            motion: 204,
        },
    ),
    (
        "CaptainSpecialLw",
        Special {
            targets: &["Captain"],
            fighter: "Captain",
            motion: 205,
        },
    ),
    (
        "CaptainSpecialLwAir",
        Special {
            targets: &["Captain"],
            fighter: "Captain",
            motion: 206,
        },
    ),
    (
        "CaptainSpecialLwLanding",
        Special {
            targets: &["Captain"],
            fighter: "Captain",
            motion: 207,
        },
    ),
    (
        "CaptainSpecialAirLw",
        Special {
            targets: &["Captain"],
            fighter: "Captain",
            motion: 208,
        },
    ),
    (
        "CaptainSpecialLwBound",
        Special {
            targets: &["Captain"],
            fighter: "Captain",
            motion: 206,
        },
    ),
    (
        "CaptainSpecialHi",
        Special {
            targets: &["Captain"],
            fighter: "Captain",
            motion: 210,
        },
    ),
    (
        "CaptainSpecialHiCatch",
        Special {
            targets: &["Captain"],
            fighter: "Captain",
            motion: 211,
        },
    ),
    (
        "CaptainSpecialHiThrow",
        Special {
            targets: &["Captain"],
            fighter: "Captain",
            motion: 209,
        },
    ),
    (
        "CaptainSpecialAirHi",
        Special {
            targets: &["Captain"],
            fighter: "Captain",
            motion: 210,
        },
    ),
    (
        "KirbyAttack11",
        Special {
            targets: &["Kirby"],
            fighter: "Kirby",
            motion: 165,
        },
    ),
    (
        "KirbyAttack12",
        Special {
            targets: &["Kirby"],
            fighter: "Kirby",
            motion: 166,
        },
    ),
    (
        "KirbyAttackDash",
        Special {
            targets: &["Kirby"],
            fighter: "Kirby",
            motion: 167,
        },
    ),
    (
        "KirbyAttackS3Hi",
        Special {
            targets: &["Kirby"],
            fighter: "Kirby",
            motion: 168,
        },
    ),
    (
        "KirbyAttackS3",
        Special {
            targets: &["Kirby"],
            fighter: "Kirby",
            motion: 170,
        },
    ),
    (
        "KirbyAttackS3Lw",
        Special {
            targets: &["Kirby"],
            fighter: "Kirby",
            motion: 172,
        },
    ),
    (
        "KirbyAttackHi3",
        Special {
            targets: &["Kirby"],
            fighter: "Kirby",
            motion: 174,
        },
    ),
    (
        "KirbyAttackLw3",
        Special {
            targets: &["Kirby"],
            fighter: "Kirby",
            motion: 176,
        },
    ),
    (
        "KirbyAttackS4",
        Special {
            targets: &["Kirby"],
            fighter: "Kirby",
            motion: 179,
        },
    ),
    (
        "KirbyAttackHi4",
        Special {
            targets: &["Kirby"],
            fighter: "Kirby",
            motion: 182,
        },
    ),
    (
        "KirbyAttackLw4",
        Special {
            targets: &["Kirby"],
            fighter: "Kirby",
            motion: 183,
        },
    ),
    (
        "KirbyAttackAirN",
        Special {
            targets: &["Kirby"],
            fighter: "Kirby",
            motion: 184,
        },
    ),
    (
        "KirbyAttackAirF",
        Special {
            targets: &["Kirby"],
            fighter: "Kirby",
            motion: 185,
        },
    ),
    (
        "KirbyAttackAirB",
        Special {
            targets: &["Kirby"],
            fighter: "Kirby",
            motion: 186,
        },
    ),
    (
        "KirbyAttackAirHi",
        Special {
            targets: &["Kirby"],
            fighter: "Kirby",
            motion: 187,
        },
    ),
    (
        "KirbyAttackAirLw",
        Special {
            targets: &["Kirby"],
            fighter: "Kirby",
            motion: 188,
        },
    ),
    (
        "KirbyLandingAirF",
        Special {
            targets: &["Kirby"],
            fighter: "Kirby",
            motion: 190,
        },
    ),
    (
        "KirbyLandingAirB",
        Special {
            targets: &["Kirby"],
            fighter: "Kirby",
            motion: 191,
        },
    ),
    (
        "KirbyAttack100Start",
        Special {
            targets: &["Kirby"],
            fighter: "Kirby",
            motion: 195,
        },
    ),
    (
        "KirbyAttack100Loop",
        Special {
            targets: &["Kirby"],
            fighter: "Kirby",
            motion: 196,
        },
    ),
    (
        "KirbyAttack100End",
        Special {
            targets: &["Kirby"],
            fighter: "Kirby",
            motion: 197,
        },
    ),
    (
        "KirbyJumpAerialF1",
        Special {
            targets: &["Kirby"],
            fighter: "Kirby",
            motion: 198,
        },
    ),
    (
        "KirbyJumpAerialF2",
        Special {
            targets: &["Kirby"],
            fighter: "Kirby",
            motion: 199,
        },
    ),
    (
        "KirbyJumpAerialF3",
        Special {
            targets: &["Kirby"],
            fighter: "Kirby",
            motion: 200,
        },
    ),
    (
        "KirbyJumpAerialF4",
        Special {
            targets: &["Kirby"],
            fighter: "Kirby",
            motion: 201,
        },
    ),
    (
        "KirbyJumpAerialF5",
        Special {
            targets: &["Kirby"],
            fighter: "Kirby",
            motion: 202,
        },
    ),
    (
        "KirbyThrowF",
        Special {
            targets: &["Kirby"],
            fighter: "Kirby",
            motion: 203,
        },
    ),
    (
        "KirbyThrowFFall",
        Special {
            targets: &["Kirby"],
            fighter: "Kirby",
            motion: 204,
        },
    ),
    (
        "KirbyThrowFLanding",
        Special {
            targets: &["Kirby"],
            fighter: "Kirby",
            motion: 205,
        },
    ),
    (
        "KirbySpecialHi",
        Special {
            targets: &["Kirby"],
            fighter: "Kirby",
            motion: 231,
        },
    ),
    (
        "KirbySpecialHiLanding",
        Special {
            targets: &["Kirby"],
            fighter: "Kirby",
            motion: 232,
        },
    ),
    (
        "KirbySpecialAirHi",
        Special {
            targets: &["Kirby"],
            fighter: "Kirby",
            motion: 231,
        },
    ),
    (
        "KirbySpecialAirHiFall",
        Special {
            targets: &["Kirby"],
            fighter: "Kirby",
            motion: 234,
        },
    ),
    (
        "KirbySpecialLwStart",
        Special {
            targets: &["Kirby"],
            fighter: "Kirby",
            motion: 235,
        },
    ),
    (
        "KirbySpecialLwUnk",
        Special {
            targets: &["Kirby"],
            fighter: "Kirby",
            motion: 236,
        },
    ),
    (
        "KirbySpecialLwHold",
        Special {
            targets: &["Kirby"],
            fighter: "Kirby",
            motion: 236,
        },
    ),
    (
        "KirbySpecialLwEnd",
        Special {
            targets: &["Kirby"],
            fighter: "Kirby",
            motion: 238,
        },
    ),
    (
        "KirbySpecialAirLwStart",
        Special {
            targets: &["Kirby"],
            fighter: "Kirby",
            motion: 239,
        },
    ),
    (
        "KirbySpecialAirLwHold",
        Special {
            targets: &["Kirby"],
            fighter: "Kirby",
            motion: 236,
        },
    ),
    (
        "KirbySpecialAirLwLanding",
        Special {
            targets: &["Kirby"],
            fighter: "Kirby",
            motion: 236,
        },
    ),
    (
        "KirbySpecialAirLwEnd",
        Special {
            targets: &["Kirby"],
            fighter: "Kirby",
            motion: 238,
        },
    ),
    (
        "KirbySpecialNStart",
        Special {
            targets: &["Kirby"],
            fighter: "Kirby",
            motion: 244,
        },
    ),
    (
        "KirbySpecialNLoop",
        Special {
            targets: &["Kirby"],
            fighter: "Kirby",
            motion: 245,
        },
    ),
    (
        "KirbySpecialNEnd",
        Special {
            targets: &["Kirby"],
            fighter: "Kirby",
            motion: 246,
        },
    ),
    (
        "KirbySpecialNEat",
        Special {
            targets: &["Kirby"],
            fighter: "Kirby",
            motion: 247,
        },
    ),
    (
        "KirbySpecialNThrow",
        Special {
            targets: &["Kirby"],
            fighter: "Kirby",
            motion: 248,
        },
    ),
    (
        "KirbySpecialNWait",
        Special {
            targets: &["Kirby"],
            fighter: "Kirby",
            motion: 249,
        },
    ),
    (
        "KirbySpecialNTurn",
        Special {
            targets: &["Kirby"],
            fighter: "Kirby",
            motion: 250,
        },
    ),
    (
        "KirbySpecialNCopy",
        Special {
            targets: &["Kirby"],
            fighter: "Kirby",
            motion: 251,
        },
    ),
    (
        "KirbyCopyMarioSpecialN",
        Special {
            targets: &["Kirby"],
            fighter: "Kirby",
            motion: 206,
        },
    ),
    (
        "KirbyCopyMarioSpecialAirN",
        Special {
            targets: &["Kirby"],
            fighter: "Kirby",
            motion: 207,
        },
    ),
    (
        "KirbyCopyFoxSpecialN",
        Special {
            targets: &["Kirby"],
            fighter: "Kirby",
            motion: 210,
        },
    ),
    (
        "KirbyCopyFoxSpecialAirN",
        Special {
            targets: &["Kirby"],
            fighter: "Kirby",
            motion: 211,
        },
    ),
    (
        "KirbyCopySamusSpecialNStart",
        Special {
            targets: &["Kirby"],
            fighter: "Kirby",
            motion: 212,
        },
    ),
    (
        "KirbyCopySamusSpecialNLoop",
        Special {
            targets: &["Kirby"],
            fighter: "Kirby",
            motion: 213,
        },
    ),
    (
        "KirbyCopySamusSpecialNEnd",
        Special {
            targets: &["Kirby"],
            fighter: "Kirby",
            motion: 214,
        },
    ),
    (
        "KirbyCopySamusSpecialAirNStart",
        Special {
            targets: &["Kirby"],
            fighter: "Kirby",
            motion: 215,
        },
    ),
    (
        "KirbyCopySamusSpecialAirNEnd",
        Special {
            targets: &["Kirby"],
            fighter: "Kirby",
            motion: 216,
        },
    ),
    (
        "KirbyCopyDonkeySpecialNStart",
        Special {
            targets: &["Kirby"],
            fighter: "Kirby",
            motion: 217,
        },
    ),
    (
        "KirbyCopyDonkeySpecialAirNStart",
        Special {
            targets: &["Kirby"],
            fighter: "Kirby",
            motion: 218,
        },
    ),
    (
        "KirbyCopyDonkeySpecialNLoop",
        Special {
            targets: &["Kirby"],
            fighter: "Kirby",
            motion: 219,
        },
    ),
    (
        "KirbyCopyDonkeySpecialAirNLoop",
        Special {
            targets: &["Kirby"],
            fighter: "Kirby",
            motion: 220,
        },
    ),
    (
        "KirbyCopyDonkeySpecialNEnd",
        Special {
            targets: &["Kirby"],
            fighter: "Kirby",
            motion: 221,
        },
    ),
    (
        "KirbyCopyDonkeySpecialAirNEnd",
        Special {
            targets: &["Kirby"],
            fighter: "Kirby",
            motion: 222,
        },
    ),
    (
        "KirbyCopyLinkSpecialN",
        Special {
            targets: &["Kirby"],
            fighter: "Kirby",
            motion: 260,
        },
    ),
    (
        "KirbyCopyLinkSpecialNGet",
        Special {
            targets: &["Kirby"],
            fighter: "Kirby",
            motion: 261,
        },
    ),
    (
        "KirbyCopyLinkSpecialAirN",
        Special {
            targets: &["Kirby"],
            fighter: "Kirby",
            motion: 263,
        },
    ),
    (
        "KirbyCopyLinkSpecialAirNReturn",
        Special {
            targets: &["Kirby"],
            fighter: "Kirby",
            motion: 264,
        },
    ),
    (
        "KirbyCopyCaptainSpecialN",
        Special {
            targets: &["Kirby"],
            fighter: "Kirby",
            motion: 268,
        },
    ),
    (
        "KirbyCopyCaptainSpecialAirN",
        Special {
            targets: &["Kirby"],
            fighter: "Kirby",
            motion: 269,
        },
    ),
    (
        "KirbyCopyYoshiSpecialN",
        Special {
            targets: &["Kirby"],
            fighter: "Kirby",
            motion: 270,
        },
    ),
    (
        "KirbyCopyYoshiSpecialNRelease",
        Special {
            targets: &["Kirby"],
            fighter: "Kirby",
            motion: 272,
        },
    ),
    (
        "KirbyCopyYoshiSpecialAirN",
        Special {
            targets: &["Kirby"],
            fighter: "Kirby",
            motion: 273,
        },
    ),
    (
        "KirbyCopyYoshiSpecialAirNRelease",
        Special {
            targets: &["Kirby"],
            fighter: "Kirby",
            motion: 275,
        },
    ),
    (
        "PikachuAttack11",
        Special {
            targets: &["Pikachu"],
            fighter: "Pikachu",
            motion: 165,
        },
    ),
    (
        "PikachuAttackDash",
        Special {
            targets: &["Pikachu"],
            fighter: "Pikachu",
            motion: 167,
        },
    ),
    (
        "PikachuAttackS3Hi",
        Special {
            targets: &["Pikachu"],
            fighter: "Pikachu",
            motion: 168,
        },
    ),
    (
        "PikachuAttackS3",
        Special {
            targets: &["Pikachu"],
            fighter: "Pikachu",
            motion: 170,
        },
    ),
    (
        "PikachuAttackS3Lw",
        Special {
            targets: &["Pikachu"],
            fighter: "Pikachu",
            motion: 172,
        },
    ),
    (
        "PikachuAttackHi3",
        Special {
            targets: &["Pikachu"],
            fighter: "Pikachu",
            motion: 174,
        },
    ),
    (
        "PikachuAttackLw3",
        Special {
            targets: &["Pikachu"],
            fighter: "Pikachu",
            motion: 176,
        },
    ),
    (
        "PikachuAttackS4",
        Special {
            targets: &["Pikachu"],
            fighter: "Pikachu",
            motion: 179,
        },
    ),
    (
        "PikachuAttackHi4",
        Special {
            targets: &["Pikachu"],
            fighter: "Pikachu",
            motion: 182,
        },
    ),
    (
        "PikachuAttackLw4",
        Special {
            targets: &["Pikachu"],
            fighter: "Pikachu",
            motion: 183,
        },
    ),
    (
        "PikachuAttackAirN",
        Special {
            targets: &["Pikachu"],
            fighter: "Pikachu",
            motion: 184,
        },
    ),
    (
        "PikachuAttackAirF",
        Special {
            targets: &["Pikachu"],
            fighter: "Pikachu",
            motion: 185,
        },
    ),
    (
        "PikachuAttackAirB",
        Special {
            targets: &["Pikachu"],
            fighter: "Pikachu",
            motion: 186,
        },
    ),
    (
        "PikachuAttackAirHi",
        Special {
            targets: &["Pikachu"],
            fighter: "Pikachu",
            motion: 187,
        },
    ),
    (
        "PikachuAttackAirLw",
        Special {
            targets: &["Pikachu"],
            fighter: "Pikachu",
            motion: 188,
        },
    ),
    (
        "PikachuLandingAirF",
        Special {
            targets: &["Pikachu"],
            fighter: "Pikachu",
            motion: 190,
        },
    ),
    (
        "PikachuLandingAirLw",
        Special {
            targets: &["Pikachu"],
            fighter: "Pikachu",
            motion: 193,
        },
    ),
    (
        "PikachuSpecialN",
        Special {
            targets: &["Pikachu"],
            fighter: "Pikachu",
            motion: 197,
        },
    ),
    (
        "PikachuSpecialAirN",
        Special {
            targets: &["Pikachu"],
            fighter: "Pikachu",
            motion: 198,
        },
    ),
    (
        "PikachuSpecialLwStart",
        Special {
            targets: &["Pikachu"],
            fighter: "Pikachu",
            motion: 199,
        },
    ),
    (
        "PikachuSpecialLwLoop",
        Special {
            targets: &["Pikachu"],
            fighter: "Pikachu",
            motion: 200,
        },
    ),
    (
        "PikachuSpecialLwHit",
        Special {
            targets: &["Pikachu"],
            fighter: "Pikachu",
            motion: 200,
        },
    ),
    (
        "PikachuSpecialLwEnd",
        Special {
            targets: &["Pikachu"],
            fighter: "Pikachu",
            motion: 202,
        },
    ),
    (
        "PikachuSpecialAirLwStart",
        Special {
            targets: &["Pikachu"],
            fighter: "Pikachu",
            motion: 203,
        },
    ),
    (
        "PikachuSpecialAirLwLoop",
        Special {
            targets: &["Pikachu"],
            fighter: "Pikachu",
            motion: 204,
        },
    ),
    (
        "PikachuSpecialAirLwHit",
        Special {
            targets: &["Pikachu"],
            fighter: "Pikachu",
            motion: 204,
        },
    ),
    (
        "PikachuSpecialAirLwEnd",
        Special {
            targets: &["Pikachu"],
            fighter: "Pikachu",
            motion: 206,
        },
    ),
    (
        "PikachuSpecialHi",
        Special {
            targets: &["Pikachu"],
            fighter: "Pikachu",
            motion: 207,
        },
    ),
    (
        "PikachuSpecialHiEnd",
        Special {
            targets: &["Pikachu"],
            fighter: "Pikachu",
            motion: 207,
        },
    ),
    (
        "PikachuSpecialAirHi",
        Special {
            targets: &["Pikachu"],
            fighter: "Pikachu",
            motion: 209,
        },
    ),
    (
        "PikachuSpecialAirHiEnd",
        Special {
            targets: &["Pikachu"],
            fighter: "Pikachu",
            motion: 209,
        },
    ),
    (
        "KirbyCopyPikachuSpecialN",
        Special {
            targets: &["Kirby"],
            fighter: "Kirby",
            motion: 227,
        },
    ),
    (
        "KirbyCopyPikachuSpecialAirN",
        Special {
            targets: &["Kirby"],
            fighter: "Kirby",
            motion: 228,
        },
    ),
    (
        "PurinAttack11",
        Special {
            targets: &["Purin"],
            fighter: "Purin",
            motion: 165,
        },
    ),
    (
        "PurinAttack12",
        Special {
            targets: &["Purin"],
            fighter: "Purin",
            motion: 166,
        },
    ),
    (
        "PurinAttackDash",
        Special {
            targets: &["Purin"],
            fighter: "Purin",
            motion: 167,
        },
    ),
    (
        "PurinAttackS3Hi",
        Special {
            targets: &["Purin"],
            fighter: "Purin",
            motion: 168,
        },
    ),
    (
        "PurinAttackS3",
        Special {
            targets: &["Purin"],
            fighter: "Purin",
            motion: 170,
        },
    ),
    (
        "PurinAttackS3Lw",
        Special {
            targets: &["Purin"],
            fighter: "Purin",
            motion: 172,
        },
    ),
    (
        "PurinAttackHi3",
        Special {
            targets: &["Purin"],
            fighter: "Purin",
            motion: 174,
        },
    ),
    (
        "PurinAttackLw3",
        Special {
            targets: &["Purin"],
            fighter: "Purin",
            motion: 176,
        },
    ),
    (
        "PurinAttackS4",
        Special {
            targets: &["Purin"],
            fighter: "Purin",
            motion: 179,
        },
    ),
    (
        "PurinAttackHi4",
        Special {
            targets: &["Purin"],
            fighter: "Purin",
            motion: 182,
        },
    ),
    (
        "PurinAttackLw4",
        Special {
            targets: &["Purin"],
            fighter: "Purin",
            motion: 183,
        },
    ),
    (
        "PurinAttackAirN",
        Special {
            targets: &["Purin"],
            fighter: "Purin",
            motion: 184,
        },
    ),
    (
        "PurinAttackAirF",
        Special {
            targets: &["Purin"],
            fighter: "Purin",
            motion: 185,
        },
    ),
    (
        "PurinAttackAirB",
        Special {
            targets: &["Purin"],
            fighter: "Purin",
            motion: 186,
        },
    ),
    (
        "PurinAttackAirHi",
        Special {
            targets: &["Purin"],
            fighter: "Purin",
            motion: 187,
        },
    ),
    (
        "PurinAttackAirLw",
        Special {
            targets: &["Purin"],
            fighter: "Purin",
            motion: 188,
        },
    ),
    (
        "PurinLandingAirF",
        Special {
            targets: &["Purin"],
            fighter: "Purin",
            motion: 190,
        },
    ),
    (
        "PurinLandingAirB",
        Special {
            targets: &["Purin"],
            fighter: "Purin",
            motion: 191,
        },
    ),
    (
        "PurinJumpAerialF1",
        Special {
            targets: &["Purin"],
            fighter: "Purin",
            motion: 198,
        },
    ),
    (
        "PurinJumpAerialF2",
        Special {
            targets: &["Purin"],
            fighter: "Purin",
            motion: 199,
        },
    ),
    (
        "PurinJumpAerialF3",
        Special {
            targets: &["Purin"],
            fighter: "Purin",
            motion: 200,
        },
    ),
    (
        "PurinJumpAerialF4",
        Special {
            targets: &["Purin"],
            fighter: "Purin",
            motion: 201,
        },
    ),
    (
        "PurinJumpAerialF5",
        Special {
            targets: &["Purin"],
            fighter: "Purin",
            motion: 202,
        },
    ),
    (
        "PurinSpecialN",
        Special {
            targets: &["Purin"],
            fighter: "Purin",
            motion: 205,
        },
    ),
    (
        "PurinSpecialAirN",
        Special {
            targets: &["Purin"],
            fighter: "Purin",
            motion: 206,
        },
    ),
    (
        "PurinSpecialHi",
        Special {
            targets: &["Purin"],
            fighter: "Purin",
            motion: 207,
        },
    ),
    (
        "PurinSpecialLw",
        Special {
            targets: &["Purin"],
            fighter: "Purin",
            motion: 208,
        },
    ),
    (
        "KirbyCopyPurinSpecialN",
        Special {
            targets: &["Kirby"],
            fighter: "Kirby",
            motion: 266,
        },
    ),
    (
        "KirbyCopyPurinSpecialAirN",
        Special {
            targets: &["Kirby"],
            fighter: "Kirby",
            motion: 267,
        },
    ),
    ("FuraSleep", Common(165)),
    (
        "NessAttack11",
        Special {
            targets: &["Ness"],
            fighter: "Ness",
            motion: 165,
        },
    ),
    (
        "NessAttack12",
        Special {
            targets: &["Ness"],
            fighter: "Ness",
            motion: 166,
        },
    ),
    (
        "NessAttackDash",
        Special {
            targets: &["Ness"],
            fighter: "Ness",
            motion: 167,
        },
    ),
    (
        "NessAttackS3Hi",
        Special {
            targets: &["Ness"],
            fighter: "Ness",
            motion: 168,
        },
    ),
    (
        "NessAttackS3",
        Special {
            targets: &["Ness"],
            fighter: "Ness",
            motion: 170,
        },
    ),
    (
        "NessAttackS3Lw",
        Special {
            targets: &["Ness"],
            fighter: "Ness",
            motion: 172,
        },
    ),
    (
        "NessAttackHi3",
        Special {
            targets: &["Ness"],
            fighter: "Ness",
            motion: 174,
        },
    ),
    (
        "NessAttackLw3",
        Special {
            targets: &["Ness"],
            fighter: "Ness",
            motion: 176,
        },
    ),
    (
        "NessAttackS4",
        Special {
            targets: &["Ness"],
            fighter: "Ness",
            motion: 179,
        },
    ),
    (
        "NessAttackHi4",
        Special {
            targets: &["Ness"],
            fighter: "Ness",
            motion: 182,
        },
    ),
    (
        "NessAttackLw4",
        Special {
            targets: &["Ness"],
            fighter: "Ness",
            motion: 183,
        },
    ),
    (
        "NessAttackAirN",
        Special {
            targets: &["Ness"],
            fighter: "Ness",
            motion: 184,
        },
    ),
    (
        "NessAttackAirF",
        Special {
            targets: &["Ness"],
            fighter: "Ness",
            motion: 185,
        },
    ),
    (
        "NessAttackAirB",
        Special {
            targets: &["Ness"],
            fighter: "Ness",
            motion: 186,
        },
    ),
    (
        "NessAttackAirHi",
        Special {
            targets: &["Ness"],
            fighter: "Ness",
            motion: 187,
        },
    ),
    (
        "NessAttackAirLw",
        Special {
            targets: &["Ness"],
            fighter: "Ness",
            motion: 188,
        },
    ),
    (
        "NessLandingAirF",
        Special {
            targets: &["Ness"],
            fighter: "Ness",
            motion: 190,
        },
    ),
    (
        "NessLandingAirB",
        Special {
            targets: &["Ness"],
            fighter: "Ness",
            motion: 191,
        },
    ),
    (
        "NessLandingAirLw",
        Special {
            targets: &["Ness"],
            fighter: "Ness",
            motion: 14,
        },
    ),
    (
        "NessAttack13",
        Special {
            targets: &["Ness"],
            fighter: "Ness",
            motion: 195,
        },
    ),
    (
        "NessSpecialN",
        Special {
            targets: &["Ness"],
            fighter: "Ness",
            motion: 201,
        },
    ),
    (
        "NessSpecialAirN",
        Special {
            targets: &["Ness"],
            fighter: "Ness",
            motion: 202,
        },
    ),
    (
        "NessSpecialHiStart",
        Special {
            targets: &["Ness"],
            fighter: "Ness",
            motion: 203,
        },
    ),
    (
        "NessSpecialHiHold",
        Special {
            targets: &["Ness"],
            fighter: "Ness",
            motion: 204,
        },
    ),
    (
        "NessSpecialHiEnd",
        Special {
            targets: &["Ness"],
            fighter: "Ness",
            motion: 205,
        },
    ),
    (
        "NessSpecialHiJibaku",
        Special {
            targets: &["Ness"],
            fighter: "Ness",
            motion: 206,
        },
    ),
    (
        "NessSpecialAirHiStart",
        Special {
            targets: &["Ness"],
            fighter: "Ness",
            motion: 207,
        },
    ),
    (
        "NessSpecialAirHiHold",
        Special {
            targets: &["Ness"],
            fighter: "Ness",
            motion: 208,
        },
    ),
    (
        "NessSpecialAirHiEnd",
        Special {
            targets: &["Ness"],
            fighter: "Ness",
            motion: 209,
        },
    ),
    (
        "NessSpecialAirHiBound",
        Special {
            targets: &["Ness"],
            fighter: "Ness",
            motion: 49,
        },
    ),
    (
        "NessSpecialAirHiJibaku",
        Special {
            targets: &["Ness"],
            fighter: "Ness",
            motion: 206,
        },
    ),
    (
        "NessSpecialLwStart",
        Special {
            targets: &["Ness"],
            fighter: "Ness",
            motion: 212,
        },
    ),
    (
        "NessSpecialLwHold",
        Special {
            targets: &["Ness"],
            fighter: "Ness",
            motion: 213,
        },
    ),
    (
        "NessSpecialLwHit",
        Special {
            targets: &["Ness"],
            fighter: "Ness",
            motion: 213,
        },
    ),
    (
        "NessSpecialLwEnd",
        Special {
            targets: &["Ness"],
            fighter: "Ness",
            motion: 215,
        },
    ),
    (
        "NessSpecialAirLwStart",
        Special {
            targets: &["Ness"],
            fighter: "Ness",
            motion: 216,
        },
    ),
    (
        "NessSpecialAirLwHold",
        Special {
            targets: &["Ness"],
            fighter: "Ness",
            motion: 217,
        },
    ),
    (
        "NessSpecialAirLwHit",
        Special {
            targets: &["Ness"],
            fighter: "Ness",
            motion: 217,
        },
    ),
    (
        "NessSpecialAirLwEnd",
        Special {
            targets: &["Ness"],
            fighter: "Ness",
            motion: 219,
        },
    ),
    (
        "KirbyCopyNessSpecialN",
        Special {
            targets: &["Kirby"],
            fighter: "Kirby",
            motion: 229,
        },
    ),
    (
        "KirbyCopyNessSpecialAirN",
        Special {
            targets: &["Kirby"],
            fighter: "Kirby",
            motion: 230,
        },
    ),
    ("WallDamage", Common(56)),
    ("StopCeil", Common(66)),
    ("DownBounceD", Common(67)),
    ("DownBounceU", Common(68)),
    ("DownStandD", Common(71)),
    ("DownStandU", Common(72)),
    ("PassiveStandF", Common(73)),
    ("PassiveStandB", Common(74)),
    ("DownForwardD", Common(75)),
    ("DownForwardU", Common(76)),
    ("DownBackD", Common(77)),
    ("DownBackU", Common(78)),
    ("DownAttackD", Common(79)),
    ("DownAttackU", Common(80)),
    ("Passive", Common(81)),
    ("Rebound", Common(83)),
    ("EscapeF", Common(156)),
    ("EscapeB", Common(157)),
    ("ShieldBreakFly", Common(158)),
    ("ShieldBreakFall", Common(159)),
    ("ShieldBreakDownD", Common(160)),
    ("ShieldBreakDownU", Common(161)),
    ("ShieldBreakStandD", Common(162)),
    ("ShieldBreakStandU", Common(163)),
    ("FuraFura", Common(164)),
    ("CliffCatch", Common(84)),
    ("CliffWait", Common(85)),
    ("CliffQuick", Common(86)),
    ("CliffClimbQuick1", Common(87)),
    ("CliffClimbQuick2", Common(88)),
    ("CliffSlow", Common(89)),
    ("CliffClimbSlow1", Common(90)),
    ("CliffClimbSlow2", Common(91)),
    ("CliffAttackQuick1", Common(92)),
    ("CliffAttackQuick2", Common(93)),
    ("CliffAttackSlow1", Common(94)),
    ("CliffAttackSlow2", Common(95)),
    ("CliffEscapeQuick1", Common(96)),
    ("CliffEscapeQuick2", Common(97)),
    ("CliffEscapeSlow1", Common(98)),
    ("CliffEscapeSlow2", Common(99)),
    ("DamageHi1", Common(37)),
    ("DamageHi2", Common(38)),
    ("DamageHi3", Common(39)),
    ("DamageN1", Common(40)),
    ("DamageN2", Common(41)),
    ("DamageN3", Common(42)),
    ("DamageLw1", Common(43)),
    ("DamageLw2", Common(44)),
    ("DamageLw3", Common(45)),
    ("DamageAir1", Common(46)),
    ("DamageAir2", Common(47)),
    ("DamageAir3", Common(48)),
    ("DamageE1", Common(49)),
    ("DamageE2", Common(50)),
    ("DamageFlyHi", Common(51)),
    ("DamageFlyN", Common(52)),
    ("DamageFlyLw", Common(53)),
    ("DamageFlyTop", Common(54)),
    ("DamageFlyRoll", Common(55)),
    ("DamageFall", Common(57)),
    ("FallSpecial", Common(58)),
    ("LandingFallSpecial", Common(59)),
    ("Appeal", Common(189)),
    ("Attack11", Common(190)),
    ("Attack12", Common(191)),
    ("AttackDash", Common(192)),
    ("AttackS3Hi", Common(193)),
    ("AttackS3HiS", Common(194)),
    ("AttackS3", Common(195)),
    ("AttackS3LwS", Common(196)),
    ("AttackS3Lw", Common(197)),
    ("AttackHi3F", Common(198)),
    ("AttackHi3", Common(199)),
    ("AttackHi3B", Common(200)),
    ("AttackLw3", Common(201)),
    ("AttackS4Hi", Common(202)),
    ("AttackS4HiS", Common(203)),
    ("AttackS4", Common(204)),
    ("AttackS4LwS", Common(205)),
    ("AttackS4Lw", Common(206)),
    ("AttackHi4", Common(207)),
    ("AttackLw4", Common(208)),
    ("AttackAirN", Common(209)),
    ("AttackAirF", Common(210)),
    ("AttackAirB", Common(211)),
    ("AttackAirHi", Common(212)),
    ("AttackAirLw", Common(213)),
    ("LandingAirN", Common(214)),
    ("LandingAirF", Common(215)),
    ("LandingAirB", Common(216)),
    ("LandingAirHi", Common(217)),
    ("LandingAirLw", Common(218)),
    ("LandingAirNull", Common(219)),
    (
        "MarioAttack13",
        Special {
            targets: &["Mario", "Luigi"],
            fighter: "Mario",
            motion: 195,
        },
    ),
    ("RebirthDown", Common(7)),
    ("RebirthStand", Common(8)),
    ("RebirthWait", Common(9)),
    ("WalkEnd", Common(14)),
    ("TurnRun", Common(19)),
    ("GuardKneeBend", Common(21)),
    ("GuardPass", Common(34)),
    ("OttottoWait", Common(35)),
    ("Ottotto", Common(36)),
    ("Twister", Common(60)),
    ("DokanStart", Common(62)),
    ("DokanEnd", Common(64)),
    ("DokanWalk", Common(65)),
    ("LightGet", Common(100)),
    ("HeavyGet", Common(101)),
    ("LightThrowDrop", Common(104)),
    ("LightThrowDash", Common(105)),
    ("LightThrowF", Common(106)),
    ("LightThrowB", Common(107)),
    ("LightThrowHi", Common(108)),
    ("LightThrowLw", Common(109)),
    ("LightThrowF4", Common(110)),
    ("LightThrowB4", Common(111)),
    ("LightThrowHi4", Common(112)),
    ("LightThrowLw4", Common(113)),
    ("LightThrowAirF", Common(114)),
    ("LightThrowAirB", Common(115)),
    ("LightThrowAirHi", Common(116)),
    ("LightThrowAirLw", Common(117)),
    ("LightThrowAirF4", Common(118)),
    ("LightThrowAirB4", Common(119)),
    ("LightThrowAirHi4", Common(120)),
    ("LightThrowAirLw4", Common(121)),
    ("HeavyThrowF", Common(122)),
    ("HeavyThrowB", Common(123)),
    ("HeavyThrowF4", Common(124)),
    ("HeavyThrowB4", Common(125)),
    ("SwordSwing1", Common(126)),
    ("SwordSwing3", Common(127)),
    ("SwordSwing4", Common(128)),
    ("SwordSwingDash", Common(129)),
    ("BatSwing1", Common(130)),
    ("BatSwing3", Common(131)),
    ("BatSwing4", Common(132)),
    ("BatSwingDash", Common(133)),
    ("HarisenSwing1", Common(134)),
    ("HarisenSwing3", Common(135)),
    ("HarisenSwing4", Common(136)),
    ("HarisenSwingDash", Common(137)),
    ("StarRodSwing1", Common(138)),
    ("StarRodSwing3", Common(139)),
    ("StarRodSwing4", Common(140)),
    ("StarRodSwingDash", Common(141)),
    ("LGunShoot", Common(142)),
    ("LGunShootAir", Common(143)),
    ("FireFlowerShoot", Common(144)),
    ("FireFlowerShootAir", Common(145)),
    ("HammerWait", Common(146)),
    ("HammerWalk", Common(147)),
    ("GuardOn", Common(152)),
    ("GuardOff", Common(154)),
    ("ThrownKirbyStar", Common(175)),
    ("ThrownCopyStar", Common(176)),
    ("YoshiEgg", Common(178)),
    ("CaptureCaptain", Common(179)),
    ("ThrownDonkeyUnk", Common(180)),
    ("AppearR", Appear(0)),
    ("AppearL", Appear(1)),
    ("AppearRStart", Appear(2)),
    ("AppearLStart", Appear(3)),
    ("AppearREnd", Appear(4)),
    ("AppearLEnd", Appear(5)),
    ("AppearWait", Appear(6)),
    ("Win1", Demo(1)),
    ("Win2", Demo(2)),
    ("Win3", Demo(3)),
    ("Win4", Demo(4)),
    ("Lose", Demo(5)),
    ("FigureDropped", Demo(9)),
    ("FigureStand", Demo(10)),
    ("IntroL", Demo(13)),
    ("IntroR", Demo(14)),
    ("BossDefault", Boss(0)),
    ("BossHippataku", Boss(1)),
    ("BossHarau", Boss(2)),
    ("BossOkuhikouki1", Boss(3)),
    ("BossOkuhikouki2", Boss(4)),
    ("BossOkuhikouki3", Boss(5)),
    ("BossWalk", Boss(6)),
    ("BossWalkLoop", Boss(7)),
    ("BossWalkWait", Boss(8)),
    ("BossWalkShoot", Boss(9)),
    ("BossGootsubusuUp", Boss(10)),
    ("BossGootsubusuWait", Boss(11)),
    ("BossGootsubusuEnd", Boss(12)),
    ("BossGootsubusuDown", Boss(13)),
    ("BossTsutsuku1", Boss(14)),
    ("BossTsutsuku3", Boss(15)),
    ("BossTsutsuku2", Boss(16)),
    ("BossDrill", Boss(17)),
    ("BossOkukouki", Boss(18)),
    ("BossYubideppou1", Boss(19)),
    ("BossYubideppou3", Boss(20)),
    ("BossYubideppou2", Boss(21)),
    ("BossOkupunch1", Boss(22)),
    ("BossOkupunch2", Boss(23)),
    ("BossOkupunch3", Boss(24)),
    ("BossOkutsubushi", Boss(25)),
    ("BossDeadLeft", Boss(26)),
    ("BossDeadCenter", Boss(27)),
    ("BossDeadRight", Boss(28)),
    ("BossAppear", Boss(29)),
    ("DemoRun", OpeningDemo(6)),
    ("DemoJump", OpeningDemo(7)),
    ("FigurePulled", OpeningDemo(8)),
    ("Clash", OpeningDemo(11)),
    ("Stance", OpeningDemo(12)),
    ("Opening1", OpeningRow(15)),
    ("Opening2", OpeningRow(16)),
    ("Opening3", OpeningRow(17)),
    ("Opening4", OpeningRow(18)),
    ("Opening5", OpeningRow(19)),
    ("Opening6", OpeningRow(20)),
    ("Opening7", OpeningRow(21)),
    ("Opening8", OpeningRow(22)),
    ("Opening9", OpeningRow(23)),
];

/// The variant fighters and the fighter whose motion enum their tables use.
fn owner(fighter: &str) -> &str {
    fighters::fighter(fighter).base.unwrap_or(fighter)
}

/// One resolved slot.
#[derive(Clone, Copy, Default)]
struct Entry {
    file: u32,
    runtime: u32,
    /// `None`: no length (none read, a loop, or a file that does not walk).
    frames: Option<u32>,
}

struct Tables<'a> {
    rom: &'a Source,
    lengths: Lengths,
    common: Vec<i16>,
    demo: Vec<u32>,
}

impl Tables<'_> {
    fn descs(&self, fighter: &str) -> Result<Vec<MotionDesc>> {
        motion_descs(self.rom, fighters::fighter(fighter).motion_descs)
    }

    fn subs(&self, fighter: &str) -> Result<Vec<MotionDesc>> {
        motion_descs(self.rom, fighters::fighter(fighter).sub_motion_descs)
    }

    /// The length the slot's figatree runs; an error unless `tolerant`.
    fn frames(&self, file: u32, tolerant: bool, what: &str) -> Option<u32> {
        match self.lengths.frames(self.rom, file) {
            Some(n) => n,
            None if tolerant => None,
            None => panic!("{what}: animation file {file} does not walk as a figatree"),
        }
    }

    fn row(&self, d: Option<&MotionDesc>, frames: bool, tolerant: bool, what: &str) -> Entry {
        match d {
            Some(d) if d.anim_file != 0 => Entry {
                file: d.anim_file,
                runtime: d.anim_desc & HIDDEN_PART_BITS,
                frames: if frames {
                    self.frames(d.anim_file, tolerant, what)
                } else {
                    None
                },
            },
            _ => Entry::default(),
        }
    }

    fn motion(&self, status: u16) -> usize {
        let m = self.common[status as usize];
        assert!(m >= 0, "common status {status} has no motion");
        m as usize
    }

    fn resolve(&self, fighter: &str, slot: &str, recipe: Recipe) -> Result<Entry> {
        let what = format!("{fighter} {slot}");
        let table = self.descs(fighter)?;
        Ok(match recipe {
            Status(status) => self.row(table.get(self.motion(status)), true, false, &what),
            Common(status) => {
                if fighter == "Boss" {
                    Entry::default()
                } else {
                    self.row(table.get(self.motion(status)), true, false, &what)
                }
            }
            Special {
                targets,
                fighter: named_by,
                motion,
            } => {
                let file = self.descs(named_by)?[motion as usize].anim_file;
                let own = owner(fighter);
                if targets.contains(&fighter) {
                    let runtimes: BTreeSet<u32> = table
                        .iter()
                        .filter(|d| d.anim_file == file)
                        .map(|d| d.anim_desc & HIDDEN_PART_BITS)
                        .collect();
                    assert_eq!(
                        runtimes.len(),
                        1,
                        "{what}: inconsistent runtime-joint flags"
                    );
                    Entry {
                        file,
                        runtime: *runtimes.first().expect("one"),
                        frames: self.frames(file, false, &what),
                    }
                } else if own != fighter && targets.contains(&own) {
                    // The variant's own row at every motion id where its
                    // base names the file. Past a gap in a shorter table
                    // (Polygon Kirby, Polygon Pikachu) the base enum reaches
                    // other motions; only rows still aligned resolve.
                    let base = self.descs(own)?;
                    let mut ids: Vec<usize> = (0..base.len())
                        .filter(|&i| base[i].anim_file == file)
                        .collect();
                    if table.len() != base.len() {
                        ids.retain(|&i| table.get(i).is_some_and(|d| d.anim_file == file));
                    }
                    let rows: BTreeSet<(u32, u32)> = ids
                        .iter()
                        .filter_map(|&i| table.get(i))
                        .map(|d| (d.anim_file, d.anim_desc & HIDDEN_PART_BITS))
                        .collect();
                    match rows.len() {
                        0 => Entry::default(),
                        1 => {
                            let (file, runtime) = *rows.first().expect("one");
                            if file == 0 {
                                Entry::default()
                            } else {
                                Entry {
                                    file,
                                    runtime,
                                    frames: self.frames(file, false, &what),
                                }
                            }
                        }
                        _ => panic!("{what}: the file resolves to {} rows", rows.len()),
                    }
                } else {
                    Entry::default()
                }
            }
            Appear(i) => {
                let motions = APPEAR_MOTIONS
                    .iter()
                    .find(|(o, _)| *o == owner(fighter))
                    .map(|(_, m)| m[i]);
                let d = motions.flatten().and_then(|m| table.get(m as usize));
                self.row(d, false, false, &what)
            }
            Demo(status) => {
                let subs = self.subs(fighter)?;
                let d = subs.get(self.demo[status as usize] as usize);
                assert!(
                    d.is_none_or(|d| d.anim_desc & ANIM_FLAG_ANIMJOINT == 0),
                    "{what}: an AnimJoint clip"
                );
                self.row(d, true, false, &what)
            }
            Boss(i) => {
                if fighter != "Boss" {
                    Entry::default()
                } else {
                    let d = &table[BOSS_MOTIONS[i] as usize];
                    let joint = d.anim_desc & ANIM_FLAG_ANIMJOINT != 0;
                    self.row(Some(d), !joint, true, &what)
                }
            }
            OpeningDemo(status) => {
                let subs = self.subs(fighter)?;
                let d = subs.get(self.demo[status as usize] as usize);
                assert!(
                    d.is_none_or(|d| d.anim_desc & ANIM_FLAG_ANIMJOINT == 0),
                    "{what}: an AnimJoint clip"
                );
                self.row(d, true, true, &what)
            }
            OpeningRow(row) => {
                let subs = self.subs(fighter)?;
                let d = subs.get(row);
                assert!(
                    d.is_none_or(|d| d.anim_desc & ANIM_FLAG_ANIMJOINT == 0),
                    "{what}: an AnimJoint clip"
                );
                self.row(d, true, true, &what)
            }
        })
    }
}

pub fn generate(rom: Option<&Source>) -> Result<String> {
    let mut rows: Vec<(&str, Vec<Entry>)> = Vec::new();
    let mut boss_joint = [false; BOSS_MOTIONS.len()];
    if let Some(rom) = rom {
        let (at, count) = DEMO_STATUS_TABLE;
        let demo = (0..count)
            .map(|i| Ok(rom.u32(at + 8 * i)? - 0x10000))
            .collect::<Result<_>>()?;
        let t = Tables {
            rom,
            lengths: Lengths::default(),
            common: fighters::common_status_motions(rom)?,
            demo,
        };
        for fighter in FIGHTERS {
            let mut entry = Vec::with_capacity(SLOTS.len());
            for &(slot, recipe) in &SLOTS {
                let e = t.resolve(fighter, slot, recipe)?;
                if fighter != "Boss" && TIMED_SLOTS.contains(&slot) && e.file != 0 {
                    assert!(
                        e.frames.is_some(),
                        "{fighter} {slot}: loops; it has no length"
                    );
                }
                entry.push(e);
            }
            rows.push((fighter, entry));
        }
        let boss = t.descs("Boss")?;
        for (flag, m) in boss_joint.iter_mut().zip(BOSS_MOTIONS) {
            *flag = boss[m as usize].anim_desc & ANIM_FLAG_ANIMJOINT != 0;
        }
    } else {
        for fighter in FIGHTERS {
            rows.push((fighter, vec![Entry::default(); SLOTS.len()]));
        }
    }

    let mut w = emit::header("the fighter animation table");
    let names: Vec<String> = SLOTS.iter().map(|(s, _)| format!("\"{s}\"")).collect();
    w.push_str("/// The statuses carried, in slot order.\n");
    let _ = write!(
        w,
        "pub const SLOT_NAMES: [&str; SLOT_COUNT] = [{}];\n\n",
        names.join(", ")
    );
    let table_head = |w: &mut String, ty: &str| {
        let _ = writeln!(
            w,
            "#[rustfmt::skip]\n#[allow(clippy::large_const_arrays)]\npub const {ty}{}] = [",
            rows.len()
        );
    };
    table_head(&mut w, "FIGHTER_ANIMS: [FighterAnims; ");
    for (fighter, entry) in &rows {
        let ids: Vec<String> = entry.iter().map(|e| format!("{:4}", e.file)).collect();
        let pad = " ".repeat(9 - fighter.len());
        let _ = writeln!(
            w,
            "    FighterAnims {{ name: \"{fighter}\",{pad}files: [{}] }},",
            ids.join(", ")
        );
    }
    w.push_str("];\n\n");
    w.push_str(
        "/// Whether a motion's extra figatree entry is a leading runtime joint.\n\
         /// Read from `FTMotionDesc.anim_desc`.\n",
    );
    table_head(&mut w, "LEADING_RUNTIME_JOINT: [[bool; SLOT_COUNT]; ");
    for (fighter, entry) in &rows {
        let flags: Vec<&str> = entry
            .iter()
            .map(|e| {
                if e.runtime & LEADING_BITS != 0 {
                    "true"
                } else {
                    "false"
                }
            })
            .collect();
        let _ = writeln!(w, "    [{}],  // {fighter}", flags.join(", "));
    }
    w.push_str("];\n\n");
    w.push_str(
        "/// Each motion's `FTAnimDesc` hidden-part bits (31..5): bit `31 - i`\n\
         /// inserts `FTAttributes.hiddenparts[i]` before the figatree binds.\n",
    );
    table_head(&mut w, "HIDDEN_PARTS: [[u32; SLOT_COUNT]; ");
    for (fighter, entry) in &rows {
        let bits: Vec<String> = entry.iter().map(|e| format!("{:#x}", e.runtime)).collect();
        let _ = writeln!(w, "    [{}],  // {fighter}", bits.join(", "));
    }
    w.push_str("];\n\n");
    let flags: Vec<&str> = boss_joint
        .iter()
        .map(|b| if *b { "true" } else { "false" })
        .collect();
    w.push_str(
        "/// Which of Master Hand's slots (`SLOT_BOSS_DEFAULT` on) hold a 32-bit\n\
         /// `AnimJoint` clip (`FTANIM_FLAG_ANIMJOINT` in `dFTBossMotionDescs`).\n",
    );
    let _ = write!(
        w,
        "pub const BOSS_ANIM_JOINT: [bool; {}] = [{}];\n\n",
        boss_joint.len(),
        flags.join(", ")
    );
    w.push_str(
        "/// Each slot's figatree length, walked by the generator (0 for none,\n\
         /// a loop, or an `AnimJoint` clip). `romtool anims --verify` checks\n\
         /// `ssb_rom::anim`'s own walk against these.\n",
    );
    table_head(&mut w, "EXPECTED_FRAMES: [[u16; SLOT_COUNT]; ");
    for (fighter, entry) in &rows {
        let lens: Vec<String> = entry
            .iter()
            .map(|e| format!("{:3}", e.frames.unwrap_or(0)))
            .collect();
        let _ = writeln!(w, "    [{}],  // {fighter}", lens.join(", "));
    }
    w.push_str("];\n");
    Ok(w)
}
