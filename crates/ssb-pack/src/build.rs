//! The pack build: every converter in the order the shipped pack needs.

use crate::*;

/// What one build packed: the counts `romtool pack` prints.
#[derive(Debug, Default)]
pub struct PackStats {
    pub meshes: usize,
    pub triangles: usize,
    pub sprites: usize,
    pub objects: usize,
    pub placed_meshes: usize,
    pub node_dls: usize,
    pub extra_leaves: usize,
    pub costume_overrides_added: usize,
    pub with_collision: usize,
    pub resolved_layers: usize,
    pub stage_layers: usize,
    pub fighters: usize,
    pub fighters_failed: Vec<&'static str>,
    pub packed_anims: usize,
    pub anim_joints_packed: usize,
    pub hidden_joint_anims: usize,
    pub walked_anims: usize,
    pub anims_failed: Vec<String>,
    pub stage_anims: usize,
    pub stage_anim_joints: usize,
    pub transition_anim_joints: usize,
    pub effect_anims: usize,
    pub effect_anim_joints: usize,
    pub weapon_anims: usize,
    pub weapon_anim_rebased: usize,
    pub monster_anims: usize,
    pub ground_anims: usize,
    pub ground_anim_joints: usize,
    pub ground_mats: usize,
    pub ground_mat_joints: usize,
    pub shield_pose_anims: usize,
    pub particle_scripts: usize,
    pub particle_textures: usize,
    pub particle_frames: usize,
    /// Fighters with a packed shield pose.
    pub shield_fighters: usize,
    /// RE-318 census: source primitives lowered, triangles split,
    /// primitives emitted.
    pub wide_tiles: (usize, usize, usize),
    /// RE-321 census: every classified two-tile blend primitive.
    pub lod_blend_sites: Vec<LodBlendSite>,
    /// Primitives that named a sprite-table script before any primitive
    /// drawing its image did (RE-326).
    pub non_owner_first: usize,
}

/// Builds the asset pack from the ROM's bytes: one pack, the opening's
/// models and the menu scenes' content included; each scene loads its own
/// files from it (RE-475; the opening's models and the menus had packs of
/// their own before it, RE-467 and RE-461).
///
/// Installs the `TABLES` section it builds (`ssb_tables::install`) before
/// anything reads a table.
///
/// Writes the pack to `sink` in order (`PackWriter::finish_to`): the whole
/// pack is never in memory at once (RE-588).
pub fn build<'r>(
    rom: impl Into<RomRef<'r>>,
    opts: &PackOptions,
    log: &dyn PackLog,
    observer: &dyn PackObserver,
    sink: &mut dyn ssb_rom::pack::PackSink,
) -> PackResult<PackStats> {
    build_with(rom.into(), opts, log, observer, sink, None)
}

/// [`build`], with the blob's compressed pages spilled to `spill` when
/// given (the on-device build: RE-588, the PSP-1000's memory). `rom` may
/// be a paged reader (`ssb_rom::rom_read::PagedRom`) in any byte order.
/// The pack's bytes depend on neither.
pub fn build_with<'r>(
    rom: impl Into<RomRef<'r>>,
    opts: &PackOptions,
    log: &dyn PackLog,
    observer: &dyn PackObserver,
    sink: &mut dyn ssb_rom::pack::PackSink,
    spill: Option<Box<dyn ssb_rom::blob_store::BlobSpill>>,
) -> PackResult<PackStats> {
    use ssb_rom::{mesh, pack as fmt};

    let (only_file, swizzle) = (opts.only_file, opts.swizzle);
    let ctx = PackCtx::new(opts, log, observer);
    let ctx = &ctx;

    let rom: RomRef<'r> = rom.into();
    let info = ssb_rom::rom::identify_reader(rom)?;
    let archive = ssb_rom::archive::Archive::open(rom, info.region)?;

    // The TABLES section (D-048, `ssb-tables`), installed before anything
    // reads a table: the packer reads `ssb_engine::math`'s sines.
    // Read through the ROM reader, never copied (RE-588).
    let bridge = crate::rom_bridge::Bridge(rom);
    let source = ssb_tablegen::Source::over((&bridge).into());
    let tables = ssb_tablegen::tables_section(Some(&source)).map_err(|e| format!("tables: {e}"))?;
    drop(source);
    // Leaked once: the installed section is also the one the pack writes
    // (RE-588: one copy).
    let tables: &'static [u8] = Box::leak(tables.into_boxed_slice());
    // `unsafe` in a runtime's `eager` build (the PSP): no table is read yet,
    // on any thread; the block is unused in the host's checked build.
    #[allow(unused_unsafe)]
    // SAFETY: nothing has read a table before this point in the build.
    let installed = unsafe { ssb_tables::install(tables) };
    match installed {
        // SAFETY: just installed; no hot value has been read yet.
        Ok(_) => unsafe { ssb_game::tables::fill() },
        // A host process that read a table first (its `host-rom`
        // fallback, or an earlier build) holds this ROM's section already.
        Err(ssb_tables::InstallError::AlreadyInstalled)
            if ssb_tables::installed().is_some_and(|s| s.bytes() == tables) => {}
        Err(e) => return Err(format!("tables section: {e}").into()),
    }

    // The audio section: sequences, both banks, the FGM files and the
    // synthesizer's constant tables, loaded once at boot (D-049). Checked
    // and sized here; streamed from the ROM after the blob (RE-588: never
    // held whole).
    let audio_len =
        ssb_rom::audio::section_len(rom).map_err(|e| format!("audio section: {e:?}"))?;
    ctx.log.phase("tables");
    let mut writer = fmt::PackWriter::new();
    if let Some(spill) = spill {
        writer.set_blob_spill(spill);
    }
    // The same texture (image *and* palette, RE-098) is bound by many
    // primitives; upload each once.
    let mut tex_index: BTreeMap<TexKey, u32> = BTreeMap::new();
    // The same material animation script drives many primitives; upload
    // each once. Keyed the same way `resolve_layer_mat_anims` resolves a
    // script's identity: (source archive file, script offset) (RE-091).
    let mut mat_anim_index: BTreeMap<ssb_rom::mesh::MatAnimRef, u32> = BTreeMap::new();
    // Every real, script-verified palette animation found so far, keyed by
    // the same `(source_file, script)` identity `mesh::MatAnimRef` carries
    // -- populated per stage layer as its graph is reached, consumed by
    // `pack_mesh` the moment a primitive names one (RE-089/090/091).
    let mut mat_anim_data: BTreeMap<ssb_rom::mesh::MatAnimRef, MatAnimData> = BTreeMap::new();
    let mut meshes = 0usize;
    let mut triangles = 0usize;
    let mut objects = 0usize;
    let mut placed_meshes = 0usize;
    let mut node_dls = 0usize;
    let mut extra_leaves = 0usize;
    let mut costume_overrides_added = 0usize;
    let (mut skeleton_parts_added, mut skeleton_parts_dropped) = (0usize, 0usize);
    let mut skeleton_fighters = alloc::collections::BTreeSet::new();
    let mut model_parts_added = 0usize;
    let mut model_part_flags_added = 0usize;
    let mut model_parts_empty = 0usize;
    let mut accessories_added = 0usize;
    // A stage names its render layers by the `DObjDesc` address they start at,
    // and `add_object` is given that same address -- so the layer lookup is an
    // exact match, never a search.
    let mut object_index: BTreeMap<(u32, u32), u32> = BTreeMap::new();

    let loaded = load_all(&archive);
    ctx.log.phase("load_all");
    let skeleton_graphs = fighter_skeleton_graphs(&loaded);
    let ground_graphs = ground_layer1_graphs(&loaded);
    let transition_graphs = lb_transition_graphs();

    // The installer's progress line: decompressed bytes, which follow the
    // phase's time more closely than file ids (most of the time goes to
    // the first fifth of the ids).
    let words = |id: u32| {
        archive
            .entry(id)
            .map_or(0, |e| usize::from(e.decompressed_words))
    };
    let total_words: usize = (0..archive.len() as u32).map(words).sum();
    let mut done_words = 0;
    for id in 0..archive.len() as u32 {
        ctx.log.progress(done_words, total_words);
        done_words += words(id);
        if only_file.is_some_and(|f| f != id) {
            continue;
        }
        let Some(file) = loaded.files.get(id as usize) else {
            continue;
        };
        let file = &*file;

        let all = ssb_rom::scan::root_display_lists(file);
        // convert() inlines G_DL callees, so packing a list that another list
        // calls would duplicate its geometry.
        let called: alloc::collections::BTreeSet<u32> =
            all.iter().flat_map(|d| d.referenced_lists()).collect();

        // A scene-graph node's `dl` is an *authoritative* list start: the game
        // itself passes that pointer to `gcAddDObjForGObj`. Blind discovery is
        // a heuristic by comparison, and its outermost-list reduction actively
        // discards these -- a hierarchy's per-joint lists sit inside the span
        // of a larger list the scan preferred. So convert the authoritative
        // offsets first and let discovery fill in only what they miss.
        let graphs: &[ssb_rom::scene::SceneGraph] =
            loaded.graphs.get(&id).map_or(&[], Vec::as_slice);
        // A node's `dl` may be a `Gfx*`, a `DObjDLLink[]` or a pre/post pair;
        // the resolver sorts that out.
        let resolver = ssb_rom::scene::DlResolver::new(file);

        // Every list a graph draws, in the order the game draws it, so the
        // vertex cache can be threaded through them; see convert_sequence.
        let plans: Vec<Vec<PlannedList>> = graphs
            .iter()
            .map(|g| plan_draw_order(file, g, &resolver))
            .collect();
        let authoritative: alloc::collections::BTreeSet<u32> = plans
            .iter()
            .flatten()
            .map(|p| p.dl)
            .filter(|&d| d != NO_LIST)
            .collect();

        let mut node_mesh: Vec<Vec<Option<u32>>> =
            graphs.iter().map(|g| vec![None; g.nodes.len()]).collect();
        // Geometry a node cannot hold itself: extra link entries, and the
        // pre-matrix half of a pair, which draws in the parent's space.
        let mut node_extra: Vec<Vec<(Option<usize>, u32)>> =
            graphs.iter().map(|_| Vec::new()).collect();

        for (gi, plan) in plans.iter().enumerate() {
            // Decode authoritative offsets *directly*, not through
            // `find_display_lists_at`: that re-applies the discovery
            // heuristics, and a heuristic can only lose information once the
            // game itself has told us where the list starts. Routing them
            // through it placed 742 of 1661 node lists; decoding them straight
            // places 1417.
            let mut decoded: Vec<Vec<ssb_rom::dl::Cmd>> = plan
                .iter()
                .map(|p| {
                    file.data
                        .get(p.dl as usize..)
                        .and_then(|d| ssb_rom::dl::decode_list_at(d, p.dl).ok())
                        .unwrap_or_default()
                })
                .collect();
            seed_player_interface_commands((id, graphs[gi].offset), &mut decoded);
            // A node's palette lives in its `MObj` chain, not its display
            // list; see `ssb_rom::mobj`.
            let materials = loaded.materials(file, &graphs[gi]);
            // Empty for every graph except a stage layer with a resolvable
            // (same-file) `p_matanim_joints` -- see the function's own doc
            // comment for why this has to be resolved per graph, not once
            // archive-wide, and RE-088/089/090's own reasoning for why the
            // bound has to come from the script (`resolve_layer_mat_anims`
            // also fills `mat_anim_data`, consumed later by `pack_mesh`).
            let mat_anims = resolve_layer_mat_anims(
                ctx,
                &loaded,
                file,
                graphs[gi].offset,
                &materials,
                &mut mat_anim_data,
            );
            let initial = initial_material_for(
                &skeleton_graphs,
                &ground_graphs,
                &transition_graphs,
                id,
                graphs[gi].offset,
            );
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

            for (p, converted) in plan.iter().zip(mesh::convert_sequence(
                &items,
                mesh::Source::of(file),
                initial,
            )) {
                let Ok(mut m) = converted else { continue };
                map_vertex_bindings(&mut m, plan);
                if m.triangle_count() == 0 {
                    continue;
                }
                let index = pack_mesh(
                    ctx,
                    &mut writer,
                    &mut tex_index,
                    &mut mat_anim_index,
                    &mat_anim_data,
                    Texels {
                        home: file,
                        all: &loaded.files,
                    },
                    id,
                    p.dl,
                    &m,
                    swizzle,
                );
                meshes += 1;
                triangles += m.triangle_count();

                // A node holds one mesh index, but a link array or a pre/post
                // pair can name several lists. The one drawn under the node's
                // own matrix goes on the node; the rest become extra leaves in
                // whichever space they actually run in.
                let slot = &mut node_mesh[gi][p.node];
                if p.own_space() && slot.is_none() {
                    *slot = Some(index);
                } else {
                    node_extra[gi].push((p.space, index));
                    extra_leaves += 1;
                }
            }
        }

        // Discovery fills in only what the graphs never named.
        for dl in all
            .iter()
            .filter(|d| !called.contains(&d.offset) && !authoritative.contains(&d.offset))
        {
            if unconvertible_without_materials(&dl.commands, file) {
                continue;
            }
            let Ok(m) = convert_discovered(&dl.commands, file) else {
                continue;
            };
            if m.triangle_count() == 0 {
                continue;
            }
            pack_mesh(
                ctx,
                &mut writer,
                &mut tex_index,
                &mut mat_anim_index,
                &mat_anim_data,
                Texels {
                    home: file,
                    all: &loaded.files,
                },
                id,
                dl.offset,
                &m,
                swizzle,
            );
            meshes += 1;
            triangles += m.triangle_count();
        }

        // Mario's Fireball graphic is not attached to a `DObjDesc` tree: the
        // weapon descriptor points straight at file 297's vertex/display-list
        // pair and separately supplies the one-entry MObj table at 0xD8.
        // Generic graph discovery correctly finds no object there, so retain
        // this explicit, source-proven asset instead of inventing a PSP-side
        // substitute primitive.
        //
        // Luigi's `WPAttributes` (file 222) names the same list and table.
        // `wpMarioFireballMakeWeapon` only sets `mobj->palette_id` from the
        // row's `anim_frame`: 0 for Mario, 1 for Luigi. Those two rows bound
        // `palettes[]` to two entries, so each palette gets its own mesh. The
        // shared list cannot key Luigi's copy, so it is keyed by the palette
        // it binds (`palettes[1]`, file offset 0x08).
        if id == 297 {
            const FIREBALL_MOBJ_TABLE: u32 = 0xD8;
            // The mesh key: the decomp's `SuperJumpPunchDL` label, which the
            // runtime looks the mesh up by (`psp-runtime` `scene.rs`).
            const FIREBALL_DISPLAY_LIST: u32 = 0x1D8;
            // `WPAttributes.data` (`llMarioMainFireballWeaponAttributes`
            // relocates here) and where the conversion starts: the list
            // starts six commands earlier than the label, setting its own
            // TLUT (`G_TT_RGBA16`), alpha compare (threshold), render mode
            // (`0x00553048`, `G_RM_AA_TEX_EDGE`) and combiner, as the Charge
            // Shot's (file 321) and Falcon Punch's (file 333) lists do. The
            // render mode maps to the RE-424 alpha-edge material.
            const FIREBALL_DATA: u32 = 0x1A8;
            const LUIGI_FIREBALL_KEY: u32 = 0x08;
            if let (Some(materials), Ok(cmds)) = (
                ssb_rom::mobj::read_table(file, FIREBALL_MOBJ_TABLE, 1),
                ssb_rom::dl::decode_list_at(&file.data[FIREBALL_DATA as usize..], FIREBALL_DATA),
            ) {
                let palettes = materials.nodes[0]
                    .first()
                    .and_then(|sub| ssb_rom::mobj::read_palettes(file, sub.at, 2))
                    .expect("file 297 Fireball MObjSub has Mario and Luigi palettes");
                for (palette, key) in palettes
                    .iter()
                    .zip([FIREBALL_DISPLAY_LIST, LUIGI_FIREBALL_KEY])
                {
                    let mut mobjs = materials.nodes[0].clone();
                    mobjs[0].palette = Some(*palette);
                    let item = mesh::SequenceItem {
                        cmds: &cmds,
                        world: ssb_rom::scene::Mat4::IDENTITY,
                        mobjs: &mobjs,
                        mat_anims: &[],
                        depth_seed: None,
                        // `wpDisplayDLHead1` draws the whole list on head 1.
                        stream: weapon_stream((297, FIREBALL_DATA)),
                    };
                    let Some(Ok(fireball)) = mesh::convert_sequence(
                        &[item],
                        mesh::Source::of(file),
                        mesh::InitialMaterial::WEAPON_EXTERNAL,
                    )
                    .into_iter()
                    .next() else {
                        continue;
                    };
                    if fireball.triangle_count() != 0 {
                        pack_mesh(
                            ctx,
                            &mut writer,
                            &mut tex_index,
                            &mut mat_anim_index,
                            &mat_anim_data,
                            Texels {
                                home: file,
                                all: &loaded.files,
                            },
                            id,
                            key,
                            &fireball,
                            swizzle,
                        );
                        meshes += 1;
                        triangles += fireball.triangle_count();
                    }
                }
            }
        }

        // Fox Special1's `WPAttributes.data` relocates to file 316 + 0x40.
        // Like Mario's Fireball, this weapon has a direct display list and
        // no DObj graph for generic scene discovery to place.
        if id == 316 {
            const BLASTER_DISPLAY_LIST: u32 = 0x40;
            if let Some(Ok(blaster)) = file
                .data
                .get(BLASTER_DISPLAY_LIST as usize..)
                .and_then(|data| ssb_rom::dl::decode_list_at(data, BLASTER_DISPLAY_LIST).ok())
                .and_then(|cmds| {
                    mesh::convert_sequence(
                        &[mesh::SequenceItem {
                            cmds: &cmds,
                            world: ssb_rom::scene::Mat4::IDENTITY,
                            mobjs: &[],
                            mat_anims: &[],
                            depth_seed: None,
                            stream: weapon_stream((316, BLASTER_DISPLAY_LIST)),
                        }],
                        mesh::Source::of(file),
                        mesh::InitialMaterial::WEAPON_EXTERNAL,
                    )
                    .into_iter()
                    .next()
                })
            {
                if blaster.triangle_count() != 0 {
                    pack_mesh(
                        ctx,
                        &mut writer,
                        &mut tex_index,
                        &mut mat_anim_index,
                        &mat_anim_data,
                        Texels {
                            home: file,
                            all: &loaded.files,
                        },
                        id,
                        BLASTER_DISPLAY_LIST,
                        &blaster,
                        swizzle,
                    );
                    meshes += 1;
                    triangles += blaster.triangle_count();
                }
            }
        }

        // Samus Special1's `WPAttributes.data` relocates to file 321 + 0x270,
        // with no `MObjSub` table: the list loads its own texture and draws
        // one 30x30 quad (vertices at 0x230). The decomp's symbol split
        // labels 0x230..0x2A0 as seven vertices; the bytes from 0x270 are
        // display-list words that run on to the `G_ENDDL` at 0x348.
        if id == 321 {
            const CHARGE_SHOT_DISPLAY_LIST: u32 = 0x270;
            if let Some(Ok(shot)) = file
                .data
                .get(CHARGE_SHOT_DISPLAY_LIST as usize..)
                .and_then(|data| ssb_rom::dl::decode_list_at(data, CHARGE_SHOT_DISPLAY_LIST).ok())
                .and_then(|cmds| {
                    mesh::convert_sequence(
                        &[mesh::SequenceItem {
                            cmds: &cmds,
                            world: ssb_rom::scene::Mat4::IDENTITY,
                            mobjs: &[],
                            mat_anims: &[],
                            depth_seed: None,
                            stream: weapon_stream((321, CHARGE_SHOT_DISPLAY_LIST)),
                        }],
                        mesh::Source::of(file),
                        mesh::InitialMaterial::WEAPON_EXTERNAL,
                    )
                    .into_iter()
                    .next()
                })
            {
                if shot.triangle_count() != 0 {
                    pack_mesh(
                        ctx,
                        &mut writer,
                        &mut tex_index,
                        &mut mat_anim_index,
                        &mat_anim_data,
                        Texels {
                            home: file,
                            all: &loaded.files,
                        },
                        id,
                        CHARGE_SHOT_DISPLAY_LIST,
                        &shot,
                        swizzle,
                    );
                    meshes += 1;
                    triangles += shot.triangle_count();
                }
            }
        }

        // The Shells share file 86's tree at 0x5F88 (`llITCommonDataShell*`).
        // `itGShellMakeItem` sets `mobj->palette_id` to 1 and
        // `itRShellMakeItem` to 0; the tree's own mesh binds palette 0 (red),
        // so the Green Shell's list (0x5EC0) gets a second mesh, keyed by the
        // palette it binds (`palettes[1]`, file offset 0x5578), as Luigi's
        // Fireball is (RE-434). Its spin script's sprite frames convert
        // under that palette, so its `MatAnimRef` names the palette rather
        // than the `MObjSub` (RE-442).
        //
        // `itBombHeiCommonSetWalkLR` swaps the Bob-omb root's list between
        // the tree's (0x3310, right) and 0x34C0 (left), under the same
        // `MObj` and walk script (RE-442).
        //
        // The Bumper's hit sets `palette_id` 1 (`itNBumper*ProcHit`), and
        // `itNBumperAttachedInitVars` swaps its root to the Wait list
        // (0x7AF8) under the Wait `MObjSub` (0x7A38) alone
        // (`gcRemoveMObjAll` + `gcAddMObjForDObj`). Each lit list is keyed
        // by the palette it binds, the attached unlit one by the Wait
        // `MObjSub` (RE-442).
        if id == 86 {
            const SHELL_MOBJ_TABLE: u32 = 0x5DE0;
            const SHELL_DISPLAY_LIST: u32 = 0x5EC0;
            const GREEN_SHELL_KEY: u32 = 0x5578;
            const BOMBHEI_MOBJ_TABLE: u32 = 0x3230;
            const BOMBHEI_LEFT_LIST: u32 = 0x34C0;
            const BUMPER_MOBJ_TABLE: u32 = 0x7488;
            const BUMPER_LIST: u32 = 0x7558;
            const BUMPER_LIT_KEY: u32 = 0x7238;
            const BUMPER_WAIT_MOBJ: u32 = 0x7A38;
            const BUMPER_WAIT_LIST: u32 = 0x7AF8;
            const BUMPER_WAIT_LIT_KEY: u32 = 0x76D8;
            const MBALL_MOBJ_TABLE: u32 = 0x9120;
            const MBALL_BALL_NODE: usize = 3;
            const MBALL_BALL_LIST: u32 = 0x9340;
            const MBALL_SCRIPT: u32 = 0x9520;
            let mut variant =
                |dl: u32,
                 mobjs: &[ssb_rom::mobj::MObjMaterial],
                 mat_anims: &[Option<ssb_rom::mesh::MatAnimRef>],
                 key: u32,
                 initial: mesh::InitialMaterial,
                 stream: u8,
                 mat_anim_data: &BTreeMap<ssb_rom::mesh::MatAnimRef, MatAnimData>| {
                    let cmds = file
                        .data
                        .get(dl as usize..)
                        .and_then(|data| ssb_rom::dl::decode_list_at(data, dl).ok())
                        .unwrap_or_else(|| panic!("file 86 list 0x{dl:X} decodes"));
                    let item = mesh::SequenceItem {
                        cmds: &cmds,
                        world: ssb_rom::scene::Mat4::IDENTITY,
                        mobjs,
                        mat_anims,
                        depth_seed: None,
                        stream,
                    };
                    let converted =
                        mesh::convert_sequence(&[item], mesh::Source::of(file), initial)
                            .into_iter()
                            .next()
                            .and_then(Result::ok)
                            .filter(|m| m.triangle_count() != 0)
                            .unwrap_or_else(|| panic!("file 86 list 0x{dl:X} converts"));
                    pack_mesh(
                        ctx,
                        &mut writer,
                        &mut tex_index,
                        &mut mat_anim_index,
                        mat_anim_data,
                        Texels {
                            home: file,
                            all: &loaded.files,
                        },
                        id,
                        key,
                        &converted,
                        swizzle,
                    );
                    meshes += 1;
                    triangles += converted.triangle_count();
                };
            // The script each root list runs, as the tree's own conversion
            // resolved it.
            let root_ref = |data: &BTreeMap<ssb_rom::mesh::MatAnimRef, MatAnimData>,
                            mobj: u32,
                            script: u32| {
                let key = ssb_rom::mesh::MatAnimRef {
                    source_file: id,
                    script,
                    source_mobj: mobj,
                };
                assert!(
                    data.contains_key(&key),
                    "file 86 script 0x{script:X} on MObjSub 0x{mobj:X} resolved"
                );
                key
            };

            let shell = ssb_rom::mobj::read_table(file, SHELL_MOBJ_TABLE, 2)
                .expect("file 86 Shell MObjSub table");
            let palettes = shell.nodes[1]
                .first()
                .and_then(|sub| ssb_rom::mobj::read_palettes(file, sub.at, 2))
                .expect("file 86 Shell MObjSub has red and green palettes");
            assert_eq!(palettes[1].offset, GREEN_SHELL_KEY);
            let mut mobjs = shell.nodes[1].clone();
            mobjs[0].palette = Some(palettes[1]);
            let red = root_ref(&mat_anim_data, mobjs[0].at, 0x6048);
            let green = ssb_rom::mesh::MatAnimRef {
                source_mobj: GREEN_SHELL_KEY,
                ..red
            };
            let data = mat_anim_data.get(&red).map(|d| MatAnimData {
                source_offset: d.source_offset,
                palette_entries: d.palette_entries,
                palettes: d.palettes.clone(),
                sprites: d.sprites.clone(),
                base_tracks: d.base_tracks,
                uv_mode: d.uv_mode,
                uv_tile_params: d.uv_tile_params,
                uv_half: d.uv_half,
                uv_static: d.uv_static,
                drives_lod: d.drives_lod,
                max_current: d.max_current,
                current_closed: d.current_closed,
                max_lod_frac: d.max_lod_frac,
                extra_scripts: d.extra_scripts.clone(),
            });
            if let Some(data) = data {
                mat_anim_data.insert(green, data);
            }
            variant(
                SHELL_DISPLAY_LIST,
                &mobjs,
                &[Some(green)],
                GREEN_SHELL_KEY,
                mesh::InitialMaterial::SCENE,
                0,
                &mat_anim_data,
            );

            let bombhei = ssb_rom::mobj::read_table(file, BOMBHEI_MOBJ_TABLE, 2)
                .expect("file 86 Bob-omb MObjSub table");
            let walk = root_ref(&mat_anim_data, bombhei.nodes[1][0].at, 0x35B8);
            variant(
                BOMBHEI_LEFT_LIST,
                &bombhei.nodes[1],
                &[Some(walk)],
                BOMBHEI_LEFT_LIST,
                mesh::InitialMaterial::SCENE,
                0,
                &mat_anim_data,
            );

            let bumper = ssb_rom::mobj::read_table(file, BUMPER_MOBJ_TABLE, 2)
                .expect("file 86 Bumper MObjSub table");
            let lit = |sub: &ssb_rom::mobj::MObjMaterial, key: u32| {
                let palettes = ssb_rom::mobj::read_palettes(file, sub.at, 2)
                    .expect("file 86 Bumper MObjSub has two palettes");
                assert_eq!(palettes[1].offset, key);
                ssb_rom::mobj::MObjMaterial {
                    palette: Some(palettes[1]),
                    ..*sub
                }
            };
            let lit_root = vec![lit(&bumper.nodes[1][0], BUMPER_LIT_KEY)];
            variant(
                BUMPER_LIST,
                &lit_root,
                &[None],
                BUMPER_LIT_KEY,
                mesh::InitialMaterial::SCENE,
                0,
                &mat_anim_data,
            );
            let wait = ssb_rom::mobj::read_sub(file, BUMPER_WAIT_MOBJ)
                .expect("file 86 Bumper Wait MObjSub");
            let lit_wait = vec![lit(&wait, BUMPER_WAIT_LIT_KEY)];
            variant(
                BUMPER_WAIT_LIST,
                &[wait],
                &[None],
                BUMPER_WAIT_MOBJ,
                mesh::InitialMaterial::SCENE,
                0,
                &mat_anim_data,
            );
            variant(
                BUMPER_WAIT_LIST,
                &lit_wait,
                &[None],
                BUMPER_WAIT_LIT_KEY,
                mesh::InitialMaterial::SCENE,
                0,
                &mat_anim_data,
            );

            // `itMBallOpenAddAnim` adds `llITCommonDataMBallMatAnimJoint`
            // (0x9520, a looping cycle of the eight ball textures) to the
            // closed ball's `MObj` when the ball is thrown or dropped. The
            // tree itself binds the entry effect's table (0x9740), whose
            // script ends, so the ball's list gets a second mesh keyed by
            // the item's script (RE-443).
            let mball = ssb_rom::mobj::read_table(file, MBALL_MOBJ_TABLE, 4)
                .expect("file 86 Poké Ball MObjSub table");
            let scripts: Vec<Vec<Option<u32>>> = mball
                .nodes
                .iter()
                .enumerate()
                .map(|(n, chain)| {
                    (0..chain.len())
                        .map(|m| (n == MBALL_BALL_NODE && m == 0).then_some(MBALL_SCRIPT))
                        .collect()
                })
                .collect();
            let refs = resolve_script_matrix(
                file,
                &scripts,
                &mball.nodes,
                |node, m| {
                    mball
                        .nodes
                        .get(node)?
                        .get(m)
                        .map(|s| (s.at, s.palette_entries))
                },
                &mut mat_anim_data,
            );
            let ball = refs[MBALL_BALL_NODE][0];
            assert!(ball.is_some(), "file 86 Poké Ball script 0x9520 resolved");
            variant(
                MBALL_BALL_LIST,
                &mball.nodes[MBALL_BALL_NODE],
                &[ball],
                MBALL_SCRIPT,
                mesh::InitialMaterial::SCENE,
                0,
                &mat_anim_data,
            );

            // Status callbacks draw Snorlax's fall and Clefairy's
            // Hitmonlee/Starmie imitations under AA_XLU_SURF. Clefairy's
            // swarm weapon uses the same wrapper. Seed before converting
            // so any render-mode write in the list retains precedence.
            for key in [
                ssb_rom::mmonster::KABIGON_FALL_MESH,
                ssb_rom::mmonster::PIPPI_XLU_MESH,
            ] {
                let graph = graphs
                    .iter()
                    .find(|g| g.offset == key)
                    .expect("Pokémon tree");
                let materials = loaded.materials(file, graph);
                let dl = graph.nodes[1].desc.dl.expect("Pokémon root list");
                variant(
                    dl,
                    &materials[1],
                    &[],
                    key,
                    mesh::InitialMaterial::WEAPON_EXTERNAL,
                    // Head 0: item displays, and Clefairy's swarm weapon's
                    // (`ssb_rom::task_head::HEAD0_DISPLAY_WEAPONS`).
                    0,
                    &mat_anim_data,
                );
            }

            // The rock maker assigns texture_id_curr directly, with no
            // material script. Convert each of the three original sprites.
            let graph = graphs
                .iter()
                .find(|g| g.offset == ssb_rom::mmonster::ROCK_GRAPH)
                .expect("rock tree");
            let materials = loaded.materials(file, graph);
            let root = &materials[1];
            let sprites =
                ssb_rom::mobj::read_sprites(file, root[0].at, 3).expect("three rock sprites");
            for (sprite, key) in sprites
                .into_iter()
                .zip(ssb_rom::mmonster::ROCK_TEXTURE_KEYS)
            {
                assert_eq!(sprite.offset, key);
                let mut mobjs = root.clone();
                mobjs[0].sprite = Some(sprite);
                variant(
                    graph.nodes[1].desc.dl.expect("rock list"),
                    &mobjs,
                    &[],
                    key,
                    mesh::InitialMaterial::WEAPON_EXTERNAL,
                    weapon_stream((ssb_rom::mmonster::FILE, ssb_rom::mmonster::ROCK_GRAPH)),
                    &mat_anim_data,
                );
            }
        }

        // Samus's Bomb (`llSamusMainBombWeaponAttributes`, file 217 + 0x0C)
        // names file 320's direct list at 0xE0D8 and the `MObjSub` table at
        // 0xE008. `wpSamusBombProcUpdate` blinks by toggling
        // `mobj->palette_id` between 0 and 1, so both palettes get a mesh.
        // The second is keyed by the palette it binds (`palettes[1]`, file
        // offset 0xDF38), as Luigi's Fireball is.
        if id == 320 {
            const BOMB_MOBJ_TABLE: u32 = 0xE008;
            const BOMB_DISPLAY_LIST: u32 = 0xE0D8;
            const BOMB_BLINK_KEY: u32 = 0xDF38;
            if let (Some(materials), Some(Ok(cmds))) = (
                ssb_rom::mobj::read_table(file, BOMB_MOBJ_TABLE, 1),
                file.data
                    .get(BOMB_DISPLAY_LIST as usize..)
                    .map(|data| ssb_rom::dl::decode_list_at(data, BOMB_DISPLAY_LIST)),
            ) {
                let palettes = materials.nodes[0]
                    .first()
                    .and_then(|sub| ssb_rom::mobj::read_palettes(file, sub.at, 2))
                    .expect("file 320 Bomb MObjSub has two blink palettes");
                for (palette, key) in palettes.iter().zip([BOMB_DISPLAY_LIST, BOMB_BLINK_KEY]) {
                    let mut mobjs = materials.nodes[0].clone();
                    mobjs[0].palette = Some(*palette);
                    let item = mesh::SequenceItem {
                        cmds: &cmds,
                        world: ssb_rom::scene::Mat4::IDENTITY,
                        mobjs: &mobjs,
                        mat_anims: &[],
                        depth_seed: None,
                        stream: weapon_stream((320, BOMB_DISPLAY_LIST)),
                    };
                    let Some(Ok(bomb)) = mesh::convert_sequence(
                        &[item],
                        mesh::Source::of(file),
                        mesh::InitialMaterial::WEAPON_EXTERNAL,
                    )
                    .into_iter()
                    .next() else {
                        continue;
                    };
                    if bomb.triangle_count() != 0 {
                        pack_mesh(
                            ctx,
                            &mut writer,
                            &mut tex_index,
                            &mut mat_anim_index,
                            &mat_anim_data,
                            Texels {
                                home: file,
                                all: &loaded.files,
                            },
                            id,
                            key,
                            &bomb,
                            swizzle,
                        );
                        meshes += 1;
                        triangles += bomb.triangle_count();
                    }
                }
            }
        }

        // Saffron's makers select MObj texture IDs directly, without a
        // material animation. Pack both source sprites as distinct meshes.
        if id == ssb_rom::ground_obj::MONSTER_FILE {
            for (graph_at, table_at, expected) in [
                (0x1990, 0x17D0, ssb_rom::ground_obj::MONSTER_TEXTURES[0]),
                (0x2340, 0x2180, ssb_rom::ground_obj::MONSTER_TEXTURES[1]),
            ] {
                let graph = loaded.graphs[&id]
                    .iter()
                    .find(|g| g.offset == graph_at)
                    .ok_or("monster graph missing")?;
                let materials = ssb_rom::mobj::read_table(file, table_at, graph.nodes.len())
                    .ok_or("monster materials missing")?;
                let mobjs = &materials.nodes[1];
                let sprites = ssb_rom::mobj::read_sprites(file, mobjs[0].at, 2)
                    .ok_or("monster sprites missing")?;
                assert!(sprites.iter().map(|s| s.offset).eq(expected));
                let dl = graph.nodes[1]
                    .desc
                    .dl
                    .ok_or("monster display list missing")?;
                let cmds = ssb_rom::dl::decode_list_at(&file.data[dl as usize..], dl)?;
                for sprite in sprites {
                    let mut chain = mobjs.clone();
                    chain[0].sprite = Some(sprite);
                    let item = mesh::SequenceItem {
                        cmds: &cmds,
                        world: ssb_rom::scene::Mat4::IDENTITY,
                        mobjs: &chain,
                        mat_anims: &[],
                        depth_seed: None,
                        stream: 0,
                    };
                    let frame = mesh::convert_sequence(
                        &[item],
                        mesh::Source::of(file),
                        // `itDisplayOPAProcDisplay` walks the item tree
                        // without the weapon wrapper's translucent seed.
                        mesh::InitialMaterial::SCENE,
                    )
                    .into_iter()
                    .next()
                    .ok_or("monster mesh missing")?
                    .map_err(|e| format!("monster mesh: {e:?}"))?;
                    pack_mesh(
                        ctx,
                        &mut writer,
                        &mut tex_index,
                        &mut mat_anim_index,
                        &mat_anim_data,
                        Texels {
                            home: file,
                            all: &loaded.files,
                        },
                        id,
                        sprite.offset,
                        &frame,
                        swizzle,
                    );
                    meshes += 1;
                    triangles += frame.triangle_count();
                }
            }
        }

        // Pikachu's Thunder (RE-417). The head and trail
        // (`llPikachuMainThunderHeadWeaponAttributes`, file 243 + 0x0C, and
        // `...TrailWeaponAttributes`, + 0x40) and the fading segment
        // (`dEFManagerPikachuThunderTrailEffectDesc`) all name file 341's
        // `DObjDLLink` array at 0x95B0 (one list, 0x94F8) and the `MObjSub`
        // table at 0x9420. Their code picks the frame by
        // `mobj->texture_id_curr` (the head 3, a trail and a segment
        // `syUtilsRandIntRange(3)`, a segment's last frame 3), with no
        // material script, so each of the four sprites gets a mesh, keyed by
        // the sprite it binds. `wpDisplayDrawNormal` and
        // `efManagerPikachuThunderTrailProcDisplay` both draw it under
        // `G_RM_AA_XLU_SURF` without a depth test: the weapon seed.
        if id == 341 {
            const THUNDER_MOBJ_TABLE: u32 = 0x9420;
            const THUNDER_DISPLAY_LIST: u32 = 0x94F8;
            // The array's one `DObjDLLink` (`{ list_id, dl }`) names the
            // task head the head, trail and segment draw the list on
            // (`gcDrawDObjDLLinksForGObj`).
            const THUNDER_LINKS: u32 = 0x95B0;
            assert_eq!(
                ssb_rom::sprite::pointer_place(file, THUNDER_LINKS + 4),
                Some((341, THUNDER_DISPLAY_LIST)),
                "file 341 Thunder link names its list"
            );
            let thunder_list_id = u32::from_be_bytes(
                file.data[THUNDER_LINKS as usize..][..4]
                    .try_into()
                    .expect("four bytes"),
            );
            if let (Some(materials), Some(Ok(cmds))) = (
                ssb_rom::mobj::read_table(file, THUNDER_MOBJ_TABLE, 1),
                file.data
                    .get(THUNDER_DISPLAY_LIST as usize..)
                    .map(|data| ssb_rom::dl::decode_list_at(data, THUNDER_DISPLAY_LIST)),
            ) {
                let sprites = materials.nodes[0]
                    .first()
                    .and_then(|sub| {
                        ssb_rom::mobj::read_sprites(
                            file,
                            sub.at,
                            ssb_rom::effect::PIKACHU_THUNDER_SPRITES.len(),
                        )
                    })
                    .expect("file 341 Thunder MObjSub has four sprites");
                assert!(
                    sprites.iter().map(|s| (s.file, s.offset)).eq(
                        ssb_rom::effect::PIKACHU_THUNDER_SPRITES
                            .iter()
                            .map(|&o| (None, o))
                    ),
                    "file 341 Thunder sprites moved"
                );
                for sprite in &sprites {
                    let mut mobjs = materials.nodes[0].clone();
                    mobjs[0].sprite = Some(*sprite);
                    let item = mesh::SequenceItem {
                        cmds: &cmds,
                        world: ssb_rom::scene::Mat4::IDENTITY,
                        mobjs: &mobjs,
                        mat_anims: &[],
                        depth_seed: None,
                        stream: u8::from(thunder_list_id == 1),
                    };
                    let Some(Ok(frame)) = mesh::convert_sequence(
                        &[item],
                        mesh::Source::of(file),
                        mesh::InitialMaterial::WEAPON_EXTERNAL,
                    )
                    .into_iter()
                    .next() else {
                        continue;
                    };
                    if frame.triangle_count() != 0 {
                        pack_mesh(
                            ctx,
                            &mut writer,
                            &mut tex_index,
                            &mut mat_anim_index,
                            &mat_anim_data,
                            Texels {
                                home: file,
                                all: &loaded.files,
                            },
                            id,
                            sprite.offset,
                            &frame,
                            swizzle,
                        );
                        meshes += 1;
                        triangles += frame.triangle_count();
                    }
                }
            }
        }

        // Ness's PK Thunder trail (RE-550). `wpNessPKThunderTrailProcUpdate`
        // picks the frame by `mobj->texture_id_curr =
        // syUtilsRandIntRange(WPPKTHUNDER_TEXTURES_NUM - 1)`, with no
        // material script, so each of `MObjSub` 0x89D8's three sprites gets a
        // mesh of the trail's one list (link 1 of the `DObjDLLink` array at
        // 0x8B40), keyed by the sprite it binds. Same seed, head and
        // PRIM/ENV splice as the graph's own mesh (`PK_THUNDER_TRAIL_GRAPH`).
        if id == ssb_rom::effect::NESS_PK_TRAIL_FILE {
            const TRAIL_MOBJ_TABLE: u32 = 0x89C8;
            let list = ssb_rom::effect::NESS_PK_TRAIL_LIST;
            if let (Some(materials), Some(Ok(cmds))) = (
                ssb_rom::mobj::read_table(file, TRAIL_MOBJ_TABLE, 1),
                file.data
                    .get(list as usize..)
                    .map(|data| ssb_rom::dl::decode_list_at(data, list)),
            ) {
                let mut decoded = [cmds];
                seed_player_interface_commands(PK_THUNDER_TRAIL_GRAPH, &mut decoded);
                let [cmds] = decoded;
                let sprites = materials.nodes[0]
                    .first()
                    .and_then(|sub| {
                        ssb_rom::mobj::read_sprites(
                            file,
                            sub.at,
                            ssb_rom::effect::NESS_PK_TRAIL_SPRITES.len(),
                        )
                    })
                    .expect("file 335 PK Thunder trail MObjSub has three sprites");
                assert!(
                    sprites.iter().map(|s| (s.file, s.offset)).eq(
                        ssb_rom::effect::NESS_PK_TRAIL_SPRITES
                            .iter()
                            .map(|&o| (None, o))
                    ),
                    "file 335 PK Thunder trail sprites moved"
                );
                let initial = initial_material_for(
                    &skeleton_graphs,
                    &ground_graphs,
                    &transition_graphs,
                    id,
                    PK_THUNDER_TRAIL_GRAPH.1,
                );
                for sprite in &sprites {
                    let mut mobjs = materials.nodes[0].clone();
                    mobjs[0].sprite = Some(*sprite);
                    let item = mesh::SequenceItem {
                        cmds: &cmds,
                        world: ssb_rom::scene::Mat4::IDENTITY,
                        mobjs: &mobjs,
                        mat_anims: &[],
                        depth_seed: None,
                        // Link 1: head 1, as the graph's own plan converts it.
                        stream: 1,
                    };
                    let Some(Ok(frame)) =
                        mesh::convert_sequence(&[item], mesh::Source::of(file), initial)
                            .into_iter()
                            .next()
                    else {
                        continue;
                    };
                    if frame.triangle_count() != 0 {
                        pack_mesh(
                            ctx,
                            &mut writer,
                            &mut tex_index,
                            &mut mat_anim_index,
                            &mat_anim_data,
                            Texels {
                                home: file,
                                all: &loaded.files,
                            },
                            id,
                            sprite.offset,
                            &frame,
                            swizzle,
                        );
                        meshes += 1;
                        triangles += frame.triangle_count();
                    }
                }
            }
        }

        // Yoshi's Bomb star (`llYoshiMainStarWeaponAttributes`, file 247 +
        // 0x40) names file 86's direct list at 0x5458, with no `MObjSub`
        // table. The list sets its own combiner but no render mode, so it
        // draws under `wpDisplayDrawNormal`'s `G_RM_AA_XLU_SURF` without
        // `G_ZBUFFER`, like the Fireball. Discovery already packs the same
        // list, keyed (86, 0x5458), under the default seed, so this copy is
        // keyed by the attributes record that names it: (247, 0x40).
        if id == 86 || id == ssb_rom::ground_obj::BONUS3_NODES_FILE {
            // The Box and TaruBomb smash makers name these lists directly;
            // it has no descriptor for graph discovery to reach.
            let container_piece = if id == 86 { 0x68F0 } else { 0x8A0 };
            let cmds = ssb_rom::dl::decode_list_at(
                &file.data[container_piece as usize..],
                container_piece,
            )?;
            let piece = mesh::convert_sequence(
                &[mesh::SequenceItem {
                    cmds: &cmds,
                    world: ssb_rom::scene::Mat4::IDENTITY,
                    mobjs: &[],
                    mat_anims: &[],
                    depth_seed: None,
                    stream: 0,
                }],
                mesh::Source::of(file),
                mesh::InitialMaterial::SCENE,
            )
            .into_iter()
            .next()
            .ok_or("container piece missing")?
            .map_err(|e| format!("container piece conversion: {e:?}"))?;
            pack_mesh(
                ctx,
                &mut writer,
                &mut tex_index,
                &mut mat_anim_index,
                &mat_anim_data,
                Texels {
                    home: file,
                    all: &loaded.files,
                },
                id,
                container_piece,
                &piece,
                swizzle,
            );
            meshes += 1;
            triangles += piece.triangle_count();
        }
        if id == 86 {
            const STAR_DISPLAY_LIST: u32 = 0x5458;
            const STAR_KEY: (u32, u32) = (247, 0x40);
            if let Some(Ok(star)) = file
                .data
                .get(STAR_DISPLAY_LIST as usize..)
                .and_then(|data| ssb_rom::dl::decode_list_at(data, STAR_DISPLAY_LIST).ok())
                .and_then(|cmds| {
                    mesh::convert_sequence(
                        &[mesh::SequenceItem {
                            cmds: &cmds,
                            world: ssb_rom::scene::Mat4::IDENTITY,
                            mobjs: &[],
                            mat_anims: &[],
                            depth_seed: None,
                            stream: weapon_stream((86, STAR_DISPLAY_LIST)),
                        }],
                        mesh::Source::of(file),
                        mesh::InitialMaterial::WEAPON_EXTERNAL,
                    )
                    .into_iter()
                    .next()
                })
            {
                if star.triangle_count() != 0 {
                    pack_mesh(
                        ctx,
                        &mut writer,
                        &mut tex_index,
                        &mut mat_anim_index,
                        &mat_anim_data,
                        Texels {
                            home: file,
                            all: &loaded.files,
                        },
                        STAR_KEY.0,
                        STAR_KEY.1,
                        &star,
                        swizzle,
                    );
                    meshes += 1;
                    triangles += star.triangle_count();
                }
            }
            // Ray Gun ammo: the shared item's standalone display list.
            const RAY_DISPLAY_LIST: u32 = 0x40A8;
            const RAY_KEY: (u32, u32) = (251, 0x2B0);
            if let Some(Ok(ray)) = file
                .data
                .get(RAY_DISPLAY_LIST as usize..)
                .and_then(|data| ssb_rom::dl::decode_list_at(data, RAY_DISPLAY_LIST).ok())
                .and_then(|cmds| {
                    mesh::convert_sequence(
                        &[mesh::SequenceItem {
                            cmds: &cmds,
                            world: ssb_rom::scene::Mat4::IDENTITY,
                            mobjs: &[],
                            mat_anims: &[],
                            depth_seed: None,
                            stream: weapon_stream((86, RAY_DISPLAY_LIST)),
                        }],
                        mesh::Source::of(file),
                        mesh::InitialMaterial::WEAPON_EXTERNAL,
                    )
                    .into_iter()
                    .next()
                })
            {
                if ray.triangle_count() != 0 {
                    pack_mesh(
                        ctx,
                        &mut writer,
                        &mut tex_index,
                        &mut mat_anim_index,
                        &mat_anim_data,
                        Texels {
                            home: file,
                            all: &loaded.files,
                        },
                        RAY_KEY.0,
                        RAY_KEY.1,
                        &ray,
                        swizzle,
                    );
                    meshes += 1;
                    triangles += ray.triangle_count();
                }
            }
        }

        // Sector Z's Arwing lasers (RE-428): both `WPAttributes` (file 262
        // + 0xBC and + 0xF0) name file 153's list at 0x1C50, whose textures
        // are file 161's. Keyed by the 2D record; the 3D shot draws the same
        // list.
        if id == ssb_rom::sector::FLIGHT_FILE {
            const LASER_KEY: (u32, u32) = (
                ssb_rom::sector::MAP_FILE,
                ssb_rom::sector::LASER_2D_ATTRIBUTES,
            );
            let dl = ssb_rom::sector::LASER_DISPLAY_LIST;
            if let Some(Ok(laser)) = file
                .data
                .get(dl as usize..)
                .and_then(|data| ssb_rom::dl::decode_list_at(data, dl).ok())
                .and_then(|cmds| {
                    mesh::convert_sequence(
                        &[mesh::SequenceItem {
                            cmds: &cmds,
                            world: ssb_rom::scene::Mat4::IDENTITY,
                            mobjs: &[],
                            mat_anims: &[],
                            depth_seed: None,
                            stream: weapon_stream((ssb_rom::sector::FLIGHT_FILE, dl)),
                        }],
                        mesh::Source::of(file),
                        mesh::InitialMaterial::WEAPON_EXTERNAL,
                    )
                    .into_iter()
                    .next()
                })
            {
                if laser.triangle_count() != 0 {
                    pack_mesh(
                        ctx,
                        &mut writer,
                        &mut tex_index,
                        &mut mat_anim_index,
                        &mat_anim_data,
                        Texels {
                            home: file,
                            all: &loaded.files,
                        },
                        LASER_KEY.0,
                        LASER_KEY.1,
                        &laser,
                        swizzle,
                    );
                    meshes += 1;
                    triangles += laser.triangle_count();
                }
            }
        }

        for (gi, graph) in graphs.iter().enumerate() {
            node_dls += graph.display_lists().count();
            placed_meshes += node_mesh[gi].iter().filter(|m| m.is_some()).count();
            let object = writer.add_object(graph, id, |n| node_mesh[gi][n], &node_extra[gi]);
            object_index.insert((id, graph.offset), object);
            if (id, graph.offset) == ssb_rom::effect::ENTRY_CAR_KEY {
                let first = writer.object(object).unwrap().first_node;
                for n in ssb_rom::effect::ENTRY_CAR_BILLBOARD_NODES {
                    writer.add_node_flags(first + n, ssb_rom::pack::NodeDesc::FLAG_BILLBOARD);
                }
            }
            objects += 1;

            // RE-098: a fighter's alternate costumes share this one object's
            // node table. Most nodes draw identically at every costume
            // (measured archive-wide: a third to two-thirds actually
            // differ, never all of them), so only the (node, costume) pairs
            // whose *converted mesh content* genuinely differs from costume
            // 0 get their own substitute mesh, looked up by global node
            // index at draw time (`Pack::costume_mesh`). Comparing the full
            // converted `Mesh` rather than each node's own raw `MObj`
            // fields is deliberate: a node's colour can also change because
            // an *earlier* node's state leaked into it (RE-064's
            // cross-node inheritance), not only because its own materials
            // table entry did.
            //
            // Scoped to each node's own primary mesh slot -- the rare
            // "extra leaf" shape `node_extra` covers (a joint drawn a
            // second time into another render layer) is not handled by
            // this pass; no costume-bearing graph in the archive was found
            // to need it.
            //
            // RE-425: every costume but 0 also wears the fighter's headgear
            // accessory, drawn in its joint's matrix before (Jigglypuff) or
            // after (Pikachu) the joint's own list, so it is converted in
            // that place of the walk. Its mesh is keyed
            // `accessory_costume(costume)` on the joint's node, and any node
            // after it whose state it changed gets its own override.
            let costumes = fighter_costume_count(id);
            let accessory = fighter_accessory(&loaded, id, graph);
            if costumes > 1 && loaded.tables.costumes_for(id, graph.offset).is_some() {
                let base_materials = loaded.materials(file, graph);
                let first_node = writer.object(object).unwrap().first_node;
                let plan = &plans[gi];
                let initial = initial_material_for(
                    &skeleton_graphs,
                    &ground_graphs,
                    &transition_graphs,
                    id,
                    graph.offset,
                );
                let base_converted = convert_graph_at(
                    ctx,
                    &loaded,
                    file,
                    graph.offset,
                    plan,
                    &base_materials,
                    &mut mat_anim_data,
                    initial,
                );
                let with_accessory = accessory.as_ref().and_then(|(a, before)| {
                    plan_with_accessory(plan, graph, a, *before).map(|(p, slot)| (p, slot, *a))
                });
                for costume in 1..costumes {
                    let mut materials_k = loaded.materials_at(file, graph, costume as f32);
                    let (plan_k, access_slot): (&[PlannedList], Option<usize>) =
                        match &with_accessory {
                            Some((p, slot, a)) => {
                                materials_k.push(part_materials(
                                    file,
                                    a.mobjsubs,
                                    a.costume_matanim_joints,
                                    costume,
                                ));
                                (p, Some(*slot))
                            }
                            None => (plan, None),
                        };
                    let converted_k = convert_graph_at(
                        ctx,
                        &loaded,
                        file,
                        graph.offset,
                        plan_k,
                        &materials_k,
                        &mut mat_anim_data,
                        initial,
                    );
                    let mut base_slot = 0usize;
                    for (i, (p, m_k)) in plan_k.iter().zip(&converted_k).enumerate() {
                        if Some(i) == access_slot {
                            let (Ok(m), Some(space)) = (m_k, p.space) else {
                                continue;
                            };
                            if m.triangle_count() == 0 {
                                continue;
                            }
                            let mesh = pack_mesh(
                                ctx,
                                &mut writer,
                                &mut tex_index,
                                &mut mat_anim_index,
                                &mat_anim_data,
                                Texels {
                                    home: file,
                                    all: &loaded.files,
                                },
                                id,
                                p.dl,
                                m,
                                swizzle,
                            );
                            writer.add_costume_override(
                                first_node + space as u32,
                                ssb_rom::pack::accessory_costume(costume),
                                mesh,
                            );
                            accessories_added += 1;
                            continue;
                        }
                        let base_index = base_slot;
                        base_slot += 1;
                        if !p.own_space() {
                            continue;
                        }
                        let Ok(m_k) = m_k else { continue };
                        if m_k.triangle_count() == 0 {
                            continue;
                        }
                        if base_converted.get(base_index) == Some(&Ok(m_k.clone())) {
                            continue;
                        }
                        let variant_index = pack_mesh(
                            ctx,
                            &mut writer,
                            &mut tex_index,
                            &mut mat_anim_index,
                            &mat_anim_data,
                            Texels {
                                home: file,
                                all: &loaded.files,
                            },
                            id,
                            p.dl,
                            m_k,
                            swizzle,
                        );
                        writer.add_costume_override(
                            first_node + p.node as u32,
                            costume,
                            variant_index,
                        );
                        costume_overrides_added += 1;
                    }
                }
            }

            // RE-414: the electric-damage skeletons. Each set replaces every
            // node's list with its `FTSkeleton` entry (or nothing), drawn under
            // the node's own materials (`ftDisplayMainDrawSkeleton`); the
            // converted meshes are keyed as costume overrides at
            // `SKELETON_COSTUME_BASE + set`.
            for (owner, set, parts) in skeleton_sets_for(&loaded, id, graph) {
                let plan = plan_skeleton_order(graph, &parts);
                let materials = loaded.materials(file, graph);
                let initial = initial_material_for(
                    &skeleton_graphs,
                    &ground_graphs,
                    &transition_graphs,
                    id,
                    graph.offset,
                );
                let converted = convert_graph_at(
                    ctx,
                    &loaded,
                    file,
                    graph.offset,
                    &plan,
                    &materials,
                    &mut mat_anim_data,
                    initial,
                );
                let first_node = writer.object(object).unwrap().first_node;
                for (p, m) in plan.iter().zip(&converted) {
                    let Ok(m) = m else { continue };
                    if m.triangle_count() == 0 {
                        continue;
                    }
                    if !p.own_space() {
                        skeleton_parts_dropped += 1;
                        continue;
                    }
                    let mesh = pack_mesh(
                        ctx,
                        &mut writer,
                        &mut tex_index,
                        &mut mat_anim_index,
                        &mat_anim_data,
                        Texels {
                            home: file,
                            all: &loaded.files,
                        },
                        id,
                        p.dl,
                        m,
                        swizzle,
                    );
                    writer.add_costume_override(
                        first_node + p.node as u32,
                        ssb_rom::pack::SKELETON_COSTUME_BASE + set,
                        mesh,
                    );
                    skeleton_parts_added += 1;
                }
                skeleton_fighters.insert(owner);
            }

            // RE-417, RE-425: the model parts `ftParamSetModelPartID` can
            // give a joint -- the motion scripts' hands, faces, Link's
            // shield in hand and Fox's Blaster, and Kirby's copy hats. It
            // swaps the joint's list and `MObj`s for that `FTModelPart`
            // (high detail), its colours played from the part's own
            // costume scripts. A part in the model file is converted in the
            // whole graph's order, as the base costumes are; one in another
            // file (Fox's Blaster, file 315) on its own, under the graph's
            // seed. Each is keyed `modelpart_costume(part, costume)` on the
            // joint's node, stored only where it differs from what the
            // lookup would otherwise draw (see `MODELPART_COSTUME_BASE`).
            let parts = fighter_model_parts(&loaded, id, graph);
            if !parts.is_empty() {
                let first_node = writer.object(object).unwrap().first_node;
                let initial = initial_material_for(
                    &skeleton_graphs,
                    &ground_graphs,
                    &transition_graphs,
                    id,
                    graph.offset,
                );
                let plan = &plans[gi];
                // The node's own mesh in each costume, as drawn without a
                // part (the lookup's last fallback).
                let own_by_costume: Vec<Vec<Option<ssb_rom::mesh::Mesh>>> = (0..costumes.max(1))
                    .map(|costume| {
                        let materials = loaded.materials_at(file, graph, costume as f32);
                        let converted = convert_graph_at(
                            ctx,
                            &loaded,
                            file,
                            graph.offset,
                            plan,
                            &materials,
                            &mut mat_anim_data,
                            initial,
                        );
                        let mut own = vec![None; graph.nodes.len()];
                        for (p, m) in plan.iter().zip(converted) {
                            if p.own_space() {
                                own[p.node] = m.ok().filter(|m| m.triangle_count() != 0);
                            }
                        }
                        own
                    })
                    .collect();
                for (joint, part, mp) in parts {
                    let node = (joint - ssb_rom::fighter::JOINT_COMMON_START) as usize;
                    // RE-453: `parts->flags = modelpart->flags`; the fog
                    // bits (`FTPARTS_FLAG_NOFOG`) the draw reads per joint.
                    if mp.flags & !0xF != 0 {
                        writer.add_costume_override(
                            first_node + node as u32,
                            ssb_rom::pack::modelpart_flags_key(part),
                            u32::from(mp.flags),
                        );
                        model_part_flags_added += 1;
                    }
                    let Some(part_file) = loaded.files.get(mp.dl.0 as usize) else {
                        continue;
                    };
                    let mut stored_first: Option<ssb_rom::mesh::Mesh> = None;
                    for costume in 0..costumes.max(1) {
                        let converted: Option<ssb_rom::mesh::Mesh> = if mp.dl.0 == id {
                            let (plan_p, slot) = plan_with_node_list(plan, graph, node, mp.dl.1);
                            let mut materials = loaded.materials_at(file, graph, costume as f32);
                            materials[node] = part_materials(
                                file,
                                mp.mobjsubs,
                                mp.costume_matanim_joints,
                                costume,
                            );
                            // RE-426: the part's `MObj` at the texture
                            // part's place takes the face swap too.
                            loaded.tag_texture_parts(id, graph.offset, node, &mut materials[node]);
                            convert_graph_at(
                                ctx,
                                &loaded,
                                file,
                                graph.offset,
                                &plan_p,
                                &materials,
                                &mut mat_anim_data,
                                initial,
                            )
                            .into_iter()
                            .nth(slot)
                            .and_then(Result::ok)
                        } else {
                            let cmds = part_file
                                .data
                                .get(mp.dl.1 as usize..)
                                .and_then(|d| ssb_rom::dl::decode_list_at(d, mp.dl.1).ok())
                                .unwrap_or_default();
                            let world = graph.world_transforms()[node];
                            let mobjs = part_materials(
                                &part_file,
                                mp.mobjsubs,
                                mp.costume_matanim_joints,
                                costume,
                            );
                            let single = [PlannedList {
                                node,
                                dl: mp.dl.1,
                                space: Some(node),
                                world,
                                list_id: None,
                            }];
                            ssb_rom::mesh::convert_sequence(
                                &[ssb_rom::mesh::SequenceItem {
                                    cmds: &cmds,
                                    world,
                                    mobjs: &mobjs,
                                    mat_anims: &vec![None; mobjs.len()],
                                    depth_seed: None,
                                    stream: 0,
                                }],
                                ssb_rom::mesh::Source::of(&part_file),
                                initial,
                            )
                            .into_iter()
                            .next()
                            .and_then(Result::ok)
                            .map(|mut m| {
                                map_vertex_bindings(&mut m, &single);
                                m
                            })
                        };
                        let Some(m) = converted.filter(|m| m.triangle_count() != 0) else {
                            model_parts_empty += 1;
                            continue;
                        };
                        let fallback = stored_first
                            .as_ref()
                            .or(own_by_costume[costume as usize][node].as_ref());
                        if fallback == Some(&m) {
                            continue;
                        }
                        let mesh = pack_mesh(
                            ctx,
                            &mut writer,
                            &mut tex_index,
                            &mut mat_anim_index,
                            &mat_anim_data,
                            Texels {
                                home: &part_file,
                                all: &loaded.files,
                            },
                            mp.dl.0,
                            mp.dl.1,
                            &m,
                            swizzle,
                        );
                        writer.add_costume_override(
                            first_node + node as u32,
                            ssb_rom::pack::modelpart_costume(part, costume),
                            mesh,
                        );
                        if costume == 0 {
                            stored_first = Some(m);
                        }
                        model_parts_added += 1;
                    }
                }
            }
        }
    }
    pack_log!(
        ctx,
        "  skeletons   {skeleton_parts_added} electric-skeleton part meshes for {} fighters \
         ({skeleton_parts_dropped} pre-matrix lists with triangles dropped)",
        skeleton_fighters.len()
    );
    pack_log!(
        ctx,
        "  model parts {model_parts_added} fighter model-part meshes \
         ({model_parts_empty} costumes with no triangles), {accessories_added} accessory meshes, \
         {model_part_flags_added} part flag rows"
    );
    {
        let s = ctx.texture_parts.get();
        pack_log!(
            ctx,
            "  tex parts   {} texture-part primitives, {} sprite textures \
             ({} ids unresolved, {} primitives split by the wide-tile lowering)",
            s.prims,
            s.sprites,
            s.unresolved,
            s.lowered
        );
    }
    let unlerped_swaps = {
        let s = ctx.rgb_lerp.get();
        pack_log!(
            ctx,
            "  rgb lerp    {} primitives ({} with image swaps, {} with palette swaps); \
             {} rgb-unused primitives with palette swaps",
            s.lerped,
            s.lerped_sprite_anim,
            s.lerped_palette_anim,
            s.unused_palette_anim
        );
        s.lerped_palette_anim + s.unused_palette_anim
    };
    if unlerped_swaps != 0 {
        return Err(format!(
            "{unlerped_swaps} primitives swap palettes the converter does not rewrite (RE-527)"
        )
        .into());
    }

    writer.trim_descriptors();
    ctx.log.phase("meshes");
    // RE-426: each fighter's high- and low-detail objects, including the
    // fighters that share another's graphs.
    for entry in ssb_rom::fighter::FIGHTER_FILES
        .iter()
        .filter(|e| e.kind != 12)
    {
        let Some(main) = loaded.files.get(entry.file as usize) else {
            continue;
        };
        let [Some(hi), lo] = ssb_rom::fighter::common_parts(&main, *entry) else {
            continue;
        };
        if !loaded
            .fighter_graphs
            .contains_key(&(hi.model_file, hi.graph))
        {
            continue;
        }
        let low = lo
            .and_then(|lo| object_index.get(&(lo.model_file, lo.graph)).copied())
            .unwrap_or(u32::MAX);
        if let Some(&high) = object_index.get(&(hi.model_file, hi.graph)) {
            writer.add_fighter_model(u32::from(entry.kind), high, low);
        }
    }

    // Stages last: a layer can only be resolved once every object exists.
    let mut stage_layers = 0usize;
    let mut resolved_layers = 0usize;
    let mut with_collision = 0usize;
    let (mut stage_anims, mut stage_anim_joints) = (0usize, 0usize);
    /// A stage's animation, held back until the fighter block is complete:
    /// stage index, source file, and one (script offset, node) per joint.
    type StageAnim = (u32, u32, Vec<(Option<u32>, Option<u32>)>);
    let mut deferred_stage_anims: Vec<StageAnim> = Vec::new();
    for ground in &loaded.stages {
        let map = ground.map_geometry.and_then(|(f, at)| {
            let file = loaded.files.get(f as usize)?;
            ssb_rom::collision::read(&file, at)
        });
        if map.is_some() {
            with_collision += 1;
        }
        stage_layers += ground.layers.len();
        resolved_layers += ground
            .layers
            .iter()
            .filter(|l| object_index.contains_key(&l.graph))
            .count();
        let stage_index = writer.add_stage(ground, map.as_ref(), |file, offset| {
            object_index.get(&(file, offset)).copied()
        });
        // The hazard controllers' descriptor (RE-356). Zebes also needs its
        // acid surface: `grZebesMakeAcid` builds `llGRZebesMapAcidDObjDesc`,
        // the graph `map_nodes` names, and the surface is its node 1.
        if let Some(words) = loaded
            .files
            .get(ground.file as usize)
            .filter(|_| ground.offset == 0x14)
            .and_then(|f| ssb_rom::stage::hazard_words(&f))
        {
            let mut surface_y = 0.0;
            if ssb_rom::stage::vs_ground_kind(ground.file) == Some(3) {
                surface_y = ground
                    .map_nodes
                    .and_then(|(f, at)| {
                        let graph = loaded.graphs.get(&f)?.iter().find(|g| g.offset == at)?;
                        let node = graph.nodes.get(1).filter(|n| n.parent == Some(0))?;
                        Some(node.desc.translate[1])
                    })
                    .ok_or("Zebes: no acid graph at map_nodes")?;
            }
            writer.set_stage_hazard(stage_index, words, surface_y);
        }

        // Stage scenery animates through the 32-bit event stream (RE-050).
        // Each layer names one script per graph node, so the joint entries are
        // (script offset in the file, absolute node index in the pack) — the
        // same shape a fighter's are, which is why this reuses the animation
        // tables rather than adding a parallel set.
        let mut joints: Vec<(Option<u32>, Option<u32>)> = Vec::new();
        let mut anim_file = None;
        for layer in &ground.layers {
            let Some((af, at)) = layer.anim_joints else {
                continue;
            };
            // The scripts and the graph have to be in one file: a joint entry
            // holds an offset, and the blob it indexes is that one file's.
            if af != layer.graph.0 || anim_file.is_some_and(|f| f != af) {
                continue;
            }
            let Some(file) = loaded.files.get(af as usize) else {
                continue;
            };
            let Some(&object) = object_index.get(&layer.graph) else {
                continue;
            };
            let Some(first_node) = writer.object_first_node(object) else {
                continue;
            };
            // The *graph's* node count, not the object's. An object also holds
            // the extra leaf nodes the packer adds for lists a node could not
            // carry, and reading the `anim_joints` table that far walks past
            // its end into whatever follows — which yields script offsets that
            // are not offsets (RE-052).
            let nodes = loaded
                .graphs
                .get(&layer.graph.0)
                .and_then(|gs| gs.iter().find(|g| g.offset == layer.graph.1))
                .map_or(0, |g| g.nodes.len());
            for (i, script) in ssb_rom::objanim::joint_scripts(&file.data, at, nodes)
                .into_iter()
                .enumerate()
            {
                let Some(script) = script else { continue };
                joints.push((Some(script), Some(first_node + i as u32)));
            }
            anim_file = Some(af);
        }
        // Deferred, not written here. `Pack::fighter_anim` finds a fighter's
        // animation by arithmetic — `anim(fighter * SLOT_COUNT + slot)` — which
        // only holds while the fighter entries are a dense block starting at
        // index 0. Stages are packed before fighters, so writing these now puts
        // 35 stage entries in front of that block and every fighter animation
        // resolves to the wrong row (RE-051).
        if let (Some(af), false) = (anim_file, joints.is_empty()) {
            deferred_stage_anims.push((stage_index, af, joints));
        }
    }

    ctx.log.phase("objects+stages");
    // Fighters: 27 small reads, in `FTKind` order so the table can be indexed
    // by kind. A character whose attributes will not decode is skipped rather
    // than packed as zeros, which would look like a fighter with no gravity.
    //
    // The animation lengths come from a second set of files entirely, so the
    // two decodes are zipped here; a fighter is packed only if both worked,
    // for the same reason.
    let mut fighters = 0usize;
    let mut fighters_failed: Vec<&str> = Vec::new();
    let anims = ssb_rom::anim::decode_all(&archive);
    for (f, a) in ssb_rom::fighter::decode_all(&archive)
        .into_iter()
        .zip(anims)
    {
        match (f, a) {
            (Ok(f), Ok(a)) if f.attributes.looks_plausible() => {
                writer.add_fighter(&f, &a);
                fighters += 1;
            }
            (Ok(f), _) => fighters_failed.push(f.file.name),
            (Err(_), _) => fighters_failed.push("?"),
        }
    }

    // Animations. Each one needs three things resolved that only exist at
    // build time: which object is the fighter's skeleton, which of that
    // object's nodes are joints, and where each joint's script starts. See
    // RE-036 -- the mask and the container record are both read here so the
    // runtime never has to.
    let mut packed_anims = 0usize;
    let mut anim_joints_packed = 0usize;
    let mut anims_failed: Vec<String> = Vec::new();
    let mut hidden_joint_anims = 0usize;
    let mut walked_anims = 0usize;
    let mut deferred_shield_poses: Vec<(
        u32,
        ssb_rom::fighter::ShieldPoseRefs,
        Vec<Vec<(Option<u32>, Option<u32>)>>,
    )> = Vec::new();
    for (kind, entry) in ssb_rom::anim::FIGHTER_ANIMS.iter().enumerate() {
        let file_entry = ssb_rom::fighter::FIGHTER_FILES[kind];
        let nodes = loaded.files.get(file_entry.file as usize).and_then(|main| {
            let part = ssb_rom::fighter::common_parts(&main, file_entry)[0]?;
            let mask = ssb_rom::fighter::setup_parts(&main, file_entry)?;
            let first = object_index.get(&(part.model_file, part.graph))?;
            let object = writer.object(*first)?;
            // The nth animation script drives the nth *set* mask bit.
            Some(
                (0..object.node_count)
                    .filter(|i| mask >> i & 1 != 0)
                    .map(|i| object.first_node + i)
                    .collect::<Vec<u32>>(),
            )
        });
        let Some(nodes) = nodes else {
            anims_failed.push(format!("{}: no skeleton", entry.name));
            continue;
        };
        // Motions that insert hidden model parts bind by the original tree
        // walk (RE-432): each needs every descriptor's depth and the parts.
        let walk = loaded.files.get(file_entry.file as usize).and_then(|main| {
            let common = ssb_rom::fighter::common_parts(&main, file_entry)[0]?;
            let mask = ssb_rom::fighter::setup_parts(&main, file_entry)?;
            let object = writer.object(*object_index.get(&(common.model_file, common.graph))?)?;
            let graph = loaded
                .graphs
                .get(&common.model_file)?
                .iter()
                .find(|g| g.offset == common.graph)?;
            let depths: Vec<u32> = graph.nodes.iter().map(|n| n.desc.depth()).collect();
            let used = ssb_rom::anim::HIDDEN_PARTS[kind]
                .iter()
                .fold(0u32, |m, &bits| m | bits);
            let count = (0..27)
                .filter(|i| used >> (31 - i) & 1 != 0)
                .max()
                .map_or(0, |i| i + 1);
            let hidden = (0..count)
                .map(|i| ssb_rom::fighter::hidden_part(&main, file_entry, i))
                .collect::<Option<Vec<_>>>()?;
            Some((depths, mask, hidden, object.first_node))
        });

        for (slot, &id) in entry.files.iter().enumerate() {
            if id == 0 {
                continue; // a move the fighter does not have
            }
            let Ok(file) = archive.load(id as u32) else {
                anims_failed.push(format!(
                    "{}.{}",
                    entry.name,
                    ssb_rom::anim::SLOT_NAMES[slot]
                ));
                continue;
            };
            let Some(table) = joint_table(&file.data) else {
                anims_failed.push(format!(
                    "{}.{}",
                    entry.name,
                    ssb_rom::anim::SLOT_NAMES[slot]
                ));
                continue;
            };
            // The battle-entry slots are 32-bit `AnimJoint` clips (RE-401):
            // their table leads with the runtime joints, then walks the
            // model joints, and their
            // length is the tick every joint's stream ends on, less one.
            if ssb_rom::anim::is_anim_joint_slot(slot) {
                let ends: Vec<u32> = table
                    .iter()
                    .filter(|&&at| at != 0)
                    .filter_map(|&at| ssb_rom::objanim::script_end_tick(&file.data, at, 4096))
                    .collect();
                let (Some(&first), true) = (ends.first(), table.len() > nodes.len()) else {
                    anims_failed.push(format!(
                        "{}.{}: AnimJoint",
                        entry.name,
                        ssb_rom::anim::SLOT_NAMES[slot]
                    ));
                    continue;
                };
                if ends.iter().any(|&e| e != first) {
                    anims_failed.push(format!(
                        "{}.{}: uneven ends",
                        entry.name,
                        ssb_rom::anim::SLOT_NAMES[slot]
                    ));
                    continue;
                }
                // One leading runtime joint, or as many as the motion's
                // descriptor names: Link's are `TransN` then `XRotN`
                // (`ftdata.c:3354`), so his model joints start at entry 2.
                let leads = ((ssb_rom::anim::HIDDEN_PARTS[kind][slot] & 0xE000_0000).count_ones()
                    as usize)
                    .max(1);
                let joints: Vec<(Option<u32>, Option<u32>)> = table
                    .iter()
                    .enumerate()
                    .map(|(j, &at)| {
                        (
                            (at != 0).then_some(at),
                            j.checked_sub(leads).and_then(|k| nodes.get(k).copied()),
                        )
                    })
                    .collect();
                anim_joints_packed += joints.len();
                writer.add_anim(
                    kind as u32,
                    slot as u32,
                    id as u32,
                    first - 1,
                    &file.data,
                    &joints,
                );
                packed_anims += 1;
                continue;
            }
            let frames = ssb_rom::anim::decode_length(id as u32, &file)
                .ok()
                .and_then(|l| l.frames())
                .unwrap_or(0);
            // The motion descriptor, not the table length alone, identifies
            // a leading runtime joint (`TransN`, `XRotN`, or `YRotN`). A
            // disabled trailing model part can also leave one more script
            // than active joints; treating that as a leading joint shifts
            // every model transform and sinks the visible fighter (RE-331).
            // Runtime motions can also include trailing unused entries (Kirby's
            // cliff escape), so the descriptor remains authoritative beyond +1.
            let hidden_joint =
                table.len() > nodes.len() && ssb_rom::anim::LEADING_RUNTIME_JOINT[kind][slot];
            // Hidden *model* parts (bits 28..5) change the walk, and so do
            // two leading runtime joints (Link's entry: `TransN` then
            // `XRotN`, `ftdata.c:3354`), which the one-joint shift above
            // would misplace by one. A single leading joint keeps the shift.
            let parts = ssb_rom::anim::HIDDEN_PARTS[kind][slot];
            let walked = parts & 0x1FFF_FFE0 != 0 || (parts & 0xE000_0000).count_ones() > 1;
            let order = walked
                .then(|| {
                    walk.as_ref().and_then(|(depths, mask, hidden, first)| {
                        let order = ssb_rom::fighter::figatree_order(depths, *mask, hidden, parts)?;
                        (table.len() >= order.len()).then(|| {
                            order
                                .iter()
                                .map(|j| match *j {
                                    ssb_rom::fighter::TreeJoint::Desc(d) => Some(first + d),
                                    ssb_rom::fighter::TreeJoint::Runtime(_) => None,
                                })
                                .collect::<Vec<_>>()
                        })
                    })
                })
                .flatten();
            if walked {
                match &order {
                    Some(_) => walked_anims += 1,
                    None => anims_failed.push(format!(
                        "{}.{}: hidden-part walk",
                        entry.name,
                        ssb_rom::anim::SLOT_NAMES[slot]
                    )),
                }
            }
            let joints: Vec<(Option<u32>, Option<u32>)> = table
                .iter()
                .enumerate()
                .map(|(j, &at)| {
                    let node = if let Some(order) = &order {
                        order.get(j).copied().flatten()
                    } else if hidden_joint {
                        j.checked_sub(1).and_then(|k| nodes.get(k).copied())
                    } else {
                        nodes.get(j).copied()
                    };
                    ((at != 0).then_some(at), node)
                })
                .collect();
            if hidden_joint {
                hidden_joint_anims += 1;
            }
            anim_joints_packed += joints.len();
            writer.add_anim(
                kind as u32,
                slot as u32,
                id as u32,
                frames as u32,
                &file.data,
                &joints,
            );
            packed_anims += 1;
        }

        // The shield poses (RE-367). Each sector table walks the tree from
        // `XRotN`: `XRotN`, then every model joint, then `YRotN`. The
        // `dobj_lookup` array has one `DObjDesc` per entry and ends right
        // after `YRotN`, which checks that count.
        if let Some(refs) = loaded
            .files
            .get(file_entry.file as usize)
            .and_then(|main| ssb_rom::fighter::shield_pose_refs(&main, file_entry))
        {
            match shield_pose_joints(&loaded, refs, &nodes) {
                Ok(tables) => deferred_shield_poses.push((kind as u32, refs, tables)),
                Err(e) => anims_failed.push(format!("{}.ShieldPose: {e}", entry.name)),
            }
        }
    }

    // Now that the fighter block is complete and dense, the stage entries can
    // go after it without disturbing `fighter_anim`'s indexing.
    for (stage_index, af, joints) in deferred_stage_anims {
        let Some(file) = loaded.files.get(af as usize) else {
            continue;
        };
        writer.add_anim(
            ssb_rom::pack::AnimDesc::STAGE,
            stage_index,
            af,
            0,
            &file.data,
            &joints,
        );
        stage_anims += 1;
        stage_anim_joints += joints.len();
    }

    // Results-screen wipes use the same 32-bit DObj event stream as stage
    // scenery. Append them after both existing animation classes so fighter's
    // dense arithmetic index remains unchanged (RE-146).
    let mut transition_anim_joints = 0usize;
    for (transition_index, asset) in ssb_rom::transition::ASSETS.iter().enumerate() {
        let file = loaded
            .files
            .get(asset.file as usize)
            .ok_or_else(|| format!("transition {}: file {} missing", asset.name, asset.file))?;
        let graph = loaded
            .graphs
            .get(&asset.file)
            .and_then(|graphs| graphs.iter().find(|graph| graph.offset == asset.graph))
            .ok_or_else(|| {
                format!(
                    "transition {}: graph 0x{:X} missing",
                    asset.name, asset.graph
                )
            })?;
        let object = object_index
            .get(&(asset.file, asset.graph))
            .and_then(|&index| writer.object(index))
            .ok_or_else(|| format!("transition {}: packed object missing", asset.name))?;
        let joints: Vec<_> =
            ssb_rom::objanim::joint_scripts(&file.data, asset.anim_joints, graph.nodes.len())
                .into_iter()
                .enumerate()
                .filter_map(|(node, script)| {
                    script.map(|script| (Some(script), Some(object.first_node + node as u32)))
                })
                .collect();
        if joints.is_empty() {
            return Err(format!("transition {}: no animation scripts", asset.name).into());
        }
        transition_anim_joints += joints.len();
        writer.add_anim(
            ssb_rom::pack::AnimDesc::TRANSITION,
            transition_index as u32,
            asset.file,
            asset.frames,
            &file.data,
            &joints,
        );
    }

    // Manager-effect DObjs use the same AObjEvent32 transform stream as stage
    // scenery and results transitions. Pack the 35 source descriptors that
    // actually name an o_anim_joint table, keyed by the shared 46-effect
    // inventory slot so runtime selection cannot depend on pack object order.
    let mut effect_anims = 0usize;
    let mut effect_anim_joints = 0usize;
    for (effect_index, (&(file_id, graph_at), &anim_at)) in ssb_rom::effect::MANAGER_EFFECT_KEYS
        .iter()
        .zip(ssb_rom::effect::MANAGER_EFFECT_ANIM_JOINTS.iter())
        .enumerate()
    {
        let Some(anim_at) = anim_at else { continue };
        let file = loaded
            .files
            .get(file_id as usize)
            .ok_or_else(|| format!("effect {effect_index}: file {file_id} missing"))?;
        let graph = loaded
            .graphs
            .get(&file_id)
            .and_then(|graphs| graphs.iter().find(|graph| graph.offset == graph_at))
            .ok_or_else(|| format!("effect {effect_index}: graph 0x{graph_at:X} missing"))?;
        let object = object_index
            .get(&(file_id, graph_at))
            .and_then(|&index| writer.object(index))
            .ok_or_else(|| format!("effect {effect_index}: packed object missing"))?;
        let joints: Vec<_> =
            ssb_rom::objanim::joint_scripts(&file.data, anim_at, graph.nodes.len())
                .into_iter()
                .enumerate()
                .filter_map(|(node, script)| {
                    script.map(|script| (Some(script), Some(object.first_node + node as u32)))
                })
                .collect();
        if joints.is_empty() {
            return Err(format!("effect {effect_index}: no animation scripts").into());
        }
        effect_anim_joints += joints.len();
        effect_anims += 1;
        writer.add_anim(
            ssb_rom::pack::AnimDesc::EFFECT,
            effect_index as u32,
            file_id,
            0,
            &file.data,
            &joints,
        );
    }

    // The quakes' single DObj (`gcAddDObjForGObj(effect_gobj, NULL)`) has no
    // graph: each packs its one script with no node (RE-416).
    {
        let file_id = ssb_rom::effect::QUAKE_ANIM_FILE;
        let file = loaded
            .files
            .get(file_id as usize)
            .ok_or_else(|| format!("quake: file {file_id} missing"))?;
        for (m, &table) in ssb_rom::effect::QUAKE_ANIM_JOINTS.iter().enumerate() {
            let script = ssb_rom::objanim::joint_scripts(&file.data, table, 1)
                .first()
                .copied()
                .flatten()
                .ok_or_else(|| format!("quake {m}: no script at 0x{table:X}"))?;
            writer.add_anim(
                ssb_rom::pack::AnimDesc::EFFECT,
                ssb_rom::effect::QUAKE_ANIM_SLOT + m as u32,
                file_id,
                0,
                &file.data,
                &[(Some(script), None)],
            );
            effect_anims += 1;
        }
    }

    // The tables makers add in place of an `EFDesc`'s own
    // (`ssb_rom::effect::VARIANT_ANIMS`): the Egg Lay egg's Wait and Break
    // (`dEFManagerYoshiEggLayAnimJoints`, RE-417), the Thunder Shock
    // spark's animations 1 and 2, and the Reflector hexagon's Loop, Hit and
    // End.
    for &(first_slot, (file_id, graph_at), tables) in &ssb_rom::effect::VARIANT_ANIMS {
        let file = loaded
            .files
            .get(file_id as usize)
            .ok_or_else(|| format!("variant anim {first_slot:#X}: file {file_id} missing"))?;
        let graph = loaded
            .graphs
            .get(&file_id)
            .and_then(|graphs| graphs.iter().find(|graph| graph.offset == graph_at))
            .ok_or_else(|| format!("variant anim {first_slot:#X}: graph missing"))?;
        let object = object_index
            .get(&(file_id, graph_at))
            .and_then(|&index| writer.object(index))
            .ok_or_else(|| format!("variant anim {first_slot:#X}: packed object missing"))?;
        for (i, &table) in tables.iter().enumerate() {
            let joints: Vec<_> =
                ssb_rom::objanim::joint_scripts(&file.data, table, graph.nodes.len())
                    .into_iter()
                    .enumerate()
                    .filter_map(|(node, script)| {
                        script.map(|script| (Some(script), Some(object.first_node + node as u32)))
                    })
                    .collect();
            if joints.is_empty() {
                return Err(format!(
                    "variant anim {first_slot:#X} {i}: no animation scripts at 0x{table:X}"
                )
                .into());
            }
            effect_anim_joints += joints.len();
            effect_anims += 1;
            writer.add_anim(
                ssb_rom::pack::AnimDesc::EFFECT,
                first_slot + i as u32,
                file_id,
                0,
                &file.data,
                &joints,
            );
        }
    }

    // The ground effects' `o_anim_joint` tables (`efGroundMakeEffect`:
    // `lbCommonAddTreeDObjsAnimAll` on the root's child, the DObj array's
    // first node), one slot per desc: descs share graphs but not tables.
    for desc in 0..ssb_game::ground_effect::DESC_COUNT {
        let table = ssb_game::ground_effect::O_ANIM_JOINT[desc];
        let Some(file_id) = ssb_game::ground_effect::desc_file(desc) else {
            continue;
        };
        if table == 0 {
            continue;
        }
        let graph_at = ssb_game::ground_effect::O_DOBJSETUP[desc];
        let slot = ssb_rom::effect::GROUND_EFFECT_ANIM_SLOT + desc as u32;
        let file = loaded
            .files
            .get(file_id as usize)
            .ok_or_else(|| format!("ground effect {desc}: file {file_id} missing"))?;
        let graph = loaded
            .graphs
            .get(&file_id)
            .and_then(|graphs| graphs.iter().find(|graph| graph.offset == graph_at))
            .ok_or_else(|| format!("ground effect {desc}: graph missing"))?;
        let object = object_index
            .get(&(file_id, graph_at))
            .and_then(|&index| writer.object(index))
            .ok_or_else(|| format!("ground effect {desc}: packed object missing"))?;
        let joints: Vec<_> = ssb_rom::objanim::joint_scripts(&file.data, table, graph.nodes.len())
            .into_iter()
            .enumerate()
            .filter_map(|(node, script)| {
                script.map(|script| (Some(script), Some(object.first_node + node as u32)))
            })
            .collect();
        if joints.is_empty() {
            return Err(
                format!("ground effect {desc}: no animation scripts at 0x{table:X}").into(),
            );
        }
        effect_anim_joints += joints.len();
        effect_anims += 1;
        writer.add_anim(
            ssb_rom::pack::AnimDesc::EFFECT,
            slot,
            file_id,
            0,
            &file.data,
            &joints,
        );
    }

    // RE-425: the entry makers' own tables and single scripts
    // (`ssb_rom::effect::ENTRY_ANIMS`), on their trees' packed nodes. The
    // Arwing's tables live in file 346 and drive file 161's tree.
    for &(slot, (file_id, graph_at), anim_file, table, singles) in &ssb_rom::effect::ENTRY_ANIMS {
        let file = loaded
            .files
            .get(anim_file as usize)
            .ok_or_else(|| format!("entry anim {slot:#X}: file {anim_file} missing"))?;
        let graph = loaded
            .graphs
            .get(&file_id)
            .and_then(|graphs| graphs.iter().find(|graph| graph.offset == graph_at))
            .ok_or_else(|| format!("entry anim {slot:#X}: graph missing"))?;
        let object = object_index
            .get(&(file_id, graph_at))
            .and_then(|&index| writer.object(index))
            .ok_or_else(|| format!("entry anim {slot:#X}: packed object missing"))?;
        let mut joints: Vec<_> = table
            .map(|table| ssb_rom::objanim::joint_scripts(&file.data, table, graph.nodes.len()))
            .unwrap_or_default()
            .into_iter()
            .enumerate()
            .filter_map(|(node, script)| {
                script.map(|script| (Some(script), Some(object.first_node + node as u32)))
            })
            .collect();
        joints.extend(
            singles
                .iter()
                .map(|&(script, node)| (Some(script), Some(object.first_node + node))),
        );
        if joints.is_empty() {
            return Err(format!("entry anim {slot:#X}: no animation scripts").into());
        }
        effect_anim_joints += joints.len();
        effect_anims += 1;
        writer.add_anim(
            ssb_rom::pack::AnimDesc::EFFECT,
            slot,
            anim_file,
            0,
            &file.data,
            &joints,
        );
    }

    // Weapon DObj trees animate through `WPAttributes.anim_joints`
    // (`wpManagerMakeWeapon` -> `gcAddAnimAll`). Link's Boomerang
    // (`226_LinkSpecial1.c`) names file 325's graph at 0x610 and its table
    // at 0x6C0, whose one script spins node 1 about X.
    // Kirby's Final Cutter wave (`llKirbyMainCutterWeaponAttributes`, file
    // 229 + 0x08) names file 328's graph at 0x1D388 and its table at
    // 0x1D410, whose one script flips node 1's `RotZ` and `ScaZ` each frame.
    // File 328 is Kirby's whole model, so only the table and its script
    // (0x1D410..0x1D4B8) are packed, rebased to the window's start.
    // `(slot, file, graph, anim_joints, script window end)`; `None` packs
    // the whole file.
    const WEAPON_ANIMS: &[(u32, u32, u32, u32, u32, Option<u32>)] = &[
        (
            ssb_rom::pack::AnimDesc::WEAPON,
            ssb_rom::pack::AnimDesc::WEAPON_ANIM_LINK_BOOMERANG,
            325,
            0x610,
            0x6C0,
            None,
        ),
        (
            ssb_rom::pack::AnimDesc::WEAPON,
            ssb_rom::pack::AnimDesc::WEAPON_ANIM_KIRBY_CUTTER,
            328,
            0x1D388,
            0x1D410,
            Some(0x1D4B8),
        ),
        // Pikachu's Thunder Jolt (RE-379): the aerial list's one script
        // spins and pulses it (342 + 0x360..0x3F8); the ground tree's table
        // sweeps node 1 and blinks nodes 2-7 (342 + 0x1A20..0x1AE0).
        (
            ssb_rom::pack::AnimDesc::WEAPON,
            ssb_rom::pack::AnimDesc::WEAPON_ANIM_PIKACHU_JOLT_AIR,
            342,
            0x270,
            0x360,
            Some(0x3F8),
        ),
        (
            ssb_rom::pack::AnimDesc::WEAPON,
            ssb_rom::pack::AnimDesc::WEAPON_ANIM_PIKACHU_JOLT_GROUND,
            342,
            0x1888,
            0x1A20,
            Some(0x1AE0),
        ),
        // Ness's PK Thunder head (RE-381): the root's scale pulse
        // (335 + 0x7D20..0x7D70).
        (
            ssb_rom::pack::AnimDesc::WEAPON,
            ssb_rom::pack::AnimDesc::WEAPON_ANIM_NESS_PK_THUNDER,
            335,
            0x7C98,
            0x7D20,
            Some(0x7D70),
        ),
        // Items animate through `ITAttributes.anim_joints` the same way
        // (`itManagerMakeItem` -> `gcAddAnimAll`), RE-382: the PK Fire flame
        // (file 240 + 0x34) names file 336's tree at 0xA08 and table at
        // 0xAF0; Link's Bomb (`llLinkMainBombItemAttributes`, file 225 +
        // 0x40) file 353's tree at 0x18D8 and table at 0x1990 (RE-383).
        (
            ssb_rom::pack::AnimDesc::ITEM,
            ssb_rom::pack::AnimDesc::ITEM_ANIM_NESS_PK_FIRE,
            336,
            0xA08,
            0xAF0,
            None,
        ),
        (
            ssb_rom::pack::AnimDesc::ITEM,
            ssb_rom::pack::AnimDesc::ITEM_ANIM_LINK_BOMB,
            353,
            0x18D8,
            0x1990,
            None,
        ),
        // Normal Egg: file 251 + 0xACC names file 86's three-node
        // tree and its node-2 scale animation.
        (
            ssb_rom::pack::AnimDesc::ITEM,
            ssb_rom::pack::AnimDesc::ITEM_ANIM_EGG,
            86,
            0x104A0,
            0x10550,
            None,
        ),
        // The Poké Ball Pokémon's weapons (RE-435): the Hydro Pump's reach
        // and the Smog's swell, which gameplay keeps as tables.
        (
            ssb_rom::pack::AnimDesc::WEAPON,
            ssb_rom::pack::AnimDesc::WEAPON_ANIM_KAMEX_HYDRO,
            ssb_rom::mmonster::FILE,
            ssb_rom::mmonster::HYDRO_GRAPH,
            ssb_rom::mmonster::HYDRO_ANIM_JOINTS,
            None,
        ),
        (
            ssb_rom::pack::AnimDesc::WEAPON,
            ssb_rom::pack::AnimDesc::WEAPON_ANIM_DOGAS_SMOG,
            ssb_rom::mmonster::FILE,
            ssb_rom::mmonster::SMOG_GRAPH,
            ssb_rom::mmonster::SMOG_ANIM_JOINTS,
            None,
        ),
        // Link's Spin Attack weapon graphic
        // (`llLinkMainSpinAttackWeaponAttributes`, file 225 + 0x0C): node
        // 1's spin and swell (324 + 0x11990..0x11A40). File 324 is Link's
        // whole model, so only the table and its script are packed.
        (
            ssb_rom::pack::AnimDesc::WEAPON,
            ssb_rom::pack::AnimDesc::WEAPON_ANIM_LINK_SPIN_ATTACK,
            324,
            0x11908,
            0x11990,
            Some(0x11A40),
        ),
    ];
    let mut weapon_anims = 0usize;
    let mut weapon_anim_rebased = 0usize;
    for &(stream, slot, file_id, graph_at, anim_at, window_end) in WEAPON_ANIMS {
        let file = loaded
            .files
            .get(file_id as usize)
            .ok_or_else(|| format!("weapon anim {slot}: file {file_id} missing"))?;
        let graph = loaded
            .graphs
            .get(&file_id)
            .and_then(|graphs| graphs.iter().find(|graph| graph.offset == graph_at))
            .ok_or_else(|| format!("weapon anim {slot}: graph 0x{graph_at:X} missing"))?;
        let object = object_index
            .get(&(file_id, graph_at))
            .and_then(|&index| writer.object(index))
            .ok_or_else(|| format!("weapon anim {slot}: packed object missing"))?;
        // The script window and the offset its pointers are rebased by. Every
        // internal pointer slot inside the window must target the window.
        let (base, blob) = match window_end {
            None => (0, file.data.clone()),
            Some(end) => {
                let mut blob = file
                    .data
                    .get(anim_at as usize..end as usize)
                    .ok_or_else(|| format!("weapon anim {slot}: window past file end"))?
                    .to_vec();
                for reloc in file
                    .intern_relocs
                    .iter()
                    .filter(|r| (anim_at..end).contains(&r.at))
                {
                    if !(anim_at..end).contains(&reloc.target) {
                        return Err(format!(
                            "weapon anim {slot}: pointer at 0x{:X} leaves the window",
                            reloc.at
                        )
                        .into());
                    }
                    let at = (reloc.at - anim_at) as usize;
                    blob[at..at + 4].copy_from_slice(&(reloc.target - anim_at).to_be_bytes());
                    weapon_anim_rebased += 1;
                }
                (anim_at, blob)
            }
        };
        let joints: Vec<_> =
            ssb_rom::objanim::joint_scripts(&blob, anim_at - base, graph.nodes.len())
                .into_iter()
                .enumerate()
                .filter_map(|(node, script)| {
                    script.map(|script| (Some(script), Some(object.first_node + node as u32)))
                })
                .collect();
        if joints.is_empty() {
            return Err(format!("weapon anim {slot}: no animation scripts").into());
        }
        weapon_anims += 1;
        writer.add_anim(stream, slot, file_id, 0, &blob, &joints);
    }

    // The Poké Ball Pokémon (RE-435): each maker's appear script and each
    // status's own script, single scripts on one node of the kind's tree.
    let mut monster_anims = 0usize;
    {
        let file_id = ssb_rom::mmonster::FILE;
        let file = loaded
            .files
            .get(file_id as usize)
            .ok_or("monster anims: file 86 missing")?;
        for (kind, visual) in ssb_rom::mmonster::VISUALS.iter().enumerate() {
            let object = object_index
                .get(&(file_id, visual.graph))
                .and_then(|&index| writer.object(index))
                .ok_or_else(|| format!("monster {kind}: packed object missing"))?;
            let mut add = |slot: u32, script: u32, node: u32| -> PackResult {
                if node >= object.node_count {
                    return Err(format!("monster {kind}: node {node} past its tree").into());
                }
                writer.add_anim(
                    ssb_rom::pack::AnimDesc::ITEM,
                    slot + kind as u32,
                    file_id,
                    0,
                    &file.data,
                    &[(Some(script), Some(object.first_node + node))],
                );
                monster_anims += 1;
                Ok(())
            };
            add(
                ssb_rom::pack::AnimDesc::ITEM_ANIM_MMONSTER_APPEAR,
                ssb_rom::mmonster::APPEAR_SCRIPT,
                visual.appear_node,
            )?;
            if let Some((script, node)) = visual.status {
                add(
                    ssb_rom::pack::AnimDesc::ITEM_ANIM_MMONSTER_STATUS,
                    script,
                    node,
                )?;
            }
        }
    }

    // The Shells' slide spin (RE-442): `it{G,R}ShellSpinAddAnim` adds
    // `llITCommonDataShellAnimJoint` (86 + 0x6018) to the root, node 1 of
    // the tree at 0x5F88 once descriptor 0 is ejected.
    {
        const SHELL_TREE: u32 = 0x5F88;
        const SHELL_SPIN_SCRIPT: u32 = 0x6018;
        let file = loaded.files.get(86).ok_or("shell spin: file 86 missing")?;
        let object = object_index
            .get(&(86, SHELL_TREE))
            .and_then(|&index| writer.object(index))
            .ok_or("shell spin: packed object missing")?;
        if object.node_count != 2 {
            return Err(format!("shell spin: tree has {} nodes", object.node_count).into());
        }
        writer.add_anim(
            ssb_rom::pack::AnimDesc::ITEM,
            ssb_rom::pack::AnimDesc::ITEM_ANIM_SHELL_SPIN,
            86,
            0,
            &file.data,
            &[(Some(SHELL_SPIN_SCRIPT), Some(object.first_node + 1))],
        );
    }

    // `sc1PBonusStageMakeTargets`: each course script animates the shared
    // item model, not its placement descriptor's display list.
    let target_model = object_index
        .get(&ssb_rom::bonus1::MODEL)
        .and_then(|&i| writer.object(i))
        .ok_or("target model missing")?;
    for (kind, course) in ssb_rom::bonus1::COURSES.iter().enumerate() {
        let ground = loaded
            .stages
            .iter()
            .find(|g| g.file == 271 + kind as u32)
            .ok_or("target course missing")?;
        let layer = ground
            .layers
            .iter()
            .find(|l| l.index == 1)
            .ok_or("target course layer 1 missing")?;
        if layer.graph != (124 + kind as u32, course.start) {
            return Err("target course base differs from US labels".into());
        }
        let file = loaded
            .files
            .get(124 + kind)
            .ok_or("target course data missing")?;
        let scripts = ssb_rom::objanim::joint_scripts(&file.data, course.scripts, 11);
        for (instance, script) in scripts.into_iter().skip(1).enumerate() {
            if let Some(script) = script {
                writer.add_anim(
                    ssb_rom::pack::AnimDesc::ITEM,
                    ssb_rom::bonus1::anim(kind as u8, instance as u8),
                    file.id,
                    0,
                    &file.data,
                    &[(
                        Some(script),
                        Some(target_model.first_node + ssb_rom::ground_obj::ITEM_ROOT as u32),
                    )],
                );
            }
        }
    }

    // Platform children keep independent clocks; their parent's course
    // animation continues to move the collision group after boarding.
    let file = loaded
        .files
        .get(ssb_rom::bonus2::FILE as usize)
        .ok_or("platform file missing")?;
    for (i, tree) in ssb_rom::bonus2::TREES.iter().enumerate() {
        let object = object_index
            .get(&(file.id, tree[0]))
            .and_then(|&i| writer.object(i))
            .ok_or("platform tree missing")?;
        let scripts =
            ssb_rom::objanim::joint_scripts(&file.data, tree[1], object.node_count as usize);
        let joints: Vec<_> = scripts
            .into_iter()
            .enumerate()
            .map(|(i, s)| (s, Some(object.first_node + i as u32)))
            .collect();
        writer.add_anim(
            ssb_rom::pack::AnimDesc::ITEM,
            ssb_rom::bonus2::FIRST_ANIM + i as u32,
            file.id,
            0,
            &file.data,
            &joints,
        );
    }
    let bumper = object_index
        .get(&ssb_rom::ground_obj::GBUMPER_SOURCE)
        .and_then(|&i| writer.object(i))
        .ok_or("Bumper tree missing")?;
    for (kind, labels) in ssb_rom::bonus2::BUMPERS.iter().enumerate() {
        let Some((graph, table)) = labels else {
            continue;
        };
        let file = loaded
            .files
            .get(137 + kind)
            .ok_or("Bumper course missing")?;
        let placements = object_index
            .get(&(file.id, *graph))
            .and_then(|&i| writer.object(i))
            .ok_or("Bumper placements missing")?;
        for (i, script) in
            ssb_rom::objanim::joint_scripts(&file.data, *table, placements.node_count as usize)
                .into_iter()
                .skip(1)
                .enumerate()
        {
            if let Some(script) = script {
                writer.add_anim(
                    ssb_rom::pack::AnimDesc::ITEM,
                    ssb_rom::bonus2::BUMPER_ANIM + kind as u32 * 10 + i as u32,
                    file.id,
                    0,
                    &file.data,
                    &[(Some(script), Some(bumper.first_node + 1))],
                );
            }
        }
    }

    // Stage controller objects (RE-357): the GObjs a `gr*.c` controller
    // makes from `MPGroundData::map_nodes` and animates itself. Each label
    // is an offset into the file `map_nodes` lands in, and only once
    // `map_nodes` is confirmed to point at the label the controller
    // subtracts. Keyed by `ground_obj::ANIMS` index.
    let mut ground_anims = 0usize;
    let mut ground_anim_joints = 0usize;
    for (anim_index, anim) in ssb_rom::ground_obj::ANIMS.iter().enumerate() {
        let asset = &ssb_rom::ground_obj::OBJECTS[anim.object as usize];
        let ground = loaded
            .stages
            .iter()
            .find(|g| g.file == asset.gr_file)
            .ok_or_else(|| format!("ground {}: no header in file {}", anim.name, asset.gr_file))?;
        let (file_id, head) = ground
            .map_nodes
            .ok_or_else(|| format!("ground {}: no map_nodes", anim.name))?;
        let graph_file_id = asset.source_file.unwrap_or(file_id);
        // Race's Bumpers use ITCommonData's tree, but the joint scripts
        // live under GRBonus3Map.map_nodes in file 162.
        let file_id = if asset.gr_file == ssb_rom::ground_obj::BONUS3_FILE {
            file_id
        } else {
            graph_file_id
        };
        if head != asset.map_head {
            return Err(format!(
                "ground {}: map_nodes at 0x{head:X}, controller subtracts 0x{:X}",
                anim.name, asset.map_head
            )
            .into());
        }
        let file = loaded
            .files
            .get(file_id as usize)
            .ok_or_else(|| format!("ground {}: file {file_id} missing", anim.name))?;
        let graph = loaded
            .graphs
            .get(&graph_file_id)
            .and_then(|graphs| graphs.iter().find(|graph| graph.offset == asset.graph))
            .ok_or_else(|| format!("ground {}: graph 0x{:X} missing", asset.name, asset.graph))?;
        let object = object_index
            .get(&(graph_file_id, asset.graph))
            .and_then(|&index| writer.object(index))
            .ok_or_else(|| format!("ground {}: packed object missing", asset.name))?;
        let joints: Vec<_> = match anim.target {
            ssb_rom::ground_obj::AnimTarget::Table => {
                ssb_rom::objanim::joint_scripts(&file.data, anim.script, graph.nodes.len())
                    .into_iter()
                    .enumerate()
                    .filter_map(|(node, script)| {
                        script.map(|script| (Some(script), Some(object.first_node + node as u32)))
                    })
                    .collect()
            }
            ssb_rom::ground_obj::AnimTarget::Node(node) => {
                vec![(Some(anim.script), Some(object.first_node + node as u32))]
            }
        };
        if joints.is_empty() {
            return Err(format!("ground {}: no animation scripts", anim.name).into());
        }
        ground_anim_joints += joints.len();
        ground_anims += 1;
        writer.add_anim(
            ssb_rom::pack::AnimDesc::GROUND,
            anim_index as u32,
            file_id,
            0,
            &file.data,
            &joints,
        );
    }

    // Their material animations (RE-364), keyed by `ground_obj::MAT_ANIMS`
    // index. Each joint names the `MatAnimDesc` its `MObj`'s primitives
    // carry; a cloud's names its leaf's (RE-365).
    let mut ground_mats = 0usize;
    let mut ground_mat_joints = 0usize;
    for (slot, asset) in ssb_rom::ground_obj::MAT_ANIMS.iter().enumerate() {
        let file_id = ground_map_file(&loaded, asset.gr_file, asset.map_head).ok_or_else(|| {
            format!(
                "ground material {}: map_nodes does not land on 0x{:X}",
                asset.name, asset.map_head
            )
        })?;
        let file = loaded
            .files
            .get(file_id as usize)
            .ok_or_else(|| format!("ground material {}: file {file_id} missing", asset.name))?;
        let mut joints = Vec::new();
        let graph_at = asset.graph;
        let graph = loaded
            .graphs
            .get(&file_id)
            .and_then(|graphs| graphs.iter().find(|graph| graph.offset == graph_at))
            .ok_or_else(|| {
                format!(
                    "ground material {}: graph 0x{graph_at:X} missing",
                    asset.name
                )
            })?;
        let materials = loaded.materials(&file, graph);
        let tables = ground_mat_scripts(&loaded, &file, graph_at, &materials);
        let (_, table) = tables
            .iter()
            .find(|(s, _)| *s == slot)
            .ok_or_else(|| format!("ground material {}: table unresolved", asset.name))?;
        for (node, chain) in table.iter().enumerate() {
            for (m, script) in chain.iter().enumerate() {
                let Some(script) = *script else { continue };
                let base = ground_mat_base(&tables, node, m).unwrap_or(script);
                let key = ssb_rom::mesh::MatAnimRef {
                    source_file: file_id,
                    script: base,
                    source_mobj: materials[node][m].at,
                };
                let target = mat_anim_index.get(&key).ok_or_else(|| {
                    format!(
                        "ground material {}: node {node} MObj {m} has no packed MatAnimDesc",
                        asset.name
                    )
                })?;
                joints.push((Some(script), Some(*target)));
            }
        }
        if joints.is_empty() {
            return Err(format!("ground material {}: no scripts", asset.name).into());
        }
        ground_mat_joints += joints.len();
        ground_mats += 1;
        writer.add_anim(
            ssb_rom::pack::AnimDesc::GROUND_MAT,
            slot as u32,
            file_id,
            0,
            &file.data,
            &joints,
        );
    }

    // Sector Z's Arwing (RE-428): file 153 (the flight paths `map_nodes`
    // points into) and file 161 (the Arwing's own scripts) whole, so every
    // script, `SYInterpDesc` and path pointer stays a file offset. A file an
    // earlier animation stored is shared only if it was stored whole.
    {
        use ssb_rom::sector;
        if ground_map_file(&loaded, sector::MAP_FILE, 0) != Some(sector::FLIGHT_FILE) {
            return Err("sector: map_nodes does not land on file 153 + 0".into());
        }
        for (slot, file_id) in [
            (sector::SLOT_FLIGHT, sector::FLIGHT_FILE),
            (sector::SLOT_ARWING, sector::ARWING_FILE),
        ] {
            let file = loaded
                .files
                .get(file_id as usize)
                .ok_or_else(|| format!("sector: file {file_id} missing"))?;
            if let Some(len) = writer.anim_file_len(file_id) {
                if len as usize != file.data.len() {
                    return Err(format!("sector: file {file_id} already packed in part").into());
                }
            }
            writer.add_anim(
                ssb_rom::pack::AnimDesc::SECTOR,
                slot,
                file_id,
                0,
                &file.data,
                &[],
            );
        }
    }

    // Shield poses go last, so every earlier animation keeps its index.
    // `frames` carries the `dobj_lookup` offset: the neutral pose the
    // runtime blends toward, read from the same file bytes as the scripts.
    let mut shield_pose_anims = 0usize;
    for (kind, refs, tables) in &deferred_shield_poses {
        let Some(file) = loaded.files.get(refs.file as usize) else {
            continue;
        };
        for (angle, joints) in tables.iter().enumerate() {
            writer.add_anim(
                ssb_rom::pack::AnimDesc::SHIELD_POSE,
                kind * ssb_rom::pack::AnimDesc::SHIELD_SECTORS + angle as u32,
                refs.file,
                refs.lookup,
                &file.data,
                joints,
            );
            shield_pose_anims += 1;
        }
    }

    // `FTAttributes.translate_scales` is an internal pointer into Luigi's
    // *Main file. Read the actual ROM vectors instead of copying decomp data.
    for entry in &ssb_rom::fighter::FIGHTER_FILES {
        let Some(main) = loaded.files.get(entry.file as usize) else {
            continue;
        };
        let Some(reloc) = main
            .intern_relocs
            .iter()
            .find(|r| r.at == entry.offset + 0x324)
        else {
            continue;
        };
        // FTPartsJoint enum: TopN, TransN, XRotN, YRotN, then 25
        // model joints. Luigi's source array has 29 Vec3f entries.
        let start = reloc.target as usize;
        let end = start + 29 * 12;
        let Some(scales) = main.data.get(start..end) else {
            return Err(format!("{}: invalid translation scales", entry.name).into());
        };
        writer.add_anim(
            ssb_rom::pack::AnimDesc::TRANSLATE_SCALES,
            entry.kind as u32,
            entry.file,
            0,
            scales,
            &[],
        );
    }

    writer.trim_descriptors();
    ctx.log.phase("anims");
    // Independent LBParticle banks live outside relocData. Decode and convert
    // them after scene work so their frame textures append without disturbing
    // any existing material texture indices (RE-180/181).
    let mut particle_scripts = 0usize;
    let mut particle_textures = 0usize;
    let mut particle_frames = 0usize;
    for &spec in ssb_rom::particle::BANKS {
        let scripts = rom.read(spec.scripts_lo, spec.scripts_hi - spec.scripts_lo)?;
        let textures = rom.read(spec.textures_lo, spec.textures_hi - spec.textures_lo)?;
        let (scripts, textures) = ssb_rom::particle::decode_bank_parts(&scripts, &textures)
            .map_err(|error| format!("{}: {error:?}", spec.name))?;
        let mut frames = Vec::with_capacity(textures.len());
        for texture in &textures {
            let mut packed = Vec::with_capacity(texture.images.len());
            for frame in 0..texture.images.len() {
                packed.push(
                    convert_particle_frame(texture, frame, swizzle).map_err(|error| {
                        format!(
                            "{} texture {} frame {frame}: {error}",
                            spec.name,
                            frames.len()
                        )
                    })?,
                );
            }
            particle_frames += packed.len();
            frames.push(packed);
        }
        writer.add_particle_bank(&scripts, &textures, &frames);
        particle_scripts += scripts.len();
        particle_textures += textures.len();
    }

    // The player interface shares its ROM file between the tree and mask.
    let interface_file = loaded
        .files
        .get(ssb_rom::player_interface::FILE as usize)
        .ok_or("player interface file missing")?;
    let frame =
        ssb_rom::player_interface::frame(&interface_file.data).ok_or("magnifier frame missing")?;
    let texture = add_sprite_texture(&mut writer, &frame, swizzle);
    writer.add_sprite(sprite_desc(
        ssb_rom::player_interface::FILE,
        ssb_rom::player_interface::FRAME,
        &frame,
        texture,
        0,
        0,
        0,
    ));
    let arrow_object = object_index
        .get(&(
            ssb_rom::player_interface::FILE,
            ssb_rom::player_interface::ARROWS,
        ))
        .and_then(|&i| writer.object(i))
        .ok_or("player arrow object missing")?;
    let joints: Vec<_> = ssb_rom::objanim::joint_scripts(
        &interface_file.data,
        ssb_rom::player_interface::ARROWS_ANIM,
        arrow_object.node_count as usize,
    )
    .into_iter()
    .enumerate()
    .filter_map(|(n, script)| script.map(|s| (Some(s), Some(arrow_object.first_node + n as u32))))
    .collect();
    writer.add_anim(
        ssb_rom::pack::AnimDesc::EFFECT,
        ssb_rom::player_interface::ARROWS_SLOT,
        ssb_rom::player_interface::FILE,
        0,
        &interface_file.data,
        &joints,
    );
    // The magnifier's color-image-to-Z draw needs the original IA8 samples.
    writer.add_anim(
        ssb_rom::pack::AnimDesc::EFFECT,
        ssb_rom::player_interface::ARROWS_SLOT + 1,
        ssb_rom::player_interface::FILE,
        0,
        &interface_file.data,
        &[],
    );

    // `ftShadowProcDisplay` loads this I4 image directly and writes its
    // own four-to-eight vertices every
    // frame.  Extract its first 16x16 frame explicitly rather than hoping an
    // effect bank happens to retain the same bytes.  The N64 uses mirror+wrap
    // on both axes; bake one mirrored 32x32 period so the PSP can repeat it
    // with ordinary `sceGuTexWrap(Repeat)` without changing the source UVs.
    let shadow_file = loaded
        .files
        .get(84)
        .ok_or("fighter shadow source file 84 missing")?;
    let shadow = fighter_shadow_texture(&shadow_file.data, swizzle)?;
    writer.add_fighter_shadow_texture(&shadow);

    // sc1PIntro cameras play once, with no update process. Preserve their
    // ten scalar tracks in native format; eye/at overrides remain in the host.
    let intro = archive.load(ssb_rom::campaign::INTRO.file)?;
    let mut cameras = Vec::new();
    for offset in *ssb_rom::campaign::CAMERA_OFFSETS {
        let values = ssb_rom::campaign::initial_camera(&intro.data, offset)?;
        for value in values {
            cameras.extend_from_slice(&value.to_le_bytes());
        }
    }
    writer.add_anim(
        ssb_rom::pack::AnimDesc::EFFECT,
        ssb_rom::campaign::CAMERA_SLOT,
        ssb_rom::campaign::INTRO.file,
        0,
        &cameras,
        &[],
    );

    // Master Hand's two camera animations (`gmCameraSetStatusAnim`) from
    // Final Destination's layer file, baked one frame per play. Their
    // synthetic source ids keep `add_anim` from sharing a blob.
    let last = archive.load(ssb_rom::campaign::BOSS_CAMERA_FILE)?;
    for (offset, slot) in [
        (
            ssb_rom::campaign::BOSS_INTRO_CAMERA,
            ssb_rom::campaign::BOSS_INTRO_CAMERA_SLOT,
        ),
        (
            ssb_rom::campaign::BOSS_DEFEAT_CAMERA,
            ssb_rom::campaign::BOSS_DEFEAT_CAMERA_SLOT,
        ),
    ] {
        let mut bytes = Vec::new();
        for frame in ssb_rom::campaign::camera_frames(&last.data, offset)? {
            for value in frame {
                bytes.extend_from_slice(&value.to_le_bytes());
            }
        }
        writer.add_anim(ssb_rom::pack::AnimDesc::EFFECT, slot, slot, 0, &bytes, &[]);
    }

    // The staff roll's credits tables: `ovl59`'s `.data`, which no archive
    // file holds (`ssb_game::spgame::staffroll::Credits`).
    let (at, len) = ssb_rom::ending::credits_range();
    let credits = rom
        .read(at, len)
        .map_err(|e| format!("ovl59's credits: {e}"))?;
    let credits = &credits[..];
    if ssb_game::spgame::staffroll::Credits::parse(credits).is_none() {
        return Err("ovl59's credits tables do not parse".into());
    }
    writer.add_anim(
        ssb_rom::pack::AnimDesc::EFFECT,
        ssb_rom::ending::CREDITS_SLOT,
        ssb_rom::ending::CREDITS_SLOT,
        0,
        credits,
        &[],
    );
    // `mvEndingSetupOperatorCamera`'s camera animation, baked one frame per
    // play as Master Hand's are.
    let ending = archive.load(ssb_rom::ending::ENDING_FILE)?;
    let mut bytes = Vec::new();
    for frame in ssb_rom::campaign::camera_frames(&ending.data, ssb_rom::ending::ENDING_CAMERA)? {
        for value in frame {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
    }
    writer.add_anim(
        ssb_rom::pack::AnimDesc::EFFECT,
        ssb_rom::ending::ENDING_CAMERA_SLOT,
        ssb_rom::ending::ENDING_CAMERA_SLOT,
        0,
        &bytes,
        &[],
    );
    // The room's prop animations and the staff roll's path and tilt run
    // from their files' own bytes: carry each whole file.
    for (file, slot) in [
        (
            ssb_rom::ending::ROOM_FILE,
            ssb_rom::ending::ROOM_SCRIPTS_SLOT,
        ),
        (
            ssb_rom::ending::STAFFROLL_FILE,
            ssb_rom::ending::STAFFROLL_SCRIPTS_SLOT,
        ),
    ] {
        let f = loaded
            .files
            .get(file as usize)
            .ok_or_else(|| format!("file {file} missing"))?;
        if writer
            .anim_file_len(file)
            .is_some_and(|len| len as usize != f.data.len())
        {
            return Err(format!("file {file}'s animation blob is not the whole file").into());
        }
        writer.add_anim(ssb_rom::pack::AnimDesc::EFFECT, slot, file, 0, &f.data, &[]);
    }

    // `SObj` sprites (RE-392): each through its format's combiner, in 8888,
    // clamped on both axes like a texture rectangle.
    let mut sprites = 0usize;
    for f in ssb_rom::sprite::FILES.iter() {
        let file = loaded
            .files
            .get(f.file as usize)
            .ok_or_else(|| format!("sprite file {} missing", f.file))?;
        for &at in f.offsets {
            let s = ssb_rom::sprite::decode(&file, at)
                .map_err(|e| format!("sprite {}+{at:#x}: {e:?}", f.file))?;
            // The unlock message's collage is CI4 through an RGBA16
            // palette: 5551 holds it exactly at half 8888's size, as the
            // wallpapers (RE-419).
            let texture = if (f.file, at)
                == (
                    ssb_rom::ending::MESSAGE_COLLAGE.file,
                    ssb_rom::ending::COLLAGE,
                ) {
                let image = ssb_rom::sprite::psp_image(&s);
                let tex = ssb_rom::psp_texture::pack_rgba(
                    &image,
                    ssb_rom::psp_texture::Psm::Psm5551,
                    swizzle,
                );
                writer.add_texture(&tex, true, true)
            } else {
                add_sprite_texture(&mut writer, &s, swizzle)
            };
            writer.add_sprite(sprite_desc(f.file, at, &s, texture, 0, 0, 0));
            sprites += 1;
        }
    }
    // The congratulations pictures: 300 x 110 RGBA32, 24-bit throughout.
    // Each is cut at column 256 so its two parts pad to 256 and 64 texels
    // rather than 512, and stays exact in 8888.
    for f in &ssb_rom::ending::CONGRA {
        let file = loaded
            .files
            .get(f.file as usize)
            .ok_or_else(|| format!("congratulations file {} missing", f.file))?;
        let s = ssb_rom::sprite::decode(&file, ssb_rom::ending::CONGRA_SPRITE)
            .map_err(|e| format!("congratulations picture {}: {e:?}", f.file))?;
        for (at, part) in [
            (
                ssb_rom::ending::CONGRA_SPRITE,
                ssb_rom::ending::congra_part(&s, 0),
            ),
            (
                ssb_rom::ending::CONGRA_SPRITE_RIGHT,
                ssb_rom::ending::congra_part(&s, 1),
            ),
        ] {
            let texture = add_sprite_texture(&mut writer, &part, swizzle);
            writer.add_sprite(sprite_desc(f.file, at, &part, texture, 0, 0, 0));
            sprites += 1;
        }
    }
    // The staff roll's name and job letters (`scStaffrollInitNameAndJobDisplayLists`):
    // raw I4 images, each cut to the texels its quad samples, keyed by
    // their offset in file 195.
    let staffroll = loaded
        .files
        .get(ssb_rom::ending::STAFFROLL_FILE as usize)
        .ok_or("staff roll file 195 missing")?;
    for (i, &at) in ssb_rom::ending::NAME_IMAGES.iter().enumerate() {
        let [w, h] = ssb_game::spgame::staffroll::NAME_GLYPHS[i];
        let s = ssb_rom::ending::name_glyph(&staffroll, at, u16::from(w), u16::from(h))
            .map_err(|e| format!("staff roll letter {i} at {at:#x}: {e:?}"))?;
        let texture = add_sprite_texture(&mut writer, &s, swizzle);
        writer.add_sprite(sprite_desc(
            ssb_rom::ending::STAFFROLL_FILE,
            at,
            &s,
            texture,
            0,
            0,
            0,
        ));
        sprites += 1;
    }
    // `sc1PTrainingModeLoadSprites`: the layout file's external pointers
    // select sprites in file 29. Keep the table-slot identity in the pack
    // and the original signed Vec2h positions, without runtime relocations.
    let training = loaded
        .files
        .get(ssb_rom::sprite::TRAINING_FILE as usize)
        .ok_or("Training sprite table missing")?;
    for slot in ssb_rom::sprite::training_sprite_slots() {
        let (file_id, at) = ssb_rom::sprite::training_sprite_ref(&training, slot)
            .map_err(|e| format!("Training sprite pointer {slot:#x}: {e:?}"))?;
        let file = loaded
            .files
            .get(file_id as usize)
            .ok_or("Training sprite source missing")?;
        let s = ssb_rom::sprite::decode(&file, at)
            .map_err(|e| format!("Training sprite {file_id}+{at:#x}: {e:?}"))?;
        let texture = add_sprite_texture(&mut writer, &s, swizzle);
        writer.add_sprite(sprite_desc(
            ssb_rom::sprite::TRAINING_FILE,
            slot,
            &s,
            texture,
            0,
            0,
            0,
        ));
        sprites += 1;
    }
    writer.add_anim(
        ssb_rom::pack::AnimDesc::TRAINING_LAYOUT,
        0,
        ssb_rom::sprite::TRAINING_FILE,
        0,
        training
            .data
            .get(..ssb_rom::sprite::TRAINING_LAYOUT_LEN)
            .ok_or("Training layout truncated")?,
        &[],
    );

    // The VS gate card once per `mnPlayersVSSetGateLUT` TLUT (RE-411).
    let common = loaded
        .files
        .get(ssb_rom::sprite::PLAYERS_COMMON.file as usize)
        .ok_or("sprite file 17 missing")?;
    for lut in 0..ssb_rom::sprite::GATE_LUTS.len() {
        let s = ssb_rom::sprite::decode_gate(&common, lut)
            .map_err(|e| format!("gate card {lut}: {e:?}"))?;
        let texture = add_sprite_texture(&mut writer, &s, swizzle);
        writer.add_sprite(sprite_desc(
            ssb_rom::sprite::PLAYERS_COMMON.file,
            ssb_rom::sprite::GATE_CARD,
            &s,
            texture,
            0,
            ssb_rom::pack::SpriteDesc::ROLE_LUT,
            lut as u8,
        ));
        sprites += 1;
    }
    // The Training select's card once per `mnPlayers1PTrainingSetGateLUT`
    // TLUT: the player's red, the CPU's grey.
    let card_file = loaded
        .files
        .get(ssb_rom::sprite::PLAYERS_1P_MODE_FILE as usize)
        .ok_or("sprite file 23 missing")?;
    for (lut, &(lut_file, _)) in ssb_rom::sprite::TRAINING_GATE_LUTS.iter().enumerate() {
        let lut_file = loaded
            .files
            .get(lut_file as usize)
            .ok_or_else(|| format!("sprite file {lut_file} missing"))?;
        let s = ssb_rom::sprite::decode_training_gate(&card_file, &lut_file, lut)
            .map_err(|e| format!("training gate card {lut}: {e:?}"))?;
        let texture = add_sprite_texture(&mut writer, &s, swizzle);
        writer.add_sprite(sprite_desc(
            ssb_rom::sprite::PLAYERS_1P_MODE_FILE,
            ssb_rom::sprite::TRAINING_GATE_CARD,
            &s,
            texture,
            0,
            ssb_rom::pack::SpriteDesc::ROLE_LUT,
            lut as u8,
        ));
        sprites += 1;
    }
    // The wallpapers (RE-419): each VS stage's `MPGroundData.wallpaper`,
    // keyed by its `GRKind`, then Training's three. They are 300 x 220
    // RGBA16, so 5551 keeps every texel exact at half 8888's size.
    let add_wallpaper = |writer: &mut ssb_rom::pack::PackWriter,
                         file: u32,
                         at: u32,
                         role: u8,
                         gkind: u8|
     -> Result<(), String> {
        let f = loaded
            .files
            .get(file as usize)
            .ok_or_else(|| format!("wallpaper file {file} missing"))?;
        let s = ssb_rom::sprite::decode(&f, at)
            .map_err(|e| format!("wallpaper {file}+{at:#x}: {e:?}"))?;
        let tex = ssb_rom::psp_texture::pack_rgba_trimmed(
            &s.image,
            ssb_rom::psp_texture::Psm::Psm5551,
            swizzle,
        );
        let texture = writer.add_texture(&tex, true, true);
        writer.add_sprite(sprite_desc(file, at, &s, texture, 0, role, gkind));
        Ok(())
    };
    let vs = (0u8..).zip(ssb_rom::stage::VS_GROUND_FILES.map(|_| *ssb_rom::stage::MAP_HEADER));
    let others = ssb_rom::stage::ONE_P_WALLPAPER_GROUNDS
        .iter()
        .copied()
        .chain([ssb_rom::stage::explain_wallpaper_ground()]);
    for (gkind, header) in vs.chain(others) {
        let map_id = ssb_rom::stage::COMMON_GROUND_FILES[usize::from(gkind)];
        let map = loaded
            .files
            .get(map_id as usize)
            .ok_or_else(|| format!("stage file {map_id} missing"))?;
        let (file, at) = ssb_rom::stage::wallpaper(&map, header)
            .ok_or_else(|| format!("stage file {map_id}: no wallpaper"))?;
        add_wallpaper(
            &mut writer,
            file,
            at,
            ssb_rom::pack::SpriteDesc::ROLE_WALLPAPER,
            gkind,
        )?;
        sprites += 1;
    }
    // The bonus stages' wallpaper (`grWallpaperMakeStatic` for every
    // `gkind >= nGRKindBonusStageStart`): each Break the Targets and Board
    // the Platforms map names one sprite, packed once under every kind.
    let mut bonus: Option<((u32, u32), ssb_rom::sprite::Sprite, u32)> = None;
    for gkind in ssb_rom::stage::BONUS_WALLPAPER_GROUNDS {
        let map_id = ssb_rom::scene_roots::ground_map(u32::from(gkind))
            .ok_or_else(|| format!("bonus kind {gkind}: no map"))?;
        let map = loaded
            .files
            .get(map_id as usize)
            .ok_or_else(|| format!("stage file {map_id} missing"))?;
        let target = ssb_rom::stage::wallpaper(&map, *ssb_rom::stage::BONUS_MAP_HEADER)
            .ok_or_else(|| format!("stage file {map_id}: no wallpaper"))?;
        if bonus.as_ref().is_none_or(|(t, _, _)| *t != target) {
            let (file, at) = target;
            let f = loaded
                .files
                .get(file as usize)
                .ok_or_else(|| format!("wallpaper file {file} missing"))?;
            let s = ssb_rom::sprite::decode(&f, at)
                .map_err(|e| format!("wallpaper {file}+{at:#x}: {e:?}"))?;
            let tex = ssb_rom::psp_texture::pack_rgba_trimmed(
                &s.image,
                ssb_rom::psp_texture::Psm::Psm5551,
                swizzle,
            );
            let texture = writer.add_texture(&tex, true, true);
            bonus = Some((target, s, texture));
        }
        let Some(((file, at), s, texture)) = bonus.as_ref() else {
            unreachable!()
        };
        writer.add_sprite(sprite_desc(
            *file,
            *at,
            s,
            *texture,
            0,
            ssb_rom::pack::SpriteDesc::ROLE_WALLPAPER,
            gkind,
        ));
        sprites += 1;
    }
    for file in *ssb_rom::sprite::TRAINING_WALLPAPER_FILES {
        add_wallpaper(
            &mut writer,
            file,
            *ssb_rom::sprite::TRAINING_WALLPAPER_SPRITE,
            ssb_rom::pack::SpriteDesc::ROLE_NONE,
            0,
        )?;
        sprites += 1;
    }
    // Each playable fighter's `FTSprites` (RE-393): the emblem, and the stock
    // icon once per `stock_luts` costume.
    let file_of = |id: u32| {
        loaded
            .files
            .get(id as usize)
            .ok_or_else(|| format!("file {id} missing"))
    };
    // Every fighter's emblem and stock icon, which the 1P Game's interface
    // draws for its enemies: the twelve, Master Hand, Metal Mario, the
    // Polygons and Giant Donkey Kong (RE-458). The Polygons have Master
    // Hand's emblem and no stock icon; their team's interface draws its
    // own (`FTStocksZako`).
    for e in ssb_rom::fighter::FIGHTER_FILES.iter() {
        let main = file_of(e.file)?;
        let fs = ssb_rom::sprite::fighter_sprites(&main, e.offset)
            .ok_or_else(|| format!("{}: no FTSprites", e.name))?;
        if let Some((file, at)) = fs.emblem {
            let s = ssb_rom::sprite::decode(&*file_of(file)?, at)
                .map_err(|err| format!("{} emblem: {err:?}", e.name))?;
            let texture = add_sprite_texture(&mut writer, &s, swizzle);
            writer.add_sprite(sprite_desc(
                file,
                at,
                &s,
                texture,
                e.kind,
                ssb_rom::pack::SpriteDesc::ROLE_EMBLEM,
                0,
            ));
            sprites += 1;
        }
        let Some((file, at)) = fs.stock else {
            continue;
        };
        for (costume, &(lut_file, lut_at)) in fs.stock_luts.iter().enumerate() {
            let lut_file = file_of(lut_file)?;
            let lut = lut_file
                .data
                .get(lut_at as usize..lut_at as usize + 32)
                .ok_or_else(|| format!("{} stock LUT {costume}", e.name))?;
            let s = ssb_rom::sprite::decode_with_tlut(
                &*file_of(file)?,
                at,
                &ssb_rom::texture::parse_tlut(lut),
            )
            .map_err(|err| format!("{} stock: {err:?}", e.name))?;
            let texture = add_sprite_texture(&mut writer, &s, swizzle);
            writer.add_sprite(sprite_desc(
                file,
                at,
                &s,
                texture,
                e.kind,
                ssb_rom::pack::SpriteDesc::ROLE_STOCK,
                costume as u8,
            ));
            sprites += 1;
        }
    }

    ctx.log.phase("particles+interface");
    // The scenes' own sprites, plays and cameras (RE-475; the menu packs
    // before it, RE-461).
    let menu_sprites = add_menu_scenes(&mut writer, &loaded.files, swizzle)?;
    // Baked plays keyed by a slot load with the file their scene lists:
    // Master Hand's cameras with his main file, the credits with the staff
    // roll's, the ending's camera with `MVEnding` (RE-475).
    for (slot, file) in [
        (
            ssb_rom::campaign::BOSS_INTRO_CAMERA_SLOT,
            ssb_rom::scene_roots::FIGHTER_MAIN[12],
        ),
        (
            ssb_rom::campaign::BOSS_DEFEAT_CAMERA_SLOT,
            ssb_rom::scene_roots::FIGHTER_MAIN[12],
        ),
        (
            ssb_rom::ending::CREDITS_SLOT,
            ssb_rom::scene_roots::STAFFROLL[0],
        ),
        (
            ssb_rom::ending::ENDING_CAMERA_SLOT,
            ssb_rom::scene_roots::ENDING[1],
        ),
    ] {
        writer.set_owner(slot, file);
    }
    pack_log!(ctx, "  menu scenes {menu_sprites} sprites");
    // Each file's extern IDs, so a runtime loading a file loads what it
    // points at (`lbRelocLoadFilesExtern`, RE-475).
    for id in 0..archive.len() as u32 {
        if let Ok(deps) = archive.extern_ids(id) {
            writer.set_file_deps(id, &deps);
        }
    }
    ctx.log.phase("menus+deps");
    let (hits, decodes) = loaded.files.stats();
    pack_log!(
        ctx,
        "  file store  {hits} hits, {decodes} decodes of {} files (RE-588)",
        loaded.files.len()
    );
    // Only the writer is needed from here (RE-588): the write is the peak.
    let shield_fighters = deferred_shield_poses.len();
    drop((
        loaded,
        tex_index,
        mat_anim_index,
        mat_anim_data,
        object_index,
        deferred_shield_poses,
    ));
    pack_log!(ctx, "  audio section {audio_len} bytes");
    writer.set_audio_later(audio_len);
    pack_log!(ctx, "  tables section {} bytes", tables.len());
    writer.set_tables(tables);
    ctx.log.phase("audio");
    let (held, raw) = writer.blob_held();
    let spill = writer.spill_stats();
    pack_log!(
        ctx,
        "  blob store  {held} bytes held for {raw} raw ({} chunks, RE-588); \
         spilled {} bytes in {} pages, {} reads",
        writer.chunk_count(),
        spill.bytes_written,
        spill.appends,
        spill.reads
    );
    writer
        .finish_to(sink, &mut |out| ssb_rom::audio::write_section(rom, out))
        .map_err(|e| format!("pack write: {e}"))?;
    ctx.log.phase("finish");
    let stats = PackStats {
        meshes,
        triangles,
        sprites,
        objects,
        placed_meshes,
        node_dls,
        extra_leaves,
        costume_overrides_added,
        with_collision,
        resolved_layers,
        stage_layers,
        fighters,
        fighters_failed,
        packed_anims,
        anim_joints_packed,
        hidden_joint_anims,
        walked_anims,
        anims_failed,
        stage_anims,
        stage_anim_joints,
        transition_anim_joints,
        effect_anims,
        effect_anim_joints,
        weapon_anims,
        weapon_anim_rebased,
        monster_anims,
        ground_anims,
        ground_anim_joints,
        ground_mats,
        ground_mat_joints,
        shield_pose_anims,
        particle_scripts,
        particle_textures,
        particle_frames,
        shield_fighters,
        wide_tiles: ctx.wide_tiles.get(),
        lod_blend_sites: ctx.lod_blend_sites.take(),
        non_owner_first: ctx.non_owner_first.get(),
    };
    Ok(stats)
}
