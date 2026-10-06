//! Master Hand, the 1P Game's final boss: `ft/ftchar/ftboss/*.c`.
//!
//! Every status of `dFTBossSpecialStatusDescs` is ported with its
//! `proc_update`, `proc_interrupt`, `proc_physics` and `proc_map`, the
//! CPU's attack choice (`ftBossWaitDecideStatusComputer`), the player
//! controls (`ftBossWaitDecideStatusPlayer`), the hit-point bookkeeping
//! (`ftBossCommonUpdateDamageStats`) and the defeat statuses.
//!
//! The port differs from the source's layout in three places:
//!
//! - `proc_update` and `proc_interrupt` read the map, so
//!   [`Fighter::tick_interrupt`] calls [`update`] with the stage's surfaces
//!   instead of `status::update`'s map-free dispatch. The order within the
//!   frame is the source's: animation, motion script, update, interrupt.
//! - `proc_physics` and `proc_map` run from [`tick_status`], which owns
//!   the fighter's position the way `ftMainProcPhysicsMap` does for an
//!   airborne fighter: physics, `translate += vel_air`, then the map
//!   callback of whatever status physics left current.
//! - The target fighter (`ftBossInfo::target_gobj`) is another object; the
//!   host copies its position and floor line into [`BossState::target`]
//!   before Master Hand's tick ([`Target`]).
//!
//! The scene half (`sc1PGameBossDefeatInitInterface`, the camera requests
//! of `gmCameraSetStatusMapZoom`/`...Default`) is raised as flags for the
//! host: [`BossState::defeated`] and [`BossState::camera`].

use crate::fighter::{Facing, Fighter};
use crate::status::{self, AnyStatus, BossStatus as B, StatusTiming};
use crate::weapon::{MapSurface, MapSurfaceKind, WeaponKind, WeaponSpawn};
use ssb_engine::math::{sin_cos, Vec2, Vec3};

#[cfg(test)]
#[path = "boss_tests.rs"]
mod tests;

/// `FTBOSS_ATTACK_WAIT_MAX`.
pub const ATTACK_WAIT_MAX: i32 = 120;
/// `FTBOSS_ATTACK_WAIT_LEVEL_DIV`.
pub const ATTACK_WAIT_LEVEL_DIV: i32 = 100;
/// `FTBOSS_HARAU_VEL_X`.
pub const HARAU_VEL_X: f32 = 50.0;
/// `FTBOSS_OKUHIKOUKI_VEL_ADD`.
pub const OKUHIKOUKI_VEL_ADD: f32 = 40.0;
/// `ftBossCommonUpdateDamageStats`: Master Hand falls at 300 damage and
/// attacks faster (`wait_div = 1.5`) from 200.
pub const HIT_POINTS: u16 = 300;
pub const ENRAGE_DAMAGE: u16 = 200;
/// The `wait_div` of an enraged Master Hand.
pub const ENRAGED_WAIT_DIV: f32 = 1.5;
/// `gmCameraSetStatusMapZoom(&{0, 0, 0}, &{0, 1000, 7000})`: the three
/// background attacks.
pub const BACKGROUND_ZOOM_EYE: Vec3 = Vec3::new(0.0, 1000.0, 7000.0);
/// The background attacks' depth, which `ftBossCommonUpdateFogColor`
/// fades from.
pub const BACKGROUND_Z: f32 = -15000.0;

/// `ssb_rom::anim::SLOT_BOSS_DEFAULT`: the first of Master Hand's thirty
/// motion slots, in `ftBossMotion` order (checked by
/// `crates/ssb-rom/tests/boss.rs`).
pub const SLOT_DEFAULT: usize = 623;

/// Frames of the eight 32-bit `AnimJoint` clips (`FTANIM_FLAG_ANIMJOINT`
/// in `dFTBossMotionDescs`), read from the ROM like the entry clips'
/// (RE-401): the tick their joint streams end on, less one. The figatree
/// statuses read theirs from the generated motion table
/// ([`crate::motion::anim_length`]). `crates/ssb-rom/tests/boss.rs`
/// checks both against the ROM.
pub const LAUNCH_FRAMES: f32 = 120.0;
pub const FLY_FRAMES: f32 = 120.0;
pub const LANDING_FRAMES: f32 = 120.0;
pub const DRILL_FRAMES: f32 = 240.0;
pub const PUNCH3_FRAMES: f32 = 80.0;
pub const PUNCH_END_FRAMES: f32 = 60.0;
pub const SLAM_FRAMES: f32 = 130.0;
pub const APPEAR_FRAMES: f32 = 600.0;

/// The joint the host's defeat zoom centres on: the palm's, the first
/// hurtbox's (`dBossMain_attr.damage_coll_descs[0]`), standing in for the
/// joint last struck (`fp->damage_joint_id`, RE-457).
pub const DEFEAT_ZOOM_JOINT: u8 = 20;

/// `WPYUBIBULLETVEL_X`, `..._Y`.
pub const BULLET_VEL_X: f32 = 160.0;
pub const BULLET_VEL_Y: f32 = -25.0;
/// The joints the finger gun fires from (`fp->joints[15]` on the first
/// shot, `fp->joints[19]` on the follow-up).
pub const BULLET_JOINT_FIRST: u8 = 15;
pub const BULLET_JOINT_FOLLOW: u8 = 19;

/// `ftbosswait.o`'s `.data` (0x80188DC0, 0x40 bytes) and the 16 zero
/// bytes of `ftkirbyspecialn.o` after it, as the US ROM holds them
/// (`crates/ssb-rom/tests/boss.rs`): `dFTBossWaitRandomStatusIDs` (+0x00),
/// `...StatusLookup` (+0x0C), `...Ground` (+0x1C), `...NoGround` (+0x24)
/// and `...ArrayLookup` (+0x2C). `ftBossWaitDecideStatusComputer` indexes
/// past the tables: its first roll reads `ArrayLookup[-3..-1]` (the
/// initial `status_id` is -1), which can pick group 10, whose rows lie in
/// the following bytes. The port reads the same bytes.
pub const WAIT_DATA: [u8; 0x50] = [
    0xDF, 0xE0, 0xE4, 0xE8, 0xEC, 0xEF, 0xF0, 0xE1, 0xF4, 0xF8, 0xF1, 0x00, 0x00, 0x01, 0x02, 0x03,
    0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x04, 0x06, 0x00, 0x00, 0x00, 0x00, 0x03, 0x03, 0x04,
    0x07, 0x03, 0x0A, 0x01, 0x00, 0x03, 0x0B, 0x02, 0x07, 0x03, 0x0A, 0x01, 0x01, 0x02, 0x03, 0x00,
    0x02, 0x03, 0x00, 0x01, 0x03, 0x00, 0x01, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
];
const STATUS_IDS: i32 = 0x00;
const STATUS_LOOKUP: i32 = 0x0C;
const RANDOM_GROUND: i32 = 0x1C;
const RANDOM_NO_GROUND: i32 = 0x24;
const ARRAY_LOOKUP: i32 = 0x2C;

/// The statuses `dFTBossWaitRandomStatusIDs` names.
const ATTACKS: [B; 11] = [
    B::Hippataku,
    B::Harau,
    B::Walk,
    B::GootsubusuUp,
    B::Tsutsuku1,
    B::Drill,
    B::Okukouki,
    B::Okuhikouki1,
    B::Okupunch1,
    B::OkutsubushiStart,
    B::Yubideppou1,
];

/// One byte of [`WAIT_DATA`] by its offset (`.data` beyond reads zero).
fn wait_byte(offset: i32) -> u8 {
    usize::try_from(offset)
        .ok()
        .and_then(|i| WAIT_DATA.get(i).copied())
        .unwrap_or(0)
}

/// What the host knows of Master Hand's target this frame
/// (`passive_vars.boss.p->target_gobj`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Target {
    pub port: u8,
    /// `DObjGetStruct(target_gobj)->translate`.
    pub pos: Vec3,
    /// `coll_data.floor_line_id`, `None` for -1 and -2.
    pub floor_line: Option<u16>,
}

/// The status `ftBossMoveSetStatus` hands over to on arrival
/// (`status_vars.boss.move.proc_setstatus`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Next {
    Hippataku,
    Harau,
    Walk,
    GootsubusuUp,
    Tsutsuku1,
    Drill,
    Okukouki,
    Okupunch1,
    OkutsubushiStart,
    Yubideppou1,
}

/// `fp->proc_physics` where a status swaps it at run time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Physics {
    /// The status descriptor's own.
    #[default]
    Status,
    /// `ftBossDrillProcPhysicsFollow`.
    DrillFollow,
    /// `ftPhysicsApplyAirVelTransNYZ` after the drill's follow.
    TransNYZ,
    /// `ftPhysicsSetAirVelTransN` after the slam's follow.
    SetTransN,
    /// `NULL`: Dead Center after its 200 ticks.
    None,
}

/// A camera change Master Hand's statuses request.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CameraRequest {
    /// `gmCameraSetStatusMapZoom(&at, &eye)`.
    MapZoom { at: Vec3, eye: Vec3 },
    /// `gmCameraSetStatusDefault`.
    Default,
}

/// `FTBossPassiveVars` (`ftBossInfo`) and `FTBossStatusVars`, plus the
/// collision fields Master Hand's own map callbacks keep.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BossState {
    pub target: Option<Target>,
    pub current_line: Option<u16>,
    pub default_line: Option<u16>,
    pub wait_div: f32,
    /// `u32 wait_timer`: decremented before the zero test, so a zero wraps.
    pub wait_timer: u32,
    pub status_id: i8,
    pub status_id_random: i8,
    pub status_id_guard: u8,
    /// `fp->level`, the CPU level.
    pub level: u8,
    /// `fp->pkind == nFTPlayerKindMan`.
    pub is_human: bool,
    /// `fp->lr == 0`: the background attacks and the entry face the
    /// camera ([`model_yaw`]).
    pub lr_zero: bool,
    /// `coll_data.floor_line_id` and `floor_dist` as Master Hand's
    /// `mpCommonSetFighterProjectFloor` leaves them.
    pub floor_line: Option<u16>,
    pub floor_dist: f32,
    /// `status_vars.boss.{wait,okuhikouki,okupunch,okutsubushi}.pos` and
    /// `move.vel` (the destination).
    pub pos: Vec3,
    pub next: Option<Next>,
    /// `status_vars.boss.move.magnitude`.
    pub magnitude: f32,
    /// The status's `s16`/`u16` timer: `gootsubu.wait_timer`,
    /// `tsutsuku.wait_timer`, `drill.follow_timer`,
    /// `yubideppou.wait_timer`, `okutsubushi.follow_timer` and
    /// `dead.dead_timer`.
    pub timer: i32,
    pub edge_left: f32,
    pub edge_right: f32,
    pub bullet_count: i16,
    pub shoot_timer: u8,
    pub physics: Physics,
    /// `is_use_fogcolor` with `fog_color`'s grey level (alpha 0xFF).
    pub fog: Option<u8>,
    /// The camera change this frame requested, for the host.
    pub camera: Option<CameraRequest>,
    /// `sc1PGameBossDefeatInitInterface`: set once, for the host.
    pub defeated: bool,
}

impl Default for BossState {
    /// `ftManagerInitFighter`'s Boss case.
    fn default() -> Self {
        BossState {
            target: None,
            current_line: None,
            default_line: None,
            wait_div: 1.0,
            wait_timer: 0,
            status_id: -1,
            status_id_random: -1,
            status_id_guard: 0,
            level: 1,
            is_human: false,
            lr_zero: false,
            floor_line: None,
            floor_dist: 0.0,
            pos: Vec3::ZERO,
            next: None,
            magnitude: 0.0,
            timer: 0,
            edge_left: 0.0,
            edge_right: 0.0,
            bullet_count: 0,
            shoot_timer: 0,
            physics: Physics::Status,
            fog: None,
            camera: None,
            defeated: false,
        }
    }
}

/// The animation slot of a status: its `ftBossMotion` from
/// `dFTBossSpecialStatusDescs`.
pub fn anim_slot(s: B) -> usize {
    SLOT_DEFAULT
        + match s {
            B::Default | B::Wait | B::Move => 0,
            B::Hippataku => 1,
            B::Harau => 2,
            B::Okuhikouki1 => 3,
            B::Okuhikouki2 => 4,
            B::Okuhikouki3 => 5,
            B::Walk => 6,
            B::WalkLoop => 7,
            B::WalkWait => 8,
            B::WalkShoot => 9,
            B::GootsubusuUp => 10,
            B::GootsubusuWait => 11,
            B::GootsubusuEnd => 12,
            B::GootsubusuDown => 13,
            B::Tsutsuku1 => 14,
            B::Tsutsuku3 => 15,
            B::Tsutsuku2 => 16,
            B::Drill => 17,
            B::Okukouki => 18,
            B::Yubideppou1 => 19,
            B::Yubideppou3 => 20,
            B::Yubideppou2 => 21,
            // `nFTBossStatusOkutsubushiStart` plays `nFTBossMotionOkupunch1`.
            B::Okupunch1 | B::OkutsubushiStart => 22,
            B::Okupunch2 => 23,
            B::Okupunch3 => 24,
            B::Okutsubushi => 25,
            B::DeadLeft => 26,
            B::DeadCenter => 27,
            B::DeadRight => 28,
            B::Appear => 29,
        }
}

/// The status's animation timing: the `AnimJoint` clips' ROM lengths, the
/// figatrees' from the motion table, and none for a looping clip.
fn timing(s: B) -> StatusTiming {
    let joint = match s {
        B::Okuhikouki1 => Some(LAUNCH_FRAMES),
        B::Okuhikouki2 => Some(FLY_FRAMES),
        B::Okuhikouki3 => Some(LANDING_FRAMES),
        B::Drill => Some(DRILL_FRAMES),
        B::Okupunch2 => Some(PUNCH3_FRAMES),
        B::Okupunch3 => Some(PUNCH_END_FRAMES),
        B::Okutsubushi => Some(SLAM_FRAMES),
        B::Appear => Some(APPEAR_FRAMES),
        _ => None,
    };
    match joint.or_else(|| {
        crate::motion::anim_length(crate::fighter::FighterKind::Boss, AnyStatus::Boss(s))
    }) {
        Some(len) => StatusTiming::frames(len),
        None => StatusTiming::unknown(),
    }
}

/// `ftMainSetStatus(fighter_gobj, status, frame_begin, 1.0F,
/// FTSTATUS_PRESERVE_NONE)`.
fn set_at(f: &mut Fighter, s: B, frame_begin: f32) {
    status::set_any_status(f, AnyStatus::Boss(s), frame_begin, timing(s));
    f.boss.physics = Physics::Status;
}

fn set(f: &mut Fighter, s: B) {
    set_at(f, s, 0.0);
}

/// `fp->lr`: -1, 0 or +1.
pub fn lr(f: &Fighter) -> f32 {
    if f.boss.lr_zero {
        0.0
    } else {
        f.facing.sign()
    }
}

fn set_lr(f: &mut Fighter, lr: i32) {
    f.boss.lr_zero = lr == 0;
    if lr > 0 {
        f.facing = Facing::Right;
    } else if lr < 0 {
        f.facing = Facing::Left;
    }
}

/// `joints[nFTPartsJointTopN]->rotate.y = lr * 90°`: the model yaw the
/// renderer draws, `None` when it follows the facing.
pub fn model_yaw(f: &Fighter) -> Option<f32> {
    (f.kind == crate::fighter::FighterKind::Boss && f.boss.lr_zero).then_some(0.0)
}

fn target_pos(f: &Fighter) -> Vec3 {
    f.boss.target.map_or(f.pos, |t| t.pos)
}

fn target_floor(f: &Fighter) -> Option<u16> {
    f.boss.target.and_then(|t| t.floor_line)
}

// ---------------------------------------------------------------------------
// ftbosscommon.c
// ---------------------------------------------------------------------------

/// `ftBossCommonInvertLR`.
fn invert_lr(f: &mut Fighter) {
    if !f.boss.lr_zero {
        f.facing = f.facing.flipped();
    }
}

/// `ftBossCommonCheckEdgeInvertLR`.
fn check_edge_invert_lr<I, F>(f: &mut Fighter, surfaces: &F)
where
    F: Fn() -> I,
    I: IntoIterator<Item = MapSurface>,
{
    let Some(line) = f.boss.current_line else {
        return;
    };
    let (Some(l), Some(r)) = (
        crate::map::floor_edge(surfaces, line, false),
        crate::map::floor_edge(surfaces, line, true),
    ) else {
        return;
    };
    if ((l.x + r.x) * 0.5 - f.pos.x) * lr(f) < 0.0 {
        invert_lr(f);
    }
}

/// `ftBossCommonCheckPlayerInvertLR`.
fn check_player_invert_lr(f: &mut Fighter) {
    if (target_pos(f).x - f.pos.x) * lr(f) < 0.0 {
        invert_lr(f);
    }
}

/// `ftBossCommonRandEdgeLR`.
fn rand_edge<I, F>(surfaces: &F, line: Option<u16>) -> Vec3
where
    F: Fn() -> I,
    I: IntoIterator<Item = MapSurface>,
{
    let right = crate::rng::rand_ushort().is_multiple_of(2);
    let edge = line
        .and_then(|l| crate::map::floor_edge(surfaces, l, right))
        .unwrap_or(Vec2::ZERO);
    Vec3::new(edge.x, edge.y, 0.0)
}

/// `ftBossCommonGotoTargetEdge`: an end of the target's floor line, else
/// Master Hand's own, else the default. The source's second test also
/// requires the target's line not be -2, which [`Target`] folds into
/// `None` with -1.
fn goto_target_edge<I, F>(f: &mut Fighter, surfaces: &F) -> Vec3
where
    F: Fn() -> I,
    I: IntoIterator<Item = MapSurface>,
{
    f.boss.current_line = match (target_floor(f), f.boss.floor_line) {
        (Some(line), _) => Some(line),
        (None, Some(line)) => Some(line),
        (None, None) => f.boss.default_line,
    };
    let mut pos = rand_edge(surfaces, f.boss.current_line);
    pos.y += 100.0;
    pos
}

/// `ftBossCommonSetPosOffsetY`.
fn pos_offset_y(f: &Fighter, off_y: f32) -> Vec3 {
    let t = target_pos(f);
    Vec3::new(t.x, t.y + off_y, 0.0)
}

/// `ftBossCommonSetPosAddVelPlayer`: beside the target, on its floor.
fn pos_add_vel_player<I, F>(f: &Fighter, surfaces: &F, vel_x: f32, vel_y: f32) -> Vec3
where
    F: Fn() -> I,
    I: IntoIterator<Item = MapSurface>,
{
    let mut translate = target_pos(f);
    let x = translate.x;
    translate.x += if !crate::rng::rand_ushort().is_multiple_of(2) {
        vel_x
    } else {
        -vel_x
    };
    let floor = |px: f32| {
        target_floor(f).and_then(|line| crate::map::floor_point(surfaces, line, px).map(|(y, _)| y))
    };
    let mut floor_y = floor(translate.x);
    let out_x = if floor_y.is_some() {
        translate.x
    } else {
        translate.x = if x < translate.x {
            x - vel_x
        } else {
            x + vel_x
        };
        floor_y = floor(translate.x);
        if floor_y.is_some() {
            translate.x
        } else {
            x
        }
    };
    // `y` is left at the last query's distance: zero when both failed.
    let dist = floor_y.map_or(0.0, |y| y - translate.y);
    Vec3::new(out_x, translate.y + dist + vel_y, 0.0)
}

/// `ftBossCommonSetPosAddVelAuto`.
fn pos_add_vel_auto(f: &Fighter, vel_x: f32, vel_y: f32) -> Vec3 {
    let t = target_pos(f);
    let dx = if !crate::rng::rand_ushort().is_multiple_of(2) {
        vel_x
    } else {
        -vel_x
    };
    Vec3::new(t.x + dx, t.y + vel_y, 0.0)
}

/// `ftBossCommonGetPositionCenter`: the middle of a floor line, on it.
fn position_center<I, F>(surfaces: &F, line: Option<u16>) -> Vec3
where
    F: Fn() -> I,
    I: IntoIterator<Item = MapSurface>,
{
    let Some(line) = line else {
        return Vec3::ZERO;
    };
    let l = crate::map::floor_edge(surfaces, line, false).unwrap_or(Vec2::ZERO);
    let r = crate::map::floor_edge(surfaces, line, true).unwrap_or(Vec2::ZERO);
    let x = (l.x + r.x) * 0.5;
    let y = crate::map::floor_point(surfaces, line, x).map_or(0.0, |(y, _)| y);
    Vec3::new(x, y, 0.0)
}

/// `ftBossCommonSetNextAttackWait`.
pub fn set_next_attack_wait(f: &mut Fighter) {
    let level = i32::from(f.boss.level.max(1));
    let wait = (crate::rng::rand_int_range(ATTACK_WAIT_MAX) + ATTACK_WAIT_LEVEL_DIV / level) as f32
        / f.boss.wait_div;
    f.boss.wait_timer = wait as u32;
}

/// `ftBossCommonUpdateFogColor`: the deeper in the background, the
/// darker.
fn update_fog_color(f: &mut Fighter) {
    let z = f.pos.z;
    let fog_dist = if z > 0.0 {
        1.0
    } else if z < BACKGROUND_Z {
        0.0
    } else {
        (z - BACKGROUND_Z) / -BACKGROUND_Z
    };
    let blend = (127.0 * fog_dist) as i32 + 0x80;
    f.boss.fog = Some(blend.min(0xFF) as u8);
}

/// `ftBossCommonSetUseFogColor`: the fog starts at `fog_color`'s current
/// level, which the first physics frame sets.
fn set_use_fog_color(f: &mut Fighter) {
    if f.boss.fog.is_none() {
        f.boss.fog = Some(0);
    }
}

/// `ftBossCommonSetDisableFogColor`.
fn set_disable_fog_color(f: &mut Fighter) {
    f.boss.fog = None;
}

/// `ftManagerInitFighter`'s Boss case for a non-demo fighter:
/// `ftBossCommonSetNextAttackWait` and `ftBossCommonSetDefaultLineID`
/// (the first floor line).
pub fn init<I, F>(f: &mut Fighter, surfaces: &F, level: u8, is_human: bool)
where
    F: Fn() -> I,
    I: IntoIterator<Item = MapSurface>,
{
    f.boss = BossState {
        level,
        is_human,
        ..BossState::default()
    };
    set_next_attack_wait(f);
    f.boss.default_line = crate::map::lines_of(surfaces(), MapSurfaceKind::Floor)
        .next()
        .map(|(line, _)| line);
}

/// `ftBossCommonUpdateDamageStats`, after a hit's damage
/// (`ftMainProcParams`).
pub fn update_damage_stats(f: &mut Fighter) {
    let AnyStatus::Boss(s) = f.status.status else {
        return;
    };
    if matches!(s, B::DeadLeft | B::DeadCenter | B::DeadRight) {
        return;
    }
    if f.damage >= HIT_POINTS {
        f.boss.defeated = true;
        if lr(f) == -1.0 {
            set_dead_left(f);
        } else {
            set_dead_right(f);
        }
    } else if f.damage >= ENRAGE_DAMAGE {
        f.boss.wait_div = ENRAGED_WAIT_DIV;
    }
}

// ---------------------------------------------------------------------------
// Setters
// ---------------------------------------------------------------------------

/// `ftBossWaitSetStatus`.
pub fn set_wait<I, F>(f: &mut Fighter, surfaces: &F)
where
    F: Fn() -> I,
    I: IntoIterator<Item = MapSurface>,
{
    set(f, B::Wait);
    let mut pos = Vec3::ZERO;
    if f.boss.floor_line.is_some() {
        pos.x = f.pos.x;
        pos.y = f.pos.y + f.boss.floor_dist;
    } else {
        let line = target_floor(f).or(f.boss.default_line);
        let bounds = crate::computer::behave::geometry_bounds(surfaces());
        let p = if f.pos.x > bounds.right {
            line.and_then(|l| crate::map::floor_edge(surfaces, l, true))
                .map(|e| Vec3::new(e.x, e.y, 0.0))
                .unwrap_or(Vec3::ZERO)
        } else if f.pos.x < bounds.left {
            line.and_then(|l| crate::map::floor_edge(surfaces, l, false))
                .map(|e| Vec3::new(e.x, e.y, 0.0))
                .unwrap_or(Vec3::ZERO)
        } else {
            position_center(surfaces, line)
        };
        pos.x = p.x;
        pos.y = p.y;
    }
    pos.y += if f.boss.wait_div == ENRAGED_WAIT_DIV {
        600.0
    } else {
        400.0
    };
    pos.z = 0.0;
    f.boss.pos = pos;
}

/// `ftBossMoveSetStatus`: fly to `dest`, then become `next`. The motion
/// keeps its frame.
fn set_move(f: &mut Fighter, next: Next, dest: Vec3) {
    let frame = f.status.anim_frame;
    set_at(f, B::Move, frame);
    f.boss.next = Some(next);
    f.boss.pos = dest;
    if (dest.x - f.pos.x) * lr(f) < 0.0 {
        invert_lr(f);
    }
}

fn set_next<I, F>(f: &mut Fighter, next: Next, surfaces: &F)
where
    F: Fn() -> I,
    I: IntoIterator<Item = MapSurface>,
{
    match next {
        Next::Hippataku => set_hippataku(f, surfaces),
        Next::Harau => set_harau(f, surfaces),
        Next::Walk => set_walk(f, surfaces),
        Next::GootsubusuUp => set(f, B::GootsubusuUp),
        Next::Tsutsuku1 => set(f, B::Tsutsuku1),
        Next::Drill => set_drill(f, surfaces),
        Next::Okukouki => {
            set(f, B::Okukouki);
            check_player_invert_lr(f);
        }
        Next::Okupunch1 => set_okupunch1(f, surfaces),
        Next::OkutsubushiStart => set_okutsubushi_start(f, surfaces),
        Next::Yubideppou1 => {
            set(f, B::Yubideppou1);
            check_player_invert_lr(f);
            f.boss.bullet_count = 0;
        }
    }
}

/// `ftBossHippatakuSetStatus`.
fn set_hippataku<I, F>(f: &mut Fighter, surfaces: &F)
where
    F: Fn() -> I,
    I: IntoIterator<Item = MapSurface>,
{
    set(f, B::Hippataku);
    check_edge_invert_lr(f, surfaces);
}

/// `ftBossHarauSetStatus`.
fn set_harau<I, F>(f: &mut Fighter, surfaces: &F)
where
    F: Fn() -> I,
    I: IntoIterator<Item = MapSurface>,
{
    set(f, B::Harau);
    check_edge_invert_lr(f, surfaces);
}

/// `ftBossOkuhikouki1SetStatus`.
fn set_okuhikouki1<I, F>(f: &mut Fighter, surfaces: &F)
where
    F: Fn() -> I,
    I: IntoIterator<Item = MapSurface>,
{
    set(f, B::Okuhikouki1);
    let line = target_floor(f).or(f.boss.default_line);
    f.boss.pos = position_center(surfaces, line);
}

/// `ftBossOkuhikouki2SetStatus`: across the background, 15000 deep.
fn set_okuhikouki2(f: &mut Fighter) {
    set_lr(f, 0);
    set(f, B::Okuhikouki2);
    status::play_anim_events(f);
    f.pos = Vec3::new(f.boss.pos.x - 9000.0, f.boss.pos.y + 6000.0, BACKGROUND_Z);
    f.boss.camera = Some(CameraRequest::MapZoom {
        at: Vec3::ZERO,
        eye: BACKGROUND_ZOOM_EYE,
    });
    set_use_fog_color(f);
}

/// `ftBossOkuhikouki3SetStatus`.
fn set_okuhikouki3(f: &mut Fighter) {
    set_lr(f, -1);
    set(f, B::Okuhikouki3);
    status::play_anim_events(f);
    f.pos = Vec3::new(f.boss.pos.x + 9000.0, f.boss.pos.y + 3000.0, 0.0);
}

/// `ftBossWalkSetStatus`.
fn set_walk<I, F>(f: &mut Fighter, surfaces: &F)
where
    F: Fn() -> I,
    I: IntoIterator<Item = MapSurface>,
{
    set(f, B::Walk);
    check_edge_invert_lr(f, surfaces);
    f.physics.vel_air.x = 0.0;
    f.physics.vel_air.y = f.boss.floor_dist / 60.0;
}

/// `ftBossWalkLoopSetStatus`.
fn set_walk_loop(f: &mut Fighter) {
    f.physics.vel_air.y = 0.0;
    set(f, B::WalkLoop);
    f.physics.vel_air.x = lr(f) * 35.0;
}

/// `ftBossWalkWaitSetStatus`.
fn set_walk_wait(f: &mut Fighter) {
    set(f, B::WalkWait);
    f.physics.vel_air = Vec3::ZERO;
}

/// `ftBossGootsubusuWaitSetStatus`.
fn set_gootsubusu_wait<I, F>(f: &mut Fighter, surfaces: &F)
where
    F: Fn() -> I,
    I: IntoIterator<Item = MapSurface>,
{
    set(f, B::GootsubusuWait);
    f.boss.timer = crate::rng::rand_int_range(60) + 60;
    store_floor_edges(f, surfaces);
}

/// `mpCollisionGetFloorEdgeL`/`R` of Master Hand's own floor line into
/// `edgeleft_pos_x`/`edgeright_pos_x`.
fn store_floor_edges<I, F>(f: &mut Fighter, surfaces: &F)
where
    F: Fn() -> I,
    I: IntoIterator<Item = MapSurface>,
{
    let line = f.boss.floor_line;
    let edge = |right| {
        line.and_then(|l| crate::map::floor_edge(surfaces, l, right))
            .map_or(0.0, |e| e.x)
    };
    f.boss.edge_left = edge(false);
    f.boss.edge_right = edge(true);
}

/// `ftBossGootsubusuEndSetStatus`.
fn set_gootsubusu_end(f: &mut Fighter) {
    set(f, B::GootsubusuEnd);
    f.physics.vel_air.y = 0.0;
}

/// `ftBossGootsubusuDownSetStatus`.
fn set_gootsubusu_down(f: &mut Fighter) {
    set(f, B::GootsubusuDown);
    f.physics.vel_air.x = 0.0;
    f.physics.vel_air.y = -400.0;
}

/// `ftBossTsutsuku2SetStatus`.
fn set_tsutsuku2(f: &mut Fighter) {
    set(f, B::Tsutsuku2);
    f.boss.timer = crate::rng::rand_int_range(80) + 60;
}

/// `ftBossDrillSetStatus`.
fn set_drill<I, F>(f: &mut Fighter, surfaces: &F)
where
    F: Fn() -> I,
    I: IntoIterator<Item = MapSurface>,
{
    set(f, B::Drill);
    check_edge_invert_lr(f, surfaces);
    store_floor_edges(f, surfaces);
    f.boss.timer = 61;
}

/// `ftBossYubideppou2SetStatus`.
fn set_yubideppou2(f: &mut Fighter) {
    set(f, B::Yubideppou2);
    f.boss.timer = crate::rng::rand_int_range(120) + 60;
}

/// `ftBossYubideppou3SetStatus`: a shot from the fingertip.
fn set_yubideppou3(f: &mut Fighter) {
    set(f, B::Yubideppou3);
    f.boss.bullet_count += 1;
    let pos = f.joint_world(BULLET_JOINT_FIRST, Vec3::ZERO);
    make_bullet(f, pos);
    f.boss.shoot_timer = 4;
}

/// `wpBossBulletHardMakeWeapon` while enraged short of the third shot,
/// otherwise `wpBossBulletNormalMakeWeapon`.
fn make_bullet(f: &mut Fighter, pos: Vec3) {
    let hard = f.boss.wait_div == ENRAGED_WAIT_DIV && f.boss.bullet_count != 3;
    f.weapon_spawn = Some(WeaponSpawn {
        kind: WeaponKind::BossBullet { hard },
        owner_port: f.port,
        team: f.team,
        position: pos,
        facing: lr(f),
        stale: crate::stale::WeaponStale::of(f),
    });
}

/// `ftBossOkupunch1SetStatus`.
fn set_okupunch1<I, F>(f: &mut Fighter, surfaces: &F)
where
    F: Fn() -> I,
    I: IntoIterator<Item = MapSurface>,
{
    set(f, B::Okupunch1);
    let line = target_floor(f).or(f.boss.default_line);
    f.boss.pos = position_center(surfaces, line);
}

/// `ftBossOkupunch2SetStatus`: the fist rises out of the background.
fn set_okupunch2(f: &mut Fighter) {
    set_lr(f, 0);
    set(f, B::Okupunch2);
    status::play_anim_events(f);
    f.pos = Vec3::new(f.boss.pos.x, f.boss.pos.y, BACKGROUND_Z);
    f.boss.camera = Some(CameraRequest::MapZoom {
        at: Vec3::ZERO,
        eye: BACKGROUND_ZOOM_EYE,
    });
    set_use_fog_color(f);
}

/// `ftBossOkupunch3SetStatus`.
fn set_okupunch3(f: &mut Fighter) {
    set_lr(f, -1);
    set(f, B::Okupunch3);
    status::play_anim_events(f);
    f.pos = Vec3::new(f.boss.pos.x, f.boss.pos.y + 6000.0, 0.0);
}

/// `ftBossOkutsubushiStartSetStatus`.
fn set_okutsubushi_start<I, F>(f: &mut Fighter, surfaces: &F)
where
    F: Fn() -> I,
    I: IntoIterator<Item = MapSurface>,
{
    set(f, B::OkutsubushiStart);
    let line = target_floor(f).or(f.boss.default_line);
    f.boss.pos = position_center(surfaces, line);
}

/// `ftBossOkutsubushiSetStatus`.
fn set_okutsubushi(f: &mut Fighter) {
    set_lr(f, 0);
    set(f, B::Okutsubushi);
    status::play_anim_events(f);
    f.pos = Vec3::new(f.boss.pos.x, f.boss.pos.y, BACKGROUND_Z);
    f.boss.timer = 61;
    f.boss.camera = Some(CameraRequest::MapZoom {
        at: Vec3::ZERO,
        eye: BACKGROUND_ZOOM_EYE,
    });
    set_use_fog_color(f);
}

/// `ftBossDeadLeftSetStatus`.
fn set_dead_left(f: &mut Fighter) {
    set(f, B::DeadLeft);
    f.physics.vel_air = Vec3::ZERO;
}

/// `ftBossDeadRightSetStatus`.
fn set_dead_right(f: &mut Fighter) {
    set_lr(f, -1);
    set(f, B::DeadRight);
    f.physics.vel_air = Vec3::ZERO;
}

/// `ftBossDeadCenterSetStatus`: up and away into the background.
fn set_dead_center(f: &mut Fighter) {
    set(f, B::DeadCenter);
    let (sin, cos) = sin_cos(45.0_f32.to_radians());
    f.physics.vel_air = Vec3::new(0.0, sin * 100.0, -(cos * 100.0));
    f.boss.timer = 200;
}

/// `ftCommonAppearSetStatus` for Master Hand: the target is the first
/// other fighter (`gGCCommonLinks[nGCCommonLinkIDFighter]`), which the
/// host has put in [`BossState::target`].
pub fn appear_set_status(f: &mut Fighter) {
    f.entry = crate::appear::Entry {
        pos: f.pos,
        lr: Some(f.facing),
        floor_line: f.floor.map(|s| s.line).or(f.boss.floor_line),
        ..crate::appear::Entry::default()
    };
    set_lr(f, 0);
    f.become_airborne();
    set(f, B::Appear);
    // `ftCommonAppearInitStatusVars`.
    f.dead.is_ghost = true;
    f.dead.camera_mode = crate::dead::CameraMode::Entry;
    f.is_shadow_hidden = true;
    f.interface.tag_hide = true;
    f.motion_script.flags = [0; 4];
}

// ---------------------------------------------------------------------------
// proc_update and proc_interrupt
// ---------------------------------------------------------------------------

/// `proc_update`, then `proc_interrupt`, of the current status. Runs after
/// the frame's animation and motion script, outside hitlag.
pub fn update<I, F>(f: &mut Fighter, surfaces: &F)
where
    F: Fn() -> I,
    I: IntoIterator<Item = MapSurface>,
{
    let AnyStatus::Boss(s) = f.status.status else {
        return;
    };
    let ended = f.status.animation_ended();
    match s {
        B::Hippataku
        | B::Okuhikouki3
        | B::WalkWait
        | B::WalkShoot
        | B::GootsubusuEnd
        | B::Tsutsuku3
        | B::Drill
        | B::Okukouki
        | B::Okupunch3 => {
            if ended {
                set_wait(f, surfaces);
            }
        }
        B::Harau => {
            if ended {
                if f.boss.floor_line.is_none() {
                    set_wait(f, surfaces);
                } else {
                    // `ftBossHarauResetStatus`.
                    set(f, B::Harau);
                }
            }
        }
        B::Okuhikouki1 => {
            if ended {
                set_okuhikouki2(f);
            }
        }
        B::Okuhikouki2 => {
            if ended {
                f.boss.camera = Some(CameraRequest::Default);
                set_okuhikouki3(f);
                set_disable_fog_color(f);
            }
        }
        B::Walk => {
            if ended {
                set_walk_loop(f);
            }
        }
        B::Tsutsuku1 => {
            if ended {
                set_tsutsuku2(f);
            }
        }
        B::Yubideppou1 => {
            if ended {
                set_yubideppou2(f);
            }
        }
        B::Yubideppou3 => {
            if f.boss.wait_div == ENRAGED_WAIT_DIV
                && f.status.anim_frame >= 20.0
                && f.boss.bullet_count != 3
            {
                set_yubideppou3(f);
            } else if ended {
                set_wait(f, surfaces);
            }
        }
        B::Okupunch1 => {
            if ended {
                set_okupunch2(f);
            }
        }
        B::Okupunch2 => {
            if ended {
                f.boss.camera = Some(CameraRequest::Default);
                set_okupunch3(f);
                set_disable_fog_color(f);
            }
        }
        B::Okutsubushi => {
            if ended {
                set_lr(f, -1);
                f.boss.camera = Some(CameraRequest::Default);
                set_wait(f, surfaces);
                set_disable_fog_color(f);
            }
        }
        B::OkutsubushiStart => {
            if ended {
                set_okutsubushi(f);
            }
        }
        // `DeadRight` shares `ftBossDeadLeftProcUpdate`.
        B::DeadLeft | B::DeadRight => {
            if ended {
                set_dead_center(f);
            }
        }
        // `ftCommonAppearProcUpdate`.
        B::Appear => {
            if f.motion_script.flags[2] != 0 {
                f.motion_script.flags[2] = 0;
                f.is_shadow_hidden = false;
            }
            f.motion_script.flags[1] = 0;
            if ended {
                if let Some(lr) = f.entry.lr.take() {
                    set_lr(f, if lr == Facing::Right { 1 } else { -1 });
                }
                f.pos = f.entry.pos;
                f.boss.floor_line = f.entry.floor_line;
                set_wait(f, surfaces);
            }
        }
        // `ftBossWaitProcInterrupt`.
        B::Wait => {
            if f.boss.is_human {
                decide_player(f, surfaces);
            } else {
                decide_computer(f, surfaces);
            }
        }
        // `ftBossDefaultProcInterrupt` needs the game status; nothing sets
        // `Default` (`ftBossDefaultSetStatus` is unused).
        B::Default
        | B::Move
        | B::WalkLoop
        | B::GootsubusuUp
        | B::GootsubusuWait
        | B::GootsubusuDown
        | B::Tsutsuku2
        | B::Yubideppou2
        | B::DeadCenter => {}
    }
}

/// `ftBossWaitDecideStatusComputer`: every `wait_timer` ticks, an attack
/// from the next group.
fn decide_computer<I, F>(f: &mut Fighter, surfaces: &F)
where
    F: Fn() -> I,
    I: IntoIterator<Item = MapSurface>,
{
    f.boss.wait_timer = f.boss.wait_timer.wrapping_sub(1);
    if f.boss.wait_timer != 0 {
        return;
    }
    let group: i32 = if f.boss.status_id_guard == 3 {
        f.boss.status_id_guard = 0;
        2
    } else {
        let random = crate::rng::rand_int_range(3);
        let group = i32::from(wait_byte(
            ARRAY_LOOKUP + i32::from(f.boss.status_id) * 3 + random,
        ));
        if group == 2 {
            f.boss.status_id_guard = 0;
        } else {
            f.boss.status_id_guard += 1;
        }
        group
    };
    let table = if target_floor(f).is_none() {
        RANDOM_NO_GROUND
    } else {
        RANDOM_GROUND
    };
    let first = i32::from(wait_byte(table + group * 2));
    let count = i32::from(wait_byte(table + group * 2 + 1));
    let random = wait_byte(STATUS_LOOKUP + first + crate::rng::rand_int_range(count));
    f.boss.status_id = group as i8;
    f.boss.status_id_random = random as i8;
    let status = wait_byte(STATUS_IDS + i32::from(random));
    match ATTACKS.iter().copied().find(|&s| s as u8 == status) {
        Some(B::Hippataku) => {
            let pos = goto_target_edge(f, surfaces);
            set_move(f, Next::Hippataku, pos);
        }
        Some(B::Okuhikouki1) => set_okuhikouki1(f, surfaces),
        Some(B::GootsubusuUp) => {
            let pos = pos_offset_y(f, 800.0);
            set_move(f, Next::GootsubusuUp, pos);
        }
        Some(B::Walk) => {
            let pos = goto_target_edge(f, surfaces);
            set_move(f, Next::Walk, pos);
        }
        Some(B::Yubideppou1) => {
            let pos = pos_add_vel_auto(f, 3000.0, 0.0);
            set_move(f, Next::Yubideppou1, pos);
        }
        Some(B::Okupunch1) => {
            let pos = pos_add_vel_auto(f, 600.0, 0.0);
            set_move(f, Next::Okupunch1, pos);
        }
        Some(B::OkutsubushiStart) => {
            let pos = pos_add_vel_auto(f, 600.0, 0.0);
            set_move(f, Next::OkutsubushiStart, pos);
        }
        Some(B::Okukouki) => {
            let pos = pos_add_vel_auto(f, 3000.0, 100.0);
            set_move(f, Next::Okukouki, pos);
        }
        Some(B::Drill) => {
            let pos = pos_add_vel_player(f, surfaces, 600.0, 100.0);
            set_move(f, Next::Drill, pos);
        }
        Some(B::Harau) => {
            let pos = goto_target_edge(f, surfaces);
            set_move(f, Next::Harau, pos);
        }
        Some(B::Tsutsuku1) => {
            let pos = pos_offset_y(f, 800.0);
            set_move(f, Next::Tsutsuku1, pos);
        }
        _ => {}
    }
    set_next_attack_wait(f);
}

/// `ftParamGetStickAngleRads`.
fn stick_angle(f: &Fighter) -> f32 {
    ssb_engine::math::atan2(f32::from(f.stick.y), f32::from(f.stick.x).abs())
}

/// `ftBossWaitDecideStatusPlayer`: a human Master Hand (the debug
/// battle's), on B and A with the stick.
fn decide_player<I, F>(f: &mut Fighter, surfaces: &F)
where
    F: Fn() -> I,
    I: IntoIterator<Item = MapSurface>,
{
    use ssb_engine::input::N64Buttons;
    const DEG_50: f32 = 0.872_664_6;
    let x = i32::from(f.stick.x);
    let y = i32::from(f.stick.y);
    // `ftCommonTurnCheckInputSuccess`.
    if (x as f32 * lr(f)) <= status::TURN_STICK_MIN as f32 {
        invert_lr(f);
    }
    let angle = stick_angle(f);
    let taps = f.button_tap();
    let forward = x as f32 * lr(f);
    if taps.contains(N64Buttons::B) {
        if forward >= 20.0 && angle.abs() <= DEG_50 {
            let pos = goto_target_edge(f, surfaces);
            return set_move(f, Next::Hippataku, pos);
        }
        if y >= 20 && DEG_50 < angle {
            return set_okuhikouki1(f, surfaces);
        } else if y <= -20 && angle < -DEG_50 && target_floor(f).is_some() {
            let pos = pos_offset_y(f, 800.0);
            return set_move(f, Next::GootsubusuUp, pos);
        }
        let pos = goto_target_edge(f, surfaces);
        set_move(f, Next::Walk, pos);
    } else if taps.contains(N64Buttons::A) {
        if x.abs() >= 56 && f.stick.tap_x < 3 {
            let pos = pos_add_vel_auto(f, 3000.0, 0.0);
            return set_move(f, Next::Yubideppou1, pos);
        } else if y >= 53 && f.stick.tap_y < 4 {
            let pos = pos_add_vel_auto(f, 600.0, 0.0);
            return set_move(f, Next::Okupunch1, pos);
        } else if y <= -53 && f.stick.tap_y < 4 {
            let pos = pos_add_vel_auto(f, 600.0, 0.0);
            return set_move(f, Next::OkutsubushiStart, pos);
        } else if forward >= 20.0 && angle <= DEG_50 {
            let pos = pos_add_vel_auto(f, 3000.0, 100.0);
            return set_move(f, Next::Okukouki, pos);
        } else if y >= 20 && DEG_50 < angle && target_floor(f).is_some() {
            let pos = pos_add_vel_player(f, surfaces, 600.0, 100.0);
            return set_move(f, Next::Drill, pos);
        }
        if y <= -20 && angle < -DEG_50 {
            let pos = goto_target_edge(f, surfaces);
            return set_move(f, Next::Harau, pos);
        }
        let pos = pos_offset_y(f, 800.0);
        set_move(f, Next::Tsutsuku1, pos);
    } else {
        // `ftBossWaitSetVelStickRange`.
        f.physics.vel_air.x = if x.abs() >= 8 { x as f32 } else { 0.0 };
        f.physics.vel_air.y = if y.abs() >= 8 { y as f32 } else { 0.0 };
    }
}

// ---------------------------------------------------------------------------
// proc_physics and proc_map
// ---------------------------------------------------------------------------

/// `ftPhysicsApplyAirVelTransNAll`.
fn transn_all(f: &mut Fighter) {
    let lr = lr(f);
    crate::physics::apply_air_vel_transn_all(&mut f.physics, f.root_motion, lr, f.attributes.size);
}

/// `ftPhysicsApplyAirVelTransNYZ`: TransN's Y and Z; X stays.
fn transn_yz(f: &mut Fighter) {
    let x = f.physics.vel_air.x;
    transn_all(f);
    f.physics.vel_air.x = x;
}

/// `ftPhysicsSetAirVelTransN`: TransN's motion in world axes, unturned.
/// The port reads the runtime's TransN sample (RE-457).
fn set_transn(f: &mut Fighter) {
    let size = f.attributes.size;
    f.physics.vel_air = f.root_motion.delta * size;
}

/// Velocity a tenth of the way to `dest`, all of it under 5 units; the
/// source's `vel_air.z` is left alone. Returns the distance.
fn approach(f: &mut Fighter, dest: Vec3) -> f32 {
    let vel = dest - f.pos;
    let mag = vel.length();
    let v = if mag < 5.0 { vel } else { vel * 0.1 };
    f.physics.vel_air.x = v.x;
    f.physics.vel_air.y = v.y;
    mag
}

/// Chase the target's X at up to `max` per tick: `vel_air.x` set
/// (`add` false) or increased.
fn follow_x(f: &mut Fighter, max: f32, add: bool) {
    let dist = target_pos(f).x - f.pos.x;
    let v = if dist.abs() > max {
        if dist > 0.0 {
            max
        } else {
            -max
        }
    } else {
        dist
    };
    if add {
        f.physics.vel_air.x += v;
    } else {
        f.physics.vel_air.x = v;
    }
}

fn physics(f: &mut Fighter, s: B) {
    match f.boss.physics {
        Physics::Status => {}
        Physics::None => return,
        Physics::TransNYZ => return transn_yz(f),
        Physics::SetTransN => return set_transn(f),
        // `ftBossDrillProcPhysicsFollow`.
        Physics::DrillFollow => {
            transn_yz(f);
            follow_x(f, 30.0, false);
            f.boss.timer -= 1;
            if f.boss.timer == 0 {
                f.physics.vel_air.x = 0.0;
                f.boss.physics = Physics::TransNYZ;
            }
            return;
        }
    }
    match s {
        // `ftBossWaitProcPhysics`.
        B::Wait => {
            if !f.boss.is_human {
                let dest = f.boss.pos;
                approach(f, dest);
            }
        }
        // `ftBossMoveProcPhysics`.
        B::Move => {
            let dest = f.boss.pos;
            let mag = approach(f, dest);
            f.boss.magnitude = if mag < 5.0 { 0.0 } else { mag };
        }
        B::Hippataku
        | B::Okuhikouki1
        | B::Okuhikouki3
        | B::GootsubusuEnd
        | B::Tsutsuku1
        | B::Tsutsuku3
        | B::Okukouki
        | B::Yubideppou1 => transn_all(f),
        // `ftBossHarauProcPhysics`.
        B::Harau => {
            transn_all(f);
            f.physics.vel_air.x += lr(f) * HARAU_VEL_X;
        }
        // `ftBossOkuhikouki2ProcPhysics`.
        B::Okuhikouki2 => {
            set_transn(f);
            follow_x(f, OKUHIKOUKI_VEL_ADD, true);
            update_fog_color(f);
        }
        // `ftBossWalkLoopProcPhysics`.
        B::WalkLoop => {
            if walk_loop_in_range(f) {
                set(f, B::WalkShoot);
            }
        }
        // `ftBossGootsubusuUpProcPhysics`.
        B::GootsubusuUp => {
            transn_yz(f);
            f.physics.vel_air.y += 50.0;
        }
        // `ftBossGootsubusuWaitProcPhysics`.
        B::GootsubusuWait => {
            transn_yz(f);
            f.boss.timer -= 1;
            if f.boss.timer == 0 {
                set_gootsubusu_down(f);
            } else {
                follow_x(f, 35.0, false);
            }
        }
        // `ftBossTsutsuku2ProcPhysics`.
        B::Tsutsuku2 => {
            f.boss.timer -= 1;
            if f.boss.timer == 0 {
                set(f, B::Tsutsuku3);
            } else {
                let t = target_pos(f);
                let dest = Vec3::new(t.x + -lr(f) * 900.0, t.y + 300.0, 0.0);
                approach(f, dest);
            }
        }
        // `ftBossDrillProcPhysics`.
        B::Drill => {
            transn_yz(f);
            f.boss.timer -= 1;
            if f.boss.timer == 0 {
                f.boss.physics = Physics::DrillFollow;
                f.boss.timer = 39;
            }
        }
        // `ftBossYubideppou2ProcPhysics`.
        B::Yubideppou2 => {
            f.boss.timer -= 1;
            if f.boss.timer == 0 {
                set_yubideppou3(f);
            } else {
                yubideppou2_update_position(f);
            }
        }
        // `ftBossYubideppou3ProcPhysics`.
        B::Yubideppou3 => {
            if f.boss.shoot_timer != 0 {
                f.boss.shoot_timer -= 1;
                if f.boss.shoot_timer == 0 {
                    let pos = f.joint_world(BULLET_JOINT_FOLLOW, Vec3::ZERO);
                    make_bullet(f, pos);
                }
            }
            if f.boss.wait_div == ENRAGED_WAIT_DIV {
                yubideppou2_update_position(f);
            }
        }
        B::Okupunch1 | B::Okupunch3 | B::OkutsubushiStart => transn_yz(f),
        // `ftBossOkupunch2ProcPhysics`.
        B::Okupunch2 => {
            set_transn(f);
            follow_x(f, 40.0, true);
            update_fog_color(f);
        }
        // `ftBossOkutsubushiProcPhysics`.
        B::Okutsubushi => {
            set_transn(f);
            follow_x(f, 40.0, true);
            f.boss.timer -= 1;
            if f.boss.timer == 0 {
                f.physics.vel_air.x = 0.0;
                f.boss.physics = Physics::SetTransN;
            }
            update_fog_color(f);
        }
        // `ftBossDeadCenterProcPhysics`.
        B::DeadCenter => {
            f.boss.timer -= 1;
            if f.boss.timer == 0 {
                f.boss.physics = Physics::None;
            }
        }
        // `ftBossAppearProcPhysics`: TopN takes TransN's translation.
        B::Appear => {
            f.pos = f.root_motion.translate;
            f.physics.vel_air = Vec3::ZERO;
        }
        B::Default
        | B::Walk
        | B::WalkWait
        | B::WalkShoot
        | B::GootsubusuDown
        | B::DeadLeft
        | B::DeadRight => {}
    }
}

/// `ftBossWalkLoopCheckPlayerInRange`.
fn walk_loop_in_range(f: &Fighter) -> bool {
    let t = target_pos(f);
    let dy = t.y - f.pos.y;
    dy > -300.0 && dy < 500.0 && (t.x - f.pos.x) * lr(f) < 1200.0
}

/// `ftBossYubideppou2UpdatePosition`: 3000 behind the target, level.
fn yubideppou2_update_position(f: &mut Fighter) {
    let t = target_pos(f);
    let dest = Vec3::new(t.x + -lr(f) * 3000.0, t.y, 0.0);
    approach(f, dest);
}

/// `mpCommonSetFighterProjectFloor` (`mpProcessSetCollProjectFloorID`):
/// the floor under the body's bottom, if any.
fn project_floor<I, F>(f: &mut Fighter, surfaces: &F)
where
    F: Fn() -> I,
    I: IntoIterator<Item = MapSurface>,
{
    let probe = Vec3::new(f.pos.x, f.pos.y + f.coll.bottom, f.pos.z);
    match crate::map::project_floor_line(surfaces, probe) {
        Some((line, dist)) => {
            f.boss.floor_line = Some(line);
            f.boss.floor_dist = dist;
        }
        None => f.boss.floor_line = None,
    }
}

/// `mpCommonCheckFighterOnFloor` for the finger walk
/// (`mpProcessCheckTestFloorCollisionNew`): while Master Hand's floor line
/// still spans his X, he is put on it; past its end he is put at the
/// end's height and the test fails.
fn on_floor<I, F>(f: &mut Fighter, surfaces: &F) -> bool
where
    F: Fn() -> I,
    I: IntoIterator<Item = MapSurface>,
{
    let Some(line) = f.boss.floor_line else {
        project_floor(f, surfaces);
        return false;
    };
    if let Some((y, _)) = crate::map::floor_point(surfaces, line, f.pos.x) {
        f.pos.y = y - f.coll.bottom;
        f.boss.floor_dist = 0.0;
        return true;
    }
    let left = crate::map::floor_edge(surfaces, line, false);
    let edge = if left.is_some_and(|l| f.pos.x <= l.x) {
        left
    } else {
        crate::map::floor_edge(surfaces, line, true)
    };
    if let Some(edge) = edge {
        f.pos.y = edge.y - f.coll.bottom;
    }
    false
}

/// `mpCommonCheckFighterLanding` for the falling fist: the swept floor
/// test from last tick's position.
fn landing<I, F>(f: &mut Fighter, from: Vec3, surfaces: &F) -> bool
where
    F: Fn() -> I,
    I: IntoIterator<Item = MapSurface>,
{
    let b = f.coll.bottom;
    let hit = crate::collision::check_floor(
        crate::map::floors(surfaces()),
        Vec2::new(from.x, from.y + b),
        Vec2::new(f.pos.x, f.pos.y + b),
    );
    match hit {
        Some(hit) => {
            f.pos.x = hit.point.x;
            f.pos.y = hit.point.y - b;
            f.boss.floor_line = Some(hit.line);
            f.boss.floor_dist = 0.0;
            true
        }
        None => false,
    }
}

fn map<I, F>(f: &mut Fighter, from: Vec3, surfaces: &F)
where
    F: Fn() -> I,
    I: IntoIterator<Item = MapSurface>,
{
    let AnyStatus::Boss(s) = f.status.status else {
        return;
    };
    match s {
        // `ftBossMoveProcMap`.
        B::Move => {
            project_floor(f, surfaces);
            if f.boss.magnitude == 0.0 {
                f.physics.vel_air = Vec3::ZERO;
                if let Some(next) = f.boss.next.take() {
                    set_next(f, next, surfaces);
                }
            }
        }
        // `ftBossWalkLoopProcMap`.
        B::WalkLoop => {
            if !on_floor(f, surfaces) {
                set_walk_wait(f);
            }
        }
        // `ftBossGootsubusuUpProcMap`.
        B::GootsubusuUp => {
            project_floor(f, surfaces);
            if f.boss.floor_line.is_some() && -f.boss.floor_dist >= 3000.0 {
                f.pos.y += f.boss.floor_dist + 3000.0;
                set_gootsubusu_wait(f, surfaces);
            }
        }
        // `ftBossGootsubusuWaitProcMap`, its sign as the source has it.
        B::GootsubusuWait => {
            f.pos.x = f.pos.x.min(f.boss.edge_right);
            if f.pos.x < f.boss.edge_left {
                f.pos.x = f.boss.edge_left;
            }
            project_floor(f, surfaces);
            let var = f.boss.floor_dist + 3000.0;
            f.pos.y -= var;
            f.boss.floor_dist = 3000.0;
        }
        // `ftBossGootsubusuDownProcMap`.
        B::GootsubusuDown => {
            if landing(f, from, surfaces) {
                set_gootsubusu_end(f);
            }
        }
        // `ftBossDrillProcMap`.
        B::Drill => {
            if f.pos.x > f.boss.edge_right {
                f.pos.x = f.boss.edge_right;
            } else if f.pos.x < f.boss.edge_left {
                f.pos.x = f.boss.edge_left;
            }
            project_floor(f, surfaces);
        }
        // No `proc_map`.
        B::Walk | B::DeadLeft | B::DeadRight => {}
        // `mpCommonUpdateFighterProjectFloor`.
        _ => project_floor(f, surfaces),
    }
}

/// `ftMainProcPhysicsMap` for Master Hand: `proc_physics`, the airborne
/// `translate += vel_air`, then `proc_map`. Returns whether Master Hand
/// owned the step, so the caller skips the common kinetics.
pub fn tick_status<I, F>(f: &mut Fighter, surfaces: &F) -> bool
where
    F: Fn() -> I,
    I: IntoIterator<Item = MapSurface>,
{
    let AnyStatus::Boss(s) = f.status.status else {
        return false;
    };
    let from = f.pos;
    physics(f, s);
    f.pos += f.physics.vel_air;
    map(f, from, surfaces);
    true
}
