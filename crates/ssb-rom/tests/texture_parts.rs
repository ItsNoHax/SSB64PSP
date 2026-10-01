//! RE-426: ROM-backed face tables and both fighter detail models.
use ssb_game::fighter::FighterKind;
use ssb_rom::{
    fighter,
    pack::{modelpart_costume, NodeDesc, Pack, TextureDesc},
    Archive,
};

fn inputs() -> Option<(Vec<u8>, Vec<u8>)> {
    let rom = std::fs::read(std::env::var_os("SSB64_ROM")?).unwrap();
    let pack = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../assets/generated/ssb64.pak"
    ))
    .unwrap();
    Some((rom, pack))
}

#[test]
fn texture_part_joint_and_material_positions_equal_the_rom() {
    let Some((rom, _)) = inputs() else { return };
    let a = Archive::open(&rom, ssb_rom::rom::identify(&rom).unwrap().region).unwrap();
    for &kind in FighterKind::PLAYABLE {
        let e = *fighter::FIGHTER_FILES
            .iter()
            .find(|e| e.kind == kind as u8)
            .unwrap();
        let f = a.load(e.file).unwrap();
        assert_eq!(
            fighter::texture_parts(&f, e).map(|t| t.map(|p| (p.joint, p.detail))),
            ssb_game::modelpart::texture_part_table(kind),
            "{kind:?}"
        );
    }
}

#[test]
fn both_details_pack_the_source_models_fallbacks_and_reachable_parts() {
    let Some((rom, bytes)) = inputs() else { return };
    let p = Pack::open(&bytes).unwrap();
    let a = Archive::open(&rom, ssb_rom::rom::identify(&rom).unwrap().region).unwrap();
    for &kind in FighterKind::PLAYABLE {
        let e = *fighter::FIGHTER_FILES
            .iter()
            .find(|e| e.kind == kind as u8)
            .unwrap();
        let f = a.load(e.file).unwrap();
        let refs = fighter::common_parts(&f, e);
        let models = p.fighter_model(kind as u32).unwrap();
        let high = p.object(models.high).unwrap();
        let mut wanted = ssb_game::motion::model_part_events(kind);
        if kind == FighterKind::Link {
            wanted.extend([(19, 0), (21, 0)]);
        }
        if kind == FighterKind::Yoshi {
            wanted.insert((7, 1));
        }
        if kind == FighterKind::Kirby {
            wanted.extend(
                ssb_game::kirby_copy::COPY_MODELPART_IDS
                    .iter()
                    .map(|&id| (6, i32::from(id))),
            );
        }
        for (detail, index) in [models.high, models.low].into_iter().enumerate() {
            let r = refs[detail].unwrap();
            let o = p.object(index).unwrap();
            assert_eq!((o.source_file, o.source_offset), (r.model_file, r.graph));
            assert_eq!(o.node_count, high.node_count);
            let file = a.load(r.model_file).unwrap();
            let graphs = ssb_rom::scene::find_scene_graphs(&file);
            let graph = graphs.iter().find(|g| g.offset == r.graph).unwrap();
            let hi = graphs
                .iter()
                .find(|g| g.offset == refs[0].unwrap().graph)
                .unwrap();
            let mask = ssb_game::modelpart::joint_masks(kind).unwrap().0;
            for (n, node) in graph.nodes.iter().enumerate() {
                // Unmade joints (Yoshi 5 and Samus 17..25) never draw;
                // the converter may merge their lists into another slot.
                if mask >> n & 1 == 0 {
                    continue;
                }
                if node.desc.dl.or(hi.nodes[n].desc.dl).is_some() {
                    let mesh = p.node(o.first_node + n as u32).unwrap().mesh;
                    assert_ne!(mesh, NodeDesc::NO_MESH, "{kind:?} detail {detail} node {n}");
                    // Identical lists can deduplicate across source offsets.
                    // A missing low list must retain the high model's mesh.
                    if detail == 1 && node.desc.dl.is_none() {
                        let hi_mesh = p
                            .mesh(p.node(high.first_node + n as u32).unwrap().mesh)
                            .unwrap();
                        let lo_mesh = p.mesh(mesh).unwrap();
                        assert_eq!(
                            (lo_mesh.vertex_count, lo_mesh.prim_count),
                            (hi_mesh.vertex_count, hi_mesh.prim_count),
                            "{kind:?} low fallback geometry node {n}"
                        );
                    }
                }
            }
            for &(joint, part) in &wanted {
                if part < 0 || joint < 4 || mask >> (joint - 4) & 1 == 0 {
                    continue;
                }
                let mp =
                    fighter::model_part(&f, e, joint as u32, part as u32, detail as u32).unwrap();
                if let Some(tps) = fighter::texture_parts(&f, e) {
                    for (texture_part, tp) in tps
                        .iter()
                        .enumerate()
                        .filter(|(_, tp)| i32::from(tp.joint) == joint)
                    {
                        let count = mp
                            .mobjsubs
                            .map(|(file, at)| {
                                ssb_rom::mobj::read_chain(&a.load(file).unwrap(), at)
                                    .unwrap()
                                    .len()
                            })
                            .unwrap_or(0);
                        let exists = count > usize::from(tp.detail[detail]);
                        let mut state = ssb_game::modelpart::ModelParts::new(kind);
                        state.set_detail_all(if detail == 0 {
                            ssb_game::modelpart::Detail::High
                        } else {
                            ssb_game::modelpart::Detail::Low
                        });
                        state.set(joint as u8, part as i8);
                        state.set_texture(texture_part, 1);
                        assert_eq!(
                            state.texture.curr[texture_part] == 1,
                            exists,
                            "{kind:?} joint {joint} part {part} detail {detail} material presence"
                        );
                    }
                }
                for costume in 0..4 {
                    let node = o.first_node + joint as u32 - 4;
                    let mesh = p
                        .costume_mesh(node, modelpart_costume(part as u32, costume))
                        .or_else(|| p.costume_mesh(node, modelpart_costume(part as u32, 0)))
                        .or_else(|| p.costume_mesh(node, costume))
                        .unwrap_or_else(|| p.node(node).unwrap().mesh);
                    let mesh = p.mesh(mesh).unwrap();
                    assert_eq!(
                        (mesh.source_file, mesh.source_offset),
                        mp.dl,
                        "{kind:?} detail {detail} joint {joint} part {part} costume {costume}"
                    );
                }
            }
        }
    }
}

#[test]
fn packed_texture_parts_have_every_reachable_sprite_and_valid_texture() {
    let Some((_, bytes)) = inputs() else { return };
    let p = Pack::open(&bytes).unwrap();
    assert!(p.texture_part_count() > 100);
    for i in 0..p.texture_part_count() {
        let t = p.texture_part_at(i).unwrap();
        assert!(t.part < 2);
        let mesh = p.mesh(t.mesh).unwrap();
        assert!(t.prim < mesh.prim_count);
        assert_eq!(
            t.textures[0],
            p.prim(mesh.first_prim + t.prim).unwrap().texture
        );
        let kind = FighterKind::PLAYABLE
            .iter()
            .copied()
            .find(|&k| {
                let e = fighter::FIGHTER_FILES
                    .iter()
                    .find(|e| e.kind == k as u8)
                    .unwrap();
                // Both detail models and their costume/part meshes stay in the fighter's model file.
                p.object(p.fighter_model(k as u32).unwrap().high)
                    .unwrap()
                    .source_file
                    == mesh.source_file
                    && e.kind == k as u8
            })
            .unwrap();
        for (part, id) in ssb_game::motion::texture_part_events(kind) {
            if part != t.part as i32 || id <= 0 {
                continue;
            }
            let tex = t.textures[id as usize];
            assert_ne!(
                tex,
                TextureDesc::NO_ANIM,
                "{kind:?} mesh {} part {part} sprite {id}",
                t.mesh
            );
            assert!(p.texture(tex).is_some());
        }
    }
}
