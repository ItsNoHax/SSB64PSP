//! Hyrule Castle: the Twister (`grhyrule.c`).

use super::{mapobj, objects_of, top_n, MapQuery, Obstacle, Registry, StageInit};
use crate::fighter::Fighter;
use crate::map::{self};
use crate::rng;
use crate::status::{AnyStatus, Status};
use crate::weapon::{MapSurface, MapSurfaceKind};
use ssb_engine::math::Vec3;

/// `grHyruleTwisterStatus`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TwisterStatus {
    Sleep,
    Wait,
    Summon,
    Move,
    Turn,
    Stop,
    Subside,
}

/// `GRCommonGroundVarsHyrule`. The twister's `DObj` translation is
/// [`Hyrule::twister_pos`]; it exists from Summon until Stop.
#[derive(Debug, Clone, PartialEq)]
pub struct Hyrule {
    pub status: TwisterStatus,
    pub twister_pos: Vec3,
    pub left_edge_x: f32,
    pub right_edge_x: f32,
    pub vel: f32,
    pub wait: u16,
    pub speed_wait: u16,
    pub turn_wait: u16,
    pub line: u16,
    /// `twister_pos_ids`, resolved to positions (at most ten).
    pub positions: [Vec3; 10],
    pub position_count: u8,
}

impl Hyrule {
    /// `grHyruleTwisterInitVars` @ 0x8010A9C8. The source spins forever on
    /// zero or more than ten positions; here the twister never appears.
    pub fn new(init: &StageInit<'_>) -> Self {
        let mut positions = [Vec3::ZERO; 10];
        let mut count = 0;
        for (slot, pos) in positions
            .iter_mut()
            .zip(objects_of(init.map_objects, mapobj::TWISTER))
        {
            *slot = pos;
            count += 1;
        }
        Hyrule {
            status: TwisterStatus::Sleep,
            twister_pos: Vec3::ZERO,
            left_edge_x: 0.0,
            right_edge_x: 0.0,
            vel: 0.0,
            wait: 0,
            speed_wait: 0,
            turn_wait: 0,
            line: 0,
            positions,
            position_count: count,
        }
    }

    fn rearm(&mut self) {
        self.wait = (rng::rand_int_range(1200) + 1600) as u16;
    }

    /// `grHyruleMakeTwister` @ 0x8010A1E4. The particle effect is not
    /// ported, so its allocation cannot fail here.
    fn make<I, F>(&mut self, pos: Vec3, map: &MapQuery<'_, F>) -> bool
    where
        F: Fn() -> I,
        I: IntoIterator<Item = MapSurface>,
    {
        let Some((line, dist)) = map::project_floor_line(&map.surfaces, pos) else {
            return false;
        };
        self.twister_pos = Vec3::new(pos.x, pos.y + dist, pos.z);
        self.line = line;
        let edge_x = |right: bool| map::floor_edge(&map.surfaces, line, right).map_or(0.0, |p| p.x);
        let under = |right: bool| map::floor_edge_under(&map.surfaces, line, right);
        self.left_edge_x = if under(false) == Some(MapSurfaceKind::RightWall) {
            edge_x(false) + 300.0
        } else {
            edge_x(false)
        };
        self.right_edge_x = if under(true) == Some(MapSurfaceKind::LeftWall) {
            edge_x(true) - 300.0
        } else {
            edge_x(true)
        };
        true
    }

    /// `grHyruleTwisterGetLR` @ 0x8010A52C.
    fn players_lr(&self, fighters: &[&mut Fighter]) -> i32 {
        let (mut right, mut left) = (0, 0);
        for f in fighters.iter() {
            if f.is_grounded() && f.floor.is_some_and(|s| s.line == self.line) {
                if top_n(f).x > self.twister_pos.x {
                    right += 1;
                } else {
                    left += 1;
                }
            }
        }
        if left == 0 && right == 0 {
            0
        } else if left == right {
            if !rng::rand_ushort().is_multiple_of(2) {
                -1
            } else {
                1
            }
        } else if right < left {
            -1
        } else {
            1
        }
    }

    /// `grHyruleTwisterDecLifetimeCheckStop`.
    fn lifetime_ended(&mut self) -> bool {
        self.wait = self.wait.wrapping_sub(1);
        if self.wait == 0 {
            self.status = TwisterStatus::Stop;
            true
        } else {
            false
        }
    }

    /// `grHyruleTwisterProcUpdate` @ 0x8010A91C.
    pub fn tick<I, F>(
        &mut self,
        fighters: &[&mut Fighter],
        registry: &mut Registry,
        map: &MapQuery<'_, F>,
        started: bool,
    ) where
        F: Fn() -> I,
        I: IntoIterator<Item = MapSurface>,
    {
        match self.status {
            TwisterStatus::Sleep => {
                if started {
                    self.status = TwisterStatus::Wait;
                    self.rearm();
                }
            }
            TwisterStatus::Wait => {
                self.wait = self.wait.wrapping_sub(1);
                if self.wait == 0 {
                    if self.position_count == 0 {
                        self.rearm();
                        return;
                    }
                    let i = rng::rand_int_range(i32::from(self.position_count)) as usize;
                    if self.make(self.positions[i], map) {
                        self.wait = 80;
                        self.status = TwisterStatus::Summon;
                    } else {
                        self.rearm();
                    }
                }
            }
            TwisterStatus::Summon => {
                self.wait = self.wait.wrapping_sub(1);
                if self.wait == 0 {
                    self.status = TwisterStatus::Move;
                    self.wait = (rng::rand_int_range(600) + 520) as u16;
                    let lr = if !rng::rand_ushort().is_multiple_of(2) {
                        1.0
                    } else {
                        -1.0
                    };
                    self.turn_wait = 0;
                    self.vel = lr * 10.0;
                    self.speed_wait = (rng::rand_int_range(120) + 180) as u16;
                    registry.add_obstacle(Obstacle::Twister);
                    crate::sound::play_fgm(crate::sound::id::nSYAudioFGMHyruleTwisterAppear);

                }
            }
            TwisterStatus::Move => {
                if self.lifetime_ended() {
                    return;
                }
                if self.turn_wait != 0 {
                    self.turn_wait -= 1;
                    if self.turn_wait == 0 {
                        self.vel = if self.vel < 0.0 { -10.0 } else { 10.0 };
                    }
                } else {
                    self.speed_wait = self.speed_wait.wrapping_sub(1);
                    if self.speed_wait == 0 {
                        let lr = self.players_lr(fighters);
                        if lr != 0 && rng::rand_int_range(5) == 0 {
                            self.turn_wait = (rng::rand_int_range(180) + 300) as u16;
                            self.vel = lr as f32 * 50.0;
                        }
                    }
                }
                let x = self.twister_pos.x + self.vel;
                if self.right_edge_x < x || x < self.left_edge_x {
                    self.twister_pos.x = if self.right_edge_x < x {
                        self.right_edge_x - 10.0
                    } else {
                        self.left_edge_x + 10.0
                    };
                    self.status = TwisterStatus::Turn;
                    self.turn_wait = 120;
                } else {
                    self.twister_pos.x = x;
                }
                // `mpCollisionGetFCCommonFloor` then `pos->y += ground_level`.
                if let Some((y, _)) = map::floor_point(&map.surfaces, self.line, self.twister_pos.x)
                {
                    self.twister_pos.y = y;
                }
            }
            TwisterStatus::Turn => {
                if self.lifetime_ended() {
                    return;
                }
                self.turn_wait = self.turn_wait.wrapping_sub(1);
                if self.turn_wait == 0 {
                    self.status = TwisterStatus::Move;
                    self.turn_wait = 0;
                    self.vel = -self.vel;
                }
            }
            TwisterStatus::Stop => {
                if fighters
                    .iter()
                    .any(|f| f.status.status == AnyStatus::Common(Status::Twister))
                {
                    return;
                }
                self.status = TwisterStatus::Subside;
                self.wait = 32;
                registry.clear_obstacle(Obstacle::Twister);
            }
            TwisterStatus::Subside => {
                self.wait = self.wait.wrapping_sub(1);
                if self.wait == 0 {
                    self.status = TwisterStatus::Wait;
                    self.rearm();
                }
            }
        }
    }

    /// `grHyruleTwisterCheckGetDamageKind` @ 0x8010AB74.
    // Out of line: inlined into `Stage::check_obstacles`, LLVM hoists these
    // compares above the `Controller::Hyrule` match and reads another
    // variant's bytes as floats; a NaN pattern traps the PSP FPU (RE-360).
    #[inline(never)]
    pub fn check_twister(&self, f: &Fighter) -> bool {
        if f.hazard.twister_wait != 0
            || f.status.status == AnyStatus::Common(Status::Twister)
            || f.grab.capture_immune
            || crate::hazard::best_hit_status_all(f) != crate::combat::HitStatus::Normal
        {
            return false;
        }
        let pos = top_n(f);
        let dx = (self.twister_pos.x - pos.x).abs();
        let dy = pos.y - self.twister_pos.y;
        dx < 300.0 && dy < 600.0 && dy > -300.0
    }

    /// `grHyruleTwisterCheckGetPosition` @ 0x8010AC70.
    pub fn visible_position(&self) -> Option<Vec3> {
        matches!(self.status, TwisterStatus::Move | TwisterStatus::Turn).then_some(self.twister_pos)
    }
}
