//! Mushroom Kingdom: the balance scales, the Piranha Plants and the POW
//! Block spawner (`grinishie.c`).

use super::{
    mapobj, objects_of, standing_group, Hazard, MapQuery, Registry, StageAnim, StageInit,
    StageItem, StageObj, StageObjects,
};
use crate::fighter::Fighter;
use crate::map::{GroupStatus, MapGroup};
use crate::rng;
use ssb_engine::math::Vec3;

/// `dGRInishieScaleLineGroups`.
pub const SCALE_GROUPS: [u8; 2] = [1, 2];
/// The scale height at which both platforms fall.
pub const SCALE_ALT_MAX: f32 = 1100.0;

/// `grInishieScaleStatus`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScaleStatus {
    Wait,
    Fall,
    Sleep,
    Retract,
}

/// `grInishiePowerBlockStatus`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerBlockStatus {
    Wait,
    Make,
    Sleep,
    Damage,
}

/// `GRCommonGroundVarsInishie`.
#[derive(Debug, Clone, PartialEq)]
pub struct Inishie {
    /// Each platform `DObj`'s translation.
    pub platform: [Vec3; 2],
    pub platform_base_y: [f32; 2],
    pub alt: f32,
    pub accelerate: f32,
    pub wait: u16,
    pub status: ScaleStatus,
    pub players_tt: [u8; 4],
    /// `players_ga`, as "was grounded" (`nMPKineticsGround` is 0).
    pub players_grounded: [bool; 4],
    pub bound_bottom: f32,
    pub pblock: Option<u32>,
    pub pblock_wait: u16,
    pub pblock_status: PowerBlockStatus,
    pub pblock_positions: [Vec3; 10],
    pub pblock_position_count: u8,
    pub pakkun: [Option<u32>; 2],
}

impl Inishie {
    /// `grInishieMakeGround` @ 0x80109C0C: scales, Piranha Plants, POW.
    pub fn new(
        init: &StageInit<'_>,
        groups: &mut [MapGroup],
        objects: &mut dyn StageObjects,
    ) -> Self {
        let mut platform = [Vec3::ZERO; 2];
        for (i, kind) in [mapobj::SCALE_L, mapobj::SCALE_R].into_iter().enumerate() {
            platform[i] = objects_of(init.map_objects, kind)
                .next()
                .unwrap_or(Vec3::ZERO);
            if let Some(g) = groups.get_mut(SCALE_GROUPS[i] as usize) {
                g.status = GroupStatus::On;
            }
        }
        let mut pakkun = [None; 2];
        for (i, kind) in [mapobj::PAKKUN_L, mapobj::PAKKUN_R].into_iter().enumerate() {
            let pos = objects_of(init.map_objects, kind)
                .next()
                .unwrap_or(Vec3::ZERO);
            pakkun[i] = objects.make_item(StageItem::Pakkun(i as u8), pos);
        }
        let mut pblock_positions = [Vec3::ZERO; 10];
        let mut count = 0;
        for (slot, pos) in pblock_positions
            .iter_mut()
            .zip(objects_of(init.map_objects, mapobj::POWER_BLOCK))
        {
            *slot = pos;
            count += 1;
        }
        Inishie {
            platform,
            platform_base_y: [platform[0].y, platform[1].y],
            alt: 0.0,
            accelerate: 0.0,
            wait: 0,
            status: ScaleStatus::Wait,
            players_tt: [0; 4],
            players_grounded: [true; 4],
            bound_bottom: init.bound_bottom,
            pblock: None,
            pblock_wait: 0,
            pblock_status: PowerBlockStatus::Wait,
            pblock_positions,
            pblock_position_count: count,
            pakkun,
        }
    }

    /// `grInishieScaleUpdateFighterStatsGA` @ 0x80108CD0.
    fn update_players(&mut self, fighters: &[&mut Fighter]) {
        for f in fighters.iter() {
            let p = (f.port as usize).min(3);
            if f.is_grounded() {
                if !self.players_grounded[p] {
                    self.players_tt[p] = 1;
                } else if self.players_tt[p] != 0 {
                    self.players_tt[p] -= 1;
                }
            } else {
                self.players_tt[p] = 0;
            }
            self.players_grounded[p] = f.is_grounded();
        }
    }

    /// `grInishieScaleGetPressure` @ 0x80108D50.
    fn pressure<F>(&self, fighters: &[&mut Fighter], map: &MapQuery<'_, F>, id: u8) -> f32 {
        let mut pressure = 0.0;
        for f in fighters.iter() {
            if standing_group(f, map) == Some(id) {
                let weight = (1.0 - f.attributes.weight) + 1.4;
                if self.players_tt[(f.port as usize).min(3)] != 0 {
                    pressure += weight * 8.0;
                } else {
                    pressure += weight;
                }
            }
        }
        pressure
    }

    fn place(&mut self) {
        self.platform[0].y = self.platform_base_y[0] + self.alt;
        self.platform[1].y = self.platform_base_y[1] - self.alt;
    }

    /// `grInishieScaleUpdateWait` @ 0x801085E0.
    fn update_wait<F>(&mut self, fighters: &[&mut Fighter], map: &MapQuery<'_, F>) {
        self.update_players(fighters);
        let l = self.pressure(fighters, map, SCALE_GROUPS[0]);
        let r = self.pressure(fighters, map, SCALE_GROUPS[1]);
        if l == 0.0 && r == 0.0 {
            if self.alt != 0.0 {
                if self.alt < 0.0 {
                    self.alt = (self.alt + 8.0).min(0.0);
                } else {
                    self.alt = (self.alt - 8.0).max(0.0);
                }
            }
            self.accelerate = 0.0;
        } else {
            self.accelerate += r - l;
            if l != 0.0 && r != 0.0 && self.accelerate != 0.0 {
                self.accelerate *= 0.93;
            } else if self.accelerate > 0.0 {
                self.accelerate = (self.accelerate - 0.9).max(0.0);
            } else if self.accelerate < 0.0 {
                self.accelerate = (self.accelerate + 0.9).min(0.0);
            }
            self.alt += self.accelerate;
        }
        if self.alt.abs() > SCALE_ALT_MAX {
            self.accelerate = 0.0;
            self.alt = if self.alt < 0.0 {
                -SCALE_ALT_MAX
            } else {
                SCALE_ALT_MAX
            };
            self.status = ScaleStatus::Fall;
        }
        self.place();
    }

    /// `grInishieScaleProcUpdate` @ 0x801093EC.
    fn tick_scales<F>(
        &mut self,
        fighters: &[&mut Fighter],
        groups: &mut [MapGroup],
        objects: &mut dyn StageObjects,
        map: &MapQuery<'_, F>,
    ) {
        match self.status {
            ScaleStatus::Wait => self.update_wait(fighters, map),
            ScaleStatus::Fall => {
                self.accelerate = (self.accelerate + 3.0).min(70.0);
                self.platform[0].y -= self.accelerate;
                self.platform[1].y -= self.accelerate;
                let deadzone = self.bound_bottom + -1000.0;
                if self.platform[0].y < deadzone && self.platform[1].y < deadzone {
                    self.status = ScaleStatus::Sleep;
                    self.accelerate = 0.0;
                    for id in SCALE_GROUPS {
                        if let Some(g) = groups.get_mut(id as usize) {
                            g.status = GroupStatus::Off;
                        }
                    }
                    self.wait = 180;
                }
            }
            ScaleStatus::Sleep => {
                self.wait = self.wait.wrapping_sub(1);
                if self.wait == 0 {
                    self.status = ScaleStatus::Retract;
                    objects.play(StageAnim::ScaleRetract(0));
                    objects.play(StageAnim::ScaleRetract(1));
                }
            }
            ScaleStatus::Retract => {
                let mut complete = false;
                if self.alt != 0.0 {
                    if self.alt < 0.0 {
                        self.alt += 10.0;
                        complete = self.alt >= 0.0;
                    } else {
                        self.alt -= 10.0;
                        complete = self.alt <= 0.0;
                    }
                }
                if complete {
                    self.alt = 0.0;
                    for (i, id) in SCALE_GROUPS.into_iter().enumerate() {
                        objects.stop(StageObj::Scale(i as u8));
                        if let Some(g) = groups.get_mut(id as usize) {
                            g.status = GroupStatus::On;
                        }
                    }
                    self.status = ScaleStatus::Wait;
                }
                self.place();
            }
        }
        for (i, id) in SCALE_GROUPS.into_iter().enumerate() {
            if let Some(g) = groups.get_mut(id as usize) {
                g.set_position(self.platform[i]);
            }
        }
    }

    fn set_pblock_wait(&mut self) {
        self.pblock_status = PowerBlockStatus::Make;
        self.pblock_wait = 1800;
    }

    /// `grInishiePowerBlockProcUpdate` @ 0x80109968.
    fn tick_pblock(
        &mut self,
        registry: &mut Registry,
        objects: &mut dyn StageObjects,
        started: bool,
    ) {
        match self.pblock_status {
            PowerBlockStatus::Wait => {
                if started {
                    self.set_pblock_wait();
                }
            }
            PowerBlockStatus::Make => {
                self.pblock_wait = self.pblock_wait.wrapping_sub(1);
                if self.pblock_wait == 0 {
                    let count = i32::from(self.pblock_position_count.max(1));
                    let pos = self.pblock_positions[rng::rand_int_range(count) as usize];
                    match objects.make_item(StageItem::PowerBlock, pos) {
                        Some(item) => {
                            self.pblock = Some(item);
                            self.pblock_status = PowerBlockStatus::Sleep;
                        }
                        None => self.set_pblock_wait(),
                    }
                }
            }
            PowerBlockStatus::Damage => {
                self.pblock_wait = self.pblock_wait.wrapping_sub(1);
                if self.pblock_wait == 0 {
                    registry.clear_hazard(|h| matches!(h, Hazard::PowerBlock { .. }));
                }
            }
            PowerBlockStatus::Sleep => {}
        }
    }

    pub fn tick<F>(
        &mut self,
        fighters: &[&mut Fighter],
        groups: &mut [MapGroup],
        objects: &mut dyn StageObjects,
        map: &MapQuery<'_, F>,
        started: bool,
        registry: &mut Registry,
    ) {
        self.tick_scales(fighters, groups, objects, map);
        self.tick_pblock(registry, objects, started);
    }

    /// `grInishiePowerBlockSetDamage` @ 0x80109B4C, called by the POW item.
    pub fn set_power_block_damage(&mut self, registry: &mut Registry, handicap: u8) {
        registry.add_hazard(Hazard::PowerBlock { handicap });
        self.pblock_wait = 2;
        self.pblock_status = PowerBlockStatus::Damage;
    }

    /// `grInishiePowerBlockSetWait` @ 0x8010986C, called when the POW goes.
    pub fn power_block_gone(&mut self) {
        self.pblock = None;
        self.set_pblock_wait();
    }
}
