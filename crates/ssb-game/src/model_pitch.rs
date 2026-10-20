//! Writes a status makes straight onto the model's DObjs: joint 4's pitch
//! (and Pikachu's Quick Attack stretch) and TransN's roll (RE-503).
//!
//! | Decomp | Writes |
//! |---|---|
//! | `ftCommonDamageFlyRollUpdateModelPitch` | joint 4 `rotate.x = ArcTan2(vel_air.x + vel_damage_air.x, vel_air.y + vel_damage_air.y) * lr` |
//! | `ftNessSpecialHiUpdateModelPitch`, `ftFoxSpecialHiUpdateModelPitch` | joint 4 `rotate.x = ArcTan2(vel_air.x, vel_air.y) * lr - F_CST_DTOR32(90)` |
//! | `ftPikachuSpecialHiUpdateModelPitchScale` | the same, and `scale = (0.8, 0.8, 1.2)` |
//! | `ftCaptainSpecialLwAirSetStatus` | TransN `rotate.z = TopN.rotate.z` |
//!
//! The skeleton is the runtime's, so a write is queued in [`PoseWrites`]
//! and put on the pose before `proc_slope` and the parts transform pass.
//! There it lasts as long as the DObj value does in the original: until
//! the figatree's own track for that channel next plays, or until a status
//! with a figatree of its own resets every joint from joint 4 on to its
//! `DObjDesc` and TransN's `rotate.z` to 0 (`ftMainSetStatus`,
//! `ftmain.c:4657-4671`), which drops a queued write too
//! ([`crate::status::set_any_status_preserve`]).

use crate::fighter::Fighter;
use crate::status::{AnyStatus, FoxStatus, NessStatus, PikachuStatus, Status};
use ssb_engine::math::{f_cst_dtor32, sy_arc_tan2, Vec3};

/// `nFTPartsJointCommonStart` (`FTPIKACHU_QUICKATTACK_BASE_JOINT`): the
/// first model joint, the root of the drawn body.
pub const BASE_JOINT: u8 = 4;

/// `FTPIKACHU_QUICKATTACK_SCALE_X`, `_Y`, `_Z`.
pub const QUICK_ATTACK_SCALE: [f32; 3] = [0.8, 0.8, 1.2];

/// One write to joint 4.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ModelPitch {
    /// `joints[4]->rotate.vec.f.x`.
    pub rotate_x: f32,
    /// `joints[4]->scale.vec.f`, when the write sets it.
    pub scale: Option<[f32; 3]>,
}

/// The pose writes queued since the runtime last took them.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct PoseWrites {
    pub joint4: Option<ModelPitch>,
    /// `joints[nFTPartsJointTransN]->rotate.vec.f.z`.
    pub transn_rotate_z: Option<f32>,
}

impl PoseWrites {
    pub const NONE: Self = Self {
        joint4: None,
        transn_rotate_z: None,
    };
}

/// The Ness, Fox and Pikachu pitch: the travel direction measured from
/// straight up, turned by the facing, less a quarter turn.
pub fn special_hi_pitch(vel_air: Vec3, lr: f32) -> f32 {
    (sy_arc_tan2(vel_air.x, vel_air.y) * lr) - f_cst_dtor32(90.0)
}

/// `ftCommonDamageFlyRollUpdateModelPitch` (`ftcommondamage.c:204`).
pub fn fly_roll(f: &mut Fighter) {
    let v = f.physics.vel_air;
    let d = f.physics.vel_knockback;
    f.pose_writes.joint4 = Some(ModelPitch {
        rotate_x: sy_arc_tan2(v.x + d.x, v.y + d.y) * f.facing.sign(),
        scale: None,
    });
}

/// `ftNessSpecialHiUpdateModelPitch` (`ftnessspecialhi.c:505`) and
/// `ftFoxSpecialHiUpdateModelPitch` (`ftfoxspecialhi.c:138`).
pub fn special_hi(f: &mut Fighter) {
    f.pose_writes.joint4 = Some(ModelPitch {
        rotate_x: special_hi_pitch(f.physics.vel_air, f.facing.sign()),
        scale: None,
    });
}

/// `ftPikachuSpecialHiUpdateModelPitchScale` (`ftpikachuspecialhi.c:149`).
pub fn quick_attack(f: &mut Fighter) {
    f.pose_writes.joint4 = Some(ModelPitch {
        rotate_x: special_hi_pitch(f.physics.vel_air, f.facing.sign()),
        scale: Some(QUICK_ATTACK_SCALE),
    });
}

/// The tail of a grounded `proc_physics`, before `ftPhysicsSetGroundVelTransferAir`:
/// `ftCommonDamageCommonProcPhysics` pitches a `DamageFlyRoll` on either
/// kinetics (`ftcommondamage.c:228`).
pub(crate) fn after_ground_status_physics(f: &mut Fighter) {
    if f.status.status == Status::DamageFlyRoll {
        fly_roll(f);
    }
}

/// The tail of a grounded `proc_physics` that transfers its ground speed
/// to `vel_air` first: `ftNessSpecialHiJibakuProcPhysics`
/// (`ftnessspecialhi.c:559`) and `ftPikachuSpecialHiProcPhysics`
/// (`ftpikachuspecialhi.c:166`).
pub(crate) fn after_ground_transfer(f: &mut Fighter) {
    match f.status.status {
        AnyStatus::Ness(NessStatus::SpecialHiJibaku) => special_hi(f),
        AnyStatus::Pikachu(PikachuStatus::SpecialHi) => quick_attack(f),
        _ => {}
    }
}

/// The tail of an aerial `proc_physics`: `ftCommonDamageCommonProcPhysics`
/// (`:230`), `ftNessSpecialAirHiJibakuProcPhysics` (`:580`),
/// `ftFoxSpecialAirHiProcPhysics` (`:189`) and
/// `ftPikachuSpecialAirHiProcPhysics` (`:172`).
pub(crate) fn after_air_physics(f: &mut Fighter) {
    match f.status.status {
        AnyStatus::Common(Status::DamageFlyRoll) => fly_roll(f),
        AnyStatus::Ness(NessStatus::SpecialAirHiJibaku)
        | AnyStatus::Fox(FoxStatus::SpecialAirHi) => special_hi(f),
        AnyStatus::Pikachu(PikachuStatus::SpecialAirHi) => quick_attack(f),
        _ => {}
    }
}

#[cfg(test)]
#[path = "model_pitch_tests.rs"]
mod tests;
