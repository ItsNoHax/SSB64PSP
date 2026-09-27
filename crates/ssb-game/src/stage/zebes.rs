//! Planet Zebes: the rising acid (`grzebes.c`).

use super::{Hazard, Registry, StageInit};
use crate::fighter::Fighter;
use crate::rng;

/// `GRZebesAcid`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AcidStep {
    pub wait_base: u16,
    pub random_min: u16,
    pub random_max: u16,
    pub level: f32,
}

const fn step(wait_base: u16, random_min: u16, random_max: u16, level: f32) -> AcidStep {
    AcidStep {
        wait_base,
        random_min,
        random_max,
        level,
    }
}

/// `dGRZebesAcidAttributes` @ 0x8012EA60.
pub const ACID_STEPS: [AcidStep; 16] = [
    step(1200, 60, 70, -3600.0),
    step(180, 60, 70, -1000.0),
    step(60, 60, 70, -200.0),
    step(60, 60, 70, 1800.0),
    step(60, 60, 70, -3600.0),
    step(120, 0, 0, 2600.0),
    step(30, 0, 0, -1000.0),
    step(1200, 60, 70, -500.0),
    step(600, 60, 70, -400.0),
    step(100, 60, 70, 800.0),
    step(1200, 60, 70, 1200.0),
    step(60, 0, 0, -1000.0),
    step(60, 60, 70, 0.0),
    step(60, 60, 70, -1000.0),
    step(120, 60, 70, 500.0),
    step(200, 60, 70, -3000.0),
];

/// `grZebesStatus`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AcidStatus {
    Wait,
    Normal,
    Shake,
    Rise,
}

/// `GRCommonGroundVarsZebes`.
#[derive(Debug, Clone, PartialEq)]
pub struct Zebes {
    pub level: f32,
    pub level_step: f32,
    pub wait: u16,
    pub status: AcidStatus,
    pub attr_id: u8,
    pub rumble_wait: u8,
    /// The acid `DObj`'s child translation, the surface above the root.
    pub surface_y: f32,
}

impl Zebes {
    /// `grZebesMakeGround` @ 0x80108448: `grZebesMakeAcid`, then the hazard.
    pub fn new(init: &StageInit<'_>, registry: &mut Registry) -> Self {
        let mut z = Zebes {
            level: ACID_STEPS[ACID_STEPS.len() - 1].level,
            level_step: 0.0,
            wait: 0,
            status: AcidStatus::Wait,
            attr_id: 0,
            rumble_wait: 0,
            surface_y: init.acid_surface_y,
        };
        z.set_random_wait();
        registry.add_hazard(Hazard::Acid);
        z
    }

    /// `grZebesAcidSetRandomWait` @ 0x80108088.
    fn set_random_wait(&mut self) {
        let s = ACID_STEPS[self.attr_id as usize];
        self.wait = s.wait_base
            + s.random_min
            + rng::rand_int_range(i32::from(s.random_max) - i32::from(s.random_min)) as u16;
    }

    /// `grZebesAcidSetLevelStep` @ 0x80108020.
    fn set_level_step(&mut self) {
        let target = ACID_STEPS[self.attr_id as usize].level + rng::rand_float() * 250.0;
        self.level_step = (target - self.level) / 240.0;
    }

    /// `grZebesAcidUpdateRumble`: only the quake timer.
    fn rumble(&mut self) {
        if self.rumble_wait == 0 {
            self.rumble_wait = 18;
        }
        self.rumble_wait -= 1;
    }

    /// `grZebesProcUpdate` @ 0x801083C4.
    pub fn tick(&mut self, started: bool) {
        match self.status {
            AcidStatus::Wait => {
                if started {
                    self.status = AcidStatus::Normal;
                }
            }
            AcidStatus::Normal => {
                self.wait = self.wait.wrapping_sub(1);
                if self.wait == 0 {
                    self.status = AcidStatus::Shake;
                    self.wait = 18;
                    self.rumble_wait = 0;
                }
            }
            AcidStatus::Shake => {
                self.wait = self.wait.wrapping_sub(1);
                if self.wait == 0 {
                    self.status = AcidStatus::Rise;
                    self.wait = 240;
                    self.set_level_step();
                }
                self.rumble();
            }
            AcidStatus::Rise => {
                self.level += self.level_step;
                self.wait = self.wait.wrapping_sub(1);
                if self.wait == 0 {
                    self.status = AcidStatus::Normal;
                    self.attr_id += 1;
                    if self.attr_id as usize >= ACID_STEPS.len() {
                        self.attr_id = 0;
                    }
                    self.set_random_wait();
                }
                self.rumble();
            }
        }
    }

    /// The acid `DObj`'s translation Y, which the controller writes.
    pub fn root_y(&self) -> f32 {
        self.level
    }

    /// `grZebesAcidCheckGetDamageKind` @ 0x801084AC.
    pub fn check_acid(&self, f: &Fighter) -> bool {
        f.hazard.acid_wait == 0 && super::top_n(f).y < self.level + self.surface_y
    }

    /// `grZebesAcidGetLevelInfo` @ 0x8010850C.
    pub fn level_info(&self) -> (f32, f32) {
        let step = if self.status == AcidStatus::Rise {
            self.level_step
        } else {
            0.0
        };
        (self.level, step)
    }
}
