//! Samus motion attacks from `relocData/216_SamusMainMotion.c` (US).
//! Windows are half open and follow the script's Wait/WaitAsync cursor.
//! Lengths are the figatree lengths from `ssb_rom::anim::EXPECTED_FRAMES`.

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

mv!(
    JAB1,
    22.0,
    None,
    [
        h!(3, 180.0, 120.0, 0.0, 0.0, 361, 100, 0, 0, 4.0, 6.0),
        h!(3, 180.0, 60.0, 0.0, 0.0, 361, 100, 0, 0, 4.0, 6.0),
        h!(3, 180.0, -60.0, 0.0, 0.0, 361, 100, 0, 0, 4.0, 6.0),
    ]
);
mv!(
    JAB2,
    30.0,
    None,
    [
        h!(7, 220.0, 120.0, 0.0, 0.0, 361, 100, 0, 10, 7.0, 12.0),
        h!(7, 220.0, 30.0, 0.0, 0.0, 361, 100, 0, 10, 7.0, 12.0),
    ]
);
// The script replaces slot 0 at frame 10 without clearing it, so the weaker
// box keeps the first box's hit record.
mv!(
    DASH,
    40.0,
    None,
    [
        h!(12, 220.0, 16.0, 0.0, 0.0, 361, 100, 0, 0, 6.0, 10.0),
        h!(10, 220.0, 16.0, 0.0, 0.0, 361, 100, 0, 0, 10.0, 26.0),
    ]
);

macro_rules! ftilt {
    ($damage:expr) => {
        MoveData {
            hitboxes: &[
                h!($damage, 110.0, 0.0, 0.0, 0.0, 361, 100, 0, 10, 7.0, 12.0),
                h!($damage, 200.0, 0.0, 0.0, 0.0, 361, 100, 0, 10, 7.0, 12.0),
                h!($damage, 200.0, 180.0, 0.0, 0.0, 361, 100, 0, 10, 7.0, 12.0),
            ],
            length_frames: 32.0,
            landing_lag_percent: None,
        }
    };
}
pub static FTILT_HI: MoveData = ftilt!(12);
pub static FTILT_HI_S: MoveData = ftilt!(11);
pub static FTILT: MoveData = ftilt!(10);
pub static FTILT_LW_S: MoveData = ftilt!(9);
pub static FTILT_LW: MoveData = ftilt!(9);
mv!(
    UTILT,
    44.0,
    None,
    [
        h!(8, 200.0, -20.0, 0.0, 0.0, 361, 100, 0, 0, 8.0, 25.0),
        h!(8, 200.0, 180.0, 0.0, 0.0, 361, 100, 0, 0, 8.0, 25.0),
        h!(13, 300.0, -20.0, 0.0, 0.0, 361, 100, 0, 5, 25.0, 30.0),
        h!(13, 300.0, 180.0, 0.0, 0.0, 361, 100, 0, 5, 25.0, 30.0),
    ]
);
mv!(
    DTILT,
    40.0,
    None,
    [
        h!(13, 200.0, -40.0, 0.0, 0.0, 40, 100, 0, 10, 8.0, 13.0),
        h!(13, 200.0, 180.0, 0.0, 0.0, 40, 100, 0, 10, 8.0, 13.0),
    ]
);

macro_rules! fsmash {
    ($damage:expr) => {
        MoveData {
            hitboxes: &[
                h!($damage, 210.0, -80.0, 0.0, 0.0, 361, 100, 0, 15, 12.0, 16.0),
                h!($damage, 240.0, 120.0, 0.0, 0.0, 361, 100, 0, 15, 12.0, 16.0),
                h!($damage, 150.0, 0.0, 0.0, 0.0, 40, 100, 0, 15, 12.0, 16.0),
            ],
            length_frames: 42.0,
            landing_lag_percent: None,
        }
    };
}
pub static FSMASH_HI: MoveData = fsmash!(20);
pub static FSMASH_HI_S: MoveData = fsmash!(19);
pub static FSMASH: MoveData = fsmash!(18);
pub static FSMASH_LW_S: MoveData = fsmash!(17);
pub static FSMASH_LW: MoveData = fsmash!(16);

/// Five fire pulses. Each `RefreshAttackCollID` after a clear starts a new
/// hit record; the US build holds each pulse for three frames.
const fn usmash_boxes() -> [ActiveHitbox; 10] {
    let empty = ActiveHitbox::new(hit(0, 0.0, 0.0, 0.0, 0.0, 0, 0, 0, 0), 0.0, 0.0);
    let mut out = [empty; 10];
    let mut pulse = 0;
    while pulse < 5 {
        let start = 17.0 + pulse as f32 * 4.0;
        let end = if pulse == 4 { start + 2.0 } else { start + 3.0 };
        out[pulse * 2] =
            ActiveHitbox::new(hit(10, 190.0, -80.0, 0.0, 0.0, 80, 100, 0, 22), start, end)
                .with_hit_generation(pulse as u8);
        out[pulse * 2 + 1] =
            ActiveHitbox::new(hit(10, 400.0, 200.0, 0.0, 0.0, 80, 100, 0, 22), start, end)
                .with_hit_generation(pulse as u8);
        pulse += 1;
    }
    out
}
const USMASH_BOXES: [ActiveHitbox; 10] = usmash_boxes();
pub static USMASH: MoveData = MoveData {
    hitboxes: &USMASH_BOXES,
    length_frames: 60.0,
    landing_lag_percent: None,
};
mv!(
    DSMASH,
    50.0,
    None,
    [
        h!(16, 210.0, -60.0, 0.0, 0.0, 60, 80, 0, 35, 8.0, 13.0),
        h!(16, 240.0, 220.0, 0.0, 0.0, 60, 80, 0, 35, 8.0, 13.0),
        h!(14, 210.0, -60.0, 0.0, 0.0, 60, 80, 0, 35, 19.0, 23.0).with_hit_generation(1),
        h!(14, 240.0, 220.0, 0.0, 0.0, 60, 80, 0, 35, 19.0, 23.0).with_hit_generation(1),
    ]
);

mv!(
    AIR_N,
    50.0,
    Some(50),
    [
        h!(16, 240.0, -30.0, 0.0, 0.0, 361, 100, 0, 10, 4.0, 8.0),
        h!(16, 280.0, 50.0, 0.0, 0.0, 361, 100, 0, 10, 4.0, 8.0),
        h!(13, 240.0, -30.0, 0.0, 0.0, 361, 100, 0, 10, 8.0, 28.0),
        h!(13, 280.0, 50.0, 0.0, 0.0, 361, 100, 0, 10, 8.0, 28.0),
    ]
);

/// Four two-frame fire pulses, seven frames apart.
const fn air_f_boxes() -> [ActiveHitbox; 8] {
    let empty = ActiveHitbox::new(hit(0, 0.0, 0.0, 0.0, 0.0, 0, 0, 0, 0), 0.0, 0.0);
    let mut out = [empty; 8];
    let mut pulse = 0;
    while pulse < 4 {
        let start = 5.0 + pulse as f32 * 7.0;
        out[pulse * 2] = ActiveHitbox::new(
            hit(5, 230.0, -110.0, 0.0, 0.0, 361, 100, 0, 0),
            start,
            start + 2.0,
        )
        .with_hit_generation(pulse as u8);
        out[pulse * 2 + 1] = ActiveHitbox::new(
            hit(5, 350.0, 200.0, 0.0, 0.0, 361, 100, 0, 0),
            start,
            start + 2.0,
        )
        .with_hit_generation(pulse as u8);
        pulse += 1;
    }
    out
}
const AIR_F_BOXES: [ActiveHitbox; 8] = air_f_boxes();
pub static AIR_F: MoveData = MoveData {
    hitboxes: &AIR_F_BOXES,
    length_frames: 52.0,
    landing_lag_percent: Some(20),
};
mv!(
    AIR_B,
    40.0,
    None,
    [
        h!(14, 240.0, -30.0, 0.0, 0.0, 361, 145, 0, 0, 5.0, 9.0),
        h!(14, 260.0, 110.0, 0.0, 0.0, 361, 145, 0, 0, 5.0, 9.0),
        h!(10, 240.0, -30.0, 0.0, 0.0, 361, 145, 0, 0, 9.0, 17.0),
        h!(10, 260.0, 110.0, 0.0, 0.0, 361, 145, 0, 0, 9.0, 17.0),
    ]
);

/// Five two-frame pulses. The script's final 4-damage pair is created and
/// cleared on the same frame, so it never reaches a collision check.
const fn air_hi_boxes() -> [ActiveHitbox; 10] {
    let empty = ActiveHitbox::new(hit(0, 0.0, 0.0, 0.0, 0.0, 0, 0, 0, 0), 0.0, 0.0);
    let mut out = [empty; 10];
    let mut pulse = 0;
    while pulse < 5 {
        let start = 6.0 + pulse as f32 * 3.0;
        out[pulse * 2] = ActiveHitbox::new(
            hit(2, 320.0, -50.0, 45.0, 0.0, 80, 100, 30, 0),
            start,
            start + 2.0,
        )
        .with_hit_generation(pulse as u8);
        out[pulse * 2 + 1] = ActiveHitbox::new(
            hit(2, 360.0, 100.0, 30.0, 0.0, 80, 100, 30, 0),
            start,
            start + 2.0,
        )
        .with_hit_generation(pulse as u8);
        pulse += 1;
    }
    out
}
const AIR_HI_BOXES: [ActiveHitbox; 10] = air_hi_boxes();
pub static AIR_HI: MoveData = MoveData {
    hitboxes: &AIR_HI_BOXES,
    length_frames: 40.0,
    landing_lag_percent: None,
};
mv!(
    AIR_LW,
    40.0,
    Some(20),
    [
        h!(14, 250.0, -60.0, 0.0, 0.0, -90, 100, 0, 0, 4.0, 13.0),
        h!(14, 280.0, 120.0, 0.0, 0.0, -90, 100, 0, 0, 4.0, 13.0),
    ]
);

/// Screw Attack's grounded script. Four joint-0 boxes per window: the first
/// loop pass overwrites the frame-4 boxes without a clear, so frames 4..8
/// share one hit record; every later pass follows a clear.
const fn screw_ground_boxes() -> [ActiveHitbox; 56] {
    let empty = ActiveHitbox::new(hit(0, 0.0, 0.0, 0.0, 0.0, 0, 0, 0, 0), 0.0, 0.0);
    let mut out = [empty; 56];
    out[0] = ActiveHitbox::new(hit(2, 200.0, 0.0, -30.0, 140.0, 100, 0, 1, 120), 4.0, 6.0);
    out[1] = ActiveHitbox::new(hit(2, 200.0, 0.0, -30.0, -140.0, 100, 0, 1, 120), 4.0, 6.0);
    out[2] = ActiveHitbox::new(hit(2, 200.0, 0.0, 330.0, 140.0, 100, 0, 1, 110), 4.0, 6.0);
    out[3] = ActiveHitbox::new(hit(2, 200.0, 0.0, 330.0, -140.0, 100, 0, 1, 110), 4.0, 6.0);
    let mut pass = 0;
    while pass < 3 {
        let start = 6.0 + pass as f32 * 2.0;
        let generation = pass as u8;
        let base = 4 + pass * 4;
        out[base] = ActiveHitbox::new(
            hit(1, 180.0, 0.0, -80.0, 110.0, 105, 0, 1, 110),
            start,
            start + 2.0,
        )
        .with_hit_generation(generation);
        out[base + 1] = ActiveHitbox::new(
            hit(1, 180.0, 0.0, -80.0, -110.0, 105, 0, 1, 110),
            start,
            start + 2.0,
        )
        .with_hit_generation(generation);
        out[base + 2] = ActiveHitbox::new(
            hit(1, 180.0, 0.0, 310.0, 110.0, 100, 0, 1, 70),
            start,
            start + 2.0,
        )
        .with_hit_generation(generation);
        out[base + 3] = ActiveHitbox::new(
            hit(1, 180.0, 0.0, 310.0, -110.0, 100, 0, 1, 70),
            start,
            start + 2.0,
        )
        .with_hit_generation(generation);
        pass += 1;
    }
    let mut pass = 0;
    while pass < 10 {
        let start = 12.0 + pass as f32 * 2.0;
        let generation = pass as u8 + 3;
        let base = 16 + pass * 4;
        out[base] = ActiveHitbox::new(
            hit(1, 140.0, 0.0, 290.0, 100.0, 200, 0, 1, 20),
            start,
            start + 2.0,
        )
        .with_hit_generation(generation);
        out[base + 1] = ActiveHitbox::new(
            hit(1, 140.0, 0.0, 290.0, -100.0, 200, 0, 1, 20),
            start,
            start + 2.0,
        )
        .with_hit_generation(generation);
        out[base + 2] = ActiveHitbox::new(
            hit(1, 140.0, 0.0, 10.0, 100.0, 110, 0, 1, 50),
            start,
            start + 2.0,
        )
        .with_hit_generation(generation);
        out[base + 3] = ActiveHitbox::new(
            hit(1, 140.0, 0.0, 10.0, -100.0, 110, 0, 1, 50),
            start,
            start + 2.0,
        )
        .with_hit_generation(generation);
        pass += 1;
    }
    out
}

/// Screw Attack's aerial script: thirteen two-frame passes, then the
/// `0x1C68` finisher at frame 30.
const fn screw_air_boxes() -> [ActiveHitbox; 53] {
    let empty = ActiveHitbox::new(hit(0, 0.0, 0.0, 0.0, 0.0, 0, 0, 0, 0), 0.0, 0.0);
    let mut out = [empty; 53];
    let mut pass = 0;
    while pass < 13 {
        let start = 4.0 + pass as f32 * 2.0;
        let generation = pass as u8;
        let base = pass * 4;
        out[base] = ActiveHitbox::new(
            hit(1, 160.0, 0.0, 290.0, 150.0, 200, 0, 1, 20),
            start,
            start + 2.0,
        )
        .with_hit_generation(generation);
        out[base + 1] = ActiveHitbox::new(
            hit(1, 160.0, 0.0, 290.0, -150.0, 200, 0, 1, 20),
            start,
            start + 2.0,
        )
        .with_hit_generation(generation);
        out[base + 2] = ActiveHitbox::new(
            hit(1, 160.0, 0.0, 10.0, 150.0, 110, 0, 1, 50),
            start,
            start + 2.0,
        )
        .with_hit_generation(generation);
        out[base + 3] = ActiveHitbox::new(
            hit(1, 160.0, 0.0, 10.0, -150.0, 110, 0, 1, 50),
            start,
            start + 2.0,
        )
        .with_hit_generation(generation);
        pass += 1;
    }
    out[52] = ActiveHitbox::new(hit(1, 400.0, 0.0, 100.0, 0.0, 361, 100, 80, 0), 30.0, 32.0)
        .with_hit_generation(13);
    out
}
const SCREW_GROUND_BOXES: [ActiveHitbox; 56] = screw_ground_boxes();
const SCREW_AIR_BOXES: [ActiveHitbox; 53] = screw_air_boxes();
pub static SCREW_GROUND: MoveData = MoveData {
    hitboxes: &SCREW_GROUND_BOXES,
    length_frames: 50.0,
    landing_lag_percent: None,
};
pub static SCREW_AIR: MoveData = MoveData {
    hitboxes: &SCREW_AIR_BOXES,
    length_frames: 48.0,
    landing_lag_percent: None,
};
