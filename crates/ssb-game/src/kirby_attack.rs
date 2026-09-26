//! Kirby's US motion collisions (`228_KirbyMainMotion.c`). Windows are half
//! open; replacing a live slot keeps its hit generation, and a
//! `ClearAttackCollAll` or `RefreshAttackCollID` starts a new one.

use crate::attack::{ActiveHitbox, Hitbox, MoveData};
use ssb_engine::math::Vec3;

#[allow(clippy::too_many_arguments)]
const fn hit(d: i32, size: f32, x: f32, y: f32, z: f32, a: i32, g: i32, w: i32, b: i32) -> Hitbox {
    Hitbox {
        damage: d,
        radius: size / 2.0,
        offset: Vec3::new(x, y, z),
        angle: a,
        kb_scale: g,
        kb_weight: w,
        kb_base: b,
    }
}
macro_rules! h {
    ($d:expr,$sz:expr,$x:expr,$y:expr,$z:expr,$a:expr,$g:expr,$w:expr,$b:expr,$s:expr,$e:expr) => {
        ActiveHitbox::new(hit($d, $sz, $x, $y, $z, $a, $g, $w, $b), $s, $e)
    };
}
macro_rules! mv {
    ($name:ident,$len:expr,$lag:expr,[$($box:expr),* $(,)?]) => {
        pub static $name: MoveData = MoveData { hitboxes: &[$($box),*], length_frames: $len, landing_lag_percent: $lag };
    };
}

/// The 10-bit signed angle field: `934` and `939` read as -90 and -85.
const DOWN_90: i32 = 934 - 1024;
const DOWN_85: i32 = 939 - 1024;

pub const JAB1_FLAG1_FRAME: f32 = 10.0;
pub const JAB2_FLAG1_FRAME: f32 = 8.0;

mv!(
    JAB1,
    18.0,
    None,
    [
        h!(3, 210.0, -50.0, 0.0, 0.0, 361, 50, 0, 8, 3.0, 5.0),
        h!(3, 210.0, 70.0, 0.0, 0.0, 361, 50, 0, 8, 3.0, 5.0),
    ]
);
mv!(
    JAB2,
    20.0,
    None,
    [
        h!(4, 210.0, -50.0, 0.0, 0.0, 70, 50, 0, 8, 3.0, 6.0),
        h!(4, 210.0, 70.0, 0.0, 0.0, 70, 50, 0, 8, 3.0, 6.0),
    ]
);

/// `FTKirbyAnimJabLoop` wraps every 25 frames.
pub const RAPID_LOOP_LENGTH: f32 = 25.0;
/// `SetFlag1(1)` after each `Jab_Subroutine` but the first.
pub const RAPID_FLAG1_FRAMES: [f32; 4] = [8.0, 13.0, 18.0, 23.0];
/// After a wrap the `PauseScript` resumes into `0x120C`: one more pulse and
/// flag 1 at frame 2, then `Goto(JabLoop)` pulses again at once.
pub const RAPID_WRAPPED_FLAG1_FRAMES: [f32; 5] = [2.0, 8.0, 13.0, 18.0, 23.0];

const fn rapid_pair(start: f32, generation: u8) -> [ActiveHitbox; 2] {
    [
        ActiveHitbox::new(
            hit(1, 100.0, -60.0, 0.0, 0.0, 65, 50, 0, 8),
            start,
            start + 2.0,
        )
        .with_hit_generation(generation),
        ActiveHitbox::new(
            hit(1, 100.0, 165.0, 0.0, 0.0, 65, 50, 0, 8),
            start,
            start + 2.0,
        )
        .with_hit_generation(generation),
    ]
}

const fn rapid_boxes<const N: usize>(starts: [f32; N]) -> [[ActiveHitbox; 2]; N] {
    let mut out = [rapid_pair(0.0, 0); N];
    let mut i = 0;
    while i < N {
        out[i] = rapid_pair(starts[i], i as u8);
        i += 1;
    }
    out
}

const RAPID_FIRST: [[ActiveHitbox; 2]; 5] = rapid_boxes([0.0, 6.0, 11.0, 16.0, 21.0]);
const RAPID_WRAPPED: [[ActiveHitbox; 2]; 6] = rapid_boxes([0.0, 2.0, 6.0, 11.0, 16.0, 21.0]);

pub static RAPID_LOOP_FIRST: MoveData = MoveData {
    hitboxes: RAPID_FIRST.as_flattened(),
    length_frames: RAPID_LOOP_LENGTH,
    landing_lag_percent: None,
};
pub static RAPID_LOOP_WRAPPED: MoveData = MoveData {
    hitboxes: RAPID_WRAPPED.as_flattened(),
    length_frames: RAPID_LOOP_LENGTH,
    landing_lag_percent: None,
};

mv!(
    DASH,
    40.0,
    None,
    [
        h!(10, 260.0, 0.0, 150.0, 200.0, 361, 100, 0, 10, 4.0, 8.0),
        h!(8, 260.0, 0.0, 150.0, 200.0, 361, 100, 0, 10, 8.0, 18.0),
    ]
);
macro_rules! ftilt {
    ($name:ident,$damage:expr) => {
        mv!(
            $name,
            28.0,
            None,
            [
                h!($damage, 250.0, 0.0, 0.0, 0.0, 361, 100, 0, 8, 4.0, 10.0),
                h!($damage, 250.0, 160.0, 0.0, 0.0, 361, 100, 0, 8, 4.0, 10.0),
            ]
        );
    };
}
ftilt!(FTILT_HI, 11);
ftilt!(FTILT, 10);
ftilt!(FTILT_LW, 9);
mv!(
    UTILT,
    18.0,
    None,
    [
        h!(14, 280.0, 0.0, 0.0, 0.0, 96, 100, 0, 30, 4.0, 6.0),
        h!(14, 320.0, 200.0, 0.0, 0.0, 96, 100, 0, 30, 4.0, 6.0),
        h!(10, 280.0, 0.0, 0.0, 0.0, 88, 100, 0, 20, 6.0, 12.0),
        h!(10, 320.0, 200.0, 0.0, 0.0, 88, 100, 0, 20, 6.0, 12.0),
    ]
);
mv!(
    DTILT,
    30.0,
    None,
    [
        h!(9, 250.0, 0.0, 0.0, 0.0, 20, 110, 0, 0, 4.0, 11.0),
        h!(9, 350.0, 150.0, 0.0, 0.0, 20, 110, 0, 0, 4.0, 11.0),
    ]
);
mv!(
    FSMASH,
    40.0,
    None,
    [
        h!(17, 280.0, -20.0, 0.0, 0.0, 361, 120, 0, 20, 10.0, 14.0),
        h!(17, 280.0, 140.0, 0.0, 0.0, 361, 120, 0, 20, 10.0, 14.0),
        h!(12, 250.0, -20.0, 0.0, 0.0, 361, 120, 0, 12, 14.0, 24.0),
        h!(12, 250.0, 140.0, 0.0, 0.0, 361, 120, 0, 12, 14.0, 24.0),
    ]
);
mv!(
    USMASH,
    50.0,
    None,
    [
        h!(16, 320.0, 140.0, 0.0, 0.0, 90, 120, 0, 20, 14.0, 18.0),
        h!(16, 280.0, 0.0, 0.0, 0.0, 90, 120, 0, 20, 14.0, 18.0),
        h!(12, 200.0, 140.0, 0.0, 0.0, 361, 100, 0, 10, 18.0, 24.0),
        h!(12, 230.0, 0.0, 0.0, 0.0, 361, 100, 0, 10, 18.0, 24.0),
    ]
);
// `SetAttackCollDamage` after `Wait(4)` keeps the slots and their record.
mv!(
    DSMASH,
    56.0,
    None,
    [
        h!(18, 300.0, 180.0, 0.0, 0.0, 34, 75, 0, 30, 7.0, 11.0),
        h!(18, 300.0, 180.0, 0.0, 0.0, 34, 75, 0, 30, 7.0, 11.0),
        h!(18, 180.0, 0.0, 0.0, 0.0, 35, 75, 0, 30, 7.0, 11.0),
        h!(10, 300.0, 180.0, 0.0, 0.0, 34, 75, 0, 30, 11.0, 27.0),
        h!(10, 300.0, 180.0, 0.0, 0.0, 34, 75, 0, 30, 11.0, 27.0),
        h!(10, 180.0, 0.0, 0.0, 0.0, 35, 75, 0, 30, 11.0, 27.0),
    ]
);
mv!(
    AIR_N,
    50.0,
    Some(50),
    [
        h!(15, 220.0, 0.0, 0.0, 0.0, 361, 100, 0, 30, 3.0, 7.0),
        h!(15, 260.0, 120.0, 0.0, 0.0, 361, 100, 0, 30, 3.0, 7.0),
        h!(10, 210.0, 0.0, 0.0, 0.0, 361, 100, 0, 10, 7.0, 31.0),
        h!(10, 260.0, 120.0, 0.0, 0.0, 361, 100, 0, 10, 7.0, 31.0),
    ]
);

/// Seven two-frame drill pulses (the `LoopBegin(6)` refreshes plus the one
/// after the loop), then the one-frame finisher.
const fn air_f_boxes() -> [ActiveHitbox; 16] {
    let empty = ActiveHitbox::new(hit(0, 0.0, 0.0, 0.0, 0.0, 0, 0, 0, 0), 0.0, 0.0);
    let mut out = [empty; 16];
    let mut i = 0;
    while i < 7 {
        let start = 8.0 + 3.0 * i as f32;
        out[i * 2] = ActiveHitbox::new(
            hit(2, 380.0, 0.0, 150.0, 220.0, DOWN_90, 100, 30, 0),
            start,
            start + 2.0,
        )
        .with_hit_generation(i as u8);
        out[i * 2 + 1] = ActiveHitbox::new(
            hit(2, 360.0, 0.0, 150.0, 100.0, DOWN_90, 100, 30, 0),
            start,
            start + 2.0,
        )
        .with_hit_generation(i as u8);
        i += 1;
    }
    out[14] = ActiveHitbox::new(
        hit(6, 380.0, 0.0, 150.0, 220.0, 361, 120, 0, 10),
        28.0,
        29.0,
    )
    .with_hit_generation(7);
    out[15] = ActiveHitbox::new(
        hit(6, 360.0, 0.0, 150.0, 100.0, 361, 120, 0, 10),
        28.0,
        29.0,
    )
    .with_hit_generation(7);
    out
}
const AIR_F_BOXES: [ActiveHitbox; 16] = air_f_boxes();
pub static AIR_F: MoveData = MoveData {
    hitboxes: &AIR_F_BOXES,
    length_frames: 40.0,
    landing_lag_percent: Some(30),
};
mv!(
    LANDING_AIR_F,
    30.0,
    None,
    [
        h!(3, 300.0, 0.0, 150.0, 220.0, 361, 100, 80, 0, 0.0, 3.0),
        h!(3, 270.0, 0.0, 150.0, 100.0, 361, 100, 80, 0, 0.0, 3.0),
    ]
);
mv!(
    AIR_B,
    40.0,
    Some(100),
    [
        h!(16, 380.0, 0.0, 150.0, -240.0, 361, 100, 0, 10, 6.0, 10.0),
        h!(16, 360.0, 0.0, 150.0, -100.0, 361, 100, 0, 10, 6.0, 10.0),
        h!(12, 380.0, 0.0, 150.0, -240.0, 361, 100, 0, 0, 10.0, 26.0),
        h!(12, 360.0, 0.0, 150.0, -100.0, 361, 100, 0, 0, 10.0, 26.0),
    ]
);
// The 12 of `MakeAttackColl` is replaced on the same frame: the following
// `WaitAsync(8)` has already passed at frame 10.
mv!(
    AIR_HI,
    80.0,
    Some(40),
    [
        h!(10, 210.0, 0.0, 60.0, 0.0, 361, 100, 100, 10, 10.0, 18.0),
        h!(8, 210.0, 0.0, 60.0, 0.0, 361, 100, 100, 10, 18.0, 30.0),
        h!(6, 210.0, 0.0, 60.0, 0.0, 361, 100, 100, 10, 30.0, 46.0),
    ]
);

/// Ten two-frame pulses: `LoopBegin(9)` refreshes the pair every 3 frames.
const fn air_lw_boxes() -> [ActiveHitbox; 20] {
    let empty = ActiveHitbox::new(hit(0, 0.0, 0.0, 0.0, 0.0, 0, 0, 0, 0), 0.0, 0.0);
    let mut out = [empty; 20];
    let mut i = 0;
    while i < 10 {
        let start = 4.0 + 3.0 * i as f32;
        out[i * 2] = ActiveHitbox::new(
            hit(3, 380.0, 0.0, -120.0, 100.0, DOWN_90, 100, 0, 30),
            start,
            start + 2.0,
        )
        .with_hit_generation(i as u8);
        out[i * 2 + 1] = ActiveHitbox::new(
            hit(3, 360.0, 0.0, 0.0, 60.0, DOWN_90, 100, 0, 30),
            start,
            start + 2.0,
        )
        .with_hit_generation(i as u8);
        i += 1;
    }
    out
}
const AIR_LW_BOXES: [ActiveHitbox; 20] = air_lw_boxes();
pub static AIR_LW: MoveData = MoveData {
    hitboxes: &AIR_LW_BOXES,
    length_frames: 50.0,
    landing_lag_percent: Some(22),
};
// `LandingAirX_0x18F4`, Kirby's `LandingAirNull` script.
mv!(
    LANDING_AIR_NULL,
    0.0,
    None,
    [
        h!(3, 300.0, 0.0, 0.0, 150.0, 361, 100, 80, 0, 0.0, 1.0),
        h!(3, 300.0, 0.0, 0.0, -150.0, 361, 100, 80, 0, 0.0, 1.0),
    ]
);
// `ForwardThrowRecoil` (US): the suplex impact.
mv!(
    THROWF_LANDING,
    35.0,
    None,
    [h!(7, 200.0, 0.0, 0.0, 0.0, 361, 80, 0, 10, 0.0, 8.0)]
);
// `FinalCutter`: four rising boxes for one frame, then the falling blade.
// `SetAttackCollSize(0, 0)` and `(0, 450)` keep the slot and its record.
mv!(
    FINAL_CUTTER,
    60.0,
    None,
    [
        h!(8, 150.0, 0.0, 100.0, 200.0, 88, 100, 121, 0, 23.0, 24.0),
        h!(8, 150.0, 0.0, 100.0, 530.0, 96, 100, 121, 0, 23.0, 24.0),
        h!(8, 150.0, 0.0, 400.0, 200.0, 88, 100, 105, 0, 23.0, 24.0),
        h!(8, 150.0, 0.0, 400.0, 530.0, 96, 100, 105, 0, 23.0, 24.0),
        h!(2, 330.0, 0.0, 300.0, 400.0, DOWN_85, 100, 200, 0, 46.0, 50.0).with_hit_generation(1),
        h!(2, 0.0, 0.0, 300.0, 400.0, DOWN_85, 100, 200, 0, 50.0, 54.0).with_hit_generation(1),
        h!(2, 450.0, 0.0, 300.0, 400.0, DOWN_85, 100, 200, 0, 54.0, 57.0).with_hit_generation(1),
    ]
);
// `StoneGround_0x1BB4` / `_0x1BEC`: the falling stone, never cleared.
mv!(
    STONE,
    0.0,
    None,
    [h!(20, 400.0, 0.0, 0.0, 0.0, 50, 70, 0, 70, 0.0, f32::MAX)]
);
/// `dFTCommonMoveset_DamageBumpHit`: every fighter's spat-out star. The
/// damage is replaced by the victim's [`STAR_DAMAGE`].
pub const STAR: Hitbox = hit(15, 300.0, 0.0, 210.0, 0.0, 361, 70, 0, 60);
static STAR_BOXES: [ActiveHitbox; 1] = [ActiveHitbox::new(STAR, 0.0, f32::MAX)];
pub static STAR_MOVE: MoveData = MoveData {
    hitboxes: &STAR_BOXES,
    length_frames: 0.0,
    landing_lag_percent: None,
};

/// `FTKirbyCopy[27]` at `KirbyMainMotion` 0x0000: `(copy_id, star_damage)`
/// per swallowed `FTKind`. The model-part and scale columns are
/// presentation.
pub const COPY: [(u8, i32); 27] = [
    (0, 17),
    (1, 17),
    (2, 30),
    (3, 17),
    (4, 17),
    (5, 17),
    (6, 25),
    (7, 17),
    (8, 17),
    (9, 17),
    (10, 17),
    (11, 17),
    (8, 17),
    (8, 17),
    (8, 17),
    (8, 17),
    (8, 30),
    (8, 17),
    (8, 17),
    (8, 17),
    (8, 17),
    (8, 17),
    (8, 17),
    (8, 17),
    (8, 17),
    (8, 17),
    (2, 50),
];

/// Star damage for a swallowed fighter kind.
pub fn star_damage(kind: crate::fighter::FighterKind) -> i32 {
    COPY[kind as usize].1
}

/// The Kirby-owned attack joints, in [`MoveData`] box order.
pub fn joints(status: crate::status::AnyStatus, index: usize) -> Option<u8> {
    use crate::status::{AnyStatus, KirbyStatus as K, Status};
    let ids: &[u8] = match status {
        AnyStatus::Common(Status::Attack11) => &[11, 11],
        AnyStatus::Common(Status::Attack12) => &[16, 16],
        AnyStatus::Kirby(K::Attack100Loop) => &[16],
        AnyStatus::Common(Status::AttackS3Hi | Status::AttackS3 | Status::AttackS3Lw) => &[24],
        AnyStatus::Common(Status::AttackHi3) => &[29],
        AnyStatus::Common(Status::AttackLw3) => &[24],
        AnyStatus::Common(Status::AttackS4) => &[24],
        AnyStatus::Common(Status::AttackHi4) => &[29, 27, 29, 27],
        AnyStatus::Common(Status::AttackLw4) => &[29, 24, 5, 29, 24, 5],
        AnyStatus::Common(Status::AttackAirN) => &[5, 24, 5, 24],
        AnyStatus::Common(Status::AttackAirHi) => &[5],
        _ => &[0],
    };
    Some(ids.get(index).copied().unwrap_or(ids[ids.len() - 1]))
}
