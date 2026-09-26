//! Pikachu US motion collisions from `242_PikachuMainMotion.c`.
use crate::attack::{ActiveHitbox, Hitbox, MoveData};
use crate::status::{AnyStatus, Status};
use ssb_engine::math::Vec3;

pub static ATTACK11: MoveData = MoveData {
    hitboxes: &[
        ActiveHitbox::new(
            Hitbox {
                damage: 2,
                radius: 130.0,
                offset: Vec3::new(0.0, 65.0, 15.0),
                angle: 361,
                kb_scale: 50,
                kb_weight: 0,
                kb_base: 4,
            },
            2.0,
            6.0,
        )
        .with_hit_generation(0),
        ActiveHitbox::new(
            Hitbox {
                damage: 2,
                radius: 80.0,
                offset: Vec3::new(0.0, 0.0, 0.0),
                angle: 361,
                kb_scale: 50,
                kb_weight: 0,
                kb_base: 4,
            },
            2.0,
            6.0,
        )
        .with_hit_generation(0),
    ],
    length_frames: 34.0,
    landing_lag_percent: None,
};
pub static ATTACKDASH: MoveData = MoveData {
    hitboxes: &[
        ActiveHitbox::new(
            Hitbox {
                damage: 12,
                radius: 135.0,
                offset: Vec3::new(0.0, 70.0, 15.0),
                angle: 361,
                kb_scale: 70,
                kb_weight: 0,
                kb_base: 40,
            },
            4.0,
            19.0,
        )
        .with_hit_generation(0),
        ActiveHitbox::new(
            Hitbox {
                damage: 12,
                radius: 90.0,
                offset: Vec3::new(0.0, 20.0, 0.0),
                angle: 361,
                kb_scale: 70,
                kb_weight: 0,
                kb_base: 40,
            },
            4.0,
            19.0,
        )
        .with_hit_generation(0),
    ],
    length_frames: 57.0,
    landing_lag_percent: None,
};
pub static ATTACKS3HI: MoveData = MoveData {
    hitboxes: &[
        ActiveHitbox::new(
            Hitbox {
                damage: 11,
                radius: 140.0,
                offset: Vec3::new(0.0, 0.0, 0.0),
                angle: 361,
                kb_scale: 100,
                kb_weight: 0,
                kb_base: 8,
            },
            5.0,
            15.0,
        )
        .with_hit_generation(0),
        ActiveHitbox::new(
            Hitbox {
                damage: 11,
                radius: 140.0,
                offset: Vec3::new(0.0, 0.0, 0.0),
                angle: 361,
                kb_scale: 100,
                kb_weight: 0,
                kb_base: 8,
            },
            5.0,
            15.0,
        )
        .with_hit_generation(0),
        ActiveHitbox::new(
            Hitbox {
                damage: 11,
                radius: 90.0,
                offset: Vec3::new(0.0, 0.0, 0.0),
                angle: 361,
                kb_scale: 100,
                kb_weight: 0,
                kb_base: 8,
            },
            5.0,
            15.0,
        )
        .with_hit_generation(0),
    ],
    length_frames: 30.0,
    landing_lag_percent: None,
};
pub static ATTACKS3: MoveData = MoveData {
    hitboxes: &[
        ActiveHitbox::new(
            Hitbox {
                damage: 10,
                radius: 140.0,
                offset: Vec3::new(0.0, 0.0, 0.0),
                angle: 361,
                kb_scale: 100,
                kb_weight: 0,
                kb_base: 6,
            },
            5.0,
            15.0,
        )
        .with_hit_generation(0),
        ActiveHitbox::new(
            Hitbox {
                damage: 10,
                radius: 140.0,
                offset: Vec3::new(0.0, 0.0, 0.0),
                angle: 361,
                kb_scale: 100,
                kb_weight: 0,
                kb_base: 6,
            },
            5.0,
            15.0,
        )
        .with_hit_generation(0),
        ActiveHitbox::new(
            Hitbox {
                damage: 10,
                radius: 90.0,
                offset: Vec3::new(0.0, 0.0, 0.0),
                angle: 361,
                kb_scale: 100,
                kb_weight: 0,
                kb_base: 6,
            },
            5.0,
            15.0,
        )
        .with_hit_generation(0),
    ],
    length_frames: 30.0,
    landing_lag_percent: None,
};
pub static ATTACKS3LW: MoveData = MoveData {
    hitboxes: &[
        ActiveHitbox::new(
            Hitbox {
                damage: 9,
                radius: 140.0,
                offset: Vec3::new(0.0, 0.0, 0.0),
                angle: 361,
                kb_scale: 100,
                kb_weight: 0,
                kb_base: 6,
            },
            5.0,
            15.0,
        )
        .with_hit_generation(0),
        ActiveHitbox::new(
            Hitbox {
                damage: 9,
                radius: 140.0,
                offset: Vec3::new(0.0, 0.0, 0.0),
                angle: 361,
                kb_scale: 100,
                kb_weight: 0,
                kb_base: 6,
            },
            5.0,
            15.0,
        )
        .with_hit_generation(0),
        ActiveHitbox::new(
            Hitbox {
                damage: 9,
                radius: 90.0,
                offset: Vec3::new(0.0, 0.0, 0.0),
                angle: 361,
                kb_scale: 100,
                kb_weight: 0,
                kb_base: 6,
            },
            5.0,
            15.0,
        )
        .with_hit_generation(0),
    ],
    length_frames: 30.0,
    landing_lag_percent: None,
};
pub static ATTACKHI3: MoveData = MoveData {
    hitboxes: &[
        ActiveHitbox::new(
            Hitbox {
                damage: 11,
                radius: 80.0,
                offset: Vec3::new(0.0, 0.0, 0.0),
                angle: 87,
                kb_scale: 100,
                kb_weight: 0,
                kb_base: 10,
            },
            5.0,
            15.0,
        )
        .with_hit_generation(0),
        ActiveHitbox::new(
            Hitbox {
                damage: 11,
                radius: 150.0,
                offset: Vec3::new(115.0, 180.0, -60.0),
                angle: 87,
                kb_scale: 100,
                kb_weight: 0,
                kb_base: 10,
            },
            5.0,
            15.0,
        )
        .with_hit_generation(0),
    ],
    length_frames: 26.0,
    landing_lag_percent: None,
};
pub static ATTACKLW3: MoveData = MoveData {
    hitboxes: &[
        ActiveHitbox::new(
            Hitbox {
                damage: 12,
                radius: 80.0,
                offset: Vec3::new(0.0, 0.0, 0.0),
                angle: 35,
                kb_scale: 100,
                kb_weight: 0,
                kb_base: 12,
            },
            6.0,
            14.0,
        )
        .with_hit_generation(0),
        ActiveHitbox::new(
            Hitbox {
                damage: 12,
                radius: 140.0,
                offset: Vec3::new(110.0, 170.0, -65.0),
                angle: 35,
                kb_scale: 100,
                kb_weight: 0,
                kb_base: 12,
            },
            6.0,
            14.0,
        )
        .with_hit_generation(0),
    ],
    length_frames: 26.0,
    landing_lag_percent: None,
};
pub static ATTACKS4: MoveData = MoveData {
    hitboxes: &[
        ActiveHitbox::new(
            Hitbox {
                damage: 18,
                radius: 80.0,
                offset: Vec3::new(0.0, 100.0, 200.0),
                angle: 361,
                kb_scale: 100,
                kb_weight: 0,
                kb_base: 20,
            },
            21.0,
            25.0,
        )
        .with_hit_generation(0),
        ActiveHitbox::new(
            Hitbox {
                damage: 18,
                radius: 120.0,
                offset: Vec3::new(0.0, 100.0, 200.0),
                angle: 361,
                kb_scale: 100,
                kb_weight: 0,
                kb_base: 20,
            },
            25.0,
            43.0,
        )
        .with_hit_generation(0),
        ActiveHitbox::new(
            Hitbox {
                damage: 18,
                radius: 130.0,
                offset: Vec3::new(0.0, 100.0, 425.0),
                angle: 361,
                kb_scale: 100,
                kb_weight: 0,
                kb_base: 20,
            },
            25.0,
            43.0,
        )
        .with_hit_generation(0),
        ActiveHitbox::new(
            Hitbox {
                damage: 18,
                radius: 160.0,
                offset: Vec3::new(0.0, 100.0, 750.0),
                angle: 361,
                kb_scale: 100,
                kb_weight: 0,
                kb_base: 20,
            },
            25.0,
            43.0,
        )
        .with_hit_generation(0),
    ],
    length_frames: 62.0,
    landing_lag_percent: None,
};
pub static ATTACKHI4: MoveData = MoveData {
    hitboxes: &[
        ActiveHitbox::new(
            Hitbox {
                damage: 18,
                radius: 80.0,
                offset: Vec3::new(0.0, 0.0, 0.0),
                angle: 95,
                kb_scale: 100,
                kb_weight: 0,
                kb_base: 40,
            },
            10.0,
            13.0,
        )
        .with_hit_generation(0),
        ActiveHitbox::new(
            Hitbox {
                damage: 18,
                radius: 155.0,
                offset: Vec3::new(115.0, 180.0, -60.0),
                angle: 95,
                kb_scale: 100,
                kb_weight: 0,
                kb_base: 40,
            },
            10.0,
            13.0,
        )
        .with_hit_generation(0),
        ActiveHitbox::new(
            Hitbox {
                damage: 9,
                radius: 100.0,
                offset: Vec3::new(0.0, 0.0, 0.0),
                angle: 90,
                kb_scale: 100,
                kb_weight: 0,
                kb_base: 10,
            },
            13.0,
            19.0,
        )
        .with_hit_generation(0),
        ActiveHitbox::new(
            Hitbox {
                damage: 9,
                radius: 115.0,
                offset: Vec3::new(115.0, 180.0, -60.0),
                angle: 90,
                kb_scale: 100,
                kb_weight: 0,
                kb_base: 10,
            },
            13.0,
            19.0,
        )
        .with_hit_generation(0),
    ],
    length_frames: 40.0,
    landing_lag_percent: None,
};
pub static ATTACKLW4: MoveData = MoveData {
    hitboxes: &[
        ActiveHitbox::new(
            Hitbox {
                damage: 16,
                radius: 135.0,
                offset: Vec3::new(100.0, 0.0, 0.0),
                angle: 361,
                kb_scale: 120,
                kb_weight: 0,
                kb_base: 30,
            },
            10.0,
            14.0,
        )
        .with_hit_generation(0),
        ActiveHitbox::new(
            Hitbox {
                damage: 16,
                radius: 135.0,
                offset: Vec3::new(100.0, 0.0, 0.0),
                angle: 361,
                kb_scale: 120,
                kb_weight: 0,
                kb_base: 30,
            },
            10.0,
            14.0,
        )
        .with_hit_generation(0),
        ActiveHitbox::new(
            Hitbox {
                damage: 16,
                radius: 90.0,
                offset: Vec3::new(0.0, 0.0, 0.0),
                angle: 361,
                kb_scale: 120,
                kb_weight: 0,
                kb_base: 30,
            },
            10.0,
            14.0,
        )
        .with_hit_generation(0),
        ActiveHitbox::new(
            Hitbox {
                damage: 13,
                radius: 135.0,
                offset: Vec3::new(100.0, 0.0, 0.0),
                angle: 361,
                kb_scale: 110,
                kb_weight: 0,
                kb_base: 30,
            },
            25.0,
            29.0,
        )
        .with_hit_generation(1),
        ActiveHitbox::new(
            Hitbox {
                damage: 13,
                radius: 135.0,
                offset: Vec3::new(100.0, 0.0, 0.0),
                angle: 361,
                kb_scale: 110,
                kb_weight: 0,
                kb_base: 30,
            },
            25.0,
            29.0,
        )
        .with_hit_generation(1),
        ActiveHitbox::new(
            Hitbox {
                damage: 13,
                radius: 90.0,
                offset: Vec3::new(0.0, 0.0, 0.0),
                angle: 361,
                kb_scale: 110,
                kb_weight: 0,
                kb_base: 30,
            },
            25.0,
            29.0,
        )
        .with_hit_generation(1),
    ],
    length_frames: 54.0,
    landing_lag_percent: None,
};
pub static ATTACKAIRN: MoveData = MoveData {
    hitboxes: &[
        ActiveHitbox::new(
            Hitbox {
                damage: 14,
                radius: 120.0,
                offset: Vec3::new(10.0, 0.0, 0.0),
                angle: 361,
                kb_scale: 100,
                kb_weight: 0,
                kb_base: 15,
            },
            3.0,
            11.0,
        )
        .with_hit_generation(0),
        ActiveHitbox::new(
            Hitbox {
                damage: 14,
                radius: 120.0,
                offset: Vec3::new(10.0, 0.0, 0.0),
                angle: 361,
                kb_scale: 100,
                kb_weight: 0,
                kb_base: 15,
            },
            3.0,
            11.0,
        )
        .with_hit_generation(0),
        ActiveHitbox::new(
            Hitbox {
                damage: 14,
                radius: 130.0,
                offset: Vec3::new(0.0, 0.0, 0.0),
                angle: 361,
                kb_scale: 100,
                kb_weight: 0,
                kb_base: 15,
            },
            3.0,
            11.0,
        )
        .with_hit_generation(0),
        ActiveHitbox::new(
            Hitbox {
                damage: 11,
                radius: 120.0,
                offset: Vec3::new(10.0, 0.0, 0.0),
                angle: 361,
                kb_scale: 100,
                kb_weight: 0,
                kb_base: 0,
            },
            11.0,
            29.0,
        )
        .with_hit_generation(0),
        ActiveHitbox::new(
            Hitbox {
                damage: 11,
                radius: 120.0,
                offset: Vec3::new(10.0, 0.0, 0.0),
                angle: 361,
                kb_scale: 100,
                kb_weight: 0,
                kb_base: 0,
            },
            11.0,
            29.0,
        )
        .with_hit_generation(0),
        ActiveHitbox::new(
            Hitbox {
                damage: 11,
                radius: 130.0,
                offset: Vec3::new(0.0, 0.0, 0.0),
                angle: 361,
                kb_scale: 100,
                kb_weight: 0,
                kb_base: 0,
            },
            11.0,
            29.0,
        )
        .with_hit_generation(0),
    ],
    length_frames: 37.0,
    landing_lag_percent: Some(50),
};
pub static ATTACKAIRF: MoveData = MoveData {
    hitboxes: &[
        ActiveHitbox::new(
            Hitbox {
                damage: 3,
                radius: 195.0,
                offset: Vec3::new(0.0, 65.0, 15.0),
                angle: 361,
                kb_scale: 100,
                kb_weight: 30,
                kb_base: 0,
            },
            7.0,
            9.0,
        )
        .with_hit_generation(0),
        ActiveHitbox::new(
            Hitbox {
                damage: 3,
                radius: 170.0,
                offset: Vec3::new(0.0, 0.0, 0.0),
                angle: 361,
                kb_scale: 100,
                kb_weight: 30,
                kb_base: 0,
            },
            7.0,
            9.0,
        )
        .with_hit_generation(0),
        ActiveHitbox::new(
            Hitbox {
                damage: 3,
                radius: 195.0,
                offset: Vec3::new(0.0, 65.0, 15.0),
                angle: 361,
                kb_scale: 100,
                kb_weight: 30,
                kb_base: 0,
            },
            10.0,
            12.0,
        )
        .with_hit_generation(1),
        ActiveHitbox::new(
            Hitbox {
                damage: 3,
                radius: 170.0,
                offset: Vec3::new(0.0, 0.0, 0.0),
                angle: 361,
                kb_scale: 100,
                kb_weight: 30,
                kb_base: 0,
            },
            10.0,
            12.0,
        )
        .with_hit_generation(1),
        ActiveHitbox::new(
            Hitbox {
                damage: 3,
                radius: 195.0,
                offset: Vec3::new(0.0, 65.0, 15.0),
                angle: 361,
                kb_scale: 100,
                kb_weight: 30,
                kb_base: 0,
            },
            13.0,
            15.0,
        )
        .with_hit_generation(2),
        ActiveHitbox::new(
            Hitbox {
                damage: 3,
                radius: 170.0,
                offset: Vec3::new(0.0, 0.0, 0.0),
                angle: 361,
                kb_scale: 100,
                kb_weight: 30,
                kb_base: 0,
            },
            13.0,
            15.0,
        )
        .with_hit_generation(2),
        ActiveHitbox::new(
            Hitbox {
                damage: 3,
                radius: 195.0,
                offset: Vec3::new(0.0, 65.0, 15.0),
                angle: 361,
                kb_scale: 100,
                kb_weight: 30,
                kb_base: 0,
            },
            16.0,
            18.0,
        )
        .with_hit_generation(3),
        ActiveHitbox::new(
            Hitbox {
                damage: 3,
                radius: 170.0,
                offset: Vec3::new(0.0, 0.0, 0.0),
                angle: 361,
                kb_scale: 100,
                kb_weight: 30,
                kb_base: 0,
            },
            16.0,
            18.0,
        )
        .with_hit_generation(3),
        ActiveHitbox::new(
            Hitbox {
                damage: 3,
                radius: 195.0,
                offset: Vec3::new(0.0, 65.0, 15.0),
                angle: 361,
                kb_scale: 100,
                kb_weight: 30,
                kb_base: 0,
            },
            19.0,
            21.0,
        )
        .with_hit_generation(4),
        ActiveHitbox::new(
            Hitbox {
                damage: 3,
                radius: 170.0,
                offset: Vec3::new(0.0, 0.0, 0.0),
                angle: 361,
                kb_scale: 100,
                kb_weight: 30,
                kb_base: 0,
            },
            19.0,
            21.0,
        )
        .with_hit_generation(4),
        ActiveHitbox::new(
            Hitbox {
                damage: 3,
                radius: 195.0,
                offset: Vec3::new(0.0, 65.0, 15.0),
                angle: 361,
                kb_scale: 100,
                kb_weight: 30,
                kb_base: 0,
            },
            22.0,
            24.0,
        )
        .with_hit_generation(5),
        ActiveHitbox::new(
            Hitbox {
                damage: 3,
                radius: 170.0,
                offset: Vec3::new(0.0, 0.0, 0.0),
                angle: 361,
                kb_scale: 100,
                kb_weight: 30,
                kb_base: 0,
            },
            22.0,
            24.0,
        )
        .with_hit_generation(5),
        ActiveHitbox::new(
            Hitbox {
                damage: 3,
                radius: 195.0,
                offset: Vec3::new(0.0, 65.0, 15.0),
                angle: 361,
                kb_scale: 100,
                kb_weight: 30,
                kb_base: 0,
            },
            25.0,
            27.0,
        )
        .with_hit_generation(6),
        ActiveHitbox::new(
            Hitbox {
                damage: 3,
                radius: 170.0,
                offset: Vec3::new(0.0, 0.0, 0.0),
                angle: 361,
                kb_scale: 100,
                kb_weight: 30,
                kb_base: 0,
            },
            25.0,
            27.0,
        )
        .with_hit_generation(6),
    ],
    length_frames: 40.0,
    landing_lag_percent: None,
};
pub static ATTACKAIRB: MoveData = MoveData {
    hitboxes: &[
        ActiveHitbox::new(
            Hitbox {
                damage: 16,
                radius: 170.0,
                offset: Vec3::new(100.0, 0.0, 0.0),
                angle: 361,
                kb_scale: 100,
                kb_weight: 0,
                kb_base: 15,
            },
            10.0,
            14.0,
        )
        .with_hit_generation(0),
        ActiveHitbox::new(
            Hitbox {
                damage: 16,
                radius: 150.0,
                offset: Vec3::new(0.0, 0.0, 0.0),
                angle: 361,
                kb_scale: 100,
                kb_weight: 0,
                kb_base: 15,
            },
            10.0,
            14.0,
        )
        .with_hit_generation(0),
        ActiveHitbox::new(
            Hitbox {
                damage: 14,
                radius: 155.0,
                offset: Vec3::new(40.0, 0.0, 0.0),
                angle: 361,
                kb_scale: 100,
                kb_weight: 0,
                kb_base: 0,
            },
            14.0,
            22.0,
        )
        .with_hit_generation(0),
        ActiveHitbox::new(
            Hitbox {
                damage: 14,
                radius: 135.0,
                offset: Vec3::new(0.0, 0.0, 0.0),
                angle: 361,
                kb_scale: 100,
                kb_weight: 0,
                kb_base: 0,
            },
            14.0,
            22.0,
        )
        .with_hit_generation(0),
    ],
    length_frames: 42.0,
    landing_lag_percent: Some(50),
};
pub static ATTACKAIRHI: MoveData = MoveData {
    hitboxes: &[
        ActiveHitbox::new(
            Hitbox {
                damage: 10,
                radius: 90.0,
                offset: Vec3::new(0.0, 0.0, 0.0),
                angle: 361,
                kb_scale: 100,
                kb_weight: 0,
                kb_base: 10,
            },
            3.0,
            11.0,
        )
        .with_hit_generation(0),
        ActiveHitbox::new(
            Hitbox {
                damage: 10,
                radius: 140.0,
                offset: Vec3::new(96.0, 150.0, -50.0),
                angle: 361,
                kb_scale: 100,
                kb_weight: 0,
                kb_base: 10,
            },
            3.0,
            11.0,
        )
        .with_hit_generation(0),
    ],
    length_frames: 34.0,
    landing_lag_percent: None,
};
pub static ATTACKAIRLW: MoveData = MoveData {
    hitboxes: &[
        ActiveHitbox::new(
            Hitbox {
                damage: 13,
                radius: 165.0,
                offset: Vec3::new(0.0, 65.0, 15.0),
                angle: 361,
                kb_scale: 100,
                kb_weight: 0,
                kb_base: 20,
            },
            8.0,
            26.0,
        )
        .with_hit_generation(0),
        ActiveHitbox::new(
            Hitbox {
                damage: 13,
                radius: 140.0,
                offset: Vec3::new(0.0, 0.0, 0.0),
                angle: 361,
                kb_scale: 100,
                kb_weight: 0,
                kb_base: 20,
            },
            8.0,
            26.0,
        )
        .with_hit_generation(0),
    ],
    length_frames: 45.0,
    landing_lag_percent: None,
};
pub static LANDINGAIRF: MoveData = MoveData {
    hitboxes: &[ActiveHitbox::new(
        Hitbox {
            damage: 6,
            radius: 180.0,
            offset: Vec3::new(0.0, 65.0, 15.0),
            angle: 361,
            kb_scale: 100,
            kb_weight: 0,
            kb_base: 30,
        },
        0.0,
        2.0,
    )
    .with_hit_generation(0)],
    length_frames: 16.0,
    landing_lag_percent: None,
};
pub static LANDINGAIRLW: MoveData = MoveData {
    hitboxes: &[],
    length_frames: 40.0,
    landing_lag_percent: None,
};
pub static THUNDERHIT: MoveData = MoveData {
    hitboxes: &[ActiveHitbox::new(
        Hitbox {
            damage: 16,
            radius: 350.0,
            offset: Vec3::new(0.0, 300.0, 0.0),
            angle: 361,
            kb_scale: 60,
            kb_weight: 0,
            kb_base: 100,
        },
        0.0,
        10.0,
    )
    .with_hit_generation(0)],
    length_frames: 60.0,
    landing_lag_percent: None,
};
pub fn joints(s: AnyStatus, index: usize) -> u8 {
    let ids: &[u8] = match s {
        AnyStatus::Common(Status::Attack11) => &[11, 5],
        AnyStatus::Common(Status::AttackDash) => &[11, 5],
        AnyStatus::Common(Status::AttackS3Hi) => &[28, 23, 5],
        AnyStatus::Common(Status::AttackS3) => &[28, 23, 5],
        AnyStatus::Common(Status::AttackS3Lw) => &[28, 23, 5],
        AnyStatus::Common(Status::AttackHi3) => &[29, 29],
        AnyStatus::Common(Status::AttackLw3) => &[29, 29],
        AnyStatus::Common(Status::AttackS4) => &[0, 0, 0, 0],
        AnyStatus::Common(Status::AttackHi4) => &[29, 29, 29, 29],
        AnyStatus::Common(Status::AttackLw4) => &[26, 21, 5, 26, 21, 5],
        AnyStatus::Common(Status::AttackAirN) => &[26, 21, 5, 26, 21, 5],
        AnyStatus::Common(Status::AttackAirF) => &[11, 5, 11, 5, 11, 5, 11, 5, 11, 5, 11, 5, 11, 5],
        AnyStatus::Common(Status::AttackAirB) => &[21, 5, 21, 5],
        AnyStatus::Common(Status::AttackAirHi) => &[29, 29],
        AnyStatus::Common(Status::AttackAirLw) => &[11, 5],
        AnyStatus::Common(Status::LandingAirF) => &[11],
        AnyStatus::Common(Status::LandingAirLw) => &[],
        AnyStatus::Pikachu(
            crate::status::PikachuStatus::SpecialLwHit
            | crate::status::PikachuStatus::SpecialAirLwHit,
        ) => &[0],
        _ => &[0],
    };
    ids.get(index).copied().unwrap_or(0)
}
