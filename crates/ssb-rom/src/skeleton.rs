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

    /// Whether every active 32-bit event stream has reached its end marker.
    pub fn ended(&self) -> bool {
        self.joints[..self.count].iter().all(|joint| joint.ended())
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
    /// does. A node outside every animated ancestor chain keeps its packed
    /// matrix; an unanimated child of a moving joint is recomposed from rest.
    pub fn compose(&self, pack: &Pack<'_>, object: &ObjectDesc, out: &mut [Mat4]) -> usize {
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
            changed[i] = pose.is_some() || parent_local.is_some_and(|parent| changed[parent]);
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
            let pose = pose.unwrap_or(&rest);
            let t = [
                pose.translate[0] / MODEL_SCALE,
                pose.translate[1] / MODEL_SCALE,
                pose.translate[2] / MODEL_SCALE,
            ];
            let local = Mat4::from_trs(t, pose.rotate, pose.scale);
            out[i] = match parent_local {
                Some(parent) => out[parent].mul(&local),
                None => local,
            };
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
/// property of a *texture*, not a fighter or a stage layer — there is no
/// per-object "start" boundary to restart on. [`Self::start`] runs once,
/// when the pack loads, and [`Self::tick`] runs every frame after that for
/// as long as the pack is loaded, independent of which stage or fighter is
/// currently on screen (cheap either way: one short script per entry).
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

    /// How many `MatAnimDesc` entries this animator ticks.
    pub fn len(&self) -> usize {
        self.joints.len()
    }

    pub fn is_empty(&self) -> bool {
        self.joints.is_empty()
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

    /// The currently-resolved palette variant, as an absolute index into
    /// [`Pack::mat_anim_palette`] ready to hand to [`Pack::mat_anim_palette_data`]
    /// — or `None` if `mat_anim` names no tracked entry, its script never set
    /// `PaletteID`, or the current value did not arrive by a step (the only
    /// kind `PaletteID` can trust; see [`crate::matanim::MaterialJoint`]'s own
    /// doc comment).
    ///
    /// Clamped into the entry's own `palette_count` rather than trusting the
    /// script's raw value: a corrupted or out-of-range replay would otherwise
    /// read into a neighbouring `MatAnimDesc`'s own variants in the shared
    /// palette table instead of failing safely.
    pub fn resolved_palette(&self, pack: &Pack<'_>, mat_anim: u32) -> Option<u32> {
        let j = self.joints.get(mat_anim as usize)?;
        if !j.track_is_stepped(crate::matanim::TRACK_PALETTE_ID) {
            return None;
        }
        let v = j.track_value(crate::matanim::TRACK_PALETTE_ID)?;
        let a = pack.mat_anim(mat_anim)?;
        if a.palette_count == 0 {
            return None;
        }
        // `core` has no `round()` without `std`/`libm` (see `scene.rs`'s own
        // note on `sin`/`cos`); round-to-nearest for a non-negative value is
        // just "add a half, truncate" (the same trick `mesh.rs`'s vertex
        // rounding already uses).
        let index = ((v.max(0.0) + 0.5) as u32).min(a.palette_count - 1);
        Some(a.first_palette + index)
    }

    /// The current `TextureIDCurrent` sprite frame. This is deliberately not
    /// step-gated: the original assigns every live interpolation kind before
    /// truncating it to the `u16` texture index in the draw path.
    pub fn resolved_texture(&self, pack: &Pack<'_>, mat_anim: u32) -> Option<u32> {
        let j = self.joints.get(mat_anim as usize)?;
        let a = pack.mat_anim(mat_anim)?;
        let value = j.track_value(crate::matanim::TRACK_TEXTURE_ID_CURRENT)?;
        if a.texture_count == 0 {
            return None;
        }
        // `texture_id_curr` is a u16 in the original MObj, so its assignment
        // from the live float truncates toward zero.  PaletteID is different
        // (`f32`) and keeps its established round-to-nearest resolver.
        let index = (value.max(0.0) as u32).min(a.texture_count - 1);
        let texture = a.textures[index as usize];
        (texture != crate::pack::TextureDesc::NO_ANIM).then_some(texture)
    }

    /// Resolves `TraU`/`TraV`/`ScaU`/`ScaV`, retaining the MObjSub rest
    /// values for tracks the script leaves untouched. `ScrU`/`ScrV` target
    /// tile 1 and `SetLFrac` is a two-frame RDP blend; neither aliases these
    /// tile-0 coordinates (RE-086/RE-211); [`Self::resolved_lod_blend`] owns
    /// both (RE-321).
    pub fn resolved_uv(&self, pack: &Pack<'_>, mat_anim: u32) -> Option<MaterialUv> {
        let j = self.joints.get(mat_anim as usize)?;
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
        })
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
        use crate::matanim::{
            TRACK_SCA_U, TRACK_SCA_V, TRACK_SCR_U, TRACK_SCR_V, TRACK_SET_LFRAC,
            TRACK_TEXTURE_ID_NEXT,
        };
        let lod = pack.lod_blend(mat_anim)?;
        let a = pack.mat_anim(mat_anim)?;
        if lod.next_count == 0 {
            return None;
        }
        let live = |track: usize| {
            self.joints
                .get(mat_anim as usize)
                .and_then(|j| j.track_value(track))
        };
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
            // `G_TEXTURE` scale bit is shared with tile 0.
            mode: 1 | (a.uv_mode & 4),
            tile_bias: lod.tile1_params[0] as f32,
            tile_width: lod.tile1_params[1] as f32,
            tile_height: lod.tile1_params[2] as f32,
        });
        (texture != crate::pack::TextureDesc::NO_ANIM).then_some(LodBlendState {
            texture,
            frac: crate::lod_blend::prim_lod_frac(lfrac),
            uv,
        })
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

/// Distinct `MatAnimDesc` scripts one spawned effect's own primitives can
/// reference at once. Many primitives on one effect commonly share a script
/// (RE-175: Link Spin Attack's inner `AObjEvent32 *[5]` table names the same
/// script from every non-null slot), so this is sized by distinct scripts,
/// not primitives -- eight covers every manager-effect asset measured so
/// far with headroom, the same relationship [`crate::pack::MatAnimDesc::
/// MAX_TEXTURES`] has to its own measured maximum.
pub const MAX_EFFECT_MAT_ANIMS: usize = 8;

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
pub struct EffectMaterialAnimator {
    slots: [(u32, crate::matanim::MaterialJoint); MAX_EFFECT_MAT_ANIMS],
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
    /// colour. `flat_color` and `texture_blend` name the two source-backed
    /// combiner mappings established by RE-073/080; other RGB shapes retain
    /// their converter-resolved value. PRIM alpha is independent of RGB and
    /// replaces the packed vertex alpha whenever its track is live.
    pub fn vertex_color(self, packed: u32, flat_color: bool, texture_blend: bool) -> u32 {
        let mut rgba = packed.to_le_bytes();
        if flat_color {
            if let Some(prim) = self.prim {
                rgba[..3].copy_from_slice(&prim[..3]);
            } else if let Some(env) = self.env {
                rgba[..3].copy_from_slice(&env[..3]);
            }
        } else if texture_blend {
            if let Some(env) = self.env {
                rgba[..3].copy_from_slice(&env[..3]);
            }
        }
        if let Some(prim) = self.prim {
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
            self.slots[self.count] = (i, crate::matanim::MaterialJoint::start(script, 0.0));
            self.count += 1;
        }
    }

    /// Advances every tracked script one tick, each against its own source
    /// file's bytes (mirrors [`MaterialAnimator::tick`]).
    pub fn tick(&mut self, pack: &Pack<'_>) {
        for (i, j) in &mut self.slots[..self.count] {
            let Some(a) = pack.mat_anim(*i) else { continue };
            let Some(data) = pack.mat_anim_file(&a) else {
                continue;
            };
            let _ = j.tick(data, 1.0);
        }
    }

    fn joint(&self, mat_anim: u32) -> Option<&crate::matanim::MaterialJoint> {
        self.slots[..self.count]
            .iter()
            .find(|&&(i, _)| i == mat_anim)
            .map(|(_, j)| j)
    }

    /// The currently-resolved palette variant -- the same resolution rule as
    /// [`MaterialAnimator::resolved_palette`], just against this player's
    /// own restarted joints.
    pub fn resolved_palette(&self, pack: &Pack<'_>, mat_anim: u32) -> Option<u32> {
        let j = self.joint(mat_anim)?;
        if !j.track_is_stepped(crate::matanim::TRACK_PALETTE_ID) {
            return None;
        }
        let v = j.track_value(crate::matanim::TRACK_PALETTE_ID)?;
        let a = pack.mat_anim(mat_anim)?;
        if a.palette_count == 0 {
            return None;
        }
        let index = ((v.max(0.0) + 0.5) as u32).min(a.palette_count - 1);
        Some(a.first_palette + index)
    }

    /// The currently-resolved sprite variant, as an absolute pack texture
    /// index ready for [`crate::pack::Pack::texture`] -- `TextureIDCurrent`
    /// resolved into `MatAnimDesc::textures[]`, the sprite-table analogue of
    /// [`Self::resolved_palette`].
    pub fn resolved_texture(&self, pack: &Pack<'_>, mat_anim: u32) -> Option<u32> {
        // Not gated on `track_is_stepped` the way `resolved_palette` is:
        // `gcPlayMObjMatAnim` assigns `mobj->texture_id_curr` for any live
        // kind, and RE-175 measured real manager scripts (CommonSpark's own
        // UV/texture-id stream) driving it with a plain `Kind::Linear` ramp
        // rather than a `_After` step list.
        let j = self.joint(mat_anim)?;
        let v = j.track_value(crate::matanim::TRACK_TEXTURE_ID_CURRENT)?;
        let a = pack.mat_anim(mat_anim)?;
        if a.texture_count == 0 {
            return None;
        }
        let index = ((v.max(0.0) + 0.5) as u32).min(a.texture_count - 1);
        let texture = a.textures[index as usize];
        (texture != crate::pack::TextureDesc::NO_ANIM).then_some(texture)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pack::PackWriter;
    use crate::scene::{DObjDesc, DObjNode, SceneGraph};

    #[test]
    fn effect_colors_map_onto_the_two_packed_combiner_sources() {
        let colors = EffectColors {
            prim: Some([0x11, 0x22, 0x33, 0x44]),
            env: Some([0x55, 0x66, 0x77, 0x88]),
            ..EffectColors::default()
        };
        let original = crate::psp_texture::pack_abgr([1, 2, 3, 4]);

        assert_eq!(
            colors.vertex_color(original, true, false),
            crate::psp_texture::pack_abgr([0x11, 0x22, 0x33, 0x44])
        );
        assert_eq!(
            colors.vertex_color(original, false, true),
            crate::psp_texture::pack_abgr([0x55, 0x66, 0x77, 0x44])
        );
        assert_eq!(
            colors.vertex_color(original, false, false),
            crate::psp_texture::pack_abgr([1, 2, 3, 0x44])
        );
        assert_eq!(colors.packed_prim(), Some(0x4433_2211));
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

    #[test]
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
        assert!(prim[3] > 0, "measurably visible after one tick: {prim:?}");
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
