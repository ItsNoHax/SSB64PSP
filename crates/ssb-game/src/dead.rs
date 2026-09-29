//! Blast-zone deaths and rebirth — `ft/ftcommon/ftcommondead.c` and
//! `ft/ftcommon/ftcommonrebirth.c`.
//!
//! `ftMainProcPhysicsMap` runs [`check`] every frame between the position
//! step and `proc_map`. Crossing the stage's `map_bound_*` enters one of the
//! `Dead*` statuses; its wait runs out into `ftCommonDeadCheckRebirth`,
//! which respawns the fighter under a halo at the stage's rebirth point
//! (`RebirthDown` → `RebirthStand` → `RebirthWait` → `Fall`).
//!
//! The halo number a respawn takes depends on the other fighters in a
//! rebirth status, which one fighter's update cannot see, so the rebirth
//! itself is the host's call: a `Dead*` status whose wait ends sets
//! [`DeadState::rebirth_pending`], and the match calls [`rebirth_down`] at
//! once, in the same slot of the frame.
//!
//! Scores, rumble, quake, sounds and the effects are presentation. The
//! explosion's placement ([`DeadState::explode`]), the screen flash
//! request ([`DeadState::flash`]) and the star KO sparkle's position
//! ([`DeadState::sparkle`]) are recorded for the host, which runs them
//! ([`crate::ko`]); the halo is drawn while the fighter is in a rebirth
//! status ([`halo_scale`]). The top-out fade and the rebirth glow are the
//! fighter's colour animation ([`crate::colanim`]).

use ssb_engine::math::{Vec2, Vec3};

use crate::fighter::{Facing, Fighter, FighterKind, Situation};
use crate::status::{self, AnyStatus, BlastZone, Preserve, Status, StatusTiming};

/// `FTCOMMON_DEAD_WAIT`.
pub const DEAD_WAIT: i32 = 45;
/// `FTCOMMON_DEADUP_WAIT`.
pub const DEADUP_WAIT: i32 = 180;
/// `FTCOMMON_DEADUPSTAR_VEL_Z`.
pub const DEADUPSTAR_VEL_Z: f32 = -83.333_336;
/// `ftCommonDeadCheckInterruptCommon`: a top-out explodes in the foreground
/// (`DeadUpFall`) one time in six.
pub const DEADUP_FALL_CHANCE: f32 = 1.0 / 6.0;
/// `FTCOMMON_REBIRTH_INVINCIBLE_FRAMES`.
pub const REBIRTH_INVINCIBLE_FRAMES: u16 = 120;
/// `FTCOMMON_REBIRTH_HALO_LOWER_WAIT`.
pub const HALO_LOWER_WAIT: i32 = 90;
/// `FTCOMMON_REBIRTH_HALO_DESPAWN_WAIT`.
pub const HALO_DESPAWN_WAIT: i32 = 390;
/// `FTCOMMON_REBIRTH_HALO_UNK_WAIT`: the camera takes the fighter back.
pub const HALO_CAMERA_WAIT: i32 = 45;
/// `FTCOMMON_REBIRTH_HALO_STAND_WAIT`.
pub const HALO_STAND_WAIT: i32 = 75;
/// `ftCommonRebirthDownSetStatus`'s `frame_begin`.
pub const REBIRTH_DOWN_FRAME_BEGIN: f32 = 100.0;
/// `dFTCommonRebirthOffsetsX`, by halo number.
pub const REBIRTH_OFFSETS_X: [f32; 4] = [0.0, -1000.0, 1000.0, -2000.0];

/// `nFTCameraMode*`: how the battle camera treats the fighter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CameraMode {
    #[default]
    Default,
    /// `nFTCameraModeDeadUp`: a top-out flies off screen.
    DeadUp,
    /// `nFTCameraModeGhost`: a respawn's first 45 halo ticks, and a
    /// fighter out of stocks (`ftCommonSleepSetStatus`).
    Ghost,
    /// `nFTCameraModeEntry`: from `ftCommonAppearInitStatusVars` until
    /// "Go" (`ifCommonAnnounceGoSetStatus`), framed at the entry position.
    Entry,
}

/// Which way `efManagerDeadExplodeMakeEffect` points the blast.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExplodeKind {
    Down = 0,
    Right = 1,
    Left = 3,
}

impl ExplodeKind {
    /// `dEFManagerDeadExplodeRotateD[type]`: the effect root's `rotate.z`,
    /// in degrees.
    pub fn rotate_z_degrees(self) -> f32 {
        [0.0, 90.0, 180.0, 270.0][self as usize]
    }
}

/// `FTAttributes::halo_size`, the respawn halo's scale
/// (`efManagerRebirthHaloMakeEffect`), from each fighter's `*MainAttributes`
/// (`relocData/2xx_*Main.c`). The polygons, Metal Mario and Giant Donkey
/// Kong carry their base fighter's value.
pub fn halo_size(kind: FighterKind) -> f32 {
    use FighterKind as K;
    let base = kind.polygon_base().unwrap_or(kind);
    match base {
        K::Mario | K::MetalMario | K::Ness | K::Boss => 1.0,
        K::Fox | K::Link | K::Pikachu => 1.1,
        K::Donkey | K::GiantDonkey => 1.7,
        K::Samus | K::Yoshi | K::Captain | K::Purin => 1.2,
        K::Luigi => 1.02,
        K::Kirby => 1.14,
        _ => 1.0,
    }
}

/// The halo's child scale while `efManagerRebirthHaloMakeEffect`'s effect
/// lives. `ftCommonRebirthDownSetStatus` makes it and sets
/// `is_effect_attach`; `RebirthStand` and `RebirthWait` keep it
/// (`FTSTATUS_PRESERVE_EFFECT`), and the next status ends it
/// (`ftParamProcStopEffect`). Its root follows TopN (matrix kind 0x50,
/// `func_ovl0_800C99CC`).
pub fn halo_scale(f: &Fighter) -> Option<f32> {
    is_rebirth_status(f.status.status).then(|| halo_size(f.kind))
}

/// The stage data the checks read: `MPGroundData`'s `map_bound_*` and
/// `camera_bound_*`, and the `nMPMapObjKindRebirth` point.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StageBounds {
    pub map: BlastZone,
    pub camera: BlastZone,
    pub rebirth: Vec2,
    /// `MPGroundData.fog_color`: the colour a star KO fades towards.
    pub fog_color: [u8; 3],
}

/// `status_vars.common.rebirth`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct RebirthVars {
    pub halo_lower_wait: i32,
    pub halo_despawn_wait: i32,
    pub pos: Vec3,
    pub halo_offset: Vec3,
    pub halo_number: u8,
}

/// The fighter's death and rebirth state.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct DeadState {
    /// The stage, set by the host when it spawns the fighter. `None`
    /// disables the check.
    pub bounds: Option<StageBounds>,
    /// `gSCManagerBattleState->game_rules & SCBATTLE_GAMERULE_STOCK`. Off in
    /// Training, whose rules are timed.
    pub stock_rule: bool,
    /// `CObjGetStruct(gGMCameraGObj)->vec.eye`, written by the host each
    /// frame; `DeadUpFall` drops the fighter from above it.
    pub camera_eye: Vec3,
    /// `FTStruct::is_ghost`: out of the fight, unhittable, and not checked.
    pub is_ghost: bool,
    /// `FTStruct::is_rebirth`.
    pub is_rebirth: bool,
    pub camera_mode: CameraMode,
    /// `status_vars.common.dead.pos`: where a top-out started, which the
    /// camera frames at the top (`gmCameraSetDeadUpStarPosition`).
    pub up_pos: Vec3,
    /// `status_vars.common.dead.wait`.
    pub wait: i32,
    /// `motion_vars.flags.flag1`: the top-out phases.
    pub step: u8,
    pub rebirth: RebirthVars,
    /// Set when a `Dead*` wait runs out; the host then calls
    /// [`rebirth_down`].
    pub rebirth_pending: bool,
    /// Set on the frame the fighter dies, for the host to destroy its
    /// weapons (`ftManagerDestroyFighterWeapons`).
    pub died: bool,
    /// Where and which way `efManagerDeadExplodeMakeEffect` sets off the
    /// explosion, for the host to take.
    pub explode: Option<(Vec3, ExplodeKind)>,
    /// `ifScreenFlashSetColAnimID(nGMColAnimScreenFlashDeadExplode, 0)`,
    /// for the host to take.
    pub flash: bool,
    /// Where `efManagerSparkleWhiteDeadMakeEffect` makes the star KO's
    /// sparkle (TopN, scale 5), for the host to take.
    pub sparkle: Option<Vec3>,
    /// `gSCManagerBattleState->players[].falls`.
    pub falls: u16,
    /// Set when `ftCommonDeadUpdateScore` runs, for the host to report the
    /// fall and `damage_player` to the battle ([`crate::battle`]).
    pub scored: bool,
}

/// `ftMainSetStatus`'s resets of the fields this module owns. The entry's
/// camera mode survives until "Go".
pub(crate) fn on_set_status(f: &mut Fighter) {
    f.dead.is_ghost = false;
    f.dead.is_rebirth = false;
    if f.dead.camera_mode != CameraMode::Entry {
        f.dead.camera_mode = CameraMode::Default;
    }
}

/// Statuses whose setter owns the ground/air situation.
pub(crate) fn keeps_situation(status: AnyStatus) -> bool {
    matches!(
        status,
        AnyStatus::Common(
            Status::DeadDown
                | Status::DeadLeftRight
                | Status::DeadUpStar
                | Status::DeadUpFall
                | Status::RebirthDown
                | Status::RebirthStand
                | Status::RebirthWait
        )
    )
}

fn is_rebirth_status(status: AnyStatus) -> bool {
    matches!(
        status,
        AnyStatus::Common(Status::RebirthDown | Status::RebirthStand | Status::RebirthWait)
    )
}

/// `ftCommonDeadCheckInterruptCommon`, outside the 1P game's team bounds
/// and `is_limit_map_bounds`. The order is bottom, right, left, then top;
/// a top-out draws one `syUtilsRandFloat`.
pub fn check(f: &mut Fighter) -> bool {
    let Some(bounds) = f.dead.bounds else {
        return false;
    };
    if f.kind == FighterKind::Boss || f.dead.is_ghost {
        return false;
    }
    let b = bounds.map;
    if f.pos.y < b.bottom {
        set_dead_down(f);
    } else if f.pos.x > b.right {
        set_dead_right(f);
    } else if f.pos.x < b.left {
        set_dead_left(f);
    } else if f.pos.y > b.top {
        if crate::rng::rand_float() < DEADUP_FALL_CHANCE {
            set_dead_up_fall(f);
        } else {
            set_dead_up_star(f);
        }
    } else {
        return false;
    }
    true
}

/// `ftCommonDeadResetCommonVars`.
fn reset_common_vars(f: &mut Fighter) {
    f.dead.died = true;
    // `ftCommonThrownDecideDeadResult`.
    crate::grab::release_on_dead(f);
    f.situation = Situation::Air;
    f.floor = None;
    // `itMainDestroyItem`.
    if f.items.held.is_some() {
        f.items.request(crate::item::ItemRequest::Destroy);
        f.items.held = None;
    }
}

/// `ftCommonDeadResetSpecialStats`.
fn reset_special_stats(f: &mut Fighter) {
    f.dead.is_ghost = true;
    f.is_shadow_hidden = true;
}

/// `ftCommonDeadUpdateScore`: a stock match takes a stock.
fn update_score(f: &mut Fighter) {
    f.dead.falls = f.dead.falls.saturating_add(1);
    f.dead.scored = true;
    if f.dead.stock_rule {
        f.stocks -= 1;
    }
}

/// `ftCommonDeadInitStatusVars`.
fn init_status_vars(f: &mut Fighter) {
    f.dead.wait = DEAD_WAIT;
    crate::physics::stop_all(&mut f.physics);
    f.is_invisible = true;
    update_score(f);
}

fn enter(f: &mut Fighter, status: Status) {
    reset_common_vars(f);
    status::set_status(f, status, 0.0, StatusTiming::unknown());
    reset_special_stats(f);
    init_status_vars(f);
}

/// The explosion's position: the death point, clamped into the camera's
/// range on the axis the fighter did not leave by.
fn explode(f: &mut Fighter, kind: ExplodeKind) {
    let Some(bounds) = f.dead.bounds else {
        return;
    };
    let c = bounds.camera;
    let mut pos = f.pos;
    match kind {
        ExplodeKind::Down => pos.x = pos.x.min(c.right).max(c.left),
        ExplodeKind::Right | ExplodeKind::Left => pos.y = pos.y.min(c.top).max(c.bottom),
    }
    f.dead.explode = Some((pos, kind));
    f.dead.flash = true;
}

/// `ftCommonDeadDownSetStatus`.
pub fn set_dead_down(f: &mut Fighter) {
    enter(f, Status::DeadDown);
    explode(f, ExplodeKind::Down);
}

/// `ftCommonDeadRightSetStatus`.
pub fn set_dead_right(f: &mut Fighter) {
    enter(f, Status::DeadLeftRight);
    explode(f, ExplodeKind::Right);
}

/// `ftCommonDeadLeftSetStatus`.
pub fn set_dead_left(f: &mut Fighter) {
    enter(f, Status::DeadLeftRight);
    explode(f, ExplodeKind::Left);
}

fn enter_up(f: &mut Fighter, status: Status) {
    reset_common_vars(f);
    let timing = match crate::motion::anim_length(f.kind, status.into()) {
        Some(len) => StatusTiming::frames(len),
        None => StatusTiming::unknown(),
    };
    status::set_status(f, status, 0.0, timing);
    crate::physics::stop_all(&mut f.physics);
    f.dead.up_pos = f.pos;
    f.dead.camera_mode = CameraMode::DeadUp;
    f.dead.wait = 1;
    f.dead.step = 0;
    reset_special_stats(f);
}

/// `ftCommonDeadUpStarSetStatus`, which ends with
/// `ftParamResetFighterColAnim`.
pub fn set_dead_up_star(f: &mut Fighter) {
    enter_up(f, Status::DeadUpStar);
    f.colanim.reset();
}

/// `ftCommonDeadUpFallSetStatus`.
pub fn set_dead_up_fall(f: &mut Fighter) {
    enter_up(f, Status::DeadUpFall);
}

/// `ftCommonDeadCheckRebirth`: out of stocks the fighter sleeps; otherwise
/// the host respawns it.
fn check_rebirth(f: &mut Fighter) {
    if f.dead.stock_rule && f.stocks == -1 {
        // `ftCommonSleepSetStatus`.
        status::set_status(f, Status::Sleep, 0.0, StatusTiming::unknown());
        f.dead.is_ghost = true;
        f.dead.camera_mode = CameraMode::Ghost;
        return;
    }
    f.dead.rebirth_pending = true;
}

/// `proc_update` and `proc_interrupt` of the dead and rebirth statuses.
/// Returns `false` when none is current.
pub fn update(f: &mut Fighter, current: Status) -> bool {
    match current {
        // `ftCommonDeadCommonProcUpdate`.
        Status::DeadDown | Status::DeadLeftRight => {
            f.dead.wait -= 1;
            if f.dead.wait == 0 {
                check_rebirth(f);
            }
        }
        Status::DeadUpStar => update_up_star(f),
        Status::DeadUpFall => update_up_fall(f),
        // `ftCommonRebirthDownProcUpdate`.
        Status::RebirthDown => {
            update_halo_wait(f);
            let wait = f.dead.rebirth.halo_despawn_wait;
            if wait == HALO_DESPAWN_WAIT - HALO_CAMERA_WAIT {
                f.dead.camera_mode = CameraMode::Default;
            }
            if wait == HALO_DESPAWN_WAIT - HALO_STAND_WAIT {
                set_rebirth_stand(f);
            }
        }
        // `ftCommonRebirthStandProcUpdate`.
        Status::RebirthStand => {
            update_halo_wait(f);
            if f.status.animation_ended() {
                set_rebirth_wait(f);
            }
        }
        // `ftCommonRebirthWaitProcUpdate`, then `...ProcInterrupt`.
        Status::RebirthWait => {
            update_halo_wait(f);
            if f.dead.rebirth.halo_despawn_wait == 0 {
                crate::reaction::set_timed_invincible(f, REBIRTH_INVINCIBLE_FRAMES);
                status::set_fall(f);
            } else if status::ground_interrupt(f) {
                crate::reaction::set_timed_invincible(f, REBIRTH_INVINCIBLE_FRAMES);
            }
        }
        _ => return false,
    }
    true
}

/// `ftCommonDeadUpStarProcUpdate`. While the fighter flies away it fades
/// into the stage's fog colour, up to half strength.
fn update_up_star(f: &mut Fighter) {
    if f.dead.step == 1 {
        f.colanim.color1.rgba[3] = (128 - (f.dead.wait * 128) / DEADUP_WAIT) as u8;
    }
    if f.dead.wait != 0 {
        f.dead.wait -= 1;
    }
    if f.dead.wait != 0 {
        return;
    }
    match f.dead.step {
        0 => {
            let top = f.dead.bounds.map_or(0.0, |b| b.camera.top);
            f.physics.vel_air.y = (top * 0.6 - f.pos.y) / DEADUP_WAIT as f32;
            f.physics.vel_air.z = DEADUPSTAR_VEL_Z;
            let fog = f.dead.bounds.map_or([0; 3], |b| b.fog_color);
            f.colanim.is_use_color1 = true;
            f.colanim.color1.rgba = [fog[0], fog[1], fog[2], 0];
            f.dead.wait = DEADUP_WAIT;
            f.dead.step += 1;
        }
        1 => {
            crate::physics::stop_all(&mut f.physics);
            // TopN is the fighter's position.
            f.dead.sparkle = Some(f.pos);
            f.is_invisible = true;
            update_score(f);
            f.colanim.is_use_color1 = false;
            f.dead.wait = DEAD_WAIT;
            f.dead.step += 1;
        }
        2 => check_rebirth(f),
        _ => {}
    }
}

/// `ftCommonDeadUpFallProcUpdate`.
fn update_up_fall(f: &mut Fighter) {
    let bounds = f.dead.bounds;
    if f.dead.step == 1 && bounds.is_some_and(|b| f.pos.y < b.map.bottom) {
        f.physics.vel_air.y = 0.0;
    }
    if f.dead.wait != 0 {
        f.dead.wait -= 1;
    }
    if f.dead.wait != 0 {
        return;
    }
    match f.dead.step {
        0 => {
            let (bottom, top) = bounds.map_or((0.0, f32::MAX), |b| (b.camera.bottom, b.map.top));
            f.physics.vel_air.y = (bottom - f.pos.y) / DEADUP_WAIT as f32;
            let eye = f.dead.camera_eye;
            f.pos.z = (eye.z - 3000.0).max(2000.0);
            f.pos.x = eye.x;
            f.pos.y = (eye.y + 3000.0).min(top);
            f.dead.wait = DEADUP_WAIT;
            f.dead.step += 1;
        }
        1 => {
            crate::physics::stop_all(&mut f.physics);
            f.dead.flash = true;
            f.is_invisible = true;
            update_score(f);
            f.dead.wait = DEAD_WAIT;
            f.dead.step += 1;
        }
        2 => check_rebirth(f),
        _ => {}
    }
}

/// `ftCommonRebirthCommonUpdateHaloWait`.
fn update_halo_wait(f: &mut Fighter) {
    let r = &mut f.dead.rebirth;
    if r.halo_despawn_wait != 0 {
        r.halo_despawn_wait -= 1;
    }
    if r.halo_lower_wait != 0 {
        r.halo_lower_wait -= 1;
    }
}

/// The halo number `ftCommonRebirthDownSetStatus` gives a fighter: the
/// lowest that no other fighter in a rebirth status holds.
pub fn halo_number(others: impl Iterator<Item = (AnyStatus, u8)> + Clone) -> u8 {
    let mut n = 0;
    while others
        .clone()
        .any(|(status, halo)| is_rebirth_status(status) && halo == n)
    {
        n += 1;
    }
    n
}

/// `ftManagerInitFighter` with `dFTManagerDefaultFighterDesc`: everything
/// but the fighter's identity and loaded data goes back to its spawn state.
fn reinit(f: &mut Fighter, pos: Vec3, facing: Facing) {
    let mut fresh = Fighter::new(f.kind, f.port, f.stocks);
    // `ftManagerMakeFighter` sets `team`; `ftManagerInitFighter` keeps it.
    fresh.team = f.team;
    fresh.attributes = f.attributes;
    fresh.coll = f.coll;
    fresh.cliff_reach = f.cliff_reach;
    fresh.cliff_air_mask = f.cliff_air_mask;
    fresh.anim = f.anim;
    fresh.costume = f.costume;
    fresh.handicap = f.handicap;
    fresh.input = f.input;
    fresh.prev_input = f.prev_input;
    fresh.stale = f.stale;
    fresh.dead = DeadState {
        bounds: f.dead.bounds,
        stock_rule: f.dead.stock_rule,
        camera_eye: f.dead.camera_eye,
        falls: f.dead.falls,
        ..DeadState::default()
    };
    fresh.pos = pos;
    fresh.facing = facing;
    *f = fresh;
}

/// `ftCommonRebirthDownSetStatus`: the fighter comes back at the top of the
/// stage, above its halo's point, and is lowered onto it.
pub fn rebirth_down(f: &mut Fighter, halo_number: u8) {
    let Some(bounds) = f.dead.bounds else {
        f.dead.rebirth_pending = false;
        return;
    };
    let offset_x = REBIRTH_OFFSETS_X[usize::from(halo_number.min(3))];
    let halo = Vec3::new(offset_x + bounds.rebirth.x, bounds.rebirth.y, 0.0);
    reinit(f, Vec3::new(halo.x, bounds.map.top, 0.0), f.facing);
    // `mpCommonSetFighterGround` with the fake floor `-2`.
    f.situation = Situation::Ground;
    f.floor = None;
    status::set_any_status_preserve(
        f,
        Status::RebirthDown.into(),
        REBIRTH_DOWN_FRAME_BEGIN,
        StatusTiming::unknown(),
        Preserve::NONE,
    );
    status::play_anim_events(f);
    crate::physics::stop_all(&mut f.physics);
    let r = &mut f.dead.rebirth;
    r.halo_lower_wait = HALO_LOWER_WAIT;
    r.halo_despawn_wait = HALO_DESPAWN_WAIT;
    r.pos = f.pos;
    r.halo_offset = halo;
    r.halo_number = halo_number;
    f.dead.is_ghost = true;
    f.is_shadow_hidden = true;
    f.dead.is_rebirth = true;
    f.dead.camera_mode = CameraMode::Ghost;
    f.colanim
        .check_set(crate::colanim::ColAnimId::FighterRebirth, 0);
}

/// `FTSTATUS_PRESERVE_PLAYERTAG | FTSTATUS_PRESERVE_EFFECT |
/// FTSTATUS_PRESERVE_COLANIM`: the halo and the glow carry on.
const REBIRTH_PRESERVE: Preserve = Preserve {
    colanim: true,
    ..Preserve::NONE
};

/// `ftCommonRebirthStandSetStatus`.
fn set_rebirth_stand(f: &mut Fighter) {
    let timing = match crate::motion::anim_length(f.kind, Status::RebirthStand.into()) {
        Some(len) => StatusTiming::frames(len),
        None => StatusTiming::unknown(),
    };
    status::set_any_status_preserve(
        f,
        Status::RebirthStand.into(),
        0.0,
        timing,
        REBIRTH_PRESERVE,
    );
    status::play_anim_events(f);
    f.dead.is_ghost = true;
    f.is_shadow_hidden = true;
    f.dead.is_rebirth = true;
}

/// `ftCommonRebirthWaitSetStatus`.
fn set_rebirth_wait(f: &mut Fighter) {
    status::set_any_status_preserve(
        f,
        Status::RebirthWait.into(),
        0.0,
        StatusTiming::unknown(),
        REBIRTH_PRESERVE,
    );
    f.dead.is_ghost = true;
    f.is_shadow_hidden = true;
    f.dead.is_rebirth = true;
}

/// The position step and `proc_map` of these statuses, in place of the
/// ordinary ground and air ticks. The `Dead*` statuses have neither
/// `proc_physics` nor `proc_map`: the fighter only moves by its air
/// velocity. The rebirth statuses lower the fighter along
/// `ftCommonRebirthCommonProcMap`'s curve. Returns `false` for any other
/// status.
pub fn tick_status(f: &mut Fighter) -> bool {
    match f.status.status {
        AnyStatus::Common(
            Status::DeadDown | Status::DeadLeftRight | Status::DeadUpStar | Status::DeadUpFall,
        ) => {
            f.pos += f.physics.vel_air;
            true
        }
        AnyStatus::Common(Status::RebirthDown | Status::RebirthStand | Status::RebirthWait) => {
            let r = f.dead.rebirth;
            let lower = r.halo_lower_wait as f32;
            f.pos.y = ((r.pos.y - r.halo_offset.y) / 8100.0) * (lower * lower) + r.halo_offset.y;
            true
        }
        _ => false,
    }
}

#[cfg(test)]
#[path = "dead_tests.rs"]
mod tests;
