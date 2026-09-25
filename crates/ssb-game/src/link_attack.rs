//! Link motion attacks from `relocData/224_LinkMainMotion.c` (US).
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

/// `Jab1` sets flag 1 at frame 10 and `Jab2` at frame 14, well before their
/// figatrees end. `ftCommonAttack11ProcUpdate` and `...12ProcUpdate` chain
/// on that flag, not on the animation end.
pub const JAB1_FLAG1_FRAME: f32 = 10.0;
pub const JAB2_FLAG1_FRAME: f32 = 14.0;

mv!(
    JAB1,
    24.0,
    None,
    [
        h!(5, 220.0, 0.0, 0.0, 250.0, 361, 50, 0, 8, 6.0, 10.0),
        h!(5, 160.0, 0.0, 0.0, 0.0, 361, 50, 0, 8, 6.0, 10.0),
    ]
);
mv!(
    JAB2,
    22.0,
    None,
    [
        h!(3, 220.0, 0.0, 0.0, 250.0, 361, 50, 0, 8, 6.0, 8.0),
        h!(3, 160.0, 0.0, 0.0, 0.0, 361, 50, 0, 8, 6.0, 8.0),
    ]
);
mv!(
    JAB3,
    40.0,
    None,
    [
        h!(4, 220.0, 0.0, 0.0, 250.0, 361, 100, 0, 6, 6.0, 12.0),
        h!(4, 160.0, 0.0, 0.0, 0.0, 361, 100, 0, 6, 6.0, 12.0),
    ]
);

/// `JabLoop` figatree cycle. The script pauses after its fifth pulse and
/// resumes when the animation wraps.
pub const RAPID_LOOP_LENGTH: f32 = 35.0;
/// The pulse ends, where the script sets flag 1. The fifth flag 1 fires when
/// the paused script resumes at the wrap.
pub const RAPID_FLAG1_FRAMES: [f32; 5] = [5.0, 12.0, 19.0, 26.0, RAPID_LOOP_LENGTH];

/// Five one-damage pulses, each cleared before the next.
const fn rapid_boxes() -> [ActiveHitbox; 10] {
    let empty = ActiveHitbox::new(hit(0, 0.0, 0.0, 0.0, 0.0, 0, 0, 0, 0), 0.0, 0.0);
    let mut out = [empty; 10];
    let mut pulse = 0;
    while pulse < 5 {
        let start = 3.0 + pulse as f32 * 7.0;
        out[pulse * 2] = ActiveHitbox::new(
            hit(1, 220.0, 0.0, 0.0, 270.0, 361, 50, 0, 8),
            start,
            start + 2.0,
        )
        .with_hit_generation(pulse as u8);
        out[pulse * 2 + 1] = ActiveHitbox::new(
            hit(1, 160.0, 0.0, 0.0, 0.0, 361, 50, 0, 8),
            start,
            start + 2.0,
        )
        .with_hit_generation(pulse as u8);
        pulse += 1;
    }
    out
}
const RAPID_BOXES: [ActiveHitbox; 10] = rapid_boxes();
pub static RAPID_LOOP: MoveData = MoveData {
    hitboxes: &RAPID_BOXES,
    length_frames: RAPID_LOOP_LENGTH,
    landing_lag_percent: None,
};

mv!(
    DASH,
    46.0,
    None,
    [
        h!(14, 220.0, 0.0, 0.0, 190.0, 361, 100, 0, 0, 8.0, 12.0),
        h!(16, 180.0, 0.0, 0.0, 0.0, 361, 100, 0, 10, 8.0, 12.0),
        h!(10, 220.0, 0.0, 0.0, 190.0, 361, 100, 0, 0, 12.0, 34.0),
        h!(11, 180.0, 0.0, 0.0, 0.0, 361, 100, 0, 5, 12.0, 34.0),
    ]
);
mv!(
    FTILT,
    40.0,
    None,
    [
        h!(18, 260.0, 0.0, 0.0, 280.0, 361, 100, 0, 0, 15.0, 21.0),
        h!(17, 180.0, 0.0, 0.0, 0.0, 361, 100, 0, 0, 15.0, 21.0),
    ]
);
mv!(
    UTILT,
    30.0,
    None,
    [
        h!(10, 200.0, 0.0, 0.0, 240.0, 79, 100, 0, 20, 8.0, 17.0),
        h!(10, 160.0, 0.0, 0.0, 0.0, 79, 100, 0, 20, 8.0, 17.0),
    ]
);
mv!(
    DTILT,
    40.0,
    None,
    [
        h!(12, 240.0, 0.0, 0.0, 250.0, 80, 85, 0, 30, 12.0, 20.0),
        h!(12, 180.0, 0.0, 0.0, 0.0, 80, 85, 0, 30, 12.0, 20.0),
    ]
);
mv!(
    FSMASH,
    58.0,
    None,
    [
        h!(20, 280.0, 0.0, 0.0, 270.0, 50, 100, 0, 30, 16.0, 20.0),
        h!(20, 280.0, 0.0, 0.0, 130.0, 50, 100, 0, 30, 16.0, 20.0),
        h!(20, 200.0, 0.0, 0.0, 0.0, 50, 100, 0, 30, 16.0, 20.0),
        h!(12, 240.0, 0.0, 0.0, 270.0, 361, 100, 0, 20, 20.0, 26.0),
        h!(12, 240.0, 0.0, 0.0, 130.0, 361, 100, 0, 20, 20.0, 26.0),
        h!(12, 180.0, 0.0, 0.0, 0.0, 361, 100, 0, 20, 20.0, 26.0),
    ]
);
// Three swings. The second phase of the first swing replaces its boxes
// without a clear; each later swing follows a clear.
mv!(
    USMASH,
    55.0,
    None,
    [
        h!(7, 240.0, 0.0, 0.0, 180.0, 120, 100, 80, 0, 11.0, 13.0),
        h!(7, 240.0, 0.0, 0.0, 0.0, 100, 100, 80, 0, 11.0, 13.0),
        h!(7, 220.0, 0.0, 0.0, 180.0, 80, 100, 30, 0, 13.0, 17.0),
        h!(7, 160.0, 0.0, 0.0, 0.0, 80, 100, 30, 0, 13.0, 17.0),
        h!(3, 380.0, 0.0, 0.0, 180.0, 80, 100, 25, 0, 21.0, 26.0).with_hit_generation(1),
        h!(3, 200.0, 0.0, 0.0, 0.0, 80, 100, 25, 0, 21.0, 26.0).with_hit_generation(1),
        h!(12, 380.0, 0.0, 0.0, 180.0, 90, 90, 0, 30, 31.0, 36.0).with_hit_generation(2),
        h!(12, 200.0, 0.0, 0.0, 0.0, 90, 90, 0, 30, 31.0, 36.0).with_hit_generation(2),
    ]
);
mv!(
    DSMASH,
    50.0,
    None,
    [
        h!(16, 220.0, 0.0, 0.0, 260.0, 40, 100, 0, 35, 9.0, 14.0),
        h!(16, 150.0, 0.0, 0.0, 0.0, 75, 100, 0, 35, 9.0, 14.0),
        h!(16, 220.0, 0.0, 0.0, 260.0, 40, 100, 0, 35, 21.0, 24.0).with_hit_generation(1),
        h!(16, 150.0, 0.0, 0.0, 0.0, 75, 100, 0, 35, 21.0, 24.0).with_hit_generation(1),
    ]
);

mv!(
    AIR_N,
    40.0,
    Some(50),
    [
        h!(10, 240.0, 10.0, 0.0, 0.0, 361, 100, 0, 15, 4.0, 6.0),
        h!(10, 240.0, 60.0, 0.0, 0.0, 361, 100, 0, 15, 4.0, 6.0),
        h!(10, 260.0, 0.0, 0.0, 0.0, 361, 100, 0, 15, 4.0, 6.0),
        h!(8, 240.0, 10.0, 0.0, 0.0, 361, 100, 0, 10, 6.0, 30.0),
        h!(8, 240.0, 60.0, 0.0, 0.0, 361, 100, 0, 10, 6.0, 30.0),
        h!(8, 260.0, 0.0, 0.0, 0.0, 361, 100, 0, 10, 6.0, 30.0),
    ]
);
// Link has a `LandingAirF` motion, so this flag 1 is never read.
mv!(
    AIR_F,
    56.0,
    None,
    [
        h!(20, 280.0, 0.0, 0.0, 250.0, 361, 100, 0, 5, 15.0, 18.0),
        h!(20, 220.0, 0.0, 0.0, 0.0, 361, 100, 0, 5, 15.0, 18.0),
        h!(12, 180.0, 0.0, 0.0, 250.0, 361, 100, 0, 0, 18.0, 35.0),
        h!(12, 150.0, 0.0, 0.0, 0.0, 361, 100, 0, 0, 18.0, 35.0),
    ]
);
mv!(
    AIR_B,
    40.0,
    Some(50),
    [
        h!(10, 280.0, 10.0, 0.0, 0.0, 361, 100, 0, 0, 6.0, 14.0),
        h!(10, 280.0, 90.0, 0.0, 0.0, 361, 100, 0, 0, 6.0, 14.0),
        h!(10, 310.0, 0.0, 0.0, 0.0, 361, 100, 0, 0, 6.0, 14.0),
        h!(10, 280.0, 10.0, 0.0, 0.0, 361, 100, 0, 15, 18.0, 27.0).with_hit_generation(1),
        h!(10, 280.0, 90.0, 0.0, 0.0, 361, 100, 0, 15, 18.0, 27.0).with_hit_generation(1),
        h!(10, 310.0, 0.0, 0.0, 0.0, 361, 100, 0, 15, 18.0, 27.0).with_hit_generation(1),
    ]
);
mv!(
    AIR_HI,
    70.0,
    Some(20),
    [
        h!(16, 300.0, 0.0, 0.0, 100.0, 70, 100, 0, 5, 5.0, 49.0),
        h!(16, 200.0, 0.0, 0.0, 0.0, 70, 100, 0, 5, 5.0, 49.0),
    ]
);
// Link has a `LandingAirLw` motion (`LandingAirD`), so flag 1 is not read.
mv!(
    AIR_LW,
    90.0,
    None,
    [
        h!(16, 300.0, 0.0, 0.0, 100.0, 361, 100, 0, 25, 5.0, 65.0),
        h!(16, 200.0, 0.0, 0.0, 0.0, 361, 100, 0, 25, 5.0, 65.0),
    ]
);

/// `UpSpecial`, which both Spin Attack statuses run. The frame-12 boxes
/// replace the opening pair without a clear.
const SPIN_BOXES: [ActiveHitbox; 4] = [
    h!(16, 300.0, 0.0, 0.0, 300.0, 361, 100, 0, 30, 8.0, 12.0),
    h!(16, 200.0, 0.0, 0.0, 0.0, 361, 100, 0, 30, 8.0, 12.0),
    h!(8, 180.0, 0.0, 0.0, 300.0, 361, 100, 0, 10, 12.0, 40.0),
    h!(8, 140.0, 0.0, 0.0, 0.0, 361, 100, 0, 10, 12.0, 40.0),
];
pub static SPIN_GROUND: MoveData = MoveData {
    hitboxes: &SPIN_BOXES,
    length_frames: 60.0,
    landing_lag_percent: None,
};
pub static SPIN_AIR: MoveData = MoveData {
    hitboxes: &SPIN_BOXES,
    length_frames: 100.0,
    landing_lag_percent: None,
};
