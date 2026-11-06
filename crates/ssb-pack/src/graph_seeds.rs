//! Which scene graphs the pack converts and how: fighter parts, seeded
//! effect, weapon and item graphs, initial material state.

use crate::*;

/// Every `(model_file, graph_offset)` pair either of a fighter's two
/// `FTCommonPart` detail levels names (RE-242).
///
/// `ftDisplayMainProcDisplay` sets `G_LIGHTING` and `gDPSetRenderMode(
/// G_RM_FOG_PRIM_A, G_RM_AA_ZB_OPA_SURF2)` unconditionally before either
/// graph's own node lists run (RE-021, RE-244), so any
/// [`ssb_rom::mesh::convert_sequence`] call decoding one of these graphs
/// must seed [`ssb_rom::mesh::InitialMaterial::FIGHTER_EXTERNAL`] to match —
/// every other graph in the archive keeps the RDP reset default.
pub fn fighter_skeleton_graphs(loaded: &Loaded) -> alloc::collections::BTreeSet<(u32, u32)> {
    ssb_rom::fighter::FIGHTER_FILES
        .iter()
        .flat_map(|entry| {
            let main = loaded.files.get(entry.file as usize);
            main.into_iter().flat_map(|main| {
                ssb_rom::fighter::common_parts(&main, *entry)
                    .into_iter()
                    .flatten()
                    .map(|part| (part.model_file, part.graph))
            })
        })
        .collect()
}

/// The electric-damage skeleton sets (`FTAttributes::skeleton[1..=2]`,
/// RE-414) of every fighter whose high- or low-detail `FTCommonPart` is
/// `graph` of `file`: `(fighter name, set id, parts)`.
pub fn skeleton_sets_for(
    loaded: &Loaded,
    file: u32,
    graph: &ssb_rom::scene::SceneGraph,
) -> Vec<(&'static str, u32, Vec<ssb_rom::fighter::SkeletonPart>)> {
    let mut out = Vec::new();
    let Some(model) = loaded.files.get(file as usize) else {
        return out;
    };
    for entry in &ssb_rom::fighter::FIGHTER_FILES {
        let Some(main) = loaded.files.get(entry.file as usize) else {
            continue;
        };
        // RE-426: either detail's graph; the skeleton draws under the
        // node's own `MObj`s at both.
        if !ssb_rom::fighter::common_parts(&main, *entry)
            .iter()
            .flatten()
            .any(|part| (part.model_file, part.graph) == (file, graph.offset))
        {
            continue;
        }
        let Some(sk) = ssb_rom::fighter::skeletons(&main, *entry, graph.nodes.len(), &model) else {
            continue;
        };
        for (i, set) in sk.sets.into_iter().enumerate() {
            if let Some(parts) = set {
                // Every list must be in the model file: the plan reads them
                // from it.
                if parts
                    .iter()
                    .flat_map(|p| p.dls)
                    .flatten()
                    .all(|(f, _)| f == file)
                {
                    out.push((entry.name, i as u32 + 1, parts));
                }
            }
        }
    }
    out
}

/// [`plan_draw_order`] for a skeleton set: each node draws its
/// `FTSkeleton` entry instead of its own list, a `dls` pair's first half in
/// the parent's space.
pub fn plan_skeleton_order(
    graph: &ssb_rom::scene::SceneGraph,
    parts: &[ssb_rom::fighter::SkeletonPart],
) -> Vec<PlannedList> {
    use ssb_rom::scene::Mat4;
    let worlds = graph.world_transforms();
    let mut out = Vec::new();
    for (i, (node, part)) in graph.nodes.iter().zip(parts).enumerate() {
        let own = |dl| PlannedList {
            node: i,
            dl,
            space: Some(i),
            world: worlds[i],
            list_id: None,
        };
        match part.flags & 0xF {
            0 => {
                if let Some((_, dl)) = part.dls[0] {
                    out.push(own(dl));
                }
            }
            1 => {
                if let Some((_, dl)) = part.dls[0] {
                    out.push(PlannedList {
                        node: i,
                        dl,
                        space: node.parent,
                        world: node.parent.map_or(Mat4::IDENTITY, |p| worlds[p]),
                        list_id: None,
                    });
                }
                out.push(own(part.dls[1].map_or(NO_LIST, |(_, dl)| dl)));
            }
            _ => {}
        }
    }
    out
}

/// Every stage render layer's `(file, graph_offset)` and its layer slot
/// (RE-245, RE-422).
///
/// `grDisplayLayer1PriProcDisplay`/`SecProcDisplay` set `G_ZBUFFER` and
/// `gDPSetRenderMode(G_RM_AA_ZB_OPA_SURF, G_RM_AA_ZB_OPA_SURF2)` unconditionally
/// before walking a stage's own layer-1 node lists -- see
/// [`ssb_rom::mesh::InitialMaterial::GROUND_LAYER1_EXTERNAL`] for the full
/// citation. Layers 0, 2 and 3 clear `G_ZBUFFER` and set a non-`ZB` render
/// mode on head 0, which is already [`ssb_rom::mesh::InitialMaterial::default`];
/// their `SecProcDisplay` sets `G_RM_AA_XLU_SURF` with `G_ZBUFFER` cleared on
/// head 1 ([`ssb_rom::mesh::Head1Seed::LayerXlu`]).
///
/// A stage controller's object on DL link 6 (`gcAddGObjDisplay(..., 6,
/// ...)`: Saffron City's gate, Yoshi's Island's clouds, Kongo Jungle's
/// barrel, Mushroom Kingdom's scales) draws right after layer 1 in the same
/// link, under the `G_ZBUFFER` and `G_RM_AA_ZB_OPA_SURF` that layer 1 set and
/// its lists restore, so it takes layer 1's seed too (RE-423). The N64
/// frame-414 RDRAM traces show every such list's triangles with `G_ZBUFFER`,
/// `Z_CMP` and, on head 0, `Z_UPD`.
pub fn ground_layer1_graphs(loaded: &Loaded) -> alloc::collections::BTreeMap<(u32, u32), u32> {
    let mut graphs: alloc::collections::BTreeMap<(u32, u32), u32> = loaded
        .stages
        .iter()
        .flat_map(|stage| &stage.layers)
        .map(|layer| (layer.graph, layer.index))
        .collect();
    for tree in ssb_rom::bonus2::TREES {
        graphs.insert((ssb_rom::bonus2::FILE, tree[0]), 1);
    }
    for asset in &ssb_rom::ground_obj::OBJECTS {
        if asset.dl_link != LAYER1_DL_LINK {
            continue;
        }
        let Some((file, _)) = loaded
            .stages
            .iter()
            .find(|g| g.file == asset.gr_file)
            .and_then(|g| g.map_nodes)
            .filter(|&(_, head)| head == asset.map_head)
        else {
            continue;
        };
        graphs.entry((file, asset.graph)).or_insert(1);
        if let Some(leaf) = asset.leaf {
            graphs.entry((file, leaf.dl)).or_insert(1);
        }
    }
    graphs
}

/// `dGRDisplayDescs[1].dl_link`: the display link stage layer 1 draws on.
pub const LAYER1_DL_LINK: u8 = 6;

/// Every loading-break transition scene's `(file, graph_offset)` (RE-246).
///
/// `ssb_rom::transition::ASSETS` already names each of the eleven
/// `dLBTransitionDescs` entries' exact file and graph offset (built for
/// `R0.13`'s framebuffer-capture work); `_(file, graph)` fields need no
/// further validation here, the same way [`ground_layer1_graphs`] trusts
/// `loaded.stages`.
pub fn lb_transition_graphs() -> alloc::collections::BTreeSet<(u32, u32)> {
    ssb_rom::transition::ASSETS
        .iter()
        .map(|asset| (asset.file, asset.graph))
        .collect()
}

/// `FTKIRBY_COPY_MODELPARTS_JOINT`: the joint Kirby's copy swaps.
pub const KIRBY_COPY_JOINT: u32 = 6;

/// Kirby's model file (`KirbyModel`).
pub const KIRBY_MODEL_FILE: u32 = 328;

/// `llKirbyMainMotionSpecialNFTKirbyCopy`: file 228's `FTKirbyCopy[27]`
/// table at 0x0 (`copy_id` u16, `copy_modelpart_id` s16, then eight bytes),
/// indexed by the copied fighter's kind.
pub const KIRBY_COPY_TABLE: (u32, u32) = (228, 0x0);
pub const KIRBY_COPY_ENTRY_SIZE: u32 = 12;

/// Every copy hat Kirby can wear (RE-417): the distinct nonzero
/// `copy_modelpart_id`s of the twelve playable fighters' `FTKirbyCopy`
/// rows, with joint 6's high-detail `FTModelPart` for each, when `graph` is
/// Kirby's model. Part 0 is his own head, the node's base mesh.
pub fn kirby_copy_hats(
    loaded: &Loaded,
    file: u32,
    graph: &ssb_rom::scene::SceneGraph,
) -> Vec<(u32, ssb_rom::fighter::ModelPart)> {
    let Some(entry) = ssb_rom::fighter::FIGHTER_FILES
        .iter()
        .find(|e| e.name == "Kirby")
    else {
        return Vec::new();
    };
    let (Some(main), Some(motion)) = (
        loaded.files.get(entry.file as usize),
        loaded.files.get(KIRBY_COPY_TABLE.0 as usize),
    ) else {
        return Vec::new();
    };
    let Some(detail) = ssb_rom::fighter::common_parts(&main, *entry)
        .iter()
        .position(|p| p.is_some_and(|p| (p.model_file, p.graph) == (file, graph.offset)))
    else {
        return Vec::new();
    };
    let mut parts: Vec<u32> = (0..12u32)
        .filter_map(|kind| {
            let at = (KIRBY_COPY_TABLE.1 + kind * KIRBY_COPY_ENTRY_SIZE + 2) as usize;
            let id = i16::from_be_bytes(motion.data.get(at..at + 2)?.try_into().ok()?);
            (id > 0).then_some(id as u32)
        })
        .collect();
    parts.sort_unstable();
    parts.dedup();
    parts
        .into_iter()
        .filter_map(|p| {
            let mp =
                ssb_rom::fighter::model_part(&main, *entry, KIRBY_COPY_JOINT, p, detail as u32)?;
            (mp.dl.0 == file).then_some((p, mp))
        })
        .collect()
}

/// The playable fighter whose high- or low-detail `FTCommonPart` is
/// `graph`, its main file and the detail (0 high, 1 low; RE-426).
pub fn playable_fighter_of(
    loaded: &Loaded,
    file: u32,
    graph: &ssb_rom::scene::SceneGraph,
) -> Option<(
    ssb_rom::fighter::FighterFile,
    alloc::rc::Rc<ssb_rom::archive::File>,
    u32,
)> {
    let fg = loaded.fighter_graphs.get(&(file, graph.offset))?;
    let main = loaded.files.get(fg.entry.file as usize)?;
    Some((fg.entry, main, fg.detail))
}

/// Joint `joint`'s parts `ftParamSetModelPartID` can give a playable
/// fighter outside its motion scripts: Link's shield in hand
/// (`ftCommonCatchWaitSetStatus`, and his make's defaults), Yoshi's closed
/// jaw, and Kirby's copy hats (RE-417).
pub fn code_model_parts(kind: u8) -> &'static [(i32, i32)] {
    match kind {
        // Link and Polygon Link (`ftManagerInitFighter`'s defaults).
        5 | 19 => &[(19, 0), (21, 0)],
        6 => &[(7, 1)],
        _ => &[],
    }
}

/// Every `(joint, part, FTModelPart)` a playable fighter can wear that the
/// pack needs (RE-425): the parts its motion scripts set
/// (`ssb_game::motion::model_part_events`), the code-set ones and Kirby's
/// copy hats, on joints `setup_parts` or a hidden part makes, high detail.
/// Part -1 hides a joint and needs no mesh.
pub fn fighter_model_parts(
    loaded: &Loaded,
    file: u32,
    graph: &ssb_rom::scene::SceneGraph,
) -> Vec<(u32, u32, ssb_rom::fighter::ModelPart)> {
    let Some((entry, main, detail)) = playable_fighter_of(loaded, file, graph) else {
        return Vec::new();
    };
    let Some(kind) = ssb_game::fighter::FighterKind::from_ordinal(entry.kind) else {
        return Vec::new();
    };
    // The joints `setup_parts` makes, and the hidden parts a motion makes
    // (`ftMainUpdateHiddenPartID`): Samus's grapple beam links take their
    // lists from model parts (RE-468).
    let present = ssb_game::modelpart::joint_masks(kind).map_or(0, |m| m.0)
        | ssb_game::modelpart::hidden_part_joints(kind)
            .iter()
            .fold(0u64, |m, &(_, root, _)| m | 1 << (root - 4));
    let mut wanted: alloc::collections::BTreeSet<(i32, i32)> =
        ssb_game::motion::model_part_events(kind);
    // A fighter wearing another's graph (Giant Donkey Kong Donkey Kong's,
    // Polygon Luigi Polygon Mario's) sets parts from its own scripts.
    for other in ssb_rom::fighter::FIGHTER_FILES
        .iter()
        .filter(|e| e.kind != 12 && e.kind != entry.kind)
    {
        let Some(other_main) = loaded.files.get(other.file as usize) else {
            continue;
        };
        let shares = ssb_rom::fighter::common_parts(&other_main, *other)
            .iter()
            .flatten()
            .any(|c| (c.model_file, c.graph) == (file, graph.offset));
        if let (true, Some(k)) = (
            shares,
            ssb_game::fighter::FighterKind::from_ordinal(other.kind),
        ) {
            wanted.extend(ssb_game::motion::model_part_events(k));
        }
    }
    wanted.extend(code_model_parts(entry.kind).iter().copied());
    if kind == ssb_game::fighter::FighterKind::Kirby {
        wanted.extend(
            kirby_copy_hats(loaded, file, graph)
                .into_iter()
                .map(|(p, _)| (KIRBY_COPY_JOINT as i32, p as i32)),
        );
    }
    wanted
        .into_iter()
        .filter_map(|(joint, part)| {
            let node = u32::try_from(joint)
                .ok()?
                .checked_sub(ssb_rom::fighter::JOINT_COMMON_START)?;
            let part = u32::try_from(part).ok()?;
            if node as usize >= graph.nodes.len() || present >> node & 1 == 0 {
                return None;
            }
            let mp = ssb_rom::fighter::model_part(&main, entry, joint as u32, part, detail)?;
            (mp.flags & 0xF == 0).then_some((joint as u32, part, mp))
        })
        .collect()
}

/// `plan` with node `node`'s own list replaced by `dl`, or `dl` inserted
/// for a node that has none, where the walk reaches that node.
pub fn plan_with_node_list(
    plan: &[PlannedList],
    graph: &ssb_rom::scene::SceneGraph,
    node: usize,
    dl: u32,
) -> (Vec<PlannedList>, usize) {
    let mut out: Vec<PlannedList> = plan.to_vec();
    if let Some(slot) = out.iter().position(|p| p.node == node && p.own_space()) {
        out[slot].dl = dl;
        return (out, slot);
    }
    let slot = out.iter().position(|p| p.node > node).unwrap_or(out.len());
    out.insert(
        slot,
        PlannedList {
            node,
            dl,
            space: Some(node),
            world: graph.world_transforms()[node],
            list_id: None,
        },
    );
    (out, slot)
}

/// `ftDisplayMainDrawAccessory`'s place in the walk: Jigglypuff's bow is
/// drawn just before its joint's own list, Pikachu's hat just after
/// (`ftDisplayMainDrawDefault`), both in the joint's matrix. The accessory
/// is planned as an extra node, `graph.nodes.len()`, whose materials the
/// caller appends.
pub fn plan_with_accessory(
    plan: &[PlannedList],
    graph: &ssb_rom::scene::SceneGraph,
    access: &ssb_rom::fighter::AccessPart,
    before: bool,
) -> Option<(Vec<PlannedList>, usize)> {
    let node = access
        .joint
        .checked_sub(ssb_rom::fighter::JOINT_COMMON_START)? as usize;
    let own = plan.iter().position(|p| p.node == node && p.own_space())?;
    let slot = if before { own } else { own + 1 };
    let mut out = plan.to_vec();
    out.insert(
        slot,
        PlannedList {
            node: graph.nodes.len(),
            dl: access.dl.1,
            space: Some(node),
            world: plan[own].world,
            list_id: None,
        },
    );
    Some((out, slot))
}

/// A fighter's headgear accessory (`FTAttributes::accesspart`, RE-425),
/// when `graph` is its high-detail model and the accessory's list and
/// materials live in the model file; `true` when it draws before its
/// joint's list (Jigglypuff) rather than after (Pikachu).
pub fn fighter_accessory(
    loaded: &Loaded,
    file: u32,
    graph: &ssb_rom::scene::SceneGraph,
) -> Option<(ssb_rom::fighter::AccessPart, bool)> {
    let (entry, main, _) = playable_fighter_of(loaded, file, graph)?;
    let access = ssb_rom::fighter::access_part(&main, entry)?;
    if access.dl.0 != file || access.mobjsubs.is_some_and(|(f, _)| f != file) {
        return None;
    }
    Some((access, entry.kind == 10))
}

/// A part's or accessory's own `MObj` chain, coloured at `costume` from its
/// `costume_matanim_joints` (`lbCommonAddMObjForFighterPartsDObj`).
pub fn part_materials(
    file: &ssb_rom::archive::File,
    mobjsubs: Option<(u32, u32)>,
    costume_matanim_joints: Option<(u32, u32)>,
    costume: u32,
) -> ssb_rom::mobj::NodeMaterials {
    let mut own = mobjsubs
        .filter(|&(f, _)| f == file.id)
        .and_then(|(_, at)| ssb_rom::mobj::read_chain(file, at))
        .unwrap_or_default();
    if let Some((_, scripts)) = costume_matanim_joints.filter(|&(f, _)| f == file.id) {
        let colors: Vec<_> = (0..own.len() as u32)
            .map(|m| {
                let script = file
                    .data
                    .get((scripts + 4 * m) as usize..(scripts + 4 * m + 4) as usize)
                    .map(|b| u32::from_be_bytes(b.try_into().unwrap()))
                    .filter(|&s| s != 0)?;
                ssb_rom::matanim::colors_at(&file.data, script, costume as f32).ok()
            })
            .collect();
        apply_costume_colors(file, &mut own, &colors);
    }
    own
}

/// DL link 10 effect trees drawn under the fighters' head-0 state: lists
/// that set no render mode of their own run under the
/// `G_RM_AA_ZB_OPA_SURF` with `G_ZBUFFER` that `ftDisplayMainProcDisplay`
/// leaves on head 0 before DL link 10's effects. The fighter seed is that
/// state. Each key is backed by an RDRAM display-list walk of an N64 frame
/// drawing it: the Arwing and the car (RE-425), Mario's entry pipe (shared
/// by Luigi), Samus's entry point, Donkey Kong's barrel and the rebirth
/// halo (RE-567). Their head-1 lists set `G_RM_AA_ZB_XLU_SURF` themselves.
/// The other DL link 10 effects' lists set their own render mode.
pub const FIGHTER_STATE_EFFECT_GRAPHS: &[(u32, u32)] = &[
    (161, 0x2C30),
    (350, 0x5FC0),
    (356, 0x0608),
    (349, 0x0B90),
    (355, 0x07C8),
    (85, 0x2AC0),
];

/// Weapon `DObjDesc` trees whose lists set no render mode, so they draw
/// under `wpDisplayDrawNormal`'s state (`G_ZBUFFER` cleared,
/// `G_RM_AA_XLU_SURF`). Kirby's Final Cutter wave (328 + 0x1D388) sets only
/// alpha compare and cycle type (RE-378). Pikachu's Thunder Jolt draws
/// under the same state: the aerial list (342 + 0x270) sets its own render
/// mode but not `G_ZBUFFER`, and the ground tree's lists (342 + 0x1888) set
/// neither (RE-379).
/// Ness's PK Fire spark (336 + 0x168), PK Thunder head (335 + 0x7C98) and
/// trail (335 + 0x8B40) draw the same way (RE-381).
/// Saffron's Razor Leaf (159 + 0x2A50) uses the same weapon wrapper (RE-430).
/// So do the Poké Ball Pokémon's rock, coin, swarm, Hydro Pump, Swift and
/// Smog (file 86, RE-435). Clefairy's swarm draws Clefairy's item tree under
/// its own `G_RM_AA_XLU_SURF` and is left with the item.
/// Link's Spin Attack band (324 + 0x11908, `dWPLinkSpinAttackWeaponDesc`
/// flags `DOBJDESC | DOBJLINKS`) draws its link-1 list through
/// `wpDisplayDObjTreeDLLinks`; the list (324 + 0x11680) sets neither a
/// render mode nor `G_ZBUFFER` (RE-540).
pub const WEAPON_SEEDED_GRAPHS: &[(u32, u32)] = &[
    (324, 0x11908),
    (328, 0x1D388),
    (342, 0x270),
    (342, 0x1888),
    (336, 0x168),
    (335, 0x7C98),
    (335, 0x8B40),
    (159, 0x2A50),
    (ssb_rom::mmonster::FILE, ssb_rom::mmonster::ROCK_GRAPH),
    (ssb_rom::mmonster::FILE, ssb_rom::mmonster::COIN_GRAPH),
    (
        ssb_rom::mmonster::FILE,
        ssb_rom::mmonster::SPEAR_SWARM_GRAPH,
    ),
    (ssb_rom::mmonster::FILE, ssb_rom::mmonster::HYDRO_GRAPH),
    (ssb_rom::mmonster::FILE, ssb_rom::mmonster::SWIFT_GRAPH),
    (ssb_rom::mmonster::FILE, ssb_rom::mmonster::SMOG_GRAPH),
];

/// `dEFManagerShieldEffectDesc`'s tree (`llFTManagerCommonShieldDObjDesc`,
/// file 163 + 0x300), drawn under `efManagerShieldProcDisplay`'s PRIM and
/// ENV (RE-384).
pub const SHIELD_GRAPH: (u32, u32) = (163, 0x300);

/// `dEFManagerImpactWaveEffectDesc`'s list (file 83 + 0x7C28), drawn by
/// `efManagerImpactWaveProcDisplay`: `G_RM_AA_ZB_XLU_SURF`, PRIM
/// (`dEFManagerImpactWavePrimColor*[index]`, the fading alpha) and ENV
/// (black), then `gcDrawDObjDLHead0`. The list sets its own combiner
/// (`(PRIM - ENV) * TEXEL0 + ENV`, alpha `TEXEL0 * PRIM`) but no render mode
/// before its triangles, so it draws translucent, depth-tested, without a
/// depth write. Unseeded, it drew opaque and hid the fighter it surrounds
/// (Samus's and Kirby's Charge Shot, Donkey Kong's Spinning Kong and Hand
/// Slap, landings).
pub const IMPACT_WAVE_GRAPH: (u32, u32) = (83, 0x7C28);

/// Ness's PK Thunder trail (`llNessMainPKThunderTrailWeaponAttributes`,
/// file 239 + 0x40): file 335's `DObjDLLink` array at 0x8B40, drawn by
/// `wpDisplayPKThunderProcDisplay` under PRIM and ENV by `trail_id`.
pub const PK_THUNDER_TRAIL_GRAPH: (u32, u32) = (335, 0x8B40);

/// Final Destination's boss comets (`llGRLastMapEffects0DObjDesc`, file
/// 114): `ssb_game::spgame::boss::EFFECT_ASSETS[0][0]`, drawn by
/// `SC1PGameBossWallpaper0ProcDisplay` under PRIM and ENV by `color_id`.
pub const BOSS_COMET_GRAPH: (u32, u32) = (114, 0x8960);

/// Final Destination's boss wallpaper trees whose display procs set the
/// render mode (and, for the Fall, the cycle type) before the lists, which
/// set neither (`sc1pgameboss.c`):
/// - `SC1PGameBossWallpaper1ProcDisplay` (`llGRLastMapEffects1DObjDesc`,
///   0xA188): `G_CYC_2CYCLE`, `G_RM_PASS, G_RM_AA_XLU_SURF2`, PRIM
///   (0, 0, 0, alpha), head 1;
/// - `SC1PGameBossWallpaper2ProcDisplay` (0xDD90): `G_RM_AA_XLU_SURF`,
///   PRIM (FF, FF, FF, alpha), head 1;
/// - `SC1PGameBossWallpaper3ProcDisplay0` (0x11268): the same, head 0.
///
/// Unseeded, all three drew opaque (RE-457). The renderer replaces the
/// PRIM alpha per effect.
pub const BOSS_WALLPAPER_GRAPHS: &[((u32, u32), bool, [u8; 3])] = &[
    ((114, 0xA188), true, [0, 0, 0]),
    ((114, 0xDD90), false, [0xFF; 3]),
    ((114, 0x11268), false, [0xFF; 3]),
];

/// `efManagerMakeEffect` effects whose own display sets head 1 to
/// `G_RM_AA_XLU_SURF` and `G_AC_NONE` before `gcDrawDObjDLLinksForGObj`
/// (no `Z_CMP`): the list sets no render mode of its own, so it blends with
/// no depth test ([`ssb_rom::mesh::Head1Seed::LayerXlu`]). Ness's PK Thunder
/// trail effect, `efManagerNessPKThunderTrailProcDisplay` (both
/// `dEFManagerNessPKThunderTrailEffectDesc` and the reflected trail's).
pub const EFFECT_XLU_GRAPHS: &[(u32, u32)] = &[(335, 0x9050)];

/// `efManagerMakeEffect` effects on DL link 15 or 18 whose display draws
/// the tree's `DObjDLLink` lists (`gcDrawDObjTreeDLLinksForGObj`): the
/// head-1 lists start under `efDisplayCLDProcDisplay`'s state, which every
/// priority-2 effect on those links inherits (RE-421). From
/// `ef/efmanager.c`'s `EFDesc` table: DamageSlash, DamageFlyMDust,
/// CatchSwirl, ReflectBreak, DeadExplode, ItemGetSwirl, Pikachu's Thunder
/// Shock, Kirby's Vulcan Jab and Final Cutter trail, Samus's grapple beam,
/// Jigglypuff's Sing, Ness's PSI Magnet and PK Thunder wave, and Link's
/// spin attack. The shield ([`SHIELD_GRAPH`]) is the same case with its
/// own ENV seed.
pub const EFFECT_CLD_GRAPHS: &[(u32, u32)] = &[
    (83, 0x7750),
    (83, 0xCAC8),
    (84, 0x2760),
    (84, 0x3398),
    (84, 0x53E8),
    (85, 0x3170),
    (347, 0x1640),
    (348, 0x0B20),
    (348, 0x0DF8),
    (349, 0x0380),
    (351, 0x2130),
    (352, 0x09A8),
    (335, 0x9A10),
    (353, 0x11C0),
];

/// `mesh::SequenceItem::stream` for the plain lists of the object at
/// `place`: 1 when the N64 draws that whole object on task head 1, by its
/// manager effect's `proc_display` or its weapon's `WPDesc.flags`
/// ([`ssb_rom::task_head`]). The ScaleX effects (RE-427, RE-443, RE-551),
/// Falcon Punch, Yoshi's entry egg and every unlinked weapon are;
/// `DObjDLLink` lists name their head themselves.
pub fn whole_object_stream(place: (u32, u32)) -> u8 {
    u8::from(ssb_rom::task_head::whole_head1_object(place).is_some())
}

/// [`whole_object_stream`] for a weapon converted outside the graph walk:
/// `place` must be a whole-head-1 weapon's `WPAttributes.data`, so a
/// block keyed to the wrong place fails the build instead of drawing on
/// the wrong head.
pub fn weapon_stream(place: (u32, u32)) -> u8 {
    assert_eq!(
        ssb_rom::task_head::whole_head1_object(place),
        Some(ssb_rom::task_head::Head1Object::Weapon),
        "{place:?} is not a whole-head-1 weapon's object"
    );
    1
}

/// Weapons whose `WPAttributes.data` is a direct display list with its own
/// `anim_joints`, so they need a one-node graph for the animation to bind
/// to, as [`DIRECT_MANAGER_EFFECT_ASSETS`] gives direct effects. Pikachu's
/// aerial Thunder Jolt (`llPikachuSpecial1ThunderJoltAirWeaponAttributes`,
/// file 244 + 0x00) names file 342's list at 0x270 (RE-379). Ness's PK Fire
/// spark (`llNessSpecial1PKFireWeaponAttributes`, file 240 + 0x00) names
/// file 336's list at 0x168, and the PK Thunder trail
/// (`llNessMainPKThunderTrailWeaponAttributes`, file 239 + 0x40) file 335's
/// `DObjDLLink` array at 0x8B40 (RE-381).
pub const DIRECT_WEAPON_GRAPHS: &[(u32, u32)] = &[(342, 0x270), (336, 0x168), (335, 0x8B40)];

/// `WPAttributes.p_matanim_joints` for weapon trees, keyed by graph. The
/// ground Thunder Jolt (file 244 + 0x34) names file 342's table at 0x1AE0
/// (RE-379).
/// The PK Fire spark (file 240 + 0x00) names file 336's 0x1E0 and the PK
/// Thunder head (file 239 + 0x0C) file 335's 0x7D70 (RE-381).
/// The Hydro Pump (file 251 + 0xA50), the Smog (+ 0xC40) and Beedrill's
/// swarm (+ 0x9D4) name file 86's 0xFB70, 0x131E0 and 0xE560 (RE-435).
pub const WEAPON_MAT_ANIM_JOINTS: &[((u32, u32), u32)] = &[
    ((342, 0x1888), 0x1AE0),
    // Link's Spin Attack weapon graphic (file 225 + 0x0C) names file 324's
    // 0x11A40: node 1's three `MObj`s fade in and out.
    ((324, 0x11908), 0x11A40),
    ((336, 0x168), 0x1E0),
    ((335, 0x7C98), 0x7D70),
    (
        (ssb_rom::mmonster::FILE, ssb_rom::mmonster::HYDRO_GRAPH),
        ssb_rom::mmonster::HYDRO_MAT_ANIM_JOINTS,
    ),
    (
        (ssb_rom::mmonster::FILE, ssb_rom::mmonster::SMOG_GRAPH),
        ssb_rom::mmonster::SMOG_MAT_ANIM_JOINTS,
    ),
    (
        (
            ssb_rom::mmonster::FILE,
            ssb_rom::mmonster::SPEAR_SWARM_GRAPH,
        ),
        ssb_rom::mmonster::SPEAR_SWARM_MAT_ANIM_JOINTS,
    ),
];

/// Where an item's material script comes from (RE-442).
#[derive(Clone, Copy)]
pub enum ItemMatTarget {
    /// `ITAttributes.p_matanim_joints`, which `itManagerMakeItem` hands to
    /// `gcAddAnimAll`.
    Table(u32),
    /// One script a status adds to the root's first `MObj`
    /// (`gcAddMObjMatAnimJoint(dobj->mobj, ...)`).
    Root(u32),
    /// One script a status adds to a packed node's first `MObj`: a
    /// Pokémon's root (node 1) or its child (node 2) (RE-443).
    Node(usize, u32),
}

/// The item root once `itManagerMakeItem` ejects descriptor 0.
pub const ITEM_ROOT_NODE: usize = 1;

/// File 86's items with material scripts, by tree (`ITAttributes.data`).
pub const ITEM_MAT_ANIMS: &[((u32, u32), ItemMatTarget)] = &[
    // Star (251 + 0x148): the palette flicker of both root `MObj`s.
    ((86, 0x1560), ItemMatTarget::Table(0x15F0)),
    // Ray Gun (251 + 0x2E4): the barrel's palette.
    ((86, 0x46B0), ItemMatTarget::Table(0x4760)),
    // `itBombHeiWalkInitVars`: `llITCommonDataBombHeiWalkMatAnimJoint`.
    ((86, 0x33F8), ItemMatTarget::Root(0x35B8)),
    // `it{G,R}ShellSpinAddAnim`: `llITCommonDataShellMatAnimJoint`.
    ((86, 0x5F88), ItemMatTarget::Root(0x6048)),
];

/// A tree's item material script: [`ITEM_MAT_ANIMS`], or a Pokémon status's
/// `gcAddMObjMatAnimJoint` (`ssb_rom::mmonster::Visual::status_mat`,
/// RE-443).
pub fn item_mat_target(key: (u32, u32)) -> Option<ItemMatTarget> {
    ITEM_MAT_ANIMS
        .iter()
        .find(|(k, _)| *k == key)
        .map(|&(_, target)| target)
        .or_else(|| {
            ssb_rom::mmonster::VISUALS
                .iter()
                .filter(|v| (ssb_rom::mmonster::FILE, v.graph) == key)
                .find_map(|v| v.status_mat)
                .map(|(script, node)| ItemMatTarget::Node(node as usize, script))
        })
}

/// The seed a [`ssb_rom::mesh::convert_sequence`] call for `(file,
/// graph_offset)` must use -- see [`fighter_skeleton_graphs`],
/// [`ground_layer1_graphs`] and [`lb_transition_graphs`].
/// The interface wrappers set PRIM before entering these ROM display lists.
pub fn seed_player_interface_commands(key: (u32, u32), decoded: &mut [Vec<ssb_rom::dl::Cmd>]) {
    use ssb_rom::{dl::Cmd, player_interface as asset};
    if key == (86, ssb_rom::mmonster::PIPPI_SWARM_GRAPH) {
        // itPippiCommonProcDisplay, before the list's own writes.
        if let Some(cmds) = decoded.first_mut() {
            cmds.insert(
                0,
                Cmd::SetOtherModeL {
                    shift: 3,
                    len: 29,
                    data: 0x0055_3078,
                },
            );
        }
        return;
    }
    if key == IMPACT_WAVE_GRAPH {
        // efManagerImpactWaveProcDisplay, before the list's own writes:
        // index 4 (every fighter's wave, `ftParamMakeEffect`) is white, the
        // alpha starts at 255 and the renderer replaces it per frame.
        if let Some(cmds) = decoded.first_mut() {
            cmds.splice(
                0..0,
                [
                    Cmd::SetOtherModeL {
                        shift: 3,
                        len: 29,
                        data: 0x0050_49D8, // G_RM_AA_ZB_XLU_SURF, SURF2
                    },
                    Cmd::SetPrimColor {
                        m: 0,
                        l: 0,
                        rgba: [0xFF; 4],
                    },
                    Cmd::SetEnvColor([0, 0, 0, 0xFF]),
                ],
            );
        }
        return;
    }
    if key == PK_THUNDER_TRAIL_GRAPH {
        // wpDisplayPKThunderProcDisplay, before the list's own writes: the
        // list's `(PRIM - ENV) * TEXEL0 + ENV` reads both and sets neither
        // (its `MObj` flags are `MOBJ_FLAG_ALPHA` only). Trail 0's pair; the
        // renderer replaces both per `trail_id`.
        use ssb_game::weapon::{PK_THUNDER_TRAIL_ENV as ENV, PK_THUNDER_TRAIL_PRIM as PRIM};
        if let Some(cmds) = decoded.first_mut() {
            let [r, g, b] = PRIM[0];
            let [er, eg, eb] = ENV[0];
            cmds.splice(
                0..0,
                [
                    Cmd::SetPrimColor {
                        m: 0,
                        l: 0,
                        rgba: [r, g, b, 0xFF],
                    },
                    Cmd::SetEnvColor([er, eg, eb, 0xFF]),
                ],
            );
        }
        return;
    }
    if let Some(&(_, two_cycle, [r, g, b])) = BOSS_WALLPAPER_GRAPHS.iter().find(|(k, ..)| *k == key)
    {
        if let Some(cmds) = decoded.first_mut() {
            let mut seed = Vec::with_capacity(3);
            if two_cycle {
                seed.push(Cmd::SetOtherModeH {
                    shift: 20,
                    len: 2,
                    data: 0x0010_0000, // G_CYC_2CYCLE
                });
            }
            seed.push(Cmd::SetOtherModeL {
                shift: 3,
                len: 29,
                // G_RM_PASS, G_RM_AA_XLU_SURF2; G_RM_AA_XLU_SURF, SURF2
                data: if two_cycle { 0x0C18_41C8 } else { 0x0050_41C8 },
            });
            seed.push(Cmd::SetPrimColor {
                m: 0,
                l: 0,
                rgba: [r, g, b, 0xFF],
            });
            cmds.splice(0..0, seed);
        }
        return;
    }
    if key == BOSS_COMET_GRAPH {
        // SC1PGameBossWallpaper0ProcDisplay, before the tree's lists: PRIM
        // white at the effect's alpha and ENV the comet's colour
        // (`dSC1PGameBossCometEnvColor*[color_id]`). The lists'
        // `(PRIM - ENV) * TEXEL0 + ENV` reads both and sets neither, and no
        // `MObj` sets either register. Colour 0's pair; the renderer
        // replaces both per effect.
        use ssb_game::spgame::boss::COMET_ENV_COLORS as ENV;
        if let Some(cmds) = decoded.first_mut() {
            let [er, eg, eb] = ENV[0];
            cmds.splice(
                0..0,
                [
                    Cmd::SetPrimColor {
                        m: 0,
                        l: 0,
                        rgba: [0xFF; 4],
                    },
                    Cmd::SetEnvColor([er, eg, eb, 0xFF]),
                ],
            );
        }
        return;
    }
    if key.0 != asset::FILE {
        return;
    }
    let Some(cmds) = decoded.first_mut() else {
        return;
    };
    let rgba = match key.1 {
        asset::ARROWS => {
            // `dIFCommonPlayerArrowsDisplayList`: G_CC_PRIMITIVE in both cycles.
            cmds.insert(
                0,
                Cmd::SetCombine {
                    hi: 0x00FF_FFFF,
                    lo: 0xFFFD_F6FB,
                },
            );
            [255, 0, 0, 128]
        }
        asset::POINTER => [255; 4], // dynamic player colour at display time
        _ => return,
    };
    cmds.insert(0, Cmd::SetPrimColor { m: 0, l: 0, rgba });
}

pub fn initial_material_for(
    skeleton_graphs: &alloc::collections::BTreeSet<(u32, u32)>,
    ground_layer1_graphs: &alloc::collections::BTreeMap<(u32, u32), u32>,
    lb_transition_graphs: &alloc::collections::BTreeSet<(u32, u32)>,
    file: u32,
    graph_offset: u32,
) -> ssb_rom::mesh::InitialMaterial {
    if file == ssb_rom::player_interface::FILE {
        ssb_rom::mesh::InitialMaterial {
            lit: false,
            translucent: graph_offset == ssb_rom::player_interface::ARROWS,
            ..ssb_rom::mesh::InitialMaterial::SCENE
        }
    } else if skeleton_graphs.contains(&(file, graph_offset))
        || FIGHTER_STATE_EFFECT_GRAPHS.contains(&(file, graph_offset))
    {
        ssb_rom::mesh::InitialMaterial::FIGHTER_EXTERNAL
    } else if ground_layer1_graphs.get(&(file, graph_offset)) == Some(&1) {
        ssb_rom::mesh::InitialMaterial::GROUND_LAYER1_EXTERNAL
    } else if ground_layer1_graphs.contains_key(&(file, graph_offset)) {
        ssb_rom::mesh::InitialMaterial {
            head1: ssb_rom::mesh::Head1Seed::LayerXlu,
            ..ssb_rom::mesh::InitialMaterial::SCENE
        }
    } else if lb_transition_graphs.contains(&(file, graph_offset))
        || ssb_rom::opening::CAMERA_DEFAULT_GRAPHS.contains(&(file, graph_offset))
    {
        // The opening's lists run under the camera's untouched default too
        // (RE-467).
        ssb_rom::mesh::InitialMaterial::LB_TRANSITION_EXTERNAL
    } else if ssb_rom::opening::HEAD1_DL_GRAPHS.contains(&(file, graph_offset)) {
        // The newcomers' lists run under the camera's head-1 default,
        // `G_RM_AA_ZB_XLU_SURF` (RE-467).
        ssb_rom::mesh::InitialMaterial {
            depth_test: true,
            depth_write: false,
            depth_mode: ssb_rom::mesh::ZMode::Translucent,
            translucent: true,
            ..ssb_rom::mesh::InitialMaterial::SCENE
        }
    } else if WEAPON_SEEDED_GRAPHS.contains(&(file, graph_offset))
        || (ssb_rom::task_head::whole_head1_object((file, graph_offset))
            == Some(ssb_rom::task_head::Head1Object::Weapon)
            && ssb_rom::task_head::whole_head1_effect((file, graph_offset)).is_none())
    {
        // Every whole-head-1 weapon's head 1 starts under
        // `wpDisplayDrawNormal` (RE-379); an object a manager effect also
        // draws keeps the effect's seed (its weapon copy is converted
        // separately where the two differ).
        ssb_rom::mesh::InitialMaterial::WEAPON_EXTERNAL
    } else if (file, graph_offset) == SHIELD_GRAPH {
        ssb_rom::mesh::InitialMaterial::SHIELD_EXTERNAL
    } else if EFFECT_XLU_GRAPHS.contains(&(file, graph_offset)) {
        ssb_rom::mesh::InitialMaterial {
            head1: ssb_rom::mesh::Head1Seed::LayerXlu,
            ..ssb_rom::mesh::InitialMaterial::SCENE
        }
    } else if EFFECT_CLD_GRAPHS.contains(&(file, graph_offset))
        || ssb_rom::task_head::whole_head1_effect((file, graph_offset)).is_some_and(|e| e.cld)
    {
        ssb_rom::mesh::InitialMaterial {
            head1: ssb_rom::mesh::Head1Seed::EffectCld,
            ..ssb_rom::mesh::InitialMaterial::SCENE
        }
    } else {
        ssb_rom::mesh::InitialMaterial::SCENE
    }
}

/// A [`ssb_rom::mesh::SequenceItem::depth_seed`] for one `PlannedList` entry,
/// when its graph's own [`initial_material_for`] result and its own
/// `list_id` call for one (RE-250).
///
/// `GROUND_LAYER1_EXTERNAL` seeds every render-layer-1 node with task list
/// 0's `Z_CMP | Z_UPD | ZMODE_OPA` -- correct for the 142 of 163 `DObjDLLink`
/// entries that really are on list 0, but not the 21 on list 1, which
/// `grDisplayLayer1{Pri,Sec}ProcDisplay` resets separately to
/// `G_RM_AA_ZB_XLU_SURF` (`Z_CMP | !Z_UPD | ZMODE_XLU`, blending; RE-323; see
/// [`ssb_rom::mesh::SequenceItem::depth_seed`]'s own doc comment). No other seed (`FIGHTER_EXTERNAL`, `LB_TRANSITION_EXTERNAL`)
/// has a second task list to correct for.
pub fn ground_layer1_list1_depth_seed(
    initial: ssb_rom::mesh::InitialMaterial,
    list_id: Option<u32>,
) -> Option<(bool, bool, ssb_rom::mesh::ZMode)> {
    (initial == ssb_rom::mesh::InitialMaterial::GROUND_LAYER1_EXTERNAL && list_id == Some(1))
        .then_some((true, false, ssb_rom::mesh::ZMode::Translucent))
}

/// Converts one graph's whole plan under a given per-node materials array
/// (RE-098): the same shape [`pack`]'s own main loop builds inline for
/// costume 0, factored out so a fighter's alternate costumes can call it
/// again with a different materials array without duplicating the
/// decode/`SequenceItem`/`convert_sequence` plumbing.
///
/// A plain function rather than a closure on purpose: it needs `&mut
/// mat_anim_data` only for the duration of one call
/// (`resolve_layer_mat_anims` returns an owned `Vec`, retaining no borrow),
/// and a closure capturing that mutably for its own lifetime would keep the
/// borrow alive across every call site using it -- including the later
/// `pack_mesh` calls that need `&mat_anim_data` immutably in the same scope.
#[allow(clippy::too_many_arguments)]
pub fn convert_graph_at(
    ctx: &PackCtx<'_>,
    loaded: &Loaded,
    file: &ssb_rom::archive::File,
    graph_offset: u32,
    plan: &[PlannedList],
    materials: &[ssb_rom::mobj::NodeMaterials],
    mat_anim_data: &mut BTreeMap<ssb_rom::mesh::MatAnimRef, MatAnimData>,
    initial: ssb_rom::mesh::InitialMaterial,
) -> Vec<Result<ssb_rom::mesh::Mesh, ssb_rom::mesh::MeshError>> {
    use ssb_rom::mesh;

    let mut decoded: Vec<Vec<ssb_rom::dl::Cmd>> = plan
        .iter()
        .map(|p| {
            file.data
                .get(p.dl as usize..)
                .and_then(|d| ssb_rom::dl::decode_list_at(d, p.dl).ok())
                .unwrap_or_default()
        })
        .collect();
    seed_player_interface_commands((file.id, graph_offset), &mut decoded);
    let mat_anims =
        resolve_layer_mat_anims(ctx, loaded, file, graph_offset, materials, mat_anim_data);
    let items: Vec<mesh::SequenceItem> = plan
        .iter()
        .zip(&decoded)
        .map(|(p, cmds)| mesh::SequenceItem {
            cmds,
            world: p.world,
            mobjs: &materials[p.node],
            mat_anims: &mat_anims[p.node],
            depth_seed: ground_layer1_list1_depth_seed(initial, p.list_id),
            stream: u8::from(p.list_id == Some(1)),
        })
        .collect();
    let mut converted = mesh::convert_sequence(&items, mesh::Source::of(file), initial);
    for m in converted.iter_mut().filter_map(|m| m.as_mut().ok()) {
        map_vertex_bindings(m, plan);
    }
    converted
}
