//! The scripted pad of the `psp-game` fighter-move scenes.
//!
//! One fighter in Training presses buttons on a fixed schedule so that its
//! capture tick shows a move (a Charge Shot, a Boomerang, a shield). The
//! schedule lives here, not in `psp-game`, so a host test can replay it
//! (`crates/ssb-rom/tests/scene_inputs.rs`) and assert that the fighter
//! reaches the move the scene is named for: an A at tick 8 starts a jab
//! that swallows a B a few ticks later, and nothing else notices
//! (RE-516).
//!
//! `tick` is the scene's tick counter, the one [`GameScene`]'s capture tick
//! is measured on; Training's first battle frame is tick 9.
//!
//! This is input only. Scenes that need a route to the dummy or an item
//! (the grab, items, menus) keep their scripts in `psp-game`.

use crate::GameScene;

/// The tick a special move is pressed in the scenes that press one after
/// Training's opening jab. The A at tick 8 starts `Attack11` on Training's
/// first frame, and a jab holds the fighter for 17 to 33 ticks (Mario,
/// Kirby, Ness and Jigglypuff 17, Samus and Captain Falcon 21, Link and
/// Yoshi 23, Donkey Kong 24, Pikachu 33); an input inside it is swallowed.
/// The scripts press at 45, past the longest, and the capture ticks are
/// 25 later than the old ones (a press at 20) so each scene shows the same
/// frame of its move (RE-516).
const SPECIAL_TICK: u64 = 45;

/// The first press of the Fox and Luigi scenes' blaster and Fireball: in
/// the air, seven ticks after the C-Up jump the default script presses at
/// 33 (RE-516).
const AIR_SPECIAL_TICK: u64 = 40;

/// `foxfire` and `pikachuquick`'s stick: up past the special-high range
/// and left, the way Training's spawn faces; `|x| + |y|` is well past
/// Fire Fox's 45-unit angled gate.
const DIAGONAL_X: i8 = -45;
const DIAGONAL_Y: i8 = 70;

/// `nessjibaku`'s route (RE-503): run right from the jab's end until Ness
/// brakes under Dream Land's right platform (x 1393; the left one holds
/// the dummy, which PK Thunder's head would hit), jump onto it, Up+B on
/// it; steer down-right from [`JIBAKU_STEER`], left from
/// [`JIBAKU_STEER_LEFT`].
const JIBAKU_RUN: u64 = 30;
const JIBAKU_RUN_END: u64 = 57;
const JIBAKU_JUMP_TICK: u64 = 80;
const JIBAKU_B_TICK: u64 = 140;
const JIBAKU_STEER: u64 = 170;
const JIBAKU_STEER_LEFT: u64 = 191;
/// `kirbystoneslope`'s Down+B, Kirby standing on Kongo Jungle's left slope
/// (RE-503).
const STONE_SLOPE_B_TICK: u64 = 120;
/// `captainkickroll`'s Down+B, Captain Falcon facing left on Peach's
/// Castle's roof (RE-503).
const KICK_ROLL_B_TICK: u64 = 60;
/// `kirbycutter`'s route: Kirby dashes left from the jab's end to the left
/// bridge's end on Peach's Castle (x -1940, where the N64 trace's Kirby
/// stops), then Up+B there.
const CUTTER_RUN: core::ops::Range<u64> = 30..70;
const CUTTER_B_TICK: u64 = 95;

/// The delay of the jump route the dummy scenes follow. The route was
/// found for a fighter standing free from tick 13; the jab runs to tick 25
/// (Mario, Kirby) or 31 (Yoshi) and swallowed its first jump (RE-516).
const ROUTE_DELAY: u64 = 17;

/// `kirbywiggle`: from this tick the captured dummy reads the pad
/// (`SpecialNWait` from about 166).
pub const KIRBY_WIGGLE_MIRROR_TICK: u64 = 190 + ROUTE_DELAY;
/// `kirbywiggle`: the stick tap the dummy wiggles with, mid-clip.
pub const KIRBY_WIGGLE_TICK: u64 = 200 + ROUTE_DELAY;

/// N64 button bits, as `ssb_engine::input::N64Buttons` (a test in
/// `ssb-rom` checks them against it).
pub mod button {
    pub const A: u16 = 0x8000;
    pub const B: u16 = 0x4000;
    pub const Z: u16 = 0x2000;
    pub const C_UP: u16 = 0x0008;
    pub const L: u16 = 0x0020;
}

/// One tick of the scripted pad.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Pad {
    pub buttons: u16,
    pub stick_x: i8,
    pub stick_y: i8,
}

/// The pad the scene's script holds at `tick`, or `None` for a scene
/// whose script is not in this module.
pub fn fighter_pad(scene: GameScene, tick: u64) -> Option<Pad> {
    is_fighter_scene(scene).then(|| Pad {
        buttons: buttons(scene, tick),
        stick_x: stick_x(scene, tick),
        stick_y: stick_y(scene, tick),
    })
}

/// The pattern of the scenes [`fighter_pad`] scripts and [`capture_tick`]
/// times. `psp-game` matches its scenes exhaustively with this pattern as
/// one arm, so a new scene fails to compile there until it is timed.
#[macro_export]
macro_rules! scripted_scene {
    () => {
        $crate::GameScene::Samus
            | $crate::GameScene::SamusShot
            | $crate::GameScene::SamusBomb
            | $crate::GameScene::Link
            | $crate::GameScene::LinkSpin
            | $crate::GameScene::LinkSword
            | $crate::GameScene::Yoshi
            | $crate::GameScene::YoshiBomb
            | $crate::GameScene::Captain
            | $crate::GameScene::CaptainKick
            | $crate::GameScene::Kirby
            | $crate::GameScene::KirbyJab
            | $crate::GameScene::Pikachu
            | $crate::GameScene::PikachuSmash
            | $crate::GameScene::Purin
            | $crate::GameScene::Donkey
            | $crate::GameScene::Ness
            | $crate::GameScene::NessThunder
            | $crate::GameScene::NessMagnet
            | $crate::GameScene::LinkBomb
            | $crate::GameScene::KirbyHat
            | $crate::GameScene::KirbySpit
            | $crate::GameScene::KirbyLose
            | $crate::GameScene::KirbyWiggle
            | $crate::GameScene::YoshiEgg
            | $crate::GameScene::PikachuThunder
            | $crate::GameScene::PikachuAir
            | $crate::GameScene::Fox
            | $crate::GameScene::Luigi
            | $crate::GameScene::Fireball
            | $crate::GameScene::Superjump
            | $crate::GameScene::Grab
            | $crate::GameScene::Jab
            | $crate::GameScene::Shield
            | $crate::GameScene::YoshiShield
            | $crate::GameScene::YoshiRollF
            | $crate::GameScene::YoshiRollB
            | $crate::GameScene::YoshiShieldBreak
            | $crate::GameScene::Shadows
            | $crate::GameScene::FoxFire
            | $crate::GameScene::PikachuQuick
            | $crate::GameScene::NessJibaku
            | $crate::GameScene::KirbyStoneSlope
            | $crate::GameScene::CaptainKickRoll
            | $crate::GameScene::KirbyCutter
    };
}

/// The scenes [`fighter_pad`] scripts.
pub const fn is_fighter_scene(scene: GameScene) -> bool {
    matches!(scene, crate::scripted_scene!())
}

/// The tick the scene is captured on, `Some` exactly for the scenes
/// [`scripted_scene`] matches.
pub const fn capture_tick(scene: GameScene) -> Option<u64> {
    use GameScene::*;
    Some(match scene {
        Fireball => 167,
        // B+up is pressed at tick 150; frame 2's strong opening hit has
        // resolved by this point while the source TransN launch is still
        // clearly visible.
        Superjump => 156,
        Fox => 66,
        // B at tick 40, as in the Fox scene; the Fireball is in flight.
        Luigi => 66,
        // B at tick 40, once Training's opening jab is over;
        // `SpecialNStart` runs 16 frames, then the charge grows a level every
        // 20 frames. Level 2 is on the arm cannon here.
        Samus => 100,
        // A second B at tick 80 ends the charge; the level-1 shot is in
        // flight a few frames later.
        SamusShot => 84,
        // Down+B at tick 45; the Bomb spawns on `SpecialLw` frame 10 and
        // rests under Samus. She walks right from tick 105, uncovering it
        // with some 30 frames of its 100-frame fuse left.
        SamusBomb => 125,
        // B at tick 45; `SpecialN` throws the Boomerang on frame 26, and it
        // has flown some 14 frames here.
        Link => 85,
        // Up+B at tick 45 enters `SpecialHi` with the swirl; 13 swirl
        // frames later its primitive alpha is 204 (RE-324).
        LinkSpin => 58,
        // A at tick 50 enters `Jab1` (tick 8's A jabs first); its trail
        // samples from frame 5 and draws from the second sample: three
        // samples here.
        LinkSword => 56,
        // Forward and A at tick 50 enter `AttackS4`; a spark comes every 3
        // frames from frame 21 and lasts 7 plays: two are out here.
        PikachuSmash => 76,
        // Up+B at tick 45 with the stick up and left; `SpecialHiHold`
        // launches `SpecialAirHi` near tick 87, 13 of its 30 travel frames
        // ago here.
        FoxFire => 100,
        // Up+B at tick 45 with the stick up and left; the zip
        // (`SpecialAirHi`) runs ticks 65 to 69 and is 3 frames in here.
        PikachuQuick => 68,
        // Up+B on the right platform at 140; PK Thunder loops round under
        // the platform and back up into Ness: PK Thunder 2 off a
        // pass-through floor is `SpecialAirHiJibaku`, up and left.
        NessJibaku => 227,
        // Down+B at 120 on the slope; Stone slides down it from tick ~125,
        // TopN rolled 19.7 degrees, until the bowl's floor near 160.
        KirbyStoneSlope => STONE_SLOPE_B_TICK + 20,
        // Down+B at 60 facing left; the kick runs up the roof's left slope
        // and leaves its end near tick 89: `SpecialLwAir` with the slope's
        // 14.7-degree roll until near 120.
        CaptainKickRoll => KICK_ROLL_B_TICK + 40,
        // Up+B at tick 45; Egg Throw makes the egg on frame 4 and throws it
        // on frame 23, and it has flown some 16 frames here.
        Yoshi => 85,
        // Down+B at tick 45; the Yoshi Bomb hops on its TransN, drops and
        // lands near tick 84, and `SpecialLwLanding` frame 3 makes the two
        // stars. They are some 6 of their 16 frames old here (RE-378).
        YoshiBomb => 93,
        // B at tick 45; `SpecialN` makes the flame on frame 42 and stops it
        // on frame 55. It has played some 7 frames here.
        Captain => 94,
        // Down+B at tick 45; `SpecialLw` makes the flame on frame 12 and
        // stops it on frame 32. It has played some 12 frames here.
        CaptainKick => 69,
        // Up+B at tick 45; Final Cutter rises on its TransN, lands on the
        // top platform near tick 102, and `SpecialHiLanding` frame 3 makes
        // the wave. It is some 6 of its 20 frames old here.
        Kirby => 108,
        // Up+B at 95 on the bridge's end: `SpecialHi` frame 52 makes the
        // Trail, which falls with Kirby until frame 58. `SpecialHi` frame
        // 54 here, the Trail's third play: the N64 trace's screenshot.
        KirbyCutter => CUTTER_B_TICK + 54,
        // A from tick 50 on every other tick: jabs 1-3, then the rapid jab,
        // whose loop makes a puff at its frames 0, 6, 11, 16 and 21 (RE-513).
        KirbyJab => 110,
        // B at tick 45; `SpecialN` makes the aerial jolt near tick 69, which
        // lands on its first frame as the ground jolt. Its animation is 8
        // plays into its first 15-frame push cycle here.
        Pikachu => 77,
        // C-Up at tick 45 jumps; B at tick 56 enters `SpecialAirN`, which
        // makes the aerial jolt near tick 77. It is some 11 frames into its
        // flight here, still in the air.
        PikachuAir => 88,
        // Up+B at tick 45; Sing makes its notes on its first update.
        Purin => 75,
        // B at tick 40 starts the Giant Punch; it is charging here.
        Donkey => 60,
        // B at tick 45; PK Fire's spark appears near tick 70 and is in
        // flight here.
        Ness => 75,
        // Up+B at tick 45; PK Thunder's head rises with its four trails
        // while Ness holds `SpecialHiHold`.
        NessThunder => 85,
        // Down+B at tick 45, held; PSI Magnet's field is up in
        // `SpecialLwHold`.
        NessMagnet => 75,
        // Down+B at tick 45 pulls a Bomb; it is in Link's hand here.
        LinkBomb => 80,
        // Z+A at tick 125; the catch box is live on `Catch` frame 6, the
        // two-frame pull follows, and the dummy then hangs in `CaptureWait`.
        Grab => 118 + ROUTE_DELAY,
        // A at tick 125 lands on tick 126 (host `romtool jumptest`); the
        // dummy's hitlag is over and it is in `DamageN1`.
        Jab => 118 + ROUTE_DELAY,
        // Z begins at tick 40; the diagonal stick is applied after GuardOn
        // has entered Guard, then frozen with the shield still raised.
        Shield => 60,
        // The press at 30 jumps; nine ticks later the rising Mario is
        // still over the main floor.
        Shadows => 39,
        // Down+B at tick 90, once the dash and its brake are over; the same 42
        // ticks after the press as the original capture at 92.
        PikachuThunder => 132,
        KirbyHat => 260 + ROUTE_DELAY,
        // A at 170 spits the dummy; its star flies with the captured-star
        // effect and its sparks.
        KirbySpit => 200 + ROUTE_DELAY,
        // L at 250, once the copy is over, drops it; the lost star is
        // rising on its arc.
        KirbyLose => 262 + ROUTE_DELAY,
        // The dummy's wiggle lands on the tick after the tap (the `re552`
        // capture log); the frame after that draws Kirby's restarted clip
        // on frame 0.
        KirbyWiggle => KIRBY_WIGGLE_TICK + 2,
        // The egg laid at 293 lands on the top platform 39 ticks later and
        // stops dead there (`ftCommonYoshiEggProcMap` only sets `ga`); the
        // Training CPU wiggles it open at 458 (`ftComputer`'s `YoshiEgg`
        // wiggle), so the capture is the egg at rest some 48 ticks in.
        YoshiEgg => 380,
        // Z from tick 40: the egg is up from the end of `GuardOn`, and 16
        // ticks a point of the shield's 55 decays, darkening it.
        YoshiShield => 600,
        YoshiRollF | YoshiRollB => 69,
        YoshiShieldBreak => 84,
        _ => return None,
    })
}

/// The buttons of every scene without a script of its own: the menu
/// confirm, the C-Up jumps, and a jab at 98 once the landing lag has
/// cleared.
pub fn default_buttons(tick: u64) -> u16 {
    match tick {
        4 | 8 => button::A,
        13 | 33 => button::C_UP,
        98 => button::A,
        _ => 0,
    }
}

/// The stick's x of every scene without a script of its own: held left
/// (toward the dummy) for the carry phase of the scripted jump.
pub fn default_stick_x(tick: u64) -> i8 {
    if (34..90).contains(&tick) {
        -30
    } else {
        0
    }
}

fn buttons(scene: GameScene, tick: u64) -> u16 {
    use GameScene::*;
    match scene {
        Samus | SamusShot | SamusBomb | Link | LinkSpin | LinkSword | Yoshi | YoshiBomb
        | Captain | CaptainKick | Kirby | KirbyJab | Pikachu | PikachuSmash | Purin | Donkey
        | Ness | NessThunder | NessMagnet | LinkBomb | FoxFire | PikachuQuick => match tick {
            4 | 8 => button::A,
            // `linksword` jabs; `pikachusmash` taps forward with A. The A at
            // tick 8 jabs on Training's first frame; both wait it out.
            50 if matches!(scene, LinkSword | PikachuSmash) => button::A,
            // `kirbyjab` mashes A (no B at tick 20).
            t if scene == KirbyJab => {
                if t >= 50 && t % 2 == 0 {
                    button::A
                } else {
                    0
                }
            }
            // Samus's and Donkey Kong's jabs from the A at tick 8 are still
            // running at tick 20 and swallow a B there; their Charge Shot and
            // Giant Punch start at 40.
            40 if matches!(scene, Samus | SamusShot | Donkey) => button::B,
            80 if scene == SamusShot => button::B,
            SPECIAL_TICK
                if !matches!(scene, Samus | SamusShot | Donkey | LinkSword | PikachuSmash) =>
            {
                button::B
            }
            // Extended controls hold through the critical bloat, then throw
            // backward. The original Link Bomb capture stays at tick 55.
            325 if scene == LinkBomb => button::A,
            // PSI Magnet lasts while B is held.
            t if scene == NessMagnet && t > SPECIAL_TICK => button::B,
            _ => 0,
        },
        // `kirbyhat` and `yoshiegg` take the grab scene's route onto the
        // dummy's platform, then B: Kirby inhales and, with a second B from
        // `SpecialNWait`, copies (`ftKirbySpecialNCopyCheckGotoCopy`); Yoshi's
        // tongue swallows and lays the dummy.
        KirbyHat | KirbySpit | KirbyLose | KirbyWiggle | YoshiEgg => match tick {
            4 | 8 => button::A,
            t if scene != YoshiEgg && (t == 13 + ROUTE_DELAY || t == 30 + ROUTE_DELAY) => {
                button::C_UP
            }
            t if scene == KirbySpit && t == 170 + ROUTE_DELAY => button::A,
            t if scene == KirbyWiggle && t == 108 + ROUTE_DELAY => button::B,
            t if !matches!(scene, YoshiEgg | KirbyWiggle)
                && (t == 108 + ROUTE_DELAY || t == 170 + ROUTE_DELAY) =>
            {
                button::B
            }
            t if scene == KirbyLose && t == 250 + ROUTE_DELAY => button::L,
            // Yoshi's jab (to tick 31) shortens his walk, which ends at 123
            // under the left platform (RE-516). He jumps twice straight up
            // and lands on that platform at tick 238, 384 units right of the
            // dummy: inside the 340-785 units joint 31, the tongue tip,
            // reaches in the catch window (RE-432). With the N64's first
            // frame played by the setter (RE-473) the tongue at 250 catches
            // at 267 and lays the egg at 293.
            t if scene == YoshiEgg && (t == 130 || t == 148) => button::C_UP,
            t if scene == YoshiEgg && t == 250 => button::B,
            _ => 0,
        },
        // `pikachuthunder`: Pikachu dashes right, out from under the top
        // platform that would stop the head, then Down+B.
        PikachuThunder => match tick {
            4 | 8 => button::A,
            90 => button::B,
            _ => 0,
        },
        // `nessjibaku`: run right under the right platform, jump onto it,
        // then Up+B there.
        NessJibaku => match tick {
            4 | 8 => button::A,
            JIBAKU_JUMP_TICK => button::C_UP,
            JIBAKU_B_TICK => button::B,
            _ => 0,
        },
        // `kirbystoneslope`, `captainkickroll`: Down+B once standing.
        KirbyStoneSlope | CaptainKickRoll | KirbyCutter => match tick {
            4 | 8 => button::A,
            STONE_SLOPE_B_TICK if scene == KirbyStoneSlope => button::B,
            KICK_ROLL_B_TICK if scene == CaptainKickRoll => button::B,
            CUTTER_B_TICK if scene == KirbyCutter => button::B,
            _ => 0,
        },
        PikachuAir => match tick {
            4 | 8 => button::A,
            45 => button::C_UP,
            56 => button::B,
            _ => 0,
        },
        Fox | Luigi if tick == AIR_SPECIAL_TICK => button::B,
        Fireball | Superjump if tick == 150 => button::B,
        Fox | Luigi | Fireball | Superjump => default_buttons(tick),
        // The grab scene has its own route onto the dummy's platform, found
        // with `romtool jumptest --jump-tick 5 --jump2-tick 22 --stick-x -30
        // --stick-switch-tick 6 --stick-release-tick 44` (local ticks): Mario
        // lands at x -1253, 144 units right of the dummy, still facing left
        // from his spawn at x = 0 (RE-387); tick 108 is Z held with an A edge
        // (`ftCommonCatchCheckInterruptCommon`).
        Grab => match tick {
            4 | 8 => button::A,
            t if t == 13 + ROUTE_DELAY || t == 30 + ROUTE_DELAY => button::C_UP,
            125 => button::Z | button::A,
            _ => 0,
        },
        // The jab scene takes the grab's route and taps A where the grab
        // pressed Z+A (RE-351).
        Jab => match tick {
            4 | 8 | 125 => button::A,
            t if t == 13 + ROUTE_DELAY || t == 30 + ROUTE_DELAY => button::C_UP,
            _ => 0,
        },
        // Mario jumps at 30, once the opening jab is over, and is still over
        // the main floor nine ticks later.
        Shadows => match tick {
            4 | 8 => button::A,
            30 => button::C_UP,
            _ => 0,
        },
        Shield => match tick {
            4 | 8 => button::A,
            40..=60 => button::Z,
            _ => 0,
        },
        YoshiShield | YoshiRollF | YoshiRollB | YoshiShieldBreak => match tick {
            4 | 8 => button::A,
            t if t >= 40 && (scene != YoshiShieldBreak || t < 80) => button::Z,
            _ => 0,
        },
        _ => 0,
    }
}

fn stick_x(scene: GameScene, tick: u64) -> i8 {
    use GameScene::*;
    match scene {
        // Up and to the left from the B on: the special-high check, then
        // the launch direction (RE-503).
        FoxFire | PikachuQuick => {
            if tick >= SPECIAL_TICK {
                DIAGONAL_X
            } else {
                0
            }
        }
        // The run right, then the steering: `wpNessPKThunderHeadProcUpdate`
        // turns the head 6 degrees a frame toward the stick, down-right
        // (clockwise from straight up) round under the platform, then
        // left, back up into Ness from below.
        NessJibaku => {
            if (JIBAKU_RUN..JIBAKU_RUN_END).contains(&tick) {
                80
            } else if (JIBAKU_STEER..JIBAKU_STEER_LEFT).contains(&tick) {
                57
            } else if tick >= JIBAKU_STEER_LEFT {
                -80
            } else {
                0
            }
        }
        // Kirby spawns facing right on the flat left of the bowl; the dash
        // stops him on its left slope.
        KirbyStoneSlope => {
            if (30..42).contains(&tick) {
                80
            } else {
                0
            }
        }
        // Captain Falcon spawns facing right on the roof; a two-tick tap
        // turns him left.
        CaptainKickRoll => {
            if (30..32).contains(&tick) {
                -80
            } else {
                0
            }
        }
        KirbyCutter => {
            if CUTTER_RUN.contains(&tick) {
                -127
            } else {
                0
            }
        }
        LinkBomb => {
            if tick == 325 {
                80
            } else {
                0
            }
        }
        Shield => {
            if (50..=60).contains(&tick) {
                40
            } else {
                0
            }
        }
        Grab | Jab | KirbyHat | KirbySpit | KirbyLose | KirbyWiggle => {
            if (14 + ROUTE_DELAY..52 + ROUTE_DELAY).contains(&tick) {
                -30
            } else if scene == KirbyWiggle && tick == KIRBY_WIGGLE_TICK {
                // Kirby faces left: a push its way, no `SpecialNTurn`.
                -80
            } else {
                0
            }
        }
        YoshiEgg => {
            if (14..123).contains(&tick) {
                -30
            } else {
                0
            }
        }
        YoshiRollF | YoshiRollB => {
            if (60..62).contains(&tick) {
                // Dream Land spawn 0 is at x=0 and faces left.
                if scene == YoshiRollF {
                    -80
                } else {
                    80
                }
            } else {
                0
            }
        }
        Samus | SamusShot | Link | LinkSpin | LinkSword | Yoshi | YoshiBomb | Captain
        | CaptainKick | Kirby | KirbyJab | Pikachu | PikachuAir | Purin | Donkey | Ness
        | NessThunder | NessMagnet | YoshiShield | YoshiShieldBreak | Shadows => 0,
        SamusBomb => {
            if (105..125).contains(&tick) {
                80
            } else {
                0
            }
        }
        // A forward tap with the A edge: `ftCommonAttackS4CheckInterCommon`.
        // Pikachu spawns facing left.
        PikachuSmash => {
            if tick == 50 {
                -80
            } else {
                0
            }
        }
        PikachuThunder => {
            if (42..=66).contains(&tick) {
                80
            } else {
                0
            }
        }
        _ => default_stick_x(tick),
    }
}

fn stick_y(scene: GameScene, tick: u64) -> i8 {
    use GameScene::*;
    if scene == Shield && (50..=60).contains(&tick) {
        40
    } else if matches!(scene, FoxFire | PikachuQuick) && tick >= SPECIAL_TICK {
        DIAGONAL_Y
    } else if scene == NessJibaku && tick == JIBAKU_B_TICK {
        80
    } else if scene == NessJibaku && (JIBAKU_STEER..JIBAKU_STEER_LEFT).contains(&tick) {
        -57
    } else if (scene == Superjump && tick == 150)
        || (matches!(scene, LinkSpin | Yoshi | Kirby | Purin | NessThunder) && tick == SPECIAL_TICK)
        || (scene == KirbyCutter && tick == CUTTER_B_TICK)
    {
        80
    } else if (matches!(scene, SamusBomb | YoshiBomb | CaptainKick | LinkBomb)
        && tick == SPECIAL_TICK)
        || (scene == PikachuThunder && tick == 90)
        || (scene == KirbyStoneSlope && tick == STONE_SLOPE_B_TICK)
        || (scene == CaptainKickRoll && tick == KICK_ROLL_B_TICK)
        || (scene == NessMagnet && tick >= SPECIAL_TICK)
    {
        // The special-low check's downward stick with the B edge.
        -80
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_scripted_scene_has_a_capture_tick() {
        for scene in GameScene::ALL {
            assert_eq!(
                is_fighter_scene(scene),
                capture_tick(scene).is_some(),
                "{scene:?}"
            );
        }
    }
}
