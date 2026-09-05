//! Measures whether packed stage animations drive billboard rotations.
//!
//! ```text
//! cargo run -p ssb-rom --example billboard_animation_inventory -- \
//!   assets/generated/ssb64.pak
//! ```

use ssb_rom::pack::{NodeDesc, Pack};
use ssb_rom::scene::Mat4;
use ssb_rom::skeleton::{StageAnimator, MAX_NODES};
use std::collections::BTreeSet;

const FRAMES: usize = 240;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args().nth(1).ok_or("expected pack path")?;
    let bytes = std::fs::read(path)?;
    let pack = Pack::open(&bytes).map_err(|e| format!("{e:?}"))?;
    let mut animated_billboards = 0u32;
    let mut selected_spin_changes = 0u32;
    let mut stage_refs = 0u32;
    let mut fighter_refs = 0u32;
    let mut referenced_nodes = BTreeSet::new();
    let mut inherited_only = BTreeSet::new();
    let mut inherited_that_move = BTreeSet::new();
    let mut inherited_moving_at_final_frame = BTreeSet::new();

    // First establish whether a different animation family can reach a
    // billboard. Stage and fighter scripts use different encodings.
    for anim_index in 0..pack.anim_count() {
        let Some(anim) = pack.anim(anim_index) else {
            continue;
        };
        for joint_index in 0..anim.joint_count {
            let Some(joint) = pack.anim_joint(anim.first_joint + joint_index) else {
                continue;
            };
            let Some(node) = pack.node(joint.node) else {
                continue;
            };
            if node.flags & NodeDesc::FLAG_BILLBOARD == 0 {
                continue;
            }
            referenced_nodes.insert(joint.node);
            if anim.fighter == ssb_rom::pack::AnimDesc::STAGE {
                stage_refs += 1;
            } else {
                fighter_refs += 1;
            }
        }
    }

    println!(
        "stage\tfile\tgraph\tlocal_node\tpack_node\tpitch_locked\tspin_z_kind\t\
         min_rx\tmax_rx\tmin_ry\tmax_ry\tmin_rz\tmax_rz\tselected_spin_changes\t\
         translation_changes\tmin_sx\tmax_sx\tmin_sy\tmax_sy\tmin_sz\tmax_sz"
    );

    for stage_index in 0..pack.stage_count() {
        let Some(anim) = pack.stage_anim(stage_index) else {
            continue;
        };
        let script = pack
            .anim_script(&anim)
            .ok_or("missing stage animation bytes")?;
        let mut player = StageAnimator::new();
        player.start(&pack, &anim);

        struct Range {
            node: u32,
            min: [f32; 3],
            max: [f32; 3],
            initial_spin: f32,
            spin_changed: bool,
            initial_translate: [f32; 3],
            translation_changed: bool,
            min_scale: [f32; 3],
            max_scale: [f32; 3],
        }

        let mut ranges = Vec::new();
        for joint in 0..player.joint_count() {
            let Some((node_index, pose)) = player.joint(joint) else {
                continue;
            };
            let Some(node) = pack.node(node_index) else {
                continue;
            };
            if node.flags & NodeDesc::FLAG_BILLBOARD == 0 {
                continue;
            }
            let spin = if node.flags & NodeDesc::FLAG_BILLBOARD_SPIN_Z != 0 {
                pose.rotate[2]
            } else {
                0.0
            };
            ranges.push(Range {
                node: node_index,
                min: pose.rotate,
                max: pose.rotate,
                initial_spin: spin,
                spin_changed: false,
                initial_translate: pose.translate,
                translation_changed: false,
                min_scale: pose.scale,
                max_scale: pose.scale,
            });
        }

        let driven: BTreeSet<_> = (0..player.joint_count())
            .filter_map(|joint| player.joint(joint).map(|(node, _)| node))
            .collect();
        let mut stage_inherited = Vec::new();
        if let Some(stage) = pack.stage(stage_index) {
            for object_index in stage.layers {
                let Some(object) = pack.object(object_index) else {
                    continue;
                };
                for local in 0..object.node_count {
                    let node_index = object.first_node + local;
                    let Some(node) = pack.node(node_index) else {
                        continue;
                    };
                    if node.flags & NodeDesc::FLAG_BILLBOARD == 0 || driven.contains(&node_index) {
                        continue;
                    }
                    let mut parent = node.parent;
                    while parent != NodeDesc::NO_PARENT {
                        if driven.contains(&parent) {
                            inherited_only.insert((stage_index, node_index));
                            stage_inherited.push(node_index);
                            break;
                        }
                        parent = pack
                            .node(parent)
                            .map_or(NodeDesc::NO_PARENT, |ancestor| ancestor.parent);
                    }
                }
            }
        }

        for frame in 0..FRAMES {
            player.tick(script)?;
            for range in &mut ranges {
                let (_, pose) = (0..player.joint_count())
                    .filter_map(|i| player.joint(i))
                    .find(|(node, _)| *node == range.node)
                    .ok_or("animated billboard joint disappeared")?;
                for axis in 0..3 {
                    range.min[axis] = range.min[axis].min(pose.rotate[axis]);
                    range.max[axis] = range.max[axis].max(pose.rotate[axis]);
                }
                let node = pack.node(range.node).ok_or("missing billboard node")?;
                let spin = if node.flags & NodeDesc::FLAG_BILLBOARD_SPIN_Z != 0 {
                    pose.rotate[2]
                } else {
                    0.0
                };
                range.spin_changed |= spin != range.initial_spin;
                range.translation_changed |= pose.translate != range.initial_translate;
                for axis in 0..3 {
                    range.min_scale[axis] = range.min_scale[axis].min(pose.scale[axis]);
                    range.max_scale[axis] = range.max_scale[axis].max(pose.scale[axis]);
                }
            }
            if let Some(stage) = pack.stage(stage_index) {
                for object_index in stage.layers {
                    let Some(object) = pack.object(object_index) else {
                        continue;
                    };
                    let mut posed = [Mat4::IDENTITY; MAX_NODES];
                    player.compose(&pack, &object, &mut posed);
                    for &node_index in &stage_inherited {
                        if node_index < object.first_node
                            || node_index - object.first_node >= object.node_count
                        {
                            continue;
                        }
                        let local = (node_index - object.first_node) as usize;
                        let rest = pack.node(node_index).ok_or("missing inherited node")?.world;
                        if posed[local]
                            .0
                            .iter()
                            .zip(rest)
                            .any(|(a, b)| (a - b).abs() > 1e-6)
                        {
                            inherited_that_move.insert((stage_index, node_index));
                            if frame + 1 == FRAMES {
                                inherited_moving_at_final_frame.insert((stage_index, node_index));
                            }
                        }
                    }
                }
            }
        }

        for range in ranges {
            let node = pack.node(range.node).ok_or("missing billboard node")?;
            let owners: Vec<_> = (0..pack.object_count())
                .filter_map(|i| pack.object(i))
                .filter(|o| range.node >= o.first_node && range.node - o.first_node < o.node_count)
                .collect();
            if owners.len() != 1 {
                return Err(format!(
                    "node {}: expected one owner, got {}",
                    range.node,
                    owners.len()
                )
                .into());
            }
            let owner = owners[0];
            let pitch_locked = node.flags & NodeDesc::FLAG_BILLBOARD_PITCH_LOCKED != 0;
            let spin_z = node.flags & NodeDesc::FLAG_BILLBOARD_SPIN_Z != 0;
            animated_billboards += 1;
            selected_spin_changes += u32::from(range.spin_changed);
            println!(
                "{stage_index}\t{}\t0x{:X}\t{}\t{}\t{pitch_locked}\t{spin_z}\t\
                 {}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
                owner.source_file,
                owner.source_offset,
                range.node - owner.first_node,
                range.node,
                range.min[0],
                range.max[0],
                range.min[1],
                range.max[1],
                range.min[2],
                range.max[2],
                range.spin_changed,
                range.translation_changed,
                range.min_scale[0],
                range.max_scale[0],
                range.min_scale[1],
                range.max_scale[1],
                range.min_scale[2],
                range.max_scale[2]
            );
        }
    }

    eprintln!(
        "stage_animations={} frames_each={FRAMES} animated_billboards={animated_billboards} \
         selected_spin_changes={selected_spin_changes} stage_billboard_refs={stage_refs} \
         fighter_billboard_refs={fighter_refs} unique_referenced_nodes={} \
         inherited_only_billboards={} inherited_billboards_that_move={} \
         inherited_moving_at_final_frame={}",
        (0..pack.stage_count())
            .filter(|&stage| pack.stage_anim(stage).is_some())
            .count(),
        referenced_nodes.len(),
        inherited_only.len(),
        inherited_that_move.len(),
        inherited_moving_at_final_frame.len()
    );
    for (stage, node_index) in &inherited_that_move {
        let owner = (0..pack.object_count())
            .filter_map(|i| pack.object(i))
            .find(|o| *node_index >= o.first_node && *node_index - o.first_node < o.node_count)
            .ok_or("moving inherited billboard has no owner")?;
        eprintln!(
            "inherited_moving stage={stage} file={} graph=0x{:X} local_node={} pack_node={node_index}",
            owner.source_file,
            owner.source_offset,
            node_index - owner.first_node
        );
    }
    for (stage, node_index) in &inherited_moving_at_final_frame {
        eprintln!("inherited_moving_at_frame_{FRAMES} stage={stage} pack_node={node_index}");
    }
    Ok(())
}
