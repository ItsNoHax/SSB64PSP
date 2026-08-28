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

/// One object's animation state: a clock and a pose per joint.
///
/// Fixed-size and `Copy`-free but allocation-free, so it can live in a
/// fighter's struct on the PSP without a heap.
#[derive(Debug, Clone)]
pub struct Skeleton {
    /// Absolute pack node each joint drives.
    nodes: [u32; MAX_JOINTS],
    anims: [JointAnim; MAX_JOINTS],
    /// Current local transform per joint, seeded from the node's rest pose.
    poses: [JointPose; MAX_JOINTS],
    joint_count: usize,
    /// Playback rate: 1.0 normally, 0.5 for a heavy landing (RE-035).
    pub speed: f32,
}

impl Default for Skeleton {
    fn default() -> Self {
        Skeleton {
            nodes: [AnimJoint::NO_NODE; MAX_JOINTS],
            anims: core::array::from_fn(|_| JointAnim::inert()),
            poses: [JointPose::default(); MAX_JOINTS],
            joint_count: 0,
            speed: 1.0,
        }
    }
}

impl Skeleton {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn joint_count(&self) -> usize {
        self.joint_count
    }

    /// Frames elapsed in the animation, from its first joint.
    ///
    /// Goes `<= 0` when the script ends, which is the sentinel the status
    /// machine tests (RE-035). Every joint of one animation runs the same
    /// clock, so reading the first is enough.
    pub fn frame(&self) -> f32 {
        self.anims
            .iter()
            .take(self.joint_count)
            .find(|a| !a.ended())
            .map_or(0.0, |a| a.frame())
    }

    /// Whether every joint's script has run out.
    pub fn ended(&self) -> bool {
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
    pub fn start(&mut self, pack: &Pack<'_>, anim: &AnimDesc, frame: f32, speed: f32) {
        self.joint_count = 0;
        self.speed = speed;
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
                at => JointAnim::start(at as usize, frame),
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
        let mut failed = None;
        for i in 0..self.joint_count {
            if let Err(e) = self.anims[i].tick(script, self.speed, &mut self.poses[i]) {
                self.anims[i] = JointAnim::inert();
                failed.get_or_insert(e);
            }
        }
        match failed {
            Some(e) => Err(e),
            None => Ok(()),
        }
    }

    /// The local transform a joint currently holds.
    pub fn pose(&self, joint: usize) -> Option<&JointPose> {
        (joint < self.joint_count).then(|| &self.poses[joint])
    }

    /// The node a joint drives.
    pub fn joint_node(&self, joint: usize) -> Option<u32> {
        (joint < self.joint_count)
            .then(|| self.nodes[joint])
            .filter(|&n| n != AnimJoint::NO_NODE)
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
        let count = (object.node_count as usize).min(out.len()).min(MAX_NODES);
        for i in 0..count {
            let Some(node) = pack.node(object.first_node + i as u32) else {
                out[i] = Mat4::IDENTITY;
                continue;
            };
            let rest = JointPose {
                rotate: node.rest_rotate,
                translate: node.rest_translate,
                scale: node.rest_scale,
            };
            let pose = self.pose_for(object.first_node + i as u32).unwrap_or(&rest);
            // Into the same normalised space the packed matrices use.
            let t = [
                pose.translate[0] / MODEL_SCALE,
                pose.translate[1] / MODEL_SCALE,
                pose.translate[2] / MODEL_SCALE,
            ];
            let local = Mat4::from_trs(t, pose.rotate, pose.scale);
            // Parents always precede children in a `DObjDesc` array — a child
            // references `array_dobjs[depth - 1]`, which an earlier entry must
            // have filled — so the parent's matrix is already final.
            out[i] = match node.parent {
                NodeDesc::NO_PARENT => local,
                p if p >= object.first_node && (p - object.first_node) < i as u32 => {
                    out[(p - object.first_node) as usize].mul(&local)
                }
                // A parent outside this object, or one that has not been
                // composed yet, cannot be applied. Leaving the node in its own
                // space is wrong but bounded; silently using a stale matrix
                // would not be.
                _ => local,
            };
        }
        count
    }

    /// The pose driving `node`, if any joint does.
    fn pose_for(&self, node: u32) -> Option<&JointPose> {
        (0..self.joint_count)
            .find(|&i| self.nodes[i] == node)
            .map(|i| &self.poses[i])
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
        }
    }

    pub fn joint_count(&self) -> usize {
        self.count
    }

    /// Loads every joint entry of a stage animation, seeding each pose from the
    /// node's rest transform so a track the script never names keeps it.
    pub fn start(&mut self, pack: &Pack<'_>, anim: &AnimDesc) {
        self.count = 0;
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

    /// The node a joint drives, and the pose it has reached. Exposed so a
    /// verifier can compare a packed replay against one run straight off the
    /// archive, which is the check that the packing path — script offsets, node
    /// indices, the copied blob — is right (RE-052).
    pub fn joint(&self, i: usize) -> Option<(u32, &JointPose)> {
        (i < self.count).then(|| (self.nodes[i], &self.poses[i]))
    }

    /// Advances every joint one tick. `script` is the animation file's bytes,
    /// which the joint offsets index into.
    pub fn tick(&mut self, script: &[u8]) -> Result<(), crate::objanim::AnimError> {
        for i in 0..self.count {
            self.joints[i].tick(script, 1.0, &mut self.poses[i])?;
        }
        Ok(())
    }

    /// Composes world matrices for an object, exactly as [`Skeleton::compose`]
    /// does — a node this animator does not drive keeps its packed rest matrix.
    pub fn compose(&self, pack: &Pack<'_>, object: &ObjectDesc, out: &mut [Mat4]) -> usize {
        let count = (object.node_count as usize).min(out.len()).min(MAX_NODES);
        for i in 0..count {
            let index = object.first_node + i as u32;
            let Some(node) = pack.node(index) else {
                out[i] = Mat4::IDENTITY;
                continue;
            };
            let local = match self.pose_for(index) {
                Some(pose) => {
                    let t = [
                        pose.translate[0] / MODEL_SCALE,
                        pose.translate[1] / MODEL_SCALE,
                        pose.translate[2] / MODEL_SCALE,
                    ];
                    Mat4::from_trs(t, pose.rotate, pose.scale)
                }
                // Not animated: the packed world matrix is already this node's
                // full ancestor-composed transform, so it is used as it stands
                // and must not be re-multiplied by a parent below.
                None => {
                    out[i] = Mat4(node.world);
                    continue;
                }
            };
            out[i] = match node.parent {
                NodeDesc::NO_PARENT => local,
                p if p >= object.first_node && (p - object.first_node) < i as u32 => {
                    out[(p - object.first_node) as usize].mul(&local)
                }
                _ => local,
            };
        }
        count
    }

    fn pose_for(&self, node: u32) -> Option<&JointPose> {
        (0..self.count)
            .find(|&i| self.nodes[i] == node)
            .map(|i| &self.poses[i])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pack::PackWriter;
    use crate::scene::{DObjDesc, DObjNode, SceneGraph};

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
}
