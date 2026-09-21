//! Fox normal-attack motion data from `relocData/208_FoxMainMotion.c`.
//! The fixed windows follow each `MakeAttackColl`, replacement, clear, and
//! refresh event in the US motion scripts.

use crate::attack::{ActiveHitbox, Hitbox, MoveData};
use ssb_engine::math::Vec3;

/// `dFoxMainMotion_ShineStart`: `MakeAttackColl(0,0,0,5,0,2,360,
/// 0,240,0,0,100,80,3,0,0,0,0)`, cleared after `Wait(2)`.
pub static FOX_REFLECTOR_START: MoveData = MoveData {
    hitboxes: &[ActiveHitbox::new(
        Hitbox {
            damage: 5,
            offset: Vec3::new(0.0, 240.0, 0.0),
            radius: 180.0,
            angle: 0,
            kb_scale: 100,
            kb_weight: 80,
            kb_base: 0,
        },
        0.0,
        2.0,
    )],
    length_frames: 4.0,
    landing_lag_percent: None,
};

/// Source: `dFoxMainMotion_Jab1`, `relocData/208_FoxMainMotion.c` (US branch).
pub static FOX_JAB1: MoveData = MoveData {
    hitboxes: &[
        ActiveHitbox::new(
            Hitbox {
                damage: 4,
                offset: Vec3::new(140.0, 0.0, 0.0),
                radius: 100.0,
                angle: 70,
                kb_scale: 100,
                kb_weight: 0,
                kb_base: 0,
            },
            3.0,
            5.0,
        )
        .with_hit_generation(0),
        ActiveHitbox::new(
            Hitbox {
                damage: 4,
                offset: Vec3::new(0.0, 0.0, 0.0),
                radius: 100.0,
                angle: 70,
                kb_scale: 100,
                kb_weight: 0,
                kb_base: 0,
            },
            3.0,
            5.0,
        )
        .with_hit_generation(0),
    ],
    length_frames: 10.0,
    landing_lag_percent: None,
};

/// Source: `dFoxMainMotion_JabLoop`, `relocData/208_FoxMainMotion.c` (US branch).
pub static FOX_JABLOOP: MoveData = MoveData {
    hitboxes: &[
        ActiveHitbox::new(
            Hitbox {
                damage: 1,
                offset: Vec3::new(20.0, 0.0, 0.0),
                radius: 100.0,
                angle: 80,
                kb_scale: 80,
                kb_weight: 0,
                kb_base: 10,
            },
            2.0,
            4.0,
        )
        .with_hit_generation(0),
        ActiveHitbox::new(
            Hitbox {
                damage: 1,
                offset: Vec3::new(220.0, 0.0, 0.0),
                radius: 100.0,
                angle: 80,
                kb_scale: 80,
                kb_weight: 0,
                kb_base: 10,
            },
            2.0,
            4.0,
        )
        .with_hit_generation(0),
        ActiveHitbox::new(
            Hitbox {
                damage: 1,
                offset: Vec3::new(20.0, 0.0, 0.0),
                radius: 100.0,
                angle: 80,
                kb_scale: 80,
                kb_weight: 0,
                kb_base: 10,
            },
            9.0,
            11.0,
        )
        .with_hit_generation(2),
        ActiveHitbox::new(
            Hitbox {
                damage: 1,
                offset: Vec3::new(220.0, 0.0, 0.0),
                radius: 100.0,
                angle: 80,
                kb_scale: 80,
                kb_weight: 0,
                kb_base: 10,
            },
            9.0,
            11.0,
        )
        .with_hit_generation(2),
        ActiveHitbox::new(
            Hitbox {
                damage: 1,
                offset: Vec3::new(20.0, 0.0, 0.0),
                radius: 100.0,
                angle: 80,
                kb_scale: 80,
                kb_weight: 0,
                kb_base: 10,
            },
            16.0,
            18.0,
        )
        .with_hit_generation(4),
        ActiveHitbox::new(
            Hitbox {
                damage: 1,
                offset: Vec3::new(220.0, 0.0, 0.0),
                radius: 100.0,
                angle: 80,
                kb_scale: 80,
                kb_weight: 0,
                kb_base: 10,
            },
            16.0,
            18.0,
        )
        .with_hit_generation(4),
        ActiveHitbox::new(
            Hitbox {
                damage: 1,
                offset: Vec3::new(20.0, 0.0, 0.0),
                radius: 100.0,
                angle: 80,
                kb_scale: 80,
                kb_weight: 0,
                kb_base: 10,
            },
            23.0,
            25.0,
        )
        .with_hit_generation(6),
        ActiveHitbox::new(
            Hitbox {
                damage: 1,
                offset: Vec3::new(220.0, 0.0, 0.0),
                radius: 100.0,
                angle: 80,
                kb_scale: 80,
                kb_weight: 0,
                kb_base: 10,
            },
            23.0,
            25.0,
        )
        .with_hit_generation(6),
        ActiveHitbox::new(
            Hitbox {
                damage: 1,
                offset: Vec3::new(20.0, 0.0, 0.0),
                radius: 100.0,
                angle: 60,
                kb_scale: 80,
                kb_weight: 0,
                kb_base: 10,
            },
            30.0,
            32.0,
        )
        .with_hit_generation(7),
        ActiveHitbox::new(
            Hitbox {
                damage: 1,
                offset: Vec3::new(220.0, 0.0, 0.0),
                radius: 100.0,
                angle: 60,
                kb_scale: 80,
                kb_weight: 0,
                kb_base: 10,
            },
            30.0,
            32.0,
        )
        .with_hit_generation(7),
    ],
    length_frames: 32.0,
    landing_lag_percent: None,
};

/// Source: `dFoxMainMotion_Jab2`, `relocData/208_FoxMainMotion.c` (US branch).
pub static FOX_JAB2: MoveData = MoveData {
    hitboxes: &[
        ActiveHitbox::new(
            Hitbox {
                damage: 4,
                offset: Vec3::new(140.0, 0.0, 0.0),
                radius: 100.0,
                angle: 70,
                kb_scale: 100,
                kb_weight: 0,
                kb_base: 0,
            },
            4.0,
            7.0,
        )
        .with_hit_generation(0),
        ActiveHitbox::new(
            Hitbox {
                damage: 4,
                offset: Vec3::new(0.0, 0.0, 0.0),
                radius: 100.0,
                angle: 70,
                kb_scale: 100,
                kb_weight: 0,
                kb_base: 0,
            },
            4.0,
            7.0,
        )
        .with_hit_generation(0),
    ],
    length_frames: 10.0,
    landing_lag_percent: None,
};

/// Source: `dFoxMainMotion_DashAttack`, `relocData/208_FoxMainMotion.c` (US branch).
pub static FOX_DASHATTACK: MoveData = MoveData {
    hitboxes: &[
        ActiveHitbox::new(
            Hitbox {
                damage: 10,
                offset: Vec3::new(140.0, 0.0, 0.0),
                radius: 115.0,
                angle: 361,
                kb_scale: 90,
                kb_weight: 0,
                kb_base: 10,
            },
            4.0,
            8.0,
        )
        .with_hit_generation(0),
        ActiveHitbox::new(
            Hitbox {
                damage: 10,
                offset: Vec3::new(0.0, 0.0, 0.0),
                radius: 115.0,
                angle: 361,
                kb_scale: 90,
                kb_weight: 0,
                kb_base: 10,
            },
            4.0,
            8.0,
        )
        .with_hit_generation(0),
        ActiveHitbox::new(
            Hitbox {
                damage: 7,
                offset: Vec3::new(140.0, 0.0, 0.0),
                radius: 115.0,
                angle: 361,
                kb_scale: 90,
                kb_weight: 0,
                kb_base: 10,
            },
            8.0,
            24.0,
        )
        .with_hit_generation(0),
        ActiveHitbox::new(
            Hitbox {
                damage: 7,
                offset: Vec3::new(0.0, 0.0, 0.0),
                radius: 115.0,
                angle: 361,
                kb_scale: 90,
                kb_weight: 0,
                kb_base: 10,
            },
            8.0,
            24.0,
        )
        .with_hit_generation(0),
    ],
    length_frames: 24.0,
    landing_lag_percent: None,
};

/// Source: `dFoxMainMotion_FTiltHigh`, `relocData/208_FoxMainMotion.c` (US branch).
pub static FOX_FTILTHIGH: MoveData = MoveData {
    hitboxes: &[
        ActiveHitbox::new(
            Hitbox {
                damage: 11,
                offset: Vec3::new(20.0, 0.0, 0.0),
                radius: 100.0,
                angle: 361,
                kb_scale: 100,
                kb_weight: 0,
                kb_base: 0,
            },
            6.0,
            14.0,
        )
        .with_hit_generation(0),
        ActiveHitbox::new(
            Hitbox {
                damage: 11,
                offset: Vec3::new(140.0, 0.0, 0.0),
                radius: 100.0,
                angle: 361,
                kb_scale: 100,
                kb_weight: 0,
                kb_base: 0,
            },
            6.0,
            14.0,
        )
        .with_hit_generation(0),
    ],
    length_frames: 14.0,
    landing_lag_percent: None,
};

/// Source: `dFoxMainMotion_FTiltMidHigh`, `relocData/208_FoxMainMotion.c` (US branch).
pub static FOX_FTILTMIDHIGH: MoveData = MoveData {
    hitboxes: &[
        ActiveHitbox::new(
            Hitbox {
                damage: 10,
                offset: Vec3::new(20.0, 0.0, 0.0),
                radius: 100.0,
                angle: 361,
                kb_scale: 100,
                kb_weight: 0,
                kb_base: 0,
            },
            6.0,
            14.0,
        )
        .with_hit_generation(0),
        ActiveHitbox::new(
            Hitbox {
                damage: 10,
                offset: Vec3::new(140.0, 0.0, 0.0),
                radius: 100.0,
                angle: 361,
                kb_scale: 100,
                kb_weight: 0,
                kb_base: 0,
            },
            6.0,
            14.0,
        )
        .with_hit_generation(0),
    ],
    length_frames: 14.0,
    landing_lag_percent: None,
};

/// Source: `dFoxMainMotion_FTilt`, `relocData/208_FoxMainMotion.c` (US branch).
pub static FOX_FTILT: MoveData = MoveData {
    hitboxes: &[
        ActiveHitbox::new(
            Hitbox {
                damage: 9,
                offset: Vec3::new(20.0, 0.0, 0.0),
                radius: 100.0,
                angle: 361,
                kb_scale: 100,
                kb_weight: 0,
                kb_base: 0,
            },
            6.0,
            14.0,
        )
        .with_hit_generation(0),
        ActiveHitbox::new(
            Hitbox {
                damage: 9,
                offset: Vec3::new(140.0, 0.0, 0.0),
                radius: 100.0,
                angle: 361,
                kb_scale: 100,
                kb_weight: 0,
                kb_base: 0,
            },
            6.0,
            14.0,
        )
        .with_hit_generation(0),
    ],
    length_frames: 14.0,
    landing_lag_percent: None,
};

/// Source: `dFoxMainMotion_FTiltMidLow`, `relocData/208_FoxMainMotion.c` (US branch).
pub static FOX_FTILTMIDLOW: MoveData = MoveData {
    hitboxes: &[
        ActiveHitbox::new(
            Hitbox {
                damage: 8,
                offset: Vec3::new(20.0, 0.0, 0.0),
                radius: 100.0,
                angle: 361,
                kb_scale: 100,
                kb_weight: 0,
                kb_base: 0,
            },
            6.0,
            14.0,
        )
        .with_hit_generation(0),
        ActiveHitbox::new(
            Hitbox {
                damage: 8,
                offset: Vec3::new(140.0, 0.0, 0.0),
                radius: 100.0,
                angle: 361,
                kb_scale: 100,
                kb_weight: 0,
                kb_base: 0,
            },
            6.0,
            14.0,
        )
        .with_hit_generation(0),
    ],
    length_frames: 14.0,
    landing_lag_percent: None,
};

/// Source: `dFoxMainMotion_FTiltLow`, `relocData/208_FoxMainMotion.c` (US branch).
pub static FOX_FTILTLOW: MoveData = MoveData {
    hitboxes: &[
        ActiveHitbox::new(
            Hitbox {
                damage: 8,
                offset: Vec3::new(20.0, 0.0, 0.0),
                radius: 100.0,
                angle: 361,
                kb_scale: 100,
                kb_weight: 10,
                kb_base: 0,
            },
            6.0,
            14.0,
        )
        .with_hit_generation(0),
        ActiveHitbox::new(
            Hitbox {
                damage: 8,
                offset: Vec3::new(140.0, 0.0, 0.0),
                radius: 100.0,
                angle: 361,
                kb_scale: 100,
                kb_weight: 10,
                kb_base: 0,
            },
            6.0,
            14.0,
        )
        .with_hit_generation(0),
    ],
    length_frames: 14.0,
    landing_lag_percent: None,
};

/// Source: `dFoxMainMotion_UTilt`, `relocData/208_FoxMainMotion.c` (US branch).
pub static FOX_UTILT: MoveData = MoveData {
    hitboxes: &[
        ActiveHitbox::new(
            Hitbox {
                damage: 9,
                offset: Vec3::new(20.0, 0.0, 0.0),
                radius: 110.0,
                angle: 80,
                kb_scale: 150,
                kb_weight: 0,
                kb_base: 0,
            },
            6.0,
            14.0,
        )
        .with_hit_generation(0),
        ActiveHitbox::new(
            Hitbox {
                damage: 9,
                offset: Vec3::new(80.0, 0.0, 0.0),
                radius: 175.0,
                angle: 80,
                kb_scale: 150,
                kb_weight: 0,
                kb_base: 0,
            },
            6.0,
            14.0,
        )
        .with_hit_generation(0),
    ],
    length_frames: 14.0,
    landing_lag_percent: None,
};

/// Source: `dFoxMainMotion_DTilt`, `relocData/208_FoxMainMotion.c` (US branch).
pub static FOX_DTILT: MoveData = MoveData {
    hitboxes: &[
        ActiveHitbox::new(
            Hitbox {
                damage: 12,
                offset: Vec3::new(0.0, 0.0, -60.0),
                radius: 115.0,
                angle: 70,
                kb_scale: 125,
                kb_weight: 0,
                kb_base: 25,
            },
            6.0,
            10.0,
        )
        .with_hit_generation(0),
        ActiveHitbox::new(
            Hitbox {
                damage: 12,
                offset: Vec3::new(0.0, 0.0, -200.0),
                radius: 115.0,
                angle: 90,
                kb_scale: 125,
                kb_weight: 0,
                kb_base: 25,
            },
            6.0,
            10.0,
        )
        .with_hit_generation(0),
    ],
    length_frames: 16.0,
    landing_lag_percent: None,
};

/// Source: `dFoxMainMotion_FSmash`, `relocData/208_FoxMainMotion.c` (US branch).
pub static FOX_FSMASH: MoveData = MoveData {
    hitboxes: &[
        ActiveHitbox::new(
            Hitbox {
                damage: 17,
                offset: Vec3::new(60.0, 0.0, 0.0),
                radius: 140.0,
                angle: 361,
                kb_scale: 120,
                kb_weight: 0,
                kb_base: 0,
            },
            12.0,
            17.0,
        )
        .with_hit_generation(0),
        ActiveHitbox::new(
            Hitbox {
                damage: 13,
                offset: Vec3::new(60.0, 0.0, 0.0),
                radius: 140.0,
                angle: 361,
                kb_scale: 120,
                kb_weight: 0,
                kb_base: 0,
            },
            17.0,
            25.0,
        )
        .with_hit_generation(0),
    ],
    length_frames: 45.0,
    landing_lag_percent: None,
};

/// Source: `dFoxMainMotion_USmash`, `relocData/208_FoxMainMotion.c` (US branch).
pub static FOX_USMASH: MoveData = MoveData {
    hitboxes: &[
        ActiveHitbox::new(
            Hitbox {
                damage: 16,
                offset: Vec3::new(0.0, 0.0, 0.0),
                radius: 100.0,
                angle: 80,
                kb_scale: 140,
                kb_weight: 0,
                kb_base: 25,
            },
            6.0,
            10.0,
        )
        .with_hit_generation(0),
        ActiveHitbox::new(
            Hitbox {
                damage: 16,
                offset: Vec3::new(70.0, 0.0, 0.0),
                radius: 140.0,
                angle: 80,
                kb_scale: 140,
                kb_weight: 0,
                kb_base: 25,
            },
            6.0,
            10.0,
        )
        .with_hit_generation(0),
        ActiveHitbox::new(
            Hitbox {
                damage: 8,
                offset: Vec3::new(0.0, 0.0, 0.0),
                radius: 100.0,
                angle: 361,
                kb_scale: 100,
                kb_weight: 0,
                kb_base: 10,
            },
            10.0,
            22.0,
        )
        .with_hit_generation(0),
        ActiveHitbox::new(
            Hitbox {
                damage: 8,
                offset: Vec3::new(70.0, 0.0, 0.0),
                radius: 115.0,
                angle: 361,
                kb_scale: 100,
                kb_weight: 0,
                kb_base: 10,
            },
            10.0,
            22.0,
        )
        .with_hit_generation(0),
    ],
    length_frames: 30.0,
    landing_lag_percent: None,
};

/// Source: `dFoxMainMotion_DSmash`, `relocData/208_FoxMainMotion.c` (US branch).
pub static FOX_DSMASH: MoveData = MoveData {
    hitboxes: &[
        ActiveHitbox::new(
            Hitbox {
                damage: 14,
                offset: Vec3::new(90.0, 0.0, 0.0),
                radius: 130.0,
                angle: 25,
                kb_scale: 80,
                kb_weight: 0,
                kb_base: 35,
            },
            6.0,
            11.0,
        )
        .with_hit_generation(0),
        ActiveHitbox::new(
            Hitbox {
                damage: 14,
                offset: Vec3::new(90.0, 0.0, 0.0),
                radius: 130.0,
                angle: 25,
                kb_scale: 80,
                kb_weight: 0,
                kb_base: 35,
            },
            6.0,
            11.0,
        )
        .with_hit_generation(0),
    ],
    length_frames: 28.0,
    landing_lag_percent: None,
};

/// Source: `dFoxMainMotion_AttackAirN`, `relocData/208_FoxMainMotion.c` (US branch).
pub static FOX_ATTACKAIRN: MoveData = MoveData {
    hitboxes: &[
        ActiveHitbox::new(
            Hitbox {
                damage: 14,
                offset: Vec3::new(0.0, 0.0, 0.0),
                radius: 105.0,
                angle: 361,
                kb_scale: 100,
                kb_weight: 0,
                kb_base: 0,
            },
            4.0,
            8.0,
        )
        .with_hit_generation(0),
        ActiveHitbox::new(
            Hitbox {
                damage: 14,
                offset: Vec3::new(70.0, 0.0, 0.0),
                radius: 105.0,
                angle: 361,
                kb_scale: 100,
                kb_weight: 0,
                kb_base: 0,
            },
            4.0,
            8.0,
        )
        .with_hit_generation(0),
        ActiveHitbox::new(
            Hitbox {
                damage: 14,
                offset: Vec3::new(70.0, 0.0, 0.0),
                radius: 90.0,
                angle: 361,
                kb_scale: 100,
                kb_weight: 0,
                kb_base: 0,
            },
            4.0,
            8.0,
        )
        .with_hit_generation(0),
        ActiveHitbox::new(
            Hitbox {
                damage: 9,
                offset: Vec3::new(0.0, 0.0, 0.0),
                radius: 105.0,
                angle: 361,
                kb_scale: 100,
                kb_weight: 0,
                kb_base: 0,
            },
            8.0,
            32.0,
        )
        .with_hit_generation(0),
        ActiveHitbox::new(
            Hitbox {
                damage: 9,
                offset: Vec3::new(70.0, 0.0, 0.0),
                radius: 105.0,
                angle: 361,
                kb_scale: 100,
                kb_weight: 0,
                kb_base: 0,
            },
            8.0,
            32.0,
        )
        .with_hit_generation(0),
        ActiveHitbox::new(
            Hitbox {
                damage: 9,
                offset: Vec3::new(70.0, 0.0, 0.0),
                radius: 90.0,
                angle: 361,
                kb_scale: 100,
                kb_weight: 0,
                kb_base: 0,
            },
            8.0,
            32.0,
        )
        .with_hit_generation(0),
    ],
    length_frames: 32.0,
    landing_lag_percent: Some(50),
};

/// Source: `dFoxMainMotion_AttackAirF`, `relocData/208_FoxMainMotion.c` (US branch).
pub static FOX_ATTACKAIRF: MoveData = MoveData {
    hitboxes: &[
        ActiveHitbox::new(
            Hitbox {
                damage: 12,
                offset: Vec3::new(0.0, 0.0, 0.0),
                radius: 155.0,
                angle: 361,
                kb_scale: 100,
                kb_weight: 0,
                kb_base: 0,
            },
            6.0,
            10.0,
        )
        .with_hit_generation(0),
        ActiveHitbox::new(
            Hitbox {
                damage: 12,
                offset: Vec3::new(120.0, 0.0, 0.0),
                radius: 155.0,
                angle: 361,
                kb_scale: 100,
                kb_weight: 0,
                kb_base: 0,
            },
            6.0,
            10.0,
        )
        .with_hit_generation(0),
        ActiveHitbox::new(
            Hitbox {
                damage: 8,
                offset: Vec3::new(0.0, 0.0, 0.0),
                radius: 140.0,
                angle: 361,
                kb_scale: 100,
                kb_weight: 0,
                kb_base: 0,
            },
            10.0,
            24.0,
        )
        .with_hit_generation(0),
        ActiveHitbox::new(
            Hitbox {
                damage: 8,
                offset: Vec3::new(120.0, 0.0, 0.0),
                radius: 140.0,
                angle: 361,
                kb_scale: 100,
                kb_weight: 0,
                kb_base: 0,
            },
            10.0,
            24.0,
        )
        .with_hit_generation(0),
    ],
    length_frames: 24.0,
    landing_lag_percent: None,
};

/// Source: `dFoxMainMotion_AttackAirB`, `relocData/208_FoxMainMotion.c` (US branch).
pub static FOX_ATTACKAIRB: MoveData = MoveData {
    hitboxes: &[
        ActiveHitbox::new(
            Hitbox {
                damage: 12,
                offset: Vec3::new(0.0, 0.0, 0.0),
                radius: 110.0,
                angle: 361,
                kb_scale: 100,
                kb_weight: 0,
                kb_base: 0,
            },
            4.0,
            8.0,
        )
        .with_hit_generation(0),
        ActiveHitbox::new(
            Hitbox {
                damage: 12,
                offset: Vec3::new(120.0, 0.0, 0.0),
                radius: 150.0,
                angle: 361,
                kb_scale: 100,
                kb_weight: 0,
                kb_base: 0,
            },
            4.0,
            8.0,
        )
        .with_hit_generation(0),
        ActiveHitbox::new(
            Hitbox {
                damage: 8,
                offset: Vec3::new(60.0, 0.0, 0.0),
                radius: 100.0,
                angle: 361,
                kb_scale: 100,
                kb_weight: 0,
                kb_base: 0,
            },
            4.0,
            8.0,
        )
        .with_hit_generation(0),
        ActiveHitbox::new(
            Hitbox {
                damage: 8,
                offset: Vec3::new(0.0, 0.0, 0.0),
                radius: 100.0,
                angle: 361,
                kb_scale: 100,
                kb_weight: 0,
                kb_base: 0,
            },
            8.0,
            28.0,
        )
        .with_hit_generation(0),
        ActiveHitbox::new(
            Hitbox {
                damage: 8,
                offset: Vec3::new(120.0, 0.0, 0.0),
                radius: 120.0,
                angle: 361,
                kb_scale: 100,
                kb_weight: 0,
                kb_base: 0,
            },
            8.0,
            28.0,
        )
        .with_hit_generation(0),
        ActiveHitbox::new(
            Hitbox {
                damage: 8,
                offset: Vec3::new(60.0, 0.0, 0.0),
                radius: 100.0,
                angle: 361,
                kb_scale: 100,
                kb_weight: 0,
                kb_base: 0,
            },
            8.0,
            28.0,
        )
        .with_hit_generation(0),
    ],
    length_frames: 28.0,
    landing_lag_percent: None,
};

/// Source: `dFoxMainMotion_AttackAirU`, `relocData/208_FoxMainMotion.c` (US branch).
pub static FOX_ATTACKAIRU: MoveData = MoveData {
    hitboxes: &[
        ActiveHitbox::new(
            Hitbox {
                damage: 2,
                offset: Vec3::new(0.0, 0.0, 0.0),
                radius: 160.0,
                angle: 90,
                kb_scale: 100,
                kb_weight: 100,
                kb_base: 0,
            },
            6.0,
            12.0,
        )
        .with_hit_generation(0),
        ActiveHitbox::new(
            Hitbox {
                damage: 2,
                offset: Vec3::new(110.0, 0.0, 0.0),
                radius: 170.0,
                angle: 90,
                kb_scale: 100,
                kb_weight: 100,
                kb_base: 0,
            },
            6.0,
            12.0,
        )
        .with_hit_generation(0),
        ActiveHitbox::new(
            Hitbox {
                damage: 13,
                offset: Vec3::new(0.0, 0.0, 0.0),
                radius: 170.0,
                angle: 90,
                kb_scale: 135,
                kb_weight: 0,
                kb_base: 0,
            },
            12.0,
            14.0,
        )
        .with_hit_generation(0),
        ActiveHitbox::new(
            Hitbox {
                damage: 13,
                offset: Vec3::new(110.0, 0.0, 0.0),
                radius: 185.0,
                angle: 90,
                kb_scale: 135,
                kb_weight: 0,
                kb_base: 0,
            },
            12.0,
            14.0,
        )
        .with_hit_generation(0),
    ],
    length_frames: 14.0,
    landing_lag_percent: Some(20),
};

/// Source: `dFoxMainMotion_AttackAirD`, `relocData/208_FoxMainMotion.c` (US branch).
pub static FOX_ATTACKAIRD: MoveData = MoveData {
    hitboxes: &[
        ActiveHitbox::new(
            Hitbox {
                damage: 2,
                offset: Vec3::new(-40.0, 45.0, 0.0),
                radius: 155.0,
                angle: 954,
                kb_scale: 100,
                kb_weight: 30,
                kb_base: 0,
            },
            4.0,
            6.0,
        )
        .with_hit_generation(0),
        ActiveHitbox::new(
            Hitbox {
                damage: 2,
                offset: Vec3::new(30.0, 30.0, 0.0),
                radius: 180.0,
                angle: 954,
                kb_scale: 100,
                kb_weight: 30,
                kb_base: 0,
            },
            4.0,
            6.0,
        )
        .with_hit_generation(0),
        ActiveHitbox::new(
            Hitbox {
                damage: 2,
                offset: Vec3::new(-40.0, 45.0, 0.0),
                radius: 155.0,
                angle: 954,
                kb_scale: 100,
                kb_weight: 30,
                kb_base: 0,
            },
            7.0,
            9.0,
        )
        .with_hit_generation(1),
        ActiveHitbox::new(
            Hitbox {
                damage: 2,
                offset: Vec3::new(30.0, 30.0, 0.0),
                radius: 180.0,
                angle: 954,
                kb_scale: 100,
                kb_weight: 30,
                kb_base: 0,
            },
            7.0,
            9.0,
        )
        .with_hit_generation(1),
        ActiveHitbox::new(
            Hitbox {
                damage: 2,
                offset: Vec3::new(-40.0, 45.0, 0.0),
                radius: 155.0,
                angle: 954,
                kb_scale: 100,
                kb_weight: 30,
                kb_base: 0,
            },
            10.0,
            12.0,
        )
        .with_hit_generation(2),
        ActiveHitbox::new(
            Hitbox {
                damage: 2,
                offset: Vec3::new(30.0, 30.0, 0.0),
                radius: 180.0,
                angle: 954,
                kb_scale: 100,
                kb_weight: 30,
                kb_base: 0,
            },
            10.0,
            12.0,
        )
        .with_hit_generation(2),
        ActiveHitbox::new(
            Hitbox {
                damage: 2,
                offset: Vec3::new(-40.0, 45.0, 0.0),
                radius: 155.0,
                angle: 954,
                kb_scale: 100,
                kb_weight: 30,
                kb_base: 0,
            },
            13.0,
            15.0,
        )
        .with_hit_generation(3),
        ActiveHitbox::new(
            Hitbox {
                damage: 2,
                offset: Vec3::new(30.0, 30.0, 0.0),
                radius: 180.0,
                angle: 954,
                kb_scale: 100,
                kb_weight: 30,
                kb_base: 0,
            },
            13.0,
            15.0,
        )
        .with_hit_generation(3),
        ActiveHitbox::new(
            Hitbox {
                damage: 2,
                offset: Vec3::new(-40.0, 45.0, 0.0),
                radius: 155.0,
                angle: 954,
                kb_scale: 100,
                kb_weight: 30,
                kb_base: 0,
            },
            16.0,
            18.0,
        )
        .with_hit_generation(4),
        ActiveHitbox::new(
            Hitbox {
                damage: 2,
                offset: Vec3::new(30.0, 30.0, 0.0),
                radius: 180.0,
                angle: 954,
                kb_scale: 100,
                kb_weight: 30,
                kb_base: 0,
            },
            16.0,
            18.0,
        )
        .with_hit_generation(4),
        ActiveHitbox::new(
            Hitbox {
                damage: 2,
                offset: Vec3::new(-40.0, 45.0, 0.0),
                radius: 155.0,
                angle: 954,
                kb_scale: 100,
                kb_weight: 30,
                kb_base: 0,
            },
            19.0,
            21.0,
        )
        .with_hit_generation(5),
        ActiveHitbox::new(
            Hitbox {
                damage: 2,
                offset: Vec3::new(30.0, 30.0, 0.0),
                radius: 180.0,
                angle: 954,
                kb_scale: 100,
                kb_weight: 30,
                kb_base: 0,
            },
            19.0,
            21.0,
        )
        .with_hit_generation(5),
        ActiveHitbox::new(
            Hitbox {
                damage: 2,
                offset: Vec3::new(-40.0, 45.0, 0.0),
                radius: 155.0,
                angle: 954,
                kb_scale: 100,
                kb_weight: 30,
                kb_base: 0,
            },
            22.0,
            24.0,
        )
        .with_hit_generation(6),
        ActiveHitbox::new(
            Hitbox {
                damage: 2,
                offset: Vec3::new(30.0, 30.0, 0.0),
                radius: 180.0,
                angle: 954,
                kb_scale: 100,
                kb_weight: 30,
                kb_base: 0,
            },
            22.0,
            24.0,
        )
        .with_hit_generation(6),
    ],
    length_frames: 24.0,
    landing_lag_percent: Some(20),
};
