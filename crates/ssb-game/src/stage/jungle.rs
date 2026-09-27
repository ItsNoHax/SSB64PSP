//! Kongo Jungle: the Barrel Cannon (`grjungle.c`).
//!
//! The barrel's path is its default animation, played by the runtime; the
//! controller only spins it.

use super::{Obstacle, Registry, StageAnim, StageObjects};
use crate::fighter::Fighter;
use crate::rng;
use crate::status::{AnyStatus, Status};
use ssb_engine::math::Vec3;

/// `grJungleTaruCannStatus`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BarrelStatus {
    Move,
    Rotate,
}

/// `GRCommonGroundVarsJungle`, plus the barrel root's `rotate.z`.
#[derive(Debug, Clone, PartialEq)]
pub struct Jungle {
    pub status: BarrelStatus,
    pub wait: u16,
    pub rotate_step: f32,
    pub rotate: f32,
}

impl Jungle {
    /// `grJungleMakeTaruCann` @ 0x80109E84.
    pub fn new(objects: &mut dyn StageObjects, registry: &mut Registry) -> Self {
        objects.play(StageAnim::TaruCannDefault);
        registry.add_obstacle(Obstacle::TaruCann);
        Jungle {
            status: BarrelStatus::Move,
            wait: (rng::rand_int_range(180) + 180) as u16,
            rotate_step: 0.0,
            rotate: 0.0,
        }
    }

    /// `grJungleTaruCannProcUpdate` @ 0x80109E34.
    pub fn tick(&mut self) {
        match self.status {
            BarrelStatus::Move => {
                self.wait = self.wait.wrapping_sub(1);
                if self.wait == 0 {
                    self.status = BarrelStatus::Rotate;
                    self.rotate_step = if !rng::rand_ushort().is_multiple_of(2) {
                        0.07
                    } else {
                        -0.07
                    };
                    self.wait = 90;
                }
            }
            BarrelStatus::Rotate => {
                self.wait = self.wait.wrapping_sub(1);
                if self.wait == 0 {
                    self.status = BarrelStatus::Move;
                    self.wait = (rng::rand_int_range(180) + 180) as u16;
                    self.rotate = 0.0;
                } else {
                    self.rotate += self.rotate_step;
                }
            }
        }
    }
}

/// `grJungleTaruCannCheckGetDamageKind` @ 0x80109FD8, without the fill
/// animation the caller starts. `taken` is another fighter already in the
/// barrel.
pub fn check_tarucann(f: &Fighter, barrel: Vec3, taken: bool) -> bool {
    if f.hazard.tarucann_wait != 0
        || f.status.status == AnyStatus::Common(Status::TaruCann)
        || f.grab.capture_immune
    {
        return false;
    }
    let pos = super::top_n(f);
    if (barrel.x - pos.x).abs() < 280.0 && (barrel.y - pos.y).abs() < 280.0 {
        return !taken;
    }
    false
}
