//! Sector Z's Arwing (`grsector.c`): the runtime half of the stage's own
//! twelve-`DObj` ship.
//!
//! `grSectorInitAll` builds the Arwing from Fox's entry-Arwing tree (file
//! 161, `llFoxSpecial3EntryArwingDObjDesc`) with `grModelSetupGroundDObjs`
//! and the transform kinds of `dGRSectorArwingTransformKinds`, and animates
//! it one `DObj` at a time with `gcAddDObjAnimJoint` scripts from two files:
//!
//! * file 153, the file `MPGroundData::map_nodes` points into
//!   (`gGRCommonStruct.sector.map_head`): the eight flight patterns'
//!   `GRSectorDesc`s, whose scripts fly node 0 along a path spline
//!   ([`crate::interp`]) and hide or show nodes 7 and 9; the five pilot
//!   manoeuvres on node 1; and the laser muzzle scripts on nodes 2–5;
//! * file 161 itself (`map_file`): the engine glow's loop on node 10 and
//!   the engine flare on node 8.
//!
//! Node 0 draws through `grSectorArwingLaser3DFuncMatrix`, a matrix the
//! controller derives from the path and stores in [`Arwing::root`]. Both
//! files' bytes are packed whole under
//! [`AnimDesc::SECTOR`], so every script, `SYInterpDesc` and path pointer
//! is a file offset.

use crate::figatree::JointPose;
use crate::interp::Spline;
use crate::objanim::{AnimError, StageJoint};
use crate::pack::{AnimDesc, ObjectDesc, Pack, MODEL_SCALE};
use crate::scene::Mat4;

/// `llGRSectorMapFileID`: the `GRSectorMap` file with the header and the
/// lasers' `WPAttributes`.
pub const MAP_FILE: u32 = 0x106;
/// The file `map_nodes` points into, at `llGRSectorMapMapHead` (0).
pub const FLIGHT_FILE: u32 = 153;
/// `llFoxSpecial3FileID`.
pub const ARWING_FILE: u32 = 161;
/// `llFoxSpecial3EntryArwingDObjDesc`.
pub const ARWING_GRAPH: u32 = 0x2C30;
/// `llGRSectorMapArwingLaser{2D,3D}WeaponAttributes` in [`MAP_FILE`].
pub const LASER_2D_ATTRIBUTES: u32 = 0xBC;
pub const LASER_3D_ATTRIBUTES: u32 = 0xF0;
/// Both lasers' `WPAttributes.data`: file 153's display list.
pub const LASER_DISPLAY_LIST: u32 = 0x1C50;

/// `gcAddGObjDisplay(map_gobj, gcDrawDObjTreeDLLinksForGObj, 6, ...)`.
pub const DL_LINK: u8 = 6;

/// The `DObj`s `grModelSetupGroundDObjs` makes from the tree.
pub const NODES: usize = 12;

/// [`AnimDesc::SECTOR`] slots: which file's bytes.
pub const SLOT_FLIGHT: u32 = 0;
pub const SLOT_ARWING: u32 = 1;

/// `dGRSectorArwingSectorDescs`, in [`FLIGHT_FILE`].
pub const SECTOR_DESCS: [u32; 8] = [0x0, 0x250, 0x6D0, 0x3E0, 0xD10, 0xEB0, 0x1510, 0x11D0];
/// `dGRSectorArwingAnimJoints`, in [`FLIGHT_FILE`]. Entry 0 is never
/// played: pilot 0 means no manoeuvre.
pub const PILOT_ANIMS: [u32; 6] = [0x0, 0x1D34, 0x1DA4, 0x1DC4, 0x1D54, 0x1DE4];
/// The `GRSectorDesc` fields `func_ovl2_80107D50` starts, with their nodes.
pub const DESC_FIELDS: [(u32, usize); 4] = [(0x0, 0), (0x1C, 7), (0x24, 9), (0x2C, 11)];

/// One script the controller starts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Script {
    /// A `GRSectorDesc` field of pattern `pattern`, on its node.
    Flight { pattern: u8, field: u8 },
    /// `dGRSectorArwingAnimJoints[pilot]` on node 1.
    Pilot(u8),
    /// `llFoxSpecial3_1B84_AnimJoint` (in [`FLIGHT_FILE`]): a muzzle
    /// charging, on nodes 4 and 5.
    LaserCharge,
    /// `llFoxSpecial3_1B34_AnimJoint` (in [`FLIGHT_FILE`]): a muzzle
    /// firing, on nodes 2 and 3.
    LaserFire,
    /// `llFoxSpecial3_2EB4_AnimJoint`: the engine flare on node 8.
    Flare,
    /// `llFoxSpecial3_2E74_AnimJoint`: the engine glow's loop on node 10.
    Glow,
}

impl Script {
    /// The script's file slot and offset, or `None` for a NULL script.
    fn locate(self, flight: &[u8]) -> Option<(usize, u32)> {
        Some(match self {
            Script::Flight { pattern, field } => {
                let desc = *SECTOR_DESCS.get(pattern as usize)?;
                let (at, _) = *DESC_FIELDS.get(field as usize)?;
                let at = (desc + at) as usize;
                let ptr = u32::from_be_bytes(flight.get(at..at + 4)?.try_into().ok()?);
                if ptr == 0 {
                    return None;
                }
                (0, ptr)
            }
            Script::Pilot(id) => (0, *PILOT_ANIMS.get(id as usize).filter(|&&s| s != 0)?),
            Script::LaserCharge => (0, 0x1B84),
            Script::LaserFire => (0, 0x1B34),
            Script::Flare => (1, 0x2EB4),
            Script::Glow => (1, 0x2E74),
        })
    }
}

/// `dGRSectorArwingTransformKinds`: each node's first matrix.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    /// `grSectorArwingLaser3DFuncMatrix` (US kind 0x53).
    Custom,
    TraRotRpyR,
    Tra,
    /// No matrix: the node draws in its parent's.
    Null,
}

const KINDS: [Kind; NODES] = [
    Kind::Custom,
    Kind::TraRotRpyR,
    Kind::Tra,
    Kind::Tra,
    Kind::Tra,
    Kind::Tra,
    Kind::Tra,
    Kind::Tra,
    Kind::Tra,
    Kind::Tra,
    Kind::Tra,
    Kind::Null,
];

/// The live Arwing.
#[derive(Clone, Copy)]
pub struct Arwing {
    pub object: ObjectDesc,
    count: usize,
    joints: [StageJoint; NODES],
    /// `anim_wait != AOBJ_ANIM_NULL`.
    live: [bool; NODES],
    /// Which file each node's script came from (index into `files`).
    source: [usize; NODES],
    poses: [JointPose; NODES],
    parents: [Option<usize>; NODES],
    /// The two files' bytes in the pack blob: (offset, length).
    files: [(u32, u32); 2],
    /// `map_gobj->flags == GOBJ_FLAG_HIDDEN`.
    pub hidden: bool,
    /// Node 0's `grSectorArwingLaser3DFuncMatrix`, in the N64 `Mtx44f`
    /// layout and game units, as the controller last derived it.
    pub root: Mat4,
}

impl Arwing {
    /// The Arwing at rest, hidden, or `None` when the pack lacks its tree
    /// or either file.
    pub fn new(pack: &Pack<'_>) -> Option<Self> {
        let object = (0..pack.object_count())
            .filter_map(|i| pack.object(i))
            .find(|o| (o.source_file, o.source_offset) == (ARWING_FILE, ARWING_GRAPH))?;
        let file = |slot| {
            (0..pack.anim_count())
                .filter_map(|i| pack.anim(i))
                .find(|a| a.fighter == AnimDesc::SECTOR && a.slot == slot)
                .map(|a| (a.script_offset, a.script_len))
        };
        let files = [file(SLOT_FLIGHT)?, file(SLOT_ARWING)?];
        let count = (object.node_count as usize).min(NODES);
        let mut poses = [JointPose::default(); NODES];
        let mut parents = [None; NODES];
        for i in 0..count {
            let node = pack.node(object.first_node + i as u32)?;
            poses[i] = JointPose {
                rotate: node.rest_rotate,
                translate: node.rest_translate,
                scale: node.rest_scale,
            };
            parents[i] = node
                .parent
                .checked_sub(object.first_node)
                .map(|p| p as usize)
                .filter(|&p| p < i);
        }
        Some(Arwing {
            object,
            count,
            joints: [StageJoint::start(0, 0.0); NODES],
            live: [false; NODES],
            source: [0; NODES],
            poses,
            parents,
            files,
            hidden: true,
            root: Mat4::IDENTITY,
        })
    }

    fn file<'a>(&self, pack: &Pack<'a>, slot: usize) -> Option<&'a [u8]> {
        let (at, len) = self.files[slot];
        pack.blob(at, len as usize)
    }

    /// `gcAddDObjAnimJoint`: node `node` restarts on `script` at frame
    /// zero; its pose and flags stay. A NULL script stops the node.
    pub fn add_anim_joint(&mut self, pack: &Pack<'_>, node: usize, script: Script) {
        if node >= self.count {
            return;
        }
        let located = self.file(pack, 0).and_then(|f| script.locate(f));
        match located {
            Some((slot, at)) => {
                let flags = self.joints[node].flags;
                self.joints[node] = StageJoint::start_changed(at, 0.0);
                self.joints[node].flags = flags;
                self.source[node] = slot;
                self.live[node] = true;
            }
            None => self.live[node] = false,
        }
    }

    /// `grSectorArwingAddAnim`: [`Self::add_anim_joint`], then the node's
    /// own parse and play at once; a NULL script sets `anim_wait` to
    /// `AOBJ_ANIM_NULL`.
    pub fn add_anim(
        &mut self,
        pack: &Pack<'_>,
        node: usize,
        script: Option<Script>,
    ) -> Result<(), AnimError> {
        match script {
            Some(s) => {
                self.add_anim_joint(pack, node, s);
                self.tick_node(pack, node)
            }
            None => {
                self.stop(node);
                Ok(())
            }
        }
    }

    fn tick_node(&mut self, pack: &Pack<'_>, i: usize) -> Result<(), AnimError> {
        if i >= self.count || !self.live[i] {
            return Ok(());
        }
        let Some(data) = self.file(pack, self.source[i]) else {
            return Ok(());
        };
        self.joints[i].tick(data, 1.0, &mut self.poses[i])?;
        // `AOBJ_ANIM_END` becomes `AOBJ_ANIM_NULL` at the end of the play.
        if self.joints[i].ended() {
            self.live[i] = false;
        }
        Ok(())
    }

    /// `gcPlayAnimAll`: every node in tree order.
    pub fn play_all(&mut self, pack: &Pack<'_>) -> Result<(), AnimError> {
        for i in 0..self.count {
            self.tick_node(pack, i)?;
        }
        Ok(())
    }

    /// `dobj->anim_wait == AOBJ_ANIM_NULL`.
    pub fn anim_null(&self, node: usize) -> bool {
        !self.live.get(node).copied().unwrap_or(false)
    }

    /// `dobj->anim_wait = AOBJ_ANIM_NULL`.
    pub fn stop(&mut self, node: usize) {
        if let Some(l) = self.live.get_mut(node) {
            *l = false;
        }
    }

    /// `DObj::flags`: bit 0 hides the node's list, bit 1 its subtree.
    pub fn flags(&self, node: usize) -> u16 {
        self.joints.get(node).map_or(0, |j| j.flags)
    }

    pub fn set_flags(&mut self, node: usize, flags: u16) {
        if let Some(j) = self.joints.get_mut(node) {
            j.flags = flags;
        }
    }

    pub fn translate(&self, node: usize) -> [f32; 3] {
        self.poses.get(node).map_or([0.0; 3], |p| p.translate)
    }

    pub fn set_translate(&mut self, node: usize, t: [f32; 3]) {
        if let Some(p) = self.poses.get_mut(node) {
            p.translate = t;
        }
    }

    pub fn rotate(&self, node: usize) -> [f32; 3] {
        self.poses.get(node).map_or([0.0; 3], |p| p.rotate)
    }

    pub fn set_rotate(&mut self, node: usize, r: [f32; 3]) {
        if let Some(p) = self.poses.get_mut(node) {
            p.rotate = r;
        }
    }

    /// `gcGetAObjValue` of the node's `TraI` track, unclamped, or `None`
    /// while it has none.
    pub fn path_fraction(&self, node: usize) -> Option<f32> {
        self.joints
            .get(node)?
            .track_value(crate::figatree::TRACK_TRA_I)
    }

    fn spline<'a>(&self, pack: &Pack<'a>, node: usize) -> Option<(Spline, &'a [u8])> {
        let at = self.joints.get(node)?.interp()?;
        let data = self.file(pack, *self.source.get(node)?)?;
        Some((Spline::read(data, at)?, data))
    }

    /// `syInterpQuad(aobj->interpolate, t)` on the node's path.
    pub fn path_tangent(&self, pack: &Pack<'_>, node: usize, t: f32) -> Option<[f32; 3]> {
        let (s, data) = self.spline(pack, node)?;
        s.quad(data, t)
    }

    /// `syInterpCubic(aobj->interpolate, t)` on the node's path.
    pub fn path_point(&self, pack: &Pack<'_>, node: usize, t: f32) -> Option<[f32; 3]> {
        let (s, data) = self.spline(pack, node)?;
        s.cubic(data, t)
    }

    pub fn node_count(&self) -> usize {
        self.count
    }

    /// Node `i` draws: neither it nor an ancestor hides it.
    pub fn visible(&self, i: usize) -> bool {
        if self.flags(i) & 1 != 0 {
            return false;
        }
        let mut at = Some(i);
        while let Some(n) = at {
            if self.flags(n) & 2 != 0 {
                return false;
            }
            at = self.parents[n];
        }
        true
    }

    /// Every node's matrix, node 0's being [`Self::root`]. The rest follow
    /// `dGRSectorArwingTransformKinds`: node 1 translates and rotates, the
    /// others only translate, and node 11 has no matrix of its own.
    pub fn compose(&self, out: &mut [Mat4]) -> usize {
        let count = self.count.min(out.len());
        for i in 0..count {
            let pose = &self.poses[i];
            let t = pose.translate.map(|v| v / MODEL_SCALE);
            let local = match KINDS[i] {
                Kind::Custom => {
                    let mut m = self.root;
                    for k in 12..15 {
                        m.0[k] /= MODEL_SCALE;
                    }
                    m
                }
                Kind::TraRotRpyR => Mat4::from_trs(t, pose.rotate, [1.0; 3]),
                Kind::Tra => Mat4::from_trs(t, [0.0; 3], [1.0; 3]),
                Kind::Null => Mat4::IDENTITY,
            };
            out[i] = match self.parents[i] {
                Some(p) if i != 0 => out[p].mul(&local),
                _ => local,
            };
        }
        count
    }

    /// The billboard nodes' scales (`nGCMatrixKindRecalcRotRpyRSca` reads
    /// the node's own scale, times its ancestors' X scale).
    pub fn billboard_scales(&self, out: &mut [[f32; 2]]) -> usize {
        let count = self.count.min(out.len());
        let mut cumulative_x = [1.0f32; NODES];
        for i in 0..count {
            let parent_x = self.parents[i].map_or(1.0, |p| cumulative_x[p]);
            let s = self.poses[i].scale;
            out[i] = [parent_x * s[0], parent_x * s[1]];
            cumulative_x[i] = out[i][0];
        }
        count
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_flight_field_reads_the_descriptor_pointer() {
        let mut flight = alloc::vec![0u8; 0x300];
        flight[0x250 + 0x1C..0x250 + 0x20].copy_from_slice(&0x3B8u32.to_be_bytes());
        let s = Script::Flight {
            pattern: 1,
            field: 1,
        };
        assert_eq!(s.locate(&flight), Some((0, 0x3B8)));
        // Pattern 1 has no node-11 path: a NULL field.
        let s = Script::Flight {
            pattern: 1,
            field: 3,
        };
        assert_eq!(s.locate(&flight), None);
        assert_eq!(Script::Pilot(0).locate(&flight), None);
        assert_eq!(Script::Glow.locate(&flight), Some((1, 0x2E74)));
    }
}
