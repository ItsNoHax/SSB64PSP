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
//! Saffron's item models instead use the separate loaded item file 159;
//! [`GroundObjectAsset::source_file`] overrides the gate's map-node file 160.
//!
//! [`OBJECTS`] and [`ANIMS`] are the ports' asset tables: the US
//! `reloc_data_symbols.us.txt` labels the controllers pass to `gcAddAnimAll`,
//! `gcAddAnimJointAll` and `gcAddDObjAnimJoint`. Packed animations are keyed
//! by their [`ANIMS`] index ([`crate::pack::AnimDesc::GROUND`]).
//!
//! [`GroundObjects`] is the runtime half: one object per entry, its `DObj`
//! clocks ([`StageJoint`]) and poses, and the `GObj::anim_frame` the
//! controllers poll.
//!
//! [`MAT_ANIMS`] are the material animations (`p_matanim_joints`) the same
//! controllers start: Whispy's eye and mouth textures, the acid's scroll and
//! the Yoshi's Island cloud fades. Packed ones are keyed by their
//! [`MAT_ANIMS`] index ([`crate::pack::AnimDesc::GROUND_MAT`]). Each object
//! plays them on its own clock ([`GroundObject::materials`]); a primitive
//! whose script has not started draws its `MObjSub` rest material.
//!
//! Not every controller object comes from a `DObjDesc` array ([`Build`]).
//! The Mushroom Kingdom platforms are one `DObj` made straight from a
//! display list, the Castle ground is one `DObj` with none, and each Yoshi's
//! Island cloud adds a display-list `DObj` under every child of its graph
//! ([`Leaf`]). The pack builder gives each of these a one-node graph keyed
//! by the label the controller passes (RE-365).

use crate::figatree::JointPose;
use crate::objanim::{AnimError, StageJoint};
use crate::pack::{AnimDesc, AnimJoint, ObjectDesc, Pack, MODEL_SCALE};
use crate::scene::Mat4;
use crate::skeleton::EffectMaterialAnimator;

/// One controller-made object.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GroundObjectAsset {
    pub name: &'static str,
    /// The `GR*Map` file holding the stage's `MPGroundData`
    /// (`ll*MapFileID`); the pack's `StageDesc::source_file`.
    pub gr_file: u32,
    /// Item data can live outside the map-node file (Saffron file 159).
    pub source_file: Option<u32>,
    /// The label the controller subtracts from `map_nodes`.
    pub map_head: u32,
    /// The object's `DObjDesc` array.
    pub graph: u32,
    /// The display link the controller draws it on (`gcAddGObjDisplay`).
    /// Stage layers draw on 4, 6, 13 and 17 (`dGRDisplayDescs`);
    /// [`NO_LINK`] for an object with no display.
    pub dl_link: u8,
    /// How the controller makes the object's `DObj`s from `graph`.
    pub build: Build,
    /// A display-list `DObj` added under every child of the root.
    pub leaf: Option<Leaf>,
    /// Identical objects the controller makes from the same labels.
    pub instances: u8,
    /// Every `DObj` carries only a `nGCMatrixKindTra` matrix, so its
    /// rotation and scale never reach a drawn matrix.
    pub translate_only: bool,
    /// A stage item's tree (`itManagerMakeItem` from the stage's
    /// `ITAttributes`): hidden until its item lives, played by the item
    /// rather than [`GroundObjects::advance`]. `lbCommonEjectTreeDObj`
    /// removes the descriptor's empty root, so node [`ITEM_ROOT`] is the
    /// item's `DObjGetStruct`.
    pub item: bool,
}

/// An object with no display (`gcAddGObjDisplay` is never called).
pub const NO_LINK: u8 = u8::MAX;

/// How a controller builds an object's `DObj`s.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Build {
    /// `gcSetupCustomDObjs` / `grModelSetupGroundDObjs` on the `DObjDesc`
    /// array at `graph`.
    Desc,
    /// `gcAddDObjForGObj(gobj, graph)`: one root `DObj` drawing the display
    /// list at `graph`.
    Dl,
    /// `gcAddDObjForGObj(gobj, NULL)`: one root `DObj` with no display
    /// list. `graph` only keys the packed node.
    Empty,
}

/// `gcAddChildForDObj(child, dl)` on every child of the root, each with
/// `nGCMatrixKindTra` and `nGCMatrixKind48` matrices and the `MObj`s of
/// `lbCommonAddMObjForTreeDObjs(leaf, mobjsub)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Leaf {
    /// The display list; also the key of its packed one-node graph.
    pub dl: u32,
    /// The `MObjSub ***` table for the one-node tree.
    pub mobjsub: u32,
}

/// The `DObjDesc::id` the pack builder gives a [`Leaf`]: depth 0 with the
/// `0x2000` bit, which `gcSetupCommonDObjs` turns into the same
/// `nGCMatrixKindTra` + `nGCMatrixKind48` pair the controller adds.
pub const LEAF_DESC_ID: u32 = 0x2000;

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
/// `llGRYosterMapFileID`.
pub const YOSTER_FILE: u32 = 0x107;
/// `llGRInishieMapFileID`, `llGRCastleMapFileID`.
pub const INISHIE_FILE: u32 = 0x104;
pub const CASTLE_FILE: u32 = 0x103;

/// `llGRPupupuMapMapHead`.
const PUPUPU_HEAD: u32 = 0x10F0;

pub const WHISPY_EYES: u8 = 0;
pub const WHISPY_MOUTH: u8 = 1;
pub const FLOWERS_BACK: u8 = 2;
pub const FLOWERS_FRONT: u8 = 3;
pub const TARUCANN: u8 = 4;
pub const GATE: u8 = 5;
pub const ACID: u8 = 6;
pub const CLOUD: u8 = 7;
pub const SCALE_STRINGS: u8 = 8;
pub const SCALE_PLATFORM: u8 = 9;
pub const CASTLE_GROUND: u8 = 10;
pub const POWER_BLOCK: u8 = 11;
pub const PAKKUN: u8 = 12;
pub const MONSTER_FIRST: u8 = 13;
pub const BONUS3_FILE: u32 = 0x127;
pub const BONUS3_NODES_FILE: u32 = 0xA2;
pub const BONUS3_BUMPER_FIRST: u8 = 18;
pub const BONUS3_BUMPER_ANIM_FIRST: usize = 40;
pub const TARUBOMB_SOURCE: (u32, u32) = (BONUS3_NODES_FILE, 0x788);
pub const MONSTER_FILE: u32 = 159;
/// Two direct texture-ID frames of Charmander and Venusaur.
pub const MONSTER_TEXTURES: [[u32; 2]; 2] = [[0x1410, 0x1048], [0x1E80, 0x1B78]];

/// `gcAddGObjDisplay(item_gobj, ..., 11, ...)` in `itManagerMakeItem`.
pub const ITEM_LINK: u8 = 11;
/// The node an item tree's `DObjGetStruct(item_gobj)` names once
/// `itManagerMakeItem`'s `lbCommonEjectTreeDObj` has removed node 0 and
/// promoted its child (RE-429).
pub const ITEM_ROOT: usize = 1;
/// `ITCommonData` file 251 + 0xCF0 names this exact graph. The nearby
/// 0x7BE8 graph is a separate tree, not this pointer's target (RE-429).
pub const GBUMPER_SOURCE: (u32, u32) = (86, 0x7648);

/// `llGRInishieMapMapHead`: also the platform display list.
const INISHIE_HEAD: u32 = 0x5F0;

const fn desc(
    name: &'static str,
    gr_file: u32,
    map_head: u32,
    graph: u32,
    dl_link: u8,
) -> GroundObjectAsset {
    GroundObjectAsset {
        name,
        gr_file,
        source_file: None,
        map_head,
        graph,
        dl_link,
        build: Build::Desc,
        leaf: None,
        instances: 1,
        translate_only: false,
        item: false,
    }
}

/// `grPupupuInitAll`, `grJungleMakeTaruCann`, `grYamabukiMakeGate`,
/// `grZebesMakeAcid`, `grYosterInitAll`, `grInishieMakeScale`,
/// `grCastleInitAll`, and the Mushroom Kingdom items
/// `itPowerBlockMakeItem` and `itPakkunMakeItem`.
pub const OBJECTS: [GroundObjectAsset; 46] = [
    desc("WhispyEyes", PUPUPU_FILE, PUPUPU_HEAD, 0x10F0, 4),
    desc("WhispyMouth", PUPUPU_FILE, PUPUPU_HEAD, 0x1770, 4),
    desc("FlowersBack", PUPUPU_FILE, PUPUPU_HEAD, 0x2A80, 4),
    desc("FlowersFront", PUPUPU_FILE, PUPUPU_HEAD, 0x31F8, 16),
    desc("TaruCann", JUNGLE_FILE, 0xA98, 0xA98, 6),
    desc("Gate", YAMABUKI_FILE, 0x8A0, 0x8A0, 6),
    // `llGRZebesMapAcidDObjDesc` is both the map head and the graph.
    desc("Acid", ZEBES_FILE, 0xB08, 0xB08, 12),
    // `llGRYosterMapMapHead` with `nGCMatrixKindTra` on every node, then
    // `llGRYosterMapCloudDisplayList` under each of the three children.
    GroundObjectAsset {
        leaf: Some(Leaf {
            dl: 0x580,
            mobjsub: 0x4B8,
        }),
        instances: CLOUD_COUNT as u8,
        translate_only: true,
        ..desc("Cloud", YOSTER_FILE, YOSTER_HEAD, YOSTER_HEAD, 6)
    },
    // `llGRInishieMapScaleDObjDesc` with `dGRInishieScaleTransformKinds`:
    // `nGCMatrixKindTra` on all five nodes.
    GroundObjectAsset {
        translate_only: true,
        ..desc("ScaleStrings", INISHIE_FILE, INISHIE_HEAD, 0x380, 6)
    },
    // `gcAddDObjForGObj(llGRInishieMapMapHead)` with `nGCMatrixKindTra`.
    GroundObjectAsset {
        build: Build::Dl,
        instances: 2,
        translate_only: true,
        ..desc("ScalePlatform", INISHIE_FILE, INISHIE_HEAD, INISHIE_HEAD, 6)
    },
    // `gcAddDObjForGObj(NULL)`, animated by the table `map_nodes` names;
    // it has no display.
    GroundObjectAsset {
        build: Build::Empty,
        ..desc("CastleGround", CASTLE_FILE, 0x0, 0x0, NO_LINK)
    },
    // `llGRInishieMapPowerBlockItemAttributes` (file 260 + 0xD8) names the
    // `DObjDesc` array at `map_head` + 0x11F8 (`itGetPData` subtracts
    // `llGRInishieMapPowerBlockDataStart` from it).
    GroundObjectAsset {
        item: true,
        ..desc("PowerBlock", INISHIE_FILE, INISHIE_HEAD, 0x11F8, ITEM_LINK)
    },
    // `llGRInishieMapPakkunItemAttributes` (file 260 + 0x120): the tree
    // at `map_head` + 0xC30, one per `pakkun_gobj` slot.
    GroundObjectAsset {
        item: true,
        instances: 2,
        ..desc("Pakkun", INISHIE_FILE, INISHIE_HEAD, 0xC30, ITEM_LINK)
    },
    GroundObjectAsset {
        item: true,
        source_file: Some(MONSTER_FILE),
        ..desc("Chansey", YAMABUKI_FILE, 0x8A0, 0x360, ITEM_LINK)
    },
    GroundObjectAsset {
        item: true,
        source_file: Some(MONSTER_FILE),
        ..desc("Electrode", YAMABUKI_FILE, 0x8A0, 0x790, ITEM_LINK)
    },
    GroundObjectAsset {
        item: true,
        source_file: Some(MONSTER_FILE),
        ..desc("Charmander", YAMABUKI_FILE, 0x8A0, 0x1990, ITEM_LINK)
    },
    GroundObjectAsset {
        item: true,
        source_file: Some(MONSTER_FILE),
        ..desc("Venusaur", YAMABUKI_FILE, 0x8A0, 0x2340, ITEM_LINK)
    },
    GroundObjectAsset {
        item: true,
        source_file: Some(MONSTER_FILE),
        ..desc("Porygon", YAMABUKI_FILE, 0x8A0, 0xEA0, ITEM_LINK)
    },
    bonus3_bumper("Bonus3Bumper0"),
    bonus3_bumper("Bonus3Bumper1"),
    bonus3_bumper("Bonus3Bumper2"),
    bonus3_bumper("Bonus3Bumper3"),
    bonus1_targets(0),
    bonus1_targets(1),
    bonus1_targets(2),
    bonus1_targets(3),
    bonus1_targets(4),
    bonus1_targets(5),
    bonus1_targets(6),
    bonus1_targets(7),
    bonus1_targets(8),
    bonus1_targets(9),
    bonus1_targets(10),
    bonus1_targets(11),
    bonus2_bumpers(0),
    bonus2_bumpers(1),
    bonus2_bumpers(2),
    bonus2_bumpers(3),
    bonus2_bumpers(4),
    bonus2_bumpers(5),
    bonus2_bumpers(6),
    bonus2_bumpers(7),
    bonus2_bumpers(8),
    bonus2_bumpers(9),
    bonus2_bumpers(10),
    bonus2_bumpers(11),
];

const fn bonus2_bumpers(kind: usize) -> GroundObjectAsset {
    GroundObjectAsset {
        item: true,
        instances: 10,
        source_file: Some(GBUMPER_SOURCE.0),
        ..desc(
            "Bonus2Bumper",
            283 + kind as u32,
            match crate::bonus2::BUMPERS[kind] {
                Some((graph, _)) => graph,
                None => 0,
            },
            GBUMPER_SOURCE.1,
            ITEM_LINK,
        )
    }
}

const fn bonus1_targets(kind: usize) -> GroundObjectAsset {
    GroundObjectAsset {
        item: true,
        instances: 10,
        source_file: Some(crate::bonus1::MODEL.0),
        ..desc(
            "Bonus1Target",
            271 + kind as u32,
            0,
            crate::bonus1::MODEL.1,
            ITEM_LINK,
        )
    }
}

const fn bonus3_bumper(name: &'static str) -> GroundObjectAsset {
    GroundObjectAsset {
        item: true,
        source_file: Some(GBUMPER_SOURCE.0),
        ..desc(name, BONUS3_FILE, 0, GBUMPER_SOURCE.1, ITEM_LINK)
    }
}

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
pub const ANIMS: [GroundAnimAsset; 44] = [
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
    // `grInishieScaleUpdateStep`: `llGRInishieMapScaleRetractAnimJoint` on
    // one platform's only `DObj`.
    GroundAnimAsset {
        name: "ScaleRetract",
        object: SCALE_PLATFORM,
        script: 0x734,
        target: AnimTarget::Node(0),
    },
    // `grCastleInitAll`: the table at `map_nodes` itself.
    table("CastleGround", CASTLE_GROUND, 0x0),
    // The POW Block's `ITAttributes::anim_joints` (file 260 + 0xD8 + 8):
    // NULL for the descriptor's root, the pop-in for its child, which the
    // eject makes the item's root.
    table("PowerBlockAppear", POWER_BLOCK, 0x13B0),
    // `itPowerBlockWaitProcDamage`: `llGRInishieMapPowerBlockAnimJoint` on
    // the item's root.
    GroundAnimAsset {
        name: "PowerBlockDamage",
        object: POWER_BLOCK,
        script: 0x1288,
        target: AnimTarget::Node(ITEM_ROOT as u8),
    },
    // `itPakkunWaitProcUpdate`: `llGRInishieMapPakkunAppearAnimJoint` on
    // the item's root.
    GroundAnimAsset {
        name: "PakkunAppear",
        object: PAKKUN,
        script: 0xCC8,
        target: AnimTarget::Node(ITEM_ROOT as u8),
    },
    table("Chansey", MONSTER_FIRST, 0x3F0),
    table("Electrode", MONSTER_FIRST + 1, 0x820),
    table("Charmander", MONSTER_FIRST + 2, 0x1A20),
    table("Venusaur", MONSTER_FIRST + 3, 0x23D0),
    table("Porygon", MONSTER_FIRST + 4, 0xF30),
    GroundAnimAsset {
        name: "Bonus3Bumper0",
        object: BONUS3_BUMPER_FIRST,
        script: 0x124,
        target: AnimTarget::Node(ITEM_ROOT as u8),
    },
    GroundAnimAsset {
        name: "Bonus3Bumper1",
        object: BONUS3_BUMPER_FIRST + 1,
        script: 0x150,
        target: AnimTarget::Node(ITEM_ROOT as u8),
    },
    GroundAnimAsset {
        name: "Bonus3Bumper2",
        object: BONUS3_BUMPER_FIRST + 2,
        script: 0x18C,
        target: AnimTarget::Node(ITEM_ROOT as u8),
    },
    GroundAnimAsset {
        name: "Bonus3Bumper3",
        object: BONUS3_BUMPER_FIRST + 3,
        script: 0x1C8,
        target: AnimTarget::Node(ITEM_ROOT as u8),
    },
];

/// One material-animation table a controller starts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GroundMatAnimAsset {
    pub name: &'static str,
    /// Index into [`OBJECTS`].
    pub object: u8,
    /// The packed graph whose `MObj`s the table drives: the object's own,
    /// or its [`Leaf`]'s for a cloud (`lbCommonAddTreeDObjsAnimAll` on the
    /// leaf `DObj`).
    pub graph: u32,
    /// The `GR*Map` file and the label its controller subtracts from
    /// `map_nodes`, as in [`GroundObjectAsset`].
    pub gr_file: u32,
    pub map_head: u32,
    /// The `AObjEvent32 ***` table: one entry per `DObj` in tree order,
    /// each an array parallel to that node's `MObj` chain; or, for
    /// [`MatTarget::NodeMObj`], the script itself.
    pub table: u32,
    pub target: MatTarget,
}

/// Which `MObj`s a material animation drives.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatTarget {
    /// `gcAddAnimAll`'s `p_matanim_joints` table.
    Table,
    /// `gcAddMObjMatAnimJoint(dobj->mobj, script)`: the first `MObj` of
    /// node `n` (in tree order) alone.
    NodeMObj(u8),
}

const fn mat(name: &'static str, object: u8, table: u32) -> GroundMatAnimAsset {
    let asset = &OBJECTS[object as usize];
    GroundMatAnimAsset {
        name,
        object,
        graph: match asset.leaf {
            Some(leaf) => leaf.dl,
            None => asset.graph,
        },
        gr_file: asset.gr_file,
        map_head: asset.map_head,
        table,
        target: MatTarget::Table,
    }
}

/// A script on an item tree's root `MObj` (`DObjGetStruct(item_gobj)->mobj`).
const fn root_mat(name: &'static str, object: u8, script: u32) -> GroundMatAnimAsset {
    GroundMatAnimAsset {
        target: MatTarget::NodeMObj(ITEM_ROOT as u8),
        ..mat(name, object, script)
    }
}

/// `llGRYosterMapMapHead`.
const YOSTER_HEAD: u32 = 0x100;

/// Every material animation the ported controllers start. The order is
/// the index [`mat_anim_of`] and the cloud constants name; do not reorder.
pub const MAT_ANIMS: [GroundMatAnimAsset; 15] = [
    // `dGRPupupuWhispyEyesAnims[lr][Turn][1]`; Blink has none.
    mat("WhispyEyesLeftTurn", WHISPY_EYES, 0x11E0),
    mat("WhispyEyesRightTurn", WHISPY_EYES, 0x1270),
    // `dGRPupupuWhispyMouthAnims[lr][status][1]`.
    mat("WhispyMouthLeftStretch", WHISPY_MOUTH, 0x1A00),
    mat("WhispyMouthLeftTurn", WHISPY_MOUTH, 0x1CE0),
    mat("WhispyMouthLeftOpen", WHISPY_MOUTH, 0x20B0),
    mat("WhispyMouthLeftClose", WHISPY_MOUTH, 0x22A0),
    mat("WhispyMouthRightStretch", WHISPY_MOUTH, 0x1BA0),
    mat("WhispyMouthRightTurn", WHISPY_MOUTH, 0x1E30),
    mat("WhispyMouthRightOpen", WHISPY_MOUTH, 0x2540),
    mat("WhispyMouthRightClose", WHISPY_MOUTH, 0x2740),
    // `grZebesMakeAcid`: `llGRZebesMapAcidMatAnimJoint`.
    mat("Acid", ACID, 0xBD0),
    // `dGRYosterCloudMatAnimJoints`.
    mat("CloudSolid", CLOUD, 0x670),
    mat("CloudEvaporate", CLOUD, 0x690),
    // `itPakkunWaitProcUpdate` / `itPakkunAppearProcDamage`.
    root_mat("PakkunAppear", PAKKUN, 0xCF8),
    root_mat("PakkunDamaged", PAKKUN, 0xE04),
];

pub const CLOUD_SOLID_MAT: usize = 11;
pub const CLOUD_EVAPORATE_MAT: usize = 12;
pub const PAKKUN_APPEAR_MAT: usize = 13;
pub const PAKKUN_DAMAGED_MAT: usize = 14;
/// `ARRAY_COUNT(gGRCommonStruct.yoster.clouds)`.
pub const CLOUD_COUNT: usize = 3;

/// The [`MAT_ANIMS`] table the controller passes alongside [`ANIMS`]`[anim]`
/// to `gcAddAnimAll`, or `None` when it passes NULL (Whispy's blink) or
/// starts only a joint animation (`gcAddAnimJointAll`).
pub const fn mat_anim_of(anim: usize) -> Option<usize> {
    match anim {
        0 => Some(0), // WhispyEyesLeftTurn
        2 => Some(1), // WhispyEyesRightTurn
        4..=11 => Some(anim - 2),
        ACID_ANIM => Some(10),
        PAKKUN_APPEAR => Some(PAKKUN_APPEAR_MAT),
        _ => None,
    }
}

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
pub const SCALE_RETRACT: usize = 30;
pub const CASTLE_GROUND_ANIM: usize = 31;
pub const POWER_BLOCK_APPEAR: usize = 32;
pub const POWER_BLOCK_DAMAGE: usize = 33;
pub const PAKKUN_APPEAR: usize = 34;

/// Nodes one controller object may have, counting the extra leaves the
/// packer adds for display lists a node could not carry (identity locals
/// under their node). The largest ported graph (the front flower bed) has 10.
pub const MAX_OBJECT_NODES: usize = 32;
/// Controller objects one stage may have, counting instances (Dream
/// Land's four; Mushroom Kingdom's strings, two platforms, the POW Block
/// and two Piranha Plants).
pub const MAX_STAGE_OBJECTS: usize = 10;

/// One live controller object: its nodes' clocks and poses, and the
/// `GObj::anim_frame` its parses write.
#[derive(Clone, Copy)]
pub struct GroundObject {
    /// Index into [`OBJECTS`].
    pub asset: u8,
    /// Which of the asset's [`GroundObjectAsset::instances`] this is.
    pub instance: u8,
    pub object: ObjectDesc,
    /// The packed one-node graph of the asset's [`Leaf`], drawn under
    /// every child of the root.
    pub leaf: Option<ObjectDesc>,
    count: usize,
    translate_only: bool,
    joints: [StageJoint; MAX_OBJECT_NODES],
    /// `anim_wait != AOBJ_ANIM_NULL`: the node has a script to run.
    live: [bool; MAX_OBJECT_NODES],
    poses: [JointPose; MAX_OBJECT_NODES],
    /// `GObj::anim_frame`.
    pub frame: f32,
    pub texture: u8,
    texture_meshes: [Option<u32>; 2],
    /// An item tree with no live item: not drawn.
    pub hidden: bool,
    /// `itPakkunAppearProcDamage` replaces xobj 1's kind 48 with 0x46
    /// (70, `func_ovl0_800CA194`). Rebirth clears the spin, not the kind.
    pakkun_damaged_matrix: bool,
    /// The animation file's bytes in the pack blob (offset, length); every
    /// animation of one object comes from the same file.
    script: Option<(u32, u32)>,
    /// Its `MObj`s' material clocks, keyed by the primitives'
    /// `MatAnimDesc` index. A cloud's three leaves share one script start,
    /// so one clock stands for all three.
    materials: EffectMaterialAnimator,
}

impl GroundObject {
    fn new(
        pack: &Pack<'_>,
        asset: u8,
        instance: u8,
        object: ObjectDesc,
        leaf: Option<ObjectDesc>,
    ) -> Self {
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
            instance,
            object,
            leaf,
            count,
            translate_only: OBJECTS[asset as usize].translate_only,
            joints: [StageJoint::start(0, 0.0); MAX_OBJECT_NODES],
            live: [false; MAX_OBJECT_NODES],
            poses,
            frame: 0.0,
            texture: 0,
            texture_meshes: {
                let frames = match asset {
                    15 => Some(MONSTER_TEXTURES[0]),
                    16 => Some(MONSTER_TEXTURES[1]),
                    _ => None,
                };
                core::array::from_fn(|i| {
                    frames.and_then(|f| {
                        (0..pack.mesh_count()).find(|&m| {
                            pack.mesh(m).is_some_and(|m| {
                                m.source_file == MONSTER_FILE && m.source_offset == f[i]
                            })
                        })
                    })
                })
            },
            hidden: OBJECTS[asset as usize].item,
            pakkun_damaged_matrix: false,
            script: None,
            materials: EffectMaterialAnimator::new(),
        }
    }

    /// The material player the object's primitives resolve against.
    pub fn materials(&self) -> &EffectMaterialAnimator {
        &self.materials
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

    /// `DObjGetStruct(gobj)->translate.vec.f = t`.
    pub fn set_translate(&mut self, t: [f32; 3]) {
        self.poses[0].translate = t;
    }

    /// Node `i`'s translation, in game units.
    pub fn node_translate(&self, i: usize) -> Option<[f32; 3]> {
        self.poses[..self.count].get(i).map(|p| p.translate)
    }

    /// `dobjs[i]->translate.vec.f.y = y` on a node the controller kept.
    pub fn set_node_translate_y(&mut self, i: usize, y: f32) {
        if let Some(pose) = self.poses[..self.count].get_mut(i) {
            pose.translate[1] = y;
        }
    }

    /// `dobj->anim_wait = AOBJ_ANIM_NULL; dobj->flags = DOBJ_FLAG_NONE` on
    /// the root.
    pub fn stop(&mut self) {
        self.live[0] = false;
        self.joints[0].flags = 0;
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

    /// The item's runtime matrix kinds override the descriptor's kinds.
    /// Kind 70 uses the camera basis and Z spin without object scale.
    pub fn draw_node(&self, i: usize, mut node: crate::pack::NodeDesc) -> crate::pack::NodeDesc {
        if i == ITEM_ROOT {
            if let Some(mesh) = self.texture_meshes[usize::from(self.texture.min(1))] {
                node.mesh = mesh;
            }
        }
        if self.asset == MONSTER_FIRST + 1 && i == ITEM_ROOT {
            node.flags = crate::pack::NodeDesc::FLAG_BILLBOARD
                | crate::pack::NodeDesc::FLAG_BILLBOARD_SPIN_Z;
        }
        if self.asset == PAKKUN && i == ITEM_ROOT {
            use crate::pack::NodeDesc;
            node.flags = NodeDesc::FLAG_BILLBOARD;
            if self.pakkun_damaged_matrix {
                node.flags |= NodeDesc::FLAG_BILLBOARD_SPIN_Z;
                node.rest_rotate[2] = self.poses[i].rotate[2];
                let t = [node.world[12], node.world[13], node.world[14]];
                node.world = Mat4::from_trs(t, [0.0; 3], [1.0; 3]).0;
            } else {
                node.flags |= NodeDesc::FLAG_BILLBOARD_PITCH_LOCKED;
            }
        }
        node
    }

    /// `DObj::flags`: bit 0 hides the node's mesh, bit 1 its subtree.
    pub fn flags(&self, i: usize) -> u16 {
        if i < self.count {
            self.joints[i].flags
        } else {
            0
        }
    }

    fn parent(&self, pack: &Pack<'_>, i: usize) -> Option<usize> {
        pack.node(self.object.first_node + i as u32)
            .and_then(|n| n.parent.checked_sub(self.object.first_node))
            .map(|p| p as usize)
            .filter(|&p| p < i)
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
            let Some(parent) = self.parent(pack, at) else {
                return true;
            };
            at = parent;
        }
        true
    }

    /// The nodes a [`Leaf`] hangs under: every child of the root, in tree
    /// order.
    pub fn leaf_parents<'a>(&'a self, pack: &'a Pack<'_>) -> impl Iterator<Item = usize> + 'a {
        (1..self.count).filter(move |&i| self.leaf.is_some() && self.parent(pack, i) == Some(0))
    }

    /// Composes every node's world matrix from the live poses, as
    /// [`crate::skeleton::StageAnimator::compose`] does. A translate-only
    /// object drops rotation and scale, as its `nGCMatrixKindTra` matrices
    /// do.
    pub fn compose(&self, pack: &Pack<'_>, out: &mut [Mat4]) -> usize {
        let count = self.count.min(out.len());
        for i in 0..count {
            let pose = &self.poses[i];
            let t = [
                pose.translate[0] / MODEL_SCALE,
                pose.translate[1] / MODEL_SCALE,
                pose.translate[2] / MODEL_SCALE,
            ];
            let local = if self.translate_only {
                Mat4::from_trs(t, [0.0; 3], [1.0; 3])
            } else {
                Mat4::from_trs(t, pose.rotate, pose.scale)
            };
            out[i] = match self.parent(pack, i) {
                Some(p) => out[p].mul(&local),
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

    /// `gcPlayAnimAll`: every node in tree order, then its `MObj`s. No
    /// material parse reads or writes a node's clock, so the material
    /// clocks can tick after the whole tree.
    fn tick(&mut self, pack: &Pack<'_>) -> Result<(), AnimError> {
        self.materials.tick(pack);
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
    target_anims: [Option<AnimDesc>; 10],
    objects: [Option<GroundObject>; MAX_STAGE_OBJECTS],
    /// Sector Z's Arwing ([`crate::sector`]), whose scripts come from two
    /// files and play node by node.
    pub arwing: Option<crate::sector::Arwing>,
    anims: [Option<AnimDesc>; ANIMS.len()],
    mat_anims: [Option<AnimDesc>; MAT_ANIMS.len()],
}

impl GroundObjects {
    /// No objects: a stage with no ported controller objects.
    pub fn empty() -> Self {
        GroundObjects {
            target_anims: [None; 10],
            objects: [None; MAX_STAGE_OBJECTS],
            arwing: None,
            anims: [None; ANIMS.len()],
            mat_anims: [None; MAT_ANIMS.len()],
        }
    }

    /// The objects of the stage whose `MPGroundData` came from `gr_file`
    /// (`StageDesc::source_file`), at rest. The stage's packed animations
    /// name the file `map_nodes` points into; every object is the one
    /// packed from its label in that file.
    pub fn new(pack: &Pack<'_>, gr_file: u32) -> Self {
        let mut this = Self::empty();
        if let Some(kind) = crate::bonus2::kind(gr_file) {
            if let Some((graph, _)) = crate::bonus2::BUMPERS[kind as usize] {
                let placements = (0..pack.object_count())
                    .filter_map(|i| pack.object(i))
                    .find(|o| (o.source_file, o.source_offset) == (137 + u32::from(kind), graph));
                let model = (0..pack.object_count())
                    .filter_map(|i| pack.object(i))
                    .find(|o| (o.source_file, o.source_offset) == GBUMPER_SOURCE);
                if let (Some(placements), Some(model)) = (placements, model) {
                    for i in 0..(placements.node_count - 1).min(MAX_STAGE_OBJECTS as u32) {
                        this.objects[i as usize] = Some(GroundObject::new(
                            pack,
                            crate::bonus2::FIRST_BUMPER_ASSET + kind,
                            i as u8,
                            model,
                            None,
                        ));
                        this.target_anims[i as usize] =
                            pack.item_anim(crate::bonus2::BUMPER_ANIM + u32::from(kind) * 10 + i);
                    }
                }
            }
            return this;
        }
        if let Some(kind) = crate::bonus1::kind(gr_file) {
            let object = (0..pack.object_count())
                .filter_map(|i| pack.object(i))
                .find(|o| (o.source_file, o.source_offset) == crate::bonus1::MODEL);
            if let Some(object) = object {
                for i in 0..10 {
                    this.objects[i] = Some(GroundObject::new(
                        pack,
                        crate::bonus1::FIRST_ASSET + kind,
                        i as u8,
                        object,
                        None,
                    ));
                    this.target_anims[i] = pack.item_anim(crate::bonus1::anim(kind, i as u8));
                }
            }
            return this;
        }
        if gr_file == crate::sector::MAP_FILE {
            this.arwing = crate::sector::Arwing::new(pack);
        }
        let mut file = None;
        for i in 0..pack.anim_count() {
            let Some(a) = pack.anim(i) else { continue };
            let gr = if a.fighter == AnimDesc::GROUND {
                ANIMS
                    .get(a.slot as usize)
                    .map(|anim| OBJECTS[anim.object as usize].gr_file)
            } else if a.fighter == AnimDesc::GROUND_MAT {
                MAT_ANIMS.get(a.slot as usize).map(|m| m.gr_file)
            } else {
                None
            };
            if gr != Some(gr_file) {
                continue;
            }
            if a.fighter == AnimDesc::GROUND_MAT
                || gr_file == BONUS3_FILE
                || OBJECTS[ANIMS[a.slot as usize].object as usize]
                    .source_file
                    .is_none()
            {
                file = Some(a.source_file);
            }
            if a.fighter == AnimDesc::GROUND {
                this.anims[a.slot as usize] = Some(a);
            } else {
                this.mat_anims[a.slot as usize] = Some(a);
            }
        }
        let Some(file) = file else { return this };
        let find = |file: u32, offset: u32| {
            (0..pack.object_count())
                .filter_map(|i| pack.object(i))
                .find(|o| o.source_file == file && o.source_offset == offset)
        };
        let mut n = 0;
        for (index, asset) in OBJECTS.iter().enumerate() {
            if asset.gr_file != gr_file {
                continue;
            }
            let source = asset.source_file.unwrap_or(file);
            let Some(object) = find(source, asset.graph) else {
                continue;
            };
            let leaf = asset.leaf.and_then(|l| find(source, l.dl));
            for instance in 0..asset.instances {
                if n == MAX_STAGE_OBJECTS {
                    return this;
                }
                this.objects[n] =
                    Some(GroundObject::new(pack, index as u8, instance, object, leaf));
                n += 1;
            }
        }
        this
    }

    pub fn iter(&self) -> impl Iterator<Item = &GroundObject> {
        self.objects.iter().flatten()
    }

    /// The live object for an [`OBJECTS`] index (its first instance).
    pub fn get(&self, asset: u8) -> Option<&GroundObject> {
        self.instance(asset, 0)
    }

    pub fn get_mut(&mut self, asset: u8) -> Option<&mut GroundObject> {
        self.instance_mut(asset, 0)
    }

    /// Instance `instance` of an [`OBJECTS`] index.
    pub fn instance(&self, asset: u8, instance: u8) -> Option<&GroundObject> {
        self.iter()
            .find(|o| o.asset == asset && o.instance == instance)
    }

    pub fn instance_mut(&mut self, asset: u8, instance: u8) -> Option<&mut GroundObject> {
        self.objects
            .iter_mut()
            .flatten()
            .find(|o| o.asset == asset && o.instance == instance)
    }

    /// Whether the pack carries animation `anim` for this stage.
    pub fn has(&self, anim: usize) -> bool {
        self.anims.get(anim).is_some_and(Option::is_some)
    }

    /// Starts [`ANIMS`]`[anim]` on the object's first instance; see
    /// [`Self::play_on`].
    pub fn play(&mut self, pack: &Pack<'_>, anim: usize) -> Result<(), AnimError> {
        self.play_on(pack, anim, 0)
    }

    /// Starts [`ANIMS`]`[anim]` on instance `instance` and parses it at
    /// once, as the controller's own `gcPlayAnimAll` (or
    /// `gcParseDObjAnimJoint`) call does. An animation the pack lacks
    /// leaves the object as it is.
    pub fn play_on(&mut self, pack: &Pack<'_>, anim: usize, instance: u8) -> Result<(), AnimError> {
        let (Some(asset), Some(Some(desc))) = (ANIMS.get(anim), self.anims.get(anim).copied())
        else {
            return Ok(());
        };
        let mat = mat_anim_of(anim).and_then(|m| self.mat_anims[m]);
        let Some(obj) = self.instance_mut(asset.object, instance) else {
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
                // Its `p_matanim_joints`: each non-NULL entry restarts its
                // `MObj`; a NULL table or entry leaves the `MObj` as it is.
                if let Some(mat) = mat {
                    restart_materials(pack, obj, &mat);
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
    /// before the controller. Item trees play from their item's own
    /// process instead ([`Self::item_play`]).
    pub fn advance(&mut self, pack: &Pack<'_>) -> Result<(), AnimError> {
        for obj in self
            .objects
            .iter_mut()
            .flatten()
            .filter(|o| !OBJECTS[o.asset as usize].item)
        {
            obj.tick(pack)?;
        }
        if let Some(a) = self.arwing.as_mut() {
            a.play_all(pack)?;
        }
        Ok(())
    }

    /// `grYosterUpdateCloudAnim`: `lbCommonAddTreeDObjsAnimAll` with
    /// [`MAT_ANIMS`]`[mat]` on every leaf of cloud `cloud`. Only the `DObj`
    /// plays at once (`gcPlayDObjAnimJoint`); the `MObj` parses on the next
    /// [`Self::advance`]. A script the pack lacks leaves the cloud as it is.
    pub fn play_cloud(&mut self, pack: &Pack<'_>, cloud: usize, mat: usize) {
        let Some(Some(desc)) = self.mat_anims.get(mat).copied() else {
            return;
        };
        if let Some(obj) = self.instance_mut(CLOUD, cloud as u8) {
            restart_materials(pack, obj, &desc);
        }
    }

    /// `clouds[cloud].dobj[0]->mobj->anim_wait == AOBJ_ANIM_NULL`: the
    /// cloud has no script, or its script ended on an earlier parse.
    pub fn cloud_idle(&self, pack: &Pack<'_>, cloud: usize) -> bool {
        let Some(obj) = self.instance(CLOUD, cloud as u8) else {
            return true;
        };
        let Some(target) = self.mat_anims[CLOUD_SOLID_MAT]
            .iter()
            .flat_map(|d| (0..d.joint_count).filter_map(|j| pack.anim_joint(d.first_joint + j)))
            .find(|j| j.node != AnimJoint::NO_NODE)
            .map(|j| j.node)
        else {
            return true;
        };
        obj.materials.joint_for(target).is_none_or(|j| j.ended())
    }

    /// Whether the pack carries [`MAT_ANIMS`]`[mat]` for this stage.
    pub fn has_mat(&self, mat: usize) -> bool {
        self.mat_anims.get(mat).is_some_and(Option::is_some)
    }
}

/// The root translation axes a play wrote: those with a live track, while
/// the root's clock ran (`gcPlayDObjAnimJoint` skips an idle `DObj`).
pub type RootWrite = [Option<f32>; 3];

impl GroundObjects {
    /// `itManagerMakeItem` for an item tree: the rest pose, no clocks, then
    /// `gcAddAnimAll` + `gcPlayAnimAll` with its `ITAttributes` scripts (the
    /// POW Block's pop-in; the Piranha Plant has none).
    pub fn item_make(&mut self, pack: &Pack<'_>, asset: u8, instance: u8) {
        let target = if asset >= crate::bonus1::FIRST_ASSET {
            self.target_anims.get(instance as usize).copied().flatten()
        } else {
            None
        };
        let Some(obj) = self.instance_mut(asset, instance) else {
            return;
        };
        *obj = GroundObject::new(pack, obj.asset, obj.instance, obj.object, obj.leaf);
        // The ejected descriptor root keeps no transform of its own.
        obj.poses[0] = JointPose::default();
        if let Some(a) = target {
            obj.script = Some((a.script_offset, a.script_len));
            let script = pack
                .anim_joint(a.first_joint)
                .filter(|j| j.script != AnimJoint::NO_SCRIPT)
                .map(|j| j.script);
            obj.set_script(ITEM_ROOT, script);
            let _ = obj.tick(pack);
            return;
        }
        if asset == POWER_BLOCK {
            let _ = self.play_on(pack, POWER_BLOCK_APPEAR, instance);
        } else if (MONSTER_FIRST..MONSTER_FIRST + 5).contains(&asset) {
            let _ = self.play_on(pack, 35 + usize::from(asset - MONSTER_FIRST), instance);
        } else if (BONUS3_BUMPER_FIRST..BONUS3_BUMPER_FIRST + 4).contains(&asset) {
            let _ = self.play_on(
                pack,
                BONUS3_BUMPER_ANIM_FIRST + usize::from(asset - BONUS3_BUMPER_FIRST),
                instance,
            );
        }
    }

    /// `gcPlayAnimAll` on an item tree: `itProcessProcItemMain` outside
    /// hitlag.
    pub fn item_play(&mut self, pack: &Pack<'_>, asset: u8, instance: u8) -> RootWrite {
        let Some(obj) = self.instance_mut(asset, instance) else {
            return [None; 3];
        };
        let r = ITEM_ROOT.min(obj.count.saturating_sub(1));
        let ran = obj.live[r] && !obj.joints[r].ended();
        let _ = obj.tick(pack);
        if !ran {
            return [None; 3];
        }
        let t = obj.poses[r].translate;
        core::array::from_fn(|i| {
            obj.joints[r]
                .track_value(crate::figatree::TRACK_TRA_X + i)
                .map(|_| t[i])
        })
    }

    /// `gcAddDObjAnimJoint(root, ANIMS[anim])` and/or
    /// `gcAddMObjMatAnimJoint(root->mobj, MAT_ANIMS[mat])` at frame 0, then
    /// the caller's `gcPlayAnimAll`. An animation the pack lacks leaves its
    /// clock as it is.
    pub fn item_add_play(
        &mut self,
        pack: &Pack<'_>,
        anim: Option<usize>,
        mat: Option<usize>,
        asset: u8,
        instance: u8,
    ) -> RootWrite {
        let damaged_matrix = asset == PAKKUN && mat == Some(PAKKUN_DAMAGED_MAT);
        let joint = anim.and_then(|a| Some((self.anims.get(a).copied()??, ANIMS.get(a)?)));
        let mat = mat.and_then(|m| self.mat_anims.get(m).copied().flatten());
        let Some(obj) = self.instance_mut(asset, instance) else {
            return [None; 3];
        };
        if damaged_matrix {
            obj.pakkun_damaged_matrix = true;
        }
        if let Some((desc, asset_anim)) = joint {
            let node = match asset_anim.target {
                AnimTarget::Node(n) => n as usize,
                AnimTarget::Table => 0,
            };
            let script = (0..desc.joint_count)
                .filter_map(|j| pack.anim_joint(desc.first_joint + j))
                .find(|j| {
                    j.node == obj.object.first_node + node as u32
                        && j.script != AnimJoint::NO_SCRIPT
                })
                .map(|j| j.script);
            obj.script = Some((desc.script_offset, desc.script_len));
            if node < obj.count {
                obj.set_script(node, script);
            }
        }
        if let Some(mat) = mat {
            restart_materials(pack, obj, &mat);
        }
        self.item_play(pack, asset, instance)
    }

    /// `DObjGetStruct(item_gobj)->anim_wait == AOBJ_ANIM_NULL` (node
    /// [`ITEM_ROOT`]).
    pub fn item_root_frame(&self, asset: u8, instance: u8) -> f32 {
        self.instance(asset, instance).map_or(0.0, |o| {
            o.joints[ITEM_ROOT.min(o.count.saturating_sub(1))].frame()
        })
    }

    pub fn item_root_idle(&self, asset: u8, instance: u8) -> bool {
        self.instance(asset, instance).is_none_or(|o| {
            let r = ITEM_ROOT.min(o.count.saturating_sub(1));
            !o.live[r] || o.joints[r].ended()
        })
    }

    /// `DObjGetStruct(item_gobj)->anim_wait = AOBJ_ANIM_NULL`: the root
    /// keeps its pose and flags.
    pub fn item_stop_root(&mut self, asset: u8, instance: u8) {
        if let Some(o) = self.instance_mut(asset, instance) {
            let r = ITEM_ROOT.min(o.count.saturating_sub(1));
            o.live[r] = false;
        }
    }

    /// `dobj->mobj->anim_wait = AOBJ_ANIM_NULL`.
    pub fn item_stop_material(&mut self, asset: u8, instance: u8) {
        if let Some(o) = self.instance_mut(asset, instance) {
            o.materials.halt_all();
        }
    }

    /// Places a live item's tree for drawing: its root at the item's
    /// position, with the item's own `rotate.z`.
    pub fn item_place(&mut self, asset: u8, instance: u8, pos: [f32; 3], rotate_z: f32) {
        if let Some(o) = self.instance_mut(asset, instance) {
            let r = ITEM_ROOT.min(o.count.saturating_sub(1));
            o.hidden = false;
            o.poses[r].translate = pos;
            o.poses[r].rotate[2] = rotate_z;
        }
    }

    /// Hides every item tree; [`Self::item_place`] shows the live ones.
    pub fn hide_items(&mut self) {
        for o in self.objects.iter_mut().flatten() {
            if OBJECTS[o.asset as usize].item {
                o.hidden = true;
            }
        }
    }
}

/// Restarts every `MObj` a packed material table names on `obj`.
fn restart_materials(pack: &Pack<'_>, obj: &mut GroundObject, mat: &AnimDesc) {
    for j in (0..mat.joint_count).filter_map(|j| pack.anim_joint(mat.first_joint + j)) {
        if j.node != AnimJoint::NO_NODE && j.script != AnimJoint::NO_SCRIPT {
            obj.materials.restart(j.node, j.script);
        }
    }
}
