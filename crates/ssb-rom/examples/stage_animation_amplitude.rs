//! Ranks animated stage nodes by how far they actually move, to find a
//! candidate visible enough for a physical-PSP "stage animation works" R2
//! capture (`STATUS.md` 2026-09-10: no existing golden scene's frozen camera
//! window shows visibly animated stage geometry).
//!
//! ```text
//! cargo run -p ssb-rom --example stage_animation_amplitude -- \
//!   assets/generated/ssb64.pak
//! ```

use ssb_rom::pack::{NodeDesc, Pack};
use ssb_rom::skeleton::StageAnimator;

const FRAMES: usize = 240;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args().nth(1).ok_or("expected pack path")?;
    let bytes = std::fs::read(path)?;
    let pack = Pack::open(&bytes).map_err(|e| format!("{e:?}"))?;

    struct Row {
        stage: u32,
        node: u32,
        has_mesh: bool,
        billboard: bool,
        rest: [f32; 3],
        translate_amplitude: f32,
        rotate_amplitude_deg: f32,
        owner_file: u32,
        owner_offset: u32,
    }

    let mut rows = Vec::new();

    for stage_index in 0..pack.stage_count() {
        let Some(anim) = pack.stage_anim(stage_index) else {
            continue;
        };
        let script = pack
            .anim_script(&anim)
            .ok_or("missing stage animation bytes")?;
        let mut player = StageAnimator::new();
        player.start(&pack, &anim);

        struct Track {
            node: u32,
            min_t: [f32; 3],
            max_t: [f32; 3],
            min_r: [f32; 3],
            max_r: [f32; 3],
        }
        let mut tracks: Vec<Track> = (0..player.joint_count())
            .filter_map(|joint| player.joint(joint))
            .map(|(node, pose)| Track {
                node,
                min_t: pose.translate,
                max_t: pose.translate,
                min_r: pose.rotate,
                max_r: pose.rotate,
            })
            .collect();

        for _ in 0..FRAMES {
            player.tick(script)?;
            for track in &mut tracks {
                let Some((_, pose)) = (0..player.joint_count())
                    .filter_map(|i| player.joint(i))
                    .find(|(node, _)| *node == track.node)
                else {
                    continue;
                };
                for axis in 0..3 {
                    track.min_t[axis] = track.min_t[axis].min(pose.translate[axis]);
                    track.max_t[axis] = track.max_t[axis].max(pose.translate[axis]);
                    track.min_r[axis] = track.min_r[axis].min(pose.rotate[axis]);
                    track.max_r[axis] = track.max_r[axis].max(pose.rotate[axis]);
                }
            }
        }

        for track in tracks {
            let Some(node) = pack.node(track.node) else {
                continue;
            };
            let translate_amplitude = (0..3)
                .map(|axis| track.max_t[axis] - track.min_t[axis])
                .fold(0.0f32, f32::max);
            let rotate_amplitude_deg = (0..3)
                .map(|axis| (track.max_r[axis] - track.min_r[axis]).to_degrees())
                .fold(0.0f32, f32::max);
            let owner = (0..pack.object_count())
                .filter_map(|i| pack.object(i))
                .find(|o| track.node >= o.first_node && track.node - o.first_node < o.node_count);
            rows.push(Row {
                stage: stage_index,
                node: track.node,
                has_mesh: node.mesh != NodeDesc::NO_MESH,
                billboard: node.flags & NodeDesc::FLAG_BILLBOARD != 0,
                rest: [node.world[12], node.world[13], node.world[14]],
                translate_amplitude,
                rotate_amplitude_deg,
                owner_file: owner.map_or(u32::MAX, |o| o.source_file),
                owner_offset: owner.map_or(0, |o| o.source_offset),
            });
        }
    }

    rows.sort_by(|a, b| {
        b.rotate_amplitude_deg
            .max(b.translate_amplitude)
            .total_cmp(&a.rotate_amplitude_deg.max(a.translate_amplitude))
    });

    println!(
        "stage\tnode\thas_mesh\tbillboard\towner_file\towner_offset\trest_x\trest_y\trest_z\ttranslate_amp\trotate_amp_deg"
    );
    for row in rows.iter().filter(|r| !r.billboard && r.has_mesh).take(40) {
        println!(
            "{}\t{}\t{}\t{}\t{}\t0x{:X}\t{:.3}\t{:.3}\t{:.3}\t{:.4}\t{:.2}",
            row.stage,
            row.node,
            row.has_mesh,
            row.billboard,
            row.owner_file,
            row.owner_offset,
            row.rest[0],
            row.rest[1],
            row.rest[2],
            row.translate_amplitude,
            row.rotate_amplitude_deg
        );
    }

    Ok(())
}
