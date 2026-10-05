//! The CPU's behaviours that Training uses — `ftComputerProcStand`,
//! `...ProcWalk`, `...ProcEvade`, `...ProcJump` and `func_ovl3_80137E70` —
//! with the objective check they all start from
//! (`ftComputerGetObjectiveStatus`) and the objectives they hand out:
//! stand, walk, evade, recover and the landing counter-attack. The
//! attacking behaviour and the item objectives live in [`super::attack`].

use ssb_engine::math::{Vec2, Vec3};

use super::{input, Behavior, Computer, LEVEL_MAX};
use crate::dead::StageBounds;
use crate::fighter::{Facing, Fighter, FighterKind, Situation};
use crate::ground::BodyColl;
use crate::item::{ItemKind, ItemWeight};
use crate::map;
use crate::stage_select::gkind;
use crate::status::{
    AnyStatus, BlastZone, DonkeyStatus, FoxStatus, KirbyStatus, NessStatus, PikachuStatus,
    SamusStatus, Status,
};
use crate::weapon::{MapSurface, MapSurfaceKind};

/// `nFTComputerObjective*`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Objective {
    #[default]
    Stand,
    Walk,
    Attack,
    Evade,
    Recover,
    TrackItem,
    UseItem,
    CounterAttack,
    Unknown1,
    Ally,
    Patrol,
    Rush,
}

/// What the CPU knows about one other fighter.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Opponent {
    pub pos: Vec3,
    pub vel_air: Vec3,
    pub status: AnyStatus,
    pub grounded: bool,
    pub floor_line: Option<u16>,
    pub facing: Facing,
    pub damage: u16,
    /// `star_hitstatus == nGMHitStatusInvincible`.
    pub star_invincible: bool,
    /// Holding the Hammer.
    pub has_hammer: bool,
    pub kind: FighterKind,
    /// `damage_coll_size`; see [`damage_size`].
    pub damage_size: Vec2,
    pub tvel_base: f32,
    pub gravity: f32,
}

/// One live hit of a weapon, as `func_ovl3_80135B78` reads it: every
/// weapon whose attack is past `nGMAttackStateNew` and can hit fighters,
/// one entry per `attack_pos`, in link order.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WeaponThreat {
    /// `owner_gobj`'s port.
    pub owner: u8,
    /// `wp->team`.
    pub team: u8,
    /// `attack_pos[i].pos_curr`.
    pub pos: Vec2,
    pub vel_x: f32,
    /// `wp->lr`.
    pub lr: f32,
    /// `attack_coll.size`: the radius, which the check halves again.
    pub size: f32,
}

/// What the CPU reads of one item (`ITStruct`), in link order: the item
/// searches of `ftComputerCheckFindItem`,
/// `ftComputerCheckTargetItemInRange`,
/// `ftComputerCheckTargetItemOrTwister` and `func_ovl3_80135B78`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ItemSight {
    /// The root DObj's `translate`.
    pub pos: Vec3,
    /// `owner_gobj`'s port.
    pub owner: Option<u8>,
    pub team: u8,
    pub kind: ItemKind,
    pub weight: ItemWeight,
    pub is_allow_pickup: bool,
    pub is_damage_all: bool,
    /// `coll_data.floor_line_id` while `ga` is ground.
    pub floor_line: Option<u16>,
    /// `coll_data.map_coll`.
    pub coll: BodyColl,
    pub vel_x: f32,
    pub lr: f32,
    /// `attack_coll.attack_state` is past `New` and its `interact_mask`
    /// has the fighter bit.
    pub attack_live: bool,
    /// `attack_coll.size`, the radius.
    pub attack_size: f32,
    /// `attack_coll.attack_pos[..attack_count].pos_curr`.
    pub attack_count: usize,
    pub attack_pos: [Vec2; crate::item::ATTACK_COLLS],
}

impl ItemSight {
    /// The live attack positions.
    pub fn attacks(&self) -> &[Vec2] {
        &self.attack_pos[..self.attack_count.min(self.attack_pos.len())]
    }
}

/// `damage_coll_size`, the hurtboxes' extent. The map body's width and top
/// stand in for it.
pub fn damage_size(f: &Fighter) -> Vec2 {
    Vec2::new(f.coll.width, f.coll.top)
}

/// The world a CPU reads each frame.
pub struct World<'a, F> {
    /// The live map.
    pub surfaces: F,
    /// `gMPCollisionBounds.current`: the extent of every line.
    pub geometry: BlastZone,
    pub stage: StageBounds,
    /// `gSCManagerBattleState->gkind`.
    pub gkind: Option<u8>,
    /// Every fighter on another team ([`is_opponent`]).
    pub opponents: &'a [Opponent],
    /// Every item, in link order. The searches skip the CPU's own and, with
    /// team attack off, its team's ([`crate::team::TeamRules::spares`]).
    pub items: &'a [ItemSight],
    /// Every weapon's live hits, in link order; the same skips apply.
    pub weapon_threats: &'a [WeaponThreat],
    /// `gSCManagerBattleState->is_team_battle` and `is_team_attack`.
    pub team_rules: crate::team::TeamRules,
    /// `gSCManagerBattleState->game_type == nSCBattleGameType1PGame`.
    pub is_1p_game: bool,
    /// `ftComputerGetOwnWeaponPositionKind(fp, nWPKindPKThunderTrail)`:
    /// the CPU's own first PK Thunder trail in the link.
    pub pk_thunder_trail: Option<Vec2>,
    /// `grHyruleTwisterCheckGetPosition`.
    pub twister: Option<Vec2>,
    /// `grZebesAcidGetLevelInfo`: level and step.
    pub acid: Option<(f32, f32)>,
}

/// `mpCollisionUpdateBounds`: the extent of every line's vertices,
/// `gMPCollisionBounds.current`.
pub fn geometry_bounds<I>(surfaces: I) -> BlastZone
where
    I: IntoIterator<Item = MapSurface>,
{
    let mut b = BlastZone {
        top: f32::MIN,
        bottom: f32::MAX,
        left: f32::MAX,
        right: f32::MIN,
    };
    for s in surfaces {
        let [x1, y1, x2, y2] = s.coords();
        for (x, y) in [(x1, y1), (x2, y2)] {
            b.top = b.top.max(y);
            b.bottom = b.bottom.min(y);
            b.right = b.right.max(x);
            b.left = b.left.min(x);
        }
    }
    b
}

/// What the CPU sees of another fighter.
pub fn opponent(f: &Fighter) -> Opponent {
    Opponent {
        pos: f.pos,
        vel_air: f.physics.vel_air,
        status: f.status.status,
        grounded: f.situation == Situation::Ground,
        floor_line: f.floor.map(|s| s.line),
        facing: f.facing,
        damage: f.damage,
        star_invincible: false,
        has_hammer: crate::item_use::holds_hammer(f),
        kind: f.kind,
        damage_size: damage_size(f),
        tvel_base: f.attributes.tvel_base,
        gravity: f.attributes.gravity,
    }
}

/// Whether `other` belongs in `this` CPU's [`World::opponents`]: the
/// fighter walks of `ftComputerCheckFindTarget`,
/// `ftComputerCheckEvadeDistance`, `ftComputerWaitGetTarget`,
/// `func_ovl3_80135B78` and `ftComputerCheckSetEvadeTarget` skip the CPU
/// itself and every fighter with its `team`. They test no battle rule: a
/// free-for-all's teams are the ports, so there only the CPU is skipped.
pub fn is_opponent(this: &Fighter, other: &Fighter) -> bool {
    other.port != this.port && other.team != this.team
}

/// Whether a status is at or past `nFTCommonStatusWait`: a fighter in play.
fn in_play(s: AnyStatus) -> bool {
    !matches!(s, AnyStatus::Common(c) if (c as u16) < Status::Wait as u16)
}

impl<F, I> World<'_, F>
where
    F: Fn() -> I,
    I: IntoIterator<Item = MapSurface>,
{
    /// `func_ovl2_800F8FFC`: some floor lies below the point.
    pub fn over_stage(&self, pos: Vec3) -> bool {
        map::lines_of((self.surfaces)(), MapSurfaceKind::Floor).any(|(_, s)| {
            let [x1, y1, x2, y2] = s.coords();
            let (lo, hi) = if x1 <= x2 { (x1, x2) } else { (x2, x1) };
            y1.min(y2) <= pos.y + 0.001 && lo <= pos.x && pos.x <= hi
        })
    }

    /// The item and weapon walks' skip: not the CPU's own, nor, with team
    /// attack off, its team's.
    pub(super) fn sees_item(&self, f: &Fighter, it: &ItemSight) -> bool {
        it.owner != Some(f.port) && !self.team_rules.spares(it.team, f.team)
    }

    fn floor_edge(&self, line: u16, right: bool) -> Option<Vec2> {
        map::line_edge(&self.surfaces, MapSurfaceKind::Floor, line, right)
    }

    /// `mpCollisionCheck{Ceil,LWall,RWall}LineCollisionSame`: the first line
    /// of `kind` the segment `from`→`to` crosses.
    pub(super) fn crossing(&self, kind: MapSurfaceKind, from: Vec2, to: Vec2) -> Option<u16> {
        map::lines_of((self.surfaces)(), kind).find_map(|(line, s)| {
            let [x1, y1, x2, y2] = s.coords();
            segments_cross(from, to, Vec2::new(x1, y1), Vec2::new(x2, y2)).then_some(line)
        })
    }
}

fn segments_cross(a: Vec2, b: Vec2, c: Vec2, d: Vec2) -> bool {
    let cross = |o: Vec2, p: Vec2, q: Vec2| (p.x - o.x) * (q.y - o.y) - (p.y - o.y) * (q.x - o.x);
    let d1 = cross(c, d, a);
    let d2 = cross(c, d, b);
    let d3 = cross(a, b, c);
    let d4 = cross(a, b, d);
    ((d1 > 0.0) != (d2 > 0.0)) && ((d3 > 0.0) != (d4 > 0.0))
}

fn sq(v: f32) -> f32 {
    v * v
}

fn distance(a: f32, b: f32) -> f32 {
    (a - b).abs()
}

fn common(f: &Fighter) -> Option<Status> {
    match f.status.status {
        AnyStatus::Common(s) => Some(s),
        _ => None,
    }
}

impl Computer {
    /// `ftComputerSetupAll`'s map half: the home point on the floor below
    /// an airborne fighter, its floor line, and the cliff scan.
    pub fn setup_world<F, I>(&mut self, f: &Fighter, world: &World<'_, F>)
    where
        F: Fn() -> I,
        I: IntoIterator<Item = MapSurface>,
    {
        let below = map::project_floor_line(&world.surfaces, f.pos);
        if f.situation != Situation::Ground {
            let dist = below.map_or(0.0, |(_, d)| d);
            self.origin_pos.y = f.pos.y + dist;
            self.target_pos.y = self.origin_pos.y;
        }
        if self.floor_line.is_none() {
            self.floor_line = below.map(|(line, _)| line);
        }
        self.setup_cliffs(world);
    }

    /// `ftComputerSetupAll`'s cliff scan: the lowest edges near the stage's
    /// sides, the recovery aims.
    pub fn setup_cliffs<F, I>(&mut self, world: &World<'_, F>)
    where
        F: Fn() -> I,
        I: IntoIterator<Item = MapSurface>,
    {
        let mut nearest_left = 2000.0_f32;
        let mut nearest_right = -2000.0_f32;
        self.cliff_left = Vec2::new(0.0, 9999.9);
        self.cliff_right = Vec2::new(0.0, 9999.9);
        let mut lines: [Option<u16>; 64] = [None; 64];
        let mut n = 0;
        for (line, s) in map::lines_of((world.surfaces)(), MapSurfaceKind::Floor) {
            // `mpCollisionCheckExistPlatformLineID`: a moving group's line.
            if s.motion.is_some() || lines[..n].contains(&Some(line)) || n == lines.len() {
                continue;
            }
            lines[n] = Some(line);
            n += 1;
        }
        for line in lines[..n].iter().flatten() {
            if let Some(e) = world.floor_edge(*line, false) {
                if nearest_left > e.x {
                    nearest_left = e.x;
                    if self.cliff_left.y > e.y {
                        self.cliff_left = e;
                    }
                } else if e.x - world.geometry.left < 500.0 && self.cliff_left.y > e.y {
                    self.cliff_left = e;
                }
            }
            if let Some(e) = world.floor_edge(*line, true) {
                if nearest_right < e.x {
                    nearest_right = e.x;
                    if self.cliff_right.y > e.y {
                        self.cliff_right = e;
                    }
                } else if world.geometry.right - e.x < 500.0 && self.cliff_right.y > e.y {
                    self.cliff_right = e;
                }
            }
        }
    }

    /// `ftComputerProcessAll`: count down, pick a behaviour and an
    /// objective when idle, then run the command script.
    pub fn process<F, I>(&mut self, f: &Fighter, world: &World<'_, F>)
    where
        F: Fn() -> I,
        I: IntoIterator<Item = MapSurface>,
    {
        if f.kind == FighterKind::Boss {
            return;
        }
        self.behavior_change_wait = self.behavior_change_wait.saturating_sub(1);
        if self.input_wait == 0 {
            self.process_trait(f);
            // `ftComputerProcessObjective`.
            let proceed = match self.behavior {
                Behavior::Stand => self.proc_stand(f, world),
                Behavior::Walk => self.proc_walk(f, world),
                Behavior::Evade => self.proc_evade(f, world),
                Behavior::Jump => self.proc_jump(f, world),
                Behavior::Unk5 => self.proc_origin(f, world),
                _ => self.proc_default(f, world),
            };
            if proceed != 0 {
                self.follow_objective(f, world);
            }
        }
        let target_status = self
            .target_user
            .and_then(|i| world.opponents.get(i))
            .map(|o| o.status);
        self.update_inputs(
            f,
            &super::Senses {
                target_status,
                pk_thunder_trail: world.pk_thunder_trail,
            },
        );
    }

    /// The objective switch of `ftComputerProcessObjective`.
    fn follow_objective<F, I>(&mut self, f: &Fighter, world: &World<'_, F>)
    where
        F: Fn() -> I,
        I: IntoIterator<Item = MapSurface>,
    {
        match self.objective {
            Objective::Stand => {
                self.input_wait = 1;
                self.command = Some((input::STICK_N, 0));
            }
            Objective::Walk => self.follow_walk(f, world),
            Objective::Evade => {
                if self.check_set_evade_target(f, world) {
                    self.follow_walk(f, world);
                }
            }
            Objective::Recover => self.follow_recover(f, world),
            Objective::CounterAttack => self.follow_counter_attack(f, world),
            Objective::TrackItem => self.follow_track_item(f, world),
            Objective::UseItem => self.follow_use_item(f, world),
            Objective::Attack | Objective::Unknown1 | Objective::Ally | Objective::Patrol => {
                self.follow_attack(f, world, self.objective)
            }
            Objective::Rush => self.follow_rush(f, world),
        }
    }

    /// `ftComputerCheckFindTarget`: the nearest opponent in play on the
    /// stage, or one hanging on a ledge when this fighter stands.
    pub fn find_target<F, I>(&mut self, f: &Fighter, world: &World<'_, F>) -> bool
    where
        F: Fn() -> I,
        I: IntoIterator<Item = MapSurface>,
    {
        let mut best = f32::MAX;
        for (i, o) in world.opponents.iter().enumerate() {
            let on_stage = world.over_stage(o.pos)
                && o.pos.x <= world.geometry.right
                && o.pos.x >= world.geometry.left
                && o.pos.y >= world.geometry.bottom
                && o.pos.y < world.stage.camera.top;
            let ledge = f.situation == Situation::Ground
                && matches!(
                    o.status,
                    AnyStatus::Common(Status::CliffCatch | Status::CliffWait)
                );
            // Metal Mario only targets grounded opponents
            // (`this_fp->fkind != nFTKindMMario || other_fp->ga == Ground`).
            let kind_ok = f.kind != FighterKind::MetalMario || o.grounded;
            if in_play(o.status) && (on_stage || ledge) && kind_ok {
                let d = sq(f.pos.x - o.pos.x) + sq(f.pos.y - o.pos.y);
                if d < best {
                    self.target_pos = Vec2::new(o.pos.x, o.pos.y);
                    self.target_user = Some(i);
                    best = d;
                }
            }
        }
        if best == f32::MAX {
            self.target_line = None;
            self.target_dist = f32::MAX;
            self.stop_at_ledged_target = false;
            return false;
        }
        self.stop_at_ledged_target = true;
        self.target_dist = ssb_engine::math::sqrt(best);
        let o = world.opponents[self.target_user.unwrap_or(0)];
        self.target_line = if o.grounded { o.floor_line } else { None };
        true
    }

    /// `ftComputerGetObjectiveStatus`: 0 keeps the command just set, 1
    /// means a new objective was given, -1 lets the behaviour decide.
    pub(super) fn objective_status<F, I>(&mut self, f: &Fighter, world: &World<'_, F>) -> i32
    where
        F: Fn() -> I,
        I: IntoIterator<Item = MapSurface>,
    {
        let grounded = f.situation == Situation::Ground;
        let slack = i32::from(LEVEL_MAX - self.level.min(LEVEL_MAX));
        let status = f.status.status;
        if status == AnyStatus::Common(Status::CliffWait) {
            let fall_wait = f.cliff.fall_wait;
            let mut wait = if fall_wait > 480 {
                1080 - fall_wait
            } else {
                480 - fall_wait
            };
            if crate::rng::rand_float() < 0.01 {
                wait *= 2;
            }
            if slack * 15 < wait {
                let script = if crate::rng::rand_float() < 0.4 {
                    input::STICK_N_BUTTON_B_Z_RELEASE_A_PRESS
                } else if crate::rng::rand_float() < 0.5 {
                    input::BUTTON_Z1
                } else {
                    input::MOVE_AUTO_STICK_TILT_HI_RELEASE_Z
                };
                self.set_command_wait_short(grounded, script);
                return 0;
            }
        }
        if matches!(
            status,
            AnyStatus::Common(Status::DownWaitD | Status::DownWaitU)
        ) {
            let wait = 180 - f.reaction.stand_wait;
            if slack * 25 < wait {
                self.find_target(f, world);
                // Giant Donkey Kong never rolls out of a down
                // (`this_fp->fkind != nFTKindGDonkey`).
                if f.kind != FighterKind::GiantDonkey
                    && self.target_dist < 800.0
                    && self.level >= 4
                    && crate::rng::rand_float() * f32::from(11 - self.level.min(10)) < 1.0
                {
                    let script = if self.target_pos.x < f.pos.x {
                        input::ESCAPE_L
                    } else {
                        input::ESCAPE_R
                    };
                    self.set_command_wait_short(grounded, script);
                    return 0;
                }
                self.set_command_wait_short(grounded, input::STICK_N_BUTTON_B_Z_RELEASE_A_PRESS);
                return 0;
            }
        }
        if matches!(
            status,
            AnyStatus::Common(Status::OttottoWait | Status::Ottotto | Status::SquatWait)
        ) {
            self.set_command_wait_short(grounded, input::MOVE_AUTO_STICK_TILT_HI_RELEASE_Z);
            return 0;
        }
        if status == AnyStatus::Common(Status::CatchWait) {
            let script = if f.pos.x < 0.0 {
                input::STICK_SMASH_L
            } else {
                input::STICK_SMASH_R
            };
            self.set_command_immediate(script);
            return 0;
        }
        if matches!(
            status,
            AnyStatus::Common(
                Status::CaptureWaitKirby
                    | Status::YoshiEgg
                    | Status::FuraFura
                    | Status::FuraSleep
                    | Status::Shouldered
            )
        ) {
            self.target_find_wait += 1;
            if (slack * 15) < i32::from(self.target_find_wait) {
                self.set_command_wait_short(grounded, input::WIGGLE);
                return 0;
            }
        } else {
            self.target_find_wait = 0;
        }
        if f.kind == FighterKind::Kirby {
            match status {
                AnyStatus::Kirby(
                    KirbyStatus::SpecialNCatch
                    | KirbyStatus::SpecialAirNCatch
                    | KirbyStatus::SpecialNEat
                    | KirbyStatus::SpecialAirNEat,
                ) => {
                    self.set_command_immediate(input::STICK_N);
                    return 0;
                }
                AnyStatus::Kirby(KirbyStatus::SpecialNWait | KirbyStatus::SpecialAirNWait) => {
                    self.set_command_wait_short(grounded, input::STICK_SMASH_LW_BUTTON_B);
                    return 0;
                }
                _ => {}
            }
        }
        if f.kind == FighterKind::Ness
            && matches!(
                status,
                AnyStatus::Ness(NessStatus::SpecialHiHold | NessStatus::SpecialAirHiHold)
            )
        {
            if !world.over_stage(f.pos) {
                self.target_pos.x = f.pos.x + if f.pos.x > 0.0 { 200.0 } else { -200.0 };
                self.target_pos.y = f.pos.y - 100.0;
            } else {
                self.find_target(f, world);
            }
            self.set_command_immediate(input::NESS_SPECIAL_HI_AIM);
            return 0;
        }
        if grounded {
            let moved =
                sq(f.pos.x - self.stand_pos.x) + sq(f.pos.y - self.stand_pos.y) > 100.0 * 100.0;
            if moved || self.behavior == Behavior::Stand {
                self.stand_pos = Vec2::new(f.pos.x, f.pos.y);
                self.stand_stop_wait = 0;
                self.is_stop_stand = false;
            } else {
                self.stand_stop_wait += 1;
                if self.stand_stop_wait > 300 {
                    self.is_stop_stand = true;
                }
            }
        } else {
            self.stand_stop_wait = 0;
            self.is_stop_stand = false;
        }
        if self.is_stop_stand {
            self.set_command_immediate(input::MOVE_AUTO_STICK_TILT_HI_RELEASE_Z);
            return 0;
        }
        let inishie_low_platform = world.gkind == Some(gkind::INISHIE)
            && f.floor.is_some()
            && f.pos.y < -100.0
            && self.on_platform(f, world);
        if !world.over_stage(f.pos) || inishie_low_platform {
            if self.objective != Objective::Recover {
                self.is_attempt_specialhi_recovery = false;
            }
            self.objective = Objective::Recover;
            return 1;
        }
        if world.gkind == Some(gkind::ZEBES) {
            if let Some((level, step)) = world.acid {
                if f.pos.y < level + 500.0 || f.pos.y < level + 5.0 * step + 500.0 {
                    self.objective = Objective::Recover;
                    return 1;
                }
            }
        }
        self.is_within_vertical_bounds = false;
        if self.objective == Objective::Recover {
            self.objective = Objective::Walk;
        }
        let floor_dist = map::project_floor_line(&world.surfaces, f.pos).map_or(0.0, |(_, d)| d);
        if status == AnyStatus::Common(Status::DamageFall)
            && f.physics.vel_air.y * 5.0 + -floor_dist <= 0.0
        {
            if !self.landing_checked {
                self.landing_checked = true;
                if self.level >= 3
                    && crate::rng::rand_float() * f32::from(325 - u16::from(self.level) * 25) < 70.0
                {
                    self.objective = Objective::CounterAttack;
                    self.is_counterattack = true;
                    return 1;
                }
            }
        } else {
            self.landing_checked = false;
        }
        -1
    }

    fn on_platform<F, I>(&self, f: &Fighter, world: &World<'_, F>) -> bool
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

    /// `ftComputerProcStand`.
    fn proc_stand<F, I>(&mut self, f: &Fighter, world: &World<'_, F>) -> i32
    where
        F: Fn() -> I,
        I: IntoIterator<Item = MapSurface>,
    {
        let status = self.objective_status(f, world);
        self.stop_at_ledged_target = false;
        if status == 0 || status == 1 {
            return status;
        }
        if f.status.status == AnyStatus::Common(Status::RebirthWait) {
            self.target_pos = self.origin_pos;
            self.target_line = self.floor_line;
            self.objective = Objective::Walk;
        } else {
            self.objective = Objective::Stand;
        }
        1
    }

    fn stand_when_near(&mut self, f: &Fighter) {
        self.objective = Objective::Walk;
        let d = sq(self.target_pos.x - f.pos.x) + sq(self.target_pos.y - f.pos.y);
        if d < 100.0 * 100.0 {
            self.objective = Objective::Stand;
        }
    }

    /// `ftComputerProcWalk`: pace to random points along the home floor.
    fn proc_walk<F, I>(&mut self, f: &Fighter, world: &World<'_, F>) -> i32
    where
        F: Fn() -> I,
        I: IntoIterator<Item = MapSurface>,
    {
        let status = self.objective_status(f, world);
        self.stop_at_ledged_target = false;
        if status == 0 || status == 1 {
            return status;
        }
        self.target_pos.y = self.origin_pos.y;
        self.target_line = self.floor_line;
        if distance(self.target_pos.x, f.pos.x) < 100.0 {
            self.target_pos.x = (((2.0 * crate::rng::rand_float() as f64) - 1.0) * 2500.0) as f32
                + self.origin_pos.x;
            match self.floor_line {
                Some(line) => {
                    let left = world.floor_edge(line, false);
                    let right = world.floor_edge(line, true);
                    match (left, right) {
                        (Some(l), Some(r)) => {
                            if self.target_pos.x < l.x {
                                self.target_pos = l;
                            }
                            if self.target_pos.x > r.x {
                                self.target_pos = r;
                            }
                        }
                        _ => self.target_pos.x = self.origin_pos.x,
                    }
                }
                None => self.target_pos.x = self.origin_pos.x,
            }
        }
        self.stand_when_near(f);
        1
    }

    /// `ftComputerProcEvade`.
    fn proc_evade<F, I>(&mut self, f: &Fighter, world: &World<'_, F>) -> i32
    where
        F: Fn() -> I,
        I: IntoIterator<Item = MapSurface>,
    {
        let status = self.objective_status(f, world);
        self.stop_at_ledged_target = false;
        if status == 0 || status == 1 {
            return status;
        }
        self.objective = Objective::Evade;
        1
    }

    /// `ftComputerProcJump`: jump from the home point every 30–60 ticks.
    fn proc_jump<F, I>(&mut self, f: &Fighter, world: &World<'_, F>) -> i32
    where
        F: Fn() -> I,
        I: IntoIterator<Item = MapSurface>,
    {
        let status = self.objective_status(f, world);
        self.stop_at_ledged_target = false;
        if status == 0 || status == 1 {
            return status;
        }
        self.target_pos.x = self.origin_pos.x;
        if self.jump_wait == 0 {
            self.jump_wait = (crate::rng::rand_float() * 30.0 + 30.0) as u16;
            self.target_pos.y = self.origin_pos.y + 1100.0;
            self.target_line = None;
        } else {
            self.target_pos.y = self.origin_pos.y;
            self.target_line = self.floor_line;
            self.jump_wait -= 1;
        }
        self.stand_when_near(f);
        1
    }

    /// `func_ovl3_80137E70`: walk back to the home point.
    fn proc_origin<F, I>(&mut self, f: &Fighter, world: &World<'_, F>) -> i32
    where
        F: Fn() -> I,
        I: IntoIterator<Item = MapSurface>,
    {
        let status = self.objective_status(f, world);
        self.stop_at_ledged_target = false;
        if status == 0 || status == 1 {
            return status;
        }
        self.target_pos = self.origin_pos;
        self.target_line = self.floor_line;
        self.stand_when_near(f);
        1
    }

    /// `ftComputerCheckTargetItemOrTwister`: steer clear of a live item
    /// attack or the Twister.
    fn avoid_item_or_twister<F, I>(&mut self, f: &Fighter, world: &World<'_, F>) -> bool
    where
        F: Fn() -> I,
        I: IntoIterator<Item = MapSurface>,
    {
        let px = f.pos.x + f.physics.vel_air.x * 5.0;
        let py = f.pos.y + f.physics.vel_air.y * 5.0;
        for it in world.items {
            if !world.sees_item(f, it)
                || !((it.kind == ItemKind::MSBomb && it.is_damage_all) || it.attack_live)
            {
                continue;
            }
            // Only the first item with an attack is weighed.
            let Some(&at) = it.attacks().first() else {
                continue;
            };
            let size = it.attack_size;
            if (px - at.x).abs() < size && (py - at.y).abs() < size {
                self.stop_at_ledged_target = false;
                if f.situation != Situation::Ground {
                    self.target_pos.x = if self.target_pos.x < at.x {
                        at.x + 1500.0
                    } else {
                        at.x - 1500.0
                    };
                    self.target_pos.y = at.y;
                } else {
                    self.target_pos = Vec2::new(f.pos.x, f.pos.y + 1100.0);
                }
            }
            return true;
        }
        if world.gkind == Some(gkind::HYRULE) {
            if let Some(t) = world.twister {
                let dy = py - t.y;
                if (px - t.x).abs() < 600.0 && dy < 600.0 && dy > -300.0 {
                    self.stop_at_ledged_target = false;
                    let x = if f.pos.x < t.x - 300.0 {
                        t.x - 1200.0
                    } else {
                        t.x + 1200.0
                    };
                    self.target_pos = Vec2::new(x, t.y + 1100.0);
                    return true;
                }
            }
        }
        false
    }

    /// `ftComputerFollowObjectiveWalk`: go around ceilings and walls between
    /// here and the target, then move toward it — jumping, fast-falling or
    /// using up special to get there.
    pub fn follow_walk<F, I>(&mut self, f: &Fighter, world: &World<'_, F>)
    where
        F: Fn() -> I,
        I: IntoIterator<Item = MapSurface>,
    {
        let grounded = f.situation == Situation::Ground;
        let pos = Vec2::new(f.pos.x, f.pos.y);
        let floor_dist = map::project_floor_line(&world.surfaces, f.pos).map_or(0.0, |(_, d)| d);
        let floor = f.floor;
        let pass = floor.is_some_and(|s| s.flags & crate::collision::flags::PASS != 0);
        if grounded && pos.y < self.target_pos.y {
            if let Some(line) = world.crossing(MapSurfaceKind::Ceiling, pos, self.target_pos) {
                let l = map::line_edge(&world.surfaces, MapSurfaceKind::Ceiling, line, false);
                let r = map::line_edge(&world.surfaces, MapSurfaceKind::Ceiling, line, true);
                if let (Some(l), Some(r)) = (l, r) {
                    self.target_pos.x = if distance(l.x, pos.x) < distance(r.x, pos.x) {
                        l.x - 600.0
                    } else {
                        r.x + 600.0
                    };
                    self.target_pos.y = pos.y;
                }
            }
        } else if !pass
            && floor.is_some()
            && self.target_pos.y < (pos.y + floor_dist) - 500.0
            && distance(self.target_pos.x, pos.x) < distance(self.target_pos.y, pos.y) * 0.2
        {
            let line = floor.map_or(0, |s| s.line);
            if let (Some(l), Some(r)) =
                (world.floor_edge(line, false), world.floor_edge(line, true))
            {
                if distance(l.x, pos.x) < distance(r.x, pos.x) {
                    self.target_pos = Vec2::new(l.x - 600.0, l.y);
                } else {
                    self.target_pos = Vec2::new(r.x + 600.0, r.y);
                }
            }
        } else if grounded
            || f.physics.vel_air.y < 0.0
            || matches!(f.kind, FighterKind::Kirby | FighterKind::Purin)
        {
            if pos.x < self.target_pos.x {
                if let Some(line) = world.crossing(MapSurfaceKind::LeftWall, pos, self.target_pos) {
                    if let Some(top) =
                        map::line_edge(&world.surfaces, MapSurfaceKind::LeftWall, line, true)
                    {
                        self.target_pos = Vec2::new(top.x + 100.0, top.y + 100.0);
                        if grounded && f.facing == Facing::Left {
                            self.target_pos = Vec2::new(pos.x + 100.0, pos.y);
                            self.set_command_immediate(input::STICK_TILT_AUTO_X_D5);
                            return;
                        }
                    }
                }
            } else if let Some(line) =
                world.crossing(MapSurfaceKind::RightWall, pos, self.target_pos)
            {
                if let Some(top) =
                    map::line_edge(&world.surfaces, MapSurfaceKind::RightWall, line, true)
                {
                    self.target_pos = Vec2::new(top.x - 100.0, top.y + 100.0);
                    if grounded && f.facing == Facing::Right {
                        self.target_pos = Vec2::new(pos.x - 100.0, pos.y);
                        self.set_command_immediate(input::STICK_TILT_AUTO_X_D5);
                        return;
                    }
                }
            }
        }
        self.avoid_item_or_twister(f, world);
        self.set_command_immediate(input::MOVE_AUTO);
        if !grounded {
            self.follow_walk_air(f, world);
        } else {
            self.follow_walk_ground(f, world, floor.map(|s| s.line), pass);
        }
    }

    fn follow_walk_air<F, I>(&mut self, f: &Fighter, world: &World<'_, F>)
    where
        F: Fn() -> I,
        I: IntoIterator<Item = MapSurface>,
    {
        let pos = f.pos;
        match self.behavior {
            Behavior::YoshiTeam if pos.y < 0.0 => return,
            Behavior::KirbyTeam | Behavior::PolyTeam if pos.y < -300.0 => return,
            _ => {}
        }
        let status = common(f);
        let jumps_left = f.physics.jumps_used < f.attributes.jumps_max;
        if pos.y < self.target_pos.y {
            if self.objective != Objective::Recover && self.target_pos.y - 200.0 < pos.y {
                self.target_pos.y = pos.y;
                return;
            }
            self.fastfall_checked = false;
            if pos.y < world.stage.map.top - 4000.0 && f.physics.vel_air.y < 0.0 {
                let flying = matches!(
                    status,
                    Some(
                        Status::DamageFlyHi
                            | Status::DamageFlyN
                            | Status::DamageFlyLw
                            | Status::DamageFlyTop
                            | Status::DamageFlyRoll
                    )
                );
                if jumps_left && flying && f.hitstun == 0 {
                    self.set_command_immediate(input::MOVE_AUTO_STICK_TILT_HI_RELEASE_Z);
                    return;
                }
                if jumps_left
                    && matches!(
                        status,
                        Some(
                            Status::JumpF
                                | Status::JumpB
                                | Status::Fall
                                | Status::DamageFall
                                | Status::FallSpecial
                        )
                    )
                {
                    self.set_command_immediate(input::MOVE_AUTO_STICK_TILT_HI_RELEASE_Z);
                    return;
                }
                if self.objective == Objective::Recover
                    && !self.is_attempt_specialhi_recovery
                    && !matches!(f.kind, FighterKind::Yoshi | FighterKind::Purin)
                    && matches!(
                        status,
                        Some(
                            Status::JumpAerialF
                                | Status::JumpAerialB
                                | Status::FallAerial
                                | Status::DamageFall
                        )
                    )
                {
                    self.is_attempt_specialhi_recovery = true;
                    // Giant Donkey Kong always tries (and, short-circuiting,
                    // draws no random number).
                    if f.kind == FighterKind::GiantDonkey
                        || crate::rng::rand_float() < f32::from(self.level + 2) / 9.0
                    {
                        self.set_command_immediate(input::STICK_SMASH_HI_BUTTON_B);
                        return;
                    }
                }
            }
        } else if f.physics.vel_air.y < 0.0
            && self.objective != Objective::Recover
            && !f.physics.is_fastfall
            && !self.fastfall_checked
        {
            self.fastfall_checked = true;
            let chance = self.level > 5
                && crate::rng::rand_float() * f32::from(550 - u16::from(self.level) * 50) < 70.0;
            if chance
                || matches!(
                    self.behavior,
                    Behavior::YoshiTeam | Behavior::KirbyTeam | Behavior::PolyTeam
                )
            {
                self.set_command_wait_short(false, input::STICK_N_D1_MOVE_AUTO_SMASH_LW);
                return;
            }
        }
        if matches!(
            self.behavior,
            Behavior::YoshiTeam | Behavior::KirbyTeam | Behavior::PolyTeam
        ) && status == Some(Status::Fall)
            && world.over_stage(pos)
        {
            self.target_pos = Vec2::new(pos.x, pos.y - 500.0);
        }
        if self.level >= 5
            && self.stop_at_ledged_target
            && self.target_pos.y + 1100.0 < pos.y
            && f.physics.vel_air.y < 0.0
        {
            if let Some(t) = self.target_user.and_then(|i| world.opponents.get(i)) {
                let side = t.facing.sign() * 1500.0;
                let behind = Vec3::new(t.pos.x - side, t.pos.y, 0.0);
                if world.over_stage(behind) {
                    self.target_pos = Vec2::new(behind.x, behind.y);
                } else {
                    let front = Vec3::new(t.pos.x + side, t.pos.y, 0.0);
                    if world.over_stage(front) {
                        self.target_pos = Vec2::new(front.x, front.y);
                    }
                }
            }
        }
        if self.stop_at_ledged_target {
            if let Some(line) = self.target_line {
                if let (Some(l), Some(r)) =
                    (world.floor_edge(line, false), world.floor_edge(line, true))
                {
                    if l.x <= self.target_pos.x && r.x >= self.target_pos.x {
                        self.target_pos.x = self.target_pos.x.max(l.x + 200.0).min(r.x - 200.0);
                    }
                }
            }
        }
        self.set_command_immediate(input::MOVE_AUTO);
    }

    fn follow_walk_ground<F, I>(
        &mut self,
        f: &Fighter,
        world: &World<'_, F>,
        floor_line: Option<u16>,
        pass: bool,
    ) where
        F: Fn() -> I,
        I: IntoIterator<Item = MapSurface>,
    {
        let pos = f.pos;
        let kneebend = common(f) == Some(Status::KneeBend);
        if self.dash_predict <= distance(self.target_pos.x, pos.x) {
            if pos.y < self.target_pos.y {
                let edge_dist = floor_line
                    .and_then(|line| world.floor_edge(line, f.physics.vel_air.x >= 0.0))
                    .map_or(0.0, |e| distance(e.x, pos.x));
                if edge_dist - f.physics.vel_air.x.abs() < 200.0 {
                    if !kneebend {
                        self.set_command_wait_short(true, input::MOVE_AUTO_STICK_TILT_HI_RELEASE_Z);
                    }
                } else {
                    self.set_command_immediate(input::MOVE_AUTO);
                }
            } else {
                self.set_command_immediate(input::MOVE_AUTO);
            }
        } else if common(f) != Some(Status::Dash) {
            if self.target_line != floor_line {
                if pos.y < self.target_pos.y {
                    if !kneebend {
                        self.set_command_wait_short(true, input::MOVE_AUTO_STICK_TILT_HI_RELEASE_Z);
                    }
                } else if pass {
                    self.set_command_wait_short(true, input::STICK_N_D1_MOVE_AUTO_SMASH_LW);
                }
            } else {
                self.set_command_immediate(input::MOVE_AUTO);
            }
        } else {
            self.set_command_immediate(input::MOVE_AUTO);
        }
    }

    /// `ftComputerCheckTryCancelSpecialN`: let go of a charging neutral
    /// special, Kirby's copied ones included. The source's switch reads
    /// Kirby's `copy_id` first and lists Donkey Kong with Giant Donkey Kong
    /// (`case nFTKindDonkey: case nFTKindGDonkey:`) and Samus; only those
    /// kinds (and Kirby with that copy) can be in these statuses, so
    /// checking the statuses directly is the same test.
    pub(super) fn try_cancel_special_n(&mut self, f: &Fighter) -> bool {
        let charging = matches!(
            f.status.status,
            AnyStatus::Donkey(
                DonkeyStatus::SpecialNStart
                    | DonkeyStatus::SpecialAirNStart
                    | DonkeyStatus::SpecialNLoop
                    | DonkeyStatus::SpecialAirNLoop
            ) | AnyStatus::Samus(
                SamusStatus::SpecialNStart
                    | SamusStatus::SpecialAirNStart
                    | SamusStatus::SpecialNLoop
                    | SamusStatus::SpecialAirNEnd
            ) | AnyStatus::Kirby(
                KirbyStatus::CopyDonkeySpecialNStart
                    | KirbyStatus::CopyDonkeySpecialAirNStart
                    | KirbyStatus::CopyDonkeySpecialNLoop
                    | KirbyStatus::CopyDonkeySpecialAirNLoop
                    | KirbyStatus::CopySamusSpecialNStart
                    | KirbyStatus::CopySamusSpecialAirNStart
                    | KirbyStatus::CopySamusSpecialNLoop
                    | KirbyStatus::CopySamusSpecialAirNEnd
            )
        );
        if charging {
            self.set_command_wait_short(f.situation == Situation::Ground, input::BUTTON_Z1);
        }
        charging
    }

    /// `ftComputerFollowObjectiveRecover`.
    fn follow_recover<F, I>(&mut self, f: &Fighter, world: &World<'_, F>)
    where
        F: Fn() -> I,
        I: IntoIterator<Item = MapSurface>,
    {
        if self.try_cancel_special_n(f) {
            return;
        }
        self.aim_recovery(f, world);
        match f.status.status {
            AnyStatus::Pikachu(PikachuStatus::SpecialAirHiStart) => {
                self.target_pos = Vec2::new(f.pos.x, f.pos.y + 1100.0);
            }
            AnyStatus::Pikachu(PikachuStatus::SpecialAirHi) => {
                self.target_pos = Vec2::new(0.0, f.pos.y);
            }
            _ => {}
        }
        self.follow_walk(f, world);
    }

    /// `func_ovl3_80134964`: aim for a reachable edge, else back over the
    /// stage.
    fn aim_recovery<F, I>(&mut self, f: &Fighter, world: &World<'_, F>)
    where
        F: Fn() -> I,
        I: IntoIterator<Item = MapSurface>,
    {
        let pos = f.pos;
        self.target_line = None;
        self.stop_at_ledged_target = false;
        if !self.is_within_vertical_bounds
            && pos.x <= world.geometry.right
            && pos.x >= world.geometry.left
        {
            let order: [(bool, bool); 4] = if f.physics.vel_air.x < 0.0 {
                [(true, false), (false, false), (true, true), (false, true)]
            } else {
                [(false, false), (true, false), (false, true), (true, true)]
            };
            for (right, find) in order {
                if self.set_target_edge(f, world, right, find) {
                    return;
                }
            }
            // `(pos.x + (pos.x < 0.0F)) != FALSE`.
            self.target_pos.x = if pos.x + if pos.x < 0.0 { 1.0 } else { 0.0 } != 0.0 {
                500.0
            } else {
                -500.0
            };
            self.target_pos.y = if pos.y + 1100.0 > world.stage.camera.top {
                pos.y
            } else {
                pos.y + 1100.0
            };
        } else {
            self.aim_back(f, world);
        }
    }

    /// `ftComputerCheckSetTargetEdgeRight` / `...Left`.
    fn set_target_edge<F, I>(
        &mut self,
        f: &Fighter,
        world: &World<'_, F>,
        right: bool,
        find: bool,
    ) -> bool
    where
        F: Fn() -> I,
        I: IntoIterator<Item = MapSurface>,
    {
        let side = if right { -1.0 } else { 1.0 };
        if f.physics.vel_air.y >= 0.0 {
            self.target_pos = Vec2::new(f.pos.x + side * 500.0, f.pos.y);
            return true;
        }
        // Metal Mario (`fkind == nFTKindMMario`) and Saffron City take no
        // offset.
        let edge_offset =
            if f.kind == FighterKind::MetalMario || world.gkind == Some(gkind::YAMABUKI) {
                0.0
            } else {
                self.jump_predict * 0.75
            };
        let a = &f.attributes;
        let mut seen: [Option<u16>; 64] = [None; 64];
        let mut n = 0;
        for (line, s) in map::lines_of((world.surfaces)(), MapSurfaceKind::Floor) {
            if seen[..n].contains(&Some(line)) || n == seen.len() {
                continue;
            }
            seen[n] = Some(line);
            n += 1;
            let Some(edge) = world.floor_edge(line, right) else {
                continue;
            };
            if world.gkind == Some(gkind::ZEBES) {
                if let Some((level, step)) = world.acid {
                    if edge.y < level + 500.0 || edge.y < 5.0 * step + level + 500.0 {
                        continue;
                    }
                }
            }
            if world.gkind == Some(gkind::INISHIE) && s.motion.is_some() {
                continue;
            }
            let dist = f.pos.x - edge.x;
            if (right && dist > 0.0) || (!right && dist < 0.0) {
                let predict_x = (dist / (-side * a.air_speed_max_x)) as i32 as f32;
                let fall = (-(-a.tvel_base - f.physics.vel_air.y) / a.gravity) as i32;
                let vy = f.physics.vel_air.y;
                let predict_y = if fall <= 0 {
                    f.pos.y + vy * predict_x
                } else if predict_x < fall as f32 {
                    f.pos.y + (vy * predict_x - a.gravity * predict_x * predict_x * 0.5)
                } else {
                    let fall = fall as f32;
                    f.pos.y
                        + ((vy * predict_x - a.gravity * fall * fall * 0.5)
                            - a.tvel_base * (predict_x - fall))
                };
                if !find && predict_y < edge.y - self.jump_predict {
                    continue;
                }
                if predict_y < edge.y && edge.y - edge_offset < predict_y {
                    continue;
                }
                self.target_pos = Vec2::new(edge.x + side * 500.0, edge.y);
                return true;
            }
        }
        false
    }

    /// `func_ovl3_801346D4`: head back toward the stage from past its side,
    /// jumping if the nearest cliff is out of reach.
    fn aim_back<F, I>(&mut self, f: &Fighter, world: &World<'_, F>)
    where
        F: Fn() -> I,
        I: IntoIterator<Item = MapSurface>,
    {
        let pos = f.pos;
        let mut range = self.jump_predict;
        if f.physics.jumps_used == f.attributes.jumps_max {
            match f.kind {
                FighterKind::Fox | FighterKind::Donkey | FighterKind::GiantDonkey => range *= 0.5,
                FighterKind::Ness => range = -self.jump_predict,
                _ => {}
            }
        }
        // Metal Mario and Giant Donkey Kong never hold back for the cliff.
        if matches!(f.kind, FighterKind::MetalMario | FighterKind::GiantDonkey) {
            range = 0.0;
        }
        let jumps_left = f.physics.jumps_used < f.attributes.jumps_max;
        let next_y = pos.y + f.physics.vel_air.y;
        if pos.x > world.geometry.right {
            if f.facing == Facing::Right {
                range *= 0.75;
            }
            self.target_pos.x = pos.x - 1100.0;
            if pos.x < self.cliff_right.x + 300.0 {
                range = 0.0;
            }
            self.target_pos.y = if (jumps_left && pos.x - world.geometry.right > 1500.0)
                || next_y < self.cliff_right.y - range
            {
                pos.y + 1100.0
            } else {
                pos.y
            };
        } else {
            if f.facing == Facing::Left {
                range *= 0.75;
            }
            self.target_pos.x = pos.x + 1100.0;
            if self.cliff_left.x - 300.0 < pos.x {
                range = 0.0;
            }
            self.target_pos.y = if (jumps_left && world.geometry.left - pos.x > 1500.0)
                || next_y < self.cliff_left.y - range
            {
                pos.y + 1100.0
            } else {
                pos.y
            };
        }
    }

    /// `ftComputerFollowObjectiveCounterAttack`: tech the landing from a
    /// damage fall.
    fn follow_counter_attack<F, I>(&mut self, f: &Fighter, world: &World<'_, F>)
    where
        F: Fn() -> I,
        I: IntoIterator<Item = MapSurface>,
    {
        self.counter_target(f);
        if self.is_counterattack {
            self.set_command_immediate(input::BUTTON_Z2);
            self.is_counterattack = false;
        } else if self.is_opponent_ra {
            self.is_opponent_ra = false;
            // `switch (fp->fkind)`: Fox and Polygon Fox, Ness and Polygon
            // Ness reflect unless already scoping; everyone else does
            // nothing. A Polygon is never in its model's special statuses,
            // so it always reflects.
            let scoping = match f.kind {
                FighterKind::Fox | FighterKind::PolyFox => matches!(
                    f.status.status,
                    AnyStatus::Fox(s) if (FoxStatus::SpecialLwStart as u16
                        ..=FoxStatus::SpecialAirLwTurn as u16)
                        .contains(&(s as u16))
                ),
                FighterKind::Ness | FighterKind::PolyNess => matches!(
                    f.status.status,
                    AnyStatus::Ness(s) if (NessStatus::SpecialLwStart as u16
                        ..=NessStatus::SpecialAirLwEnd as u16)
                        .contains(&(s as u16))
                ),
                _ => true,
            };
            if !scoping {
                self.set_command_immediate(input::STICK_N_X_SMASH_LW_BUTTON_B_RELEASE_B_HOLD);
            }
        } else if self.is_shield_item_weapon {
            self.is_shield_item_weapon = false;
            if !matches!(
                f.status.status,
                AnyStatus::Common(
                    Status::GuardOn | Status::Guard | Status::GuardOff | Status::GuardSetOff
                )
            ) {
                self.set_command_immediate(input::STICK_N_BUTTON_Z_HOLD);
            }
        } else {
            self.follow_walk(f, world);
        }
    }

    /// `func_ovl3_801361BC`: where to go when a hit is on its way.
    fn counter_target(&mut self, f: &Fighter) {
        self.target_line = None;
        self.stop_at_ledged_target = false;
        if self.is_counterattack || self.is_opponent_ra {
            return;
        }
        if f.situation != Situation::Ground {
            self.target_pos.x = f.pos.x;
            // The map body's top stands in for `damage_coll_size.y`, the
            // hurtboxes' height.
            self.target_pos.y =
                if f.pos.y + f.coll.top < self.target_pos.y && f.physics.vel_air.y <= 0.0 {
                    f.pos.y - 1100.0
                } else {
                    f.pos.y + 1100.0
                };
        } else {
            // Donkey Kong, Polygon Donkey Kong and Giant Donkey Kong 11,
            // Metal Mario 20, everyone else 7.
            let frames = match f.kind {
                FighterKind::Donkey | FighterKind::PolyDonkey | FighterKind::GiantDonkey => 11.0,
                FighterKind::MetalMario => 20.0,
                _ => 7.0,
            };
            if self.hit_predict < frames || matches!(common(f), Some(Status::Run | Status::Dash)) {
                self.is_shield_item_weapon = true;
            } else {
                self.target_pos = Vec2::new(f.pos.x, f.pos.y + 1100.0);
            }
        }
    }

    /// `ftComputerCheckSetEvadeTarget`: stay 2,500 units from the nearest
    /// threat, turning back at the floor's end.
    fn check_set_evade_target<F, I>(&mut self, f: &Fighter, world: &World<'_, F>) -> bool
    where
        F: Fn() -> I,
        I: IntoIterator<Item = MapSurface>,
    {
        const FAR: f32 = 6_250_000.0;
        let mut nearest = FAR;
        self.stop_at_ledged_target = false;
        for (i, o) in world.opponents.iter().enumerate() {
            let px = o.pos.x + o.vel_air.x * 3.0;
            // `vel_air.x` in the source's y prediction too.
            let py = o.pos.y + o.vel_air.x * 3.0;
            if o.star_invincible || o.has_hammer {
                self.target_user = Some(i);
                nearest = sq(f.pos.x - px) + sq(f.pos.y - py);
                break;
            } else if in_play(o.status)
                && px <= world.geometry.right
                && px >= world.geometry.left
                && py >= world.geometry.bottom
            {
                let d = sq(f.pos.x - px) + sq(f.pos.y - py);
                if nearest > d {
                    self.target_user = Some(i);
                    nearest = d;
                }
            }
        }
        if nearest == FAR {
            self.target_pos = Vec2::new(f.pos.x, f.pos.y);
            self.target_line = None;
            return true;
        }
        let Some(t) = self
            .target_user
            .and_then(|i| world.opponents.get(i))
            .copied()
        else {
            return true;
        };
        let edge = match f.floor {
            Some(s) => world
                .floor_edge(s.line, f.pos.x >= t.pos.x)
                .unwrap_or(Vec2::new(f.pos.x, f.pos.y)),
            None => Vec2::new(f.pos.x, f.pos.y),
        };
        self.target_pos.y = if t.grounded { t.pos.y } else { f.pos.y };
        let grounded = f.situation == Situation::Ground;
        if f.pos.x < t.pos.x {
            self.target_pos.x = t.pos.x - 2500.0;
            if self.target_pos.x < edge.x {
                self.target_pos.x = t.pos.x + 2500.0;
                self.set_command_wait_short(grounded, input::MOVE_AUTO_STICK_TILT_HI_RELEASE_Z);
                return false;
            }
        } else {
            self.target_pos.x = t.pos.x + 2500.0;
            if self.target_pos.x > edge.x {
                self.target_pos.x = t.pos.x - 2500.0;
                self.set_command_wait_short(grounded, input::MOVE_AUTO_STICK_TILT_HI_RELEASE_Z);
                return false;
            }
        }
        self.target_dist = ssb_engine::math::sqrt(nearest);
        self.target_line = None;
        self.objective = Objective::Evade;
        true
    }
}

#[cfg(test)]
#[path = "behave_tests.rs"]
mod tests;
