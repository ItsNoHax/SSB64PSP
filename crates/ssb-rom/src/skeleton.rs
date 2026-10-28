//! Playing a packed animation onto an object's nodes, at runtime.
//!
//! [`figatree`](crate::figatree) turns one joint's script into one joint's
//! local transform. [`pack`](crate::pack) says which node each joint drives.
//! This is the join between them: tick every joint, then rebuild the object's
//! node matrices from the results.
//!
//! ## Why the matrices have to be rebuilt
//!
//! The pack stores each node's world matrix already composed, which is what
//! makes the static draw path one `sceGumLoadMatrix` per node and no matrix
//! maths at all. That is only correct while nothing moves. Once a joint's local
//! transform changes, every node beneath it is stale, so the chain has to be
//! recomposed from the locals — which is why `NodeDesc` carries the rest ones.
//!
//! The cost is bounded and small: a fighter is under 35 nodes, and a node is
//! one `from_trs` plus one 4x4 multiply.
//!
//! ## Scale
//!
//! `NodeDesc::world` has its translation pre-divided by [`MODEL_SCALE`] so it
//! matches the `i16` vertex positions the GE reads. Composing does the same,
//! per local, before multiplying — and that is equivalent rather than merely
//! close: a chain of `T * R * S` products is linear in the translations, so
//! scaling every local translation by `1/S` scales the composed one by exactly
//! `1/S`. [`Skeleton::compose`] reproducing the baked matrices from the rest
//! pose is a test of precisely that.

use crate::figatree::{Desynchronised, JointAnim, JointPose};
use crate::objanim::StageJoint;
use crate::pack::{AnimDesc, AnimJoint, NodeDesc, ObjectDesc, Pack, MODEL_SCALE};
use crate::scene::Mat4;

/// Joints one skeleton can hold.
///
/// The largest playable fighter's skeleton is 33 joints; 40 leaves room
/// without making the struct large enough to care about. A joint past this is
/// dropped rather than wrapping onto another.
pub const MAX_JOINTS: usize = 40;

/// Nodes one object can hold, for [`Skeleton::compose`]'s output.
///
/// The biggest scene graph in the archive is well under this.
pub const MAX_NODES: usize = 64;

/// One big-endian `Vec3f` in `FTAttributes.translate_scales`.
pub fn translation_scale(bytes: &[u8], joint: usize) -> [f32; 3] {
    let at = joint * 12;
    core::array::from_fn(|axis| {
        bytes
            .get(at + axis * 4..at + axis * 4 + 4)
            .and_then(|b| b.try_into().ok())
            .map(|b: [u8; 4]| f32::from_be_bytes(b))
            .unwrap_or(1.0)
    })
}

/// One object's animation state: a clock and a pose per joint.
///
/// Fixed-size and `Copy`-free but allocation-free, so it can live in a
/// fighter's struct on the PSP without a heap.
#[derive(Debug, Clone)]
pub struct Skeleton {
    /// Absolute pack node each joint drives.
    nodes: [u32; MAX_JOINTS],
    anims: [JointAnim; MAX_JOINTS],
    /// A 32-bit `AnimJoint` clip's per-joint streams (RE-402), used instead
    /// of `anims` while `anim_joint` is set: the battle-entry slots.
    joints32: [Option<StageJoint>; MAX_JOINTS],
    anim_joint: bool,
    /// The fighter `GObj::anim_frame`: seeded by `start`, then rewritten by
    /// every joint whose parse writes it, in joint order, so the last writer
    /// of a tick wins (`ftParamUpdateAnimKeys`). Both the 16-bit and the
    /// 32-bit parse write it.
    gobj_frame: f32,
    /// Current local transform per joint, seeded from the node's rest pose.
    poses: [JointPose; MAX_JOINTS],
    joint_count: usize,
    /// Playback rate: 1.0 normally, 0.5 for a heavy landing (RE-035).
    pub speed: f32,
    /// The clip's leading runtime joint is `XRotN` (`anim_desc` bit 31,
    /// `is_use_xrotn_joint`, which `ftdef.h` names `FTANIM_FLAG_TRANSN_JOINT`):
    /// `ftMainSetStatus` interposes it between TopN and the model, so its
    /// pose parents every root node. A `TransN` leading joint (bit 30) is
    /// detached instead and only moves the fighter (RE-468).
    pub lead_xrotn: bool,
    /// With [`Self::lead_xrotn`], a detached `TransN` (bit 30) leads
    /// before it: a motion with both bits (Link's entry, `ftdata.c:3354`)
    /// walks `TransN`, then `XRotN`, then the model, so `XRotN` is the
    /// clip's second joint.
    pub lead_transn: bool,
    /// A fighter's animation locks (RE-509). Left at their default, no
    /// node is locked and nothing changes.
    pub locks: AnimLocks,
}

/// A fighter's animation locks: `FTAttributes::animlock` and
/// `fp->is_use_animlocks` (RE-509).
///
/// The name reads backwards. A motion *without* `FTANIM_FLAG_ANIMLOCKS`
/// (every motion but Samus's four grabs and Yoshi's 71) freezes the mask's
/// parts: `ftParamSetAnimLocks` stores each one's
/// `gmCollisionTransformMatrixAll` matrix right after `ftMainSetStatus` has
/// reset every part to its `DObjDesc` transform, and sets
/// `transform_update_mode = 3`. Until a flagged motion clears them
/// (`ftParamClearAnimLocks`) the display draws those parts with
/// `syMatrixF2LFixedW` of that rest matrix and the collision walk uses it,
/// whatever the figatree does to them. A flagged motion animates every part
/// and builds each with the "no child scale" matrices instead:
/// `lbCommonMatrixTraRotScaInv` on screen and `gmCollisionSetMatrixNcs`
/// for collisions, both carrying the scale accumulated from TopN down
/// (`gLBCommonScale`, `FTParts::vec_scale`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AnimLocks {
    /// `FTAttributes::animlock`: bit `n` locks descriptor `n`, the object's
    /// node `n` (joint `n + 4`).
    pub mask: u64,
    /// The playing motion's `FTANIM_FLAG_ANIMLOCKS` (`is_use_animlocks`).
    pub flagged: bool,
    /// TopN's scale, `attr->size`: where the flagged chain's accumulated
    /// scale starts.
    pub root_scale: [f32; 3],
    /// Nodes a collision query read before this frame's display
    /// (`gmCollisionGetFighterPartsWorldPosition` and its kin set
    /// `transform_update_mode = 1` on the joint and every ancestor; the
    /// map process clears it next frame). The display then draws them from
    /// the collision local, `syMatrixF2LFixedW(&unk_dobjtrans_0x10)`.
    /// The set is exact, ancestors included: the gameplay side marks the
    /// chain and the resets clear single parts (the thigh and knee after a
    /// slope contour, TopN in the map process). The words differ from the
    /// display's own by up to about 100/65536 per entry: `lbCommonSin`'s
    /// matrix against `syMatrixTraRotRpyR`'s, or `gmCollisionSetMatrixNcs`'s
    /// against `lbCommonMatrixTraRotScaInv`'s (RE-509).
    pub queried: u64,
    /// The lead `XRotN`'s mode 1, when the clip interposes it: a query
    /// below it walks through it to TopN.
    pub queried_lead: bool,
}

impl Default for AnimLocks {
    fn default() -> Self {
        AnimLocks {
            mask: 0,
            flagged: false,
            root_scale: [1.0; 3],
            queried: 0,
            queried_lead: false,
        }
    }
}

/// The collision locals one compose built, and how each node's matrix
/// composes from them, kept so the matrices can follow without a second
/// compose ([`Skeleton::collision_parts_keep`],
/// [`Skeleton::compose_collision_from`]).
#[derive(Debug, Clone)]
pub struct CollisionLocals {
    count: usize,
    local: [Mat4; MAX_NODES],
    /// Node `i`'s parent index, or one of the `LINK_` cases.
    link: [u8; MAX_NODES],
    /// The interposed `XRotN`'s local, when the clip leads with it.
    lead: Option<Mat4>,
}

impl CollisionLocals {
    /// No such node in the pack: the identity.
    const LINK_NONE: u8 = 0xFF;
    /// A root node: under the lead `XRotN` when there is one.
    const LINK_ROOT: u8 = 0xFE;
    /// A parent outside the object: left in its own space.
    const LINK_OWN: u8 = 0xFD;

    pub fn new() -> Self {
        CollisionLocals {
            count: 0,
            local: [Mat4::IDENTITY; MAX_NODES],
            link: [Self::LINK_NONE; MAX_NODES],
            lead: None,
        }
    }
}

impl Default for CollisionLocals {
    fn default() -> Self {
        Self::new()
    }
}

/// Which of a part's two matrices [`Skeleton::compose_with`] builds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Space {
    /// `lbCommonFighterPartsFuncMatrix`'s fixed-point display matrix.
    Display,
    /// `gmCollisionTransformMatrixAll` / `gmCollisionSetMatrixNcs`.
    Collision,
}

impl Default for Skeleton {
    fn default() -> Self {
        Skeleton {
            nodes: [AnimJoint::NO_NODE; MAX_JOINTS],
            anims: core::array::from_fn(|_| JointAnim::inert()),
            joints32: [None; MAX_JOINTS],
            anim_joint: false,
            gobj_frame: 0.0,
            poses: [JointPose::default(); MAX_JOINTS],
            joint_count: 0,
            speed: 1.0,
            lead_xrotn: false,
            lead_transn: false,
            locks: AnimLocks::default(),
        }
    }
}

impl Skeleton {
    pub fn new() -> Self {
        Self::default()
    }

    /// Writes [`Skeleton::default`] into `dst` one joint at a time.
    ///
    /// The value is about 26 KB on the PSP; built by value it lands on the
    /// stack and is copied, because `panic = unwind` keeps LLVM from
    /// constructing it in a heap or arena slot (RE-469).
    ///
    /// # Safety
    ///
    /// `dst` must be valid for writes and aligned. Its old contents are
    /// overwritten without being dropped.
    #[inline(never)]
    pub unsafe fn write_new(dst: *mut Skeleton) {
        use core::ptr::addr_of_mut;
        ssb_engine::all_fields!(Skeleton {
            nodes,
            anims,
            joints32,
            anim_joint,
            gobj_frame,
            poses,
            joint_count,
            speed,
            lead_xrotn,
            lead_transn,
            locks
        });
        // SAFETY: every field of `*dst` is written once, through raw
        // pointers, before anything reads it.
        unsafe {
            let nodes = addr_of_mut!((*dst).nodes).cast::<u32>();
            let anims = addr_of_mut!((*dst).anims).cast::<JointAnim>();
            let joints32 = addr_of_mut!((*dst).joints32).cast::<Option<StageJoint>>();
            let poses = addr_of_mut!((*dst).poses).cast::<JointPose>();
            for i in 0..MAX_JOINTS {
                nodes.add(i).write(AnimJoint::NO_NODE);
                anims.add(i).write(JointAnim::inert());
                joints32.add(i).write(None);
                poses.add(i).write(JointPose::default());
            }
            addr_of_mut!((*dst).anim_joint).write(false);
            addr_of_mut!((*dst).gobj_frame).write(0.0);
            addr_of_mut!((*dst).joint_count).write(0);
            addr_of_mut!((*dst).speed).write(1.0);
            addr_of_mut!((*dst).lead_xrotn).write(false);
            addr_of_mut!((*dst).lead_transn).write(false);
            addr_of_mut!((*dst).locks).write(AnimLocks::default());
        }
    }

    /// Back to [`Skeleton::default`] in place, with no stack temporary.
    pub fn reset(&mut self) {
        // The old value is overwritten, not dropped: it owns nothing.
        const { assert!(!core::mem::needs_drop::<Skeleton>()) };
        // SAFETY: `self` is valid for writes and aligned.
        unsafe { Self::write_new(self) }
    }

    /// Writes a clone of `src` into `dst` one joint at a time: `clone()`
    /// builds the 26 KB value on the stack and copies it (RE-469).
    ///
    /// # Safety
    ///
    /// `dst` must be valid for writes, aligned and not overlap `src`. Its
    /// old contents are overwritten without being dropped.
    pub unsafe fn write_clone(dst: *mut Skeleton, src: &Skeleton) {
        use core::ptr::addr_of_mut;
        ssb_engine::all_fields!(Skeleton {
            nodes,
            anims,
            joints32,
            anim_joint,
            gobj_frame,
            poses,
            joint_count,
            speed,
            lead_xrotn,
            lead_transn,
            locks
        });
        // SAFETY: every field of `*dst` is written once, through raw
        // pointers, before anything reads it.
        unsafe {
            addr_of_mut!((*dst).nodes).write(src.nodes);
            let anims = addr_of_mut!((*dst).anims).cast::<JointAnim>();
            for (i, a) in src.anims.iter().enumerate() {
                anims.add(i).write(a.clone());
            }
            addr_of_mut!((*dst).joints32).write(src.joints32);
            addr_of_mut!((*dst).anim_joint).write(src.anim_joint);
            addr_of_mut!((*dst).gobj_frame).write(src.gobj_frame);
            addr_of_mut!((*dst).poses).write(src.poses);
            addr_of_mut!((*dst).joint_count).write(src.joint_count);
            addr_of_mut!((*dst).speed).write(src.speed);
            addr_of_mut!((*dst).lead_xrotn).write(src.lead_xrotn);
            addr_of_mut!((*dst).lead_transn).write(src.lead_transn);
            addr_of_mut!((*dst).locks).write(src.locks);
        }
    }

    /// `*self = src.clone()` in place, with no stack temporary.
    pub fn clone_from_in_place(&mut self, src: &Skeleton) {
        // The old value is overwritten, not dropped: it owns nothing.
        const { assert!(!core::mem::needs_drop::<Skeleton>()) };
        // SAFETY: `self` is valid for writes and aligned, and a `&mut`
        // cannot overlap `src`.
        unsafe { Self::write_clone(self, src) }
    }

    /// [`Skeleton::default`] in its own heap box, built there.
    pub fn new_boxed() -> alloc::boxed::Box<Self> {
        let mut slot = alloc::boxed::Box::<Self>::new_uninit();
        // SAFETY: the box is valid and aligned; `write_new` fills it.
        unsafe {
            Self::write_new(slot.as_mut_ptr());
            slot.assume_init()
        }
    }

    pub fn joint_count(&self) -> usize {
        self.joint_count
    }

    /// The fighter `GObj::anim_frame`: frames elapsed while the clip runs.
    ///
    /// Goes `<= 0` when the script ends, which is the sentinel the status
    /// machine tests (RE-035). After the end no joint writes it, so it keeps
    /// the end tick's `anim_wait` (`ftanim.c:120`), which is `0.0` only when
    /// the clip's length is a whole number of ticks.
    pub fn frame(&self) -> f32 {
        self.gobj_frame
    }

    /// Whether every joint's script has run out.
    pub fn ended(&self) -> bool {
        if self.anim_joint {
            return self.joint_count == 0
                || self.joints32[..self.joint_count]
                    .iter()
                    .all(|j| j.is_none_or(|j| j.ended()));
        }
        self.joint_count == 0 || self.anims[..self.joint_count].iter().all(|a| a.ended())
    }

    /// Starts `anim`, seeding each joint from the rest pose of the node it
    /// drives.
    ///
    /// Seeding from rest is not a nicety. A figatree names only the tracks it
    /// moves, and the original resets every joint to its `DObjDesc` transform
    /// whenever an animation is set:
    ///
    /// ```c
    /// for (i = nFTPartsJointCommonStart; dobjdesc->id != DOBJ_ARRAY_MAX; i++, dobjdesc++) {
    ///     joint->translate.vec.f = dobjdesc->translate;  // rotate, scale too
    /// }
    /// ```
    ///
    /// Carrying the previous animation's pose across instead would leave a
    /// joint wherever the last one left it on any track the new one is silent
    /// about.
    ///
    /// An entry slot's clip ([`crate::anim::is_anim_joint_slot`], RE-401)
    /// plays its 32-bit streams instead: `gcAddDObjAnimJoint`, whose first
    /// parse comes with the next tick.
    pub fn start(&mut self, pack: &Pack<'_>, anim: &AnimDesc, frame: f32, speed: f32) {
        self.joint_count = 0;
        self.speed = speed;
        self.anim_joint = anim.fighter < 27 && crate::anim::is_anim_joint_slot(anim.slot as usize);
        self.gobj_frame = frame;
        let count = (anim.joint_count as usize).min(MAX_JOINTS);
        for i in 0..count {
            let Some(joint) = pack.anim_joint(anim.first_joint + i as u32) else {
                break;
            };
            self.nodes[i] = joint.node;
            self.poses[i] = match pack.node(joint.node) {
                Some(n) => JointPose {
                    rotate: n.rest_rotate,
                    translate: n.rest_translate,
                    scale: n.rest_scale,
                },
                None => JointPose::default(),
            };
            self.anims[i] = match joint.script {
                AnimJoint::NO_SCRIPT => JointAnim::inert(),
                _ if self.anim_joint => JointAnim::inert(),
                at => JointAnim::start(at as usize, frame),
            };
            self.joints32[i] = match joint.script {
                AnimJoint::NO_SCRIPT => None,
                at if self.anim_joint => Some(StageJoint::start_changed(at, frame)),
                _ => None,
            };
            self.joint_count = i + 1;
        }
    }

    /// Advances every joint one tick.
    ///
    /// `script` is the animation's bytes, [`Pack::anim_script`]. A joint whose
    /// stream desynchronises stops rather than taking the others down with it,
    /// and the error is returned once every joint has been given its tick —
    /// a half-posed skeleton is worse than a fully posed one with a bad joint.
    pub fn tick(&mut self, script: &[u8]) -> Result<(), Desynchronised> {
        self.tick_scaled(script, None, 0)
    }

    /// Play fighter translation tracks with the source joint's scale vector.
    /// Model joint zero is FTParts joint 4; a leading runtime joint has no
    /// packed node and uses the identity vector.
    pub fn tick_scaled(
        &mut self,
        script: &[u8],
        scales: Option<&[u8]>,
        first_node: u32,
    ) -> Result<(), Desynchronised> {
        let mut failed = None;
        for i in 0..self.joint_count {
            let scale = scales.map_or([1.0; 3], |bytes| {
                if self.nodes[i] == AnimJoint::NO_NODE || self.nodes[i] < first_node {
                    [1.0; 3]
                } else {
                    translation_scale(bytes, (self.nodes[i] - first_node) as usize + 4)
                }
            });
            if self.anim_joint {
                let Some(joint) = self.joints32[i].as_mut() else {
                    continue;
                };
                if joint
                    .tick_scaled(script, self.speed, &mut self.poses[i], scale)
                    .is_err()
                {
                    self.joints32[i] = None;
                    continue;
                }
                // Each joint that writes it sets the fighter's `anim_frame`,
                // which `ftCommonAppearProcUpdate` reads; the last one wins.
                if let Some(f) = joint.gobj_frame() {
                    self.gobj_frame = f;
                }
                continue;
            }
            if let Err(e) = self.anims[i].tick_scaled(script, self.speed, &mut self.poses[i], scale)
            {
                self.anims[i] = JointAnim::inert();
                failed.get_or_insert(e);
                continue;
            }
            if let Some(f) = self.anims[i].gobj_frame() {
                self.gobj_frame = f;
            }
        }
        match failed {
            Some(e) => Err(e),
            None => Ok(()),
        }
    }

    /// `ftParamUpdateAnimKeys`'s `motion_id == -2` branch: a status with no
    /// motion of its own (`CatchWait`, `CaptureWait`, ...) parses nothing,
    /// but every joint the installed clip animates plays once more with
    /// `anim_wait` forced to `AOBJ_ANIM_END`, rewriting its held values
    /// without advancing the clock (RE-510). Same arguments as
    /// [`Self::tick_scaled`].
    pub fn hold_scaled(&mut self, script: &[u8], scales: Option<&[u8]>, first_node: u32) {
        for i in 0..self.joint_count {
            let scale = scales.map_or([1.0; 3], |bytes| {
                if self.nodes[i] == AnimJoint::NO_NODE || self.nodes[i] < first_node {
                    [1.0; 3]
                } else {
                    translation_scale(bytes, (self.nodes[i] - first_node) as usize + 4)
                }
            });
            if self.anim_joint {
                if let Some(joint) = self.joints32[i].as_mut() {
                    joint.hold_scaled(script, &mut self.poses[i], scale);
                }
                continue;
            }
            self.anims[i].hold_scaled(script, &mut self.poses[i], scale);
        }
    }

    /// The local transform a joint currently holds.
    pub fn pose(&self, joint: usize) -> Option<&JointPose> {
        (joint < self.joint_count).then(|| &self.poses[joint])
    }

    /// [`Self::pose`], to adjust in place: the next tick writes only the
    /// tracks the clip animates (TransN's roll a status writes,
    /// `ftCaptainSpecialLwAirSetStatus`).
    pub fn pose_mut(&mut self, joint: usize) -> Option<&mut JointPose> {
        (joint < self.joint_count).then(|| &mut self.poses[joint])
    }

    /// The interposed `XRotN`'s pose, when the clip leads with it
    /// ([`Skeleton::lead_xrotn`]).
    pub fn lead_xrotn_pose(&self) -> Option<&JointPose> {
        let at = usize::from(self.lead_transn);
        (self.lead_xrotn && self.joint_count > at && self.nodes[at] == AnimJoint::NO_NODE)
            .then(|| &self.poses[at])
    }

    /// The pose of the clip's last stream when it drives no packed node:
    /// the runtime joint a motion's `is_use_yrotn_joint` adds as a hidden
    /// part (`ftMainAddHiddenPartID`), which the figatree walk reaches
    /// after the model. Yoshi's Egg Throw animates `YRotN` this way; its
    /// scale steps match the N64 egg's `DObj` scale frame for frame.
    pub fn trailing_runtime_pose(&self) -> Option<&JointPose> {
        let last = self.joint_count.checked_sub(1).filter(|&i| i > 0)?;
        (self.nodes[last] == AnimJoint::NO_NODE).then(|| &self.poses[last])
    }

    /// A joint's `DObj::flags`, as its clip's `SetFlags` commands last
    /// wrote them (`gcParseDObjAnimJoint`, `ftAnimParseDObjFigatree`).
    /// `ftMainSetStatus` clears every model joint's and the interposed
    /// runtime joints' flags when it installs a clip, as [`Self::start`]
    /// starts each stream at 0.
    fn joint_flags(&self, joint: usize) -> u16 {
        if self.anim_joint {
            self.joints32[joint].as_ref().map_or(0, |j| j.flags)
        } else {
            self.anims[joint].flags
        }
    }

    /// Whether any joint's clip has set `DObj::flags`: the only case in
    /// which [`Self::hidden`] can hide anything.
    pub fn any_flags(&self) -> bool {
        (0..self.joint_count).any(|i| self.joint_flags(i) != 0)
    }

    /// Whether `ftDisplayMainDrawDefault` skips `node`'s display list:
    /// its own `DOBJ_FLAG_NOTEXTURE` (bit 0), or `DOBJ_FLAG_HIDDEN` (bit 1)
    /// on it or an ancestor, which skips the whole subtree. The leading
    /// runtime joints that bind no model node (`TransN`, an interposed
    /// `XRotN`) are ancestors of every model node. Yoshi's entry clip hides
    /// his model root until he hatches (RE-401).
    pub fn hidden(&self, pack: &Pack<'_>, node: u32) -> bool {
        let flags = |n: u32| {
            (0..self.joint_count)
                .find(|&i| self.nodes[i] == n)
                .map_or(0, |i| self.joint_flags(i))
        };
        if flags(node) & 1 != 0 {
            return true;
        }
        if (0..self.joint_count)
            .any(|i| self.nodes[i] == AnimJoint::NO_NODE && self.joint_flags(i) & 2 != 0)
        {
            return true;
        }
        let mut at = Some(node);
        for _ in 0..MAX_NODES {
            let Some(n) = at else { break };
            if flags(n) & 2 != 0 {
                return true;
            }
            at = pack.node_parent(n).filter(|&p| p != u32::MAX);
        }
        false
    }

    /// The node a joint drives.
    pub fn joint_node(&self, joint: usize) -> Option<u32> {
        (joint < self.joint_count)
            .then(|| self.nodes[joint])
            .filter(|&n| n != AnimJoint::NO_NODE)
    }

    /// The local pose of a model node. Figatrees can omit runtime joints,
    /// so their animation indices are not `FTStruct::joints` indices.
    pub fn node_pose(&self, node: u32) -> Option<&JointPose> {
        self.nodes[..self.joint_count]
            .iter()
            .position(|&n| n == node)
            .map(|i| &self.poses[i])
    }

    /// Rebuilds `out[0..object.node_count]` from the current poses.
    ///
    /// Nodes an animation does not drive keep their rest transform, so this is
    /// correct for a partly animated object and for a completely un-animated
    /// one — [`Skeleton::default`] drives nothing and reproduces the pack's own
    /// baked matrices.
    ///
    /// Returns how many matrices were written.
    pub fn compose(&self, pack: &Pack<'_>, object: &ObjectDesc, out: &mut [Mat4]) -> usize {
        self.compose_with::<true>(pack, object, out, Space::Display, None, None)
    }

    /// [`Skeleton::compose`] with this frame's queried parts in place of
    /// [`AnimLocks::queried`] and [`AnimLocks::queried_lead`]: a fighter's
    /// gameplay records them as it queries (RE-509).
    pub fn compose_queried(
        &self,
        pack: &Pack<'_>,
        object: &ObjectDesc,
        out: &mut [Mat4],
        queried: u64,
        queried_lead: bool,
    ) -> usize {
        self.compose_with::<true>(
            pack,
            object,
            out,
            Space::Display,
            Some((queried, queried_lead)),
            None,
        )
    }

    /// [`Skeleton::compose`] as `gmCollisionGetFighterPartsWorldPosition`
    /// builds it: each joint's matrix from the `lbCommonSin` table
    /// (`gmCollisionTransformMatrixAll`), not the display's. Gameplay
    /// positions (hit and hurt collisions, attachments) read these; the
    /// display keeps [`Skeleton::compose`] (RE-468).
    pub fn compose_collision(
        &self,
        pack: &Pack<'_>,
        object: &ObjectDesc,
        out: &mut [Mat4],
    ) -> usize {
        self.compose_with::<true>(pack, object, out, Space::Collision, None, None)
    }

    /// Every node's collision local (`FTParts::unk_dobjtrans_0x10`) in the
    /// original's units, rows 0 to 2 the axes and row 3 the translation,
    /// and its `DObj::translate.x`; plus the interposed `XRotN`'s, when
    /// the clip leads with it. The slope contour walks these (RE-510).
    /// Returns how many nodes were written.
    pub fn collision_parts(
        &self,
        pack: &Pack<'_>,
        object: &ObjectDesc,
        locals: &mut [[[f32; 3]; 4]],
        translate_x: &mut [f32],
        lead: &mut Option<([[f32; 3]; 4], f32)>,
    ) -> usize {
        let mut composed = CollisionLocals::new();
        self.collision_parts_keep(pack, object, &mut composed, locals, translate_x, lead)
    }

    /// [`Skeleton::collision_parts`] that keeps the composed locals in
    /// `composed`, so [`Skeleton::compose_collision_from`] can build the
    /// matrices [`Skeleton::compose_collision`] would from them while the
    /// poses stay as they are. Only the locals are composed here; the world
    /// products are left to that call.
    pub fn collision_parts_keep(
        &self,
        pack: &Pack<'_>,
        object: &ObjectDesc,
        composed: &mut CollisionLocals,
        locals: &mut [[[f32; 3]; 4]],
        translate_x: &mut [f32],
        lead: &mut Option<([[f32; 3]; 4], f32)>,
    ) -> usize {
        let count = self.compose_with::<false>(
            pack,
            object,
            &mut [],
            Space::Collision,
            None,
            Some(composed),
        );
        let rows = |m: &Mat4| -> [[f32; 3]; 4] {
            let r = |i: usize| [m.0[i * 4], m.0[i * 4 + 1], m.0[i * 4 + 2]];
            let t = r(3);
            // The pack's matrices are normalised by a power of two, so the
            // product is exact.
            [r(0), r(1), r(2), t.map(|v| v * MODEL_SCALE)]
        };
        let joint_of = self.joint_table(object.first_node);
        let count = count.min(locals.len()).min(translate_x.len());
        for i in 0..count {
            locals[i] = rows(&composed.local[i]);
            let global = object.first_node + i as u32;
            translate_x[i] = match self.pose_at(&joint_of, i) {
                Some(p) => p.translate[0],
                None => pack.node_rest(global).unwrap_or_default().translate[0],
            };
        }
        *lead = composed.lead.map(|m| {
            (
                rows(&m),
                self.lead_xrotn_pose().map_or(0.0, |p| p.translate[0]),
            )
        });
        count
    }

    /// The matrices [`Skeleton::compose_collision`] builds, from the locals
    /// a [`Skeleton::collision_parts_keep`] call kept: the same products in
    /// the same order, so the same bits, as long as no pose changed between
    /// the two calls.
    pub fn compose_collision_from(composed: &CollisionLocals, out: &mut [Mat4]) -> usize {
        let count = composed.count.min(out.len());
        for i in 0..count {
            let local = composed.local[i];
            out[i] = match composed.link[i] {
                CollisionLocals::LINK_NONE => Mat4::IDENTITY,
                CollisionLocals::LINK_ROOT => composed.lead.map_or(local, |lead| lead.mul(&local)),
                CollisionLocals::LINK_OWN => local,
                p => out[usize::from(p)].mul(&local),
            };
        }
        count
    }

    /// The pose driving `node`, for code that adjusts it in place while the
    /// animation keeps playing: the next tick (or a `motion_id` -2 hold,
    /// [`Self::hold_scaled`]) writes the tracks it animates and leaves the
    /// rest (`func_ovl2_800EBD08`'s leg fit).
    pub fn node_pose_mut(&mut self, node: u32) -> Option<&mut JointPose> {
        let i = (0..self.joint_count).find(|&i| self.nodes[i] == node)?;
        Some(&mut self.poses[i])
    }

    /// A part's local matrix, and the scale its children accumulate from
    /// (`vec_scale`, read only by a flagged motion's chain).
    fn local(&self, pose: &JointPose, parent_scale: [f32; 3], space: Space) -> (Mat4, [f32; 3]) {
        // Into the same normalised space the packed matrices use.
        let t = [
            pose.translate[0] / MODEL_SCALE,
            pose.translate[1] / MODEL_SCALE,
            pose.translate[2] / MODEL_SCALE,
        ];
        let (r, s) = (pose.rotate, pose.scale);
        match (self.locks.flagged, space) {
            (false, Space::Display) => (Mat4::from_trs_display(t, r, s), s),
            (false, Space::Collision) => (Mat4::from_trs_collision(t, r, s), s),
            // `parts->vec_scale = dobj->scale * gLBCommonScale`.
            (true, Space::Display) => {
                let acc = [
                    s[0] * parent_scale[0],
                    s[1] * parent_scale[1],
                    s[2] * parent_scale[2],
                ];
                (
                    Mat4::from_trs_scale_inv_display(t, r, parent_scale, acc),
                    acc,
                )
            }
            (true, Space::Collision) => Mat4::from_trs_ncs(t, r, s, Some(parent_scale)),
        }
    }

    /// Which joint drives each of `object`'s nodes, indexed from
    /// `first_node`: [`Self::pose_for`]'s answer for every node at once,
    /// instead of one scan of the joints per node.
    fn joint_table(&self, first_node: u32) -> [u8; MAX_NODES] {
        let mut table = [u8::MAX; MAX_NODES];
        // Backwards, so the first joint naming a node wins, as `pose_for`'s
        // `find` has it.
        for j in (0..self.joint_count).rev() {
            let k = self.nodes[j].wrapping_sub(first_node) as usize;
            if k < MAX_NODES {
                table[k] = j as u8;
            }
        }
        table
    }

    /// [`Self::pose_for`] of node `first_node + i`, through a
    /// [`Self::joint_table`] built for `first_node`.
    #[inline(always)]
    fn pose_at(&self, table: &[u8; MAX_NODES], i: usize) -> Option<&JointPose> {
        match table.get(i) {
            Some(&j) if j != u8::MAX => Some(&self.poses[usize::from(j)]),
            _ => None,
        }
    }

    /// Composes `object`'s nodes. With `WORLD`, writes each node's matrix to
    /// `out`; without, `out` is not touched and only `locals_out` is filled
    /// (its locals do not depend on the world matrices).
    fn compose_with<const WORLD: bool>(
        &self,
        pack: &Pack<'_>,
        object: &ObjectDesc,
        out: &mut [Mat4],
        space: Space,
        queried: Option<(u64, bool)>,
        mut locals_out: Option<&mut CollisionLocals>,
    ) -> usize {
        let count = if WORLD {
            (object.node_count as usize).min(out.len()).min(MAX_NODES)
        } else {
            (object.node_count as usize).min(MAX_NODES)
        };
        let joint_of = self.joint_table(object.first_node);
        let mut locks = self.locks;
        if let Some((nodes, lead)) = queried {
            locks.queried = nodes;
            locks.queried_lead = lead;
        }
        // `vec_scale` per node: the flagged chain's accumulated scale.
        let mut scales = [[1.0f32; 3]; MAX_NODES];
        // `XRotN`, when the clip interposes it, under every root node; TopN
        // (`attr->size`) above both.
        let queried = |i: usize| i < 64 && locks.queried >> i & 1 != 0;
        let (lead, root_scale) = match self.lead_xrotn_pose() {
            Some(p) => {
                let (m, s) = match space {
                    Space::Display if locks.queried_lead => {
                        let (m, s) = self.local(p, locks.root_scale, Space::Collision);
                        (m.f2l_fixed_w(), s)
                    }
                    _ => self.local(p, locks.root_scale, space),
                };
                (Some(m), s)
            }
            None => (None, locks.root_scale),
        };
        if let Some(kept) = locals_out.as_deref_mut() {
            kept.lead = lead;
            kept.count = count;
        }
        for i in 0..count {
            let global = object.first_node + i as u32;
            let Some(parent) = pack.node_parent(global) else {
                if WORLD {
                    out[i] = Mat4::IDENTITY;
                }
                if let Some(kept) = locals_out.as_deref_mut() {
                    kept.local[i] = Mat4::IDENTITY;
                    kept.link[i] = CollisionLocals::LINK_NONE;
                }
                continue;
            };
            let parent_index = match parent {
                p if p != NodeDesc::NO_PARENT
                    && p >= object.first_node
                    && (p - object.first_node) < i as u32 =>
                {
                    Some((p - object.first_node) as usize)
                }
                _ => None,
            };
            let parent_scale = parent_index.map_or(root_scale, |p| scales[p]);
            let local = if !locks.flagged && i < 64 && locks.mask >> i & 1 != 0 {
                // Frozen at the rest pose `ftParamSetAnimLocks` stored, in
                // `gmCollisionTransformMatrixAll`'s floats: the collision
                // walk reads them as they are, the display through
                // `syMatrixF2LFixedW`.
                let rest = pack.node_rest(global).unwrap_or_default();
                let (m, _) = self.local(&rest, parent_scale, Space::Collision);
                scales[i] = rest.scale;
                match space {
                    Space::Display => m.f2l_fixed_w(),
                    Space::Collision => m,
                }
            } else {
                // The rest pose is read only for a node no joint drives.
                let rest;
                let pose = match self.pose_at(&joint_of, i) {
                    Some(pose) => pose,
                    None => {
                        rest = pack.node_rest(global).unwrap_or_default();
                        &rest
                    }
                };
                let (m, s) = match space {
                    Space::Display if queried(i) => {
                        let (m, s) = self.local(pose, parent_scale, Space::Collision);
                        (m.f2l_fixed_w(), s)
                    }
                    _ => self.local(pose, parent_scale, space),
                };
                scales[i] = s;
                m
            };
            if let Some(kept) = locals_out.as_deref_mut() {
                kept.local[i] = local;
                kept.link[i] = match (parent, parent_index) {
                    (_, Some(p)) => p as u8,
                    (NodeDesc::NO_PARENT, None) => CollisionLocals::LINK_ROOT,
                    _ => CollisionLocals::LINK_OWN,
                };
            }
            if WORLD {
                // Parents always precede children in a `DObjDesc` array — a
                // child references `array_dobjs[depth - 1]`, which an earlier
                // entry must have filled — so the parent's matrix is already
                // final.
                out[i] = match (parent, parent_index) {
                    (_, Some(p)) => out[p].mul(&local),
                    (NodeDesc::NO_PARENT, None) => lead.map_or(local, |lead| lead.mul(&local)),
                    // A parent outside this object, or one that has not been
                    // composed yet, cannot be applied. Leaving the node in its
                    // own space is wrong but bounded; silently using a stale
                    // matrix would not be.
                    _ => local,
                };
            }
        }
        count
    }

    /// The pose driving `node`, if any joint does: what a
    /// [`Self::joint_table`] answers for every node at once.
    #[cfg(test)]
    fn pose_for(&self, node: u32) -> Option<&JointPose> {
        (0..self.joint_count)
            .find(|&i| self.nodes[i] == node)
            .map(|i| &self.poses[i])
    }

    /// Stops the joint driving `node` and hands back its pose, for code that
    /// sets the pose itself. `anim_wait = AOBJ_ANIM_NULL` in the original.
    pub fn take_node(&mut self, node: u32) -> Option<&mut JointPose> {
        let i = (0..self.joint_count).find(|&i| self.nodes[i] == node)?;
        self.anims[i] = JointAnim::inert();
        self.joints32[i] = None;
        Some(&mut self.poses[i])
    }
}

/// Which joints a shield pose writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShieldJoints {
    /// `ftCommonGuardUpdateJoints`: `YRotN` only.
    ShieldJoint,
    /// `ftCommonGuardInitJoints`: every joint the sector table names.
    All,
}

/// The neutral shield pose of shield-table entry `joint`: the translate and
/// rotate of `dobj_lookup[joint]`, read from the script bytes at
/// [`AnimDesc::frames`] (RE-367).
pub fn shield_lookup(script: &[u8], anim: &AnimDesc, joint: u32) -> Option<([f32; 3], [f32; 3])> {
    let at = anim.frames as usize + joint as usize * crate::fighter::DOBJDESC_SIZE as usize;
    let f = |k: usize| -> Option<f32> {
        let b = script.get(at + 8 + 4 * k..at + 12 + 4 * k)?;
        Some(f32::from_bits(u32::from_be_bytes(b.try_into().ok()?)))
    };
    Some(([f(0)?, f(1)?, f(2)?], [f(3)?, f(4)?, f(5)?]))
}

/// Poses a fighter's shield from one stick-sector table (RE-367).
///
/// Each named joint restarts its sector script at `frame` (`angle_f`) and
/// plays one tick on top of its current pose, as `gcAddDObjAnimJoint` and
/// `ftMainPlayAnim` do. The scripts are `AObjEvent32` streams, not figatree. `ftCommonGuardGetJointTransform` then pulls its
/// rotate and translate toward the `dobj_lookup` pose by `range`
/// (`shield_rotate_range`): the stick at rest gives the neutral shield pose.
/// A joint the table leaves null keeps its pose.
///
/// Entry 0 is `XRotN`, which no packed node represents. Only Link's tables
/// name it, and only to hold it at rest. The last entry is `YRotN`, the shield's joint, kept in
/// `yrotn`. The fighter joints between stop their own clip, so the next
/// frame starts from this pose.
///
/// Luigi's `translate_scales` variant (`ftCommonGuardGetJointTransformScale`)
/// scales both the scripted and neutral translations.
pub fn apply_shield_pose(
    pack: &Pack<'_>,
    anim: &AnimDesc,
    frame: f32,
    range: f32,
    joints: ShieldJoints,
    skeleton: &mut Skeleton,
    yrotn: &mut JointPose,
) {
    let Some(script) = pack.anim_script(anim) else {
        return;
    };
    let scales = pack.fighter_translate_scales(anim.slot / AnimDesc::SHIELD_SECTORS);
    let last = anim.joint_count.saturating_sub(1);
    let first = match joints {
        ShieldJoints::ShieldJoint => last,
        ShieldJoints::All => 1,
    };
    for j in first..anim.joint_count {
        let Some(joint) = pack.anim_joint(anim.first_joint + j) else {
            continue;
        };
        if joint.script == AnimJoint::NO_SCRIPT {
            continue;
        }
        let Some((translate, rotate)) = shield_lookup(script, anim, j) else {
            continue;
        };
        let pose = if j == last {
            &mut *yrotn
        } else {
            match skeleton.take_node(joint.node) {
                Some(pose) => pose,
                None => continue,
            }
        };
        // `InitJoints` sets `is_anim_joint`, so `ftParamUpdateAnimKeys` parses
        // these with `gcParseDObjAnimJoint`: the 32-bit event stream.
        let mut player = crate::objanim::StageJoint::start_changed(joint.script, frame);
        let scale_joint = if j == last { 3 } else { (j + 3) as usize };
        let scale = scales.map_or([1.0; 3], |bytes| translation_scale(bytes, scale_joint));
        if player.tick_scaled(script, 1.0, pose, scale).is_err() {
            continue;
        }
        for k in 0..3 {
            pose.rotate[k] = (pose.rotate[k] - rotate[k]) * range + rotate[k];
            let neutral = translate[k] * scale[k];
            pose.translate[k] = (pose.translate[k] - neutral) * range + neutral;
        }
    }
}

/// The most animated nodes a stage layer set is allowed. The busiest in the
/// archive uses far fewer; the cap bounds the fixed array rather than the data.
pub const MAX_STAGE_JOINTS: usize = 64;

/// A stage's scenery animation, playing the 32-bit event stream (RE-050).
///
/// The fighter [`Skeleton`] above and this are deliberately the same shape —
/// per-joint clocks, poses seeded from the rest transform, one `compose` that
/// walks the parent chain — because they *are* the same machine with different
/// instruction encodings. Only the tick differs.
pub struct StageAnimator {
    nodes: [u32; MAX_STAGE_JOINTS],
    joints: [crate::objanim::StageJoint; MAX_STAGE_JOINTS],
    poses: [JointPose; MAX_STAGE_JOINTS],
    count: usize,
    /// `GObj::anim_frame`: the start frame, then whatever the last joint in
    /// tree order to write it wrote (`gcParseDObjAnimJoint`).
    gobj_frame: f32,
}

impl Default for StageAnimator {
    fn default() -> Self {
        StageAnimator::new()
    }
}

impl StageAnimator {
    pub fn new() -> Self {
        StageAnimator {
            nodes: [0; MAX_STAGE_JOINTS],
            joints: [crate::objanim::StageJoint::start(0, 0.0); MAX_STAGE_JOINTS],
            poses: [JointPose {
                rotate: [0.0; 3],
                translate: [0.0; 3],
                scale: [1.0; 3],
            }; MAX_STAGE_JOINTS],
            count: 0,
            gobj_frame: 0.0,
        }
    }

    /// Writes [`StageAnimator::new`] into `dst` one joint at a time, so the
    /// 23 KB value is never built on the stack and copied (RE-469).
    ///
    /// # Safety
    ///
    /// `dst` must be valid for writes and aligned. Its old contents are
    /// overwritten without being dropped.
    #[inline(never)]
    pub unsafe fn write_new(dst: *mut StageAnimator) {
        use core::ptr::addr_of_mut;
        ssb_engine::all_fields!(StageAnimator {
            nodes,
            joints,
            poses,
            count,
            gobj_frame
        });
        // SAFETY: every field of `*dst` is written once, through raw
        // pointers, before anything reads it.
        unsafe {
            let nodes = addr_of_mut!((*dst).nodes).cast::<u32>();
            let joints = addr_of_mut!((*dst).joints).cast::<crate::objanim::StageJoint>();
            let poses = addr_of_mut!((*dst).poses).cast::<JointPose>();
            for i in 0..MAX_STAGE_JOINTS {
                nodes.add(i).write(0);
                joints
                    .add(i)
                    .write(crate::objanim::StageJoint::start(0, 0.0));
                poses.add(i).write(JointPose {
                    rotate: [0.0; 3],
                    translate: [0.0; 3],
                    scale: [1.0; 3],
                });
            }
            addr_of_mut!((*dst).count).write(0);
            addr_of_mut!((*dst).gobj_frame).write(0.0);
        }
    }

    /// `GObj::anim_frame` after the last tick (`lbTransitionProcUpdate`
    /// ejects its wipe once it is at or below 0).
    pub fn gobj_frame(&self) -> f32 {
        self.gobj_frame
    }

    pub fn joint_count(&self) -> usize {
        self.count
    }

    /// Whether every active 32-bit event stream has reached its end marker.
    pub fn ended(&self) -> bool {
        self.joints[..self.count].iter().all(|joint| joint.ended())
    }

    /// Loads every joint entry of a stage animation, seeding each pose from the
    /// node's rest transform so a track the script never names keeps it.
    pub fn start(&mut self, pack: &Pack<'_>, anim: &AnimDesc) {
        self.count = 0;
        // `gcAddAnimJointAll(gobj, ..., 0.0F)`.
        self.gobj_frame = 0.0;
        for i in 0..anim.joint_count {
            if self.count == MAX_STAGE_JOINTS {
                break;
            }
            let Some(j) = pack.anim_joint(anim.first_joint + i) else {
                continue;
            };
            if j.script == AnimJoint::NO_SCRIPT || j.node == AnimJoint::NO_NODE {
                continue;
            }
            let Some(node) = pack.node(j.node) else {
                continue;
            };
            self.nodes[self.count] = j.node;
            self.joints[self.count] = crate::objanim::StageJoint::start(j.script, 0.0);
            self.poses[self.count] = JointPose {
                rotate: node.rest_rotate,
                translate: node.rest_translate,
                scale: node.rest_scale,
            };
            self.count += 1;
        }
    }

    /// `gcAddAnimAll`: the first play parses frame zero without advancing
    /// time. Kept separate from existing stage players' advancing start.
    pub fn start_changed(&mut self, pack: &Pack<'_>, anim: &AnimDesc) {
        self.start(pack, anim);
        for i in 0..self.count {
            if let Some(joint) = (0..anim.joint_count)
                .filter_map(|j| pack.anim_joint(anim.first_joint + j))
                .find(|joint| joint.node == self.nodes[i])
            {
                self.joints[i] = crate::objanim::StageJoint::start_changed(joint.script, 0.0);
            }
        }
    }

    /// An item's tree (`itManagerMakeItem`): `gcAddAnimAll(..., 0.0F)` then
    /// the make's own `gcPlayAnimAll`, which parses frame 0. Each tick of
    /// this player is one of `Item::anim_ticks`' plays, so after `n` the
    /// tree stands at frame `n - 1`. The N64 shows the Link Bomb's `GObj`
    /// frame 1 on its creation frame, after the item process's play
    /// (RE-444), and Egg's script replays the ROM this way (RE-431).
    pub fn start_item(&mut self, pack: &Pack<'_>, anim: &AnimDesc) {
        self.start_changed(pack, anim);
    }

    /// The pose `grDisplayMakeGeometryLayer`'s `gcPlayAnimAll` leaves on a
    /// stage layer when it is made, before its first tick: frame zero
    /// parsed, time not advanced. [`Self::start`]'s first tick plays this
    /// and the next frame at once, so a moving collision group's first
    /// speed is measured from here, not from the rest pose
    /// (`mpCollisionPlayYakumonoAnim`, RE-524).
    pub fn creation_pose(
        pack: &Pack<'_>,
        anim: &AnimDesc,
    ) -> Result<Self, crate::objanim::AnimError> {
        let mut a = Self::new();
        a.play_creation_pose(pack, anim)?;
        Ok(a)
    }

    /// [`Self::creation_pose`] played on this animator instead of a new
    /// one. Every reader stops at the joint count, which [`Self::start`]
    /// rewrites with the joints below it, so a later `start` of the same
    /// clip leaves no trace of this play.
    pub fn play_creation_pose(
        &mut self,
        pack: &Pack<'_>,
        anim: &AnimDesc,
    ) -> Result<(), crate::objanim::AnimError> {
        self.start_changed(pack, anim);
        if let Some(script) = pack.anim_script(anim) {
            self.tick(script)?;
        }
        Ok(())
    }

    /// The node a joint drives, and the pose it has reached. Exposed so a
    /// verifier can compare a packed replay against one run straight off the
    /// archive, which is the check that the packing path — script offsets, node
    /// indices, the copied blob — is right (RE-052).
    pub fn joint(&self, i: usize) -> Option<(u32, &JointPose)> {
        (i < self.count).then(|| (self.nodes[i], &self.poses[i]))
    }

    /// The pose the joint driving `node` has reached, if one drives it.
    pub fn node_pose(&self, node: u32) -> Option<&JointPose> {
        self.pose_for(node)
    }

    /// Whether the joint driving `node` has reached its script's end (its
    /// `DObj::anim_frame` at or below 0), if one drives it (RE-425: the
    /// Arwing and the car end with one node, not the whole tree).
    pub fn node_ended(&self, node: u32) -> Option<bool> {
        (0..self.count)
            .find(|&i| self.nodes[i] == node)
            .map(|i| self.joints[i].ended())
    }

    /// `node`'s `track` value, or `None` while its animation has not keyed
    /// that track (or no joint drives the node).
    pub fn track_value(&self, node: u32, track: usize) -> Option<f32> {
        let i = (0..self.count).find(|&i| self.nodes[i] == node)?;
        self.joints[i].track_value(track)
    }

    /// Source DObj flags: bit 0 hides this mesh; bit 1 hides its subtree.
    pub fn flags(&self, node: u32) -> u16 {
        (0..self.count)
            .find(|&i| self.nodes[i] == node)
            .map_or(0, |i| self.joints[i].flags)
    }

    pub fn visible(&self, pack: &Pack<'_>, node: u32) -> bool {
        if self.flags(node) & 1 != 0 {
            return false;
        }
        let mut ancestor = node;
        for _ in 0..MAX_NODES {
            if self.flags(ancestor) & 2 != 0 {
                return false;
            }
            let Some(parent) = pack.node_parent(ancestor) else {
                return true;
            };
            if parent == u32::MAX {
                return true;
            }
            ancestor = parent;
        }
        false
    }

    /// `dobj->anim_joint.event32 = NULL` on every node this player drives
    /// ([`crate::objanim::StageJoint::clear_script`]).
    pub fn clear_scripts(&mut self) {
        for joint in &mut self.joints[..self.count] {
            joint.clear_script();
        }
    }

    /// Advances every joint one tick. `script` is the animation file's bytes,
    /// which the joint offsets index into.
    pub fn tick(&mut self, script: &[u8]) -> Result<(), crate::objanim::AnimError> {
        self.tick_nodes(script, |_| true)
    }

    /// [`Self::tick`] at a `DObj::anim_speed` other than 1
    /// (`gcSetAllAnimSpeed`).
    pub fn tick_speed(
        &mut self,
        script: &[u8],
        speed: f32,
    ) -> Result<(), crate::objanim::AnimError> {
        for i in 0..self.count {
            self.joints[i].tick(script, speed, &mut self.poses[i])?;
            if let Some(f) = self.joints[i].gobj_frame() {
                self.gobj_frame = f;
            }
        }
        Ok(())
    }

    /// Stage collision On/Off groups skip their own animation callbacks.
    pub fn tick_nodes(
        &mut self,
        script: &[u8],
        enabled: impl Fn(u32) -> bool,
    ) -> Result<(), crate::objanim::AnimError> {
        for i in 0..self.count {
            if enabled(self.nodes[i]) {
                self.joints[i].tick(script, 1.0, &mut self.poses[i])?;
                if let Some(f) = self.joints[i].gobj_frame() {
                    self.gobj_frame = f;
                }
            }
        }
        Ok(())
    }

    /// Composes world matrices for an object, exactly as [`Skeleton::compose`]
    /// does. A node outside every animated ancestor chain keeps its packed
    /// matrix; an unanimated child of a moving joint is recomposed from rest.
    pub fn compose(&self, pack: &Pack<'_>, object: &ObjectDesc, out: &mut [Mat4]) -> usize {
        self.compose_scaled(pack, object, out, None)
    }

    /// [`StageAnimator::compose`] with `node`'s own scale set to `scale` on
    /// every axis its animation does not write: the original sets
    /// `dobj->scale` after the effect is made (the rebirth halo's
    /// `child->scale`) and only a scale track overwrites it again.
    pub fn compose_scaled(
        &self,
        pack: &Pack<'_>,
        object: &ObjectDesc,
        out: &mut [Mat4],
        set_scale: Option<(u32, f32)>,
    ) -> usize {
        let count = (object.node_count as usize).min(out.len()).min(MAX_NODES);
        let mut changed = [false; MAX_NODES];
        for i in 0..count {
            let index = object.first_node + i as u32;
            let Some(node) = pack.node(index) else {
                out[i] = Mat4::IDENTITY;
                continue;
            };
            let parent_local = node
                .parent
                .checked_sub(object.first_node)
                .filter(|&parent| parent < i as u32)
                .map(|parent| parent as usize);
            let pose = self.pose_for(index);
            let scaled = set_scale.filter(|&(n, _)| n == index).map(|(_, v)| v);
            changed[i] = pose.is_some()
                || scaled.is_some()
                || parent_local.is_some_and(|parent| changed[parent]);
            if !changed[i] {
                // Neither this node nor an ancestor moved, so its packed world
                // matrix remains exact and avoids needless recomposition.
                out[i] = Mat4(node.world);
                continue;
            }
            let rest = JointPose {
                rotate: node.rest_rotate,
                translate: node.rest_translate,
                scale: node.rest_scale,
            };
            let mut pose = *pose.unwrap_or(&rest);
            if let Some(v) = scaled {
                for (axis, s) in pose.scale.iter_mut().enumerate() {
                    if self
                        .track_value(index, crate::figatree::TRACK_SCA_X + axis)
                        .is_none()
                    {
                        *s = v;
                    }
                }
            }
            let pose = &pose;
            let t = [
                pose.translate[0] / MODEL_SCALE,
                pose.translate[1] / MODEL_SCALE,
                pose.translate[2] / MODEL_SCALE,
            ];
            let local = Mat4::from_trs_display(t, pose.rotate, pose.scale);
            out[i] = match parent_local {
                Some(parent) => out[parent].mul(&local),
                None => local,
            };
        }
        count
    }

    /// Item tree after `lbCommonEjectTreeDObj` removed descriptor 0.
    /// The caller supplies descriptor 1's live root transform (its world
    /// translation is owned by gameplay); descendant scripts remain local.
    pub fn compose_item(
        &self,
        pack: &Pack<'_>,
        object: &ObjectDesc,
        root: JointPose,
        out: &mut [Mat4],
    ) -> usize {
        let count = (object.node_count as usize).min(out.len()).min(MAX_NODES);
        if count != 0 {
            out[0] = Mat4::IDENTITY;
        }
        for i in 1..count {
            let index = object.first_node + i as u32;
            let Some(node) = pack.node(index) else {
                out[i] = Mat4::IDENTITY;
                continue;
            };
            let pose = if i == 1 {
                root
            } else {
                self.pose_for(index).copied().unwrap_or(JointPose {
                    rotate: node.rest_rotate,
                    translate: node.rest_translate,
                    scale: node.rest_scale,
                })
            };
            let local = Mat4::from_trs_display(
                pose.translate.map(|t| t / MODEL_SCALE),
                pose.rotate,
                pose.scale,
            );
            out[i] = node
                .parent
                .checked_sub(object.first_node)
                .filter(|&parent| parent >= 1 && parent < i as u32)
                .map_or(local, |parent| out[parent as usize].mul(&local));
        }
        count
    }

    /// Signed billboard X/Y scales for every node in `object`.
    ///
    /// `gcPrepDObjMatrix` does not derive these from a composed matrix. It
    /// carries `gGCScaleX` down the DObj tree and uses:
    ///
    /// ```text
    /// x = ancestor_x * node.scale.x
    /// y = ancestor_x * node.scale.y
    /// ```
    ///
    /// Computing from the local poses preserves negative animated scale and
    /// also matches the original's deliberate use of ancestor X for both
    /// axes. Entries for non-billboard nodes are populated because descendants
    /// need their cumulative X value.
    pub fn billboard_scales(
        &self,
        pack: &Pack<'_>,
        object: &ObjectDesc,
        out: &mut [[f32; 2]],
    ) -> usize {
        let count = (object.node_count as usize).min(out.len()).min(MAX_NODES);
        let mut cumulative_x = [1.0; MAX_NODES];
        for i in 0..count {
            let index = object.first_node + i as u32;
            let Some(node) = pack.node(index) else {
                out[i] = [1.0; 2];
                continue;
            };
            let parent_x = node
                .parent
                .checked_sub(object.first_node)
                .filter(|&parent| parent < i as u32)
                .map_or(1.0, |parent| cumulative_x[parent as usize]);
            let scale = self
                .pose_for(index)
                .map_or(node.rest_scale, |pose| pose.scale);
            out[i] = [parent_x * scale[0], parent_x * scale[1]];
            cumulative_x[i] = out[i][0];
        }
        count
    }

    fn pose_for(&self, node: u32) -> Option<&JointPose> {
        (0..self.count)
            .find(|&i| self.nodes[i] == node)
            .map(|i| &self.poses[i])
    }
}

/// Ticks every `MatAnimDesc` the pack defines (RE-089–094), once per frame.
///
/// Unlike [`Skeleton`]/[`StageAnimator`] above, a `MatAnimDesc` entry is a
/// property of a *texture*, not one fighter or stage layer, so every entry
/// shares one clock. The battle restarts it at its ground's creation
/// ([`Self::start_stage`]), and [`Self::tick`] runs on each world update
/// after that, independent of which stage or fighter is on screen (cheap
/// either way: one short script per entry).
///
/// One joint per `MatAnimDesc`, sized from the pack at [`Self::start`]
/// (RE-322). A fixed 64-slot array silently left the v33 pack's entries
/// 64..103 unticked: Race to the Finish's colour tracks and the later
/// stages' palette and texture cycles never ran.
pub struct MaterialAnimator {
    joints: alloc::vec::Vec<crate::matanim::MaterialJoint>,
}

/// The live material UV state before the renderer converts the original
/// tile-window equations to the GE's scale/offset registers.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MaterialUv {
    pub trau: f32,
    pub trav: f32,
    pub scau: f32,
    pub scav: f32,
    pub base_trau: f32,
    pub base_trav: f32,
    pub base_scau: f32,
    pub base_scav: f32,
    pub mode: u32,
    pub tile_bias: f32,
    pub tile_width: f32,
    pub tile_height: f32,
    /// Under [`crate::mobj::UV_MODE_HALF`], the `unk10 == 1` inputs for
    /// this tile's U origin: `unk24` (tile 0) or `unk44` (tile 1), then
    /// `unk28` (RE-327).
    pub half: [f32; 2],
}

impl MaterialUv {
    /// This window restricted to the state the primitive's `MObj` still owns
    /// ([`crate::pack::flags::TILE0_ANIM`], [`crate::pack::flags::SCALE_ANIM`],
    /// RE-326), or `None` when it owns neither.
    pub fn owned_by(self, prim_flags: u32) -> Option<MaterialUv> {
        use crate::pack::flags::{SCALE_ANIM, TILE0_ANIM};
        let mut mode = self.mode;
        if prim_flags & TILE0_ANIM == 0 {
            mode &= !3;
        }
        if prim_flags & SCALE_ANIM == 0 {
            mode &= !4;
        }
        (mode & 7 != 0).then_some(MaterialUv { mode, ..self })
    }

    /// `tra`/`sca` as `gcDrawMObjForDObj` reads them after its
    /// `unk10 == 1` branch (`objdisplay.c:1173-1178`, [`crate::mobj::halve_u`]).
    fn drawn(&self, tra: [f32; 2], sca: [f32; 2]) -> ([f32; 2], [f32; 2]) {
        if self.mode & u32::from(crate::mobj::UV_MODE_HALF) == 0 {
            return (tra, sca);
        }
        let [origin, unk28] = self.half;
        let (scau, trau, _) = crate::mobj::halve_u(sca[0], tra[0], 0.0, origin, unk28, 0.0);
        ([trau, tra[1]], [scau, sca[1]])
    }

    /// `gDPSetTileSize`'s `uls`/`ult` for `tra`/`sca` as
    /// `gcDrawMObjForDObj` computes them: `s32` quarter texels, truncated
    /// where the source truncates (`objdisplay.c:1353-1397`).
    fn tile_origin(&self, tra: [f32; 2], sca: [f32; 2]) -> [i32; 2] {
        const EPS: f32 = 1.0 / 65535.0;
        let (tra, sca) = self.drawn(tra, sca);
        let (w, h, a) = (self.tile_width, self.tile_height, self.tile_bias);
        let [trau, trav] = tra;
        let [scau, scav] = sca;
        match self.mode & 3 {
            2 => {
                let uls = if scau.abs() > EPS {
                    ((w * trau) / scau) * 4.0
                } else {
                    0.0
                };
                let ult = if scav.abs() > EPS {
                    ((h * trav) / scav) * 4.0
                } else {
                    0.0
                };
                [(uls as i32).max(0), (ult as i32).max(0)]
            }
            1 => {
                let uls = if scau.abs() > EPS {
                    (((w * trau) + a) / scau) * 4.0
                } else {
                    0.0
                };
                let ult = if scav.abs() > EPS {
                    (((((1.0 - scav) - trav) * h) + a) / scav) * 4.0
                } else {
                    0.0
                };
                [uls as i32, ult as i32]
            }
            _ => [0, 0],
        }
    }

    /// The GE texture scale and offset that move a primitive packed at the
    /// rest window to this live one (RE-326). `dims` are the dimensions
    /// handed to `sceGuTexImage`; the offset is in the GE's normalised
    /// units, added after the scale. Clamp affects sampling, not this affine.
    ///
    /// Per axis, in texels, a packed coordinate `x` is the vertex loaded
    /// under the rest `gSPTexture` scale, less the rest tile origin on every
    /// axis (`mesh::Builder::push_vertex`, as `mobj::read` clamps
    /// it). The RDP samples the vertex under the live scale less the live
    /// origin, which `gDPSetTileSize` holds in a 12-bit field:
    ///
    /// ```text
    /// rdp(x) = (x + baked) * k - (uls & 0xFFF) / 4,   k = s_live / s_rest
    /// ```
    ///
    /// `k` is `base_sca / sca`, after the `unk10 == 1` halving: the `u16`
    /// truncation of `gSPTexture`'s scale is not modelled, as `unk08` is
    /// not packed.
    pub fn ge_affine(&self, dims: (u32, u32), _clamp: (bool, bool)) -> UvAffine {
        const EPS: f32 = 1.0 / 65535.0;
        let (_, live) = self.drawn([self.trau, self.trav], [self.scau, self.scav]);
        let (_, rest) = self.drawn(
            [self.base_trau, self.base_trav],
            [self.base_scau, self.base_scav],
        );
        if live.iter().chain(&rest).any(|s| s.abs() <= EPS) {
            return UvAffine::IDENTITY;
        }
        let k = if self.mode & 4 != 0 {
            [rest[0] / live[0], rest[1] / live[1]]
        } else {
            [1.0, 1.0]
        };
        let rest = self.tile_origin(
            [self.base_trau, self.base_trav],
            [self.base_scau, self.base_scav],
        );
        let live = self.tile_origin([self.trau, self.trav], [self.scau, self.scav]);
        let dim = [dims.0.max(1) as f32, dims.1.max(1) as f32];
        let offset = |axis: usize| {
            if self.mode & 3 == 0 {
                return 0.0;
            }
            let baked = rest[axis].clamp(0, 0xFFFF) as f32 / 4.0;
            (baked * k[axis] - (live[axis] & 0xFFF) as f32 / 4.0) / dim[axis]
        };
        UvAffine {
            scale_s: k[0],
            scale_t: k[1],
            offset_s: offset(0),
            offset_t: offset(1),
        }
    }
}

/// `sceGuTexScale`/`sceGuTexOffset` factors a live material window applies
/// on top of a primitive's static mapping: `u' = u * scale + offset`, in
/// the GE's normalised texture units.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UvAffine {
    pub scale_s: f32,
    pub scale_t: f32,
    pub offset_s: f32,
    pub offset_t: f32,
}

impl UvAffine {
    pub const IDENTITY: Self = Self {
        scale_s: 1.0,
        scale_t: 1.0,
        offset_s: 0.0,
        offset_t: 0.0,
    };
}

/// The live second pass of a two-tile fractional image blend (RE-321).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LodBlendState {
    /// `TEXEL1`'s texture: [`crate::pack::LodBlendDesc::next_textures`] at
    /// the live `TextureIDNext`.
    pub texture: u32,
    /// `PRIM_LOD_FRAC` from the live `lfrac`.
    pub frac: u8,
    /// Tile 1's live window, in [`MaterialUv`] form: `ScrU`/`ScrV` in the
    /// `Tra*` slots, over the tile-1 equation inputs. `None` when neither
    /// tile 1's window nor the shared scale moves.
    pub uv: Option<MaterialUv>,
    /// [`crate::pack::LodBlendDesc::origin_delta`]: tile 0's static origin
    /// less tile 1's, quarter texels.
    pub origin_delta: [i16; 2],
}

impl LodBlendState {
    /// The second pass's GE scale and offset: tile 1's live window
    /// ([`MaterialUv::ge_affine`], or the identity at rest) re-based from
    /// the tile 0 origin the packed UVs had removed. Per axis, in texels,
    /// with `x` packed and `k` the live scale over the rest:
    ///
    /// ```text
    /// rdp(x) = (x + o0) * k - live1 = [(x + o1) * k - live1] + (o0 - o1) * k
    /// ```
    ///
    /// The bracket is the window affine (its rest origin is tile 1's, as
    /// the converter required when the two were equal); the last term
    /// moves by the static difference, scaled with the coordinate.
    pub fn ge_affine(&self, dims: (u32, u32), clamp: (bool, bool)) -> UvAffine {
        let mut affine = self
            .uv
            .map_or(UvAffine::IDENTITY, |uv| uv.ge_affine(dims, clamp));
        let dim = [dims.0.max(1) as f32, dims.1.max(1) as f32];
        affine.offset_s += f32::from(self.origin_delta[0]) / 4.0 * affine.scale_s / dim[0];
        affine.offset_t += f32::from(self.origin_delta[1]) / 4.0 * affine.scale_t / dim[1];
        affine
    }
}

impl Default for MaterialAnimator {
    fn default() -> Self {
        MaterialAnimator::new()
    }
}

impl MaterialAnimator {
    pub fn new() -> Self {
        MaterialAnimator {
            joints: alloc::vec::Vec::new(),
        }
    }

    /// Loads one [`crate::matanim::MaterialJoint`] per [`crate::pack::MatAnimDesc`]
    /// the pack defines. A pack's own entry order is its `MatAnimDesc` table
    /// order, so position `i` here is always the same `i` a [`TextureDesc::
    /// mat_anim`](crate::pack::TextureDesc::mat_anim) index names — no
    /// separate lookup table is needed.
    pub fn start(&mut self, pack: &Pack<'_>) {
        self.joints.clear();
        self.joints.extend((0..pack.mat_anim_count()).map(|i| {
            let script = pack.mat_anim(i).map_or(0, |a| a.script);
            crate::matanim::MaterialJoint::start(script, 0.0)
        }));
    }

    /// The battle ground's creation (`grDisplayMakeGeometryLayer`,
    /// `grZebesMakeAcid`, ...): `gcAddAnimAll(..., 0.0F)` restarts every
    /// MObj's `matanim_joint` (`anim_wait = AOBJ_ANIM_CHANGED`,
    /// `anim_frame = 0`), then the maker calls `gcPlayAnimAll` once directly
    /// (`grdisplay.c:208-210`, `grzebes.c:95-102`). That call's parse takes
    /// the `CHANGED` branch (no clock advance, `objanim.c:842-845`) but its
    /// play still adds `anim_speed` to every live track's length
    /// (`objanim.c:1268-1271`): one [`Self::tick`]. The GObj's
    /// `gcPlayAnimAll` process then runs one more per `gcRunAll`, so after
    /// `n` battle updates every clock reads `anim_frame == n`, from the
    /// ground's creation and not from boot.
    pub fn start_stage(&mut self, pack: &Pack<'_>) {
        self.start(pack);
        self.tick(pack);
    }

    /// How many `MatAnimDesc` entries this animator ticks.
    pub fn len(&self) -> usize {
        self.joints.len()
    }

    pub fn is_empty(&self) -> bool {
        self.joints.is_empty()
    }

    /// The joint driving `mat_anim`, for reference checks that read raw
    /// track values rather than a resolved texture, palette or UV.
    pub fn joint(&self, mat_anim: u32) -> Option<&crate::matanim::MaterialJoint> {
        self.joints.get(mat_anim as usize)
    }

    /// Advances every tracked animation one tick. Each entry ticks against
    /// its own source file's bytes, since different `MatAnimDesc`s can come
    /// from different archive files (RE-089's own same-file scope limit is
    /// about *resolving* a script, not about where two different scripts
    /// live relative to each other).
    pub fn tick(&mut self, pack: &Pack<'_>) {
        for (i, joint) in self.joints.iter_mut().enumerate() {
            let Some(a) = pack.mat_anim(i as u32) else {
                continue;
            };
            let Some(data) = pack.mat_anim_file(&a) else {
                continue;
            };
            let _ = joint.tick(data, 1.0);
        }
    }

    /// The currently-resolved palette variant; see [`resolve_palette`].
    pub fn resolved_palette(&self, pack: &Pack<'_>, mat_anim: u32) -> Option<u32> {
        resolve_palette(self.joints.get(mat_anim as usize)?, pack, mat_anim)
    }

    /// The current `TextureIDCurrent` sprite frame; see [`resolve_texture`].
    pub fn resolved_texture(&self, pack: &Pack<'_>, mat_anim: u32) -> Option<u32> {
        resolve_texture(self.joints.get(mat_anim as usize)?, pack, mat_anim)
    }

    /// Resolves `TraU`/`TraV`/`ScaU`/`ScaV`, retaining the MObjSub rest
    /// values for tracks the script leaves untouched. `ScrU`/`ScrV` target
    /// tile 1 and `SetLFrac` is a two-frame RDP blend; neither aliases these
    /// tile-0 coordinates (RE-086/RE-211); [`Self::resolved_lod_blend`] owns
    /// both (RE-321).
    pub fn resolved_uv(&self, pack: &Pack<'_>, mat_anim: u32) -> Option<MaterialUv> {
        resolve_uv(self.joints.get(mat_anim as usize)?, pack, mat_anim)
    }

    /// The two-tile fractional blend's second pass as of the last tick, or
    /// `None` when `mat_anim` has no [`crate::pack::LodBlendDesc`] (RE-321).
    ///
    /// Read in the same frame, from the same joint, as
    /// [`Self::resolved_texture`] and [`Self::resolved_uv`], so the two
    /// texture ids, the fraction and both tile windows can never be a tick
    /// apart. Each value follows `gcDrawMObjForDObj`: `texture_id_next` is a
    /// `u16` (the float truncates), and the fraction is
    /// [`crate::lod_blend::prim_lod_frac`] of `lfrac`, which starts at the
    /// `MObjSub`'s `prim_l / 255`.
    pub fn resolved_lod_blend(&self, pack: &Pack<'_>, mat_anim: u32) -> Option<LodBlendState> {
        resolve_lod_blend(self.joints.get(mat_anim as usize), pack, mat_anim)
    }

    /// The material colour registers driven by a stage script.  This is the
    /// same original `gcPlayMObjMatAnim` colour window as manager effects;
    /// stages merely own a pack-lifetime clock instead of a spawn-local one.
    pub fn resolved_colors(&self, mat_anim: u32) -> Option<EffectColors> {
        let j = self.joints.get(mat_anim as usize)?;
        Some(EffectColors {
            prim: j.track_color(crate::matanim::TRACK_PRIM_COLOR),
            env: j.track_color(crate::matanim::TRACK_ENV_COLOR),
            blend: j.track_color(crate::matanim::TRACK_BLEND_COLOR),
            light1: j.track_color(crate::matanim::TRACK_LIGHT1_COLOR),
            light2: j.track_color(crate::matanim::TRACK_LIGHT2_COLOR),
        })
    }
}

/// See [`MaterialAnimator::resolved_uv`].
fn resolve_uv(
    j: &crate::matanim::MaterialJoint,
    pack: &Pack<'_>,
    mat_anim: u32,
) -> Option<MaterialUv> {
    let a = pack.mat_anim(mat_anim)?;
    if a.uv_mode == 0
        || !(crate::matanim::TRACK_TRA_U..=crate::matanim::TRACK_SCA_V)
            .any(|i| j.track_value(i).is_some())
    {
        return None;
    }
    let base = |i| f32::from_bits(a.base_tracks[i]);
    Some(MaterialUv {
        trau: j
            .track_value(crate::matanim::TRACK_TRA_U)
            .unwrap_or_else(|| base(1)),
        trav: j
            .track_value(crate::matanim::TRACK_TRA_V)
            .unwrap_or_else(|| base(2)),
        scau: j
            .track_value(crate::matanim::TRACK_SCA_U)
            .unwrap_or_else(|| base(3)),
        scav: j
            .track_value(crate::matanim::TRACK_SCA_V)
            .unwrap_or_else(|| base(4)),
        base_trau: base(1),
        base_trav: base(2),
        base_scau: base(3),
        base_scav: base(4),
        mode: a.uv_mode,
        tile_bias: a.uv_tile_params[0] as f32,
        tile_width: a.uv_tile_params[1] as f32,
        tile_height: a.uv_tile_params[2] as f32,
        half: [f32::from_bits(a.uv_half[0]), f32::from_bits(a.uv_half[1])],
    })
}

/// See [`MaterialAnimator::resolved_lod_blend`]. `j` is `None` when no
/// player ticks `mat_anim`: every track reads its rest value.
fn resolve_lod_blend(
    j: Option<&crate::matanim::MaterialJoint>,
    pack: &Pack<'_>,
    mat_anim: u32,
) -> Option<LodBlendState> {
    use crate::matanim::{
        TRACK_SCA_U, TRACK_SCA_V, TRACK_SCR_U, TRACK_SCR_V, TRACK_SET_LFRAC, TRACK_TEXTURE_ID_NEXT,
    };
    let lod = pack.lod_blend(mat_anim)?;
    let a = pack.mat_anim(mat_anim)?;
    if lod.next_count == 0 {
        return None;
    }
    let live = |track: usize| j.and_then(|j| j.track_value(track));
    let base = |i: usize| f32::from_bits(a.base_tracks[i]);
    let lfrac = live(TRACK_SET_LFRAC).unwrap_or_else(|| base(TRACK_SET_LFRAC));
    let next = live(TRACK_TEXTURE_ID_NEXT).map_or(0, |v| v.max(0.0) as u32);
    let texture = lod.next_textures[next.min(lod.next_count - 1) as usize];
    let moves = [TRACK_SCA_U, TRACK_SCA_V, TRACK_SCR_U, TRACK_SCR_V]
        .into_iter()
        .any(|t| live(t).is_some());
    let uv = (a.uv_mode != 0 && moves).then(|| MaterialUv {
        trau: live(TRACK_SCR_U).unwrap_or_else(|| base(TRACK_SCR_U)),
        trav: live(TRACK_SCR_V).unwrap_or_else(|| base(TRACK_SCR_V)),
        scau: live(TRACK_SCA_U).unwrap_or_else(|| base(TRACK_SCA_U)),
        scav: live(TRACK_SCA_V).unwrap_or_else(|| base(TRACK_SCA_V)),
        base_trau: base(TRACK_SCR_U),
        base_trav: base(TRACK_SCR_V),
        base_scau: base(TRACK_SCA_U),
        base_scav: base(TRACK_SCA_V),
        // `gDPSetTileSize(1, ...)` has only the normal window form; the
        // `G_TEXTURE` scale bit and the `unk10 == 1` halving, here of
        // `scrollu` about `unk44`, are shared with tile 0.
        mode: 1 | (a.uv_mode & (4 | u32::from(crate::mobj::UV_MODE_HALF))),
        tile_bias: lod.tile1_params[0] as f32,
        tile_width: lod.tile1_params[1] as f32,
        tile_height: lod.tile1_params[2] as f32,
        half: [f32::from_bits(a.uv_half[2]), f32::from_bits(a.uv_half[1])],
    });
    (texture != crate::pack::TextureDesc::NO_ANIM).then_some(LodBlendState {
        texture,
        frac: crate::lod_blend::prim_lod_frac(lfrac),
        uv,
        origin_delta: lod.origin_delta,
    })
}

/// `palettes[(s32)mobj->palette_id]` (`gcDrawMObjForDObj`), as an absolute
/// index into [`Pack::mat_anim_palette`] — or `None` if the script never set
/// `PaletteID` or the entry packed no palette table.
///
/// `gcPlayMObjMatAnim` writes `palette_id` for every live kind, and the draw
/// truncates it: a linear hold at 0 selects palette 0 (RE-324). Clamped into
/// the entry's own `palette_count` so an out-of-range value cannot read a
/// neighbouring `MatAnimDesc`'s palettes.
fn resolve_palette(
    j: &crate::matanim::MaterialJoint,
    pack: &Pack<'_>,
    mat_anim: u32,
) -> Option<u32> {
    let v = j.track_value(crate::matanim::TRACK_PALETTE_ID)?;
    let a = pack.mat_anim(mat_anim)?;
    if a.palette_count == 0 {
        return None;
    }
    let index = (v as i32).clamp(0, a.palette_count as i32 - 1) as u32;
    Some(a.first_palette + index)
}

/// `sprites[texture_id_curr]` as an absolute pack texture index, or `None`
/// if the script never set `TextureIDCurrent`. Not step-gated:
/// `gcPlayMObjMatAnim` assigns every live kind (RE-175), and the `u16`
/// field truncates the float.
fn resolve_texture(
    j: &crate::matanim::MaterialJoint,
    pack: &Pack<'_>,
    mat_anim: u32,
) -> Option<u32> {
    let value = j.track_value(crate::matanim::TRACK_TEXTURE_ID_CURRENT)?;
    let a = pack.mat_anim(mat_anim)?;
    if a.texture_count == 0 {
        return None;
    }
    let index = (value.max(0.0) as u32).min(a.texture_count - 1);
    let texture = a.textures[index as usize];
    (texture != crate::pack::TextureDesc::NO_ANIM).then_some(texture)
}

/// Distinct `MatAnimDesc` scripts (or stage `MObj`s) one
/// [`EffectMaterialAnimator`] can play at once. Many primitives on one
/// object share a script (RE-175), so this counts distinct scripts, not
/// primitives. Measured over every pack object except the stage map layers
/// (`StageDesc::layers`): the most is Link's Spin Attack band (file 324 at
/// 0x11908), 9. A slot past the cap is dropped and plays the pack-lifetime
/// clock instead; `tests/effect_mat_anim_capacity.rs` fails if any object
/// outside the map layers exceeds it.
///
/// The map layers play [`MaterialAnimator`] (a `Vec`, restarted per ground
/// creation by `start_stage`), never this player, so two of them exceed
/// the cap without loss: Zebes' layer 1 (file 105 at 0x5AC0) has 18,
/// Final Destination's (file 114 at 0x4D48) 13. Sizing to them would cost
/// 4.6 KB per instance for objects nothing here plays.
pub const MAX_EFFECT_MAT_ANIMS: usize = 9;

/// Restarts and ticks only the `MatAnimDesc` entries one spawned effect's
/// own primitives reference, instead of [`MaterialAnimator`]'s pack-lifetime
/// clock.
///
/// [`MaterialAnimator`]'s own doc comment explains why a *texture's* palette
/// cycle never needed a restart boundary: there is no per-object "start" for
/// a property fixed to a texture. A manager-effect material script is
/// different -- its real lifetime starts at spawn/selection
/// (`gcAddMObjMatAnimJoint`/`gcParseMObjMatAnimJoint`, replayed fresh every
/// time the effect is selected), not once when the pack loads (RE-175).
/// [`Self::start`] is that boundary; a caller restarts a fresh
/// `EffectMaterialAnimator` (or calls [`Self::start`] again) whenever the
/// effect itself restarts.
///
/// A stage controller object uses the same player (RE-364): it starts a
/// script per `MObj` with [`Self::restart`] whenever its controller calls
/// `gcAddAnimAll`, and an `MObj` no script has reached draws its rest
/// material.
#[derive(Clone, Copy)]
pub struct EffectMaterialAnimator {
    slots: [(u32, crate::matanim::MaterialJoint); MAX_EFFECT_MAT_ANIMS],
    /// `mobj->anim_wait = AOBJ_ANIM_NULL`: the slot keeps its values but no
    /// longer ticks, until a restart.
    halted: [bool; MAX_EFFECT_MAT_ANIMS],
    count: usize,
}

impl Default for EffectMaterialAnimator {
    fn default() -> Self {
        EffectMaterialAnimator::new()
    }
}

/// The five colour tracks [`EffectMaterialAnimator::resolved_colors`]
/// reads, one slot per track exactly like [`crate::matanim::Colors`] but
/// covering all five rather than the three a fighter costume list ever used.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct EffectColors {
    pub prim: Option<[u8; 4]>,
    pub env: Option<[u8; 4]>,
    pub blend: Option<[u8; 4]>,
    pub light1: Option<[u8; 4]>,
    pub light2: Option<[u8; 4]>,
}

impl EffectColors {
    /// Resolves live material colour registers into one packed PSP vertex
    /// colour for a primitive with these [`crate::pack::flags`].
    /// `FLAT_COLOR`, `TEXTURE_BLEND` and `SHADE_MINUS_ENV` name the
    /// source-backed combiner mappings of RE-073/080 and RE-418; other RGB
    /// shapes retain their converter-resolved value. PRIM alpha is
    /// independent of RGB and replaces the packed vertex alpha whenever its
    /// track is live, unless the alpha formula reads no `PRIMITIVE`
    /// (`ALPHA_NO_PRIM`).
    pub fn vertex_color(self, packed: u32, prim_flags: u32) -> u32 {
        use crate::pack::flags;
        let mut rgba = packed.to_le_bytes();
        if prim_flags & flags::FLAT_COLOR != 0 {
            if let Some(prim) = self.prim {
                rgba[..3].copy_from_slice(&prim[..3]);
            } else if let Some(env) = self.env {
                rgba[..3].copy_from_slice(&env[..3]);
            }
        } else if prim_flags & flags::TEXTURE_BLEND != 0 {
            if let Some(env) = self.env {
                rgba[..3].copy_from_slice(&env[..3]);
            }
        } else if prim_flags & flags::SHADE_MINUS_ENV != 0 {
            if let Some(env) = self.env {
                for (c, e) in rgba[..3].iter_mut().zip(env) {
                    *c = c.saturating_sub(e);
                }
            }
        }
        if let Some(prim) = self.prim.filter(|_| prim_flags & flags::ALPHA_NO_PRIM == 0) {
            rgba[3] = prim[3];
        }
        crate::psp_texture::pack_abgr(rgba)
    }

    pub fn packed_prim(self) -> Option<u32> {
        self.prim.map(crate::psp_texture::pack_abgr)
    }

    pub fn packed_light1(self) -> Option<u32> {
        self.light1.map(crate::psp_texture::pack_abgr)
    }

    pub fn packed_light2(self) -> Option<u32> {
        self.light2.map(crate::psp_texture::pack_abgr)
    }
}

impl EffectMaterialAnimator {
    pub fn new() -> Self {
        EffectMaterialAnimator {
            slots: [(
                crate::pack::TextureDesc::NO_ANIM,
                crate::matanim::MaterialJoint::start(0, 0.0),
            ); MAX_EFFECT_MAT_ANIMS],
            halted: [false; MAX_EFFECT_MAT_ANIMS],
            count: 0,
        }
    }

    /// Restarts fresh joints, one per distinct `mat_anim` index `indices`
    /// names, each at frame 0. Duplicates collapse to one slot -- ticking
    /// the same script twice would waste a slot and could disagree with
    /// itself by a frame of drift -- and an index past
    /// [`MAX_EFFECT_MAT_ANIMS`] is dropped rather than wrapping onto
    /// another slot.
    pub fn start(&mut self, pack: &Pack<'_>, indices: impl Iterator<Item = u32>) {
        self.start_at(pack, indices, 0.0);
    }

    /// [`Self::start`] at `anim_frame`: `gcAddMatAnimJointAll(gobj,
    /// joints, anim_frame)`, whose first `gcPlayAnimAll` fast-forwards each
    /// script by that many frames (RE-420: the results emblem's colour).
    pub fn start_at(
        &mut self,
        pack: &Pack<'_>,
        indices: impl Iterator<Item = u32>,
        anim_frame: f32,
    ) {
        self.count = 0;
        for i in indices {
            if i == crate::pack::TextureDesc::NO_ANIM {
                continue;
            }
            if self.slots[..self.count].iter().any(|&(s, _)| s == i) {
                continue;
            }
            if self.count >= MAX_EFFECT_MAT_ANIMS {
                break;
            }
            let script = pack.mat_anim(i).map_or(0, |a| a.script);
            self.slots[self.count] = (i, crate::matanim::MaterialJoint::start(script, anim_frame));
            self.halted[self.count] = false;
            self.count += 1;
        }
    }

    /// `gcAddMObjMatAnimJoint(mobj, script, 0)`: restarts `mat_anim`'s
    /// joint on `script`, which may be any script in the same file as the
    /// entry's own, and leaves every other slot as it is. Returns `false`
    /// when `mat_anim` is new and every slot is taken.
    pub fn restart(&mut self, mat_anim: u32, script: u32) -> bool {
        let joint = crate::matanim::MaterialJoint::start(script, 0.0);
        if let Some(n) = self.slots[..self.count]
            .iter()
            .position(|(i, _)| *i == mat_anim)
        {
            self.slots[n].1 = joint;
            self.halted[n] = false;
            return true;
        }
        if self.count >= MAX_EFFECT_MAT_ANIMS {
            return false;
        }
        self.slots[self.count] = (mat_anim, joint);
        self.halted[self.count] = false;
        self.count += 1;
        true
    }

    /// `mobj->anim_wait = AOBJ_ANIM_NULL` on every `MObj` this player
    /// drives: each keeps its current values and stops ticking.
    pub fn halt_all(&mut self) {
        self.halted = [true; MAX_EFFECT_MAT_ANIMS];
    }

    /// `mobj->matanim_joint.event32 = NULL` on every `MObj` this player
    /// drives ([`crate::matanim::MaterialJoint::clear_script`]).
    pub fn clear_scripts(&mut self) {
        for (_, joint) in &mut self.slots[..self.count] {
            joint.clear_script();
        }
    }

    /// How many joints this player ticks.
    pub fn len(&self) -> usize {
        self.count
    }

    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// The joint playing `mat_anim`, if one was started.
    pub fn joint_for(&self, mat_anim: u32) -> Option<&crate::matanim::MaterialJoint> {
        self.joint(mat_anim)
    }

    /// Advances every tracked script one tick, each against its own source
    /// file's bytes (mirrors [`MaterialAnimator::tick`]).
    pub fn tick(&mut self, pack: &Pack<'_>) {
        self.tick_speed(pack, 1.0);
    }

    /// [`Self::tick`] at an `MObj::anim_speed` other than 1
    /// (`gcSetAllAnimSpeed`).
    pub fn tick_speed(&mut self, pack: &Pack<'_>, speed: f32) {
        for (n, (i, j)) in self.slots[..self.count].iter_mut().enumerate() {
            if self.halted[n] {
                continue;
            }
            let Some(a) = pack.mat_anim(*i) else { continue };
            let Some(data) = pack.mat_anim_file(&a) else {
                continue;
            };
            let _ = j.tick(data, speed);
        }
    }

    fn joint(&self, mat_anim: u32) -> Option<&crate::matanim::MaterialJoint> {
        self.slots[..self.count]
            .iter()
            .find(|&&(i, _)| i == mat_anim)
            .map(|(_, j)| j)
    }

    /// The currently-resolved palette variant, against this player's own
    /// restarted joints; see [`resolve_palette`].
    pub fn resolved_palette(&self, pack: &Pack<'_>, mat_anim: u32) -> Option<u32> {
        resolve_palette(self.joint(mat_anim)?, pack, mat_anim)
    }

    /// The currently-resolved sprite variant, against this player's own
    /// restarted joints; see [`resolve_texture`].
    pub fn resolved_texture(&self, pack: &Pack<'_>, mat_anim: u32) -> Option<u32> {
        resolve_texture(self.joint(mat_anim)?, pack, mat_anim)
    }

    /// The live tile-0 window and scale, against this player's own
    /// restarted joints; see [`MaterialAnimator::resolved_uv`].
    pub fn resolved_uv(&self, pack: &Pack<'_>, mat_anim: u32) -> Option<MaterialUv> {
        resolve_uv(self.joint(mat_anim)?, pack, mat_anim)
    }

    /// The two-tile blend's second pass, against this player's own
    /// restarted joints, or `None` when this player does not tick
    /// `mat_anim`; see [`MaterialAnimator::resolved_lod_blend`].
    pub fn resolved_lod_blend(&self, pack: &Pack<'_>, mat_anim: u32) -> Option<LodBlendState> {
        resolve_lod_blend(Some(self.joint(mat_anim)?), pack, mat_anim)
    }

    /// The five colour tracks' current RGBA bytes -- `None` for any this
    /// script never set, or set only by a kind
    /// [`crate::matanim::MaterialJoint::track_color`] does not trust (see
    /// its own doc comment).
    pub fn resolved_colors(&self, mat_anim: u32) -> Option<EffectColors> {
        let j = self.joint(mat_anim)?;
        Some(EffectColors {
            prim: j.track_color(crate::matanim::TRACK_PRIM_COLOR),
            env: j.track_color(crate::matanim::TRACK_ENV_COLOR),
            blend: j.track_color(crate::matanim::TRACK_BLEND_COLOR),
            light1: j.track_color(crate::matanim::TRACK_LIGHT1_COLOR),
            light2: j.track_color(crate::matanim::TRACK_LIGHT2_COLOR),
        })
    }
}

/// The `mat_anim` index of every primitive on `object`'s own meshes, in
/// node then primitive order: the `MObj`s a made graphic's material
/// scripts drive.
pub fn object_mat_anims<'p>(
    p: &'p Pack<'_>,
    object: &ObjectDesc,
) -> impl Iterator<Item = u32> + 'p {
    let (first, count) = (object.first_node, object.node_count);
    (0..count)
        .filter_map(move |n| p.node(first + n))
        .filter(|n| n.mesh != NodeDesc::NO_MESH)
        .filter_map(move |n| p.mesh(n.mesh))
        .flat_map(move |m| (0..m.prim_count).filter_map(move |i| p.prim(m.first_prim + i)))
        .map(|prim| prim.mat_anim)
}

/// A made weapon's or effect's transform and material players, driven by
/// its count of `gcPlayAnimAll` calls since `gcAddAnimAll(gobj, ..., 0.0F)`.
///
/// `gcAddAnimAll` leaves every `DObj` and `MObj` at `AOBJ_ANIM_CHANGED`, so
/// the first counted play only parses frame 0 (`objanim.c:277-282`): after
/// `n` plays both players stand at frame `n - 1`.
/// - Weapons: `wpManagerMakeWeapon` calls `gcAddAnimAll` and no
///   `gcPlayAnimAll`. `wpProcessProcWeaponMain`, added there, makes the
///   first play on the creation frame. The N64 shows the Spin Attack
///   weapon's `GObj` frame 0 on its creation frame and 1 on the next
///   (RE-540), the Thunder Jolts frame 0 on theirs (RE-543).
/// - Effects: `efManagerMakeEffect`'s own `gcPlayAnimAll` is the first play
///   (RE-540).
#[derive(Default)]
pub struct GraphicPlayers {
    pub anim: StageAnimator,
    pub materials: EffectMaterialAnimator,
}

impl GraphicPlayers {
    /// [`GraphicPlayers::default`] written straight into `dst`, with no
    /// 27 KB stack temporary (RE-469).
    ///
    /// # Safety
    ///
    /// `dst` must be valid for writes and aligned. Its old contents are
    /// overwritten without being dropped.
    #[inline(never)]
    pub unsafe fn write_new(dst: *mut GraphicPlayers) {
        use core::ptr::addr_of_mut;
        ssb_engine::all_fields!(GraphicPlayers { anim, materials });
        // SAFETY: the caller's contract; each field is written once.
        unsafe {
            StageAnimator::write_new(addr_of_mut!((*dst).anim));
            addr_of_mut!((*dst).materials).write(EffectMaterialAnimator::new());
        }
    }

    /// `gcAddAnimAll`: restarts `anim`'s joints and, given `mat_anims`,
    /// the material scripts, all at `AOBJ_ANIM_CHANGED`.
    pub fn start(
        &mut self,
        pack: &Pack<'_>,
        anim: Option<&AnimDesc>,
        mat_anims: Option<impl Iterator<Item = u32>>,
    ) {
        if let Some(anim) = anim {
            self.anim.start_changed(pack, anim);
        }
        if let Some(mat_anims) = mat_anims {
            self.materials.start(pack, mat_anims);
        }
    }

    /// One counted `gcPlayAnimAll` at `anim_speed` (`gcSetAllAnimSpeed`):
    /// the transform script, given its bytes, then the materials when
    /// `materials` is set.
    pub fn play(&mut self, pack: &Pack<'_>, script: Option<&[u8]>, materials: bool, speed: f32) {
        if let Some(script) = script {
            let _ = if speed == 1.0 {
                self.anim.tick(script)
            } else {
                self.anim.tick_speed(script, speed)
            };
        }
        if materials {
            if speed == 1.0 {
                self.materials.tick(pack);
            } else {
                self.materials.tick_speed(pack, speed);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pack::PackWriter;
    use crate::scene::{DObjDesc, DObjNode, SceneGraph};

    #[test]
    fn stage_animator_write_new_matches_new() {
        let mut slot = alloc::boxed::Box::<StageAnimator>::new_uninit();
        // SAFETY: the box is a valid, aligned slot; `write_new` fills it.
        let built = unsafe {
            StageAnimator::write_new(slot.as_mut_ptr());
            slot.assume_init()
        };
        let fresh = StageAnimator::new();
        let fields = |a: &StageAnimator| {
            alloc::format!(
                "{:?} {:?} {:?} {} {}",
                a.nodes,
                a.joints,
                a.poses,
                a.count,
                a.gobj_frame
            )
        };
        assert_eq!(fields(&built), fields(&fresh));
    }

    #[test]
    fn write_new_matches_default() {
        let mut slot = alloc::boxed::Box::<Skeleton>::new_uninit();
        // SAFETY: the box is a valid, aligned slot; `write_new` fills it.
        let built = unsafe {
            Skeleton::write_new(slot.as_mut_ptr());
            slot.assume_init()
        };
        assert_eq!(
            alloc::format!("{built:?}"),
            alloc::format!("{:?}", Skeleton::default())
        );
    }

    #[test]
    fn clone_from_in_place_matches_clone() {
        let mut src = Skeleton::new_boxed();
        src.nodes[2] = 7;
        src.anims[3].flags = 5;
        src.joints32[1] = Some(crate::objanim::StageJoint::start(4, 2.0));
        src.anim_joint = true;
        src.gobj_frame = 3.5;
        src.poses[4].translate = [1.0, 2.0, 3.0];
        src.joint_count = 9;
        src.speed = 0.5;
        src.lead_xrotn = true;
        src.lead_transn = true;
        src.locks.mask = 0x30;
        let mut dst = Skeleton::new_boxed();
        dst.clone_from_in_place(&src);
        let want = alloc::format!("{:?}", src.clone());
        assert_eq!(alloc::format!("{dst:?}"), want);
        assert_ne!(want, alloc::format!("{:?}", Skeleton::default()));
    }

    /// Link's entry leads with `TransN`, then `XRotN`: the interposed pose
    /// is the second joint's.
    #[test]
    fn the_xrotn_pose_follows_a_leading_transn() {
        let mut sk = Skeleton::new();
        sk.nodes[0] = AnimJoint::NO_NODE;
        sk.nodes[1] = AnimJoint::NO_NODE;
        sk.nodes[2] = 7;
        sk.poses[0].translate = [1.0, 0.0, 0.0];
        sk.poses[1].translate = [2.0, 0.0, 0.0];
        sk.joint_count = 3;
        assert!(sk.lead_xrotn_pose().is_none());
        sk.lead_xrotn = true;
        assert_eq!(sk.lead_xrotn_pose().unwrap().translate[0], 1.0);
        sk.lead_transn = true;
        assert_eq!(sk.lead_xrotn_pose().unwrap().translate[0], 2.0);
        // A model node in the slot is no lead.
        sk.nodes[1] = 6;
        assert!(sk.lead_xrotn_pose().is_none());
    }

    #[test]
    fn effect_colors_map_onto_the_two_packed_combiner_sources() {
        let colors = EffectColors {
            prim: Some([0x11, 0x22, 0x33, 0x44]),
            env: Some([0x55, 0x66, 0x77, 0x88]),
            ..EffectColors::default()
        };
        let original = crate::psp_texture::pack_abgr([1, 2, 3, 4]);

        assert_eq!(
            colors.vertex_color(original, crate::pack::flags::FLAT_COLOR),
            crate::psp_texture::pack_abgr([0x11, 0x22, 0x33, 0x44])
        );
        assert_eq!(
            colors.vertex_color(original, crate::pack::flags::TEXTURE_BLEND),
            crate::psp_texture::pack_abgr([0x55, 0x66, 0x77, 0x44])
        );
        assert_eq!(
            colors.vertex_color(original, 0),
            crate::psp_texture::pack_abgr([1, 2, 3, 0x44])
        );
        // RE-418: `(SHADE - ENV) * TEXEL0` subtracts ENV, clamped at zero.
        let shade = crate::psp_texture::pack_abgr([0xFF, 0x40, 0x60, 0x90]);
        assert_eq!(
            colors.vertex_color(shade, crate::pack::flags::SHADE_MINUS_ENV),
            crate::psp_texture::pack_abgr([0xFF - 0x55, 0, 0, 0x44])
        );
        assert_eq!(colors.packed_prim(), Some(0x4433_2211));
        // An alpha formula without `PRIMITIVE` keeps the packed alpha; the
        // colour mapping is unchanged.
        let flags = crate::pack::flags::TEXTURE_BLEND | crate::pack::flags::ALPHA_NO_PRIM;
        assert_eq!(
            colors.vertex_color(original, flags),
            crate::psp_texture::pack_abgr([0x55, 0x66, 0x77, 4])
        );
    }

    /// A three-deep chain with a rotation, a translation and a scale at each
    /// level, so composition order and every component are exercised.
    fn chain() -> SceneGraph {
        let node = |depth: u32, parent, t: [f32; 3], r: [f32; 3], s: [f32; 3]| DObjNode {
            desc: DObjDesc {
                id: depth,
                dl: None,
                translate: t,
                rotate: r,
                scale: s,
            },
            parent,
        };
        SceneGraph {
            offset: 0x100,
            nodes: alloc::vec![
                node(
                    0,
                    None,
                    [10.0, 20.0, 30.0],
                    [0.1, 0.2, 0.3],
                    [1.0, 1.0, 1.0]
                ),
                node(
                    1,
                    Some(0),
                    [5.0, 0.0, -2.0],
                    [0.0, 0.4, 0.0],
                    [2.0, 1.0, 1.0]
                ),
                node(
                    2,
                    Some(1),
                    [0.0, 7.5, 0.0],
                    [-0.3, 0.0, 0.6],
                    [1.0, 0.5, 1.0]
                ),
            ],
        }
    }

    fn packed(graph: &SceneGraph) -> alloc::vec::Vec<u8> {
        let mut w = PackWriter::new();
        w.add_object(graph, 296, |_| None, &[]);
        w.finish()
    }

    #[test]
    fn item_composition_ejects_the_placeholder_and_keeps_descendant_transforms() {
        let bytes = packed(&chain());
        let pack = Pack::open(&bytes).unwrap();
        let object = pack.object(0).unwrap();
        let root = JointPose {
            translate: [0.0; 3],
            rotate: [0.0; 3],
            scale: [1.5; 3],
        };
        let mut out = [Mat4::IDENTITY; 3];
        assert_eq!(
            StageAnimator::default().compose_item(&pack, &object, root, &mut out),
            3
        );
        assert_eq!(out[0], Mat4::IDENTITY);
        assert_eq!(out[1], Mat4::from_trs_display([0.0; 3], [0.0; 3], [1.5; 3]));
        let child = pack.node(object.first_node + 2).unwrap();
        let expected = out[1].mul(&Mat4::from_trs_display(
            child.rest_translate.map(|t| t / MODEL_SCALE),
            child.rest_rotate,
            child.rest_scale,
        ));
        assert_eq!(out[2], expected);
        assert!(StageAnimator::default().compose(&pack, &object, &mut [Mat4::IDENTITY; 3]) > 0);
        assert_ne!(out[1].0, pack.node(object.first_node + 1).unwrap().world);
    }

    #[test]
    fn composing_the_rest_pose_reproduces_the_baked_matrices() {
        // The load-bearing claim: the runtime path and the build-time path
        // agree. If they do not, an animated object would visibly jump the
        // moment it started animating, even on frame 0 of an animation that
        // moves nothing.
        let graph = chain();
        let bytes = packed(&graph);
        let pack = crate::pack::Pack::open(&bytes).unwrap();
        let object = pack.object(0).unwrap();

        let mut out = [Mat4::IDENTITY; MAX_NODES];
        let n = Skeleton::new().compose(&pack, &object, &mut out);
        assert_eq!(n, 3);

        for (i, composed) in out.iter().take(3).enumerate() {
            let baked = pack.node(i as u32).unwrap().world;
            for (k, (&got, &want)) in composed.0.iter().zip(baked.iter()).enumerate() {
                assert!(
                    (got - want).abs() < 1e-5,
                    "node {i} element {k}: composed {got} vs baked {want}"
                );
            }
        }
    }

    #[test]
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
    fn a_joint_transform_moves_its_children_too() {
        // What makes it a skeleton rather than a list of transforms: rotating
        // a parent has to carry the leaf with it.
        let graph = chain();
        let bytes = packed(&graph);
        let pack = crate::pack::Pack::open(&bytes).unwrap();
        let object = pack.object(0).unwrap();

        let mut rest = [Mat4::IDENTITY; MAX_NODES];
        Skeleton::new().compose(&pack, &object, &mut rest);

        // Drive node 1 (the middle of the chain) by hand.
        let mut sk = Skeleton::new();
        sk.joint_count = 1;
        sk.nodes[0] = object.first_node + 1;
        sk.poses[0] = JointPose {
            rotate: [0.0, 1.2, 0.0],
            translate: [5.0, 0.0, -2.0],
            scale: [2.0, 1.0, 1.0],
        };
        let mut posed = [Mat4::IDENTITY; MAX_NODES];
        sk.compose(&pack, &object, &mut posed);

        assert_eq!(posed[0], rest[0], "the root was not driven");
        assert_ne!(posed[1], rest[1], "the driven joint moved");
        assert_ne!(posed[2], rest[2], "and so did its child");
    }

    #[test]
    fn starting_an_animation_seeds_every_joint_from_its_node_rest_pose() {
        // A figatree names only the tracks it moves. The others must come from
        // the model, not from whatever the previous animation left behind.
        let graph = chain();
        let mut w = PackWriter::new();
        w.add_object(&graph, 296, |_| None, &[]);
        // One joint on node 2, with no script at all.
        w.add_anim(0, 0, 504, 10, &[0u8; 16], &[(None, Some(2))]);
        let bytes = w.finish();
        let pack = crate::pack::Pack::open(&bytes).unwrap();
        let anim = pack.anim(0).unwrap();

        let mut sk = Skeleton::new();
        sk.speed = 1.0;
        sk.start(&pack, &anim, 0.0, 1.0);
        assert_eq!(sk.joint_count(), 1);
        let pose = sk.pose(0).unwrap();
        assert_eq!(pose.translate, [0.0, 7.5, 0.0]);
        assert_eq!(pose.rotate, [-0.3, 0.0, 0.6]);
        assert_eq!(pose.scale, [1.0, 0.5, 1.0]);

        // And with no script it stays there, however long it is ticked.
        let script = pack.anim_script(&anim).unwrap();
        for _ in 0..30 {
            sk.tick(script).unwrap();
        }
        assert_eq!(sk.pose(0).unwrap().translate, [0.0, 7.5, 0.0]);
        assert!(sk.ended(), "a skeleton with no live script has ended");
    }

    /// Two joints on nodes 1 and 2: the first blocks `a` frames, the second
    /// `b`, both then End.
    fn two_joint_clip(a: u16, b: u16) -> alloc::vec::Vec<u8> {
        const BLOCK: u16 = (1 << 11) | 1;
        let words = [BLOCK, a, 0, BLOCK, b, 0];
        let script: alloc::vec::Vec<u8> = words.iter().flat_map(|w| w.to_be_bytes()).collect();
        let graph = chain();
        let mut w = PackWriter::new();
        w.add_object(&graph, 296, |_| None, &[]);
        w.add_anim(
            0,
            0,
            504,
            10,
            &script,
            &[(Some(0), Some(1)), (Some(6), Some(2))],
        );
        w.finish()
    }

    fn frames(bytes: &[u8], start: f32, ticks: usize) -> alloc::vec::Vec<f32> {
        let pack = crate::pack::Pack::open(bytes).unwrap();
        let anim = pack.anim(0).unwrap();
        let script = pack.anim_script(&anim).unwrap();
        let mut sk = Skeleton::new();
        sk.start(&pack, &anim, start, 1.0);
        (0..ticks)
            .map(|_| {
                sk.tick(script).unwrap();
                sk.frame()
            })
            .collect()
    }

    #[test]
    fn the_last_joint_to_write_the_frame_in_a_tick_wins() {
        // `ftParamUpdateAnimKeys` parses joints in order and each writes the
        // fighter GObj's `anim_frame`. A shorter last joint ends on tick 3
        // and its End writes 0 over the first joint's 2.
        let bytes = two_joint_clip(4, 2);
        assert_eq!(frames(&bytes, 0.0, 7), [0.0, 1.0, 0.0, 3.0, 0.0, 0.0, 0.0]);
        // A shorter first joint is overwritten by the still-running second.
        let bytes = two_joint_clip(2, 4);
        assert_eq!(frames(&bytes, 0.0, 7), [0.0, 1.0, 2.0, 3.0, 0.0, 0.0, 0.0]);
    }

    #[test]
    fn after_the_end_the_frame_keeps_the_end_tick_wait() {
        // Started half a frame in, the clip ends at `anim_wait` -0.5 and the
        // GObj keeps it: no later tick writes, and it never reads 0.
        let bytes = two_joint_clip(2, 2);
        let got = frames(&bytes, 0.5, 6);
        assert_eq!(got, [0.5, 1.5, -0.5, -0.5, -0.5, -0.5]);
    }

    #[test]
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
    fn animation_locks_freeze_masked_parts_and_queries_draw_the_collision_chain() {
        // RE-509, checked against N64 RDRAM traces of Yoshi and Samus.
        let graph = chain();
        let bytes = packed(&graph);
        let pack = crate::pack::Pack::open(&bytes).unwrap();
        let object = pack.object(0).unwrap();
        let pose = JointPose {
            rotate: [0.4, -1.1, 0.25],
            translate: [3.0, -2.0, 1.5],
            scale: [1.0, 1.0, 1.0],
        };
        let mut sk = Skeleton::new();
        sk.joint_count = 3;
        for i in 0..3 {
            sk.nodes[i] = object.first_node + i as u32;
            sk.poses[i] = pose;
        }
        sk.locks.root_scale = [1.1; 3];
        let t = pose.translate.map(|v| v / MODEL_SCALE);
        let rest = pack.node_rest(object.first_node + 1).unwrap();
        let rest_t = rest.translate.map(|v| v / MODEL_SCALE);
        let mut out = [Mat4::IDENTITY; MAX_NODES];
        let local =
            |out: &[Mat4; MAX_NODES], i: usize| out[i - 1].inverse_affine().unwrap().mul(&out[i]);
        let close = |a: &Mat4, b: &Mat4| {
            a.0.iter()
                .zip(b.0.iter())
                .all(|(x, y)| (x - y).abs() < 1e-4)
        };

        // A normal motion: the masked middle part is drawn at its rest pose,
        // through the collision matrix, whatever its joint does.
        sk.locks.mask = 0b010;
        sk.compose(&pack, &object, &mut out);
        let frozen = Mat4::from_trs_collision(rest_t, rest.rotate, rest.scale).f2l_fixed_w();
        assert!(close(&local(&out, 1), &frozen));
        assert!(close(
            &local(&out, 2),
            &Mat4::from_trs_display(t, pose.rotate, pose.scale)
        ));

        // A flagged motion animates it, without the children's scale.
        sk.locks.flagged = true;
        sk.compose(&pack, &object, &mut out);
        let inv = Mat4::from_trs_scale_inv_display(t, pose.rotate, [1.1; 3], [1.1; 3]);
        assert!(close(&local(&out, 1), &inv));
        assert!(close(&local(&out, 2), &inv));

        // A query of the leaf marks it and its ancestors; the marked parts
        // draw from their `gmCollisionSetMatrixNcs` locals.
        sk.locks.queried = 0b111;
        sk.compose(&pack, &object, &mut out);
        let (ncs, _) = Mat4::from_trs_ncs(t, pose.rotate, pose.scale, Some([1.1; 3]));
        assert!(close(&local(&out, 1), &ncs.f2l_fixed_w()));
        assert!(close(&local(&out, 2), &ncs.f2l_fixed_w()));
        assert!(
            !close(&ncs.f2l_fixed_w(), &inv),
            "the test tells the two apart"
        );
    }

    #[test]
    fn an_unanimated_skeleton_composes_the_same_matrices_as_no_skeleton() {
        // The static path and the animated path must not disagree about an
        // object nothing is driving; the viewer draws both kinds.
        let graph = chain();
        let bytes = packed(&graph);
        let pack = crate::pack::Pack::open(&bytes).unwrap();
        let object = pack.object(0).unwrap();

        let mut a = [Mat4::IDENTITY; MAX_NODES];
        let mut b = [Mat4::IDENTITY; MAX_NODES];
        Skeleton::new().compose(&pack, &object, &mut a);
        Skeleton::default().compose(&pack, &object, &mut b);
        assert_eq!(a, b);
    }

    #[test]
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
    fn a_stage_joint_transform_moves_an_unanimated_child() {
        // `gcAddAnimJointAll` permits a NULL child script. The child remains
        // attached to its DObj parent and must inherit that parent's motion.
        let graph = chain();
        let bytes = packed(&graph);
        let pack = crate::pack::Pack::open(&bytes).unwrap();
        let object = pack.object(0).unwrap();

        let mut rest = [Mat4::IDENTITY; MAX_NODES];
        StageAnimator::new().compose(&pack, &object, &mut rest);
        for (i, matrix) in rest.iter().take(object.node_count as usize).enumerate() {
            assert_eq!(
                matrix.0,
                pack.node(object.first_node + i as u32).unwrap().world
            );
        }

        let mut animator = StageAnimator::new();
        animator.count = 1;
        animator.nodes[0] = object.first_node + 1;
        animator.poses[0] = JointPose {
            rotate: [0.0, 1.2, 0.0],
            translate: [5.0, 0.0, -2.0],
            scale: [2.0, 1.0, 1.0],
        };
        let mut posed = [Mat4::IDENTITY; MAX_NODES];
        animator.compose(&pack, &object, &mut posed);

        assert_eq!(posed[0], rest[0], "the root was not driven");
        assert_ne!(posed[1], rest[1], "the driven stage joint moved");
        assert_ne!(
            posed[2], rest[2],
            "its child must inherit motion without its own script"
        );
    }

    #[test]
    fn stage_flags_hide_mesh_or_subtree_without_hiding_the_parent() {
        let bytes = packed(&chain());
        let pack = crate::pack::Pack::open(&bytes).unwrap();
        let mut animator = StageAnimator::new();
        animator.count = 1;
        animator.nodes[0] = 1;
        animator.joints[0].flags = 1;
        assert!(animator.visible(&pack, 0));
        assert!(!animator.visible(&pack, 1));
        assert!(animator.visible(&pack, 2));
        animator.joints[0].flags = 2;
        assert!(animator.visible(&pack, 0));
        assert!(!animator.visible(&pack, 1));
        assert!(!animator.visible(&pack, 2));
        animator.joints[0].flags = 0;
        assert!(animator.visible(&pack, 2));
    }

    #[test]
    fn stage_billboard_scales_keep_sign_and_use_ancestor_x_for_both_axes() {
        let graph = chain();
        let bytes = packed(&graph);
        let pack = crate::pack::Pack::open(&bytes).unwrap();
        let object = pack.object(0).unwrap();
        let mut animator = StageAnimator::new();
        animator.count = 1;
        animator.nodes[0] = object.first_node + 1;
        animator.poses[0] = JointPose {
            rotate: [0.0; 3],
            translate: [0.0; 3],
            scale: [-2.0, 3.0, 4.0],
        };

        let mut scales = [[0.0; 2]; MAX_NODES];
        assert_eq!(animator.billboard_scales(&pack, &object, &mut scales), 3);
        assert_eq!(scales[0], [1.0, 1.0]);
        assert_eq!(scales[1], [-2.0, 3.0]);
        // Node 2's own rest scale is [1, 0.5, 1]. Both axes inherit the
        // parent's signed X (-2), not its Y (3).
        assert_eq!(scales[2], [-2.0, -1.0]);
    }

    #[test]
    fn posing_a_stage_twice_gives_the_same_pose() {
        // The battle poses a layer once and draws both task heads with that
        // pose (`StagePoseCache`, RE-480): posing must read nothing but the
        // animator and the pack, and posing again must write the same values.
        let graph = chain();
        let bytes = packed(&graph);
        let pack = crate::pack::Pack::open(&bytes).unwrap();
        let object = pack.object(0).unwrap();
        let mut animator = StageAnimator::new();
        animator.count = 1;
        animator.nodes[0] = object.first_node + 1;
        animator.poses[0] = JointPose {
            rotate: [0.3, 1.2, -0.7],
            translate: [5.0, -1.0, -2.0],
            scale: [-2.0, 3.0, 4.0],
        };

        let mut first = [Mat4::IDENTITY; MAX_NODES];
        let mut second = [Mat4([9.0; 16]); MAX_NODES];
        let n = animator.compose(&pack, &object, &mut first);
        assert_eq!(animator.compose(&pack, &object, &mut second), n);
        assert_eq!(first[..n], second[..n]);

        let mut first_scales = [[0.0; 2]; MAX_NODES];
        let mut second_scales = [[9.0; 2]; MAX_NODES];
        let s = animator.billboard_scales(&pack, &object, &mut first_scales);
        assert_eq!(
            animator.billboard_scales(&pack, &object, &mut second_scales),
            s
        );
        assert_eq!(s, n);
        assert_eq!(first_scales[..s], second_scales[..s]);
    }

    fn mat_script(words: &[u32]) -> alloc::vec::Vec<u8> {
        words.iter().flat_map(|w| w.to_be_bytes()).collect()
    }

    const fn mat_cmd(opcode: u32, flags: u32, payload: u32) -> u32 {
        (opcode << 25) | (flags << 15) | payload
    }

    /// RE-086/RE-087's real archive shape: `PaletteID` steps through three
    /// values, then loops forever via `SET_ANIM` rather than ending.
    fn palette_cycle_script() -> alloc::vec::Vec<u8> {
        const OP_SET_VAL_AFTER_BLOCK: u32 = 10;
        const OP_SET_ANIM: u32 = 14;
        const TRACK_PALETTE_ID: u32 = crate::matanim::TRACK_PALETTE_ID as u32;
        mat_script(&[
            mat_cmd(OP_SET_VAL_AFTER_BLOCK, 1 << TRACK_PALETTE_ID, 0),
            0.0f32.to_bits(),
            mat_cmd(OP_SET_VAL_AFTER_BLOCK, 1 << TRACK_PALETTE_ID, 2),
            1.0f32.to_bits(),
            mat_cmd(OP_SET_VAL_AFTER_BLOCK, 1 << TRACK_PALETTE_ID, 2),
            2.0f32.to_bits(),
            // Real loops close on the start value (e.g. file 105's): the
            // payload-0 step after `SET_ANIM` would otherwise replace `2`
            // on the frame it becomes due.
            mat_cmd(OP_SET_VAL_AFTER_BLOCK, 1 << TRACK_PALETTE_ID, 2),
            0.0f32.to_bits(),
            mat_cmd(OP_SET_ANIM, 0, 0),
            0, // jump target: this script's own start
        ])
    }

    fn packed_mat_anim() -> (alloc::vec::Vec<u8>, u32) {
        use crate::psp_texture::{Psm, PspTexture};

        let mut w = PackWriter::new();
        let tex = PspTexture {
            width: 32,
            height: 8,
            stride: 32,
            format: Psm::PsmT4,
            data: alloc::vec![0xCDu8; 128],
            swizzled: true,
            palette: alloc::vec![0xFF00_00FFu32; 16],
            levels: 1,
        };
        let texture = w.add_texture(&tex, false, false);
        let file_bytes = palette_cycle_script();
        let palettes = alloc::vec![
            alloc::vec![0x1111_1111u32; 16],
            alloc::vec![0x2222_2222u32; 16],
            alloc::vec![0x3333_3333u32; 16],
        ];
        let mat_anim = w.add_mat_anim(
            105,
            &file_bytes,
            0,
            0x1000,
            &palettes,
            &[],
            [0; 10],
            0,
            [0; 3],
        );
        w.set_texture_mat_anim(texture, mat_anim);
        (w.finish(), mat_anim)
    }

    #[test]
    fn material_animator_ticks_a_real_palette_cycle_to_its_resolved_variant() {
        let (bytes, mat_anim) = packed_mat_anim();
        let pack = crate::pack::Pack::open(&bytes).unwrap();
        let a = pack.mat_anim(mat_anim).unwrap();

        let mut m = MaterialAnimator::new();
        m.start(&pack);

        // `MaterialJoint`'s own exact per-tick timing (when a `_AFTER_BLOCK`
        // step's target actually becomes visible) is already pinned by
        // `matanim::tick_tests`; this test is about `MaterialAnimator`
        // wrapping and resolving it correctly, not re-deriving that timing.
        // `SET_ANIM` loops back rather than ending, so ticking well past the
        // script's own length must keep cycling through every real variant,
        // never stall, and never resolve outside this entry's own three.
        let mut seen = alloc::collections::BTreeSet::new();
        for _ in 0..30 {
            m.tick(&pack);
            if let Some(v) = m.resolved_palette(&pack, mat_anim) {
                assert!(
                    (a.first_palette..a.first_palette + a.palette_count).contains(&v),
                    "resolved variant must stay within this entry's own range"
                );
                seen.insert(v);
            }
        }
        assert_eq!(
            seen,
            alloc::collections::BTreeSet::from([
                a.first_palette,
                a.first_palette + 1,
                a.first_palette + 2
            ]),
            "a real script visits every one of its variants and loops, it does not freeze"
        );
    }

    /// A Zebes-acid-shaped stepped cycle: `PaletteID` 1 after 6 frames,
    /// back to 0 after 6 more, looping (`Jump` to the start).
    fn six_tick_flip_pack() -> (alloc::vec::Vec<u8>, u32) {
        const OP_SET_VAL_AFTER_BLOCK: u32 = 10;
        const OP_JUMP: u32 = 1;
        const TRACK_PALETTE_ID: u32 = crate::matanim::TRACK_PALETTE_ID as u32;
        let file_bytes = mat_script(&[
            mat_cmd(OP_SET_VAL_AFTER_BLOCK, 1 << TRACK_PALETTE_ID, 6),
            1.0f32.to_bits(),
            mat_cmd(OP_SET_VAL_AFTER_BLOCK, 1 << TRACK_PALETTE_ID, 6),
            0.0f32.to_bits(),
            mat_cmd(OP_JUMP, 0, 0),
            0,
        ]);
        let mut w = PackWriter::new();
        let palettes = alloc::vec![
            alloc::vec![0x1111_1111u32; 16],
            alloc::vec![0x2222_2222u32; 16]
        ];
        let mat_anim = w.add_mat_anim(
            157,
            &file_bytes,
            0,
            0x8D8,
            &palettes,
            &[],
            [0; 10],
            0,
            [0; 3],
        );
        (w.finish(), mat_anim)
    }

    /// `PaletteID` the joint holds after `updates` battle updates.
    fn palette_after(m: &MaterialAnimator, mat_anim: u32) -> f32 {
        let j = m.joint(mat_anim).unwrap();
        j.track_value(crate::matanim::TRACK_PALETTE_ID).unwrap()
    }

    #[test]
    fn start_stage_plays_the_creation_call_so_update_n_is_n64_update_n() {
        // N64: `gcAddAnimAll` + the maker's `gcPlayAnimAll` (length 0),
        // then update `n` reads length `n`: the value flips at updates 6,
        // 12, 18, ... and `anim_frame == n`.
        let (bytes, mat_anim) = six_tick_flip_pack();
        let pack = crate::pack::Pack::open(&bytes).unwrap();
        let mut fixed = MaterialAnimator::new();
        fixed.start_stage(&pack);
        assert_eq!(palette_after(&fixed, mat_anim), 0.0);
        let mut boot = MaterialAnimator::new();
        boot.start(&pack);
        let mut late = 0;
        for n in 1..=36u32 {
            fixed.tick(&pack);
            boot.tick(&pack);
            let n64 = ((n / 6) % 2) as f32;
            assert_eq!(palette_after(&fixed, mat_anim), n64, "update {n}");
            assert_eq!(fixed.joint(mat_anim).unwrap().anim_frame(), n as f32);
            // Without the creation call every flip lands one update late.
            if palette_after(&boot, mat_anim) != n64 {
                late += 1;
                assert_eq!(n % 6, 0, "update {n}");
            }
        }
        assert_eq!(
            late, 6,
            "the unfixed clock trails by one update at every flip"
        );
    }

    #[test]
    fn start_stage_restarts_the_clock_whatever_ran_before() {
        // A second battle's ground is made fresh: its phase must not carry
        // the previous battle's (or the menus') ticks.
        let (bytes, mat_anim) = six_tick_flip_pack();
        let pack = crate::pack::Pack::open(&bytes).unwrap();
        let mut fresh = MaterialAnimator::new();
        fresh.start_stage(&pack);
        let mut reused = MaterialAnimator::new();
        reused.start_stage(&pack);
        for _ in 0..7 {
            reused.tick(&pack);
        }
        reused.start_stage(&pack);
        for n in 1..=24u32 {
            fresh.tick(&pack);
            reused.tick(&pack);
            assert_eq!(
                palette_after(&reused, mat_anim),
                palette_after(&fresh, mat_anim),
                "update {n}"
            );
            assert_eq!(reused.joint(mat_anim).unwrap().anim_frame(), n as f32);
        }
    }

    #[test]
    fn material_animator_clamps_a_value_outside_its_own_variant_count() {
        // Defence against reading into a neighbouring MatAnimDesc's own
        // variants in the shared palette table: a script producing an
        // out-of-range PaletteID must clamp into *this* entry's own count,
        // not silently index past it.
        let mut w = PackWriter::new();
        use crate::psp_texture::{Psm, PspTexture};
        let tex = PspTexture {
            width: 32,
            height: 8,
            stride: 32,
            format: Psm::PsmT4,
            data: alloc::vec![0xCDu8; 128],
            swizzled: true,
            palette: alloc::vec![0xFF00_00FFu32; 16],
            levels: 1,
        };
        let texture = w.add_texture(&tex, false, false);
        const OP_SET_VAL_AFTER_BLOCK: u32 = 10;
        const OP_END: u32 = 0;
        const TRACK_PALETTE_ID: u32 = crate::matanim::TRACK_PALETTE_ID as u32;
        let file_bytes = mat_script(&[
            mat_cmd(OP_SET_VAL_AFTER_BLOCK, 1 << TRACK_PALETTE_ID, 0),
            9.0f32.to_bits(), // far past this entry's own 2 variants
            mat_cmd(OP_END, 0, 0),
        ]);
        let palettes = alloc::vec![
            alloc::vec![0x1111_1111u32; 16],
            alloc::vec![0x2222_2222u32; 16]
        ];
        let mat_anim = w.add_mat_anim(
            105,
            &file_bytes,
            0,
            0x1000,
            &palettes,
            &[],
            [0; 10],
            0,
            [0; 3],
        );
        w.set_texture_mat_anim(texture, mat_anim);
        let bytes = w.finish();
        let pack = crate::pack::Pack::open(&bytes).unwrap();
        let a = pack.mat_anim(mat_anim).unwrap();

        let mut m = MaterialAnimator::new();
        m.start(&pack);
        m.tick(&pack);
        assert_eq!(
            m.resolved_palette(&pack, mat_anim),
            Some(a.first_palette + 1),
            "clamped to the last real variant, not read past it"
        );
    }

    /// A tile-0 window at rest at the origin with unit scale.
    fn window(mode: u32, width: f32) -> MaterialUv {
        MaterialUv {
            trau: 0.0,
            trav: 0.0,
            scau: 1.0,
            scav: 1.0,
            base_trau: 0.0,
            base_trav: 0.0,
            base_scau: 1.0,
            base_scav: 1.0,
            mode,
            tile_bias: 0.0,
            tile_width: width,
            tile_height: width,
            half: [0.0; 2],
        }
    }

    #[test]
    fn the_live_tile_origin_truncates_to_a_quarter_texel() {
        // `uls = ((128 * 0.0301) / 1) * 4 = 15.41` is an `s32`: 15.
        let uv = MaterialUv {
            trau: 0.0301,
            ..window(2, 128.0)
        };
        let a = uv.ge_affine((128, 128), (false, false));
        assert_eq!(a.offset_s, -(15.0 / 4.0) / 128.0);
        assert_eq!((a.scale_s, a.offset_t), (1.0, 0.0));
    }

    #[test]
    fn the_tile_origin_wraps_in_its_twelve_bit_field() {
        // 128 * 10 * 4 = 5120 quarter texels; `gDPSetTileSize` keeps 1024.
        let uv = MaterialUv {
            trau: 10.0,
            ..window(2, 128.0)
        };
        let a = uv.ge_affine((128, 128), (false, false));
        assert_eq!(a.offset_s, -256.0 / 128.0);
    }

    #[test]
    fn the_offset_is_in_the_uploaded_texture_s_units() {
        // A 24-texel tile uploaded 32 wide: 12 texels are 12/32, not 1/2.
        let uv = MaterialUv {
            trau: 0.5,
            ..window(2, 24.0)
        };
        let a = uv.ge_affine((32, 32), (false, false));
        assert_eq!(a.offset_s, -12.0 / 32.0);
    }

    #[test]
    fn every_axis_restores_the_origin_the_packer_baked_out() {
        // Rest origin 2 texels, live 3, scale doubled by `gSPTexture`:
        // `(x + 2) * 2 - 3 = 2x + 1`.
        let base = MaterialUv {
            base_trau: 2.0 / 128.0,
            trau: 3.0 / 128.0 * 0.5,
            scau: 0.5,
            ..window(2 | 4, 128.0)
        };
        let a = base.ge_affine((128, 128), (true, false));
        assert_eq!(a.scale_s, 2.0);
        assert_eq!(a.offset_s, 1.0 / 128.0);
        // Repeating coordinates have the same rest origin baked out.
        let a = base.ge_affine((128, 128), (false, false));
        assert_eq!(a.offset_s, 1.0 / 128.0);
    }

    #[test]
    fn scale_moves_only_with_the_mobj_s_g_texture() {
        let uv = MaterialUv {
            scau: 0.5,
            ..window(2, 128.0)
        };
        assert_eq!(uv.ge_affine((128, 128), (false, false)).scale_s, 1.0);
        let uv = MaterialUv { mode: 2 | 4, ..uv };
        assert_eq!(uv.ge_affine((128, 128), (false, false)).scale_s, 2.0);
    }

    /// RE-327: the resolver's `unk10 == 1` window lands where `mobj::read`
    /// puts the static one (`mobj::tests::unk10_one_halves_the_u_windows_and_scale`,
    /// same inputs): tile 0 about `unk24`, tile 1 about `unk44`.
    #[test]
    fn unk10_one_halves_the_window_as_the_static_read_does() {
        let half = u32::from(crate::mobj::UV_MODE_HALF);
        let tile0 = MaterialUv {
            trau: 0.25,
            base_trau: 0.25,
            tile_width: 64.0,
            tile_height: 32.0,
            half: [0.125, 0.5],
            ..window(1 | 4 | half, 64.0)
        };
        assert_eq!(tile0.tile_origin([0.25, 0.0], [1.0, 1.0]), [224, 0]);
        let tile1 = MaterialUv {
            half: [0.0625, 0.5],
            ..tile0
        };
        assert_eq!(tile1.tile_origin([0.25, 0.0], [1.0, 1.0]), [240, 0]);
        // Without the bit, the unhalved window.
        let plain = MaterialUv {
            mode: 1 | 4,
            ..tile0
        };
        assert_eq!(plain.tile_origin([0.25, 0.0], [1.0, 1.0]), [64, 0]);
        // The halving scales rest and live alike: `k` is their ratio.
        let live = MaterialUv { scau: 0.5, ..tile0 };
        assert_eq!(live.ge_affine((64, 32), (false, false)).scale_s, 2.0);
        // The halving bit alone owns no window.
        assert!(tile0.owned_by(0).is_none());
    }

    #[test]
    fn a_window_keeps_only_the_state_its_mobj_owns() {
        use crate::pack::flags::{SCALE_ANIM, TILE0_ANIM};
        let uv = window(1 | 4, 32.0);
        assert_eq!(uv.owned_by(TILE0_ANIM | SCALE_ANIM).unwrap().mode, 5);
        assert_eq!(uv.owned_by(TILE0_ANIM).unwrap().mode, 1);
        assert_eq!(uv.owned_by(SCALE_ANIM).unwrap().mode, 4);
        assert!(uv.owned_by(0).is_none());
    }

    #[test]
    fn a_linear_palette_hold_selects_its_truncated_index() {
        // File 114's pond scripts open with `SetValBlock(PaletteID, 0)` held
        // for 90 frames. `gcDrawMObjForDObj` reads `palettes[(s32)
        // palette_id]` whatever the kind, so the hold draws palette 0, and a
        // linear ramp through 1.75 draws palette 1.
        let mut w = PackWriter::new();
        const OP_SET_VAL_BLOCK: u32 = 3;
        const OP_END: u32 = 0;
        const TRACK_PALETTE_ID: u32 = crate::matanim::TRACK_PALETTE_ID as u32;
        let file_bytes = mat_script(&[
            mat_cmd(OP_SET_VAL_BLOCK, 1 << TRACK_PALETTE_ID, 0),
            0.0f32.to_bits(),
            mat_cmd(OP_SET_VAL_BLOCK, 1 << TRACK_PALETTE_ID, 4),
            2.0f32.to_bits(),
            mat_cmd(OP_END, 0, 0),
        ]);
        let palettes = alloc::vec![alloc::vec![0u32; 16]; 3];
        let mat_anim = w.add_mat_anim(
            105,
            &file_bytes,
            0,
            0x1000,
            &palettes,
            &[],
            [0; 10],
            0,
            [0; 3],
        );
        let bytes = w.finish();
        let pack = crate::pack::Pack::open(&bytes).unwrap();
        let a = pack.mat_anim(mat_anim).unwrap();

        let mut m = MaterialAnimator::new();
        m.start(&pack);
        let mut got = alloc::vec::Vec::new();
        for _ in 0..4 {
            m.tick(&pack);
            got.push(
                m.resolved_palette(&pack, mat_anim)
                    .map(|p| p - a.first_palette),
            );
        }
        // 0.0, 0.5, 1.0, 1.5 per frame.
        assert_eq!(got, [Some(0), Some(0), Some(1), Some(1)]);
    }

    #[test]
    fn material_animator_loops_texture_frames_and_keeps_its_palette() {
        const OP_SET_VAL_AFTER_BLOCK: u32 = 10;
        const OP_SET_ANIM: u32 = 14;
        const TEXTURE: u32 = crate::matanim::TRACK_TEXTURE_ID_CURRENT as u32;
        const PALETTE: u32 = crate::matanim::TRACK_PALETTE_ID as u32;
        use crate::psp_texture::{Psm, PspTexture};

        let mut w = PackWriter::new();
        let image = |byte| PspTexture {
            width: 16,
            height: 8,
            stride: 16,
            format: Psm::PsmT4,
            data: alloc::vec![byte; 64],
            swizzled: true,
            palette: alloc::vec![0xFF00_00FF; 16],
            levels: 1,
        };
        let frame0 = w.add_texture(&image(0x11), false, false);
        let frame1 = w.add_texture(&image(0x22), false, false);
        let script = mat_script(&[
            mat_cmd(OP_SET_VAL_AFTER_BLOCK, (1 << TEXTURE) | (1 << PALETTE), 1),
            0.0f32.to_bits(),
            0.0f32.to_bits(),
            mat_cmd(OP_SET_VAL_AFTER_BLOCK, (1 << TEXTURE) | (1 << PALETTE), 1),
            1.0f32.to_bits(),
            1.0f32.to_bits(),
            mat_cmd(OP_SET_ANIM, 0, 0),
            0,
        ]);
        let palettes = alloc::vec![alloc::vec![0x1111_1111; 16], alloc::vec![0x2222_2222; 16],];
        let anim = w.add_mat_anim(
            104,
            &script,
            0,
            0x1200,
            &palettes,
            &[frame0, frame1],
            [0; 10],
            0,
            [0; 3],
        );
        let pack_bytes = w.finish();
        let pack = Pack::open(&pack_bytes).unwrap();
        let mut animator = MaterialAnimator::new();
        animator.start(&pack);
        let mut pairs = alloc::collections::BTreeSet::new();
        for _ in 0..12 {
            animator.tick(&pack);
            pairs.insert((
                animator.resolved_texture(&pack, anim),
                animator.resolved_palette(&pack, anim),
            ));
        }
        let a = pack.mat_anim(anim).unwrap();
        assert!(pairs.contains(&(Some(frame0), Some(a.first_palette))));
        assert!(pairs.contains(&(Some(frame1), Some(a.first_palette + 1))));
    }

    /// A `SetLFrac`/`TextureIDNext` script (RE-321): three steps that move
    /// the fraction and swap which image is current and which is next.
    fn lod_blend_pack() -> (alloc::vec::Vec<u8>, u32, [u32; 2], [u32; 2]) {
        use crate::psp_texture::{Psm, PspTexture};
        const OP_SET_VAL_AFTER_BLOCK: u32 = 10;
        const OP_WAIT: u32 = 2;
        const CURRENT: u32 = crate::matanim::TRACK_TEXTURE_ID_CURRENT as u32;
        const NEXT: u32 = crate::matanim::TRACK_TEXTURE_ID_NEXT as u32;
        const LFRAC: u32 = crate::matanim::TRACK_SET_LFRAC as u32;
        const SCRV: u32 = crate::matanim::TRACK_SCR_V as u32;
        let tracks = (1 << CURRENT) | (1 << NEXT) | (1 << LFRAC);
        let script = mat_script(&[
            // Values follow in ascending track order.
            mat_cmd(OP_SET_VAL_AFTER_BLOCK, tracks, 3),
            0.0f32.to_bits(),
            1.0f32.to_bits(),
            0.45f32.to_bits(),
            mat_cmd(OP_SET_VAL_AFTER_BLOCK, tracks | (1 << SCRV), 3),
            1.0f32.to_bits(),
            0.0f32.to_bits(),
            (-0.5f32).to_bits(),
            0.6f32.to_bits(),
            mat_cmd(OP_WAIT, 0, 50),
        ]);
        let image = |byte| PspTexture {
            width: 32,
            height: 32,
            stride: 32,
            format: Psm::PsmT4,
            data: alloc::vec![byte; 512],
            swizzled: true,
            palette: alloc::vec![0xFFFF_FFFF; 16],
            levels: 1,
        };
        let mut w = PackWriter::new();
        let current = [
            w.add_texture(&image(0x11), true, true),
            w.add_texture(&image(0x22), true, true),
        ];
        let next = [
            w.add_texture(&image(0x33), true, true),
            w.add_texture(&image(0x44), true, true),
        ];
        let mut base = [0u32; 10];
        base[1] = 0.0066f32.to_bits();
        base[3] = 2.0f32.to_bits();
        base[4] = 4.0f32.to_bits();
        base[6] = 0.0066f32.to_bits();
        base[8] = (115.0f32 / 255.0).to_bits();
        let anim = w.add_mat_anim(
            104,
            &script,
            0,
            0x1F78,
            &[],
            &current,
            base,
            1,
            [384, 128, 128],
        );
        let mut next_textures =
            [crate::pack::TextureDesc::NO_ANIM; crate::pack::MatAnimDesc::MAX_TEXTURES];
        next_textures[..2].copy_from_slice(&next);
        assert!(w.add_lod_blend(crate::pack::LodBlendDesc {
            mat_anim: anim,
            next_count: 2,
            next_textures,
            tile1_params: [384, 128, 128],
            origin_delta: [0, 0],
        }));
        (w.finish(), anim, current, next)
    }

    #[test]
    fn lod_blend_follows_the_fraction_and_both_texture_ids_on_the_same_tick() {
        let (bytes, anim, current, next) = lod_blend_pack();
        let pack = Pack::open(&bytes).unwrap();
        let mut animator = MaterialAnimator::new();
        animator.start(&pack);

        // Before any tick the fraction is the MObjSub's `prim_l`, and
        // `texture_id_next` its zeroed start.
        let rest = animator.resolved_lod_blend(&pack, anim).unwrap();
        assert_eq!(rest.frac, crate::lod_blend::prim_lod_frac(115.0 / 255.0));
        assert_eq!(rest.texture, next[0]);
        assert_eq!(rest.uv, None);

        // Every tick, the current texture, the next texture and the fraction
        // resolve from one joint state: no pair of them is ever a tick apart.
        let mut seen = alloc::vec::Vec::new();
        for _ in 0..8 {
            animator.tick(&pack);
            let lod = animator.resolved_lod_blend(&pack, anim).unwrap();
            let cur = animator.resolved_texture(&pack, anim).unwrap();
            let state = (cur, lod.texture, lod.frac);
            if seen.last() != Some(&state) {
                seen.push(state);
            }
        }
        // `matanim::tick_tests` pins when an `_AFTER_BLOCK` step lands; before
        // the first one every track still reads its start value.
        assert!(
            seen.ends_with(&[(current[0], next[1], 114), (current[1], next[0], 153)]),
            "the fraction animates and current/next swap together: {seen:?}"
        );
    }

    #[test]
    fn lod_blend_moves_tile_one_by_its_own_scroll_tracks() {
        let (bytes, anim, ..) = lod_blend_pack();
        let pack = Pack::open(&bytes).unwrap();
        let mut animator = MaterialAnimator::new();
        animator.start(&pack);
        let mut uv = None;
        for _ in 0..8 {
            animator.tick(&pack);
            uv = animator.resolved_lod_blend(&pack, anim).unwrap().uv.or(uv);
        }
        let uv = uv.expect("ScrV moves tile 1");
        assert_eq!(uv.trav, -0.5, "ScrV drives tile 1, not TraV");
        assert_eq!(uv.trau, 0.0066, "an undriven ScrU keeps its rest value");
        assert_eq!((uv.scau, uv.scav), (2.0, 4.0));
        assert_eq!(uv.mode & 3, 1, "tile 1 has only the normal window form");
        assert_eq!(
            (uv.tile_bias, uv.tile_width, uv.tile_height),
            (384.0, 128.0, 128.0)
        );
        assert!(
            animator.resolved_uv(&pack, anim).is_none(),
            "tile 0 has no UV track here, so its window stays at rest"
        );
    }

    /// RE-321: tiles whose static origins differ (Master Hand's Fall
    /// wallpaper, tile 0 at T 128 and tile 1 at T 0 quarter texels): the
    /// second pass samples tile 1 at the packed coordinate plus tile 0's
    /// origin less tile 1's, scaled with the coordinate when the window
    /// scale moves.
    #[test]
    fn lod_blend_rebases_a_tile_one_origin_that_differs() {
        let rest = LodBlendState {
            texture: 0,
            frac: 128,
            uv: None,
            origin_delta: [0, 128],
        };
        let a = rest.ge_affine((32, 64), (true, true));
        assert_eq!((a.scale_s, a.scale_t), (1.0, 1.0));
        assert_eq!(a.offset_s, 0.0);
        // 32 texels on a 64-texel GE axis.
        assert_eq!(a.offset_t, 0.5);
        let (bytes, anim, ..) = lod_blend_pack();
        let pack = Pack::open(&bytes).unwrap();
        let mut animator = MaterialAnimator::new();
        animator.start(&pack);
        let mut uv = None;
        for _ in 0..8 {
            animator.tick(&pack);
            uv = animator.resolved_lod_blend(&pack, anim).unwrap().uv.or(uv);
        }
        let uv = uv.unwrap();
        let live = LodBlendState {
            uv: Some(uv),
            ..rest
        };
        let window = uv.ge_affine((128, 128), (true, true));
        let a = live.ge_affine((128, 128), (true, true));
        assert_eq!(a.scale_t, window.scale_t);
        let want = window.offset_t + 32.0 * window.scale_t / 128.0;
        assert!((a.offset_t - want).abs() < 1e-6, "{a:?} {window:?}");
        assert_eq!(a.offset_s, window.offset_s);
    }

    #[test]
    fn lod_blend_is_absent_without_a_record() {
        let (bytes, mat_anim) = packed_mat_anim();
        let pack = Pack::open(&bytes).unwrap();
        let mut animator = MaterialAnimator::new();
        animator.start(&pack);
        animator.tick(&pack);
        assert!(animator.resolved_lod_blend(&pack, mat_anim).is_none());
    }

    #[test]
    fn material_animator_uses_rest_uv_for_untouched_tracks_and_resets() {
        const OP_SET_VAL_AFTER_BLOCK: u32 = 10;
        const OP_WAIT: u32 = 2;
        const TRAU: u32 = crate::matanim::TRACK_TRA_U as u32;
        let script = mat_script(&[
            mat_cmd(OP_SET_VAL_AFTER_BLOCK, 1 << TRAU, 1),
            0.5f32.to_bits(),
            mat_cmd(OP_WAIT, 0, 10),
        ]);
        let mut base = [0u32; 10];
        base[1] = 0.25f32.to_bits();
        base[2] = 0.125f32.to_bits();
        base[3] = 1.0f32.to_bits();
        base[4] = 0.5f32.to_bits();
        let mut w = PackWriter::new();
        let anim = w.add_mat_anim(104, &script, 0, 0x2200, &[], &[], base, 1, [2, 64, 32]);
        let bytes = w.finish();
        let pack = Pack::open(&bytes).unwrap();
        let mut animator = MaterialAnimator::new();
        animator.start(&pack);
        // Frame 1 holds the step's zero base; its target is due on frame 2.
        animator.tick(&pack);
        animator.tick(&pack);
        let uv = animator.resolved_uv(&pack, anim).unwrap();
        assert_eq!(uv.trau, 0.5);
        assert_eq!(uv.trav, 0.125);
        assert_eq!(uv.scau, 1.0);
        assert_eq!(uv.scav, 0.5);
        assert_eq!(
            (uv.tile_bias, uv.tile_width, uv.tile_height),
            (2.0, 64.0, 32.0)
        );
        animator.start(&pack);
        assert!(
            animator.resolved_uv(&pack, anim).is_none(),
            "restart clears live track state"
        );
    }

    #[test]
    fn effect_uv_window_uses_its_restart_clock() {
        const SET_AFTER: u32 = 10;
        const WAIT: u32 = 2;
        const TRAU: u32 = crate::matanim::TRACK_TRA_U as u32;
        let script = mat_script(&[
            mat_cmd(SET_AFTER, 1 << TRAU, 1),
            0.5f32.to_bits(),
            mat_cmd(WAIT, 0, 20),
        ]);
        let mut base = [0u32; 10];
        base[1] = 0.25f32.to_bits();
        base[3] = 1.0f32.to_bits();
        base[4] = 1.0f32.to_bits();
        let mut writer = PackWriter::new();
        let anim = writer.add_mat_anim(333, &script, 0, 0x100, &[], &[], base, 1, [0, 64, 32]);
        let bytes = writer.finish();
        let pack = Pack::open(&bytes).unwrap();

        let mut stage = MaterialAnimator::new();
        stage.start(&pack);
        for _ in 0..5 {
            stage.tick(&pack);
        }
        assert_eq!(stage.resolved_uv(&pack, anim).unwrap().trau, 0.5);

        let mut effect = EffectMaterialAnimator::new();
        effect.start(&pack, core::iter::once(anim));
        assert!(effect.resolved_uv(&pack, anim).is_none());
        effect.tick(&pack);
        assert_ne!(effect.resolved_uv(&pack, anim).map(|uv| uv.trau), Some(0.5));
        effect.tick(&pack);
        assert_eq!(effect.resolved_uv(&pack, anim).unwrap().trau, 0.5);

        effect.start(&pack, core::iter::once(anim));
        assert!(effect.resolved_uv(&pack, anim).is_none());
        assert_eq!(stage.resolved_uv(&pack, anim).unwrap().trau, 0.5);
    }

    #[test]
    fn effect_material_animator_restarts_at_frame_zero_on_start() {
        // The whole point of a per-effect player over `MaterialAnimator`'s
        // pack-lifetime clock (RE-175): calling `start` again must replay
        // the script from its own beginning, not continue from wherever a
        // prior run had ticked to. Duration 5 on the second step (not
        // `palette_cycle_script`'s 2) leaves enough margin that one tick
        // provably has not switched yet -- see
        // `matanim::tick_tests::a_palette_step_switches_after_its_payload_frames`
        // for the same margin choice and why a too-small duration can
        // already resolve the second step within the very first tick.
        // A plain `Wait`/`End` rather than `palette_cycle_script`'s looping
        // `SET_ANIM`, so seven ticks cannot land back on variant 0 by
        // having already wrapped around.
        const OP_SET_VAL_AFTER_BLOCK: u32 = 10;
        const OP_WAIT: u32 = 2;
        const OP_END: u32 = 0;
        const TRACK_PALETTE_ID: u32 = crate::matanim::TRACK_PALETTE_ID as u32;
        let mut w = PackWriter::new();
        let file_bytes = mat_script(&[
            mat_cmd(OP_SET_VAL_AFTER_BLOCK, 1 << TRACK_PALETTE_ID, 0),
            0.0f32.to_bits(),
            mat_cmd(OP_SET_VAL_AFTER_BLOCK, 1 << TRACK_PALETTE_ID, 5),
            1.0f32.to_bits(),
            mat_cmd(OP_WAIT, 0, 97),
            mat_cmd(OP_END, 0, 0),
        ]);
        let palettes = alloc::vec![
            alloc::vec![0x1111_1111u32; 16],
            alloc::vec![0x2222_2222u32; 16],
        ];
        let mat_anim = w.add_mat_anim(
            105,
            &file_bytes,
            0,
            0x1000,
            &palettes,
            &[],
            [0; 10],
            0,
            [0; 3],
        );
        let bytes = w.finish();
        let pack = crate::pack::Pack::open(&bytes).unwrap();
        let a = pack.mat_anim(mat_anim).unwrap();

        let mut m = EffectMaterialAnimator::new();
        m.start(&pack, core::iter::once(mat_anim));
        for _ in 0..7 {
            m.tick(&pack);
        }
        assert_ne!(
            m.resolved_palette(&pack, mat_anim),
            Some(a.first_palette),
            "seven ticks in, this should have moved off variant 0"
        );

        m.start(&pack, core::iter::once(mat_anim));
        assert_eq!(
            m.resolved_palette(&pack, mat_anim),
            None,
            "freshly restarted, not yet ticked at all"
        );
        m.tick(&pack);
        assert_eq!(
            m.resolved_palette(&pack, mat_anim),
            Some(a.first_palette),
            "restarted at frame zero, the same as a brand new script"
        );
    }

    #[test]
    fn effect_material_animator_resolves_a_sprite_frame_list() {
        // CommonSpark/Poké Ball's own shape (RE-175): `TextureIDCurrent`
        // stepping through a list of sprite frames, resolved into
        // `MatAnimDesc::textures[]` the same way `resolved_palette` resolves
        // `PaletteID` into the palette table. Duration 5 on the second step
        // for the same "provably not switched after one tick" margin as
        // `effect_material_animator_restarts_at_frame_zero_on_start`.
        const OP_SET_VAL_AFTER_BLOCK: u32 = 10;
        const OP_END: u32 = 0;
        const TRACK_TEXTURE_ID_CURRENT: u32 = crate::matanim::TRACK_TEXTURE_ID_CURRENT as u32;
        let mut w = PackWriter::new();
        let file_bytes = mat_script(&[
            mat_cmd(OP_SET_VAL_AFTER_BLOCK, 1 << TRACK_TEXTURE_ID_CURRENT, 0),
            0.0f32.to_bits(),
            mat_cmd(OP_SET_VAL_AFTER_BLOCK, 1 << TRACK_TEXTURE_ID_CURRENT, 5),
            1.0f32.to_bits(),
            mat_cmd(OP_END, 0, 0),
        ]);
        let textures = alloc::vec![7u32, 9u32];
        let mat_anim = w.add_mat_anim(
            83,
            &file_bytes,
            0,
            0x90C0,
            &[],
            &textures,
            [0; 10],
            0,
            [0; 3],
        );
        let bytes = w.finish();
        let pack = crate::pack::Pack::open(&bytes).unwrap();

        let mut m = EffectMaterialAnimator::new();
        m.start(&pack, core::iter::once(mat_anim));
        m.tick(&pack);
        assert_eq!(
            m.resolved_texture(&pack, mat_anim),
            Some(7),
            "not yet switched"
        );
        for _ in 0..5 {
            m.tick(&pack);
        }
        assert_eq!(m.resolved_texture(&pack, mat_anim), Some(9));
    }

    #[test]
    fn effect_material_animator_resolves_link_spin_attacks_colour_ramp() {
        // Verbatim shape from `353_LinkSpecial2.c`'s
        // `SpinAttackMatAnimJoint_MatAnimJoint_data`: authored frame-zero
        // primitive alpha of zero, ramping to visible -- the same script
        // `matanim::tick_tests::link_spin_attack_primitive_alpha_ramps_up_from_zero`
        // pins numerically, exercised here through the object-scoped player
        // an audit/renderer actually calls.
        const OP_EXT_VAL_BLOCK: u32 = 20;
        const OP_WAIT: u32 = 2;
        const OP_END: u32 = 0;
        const TRACK_PRIM: u32 = crate::matanim::TRACK_PRIM as u32;
        let mut w = PackWriter::new();
        let file_bytes = mat_script(&[
            mat_cmd(OP_EXT_VAL_BLOCK, 1 << TRACK_PRIM, 0),
            0xFFFF_6000,
            mat_cmd(OP_EXT_VAL_BLOCK, 1 << TRACK_PRIM, 12),
            0xFFFF_60CC,
            mat_cmd(OP_WAIT, 0, 97),
            mat_cmd(OP_END, 0, 0),
        ]);
        let mat_anim = w.add_mat_anim(353, &file_bytes, 0, 0x12F0, &[], &[], [0; 10], 0, [0; 3]);
        let bytes = w.finish();
        let pack = crate::pack::Pack::open(&bytes).unwrap();

        let mut m = EffectMaterialAnimator::new();
        m.start(&pack, core::iter::once(mat_anim));
        assert_eq!(
            m.resolved_colors(mat_anim),
            Some(EffectColors::default()),
            "not yet ticked: nothing resolved"
        );
        m.tick(&pack);
        let prim = m
            .resolved_colors(mat_anim)
            .and_then(|c| c.prim)
            .expect("a live ramp");
        assert_eq!(prim, [255, 255, 96, 0], "frame 1 is the authored start");
        m.tick(&pack);
        let prim = m.resolved_colors(mat_anim).and_then(|c| c.prim).unwrap();
        assert!(prim[3] > 0, "measurably visible on frame 2: {prim:?}");
    }

    #[test]
    fn material_animator_ticks_every_entry_the_pack_defines() {
        // RE-322: the v33 pack has 103 `MatAnimDesc`s; a fixed 64-slot
        // array left the last 39 frozen, Race to the Finish among them.
        const OP_EXT_VAL_BLOCK: u32 = 20;
        const OP_WAIT: u32 = 2;
        const TRACK_PRIM: u32 = crate::matanim::TRACK_PRIM as u32;
        let mut w = PackWriter::new();
        let mut last = 0;
        for i in 0..103 {
            let file_bytes = mat_script(&[
                mat_cmd(OP_EXT_VAL_BLOCK, 1 << TRACK_PRIM, 0),
                0xFFFF_FF00 | i,
                mat_cmd(OP_WAIT, 0, 100),
                0,
            ]);
            last = w.add_mat_anim(500 + i, &file_bytes, 0, 0, &[], &[], [0; 10], 0, [0; 3]);
        }
        let bytes = w.finish();
        let pack = crate::pack::Pack::open(&bytes).unwrap();
        let mut m = MaterialAnimator::new();
        m.start(&pack);
        assert_eq!(m.len(), 103);
        // Frame 1 shows the zero base; frame 2 the payload-0 key.
        m.tick(&pack);
        m.tick(&pack);
        let prim = m.resolved_colors(last).and_then(|c| c.prim);
        assert_eq!(prim, Some([0xFF, 0xFF, 0xFF, 102]), "the last entry ticks");
        assert_eq!(m.resolved_colors(103), None, "past the pack's own count");
    }

    #[test]
    fn effect_material_animator_deduplicates_and_caps_its_slots() {
        let mut w = PackWriter::new();
        let mut indices = alloc::vec::Vec::new();
        for i in 0..MAX_EFFECT_MAT_ANIMS as u32 + 3 {
            let file_bytes = mat_script(&[0]);
            indices.push(w.add_mat_anim(200 + i, &file_bytes, 0, 0, &[], &[], [0; 10], 0, [0; 3]));
        }
        let bytes = w.finish();
        let pack = crate::pack::Pack::open(&bytes).unwrap();

        let mut m = EffectMaterialAnimator::new();
        // Every index twice: duplicates must collapse to one slot each
        // rather than starving the real cap.
        let doubled = indices.iter().copied().chain(indices.iter().copied());
        m.start(&pack, doubled);
        assert_eq!(m.count, MAX_EFFECT_MAT_ANIMS, "capped, not wrapped");
        let distinct: alloc::collections::BTreeSet<_> =
            m.slots[..m.count].iter().map(|&(i, _)| i).collect();
        assert_eq!(
            distinct.len(),
            MAX_EFFECT_MAT_ANIMS,
            "every slot is a distinct script, duplicates did not waste one"
        );
    }
}

/// The kept-locals and table-driven composes against the compose as it was
/// before them, bit for bit.
#[cfg(test)]
mod compose_equivalence_tests {
    use super::*;
    use crate::scene::{DObjDesc, DObjNode, SceneGraph};

    /// `compose_with` before the joint table, the kept locals and the
    /// locals-only pass, kept verbatim as the reference.
    fn reference_compose(
        sk: &Skeleton,
        pack: &Pack<'_>,
        object: &ObjectDesc,
        out: &mut [Mat4],
        space: Space,
        queried: Option<(u64, bool)>,
        mut locals_out: Option<(&mut [Mat4], &mut Option<Mat4>)>,
    ) -> usize {
        let count = (object.node_count as usize).min(out.len()).min(MAX_NODES);
        let mut locks = sk.locks;
        if let Some((nodes, lead)) = queried {
            locks.queried = nodes;
            locks.queried_lead = lead;
        }
        let mut scales = [[1.0f32; 3]; MAX_NODES];
        let queried = |i: usize| i < 64 && locks.queried >> i & 1 != 0;
        let (lead, root_scale) = match sk.lead_xrotn_pose() {
            Some(p) => {
                let (m, s) = match space {
                    Space::Display if locks.queried_lead => {
                        let (m, s) = sk.local(p, locks.root_scale, Space::Collision);
                        (m.f2l_fixed_w(), s)
                    }
                    _ => sk.local(p, locks.root_scale, space),
                };
                (Some(m), s)
            }
            None => (None, locks.root_scale),
        };
        if let Some((_, lead_out)) = locals_out.as_mut() {
            **lead_out = lead;
        }
        for i in 0..count {
            let global = object.first_node + i as u32;
            let Some(parent) = pack.node_parent(global) else {
                out[i] = Mat4::IDENTITY;
                continue;
            };
            let parent_index = match parent {
                p if p != NodeDesc::NO_PARENT
                    && p >= object.first_node
                    && (p - object.first_node) < i as u32 =>
                {
                    Some((p - object.first_node) as usize)
                }
                _ => None,
            };
            let parent_scale = parent_index.map_or(root_scale, |p| scales[p]);
            let local = if !locks.flagged && i < 64 && locks.mask >> i & 1 != 0 {
                let rest = pack.node_rest(global).unwrap_or_default();
                let (m, _) = sk.local(&rest, parent_scale, Space::Collision);
                scales[i] = rest.scale;
                match space {
                    Space::Display => m.f2l_fixed_w(),
                    Space::Collision => m,
                }
            } else {
                let rest;
                let pose = match sk.pose_for(global) {
                    Some(pose) => pose,
                    None => {
                        rest = pack.node_rest(global).unwrap_or_default();
                        &rest
                    }
                };
                let (m, s) = match space {
                    Space::Display if queried(i) => {
                        let (m, s) = sk.local(pose, parent_scale, Space::Collision);
                        (m.f2l_fixed_w(), s)
                    }
                    _ => sk.local(pose, parent_scale, space),
                };
                scales[i] = s;
                m
            };
            if let Some((locals, _)) = locals_out.as_mut() {
                if let Some(slot) = locals.get_mut(i) {
                    *slot = local;
                }
            }
            out[i] = match (parent, parent_index) {
                (_, Some(p)) => out[p].mul(&local),
                (NodeDesc::NO_PARENT, None) => lead.map_or(local, |lead| lead.mul(&local)),
                _ => local,
            };
        }
        count
    }

    /// `collision_parts` before the change, on the reference compose.
    #[allow(clippy::type_complexity)]
    fn reference_parts(
        sk: &Skeleton,
        pack: &Pack<'_>,
        object: &ObjectDesc,
    ) -> (
        usize,
        [[[f32; 3]; 4]; MAX_NODES],
        [f32; MAX_NODES],
        Option<([[f32; 3]; 4], f32)>,
    ) {
        let mut world = [Mat4::IDENTITY; MAX_NODES];
        let mut local = [Mat4::IDENTITY; MAX_NODES];
        let mut lead_local = None;
        let count = reference_compose(
            sk,
            pack,
            object,
            &mut world,
            Space::Collision,
            None,
            Some((&mut local, &mut lead_local)),
        );
        let rows = |m: &Mat4| -> [[f32; 3]; 4] {
            let r = |i: usize| [m.0[i * 4], m.0[i * 4 + 1], m.0[i * 4 + 2]];
            let t = r(3);
            [r(0), r(1), r(2), t.map(|v| v * MODEL_SCALE)]
        };
        let mut locals = [[[0.0f32; 3]; 4]; MAX_NODES];
        let mut translate_x = [0.0f32; MAX_NODES];
        for i in 0..count {
            locals[i] = rows(&local[i]);
            let global = object.first_node + i as u32;
            translate_x[i] = match sk.pose_for(global) {
                Some(p) => p.translate[0],
                None => pack.node_rest(global).unwrap_or_default().translate[0],
            };
        }
        let lead = lead_local.map(|m| {
            (
                rows(&m),
                sk.lead_xrotn_pose().map_or(0.0, |p| p.translate[0]),
            )
        });
        (count, locals, translate_x, lead)
    }

    fn bits(m: &[Mat4]) -> alloc::vec::Vec<[u32; 16]> {
        m.iter().map(|m| m.0.map(f32::to_bits)).collect()
    }

    fn rows_bits(r: &[[[f32; 3]; 4]]) -> alloc::vec::Vec<[[u32; 3]; 4]> {
        r.iter()
            .map(|m| m.map(|row| row.map(f32::to_bits)))
            .collect()
    }

    /// Every compose this file offers against the reference, on `sk` as it
    /// stands.
    fn check(sk: &Skeleton, pack: &Pack<'_>, object: &ObjectDesc, what: &str) {
        let mut want = [Mat4::IDENTITY; MAX_NODES];
        let mut got = [Mat4::IDENTITY; MAX_NODES];
        // Display, with the skeleton's own queried set and with another.
        for queried in [
            None,
            Some((0x5555_5555_5555_5555u64, true)),
            Some((0, false)),
        ] {
            let n = reference_compose(sk, pack, object, &mut want, Space::Display, queried, None);
            let m = match queried {
                None => sk.compose(pack, object, &mut got),
                Some((q, l)) => sk.compose_queried(pack, object, &mut got, q, l),
            };
            assert_eq!(n, m, "{what}");
            assert_eq!(
                bits(&want[..n]),
                bits(&got[..n]),
                "{what} display {queried:?}"
            );
        }
        // Collision.
        let n = reference_compose(sk, pack, object, &mut want, Space::Collision, None, None);
        assert_eq!(sk.compose_collision(pack, object, &mut got), n, "{what}");
        assert_eq!(bits(&want[..n]), bits(&got[..n]), "{what} collision");
        // The parts, plain and kept, and the world from the kept locals.
        let (rn, rlocals, rx, rlead) = reference_parts(sk, pack, object);
        let mut locals = [[[0.0f32; 3]; 4]; MAX_NODES];
        let mut tx = [0.0f32; MAX_NODES];
        let mut lead = None;
        assert_eq!(
            sk.collision_parts(pack, object, &mut locals, &mut tx, &mut lead),
            rn
        );
        assert_eq!(
            rows_bits(&rlocals[..rn]),
            rows_bits(&locals[..rn]),
            "{what} parts"
        );
        assert_eq!(
            rx[..rn].map_or_bits(),
            tx[..rn].map_or_bits(),
            "{what} translate_x"
        );
        assert_eq!(
            rlead.map(|(r, x)| (r.map(|row| row.map(f32::to_bits)), x.to_bits())),
            lead.map(|(r, x)| (r.map(|row| row.map(f32::to_bits)), x.to_bits())),
            "{what} lead"
        );
        let mut kept = CollisionLocals::new();
        // Stale contents must not leak into the next keep.
        kept.local = [Mat4::from_trs([1.0; 3], [0.5; 3], [2.0; 3]); MAX_NODES];
        kept.link = [7; MAX_NODES];
        let mut locals2 = [[[0.0f32; 3]; 4]; MAX_NODES];
        let mut tx2 = [0.0f32; MAX_NODES];
        let mut lead2 = None;
        let k =
            sk.collision_parts_keep(pack, object, &mut kept, &mut locals2, &mut tx2, &mut lead2);
        assert_eq!(k, rn);
        assert_eq!(
            rows_bits(&locals[..rn]),
            rows_bits(&locals2[..rn]),
            "{what} kept parts"
        );
        let mut from = [Mat4::IDENTITY; MAX_NODES];
        let f = Skeleton::compose_collision_from(&kept, &mut from);
        assert_eq!(f, n, "{what}");
        assert_eq!(
            bits(&want[..n]),
            bits(&from[..f]),
            "{what} world from kept locals"
        );
    }

    trait Bits {
        fn map_or_bits(&self) -> alloc::vec::Vec<u32>;
    }
    impl Bits for [f32] {
        fn map_or_bits(&self) -> alloc::vec::Vec<u32> {
            self.iter().map(|v| v.to_bits()).collect()
        }
    }

    struct Lcg(u32);
    impl Lcg {
        fn next(&mut self) -> u32 {
            self.0 = self.0.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            self.0 >> 8
        }
        fn f(&mut self, lo: f32, hi: f32) -> f32 {
            lo + (hi - lo) * (self.next() as f32 / (1u32 << 24) as f32)
        }
        fn pose(&mut self) -> JointPose {
            JointPose {
                translate: [
                    self.f(-80.0, 80.0),
                    self.f(-80.0, 80.0),
                    self.f(-80.0, 80.0),
                ],
                rotate: [self.f(-7.0, 7.0), self.f(-7.0, 7.0), self.f(-7.0, 7.0)],
                scale: [self.f(0.5, 1.6), self.f(0.5, 1.6), self.f(0.5, 1.6)],
            }
        }
    }

    /// A branching tree: two roots, a node whose parent comes later
    /// (left in its own space) and chains three deep.
    fn tree() -> SceneGraph {
        let node = |id: u32, parent| DObjNode {
            desc: DObjDesc {
                id,
                dl: None,
                translate: [id as f32 * 3.0, 1.5, -(id as f32)],
                rotate: [0.1 * id as f32, -0.2, 0.05 * id as f32],
                scale: [1.0, 1.0 + 0.1 * id as f32, 1.0],
            },
            parent,
        };
        let parents = [
            None,
            Some(0),
            Some(1),
            Some(1),
            Some(0),
            None,
            Some(5),
            Some(6),
            Some(3),
            Some(8),
        ];
        SceneGraph {
            offset: 0x100,
            nodes: parents
                .iter()
                .enumerate()
                .map(|(i, &p)| node(i as u32, p))
                .collect(),
        }
    }

    fn randomise(sk: &mut Skeleton, rng: &mut Lcg, first_node: u32, nodes: u32, lead: bool) {
        sk.joint_count = 0;
        if lead {
            sk.nodes[0] = AnimJoint::NO_NODE;
            sk.poses[0] = rng.pose();
            sk.joint_count = 1;
        }
        sk.lead_xrotn = lead;
        for n in 0..nodes {
            if sk.joint_count == MAX_JOINTS {
                break;
            }
            // Leave some nodes undriven, so their rest pose is read.
            if rng.next().is_multiple_of(4) {
                continue;
            }
            let j = sk.joint_count;
            sk.nodes[j] = first_node + n;
            sk.poses[j] = rng.pose();
            sk.joint_count += 1;
        }
        // A second joint on an already driven node: the first one wins.
        if sk.joint_count > 2 && sk.joint_count < MAX_JOINTS {
            let j = sk.joint_count;
            sk.nodes[j] = sk.nodes[sk.joint_count - 1];
            sk.poses[j] = rng.pose();
            sk.joint_count += 1;
        }
        // And one on a node of no object.
        if sk.joint_count < MAX_JOINTS {
            let j = sk.joint_count;
            sk.nodes[j] = first_node + 1000;
            sk.poses[j] = rng.pose();
            sk.joint_count += 1;
        }
        sk.locks = AnimLocks {
            mask: (u64::from(rng.next()) << 32 | u64::from(rng.next())) & 0x3_FFFF,
            flagged: rng.next().is_multiple_of(2),
            root_scale: [rng.f(0.8, 1.7); 3],
            queried: u64::from(rng.next()) << 32 | u64::from(rng.next()),
            queried_lead: rng.next().is_multiple_of(2),
        };
    }

    #[test]
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
    fn every_compose_matches_the_reference_on_random_poses() {
        let graph = tree();
        let mut w = crate::pack::PackWriter::new();
        w.add_object(&graph, 296, |_| None, &[]);
        let bytes = w.finish();
        let pack = Pack::open(&bytes).unwrap();
        let whole = pack.object(0).unwrap();
        // The object as packed; one starting a node in, whose first node's
        // parent lies outside it; one running past the node table, whose
        // last nodes have no entry at all.
        let objects = [
            whole,
            ObjectDesc {
                first_node: whole.first_node + 1,
                node_count: whole.node_count - 1,
                ..whole
            },
            ObjectDesc {
                first_node: whole.first_node + 4,
                node_count: whole.node_count,
                ..whole
            },
        ];
        let mut rng = Lcg(0x5EED);
        for round in 0..600 {
            let object = objects[round % 3];
            let mut sk = Skeleton::new();
            randomise(
                &mut sk,
                &mut rng,
                object.first_node,
                object.node_count,
                round % 4 < 2,
            );
            if round % 5 == 0 {
                sk.locks = AnimLocks::default();
            }
            check(&sk, &pack, &object, &alloc::format!("round {round}"));
        }
    }

    #[test]
    fn the_joint_table_answers_as_pose_for_does() {
        let mut rng = Lcg(77);
        for _ in 0..200 {
            let mut sk = Skeleton::new();
            let first = rng.next() % 300;
            let lead = rng.0.is_multiple_of(2);
            randomise(&mut sk, &mut rng, first, 50, lead);
            // Joints outside the object around it, too.
            if sk.joint_count + 2 <= MAX_JOINTS {
                sk.nodes[sk.joint_count] = first.wrapping_sub(1);
                sk.nodes[sk.joint_count + 1] = first + MAX_NODES as u32;
                sk.joint_count += 2;
            }
            let table = sk.joint_table(first);
            for i in 0..MAX_NODES {
                let want = sk.pose_for(first + i as u32).map(|p| p as *const JointPose);
                let got = sk.pose_at(&table, i).map(|p| p as *const JointPose);
                assert_eq!(want, got, "node {i}");
            }
        }
    }

    fn real_pack() -> Option<alloc::vec::Vec<u8>> {
        ssb_tablegen::test_rom(env!("CARGO_MANIFEST_DIR"))?;
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../assets/generated/ssb64.pak");
        std::fs::read(path).ok()
    }

    /// Every fighter's model, through a spread of its own clips, each a few
    /// frames in, with and without locks and the lead `XRotN`.
    #[test]
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
    fn every_compose_matches_the_reference_on_every_fighter() {
        let Some(bytes) = real_pack() else { return };
        let pack = Pack::open(&bytes).unwrap();
        let mut checked = 0;
        for kind in 0..27u32 {
            let Some(index) = crate::scene_deps::fighter_object(&pack, kind) else {
                continue;
            };
            let object = pack.object(index).unwrap();
            let mut rng = Lcg(kind + 1);
            for slot in (0..700u32).step_by(23) {
                let Some(anim) = pack.fighter_anim(kind, slot) else {
                    continue;
                };
                let Some(script) = pack.anim_script(&anim) else {
                    continue;
                };
                let mut sk = Skeleton::new();
                sk.start(&pack, &anim, 0.0, 1.0);
                for _ in 0..(rng.next() % 9) {
                    let _ = sk.tick(script);
                }
                sk.lead_xrotn = rng.next().is_multiple_of(2);
                sk.locks = AnimLocks {
                    mask: if rng.next().is_multiple_of(2) {
                        0x1_C3F0
                    } else {
                        0
                    },
                    flagged: rng.next().is_multiple_of(3),
                    root_scale: [rng.f(0.8, 1.7); 3],
                    queried: u64::from(rng.next()) << 32 | u64::from(rng.next()),
                    queried_lead: rng.next().is_multiple_of(2),
                };
                check(
                    &sk,
                    &pack,
                    &object,
                    &alloc::format!("kind {kind} slot {slot}"),
                );
                // Every node's joint as `pose_for` finds it.
                let table = sk.joint_table(object.first_node);
                for i in 0..(object.node_count as usize).min(MAX_NODES) {
                    let want = sk
                        .pose_for(object.first_node + i as u32)
                        .map(|p| p as *const _);
                    let got = sk.pose_at(&table, i).map(|p| p as *const _);
                    assert_eq!(want, got, "kind {kind} slot {slot} node {i}");
                }
                checked += 1;
            }
        }
        assert!(checked > 100, "{checked}");
    }
}
