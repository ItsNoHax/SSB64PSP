//! The Poké Ball Pokémon's and their weapons' trees and animations in
//! relocData file 86 (`gITManagerCommonData`; RE-435). File 251 holds their
//! `ITAttributes`/`WPAttributes`, whose `data` relocations name these trees.
//!
//! `itManagerMakeItem` ejects a Pokémon tree's descriptor 0, so its root is
//! packed node 1; a weapon's tree keeps descriptor 0 as its root.

/// `llITCommonDataFileID`'s data file.
pub const FILE: u32 = 86;

/// `llITCommonDataMonsterAnimBankStart`: the rise's squash-and-stretch
/// script every maker adds (`itGetMonsterAnimNode`), on the root or, for
/// Beedrill, Chansey and Koffing, its child.
pub const APPEAR_SCRIPT: u32 = 0x13624;

/// One Pokémon's tree and scripts, in `ITKind` order from Onix.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Visual {
    /// `ITAttributes::data`.
    pub graph: u32,
    /// The packed node the appear script drives.
    pub appear_node: u32,
    /// The script its status adds (`gcAddDObjAnimJoint`), and the node.
    pub status: Option<(u32, u32)>,
    /// The tree whose node 1 is the display list the attack status puts on
    /// the root (`dobj->dl = ...DisplayList`): Onix 0xA640, Blastoise
    /// 0xED60 and Hitmonlee 0x12340, each the first list of the tree
    /// packed after it.
    pub attack_dl_graph: Option<u32>,
}

const fn visual(graph: u32, appear_node: u32) -> Visual {
    Visual {
        graph,
        appear_node,
        status: None,
        attack_dl_graph: None,
    }
}

pub const VISUALS: [Visual; 13] = [
    Visual {
        attack_dl_graph: Some(0xA730),
        ..visual(0xA140, 1)
    },
    visual(0xB158, 1),
    // `itTosakintoBounceInitVars`: `llITCommonDataTosakintoAnimJoint` on
    // the child.
    Visual {
        status: Some((0xB7CC, 2)),
        ..visual(0xB708, 1)
    },
    visual(0xC130, 1),
    // `itLizardonAttackInitVars`: its root.
    Visual {
        status: Some((0xD658, 1)),
        ..visual(0xD5C0, 1)
    },
    // `itSpearAppearInitVars`: the child.
    Visual {
        status: Some((0xDFFC, 2)),
        ..visual(0xDF38, 2)
    },
    Visual {
        attack_dl_graph: Some(0xEE50),
        ..visual(0xEA60, 1)
    },
    // `itMLuckyMakeEggInitVars`.
    Visual {
        status: Some((0x100BC, 2)),
        ..visual(0x10000, 2)
    },
    visual(0x112A0, 1),
    Visual {
        attack_dl_graph: Some(0x12430),
        ..visual(0x11F40, 1)
    },
    // `itDogasAttackInitVars`.
    Visual {
        status: Some((0x128DC, 2)),
        ..visual(0x12820, 2)
    },
    visual(0x13598, 1),
    visual(0xBCC0, 1),
];

/// The weapons' `WPAttributes::data` trees.
pub const ROCK_GRAPH: u32 = 0xAB98;
pub const COIN_GRAPH: u32 = 0xC520;
pub const SPEAR_SWARM_GRAPH: u32 = 0xE4A8;
/// Clefairy's swarm draws Clefairy (`llITCommonDataPippiSwarmWeaponAttributes`).
pub const PIPPI_SWARM_GRAPH: u32 = 0x13598;
pub const HYDRO_GRAPH: u32 = 0xF9D8;
pub const SWIFT_GRAPH: u32 = 0x119D8;
pub const SMOG_GRAPH: u32 = 0x13100;
/// The `WPAttributes::anim_joints` tables `wpManagerMakeWeapon` adds.
pub const HYDRO_ANIM_JOINTS: u32 = 0xFA90;
pub const SMOG_ANIM_JOINTS: u32 = 0x13190;
/// Their `WPAttributes::p_matanim_joints` tables: the spray's, the cloud's
/// and the swarm's texture frames.
pub const HYDRO_MAT_ANIM_JOINTS: u32 = 0xFB70;
pub const SMOG_MAT_ANIM_JOINTS: u32 = 0x131E0;
pub const SPEAR_SWARM_MAT_ANIM_JOINTS: u32 = 0xE560;
