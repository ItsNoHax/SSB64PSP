//! Where a fighter's weapon maker reads its spawn point, by the process
//! the original runs it in.
//!
//! `ftMainProcUpdateInterrupt` plays the frame (`ftMainPlayAnimEventsAll`,
//! ftmain.c:1398) before `proc_update` (:1501). `ftMainProcPhysicsMap` runs
//! `proc_physics` (:1763), the map step (:1836), `proc_slope`,
//! `ftParamsUpdateFighterPartsTransformAll` (:1846), the forward effects,
//! then `proc_accessory` (:1857). The port's status update runs before the
//! frame's play and samples its joints after the map step, so a maker
//! queues its weapon and the host re-places it:
//!
//! - [`SpawnPhase::Update`]: makers in `proc_update` or a setter read this
//!   frame's pose with TopN before the physics move. The host samples the
//!   joints after the play and calls [`place_update`]. When a status set
//!   replaced the maker's status after the make ([`on_set`]), the pose is
//!   the replaced status's play ([`SpawnPose::played`]): the host plays it
//!   aside and calls [`place_played`].
//! - [`SpawnPhase::Accessory`]: makers in `proc_accessory` read the pose
//!   and TopN the map step and parts pass left. [`place_accessory`] runs
//!   with those samples (`item_use::accessory`).
//! - [`SpawnPhase::Switch`]: a status set inside `ftMainProcPhysicsMap`
//!   (a landing, a lost floor) runs the old status's `proc_accessory`
//!   first (`ftMainSetStatus`, ftmain.c:4397-4403): the pose is the old
//!   motion's play with TopN where the map step put it. The host samples
//!   before the new motion's first parse and calls [`place_switch`].

use crate::colanim::ColAnimId;
use crate::fighter::Fighter;
use crate::status::StatusState;
use crate::weapon::WeaponKind;
use ssb_engine::math::Vec3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpawnPhase {
    Update,
    Accessory,
    Switch,
}

/// One maker's `gmCollisionGetFighterPartsWorldPosition` call and the
/// edits it makes to the result.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpawnPose {
    pub phase: SpawnPhase,
    /// `fp->joints[joint]`.
    pub joint: u8,
    /// The `pos` passed in: an offset along the joint's axes.
    pub local: Vec3,
    /// Added in world space afterwards; x is scaled by `fp->lr`.
    pub world: Vec3,
    /// The maker then sets y itself (Pikachu's Thunder,
    /// `map_bound_top - 500`): the queued y stays.
    pub keep_y: bool,
    /// The maker then sets `pos.z = 0`.
    pub zero_z: bool,
    /// `ftParamCheckSetFighterColAnimID(id, 0)` after the make (the Thunder
    /// Jolts' `proc_accessory`).
    pub colanim: Option<ColAnimId>,
    /// `status.entry` at the queue ([`Fighter::queue_weapon_spawn_posed`]).
    pub entry: u32,
    /// An [`SpawnPhase::Update`] maker whose status a set then replaced
    /// (or a setter that queries before its motion plays): the status whose
    /// play this frame the query read.
    pub played: Option<StatusState>,
}

impl SpawnPose {
    pub const fn update(joint: u8, local: Vec3) -> Self {
        Self {
            phase: SpawnPhase::Update,
            joint,
            local,
            world: Vec3::ZERO,
            keep_y: false,
            zero_z: false,
            colanim: None,
            entry: 0,
            played: None,
        }
    }

    pub const fn accessory(joint: u8, local: Vec3) -> Self {
        Self {
            phase: SpawnPhase::Accessory,
            ..Self::update(joint, local)
        }
    }

    pub const fn with_world(self, world: Vec3) -> Self {
        Self { world, ..self }
    }

    pub const fn keep_y(self) -> Self {
        Self {
            keep_y: true,
            ..self
        }
    }

    pub const fn zero_z(self) -> Self {
        Self {
            zero_z: true,
            ..self
        }
    }

    pub const fn with_colanim(self, id: ColAnimId) -> Self {
        Self {
            colanim: Some(id),
            ..self
        }
    }

    /// The query reads `status`'s play this frame, which a set has
    /// replaced (`ftBossYubideppou3SetStatus` sets, then queries, with no
    /// play between).
    pub const fn played(self, status: StatusState) -> Self {
        Self {
            played: Some(status),
            ..self
        }
    }

    /// The spawn point from `f`'s current joint samples; `None` when a joint
    /// below TopN has no sample (a host fighter without a skeleton).
    /// `mark`: the query marks the joint's chain ([`Fighter::joint_query`]);
    /// a query a later status set wiped does not.
    pub fn point(&self, f: &Fighter, queued: Vec3, mark: bool) -> Option<Vec3> {
        if self.joint != 0
            && f.joint_transforms
                .get(usize::from(self.joint))
                .copied()
                .flatten()
                .is_none()
        {
            return None;
        }
        let mut p = if mark {
            f.joint_query(self.joint, self.local)
        } else {
            f.joint_world(self.joint, self.local)
        };
        p.x += self.world.x * f.facing.sign();
        p.y += self.world.y;
        p.z += self.world.z;
        if self.keep_y {
            p.y = queued.y;
        }
        if self.zero_z {
            p.z = 0.0;
        }
        Some(p)
    }
}

fn pending(f: &Fighter) -> Option<SpawnPose> {
    f.weapon_spawn.and(f.weapon_spawn_pose)
}

/// Whether the queued weapon's maker is a [`SpawnPhase::Update`] one the
/// host has to re-place from this frame's play.
pub fn update_pending(f: &Fighter) -> bool {
    pending(f).is_some_and(|p| p.phase == SpawnPhase::Update && p.played.is_none())
}

/// Whether the host samples this frame's play before the physics step:
/// an [`update_pending`] weapon to re-place, or a `proc_physics` maker
/// whose query reads that play ([`crate::boss::physics_queries_joints`]).
pub fn sample_before_physics(f: &Fighter) -> bool {
    update_pending(f)
        || crate::boss::physics_queries_joints(f)
        || crate::yoshi::physics_queries_joints(f)
}

/// The status whose play this frame the queued weapon's maker read, when a
/// set has replaced it ([`SpawnPose::played`]): the host plays that motion
/// one frame aside, samples it and calls [`place_played`]. Call right
/// after the status update, before the frame's own play.
pub fn played_status(f: &Fighter) -> Option<StatusState> {
    pending(f)
        .filter(|p| p.phase == SpawnPhase::Update)
        .and_then(|p| p.played)
}

/// Whether a status set inside the physics and map step left a weapon for
/// [`place_switch`]: sample before the new motion's first parse.
pub fn switch_pending(f: &Fighter) -> bool {
    pending(f).is_some_and(|p| p.phase == SpawnPhase::Switch)
}

/// Re-places a [`SpawnPhase::Update`] weapon. Call with `joint_transforms`
/// sampled from this frame's play, before the physics move.
pub fn place_update(f: &mut Fighter) {
    place(f, SpawnPhase::Update, false);
}

/// Re-places an [`SpawnPhase::Update`] weapon whose maker read a replaced
/// status's play. Call with `joint_transforms` sampled from that play
/// ([`played_status`]).
pub fn place_played(f: &mut Fighter) {
    place(f, SpawnPhase::Update, true);
}

/// Re-places a [`SpawnPhase::Accessory`] weapon. Call with
/// `joint_transforms` sampled after the map step.
pub fn place_accessory(f: &mut Fighter) {
    place(f, SpawnPhase::Accessory, false);
}

/// Re-places a [`SpawnPhase::Switch`] weapon. Call with `joint_transforms`
/// sampled after the map step, from the old motion's play.
pub fn place_switch(f: &mut Fighter) {
    place(f, SpawnPhase::Switch, false);
}

/// `ftMainSetStatus` about to replace the status. Call before the set
/// changes anything.
///
/// - An [`SpawnPhase::Update`] maker of this status read its play:
///   [`SpawnPose::played`] keeps the status.
/// - Inside the frame's passes (`is_events_forward`) the old status's
///   `proc_accessory` runs now (ftmain.c:4397-4403): an
///   [`SpawnPhase::Accessory`] maker of this status makes its weapon here,
///   with the colour animation and `fp->ga` of this moment. Before the
///   physics step its pose is this status's play (as `Update`); inside
///   it, the play with TopN after the map step ([`SpawnPhase::Switch`]).
pub fn on_set(f: &mut Fighter) {
    let Some(mut pose) = pending(f) else {
        return;
    };
    if pose.entry != f.status.entry || pose.played.is_some() {
        return;
    }
    match pose.phase {
        SpawnPhase::Update => pose.played = Some(f.status),
        SpawnPhase::Accessory if f.motion_script.is_events_forward => {
            made_in_accessory(f, &pose);
            if f.physics_pass {
                pose.phase = SpawnPhase::Switch;
            } else {
                pose.phase = SpawnPhase::Update;
                pose.played = Some(f.status);
            }
        }
        _ => return,
    }
    f.weapon_spawn_pose = Some(pose);
}

/// What a `proc_accessory` maker does besides the query: the colour
/// animation, and `fp->ga` picking the PK Fire's spark
/// (`ftNessSpecialNProcAccessory`).
fn made_in_accessory(f: &mut Fighter, pose: &SpawnPose) {
    if let Some(id) = pose.colanim {
        crate::colanim::check_set(f, id, 0);
    }
    let ground = f.is_grounded();
    if let Some(WeaponKind::NessPKFire { grounded }) = f.weapon_spawn.as_mut().map(|s| &mut s.kind)
    {
        *grounded = ground;
    }
}

fn place(f: &mut Fighter, phase: SpawnPhase, played: bool) {
    let Some(pose) = f
        .weapon_spawn_pose
        .filter(|p| p.phase == phase && p.played.is_some() == played)
    else {
        return;
    };
    let Some(queued) = f.weapon_spawn.map(|w| w.position) else {
        return;
    };
    // One query per make.
    f.weapon_spawn_pose = None;
    // A `proc_update` maker whose status a set the port did not see
    // replaced: the queued sample stays.
    if phase == SpawnPhase::Update && !played && pose.entry != f.status.entry {
        return;
    }
    if phase == SpawnPhase::Accessory {
        made_in_accessory(f, &pose);
    }
    // The `Update` query marked the chain at the queue; a set after it
    // cleared the marks (`played`), as it did the `Switch` query's.
    let mark = phase == SpawnPhase::Accessory;
    if let Some(p) = pose.point(f, queued, mark) {
        if let Some(spawn) = f.weapon_spawn.as_mut() {
            spawn.position = p;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::colanim::ColAnimId;
    use crate::fighter::{Facing, FighterKind, JointTransform, Situation};
    use crate::status::{self, AnyStatus};
    use crate::weapon::WeaponSpawn;

    /// A joint at `origin` whose X axis is world +Y, scaled by 2.
    fn turned(origin: Vec3) -> Option<JointTransform> {
        Some(JointTransform {
            axes: [
                Vec3::new(0.0, 2.0, 0.0),
                Vec3::new(-2.0, 0.0, 0.0),
                Vec3::new(0.0, 0.0, 2.0),
            ],
            origin,
        })
    }

    fn kirby(copy: FighterKind) -> Fighter {
        let mut f = Fighter::new(FighterKind::Kirby, 0, 3);
        f.situation = Situation::Ground;
        f.kirby.copy_id = copy;
        f
    }

    fn until_made(f: &mut Fighter, step: fn(&mut Fighter)) {
        for _ in 0..80 {
            if f.weapon_spawn.is_some() {
                return;
            }
            step(f);
        }
        panic!("no weapon made");
    }

    fn kirby_step(f: &mut Fighter) {
        status::play_anim(f);
        crate::kirby_copy::update(f);
    }

    fn spawn(f: &Fighter) -> WeaponSpawn {
        f.weapon_spawn.expect("the weapon stays queued")
    }

    /// `ftKirbyCopyFoxSpecialNProcUpdate`: joint 17, 70 along its X, from
    /// this frame's play; not Fox's 60 (`FTFOX_BLASTER_SPAWN_OFF_X`).
    #[test]
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
    fn the_copied_blaster_leaves_joint_17_at_70_from_this_frames_play() {
        let mut f = kirby(FighterKind::Fox);
        f.joint_transforms[17] = turned(Vec3::new(1.0, 2.0, 3.0));
        crate::kirby_copy::set_special_n(&mut f);
        until_made(&mut f, kirby_step);
        assert!(update_pending(&f));
        f.joint_transforms[17] = turned(Vec3::new(100.0, 300.0, 50.0));
        place_update(&mut f);
        assert_eq!(spawn(&f).position, Vec3::new(100.0, 440.0, 50.0));
        // Made: neither hook moves it again.
        f.joint_transforms[17] = turned(Vec3::ZERO);
        place_update(&mut f);
        crate::item_use::accessory(&mut f);
        assert_eq!(spawn(&f).position, Vec3::new(100.0, 440.0, 50.0));
    }

    /// `ftKirbyCopyPikachuSpecialNProcAccessory`: TopN after the map step,
    /// plus world offsets (x by `fp->lr`).
    #[test]
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
    fn the_copied_thunder_jolt_reads_topn_after_the_map_step() {
        let mut f = kirby(FighterKind::Pikachu);
        f.facing = Facing::Left;
        f.joint_transforms[0] = turned(Vec3::new(1.0, 2.0, 3.0));
        crate::kirby_copy::set_special_n(&mut f);
        until_made(&mut f, kirby_step);
        assert!(!update_pending(&f));
        f.joint_transforms[0] = turned(Vec3::new(100.0, 300.0, 50.0));
        place_update(&mut f);
        assert_eq!(spawn(&f).position, f.pos);
        // The colour animation comes with the make, in the accessory
        // (ftkirbycopypikachuspecialn.c:18-43).
        assert_ne!(f.colanim.id, ColAnimId::FIGHTER_PIKACHU_SPECIAL_N);
        f.joint_transforms[0] = turned(Vec3::new(100.0, 300.0, 50.0));
        crate::item_use::accessory(&mut f);
        assert_eq!(spawn(&f).position, Vec3::new(-100.0, 500.0, 50.0));
        assert_eq!(f.colanim.id, ColAnimId::FIGHTER_PIKACHU_SPECIAL_N);
    }

    /// `ftNessSpecialNProcAccessory`: TopN, `fp->lr` and `fp->ga` as the
    /// map step left them; `pos.z = 0`.
    #[test]
    fn pk_fire_reads_topn_and_the_situation_after_the_map_step() {
        let mut f = Fighter::new(FighterKind::Ness, 0, 3);
        status::set_wait(&mut f);
        f.facing = Facing::Left;
        crate::ness::set_special_n(&mut f);
        until_made(&mut f, status::update);
        assert_eq!(spawn(&f).kind, WeaponKind::NessPKFire { grounded: true });
        // The map step: off a ledge and down.
        assert!(crate::ness::on_ground_lost(&mut f));
        f.joint_transforms[0] = turned(Vec3::new(100.0, -40.0, 50.0));
        crate::item_use::accessory(&mut f);
        let made = spawn(&f);
        assert_eq!(made.kind, WeaponKind::NessPKFire { grounded: false });
        assert_eq!(made.position, Vec3::new(0.0, 140.0, 0.0));
    }

    /// `ftSamusSpecialNEndProcUpdate` reads joint 16 after the play, before
    /// the physics move.
    #[test]
    fn the_charge_shot_leaves_joint_16_from_this_frames_play() {
        let mut f = Fighter::new(FighterKind::Samus, 0, 3);
        f.situation = Situation::Ground;
        f.samus.charge_level = crate::samus::CHARGE_MAX;
        crate::samus::set_special_n(&mut f);
        until_made(&mut f, status::update);
        assert!(update_pending(&f));
        f.joint_transforms[16] = turned(Vec3::new(100.0, 300.0, 50.0));
        place_update(&mut f);
        assert_eq!(spawn(&f).position, Vec3::new(100.0, 660.0, 50.0));
    }

    /// `ftPikachuSpecialLwStartProcUpdate` makes the Thunder from this
    /// frame's play, then its anim end sets the Loop
    /// (ftpikachuspeciallw.c:21-58), whose `ftMainSetStatus` plays nothing:
    /// the host places it from the Start's play, not the Loop's.
    #[test]
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
    fn a_maker_followed_by_a_status_set_reads_the_replaced_status_play() {
        let mut f = Fighter::new(FighterKind::Pikachu, 0, 3);
        status::set_fall(&mut f);
        f.pikachu.map_bound_top = Some(5000.0);
        f.joint_transforms[11] = turned(Vec3::new(123.0, 456.0, 78.0));
        crate::pikachu::set_special_lw(&mut f);
        until_made(&mut f, status::update);
        let played = played_status(&f).expect("the set kept the maker's status");
        assert_eq!(
            played.status,
            AnyStatus::Pikachu(status::PikachuStatus::SpecialAirLwStart)
        );
        assert_ne!(f.status.status, played.status);
        assert!(!update_pending(&f));
        f.joint_transforms[11] = turned(Vec3::new(1.0, 2.0, 3.0));
        place_update(&mut f);
        assert_eq!(spawn(&f).position, Vec3::new(123.0, 4500.0, 78.0));
        place_played(&mut f);
        assert_eq!(spawn(&f).position, Vec3::new(1.0, 4500.0, 3.0));
        assert!(f.weapon_spawn_pose.is_none(), "one query per make");
    }

    /// A Thunder Jolt made this frame, its `proc_accessory` still to run.
    fn pending_jolt() -> Fighter {
        let mut f = Fighter::new(FighterKind::Pikachu, 0, 3);
        status::set_fall(&mut f);
        crate::pikachu::set_special_n(&mut f);
        until_made(&mut f, status::update);
        assert_eq!(pending(&f).map(|p| p.phase), Some(SpawnPhase::Accessory));
        f
    }

    /// A landing in the map step: `ftMainSetStatus` runs the old status's
    /// `proc_accessory` first (ftmain.c:4397-4403), so the jolt and its
    /// colour animation are made there, from the frame's play with TopN
    /// after the move, before the Landing parses its motion; the new
    /// status's accessory makes nothing.
    #[test]
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
    fn a_map_step_switch_makes_the_jolt_in_the_old_accessory() {
        let mut f = pending_jolt();
        f.motion_script.is_events_forward = true;
        f.physics_pass = true;
        assert!(crate::pikachu::on_landing(
            &mut f,
            0.0,
            ssb_engine::math::Vec2::new(0.0, 1.0)
        ));
        assert!(switch_pending(&f));
        assert_eq!(f.colanim.id, ColAnimId::FIGHTER_PIKACHU_SPECIAL_N);
        f.joint_transforms[11] = turned(Vec3::new(7.0, 8.0, 9.0));
        place_switch(&mut f);
        assert_eq!(spawn(&f).position, Vec3::new(7.0, 8.0, 9.0));
        f.joint_transforms[11] = turned(Vec3::ZERO);
        crate::item_use::accessory(&mut f);
        assert_eq!(spawn(&f).position, Vec3::new(7.0, 8.0, 9.0));
    }

    /// The same set before the physics step (`proc_update` or
    /// `proc_interrupt`): the old accessory reads that frame's play, TopN
    /// before the move, as an update maker.
    #[test]
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
    fn an_update_set_makes_the_jolt_from_the_replaced_play() {
        let mut f = pending_jolt();
        let replaced = f.status;
        f.motion_script.is_events_forward = true;
        // The air-ground switch keeps the colour animation
        // (`FTSTATUS_PRESERVE_COLANIM`, ftmain.c:4445).
        assert!(crate::pikachu::on_landing(
            &mut f,
            0.0,
            ssb_engine::math::Vec2::new(0.0, 1.0)
        ));
        assert_eq!(played_status(&f), Some(replaced));
        assert_eq!(f.colanim.id, ColAnimId::FIGHTER_PIKACHU_SPECIAL_N);
        f.joint_transforms[11] = turned(Vec3::new(4.0, 5.0, 6.0));
        place_played(&mut f);
        assert_eq!(spawn(&f).position, Vec3::new(4.0, 5.0, 6.0));
    }

    /// Outside the frame's passes `ftMainSetStatus` runs no accessory.
    #[test]
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
    fn a_set_outside_the_passes_leaves_the_accessory_maker() {
        let mut f = pending_jolt();
        f.motion_script.is_events_forward = false;
        crate::pikachu::set_special_n(&mut f);
        assert_eq!(pending(&f).map(|p| p.phase), Some(SpawnPhase::Accessory));
        assert_ne!(f.colanim.id, ColAnimId::FIGHTER_PIKACHU_SPECIAL_N);
    }

    /// Without a set after it, an update maker takes this frame's samples;
    /// the edits after the query apply to them.
    #[test]
    fn an_update_maker_keeps_its_own_y_and_zeroes_z_on_the_new_sample() {
        let mut f = Fighter::new(FighterKind::Pikachu, 0, 3);
        f.facing = Facing::Left;
        f.joint_transforms[11] = turned(Vec3::new(1.0, 2.0, 3.0));
        let made = WeaponSpawn {
            kind: WeaponKind::PikachuThunder,
            owner_port: 0,
            team: 0,
            stale: crate::stale::WeaponStale::of(&f),
            position: Vec3::new(0.0, 4500.0, 0.0),
            facing: -1.0,
        };
        let pose = SpawnPose::update(11, Vec3::new(10.0, 0.0, 0.0))
            .with_world(Vec3::new(5.0, 0.0, 0.0))
            .keep_y()
            .zero_z();
        f.queue_weapon_spawn_posed(made, pose);
        assert_eq!(spawn(&f).position, Vec3::new(-4.0, 4500.0, 0.0));
        f.joint_transforms[11] = turned(Vec3::new(100.0, 300.0, 50.0));
        place_accessory(&mut f);
        assert_eq!(spawn(&f).position, Vec3::new(-4.0, 4500.0, 0.0));
        place_update(&mut f);
        assert_eq!(spawn(&f).position, Vec3::new(95.0, 4500.0, 0.0));
    }
}
