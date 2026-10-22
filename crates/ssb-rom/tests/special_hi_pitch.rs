//! Joint 4's Fire Fox and Quick Attack pitch (and Pikachu's stretch)
//! against the original, tick by tick (RE-503).
//!
//! The rows are the N64 game's own DObj values: Mupen64Plus, a Training
//! warp ROM, P1 pad `4:1:a;41:200:b+x=-45+y=70` through
//! `tools/n64-headless/pitch_trace.py` (the jab A and the
//! `foxfire`/`pikachuquick` B with the diagonal stick held). N64 frame
//! `t` is golden tick `t + 4`: both jabs start on the A's frame and last
//! as long (Fox 17, Pikachu 33 frames), and every later status edge lands
//! on the same tick. The N64 stage is Peach's Castle, the replay's Dream
//! Land; both moves leave and land nowhere in these ticks, so the stage
//! changes positions and the spawn facing only: Training faces a fighter
//! spawned at `x >= 0` left (`sc1ptrainingmode.c:1806`), P1 on the Castle
//! right, on Dream Land left. `lr` is compared from the launch on, where
//! the stick sets it.
//!
//! Per row: status, `lr`, `vel_air` x/y, joint 4 `rotate` and `scale`, all
//! as f32 bits. The comparison is exact: the port computes the pitch
//! with the original's `syUtilsArcTan2` from the same velocity.

use ssb_capture::script;
use ssb_capture::GameScene;
use ssb_engine::input::{ControllerState, N64Buttons};
use ssb_game::fighter::FighterKind;

#[path = "common/posed.rs"]
mod posed;
use posed::Posed;

#[path = "common/dream_land.rs"]
mod dream_land;
use dream_land::{dream_land_map, stage_floors, training_fighter};

/// Training's first battle frame (`scene_inputs::FIRST_TICK`).
const FIRST_TICK: u64 = 8;

/// One measured N64 frame.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Row {
    tick: u64,
    status: &'static str,
    lr: i32,
    vel: [u32; 2],
    rotate: [u32; 3],
    scale: [u32; 3],
    /// `fp->ga` (0 ground, 1 air), or [`NO_GA`] where not measured.
    ga: u32,
}

/// [`Row::ga`] for the tables traced before `ga` was compared.
const NO_GA: u32 = u32::MAX;

const fn row(
    tick: u64,
    status: &'static str,
    lr: i32,
    vel: [u32; 2],
    rotate: [u32; 3],
    scale: [u32; 3],
) -> Row {
    Row {
        tick,
        status,
        lr,
        vel,
        rotate,
        scale,
        ga: NO_GA,
    }
}

/// `warp_fox`, N64 frames 40-120.
#[rustfmt::skip]
const FOX: &[Row] = &[
    row(44, "Wait", 1, [0x00000000, 0x80000000], [0x00000000, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]),
    row(45, "SpecialHiStart", 1, [0x00000000, 0x80000000], [0x00000000, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]),
    row(46, "SpecialHiStart", 1, [0x00000000, 0x80000000], [0x00000000, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]),
    row(47, "SpecialHiStart", 1, [0x00000000, 0x80000000], [0x00000000, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]),
    row(48, "SpecialHiStart", 1, [0x00000000, 0x80000000], [0x00000000, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]),
    row(49, "SpecialHiStart", 1, [0x00000000, 0x80000000], [0x00000000, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]),
    row(50, "SpecialHiStart", 1, [0x00000000, 0x80000000], [0x00000000, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]),
    row(51, "SpecialHiStart", 1, [0x00000000, 0x80000000], [0x00000000, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]),
    row(52, "SpecialHiHold", 1, [0x00000000, 0x80000000], [0x00000000, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]),
    row(53, "SpecialHiHold", 1, [0x00000000, 0x80000000], [0x00000000, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]),
    row(54, "SpecialHiHold", 1, [0x00000000, 0x80000000], [0x00000000, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]),
    row(55, "SpecialHiHold", 1, [0x00000000, 0x80000000], [0x00000000, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]),
    row(56, "SpecialHiHold", 1, [0x00000000, 0x80000000], [0x00000000, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]),
    row(57, "SpecialHiHold", 1, [0x00000000, 0x80000000], [0x00000000, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]),
    row(58, "SpecialHiHold", 1, [0x00000000, 0x80000000], [0x00000000, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]),
    row(59, "SpecialHiHold", 1, [0x00000000, 0x80000000], [0x00000000, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]),
    row(60, "SpecialHiHold", 1, [0x00000000, 0x80000000], [0x00000000, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]),
    row(61, "SpecialHiHold", 1, [0x00000000, 0x80000000], [0x00000000, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]),
    row(62, "SpecialHiHold", 1, [0x00000000, 0x80000000], [0x00000000, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]),
    row(63, "SpecialHiHold", 1, [0x00000000, 0x80000000], [0x00000000, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]),
    row(64, "SpecialHiHold", 1, [0x00000000, 0x80000000], [0x00000000, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]),
    row(65, "SpecialHiHold", 1, [0x00000000, 0x80000000], [0x00000000, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]),
    row(66, "SpecialHiHold", 1, [0x00000000, 0x80000000], [0x00000000, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]),
    row(67, "SpecialHiHold", 1, [0x00000000, 0x80000000], [0x00000000, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]),
    row(68, "SpecialHiHold", 1, [0x00000000, 0x80000000], [0x00000000, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]),
    row(69, "SpecialHiHold", 1, [0x00000000, 0x80000000], [0x00000000, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]),
    row(70, "SpecialHiHold", 1, [0x00000000, 0x80000000], [0x00000000, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]),
    row(71, "SpecialHiHold", 1, [0x00000000, 0x80000000], [0x00000000, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]),
    row(72, "SpecialHiHold", 1, [0x00000000, 0x80000000], [0x00000000, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]),
    row(73, "SpecialHiHold", 1, [0x00000000, 0x80000000], [0x00000000, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]),
    row(74, "SpecialHiHold", 1, [0x00000000, 0x80000000], [0x00000000, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]),
    row(75, "SpecialHiHold", 1, [0x00000000, 0x80000000], [0x00000000, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]),
    row(76, "SpecialHiHold", 1, [0x00000000, 0x80000000], [0x00000000, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]),
    row(77, "SpecialHiHold", 1, [0x00000000, 0x80000000], [0x00000000, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]),
    row(78, "SpecialHiHold", 1, [0x00000000, 0x80000000], [0x00000000, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]),
    row(79, "SpecialHiHold", 1, [0x00000000, 0x80000000], [0x00000000, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]),
    row(80, "SpecialHiHold", 1, [0x00000000, 0x80000000], [0x00000000, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]),
    row(81, "SpecialHiHold", 1, [0x00000000, 0x80000000], [0x00000000, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]),
    row(82, "SpecialHiHold", 1, [0x00000000, 0x80000000], [0x00000000, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]),
    row(83, "SpecialHiHold", 1, [0x00000000, 0x80000000], [0x00000000, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]),
    row(84, "SpecialHiHold", 1, [0x00000000, 0x80000000], [0x00000000, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]),
    row(85, "SpecialHiHold", 1, [0x00000000, 0x80000000], [0x00000000, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]),
    row(86, "SpecialHiHold", 1, [0x00000000, 0x80000000], [0x00000000, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]),
    row(87, "SpecialAirHi", -1, [0xc278bf9c, 0x42c17897], [0xbf7fdc8a, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]),
    row(88, "SpecialAirHi", -1, [0xc2722ea0, 0x42bc5d28], [0xbf7fdc8a, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]),
    row(89, "SpecialAirHi", -1, [0xc26b9da4, 0x42b741b9], [0xbf7fdc8a, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]),
    row(90, "SpecialAirHi", -1, [0xc2650ca8, 0x42b2264a], [0xbf7fdc8a, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]),
    row(91, "SpecialAirHi", -1, [0xc25e7bac, 0x42ad0adb], [0xbf7fdc8a, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]),
    row(92, "SpecialAirHi", -1, [0xc257eab0, 0x42a7ef6c], [0xbf7fdc89, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]),
    row(93, "SpecialAirHi", -1, [0xc25159b4, 0x42a2d3fd], [0xbf7fdc89, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]),
    row(94, "SpecialAirHi", -1, [0xc24ac8b8, 0x429db88e], [0xbf7fdc89, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]),
    row(95, "SpecialAirHi", -1, [0xc24437bc, 0x42989d1f], [0xbf7fdc88, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]),
    row(96, "SpecialAirHi", -1, [0xc23da6c0, 0x429381b0], [0xbf7fdc88, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]),
    row(97, "SpecialAirHi", -1, [0xc23715c4, 0x428e6641], [0xbf7fdc88, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]),
    row(98, "SpecialAirHi", -1, [0xc23084c8, 0x42894ad2], [0xbf7fdc88, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]),
    row(99, "SpecialAirHi", -1, [0xc229f3cc, 0x42842f63], [0xbf7fdc87, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]),
    row(100, "SpecialAirHi", -1, [0xc22362d0, 0x427e27e9], [0xbf7fdc87, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]),
    row(101, "SpecialAirHi", -1, [0xc21cd1d4, 0x4273f10c], [0xbf7fdc88, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]),
    row(102, "SpecialAirHi", -1, [0xc21640d8, 0x4269ba2f], [0xbf7fdc88, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]),
    row(103, "SpecialAirHi", -1, [0xc20fafdc, 0x425f8352], [0xbf7fdc88, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]),
    row(104, "SpecialAirHi", -1, [0xc2091ee0, 0x42554c75], [0xbf7fdc88, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]),
    row(105, "SpecialAirHi", -1, [0xc2028de4, 0x424b1598], [0xbf7fdc88, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]),
    row(106, "SpecialAirHi", -1, [0xc1f7f9d0, 0x4240debb], [0xbf7fdc88, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]),
    row(107, "SpecialAirHi", -1, [0xc1ead7d8, 0x4236a7de], [0xbf7fdc88, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]),
    row(108, "SpecialAirHi", -1, [0xc1ddb5e0, 0x422c7101], [0xbf7fdc88, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]),
    row(109, "SpecialAirHi", -1, [0xc1d093e8, 0x42223a24], [0xbf7fdc88, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]),
    row(110, "SpecialAirHi", -1, [0xc1c371f0, 0x42180347], [0xbf7fdc88, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]),
    row(111, "SpecialAirHi", -1, [0xc1b64ff8, 0x420dcc6a], [0xbf7fdc88, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]),
    row(112, "SpecialAirHi", -1, [0xc1a92e00, 0x4203958d], [0xbf7fdc88, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]),
    row(113, "SpecialAirHi", -1, [0xc19c0c08, 0x41f2bd60], [0xbf7fdc89, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]),
    row(114, "SpecialAirHi", -1, [0xc18eea10, 0x41de4fa6], [0xbf7fdc89, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]),
    row(115, "SpecialAirHi", -1, [0xc181c818, 0x41c9e1ec], [0xbf7fdc89, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]),
    row(116, "SpecialAirHi", -1, [0xc1694c41, 0x41b57432], [0xbf7fdc89, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]),
    row(117, "SpecialAirHiEnd", -1, [0xc17a190e, 0x41957432], [0x00000000, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]),
    row(118, "SpecialAirHiEnd", -1, [0xc18572ee, 0x416ae864], [0x00000000, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]),
    row(119, "SpecialAirHiEnd", -1, [0xc18dd955, 0x412ae864], [0x00000000, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]),
    row(120, "SpecialAirHiEnd", -1, [0xc1963fbc, 0x40d5d0c8], [0x00000000, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]),
    row(121, "SpecialAirHiEnd", -1, [0xc19ea623, 0x402ba190], [0x00000000, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]),
    row(122, "SpecialAirHiEnd", -1, [0xc1a70c8a, 0xbfa8bce0], [0x00000000, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]),
    row(123, "SpecialAirHiEnd", -1, [0xc1af72f1, 0xc0aa2f38], [0x00000000, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]),
    row(124, "SpecialAirHiEnd", -1, [0xc1b7d958, 0xc115179c], [0x00000000, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]),
];

/// `warp_pikachu`, N64 frames 40-71.
#[rustfmt::skip]
const PIKACHU: &[Row] = &[
    row(44, "Wait", 1, [0x00000000, 0x80000000], [0xbd500000, 0x3d840000, 0xbd600000], [0x3f800000, 0x3f800000, 0x3f800000]),
    row(45, "SpecialHiStart", 1, [0x00000000, 0x80000000], [0xbd500000, 0x3d840000, 0xbd600000], [0x3f800000, 0x3f800000, 0x3f800000]),
    row(46, "SpecialHiStart", 1, [0x00000000, 0x80000000], [0xbd500000, 0x3d840000, 0xbd600000], [0x3f800000, 0x3f800000, 0x3f800000]),
    row(47, "SpecialHiStart", 1, [0x00000000, 0x80000000], [0xbd500000, 0x3d840000, 0xbd600000], [0x3f800000, 0x3f800000, 0x3f800000]),
    row(48, "SpecialHiStart", 1, [0x00000000, 0x80000000], [0xbd500000, 0x3d840000, 0xbd600000], [0x3f800000, 0x3f800000, 0x3f800000]),
    row(49, "SpecialHiStart", 1, [0x00000000, 0x80000000], [0xbd500000, 0x3d840000, 0xbd600000], [0x3f800000, 0x3f800000, 0x3f800000]),
    row(50, "SpecialHiStart", 1, [0x00000000, 0x80000000], [0xbd500000, 0x3d840000, 0xbd600000], [0x3f800000, 0x3f800000, 0x3f800000]),
    row(51, "SpecialHiStart", 1, [0x00000000, 0x80000000], [0xbd500000, 0x3d840000, 0xbd600000], [0x3f800000, 0x3f800000, 0x3f800000]),
    row(52, "SpecialHiStart", 1, [0x00000000, 0x80000000], [0xbd500000, 0x3d840000, 0xbd600000], [0x3f800000, 0x3f800000, 0x3f800000]),
    row(53, "SpecialHiStart", 1, [0x00000000, 0x80000000], [0xbd500000, 0x3d840000, 0xbd600000], [0x3f800000, 0x3f800000, 0x3f800000]),
    row(54, "SpecialHiStart", 1, [0x00000000, 0x80000000], [0xbd500000, 0x3d840000, 0xbd600000], [0x3f800000, 0x3f800000, 0x3f800000]),
    row(55, "SpecialHiStart", 1, [0x00000000, 0x80000000], [0xbd500000, 0x3d840000, 0xbd600000], [0x3f800000, 0x3f800000, 0x3f800000]),
    row(56, "SpecialHiStart", 1, [0x00000000, 0x80000000], [0xbd500000, 0x3d840000, 0xbd600000], [0x3f800000, 0x3f800000, 0x3f800000]),
    row(57, "SpecialHiStart", 1, [0x00000000, 0x80000000], [0xbd500000, 0x3d840000, 0xbd600000], [0x3f800000, 0x3f800000, 0x3f800000]),
    row(58, "SpecialHiStart", 1, [0x00000000, 0x80000000], [0xbd500000, 0x3d840000, 0xbd600000], [0x3f800000, 0x3f800000, 0x3f800000]),
    row(59, "SpecialHiStart", 1, [0x00000000, 0x80000000], [0xbd500000, 0x3d840000, 0xbd600000], [0x3f800000, 0x3f800000, 0x3f800000]),
    row(60, "SpecialHiStart", 1, [0x00000000, 0x80000000], [0xbd500000, 0x3d840000, 0xbd600000], [0x3f800000, 0x3f800000, 0x3f800000]),
    row(61, "SpecialHiStart", 1, [0x00000000, 0x80000000], [0xbd500000, 0x3d840000, 0xbd600000], [0x3f800000, 0x3f800000, 0x3f800000]),
    row(62, "SpecialHiStart", 1, [0x00000000, 0x80000000], [0xbd500000, 0x3d840000, 0xbd600000], [0x3f800000, 0x3f800000, 0x3f800000]),
    row(63, "SpecialHiStart", 1, [0x00000000, 0x80000000], [0xbd500000, 0x3d840000, 0xbd600000], [0x3f800000, 0x3f800000, 0x3f800000]),
    row(64, "SpecialHiStart", 1, [0x00000000, 0x80000000], [0xbd500000, 0x3d840000, 0xbd600000], [0x3f800000, 0x3f800000, 0x3f800000]),
    row(65, "SpecialAirHi", -1, [0xc3327332, 0x438acb61], [0xbf7fdc8a, 0x3d840000, 0xbd600000], [0x3f4ccccd, 0x3f4ccccd, 0x3f99999a]),
    row(66, "SpecialAirHi", -1, [0xc3327332, 0x438acb61], [0xbf7fdc8a, 0x3d840000, 0xbd600000], [0x3f4ccccd, 0x3f4ccccd, 0x3f99999a]),
    row(67, "SpecialAirHi", -1, [0xc3327332, 0x438acb61], [0xbf7fdc8a, 0x3d840000, 0xbd600000], [0x3f4ccccd, 0x3f4ccccd, 0x3f99999a]),
    row(68, "SpecialAirHi", -1, [0xc3327332, 0x438acb61], [0xbf7fdc8a, 0x3d840000, 0xbd600000], [0x3f4ccccd, 0x3f4ccccd, 0x3f99999a]),
    row(69, "SpecialAirHi", -1, [0xc3327332, 0x438acb61], [0xbf7fdc8a, 0x3d840000, 0xbd600000], [0x3f4ccccd, 0x3f4ccccd, 0x3f99999a]),
    row(70, "SpecialAirHiEnd", -1, [0xc20cf5c2, 0x42456584], [0xbd500000, 0x3d840000, 0xbd600000], [0x3f800000, 0x3f800000, 0x3f800000]),
    row(71, "SpecialAirHiEnd", -1, [0xc20b28f5, 0x422f76ae], [0xbd500000, 0x3d840000, 0xbd600000], [0x3e998000, 0x40400000, 0x3e4cc000]),
    row(72, "SpecialAirHiEnd", -1, [0xc2095c28, 0x421bf7b7], [0xbd500000, 0x3d840000, 0xbd600000], [0x3e4cc000, 0x40a00000, 0x3dcc8000]),
    row(73, "SpecialAirHiEnd", -1, [0xc2078f5b, 0x420aa34d], [0xbd500000, 0x3d840000, 0xbd600000], [0x3e998000, 0x403ffc00, 0x3e4cc000]),
    row(74, "SpecialAirHiEnd", -1, [0xc205c28e, 0x41f677a5], [0xbd500000, 0x3d840000, 0xbd600000], [0x3f800000, 0x3f800000, 0x3f800000]),
    row(75, "SpecialAirHiEnd", -1, [0xc203f5c1, 0x41db1504], [0xbd500000, 0x3d840000, 0xbd600000], [0x3eccc000, 0x3e4cc000, 0x402fd400]),
];

fn open_pack() -> Option<Vec<u8>> {
    ssb_tablegen::test_rom(env!("CARGO_MANIFEST_DIR"))?;
    std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/generated/ssb64.pak"),
    )
    .ok()
}

/// The scene's pad replayed through the posed fighter, one row per tick
/// up to `last`, in the measured rows' terms.
fn replay(
    pack: &ssb_rom::pack::Pack<'_>,
    scene: GameScene,
    kind: FighterKind,
    last: u64,
) -> Vec<Row> {
    let pad = |tick| {
        let pad = script::fighter_pad(scene, tick).unwrap();
        (pad.buttons, pad.stick_x, pad.stick_y)
    };
    replay_pad(pack, DREAM_LAND, kind, last, &pad).0
}

/// Pack stage index of Dream Land and of Peach's Castle.
const DREAM_LAND: u32 = 0;
const CASTLE: u32 = 4;

/// `pad(tick)` (buttons, stick x, stick y) replayed through the posed
/// fighter on pack stage `stage`, one row and one position per tick up to
/// `last`. A C button's press and release are the stick's jump edges.
fn replay_pad(
    pack: &ssb_rom::pack::Pack<'_>,
    stage: u32,
    kind: FighterKind,
    last: u64,
    pad: &dyn Fn(u64) -> (u16, i8, i8),
) -> (Vec<Row>, Vec<[f32; 2]>) {
    let (stage, segments) = stage_floors(pack, stage);
    let map = dream_land_map(pack, &stage);
    let map_fn = || map.iter().copied();
    let mut p = Posed::new(pack, training_fighter(pack, &stage, kind, 0, &segments));
    let first_node = pack.object(p.object).map(|o| o.first_node).unwrap();
    let mut rows = Vec::new();
    let mut positions = Vec::new();
    let mut jump_was_held = false;
    for tick in FIRST_TICK..=last {
        let (buttons, stick_x, stick_y) = pad(tick);
        let held = buttons
            & (N64Buttons::C_UP | N64Buttons::C_DOWN | N64Buttons::C_LEFT | N64Buttons::C_RIGHT)
            != 0;
        let (tapped, released) = (held && !jump_was_held, !held && jump_was_held);
        jump_was_held = held;
        let input = ControllerState {
            buttons: N64Buttons(buttons),
            stick_x,
            stick_y,
            connected: true,
        };
        let top = posed::map_bound_top(&p);
        p.tick_interrupt(pack, top, input, tapped, released, &map_fn);
        p.tick_physics(pack, &map_fn);
        let _ = p.fighter.take_weapon_spawn();
        p.fighter.resolve_cliff_release(&map_fn);
        p.settle_status(pack);
        let pose = p
            .skeleton
            .node_pose(first_node)
            .copied()
            .unwrap_or_default();
        let f = &p.fighter;
        let status = format!("{:?}", f.status.status);
        rows.push(Row {
            tick,
            status: Box::leak(status.into_boxed_str()),
            lr: f.facing.sign() as i32,
            vel: [f.physics.vel_air.x.to_bits(), f.physics.vel_air.y.to_bits()],
            rotate: pose.rotate.map(f32::to_bits),
            scale: pose.scale.map(f32::to_bits),
            ga: u32::from(!f.is_grounded()),
        });
        positions.push([f.pos.x, f.pos.y]);
    }
    (rows, positions)
}

fn compare(scene: GameScene, kind: FighterKind, measured: &[Row]) {
    let Some(bytes) = open_pack() else {
        return;
    };
    let pack = ssb_rom::pack::Pack::open(&bytes).unwrap();
    let last = measured.last().unwrap().tick;
    let port = replay(&pack, scene, kind, last);
    check(&port, &[], measured);
}

/// Each measured row against the port's row of the same tick.
fn check(port: &[Row], positions: &[[f32; 2]], measured: &[Row]) {
    let mut failures = Vec::new();
    for n64 in measured {
        let at = port.iter().position(|r| r.tick == n64.tick).unwrap();
        let psp = &port[at];
        let fl = |b: u32| f32::from_bits(b);
        let same = psp.status.ends_with(&format!("({})", n64.status))
            && (psp.lr == n64.lr || !n64.status.contains("Air"))
            && (n64.ga == NO_GA || psp.ga == n64.ga)
            // A grounded `vel_air.y` is `(lr * -floor_angle.x) * vel_ground.x`
            // (`ftPhysicsSetGroundVelTransferAir`): before the launch, where the
            // stages face the fighter apart, its zero carries the facing's
            // sign, so only there the value is compared and not the bits.
            && (psp.vel == n64.vel
                || (psp.lr != n64.lr && psp.vel.map(fl) == n64.vel.map(fl)))
            && psp.rotate == n64.rotate
            && psp.scale == n64.scale;
        let line = format!(
            "tick {}: N64 {} lr {} vel ({}, {}) rot {:?} scale {:?} | port {} lr {} vel ({}, {}) rot {:?} scale {:?}",
            n64.tick,
            n64.status,
            n64.lr,
            fl(n64.vel[0]),
            fl(n64.vel[1]),
            n64.rotate.map(fl),
            n64.scale.map(fl),
            psp.status,
            psp.lr,
            fl(psp.vel[0]),
            fl(psp.vel[1]),
            psp.rotate.map(fl),
            psp.scale.map(fl),
        );
        let line = match (n64.ga, positions.get(at)) {
            (NO_GA, _) => line,
            (ga, pos) => format!("{line} | ga N64 {ga} port {} pos {pos:?}", psp.ga),
        };
        if std::env::var_os("SCENE_TRACE").is_some() {
            println!("{}{line}", if same { "  " } else { "! " });
        }
        if !same {
            failures.push(line);
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} ticks differ from the N64:\n{}",
        failures.len(),
        measured.len(),
        failures.join("\n")
    );
}

#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
#[test]
fn fire_fox_pitch_matches_the_n64() {
    compare(GameScene::FoxFire, FighterKind::Fox, FOX);
}

#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
#[test]
fn quick_attack_pitch_and_stretch_match_the_n64() {
    compare(GameScene::PikachuQuick, FighterKind::Pikachu, PIKACHU);
}

/// N64 pad windows `(start, len, buttons, x, y)` in N64 frames, as
/// `pitch_trace.py` takes them; frame `t` is tick `t + 4`.
fn windows_pad(windows: &'static [(u64, u64, u16, i8, i8)]) -> impl Fn(u64) -> (u16, i8, i8) {
    move |tick| {
        let t = tick.wrapping_sub(4);
        windows
            .iter()
            .rev()
            .find(|w| w.0 <= t && t < w.0 + w.1)
            .map_or((0, 0, 0), |w| (w.2, w.3, w.4))
    }
}

/// Fox on the Castle roof starts Fire Fox with the stick up, then holds it
/// straight down: the launch passes the roof and bounds off the floor
/// below (`4:1:b+y=80;5:100:y=-80`).
const BOUND_PAD: &[(u64, u64, u16, i8, i8)] = &[(4, 1, N64Buttons::B, 0, 80), (5, 100, 0, 0, -80)];

/// `warp_fox` on Peach's Castle with [`BOUND_PAD`], N64 frames 40-90: the
/// launch bounds off the floor at frame 54 and stays airborne (`ga` 1)
/// through `SpecialAirHiBound` and `FallSpecial` (`ftfoxspecialhi.c:459`).
#[rustfmt::skip]
const BOUND: &[Row] = &[
    Row { ga: 0, ..row(44, "SpecialHiHold", 1, [0x00000000, 0x80000000], [0x00000000, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]) },
    Row { ga: 0, ..row(45, "SpecialHiHold", 1, [0x00000000, 0x80000000], [0x00000000, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]) },
    Row { ga: 0, ..row(46, "SpecialHiHold", 1, [0x00000000, 0x80000000], [0x00000000, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]) },
    Row { ga: 0, ..row(47, "SpecialHiHold", 1, [0x00000000, 0x80000000], [0x00000000, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]) },
    Row { ga: 0, ..row(48, "SpecialHiHold", 1, [0x00000000, 0x80000000], [0x00000000, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]) },
    Row { ga: 0, ..row(49, "SpecialHiHold", 1, [0x00000000, 0x80000000], [0x00000000, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]) },
    Row { ga: 1, ..row(50, "SpecialAirHi", 1, [0xb6a8abf7, 0xc2e60000], [0xc096cbe4, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]) },
    Row { ga: 1, ..row(51, "SpecialAirHi", 1, [0xb6a4381f, 0xc2dfedb7], [0xc096cbe4, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]) },
    Row { ga: 1, ..row(52, "SpecialAirHi", 1, [0xb69fc447, 0xc2d9db6e], [0xc096cbe4, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]) },
    Row { ga: 1, ..row(53, "SpecialAirHi", 1, [0xb69b506f, 0xc2d3c925], [0xc096cbe4, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]) },
    Row { ga: 1, ..row(54, "SpecialAirHi", 1, [0xb696dc97, 0xc2cdb6dc], [0xc096cbe4, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]) },
    Row { ga: 1, ..row(55, "SpecialAirHi", 1, [0xb69268bf, 0xc2c7a493], [0xc096cbe4, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]) },
    Row { ga: 1, ..row(56, "SpecialAirHi", 1, [0xb68df4e7, 0xc2c1924a], [0xc096cbe4, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]) },
    Row { ga: 1, ..row(57, "SpecialAirHi", 1, [0xb689810f, 0xc2bb8001], [0xc096cbe4, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]) },
    Row { ga: 1, ..row(58, "SpecialAirHiBound", 1, [0xb6850d37, 0xc2b56db8], [0x00000000, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]) },
    Row { ga: 1, ..row(59, "SpecialAirHiBound", 1, [0x00000000, 0x422edc72], [0x3fa2f9a8, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]) },
    Row { ga: 1, ..row(60, "SpecialAirHiBound", 1, [0x00000000, 0x4224071c], [0x40103cd4, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]) },
    Row { ga: 1, ..row(61, "SpecialAirHiBound", 1, [0x00000000, 0x42185c72], [0x4042ae46, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]) },
    Row { ga: 1, ..row(62, "SpecialAirHiBound", 1, [0x00000000, 0x420bdc70], [0x406f3f72, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]) },
    Row { ga: 1, ..row(63, "SpecialAirHiBound", 1, [0x00000000, 0x41fd0e38], [0x408e2f4d, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]) },
    Row { ga: 1, ..row(64, "SpecialAirHiBound", 1, [0x00000000, 0x41e0b8e8], [0x40a83d02, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]) },
    Row { ga: 1, ..row(65, "SpecialAirHiBound", 1, [0x00000000, 0x41c2b8e8], [0x40c90000, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]) },
    Row { ga: 1, ..row(66, "SpecialAirHiBound", 1, [0x00000000, 0x41a30e38], [0x40ebf24c, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]) },
    Row { ga: 1, ..row(67, "SpecialAirHiBound", 1, [0x00000000, 0x4181b8e0], [0x41054c93, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]) },
    Row { ga: 1, ..row(68, "SpecialAirHiBound", 1, [0x00000000, 0x413d71a0], [0x41129e3d, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]) },
    Row { ga: 1, ..row(69, "SpecialAirHiBound", 1, [0x00000000, 0x40e838c0], [0x411e121a, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]) },
    Row { ga: 1, ..row(70, "SpecialAirHiBound", 1, [0x00000000, 0x401dc800], [0x4127cc21, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]) },
    Row { ga: 1, ..row(71, "SpecialAirHiBound", 1, [0x00000000, 0xc01dc700], [0x412ff049, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]) },
    Row { ga: 1, ..row(72, "SpecialAirHiBound", 1, [0x00000000, 0xc0e83900], [0x4136a289, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]) },
    Row { ga: 1, ..row(73, "SpecialAirHiBound", 1, [0x00000000, 0xc13d71c0], [0x413c06d7, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]) },
    Row { ga: 1, ..row(74, "SpecialAirHiBound", 1, [0x00000000, 0xc181b8e0], [0x41404128, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]) },
    Row { ga: 1, ..row(75, "SpecialAirHiBound", 1, [0x00000000, 0xc1a30e30], [0x41437578, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]) },
    Row { ga: 1, ..row(76, "SpecialAirHiBound", 1, [0x00000000, 0xc1c2b8f0], [0x4145c7b6, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]) },
    Row { ga: 1, ..row(77, "SpecialAirHiBound", 1, [0x00000000, 0xc1e0b8e0], [0x41475bde, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]) },
    Row { ga: 1, ..row(78, "FallSpecial", 1, [0x00000000, 0xc2005c70], [0x00000000, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]) },
    Row { ga: 1, ..row(79, "FallSpecial", 1, [0x00000000, 0xc2105c70], [0x00000000, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]) },
    Row { ga: 1, ..row(80, "FallSpecial", 1, [0x00000000, 0xc2205c70], [0x00000000, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]) },
    Row { ga: 1, ..row(81, "FallSpecial", 1, [0x00000000, 0xc2305c70], [0x00000000, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]) },
    Row { ga: 0, ..row(82, "LandingFallSpecial", 1, [0x00000000, 0xc2405c70], [0x3e6a0000, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]) },
    Row { ga: 0, ..row(83, "LandingFallSpecial", 1, [0x00000000, 0x80000000], [0x3e68c497, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]) },
    Row { ga: 0, ..row(84, "LandingFallSpecial", 1, [0x00000000, 0x80000000], [0x3e653728, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]) },
    Row { ga: 0, ..row(85, "LandingFallSpecial", 1, [0x00000000, 0x80000000], [0x3e5f8ee2, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]) },
    Row { ga: 0, ..row(86, "LandingFallSpecial", 1, [0x00000000, 0x80000000], [0x3e5802f0, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]) },
    Row { ga: 0, ..row(87, "LandingFallSpecial", 1, [0x00000000, 0x80000000], [0x3e4eca86, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]) },
    Row { ga: 0, ..row(88, "LandingFallSpecial", 1, [0x00000000, 0x80000000], [0x3e441cd0, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]) },
    Row { ga: 0, ..row(89, "LandingFallSpecial", 1, [0x00000000, 0x80000000], [0x3e3830fc, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]) },
    Row { ga: 0, ..row(90, "LandingFallSpecial", 1, [0x00000000, 0x80000000], [0x3e2b3e3b, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]) },
    Row { ga: 0, ..row(91, "LandingFallSpecial", 1, [0x00000000, 0x80000000], [0x3e1d7bba, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]) },
    Row { ga: 0, ..row(92, "LandingFallSpecial", 1, [0x00000000, 0x80000000], [0x3e0f20a9, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]) },
    Row { ga: 0, ..row(93, "LandingFallSpecial", 1, [0x00000000, 0x80000000], [0x3e006436, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]) },
    Row { ga: 0, ..row(94, "SquatWait", 1, [0x00000000, 0x80000000], [0x00000000, 0x00000000, 0x00000000], [0x3f800000, 0x3f800000, 0x3f800000]) },
];

#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
#[test]
fn fire_fox_bound_matches_the_n64() {
    let Some(bytes) = open_pack() else {
        return;
    };
    let pack = ssb_rom::pack::Pack::open(&bytes).unwrap();
    let last = BOUND.last().unwrap().tick;
    let (port, positions) = replay_pad(
        &pack,
        CASTLE,
        FighterKind::Fox,
        last,
        &windows_pad(BOUND_PAD),
    );
    check(&port, &positions, BOUND);
}

/// Pikachu on the Castle roof starts Quick Attack with the stick up, then
/// holds it sideways: the zip runs along the roof and ends grounded
/// (`4:1:b+y=80;5:70:x=80`).
const QUICK_END_PAD: &[(u64, u64, u16, i8, i8)] =
    &[(4, 1, N64Buttons::B, 0, 80), (5, 70, 0, 80, 0)];

/// `warp_pikachu` on Peach's Castle with [`QUICK_END_PAD`], N64 frames
/// 24-74: the grounded `SpecialHiEnd` holds still until motion flag 1
/// (`ftPikachuSpecialHiEndProcPhysics` @ `ftpikachuspecialhi.c:449`), then
/// Wait's `proc_interrupt` walks on the frame the End ends.
///
/// Left out, as the one-fighter replay cannot have them:
/// - frames 20-23 (`SpecialHiStart`): the N64 joint keeps the Wait pose of
///   frames 0-3, which the replay (from frame 4) never plays;
/// - frames 27-28: P1, standing at x 765, jostles the zip by -6.75 then
///   +6.75 in `vel_air.x`; N64 and port both reach x 1272.4458 on frame 28.
#[rustfmt::skip]
const QUICK_END: &[Row] = &[
    Row { ga: 1, ..row(28, "SpecialAirHi", 1, [0x43a50000, 0x00000000], [0x00000000, 0x3d840000, 0xbd600000], [0x3f4ccccd, 0x3f4ccccd, 0x3f99999a]) },
    Row { ga: 1, ..row(29, "SpecialAirHi", 1, [0x43a50000, 0x00000000], [0x00000000, 0x3d840000, 0xbd600000], [0x3f4ccccd, 0x3f4ccccd, 0x3f99999a]) },
    Row { ga: 0, ..row(30, "SpecialHi", 1, [0x43a50000, 0x00000000], [0xbd500000, 0x3d840000, 0xbd600000], [0x3f800000, 0x3f800000, 0x3f800000]) },
    Row { ga: 0, ..row(33, "SpecialHiEnd", 1, [0x00000000, 0x00000000], [0xbd500000, 0x3d840000, 0xbd600000], [0x3f800000, 0x3f800000, 0x3f800000]) },
    Row { ga: 0, ..row(34, "SpecialHiEnd", 1, [0x00000000, 0x00000000], [0xbd500000, 0x3d840000, 0xbd600000], [0x3e998000, 0x40400000, 0x3e4cc000]) },
    Row { ga: 0, ..row(35, "SpecialHiEnd", 1, [0x00000000, 0x00000000], [0xbd500000, 0x3d840000, 0xbd600000], [0x3e4cc000, 0x40a00000, 0x3dcc8000]) },
    Row { ga: 0, ..row(36, "SpecialHiEnd", 1, [0x00000000, 0x00000000], [0xbd500000, 0x3d840000, 0xbd600000], [0x3e998000, 0x403ffc00, 0x3e4cc000]) },
    Row { ga: 0, ..row(37, "SpecialHiEnd", 1, [0x00000000, 0x00000000], [0xbd500000, 0x3d840000, 0xbd600000], [0x3f800000, 0x3f800000, 0x3f800000]) },
    Row { ga: 0, ..row(38, "SpecialHiEnd", 1, [0x00000000, 0x00000000], [0xbd500000, 0x3d840000, 0xbd600000], [0x3eccc000, 0x3e4cc000, 0x402fd400]) },
    Row { ga: 0, ..row(39, "SpecialHiEnd", 1, [0x00000000, 0x00000000], [0xbd500000, 0x3d840000, 0xbd600000], [0x3dcc8000, 0x3dcc8000, 0x40900000]) },
    Row { ga: 0, ..row(40, "SpecialHiEnd", 1, [0x00000000, 0x00000000], [0xbd500000, 0x3d840000, 0xbd600000], [0x3eccc000, 0x3e4cc000, 0x40289c00]) },
    Row { ga: 0, ..row(41, "SpecialHiEnd", 1, [0x42666934, 0xc0d8db7d], [0xbd500000, 0x3d840000, 0xbd600000], [0x3f800000, 0x3f800000, 0x3f800000]) },
    Row { ga: 0, ..row(42, "SpecialHiEnd", 1, [0x4246a150, 0xc0baf22e], [0xbd500000, 0x3d840000, 0xbd600000], [0x3f800000, 0x3f800000, 0x3f800000]) },
    Row { ga: 0, ..row(43, "SpecialHiEnd", 1, [0x421ad74b, 0xc1825b8a], [0xbd500000, 0x3d840000, 0xbd600000], [0x3f800000, 0x3f800000, 0x3f800000]) },
    Row { ga: 0, ..row(44, "SpecialHiEnd", 1, [0x41fab1e7, 0xc1530e1d], [0xbd4fffff, 0x3d840000, 0xbd600000], [0x3f800000, 0x3f800000, 0x3f800000]) },
    Row { ga: 0, ..row(45, "SpecialHiEnd", 1, [0x41bfb538, 0xc1216525], [0xbd500000, 0x3d840000, 0xbd600000], [0x3f800000, 0x3f800000, 0x3f800000]) },
    Row { ga: 0, ..row(46, "SpecialHiEnd", 1, [0x4184b889, 0xc0df785b], [0xbd500000, 0x3d840000, 0xbd600000], [0x3f800000, 0x3f800000, 0x3f800000]) },
    Row { ga: 0, ..row(47, "SpecialHiEnd", 1, [0x411377b5, 0xc0784cd7], [0xbd500000, 0x3d840000, 0xbd600000], [0x3f800000, 0x3f800000, 0x3f800000]) },
    Row { ga: 0, ..row(48, "SpecialHiEnd", 1, [0x3febf2bb, 0xbf46a3df], [0xbd500000, 0x3d840000, 0xbd600000], [0x3f800000, 0x3f800000, 0x3f800000]) },
    Row { ga: 0, ..row(49, "SpecialHiEnd", 1, [0x00000000, 0x80000000], [0xbd500000, 0x3d840000, 0xbd600000], [0x3f800000, 0x3f800000, 0x3f800000]) },
    Row { ga: 0, ..row(50, "SpecialHiEnd", 1, [0x00000000, 0x80000000], [0xbd500000, 0x3d840000, 0xbd600000], [0x3f800000, 0x3f800000, 0x3f800000]) },
    Row { ga: 0, ..row(51, "SpecialHiEnd", 1, [0x00000000, 0x80000000], [0xbd500000, 0x3d840000, 0xbd600000], [0x3f800000, 0x3f800000, 0x3f800000]) },
    Row { ga: 0, ..row(52, "SpecialHiEnd", 1, [0x00000000, 0x80000000], [0xbd500000, 0x3d840000, 0xbd600000], [0x3f800000, 0x3f800000, 0x3f800000]) },
    Row { ga: 0, ..row(53, "SpecialHiEnd", 1, [0x00000000, 0x80000000], [0xbd500000, 0x3d840000, 0xbd600000], [0x3f800000, 0x3f800000, 0x3f800000]) },
    Row { ga: 0, ..row(54, "SpecialHiEnd", 1, [0x00000000, 0x80000000], [0xbd500000, 0x3d840000, 0xbd600000], [0x3f800000, 0x3f199000, 0x3fa66000]) },
    Row { ga: 0, ..row(55, "SpecialHiEnd", 1, [0x00000000, 0x80000000], [0xbd500000, 0x3d840000, 0xbd600000], [0x3f800000, 0x3f4cc800, 0x3f933000]) },
    Row { ga: 0, ..row(56, "SpecialHiEnd", 1, [0x00000000, 0x80000000], [0xbd500000, 0x3d840000, 0xbd600000], [0x3f800000, 0x3f800000, 0x3f800000]) },
    Row { ga: 0, ..row(57, "SpecialHiEnd", 1, [0x00000000, 0x80000000], [0xbd500000, 0x3d840000, 0xbd600000], [0x3f800000, 0x3f800000, 0x3f800000]) },
    Row { ga: 0, ..row(58, "SpecialHiEnd", 1, [0x00000000, 0x80000000], [0xbd500000, 0x3d840000, 0xbd600000], [0x3f800000, 0x3f800000, 0x3f800000]) },
    Row { ga: 0, ..row(59, "SpecialHiEnd", 1, [0x00000000, 0x80000000], [0xbd500000, 0x3d840000, 0xbd600000], [0x3f800000, 0x3f800000, 0x3f800000]) },
    Row { ga: 0, ..row(60, "SpecialHiEnd", 1, [0x00000000, 0x80000000], [0xbd500000, 0x3d840000, 0xbd600000], [0x3f800000, 0x3f800000, 0x3f800000]) },
    Row { ga: 0, ..row(61, "SpecialHiEnd", 1, [0x00000000, 0x80000000], [0xbd500000, 0x3d840000, 0xbd600000], [0x3f800000, 0x3f800000, 0x3f800000]) },
    Row { ga: 0, ..row(62, "SpecialHiEnd", 1, [0x00000000, 0x80000000], [0xbd500000, 0x3d840000, 0xbd600000], [0x3f800000, 0x3f800000, 0x3f800000]) },
    Row { ga: 0, ..row(63, "SpecialHiEnd", 1, [0x00000000, 0x80000000], [0xbd500000, 0x3d840000, 0xbd600000], [0x3f800000, 0x3f800000, 0x3f800000]) },
    Row { ga: 0, ..row(64, "SpecialHiEnd", 1, [0x00000000, 0x80000000], [0xbd500000, 0x3d840000, 0xbd600000], [0x3f800000, 0x3f800000, 0x3f800000]) },
    Row { ga: 0, ..row(65, "SpecialHiEnd", 1, [0x00000000, 0x80000000], [0xbd500000, 0x3d840000, 0xbd600000], [0x3f800000, 0x3f800000, 0x3f800000]) },
    Row { ga: 0, ..row(66, "SpecialHiEnd", 1, [0x00000000, 0x80000000], [0xbd500000, 0x3d840000, 0xbd600000], [0x3f800000, 0x3f800000, 0x3f800000]) },
    Row { ga: 0, ..row(67, "SpecialHiEnd", 1, [0x00000000, 0x80000000], [0xbd500000, 0x3d840000, 0xbd600000], [0x3f800000, 0x3f800000, 0x3f800000]) },
    Row { ga: 0, ..row(68, "SpecialHiEnd", 1, [0x00000000, 0x80000000], [0xbd500000, 0x3d840000, 0xbd600000], [0x3f800000, 0x3f800000, 0x3f800000]) },
    Row { ga: 0, ..row(69, "SpecialHiEnd", 1, [0x00000000, 0x80000000], [0xbd500000, 0x3d840000, 0xbd600000], [0x3f800000, 0x3f800000, 0x3f800000]) },
    Row { ga: 0, ..row(70, "SpecialHiEnd", 1, [0x00000000, 0x80000000], [0xbd500000, 0x3d840000, 0xbd600000], [0x3f800000, 0x3f800000, 0x3f800000]) },
    Row { ga: 0, ..row(71, "SpecialHiEnd", 1, [0x00000000, 0x80000000], [0xbd500000, 0x3d840000, 0xbd600000], [0x3f800000, 0x3f800000, 0x3f800000]) },
    Row { ga: 0, ..row(72, "SpecialHiEnd", 1, [0x00000000, 0x80000000], [0xbd500000, 0x3d840000, 0xbd600000], [0x3f800000, 0x3f800000, 0x3f800000]) },
    Row { ga: 0, ..row(73, "SpecialHiEnd", 1, [0x00000000, 0x80000000], [0xbd500000, 0x3d840000, 0xbd600000], [0x3f800000, 0x3f800000, 0x3f800000]) },
    Row { ga: 0, ..row(74, "SpecialHiEnd", 1, [0x00000000, 0x80000000], [0xbd500000, 0x3d840000, 0xbd600000], [0x3f800000, 0x3f800000, 0x3f800000]) },
    Row { ga: 0, ..row(75, "SpecialHiEnd", 1, [0x00000000, 0x80000000], [0xbd500000, 0x3d840000, 0xbd600000], [0x3f800000, 0x3f800000, 0x3f800000]) },
    Row { ga: 0, ..row(76, "SpecialHiEnd", 1, [0x00000000, 0x80000000], [0xbd500000, 0x3d840000, 0xbd600000], [0x3f800000, 0x3f800000, 0x3f800000]) },
    Row { ga: 0, ..row(77, "SpecialHiEnd", 1, [0x00000000, 0x80000000], [0xbd500000, 0x3d840000, 0xbd600000], [0x3f800000, 0x3f800000, 0x3f800000]) },
    Row { ga: 0, ..row(78, "WalkFast", 1, [0x41f7bedd, 0xc1509276], [0xbd500000, 0x3d840000, 0xbd600000], [0x3f800000, 0x3f800000, 0x3f800000]) },
];

#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
#[test]
fn quick_attack_ground_end_matches_the_n64() {
    let Some(bytes) = open_pack() else {
        return;
    };
    let pack = ssb_rom::pack::Pack::open(&bytes).unwrap();
    let last = QUICK_END.last().unwrap().tick;
    let (port, positions) = replay_pad(
        &pack,
        CASTLE,
        FighterKind::Pikachu,
        last,
        &windows_pad(QUICK_END_PAD),
    );
    check(&port, &positions, QUICK_END);
}
