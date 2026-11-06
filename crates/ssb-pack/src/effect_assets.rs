//! The manager effects the pack converts.

use crate::*;

/// The whole archive, read once.
///
/// Material tables have to be resolved before any model can be converted,
/// because the `FTCommonPart` record that names a model's table lives in the
/// fighter's `*Main` file, not the `*Model` file it describes. Decompressing
/// everything up front costs about 17 MB of RAM and saves a second pass.
/// File-84 graph/material pairs named by the original manager's static
/// `EFDesc` records. The offsets follow the corrected relocData layout, whose
/// old linker-symbol names had the graph and preceding MObj wrapper swapped.
/// Keeping this table separate makes the source relationship testable without
/// requiring a copyrighted ROM in CI (RE-172).
pub const EF_COMMON_EFFECTS2_MOBJ_PAIRS: &[(u32, u32, u32)] = &[
    (84, 0x2040, 0x1EA0), // FireSpark
    (84, 0x2760, 0x22B8), // CatchSwirl
    (84, 0x3398, 0x2F78), // ReflectBreak
    (84, 0x53E8, 0x4F08), // DeadExplode
    (84, 0x6D00, 0x6B40), // NessPKFlash
];

#[derive(Clone, Copy)]
pub struct EffectAsset {
    pub name: &'static str,
    pub file: u32,
    pub graph: u32,
}

/// Unique display-bearing scene graphs referenced by the 53 static `EFDesc`
/// records in `ef/efmanager.c`. Three controller-only descriptors have a NULL
/// display callback and no graph; four pairs of descriptors intentionally
/// share one graph. The remaining 46 are the source-named DObj effect surface
/// that can be checked independently of the still-unimplemented LBParticle
/// script runtime (RE-172).
pub const MANAGER_EFFECT_ASSETS: &[EffectAsset] = &[
    EffectAsset {
        name: "DamageSlash",
        file: 83,
        graph: 0x7750,
    },
    EffectAsset {
        name: "DamageFlyOrbs",
        file: 83,
        graph: 0x7E80,
    },
    EffectAsset {
        name: "ImpactWave",
        file: 83,
        graph: 0x7C28,
    },
    EffectAsset {
        name: "CommonSpark",
        file: 83,
        graph: 0x8FA0,
    },
    EffectAsset {
        name: "DamageFlyMDust",
        file: 83,
        graph: 0xCAC8,
    },
    EffectAsset {
        name: "ShockSmall",
        file: 84,
        graph: 0x1500,
    },
    EffectAsset {
        name: "FireSpark",
        file: 84,
        graph: 0x2040,
    },
    EffectAsset {
        name: "CatchSwirl",
        file: 84,
        graph: 0x2760,
    },
    EffectAsset {
        name: "ReflectBreak",
        file: 84,
        graph: 0x3398,
    },
    EffectAsset {
        name: "DeadExplode",
        file: 84,
        graph: 0x53E8,
    },
    EffectAsset {
        name: "NessPKFlash",
        file: 84,
        graph: 0x6D00,
    },
    EffectAsset {
        name: "MBallRays",
        file: 85,
        graph: 0x0628,
    },
    EffectAsset {
        name: "RebirthHalo",
        file: 85,
        graph: 0x2AC0,
    },
    EffectAsset {
        name: "ItemGetSwirl",
        file: 85,
        graph: 0x3170,
    },
    EffectAsset {
        name: "Shield",
        file: 163,
        graph: 0x0300,
    },
    EffectAsset {
        name: "FoxReflector",
        file: 346,
        graph: 0x02B0,
    },
    EffectAsset {
        name: "YoshiShield",
        file: 338,
        graph: 0xA860,
    },
    EffectAsset {
        name: "PikachuUnk",
        file: 347,
        graph: 0x0800,
    },
    EffectAsset {
        name: "PikachuThunderShock",
        file: 347,
        graph: 0x1640,
    },
    EffectAsset {
        name: "PikachuThunderTrail",
        file: 341,
        graph: 0x95B0,
    },
    EffectAsset {
        name: "ThunderJolt",
        file: 342,
        graph: 0x2258,
    },
    EffectAsset {
        name: "VulcanJab",
        file: 348,
        graph: 0x0B20,
    },
    EffectAsset {
        name: "KirbyCutterTrail",
        file: 348,
        graph: 0x0DF8,
    },
    EffectAsset {
        name: "KirbyCutterUp",
        file: 348,
        graph: 0x12E8,
    },
    EffectAsset {
        name: "KirbyEntryStar",
        file: 348,
        graph: 0x1DA8,
    },
    EffectAsset {
        name: "KirbyCutterDown",
        file: 348,
        graph: 0x2390,
    },
    EffectAsset {
        name: "KirbyCutterDraw",
        file: 348,
        graph: 0x2888,
    },
    EffectAsset {
        name: "SamusGrappleBeam",
        file: 349,
        graph: 0x0380,
    },
    EffectAsset {
        name: "SamusEntryPoint",
        file: 349,
        graph: 0x0B90,
    },
    EffectAsset {
        name: "FalconKick",
        file: 350,
        graph: 0x0B08,
    },
    EffectAsset {
        name: "CaptainEntryCar",
        file: 350,
        graph: 0x5FC0,
    },
    EffectAsset {
        name: "FalconPunch",
        file: 333,
        graph: 0x0760,
    },
    EffectAsset {
        name: "PurinSing",
        file: 351,
        graph: 0x2130,
    },
    EffectAsset {
        name: "NessPsychicMagnet",
        file: 352,
        graph: 0x09A8,
    },
    EffectAsset {
        name: "NessPKThunderTrail",
        file: 335,
        graph: 0x9050,
    },
    EffectAsset {
        name: "NessPKThunderWave",
        file: 335,
        graph: 0x9A10,
    },
    EffectAsset {
        name: "LinkEntryWave",
        file: 353,
        graph: 0x03F8,
    },
    EffectAsset {
        name: "LinkEntryBeam",
        file: 353,
        graph: 0x07B8,
    },
    EffectAsset {
        name: "LinkSpinAttack",
        file: 353,
        graph: 0x11C0,
    },
    EffectAsset {
        name: "MBallThrown",
        // `gITManagerCommonData` is relocData file 86. Static file 251 owns
        // the ItemAttributes/file-handle record whose extern targets it; the
        // `llITCommonData*` offset itself is relative to file 86.
        file: 86,
        graph: 0x9430,
    },
    EffectAsset {
        name: "KirbyStar",
        file: 86,
        graph: 0x5458,
    },
    EffectAsset {
        name: "YoshiEntryEgg",
        file: 354,
        graph: 0x0530,
    },
    EffectAsset {
        name: "YoshiEggLay",
        file: 339,
        graph: 0x0960,
    },
    EffectAsset {
        name: "DonkeyEntryTaru",
        file: 355,
        graph: 0x07C8,
    },
    EffectAsset {
        name: "MarioEntryDokan",
        file: 356,
        graph: 0x0608,
    },
    EffectAsset {
        name: "FoxEntryArwing",
        file: 161,
        graph: 0x2C30,
    },
];

/// `EFDesc` records whose `o_dobjsetup` is passed directly to
/// `gcAddDObjForGObj`/`gcAddChildForDObj`, rather than to a DObjDesc-tree
/// setup function. The pointed data is a `Gfx*` or `DObjDLLink*`; represent it
/// as one identity node so the existing resolver and object packer preserve
/// exactly the display path the manager constructs at run time (RE-172).
pub const DIRECT_MANAGER_EFFECT_ASSETS: &[EffectAsset] = &[
    EffectAsset {
        name: "DamageFlyOrbs",
        file: 83,
        graph: 0x7E80,
    },
    EffectAsset {
        name: "ImpactWave",
        file: 83,
        graph: 0x7C28,
    },
    EffectAsset {
        name: "CommonSpark",
        file: 83,
        graph: 0x8FA0,
    },
    EffectAsset {
        name: "DamageFlyMDust",
        file: 83,
        graph: 0xCAC8,
    },
    EffectAsset {
        name: "ShockSmall",
        file: 84,
        graph: 0x1500,
    },
    EffectAsset {
        name: "YoshiShield",
        file: 338,
        graph: 0xA860,
    },
    EffectAsset {
        name: "PikachuThunderTrail",
        file: 341,
        graph: 0x95B0,
    },
    EffectAsset {
        name: "FalconPunch",
        file: 333,
        graph: 0x0760,
    },
    EffectAsset {
        name: "NessPKThunderTrail",
        file: 335,
        graph: 0x9050,
    },
    EffectAsset {
        name: "MBallThrown",
        file: 86,
        graph: 0x9430,
    },
    EffectAsset {
        name: "KirbyStar",
        file: 86,
        graph: 0x5458,
    },
    EffectAsset {
        name: "YoshiEntryEgg",
        file: 354,
        graph: 0x0530,
    },
];

pub const DIRECT_MANAGER_EFFECT_MOBJ_PAIRS: &[(u32, u32, u32)] = &[
    (83, 0x7C28, 0x7A80),  // ImpactWave
    (83, 0x8FA0, 0x8EC0),  // CommonSpark
    (83, 0xCAC8, 0xC978),  // DamageFlyMDust
    (84, 0x1500, 0x1428),  // ShockSmall
    (86, 0x9430, 0x9120),  // MBallThrown
    (333, 0x0760, 0x0690), // FalconPunch
    (341, 0x95B0, 0x9420), // PikachuThunderTrail
    (354, 0x0530, 0x0460), // YoshiEntryEgg
];

/// Every distinct `PrimDesc.mat_anim` index reachable from an object's own
/// nodes -- RE-175's per-primitive attachment, as opposed to the
/// texture-keyed `TextureDesc.mat_anim` RE-089/090/091 used. A `BTreeSet`
/// because several primitives (or, for Link Spin Attack, several distinct
/// scripts on the same object) commonly share or duplicate an index.
pub fn object_mat_anims(
    pack: &ssb_rom::pack::Pack<'_>,
    object: &ssb_rom::pack::ObjectDesc,
) -> BTreeSet<u32> {
    (0..object.node_count)
        .filter_map(|n| pack.node(object.first_node + n))
        .filter_map(|n| (n.mesh != ssb_rom::pack::NodeDesc::NO_MESH).then_some(n.mesh))
        .filter_map(|m| pack.mesh(m))
        .flat_map(|m| (0..m.prim_count).filter_map(move |p| pack.prim(m.first_prim + p)))
        .filter_map(|p| (p.mat_anim != ssb_rom::pack::TextureDesc::NO_ANIM).then_some(p.mat_anim))
        .collect()
}
