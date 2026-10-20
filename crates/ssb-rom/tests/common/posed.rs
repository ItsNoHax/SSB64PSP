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
    /// `clip_restarts` the skeleton last restarted for.
    pub clip_restarts_seen: u8,
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
            clip_restarts_seen: 0,
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
        if status.status != Status::CapturePulled && status.status != Status::CaptureYoshi
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

    /// `settle_catch`: a `proc_catch` that set its status in the hit phase
    /// (`ftYoshiSpecialNCatchProcCatch`) poses it before the caught
    /// fighter's `proc_capture` reads the joint.
    pub fn settle_catch(&mut self, pack: &ssb_rom::pack::Pack<'_>) {
        let status = self.fighter.status;
        if self.started == Some((status.status, status.entry)) || self.fighter.grab.catch.is_none()
        {
            return;
        }
        self.tick_animation(pack);
        self.sample_joints(pack);
        self.sample_anchor(false);
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
        let clip_restarted = self.clip_restarts_seen != status.clip_restarts;
        self.clip_restarts_seen = status.clip_restarts;
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
                self.skeleton.lead_transn =
                    ssb_game::motion::uses_transn(self.fighter.kind, status.status);
            }
        }
        // A wiggle's `lbCommonAddFighterPartsFigatree`, after the entry.
        let mut wiggle_root = None;
        if clip_restarted && !status.status.keeps_motion() {
            if let Some(anim) = pack.fighter_anim(kind, slot) {
                wiggle_root = Some(self.skeleton.pose(0).map(|p| ssb_rom::figatree::JointPose {
                    translate: if restarted { [0.0; 3] } else { p.translate },
                    ..*p
                }));
                self.skeleton.start(pack, &anim, 0.0, speed);
            }
        }
        self.skeleton.speed = speed;
        self.root_before = if let Some(root) = wiggle_root {
            root
        } else if restarted && setter_played {
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

/// One match-loop tick of a two-fighter Training replay, in the match
/// loop's process order (`common/posed.rs`, after `explain_movement`):
/// fighter 0 plays `pad0(tick)` (buttons, stick x, stick y), fighter 1
/// (the dummy) holds neutral. Returns each fighter's effects queued this
/// tick, read before `finish_frame` flushes them.
pub fn tick_pair(
    pack: &ssb_rom::pack::Pack<'_>,
    map: &[ssb_game::weapon::MapSurface],
    fighters: &mut [Posed],
    pad0: &dyn Fn(u64) -> (u16, i8, i8),
    tick: u64,
    jump_was_held: &mut bool,
) -> [Vec<ssb_game::fteffect::FighterEffect>; 2] {
    tick_pair_pads(
        pack,
        map,
        fighters,
        pad0,
        &|_| (0, 0, 0),
        tick,
        jump_was_held,
    )
}

/// [`tick_pair`] with fighter 1 on its own pad, `pad1(tick)` (buttons,
/// stick x, stick y; no jump edges), as a human player 2.
pub fn tick_pair_pads(
    pack: &ssb_rom::pack::Pack<'_>,
    map: &[ssb_game::weapon::MapSurface],
    fighters: &mut [Posed],
    pad0: &dyn Fn(u64) -> (u16, i8, i8),
    pad1: &dyn Fn(u64) -> (u16, i8, i8),
    tick: u64,
    jump_was_held: &mut bool,
) -> [Vec<ssb_game::fteffect::FighterEffect>; 2] {
    let map_fn = || map.iter().copied();
    let map_fn = &map_fn;
    // Priority 5: input, TransN's sample, the interrupt, jostle and the
    // grab exchange.
    for i in 0..2 {
        let (c, tapped, released) = if i == 0 {
            let (buttons, stick_x, stick_y) = pad0(tick);
            let held = buttons
                & (ssb_engine::input::N64Buttons::C_UP
                    | ssb_engine::input::N64Buttons::C_DOWN
                    | ssb_engine::input::N64Buttons::C_LEFT
                    | ssb_engine::input::N64Buttons::C_RIGHT)
                != 0;
            let edges = (held && !*jump_was_held, !held && *jump_was_held);
            *jump_was_held = held;
            (
                ssb_engine::input::ControllerState {
                    buttons: ssb_engine::input::N64Buttons(buttons),
                    stick_x,
                    stick_y,
                    connected: true,
                },
                edges.0,
                edges.1,
            )
        } else {
            let (buttons, stick_x, stick_y) = pad1(tick);
            (
                ssb_engine::input::ControllerState {
                    buttons: ssb_engine::input::N64Buttons(buttons),
                    stick_x,
                    stick_y,
                    connected: true,
                },
                false,
                false,
            )
        };
        let p = &mut fighters[i];
        p.fighter.set_input(c, tapped, released);
        p.sample_held_child_offset();
        p.set_root_motion(None);
        p.fighter.tick_interrupt(map_fn);
        p.after_interrupt(pack);
        if !fighters[i].fighter.is_in_hitlag() {
            let other = ssb_game::fighter::JostleBody::of(&fighters[1 - i].fighter);
            ssb_game::fighter::jostle(&mut fighters[i].fighter, &[(other, i == 0)]);
        }
        exchange(fighters, i);
    }
    // Priority 4: the clip, the physics and map, the joint samples.
    for i in 0..2 {
        fighters[i].physics(pack, map_fn);
        let _ = fighters[i].fighter.take_weapon_spawn();
        exchange(fighters, i);
    }
    // Priorities 1 and 0: the catch search, the hit search, then
    // `ftMainProcParams`.
    let queued;
    for i in 0..2 {
        let [a, b] = &mut fighters[..] else {
            unreachable!()
        };
        let (catcher, other) = if i == 0 { (a, b) } else { (b, a) };
        if ssb_game::grab::nearest_catch(
            &catcher.fighter,
            core::iter::once(&other.fighter),
            ssb_game::team::TeamRules::FREE_FOR_ALL,
        )
        .is_some()
        {
            ssb_game::grab::search_catch(
                &mut catcher.fighter,
                &mut other.fighter,
                ssb_game::team::TeamRules::FREE_FOR_ALL,
            );
        }
    }
    for p in fighters.iter_mut() {
        p.settle_catch(pack);
    }
    for i in 0..2 {
        exchange(fighters, i);
    }
    for p in fighters.iter_mut() {
        p.settle_capture(pack, map_fn);
    }
    {
        let [a, b] = &mut fighters[..] else {
            unreachable!()
        };
        let mut both = [&mut a.fighter, &mut b.fighter];
        ssb_game::combat::search_all(&mut both, ssb_game::team::TeamRules::FREE_FOR_ALL);
        queued = [0, 1].map(|i| both[i].effects.iter().copied().collect());
        ssb_game::combat::finish_frame(&mut both);
    }
    for p in fighters.iter_mut() {
        p.fighter.resolve_cliff_release(map_fn);
    }
    for i in 0..2 {
        exchange(fighters, i);
    }
    for p in fighters.iter_mut() {
        p.settle_status(pack);
    }
    queued
}
