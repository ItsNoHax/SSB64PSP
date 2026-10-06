//! The battle entry — `ftcommonentry.c`: `ftCommonEntrySetStatus` hides a
//! VS fighter at its spawn until `ifCommonEntryFocusThread` calls
//! `ftCommonAppearSetStatus`, which plays the fighter's own entry clip (the
//! 32-bit `AnimJoint` slots, RE-401) from its spawn point with the model
//! facing the camera (`lr = 0`), then stands it in `Wait` there.
//!
//! The clip's lead joint (`TransN`) carries the entry's movement:
//! `ftCommonAppearProcPhysics` puts the fighter at the spawn point plus
//! TransN's translation every frame, and no map collision runs. Captain
//! Falcon's and Ness's entries have phases (`ftCaptainAppear*`,
//! `ftNessAppear*`). The entry effects (the pipe, the Arwing and the rest)
//! run on clocks here and draw in `psp-game` (RE-403, RE-425).

use ssb_engine::math::Vec3;

use crate::fighter::{Facing, Fighter, FighterKind};
use crate::status::{
    self, AnyStatus, CaptainStatus, DonkeyStatus, FoxStatus, KirbyStatus, LinkStatus, MarioStatus,
    NessStatus, PikachuStatus, PurinStatus, SamusStatus, Status, StatusTiming, YoshiStatus,
};

/// `ssb_rom::anim::SLOT_APPEAR_R` and the six after it.
pub const SLOT_APPEAR_R: usize = 607;
pub const SLOT_APPEAR_L: usize = 608;
pub const SLOT_APPEAR_R_START: usize = 609;
pub const SLOT_APPEAR_L_START: usize = 610;
pub const SLOT_APPEAR_R_END: usize = 611;
pub const SLOT_APPEAR_L_END: usize = 612;
pub const SLOT_APPEAR_WAIT: usize = 613;

/// `status_vars.common.entry` and `fp->entry_pos`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Entry {
    /// Where the fighter stood when its entry began.
    pub pos: Vec3,
    /// The facing it will take at the end; `None` outside an entry.
    pub lr: Option<Facing>,
    pub floor_line: Option<u16>,
    /// Captain Falcon's leftward entry turns the model around.
    pub is_rotate: bool,
    /// Ticks since the entry effect was made, `None` before any entry
    /// (RE-403). The effect runs on after the fighter stands.
    pub effect_ticks: Option<u16>,
    /// Ticks since the Poké Ball's rays were made
    /// (`efManagerMBallRaysMakeEffect`, on motion flag 1), `None` before
    /// (RE-425).
    pub rays_ticks: Option<u16>,
    /// `ftParamMoveDLLink(fighter_gobj, 1)`: Captain Falcon's leftward
    /// entry draws him on DL link 1, before the stage, until his TopN is
    /// nearer than z -1000 (`ftCaptainAppearStartProcUpdate`, RE-425).
    pub is_link_1: bool,
}

/// The effect `ftCommonAppearSetStatus` makes for each fighter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryEffect {
    /// `efManagerMarioEntryDokanMakeEffect` (Mario and Luigi).
    Pipe,
    /// `efManagerFoxEntryArwingMakeEffect`.
    Arwing,
    /// `efManagerDonkeyEntryTaruMakeEffect`.
    Barrel,
    /// `efManagerSamusEntryPointMakeEffect`.
    Point,
    /// `efManagerLinkEntryWaveMakeEffect` and `...BeamMakeEffect`.
    WaveAndBeam,
    /// `efManagerYoshiEntryEggMakeEffect`.
    Egg,
    /// `efManagerKirbyEntryStarMakeEffect`.
    Star,
    /// `efManagerMBallThrownMakeEffect` (Pikachu and Jigglypuff).
    PokeBall,
    /// `efManagerCaptainEntryCarMakeEffect`.
    Car,
}

/// Which entry effect a fighter makes; Ness makes none.
pub fn entry_effect(kind: FighterKind) -> Option<EntryEffect> {
    // `ftCommonEntrySetStatus`'s switch adds Metal Mario to the pipe and
    // Giant Donkey Kong to the barrel; the Polygons make none.
    Some(match kind {
        FighterKind::Mario | FighterKind::Luigi | FighterKind::MetalMario => EntryEffect::Pipe,
        FighterKind::Fox => EntryEffect::Arwing,
        FighterKind::Donkey | FighterKind::GiantDonkey => EntryEffect::Barrel,
        FighterKind::Samus => EntryEffect::Point,
        FighterKind::Link => EntryEffect::WaveAndBeam,
        FighterKind::Yoshi => EntryEffect::Egg,
        FighterKind::Kirby => EntryEffect::Star,
        FighterKind::Pikachu | FighterKind::Purin => EntryEffect::PokeBall,
        FighterKind::Captain => EntryEffect::Car,
        _ => return None,
    })
}

/// Advances the entry effect's clock and the rays', once per frame from
/// `ftMainProcPhysicsMap`'s slot (the effects' `gcPlayAnimAll`).
pub fn tick_effect_clock(f: &mut Fighter) {
    for t in [f.entry.effect_ticks.as_mut(), f.entry.rays_ticks.as_mut()]
        .into_iter()
        .flatten()
    {
        *t = t.saturating_add(1);
    }
}

/// The entry clip for a status, if it is one.
pub fn slot(s: AnyStatus) -> Option<usize> {
    Some(match s {
        AnyStatus::Mario(MarioStatus::AppearR)
        | AnyStatus::Fox(FoxStatus::AppearR)
        | AnyStatus::Donkey(DonkeyStatus::AppearR)
        | AnyStatus::Samus(SamusStatus::AppearR)
        | AnyStatus::Link(LinkStatus::AppearR)
        | AnyStatus::Yoshi(YoshiStatus::AppearR)
        | AnyStatus::Kirby(KirbyStatus::AppearR)
        | AnyStatus::Pikachu(PikachuStatus::AppearR)
        | AnyStatus::Purin(PurinStatus::AppearR) => SLOT_APPEAR_R,
        AnyStatus::Mario(MarioStatus::AppearL)
        | AnyStatus::Fox(FoxStatus::AppearL)
        | AnyStatus::Donkey(DonkeyStatus::AppearL)
        | AnyStatus::Samus(SamusStatus::AppearL)
        | AnyStatus::Link(LinkStatus::AppearL)
        | AnyStatus::Yoshi(YoshiStatus::AppearL)
        | AnyStatus::Kirby(KirbyStatus::AppearL)
        | AnyStatus::Pikachu(PikachuStatus::AppearL)
        | AnyStatus::Purin(PurinStatus::AppearL) => SLOT_APPEAR_L,
        AnyStatus::Captain(CaptainStatus::AppearRStart)
        | AnyStatus::Ness(NessStatus::AppearRStart) => SLOT_APPEAR_R_START,
        AnyStatus::Captain(CaptainStatus::AppearLStart)
        | AnyStatus::Ness(NessStatus::AppearLStart) => SLOT_APPEAR_L_START,
        AnyStatus::Captain(CaptainStatus::AppearREnd) | AnyStatus::Ness(NessStatus::AppearREnd) => {
            SLOT_APPEAR_R_END
        }
        AnyStatus::Captain(CaptainStatus::AppearLEnd) | AnyStatus::Ness(NessStatus::AppearLEnd) => {
            SLOT_APPEAR_L_END
        }
        AnyStatus::Ness(NessStatus::AppearWait) => SLOT_APPEAR_WAIT,
        _ => return None,
    })
}

/// The clip lengths RE-401 measured: 120 frames, Captain Falcon's 90 and
/// 30, Ness's 40, 50 and 30.
fn length(s: AnyStatus) -> f32 {
    match s {
        AnyStatus::Captain(CaptainStatus::AppearRStart | CaptainStatus::AppearLStart) => 90.0,
        AnyStatus::Captain(CaptainStatus::AppearREnd | CaptainStatus::AppearLEnd) => 30.0,
        AnyStatus::Ness(NessStatus::AppearRStart | NessStatus::AppearLStart) => 40.0,
        AnyStatus::Ness(NessStatus::AppearWait) => 50.0,
        AnyStatus::Ness(NessStatus::AppearREnd | NessStatus::AppearLEnd) => 30.0,
        _ => 120.0,
    }
}

/// Whether the fighter is waiting to enter or entering.
pub fn is_entering(s: AnyStatus) -> bool {
    s == AnyStatus::Common(Status::Entry) || slot(s).is_some()
}

/// `ftCommonEntrySetStatus`: hidden at the spawn until the entry.
pub fn entry_set_status(f: &mut Fighter) {
    status::set_status(f, Status::Entry, 0.0, StatusTiming::unknown());
    f.is_invisible = true;
    f.is_shadow_hidden = true;
    f.dead.is_ghost = true;
}

fn set(f: &mut Fighter, s: AnyStatus) {
    status::set_any_status(f, s, 0.0, StatusTiming::frames(length(s)));
    // `ftCommonAppearInitStatusVars`.
    f.dead.is_ghost = true;
    f.is_shadow_hidden = true;
    f.interface.tag_hide = true;
    f.motion_script.flags = [0; 4];
}

/// `ftCommonAppearSetStatus`.
pub fn appear_set_status(f: &mut Fighter) {
    if f.kind == FighterKind::Boss {
        return crate::boss::appear_set_status(f);
    }
    let right = f.facing == Facing::Right;
    f.entry = Entry {
        pos: f.pos,
        lr: Some(f.facing),
        floor_line: f.floor.map(|s| s.line),
        is_rotate: f.kind == FighterKind::Captain && !right,
        effect_ticks: entry_effect(f.kind).map(|_| 0),
        rays_ticks: None,
        is_link_1: f.kind == FighterKind::Captain && !right,
    };
    // `dFTCommonEntryAppearStatusIDs`: Metal Mario's row is Mario's and
    // Giant Donkey Kong's Donkey Kong's.
    let kind = match f.kind {
        FighterKind::MetalMario => FighterKind::Mario,
        FighterKind::GiantDonkey => FighterKind::Donkey,
        k => k,
    };
    let s = match (kind, right) {
        (FighterKind::Mario | FighterKind::Luigi, true) => AnyStatus::Mario(MarioStatus::AppearR),
        (FighterKind::Mario | FighterKind::Luigi, false) => AnyStatus::Mario(MarioStatus::AppearL),
        (FighterKind::Fox, true) => AnyStatus::Fox(FoxStatus::AppearR),
        (FighterKind::Fox, false) => AnyStatus::Fox(FoxStatus::AppearL),
        (FighterKind::Donkey, true) => AnyStatus::Donkey(DonkeyStatus::AppearR),
        (FighterKind::Donkey, false) => AnyStatus::Donkey(DonkeyStatus::AppearL),
        (FighterKind::Samus, true) => AnyStatus::Samus(SamusStatus::AppearR),
        (FighterKind::Samus, false) => AnyStatus::Samus(SamusStatus::AppearL),
        (FighterKind::Link, true) => AnyStatus::Link(LinkStatus::AppearR),
        (FighterKind::Link, false) => AnyStatus::Link(LinkStatus::AppearL),
        (FighterKind::Yoshi, true) => AnyStatus::Yoshi(YoshiStatus::AppearR),
        (FighterKind::Yoshi, false) => AnyStatus::Yoshi(YoshiStatus::AppearL),
        (FighterKind::Captain, true) => AnyStatus::Captain(CaptainStatus::AppearRStart),
        (FighterKind::Captain, false) => AnyStatus::Captain(CaptainStatus::AppearLStart),
        (FighterKind::Kirby, true) => AnyStatus::Kirby(KirbyStatus::AppearR),
        (FighterKind::Kirby, false) => AnyStatus::Kirby(KirbyStatus::AppearL),
        (FighterKind::Pikachu, true) => AnyStatus::Pikachu(PikachuStatus::AppearR),
        (FighterKind::Pikachu, false) => AnyStatus::Pikachu(PikachuStatus::AppearL),
        (FighterKind::Purin, true) => AnyStatus::Purin(PurinStatus::AppearR),
        (FighterKind::Purin, false) => AnyStatus::Purin(PurinStatus::AppearL),
        (FighterKind::Ness, true) => AnyStatus::Ness(NessStatus::AppearRStart),
        (FighterKind::Ness, false) => AnyStatus::Ness(NessStatus::AppearLStart),
        // The Polygons' rows are `nFTCommonStatusEntryNull`.
        _ => return status::set_wait(f),
    };
    set(f, s);
    // `ftCommonAppearInitStatusVars`.
    f.dead.camera_mode = crate::dead::CameraMode::Entry;
}

/// `ftCommonAppearSetPosition`: a 1P Game team member drops in from
/// halfway between `camera_bound_top` and `map_bound_top`, the camera
/// holding its start point until Go.
pub fn appear_set_position(f: &mut Fighter, camera_bound_top: f32, map_bound_top: f32) {
    f.dead.camera_mode = crate::dead::CameraMode::Entry;
    f.entry.pos = f.pos;
    f.pos.y = (camera_bound_top + map_bound_top) * 0.5;
    status::set_fall(f);
}

/// `ifCommonAnnounceGoSetStatus`'s camera half: the entry's camera mode
/// ends.
pub fn on_go(f: &mut Fighter) {
    if f.dead.camera_mode == crate::dead::CameraMode::Entry {
        f.dead.camera_mode = crate::dead::CameraMode::Default;
    }
}

/// The model's yaw while entering: `lr = 0` faces the camera, and Captain
/// Falcon's leftward entry turns it 180°. `None` outside an entry.
pub fn model_yaw(f: &Fighter) -> Option<f32> {
    slot(f.status.status)?;
    Some(if f.entry.is_rotate {
        core::f32::consts::PI
    } else {
        0.0
    })
}

/// `ftCommonAppearProcUpdate` and the phase setters.
pub fn update(f: &mut Fighter) {
    // `ftCommonAppearUpdateEffects`: flag 1 makes the Poké Ball's rays at
    // the entry position (RE-425), flag 2 shows the shadow.
    if f.motion_script.flags[1] != 0 {
        if matches!(
            f.kind,
            FighterKind::Pikachu
                | FighterKind::Purin
                | FighterKind::PolyPikachu
                | FighterKind::PolyPurin
        ) {
            f.entry.rays_ticks = Some(0);
        }
        f.motion_script.flags[1] = 0;
    }
    if f.motion_script.flags[2] != 0 {
        f.motion_script.flags[2] = 0;
        f.is_shadow_hidden = false;
    }
    // `ftCaptainAppearStartProcUpdate`: back to the fighters' link once
    // nearer than z -1000.
    if f.entry.is_link_1
        && matches!(
            f.status.status,
            AnyStatus::Captain(CaptainStatus::AppearLStart)
        )
        && f.pos.z > -1000.0
    {
        f.entry.is_link_1 = false;
    }
    if !f.status.animation_ended() {
        return;
    }
    let right = f.entry.lr != Some(Facing::Left);
    let next = match f.status.status {
        AnyStatus::Ness(NessStatus::AppearRStart | NessStatus::AppearLStart) => {
            Some(AnyStatus::Ness(NessStatus::AppearWait))
        }
        AnyStatus::Ness(NessStatus::AppearWait) => Some(AnyStatus::Ness(if right {
            NessStatus::AppearREnd
        } else {
            NessStatus::AppearLEnd
        })),
        AnyStatus::Captain(CaptainStatus::AppearRStart | CaptainStatus::AppearLStart) => {
            Some(AnyStatus::Captain(if right {
                CaptainStatus::AppearREnd
            } else {
                CaptainStatus::AppearLEnd
            }))
        }
        _ => None,
    };
    if let Some(s) = next {
        set(f, s);
        // The later phases show the shadow at once.
        f.is_shadow_hidden = false;
        return;
    }
    // The entry is over: back to the spawn, facing its way, standing.
    finish(f);
}

fn finish(f: &mut Fighter) {
    if let Some(lr) = f.entry.lr {
        f.facing = lr;
    }
    f.pos = f.entry.pos;
    f.physics.vel_air = Vec3::ZERO;
    f.situation = crate::fighter::Situation::Ground;
    f.entry.lr = None;
    // `ftCommonWaitSetStatus` from `proc_update`: Wait's interrupt runs this
    // frame too (RE-466).
    status::anim_end_set_wait(f);
}

/// `ftCommonAppearProcPhysics`, and no map step: the spawn point plus the
/// clip's TransN translation (`root_motion.translate`), mirrored across for
/// Captain Falcon's turned entry. Returns whether the fighter is entering,
/// so the caller skips its own physics and map.
pub fn tick_status(f: &mut Fighter) -> bool {
    if f.status.status == AnyStatus::Common(Status::Entry) {
        return true;
    }
    if slot(f.status.status).is_none() {
        return false;
    }
    let t = f.root_motion.translate;
    place(f, t);
    true
}

/// `ftCommonAppearProcPhysics`'s placement: the entry position plus the
/// clip's TransN translation `t`, mirrored for Captain Falcon's turned
/// entry. A runtime with a skeleton calls this again with the pose of the
/// frame just played.
pub fn place(f: &mut Fighter, t: Vec3) {
    let e = f.entry.pos;
    f.pos.y = e.y + t.y;
    if f.entry.is_rotate {
        f.pos.x = e.x - t.x;
        f.pos.z = e.z - t.z;
    } else {
        f.pos.x = e.x + t.x;
        f.pos.z = e.z + t.z;
    }
}

/// `dIFCommonEntryFocusSleepTics`.
const FOCUS_SLEEP: [u32; 3] = [22, 15, 60];

/// `ifCommonEntryFocusThread`, made at the countdown's creation with
/// `syUtilsRandIntRange(3)` as its `id`: when each fighter enters, and for
/// `id` 2 which one the camera zooms on (`gmCameraSetStatusPlayerZoom`,
/// FOV 28). Ticks count from the thread's first run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EntryFocus {
    pub id: u8,
    /// `pl_count + cp_count`: under three, the thread first sleeps its
    /// interval.
    pub count: u8,
}

/// `gmCameraSetStatusPlayerZoom`'s FOV for the entry zoom.
pub const FOCUS_ZOOM_FOV: f32 = 28.0;

impl EntryFocus {
    fn sleep(&self) -> u32 {
        FOCUS_SLEEP[usize::from(self.id.min(2))]
    }

    /// The tick fighter `i` (in link order) enters on.
    pub fn appear_tick(&self, i: usize) -> u32 {
        let lead = if self.id == 1 { 90 } else { 0 };
        let first = lead + if self.count < 3 { self.sleep() } else { 0 };
        first + i as u32 * self.sleep()
    }

    /// For `id` 2, the fighter the camera holds on at `tick`: from 30 after
    /// its entry until the next one's zoom, the last until 30 after the
    /// loop's end (`gmCameraSetStatusDefault`).
    pub fn zoom(&self, tick: u32) -> Option<usize> {
        if self.id != 2 {
            return None;
        }
        let n = usize::from(self.count);
        let end = self.appear_tick(n.saturating_sub(1)) + self.sleep() + 30;
        (0..n)
            .rev()
            .find(|&i| tick >= self.appear_tick(i) + 30)
            .filter(|_| tick < end)
    }
}

#[cfg(test)]
#[path = "appear_tests.rs"]
mod tests;
