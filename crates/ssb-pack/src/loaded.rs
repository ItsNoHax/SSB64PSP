//! Every archive file decoded once, with its scene graphs.

use crate::*;

pub struct Loaded {
    pub files: Vec<Option<ssb_rom::archive::File>>,
    pub graphs: BTreeMap<u32, Vec<ssb_rom::scene::SceneGraph>>,
    pub tables: ssb_rom::mobj::PartTables,
    pub stages: Vec<ssb_rom::stage::GroundData>,
    /// Every playable fighter's two `FTCommonPart` graphs (RE-426), keyed
    /// `(model file, graph offset)`.
    pub fighter_graphs: BTreeMap<(u32, u32), FighterGraph>,
}

/// One of a playable fighter's `FTCommonPart` graphs (RE-426).
#[derive(Debug, Clone)]
pub struct FighterGraph {
    pub entry: ssb_rom::fighter::FighterFile,
    /// 0 for `nFTPartsDetailHigh`, 1 for `nFTPartsDetailLow`.
    pub detail: u32,
    /// The high-detail graph's offset (this one's at high detail).
    pub high: u32,
    /// Low detail: the nodes whose descriptor has no list, which
    /// `lbCommonSetupFighterPartsDObjs` gives the high-detail list and
    /// `MObj`s (`commonparts[1].dobjdesc[i].dl == NULL`). `load_all` has
    /// already put the list on the graph's node.
    pub fallback: Vec<usize>,
}

pub fn load_all(archive: &ssb_rom::archive::Archive) -> Loaded {
    use ssb_rom::{mobj, scene};

    let files: Vec<Option<ssb_rom::archive::File>> = (0..archive.len() as u32)
        .map(|id| archive.load(id).ok())
        .collect();
    let mut graphs: BTreeMap<u32, Vec<scene::SceneGraph>> = files
        .iter()
        .flatten()
        .map(|f| (f.id, scene::find_scene_graphs(f)))
        .collect();
    for &(file, graph) in DIRECT_WEAPON_GRAPHS.iter().chain(
        &[
            (
                ssb_rom::player_interface::FILE,
                ssb_rom::player_interface::POINTER,
            ),
            // `mvEndingMakeRoomTissues`: one `DObj` on the bare list, which its
            // joint animation moves.
            (ssb_rom::ending::ROOM_FILE, ssb_rom::ending::ROOM_TISSUES),
        ]
        .into_iter()
        .chain(
            // The opening's lists on one `DObj` each (RE-467).
            ssb_rom::opening::DL_GRAPHS
                .iter()
                .map(|&(file, dl, _)| (file, dl)),
        )
        .collect::<Vec<_>>(),
    ) {
        let file_graphs = graphs.entry(file).or_default();
        if !file_graphs.iter().any(|g| g.offset == graph) {
            file_graphs.push(scene::SceneGraph {
                offset: graph,
                nodes: vec![scene::DObjNode {
                    desc: scene::DObjDesc {
                        id: 0,
                        dl: Some(graph),
                        translate: [0.0; 3],
                        rotate: [0.0; 3],
                        scale: [1.0; 3],
                    },
                    parent: None,
                }],
            });
        }
    }
    for asset in DIRECT_MANAGER_EFFECT_ASSETS {
        let file_graphs = graphs.entry(asset.file).or_default();
        if file_graphs.iter().any(|graph| graph.offset == asset.graph) {
            continue;
        }
        file_graphs.push(scene::SceneGraph {
            offset: asset.graph,
            nodes: vec![scene::DObjNode {
                desc: scene::DObjDesc {
                    id: 0,
                    dl: Some(asset.graph),
                    translate: [0.0; 3],
                    rotate: [0.0; 3],
                    scale: [1.0; 3],
                },
                parent: None,
            }],
        });
    }
    // A record only counts if a graph really starts where it points *and* the
    // table it names parses for that graph's node count; see `PartTables::scan`.
    let mut tables = mobj::PartTables::scan(files.iter().flatten(), |model, graph, table| {
        let Some(nodes) = graphs
            .get(&model)
            .and_then(|gs| gs.iter().find(|g| g.offset == graph))
            .map(|g| g.nodes.len())
        else {
            return false;
        };
        files[model as usize]
            .as_ref()
            .is_some_and(|f| mobj::read_table(f, table, nodes).is_some())
    });
    // `sc1PBonusStageInitPlatforms` attaches these exact MObj tables to
    // descriptor 0 (a child of the existing yakumono, not an empty root).
    for tree in ssb_rom::bonus2::TREES.iter().take(3) {
        let graph = graphs
            .get(&ssb_rom::bonus2::FILE)
            .and_then(|gs| gs.iter().find(|g| g.offset == tree[0]))
            .expect("platform graph absent");
        assert!(
            files[ssb_rom::bonus2::FILE as usize]
                .as_ref()
                .is_some_and(|f| mobj::read_table(f, tree[2], graph.nodes.len()).is_some()),
            "platform material table invalid"
        );
        tables.insert(ssb_rom::bonus2::FILE, tree[0], tree[2]);
    }
    // Stage layers name their table through `MPGroundDesc`, which puts it two
    // words after the graph rather than one. Same idea, different struct.
    let is_graph = |file: u32, offset: u32| {
        graphs
            .get(&file)
            .is_some_and(|gs| gs.iter().any(|g| g.offset == offset))
    };
    let stages: Vec<ssb_rom::stage::GroundData> = files
        .iter()
        .flatten()
        .flat_map(|f| ssb_rom::stage::find_ground_data(f, is_graph))
        .collect();

    // Stage controller objects the controller does not build from a
    // `DObjDesc` array (RE-365): one node per label, as for the direct
    // manager effects above. Labels are offsets into the file `map_nodes`
    // lands in, once it lands on the label the controller subtracts.
    let mut leaf_tables = Vec::new();
    for asset in &ssb_rom::ground_obj::OBJECTS {
        let Some((file, _)) = stages
            .iter()
            .find(|g| g.file == asset.gr_file)
            .and_then(|g| g.map_nodes)
            .filter(|&(_, head)| head == asset.map_head)
        else {
            continue;
        };
        let mut one = |offset: u32, id: u32, dl: Option<u32>| {
            let file_graphs = graphs.entry(file).or_default();
            if file_graphs.iter().all(|graph| graph.offset != offset) {
                file_graphs.push(scene::SceneGraph {
                    offset,
                    nodes: vec![scene::DObjNode {
                        desc: scene::DObjDesc {
                            id,
                            dl,
                            translate: [0.0; 3],
                            rotate: [0.0; 3],
                            scale: [1.0; 3],
                        },
                        parent: None,
                    }],
                });
            }
        };
        match asset.build {
            ssb_rom::ground_obj::Build::Desc => {}
            ssb_rom::ground_obj::Build::Dl => one(asset.graph, 0, Some(asset.graph)),
            ssb_rom::ground_obj::Build::Empty => one(asset.graph, 0, None),
        }
        if let Some(leaf) = asset.leaf {
            one(leaf.dl, ssb_rom::ground_obj::LEAF_DESC_ID, Some(leaf.dl));
            leaf_tables.push((file, leaf.dl, leaf.mobjsub));
        }
    }

    for (file, graph, table) in leaf_tables {
        let parses = files[file as usize]
            .as_ref()
            .is_some_and(|f| mobj::read_table(f, table, 1).is_some());
        if parses {
            tables.insert(file, graph, table);
        }
    }
    for &(file, graph, table) in DIRECT_MANAGER_EFFECT_MOBJ_PAIRS {
        let nodes = graphs
            .get(&file)
            .and_then(|gs| gs.iter().find(|g| g.offset == graph))
            .map_or(0, |g| g.nodes.len());
        let parses = files[file as usize]
            .as_ref()
            .is_some_and(|f| mobj::read_table(f, table, nodes).is_some());
        if parses {
            tables.insert(file, graph, table);
        }
    }
    for layer in stages.iter().flat_map(|s| &s.layers) {
        let Some((table_file, table)) = layer.mobjsub_table else {
            continue;
        };
        let (graph_file, graph) = layer.graph;
        // A layer whose table lives in another file cannot be followed; the
        // chain reader works within one file.
        if table_file != graph_file {
            continue;
        }
        let nodes = graphs
            .get(&graph_file)
            .and_then(|gs| gs.iter().find(|g| g.offset == graph))
            .map_or(0, |g| g.nodes.len());
        let parses = files[graph_file as usize]
            .as_ref()
            .is_some_and(|f| mobj::read_table(f, table, nodes).is_some());
        if parses {
            tables.insert(graph_file, graph, table);
        }
    }

    // A fighter's per-move entrance effects (the flash/warp-in a special
    // character does when a match starts) are named by an `EFDesc` record —
    // a *third* shape, distinct from `FTCommonPart` and `MPGroundDesc`, with
    // the same adjacent `DObjDesc*`/`MObjSub***` fields (`o_dobjsetup` then
    // `o_mobjsub`, `refs/ssb-decomp-re/src/ef/eftypes.h:11-24`). Unlike those
    // two, the `EFDesc` instances themselves live in the game's static
    // executable data, not in any relocData archive file, so there is no
    // `file.extern_relocs` entry for `PartTables::scan` to ever find — no
    // scan of the archive can discover this pairing, only reading the
    // decompilation can (RE-059). Hand-entered from
    // `refs/ssb-decomp-re/src/ef/efmanager.c`'s
    // `dEFManagerLinkEntryWaveEffectDesc`/`dEFManagerLinkEntryBeamEffectDesc`,
    // cross-checked against `refs/ssb-decomp-re/src/relocData/353_LinkSpecial2.c`'s
    // own offset comments. Link's third file-353 graph (`SpinAttackDObjDesc`
    // @ 0x11C0) is named by a `WPAttributes` instead
    // (`refs/ssb-decomp-re/src/wp/wplink/wplinkspinattack.c`), but that
    // struct instance is not yet typed in the decompilation, so its
    // `p_mobjsubs` field cannot be read from source; deliberately not
    // inserted here until it can be confirmed non-null.
    //
    // The opening movie's room scene (`refs/ssb-decomp-re/src/mv/mvopening/mvopeningroom.c`)
    // pairs its five graphs a *fourth* way: `gcSetupCommonDObjs(gobj, dobjdesc)`
    // followed by a separate `gcAddMObjAll(gobj, mobjsub)` call on the same
    // `GObj`, both against symbols read straight out of `52_MVCommon.c`. There
    // is no struct in memory linking them at all — the pairing exists only in
    // the sequence of two calls in the executable's code, not as data — so
    // this is a fourth reason `PartTables::scan` can never discover a pairing
    // and has to be told by hand (RE-060). Every `gcSetupCommonDObjs` call in
    // that file was checked against a following `gcAddMObjAll` on the same
    // `gobj`; these five have one; `RoomDesk`/`RoomTissues`/etc. do not and
    // are correctly left unpaired.
    //
    // RE-077: the same category as file 86's still-blocked case (
    // R0.7, RE-061) -- a pairing `PartTables::scan` cannot find
    // structurally, only demand-matching `--search` can even suggest -- but
    // where file 86's search stayed at 27 ambiguous candidates with nothing
    // typed to confirm any of them, this one stayed at exactly one. A raw,
    // unlinked
    // `MObjSub**` array that exists in the decompilation, fully typed, but
    // is never the direct target of a pointer field adjacent to its
    // `DObjDesc` array the way `FTCommonPart`/`MPGroundDesc` are — nothing
    // in the source or the archive's own relocations names the connection,
    // only the two symbols' *addresses* being close enough for
    // `ssb_rom::mobj::search_tables`'s demand-length matching (anchored to
    // real intern-relocation pointer slots, not a blind byte scan) to find
    // it. Kirby's `JointTree_0x19F08`
    // graph (22 real nodes) was one of R0.7's untraced 64 (RE-076); the
    // search found exactly one candidate, 0x18D60 — confirmed against
    // `refs/ssb-decomp-re/src/relocData/328_KirbyModel.c:7254`'s
    // `dKirbyModel_gap_0x31CC_sub_0x15894_post[24]`, a real, fully-typed
    // `MObjSub **` array whose 24 slots span 0x18D58..0x18DB8: 0x18D60 is
    // exactly slot 2, and slots [2..24) are exactly 22 entries -- the graph's
    // own node count. Not a coincidence of overlapping ranges; the slot
    // count, the node count and the per-slot NULL/non-NULL pattern all agree
    // with the graph's own per-node demand. The other 4 graphs this session
    // found unpaired in the same file (`--search`, all 10-way ambiguous, not
    // inserted) have candidate sets overlapping this same array's later
    // slots, suggesting they may be further sub-ranges of it -- worth
    // revisiting with the same cross-check once each has a genuinely unique
    // candidate, not guessed at here.
    //
    // RE-078: the same search-plus-decomp-cross-check approach run over
    // every remaining unpaired graph archive-wide, not just Kirby's file.
    // `ssb_rom::mobj::search_tables` found 13 (of 63) with exactly one
    // candidate; each of the following 6 was independently confirmed
    // against a precisely-matching, fully-typed decompiled symbol (address
    // and entry count both agreeing with the graph's own node count) before
    // being inserted -- the other 7 unique hits either matched nothing typed
    // in the decompilation (still just raw bytes there) or, on inspection,
    // turned out to be a substring coincidence in a symbol name rather than
    // a real address match, and are left unfixed rather than guessed at.
    for (file, graph, table) in [
        // Both GBumper (file 251 + 0xCF0) and NBumper (+ 0x69C) name
        // 0x7648 exactly. 0x7BE8 is a separate tree, not a later start
        // of that graph (correction to RE-162, measured in RE-429).
        (86u32, 0x7648u32, 0x7488u32), // GBumper / NBumper ItemAttributes
        (86u32, 0x7BE8u32, 0x7488u32), // Legacy neighboring-tree pairing, RE-162
        // GRInishieMap's Pakkun ItemAttributes (file 260 + 0x120) names the
        // `DObjDesc` array at StageInishieFile3 0xC30 and its `MObjSub ***`
        // table at 0xA68, which `itManagerMakeItem` hands to
        // `gcSetupCustomDObjsWithMObj` (RE-429).
        (155u32, 0xC30u32, 0xA68u32), // Pakkun ItemAttributes
        (159, 0x1990, 0x17D0),        // Charmander ItemAttributes
        (159, 0x2340, 0x2180),        // Venusaur ItemAttributes
        // LinkModel's `JointTree_0x9CF8` uses the raw MObjSub** dispatch at
        // 0x84B8.  Its three leading NULL slots match the root
        // Joint_0x93B8's zero graphics-heap demand; slots 3 and 4 name the
        // 0x86C0/0x86D0 material chains that LinkMain's FTModelPart records
        // assign to Joint_0x94F0 and Joint_0x9B98.  Record the true source
        // start (0x84B8), rather than demand search's first nonzero-demand
        // slot at 0x84C0.
        (324u32, 0x9CF8u32, 0x84B8u32),  // LinkModel passive-part tree
        (353u32, 0x3F8u32, 0x130u32),    // LinkSpecial2 EntryWave
        (353u32, 0x7B8u32, 0x4F0u32),    // LinkSpecial2 EntryBeam
        (52u32, 0x7E98u32, 0x42F8u32),   // MVCommon RoomBackground
        (52u32, 0x1C4A8u32, 0x1BC60u32), // MVCommon RoomLogo
        (52u32, 0x1DF28u32, 0x1DCA0u32), // MVCommon RoomCloseUpEffectAir
        (52u32, 0x1F270u32, 0x1F0F8u32), // MVCommon RoomCloseUpEffectGround
        (52u32, 0x22440u32, 0x20480u32), // MVCommon RoomDeskGround
        (328u32, 0x19F08u32, 0x18D60u32), // KirbyModel JointTree_0x19F08
        (22u32, 0x568u32, 0x408u32),     // MNPlayersSpotlight MObjSub_0x0408
        (69u32, 0x6950u32, 0x6140u32),   // MVOpeningStandoff LightningMObjSub_MObjSub
        (75u32, 0x35F8u32, 0x2AA8u32),   // MVOpeningRunCrash MObjSub_0x2AA8_MObjSub
        (83u32, 0x7750u32, 0x73E0u32),   // EFCommonEffects1 DamageSlash_MObjSub
        (167u32, 0x28DA8u32, 0x287D8u32), // MNTitle SlashMObjSub_MObjSub
    ]
    .into_iter()
    // The opening's one-`DObj` lists with a `gcAddMObjAll` table (RE-467).
    .chain(
        ssb_rom::opening::DL_GRAPHS
            .iter()
            .filter(|&&(_, _, mobjsub)| mobjsub != 0)
            .map(|&(file, dl, mobjsub)| (file, dl, mobjsub)),
    ) {
        let nodes = graphs
            .get(&file)
            .and_then(|gs| gs.iter().find(|g| g.offset == graph))
            .map_or(0, |g| g.nodes.len());
        let parses = files[file as usize]
            .as_ref()
            .is_some_and(|f| mobj::read_table(f, table, nodes).is_some());
        if parses {
            tables.insert(file, graph, table);
        }
    }

    // RE-125: re-ran `--search` over every one of the 57 still-unpaired
    // graphs (not just ones that already narrowed to exactly one
    // candidate) and cross-referenced every candidate within 8 bytes of a
    // fully-typed `MObjSub **name[N]` decomp symbol
    // (`tools/mobjtable-ground-truth.py`'s own answer key). 23 candidates
    // landed close to a real, typed symbol; the *typed symbol's own
    // address* was independently checked with `read_table` and found to
    // NOT satisfy each graph's own demand vector at all, while the search's
    // reported (nearby, not identical) candidate did -- ruling out the
    // labeled address as an equally-plausible alternate reading, the same
    // check that would have caught a coincidence rather than a real match.
    //
    // 3 of the 23 were rejected on that same evidence standard: file 86's
    // is the identical 27-way-ambiguous NBumper graph RE-061 already
    // measured and declined -- a match against 1 of 27 candidates is
    // exactly the near-chance fingerprint this project already rejected
    // for this specific graph, not new evidence. Files 108's and one of
    // 152's two candidates land inside a texture's own trailing pixel
    // bytes with no gap of any kind in the decomp (the preceding texture's
    // declared size ends exactly where the table begins) -- any
    // non-pointer byte range trivially reads as a zero-length chain, so a
    // demand-vector match there is not evidence, unlike the other 20.
    //
    // The other 20, across 8 files, each have a *decomp-documented* reason
    // for the small gap between the search's own candidate and the typed
    // symbol's label: an explicit `PAD(4)` immediately preceding a 1-entry
    // table (105, 111, 112, 157 -- the same shape RE-078 already confirmed
    // once for file 84's `PAD(8)`), explicit leading/trailing `NULL`
    // entries the decomp source itself declares inside a larger typed
    // array (104, 152's other candidate, 342), or an explicitly
    // documented "combined chain" comment naming exactly this sub-range
    // (328's `JointVerts_Vtx`, RE-077's own file, a second real table in
    // it distinct from the one RE-077 already fixed).
    for &(file, graph, table) in &[
        (104u32, 0x33B8u32, 0x1F54u32), // StagePupupuFile2 (Dream Land) -- gap_0x1D00_sub_0x250[4], entries [0][1] decomp-documented NULL
        (105u32, 0xCEE8u32, 0xCC90u32), // StageZebesFile2 -- data_0xCC94[1], PAD(4) immediately before it
        (105u32, 0xDE28u32, 0xCC90u32), // StageZebesFile2, same table, second graph
        (111u32, 0x77E0u32, 0x7610u32), // StageYosterFile2 -- data_0x7614[1], PAD(4) immediately before it
        (111u32, 0x8528u32, 0x7610u32), // StageYosterFile2, same table
        (111u32, 0x8778u32, 0x7610u32), // StageYosterFile2, same table
        (111u32, 0x96E8u32, 0x7610u32), // StageYosterFile2, same table
        (111u32, 0xA5A8u32, 0x7610u32), // StageYosterFile2, same table
        (111u32, 0xB458u32, 0x7610u32), // StageYosterFile2, same table
        (112u32, 0x9958u32, 0x9790u32), // StageYamabukiFile2 -- data_0x9794[1], PAD(4) immediately before it
        (112u32, 0xC890u32, 0x9790u32), // StageYamabukiFile2, same table
        (112u32, 0xE400u32, 0x9790u32), // StageYamabukiFile2, same table
        (112u32, 0xFE58u32, 0x9790u32), // StageYamabukiFile2, same table
        (152u32, 0x1770u32, 0x13B0u32), // StagePupupuFile3 -- mobjlink_0x13AC[7], entries [0..3]/[5][6] decomp-documented NULL
        (157u32, 0xB08u32, 0x8C0u32), // StageZebesFile3 -- mobjlink_0x08C4[1], PAD(4) immediately before it
        (328u32, 0x4230u32, 0x18u32), // KirbyModel -- JointVerts_Vtx[8] slots 6-7, decomp's own "combined chain" comment
        (328u32, 0x49D8u32, 0x18u32), // KirbyModel, same slots
        (328u32, 0x16AB0u32, 0x18u32), // KirbyModel, same slots
        (328u32, 0x176D8u32, 0x18u32), // KirbyModel, same slots
        // RE-379 corrects RE-125's 0x101C here: `dEFManagerPikachuThunderJoltEffectDesc`
        // names `llPikachuSpecial3ThunderJoltMObjSub` (0x20A0, two slots)
        // for this graph. The 0x1018 table belongs to the ground Thunder
        // Jolt tree below, whose `WPAttributes.p_mobjsubs` names it.
        (342u32, 0x2258u32, 0x20A0u32), // PikachuSpecial3 ThunderJoltMObjSub
        (342u32, 0x1888u32, 0x1018u32), // PikachuSpecial3 ground Thunder Jolt, file 244 + 0x34
        // RE-381: Ness's weapons, each named by its `WPAttributes.p_mobjsubs`.
        (336u32, 0x168u32, 0x0u32), // NessSpecial3 PK Fire spark, file 240 + 0x00
        (335u32, 0x7C98u32, 0x7AF8u32), // NessModel PK Thunder head, file 239 + 0x0C
        (335u32, 0x8B40u32, 0x89C8u32), // NessModel PK Thunder trail, file 239 + 0x40
        // RE-383: Link's Spin Attack weapon graphic, named by
        // `llLinkMainSpinAttackWeaponAttributes.p_mobjsubs` (file 225 +
        // 0x0C). The decomp declares the table as a NULL placeholder; the ROM
        // holds [NULL, 0x114E8].
        (324u32, 0x11908u32, 0x110A8u32), // LinkModel Spin Attack weapon
    ] {
        let nodes = graphs
            .get(&file)
            .and_then(|gs| gs.iter().find(|g| g.offset == graph))
            .map_or(0, |g| g.nodes.len());
        let parses = files[file as usize]
            .as_ref()
            .is_some_and(|f| mobj::read_table(f, table, nodes).is_some());
        if parses {
            tables.insert(file, graph, table);
        }
    }

    // RE-153: character-select / results emblems use the same fourth
    // call-sequence mechanism as RE-060, but are all driven through parallel
    // arrays rather than one call site per graph.  `mnCharactersMakeEmblem`
    // (`mn/mndata/mncharacters.c`) and `mnVSResultsMakeEmblem`
    // (`mn/mnvsmode/mnvsresults.c`) give `gcSetupCommonDObjs` one member of
    // `dobjdescs[]`, then give `gcAddMObjAll` the same-index member of
    // `mobjsubs[]`.  Each linker symbol is backed by the explicitly typed
    // leading `MObjSub **..._pre` table in
    // `relocData/35_FTEmblemModels.c`; the first slot is the intentional
    // root-node NULL and the second leads to the rendered emblem's MObj
    // chain.  The mapping below records those original call-pairings, rather
    // than selecting one of `search_tables`' several demand-compatible
    // candidates.
    for &(file, graph, table) in &[
        (35u32, 0x990u32, 0x0u32),     // Mario_MObjSub_pre
        (35u32, 0x1348u32, 0xB00u32),  // Donkey_MObjSub_pre
        (35u32, 0x1860u32, 0x1470u32), // Metroid_MObjSub_pre
        (35u32, 0x21D0u32, 0x1940u32), // Fox_MObjSub_pre
        (35u32, 0x2520u32, 0x22B0u32), // Zelda_MObjSub_pre
        (35u32, 0x2F10u32, 0x2690u32), // Yoshi_MObjSub_pre
        (35u32, 0x3828u32, 0x2FF0u32), // FZero_MObjSub_pre
        (35u32, 0x3E68u32, 0x3900u32), // Kirby_MObjSub_pre
        (35u32, 0x4710u32, 0x3F40u32), // PMonsters_MObjSub_pre
        (35u32, 0x5A00u32, 0x4840u32), // Mother_MObjSub_pre
    ] {
        let nodes = graphs
            .get(&file)
            .and_then(|gs| gs.iter().find(|g| g.offset == graph))
            .map_or(0, |g| g.nodes.len());
        let parses = files[file as usize]
            .as_ref()
            .is_some_and(|f| mobj::read_table(f, table, nodes).is_some());
        if parses {
            tables.insert(file, graph, table);
        }
    }

    // RE-154/RE-172: file-84's effect `EFDesc` records name each graph and
    // MObj table together.  As with the Link entry effects above, these
    // descriptors live in the static executable, so neither archive-local
    // pointer scanning nor demand matching is the authority for their
    // pairing.  The current decomp's corrected relocData layout fixes a
    // historical one-entry shift in the hand-entered graph offsets: Catch
    // Swirl is 0x2760, Reflect Break is 0x3398, Dead Explode is 0x53E8, and
    // Ness PK Flash is 0x6D00.  The latter's formerly-mistyped 0x6B40 block
    // is its MObj wrapper, not its DObjDesc.  Keep the already-entered Catch
    // mapping above and enter the remaining exact source pairs here.
    for &(file, graph, table) in EF_COMMON_EFFECTS2_MOBJ_PAIRS {
        let nodes = graphs
            .get(&file)
            .and_then(|gs| gs.iter().find(|g| g.offset == graph))
            .map_or(0, |g| g.nodes.len());
        let parses = files[file as usize]
            .as_ref()
            .is_some_and(|f| mobj::read_table(f, table, nodes).is_some());
        if parses {
            tables.insert(file, graph, table);
        }
    }

    // RE-155: `EFDesc` also names the two remaining file-85 effect graphs
    // directly: `dEFManagerMBallRaysEffectDesc` and
    // `dEFManagerItemGetSwirlEffectDesc` in `ef/efmanager.c`.  The linker
    // targets are wrapper starts, not the visible `MObjSub **head` arrays:
    // each leading PAD word corresponds to a DObj with no material.  The
    // same source-confirmed pairing shape occurs in the one-player bonus
    // stage: `dSC1PBonusStagePlatformDescs` in `sc1pbonusstage.c` has
    // parallel DObj/MObj columns for the small, medium, and large platforms.
    // The three `Bonus2Common` MObj tables happen to have identical demand
    // fingerprints, so `--search` reports all three for every graph; the
    // source's same-row relationship is the authority selecting them.
    for &(file, graph, table) in &[
        (85u32, 0x628u32, 0x108u32),    // MBallRaysEffectDesc
        (85u32, 0x3170u32, 0x2CA8u32),  // ItemGetSwirlEffectDesc
        (136u32, 0x3DA8u32, 0x3720u32), // PlatformSmall
        (136u32, 0x45D8u32, 0x3F70u32), // PlatformMedium
        (136u32, 0x4E08u32, 0x47A0u32), // PlatformLarge
    ] {
        let nodes = graphs
            .get(&file)
            .and_then(|gs| gs.iter().find(|g| g.offset == graph))
            .map_or(0, |g| g.nodes.len());
        let parses = files[file as usize]
            .as_ref()
            .is_some_and(|f| mobj::read_table(f, table, nodes).is_some());
        if parses {
            tables.insert(file, graph, table);
        }
    }

    // RE-158: static effect descriptors also name nine more graph/table
    // relationships that archive-local scans cannot see. `EFGroundDesc`
    // records in `ef/efground.c` bind Kongo Jungle's bird, while `EFDesc`
    // records in `ef/efmanager.c` bind Pikachu's two Special2 effects and
    // Ness's PK Thunder Wave and Link's Spin Attack, alongside the four
    // special-move effects whose tables were already typed in relocData. The
    // matching relocData declarations identify the linker-target offsets.
    // These are source relationships, not choices made from the
    // demand-compatible candidates reported by `--search`.
    for &(file, graph, table) in &[
        (108u32, 0xF400u32, 0xF230u32), // GRJungleMapBird
        (347u32, 0x800u32, 0x640u32),   // PikachuSpecial2 Unk
        (347u32, 0x1640u32, 0x13A0u32), // PikachuSpecial2 ThunderShock
        (335u32, 0x9A10u32, 0x9870u32), // NessModel PKThunderWave
        (353u32, 0x11C0u32, 0x1038u32), // LinkSpecial2 SpinAttack
        (349u32, 0x380u32, 0x210u32),   // SamusSpecial2 GrappleBeam
        (350u32, 0xB08u32, 0x960u32),   // CaptainSpecial2 FalconKick
        (351u32, 0x2130u32, 0x1C20u32), // PurinSpecial2 Sing
        (352u32, 0x9A8u32, 0x810u32),   // NessSpecial2 PsychicMagnet
    ] {
        let nodes = graphs
            .get(&file)
            .and_then(|gs| gs.iter().find(|g| g.offset == graph))
            .map_or(0, |g| g.nodes.len());
        let parses = files[file as usize]
            .as_ref()
            .is_some_and(|f| mobj::read_table(f, table, nodes).is_some());
        if parses {
            tables.insert(file, graph, table);
        }
    }

    // RE-159: the explanation screen constructs its control-stick interface
    // with `gcSetupCustomDObjs` followed by `gcAddMObjAll` in the same source
    // call sequence. The static code therefore names file 198's otherwise
    // ambiguous graph/table pair directly; its five demand matches are not
    // evidence enough to select the table by themselves.
    {
        let (file, graph, table) = (198u32, 0x5300u32, 0x5028u32);
        let nodes = graphs
            .get(&file)
            .and_then(|gs| gs.iter().find(|g| g.offset == graph))
            .map_or(0, |g| g.nodes.len());
        let parses = files[file as usize]
            .as_ref()
            .is_some_and(|f| mobj::read_table(f, table, nodes).is_some());
        if parses {
            tables.insert(file, graph, table);
        }
    }

    // RE-160: six remaining graphs are named by source-level scene/effect
    // descriptors, not by an archive-local pointer relation.  `efground.c`
    // puts Sector Z's Ship DObj/MObj offsets in the same `EFGroundDesc`;
    // `sc1pgameboss.c` does the same for Final Destination's four wallpaper
    // effects in `SC1PGameBossEffect`; and `grpupupu.c` passes Dream Land's
    // Whispy-eyes pair together to `grPupupuMakeMapGObj`.  The linker-offset
    // labels in `tools/relocFileDescriptions.us.txt` independently identify
    // the targets.  These are direct original-source pairings, not choices
    // among the ambiguous candidates reported by `mobj --search`.
    for &(file, graph, table) in &[
        (109u32, 0xB6F8u32, 0xB3C0u32),   // GRSectorMapShip
        (114u32, 0x8960u32, 0x86D8u32),   // GRLastMapEffects0
        (114u32, 0xA188u32, 0x97B0u32),   // GRLastMapEffects1
        (114u32, 0xDD90u32, 0xD470u32),   // GRLastMapEffects2_0
        (114u32, 0x11268u32, 0x10788u32), // GRLastMapEffects2_1 / Effects3_0
        (152u32, 0x10F0u32, 0xF00u32),    // GRPupupuMapWhispyEyesTransformKinds
    ] {
        let nodes = graphs
            .get(&file)
            .and_then(|gs| gs.iter().find(|g| g.offset == graph))
            .map_or(0, |g| g.nodes.len());
        let parses = files[file as usize]
            .as_ref()
            .is_some_and(|f| mobj::read_table(f, table, nodes).is_some());
        if parses {
            tables.insert(file, graph, table);
        }
    }

    // The other ground effects: every `EFGroundDesc` names its DObj array
    // and its `MObjSub` table together (`o_dobjsetup`, `o_mobjsub`), which
    // `efGroundMakeEffect` hands to `lbCommonAddMObjForTreeDObjs`. Kongo
    // Jungle's bird and Sector Z's ship above are two of them.
    for desc in 0..ssb_game::ground_effect::DESC_COUNT {
        let table = ssb_game::ground_effect::O_MOBJSUB[desc];
        let Some(file) = ssb_game::ground_effect::desc_file(desc) else {
            continue;
        };
        if table == 0 {
            continue;
        }
        let graph = ssb_game::ground_effect::O_DOBJSETUP[desc];
        let nodes = graphs
            .get(&file)
            .and_then(|gs| gs.iter().find(|g| g.offset == graph))
            .map_or(0, |g| g.nodes.len());
        let parses = files[file as usize]
            .as_ref()
            .is_some_and(|f| mobj::read_table(f, table, nodes).is_some());
        if parses {
            tables.insert(file, graph, table);
        }
    }

    // RE-426: each fighter's high- and low-detail graphs (all but Master
    // Hand: the twelve, Metal Mario, the Polygons and Giant Donkey Kong). A
    // low-detail joint whose descriptor has no list draws the high-detail
    // one with its `MObj`s, so that list goes on the low graph's node.
    // Giant Donkey Kong wears Donkey Kong's model and Polygon Luigi Polygon
    // Mario's; the first fighter to name a graph owns it.
    let mut fighter_graphs = BTreeMap::new();
    for entry in ssb_rom::fighter::FIGHTER_FILES
        .iter()
        .filter(|e| e.kind != 12)
    {
        let Some(main) = files.get(entry.file as usize).and_then(Option::as_ref) else {
            continue;
        };
        let [Some(hi), lo] = ssb_rom::fighter::common_parts(main, *entry) else {
            continue;
        };
        if fighter_graphs.contains_key(&(hi.model_file, hi.graph)) {
            continue;
        }
        fighter_graphs.insert(
            (hi.model_file, hi.graph),
            FighterGraph {
                entry: *entry,
                detail: 0,
                high: hi.graph,
                fallback: Vec::new(),
            },
        );
        let Some(lo) = lo.filter(|lo| lo.model_file == hi.model_file) else {
            continue;
        };
        let Some(hi_dls) = graphs.get(&hi.model_file).and_then(|gs| {
            gs.iter()
                .find(|g| g.offset == hi.graph)
                .map(|g| g.nodes.iter().map(|n| n.desc.dl).collect::<Vec<_>>())
        }) else {
            continue;
        };
        let Some(low) = graphs
            .get_mut(&lo.model_file)
            .and_then(|gs| gs.iter_mut().find(|g| g.offset == lo.graph))
        else {
            continue;
        };
        let mut fallback = Vec::new();
        for (n, (node, hi_dl)) in low.nodes.iter_mut().zip(&hi_dls).enumerate() {
            if node.desc.dl.is_none() && hi_dl.is_some() {
                node.desc.dl = *hi_dl;
                fallback.push(n);
            }
        }
        fighter_graphs.insert(
            (lo.model_file, lo.graph),
            FighterGraph {
                entry: *entry,
                detail: 1,
                high: hi.graph,
                fallback,
            },
        );
    }

    Loaded {
        files,
        graphs,
        tables,
        stages,
        fighter_graphs,
    }
}

impl Loaded {
    /// The materials for each node of a graph at the default costume, or
    /// empty vectors when no record names its table.
    pub fn materials(
        &self,
        file: &ssb_rom::archive::File,
        graph: &ssb_rom::scene::SceneGraph,
    ) -> Vec<ssb_rom::mobj::NodeMaterials> {
        self.materials_at(file, graph, DEFAULT_COSTUME)
    }

    /// The materials for each node of a graph at a given costume (RE-098),
    /// or empty vectors when no record names its table. Costume 0 is
    /// [`DEFAULT_COSTUME`], the one every non-fighter graph and every
    /// existing call site before RE-098 implicitly used.
    pub fn materials_at(
        &self,
        file: &ssb_rom::archive::File,
        graph: &ssb_rom::scene::SceneGraph,
        costume: f32,
    ) -> Vec<ssb_rom::mobj::NodeMaterials> {
        let mut nodes = self
            .tables
            .table_for(file.id, graph.offset)
            .and_then(|at| ssb_rom::mobj::read_table(file, at, graph.nodes.len()))
            .map(|t| t.nodes)
            .unwrap_or_else(|| vec![Vec::new(); graph.nodes.len()]);

        // The colours baked into `MObjSub` are the last costume's. The record
        // names a per-costume list alongside, and the game overwrites them
        // from it at setup; costume 0 is the default one (RE-040).
        if let Some(at) = self.tables.costumes_for(file.id, graph.offset) {
            let colors = ssb_rom::matanim::costume_colors(
                file,
                at,
                graph.nodes.len(),
                |n| nodes.get(n).map_or(0, Vec::len),
                costume,
            );
            for (chain, per_node) in nodes.iter_mut().zip(colors) {
                apply_costume_colors(file, chain, &per_node);
            }
        }
        // RE-426: a low-detail joint drawing the high-detail list wears its
        // `MObj`s too.
        if let Some(fg) = self.fighter_graphs.get(&(file.id, graph.offset)) {
            if !fg.fallback.is_empty() {
                if let Some(high) = self
                    .graphs
                    .get(&file.id)
                    .and_then(|gs| gs.iter().find(|g| g.offset == fg.high))
                {
                    let hi = self.materials_at(file, high, costume);
                    for &n in &fg.fallback {
                        if let (Some(slot), Some(m)) = (nodes.get_mut(n), hi.get(n)) {
                            *slot = m.clone();
                        }
                    }
                }
            }
            for (n, chain) in nodes.iter_mut().enumerate() {
                self.tag_texture_parts(file.id, graph.offset, n, chain);
            }
        }
        nodes
    }

    /// Marks node `node`'s texture-part `MObj`s (RE-426): each
    /// `FTTexturePart` on this joint names its `MObj`'s place in the chain
    /// at this graph's detail, and the ids the fighter's scripts set and
    /// its costume script makes the `MObj` with (RE-569).
    pub fn tag_texture_parts(
        &self,
        file: u32,
        graph_offset: u32,
        node: usize,
        chain: &mut ssb_rom::mobj::NodeMaterials,
    ) {
        let Some(fg) = self.fighter_graphs.get(&(file, graph_offset)) else {
            return;
        };
        let Some(main) = self
            .files
            .get(fg.entry.file as usize)
            .and_then(Option::as_ref)
        else {
            return;
        };
        let Some(parts) = ssb_rom::fighter::texture_parts(main, fg.entry) else {
            return;
        };
        let Some(kind) = ssb_game::fighter::FighterKind::from_ordinal(fg.entry.kind) else {
            return;
        };
        let events = ssb_game::motion::texture_part_events(kind);
        let made = ssb_game::modelpart::texture_part_made(kind);
        for (p, tp) in parts.iter().enumerate() {
            if u32::from(tp.joint) != node as u32 + ssb_rom::fighter::JOINT_COMMON_START {
                continue;
            }
            let ids = events
                .iter()
                .filter(|&&(part, _)| part == p as i32)
                .map(|&(_, id)| id)
                .chain(made[p].0.iter().map(|&id| i32::from(id)))
                .filter(|&id| id > 0 && id < 16)
                .fold(0u16, |m, id| m | 1 << id);
            if ids == 0 {
                continue;
            }
            if let Some(m) = chain.get_mut(usize::from(tp.detail[fg.detail as usize])) {
                m.texture_part = Some(ssb_rom::mobj::TexturePartTag {
                    part: p as u8,
                    ids,
                    at: m.at,
                });
            }
        }
    }
}

/// Overwrites one `MObj` chain's colours (and palette) with a costume's
/// (RE-040, RE-096): what `lbCommonAddMObjForFighterPartsDObj` plays from
/// the costume's material scripts.
pub fn apply_costume_colors(
    file: &ssb_rom::archive::File,
    chain: &mut [ssb_rom::mobj::MObjMaterial],
    colors: &[Option<ssb_rom::matanim::Colors>],
) {
    for (m, c) in chain.iter_mut().zip(colors) {
        let Some(c) = c else { continue };
        if c.prim.is_some() {
            m.prim_color = c.prim;
        }
        if c.env.is_some() {
            m.env_color = c.env;
        }
        if c.blend.is_some() {
            m.blend_color = c.blend;
        }
        if c.light1.is_some() {
            m.light1_color = c.light1;
        }
        if c.light2.is_some() {
            m.light2_color = c.light2;
        }
        // `PaletteID` (RE-096: 45% of real fighter costume scripts carry
        // one) selects which of `MObjSub.palettes[]` this costume actually
        // wears -- `m.palette` otherwise only ever holds `palettes[0]`
        // (`read_material`'s own scope), silently costume 0's choice for
        // every other costume. `objdisplay.c` reads it back as
        // `palettes[(s32)mobj->palette_id]`, the same cast `colors_at`
        // already applied.
        if let Some(id) = c.palette_id.filter(|&id| id >= 0) {
            if let Some(ptrs) = ssb_rom::mobj::read_palettes(file, m.at, id as usize + 1) {
                if let Some(&p) = ptrs.get(id as usize) {
                    m.palette = Some(p);
                }
            }
        }
    }
}

/// Which costume the pack is built for.
///
/// A fighter's colours are per-costume and the game picks at match start; the
/// converter has to pick one. Zero is the default the character select opens
/// on — Mario in red, Luigi in green.
pub const DEFAULT_COSTUME: f32 = 0.0;

/// Real per-fighter costume counts (RE-098): `dFTParamCostumeIDs[fkind]
/// .develop + 1`, `refs/ssb-decomp-re/src/ft/ftparam.c:56`. `develop` is the
/// highest costume index `ftParamGetCostumeDebug` (the same table's own
/// accessor) ever returns for that fighter, and every `royal`/`team` colour
/// choice the game exposes is within `0..=develop` — so `develop + 1` is the
/// exact count of costumes that can ever be selected, not a guess. This
/// table lives in the game's executable, not any archive file, so it cannot
/// be read from the ROM the way `PartTables` reads structural pairings; it is
/// hand-transcribed and cited, the same established pattern as `EFDesc`
/// (RE-058/059) and `MPGroundData.light_angle` (RE-065). Keyed by archive
/// file id (`FTCommonPart`'s own model file), matching RE-077's own per-
/// fighter file-id census.
pub const FIGHTER_COSTUME_COUNTS: &[(u32, u32)] = &[
    (296, 5), // Mario
    (313, 4), // Fox
    (317, 5), // Donkey Kong
    (320, 5), // Samus
    (323, 4), // Luigi
    (324, 4), // Link
    (328, 5), // Kirby
    (330, 4), // Jigglypuff
    (332, 6), // Captain Falcon
    (335, 4), // Ness
    (338, 6), // Yoshi
    (341, 4), // Pikachu
];

pub fn fighter_costume_count(file: u32) -> u32 {
    FIGHTER_COSTUME_COUNTS
        .iter()
        .find(|&&(f, _)| f == file)
        .map_or(1, |&(_, n)| n)
}

/// Every mesh a file yields, converted the way [`pack`] converts it.
///
/// Graph-driven conversion first, then blind discovery for whatever the graphs
/// did not claim — the same two-tier arrangement and, importantly, the same
/// material state, so a diagnostic built on this sees what ships.
pub fn file_meshes(loaded: &Loaded, file: &ssb_rom::archive::File) -> Vec<ssb_rom::mesh::Mesh> {
    use ssb_rom::mesh;

    let resolver = ssb_rom::scene::DlResolver::new(file);
    let graphs: &[ssb_rom::scene::SceneGraph] =
        loaded.graphs.get(&file.id).map_or(&[], Vec::as_slice);
    let skeleton_graphs = fighter_skeleton_graphs(loaded);
    let ground_graphs = ground_layer1_graphs(loaded);
    let transition_graphs = lb_transition_graphs();
    let mut out = Vec::new();
    let mut claimed = BTreeSet::new();

    for graph in graphs {
        let plan = plan_draw_order(file, graph, &resolver);
        let decoded: Vec<Vec<ssb_rom::dl::Cmd>> = plan
            .iter()
            .map(|p| {
                claimed.insert(p.dl);
                file.data
                    .get(p.dl as usize..)
                    .and_then(|d| ssb_rom::dl::decode_list_at(d, p.dl).ok())
                    .unwrap_or_default()
            })
            .collect();
        let materials = loaded.materials(file, graph);
        let initial = initial_material_for(
            &skeleton_graphs,
            &ground_graphs,
            &transition_graphs,
            file.id,
            graph.offset,
        );
        let items: Vec<mesh::SequenceItem> = plan
            .iter()
            .zip(&decoded)
            .map(|(p, cmds)| mesh::SequenceItem {
                cmds,
                world: p.world,
                mobjs: &materials[p.node],
                mat_anims: &[],
                depth_seed: ground_layer1_list1_depth_seed(initial, p.list_id),
                stream: u8::from(p.list_id == Some(1)),
            })
            .collect();
        out.extend(
            mesh::convert_sequence(&items, mesh::Source::of(file), initial)
                .into_iter()
                .flatten(),
        );
    }

    // Exactly the packer's discovery rule, and it has to stay exactly it: this
    // function's whole claim is that its counts describe the pack. `convert`
    // inlines callees, so a list another list calls is not a root -- converting
    // it standalone runs it without the state its caller sets up, which is how
    // the report came to claim 75 unresolved textures the pack does not have.
    let all = ssb_rom::scan::root_display_lists(file);
    let called: BTreeSet<u32> = all.iter().flat_map(|d| d.referenced_lists()).collect();
    for dl in all
        .iter()
        .filter(|d| !called.contains(&d.offset) && !claimed.contains(&d.offset))
    {
        if unconvertible_without_materials(&dl.commands, file) {
            continue;
        }
        out.extend(convert_discovered(&dl.commands, file));
    }
    out
}

/// A list no scene graph names, converted under the scene's state
/// ([`ssb_rom::mesh::InitialMaterial::SCENE`], RE-424).
pub fn convert_discovered(
    cmds: &[ssb_rom::dl::Cmd],
    file: &ssb_rom::archive::File,
) -> Result<ssb_rom::mesh::Mesh, ssb_rom::mesh::MeshError> {
    let item = ssb_rom::mesh::SequenceItem {
        cmds,
        world: ssb_rom::scene::Mat4::IDENTITY,
        mobjs: &[],
        mat_anims: &[],
        depth_seed: None,
        stream: 0,
    };
    ssb_rom::mesh::convert_sequence(
        &[item],
        ssb_rom::mesh::Source::of(file),
        ssb_rom::mesh::InitialMaterial::SCENE,
    )
    .pop()
    .expect("one item")
}

/// Whether a discovered list needs material state this converter cannot supply.
///
/// A list that calls segment `0x0E` gets its texture, palette or colour from
/// the node's `MObj` chain. Discovery finds lists by scanning bytes, so it has
/// no node and no chain, and the converter correctly drops the binding rather
/// than guessing — leaving geometry whose palette never loads. Packing that
/// means shipping a surface we know draws wrong. Lists a scene graph names are
/// unaffected: they come with their chain (RE-047).
pub fn unconvertible_without_materials(
    cmds: &[ssb_rom::dl::Cmd],
    file: &ssb_rom::archive::File,
) -> bool {
    ssb_rom::mobj::demand(cmds, &file.data) > 0
}

/// Reads a figatree's joint pointer table.
///
/// The table's length is not stored: the first non-null pointer is the offset
/// of the first script, which is exactly where the table ends.
/// The eight sector tables of a fighter's shield pose, each entry paired
/// with the node it drives. `XRotN` and `YRotN` drive no packed node.
pub fn shield_pose_joints(
    loaded: &Loaded,
    refs: ssb_rom::fighter::ShieldPoseRefs,
    nodes: &[u32],
) -> Result<Vec<Vec<(Option<u32>, Option<u32>)>>, String> {
    let file = loaded
        .files
        .get(refs.file as usize)
        .and_then(Option::as_ref)
        .ok_or_else(|| format!("file {} missing", refs.file))?;
    let data = &file.data;
    let word = |at: usize| -> Option<u32> {
        Some(u32::from_be_bytes(data.get(at..at + 4)?.try_into().ok()?))
    };
    let entries = nodes.len() + 2;
    let end = refs.lookup as usize + entries * ssb_rom::fighter::DOBJDESC_SIZE as usize;
    if word(end) != Some(ssb_rom::fighter::DOBJ_ARRAY_MAX) {
        return Err(format!(
            "dobj_lookup does not end after {entries} entries at 0x{end:X}"
        ));
    }
    refs.tables
        .iter()
        .map(|&table| {
            (0..entries)
                .map(|j| {
                    let at = word(table as usize + j * 4)
                        .ok_or_else(|| format!("table 0x{table:X} truncated"))?;
                    if at != 0 && at as usize >= data.len() {
                        return Err(format!("table 0x{table:X} entry {j} points outside"));
                    }
                    let node = j.checked_sub(1).and_then(|k| nodes.get(k).copied());
                    Ok(((at != 0).then_some(at), node))
                })
                .collect()
        })
        .collect()
}

pub fn joint_table(data: &[u8]) -> Option<Vec<u32>> {
    let word = |at: usize| -> Option<u32> {
        Some(u32::from_be_bytes(data.get(at..at + 4)?.try_into().ok()?))
    };
    let len = ssb_rom::anim::joint_table_len(data)?;
    (0..len).map(|i| word(i * 4)).collect()
}
