//! Donkey Kong motion attacks from `relocData/212_DonkeyMainMotion.c` (US).
//! Windows are half open and follow the script's Wait/WaitAsync cursor.

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
    25.0,
    None,
    [
        h!(4, 280.0, 140.0, 0.0, 0.0, 361, 100, 0, 0, 5.0, 9.0),
        h!(4, 210.0, -40.0, 0.0, 0.0, 361, 100, 0, 0, 5.0, 9.0),
    ]
);
mv!(
    JAB2,
    34.0,
    None,
    [
        h!(4, 280.0, 140.0, 0.0, 0.0, 361, 100, 0, 0, 6.0, 12.0),
        h!(4, 210.0, -40.0, 0.0, 0.0, 361, 100, 0, 0, 6.0, 12.0),
    ]
);
mv!(
    DASH,
    70.0,
    None,
    [h!(12, 290.0, 90.0, 0.0, 0.0, 100, 100, 120, 0, 2.0, 26.0),]
);

macro_rules! tilt {
    ($damage:expr) => {
        MoveData {
            hitboxes: &[
                h!($damage, 240.0, 80.0, 0.0, 0.0, 361, 100, 0, 0, 12.0, 18.0),
                h!($damage, 280.0, 150.0, 0.0, 0.0, 361, 100, 0, 0, 12.0, 18.0),
                h!($damage, 180.0, -30.0, 0.0, 0.0, 361, 100, 0, 0, 12.0, 18.0),
            ],
            length_frames: 43.0,
            landing_lag_percent: None,
        }
    };
}
pub static FTILT_HI: MoveData = tilt!(13);
pub static FTILT: MoveData = tilt!(12);
pub static FTILT_LW: MoveData = tilt!(11);
mv!(
    UTILT,
    60.0,
    None,
    [
        h!(13, 240.0, 80.0, 0.0, 0.0, 100, 130, 0, 10, 4.0, 24.0),
        h!(13, 250.0, 190.0, 0.0, 0.0, 100, 130, 0, 10, 4.0, 24.0),
    ]
);
mv!(
    DTILT,
    22.0,
    None,
    [
        h!(8, 220.0, 80.0, 0.0, 0.0, 40, 80, 0, 30, 11.0, 17.0),
        h!(8, 240.0, 180.0, 0.0, 0.0, 40, 80, 0, 30, 11.0, 17.0),
    ]
);

macro_rules! fsmash {
    ($damage:expr) => {
        MoveData {
            hitboxes: &[
                h!($damage, 280.0, 80.0, 0.0, 0.0, 361, 100, 0, 20, 27.0, 33.0),
                h!($damage, 320.0, 240.0, 0.0, 0.0, 361, 100, 0, 20, 27.0, 33.0),
                h!($damage, 200.0, -30.0, 0.0, 0.0, 361, 100, 0, 20, 27.0, 33.0),
            ],
            length_frames: 60.0,
            landing_lag_percent: None,
        }
    };
}
pub static FSMASH_HI: MoveData = fsmash!(21);
pub static FSMASH_HI_S: MoveData = fsmash!(21);
pub static FSMASH: MoveData = fsmash!(20);
pub static FSMASH_LW_S: MoveData = fsmash!(19);
pub static FSMASH_LW: MoveData = fsmash!(19);
mv!(
    USMASH,
    60.0,
    None,
    [
        h!(21, 480.0, 180.0, 0.0, 0.0, 90, 100, 0, 40, 16.0, 19.0),
        h!(21, 480.0, 180.0, 0.0, 0.0, 90, 100, 0, 40, 16.0, 19.0),
    ]
);
mv!(
    DSMASH,
    70.0,
    None,
    [
        h!(19, 210.0, 0.0, 0.0, 0.0, 60, 100, 0, 30, 12.0, 32.0),
        h!(19, 210.0, 0.0, 0.0, 0.0, 60, 100, 0, 30, 12.0, 32.0),
        h!(19, 270.0, 180.0, 0.0, 0.0, 60, 100, 0, 30, 12.0, 32.0),
        h!(19, 270.0, 180.0, 0.0, 0.0, 60, 100, 0, 30, 12.0, 32.0),
    ]
);

mv!(
    AIR_N,
    60.0,
    Some(50),
    [
        h!(15, 200.0, 120.0, 0.0, 0.0, 361, 100, 0, 0, 4.0, 8.0),
        h!(15, 200.0, 120.0, 0.0, 0.0, 361, 100, 0, 0, 4.0, 8.0),
        h!(15, 280.0, 0.0, 0.0, 0.0, 361, 100, 0, 0, 4.0, 8.0),
        h!(12, 200.0, 120.0, 0.0, 0.0, 361, 100, 0, 0, 8.0, 30.0),
        h!(12, 200.0, 120.0, 0.0, 0.0, 361, 100, 0, 0, 8.0, 30.0),
        h!(12, 280.0, 0.0, 0.0, 0.0, 361, 100, 0, 0, 8.0, 30.0),
    ]
);
mv!(
    AIR_F,
    50.0,
    None,
    [
        h!(16, 240.0, 220.0, 0.0, 0.0, -70, 100, 0, 0, 8.0, 16.0),
        h!(16, 160.0, 80.0, 0.0, 0.0, 361, 100, 0, 20, 8.0, 16.0),
        h!(16, 160.0, 80.0, 0.0, 0.0, 361, 100, 0, 20, 8.0, 16.0),
    ]
);
mv!(
    AIR_B,
    60.0,
    None,
    [
        h!(15, 340.0, 0.0, 200.0, -370.0, 361, 100, 0, 10, 8.0, 14.0),
        h!(15, 280.0, 0.0, 140.0, -170.0, 361, 100, 0, 10, 8.0, 14.0),
        h!(10, 340.0, 0.0, 200.0, -370.0, 361, 100, 0, 0, 14.0, 36.0),
        h!(10, 280.0, 0.0, 140.0, -170.0, 361, 100, 0, 0, 14.0, 36.0),
    ]
);
mv!(
    AIR_HI,
    60.0,
    Some(20),
    [
        h!(12, 260.0, 10.0, 0.0, 0.0, 110, 100, 0, 0, 3.0, 21.0),
        h!(12, 300.0, 150.0, 0.0, 0.0, 110, 100, 0, 0, 3.0, 21.0),
    ]
);
mv!(
    AIR_LW,
    60.0,
    Some(20),
    [
        h!(13, 330.0, 80.0, 0.0, 0.0, -90, 90, 0, 15, 6.0, 12.0),
        h!(13, 330.0, 80.0, 0.0, 0.0, -90, 90, 0, 15, 6.0, 12.0),
        h!(13, 280.0, -80.0, 0.0, 0.0, -90, 90, 0, 15, 6.0, 12.0),
        h!(13, 280.0, -80.0, 0.0, 0.0, -90, 90, 0, 15, 6.0, 12.0),
        h!(10, 300.0, 80.0, 0.0, 0.0, -90, 90, 0, 15, 12.0, 30.0),
        h!(10, 300.0, 80.0, 0.0, 0.0, -90, 90, 0, 15, 12.0, 30.0),
        h!(10, 260.0, -80.0, 0.0, 0.0, -90, 90, 0, 15, 12.0, 30.0),
        h!(10, 260.0, -80.0, 0.0, 0.0, -90, 90, 0, 15, 12.0, 30.0),
    ]
);

// Giant Punch: the non-full script creates 14-damage hitboxes at frame 9;
// ftDonkeySpecialNEndProcUpdate adds two damage per stored charge. The full
// script instead creates fixed 36-damage hitboxes for frames 9..17.
mv!(
    PUNCH,
    80.0,
    None,
    [
        h!(14, 280.0, 0.0, 0.0, 0.0, 361, 100, 0, 0, 9.0, 12.0),
        h!(14, 340.0, 290.0, 0.0, 0.0, 361, 100, 0, 0, 9.0, 12.0),
    ]
);
mv!(
    PUNCH_FULL,
    80.0,
    None,
    [
        h!(36, 280.0, 0.0, 0.0, 0.0, 361, 100, 0, 0, 9.0, 17.0),
        h!(36, 340.0, 290.0, 0.0, 0.0, 361, 100, 0, 0, 9.0, 17.0),
        h!(36, 50.0, 0.0, 0.0, 0.0, 361, 100, 0, 0, 9.0, 17.0),
    ]
);

// Spinning Kong recreates collision on eight-frame loop boundaries. The
// aerial script appends three smaller, three-damage loops after the five
// shared loops. The slot generation tracks those recreations.
const fn spin_boxes<const N: usize>(air: bool) -> [ActiveHitbox; N] {
    let empty = ActiveHitbox::new(hit(0, 0.0, 0.0, 0.0, 0.0, 0, 0, 0, 0), 0.0, 0.0);
    let mut out = [empty; N];
    let first_size = if air { 220.0 } else { 200.0 };
    out[0] = ActiveHitbox::new(
        hit(12, first_size, 200.0, 0.0, 0.0, 361, 120, 100, 0),
        3.0,
        9.0,
    );
    out[1] = out[0];
    let mut cycle = 0;
    while cycle < 5 {
        let start = 9.0 + cycle as f32 * 8.0;
        let generation = cycle as u8 + 1;
        let base = 2 + cycle * 3;
        out[base] = ActiveHitbox::new(
            hit(8, 200.0, 200.0, 0.0, 0.0, 361, 120, 100, 0),
            start,
            start + 8.0,
        )
        .with_hit_generation(generation);
        out[base + 1] = out[base];
        out[base + 2] = ActiveHitbox::new(
            hit(8, 160.0, 100.0, 0.0, 0.0, 361, 120, 100, 0),
            start,
            start + 8.0,
        )
        .with_hit_generation(generation);
        cycle += 1;
    }
    if air {
        let mut tail = 0;
        while tail < 3 {
            let start = 49.0 + tail as f32 * 8.0;
            let generation = tail as u8 + 6;
            let base = 17 + tail * 3;
            out[base] = ActiveHitbox::new(
                hit(3, 140.0, 200.0, 0.0, 0.0, 361, 100, 0, 0),
                start,
                start + 8.0,
            )
            .with_hit_generation(generation);
            out[base + 1] = out[base];
            out[base + 2] = ActiveHitbox::new(
                hit(3, 80.0, 100.0, 0.0, 0.0, 361, 100, 0, 0),
                start,
                start + 8.0,
            )
            .with_hit_generation(generation);
            tail += 1;
        }
    }
    out
}
const SPIN_GROUND_BOXES: [ActiveHitbox; 17] = spin_boxes::<17>(false);
const SPIN_AIR_BOXES: [ActiveHitbox; 26] = spin_boxes::<26>(true);
pub static SPIN_GROUND: MoveData = MoveData {
    hitboxes: &SPIN_GROUND_BOXES,
    length_frames: 100.0,
    landing_lag_percent: None,
};
pub static SPIN_AIR: MoveData = MoveData {
    hitboxes: &SPIN_AIR_BOXES,
    length_frames: 100.0,
    landing_lag_percent: None,
};

mv!(
    HAND_SLAP,
    28.0,
    None,
    [
        h!(10, 400.0, 0.0, 0.0, -500.0, 90, 100, 150, 0, 16.0, 18.0),
        h!(10, 400.0, 0.0, 0.0, -100.0, 90, 100, 150, 0, 16.0, 18.0),
        h!(10, 400.0, 0.0, 0.0, 300.0, 90, 100, 150, 0, 16.0, 18.0),
        h!(10, 400.0, 0.0, 0.0, 700.0, 90, 100, 150, 0, 16.0, 18.0),
        h!(10, 400.0, 0.0, 0.0, -500.0, 90, 100, 150, 0, 26.0, 28.0).with_hit_generation(1),
        h!(10, 400.0, 0.0, 0.0, -100.0, 90, 100, 150, 0, 26.0, 28.0).with_hit_generation(1),
        h!(10, 400.0, 0.0, 0.0, 300.0, 90, 100, 150, 0, 26.0, 28.0).with_hit_generation(1),
        h!(10, 400.0, 0.0, 0.0, 700.0, 90, 100, 150, 0, 26.0, 28.0).with_hit_generation(1),
    ]
);
