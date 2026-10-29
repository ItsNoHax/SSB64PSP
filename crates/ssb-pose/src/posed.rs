//! [`PosedFighter`]: one fighter and the skeleton its gameplay joints are
//! sampled from, ticked in the runtime's two halves (priority 5's
//! interrupt, priority 4's physics and map) and settled after the hit
//! phase. Moved from the PSP runtime's `FighterScene`, which embeds it.

use ssb_game::status::AnyStatus;
use ssb_game::weapon::MapSurface;
use ssb_rom::pack::{ObjectDesc, Pack};

use crate::joints::{part_locals_into, set_joint_parents, set_posed_joints};
use crate::skeleton_tick::{fighter_locks, tick_skeleton_animation};

/// One fighter as a match runs it: its gameplay state, the skeleton its
/// clip poses and the joints gameplay reads are sampled from, and the
/// clip's bookkeeping.
pub struct PosedFighter {
    pub fighter: ssb_game::fighter::Fighter,
    /// The fighter's animation, and the status it was started for.
    ///
    /// Kept here rather than in `ssb-game` because starting one needs the
    /// pack, which Layer A must not know about — the same split the physics
    /// constants use.
    pub skeleton: ssb_rom::skeleton::Skeleton,
    /// Object whose nodes the skeleton drives, or `u32::MAX` when the pack has
    /// no model for this character.
    pub object: u32,
    started: Option<(AnyStatus, u32)>,
    /// The [`StatusState::clip_restarts`](ssb_game::status::StatusState)
    /// the skeleton last restarted for.
    clip_restarts_seen: u8,
    /// `YRotN`'s local pose, the shield's joint. Only the shield poses move
    /// it (RE-367); the original keeps it between shields too.
    shield_yrotn: ssb_rom::figatree::JointPose,
    /// TransN pose immediately before the last animation parser advance. On
    /// the next fighter tick it pairs with the current hidden pose to recover
    /// the exact `transn - anim_vel` delta the original physics reads.
    root_motion_before_tick: Option<ssb_rom::figatree::JointPose>,
    /// This frame's clip was already played after the interrupt: a throw's
    /// release reads the catcher's hand as `ftMainPlayAnimEventsAll` left it
    /// (RE-472).
    anim_ticked_early: bool,
}

/// What [`PosedFighter::place_played_spawn`]'s aside play may change and
/// puts back: the skeleton and its clip state, and every fighter field the
/// clip tick and the joint sample write. One struct, so a field cannot be
/// left out of either the save or the restore.
///
/// Not kept: `fighter.yoshi.egg_joint_scale`, which the sample writes only
/// for a motion with `is_use_yrotn_joint` (Yoshi's Egg Throw); the PSP
/// runtime this was moved from never restored it.
#[derive(Clone, Debug)]
pub struct Snapshot {
    skeleton: ssb_rom::skeleton::Skeleton,
    started: Option<(AnyStatus, u32)>,
    clip_restarts_seen: u8,
    root_motion_before_tick: Option<ssb_rom::figatree::JointPose>,
    shield_yrotn: ssb_rom::figatree::JointPose,
    joint_transforms:
        [Option<ssb_game::fighter::JointTransform>; ssb_game::fighter::FIGHTER_JOINTS],
    joint4_rotate_x: Option<f32>,
    joint_parents: [u8; ssb_game::fighter::FIGHTER_JOINTS],
    is_use_animlocks: bool,
    thrown_part4: Option<ssb_engine::math::Vec3>,
}

/// A span of [`PosedFighter::tick_physics_with`] a host may time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PhysicsSpan {
    /// A clip tick.
    Anim,
    /// `ftMainProcPhysicsMap`'s physics and map step.
    Map,
    /// The joint sample after the slope.
    Joints,
}

/// What a host adds to [`PosedFighter::tick_physics_with`]: timing around
/// its spans, and a look at the fighter right after the map step. The
/// defaults do nothing; `()` is the host that adds nothing.
pub trait PhysicsHooks {
    /// Runs `f`, the work of `span`.
    #[inline(always)]
    fn span<R>(&mut self, span: PhysicsSpan, f: impl FnOnce() -> R) -> R {
        let _ = span;
        f()
    }

    /// Right after the physics and map step, before anything samples.
    #[inline(always)]
    fn after_map(&mut self, fighter: &ssb_game::fighter::Fighter) {
        let _ = fighter;
    }
}

impl PhysicsHooks for () {}

impl PosedFighter {
    /// `fighter` with its skeleton as `ftManagerMakeFighter` leaves it
    /// ([`crate::fighter_skeleton`]), its clip not yet started. Build it after
    /// `fighter.attributes`: TopN carries their `size`.
    ///
    /// A host with the fighter in hand; the PSP builds its fighters in
    /// place with [`Self::write_around_fighter`].
    pub fn new(pack: &Pack<'_>, fighter: ssb_game::fighter::Fighter) -> PosedFighter {
        let mut posed = core::mem::MaybeUninit::<PosedFighter>::uninit();
        let dst = posed.as_mut_ptr();
        // SAFETY: the fighter is written first, then every other field.
        unsafe {
            core::ptr::addr_of_mut!((*dst).fighter).write(fighter);
            Self::write_around_fighter(dst, pack);
            posed.assume_init()
        }
    }

    /// [`Self::new`] in place, around the fighter already at
    /// `(*dst).fighter`: every other field is written into `*dst`, the
    /// skeleton made there and its locks set after, so no `PosedFighter`
    /// or skeleton-sized temporary lands on the PSP's game stack
    /// (`FighterScene::write_at_position`, RE-469).
    ///
    /// # Safety
    ///
    /// `dst` is valid for writes and aligned, and its `fighter` field is
    /// initialised. Every other field is overwritten without being read or
    /// dropped.
    #[inline(never)]
    pub unsafe fn write_around_fighter(dst: *mut PosedFighter, pack: &Pack<'_>) {
        use core::ptr::addr_of_mut;
        ssb_engine::all_fields!(PosedFighter {
            fighter,
            skeleton,
            object,
            started,
            clip_restarts_seen,
            shield_yrotn,
            root_motion_before_tick,
            anim_ticked_early
        });
        // SAFETY: the caller initialised the fighter; nothing else is read.
        let (kind, size) = unsafe {
            let fighter = &*addr_of_mut!((*dst).fighter);
            (fighter.kind, fighter.attributes.size)
        };
        let locks = fighter_locks(kind, size);
        // Which object a character's animations drive is stored per joint,
        // so any one of them names it; a scan over the objects finds which
        // one owns that node. Done once, here, rather than per tick.
        let object = ssb_rom::scene_deps::fighter_object(pack, kind as u32).unwrap_or(u32::MAX);
        // SAFETY: `dst` is valid for writes (the caller's contract); each
        // field is written once, the skeleton before its locks are set.
        unsafe {
            addr_of_mut!((*dst).object).write(object);
            let skeleton = addr_of_mut!((*dst).skeleton);
            ssb_rom::skeleton::Skeleton::write_new(skeleton);
            (*skeleton).locks = locks;
            addr_of_mut!((*dst).started).write(None);
            addr_of_mut!((*dst).clip_restarts_seen).write(0);
            addr_of_mut!((*dst).shield_yrotn).write(ssb_rom::figatree::JointPose::default());
            addr_of_mut!((*dst).root_motion_before_tick).write(None);
            addr_of_mut!((*dst).anim_ticked_early).write(false);
        }
    }

    /// The priority-5 half: input, runtime samples,
    /// `ftMainProcUpdateInterrupt`, then [`Self::after_interrupt`].
    /// `map_bound_top` is the stage's `MPGroundData::map_bound_top`
    /// (Pikachu's Thunder reads it); `None` leaves the fighter's as it is.
    /// `tapped` and `released` are the jump button's edges this frame.
    pub fn tick_interrupt<I, F>(
        &mut self,
        pack: &Pack<'_>,
        map_bound_top: Option<f32>,
        input: ssb_engine::input::ControllerState,
        tapped: bool,
        released: bool,
        surfaces: &F,
    ) where
        F: Fn() -> I,
        I: IntoIterator<Item = MapSurface>,
    {
        self.before_interrupt(map_bound_top, input, tapped, released);
        self.fighter.tick_interrupt(surfaces);
        self.after_interrupt(pack);
    }

    /// [`Self::tick_interrupt`] up to `ftMainProcUpdateInterrupt`: the
    /// input, the held fighter's TopN child and TransN's step.
    pub fn before_interrupt(
        &mut self,
        map_bound_top: Option<f32>,
        input: ssb_engine::input::ControllerState,
        tapped: bool,
        released: bool,
    ) {
        if let Some(top) = map_bound_top {
            self.fighter.pikachu.map_bound_top = Some(top);
        }
        self.fighter.set_input(input, tapped, released);
        self.sample_held_child_offset();
        // Every status gets its TransN sample: `ssb-game`'s physics reads it
        // only where the source's `proc_physics` applies TransN, so a list
        // here could only miss statuses (Final Cutter, the Falcon Kick,
        // RE-378).
        if let (Some(before), Some(current)) = (self.root_motion_before_tick, self.skeleton.pose(0))
        {
            self.fighter.set_root_motion(ssb_game::physics::RootMotion {
                delta: ssb_engine::math::Vec3::new(
                    current.translate[0] - before.translate[0],
                    current.translate[1] - before.translate[1],
                    current.translate[2] - before.translate[2],
                ),
                rotate_z: current.rotate[2],
                translate: ssb_engine::math::Vec3::new(
                    current.translate[0],
                    current.translate[1],
                    current.translate[2],
                ),
            });
        }
    }

    /// [`Self::tick_interrupt`] after `ftMainProcUpdateInterrupt`: a
    /// replaced maker's placement, and a throw's release from this frame's
    /// play.
    pub fn after_interrupt(&mut self, pack: &Pack<'_>) {
        self.place_played_spawn(pack);
        // `ftCommonThrowProcUpdate` releases from the hand this frame's play
        // has posed (`ftCommonThrownProcPhysics` then the release), before
        // `ftMainProcPhysicsMap`: play the clip now and sample the hand for
        // the release `grab::exchange` delivers next.
        if ssb_game::grab::release_pending(&self.fighter) && !self.fighter.is_in_hitlag() {
            self.tick_animation(pack);
            self.anim_ticked_early = true;
            self.sample_gameplay_joints(pack);
            let hand = self.hand_transform(pack);
            self.fighter.grab.anchor = hand.map(|j| j.origin);
            self.fighter.grab.anchor_transform = hand;
        }
    }

    /// Everything [`Self::place_played_spawn`]'s aside play may change.
    pub fn snapshot(&self) -> Snapshot {
        let mut s = core::mem::MaybeUninit::<Snapshot>::uninit();
        // SAFETY: a local slot, which `write_snapshot` fills.
        unsafe {
            self.write_snapshot(s.as_mut_ptr());
            s.assume_init()
        }
    }

    /// [`Self::snapshot`] written into `dst` field by field: built by value,
    /// its 26 KB skeleton is cloned into a stack temporary and copied
    /// (RE-469).
    ///
    /// # Safety
    ///
    /// `dst` must be valid for writes and aligned; its old contents are
    /// overwritten without being dropped.
    unsafe fn write_snapshot(&self, dst: *mut Snapshot) {
        use core::ptr::addr_of_mut;
        ssb_engine::all_fields!(Snapshot {
            skeleton,
            started,
            clip_restarts_seen,
            root_motion_before_tick,
            shield_yrotn,
            joint_transforms,
            joint4_rotate_x,
            joint_parents,
            is_use_animlocks,
            thrown_part4
        });
        // SAFETY: every field of `*dst` is written once, through raw
        // pointers.
        unsafe {
            ssb_rom::skeleton::Skeleton::write_clone(addr_of_mut!((*dst).skeleton), &self.skeleton);
            addr_of_mut!((*dst).started).write(self.started);
            addr_of_mut!((*dst).clip_restarts_seen).write(self.clip_restarts_seen);
            addr_of_mut!((*dst).root_motion_before_tick).write(self.root_motion_before_tick);
            addr_of_mut!((*dst).shield_yrotn).write(self.shield_yrotn);
            addr_of_mut!((*dst).joint_transforms).write(self.fighter.joint_transforms);
            addr_of_mut!((*dst).joint4_rotate_x).write(self.fighter.joint4_rotate_x);
            addr_of_mut!((*dst).joint_parents).write(self.fighter.joint_parents);
            addr_of_mut!((*dst).is_use_animlocks).write(self.fighter.is_use_animlocks);
            addr_of_mut!((*dst).thrown_part4).write(self.fighter.grab.thrown_part4);
        }
    }

    /// Puts back what [`Self::snapshot`] saved.
    pub fn restore(&mut self, s: Snapshot) {
        self.restore_from(&s);
    }

    /// [`Self::restore`] from a reference: the skeleton is copied straight
    /// back.
    fn restore_from(&mut self, s: &Snapshot) {
        let Snapshot {
            skeleton,
            started,
            clip_restarts_seen,
            root_motion_before_tick,
            shield_yrotn,
            joint_transforms,
            joint4_rotate_x,
            joint_parents,
            is_use_animlocks,
            thrown_part4,
        } = s;
        self.skeleton.clone_from_in_place(skeleton);
        self.started = *started;
        self.clip_restarts_seen = *clip_restarts_seen;
        self.root_motion_before_tick = *root_motion_before_tick;
        self.shield_yrotn = *shield_yrotn;
        self.fighter.joint_transforms = *joint_transforms;
        self.fighter.joint4_rotate_x = *joint4_rotate_x;
        self.fighter.joint_parents = *joint_parents;
        self.fighter.is_use_animlocks = *is_use_animlocks;
        self.fighter.grab.thrown_part4 = *thrown_part4;
    }

    /// Runs `f` aside: what it changes of [`Snapshot`] is put back after.
    /// The snapshot is written once, in this frame, and restored from
    /// there.
    fn aside<R>(&mut self, f: impl FnOnce(&mut Self) -> R) -> R {
        // Never dropped, only forgotten: it owns nothing.
        const { assert!(!core::mem::needs_drop::<Snapshot>()) };
        let mut saved = core::mem::MaybeUninit::<Snapshot>::uninit();
        // SAFETY: a local slot, which `write_snapshot` fills.
        unsafe { self.write_snapshot(saved.as_mut_ptr()) };
        let r = f(self);
        // SAFETY: written above.
        self.restore_from(unsafe { saved.assume_init_ref() });
        r
    }

    /// `ftManagerMakeFighter`'s `ftComputerSetFighterDamageDetectSize`, for
    /// a human or CPU fighter: the spawn status's first pose (`Entry` and
    /// `Wait` both play `Wait` from frame 0), sampled aside, sets
    /// `damage_coll_size`. Call once, after the spawn status is set.
    pub fn set_damage_detect_size(&mut self, pack: &Pack<'_>) {
        let size = self.aside(|p| {
            p.tick_animation(pack);
            p.sample_gameplay_joints(pack);
            ssb_game::computer::behave::damage_detect_size(&p.fighter)
        });
        self.fighter.damage_coll_size = Some(size);
    }

    /// A `proc_update` maker whose status a set then replaced (Pikachu's
    /// Thunder, the boss's finger gun) read the play `ftMainPlayAnimEventsAll`
    /// gave that status this frame, before the update; the port plays after
    /// it. Play the replaced status's motion aside, sample it and place the
    /// weapon, then put the skeleton back for the new status's play.
    pub fn place_played_spawn(&mut self, pack: &Pack<'_>) {
        if self.fighter.is_in_hitlag() {
            return;
        }
        let Some(played) = ssb_game::spawn_pose::played_status(&self.fighter) else {
            return;
        };
        self.aside(|p| {
            p.play_aside(pack, played);
            ssb_game::spawn_pose::place_played(&mut p.fighter);
        });
    }

    /// `played`'s play this frame and its joint sample, with the fighter's
    /// status put back after.
    pub fn play_aside(&mut self, pack: &Pack<'_>, played: ssb_game::status::StatusState) {
        let current = core::mem::replace(&mut self.fighter.status, played);
        self.tick_animation(pack);
        self.sample_gameplay_joints(pack);
        self.fighter.status = current;
    }

    /// [`Self::tick_physics_with`] for a host that adds nothing.
    pub fn tick_physics<I, F>(&mut self, pack: &Pack<'_>, surfaces: &F)
    where
        F: Fn() -> I,
        I: IntoIterator<Item = MapSurface>,
    {
        self.tick_physics_with(pack, surfaces, &mut ());
    }

    /// The priority-4 half: `ftMainProcPhysicsMap`, then the animation and
    /// the joint samples gameplay reads.
    pub fn tick_physics_with<I, F, H>(&mut self, pack: &Pack<'_>, surfaces: &F, hooks: &mut H)
    where
        F: Fn() -> I,
        I: IntoIterator<Item = MapSurface>,
        H: PhysicsHooks,
    {
        // `ftMainProcUpdateInterrupt`'s `ftMainPlayAnimEventsAll` has played
        // this frame's pose before `ftMainProcPhysicsMap` runs, so TransN's
        // step (`translate - anim_vel`) and the entry's placement read the
        // frame the status is on (RE-466). Hitlag freezes the motion with
        // the status.
        if !self.fighter.is_in_hitlag() {
            if !core::mem::take(&mut self.anim_ticked_early) {
                hooks.span(PhysicsSpan::Anim, || self.tick_animation(pack));
            }
            if let (Some(before), Some(current)) =
                (self.root_motion_before_tick, self.skeleton.pose(0))
            {
                // The interrupt may have turned TransN (Mario's Super Jump
                // Punch); its rotation is kept.
                let rotate_z = self.fighter.root_motion.rotate_z;
                self.fighter.set_root_motion(ssb_game::physics::RootMotion {
                    delta: ssb_engine::math::Vec3::new(
                        current.translate[0] - before.translate[0],
                        current.translate[1] - before.translate[1],
                        current.translate[2] - before.translate[2],
                    ),
                    rotate_z,
                    translate: ssb_engine::math::Vec3::new(
                        current.translate[0],
                        current.translate[1],
                        current.translate[2],
                    ),
                });
            }
        } else {
            // No TransN motion accrues while frozen.
            self.root_motion_before_tick = self.skeleton.pose(0).copied();
        }
        // `ftCommonCapturePulledRotateScale` reads TransN as this frame's
        // play left it, so the `proc_map` floor probe below runs at the
        // hand's new X.
        self.sample_held_child_offset();
        // A `proc_update` maker (Fox's Blaster, RE-473) reads the joints
        // this frame's play posed, before the physics step; so does a
        // `proc_physics` maker (the boss's finger gun).
        if ssb_game::spawn_pose::sample_before_physics(&self.fighter) {
            self.sample_gameplay_joints(pack);
            ssb_game::spawn_pose::place_update(&mut self.fighter);
        }
        // A landing's `ftCommonDownBounceCheckUpOrDown` reads joint 4 as
        // this frame's play left it.
        self.refresh_joint4_rotate_x(pack);
        hooks.span(PhysicsSpan::Map, || {
            self.fighter.tick_physics_map_before_slope(surfaces)
        });
        hooks.after_map(&self.fighter);
        // A status set inside the physics and map step ran the old status's
        // `proc_accessory` first (ftmain.c:4397-4403): its maker read the
        // frame's play, TopN after the move, before the new motion parses.
        if ssb_game::spawn_pose::switch_pending(&self.fighter) {
            self.sample_gameplay_joints(pack);
            ssb_game::spawn_pose::place_switch(&mut self.fighter);
        }
        // A status the map step set (a landing) parses its first frame at
        // once (`ftMainSetStatus`). A `motion_id` -1 or -2 status parses
        // nothing there (`ftmain.c:4613`): its pose stands as this frame's
        // play left it, and the next play advances (-1: Kirby's
        // `ftKirbySpecialAirNCatchSwitchStatusGround`) or holds (-2:
        // `ftCommonCapturePulledProcPhysics`'s `CaptureWait`) it.
        if !self.fighter.is_in_hitlag()
            && self.started != Some((self.fighter.status.status, self.fighter.status.entry))
        {
            if self.fighter.status.status.keeps_motion() {
                self.started = Some((self.fighter.status.status, self.fighter.status.entry));
                self.skeleton.speed = ssb_game::status::clip_speed(&self.fighter);
            } else {
                hooks.span(PhysicsSpan::Anim, || self.tick_animation(pack));
            }
        }
        self.sample_held_child_offset();
        if (ssb_game::map::is_cliff_hold(self.fighter.status.status)
            || ssb_game::map::is_cliff_phase2(self.fighter.status.status))
            && self.skeleton.joint_node(0).is_none()
        {
            if let Some(pose) = self.skeleton.pose(0) {
                self.fighter.transn = ssb_engine::math::Vec3::new(
                    pose.translate[0],
                    pose.translate[1],
                    pose.translate[2],
                );
                if ssb_game::map::is_cliff_hold(self.fighter.status.status) {
                    let f = &mut self.fighter;
                    f.pos.x = f.cliff.corner.x + f.transn.z * f.facing.sign() * f.attributes.size;
                    f.pos.y = f.cliff.corner.y + f.transn.y * f.attributes.size;
                }
            }
        }
        ssb_game::grab::refresh_held_attachment(&mut self.fighter);
        // `proc_slope`, then the parts transform pass and the forward
        // effects (`ftMainProcPhysicsMap`). The parts' locals the slope
        // composed give the pass its matrices when no leg moved: nothing
        // between touches the skeleton, so they are the ones a second
        // compose would build, bit for bit.
        self.apply_pose_writes(pack);
        let mut kept = None;
        let moved = self.slope_contour(pack, surfaces, false, &mut kept);
        self.fighter.finish_physics_map();
        hooks.span(PhysicsSpan::Joints, || {
            self.sample_gameplay_joints_from(pack, kept.as_ref().filter(|_| !moved))
        });
        // `proc_accessory` reads the updated hand, after the map and parts
        // transform pass (`ftMainProcPhysicsMap`).
        ssb_game::item_use::accessory(&mut self.fighter);
        // A held fighter hangs from this fighter's `joint_itemheavy_id`; the
        // match loop hands the sampled position over in `grab::exchange`.
        // A catch status samples it too: the catch is made in the hit phase
        // and `ftCommonCapturePulledProcCapture` places the caught fighter
        // from this frame's hand at once (RE-472).
        // The held fighter's capture processes query the hand (RE-509).
        ssb_game::grab::mark_held_queries(&self.fighter);
        self.sample_anchor(pack);
    }

    /// The skeleton's joint matrices for gameplay positions, from
    /// `lbCommonSin`, as `gmCollisionGetFighterPartsWorldPosition`
    /// builds them for hit and hurt collisions and attachments (RE-468).
    /// Display composes differently (`FighterScene::compose_model`).
    pub fn compose_collision(
        &self,
        pack: &Pack<'_>,
        object: &ObjectDesc,
        out: &mut [ssb_rom::scene::Mat4],
    ) -> usize {
        self.skeleton.compose_collision(pack, object, out)
    }

    /// The heavy-item joint a held fighter hangs from, while this fighter
    /// holds or is in a catch status; `grab::exchange` hands it over.
    pub fn sample_anchor(&mut self, pack: &Pack<'_>) {
        let holds = self.fighter.grab.catch.is_some() || self.fighter.grab.is_catchstatus;
        let hand = if holds {
            self.hand_transform(pack)
        } else {
            None
        };
        self.fighter.grab.anchor = hand.map(|joint| joint.origin);
        self.fighter.grab.anchor_transform = hand;
    }

    /// Starts the animation the current status calls for, then advances it.
    pub fn tick_animation(&mut self, pack: &Pack<'_>) {
        if self.object == u32::MAX {
            return;
        }
        self.root_motion_before_tick = tick_skeleton_animation(
            pack,
            self.fighter.kind as u32,
            pack.object(self.object).map_or(0, |o| o.first_node),
            &self.fighter.status,
            ssb_game::status::clip_speed(&self.fighter),
            &mut self.skeleton,
            &mut self.started,
            &mut self.clip_restarts_seen,
        );
        self.apply_shield_pose(pack);
    }

    /// `ftCommonGuardUpdateJoints` / `ftCommonGuardInitJoints`: the stick
    /// tilts the shield and, in `Guard`, the whole fighter (RE-367).
    fn apply_shield_pose(&mut self, pack: &Pack<'_>) {
        use ssb_game::status::ShieldPose;
        use ssb_rom::skeleton::ShieldJoints;
        let Some(pose) = ssb_game::status::shield_pose(&self.fighter) else {
            return;
        };
        let guard = &self.fighter.guard;
        let Some(anim) = pack.shield_pose(self.fighter.kind as u32, guard.angle_i as u32) else {
            return;
        };
        let joints = match pose {
            ShieldPose::ShieldJoint => ShieldJoints::ShieldJoint,
            ShieldPose::AllJoints => ShieldJoints::All,
        };
        ssb_rom::skeleton::apply_shield_pose(
            pack,
            &anim,
            guard.angle_f,
            guard.shield_rotate_range,
            joints,
            &mut self.skeleton,
            &mut self.shield_yrotn,
        );
    }

    /// A status set in the hit phase that asked for its `proc_slope`
    /// (`ftCommonCatchPullProcCatch` and the other pending-slope sites):
    /// the new pose, the slope, and the joints and hand resampled from it.
    /// Call for every fighter before [`Self::settle_capture`].
    pub fn settle_catch<I, F>(&mut self, pack: &Pack<'_>, surfaces: &F)
    where
        F: Fn() -> I,
        I: IntoIterator<Item = MapSurface>,
    {
        // `ftCommonCatchPullProcCatch`'s `proc_slope` (RE-510), on the pose
        // `ftMainSetStatus` parsed for `CatchPull`. The caught fighter's
        // `ftCommonCapturePulledProcCapture` then reads the hand from that
        // pose (`func_ovl0_800C9A38` on the DObjs as they stand), not the
        // `Catch` frame the physics pass sampled (RE-511).
        let status = &self.fighter.status;
        let fresh = self.started != Some((status.status, status.entry));
        if !self.fighter.slope_pending {
            // Any other `proc_catch` (`ftYoshiSpecialNCatchProcCatch`) sets
            // its status before the caught fighter's `proc_capture` reads
            // the joint, so that reads the new status's pose too.
            if fresh && self.fighter.grab.catch.is_some() {
                self.tick_animation(pack);
                self.sample_gameplay_joints(pack);
                self.sample_anchor(pack);
            }
            return;
        }
        if fresh {
            self.tick_animation(pack);
        }
        self.slope_contour(pack, surfaces, true, &mut None);
        self.sample_gameplay_joints(pack);
        self.sample_anchor(pack);
    }

    /// The tail of `ftCommonCapturePulledProcCapture`, for a fighter caught
    /// in this frame's hit phase: `ftMainPlayAnimEventsAll` plays
    /// `CapturePulled`'s first frame, then `ftCommonCapturePulledProcPhysics`
    /// and `ftCommonCapturePulledProcMap` place it at the catcher's hand on
    /// the catch frame itself (RE-472). `ftCommonCaptureYoshiProcCapture`
    /// likewise runs `ftCommonCaptureYoshiProcPhysics`, placing the
    /// swallowed fighter at the tongue. Call after [`Self::settle_catch`]
    /// and the `grab::exchange` that delivers the capture and the hand.
    pub fn settle_capture<I, F>(&mut self, pack: &Pack<'_>, surfaces: &F)
    where
        F: Fn() -> I,
        I: IntoIterator<Item = MapSurface>,
    {
        let status = &self.fighter.status;
        if status.status != ssb_game::status::Status::CapturePulled
            && status.status != ssb_game::status::Status::CaptureYoshi
            || self.started == Some((status.status, status.entry))
            || self.fighter.grab.holder.is_none()
        {
            return;
        }
        self.tick_animation(pack);
        self.sample_held_child_offset();
        ssb_game::grab::tick_held(&mut self.fighter, || ssb_game::map::floors(surfaces()));
        self.sample_gameplay_joints(pack);
    }

    /// `ftMainSetStatus` in the hit phase (a hit's damage status, a
    /// release): the new figatree is parsed at its first frame at once, so
    /// the pose drawn and the joints sampled through the hitlag that follows
    /// are the new status's (RE-472: Luigi's `DamageAir1` joints while a
    /// second flame meets him in hitlag). Call after the hit phase.
    pub fn settle_status(&mut self, pack: &Pack<'_>) {
        let status = &self.fighter.status;
        if self.started == Some((status.status, status.entry)) {
            return;
        }
        self.tick_animation(pack);
        self.apply_pose_writes(pack);
        self.sample_gameplay_joints(pack);
    }

    /// Puts the DObj writes a status queued (joint 4's pitch and scale,
    /// TransN's roll: [`ssb_game::model_pitch`]) on the poses, where they
    /// last until the clip's own track for that channel next plays or a
    /// status restarts the clip from its rest pose (`ftMainSetStatus`).
    pub fn apply_pose_writes(&mut self, pack: &Pack<'_>) {
        let writes = core::mem::take(&mut self.fighter.pose_writes);
        if let Some(pitch) = writes.joint4 {
            let first = pack.object(self.object).map(|o| o.first_node);
            if let Some(pose) = first.and_then(|n| self.skeleton.node_pose_mut(n)) {
                pose.rotate[0] = pitch.rotate_x;
                if let Some(scale) = pitch.scale {
                    pose.scale = scale;
                }
            }
        }
        if let Some(z) = writes.transn_rotate_z {
            if self.skeleton.joint_node(0).is_none() {
                if let Some(pose) = self.skeleton.pose_mut(0) {
                    pose.rotate[2] = z;
                }
            }
        }
    }

    /// `mpCommonUpdateFighterSlopeContour` on this skeleton: hands the
    /// parts' collision locals to the gameplay side's leg fit and puts the
    /// rotations `func_ovl2_800EBD08` wrote on the poses, where the next
    /// figatree tick overwrites the tracks it animates (RE-510). With
    /// `pending`, only a slope a status change asked for. Returns whether a
    /// leg moved. The parts' locals, when it composed them, are left in
    /// `kept` for [`Self::sample_gameplay_joints_from`].
    fn slope_contour<I, F>(
        &mut self,
        pack: &Pack<'_>,
        surfaces: &F,
        pending: bool,
        kept: &mut Option<ssb_rom::skeleton::CollisionLocals>,
    ) -> bool
    where
        F: Fn() -> I,
        I: IntoIterator<Item = MapSurface>,
    {
        if pending && !self.fighter.slope_pending {
            return false;
        }
        let Some(object) = pack.object(self.object) else {
            return false;
        };
        self.refresh_joint_parents(pack, &object);
        // `fp->is_use_animlocks` of the clip the map step left playing.
        self.fighter.is_use_animlocks = self.skeleton.locks.flagged;
        // Composed only when the leg fit reads them: not in the air, in
        // hitlag or off a floor.
        // Without them no foot reads the parts (the gate fails first or no
        // foot flag is set), so none are passed rather than empty ones.
        let parts = if self.fighter.slope_contour != 0 && self.fighter.slope_contour_reads_parts() {
            let kept = kept.insert(ssb_rom::skeleton::CollisionLocals::new());
            Some(part_locals_into(&self.skeleton, pack, &object, kept))
        } else {
            None
        };
        let fits = if pending {
            self.fighter
                .run_pending_slope_contour(surfaces, parts.as_ref())
        } else {
            self.fighter.slope_contour(surfaces, parts.as_ref())
        };
        let mut moved = false;
        for fit in fits.into_iter().flatten() {
            let node = |joint: u8| object.first_node + u32::from(joint) - 4;
            if let Some(pose) = self.skeleton.node_pose_mut(node(fit.thigh)) {
                pose.rotate = fit.thigh_rotate;
                moved = true;
            }
            if let Some(pose) = self.skeleton.node_pose_mut(node(fit.knee)) {
                pose.rotate[2] = fit.knee_rotate_z;
                moved = true;
            }
        }
        moved
    }

    /// Every part's collision local (`unk_dobjtrans_0x10`) and
    /// `translate.x` as the skeleton poses them now, the lead `XRotN` as
    /// joint 2.
    fn part_locals(
        &self,
        pack: &Pack<'_>,
        object: &ObjectDesc,
    ) -> ssb_game::slope_contour::PartLocals {
        let mut kept = ssb_rom::skeleton::CollisionLocals::new();
        part_locals_into(&self.skeleton, pack, object, &mut kept)
    }

    /// `func_ovl0_800C9A38` on `joint_itemheavy_id`: the hand a held
    /// fighter hangs from, its axes without scale, in the original's
    /// arithmetic (`ftCommonCapturePulledRotateScale`, RE-511).
    fn hand_transform(&mut self, pack: &Pack<'_>) -> Option<ssb_game::fighter::JointTransform> {
        use ssb_engine::math::Vec3;
        let joint = ssb_game::grab::itemheavy_joint(self.fighter.kind)?;
        let object = pack.object(self.object)?;
        self.refresh_joint_parents(pack, &object);
        let parts = self.part_locals(pack, &object);
        if !parts.has(joint) {
            // No part locals: the sampled joint with its scale taken out.
            return self
                .fighter
                .joint_transforms
                .get(joint)
                .copied()
                .flatten()
                .map(|j| ssb_game::fighter::JointTransform {
                    axes: ssb_game::grab::held_root_axes(j),
                    origin: j.origin,
                });
        }
        let m = ssb_game::slope_contour::unscaled_world(
            &parts,
            ssb_game::slope_contour::topn_local(&self.fighter),
            &self.fighter.joint_parents,
            self.skeleton.locks.flagged,
            joint,
        );
        let v = |r: [f32; 3]| Vec3::new(r[0], r[1], r[2]);
        Some(ssb_game::fighter::JointTransform {
            axes: [v(m[0]), v(m[1]), v(m[2])],
            origin: v(m[3]),
        })
    }

    /// The part tree a query walks to TopN: node `n` is joint `n + 4`, a
    /// root node hangs from the lead `XRotN` when the clip has one.
    fn refresh_joint_parents(&mut self, pack: &Pack<'_>, object: &ObjectDesc) {
        set_joint_parents(pack, object, &self.skeleton, &mut self.fighter);
    }

    pub fn sample_held_child_offset(&mut self) {
        self.fighter.grab.held_child_offset = if ssb_game::grab::is_held(self.fighter.status.status)
        {
            self.skeleton.pose(0).map(|p| {
                ssb_engine::math::Vec3::new(p.translate[0], p.translate[1], p.translate[2])
            })
        } else {
            None
        };
    }

    /// Samples all posed model nodes once for gameplay collision. Joint IDs
    /// 4.. are the packed DObjDesc nodes; 0 is TopN. The other runtime joints
    /// have no packed node and are not used by the ported motion collisions.
    pub fn sample_gameplay_joints(&mut self, pack: &Pack<'_>) {
        self.sample_gameplay_joints_from(pack, None);
    }

    /// [`Fighter::joint4_rotate_x`](ssb_game::fighter::Fighter) from the
    /// skeleton as it stands.
    fn refresh_joint4_rotate_x(&mut self, pack: &Pack<'_>) {
        self.fighter.joint4_rotate_x = pack
            .object(self.object)
            .and_then(|o| self.skeleton.node_pose(o.first_node))
            .map(|p| p.rotate[0]);
    }

    /// [`Self::sample_gameplay_joints`], its matrices built from `kept`
    /// when given: the locals a [`Self::slope_contour`] composed on the
    /// skeleton as it still stands.
    fn sample_gameplay_joints_from(
        &mut self,
        pack: &Pack<'_>,
        kept: Option<&ssb_rom::skeleton::CollisionLocals>,
    ) {
        use ssb_engine::math::Vec3;
        use ssb_game::fighter::{JointTransform, FIGHTER_JOINTS};
        self.fighter.joint_transforms = [None; FIGHTER_JOINTS];
        // TopN carries `attr->size` (`ftManagerMakeFighter`), which every
        // joint below it inherits (RE-458).
        let size = self.fighter.attributes.size;
        let axes = ssb_game::item_throw::collision_axes(&self.fighter).map(|a| a * size);
        let world = |v: Vec3| axes[0] * v.x + axes[1] * v.y + axes[2] * v.z;
        let root = JointTransform {
            axes,
            origin: self.fighter.pos,
        };
        self.fighter.joint_transforms[0] = Some(root);
        // `fp->is_use_animlocks` picks which parts a query marks (RE-509).
        self.fighter.is_use_animlocks = self.skeleton.locks.flagged;
        self.refresh_joint4_rotate_x(pack);
        let Some(object) = pack.object(self.object) else {
            return;
        };
        self.refresh_joint_parents(pack, &object);
        let mut posed = [ssb_rom::scene::Mat4::IDENTITY; ssb_rom::skeleton::MAX_NODES];
        let count = match kept {
            Some(kept) => ssb_rom::skeleton::Skeleton::compose_collision_from(kept, &mut posed),
            None => self.compose_collision(pack, &object, &mut posed),
        };
        set_posed_joints(&posed[..count], axes, &mut self.fighter);
        // `ftCommonThrownReleaseFighterLoseGrip` drops a thrown fighter to
        // `joints[4]` (RE-511).
        self.fighter.grab.thrown_part4 = if ssb_game::grab::is_thrown(self.fighter.status.status) {
            let parts = self.part_locals(pack, &object);
            parts.has(4).then(|| {
                ssb_game::slope_contour::point_below_topn(
                    &parts,
                    &self.fighter.joint_parents,
                    4,
                    Vec3::ZERO,
                )
            })
        } else {
            None
        };
        // `YRotN` hangs off `XRotN`, which nothing rotates, so its local
        // pose is its model-space pose. Only a raised shield reads it.
        if ssb_game::status::shield_pose(&self.fighter).is_some() {
            let pose = &self.shield_yrotn;
            let scale = ssb_rom::pack::MODEL_SCALE;
            let local = |p: &ssb_rom::figatree::JointPose, s: [f32; 3]| {
                let t = [
                    p.translate[0] / scale,
                    p.translate[1] / scale,
                    p.translate[2] / scale,
                ];
                ssb_rom::scene::Mat4::from_trs_collision(t, p.rotate, s)
            };
            let mut m = local(pose, [1.0; 3]);
            // Under `XRotN` when the guard clip interposes it.
            if let Some(lead) = self.skeleton.lead_xrotn_pose() {
                m = local(lead, lead.scale).mul(&m);
            }
            let axis =
                |col: usize| world(Vec3::new(m.0[col * 4], m.0[col * 4 + 1], m.0[col * 4 + 2]));
            let t = m.translation();
            self.fighter.joint_transforms[3] = Some(JointTransform {
                axes: [axis(0), axis(1), axis(2)],
                origin: self.fighter.pos
                    + world(Vec3::new(t[0] * scale, t[1] * scale, t[2] * scale)),
            });
        } else if let Some(pose) =
            ssb_game::motion::uses_yrotn(self.fighter.kind, self.fighter.status.status)
                .then(|| self.skeleton.trailing_runtime_pose())
                .flatten()
        {
            // A motion with `is_use_yrotn_joint` (Yoshi's Egg Throw) drives
            // `YRotN` from its figatree's last stream. The held egg reads
            // its world position and copies its scale
            // (`ftYoshiSpecialHiUpdateEggVectors`).
            let scale = ssb_rom::pack::MODEL_SCALE;
            let local = |p: &ssb_rom::figatree::JointPose| {
                let t = [
                    p.translate[0] / scale,
                    p.translate[1] / scale,
                    p.translate[2] / scale,
                ];
                ssb_rom::scene::Mat4::from_trs_collision(t, p.rotate, p.scale)
            };
            let mut m = local(pose);
            if let Some(lead) = self.skeleton.lead_xrotn_pose() {
                m = local(lead).mul(&m);
            }
            let axis =
                |col: usize| world(Vec3::new(m.0[col * 4], m.0[col * 4 + 1], m.0[col * 4 + 2]));
            let t = m.translation();
            self.fighter.joint_transforms[3] = Some(JointTransform {
                axes: [axis(0), axis(1), axis(2)],
                origin: self.fighter.pos
                    + world(Vec3::new(t[0] * scale, t[1] * scale, t[2] * scale)),
            });
            self.fighter.yoshi.egg_joint_scale =
                Some(Vec3::new(pose.scale[0], pose.scale[1], pose.scale[2]));
        }
    }
}

#[cfg(test)]
mod tests;
