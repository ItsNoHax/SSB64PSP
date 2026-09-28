//! The CPU's fighting behaviour — `ftComputerProcDefault`, the default
//! trait's behaviour changes (`ftComputerProcessTrait`), the attack chooser
//! `ftComputerCheckDetectTarget` over the per-fighter attack tables, and the
//! Attack, Unknown1, Ally and Patrol objectives with their helpers
//! (`func_ovl3_8013837C`, `...8013877C`, `...80138AA8`, `...80138EE4`,
//! `ftComputerCheckTryChargeSpecialN`, `ftComputerCheckEvadeDistance`).
//! Item tracking and use are not ported; with no items in play they never
//! apply.

use ssb_engine::math::{Vec2, Vec3};

use super::behave::{Objective, World};
use super::scripts::{Attack, ATTACKS};
use super::{input, Behavior, Computer, LEVEL_MAX};
use crate::fighter::{Facing, Fighter, FighterKind, Situation};
use crate::map;
use crate::stage_select::gkind;
use crate::status::{
    AnyStatus, DonkeyStatus, FoxStatus, KirbyStatus, NessStatus, SamusStatus, Status,
};
use crate::weapon::{MapSurface, MapSurfaceKind};

/// `FTDONKEY_GIANTPUNCH_CHARGE_MAX`.
const DONKEY_CHARGE_MAX: u8 = 10;

/// `nFTComputerTrait*`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Trait {
    /// `nFTComputerTraitDefault`: VS. The behaviour changes every 900–1,800
    /// ticks.
    #[default]
    Default,
    /// `nFTComputerTraitNone`: Training keeps the menu's behaviour.
    None,
}

fn sq(v: f32) -> f32 {
    v * v
}

fn common(f: &Fighter) -> Option<Status> {
    match f.status.status {
        AnyStatus::Common(s) => Some(s),
        _ => None,
    }
}

fn damage_flying(s: AnyStatus) -> bool {
    matches!(
        s,
        AnyStatus::Common(
            Status::DamageFlyHi
                | Status::DamageFlyN
                | Status::DamageFlyLw
                | Status::DamageFlyTop
                | Status::DamageFall
                | Status::DamageFlyRoll
        )
    )
}

/// The fighter a Kirby's copied moves stand for, else the fighter.
fn effective_kind(f: &Fighter) -> FighterKind {
    if f.kind == FighterKind::Kirby {
        f.kirby.copy_id
    } else {
        f.kind
    }
}

fn samus_charging(s: AnyStatus) -> bool {
    matches!(
        s,
        AnyStatus::Samus(
            SamusStatus::SpecialNStart
                | SamusStatus::SpecialAirNStart
                | SamusStatus::SpecialNLoop
                | SamusStatus::SpecialAirNEnd
        ) | AnyStatus::Kirby(
            KirbyStatus::CopySamusSpecialNStart
                | KirbyStatus::CopySamusSpecialAirNStart
                | KirbyStatus::CopySamusSpecialNLoop
                | KirbyStatus::CopySamusSpecialAirNEnd
        )
    )
}

fn donkey_charging(s: AnyStatus) -> bool {
    matches!(
        s,
        AnyStatus::Donkey(
            DonkeyStatus::SpecialNStart
                | DonkeyStatus::SpecialAirNStart
                | DonkeyStatus::SpecialNLoop
                | DonkeyStatus::SpecialAirNLoop
        ) | AnyStatus::Kirby(
            KirbyStatus::CopyDonkeySpecialNStart
                | KirbyStatus::CopyDonkeySpecialAirNStart
                | KirbyStatus::CopyDonkeySpecialNLoop
                | KirbyStatus::CopyDonkeySpecialAirNLoop
        )
    )
}

/// The eleven per-input weight counters `ftComputerCheckDetectTarget`
/// raises for moves it passed over.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AttackCounts(pub [u8; 11]);

fn count_slot(input_kind: usize) -> Option<usize> {
    Some(match input_kind {
        input::STICK_N_BUTTON_A => 0,
        input::STICK_TILT_AUTO_X_BUTTON_A => 1,
        input::STICK_SMASH_AUTO_X_N_Y_BUTTON_A => 2,
        input::STICK_TILT_HI_BUTTON_A => 3,
        input::STICK_SMASH_HI_BUTTON_A => 4,
        input::STICK_TILT_LW_BUTTON_A => 5,
        input::STICK_SMASH_LW_BUTTON_A => 6,
        input::STICK_SMASH_AUTO_X_BUTTON_B => 7,
        input::STICK_SMASH_HI_BUTTON_B => 8,
        input::STICK_SMASH_LW_BUTTON_B => 9,
        input::STICK_N_BUTTON_Z_BUTTON_A => 10,
        _ => return None,
    })
}

impl Computer {
    /// `ftComputerProcessTrait` for the default trait, then
    /// `ftComputerProcessBehavior`'s base objective.
    pub(super) fn process_trait(&mut self) {
        if self.trait_kind == Trait::Default && self.behavior_change_wait == 0 {
            self.behavior_change_wait = (crate::rng::rand_float() * 900.0 + 900.0) as u16;
            self.behavior = match crate::rng::rand_ushort() & 3 {
                0 => Behavior::Default,
                1 => Behavior::Unk2,
                2 => {
                    self.behavior_change_wait >>= 2;
                    Behavior::Ally
                }
                _ => Behavior::Captain,
            };
        }
        self.objective_base = match self.behavior {
            Behavior::Unk1 => Objective::Evade,
            Behavior::Unk2 => Objective::Unknown1,
            Behavior::Ally | Behavior::YoshiTeam => Objective::Ally,
            Behavior::Captain | Behavior::KirbyTeam => Objective::Patrol,
            Behavior::Bonus3 => Objective::Rush,
            _ => Objective::Attack,
        };
    }

    /// `ftComputerProcDefault`.
    pub(super) fn proc_default<F, I>(&mut self, f: &Fighter, world: &World<'_, F>) -> i32
    where
        F: Fn() -> I,
        I: IntoIterator<Item = MapSurface>,
    {
        let result = self.objective_status(f, world);
        if result == 0 || result == 1 {
            return result;
        }
        if self.check_evade_distance(f, world) {
            self.objective = Objective::Evade;
            return 1;
        }
        if self.check_weapon_threat(f, world) {
            self.objective = Objective::CounterAttack;
            return 1;
        }
        let grounded = f.situation == Situation::Ground;
        let scoping = match f.status.status {
            AnyStatus::Fox(s) => (FoxStatus::SpecialLwStart as u16
                ..=FoxStatus::SpecialAirLwTurn as u16)
                .contains(&(s as u16)),
            AnyStatus::Ness(s) => (NessStatus::SpecialLwStart as u16
                ..=NessStatus::SpecialAirLwEnd as u16)
                .contains(&(s as u16)),
            _ => false,
        };
        if scoping {
            self.set_command_wait_short(grounded, input::STICK_N_BUTTON_B_RELEASE);
            return 0;
        }
        if matches!(
            common(f),
            Some(Status::GuardOn | Status::Guard | Status::GuardOff | Status::GuardSetOff)
        ) {
            self.set_command_wait_short(grounded, input::BUTTON_Z_RELEASE);
            return 0;
        }
        self.find_target(f, world);
        if self.target_dist < 350.0 {
            self.objective = Objective::Attack;
            return 1;
        }
        // `ftComputerCheckFindItem`: no items are in play.
        self.item_track_wait = 0;
        self.objective = self.objective_base;
        1
    }

    /// `ftComputerCheckEvadeDistance`: a star-invincible opponent within
    /// 1,500, or one with the Hammer within 2,500.
    fn check_evade_distance<F, I>(&self, f: &Fighter, world: &World<'_, F>) -> bool
    where
        F: Fn() -> I,
        I: IntoIterator<Item = MapSurface>,
    {
        world.opponents.iter().any(|o| {
            let x = o.pos.x + o.vel_air.x * 3.0;
            // `vel_air.x` in the source's y prediction too.
            let y = o.pos.y + o.vel_air.x * 3.0;
            let d = ssb_engine::math::sqrt(sq(f.pos.y - y) + sq(f.pos.x - x));
            (o.star_invincible && d < 1500.0) || (o.has_hammer && d < 2500.0)
        })
    }

    /// `func_ovl3_80135B78`: an opponent's weapon about to connect.
    fn check_weapon_threat<F, I>(&mut self, f: &Fighter, world: &World<'_, F>) -> bool
    where
        F: Fn() -> I,
        I: IntoIterator<Item = MapSurface>,
    {
        for t in world.weapon_threats {
            let vel = (t.vel_x - f.physics.vel_air.x) * t.lr;
            if vel <= 0.0 {
                continue;
            }
            let half = t.size * 0.5;
            let gap = (f.pos.x - t.pos.x) * t.lr - (super::behave::damage_size(f).x + half);
            if gap <= 0.0 {
                continue;
            }
            let frames = gap / vel;
            if frames >= 15.0 {
                continue;
            }
            let y = if f.situation != Situation::Ground {
                f.physics.vel_air.y * frames + f.pos.y
            } else {
                f.pos.y
            };
            if (t.pos.y - half) - super::behave::damage_size(f).y < y && y < t.pos.y + half {
                self.target_pos.y = y;
                self.hit_predict = frames;
                if crate::rng::rand_float() < f32::from(self.level + 2) / 9.0 {
                    return true;
                }
            }
        }
        false
    }

    /// `func_ovl3_8013837C`: pick the lowest-damage opponent after a wait,
    /// else follow the nearest one. -1 means a special was charged or let
    /// go instead.
    fn follow_target<F, I>(&mut self, f: &Fighter, world: &World<'_, F>) -> i32
    where
        F: Fn() -> I,
        I: IntoIterator<Item = MapSurface>,
    {
        if self.behavior != Behavior::YoshiTeam {
            if let Some(i) = self.wait_get_target(world) {
                let o = world.opponents[i];
                self.target_user = Some(i);
                self.target_pos = Vec2::new(o.pos.x, o.pos.y);
                self.target_dist =
                    ssb_engine::math::sqrt(sq(f.pos.x - o.pos.x) + sq(f.pos.y - o.pos.y));
                self.stop_at_ledged_target = true;
                self.target_line = if o.grounded { o.floor_line } else { None };
                return 1;
            }
        }
        let slack = f32::from(LEVEL_MAX - self.level.min(LEVEL_MAX));
        if !self.find_target(f, world) || self.follow_since == 0 {
            self.follow_since = 1;
            self.follow_wait =
                u16::from((crate::rng::rand_float() * slack * 4.0) as u8) + (slack as u16) * 16;
            if self.behavior == Behavior::YoshiTeam {
                self.follow_wait =
                    (f32::from(self.follow_wait) + 300.0 + crate::rng::rand_float() * 300.0) as u16;
            }
            self.follow_end =
                (crate::rng::rand_float() * 120.0 + f32::from(self.follow_wait + 120)) as u16;
            return 0;
        }
        self.follow_since += 1;
        if self.follow_wait < self.follow_since {
            if self.follow_end < self.follow_since {
                self.follow_since = 0;
            }
            if self.target_dist < 1200.0 {
                if self.try_cancel_special_n(f) {
                    return -1;
                }
            } else if f.situation == Situation::Ground && self.try_charge_special_n(f) {
                return -1;
            }
            return 1;
        }
        0
    }

    /// `ftComputerWaitGetTarget`: stay on the lowest-damage opponent for
    /// 600–900 ticks while its damage holds.
    fn wait_get_target<F>(&mut self, world: &World<'_, F>) -> Option<usize> {
        let mut best = 9999;
        let mut pick = None;
        for (i, o) in world.opponents.iter().enumerate() {
            // `nFTCommonStatusControlStart`.
            if matches!(o.status, AnyStatus::Common(s) if (s as u16) < Status::Wait as u16) {
                continue;
            }
            if self.wait_target == Some(i) {
                if o.damage == self.wait_target_damage {
                    self.wiggle_wait = self.wiggle_wait.saturating_sub(1);
                    return (self.wiggle_wait == 0).then_some(i);
                }
                self.wait_target = None;
            }
            if i32::from(o.damage) < best {
                best = i32::from(o.damage);
                pick = Some(i);
            }
        }
        if let Some(i) = pick {
            self.wait_target_damage = best as u16;
            self.wait_target = Some(i);
            self.wiggle_wait = (crate::rng::rand_float() * 300.0 + 600.0) as u16;
        }
        None
    }

    /// `ftComputerCheckTryChargeSpecialN`: start charging a neutral special
    /// that is not full.
    fn try_charge_special_n(&mut self, f: &Fighter) -> bool {
        let s = f.status.status;
        let charge = match f.kind {
            FighterKind::Donkey => {
                !donkey_charging(s)
                    && !matches!(
                        s,
                        AnyStatus::Donkey(
                            DonkeyStatus::SpecialLwStart
                                | DonkeyStatus::SpecialLwLoop
                                | DonkeyStatus::SpecialLwEnd
                        )
                    )
                    && f.donkey_special_n.charge_level < DONKEY_CHARGE_MAX
            }
            FighterKind::Samus => {
                !samus_charging(s) && f.samus.charge_level < crate::samus::CHARGE_MAX
            }
            FighterKind::Kirby => match f.kirby.copy_id {
                FighterKind::Donkey => {
                    !donkey_charging(s)
                        && f.kirby.copy.donkey_charge_level
                            < crate::kirby_copy::GIANTPUNCH_CHARGE_MAX
                }
                FighterKind::Samus => {
                    !samus_charging(s)
                        && f.kirby.copy.samus_charge_level < crate::kirby_copy::CHARGE_MAX
                }
                _ => false,
            },
            _ => false,
        };
        if charge {
            self.set_command_wait_short(
                f.situation == Situation::Ground,
                input::STICK_TILT_AUTO_X_N_Y_D1_BUTTON_B,
            );
        }
        charge
    }

    /// `func_ovl3_8013877C`: stand a moment, then wander around home.
    fn idle_wander<F, I>(&mut self, f: &Fighter, world: &World<'_, F>)
    where
        F: Fn() -> I,
        I: IntoIterator<Item = MapSurface>,
    {
        self.walk_stop_wait += 1;
        if self.walk_stop_wait >= 30 {
            if self.walk_stop_wait == 30 {
                if let Some(s) = f.floor {
                    self.floor_line = Some(s.line);
                    self.origin_pos = Vec2::new(f.pos.x, f.pos.y);
                }
                self.edge_pos = self.origin_pos;
            }
            self.target_line = self.floor_line;
            let edges = self.floor_line.and_then(|line| {
                Some((
                    map::line_edge(&world.surfaces, MapSurfaceKind::Floor, line, false)?,
                    map::line_edge(&world.surfaces, MapSurfaceKind::Floor, line, true)?,
                ))
            });
            match edges {
                None => {
                    if self.find_target(f, world) {
                        self.follow_walk(f, world);
                    }
                }
                Some((l, r)) => {
                    if (self.edge_pos.x - f.pos.x).abs() < 100.0 {
                        self.edge_pos.x = (((2.0 * crate::rng::rand_float() as f64) - 1.0) * 2500.0)
                            as f32
                            + self.origin_pos.x;
                        if self.edge_pos.x < l.x {
                            self.edge_pos = l;
                        }
                        if self.edge_pos.x > r.x {
                            self.edge_pos = r;
                        }
                    }
                    self.target_pos = self.edge_pos;
                    self.stop_at_ledged_target = false;
                    self.follow_walk(f, world);
                    if self.walk_stop_wait > 150 {
                        self.walk_stop_wait = 0;
                    }
                }
            }
        } else {
            self.input_wait = 1;
            self.command = Some((input::STICK_N, 0));
            let at = 18 - i32::from(self.level) * 2;
            if i32::from(self.walk_stop_wait) == at && f.situation == Situation::Ground {
                self.try_charge_special_n(f);
                match f.kind {
                    FighterKind::Link
                        if self.find_target(f, world) && self.target_dist < 1500.0 =>
                    {
                        self.set_command_wait_short(true, input::STICK_SMASH_LW_BUTTON_B);
                    }
                    FighterKind::Ness if crate::rng::rand_float() < 0.25 => {
                        self.set_command_wait_short(true, input::STICK_SMASH_HI_BUTTON_B);
                    }
                    _ => {}
                }
            }
        }
    }

    /// `func_ovl3_80138AA8`: fire a projectile special at a target on the
    /// same level, when the way is clear.
    fn try_projectile<F, I>(&mut self, f: &Fighter, world: &World<'_, F>, delay: bool) -> bool
    where
        F: Fn() -> I,
        I: IntoIterator<Item = MapSurface>,
    {
        if (f.pos.y - self.target_pos.y).abs() >= 400.0 {
            return false;
        }
        let slack = f32::from(LEVEL_MAX - self.level.min(LEVEL_MAX));
        if self.projectile_count == 0 {
            self.projectile_count = (2.0 * (crate::rng::rand_float() * slack)) as u8;
        }
        let target_kind = self
            .target_user
            .and_then(|i| world.opponents.get(i))
            .map(|o| o.kind);
        if crate::rng::rand_float() < f32::from(self.level.saturating_sub(1)) / 9.0
            && matches!(target_kind, Some(FighterKind::Ness | FighterKind::Fox))
        {
            return false;
        }
        let kind = effective_kind(f);
        if kind == FighterKind::Samus && samus_charging(f.status.status) {
            return false;
        }
        match kind {
            FighterKind::Link
            | FighterKind::Mario
            | FighterKind::Fox
            | FighterKind::Samus
            | FighterKind::Luigi
            | FighterKind::Pikachu => {}
            _ => return false,
        }
        if kind == FighterKind::Link && self.target_dist < 1500.0 && crate::rng::rand_float() < 0.3
        {
            self.set_command_wait_short(
                f.situation == Situation::Ground,
                input::STICK_SMASH_LW_BUTTON_B,
            );
        }
        self.projectile_count += 1;
        if self.projectile_count >= 5 {
            self.follow_walk(f, world);
            return true;
        }
        let from = Vec2::new(f.pos.x, f.pos.y);
        let wall = if self.target_pos.x < f.pos.x {
            MapSurfaceKind::RightWall
        } else {
            MapSurfaceKind::LeftWall
        };
        for kind in [wall, MapSurfaceKind::Floor, MapSurfaceKind::Ceiling] {
            if world.crossing(kind, from, self.target_pos).is_some() {
                return false;
            }
        }
        let script = if delay {
            input::STICK_TILT_AUTO_X_N_Y_D5_SMASH_AUTO_X_BUTTON_B
        } else {
            input::STICK_SMASH_AUTO_X_BUTTON_B
        };
        self.set_command_wait_long(f.situation == Situation::Ground, script);
        true
    }

    /// `func_ovl3_80138EE4`: roll behind a close target.
    fn try_roll<F, I>(&mut self, f: &Fighter, world: &World<'_, F>) -> bool
    where
        F: Fn() -> I,
        I: IntoIterator<Item = MapSurface>,
    {
        let kind = effective_kind(f);
        if kind == FighterKind::Samus && samus_charging(f.status.status) {
            return false;
        }
        if kind == FighterKind::Donkey
            && (donkey_charging(f.status.status)
                || matches!(
                    f.status.status,
                    AnyStatus::Donkey(
                        DonkeyStatus::ThrowFWait
                            | DonkeyStatus::ThrowFWalkSlow
                            | DonkeyStatus::ThrowFWalkMiddle
                            | DonkeyStatus::ThrowFWalkFast
                            | DonkeyStatus::ThrowFTurn
                    )
                ))
        {
            return false;
        }
        let target = self
            .target_user
            .and_then(|i| world.opponents.get(i))
            .copied();
        if self.stop_at_ledged_target
            && target.is_some_and(|t| {
                matches!(
                    t.status,
                    AnyStatus::Common(Status::CliffCatch | Status::CliffWait)
                )
            })
        {
            return false;
        }
        let random = crate::rng::rand_float();
        let facing = f.facing.sign();
        let behind_me = (self.target_pos.x - f.pos.x) * facing < 0.0;
        let near = self.target_dist < random * 450.0 + 350.0
            && ((self.level >= 4
                && crate::rng::rand_float() * f32::from(11 - self.level.min(10)) < 1.0)
                || (self.level >= 3 && behind_me));
        let behind_them = self.target_dist < 800.0
            && self.level >= 7
            && crate::rng::rand_float() * f32::from(10 - self.level.min(9)) < 1.0
            && target.is_some_and(|t| (self.target_pos.x - f.pos.x) * t.facing.sign() < 0.0);
        if f.situation != Situation::Ground || !(near || behind_them) {
            return false;
        }
        if let Some(s) = f.floor {
            let l = map::line_edge(&world.surfaces, MapSurfaceKind::Floor, s.line, false);
            let r = map::line_edge(&world.surfaces, MapSurfaceKind::Floor, s.line, true);
            if let (Some(l), Some(r)) = (l, r) {
                if self.target_pos.x < f.pos.x {
                    if f.pos.x < l.x + 500.0 {
                        return false;
                    }
                } else if f.pos.x > r.x - 500.0 {
                    return false;
                }
            }
        }
        let script = if self.target_pos.x < f.pos.x {
            input::ESCAPE_L
        } else {
            input::ESCAPE_R
        };
        self.set_command_wait_short(true, script);
        true
    }

    /// `ftComputerCheckDetectTarget`: every attack of the fighter's table
    /// whose box around the fighter will hold the target when its hitbox
    /// comes out, weighted, then one picked at random.
    fn detect_target<F, I>(&mut self, f: &Fighter, world: &World<'_, F>, range_base: f32) -> bool
    where
        F: Fn() -> I,
        I: IntoIterator<Item = MapSurface>,
    {
        let Some(target) = self
            .target_user
            .and_then(|i| world.opponents.get(i))
            .copied()
        else {
            return false;
        };
        if world.gkind == Some(gkind::INISHIE) && self.on_moving_floor(f, world) {
            return false;
        }
        let grounded = f.situation == Situation::Ground;
        if world.gkind == Some(gkind::YAMABUKI) && !grounded {
            let (near, far) = if f.physics.vel_air.x > 0.0 {
                (f.pos.x, f.pos.x + f.physics.vel_air.x * 40.0)
            } else {
                (f.pos.x + f.physics.vel_air.x * 40.0, f.pos.x)
            };
            let mut x = (f.pos.x + near) - 100.0;
            while x < (f.pos.x + far) + 100.0 {
                if !world.over_stage(Vec3::new(x, f.pos.y, 0.0)) {
                    return false;
                }
                x += 100.0;
            }
        }
        let slack = f32::from(LEVEL_MAX - self.level.min(LEVEL_MAX));
        let size_mul = ((crate::rng::rand_float() - 0.5) * slack) * 0.1 + 1.0;
        let hurt_w = target.damage_size.x * size_mul;
        let hurt_h = target.damage_size.y * size_mul;
        let a = &f.attributes;
        let this_tvel = -a.tvel_base;
        let target_tvel = -target.tvel_base;
        let target_gravity = target.gravity;
        let Some(&(ground, air)) = ATTACKS.get(f.kind as usize) else {
            return false;
        };
        let table: &[Attack] = if grounded { ground } else { air };
        let mut kinds = [0usize; 20];
        let mut ranges = [0.0f32; 20];
        let mut count = 0;
        let lr = f.facing.sign();
        for attack in table {
            if attack.hit_start_frame == 0 {
                continue;
            }
            let hit = attack.hit_start_frame as f32;
            let predict_x = ((target.pos.x + target.vel_air.x * hit)
                - (f.pos.x + f.physics.vel_air.x * hit))
                * lr;
            // `-(tvel_base - pos.y) / gravity`, as the source writes it.
            let this_frame = (-(this_tvel - f.pos.y) / a.gravity) as i32;
            let vy = f.physics.vel_air.y;
            let adjust_y = if common(f) == Some(Status::Pass) || this_frame <= 0 {
                vy * hit + f.pos.y
            } else if hit < this_frame as f32 {
                (vy * hit - (a.gravity * hit * hit) * 0.5) + f.pos.y
            } else {
                let tf = this_frame as f32;
                ((hit * vy - (a.gravity * tf * tf) * 0.5) + this_tvel * (hit - tf)) + f.pos.y
            };
            let tvy = target.vel_air.y;
            let predict_y = if target.status != AnyStatus::Common(Status::Pass) && !target.grounded
            {
                let tf = (-(target_tvel - tvy) / target_gravity) as i32;
                if tf <= 0 {
                    (hit * tvy + target.pos.y) - adjust_y
                } else if hit < tf as f32 {
                    ((hit * tvy + target.pos.y) - (target_gravity * hit * hit) * 0.5) - adjust_y
                } else {
                    let tf = tf as f32;
                    (((hit * tvy + target.pos.y) - (target_gravity * tf * tf) * 0.5)
                        + target_tvel * (hit - tf))
                        - adjust_y
                }
            } else {
                (hit * tvy + target.pos.y) - adjust_y
            };
            let (near_x, far_x) = if lr > 0.0 {
                (attack.detect_near_x, attack.detect_far_x)
            } else {
                (-attack.detect_far_x, -attack.detect_near_x)
            };
            let mut near_y = attack.detect_near_y;
            let far_y = attack.detect_far_y;
            let cliffcatch = match f.kind {
                FighterKind::Mario | FighterKind::Luigi => {
                    attack.input == input::STICK_SMASH_HI_BUTTON_B
                }
                FighterKind::Kirby => {
                    attack.input == input::STICK_SMASH_HI_BUTTON_B
                        || attack.input == input::STICK_SMASH_LW_BUTTON_B
                }
                FighterKind::Yoshi | FighterKind::Captain => {
                    attack.input == input::STICK_SMASH_LW_BUTTON_B
                }
                _ => false,
            };
            if cliffcatch {
                let mut x = (f.pos.x + near_x) - 100.0;
                let mut off = false;
                while x < (f.pos.x + far_x) + 100.0 {
                    if !world.over_stage(Vec3::new(x, f.pos.y, 0.0)) {
                        off = true;
                        break;
                    }
                    x += 100.0;
                }
                if off {
                    continue;
                }
                if f.pos.x < self.target_pos.x {
                    if self.cliff_right.x < self.target_pos.x + 1200.0 {
                        continue;
                    }
                } else if self.cliff_left.x > self.target_pos.x - 1200.0 {
                    continue;
                }
            }
            if self.stop_at_ledged_target
                && matches!(
                    target.status,
                    AnyStatus::Common(Status::CliffCatch | Status::CliffWait)
                )
                && near_y < 0.0
            {
                near_y -= 500.0;
            }
            if predict_y < far_y
                && (near_y - hurt_h) < predict_y
                && (near_x - hurt_w) < predict_x
                && predict_x < (far_x + hurt_w)
            {
                if count == kinds.len() {
                    break;
                }
                kinds[count] = attack.input;
                ranges[count] = match attack.input {
                    input::STICK_SMASH_AUTO_X_BUTTON_B => {
                        let reflector_target = self.level >= 5
                            && matches!(target.kind, FighterKind::Ness | FighterKind::Fox)
                            && matches!(
                                effective_kind(f),
                                FighterKind::Mario
                                    | FighterKind::Fox
                                    | FighterKind::Samus
                                    | FighterKind::Luigi
                                    | FighterKind::Link
                                    | FighterKind::Pikachu
                            );
                        if reflector_target {
                            continue;
                        }
                        let full = match f.kind {
                            FighterKind::Donkey => {
                                f.donkey_special_n.charge_level == DONKEY_CHARGE_MAX
                            }
                            FighterKind::Samus => f.samus.charge_level == crate::samus::CHARGE_MAX,
                            FighterKind::Kirby => match f.kirby.copy_id {
                                FighterKind::Donkey => {
                                    f.kirby.copy.donkey_charge_level
                                        == crate::kirby_copy::GIANTPUNCH_CHARGE_MAX
                                }
                                FighterKind::Samus => {
                                    f.kirby.copy.samus_charge_level == crate::kirby_copy::CHARGE_MAX
                                }
                                _ => true,
                            },
                            _ => false,
                        };
                        if full {
                            4.0
                        } else {
                            1.0 + range_base
                        }
                    }
                    input::STICK_SMASH_HI_BUTTON_B | input::STICK_SMASH_LW_BUTTON_B => {
                        range_base * 0.5 + 1.0
                    }
                    input::STICK_N_BUTTON_Z_BUTTON_A
                        if !matches!(f.kind, FighterKind::Link | FighterKind::Samus) =>
                    {
                        4.0
                    }
                    _ => 1.0,
                };
                count += 1;
            }
        }
        if count == 0 {
            return false;
        }
        if self.level < 3
            && f32::from(f.damage) + 5.0
                < crate::rng::rand_float() * (200.0 - f32::from(self.level) * 50.0)
        {
            self.follow_since = 0;
            return false;
        }
        let mut total = 0.0;
        for i in 0..count {
            if self.input_kind == Some(kinds[i]) {
                ranges[i] *= 0.25;
            }
            if let Some(slot) = count_slot(kinds[i]) {
                ranges[i] += f32::from(self.attack_counts.0[slot]) * 0.2;
                self.attack_counts.0[slot] = self.attack_counts.0[slot].saturating_add(1);
            }
            ranges[i] += total;
            total = ranges[i];
        }
        let pick = crate::rng::rand_float() * total;
        for i in 0..count {
            if pick < ranges[i] {
                if self.input_kind == Some(kinds[i]) {
                    self.input_repeat_count += 1;
                    if self.input_repeat_count >= 4 {
                        self.set_command_immediate(input::MOVE_AUTO_STICK_TILT_HI_RELEASE_Z);
                        return true;
                    }
                } else {
                    self.input_repeat_count = 0;
                }
                self.set_command_wait_short(grounded, kinds[i]);
                self.input_kind = Some(kinds[i]);
                if let Some(slot) = count_slot(kinds[i]) {
                    self.attack_counts.0[slot] = 0;
                }
                if f.kind == FighterKind::Purin
                    && kinds[i] == input::STICK_SMASH_HI_BUTTON_B
                    && crate::rng::rand_float() < 0.9
                {
                    return false;
                }
                return true;
            }
        }
        false
    }

    fn on_moving_floor<F, I>(&self, f: &Fighter, world: &World<'_, F>) -> bool
    where
        F: Fn() -> I,
        I: IntoIterator<Item = MapSurface>,
    {
        let Some(line) = f.floor.map(|s| s.line) else {
            return false;
        };
        map::lines_of((world.surfaces)(), MapSurfaceKind::Floor)
            .any(|(l, s)| l == line && s.motion.is_some())
    }

    /// The shared body of `ftComputerFollowObjectiveAttack`,
    /// `func_ovl3_801397F4`, `...Ally` and `...Patrol`: close in, then roll,
    /// attack, chase a launched target or wander; from afar, shoot or walk.
    pub(super) fn follow_attack<F, I>(
        &mut self,
        f: &Fighter,
        world: &World<'_, F>,
        objective: Objective,
    ) where
        F: Fn() -> I,
        I: IntoIterator<Item = MapSurface>,
    {
        let result = self.follow_target(f, world);
        if result == 0 {
            self.idle_wander(f, world);
            return;
        }
        if result == -1 {
            return;
        }
        crate::rng::rand_float();
        crate::rng::rand_float();
        let random = crate::rng::rand_float();
        let facing_target = (f.pos.x < self.target_pos.x && f.facing == Facing::Right)
            || (f.pos.x > self.target_pos.x && f.facing == Facing::Left);
        let delay = !facing_target;
        let (range_base, chase_chance, burst) = match objective {
            Objective::Unknown1 => (0.0, f32::from(self.level + 5) / 9.0, None),
            Objective::Ally => (-0.5, 0.0, Some((0.01, 30.0))),
            Objective::Patrol => (2.0, f32::from(self.level + 2) / 9.0, Some((0.05, 15.0))),
            _ => (0.0, f32::from(self.level + 2) / 9.0, Some((0.05, 30.0))),
        };
        let target = self
            .target_user
            .and_then(|i| world.opponents.get(i))
            .copied();
        if self.target_dist < random * 300.0 + 1200.0 {
            if self.try_roll(f, world) {
                self.projectile_count = 0;
                return;
            }
            if self.detect_target(f, world, range_base) {
                if objective != Objective::Unknown1 && objective != Objective::Ally {
                    self.walk_burst = 0;
                }
                self.projectile_count = 0;
                return;
            }
            if objective == Objective::Ally {
                self.idle_wander(f, world);
                return;
            }
            if let Some(t) = target {
                if t.vel_air.y > 30.0
                    && crate::rng::rand_float() < chase_chance
                    && damage_flying(t.status)
                {
                    self.set_command_immediate(input::MOVE_AUTO_STICK_TILT_HI_RELEASE_Z);
                    self.walk_stop_wait = 0;
                    return;
                }
            }
            if objective == Objective::Unknown1 {
                self.follow_walk(f, world);
                self.walk_stop_wait = 0;
                return;
            }
            self.walk_or_wander(f, world, burst.unwrap_or((0.05, 30.0)), false);
        } else {
            self.item_throw_wait = 0;
            if self.try_projectile(f, world, delay) {
                self.walk_stop_wait = 0;
                return;
            }
            if objective == Objective::Unknown1 {
                self.projectile_count = 0;
                self.follow_walk(f, world);
                self.walk_stop_wait = 0;
                return;
            }
            let burst = if objective == Objective::Patrol {
                (0.05, 30.0)
            } else {
                burst.unwrap_or((0.05, 30.0))
            };
            self.walk_or_wander(f, world, burst, true);
        }
    }

    /// Walk to the target on a different level or in a walking burst, else
    /// start a burst at random or wander.
    fn walk_or_wander<F, I>(
        &mut self,
        f: &Fighter,
        world: &World<'_, F>,
        burst: (f32, f32),
        reset_projectile: bool,
    ) where
        F: Fn() -> I,
        I: IntoIterator<Item = MapSurface>,
    {
        if (self.target_pos.y - f.pos.y).abs() > 1500.0 || self.walk_burst != 0 {
            self.follow_walk(f, world);
            self.walk_stop_wait = 0;
            self.walk_burst = self.walk_burst.saturating_sub(1);
        } else if crate::rng::rand_float() < burst.0 {
            self.walk_burst = (crate::rng::rand_float() * 60.0 + burst.1) as u16;
            self.follow_walk(f, world);
            self.walk_stop_wait = 0;
        } else {
            if reset_projectile {
                self.projectile_count = 0;
            }
            self.idle_wander(f, world);
        }
    }
}

#[cfg(test)]
#[path = "attack_tests.rs"]
mod tests;
