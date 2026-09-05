//! Repeatable rest-pose inventory, not a visual correctness verdict.
//! cargo run -p ssb-rom --example billboard_inventory -- assets/generated/ssb64.pak
use ssb_rom::pack::{NodeDesc, Pack};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args().nth(1).ok_or("expected pack path")?;
    let bytes = std::fs::read(path)?;
    let pack = Pack::open(&bytes).map_err(|e| format!("{e:?}"))?;
    let mut count = 0;
    let mut pitch_locked = 0;
    let mut anomalies = 0;
    let mut spinning_kind = 0;
    println!("file\tgraph\tlocal_node\tpack_node\tmesh\tpitch_locked\tfinite\tscale_x\tscale_y\tscale_z\tx\ty\tz\trotate_x\trotate_y\trotate_z\tspin_z_kind\tspin_angle");
    for i in 0..pack.node_count() {
        let node = pack.node(i).ok_or("missing node")?;
        if node.flags & NodeDesc::FLAG_BILLBOARD == 0 {
            continue;
        }
        let owners: Vec<_> = (0..pack.object_count())
            .filter_map(|j| pack.object(j))
            .filter(|o| i >= o.first_node && i - o.first_node < o.node_count)
            .collect();
        if owners.len() != 1 {
            return Err(format!("node {i}: expected one owner, got {}", owners.len()).into());
        }
        let owner = owners[0];
        let w = node.world;
        let length = |c: usize| w[c].hypot(w[c + 1]).hypot(w[c + 2]);
        let scale = [length(0), length(4), length(8)];
        let finite = w.iter().all(|v| v.is_finite());
        // Report degeneracy without guessing bounds for authored positions.
        // Zero scale may be intentional; it requires investigation, not repair.
        if !finite || scale.contains(&0.0) {
            anomalies += 1;
        }
        let locked = node.flags & NodeDesc::FLAG_BILLBOARD_PITCH_LOCKED != 0;
        pitch_locked += u32::from(locked);
        let spin_z = node.flags & NodeDesc::FLAG_BILLBOARD_SPIN_Z != 0;
        spinning_kind += u32::from(spin_z);
        count += 1;
        println!(
            "{}\t0x{:X}\t{}\t{i}\t{}\t{locked}\t{finite}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{spin_z}\t{}",
            owner.source_file,
            owner.source_offset,
            i - owner.first_node,
            node.mesh,
            scale[0],
            scale[1],
            scale[2],
            w[12],
            w[13],
            w[14],
            node.rest_rotate[0],
            node.rest_rotate[1],
            node.rest_rotate[2],
            node.billboard_rest_spin()
        );
    }
    eprintln!("billboards={count} pitch_locked={pitch_locked} spin_z_kind={spinning_kind} structural_anomalies={anomalies}; visual verification pending");
    if count == 0 || anomalies != 0 {
        return Err("empty inventory or structural anomalies require investigation".into());
    }
    Ok(())
}
