//! Stage controller objects: the GObjs a `gr*.c` controller makes and
//! animates itself, outside the four `MPGroundDesc` render layers.
//!
//! Each controller finds its objects through `MPGroundData::map_nodes`:
//!
//! ```c
//! map_head = (uintptr_t)gMPCollisionGroundData->map_nodes - (intptr_t)&llGRPupupuMapMapHead;
//! gcSetupCustomDObjs(gobj, map_head + llGRPupupuMapWhispyMouthTransformKindsDObjDesc, ...);
//! ```
//!
//! `map_nodes` points into a file of its own (Dream Land's is file 152, not
//! the `GRPupupuMap` file 255 that holds the header), at the label the
//! controller subtracts. So every `ll*` offset below is an offset into the
//! file `map_nodes` names, and the pack builder checks that `map_nodes`
//! really lands on [`GroundObjectAsset::map_head`] before trusting it.
//!
//! [`OBJECTS`] and [`ANIMS`] are the ports' asset tables: the US
//! `reloc_data_symbols.us.txt` labels the controllers pass to `gcAddAnimAll`,
//! `gcAddAnimJointAll` and `gcAddDObjAnimJoint`. Packed animations are keyed
//! by their [`ANIMS`] index ([`crate::pack::AnimDesc::GROUND`]).
//!
//! [`GroundObjects`] is the runtime half: one object per entry, its `DObj`
//! clocks ([`StageJoint`]) and poses, and the `GObj::anim_frame` the
//! controllers poll. Material animation (`MatAnimJoint`: Whispy's eye and
//! mouth textures, the acid's) is not played; the objects draw their rest
//! materials.

use crate::figatree::JointPose;
use crate::objanim::{AnimError, StageJoint};
use crate::pack::{AnimDesc, AnimJoint, ObjectDesc, Pack, MODEL_SCALE};
use crate::scene::Mat4;

/// One controller-made object.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GroundObjectAsset {
    pub name: &'static str,
    /// The `GR*Map` file holding the stage's `MPGroundData`
    /// (`ll*MapFileID`); the pack's `StageDesc::source_file`.
    pub gr_file: u32,
    /// The label the controller subtracts from `map_nodes`.
    pub map_head: u32,
    /// The object's `DObjDesc` array.
    pub graph: u32,
    /// The display link the controller draws it on (`gcAddGObjDisplay`).
    /// Stage layers draw on 4, 6, 13 and 17 (`dGRDisplayDescs`).
    pub dl_link: u8,
}

/// Which `DObj`s an animation drives.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimTarget {
    /// `gcAddAnimAll` / `gcAddAnimJointAll`: an `AObjEvent32 *[]`, one
    /// script per node in tree order; a NULL entry stops that node.
    Table,
    /// `gcAddDObjAnimJoint` on one node (by tree index), followed at once
    /// by `gcParseDObjAnimJoint` + `gcPlayDObjAnimJoint` on it alone. The
    /// label is the script itself.
    Node(u8),
}

/// One animation a controller starts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GroundAnimAsset {
    pub name: &'static str,
    /// Index into [`OBJECTS`].
    pub object: u8,
    pub script: u32,
    pub target: AnimTarget,
}

/// `llGRPupupuMapFileID`, `llGRJungleMapFileID`, `llGRYamabukiMapFileID`,
/// `llGRZebesMapFileID`.
pub const PUPUPU_FILE: u32 = 0xFF;
pub const JUNGLE_FILE: u32 = 0x105;
pub const YAMABUKI_FILE: u32 = 0x108;
pub const ZEBES_FILE: u32 = 0x101;

/// `llGRPupupuMapMapHead`.
const PUPUPU_HEAD: u32 = 0x10F0;

pub const WHISPY_EYES: u8 = 0;
pub const WHISPY_MOUTH: u8 = 1;
pub const FLOWERS_BACK: u8 = 2;
pub const FLOWERS_FRONT: u8 = 3;
pub const TARUCANN: u8 = 4;
pub const GATE: u8 = 5;
pub const ACID: u8 = 6;

/// `grPupupuInitAll`, `grJungleMakeTaruCann`, `grYamabukiMakeGate`,
/// `grZebesMakeAcid`.
pub const OBJECTS: [GroundObjectAsset; 7] = [
    GroundObjectAsset {
        name: "WhispyEyes",
        gr_file: PUPUPU_FILE,
        map_head: PUPUPU_HEAD,
        graph: 0x10F0,
        dl_link: 4,
    },
    GroundObjectAsset {
        name: "WhispyMouth",
        gr_file: PUPUPU_FILE,
        map_head: PUPUPU_HEAD,
        graph: 0x1770,
        dl_link: 4,
    },
    GroundObjectAsset {
        name: "FlowersBack",
        gr_file: PUPUPU_FILE,
        map_head: PUPUPU_HEAD,
        graph: 0x2A80,
        dl_link: 4,
    },
    GroundObjectAsset {
        name: "FlowersFront",
        gr_file: PUPUPU_FILE,
        map_head: PUPUPU_HEAD,
        graph: 0x31F8,
        dl_link: 16,
    },
    GroundObjectAsset {
        name: "TaruCann",
        gr_file: JUNGLE_FILE,
        map_head: 0xA98,
        graph: 0xA98,
        dl_link: 6,
    },
    GroundObjectAsset {
        name: "Gate",
        gr_file: YAMABUKI_FILE,
        map_head: 0x8A0,
        graph: 0x8A0,
        dl_link: 6,
    },
    // `llGRZebesMapAcidDObjDesc` is both the map head and the graph.
    GroundObjectAsset {
        name: "Acid",
        gr_file: ZEBES_FILE,
        map_head: 0xB08,
        graph: 0xB08,
        dl_link: 12,
    },
];

const fn table(name: &'static str, object: u8, script: u32) -> GroundAnimAsset {
    GroundAnimAsset {
        name,
        object,
        script,
        target: AnimTarget::Table,
    }
}

/// Every animation the ported controllers start. The order is the index
/// the lookup functions below compute; do not reorder.
pub const ANIMS: [GroundAnimAsset; 30] = [
    // `dGRPupupuWhispyEyesAnims[lr][status][0]`: Turn, Blink.
    table("WhispyEyesLeftTurn", WHISPY_EYES, 0x11A0),
    table("WhispyEyesLeftBlink", WHISPY_EYES, 0x12B0),
    table("WhispyEyesRightTurn", WHISPY_EYES, 0x1220),
    table("WhispyEyesRightBlink", WHISPY_EYES, 0x1330),
    // `dGRPupupuWhispyMouthAnims[lr][status][0]`: Stretch, Turn, Open, Close.
    table("WhispyMouthLeftStretch", WHISPY_MOUTH, 0x18B0),
    table("WhispyMouthLeftTurn", WHISPY_MOUTH, 0x1BE0),
    table("WhispyMouthLeftOpen", WHISPY_MOUTH, 0x1E80),
    table("WhispyMouthLeftClose", WHISPY_MOUTH, 0x2100),
    table("WhispyMouthRightStretch", WHISPY_MOUTH, 0x1A40),
    table("WhispyMouthRightTurn", WHISPY_MOUTH, 0x1D30),
    table("WhispyMouthRightOpen", WHISPY_MOUTH, 0x22F0),
    table("WhispyMouthRightClose", WHISPY_MOUTH, 0x2590),
    // `dGRPupupuWhispyMouthTextures[lr][phase]`, on the back flowers.
    table("WhispyMouthLeftOpenTexture", FLOWERS_BACK, 0x2BE0),
    table("WhispyMouthLeftBlowTexture", FLOWERS_BACK, 0x2C30),
    table("WhispyMouthLeftCloseTexture", FLOWERS_BACK, 0x2C80),
    table("WhispyMouthRightOpenTexture", FLOWERS_BACK, 0x2CD0),
    table("WhispyMouthRightBlowTexture", FLOWERS_BACK, 0x2D20),
    table("WhispyMouthRightCloseTexture", FLOWERS_BACK, 0x2D70),
    // `dGRPupupuWhispyEyesTextures[lr][phase]`, on the front flowers.
    table("WhispyEyesLeft0Texture", FLOWERS_FRONT, 0x33E0),
    table("WhispyEyesLeft1Texture", FLOWERS_FRONT, 0x3450),
    table("WhispyEyesLeft2Texture", FLOWERS_FRONT, 0x34B0),
    table("WhispyEyesRight0Texture", FLOWERS_FRONT, 0x3510),
    table("WhispyEyesRight1Texture", FLOWERS_FRONT, 0x35C0),
    table("WhispyEyesRight2Texture", FLOWERS_FRONT, 0x3660),
    // `grJungleMakeTaruCann`, `grJungleTaruCannAddAnimOffset` (the
    // barrel's child DObj).
    table("TaruCannDefault", TARUCANN, 0xB20),
    GroundAnimAsset {
        name: "TaruCannFill",
        object: TARUCANN,
        script: 0xB68,
        target: AnimTarget::Node(1),
    },
    GroundAnimAsset {
        name: "TaruCannShoot",
        object: TARUCANN,
        script: 0xBF8,
        target: AnimTarget::Node(1),
    },
    // `grYamabukiGateAddAnimOffset`.
    table("GateOpen", GATE, 0x9B0),
    table("GateClose", GATE, 0xA20),
    // `grZebesMakeAcid`: `llGRZebesMapAcidAnimJoint`.
    table("Acid", ACID, 0xB90),
];

/// `dGRPupupuWhispyEyesAnims[lr][blink]`.
pub const fn whispy_eyes(lr: u8, blink: bool) -> usize {
    lr as usize * 2 + blink as usize
}
/// `dGRPupupuWhispyMouthAnims[lr][status]`, status Stretch 0 .. Close 3.
pub const fn whispy_mouth(lr: u8, status: u8) -> usize {
    4 + lr as usize * 4 + status as usize
}
pub const fn flowers_back(lr: u8, phase: u8) -> usize {
    12 + lr as usize * 3 + phase as usize
}
pub const fn flowers_front(lr: u8, phase: u8) -> usize {
    18 + lr as usize * 3 + phase as usize
}
pub const TARUCANN_DEFAULT: usize = 24;
pub const TARUCANN_FILL: usize = 25;
pub const TARUCANN_SHOOT: usize = 26;
pub const GATE_OPEN: usize = 27;
pub const GATE_CLOSE: usize = 28;
pub const ACID_ANIM: usize = 29;

/// Nodes one controller object may have, counting the extra leaves the
/// packer adds for display lists a node could not carry (identity locals
/// under their node). The largest ported graph (the front flower bed) has 10.
pub const MAX_OBJECT_NODES: usize = 32;
/// Controller objects one stage may have (Dream Land's four).
pub const MAX_STAGE_OBJECTS: usize = 4;

/// One live controller object: its nodes' clocks and poses, and the
/// `GObj::anim_frame` its parses write.
#[derive(Clone, Copy)]
pub struct GroundObject {
    /// Index into [`OBJECTS`].
    pub asset: u8,
    pub object: ObjectDesc,
    count: usize,
    joints: [StageJoint; MAX_OBJECT_NODES],
    /// `anim_wait != AOBJ_ANIM_NULL`: the node has a script to run.
    live: [bool; MAX_OBJECT_NODES],
    poses: [JointPose; MAX_OBJECT_NODES],
    /// `GObj::anim_frame`.
    pub frame: f32,
    /// The animation file's bytes in the pack blob (offset, length); every
    /// animation of one object comes from the same file.
    script: Option<(u32, u32)>,
}

impl GroundObject {
    fn new(pack: &Pack<'_>, asset: u8, object: ObjectDesc) -> Self {
        let count = (object.node_count as usize).min(MAX_OBJECT_NODES);
        let mut poses = [JointPose::default(); MAX_OBJECT_NODES];
        for (i, pose) in poses.iter_mut().enumerate().take(count) {
            if let Some(node) = pack.node(object.first_node + i as u32) {
                *pose = JointPose {
                    rotate: node.rest_rotate,
                    translate: node.rest_translate,
                    scale: node.rest_scale,
                };
            }
        }
        GroundObject {
            asset,
            object,
            count,
            joints: [StageJoint::start(0, 0.0); MAX_OBJECT_NODES],
            live: [false; MAX_OBJECT_NODES],
            poses,
            frame: 0.0,
            script: None,
        }
    }

    pub fn node_count(&self) -> usize {
        self.count
    }

    /// The root `DObj`'s translation, in game units.
    pub fn translate(&self) -> [f32; 3] {
        self.poses[0].translate
    }

    /// `DObjGetStruct(gobj)->translate.vec.f.y = y`: a controller write.
    /// A root with no script keeps it; a scripted root would overwrite it
    /// on its next parse, as the source's would.
    pub fn set_translate_y(&mut self, y: f32) {
        self.poses[0].translate[1] = y;
    }

    /// `DObjGetStruct(gobj)->child->translate`: the root's first child is
    /// node 1 in tree order.
    pub fn child_translate(&self, pack: &Pack<'_>) -> Option<[f32; 3]> {
        let child = pack.node(self.object.first_node + 1)?;
        (self.count > 1 && child.parent == self.object.first_node)
            .then_some(self.poses[1].translate)
    }

    /// Node `i`'s current local transform.
    pub fn pose(&self, i: usize) -> Option<&JointPose> {
        self.poses[..self.count].get(i)
    }

    /// `DObj::flags`: bit 0 hides the node's mesh, bit 1 its subtree.
    pub fn flags(&self, i: usize) -> u16 {
        if i < self.count {
            self.joints[i].flags
        } else {
            0
        }
    }

    /// Whether node `i` (object-local) draws: neither it nor an ancestor
    /// hides it.
    pub fn visible(&self, pack: &Pack<'_>, i: usize) -> bool {
        if self.flags(i) & 1 != 0 {
            return false;
        }
        let mut at = i;
        for _ in 0..MAX_OBJECT_NODES {
            if self.flags(at) & 2 != 0 {
                return false;
            }
            let Some(parent) = pack
                .node(self.object.first_node + at as u32)
                .and_then(|n| n.parent.checked_sub(self.object.first_node))
                .filter(|&p| (p as usize) < at)
            else {
                return true;
            };
            at = parent as usize;
        }
        true
    }

    /// Composes every node's world matrix from the live poses, as
    /// [`crate::skeleton::StageAnimator::compose`] does.
    pub fn compose(&self, pack: &Pack<'_>, out: &mut [Mat4]) -> usize {
        let count = self.count.min(out.len());
        for i in 0..count {
            let pose = &self.poses[i];
            let t = [
                pose.translate[0] / MODEL_SCALE,
                pose.translate[1] / MODEL_SCALE,
                pose.translate[2] / MODEL_SCALE,
            ];
            let local = Mat4::from_trs(t, pose.rotate, pose.scale);
            let parent = pack
                .node(self.object.first_node + i as u32)
                .and_then(|n| n.parent.checked_sub(self.object.first_node))
                .filter(|&p| (p as usize) < i);
            out[i] = match parent {
                Some(p) => out[p as usize].mul(&local),
                None => local,
            };
        }
        count
    }

    /// Replaces node `i`'s clock with `script` at frame zero
    /// (`gcAddDObjAnimJoint`): the tracks reset, the pose and flags stay.
    fn set_script(&mut self, i: usize, script: Option<u32>) {
        match script {
            Some(s) => {
                let flags = self.joints[i].flags;
                self.joints[i] = StageJoint::start_changed(s, 0.0);
                self.joints[i].flags = flags;
                self.live[i] = true;
            }
            None => self.live[i] = false,
        }
    }

    /// Parses and plays node `i` once, writing the `GObj` clock.
    fn tick_node(&mut self, data: &[u8], i: usize) -> Result<(), AnimError> {
        if !self.live[i] {
            return Ok(());
        }
        self.joints[i].tick(data, 1.0, &mut self.poses[i])?;
        if let Some(f) = self.joints[i].gobj_frame() {
            self.frame = f;
        }
        Ok(())
    }

    /// `gcPlayAnimAll`: every node in tree order.
    fn tick(&mut self, pack: &Pack<'_>) -> Result<(), AnimError> {
        let Some((at, len)) = self.script else {
            return Ok(());
        };
        let Some(data) = pack.blob(at, len as usize) else {
            return Ok(());
        };
        for i in 0..self.count {
            self.tick_node(data, i)?;
        }
        Ok(())
    }
}

/// A stage's controller objects and the animations its controller may start.
pub struct GroundObjects {
    objects: [Option<GroundObject>; MAX_STAGE_OBJECTS],
    anims: [Option<AnimDesc>; ANIMS.len()],
}

impl GroundObjects {
    /// No objects: a stage with no ported controller objects.
    pub fn empty() -> Self {
        GroundObjects {
            objects: [None; MAX_STAGE_OBJECTS],
            anims: [None; ANIMS.len()],
        }
    }

    /// The objects of the stage whose `MPGroundData` came from `gr_file`
    /// (`StageDesc::source_file`), at rest. An object is found through its
    /// packed animations: they name the file the scripts and the graph
    /// share, and the object is the one packed from that graph.
    pub fn new(pack: &Pack<'_>, gr_file: u32) -> Self {
        let mut this = Self::empty();
        for i in 0..pack.anim_count() {
            let Some(a) = pack.anim(i) else { continue };
            if a.fighter != AnimDesc::GROUND {
                continue;
            }
            let Some(asset) = ANIMS.get(a.slot as usize) else {
                continue;
            };
            if OBJECTS[asset.object as usize].gr_file == gr_file {
                this.anims[a.slot as usize] = Some(a);
            }
        }
        let mut n = 0;
        for (index, asset) in OBJECTS.iter().enumerate() {
            if asset.gr_file != gr_file || n == MAX_STAGE_OBJECTS {
                continue;
            }
            let file = ANIMS
                .iter()
                .zip(this.anims.iter())
                .find(|(anim, desc)| anim.object as usize == index && desc.is_some())
                .and_then(|(_, desc)| desc.map(|d| d.source_file));
            let Some(file) = file else { continue };
            let object = (0..pack.object_count())
                .filter_map(|i| pack.object(i))
                .find(|o| o.source_file == file && o.source_offset == asset.graph);
            if let Some(object) = object {
                this.objects[n] = Some(GroundObject::new(pack, index as u8, object));
                n += 1;
            }
        }
        this
    }

    pub fn iter(&self) -> impl Iterator<Item = &GroundObject> {
        self.objects.iter().flatten()
    }

    /// The live object for an [`OBJECTS`] index.
    pub fn get(&self, asset: u8) -> Option<&GroundObject> {
        self.iter().find(|o| o.asset == asset)
    }

    pub fn get_mut(&mut self, asset: u8) -> Option<&mut GroundObject> {
        self.objects.iter_mut().flatten().find(|o| o.asset == asset)
    }

    /// Whether the pack carries animation `anim` for this stage.
    pub fn has(&self, anim: usize) -> bool {
        self.anims.get(anim).is_some_and(Option::is_some)
    }

    /// Starts [`ANIMS`]`[anim]` and parses it at once, as the controller's
    /// own `gcPlayAnimAll` (or `gcParseDObjAnimJoint`) call does. An
    /// animation the pack lacks leaves the object as it is.
    pub fn play(&mut self, pack: &Pack<'_>, anim: usize) -> Result<(), AnimError> {
        let (Some(asset), Some(Some(desc))) = (ANIMS.get(anim), self.anims.get(anim).copied())
        else {
            return Ok(());
        };
        let Some(obj) = self.get_mut(asset.object) else {
            return Ok(());
        };
        let script_of = |node: u32| {
            (0..desc.joint_count)
                .filter_map(|j| pack.anim_joint(desc.first_joint + j))
                .find(|j| j.node == node && j.script != AnimJoint::NO_SCRIPT)
                .map(|j| j.script)
        };
        obj.script = Some((desc.script_offset, desc.script_len));
        let Some(data) = pack.anim_script(&desc) else {
            return Ok(());
        };
        match asset.target {
            AnimTarget::Table => {
                // `gcAddAnimAll`: the GObj clock restarts at zero.
                obj.frame = 0.0;
                for i in 0..obj.count {
                    obj.set_script(i, script_of(obj.object.first_node + i as u32));
                }
                obj.tick(pack)
            }
            AnimTarget::Node(n) => {
                let i = n as usize;
                if i >= obj.count {
                    return Ok(());
                }
                obj.set_script(i, script_of(obj.object.first_node + i as u32));
                obj.tick_node(data, i)
            }
        }
    }

    /// `gcPlayAnimAll` on every object: the priority-5 process that runs
    /// before the controller.
    pub fn advance(&mut self, pack: &Pack<'_>) -> Result<(), AnimError> {
        for obj in self.objects.iter_mut().flatten() {
            obj.tick(pack)?;
        }
        Ok(())
    }
}
