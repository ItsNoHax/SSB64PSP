//! Yoshi motion attacks from `relocData/246_YoshiMainMotion.c` (US).
//! Windows are half open and follow the script's Wait/WaitAsync cursor.
//! Lengths are the figatree lengths from `ssb_rom::anim::EXPECTED_FRAMES`.
//! A `MakeAttackColl` that replaces a live slot without a clear keeps the
//! slot's hit record, so both phases share one hit generation.

use crate::attack::{ActiveHitbox, Hitbox, MoveData};
use ssb_engine::math::Vec3;

#[allow(clippy::too_many_arguments)]
const fn hit(
    damage: i32,
    size: f32,
    x: f32,
    y: f32,
    z: f32,
    angle: i32,
    growth: i32,
    weight: i32,
    base: i32,
) -> Hitbox {
    Hitbox {
        damage,
        radius: size / 2.0,
        offset: Vec3::new(x, y, z),
        angle,
        kb_scale: growth,
        kb_weight: weight,
        kb_base: base,
    }
}

macro_rules! h {
    ($d:expr,$sz:expr,$x:expr,$y:expr,$z:expr,$a:expr,$g:expr,$w:expr,$b:expr,$s:expr,$e:expr) => {
        ActiveHitbox::new(hit($d, $sz, $x, $y, $z, $a, $g, $w, $b), $s, $e)
    };
}
macro_rules! mv {
    ($name:ident,$len:expr,$lag:expr,[$($hb:expr),* $(,)?]) => {
        pub static $name: MoveData = MoveData { hitboxes: &[$($hb),*], length_frames: $len, landing_lag_percent: $lag };
    };
}

/// `Jab1` sets flag 1 at frame 10 of its 24 frames, so
/// `ftCommonAttack11ProcUpdate` chains on the flag.
pub const JAB1_FLAG1_FRAME: f32 = 10.0;

mv!(
    JAB1,
    24.0,
    None,
    [
        h!(3, 280.0, 0.0, 0.0, 0.0, 361, 50, 0, 8, 3.0, 6.0),
        h!(3, 280.0, 0.0, 0.0, 0.0, 361, 50, 0, 8, 3.0, 6.0),
    ]
);
mv!(
    JAB2,
    21.0,
    None,
    [
        h!(5, 300.0, 0.0, 0.0, 0.0, 361, 120, 0, 8, 1.0, 4.0),
        h!(5, 300.0, 0.0, 0.0, 0.0, 361, 120, 0, 8, 1.0, 4.0),
    ]
);
// `SetAttackCollDamage(0/1, 8)` at frame 12 keeps the boxes and the record.
mv!(
    DASH,
    36.0,
    None,
    [
        h!(12, 280.0, 0.0, 180.0, 280.0, 361, 100, 0, 10, 8.0, 12.0),
        h!(12, 200.0, 0.0, 180.0, 30.0, 361, 100, 0, 10, 8.0, 12.0),
        h!(8, 280.0, 0.0, 180.0, 280.0, 361, 100, 0, 10, 12.0, 22.0),
        h!(8, 200.0, 0.0, 180.0, 30.0, 361, 100, 0, 10, 12.0, 22.0),
    ]
);
// `FTiltHigh`, `FTilt` and `FTiltLow` carry the same boxes; only the
// figatree differs.
mv!(
    FTILT,
    30.0,
    None,
    [
        h!(13, 300.0, 0.0, 0.0, 0.0, 70, 100, 0, 8, 8.0, 11.0),
        h!(13, 300.0, 90.0, 0.0, 0.0, 70, 100, 0, 8, 8.0, 11.0),
    ]
);
// US boxes (`#else` branch). The knockback is weight-based.
mv!(
    UTILT,
    26.0,
    None,
    [
        h!(12, 390.0, 0.0, 40.0, 40.0, 100, 100, 130, 0, 7.0, 11.0),
        h!(12, 240.0, 0.0, 0.0, 0.0, 100, 100, 130, 0, 7.0, 11.0),
    ]
);
mv!(
    DTILT,
    24.0,
    None,
    [
        h!(10, 260.0, 0.0, -20.0, 0.0, 30, 100, 110, 0, 8.0, 11.0),
        h!(10, 210.0, 0.0, 0.0, 0.0, 30, 100, 110, 0, 8.0, 11.0),
    ]
);
// The three forward smashes jump into one script (`0x1164`).
mv!(
    FSMASH,
    51.0,
    None,
    [
        h!(18, 380.0, 0.0, 40.0, 40.0, 361, 100, 0, 20, 18.0, 25.0),
        h!(18, 200.0, 0.0, 0.0, 0.0, 361, 100, 0, 20, 18.0, 25.0),
    ]
);
// `SetAttackCollSize` at frame 10, then new boxes at frame 12 without a
// clear: one record throughout.
mv!(
    USMASH,
    48.0,
    None,
    [
        h!(18, 320.0, 0.0, 40.0, 40.0, 361, 118, 0, 20, 9.0, 10.0),
        h!(18, 210.0, 0.0, 0.0, 0.0, 361, 118, 0, 20, 9.0, 10.0),
        h!(18, 370.0, 0.0, 40.0, 40.0, 361, 118, 0, 20, 10.0, 12.0),
        h!(18, 240.0, 0.0, 0.0, 0.0, 361, 118, 0, 20, 10.0, 12.0),
        h!(18, 320.0, 0.0, 40.0, 40.0, 80, 110, 0, 20, 12.0, 16.0),
        h!(18, 210.0, 0.0, 0.0, 0.0, 80, 110, 0, 20, 12.0, 16.0),
    ]
);
// US boxes (14 damage, angle 30).
mv!(
    DSMASH,
    60.0,
    None,
    [
        h!(14, 300.0, 0.0, 0.0, 0.0, 30, 105, 0, 20, 6.0, 8.0),
        h!(14, 260.0, 0.0, 0.0, 0.0, 30, 105, 0, 20, 6.0, 8.0),
        h!(14, 300.0, 0.0, 0.0, 0.0, 30, 105, 0, 20, 21.0, 23.0).with_hit_generation(1),
        h!(14, 260.0, 0.0, 0.0, 0.0, 30, 105, 0, 20, 21.0, 23.0).with_hit_generation(1),
    ]
);

mv!(
    AIR_N,
    50.0,
    Some(50),
    [
        h!(14, 240.0, 10.0, 0.0, 0.0, 361, 100, 0, 15, 5.0, 9.0),
        h!(14, 240.0, 10.0, 0.0, 0.0, 361, 100, 0, 15, 5.0, 9.0),
        h!(14, 260.0, 0.0, 0.0, 0.0, 361, 100, 0, 15, 5.0, 9.0),
        h!(11, 240.0, 10.0, 0.0, 0.0, 361, 100, 0, 0, 9.0, 39.0),
        h!(11, 240.0, 10.0, 0.0, 0.0, 361, 100, 0, 0, 9.0, 39.0),
        h!(11, 260.0, 0.0, 0.0, 0.0, 361, 100, 0, 0, 9.0, 39.0),
    ]
);
// Yoshi has a `LandingAirF` motion, so this flag 1 is never read. The
// negative angles send the target downward.
mv!(
    AIR_F,
    40.0,
    None,
    [
        h!(18, 300.0, 0.0, 40.0, 40.0, -85, 100, 0, 0, 11.0, 13.0),
        h!(18, 200.0, 0.0, 0.0, 0.0, -85, 100, 0, 0, 11.0, 13.0),
        h!(18, 300.0, 0.0, 40.0, 40.0, -100, 100, 0, 0, 13.0, 16.0),
        h!(18, 200.0, 0.0, 0.0, 0.0, -100, 100, 0, 0, 13.0, 16.0),
    ]
);
// Yoshi has a `LandingAirB` motion, so this flag 1 is never read.
mv!(
    AIR_B,
    42.0,
    None,
    [
        h!(16, 270.0, -30.0, 45.0, 0.0, 361, 100, 0, 10, 10.0, 14.0),
        h!(16, 340.0, 110.0, 40.0, 0.0, 361, 100, 0, 10, 10.0, 14.0),
        h!(10, 240.0, -30.0, 45.0, 0.0, 361, 100, 0, 0, 14.0, 20.0),
        h!(10, 300.0, 80.0, 30.0, 0.0, 361, 100, 0, 0, 14.0, 20.0),
    ]
);
mv!(
    AIR_HI,
    42.0,
    Some(50),
    [
        h!(15, 340.0, 0.0, 0.0, 0.0, 90, 100, 0, 20, 9.0, 11.0),
        h!(15, 340.0, 0.0, 120.0, 0.0, 90, 100, 0, 20, 9.0, 11.0),
    ]
);

/// `AttackAirD`: boxes at frame 4, then fourteen `Wait(1)`, clear, `Wait(1)`,
/// `RefreshAttackCollID` loops. Each refresh is a fresh record; the last one
/// is cleared on the frame it is made (frame 32).
const fn air_lw_boxes() -> [ActiveHitbox; 28] {
    let empty = ActiveHitbox::new(hit(0, 0.0, 0.0, 0.0, 0.0, 0, 0, 0, 0), 0.0, 0.0);
    let mut out = [empty; 28];
    let mut pulse = 0;
    while pulse < 14 {
        let start = 4.0 + pulse as f32 * 2.0;
        let b = hit(4, 300.0, 0.0, 0.0, 0.0, -90, 90, 0, 5);
        out[pulse * 2] = ActiveHitbox::new(b, start, start + 1.0).with_hit_generation(pulse as u8);
        out[pulse * 2 + 1] =
            ActiveHitbox::new(b, start, start + 1.0).with_hit_generation(pulse as u8);
        pulse += 1;
    }
    out
}
const AIR_LW_BOXES: [ActiveHitbox; 28] = air_lw_boxes();
pub static AIR_LW: MoveData = MoveData {
    hitboxes: &AIR_LW_BOXES,
    length_frames: 50.0,
    landing_lag_percent: Some(50),
};

/// Yoshi Bomb's descent box: `GroundPound` makes it at frame 30 and
/// `GroundPoundAir` at frame 24, each followed by `Wait(100)`. The start
/// figatrees end at 30 and 28, and `ftYoshiSpecialAirLwLoopSetStatus`
/// keeps the box (`FTSTATUS_PRESERVE_HIT`) while its zero animation speed
/// holds the script, so the box lasts until the landing status clears it.
const fn bomb_box(start: f32) -> ActiveHitbox {
    ActiveHitbox::new(
        hit(18, 460.0, 0.0, 160.0, 0.0, 60, 100, 0, 50),
        start,
        f32::INFINITY,
    )
}
pub static BOMB_GROUND_START: MoveData = MoveData {
    hitboxes: &[bomb_box(30.0)],
    length_frames: 30.0,
    landing_lag_percent: None,
};
pub static BOMB_AIR_START: MoveData = MoveData {
    hitboxes: &[bomb_box(24.0)],
    length_frames: 28.0,
    landing_lag_percent: None,
};
/// The loop holds the start's frame, which is past either start's window
/// opening.
pub static BOMB_LOOP: MoveData = MoveData {
    hitboxes: &[bomb_box(0.0)],
    length_frames: 0.0,
    landing_lag_percent: None,
};
