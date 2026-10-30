//! Planet Zebes: the rising acid (`grzebes.c`).

use super::{Hazard, Registry, StageAnim, StageInit, StageObj, StageObjects};
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

/// `dGRZebesAcidAttributes`, generated at build time from the ROM by
/// `crates/ssb-tablegen` (`battle_data.rs`).
pub use generated::ACID_STEPS;
mod generated {
    use super::AcidStep;
    include!(concat!(env!("OUT_DIR"), "/zebes_acid.rs"));
}

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
    /// The acid `DObj`'s child translation Y, the surface above the root.
    /// `llGRZebesMapAcidAnimJoint` moves it; [`Zebes::tick`] reads it after
    /// the priority-5 animation, before any fighter's hit search.
    pub surface_y: f32,
    /// This frame's `efManagerQuakeMakeEffect` calls, made by
    /// [`super::Stage::flush_effects`].
    pub fx: crate::wpeffect::Emit,
}

impl Zebes {
    /// `grZebesMakeGround` @ 0x80108448: `grZebesMakeAcid`, then the hazard.
    pub fn new(
        init: &StageInit<'_>,
        objects: &mut dyn StageObjects,
        registry: &mut Registry,
    ) -> Self {
        // `grZebesMakeAcid` @ 0x801080EC: `gcAddAnimAll` + `gcPlayAnimAll`.
        objects.play(StageAnim::Acid);
        let mut z = Zebes {
            level: ACID_STEPS[ACID_STEPS.len() - 1].level,
            level_step: 0.0,
            wait: 0,
            status: AcidStatus::Wait,
            attr_id: 0,
            rumble_wait: 0,
            surface_y: init.acid_surface_y,
            fx: crate::wpeffect::Emit::default(),
        };
        z.set_random_wait();
        objects.set_translate_y(StageObj::Acid, z.level);
        z.read_surface(objects);
        registry.add_hazard(Hazard::Acid);
        z
    }

    /// The acid child's animated Y; the rest value when there is no object.
    fn read_surface(&mut self, objects: &dyn StageObjects) {
        if let Some(t) = objects.child_translate(StageObj::Acid) {
            self.surface_y = t.y;
        }
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

    /// `grZebesAcidUpdateRumble` @ 0x80108268 (grzebes.c:127-136), run by
    /// the Shake and Rise updates.
    fn rumble(&mut self) {
        if self.rumble_wait == 0 {
            self.fx.push(crate::wpeffect::WeaponEffect::Quake(0));
            self.rumble_wait = 18;
        }
        self.rumble_wait -= 1;
    }

    /// `grZebesProcUpdate` @ 0x801083C4.
    pub fn tick(&mut self, started: bool, objects: &mut dyn StageObjects) {
        self.read_surface(objects);
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
                objects.set_translate_y(StageObj::Acid, self.level);
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

    /// `grZebesAcidCheckGetDamageKind` @ 0x801084AC.
    // Out of line for the same reason as `Hyrule::check_twister` (RE-360).
    #[inline(never)]
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
