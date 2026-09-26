//! Jigglypuff's US motion collisions (`232_PurinMainMotion.c`) and Kirby's
//! Pound (`228_KirbyMainMotion.c`), transcribed with
//! `tools/extract_purin_moves.py`. Windows are half open; a
//! `SetAttackCollDamage` or `SetAttackCollSize` keeps the hit generation,
//! and a `ClearAttackCollAll` or `RefreshAttackCollID` starts a new one.

use crate::attack::{ActiveHitbox, Hitbox, MoveData};
use crate::status::{AnyStatus, PurinStatus as P, Status};
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

/// `Jab1`: `WaitAsync(10)`, then `SetFlag1(1)`. `Jab2` sets none.
pub const JAB1_FLAG1_FRAME: f32 = 10.0;

mv!(
    ATTACK11,
    18.0,
    None,
    [
        h!(3, 210.0, -50.0, 0.0, 0.0, 361, 50, 0, 8, 5.0, 7.0),
        h!(3, 210.0, 70.0, 0.0, 0.0, 361, 50, 0, 8, 5.0, 7.0),
    ]
);
mv!(
    ATTACK12,
    20.0,
    None,
    [
        h!(4, 210.0, -50.0, 0.0, 0.0, 70, 50, 0, 8, 5.0, 8.0),
        h!(4, 210.0, 70.0, 0.0, 0.0, 70, 50, 0, 8, 5.0, 8.0),
    ]
);
mv!(
    ATTACKDASH,
    40.0,
    None,
    [
        h!(10, 260.0, 0.0, 150.0, 200.0, 361, 100, 0, 10, 4.0, 8.0),
        h!(8, 260.0, 0.0, 150.0, 200.0, 361, 100, 0, 10, 8.0, 18.0),
    ]
);
mv!(
    ATTACKS3HI,
    28.0,
    None,
    [
        h!(8, 250.0, 0.0, 0.0, 0.0, 361, 100, 0, 5, 7.0, 11.0),
        h!(8, 250.0, 150.0, 0.0, 0.0, 361, 100, 0, 5, 7.0, 11.0),
    ]
);
mv!(
    ATTACKS3,
    28.0,
    None,
    [
        h!(8, 200.0, 0.0, 0.0, 0.0, 361, 100, 0, 5, 7.0, 11.0),
        h!(8, 200.0, 150.0, 0.0, 0.0, 361, 100, 0, 5, 7.0, 11.0),
    ]
);
mv!(
    ATTACKS3LW,
    28.0,
    None,
    [
        h!(8, 250.0, 0.0, 0.0, 0.0, 361, 100, 0, 5, 7.0, 11.0),
        h!(8, 250.0, 150.0, 0.0, 0.0, 361, 100, 0, 5, 7.0, 11.0),
    ]
);
mv!(
    ATTACKHI3,
    24.0,
    None,
    [
        h!(10, 280.0, 0.0, 0.0, 0.0, 85, 100, 0, 10, 7.0, 15.0),
        h!(10, 320.0, 140.0, 0.0, 0.0, 85, 100, 0, 10, 7.0, 15.0),
    ]
);
mv!(
    ATTACKLW3,
    40.0,
    None,
    [
        h!(10, 200.0, 0.0, 0.0, 0.0, 70, 120, 0, 0, 11.0, 16.0),
        h!(10, 200.0, 140.0, 0.0, 0.0, 70, 120, 0, 0, 11.0, 16.0),
    ]
);
mv!(
    ATTACKS4,
    40.0,
    None,
    [
        h!(16, 280.0, 0.0, 0.0, 0.0, 361, 120, 0, 10, 12.0, 15.0),
        h!(16, 280.0, 140.0, 0.0, 0.0, 361, 120, 0, 10, 12.0, 15.0),
        h!(10, 280.0, 0.0, 0.0, 0.0, 361, 120, 0, 10, 15.0, 20.0),
        h!(10, 280.0, 140.0, 0.0, 0.0, 361, 120, 0, 10, 15.0, 20.0),
    ]
);
mv!(
    ATTACKHI4,
    55.0,
    None,
    [
        h!(18, 380.0, 0.0, 180.0, 0.0, 80, 130, 0, 22, 8.0, 12.0),
        h!(18, 280.0, 0.0, 0.0, 0.0, 80, 130, 0, 22, 8.0, 12.0),
        h!(10, 380.0, 0.0, 180.0, 0.0, 361, 100, 0, 10, 12.0, 15.0),
        h!(10, 260.0, 0.0, 0.0, 0.0, 361, 100, 0, 10, 12.0, 15.0),
    ]
);
mv!(
    ATTACKLW4,
    55.0,
    None,
    [
        h!(16, 260.0, 200.0, 0.0, 0.0, 40, 100, 0, 20, 7.0, 11.0),
        h!(16, 260.0, 200.0, 0.0, 0.0, 40, 100, 0, 20, 7.0, 11.0),
        h!(16, 160.0, 100.0, 0.0, 0.0, 40, 100, 0, 20, 7.0, 11.0),
        h!(10, 260.0, 200.0, 0.0, 0.0, 40, 100, 0, 20, 11.0, 27.0),
        h!(10, 260.0, 200.0, 0.0, 0.0, 40, 100, 0, 20, 11.0, 27.0),
        h!(10, 160.0, 100.0, 0.0, 0.0, 40, 100, 0, 20, 11.0, 27.0),
    ]
);
mv!(
    ATTACKAIRN,
    50.0,
    Some(50),
    [
        h!(14, 220.0, 0.0, 0.0, 0.0, 361, 100, 0, 30, 6.0, 10.0),
        h!(14, 260.0, 120.0, 0.0, 0.0, 361, 100, 0, 30, 6.0, 10.0),
        h!(9, 210.0, 0.0, 0.0, 0.0, 361, 100, 0, 10, 10.0, 34.0),
        h!(9, 260.0, 120.0, 0.0, 0.0, 361, 100, 0, 10, 10.0, 34.0),
    ]
);
mv!(
    ATTACKAIRF,
    40.0,
    None,
    [
        h!(13, 220.0, 0.0, 150.0, 220.0, 361, 100, 0, 10, 8.0, 12.0),
        h!(13, 270.0, 0.0, 150.0, 100.0, 361, 100, 0, 10, 8.0, 12.0),
        h!(9, 220.0, 0.0, 150.0, 220.0, 361, 100, 0, 0, 12.0, 24.0),
        h!(9, 270.0, 0.0, 150.0, 100.0, 361, 100, 0, 0, 12.0, 24.0),
    ]
);
mv!(
    ATTACKAIRB,
    40.0,
    None,
    [
        h!(13, 220.0, -30.0, 45.0, 0.0, 361, 100, 0, 10, 8.0, 12.0),
        h!(13, 270.0, 150.0, 30.0, 0.0, 361, 100, 0, 10, 8.0, 12.0),
        h!(9, 220.0, -30.0, 45.0, 0.0, 361, 100, 0, 0, 12.0, 22.0),
        h!(9, 270.0, 150.0, 30.0, 0.0, 361, 100, 0, 0, 12.0, 22.0),
    ]
);
mv!(
    ATTACKAIRHI,
    40.0,
    Some(40),
    [h!(16, 230.0, 0.0, 0.0, 0.0, 70, 120, 0, 0, 8.0, 17.0),]
);
mv!(
    ATTACKAIRLW,
    50.0,
    Some(50),
    [
        h!(3, 360.0, 0.0, -120.0, 100.0, -90, 100, 30, 0, 4.0, 6.0),
        h!(3, 360.0, 0.0, 0.0, 60.0, -90, 100, 30, 0, 4.0, 6.0),
        h!(3, 360.0, 0.0, -120.0, 100.0, -90, 100, 30, 0, 7.0, 9.0).with_hit_generation(1),
        h!(3, 360.0, 0.0, 0.0, 60.0, -90, 100, 30, 0, 7.0, 9.0).with_hit_generation(1),
        h!(3, 360.0, 0.0, -120.0, 100.0, -90, 100, 30, 0, 10.0, 12.0).with_hit_generation(2),
        h!(3, 360.0, 0.0, 0.0, 60.0, -90, 100, 30, 0, 10.0, 12.0).with_hit_generation(2),
        h!(3, 360.0, 0.0, -120.0, 100.0, -90, 100, 30, 0, 13.0, 15.0).with_hit_generation(3),
        h!(3, 360.0, 0.0, 0.0, 60.0, -90, 100, 30, 0, 13.0, 15.0).with_hit_generation(3),
        h!(3, 360.0, 0.0, -120.0, 100.0, -90, 100, 30, 0, 16.0, 18.0).with_hit_generation(4),
        h!(3, 360.0, 0.0, 0.0, 60.0, -90, 100, 30, 0, 16.0, 18.0).with_hit_generation(4),
        h!(3, 360.0, 0.0, -120.0, 100.0, -90, 100, 30, 0, 19.0, 21.0).with_hit_generation(5),
        h!(3, 360.0, 0.0, 0.0, 60.0, -90, 100, 30, 0, 19.0, 21.0).with_hit_generation(5),
        h!(3, 360.0, 0.0, -120.0, 100.0, -90, 100, 30, 0, 22.0, 24.0).with_hit_generation(6),
        h!(3, 360.0, 0.0, 0.0, 60.0, -90, 100, 30, 0, 22.0, 24.0).with_hit_generation(6),
        h!(3, 360.0, 0.0, -120.0, 100.0, -90, 100, 30, 0, 25.0, 27.0).with_hit_generation(7),
        h!(3, 360.0, 0.0, 0.0, 60.0, -90, 100, 30, 0, 25.0, 27.0).with_hit_generation(7),
        h!(3, 360.0, 0.0, -120.0, 100.0, -90, 100, 30, 0, 28.0, 30.0).with_hit_generation(8),
        h!(3, 360.0, 0.0, 0.0, 60.0, -90, 100, 30, 0, 28.0, 30.0).with_hit_generation(8),
        h!(3, 360.0, 0.0, -120.0, 100.0, -90, 100, 30, 0, 31.0, 33.0).with_hit_generation(9),
        h!(3, 360.0, 0.0, 0.0, 60.0, -90, 100, 30, 0, 31.0, 33.0).with_hit_generation(9),
    ]
);
mv!(
    POUND,
    55.0,
    None,
    [
        h!(13, 310.0, -50.0, 0.0, 0.0, 120, 75, 0, 20, 12.0, 28.0),
        h!(13, 310.0, 70.0, 0.0, 0.0, 120, 75, 0, 20, 12.0, 28.0),
    ]
);
mv!(
    SING,
    180.0,
    None,
    [
        h!(0, 600.0, 0.0, 70.0, 0.0, 361, 100, 0, 0, 28.0, 36.0)
            .sleep()
            .ground_only(),
        h!(0, 60.0, 0.0, 70.0, 0.0, 361, 100, 0, 0, 36.0, 69.0)
            .sleep()
            .ground_only(),
        h!(0, 600.0, 0.0, 70.0, 0.0, 361, 100, 0, 0, 69.0, 77.0)
            .sleep()
            .ground_only(),
        h!(0, 60.0, 0.0, 70.0, 0.0, 361, 100, 0, 0, 77.0, 113.0)
            .sleep()
            .ground_only(),
        h!(0, 760.0, 0.0, 70.0, 0.0, 361, 100, 0, 0, 113.0, 126.0)
            .sleep()
            .ground_only(),
    ]
);
mv!(
    REST,
    250.0,
    None,
    [h!(20, 260.0, 0.0, 150.0, 0.0, 361, 120, 0, 60, 1.0, 2.0),]
);
mv!(
    COPY_POUND,
    55.0,
    None,
    [
        h!(13, 310.0, -50.0, 0.0, 0.0, 120, 75, 0, 20, 12.0, 28.0),
        h!(13, 310.0, 70.0, 0.0, 0.0, 120, 75, 0, 20, 12.0, 28.0),
    ]
);

/// The `jid` of each box, in [`MoveData`] order.
pub fn joints(s: AnyStatus, index: usize) -> u8 {
    let ids: &[u8] = match s {
        AnyStatus::Common(Status::Attack11) => &[11, 11],
        AnyStatus::Common(Status::Attack12) => &[15, 15],
        AnyStatus::Common(Status::AttackS3Hi | Status::AttackS3 | Status::AttackS3Lw) => &[23],
        AnyStatus::Common(Status::AttackHi3) => &[28, 28],
        AnyStatus::Common(Status::AttackLw3 | Status::AttackS4) => &[23],
        AnyStatus::Common(Status::AttackHi4) => &[5],
        AnyStatus::Common(Status::AttackLw4) => &[28, 23, 5, 28, 23, 5],
        AnyStatus::Common(Status::AttackAirN) => &[5, 23, 5, 23],
        AnyStatus::Common(Status::AttackAirB) => &[26],
        AnyStatus::Common(Status::AttackAirHi) => &[11],
        AnyStatus::Purin(P::SpecialN | P::SpecialAirN) => &[15],
        AnyStatus::Purin(P::SpecialHi | P::SpecialAirHi) => &[6],
        _ => &[0],
    };
    ids.get(index).copied().unwrap_or(ids[ids.len() - 1])
}
