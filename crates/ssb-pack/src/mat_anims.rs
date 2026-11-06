//! Material-animation resolution: stage layers, grounds, bosses (RE-089,
//! RE-090, RE-091).

use crate::*;

pub fn map_vertex_bindings(mesh: &mut ssb_rom::mesh::Mesh, plan: &[PlannedList]) {
    for v in &mut mesh.vertices {
        if let Some((item, pos)) = v.binding {
            let node = plan[item as usize].space.map_or(u16::MAX - 1, |n| n as u16);
            v.binding = Some((node, pos));
        }
    }
}

/// Long enough that a looping script proves it loops (RE-089's own budget
/// for the same replay).
pub const MAT_ANIM_REPLAY_FRAMES: u32 = 600;

/// Runs one material animation script to see whether it drives anything a
/// packed primitive can attach: a real `PaletteID` cycle (RE-089/RE-090/
/// RE-091), a texture-id cycle (the manager-effect sprite frame lists --
/// CommonSpark and Poké Ball's eight/seven-frame tables), or a colour track
/// (`crate::matanim::MaterialJoint::track_color`, e.g. Link Spin Attack's
/// primitive-alpha ramp). `sub_at(node, m)` resolves a `(node,
/// MObj-chain-position)` to its `MObjSub`'s own file offset and CLUT entry
/// width -- the only source-backed bound on `palettes[]`/`sprites[]`;
/// `None` declines rather than guessing a table length.
// The independent source fields deliberately remain explicit here: callers
// resolve them at different stages of mesh/material extraction.
#[allow(clippy::too_many_arguments)]
pub fn resolve_one_mat_anim(
    file: &ssb_rom::archive::File,
    node: usize,
    m: usize,
    script: u32,
    sub_at: &impl Fn(usize, usize) -> Option<(u32, u16)>,
    base_tracks: [u32; 10],
    uv_mode: u8,
    uv_tile_params: [u16; 3],
) -> Option<(ssb_rom::mesh::MatAnimRef, MatAnimData)> {
    let mut j = ssb_rom::matanim::MaterialJoint::start(script, 0.0);
    let mut max_palette = 0.0f32;
    let mut palette_seen = false;
    let mut max_texture = 0.0f32;
    // The sprite indices the replay selects (RE-467: a sparse table).
    let mut used_textures = BTreeSet::new();
    let mut frames = 0u32;
    let mut seen_material = false;
    // RE-318: tile-0 UV tracks against their rest values, per replayed frame.
    let uv_rest: [f32; 4] = core::array::from_fn(|k| f32::from_bits(base_tracks[k + 1]));
    let mut uv_seen = [false; 4];
    let mut uv_replay_holds = true;
    let mut drives_lod = false;
    let mut max_current = 0.0f32;
    let lfrac_rest = f32::from_bits(base_tracks[ssb_rom::matanim::TRACK_SET_LFRAC]);
    let mut max_lod_frac = ssb_rom::lod_blend::prim_lod_frac(lfrac_rest);
    // `current_closed`: a parse that stops at a command the replay already
    // stopped at has run the whole cycle (jumps are unconditional).
    let mut stops = BTreeSet::from([j.pc()]);
    let mut last_stop = j.pc();
    let mut cycled = false;
    loop {
        // A decoder error means this script is not something this resolver
        // can trust the replay of -- decline it outright rather than attach
        // a partial read.
        j.tick(&file.data, 1.0).ok()?;
        frames += 1;
        if let Some(v) = j.track_value(ssb_rom::matanim::TRACK_PALETTE_ID) {
            max_palette = max_palette.max(v);
            palette_seen = true;
        }
        if let Some(v) = j.track_value(ssb_rom::matanim::TRACK_TEXTURE_ID_CURRENT) {
            max_texture = max_texture.max(v);
            max_current = max_current.max(v);
            used_textures.insert(v.max(0.0) as usize);
        }
        if let Some(v) = j.track_value(ssb_rom::matanim::TRACK_TEXTURE_ID_NEXT) {
            max_texture = max_texture.max(v);
            used_textures.insert(v.max(0.0) as usize);
        }
        seen_material |= (0..10).any(|track| j.track_value(track).is_some());
        if let Some(v) = j.track_value(ssb_rom::matanim::TRACK_SET_LFRAC) {
            max_lod_frac = max_lod_frac.max(ssb_rom::lod_blend::prim_lod_frac(v));
        }
        drives_lod |= j.track_value(ssb_rom::matanim::TRACK_SET_LFRAC).is_some()
            || j.track_value(ssb_rom::matanim::TRACK_TEXTURE_ID_NEXT)
                .is_some();
        for (k, seen) in uv_seen.iter_mut().enumerate() {
            if let Some(v) = j.track_value(ssb_rom::matanim::TRACK_TRA_U + k) {
                *seen = true;
                uv_replay_holds &= v == uv_rest[k];
            }
        }
        if j.pc() != last_stop {
            last_stop = j.pc();
            cycled |= !stops.insert(last_stop);
        }
        if j.ended() || j.looped() || cycled || frames >= MAT_ANIM_REPLAY_FRAMES {
            break;
        }
    }
    let current_closed = j.ended() || j.looped() || cycled;

    // `gcDrawMObjForDObj` draws `palettes[(s32)palette_id]` whatever kind
    // wrote it (RE-325), so neither gate requires a step and both indices
    // truncate (RE-326). `has_texture`: `gcPlayMObjMatAnim` assigns
    // `mobj->texture_id_curr = value` for *any* live kind (RE-175 measured
    // real manager scripts using a plain `Kind::Linear` ramp for this --
    // CommonSpark's own UV/texture-id stream among them, not a `_After`
    // step list the way DamageSlash's is), so requiring a step here would
    // wrongly decline a real, in-source frame selection. A colour track has
    // no such requirement either -- `track_color` already handles a smooth
    // `Kind::Linear` ramp correctly, unlike `track_value`.
    let has_palette = palette_seen;
    let has_texture = j
        .track_value(ssb_rom::matanim::TRACK_TEXTURE_ID_CURRENT)
        .is_some()
        || j.track_value(ssb_rom::matanim::TRACK_TEXTURE_ID_NEXT)
            .is_some();
    let has_color = (0..5).any(|i| {
        j.track_color(ssb_rom::matanim::TICK_EXT_START + i)
            .is_some()
    });
    if !has_palette && !has_texture && !has_color && !seen_material {
        return None;
    }

    let (sub_offset, palette_entries) = sub_at(node, m)?;
    let palette_count = if has_palette {
        max_palette.max(0.0) as u32 + 1
    } else {
        0
    };
    let palettes = if palette_count > 1 {
        ssb_rom::mobj::read_palettes(file, sub_offset, palette_count as usize)?
    } else {
        Vec::new()
    };
    let texture_count = if has_texture {
        max_texture.max(0.0) as u32 + 1
    } else {
        0
    };
    let sprites = if texture_count > 0 {
        match ssb_rom::mobj::read_sprites(file, sub_offset, texture_count as usize) {
            Some(s) => s,
            None if (ssb_rom::opening::file::MV_COMMON..=ssb_rom::opening::file::RUN_CRASH)
                .contains(&file.id)
                || file.id == GROUND_THUNDER_JOLT_FILE =>
            {
                sparse_opening_sprites(file, sub_offset, texture_count as usize, &used_textures)?
            }
            None => return None,
        }
    } else {
        Vec::new()
    };
    // A track that is live but never actually reaches a second index (e.g.
    // `PaletteID` stepped once to 0 and never touched again) resolved no
    // array and is not real cycling on its own -- decline unless something
    // else on this script is worth attaching.
    if palettes.is_empty() && sprites.is_empty() && !has_color && !seen_material {
        return None;
    }

    Some((
        ssb_rom::mesh::MatAnimRef {
            source_file: file.id,
            script,
            source_mobj: sub_offset,
        },
        MatAnimData {
            source_offset: sub_offset,
            palette_entries,
            palettes,
            sprites,
            base_tracks,
            uv_mode,
            uv_tile_params,
            // Filled by the caller, which holds the `MObjMaterial`.
            uv_half: [0; 3],
            // Every reachable write holds the rest value, and each written
            // track reached it in the replay without a transient: from then
            // on nothing can move it.
            uv_static: uv_replay_holds
                && ssb_rom::matanim::uv_writes_hold(&file.data, script, uv_rest)
                    .ok()
                    .flatten()
                    .is_some_and(|written| {
                        written.iter().zip(uv_seen).all(|(&w, seen)| !w || seen)
                    }),
            drives_lod,
            // `texture_id_curr` is a `u16`: the live float truncates.
            max_current: max_current.max(0.0) as usize,
            current_closed,
            max_lod_frac,
            extra_scripts: Vec::new(),
        },
    ))
}

/// File 342, Pikachu's Thunder Jolt. The ground jolt's node 4 `MObjSub`
/// (0x1170) names sprites `[0xC18, NULL, 0x408]` and its script (0x1B68)
/// selects only 0 and 2 (RE-542), so it takes the sparse table below.
pub const GROUND_THUNDER_JOLT_FILE: u32 = 342;

/// RE-467: an opening `MObjSub.sprites` array with NULL entries its script
/// never selects (the standoff's lightning: 0, 2, 3 and 4 of five, entry 1
/// NULL), which [`ssb_rom::mobj::read_sprites`] declines whole. The ground
/// Thunder Jolt's node 4 has the same shape (RE-542). Each NULL
/// entry the replay never reaches takes the first real one's place, so the
/// table keeps its indices; a NULL entry the script selects still declines.
pub fn sparse_opening_sprites(
    file: &ssb_rom::archive::File,
    sub_offset: u32,
    count: usize,
    used: &BTreeSet<usize>,
) -> Option<Vec<ssb_rom::mobj::Ptr>> {
    let entries: Vec<Option<ssb_rom::mobj::Ptr>> = (0..count)
        .map(|i| ssb_rom::mobj::read_sprite_at(file, sub_offset, i))
        .collect();
    if used
        .iter()
        .any(|&i| entries.get(i).is_none_or(|e| e.is_none()))
    {
        return None;
    }
    let fill = entries.iter().flatten().next().copied()?;
    Some(entries.into_iter().map(|e| e.unwrap_or(fill)).collect())
}

/// Resolves an `AObjEvent32 ***` joint table (RE-089's `p_matanim_joints`
/// shape) into one [`ssb_rom::mesh::MatAnimRef`] per `(node,
/// MObj-chain-position)`, stashing each resolved script's [`MatAnimData`]
/// into `mat_anim_data` for [`pack_mesh`] to consume. Generic over how a
/// caller locates each chain position's own `MObjSub` (`sub_at`): a stage
/// layer's external `p_mobjsub` table and a manager effect's `o_mobjsub`
/// table are two different pointers naming the same shape (RE-175), and
/// `materials` (already resolved through whichever one applies) already
/// carries the same offsets a third way for anything with a registered
/// [`Loaded::materials`] table -- see the two call sites below for which
/// each caller uses.
pub fn resolve_mat_anims(
    file: &ssb_rom::archive::File,
    matanim_table: u32,
    materials: &[ssb_rom::mobj::NodeMaterials],
    sub_at: impl Fn(usize, usize) -> Option<(u32, u16)>,
    mat_anim_data: &mut BTreeMap<ssb_rom::mesh::MatAnimRef, MatAnimData>,
) -> Vec<Vec<Option<ssb_rom::mesh::MatAnimRef>>> {
    let scripts = ssb_rom::matanim::resolve_scripts(file, matanim_table, materials.len(), |n| {
        materials[n].len()
    });
    resolve_script_matrix(file, &scripts, materials, sub_at, mat_anim_data)
}

/// [`resolve_mat_anims`] for a script per `(node, MObj-chain-position)`
/// already resolved: a table's, or one script a maker or status adds to
/// one `MObj` with `gcAddMObjMatAnimJoint`.
pub fn resolve_script_matrix(
    file: &ssb_rom::archive::File,
    scripts: &[Vec<Option<u32>>],
    materials: &[ssb_rom::mobj::NodeMaterials],
    sub_at: impl Fn(usize, usize) -> Option<(u32, u16)>,
    mat_anim_data: &mut BTreeMap<ssb_rom::mesh::MatAnimRef, MatAnimData>,
) -> Vec<Vec<Option<ssb_rom::mesh::MatAnimRef>>> {
    let mut refs: Vec<Vec<Option<ssb_rom::mesh::MatAnimRef>>> =
        scripts.iter().map(|c| vec![None; c.len()]).collect();
    for (node, chain) in scripts.iter().enumerate() {
        for (m, script) in chain.iter().enumerate() {
            let Some(script) = *script else { continue };
            let Some(material) = materials.get(node).and_then(|chain| chain.get(m)) else {
                continue;
            };
            let key = ssb_rom::mesh::MatAnimRef {
                source_file: file.id,
                script,
                source_mobj: material.at,
            };
            if mat_anim_data.contains_key(&key) {
                refs[node][m] = Some(key);
                continue;
            }
            let Some((r, mut data)) = resolve_one_mat_anim(
                file,
                node,
                m,
                script,
                &sub_at,
                material.mat_anim_tracks,
                material.mat_anim_uv_mode,
                material.mat_anim_tile_params,
            ) else {
                continue;
            };
            refs[node][m] = Some(r);
            data.uv_half = material.mat_anim_uv_half;
            mat_anim_data.insert(key, data);
        }
    }
    refs
}

/// The file `map_nodes` names for a controller's `GR*Map` file, once
/// `map_nodes` is confirmed to land on the label the controller subtracts
/// (RE-357).
pub fn ground_map_file(loaded: &Loaded, gr_file: u32, map_head: u32) -> Option<u32> {
    loaded
        .stages
        .iter()
        .find(|g| g.file == gr_file)?
        .map_nodes
        .filter(|&(_, head)| head == map_head)
        .map(|(file, _)| file)
}

/// The `ground_obj::MAT_ANIMS` tables a controller plays on the object
/// packed from `(file, graph_offset)`, each resolved to one script per
/// `(node, MObj-chain-position)` (RE-364). Empty for any other graph.
pub fn ground_mat_scripts(
    loaded: &Loaded,
    file: &ssb_rom::archive::File,
    graph_offset: u32,
    materials: &[ssb_rom::mobj::NodeMaterials],
) -> Vec<(usize, Vec<Vec<Option<u32>>>)> {
    ssb_rom::ground_obj::MAT_ANIMS
        .iter()
        .enumerate()
        .filter(|(_, m)| {
            m.graph == graph_offset
                && ground_map_file(loaded, m.gr_file, m.map_head) == Some(file.id)
        })
        .map(|(slot, m)| {
            let scripts = match m.target {
                ssb_rom::ground_obj::MatTarget::Table => {
                    ssb_rom::matanim::resolve_scripts(file, m.table, materials.len(), |n| {
                        materials[n].len()
                    })
                }
                // `gcAddMObjMatAnimJoint(dobj->mobj, script)` on one node.
                ssb_rom::ground_obj::MatTarget::NodeMObj(node) => materials
                    .iter()
                    .enumerate()
                    .map(|(n, chain)| {
                        (0..chain.len())
                            .map(|i| (n == usize::from(node) && i == 0).then_some(m.table))
                            .collect()
                    })
                    .collect(),
            };
            (slot, scripts)
        })
        .collect()
}

/// The script whose `MatAnimDesc` a ground object's `MObj` carries: the
/// first one, in `MAT_ANIMS` order, of all the controller may start on it.
/// The others restart the same entry at run time.
pub fn ground_mat_base(
    tables: &[(usize, Vec<Vec<Option<u32>>>)],
    node: usize,
    m: usize,
) -> Option<u32> {
    tables
        .iter()
        .find_map(|(_, t)| t.get(node).and_then(|c| c.get(m)).copied().flatten())
}

/// A ground object's material animations (RE-364). Several controller
/// tables can restart one `MObj` (Whispy's mouth has eight), but a
/// primitive names one `MatAnimDesc`. Every script shares the `MObj`'s
/// sprite and palette arrays, so the entry keeps the first script and the
/// widest arrays any of them reaches. A script that cycles palettes is
/// declined when another shares its `MObj`: the palettes reachable from the
/// first script alone are the ones [`pack_mesh`] converts.
pub fn resolve_ground_mat_anims(
    ctx: &PackCtx<'_>,
    file: &ssb_rom::archive::File,
    tables: &[(usize, Vec<Vec<Option<u32>>>)],
    materials: &[ssb_rom::mobj::NodeMaterials],
    mat_anim_data: &mut BTreeMap<ssb_rom::mesh::MatAnimRef, MatAnimData>,
) -> Vec<Vec<Option<ssb_rom::mesh::MatAnimRef>>> {
    let sub_at = |node: usize, m: usize| {
        materials
            .get(node)?
            .get(m)
            .map(|s| (s.at, s.palette_entries))
    };
    let mut refs: Vec<Vec<Option<ssb_rom::mesh::MatAnimRef>>> =
        materials.iter().map(|c| vec![None; c.len()]).collect();
    for (node, chain) in materials.iter().enumerate() {
        for (m, material) in chain.iter().enumerate() {
            let Some(base) = ground_mat_base(tables, node, m) else {
                continue;
            };
            let key = ssb_rom::mesh::MatAnimRef {
                source_file: file.id,
                script: base,
                source_mobj: material.at,
            };
            if mat_anim_data.contains_key(&key) {
                refs[node][m] = Some(key);
                continue;
            }
            let mut scripts: Vec<u32> = Vec::new();
            for (_, t) in tables {
                if let Some(s) = t.get(node).and_then(|c| c.get(m)).copied().flatten() {
                    if !scripts.contains(&s) {
                        scripts.push(s);
                    }
                }
            }
            let resolve = |script: u32| {
                resolve_one_mat_anim(
                    file,
                    node,
                    m,
                    script,
                    &sub_at,
                    material.mat_anim_tracks,
                    material.mat_anim_uv_mode,
                    material.mat_anim_tile_params,
                )
            };
            let Some((r, mut data)) = resolve(base) else {
                continue;
            };
            let mut declined = false;
            for &s in &scripts[1..] {
                let Some((_, other)) = resolve(s) else {
                    data.current_closed = false;
                    continue;
                };
                if !data.palettes.is_empty() || !other.palettes.is_empty() {
                    declined = true;
                    break;
                }
                if other.sprites.len() > data.sprites.len() {
                    data.sprites = other.sprites;
                }
                data.drives_lod |= other.drives_lod;
                data.max_current = data.max_current.max(other.max_current);
                data.current_closed &= other.current_closed;
                data.max_lod_frac = data.max_lod_frac.max(other.max_lod_frac);
                data.uv_static &= other.uv_static;
            }
            if declined {
                pack_diag!(ctx,
                    "ground material file {} MObj 0x{:X}: palette script shares its MObj; not attached",
                    file.id, material.at
                );
                continue;
            }
            refs[node][m] = Some(r);
            data.uv_half = material.mat_anim_uv_half;
            mat_anim_data.insert(key, data);
        }
    }
    refs
}

/// Final Destination's boss wallpaper effects (`sc1PGameBossMakeWallpaperEffect`):
/// each effect with materials adds its `o_matanim_joint` table to its tree
/// (`lbCommonAddTreeDObjsAnimAll`). `tables` are the tables of every effect
/// drawing this graph, in `EFFECT_ASSETS` order: `llGRLastMapEffects2_1`
/// serves two (wallpaper 2's palette loop and wallpaper 3's palette run).
/// A primitive names one `MatAnimDesc`, so the entry keeps the first
/// script, the widest `palettes[]`/`sprites[]` any of them reaches (each a
/// prefix of the same `MObjSub` arrays) and the other scripts, whose
/// palette states [`reachable_palettes`] adds; the runtime restarts the
/// entry on its own effect's script.
pub fn resolve_boss_mat_anims(
    file: &ssb_rom::archive::File,
    tables: &[u32],
    materials: &[ssb_rom::mobj::NodeMaterials],
    mat_anim_data: &mut BTreeMap<ssb_rom::mesh::MatAnimRef, MatAnimData>,
) -> Vec<Vec<Option<ssb_rom::mesh::MatAnimRef>>> {
    let sub_at = |node: usize, m: usize| {
        materials
            .get(node)?
            .get(m)
            .map(|s| (s.at, s.palette_entries))
    };
    let scripts: Vec<Vec<Vec<Option<u32>>>> = tables
        .iter()
        .map(|&t| {
            ssb_rom::matanim::resolve_scripts(file, t, materials.len(), |n| materials[n].len())
        })
        .collect();
    let mut refs: Vec<Vec<Option<ssb_rom::mesh::MatAnimRef>>> =
        materials.iter().map(|c| vec![None; c.len()]).collect();
    for (node, chain) in materials.iter().enumerate() {
        for (m, material) in chain.iter().enumerate() {
            let mut own: Vec<u32> = Vec::new();
            for t in &scripts {
                if let Some(s) = t.get(node).and_then(|c| c.get(m)).copied().flatten() {
                    if !own.contains(&s) {
                        own.push(s);
                    }
                }
            }
            let resolve = |script: u32| {
                resolve_one_mat_anim(
                    file,
                    node,
                    m,
                    script,
                    &sub_at,
                    material.mat_anim_tracks,
                    material.mat_anim_uv_mode,
                    material.mat_anim_tile_params,
                )
            };
            let mut base: Option<(ssb_rom::mesh::MatAnimRef, MatAnimData)> = None;
            let mut unresolved = false;
            for &s in &own {
                let Some((r, other)) = resolve(s) else {
                    unresolved = true;
                    continue;
                };
                let Some((_, data)) = base.as_mut() else {
                    base = Some((r, other));
                    continue;
                };
                if other.palettes.len() > data.palettes.len() {
                    data.palettes = other.palettes;
                }
                if other.sprites.len() > data.sprites.len() {
                    data.sprites = other.sprites;
                }
                data.drives_lod |= other.drives_lod;
                data.max_current = data.max_current.max(other.max_current);
                data.current_closed &= other.current_closed;
                data.max_lod_frac = data.max_lod_frac.max(other.max_lod_frac);
                data.uv_static &= other.uv_static;
                data.extra_scripts.push(s);
            }
            let Some((r, mut data)) = base else {
                continue;
            };
            refs[node][m] = Some(r);
            data.uv_half = material.mat_anim_uv_half;
            data.current_closed &= !unresolved;
            mat_anim_data.entry(r).or_insert(data);
        }
    }
    refs
}

/// For a scene graph's own material animation (a stage layer's
/// `MPGroundDesc::p_matanim_joints`, or one of the 26 manager-effect
/// `EFDesc::o_matanim_joint` tables with a non-NULL entry -- RE-175), resolves
/// which `(node, MObj-chain-position)` entries carry a script worth
/// attaching and stashes each one's data into `mat_anim_data`. Returns
/// all-`None` for any other graph, which is the overwhelming majority, so
/// this is cheap to call unconditionally per graph rather than precomputing
/// archive-wide.
///
/// Same-file only, matching RE-089's own scope limit: a layer whose
/// `p_matanim_joints`/`p_mobjsubs` cross into another archive file is left
/// alone rather than guessed at. Manager effects are the same way (RE-089's
/// own scan already resolves `o_matanim_joint` same-file only).
pub fn resolve_layer_mat_anims(
    ctx: &PackCtx<'_>,
    loaded: &Loaded,
    file: &ssb_rom::archive::File,
    graph_offset: u32,
    materials: &[ssb_rom::mobj::NodeMaterials],
    mat_anim_data: &mut BTreeMap<ssb_rom::mesh::MatAnimRef, MatAnimData>,
) -> Vec<Vec<Option<ssb_rom::mesh::MatAnimRef>>> {
    let empty = || materials.iter().map(|c| vec![None; c.len()]).collect();

    if file.id == ssb_rom::bonus2::FILE {
        if let Some(tree) = ssb_rom::bonus2::TREES
            .iter()
            .find(|t| t[0] == graph_offset && t[3] != 0)
        {
            return resolve_mat_anims(
                file,
                tree[3],
                materials,
                |n, m| {
                    materials
                        .get(n)
                        .and_then(|c| c.get(m))
                        .map(|m| (m.at, m.palette_entries))
                },
                mat_anim_data,
            );
        }
    }

    if file.id == ssb_game::spgame::boss::EFFECT_FILE {
        let tables: Vec<u32> = ssb_game::spgame::boss::EFFECT_ASSETS
            .iter()
            .flatten()
            .flatten()
            .filter(|a| a.graph == graph_offset)
            .filter_map(|a| a.matanim_joint)
            .collect();
        if !tables.is_empty() {
            return resolve_boss_mat_anims(file, &tables, materials, mat_anim_data);
        }
    }

    // Ground effects (`efGroundMakeEffect`): each desc drawing this graph
    // adds its own `o_matanim_joint` table to the tree
    // (`lbCommonAddTreeDObjsAnimAll`); descs share graphs (Sector Z's two
    // ships, Dream Land's 0x33B8 with two tables), as the boss wallpapers do.
    let effect_tables: Vec<u32> = (0..ssb_game::ground_effect::DESC_COUNT)
        .filter(|&k| {
            ssb_game::ground_effect::desc_file(k) == Some(file.id)
                && ssb_game::ground_effect::O_DOBJSETUP[k] == graph_offset
                && ssb_game::ground_effect::O_MATANIM_JOINT[k] != 0
        })
        .map(|k| ssb_game::ground_effect::O_MATANIM_JOINT[k])
        .collect();
    if !effect_tables.is_empty() {
        return resolve_boss_mat_anims(file, &effect_tables, materials, mat_anim_data);
    }

    let ground = ground_mat_scripts(loaded, file, graph_offset, materials);
    if !ground.is_empty() {
        return resolve_ground_mat_anims(ctx, file, &ground, materials, mat_anim_data);
    }

    // Manager effects: `materials` is already resolved through whichever
    // `o_mobjsub` table applies (`Loaded::materials`, fed by `PartTables`/
    // the hand-entered pairs this file's `load()` builds), so a chain
    // position's own `MObjSub` offset is just `materials[node][m].at` --
    // no second, table lookup needed the way a stage layer's external
    // `p_mobjsub` array requires below.
    if let Some(idx) = ssb_rom::effect::MANAGER_EFFECT_KEYS
        .iter()
        .position(|&k| k == (file.id, graph_offset))
    {
        let Some(mat) = ssb_rom::effect::MANAGER_EFFECT_MAT_ANIM_JOINTS[idx] else {
            return empty();
        };
        return resolve_mat_anims(
            file,
            mat,
            materials,
            |node, m| {
                materials
                    .get(node)?
                    .get(m)
                    .map(|s| (s.at, s.palette_entries))
            },
            mat_anim_data,
        );
    }

    // Weapon trees: `WPAttributes.p_matanim_joints`, against the materials
    // its `p_mobjsubs` table already resolved, as for a manager effect.
    if let Some(&(_, mat)) = WEAPON_MAT_ANIM_JOINTS
        .iter()
        .find(|(key, _)| *key == (file.id, graph_offset))
    {
        return resolve_mat_anims(
            file,
            mat,
            materials,
            |node, m| {
                materials
                    .get(node)?
                    .get(m)
                    .map(|s| (s.at, s.palette_entries))
            },
            mat_anim_data,
        );
    }

    // The opening movie's trees and lists (RE-467): `gcAddMatAnimJointAll`
    // against the `MObjSub` table the hand-entered pairings resolved.
    if let Some(&(_, mat)) = ssb_rom::opening::MAT_ANIM_JOINTS
        .iter()
        .find(|(key, _)| *key == (file.id, graph_offset))
    {
        return resolve_mat_anims(
            file,
            mat,
            materials,
            |node, m| {
                materials
                    .get(node)?
                    .get(m)
                    .map(|s| (s.at, s.palette_entries))
            },
            mat_anim_data,
        );
    }

    // RE-442: the items' material scripts, against the materials their
    // `ITAttributes.p_mobjsubs` tables resolved.
    if let Some(target) = item_mat_target((file.id, graph_offset)) {
        let scripts: Vec<Vec<Option<u32>>> = match target {
            ItemMatTarget::Table(table) => {
                ssb_rom::matanim::resolve_scripts(file, table, materials.len(), |n| {
                    materials[n].len()
                })
            }
            ItemMatTarget::Root(script) | ItemMatTarget::Node(_, script) => {
                let node = match target {
                    ItemMatTarget::Node(node, _) => node,
                    _ => ITEM_ROOT_NODE,
                };
                materials
                    .iter()
                    .enumerate()
                    .map(|(n, chain)| {
                        (0..chain.len())
                            .map(|m| (n == node && m == 0).then_some(script))
                            .collect()
                    })
                    .collect()
            }
        };
        return resolve_script_matrix(
            file,
            &scripts,
            materials,
            |node, m| {
                materials
                    .get(node)?
                    .get(m)
                    .map(|s| (s.at, s.palette_entries))
            },
            mat_anim_data,
        );
    }

    // RE-420: the series emblems' `llFTEmblemModels*MatAnimJoint`
    // (`mnVSResultsMakeEmblem`'s `matanim_joints[]`), against the materials
    // their `MObjSub` tables resolved, as for a weapon tree.
    if let Some(e) = ssb_rom::effect::EMBLEMS
        .iter()
        .find(|e| (ssb_rom::effect::EMBLEM_FILE, e.dobjdesc) == (file.id, graph_offset))
    {
        return resolve_mat_anims(
            file,
            e.matanim_joint,
            materials,
            |node, m| {
                materials
                    .get(node)?
                    .get(m)
                    .map(|s| (s.at, s.palette_entries))
            },
            mat_anim_data,
        );
    }

    let Some(layer) = loaded
        .stages
        .iter()
        .flat_map(|s| &s.layers)
        .find(|l| l.graph == (file.id, graph_offset))
    else {
        return empty();
    };
    let (Some((mf, mat)), Some((tf, at))) = (layer.matanim_joints, layer.mobjsub_table) else {
        return empty();
    };
    if mf != file.id || tf != file.id {
        return empty();
    }
    let Some(chain_table) = ssb_rom::mobj::read_table(file, at, materials.len()) else {
        return empty();
    };
    resolve_mat_anims(
        file,
        mat,
        materials,
        |node, m| {
            chain_table
                .nodes
                .get(node)?
                .get(m)
                .map(|s| (s.at, s.palette_entries))
        },
        mat_anim_data,
    )
}
