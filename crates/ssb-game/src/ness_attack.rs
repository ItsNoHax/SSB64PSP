//! Ness's US motion collisions (`238_NessMainMotion.c`), transcribed with
//! `tools/extract_ness_moves.py`. Windows are half open; a
//! `SetAttackCollDamage` or `SetAttackCollSize` keeps the hit generation,
//! and a `ClearAttackCollAll` or `RefreshAttackCollID` starts a new one.

use crate::attack::{ActiveHitbox, Hitbox, MoveData};
use crate::status::{AnyStatus, NessStatus as N, Status};
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

mv!(
    ATTACK11,
    18.0,
    None,
    [
        h!(2, 160.0, 0.0, 0.0, 0.0, 361, 50, 0, 8, 2.0, 4.0),
        h!(2, 160.0, 0.0, 0.0, 0.0, 361, 50, 0, 8, 2.0, 4.0),
    ]
);
// joints ATTACK11: [10, 9]

mv!(
    ATTACK12,
    20.0,
    None,
    [
        h!(2, 180.0, 16.0, 0.0, 0.0, 70, 50, 0, 8, 3.0, 6.0),
        h!(2, 180.0, 0.0, 0.0, 0.0, 70, 50, 0, 8, 3.0, 6.0),
    ]
);
// joints ATTACK12: [16, 15]

mv!(
    ATTACK13,
    25.0,
    None,
    [
        h!(4, 150.0, 0.0, 0.0, 0.0, 361, 100, 0, 10, 6.0, 8.0),
        h!(4, 280.0, 0.0, 0.0, 0.0, 361, 100, 0, 10, 6.0, 8.0),
        h!(4, 180.0, 0.0, 0.0, 0.0, 361, 100, 0, 10, 8.0, 12.0),
        h!(4, 280.0, 0.0, 0.0, 0.0, 361, 100, 0, 10, 8.0, 12.0),
    ]
);
// joints ATTACK13: [26, 28, 26, 28]

mv!(
    ATTACKDASH,
    37.0,
    None,
    [
        h!(12, 280.0, 0.0, 140.0, 120.0, 361, 100, 0, 16, 7.0, 11.0),
        h!(9, 250.0, 0.0, 140.0, 120.0, 361, 100, 0, 10, 11.0, 25.0),
    ]
);
// joints ATTACKDASH: [0, 0]

mv!(
    ATTACKS3HI,
    35.0,
    None,
    [
        h!(11, 180.0, 20.0, 0.0, 0.0, 361, 100, 0, 10, 7.0, 12.0),
        h!(11, 230.0, 80.0, 0.0, 0.0, 361, 100, 0, 10, 7.0, 12.0),
    ]
);
// joints ATTACKS3HI: [25, 26]

mv!(
    ATTACKS3,
    35.0,
    None,
    [
        h!(10, 180.0, 20.0, 0.0, 0.0, 361, 100, 0, 10, 7.0, 12.0),
        h!(10, 230.0, 80.0, 0.0, 0.0, 361, 100, 0, 10, 7.0, 12.0),
    ]
);
// joints ATTACKS3: [25, 26]

mv!(
    ATTACKS3LW,
    35.0,
    None,
    [
        h!(9, 180.0, 20.0, 0.0, 0.0, 361, 100, 0, 10, 7.0, 12.0),
        h!(9, 230.0, 80.0, 0.0, 0.0, 361, 100, 0, 10, 7.0, 12.0),
    ]
);
// joints ATTACKS3LW: [25, 26]

mv!(
    ATTACKHI3,
    35.0,
    None,
    [
        h!(7, 370.0, 0.0, 0.0, 0.0, 100, 40, 0, 80, 5.0, 20.0),
        h!(7, 370.0, 0.0, 0.0, 0.0, 100, 40, 0, 80, 5.0, 20.0),
    ]
);
// joints ATTACKHI3: [10, 16]

mv!(
    ATTACKLW3,
    14.0,
    None,
    [
        h!(3, 160.0, 20.0, 0.0, 0.0, 361, 20, 0, 2, 4.0, 9.0),
        h!(3, 200.0, 140.0, 0.0, 0.0, 361, 20, 0, 2, 4.0, 9.0),
    ]
);
// joints ATTACKLW3: [19, 20]

mv!(
    ATTACKS4,
    50.0,
    None,
    [
        h!(18, 200.0, 0.0, 280.0, 0.0, 361, 65, 0, 70, 18.0, 22.0),
        h!(18, 180.0, 0.0, 60.0, 0.0, 361, 65, 0, 70, 18.0, 22.0),
    ]
);
// joints ATTACKS4: [17, 17]

mv!(
    ATTACKHI4,
    40.0,
    None,
    [
        h!(17, 200.0, 0.0, 0.0, 0.0, 110, 100, 0, 20, 13.0, 18.0),
        h!(15, 200.0, 0.0, 0.0, 0.0, 78, 100, 0, 0, 18.0, 21.0),
        h!(13, 200.0, 0.0, 0.0, 0.0, 361, 100, 0, 0, 21.0, 26.0),
    ]
);
// joints ATTACKHI4: [30, 30, 30]

mv!(
    ATTACKLW4,
    55.0,
    None,
    [h!(19, 200.0, 0.0, 0.0, 0.0, 361, 100, 0, 0, 13.0, 53.0),]
);
// joints ATTACKLW4: [30]

mv!(
    ATTACKAIRN,
    40.0,
    Some(50),
    [
        h!(14, 240.0, 10.0, 0.0, 0.0, 361, 100, 0, 15, 5.0, 13.0),
        h!(14, 240.0, 10.0, 0.0, 0.0, 361, 100, 0, 15, 5.0, 13.0),
        h!(14, 260.0, 0.0, 0.0, 0.0, 361, 100, 0, 15, 5.0, 13.0),
        h!(11, 240.0, 10.0, 0.0, 0.0, 361, 100, 0, 0, 13.0, 43.0),
        h!(11, 240.0, 10.0, 0.0, 0.0, 361, 100, 0, 0, 13.0, 43.0),
        h!(11, 260.0, 0.0, 0.0, 0.0, 361, 100, 0, 0, 13.0, 43.0),
    ]
);
// joints ATTACKAIRN: [26, 20, 5, 26, 20, 5]

mv!(
    ATTACKAIRF,
    42.0,
    None,
    [
        h!(12, 310.0, 0.0, 140.0, 180.0, 361, 100, 0, 16, 10.0, 14.0),
        h!(10, 280.0, 0.0, 140.0, 180.0, 361, 100, 0, 0, 14.0, 27.0),
    ]
);
// joints ATTACKAIRF: [0, 0]

mv!(
    ATTACKAIRB,
    40.0,
    None,
    [
        h!(16, 240.0, -30.0, 45.0, 0.0, 361, 100, 0, 10, 10.0, 14.0),
        h!(16, 290.0, 80.0, 30.0, 0.0, 361, 100, 0, 10, 10.0, 14.0),
        h!(10, 220.0, -30.0, 45.0, 0.0, 361, 100, 0, 0, 14.0, 20.0),
        h!(10, 270.0, 80.0, 30.0, 0.0, 361, 100, 0, 0, 14.0, 20.0),
    ]
);
// joints ATTACKAIRB: [26, 26, 26, 26]

mv!(
    ATTACKAIRHI,
    42.0,
    None,
    [h!(15, 350.0, 0.0, 100.0, 0.0, 85, 110, 0, 10, 8.0, 17.0),]
);
// joints ATTACKAIRHI: [12]

mv!(
    ATTACKAIRLW,
    30.0,
    None,
    [
        h!(15, 320.0, 0.0, -30.0, 30.0, -90, 120, 0, 10, 4.0, 8.0),
        h!(15, 300.0, 0.0, -30.0, 30.0, -90, 120, 0, 0, 8.0, 21.0),
    ]
);
// joints ATTACKAIRLW: [0, 0]

mv!(
    THROWF,
    45.0,
    None,
    [h!(10, 300.0, 0.0, 0.0, 0.0, 361, 80, 0, 10, 3.0, 27.0),]
);
// joints THROWF: [30]

mv!(
    THROWB,
    45.0,
    None,
    [h!(10, 300.0, 0.0, 0.0, 0.0, 361, 80, 0, 10, 3.0, 27.0),]
);
// joints THROWB: [30]

mv!(
    PKJIBAKU,
    28.0,
    None,
    [h!(30, 300.0, 0.0, 100.0, 0.0, 361, 84, 0, 40, 0.0, 19.0),]
);
// joints PKJIBAKU: [0]

pub fn joints(s: AnyStatus, index: usize) -> u8 {
    let joints: &[u8] = match s {
        AnyStatus::Common(Status::Attack11) => &[10, 9],
        AnyStatus::Common(Status::Attack12) => &[16, 15],
        AnyStatus::Ness(N::Attack13) => &[26, 28, 26, 28],
        AnyStatus::Common(Status::AttackDash) => &[0, 0],
        AnyStatus::Common(Status::AttackS3Hi) => &[25, 26],
        AnyStatus::Common(Status::AttackS3) => &[25, 26],
        AnyStatus::Common(Status::AttackS3Lw) => &[25, 26],
        AnyStatus::Common(Status::AttackHi3) => &[10, 16],
        AnyStatus::Common(Status::AttackLw3) => &[19, 20],
        AnyStatus::Common(Status::AttackS4) => &[17, 17],
        AnyStatus::Common(Status::AttackHi4) => &[30, 30, 30],
        AnyStatus::Common(Status::AttackLw4) => &[30],
        AnyStatus::Common(Status::AttackAirN) => &[26, 20, 5, 26, 20, 5],
        AnyStatus::Common(Status::AttackAirF) => &[0, 0],
        AnyStatus::Common(Status::AttackAirB) => &[26, 26, 26, 26],
        AnyStatus::Common(Status::AttackAirHi) => &[12],
        AnyStatus::Common(Status::AttackAirLw) => &[0, 0],
        AnyStatus::Common(Status::ThrowF) => &[30],
        AnyStatus::Common(Status::ThrowB) => &[30],
        AnyStatus::Ness(N::SpecialHiJibaku | N::SpecialAirHiJibaku) => &[0],
        _ => &[],
    };
    joints.get(index).copied().unwrap_or(0)
}
