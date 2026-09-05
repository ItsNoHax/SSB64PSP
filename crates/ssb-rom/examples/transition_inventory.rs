//! Verifies and replays the eleven original LB screen-transition animations.
//!
//! `cargo run -p ssb-rom --example transition_inventory -- <rom.z64> [ssb64.pak]`

use ssb_rom::objanim::StageJoint;
use ssb_rom::scene::find_scene_graphs;
use ssb_rom::{rom, Archive};

const MAX_FRAMES: u32 = 1200;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let path = args.next().ok_or("expected ROM path")?;
    let bytes = std::fs::read(path)?;
    let info = rom::identify(&bytes)?;
    let archive = Archive::open(&bytes, info.region)?;

    println!("id\tname\tfile\tgraph\tnodes\tscripts\tframes");
    for (id, asset) in ssb_rom::transition::ASSETS.iter().enumerate() {
        let file = archive.load(asset.file)?;
        let graph = find_scene_graphs(&file)
            .into_iter()
            .find(|graph| graph.offset == asset.graph)
            .ok_or_else(|| format!("{}: graph 0x{:X} missing", asset.name, asset.graph))?;
        let scripts =
            ssb_rom::objanim::joint_scripts(&file.data, asset.anim_joints, graph.nodes.len());
        let active: Vec<_> = scripts.into_iter().flatten().collect();
        if active.is_empty() {
            return Err(format!("{}: no animation scripts", asset.name).into());
        }
        let mut joints: Vec<_> = active
            .iter()
            .map(|&script| (StageJoint::start(script, 0.0), Default::default()))
            .collect();
        let mut frames = 0;
        while joints.iter().any(|(joint, _)| !joint.ended()) && frames < MAX_FRAMES {
            for (joint, pose) in &mut joints {
                joint.tick(&file.data, 1.0, pose)?;
            }
            frames += 1;
        }
        if joints.iter().any(|(joint, _)| !joint.ended()) {
            return Err(format!("{}: animation exceeds {MAX_FRAMES} frames", asset.name).into());
        }
        if frames != asset.frames {
            return Err(format!(
                "{}: expected {} frames, replayed {frames}",
                asset.name, asset.frames
            )
            .into());
        }
        println!(
            "{id}\t{}\t{}\t0x{:X}\t{}\t{}\t{frames}",
            asset.name,
            asset.file,
            asset.graph,
            graph.nodes.len(),
            active.len()
        );
    }

    if let Some(pack_path) = args.next() {
        verify_pack(&std::fs::read(pack_path)?)?;
    }
    Ok(())
}

fn verify_pack(bytes: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
    use ssb_rom::skeleton::{StageAnimator, MAX_NODES};

    let pack = ssb_rom::pack::Pack::open(bytes).map_err(|error| format!("{error:?}"))?;
    for (id, asset) in ssb_rom::transition::ASSETS.iter().enumerate() {
        let anim = pack
            .transition_anim(id as u32)
            .ok_or_else(|| format!("{}: packed animation missing", asset.name))?;
        let object = pack
            .transition_object(&anim)
            .ok_or_else(|| format!("{}: packed object missing", asset.name))?;
        let script = pack
            .anim_script(&anim)
            .ok_or_else(|| format!("{}: packed script missing", asset.name))?;
        let mut player = StageAnimator::new();
        player.start(&pack, &anim);
        if player.joint_count() == 0 {
            return Err(format!("{}: no packed joints", asset.name).into());
        }
        let mut matrices = [ssb_rom::scene::Mat4::IDENTITY; MAX_NODES];
        for _ in 0..asset.frames {
            player.tick(script)?;
            player.compose(&pack, &object, &mut matrices);
        }
        if !player.ended() {
            return Err(format!("{}: packed animation did not end", asset.name).into());
        }
    }
    println!("packed\t11 transitions replayed and object-bound");
    Ok(())
}
