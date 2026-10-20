//! One fighter posed as `psp_runtime::scene::FighterScene` runs it, for
//! the host replays that need the skeleton: the catch and hit searches read
//! joints the clip poses (`explain_movement`, `scene_inputs`).

#![allow(dead_code)]

use ssb_engine::math::Vec3;
use ssb_game::fighter::Fighter;
use ssb_game::status::Status;
use ssb_game::weapon::MapSurface;

/// One fighter as `psp_runtime::scene::FighterScene` runs it: the
/// skeleton the gameplay joints are sampled from, with its clip state.
pub struct Posed {
    pub fighter: Fighter,
    pub skeleton: ssb_rom::skeleton::Skeleton,
    pub object: u32,
    pub started: Option<(ssb_game::status::AnyStatus, u32)>,
    pub root_before: Option<ssb_rom::figatree::JointPose>,
    /// The clip was played after the interrupt for a throw's release.
    pub anim_ticked_early: bool,
    /// `YRotN`'s guard pose (`apply_shield_pose`).
    pub shield_yrotn: ssb_rom::figatree::JointPose,
}

impl Posed {
    /// `fighter` with its skeleton not yet started.
    pub fn new(pack: &ssb_rom::pack::Pack<'_>, fighter: Fighter) -> Posed {
        let object = ssb_rom::scene_deps::fighter_object(pack, fighter.kind as u32)
            .expect("fighter object in pack");
        Posed {
            fighter,
            skeleton: ssb_rom::skeleton::Skeleton::new(),
            object,
            started: None,
            root_before: None,
            anim_ticked_early: false,
            shield_yrotn: ssb_rom::figatree::JointPose::default(),
        }
    }

    /// `sample_held_child_offset`: TopN's first child, while held.
    pub fn sample_held_child_offset(&mut self) {
        self.fighter.grab.held_child_offset = if ssb_game::grab::is_held(self.fighter.status.status)
        {
            self.skeleton
                .pose(0)
                .map(|p| Vec3::new(p.translate[0], p.translate[1], p.translate[2]))
        } else {
            None
        };
    }

    /// The heavy-item joint a held fighter hangs from: while holding or
    /// catching, or (`always`) for a release the catcher has just queued.
    pub fn sample_anchor(&mut self, always: bool) {
        let f = &mut self.fighter;
        let hand = (always || f.grab.catch.is_some() || f.grab.is_catchstatus)
            .then(|| ssb_game::grab::itemheavy_joint(f.kind))
            .flatten()
            .and_then(|joint| f.joint_transforms.get(joint).copied().flatten());
        f.grab.anchor = hand.map(|j| j.origin);
        // `func_ovl0_800C9A38`: the hand without its scale (RE-511).
        f.grab.anchor_transform = hand.map(|j| ssb_game::fighter::JointTransform {
            axes: ssb_game::grab::held_root_axes(j),
            origin: j.origin,
        });
    }

    /// The runtime's priority-5 half after `tick_interrupt`: a throw's
    /// release reads the hand this frame's play posed.
    pub fn after_interrupt(&mut self, pack: &ssb_rom::pack::Pack<'_>) {
        if ssb_game::grab::release_pending(&self.fighter) && !self.fighter.is_in_hitlag() {
            self.tick_animation(pack);
            self.anim_ticked_early = true;
            self.sample_joints(pack);
            self.sample_anchor(true);
        }
    }

    /// `tick_fighter_physics`.
    pub fn physics<I, F>(&mut self, pack: &ssb_rom::pack::Pack<'_>, map: &F)
    where
        F: Fn() -> I,
        I: IntoIterator<Item = MapSurface>,
    {
        if !self.fighter.is_in_hitlag() {
            if !std::mem::take(&mut self.anim_ticked_early) {
                self.tick_animation(pack);
            }
            let rotate_z = self.fighter.root_motion.rotate_z;
            self.set_root_motion(Some(rotate_z));
        } else {
            self.root_before = self.skeleton.pose(0).copied();
        }
        self.fighter.tick_physics_map_before_accessory(map);
        if !self.fighter.is_in_hitlag()
            && self.started != Some((self.fighter.status.status, self.fighter.status.entry))
        {
            self.tick_animation(pack);
        }
        self.sample_held_child_offset();
        let status = self.fighter.status.status;
        if (ssb_game::map::is_cliff_hold(status) || ssb_game::map::is_cliff_phase2(status))
            && self.skeleton.joint_node(0).is_none()
        {
            if let Some(pose) = self.skeleton.pose(0) {
                let f = &mut self.fighter;
                f.transn = Vec3::new(pose.translate[0], pose.translate[1], pose.translate[2]);
                if ssb_game::map::is_cliff_hold(status) {
                    f.pos.x = f.cliff.corner.x + f.transn.z * f.facing.sign() * f.attributes.size;
                    f.pos.y = f.cliff.corner.y + f.transn.y * f.attributes.size;
                }
            }
        }
        ssb_game::grab::refresh_held_attachment(&mut self.fighter);
        self.apply_pose_writes(pack);
        self.sample_joints(pack);
        ssb_game::item_use::accessory(&mut self.fighter);
        self.sample_anchor(false);
    }

    /// `settle_capture`: a fighter caught in the hit phase is placed at the
    /// hand on the catch frame.
    pub fn settle_capture<I, F>(&mut self, pack: &ssb_rom::pack::Pack<'_>, map: &F)
    where
        F: Fn() -> I,
        I: IntoIterator<Item = MapSurface>,
    {
        let status = self.fighter.status;
        if status.status != Status::CapturePulled
            || self.started == Some((status.status, status.entry))
            || self.fighter.grab.holder.is_none()
        {
            return;
        }
        self.tick_animation(pack);
        self.sample_held_child_offset();
        ssb_game::grab::tick_held(&mut self.fighter, || ssb_game::map::floors(map()));
        self.sample_joints(pack);
    }

    /// `settle_status`: a status the hit phase set stands at its first frame.
    pub fn settle_status(&mut self, pack: &ssb_rom::pack::Pack<'_>) {
        let status = self.fighter.status;
        if self.started == Some((status.status, status.entry)) {
            return;
        }
        self.tick_animation(pack);
        self.apply_pose_writes(pack);
        self.sample_joints(pack);
    }

    /// `apply_pose_writes`: the DObj writes a status queued.
    pub fn apply_pose_writes(&mut self, pack: &ssb_rom::pack::Pack<'_>) {
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

    /// `tick_skeleton_animation`: a new status starts its clip on its
    /// current frame, then the clip advances one frame.
    pub fn tick_animation(&mut self, pack: &ssb_rom::pack::Pack<'_>) {
        let kind = self.fighter.kind as u32;
        let first_node = pack.object(self.object).map_or(0, |o| o.first_node);
        let status = self.fighter.status;
        let slot = status.status.anim_slot() as u32;
        let setter_played = status.clock > status.anim_frame_begin;
        let speed = ssb_game::status::clip_speed(&self.fighter);
        let mut restarted = false;
        if self.started != Some((status.status, status.entry)) {
            self.started = Some((status.status, status.entry));
            restarted = !status.status.keeps_motion();
            if status.status.keeps_motion() {
                self.skeleton.speed = ssb_game::status::clip_speed(&self.fighter);
            } else if let Some(anim) = pack.fighter_anim(kind, slot) {
                let frame = if setter_played {
                    status.clock - speed
                } else {
                    status.clock
                };
                self.skeleton.start(pack, &anim, frame, speed);
                self.skeleton.lead_xrotn =
                    ssb_game::motion::leads_with_xrotn(self.fighter.kind, status.status);
            }
        }
        self.skeleton.speed = speed;
        self.root_before = if restarted && setter_played {
            if let Some(anim) = pack.fighter_anim(kind, slot) {
                if let Some(script) = pack.anim_script(&anim) {
                    let _ = self.skeleton.tick_scaled(
                        script,
                        pack.fighter_translate_scales(kind),
                        first_node,
                    );
                }
            }
            self.skeleton.pose(0).copied()
        } else if restarted {
            self.skeleton.pose(0).map(|p| ssb_rom::figatree::JointPose {
                translate: [0.0; 3],
                ..*p
            })
        } else {
            self.skeleton.pose(0).copied()
        };
        if let Some(anim) = pack.fighter_anim(kind, slot) {
            if let Some(script) = pack.anim_script(&anim) {
                let _ = self.skeleton.tick_scaled(
                    script,
                    pack.fighter_translate_scales(kind),
                    first_node,
                );
            }
        }
        self.apply_shield_pose(pack);
    }

    /// `apply_shield_pose`: `ftCommonGuardUpdateJoints` tilts the shield
    /// and, in `Guard`, the whole fighter.
    pub fn apply_shield_pose(&mut self, pack: &ssb_rom::pack::Pack<'_>) {
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

    /// TransN's step since the last parse, for the status's physics.
    pub fn set_root_motion(&mut self, rotate_z: Option<f32>) {
        if let (Some(before), Some(current)) = (self.root_before, self.skeleton.pose(0)) {
            let t = current.translate;
            self.fighter.set_root_motion(ssb_game::physics::RootMotion {
                delta: Vec3::new(
                    t[0] - before.translate[0],
                    t[1] - before.translate[1],
                    t[2] - before.translate[2],
                ),
                rotate_z: rotate_z.unwrap_or(current.rotate[2]),
                translate: Vec3::new(t[0], t[1], t[2]),
            });
        }
    }

    /// `sample_gameplay_joints`: TopN, then every model joint from the
    /// collision matrices.
    pub fn sample_joints(&mut self, pack: &ssb_rom::pack::Pack<'_>) {
        use ssb_game::fighter::{JointTransform, FIGHTER_JOINTS};
        let f = &mut self.fighter;
        f.joint_transforms = [None; FIGHTER_JOINTS];
        let size = f.attributes.size;
        let axes = ssb_game::item_throw::collision_axes(f).map(|a| a * size);
        let world = |v: Vec3| axes[0] * v.x + axes[1] * v.y + axes[2] * v.z;
        f.joint_transforms[0] = Some(JointTransform {
            axes,
            origin: f.pos,
        });
        let Some(object) = pack.object(self.object) else {
            return;
        };
        let mut posed = [ssb_rom::scene::Mat4::IDENTITY; ssb_rom::skeleton::MAX_NODES];
        let count = self
            .skeleton
            .compose_collision(pack, &object, &mut posed)
            .min(FIGHTER_JOINTS - 4);
        let scale = ssb_rom::pack::MODEL_SCALE;
        for (i, m) in posed[..count].iter().enumerate() {
            let axis = |c: usize| world(Vec3::new(m.0[c * 4], m.0[c * 4 + 1], m.0[c * 4 + 2]));
            let t = m.translation();
            f.joint_transforms[i + 4] = Some(JointTransform {
                axes: [axis(0), axis(1), axis(2)],
                origin: f.pos + world(Vec3::new(t[0] * scale, t[1] * scale, t[2] * scale)),
            });
        }
        // `YRotN` hangs off `XRotN`; only a raised shield reads it.
        if ssb_game::status::shield_pose(&self.fighter).is_some() {
            let local = |p: &ssb_rom::figatree::JointPose, s: [f32; 3]| {
                let t = [
                    p.translate[0] / scale,
                    p.translate[1] / scale,
                    p.translate[2] / scale,
                ];
                ssb_rom::scene::Mat4::from_trs_collision(t, p.rotate, s)
            };
            let mut m = local(&self.shield_yrotn, [1.0; 3]);
            if let Some(lead) = self.skeleton.lead_xrotn_pose() {
                m = local(lead, lead.scale).mul(&m);
            }
            let axis = |c: usize| world(Vec3::new(m.0[c * 4], m.0[c * 4 + 1], m.0[c * 4 + 2]));
            let t = m.translation();
            self.fighter.joint_transforms[3] = Some(JointTransform {
                axes: [axis(0), axis(1), axis(2)],
                origin: self.fighter.pos
                    + world(Vec3::new(t[0] * scale, t[1] * scale, t[2] * scale)),
            });
        }
    }
}

/// `grab::exchange` from fighter `from` to the other, as the match loop's
/// `exchange_from` runs it.
pub fn exchange(fighters: &mut [Posed], from: usize) {
    let [a, b] = fighters else { unreachable!() };
    if from == 0 {
        ssb_game::grab::exchange(&mut a.fighter, &mut b.fighter);
    } else {
        ssb_game::grab::exchange(&mut b.fighter, &mut a.fighter);
    }
}
